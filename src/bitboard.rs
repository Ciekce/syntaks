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

use crate::core::*;
use std::ops::*;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct Bitboard {
    raw: u64,
}

impl Bitboard {
    const MASK: u64 = (1 << Square::COUNT) - 1;

    pub const UPPER_EDGE: Self = Self::from_raw(0xfc0000000);
    pub const LOWER_EDGE: Self = Self::from_raw(0x3f);
    pub const LEFT_EDGE: Self = Self::from_raw(0x41041041);
    pub const RIGHT_EDGE: Self = Self::from_raw(0x820820820);

    #[must_use]
    pub const fn empty() -> Self {
        Self { raw: 0 }
    }

    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self {
            raw: raw & Self::MASK,
        }
    }

    #[must_use]
    pub const fn edge(dir: Direction) -> Self {
        [
            Self::UPPER_EDGE,
            Self::LOWER_EDGE,
            Self::LEFT_EDGE,
            Self::RIGHT_EDGE,
        ][dir.idx()]
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.raw
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.raw == 0
    }

    #[must_use]
    pub const fn has_sq(self, sq: Square) -> bool {
        (self.raw & sq.bb().raw) != 0
    }

    #[must_use]
    pub const fn with_sq(self, sq: Square) -> Self {
        Self {
            raw: self.raw | sq.bb().raw,
        }
    }

    #[must_use]
    pub const fn without_sq(self, sq: Square) -> Self {
        Self {
            raw: self.raw & !sq.bb().raw,
        }
    }

    #[must_use]
    pub const fn with_sq_toggled(self, sq: Square) -> Self {
        Self {
            raw: self.raw ^ sq.bb().raw,
        }
    }

    pub const fn set_sq(&mut self, sq: Square) {
        self.raw |= sq.bb().raw;
    }

    pub const fn clear_sq(&mut self, sq: Square) {
        self.raw &= !sq.bb().raw;
    }

    pub const fn toggle_sq(&mut self, sq: Square) {
        self.raw ^= sq.bb().raw;
    }

    #[must_use]
    pub const fn cmpl(self) -> Self {
        Self {
            raw: !self.raw & Self::MASK,
        }
    }

    #[must_use]
    pub const fn and(self, other: Self) -> Self {
        Self {
            raw: self.raw & other.raw,
        }
    }

    #[must_use]
    pub const fn or(self, other: Self) -> Self {
        Self {
            raw: self.raw | other.raw,
        }
    }

    #[must_use]
    pub const fn xor(self, other: Self) -> Self {
        Self {
            raw: self.raw ^ other.raw,
        }
    }

    #[must_use]
    pub const fn shr(self, count: u32) -> Self {
        Self {
            raw: self.raw >> count,
        }
    }

    #[must_use]
    pub const fn shl(self, count: u32) -> Self {
        Self {
            raw: (self.raw << count) & Self::MASK,
        }
    }

    #[must_use]
    pub const fn lsb(self) -> Option<Square> {
        if self.is_empty() {
            None
        } else {
            Some(Square::from_raw(self.raw.trailing_zeros() as u8).unwrap())
        }
    }

    pub fn pop_lsb(&mut self) -> Option<Square> {
        let sq = self.lsb()?;
        self.raw &= self.raw - 1;
        Some(sq)
    }

    #[must_use]
    pub const fn popcount(self) -> u32 {
        self.raw.count_ones()
    }

    #[must_use]
    pub const fn shift(self, dir: Direction) -> Self {
        match dir {
            Direction::Up => self.shl(dir.offset() as u32),
            Direction::Down => self.shr(-dir.offset() as u32),
            Direction::Left => self.and(Self::LEFT_EDGE.cmpl()).shr(-dir.offset() as u32),
            Direction::Right => self.and(Self::RIGHT_EDGE.cmpl()).shl(dir.offset() as u32),
        }
    }
}

impl Not for Bitboard {
    type Output = Self;

    fn not(self) -> Self::Output {
        self.cmpl()
    }
}

impl BitAnd for Bitboard {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.and(rhs)
    }
}

impl BitOr for Bitboard {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.or(rhs)
    }
}

impl BitXor for Bitboard {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        self.xor(rhs)
    }
}

impl Shr<u32> for Bitboard {
    type Output = Bitboard;

    fn shr(self, rhs: u32) -> Self::Output {
        self.shr(rhs)
    }
}

impl Shl<u32> for Bitboard {
    type Output = Bitboard;

    fn shl(self, rhs: u32) -> Self::Output {
        self.shl(rhs)
    }
}

impl BitAndAssign for Bitboard {
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs
    }
}

impl BitOrAssign for Bitboard {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs
    }
}

impl BitXorAssign for Bitboard {
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs
    }
}

impl ShrAssign<u32> for Bitboard {
    fn shr_assign(&mut self, rhs: u32) {
        *self = *self >> rhs;
    }
}

impl ShlAssign<u32> for Bitboard {
    fn shl_assign(&mut self, rhs: u32) {
        *self = *self << rhs;
    }
}

impl IntoIterator for Bitboard {
    type Item = Square;
    type IntoIter = Biterator;

    fn into_iter(self) -> Self::IntoIter {
        Biterator { board: self }
    }
}

pub struct Biterator {
    board: Bitboard,
}

impl Iterator for Biterator {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        self.board.pop_lsb()
    }
}
