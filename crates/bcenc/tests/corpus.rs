//! `bcenc` equals the vendored rgbcx on real images. Each test walks level 0 of every PNG under the folder named by
//! `SDC_BCENC_CORPUS` (read recursively, sorted by path) in 4x4 blocks and compares the bytes of both encoders per
//! block. `SDC_BCENC_CORPUS_EVERY=n` takes every n-th file (default 1). Each format is its own test, so a run that
//! needs only some formats names them as cargo test filters, for example
//! `cargo test --offline --release -p bcenc --test corpus -- --ignored corpus_bc3 corpus_bc4 corpus_bc5` (with
//! `SDC_BCENC_CORPUS` and `SDC_BCENC_CORPUS_HASHES` set, as below, or every test prints "skipped"; a filter matches every test whose name contains it, so `corpus_bc3` also runs `corpus_bc3_equals_rgbcx_fast`).
//! With `SDC_BCENC_CORPUS` unset a test prints "skipped" and passes. The `*_fast` tests compare `Bc1Search::Fast`
//! with rgbcx at `BC1_FAST_LEVEL`. Every block is compared on every code path the machine has (`support/paths.rs`),
//! through the hidden `_on` entries; a mismatch names its path.
//!
//! A corpus run keeps its per-file hashes (`CODING_STANDARDS.md`, the long-run results rule): with `SDC_BCENC_CORPUS`
//! set, `SDC_BCENC_CORPUS_HASHES=<dir>` is required. If `<dir>\<test>.txt` is missing, the run is a baseline: every
//! path is compared with rgbcx, and after a run with no mismatch and at least one readable file the file is published
//! atomically and never over an existing one (a file that appeared during the run stays, and the run says so) (first line `# commit <text>
//! input <folder> every <n>`, needs `SDC_BCENC_CORPUS_COMMIT=<text>`; then `<relative path> <blocks> <fnv1a64 of the
//! first path's output bytes>` per image). If the file exists, the run is a check: no rgbcx encode, every path is
//! compared with the first one, and each image's block count and digest must equal its line. The file is never
//! rewritten by a check, and its header's `input` and `every` must equal the run's (a panic otherwise).
use bcenc::{Bc1Search, BC1_FAST_LEVEL};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

#[path = "support/paths.rs"]
mod paths;

type Block = [[u8; 4]; 16];

/// The search radius the engine uses for BC3, BC4 and BC5 by default (`BC45_RADIUS_DEFAULT`).
const RADIUS: u32 = 5;
/// The BC3 alpha cut of the default cutout (alpha 128 and up passes).
const CUT: u8 = 128;
/// Mismatches printed per image; the image is still finished and every mismatch counted.
const MAX_REPORTED: usize = 5;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// One mode of one block whose two outputs differ.
struct Diff {
    mode: String,
    got: Vec<u8>,
    want: Vec<u8>,
}

/// What the checks of one block fill: the differing modes, and the image's running digest.
struct Sink {
    diffs: Vec<Diff>,
    hash: u64,
    /// Compare with rgbcx (a baseline run); otherwise every path is compared with the first one.
    vs_rgbcx: bool,
}

fn fnv(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= u64::from(b);
        *h = h.wrapping_mul(FNV_PRIME);
    }
}

/// `got` on every path against the reference; each path that differs is one `Diff`, its mode naming the path. The
/// first path's output feeds the digest once. `want` (the rgbcx encode) is called only in a baseline run.
fn cmp<const N: usize>(
    sink: &mut Sink,
    mode: impl Fn() -> String,
    got: impl Fn(bcenc::Path) -> [u8; N],
    want: impl FnOnce() -> [u8; N],
) {
    let all = paths::paths();
    let first = got(all[0]);
    fnv(&mut sink.hash, &first);
    let (reference, against) = if sink.vs_rgbcx {
        (want(), "")
    } else {
        (first, " vs base")
    };
    for (i, &path) in all.iter().enumerate() {
        if i == 0 && !sink.vs_rgbcx {
            continue;
        }
        let g = if i == 0 { first } else { got(path) };
        if g != reference {
            sink.diffs.push(Diff {
                mode: format!("path {}{against}, {}", path.name(), mode()),
                got: g.to_vec(),
                want: reference.to_vec(),
            });
        }
    }
}

