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

use crate::core::{Color, Score};
use crate::position::Position;
use std::cell::Cell;
use std::ops::Index;

#[derive(Clone, Debug, Default)]
#[repr(C)]
struct Entry {
    value: Cell<i16>,
}

impl Entry {
    const LIMIT: i32 = 1024;

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
struct HashedTable<const ENTRIES: usize> {
    entries: [Entry; ENTRIES],
}

impl<const ENTRIES: usize> HashedTable<ENTRIES> {
    const fn new() -> Self {
        Self {
            entries: [const { Entry::new() }; _],
        }
    }

    fn clear(&self) {
        self.entries.iter().for_each(|entry| entry.value.set(0));
    }
}

impl<const ENTRIES: usize> Default for HashedTable<ENTRIES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const ENTRIES: usize> Index<u64> for HashedTable<ENTRIES> {
    type Output = Entry;

    fn index(&self, index: u64) -> &Self::Output {
        &self.entries[index as usize % ENTRIES]
    }
}

const ENTRIES: usize = 16384;
const CONT_ENTRIES: usize = 32768;
const RESERVE_ENTRIES: usize = 32 * 32 * 4;

#[derive(Clone, Default)]
struct SidedTables {
    blocker: HashedTable<ENTRIES>,
    road: HashedTable<ENTRIES>,
    tops: HashedTable<ENTRIES>,
    reserves: HashedTable<RESERVE_ENTRIES>,
}

impl SidedTables {
    fn clear(&self) {
        self.blocker.clear();
        self.road.clear();
        self.tops.clear();
        self.reserves.clear();
    }
}

fn reserves_key(pos: &Position) -> u64 {
    let us = pos.stm();
    let them = us.flip();
    let flats = pos.flats_in_hand(us) as u64 * 32 + pos.flats_in_hand(them) as u64;
    let caps = pos.caps_in_hand(us) as u64 * 2 + pos.caps_in_hand(them) as u64;
    flats * 4 + caps
}

pub struct CorrectionHistory {
    tables: [SidedTables; Color::COUNT],
    cont: HashedTable<CONT_ENTRIES>,
}

impl CorrectionHistory {
    const MAX_BONUS: i32 = Entry::LIMIT / 4;

    #[must_use]
    pub fn boxed() -> Box<Self> {
        //SAFETY: corrhist tables are all just u16s,
        // for which all-zeroes is a valid bitpattern
        unsafe { Box::new_zeroed().assume_init() }
    }

    pub fn clear(&self) {
        for table in self.tables.iter() {
            table.clear();
        }
        self.cont.clear();
    }

    pub fn slice<'a>(&'a self, pos: &Position, key_history: &[u64]) -> CorrectionHistorySlice<'a> {
        let get_cont = |offset: usize| {
            if key_history.len() >= offset {
                let diff = pos.key() ^ key_history[key_history.len() - offset];
                Some(&self.cont[diff])
            } else {
                None
            }
        };

        let tables = &self.tables[pos.stm()];
        CorrectionHistorySlice {
            blocker: &tables.blocker[pos.blocker_key()],
            road: &tables.road[pos.road_key()],
            tops: &tables.tops[pos.top_key()],
            reserves: &tables.reserves[reserves_key(pos)],
            cont_1: get_cont(1),
            cont_2: get_cont(2),
        }
    }
}

#[derive(Copy, Clone)]
pub struct CorrectionHistorySlice<'a> {
    blocker: &'a Entry,
    road: &'a Entry,
    tops: &'a Entry,
    reserves: &'a Entry,
    cont_1: Option<&'a Entry>,
    cont_2: Option<&'a Entry>,
}

impl<'a> CorrectionHistorySlice<'a> {
    pub fn get(&self) -> i32 {
        let mut correction = 0;
        correction += self.blocker.get();
        correction += self.road.get();
        correction += self.tops.get();
        correction += self.reserves.get();

        correction += self.cont_1.map_or(0, Entry::get);
        correction += self.cont_2.map_or(0, Entry::get);

        correction / 32
    }

    pub fn update(&self, depth: i32, search_score: Score, static_eval: Score) {
        let bonus = ((search_score - static_eval) * depth / 8)
            .clamp(-CorrectionHistory::MAX_BONUS, CorrectionHistory::MAX_BONUS);

        self.blocker.update(bonus);
        self.road.update(bonus);
        self.tops.update(bonus);
        self.reserves.update(bonus);
        if let Some(cont_1) = self.cont_1 {
            cont_1.update(bonus);
        }
        if let Some(cont_2) = self.cont_2 {
            cont_2.update(bonus);
        }
    }
}
