#[macro_export]
macro_rules! tunable {
    ($name: ident: $type: ty = $value: expr; range($min: expr, $max: expr)) => {
        #[cfg(feature = "tune")]
        static $name: $crate::tune::inner::TunableValue<$type> = {
            static INNER: $crate::tune::inner::MakeSync<$type> = $crate::tune::inner::MakeSync::new($value);
            $crate::tune::inner::TunableValue::new(&INNER)
        };
        #[cfg(feature = "tune")]
        inventory::submit! {
            $crate::tune::inner::Value::new(&stringify!($name), &$name, $min, $max)
        }
        #[cfg(not(feature = "tune"))]
        const $name: $type = $value;
    };
}

#[cfg(feature = "tune")]
pub mod inner {
    use std::{
        cell::UnsafeCell,
        collections::BTreeMap,
        error::Error,
        fmt::Display,
        ops::{Add, AddAssign, Deref, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
        str::FromStr,
    };

    trait TunableType {
        const OB_SPSA_TYPE: &'static str;

        fn ob_spsa_c_end(min: &Self, max: &Self) -> f64;
    }

    macro_rules! impl_tunable_type_int {
        ($($type: ty);*) => {
            $(
                impl TunableType for $type {
                    const OB_SPSA_TYPE: &'static str = "int";

                    fn ob_spsa_c_end(min: &Self, max: &Self) -> f64 {
                        (*max as f64 - *min as f64) / 20.0
                    }
                }
            )*
        };
    }
    macro_rules! impl_tunable_type_float {
        ($($type: ty);*) => {
            $(
                impl TunableType for $type {
                    const OB_SPSA_TYPE: &'static str = "float";

                    fn ob_spsa_c_end(min: &Self, max: &Self) -> f64 {
                        (*max as f64 - *min as f64) / 20.0
                    }
                }
            )*
        };
    }

    impl_tunable_type_int!(i8; i16; i32; i64; i128; isize; u8; u16; u32; u64; u128; usize);
    impl_tunable_type_float!(f32; f64);

    pub struct Value<T: Sync + 'static> {
        name: &'static str,
        value: &'static TunableValue<T>,
        min: T,
        max: T,
    }

