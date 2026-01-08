use crate::board::Position;
use crate::core::Player;

mod bitboard;
mod board;
mod core;
mod movegen;
mod perft;
mod takmove;
mod tei;

fn main() {
    let pos = "x,1,x4/x,1,2,x3/x,1,2,x3/x,1,2,x3/x,1,2,x3/2,1,x4 2 6"
        .parse::<Position>()
        .unwrap();

    assert!(pos.has_road(Player::P1));
    assert!(!pos.has_road(Player::P2));

    println!("nya");

    tei::run();
}
