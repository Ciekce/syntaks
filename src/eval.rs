use crate::board::Position;
use crate::core::{Piece, Player};
use crate::search::Score;

#[must_use]
pub fn static_eval(pos: &Position) -> Score {
    let p1_flats = pos.player_piece_bb(Piece::P1Flat).popcount() as Score;
    let p2_flats = (pos.player_piece_bb(Piece::P2Flat).popcount() + Position::KOMI) as Score;

    let flat_diff = p1_flats - p2_flats;
    let flat_diff = flat_diff * 100;

    match pos.stm() {
        Player::P1 => flat_diff,
        Player::P2 => -flat_diff,
    }
}
