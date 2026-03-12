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

#[cfg(target_feature = "avx2")]
mod avx2;

#[cfg(all(not(target_feature = "avx2"), target_feature = "sse4.2"))]
mod sse;

use crate::bitboard::Bitboard;

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
        return unsafe { avx2::has_road(road_occ, up, down, left, right) };
    }

    #[cfg(all(not(target_feature = "avx2"), target_feature = "sse4.2"))]
    {
        return unsafe { sse::has_road(road_occ, up, down, left, right) };
    }

    todo!();
}
