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
use crate::road::has_road;
use crate::takmove::Move;
use std::cmp::Ordering;
use std::str::FromStr;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
struct Keys {
    stacks: u64,
    blockers: u64,
}

impl Keys {
    fn reset(&mut self) {
        *self = Default::default();
    }

    fn toggle_top_key(&mut self, pt: PieceType, sq: Square) {
        self.stacks ^= keys::top_key(pt, sq);
        if pt.is_blocker() {
            self.blockers ^= keys::top_key(pt, sq);
        }
    }

    fn toggle_player_key(&mut self, height: u8, player: Player, sq: Square) {
        self.stacks ^= keys::player_key(height, player, sq);
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Stacks {
    players: [u64; Square::COUNT],
    heights: [u8; Square::COUNT],
    tops: [Option<PieceType>; Square::COUNT],
    keys: Keys,
}

impl Stacks {
    // all flats + cap
    pub const MAX_HEIGHT: usize = 30 + 30 + 1;

    #[must_use]
    pub fn is_empty(&self, sq: Square) -> bool {
        self.tops[sq.idx()].is_none()
    }

    #[must_use]
    pub fn top(&self, sq: Square) -> Option<PieceType> {
        self.tops[sq.idx()]
    }

    #[must_use]
    pub fn top_player(&self, sq: Square) -> Option<Player> {
        if self.tops[sq.idx()].is_none() {
            None
        } else {
            Some(
                Player::from_raw((self.players[sq.idx()] >> (self.heights[sq.idx()] - 1)) as u8)
                    .unwrap(),
            )
        }
    }

    #[must_use]
    pub fn height(&self, sq: Square) -> u8 {
        self.heights[sq.idx()]
    }

    #[must_use]
    pub fn players(&self, sq: Square) -> u64 {
        self.players[sq.idx()]
    }

    fn push(&mut self, sq: Square, pt: PieceType, player: Player) {
        debug_assert_ne!(self.tops[sq.idx()], Some(PieceType::Capstone));

        if let Some(prev_top) = self.tops[sq.idx()] {
            self.keys.toggle_top_key(prev_top, sq);
        }

        self.keys.toggle_top_key(pt, sq);

        let height = self.heights[sq.idx()];
        self.keys.toggle_player_key(height, player, sq);

        self.players[sq.idx()] |= (player.raw() as u64) << self.heights[sq.idx()];
        self.heights[sq.idx()] += 1;
        self.tops[sq.idx()] = Some(pt);
    }

    fn take(&mut self, sq: Square, count: u8) -> (u8, PieceType, Option<Player>) {
        debug_assert!(count <= self.heights[sq.idx()]);
        debug_assert!(count > 0);
        debug_assert!(count <= 6);

        let players =
            (self.players[sq.idx()] >> (self.heights[sq.idx()] - count)) & ((1 << count) - 1);
        let top = self.tops[sq.idx()].unwrap();

        self.keys.toggle_top_key(top, sq);

        let old_height = self.heights[sq.idx()];
        self.heights[sq.idx()] -= count;
        let new_height = self.heights[sq.idx()];

        for height in new_height..old_height {
            let player =
                Player::from_raw(((self.players[sq.idx()] >> height) & 0x1) as u8).unwrap();
            self.keys.toggle_player_key(height, player, sq);
        }

        self.players[sq.idx()] &= (1 << new_height) - 1;

        if new_height == 0 {
            self.tops[sq.idx()] = None;
            (players as u8, top, None)
        } else {
            self.keys.toggle_top_key(PieceType::Flat, sq);
            self.tops[sq.idx()] = Some(PieceType::Flat);
            let new_top_player =
                Player::from_raw(((self.players[sq.idx()] >> (new_height - 1)) & 0x1) as u8)
                    .unwrap();
            (players as u8, top, Some(new_top_player))
        }
    }

    fn regen_key(&mut self, occ: Bitboard) {
        self.keys.reset();

        for sq in occ {
            let players = self.players(sq);
            let height = self.height(sq);
            let top = self.top(sq).unwrap();

            self.keys.toggle_top_key(top, sq);

            for i in 0..height {
                let player = Player::from_raw(((players >> i) & 0x1) as u8).unwrap();
                self.keys.toggle_player_key(i, player, sq);
            }
        }
    }

    pub fn iter(&self, sq: Square) -> StackIterator {
        StackIterator {
            players: self.players[sq.idx()],
            height: self.heights[sq.idx()],
            idx: 0,
        }
    }
}

impl Default for Stacks {
    fn default() -> Self {
        Self {
            players: [u64::default(); Square::COUNT],
            heights: [u8::default(); Square::COUNT],
            tops: [None; Square::COUNT],
            keys: Default::default(),
        }
    }
}

pub struct StackIterator {
    players: u64,
    height: u8,
    idx: u8,
}

impl Iterator for StackIterator {
    type Item = Player;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx == self.height {
            return None;
        }

        let player = Player::from_raw(((self.players >> self.idx) & 0x1) as u8).unwrap();
        self.idx += 1;
        Some(player)
    }
}

pub enum FlatCountOutcome {
    None,
    Draw,
    Win(Player),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Position {
    stacks: Stacks,
    players: [Bitboard; Player::COUNT],
    pieces: [Bitboard; PieceType::COUNT],
    flats_in_hand: [u8; Player::COUNT],
    caps_in_hand: [u8; Player::COUNT],
    stm: Player,
    ply: u16,
    player_key: u64,
}

impl Position {
    pub const CARRY_LIMIT: u8 = 6;
    pub const KOMI: u32 = 2;

