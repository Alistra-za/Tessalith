//! Speed and output hash of forced ISPC paths: the AVX2 path against the CPU's best path, or any list of paths.
//!
//!   cargo run --offline --release -p encoders-sys --example ispc_speed -- <list file> [--all | --hdr] [--rounds N]
//!       [--targets <name>,<name>,...]
//!
//! The list file names one file per line (PNGs; with `--hdr`, HDR files). Each configuration encodes every 4x4 block
//! of every file with the calls the engine makes (bc7e: 64 blocks per call, one block row at a time; BC6H: one 4-row
//! strip per call), once per target per round, one target after another, and prints the median time per block and a
//! 64-bit FNV-1a hash of all output bytes (docs/superpowers/plans/2026-11-13-avx512-ispc-survey.md, section 8, "Hash
//! recipe"). Only encode calls are timed. All encodes run on the calling thread.
//!
//! Without `--targets` the line compares AVX2 with the best path (`avx2=<us> <hash> best=<name> <us> <hash> ratio=`).
//! With `--targets` each listed path is forced in list order (the `*_on` calls with an `IspcTarget`), and the line is
//! `ISPC <name> blocks=<n>`, then ` <target>=<us> <hash>` per target, then ` <target>/<first>=<ratio>` (median time
//! over the first target's) for every target after the first. Example: `--targets sse2,avx2`.
//!
//! `--hdr` reads linear float files (anything `image::open` reads as float), keeps values above 1.0, clamps as the
//! engine's `half_bits` does (R, G, B to [0, 65504] as half bits, alpha 1) and runs the six BC6H levels (no BC7);
//! it does not combine with `--all`.
use encoders_sys::bc7_fixup::force_opaque_rotated;
use encoders_sys::{
    bc6h_compress_rows_on, bc7e_compress_blocks_on, gather_blocks, Bc6hLevel, Bc7Level, Bc7Params,
    IspcTarget,
};
use std::error::Error;
use std::path::Path;
use std::time::Instant;

struct Img {
    w: u32,
    h: u32,
    px: Vec<[u8; 4]>,
}

#[derive(Clone, Copy)]
enum Kind {
    Bc7(Bc7Params),
    Bc6h(Bc6hLevel),
}

/// Prepared input of one configuration: BC7 gathered blocks (alpha 255) with the blocks per row, or BC6H half texels
/// with the width.
enum Prepared {
    Bc7(Vec<(Vec<[u8; 4]>, usize)>),
    Bc6h(Vec<(Vec<[u16; 4]>, u32)>),
}

fn bc7(level: Bc7Level, perceptual: bool, exact_opaque: bool) -> Kind {
    Kind::Bc7(Bc7Params {
        level,
        perceptual,
        exact_opaque,
    })
}

