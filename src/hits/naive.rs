use crate::bitboard::Bitboard;
use crate::core::{Direction, Square};

#[must_use]
fn find_hit_for_dir_naive(blockers: Bitboard, start: Square, dir: Direction) -> (u8, Square) {
    let mut sq = start;
    let mut dist = 0;

    while let Some(next) = sq.shift_checked(dir) {
        sq = next;
        dist += 1;

        if blockers.has_sq(sq) {
            break;
        }
    }

    (dist, sq)
}

#[must_use]
pub(super) fn find_hits_naive(blockers: Bitboard, start: Square) -> super::Hits {
    std::array::from_fn(|idx| {
        let dir = Direction::from_raw(idx as u8).unwrap();
        find_hit_for_dir_naive(blockers, start, dir)
    })
}
