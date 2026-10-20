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
use arrayvec::ArrayVec;

#[derive(Clone)]
#[repr(align(64))]
pub(super) struct Accumulator {
    pub values: [[i16; L1_SIZE]; Color::COUNT],
    pub ctx: UpdateContext,
    dirty: [bool; Color::COUNT],
}

impl Accumulator {
    pub fn reset(&mut self, pos: &Position, perspective: Color) {
        let transform = normalizer(pos.cap_sq(perspective));

        let mut rows = Updates::default();

        for side in [Color::White, Color::Black] {
            for sq in pos.color_bb(side) {
                let perspective_sq = sq.transform(transform);
                let pt = pos.top_pt(sq).unwrap();
                rows.adds
                    .push(&NET.ftw_top[top_feature_idx(perspective, side, pt, perspective_sq)]);
            }
        }

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
                rows.adds
                    .push(&NET.ftw_stack[stack_feature_idx(perspective, top, color, depth, perspective_sq)]);
                colors >>= 1;
            }
        }

        for color in [Color::White, Color::Black] {
            let reserves = pos.flats_in_hand(color);

            if reserves == 0 {
                continue;
            }

            rows.adds
                .push(&NET.ftw_reserves[reserves_feature_idx(perspective, color, reserves)]);
        }

        rows.apply(&mut self.values[perspective], &NET.ftb);

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

    fn collect_top_updates(&self, perspective: Color, transform: Transform, rows: &mut Updates) {
        let map_sq = |sq: Square| sq.transform(transform);
        let updates = &self.ctx.updates;

        for &(piece, sq) in updates.adds.iter() {
            rows.adds
                .push(&NET.ftw_top[top_feature_idx(perspective, piece.color(), piece.piece_type(), map_sq(sq))]);
        }

        for &(piece, sq) in updates.subs.iter() {
            rows.subs
                .push(&NET.ftw_top[top_feature_idx(perspective, piece.color(), piece.piece_type(), map_sq(sq))]);
        }
    }

    fn collect_stack_updates(&self, perspective: Color, transform: Transform, rows: &mut Updates) {
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
                for depth in 0..height_before {
                    let before = Color::from_raw((colors_before & 0x1) as u8).unwrap();
                    colors_before >>= 1;
                    rows.subs.push(
                        &NET.ftw_stack
                            [stack_feature_idx(perspective, top_before.unwrap(), before, depth, perspective_sq)],
                    );
                }

                for depth in 0..height_after {
                    let after = Color::from_raw((colors_after & 0x1) as u8).unwrap();
                    colors_after >>= 1;
                    rows.adds.push(
                        &NET.ftw_stack
                            [stack_feature_idx(perspective, top_after.unwrap(), after, depth, perspective_sq)],
                    );
                }

                continue;
            }

            let top = top_before.unwrap();

            for depth in 0..height_before.max(height_after) {
                let before = Color::from_raw((colors_before & 0x1) as u8).unwrap();
                let after = Color::from_raw((colors_after & 0x1) as u8).unwrap();

                colors_before >>= 1;
                colors_after >>= 1;

                let in_before = depth < height_before;
                let in_after = depth < height_after;

                if in_before && in_after && before == after {
                    continue;
                }

                if in_before {
                    rows.subs
                        .push(&NET.ftw_stack[stack_feature_idx(perspective, top, before, depth, perspective_sq)]);
                }

                if in_after {
                    rows.adds
                        .push(&NET.ftw_stack[stack_feature_idx(perspective, top, after, depth, perspective_sq)]);
                }
            }
        }
    }

    fn collect_reserve_updates(&self, perspective: Color, rows: &mut Updates) {
        for color in [Color::White, Color::Black] {
            let [reserves_before, reserves_after] = self.ctx.reserves[color];

            if reserves_before == reserves_after {
                continue;
            }

            assert!(reserves_before > reserves_after);

            rows.subs
                .push(&NET.ftw_reserves[reserves_feature_idx(perspective, color, reserves_before)]);

            if reserves_after != 0 {
                rows.adds
                    .push(&NET.ftw_reserves[reserves_feature_idx(perspective, color, reserves_after)]);
            }
        }
    }

    pub fn apply_updates(&mut self, src: &Self, perspective: Color) {
        let transform = normalizer(self.ctx.caps[perspective]);

        let mut rows = Updates::default();
        self.collect_top_updates(perspective, transform, &mut rows);
        self.collect_stack_updates(perspective, transform, &mut rows);
        self.collect_reserve_updates(perspective, &mut rows);

        rows.apply(&mut self.values[perspective], &src.values[perspective]);

        self.set_updated(perspective);
    }
}

#[cfg(target_feature = "avx512f")]
const UPDATE_CHUNK: usize = L1_SIZE;
#[cfg(not(target_feature = "avx512f"))]
const UPDATE_CHUNK: usize = 256;
const MAX_UPDATE_ROWS: usize = 128;

#[derive(Default)]
struct Updates {
    adds: ArrayVec<&'static [i16; L1_SIZE], MAX_UPDATE_ROWS>,
    subs: ArrayVec<&'static [i16; L1_SIZE], MAX_UPDATE_ROWS>,
}

impl Updates {
    fn apply(&self, dst: &mut [i16; L1_SIZE], src: &[i16; L1_SIZE]) {
        let paired = self.adds.len().min(self.subs.len());

        for offset in (0..L1_SIZE).step_by(UPDATE_CHUNK) {
            let mut chunk: [i16; UPDATE_CHUNK] = src[offset..offset + UPDATE_CHUNK].try_into().unwrap();

            for (&add, &sub) in self.adds.iter().zip(self.subs.iter()) {
                for ((v, add), sub) in chunk.iter_mut().zip(&add[offset..]).zip(&sub[offset..]) {
                    *v += *add - *sub;
                }
            }

            for &add in &self.adds[paired..] {
                for (v, add) in chunk.iter_mut().zip(&add[offset..]) {
                    *v += *add;
                }
            }

            for &sub in &self.subs[paired..] {
                for (v, sub) in chunk.iter_mut().zip(&sub[offset..]) {
                    *v -= *sub;
                }
            }

            dst[offset..offset + UPDATE_CHUNK].copy_from_slice(&chunk);
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
