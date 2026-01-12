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
use crate::movegen::generate_moves;
use crate::takmove::Move;
use std::time::Instant;

fn do_perft(pos: &Position, depth: i32, movelists: &mut [Vec<Move>]) -> usize {
    if depth <= 0 {
        return 1;
    }

    let (moves, movelists) = movelists.split_first_mut().unwrap();
    generate_moves(moves, pos);

    if depth == 1 {
        return moves.len();
    }

    let mut total = 0;

    for &mut mv in moves {
        debug_assert!(pos.is_legal(mv));

        let pos = pos.apply_move(mv);
        total += do_perft(&pos, depth - 1, movelists);
    }

    total
}

#[must_use]
pub fn perft(pos: &Position, depth: i32) -> usize {
    let mut movelists = vec![Vec::with_capacity(256); depth as usize];
    do_perft(pos, depth.max(1), &mut movelists)
}

pub fn split_perft(pos: &Position, depth: i32) {
    let depth = depth.max(1);

    let mut movelists = vec![Vec::with_capacity(256); depth as usize];

    let start = Instant::now();

    let (moves, movelists) = movelists.split_first_mut().unwrap();
    generate_moves(moves, pos);

    let mut total = 0;

    for &mut mv in moves {
        debug_assert!(pos.is_legal(mv));

        print!("{:9}  ", mv.to_string());

        let pos = pos.apply_move(mv);
        let value = do_perft(&pos, depth - 1, movelists);

        total += value;
        println!("{}", value);
    }

    let nps = (total as f64 / start.elapsed().as_secs_f64()) as usize;

    println!();
    println!("total: {}", total);
    println!("{} nps", nps);
}
