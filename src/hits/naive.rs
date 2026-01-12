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
use crate::core::{Direction, Square};

#[must_use]
fn find_hit_for_dir_naive(blockers: Bitboard, start: Square, dir: Direction) -> (u8, Square) {
    let mut sq = start;
    let mut dist = 0;

    while let Some(next) = sq.shift_checked(dir) {
        sq = next;
        dist += 1;

        if blockers.has_sq(sq) {
            break;
        }
    }

    (dist, sq)
}

#[must_use]
pub(super) fn find_hits_naive(blockers: Bitboard, start: Square) -> super::Hits {
    std::array::from_fn(|idx| {
        let dir = Direction::from_raw(idx as u8).unwrap();
        find_hit_for_dir_naive(blockers, start, dir)
    })
}
