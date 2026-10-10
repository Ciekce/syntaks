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
pub fn influence(road_occ: Bitboard) -> (bool, [Bitboard; 4]) {
    let upper_edge = Bitboard::UPPER_EDGE;
    let lower_edge = Bitboard::LOWER_EDGE;
    let left_edge = Bitboard::LEFT_EDGE;
    let right_edge = Bitboard::RIGHT_EDGE;

    let mut edges = [
        road_occ & upper_edge,
        road_occ & lower_edge,
        road_occ & left_edge,
        road_occ & right_edge,
    ];

    edges[0] |= edges[0] >> 6 & road_occ;
    edges[1] |= edges[1] << 6 & road_occ;
    edges[2] |= edges[2] << 1 & road_occ;
    edges[3] |= edges[3] >> 1 & road_occ;

    let result;

    #[cfg(target_feature = "avx2")]
    {
        //SAFETY: self-explanatory
        result = unsafe { avx2::influence(road_occ, &mut edges) };
    }

    #[cfg(all(not(target_feature = "avx2"), target_feature = "sse4.2"))]
    {
        result = unsafe { sse::influence(road_occ, &mut edges) };
    }

    (result, edges)
}
