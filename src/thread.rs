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

use crate::limit::Limits;
use crate::ttable::{DEFAULT_TT_SIZE_MIB, TranspositionTable};
use crate::util::counter::Counter;
use crate::{
    board::Position,
    correction::CorrectionHistory,
    history::History,
    movepick::KillerTable,
    search::{MAX_PLY, SCORE_INF, Score},
    takmove::Move,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

pub struct SearcherCount {
    count: AtomicU32,
}

impl SearcherCount {
    fn new() -> Self {
        Self {
            count: AtomicU32::new(0),
        }
    }

    fn start(&self) {
        self.count.store(1, Ordering::Relaxed);
    }

    pub fn register_thread(&self) {
        self.count.fetch_add(1, Ordering::Release);
    }

    pub fn unregister_thread(&self) {
        let remaining = self.count.fetch_sub(1, Ordering::AcqRel);
        if remaining == 2 {
            atomic_wait::wake_all(&self.count);
        }
    }

    pub fn unregister_and_wait(&self) {
        let remaining = self.count.fetch_sub(1, Ordering::AcqRel);
        if remaining > 2 {
            self.wait_for(1);
        }
    }

    pub fn complete_search(&self) {
        let count = self.count.fetch_sub(1, Ordering::AcqRel);
        assert_eq!(count, 1);
        atomic_wait::wake_all(&self.count);
    }

    pub fn wait(&self) {
        self.wait_for(0);
    }

    fn wait_for(&self, target: u32) {
        let mut count = self.count.load(Ordering::Acquire);
        while count > target {
            atomic_wait::wait(&self.count, count);
            count = self.count.load(Ordering::Acquire);
        }
    }
}

pub struct SharedContext {
    pub tt: TranspositionTable,
    start_time: Instant,
    limits: Limits,
    stopped: AtomicBool,
    counter: Arc<SearcherCount>,
    nodes: Counter,
}

impl SharedContext {
    pub fn new() -> Self {
        let time = Instant::now();
        Self {
            tt: TranspositionTable::new(DEFAULT_TT_SIZE_MIB),
            start_time: time,
            limits: Limits::new(time),
            stopped: AtomicBool::new(false),
            counter: Arc::new(SearcherCount::new()),
            nodes: Counter::new(1),
        }
    }

    pub fn set_threads(&mut self, threads: u32) {
        self.nodes.resize(threads as usize);
    }

    pub fn init_search(&mut self, start_time: Instant, limits: Limits) {
        self.start_time = start_time;
        self.limits = limits;
        self.stopped.store(false, Ordering::Relaxed);
        self.counter.start();
        self.nodes.reset();
    }

    pub fn get_counter(&self) -> Arc<SearcherCount> {
        self.counter.clone()
    }

    pub fn is_searching(&self) -> bool {
        self.counter.count.load(Ordering::Relaxed) > 0
    }

    pub fn check_stop_soft(&self, nodes: usize, best_move_nodes_fraction: f64) -> bool {
        if self
            .limits
            .should_stop_soft(nodes, best_move_nodes_fraction)
        {
            self.stopped.store(true, Ordering::Relaxed);
            return true;
        }

        false
    }

    pub fn check_stop_hard(&self, nodes: usize) -> bool {
        if self.limits.should_stop_hard(nodes) {
            self.stopped.store(true, Ordering::Relaxed);
            return true;
        }

        false
    }

    #[must_use]
    pub fn total_nodes(&self) -> usize {
        self.nodes.total()
    }

    #[must_use]
    pub fn elapsed(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }

    #[must_use]
    pub fn has_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }
}

pub type PvList = arrayvec::ArrayVec<Move, { MAX_PLY as usize }>;

pub fn update_pv(pv: &mut PvList, mv: Move, child: &PvList) {
    pv.clear();
    pv.push(mv);
    pv.try_extend_from_slice(child).unwrap();
}

#[derive(Clone, Debug)]
pub struct RootMove {
    pub score: Score,
    pub window_score: Score,
    pub display_score: Score,
    pub previous_score: Score,
    pub upper_bound: bool,
    pub lower_bound: bool,
    pub seldepth: i32,
    pub pv: PvList,
    pub nodes: usize,
}

