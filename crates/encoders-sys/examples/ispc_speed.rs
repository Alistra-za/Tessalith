//! Speed and output hash of the AVX2 ISPC path against the CPU's best ISPC path.
//!
//!   cargo run --offline --release -p encoders-sys --example ispc_speed -- <list file> [--all] [--rounds N]
//!
//! The list file names one PNG per line. Each configuration encodes every 4x4 block of every file with the calls the
//! engine makes (bc7e: 64 blocks per call, one block row at a time; BC6H: one 4-row strip per call), once per target
//! per round, alternating the two, and prints the median time per block and a 64-bit FNV-1a hash of all output bytes
//! (docs/superpowers/plans/2026-11-13-avx512-ispc-survey.md, section 8, "Hash recipe"). Only encode calls are timed.
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

fn load(list: &Path) -> Result<Vec<Img>, Box<dyn Error>> {
    let text = std::fs::read_to_string(list)?;
    let mut imgs = Vec::new();
    for p in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let im = image::open(p).map_err(|e| format!("{p}: {e}"))?.to_rgba8();
        let (w, h) = im.dimensions();
        if w % 4 != 0 || h % 4 != 0 {
            return Err(format!("{p}: size {w}x{h} is not a multiple of 4").into());
        }
        imgs.push(Img {
            w,
            h,
            px: im.pixels().map(|p| p.0).collect(),
        });
    }
    Ok(imgs)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let list = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .ok_or("usage: ispc_speed <list file> [--all] [--rounds N]")?;
    let all = args.iter().any(|a| a == "--all");
    let rounds: usize = match args.iter().position(|a| a == "--rounds") {
        Some(i) => args.get(i + 1).ok_or("--rounds needs a number")?.parse()?,
        None => 1,
    };
    if !IspcTarget::Avx2.supported() {
        println!("ISPC: this CPU has no AVX2; nothing to compare");
        return Ok(());
    }
    let best = IspcTarget::best();
    if rounds == 0 {
        return Err("--rounds must be at least 1".into());
    }
    let imgs = load(Path::new(list))?;
    if imgs.is_empty() {
        return Err(format!("{list}: no images listed").into());
    }
    for (name, kind) in configs(all) {
        let prep = prepare(&imgs, kind);
        // Warm-up: the first image once on each target, untimed.
        let _ = encode_one(&prep, 0, kind, IspcTarget::Avx2);
        let _ = encode_one(&prep, 0, kind, best);
        let (mut ta, mut tb) = (Vec::new(), Vec::new());
        let (mut blocks, mut ha, mut hb) = (0, 0, 0);
        for round in 0..rounds {
            let (t, n, h1) = run(&prep, imgs.len(), kind, IspcTarget::Avx2);
            ta.push(t);
            let (t, _, h2) = run(&prep, imgs.len(), kind, best);
            tb.push(t);
            if round == 0 {
                (blocks, ha, hb) = (n, h1, h2);
            } else if (h1, h2) != (ha, hb) {
                return Err(format!("{name}: round {round} hashes differ from round 0").into());
            }
        }
        let (a, b) = (median(&mut ta), median(&mut tb));
        println!(
            "ISPC {name} blocks={blocks} avx2={a:.4} {ha:016x} best={} {b:.4} {hb:016x} ratio={:.3}",
            best.name(),
            b / a
        );
    }
    Ok(())
}
