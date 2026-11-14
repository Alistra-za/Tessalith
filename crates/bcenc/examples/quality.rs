//! BC1 quality of encoder variants over a folder of PNGs, scored on both vendor decoders.
//!
//! ```text
//! cargo run --offline --release -p bcenc --example quality -- --images <dir> --variants <v,...>
//!     [--ref <name>=<dds dir>]... [--baseline <variant>] [--csv <file>]
//! ```
//!
//! Definitions (those of `research/bc1-dual:crates/bench/src/bc1dual.rs`, which this tool reimplements so that
//! nothing in the repo depends on the data):
//!
//! - Blocks: `encoders_sys::gather_blocks` of `image::open(..).to_rgba8()`. Each block is encoded by the variant and
//!   decoded by `bc1_decode_block` under `Bc1Model::Nvidia` and `Bc1Model::Amd`.
//! - Error: the squared R, G, B error over the image's own `w x h` texels (the padding texels of an edge block are not
//!   counted); `mse = sse / (w * h * 3)`; PSNR `10 log10(65025 / mse)`, 99 dB when `mse <= 1e-12` (the definition of
//!   `engine::validate::psnr`; the example cannot depend on `engine`).
//! - A variant's mean is the mean of the per-image dB; the pooled figure is the PSNR of the summed squared error over
//!   all images and is printed on a second line.
//! - Paired delta against `--baseline`: the per-image dB difference, its mean and the 95% Student-t half-width
//!   (`t95`, `mean_ci` as in `bc1dual.rs`), and the images where the variant wins and loses.
//! - `--ref <name>=<dir>`: `<dir>` holds `<stem>.dds` (BC1, `DXT1` or `DX10` with DXGI 71 or 72) per PNG, scored like a
//!   variant under the name `<name>`. `beats <name>` is `yes` when the variant's unrounded mean is strictly above the
//!   reference's on both decoders.
//!
//! Variants: `cpp:<level>`, rgbcx's BC1 encoder at that level as the engine calls it (`allow_3color`, not perceptual).
//! Images are encoded on `available_parallelism()` threads; `seconds` is the wall time of one image's encode on its
//! thread, so it is rough when other jobs run.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use encoders_sys::{
    bc1_decode_block, bc1_encode_block_level, gather_blocks, Bc1Model, RGBCX_MAX_LEVEL,
};

type Block = [[u8; 4]; 16];
type Encoder = Box<dyn Fn(&Block) -> [u8; 8] + Sync>;

/// The encoder a variant name stands for, or `None` for an unknown name.
fn encoder(name: &str) -> Option<Encoder> {
    if let Some(level) = name.strip_prefix("cpp:") {
        let level: u32 = level.parse().ok().filter(|l| *l <= RGBCX_MAX_LEVEL)?;
        return Some(Box::new(move |px| {
            bc1_encode_block_level(px, level, true, false)
        }));
    }
    None
}

const KNOWN: &str = "cpp:<level 0..=18>";

/// PSNR in dB for a mean squared error over 8-bit values (99 dB when it is zero), as `engine::validate::psnr_from_mse`.
fn psnr_from_mse(mse: f64) -> f64 {
    if mse <= 1e-12 {
        99.0
    } else {
        10.0 * (65025.0 / mse).log10()
    }
}

/// Two-sided 95% Student-t critical value for `df` degrees of freedom (as `bc1dual.rs`).
fn t95(df: usize) -> f64 {
    const T: [f64; 30] = [
        12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160,
        2.145, 2.131, 2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056,
        2.052, 2.048, 2.045, 2.042,
    ];
    match df {
        0 => f64::NAN,
        1..=30 => T[df - 1],
        // Cornish-Fisher expansion of the t quantile at z = 1.96.
        _ => {
            let (z, d) = (1.96f64, df as f64);
            z + (z.powi(3) + z) / (4.0 * d)
                + (5.0 * z.powi(5) + 16.0 * z.powi(3) + 3.0 * z) / (96.0 * d * d)
        }
    }
}

/// (mean, 95% half-width) of a sample.
fn mean_ci(v: &[f64]) -> (f64, f64) {
    let n = v.len();
    let m = v.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return (m, f64::NAN);
    }
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64;
    (m, t95(n - 1) * (var / n as f64).sqrt())
}

struct Image {
    stem: String,
    w: usize,
    h: usize,
    /// The source texels, row-major, `w * h`.
    px: Vec<[u8; 4]>,
    blocks: Vec<Block>,
}

