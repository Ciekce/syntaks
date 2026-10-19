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

use crate::bitboard::Bitboard;

#[must_use]
pub(super) fn influence(road_occ: Bitboard, edges: &mut [Bitboard; 4]) -> bool {
    let mut masks = edges.map(Bitboard::raw);

    let left_edge = Bitboard::LEFT_EDGE.raw();
    let right_edge = Bitboard::RIGHT_EDGE.raw();

    let road_occ = road_occ.raw();

    let calc_next_masks = |masks: [u64; 4]| {
        masks.map(|mask| mask | (mask << 6) | (mask >> 6) | ((mask << 1) & !left_edge) | ((mask >> 1) & !right_edge))
    };

    let unchanged = |masks: [u64; 4], next_masks: [u64; 4]| {
        masks
            .iter()
            .zip(&next_masks)
            .all(|(&mask, &next_mask)| next_mask & !mask == 0)
    };

    let mut influence_masks = calc_next_masks(masks);
    let next_masks = influence_masks.map(|mask| mask & road_occ);

    let result = 'exit: {
        if unchanged(masks, next_masks) {
            break 'exit false;
        }

        masks = next_masks;

        loop {
            influence_masks = calc_next_masks(masks);
            let next_masks = influence_masks.map(|mask| mask & road_occ);

            if (next_masks[0] & next_masks[1]) | (next_masks[2] & next_masks[3]) != 0 {
                break 'exit true;
            }

            if unchanged(masks, next_masks) {
                break 'exit false;
            }

            masks = next_masks;
        }
    };

    for (mask, edge) in influence_masks.iter_mut().zip(Bitboard::EDGES) {
        *mask |= edge.raw();
    }

    *edges = influence_masks.map(Bitboard::from_raw);

    result
}
