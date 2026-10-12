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

pub const TOP_INPUT_SIZE: usize = Player::COUNT * PieceType::COUNT * Square::COUNT;
pub const STACK_INPUT_SIZE: usize = 12 * Player::COUNT * PieceType::COUNT * Square::COUNT * Player::COUNT;

pub const L1_SIZE: usize = 64;

pub const OUTPUT_BUCKETS: usize = 2;

pub const FT_Q: i32 = 255;
pub const L1_Q: i32 = 64;

pub const SCALE: i32 = 400;

#[repr(C, align(64))]
pub(super) struct Network {
    pub ftw_top: [[i16; L1_SIZE]; TOP_INPUT_SIZE],
    pub ftw_stack: [[i16; L1_SIZE]; STACK_INPUT_SIZE],
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

pub(super) fn should_flip(cap_sq: Option<Square>) -> (bool, bool) {
    if let Some(cap) = cap_sq {
        (cap.rank() >= 3, cap.file() >= 3)
    } else {
        (false, false)
    }
}

pub(super) fn top_feature_idx(perspective: Player, side: Player, pt: PieceType, sq: Square) -> usize {
    (usize::from(side != perspective) * PieceType::COUNT + pt.idx()) * Square::COUNT + sq.idx()
}

pub(super) fn stack_feature_idx(perspective: Player, top: Piece, side: Player, depth: u8, sq: Square) -> usize {
    debug_assert!(depth < 12);
    let top_idx = usize::from(top.player() != perspective) * PieceType::COUNT + top.piece_type().idx();
    ((top_idx * Player::COUNT + usize::from(side != perspective)) * Square::COUNT + sq.idx()) * 12 + depth as usize
}

#[derive(Clone, Debug, Default)]
pub(super) struct NnueUpdates {
    pub adds: ArrayVec<(Piece, Square), 6>,
    pub subs: ArrayVec<(Piece, Square), 6>,
    pub stacks: ArrayVec<(Square, Option<Piece>, Option<Piece>, u8, u8, u32, u32), 6>,
    pub refresh: [bool; Player::COUNT],
}

impl NnueUpdates {
    #[must_use]
    pub fn requires_refresh(&self, player: Player) -> bool {
        self.refresh[player.idx()]
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct UpdateContext {
    pub updates: NnueUpdates,
    pub caps: [Option<Square>; Player::COUNT],
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
        players_before: u64,
        players_after: u64,
    ) {
        self.ctx.updates.stacks.push((
            sq,
            top_before,
            top_after,
            height_before,
            height_after,
            players_before as u32,
            players_after as u32,
        ));
    }

    fn finalize(&mut self, pos_before: &Position, pos_after: &Position) {
        let should_refresh = |side: Player| {
            let cap_before = pos_before.cap_sq(side);
            let cap_after = pos_after.cap_sq(side);

            debug_assert!(cap_before.is_none() || cap_after.is_some());

            if cap_after.is_none() {
                return false;
            }

            let cap_after = cap_after.unwrap();

            let flip_rank_after = cap_after.rank() >= 3;
            let flip_file_after = cap_after.file() >= 3;

            if cap_before.is_none() {
                return flip_rank_after || flip_file_after;
            }

            let cap_before = cap_before.unwrap();

            let flip_rank_before = cap_before.rank() >= 3;
            let flip_file_before = cap_before.file() >= 3;

            (flip_rank_before != flip_rank_after) || (flip_file_before != flip_file_after)
        };

        self.ctx.updates.refresh = [Player::P1, Player::P2].map(should_refresh);

        let p1_cap = pos_after.player_piece_bb(Piece::P1Capstone).lsb();
        let p2_cap = pos_after.player_piece_bb(Piece::P2Capstone).lsb();
        self.ctx.caps = [p1_cap, p2_cap];
    }
}
