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

use crate::board::{FlatCountOutcome, Position};
use crate::eval::static_eval;
use crate::limit::Limits;
use crate::movegen::generate_moves;
use crate::movepick::Movepicker;
use crate::takmove::Move;
use crate::thread::{PvList, RootMove, ThreadData, update_pv};
use crate::ttable::{DEFAULT_TT_SIZE_MIB, TranspositionTable, TtFlag};
use std::time::Instant;

pub type Score = i32;

pub const SCORE_INF: Score = 32767;
pub const SCORE_MATE: Score = SCORE_INF - 1;
pub const SCORE_WIN: Score = 25000;
pub const SCORE_MAX_MATE: Score = SCORE_MATE - MAX_PLY as Score;

pub const MAX_PLY: i32 = 255;

#[derive(Debug)]
struct SearchContext {
    limits: Limits,
    stopped: bool,
}

impl SearchContext {
    fn new(limits: Limits) -> Self {
        Self {
            limits,
            stopped: false,
        }
    }

    fn check_stop_soft(&mut self, nodes: usize) -> bool {
        if self.limits.should_stop_soft(nodes) {
            self.stopped = true;
            return true;
        }

        false
    }

    fn check_stop_hard(&mut self, nodes: usize) -> bool {
        if self.limits.should_stop_hard(nodes) {
            self.stopped = true;
            return true;
        }

        false
    }

    #[must_use]
    fn has_stopped(&self) -> bool {
        self.stopped
    }
}

const LMR_TABLE_MOVES: usize = 64;

#[static_init::dynamic]
static LMR_REDUCTIONS: [[i32; LMR_TABLE_MOVES]; MAX_PLY as usize] = {
    const BASE: f64 = 0.5;
    const DIVISOR: f64 = 2.5;

    let mut reductions = [[0; LMR_TABLE_MOVES]; MAX_PLY as usize];

    for depth in 1..MAX_PLY as usize {
        let ln_depth = (depth as f64).ln();
        for move_number in 1..LMR_TABLE_MOVES {
            let ln_move_number = (move_number as f64).ln();
            let reduction = (BASE + ln_depth * ln_move_number / DIVISOR) as i32;
            reductions[depth][move_number] = reduction;
        }
    }

    reductions
};

trait NodeType {
    const PV_NODE: bool;
    const ROOT_NODE: bool;
}

struct NonPvNode;
impl NodeType for NonPvNode {
    const PV_NODE: bool = false;
    const ROOT_NODE: bool = false;
}

struct PvNode;
impl NodeType for PvNode {
    const PV_NODE: bool = true;
    const ROOT_NODE: bool = false;
}

struct RootNode;
impl NodeType for RootNode {
    const PV_NODE: bool = true;
    const ROOT_NODE: bool = true;
}

struct SearcherImpl {
    tt: TranspositionTable,
}

impl SearcherImpl {
    fn new() -> Self {
        Self {
            tt: TranspositionTable::new(DEFAULT_TT_SIZE_MIB),
        }
    }

    fn reset(&mut self) {
        self.tt.clear();
    }

    fn set_tt_size(&mut self, size_mib: usize) {
        self.tt.resize(size_mib);
    }

