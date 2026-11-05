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
use crate::core::{Direction, PieceType, Score, Square};
use crate::history::HistorySlice;
use crate::hits::find_hits;
use crate::movegen::generate_moves;
use crate::position::Position;
use crate::road::influence;
use crate::takmove::Move;

#[allow(unused)]
use crate::stats;
use crate::tunable;

const SPREAD_ROAD_BONUS: i32 = 1 << 18;
tunable!(SPREAD_FLAT_GAIN: i32 = 5627; range(0, 8192));
tunable!(PLACEMENT_PENALTY: i32 = 78; range(-2048, 2048));
tunable!(WALL_SMASH_BONUS: i32 = 4215; range(0, 8192));
tunable!(KILLER_MOVE_BONUS: i32 = 16442; range(-32768, 32768));

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

    touches_opposite_edges(gained, pos.influence(pos.stm())) && influence(info.roads).0
}

fn touches_opposite_edges(bb: Bitboard, infl: &[Bitboard; 4]) -> bool {
    let [up, down, left, right] = infl.map(|inf| !(inf & bb).is_empty());
    (up && down) || (left && right)
}

fn first_winning_spread(
    pos: &Position,
    sq: Square,
    dir: Direction,
    lsb: u16,
    mut pattern: u16,
    max_dist: u8,
    limit: u16,
) -> Option<Move> {
    while max_dist > 0 && pattern < limit {
        let mv = Move::spread(sq, dir, pattern);
        if spread_wins_road(pos, &spread_ordering_info(pos, mv)) {
            return Some(mv);
        }
        if pattern.count_ones() == u32::from(max_dist) {
            pattern += pattern.isolate_lowest_one();
        } else {
            pattern += lsb;
        }
    }
    None
}

#[must_use]
pub fn spread_road_win(pos: &Position) -> Option<Move> {
    let us = pos.stm();
    let infl = pos.influence(us);
    let criticals = (infl[0] & infl[1]) | (infl[2] & infl[3]);
    let near = criticals
        | criticals.shift(Direction::Up)
        | criticals.shift(Direction::Down)
        | criticals.shift(Direction::Left)
        | criticals.shift(Direction::Right);
    let top_bit = 1 << (Position::CARRY_LIMIT - 1);
    let limit = 1 << Position::CARRY_LIMIT;

    for sq in pos.color_bb(us) {
        let max_carry = pos.height(sq).min(Position::CARRY_LIMIT as u8);
        if max_carry == 1 && !near.has_sq(sq) {
            continue;
        }
        let lsb = limit >> max_carry;
        let capstone = pos.top_pt(sq) == Some(PieceType::Capstone);
        let hits = find_hits(pos.all_blockers(), sq);

        for dir in [Direction::Up, Direction::Down, Direction::Left, Direction::Right] {
            let (dist, hit_sq) = hits[dir];
            if dist == 0 {
                continue;
            }

            let can_smash = capstone && pos.top_pt(hit_sq) == Some(PieceType::Wall);
            let blocked = !can_smash && pos.all_blockers().has_sq(hit_sq);
            let max_dist = max_carry.min(dist - u8::from(blocked));

            let step = u32::from(dir.offset().unsigned_abs());
            let span = u32::from(max_dist) * step;
            let mask = if step == 6 {
                Bitboard::LEFT_EDGE
            } else {
                Bitboard::LOWER_EDGE
            };
            let prefix = mask >> (step * (Position::CARRY_LIMIT - u32::from(max_dist)));
            let (line, last) = match dir {
                Direction::Up | Direction::Right => (prefix << (sq.idx() as u32 + step), sq.bb() << span),
                Direction::Down | Direction::Left => (prefix << (sq.idx() as u32 - span), sq.bb() >> span),
            };
            let gainable = if pos.roads(us).has_sq(sq) {
                line
            } else {
                (line ^ last) | sq.bb()
            };
            if !touches_opposite_edges(gainable, infl) {
                continue;
            }

            let win = if can_smash {
                let non_smash_dist = max_dist - u8::from(dist <= max_carry);
                first_winning_spread(pos, sq, dir, lsb, top_bit, max_dist, limit)
                    .or_else(|| first_winning_spread(pos, sq, dir, lsb, lsb, non_smash_dist, top_bit))
            } else {
                first_winning_spread(pos, sq, dir, lsb, lsb, max_dist, limit)
            };

            if win.is_some() {
                return win;
            }
        }
    }

    None
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
    const COUNT: usize = 2;

    pub fn push(&mut self, mv: Move) {
        if self.killers[0] != Some(mv) {
            self.killers[1] = self.killers[0];
            self.killers[0] = Some(mv);
        }
    }
}

pub struct Movepicker<'a> {
    pos: &'a Position,
    moves: &'a mut Vec<Move>,
    scores: &'a mut Vec<i32>,
    idx: usize,
    tt_move: Option<Move>,
    killers: KillerTable,
    history: HistorySlice<'a>,
    stage: Stage,
}

impl<'a> Movepicker<'a> {
    pub fn new(
        pos: &'a Position,
        moves: &'a mut Vec<Move>,
        scores: &'a mut Vec<Score>,
        tt_move: Option<Move>,
        killers: KillerTable,
        history: HistorySlice<'a>,
    ) -> Self {
        Self {
            pos,
            moves,
            scores,
            idx: 0,
            tt_move,
            killers,
            history,
            stage: Stage::TtMove,
        }
    }

    fn score_moves(&mut self) {
        self.scores.clear();
        for &mv in self.moves.iter() {
            let mut score = self.history.score(mv);

            if self.killers.killers[1] == Some(mv) {
                score += KILLER_MOVE_BONUS;
            }
            if !mv.is_spread() {
                score -= PLACEMENT_PENALTY;
            } else {
                let info = spread_ordering_info(self.pos, mv);
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

    pub fn next(&mut self) -> Option<Move> {
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
                    self.score_moves();
                }
                Stage::Moves => {
                    while self.idx < self.moves.len() {
                        let mv = self.pick_best();
                        self.idx += 1;
                        if self.tt_move.is_none_or(|tt_move| mv != tt_move) && self.killers.killers[0] != Some(mv) {
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