enum Outcome {
    /// The file and why `image` could not decode it.
    Unreadable(PathBuf, String),
    Skipped,
    Done {
        blocks: u64,
        mismatches: u64,
        hash: u64,
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
    vs_rgbcx: bool,
    check: &(impl Fn(&Block, &mut Sink) + Sync),
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
    let mut sink = Sink {
        diffs: Vec::new(),
        hash: FNV_OFFSET,
        vs_rgbcx,
    };
    for (i, block) in blocks.as_chunks::<16>().0.iter().enumerate() {
        sink.diffs.clear();
        check(block, &mut sink);
        count += 1;
        if sink.diffs.is_empty() {
            continue;
        }
        mismatches += 1;
        if mismatches as usize <= MAX_REPORTED {
            let (x, y) = ((i % bw) * 4, (i / bw) * 4);
            for d in &sink.diffs {
                let _ = writeln!(
                    report,
                    "MISMATCH {}\n  block x {x} y {y}, mode: {}\n  texels {block:?}\n  bcenc {:?}\n  reference {:?}",
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
        hash: sink.hash,
    }
}

/// Where a run keeps its per-file hashes and the commit text its baseline file names.
struct HashMode {
    dir: PathBuf,
    commit: String,
}

/// What a run found.
struct Summary {
    files: usize,
    unreadable: u64,
    mismatches: u64,
    line: String,
}

/// The saved lines, by path, as (block count, digest as 16 hex). The first line is the header, whose `input` and
/// `every` must equal the run's (a panic otherwise); a CRLF file reads too; a path listed twice panics.
fn read_saved(file: &Path, input: &Path, every: usize) -> BTreeMap<String, (u64, String)> {
    let text = std::fs::read_to_string(file)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
    let header = text.lines().next().unwrap_or_default();
    // From the right, so a commit text holding " input " reads back; the commit text must not be empty.
    let parsed = header.strip_prefix("# commit ").and_then(|h| {
        let (rest, e) = h.rsplit_once(" every ")?;
        let (commit, i) = rest.rsplit_once(" input ")?;
        (!commit.trim().is_empty()).then_some((i, e))
    });
    let Some((saved_input, saved_every)) = parsed else {
        panic!(
            "{}: the first line is not a header (# commit <text> input <folder> every <n>): {header:?}",
            file.display()
        );
    };
    assert!(
        saved_input == input.display().to_string() && saved_every.trim() == every.to_string(),
        "{} was saved for input {saved_input} every {}; this run is input {} every {every}",
        file.display(),
        saved_every.trim(),
        input.display()
    );
    let mut out = BTreeMap::new();
    for line in text.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        // Paths may contain spaces: the digest and the block count come from the right.
        let mut parts = line.rsplitn(3, ' ');
        let (Some(digest), Some(blocks), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            panic!("{}: malformed line {line:?}", file.display());
        };
        let blocks = blocks
            .parse::<u64>()
            .unwrap_or_else(|_| panic!("{}: bad block count in {line:?}", file.display()));
        let again = out.insert(path.to_string(), (blocks, digest.to_string()));
        assert!(
            again.is_none(),
            "{}: {path:?} is listed twice",
            file.display()
        );
    }
    out
}

/// Publish a baseline: the text goes to a unique sibling file first, which is then hard-linked to `dest`. The link
/// fails when `dest` exists, so a file another run published meanwhile is never replaced, and a reader never sees a
/// partial file. Says what happened.
fn publish(dest: &Path, text: &str) -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let tmp = dest.with_file_name(format!(
        "{}.{}.{}.tmp",
        dest.file_name().expect("file name").to_string_lossy(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&tmp, text).expect("hash file");
    let linked = std::fs::hard_link(&tmp, dest);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(()) => format!("wrote {}", dest.display()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            format!("baseline not written: {} already exists", dest.display())
        }
        Err(e) => panic!("cannot publish {}: {e}", dest.display()),
    }
}

/// One corpus run over `dir`; the settings are arguments so a test touches no process-wide state. See the module doc
/// for the two hash modes.
fn run_in(
    name: &str,
    dir: &Path,
    every: usize,
    hashes: Option<&HashMode>,
    wanted: fn(&[[u8; 4]]) -> bool,
    check: impl Fn(&Block, &mut Sink) + Sync,
) -> Summary {
    let file = hashes.map(|h| h.dir.join(format!("{name}.txt")));
    let saved = file.as_ref().filter(|f| f.exists());
    if let (Some(h), None) = (hashes, saved) {
        assert!(
            !h.commit.trim().is_empty(),
            "a baseline run writes {}: set SDC_BCENC_CORPUS_COMMIT=<commit text> (its first line names the commit)",
            file.as_ref().expect("file with hashes").display()
        );
    }
    let mut expected = saved.map(|f| read_saved(f, dir, every));
    let vs_rgbcx = expected.is_none();
    // The paths line, and any `SDC_BCENC_PATHS` panic, before the walk.
    let names: Vec<&str> = paths::paths().iter().map(|p| p.name()).collect();
    let started = Instant::now();
    let mut all = Vec::new();
    walk(dir, &mut all);
    all.sort();
    assert!(!all.is_empty(), "no PNG found under {}", dir.display());
    let files: Vec<PathBuf> = all.into_iter().step_by(every).collect();
    let outcomes: Vec<Outcome> = files
        .par_iter()
        .map(|p| compare_image(p, wanted, vs_rgbcx, &check))
        .collect();
    let (mut compared, mut skipped, mut unreadable) = (0u64, 0u64, 0u64);
    let (mut blocks, mut mismatches) = (0u64, 0u64);
    let mut lines = String::new();
    for (f, o) in files.iter().zip(&outcomes) {
        match o {
            Outcome::Unreadable(p, why) => {
                unreadable += 1;
                println!("unreadable: {}: {why}", p.display());
            }
            Outcome::Skipped => skipped += 1,
            Outcome::Done {
                blocks: b,
                mismatches: m,
                hash,
            } => {
                compared += 1;
                blocks += b;
                mismatches += m;
                let rel = f.strip_prefix(dir).unwrap_or(f).display().to_string();
                let digest = format!("{hash:016x}");
                let _ = writeln!(lines, "{rel} {b} {digest}");
                if let Some(map) = expected.as_mut() {
                    match map.remove(&rel) {
                        None => {
                            mismatches += 1;
                            println!(
                                "MISMATCH {rel}: compared, but the saved file has no line for it"
                            );
                        }
                        Some((eb, ed)) if eb != *b || ed != digest => {
                            mismatches += 1;
                            println!(
                                "MISMATCH {rel}: saved {eb} blocks digest {ed}, now {b} blocks digest {digest}"
                            );
                        }
                        Some(_) => {}
                    }
                }
            }
        }
    }
    for rel in expected.iter().flat_map(BTreeMap::keys) {
        mismatches += 1;
        println!("MISMATCH {rel}: listed in the saved file, but no image was compared");
    }
    let how = match saved {
        Some(f) => format!(
            "paths {}, checked against {}",
            names.join(", "),
            f.display()
        ),
        None => format!("paths {}", names.join(", ")),
    };
    let clean = mismatches == 0 && unreadable < files.len() as u64;
    let mut note = String::new();
    if let (None, Some(file), Some(h), true) = (saved, &file, hashes, clean) {
        std::fs::create_dir_all(&h.dir).expect("hash dir");
        let head = format!(
            "# commit {} input {} every {every}\n",
            h.commit.trim(),
            dir.display()
        );
        let said = publish(file, &(head + &lines));
        println!("{said}");
        note = format!(", {said}");
    }
    let line = format!(
        "{name}: {how}, files compared {compared}, skipped {skipped}, unreadable {unreadable}, blocks compared {blocks}, mismatches {mismatches}, {:.1} s{note}",
        started.elapsed().as_secs_f64()
    );
    Summary {
        files: files.len(),
        unreadable,
        mismatches,
        line,
    }
}

fn run(name: &str, wanted: fn(&[[u8; 4]]) -> bool, check: impl Fn(&Block, &mut Sink) + Sync) {
    let Some(dir) = std::env::var_os("SDC_BCENC_CORPUS") else {
        eprintln!("skipped: SDC_BCENC_CORPUS is not set");
        return;
    };
    let Some(hash_dir) = std::env::var_os("SDC_BCENC_CORPUS_HASHES") else {
        panic!(
            "a corpus run keeps its per-file hashes (CODING_STANDARDS.md, the long-run results rule): set SDC_BCENC_CORPUS_HASHES=<folder> (a baseline file is written there, a saved one is checked)"
        );
    };
    let hashes = HashMode {
        dir: PathBuf::from(hash_dir),
        commit: std::env::var("SDC_BCENC_CORPUS_COMMIT").unwrap_or_default(),
    };
    let s = run_in(name, Path::new(&dir), every(), Some(&hashes), wanted, check);
    println!("{}", s.line);
    assert!(
        s.unreadable < s.files as u64,
        "every one of the {} files was unreadable",
        s.files
    );
    assert_eq!(
        s.mismatches, 0,
        "{name}: {} mismatch(es) against the reference",
        s.mismatches
    );
}

fn any_transparent(px: &[[u8; 4]]) -> bool {
    px.iter().any(|t| t[3] < 128)
}

fn every_image(_: &[[u8; 4]]) -> bool {
    true
}

fn mask_of(px: &Block) -> u16 {
    px.iter()
        .enumerate()
        .fold(0u16, |m, (i, t)| m | (u16::from(t[3] < 128) << i))
}

fn bc4_check(px: &Block, s: &mut Sink) {
    for channel in [0, 3] {
        cmp(
            s,
            || format!("channel {channel}, radius {RADIUS}"),
            |p| bcenc::bc4_encode_block_on(p, px, channel, RADIUS),
            || encoders_sys::bc4_encode_block(px, channel, RADIUS),
        );
    }
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs) and SDC_BCENC_CORPUS_HASHES; SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc1_equals_rgbcx() {
    run("bc1", every_image, |px, d| {
        for perceptual in [false, true] {
            cmp(
                d,
                || format!("allow_3color true, perceptual {perceptual}"),
                |p| bcenc::bc1_encode_block_on(p, px, true, perceptual, Bc1Search::Default),
                || encoders_sys::bc1_encode_block(px, true, false, perceptual),
            );
        }
    });
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs) and SDC_BCENC_CORPUS_HASHES; SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc1_alpha_equals_rgbcx() {
    run("bc1_alpha", any_transparent, |px, d| {
        let mask = mask_of(px);
        for perceptual in [false, true] {
            cmp(
                d,
                || format!("mask {mask:#06x}, perceptual {perceptual}"),
                |p| bcenc::bc1_encode_block_alpha_on(p, px, mask, perceptual, Bc1Search::Default),
                || encoders_sys::bc1_encode_block_alpha(px, mask, perceptual),
            );
        }
    });
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs) and SDC_BCENC_CORPUS_HASHES; SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc3_equals_rgbcx() {
    run("bc3", every_image, |px, d| {
        for perceptual in [false, true] {
            cmp(
                d,
                || format!("radius {RADIUS}, cut {CUT}, perceptual {perceptual}"),
                |p| {
                    bcenc::bc3_encode_block_raw_cut_on(
                        p,
                        px,
                        RADIUS,
                        CUT,
                        perceptual,
                        Bc1Search::Default,
                    )
                },
                || encoders_sys::bc3_encode_block(px, RADIUS, CUT, perceptual),
            );
        }
    });
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs) and SDC_BCENC_CORPUS_HASHES; SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc1_equals_rgbcx_fast() {
    run("bc1_fast", every_image, |px, d| {
        for perceptual in [false, true] {
            cmp(
                d,
                || format!("fast, allow_3color true, perceptual {perceptual}"),
                |p| bcenc::bc1_encode_block_on(p, px, true, perceptual, Bc1Search::Fast),
                || encoders_sys::bc1_encode_block_level(px, BC1_FAST_LEVEL, true, perceptual),
            );
        }
    });
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs) and SDC_BCENC_CORPUS_HASHES; SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc1_alpha_equals_rgbcx_fast() {
    run("bc1_alpha_fast", any_transparent, |px, d| {
        let mask = mask_of(px);
        for perceptual in [false, true] {
            cmp(
                d,
                || format!("fast, mask {mask:#06x}, perceptual {perceptual}"),
                |p| bcenc::bc1_encode_block_alpha_on(p, px, mask, perceptual, Bc1Search::Fast),
                || encoders_sys::bc1_encode_block_alpha_level(px, mask, BC1_FAST_LEVEL, perceptual),
            );
        }
    });
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs) and SDC_BCENC_CORPUS_HASHES; SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc3_equals_rgbcx_fast() {
    run("bc3_fast", every_image, |px, d| {
        for perceptual in [false, true] {
            cmp(
                d,
                || format!("fast, radius {RADIUS}, cut {CUT}, perceptual {perceptual}"),
                |p| {
                    bcenc::bc3_encode_block_raw_cut_on(
                        p,
                        px,
                        RADIUS,
                        CUT,
                        perceptual,
                        Bc1Search::Fast,
                    )
                },
                || {
                    encoders_sys::bc3_encode_block_level(
                        px,
                        RADIUS,
                        CUT,
                        BC1_FAST_LEVEL,
                        perceptual,
                    )
                },
            );
        }
    });
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs) and SDC_BCENC_CORPUS_HASHES; SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc4_equals_rgbcx() {
    run("bc4", every_image, bc4_check);
}

#[test]
#[ignore = "needs SDC_BCENC_CORPUS (a folder of PNGs) and SDC_BCENC_CORPUS_HASHES; SDC_BCENC_CORPUS_EVERY=n takes every n-th file"]
fn corpus_bc5_equals_rgbcx() {
    run("bc5", every_image, |px, d| {
        cmp(
            d,
            || format!("radius {RADIUS}"),
            |p| bcenc::bc5_encode_block_on(p, px, RADIUS),
            || encoders_sys::bc5_encode_block(px, RADIUS),
        );
    });
}

/// A scratch folder that removes itself when dropped, also while an expected panic unwinds.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = PathBuf;
    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A fresh empty folder under the temp dir for one test.
fn scratch(tag: &str) -> Scratch {
    let d = std::env::temp_dir().join(format!("bcenc-corpus-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch folder");
    Scratch(d)
}

/// A fresh hash folder and the `HashMode` over it (commit text `test`); keep the `Scratch` alive for the test.
fn mode(tag: &str) -> (Scratch, HashMode) {
    mode_with(tag, "test")
}

fn mode_with(tag: &str, commit: &str) -> (Scratch, HashMode) {
    let dir = scratch(tag);
    let hashes = HashMode {
        dir: dir.to_path_buf(),
        commit: commit.to_string(),
    };
    (dir, hashes)
}

/// A deterministic noisy RGBA image.
fn noisy(w: u32, h: u32, seed: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(w, h, |x, y| {
        let mut s = (x + 1).wrapping_mul(2_654_435_761) ^ (y + 7).wrapping_mul(40_503) ^ seed;
        let mut next = || {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (s >> 24) as u8
        };
        image::Rgba([next(), next(), next(), next()])
    })
}

/// Two images, one in a sub-folder whose name has a space; returns the image folder.
fn two_images(tag: &str) -> Scratch {
    let dir = scratch(tag);
    std::fs::create_dir_all(dir.join("sub dir")).expect("sub folder");
    noisy(8, 8, 1).save(dir.join("a.png")).expect("png a");
    noisy(4, 8, 2)
        .save(dir.join("sub dir").join("b.png"))
        .expect("png b");
    dir
}

fn bc4_run(dir: &Path, hashes: &HashMode) -> Summary {
    run_in("bc4", dir, 1, Some(hashes), every_image, bc4_check)
}

/// The baseline's lines, replaced by `edit`, written back with LF.
fn edit_baseline(file: &Path, edit: impl FnOnce(&mut Vec<String>)) {
    let text = std::fs::read_to_string(file).expect("baseline file");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    edit(&mut lines);
    std::fs::write(file, lines.join("\n") + "\n").expect("write");
}

#[test]
fn a_baseline_is_written_then_checked() {
    let dir = two_images("baseline");
    let (_hd, hashes) = mode("baseline-hashes");
    let base = bc4_run(&dir, &hashes);
    assert_eq!(base.mismatches, 0, "{}", base.line);
    let file = hashes.dir.join("bc4.txt");
    let text = std::fs::read_to_string(&file).expect("baseline file");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[0],
        format!("# commit test input {} every 1", dir.display())
    );
    assert_eq!(lines.len(), 3, "header and 2 entries: {text}");
    assert!(lines[1].starts_with("a.png "), "{text}");
    assert!(lines[2].starts_with("sub dir"), "{text}");

    let checked = bc4_run(&dir, &hashes);
    assert_eq!(checked.mismatches, 0, "{}", checked.line);
    assert!(checked.line.contains("checked against"), "{}", checked.line);
    assert_eq!(
        std::fs::read_to_string(&file).expect("file"),
        text,
        "a check never rewrites the file"
    );

    // One hex digit of one digest changed.
    let mut bad = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>();
    let last = bad[2].pop().expect("digest digit");
    bad[2].push(if last == '0' { '1' } else { '0' });
    std::fs::write(&file, bad.join("\r\n") + "\r\n").expect("write");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 1);

    // One block count changed.
    let mut parts: Vec<String> = lines[2].rsplitn(3, ' ').map(str::to_string).collect();
    parts[1] = (parts[1].parse::<u64>().expect("blocks") + 1).to_string();
    let mut bad = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>();
    bad[2] = format!("{} {} {}", parts[2], parts[1], parts[0]);
    std::fs::write(&file, bad.join("\n") + "\n").expect("write");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 1);
}

#[test]
fn a_listed_image_that_is_missing_is_a_mismatch() {
    let dir = two_images("missing");
    let (_hd, hashes) = mode("missing-hashes");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 0);
    let file = hashes.dir.join("bc4.txt");
    let mut text = std::fs::read_to_string(&file).expect("baseline file");
    text.push_str("gone.png 4 0123456789abcdef\n");
    std::fs::write(&file, text).expect("write");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 1);
}

