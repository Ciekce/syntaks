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

use std::arch::x86_64::*;

pub type VecI16 = __m256i;
pub type VecI32 = __m256i;

pub const CHUNK_SIZE_I16: usize = size_of::<VecI16>() / size_of::<i16>();
pub const CHUNK_SIZE_I32: usize = size_of::<VecI32>() / size_of::<i32>();

// ================================ i16 ================================

#[must_use]
#[inline(always)]
pub fn zero_i16() -> VecI16 {
    unsafe { _mm256_setzero_si256() }
}

#[must_use]
#[inline(always)]
pub fn set1_i16(v: i16) -> VecI16 {
    unsafe { _mm256_set1_epi16(v) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i16(ptr: *const i16) -> VecI16 {
    unsafe { _mm256_load_si256(ptr.cast()) }
}

#[inline(always)]
pub unsafe fn store_i16(ptr: *mut i16, vec: VecI16) {
    unsafe { _mm256_store_si256(ptr.cast(), vec) }
}

#[must_use]
#[inline(always)]
pub fn add_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm256_add_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn sub_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm256_sub_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn mul_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm256_mullo_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn min_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm256_min_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn max_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm256_max_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn madd_i16(a: VecI16, b: VecI16) -> VecI32 {
    unsafe { _mm256_madd_epi16(a, b) }
}

// ================================ i32 ================================

#[must_use]
#[inline(always)]
pub fn zero_i32() -> VecI32 {
    unsafe { _mm256_setzero_si256() }
}

#[must_use]
#[inline(always)]
pub fn set1_i32(v: i32) -> VecI32 {
    unsafe { _mm256_set1_epi32(v) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i32(ptr: *const i32) -> VecI32 {
    unsafe { _mm256_load_si256(ptr.cast()) }
}

#[inline(always)]
pub unsafe fn store_i32(ptr: *mut i32, vec: VecI32) {
    unsafe { _mm256_store_si256(ptr.cast(), vec) }
}

#[must_use]
#[inline(always)]
pub fn add_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { _mm256_add_epi32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn sub_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { _mm256_sub_epi32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn mul_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { _mm256_mullo_epi32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn min_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { _mm256_min_epi32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn max_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { _mm256_max_epi32(a, b) }
}

pub fn hsum_i32(v: VecI32) -> i32 {
    // https://github.com/rust-lang/rust/issues/111147
    const fn mm_shuffle(z: u32, y: u32, x: u32, w: u32) -> i32 {
        ((z << 6) | (y << 4) | (x << 2) | w) as i32
    }

    unsafe {
        let hi128 = _mm256_extracti128_si256(v, 1);
        let lo128 = _mm256_castsi256_si128(v);

        let sum128 = _mm_add_epi32(hi128, lo128);

        let hi64 = _mm_unpackhi_epi64(sum128, sum128);
        let sum64 = _mm_add_epi32(sum128, hi64);

        let hi32 = _mm_shuffle_epi32(sum64, mm_shuffle(2, 3, 0, 1));
        let sum32 = _mm_add_epi32(sum64, hi32);

        _mm_cvtsi128_si32(sum32)
    }
}
