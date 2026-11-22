//! Report only (BC45-R12): what the Max search costs against Slowest, per complete block encoder. It asserts nothing
//! and is no test.
//!
//! Run (release build, quiet machine; the ratio is the figure to compare between runs, because both encoders run in
//! alternating passes under the same load):
//!
//! ```text
//! cargo run --offline --release -p bcenc --example max_speed -- <runs> [<png>...]
//! ```
//!
//! Every 4x4 block of level 0 of the PNGs (`image::open(..).to_rgba8()`, cut by `encoders_sys::gather_blocks` as
//! `examples/speed.rs` does) is encoded `runs` times by each encoder in alternating passes; Slowest goes first in even
//! runs and Max in odd ones. With no PNG the blocks are `gen::bc4_classes(1, 200)` mapped to `[a, a ^ 0x5A, 255 - a,
//! a]`, so the example runs without owner data.
//!
//! The encoders are the complete block functions, Slowest (radius 10) against Max: `bc3` = `bc3_encode_block_with`
//! against `bc3_encode_block_max_with` at cut 128 (the colour half included); `bc4` = channel 0, `bc4_encode_block`
//! against `bc4_encode_block_max`; `bc5` = `bc5_encode_block` against `bc5_encode_block_max` (both channels).
//!
//! One line per format on stdout: `format  slowest_med_us  max_med_us  max/slowest`. The times are the median (of an
//! even count, the upper middle one) over the runs of one whole pass over all blocks, in microseconds.

use std::hint::black_box;
use std::num::NonZeroU8;
use std::time::Instant;

use bcenc::Bc1Search;

#[path = "../tests/support/gen.rs"]
mod gen;

type Block = [[u8; 4]; 16];

/// The search radius of Slowest (`BC45_RADIUS_MAX`).
const RADIUS: u32 = bcenc::MAX_SEARCH_RADIUS;

/// One pass of `f` over all blocks; returns the pass time in microseconds.
fn pass<const N: usize>(blocks: &[Block], f: &impl Fn(&Block) -> [u8; N]) -> f64 {
    let started = Instant::now();
    let mut sink = 0u8;
    for b in blocks {
        sink ^= f(black_box(b))[0];
    }
    black_box(sink);
    started.elapsed().as_secs_f64() * 1e6
}

/// The middle value; of an even count, the upper middle one.
fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn measure<const N: usize>(
    name: &str,
    blocks: &[Block],
    runs: usize,
    slowest: impl Fn(&Block) -> [u8; N],
    max: impl Fn(&Block) -> [u8; N],
) {
    let (mut slow_t, mut max_t) = (Vec::new(), Vec::new());
    for run in 0..runs {
        if run % 2 == 0 {
            slow_t.push(pass(blocks, &slowest));
            max_t.push(pass(blocks, &max));
        } else {
            max_t.push(pass(blocks, &max));
            slow_t.push(pass(blocks, &slowest));
        }
    }
    let (s, m) = (median(&mut slow_t), median(&mut max_t));
    println!("{name:<4} {s:>12.0} {m:>12.0} {:>8.3}", m / s);
}

fn load(paths: &[String]) -> Vec<Block> {
    if paths.is_empty() {
        return gen::bc4_classes(1, 200)
            .iter()
            .map(|v| v.map(|a| [a, a ^ 0x5A, 255 - a, a]))
            .collect();
    }
    let mut blocks = Vec::new();
    for p in paths {
        let img = image::open(p)
            .unwrap_or_else(|e| panic!("cannot read {p}: {e}"))
            .to_rgba8();
        let (w, h) = img.dimensions();
        let px: Vec<[u8; 4]> = img.into_raw().as_chunks::<4>().0.to_vec();
        let cut = encoders_sys::gather_blocks(&px, w, h);
        blocks.extend_from_slice(cut.as_chunks::<16>().0);
    }
    blocks
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(runs) = args
        .first()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= 1)
    else {
        eprintln!("usage: max_speed <runs (1 or more)> [<png>...]");
        return std::process::ExitCode::from(2);
    };
    let blocks = load(&args[1..]);
    if blocks.is_empty() {
        eprintln!("no 4x4 block in the given PNGs");
        return std::process::ExitCode::from(2);
    }
    eprintln!("{} blocks, {runs} runs", blocks.len());
    let cut = NonZeroU8::new(128).unwrap();
    measure(
        "bc3",
        &blocks,
        runs,
        |b| bcenc::bc3_encode_block_with(b, RADIUS, cut, false, Bc1Search::Default),
        |b| bcenc::bc3_encode_block_max_with(b, cut, false, Bc1Search::Default),
    );
    measure(
        "bc4",
        &blocks,
        runs,
        |b| bcenc::bc4_encode_block(b, 0, RADIUS),
        |b| bcenc::bc4_encode_block_max(b, 0),
    );
    measure(
        "bc5",
        &blocks,
        runs,
        |b| bcenc::bc5_encode_block(b, RADIUS),
        bcenc::bc5_encode_block_max,
    );
    std::process::ExitCode::SUCCESS
}
