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

pub struct Movepicker<'a> {
    pos: &'a Position,
    moves: &'a mut Vec<Move>,
    idx: usize,
    tt_move: Option<Move>,
    stage: Stage,
}

impl<'a> Movepicker<'a> {
    pub fn new(pos: &'a Position, moves: &'a mut Vec<Move>, tt_move: Option<Move>) -> Self {
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
                Stage::GenMoves => generate_moves(self.moves, self.pos),
                Stage::Moves => {
                    while self.idx < self.moves.len() {
                        let mv = self.moves[self.idx];
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
