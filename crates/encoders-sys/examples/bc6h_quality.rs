//! BC6H quality measurement of the vendored ISPC TC kernel on the bench's HDR images, and the comparison of two runs.
//!
//!   cargo run --offline --release -p encoders-sys --example bc6h_quality -- run --root <bench root> --out <csv>
//!       [--target sse2|avx2|avx512] [--threads N]
//!   cargo run --offline --release -p encoders-sys --example bc6h_quality -- compare <before.csv> <after.csv>
//!       [--pair <levelA>:<levelB>]
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
            let mut clamped = 0u64;
            let px: Vec<[u16; 4]> = src
                .px
                .iter()
                .map(|p| {
                    let mut o = [0x3C00u16; 4];
                    for c in 0..3 {
                        let bits =
                            half::f16::from_f32(clamp_half(half::f16::from_bits(p[c]).to_f32()))
                                .to_bits();
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let res = match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("compare") => compare(&args[1..]),
        _ => Err("usage: bc6h_quality run --root <dir> --out <csv> [--target t] [--threads N] | compare <a.csv> <b.csv> [--pair a:b]".into()),
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
    fn paired_rows_compare() {
        let a = csv(&[("hdr_real", "a", "veryslow"), ("hdr_grad", "g", "veryslow")]);
        let s = compare_pair(&a, &a, "veryslow", "veryslow").unwrap();
        assert!(s.contains("hdr_real: n = 1"), "{s}");
    }
}
