//! BC6H quality measurement of the vendored ISPC TC kernel on the bench's HDR images, and the comparison of two runs.
//!
//!   cargo run --offline --release -p encoders-sys --example bc6h_quality -- run --root <bench root> --out <csv>
//!       [--target sse2|avx2|avx512] [--threads N]
//!   cargo run --offline --release -p encoders-sys --example bc6h_quality -- compare <before.csv> <after.csv>
//!       [--pair <levelA>:<levelB>]
//!   cargo run --offline --release -p encoders-sys --example bc6h_quality -- diag --root <bench root> --out <txt>
//!       [--target sse2|avx2|avx512] [--threads N] [--gate today|d16] [--centre]
//!
//! `run` reads `<root>\images2\hdr_real` and `<root>\images3\hdr_real` (set `hdr_real`, 18 images) and
//! `<root>\images2\hdr_grad` (set `hdr_grad`, 4 images), `.f16` files sorted by path. Each channel goes through the
//! engine's `clamp_half` and back to half bits (alpha 1.0); the CSV column `clamped` counts channels that changed.
//! Every level is encoded one 4-row strip per call (`bc6h_compress_rows_on`) with the strips spread over `--threads`
//! threads (default: every available thread; later runs of a measurement chain pass the same N), decoded with
//! `bcdec_rs::bc6h_float`, and scored against the clamped input as f32: mPSNR, RMSLE and peak ratio.
//! The metrics are ported from branch `bench` 87c5539f (`crates/bench/src/metrics.rs`, `s7b.rs` `peak`,
//! `bc6peak.rs` `mean_ci`, `corpus.rs` `load_f16`).
//!
//! `compare` joins the rows of two CSVs on (set, image) and prints, per level pair (each level with itself, or
//! `--pair a:b` for level a of the first file against level b of the second), the paired mean deltas of the 18
//! `hdr_real` images with their 95 % Student-t half-widths, the count of byte-identical encodes, the total seconds
//! and the `hdr_grad` rows. RMSLE deltas are sign-flipped and shown x1000, so a positive delta is better everywhere.
//!
//! `diag` encodes the same images at every level and counts, from the source and the decoded halves, what the audit
//! of the vendored kernel asked about (it changes no kernel byte): the decode bias d = decoded - source half per
//! channel (Q5), regions whose decoded texels collapse to one colour (Q2), the picks of the Fast and Basic two-region
//! gate under today's and spec D16's rule and how often Slow's output is in the family D16 picks (Q4; the gate is
//! replicated in f32 and checked against the encoded modes, and a mismatch away from a threshold is an error unless it
//! is an `idx_ties` block: the output is the neighbour mode of the pick's family because the two widest spans are
//! within 1e-3 of each other and the kernel's fast-math rounding chose the other channel as `max_span_idx`),
//! the guarded (peak) blocks (Q6, Task 5), and the counts of astra #2, #5 and #6. `--gate` names the two-region gate
//! the replication treats as the kernel's; `--centre` the kernel's UF16 conversion `(h + 0.5) / 31 * 64` instead of
//! `(h / 31) * 64`. The Q4 "changed picks" table always compares today's gate with D16's on the source spans converted
//! as `(h / 31) * 64`, so it does not change with `--centre`; the self-check, the lift count and the astra #6 proxy
//! do. Image sides must be multiples of 4 (the block indexing assumes it; `diag` panics otherwise).
use encoders_sys::{bc6h_compress_rows_on, Bc6hLevel, IspcTarget};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

type Res<T> = Result<T, Box<dyn Error>>;

const LEVELS: [Bc6hLevel; 6] = [
    Bc6hLevel::Veryfast,
    Bc6hLevel::Fast,
    Bc6hLevel::Basic,
    Bc6hLevel::Slow,
    Bc6hLevel::Veryslow,
    Bc6hLevel::VeryslowPeak,
];

const HALF_MAX: f32 = 65504.0;

fn level_name(l: Bc6hLevel) -> &'static str {
    match l {
        Bc6hLevel::Veryfast => "veryfast",
        Bc6hLevel::Fast => "fast",
        Bc6hLevel::Basic => "basic",
        Bc6hLevel::Slow => "slow",
        Bc6hLevel::Veryslow => "veryslow",
        Bc6hLevel::VeryslowPeak => "veryslow_peak",
    }
}

/// The engine's `clamp_half` (`crates/engine/src/quantize.rs`): not `clamp`, which would keep -0.0 and NaN.
fn clamp_half(v: f32) -> f32 {
    if v > 0.0 {
        v.min(HALF_MAX)
    } else {
        0.0
    }
}

/// `--threads N` of `run`; unset, every available thread.
static THREADS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

fn threads() -> usize {
    *THREADS.get_or_init(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
}

struct HalfImage {
    width: u32,
    height: u32,
    px: Vec<[u16; 4]>,
}

/// The bench's `load_f16`: magic `F16\0`, u32 width and height, then RGBA half bits, little-endian.
fn load_f16(path: &Path) -> Res<HalfImage> {
    let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if b.len() < 12 {
        return Err(format!("{}: not an F16 file", path.display()).into());
    }
    let u = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
    if u(0) != 0x0036_3146 {
        return Err(format!("{}: not an F16 file", path.display()).into());
    }
    let (width, height) = (u(4), u(8));
    if width % 4 != 0 || height % 4 != 0 {
        return Err(format!(
            "{}: {width}x{height} is not a multiple of 4 in both directions",
            path.display()
        )
        .into());
    }
    let n = width as usize * height as usize;
    if b.len() != 12 + n * 8 {
        return Err(format!("{}: truncated", path.display()).into());
    }
    let px = (0..n)
        .map(|i| {
            let o = 12 + i * 8;
            [0, 1, 2, 3].map(|c| u16::from_le_bytes([b[o + c * 2], b[o + c * 2 + 1]]))
        })
        .collect();
    Ok(HalfImage { width, height, px })
}

fn f16_files(dir: &Path) -> Res<Vec<PathBuf>> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut v = Vec::new();
    for e in rd {
        let p = e?.path();
        if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("f16")) {
            v.push(p);
        }
    }
    v.sort();
    Ok(v)
}

/// A measured set: its name and the files, sorted by path.
fn sets(root: &Path) -> Res<Vec<(&'static str, Vec<PathBuf>)>> {
    let mut real = f16_files(&root.join("images2").join("hdr_real"))?;
    real.extend(f16_files(&root.join("images3").join("hdr_real"))?);
    real.sort();
    let grad = f16_files(&root.join("images2").join("hdr_grad"))?;
    Ok(vec![("hdr_real", real), ("hdr_grad", grad)])
}

// ---- metrics (bench 87c5539f, metrics.rs and s7b.rs) ----

/// `engine::validate::psnr_from_mse`.
fn psnr_from_mse(mse: f64) -> f64 {
    if mse <= 1e-12 {
        99.0
    } else {
        10.0 * (65025.0 / mse).log10()
    }
}

