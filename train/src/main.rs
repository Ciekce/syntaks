mod inputs;
mod loader;

use bullet_lib::game::inputs::{SparseInputType, get_num_buckets};
use bullet_lib::nn::{InitSettings, Shape};
use bullet_lib::{
    nn::optimiser::AdamW,
    trainer::{
        save::SavedFormat,
        schedule::{TrainingSchedule, TrainingSteps, lr, wdl},
        settings::LocalSettings,
    },
    value::ValueTrainerBuilder,
};
use bullet_trainer::optimiser::adam::AdamWParams;
use inputs::*;
use loader::TakReader;
use syntaks::core::{Score, is_decisive};
use syntaks::format::GameResult;
use syntaks::position::Position;
use syntaks::takmove::Move;

const HIDDEN_SIZE: usize = 512;
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;

const SUPERBATCHES: usize = 200;

fn filter(_pos: &Position, _mv: Move, score: Score, _result: GameResult) -> bool {
    !is_decisive(score)
}

fn main() {
    let data_paths: Vec<String> = std::env::args().skip(1).collect();
    assert!(!data_paths.is_empty());

    #[rustfmt::skip]
    let cap_buckets = [
        0, 0, 0, 0, 0, 0,
        0, 1, 1, 1, 1, 0,
        0, 1, 2, 2, 1, 0,
        0, 1, 2, 2, 1, 0,
        0, 1, 1, 1, 1, 0,
        0, 0, 0, 0, 0, 0,
    ];

    let inputs = TakCapBucketedTopRelativeStacks::new(cap_buckets);
    let bucket_count = inputs.num_buckets();

    let mut trainer = ValueTrainerBuilder::default()
        .dual_perspective()
        .optimiser(AdamW)
        .inputs(inputs)
        .output_buckets(StmReserveBuckets)
        .save_format(&[
            SavedFormat::id("ftw")
                .transform(move |store, weights| {
                    let factoriser = store.get("ftf").values.f32().repeat(bucket_count);
                    weights.into_iter().zip(factoriser).map(|(a, b)| a + b).collect()
                })
                .round()
                .quantise::<i16>(QA),
            SavedFormat::id("ftb").round().quantise::<i16>(QA),
            SavedFormat::id("l1w").round().quantise::<i16>(QB).transpose(),
            SavedFormat::id("l1b").round().quantise::<i16>(QA * QB),
        ])
        .loss_fn(|output, target| output.sigmoid().squared_error(target))
        .build(|builder, stm_inputs, ntm_inputs, output_buckets| {
            let ftf = builder.new_weights(
                "ftf",
                Shape::new(HIDDEN_SIZE, TakCapBucketedTopRelativeStacks::NUM_BASE_INPUTS),
                InitSettings::Zeroed,
            );
            let expanded_factorizer = ftf.repeat(bucket_count);

            let mut ft = builder.new_affine("ft", inputs.num_inputs(), HIDDEN_SIZE);
            ft.init_with_effective_input_size(inputs.max_active());
            ft.weights = ft.weights + expanded_factorizer;

            let l1 = builder.new_affine("l1", 2 * HIDDEN_SIZE, 16);

            let stm_hidden = ft.forward(stm_inputs).screlu();
            let ntm_hidden = ft.forward(ntm_inputs).screlu();
            l1.forward(stm_hidden.concat(ntm_hidden)).select(output_buckets)
        });

    let ft_params = AdamWParams {
        max_weight: 0.99,
        min_weight: -0.99,
        ..Default::default()
    };

    trainer.optimiser.set_params_for_weight("ftw", ft_params);
    trainer.optimiser.set_params_for_weight("ftf", ft_params);

    let schedule = TrainingSchedule {
        net_id: "net024".to_owned(),
        eval_scale: SCALE as f32,
        steps: TrainingSteps {
            batch_size: 16_384,
            batches_per_superbatch: 6104,
            start_superbatch: 1,
            end_superbatch: SUPERBATCHES,
        },
        wdl_scheduler: wdl::LinearWDL { start: 0.25, end: 0.5 },
        lr_scheduler: lr::CosineDecayLR {
            initial_lr: 1e-3,
            final_lr: 1e-5,
            final_superbatch: SUPERBATCHES,
        },
        save_rate: SUPERBATCHES,
    };

    let settings = LocalSettings {
        threads: 12,
        test_set: None,
        output_directory: "checkpoints",
        batch_queue_size: 64,
    };

    trainer.run(&schedule, &settings, &TakReader::new(data_paths, 1024, 6, filter));
}