#[test]
fn an_image_without_a_saved_line_is_a_mismatch() {
    let dir = two_images("unlisted");
    let (_hd, hashes) = mode("unlisted-hashes");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 0);
    let file = hashes.dir.join("bc4.txt");
    edit_baseline(&file, |lines| {
        assert_eq!(lines.len(), 3, "header and 2 entries");
        lines.remove(2);
    });
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 1);
}

#[test]
fn a_baseline_with_an_rgbcx_disagreement_writes_nothing() {
    let dir = two_images("disagree");
    let (_hd, hashes) = mode("disagree-hashes");
    // The rgbcx side differs from every path's output, so each of the 4 + 2 blocks is a mismatch.
    let s = run_in("bc4", &dir, 1, Some(&hashes), every_image, |_, d| {
        cmp(d, || "disagree".to_string(), |_| [1u8; 4], || [2u8; 4]);
    });
    assert_eq!(s.mismatches, 6, "{}", s.line);
    assert!(
        !hashes.dir.join("bc4.txt").exists(),
        "a baseline with a mismatch must not be saved"
    );
}

#[test]
fn a_baseline_of_only_unreadable_inputs_writes_nothing() {
    let dir = scratch("unreadable");
    std::fs::write(dir.join("bad.png"), b"this is not a png").expect("bad png");
    let (_hd, hashes) = mode("unreadable-hashes");
    let s = bc4_run(&dir, &hashes);
    assert_eq!(
        (s.files, s.unreadable, s.mismatches),
        (1, 1, 0),
        "{}",
        s.line
    );
    assert!(
        !hashes.dir.join("bc4.txt").exists(),
        "a baseline with no readable file must not be saved"
    );
}

