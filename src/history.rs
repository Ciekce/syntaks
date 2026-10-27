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
use crate::core::{Color, Direction, Square};
use crate::takmove::Move;
use crate::{core::PieceType, position::Position};
use std::ops::{Index, IndexMut};

#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
struct Entry {
    value: i16,
}

impl Entry {
    const LIMIT: i32 = 16384;

    fn update(&mut self, bonus: i32) {
        let mut value = self.value as i32;
        value += bonus - value * bonus.abs() / Self::LIMIT;
        self.value = value as i16;
    }

    #[must_use]
    fn get(&self) -> i32 {
        self.value as i32
    }
}

#[derive(Copy, Clone)]
struct CombinedHist {
    entries: [Entry; Self::ENTRIES],
}

impl CombinedHist {
    const ENTRIES: usize = 1 << Move::TOTAL_BITS;

    fn clear(&mut self) {
        self.entries.fill(Default::default());
    }
}

impl Default for CombinedHist {
    fn default() -> Self {
        Self {
            entries: [Default::default(); Self::ENTRIES],
        }
    }
}

impl Index<Move> for CombinedHist {
    type Output = Entry;

    fn index(&self, index: Move) -> &Self::Output {
        &self.entries[index.raw() as usize]
    }
}

impl IndexMut<Move> for CombinedHist {
    fn index_mut(&mut self, index: Move) -> &mut Self::Output {
        &mut self.entries[index.raw() as usize]
    }
}

#[derive(Copy, Clone)]
struct ConthistSubTable {
    entries: [Entry; ConthistTable::ENTRIES],
}

impl ConthistSubTable {
    fn clear(&mut self) {
        self.entries.fill(Default::default());
    }
}

impl Default for ConthistSubTable {
    fn default() -> Self {
        Self {
            entries: [Default::default(); _],
        }
    }
}

impl Index<Move> for ConthistSubTable {
    type Output = Entry;

    fn index(&self, mv: Move) -> &Self::Output {
        &self.entries[ConthistTable::move_idx(mv)]
    }
}

impl IndexMut<Move> for ConthistSubTable {
    fn index_mut(&mut self, mv: Move) -> &mut Self::Output {
        &mut self.entries[ConthistTable::move_idx(mv)]
    }
}

#[derive(Copy, Clone)]
struct ConthistTable {
    entries: [ConthistSubTable; Self::ENTRIES],
}

impl ConthistTable {
    // one for each placement type, and 4 spread directions
    const MOVE_TYPES: usize = PieceType::COUNT + Direction::COUNT;
    const ENTRIES: usize = Self::MOVE_TYPES * Square::COUNT;

    #[must_use]
    fn move_idx(mv: Move) -> usize {
        let type_idx = if mv.is_spread() {
            mv.dir().idx()
        } else {
            mv.pt().idx() + Direction::COUNT
        };
        type_idx * Square::COUNT + mv.sq().idx()
    }

    fn clear(&mut self) {
        for subtable in self.entries.iter_mut() {
            subtable.clear();
        }
    }
}

impl Default for ConthistTable {
    fn default() -> Self {
        Self {
            entries: [Default::default(); _],
        }
    }
}

impl Index<Move> for ConthistTable {
    type Output = ConthistSubTable;

    fn index(&self, mv: Move) -> &Self::Output {
        &self.entries[Self::move_idx(mv)]
    }
}

impl IndexMut<Move> for ConthistTable {
    fn index_mut(&mut self, mv: Move) -> &mut Self::Output {
        &mut self.entries[Self::move_idx(mv)]
    }
}

#[derive(Copy, Clone)]
struct InfluenceHist {
    entries: [[Entry; Square::COUNT]; Self::BUCKETS],
}

impl InfluenceHist {
    const BITS: u32 = 10;
    const BUCKETS: usize = 1 << Self::BITS;

