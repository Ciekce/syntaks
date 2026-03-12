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
#[target_feature(enable = "sse4.2")]
pub(super) fn has_road(road_occ: u64, up: u64, down: u64, left: u64, right: u64) -> bool {
    let mut masks_ul = _mm_set_epi64x(up as i64, left as i64);
    let mut masks_dr = _mm_set_epi64x(down as i64, right as i64);

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

    masks_ul = calc_next_masks(masks_ul);
    masks_dr = calc_next_masks(masks_dr);

    loop {
        let next_masks_ul = calc_next_masks(masks_ul);
        let next_masks_dr = calc_next_masks(masks_dr);

        if _mm_testz_si128(next_masks_ul, next_masks_dr) == 0 {
            return true;
        }

        let new_ul = _mm_cmpgt_epi64(next_masks_ul, masks_ul);
        let new_dr = _mm_cmpgt_epi64(next_masks_dr, masks_dr);

        if _mm_testz_si128(new_ul, new_dr) != 0 {
            return false;
        }

        masks_ul = next_masks_ul;
        masks_dr = next_masks_dr;
    }
}
