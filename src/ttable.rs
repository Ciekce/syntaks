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

use crate::core::{SCORE_MATE, Score, is_loss, is_win};
use crate::takmove::Move;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU64, Ordering};

pub const DEFAULT_TT_SIZE_MIB: usize = 64;
pub const MAX_TT_SIZE_MIB: usize = 131072;

const ENTRIES_PER_CLUSTER: usize = 3;

const FLAG_BITS: u32 = 2;
const FLAG_MASK: u8 = (1 << FLAG_BITS) - 1;
const AGE_BITS: u32 = 8 - FLAG_BITS;
const AGE_CYCLE: u8 = 1 << AGE_BITS;
const AGE_MASK: u8 = AGE_CYCLE - 1;

const AGE_PENALTY: i32 = 4;
const EMPTY_PENALTY: i32 = 1024;

const DEPTH_OFFSET: i32 = 7;

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
    score: i16,
    raw_static_eval: i16,
    mv: Option<Move>,
    offset_depth: u8,
    age_flag: AgeFlag,
}

impl Entry {
    fn new(
        score: Score,
        static_eval: Score,
        mv: Option<Move>,
        depth: i32,
        ply: i32,
        age: u8,
        flag: Option<TtFlag>,
    ) -> Self {
        Self {
            score: score_to_tt(score, ply),
            raw_static_eval: static_eval as i16,
            mv,
            offset_depth: (depth + DEPTH_OFFSET) as u8,
            age_flag: AgeFlag::new(age, flag),
        }
    }

    #[must_use]
    fn is_present(&self) -> bool {
        self.offset_depth != 0
    }

    #[must_use]
    fn depth(&self) -> i32 {
        self.offset_depth as i32 - DEPTH_OFFSET
    }
}

const _: () = assert!(size_of::<Entry>() == 8);

#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
struct AgeFlag {
    raw: u8,
}

impl AgeFlag {
    #[must_use]
    fn new(age: u8, flag: Option<TtFlag>) -> Self {
        let flag = flag.map(|flag| flag as u8).unwrap_or(0);
        Self {
            raw: (age << FLAG_BITS) | flag,
        }
    }

    #[must_use]
    fn age(self) -> u8 {
        self.raw >> FLAG_BITS
    }

    #[must_use]
    fn flag(self) -> Option<TtFlag> {
        match self.raw & FLAG_MASK {
            0 => None,
            1 => Some(TtFlag::UpperBound),
            2 => Some(TtFlag::LowerBound),
            3 => Some(TtFlag::Exact),
            _ => unreachable!(),
        }
    }
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

#[derive(Debug, Default)]
#[repr(C)]
struct KeyStorage {
    storage: AtomicU64,
}

impl KeyStorage {
    fn find(&self, entry_key: u16) -> Option<usize> {
        const LOW_BITS: u64 = 0x0001_0001_0001_0001;

        let keys = self.storage.load(Ordering::Relaxed);

        let haystack = keys | (entry_key as u64) << (16 * ENTRIES_PER_CLUSTER);
        let zeroes = haystack ^ (entry_key as u64 * LOW_BITS);
        let matches = zeroes.wrapping_sub(LOW_BITS) & !zeroes & (LOW_BITS << 15);

        let slot = matches.trailing_zeros() as usize / 16;
        (slot < ENTRIES_PER_CLUSTER).then_some(slot)
    }

    #[must_use]
    fn get(&self, slot: usize) -> u16 {
        (self.storage.load(Ordering::Relaxed) >> (16 * slot)) as u16
    }

