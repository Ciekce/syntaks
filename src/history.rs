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
use std::cell::Cell;
use std::ops::Index;

#[derive(Clone, Debug, Default)]
#[repr(C)]
struct Entry {
    value: Cell<i16>,
}

impl Entry {
    const LIMIT: i32 = 16384;

    const fn new() -> Self {
        Self { value: Cell::new(0) }
    }

    fn update(&self, bonus: i32) {
        let mut value = self.value.get() as i32;
        value += bonus - value * bonus.abs() / Self::LIMIT;
        self.value.set(value as i16);
    }

    #[must_use]
    fn get(&self) -> i32 {
        self.value.get() as i32
    }
}

#[derive(Clone)]
struct CombinedHist {
    entries: [Entry; Self::ENTRIES],
}

impl CombinedHist {
    const ENTRIES: usize = 1 << Move::TOTAL_BITS;

    const fn new() -> Self {
        Self {
            entries: [const { Entry::new() }; Self::ENTRIES],
        }
    }

    fn clear(&self) {
        self.entries.iter().for_each(|entry| entry.value.set(0));
    }
}

impl Default for CombinedHist {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<Move> for CombinedHist {
    type Output = Entry;

    fn index(&self, index: Move) -> &Self::Output {
        &self.entries[index.raw() as usize]
    }
}

#[derive(Clone)]
struct ConthistSubTable {
    entries: [Entry; ConthistTable::ENTRIES],
}

impl ConthistSubTable {
    const fn new() -> Self {
        Self {
            entries: [const { Entry::new() }; _],
        }
    }

    fn clear(&self) {
        self.entries.iter().for_each(|entry| entry.value.set(0));
    }
}

impl Default for ConthistSubTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<Move> for ConthistSubTable {
    type Output = Entry;

    fn index(&self, mv: Move) -> &Self::Output {
        &self.entries[ConthistTable::move_idx(mv)]
    }
}

#[derive(Clone)]
struct ConthistTable {
    entries: [ConthistSubTable; Self::ENTRIES],
}

impl ConthistTable {
    // one for each placement type, and 4 spread directions
    const MOVE_TYPES: usize = PieceType::COUNT + Direction::COUNT;
    const ENTRIES: usize = Self::MOVE_TYPES * Square::COUNT;

    const fn new() -> Self {
        Self {
            entries: [const { ConthistSubTable::new() }; _],
        }
    }

    #[must_use]
    fn move_idx(mv: Move) -> usize {
        let type_idx = if mv.is_spread() {
            mv.dir().idx()
        } else {
            mv.pt().idx() + Direction::COUNT
        };
        type_idx * Square::COUNT + mv.sq().idx()
    }

    fn clear(&self) {
        for subtable in self.entries.iter() {
            subtable.clear();
        }
    }
}

impl Default for ConthistTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<Move> for ConthistTable {
    type Output = ConthistSubTable;

    fn index(&self, mv: Move) -> &Self::Output {
        &self.entries[Self::move_idx(mv)]
    }
}

#[derive(Clone)]
struct InfluenceHist {
    entries: [[Entry; Square::COUNT]; Self::BUCKETS],
}

impl InfluenceHist {
    const BITS: u32 = 10;
    const BUCKETS: usize = 1 << Self::BITS;

    const fn new() -> Self {
        Self {
            entries: [const { [const { Entry::new() }; Square::COUNT] }; Self::BUCKETS],
        }
    }

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

    fn clear(&self) {
        for bucket in self.entries.iter() {
            bucket.iter().for_each(|entry| entry.value.set(0));
        }
    }
}

impl Default for InfluenceHist {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct CapstoneHist {
    entries: [CombinedHist; Square::COUNT],
}

impl CapstoneHist {
    const fn new() -> Self {
        Self {
            entries: [const { CombinedHist::new() }; Square::COUNT],
        }
    }

    fn clear(&self) {
        for subtable in self.entries.iter() {
            subtable.clear();
        }
    }
}

impl Default for CapstoneHist {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<Square> for CapstoneHist {
    type Output = CombinedHist;

    fn index(&self, square: Square) -> &Self::Output {
        &self.entries[square]
    }
}

#[derive(Clone, Default)]
struct SidedTables {
    hist: CombinedHist,
    capstone_hist: CapstoneHist,
    conthist: ConthistTable,
    influence: InfluenceHist,
}

impl SidedTables {
    fn clear(&self) {
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

    pub fn clear(&self) {
        for table in self.tables.iter() {
            table.clear();
        }
    }

    pub fn hist_slice<'a>(&'a self, pos: &Position, prev: Option<Move>) -> HistorySlice<'a> {
        let tables = &self.tables[pos.stm()];
        let bucket = InfluenceHist::bucket(pos.influence(pos.stm()));
        HistorySlice {
            hist: &tables.hist,
            capstone_hist: pos.cap_sq(pos.stm()).map(|cap_sq| &tables.capstone_hist[cap_sq]),
            conthist: prev.map(|prev| &tables.conthist[prev]),
            influence: &tables.influence.entries[bucket],
        }
    }
}

#[derive(Clone, Copy)]
pub struct HistorySlice<'a> {
    hist: &'a CombinedHist,
    capstone_hist: Option<&'a CombinedHist>,
    conthist: Option<&'a ConthistSubTable>,
    influence: &'a [Entry; Square::COUNT],
}

impl<'a> HistorySlice<'a> {
    pub fn score(&self, mv: Move) -> i32 {
        let mut res = self.hist[mv].get();
        if let Some(capstone_hist) = self.capstone_hist {
            res = (res + capstone_hist[mv].get()) / 2;
        }
        if !mv.is_spread() {
            res += self.influence[mv.sq().idx()].get();
        }
        if let Some(conthist) = self.conthist {
            res += conthist[mv].get();
        }
        res
    }

    pub fn update(&self, mv: Move, bonus: i32) {
        let bonus = bonus.clamp(-History::MAX_BONUS, History::MAX_BONUS);
        self.hist[mv].update(bonus);
        if let Some(capstone_hist) = self.capstone_hist {
            capstone_hist[mv].update(bonus);
        }
        if !mv.is_spread() {
            self.influence[mv.sq().idx()].update(bonus);
        }
        if let Some(conthist) = self.conthist {
            conthist[mv].update(bonus);
        }
    }
}
