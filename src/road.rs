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

use crate::bitboard::Bitboard;
use std::arch::x86_64::*;

#[must_use]
#[target_feature(enable = "avx2")]
fn has_road_avx2(road_occ: u64, up: u64, down: u64, left: u64, right: u64) -> bool {
    // https://github.com/rust-lang/rust/issues/111147
    const fn mm_shuffle(z: u32, y: u32, x: u32, w: u32) -> i32 {
        ((z << 6) | (y << 4) | (x << 2) | w) as i32
    }

    let mut masks = _mm256_set_epi64x(up as i64, down as i64, left as i64, right as i64);

    let left_edge = _mm256_set1_epi64x(Bitboard::LEFT_EDGE.raw() as i64);
    let right_edge = _mm256_set1_epi64x(Bitboard::RIGHT_EDGE.raw() as i64);

    let road_occ = _mm256_set1_epi64x(road_occ as i64);

    let calc_next_masks = |masks| {
        let next_masks_u = _mm256_slli_epi64::<6>(masks);
        let next_masks_d = _mm256_srli_epi64::<6>(masks);
        let next_masks_ud = _mm256_or_si256(next_masks_u, next_masks_d);

        let next_masks_l = _mm256_andnot_si256(left_edge, _mm256_slli_epi64::<1>(masks));
        let next_masks_r = _mm256_andnot_si256(right_edge, _mm256_srli_epi64::<1>(masks));
        let next_masks_lr = _mm256_or_si256(next_masks_l, next_masks_r);

        let next_masks = _mm256_or_si256(next_masks_ud, next_masks_lr);

        _mm256_and_si256(next_masks, road_occ)
    };

    let next_masks = calc_next_masks(masks);

    let new = _mm256_andnot_si256(masks, next_masks);
    let new = _mm256_cmpeq_epi64(new, _mm256_setzero_si256());
    let new = unsafe { std::mem::transmute::<__m256i, __m256d>(new) };
    let bit = _mm256_movemask_pd(new) ^ 0xF;

    if (1 << bit) & 0b1111_1000_1000_1000 == 0 {
        return false;
    }

    masks = next_masks;

    loop {
        let next_masks = calc_next_masks(masks);
        let swizzled = _mm256_shuffle_epi32::<{ mm_shuffle(1, 0, 3, 2) }>(next_masks);

        if _mm256_testz_si256(next_masks, swizzled) == 0 {
            return true;
        }

        let new = _mm256_cmpgt_epi64(next_masks, masks);
        let new = unsafe { std::mem::transmute::<__m256i, __m256d>(new) };
        let bit = _mm256_movemask_pd(new);

        if (1 << bit) & 0b1111_1000_1000_1000 == 0 {
            return false;
        }

        masks = next_masks;
    }
}

#[must_use]
pub fn has_road(road_occ: Bitboard) -> bool {
    let upper_edge = Bitboard::UPPER_EDGE.raw();
    let lower_edge = Bitboard::LOWER_EDGE.raw();
    let left_edge = Bitboard::LEFT_EDGE.raw();
    let right_edge = Bitboard::RIGHT_EDGE.raw();

    let road_occ = road_occ.raw();

    let up = road_occ & upper_edge;
    let down = road_occ & lower_edge;
    let left = road_occ & left_edge;
    let right = road_occ & right_edge;

    let up = up | (up >> 6 & road_occ);
    let down = down | (down << 6 & road_occ);
    let left = left | (left << 1 & road_occ);
    let right = right | (right >> 1 & road_occ);

    #[cfg(target_feature = "avx2")]
    {
        //SAFETY: self-explanatory
        return unsafe { has_road_avx2(road_occ, up, down, left, right) };
    }

    todo!();
}
