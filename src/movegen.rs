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

use crate::board::Position;
use crate::core::{Direction, PieceType, Square};
use crate::hits::find_hits;
use crate::takmove::Move;

fn generate_starting_moves<F: FnMut(Move)>(func: &mut F, pos: &Position) {
    for sq in !pos.occ() {
        func(Move::placement(PieceType::Flat, sq));
    }
}

fn generate_placements<F: FnMut(Move)>(func: &mut F, pos: &Position) {
    let flats = pos.flats_in_hand(pos.stm());
    let caps = pos.caps_in_hand(pos.stm());

    if flats == 0 && caps == 0 {
        return;
    }

    for sq in !pos.occ() {
        if caps > 0 {
            func(Move::placement(PieceType::Capstone, sq));
        }

        if flats > 0 {
            func(Move::placement(PieceType::Flat, sq));
            func(Move::placement(PieceType::Wall, sq));
        }
    }
}

fn do_spreads<F: FnMut(Move)>(
    func: &mut F,
    sq: Square,
    dir: Direction,
    lsb: u16,
    mut pattern: u16,
    dist: u32,
    limit: u16,
) {
    assert!(dist > 0);
    while pattern < limit {
        func(Move::spread(sq, dir, pattern));
        if pattern.count_ones() == dist {
            pattern += pattern & pattern.wrapping_neg();
        } else {
            pattern += lsb;
        }
    }
}

fn generate_spreads<F: FnMut(Move)>(func: &mut F, pos: &Position) {
    for sq in pos.player_bb(pos.stm()) {
        let top = pos.stacks().top(sq).unwrap();
        let max = pos.stacks().height(sq).min(6);

        let start_bit = (1 << Position::CARRY_LIMIT) >> max;

        let hits = find_hits(pos.all_blockers(), sq);

        for dir in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            let (mut dist, hit_sq) = hits[dir.idx()];

            if dist == 0 {
                continue;
            }

            let mut limit = 1 << Position::CARRY_LIMIT;

            match pos.stacks().top(hit_sq) {
                Some(PieceType::Wall) => {
                    if top == PieceType::Capstone {
                        // Can smash - generate spreads here with msb set
                        do_spreads(
                            func,
                            sq,
                            dir,
                            start_bit,
                            1 << (Position::CARRY_LIMIT - 1),
                            dist as u32,
                            limit,
                        );
                        limit >>= 1;
                    }
                    dist -= 1;
                }
                Some(PieceType::Capstone) => {
                    dist -= 1;
                }
                _ => {}
            }

            if dist == 0 {
                continue;
            }

            do_spreads(func, sq, dir, start_bit, start_bit, dist as u32, limit);
        }
    }
}

pub fn generate_moves<F: FnMut(Move)>(func: &mut F, pos: &Position) {
    if pos.ply() < 2 {
        generate_starting_moves(func, pos);
        return;
    }

    generate_placements(func, pos);
    generate_spreads(func, pos);
}

pub fn generate_moves_into(dst: &mut Vec<Move>, pos: &Position) {
    generate_moves(&mut |mv| dst.push(mv), pos);
}
