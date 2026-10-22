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
use crate::core::Color;
use crate::eval::static_eval_once;
use crate::limit::{Limits, MAX_MOVE_OVERHEAD_MS};
use crate::perft::{perft, split_perft};
use crate::position::*;
use crate::search;
use crate::search::{MAX_THREADS, Searcher};
use crate::ttable::{DEFAULT_TT_SIZE_MIB, MAX_TT_SIZE_MIB};
use std::sync::atomic::Ordering;
use std::time::Instant;

const NAME: &str = "syntaks";
const AUTHORS: &str = "Ciekce and chef";
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub const MAX_MULTIPV: usize = 2048;

const DEFAULT_BENCH_DEPTH: i32 = 12;

const BENCH_POSITIONS: [&str; 25] = [
    "x6/x6/x6/x6/x6/x6 1 1",
    "2,x5/x2,1,x2,1/x,2,1C,x3/x,1,2,2C,2,x/x,1,x,1,2,2/x5,1 1 11",
    "2,x5/x,1,1,1,2,1/x,21C,x2,1,x/2,1,2,2C,2,x/x,1212,x,1,2,2/x5,1 1 17",
    "2,x5/x,1,11,1S,2112S,1/1,2,1,x3/2,121C,2,2C,2,x/x,1212,1S,1,2,2/2,2S,x3,1 1 24",
    "2,x5/x,1,1,2,x2/x,2,1,2,x2/x2,2C,1C,x2/1,x4,1/x,2,x,2,x,1 1 11",
    "2,2,x4/2,1,1,2,x2/1,2,1,2,1,1/x,1,2C,1C,x,1/x,21,2,x,2S,1/x,2,2,2,x,1 1 17",
    "2,2,x4/2,1,1,2,1,x/1,212,x,2221C,1,1/x2,112C,x,1,1/x,21,2,x2,12S/x,2,2,2,x,1 1 24",
    "2,1,2,x,1,x/x2,2,x3/x3,2C,1,x/x2,1C,1,2,x/x2,2,1,x2/x2,2,1,x2 1 9",
    "2,1,2,x,1,x/x,1,2,2,2,x/x,2,1,1,1,2/x,2,1C,12C,2,x/x,1,2,1,1,1/x2,2,1,x2 1 15",
    "2,1,2,x3/2,1,2,2,21,1/x,2,1,1,12,x/212,21,1C,1,2,x/x,12,x,112C,1,1/x2,2,1,x2 1 22",
    "2,x3,1,x/x4,1,1/x4,1C,x/x,2,x,2,2,12/x4,2S,1/x5,1 1 11",
    "2,x3,1,x/2,x3,1,1/2C,1,1,x2,2S/2,2,2,2,21C,12/x,1S,2,1,2S,1/x2,1,x2,1 1 17",
    "2,x,1,2,1,x/2,x,1,2,1,1/2C,2S,1,2,1,2S/2,21,2,221C,2,12/1,1S,2,1,2S,1/x,2,1,2,1,1 1 24",
    "2,1,x4/1,2,2,x3/x,1,2,2,2,x/x,1C,2C,x3/x,1,x,1,x2/x2,1,x3 1 9",
    "2,121212,1S,x3/x,2S,1S,x3/x,1,2,2,2,x/x,1C,2C,x3/x,1,x,1,2,x/x2,1,x,1,x 1 15",
    "2,121212,1S,x3/x,2S,1S,x,1,1/x,1,2,2,2,2/x,1C,1,2,2,x/2,1,12C,1,2,1/1,2,1,1,1,2 1 22",
    "2,x,2,1,x,1/x2,2,1,x2/x3,12C,x2/x2,2,1,x2/x,2,1,1,x2/x6 1 9",
    "2,2,2,1,2,1/x,1,21C,1,2,1/x,1,2,12C,1,x/x2,2,1,2,x/x,2,1,1,2,x/x6 1 15",
    "2,2,2,1,221,1/1,1,x,12S,21112C,1/x,1,221C,x3/x2,2,1,2,x/x,2,1,1,2,x/x3,1,2,x 1 22",
    "2,1,x4/2,1,x4/x,2,2C,x3/x,1C,1,x3/x,2,1,1,2,1/x,2,x4 1 9",
    "2,1,x4/2,1,2,x3/2,21,x,1,x2/2,1C,x,2,1,x/1,2,112C,1,2,1/1,2,x4 1 15",
    "2,1,x4/2,1,2,x2,2/2,21,1,12S,x,1/21C,x,1,2,12,1/x,112112C,x,1,x,1/x,2,x3,1 1 22",
    "2,x,1,x3/x6/x,2,x4/2,2,x2,2,x/x,1,1C,x,2S,x/x,1,2,1,1,1 1 11",
    "2,x,1,x3/x4,2,x/x,2,2C,12,1,x/2,2,11C,1S,2,2/1,1,x2,2S,x/x,1,x,12,1,1 1 17",
    "2,x,1,x3/2,21,1,x,2,x/2,2211C,2C,12,1,x/2,x,2,1S,2,2/1,12S,1,2,2S,x/x,1,1,12,1,1 1 24",
];

