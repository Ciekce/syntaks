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

#![allow(unused)]

use std::io::Write;
use std::io::stdout;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

const SLOTS: usize = 32;

struct TrackedCondition {
    counts: [AtomicU64; 2],
}

impl TrackedCondition {
    #[must_use]
    const fn new() -> Self {
        Self {
            counts: [AtomicU64::new(0), AtomicU64::new(0)],
        }
    }

    fn update(&self, v: bool) {
        self.counts[usize::from(v)].fetch_add(1, Ordering::SeqCst);
    }

    fn get(&self) -> (u64, u64) {
        (
            self.counts[0].load(Ordering::Relaxed),
            self.counts[1].load(Ordering::Relaxed),
        )
    }
}

struct TrackedRange {
    min: AtomicI64,
    max: AtomicI64,
}

impl TrackedRange {
    #[must_use]
    const fn new() -> Self {
        Self {
            min: AtomicI64::new(i64::MAX),
            max: AtomicI64::new(i64::MIN),
        }
    }

    fn update(&self, v: i64) {
        self.min.fetch_min(v, Ordering::SeqCst);
        self.max.fetch_max(v, Ordering::SeqCst);
    }

    fn get(&self) -> (i64, i64) {
        (self.min.load(Ordering::Relaxed), self.max.load(Ordering::Relaxed))
    }
}

struct TrackedMean {
    total: AtomicI64,
    count: AtomicU64,
}

impl TrackedMean {
    #[must_use]
    const fn new() -> Self {
        Self {
            total: AtomicI64::new(0),
            count: AtomicU64::new(0),
        }
    }

    fn update(&self, v: i64) {
        self.total.fetch_add(v, Ordering::SeqCst);
        self.count.fetch_add(1, Ordering::SeqCst);
    }

    fn get(&self) -> (i64, u64) {
        (self.total.load(Ordering::Relaxed), self.count.load(Ordering::Relaxed))
    }
}

static ANY_USED: AtomicBool = AtomicBool::new(false);

static CONDITION_HITS: [TrackedCondition; SLOTS] = [const { TrackedCondition::new() }; _];
static RANGES: [TrackedRange; SLOTS] = [const { TrackedRange::new() }; _];
static MEANS: [TrackedMean; SLOTS] = [const { TrackedMean::new() }; _];

pub fn condition_hit(slot: usize, value: bool) {
    ANY_USED.store(true, Ordering::Relaxed);
    CONDITION_HITS[slot].update(value);
}

pub fn range(slot: usize, value: impl Into<i64>) {
    ANY_USED.store(true, Ordering::Relaxed);
    RANGES[slot].update(value.into());
}

pub fn mean(slot: usize, value: impl Into<i64>) {
    ANY_USED.store(true, Ordering::Relaxed);
    MEANS[slot].update(value.into());
}

pub fn print() {
    if !ANY_USED.load(Ordering::Relaxed) {
        return;
    }

    let mut out = stdout().lock();

    for (i, condition) in CONDITION_HITS.iter().enumerate() {
        let (hits, misses) = condition.get();

        if hits == 0 && misses == 0 {
            continue;
        }

        let hitrate = hits as f64 / (hits + misses) as f64;

        writeln!(out, "condition {}:", i).unwrap();
        writeln!(out, "    hits: {}", hits).unwrap();
        writeln!(out, "    misses: {}", misses).unwrap();
        writeln!(out, "    hitrate: {:.3}%", hitrate * 100.0).unwrap();
    }

    for (i, range) in RANGES.iter().enumerate() {
        let (min, max) = range.get();

        if min == i64::MAX {
            continue;
        }

        writeln!(out, "range {}:", i).unwrap();
        writeln!(out, "    min: {}", min).unwrap();
        writeln!(out, "    max: {}", max).unwrap();
    }

    for (i, mean) in MEANS.iter().enumerate() {
        let (total, count) = mean.get();

        if count == 0 {
            continue;
        }

        let mean = total as f64 / count as f64;

        writeln!(out, "mean {}:", i).unwrap();
        writeln!(out, "    mean: {:.6}", mean).unwrap();
        writeln!(out, "    total: {}", total).unwrap();
        writeln!(out, "    count: {}", count).unwrap();
    }
}
