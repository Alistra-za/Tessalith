//! BC7 quality measurement of the vendored bc7e kernel on lists of PNGs, and the comparison of two runs.
//!
//!   cargo run --offline --release -p encoders-sys --example bc7_quality -- run
//!       --class <name>=<path>[:<flags>] ... --out <csv> [--levels basic,veryslow,max] [--weights 1,16]
//!       [--perceptual off|on|both] [--exact off|on]
//!       [--tuning default,data,x16,data_x16,data_exact_g_x16,data_guard_<letters>] [--target sse2|avx2|avx512]
//!       [--threads N]
//!   cargo run --offline --release -p encoders-sys --example bc7_quality -- compare <before.csv> <after.csv>
//!       [--class <name>]
//!
//! Defaults: level veryslow, weights 1, perceptual off, exact off (`exact_opaque`), tuning default, target avx2, every
//! available thread.
//!
//! A class `<path>` is a folder (its `*.png`, sorted) or a `.tsv` list. The list's first column is a path and its
//! optional second column a SHA-256, which is checked: a mismatch is an error. The flags are a comma list:
//! `opaque` forces alpha to 255 before encoding (the engine's `remove`), `normal` adds the angle metric.
//! A missing folder or file is an error, as is a size that is not a multiple of 4.
//!
//! `run` encodes as the engine's `encode_band` does: RGBA8 through `image`, 4x4 blocks from `gather_blocks`, 64 blocks
//! per call through `bc7e_compress_blocks_on`, or through the tuned call when the row overrides something (Note CP; a
//! perceptual row runs once, at w 1). Each `--tuning` value is a row: `default` (the level as it is), `data` (flat
//! tolerance `Bc7FlatTolerance::Data`), `x16` (the level's weights at the level's tolerance), `data_x16` and
//! `data_exact_g_x16` (`data_x16` with exact constants on G, `Bc7Tune::exact_constants` 0b010), and
//! `data_guard_<letters>` (`data_x16` with the channel guard on the letters' channels, in R, G, B order: `r`, `g`, `b`,
//! `rg`, `rb`, `gb` or `rgb`; `data_guard_g` is `data_exact_g_x16`). The names are kept, so earlier CSVs still join
//! in `compare`: "x16" now means the level's weights, which rank as x16 since patch 0005's L16 (Note LR, LR15), where
//! these rows sent the weights scaled by 16 before it. With perceptual off a row sends the weights `[s, s, s, s * w]`
//! (s = 1 for every value; `[1, 1, 1, 1]` is sent unset), its flat tolerance and its exact-constants mask; with
//! perceptual on only its flat tolerance, so `default` and `x16` are the plain call there (Note LR). So an `x16` row
//! repeats `default`'s bytes and a `data_x16` row `data`'s, and under perceptual `data_exact_g_x16` and the
//! `data_guard` rows repeat `data`'s too: they are encoded again and kept, so every tuning has its rows for `compare`.
//! `bc7_fixup::force_opaque_rotated` is applied to every block whose source alpha is 255 throughout (no zero fix), then
//! the blocks are decoded with `bcdec_rs::bc7` and scored against the source. The chunks of 64 blocks are spread over
//! the threads, so the output does not depend on `--threads`.
//!
//! The CSV has one row per image and setting. `se_*` are integer sums of squared errors, `max_*` maximum absolute
//! errors, `ge32_rgb` and `ge32_a` count blocks whose RGB or alpha maximum absolute error is at least 32, `angle` is
//! the mean per-texel angle in degrees between the source and decoded vectors (empty without `normal`), `m0..m7`
//! count blocks by mode, `seconds` is the wall time of encode, fix, decode and scoring, and `fnv` is FNV-1a 64 of the
//! image's bytes after the post-fix. `tuning` names the row's `--tuning` value.
//!
//! `compare` joins rows on (class, image, level, w, perceptual, exact, tuning) and prints, per class and setting, the
//! paired mean dB delta (after minus before, + is better) of R, G, B, A, RGB pooled and the objective (squared error
//! RGB + w x A) with a paired percentile bootstrap 95 % interval (10,000 resamples, xorshift64* seed 20261122), wins,
//! losses and the worst image; the angle delta (+ is worse); blocks with an error of 32 or more per image; the mode
//! mix; the count of byte-identical outputs and the seconds.
use encoders_sys::bc7_fixup::force_opaque_rotated;
use encoders_sys::{
    bc7e_compress_blocks_on, bc7e_compress_blocks_tuned_on, gather_blocks, Bc7FlatTolerance,
    Bc7Level, Bc7Params, Bc7Tune, IspcTarget,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

type Res<T> = Result<T, Box<dyn Error>>;

const HEADER: &str = "class,image,level,w,perceptual,exact,tuning,target,blocks,se_r,se_g,se_b,se_a,max_r,max_g,max_b,max_a,ge32_rgb,ge32_a,angle,m0,m1,m2,m3,m4,m5,m6,m7,seconds,fnv";
const FIELDS: usize = 30;
const SEED: u64 = 20_261_122;
const RESAMPLES: usize = 10_000;
/// Blocks per encoder call, as the engine's `BC7_BLOCKS_PER_CALL`.
const CALL_BLOCKS: usize = 64;

const LEVELS: [(&str, Bc7Level); 8] = [
    ("ultrafast", Bc7Level::Ultrafast),
    ("veryfast", Bc7Level::Veryfast),
    ("fast", Bc7Level::Fast),
    ("basic", Bc7Level::Basic),
    ("slow", Bc7Level::Slow),
    ("veryslow", Bc7Level::Veryslow),
    ("slowest", Bc7Level::Slowest),
    ("max", Bc7Level::Max),
];

/// A `--tuning` value: name, weight scale, flat tolerance (`None` keeps the level's) and exact-constants mask.
type Tuning = (&'static str, u32, Option<Bc7FlatTolerance>, Option<u8>);

/// The data maps' flat tolerance, T_data.
const DATA: Option<Bc7FlatTolerance> = Some(Bc7FlatTolerance::Data);

/// The `--tuning` values. The `x16` names keep scale 1: since patch 0005's L16 the level's weights rank as x16 (Note
/// LR, LR15), and a scale of 16 would rank at x256. The scale column is kept, though every value is 1 since L16,
/// because the names and the earlier run lines and CSVs that `compare` joins on still name the x16 tunings.
const TUNINGS: [Tuning; 12] = [
    ("default", 1, None, None),
    ("data", 1, DATA, None),
    ("x16", 1, None, None),
    ("data_x16", 1, DATA, None),
    ("data_exact_g_x16", 1, DATA, Some(0b010)),
    ("data_guard_r", 1, DATA, Some(0b001)),
    ("data_guard_g", 1, DATA, Some(0b010)),
    ("data_guard_b", 1, DATA, Some(0b100)),
    ("data_guard_rg", 1, DATA, Some(0b011)),
    ("data_guard_rb", 1, DATA, Some(0b101)),
    ("data_guard_gb", 1, DATA, Some(0b110)),
    ("data_guard_rgb", 1, DATA, Some(0b111)),
];

fn parse_tuning(s: &str) -> Res<Tuning> {
    TUNINGS
        .iter()
        .find(|t| t.0 == s)
        .copied()
        .ok_or_else(|| format!("unknown tuning {s}").into())
}

fn parse_level(s: &str) -> Res<(&'static str, Bc7Level)> {
    LEVELS
        .iter()
        .find(|(n, _)| *n == s)
        .copied()
        .ok_or_else(|| format!("unknown level {s}").into())
}

/// `--threads N` of `run`; unset, every available thread.
static THREADS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

fn threads() -> usize {
    *THREADS.get_or_init(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
}

// ---- classes ----

#[derive(Debug)]
struct Class {
    name: String,
    /// Each file with its expected SHA-256 (lower-case hex), when the list gave one.
    files: Vec<(PathBuf, Option<String>)>,
    opaque: bool,
    normal: bool,
}

/// `<name>=<path>[:<flags>]`. The text after the last `:` is the flag list only when every item is a known flag, so
/// a Windows drive letter stays in the path.
fn parse_class(spec: &str) -> Res<Class> {
    let (name, rest) = spec
        .split_once('=')
        .ok_or_else(|| format!("--class {spec}: expected <name>=<path>[:<flags>]"))?;
    if name.is_empty() || name.contains(',') {
        return Err(format!("--class {spec}: bad class name").into());
    }
    let (mut opaque, mut normal) = (false, false);
    let mut path = rest;
    if let Some((p, f)) = rest.rsplit_once(':') {
        let items: Vec<&str> = f.split(',').collect();
        if items.iter().all(|i| *i == "opaque" || *i == "normal") {
            path = p;
            opaque = items.contains(&"opaque");
            normal = items.contains(&"normal");
        }
    }
    let path = Path::new(path);
    let files = if path.is_dir() {
        let mut v = Vec::new();
        for e in std::fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")) {
                v.push((p, None));
            }
        }
        v.sort();
        v
    } else if path
        .extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("tsv"))
    {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        parse_list(&text, path)?
    } else {
        return Err(format!("{}: not a folder or a .tsv list", path.display()).into());
    };
    if files.is_empty() {
        return Err(format!("class {name}: no images in {}", path.display()).into());
    }
    let mut seen = BTreeSet::new();
    for (p, _) in &files {
        let stem = image_name(p);
        if stem.contains(',') || !seen.insert(stem.clone()) {
            return Err(
                format!("class {name}: image name {stem} is repeated or holds a comma").into(),
            );
        }
    }
    Ok(Class {
        name: name.into(),
        files,
        opaque,
        normal,
    })
}

