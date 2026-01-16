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

use crate::search::{SCORE_WIN, Score};
use crate::takmove::Move;

pub const DEFAULT_TT_SIZE_MIB: usize = 64;
pub const MAX_TT_SIZE_MIB: usize = 131072;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TtFlag {
    UpperBound = 1,
    LowerBound,
    Exact,
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
struct Entry {
    key: u16,
    score: i16,
    mv: Option<Move>,
    depth: u8,
    flag: Option<TtFlag>,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct ProbedEntry {
    pub score: Score,
    pub mv: Option<Move>,
    pub depth: i32,
    pub flag: Option<TtFlag>,
}

#[must_use]
fn calc_entry_count(size_mib: usize) -> usize {
    size_mib * 1024 * 1024 / size_of::<Entry>()
}

#[must_use]
fn pack_entry_key(key: u64) -> u16 {
    key as u16
}

#[must_use]
fn score_to_tt(score: Score, ply: i32) -> i16 {
    if score < -SCORE_WIN {
        (score - ply) as i16
    } else if score > SCORE_WIN {
        (score + ply) as i16
    } else {
        score as i16
    }
}

#[must_use]
fn score_from_tt(score: i16, ply: i32) -> Score {
    let score = score as Score;
    if score < -SCORE_WIN {
        score + ply
    } else if score > SCORE_WIN {
        score - ply
    } else {
        score
    }
}

pub struct TranspositionTable {
    entries: Vec<Entry>,
}

impl TranspositionTable {
    pub fn new(size_mib: usize) -> TranspositionTable {
        assert!(size_mib > 0);

        let mut result = Self {
            entries: Vec::default(),
        };

        result.resize(size_mib);

        result
    }

    pub fn resize(&mut self, size_mib: usize) {
        self.entries.clear();
        self.entries.shrink_to_fit();

        let entry_count = calc_entry_count(size_mib);
        self.entries.resize(entry_count, Default::default());

        self.clear();
    }

    pub fn probe(&self, key: u64, ply: i32) -> (bool, ProbedEntry) {
        let idx = self.calc_index(key);
        let entry_key = pack_entry_key(key);

        let mut probed = Default::default();

        //SAFETY: index() cannot return an out-of-bounds index
        let entry = *unsafe { self.entries.get_unchecked(idx) };

        if entry.key != entry_key {
            return (false, probed);
        }

        probed.score = score_from_tt(entry.score, ply);
        probed.mv = entry.mv;
        probed.depth = entry.depth as i32;
        probed.flag = entry.flag;

        (true, probed)
    }

    pub fn store(
        &mut self,
        key: u64,
        score: Score,
        mv: Option<Move>,
        depth: i32,
        ply: i32,
        flag: TtFlag,
    ) {
        let idx = self.calc_index(key);
        let entry_key = pack_entry_key(key);

        //SAFETY: index() cannot return an out-of-bounds index
        let mut entry = *unsafe { self.entries.get_unchecked(idx) };

        if mv.is_some() || entry.key != entry_key {
            entry.mv = mv;
        }

        entry.key = entry_key;
        entry.score = score_to_tt(score, ply);
        entry.depth = depth as u8;
        entry.flag = Some(flag);

        *unsafe { self.entries.get_unchecked_mut(idx) } = entry;
    }

    pub fn clear(&mut self) {
        self.entries.fill(Default::default());
    }

    pub fn estimate_full_permille(&self) -> usize {
        let mut filled = 0;

        for &entry in self.entries[0..1000].iter() {
            if entry.flag.is_some() {
                filled += 1;
            }
        }

        filled
    }

    #[must_use]
    fn calc_index(&self, key: u64) -> usize {
        ((key as u128 * self.entries.len() as u128) >> 64) as usize
    }
}
