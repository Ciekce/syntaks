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

const ADJACENT_MASKS: [Bitboard; Square::COUNT] = {
    let mut masks = [Bitboard::empty(); Square::COUNT];

    let mut sq_idx = 0;
    while let Some(sq) = Square::from_raw(sq_idx) {
        let bb = sq.bb();
        let bb = bb
            .shift(Direction::Up)
            .or(bb.shift(Direction::Down))
            .or(bb.shift(Direction::Left))
            .or(bb.shift(Direction::Right));
        masks[sq.idx()] = bb;
        sq_idx += 1;
    }

    masks
};

#[rustfmt::skip]
const CAP_PSQT: [Score; Square::COUNT] = [
    -20,  -5,  -5,  -5,  -5, -20,
     -5,  10,  18,  18,  10,  -5,
     -5,  18,  35,  35,  18,  -5,
     -5,  18,  35,  35,  18,  -5,
     -5,  10,  18,  18,  10,  -5,
    -20,  -5,  -5,  -5,  -5, -20,
];

#[must_use]
fn static_eval_player(pos: &Position, player: Player, komi: u32) -> Score {
    let flat_bb = pos.player_piece_bb(PieceType::Flat.with_player(player));
    let flats = (flat_bb.popcount() + komi) as Score;
    let flats = flats * 75;

    let flats_in_hand = pos.flats_in_hand(player) as Score;
    let flats_in_hand = flats_in_hand * -13;

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

    let isolated_mask = pos.occ() & !flat_bb;

    let opp_vulnerabilities = {
        let mut vulnerabilities = Bitboard::empty();

        for sq in pos.player_bb(player.flip()) {
            let mut height = stacks.height(sq);

            if height == 1 || stacks.top(sq).unwrap() != PieceType::Flat {
                continue;
            }

            let mut players = stacks.players(sq) ^ player_flip;

            if height > 7 {
                players >>= height - 7;
                height = 7;
            }

            let mask = (1 << (height - 1)) - 1;

            if (!players & mask) != 0 {
                vulnerabilities.set_sq(sq);
            }
        }

        vulnerabilities
    };

    let mut psqt_score = 0;
    let mut isolated_cap_score = 0;
    let mut vulnerability_threat_score = 0;

    for cap_sq in pos.player_piece_bb(PieceType::Capstone.with_player(player)) {
        psqt_score += CAP_PSQT[cap_sq.idx()];

        let adjacent = ADJACENT_MASKS[cap_sq.idx()];

        if (adjacent & isolated_mask).is_empty() {
            isolated_cap_score -= 50;
        }

        let threatened_vulnerabilities = opp_vulnerabilities & adjacent;
        vulnerability_threat_score += threatened_vulnerabilities.popcount() as Score * 25;
    }

    flats
        + flats_in_hand
        + adj_value
        + line_value
        + support_score
        + captive_score
        + psqt_score
        + isolated_cap_score
        + vulnerability_threat_score
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