#[derive(Copy, Clone, Debug)]
pub struct TeiOptions {
    pub multipv: usize,
    pub minimal: bool,
    pub mate_scores: bool,
    pub show_curr_move: bool,
    pub silent: bool,
    pub move_overhead_ms: u64,
    pub double_black_stack: bool,
}

impl Default for TeiOptions {
    fn default() -> Self {
        Self {
            multipv: 1,
            minimal: false,
            mate_scores: true,
            show_curr_move: false,
            silent: false,
            move_overhead_ms: 10,
            double_black_stack: false,
        }
    }
}

struct TeiHandler {
    pos: Position,
    key_history: Vec<u64>,
    searcher: Searcher,
    options: TeiOptions,
}

impl TeiHandler {
    #[must_use]
    fn new() -> Self {
        Self {
            pos: Position::startpos(),
            key_history: Vec::with_capacity(1024),
            searcher: Searcher::new(DEFAULT_TT_SIZE_MIB),
            options: TeiOptions::default(),
        }
    }

    fn run(&mut self) {
        let mut line = String::with_capacity(256);
        while let Ok(bytes) = std::io::stdin().read_line(&mut line) {
            if bytes == 0 {
                break;
            }

            let start_time = Instant::now();

            let args: Vec<_> = line.split_ascii_whitespace().collect();
            if args.is_empty() {
                line.clear();
                continue;
            }

            let (&command, args) = args.split_first().unwrap();

            match command {
                "tei" => self.handle_tei(),
                "teinewgame" => self.handle_teinewgame(args),
                "setoption" => self.handle_setoption(args),
                "isready" => self.handle_isready(),
                "position" => self.handle_position(args),
                "go" => self.handle_go(args, start_time),
                "stop" => self.handle_stop(),
                "move" => self.handle_move(args),
                "wait" => self.handle_wait(),
                "d" => self.handle_d(),
                "perft" => self.handle_perft(args),
                "splitperft" => self.handle_splitperft(args),
                "bench" => self.handle_bench(args.first().copied()),
                "quit" => break,
                unknown => eprintln!("Unknown command '{}'", unknown),
            }

            line.clear();
        }
    }

    fn handle_tei(&self) {
        let half_komi = Position::KOMI * 2;
        let default_options = TeiOptions::default();

        println!("id name {} {}", NAME, VERSION);
        println!("id author {}", AUTHORS);

        println!(
            "option name HalfKomi type spin default {} min {} max {}",
            half_komi, half_komi, half_komi
        );
        println!(
            "option name Hash type spin default {} min 1 max {}",
            DEFAULT_TT_SIZE_MIB, MAX_TT_SIZE_MIB
        );
        println!("option name Threads type spin default 1 min 1 max {}", MAX_THREADS);
        println!(
            "option name MultiPV type spin default {} min 1 max {}",
            default_options.multipv, MAX_MULTIPV
        );
        println!("option name Minimal type check default {}", default_options.minimal);
        println!(
            "option name MateScores type check default {}",
            default_options.mate_scores
        );
        println!(
            "option name ShowCurrMove type check default {}",
            default_options.show_curr_move
        );
        println!(
            "option name MoveOverhead type spin default {} min 0 max {}",
            default_options.move_overhead_ms, MAX_MOVE_OVERHEAD_MS
        );
        println!("option name DoubleBlackStack type check default false");

        println!("teiok");
    }

