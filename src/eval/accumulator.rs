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

use crate::core::{Color, Square, Transform, normalizer};
use crate::eval::nnue::*;
use crate::position::Position;

#[derive(Clone)]
#[repr(align(64))]
pub(super) struct Accumulator {
    pub values: [[i16; L1_SIZE]; Color::COUNT],
    pub ctx: UpdateContext,
    dirty: [bool; Color::COUNT],
}

impl Accumulator {
    pub fn reset(&mut self, pos: &Position, perspective: Color) {
        self.values[perspective] = NET.ftb;

        let transform = normalizer(pos.cap_sq(perspective));

        let mut activate_top = |feature: usize| {
            let v = &mut self.values[perspective];
            let w = &NET.ftw_top[feature];
            for (v, w) in v.iter_mut().zip(w) {
                *v += *w;
            }
        };

        for side in [Color::White, Color::Black] {
            for sq in pos.color_bb(side) {
                let perspective_sq = sq.transform(transform);
                let pt = pos.top_pt(sq).unwrap();
                let feature = top_feature_idx(perspective, side, pt, perspective_sq);
                activate_top(feature);
            }
        }

        let mut activate_stack = |feature: usize| {
            let v = &mut self.values[perspective];
            let w = &NET.ftw_stack[feature];
            for (v, w) in v.iter_mut().zip(w) {
                *v += *w;
            }
        };

        for sq in pos.occ() {
            let height = pos.height(sq);

            if height == 1 {
                continue;
            }

            let top = pos.top(sq).unwrap();
            let perspective_sq = sq.transform(transform);

            let mut colors = pos.colors(sq) >> 1;
            let count = height.min(13) - 1;

            for depth in 0..count {
                let color = Color::from_raw((colors & 0x1) as u8).unwrap();
                let feature = stack_feature_idx(perspective, top, color, depth, perspective_sq);
                activate_stack(feature);
                colors >>= 1;
            }
        }

        let mut activate_reserves = |feature: usize| {
            let v = &mut self.values[perspective];
            let w = &NET.ftw_reserves[feature];
            for (v, w) in v.iter_mut().zip(w) {
                *v += *w;
            }
        };

        for color in [Color::White, Color::Black] {
            let reserves = pos.flats_in_hand(color);

            if reserves == 0 {
                continue;
            }

            let feature = reserves_feature_idx(perspective, color, reserves);
            activate_reserves(feature);
        }

        self.set_updated(perspective);
    }

    pub fn reset_both(&mut self, pos: &Position) {
        self.reset(pos, Color::White);
        self.reset(pos, Color::Black);
    }

    pub fn set_dirty(&mut self) {
        self.dirty.fill(true);
    }

    pub fn set_updated(&mut self, color: Color) {
        self.dirty[color] = false;
    }

    #[must_use]
    pub fn is_dirty(&self, color: Color) -> bool {
        self.dirty[color]
    }

