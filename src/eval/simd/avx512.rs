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

use std::arch::x86_64::*;

pub type VecU8 = __m512i;
pub type VecI8 = __m512i;
pub type VecI16 = __m512i;
pub type VecI32 = __m512i;

pub const CHUNK_SIZE_8: usize = size_of::<VecI16>() / size_of::<i8>();
pub const CHUNK_SIZE_16: usize = size_of::<VecI16>() / size_of::<i16>();
pub const CHUNK_SIZE_32: usize = size_of::<VecI32>() / size_of::<i32>();

// ================================ u8 ================================

#[must_use]
#[inline(always)]
pub fn zero_u8() -> VecU8 {
    unsafe { _mm512_setzero_si512() }
}

#[must_use]
#[inline(always)]
pub fn set1_u8(v: u8) -> VecU8 {
    unsafe { _mm512_set1_epi8(v as i8) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_u8(ptr: *const u8) -> VecU8 {
    unsafe { _mm512_load_si512(ptr.cast()) }
}

#[inline(always)]
pub unsafe fn store_u8(ptr: *mut u8, vec: VecU8) {
    unsafe { _mm512_store_si512(ptr.cast(), vec) }
}

// ================================ i8 ================================

#[must_use]
#[inline(always)]
pub fn zero_i8() -> VecI8 {
    unsafe { _mm512_setzero_si512() }
}

#[must_use]
#[inline(always)]
pub fn set1_i8(v: i8) -> VecI8 {
    unsafe { _mm512_set1_epi8(v) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i8(ptr: *const i8) -> VecI8 {
    unsafe { _mm512_load_si512(ptr.cast()) }
}

#[inline(always)]
pub unsafe fn store_i8(ptr: *mut i8, vec: VecI8) {
    unsafe { _mm512_store_si512(ptr.cast(), vec) }
}

// ================================ i16 ================================

#[must_use]
#[inline(always)]
pub fn zero_i16() -> VecI16 {
    unsafe { _mm512_setzero_si512() }
}

#[must_use]
#[inline(always)]
pub fn set1_i16(v: i16) -> VecI16 {
    unsafe { _mm512_set1_epi16(v) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i16(ptr: *const i16) -> VecI16 {
    unsafe { _mm512_load_si512(ptr.cast()) }
}

#[inline(always)]
pub unsafe fn store_i16(ptr: *mut i16, vec: VecI16) {
    unsafe { _mm512_store_si512(ptr.cast(), vec) }
}

#[must_use]
#[inline(always)]
pub fn add_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm512_add_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn sub_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm512_sub_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn mul_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm512_mullo_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn min_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm512_min_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn max_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm512_max_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn shli_i16<const SHIFT: u32>(v: VecI16) -> VecI16 {
    unsafe { _mm512_slli_epi16(v, SHIFT) }
}

#[must_use]
#[inline(always)]
pub fn shri_i16<const SHIFT: u32>(v: VecI16) -> VecI16 {
    unsafe { _mm512_srai_epi16(v, SHIFT) }
}

#[must_use]
#[inline(always)]
pub fn shli_mulhi_i16<const SHIFT: u32>(a: VecI16, b: VecI16) -> VecI16 {
    unsafe {
        let shifted = _mm512_slli_epi16(a, SHIFT);
        _mm512_mulhi_epi16(shifted, b)
    }
}

#[must_use]
#[inline(always)]
pub fn madd_i16(a: VecI16, b: VecI16) -> VecI32 {
    unsafe { _mm512_madd_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn packus_i16(a: VecI16, b: VecI16) -> VecU8 {
    unsafe {
        let x = _mm512_shuffle_i64x2(a, b, 136);
        let y = _mm512_shuffle_i64x2(a, b, 221);
        _mm512_packus_epi16(x, y)
    }
}

// ================================ i32 ================================

#[must_use]
#[inline(always)]
pub fn zero_i32() -> VecI32 {
    unsafe { _mm512_setzero_si512() }
}

#[must_use]
#[inline(always)]
pub fn set1_i32(v: i32) -> VecI32 {
    unsafe { _mm512_set1_epi32(v) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i32(ptr: *const i32) -> VecI32 {
    unsafe { _mm512_load_si512(ptr.cast()) }
}

#[inline(always)]
pub unsafe fn store_i32(ptr: *mut i32, vec: VecI32) {
    unsafe { _mm512_store_si512(ptr.cast(), vec) }
}

#[must_use]
#[inline(always)]
pub fn add_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { _mm512_add_epi32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn sub_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { _mm512_sub_epi32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn mul_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { _mm512_mullo_epi32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn min_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { _mm512_min_epi32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn max_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { _mm512_max_epi32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn clamp_i32(v: VecI32, min: VecI32, max: VecI32) -> VecI32 {
    min_i32(max_i32(v, min), max)
}

#[must_use]
#[inline(always)]
pub fn shli_i32<const SHIFT: u32>(v: VecI32) -> VecI32 {
    unsafe { _mm512_slli_epi32(v, SHIFT) }
}

#[must_use]
#[inline(always)]
pub fn shri_i32<const SHIFT: u32>(v: VecI32) -> VecI32 {
    unsafe { _mm512_srai_epi32(v, SHIFT) }
}

#[inline(always)]
pub fn dpbusd_i32(sum: VecI32, u: VecU8, i: VecI8) -> VecI32 {
    unsafe { _mm512_dpbusd_epi32(sum, u, i) }
}

pub fn hsum_i32(v: VecI32) -> i32 {
    unsafe { _mm512_reduce_add_epi32(v) }
}

pub fn cast_u8_i32(v: VecI32) -> VecU8 {
    v
}
