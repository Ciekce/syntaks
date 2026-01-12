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

use crate::board::Stacks;
use crate::core::*;

struct Sfc64 {
    a: u64,
    b: u64,
    c: u64,
    counter: u64,
}

impl Sfc64 {
    const fn new(seed: u64) -> Self {
        let mut result = Self {
            a: seed,
            b: seed,
            c: seed,
            counter: 1,
        };

        let mut i = 0;
        while i < 12 {
            result.next_u64();
            i += 1;
        }

        result
    }

    const fn next_u64(&mut self) -> u64 {
        let result = self.a.wrapping_add(self.b).wrapping_add(self.counter);
        self.counter = self.counter.wrapping_add(1);
        self.a = self.b ^ (self.b >> 11);
        self.b = self.c.wrapping_add(self.c << 3);
        self.c = self.c.rotate_left(24).wrapping_add(result);
        result
    }

    const fn fill(&mut self, values: &mut [u64]) {
        let mut idx = 0;
        while idx < values.len() {
            values[idx] = self.next_u64();
            idx += 1;
        }
    }
}

const P2_COUNT: usize = 1;
const TOP_COUNT: usize = PieceType::COUNT * Square::COUNT;
const PLAYER_COUNT: usize = Stacks::MAX_HEIGHT * Player::COUNT * Square::COUNT;

const TOTAL_COUNT: usize = P2_COUNT + TOP_COUNT + PLAYER_COUNT;

const P2_OFFSET: usize = 0;
const TOP_OFFSET: usize = P2_OFFSET + P2_COUNT;
const PLAYER_OFFSET: usize = TOP_OFFSET + TOP_COUNT;

#[allow(clippy::large_const_arrays)]
const KEYS: [u64; TOTAL_COUNT] = {
    const SEED: u64 = 0x75e83deec533723c;

    let mut result = [0; TOTAL_COUNT];

    let mut prng = Sfc64::new(SEED);
    prng.fill(&mut result);

    result
};

#[must_use]
pub const fn p2_key() -> u64 {
    KEYS[P2_OFFSET]
}

#[must_use]
pub const fn top_key(pt: PieceType, sq: Square) -> u64 {
    KEYS[TOP_OFFSET + sq.idx() * PieceType::COUNT + pt.idx()]
}

#[must_use]
pub const fn player_key(height: u8, player: Player, sq: Square) -> u64 {
    assert!((height as usize) < Stacks::MAX_HEIGHT);
    KEYS[PLAYER_OFFSET
        + sq.idx() * Stacks::MAX_HEIGHT * Player::COUNT
        + height as usize * Player::COUNT
        + player.idx()]
}
