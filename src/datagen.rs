use crate::core::*;
use crate::format::{GameResult, write_game};
use crate::limit::Limits;
use crate::position::{FlatCountOutcome, Position};
use crate::search::{MAX_DEPTH, Searcher};
use crate::takmove::Move;
use crate::tei::TeiOptions;
use crate::thread::RootMove;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const TT_SIZE_MIB: usize = 8;
const REPORT_INTERVAL: usize = 512;

const EVAL_NOISE: i32 = 16;
const ROOT_NOISE: i32 = 8192;

#[must_use]
pub fn eval_noise() -> i32 {
    rand::random_range(-EVAL_NOISE..=EVAL_NOISE)
}

#[must_use]
pub fn move_noise() -> i32 {
    rand::random_range(-ROOT_NOISE..=ROOT_NOISE)
}

const SOFT_NODES: usize = 10000;
const HARD_NODES: usize = 8388608;

const MAX_GAME_PLIES: usize = 1000;

static STOP: AtomicBool = AtomicBool::new(false);
static ERROR: AtomicBool = AtomicBool::new(false);

static PRINT_MUTEX: Mutex<()> = Mutex::new(());

fn search(searcher: &mut Searcher, pos: &Position, key_history: &[u64], limits: Limits, max_depth: i32) -> RootMove {
    let options = TeiOptions {
        silent: true,
        ..Default::default()
    };

    searcher.start_search(
        pos,
        key_history,
        Instant::now(),
        limits,
        max_depth,
        &[],
        &HashSet::new(),
        &options,
    );
    searcher.wait();

    searcher.result()
}

#[must_use]
fn check_terminal(pos: &Position, key_history: &[u64], prev_move: Move) -> Option<GameResult> {
    let stm = pos.stm();
    let ntm = stm.flip();

    if pos.has_road(ntm) {
        return Some(GameResult::Win);
    }

    if prev_move.is_spread() && pos.has_road(stm) {
        return Some(GameResult::Loss);
    }

    match pos.count_flats() {
        FlatCountOutcome::None => {}
        FlatCountOutcome::Draw => return Some(GameResult::Draw),
        FlatCountOutcome::Win(color) => {
            return if color == ntm {
                Some(GameResult::Win)
            } else {
                Some(GameResult::Loss)
            };
        }
    }

    if prev_move.is_spread() && key_history.contains(&pos.key()) {
        return Some(GameResult::Draw);
    }

    None
}

const HUG: &[&str] = &[
    "x6/x6/x6/x6/x6/2,1,x4 1 2",
    "x6/x6/x6/x6/1,x5/2,x5 1 2",
    "2,1,x4/x6/x6/x6/x6/x6 1 2",
    "2,x5/1,x5/x6/x6/x6/x6 1 2",
    "x4,1,2/x6/x6/x6/x6/x6 1 2",
    "x5,2/x5,1/x6/x6/x6/x6 1 2",
    "x6/x6/x6/x6/x6/x4,1,2 1 2",
    "x6/x6/x6/x6/x5,1/x5,2 1 2",
];

const ADJACENT: &[&str] = &[
    "x6/x6/x6/x6/x6/2,x4,1 1 2",
    "1,x5/x6/x6/x6/x6/2,x5 1 2",
    "2,x5/x6/x6/x6/x6/1,x5 1 2",
    "2,x4,1/x6/x6/x6/x6/x6 1 2",
    "1,x4,2/x6/x6/x6/x6/x6 1 2",
    "x5,2/x6/x6/x6/x6/x5,1 1 2",
    "x5,1/x6/x6/x6/x6/x5,2 1 2",
    "x6/x6/x6/x6/x6/1,x4,2 1 2",
];

const OPPOSITE: &[&str] = &[
    "x5,1/x6/x6/x6/x6/2,x5 1 2",
    "2,x5/x6/x6/x6/x6/x5,1 1 2",
    "x5,2/x6/x6/x6/x6/1,x5 1 2",
    "1,x5/x6/x6/x6/x6/x5,2 1 2",
];

