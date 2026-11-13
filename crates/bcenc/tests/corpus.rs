//! `bcenc` equals the vendored rgbcx on real images. Each test walks level 0 of every PNG under the folder named by
//! `SDC_BCENC_CORPUS` (read recursively, sorted by path) in 4x4 blocks and compares the bytes of both encoders per
//! block. `SDC_BCENC_CORPUS_EVERY=n` takes every n-th file (default 1). With the variable unset a test prints
//! "skipped" and passes. The tests write nothing.
use rayon::prelude::*;
use std::fmt::Write as _;
use std::num::NonZeroU8;
use std::path::{Path, PathBuf};
use std::time::Instant;

type Block = [[u8; 4]; 16];

/// The search radius the engine uses for BC3, BC4 and BC5 by default (`BC45_RADIUS_DEFAULT`).
const RADIUS: u32 = 5;
/// The BC3 alpha cut of the default cutout (alpha 128 and up passes).
const CUT: u8 = 128;
/// Mismatches printed per image; the image is still finished and every mismatch counted.
const MAX_REPORTED: usize = 5;

/// One mode of one block whose two outputs differ.
struct Diff {
    mode: String,
    got: Vec<u8>,
    want: Vec<u8>,
}

fn cmp(diffs: &mut Vec<Diff>, mode: impl FnOnce() -> String, got: &[u8], want: &[u8]) {
    if got != want {
        diffs.push(Diff {
            mode: mode(),
            got: got.to_vec(),
            want: want.to_vec(),
        });
    }
}

enum Outcome {
    /// The file and why `image` could not decode it.
    Unreadable(PathBuf, String),
    Skipped,
    Done {
        blocks: u64,
        mismatches: u64,
    },
}