fn configs(all: bool) -> Vec<(&'static str, Kind)> {
    let mut v = vec![
        ("bc7_veryslow_lin", bc7(Bc7Level::Veryslow, false, false)),
        ("bc7_fast_lin", bc7(Bc7Level::Fast, false, false)),
        ("bc6h_veryslowpeak", Kind::Bc6h(Bc6hLevel::VeryslowPeak)),
        ("bc6h_fast", Kind::Bc6h(Bc6hLevel::Fast)),
    ];
    if all {
        v.extend([
            ("bc7_veryslow_lin_xo1", bc7(Bc7Level::Veryslow, false, true)),
            ("bc7_veryslow_perc", bc7(Bc7Level::Veryslow, true, false)),
            ("bc7_max_lin", bc7(Bc7Level::Max, false, false)),
            ("bc6h_veryslow", Kind::Bc6h(Bc6hLevel::Veryslow)),
        ]);
    }
    v
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

fn prepare(imgs: &[Img], kind: Kind) -> Prepared {
    match kind {
        Kind::Bc7(_) => Prepared::Bc7(
            imgs.iter()
                .map(|im| {
                    let opaque: Vec<[u8; 4]> =
                        im.px.iter().map(|p| [p[0], p[1], p[2], 255]).collect();
                    (
                        gather_blocks(&opaque, im.w, im.h),
                        im.w.div_ceil(4) as usize,
                    )
                })
                .collect(),
        ),
        Kind::Bc6h(_) => {
            let one = half::f16::ONE.to_bits();
            let h = |k: u8| half::f16::from_f32(srgb_to_linear(k as f32 / 255.0)).to_bits();
            Prepared::Bc6h(
                imgs.iter()
                    .map(|im| {
                        (
                            im.px
                                .iter()
                                .map(|p| [h(p[0]), h(p[1]), h(p[2]), one])
                                .collect(),
                            im.w,
                        )
                    })
                    .collect(),
            )
        }
    }
}

/// One image on one target: the seconds spent in encoder calls and the final bytes (BC7 with the rotated-slot fix on
/// all-opaque blocks, outside the timed part).
fn encode_one(prep: &Prepared, i: usize, kind: Kind, target: IspcTarget) -> (f64, Vec<u8>) {
    match (prep, kind) {
        (Prepared::Bc7(v), Kind::Bc7(p)) => {
            let (blocks, bw) = &v[i];
            let mut out = vec![0u8; blocks.len()];
            let t = Instant::now();
            for (o, b) in out.chunks_mut(bw * 16).zip(blocks.chunks(bw * 16)) {
                for (oc, bc) in o.chunks_mut(64 * 16).zip(b.chunks(64 * 16)) {
                    bc7e_compress_blocks_on(bc, oc, p, target);
                }
            }
            let s = t.elapsed().as_secs_f64();
            for (blk, src) in out.chunks_mut(16).zip(blocks.chunks(16)) {
                if src.iter().all(|t| t[3] == 255) {
                    force_opaque_rotated(blk.try_into().unwrap());
                }
            }
            (s, out)
        }
        (Prepared::Bc6h(v), Kind::Bc6h(level)) => {
            let (px, w) = &v[i];
            let mut out = vec![0u8; px.len()];
            let t = Instant::now();
            for (o, rows) in out
                .chunks_mut((w / 4 * 16) as usize)
                .zip(px.chunks((w * 4) as usize))
            {
                bc6h_compress_rows_on(rows, *w, 4, o, level, target);
            }
            (t.elapsed().as_secs_f64(), out)
        }
        _ => unreachable!(),
    }
}

/// All images on one target: (time per block in microseconds, block count, hash).
fn run(prep: &Prepared, n: usize, kind: Kind, target: IspcTarget) -> (f64, usize, u64) {
    let (mut secs, mut blocks, mut hash) = (0.0, 0usize, 0xcbf2_9ce4_8422_2325u64);
    for i in 0..n {
        let (s, out) = encode_one(prep, i, kind, target);
        secs += s;
        blocks += out.len() / 16;
        fnv(&mut hash, &out);
    }
    (secs * 1e6 / blocks as f64, blocks, hash)
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

/// Block count and, per target in list order, (median microseconds per block, hash). Warm-up: image 0 once per
/// target, untimed. Each round runs every target once, in list order, on this thread; a target's hash must equal its
/// round-0 hash.
fn measure(
    prep: &Prepared,
    n: usize,
    kind: Kind,
    targets: &[IspcTarget],
    rounds: usize,
    name: &str,
) -> Result<(usize, Vec<(f64, u64)>), String> {
    for &t in targets {
        let _ = encode_one(prep, 0, kind, t);
    }
    let mut times = vec![Vec::new(); targets.len()];
    let mut first = Vec::new();
    for round in 0..rounds {
        for (k, &t) in targets.iter().enumerate() {
            let (us, blocks, h) = run(prep, n, kind, t);
            times[k].push(us);
            if round == 0 {
                first.push((blocks, h));
            } else if first[k].1 != h {
                return Err(format!("{name}: round {round} hashes differ from round 0"));
            }
        }
    }
    let blocks = first[0].0;
    let out = times
        .iter_mut()
        .zip(&first)
        .map(|(t, (_, h))| (median(t), *h))
        .collect();
    Ok((blocks, out))
}

/// The targets named in `s`, in list order.
fn parse_targets(s: &str) -> Result<Vec<IspcTarget>, String> {
    let mut out: Vec<IspcTarget> = Vec::new();
    for item in s.split(',') {
        if item.is_empty() {
            return Err("empty ISPC target in list".to_string());
        }
        let t = IspcTarget::ALL
            .into_iter()
            .find(|t| t.name() == item)
            .ok_or_else(|| {
                let known: Vec<&str> = IspcTarget::ALL.iter().map(|t| t.name()).collect();
                format!("unknown ISPC target \"{item}\"; known: {}", known.join(","))
            })?;
        if out.contains(&t) {
            return Err(format!("ISPC target \"{item}\" listed twice"));
        }
        out.push(t);
    }
    Ok(out)
}

/// One HDR texel as BC6H input: linear R, G, B as read, alpha 1. Converted as `engine::quantize::half_bits` does
/// (the example cannot depend on `engine`): clamped to [0, 65504] (NaN, -0.0 and negatives give 0), then half bits.
/// No infinity or NaN reaches the kernel, which is built with `--opt=fast-math`.
fn hdr_texel(p: [f32; 4]) -> [u16; 4] {
    let h = |v: f32| half::f16::from_f32(if v > 0.0 { v.min(65504.0) } else { 0.0 }).to_bits();
    [h(p[0]), h(p[1]), h(p[2]), half::f16::ONE.to_bits()]
}

fn hdr_configs() -> Vec<(&'static str, Kind)> {
    vec![
        ("bc6h_veryfast", Kind::Bc6h(Bc6hLevel::Veryfast)),
        ("bc6h_fast", Kind::Bc6h(Bc6hLevel::Fast)),
        ("bc6h_basic", Kind::Bc6h(Bc6hLevel::Basic)),
        ("bc6h_slow", Kind::Bc6h(Bc6hLevel::Slow)),
        ("bc6h_veryslow", Kind::Bc6h(Bc6hLevel::Veryslow)),
        ("bc6h_veryslowpeak", Kind::Bc6h(Bc6hLevel::VeryslowPeak)),
    ]
}

fn list_paths(list: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let text = std::fs::read_to_string(list)?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

fn check_size(p: &str, w: u32, h: u32) -> Result<(), Box<dyn Error>> {
    if !w.is_multiple_of(4) || !h.is_multiple_of(4) {
        return Err(format!("{p}: size {w}x{h} is not a multiple of 4").into());
    }
    Ok(())
}

fn load(paths: &[String]) -> Result<Vec<Img>, Box<dyn Error>> {
    let mut imgs = Vec::new();
    for p in paths {
        let im = image::open(p).map_err(|e| format!("{p}: {e}"))?.to_rgba8();
        let (w, h) = im.dimensions();
        check_size(p, w, h)?;
        imgs.push(Img {
            w,
            h,
            px: im.pixels().map(|p| p.0).collect(),
        });
    }
    Ok(imgs)
}

/// HDR files straight into BC6H input (linear float, see `hdr_texel`).
fn load_hdr(paths: &[String]) -> Result<Prepared, Box<dyn Error>> {
    let mut v = Vec::new();
    for p in paths {
        let im = image::open(p)
            .map_err(|e| format!("{p}: {e}"))?
            .to_rgba32f();
        let (w, h) = im.dimensions();
        check_size(p, w, h)?;
        v.push((im.pixels().map(|p| hdr_texel(p.0)).collect(), w));
    }
    Ok(Prepared::Bc6h(v))
}

const USAGE: &str =
    "usage: ispc_speed <list file> [--all | --hdr] [--rounds N] [--targets <name>,...]";

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let list = args.first().filter(|a| !a.starts_with("--")).ok_or(USAGE)?;
    let (mut all, mut hdr, mut rounds, mut targets) = (false, false, 1usize, None);
    let mut rest = args[1..].iter();
    while let Some(a) = rest.next() {
        match a.as_str() {
            "--all" => all = true,
            "--hdr" => hdr = true,
            "--rounds" => rounds = rest.next().ok_or("--rounds needs a number")?.parse()?,
            "--targets" => {
                targets = Some(parse_targets(
                    rest.next().ok_or("--targets needs a list of names")?,
                )?)
            }
            other => return Err(format!("unknown argument {other:?}").into()),
        }
    }
    if hdr && all {
        return Err("--hdr and --all do not combine".into());
    }
    let best = IspcTarget::best();
    match &targets {
        Some(t) => {
            if let Some(t) = t.iter().find(|t| !t.supported()) {
                return Err(format!("this CPU cannot run the {} ISPC path", t.name()).into());
            }
        }
        None => {
            if !IspcTarget::Avx2.supported() {
                println!("ISPC: this CPU has no AVX2; nothing to compare");
                return Ok(());
            }
        }
    }
    if rounds == 0 {
        return Err("--rounds must be at least 1".into());
    }
    let paths = list_paths(Path::new(list))?;
    if paths.is_empty() {
        return Err(format!("{list}: no images listed").into());
    }
    let imgs = if hdr { Vec::new() } else { load(&paths)? };
    let hdr_prep = if hdr { Some(load_hdr(&paths)?) } else { None };
    let configs = if hdr { hdr_configs() } else { configs(all) };
    let run_set = targets
        .clone()
        .unwrap_or_else(|| vec![IspcTarget::Avx2, best]);
    for (name, kind) in configs {
        let owned;
        let prep = match &hdr_prep {
            Some(p) => p,
            None => {
                owned = prepare(&imgs, kind);
                &owned
            }
        };
        let (blocks, res) = measure(prep, paths.len(), kind, &run_set, rounds, name)?;
        if targets.is_some() {
            let mut line = format!("ISPC {name} blocks={blocks}");
            for (t, (us, h)) in run_set.iter().zip(&res) {
                line += &format!(" {}={us:.4} {h:016x}", t.name());
            }
            for (t, (us, _)) in run_set.iter().zip(&res).skip(1) {
                line += &format!(" {}/{}={:.3}", t.name(), run_set[0].name(), us / res[0].0);
            }
            println!("{line}");
        } else {
            let ((a, ha), (b, hb)) = (res[0], res[1]);
            println!(
                "ISPC {name} blocks={blocks} avx2={a:.4} {ha:016x} best={} {b:.4} {hb:016x} ratio={:.3}",
                best.name(),
                b / a
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_parse_in_list_order() {
        assert_eq!(
            parse_targets("avx2,sse2").unwrap(),
            [IspcTarget::Avx2, IspcTarget::Sse2]
        );
        assert_eq!(parse_targets("avx512").unwrap(), [IspcTarget::Avx512]);
    }

    #[test]
    fn bad_target_lists_are_errors() {
        assert!(parse_targets("sse9")
            .unwrap_err()
            .contains("unknown ISPC target \"sse9\""));
        assert!(parse_targets("sse2,sse2")
            .unwrap_err()
            .contains("listed twice"));
        for list in ["", "sse2,", ",sse2"] {
            assert!(parse_targets(list)
                .unwrap_err()
                .contains("empty ISPC target"));
        }
    }

    #[test]
    fn the_dropped_names_are_unknown() {
        for name in ["sse4", "avx"] {
            assert!(parse_targets(name)
                .unwrap_err()
                .contains("unknown ISPC target"));
        }
    }

    #[test]
    fn hdr_texels_keep_values_above_one() {
        let bits = |v: f32| half::f16::from_f32(v).to_bits();
        assert_eq!(
            hdr_texel([2.5, 0.0, 100.0, 0.25]),
            [bits(2.5), bits(0.0), bits(100.0), half::f16::ONE.to_bits()]
        );
        // As engine::quantize::half_bits: clamped to the finite half range, no infinity, no NaN, no negative.
        assert_eq!(hdr_texel([1e9, f32::NAN, -3.0, 1.0])[..3], [0x7BFF, 0, 0]);
        assert_eq!(hdr_texel([-0.0, 1.0, 1.0, 1.0])[0], 0);
    }

    #[test]
    fn hdr_mode_runs_the_six_bc6h_levels() {
        let names: Vec<&str> = hdr_configs().iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            [
                "bc6h_veryfast",
                "bc6h_fast",
                "bc6h_basic",
                "bc6h_slow",
                "bc6h_veryslow",
                "bc6h_veryslowpeak"
            ]
        );
        assert!(hdr_configs()
            .iter()
            .all(|(_, k)| matches!(k, Kind::Bc6h(_))));
    }
}
