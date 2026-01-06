use crate::perft::perft;

mod bitboard;
mod board;
mod core;
mod movegen;
mod perft;
mod takmove;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 3 {
        println!("{} <tps> <depth>", args[0]);
        return;
    }

    let pos = args[1].parse().unwrap();
    let depth = args[2].parse().unwrap();

    println!("{}", perft(&pos, depth));
}
