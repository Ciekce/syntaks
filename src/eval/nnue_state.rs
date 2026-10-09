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
use crate::core::Player;
use crate::eval::accumulator::Accumulator;
use crate::eval::forward::forward;
use crate::eval::nnue::*;
use crate::search::MAX_DEPTH;

pub struct NnueState {
    acc_stacc: Vec<Accumulator>,
    top_idx: usize,
}

impl NnueState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            acc_stacc: vec![Default::default(); (MAX_DEPTH + 1) as usize],
            top_idx: 0,
        }
    }

    pub fn reset(&mut self, pos: &Position) {
        self.acc_stacc[0].reset_both(pos);
        self.top_idx = 0;
    }

    #[must_use]
    pub fn push(&mut self) -> NnueObserver<'_> {
        self.top_idx += 1;

        let top = &mut self.acc_stacc[self.top_idx];

        top.ctx = Default::default();
        top.set_dirty();

        NnueObserver::new(&mut top.ctx)
    }

    pub fn pop(&mut self) {
        self.top_idx -= 1;
    }

    #[must_use]
    pub fn evaluate(&mut self, pos: &Position) -> i32 {
        self.ensure_up_to_date(pos);
        forward(&self.acc_stacc[self.top_idx], pos.stm())
    }

    fn ensure_up_to_date(&mut self, _pos: &Position) {
        if !self.acc_stacc[self.top_idx].is_dirty() {
            return;
        }

        let mut curr = self.top_idx - 1;
        while self.acc_stacc[curr].is_dirty() {
            curr -= 1;
        }

        loop {
            let [prev_acc, curr_acc] = self.acc_stacc.get_disjoint_mut([curr, curr + 1]).unwrap();

            curr_acc.apply_updates(prev_acc, Player::P1);
            curr_acc.apply_updates(prev_acc, Player::P2);

            curr_acc.set_updated();

            curr += 1;
            if curr == self.top_idx {
                break;
            }
        }
    }
}
