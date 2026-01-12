use crate::bitboard::Bitboard;
use crate::core::{Direction, Square};
use crate::hits::common::{generate_mask, pdep};
use crate::hits::naive::find_hits_naive;

#[rustfmt::skip]
const MAGICS: [u64; Square::COUNT] = [
    0x0200204004001181, 0x0101002001108a00, 0x0208081002804803, 0x0402240404000200, 0x0809088808000200, 0x2400402a20000220,
    0x0308004090000108, 0x0040c210003200c2, 0x1610402001080248, 0x00610201018a8000, 0x80204a0800000414, 0x2008010200211100,
    0x45002001488c0208, 0x0181101002501800, 0x1610402001080248, 0x1203040202044100, 0x0420410041830181, 0x0201800440020001,
    0x004c018022401088, 0x008100c010400084, 0x1480410500001060, 0x0020160180100012, 0x0420410041830181, 0x808102010e000001,
    0x1200104200040800, 0x04040c1102000008, 0x0142008202060401, 0x0241010402000ab0, 0xc304081000400002, 0x2008010200211100,
    0xc4480004b0001002, 0x4404012a10004040, 0x1008181008018890, 0x000400881c000000, 0x0802894040220001, 0x8491034420002200,
];

#[rustfmt::skip]
const SHIFTS: [u32; Square::COUNT] = [
    56, 57, 57, 57, 57, 56,
    57, 58, 58, 58, 58, 57,
    57, 58, 58, 58, 58, 57,
    57, 58, 58, 58, 58, 57,
    57, 58, 58, 58, 58, 57,
    56, 57, 57, 57, 57, 56,
];

#[derive(Copy, Clone, Debug)]
struct SquareData {
    inv_mask: u64,
    offset: usize,
}

impl SquareData {
    const fn new() -> Self {
        Self {
            inv_mask: 0,
            offset: 0,
        }
    }
}

struct Data {
    squares: [SquareData; Square::COUNT],
    table_size: usize,
}

const SQUARE_DATA: Data = {
    let mut squares = [SquareData::new(); Square::COUNT];
    let mut table_size = 0;

    let mut idx = 0;
    while let Some(sq) = Square::from_raw(idx) {
        let square_data = &mut squares[sq.idx()];

        square_data.inv_mask = !generate_mask(sq);

        square_data.offset = table_size;
        table_size += 1 << (64 - SHIFTS[sq.idx()]);

        idx += 1;
    }

    Data {
        squares,
        table_size,
    }
};

#[static_init::dynamic]
static HITS: [super::Hits; SQUARE_DATA.table_size] = {
    let mut result = [[(0, Square::A1); Direction::COUNT]; SQUARE_DATA.table_size];
    let mut filled = [false; SQUARE_DATA.table_size];

    for sq in Square::all() {
        let sq_data = &SQUARE_DATA.squares[sq.idx()];

        let magic = MAGICS[sq.idx()];
        let shift = SHIFTS[sq.idx()];

        let mask = !sq_data.inv_mask;

        let max_entries = 1 << mask.count_ones();
        for i in 0..max_entries {
            let blockers = Bitboard::from_raw(pdep(i as u64, mask));

            let idx = sq_data.offset + calc_idx(blockers, sq_data.inv_mask, magic, shift);

            if filled[idx] {
                continue;
            }

            result[idx] = find_hits_naive(blockers, sq);
            filled[idx] = true;
        }
    }

    result
};

#[must_use]
fn calc_idx(blockers: Bitboard, inv_mask: u64, magic: u64, shift: u32) -> usize {
    ((blockers.raw() | inv_mask).wrapping_mul(magic) >> shift) as usize
}

#[must_use]
pub fn find_hit_for_dir_magic(blockers: Bitboard, start: Square, dir: Direction) -> super::Hit {
    find_hits_magic(blockers, start)[dir.idx()]
}

#[must_use]
pub(super) fn find_hits_magic(blockers: Bitboard, start: Square) -> super::Hits {
    let magic = MAGICS[start.idx()];
    let shift = SHIFTS[start.idx()];

    let sq_data = &SQUARE_DATA.squares[start.idx()];

    let idx = calc_idx(blockers, sq_data.inv_mask, magic, shift);
    HITS[sq_data.offset + idx]
}
