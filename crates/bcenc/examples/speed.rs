//! Speed of `bcenc` against the vendored rgbcx (C++), per format, on one thread.
//!
//! Run (release build, quiet machine; timings on a busy machine are noise, and the `rust/cpp` ratio is the figure
//! to compare between runs, because both encoders run in alternating passes under the same load):
//!
//! ```text
//! cargo run --offline --release -p bcenc --example speed -- [--path=<base|avx2|best>] [--rows=<name>,...] [--no-ref] <runs> [<png>...]
//! ```
//!
//! `--path` picks the code path `bcenc` runs on, through its hidden `_on` entries (default `best`, the one the public
//! functions take on this CPU); a path this machine cannot run, or an unknown name, exits with status 2 and says why.
//! The path goes to stderr.
//!
//! `--rows=` runs and prints only the named rows of the nine below (`bc1 bc1p bc1-4 bc1a bc3 bc4 bc5 bc1-fast
//! bc3-fast`); they always print in that order, and an unknown or empty name exits with status 2. `--no-ref` does not
//! run rgbcx at all: each line is `format  -  rust_med_us  -  <hash>`, the hash being the FNV-1a of the first pass's
//! `bcenc` output (16 hex digits), and `reference off` goes to stderr. Options come before `<runs>`, each at most
//! once, in any order. With `--no-ref` there is no `rust/cpp` ratio, so a before and after comparison of two builds
//! needs interleaved runs on a quiet machine; equal hashes then mean, barring a 64-bit collision, equal output bytes over all blocks.
//!
//! Every 4x4 block of level 0 of the PNGs (`image::open(..).to_rgba8()`, cut by `encoders_sys::gather_blocks` as
//! `tests/corpus.rs` does) is encoded `runs` times by each encoder in alternating passes (by `bcenc` alone with
//! `--no-ref`); rgbcx goes first in even runs and `bcenc` in odd ones, so neither always meets the colder caches. With
//! no PNG the blocks are `gen::colour_classes(1, 200)`, so the example runs without owner data.
//!
//! Without `--no-ref`, one line per row run on stdout: `format  cpp_med_us  rust_med_us  rust/cpp  same_bytes`.
//! The two times are the median (of an even count, the upper middle one) over the runs of one whole pass over all
//! blocks, in microseconds. `same_bytes` compares a 64-bit FNV-1a hash over every output block of the first pass of each encoder (`yes` or `NO`). The process exits with status 1
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

