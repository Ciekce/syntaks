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

use crate::bitboard::Bitboard;
use crate::eval::static_eval;
use crate::limit::Limits;
use crate::movegen::generate_moves;
use crate::movepick::Movepicker;
use crate::position::Position;
use crate::takmove::Move;
use crate::tei::TeiOptions;
use crate::thread::{PvList, RootMove, SharedContext, TerminalState, ThreadData, set_pv, update_pv};
use crate::ttable::{ProbedEntry, TtFlag};
use crate::util::command_channel::{Receiver, Sender, channel};
use crate::wdl;
use crate::{core::*, tunable};
use std::io::Write;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Instant;

#[allow(unused)]
use crate::stats;

pub const MAX_THREADS: u32 = 2048;
pub const MAX_DEPTH: i32 = 248;

const WIDEN_REPORT_DELAY: f64 = 1.0;
const VERBOSE_MULTIPV_DELAY: f64 = 1.0;
const CURRMOVE_REPORT_DELAY: f64 = 2.5;

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
static LMR_REDUCTIONS: [[i32; LMR_TABLE_MOVES]; MAX_DEPTH as usize] = {
    #![allow(clippy::needless_range_loop)]

    const BASE: f64 = 0.0;
    const DIVISOR: f64 = 3.0;

    let mut reductions = [[0; _]; _];

    for depth in 1..MAX_DEPTH as usize {
        let ln_depth = (depth as f64).ln();
        for move_number in 1..LMR_TABLE_MOVES {
            let ln_move_number = (move_number as f64).ln();
            let reduction = ((BASE + ln_depth * ln_move_number / DIVISOR) * 1024.0) as i32;
            reductions[depth][move_number] = reduction;
        }
    }

    reductions
};

#[derive(Clone)]
struct PlyData {
    movelist: Vec<Move>,
    scores: Vec<i32>,
    pv: PvList,
}

