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
use std::arch::x86_64::_pdep_u64;

pub(super) const fn generate_mask(sq: Square) -> u64 {
    let mut mask = Bitboard::empty();

    let mut dir_idx = 0;
    while let Some(dir) = Direction::from_raw(dir_idx) {
        let mut dir_bb = Bitboard::empty();

        let mut sq = sq;

        while let Some(shifted) = sq.shift_checked(dir) {
            dir_bb.set_sq(shifted);
            sq = shifted;
        }

        mask = mask.or(dir_bb.and(Bitboard::edge(dir).cmpl()));

        dir_idx += 1;
    }

    mask.raw()
}

pub(super) fn pdep(v: u64, mask: u64) -> u64 {
    #[cfg(target_feature = "bmi2")]
    {
        return unsafe { _pdep_u64(v, mask) };
    }

    let mut mask = mask;

    let mut x = 0;
    let mut bit = 1;

    while mask != 0 {
        if (v & bit) != 0 {
            x |= mask & mask.wrapping_neg();
        }

        bit <<= 1;
        mask &= mask - 1;
    }

    x
}
