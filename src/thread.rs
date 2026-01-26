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

use crate::{
    board::Position,
    correction::CorrectionHistory,
    history::History,
    movepick::KillerTable,
    search::{MAX_PLY, SCORE_INF, Score},
    takmove::Move,
};

pub type PvList = arrayvec::ArrayVec<Move, { MAX_PLY as usize }>;

pub fn update_pv(pv: &mut PvList, mv: Move, child: &PvList) {
    pv.clear();
    pv.push(mv);
    pv.try_extend_from_slice(child).unwrap();
}

pub struct RootMove {
    pub display_score: Score,
    pub score: Score,
    pub upper_bound: bool,
    pub lower_bound: bool,
    pub seldepth: i32,
    pub pv: PvList,
    pub nodes: usize,
}

impl Default for RootMove {
    fn default() -> Self {
        Self {
            display_score: -SCORE_INF,
            score: -SCORE_INF,
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
    pub max_depth: i32,
    pub seldepth: i32,
    pub nodes: usize,
    pub root_moves: Vec<RootMove>,
    pub stack: Vec<StackEntry>,
    pub corrhist: CorrectionHistory,
    pub history: History,
    pub killers: [KillerTable; MAX_PLY as usize],
    pub min_nmp_ply: i32,
}

impl ThreadData {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            key_history: Vec::with_capacity(1024),
            root_depth: 0,
            max_depth: 0,
            seldepth: 0,
            nodes: 0,
            root_moves: Vec::with_capacity(1024),
            stack: vec![StackEntry::default(); MAX_PLY as usize + 1],
            corrhist: CorrectionHistory::new(),
            history: History::new(),
            killers: [Default::default(); MAX_PLY as usize],
            min_nmp_ply: 0,
        }
    }

    pub fn is_main_thread(&self) -> bool {
        self.id == 0
    }

    pub fn inc_nodes(&mut self) {
        self.nodes += 1;
    }

    pub fn reset_seldepth(&mut self) {
        self.seldepth = 0;
    }

    pub fn update_seldepth(&mut self, ply: i32) {
        self.seldepth = self.seldepth.max(ply + 1);
    }

    pub fn sort_root_moves(&mut self) {
        self.root_moves.sort_by(|a, b| b.score.cmp(&a.score));
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
        for root_move in self.root_moves.iter() {
            if root_move.pv[0] == mv {
                return root_move;
            }
        }

        unreachable!();
    }

    #[must_use]
    pub fn get_root_move_mut(&mut self, mv: Move) -> &mut RootMove {
        for root_move in self.root_moves.iter_mut() {
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

    pub fn reset(&mut self, key_history: &[u64]) {
        self.key_history.clear();
        self.key_history
            .reserve(key_history.len() + MAX_PLY as usize);

        self.key_history.extend_from_slice(key_history);
    }
}
