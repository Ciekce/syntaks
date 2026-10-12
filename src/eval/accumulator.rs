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
use crate::eval::nnue::*;

#[derive(Clone)]
#[repr(align(64))]
pub(super) struct Accumulator {
    pub values: [[i16; L1_SIZE]; Player::COUNT],
    pub ctx: UpdateContext,
    dirty: bool,
}

impl Accumulator {
    fn reset(&mut self, pos: &Position, player: Player) {
        self.values[player.idx()] = NET.ftb;

        let mut activate_top = |feature: usize| {
            let v = &mut self.values[player.idx()];
            let w = &NET.ftw_top[feature];
            for (v, w) in v.iter_mut().zip(w) {
                *v += *w;
            }
        };

        let stacks = pos.stacks();

        for side in [Player::P1, Player::P2] {
            for sq in pos.player_bb(side) {
                let pt = stacks.top(sq).unwrap();
                let feature = top_feature_idx(player, side, pt, sq);
                activate_top(feature);
            }
        }

        let mut activate_stack = |feature: usize| {
            let v = &mut self.values[player.idx()];
            let w = &NET.ftw_stack[feature];
            for (v, w) in v.iter_mut().zip(w) {
                *v += *w;
            }
        };

        for sq in pos.occ() {
            let height = stacks.height(sq);

            if height == 1 {
                continue;
            }

            let mut players = stacks.players(sq) >> 1;
            let count = height.min(13) - 1;

            for depth in 0..count {
                let color = Player::from_raw((players & 0x1) as u8).unwrap();
                let feature = stack_feature_idx(player, color, depth, sq);
                activate_stack(feature);
                players >>= 1;
            }
        }
    }

