use crate::board::{FlatCountOutcome, Position};
use crate::core::*;
use crate::format::{GameResult, write_game};
use crate::limit::Limits;
use crate::movegen::generate_moves;
use crate::prng::{Sfc64, seed_from_entropy, splitmix64};
use crate::search::{MAX_DEPTH, Searcher};
use crate::takmove::Move;
use crate::tei::TeiOptions;
use crate::thread::RootMove;
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
const VERIF_MAX_SCORE: i32 = 1000;

const SOFT_NODES: usize = 5000;
const HARD_NODES: usize = 8388608;

const MAX_GAME_PLIES: usize = 1000;

static STOP: AtomicBool = AtomicBool::new(false);
static ERROR: AtomicBool = AtomicBool::new(false);

static PRINT_MUTEX: Mutex<()> = Mutex::new(());

fn new_searcher() -> Searcher {
    let mut searcher = Searcher::new();
    searcher.set_tt_size(TT_SIZE_MIB);
    searcher
}

fn search(searcher: &mut Searcher, pos: &Position, key_history: &[u64], limits: Limits, max_depth: i32) -> RootMove {
    let options = TeiOptions {
        silent: true,
        ..Default::default()
    };

    searcher.start_search(pos, key_history, Instant::now(), limits, max_depth, &[], &options);
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
        FlatCountOutcome::Win(player) => {
            return if player == ntm {
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

#[must_use]
fn start_game(key_history: &mut Vec<u64>, rng: &mut Sfc64, searcher: &mut Searcher) -> Position {
    let mut moves = Vec::new();

    'games: loop {
        let mut pos = Position::startpos();
        key_history.clear();

        for _ in 0..RANDOM_MOVES {
            moves.clear();
            generate_moves(&mut moves, &pos);

            let mv = moves[rng.next_u64() as usize % moves.len()];

            key_history.push(pos.key());
            pos = pos.apply_move(mv);

            if check_terminal(&pos, key_history, mv).is_some() {
                continue 'games;
            }
        }

        let verif_score = search(searcher, &pos, key_history, Limits::new(Instant::now()), VERIF_DEPTH).score;

        if verif_score.abs() <= VERIF_MAX_SCORE {
            key_history.clear();
            return pos;
        }
    }
}

fn signal_error() {
    STOP.store(true, Ordering::Release);
    ERROR.store(true, Ordering::Release);
}

fn run_thread(id: u32, seed: u64, out_dir: &Path) {
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

    let mut rng = Sfc64::new(seed);

    let mut searchers = [new_searcher(), new_searcher()];

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
        let root_pos = start_game(&mut key_history, &mut rng, &mut searchers[0]);

        for searcher in searchers.iter_mut() {
            searcher.reset();
        }

        scored_moves.clear();

        let mut pos = root_pos;

        let outcome = loop {
            let searcher = &mut searchers[pos.stm().idx()];

            let root_move = search(searcher, &pos, &key_history, limits, MAX_DEPTH);

            let mv = root_move.mv();
            let score = root_move.score;

            let p1_score = score * pos.stm().sign();
            scored_moves.push((mv, p1_score as i16));

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
            Player::P1 => outcome,
            Player::P2 => outcome.flip(),
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
        eprintln!("usage: {} <threads> <out_dir>", args[0]);
        return 1;
    }

    let threads: u32 = match args[1].parse() {
        Ok(threads) if threads > 0 => threads,
        _ => {
            eprintln!("invalid thread count '{}'", args[1]);
            return 1;
        }
    };

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

    let base_seed = seed_from_entropy();
    println!("base seed: {:016x}", base_seed);

    std::thread::scope(|s| {
        for id in 0..threads {
            let seed = splitmix64(base_seed ^ id as u64);
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
