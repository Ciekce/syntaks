use crate::board::Position;
use crate::core::Player;
use crate::eval::static_eval;
use crate::limit::Limits;
use crate::perft::{perft, split_perft};
use crate::search;
use crate::search::Searcher;
use crate::ttable::{DEFAULT_TT_SIZE_MIB, MAX_TT_SIZE_MIB};
use std::time::Instant;

const NAME: &str = "syntaks";
const AUTHORS: &str = "Ciekce";
const VERSION: &str = env!("CARGO_PKG_VERSION");

struct TeiHandler {
    pos: Position,
    key_history: Vec<u64>,
    searcher: Searcher,
}

impl TeiHandler {
    #[must_use]
    fn new() -> Self {
        Self {
            pos: Position::startpos(),
            key_history: Vec::with_capacity(1024),
            searcher: Searcher::new(),
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
                "d" => self.handle_d(),
                "perft" => self.handle_perft(args),
                "splitperft" => self.handle_splitperft(args),
                "quit" => break,
                unknown => eprintln!("Unknown command '{}'", unknown),
            }

            line.clear();
        }
    }

    fn handle_tei(&self) {
        let half_komi = Position::KOMI * 2;

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

        println!("teiok");
    }

    fn handle_teinewgame(&mut self, args: &[&str]) {
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
            "hash" => {
                if let Ok(size) = value.parse::<usize>() {
                    let size = size.clamp(1, MAX_TT_SIZE_MIB);
                    self.searcher.set_tt_size(size);
                }
            }
            unknown => eprintln!("Unknown option '{}'", unknown),
        }
    }

    fn handle_isready(&self) {
        println!("readyok");
    }

    fn handle_position(&mut self, args: &[&str]) {
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
                let count = args
                    .iter()
                    .position(|&s| s == "moves")
                    .unwrap_or(args.len());

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
        let mut limits = Limits::new(start_time);
        let mut max_depth = None;

        let mut wtime = None;
        let mut btime = None;
        let mut winc = None;
        let mut binc = None;

        let mut i = 0;
        while i < args.len() {
            let limit_str = args[i];
            match limit_str {
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
                        if !limits.set_nodes(nodes) {
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
                unsupported => eprintln!("Unsupported limit '{}'", unsupported),
            }

            i += 1;
        }

        let (our_time, our_inc) = match self.pos.stm() {
            Player::P1 => (wtime, winc),
            Player::P2 => (btime, binc),
        };

        if our_inc.is_some() && our_time.is_none() {
            println!("info string Warning: increment given but no base time");
        }

        if let Some(our_time) = our_time {
            let our_inc = our_inc.unwrap_or(0.0);
            limits.set_time_manager(our_time, our_inc);
        }

        let max_depth = max_depth
            .unwrap_or(search::MAX_PLY)
            .clamp(1, search::MAX_PLY);

        self.searcher
            .start_search(&self.pos, &self.key_history, start_time, limits, max_depth);
    }

    fn handle_d(&self) {
        println!("TPS: {}", self.pos.tps());
        println!("Key: {:016x}", self.pos.key());

        let static_eval = static_eval(&self.pos);
        let static_eval = match self.pos.stm() {
            Player::P1 => static_eval,
            Player::P2 => -static_eval,
        };

        println!(
            "Static eval (P1-relative): {:+.2}",
            (static_eval as f64) / 100.0
        );
    }

    fn handle_perft(&self, args: &[&str]) {
        if args.is_empty() {
            eprintln!("Missing depth");
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
    handler.run();
}
