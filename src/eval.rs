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
use crate::core::{Direction, Piece, PieceType, Player, Square};
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

#[rustfmt::skip]
const CAP_PSQT: [Score; Square::COUNT] = [
    -45, -30, -30, -30, -30, -45,
    -30, -15,  -7,  -7, -15, -30,
    -30,  -7,  10,  10,  -7, -30,
    -30,  -7,  10,  10,  -7, -30,
    -30, -15,  -7,  -7, -15, -30,
    -45, -30, -30, -30, -30, -45,
];

#[must_use]
fn static_eval_player(pos: &Position, player: Player, komi: u32) -> Score {
    let flat_bb = pos.player_piece_bb(PieceType::Flat.with_player(player));
    let flats = (flat_bb.popcount() + komi) as Score;
    let flats = flats * 75;

    let flats_in_hand = pos.flats_in_hand(player) as Score;
    let flats_in_hand = flats_in_hand * -13;

    let caps_in_hand = pos.caps_in_hand(player) as Score;
    let caps_in_hand = caps_in_hand * -25;

    let road_bb = pos.roads(player);

    let adj_horz = road_bb & road_bb.shift(Direction::Left);
    let adj_vert = road_bb & road_bb.shift(Direction::Down);

    let line_horz = adj_horz & adj_horz.shift(Direction::Left);
    let line_vert = adj_vert & adj_vert.shift(Direction::Down);

    let adj_value = (adj_horz.popcount() + adj_vert.popcount()) as Score;
    let line_value = (line_horz.popcount() + line_vert.popcount()) as Score;

    let adj_value = adj_value * 9;
    let line_value = line_value * 7;

    let stacks = &pos.stacks();
    let player_flip = if player == Player::P2 { u64::MAX } else { 0 };

    let mut support_score = 0;
    let mut captive_score = 0;

    for sq in pos.player_bb(player) {
        let mut height = stacks.height(sq);

        if height == 1 {
            continue;
        }

        let mut players = stacks.players(sq) ^ player_flip;

        if height > 7 {
            players >>= height - 7;
            height = 7;
        }

        let mask = (1 << (height - 1)) - 1;

        let support_count = (!players & mask).count_ones() as Score;
        let captive_count = (players & mask).count_ones() as Score;

        match stacks.top(sq).unwrap() {
            PieceType::Flat => {
                support_score += support_count * 30;
                captive_score += captive_count * -40;
            }
            PieceType::Wall => {
                support_score += support_count * 35;
                captive_score += captive_count * -15;
            }
            PieceType::Capstone => {
                support_score += support_count * 40;
                captive_score += captive_count * -20;
            }
        }
    }

    let mut psqt_score = 0;

    for cap_sq in pos.player_piece_bb(PieceType::Capstone.with_player(player)) {
        psqt_score += CAP_PSQT[cap_sq.idx()];
    }

    flats
        + flats_in_hand
        + caps_in_hand
        + adj_value
        + line_value
        + support_score
        + captive_score
        + psqt_score
}

#[must_use]
pub fn static_eval(pos: &Position) -> Score {
    let p1_score = static_eval_player(pos, Player::P1, 0);
    let p2_score = static_eval_player(pos, Player::P2, Position::KOMI);

    let p1_flat_bb = pos.player_piece_bb(Piece::P1Flat);
    let p2_flat_bb = pos.player_piece_bb(Piece::P2Flat);

    let flat_position_quality_diff = RINGS
        .iter()
        .zip([2, 8, -5, -15, -40])
        .map(|(&ring, value)| {
            (p1_flat_bb & ring).popcount() as i32 * value
                - (p2_flat_bb & ring).popcount() as i32 * value
        })
        .sum::<i32>();

    let eval = p1_score - p2_score + flat_position_quality_diff;

    eval * pos.stm().sign() + 30
}
