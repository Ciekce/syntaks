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
use crate::core::{PieceType, Score};
use crate::history::History;
use crate::movegen::generate_moves;
use crate::position::Position;
use crate::road::influence;
use crate::takmove::Move;

const SPREAD_ROAD_BONUS: i32 = 1 << 18;
const SPREAD_FLAT_GAIN: i32 = 4096;
const WALL_SMASH_BONUS: i32 = 4096;

struct SpreadInfo {
    roads: Bitboard,
    flat_delta: i32,
}

fn spread_ordering_info(pos: &Position, mv: Move) -> SpreadInfo {
    let src = mv.sq();
    let dir = mv.dir();
    let us = pos.stm();
    let roads = pos.roads(us);
    let flats = pos.piece_bb(PieceType::Flat);
    let dropped = mv.pattern().trailing_zeros();
    let taken = Position::CARRY_LIMIT - dropped;
    let own_colors = pos.colors_for(src, us);

    let mut new_roads_bb = Bitboard::empty();
    let mut flats_after = 0;
    if (own_colors >> taken) & 1 != 0 {
        new_roads_bb.set_sq(src);
        flats_after += 1;
    } else if u32::from(pos.height(src)) > taken {
        flats_after -= 1;
    }

    let mut dirty_bb = src.bb();
    let mut pattern = mv.pattern() >> dropped;
    let mut sq = src.shift(dir).unwrap();

    for color_idx in (1..taken).rev() {
        pattern >>= 1;
        if pattern & 1 != 0 {
            dirty_bb.set_sq(sq);
            if (own_colors >> color_idx) & 1 != 0 {
                new_roads_bb.set_sq(sq);
                flats_after += 1;
            } else {
                flats_after -= 1;
            }
            sq = sq.shift(dir).unwrap();
        }
    }

    dirty_bb.set_sq(sq);
    if roads.has_sq(src) {
        new_roads_bb.set_sq(sq);
    }
    if flats.has_sq(src) {
        flats_after += 1;
    }

    let flats_before = (dirty_bb & flats & pos.color_bb(us)).popcount() as i32
        - (dirty_bb & flats & pos.color_bb(us.flip())).popcount() as i32;

    SpreadInfo {
        roads: (roads & !dirty_bb) | new_roads_bb,
        flat_delta: flats_after - flats_before,
    }
}

fn spread_wins_road(pos: &Position, info: &SpreadInfo) -> bool {
    let gained = info.roads & !pos.roads(pos.stm());

    let [touches_up, touches_down, touches_left, touches_right] =
        pos.influence(pos.stm()).map(|bb| !(bb & gained).is_empty());

    ((touches_up && touches_down) || (touches_left && touches_right)) && influence(info.roads).0
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Stage {
    TtMove,
    Killer1,
    GenMoves,
    Moves,
    End,
}

impl Stage {
    fn next(&self) -> Self {
        assert_ne!(*self, Self::End);
        match *self {
            Self::TtMove => Self::Killer1,
            Self::Killer1 => Self::GenMoves,
            Self::GenMoves => Self::Moves,
            Self::Moves => Self::End,
            Self::End => unreachable!(),
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct KillerTable {
    killers: [Option<Move>; Self::COUNT],
}

impl KillerTable {
    const COUNT: usize = 1;

    pub fn push(&mut self, mv: Move) {
        self.killers[0] = Some(mv);
    }

    #[must_use]
    pub fn contains(&self, mv: Move) -> bool {
        self.killers.contains(&Some(mv))
    }
}

pub struct Movepicker<'a> {
    pos: &'a Position,
    moves: &'a mut Vec<Move>,
    scores: &'a mut Vec<i32>,
    idx: usize,
    tt_move: Option<Move>,
    killers: KillerTable,
    prev_move: Option<Move>,
    stage: Stage,
}

impl<'a> Movepicker<'a> {
    pub fn new(
        pos: &'a Position,
        moves: &'a mut Vec<Move>,
        scores: &'a mut Vec<Score>,
        tt_move: Option<Move>,
        killers: KillerTable,
        prev_move: Option<Move>,
    ) -> Self {
        Self {
            pos,
            moves,
            scores,
            idx: 0,
            tt_move,
            killers,
            prev_move,
            stage: Stage::TtMove,
        }
    }

    fn score_moves(&mut self, history: &History) {
        self.scores.clear();
        for mv in self.moves.iter() {
            let mut score = history.score(self.pos, *mv, self.prev_move);

            if !mv.is_spread() {
                score += 75;
            } else {
                let info = spread_ordering_info(self.pos, *mv);
                if spread_wins_road(self.pos, &info) {
                    score += SPREAD_ROAD_BONUS;
                }
                score += SPREAD_FLAT_GAIN * info.flat_delta;
                if self.pos.top_pt(mv.spread_dest()) == Some(PieceType::Wall) {
                    score += WALL_SMASH_BONUS;
                }
            }

            // scores have to be less than 2²⁰ to make the movepicking more efficient
            debug_assert!(score.abs() < (1 << 20));

            self.scores.push(score);
        }
    }

    fn pick_best(&mut self) -> Move {
        let packed_best = self
            .scores
            .iter()
            .enumerate()
            .skip(self.idx)
            .map(|(i, score)| *score << 11 | ((i as u32 ^ 0x7ff) as i32))
            .max()
            .unwrap();

        let best_idx = (!packed_best & 0x7ff) as usize;

        self.scores.swap(self.idx, best_idx);
        self.moves.swap(self.idx, best_idx);

        self.moves[self.idx]
    }

    pub fn next(&mut self, history: &History) -> Option<Move> {
        while self.stage != Stage::End {
            match self.stage {
                Stage::TtMove => {
                    if let Some(tt_move) = self.tt_move
                        && self.pos.is_legal(tt_move)
                    {
                        self.stage = self.stage.next();
                        return Some(tt_move);
                    }
                }
                Stage::Killer1 => {
                    if let Some(killer) = self.killers.killers[0]
                        && self.tt_move.is_none_or(|tt_move| killer != tt_move)
                        && self.pos.is_legal(killer)
                    {
                        self.stage = self.stage.next();
                        return Some(killer);
                    }
                }
                Stage::GenMoves => {
                    generate_moves(self.moves, self.pos);
                    self.score_moves(history);
                }
                Stage::Moves => {
                    while self.idx < self.moves.len() {
                        let mv = self.pick_best();
                        self.idx += 1;
                        if self.tt_move.is_none_or(|tt_move| mv != tt_move) && !self.killers.contains(mv) {
                            return Some(mv);
                        }
                    }
                }
                Stage::End => unreachable!(),
            }

            self.stage = self.stage.next();
        }

        None
    }
}
