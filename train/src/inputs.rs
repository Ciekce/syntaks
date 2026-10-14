use bullet_lib::game::inputs::SparseInputType;
use bullet_lib::game::outputs::OutputBuckets;
use bullet_lib::value::loader::{GameResult as LoaderResult, LoadableDataType};
use std::ops::Deref;
use syntaks::core::{Color, PieceType, Square, normalizer};
use syntaks::format::PackedTakBoard;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct LoadedTakBoard(pub PackedTakBoard);

impl Deref for LoadedTakBoard {
    type Target = PackedTakBoard;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl LoadableDataType for LoadedTakBoard {
    fn score(&self) -> i16 {
        self.stm_score() as i16
    }

    fn result(&self) -> LoaderResult {
        [LoaderResult::Loss, LoaderResult::Draw, LoaderResult::Win][self.stm_result().unwrap() as usize]
    }
}

#[derive(Clone, Copy, Default)]
pub struct StmBucket;

impl OutputBuckets<LoadedTakBoard> for StmBucket {
    const BUCKETS: usize = 2;

    fn bucket(&self, pos: &LoadedTakBoard) -> u8 {
        pos.stm().raw()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TakTopRelativeStacks;

impl TakTopRelativeStacks {
    const TOPS_COUNT: usize = Color::COUNT * PieceType::COUNT * Square::COUNT;
    const STACKS_COUNT: usize = 12 * Color::COUNT * PieceType::COUNT * Square::COUNT * Color::COUNT;
    const RESERVES_COUNT: usize = 30 * Color::COUNT;

    const STACKS_OFFSET: usize = Self::TOPS_COUNT;
    const RESERVES_OFFSET: usize = Self::STACKS_OFFSET + Self::STACKS_COUNT;

    pub const NUM_INPUTS: usize = Self::TOPS_COUNT + Self::STACKS_COUNT + Self::RESERVES_COUNT;
}

impl SparseInputType for TakTopRelativeStacks {
    type RequiredDataType = LoadedTakBoard;

    fn num_inputs(&self) -> usize {
        Self::NUM_INPUTS
    }

    fn max_active(&self) -> usize {
        64
    }

    fn map_features<F: FnMut(usize, usize)>(&self, pos: &LoadedTakBoard, mut f: F) {
        let pos = pos.decode_position().unwrap();
        let stm_flip = pos.stm().idx();

        let transforms = [Color::White, Color::Black].map(|perspective| normalizer(pos.cap_sq(perspective)));
        let map_sq = |perspective: Color, sq: Square| sq.transform(transforms[perspective]);

        let stacks = pos.stacks();

        for sq in pos.occ() {
            let stm_sq = map_sq(pos.stm(), sq).idx();
            let nstm_sq = map_sq(pos.stm().flip(), sq).idx();

            let top_pt = stacks.top(sq).unwrap().idx();
            let top_color = stacks.top_color(sq).unwrap().idx();

            let top_stm = ((top_color ^ stm_flip) * PieceType::COUNT + top_pt) * Square::COUNT + stm_sq;
            let top_nstm = ((top_color ^ stm_flip ^ 1) * PieceType::COUNT + top_pt) * Square::COUNT + nstm_sq;
            f(top_stm, top_nstm);

            let height = stacks.height(sq) as usize;

            if height == 1 {
                continue;
            }

            let mut colors = stacks.colors(sq) >> 1;
            let count = height.min(13) - 1;

            for depth in 0..count {
                let color = (colors & 0b1) as usize;

                colors >>= 1;

                let stm = Self::STACKS_OFFSET
                    + ((((top_color ^ stm_flip) * PieceType::COUNT + top_pt) * Color::COUNT + (color ^ stm_flip))
                        * Square::COUNT
                        + stm_sq)
                        * 12
                    + depth;
                let nstm = Self::STACKS_OFFSET
                    + ((((top_color ^ stm_flip ^ 1) * PieceType::COUNT + top_pt) * Color::COUNT
                        + (color ^ stm_flip ^ 1))
                        * Square::COUNT
                        + nstm_sq)
                        * 12
                    + depth;
                f(stm, nstm);
            }
        }

        for color in [Color::White, Color::Black] {
            let reserves = pos.flats_in_hand(color);

            if reserves == 0 {
                continue;
            }

            assert!(reserves < 30);

            let color_idx = color.idx() ^ stm_flip;

            let stm = Self::RESERVES_OFFSET + color_idx * 30 + reserves as usize - 1;
            let nstm = Self::RESERVES_OFFSET + (color_idx ^ 1) * 30 + reserves as usize - 1;

            f(stm, nstm);
        }
    }

    fn shorthand(&self) -> String {
        Self::NUM_INPUTS.to_string()
    }

    fn description(&self) -> String {
        "tak top-relative stack inputs".to_string()
    }
}
