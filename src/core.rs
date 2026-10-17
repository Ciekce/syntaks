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
use std::ops::{Index, IndexMut};
use std::str::FromStr;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub const COUNT: usize = 2;

    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::White),
            1 => Some(Self::Black),
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
    pub const fn sign(self) -> i32 {
        match self {
            Self::White => 1,
            Self::Black => -1,
        }
    }

    #[must_use]
    pub const fn flip(self) -> Self {
        Self::from_raw(self as u8 ^ 0x1).unwrap()
    }
}

impl<T> Index<Color> for [T; Color::COUNT] {
    type Output = T;

    fn index(&self, index: Color) -> &Self::Output {
        // SAFETY: only implemented for arrays of the correct length
        unsafe { self.get_unchecked(index.idx()) }
    }
}

impl<T> IndexMut<Color> for [T; Color::COUNT] {
    fn index_mut(&mut self, index: Color) -> &mut Self::Output {
        // SAFETY: only implemented for arrays of the correct length
        unsafe { self.get_unchecked_mut(index.idx()) }
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
    pub const fn with_color(self, color: Color) -> Piece {
        Piece::from_raw((self.raw() << 1) | color.raw()).unwrap()
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

impl<T> Index<PieceType> for [T; PieceType::COUNT] {
    type Output = T;

    fn index(&self, index: PieceType) -> &Self::Output {
        // SAFETY: only implemented for arrays of the correct length
        unsafe { self.get_unchecked(index.idx()) }
    }
}

impl<T> IndexMut<PieceType> for [T; PieceType::COUNT] {
    fn index_mut(&mut self, index: PieceType) -> &mut Self::Output {
        // SAFETY: only implemented for arrays of the correct length
        unsafe { self.get_unchecked_mut(index.idx()) }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Piece {
    WhiteFlat,
    BlackFlat,
    WhiteWall,
    BlackWall,
    WhiteCapstone,
    BlackCapstone,
}

impl Piece {
    pub const COUNT: usize = 6;

    pub const fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::WhiteFlat),
            1 => Some(Self::BlackFlat),
            2 => Some(Self::WhiteWall),
            3 => Some(Self::BlackWall),
            4 => Some(Self::WhiteCapstone),
            5 => Some(Self::BlackCapstone),
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
    pub const fn color(self) -> Color {
        Color::from_raw(self.raw() & 0x1).unwrap()
    }

    #[must_use]
    pub const fn piece_type(self) -> PieceType {
        PieceType::from_raw(self.raw() >> 1).unwrap()
    }
}

impl<T> Index<Piece> for [T; Piece::COUNT] {
    type Output = T;

    fn index(&self, index: Piece) -> &Self::Output {
        // SAFETY: only implemented for arrays of the correct length
        unsafe { self.get_unchecked(index.idx()) }
    }
}

impl<T> IndexMut<Piece> for [T; Piece::COUNT] {
    fn index_mut(&mut self, index: Piece) -> &mut Self::Output {
        // SAFETY: only implemented for arrays of the correct length
        unsafe { self.get_unchecked_mut(index.idx()) }
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

impl<T> Index<Direction> for [T; Direction::COUNT] {
    type Output = T;

    fn index(&self, index: Direction) -> &Self::Output {
        // SAFETY: only implemented for arrays of the correct length
        unsafe { self.get_unchecked(index.idx()) }
    }
}

impl<T> IndexMut<Direction> for [T; Direction::COUNT] {
    fn index_mut(&mut self, index: Direction) -> &mut Self::Output {
        // SAFETY: only implemented for arrays of the correct length
        unsafe { self.get_unchecked_mut(index.idx()) }
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
    pub const fn flip_rank(self) -> Self {
        let file = self.file();
        let rank = self.rank();
        Self::from_file_rank(file, 5 - rank).unwrap()
    }

    #[must_use]
    pub const fn flip_file(self) -> Self {
        let file = self.file();
        let rank = self.rank();
        Self::from_file_rank(5 - file, rank).unwrap()
    }

    #[must_use]
    pub const fn flip_diag(self) -> Self {
        let file = self.file();
        let rank = self.rank();
        Self::from_file_rank(rank, file).unwrap()
    }

    #[must_use]
    pub const fn transform(self, transform: Transform) -> Self {
        static LUT: [[Square; 8]; Square::COUNT] = {
            let mut lut = [[Square::A1; _]; _];
            let mut t = 0;
            while t < 8 {
                let transform = Transform::from_raw(t as u8).unwrap();
                let mut s = 0;
                while s < Square::COUNT {
                    let mut square = Square::from_raw(s as u8).unwrap();
                    if transform.is_flip_rank() {
                        square = square.flip_rank();
                    }
                    if transform.is_flip_file() {
                        square = square.flip_file();
                    }
                    if transform.is_flip_diag() {
                        square = square.flip_diag();
                    }
                    lut[s][t] = square;
                    s += 1;
                }
                t += 1;
            }
            lut
        };
        LUT[self.idx()][transform.idx()]
    }

    #[must_use]
    pub const fn normalizer(self) -> Transform {
        static LUT: [Transform; Square::COUNT] = {
            let mut lut = [Transform::identity(); _];
            let mut s = 0;
            while s < Square::COUNT {
                let square = Square::from_raw(s as u8).unwrap();
                let mut t = 0;
                loop {
                    let transform = Transform::from_raw(t).unwrap();
                    let new = square.transform(transform);
                    if new.rank() < 3 && new.file() < 3 && new.rank() <= new.file() {
                        lut[s] = transform;
                        break;
                    }
                    t += 1;
                }
                s += 1;
            }
            lut
        };
        LUT[self.idx()]
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
            Direction::Left if self.file() == 0 => None,
            Direction::Right if self.file() == 5 => None,
            _ => self.shift(dir),
        }
    }

    #[must_use]
    pub fn all() -> SquareIterator {
        SquareIterator { raw: 0 }
    }
}

#[must_use]
pub const fn normalizer(opt_square: Option<Square>) -> Transform {
    match opt_square {
        Some(square) => square,
        None => Square::A1,
    }
    .normalizer()
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

impl<T> Index<Square> for [T; Square::COUNT] {
    type Output = T;

    fn index(&self, index: Square) -> &Self::Output {
        // SAFETY: only implemented for arrays of the correct length
        unsafe { self.get_unchecked(index.idx()) }
    }
}

impl<T> IndexMut<Square> for [T; Square::COUNT] {
    fn index_mut(&mut self, index: Square) -> &mut Self::Output {
        // SAFETY: only implemented for arrays of the correct length
        unsafe { self.get_unchecked_mut(index.idx()) }
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Transform(u8);

impl Transform {
    #[must_use]
    pub const fn identity() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn from_raw(raw: u8) -> Option<Self> {
        if raw < 8 { Some(Self(raw)) } else { None }
    }

    #[must_use]
    pub const fn is_flip_rank(self) -> bool {
        self.0 & 1 != 0
    }

    #[must_use]
    pub const fn is_flip_file(self) -> bool {
        self.0 & 2 != 0
    }

    #[must_use]
    pub const fn is_flip_diag(self) -> bool {
        self.0 & 4 != 0
    }

    #[must_use]
    pub const fn idx(self) -> usize {
        self.0 as usize
    }
}

pub type Score = i32;

pub const SCORE_INF: Score = 32767;
pub const SCORE_MATE: Score = SCORE_INF - 1;
pub const SCORE_WIN: Score = 25000;
pub const SCORE_NONE: Score = -SCORE_INF;

#[must_use]
pub const fn is_win(score: Score) -> bool {
    score > SCORE_WIN
}

#[must_use]
pub const fn is_loss(score: Score) -> bool {
    score < -SCORE_WIN
}

#[must_use]
pub const fn is_decisive(score: Score) -> bool {
    score.abs() > SCORE_WIN
}
