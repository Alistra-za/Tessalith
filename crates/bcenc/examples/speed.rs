//! Speed of `bcenc` against the vendored rgbcx (C++), per format, on one thread.
//!
//! Run (release build, quiet machine; timings on a busy machine are noise, and the `rust/cpp` ratio is the figure
//! to compare between runs, because both encoders run in alternating passes under the same load):
//!
//! ```text
//! cargo run --offline --release -p bcenc --example speed -- [--path=<base|avx2|best>] <runs> [<png>...]
//! ```
//!
//! `--path` picks the code path `bcenc` runs on, through its hidden `_on` entries (default `best`, the one the public
//! functions take on this CPU); a path this machine cannot run, or an unknown name, exits with status 2 and says why.
//! The path goes to stderr.
//!
//! Every 4x4 block of level 0 of the PNGs (`image::open(..).to_rgba8()`, cut by `encoders_sys::gather_blocks` as
//! `tests/corpus.rs` does) is encoded `runs` times by each encoder in alternating passes; rgbcx goes first in even
//! runs and `bcenc` in odd ones, so neither always meets the colder caches. With no PNG the blocks are `gen::colour_classes(1, 200)`, so the example runs without owner data.
//!
//! One line per format on stdout: `format  cpp_med_us  rust_med_us  rust/cpp  same_bytes`. The two times are the
//! median (of an even count, the upper middle one) over the runs of one whole pass over all blocks, in
//! microseconds. `same_bytes` compares a 64-bit FNV-1a
//! hash over every output block of the first pass of each encoder (`yes` or `NO`). The process exits with status 1
//! when any format printed `NO`; the index of the first differing block then goes to stderr, as the block count
//! does.
//!
//! Formats and the arguments the engine passes, each through the `_on` entry of the public function named:
//! `bc1` = `bc1_encode_block(b, true, false)`; `bc1p` = the same perceptual; `bc1-4` = `allow_3color` false (BC3's
//! colour half); `bc1a` = `bc1_encode_block_alpha(b, mask, false)` with bit i of `mask` set when texel i's green is
//! below 64; `bc3` = radius 5, cut 128; `bc4` = channel 0, radius 5; `bc5` = radius 5. Then the fast search
//! (`Bc1Search::Fast`) against rgbcx at `BC1_FAST_LEVEL`: `bc1-fast` = `bc1` and `bc3-fast` = `bc3` at that search.

use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;

use bcenc::{Bc1Search, Path, BC1_FAST_LEVEL};

#[path = "../tests/support/gen.rs"]
mod gen;

type Block = [[u8; 4]; 16];

/// The search radius the engine uses for BC3, BC4 and BC5 by default (`BC45_RADIUS_DEFAULT`).
const RADIUS: u32 = 5;
/// The BC3 alpha cut of the default cutout.
const CUT: u8 = 128;

