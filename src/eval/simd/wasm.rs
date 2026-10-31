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

use std::arch::wasm32::*;

pub type VecU8 = v128;
pub type VecI8 = v128;
pub type VecI16 = v128;
pub type VecI32 = v128;

pub const CHUNK_SIZE_I8: usize = size_of::<VecI8>() / size_of::<i8>();
pub const CHUNK_SIZE_I16: usize = size_of::<VecI16>() / size_of::<i16>();
pub const CHUNK_SIZE_I32: usize = size_of::<VecI32>() / size_of::<i32>();

#[must_use]
#[inline(always)]
pub unsafe fn set1_u8x4(ptr: *const u8) -> VecU8 {
    unsafe { v128_load32_splat(ptr.cast()) }
}

#[must_use]
#[inline(always)]
pub unsafe fn set1_i16x2(ptr: *const i16) -> VecI16 {
    unsafe { v128_load32_splat(ptr.cast()) }
}

#[inline(always)]
pub unsafe fn store_u8(ptr: *mut u8, vec: VecU8) {
    unsafe { v128_store(ptr.cast(), vec) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i8(ptr: *const i8) -> VecI8 {
    unsafe { v128_load(ptr.cast()) }
}

#[must_use]
#[inline(always)]
pub fn zero_i16() -> VecI16 {
    set1_i16(0)
}

#[must_use]
#[inline(always)]
pub fn set1_i16(v: i16) -> VecI16 {
    i16x8_splat(v)
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i16(ptr: *const i16) -> VecI16 {
    unsafe { v128_load(ptr.cast()) }
}

#[inline(always)]
pub unsafe fn store_i16(ptr: *mut i16, vec: VecI16) {
    unsafe { v128_store(ptr.cast(), vec) }
}

#[must_use]
#[inline(always)]
pub fn min_i16(a: VecI16, b: VecI16) -> VecI16 {
    i16x8_min(a, b)
}

#[must_use]
#[inline(always)]
pub fn min_u16(a: VecI16, b: VecI16) -> VecI16 {
    u16x8_min(a, b)
}

#[must_use]
#[inline(always)]
pub fn max_i16(a: VecI16, b: VecI16) -> VecI16 {
    i16x8_max(a, b)
}

#[must_use]
#[inline(always)]
pub fn mulhi_i16(a: VecI16, b: VecI16) -> VecI16 {
    i16x8_narrow_i32x4(
        i32x4_shr(i32x4_extmul_low_i16x8(a, b), 16),
        i32x4_shr(i32x4_extmul_high_i16x8(a, b), 16),
    )
}

#[must_use]
#[inline(always)]
pub fn mulhi_shift_i16<const SHIFT: i32>(a: VecI16, b: VecI16) -> VecI16 {
    i16x8_q15mulr_sat(i16x8_shl(a, (SHIFT - 1) as u32), b)
}

#[must_use]
#[inline(always)]
pub fn packus_i16(a: VecI16, b: VecI16) -> VecU8 {
    u8x16_narrow_i16x8(a, b)
}

#[must_use]
#[inline(always)]
pub fn packus_i32(a: VecI32, b: VecI32) -> VecI16 {
    u16x8_narrow_i32x4(a, b)
}

#[must_use]
#[inline(always)]
pub fn zero_i32() -> VecI32 {
    set1_i32(0)
}

#[must_use]
#[inline(always)]
pub fn set1_i32(v: i32) -> VecI32 {
    i32x4_splat(v)
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i32(ptr: *const i32) -> VecI32 {
    unsafe { v128_load(ptr.cast()) }
}

#[inline(always)]
pub unsafe fn store_i32(ptr: *mut i32, vec: VecI32) {
    unsafe { v128_store(ptr.cast(), vec) }
}

#[must_use]
#[inline(always)]
pub fn to_array_i32(v: VecI32) -> [i32; CHUNK_SIZE_I32] {
    unsafe { std::mem::transmute(v) }
}

#[must_use]
#[inline(always)]
pub fn from_array_i32(a: [i32; CHUNK_SIZE_I32]) -> VecI32 {
    unsafe { std::mem::transmute(a) }
}

#[must_use]
#[inline(always)]
pub fn add_i32(a: VecI32, b: VecI32) -> VecI32 {
    i32x4_add(a, b)
}

#[must_use]
#[inline(always)]
pub fn mul_i32(a: VecI32, b: VecI32) -> VecI32 {
    i32x4_mul(a, b)
}

#[must_use]
#[inline(always)]
pub fn min_i32(a: VecI32, b: VecI32) -> VecI32 {
    i32x4_min(a, b)
}

#[must_use]
#[inline(always)]
pub fn max_i32(a: VecI32, b: VecI32) -> VecI32 {
    i32x4_max(a, b)
}

#[must_use]
#[inline(always)]
pub fn shift_right_i32<const SHIFT: i32>(a: VecI32) -> VecI32 {
    i32x4_shr(a, SHIFT as u32)
}

#[must_use]
#[inline(always)]
pub fn dpbusd_i32(sum: VecI32, a: VecU8, b: VecI8) -> VecI32 {
    cfg_select! {
        target_feature = "relaxed-simd" => i32x4_relaxed_dot_i8x16_i7x16_add(b, a, sum),
        _ => {
            let low = i32x4_dot_i16x8(i16x8_extend_low_u8x16(a), i16x8_extend_low_i8x16(b));
            let high = i32x4_dot_i16x8(i16x8_extend_high_u8x16(a), i16x8_extend_high_i8x16(b));
            let even = i32x4_shuffle::<0, 2, 4, 6>(low, high);
            let odd = i32x4_shuffle::<1, 3, 5, 7>(low, high);
            i32x4_add(sum, i32x4_add(even, odd))
        }
    }
}

#[must_use]
#[inline(always)]
pub fn dpbusdx2_i32(sum: VecI32, a_1: VecU8, b_1: VecI8, a_2: VecU8, b_2: VecI8) -> VecI32 {
    dpbusd_i32(dpbusd_i32(sum, a_1, b_1), a_2, b_2)
}

#[must_use]
#[inline(always)]
pub fn dpwssdx2_i32(acc: [VecI32; 2], a_1: VecI16, b_1: VecI16, a_2: VecI16, b_2: VecI16) -> [VecI32; 2] {
    [
        i32x4_add(acc[0], i32x4_dot_i16x8(a_1, b_1)),
        i32x4_add(acc[1], i32x4_dot_i16x8(a_2, b_2)),
    ]
}

#[must_use]
#[inline(always)]
pub fn reduce_dpwssd_i32(acc: [VecI32; 2]) -> VecI32 {
    add_i32(acc[0], acc[1])
}

#[must_use]
#[inline(always)]
pub fn hsum_i32(v: VecI32) -> i32 {
    to_array_i32(v).into_iter().fold(0i32, i32::wrapping_add)
}