    fn handle_teinewgame(&mut self, args: &[&str]) {
        if self.searcher.is_searching() {
            eprintln!("Search running (received 'teinewgame')");
            return;
        }

        if args.is_empty() {
            println!("info string Missing size, assuming 6x6");
        } else {
            match args[0].parse::<u32>() {
                Ok(size) => {
                    if size != 6 {
                        eprintln!("Only 6x6 supported");
                        return;
                    }
                }
                Err(_) => eprintln!("Invalid size"),
            }
        }

        self.searcher.reset();
    }

    fn handle_setoption(&mut self, args: &[&str]) {
        if self.searcher.is_searching() {
            eprintln!("Search running (received 'setoption')");
            return;
        }

        if args.len() < 2 || args[0] != "name" {
            return;
        }

        let value_idx = args.iter().position(|&s| s == "value");

        if value_idx.is_none() {
            eprintln!("Missing value");
            return;
        }

        let value_idx = value_idx.unwrap();

        if value_idx == args.len() - 1 {
            eprintln!("Missing value");
            return;
        }

        if value_idx == 1 {
            eprintln!("Missing option name");
            return;
        }

        if value_idx > 2 {
            let skipped = args[2..value_idx].join(" ");
            println!("info string Warning: spaces in option names not supported");
            println!(
                "info string Interpreting '{}' as option name and skipping '{}'",
                args[1], skipped
            );
        }

        let name = args[1].to_ascii_lowercase();
        let value = args[(value_idx + 1)..].join(" ");

        match name.as_str() {
            "halfkomi" => {
                if let Ok(half_komi) = value.parse::<u32>()
                    && half_komi != Position::KOMI * 2
                {
                    eprintln!("Unsupported komi value");
                }
            }
            "hash" => {
                if let Ok(size) = value.parse::<usize>() {
                    let size = size.clamp(1, MAX_TT_SIZE_MIB);
                    self.searcher.set_tt_size(size);
                }
            }
            "threads" => {
                if let Ok(threads) = value.parse::<u32>() {
                    let threads = threads.clamp(1, MAX_THREADS);
                    self.searcher.set_threads(threads);
                }
            }
            "multipv" => {
                if let Ok(multipv) = value.parse::<usize>() {
                    let multipv = multipv.clamp(1, MAX_MULTIPV);
                    self.options.multipv = multipv;
                }
            }
            "minimal" => {
                if let Ok(minimal) = value.parse::<bool>() {
                    self.options.minimal = minimal;
                }
            }
            "matescores" => {
                if let Ok(mate_scores) = value.parse::<bool>() {
                    self.options.mate_scores = mate_scores;
                }
            }
            "showcurrmove" => {
                if let Ok(show_curr_move) = value.parse::<bool>() {
                    self.options.show_curr_move = show_curr_move;
                }
            }
            "moveoverhead" => {
                if let Ok(overhead) = value.parse::<u64>() {
                    let overhead = overhead.clamp(0, MAX_MOVE_OVERHEAD_MS);
                    self.options.move_overhead_ms = overhead;
                }
            }
            "doubleblackstack" => {
                if let Ok(double_black_stack) = value.parse::<bool>() {
                    DOUBLE_BLACK_STACK.store(double_black_stack, Ordering::Relaxed);
                }
            }
            unknown => eprintln!("Unknown option '{}'", unknown),
        }
    }

    fn handle_isready(&self) {
        println!("readyok");
    }

