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

use std::arch::aarch64::*;

pub type VecU8 = uint8x16_t;
pub type VecI8 = int8x16_t;
pub type VecI16 = int16x8_t;
pub type VecI32 = int32x4_t;

pub const CHUNK_SIZE_I8: usize = size_of::<VecI8>() / size_of::<i8>();
pub const CHUNK_SIZE_I16: usize = size_of::<VecI16>() / size_of::<i16>();
pub const CHUNK_SIZE_I32: usize = size_of::<VecI32>() / size_of::<i32>();

#[must_use]
#[inline(always)]
pub unsafe fn set1_u8x4(ptr: *const u8) -> VecU8 {
    unsafe { vreinterpretq_u8_s32(set1_i32(ptr.cast::<i32>().read_unaligned())) }
}

#[must_use]
#[inline(always)]
pub unsafe fn set1_i16x2(ptr: *const i16) -> VecI16 {
    unsafe { vreinterpretq_s16_s32(set1_i32(ptr.cast::<i32>().read_unaligned())) }
}

#[inline(always)]
pub unsafe fn store_u8(ptr: *mut u8, vec: VecU8) {
    unsafe { vst1q_u8(ptr, vec) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i8(ptr: *const i8) -> VecI8 {
    unsafe { vld1q_s8(ptr) }
}

#[must_use]
#[inline(always)]
pub fn zero_i16() -> VecI16 {
    set1_i16(0)
}

#[must_use]
#[inline(always)]
pub fn set1_i16(v: i16) -> VecI16 {
    unsafe { vdupq_n_s16(v) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i16(ptr: *const i16) -> VecI16 {
    unsafe { vld1q_s16(ptr) }
}

#[inline(always)]
pub unsafe fn store_i16(ptr: *mut i16, vec: VecI16) {
    unsafe { vst1q_s16(ptr, vec) }
}

#[must_use]
#[inline(always)]
pub fn min_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { vminq_s16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn min_u16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { vreinterpretq_s16_u16(vminq_u16(vreinterpretq_u16_s16(a), vreinterpretq_u16_s16(b))) }
}

#[must_use]
#[inline(always)]
pub fn max_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { vmaxq_s16(a, b) }
}

#[must_use]
#[inline(always)]
pub fn mulhi_i16(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { vshrq_n_s16::<1>(vqdmulhq_s16(a, b)) }
}

#[must_use]
#[inline(always)]
pub fn mulhi_shift_i16<const SHIFT: i32>(a: VecI16, b: VecI16) -> VecI16 {
    unsafe { vqrdmulhq_s16(vshlq_s16(a, set1_i16((SHIFT - 1) as i16)), b) }
}

#[must_use]
#[inline(always)]
pub fn packus_i16(a: VecI16, b: VecI16) -> VecU8 {
    unsafe { vcombine_u8(vqmovun_s16(a), vqmovun_s16(b)) }
}

#[must_use]
#[inline(always)]
pub fn packus_i32(a: VecI32, b: VecI32) -> VecI16 {
    unsafe { vreinterpretq_s16_u16(vcombine_u16(vqmovun_s32(a), vqmovun_s32(b))) }
}

#[must_use]
#[inline(always)]
pub fn zero_i32() -> VecI32 {
    set1_i32(0)
}

#[must_use]
#[inline(always)]
pub fn set1_i32(v: i32) -> VecI32 {
    unsafe { vdupq_n_s32(v) }
}

#[must_use]
#[inline(always)]
pub unsafe fn load_i32(ptr: *const i32) -> VecI32 {
    unsafe { vld1q_s32(ptr) }
}

#[inline(always)]
pub unsafe fn store_i32(ptr: *mut i32, vec: VecI32) {
    unsafe { vst1q_s32(ptr, vec) }
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
    unsafe { vaddq_s32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn mul_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { vmulq_s32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn min_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { vminq_s32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn max_i32(a: VecI32, b: VecI32) -> VecI32 {
    unsafe { vmaxq_s32(a, b) }
}

#[must_use]
#[inline(always)]
pub fn shift_right_i32<const SHIFT: i32>(a: VecI32) -> VecI32 {
    unsafe { vshrq_n_s32::<SHIFT>(a) }
}

#[must_use]
#[inline(always)]
pub fn dpbusd_i32(sum: VecI32, a: VecU8, b: VecI8) -> VecI32 {
    cfg_select! {
        target_feature = "dotprod" => unsafe { vdotq_s32(sum, vreinterpretq_s8_u8(a), b) },
        _ => unsafe {
            let a = vreinterpretq_s8_u8(a);
            let lo = vmull_s8(vget_low_s8(a), vget_low_s8(b));
            let hi = vmull_high_s8(a, b);
            vpadalq_s16(sum, vpaddq_s16(lo, hi))
        },
    }
}

#[must_use]
#[inline(always)]
pub fn dpbusdx2_i32(sum: VecI32, a_1: VecU8, b_1: VecI8, a_2: VecU8, b_2: VecI8) -> VecI32 {
    cfg_select! {
        target_feature = "dotprod" => dpbusd_i32(dpbusd_i32(sum, a_1, b_1), a_2, b_2),
        _ => {
            let result = unsafe {
                let a_1 = vreinterpretq_s8_u8(a_1);
                let a_2 = vreinterpretq_s8_u8(a_2);
                let lo = vmlal_s8(
                    vmull_s8(vget_low_s8(a_1), vget_low_s8(b_1)),
                    vget_low_s8(a_2),
                    vget_low_s8(b_2),
                );
                let hi = vmlal_high_s8(vmull_high_s8(a_1, b_1), a_2, b_2);
                vpadalq_s16(sum, vpaddq_s16(lo, hi))
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
    unsafe {
        let lo = vmlal_s16(acc[0], vget_low_s16(a_1), vget_low_s16(b_1));
        let lo = vmlal_s16(lo, vget_low_s16(a_2), vget_low_s16(b_2));
        let hi = vmlal_high_s16(vmlal_high_s16(acc[1], a_1, b_1), a_2, b_2);
        [lo, hi]
    }
}

#[must_use]
#[inline(always)]
pub fn reduce_dpwssd_i32(acc: [VecI32; 2]) -> VecI32 {
    unsafe { vpaddq_s32(acc[0], acc[1]) }
}

pub fn hsum_i32(v: VecI32) -> i32 {
    unsafe { vaddvq_s32(v) }
}
