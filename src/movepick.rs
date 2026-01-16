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
    GenMoves,
    Moves,
    End,
}

impl Stage {
    fn next(&self) -> Self {
        assert_ne!(*self, Self::End);
        match *self {
            Self::TtMove => Self::GenMoves,
            Self::GenMoves => Self::Moves,
            Self::Moves => Self::End,
            Self::End => unreachable!(),
        }
    }
}

pub struct Movepicker<'a> {
    pos: &'a Position,
    moves: &'a mut Vec<Move>,
    scores: &'a mut Vec<i32>,
    idx: usize,
    tt_move: Option<Move>,
    stage: Stage,
}

impl<'a> Movepicker<'a> {
    pub fn new(
        pos: &'a Position,
        moves: &'a mut Vec<Move>,
        scores: &'a mut Vec<Score>,
        tt_move: Option<Move>,
    ) -> Self {
        Self {
            pos,
            moves,
            scores,
            idx: 0,
            tt_move,
            stage: Stage::TtMove,
        }
    }

    fn score_moves(&mut self, history: &History) {
        self.scores.clear();
        for mv in self.moves.iter() {
            self.scores.push(history.score(self.pos, *mv));
        }
    }

    fn pick_best(&mut self) -> Move {
        let mut best_score = self.scores[self.idx];
        let mut best_idx = self.idx;

        let mut i = self.idx + 1;
        while i < self.moves.len() {
            if self.scores[i] > best_score {
                best_score = self.scores[i];
                best_idx = i;
            }
            i += 1;
        }

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
                Stage::GenMoves => {
                    generate_moves(self.moves, self.pos);
                    self.score_moves(history);
                }
                Stage::Moves => {
                    while self.idx < self.moves.len() {
                        let mv = self.pick_best();
                        self.idx += 1;
                        if self.tt_move.is_none_or(|tt_move| mv != tt_move) {
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
