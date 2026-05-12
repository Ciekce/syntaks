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
use crate::core::Player;
use crate::search::Score;
use std::ops::{Index, IndexMut};

#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
struct Entry {
    value: i16,
}

impl Entry {
    const LIMIT: i32 = 1024;

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
struct HashedTable<const ENTRIES: usize> {
    entries: [Entry; ENTRIES],
}

impl<const ENTRIES: usize> HashedTable<ENTRIES> {
    fn clear(&mut self) {
        self.entries.fill(Default::default());
    }
}

impl<const ENTRIES: usize> Default for HashedTable<ENTRIES> {
    fn default() -> Self {
        Self {
            entries: [Default::default(); _],
        }
    }
}

impl<const ENTRIES: usize> Index<u64> for HashedTable<ENTRIES> {
    type Output = Entry;

    fn index(&self, index: u64) -> &Self::Output {
        &self.entries[index as usize % ENTRIES]
    }
}

impl<const ENTRIES: usize> IndexMut<u64> for HashedTable<ENTRIES> {
    fn index_mut(&mut self, index: u64) -> &mut Self::Output {
        &mut self.entries[index as usize % ENTRIES]
    }
}

const ENTRIES: usize = 16384;
const CONT_ENTRIES: usize = 32768;

#[derive(Copy, Clone, Default)]
struct SidedTables {
    blocker: HashedTable<ENTRIES>,
    road: HashedTable<ENTRIES>,
    tops: HashedTable<ENTRIES>,
    cap: HashedTable<ENTRIES>,
    wall: HashedTable<ENTRIES>,
}

impl SidedTables {
    fn clear(&mut self) {
        self.blocker.clear();
        self.road.clear();
        self.tops.clear();
        self.cap.clear();
        self.wall.clear();
    }
}

pub struct CorrectionHistory {
    tables: [SidedTables; Player::COUNT],
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

    pub fn clear(&mut self) {
        for table in self.tables.iter_mut() {
            table.clear();
        }
        self.cont.clear();
    }

    pub fn update(&mut self, pos: &Position, key_history: &[u64], depth: i32, search_score: Score, static_eval: Score) {
        let bonus = ((search_score - static_eval) * depth / 8).clamp(-Self::MAX_BONUS, Self::MAX_BONUS);

        let mut update_cont = |offset: usize| {
            if key_history.len() >= offset {
                let diff = pos.key() ^ key_history[key_history.len() - offset];
                self.cont[diff].update(bonus);
            }
        };

        let tables = &mut self.tables[pos.stm().idx()];

        tables.blocker[pos.blocker_key()].update(bonus);
        tables.road[pos.road_key()].update(bonus);
        tables.tops[pos.top_key()].update(bonus);
        tables.cap[pos.cap_key()].update(bonus);
        tables.wall[pos.wall_key()].update(bonus);

        update_cont(1);
    }

    #[must_use]
    pub fn correction(&self, pos: &Position, key_history: &[u64]) -> i32 {
        let tables = &self.tables[pos.stm().idx()];

        let cont = |offset: usize| {
            if key_history.len() >= offset {
                let diff = pos.key() ^ key_history[key_history.len() - offset];
                self.cont[diff].get()
            } else {
                0
            }
        };

        let mut correction = 0;

        correction += tables.blocker[pos.blocker_key()].get();
        correction += tables.road[pos.road_key()].get();
        correction += tables.tops[pos.top_key()].get();
        correction += tables.cap[pos.cap_key()].get();
        correction += tables.wall[pos.wall_key()].get();

        correction += cont(1);

        correction / 16
    }
}