/// What a competing baseline holds in the tests below.
const COMPETING: &str = "# commit other input elsewhere every 1\nsaved.png 4 0123456789abcdef\n";

#[test]
fn a_baseline_never_replaces_a_file_that_appeared_during_the_run() {
    let dir = two_images("race");
    let (_hd, hashes) = mode("race-hashes");
    let file = hashes.dir.join("flat.txt");
    let once = std::sync::Once::new();
    // Another run publishes its baseline after this one found none and before this one publishes.
    let s = run_in("flat", &dir, 1, Some(&hashes), every_image, |px, d| {
        once.call_once(|| std::fs::write(&file, COMPETING).expect("competing baseline"));
        flat_check(px, d);
    });
    assert_eq!(s.mismatches, 0, "{}", s.line);
    assert_eq!(
        std::fs::read_to_string(&file).expect("file"),
        COMPETING,
        "the competing baseline must stay byte for byte"
    );
    assert!(
        s.line.contains("baseline not written") && s.line.contains("already exists"),
        "the run must say so: {}",
        s.line
    );
    let left: Vec<_> = std::fs::read_dir(&*hashes.dir)
        .expect("hash dir")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(left.len(), 1, "no temp file may stay: {left:?}");
}

#[test]
fn the_digest_is_the_saved_format() {
    let dir = scratch("digest");
    let img = noisy(8, 4, 3);
    img.save(dir.join("one.png")).expect("png");
    let (_hd, hashes) = mode("digest-hashes");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 0);
    let text = std::fs::read_to_string(hashes.dir.join("bc4.txt")).expect("file");
    let written = text
        .lines()
        .nth(1)
        .and_then(|l| l.rsplit(' ').next())
        .expect("digest")
        .to_string();

    let px: Vec<[u8; 4]> = img.into_raw().as_chunks::<4>().0.to_vec();
    let cut = encoders_sys::gather_blocks(&px, 8, 4);
    let blocks = cut.as_chunks::<16>().0;
    assert_eq!(blocks.len(), 2);
    let out = |b: usize, c: usize| bcenc::bc4_encode_block_on(bcenc::Path::Base, &blocks[b], c, 5);
    let (a, b, c, d) = (out(0, 0), out(0, 3), out(1, 0), out(1, 3));
    for (x, y) in [(a, b), (a, c), (a, d), (b, c), (b, d), (c, d)] {
        assert_ne!(x, y, "the fixture must give four distinct outputs");
    }
    let digest = |order: [[u8; 8]; 4]| {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for bytes in order {
            for byte in bytes {
                h ^= u64::from(byte);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        format!("{h:016x}")
    };
    assert_eq!(written, digest([a, b, c, d]));
    assert_ne!(
        digest([a, b, c, d]),
        digest([a, c, b, d]),
        "block outermost, channel innermost: the swapped order must differ"
    );
}

/// Every path but the first returns something else, so every block differs between paths.
fn split_check(px: &Block, s: &mut Sink) {
    let _ = px;
    cmp(
        s,
        || "split".to_string(),
        |p| {
            if p == paths::paths()[0] {
                [1u8; 4]
            } else {
                [2u8; 4]
            }
        },
        || panic!("the rgbcx encode ran in a check run"),
    );
}

/// All paths agree, so a baseline of it has no mismatch (its rgbcx side is the same constant).
fn flat_check(px: &Block, s: &mut Sink) {
    let _ = px;
    cmp(s, || "flat".to_string(), |_| [1u8; 4], || [1u8; 4]);
}

#[test]
fn a_check_run_never_encodes_with_rgbcx_and_flags_a_path_that_differs() {
    let dir = two_images("lazy");
    let (_hd, hashes) = mode("lazy-hashes");
    let base = run_in("flat", &dir, 1, Some(&hashes), every_image, flat_check);
    assert_eq!(base.mismatches, 0, "{}", base.line);
    // With one path there is nothing to differ and `split_check`'s rgbcx side must still not run; with several,
    // 4 + 2 blocks each differ between paths (the digests, of the first path only, still match).
    let want = if paths::paths().len() < 2 { 0 } else { 6 };
    let checked = run_in("flat", &dir, 1, Some(&hashes), every_image, split_check);
    assert_eq!(checked.mismatches, want, "{}", checked.line);
}

#[test]
#[should_panic(expected = "every 1; this run is input")]
fn a_check_run_with_another_every_panics() {
    let dir = two_images("every");
    let (_hd, hashes) = mode("every-hashes");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 0);
    run_in("bc4", &dir, 2, Some(&hashes), every_image, bc4_check);
}