    fn apply_top_updates(&mut self, src: &Self, perspective: Color, transform: Transform) {
        let map_sq = |sq: Square| sq.transform(transform);

        let dst = &mut self.values[perspective];
        let src = &src.values[perspective];

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
                let add = top_feature_idx(perspective, add_piece.color(), add_piece.piece_type(), map_sq(add_sq));

                let (sub_piece, sub_sq) = updates.subs[0];
                let sub = top_feature_idx(perspective, sub_piece.color(), sub_piece.piece_type(), map_sq(sub_sq));

                Self::add_sub(&NET.ftw_top, dst, src, add, sub);

                add_idx += 1;
                sub_idx += 1;
            }
            (false, true) => {
                let (add_piece, add_sq) = updates.adds[0];
                let add = top_feature_idx(perspective, add_piece.color(), add_piece.piece_type(), map_sq(add_sq));

                Self::add(&NET.ftw_top, dst, src, add);

                add_idx += 1;
            }
            (true, false) => {
                let (sub_piece, sub_sq) = updates.subs[0];
                let sub = top_feature_idx(perspective, sub_piece.color(), sub_piece.piece_type(), map_sq(sub_sq));

                Self::sub(&NET.ftw_top, dst, src, sub);

                sub_idx += 1;
            }
            _ => {}
        }

        while add_idx < updates.adds.len() {
            let (add_piece, add_sq) = updates.adds[add_idx];
            let add = top_feature_idx(perspective, add_piece.color(), add_piece.piece_type(), map_sq(add_sq));

            Self::add_in_place(&NET.ftw_top, dst, add);

            add_idx += 1;
        }

        while sub_idx < updates.subs.len() {
            let (sub_piece, sub_sq) = updates.subs[sub_idx];
            let sub = top_feature_idx(perspective, sub_piece.color(), sub_piece.piece_type(), map_sq(sub_sq));

            Self::sub_in_place(&NET.ftw_top, dst, sub);

            sub_idx += 1;
        }
    }

    fn apply_stack_updates(&mut self, perspective: Color, transform: Transform) {
        let acc = &mut self.values[perspective];
        let updates = &self.ctx.updates;

        for &(sq, top_before, top_after, height_before, height_after, mut colors_before, mut colors_after) in
            updates.stacks.iter()
        {
            assert_ne!(height_before, height_after);

            if height_before <= 1 && height_after <= 1 {
                continue;
            }

            let perspective_sq = sq.transform(transform);

            let height_before = height_before.min(13).saturating_sub(1);
            let height_after = height_after.min(13).saturating_sub(1);

            colors_before >>= 1;
            colors_after >>= 1;

            // top changed, must refresh
            if top_before != top_after {
                let mut depth = 0;

                while depth < height_before.min(height_after) {
                    let before = Color::from_raw((colors_before & 0x1) as u8).unwrap();
                    let after = Color::from_raw((colors_after & 0x1) as u8).unwrap();

                    colors_before >>= 1;
                    colors_after >>= 1;

                    let sub = stack_feature_idx(perspective, top_before.unwrap(), before, depth, perspective_sq);
                    let add = stack_feature_idx(perspective, top_after.unwrap(), after, depth, perspective_sq);

                    Self::add_sub_in_place(&NET.ftw_stack, acc, add, sub);

                    depth += 1;
                }

                while depth < height_before {
                    let before = Color::from_raw((colors_before & 0x1) as u8).unwrap();
                    colors_before >>= 1;
                    let sub = stack_feature_idx(perspective, top_before.unwrap(), before, depth, perspective_sq);
                    Self::sub_in_place(&NET.ftw_stack, acc, sub);
                    depth += 1;
                }

                while depth < height_after {
                    let after = Color::from_raw((colors_after & 0x1) as u8).unwrap();
                    colors_after >>= 1;
                    let add = stack_feature_idx(perspective, top_after.unwrap(), after, depth, perspective_sq);
                    Self::add_in_place(&NET.ftw_stack, acc, add);
                    depth += 1;
                }

                continue;
            }

            let top = top_before.unwrap();

            let mut depth = 0;

            while depth < height_before.min(height_after) {
                let before = Color::from_raw((colors_before & 0x1) as u8).unwrap();
                let after = Color::from_raw((colors_after & 0x1) as u8).unwrap();

                colors_before >>= 1;
                colors_after >>= 1;

                if before == after {
                    depth += 1;
                    continue;
                }

                let sub = stack_feature_idx(perspective, top, before, depth, perspective_sq);
                let add = stack_feature_idx(perspective, top, after, depth, perspective_sq);

                Self::add_sub_in_place(&NET.ftw_stack, acc, add, sub);

                depth += 1;
            }

            if height_before > height_after {
                while depth < height_before {
                    let before = Color::from_raw((colors_before & 0x1) as u8).unwrap();
                    colors_before >>= 1;
                    let sub = stack_feature_idx(perspective, top, before, depth, perspective_sq);
                    Self::sub_in_place(&NET.ftw_stack, acc, sub);
                    depth += 1;
                }
            } else {
                while depth < height_after {
                    let after = Color::from_raw((colors_after & 0x1) as u8).unwrap();
                    colors_after >>= 1;
                    let add = stack_feature_idx(perspective, top, after, depth, perspective_sq);
                    Self::add_in_place(&NET.ftw_stack, acc, add);
                    depth += 1;
                }
            }
        }
    }

    fn apply_reserve_updates(&mut self, perspective: Color) {
        let acc = &mut self.values[perspective];

        for color in [Color::White, Color::Black] {
            let [reserves_before, reserves_after] = self.ctx.reserves[color];

            if reserves_before == reserves_after {
                continue;
            }

            assert!(reserves_before > reserves_after);

            if reserves_after == 0 {
                let sub = reserves_feature_idx(perspective, color, reserves_before);
                Self::sub_in_place(&NET.ftw_reserves, acc, sub);
            } else {
                let sub = reserves_feature_idx(perspective, color, reserves_before);
                let add = reserves_feature_idx(perspective, color, reserves_after);
                Self::add_sub_in_place(&NET.ftw_reserves, acc, add, sub);
            }
        }
    }

    pub fn apply_updates(&mut self, src: &Self, perspective: Color) {
        let transform = normalizer(self.ctx.caps[perspective]);

        self.apply_top_updates(src, perspective, transform);
        self.apply_stack_updates(perspective, transform);
        self.apply_reserve_updates(perspective);

        self.set_updated(perspective);
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
            dirty: [false; _],
        }
    }
}
