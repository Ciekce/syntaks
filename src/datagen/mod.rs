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

mod synpack;

use crate::board::{FlatCountOutcome, Position};
use crate::core::Player;
use crate::datagen::synpack::SynpackWriter;
use crate::limit::Limits;
use crate::movegen::generate_moves;
use crate::prng::{SeedGenerator, Sfc64, get_seed};
use crate::search::*;
use crate::takmove::Move;
use arrayvec::ArrayVec;
use rand::Rng;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const TT_SIZE_MIB: usize = 8;
const REPORT_INTERVAL: usize = 512;

const RANDOM_MOVES: usize = 6;

const VERIF_DEPTH: i32 = 6;
const VERIF_MAX_SCORE: Score = 1000;

const SOFT_NODES: usize = 5000;
const HARD_NODES: usize = 8388608;

static STOP: AtomicBool = AtomicBool::new(false);
static ERROR: AtomicBool = AtomicBool::new(false);

static PRINT_MUTEX: Mutex<()> = Mutex::new(());

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum GameResult {
    Loss,
    Draw,
    Win,
}

impl GameResult {
    #[must_use]
    fn flip(self) -> Self {
        match self {
            Self::Loss => Self::Win,
            Self::Draw => Self::Draw,
            Self::Win => Self::Loss,
        }
    }
}

#[must_use]
fn is_drawn_by_repetition(curr_key: u64, key_history: &[u64], threefold: bool) -> bool {
    let required = if threefold { 2 } else { 1 };

    let mut repetitions = 0;

    //TODO skip properly
    for &key in key_history.iter().rev() {
        if key == curr_key {
            repetitions += 1;
            if repetitions == required {
                return true;
            }
        }
    }

    false
}

#[must_use]
fn check_terminal(pos: &Position, key_history: &[u64], prev_move: Move) -> Option<GameResult> {
    if pos.has_road(pos.stm()) {
        return Some(GameResult::Win);
    }

    if prev_move.is_spread() && pos.has_road(pos.stm().flip()) {
        return Some(GameResult::Loss);
    }

    if !prev_move.is_spread() {
        match pos.count_flats() {
            FlatCountOutcome::None => {}
            FlatCountOutcome::Draw => return Some(GameResult::Draw),
            FlatCountOutcome::Win(player) => {
                return if player == pos.stm() {
                    Some(GameResult::Win)
                } else {
                    Some(GameResult::Loss)
                };
            }
        }
    }

    if prev_move.is_spread() && is_drawn_by_repetition(pos.key(), key_history, false) {
        return Some(GameResult::Draw);
    }

    None
}

#[must_use]
fn start_game(
    writer: &mut SynpackWriter,
    moves: &mut Vec<Move>,
    key_history: &mut Vec<u64>,
    rng: &mut Sfc64,
    searcher: &mut Searcher,
) -> Position {
    let mut unscored_moves = ArrayVec::<_, RANDOM_MOVES>::new();

    let pos = 'x: loop {
        unscored_moves.clear();

        let mut pos = Position::startpos();
        key_history.clear();

        for _ in 0..RANDOM_MOVES {
            moves.clear();
            generate_moves(moves, &pos);

            let mv = moves[rng.random_range(..moves.len())];

            unscored_moves.push(mv);

            key_history.push(pos.key());
            pos = pos.apply_move(mv);

            if check_terminal(&pos, key_history, mv).is_some() {
                continue 'x;
            }
        }

        searcher.run_datagen_search(&pos, key_history, Limits::new(Instant::now()), VERIF_DEPTH);

        let verif_score = searcher.thread().pv_move().score;

        if verif_score.abs() <= VERIF_MAX_SCORE {
            break pos;
        }
    };

    writer.start();

    for mv in unscored_moves {
        writer.push_unscored(mv);
    }

    pos
}

fn signal_error() {
    STOP.store(true, Ordering::Release);
    ERROR.store(true, Ordering::Release);
}

