use std::ops::Deref;
use bullet_lib::game::inputs::SparseInputType;
use bullet_lib::game::outputs::OutputBuckets;
use bullet_lib::value::loader::{GameResult as LoaderResult, LoadableDataType};
use syntaks::format::PackedTakBoard;

pub const NUM_INPUTS: usize = 216;

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

impl SparseInputType for Tak216 {
    type RequiredDataType = LoadedTakBoard;

    fn num_inputs(&self) -> usize {
        NUM_INPUTS
    }

    fn max_active(&self) -> usize {
        36
    }

    fn map_features<F: FnMut(usize, usize)>(&self, pos: &LoadedTakBoard, mut f: F) {
        let pos = pos.decode_position().unwrap();

        for sq in pos.occ() {
            let ty = pos.stacks().top(sq).unwrap().idx();
            let color = pos.stacks().top_player(sq).unwrap().idx();
            let sq = sq.idx();

            f(ty * 72 + color * 36 + sq, ty * 72 + (1 - color) * 36 + sq);
        }
    }

    fn shorthand(&self) -> String {
        "216".to_string()
    }

    fn description(&self) -> String {
        "tops only tak inputs".to_string()
    }
}
