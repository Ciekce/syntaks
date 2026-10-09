use bullet_lib::game::inputs::SparseInputType;
use bullet_lib::game::outputs::OutputBuckets;
use bullet_lib::value::loader::{GameResult as LoaderResult, LoadableDataType};
use std::ops::Deref;
use syntaks::core::{PieceType, Player, Square};
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
pub struct Tak216;

impl Tak216 {
    pub const NUM_INPUTS: usize = 216;
}

impl SparseInputType for Tak216 {
    type RequiredDataType = LoadedTakBoard;

    fn num_inputs(&self) -> usize {
        Self::NUM_INPUTS
    }

    fn max_active(&self) -> usize {
        36
    }

    fn map_features<F: FnMut(usize, usize)>(&self, pos: &LoadedTakBoard, mut f: F) {
        let pos = pos.decode_position().unwrap();
        let stm_flip = pos.stm().idx();

        for sq in pos.occ() {
            let ty = pos.stacks().top(sq).unwrap().idx();
            let color = pos.stacks().top_player(sq).unwrap().idx();
            let sq = sq.idx();

            let stm = ty * 72 + (color ^ stm_flip) * 36 + sq;
            let nstm = ty * 72 + (color ^ stm_flip ^ 1) * 36 + sq;

            f(stm, nstm);
        }
    }

    fn shorthand(&self) -> String {
        "216".to_string()
    }

    fn description(&self) -> String {
        "tops only tak inputs".to_string()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TakStacks;

impl TakStacks {
    const STACKS_OFFSET: usize = Player::COUNT * PieceType::COUNT * Square::COUNT;

    pub const NUM_INPUTS: usize = Self::STACKS_OFFSET + 12 * Square::COUNT * Player::COUNT;
}

impl SparseInputType for TakStacks {
    type RequiredDataType = LoadedTakBoard;

    fn num_inputs(&self) -> usize {
        Self::NUM_INPUTS
    }

    fn max_active(&self) -> usize {
        62
    }

    fn map_features<F: FnMut(usize, usize)>(&self, pos: &LoadedTakBoard, mut f: F) {
        let pos = pos.decode_position().unwrap();
        let stm_flip = pos.stm().idx();

        let stacks = pos.stacks();

        for sq in pos.occ() {
            let sq_idx = sq.idx();

            let top_pt = stacks.top(sq).unwrap().idx();
            let top_color = stacks.top_player(sq).unwrap().idx();

            let top_stm = ((top_color ^ stm_flip) * PieceType::COUNT + top_pt) * Square::COUNT + sq_idx;
            let top_nstm = ((top_color ^ stm_flip ^ 1) * PieceType::COUNT + top_pt) * Square::COUNT + sq_idx;
            f(top_stm, top_nstm);

            let height = stacks.height(sq) as usize;

            if height == 1 {
                continue;
            }

            let mut players = stacks.players(sq) >> 1;
            let count = height.min(13) - 1;

            for depth in 0..count {
                let player = (players & 0b1) as usize;

                players >>= 1;

                let stm = Self::STACKS_OFFSET + ((player ^ stm_flip) * Square::COUNT + sq_idx) * 12 + depth;
                let nstm = Self::STACKS_OFFSET + ((player ^ stm_flip ^ 1) * Square::COUNT + sq_idx) * 12 + depth;
                f(stm, nstm);
            }
        }
    }

    fn shorthand(&self) -> String {
        Self::NUM_INPUTS.to_string()
    }

    fn description(&self) -> String {
        "tak stack inputs".to_string()
    }
}
