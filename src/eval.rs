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

use std::array;

use crate::bitboard::Bitboard;
use crate::board::Position;
use crate::core::{Piece, Player};
use crate::search::Score;

#[static_init::dynamic]
static RINGS: [Bitboard; 5] = {
    let mut covered = Bitboard::from_raw(1 << 14 | 1 << 15 | 1 << 20 | 1 << 21);
    let mut curr = covered;
    array::from_fn(|_| {
        let r = curr;
        curr = (curr << 6 | curr >> 6 | curr << 1 | curr >> 1) & !covered;
        covered |= curr;
        r
    })
};

#[must_use]
pub fn static_eval(pos: &Position) -> Score {
    let p1_flat_bb = pos.player_piece_bb(Piece::P1Flat);
    let p2_flat_bb = pos.player_piece_bb(Piece::P2Flat);

    let p1_flats = p1_flat_bb.popcount() as Score;
    let p2_flats = (p2_flat_bb.popcount() + Position::KOMI) as Score;

    let flat_diff = p1_flats - p2_flats;
    let flat_diff = flat_diff * 100;

    let p1_flats_in_hand = pos.flats_in_hand(Player::P1) as Score;
    let p2_flats_in_hand = pos.flats_in_hand(Player::P2) as Score;

    let flats_in_hand_diff = p1_flats_in_hand - p2_flats_in_hand;
    let flats_in_hand_diff = flats_in_hand_diff * -13;

    let p1_caps_in_hand = pos.caps_in_hand(Player::P1) as Score;
    let p2_caps_in_hand = pos.caps_in_hand(Player::P2) as Score;

    let caps_in_hand_diff = p1_caps_in_hand - p2_caps_in_hand;
    let caps_in_hand_diff = caps_in_hand_diff * -25;

    let flat_position_quality_diff = RINGS
        .iter()
        .zip([2, 8, -5, -15, -40])
        .map(|(&ring, value)| {
            (p1_flat_bb & ring).popcount() as i32 * value
                - (p2_flat_bb & ring).popcount() as i32 * value
        })
        .sum::<i32>();

    let eval = flat_diff + caps_in_hand_diff + flats_in_hand_diff + flat_position_quality_diff;

    match pos.stm() {
        Player::P1 => eval,
        Player::P2 => -eval,
    }
}