/// Mean over 13 exposures (2^-6..=2^6) of the MSE of the tone-mapped values, over every RGB value.
fn mpsnr(reference: &[[f32; 3]], decoded: &[[f32; 3]]) -> f64 {
    assert_eq!(reference.len(), decoded.len());
    let n = (reference.len() * 3) as f64;
    let exposures: Vec<i32> = (-6..=6).collect();
    let per = exposures.len().div_ceil(threads());
    let mut mses = vec![0f64; exposures.len()];
    std::thread::scope(|sc| {
        for (es, out) in exposures.chunks(per).zip(mses.chunks_mut(per)) {
            sc.spawn(move || {
                for (&c, o) in es.iter().zip(out) {
                    let s = 2f64.powi(c);
                    let t = |v: f32| {
                        (255.0 * ((v as f64).max(0.0) * s).powf(1.0 / 2.2)).clamp(0.0, 255.0)
                    };
                    let mut sum = 0.0;
                    for (r, d) in reference.iter().zip(decoded) {
                        for (&rv, &dv) in r.iter().zip(d) {
                            let e = t(rv) - t(dv);
                            sum += e * e;
                        }
                    }
                    *o = sum / n;
                }
            });
        }
    });
    psnr_from_mse(mses.iter().sum::<f64>() / mses.len() as f64)
}

/// `sqrt(mean((ln_1p(dec) - ln_1p(ref))^2))` over every RGB value.
fn rmsle(reference: &[[f32; 3]], decoded: &[[f32; 3]]) -> f64 {
    assert_eq!(reference.len(), decoded.len());
    let sum: f64 = reference
        .iter()
        .zip(decoded)
        .flat_map(|(r, d)| r.iter().zip(d))
        .map(|(&r, &d)| ((d as f64).ln_1p() - (r as f64).ln_1p()).powi(2))
        .sum();
    (sum / (reference.len() * 3) as f64).sqrt()
}

/// The largest `0.2126 R + 0.7152 G + 0.0722 B`.
fn peak(px: &[[f32; 3]]) -> f32 {
    px.iter()
        .map(|c| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2])
        .fold(0.0f32, f32::max)
}

// ---- run ----

#[derive(Debug)]
struct Row {
    set: String,
    image: String,
    level: String,
    target: String,
    clamped: u64,
    mpsnr: f64,
    rmsle: f64,
    peak_ratio: f64,
    seconds: f64,
    sha256: String,
}

/// Encodes one image at one level, one 4-row strip per call, the strips spread over the threads.
fn encode_image(img: &HalfImage, level: Bc6hLevel, target: IspcTarget) -> Vec<u8> {
    let w = img.width;
    let strip_blocks = (w / 4) as usize;
    let mut out = vec![0u8; strip_blocks * (img.height / 4) as usize * 16];
    let jobs: Vec<(&[[u16; 4]], &mut [u8])> = img
        .px
        .chunks((w * 4) as usize)
        .zip(out.chunks_mut(strip_blocks * 16))
        .collect();
    let per = jobs.len().div_ceil(threads()).max(1);
    let mut it = jobs.into_iter();
    std::thread::scope(|sc| loop {
        let batch: Vec<_> = it.by_ref().take(per).collect();
        if batch.is_empty() {
            break;
        }
        sc.spawn(move || {
            for (rows, o) in batch {
                bc6h_compress_rows_on(rows, w, 4, o, level, target);
            }
        });
    });
    out
}

fn decode_image(enc: &[u8], w: u32, h: u32) -> Vec<[f32; 3]> {
    let mut img = vec![[0f32; 3]; (w * h) as usize];
    let bw = (w / 4) as usize;
    for (i, b) in enc.chunks(16).enumerate() {
        let (bx, by) = (i % bw, i / bw);
        let mut o = [0f32; 48];
        bcdec_rs::bc6h_float(b, &mut o, 12, false);
        for j in 0..4 {
            for k in 0..4 {
                let p = (by * 4 + j) * w as usize + bx * 4 + k;
                let s = (j * 4 + k) * 3;
                img[p] = [o[s], o[s + 1], o[s + 2]];
            }
        }
    }
    img
}

/// Each channel through `clamp_half` and back to half bits, alpha 1.0; the count of channels that changed.
fn clamp_image(src: &HalfImage) -> (HalfImage, u64) {
    let mut clamped = 0u64;
    let px: Vec<[u16; 4]> = src
        .px
        .iter()
        .map(|p| {
            let mut o = [0x3C00u16; 4];
            for c in 0..3 {
                let bits =
                    half::f16::from_f32(clamp_half(half::f16::from_bits(p[c]).to_f32())).to_bits();
                clamped += u64::from(bits != p[c]);
                o[c] = bits;
            }
            o
        })
        .collect();
    let img = HalfImage {
        width: src.width,
        height: src.height,
        px,
    };
    (img, clamped)
}

fn run(args: &[String]) -> Res<()> {
    let mut root = None;
    let mut out = None;
    let mut target = IspcTarget::Avx2;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--root" => root = Some(PathBuf::from(val()?)),
            "--out" => out = Some(PathBuf::from(val()?)),
            "--threads" => {
                let n: usize = val()?
                    .parse()
                    .map_err(|e| format!("--threads needs a number of 1 or more: {e}"))?;
                if n == 0 || THREADS.set(n).is_err() {
                    return Err("--threads needs a number of 1 or more, given once".into());
                }
            }
            "--target" => {
                target = match val()?.as_str() {
                    "sse2" => IspcTarget::Sse2,
                    "avx2" => IspcTarget::Avx2,
                    "avx512" => IspcTarget::Avx512,
                    t => return Err(format!("unknown target {t}").into()),
                }
            }
            _ => return Err(format!("unknown argument {a}").into()),
        }
    }
    let root = root.ok_or("--root is required")?;
    let out = out.ok_or("--out is required")?;
    if !target.supported() {
        return Err(format!("this CPU cannot run {target:?}").into());
    }
    let target_name = format!("{target:?}").to_lowercase();
    let mut csv =
        String::from("set,image,level,target,clamped,mpsnr,rmsle,peak_ratio,seconds,sha256\n");
    for (set, files) in sets(&root)? {
        for path in files {
            let src = load_f16(&path)?;
            let image = path.file_stem().unwrap().to_string_lossy().into_owned();
            let (img, clamped) = clamp_image(&src);
            let reference: Vec<[f32; 3]> = img
                .px
                .iter()
                .map(|p| [0, 1, 2].map(|c| half::f16::from_bits(p[c]).to_f32()))
                .collect();
            let ref_peak = peak(&reference).max(1e-9);
            for level in LEVELS {
                let t0 = Instant::now();
                let enc = encode_image(&img, level, target);
                let seconds = t0.elapsed().as_secs_f64();
                let dec = decode_image(&enc, img.width, img.height);
                let r = Row {
                    set: set.into(),
                    image: image.clone(),
                    level: level_name(level).into(),
                    target: target_name.clone(),
                    clamped,
                    mpsnr: mpsnr(&reference, &dec),
                    rmsle: rmsle(&reference, &dec),
                    peak_ratio: (peak(&dec) / ref_peak) as f64,
                    seconds,
                    sha256: hex::encode(Sha256::digest(&enc)),
                };
                writeln!(
                    csv,
                    "{},{},{},{},{},{:.4},{:.6},{:.6},{:.3},{}",
                    r.set,
                    r.image,
                    r.level,
                    r.target,
                    r.clamped,
                    r.mpsnr,
                    r.rmsle,
                    r.peak_ratio,
                    r.seconds,
                    r.sha256
                )?;
                eprintln!(
                    "{set} {image} {} mpsnr {:.2} peak {:.3} {:.2}s",
                    r.level, r.mpsnr, r.peak_ratio, r.seconds
                );
            }
        }
    }
    std::fs::write(&out, csv)?;
    Ok(())
}

// ---- compare ----

fn t975(df: usize) -> f64 {
    const T: [f64; 30] = [
        12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160,
        2.145, 2.131, 2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056,
        2.052, 2.048, 2.045, 2.042,
    ];
    T.get(df.wrapping_sub(1)).copied().unwrap_or(1.960)
}