/// The rows of a `.tsv` list: a path, then an optional SHA-256 (blank lines are skipped).
fn parse_list(text: &str, from: &Path) -> Res<Vec<(PathBuf, Option<String>)>> {
    let mut v = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let mut c = line.split('\t');
        let path = c.next().unwrap_or("");
        let sha = c
            .next()
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());
        if let Some(s) = &sha {
            if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("{}:{}: not a SHA-256: {s}", from.display(), n + 1).into());
            }
        }
        v.push((PathBuf::from(path), sha));
    }
    Ok(v)
}

fn image_name(p: &Path) -> String {
    p.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

struct Img {
    w: u32,
    h: u32,
    px: Vec<[u8; 4]>,
}

/// Reads a PNG as RGBA8, checking the SHA-256 of the file when one is given.
fn load(path: &Path, sha: Option<&str>, opaque: bool) -> Res<Img> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(want) = sha {
        let got = hex::encode(Sha256::digest(&bytes));
        if got != want {
            return Err(
                format!("{}: SHA-256 is {got}, the list says {want}", path.display()).into(),
            );
        }
    }
    let rgba = image::load_from_memory(&bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .to_rgba8();
    let (w, h) = rgba.dimensions();
    if w % 4 != 0 || h % 4 != 0 {
        return Err(format!("{}: {w}x{h} is not a multiple of 4", path.display()).into());
    }
    let mut px: Vec<[u8; 4]> = rgba.pixels().map(|p| p.0).collect();
    if opaque {
        for p in &mut px {
            p[3] = 255;
        }
    }
    Ok(Img { w, h, px })
}

// ---- scoring ----

#[derive(Clone, Copy, Default)]
struct Stats {
    se: [u64; 4],
    mx: [u8; 4],
    ge32_rgb: u64,
    ge32_a: u64,
    angle: f64,
    modes: [u64; 8],
}

impl Stats {
    fn add(&mut self, o: &Stats) {
        for c in 0..4 {
            self.se[c] += o.se[c];
            self.mx[c] = self.mx[c].max(o.mx[c]);
        }
        self.ge32_rgb += o.ge32_rgb;
        self.ge32_a += o.ge32_a;
        self.angle += o.angle;
        for m in 0..8 {
            self.modes[m] += o.modes[m];
        }
    }
}

fn nvec(c: &[u8; 4]) -> [f64; 3] {
    let v = [0, 1, 2].map(|i| c[i] as f64 / 255.0 * 2.0 - 1.0);
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}

/// The angle in degrees between two texels read as vectors (RGB to [-1, 1], normalised).
fn angle(s: &[u8; 4], d: &[u8; 4]) -> f64 {
    let (a, b) = (nvec(s), nvec(d));
    (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}

/// Scores one block (16 source texels against the 16 decoded) and adds its mode.
fn score_block(src: &[[u8; 4]], dec: &[[u8; 4]], mode_byte: u8, normal: bool, st: &mut Stats) {
    let mut bmax = [0u8; 4];
    for (a, b) in src.iter().zip(dec) {
        for c in 0..4 {
            let e = a[c].abs_diff(b[c]);
            st.se[c] += u64::from(e) * u64::from(e);
            bmax[c] = bmax[c].max(e);
        }
        if normal {
            st.angle += angle(a, b);
        }
    }
    for (m, b) in st.mx.iter_mut().zip(bmax) {
        *m = (*m).max(b);
    }
    st.ge32_rgb += u64::from(bmax[..3].iter().any(|&e| e >= 32));
    st.ge32_a += u64::from(bmax[3] >= 32);
    let mode = mode_byte.trailing_zeros() as usize;
    if mode < 8 {
        st.modes[mode] += 1;
    }
}

// ---- run ----

#[derive(Clone, Copy)]
struct Setting {
    level: &'static str,
    lv: Bc7Level,
    w: u32,
    perceptual: bool,
    tuning: &'static str,
    /// The weight scale s of `[s, s, s, s * w]`.
    scale: u32,
    flat: Option<Bc7FlatTolerance>,
    /// The exact-constants mask (`Bc7Tune::exact_constants`).
    exact_mask: Option<u8>,
}

/// The tuned call's overrides for a row, or `None` for the plain call. With perceptual off: the weights
/// `[s, s, s, s * w]` (`[1, 1, 1, 1]` sent unset), the flat tolerance and the exact-constants mask; with perceptual on
/// only the flat tolerance, so the level's perceptual weights stay (Note LR).
fn tune_of(s: &Setting) -> Option<Bc7Tune> {
    let (weights, exact_constants) = if s.perceptual {
        (None, None)
    } else {
        let w = [s.scale, s.scale, s.scale, s.scale * s.w];
        ((w != [1, 1, 1, 1]).then_some(w), s.exact_mask)
    };
    let t = Bc7Tune {
        weights,
        flat_tolerance: s.flat,
        exact_constants,
    };
    (t != Bc7Tune::default()).then_some(t)
}

struct Job<'a> {
    src: &'a [[u8; 4]],
    out: &'a mut [u8],
}

/// Encodes, post-fixes, decodes and scores one chunk of up to 64 blocks.
fn process_chunk(
    src: &[[u8; 4]],
    out: &mut [u8],
    s: Setting,
    exact: bool,
    target: IspcTarget,
    normal: bool,
) -> Stats {
    let p = Bc7Params {
        level: s.lv,
        perceptual: s.perceptual,
        exact_opaque: exact,
    };
    match tune_of(&s) {
        Some(tune) => bc7e_compress_blocks_tuned_on(src, out, p, tune, target),
        None => bc7e_compress_blocks_on(src, out, p, target),
    }
    let mut st = Stats::default();
    for (texels, blk) in src.chunks(16).zip(out.chunks_mut(16)) {
        let block: &mut [u8; 16] = blk.try_into().unwrap();
        if texels.iter().all(|t| t[3] == 255) {
            force_opaque_rotated(block);
        }
        let mut raw = [0u8; 64];
        bcdec_rs::bc7(block, &mut raw, 16);
        let dec: [[u8; 4]; 16] = std::array::from_fn(|i| [0, 1, 2, 3].map(|c| raw[i * 4 + c]));
        score_block(texels, &dec, block[0], normal, &mut st);
    }
    st
}

/// The stats of the whole image and its bytes after the post-fix. Chunk partials are added in chunk order, so the
/// result does not depend on the thread count.
fn encode_image(
    blocks: &[[u8; 4]],
    s: Setting,
    exact: bool,
    target: IspcTarget,
    normal: bool,
) -> (Stats, Vec<u8>) {
    let mut out = vec![0u8; blocks.len()];
    let jobs: Vec<(usize, Job)> = blocks
        .chunks(CALL_BLOCKS * 16)
        .zip(out.chunks_mut(CALL_BLOCKS * 16))
        .enumerate()
        .map(|(i, (src, out))| (i, Job { src, out }))
        .collect();
    let n_jobs = jobs.len();
    let queue = Mutex::new(jobs.into_iter());
    let mut parts: Vec<(usize, Stats)> = Vec::with_capacity(n_jobs);
    std::thread::scope(|sc| {
        let workers: Vec<_> = (0..threads().min(n_jobs).max(1))
            .map(|_| {
                sc.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let next = queue.lock().unwrap().next();
                        let Some((i, j)) = next else { break };
                        mine.push((i, process_chunk(j.src, j.out, s, exact, target, normal)));
                    }
                    mine
                })
            })
            .collect();
        for w in workers {
            parts.extend(w.join().unwrap());
        }
    });
    parts.sort_by_key(|p| p.0);
    let mut total = Stats::default();
    for (_, p) in &parts {
        total.add(p);
    }
    (total, out)
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, x| {
        (h ^ u64::from(*x)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn settings(
    levels: &[&str],
    weights: &[u32],
    perceptual: &str,
    tunings: &[&str],
) -> Res<Vec<Setting>> {
    let mut v = Vec::new();
    for l in levels {
        let (level, lv) = parse_level(l)?;
        for t in tunings {
            let (tuning, scale, flat, exact_mask) = parse_tuning(t)?;
            let row = |w, perceptual| Setting {
                level,
                lv,
                w,
                perceptual,
                tuning,
                scale,
                flat,
                exact_mask,
            };
            if perceptual != "on" {
                for &w in weights {
                    v.push(row(w, false));
                }
            }
            if perceptual != "off" {
                v.push(row(1, true));
            }
        }
    }
    Ok(v)
}

fn run(args: &[String]) -> Res<()> {
    let mut specs = Vec::new();
    let mut out = None;
    let mut levels = "veryslow".to_string();
    let mut weights = "1".to_string();
    let mut perceptual = "off".to_string();
    let mut tunings = "default".to_string();
    let mut exact = false;
    let mut target = IspcTarget::Avx2;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--class" => specs.push(val()?.clone()),
            "--out" => out = Some(PathBuf::from(val()?)),
            "--levels" => levels.clone_from(val()?),
            "--weights" => weights.clone_from(val()?),
            "--tuning" => tunings.clone_from(val()?),
            "--perceptual" => {
                perceptual.clone_from(val()?);
                if !["off", "on", "both"].contains(&perceptual.as_str()) {
                    return Err(format!("--perceptual {perceptual}: off, on or both").into());
                }
            }
            "--exact" => {
                exact = match val()?.as_str() {
                    "on" => true,
                    "off" => false,
                    v => return Err(format!("--exact {v}: on or off").into()),
                }
            }
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
    let out = out.ok_or("--out is required")?;
    if specs.is_empty() {
        return Err("at least one --class is required".into());
    }
    if !target.supported() {
        return Err(format!("this CPU cannot run {target:?}").into());
    }
    let weights: Vec<u32> = weights
        .split(',')
        .map(|w| {
            let v: u32 = w.parse().map_err(|e| format!("--weights {w}: {e}"))?;
            // `Bc7Tune::weights` refuses anything else (Note LR, LR25); a usage error beats a panic.
            if (1..=65535).contains(&v) {
                Ok(v)
            } else {
                Err(format!("--weights {w}: must be 1 to 65535"))
            }
        })
        .collect::<Result<_, String>>()?;
    let setts = settings(
        &levels.split(',').collect::<Vec<_>>(),
        &weights,
        &perceptual,
        &tunings.split(',').collect::<Vec<_>>(),
    )?;
    let classes: Vec<Class> = specs.iter().map(|s| parse_class(s)).collect::<Res<_>>()?;
    let mut names = BTreeSet::new();
    if let Some(c) = classes.iter().find(|c| !names.insert(c.name.as_str())) {
        return Err(format!("class {} is given twice", c.name).into());
    }
    let target_name = format!("{target:?}").to_lowercase();
    let mut csv = format!("{HEADER}\n");
    for class in &classes {
        for (path, sha) in &class.files {
            let img = load(path, sha.as_deref(), class.opaque)?;
            let name = image_name(path);
            let blocks = gather_blocks(&img.px, img.w, img.h);
            for s in &setts {
                let t0 = Instant::now();
                let (st, bytes) = encode_image(&blocks, *s, exact, target, class.normal);
                let seconds = t0.elapsed().as_secs_f64();
                let n_blocks = blocks.len() / 16;
                let angle = if class.normal {
                    format!("{:.6}", st.angle / (n_blocks * 16) as f64)
                } else {
                    String::new()
                };
                writeln!(
                    csv,
                    "{},{name},{},{},{},{},{},{target_name},{n_blocks},{},{},{},{},{},{},{},{},{},{},{angle},{},{},{},{},{},{},{},{},{seconds:.3},{:016x}",
                    class.name,
                    s.level,
                    s.w,
                    u8::from(s.perceptual),
                    u8::from(exact),
                    s.tuning,
                    st.se[0],
                    st.se[1],
                    st.se[2],
                    st.se[3],
                    st.mx[0],
                    st.mx[1],
                    st.mx[2],
                    st.mx[3],
                    st.ge32_rgb,
                    st.ge32_a,
                    st.modes[0],
                    st.modes[1],
                    st.modes[2],
                    st.modes[3],
                    st.modes[4],
                    st.modes[5],
                    st.modes[6],
                    st.modes[7],
                    fnv(&bytes)
                )?;
                eprintln!(
                    "{} {name} {} w{} perc {} {} {seconds:.2}s",
                    class.name,
                    s.level,
                    s.w,
                    u8::from(s.perceptual),
                    s.tuning
                );
            }
        }
    }
    std::fs::write(&out, csv).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(())
}

// ---- compare ----

#[derive(Debug, Clone)]
struct Row {
    class: String,
    image: String,
    level: String,
    w: u32,
    perceptual: u8,
    exact: u8,
    tuning: String,
    target: String,
    blocks: u64,
    se: [u64; 4],
    mx: [u32; 4],
    ge32_rgb: u64,
    ge32_a: u64,
    angle: Option<f64>,
    modes: [u64; 8],
    seconds: f64,
    fnv: String,
}

type Cell = (String, String, u32, u8, u8, String);

impl Row {
    fn cell(&self) -> Cell {
        (
            self.class.clone(),
            self.level.clone(),
            self.w,
            self.perceptual,
            self.exact,
            self.tuning.clone(),
        )
    }
}

fn read_csv(path: &str) -> Res<Vec<Row>> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    parse_csv(&text, path)
}

/// The rows of a CSV; a second row with the same key is an error.
fn parse_csv(text: &str, path: &str) -> Res<Vec<Row>> {
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Err(format!("{path}: not a bc7_quality CSV (header differs)").into());
    }
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for (n, line) in lines.enumerate() {
        let at = n + 2;
        let f: Vec<&str> = line.split(',').collect();
        if f.len() != FIELDS {
            return Err(format!("{path}:{at}: expected {FIELDS} fields").into());
        }
        let p = |i: usize| -> Res<u64> {
            f[i].parse()
                .map_err(|e| format!("{path}:{at}: field {} ({}): {e}", i + 1, f[i]).into())
        };
        let row = Row {
            class: f[0].into(),
            image: f[1].into(),
            level: f[2].into(),
            w: p(3)? as u32,
            perceptual: p(4)? as u8,
            exact: p(5)? as u8,
            tuning: f[6].into(),
            target: f[7].into(),
            blocks: p(8)?,
            se: [p(9)?, p(10)?, p(11)?, p(12)?],
            mx: [p(13)? as u32, p(14)? as u32, p(15)? as u32, p(16)? as u32],
            ge32_rgb: p(17)?,
            ge32_a: p(18)?,
            angle: if f[19].is_empty() {
                None
            } else {
                Some(
                    f[19]
                        .parse()
                        .map_err(|e| format!("{path}:{at}: angle: {e}"))?,
                )
            },
            modes: [
                p(20)?,
                p(21)?,
                p(22)?,
                p(23)?,
                p(24)?,
                p(25)?,
                p(26)?,
                p(27)?,
            ],
            seconds: f[28]
                .parse()
                .map_err(|e| format!("{path}:{at}: seconds: {e}"))?,
            fnv: f[29].into(),
        };
        if !seen.insert((row.cell(), row.image.clone())) {
            return Err(format!(
                "{path}:{at}: duplicate row {} {} {} w{} perc {} exact {} {}",
                row.class, row.image, row.level, row.w, row.perceptual, row.exact, row.tuning
            )
            .into());
        }
        rows.push(row);
    }
    Ok(rows)
}