impl RootMove {
    #[must_use]
    pub fn mv(&self) -> Move {
        self.pv[0]
    }
}

impl Default for RootMove {
    fn default() -> Self {
        Self {
            score: -SCORE_INF,
            window_score: -SCORE_INF,
            display_score: -SCORE_INF,
            previous_score: -SCORE_INF,
            upper_bound: false,
            lower_bound: false,
            seldepth: 0,
            pv: PvList::new(),
            nodes: 0,
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub struct StackEntry {
    pub mv: Option<Move>,
}

pub struct ThreadData {
    pub id: u32,
    pub key_history: Vec<u64>,
    pub root_depth: i32,
    pub seldepth: i32,
    pub pv_idx: usize,
    pub root_moves: Vec<RootMove>,
    pub stack: Vec<StackEntry>,
    pub corrhist: Box<CorrectionHistory>,
    pub history: Box<History>,
    pub killers: [KillerTable; MAX_PLY as usize],
    pub shared: Option<Arc<SharedContext>>,
}

impl ThreadData {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            key_history: Vec::with_capacity(1024),
            root_depth: 0,
            seldepth: 0,
            pv_idx: 0,
            root_moves: Vec::with_capacity(1024),
            stack: vec![StackEntry::default(); MAX_PLY as usize + 1],
            corrhist: CorrectionHistory::boxed(),
            history: History::boxed(),
            killers: [Default::default(); MAX_PLY as usize],
            shared: None,
        }
    }

    pub fn is_main_thread(&self) -> bool {
        self.id == 0
    }

    pub fn shared(&self) -> &SharedContext {
        self.shared.as_deref().unwrap()
    }

    pub fn inc_nodes(&mut self) {
        self.shared().nodes.increment(self.id as usize);
    }

    pub fn nodes(&self) -> usize {
        self.shared().nodes.get(self.id as usize)
    }

    pub fn reset_seldepth(&mut self) {
        self.seldepth = 0;
    }

    pub fn update_seldepth(&mut self, ply: i32) {
        self.seldepth = self.seldepth.max(ply + 1);
    }

    #[must_use]
    pub fn is_legal_root_move(&self, mv: Move) -> bool {
        self.root_moves[self.pv_idx..]
            .iter()
            .any(|root_move| root_move.pv[0] == mv)
    }

    pub fn sort_root_moves(&mut self) {
        self.root_moves.sort_by(|a, b| b.score.cmp(&a.score));
    }

    pub fn sort_remaining_root_moves(&mut self) {
        self.root_moves[self.pv_idx..].sort_by(|a, b| b.score.cmp(&a.score));
    }

    pub fn apply_move(&mut self, ply: i32, pos: &Position, mv: Move) -> Position {
        self.key_history.push(pos.key());
        self.stack[ply as usize].mv = Some(mv);
        pos.apply_move(mv)
    }

    pub fn apply_nullmove(&mut self, ply: i32, pos: &Position) -> Position {
        self.key_history.push(pos.key());
        self.stack[ply as usize].mv = None;
        pos.apply_nullmove()
    }

    pub fn pop_move(&mut self) {
        self.key_history.pop();
    }

    pub fn is_drawn_by_repetition(&self, curr: u64, ply: i32) -> bool {
        let mut ply = ply - 1;
        let mut repetitions = 0;

        //TODO skip properly
        for &key in self.key_history.iter().rev() {
            if key == curr {
                repetitions += 1;

                let required = 1 + if ply < 0 { 1 } else { 0 };
                if repetitions == required {
                    return true;
                }

                ply -= 1;
            }
        }

        false
    }

    #[must_use]
    pub fn get_root_move(&self, mv: Move) -> &RootMove {
        for root_move in self.root_moves[self.pv_idx..].iter() {
            if root_move.pv[0] == mv {
                return root_move;
            }
        }

        unreachable!();
    }

    #[must_use]
    pub fn get_root_move_mut(&mut self, mv: Move) -> &mut RootMove {
        for root_move in self.root_moves[self.pv_idx..].iter_mut() {
            if root_move.pv[0] == mv {
                return root_move;
            }
        }

        unreachable!();
    }

    #[must_use]
    pub fn pv_move(&self) -> &RootMove {
        &self.root_moves[0]
    }
}