    fn handle_position(&mut self, args: &[&str]) {
        if self.searcher.is_searching() {
            eprintln!("Search running (received 'position')");
            return;
        }

        if args.is_empty() {
            return;
        }

        let (&pos_type, args) = args.split_first().unwrap();

        let mut next = 0;

        match pos_type {
            "startpos" => {
                self.pos = Position::startpos();
                self.key_history.clear();
            }
            "tps" => {
                let count = args.iter().position(|&s| s == "moves").unwrap_or(args.len());

                if count == 0 {
                    eprintln!("Missing TPS");
                    return;
                }

                match Position::from_tps_parts(&args[0..count]) {
                    Ok(pos) => {
                        self.pos = pos;
                        self.key_history.clear();
                    }
                    Err(err) => {
                        eprintln!("Failed to parse TPS: {:?}", err);
                        return;
                    }
                }

                next += count;
            }
            _ => {
                eprintln!("Invalid position type {}", pos_type);
                return;
            }
        }

        if next >= args.len() || args[next] != "moves" {
            return;
        }

        for &move_str in &args[(next + 1)..] {
            match move_str.parse() {
                Ok(mv) => {
                    if !self.pos.is_legal(mv) {
                        eprintln!("Illegal move '{}'", mv);
                        return;
                    }
                    self.key_history.push(self.pos.key());
                    self.pos = self.pos.apply_move(mv);
                }
                Err(err) => {
                    eprintln!("Invalid move '{}': {:?}", move_str, err);
                    return;
                }
            }
        }
    }

    fn handle_go(&mut self, args: &[&str], start_time: Instant) {
        if self.searcher.is_searching() {
            eprintln!("Search running (received 'go')");
            return;
        }

        let mut limits = Limits::new(start_time);
        let mut max_depth = None;

        let mut wtime = None;
        let mut btime = None;
        let mut winc = None;
        let mut binc = None;

        let mut moves_to_search = Vec::new();

        let mut i = 0;
        while i < args.len() {
            let limit_str = args[i];
            match limit_str {
                "infinite" => {}
                "depth" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("Missing depth");
                        return;
                    }

                    if let Ok(depth) = args[i].parse() {
                        if max_depth.is_some() {
                            eprintln!("Duplicate depth limits");
                            return;
                        }
                        max_depth = Some(depth);
                    } else {
                        eprintln!("Invalid depth '{}'", args[i]);
                        return;
                    }
                }
                "nodes" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("Missing node count");
                        return;
                    }