/// `10 log10(65025 / mse)`, 99.0 at mse <= 1e-12 (`engine::validate::psnr_from_mse`).
fn psnr_from_mse(mse: f64) -> f64 {
    if mse <= 1e-12 {
        99.0
    } else {
        10.0 * (65025.0 / mse).log10()
    }
}

/// Metric names in print order; `metric_psnr` gives each one's PSNR for a row.
const METRICS: [&str; 6] = ["R", "G", "B", "A", "RGB", "objective"];

fn metric_psnr(r: &Row, m: usize) -> f64 {
    let t = (r.blocks * 16) as f64;
    let rgb = (r.se[0] + r.se[1] + r.se[2]) as f64;
    let mse = match m {
        0..=3 => r.se[m] as f64 / t,
        4 => rgb / (3.0 * t),
        // Squared error RGB + w x A, per texel and per unit of weight: the normalisation cancels in a paired delta.
        _ => (rgb + r.w as f64 * r.se[3] as f64) / (t * (3.0 + r.w as f64)),
    };
    psnr_from_mse(mse)
}

struct XorShift(u64);

impl XorShift {
    /// xorshift64*.
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// Mean of `d` with a paired percentile bootstrap 95 % interval: `RESAMPLES` resamples of the indices.
fn bootstrap(d: &[f64]) -> (f64, f64, f64) {
    let n = d.len();
    let mean = d.iter().sum::<f64>() / n as f64;
    let mut rng = XorShift(SEED);
    let mut means: Vec<f64> = (0..RESAMPLES)
        .map(|_| {
            let mut s = 0.0;
            for _ in 0..n {
                s += d[((rng.next() >> 11) % n as u64) as usize];
            }
            s / n as f64
        })
        .collect();
    means.sort_by(f64::total_cmp);
    (
        mean,
        means[RESAMPLES * 25 / 1000],
        means[RESAMPLES * 975 / 1000],
    )
}

fn compare_cell(cell: &Cell, pairs: &[(&Row, &Row)]) -> Res<String> {
    let mut s = String::new();
    let (c, l, w, p, e, t) = cell;
    let (b0, a0) = pairs[0];
    writeln!(
        s,
        "== {c} {l} w{w} perceptual {p} exact {e} tuning {t} (target {} -> {}): n = {}",
        b0.target,
        a0.target,
        pairs.len()
    )?;
    for (m, name) in METRICS.iter().enumerate() {
        let d: Vec<f64> = pairs
            .iter()
            .map(|(b, a)| metric_psnr(a, m) - metric_psnr(b, m))
            .collect();
        let (mean, lo, hi) = bootstrap(&d);
        let wins = d.iter().filter(|&&x| x > 0.0).count();
        let losses = d.iter().filter(|&&x| x < 0.0).count();
        let (wi, wd) = d
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, v)| (&pairs[i].0.image, *v))
            .unwrap();
        writeln!(
            s,
            "  {name:<9} mean delta {mean:+.3} dB [{lo:+.3}, {hi:+.3}]  wins {wins} losses {losses}  worst {wi} {wd:+.3}"
        )?;
    }
    if pairs
        .iter()
        .all(|(b, a)| b.angle.is_some() && a.angle.is_some())
    {
        let d: Vec<f64> = pairs
            .iter()
            .map(|(b, a)| a.angle.unwrap() - b.angle.unwrap())
            .collect();
        let (mean, lo, hi) = bootstrap(&d);
        writeln!(
            s,
            "  angle     mean delta {mean:+.4} deg [{lo:+.4}, {hi:+.4}] (+ is worse)"
        )?;
    }
    let top = |side: usize| -> [u32; 4] {
        std::array::from_fn(|c| {
            pairs
                .iter()
                .map(|p| if side == 0 { p.0.mx[c] } else { p.1.mx[c] })
                .max()
                .unwrap_or(0)
        })
    };
    writeln!(
        s,
        "  largest max error r/g/b/a over the images {:?} -> {:?}",
        top(0),
        top(1)
    )?;
    let ge: Vec<String> = pairs
        .iter()
        .filter(|(b, a)| b.ge32_rgb + b.ge32_a + a.ge32_rgb + a.ge32_a > 0)
        .map(|(b, a)| {
            format!(
                "{} {}/{} -> {}/{}",
                b.image, b.ge32_rgb, b.ge32_a, a.ge32_rgb, a.ge32_a
            )
        })
        .collect();
    let tot = |f: &dyn Fn(&Row) -> u64, side: usize| -> u64 {
        pairs
            .iter()
            .map(|p| f(if side == 0 { p.0 } else { p.1 }))
            .sum()
    };
    writeln!(
        s,
        "  ge32 blocks (rgb/alpha) total {}/{} -> {}/{}; per image, nonzero only (before -> after):{}",
        tot(&|r| r.ge32_rgb, 0),
        tot(&|r| r.ge32_a, 0),
        tot(&|r| r.ge32_rgb, 1),
        tot(&|r| r.ge32_a, 1),
        if ge.is_empty() {
            " none".to_string()
        } else {
            format!("\n    {}", ge.join("\n    "))
        }
    )?;
    let mix = |side: usize| -> String {
        (0..8)
            .map(|m| format!("m{m}={}", tot(&|r| r.modes[m], side)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    writeln!(s, "  modes before {}\n  modes after  {}", mix(0), mix(1))?;
    let k = pairs.iter().filter(|(b, a)| b.fnv == a.fnv).count();
    let secs = |side: usize| -> f64 {
        pairs
            .iter()
            .map(|p| if side == 0 { p.0.seconds } else { p.1.seconds })
            .sum()
    };
    writeln!(
        s,
        "  bytes identical: {k}/{}  seconds before {:.2} after {:.2}",
        pairs.len(),
        secs(0),
        secs(1)
    )?;
    Ok(s)
}

fn compare_rows(before: &[Row], after: &[Row], only: Option<&str>) -> Res<String> {
    fn group<'a>(
        rows: &'a [Row],
        only: Option<&str>,
    ) -> BTreeMap<Cell, BTreeMap<&'a str, &'a Row>> {
        let mut m: BTreeMap<Cell, BTreeMap<&str, &Row>> = BTreeMap::new();
        for r in rows.iter().filter(|r| only.is_none_or(|c| c == r.class)) {
            m.entry(r.cell()).or_default().insert(r.image.as_str(), r);
        }
        m
    }
    let (gb, ga) = (group(before, only), group(after, only));
    let mut s = String::new();
    let mut joined = 0;
    for (cell, b) in &gb {
        let Some(a) = ga.get(cell) else {
            writeln!(
                s,
                "-- {} {} w{} only in the first file",
                cell.0, cell.1, cell.2
            )?;
            continue;
        };
        let unpaired: Vec<&str> = b
            .keys()
            .filter(|k| !a.contains_key(*k))
            .chain(a.keys().filter(|k| !b.contains_key(*k)))
            .copied()
            .collect();
        if !unpaired.is_empty() {
            return Err(format!(
                "{} {} w{}: images in only one file: {}",
                cell.0,
                cell.1,
                cell.2,
                unpaired.join(", ")
            )
            .into());
        }
        let pairs: Vec<(&Row, &Row)> = b.iter().map(|(k, r)| (*r, a[k])).collect();
        s += &compare_cell(cell, &pairs)?;
        joined += 1;
    }
    for cell in ga.keys().filter(|c| !gb.contains_key(*c)) {
        writeln!(
            s,
            "-- {} {} w{} only in the second file",
            cell.0, cell.1, cell.2
        )?;
    }
    if joined == 0 {
        return Err("no class and setting is in both files".into());
    }
    Ok(s)
}

fn compare(args: &[String]) -> Res<()> {
    let mut files = Vec::new();
    let mut class = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--class" => class = Some(it.next().ok_or("--class needs a name")?.clone()),
            _ => files.push(a.clone()),
        }
    }
    let [fa, fb] = files.as_slice() else {
        return Err("compare needs <before.csv> <after.csv>".into());
    };
    let (before, after) = (read_csv(fa)?, read_csv(fb)?);
    print!("{}", compare_rows(&before, &after, class.as_deref())?);
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let res = match args.first().map(String::as_str) {
        Some("run") => run(&args[1..]),
        Some("compare") => compare(&args[1..]),
        _ => Err("usage: bc7_quality run --class <name>=<path>[:<flags>] ... --out <csv> [--levels l,..] [--weights w,..] [--perceptual off|on|both] [--exact off|on] [--tuning default,data,x16,data_x16,data_exact_g_x16,data_guard_<letters>] [--target sse2|avx2|avx512] [--threads N] | compare <before.csv> <after.csv> [--class <name>]".into()),
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

    fn row(class: &str, image: &str, se: [u64; 4], fnv: &str) -> String {
        format!(
            "{class},{image},veryslow,1,0,0,default,avx2,100,{},{},{},{},3,3,3,3,0,0,,0,0,0,0,0,0,100,0,1.000,{fnv}\n",
            se[0], se[1], se[2], se[3]
        )
    }

    fn csv(rows: &[(&str, &str, [u64; 4], &str)]) -> Vec<Row> {
        let body: String = rows.iter().map(|r| row(r.0, r.1, r.2, r.3)).collect();
        parse_csv(&format!("{HEADER}\n{body}"), "t.csv").unwrap()
    }

    #[test]
    fn the_header_has_every_field() {
        assert_eq!(HEADER.split(',').count(), FIELDS);
    }

    #[test]
    fn fnv_matches_the_published_vectors() {
        assert_eq!(fnv(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn a_drive_letter_is_not_a_flag_list() {
        let dir = std::env::temp_dir().join("bc7_quality_test_class");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.png"), b"x").unwrap();
        let spec = |flags: &str| format!("c={}{flags}", dir.display());
        let c = parse_class(&spec(":opaque,normal")).unwrap();
        assert!(c.opaque && c.normal && c.files.len() == 1);
        let c = parse_class(&spec("")).unwrap();
        assert!(!c.opaque && !c.normal);
        let e = parse_class("c=").unwrap_err();
        assert!(e.to_string().contains("not a folder"), "{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_list_checks_its_sha_and_a_missing_file_is_an_error() {
        let dir = std::env::temp_dir().join("bc7_quality_test_list");
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .save(&png)
            .unwrap();
        let good = hex::encode(Sha256::digest(std::fs::read(&png).unwrap()));
        assert!(load(&png, Some(&good), false).is_ok());
        let e = load(&png, Some(&"0".repeat(64)), false).err().unwrap();
        assert!(e.to_string().contains("SHA-256 is"), "{e}");
        let e = load(&dir.join("missing.png"), None, false).err().unwrap();
        assert!(e.to_string().contains("missing.png"), "{e}");
        let l = parse_list(
            &format!("{}\t{good}\r\n\n{}\n", png.display(), png.display()),
            &png,
        )
        .unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].1.as_deref(), Some(good.as_str()));
        assert!(l[1].1.is_none());
        assert!(parse_list("a.png\tnot-a-hash\n", &png).is_err());
        let opaque = load(&png, None, true).unwrap();
        assert_eq!(opaque.px[0], [1, 2, 3, 255]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_threads_give_the_serial_result() {
        // 256 blocks: four chunks of 64, spread over the threads and joined in chunk order.
        let px: Vec<[u8; 4]> = (0..64u32 * 64)
            .map(|i| {
                let v = i.wrapping_mul(2_654_435_761);
                [v as u8, (v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8]
            })
            .collect();
        let blocks = gather_blocks(&px, 64, 64);
        let s = Setting {
            level: "basic",
            lv: Bc7Level::Basic,
            w: 16,
            perceptual: false,
            tuning: "default",
            scale: 1,
            flat: None,
            exact_mask: None,
        };
        let (st, bytes) = encode_image(&blocks, s, false, IspcTarget::Sse2, true);
        let (mut serial, mut serial_bytes) = (Stats::default(), vec![0u8; blocks.len()]);
        for (src, out) in blocks
            .chunks(CALL_BLOCKS * 16)
            .zip(serial_bytes.chunks_mut(CALL_BLOCKS * 16))
        {
            let p = process_chunk(src, out, s, false, IspcTarget::Sse2, true);
            serial.add(&p);
        }
        assert_eq!(bytes, serial_bytes);
        assert_eq!(st.se, serial.se);
        assert_eq!(st.mx, serial.mx);
        assert_eq!(st.angle, serial.angle);
        assert_eq!(st.modes, serial.modes);
        assert_eq!(st.modes.iter().sum::<u64>(), 256);
    }

    /// Task 6's rule: weights `[s, s, s, s * w]`, the tolerance and the exact-constants mask with perceptual off, the
    /// tolerance alone with it on. Since L16 every value's scale is 1, so the `x16` rows send what `default`'s and
    /// `data`'s send (Note LR, LR15).
    #[test]
    fn a_tuning_row_sends_what_the_rule_says() {
        let rows = settings(
            &["veryslow"],
            &[1, 16],
            "both",
            &["default", "data", "x16", "data_x16", "data_exact_g_x16"],
        )
        .unwrap();
        let got: Vec<(&str, u32, bool, Option<Bc7Tune>)> = rows
            .iter()
            .map(|s| (s.tuning, s.w, s.perceptual, tune_of(s)))
            .collect();
        let tune_x = |weights, flat, exact_constants| {
            Some(Bc7Tune {
                weights,
                flat_tolerance: flat,
                exact_constants,
            })
        };
        let tune = |weights, flat| tune_x(weights, flat, None);
        let data = Some(Bc7FlatTolerance::Data);
        let g = Some(0b010);
        assert_eq!(
            got,
            vec![
                ("default", 1, false, None),
                ("default", 16, false, tune(Some([1, 1, 1, 16]), None)),
                ("default", 1, true, None),
                ("data", 1, false, tune(None, data)),
                ("data", 16, false, tune(Some([1, 1, 1, 16]), data)),
                ("data", 1, true, tune(None, data)),
                ("x16", 1, false, None),
                ("x16", 16, false, tune(Some([1, 1, 1, 16]), None)),
                ("x16", 1, true, None),
                ("data_x16", 1, false, tune(None, data)),
                ("data_x16", 16, false, tune(Some([1, 1, 1, 16]), data)),
                ("data_x16", 1, true, tune(None, data)),
                ("data_exact_g_x16", 1, false, tune_x(None, data, g)),
                (
                    "data_exact_g_x16",
                    16,
                    false,
                    tune_x(Some([1, 1, 1, 16]), data, g),
                ),
                ("data_exact_g_x16", 1, true, tune(None, data)),
            ]
        );
        assert!(parse_tuning("exact").is_err());
        // The channel guard rows: data_x16 with the letters' mask, in R, G, B order.
        let guards = settings(
            &["veryslow"],
            &[1],
            "both",
            &["data_guard_rg", "data_guard_gb", "data_guard_rgb"],
        )
        .unwrap();
        let got: Vec<(&str, bool, Option<Bc7Tune>)> = guards
            .iter()
            .map(|s| (s.tuning, s.perceptual, tune_of(s)))
            .collect();
        assert_eq!(
            got,
            vec![
                ("data_guard_rg", false, tune_x(None, data, Some(0b011))),
                ("data_guard_rg", true, tune(None, data)),
                ("data_guard_gb", false, tune_x(None, data, Some(0b110))),
                ("data_guard_gb", true, tune(None, data)),
                ("data_guard_rgb", false, tune_x(None, data, Some(0b111))),
                ("data_guard_rgb", true, tune(None, data)),
            ]
        );
        let (g, xg) = (
            parse_tuning("data_guard_g").unwrap(),
            parse_tuning("data_exact_g_x16").unwrap(),
        );
        assert_eq!((g.1, g.2, g.3), (xg.1, xg.2, xg.3));
        assert!(parse_tuning("data_guard_gr").is_err());
    }

    #[test]
    fn the_bootstrap_is_seeded_and_brackets_the_mean() {
        let d = [0.1, 0.2, -0.05, 0.3, 0.15];
        let a = bootstrap(&d);
        assert_eq!(a, bootstrap(&d));
        assert!(a.1 <= a.0 && a.0 <= a.2, "{a:?}");
        let (m, lo, hi) = bootstrap(&[0.5; 7]);
        assert_eq!((m, lo, hi), (0.5, 0.5, 0.5));
    }

    #[test]
    fn psnr_floors_at_99() {
        assert_eq!(psnr_from_mse(0.0), 99.0);
        assert!((psnr_from_mse(65025.0)).abs() < 1e-9);
    }

    #[test]
    fn a_duplicate_key_is_an_error() {
        let t = format!(
            "{HEADER}\n{}{}",
            row("c", "a", [1, 1, 1, 1], "aa"),
            row("c", "a", [1, 1, 1, 1], "bb")
        );
        let e = parse_csv(&t, "t.csv").unwrap_err();
        assert!(e.to_string().contains("duplicate row c a veryslow"), "{e}");
    }

    #[test]
    fn an_unpaired_image_is_an_error_naming_it() {
        let a = csv(&[("c", "a", [4; 4], "x"), ("c", "b", [4; 4], "x")]);
        let b = csv(&[("c", "a", [4; 4], "x")]);
        let e = compare_rows(&a, &b, None).unwrap_err();
        assert!(e.to_string().contains("only one file: b"), "{e}");
    }

    #[test]
    fn a_quarter_of_the_error_is_six_db_better() {
        let a = csv(&[("c", "a", [400; 4], "x"), ("c", "b", [400; 4], "y")]);
        let b = csv(&[("c", "a", [100; 4], "x"), ("c", "b", [100; 4], "z")]);
        let s = compare_rows(&a, &b, None).unwrap();
        assert!(s.contains("n = 2"), "{s}");
        assert!(s.contains("R         mean delta +6.021 dB"), "{s}");
        assert!(s.contains("wins 2 losses 0"), "{s}");
        assert!(s.contains("bytes identical: 1/2"), "{s}");
        let none = compare_rows(&a, &b, Some("other")).unwrap_err();
        assert!(none.to_string().contains("in both files"), "{none}");
    }
}