/// Mean of `d` and its 95 % Student-t half-width (bench `mean_ci`).
fn mean_ci(d: &[f64]) -> (f64, f64) {
    let m = d.iter().sum::<f64>() / d.len() as f64;
    let n = d.len();
    if n < 2 {
        return (m, f64::NAN);
    }
    let var = d.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64;
    (m, t975(n - 1) * (var / n as f64).sqrt())
}

fn read_csv(path: &str) -> Res<Vec<Row>> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    parse_csv(&text, path)
}

/// The rows of a CSV; a second row with the same (set, image, level) is an error.
fn parse_csv(text: &str, path: &str) -> Res<Vec<Row>> {
    let mut rows: Vec<Row> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (n, line) in text.lines().enumerate().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        if f.len() != 10 {
            return Err(format!("{path}:{}: expected 10 fields", n + 1).into());
        }
        if !seen.insert((f[0], f[1], f[2])) {
            return Err(
                format!("{path}:{}: duplicate row {} {} {}", n + 1, f[0], f[1], f[2]).into(),
            );
        }
        rows.push(Row {
            set: f[0].into(),
            image: f[1].into(),
            level: f[2].into(),
            target: f[3].into(),
            clamped: f[4].parse()?,
            mpsnr: f[5].parse()?,
            rmsle: f[6].parse()?,
            peak_ratio: f[7].parse()?,
            seconds: f[8].parse()?,
            sha256: f[9].into(),
        });
    }
    Ok(rows)
}

type Index<'a> = BTreeMap<(&'a str, &'a str), &'a Row>;

fn index<'a>(rows: &'a [Row], level: &str) -> Index<'a> {
    rows.iter()
        .filter(|r| r.level == level)
        .map(|r| ((r.set.as_str(), r.image.as_str()), r))
        .collect()
}

/// The paired rows of one set: (image, before, after), in (set, image) order. An image in only one file is an error.
fn joined<'a>(a: &Index<'a>, b: &Index<'a>, set: &str) -> Res<Vec<(&'a str, &'a Row, &'a Row)>> {
    let names = |i: &Index<'a>| -> Vec<&'a str> {
        i.keys()
            .filter(|(s, _)| *s == set)
            .map(|(_, n)| *n)
            .collect()
    };
    let (na, nb) = (names(a), names(b));
    let unpaired: Vec<&str> = na
        .iter()
        .filter(|n| !nb.contains(n))
        .chain(nb.iter().filter(|n| !na.contains(n)))
        .copied()
        .collect();
    if !unpaired.is_empty() {
        return Err(format!("{set}: images in only one file: {}", unpaired.join(", ")).into());
    }
    Ok(na
        .iter()
        .map(|n| (*n, a[&(set, *n)], b[&(set, *n)]))
        .collect())
}

fn known_level(l: &str) -> Res<()> {
    if LEVELS.iter().any(|k| level_name(*k) == l) {
        Ok(())
    } else {
        Err(format!("unknown level {l}").into())
    }
}

fn compare_pair(before: &[Row], after: &[Row], la: &str, lb: &str) -> Res<String> {
    known_level(la)?;
    known_level(lb)?;
    let (ia, ib) = (index(before, la), index(after, lb));
    let mut s = String::new();
    writeln!(s, "== {la} (before) vs {lb} (after)")?;
    let real = joined(&ia, &ib, "hdr_real")?;
    let grad = joined(&ia, &ib, "hdr_grad")?;
    // After minus before; RMSLE flipped and scaled, so + is better for all three.
    let deltas = |r: &(&str, &Row, &Row)| {
        [
            r.2.peak_ratio - r.1.peak_ratio,
            r.2.mpsnr - r.1.mpsnr,
            (r.1.rmsle - r.2.rmsle) * 1000.0,
        ]
    };
    writeln!(s, "hdr_real: n = {}", real.len())?;
    let per_image: Vec<[f64; 3]> = real.iter().map(deltas).collect();
    for (m, name) in ["peak ratio", "mPSNR dB", "RMSLE x1000"].iter().enumerate() {
        let d: Vec<f64> = per_image.iter().map(|x| x[m]).collect();
        if d.is_empty() {
            continue;
        }
        let (mean, hw) = mean_ci(&d);
        let wins = d.iter().filter(|&&x| x > 0.0).count();
        let losses = d.iter().filter(|&&x| x < 0.0).count();
        let (wi, wd) = d
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, v)| (real[i].0, *v))
            .unwrap();
        writeln!(
            s,
            "  {name:<12} mean delta {mean:+.3} half-width {hw:.3}  wins {wins} losses {losses} ties {}  worst {wi} {wd:+.3}",
            d.len() - wins - losses
        )?;
    }
    for (name, v) in [("hdr_real", &real), ("hdr_grad", &grad)] {
        let k = v.iter().filter(|r| r.1.sha256 == r.2.sha256).count();
        writeln!(s, "bytes identical {name}: {k}/{}", v.len())?;
    }
    let secs = |v: &[(&str, &Row, &Row)]| {
        (
            v.iter().map(|r| r.1.seconds).sum::<f64>(),
            v.iter().map(|r| r.2.seconds).sum::<f64>(),
        )
    };
    let (sr, sg) = (secs(&real), secs(&grad));
    writeln!(
        s,
        "seconds before {:.2} after {:.2}",
        sr.0 + sg.0,
        sr.1 + sg.1
    )?;
    for r in &grad {
        let d = deltas(r);
        writeln!(
            s,
            "hdr_grad {:<20} peak ratio {:+.3} mPSNR dB {:+.3} RMSLE x1000 {:+.3}",
            r.0, d[0], d[1], d[2]
        )?;
    }
    Ok(s)
}

fn compare(args: &[String]) -> Res<()> {
    let mut files = Vec::new();
    let mut pair = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--pair" => pair = Some(it.next().ok_or("--pair needs a:b")?.clone()),
            _ => files.push(a.clone()),
        }
    }
    let [fa, fb] = files.as_slice() else {
        return Err("compare needs <before.csv> <after.csv>".into());
    };
    let (before, after) = (read_csv(fa)?, read_csv(fb)?);
    let pairs: Vec<(String, String)> = match pair {
        Some(p) => {
            let (a, b) = p.split_once(':').ok_or("--pair needs a:b")?;
            vec![(a.into(), b.into())]
        }
        None => LEVELS
            .iter()
            .map(|l| (level_name(*l).into(), level_name(*l).into()))
            .collect(),
    };
    for (a, b) in pairs {
        print!("{}", compare_pair(&before, &after, &a, &b)?);
    }
    Ok(())
}

// ---- diag ----

/// kernel.ispc `get_mode_prefix`: the 5-bit (or 2-bit) mode field of kernel modes 0..13 (index 14 does not exist).
const MODE_PREFIX: [u32; 14] = [0, 1, 2, 6, 10, 14, 18, 22, 26, 30, 3, 7, 11, 15];

/// kernel.ispc `get_pattern`, entries 0-31 (the BC6H partitions): 2 bits per texel, region = (pattern >> 2k) & 3.
const PATTERNS: [u32; 32] = [
    0x50505050, 0x40404040, 0x54545454, 0x54505040, 0x50404000, 0x55545450, 0x55545040, 0x54504000,
    0x50400000, 0x55555450, 0x55544000, 0x54400000, 0x55555440, 0x55550000, 0x55555500, 0x55000000,
    0x55150100, 0x00004054, 0x15010000, 0x00405054, 0x00004050, 0x15050100, 0x05010000, 0x40505054,
    0x00404050, 0x05010100, 0x14141414, 0x05141450, 0x01155440, 0x00555500, 0x15014054, 0x05414150,
];