    fn run_search(
        &mut self,
        ctx: &mut SearchContext,
        thread: &mut ThreadData,
        root_pos: &Position,
        start_time: Instant,
    ) {
        {
            let mut root_moves = Vec::with_capacity(256);
            generate_moves(&mut root_moves, root_pos);

            thread.root_moves.clear();
            thread.root_moves.reserve(root_moves.len());

            for mv in root_moves {
                let mut root_move = RootMove::default();
                root_move.pv.push(mv);
                thread.root_moves.push(root_move);
            }
        }

        thread.nodes = 0;
        thread.root_depth = 1;

        let mut movelists = vec![Vec::with_capacity(256); MAX_PLY as usize];
        let mut pvs = vec![PvList::new(); MAX_PLY as usize];

        loop {
            thread.reset_seldepth();

            self.search::<RootNode>(
                ctx,
                thread,
                &mut movelists,
                &mut pvs,
                root_pos,
                thread.root_depth,
                0,
                -SCORE_INF,
                SCORE_INF,
                false,
            );

            thread.root_moves.sort_by(|a, b| b.score.cmp(&a.score));

            if thread.root_depth >= thread.max_depth {
                break;
            }

            if thread.is_main_thread() {
                if ctx.check_stop_soft(thread.nodes) {
                    break;
                }

                let time = start_time.elapsed().as_secs_f64();
                self.report(thread, thread.root_depth, time);
            }

            thread.root_depth += 1;
        }

        if thread.is_main_thread() {
            let time = start_time.elapsed().as_secs_f64();
            self.final_report(thread, thread.root_depth, time);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn search<NT: NodeType>(
        &mut self,
        ctx: &mut SearchContext,
        thread: &mut ThreadData,
        movelists: &mut [Vec<Move>],
        pvs: &mut [PvList],
        pos: &Position,
        depth: i32,
        ply: i32,
        mut alpha: Score,
        beta: Score,
        expected_cutnode: bool,
    ) -> Score {
        if ctx.has_stopped() {
            return 0;
        }

        if !NT::ROOT_NODE
            && thread.is_main_thread()
            && thread.root_depth > 1
            && ctx.check_stop_hard(thread.nodes)
        {
            return 0;
        }

        thread.inc_nodes();

        if depth <= 0 {
            let static_eval = static_eval(pos);
            let correction = thread.corrhist.correction(pos);
            return static_eval + correction;
        }

        if NT::PV_NODE {
            thread.update_seldepth(ply);
        }

        let (_tt_hit, tt_entry) = self.tt.probe(pos.key(), ply);

        if !NT::PV_NODE
            && tt_entry.depth >= depth
            && match tt_entry.flag {
                None => unreachable!(),
                Some(TtFlag::UpperBound) => tt_entry.score <= alpha,
                Some(TtFlag::LowerBound) => tt_entry.score >= beta,
                Some(TtFlag::Exact) => true,
            }
        {
            return tt_entry.score;
        }

        let raw_eval = static_eval(pos);
        let correction = thread.corrhist.correction(pos);
        let static_eval = raw_eval + correction;

        if !NT::PV_NODE {
            // reverse futility pruning (rfp)
            let rfp_margin = 100 * depth + 100;
            if depth <= 6 && static_eval - rfp_margin >= beta {
                return static_eval;
            }

            // nullmove pruning (nmp)
            if expected_cutnode
                && depth >= 4
                && static_eval >= beta
                && thread.stack[ply as usize - 1].mv.is_some()
            {
                let r = 3 + depth / 4;

                let new_pos = thread.apply_nullmove(ply, pos);

                let score = -self.search::<NonPvNode>(
                    ctx,
                    thread,
                    movelists,
                    pvs,
                    &new_pos,
                    (depth - r).max(1), // dont allow dropping straight to eval
                    ply + 1,
                    -beta,
                    -beta + 1,
                    false,
                );

                thread.pop_move();

                if score >= beta {
                    return if score > SCORE_WIN { beta } else { score };
                }
            }
        }

        let (moves, movelists) = movelists.split_first_mut().unwrap();
        let (pv, child_pvs) = pvs.split_first_mut().unwrap();

        let mut best_score = -SCORE_INF;
        let mut best_move = None;

        let mut tt_flag = TtFlag::UpperBound;

        let mut scores = Vec::new();
        let prev_move = if ply > 0 {
            thread.stack[(ply - 1) as usize].mv
        } else {
            None
        };
        let mut movepicker = Movepicker::new(
            pos,
            moves,
            &mut scores,
            tt_entry.mv,
            thread.killers[ply as usize],
            prev_move,
        );
        let mut move_count = 0;
        let mut faillow_moves = arrayvec::ArrayVec::<Move, 32>::new();

        while let Some(mv) = movepicker.next(&thread.history) {
            debug_assert!(pos.is_legal(mv));

            if !NT::ROOT_NODE && best_score > -SCORE_WIN {
                if depth <= 6 && move_count as i32 >= 5 + 2 * depth * depth {
                    break;
                }
            }

            move_count += 1;

            if NT::PV_NODE {
                child_pvs[0].clear();
            }

            let new_pos = thread.apply_move(ply, pos, mv);

            let score = 'recurse: {
                if new_pos.has_road(pos.stm()) {
                    break 'recurse SCORE_MATE - ply - 1;
                }

                if mv.is_spread() && new_pos.has_road(pos.stm().flip()) {
                    break 'recurse -SCORE_MATE + ply + 1;
                }

                if !mv.is_spread() {
                    match new_pos.count_flats() {
                        FlatCountOutcome::None => {}
                        FlatCountOutcome::Draw => break 'recurse 0,
                        FlatCountOutcome::Win(player) => {
                            break 'recurse if player == pos.stm() {
                                SCORE_MATE - ply - 1
                            } else {
                                -SCORE_MATE + ply + 1
                            };
                        }
                    }
                }

                if mv.is_spread() && thread.is_drawn_by_repetition(new_pos.key(), ply) {
                    break 'recurse 0;
                }

                let mut score = 0;

                let new_depth = depth - 1;

                if depth >= 2 && move_count >= 5 + 2 * usize::from(NT::ROOT_NODE) {
                    let mut r =
                        LMR_REDUCTIONS[depth as usize - 1][move_count.min(LMR_TABLE_MOVES) - 1];
                    if mv.is_spread() {
                        let gain = new_pos.fcd(pos.stm()) - pos.fcd(pos.stm());
                        r += (1 - gain).clamp(0, 3);
                    }
                    let reduced = (new_depth - r).max(1).min(new_depth - 1);

                    score = -self.search::<NonPvNode>(
                        ctx,
                        thread,
                        movelists,
                        child_pvs,
                        &new_pos,
                        reduced,
                        ply + 1,
                        -alpha - 1,
                        -alpha,
                        true,
                    );

                    if score > alpha && reduced < new_depth {
                        score = -self.search::<NonPvNode>(
                            ctx,
                            thread,
                            movelists,
                            child_pvs,
                            &new_pos,
                            new_depth,
                            ply + 1,
                            -alpha - 1,
                            -alpha,
                            !expected_cutnode,
                        );
                    }
                } else if !NT::PV_NODE || move_count > 1 {
                    score = -self.search::<NonPvNode>(
                        ctx,
                        thread,
                        movelists,
                        child_pvs,
                        &new_pos,
                        new_depth,
                        ply + 1,
                        -alpha - 1,
                        -alpha,
                        !expected_cutnode,
                    );
                }

                if NT::PV_NODE && (move_count == 1 || score > alpha) {
                    score = -self.search::<PvNode>(
                        ctx,
                        thread,
                        movelists,
                        child_pvs,
                        &new_pos,
                        new_depth,
                        ply + 1,
                        -beta,
                        -alpha,
                        false,
                    );
                }

                score
            };

            thread.pop_move();

            if ctx.has_stopped() {
                return 0;
            }

            if NT::ROOT_NODE {
                let seldepth = thread.seldepth;
                let root_move = thread.get_root_move_mut(mv);

                if move_count == 1 || score > alpha {
                    root_move.seldepth = seldepth;
                    root_move.score = score;

                    update_pv(&mut root_move.pv, mv, &child_pvs[0]);
                } else {
                    root_move.score = -SCORE_INF;
                }
            }

            if score > best_score {
                best_score = score;
            }

            if score > alpha {
                alpha = score;
                best_move = Some(mv);

                if NT::PV_NODE {
                    update_pv(pv, mv, &child_pvs[0]);
                }

                tt_flag = TtFlag::Exact;
            }

            if score >= beta {
                tt_flag = TtFlag::LowerBound;
                break;
            }

            if best_move != Some(mv) {
                faillow_moves.try_push(mv).ok();
            }
        }

        debug_assert!(move_count > 0);

        if let Some(best_move) = best_move {
            let bonus = (300 * depth - 300).clamp(0, 2500);

            thread.history.update(pos, best_move, prev_move, bonus);

            for &mv in faillow_moves.iter() {
                thread.history.update(pos, mv, prev_move, -bonus);
            }

            if best_score >= beta {
                thread.killers[ply as usize].push(best_move);
            }
        }

        if tt_flag == TtFlag::Exact
            || (tt_flag == TtFlag::UpperBound && best_score < static_eval)
            || (tt_flag == TtFlag::LowerBound && best_score > static_eval)
        {
            thread.corrhist.update(pos, depth, best_score, static_eval);
        }

        self.tt
            .store(pos.key(), best_score, best_move, depth, ply, tt_flag);

        best_score
    }

