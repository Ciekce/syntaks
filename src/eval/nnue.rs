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
use crate::board::{BoardObserver, Position};
use crate::core::{Piece, PieceType, Player, Square};
use crate::eval::accumulator::Accumulator;
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
    _ => {
        compiler_error!("No supported SIMD arch");
    }
}

pub const L1_SIZE: usize = 256;

pub const OUTPUT_BUCKETS: usize = 2;

pub const FT_Q: i32 = 255;
pub const L1_Q: i32 = 64;

pub const SCALE: i32 = 400;

#[repr(C, align(64))]
pub(super) struct Network {
    pub ftw: [[i16; L1_SIZE]; 216],
    pub ftb: [i16; L1_SIZE],
    pub l1w: [[[i16; L1_SIZE]; 2]; OUTPUT_BUCKETS],
    pub l1b: [i16; OUTPUT_BUCKETS],
}

pub(super) static NET: Network = unsafe { std::mem::transmute(*include_bytes!(env!("EVALFILE"))) };

#[must_use]
pub(super) fn evaluate_once(pos: &Position) -> i32 {
    let mut acc = Accumulator::default();
    acc.reset_both(pos);
    forward(&acc, pos.stm())
}

pub(super) fn feature_idx(perspective: Player, side: Player, pt: PieceType, sq: Square) -> usize {
    // TODO: was tired and got side and piecetype backwards
    pt.idx() * 72 + usize::from(side != perspective) * 36 + sq.idx()
}

#[derive(Clone, Debug, Default)]
pub(super) struct NnueUpdates {
    pub adds: ArrayVec<(Piece, Square), 6>,
    pub subs: ArrayVec<(Piece, Square), 6>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct UpdateContext {
    pub updates: NnueUpdates,
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

    fn finalize(&mut self, _pos: &Position) {
        //
    }
}
