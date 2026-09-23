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

use crate::core::Player;
use crate::eval::accumulator::Accumulator;
use crate::eval::nnue::*;

#[must_use]
pub(super) fn forward(acc: &Accumulator, stm: Player) -> i32 {
    use super::simd::*;

    let bucket = stm.idx();

    let zero = zero_i16();
    let one = set1_i16(FT_Q as i16);

    let mut sum = zero_i32();

    for (values, weights) in [stm, stm.flip()]
        .iter()
        .map(|player| &acc.values[player.idx()])
        .zip(&NET.l1w[bucket])
    {
        let values = values as *const i16;
        let weights = weights as *const i16;

        for i in (0..L1_SIZE).step_by(CHUNK_SIZE_I16) {
            let v = unsafe { load_i16(values.offset(i as isize)) };
            let w = unsafe { load_i16(weights.offset(i as isize)) };

            let v = max_i16(v, zero);
            let v = min_i16(v, one);

            let p = mul_i16(v, w);

            let r = madd_i16(p, v);

            sum = add_i32(sum, r);
        }
    }

    let mut sum = hsum_i32(sum);

    sum /= FT_Q;
    sum += i32::from(NET.l1b[bucket]);

    sum * SCALE / (FT_Q * L1_Q)
}
