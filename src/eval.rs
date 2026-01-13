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
use crate::core::{Piece, Player};
use crate::search::Score;

#[must_use]
pub fn static_eval(pos: &Position) -> Score {
    let p1_flats = pos.player_piece_bb(Piece::P1Flat).popcount() as Score;
    let p2_flats = (pos.player_piece_bb(Piece::P2Flat).popcount() + Position::KOMI) as Score;

    let flat_diff = p1_flats - p2_flats;
    let flat_diff = flat_diff * 100;

    let p1_caps_in_hand = pos.caps_in_hand(Player::P1) as Score;
    let p2_caps_in_hand = pos.caps_in_hand(Player::P2) as Score;

    let caps_in_hand_diff = p1_caps_in_hand - p2_caps_in_hand;
    let caps_in_hand_diff = caps_in_hand_diff * -25;

    let eval = flat_diff + caps_in_hand_diff;

    match pos.stm() {
        Player::P1 => eval,
        Player::P2 => -eval,
    }
}
