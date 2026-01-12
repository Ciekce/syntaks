mod bitboard;
mod board;
mod core;
mod eval;
mod hits;
mod keys;
mod limit;
mod movegen;
mod movepick;
mod perft;
mod road;
mod search;
mod takmove;
mod tei;
mod ttable;

fn main() {
    tei::run();
}
