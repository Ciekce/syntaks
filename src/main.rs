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

use crate::board::Position;
use crate::core::Player;

mod bitboard;
mod board;
mod core;
mod correction;
mod eval;
mod history;
mod hits;
mod keys;
mod limit;
mod movegen;
mod movepick;
mod perft;
mod road;
mod search;
mod takmove;
mod tei;
mod thread;
mod ttable;

fn main() {
    let pos = "x3,2,x,1/x,2,2,x2,2/x3,1,2,1/1,1,21,1,221C,1/x2,1,21,12,x/2,x2,1,112C,1 2 23"
        .parse::<Position>()
        .unwrap();
    assert!(pos.has_road(Player::P1));
    assert!(!pos.has_road(Player::P2));
    tei::run();
}