/// The source texels of `blocks[i]` are cut by `gather_blocks`; `flat` is its flat output of 16 texels per block.
fn to_blocks(flat: &[[u8; 4]]) -> Vec<Block> {
    flat.as_chunks::<16>().0.to_vec()
}

/// Squared R, G, B error of the decoded blocks against the source, over the image's own texels only.
fn sse(img: &Image, encoded: &[[u8; 8]], model: Bc1Model) -> f64 {
    let bw = img.w.div_ceil(4);
    let mut total = 0.0f64;
    for (i, blk) in encoded.iter().enumerate() {
        let dec = bc1_decode_block(blk, model);
        let (bx, by) = (i % bw, i / bw);
        for (t, d) in dec.iter().enumerate() {
            let (x, y) = (bx * 4 + t % 4, by * 4 + t / 4);
            if x >= img.w || y >= img.h {
                continue;
            }
            let s = img.px[y * img.w + x];
            for c in 0..3 {
                let e = f64::from(s[c]) - f64::from(d[c]);
                total += e * e;
            }
        }
    }
    total
}

/// The 8-byte BC1 blocks of a DDS file, checked against the PNG's size.
fn read_dds(path: &Path, w: usize, h: usize) -> Result<Vec<[u8; 8]>, String> {
    let name = path.display();
    let b = std::fs::read(path).map_err(|e| format!("{name}: {e}"))?;
    let u32_at = |o: usize| -> Option<u32> {
        b.get(o..o + 4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    };
    if b.get(..4) != Some(b"DDS ") {
        return Err(format!("{name}: not a DDS file"));
    }
    let (dh, dw) = (u32_at(12), u32_at(16));
    if (dw, dh) != (Some(w as u32), Some(h as u32)) {
        return Err(format!(
            "{name}: size {:?}x{:?} differs from the PNG's {w}x{h}",
            dw, dh
        ));
    }
    let four_cc = b.get(84..88).unwrap_or_default();
    let start = match four_cc {
        b"DXT1" => 128,
        b"DX10" => match u32_at(128) {
            Some(71) | Some(72) => 148,
            other => return Err(format!("{name}: DXGI format {other:?} is not BC1")),
        },
        _ => return Err(format!("{name}: FourCC {four_cc:?} is not BC1")),
    };
    let n = w.div_ceil(4) * h.div_ceil(4);
    let data = b
        .get(start..start + n * 8)
        .ok_or_else(|| format!("{name}: data is shorter than {n} blocks"))?;
    Ok(data.as_chunks::<8>().0.to_vec())
}

/// One variant's (or reference's) result for one image.
#[derive(Clone, Copy, Default)]
struct Cell {
    sse: [f64; 2],
    seconds: f64,
}

struct Args {
    images: PathBuf,
    variants: Vec<String>,
    refs: Vec<(String, PathBuf)>,
    baseline: Option<String>,
    csv: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let (mut images, mut variants, mut refs, mut baseline, mut csv) =
        (None, Vec::new(), Vec::new(), None, None);
    while let Some(a) = it.next() {
        let mut val = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--images" => images = Some(PathBuf::from(val()?)),
            "--variants" => variants = val()?.split(',').map(|s| s.trim().to_string()).collect(),
            "--ref" => {
                let v = val()?;
                let (n, d) = v
                    .split_once('=')
                    .ok_or_else(|| format!("--ref {v}: expected <name>=<dir>"))?;
                refs.push((n.to_string(), PathBuf::from(d)));
            }
            "--baseline" => baseline = Some(val()?),
            "--csv" => csv = Some(PathBuf::from(val()?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let images = images.ok_or("--images <dir> is required")?;
    if variants.is_empty() {
        return Err("--variants <v,...> is required".into());
    }
    Ok(Args {
        images,
        variants,
        refs,
        baseline,
        csv,
    })
}

fn load_images(dir: &Path) -> Result<Vec<Image>, String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| {
            let rgba = image::open(p)
                .map_err(|e| format!("{}: {e}", p.display()))?
                .to_rgba8();
            let (w, h) = (rgba.width() as usize, rgba.height() as usize);
            let px: Vec<[u8; 4]> = rgba.pixels().map(|p| p.0).collect();
            let blocks = to_blocks(&gather_blocks(&px, w as u32, h as u32));
            Ok(Image {
                stem: p.file_stem().unwrap_or_default().to_string_lossy().into(),
                w,
                h,
                px,
                blocks,
            })
        })
        .collect()
}

fn run(args: &Args) -> Result<(), String> {
    let encoders: Vec<Encoder> = args
        .variants
        .iter()
        .map(|v| encoder(v).ok_or_else(|| format!("unknown variant {v}; known: {KNOWN}")))
        .collect::<Result<_, _>>()?;
    if let Some(b) = &args.baseline {
        if !args.variants.contains(b) {
            return Err(format!("--baseline {b} is not among the variants"));
        }
    }
    let images = load_images(&args.images)?;
    if images.is_empty() {
        return Err(format!("no PNG in {}", args.images.display()));
    }
    // Columns: the variants, then the references.
    let names: Vec<String> = args
        .variants
        .iter()
        .cloned()
        .chain(args.refs.iter().map(|(n, _)| n.clone()))
        .collect();
    let cols = names.len();
    let results: Mutex<Vec<Option<Vec<Cell>>>> = Mutex::new(vec![None; images.len()]);
    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(img) = images.get(i) else { break };
                let mut row = vec![Cell::default(); cols];
                for (c, enc) in encoders.iter().enumerate() {
                    let t = Instant::now();
                    let out: Vec<[u8; 8]> = img.blocks.iter().map(enc).collect();
                    row[c].seconds = t.elapsed().as_secs_f64();
                    row[c].sse = [
                        sse(img, &out, Bc1Model::Nvidia),
                        sse(img, &out, Bc1Model::Amd),
                    ];
                }
                for (r, (_, dir)) in args.refs.iter().enumerate() {
                    let path = dir.join(format!("{}.dds", img.stem));
                    match read_dds(&path, img.w, img.h) {
                        Ok(out) => {
                            row[encoders.len() + r].sse = [
                                sse(img, &out, Bc1Model::Nvidia),
                                sse(img, &out, Bc1Model::Amd),
                            ]
                        }
                        Err(e) => errors.lock().expect("lock").push(e),
                    }
                }
                results.lock().expect("lock")[i] = Some(row);
            });
        }
    });
    if let Some(e) = errors.into_inner().expect("lock").first() {
        return Err(e.clone());
    }
    let rows: Vec<Vec<Cell>> = results
        .into_inner()
        .expect("lock")
        .into_iter()
        .map(|r| r.expect("every image scored"))
        .collect();

    let db = |img: &Image, cell: &Cell, d: usize| {
        psnr_from_mse(cell.sse[d] / (img.w * img.h * 3) as f64)
    };
    let mut csv = String::from("image,variant,psnr_nvidia,psnr_amd,seconds\n");
    for (img, row) in images.iter().zip(&rows) {
        for (c, name) in names.iter().enumerate() {
            csv += &format!(
                "{},{},{:.4},{:.4},{:.3}\n",
                img.stem,
                name,
                db(img, &row[c], 0),
                db(img, &row[c], 1),
                row[c].seconds
            );
        }
    }
    match &args.csv {
        Some(p) => std::fs::write(p, &csv).map_err(|e| format!("{}: {e}", p.display()))?,
        None => print!("{csv}"),
    }

    let per_image = |c: usize, d: usize| -> Vec<f64> {
        images
            .iter()
            .zip(&rows)
            .map(|(img, row)| db(img, &row[c], d))
            .collect()
    };
    let texels: f64 = images.iter().map(|i| (i.w * i.h * 3) as f64).sum();
    let mean = |c: usize, d: usize| mean_ci(&per_image(c, d)).0;
    println!("images: {}", images.len());
    for (c, name) in names.iter().enumerate() {
        let secs: f64 = rows.iter().map(|r| r[c].seconds).sum();
        println!(
            "{name}: nvidia {:.4} dB, amd {:.4} dB (mean of per-image dB); encode {:.1} s",
            mean(c, 0),
            mean(c, 1),
            secs
        );
        let pooled =
            |d: usize| psnr_from_mse(rows.iter().map(|r| r[c].sse[d]).sum::<f64>() / texels);
        println!(
            "{name}: pooled nvidia {:.4} dB, amd {:.4} dB",
            pooled(0),
            pooled(1)
        );
    }
    if let Some(base) = &args.baseline {
        let b = args
            .variants
            .iter()
            .position(|v| v == base)
            .expect("checked");
        for (c, name) in names.iter().enumerate().take(args.variants.len()) {
            if c == b {
                continue;
            }
            let mut parts = Vec::new();
            for (d, dec) in ["nvidia", "amd"].iter().enumerate() {
                let delta: Vec<f64> = per_image(c, d)
                    .iter()
                    .zip(per_image(b, d))
                    .map(|(x, y)| x - y)
                    .collect();
                let (m, hw) = mean_ci(&delta);
                let wins = delta.iter().filter(|x| **x > 0.0).count();
                let losses = delta.iter().filter(|x| **x < 0.0).count();
                parts.push(format!(
                    "{dec} {m:+.4} +/- {hw:.4} dB (wins {wins}, losses {losses})"
                ));
            }
            println!("{name} vs {base}: {}", parts.join("; "));
        }
    }
    for (r, (rname, _)) in args.refs.iter().enumerate() {
        let rc = args.variants.len() + r;
        for (c, name) in names.iter().enumerate().take(args.variants.len()) {
            let beats = (0..2).all(|d| mean(c, d) > mean(rc, d));
            println!("{name} beats {rname}: {}", if beats { "yes" } else { "no" });
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match parse_args().and_then(|a| run(&a)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("quality: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: usize, h: usize, px: Vec<[u8; 4]>) -> Image {
        let blocks = to_blocks(&gather_blocks(&px, w as u32, h as u32));
        Image {
            stem: "t".into(),
            w,
            h,
            px,
            blocks,
        }
    }

    #[test]
    fn psnr_follows_the_engine_definition() {
        assert_eq!(psnr_from_mse(0.0), 99.0);
        assert!((psnr_from_mse(65025.0)).abs() < 1e-12);
        assert!((psnr_from_mse(650.25) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn mean_ci_matches_a_hand_computed_sample() {
        let (m, hw) = mean_ci(&[1.0, 2.0, 3.0]);
        assert_eq!(m, 2.0);
        // sd 1, n 3: 4.303 * 1 / sqrt(3)
        assert!((hw - 4.303 / 3f64.sqrt()).abs() < 1e-12);
        assert!(mean_ci(&[5.0]).1.is_nan());
    }

    #[test]
    fn padding_texels_are_not_counted() {
        // A 1x1 image: the block repeats the one texel, so the error is that of one texel.
        let img = image(1, 1, vec![[200, 100, 50, 255]]);
        let out: Vec<[u8; 8]> = img
            .blocks
            .iter()
            .map(|b| bc1_encode_block_level(b, 18, true, false))
            .collect();
        let dec = bc1_decode_block(&out[0], Bc1Model::Nvidia)[0];
        let want: f64 = (0..3)
            .map(|c| (f64::from(img.px[0][c]) - f64::from(dec[c])).powi(2))
            .sum();
        assert_eq!(sse(&img, &out, Bc1Model::Nvidia), want);
    }

    #[test]
    fn variants_resolve_and_unknown_names_do_not() {
        assert!(encoder("cpp:18").is_some());
        assert!(encoder("cpp:19").is_none());
        assert!(encoder("cpp:x").is_none());
        assert!(encoder("rust:fast").is_none());
    }

    #[test]
    fn a_dds_file_is_read_and_checked() {
        let dir = std::env::temp_dir().join(format!("bcenc-quality-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = vec![0u8; 128];
        f[..4].copy_from_slice(b"DDS ");
        f[12..16].copy_from_slice(&4u32.to_le_bytes());
        f[16..20].copy_from_slice(&4u32.to_le_bytes());
        f[84..88].copy_from_slice(b"DXT1");
        f.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let p = dir.join("a.dds");
        std::fs::write(&p, &f).unwrap();
        assert_eq!(read_dds(&p, 4, 4).unwrap(), vec![[1, 2, 3, 4, 5, 6, 7, 8]]);
        assert!(read_dds(&p, 8, 4).unwrap_err().contains("differs"));
        // DX10 with DXGI 71
        let mut g = vec![0u8; 148];
        g[..4].copy_from_slice(b"DDS ");
        g[12..16].copy_from_slice(&4u32.to_le_bytes());
        g[16..20].copy_from_slice(&4u32.to_le_bytes());
        g[84..88].copy_from_slice(b"DX10");
        g[128..132].copy_from_slice(&71u32.to_le_bytes());
        g.extend_from_slice(&[9; 8]);
        let q = dir.join("b.dds");
        std::fs::write(&q, &g).unwrap();
        assert_eq!(read_dds(&q, 4, 4).unwrap(), vec![[9; 8]]);
        g[128..132].copy_from_slice(&98u32.to_le_bytes());
        std::fs::write(&q, &g).unwrap();
        assert!(read_dds(&q, 4, 4).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
