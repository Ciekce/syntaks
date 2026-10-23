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

use crate::core::{Color, PieceType, Score};
use crate::position::Position;

const WDL_A: f64 = 222.13;
const WDL_B: f64 = 138.82;

#[must_use]
pub fn normalise_score(score: Score) -> Score {
    (score as f64 * 100.0 / WDL_A).round() as Score
}

#[must_use]
pub fn probabilities(score: Score) -> (i32, i32, i32) {
    wdl(score as f64, WDL_A, WDL_B)
}

const HUMAN_A_SCALE: f64 = 20.0;
const HUMAN_A_BASE: f64 = 4.664196675480554;
const HUMAN_A_WALLS: f64 = 5.489418921388072;
const HUMAN_B_BASE: f64 = 5.496131687926765;
const HUMAN_B_WALLS: f64 = 0.13694544881346082;
const HUMAN_B_RESERVES_WALLS: f64 = 1.1199382695143412;

#[must_use]
pub fn human_probabilities(score: Score, pos: &Position) -> (i32, i32, i32) {
    let walls = pos.piece_bb(PieceType::Wall).popcount().min(12) as f64 / 6.0 - 1.0;

    let in_hand = pos.flats_in_hand(Color::Black)
        + pos.caps_in_hand(Color::Black)
        + pos.flats_in_hand(Color::White)
        + pos.caps_in_hand(Color::White);
    let reserves = (in_hand as f64 - 31.0) / 31.0;

    let a = HUMAN_A_SCALE * (HUMAN_A_BASE + HUMAN_A_WALLS * walls).exp().ln_1p();
    let b = (HUMAN_B_BASE + HUMAN_B_WALLS * walls + HUMAN_B_RESERVES_WALLS * reserves * walls).exp();

    let x = score as f64 * 100.0 / WDL_A;

    wdl(x, a, b)
}

fn wdl(x: f64, a: f64, b: f64) -> (i32, i32, i32) {
    let p = |x: f64| (1000.0 / (1.0 + (-x).exp())).round() as i32;
    let w = p((x - a) / b);
    let l = p((-x - a) / b);

    (w, 1000 - w - l, l)
}