#[test]
#[should_panic(expected = "was saved for input")]
fn a_check_run_with_another_input_panics() {
    let dir = two_images("input");
    let other = two_images("input-other");
    let (_hd, hashes) = mode("input-hashes");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 0);
    bc4_run(&other, &hashes);
}

#[test]
#[should_panic(expected = "is listed twice")]
fn a_saved_list_with_a_path_twice_panics() {
    let dir = two_images("twice");
    let (_hd, hashes) = mode("twice-hashes");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 0);
    let file = hashes.dir.join("bc4.txt");
    let mut text = std::fs::read_to_string(&file).expect("file");
    let again = text.lines().nth(1).expect("entry").to_string();
    text.push_str(&again);
    text.push('\n');
    std::fs::write(&file, text).expect("write");
    bc4_run(&dir, &hashes);
}

#[test]
#[should_panic(expected = "is not a header")]
fn a_header_without_the_commit_prefix_is_rejected() {
    let dir = two_images("junk");
    let (_hd, hashes) = mode("junk-hashes");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 0);
    edit_baseline(&hashes.dir.join("bc4.txt"), |lines| {
        lines[0] = format!("junk input {} every 1", dir.display());
    });
    bc4_run(&dir, &hashes);
}

#[test]
#[should_panic(expected = "is not a header")]
fn a_header_with_an_empty_commit_is_rejected() {
    let dir = two_images("nocommit");
    let (_hd, hashes) = mode("nocommit-hashes");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 0);
    edit_baseline(&hashes.dir.join("bc4.txt"), |lines| {
        lines[0] = format!("# commit  input {} every 1", dir.display());
    });
    bc4_run(&dir, &hashes);
}

#[test]
fn a_commit_label_with_input_in_it_reads_back() {
    let dir = two_images("label");
    let (_hd, hashes) = mode_with("label-hashes", "before input change");
    assert_eq!(bc4_run(&dir, &hashes).mismatches, 0);
    let head = std::fs::read_to_string(hashes.dir.join("bc4.txt")).expect("file");
    assert_eq!(
        head.lines().next(),
        Some(
            format!(
                "# commit before input change input {} every 1",
                dir.display()
            )
            .as_str()
        )
    );
    let checked = bc4_run(&dir, &hashes);
    assert_eq!(checked.mismatches, 0, "{}", checked.line);
}
