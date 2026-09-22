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
use crate::board::{BoardObserver, Position};
use crate::core::{Piece, PieceType, Player, Square};
use crate::search::MAX_DEPTH;
use arrayvec::ArrayVec;

pub const L1_SIZE: usize = 256;

pub const OUTPUT_BUCKETS: usize = 2;

pub const FT_Q: i32 = 255;
pub const L1_Q: i32 = 64;

pub const SCALE: i32 = 400;

#[repr(C, align(64))]
pub struct Network {
    pub ftw: [[i16; L1_SIZE]; 216],
    pub ftb: [i16; L1_SIZE],
    pub l1w: [[[i16; L1_SIZE]; 2]; OUTPUT_BUCKETS],
    pub l1b: [i16; OUTPUT_BUCKETS],
}

pub static NET: Network = unsafe { std::mem::transmute(*include_bytes!(env!("EVALFILE"))) };

#[must_use]
fn forward(acc: &Accumulator, stm: Player) -> i32 {
    let mut sum = 0;

    for (perspective, weights) in [stm, stm.flip()].iter().zip(&NET.l1w[stm.idx()]) {
        for (&a, &w) in acc.values[perspective.idx()].iter().zip(weights) {
            let c = a.clamp(0, FT_Q as i16);
            sum += i32::from(c) * i32::from(c * w);
        }
    }

    (sum / FT_Q + i32::from(NET.l1b[stm.idx()])) * SCALE / (FT_Q * L1_Q)
}

#[must_use]
pub(super) fn evaluate_once(pos: &Position) -> i32 {
    let mut acc = Accumulator::default();
    acc.reset_both(pos);
    forward(&acc, pos.stm())
}

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
        //self.acc_stacc[self.top_idx].reset_both(_pos);

        for player in [Player::P1, Player::P2] {
            if !self.acc_stacc[self.top_idx].is_dirty(player) {
                continue;
            }

            let mut curr = self.top_idx - 1;
            while self.acc_stacc[curr].is_dirty(player) {
                curr -= 1;
            }

            loop {
                let [prev_acc, curr_acc] = self.acc_stacc.get_disjoint_mut([curr, curr + 1]).unwrap();

                curr_acc.apply_updates(prev_acc, player);

                curr += 1;
                if curr == self.top_idx {
                    break;
                }
            }
        }
    }
}

fn feature_idx(perspective: Player, side: Player, pt: PieceType, sq: Square) -> usize {
    // TODO: was tired and got side and piecetype backwards
    pt.idx() * 72 + usize::from(side != perspective) * 36 + sq.idx()
}

#[derive(Clone, Debug, Default)]
pub struct NnueUpdates {
    adds: ArrayVec<(Piece, Square), 6>,
    subs: ArrayVec<(Piece, Square), 6>,
}

#[derive(Clone, Debug, Default)]
pub struct UpdateContext {
    pub updates: NnueUpdates,
}

#[derive(Clone)]
struct Accumulator {
    values: [[i16; L1_SIZE]; Player::COUNT],
    ctx: UpdateContext,
    dirty: [bool; Player::COUNT],
}

impl Accumulator {
    fn activate_single(&mut self, player: Player, feature: usize) {
        for (v, w) in self.values[player.idx()].iter_mut().zip(&NET.ftw[feature]) {
            *v += *w;
        }
    }

    fn activate_both(&mut self, p1_feature: usize, p2_feature: usize) {
        self.activate_single(Player::P1, p1_feature);
        self.activate_single(Player::P2, p2_feature);
    }

    fn reset(&mut self, pos: &Position, player: Player) {
        self.values[player.idx()] = NET.ftb;

        let stacks = pos.stacks();

        for side in [Player::P1, Player::P2] {
            for sq in pos.player_bb(side) {
                let pt = stacks.top(sq).unwrap();
                let feature = feature_idx(player, side, pt, sq);
                for (a, w) in self.values[player.idx()].iter_mut().zip(&NET.ftw[feature]) {
                    *a += w;
                }
            }
        }

        self.set_updated(player);
    }

    fn reset_both(&mut self, pos: &Position) {
        self.reset(pos, Player::P1);
        self.reset(pos, Player::P2);
    }

    fn set_dirty(&mut self) {
        self.dirty.fill(true);
    }

    fn set_updated(&mut self, player: Player) {
        self.dirty[player.idx()] = false;
    }

    #[must_use]
    fn is_dirty(&self, player: Player) -> bool {
        self.dirty[player.idx()]
    }

