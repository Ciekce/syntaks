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

use std::sync::atomic::{AtomicUsize, Ordering};

#[repr(align(64))]
struct CounterShard {
    value: AtomicUsize,
}

impl CounterShard {
    #[must_use]
    fn new() -> Self {
        Self {
            value: AtomicUsize::new(0),
        }
    }

    fn increment(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    fn load(&self) -> usize {
        self.value.load(Ordering::Relaxed)
    }

    fn reset(&mut self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

pub struct Counter {
    counters: Box<[CounterShard]>,
}

unsafe impl Sync for Counter {}

impl Counter {
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            counters: std::iter::from_fn(|| Some(CounterShard::new()))
                .take(count)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    pub fn increment(&self, id: usize) {
        self.counters[id].increment();
    }

    #[must_use]
    pub fn get(&self, id: usize) -> usize {
        self.counters[id].load()
    }

    #[must_use]
    pub fn total(&self) -> usize {
        self.counters.iter().map(CounterShard::load).sum()
    }

    pub fn reset(&mut self) {
        for counter in self.counters.iter_mut() {
            counter.reset();
        }
    }

    pub fn resize(&mut self, count: usize) {
        self.counters = std::iter::from_fn(|| Some(CounterShard::new()))
            .take(count)
            .collect::<Vec<_>>()
            .into_boxed_slice();
    }
}
