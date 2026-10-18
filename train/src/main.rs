mod inputs;
mod loader;

use bullet_lib::{
    nn::optimiser::{AdamW, AdamWParams},
    trainer::{
        save::SavedFormat,
        schedule::{TrainingSchedule, TrainingSteps, lr, wdl},
        settings::LocalSettings,
    },
    value::ValueTrainerBuilder,
};
use inputs::*;
use loader::TakReader;
use syntaks::core::{Score, is_decisive};
use syntaks::format::GameResult;
use syntaks::position::Position;
use syntaks::takmove::Move;

const L1: usize = 768;
const L2: usize = 32;
const L3: usize = 32;
const OUTPUT_BUCKETS: usize = 16;
const SCALE: i32 = 400;
const Q0: i16 = 255;
const Q1: i16 = 128;
const Q: i32 = 64;

const FT_SHIFT: usize = 8;
const FT_SHIFT_SCALE: f32 = Q0 as f32 / ((1 << FT_SHIFT) as f32);
const I8_RANGE: f32 = i8::MAX as f32 / (Q1 as f32);
const L1_RANGE: f32 = I8_RANGE * FT_SHIFT_SCALE * FT_SHIFT_SCALE;

const PACKUS_LANE: usize = 8;
const PACKUS_WIDTH: usize = 32;

const SUPERBATCHES: usize = 200;

fn filter(_pos: &Position, _mv: Move, score: Score, _result: GameResult) -> bool {
    !is_decisive(score)
}

fn packus_position(neuron: usize) -> usize {
    let chunk = neuron % (2 * PACKUS_WIDTH);
    let lane = chunk / (2 * PACKUS_LANE);
    let half = chunk % (2 * PACKUS_LANE) / PACKUS_LANE;
    neuron - chunk + half * PACKUS_WIDTH + lane * PACKUS_LANE + chunk % PACKUS_LANE
}

fn packus_layout(weights: Vec<f32>) -> Vec<f32> {
    let mut interleaved = vec![0.0; weights.len()];
    for (row, out) in weights
        .as_chunks::<L1>()
        .0
        .iter()
        .zip(interleaved.as_chunks_mut::<L1>().0)
    {
        for (neuron, &w) in row.iter().enumerate() {
            out[packus_position(neuron)] = w;
        }
    }
    interleaved
}

fn bucket_major(weights: Vec<f32>, inner: usize) -> Vec<f32> {
    let rows = weights.len() / (OUTPUT_BUCKETS * inner);
    let mut out = vec![0.0; weights.len()];
    for row in 0..rows {
        for ob in 0..OUTPUT_BUCKETS {
            let from = (row * OUTPUT_BUCKETS + ob) * inner;
            let to = (ob * rows + row) * inner;
            out[to..to + inner].copy_from_slice(&weights[from..from + inner]);
        }
    }
    out
}

fn l1w_layout(weights: Vec<f32>) -> Vec<f32> {
    let bucketed = bucket_major(weights, L2);
    let mut out = vec![0.0; bucketed.len()];
    for ob in 0..OUTPUT_BUCKETS {
        for neuron in 0..L1 {
            for j in 0..L2 {
                out[ob * L1 * L2 + neuron / 4 * 4 * L2 + j * 4 + neuron % 4] = bucketed[(ob * L1 + neuron) * L2 + j];
            }
        }
    }
    out
}

fn l2w_layout(weights: Vec<f32>) -> Vec<f32> {
    let bucketed = bucket_major(weights, L3);
    let mut out = vec![0.0; bucketed.len()];
    for ob in 0..OUTPUT_BUCKETS {
        for k in 0..L2 / 2 {
            let base = k / 4 * 4 + k % 2 * 2 + if k % 4 >= 2 { L2 / 2 } else { 0 };
            for j in 0..L3 {
                let to = ((ob * L2 / 2 + k) * L3 + j) * 2;
                out[to] = bucketed[(ob * L2 + base) * L3 + j];
                out[to + 1] = bucketed[(ob * L2 + base + 1) * L3 + j];
            }
        }
    }
    out
}

fn main() {
    let data_paths: Vec<String> = std::env::args().skip(1).collect();
    assert!(!data_paths.is_empty());

    let mut trainer = ValueTrainerBuilder::default()
        .dual_perspective()
        .optimiser(AdamW)
        .inputs(TakTopRelativeStacks)
        .output_buckets(StmReserveBuckets)
        .save_format(&[
            SavedFormat::id("l0w")
                .transform(|_, values| packus_layout(values))
                .round()
                .quantise::<i16>(Q0),
            SavedFormat::id("l0b")
                .transform(|_, values| packus_layout(values))
                .round()
                .quantise::<i16>(Q0),
            SavedFormat::id("l1w")
                .transform(|_, values| values.iter().map(|f| f / (FT_SHIFT_SCALE * FT_SHIFT_SCALE)).collect())
                .transform(|_, values| l1w_layout(values))
                .round()
                .quantise::<i8>(Q1),
            SavedFormat::id("l1b").round().quantise::<i32>(Q * 256),
            SavedFormat::id("l2w")
                .transform(|_, values| l2w_layout(values))
                .round()
                .quantise::<i16>(Q as i16),
            SavedFormat::id("l2b").round().quantise::<i32>(Q.pow(3)),
            SavedFormat::id("l3w")
                .transform(|_, values| bucket_major(values, 1))
                .round()
                .quantise::<i32>(Q),
            SavedFormat::id("l3b").round().quantise::<i32>(Q.pow(4)),
        ])
        .loss_fn(|output, target| output.sigmoid().squared_error(target))
        .build(|builder, stm_inputs, ntm_inputs, output_buckets| {
            let l0 = builder.new_affine("l0", TakTopRelativeStacks::NUM_INPUTS, L1);
            let l1 = builder.new_affine("l1", L1, OUTPUT_BUCKETS * L2);
            let l2 = builder.new_affine("l2", L2, OUTPUT_BUCKETS * L3);
            let l3 = builder.new_affine("l3", L3, OUTPUT_BUCKETS);

            let ft = |input, start, end| l0.slice(start, end).forward(input).crelu();
            let stm_hidden = ft(stm_inputs, 0, L1 / 2) * ft(stm_inputs, L1 / 2, L1);
            let ntm_hidden = ft(ntm_inputs, 0, L1 / 2) * ft(ntm_inputs, L1 / 2, L1);

            let hl1 = stm_hidden.concat(ntm_hidden);
            let hl2 = l1.forward(hl1).select(output_buckets).screlu();
            let hl3 = l2.forward(hl2).select(output_buckets).crelu();
            l3.forward(hl3).select(output_buckets)
        });

    let l1_clip = AdamWParams {
        max_weight: L1_RANGE,
        min_weight: -L1_RANGE,
        ..Default::default()
    };
    trainer.optimiser.set_params_for_weight("l1w", l1_clip);

    let schedule = TrainingSchedule {
        net_id: "net026".to_owned(),
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
