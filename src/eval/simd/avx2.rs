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

pub type VecU8 = __m256i;
pub type VecI8 = __m256i;
pub type VecI16 = __m256i;
pub type VecI32 = __m256i;

pub const CHUNK_SIZE_I8: usize = size_of::<VecI8>() / size_of::<i8>();
pub const CHUNK_SIZE_I16: usize = size_of::<VecI16>() / size_of::<i16>();
pub const CHUNK_SIZE_I32: usize = size_of::<VecI32>() / size_of::<i32>();

#[must_use]
#[inline(always)]
pub unsafe fn set1_u8x4(ptr: *const u8) -> VecU8 {
    set1_i32(unsafe { ptr.cast::<i32>().read_unaligned() })
}

#[must_use]
#[inline(always)]
pub unsafe fn set1_i16x2(ptr: *const i16) -> VecI16 {
    set1_i32(unsafe { ptr.cast::<i32>().read_unaligned() })
}

#[inline(always)]
pub unsafe fn store_u8(ptr: *mut u8, vec: VecU8) {
    unsafe { _mm256_store_si256(ptr.cast(), vec) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i8(ptr: *const i8) -> VecI8 {
    unsafe { _mm256_load_si256(ptr.cast()) }
}

// ================================ i16 ================================

#[must_use]
#[inline(always)]
pub fn zero_i16() -> VecI16 {
    set1_i16(0)
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
pub fn min_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm256_min_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn min_u16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm256_min_epu16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn max_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm256_max_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn mulhi_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm256_mulhi_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn mulhi_shift_i16<const SHIFT: i32>(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { _mm256_mulhrs_epi16(_mm256_sll_epi16(a, _mm_cvtsi32_si128(SHIFT - 1)), b) }
}

#[must_use]
#[inline(always)]
pub fn packus_i16(a: VecI16, b: VecI16) -> VecU8 {
    unsafe { _mm256_packus_epi16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn packus_i32(a: VecI32, b: VecI32) -> VecI16 {
    unsafe { _mm256_packus_epi32(a, b) }
}

// ================================ i32 ================================

#[must_use]
#[inline(always)]
pub fn zero_i32() -> VecI32 {
    set1_i32(0)
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
    unsafe { _mm256_add_epi32(a, b) }
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

#[must_use]
#[inline(always)]
pub fn shift_right_i32<const SHIFT: i32>(a: VecI32) -> VecI32 {
    unsafe { _mm256_srai_epi32::<SHIFT>(a) }
}

#[must_use]
#[inline(always)]
pub fn dpbusd_i32(sum: VecI32, a: VecU8, b: VecI8) -> VecI32 {
    cfg_select! {
        target_feature = "avxvnni" => unsafe { _mm256_dpbusd_avx_epi32(sum, a, b) },
        _ => unsafe { add_i32(sum, _mm256_madd_epi16(_mm256_maddubs_epi16(a, b), set1_i16(1))) },
    }
}

#[must_use]
#[inline(always)]
pub fn dpbusdx2_i32(sum: VecI32, a_1: VecU8, b_1: VecI8, a_2: VecU8, b_2: VecI8) -> VecI32 {
    cfg_select! {
        target_feature = "avxvnni" => dpbusd_i32(dpbusd_i32(sum, a_1, b_1), a_2, b_2),
        _ => {
            let result = unsafe {
                let products = _mm256_add_epi16(_mm256_maddubs_epi16(a_1, b_1), _mm256_maddubs_epi16(a_2, b_2));
                add_i32(sum, _mm256_madd_epi16(products, set1_i16(1)))
            };

            debug_assert_eq!(
                to_array_i32(result),
                to_array_i32(dpbusd_i32(dpbusd_i32(sum, a_1, b_1), a_2, b_2)),
            );

            result
        }
    }
}

#[must_use]
#[inline(always)]
pub fn dpwssdx2_i32(acc: [VecI32; 2], a_1: VecI16, b_1: VecI16, a_2: VecI16, b_2: VecI16) -> [VecI32; 2] {
    cfg_select! {
        target_feature = "avxvnni" => unsafe {
            [
                _mm256_dpwssd_avx_epi32(acc[0], a_1, b_1),
                _mm256_dpwssd_avx_epi32(acc[1], a_2, b_2),
            ]
        },
        _ => unsafe {
            [
                add_i32(acc[0], _mm256_madd_epi16(a_1, b_1)),
                add_i32(acc[1], _mm256_madd_epi16(a_2, b_2)),
            ]
        },
    }
}

#[must_use]
#[inline(always)]
pub fn reduce_dpwssd_i32(acc: [VecI32; 2]) -> VecI32 {
    add_i32(acc[0], acc[1])
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