    impl<T: Sync + 'static> Value<T> {
        pub const fn new(name: &'static str, value: &'static TunableValue<T>, min: T, max: T) -> Self {
            Self { name, value, min, max }
        }
    }

    pub struct MakeSync<T: Sync>(UnsafeCell<T>);

    impl<T: Sync> MakeSync<T> {
        pub const fn new(value: T) -> Self {
            Self(UnsafeCell::new(value))
        }
    }

    unsafe impl<T: Sync> Sync for MakeSync<T> {}

    #[derive(Copy, Clone)]
    pub struct TunableValue<T: Sync + 'static>(&'static MakeSync<T>);

    impl<T: Sync> TunableValue<T> {
        pub const fn new(value: &'static MakeSync<T>) -> Self {
            Self(value)
        }

        unsafe fn set(&self, value: T) {
            unsafe {
                *self.0.0.get() = value;
            }
        }
    }

    impl<T: Sync + Copy> crate::tune::Val for TunableValue<T> {
        type Type = T;
        fn val(&self) -> Self::Type {
            **self
        }
    }

    impl<T: Sync> Deref for TunableValue<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            unsafe { &*self.0.0.get() }
        }
    }

    macro_rules! impl_binop {
        ($trait: ident, $fn: ident, $ty: ty) => {
            impl $trait<$ty> for TunableValue<$ty> {
                type Output = <$ty as $trait<$ty>>::Output;

                fn $fn(self, rhs: $ty) -> Self::Output {
                    (*self).$fn(rhs)
                }
            }

            impl $trait<TunableValue<$ty>> for $ty {
                type Output = <$ty as $trait<$ty>>::Output;

                fn $fn(self, rhs: TunableValue<$ty>) -> Self::Output {
                    self.$fn(*rhs)
                }
            }
        };
    }

    macro_rules! impl_assign_binop {
        ($trait: ident, $fn: ident, $ty: ty) => {
            impl $trait<TunableValue<$ty>> for $ty {
                fn $fn(&mut self, rhs: TunableValue<$ty>) {
                    self.$fn(*rhs)
                }
            }
        };
    }

    macro_rules! impl_ord {
        ($ty: ty) => {
            impl PartialEq<$ty> for TunableValue<$ty> {
                fn eq(&self, other: &$ty) -> bool {
                    **self == *other
                }
            }

            impl PartialEq<TunableValue<$ty>> for $ty {
                fn eq(&self, other: &TunableValue<$ty>) -> bool {
                    *self == **other
                }
            }

            impl PartialOrd<$ty> for TunableValue<$ty> {
                fn partial_cmp(&self, other: &$ty) -> Option<std::cmp::Ordering> {
                    (**self).partial_cmp(other)
                }
            }

            impl PartialOrd<TunableValue<$ty>> for $ty {
                fn partial_cmp(&self, other: &TunableValue<$ty>) -> Option<std::cmp::Ordering> {
                    self.partial_cmp(&**other)
                }
            }
        };
    }

    macro_rules! impl_primitive_types {
        ($($type: ty);*) => {
            $(
                impl_binop!(Mul, mul, $type);
                impl_assign_binop!(MulAssign, mul_assign, $type);
                impl_binop!(Div, div, $type);
                impl_assign_binop!(DivAssign, div_assign, $type);
                impl_binop!(Add, add, $type);
                impl_assign_binop!(AddAssign, add_assign, $type);
                impl_binop!(Sub, sub, $type);
                impl_assign_binop!(SubAssign, sub_assign, $type);
                impl_ord!($type);
                inventory::collect!(Value<$type>);
            )*
        };
    }

    impl_primitive_types!(i8; i16; i32; i64; i128; isize; u8; u16; u32; u64; u128; usize; f32; f64);

    pub struct ParamHandler {
        tunables: BTreeMap<String, &'static dyn ErasedTunable>,
    }

    impl ParamHandler {
        pub fn new() -> Self {
            let mut tunables: BTreeMap<String, &'static dyn ErasedTunable> = BTreeMap::new();
            macro_rules! collect_types {
            ($($type: ty);*) => {
                $(
                    for value in inventory::iter::<Value<$type>> {
                        tunables.insert(value.name.to_ascii_lowercase(), value);
                    }
                )*
            };
        }
            collect_types!(i8; i16; i32; i64; i128; isize; u8; u16; u32; u64; u128; usize; f32; f64);
            Self { tunables }
        }

        pub fn tunables(&self) -> impl Iterator<Item = &'static dyn ErasedTunable> {
            self.tunables.values().copied()
        }

        pub fn get(&self, option: &str) -> Option<&'static dyn ErasedTunable> {
            self.tunables.get(option).copied()
        }
    }

    pub trait ErasedTunable {
        fn name(&self) -> &'static str;

        /// SAFETY: Caller must ensure that no other threads are calling `ErasedTunable::set`
        ///
        /// In practice, this means that this function should only be called in TEI handling
        unsafe fn get(&self) -> String;

        /// SAFETY: Caller must ensure that the no other threads hold a reference to the named value
        ///
        /// In practice, this means that this function should only be called in TEI handling
        unsafe fn set(&self, value: &str) -> Result<(), Box<dyn Error>>;

        fn range(&self) -> (String, String);

        fn ob_spsa_type(&self) -> &'static str;

        fn ob_spsa_c_end(&self) -> f64;
    }

    impl<T: Sync + PartialOrd + Display + TunableType> ErasedTunable for Value<T>
    where
        T: ToString + FromStr,
        <T as FromStr>::Err: Error,
    {
        fn name(&self) -> &'static str {
            self.name
        }

        unsafe fn get(&self) -> String {
            unsafe { &*self.value.0.0.get() }.to_string()
        }

        unsafe fn set(&self, string: &str) -> Result<(), Box<dyn Error>> {
            let value = T::from_str(string)?;
            if value < self.min {
                eprintln!(
                    "Warning: {string} is below the set minimum {} for tunable {}",
                    self.min, self.name
                )
            }
            if value > self.max {
                eprintln!(
                    "Warning: {string} is above the set maximum {} for tunable {}",
                    self.max, self.name
                )
            }
            unsafe { self.value.set(value) };
            Ok(())
        }

        fn range(&self) -> (String, String) {
            (self.min.to_string(), self.max.to_string())
        }

        fn ob_spsa_type(&self) -> &'static str {
            T::OB_SPSA_TYPE
        }

        fn ob_spsa_c_end(&self) -> f64 {
            T::ob_spsa_c_end(&self.min, &self.max)
        }
    }
}

#[cfg(feature = "tune")]
pub use inner::*;

/// Convenience trait to allow a uniform API between primitive values and `TunableValue`s
pub trait Val {
    type Type;
    fn val(&self) -> Self::Type;
}

macro_rules! impl_val {
    ($($type: ty);*) => {
        $(
            impl Val for $type {
                type Type = $type;
                fn val(&self) -> Self::Type {
                    *self
                }
            }
        )*
    }
}

impl_val!(i8; i16; i32; i64; i128; isize; u8; u16; u32; u64; u128; usize; f32; f64);
