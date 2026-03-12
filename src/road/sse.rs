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
pub(super) fn has_road(road_occ: u64, up: u64, down: u64, left: u64, right: u64) -> bool {
    let mut masks_lo = _mm_set_epi64x(up as i64, left as i64);
    let mut masks_hi = _mm_set_epi64x(down as i64, right as i64);

    let left_edge = _mm_set1_epi64x(Bitboard::LEFT_EDGE.raw() as i64);
    let right_edge = _mm_set1_epi64x(Bitboard::RIGHT_EDGE.raw() as i64);

    let road_occ = _mm_set1_epi64x(road_occ as i64);

    let calc_next_masks = |masks| {
        let next_masks_u = _mm_slli_epi64::<6>(masks);
        let next_masks_d = _mm_srli_epi64::<6>(masks);
        let next_masks_ud = _mm_or_si128(next_masks_u, next_masks_d);

        let next_masks_l = _mm_andnot_si128(left_edge, _mm_slli_epi64::<1>(masks));
        let next_masks_r = _mm_andnot_si128(right_edge, _mm_srli_epi64::<1>(masks));
        let next_masks_lr = _mm_or_si128(next_masks_l, next_masks_r);

        let next_masks = _mm_or_si128(next_masks_ud, next_masks_lr);

        _mm_and_si128(next_masks, road_occ)
    };

    /*
    let next_masks_lo = calc_next_masks(masks_lo);
    let next_masks_hi = calc_next_masks(masks_hi);

    let new_lo = _mm_andnot_si128(masks_lo, next_masks_lo);
    let new_lo = _mm_cmpeq_epi64(new_lo, _mm_setzero_si128());

    let new_hi = _mm_andnot_si128(masks_hi, next_masks_hi);
    let new_hi = _mm_cmpeq_epi64(new_hi, _mm_setzero_si128());

    if _mm_testz_si128(new_lo, new_hi) != 0 {
        return false;
    }

    masks_lo = next_masks_lo;
    masks_hi = next_masks_hi;
     */

    loop {
        let next_masks_lo = calc_next_masks(masks_lo);
        let next_masks_hi = calc_next_masks(masks_hi);

        if _mm_testz_si128(next_masks_lo, next_masks_hi) == 0 {
            return true;
        }

        let new_lo = _mm_cmpgt_epi64(next_masks_lo, masks_lo);
        let new_hi = _mm_cmpgt_epi64(next_masks_hi, masks_hi);

        if _mm_testz_si128(new_lo, new_hi) != 0 {
            return false;
        }

        masks_lo = next_masks_lo;
        masks_hi = next_masks_hi;
    }
}