    fn set(&self, slot: usize, entry_key: u16) {
        let shift = 16 * slot;
        let keys = self.storage.load(Ordering::Relaxed);
        self.storage.store(
            keys & !(0xFFFF << shift) | (entry_key as u64) << shift,
            Ordering::Relaxed,
        );
    }
}

#[derive(Debug, Default)]
#[repr(C, align(32))]
struct Cluster {
    entries: [EntryStorage; ENTRIES_PER_CLUSTER],
    keys: KeyStorage,
}

const _: () = assert!(size_of::<Cluster>() == 32);

#[derive(Copy, Clone, Debug, Default)]
pub struct ProbedEntry {
    pub score: Score,
    pub raw_static_eval: Score,
    pub mv: Option<Move>,
    pub depth: i32,
    pub flag: Option<TtFlag>,
}

#[must_use]
fn calc_cluster_count(size_mib: usize) -> usize {
    size_mib * 1024 * 1024 / size_of::<Cluster>()
}

#[must_use]
fn pack_entry_key(key: u64) -> u16 {
    key as u16
}

#[must_use]
fn score_to_tt(score: Score, ply: i32) -> i16 {
    if is_loss(score) {
        (score - ply) as i16
    } else if is_win(score) {
        (score + ply) as i16
    } else {
        score as i16
    }
}

#[must_use]
fn score_from_tt(score: i16, ply: i32) -> Score {
    let score = score as Score;
    if is_loss(score) {
        score + ply
    } else if is_win(score) {
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
    clusters: Vec<Cluster>,
    age: u8,
}

impl TranspositionTable {
    #[must_use]
    pub fn new(size_mib: usize) -> TranspositionTable {
        assert!(size_mib > 0);

        let mut result = Self {
            clusters: Vec::default(),
            age: 0,
        };

        result.resize(size_mib, 1);

        result
    }

    pub fn resize(&mut self, size_mib: usize, threads: usize) {
        assert!(size_mib > 0);
        assert!(threads > 0);

        // ensure the entire old TT is deallocated
        self.clusters = Vec::new();

        let cluster_count = calc_cluster_count(size_mib);
        self.clusters = Vec::with_capacity(cluster_count);

        unsafe {
            //SAFETY: all-zeroes is a valid bitpattern for Cluster, and the range is fully in bounds
            clear_threaded::<Cluster>(self.clusters.as_mut_ptr().cast(), cluster_count, threads);
            //SAFETY: we just initialised these values
            self.clusters.set_len(cluster_count);
        }

        self.age = 0;
    }

    pub fn age(&mut self) {
        self.age = (self.age + 1) & AGE_MASK;
    }

    pub fn prefetch(&self, key: u64) {
        let idx = self.calc_index(key);
        //SAFETY: calc_index() cannot return an out-of-bounds index
        let cluster = unsafe { self.clusters.get_unchecked(idx) };
        let ptr = std::ptr::from_ref(cluster);

        cfg_select! {
            target_arch = "x86_64" => unsafe {
                _mm_prefetch(ptr.cast(), _MM_HINT_T0);
            },
            // taken straight from the compiler
            // https://rust.godbolt.org/z/659YKe9av
            target_arch = "aarch64" => unsafe {
                std::arch::asm!("prfm pldl1keep, [{0}]", in(reg) ptr, options(nostack, preserves_flags, readonly));
            },
            _ => {
                _ = ptr;
            }
        }
    }

    #[must_use]
    pub fn probe(&self, key: u64, ply: i32) -> (bool, ProbedEntry) {
        let idx = self.calc_index(key);
        let entry_key = pack_entry_key(key);

        let mut probed = Default::default();

        //SAFETY: calc_index() cannot return an out-of-bounds index
        let cluster = unsafe { self.clusters.get_unchecked(idx) };

        let Some(slot) = cluster.keys.find(entry_key) else {
            return (false, probed);
        };

        let entry = cluster.entries[slot].load();

        if !entry.is_present() {
            return (false, probed);
        }

        probed.score = score_from_tt(entry.score, ply);
        probed.raw_static_eval = entry.raw_static_eval as Score;
        probed.mv = entry.mv;
        probed.depth = entry.depth();
        probed.flag = entry.age_flag.flag();

        (true, probed)
    }

    #[allow(clippy::too_many_arguments)]
    fn store_internal(
        &self,
        key: u64,
        score: Score,
        static_eval: Score,
        mv: Option<Move>,
        depth: i32,
        ply: i32,
        flag: Option<TtFlag>,
    ) {
        let idx = self.calc_index(key);
        let entry_key = pack_entry_key(key);

        //SAFETY: calc_index() cannot return an out-of-bounds index
        let cluster = unsafe { self.clusters.get_unchecked(idx) };

        let slot = self.write_slot(cluster, entry_key);

        let existing_key = cluster.keys.get(slot);
        let existing_entry = cluster.entries[slot].load();

        let replace = flag == Some(TtFlag::Exact)
            || entry_key != existing_key
            || existing_entry.age_flag.age() != self.age
            || depth + 4 > existing_entry.depth();

        if !replace {
            return;
        }

        let mv = if existing_key == entry_key {
            mv.or_else(|| existing_entry.mv)
        } else {
            mv
        };

        let new_entry = Entry::new(score, static_eval, mv, depth, ply, self.age, flag);

        cluster.entries[slot].store(new_entry);
        cluster.keys.set(slot, entry_key);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn store(
        &self,
        key: u64,
        score: Score,
        static_eval: Score,
        mv: Option<Move>,
        depth: i32,
        ply: i32,
        flag: TtFlag,
    ) {
        self.store_internal(key, score, static_eval, mv, depth, ply, Some(flag));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn store_static_eval(&self, key: u64, static_eval: Score) {
        const STATIC_EVAL_DEPTH: i32 = -DEPTH_OFFSET + 1;
        self.store_internal(key, SCORE_MATE, static_eval, None, STATIC_EVAL_DEPTH, 0, None);
    }

    #[must_use]
    fn write_slot(&self, cluster: &Cluster, entry_key: u16) -> usize {
        if let Some(slot) = cluster.keys.find(entry_key) {
            return slot;
        }

        let value = |storage: &EntryStorage| {
            let entry = storage.load();
            entry.depth()
                - AGE_PENALTY * self.relative_age(entry.age_flag)
                - EMPTY_PENALTY * i32::from(!entry.is_present())
        };

        (0..ENTRIES_PER_CLUSTER)
            .min_by_key(|&slot| value(&cluster.entries[slot]))
            .unwrap()
    }

    pub fn clear(&mut self, threads: usize) {
        assert!(threads > 0);

        //SAFETY: all-zeroes is a valid bitpattern for Cluster, and the range is fully in bounds
        unsafe { clear_threaded::<Cluster>(self.clusters.as_mut_ptr().cast(), self.clusters.len(), threads) };

        self.age = 0;
    }

    #[must_use]
    pub fn estimate_full_permille(&self) -> usize {
        let mut filled = 0;

        for i in 0..1000 {
            let entry = self.clusters[i / ENTRIES_PER_CLUSTER].entries[i % ENTRIES_PER_CLUSTER].load();
            if entry.is_present() && entry.age_flag.age() == self.age {
                filled += 1;
            }
        }

        filled
    }

    #[must_use]
    fn relative_age(&self, age_flag: AgeFlag) -> i32 {
        ((AGE_CYCLE + self.age - age_flag.age()) & AGE_MASK) as i32
    }

    #[must_use]
    fn calc_index(&self, key: u64) -> usize {
        ((key as u128 * self.clusters.len() as u128) >> 64) as usize
    }
}
