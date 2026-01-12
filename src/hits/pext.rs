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
use crate::hits::common::{generate_mask, pdep};
use crate::hits::naive::find_hits_naive;
use std::arch::x86_64::_pext_u64;

#[derive(Copy, Clone, Debug)]
struct SquareData {
    mask: u64,
    offset: usize,
}

impl SquareData {
    const fn new() -> Self {
        Self { mask: 0, offset: 0 }
    }
}

struct Data {
    squares: [SquareData; Square::COUNT],
    table_size: usize,
}

const SQUARE_DATA: Data = {
    let mut squares = [SquareData::new(); Square::COUNT];
    let mut table_size = 0;

    let mut idx = 0;
    while let Some(sq) = Square::from_raw(idx) {
        let square_data = &mut squares[sq.idx()];

        square_data.mask = generate_mask(sq);

        square_data.offset = table_size;
        table_size += 1 << square_data.mask.count_ones();

        idx += 1;
    }

    Data {
        squares,
        table_size,
    }
};

#[static_init::dynamic]
static HITS: [super::Hits; SQUARE_DATA.table_size] = {
    let mut result = [[(0, Square::A1); Direction::COUNT]; SQUARE_DATA.table_size];

    for sq in Square::all() {
        let sq_data = &SQUARE_DATA.squares[sq.idx()];
        let entries = 1 << sq_data.mask.count_ones();
        for i in 0..entries {
            let blockers = Bitboard::from_raw(pdep(i as u64, sq_data.mask));
            result[sq_data.offset + i] = find_hits_naive(blockers, sq);
        }
    }

    result
};

#[must_use]
pub fn find_hit_for_dir_pext(blockers: Bitboard, start: Square, dir: Direction) -> super::Hit {
    find_hits_pext(blockers, start)[dir.idx()]
}

#[must_use]
pub(super) fn find_hits_pext(blockers: Bitboard, start: Square) -> super::Hits {
    let sq_data = &SQUARE_DATA.squares[start.idx()];
    let idx = unsafe { _pext_u64(blockers.raw(), sq_data.mask) } as usize;
    HITS[sq_data.offset + idx]
}
