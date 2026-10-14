mod inputs;
mod loader;

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
use syntaks::position::Position;
use syntaks::core::{Score, is_decisive};
use syntaks::format::GameResult;
use syntaks::takmove::Move;

const L1_SIZE: usize = 512;
const L2_SIZE: usize = 16;
const L3_SIZE: usize = 32;

const OUTPUT_BUCKETS: usize = 2;

const SCALE: i32 = 400;

const FT_Q: i16 = 255;
const L1_Q: i16 = 128;
const Q: i16 = 64;

const FT_SHIFT: usize = 8;
const FT_SHIFT_SCALE: f32 = FT_Q as f32 / ((1 << FT_SHIFT) as f32);
const I8_RANGE: f32 = i8::MAX as f32 / L1_Q as f32;
const L1_RANGE: f32 = I8_RANGE * FT_SHIFT_SCALE * FT_SHIFT_SCALE;

const SUPERBATCHES: usize = 100;

fn filter(_pos: &Position, _mv: Move, score: Score, _result: GameResult) -> bool {
    !is_decisive(score)
}

fn main() {
    let data_paths: Vec<String> = std::env::args().skip(1).collect();
    assert!(!data_paths.is_empty());

    let mut trainer = ValueTrainerBuilder::default()
        .dual_perspective()
        .optimiser(AdamW)
        .inputs(TakTopRelativeStacks)
        .output_buckets(StmBucket)
        .save_format(&[
            SavedFormat::id("ftw").round().quantise::<i16>(FT_Q),
            SavedFormat::id("ftb").round().quantise::<i16>(FT_Q),
            SavedFormat::id("l1w")
                .transform(|_, weights| {
                    let mut permuted = vec![0.0; weights.len()];
                    for bucket in 0..OUTPUT_BUCKETS {
                        for i1 in (0..L1_SIZE).step_by(4) {
                            for o in 0..L2_SIZE {
                                for i2 in 0..4 {
                                    let param = weights[((i1 + i2) * OUTPUT_BUCKETS + bucket) * L2_SIZE + o];
                                    permuted[bucket * L1_SIZE * L2_SIZE + i1 * L2_SIZE + o * 4 + i2] = param;
                                }
                            }
                        }
                    }
                    for weight in &mut permuted {
                        *weight /= FT_SHIFT_SCALE * FT_SHIFT_SCALE;
                    }
                    permuted
                })
                .round()
                .quantise::<i8>(L1_Q),
            SavedFormat::id("l1b").round().quantise::<i32>(i32::from(Q).pow(2)),
            SavedFormat::id("l2w").transform(|_, weights| {
                let mut permuted = vec![0.0; weights.len()];
                for bucket in 0..OUTPUT_BUCKETS {
                    for i in 0..L2_SIZE {
                        for o in 0..L3_SIZE {
                            let param = weights[(i * OUTPUT_BUCKETS + bucket) * L3_SIZE + o];
                            permuted[(bucket * L2_SIZE + i) * L3_SIZE + o] = param;
                        }
                    }
                }
                permuted
            }).round().quantise::<i32>(i32::from(Q)),
            SavedFormat::id("l2b").round().quantise::<i32>(i32::from(Q).pow(3)),
            SavedFormat::id("l3w").transpose().round().quantise::<i32>(i32::from(Q)),
            SavedFormat::id("l3b").round().quantise::<i32>(i32::from(Q).pow(4)),
        ])
        .loss_fn(|output, target| output.sigmoid().squared_error(target))
        .build(|builder, stm_inputs, ntm_inputs, output_buckets| {
            let ft = builder.new_affine("ft", TakTopRelativeStacks::NUM_INPUTS, L1_SIZE);
            let l1 = builder.new_affine("l1", L1_SIZE, OUTPUT_BUCKETS * L2_SIZE);
            let l2 = builder.new_affine("l2", L2_SIZE, OUTPUT_BUCKETS * L3_SIZE);
            let l3 = builder.new_affine("l3", L3_SIZE, OUTPUT_BUCKETS);

            let ft = |input, start, end| ft.slice(start, end).forward(input).crelu();
            let stm_hidden = ft(stm_inputs, 0, L1_SIZE / 2) * ft(stm_inputs, L1_SIZE / 2, L1_SIZE);
            let ntm_hidden = ft(ntm_inputs, 0, L1_SIZE / 2) * ft(ntm_inputs, L1_SIZE / 2, L1_SIZE);
            let ft_out = stm_hidden.concat(ntm_hidden);

            let l1_out = l1.forward(ft_out).select(output_buckets);
            let l1_out = l1_out.screlu();

            let l2_out = l2.forward(l1_out).select(output_buckets);
            let l2_out = l2_out.crelu();

            let l3_out = l3.forward(l2_out).select(output_buckets);

            l3_out
        });

    let l1_clip = AdamWParams {
        max_weight: L1_RANGE,
        min_weight: -L1_RANGE,
        ..Default::default()
    };
    trainer.optimiser.set_params_for_weight("l1w", l1_clip);

    let schedule = TrainingSchedule {
        net_id: "net019".to_owned(),
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

    //trainer.run(&schedule, &settings, &TakReader::new(data_paths, 1024, 4, filter));

    trainer.load_from_checkpoint("checkpoints/net019-100");
    trainer.save_quantised("checkpoints/net019-100/quantised.bin").unwrap();
}
