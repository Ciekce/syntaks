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

use crate::core::{Color, Score};
use crate::position::Position;
use crate::takmove::Move;
use crate::thread::RootMove;
use crate::tunable;
use crate::tune::Val;
use std::time::Instant;

#[allow(unused)]
use crate::stats;

pub const MAX_MOVE_OVERHEAD_MS: u64 = 10000;

const TIME_CHECK_INTERVAL: usize = 4096;

const MAX_STONES_IN_HAND: u8 = 62;

tunable!(BASE_TIME_SCALE: i32 = 1024; range(1, 4096));
tunable!(INC_SCALE: i32 = 1024; range(1, 4096));

tunable!(STONE_COMPLEXITY_SCALE: i32 = 5120; range(1, 10240));
tunable!(STONE_COMPLEXITY_BASE: i32 = 17920; range(1, 32768));

tunable!(OPT_TIME_SCALE: i32 = 614; range(256, 1536));

tunable!(BM_STABILITY_MIN: i32 = 819; range(1, 2048));
tunable!(BM_STABILITY_MAX: i32 = 2355; range(1024, 4096));
tunable!(BM_STABILITY_SCALE: i32 = 8806; range(1024, 16384));
tunable!(BM_STABILITY_OFFSET: i32 = 922; range(0, 4096));
tunable!(BM_STABILITY_POWER: i32 = -2662; range(-4096, -1024));

tunable!(SCORE_DROP_DIVISOR: i32 = 86016; range(32768, 131072));
tunable!(SCORE_DROP_MAX: i32 = 1741; range(640, 3072));

tunable!(NODE_TM_BASE: i32 = 2560; range(1536, 3072));
tunable!(NODE_TM_SCALE: i32 = 2560; range(1536, 4096));
tunable!(NODE_TM_MIN: i32 = 512; range(1, 1024));

#[derive(Clone, Debug, Default)]
pub struct TmContext {
    last_best_move: Option<Move>,
    stability: u32,
    avg_score: Option<Score>,
}

#[derive(Copy, Clone, Debug)]
struct TimeManager {
    max_time: f64,
    opt_time: f64,
}

impl TimeManager {
    fn new(remaining: f64, increment: f64, overhead: f64, pos: &Position) -> Self {
        let base_time_scale = BASE_TIME_SCALE.val() as f64 / 1024.0;
        let inc_scale = INC_SCALE.val() as f64 / 1024.0;

        let stone_complexity_scale = STONE_COMPLEXITY_SCALE.val() as f64 / 1024.0;
        let stone_complexity_base = STONE_COMPLEXITY_BASE.val() as f64 / 1024.0;

        let opt_time_scale = OPT_TIME_SCALE.val() as f64 / 1024.0;

        let remaining = (remaining - overhead).max(0.001);

        let stones_in_hand = pos.stones_in_hand(Color::Black) + pos.stones_in_hand(Color::White);
        let stones_on_board = MAX_STONES_IN_HAND.saturating_sub(stones_in_hand);
        let complexity = stone_complexity_scale * stones_on_board as f64 / MAX_STONES_IN_HAND as f64;
        let base_time = remaining * (stone_complexity_base + complexity) * base_time_scale / 409600.0
            + increment * inc_scale / 1024.0;

        let max_time = remaining * 0.8;
        let opt_time = (base_time * opt_time_scale).min(max_time);

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
        let bm_stability_min = BM_STABILITY_MIN.val() as f64 / 1024.0;
        let bm_stability_max = BM_STABILITY_MAX.val() as f64 / 1024.0;
        let bm_stability_scale = BM_STABILITY_SCALE.val() as f64 / 1024.0;
        let bm_stability_offset = BM_STABILITY_OFFSET.val() as f64 / 1024.0;
        let bm_stability_power = BM_STABILITY_POWER.val() as f64 / 1024.0;

        let score_drop_divisor = SCORE_DROP_DIVISOR.val() as f64 / 1024.0;
        let score_drop_max = SCORE_DROP_MAX.val() as f64 / 1024.0;

        let node_tm_base = NODE_TM_BASE.val() as f64 / 1024.0;
        let node_tm_scale = NODE_TM_SCALE.val() as f64 / 1024.0;
        let node_tm_min = NODE_TM_MIN.val() as f64 / 1024.0;

        let mut scale = 1.0;

        let score = pv_move.score;
        let avg_score = match ctx.avg_score {
            Some(avg_score) => {
                ctx.avg_score = Some(avg_score.midpoint(score));
                avg_score
            }
            None => {
                ctx.avg_score = Some(score);
                score
            }
        };

        let mv = pv_move.mv();
        if ctx.last_best_move == Some(mv) {
            ctx.stability += 1;
        } else {
            ctx.stability = 1;
            ctx.last_best_move = Some(mv);
        }

        if depth >= 5 {
            let stability = ctx.stability as f64;
            scale *= (bm_stability_min
                + bm_stability_scale * (stability + bm_stability_offset).powf(bm_stability_power))
            .min(bm_stability_max);

            let score_change = score - avg_score;
            if score_change < 0 {
                scale *= (1.0 - score_change as f64 / score_drop_divisor).min(score_drop_max);
            }

            let best_move_nodes_fraction = pv_move.nodes as f64 / total_nodes as f64;
            scale *= (node_tm_base - node_tm_scale * best_move_nodes_fraction).max(node_tm_min);
        }

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

    pub fn set_time_manager(&mut self, remaining: f64, increment: f64, overhead: f64, pos: &Position) -> bool {
        match self.time_manager {
            None => {
                self.time_manager = Some(TimeManager::new(remaining, increment, overhead, pos));
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
