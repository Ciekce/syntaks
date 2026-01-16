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
use std::fmt::{Display, Formatter, Write};
use std::str::FromStr;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Player {
    P1,
    P2,
}

impl Player {
    pub const COUNT: usize = 2;

    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::P1),
            1 => Some(Self::P2),
            _ => None,
        }
    }

    #[must_use]
    pub const fn raw(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn idx(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn flip(self) -> Self {
        Self::from_raw(self as u8 ^ 0x1).unwrap()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum PieceType {
    Flat,
    Wall,
    Capstone,
}

impl PieceType {
    pub const COUNT: usize = 3;

    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Flat),
            1 => Some(Self::Wall),
            2 => Some(Self::Capstone),
            _ => None,
        }
    }

    #[must_use]
    pub const fn raw(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn idx(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn with_player(self, player: Player) -> Piece {
        Piece::from_raw((self.raw() << 1) | player.raw()).unwrap()
    }

    #[must_use]
    pub const fn is_blocker(self) -> bool {
        self.raw() > 0
    }

    #[must_use]
    pub const fn is_road(self) -> bool {
        self.raw() & 0b1 == 0
    }
}

impl Display for PieceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PieceType::Flat => f.write_char('F'),
            PieceType::Wall => f.write_char('S'),
            PieceType::Capstone => f.write_char('C'),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Piece {
    P1Flat,
    P2Flat,
    P1Wall,
    P2Wall,
    P1Capstone,
    P2Capstone,
}

impl Piece {
    pub const COUNT: usize = 6;

    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::P1Flat),
            1 => Some(Self::P2Flat),
            2 => Some(Self::P1Wall),
            3 => Some(Self::P2Wall),
            4 => Some(Self::P1Capstone),
            5 => Some(Self::P2Capstone),
            _ => None,
        }
    }

    #[must_use]
    pub const fn raw(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn idx(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn player(self) -> Player {
        Player::from_raw(self.raw() & 0x1).unwrap()
    }

    #[must_use]
    pub const fn piece_type(self) -> PieceType {
        PieceType::from_raw(self.raw() >> 1).unwrap()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub const COUNT: usize = 4;

    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Up),
            1 => Some(Self::Down),
            2 => Some(Self::Left),
            3 => Some(Self::Right),
            _ => None,
        }
    }

    #[must_use]
    pub const fn raw(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn idx(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn offset(self) -> i8 {
        [6, -6, -1, 1][self.idx()]
    }
}

impl Display for Direction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Up => f.write_char('+'),
            Direction::Down => f.write_char('-'),
            Direction::Left => f.write_char('<'),
            Direction::Right => f.write_char('>'),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
#[rustfmt::skip]
pub enum Square {
    A1, B1, C1, D1, E1, F1,
    A2, B2, C2, D2, E2, F2,
    A3, B3, C3, D3, E3, F3,
    A4, B4, C4, D4, E4, F4,
    A5, B5, C5, D5, E5, F5,
    A6, B6, C6, D6, E6, F6,
}

impl Square {
    pub const COUNT: usize = 36;

    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        if (raw as usize) < Self::COUNT {
            // SAFETY: we just bounds checked the value
            Some(unsafe { std::mem::transmute::<u8, Square>(raw) })
        } else {
            None
        }
    }

    #[must_use]
    pub const fn from_file_rank(file: u32, rank: u32) -> Option<Self> {
        if file >= 6 || rank >= 6 {
            None
        } else {
            Some(Self::from_raw((rank as u8 * 6) + file as u8).unwrap())
        }
    }

    #[must_use]
    pub const fn raw(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn idx(self) -> usize {
        self as usize
    }

    #[must_use]
    pub const fn rank(self) -> u32 {
        self.raw() as u32 / 6
    }

    #[must_use]
    pub const fn file(self) -> u32 {
        self.raw() as u32 % 6
    }

    #[must_use]
    pub const fn bb(self) -> Bitboard {
        Bitboard::from_raw(1 << self.idx())
    }

    #[must_use]
    pub const fn shift(self, dir: Direction) -> Option<Self> {
        let shifted = self as i8 + dir.offset();
        if shifted >= 0 && shifted < Self::COUNT as i8 {
            Some(Self::from_raw(shifted as u8).unwrap())
        } else {
            None
        }
    }

    #[must_use]
    pub const fn shift_checked(self, dir: Direction) -> Option<Self> {
        match dir {
            Direction::Left => {
                if self.file() == 0 {
                    return None;
                }
            }
            Direction::Right => {
                if self.file() == 5 {
                    return None;
                }
            }
            _ => {}
        }
        self.shift(dir)
    }

    #[must_use]
    pub fn all() -> SquareIterator {
        SquareIterator { raw: 0 }
    }
}

impl Display for Square {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_char((b'a' + self.file() as u8) as char)?;
        f.write_char((b'1' + self.rank() as u8) as char)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SquareStrError {
    NonAsciiString,
    WrongLength,
    InvalidFile,
    InvalidRank,
}

impl FromStr for Square {
    type Err = SquareStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_ascii() {
            return Err(SquareStrError::NonAsciiString);
        }

        let bytes = s.as_bytes();

        if bytes.len() != 2 {
            return Err(SquareStrError::WrongLength);
        }

        let file = bytes[0];
        if !(b'a'..=b'f').contains(&file) {
            return Err(SquareStrError::InvalidFile);
        }

        let rank = bytes[1];
        if !(b'1'..=b'6').contains(&rank) {
            return Err(SquareStrError::InvalidRank);
        }

        Ok(Self::from_file_rank((file - b'a') as u32, (rank - b'1') as u32).unwrap())
    }
}

pub struct SquareIterator {
    raw: u8,
}

impl Iterator for SquareIterator {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        let sq = Square::from_raw(self.raw);
        self.raw += 1;
        sq
    }
}
