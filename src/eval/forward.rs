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

use super::simd::*;
use crate::eval::accumulator::Accumulator;
use crate::eval::nnue::*;
use crate::position::Position;

#[repr(C, align(64))]
struct Align64<T>(T);

const PACKUS_LANE: usize = 8;
const PACKUS_WIDTH: usize = 32;

const fn packus_position(neuron: usize) -> usize {
    let chunk = neuron % (2 * PACKUS_WIDTH);
    let lane = chunk / (2 * PACKUS_LANE);
    let half = chunk % (2 * PACKUS_LANE) / PACKUS_LANE;
    neuron - chunk + half * PACKUS_WIDTH + lane * PACKUS_LANE + chunk % PACKUS_LANE
}

const EXPLICIT_MULHI_SHIFT_UP: i32 = 7;
const IMPLIED_MULHI_SHIFT_DOWN: i32 = 16;
const MULHI_SHIFT: i32 = EXPLICIT_MULHI_SHIFT_UP - IMPLIED_MULHI_SHIFT_DOWN;

const SHIFT: i32 = Q0_BITS * 2 + MULHI_SHIFT + Q1_BITS - Q_BITS;

const L2_UNROLL: usize = 8 / (L2_SIZE / CHUNK_SIZE_I32);

#[must_use]
pub(super) fn forward(acc: &Accumulator, pos: &Position) -> i32 {
    let output_bucket = which_output_bucket(pos);

    let mut activated_ft = Align64([0; L1_SIZE]);
    {
        let lo = zero_i16();
        let hi = set1_i16(Q0 as i16);

        for (values, out) in [pos.stm(), pos.stm().flip()]
            .iter()
            .map(|&color| &acc.values[color])
            .zip(activated_ft.0.as_chunks_mut::<L1_PAIR_COUNT>().0)
        {
            let values = values.as_ptr();
            let out = out.as_mut_ptr();

            let activate = |offset: usize| {
                let a = unsafe { load_i16(values.add(offset)) };
                let b = unsafe { load_i16(values.add(offset + L1_PAIR_COUNT)) };

                let a = min_i16(max_i16(a, lo), hi);
                let b = min_i16(b, hi);

                mulhi_shift_i16::<EXPLICIT_MULHI_SHIFT_UP>(a, b)
            };

            for i in (0..L1_PAIR_COUNT).step_by(2 * CHUNK_SIZE_I16) {
                let offset_1 = packus_position(i);
                let offset_2 = packus_position(i + PACKUS_LANE);
                let p = packus_i16(activate(offset_1), activate(offset_2));
                unsafe { store_u8(out.add(i), p) };
            }
        }
    }
    debug_assert!(activated_ft.0.iter().all(|&e| e <= i8::MAX as u8));

    let mut l1_intermediate = [[zero_i32(); L2_SIZE / CHUNK_SIZE_I32]; L2_UNROLL];
    {
        let w = NET.l1w[output_bucket].as_ptr();
        let ft_groups = activated_ft.0.as_ptr();

        for i_outer in (0..L1_SIZE / 4).step_by(2 * L2_UNROLL) {
            for (i_inner, intermediate) in l1_intermediate.iter_mut().enumerate() {
                let i_0 = i_outer + 2 * i_inner;
                let i_1 = i_0 + 1;
                let ft_vec_0 = unsafe { set1_u8x4(ft_groups.add(i_0 * 4)) };
                let ft_vec_1 = unsafe { set1_u8x4(ft_groups.add(i_1 * 4)) };

                for (j, intermediate) in intermediate.iter_mut().enumerate() {
                    let weights_0 = unsafe { load_i8(w.add(i_0 * L2_SIZE * 4 + j * CHUNK_SIZE_I8)) };
                    let weights_1 = unsafe { load_i8(w.add(i_1 * L2_SIZE * 4 + j * CHUNK_SIZE_I8)) };
                    *intermediate = dpbusdx2_i32(*intermediate, ft_vec_0, weights_0, ft_vec_1, weights_1);
                }
            }
        }
    }

    let mut l1_out = Align64([0i16; L2_SIZE]);
    let skip;
    {
        let l1_bias = NET.l1b[output_bucket].as_ptr();

        let mut biased = [zero_i32(); L2_SIZE / CHUNK_SIZE_I32];
        for (i, biased) in biased.iter_mut().enumerate() {
            *biased = unsafe { load_i32(l1_bias.add(i * CHUNK_SIZE_I32)) };
            for unrolled in &l1_intermediate {
                *biased = add_i32(*biased, unrolled[i]);
            }
        }

        skip = to_array_i32(biased[0])[0];

        let hi = set1_i16(1 << (SHIFT + Q_BITS));

        for i in 0..L2_SIZE / CHUNK_SIZE_I16 {
            let packed = packus_i32(biased[i], biased[i + L2_SIZE / CHUNK_SIZE_I16]);
            let clamped = min_u16(packed, hi);
            let screlu = mulhi_i16(clamped, clamped);

            unsafe { store_i16(l1_out.0.as_mut_ptr().add(i * CHUNK_SIZE_I16), screlu) };
        }
    }

    let mut l2_intermediate = [[zero_i32(); 2]; L3_SIZE / CHUNK_SIZE_I32];
    {
        let l2_weights = NET.l2w[output_bucket].as_ptr();
        let l1_pairs = l1_out.0.as_ptr();

        for i in (0..L2_PAIR_COUNT).step_by(2) {
            let l1_vec_0 = unsafe { set1_i16x2(l1_pairs.add(i * 2)) };
            let l1_vec_1 = unsafe { set1_i16x2(l1_pairs.add((i + 1) * 2)) };
            for (j, intermediate) in l2_intermediate.iter_mut().enumerate() {
                let l2_weight_0 = unsafe { load_i16(l2_weights.add((i * L3_SIZE + j * CHUNK_SIZE_I32) * 2)) };
                let l2_weight_1 = unsafe { load_i16(l2_weights.add(((i + 1) * L3_SIZE + j * CHUNK_SIZE_I32) * 2)) };
                *intermediate = dpwssdx2_i32(*intermediate, l1_vec_0, l2_weight_0, l1_vec_1, l2_weight_1);
            }
        }
    }

    let l2_intermediate = l2_intermediate.map(reduce_dpwssd_i32);

    let mut l3_sum = zero_i32();
    {
        let l2_biases = NET.l2b[output_bucket].as_ptr();
        let l3_weights = NET.l3w[output_bucket].as_ptr();

        let lo = zero_i32();
        let hi3 = set1_i32(1 << (3 * Q_BITS));

        for (i, &intermediate) in l2_intermediate.iter().enumerate() {
            let l2_bias = unsafe { load_i32(l2_biases.add(i * CHUNK_SIZE_I32)) };
            let l3_weight = unsafe { load_i32(l3_weights.add(i * CHUNK_SIZE_I32)) };

            let biased = add_i32(intermediate, l2_bias);
            let activated = min_i32(max_i32(biased, lo), hi3);

            l3_sum = add_i32(l3_sum, mul_i32(activated, l3_weight));
        }
    }

    let bias = NET.l3b[output_bucket];
    let skip = i64::from(skip) * i64::from(Q * Q * Q * Q / (Q * 256));
    let scaled = (i64::from(hsum_i32(l3_sum) + bias) + skip) * i64::from(SCALE);

    (scaled / i64::from(Q * Q * Q * Q)) as i32
}
