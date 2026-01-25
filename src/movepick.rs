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
use crate::history::History;
use crate::movegen::generate_moves;
use crate::search::Score;
use crate::takmove::Move;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Stage {
    TtMove,
    Killer1,
    Killer2,
    GenMoves,
    Moves,
    End,
}

impl Stage {
    fn next(&self) -> Self {
        assert_ne!(*self, Self::End);
        match *self {
            Self::TtMove => Self::Killer1,
            Self::Killer1 => Self::Killer2,
            Self::Killer2 => Self::GenMoves,
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

    #[must_use]
    pub fn contains(&self, mv: Move) -> bool {
        self.killers.contains(&Some(mv))
    }

    pub fn reset(&mut self) {
        *self = Default::default();
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
                score += 100;
            }

            self.scores.push(score);
        }
    }

    fn pick_best(&mut self) -> Move {
        let packed_best = self
            .scores
            .iter()
            .enumerate()
            .skip(self.idx)
            .map(|(i, score)| (*score as i64) << 32 | ((!i as u32) as i64))
            .max()
            .unwrap();

        let best_idx = !packed_best as u32 as usize;

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
                Stage::Killer2 => {
                    if let Some(killer) = self.killers.killers[1]
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
                        if self.tt_move.is_none_or(|tt_move| mv != tt_move)
                            && !self.killers.contains(mv)
                        {
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