    fn bucket(influence: &[Bitboard; 4]) -> usize {
        let mut hash = influence
            .iter()
            .fold(0u64, |hash, bb| (hash ^ bb.raw()).wrapping_mul(3708513320411795339));
        // splitmix64
        hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
        hash ^= hash >> 31;
        (hash >> (64 - Self::BITS)) as usize
    }

    fn clear(&mut self) {
        for bucket in self.entries.iter_mut() {
            bucket.fill(Default::default());
        }
    }
}

impl Default for InfluenceHist {
    fn default() -> Self {
        Self {
            entries: [[Default::default(); Square::COUNT]; Self::BUCKETS],
        }
    }
}

#[derive(Copy, Clone)]
struct CapstoneHist {
    entries: [[Entry; Self::ENTRIES]; Square::COUNT],
}

impl CapstoneHist {
    const ENTRIES: usize = 1 << Move::TOTAL_BITS;

    fn clear(&mut self) {
        self.entries
            .iter_mut()
            .for_each(|entries| entries.fill(Default::default()));
    }
}

impl Default for CapstoneHist {
    fn default() -> Self {
        Self {
            entries: [[Default::default(); Self::ENTRIES]; Square::COUNT],
        }
    }
}

impl Index<(Square, Move)> for CapstoneHist {
    type Output = Entry;

    fn index(&self, (square, mv): (Square, Move)) -> &Self::Output {
        &self.entries[square][mv.raw() as usize]
    }
}

impl IndexMut<(Square, Move)> for CapstoneHist {
    fn index_mut(&mut self, (square, mv): (Square, Move)) -> &mut Self::Output {
        &mut self.entries[square][mv.raw() as usize]
    }
}

#[derive(Copy, Clone, Default)]
struct SidedTables {
    hist: CombinedHist,
    capstone_hist: CapstoneHist,
    conthist: ConthistTable,
    influence: InfluenceHist,
}

impl SidedTables {
    fn clear(&mut self) {
        self.hist.clear();
        self.capstone_hist.clear();
        self.conthist.clear();
        self.influence.clear();
    }
}

pub struct History {
    tables: [SidedTables; Color::COUNT],
}

impl History {
    const MAX_BONUS: i32 = Entry::LIMIT / 4;

    #[must_use]
    pub fn boxed() -> Box<Self> {
        //SAFETY: history tables are all just u16s,
        // for which all-zeroes is a valid bitpattern
        unsafe { Box::new_zeroed().assume_init() }
    }

    pub fn clear(&mut self) {
        for table in self.tables.iter_mut() {
            table.clear();
        }
    }

    pub fn update(&mut self, pos: &Position, mv: Move, prev: Option<Move>, bonus: i32) {
        let tables = &mut self.tables[pos.stm()];
        let bonus = bonus.clamp(-Self::MAX_BONUS, Self::MAX_BONUS);
        tables.hist[mv].update(bonus);
        if let Some(cap_sq) = pos.cap_sq(pos.stm()) {
            tables.capstone_hist[(cap_sq, mv)].update(bonus);
        }
        if !mv.is_spread() {
            let bucket = InfluenceHist::bucket(pos.influence(pos.stm()));
            tables.influence.entries[bucket][mv.sq().idx()].update(bonus);
        }
        if let Some(prev) = prev {
            tables.conthist[prev][mv].update(bonus);
        }
    }

    #[must_use]
    pub fn score(&self, pos: &Position, mv: Move, prev: Option<Move>) -> i32 {
        let tables = &self.tables[pos.stm()];
        let mut res = tables.hist[mv].get();
        if let Some(cap_sq) = pos.cap_sq(pos.stm()) {
            res = (res + tables.capstone_hist[(cap_sq, mv)].get()) / 2
        }
        if !mv.is_spread() {
            let bucket = InfluenceHist::bucket(pos.influence(pos.stm()));
            res += tables.influence.entries[bucket][mv.sq().idx()].get();
        }
        if let Some(prev) = prev {
            res += tables.conthist[prev][mv].get();
        }
        res
    }
}
