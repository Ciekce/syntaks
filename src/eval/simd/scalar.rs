/*
 * syntaks, a TEI Tak engine
 * Copyright (c) 2026 Ciekce
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

#![allow(unused)]

pub type VecU8 = [u8; 16];
pub type VecI8 = [i8; 16];
pub type VecI16 = [i16; 8];
pub type VecI32 = [i32; 4];

pub const CHUNK_SIZE_I8: usize = size_of::<VecI8>() / size_of::<i8>();
pub const CHUNK_SIZE_I16: usize = size_of::<VecI16>() / size_of::<i16>();
pub const CHUNK_SIZE_I32: usize = size_of::<VecI32>() / size_of::<i32>();

#[must_use]
#[inline(always)]
pub unsafe fn set1_u8x4(ptr: *const u8) -> VecU8 {
    let bytes = unsafe { ptr.cast::<[u8; 4]>().read() };
    let mut out = [0; 16];
    for (out, &byte) in out.iter_mut().zip(bytes.iter().cycle()) {
        *out = byte;
    }
    out
}

#[must_use]
#[inline(always)]
pub unsafe fn set1_i16x2(ptr: *const i16) -> VecI16 {
    let pair = unsafe { ptr.cast::<[i16; 2]>().read() };
    let mut out = [0; 8];
    for (out, &v) in out.iter_mut().zip(pair.iter().cycle()) {
        *out = v;
    }
    out
}

#[inline(always)]
pub unsafe fn store_u8(ptr: *mut u8, vec: VecU8) {
    unsafe { ptr.cast::<VecU8>().write(vec) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i8(ptr: *const i8) -> VecI8 {
    unsafe { ptr.cast::<VecI8>().read() }
}

#[must_use]
#[inline(always)]
pub fn zero_i16() -> VecI16 {
    set1_i16(0)
}

#[must_use]
#[inline(always)]
pub fn set1_i16(v: i16) -> VecI16 {
    [v; 8]
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i16(ptr: *const i16) -> VecI16 {
    unsafe { ptr.cast::<VecI16>().read() }
}

#[inline(always)]
pub unsafe fn store_i16(ptr: *mut i16, vec: VecI16) {
    unsafe { ptr.cast::<VecI16>().write(vec) }
}

#[must_use]
#[inline(always)]
pub fn min_i16(mut a: VecI16, b: VecI16) -> VecI16 {
    for (a, b) in a.iter_mut().zip(b) {
        *a = (*a).min(b);
    }
    a
}

#[must_use]
#[inline(always)]
pub fn min_u16(mut a: VecI16, b: VecI16) -> VecI16 {
    for (a, b) in a.iter_mut().zip(b) {
        *a = (*a as u16).min(b as u16) as i16;
    }
    a
}

#[must_use]
#[inline(always)]
pub fn max_i16(mut a: VecI16, b: VecI16) -> VecI16 {
    for (a, b) in a.iter_mut().zip(b) {
        *a = (*a).max(b);
    }
    a
}

#[must_use]
#[inline(always)]
pub fn mulhi_i16(mut a: VecI16, b: VecI16) -> VecI16 {
    for (a, b) in a.iter_mut().zip(b) {
        *a = ((i32::from(*a) * i32::from(b)) >> 16) as i16;
    }
    a
}

#[must_use]
#[inline(always)]
pub fn mulhi_shift_i16<const SHIFT: i32>(mut a: VecI16, b: VecI16) -> VecI16 {
    for (a, b) in a.iter_mut().zip(b) {
        *a = ((i32::from(*a) * i32::from(b) + (1 << (15 - SHIFT))) >> (16 - SHIFT)) as i16;
    }
    a
}

#[must_use]
#[inline(always)]
pub fn packus_i16(a: VecI16, b: VecI16) -> VecU8 {
    let mut out = [0; 16];
    for (out, v) in out.iter_mut().zip(a.into_iter().chain(b)) {
        *out = v.clamp(0, u8::MAX as i16) as u8;
    }
    out
}

#[must_use]
#[inline(always)]
pub fn packus_i32(a: VecI32, b: VecI32) -> VecI16 {
    let mut out = [0; 8];
    for (out, v) in out.iter_mut().zip(a.into_iter().chain(b)) {
        *out = v.clamp(0, u16::MAX as i32) as u16 as i16;
    }
    out
}

#[must_use]
#[inline(always)]
pub fn zero_i32() -> VecI32 {
    set1_i32(0)
}

#[must_use]
#[inline(always)]
pub fn set1_i32(v: i32) -> VecI32 {
    [v; 4]
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i32(ptr: *const i32) -> VecI32 {
    unsafe { ptr.cast::<VecI32>().read() }
}

#[inline(always)]
pub unsafe fn store_i32(ptr: *mut i32, vec: VecI32) {
    unsafe { ptr.cast::<VecI32>().write(vec) }
}

#[must_use]
#[inline(always)]
pub fn to_array_i32(v: VecI32) -> [i32; CHUNK_SIZE_I32] {
    v
}

#[must_use]
#[inline(always)]
pub fn from_array_i32(a: [i32; CHUNK_SIZE_I32]) -> VecI32 {
    a
}

#[must_use]
#[inline(always)]
pub fn add_i32(mut a: VecI32, b: VecI32) -> VecI32 {
    for (a, b) in a.iter_mut().zip(b) {
        *a = a.wrapping_add(b);
    }
    a
}

#[must_use]
#[inline(always)]
pub fn mul_i32(mut a: VecI32, b: VecI32) -> VecI32 {
    for (a, b) in a.iter_mut().zip(b) {
        *a = a.wrapping_mul(b);
    }
    a
}

#[must_use]
#[inline(always)]
pub fn min_i32(mut a: VecI32, b: VecI32) -> VecI32 {
    for (a, b) in a.iter_mut().zip(b) {
        *a = (*a).min(b);
    }
    a
}

#[must_use]
#[inline(always)]
pub fn max_i32(mut a: VecI32, b: VecI32) -> VecI32 {
    for (a, b) in a.iter_mut().zip(b) {
        *a = (*a).max(b);
    }
    a
}

#[must_use]
#[inline(always)]
pub fn shift_right_i32<const SHIFT: i32>(mut a: VecI32) -> VecI32 {
    for a in a.iter_mut() {
        *a >>= SHIFT;
    }
    a
}

#[must_use]
#[inline(always)]
pub fn dpbusd_i32(mut sum: VecI32, a: VecU8, b: VecI8) -> VecI32 {
    for ((sum, a), b) in sum.iter_mut().zip(a.as_chunks::<4>().0).zip(b.as_chunks::<4>().0) {
        for (&a, &b) in a.iter().zip(b) {
            *sum = sum.wrapping_add(i32::from(a) * i32::from(b));
        }
    }
    sum
}

#[must_use]
#[inline(always)]
pub fn dpbusdx2_i32(sum: VecI32, a_1: VecU8, b_1: VecI8, a_2: VecU8, b_2: VecI8) -> VecI32 {
    dpbusd_i32(dpbusd_i32(sum, a_1, b_1), a_2, b_2)
}

#[must_use]
#[inline(always)]
pub fn dpwssdx2_i32(mut acc: [VecI32; 2], a_1: VecI16, b_1: VecI16, a_2: VecI16, b_2: VecI16) -> [VecI32; 2] {
    for (acc, (a, b)) in acc.iter_mut().zip([(a_1, b_1), (a_2, b_2)]) {
        for ((sum, a), b) in acc.iter_mut().zip(a.as_chunks::<2>().0).zip(b.as_chunks::<2>().0) {
            for (&a, &b) in a.iter().zip(b) {
                *sum = sum.wrapping_add(i32::from(a) * i32::from(b));
            }
        }
    }
    acc
}

#[must_use]
#[inline(always)]
pub fn reduce_dpwssd_i32(acc: [VecI32; 2]) -> VecI32 {
    add_i32(acc[0], acc[1])
}

pub fn hsum_i32(v: VecI32) -> i32 {
    let mut sum = 0i32;
    for v in v {
        sum = sum.wrapping_add(v);
    }
    sum
}