#[must_use]
fn start_game(key_history: &mut Vec<u64>, half_komi: i8) -> Position {
    let opening = [&HUG, &ADJACENT, &OPPOSITE][rand::random_range(0..3)];
    let tps = opening[rand::random_range(0..opening.len())];
    let parts: Vec<&str> = tps.split_whitespace().collect();
    let mut pos = Position::from_tps_parts(&parts).unwrap();
    pos.set_half_komi(half_komi);
    key_history.clear();
    pos
}

fn signal_error() {
    STOP.store(true, Ordering::Release);
    ERROR.store(true, Ordering::Release);
}

fn run_thread(id: u32, out_dir: &Path, half_komi: i8) {
    let out_file = out_dir.join(format!("{}.sf", id));

    let file = match OpenOptions::new().create(true).append(true).open(&out_file) {
        Ok(file) => file,
        Err(err) => {
            signal_error();
            let _print_lock = PRINT_MUTEX.lock();
            eprintln!("thread {}: Failed to open output file '{:?}': {}", id, out_file, err);
            return;
        }
    };

    let mut out = BufWriter::new(file);

    let mut searchers = [Searcher::new(TT_SIZE_MIB), Searcher::new(TT_SIZE_MIB)];

    let mut game_count: usize = 0;
    let mut total_positions: usize = 0;

    let mut key_history = Vec::with_capacity(1024);
    let mut scored_moves: Vec<(Move, i16)> = Vec::with_capacity(1024);

    let limits = {
        let mut limits = Limits::new(Instant::now());
        limits.set_soft_nodes(SOFT_NODES);
        limits.set_hard_nodes(HARD_NODES);
        limits
    };

    let start = Instant::now();

    let print_progress = |game_count: usize, total_positions: usize| {
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
        let root_pos = start_game(&mut key_history, half_komi);

        scored_moves.clear();

        let mut pos = root_pos;

        let outcome = loop {
            let searcher = &mut searchers[pos.stm()];

            let root_move = search(searcher, &pos, &key_history, limits, MAX_DEPTH);

            let mv = root_move.mv();
            let score = root_move.score;

            let w_score = score * pos.stm().sign();
            scored_moves.push((mv, w_score as i16));

            key_history.push(pos.key());
            let new_pos = pos.apply_move(mv);

            if let Some(outcome) = check_terminal(&new_pos, &key_history, mv) {
                break outcome;
            }

            if is_decisive(score) {
                break if is_win(score) {
                    GameResult::Win
                } else {
                    GameResult::Loss
                };
            }

            if scored_moves.len() >= MAX_GAME_PLIES {
                break GameResult::Draw;
            }

            pos = new_pos;
        };

        let outcome = match pos.stm() {
            Color::White => outcome,
            Color::Black => outcome.flip(),
        };

        match write_game(&mut out, &root_pos, outcome, &scored_moves) {
            Ok(()) => total_positions += scored_moves.len(),
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
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("usage: {} <threads> <out_dir> [half_komi]", args[0]);
        return 1;
    }

    let threads: u32 = match args[1].parse() {
        Ok(threads) if threads > 0 => threads,
        _ => {
            eprintln!("invalid thread count '{}'", args[1]);
            return 1;
        }
    };

    let half_komi: i8 = match args.get(3).map(|arg| arg.parse::<i8>()) {
        None => Position::DEFAULT_HALF_KOMI,
        Some(Ok(half_komi)) if (0..=7).contains(&half_komi) => half_komi,
        _ => {
            eprintln!("invalid half komi '{}'", args[3]);
            return 1;
        }
    };

    println!("half komi: {}", half_komi);

    let out_dir = Path::new(&args[2]);

    if let Err(err) = std::fs::create_dir_all(out_dir) {
        eprintln!("failed to create output directory '{:?}': {}", out_dir, err);
        return 1;
    }

    if let Err(err) = ctrlc::set_handler(|| {
        STOP.store(true, Ordering::Release);
    }) {
        eprintln!("failed to set ctrl+c handler: {}", err);
        return 1;
    }

    std::thread::scope(|s| {
        for id in 0..threads {
            s.spawn(move || {
                run_thread(id, out_dir, half_komi);
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
