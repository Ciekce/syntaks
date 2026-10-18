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

use crate::core::*;
use crate::position::Position;
use crate::prng::Sfc64;

const BLACK_COUNT: usize = 1;
const TOP_COUNT: usize = PieceType::COUNT * Square::COUNT;
const COLOR_COUNT: usize = Position::MAX_STACK_HEIGHT * Color::COUNT * Square::COUNT;

const TOTAL_COUNT: usize = BLACK_COUNT + TOP_COUNT + COLOR_COUNT;

const BLACK_OFFSET: usize = 0;
const TOP_OFFSET: usize = BLACK_OFFSET + BLACK_COUNT;
const COLOR_OFFSET: usize = TOP_OFFSET + TOP_COUNT;

#[allow(clippy::large_const_arrays)]
const KEYS: [u64; TOTAL_COUNT] = {
    const SEED: u64 = 0x75e83deec533723c;

    let mut result = [0; TOTAL_COUNT];

    let mut prng = Sfc64::new(SEED);
    prng.fill(&mut result);

    result
};

#[must_use]
pub const fn black_key() -> u64 {
    KEYS[BLACK_OFFSET]
}

#[must_use]
pub const fn top_key(pt: PieceType, sq: Square) -> u64 {
    KEYS[TOP_OFFSET + sq.idx() * PieceType::COUNT + pt.idx()]
}

#[must_use]
pub const fn color_key(height: u8, color: Color, sq: Square) -> u64 {
    assert!((height as usize) < Position::MAX_STACK_HEIGHT);
    KEYS[COLOR_OFFSET
        + sq.idx() * Position::MAX_STACK_HEIGHT * Color::COUNT
        + height as usize * Color::COUNT
        + color.idx()]
}