    fn apply_updates(&mut self, src: &Self, player: Player) {
        let dst = &mut self.values[player.idx()];
        let src = &src.values[player.idx()];

        let updates = &self.ctx.updates;

        if updates.adds.is_empty() && updates.subs.is_empty() {
            *dst = *src;
            self.set_updated(player);
            return;
        }

        let mut add_idx = 0;
        let mut sub_idx = 0;

        match (updates.adds.is_empty(), updates.subs.is_empty()) {
            (false, false) => {
                let (add_piece, add_sq) = updates.adds[0];
                let add = feature_idx(player, add_piece.player(), add_piece.piece_type(), add_sq);

                let (sub_piece, sub_sq) = updates.subs[0];
                let sub = feature_idx(player, sub_piece.player(), sub_piece.piece_type(), sub_sq);

                Self::add_sub(dst, src, add, sub);

                add_idx += 1;
                sub_idx += 1;
            }
            (false, true) => {
                let (add_piece, add_sq) = updates.adds[0];
                let add = feature_idx(player, add_piece.player(), add_piece.piece_type(), add_sq);

                Self::add(dst, src, add);

                add_idx += 1;
            }
            (true, false) => {
                let (sub_piece, sub_sq) = updates.subs[0];
                let sub = feature_idx(player, sub_piece.player(), sub_piece.piece_type(), sub_sq);

                Self::sub(dst, src, sub);

                sub_idx += 1;
            }
            _ => {}
        }

        while add_idx < updates.adds.len() {
            let (add_piece, add_sq) = updates.adds[add_idx];
            let add = feature_idx(player, add_piece.player(), add_piece.piece_type(), add_sq);

            Self::add_in_place(dst, add);

            add_idx += 1;
        }

        while sub_idx < updates.subs.len() {
            let (sub_piece, sub_sq) = updates.subs[sub_idx];
            let sub = feature_idx(player, sub_piece.player(), sub_piece.piece_type(), sub_sq);

            Self::sub_in_place(dst, sub);

            sub_idx += 1;
        }

        self.set_updated(player);
    }

    fn add_sub(dst: &mut [i16; L1_SIZE], src: &[i16; L1_SIZE], add: usize, sub: usize) {
        for (((dst, src), add), sub) in dst.iter_mut().zip(src).zip(&NET.ftw[add]).zip(&NET.ftw[sub]) {
            *dst = *src + *add - *sub;
        }
    }

    fn add(dst: &mut [i16; L1_SIZE], src: &[i16; L1_SIZE], add: usize) {
        for ((dst, src), add) in dst.iter_mut().zip(src).zip(&NET.ftw[add]) {
            *dst = *src + *add;
        }
    }

    fn sub(dst: &mut [i16; L1_SIZE], src: &[i16; L1_SIZE], sub: usize) {
        for ((dst, src), sub) in dst.iter_mut().zip(src).zip(&NET.ftw[sub]) {
            *dst = *src - *sub;
        }
    }

    fn add_in_place(v: &mut [i16; L1_SIZE], add: usize) {
        for (v, add) in v.iter_mut().zip(&NET.ftw[add]) {
            *v += *add;
        }
    }

    fn sub_in_place(v: &mut [i16; L1_SIZE], sub: usize) {
        for (v, sub) in v.iter_mut().zip(&NET.ftw[sub]) {
            *v -= *sub;
        }
    }
}

impl Default for Accumulator {
    fn default() -> Self {
        Self {
            values: [[0; _]; _],
            ctx: Default::default(),
            dirty: [false; _],
        }
    }
}

pub struct NnueObserver<'a> {
    ctx: &'a mut UpdateContext,
}

impl<'a> NnueObserver<'a> {
    fn new(ctx: &'a mut UpdateContext) -> Self {
        Self { ctx }
    }
}

impl<'a> BoardObserver for NnueObserver<'a> {
    fn top_added(&mut self, _pos: &Position, top: Piece, sq: Square) {
        self.ctx.updates.adds.push((top, sq));
    }

    fn top_removed(&mut self, _pos: &Position, top: Piece, sq: Square) {
        self.ctx.updates.subs.push((top, sq));
    }

    fn top_mutated(&mut self, _pos: &Position, old_top: Piece, new_top: Piece, sq: Square) {
        self.ctx.updates.adds.push((new_top, sq));
        self.ctx.updates.subs.push((old_top, sq));
    }

    fn finalize(&mut self, _pos: &Position) {
        //
    }
}
