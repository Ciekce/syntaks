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

mod common;
mod naive;

#[cfg(all(feature = "pext", target_feature = "bmi2"))]
mod pext;

#[cfg(not(all(feature = "pext", target_feature = "bmi2")))]
mod magic;

pub type Hit = (u8, Square);
pub type Hits = [Hit; Direction::COUNT];

#[must_use]
pub fn find_hit_for_dir(blockers: Bitboard, start: Square, dir: Direction) -> Hit {
    #[cfg(all(feature = "pext", target_feature = "bmi2"))]
    {
        return pext::find_hit_for_dir_pext(blockers, start, dir);
    }

    #[cfg(not(all(feature = "pext", target_feature = "bmi2")))]
    {
        return magic::find_hit_for_dir_magic(blockers, start, dir);
    }

    unreachable!();
}

#[must_use]
pub fn find_hits(blockers: Bitboard, start: Square) -> Hits {
    #[cfg(all(feature = "pext", target_feature = "bmi2"))]
    {
        return pext::find_hits_pext(blockers, start);
    }

    #[cfg(not(all(feature = "pext", target_feature = "bmi2")))]
    {
        return magic::find_hits_magic(blockers, start);
    }

    unreachable!();
}
