use bullet_lib::game::inputs::SparseInputType;
use bullet_lib::game::outputs::OutputBuckets;
use bullet_lib::value::loader::{GameResult as LoaderResult, LoadableDataType};
use std::ops::Deref;
use syntaks::core::{PieceType, Player, normalizer};
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
    const STACKS_OFFSET: usize = Player::COUNT * PieceType::COUNT * Square::COUNT;

    pub const NUM_INPUTS: usize =
        Self::STACKS_OFFSET + 12 * Player::COUNT * PieceType::COUNT * Square::COUNT * Player::COUNT;
}

impl SparseInputType for TakTopRelativeStacks {
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

        let transforms = [Player::P1, Player::P2].map(|perspective| normalizer(pos.cap_sq(perspective)));

        let map_sq = |perspective: Player, sq: Square| sq.transform(transforms[perspective.idx()]);

        let stacks = pos.stacks();

        for sq in pos.occ() {
            let stm_sq = map_sq(pos.stm(), sq).idx();
            let nstm_sq = map_sq(pos.stm().flip(), sq).idx();

            let top_pt = stacks.top(sq).unwrap().idx();
            let top_color = stacks.top_player(sq).unwrap().idx();

            let top_stm = ((top_color ^ stm_flip) * PieceType::COUNT + top_pt) * Square::COUNT + stm_sq;
            let top_nstm = ((top_color ^ stm_flip ^ 1) * PieceType::COUNT + top_pt) * Square::COUNT + nstm_sq;
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

                let stm = Self::STACKS_OFFSET
                    + ((((top_color ^ stm_flip) * PieceType::COUNT + top_pt) * Player::COUNT + (player ^ stm_flip))
                        * Square::COUNT
                        + stm_sq)
                        * 12
                    + depth;
                let nstm = Self::STACKS_OFFSET
                    + ((((top_color ^ stm_flip ^ 1) * PieceType::COUNT + top_pt) * Player::COUNT
                        + (player ^ stm_flip ^ 1))
                        * Square::COUNT
                        + nstm_sq)
                        * 12
                    + depth;
                f(stm, nstm);
            }
        }
    }

    fn shorthand(&self) -> String {
        Self::NUM_INPUTS.to_string()
    }

    fn description(&self) -> String {
        "tak top-relative stack inputs".to_string()
    }
}
