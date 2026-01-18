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
use crate::takmove::Move;
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
struct HashedTable {
    entries: [CombinedHist; Self::ENTRIES],
}

impl HashedTable {
    const ENTRIES: usize = 512;

    fn clear(&mut self) {
        self.entries.fill(Default::default());
    }
}

impl Default for HashedTable {
    fn default() -> Self {
        Self {
            entries: [Default::default(); Self::ENTRIES],
        }
    }
}

impl Index<u64> for HashedTable {
    type Output = CombinedHist;

    fn index(&self, index: u64) -> &Self::Output {
        &self.entries[index as usize % Self::ENTRIES]
    }
}

impl IndexMut<u64> for HashedTable {
    fn index_mut(&mut self, index: u64) -> &mut Self::Output {
        &mut self.entries[index as usize % Self::ENTRIES]
    }
}

#[derive(Copy, Clone, Default)]
struct SidedTables {
    hist: CombinedHist,
    blocker: HashedTable,
}

impl SidedTables {
    fn clear(&mut self) {
        self.hist.clear();
        self.blocker.clear();
    }
}

pub struct History {
    tables: [SidedTables; Player::COUNT],
}

impl History {
    const MAX_BONUS: i32 = Entry::LIMIT / 4;

    pub fn boxed() -> Box<Self> {
        //SAFETY: history tables are ultimately just a load
        // of i16s, for which all-zeroes is a valid bit pattern
        unsafe {
            let layout = std::alloc::Layout::new::<Self>();
            let ptr = std::alloc::alloc_zeroed(layout);
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            Box::from_raw(ptr.cast())
        }
    }

    pub fn clear(&mut self) {
        for table in self.tables.iter_mut() {
            table.clear();
        }
    }

    pub fn update(&mut self, pos: &Position, mv: Move, bonus: i32) {
        let bonus = bonus.clamp(-Self::MAX_BONUS, Self::MAX_BONUS);

        let tables = &mut self.tables[pos.stm().idx()];

        tables.hist[mv].update(bonus);
        tables.blocker[pos.blocker_key()][mv].update(bonus);
    }

    #[must_use]
    pub fn score(&self, pos: &Position, mv: Move) -> i32 {
        let tables = &self.tables[pos.stm().idx()];

        let mut history = 0;

        history += tables.hist[mv].get();
        history += tables.blocker[pos.blocker_key()][mv].get();

        history
    }
}