impl PlyData {
    fn new() -> Self {
        Self {
            movelist: Vec::with_capacity(256),
            scores: Vec::with_capacity(256),
            pv: PvList::new(),
        }
    }
}

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
    data_stack: &mut [PlyData],
    pos: &Position,
    depth: i32,
    ply: usize,
    mut alpha: Score,
    mut beta: Score,
    expected_cutnode: bool,
) -> Score {
    if thread.shared().has_stopped() {
        return 0;
    }

    if !NT::ROOT_NODE && thread.is_main_thread() && thread.root_depth > 1 && thread.check_stop_hard() {
        return 0;
    }

    if !NT::ROOT_NODE {
        alpha = alpha.max(loss_in(ply));
        beta = beta.min(win_in(ply));
        if alpha >= beta {
            return alpha;
        }
    }

    let beta = beta;

    if !NT::ROOT_NODE {
        let mut push_to_pv = |bb: Bitboard| {
            if NT::PV_NODE {
                let pt = if pos.flats_in_hand(pos.stm()) > 0 {
                    PieceType::Flat
                } else {
                    PieceType::Capstone
                };

                let sq = bb.lsb().unwrap();
                let mv = Move::placement(pt, sq);

                set_pv(&mut data_stack[0].pv, mv);
            }
        };

        let influence = pos.influence(pos.stm());

        let vert_crit = influence[0] & influence[1];
        let horz_crit = influence[2] & influence[3];
        let criticals = vert_crit | horz_crit;
        let open_criticals = criticals & !pos.occ();

        if !open_criticals.is_empty() {
            thread.inc_nodes();
            push_to_pv(open_criticals);
            return win_in(ply + 1);
        }

        for (criticals, edges) in [
            (vert_crit, Bitboard::UPPER_EDGE | Bitboard::LOWER_EDGE),
            (horz_crit, Bitboard::LEFT_EDGE | Bitboard::RIGHT_EDGE),
        ] {
            for sq in criticals & pos.piece_bb(PieceType::Flat) {
                let neighbors = Bitboard::neighbors(sq);
                let attackers = neighbors & pos.roads(pos.stm());
                let edge_bonus = sq.bb() & edges;
                if (attackers | edge_bonus).popcount() >= 3 {
                    thread.inc_nodes();
                    // todo push_to_pv
                    return win_in(ply + 1);
                }
            }
        }

        if pos.flats_in_hand(pos.stm()) > 0
            && ((pos.flats_in_hand(pos.stm()) == 1 && pos.caps_in_hand(pos.stm()) == 0)
                || pos.occ().popcount() == Square::COUNT as u32 - 1)
        {
            let hfcd = pos.hfcd(pos.stm());

            if hfcd >= -1 {
                thread.inc_nodes();
                push_to_pv(!pos.occ());
                return win_in(ply + 1);
            }

            if hfcd == -2 && alpha < 0 {
                push_to_pv(!pos.occ());
                alpha = 0;
                if alpha >= beta {
                    thread.inc_nodes();
                    return alpha;
                }
            }
        }
    }

    if depth <= 0 {
        return leaf::<NT>(thread, pos, 0, ply, alpha, beta);
    }

    thread.inc_nodes();

    if NT::PV_NODE {
        thread.update_seldepth(ply);
    }

    if ply >= MAX_DEPTH as usize {
        let static_eval = static_eval(&mut thread.nnue_state, pos);
        let correction = thread.corrhist.correction(pos, &thread.key_history);
        return static_eval + correction;
    }

    let mut depth = depth.min(MAX_DEPTH);

    let excluded = thread.stack[ply].excluded;

    let tt_hit;
    let tt_entry;

    if excluded.is_some() {
        tt_hit = false;
        tt_entry = ProbedEntry::default();
    } else {
        (tt_hit, tt_entry) = thread.shared().tt.probe(pos.key(), ply);

        if !NT::PV_NODE
            && tt_entry.depth >= depth
            && (tt_entry.score <= alpha || expected_cutnode)
            && match tt_entry.flag {
                None => false,
                Some(TtFlag::UpperBound) => tt_entry.score <= alpha,
                Some(TtFlag::LowerBound) => tt_entry.score >= beta,
                Some(TtFlag::Exact) => true,
            }
        {
            return tt_entry.score;
        }
    }

    let tt_move = if NT::ROOT_NODE && thread.root_depth > 1 {
        Some(thread.root_moves[thread.pv_idx].mv())
    } else {
        tt_entry.mv
    };

    if excluded.is_none()
        && (NT::PV_NODE || expected_cutnode)
        && depth >= 3
        && (tt_move.is_none() || (tt_hit && tt_entry.depth + 2 < depth))
    {
        depth -= 1;
    }

    let raw_eval = if tt_hit {
        tt_entry.raw_static_eval
    } else {
        static_eval(&mut thread.nnue_state, pos)
    };

    if !tt_hit {
        thread.shared().tt.store_static_eval(pos.key(), raw_eval);
    }

    let correction = thread.corrhist.correction(pos, &thread.key_history);
    let static_eval = raw_eval + correction;

    thread.stack[ply].static_eval = static_eval;

    let improving = ply < 2 || static_eval > thread.stack[ply - 2].static_eval;
    let opp_worsening = ply > 0 && static_eval > -thread.stack[ply - 1].static_eval;

    if !NT::PV_NODE && excluded.is_none() {
        let parent_reduction = thread.stack[ply - 1].reduction;
        let parent_static_eval = thread.stack[ply - 1].static_eval;

        if depth < MAX_DEPTH && parent_reduction >= 3 && static_eval + parent_static_eval <= 0 {
            depth += 1;
        }

        tunable!(RFP_DEPTH_SCALE: i32 = 91; range(0, 256));
        tunable!(RFP_BASE: i32 = 181; range(0, 512));
        tunable!(RFP_CUTNODE: i32 = 91; range(0, 256));
        tunable!(RFP_IMPROVING: i32 = 115; range(0, 512));
        tunable!(RFP_OPP_WORSENING: i32 = 23; range(0, 128));

        // reverse futility pruning (rfp)
        let rfp_margin = (RFP_DEPTH_SCALE * depth + RFP_BASE
            - RFP_CUTNODE * i32::from(expected_cutnode)
            - RFP_IMPROVING * i32::from(improving)
            - RFP_OPP_WORSENING * i32::from(opp_worsening))
        .max(0);
        if depth <= 10 && static_eval - rfp_margin >= beta {
            return if !is_decisive(static_eval) && !is_decisive(beta) {
                (static_eval + beta) / 2
            } else {
                static_eval
            };
        }

        tunable!(NMP_BASE_R: i32 = 4274; range(0, 8192));
        tunable!(NMP_DEPTH_SCALE: i32 = 378; range(0, 1024));

        // nullmove pruning (nmp)
        if expected_cutnode && depth >= 4 && static_eval >= beta && thread.stack[ply - 1].mv.is_some() {
            let r = (NMP_BASE_R + NMP_DEPTH_SCALE * depth) / 1024;

            let new_pos = thread.apply_nullmove(ply, pos);

            let score = -search::<NonPvNode>(
                thread,
                data_stack,
                &new_pos,
                (depth - r).max(1), // dont allow dropping straight to eval
                ply + 1,
                -beta,
                -beta + 1,
                false,
            );

            thread.pop_nullmove();

            if score >= beta {
                return if is_win(score) { beta } else { score };
            }
        }
    }

    let (data, child_data) = data_stack.split_first_mut().unwrap();

    let mut best_score = -SCORE_INF;
    let mut best_move = None;

    let mut tt_flag = TtFlag::UpperBound;

    let prev_move = if ply > 0 { thread.stack[ply - 1].mv } else { None };

    let mut movepicker = Movepicker::new(
        pos,
        &mut data.movelist,
        &mut data.scores,
        tt_move,
        thread.killers[ply],
        prev_move,
    );

    let mut move_count = 0;
    let mut faillow_moves = arrayvec::ArrayVec::<Move, 32>::new();

    while let Some(mv) = movepicker.next(&thread.history) {
        debug_assert!(pos.is_legal(mv));

        if NT::ROOT_NODE && !thread.is_legal_root_move(mv) {
            continue;
        }

        if Some(mv) == excluded {
            continue;
        }

        #[allow(clippy::collapsible_if)]
        if !NT::ROOT_NODE && !is_loss(best_score) && (!NT::PV_NODE || !cfg!(feature = "datagen")) {
            tunable!(LMP_LOSING_MARGIN: i32 = 154; range(0, 512));
            tunable!(LMP_BASE: i32 = 1385; range(0, 2048));
            tunable!(LMP_DEPTH_SCALE: i32 = 464; range(0, 1024));

            if depth <= 8 {
                let losing = static_eval + LMP_LOSING_MARGIN <= alpha;
                let limit = (LMP_BASE + LMP_DEPTH_SCALE * depth * depth)
                    * (1 + i32::from(improving))
                    * (1 + i32::from(!losing))
                    * (1 + i32::from(NT::PV_NODE))
                    / 1024;
                if move_count as i32 >= limit {
                    break;
                }
            }
        }

        move_count += 1;

        let mut extension = 0;

        tunable!(SE_BETA_SCALE: i32 = 1508; range(0, 4096));
        tunable!(SE_DEXT_MARGIN: i32 = 36; range(0, 128));
        tunable!(SE_TEXT_MARGIN: i32 = 146; range(0, 256));

        if !NT::ROOT_NODE
            && Some(mv) == tt_move
            && excluded.is_none()
            && depth >= 5
            && tt_entry.depth >= depth - 3
            && tt_entry.flag != Some(TtFlag::UpperBound)
            && !is_decisive(tt_entry.score)
        {
            let s_beta = tt_entry.score - SE_BETA_SCALE * depth / 1024;
            let s_depth = (depth - 1) / 2;

            thread.stack[ply].excluded = Some(mv);
            let score = search::<NonPvNode>(
                thread,
                child_data,
                pos,
                s_depth,
                ply,
                s_beta - 1,
                s_beta,
                expected_cutnode,
            );
            thread.stack[ply].excluded = None;

            if score < s_beta {
                extension += 1
                    + i32::from(!NT::PV_NODE && score < s_beta - SE_DEXT_MARGIN)
                    + i32::from(!NT::PV_NODE && score < s_beta - SE_TEXT_MARGIN);
            } else if !NT::PV_NODE && score >= beta {
                return score;
            } else if tt_entry.score >= beta {
                extension -= 1;
            } else if expected_cutnode {
                extension -= 1;
            }
        }

        if NT::ROOT_NODE
            && thread.shared().options.show_curr_move
            && !thread.shared().options.minimal
            && !thread.shared().options.silent
            && thread.is_main_thread()
            && thread.shared().elapsed() > CURRMOVE_REPORT_DELAY
        {
            let move_number = thread.pv_idx + move_count;
            println!("info depth {} currmove {} currmovenumber {}", depth, mv, move_number);
        }

        if NT::PV_NODE {
            child_data[0].pv.clear();
        }

        let new_pos = thread.apply_move(ply, pos, mv);
        thread.shared().tt.prefetch(new_pos.key());

        let is_crush = mv.is_spread() && pos.top_pt(mv.spread_dest()) == Some(PieceType::Wall);

        if extension == 0 && is_crush {
            extension += 1;
        }

        let nodes_before = thread.nodes();

        let score = if let Some(state) = thread.check_terminal_state(ply, &new_pos, mv) {
            match state {
                TerminalState::Win => win_in(ply + 1),
                TerminalState::Draw => 0,
                TerminalState::Loss => loss_in(ply + 1),
            }
        } else {
            let mut score = 0;

            let new_depth = depth + extension - 1;

            if depth >= 2 && move_count >= 2 + usize::from(NT::ROOT_NODE) {
                let mut r = LMR_REDUCTIONS[depth as usize - 1][move_count.min(LMR_TABLE_MOVES) - 1];

                tunable!(PV_NODE_LMR: i32 = 1301; range(0, 4096));
                tunable!(CUTNODE_LMR: i32 = 1824; range(0, 4096));
                tunable!(IMPROVING_LMR: i32 = 401; range(0, 4096));
                tunable!(HIST_LMR: i32 = 732; range(0, 4096));
                tunable!(CORR_LMR: i32 = 395; range(0, 2048));
                tunable!(THREAT_LMR: i32 = 885; range(0, 4096));
                tunable!(SPREAD_GAIN_LMR: i32 = -307; range(-2048, 2048));

                r += PV_NODE_LMR * i32::from(!NT::PV_NODE);
                r += CUTNODE_LMR * i32::from(expected_cutnode);
                r += IMPROVING_LMR * i32::from(!improving);
                r -= thread.history.score(pos, mv, prev_move) * HIST_LMR / 24576;
                r -= (correction.abs() * CORR_LMR / 64).min(1024);

                let our_influence = new_pos.influence(pos.stm());
                let threats = (((our_influence[0] & our_influence[1]) | (our_influence[2] & our_influence[3]))
                    & !new_pos.occ())
                .popcount() as i32;
                r -= THREAT_LMR * threats.min(2);

                if mv.is_spread() {
                    let gain = (new_pos.hfcd(pos.stm()) - pos.hfcd(pos.stm())) / 2;
                    r += (1 - gain).clamp(0, 3) * SPREAD_GAIN_LMR;
                }

                r /= 1024;

                let reduced = (new_depth - r).max(1).min(new_depth);

                thread.stack[ply].reduction = new_depth - reduced;
                score = -search::<NonPvNode>(thread, child_data, &new_pos, reduced, ply + 1, -alpha - 1, -alpha, true);
                thread.stack[ply].reduction = 0;

                if score > alpha && reduced < new_depth {
                    tunable!(DODEEPER_BASE: i32 = 50509; range(0, 131072));
                    tunable!(DODEEPER_SCALE: i32 = 2835; range(0, 8192));

                    let do_deeper = score > best_score + (DODEEPER_BASE + DODEEPER_SCALE * new_depth) / 1024;
                    let new_depth = new_depth + i32::from(do_deeper);
                    score = -search::<NonPvNode>(
                        thread,
                        child_data,
                        &new_pos,
                        new_depth,
                        ply + 1,
                        -alpha - 1,
                        -alpha,
                        !expected_cutnode,
                    );

                    tunable!(LMR_HIST_BONUS_SCALE: i32 = 441; range(0, 1024));
                    tunable!(LMR_HIST_BONUS_OFFSET: i32 = 468; range(0, 1024));
                    tunable!(LMR_HIST_BONUS_MAX: i32 = 3326; range(0, 4096));

                    let bonus =
                        (LMR_HIST_BONUS_SCALE * new_depth - LMR_HIST_BONUS_OFFSET).clamp(0, LMR_HIST_BONUS_MAX * 1);
                    thread
                        .history
                        .update(pos, mv, prev_move, if score > alpha { bonus } else { -bonus });
                }
            } else if !NT::PV_NODE || move_count > 1 {
                score = -search::<NonPvNode>(
                    thread,
                    child_data,
                    &new_pos,
                    new_depth,
                    ply + 1,
                    -alpha - 1,
                    -alpha,
                    !expected_cutnode,
                );
            }

            if NT::PV_NODE && (move_count == 1 || score > alpha) {
                score = -search::<PvNode>(thread, child_data, &new_pos, new_depth, ply + 1, -beta, -alpha, false);
            }

            score
        };

        let nodes_after = thread.nodes();

        thread.pop_move();

        if thread.shared().has_stopped() {
            return 0;
        }

        if NT::ROOT_NODE {
            let root_depth = thread.root_depth;
            let seldepth = thread.seldepth;

            let root_move = thread.get_root_move_mut(mv);

            root_move.nodes += nodes_after - nodes_before;
            root_move.window_score = score;

            if move_count == 1 || score > alpha {
                root_move.searched_depth = root_depth;
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

                update_pv(&mut root_move.pv, mv, &child_data[0].pv);
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
                update_pv(&mut data.pv, mv, &child_data[0].pv);
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
        tunable!(HIST_BONUS_SCALE: i32 = 590; range(0, 1024));
        tunable!(HIST_BONUS_OFFSET: i32 = 486; range(0, 1024));
        tunable!(HIST_BONUS_MAX: i32 = 3292; range(0, 4096));
        tunable!(HIST_MALUS_SCALE: i32 = 584; range(0, 1024));
        tunable!(HIST_MALUS_OFFSET: i32 = 581; range(0, 1024));
        tunable!(HIST_MALUS_MAX: i32 = 3238; range(0, 4096));

        let bonus = (HIST_BONUS_SCALE * depth - HIST_BONUS_OFFSET).clamp(0, HIST_BONUS_MAX * 1);
        let malus = (HIST_MALUS_SCALE * depth - HIST_MALUS_OFFSET).clamp(0, HIST_MALUS_MAX * 1);

        thread.history.update(pos, best_move, prev_move, bonus);

        for &mv in faillow_moves.iter() {
            thread.history.update(pos, mv, prev_move, -malus);
        }

        if best_score >= beta {
            thread.killers[ply].push(best_move);
        }
    }

    if !NT::PV_NODE && best_score >= beta && !is_decisive(best_score) && !is_decisive(beta) {
        best_score = (best_score * depth + beta) / (depth + 1);
    }

    if excluded.is_none() {
        if tt_flag == TtFlag::Exact
            || (tt_flag == TtFlag::UpperBound && best_score < static_eval)
            || (tt_flag == TtFlag::LowerBound && best_score > static_eval)
        {
            thread
                .corrhist
                .update(pos, &thread.key_history, depth, best_score, static_eval);
        }

        if !NT::ROOT_NODE || thread.pv_idx == 0 {
            thread
                .shared()
                .tt
                .store(pos.key(), best_score, raw_eval, best_move, depth, ply, tt_flag);
        }
    }

    best_score
}

fn leaf<NT: NodeType>(
    thread: &mut ThreadData,
    pos: &Position,
    depth: i32,
    ply: usize,
    alpha: Score,
    beta: Score,
) -> Score {
    thread.inc_nodes();

    if NT::PV_NODE {
        thread.update_seldepth(ply);
    }

    let (tt_hit, tt_entry) = thread.shared().tt.probe(pos.key(), ply);

    if !NT::PV_NODE
        && match tt_entry.flag {
            None => false,
            Some(TtFlag::UpperBound) => tt_entry.score <= alpha,
            Some(TtFlag::LowerBound) => tt_entry.score >= beta,
            Some(TtFlag::Exact) => true,
        }
    {
        return tt_entry.score;
    }

    let raw_eval = if tt_hit {
        tt_entry.raw_static_eval
    } else {
        static_eval(&mut thread.nnue_state, pos)
    };

    if !tt_hit {
        thread.shared().tt.store_static_eval(pos.key(), raw_eval);
    }

    let correction = thread.corrhist.correction(pos, &thread.key_history);
    let static_eval = raw_eval + correction;

    if ply >= MAX_DEPTH as usize {
        return static_eval;
    }

    let repeated = depth <= -4 && thread.key_history[thread.key_history.len() - 4] == pos.key();
    if !NT::PV_NODE
        && tt_hit
        && !repeated
        && let Some((mv, flag)) = Option::zip(tt_entry.mv, tt_entry.flag)
        && pos.is_legal(mv)
    {
        let new_pos = thread.apply_move(ply, pos, mv);
        let score = -leaf::<NT>(thread, &new_pos, depth - 1, ply + 1, -alpha - 1, -alpha);
        thread.pop_move();
        let score = match flag {
            TtFlag::UpperBound => i32::min(static_eval, score),
            TtFlag::LowerBound => i32::max(static_eval, score),
            TtFlag::Exact => unreachable!(),
        };
        return score;
    }
    static_eval
}

fn run_search(shared: Arc<SharedContext>, ctx: &SearchContext, thread: &mut ThreadData) {
    assert!(thread.shared.is_none());
    thread.shared = Some(shared);

    let counter = thread.shared().get_counter();

    thread.root_moves.clear();
    thread.root_moves.reserve(ctx.root_moves.len());

    for mv in ctx.root_moves.iter() {
        thread.root_moves.push(mv.clone());
    }

    thread.key_history.clear();
    thread.key_history.reserve(ctx.key_history.len());
    thread.key_history.extend_from_slice(&ctx.key_history);

    thread.nnue_state.reset(&ctx.root_pos);

    if thread.is_main_thread() {
        thread.tm_ctx = Default::default();
    }

    let mut data_stack = vec![PlyData::new(); MAX_DEPTH as usize * 2];

    thread.root_depth = 1;

    loop {
        for root_move in thread.root_moves.iter_mut() {
            root_move.previous_score = root_move.score;
        }

        thread.pv_idx = 0;
        while thread.pv_idx < ctx.multipv {
            thread.reset_seldepth();

            tunable!(ASP_DELTA: i32 = 32; range(1, 64));
            tunable!(ASP_WIDEN: i32 = 1334; range(1280, 4096));

            let mut delta = ASP_DELTA * 1;

            let mut alpha = -SCORE_INF;
            let mut beta = SCORE_INF;

            if thread.root_depth >= 3 {
                let last_score = thread.root_moves[thread.pv_idx].window_score;
                alpha = (last_score - delta).max(-SCORE_INF);
                beta = (last_score + delta).min(SCORE_INF);
            }

            let mut reduction = 0;

            while !thread.shared().has_stopped() {
                let root_depth = (thread.root_depth - reduction).max(1);
                let score = search::<RootNode>(
                    thread,
                    &mut data_stack,
                    &ctx.root_pos,
                    root_depth,
                    0,
                    alpha,
                    beta,
                    false,
                );

                thread.sort_remaining_root_moves();

                if (score > alpha && score < beta) || thread.shared().has_stopped() {
                    break;
                }

                if thread.is_main_thread()
                    && !thread.shared().options.minimal
                    && !thread.shared().options.silent
                    && thread.shared().options.multipv == 1
                {
                    let time = thread.shared().elapsed();
                    if time >= WIDEN_REPORT_DELAY {
                        let nodes = thread.shared().total_nodes();
                        report_single(thread, ctx, time, nodes, thread.pv_idx);
                    }
                }

                delta = (delta * ASP_WIDEN / 1024).min(SCORE_INF);
                if score <= alpha {
                    reduction = 0;
                    beta = (alpha + beta) / 2;
                    alpha = (alpha - delta).max(-SCORE_INF);
                } else {
                    reduction = (reduction + 1).min(3);
                    beta = (beta + delta).min(SCORE_INF);
                }
            }

            thread.sort_searched_root_moves();

            if thread.is_main_thread() {
                let last_pv = thread.pv_idx + 1 == ctx.multipv;

                if last_pv
                    && !thread.shared().has_stopped()
                    && (thread.root_depth >= ctx.max_depth || thread.check_stop_soft())
                {
                    thread.shared().stop();
                }

                if !thread.shared().options.silent
                    && (thread.shared().has_stopped()
                        || (!thread.shared().options.minimal
                            && (last_pv || thread.shared().elapsed() >= VERBOSE_MULTIPV_DELAY)))
                {
                    report(thread, ctx, thread.shared().elapsed());
                }
            }

            if thread.shared().has_stopped() {
                break;
            }

            thread.pv_idx += 1;
        }

        if thread.shared().has_stopped() {
            break;
        }

        thread.root_depth += 1;
    }

    if thread.is_main_thread() {
        counter.unregister_and_wait();

        #[cfg(feature = "datagen")]
        {
            thread.shared().set_result(thread.pv_move().clone());
        }

        let silent = thread.shared().options.silent;
        let best_move = thread.pv_move().mv();

        thread.shared = None;
        counter.complete_search(|| {
            if !silent {
                final_report(best_move);
            }
            stats::print();
        });
    } else {
        thread.shared = None;
        counter.unregister_thread();
    }
}

fn report_single(thread: &ThreadData, ctx: &SearchContext, time: f64, nodes: usize, pv_idx: usize) -> bool {
    let root_move = &thread.root_moves[pv_idx];

    // previous scores are exact, as the depth was completed
    let (score, upper_bound, lower_bound) = if root_move.score == -SCORE_INF {
        (root_move.previous_score, false, false)
    } else {
        (root_move.display_score, root_move.upper_bound, root_move.lower_bound)
    };

    if score == -SCORE_INF {
        // search stopped at d1 before pv_idx+1 moves were searched
        return false;
    }

    assert_ne!(score, -SCORE_INF);

    let ms = (time * 1000.0) as usize;
    let nps = ((nodes as f64) / time) as usize;

    let mut out = std::io::stdout().lock();
    write!(out, "info ").unwrap();

    if thread.shared().options.multipv > 1 {
        write!(out, "multipv {} ", pv_idx + 1).unwrap();
    }

    write!(
        out,
        "depth {} seldepth {} time {} nodes {} nps {} score ",
        root_move.searched_depth, root_move.seldepth, ms, nodes, nps
    )
    .unwrap();

    if is_decisive(score) {
        if thread.shared().options.mate_scores {
            if score > 0 {
                write!(out, "mate {}", (SCORE_MATE - score + 1) / 2).unwrap();
            } else {
                write!(out, "mate {}", -(SCORE_MATE + score) / 2).unwrap();
            }
        } else {
            const MATE_DISPLAY_BASE: Score = 30000;
            if score > 0 {
                write!(out, "cp {}", MATE_DISPLAY_BASE + score - SCORE_MATE).unwrap();
            } else {
                write!(out, "cp {}", -MATE_DISPLAY_BASE + score + SCORE_MATE).unwrap();
            }
        }
    } else {
        write!(out, "cp {}", wdl::normalise_score(score)).unwrap();
    }

    if upper_bound {
        assert!(!lower_bound);
        write!(out, " upperbound").unwrap();
    }

    if lower_bound {
        assert!(!upper_bound);
        write!(out, " lowerbound").unwrap();
    }

    if is_decisive(score) {
        if score > 0 {
            write!(out, " wdl 1000 0 0").unwrap();
        } else {
            write!(out, " wdl 0 0 1000").unwrap();
        }
    } else {
        let (w, d, l) = if thread.shared().options.human_wdl {
            wdl::human_probabilities(score, &ctx.root_pos)
        } else {
            wdl::probabilities(score)
        };
        write!(out, " wdl {} {} {}", w, d, l).unwrap();
    }

    let hashfull = thread.shared().tt.estimate_full_permille();
    write!(out, " hashfull {}", hashfull).unwrap();

    write!(out, " pv").unwrap();

    for mv in root_move.pv.iter() {
        write!(out, " {}", mv).unwrap();
    }

    writeln!(out).unwrap();

    true
}

fn report(thread: &ThreadData, ctx: &SearchContext, time: f64) {
    let nodes = thread.shared().total_nodes();
    for pv_idx in 0..ctx.multipv {
        if !report_single(thread, ctx, time, nodes, pv_idx) {
            break;
        }
    }
}

fn final_report(best_move: Move) {
    println!("bestmove {}", best_move);
}

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
enum ThreadCommand {
    Ping,
    StartSearch(Arc<SharedContext>, SearchContext),
    Clear,
    Quit,
}

pub struct Searcher {
    shared_ctx: Arc<SharedContext>,
    threads: Vec<JoinHandle<()>>,
    sender: Sender<ThreadCommand>,
}

impl Searcher {
    pub fn new(tt_size_mib: usize) -> Self {
        let shared_ctx = Arc::new(SharedContext::new(tt_size_mib));

        let (mut sender, mut receiver) = channel(1);

        let thread = thread::spawn({
            let receiver = receiver.next().unwrap();
            move || {
                if std::panic::catch_unwind(move || {
                    Self::run_thread(0, receiver);
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
        }
    }

    fn run_thread(id: u32, mut receiver: Receiver<ThreadCommand>) {
        let mut data = ThreadData::new(id);

        loop {
            match receiver.recv(|cmd| cmd.clone()) {
                ThreadCommand::Ping => {}
                ThreadCommand::StartSearch(shared, ctx) => run_search(shared, &ctx, &mut data),
                ThreadCommand::Clear => {
                    data.corrhist.clear();
                    data.history.clear();
                    data.killers.fill(Default::default());
                }
                ThreadCommand::Quit => break,
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_search(
        &mut self,
        pos: &Position,
        new_key_history: &[u64],
        start_time: Instant,
        limits: Limits,
        max_depth: i32,
        moves_to_search: &[Move],
        options: &TeiOptions,
    ) {
        let threads = self.threads.len() as u32;
        self.modify_shared_ctx(|ctx| {
            ctx.init_search(options, start_time, limits, threads);
        });

        let root_moves = init_root_moves(pos, moves_to_search);
        let multipv = options.multipv.min(root_moves.len());

        let ctx = SearchContext::new(
            max_depth,
            multipv,
            *pos,
            Arc::new(root_moves),
            Arc::new(new_key_history.to_vec()),
        );

        self.sender
            .send(ThreadCommand::StartSearch(self.shared_ctx.clone(), ctx));
    }

    pub fn stop(&mut self) {
        self.shared_ctx.stop();
    }

    #[cfg(feature = "datagen")]
    pub fn result(&self) -> RootMove {
        self.shared_ctx.result().unwrap()
    }

    pub fn is_searching(&self) -> bool {
        self.shared_ctx.is_searching()
    }

    pub fn wait(&self) {
        self.shared_ctx.get_counter().wait();
    }

    #[must_use]
    pub fn total_nodes(&self) -> usize {
        self.shared_ctx.total_nodes()
    }

    fn kill_threads(&mut self) {
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
        debug_assert!(!self.is_searching());
        let ctx =
            Arc::get_mut(&mut self.shared_ctx).expect("shared search context still referenced by a search thread");
        func(ctx);
    }

    pub fn reset(&mut self) {
        let thread_count = self.threads.len();

        self.modify_shared_ctx(|ctx| {
            ctx.tt.clear(thread_count);
        });

        self.sender.send(ThreadCommand::Clear);
    }

    pub fn set_tt_size(&mut self, size_mib: usize) {
        let thread_count = self.threads.len();
        self.modify_shared_ctx(|ctx| {
            ctx.tt.resize(size_mib, thread_count);
        });
    }

    pub fn set_threads(&mut self, count: u32) {
        self.kill_threads();

        self.modify_shared_ctx(|ctx| {
            ctx.set_threads(count);
        });

        self.threads.reserve(count as usize);

        let (sender, mut receiver) = channel(count);

        for id in 0..count {
            let thread = thread::spawn({
                let receiver = receiver.next().unwrap();
                move || {
                    if std::panic::catch_unwind(move || {
                        Self::run_thread(id, receiver);
                    })
                    .is_err()
                    {
                        std::process::exit(-1);
                    }
                }
            });
            self.threads.push(thread);
        }

        self.sender = sender;

        self.sender.send(ThreadCommand::Ping);
    }
}

fn init_root_moves(root_pos: &Position, moves_to_search: &[Move]) -> Vec<RootMove> {
    if !moves_to_search.is_empty() {
        print!("info string searchmoves:");
        for &mv in moves_to_search {
            assert!(root_pos.is_legal(mv));
            print!(" {}", mv);
        }
        println!();

        return moves_to_search.iter().map(|&mv| RootMove::new(mv)).collect();
    }

    let mut moves = Vec::with_capacity(1024);
    generate_moves(&mut moves, root_pos);

    moves.iter().map(|&mv| RootMove::new(mv)).collect()
}

impl Drop for Searcher {
    fn drop(&mut self) {
        self.kill_threads();
    }
}
