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

use std::time::Instant;

const TIME_CHECK_INTERVAL: usize = 4096;

#[derive(Copy, Clone, Debug)]
struct TimeManager {
    max_time: f64,
}

impl TimeManager {
    fn new(remaining: f64, increment: f64) -> Self {
        let time = remaining / 20.0 + increment;
        let time = time.min(remaining * 0.8);

        Self { max_time: time }
    }

    fn should_stop(&self, _nodes: usize, time: f64) -> bool {
        time >= self.max_time
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Limits {
    start_time: Instant,
    nodes: Option<usize>,
    movetime: Option<f64>,
    time_manager: Option<TimeManager>,
}

impl Limits {
    pub fn new(start_time: Instant) -> Self {
        Self {
            start_time,
            nodes: None,
            movetime: None,
            time_manager: None,
        }
    }

    pub fn set_nodes(&mut self, nodes: usize) -> bool {
        match self.nodes {
            None => {
                self.nodes = Some(nodes);
                true
            }
            Some(_) => false,
        }
    }

    pub fn set_movetime(&mut self, movetime: f64) -> bool {
        match self.movetime {
            None => {
                self.movetime = Some(movetime);
                true
            }
            Some(_) => false,
        }
    }

    pub fn set_time_manager(&mut self, remaining: f64, increment: f64) -> bool {
        match self.time_manager {
            None => {
                self.time_manager = Some(TimeManager::new(remaining, increment));
                true
            }
            Some(_) => false,
        }
    }

    #[must_use]
    pub fn should_stop_soft(&self, nodes: usize) -> bool {
        if let Some(max_nodes) = self.nodes
            && nodes >= max_nodes
        {
            return true;
        }

        let time = self.start_time.elapsed().as_secs_f64();

        if let Some(movetime) = self.movetime
            && time >= movetime
        {
            return true;
        }

        if let Some(time_manager) = self.time_manager
            && time_manager.should_stop(nodes, time)
        {
            return true;
        }

        false
    }

    #[must_use]
    pub fn should_stop_hard(&self, nodes: usize) -> bool {
        if let Some(max_nodes) = self.nodes
            && nodes >= max_nodes
        {
            return true;
        }

        if nodes > 0
            && nodes.is_multiple_of(TIME_CHECK_INTERVAL)
            && (self.movetime.is_some() || self.time_manager.is_some())
        {
            let time = self.start_time.elapsed().as_secs_f64();

            if let Some(movetime) = self.movetime
                && time >= movetime
            {
                return true;
            }

            if let Some(time_manager) = self.time_manager
                && time_manager.should_stop(nodes, time)
            {
                return true;
            }
        }

        false
    }
}
