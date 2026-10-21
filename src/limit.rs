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

use crate::takmove::Move;
use crate::thread::RootMove;
use std::time::Instant;

pub const MAX_MOVE_OVERHEAD_MS: u64 = 10000;

const TIME_CHECK_INTERVAL: usize = 4096;

#[derive(Clone, Debug, Default)]
pub struct TmContext {
    last_best_move: Option<Move>,
    stability: u32,
}

#[derive(Copy, Clone, Debug)]
struct TimeManager {
    max_time: f64,
    opt_time: f64,
}

impl TimeManager {
    fn new(remaining: f64, increment: f64, overhead: f64) -> Self {
        let remaining = (remaining - overhead).max(0.001);

        let base_time = remaining / 20.0 + increment;

        let max_time = remaining * 0.8;
        let opt_time = (base_time * 0.6).min(max_time);

        Self { max_time, opt_time }
    }

    fn should_stop_soft(
        &self,
        ctx: &mut TmContext,
        time: f64,
        depth: i32,
        total_nodes: usize,
        pv_move: &RootMove,
    ) -> bool {
        let mut scale = 1.0;

        let mv = pv_move.mv();
        if ctx.last_best_move == Some(mv) {
            ctx.stability += 1;
        } else {
            ctx.stability = 1;
            ctx.last_best_move = Some(mv);
        }

        if depth >= 5 {
            let stability = ctx.stability as f64;
            scale *= (0.8 + 8.6 * (stability + 0.9).powf(-2.6)).min(2.3);
        }

        let best_move_nodes_fraction = pv_move.nodes as f64 / total_nodes as f64;
        scale *= (2.5 - 2.5 * best_move_nodes_fraction).max(0.5);

        time >= self.opt_time * scale
    }

    fn should_stop_hard(&self, _nodes: usize, time: f64) -> bool {
        time >= self.max_time
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Limits {
    start_time: Instant,
    soft_nodes: Option<usize>,
    hard_nodes: Option<usize>,
    movetime: Option<f64>,
    time_manager: Option<TimeManager>,
}

impl Limits {
    pub fn new(start_time: Instant) -> Self {
        Self {
            start_time,
            soft_nodes: None,
            hard_nodes: None,
            movetime: None,
            time_manager: None,
        }
    }

    pub fn set_soft_nodes(&mut self, nodes: usize) -> bool {
        match self.soft_nodes {
            None => {
                self.soft_nodes = Some(nodes);
                true
            }
            Some(_) => false,
        }
    }

    pub fn set_hard_nodes(&mut self, nodes: usize) -> bool {
        match self.hard_nodes {
            None => {
                self.hard_nodes = Some(nodes);
                true
            }
            Some(_) => false,
        }
    }

    pub fn set_movetime(&mut self, movetime: f64) -> bool {
        match self.movetime {
            None => {
                self.movetime = Some(movetime);
                true
            }
            Some(_) => false,
        }
    }

    pub fn set_time_manager(&mut self, remaining: f64, increment: f64, overhead: f64) -> bool {
        match self.time_manager {
            None => {
                self.time_manager = Some(TimeManager::new(remaining, increment, overhead));
                true
            }
            Some(_) => false,
        }
    }

    #[must_use]
    pub fn should_stop_soft(&self, ctx: &mut TmContext, depth: i32, total_nodes: usize, pv_move: &RootMove) -> bool {
        if let Some(max_nodes) = self.soft_nodes
            && total_nodes >= max_nodes
        {
            return true;
        }

        let time = self.start_time.elapsed().as_secs_f64();

        if let Some(movetime) = self.movetime
            && time >= movetime
        {
            return true;
        }

        if let Some(time_manager) = self.time_manager
            && time_manager.should_stop_soft(ctx, time, depth, total_nodes, pv_move)
        {
            return true;
        }

        false
    }

    #[must_use]
    pub fn should_stop_hard(&self, nodes: usize) -> bool {
        if let Some(max_nodes) = self.hard_nodes
            && nodes >= max_nodes
        {
            return true;
        }

        if nodes > 0
            && nodes.is_multiple_of(TIME_CHECK_INTERVAL)
            && (self.movetime.is_some() || self.time_manager.is_some())
        {
            let time = self.start_time.elapsed().as_secs_f64();

            if let Some(movetime) = self.movetime
                && time >= movetime
            {
                return true;
            }

            if let Some(time_manager) = self.time_manager
                && time_manager.should_stop_hard(nodes, time)
            {
                return true;
            }
        }

        false
    }
}
