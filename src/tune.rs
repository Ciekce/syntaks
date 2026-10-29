#[macro_export]
macro_rules! tunable {
    ($name: ident: $type: ty = $value: expr; range($min: expr, $max: expr)) => {
        #[cfg(feature = "tune")]
        static $name: $crate::tune::inner::TunableValue<$type> = {
            static INNER: $crate::tune::inner::MakeSync<std::cell::UnsafeCell<$type>> =
                $crate::tune::inner::MakeSync(std::cell::UnsafeCell::new($value));
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
        collections::HashMap,
        error::Error,
        fmt::Display,
        ops::{Add, AddAssign, Deref, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
        str::FromStr,
    };

    pub struct Value<T: 'static> {
        name: &'static str,
        value: &'static TunableValue<T>,
        min: T,
        max: T,
    }

    impl<T: 'static> Value<T> {
        pub const fn new(name: &'static str, value: &'static TunableValue<T>, min: T, max: T) -> Self {
            Self { name, value, min, max }
        }
    }

    pub struct MakeSync<T>(pub T);

    unsafe impl<T> Sync for MakeSync<T> {}

    #[derive(Copy, Clone)]
    pub struct TunableValue<T: 'static>(&'static MakeSync<UnsafeCell<T>>);

    unsafe impl<T: 'static> Sync for TunableValue<T> {}

    impl<T> TunableValue<T> {
        pub const fn new(value: &'static MakeSync<UnsafeCell<T>>) -> Self {
            Self(value)
        }

        pub unsafe fn set(&self, value: T) {
            unsafe {
                *self.0.0.get() = value;
            }
        }
    }

    impl<T> Deref for TunableValue<T> {
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
                inventory::collect!(Value<$type>);
            )*
        };
    }

    impl_primitive_types!(i8; i16; i32; i64; i128; u8; u16; u32; u64; u128; f32; f64);

    pub struct ParamHandler {
        tunables: HashMap<String, &'static dyn ErasedTunable>,
    }

    impl ParamHandler {
        pub fn new() -> Self {
            let mut tunables: HashMap<String, &'static dyn ErasedTunable> = HashMap::new();
            macro_rules! collect_types {
            ($($type: ty);*) => {
                $(
                    for value in inventory::iter::<Value<$type>> {
                        tunables.insert(value.name.to_ascii_lowercase(), value);
                    }
                )*
            };
        }
            collect_types!(i8; i16; i32; i64; i128; u8; u16; u32; u64; u128; f32; f64);
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

        unsafe fn get(&self) -> String;

        // SAFETY: Caller must ensure that (TODO)
        unsafe fn set(&self, value: &str) -> Result<(), Box<dyn Error>>;

        fn range(&self) -> (String, String);
    }

    impl<T: PartialOrd + Display> ErasedTunable for Value<T>
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
    }
}

#[cfg(feature = "tune")]
pub use inner::*;

#[cfg(feature = "tune")]
#[test]
fn test_tunable_owo() {
    tunable!(X: i32 = 1; range(0, 100));
    let a = X + 1;
    assert_eq!(a, 2);
    unsafe {
        X.set(2);
    }
    let a = X + 1;
    assert_eq!(a, 3);
}