    #[must_use]
    pub fn startpos() -> Self {
        Self {
            stacks: Stacks::default(),
            players: [Bitboard::empty(); Player::COUNT],
            pieces: [Bitboard::empty(); PieceType::COUNT],
            flats_in_hand: [30; Player::COUNT],
            caps_in_hand: [1; Player::COUNT],
            stm: Player::P1,
            ply: 0,
            player_key: 0,
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

                    let mut players = Vec::with_capacity(stack.len());
                    let mut top = None;

                    for c in stack.chars() {
                        if top.is_some() {
                            return Err(TpsError::ExcessCharsAfterStackTop);
                        }

                        match c {
                            '1' => players.push(Player::P1),
                            '2' => players.push(Player::P2),
                            'F' => top = Some(PieceType::Flat), // nonstandard but why not
                            'S' => top = Some(PieceType::Wall),
                            'C' => top = Some(PieceType::Capstone),
                            _ => return Err(TpsError::InvalidCharInStack),
                        }
                    }

                    let top = top.unwrap_or(PieceType::Flat);

                    for (idx, &player) in players.iter().enumerate() {
                        if idx == players.len() - 1 {
                            pos.stacks.push(sq, top, player);
                        } else {
                            pos.stacks.push(sq, PieceType::Flat, player);
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
            "1" => pos.stm = Player::P1,
            "2" => pos.stm = Player::P2,
            _ => return Err(TpsError::InvalidStm),
        }

        if parts.len() >= 3 {
            match parts[2].parse::<u16>() {
                Ok(fullmove) => {
                    pos.ply = (fullmove.max(1) - 1) * 2 + if pos.stm == Player::P2 { 1 } else { 0 }
                }
                Err(_) => return Err(TpsError::InvalidFullmove),
            }
        }

        pos.regen();

        Ok(pos)
    }

    #[must_use]
    pub fn stm(&self) -> Player {
        self.stm
    }

    #[must_use]
    pub fn stacks(&self) -> &Stacks {
        &self.stacks
    }

    #[must_use]
    pub fn player_bb(&self, player: Player) -> Bitboard {
        self.players[player.idx()]
    }

    #[must_use]
    pub fn piece_bb(&self, pt: PieceType) -> Bitboard {
        self.pieces[pt.idx()]
    }

    #[must_use]
    pub fn player_piece_bb(&self, piece: Piece) -> Bitboard {
        self.player_bb(piece.player()) & self.piece_bb(piece.piece_type())
    }

    #[must_use]
    pub fn occ(&self) -> Bitboard {
        self.players[0] | self.players[1]
    }

    #[must_use]
    pub fn flats_in_hand(&self, player: Player) -> u8 {
        self.flats_in_hand[player.idx()]
    }

    #[must_use]
    pub fn caps_in_hand(&self, player: Player) -> u8 {
        self.caps_in_hand[player.idx()]
    }

    #[must_use]
    pub fn ply(&self) -> u16 {
        self.ply
    }

    #[must_use]
    pub fn key(&self) -> u64 {
        self.player_key ^ self.stacks().keys.stacks
    }

    #[must_use]
    pub fn blocker_key(&self) -> u64 {
        self.stacks.keys.blockers
    }

    #[must_use]
    pub fn all_blockers(&self) -> Bitboard {
        self.piece_bb(PieceType::Wall) | self.piece_bb(PieceType::Capstone)
    }

    #[must_use]
    pub fn blockers(&self, player: Player) -> Bitboard {
        self.all_blockers() & self.player_bb(player)
    }

    #[must_use]
    pub fn all_roads(&self) -> Bitboard {
        self.piece_bb(PieceType::Flat) | self.piece_bb(PieceType::Capstone)
    }

    #[must_use]
    pub fn roads(&self, player: Player) -> Bitboard {
        self.all_roads() & self.player_bb(player)
    }

    #[must_use]
    pub fn has_road(&self, player: Player) -> bool {
        has_road(self.roads(player))
    }

    #[must_use]
    fn has_no_more_pieces(&self, player: Player) -> bool {
        self.flats_in_hand(player) == 0 && self.caps_in_hand(player) == 0
    }

    #[must_use]
    pub fn count_flats(&self) -> FlatCountOutcome {
        if !(!self.occ()).is_empty()
            && !self.has_no_more_pieces(Player::P1)
            && !self.has_no_more_pieces(Player::P2)
        {
            return FlatCountOutcome::None;
        }

        let p1_flats = self.player_piece_bb(Piece::P1Flat).popcount();
        let p2_flats = self.player_piece_bb(Piece::P2Flat).popcount() + Self::KOMI;

        match p1_flats.cmp(&p2_flats) {
            Ordering::Less => FlatCountOutcome::Win(Player::P2),
            Ordering::Equal => FlatCountOutcome::Draw,
            Ordering::Greater => FlatCountOutcome::Win(Player::P1),
        }
    }

    #[must_use]
    pub fn is_legal(&self, mv: Move) -> bool {
        if mv.is_spread() {
            if self.ply < 2 {
                return false;
            }

            if self
                .stacks
                .top_player(mv.sq())
                .is_none_or(|p| p != self.stm)
            {
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
                && let Some(hit_top) = self.stacks.top(hit_sq)
            {
                match hit_top {
                    PieceType::Flat => {}
                    PieceType::Wall => {
                        // multiple pieces dropped on the final square
                        if pattern & (1 << (Self::CARRY_LIMIT - 1)) == 0 {
                            return false;
                        }

                        let top = self.stacks.top(mv.sq()).unwrap();
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
    pub fn apply_move(&self, mv: Move) -> Self {
        let mut new_pos = *self;

        if mv.is_spread() {
            debug_assert_ne!(self.stacks.top(mv.sq()), None);
            debug_assert_eq!(self.stacks.top_player(mv.sq()), Some(self.stm()));
            debug_assert!(self.ply() >= 2);

            let pattern = mv.pattern();
            let dir = mv.dir();

            let dropped = pattern.trailing_zeros();
            let taken = 6 - dropped;

            let mut pattern = pattern >> dropped;
            let (mut players, top, new_top_player) = new_pos.stacks.take(mv.sq(), taken as u8);

            let mut new_flats_bb = Bitboard::empty();
            let mut new_player_bbs = [Bitboard::empty(); Player::COUNT];

            if let Some(new_top_player) = new_top_player {
                new_player_bbs[new_top_player.idx()].set_sq(mv.sq());
                new_flats_bb.set_sq(mv.sq());
            } else {
                new_pos.players[self.stm().idx()].toggle_sq(mv.sq());
            }

            if top != PieceType::Flat || new_top_player.is_none() {
                new_pos.pieces[top.idx()].toggle_sq(mv.sq());
            }

            let mut sq = mv.sq().shift(dir).unwrap();

            for idx in 0..taken {
                let player = Player::from_raw(players & 0x1).unwrap();
                let pt = if idx == taken - 1 {
                    top
                } else {
                    PieceType::Flat
                };

                new_pos.stacks.push(sq, pt, player);

                pattern >>= 1;
                players >>= 1;

                if (pattern & 0x1) != 0 {
                    new_player_bbs[player.idx()].set_sq(sq);
                    new_flats_bb.set_sq(sq);

                    sq = sq.shift(dir).unwrap();
                }
            }

            new_player_bbs[self.stm().idx()].set_sq(sq);

            debug_assert_eq!(new_player_bbs[0] & new_player_bbs[1], Bitboard::empty());

            for player in 0..Player::COUNT {
                new_pos.players[player] = (new_pos.players[player] | new_player_bbs[player])
                    & !new_player_bbs[player ^ 0x1];
            }

            new_pos.pieces[top.idx()].set_sq(sq);
            new_pos.pieces[PieceType::Flat.idx()] |= new_flats_bb;

            if let Some(prev_dst_top) = self.stacks.top(sq)
                && prev_dst_top != top
            {
                new_pos.pieces[prev_dst_top.idx()].clear_sq(sq);
                new_pos.pieces[top.idx()].set_sq(sq);
            }

            debug_assert_eq!(
                new_pos.pieces[PieceType::Flat.idx()]
                    & new_pos.pieces[PieceType::Wall.idx()]
                    & new_pos.pieces[PieceType::Capstone.idx()],
                Bitboard::empty()
            );

            debug_assert_eq!(
                new_pos.players[Player::P1.idx()] & new_pos.players[Player::P2.idx()],
                Bitboard::empty()
            );

            debug_assert_eq!(
                new_pos.pieces[PieceType::Flat.idx()]
                    | new_pos.pieces[PieceType::Wall.idx()]
                    | new_pos.pieces[PieceType::Capstone.idx()],
                new_pos.players[Player::P1.idx()] | new_pos.players[Player::P2.idx()]
            );
        } else {
            debug_assert_eq!(self.stacks.top(mv.sq()), None);
            debug_assert!(self.ply() >= 2 || mv.pt() == PieceType::Flat);

            let dropped_player = if self.ply() < 2 {
                self.stm().flip()
            } else {
                self.stm()
            };

            new_pos.stacks.push(mv.sq(), mv.pt(), dropped_player);

            new_pos.players[dropped_player.idx()].set_sq(mv.sq());
            new_pos.pieces[mv.pt().idx()].set_sq(mv.sq());

            match mv.pt() {
                PieceType::Capstone => new_pos.caps_in_hand[dropped_player.idx()] -= 1,
                _ => new_pos.flats_in_hand[dropped_player.idx()] -= 1,
            }
        }

        new_pos.stm = new_pos.stm.flip();
        new_pos.ply += 1;

        new_pos.player_key ^= keys::p2_key();

        #[cfg(debug_assertions)]
        {
            let mut other_new = new_pos;
            other_new.regen();
            assert_eq!(new_pos, other_new);
        }

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

                    while file < 5
                        && self
                            .stacks
                            .is_empty(Square::from_file_rank(file + 1, rank).unwrap())
                    {
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

                    for player in self.stacks.iter(sq) {
                        match player {
                            Player::P1 => stack_str.push('1'),
                            Player::P2 => stack_str.push('2'),
                        }
                    }

                    match self.stacks.top(sq).unwrap() {
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
            Player::P1 => tps.push_str(" 1"),
            Player::P2 => tps.push_str(" 2"),
        }

        tps.push_str(&format!(" {}", self.ply() / 2 + 1));

        tps
    }

    fn regen(&mut self) {
        self.players.fill(Bitboard::empty());
        self.pieces.fill(Bitboard::empty());

        self.flats_in_hand.fill(30);
        self.caps_in_hand.fill(1);

        for sq_idx in 0..Square::COUNT {
            let sq = Square::from_raw(sq_idx as u8).unwrap();

            if self.stacks.is_empty(sq) {
                continue;
            }

            let player = self.stacks.top_player(sq).unwrap();
            let top = self.stacks.top(sq).unwrap();

            self.players[player.idx()].set_sq(sq);
            self.pieces[top.idx()].set_sq(sq);

            if top == PieceType::Capstone {
                self.caps_in_hand[player.idx()] -= 1;
            } else {
                self.flats_in_hand[player.idx()] -= 1;
            }

            let players = self.stacks.players(sq);
            let covered = (1 << (self.stacks.height(sq) - 1)) - 1;

            self.flats_in_hand[0] -= (!players & covered).count_ones() as u8;
            self.flats_in_hand[1] -= (players & covered).count_ones() as u8;
        }

        self.stacks.regen_key(self.occ());

        if self.stm() == Player::P2 {
            self.player_key = keys::p2_key();
        } else {
            self.player_key = 0;
        }
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