fn every() -> usize {
    match std::env::var("SDC_BCENC_CORPUS_EVERY") {
        Err(_) => 1,
        Ok(s) => match s.trim().parse::<usize>() {
            Ok(n) if n >= 1 => n,
            _ => panic!("SDC_BCENC_CORPUS_EVERY must be a whole number of 1 or more, got {s:?}"),
        },
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let rd =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    for entry in rd {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            walk(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        {
            out.push(path);
        }
    }
}

/// Compare one image and print its mismatches as soon as it is done, so a long run shows them early. `wanted`
/// decides from the whole image whether it takes part. The blocks are cut by `encoders_sys::gather_blocks` (edge
/// texels replicated, row by row), the same cut the engine uses.
fn compare_image(
    path: &Path,
    wanted: fn(&[[u8; 4]]) -> bool,
    check: &(impl Fn(&Block, &mut Vec<Diff>) + Sync),
) -> Outcome {
    let img = match image::open(path) {
        Ok(i) => i.to_rgba8(),
        Err(e) => return Outcome::Unreadable(path.to_path_buf(), e.to_string()),
    };
    let (w, h) = img.dimensions();
    // The decoded image is consumed here and the texels dropped after the cut, so one copy stays alive per image.
    let px: Vec<[u8; 4]> = img.into_raw().as_chunks::<4>().0.to_vec();
    if w == 0 || h == 0 || !wanted(&px) {
        return Outcome::Skipped;
    }
    let blocks = encoders_sys::gather_blocks(&px, w, h);
    drop(px);
    let bw = (w as usize).div_ceil(4);
    let (mut count, mut mismatches) = (0u64, 0u64);
    let mut report = String::new();
    let mut diffs = Vec::new();
    for (i, block) in blocks.as_chunks::<16>().0.iter().enumerate() {
        diffs.clear();
        check(block, &mut diffs);
        count += 1;
        if diffs.is_empty() {
            continue;
        }
        mismatches += 1;
        if mismatches as usize <= MAX_REPORTED {
            let (x, y) = ((i % bw) * 4, (i / bw) * 4);
            for d in &diffs {
                let _ = writeln!(
                    report,
                    "MISMATCH {}\n  block x {x} y {y}, mode: {}\n  texels {block:?}\n  bcenc {:?}\n  rgbcx {:?}",
                    path.display(),
                    d.mode,
                    d.got,
                    d.want
                );
            }
        }
    }
    if !report.is_empty() {
        print!("{report}");
    }
    Outcome::Done {
        blocks: count,
        mismatches,
    }
}

fn run(name: &str, wanted: fn(&[[u8; 4]]) -> bool, check: impl Fn(&Block, &mut Vec<Diff>) + Sync) {
    let Some(dir) = std::env::var_os("SDC_BCENC_CORPUS") else {
        println!("skipped: SDC_BCENC_CORPUS is not set");
        return;
    };
    let every = every();
    let started = Instant::now();
    let mut paths = Vec::new();
    walk(Path::new(&dir), &mut paths);
    paths.sort();
    assert!(
        !paths.is_empty(),
        "no PNG found under {}",
        Path::new(&dir).display()
    );
    let files: Vec<PathBuf> = paths.into_iter().step_by(every).collect();
    let outcomes: Vec<Outcome> = files
        .par_iter()
        .map(|p| compare_image(p, wanted, &check))
        .collect();
    let (mut compared, mut skipped, mut unreadable) = (0u64, 0u64, 0u64);
    let (mut blocks, mut mismatches) = (0u64, 0u64);
    for o in &outcomes {
        match o {
            Outcome::Unreadable(p, why) => {
                unreadable += 1;
                println!("unreadable: {}: {why}", p.display());
            }
            Outcome::Skipped => skipped += 1,
            Outcome::Done {
                blocks: b,
                mismatches: m,
            } => {
                compared += 1;
                blocks += b;
                mismatches += m;
            }
        }
    }
    println!(
        "{name}: files compared {compared}, skipped {skipped}, unreadable {unreadable}, blocks compared {blocks}, mismatches {mismatches}, {:.1} s",
        started.elapsed().as_secs_f64()
    );
    assert!(
        unreadable < files.len() as u64,
        "every one of the {} files was unreadable",
        files.len()
    );
    assert_eq!(
        mismatches, 0,
        "{name}: {mismatches} block(s) differ from rgbcx"
    );
}

fn any_transparent(px: &[[u8; 4]]) -> bool {
    px.iter().any(|t| t[3] < 128)
}

fn every_image(_: &[[u8; 4]]) -> bool {
    true
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs); SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc1_equals_rgbcx() {
    run("bc1", every_image, |px, d| {
        for perceptual in [false, true] {
            cmp(
                d,
                || format!("allow_3color true, perceptual {perceptual}"),
                &bcenc::bc1_encode_block(px, true, perceptual),
                &encoders_sys::bc1_encode_block(px, true, false, perceptual),
            );
        }
    });
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs); SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc1_alpha_equals_rgbcx() {
    run("bc1_alpha", any_transparent, |px, d| {
        let mask = px
            .iter()
            .enumerate()
            .fold(0u16, |m, (i, t)| m | (u16::from(t[3] < 128) << i));
        for perceptual in [false, true] {
            cmp(
                d,
                || format!("mask {mask:#06x}, perceptual {perceptual}"),
                &bcenc::bc1_encode_block_alpha(px, mask, perceptual),
                &encoders_sys::bc1_encode_block_alpha(px, mask, perceptual),
            );
        }
    });
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs); SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc3_equals_rgbcx() {
    run("bc3", every_image, |px, d| {
        for perceptual in [false, true] {
            cmp(
                d,
                || format!("radius {RADIUS}, cut {CUT}, perceptual {perceptual}"),
                &bcenc::bc3_encode_block(px, RADIUS, NonZeroU8::new(CUT).unwrap(), perceptual),
                &encoders_sys::bc3_encode_block(px, RADIUS, CUT, perceptual),
            );
        }
    });
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs); SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc4_equals_rgbcx() {
    run("bc4", every_image, |px, d| {
        for channel in [0, 3] {
            cmp(
                d,
                || format!("channel {channel}, radius {RADIUS}"),
                &bcenc::bc4_encode_block(px, channel, RADIUS),
                &encoders_sys::bc4_encode_block(px, channel, RADIUS),
            );
        }
    });
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs); SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc5_equals_rgbcx() {
    run("bc5", every_image, |px, d| {
        cmp(
            d,
            || format!("radius {RADIUS}"),
            &bcenc::bc5_encode_block(px, RADIUS),
            &encoders_sys::bc5_encode_block(px, RADIUS),
        );
    });
}
