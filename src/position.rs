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
use crate::core::*;
use crate::hits::find_hit_for_dir;
use crate::keys;
use crate::road::influence;
use crate::takmove::Move;
use std::cmp::Ordering;
use std::str::FromStr;
use std::sync::atomic;
use std::sync::atomic::AtomicBool;

pub const TOTAL_FLATS: u8 = 30;
pub const TOTAL_CAPS: u8 = 1;

pub static DOUBLE_BLACK_STACK: AtomicBool = AtomicBool::new(false);

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
struct Keys {
    stacks: u64,
    blockers: u64,
    roads: u64,
    tops: u64,
}

impl Keys {
    fn reset(&mut self) {
        *self = Default::default();
    }

    fn toggle_top_key(&mut self, pt: PieceType, sq: Square) {
        self.stacks ^= keys::top_key(pt, sq);
        self.tops ^= keys::top_key(pt, sq);
        if pt.is_blocker() {
            self.blockers ^= keys::top_key(pt, sq);
        }
        if pt.is_road() {
            self.roads ^= keys::top_key(pt, sq);
        }
    }

    fn toggle_color_key(&mut self, height: u8, color: Color, sq: Square) {
        self.stacks ^= keys::color_key(height, color, sq);
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct Stacks {
    colors: [u64; Square::COUNT],
    heights: [u8; Square::COUNT],
    tops: [Option<Piece>; Square::COUNT],
    keys: Keys,
}

impl Stacks {
    #[must_use]
    pub fn is_empty(&self, sq: Square) -> bool {
        self.tops[sq].is_none()
    }

    #[must_use]
    pub fn top(&self, sq: Square) -> Option<Piece> {
        self.tops[sq]
    }

    #[must_use]
    pub fn top_pt(&self, sq: Square) -> Option<PieceType> {
        self.tops[sq].map(Piece::piece_type)
    }

    #[must_use]
    pub fn top_color(&self, sq: Square) -> Option<Color> {
        self.tops[sq].map(Piece::color)
    }

    #[must_use]
    pub fn height(&self, sq: Square) -> u8 {
        self.heights[sq]
    }

    #[must_use]
    pub fn colors(&self, sq: Square) -> u64 {
        self.colors[sq]
    }

    fn push(&mut self, sq: Square, pt: PieceType, color: Color) {
        debug_assert!(self.tops[sq].is_none_or(|piece| piece.piece_type() != PieceType::Capstone));

        if let Some(prev_top) = self.tops[sq] {
            self.keys.toggle_top_key(prev_top.piece_type(), sq);
        }

        self.keys.toggle_top_key(pt, sq);

        let height = self.heights[sq];
        self.keys.toggle_color_key(height, color, sq);

        self.colors[sq] <<= 1;
        self.colors[sq] |= color.raw() as u64;

        self.heights[sq] += 1;

        self.tops[sq] = Some(pt.with_color(color));
    }

    fn take(&mut self, sq: Square, count: u8) -> (u8, Piece, Option<Color>) {
        debug_assert!(count <= self.heights[sq]);
        debug_assert!(count > 0);
        debug_assert!(count <= 6);

        let colors = self.colors[sq] & ((1 << count) - 1);
        let top = self.tops[sq].unwrap();

        self.keys.toggle_top_key(top.piece_type(), sq);

        let height = self.heights[sq];

        for i in 0..count {
            let height = height - i - 1;
            let color = Color::from_raw(((self.colors[sq] >> i) & 0x1) as u8).unwrap();
            self.keys.toggle_color_key(height, color, sq);
        }

        self.colors[sq] >>= count;
        self.heights[sq] -= count;

        if count == height {
            self.tops[sq] = None;
            (colors as u8, top, None)
        } else {
            self.keys.toggle_top_key(PieceType::Flat, sq);
            let new_top_color = Color::from_raw((self.colors[sq] & 0x1) as u8).unwrap();
            self.tops[sq] = Some(PieceType::Flat.with_color(new_top_color));
            (colors as u8, top, Some(new_top_color))
        }
    }

    fn regen_key(&mut self, occ: Bitboard) {
        self.keys.reset();

        for sq in occ {
            let mut colors = self.colors(sq);
            let height = self.height(sq);
            let top_pt = self.top_pt(sq).unwrap();

            self.keys.toggle_top_key(top_pt, sq);

            for i in 0..height {
                let height = height - i - 1;
                let color = Color::from_raw((colors & 0x1) as u8).unwrap();
                colors >>= 1;
                self.keys.toggle_color_key(height, color, sq);
            }
        }
    }
}

impl Default for Stacks {
    fn default() -> Self {
        Self {
            colors: [Default::default(); _],
            heights: [Default::default(); _],
            tops: [Default::default(); _],
            keys: Default::default(),
        }
    }
}

pub enum FlatCountOutcome {
    None,
    Draw,
    Win(Color),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Position {
    stacks: Stacks,
    colors: [Bitboard; Color::COUNT],
    pieces: [Bitboard; PieceType::COUNT],
    flats_in_hand: [u8; Color::COUNT],
    caps_in_hand: [u8; Color::COUNT],
    stm: Color,
    ply: u16,
    stm_key: u64,
    roads: [bool; Color::COUNT],
    influence: [[Bitboard; 4]; Color::COUNT],
}

impl Position {
    pub const CARRY_LIMIT: u32 = 6;
    pub const KOMI: u32 = 0;

    // all flats + cap
    pub const MAX_STACK_HEIGHT: usize = TOTAL_FLATS as usize * 2 + TOTAL_CAPS as usize;

    #[must_use]
    pub fn startpos() -> Self {
        Self {
            stacks: Stacks::default(),
            colors: [Bitboard::empty(); _],
            pieces: [Bitboard::empty(); _],
            flats_in_hand: [TOTAL_FLATS; _],
            caps_in_hand: [TOTAL_CAPS; _],
            stm: Color::White,
            ply: 0,
            stm_key: 0,
            roads: Default::default(),
            influence: Default::default(),
        }
    }

    pub fn from_tps_parts(parts: &[&str]) -> Result<Self, TpsError> {
        if parts.len() < 2 || parts.len() > 3 {
            return Err(TpsError::WrongNumberOfParts);
        }

        let ranks: Vec<&str> = parts[0].split('/').collect();
        if ranks.len() != 6 {
            return Err(TpsError::WrongNumberOfRanks);
        }

        let mut pos = Self::startpos();

        for rank_idx in 0..6 {
            let mut file_idx = 0;

            for stack in ranks[5 - rank_idx as usize].split(',') {
                if file_idx >= 6 {
                    return Err(TpsError::WrongNumberOfFiles);
                }

                if stack.is_empty() {
                    return Err(TpsError::BlankFile);
                }

                let mut chars = stack.chars();

                if chars.next().unwrap() == 'x' {
                    let remaining = chars.as_str();
                    if !remaining.is_empty() {
                        match remaining.parse::<u32>() {
                            Ok(empty) => file_idx += empty,
                            Err(_) => return Err(TpsError::InvalidEmptyFileCount),
                        }
                    } else {
                        file_idx += 1;
                    }
                } else {
                    let sq = Square::from_file_rank(file_idx, rank_idx).unwrap();

                    let mut colors = Vec::with_capacity(stack.len());
                    let mut top = None;

                    for c in stack.chars() {
                        if top.is_some() {
                            return Err(TpsError::ExcessCharsAfterStackTop);
                        }

                        match c {
                            '1' => colors.push(Color::White),
                            '2' => colors.push(Color::Black),
                            'F' => top = Some(PieceType::Flat), // nonstandard but why not
                            'S' => top = Some(PieceType::Wall),
                            'C' => top = Some(PieceType::Capstone),
                            _ => return Err(TpsError::InvalidCharInStack),
                        }
                    }

                    let top = top.unwrap_or(PieceType::Flat);

                    for (idx, &color) in colors.iter().enumerate() {
                        if idx == colors.len() - 1 {
                            pos.stacks.push(sq, top, color);
                        } else {
                            pos.stacks.push(sq, PieceType::Flat, color);
                        }
                    }

                    file_idx += 1;
                }
            }

            if file_idx > 6 {
                return Err(TpsError::WrongNumberOfFiles);
            }
        }

        match parts[1] {
            "1" => pos.stm = Color::White,
            "2" => pos.stm = Color::Black,
            _ => return Err(TpsError::InvalidStm),
        }

        if parts.len() >= 3 {
            match parts[2].parse::<u16>() {
                Ok(fullmove) => pos.ply = (fullmove.max(1) - 1) * 2 + if pos.stm == Color::Black { 1 } else { 0 },
                Err(_) => return Err(TpsError::InvalidFullmove),
            }
        }

        pos.regen();

        Ok(pos)
    }

    #[must_use]
    pub fn stm(&self) -> Color {
        self.stm
    }

    #[must_use]
    #[inline(always)]
    pub fn is_empty(&self, sq: Square) -> bool {
        self.stacks.is_empty(sq)
    }

    #[must_use]
    #[inline(always)]
    pub fn top(&self, sq: Square) -> Option<Piece> {
        self.stacks.top(sq)
    }

    #[must_use]
    #[inline(always)]
    pub fn top_pt(&self, sq: Square) -> Option<PieceType> {
        self.stacks.top_pt(sq)
    }

    #[must_use]
    #[inline(always)]
    pub fn top_color(&self, sq: Square) -> Option<Color> {
        self.stacks.top_color(sq)
    }

    #[must_use]
    #[inline(always)]
    pub fn height(&self, sq: Square) -> u8 {
        self.stacks.height(sq)
    }

    #[must_use]
    #[inline(always)]
    pub fn colors(&self, sq: Square) -> u64 {
        self.stacks.colors(sq)
    }

    #[must_use]
    #[inline(always)]
    pub fn colors_for(&self, sq: Square, color: Color) -> u64 {
        let colors = self.colors(sq);
        let colors = if color == Color::White { !colors } else { colors };
        colors & ((1u64 << self.height(sq)) - 1)
    }

    #[must_use]
    pub fn color_bb(&self, color: Color) -> Bitboard {
        self.colors[color]
    }

    #[must_use]
    pub fn piece_bb(&self, pt: PieceType) -> Bitboard {
        self.pieces[pt]
    }

    #[must_use]
    pub fn color_piece_bb(&self, piece: Piece) -> Bitboard {
        self.color_bb(piece.color()) & self.piece_bb(piece.piece_type())
    }

    #[must_use]
    pub fn occ(&self) -> Bitboard {
        self.colors[0] | self.colors[1]
    }

    #[must_use]
    pub fn flats_in_hand(&self, color: Color) -> u8 {
        self.flats_in_hand[color]
    }

    #[must_use]
    pub fn caps_in_hand(&self, color: Color) -> u8 {
        self.caps_in_hand[color]
    }

    #[must_use]
    pub fn cap_sq(&self, color: Color) -> Option<Square> {
        self.color_piece_bb(PieceType::Capstone.with_color(color)).lsb()
    }

    #[must_use]
    pub fn ply(&self) -> u16 {
        self.ply
    }

    #[must_use]
    pub fn key(&self) -> u64 {
        self.stm_key ^ self.stacks.keys.stacks
    }

    #[must_use]
    pub fn blocker_key(&self) -> u64 {
        self.stacks.keys.blockers
    }

    #[must_use]
    pub fn road_key(&self) -> u64 {
        self.stacks.keys.roads
    }

    #[must_use]
    pub fn top_key(&self) -> u64 {
        self.stacks.keys.tops
    }

    #[must_use]
    pub fn all_blockers(&self) -> Bitboard {
        self.piece_bb(PieceType::Wall) | self.piece_bb(PieceType::Capstone)
    }

    #[must_use]
    pub fn blockers(&self, color: Color) -> Bitboard {
        self.color_bb(color) & !self.piece_bb(PieceType::Flat)
    }

    #[must_use]
    pub fn all_roads(&self) -> Bitboard {
        self.piece_bb(PieceType::Flat) | self.piece_bb(PieceType::Capstone)
    }

    #[must_use]
    pub fn roads(&self, color: Color) -> Bitboard {
        self.color_bb(color) & !self.piece_bb(PieceType::Wall)
    }

    #[must_use]
    pub fn has_road(&self, color: Color) -> bool {
        self.roads[color]
    }

    #[must_use]
    pub fn influence(&self, color: Color) -> &[Bitboard; 4] {
        &self.influence[color]
    }

    #[must_use]
    fn has_no_more_pieces(&self, color: Color) -> bool {
        self.flats_in_hand(color) == 0 && self.caps_in_hand(color) == 0
    }

    #[must_use]
    pub fn fcd(&self, color: Color) -> i32 {
        let white_advantage = self.color_piece_bb(Piece::WhiteFlat).popcount() as i32
            - self.color_piece_bb(Piece::BlackFlat).popcount() as i32
            - Self::KOMI as i32;
        white_advantage * color.sign()
    }

    #[must_use]
    pub fn count_flats(&self) -> FlatCountOutcome {
        if !(!self.occ()).is_empty() && !self.has_no_more_pieces(Color::White) && !self.has_no_more_pieces(Color::Black)
        {
            return FlatCountOutcome::None;
        }

        let w_flats = self.color_piece_bb(Piece::WhiteFlat).popcount();
        let b_flats = self.color_piece_bb(Piece::BlackFlat).popcount() + Self::KOMI;

        match w_flats.cmp(&b_flats) {
            Ordering::Less => FlatCountOutcome::Win(Color::Black),
            Ordering::Equal => FlatCountOutcome::Draw,
            Ordering::Greater => FlatCountOutcome::Win(Color::White),
        }
    }

    #[must_use]
    pub fn is_legal(&self, mv: Move) -> bool {
        if mv.is_spread() {
            if self.ply < 2 {
                return false;
            }

            if self.stacks.top_color(mv.sq()).is_none_or(|p| p != self.stm) {
                return false;
            }

            let pattern = mv.pattern();

            let taken = 6 - pattern.trailing_zeros();
            if taken > self.stacks.height(mv.sq()) as u32 {
                return false;
            }

            let dist = pattern.count_ones() as u8;
            let (max_dist, hit_sq) = find_hit_for_dir(self.all_blockers(), mv.sq(), mv.dir());

            if dist > max_dist {
                return false;
            } else if dist == max_dist
                && let Some(hit_top) = self.stacks.top_pt(hit_sq)
            {
                match hit_top {
                    PieceType::Flat => {}
                    PieceType::Wall => {
                        // multiple pieces dropped on the final square
                        if pattern & (1 << (Self::CARRY_LIMIT - 1)) == 0 {
                            return false;
                        }

                        let top = self.stacks.top_pt(mv.sq()).unwrap();
                        if top != PieceType::Capstone {
                            return false;
                        }
                    }
                    PieceType::Capstone => return false,
                }
            }
        } else {
            if self.ply < 2 && mv.pt() != PieceType::Flat {
                return false;
            }

            if !self.stacks.is_empty(mv.sq()) {
                return false;
            }

            let reserves = match mv.pt() {
                PieceType::Flat | PieceType::Wall => self.flats_in_hand(self.stm),
                PieceType::Capstone => self.caps_in_hand(self.stm),
            };

            if reserves == 0 {
                return false;
            }
        }

        true
    }

    #[must_use]
    pub fn apply_move_with_observer<O: BoardObserver>(&self, mv: Move, observer: &mut O) -> Self {
        let mut new_pos = *self;

        if mv.is_spread() {
            debug_assert_ne!(self.stacks.top(mv.sq()), None);
            debug_assert_eq!(self.stacks.top_color(mv.sq()), Some(self.stm()));
            debug_assert!(self.ply() >= 2);

            let pattern = mv.pattern();
            let dir = mv.dir();

            let dropped = pattern.trailing_zeros();
            let taken = Self::CARRY_LIMIT - dropped;

            let mut pattern = pattern >> dropped;
            let (colors, top, new_top_color) = new_pos.stacks.take(mv.sq(), taken as u8);

            let mut color_idx = taken - 1;

            let mut new_flats_bb = Bitboard::empty();
            let mut new_color_bbs = [Bitboard::empty(); Color::COUNT];
            let mut dirty_bb = mv.sq().bb();

            if let Some(new_top_color) = new_top_color {
                new_color_bbs[new_top_color].set_sq(mv.sq());
                new_flats_bb.set_sq(mv.sq());
            } else {
                new_pos.colors[self.stm()].toggle_sq(mv.sq());
            }

            if top.piece_type() != PieceType::Flat || new_top_color.is_none() {
                new_pos.pieces[top.piece_type()].toggle_sq(mv.sq());
            }

            let mut sq = mv.sq().shift(dir).unwrap();

            for idx in 0..taken {
                let color = Color::from_raw((colors >> color_idx) & 0x1).unwrap();
                let pt = if idx == taken - 1 {
                    top.piece_type()
                } else {
                    PieceType::Flat
                };

                new_pos.stacks.push(sq, pt, color);
                dirty_bb.set_sq(sq);

                pattern >>= 1;
                color_idx = color_idx.wrapping_sub(1);

                if (pattern & 0x1) != 0 {
                    new_color_bbs[color].set_sq(sq);
                    new_flats_bb.set_sq(sq);

                    sq = sq.shift(dir).unwrap();
                }
            }

            new_color_bbs[self.stm()].set_sq(sq);

            debug_assert_eq!(new_color_bbs[0] & new_color_bbs[1], Bitboard::empty());

            for color in 0..Color::COUNT {
                new_pos.colors[color] = (new_pos.colors[color] | new_color_bbs[color]) & !new_color_bbs[color ^ 0x1];
            }

            new_pos.pieces[top.piece_type()].set_sq(sq);
            new_pos.pieces[PieceType::Flat] |= new_flats_bb;

            if let Some(prev_dst_top_pt) = self.stacks.top_pt(sq)
                && prev_dst_top_pt != top.piece_type()
            {
                new_pos.pieces[prev_dst_top_pt].clear_sq(sq);
                new_pos.pieces[top.piece_type()].set_sq(sq);
            }

            for piece_idx in 0..Piece::COUNT {
                let piece = Piece::from_raw(piece_idx as u8).unwrap();

                let prev_bb = self.color_piece_bb(piece);
                let new_bb = new_pos.color_piece_bb(piece);

                if prev_bb == new_bb {
                    continue;
                }

                for added in new_bb & !prev_bb {
                    observer.top_added(&new_pos, piece, added);
                }

                for removed in prev_bb & !new_bb {
                    observer.top_removed(&new_pos, piece, removed);
                }
            }

            for sq in dirty_bb {
                let top_before = self.stacks.top(sq);
                let top_after = new_pos.stacks.top(sq);

                let height_before = self.stacks.height(sq);
                let height_after = new_pos.stacks.height(sq);

                let colors_before = self.stacks.colors(sq);
                let colors_after = new_pos.stacks.colors(sq);

                observer.stack_changed(
                    &new_pos,
                    sq,
                    top_before,
                    top_after,
                    height_before,
                    height_after,
                    colors_before,
                    colors_after,
                );
            }

            new_pos.update_influence(Color::White);
            new_pos.update_influence(Color::Black);

            debug_assert_eq!(
                new_pos.pieces[PieceType::Flat] & new_pos.pieces[PieceType::Wall] & new_pos.pieces[PieceType::Capstone],
                Bitboard::empty()
            );

            debug_assert_eq!(
                new_pos.colors[Color::White] & new_pos.colors[Color::Black],
                Bitboard::empty()
            );

            debug_assert_eq!(
                new_pos.pieces[PieceType::Flat] | new_pos.pieces[PieceType::Wall] | new_pos.pieces[PieceType::Capstone],
                new_pos.colors[Color::White] | new_pos.colors[Color::Black]
            );
        } else {
            debug_assert_eq!(self.stacks.top(mv.sq()), None);
            debug_assert!(self.ply() >= 2 || mv.pt() == PieceType::Flat);

            let dropped_color = if self.ply() < 2 { self.stm().flip() } else { self.stm() };
            let dropped = mv.pt().with_color(dropped_color);

            new_pos.stacks.push(mv.sq(), mv.pt(), dropped_color);

            new_pos.colors[dropped_color].set_sq(mv.sq());
            new_pos.pieces[mv.pt()].set_sq(mv.sq());

            let (dropped_count, dropped_color_mask) = if self.ply() < 2
                && dropped_color == Color::Black
                && DOUBLE_BLACK_STACK.load(atomic::Ordering::Relaxed)
            {
                new_pos.stacks.push(mv.sq(), mv.pt(), dropped_color);
                (2, 0b11)
            } else {
                (1, dropped_color.raw() as u64)
            };

            match mv.pt() {
                PieceType::Capstone => new_pos.caps_in_hand[dropped_color] -= dropped_count,
                _ => new_pos.flats_in_hand[dropped_color] -= dropped_count,
            };

            observer.top_added(&new_pos, dropped, mv.sq());
            observer.stack_changed(
                &new_pos,
                mv.sq(),
                None,
                Some(dropped),
                0,
                dropped_count,
                0,
                dropped_color_mask,
            );

            new_pos.update_influence(dropped_color);
        }

        new_pos.stm = new_pos.stm.flip();
        new_pos.ply += 1;

        new_pos.stm_key ^= keys::black_key();

        observer.finalize(self, &new_pos);

        #[cfg(debug_assertions)]
        {
            let mut other_new = new_pos;
            other_new.regen();
            assert_eq!(new_pos, other_new);
        }

        new_pos
    }

    #[must_use]
    pub fn apply_move(&self, mv: Move) -> Self {
        self.apply_move_with_observer(mv, &mut NullObserver)
    }

    #[must_use]
    pub fn apply_nullmove(&self) -> Self {
        let mut new_pos = *self;

        new_pos.stm = new_pos.stm.flip();
        new_pos.ply += 1;

        new_pos.stm_key ^= keys::black_key();

        new_pos
    }

    #[must_use]
    pub fn tps(&self) -> String {
        let mut tps = String::with_capacity(21);

        for rank in (0..6).rev() {
            let mut groups = Vec::new();

            let mut file = 0;
            while file < 6 {
                let sq = Square::from_file_rank(file, rank).unwrap();

                if self.stacks.is_empty(sq) {
                    let mut empty = 1;

                    while file < 5 && self.stacks.is_empty(Square::from_file_rank(file + 1, rank).unwrap()) {
                        file += 1;
                        empty += 1;
                    }

                    if empty > 1 {
                        groups.push(format!("x{}", empty));
                    } else {
                        groups.push("x".to_owned());
                    }
                } else {
                    let mut stack_str = String::with_capacity(self.stacks.height(sq) as usize + 1);

                    let colors = self.stacks.colors(sq);
                    let height = self.stacks.height(sq);
                    for i in 0..height {
                        let height = height - i - 1;
                        let color = Color::from_raw(((colors >> height) & 0x1) as u8).unwrap();
                        match color {
                            Color::White => stack_str.push('1'),
                            Color::Black => stack_str.push('2'),
                        }
                    }

                    match self.stacks.top_pt(sq).unwrap() {
                        PieceType::Flat => {}
                        PieceType::Wall => stack_str.push('S'),
                        PieceType::Capstone => stack_str.push('C'),
                    }

                    groups.push(stack_str);
                }

                file += 1;
            }

            tps.push_str(&groups.join(","));

            if rank > 0 {
                tps.push('/');
            }
        }

        match self.stm() {
            Color::White => tps.push_str(" 1"),
            Color::Black => tps.push_str(" 2"),
        }

        tps.push_str(&format!(" {}", self.ply() / 2 + 1));

        tps
    }

    pub(crate) fn push_piece(&mut self, sq: Square, pt: PieceType, color: Color) {
        self.stacks.push(sq, pt, color);
    }

    pub(crate) fn finish_build(&mut self, stm: Color, ply: u16) {
        self.stm = stm;
        self.ply = ply;
        self.regen();
    }

    fn regen(&mut self) {
        self.colors.fill(Bitboard::empty());
        self.pieces.fill(Bitboard::empty());

        self.flats_in_hand.fill(TOTAL_FLATS);
        self.caps_in_hand.fill(TOTAL_CAPS);

        for sq in Square::all() {
            if self.stacks.is_empty(sq) {
                continue;
            }

            let top = self.stacks.top(sq).unwrap();

            self.colors[top.color()].set_sq(sq);
            self.pieces[top.piece_type()].set_sq(sq);

            if top.piece_type() == PieceType::Capstone {
                self.caps_in_hand[top.color()] -= 1;
            } else {
                self.flats_in_hand[top.color()] -= 1;
            }

            let covered = self.stacks.colors(sq) >> 1;
            let mask = (1 << (self.stacks.height(sq) - 1)) - 1;

            self.flats_in_hand[0] -= (!covered & mask).count_ones() as u8;
            self.flats_in_hand[1] -= (covered & mask).count_ones() as u8;
        }

        self.stacks.regen_key(self.occ());

        if self.stm() == Color::Black {
            self.stm_key = keys::black_key();
        } else {
            self.stm_key = 0;
        }

        self.update_influence(Color::White);
        self.update_influence(Color::Black);
    }

    fn update_influence(&mut self, color: Color) {
        let (road, influence) = influence(self.roads(color));
        self.roads[color] = road;
        self.influence[color] = influence;
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TpsError {
    WrongNumberOfParts,
    WrongNumberOfRanks,
    BlankFile,
    InvalidEmptyFileCount,
    InvalidCharInStack,
    ExcessCharsAfterStackTop,
    WrongNumberOfFiles,
    InvalidStm,
    InvalidFullmove,
}

impl FromStr for Position {
    type Err = TpsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split_ascii_whitespace().collect();
        Self::from_tps_parts(&parts)
    }
}

pub trait BoardObserver {
    fn top_added(&mut self, pos: &Position, top: Piece, sq: Square);
    fn top_removed(&mut self, pos: &Position, top: Piece, sq: Square);

    #[allow(clippy::too_many_arguments)]
    fn stack_changed(
        &mut self,
        pos: &Position,
        sq: Square,
        top_before: Option<Piece>,
        top_after: Option<Piece>,
        height_before: u8,
        height_after: u8,
        colors_before: u64,
        colors_after: u64,
    );

    fn finalize(&mut self, pos_before: &Position, pos_after: &Position);
}

pub struct NullObserver;

impl BoardObserver for NullObserver {
    fn top_added(&mut self, _pos: &Position, _top: Piece, _sq: Square) {
        // no-op
    }

    fn top_removed(&mut self, _pos: &Position, _top: Piece, _sq: Square) {
        // no-op
    }

    fn stack_changed(
        &mut self,
        _pos: &Position,
        _sq: Square,
        _top_before: Option<Piece>,
        _top_after: Option<Piece>,
        _height_before: u8,
        _height_after: u8,
        _colors_before: u64,
        _colors_after: u64,
    ) {
        // no-op
    }

    fn finalize(&mut self, _pos_before: &Position, _pos_after: &Position) {
        // no-op
    }
}
