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

use crate::board::Position;
use crate::core::*;
use std::fmt::{Display, Formatter};
use std::num::NonZeroU16;
use std::str::FromStr;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Move {
    raw: NonZeroU16,
}

impl Move {
    const SQUARE_BITS: usize = 6;
    const PATTERN_BITS: usize = 6;
    const FLAG_BITS: usize = 2;
    pub const TOTAL_BITS: usize = Self::SQUARE_BITS + Self::PATTERN_BITS + Self::FLAG_BITS;

    const SQUARE_SHIFT: usize = 0;
    const PATTERN_SHIFT: usize = Self::SQUARE_SHIFT + Self::SQUARE_BITS;
    const FLAG_SHIFT: usize = Self::PATTERN_SHIFT + Self::PATTERN_BITS;

    pub const SQUARE_MASK: u16 = (1 << Self::SQUARE_BITS) - 1;
    pub const PATTERN_MASK: u16 = (1 << Self::PATTERN_BITS) - 1;
    pub const FLAG_MASK: u16 = (1 << Self::FLAG_BITS) - 1;

    #[must_use]
    pub const fn placement(pt: PieceType, dst: Square) -> Self {
        let mut raw = 0;

        raw |= (pt.raw() as u16 + 1) << Self::FLAG_SHIFT;
        raw |= dst.raw() as u16;

        Self {
            raw: NonZeroU16::new(raw).unwrap(),
        }
    }

    #[must_use]
    pub const fn spread(src: Square, dir: Direction, pattern: u16) -> Self {
        assert!(pattern != 0);
        assert!((pattern & !Self::PATTERN_MASK) == 0);

        let mut raw = 0;

        raw |= (dir as u16) << Self::FLAG_SHIFT;
        raw |= pattern << Self::PATTERN_SHIFT;
        raw |= (src.raw() as u16) << Self::SQUARE_SHIFT;

        Self {
            raw: NonZeroU16::new(raw).unwrap(),
        }
    }

    #[must_use]
    pub const fn from_raw(raw: u16) -> Option<Move> {
        if raw == 0 {
            None
        } else {
            Some(Self {
                raw: NonZeroU16::new(raw).unwrap(),
            })
        }
    }

    pub const fn raw(self) -> u16 {
        self.raw.get()
    }

    #[must_use]
    pub const fn sq(self) -> Square {
        Square::from_raw((((self.raw.get()) >> Self::SQUARE_SHIFT) & Self::SQUARE_MASK) as u8)
            .unwrap()
    }

    #[must_use]
    pub const fn pattern(self) -> u16 {
        (self.raw.get() >> Self::PATTERN_SHIFT) & Self::PATTERN_MASK
    }

    #[must_use]
    pub const fn is_spread(self) -> bool {
        self.pattern() != 0
    }

    #[must_use]
    pub const fn pt(self) -> PieceType {
        assert!(!self.is_spread());
        PieceType::from_raw((((self.raw.get() >> Self::FLAG_SHIFT) & Self::FLAG_MASK) - 1) as u8)
            .unwrap()
    }

    #[must_use]
    pub const fn dir(self) -> Direction {
        assert!(self.is_spread());
        Direction::from_raw(((self.raw.get() >> Self::FLAG_SHIFT) & Self::FLAG_MASK) as u8).unwrap()
    }

    #[must_use]
    pub const fn spread_length(self) -> u8 {
        // it will just be zero if its not a spread
        // so its perfectly fine to call on placements, just not very useful

        self.pattern().count_ones() as u8
    }

    #[must_use]
    pub const fn spread_dest(self) -> Square {
        assert!(self.is_spread());
        let sq = self.sq();
        let dir = self.dir();

        let offset = dir.offset() * self.spread_length() as i8;

        Square::from_raw((sq.raw() as i8 + offset) as u8).unwrap()
    }
}

impl Display for Move {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_spread() {
            let pattern = self.pattern();

            let dropped = pattern.trailing_zeros();
            let taken = 6 - dropped;

            if taken == 1 {
                write!(f, "{}{}", self.sq(), self.dir())?;
            } else {
                write!(f, "{}{}{}", taken, self.sq(), self.dir())?;
                if pattern.count_ones() > 1 {
                    let mut pattern = ((pattern | (1 << Position::CARRY_LIMIT)) >> dropped) & !1;
                    while pattern != 0 {
                        let dropped = pattern.trailing_zeros();
                        pattern = (pattern >> dropped) & !1;
                        write!(f, "{}", dropped)?;
                    }
                }
            }
        } else {
            match self.pt() {
                PieceType::Flat => write!(f, "{}", self.sq())?,
                _ => write!(f, "{}{}", self.pt(), self.sq())?,
            }
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum MoveStrError {
    NonAsciiString,
    TooShort,
    MissingSquare,
    InvalidSquare(SquareStrError),
    InvalidDirection,
    TooManySpreadSteps,
    InvalidSpreadPattern,
    TooManySpreadPieces,
}

impl FromStr for Move {
    type Err = MoveStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_ascii() {
            return Err(MoveStrError::NonAsciiString);
        }

        if s.len() < 2 {
            return Err(MoveStrError::TooShort);
        }

        let bytes = s.as_bytes();

        let mut pt = None;
        let mut taken = None;

        let mut next = 0;

        match bytes[0] {
            b'F' => {
                pt = Some(PieceType::Flat);
                next += 1;
            }
            b'S' => {
                pt = Some(PieceType::Wall);
                next += 1;
            }
            b'C' => {
                pt = Some(PieceType::Capstone);
                next += 1;
            }
            b'1'..=b'9' => {
                taken = Some(bytes[0] - b'0');
                next += 1;
            }
            _ => {}
        }

        if (s.len() - next) < 2 {
            return Err(MoveStrError::MissingSquare);
        }

        let sq = match s[next..(next + 2)].parse() {
            Ok(sq) => sq,
            Err(err) => return Err(MoveStrError::InvalidSquare(err)),
        };

        next += 2;

        if next == s.len() {
            let pt = pt.unwrap_or(PieceType::Flat);
            return Ok(Self::placement(pt, sq));
        }

        let dir = match bytes[next] {
            b'+' => Direction::Up,
            b'-' => Direction::Down,
            b'<' => Direction::Left,
            b'>' => Direction::Right,
            _ => return Err(MoveStrError::InvalidDirection),
        };

        next += 1;

        let taken = taken.unwrap_or(1);
        let bytes = bytes.strip_suffix(b"*").unwrap_or(bytes);

        if (s.len() - next) > 6 {
            return Err(MoveStrError::TooManySpreadSteps);
        }

        let mut pattern = 1;
        let mut bit = 1;

        for &pattern_char in &bytes[next..] {
            if !(b'1'..=b'6').contains(&pattern_char) {
                return Err(MoveStrError::InvalidSpreadPattern);
            }

            let dropped = pattern_char - b'0';

            bit <<= dropped;
            pattern |= bit;
        }

        pattern <<= Position::CARRY_LIMIT - taken;

        if (pattern & !((1 << (Position::CARRY_LIMIT + 1)) - 1)) != 0 {
            return Err(MoveStrError::TooManySpreadPieces);
        }

        pattern &= Self::PATTERN_MASK;

        Ok(Self::spread(sq, dir, pattern))
    }
}