/// Time one format and print its line; true when both encoders produced the same bytes (always true without the
/// reference, which then does not run).
fn measure<const N: usize>(
    name: &str,
    blocks: &[Block],
    runs: usize,
    reference: bool,
    cpp: impl Fn(&Block) -> [u8; N],
    rust: impl Fn(&Block) -> [u8; N],
) -> bool {
    let (mut cpp_t, mut rust_t) = (Vec::new(), Vec::new());
    let mut rust_out = Vec::with_capacity(blocks.len() * N);
    let (mut cpp_hash, mut rust_hash) = (0, 0);
    if !reference {
        for run in 0..runs {
            rust_t.push(pass(blocks, &rust, &mut rust_out));
            if run == 0 {
                rust_hash = fnv1a(&rust_out);
            }
        }
        let r = median(&mut rust_t);
        println!(
            "{name:<8} {:>12} {r:>12.0} {:>8}  {rust_hash:016x}",
            "-", "-"
        );
        return true;
    }
    let mut cpp_out = Vec::with_capacity(blocks.len() * N);
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

/// The rows, in the order they run and print. One list gives `ROWS` and `run_rows`, so the names that `--rows=`
/// accepts, the rows that run and the labels they print are the same strings.
macro_rules! rows {
    ($path:ident; $($name:literal => $cpp:expr, $rust:expr;)*) => {
        const ROWS: [&str; [$($name),*].len()] = [$($name),*];

        /// Run the wanted rows (one flag per row of `ROWS`); true when no row printed `NO`.
        fn run_rows(
            wanted: &[bool; ROWS.len()],
            blocks: &[Block],
            runs: usize,
            reference: bool,
            $path: Path,
        ) -> bool {
            let mut ok = true;
            $(
                if wanted[ROWS.iter().position(|r| *r == $name).unwrap()] {
                    ok &= measure($name, blocks, runs, reference, $cpp, $rust);
                }
            )*
            ok
        }
    };
}

rows! {
    path;
    "bc1" => |b| encoders_sys::bc1_encode_block(b, true, false, false),
        |b| bcenc::bc1_encode_block_on(path, b, true, false, Bc1Search::Default);
    "bc1p" => |b| encoders_sys::bc1_encode_block(b, true, false, true),
        |b| bcenc::bc1_encode_block_on(path, b, true, true, Bc1Search::Default);
    "bc1-4" => |b| encoders_sys::bc1_encode_block(b, false, false, false),
        |b| bcenc::bc1_encode_block_on(path, b, false, false, Bc1Search::Default);
    "bc1a" => |b| encoders_sys::bc1_encode_block_alpha(b, mask(b), false),
        |b| bcenc::bc1_encode_block_alpha_on(path, b, mask(b), false, Bc1Search::Default);
    "bc3" => |b| encoders_sys::bc3_encode_block(b, RADIUS, CUT, false),
        |b| bcenc::bc3_encode_block_raw_cut_on(path, b, RADIUS, CUT, false, Bc1Search::Default);
    "bc4" => |b| encoders_sys::bc4_encode_block(b, 0, RADIUS),
        |b| bcenc::bc4_encode_block_on(path, b, 0, RADIUS);
    "bc5" => |b| encoders_sys::bc5_encode_block(b, RADIUS),
        |b| bcenc::bc5_encode_block_on(path, b, RADIUS);
    "bc1-fast" => |b| encoders_sys::bc1_encode_block_level(b, BC1_FAST_LEVEL, true, false),
        |b| bcenc::bc1_encode_block_on(path, b, true, false, Bc1Search::Fast);
    "bc3-fast" => |b| encoders_sys::bc3_encode_block_level(b, RADIUS, CUT, BC1_FAST_LEVEL, false),
        |b| bcenc::bc3_encode_block_raw_cut_on(path, b, RADIUS, CUT, false, Bc1Search::Fast);
}

const USAGE: &str = "usage: speed [--path=<base|avx2|best>] [--rows=<name>,...] [--no-ref] <runs (1 or more)> [<png>...]";

/// `--rows=` value to a flag per row of `ROWS`; an unknown or empty name is an error.
fn named_rows(list: &str) -> Result<[bool; ROWS.len()], String> {
    let mut wanted = [false; ROWS.len()];
    for name in list.split(',') {
        match ROWS.iter().position(|r| *r == name) {
            Some(i) => wanted[i] = true,
            None => {
                return Err(format!(
                    "unknown row {name}; the rows are {}",
                    ROWS.join(", ")
                ))
            }
        }
    }
    Ok(wanted)
}

/// What the command line asks for.
struct Config {
    path: Path,
    rows: [bool; ROWS.len()],
    no_ref: bool,
    runs: usize,
    pngs: Vec<String>,
}

/// Parse the arguments (without the program name); the error is the text for stderr, and the exit status is 2.
fn parse(args: &[String]) -> Result<Config, String> {
    let (mut path, mut rows, mut no_ref) = (None, None, false);
    let mut rest = args;
    while let Some((a, tail)) = rest.split_first().filter(|(a, _)| a.starts_with("--")) {
        rest = tail;
        if let Some(name) = a.strip_prefix("--path=") {
            if path.is_some() {
                return Err(USAGE.into());
            }
            path = Some(named_path(name)?);
        } else if let Some(list) = a.strip_prefix("--rows=") {
            if rows.is_some() {
                return Err(USAGE.into());
            }
            rows = Some(named_rows(list)?);
        } else if a == "--no-ref" && !no_ref {
            no_ref = true;
        } else {
            return Err(USAGE.into());
        }
    }
    let runs = rest
        .first()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .ok_or(USAGE)?;
    let pngs = &rest[1..];
    if pngs.iter().any(|p| p.starts_with("--")) {
        return Err(USAGE.into());
    }
    Ok(Config {
        path: path.unwrap_or_else(Path::best),
        rows: rows.unwrap_or([true; ROWS.len()]),
        no_ref,
        runs,
        pngs: pngs.to_vec(),
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = match parse(&args) {
        Ok(c) => c,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::from(2);
        }
    };
    let (path, runs) = (cfg.path, cfg.runs);
    eprintln!("path {}", path.name());
    if cfg.no_ref {
        eprintln!("reference off");
    }
    let blocks = load(&cfg.pngs);
    if blocks.is_empty() {
        eprintln!("no 4x4 block in the given PNGs");
        return ExitCode::from(2);
    }
    eprintln!("{} blocks, {runs} runs", blocks.len());
    if run_rows(&cfg.rows, &blocks, runs, !cfg.no_ref, path) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    fn names(flags: &[bool; ROWS.len()]) -> Vec<&'static str> {
        ROWS.iter()
            .zip(flags)
            .filter(|(_, &w)| w)
            .map(|(n, _)| *n)
            .collect()
    }

    #[test]
    fn rows_keep_the_fixed_order_and_count_a_name_once() {
        let w = named_rows("bc5,bc4,bc5,bc1").unwrap();
        assert_eq!(names(&w), ["bc1", "bc4", "bc5"]);
    }

    #[test]
    fn an_unknown_or_empty_row_name_is_an_error_naming_the_rows() {
        let list = "the rows are bc1, bc1p, bc1-4, bc1a, bc3, bc4, bc5, bc1-fast, bc3-fast";
        assert_eq!(
            named_rows("bc9").err().unwrap(),
            format!("unknown row bc9; {list}")
        );
        for bad in ["", "bc4,,bc5"] {
            assert_eq!(
                named_rows(bad).err().unwrap(),
                format!("unknown row ; {list}"),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn an_option_after_the_run_count_or_a_repeated_one_is_a_usage_error() {
        for bad in [
            &["1", "--no-ref"][..],
            &["--no-ref", "1", "a.png", "--rows=bc4"],
            &["--no-ref", "--no-ref", "1"],
            &["--rows=bc4", "--rows=bc5", "1"],
            &["--path=base", "--path=base", "1"],
            &["--frob", "1"],
            &["--rows=bc4"],
            &["0"],
            &[],
        ] {
            assert_eq!(parse(&args(bad)).err().unwrap(), USAGE, "{bad:?}");
        }
    }

    #[test]
    fn options_in_any_order_before_the_run_count() {
        let c = parse(&args(&[
            "--no-ref",
            "--rows=bc5,bc4",
            "--path=base",
            "3",
            "a.png",
        ]))
        .unwrap();
        assert!(c.no_ref);
        assert_eq!((c.runs, c.pngs), (3, vec!["a.png".to_string()]));
        assert_eq!(c.path.name(), "base");
        assert_eq!(names(&c.rows), ["bc4", "bc5"]);
    }
}
