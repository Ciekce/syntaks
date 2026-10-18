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

use super::forward::forward;
use crate::core::{Color, Piece, PieceType, Square, normalizer};
use crate::eval::accumulator::Accumulator;
use crate::position::{BoardObserver, Position};
use arrayvec::ArrayVec;

cfg_select! {
    all(target_feature = "avx512f", target_feature = "avx512bw") => {
        #[path = "simd/avx512.rs"]
        pub(super) mod simd;
    }
    target_feature = "avx2" => {
        #[path = "simd/avx2.rs"]
        pub(super) mod simd;
    }
    target_feature = "sse4.1" => {
        #[path = "simd/sse41.rs"]
        pub(super) mod simd;
    }
    all(target_arch = "aarch64", target_feature = "neon") => {
        #[path = "simd/neon.rs"]
        pub(super) mod simd;
    }
    _ => {
        #[path = "simd/scalar.rs"]
        pub(super) mod simd;
    }
}

pub const TOP_INPUT_SIZE: usize = Color::COUNT * PieceType::COUNT * Square::COUNT;
pub const STACK_INPUT_SIZE: usize = 12 * Color::COUNT * PieceType::COUNT * Square::COUNT * Color::COUNT;
pub const RESERVES_INPUT_SIZE: usize = 30 * Color::COUNT;

pub const L1_SIZE: usize = 768;
pub const L2_SIZE: usize = 32;
pub const L3_SIZE: usize = 32;

pub const L1_PAIR_COUNT: usize = L1_SIZE / 2;
pub const L2_PAIR_COUNT: usize = L2_SIZE / 2;

pub const OUTPUT_BUCKET_COUNT: usize = 16;

pub const Q0: i32 = 255;
pub const Q: i32 = 64;

pub const Q0_BITS: i32 = 8;
pub const Q1_BITS: i32 = 7;
pub const Q_BITS: i32 = 6;

pub const SCALE: i32 = 400;

#[repr(C, align(64))]
pub(super) struct Network {
    pub ftw_top: [[i16; L1_SIZE]; TOP_INPUT_SIZE],
    pub ftw_stack: [[i16; L1_SIZE]; STACK_INPUT_SIZE],
    pub ftw_reserves: [[i16; L1_SIZE]; RESERVES_INPUT_SIZE],
    pub ftb: [i16; L1_SIZE],
    pub l1w: [[i8; L1_SIZE * L2_SIZE]; OUTPUT_BUCKET_COUNT],
    pub l1b: [[i32; L2_SIZE]; OUTPUT_BUCKET_COUNT],
    pub l2w: [[i16; L2_SIZE * L3_SIZE]; OUTPUT_BUCKET_COUNT],
    pub l2b: [[i32; L3_SIZE]; OUTPUT_BUCKET_COUNT],
    pub l3w: [[i32; L3_SIZE]; OUTPUT_BUCKET_COUNT],
    pub l3b: [i32; OUTPUT_BUCKET_COUNT],
}

pub(super) static NET: Network = unsafe { std::mem::transmute(*include_bytes!(env!("EVALFILE"))) };

#[must_use]
pub(super) fn which_output_bucket(pos: &Position) -> usize {
    let w_reserves = pos.flats_in_hand(Color::White);
    let b_reserves = pos.flats_in_hand(Color::Black);
    let min_reserves = w_reserves.min(b_reserves);
    let bucket = [
        0, 0, 1, 2, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7,
    ][min_reserves as usize];
    pos.stm().idx() * 8 + bucket
}

#[must_use]
pub(super) fn evaluate_once(pos: &Position) -> i32 {
    let mut acc = Accumulator::default();
    acc.reset_both(pos);
    forward(&acc, pos)
}

pub(super) fn top_feature_idx(perspective: Color, side: Color, pt: PieceType, sq: Square) -> usize {
    (usize::from(side != perspective) * PieceType::COUNT + pt.idx()) * Square::COUNT + sq.idx()
}

pub(super) fn stack_feature_idx(perspective: Color, top: Piece, side: Color, depth: u8, sq: Square) -> usize {
    debug_assert!(depth < 12);
    let top_idx = usize::from(top.color() != perspective) * PieceType::COUNT + top.piece_type().idx();
    ((top_idx * Color::COUNT + usize::from(side != perspective)) * Square::COUNT + sq.idx()) * 12 + depth as usize
}

pub(super) fn reserves_feature_idx(perspective: Color, side: Color, reserves: u8) -> usize {
    usize::from(side != perspective) * 30 + (reserves as usize - 1)
}

#[derive(Clone, Debug, Default)]
pub(super) struct NnueUpdates {
    pub adds: ArrayVec<(Piece, Square), 6>,
    pub subs: ArrayVec<(Piece, Square), 6>,
    pub stacks: ArrayVec<(Square, Option<Piece>, Option<Piece>, u8, u8, u32, u32), 6>,
    pub refresh: [bool; Color::COUNT],
}

impl NnueUpdates {
    #[must_use]
    pub fn requires_refresh(&self, color: Color) -> bool {
        self.refresh[color]
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct UpdateContext {
    pub updates: NnueUpdates,
    pub caps: [Option<Square>; Color::COUNT],
    pub reserves: [[u8; 2]; Color::COUNT],
}

pub struct NnueObserver<'a> {
    ctx: &'a mut UpdateContext,
}

impl<'a> NnueObserver<'a> {
    pub(super) fn new(ctx: &'a mut UpdateContext) -> Self {
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

    fn stack_changed(
        &mut self,
        _pos: &Position,
        sq: Square,
        top_before: Option<Piece>,
        top_after: Option<Piece>,
        height_before: u8,
        height_after: u8,
        colors_before: u64,
        colors_after: u64,
    ) {
        self.ctx.updates.stacks.push((
            sq,
            top_before,
            top_after,
            height_before,
            height_after,
            colors_before as u32,
            colors_after as u32,
        ));
    }

    fn finalize(&mut self, pos_before: &Position, pos_after: &Position) {
        let should_refresh = |side: Color| normalizer(pos_before.cap_sq(side)) != normalizer(pos_after.cap_sq(side));
        self.ctx.updates.refresh = [Color::White, Color::Black].map(should_refresh);

        let w_cap = pos_after.color_piece_bb(Piece::WhiteCapstone).lsb();
        let b_cap = pos_after.color_piece_bb(Piece::BlackCapstone).lsb();
        self.ctx.caps = [w_cap, b_cap];

        self.ctx.reserves =
            [Color::White, Color::Black].map(|p| [pos_before.flats_in_hand(p), pos_after.flats_in_hand(p)]);
    }
}
