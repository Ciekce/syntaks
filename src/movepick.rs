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
use crate::core::PieceType;
use crate::movegen::generate_moves;
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

pub type ScoredMove = (Move, i32);

pub struct Movepicker<'a> {
    pos: &'a Position,
    moves: &'a mut Vec<ScoredMove>,
    idx: usize,
    tt_move: Option<Move>,
    stage: Stage,
}

impl<'a> Movepicker<'a> {
    pub fn new(pos: &'a Position, moves: &'a mut Vec<ScoredMove>, tt_move: Option<Move>) -> Self {
        Self {
            pos,
            moves,
            idx: 0,
            tt_move,
            stage: Stage::TtMove,
        }
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
                Stage::GenMoves => {
                    self.moves.clear();
                    generate_moves(&mut |mv| self.moves.push((mv, 0)), self.pos);
                    self.score_moves();
                }
                Stage::Moves => {
                    let tt_move = self.tt_move;
                    if let Some(mv) =
                        self.select_next(&|mv| tt_move.is_none_or(|tt_move| mv != tt_move))
                    {
                        return Some(mv);
                    }
                }
                Stage::End => unreachable!(),
            }

            self.stage = self.stage.next();
        }

        None
    }

    fn score_moves(&mut self) {
        for (mv, score) in self.moves.iter_mut() {
            if !mv.is_spread() {
                *score = match mv.pt() {
                    PieceType::Flat => 1,
                    PieceType::Wall => 0,
                    PieceType::Capstone => 2,
                };
            }
        }
    }

    fn select_next<F: Fn(Move) -> bool>(&mut self, predicate: &F) -> Option<Move> {
        while self.idx < self.moves.len() {
            let idx = self.find_next();
            let mv = self.moves[idx].0;
            if predicate(mv) {
                return Some(mv);
            }
        }

        None
    }

    fn find_next(&mut self) -> usize {
        let mut best_idx = self.idx;
        let mut best_score = self.moves[self.idx].1;

        for (idx, &(_, score)) in self.moves[(self.idx + 1)..].iter().enumerate() {
            if score > best_score {
                best_idx = self.idx + 1 + idx;
                best_score = score;
            }
        }

        let idx = self.idx;

        if idx != best_idx {
            self.moves.swap(idx, best_idx);
        }

        self.idx += 1;

        idx
    }
}
