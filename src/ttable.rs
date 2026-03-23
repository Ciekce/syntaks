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
use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU64, Ordering};

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

#[derive(Debug, Default)]
#[repr(C)]
struct EntryStorage {
    storage: AtomicU64,
}

impl EntryStorage {
    fn load(&self) -> Entry {
        let value = self.storage.load(Ordering::Relaxed);
        unsafe { std::mem::transmute::<u64, Entry>(value) }
    }

    fn store(&self, entry: Entry) {
        let value = unsafe { std::mem::transmute::<Entry, u64>(entry) };
        self.storage.store(value, Ordering::Relaxed);
    }
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

//SAFETY: all-zeroes must be a valid bit pattern for T, and [ptr, ptr+len) must be in bounds
unsafe fn clear_threaded<T: Send>(ptr: *mut MaybeUninit<T>, len: usize, threads: usize) {
    assert!(threads > 0);

    if len == 0 {
        return;
    }

    let threads = threads.min(len);
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr, len) };

    if threads == 1 {
        unsafe { slice.as_mut_ptr().write_bytes(0, slice.len()) };
        return;
    }

    let chunk_count = len.div_ceil(threads);

    std::thread::scope(|s| {
        for chunk in slice.chunks_mut(chunk_count) {
            s.spawn(|| unsafe { chunk.as_mut_ptr().write_bytes(0, chunk.len()) });
        }
    });
}

pub struct TranspositionTable {
    entries: Vec<EntryStorage>,
}

impl TranspositionTable {
    #[must_use]
    pub fn new(size_mib: usize) -> TranspositionTable {
        assert!(size_mib > 0);

        let mut result = Self {
            entries: Vec::default(),
        };

        result.resize(size_mib, 1);

        result
    }

    pub fn resize(&mut self, size_mib: usize, threads: usize) {
        assert!(size_mib > 0);
        assert!(threads > 0);

        // ensure the entire old TT is deallocated
        self.entries = Vec::new();

        let entry_count = calc_entry_count(size_mib);
        self.entries = Vec::with_capacity(entry_count);

        unsafe {
            //SAFETY: all-zeroes is a valid bitpattern for EntryStorage, and the range is fully in bounds
            clear_threaded::<EntryStorage>(self.entries.as_mut_ptr().cast(), entry_count, threads);
            //SAFETY: we just initialised these values
            self.entries.set_len(entry_count);
        }
    }

    pub fn prefetch(&self, key: u64) {
        #[cfg(target_arch = "x86_64")]
        {
            let idx = self.calc_index(key);
            //SAFETY: calc_index() cannot return an out-of-bounds index
            let entry = unsafe { self.entries.get_unchecked(idx) };
            let ptr = std::ptr::from_ref(entry).cast();
            unsafe { _mm_prefetch(ptr, _MM_HINT_T0) };
        }
    }

    #[must_use]
    pub fn probe(&self, key: u64, ply: i32) -> (bool, ProbedEntry) {
        let idx = self.calc_index(key);
        let entry_key = pack_entry_key(key);

        let mut probed = Default::default();

        //SAFETY: calc_index() cannot return an out-of-bounds index
        let entry = unsafe { self.entries.get_unchecked(idx) }.load();

        if entry.key != entry_key {
            return (false, probed);
        }

        probed.score = score_from_tt(entry.score, ply);
        probed.mv = entry.mv;
        probed.depth = entry.depth as i32;
        probed.flag = entry.flag;

        (true, probed)
    }

    pub fn store(&self, key: u64, score: Score, mv: Option<Move>, depth: i32, ply: i32, flag: TtFlag) {
        let idx = self.calc_index(key);
        let entry_key = pack_entry_key(key);

        //SAFETY: calc_index() cannot return an out-of-bounds index
        let storage = unsafe { self.entries.get_unchecked(idx) };

        let mut entry = storage.load();

        if mv.is_some() || entry.key != entry_key {
            entry.mv = mv;
        }

        entry.key = entry_key;
        entry.score = score_to_tt(score, ply);
        entry.depth = depth as u8;
        entry.flag = Some(flag);

        storage.store(entry);
    }

    pub fn clear(&mut self, threads: usize) {
        assert!(threads > 0);

        //SAFETY: all-zeroes is a valid bitpattern for EntryStorage, and the range is fully in bounds
        unsafe { clear_threaded::<EntryStorage>(self.entries.as_mut_ptr().cast(), self.entries.len(), threads) };
    }

    #[must_use]
    pub fn estimate_full_permille(&self) -> usize {
        let mut filled = 0;

        for storage in self.entries[0..1000].iter() {
            let entry = storage.load();
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
