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

use crate::datagen::GameResult;
use crate::search::Score;
use crate::takmove::Move;
use std::io::Write;

#[derive(Copy, Clone, Debug, bytemuck::NoUninit)]
#[repr(C)]
struct ScoredMove {
    mv: u16,
    score: i16,
}

const _SCORED_MOVE_SIZE: () =
    assert!(size_of::<ScoredMove>() == (size_of::<u16>() + size_of::<i16>()));

impl ScoredMove {
    fn new(mv: Move, score: Score) -> Self {
        Self {
            mv: mv.raw(),
            score: score as i16,
        }
    }
}

pub(super) struct SynpackWriter {
    unscored_moves: Vec<u16>,
    moves: Vec<ScoredMove>,
}

impl SynpackWriter {
    pub(super) fn new() -> Self {
        Self {
            unscored_moves: Vec::with_capacity(16),
            moves: Vec::with_capacity(1024),
        }
    }

    pub(super) fn start(&mut self) {
        self.unscored_moves.clear();
        self.moves.clear();
    }

    pub(super) fn push_unscored(&mut self, mv: Move) {
        self.unscored_moves.push(mv.raw());
    }

    pub(super) fn push(&mut self, mv: Move, score: Score) {
        self.moves.push(ScoredMove::new(mv, score));
    }

    pub(super) fn write_all_with_outcome(
        &self,
        writer: &mut dyn Write,
        outcome: GameResult,
    ) -> std::io::Result<usize> {
        const STANDARD_TYPE: u8 = 0;
        const NULL_TERMINATOR: [u8; 4] = [0; 4];

        #[cfg(not(target_endian = "little"))]
        {
            error!("not little endian");
        }

        let outcome = match outcome {
            GameResult::Loss => 0,
            GameResult::Draw => 1,
            GameResult::Win => 2,
        };

        let wdl_type = (outcome << 6) | STANDARD_TYPE;
        writer.write_all(&[wdl_type])?;

        let count = self.moves.len();

        let unscored_count = self.unscored_moves.len() as u16;
        writer.write_all(&unscored_count.to_le_bytes())?;
        writer.write_all(bytemuck::cast_slice(&self.unscored_moves))?;

        writer.write_all(bytemuck::cast_slice(&self.moves))?;
        writer.write_all(&NULL_TERMINATOR)?;

        Ok(count)
    }
}