/// kernel.ispc `get_skips` (`skip_table`), entries 0-31: the high nibble is the partition's second anchor texel.
const SKIPS: [u8; 32] = [
    0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0, 0xf0,
    0xf0, 0x20, 0x80, 0x20, 0x20, 0x80, 0x80, 0xf0, 0x20, 0x80, 0x20, 0x20, 0x80, 0x80, 0x20, 0x20,
];

/// The two-region gate the replication treats as the kernel's (`--gate`).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Gate {
    /// `max_span * margin > span` rejects.
    Today,
    /// Spec D16: modes 2 and 6 pass when `max_span * margin <= 2 * span` and `second_span * margin <= span`.
    D16,
}

#[derive(Clone, Copy)]
struct DiagOpts {
    gate: Gate,
    /// The kernel's UF16 conversion is `(h + 0.5) / 31 * 64` (`--centre`); default `(h / 31) * 64`.
    centre: bool,
}

fn h2f(h: u16) -> f32 {
    half::f16::from_bits(h).to_f32()
}

fn lum(p: [u16; 3]) -> f32 {
    0.2126f32 * h2f(p[0]) + 0.7152f32 * h2f(p[1]) + 0.0722f32 * h2f(p[2])
}

fn halves_of(d: [f32; 3]) -> [u16; 3] {
    d.map(|v| half::f16::from_f32(v).to_bits())
}

/// `n` bits of the block starting at bit `lo`.
fn field(b: &[u8; 16], lo: u32, n: u32) -> u32 {
    ((u128::from_le_bytes(*b) >> lo) & ((1u128 << n) - 1)) as u32
}

/// The kernel mode (0..=13) of an encoded block, `None` for a reserved mode field.
fn kernel_mode(b: &[u8; 16]) -> Option<usize> {
    let f2 = field(b, 0, 2);
    if f2 < 2 {
        return Some(f2 as usize);
    }
    let f5 = field(b, 0, 5);
    MODE_PREFIX.iter().position(|&p| p == f5)
}

fn partition_of(b: &[u8; 16]) -> usize {
    field(b, 77, 5) as usize
}

fn second_anchor(part: usize) -> usize {
    (SKIPS[part] >> 4) as usize
}

fn region_of(part: usize, k: usize) -> usize {
    ((PATTERNS[part] >> (2 * k)) & 3) as usize
}

/// Bit position and width of texel `k`'s index. One-region modes: from bit 65, 4 bits per texel, texel 0 (the anchor)
/// 3 bits. Two-region modes: from bit 82, 3 bits per texel, texel 0 and the partition's second anchor 2 bits.
fn index_field(two_region: bool, anchor2: usize, k: usize) -> (u32, u32) {
    if two_region {
        let w = |t: usize| if t == 0 || t == anchor2 { 2u32 } else { 3 };
        (82 + (0..k).map(w).sum::<u32>(), w(k))
    } else {
        let w = |t: usize| if t == 0 { 3u32 } else { 4 };
        (65 + (0..k).map(w).sum::<u32>(), w(k))
    }
}

fn set_index(b: &mut [u8; 16], pos: u32, width: u32, v: u32) {
    let mask = ((1u128 << width) - 1) << pos;
    let x = (u128::from_le_bytes(*b) & !mask) | (u128::from(v) << pos);
    *b = x.to_le_bytes();
}

fn decode_block(b: &[u8; 16]) -> [[u16; 3]; 16] {
    let mut o = [0f32; 48];
    bcdec_rs::bc6h_float(b, &mut o, 12, false);
    std::array::from_fn(|t| halves_of([o[t * 3], o[t * 3 + 1], o[t * 3 + 2]]))
}

fn src_block(px: &[[u16; 4]], w: usize, bx: usize, by: usize) -> [[u16; 3]; 16] {
    std::array::from_fn(|t| {
        let p = px[(by * 4 + t / 4) * w + bx * 4 + t % 4];
        [p[0], p[1], p[2]]
    })
}

/// The kernel's UF16 value of half code `h`.
fn uf16(h: u16, centre: bool) -> f32 {
    if centre {
        (f32::from(h) + 0.5) / 31.0 * 64.0
    } else {
        (f32::from(h) / 31.0) * 64.0
    }
}

/// `(max_span, max_span_idx, second_span)` of the block's UF16 channel spans: the first maximum (strict `>` from 0,
/// as `bc6h_bounds`) and the second-largest of the three.
fn rank_spans(src: &[[u16; 3]; 16], centre: bool) -> (f32, usize, f32) {
    let mut spans = [0f32; 3];
    for (c, s) in spans.iter_mut().enumerate() {
        let (mut lo, mut hi) = (f32::MAX, 0f32);
        for t in src {
            let u = uf16(t[c], centre);
            lo = lo.min(u);
            hi = hi.max(u);
        }
        *s = hi - lo;
    }
    let (mut max, mut idx) = (0f32, 0usize);
    for (p, &s) in spans.iter().enumerate() {
        if s > max {
            idx = p;
            max = s;
        }
    }
    spans.sort_by(|a, b| b.total_cmp(a));
    (max, idx, spans[1])
}

/// kernel.ispc `get_span` for the two-region modes the Fast and Basic gate tests (f32 constants, as ISPC folds them).
fn mode_span(mode: usize) -> f32 {
    match mode {
        0 => 0.9f32 * 65535.0 / 64.0,
        1 => 0.9f32 * 65535.0 / 4.0,
        2 => 0.8f32 * 65535.0 / 256.0,
        5 => 0.9f32 * 65535.0 / 32.0,
        6 => 0.9f32 * 65535.0 / 16.0,
        9 => 65535.0,
        _ => unreachable!("no span for mode {mode}"),
    }
}

/// The `bc6h_test_mode(state, mode, false, margin)` calls of `CompressBlockBC6H_core` before `bc6h_enc_2p`: mode 1
/// only at Fast (`fast_mode`), margins 0, 1 and `1 / 1.2`.
fn gate_seq(fast: bool) -> impl Iterator<Item = (usize, f32)> {
    let inv = 1.0f32 / 1.2f32;
    [
        (9, 0.0f32),
        (1, 1.0),
        (6, inv),
        (5, inv),
        (0, inv),
        (2, 1.0),
    ]
    .into_iter()
    .filter(move |&(mode, _)| fast || mode != 1)
}

fn gate_passes(mode: usize, margin: f32, max: f32, second: f32, gate: Gate) -> bool {
    let span = mode_span(mode);
    if gate == Gate::D16 && (mode == 2 || mode == 6) {
        max * margin <= 2.0 * span && second * margin <= span
    } else {
        // The kernel rejects with `if (max_span * margin > span) return`; no span here is NaN.
        max * margin <= span
    }
}

/// The kernel mode the Fast (or Basic) gate sequence leaves in `state->mode`: the last passing test wins and modes
/// 2 and 6 become `mode + max_span_idx`. Mode 9 (margin 0) always passes.
fn gate_pick(max: f32, idx: usize, second: f32, fast: bool, gate: Gate) -> usize {
    let mut pick = 9;
    for (mode, margin) in gate_seq(fast) {
        if gate_passes(mode, margin, max, second, gate) {
            pick = if mode == 2 || mode == 6 {
                mode + idx
            } else {
                mode
            };
        }
    }
    pick
}