fn run_thread(id: u32, seed: u64, out_dir: &Path) {
    let out_file = out_dir.join(format!("{}.sypk", id));

    let file = match OpenOptions::new().create(true).append(true).open(&out_file) {
        Ok(file) => file,
        Err(err) => {
            signal_error();
            let _print_lock = PRINT_MUTEX.lock();
            eprintln!(
                "thread {}: Failed to open output file '{:?}': {}",
                id, &out_file, err
            );
            return;
        }
    };

    let mut out = BufWriter::new(file);
    let mut writer = SynpackWriter::new();

    let mut rng = Sfc64::new(seed);

    let mut searchers = [Searcher::new(TT_SIZE_MIB), Searcher::new(TT_SIZE_MIB)];

    for searcher in searchers.iter_mut() {
        searcher.set_silent(true);
    }

    let mut game_count: usize = 0;
    let mut total_positions: usize = 0;

    let mut moves = Vec::with_capacity(1536);
    let mut key_history = Vec::with_capacity(1024);

    let limits = {
        let mut limits = Limits::new(Instant::now());
        limits.set_soft_nodes(SOFT_NODES);
        limits.set_hard_nodes(HARD_NODES);
        limits
    };

    let start = Instant::now();

    let print_progress = |game_count, total_positions| {
        let time = start.elapsed().as_secs_f64();

        let games_per_sec = game_count as f64 / time;
        let pos_per_sec = total_positions as f64 / time;

        let _print_lock = PRINT_MUTEX.lock();
        println!(
            "thread {}: wrote {} positions from {} games in {:.1} sec ({:.1} games/sec, {:.1} pos/sec)",
            id, total_positions, game_count, time, games_per_sec, pos_per_sec
        );
    };

    while !STOP.load(Ordering::Acquire) {
        for searcher in searchers.iter_mut() {
            searcher.reset();
        }

        let mut pos = start_game(
            &mut writer,
            &mut moves,
            &mut key_history,
            &mut rng,
            &mut searchers[0],
        );

        searchers[0].reset();

        let outcome = loop {
            let searcher = &mut searchers[pos.stm().idx()];

            searcher.run_datagen_search(&pos, &key_history, limits, MAX_PLY);

            let root_move = searcher.thread().pv_move();

            let mv = root_move.mv();
            let score = root_move.score;

            writer.push(mv, score);

            key_history.push(pos.key());
            let new_pos = pos.apply_move(mv);

            if let Some(outcome) = check_terminal(&new_pos, &key_history, mv) {
                break outcome.flip();
            }

            if score.abs() > SCORE_WIN {
                break GameResult::Win;
            } else if score.abs() < -SCORE_WIN {
                break GameResult::Loss;
            }

            pos = new_pos;
        };

        // we want to store WDL labels as p1-relative, so flip
        //  the outcome if the previous search was done as p2
        let outcome = match pos.stm() {
            Player::P1 => outcome,
            Player::P2 => outcome.flip(),
        };

        match writer.write_all_with_outcome(&mut out, outcome) {
            Ok(written) => total_positions += written,
            Err(err) => {
                signal_error();
                let _print_lock = PRINT_MUTEX.lock();
                eprintln!("thread {}: failed to serialize game: {}", id, err);
            }
        }

        if let Err(err) = out.flush() {
            signal_error();
            let _print_lock = PRINT_MUTEX.lock();
            eprintln!("thread {}: failed to flush output buffer: {}", id, err);
        }

        game_count += 1;

        if game_count.is_multiple_of(REPORT_INTERVAL) {
            print_progress(game_count, total_positions);
        }
    }

    if !game_count.is_multiple_of(REPORT_INTERVAL) {
        print_progress(game_count, total_positions);
    }
}

pub fn run() -> i32 {
    let args: Vec<_> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("usage: {} <threads> <path>", args[0]);
        return 1;
    }

    let threads = match args[1].parse() {
        Ok(threads) => threads,
        Err(_) => {
            eprintln!("invalid thread count '{}'", args[1]);
            return 1;
        }
    };

    let out_dir = args[2].as_ref();

    if let Err(err) = ctrlc::set_handler(|| {
        STOP.store(true, Ordering::Release);
    }) {
        eprintln!("failed to set ctrl+c handler: {}", err);
        return 1;
    }

    let base_seed = match get_seed() {
        Ok(seed) => seed,
        Err(err) => {
            eprintln!("failed to generate base seed: {}", err);
            return 1;
        }
    };

    println!("base seed: {:016x}", base_seed);

    let mut seed_generator = SeedGenerator::new(base_seed);

    std::thread::scope(|s| {
        for id in 0..threads {
            let seed = seed_generator.next();
            s.spawn(move || {
                run_thread(id, seed, out_dir);
            });
        }
    });

    if ERROR.load(Ordering::Acquire) {
        1
    } else {
        println!("done");
        0
    }
}