                    if let Ok(nodes) = args[i].parse() {
                        if !(limits.set_soft_nodes(nodes) && limits.set_hard_nodes(nodes)) {
                            eprintln!("Duplicate node limits");
                            return;
                        }
                    } else {
                        eprintln!("Invalid node count '{}'", args[i]);
                        return;
                    }
                }
                "movetime" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("Missing time");
                        return;
                    }

                    if let Ok(movetime) = args[i].parse::<u64>() {
                        let secs = (movetime as f64) / 1000.0;
                        if !limits.set_movetime(secs) {
                            eprintln!("Duplicate movetime limits");
                            return;
                        }
                    } else {
                        eprintln!("Invalid time '{}'", args[i]);
                        return;
                    }
                }
                "wtime" | "btime" | "winc" | "binc" => {
                    i += 1;
                    if i >= args.len() {
                        eprintln!("Missing time");
                        return;
                    }

                    if let Ok(time) = args[i].parse::<u64>() {
                        let limit = match limit_str {
                            "wtime" => &mut wtime,
                            "btime" => &mut btime,
                            "winc" => &mut winc,
                            "binc" => &mut binc,
                            _ => unreachable!(),
                        };

                        if limit.is_some() {
                            eprintln!("Duplicate {} limits", limit_str);
                            return;
                        }

                        let secs = (time as f64) / 1000.0;
                        *limit = Some(secs);
                    } else {
                        eprintln!("Invalid time '{}'", args[i]);
                        return;
                    }
                }
                "searchmoves" => {
                    while i + 1 < args.len() {
                        let candidate = args[i + 1];
                        if let Ok(mv) = candidate.parse() {
                            if moves_to_search.contains(&mv) {
                                continue;
                            }

                            if !self.pos.is_legal(mv) {
                                println!("info string searchmoves: Skipping illegal move '{}'", mv);
                            }

                            moves_to_search.push(mv);

                            i += 1;
                        } else {
                            break;
                        }
                    }
                }
                unsupported => eprintln!("Unsupported limit '{}'", unsupported),
            }

            i += 1;
        }

        let (our_time, our_inc) = match self.pos.stm() {
            Color::White => (wtime, winc),
            Color::Black => (btime, binc),
        };

        if our_inc.is_some() && our_time.is_none() {
            println!("info string Warning: increment given but no base time");
        }

        if let Some(our_time) = our_time {
            let overhead = self.options.move_overhead_ms as f64 / 1000.0;
            let our_inc = our_inc.unwrap_or(0.0);
            limits.set_time_manager(our_time, our_inc, overhead);
        }

        let max_depth = max_depth.unwrap_or(search::MAX_DEPTH).clamp(1, search::MAX_DEPTH);

        self.searcher.start_search(
            &self.pos,
            &self.key_history,
            start_time,
            limits,
            max_depth,
            &moves_to_search,
            &self.options,
        );
    }

    fn handle_stop(&mut self) {
        self.searcher.stop();
    }

    fn handle_move(&mut self, args: &[&str]) {
        if args.is_empty() {
            eprintln!("Missing move");
        }

        match args[0].parse() {
            Ok(mv) => {
                if !self.pos.is_legal(mv) {
                    eprintln!("Illegal move '{}'", mv);
                    return;
                }
                self.key_history.push(self.pos.key());
                self.pos = self.pos.apply_move(mv);
            }
            Err(err) => eprintln!("Invalid move '{}': {:?}", args[0], err),
        }
    }

    fn handle_wait(&mut self) {
        self.searcher.wait();
    }

    fn handle_d(&self) {
        println!("TPS: {}", self.pos.tps());
        println!("Key: {:016x}", self.pos.key());

        let static_eval = static_eval_once(&self.pos);
        let static_eval = match self.pos.stm() {
            Color::White => static_eval,
            Color::Black => -static_eval,
        };

        println!("Static eval (white-relative): {:+.2}", (static_eval as f64) / 100.0);
    }

    fn handle_bench(&mut self, depth: Option<&str>) {
        if self.searcher.is_searching() {
            eprintln!("Search running (received 'bench')");
            return;
        }

        let depth = match depth {
            Some(depth) => match depth.parse::<i32>() {
                Ok(depth) => depth.clamp(1, search::MAX_DEPTH),
                Err(_) => {
                    eprintln!("Invalid depth '{}'", depth);
                    return;
                }
            },
            None => DEFAULT_BENCH_DEPTH,
        };

        let mut searcher = Searcher::new(16);

        let options = TeiOptions {
            minimal: true,
            ..TeiOptions::default()
        };

        let mut total_nodes = 0;
        let mut total_time = 0.0;

        for tps in BENCH_POSITIONS {
            println!("tps: {}", tps);

            let pos = tps.parse().unwrap();

            let start_time = Instant::now();
            let limits = Limits::new(start_time);

            searcher.start_search(&pos, &[], start_time, limits, depth, &[], &options);
            searcher.wait();

            total_time += start_time.elapsed().as_secs_f64();
            total_nodes += searcher.total_nodes();

            println!();
        }

        let nps = ((total_nodes as f64) / total_time).round() as usize;

        println!("{:.3} seconds", total_time);
        println!("{} nodes {} nps", total_nodes, nps);
    }

    fn handle_perft(&self, args: &[&str]) {
        if args.is_empty() {
            eprintln!("Missing depth");
            return;
        }

        let depth = match args[0].parse() {
            Ok(depth) => depth,
            Err(_) => {
                eprintln!("Invalid depth '{}'", args[0]);
                return;
            }
        };

        println!("{}", perft(&self.pos, depth));
    }

    fn handle_splitperft(&self, args: &[&str]) {
        if args.is_empty() {
            eprintln!("Missing depth");
            return;
        }

        let depth = match args[0].parse() {
            Ok(depth) => depth,
            Err(_) => {
                eprintln!("Invalid depth '{}'", args[0]);
                return;
            }
        };

        split_perft(&self.pos, depth);
    }
}

pub fn run() {
    let mut handler = TeiHandler::new();

    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("bench") {
        handler.handle_bench(args.next().as_deref());
        return;
    }

    handler.run();
}
