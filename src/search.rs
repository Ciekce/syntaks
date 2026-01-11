use crate::board::{FlatCountOutcome, Position};
use crate::eval::static_eval;
use crate::limit::Limits;
use crate::movegen::generate_moves;
use crate::takmove::Move;
use std::time::Instant;

pub type Score = i32;

pub const SCORE_INF: Score = 32767;
pub const SCORE_MATE: Score = SCORE_INF - 1;
pub const SCORE_WIN: Score = 25000;
pub const SCORE_MAX_MATE: Score = SCORE_MATE - MAX_PLY as Score;

pub const MAX_PLY: i32 = 255;

type PvList = arrayvec::ArrayVec<Move, { MAX_PLY as usize }>;

fn update_pv(pv: &mut PvList, mv: Move, child: &PvList) {
    pv.clear();
    pv.push(mv);
    pv.try_extend_from_slice(&child).unwrap();
}

struct RootMove {
    score: Score,
    seldepth: i32,
    pv: PvList,
}

impl Default for RootMove {
    fn default() -> Self {
        Self {
            score: -SCORE_INF,
            seldepth: 0,
            pv: PvList::new(),
        }
    }
}

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

    fn has_stopped(&self) -> bool {
        self.stopped
    }
}

struct ThreadData {
    id: u32,
    key_history: Vec<u64>,
    root_depth: i32,
    max_depth: i32,
    seldepth: i32,
    nodes: usize,
    root_moves: Vec<RootMove>,
}

impl ThreadData {
    fn new(id: u32) -> Self {
        Self {
            id,
            key_history: Vec::with_capacity(1024),
            root_depth: 0,
            max_depth: 0,
            seldepth: 0,
            nodes: 0,
            root_moves: Vec::with_capacity(1024),
        }
    }

    fn is_main_thread(&self) -> bool {
        self.id == 0
    }

    fn inc_nodes(&mut self) {
        self.nodes += 1;
    }

    fn reset_seldepth(&mut self) {
        self.seldepth = 0;
    }

    fn update_seldepth(&mut self, ply: i32) {
        self.seldepth = self.seldepth.max(ply + 1);
    }

    fn apply_move(&mut self, pos: &Position, mv: Move) -> Position {
        self.key_history.push(pos.key());
        pos.apply_move(mv)
    }

    fn pop_move(&mut self) {
        self.key_history.pop();
    }

    fn is_drawn_by_repetition(&self, curr: u64, ply: i32) -> bool {
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

    fn get_root_move(&self, mv: Move) -> &RootMove {
        for root_move in self.root_moves.iter() {
            if root_move.pv[0] == mv {
                return root_move;
            }
        }

        unreachable!();
    }

    fn get_root_move_mut(&mut self, mv: Move) -> &mut RootMove {
        for root_move in self.root_moves.iter_mut() {
            if root_move.pv[0] == mv {
                return root_move;
            }
        }

        unreachable!();
    }

    fn pv_move(&self) -> &RootMove {
        &self.root_moves[0]
    }

    fn reset(&mut self, key_history: &[u64]) {
        self.key_history.clear();
        self.key_history
            .reserve(key_history.len() + MAX_PLY as usize);

        self.key_history.extend_from_slice(key_history);
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

pub struct Searcher {
    data: ThreadData,
}

impl Searcher {
    pub fn new() -> Self {
        Self {
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

        Self::run_search(&mut ctx, thread, pos, start_time);
    }

    fn run_search(
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

            Self::search::<RootNode>(
                ctx,
                thread,
                &mut movelists,
                &mut pvs,
                root_pos,
                thread.root_depth,
                0,
                -SCORE_INF,
                SCORE_INF,
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
                Self::report(thread, thread.root_depth, time);
            }

            thread.root_depth += 1;
        }

        if thread.is_main_thread() {
            let time = start_time.elapsed().as_secs_f64();
            Self::final_report(thread, thread.root_depth, time);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn search<NT: NodeType>(
        ctx: &mut SearchContext,
        thread: &mut ThreadData,
        movelists: &mut [Vec<Move>],
        pvs: &mut [PvList],
        pos: &Position,
        depth: i32,
        ply: i32,
        mut alpha: Score,
        beta: Score,
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
            return static_eval(pos);
        }

        if NT::PV_NODE {
            thread.update_seldepth(ply);
        }

        let (moves, movelists) = movelists.split_first_mut().unwrap();
        let (pv, child_pvs) = pvs.split_first_mut().unwrap();

        generate_moves(moves, pos);

        let mut best_score = -SCORE_INF;

        for (move_idx, &mv) in moves.iter().enumerate() {
            if NT::PV_NODE {
                child_pvs[0].clear();
            }

            let new_pos = thread.apply_move(pos, mv);

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

                if !NT::PV_NODE || move_idx > 0 {
                    score = -Self::search::<NonPvNode>(
                        ctx,
                        thread,
                        movelists,
                        child_pvs,
                        &new_pos,
                        depth - 1,
                        ply + 1,
                        -alpha - 1,
                        -alpha,
                    );
                }

                if NT::PV_NODE && (move_idx == 0 || score > alpha) {
                    score = -Self::search::<PvNode>(
                        ctx,
                        thread,
                        movelists,
                        child_pvs,
                        &new_pos,
                        depth - 1,
                        ply + 1,
                        -beta,
                        -alpha,
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

                if move_idx == 0 || score > alpha {
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

                if NT::PV_NODE {
                    update_pv(pv, mv, &child_pvs[0]);
                }
            }

            if score >= beta {
                break;
            }
        }

        best_score
    }

    fn report(thread: &ThreadData, depth: i32, time: f64) {
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

        print!(" pv");

        for mv in root_move.pv.iter() {
            print!(" {}", mv);
        }

        println!();
    }

    fn final_report(thread: &ThreadData, depth: i32, time: f64) {
        Self::report(thread, depth, time);

        let mv = thread.pv_move().pv[0];
        println!("bestmove {}", mv);
    }
}