    fn report(&self, thread: &ThreadData, depth: i32, time: f64) {
        let root_move = thread.pv_move();

        let score = root_move.score;
        assert_ne!(root_move.score, -SCORE_INF);

        let ms = (time * 1000.0) as usize;
        let nps = ((thread.nodes as f64) / time) as usize;

        print!(
            "info depth {} seldepth {} time {} nodes {} nps {} score ",
            depth, root_move.seldepth, ms, thread.nodes, nps
        );

        if score.abs() >= SCORE_MAX_MATE {
            print!(
                "mate {}",
                if score > 0 {
                    (SCORE_MATE - score + 1) / 2
                } else {
                    -(SCORE_MATE + score) / 2
                }
            );
        } else {
            print!("cp {}", score);
        }

        let hashfull = self.tt.estimate_full_permille();
        print!(" hashfull {}", hashfull);

        print!(" pv");

        for mv in root_move.pv.iter() {
            print!(" {}", mv);
        }

        println!();
    }

    fn final_report(&self, thread: &ThreadData, depth: i32, time: f64) {
        self.report(thread, depth, time);

        let mv = thread.pv_move().pv[0];
        println!("bestmove {}", mv);
    }
}

pub struct Searcher {
    searcher: SearcherImpl,
    data: ThreadData,
}

impl Searcher {
    pub fn new() -> Self {
        Self {
            searcher: SearcherImpl::new(),
            data: ThreadData::new(0),
        }
    }

    pub fn start_search(
        &mut self,
        pos: &Position,
        key_history: &[u64],
        start_time: Instant,
        limits: Limits,
        max_depth: i32,
    ) {
        let thread = &mut self.data;

        thread.reset(key_history);
        thread.max_depth = max_depth;

        let mut ctx = SearchContext::new(limits);
        self.searcher.run_search(&mut ctx, thread, pos, start_time);
    }

    pub fn reset(&mut self) {
        self.searcher.reset();
        self.data.corrhist.clear();
        self.data.history.clear();
        self.data.killers.fill(Default::default());
    }

    pub fn set_tt_size(&mut self, size_mib: usize) {
        self.searcher.set_tt_size(size_mib);
    }
}
