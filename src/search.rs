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
use crate::command_channel::{Receiver, Sender, channel};
use crate::core::PieceType;
use crate::eval::static_eval;
use crate::limit::Limits;
use crate::movegen::generate_moves;
use crate::movepick::Movepicker;
use crate::takmove::Move;
use crate::tei::TeiOptions;
use crate::thread::{PvList, RootMove, SharedContext, ThreadData, update_pv};
use crate::ttable::TtFlag;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

pub type Score = i32;

pub const SCORE_INF: Score = 32767;
pub const SCORE_MATE: Score = SCORE_INF - 1;
pub const SCORE_WIN: Score = 25000;
pub const SCORE_MAX_MATE: Score = SCORE_MATE - MAX_PLY as Score;

pub const MAX_PLY: i32 = 255;

#[derive(Clone, Debug)]
pub struct SearchContext {
    max_depth: i32,
    multipv: usize,
    root_pos: Position,
    root_moves: Arc<Vec<RootMove>>,
    key_history: Arc<Vec<u64>>,
}

impl SearchContext {
    fn new(
        max_depth: i32,
        multipv: usize,
        root_pos: Position,
        root_moves: Arc<Vec<RootMove>>,
        key_history: Arc<Vec<u64>>,
    ) -> Self {
        Self {
            max_depth,
            multipv,
            root_pos,
            root_moves,
            key_history,
        }
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
            let reduction = ((BASE + ln_depth * ln_move_number / DIVISOR) * 1024.0) as i32;
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

#[allow(clippy::too_many_arguments)]
fn search<NT: NodeType>(
    thread: &mut ThreadData,
    movelists: &mut [Vec<Move>],
    pvs: &mut [PvList],
    pos: &Position,
    depth: i32,
    ply: i32,
    mut alpha: Score,
    mut beta: Score,
    expected_cutnode: bool,
) -> Score {
    if thread.shared().has_stopped() {
        return 0;
    }

    if !NT::ROOT_NODE
        && thread.is_main_thread()
        && thread.root_depth > 1
        && thread.shared().check_stop_hard(thread.nodes)
    {
        return 0;
    }

    if !NT::ROOT_NODE {
        alpha = alpha.max(-SCORE_MATE + ply);
        beta = beta.min(SCORE_MATE - ply);
        if alpha >= beta {
            return alpha;
        }
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

    let (_tt_hit, tt_entry) = thread.shared().tt.probe(pos.key(), ply);

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

    let tt_move = if NT::ROOT_NODE && thread.root_depth > 1 {
        Some(thread.root_moves[thread.pv_idx].mv())
    } else {
        tt_entry.mv
    };

    let raw_eval = static_eval(pos);
    let correction = thread.corrhist.correction(pos);
    let static_eval = raw_eval + correction;

    if !NT::PV_NODE {
        // reverse futility pruning (rfp)
        let rfp_margin = 100 * depth + 100 - (expected_cutnode as i32 * 50);
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

            let score = -search::<NonPvNode>(
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
        tt_move,
        thread.killers[ply as usize],
        prev_move,
    );
    let mut move_count = 0;
    let mut faillow_moves = arrayvec::ArrayVec::<Move, 32>::new();

    while let Some(mv) = movepicker.next(&thread.history) {
        debug_assert!(pos.is_legal(mv));

        if NT::ROOT_NODE && !thread.is_legal_root_move(mv) {
            continue;
        }

        if !NT::ROOT_NODE && best_score > -SCORE_WIN {
            if depth <= 6 && move_count as i32 >= 5 + 2 * depth * depth {
                break;
            }
        }

        let mut extension = 0;

        move_count += 1;

        if NT::PV_NODE {
            child_pvs[0].clear();
        }

        let new_pos = thread.apply_move(ply, pos, mv);
        thread.shared().tt.prefetch(new_pos.key());

        let is_crush =
            mv.is_spread() && pos.stacks().top(mv.spread_dest()) == Some(PieceType::Wall);

        if is_crush {
            extension += 1;
        }

        let nodes_before = thread.nodes;

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

            let new_depth = depth + extension - 1;

            if depth >= 2 && move_count >= 5 + 2 * usize::from(NT::ROOT_NODE) {
                let mut r = LMR_REDUCTIONS[depth as usize - 1][move_count.min(LMR_TABLE_MOVES) - 1];
                if mv.is_spread() {
                    let gain = new_pos.fcd(pos.stm()) - pos.fcd(pos.stm());
                    r += (1 - gain).clamp(0, 3) * 1024;
                }

                r -= thread.history.score(pos, mv, prev_move) / 8;

                r /= 1024;

                let reduced = (new_depth - r).max(1).min(new_depth - 1);

                score = -search::<NonPvNode>(
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
                    score = -search::<NonPvNode>(
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
                score = -search::<NonPvNode>(
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
                score = -search::<PvNode>(
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

        let nodes_after = thread.nodes;

        thread.pop_move();

        if thread.shared().has_stopped() {
            return 0;
        }

        if NT::ROOT_NODE {
            let seldepth = thread.seldepth;
            let root_move = thread.get_root_move_mut(mv);

            root_move.nodes += nodes_after - nodes_before;
            root_move.window_score = score;

            if move_count == 1 || score > alpha {
                root_move.seldepth = seldepth;

                root_move.display_score = score;
                root_move.score = score;

                root_move.upper_bound = false;
                root_move.lower_bound = false;

                if score <= alpha {
                    root_move.display_score = alpha;
                    root_move.upper_bound = true;
                } else if score >= beta {
                    root_move.display_score = beta;
                    root_move.lower_bound = true;
                }

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

    if !NT::ROOT_NODE || thread.pv_idx == 0 {
        thread
            .shared()
            .tt
            .store(pos.key(), best_score, best_move, depth, ply, tt_flag);
    }

    best_score
}

fn run_search(ctx: &SearchContext, thread: &mut ThreadData) {
    thread.root_moves.clear();
    thread.root_moves.reserve(ctx.root_moves.len());

    for mv in ctx.root_moves.iter() {
        thread.root_moves.push(mv.clone());
    }

    thread.key_history.clear();
    thread.key_history.reserve(ctx.key_history.len());
    thread.key_history.extend_from_slice(&ctx.key_history);

    thread.shared().register_thread();

    thread.nodes = 0;
    thread.root_depth = 1;

    let mut movelists = vec![Vec::with_capacity(256); MAX_PLY as usize];
    let mut pvs = vec![PvList::new(); MAX_PLY as usize];

    loop {
        for root_move in thread.root_moves.iter_mut() {
            root_move.previous_score = root_move.score;
        }

        thread.pv_idx = 0;
        while !thread.shared().has_stopped() && thread.pv_idx < ctx.multipv {
            thread.reset_seldepth();

            let mut delta = 25;

            let mut alpha = -SCORE_INF;
            let mut beta = SCORE_INF;

            if thread.root_depth >= 3 {
                let last_score = thread.root_moves[thread.pv_idx].window_score;
                alpha = (last_score - delta).max(-SCORE_INF);
                beta = (last_score + delta).min(SCORE_INF);
            }

            while !thread.shared().has_stopped() {
                let score = search::<RootNode>(
                    thread,
                    &mut movelists,
                    &mut pvs,
                    &ctx.root_pos,
                    thread.root_depth,
                    0,
                    alpha,
                    beta,
                    false,
                );

                thread.sort_remaining_root_moves();

                if (score > alpha && score < beta) || thread.shared().has_stopped() {
                    break;
                }

                if thread.is_main_thread() && ctx.multipv == 1 {
                    let time = thread.shared().elapsed();
                    if time >= WIDEN_REPORT_DELAY {
                        report_single(thread, thread.root_depth, time, ctx.multipv, thread.pv_idx);
                    }
                }

                delta = (delta * 8).min(SCORE_INF);
                if score <= alpha {
                    alpha = (alpha - delta).max(-SCORE_INF);
                } else {
                    beta = (beta + delta).min(SCORE_INF);
                }
            }

            thread.sort_root_moves();

            thread.pv_idx += 1;
        }

        if thread.shared().has_stopped() {
            break;
        }

        if thread.root_depth >= ctx.max_depth {
            break;
        }

        if thread.is_main_thread() {
            if thread.shared().check_stop_soft(
                thread.nodes,
                thread.pv_move().nodes as f64 / (thread.nodes as f64),
            ) {
                break;
            }

            let time = thread.shared().elapsed();
            report(thread, thread.root_depth, time, ctx.multipv);
        }

        thread.root_depth += 1;
    }

    if thread.is_main_thread() {
        thread.shared().unregister_and_wait();

        let time = thread.shared().elapsed();
        final_report(thread, thread.root_depth, time, ctx.multipv);

        thread.shared().complete_search();
    } else {
        thread.shared().unregister_thread();
    }
}

fn report_single(thread: &ThreadData, depth: i32, time: f64, multipv: usize, pv_idx: usize) {
    let root_move = &thread.root_moves[pv_idx];

    let (depth, score) = if root_move.score == -SCORE_INF {
        ((depth - 1).max(1), root_move.previous_score)
    } else {
        (depth, root_move.display_score)
    };

    assert_ne!(depth, 0);
    assert_ne!(score, -SCORE_INF);

    let ms = (time * 1000.0) as usize;
    let nps = ((thread.nodes as f64) / time) as usize;

    print!("info ");

    if multipv > 1 {
        print!("multipv {} ", pv_idx + 1);
    }

    print!(
        "depth {} seldepth {} time {} nodes {} nps {} score ",
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

    if root_move.upper_bound {
        assert!(!root_move.lower_bound);
        print!(" upperbound");
    }

    if root_move.lower_bound {
        assert!(!root_move.upper_bound);
        print!(" lowerbound");
    }

    let hashfull = thread.shared().tt.estimate_full_permille();
    print!(" hashfull {}", hashfull);

    print!(" pv");

    for mv in root_move.pv.iter() {
        print!(" {}", mv);
    }

    println!();
}

fn report(thread: &ThreadData, depth: i32, time: f64, multipv: usize) {
    for pv_idx in 0..multipv {
        report_single(thread, depth, time, multipv, pv_idx);
    }
}

fn final_report(thread: &ThreadData, depth: i32, time: f64, multipv: usize) {
    report(thread, depth, time, multipv);

    let mv = thread.pv_move().mv();
    println!("bestmove {}", mv);
}

const WIDEN_REPORT_DELAY: f64 = 1.0;

#[derive(Clone)]
enum ThreadCommand {
    Ping,
    DropSharedCtx,
    SetSharedCtx(Arc<SharedContext>),
    StartSearch(SearchContext),
    Clear,
    Quit,
}

pub struct Searcher {
    shared_ctx: Arc<SharedContext>,
    threads: Vec<JoinHandle<()>>,
    sender: Sender<ThreadCommand>,
    root_moves: Arc<Vec<RootMove>>,
    key_history: Arc<Vec<u64>>,
}

impl Searcher {
    pub fn new() -> Self {
        let shared_ctx = Arc::new(SharedContext::new());

        let (mut sender, mut receiver) = channel(1);

        let thread = thread::spawn({
            let shared_ctx = shared_ctx.clone();
            let receiver = receiver.next().unwrap();
            move || {
                if std::panic::catch_unwind(move || {
                    Self::run_thread(0, shared_ctx, receiver);
                })
                    .is_err()
                {
                    std::process::exit(-1);
                }
            }
        });

        sender.send(ThreadCommand::Ping);

        Self {
            shared_ctx,
            threads: vec![thread],
            sender,
            root_moves: Arc::new(Vec::with_capacity(1024)),
            key_history: Arc::new(Vec::with_capacity(1024)),
        }
    }

    fn run_thread(id: u32, shared_ctx: Arc<SharedContext>, mut receiver: Receiver<ThreadCommand>) {
        let mut data = ThreadData::new(id, shared_ctx);

        loop {
            match receiver.recv(|cmd| cmd.clone()) {
                ThreadCommand::Ping => {}
                ThreadCommand::DropSharedCtx => {
                    assert!(data.shared.is_some());
                    data.shared = None;
                }
                ThreadCommand::SetSharedCtx(shared) => {
                    assert!(data.shared.is_none());
                    data.shared = Some(shared);
                }
                ThreadCommand::StartSearch(ctx) => run_search(&ctx, &mut data),
                ThreadCommand::Clear => {
                    data.corrhist.clear();
                    data.history.clear();
                    data.killers.fill(Default::default());
                }
                ThreadCommand::Quit => return,
            }
        }
    }

    pub fn start_search(
        &mut self,
        pos: &Position,
        new_key_history: &[u64],
        start_time: Instant,
        limits: Limits,
        max_depth: i32,
        options: &TeiOptions,
    ) {
        self.modify_shared_ctx(|ctx| {
            ctx.init_search(start_time, limits);
        });

        self.init_root_moves(pos);

        {
            let key_history = Arc::get_mut(&mut self.key_history).unwrap();

            key_history.clear();
            key_history.reserve(new_key_history.len() + MAX_PLY as usize);

            key_history.extend_from_slice(new_key_history);
        }

        let multipv = options.multipv.min(self.root_moves.len());

        let ctx = SearchContext::new(
            max_depth,
            multipv,
            *pos,
            self.root_moves.clone(),
            self.key_history.clone(),
        );

        self.sender.send(ThreadCommand::StartSearch(ctx));
    }

    pub fn stop(&mut self) {
        self.shared_ctx.stop();
    }

    pub fn is_searching(&self) -> bool {
        self.shared_ctx.is_searching()
    }

    pub fn kill_threads(&mut self) {
        self.stop();
        if !self.threads.is_empty() {
            self.sender.send(ThreadCommand::Quit);
            self.threads.drain(..).for_each(|thread| thread.join().unwrap());
        }
    }

    fn modify_shared_ctx<F>(&mut self, func: F)
    where
        F: Fn(&mut SharedContext),
    {
        self.sender.send(ThreadCommand::DropSharedCtx);
        {
            let ctx = Arc::get_mut(&mut self.shared_ctx).unwrap();
            func(ctx);
        }
        self.sender
            .send(ThreadCommand::SetSharedCtx(self.shared_ctx.clone()));
    }

    pub fn reset(&mut self) {
        self.modify_shared_ctx(|ctx| {
            ctx.tt.clear();
        });

        self.sender.send(ThreadCommand::Clear);
    }

    pub fn set_tt_size(&mut self, size_mib: usize) {
        self.modify_shared_ctx(|ctx| {
            ctx.tt.resize(size_mib);
        });
    }

    fn init_root_moves(&mut self, root_pos: &Position) {
        let root_moves = Arc::get_mut(&mut self.root_moves).unwrap();

        root_moves.clear();

        let mut new_root_moves = Vec::with_capacity(1024);
        generate_moves(&mut new_root_moves, root_pos);

        root_moves.clear();
        root_moves.reserve(new_root_moves.len());

        for mv in new_root_moves {
            let mut root_move = RootMove::default();
            root_move.pv.push(mv);
            root_moves.push(root_move);
        }
    }
}

impl Drop for Searcher {
    fn drop(&mut self) {
        self.kill_threads();
    }
}