/// A span within 1e-3 (relative) of a threshold of the selected gate: the kernel's fast-math `/ 31 * 64` may round
/// differently from the exact f32 division there.
fn near_threshold(max: f32, second: f32, fast: bool, gate: Gate) -> bool {
    let near = |x: f32, t: f32| (x - t).abs() <= 1e-3 * t;
    gate_seq(fast).any(|(mode, margin)| {
        let span = mode_span(mode);
        if gate == Gate::D16 && (mode == 2 || mode == 6) {
            near(max * margin, 2.0 * span) || near(second * margin, span)
        } else {
            near(max * margin, span)
        }
    })
}

/// The peak gate of `bc6h_setup`: `(guarded, peak texel)`. With `floor` each texel's luminance is floored at
/// `max(peak * 2^-12, 1e-6)` (Task 21) before the geometric mean.
fn guard_of(src: &[[u16; 3]; 16], floor: bool) -> (bool, usize, f32) {
    let l: [f32; 16] = std::array::from_fn(|k| lum(src[k]));
    let (mut pk, mut pl) = (0usize, -1.0f32);
    for (k, &v) in l.iter().enumerate() {
        if v > pl {
            pk = k;
            pl = v;
        }
    }
    let lo = if floor {
        (pl * 2f32.powi(-12)).max(1e-6)
    } else {
        0.0
    };
    let mut log_sum = 0.0f32;
    for &v in &l {
        log_sum += v.max(lo).max(1e-6).ln();
    }
    let guarded = pl.max(1e-6).ln() - log_sum / 16.0 >= 32.0f32.ln();
    (guarded, pk, pl)
}

/// Task 5: the lift (`max(x, m - 12 * 1024 * 64 / 31)` per channel, in the kernel's UF16) changes no channel.
fn lift_changes_nothing(src: &[[u16; 3]; 16], centre: bool) -> bool {
    let drop = (12 * 1024 * 64) as f32 / 31.0;
    src.iter().all(|t| {
        let u = t.map(|h| uf16(h, centre));
        let lo = u[0].max(u[1]).max(u[2]) - drop;
        u.iter().all(|&x| x >= lo)
    })
}

/// astra #5: `(some texel has m < 0x3400 and a channel below half(m) / 4096, same with m < 0x3000)`.
fn lift_falls_short(src: &[[u16; 3]; 16]) -> (bool, bool) {
    let (mut any, mut deep) = (false, false);
    for t in src {
        let m = t[0].max(t[1]).max(t[2]);
        if m < 0x3400 && t.iter().any(|&h| h2f(h) < h2f(m) / 4096.0) {
            any = true;
            deep |= m < 0x3000;
        }
    }
    (any, deep)
}

/// astra #2: whether another index for texel `k` decodes with a luminance ratio of at least 0.9 (the index bits are
/// rewritten in a copy of the 16 bytes and the block decoded with `decode_image`).
fn alt_index_reaches(b: &[u8; 16], mode: usize, k: usize, src_lum: f32) -> bool {
    let two = mode <= 9;
    let anchor2 = if two {
        second_anchor(partition_of(b))
    } else {
        0
    };
    let (pos, width) = index_field(two, anchor2, k);
    let cur = field(b, pos, width);
    (0..1u32 << width).filter(|&v| v != cur).any(|v| {
        let mut c = *b;
        set_index(&mut c, pos, width, v);
        let d = decode_image(&c, 4, 4)[k];
        lum(halves_of(d)) / src_lum >= 0.9
    })
}

const HIST_OFF: i32 = 0x8000;

#[derive(Default)]
struct GateAcc {
    blocks: u64,
    changed: u64,
    changed_slow_in_family: u64,
    two_region_out: u64,
    near: u64,
    tie_mismatches: u64,
    idx_ties: u64,
    mismatches: u64,
    examples: Vec<String>,
}

#[derive(Default, Clone, Copy)]
struct FamilyAcc {
    fits: u64,
    near10: u64,
    written: u64,
}

struct LevelAcc {
    /// Per channel, the count of texels for each d = decoded half - source half, at index d + `HIST_OFF`.
    hist: [Vec<u64>; 3],
    sq_total: u64,
    reserved: u64,
    regions: u64,
    collapsed: u64,
    collapsed_sq: u64,
    collapsed_by_mode: [u64; 14],
    gate: GateAcc,
    /// astra #6: family 2 (modes 2-4) and family 6 (modes 6-8).
    fam: [FamilyAcc; 2],
}

impl LevelAcc {
    fn new() -> Self {
        LevelAcc {
            hist: std::array::from_fn(|_| vec![0; 0x10000]),
            sq_total: 0,
            reserved: 0,
            regions: 0,
            collapsed: 0,
            collapsed_sq: 0,
            collapsed_by_mode: [0; 14],
            gate: GateAcc::default(),
            fam: [FamilyAcc::default(); 2],
        }
    }
}

/// One image's level-independent counts (Q6, astra #5) and its VeryslowPeak decoder-side counts (astra #2).
#[derive(Default)]
struct ImgRow {
    image: String,
    blocks: u64,
    guarded: u64,
    not_under_floor: u64,
    no_lift: u64,
    a5: u64,
    a5_deep: u64,
    a2_below: u64,
    a2_anchor: u64,
    a2_anchor_alt: u64,
    a2_other: u64,
    a2_other_alt: u64,
}

impl ImgRow {
    /// The counts in the table's column order.
    fn cells(&self) -> [u64; 11] {
        [
            self.blocks,
            self.guarded,
            self.not_under_floor,
            self.no_lift,
            self.a5,
            self.a5_deep,
            self.a2_below,
            self.a2_anchor,
            self.a2_anchor_alt,
            self.a2_other,
            self.a2_other_alt,
        ]
    }

    /// Adds another row's counts (the table's TOTAL line).
    fn add(&mut self, o: &ImgRow) {
        self.blocks += o.blocks;
        self.guarded += o.guarded;
        self.not_under_floor += o.not_under_floor;
        self.no_lift += o.no_lift;
        self.a5 += o.a5;
        self.a5_deep += o.a5_deep;
        self.a2_below += o.a2_below;
        self.a2_anchor += o.a2_anchor;
        self.a2_anchor_alt += o.a2_anchor_alt;
        self.a2_other += o.a2_other;
        self.a2_other_alt += o.a2_other_alt;
    }
}

fn in_family(mode: Option<usize>, fam: usize) -> bool {
    matches!((fam, mode), (0, Some(2..=4)) | (1, Some(6..=8)))
}

