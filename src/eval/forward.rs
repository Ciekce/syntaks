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
use crate::core::Color;
use crate::eval::accumulator::Accumulator;
use crate::eval::nnue::*;
use std::mem::MaybeUninit;

#[repr(C, align(64))]
struct Align64<T: ?Sized>(pub T);

const FT_SHIFT: u32 = 7;

fn activate_ft(stm: &[i16; L1_SIZE], nstm: &[i16; L1_SIZE]) -> Align64<[u8; L1_SIZE]> {
    let mut out = Align64([const { MaybeUninit::<u8>::uninit() }; L1_SIZE]);

    let zero = zero_i16();
    let one = set1_i16(FT_Q as i16);

    unsafe {
        let mut activate_perspective = |offset: usize, v: &[i16; L1_SIZE]| {
            for i in (0..(L1_SIZE / 2)).step_by(CHUNK_SIZE_16 * 2) {
                let i1_0 = load_i16(v.as_ptr().add(i + CHUNK_SIZE_16 * 0));
                let i1_1 = load_i16(v.as_ptr().add(i + CHUNK_SIZE_16 * 1));

                let i2_0 = load_i16(v.as_ptr().add(L1_SIZE / 2 + i + CHUNK_SIZE_16 * 0));
                let i2_1 = load_i16(v.as_ptr().add(L1_SIZE / 2 + i + CHUNK_SIZE_16 * 1));

                let i1_0 = min_i16(i1_0, one);
                let i1_1 = min_i16(i1_1, one);

                let i2_0 = min_i16(i2_0, one);
                let i2_1 = min_i16(i2_1, one);

                let i1_0 = max_i16(i1_0, zero);
                let i1_1 = max_i16(i1_1, zero);

                let p_0 = shli_mulhi_i16::<FT_SHIFT>(i1_0, i2_0);
                let p_1 = shli_mulhi_i16::<FT_SHIFT>(i1_1, i2_1);

                let packed = packus_i16(p_0, p_1);

                store_u8(out.0.as_mut_ptr().add(offset + i).cast(), packed);
            }
        };

        activate_perspective(0, stm);
        activate_perspective(L1_SIZE / 2, nstm);

        Align64(MaybeUninit::assume_init(out.0.into()))
    }
}

fn propagate_l1(bucket: usize, ft_out: &Align64<[u8; L1_SIZE]>) -> Align64<[i32; L2_SIZE]> {
    unsafe {
        let l1w = &NET.l1w[bucket];
        let l1b = &NET.l1b[bucket];

        let (ft_out, _) = ft_out.0.as_chunks::<4>();

        let mut intermediate = [[zero_i32(); 4]; L2_SIZE / CHUNK_SIZE_32];

        for i0 in (0..(L1_SIZE / 4)).step_by(4) {
            for j in 0..(L2_SIZE / CHUNK_SIZE_32) {
                for i1 in 0..4 {
                    let i = i0 + i1;
                    let v = cast_u8_i32(set1_i32(zerocopy::transmute!(ft_out[i])));
                    let w = load_i8(l1w[i].as_ptr().add(j * CHUNK_SIZE_8));
                    intermediate[j][i1] = dpbusd_i32(intermediate[j][i1], v, w);
                }
            }
        }

        let mut out = Align64([0; L2_SIZE]);

        for i in (0..L2_SIZE).step_by(CHUNK_SIZE_32) {
            let sum = intermediate[i / CHUNK_SIZE_32].into_iter().reduce(add_i32).unwrap();

            let biases = load_i32(l1b.as_ptr().add(i));

            let v = add_i32(shri_i32::<2>(sum), biases);

            let v = clamp_i32(v, zero_i32(), set1_i32(Q * Q));
            let v = mul_i32(v, v);

            let v = shri_i32::<{ Q_BITS * 2 }>(v);

            store_i32(out.0.as_mut_ptr().add(i), v);
        }

        out
    }
}

fn propagate_l2(bucket: usize, l1_out: &Align64<[i32; L2_SIZE]>) -> Align64<[i32; L3_SIZE]> {
    unsafe {
        let l2w = &NET.l2w[bucket];

        let mut out = Align64(NET.l2b[bucket]);

        for i in 0..L2_SIZE {
            let v = set1_i32(l1_out.0[i]);
            for j in (0..L3_SIZE).step_by(CHUNK_SIZE_32 * 2) {
                let w_0 = load_i32(l2w[i].as_ptr().add(j + CHUNK_SIZE_32 * 0));
                let w_1 = load_i32(l2w[i].as_ptr().add(j + CHUNK_SIZE_32 * 1));

                let out_0 = load_i32(out.0.as_ptr().add(j + CHUNK_SIZE_32 * 0));
                let out_1 = load_i32(out.0.as_ptr().add(j + CHUNK_SIZE_32 * 1));

                let p_0 = mul_i32(v, w_0);
                let p_1 = mul_i32(v, w_1);

                let out_0 = add_i32(out_0, p_0);
                let out_1 = add_i32(out_1, p_1);

                store_i32(out.0.as_mut_ptr().add(j + CHUNK_SIZE_32 * 0), out_0);
                store_i32(out.0.as_mut_ptr().add(j + CHUNK_SIZE_32 * 1), out_1);
            }
        }

        out
    }
}

fn propagate_l3(bucket: usize, l2_preact: &Align64<[i32; L3_SIZE]>) -> i32 {
    unsafe {
        let l3w = &NET.l3w[bucket];

        let zero = zero_i32();
        let one = set1_i32(Q.pow(3));

        let mut out_0 = zero_i32();
        let mut out_1 = zero_i32();

        for i in (0..L3_SIZE).step_by(CHUNK_SIZE_32 * 2) {
            let i_0 = load_i32(l2_preact.0.as_ptr().add(i + CHUNK_SIZE_32 * 0));
            let i_1 = load_i32(l2_preact.0.as_ptr().add(i + CHUNK_SIZE_32 * 1));

            let w_0 = load_i32(l3w.as_ptr().add(i + CHUNK_SIZE_32 * 0));
            let w_1 = load_i32(l3w.as_ptr().add(i + CHUNK_SIZE_32 * 1));

            let i_0 = clamp_i32(i_0, zero, one);
            let i_1 = clamp_i32(i_1, zero, one);

            let i_0 = mul_i32(i_0, w_0);
            let i_1 = mul_i32(i_1, w_1);

            out_0 = add_i32(out_0, i_0);
            out_1 = add_i32(out_1, i_1);
        }

        let out = add_i32(out_0, out_1);

        hsum_i32(out) + NET.l3b[bucket]
    }
}

#[must_use]
pub(super) fn forward(acc: &Accumulator, stm: Color) -> i32 {
    let bucket = stm.idx();

    let ft_out = activate_ft(&acc.values[stm.idx()], &acc.values[stm.flip().idx()]);
    let l1_out = propagate_l1(bucket, &ft_out);
    let l2_preact = propagate_l2(bucket, &l1_out);
    let l3_out = propagate_l3(bucket, &l2_preact);

    let out = l3_out as i64 * SCALE as i64;
    let out = out / Q.pow(4) as i64;

    out as i32
}