/// Bit i set when texel i's green is below 64: a stand-in transparency mask for `bc1a`.
fn mask(b: &Block) -> u16 {
    b.iter()
        .enumerate()
        .fold(0u16, |m, (i, t)| m | (u16::from(t[1] < 64) << i))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// One pass of `f` over all blocks, appending every output block to `out`; returns the pass time in microseconds.
fn pass<const N: usize>(
    blocks: &[Block],
    f: &impl Fn(&Block) -> [u8; N],
    out: &mut Vec<u8>,
) -> f64 {
    out.clear();
    let started = Instant::now();
    for b in blocks {
        out.extend_from_slice(&f(black_box(b)));
    }
    black_box(&out);
    started.elapsed().as_secs_f64() * 1e6
}

/// The middle value; of an even count, the upper middle one.
fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Time one format and print its line; true when both encoders produced the same bytes.
fn measure<const N: usize>(
    name: &str,
    blocks: &[Block],
    runs: usize,
    cpp: impl Fn(&Block) -> [u8; N],
    rust: impl Fn(&Block) -> [u8; N],
) -> bool {
    let (mut cpp_t, mut rust_t) = (Vec::new(), Vec::new());
    let (mut cpp_out, mut rust_out) = (
        Vec::with_capacity(blocks.len() * N),
        Vec::with_capacity(blocks.len() * N),
    );
    let (mut cpp_hash, mut rust_hash) = (0, 0);
    for run in 0..runs {
        if run % 2 == 0 {
            cpp_t.push(pass(blocks, &cpp, &mut cpp_out));
            rust_t.push(pass(blocks, &rust, &mut rust_out));
        } else {
            rust_t.push(pass(blocks, &rust, &mut rust_out));
            cpp_t.push(pass(blocks, &cpp, &mut cpp_out));
        }
        if run == 0 {
            cpp_hash = fnv1a(&cpp_out);
            rust_hash = fnv1a(&rust_out);
            let (a, b) = (cpp_out.as_chunks::<N>().0, rust_out.as_chunks::<N>().0);
            if let Some(i) = a.iter().zip(b).position(|(a, b)| a != b) {
                eprintln!("{name}: the first differing block is block {i}");
            }
        }
    }
    let (c, r) = (median(&mut cpp_t), median(&mut rust_t));
    let same = cpp_hash == rust_hash;
    println!(
        "{name:<8} {c:>12.0} {r:>12.0} {:>8.3}  {}",
        r / c,
        if same { "yes" } else { "NO" }
    );
    same
}

fn load(paths: &[String]) -> Vec<Block> {
    if paths.is_empty() {
        return gen::colour_classes(1, 200);
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

/// The path `--path=<name>` names: `best`, or a path of `Path::ALL` this machine can run.
fn named_path(name: &str) -> Result<Path, String> {
    if name == "best" {
        return Ok(Path::best());
    }
    let Some(path) = Path::ALL.into_iter().find(|p| p.name() == name) else {
        let names: Vec<&str> = Path::ALL.iter().map(|p| p.name()).collect();
        return Err(format!(
            "unknown path {name}; the paths are {} and best",
            names.join(", ")
        ));
    };
    match path.missing() {
        Some(why) => Err(format!("path {name} is not available: {why}")),
        None => Ok(path),
    }
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let path = match args.first().and_then(|a| a.strip_prefix("--path=")) {
        None => Path::best(),
        Some(name) => match named_path(name) {
            Ok(p) => {
                args.remove(0);
                p
            }
            Err(why) => {
                eprintln!("{why}");
                return ExitCode::from(2);
            }
        },
    };
    let Some(runs) = args
        .first()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= 1)
    else {
        eprintln!("usage: speed [--path=<base|avx2|best>] <runs (1 or more)> [<png>...]");
        return ExitCode::from(2);
    };
    eprintln!("path {}", path.name());
    let blocks = load(&args[1..]);
    if blocks.is_empty() {
        eprintln!("no 4x4 block in the given PNGs");
        return ExitCode::from(2);
    }
    eprintln!("{} blocks, {runs} runs", blocks.len());
    let ok = [
        measure(
            "bc1",
            &blocks,
            runs,
            |b| encoders_sys::bc1_encode_block(b, true, false, false),
            |b| bcenc::bc1_encode_block_on(path, b, true, false, Bc1Search::Default),
        ),
        measure(
            "bc1p",
            &blocks,
            runs,
            |b| encoders_sys::bc1_encode_block(b, true, false, true),
            |b| bcenc::bc1_encode_block_on(path, b, true, true, Bc1Search::Default),
        ),
        measure(
            "bc1-4",
            &blocks,
            runs,
            |b| encoders_sys::bc1_encode_block(b, false, false, false),
            |b| bcenc::bc1_encode_block_on(path, b, false, false, Bc1Search::Default),
        ),
        measure(
            "bc1a",
            &blocks,
            runs,
            |b| encoders_sys::bc1_encode_block_alpha(b, mask(b), false),
            |b| bcenc::bc1_encode_block_alpha_on(path, b, mask(b), false, Bc1Search::Default),
        ),
        measure(
            "bc3",
            &blocks,
            runs,
            |b| encoders_sys::bc3_encode_block(b, RADIUS, CUT, false),
            |b| bcenc::bc3_encode_block_raw_cut_on(path, b, RADIUS, CUT, false, Bc1Search::Default),
        ),
        measure(
            "bc4",
            &blocks,
            runs,
            |b| encoders_sys::bc4_encode_block(b, 0, RADIUS),
            |b| bcenc::bc4_encode_block_on(path, b, 0, RADIUS),
        ),
        measure(
            "bc5",
            &blocks,
            runs,
            |b| encoders_sys::bc5_encode_block(b, RADIUS),
            |b| bcenc::bc5_encode_block_on(path, b, RADIUS),
        ),
        measure(
            "bc1-fast",
            &blocks,
            runs,
            |b| encoders_sys::bc1_encode_block_level(b, BC1_FAST_LEVEL, true, false),
            |b| bcenc::bc1_encode_block_on(path, b, true, false, Bc1Search::Fast),
        ),
        measure(
            "bc3-fast",
            &blocks,
            runs,
            |b| encoders_sys::bc3_encode_block_level(b, RADIUS, CUT, BC1_FAST_LEVEL, false),
            |b| bcenc::bc3_encode_block_raw_cut_on(path, b, RADIUS, CUT, false, Bc1Search::Fast),
        ),
    ];
    if ok.iter().all(|&s| s) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