    pub fn reset_both(&mut self, pos: &Position) {
        self.reset(pos, Player::P1);
        self.reset(pos, Player::P2);

        self.set_updated();
    }

    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn set_updated(&mut self) {
        self.dirty = false;
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn apply_top_updates(&mut self, src: &Self, player: Player) {
        let dst = &mut self.values[player.idx()];
        let src = &src.values[player.idx()];

        let updates = &self.ctx.updates;

        if updates.adds.is_empty() && updates.subs.is_empty() {
            *dst = *src;
            return;
        }

        let mut add_idx = 0;
        let mut sub_idx = 0;

        match (updates.adds.is_empty(), updates.subs.is_empty()) {
            (false, false) => {
                let (add_piece, add_sq) = updates.adds[0];
                let add = top_feature_idx(player, add_piece.player(), add_piece.piece_type(), add_sq);

                let (sub_piece, sub_sq) = updates.subs[0];
                let sub = top_feature_idx(player, sub_piece.player(), sub_piece.piece_type(), sub_sq);

                Self::add_sub(&NET.ftw_top, dst, src, add, sub);

                add_idx += 1;
                sub_idx += 1;
            }
            (false, true) => {
                let (add_piece, add_sq) = updates.adds[0];
                let add = top_feature_idx(player, add_piece.player(), add_piece.piece_type(), add_sq);

                Self::add(&NET.ftw_top, dst, src, add);

                add_idx += 1;
            }
            (true, false) => {
                let (sub_piece, sub_sq) = updates.subs[0];
                let sub = top_feature_idx(player, sub_piece.player(), sub_piece.piece_type(), sub_sq);

                Self::sub(&NET.ftw_top, dst, src, sub);

                sub_idx += 1;
            }
            _ => {}
        }

        while add_idx < updates.adds.len() {
            let (add_piece, add_sq) = updates.adds[add_idx];
            let add = top_feature_idx(player, add_piece.player(), add_piece.piece_type(), add_sq);

            Self::add_in_place(&NET.ftw_top, dst, add);

            add_idx += 1;
        }

        while sub_idx < updates.subs.len() {
            let (sub_piece, sub_sq) = updates.subs[sub_idx];
            let sub = top_feature_idx(player, sub_piece.player(), sub_piece.piece_type(), sub_sq);

            Self::sub_in_place(&NET.ftw_top, dst, sub);

            sub_idx += 1;
        }
    }

    pub fn apply_updates(&mut self, src: &Self, player: Player) {
        self.apply_top_updates(src, player);

        let acc = &mut self.values[player.idx()];
        let updates = &self.ctx.updates;

        for &(sq, height_before, height_after, mut players_before, mut players_after) in updates.stacks.iter() {
            assert_ne!(height_before, height_after);

            if height_before <= 1 && height_after <= 1 {
                continue;
            }

            let height_before = height_before.min(13).saturating_sub(1);
            let height_after = height_after.min(13).saturating_sub(1);

            players_before >>= 1;
            players_after >>= 1;

            let mut depth = 0;

            while depth < height_before.min(height_after) {
                let before = Player::from_raw((players_before & 0x1) as u8).unwrap();
                let after = Player::from_raw((players_after & 0x1) as u8).unwrap();

                players_before >>= 1;
                players_after >>= 1;

                if before == after {
                    depth += 1;
                    continue;
                }

                let sub = stack_feature_idx(player, before, depth, sq);
                let add = stack_feature_idx(player, after, depth, sq);

                Self::add_sub_in_place(&NET.ftw_stack, acc, add, sub);

                depth += 1;
            }

            if height_before > height_after {
                while depth < height_before {
                    let before = Player::from_raw((players_before & 0x1) as u8).unwrap();
                    players_before >>= 1;
                    let sub = stack_feature_idx(player, before, depth, sq);
                    Self::sub_in_place(&NET.ftw_stack, acc, sub);
                    depth += 1;
                }
            } else {
                while depth < height_after {
                    let after = Player::from_raw((players_after & 0x1) as u8).unwrap();
                    players_after >>= 1;
                    let add = stack_feature_idx(player, after, depth, sq);
                    Self::add_in_place(&NET.ftw_stack, acc, add);
                    depth += 1;
                }
            }
        }
    }

    fn add_sub(w: &[[i16; L1_SIZE]], dst: &mut [i16; L1_SIZE], src: &[i16; L1_SIZE], add: usize, sub: usize) {
        for (((dst, src), add), sub) in dst.iter_mut().zip(src).zip(&w[add]).zip(&w[sub]) {
            *dst = *src + *add - *sub;
        }
    }

    fn add(w: &[[i16; L1_SIZE]], dst: &mut [i16; L1_SIZE], src: &[i16; L1_SIZE], add: usize) {
        for ((dst, src), add) in dst.iter_mut().zip(src).zip(&w[add]) {
            *dst = *src + *add;
        }
    }

    fn sub(w: &[[i16; L1_SIZE]], dst: &mut [i16; L1_SIZE], src: &[i16; L1_SIZE], sub: usize) {
        for ((dst, src), sub) in dst.iter_mut().zip(src).zip(&w[sub]) {
            *dst = *src - *sub;
        }
    }

    fn add_sub_in_place(w: &[[i16; L1_SIZE]], v: &mut [i16; L1_SIZE], add: usize, sub: usize) {
        for ((v, add), sub) in v.iter_mut().zip(&w[add]).zip(&w[sub]) {
            *v += *add - *sub;
        }
    }

    fn add_in_place(w: &[[i16; L1_SIZE]], v: &mut [i16; L1_SIZE], add: usize) {
        for (v, add) in v.iter_mut().zip(&w[add]) {
            *v += *add;
        }
    }

    fn sub_in_place(w: &[[i16; L1_SIZE]], v: &mut [i16; L1_SIZE], sub: usize) {
        for (v, sub) in v.iter_mut().zip(&w[sub]) {
            *v -= *sub;
        }
    }
}

impl Default for Accumulator {
    fn default() -> Self {
        Self {
            values: [[0; _]; _],
            ctx: Default::default(),
            dirty: false,
        }
    }
}
