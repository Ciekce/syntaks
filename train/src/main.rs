mod inputs;
mod loader;

use std::fmt::format;

use bullet_lib::{
    nn::optimiser::AdamW,
    trainer::{
        save::SavedFormat,
        schedule::{TrainingSchedule, TrainingSteps, lr, wdl},
        settings::LocalSettings,
    },
    value::ValueTrainerBuilder,
};
use inputs::{NUM_INPUTS, StmBucket, Tak216};
use loader::TakReader;

const HIDDEN_SIZE: usize = 256;
const SCALE: i32 = 400;
const QA: i16 = 255;
const QB: i16 = 64;

const SUPERBATCHES: usize = 50;

fn main() {
    let data_paths: Vec<String> = std::env::args().skip(1).collect();
    assert!(!data_paths.is_empty());

    let mut trainer = ValueTrainerBuilder::default()
        .dual_perspective()
        .optimiser(AdamW)
        .inputs(Tak216)
        //.output_buckets(StmBucket)
        .save_format(&[
            SavedFormat::id("l0w").round().quantise::<i16>(QA),
            SavedFormat::id("l0b").round().quantise::<i16>(QA),
            SavedFormat::id("l1w").round().quantise::<i16>(QB).transpose(),
            SavedFormat::id("l1b").round().quantise::<i16>(QA * QB),
        ])
        .loss_fn(|output, target| output.sigmoid().squared_error(target))
        .build(|builder, stm_inputs, ntm_inputs|{//, output_buckets| {
            let l0 = builder.new_affine("l0", NUM_INPUTS, HIDDEN_SIZE);
            let l1 = builder.new_affine("l1", 2 * HIDDEN_SIZE, 1);

            let stm_hidden = l0.forward(stm_inputs).screlu();
            let ntm_hidden = l0.forward(ntm_inputs).screlu();
            l1.forward(stm_hidden.concat(ntm_hidden))//.select(output_buckets)
        });

    let schedule = TrainingSchedule {
        net_id: format!("{HIDDEN_SIZE}"),
        eval_scale: SCALE as f32,
        steps: TrainingSteps {
            batch_size: 16_384,
            batches_per_superbatch: 6104,
            start_superbatch: 1,
            end_superbatch: SUPERBATCHES,
        },
        wdl_scheduler: wdl::ConstantWDL { value: 0.0 },
        lr_scheduler: lr::CosineDecayLR {
            initial_lr: 1e-3,
            final_lr: 1e-5,
            final_superbatch: SUPERBATCHES,
        },
        save_rate: SUPERBATCHES,
    };

    let settings = LocalSettings {
        threads: 8,
        test_set: None,
        output_directory: "checkpoints",
        batch_queue_size: 64,
    };

    trainer.run(&schedule, &settings, &TakReader::new(data_paths, 1024, 4));
}