fn diag_image(
    name: &str,
    img: &HalfImage,
    target: IspcTarget,
    opts: DiagOpts,
    accs: &mut [LevelAcc],
    row: &mut ImgRow,
) {
    row.image = name.to_string();
    // Block indexing below (`bw`, `bi * 16`) assumes whole blocks, as `decode_image` does.
    assert!(
        img.width.is_multiple_of(4) && img.height.is_multiple_of(4),
        "{name}: diag needs image sides that are multiples of 4"
    );
    let encs: Vec<Vec<u8>> = LEVELS
        .iter()
        .map(|&l| encode_image(img, l, target))
        .collect();
    let w = img.width as usize;
    let bw = w / 4;
    for by in 0..(img.height as usize / 4) {
        for bx in 0..bw {
            let bi = by * bw + bx;
            let src = src_block(&img.px, w, bx, by);
            row.blocks += 1;
            // The self-check below uses the spans the kernel is assumed to see (`--centre`); the Q4 table of
            // changed picks always uses the default conversion, so it does not move with that option.
            let (max, idx, second) = rank_spans(&src, opts.centre);
            let (tmax, tidx, tsecond) = if opts.centre {
                rank_spans(&src, false)
            } else {
                (max, idx, second)
            };
            let (guarded, peak_k, peak_lum) = guard_of(&src, false);
            if guarded {
                row.guarded += 1;
                row.not_under_floor += u64::from(!guard_of(&src, true).0);
                row.no_lift += u64::from(lift_changes_nothing(&src, opts.centre));
                let (any, deep) = lift_falls_short(&src);
                row.a5 += u64::from(any);
                row.a5_deep += u64::from(deep);
            }
            let outs: [&[u8; 16]; 6] =
                std::array::from_fn(|li| encs[li][bi * 16..bi * 16 + 16].try_into().unwrap());
            let modes: [Option<usize>; 6] = outs.map(kernel_mode);
            let decs: [[[u16; 3]; 16]; 6] = outs.map(decode_block);
            for (li, acc) in accs.iter_mut().enumerate() {
                let dec = &decs[li];
                // Q5 and the level's squared error.
                for (s, d) in src.iter().zip(dec) {
                    for c in 0..3 {
                        let diff = i32::from(d[c]) - i32::from(s[c]);
                        acc.hist[c][(diff + HIST_OFF) as usize] += 1;
                        acc.sq_total += (diff * diff) as u64;
                    }
                }
                // Q2.
                match modes[li] {
                    None => acc.reserved += 1,
                    Some(m) => {
                        let two = m <= 9;
                        let part = if two { partition_of(outs[li]) } else { 0 };
                        for r in 0..if two { 2 } else { 1 } {
                            // No allocation: the region's texels are a filter over 0..16.
                            let ts =
                                || (0..16usize).filter(move |&k| !two || region_of(part, k) == r);
                            let first = ts().next().unwrap();
                            let src_const = ts().all(|k| src[k] == src[first]);
                            let dec_const = ts().all(|k| dec[k] == dec[first]);
                            acc.regions += 1;
                            if dec_const && !src_const {
                                acc.collapsed += 1;
                                acc.collapsed_by_mode[m] += 1;
                                for k in ts() {
                                    for c in 0..3 {
                                        let diff = i32::from(dec[k][c]) - i32::from(src[k][c]);
                                        acc.collapsed_sq += (diff * diff) as u64;
                                    }
                                }
                            }
                        }
                    }
                }
                // Q4: the gate replication at Fast (1) and Basic (2).
                if li == 1 || li == 2 {
                    let fast = li == 1;
                    let pt = gate_pick(tmax, tidx, tsecond, fast, Gate::Today);
                    let pd = gate_pick(tmax, tidx, tsecond, fast, Gate::D16);
                    let g = &mut acc.gate;
                    g.blocks += 1;
                    if pt != pd {
                        g.changed += 1;
                        let fam = usize::from(pd >= 6);
                        if matches!(pd, 2..=4 | 6..=8) && in_family(modes[3], fam) {
                            g.changed_slow_in_family += 1;
                        }
                    }
                    let pick = gate_pick(max, idx, second, fast, opts.gate);
                    let near = near_threshold(max, second, fast, opts.gate);
                    g.near += u64::from(near);
                    if let Some(m) = modes[li].filter(|&m| m <= 9) {
                        g.two_region_out += 1;
                        if m != pick && !(!fast && m == 1) {
                            // Two channels' spans within 1e-3 of each other: the kernel's fast-math rounding may pick
                            // the other channel as `max_span_idx`, which moves mode 2 or 6 to the neighbour mode of
                            // the same family and nothing else.
                            let same_family = matches!((pick, m), (2..=4, 2..=4) | (6..=8, 6..=8));
                            if near {
                                g.tie_mismatches += 1;
                            } else if same_family && (max - second).abs() <= 1e-3 * max {
                                g.idx_ties += 1;
                            } else {
                                g.mismatches += 1;
                                if g.examples.len() < 5 {
                                    g.examples.push(format!(
                                        "{name} block ({bx},{by}): max {max} second {second} idx {idx} pick {pick} output mode {m}"
                                    ));
                                }
                            }
                        }
                    }
                }
                // astra #6 (source-side proxy) at the slow levels.
                if li >= 3 {
                    for (f, span) in [(0, mode_span(2)), (1, mode_span(6))] {
                        let fa = &mut acc.fam[f];
                        if max <= 2.0 * span && second > span {
                            fa.fits += 1;
                            fa.near10 += u64::from(second >= 0.9 * max);
                        }
                        fa.written += u64::from(in_family(modes[li], f));
                    }
                }
                // astra #2 at VeryslowPeak.
                if li == 5 && guarded {
                    let ratio = lum(dec[peak_k]) / peak_lum;
                    if ratio < 0.9 {
                        row.a2_below += 1;
                        if let Some(m) = modes[li] {
                            let two = m <= 9;
                            let anchor = peak_k == 0
                                || (two && peak_k == second_anchor(partition_of(outs[li])));
                            let alt = alt_index_reaches(outs[li], m, peak_k, peak_lum);
                            if anchor {
                                row.a2_anchor += 1;
                                row.a2_anchor_alt += u64::from(alt);
                            } else {
                                row.a2_other += 1;
                                row.a2_other_alt += u64::from(alt);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Mean and median of d over the histogram's indices `lo..=hi` (the median of an even count is the mean of the
/// two middle values); `(count, mean, median)`.
fn hist_stats(h: &[u64], lo: usize, hi: usize) -> (u64, f64, f64) {
    let n: u64 = h[lo..=hi].iter().sum();
    if n == 0 {
        return (0, f64::NAN, f64::NAN);
    }
    let sum: f64 = (lo..=hi)
        .map(|i| (i as i64 - i64::from(HIST_OFF)) as f64 * h[i] as f64)
        .sum();
    let at = |rank: u64| {
        let mut cum = 0;
        for (i, &count) in h.iter().enumerate().take(hi + 1).skip(lo) {
            cum += count;
            if cum > rank {
                return (i as i64 - i64::from(HIST_OFF)) as f64;
            }
        }
        unreachable!()
    };
    (n, sum / n as f64, (at((n - 1) / 2) + at(n / 2)) / 2.0)
}

fn pct(a: u64, b: u64) -> f64 {
    if b == 0 {
        0.0
    } else {
        100.0 * a as f64 / b as f64
    }
}

fn diag_report(set: &str, accs: &[LevelAcc], rows: &[ImgRow], opts: DiagOpts) -> String {
    let mut s = String::new();
    let c = HIST_OFF as usize;
    writeln!(s, "==== set {set} ====").unwrap();
    writeln!(
        s,
        "(gate for the replication: {:?}; UF16 conversion: {})\n",
        opts.gate,
        if opts.centre {
            "(h + 0.5) / 31 * 64"
        } else {
            "(h / 31) * 64"
        }
    )
    .unwrap();
    writeln!(s, "-- Q5 bias: d = decoded half - source half, per channel (all texels / texels with |d| <= 4) --").unwrap();
    writeln!(
        s,
        "level          ch  n_all      mean_all   median_all  n_tail     mean_tail  median_tail"
    )
    .unwrap();
    for (l, a) in LEVELS.iter().zip(accs) {
        for (ch, h) in a.hist.iter().enumerate() {
            let (n, m, md) = hist_stats(h, 0, 0xFFFF);
            let (nt, mt, mdt) = hist_stats(h, c - 4, c + 4);
            writeln!(
                s,
                "{:<14} {}   {n:<10} {m:<10.4} {md:<12.1} {nt:<10} {mt:<10.4} {mdt:.1}",
                level_name(*l),
                ["R", "G", "B"][ch]
            )
            .unwrap();
        }
    }
    writeln!(s, "\n-- Q2 collapsed regions: decoded texels all equal (R, G and B) while the source texels are not --").unwrap();
    writeln!(s, "(a region with distinct endpoints but one shared index also counts; squared error in half codes)").unwrap();
    writeln!(
        s,
        "level          regions     collapsed  share%   sq_share%  reserved  by mode 0..9 (two-region), 10..13 (one-region)"
    )
    .unwrap();
    for (l, a) in LEVELS.iter().zip(accs) {
        let modes: Vec<String> = a.collapsed_by_mode.iter().map(u64::to_string).collect();
        writeln!(
            s,
            "{:<14} {:<11} {:<9} {:<8.3} {:<10.3} {:<9} {}",
            level_name(*l),
            a.regions,
            a.collapsed,
            pct(a.collapsed, a.regions),
            pct(a.collapsed_sq, a.sq_total),
            a.reserved,
            modes.join(" ")
        )
        .unwrap();
    }
    writeln!(s, "\n-- Q4 gate: today's gate against D16 on the source spans, and the replication self-check --").unwrap();
    writeln!(
        s,
        "level  blocks     changed_picks  changed_slow_in_new_family  two_region_out  near_threshold  tie_mismatches  idx_ties  mismatches"
    )
    .unwrap();
    for li in [1, 2] {
        let g = &accs[li].gate;
        writeln!(
            s,
            "{:<6} {:<10} {:<14} {:<27} {:<15} {:<15} {:<15} {:<9} {}",
            level_name(LEVELS[li]),
            g.blocks,
            g.changed,
            g.changed_slow_in_family,
            g.two_region_out,
            g.near,
            g.tie_mismatches,
            g.idx_ties,
            g.mismatches
        )
        .unwrap();
        for e in &g.examples {
            writeln!(s, "  MISMATCH {e}").unwrap();
        }
    }
    writeln!(
        s,
        "(changed_slow_in_new_family: Slow's output is in the family D16 newly picks, kernel modes 2-4 or 6-8;\ntie_mismatches: output differs from the pick and a span is within 1e-3 of a gate threshold; idx_ties: the output is the neighbour mode of the pick's family (a different max_span_idx) and the two widest spans are within 1e-3 of each other; both are fast-math rounding, outside the stop rule;\nself-check: at Basic an output of mode 1 is not a mismatch)"
    )
    .unwrap();
    writeln!(
        s,
        "\n-- astra #6 (source-side PROXY; the deciding count is Task 22's all-variants build) --"
    )
    .unwrap();
    writeln!(
        s,
        "level          family  fits_doubled_window_second_exceeds_single  within_10pct  written_by_the_level"
    )
    .unwrap();
    for li in 3..6 {
        for (f, name) in ["2 (modes 2-4)", "6 (modes 6-8)"].into_iter().enumerate() {
            let fa = accs[li].fam[f];
            writeln!(
                s,
                "{:<14} {:<14} {:<42} {:<13} {}",
                level_name(LEVELS[li]),
                name,
                fa.fits,
                fa.near10,
                fa.written
            )
            .unwrap();
        }
    }
    writeln!(
        s,
        "\n-- Q6 guarded blocks, Task 5 and astra #5 and #2, per image --"
    )
    .unwrap();
    writeln!(
        s,
        "guarded: peak >= 32 x geometric mean luminance; floor: guarded today but not under Task 21's floor; no_lift: Task 5's lift changes no channel;"
    )
    .unwrap();
    writeln!(
        s,
        "a5: a texel with m < 0x3400 and a channel < half(m)/4096 (a5_deep: m < 0x3000); a2 (VeryslowPeak): peak texel ratio < 0.9, anchor/other texel, alt: another index reaches >= 0.9"
    )
    .unwrap();
    writeln!(
        s,
        "{}",
        img_line(
            "image",
            [
                "blocks",
                "guarded",
                "floor",
                "no_lift",
                "a5",
                "a5_deep",
                "a2_below",
                "a2_anchor",
                "anchor_alt",
                "a2_other",
                "other_alt"
            ]
        )
    )
    .unwrap();
    let mut tot = ImgRow::default();
    for r in rows {
        writeln!(s, "{}", img_line(&r.image, r.cells())).unwrap();
        tot.add(r);
    }
    writeln!(s, "{}\n", img_line("TOTAL", tot.cells())).unwrap();
    s
}

/// One line of the per-image table: the name, then the 11 columns of `ImgRow::cells`.
fn img_line<T: std::fmt::Display>(name: &str, c: [T; 11]) -> String {
    format!(
        "{name:<48} {:>9} {:>8} {:>6} {:>8} {:>5} {:>8} {:>8} {:>10} {:>9} {:>9} {:>8}",
        c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7], c[8], c[9], c[10]
    )
}

fn diag(args: &[String]) -> Res<()> {
    let mut root = None;
    let mut out = None;
    let mut target = IspcTarget::Avx2;
    let mut opts = DiagOpts {
        gate: Gate::Today,
        centre: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--root" => root = Some(PathBuf::from(val()?)),
            "--out" => out = Some(PathBuf::from(val()?)),
            "--threads" => {
                let n: usize = val()?
                    .parse()
                    .map_err(|e| format!("--threads needs a number of 1 or more: {e}"))?;
                if n == 0 || THREADS.set(n).is_err() {
                    return Err("--threads needs a number of 1 or more, given once".into());
                }
            }
            "--target" => {
                target = match val()?.as_str() {
                    "sse2" => IspcTarget::Sse2,
                    "avx2" => IspcTarget::Avx2,
                    "avx512" => IspcTarget::Avx512,
                    t => return Err(format!("unknown target {t}").into()),
                }
            }
            "--gate" => {
                opts.gate = match val()?.as_str() {
                    "today" => Gate::Today,
                    "d16" => Gate::D16,
                    g => return Err(format!("unknown gate {g}: today or d16").into()),
                }
            }
            "--centre" => opts.centre = true,
            _ => return Err(format!("unknown argument {a}").into()),
        }
    }
    let root = root.ok_or("--root is required")?;
    let out = out.ok_or("--out is required")?;
    if !target.supported() {
        return Err(format!("this CPU cannot run {target:?}").into());
    }
    let mut text = format!(
        "bc6h_quality diag, target {target:?}, gate {:?}, centre {}\n\n",
        opts.gate, opts.centre
    );
    let mut stop = 0u64;
    for (set, files) in sets(&root)? {
        let mut accs: Vec<LevelAcc> = LEVELS.iter().map(|_| LevelAcc::new()).collect();
        let mut rows = Vec::new();
        for path in files {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let (img, _) = clamp_image(&load_f16(&path)?);
            let mut row = ImgRow::default();
            diag_image(&name, &img, target, opts, &mut accs, &mut row);
            eprintln!("{set} {name} done");
            rows.push(row);
        }
        stop += accs[1].gate.mismatches + accs[2].gate.mismatches;
        text.push_str(&diag_report(set, &accs, &rows, opts));
    }
    std::fs::write(&out, &text)?;
    if stop > 0 {
        return Err(format!(
            "the gate replication disagrees with the kernel on {stop} blocks away from a threshold and from an idx_ties neighbour-mode tie (see MISMATCH lines in {})",
            out.display()
        )
        .into());
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let res = match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("compare") => compare(&args[1..]),
        Some("diag") => diag(&args[1..]),
        _ => Err("usage: bc6h_quality run --root <dir> --out <csv> [--target t] [--threads N] | compare <a.csv> <b.csv> [--pair a:b] | diag --root <dir> --out <txt> [--target t] [--threads N] [--gate today|d16] [--centre]".into()),
    };
    if let Err(e) = res {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

// `test = true` on the `[[example]]` in Cargo.toml makes a plain `cargo test -p encoders-sys` run these.
#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "set,image,level,target,clamped,mpsnr,rmsle,peak_ratio,seconds,sha256\n";

    fn csv(rows: &[(&str, &str, &str)]) -> Vec<Row> {
        let body: String = rows
            .iter()
            .map(|(s, i, l)| format!("{s},{i},{l},avx2,0,50.0000,0.003000,0.900000,1.000,aa\n"))
            .collect();
        parse_csv(&format!("{HEAD}{body}"), "t.csv").unwrap()
    }

    #[test]
    fn compare_rejects_an_unknown_level() {
        let r = csv(&[("hdr_real", "a", "veryslow")]);
        let e = compare_pair(&r, &r, "veryslow", "veryslowpeak").unwrap_err();
        assert!(e.to_string().contains("unknown level veryslowpeak"), "{e}");
    }

    #[test]
    fn a_duplicate_key_is_an_error() {
        let t = format!("{HEAD}hdr_real,a,veryslow,avx2,0,50.0,0.003,0.9,1.0,aa\nhdr_real,a,veryslow,avx2,0,50.0,0.003,0.9,1.0,bb\n");
        let e = parse_csv(&t, "t.csv").unwrap_err();
        assert!(
            e.to_string().contains("duplicate row hdr_real a veryslow"),
            "{e}"
        );
    }

    #[test]
    fn an_unpaired_image_is_an_error_naming_it() {
        let a = csv(&[("hdr_real", "a", "veryslow"), ("hdr_real", "b", "veryslow")]);
        let b = csv(&[("hdr_real", "a", "veryslow")]);
        let e = compare_pair(&a, &b, "veryslow", "veryslow").unwrap_err();
        assert!(e.to_string().contains("only one file: b"), "{e}");
    }

    #[test]
    fn the_pattern_table_has_32_sound_partitions() {
        let region = |p: usize| -> Vec<usize> { (0..16).map(|k| region_of(p, k)).collect() };
        for p in 0..32 {
            let r = region(p);
            assert_eq!(r[0], 0, "partition {p}: texel 0 is in region 0");
            assert!(
                r.iter().all(|&x| x < 2),
                "partition {p}: only regions 0 and 1"
            );
            assert!(
                r.contains(&0) && r.contains(&1),
                "partition {p}: both regions used"
            );
            assert_eq!(
                r[second_anchor(p)],
                1,
                "partition {p}: second anchor in region 1"
            );
            for q in 0..p {
                assert_ne!(r, region(q), "partitions {p} and {q} have one shape");
            }
        }
    }

    fn block_with_mode_field(f: u32) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[0] = f as u8;
        b
    }

    #[test]
    fn the_mode_mapping_goes_through_the_prefix_table() {
        for (field5, mode) in [(3, 10), (2, 2), (6, 3), (18, 6), (15, 13)] {
            assert_eq!(kernel_mode(&block_with_mode_field(field5)), Some(mode));
        }
        assert_eq!(kernel_mode(&block_with_mode_field(0)), Some(0));
        assert_eq!(kernel_mode(&block_with_mode_field(1)), Some(1));
        // The reserved 5-bit fields.
        assert_eq!(kernel_mode(&block_with_mode_field(19)), None);
    }

    #[test]
    fn the_gate_replication_matches_two_hand_computed_blocks() {
        // The audit's block: R spans 6000 UF16 units, G and B 2000. Mode 1 (span 14745) passes at Fast and nothing
        // narrower does today; under D16 mode 6 passes (6000 / 1.2 <= 2 * 3686.3 and 2000 / 1.2 <= 3686.3).
        assert_eq!(gate_pick(6000.0, 0, 2000.0, true, Gate::Today), 1);
        assert_eq!(gate_pick(6000.0, 0, 2000.0, true, Gate::D16), 6);
        assert_eq!(gate_pick(6000.0, 0, 2000.0, false, Gate::Today), 9);
        assert_eq!(gate_pick(6000.0, 0, 2000.0, false, Gate::D16), 6);
        // The widest channel is G: mode 6 becomes 7.
        assert_eq!(gate_pick(6000.0, 1, 2000.0, true, Gate::D16), 7);
        // Spans under 200: every test passes and mode 2 (+ idx) wins, under both gates.
        for gate in [Gate::Today, Gate::D16] {
            assert_eq!(gate_pick(100.0, 1, 50.0, true, gate), 3);
            assert_eq!(gate_pick(100.0, 2, 50.0, false, gate), 4);
        }
    }

    #[test]
    fn the_floor_unguards_a_block_of_black_texels_beside_a_bright_one() {
        // 11 texels of luminance 1 and 5 black: ln(1 / geomean) = 5 * 13.8 / 16 = 4.3 >= ln 32 today; floored at
        // 2^-12 the five black texels count 8.3 each: 2.6 < ln 32.
        let src: [[u16; 3]; 16] =
            std::array::from_fn(|t| if t < 11 { [0x3C00; 3] } else { [0; 3] });
        assert!(guard_of(&src, false).0);
        assert!(!guard_of(&src, true).0);
        assert_eq!(guard_of(&src, false).1, 0, "the first maximum");
        assert!(
            !guard_of(&[[0x3C00; 3]; 16], false).0,
            "a flat block is not guarded"
        );
    }

    #[test]
    fn spans_rank_the_widest_first_and_keep_the_first_maximum() {
        let mut src = [[0x3C00u16; 3]; 16];
        src[3] = [0x3C00 + 31, 0x3C00 + 31, 0x3C00];
        let (max, idx, second) = rank_spans(&src, false);
        assert!((max - 64.0).abs() < 0.01, "{max}");
        assert_eq!((idx, second), (0, max));
    }

    #[test]
    fn an_index_written_into_a_zeroed_block_reads_back() {
        for (two, anchor2) in [(false, 0usize), (true, 8), (true, 2)] {
            let mut end = 0;
            for k in 0..16 {
                let (pos, width) = index_field(two, anchor2, k);
                let is_anchor = k == 0 || (two && k == anchor2);
                assert_eq!(
                    width,
                    if is_anchor {
                        if two {
                            2
                        } else {
                            3
                        }
                    } else if two {
                        3
                    } else {
                        4
                    }
                );
                assert_eq!(
                    pos,
                    if k == 0 {
                        if two {
                            82
                        } else {
                            65
                        }
                    } else {
                        end
                    }
                );
                end = pos + width;
                for v in 0..1u32 << width {
                    let mut b = [0u8; 16];
                    set_index(&mut b, pos, width, v);
                    assert_eq!(field(&b, pos, width), v, "two {two} texel {k} index {v}");
                    assert_eq!(u128::from_le_bytes(b).count_ones(), v.count_ones());
                }
            }
            assert_eq!(end, 128, "the 16 indices end at the last bit");
        }
    }

    #[test]
    fn the_median_is_the_middle_of_the_sorted_values() {
        let mut h = vec![0u64; 0x10000];
        for d in [-3i32, 0, 0, 5] {
            h[(d + HIST_OFF) as usize] += 1;
        }
        let (n, mean, median) = hist_stats(&h, 0, 0xFFFF);
        assert_eq!((n, mean, median), (4, 0.5, 0.0));
    }

    #[test]
    fn paired_rows_compare() {
        let a = csv(&[("hdr_real", "a", "veryslow"), ("hdr_grad", "g", "veryslow")]);
        let s = compare_pair(&a, &a, "veryslow", "veryslow").unwrap();
        assert!(s.contains("hdr_real: n = 1"), "{s}");
    }
}
