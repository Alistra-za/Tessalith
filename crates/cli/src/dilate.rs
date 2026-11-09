//! `sdc dilate`: fills the texels outside a `_matid` or `_uvmask` mask by push-pull, keeping the masked texels as
//! they are (Note IB).
use crate::args::DilateArgs;
use crate::uv::glob;
use engine::api::EngineError;
use engine::config::{Category, Subtype, Target};
use engine::presets::{AlphaContent, PresetRegistry};
use engine::suggest::{suggest, SuggestContext};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

macro_rules! out {
    ($($t:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout().lock(), $($t)*);
    }};
}

/// How an input's texels are filled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// Colour in sRGB: decoded, filled in linear light, encoded.
    Srgb,
    /// A normal map: filled as stored, the filled texels renormalised.
    Normal,
    /// Filled as stored.
    Data,
}

/// A mask or input a run never reads as an input: `_matid`/`_uvmask` images, and anything inside a `dilated` folder.
fn is_skipped(p: &Path) -> bool {
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let dir = p.parent().unwrap_or(Path::new(""));
    stem.ends_with("_matid")
        || stem.ends_with("_uvmask")
        || dir
            .components()
            .any(|c| c.as_os_str().eq_ignore_ascii_case("dilated"))
}

fn is_image(p: &Path) -> bool {
    p.extension().is_some_and(|e| {
        ["png", "tga", "tif", "tiff"]
            .iter()
            .any(|x| e.eq_ignore_ascii_case(x))
    })
}

/// The images in `dir` (and below it with `recursive`).
fn images_in(dir: &Path, recursive: bool, out: &mut BTreeSet<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if recursive {
                images_in(&p, true, out);
            }
        } else if is_image(&p) {
            out.insert(p);
        }
    }
}

/// Walks `rest` (path components, some with `*` or `?`) below `base`.
fn walk(base: PathBuf, rest: &[std::ffi::OsString], out: &mut Vec<PathBuf>) {
    let Some((first, tail)) = rest.split_first() else {
        out.push(base);
        return;
    };
    let name = first.to_string_lossy().to_ascii_lowercase();
    if !name.contains(['*', '?']) {
        let p = base.join(first);
        if p.exists() {
            walk(p, tail, out);
        }
        return;
    }
    let Ok(rd) = std::fs::read_dir(&base) else {
        return;
    };
    for e in rd.flatten() {
        if glob(&name, &e.file_name().to_string_lossy().to_ascii_lowercase()) {
            walk(e.path(), tail, out);
        }
    }
}

/// The input images a PATTERN names: a file, a folder, or a glob on disk; skipped names removed.
fn expand(pat: &str, recursive: bool, out: &mut BTreeSet<PathBuf>) {
    let path = Path::new(pat);
    let mut hits = vec![];
    if path.exists() {
        hits.push(path.to_path_buf());
    } else {
        let comps: Vec<_> = path.components().collect();
        let k = comps
            .iter()
            .position(|c| c.as_os_str().to_string_lossy().contains(['*', '?']))
            .unwrap_or(comps.len());
        let base: PathBuf = comps[..k].iter().collect();
        let base = if base.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            base
        };
        let rest: Vec<_> = comps[k..]
            .iter()
            .map(|c| c.as_os_str().to_owned())
            .collect();
        walk(base, &rest, &mut hits);
    }
    for h in hits {
        if h.is_dir() {
            images_in(&h, recursive, out);
        } else {
            out.insert(h);
        }
    }
}

/// The mask beside `src`: `<base>_matid.(png|tga|tif|tiff)`, then `<base>_uvmask.png`, for the stem minus its texture
/// suffix as the engine reads it, then for the full stem.
fn find_mask(src: &Path) -> Option<PathBuf> {
    let stem = src.file_stem()?.to_string_lossy().into_owned();
    let dir = src.parent().unwrap_or(Path::new("."));
    let mut bases = vec![engine::group::base_of(&stem)];
    if bases[0] != stem {
        bases.push(stem);
    }
    bases.iter().find_map(|b| {
        ["png", "tga", "tif", "tiff"]
            .iter()
            .map(|e| format!("{b}_matid.{e}"))
            .chain([format!("{b}_uvmask.png")])
            .map(|n| dir.join(n))
            .find(|p| p.is_file())
    })
}

/// The fill kind of a file stem, and what to call it in the output. A name with no sub-type is filled as stored.
fn kind_of(stem: &str, reg: &PresetRegistry) -> (Kind, String) {
    let suffix = engine::group::split_vocab(stem)
        .and_then(|(_, v)| v.canonical())
        .or_else(|| engine::group::split_suffix(stem).1);
    let set = BTreeSet::new();
    let sug = suggest(&SuggestContext {
        target: Target::Vanilla,
        suffix: suffix.as_deref(),
        game_path: stem,
        set_suffixes: &set,
        alpha: AlphaContent::None,
        is_float: false,
        advanced_skin: false,
        may_wait: true,
    });
    let Some(st) = sug.first().map(|s| s.subtype) else {
        return (
            Kind::Data,
            "no sub-type from the name, filled as stored".into(),
        );
    };
    let kind = if st.category() == Category::Normal
        && !matches!(st, Subtype::CoatNormal | Subtype::SkinWet)
    {
        Kind::Normal
    } else if reg.base_profile(Target::Vanilla, st).color.content
        == engine::config::ColorContent::Srgb
    {
        Kind::Srgb
    } else {
        Kind::Data
    };
    (kind, st.id())
}

/// Where a mask texel counts: `_uvmask` images at 255, any other mask where R, G or B is above 0.
fn valid_of(mask: &engine::image::Rgba32F, uv: bool) -> Vec<bool> {
    mask.px
        .iter()
        .map(|p| {
            if uv {
                p[0] >= 1.0
            } else {
                p[..3].iter().any(|&v| v > 0.0)
            }
        })
        .collect()
}

fn encode_png(codes: &[u16], w: u32, h: u32, channels: u8, depth: u8) -> Result<Vec<u8>, String> {
    use image::{ExtendedColorType as E, ImageEncoder};
    let ct = match (channels, depth) {
        (1, 8) => E::L8,
        (2, 8) => E::La8,
        (3, 8) => E::Rgb8,
        (4, 8) => E::Rgba8,
        (1, 16) => E::L16,
        (2, 16) => E::La16,
        (3, 16) => E::Rgb16,
        (4, 16) => E::Rgba16,
        _ => {
            return Err(format!(
                "{channels} channels at {depth} bits is not handled"
            ))
        }
    };
    let bytes: Vec<u8> = if depth == 8 {
        codes.iter().map(|&c| c as u8).collect()
    } else {
        codes.iter().flat_map(|c| c.to_ne_bytes()).collect()
    };
    let mut out = vec![];
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&bytes, w, h, ct)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// One input. `Err((code, message))`: 1 nothing written for this input, 3 the write failed.
fn process(src: &Path, a: &DilateArgs, reg: &PresetRegistry) -> Result<String, (i32, String)> {
    let skip = |m: String| (1, format!("{}: {m}", src.display()));
    let (img, info) = engine::load::load_image(src).map_err(|e| (1, e.to_string()))?;
    if info.is_float || !matches!(info.bit_depth, 8 | 16) {
        // shortcut: float images (EXR, HDR, 32-bit TIFF) are refused, add them when a float push-pull is wanted.
        return Err(skip(
            "float images are refused: dilate reads 8- and 16-bit images".into(),
        ));
    }
    let stem = src
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mask_path = match &a.mask {
        Some(m) => m.clone(),
        None => find_mask(src).ok_or_else(|| {
            skip(format!(
                "no mask: put {stem}_matid.png (or the base's _matid or _uvmask) beside it, or pass --mask"
            ))
        })?,
    };
    let (mask, _) = engine::load::load_image(&mask_path).map_err(|e| (1, e.to_string()))?;
    if (mask.width, mask.height) != (img.width, img.height) {
        return Err(skip(format!(
            "the mask {} is {}x{}, the image {}x{}",
            mask_path.display(),
            mask.width,
            mask.height,
            img.width,
            img.height
        )));
    }
    let uv = mask_path.file_stem().is_some_and(|s| {
        s.to_string_lossy()
            .to_ascii_lowercase()
            .ends_with("_uvmask")
    });
    let valid = valid_of(&mask, uv);
    let dest_dir = match &a.out {
        Some(o) => o.clone(),
        None => src.parent().unwrap_or(Path::new(".")).join("dilated"),
    };
    let dest = dest_dir.join(format!("{stem}.png"));
    if dest.exists() && !a.overwrite {
        return Err(skip(format!(
            "{} exists, kept (--overwrite replaces it)",
            dest.display()
        )));
    }
    if std::fs::canonicalize(&dest).ok() == std::fs::canonicalize(src).ok() && dest.exists() {
        return Err(skip("the output would replace the input".into()));
    }
    let (kind, what) = kind_of(&stem, reg);

    let orig = img.px.clone();
    let mut work = img;
    if kind == Kind::Srgb {
        for p in &mut work.px {
            for c in &mut p[..3] {
                *c = engine::ops::srgb_to_linear(*c);
            }
        }
    }
    if !engine::dilate::push_pull_fill(&mut work, &valid) {
        return Err(skip(format!(
            "the mask {} marks no texel",
            mask_path.display()
        )));
    }
    let max = if info.bit_depth == 16 {
        65535.0f32
    } else {
        255.0
    };
    let code = |v: f32| (v * max).round().clamp(0.0, max) as u16;
    let mut codes = Vec::with_capacity(work.px.len() * info.channels as usize);
    for (i, ok) in valid.iter().enumerate() {
        // masked texels keep their decoded sample (k / max, exact to the code); only filled ones are re-encoded
        let mut p = if *ok { orig[i] } else { work.px[i] };
        if !ok {
            match kind {
                Kind::Srgb => {
                    for c in &mut p[..3] {
                        *c = engine::ops::linear_to_srgb(*c);
                    }
                }
                Kind::Normal if info.channels >= 3 => {
                    let n = [p[0] * 2.0 - 1.0, p[1] * 2.0 - 1.0, p[2] * 2.0 - 1.0];
                    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                    let n = if len > 1e-6 {
                        n.map(|v| v / len)
                    } else {
                        [0.0, 0.0, 1.0]
                    };
                    for (c, v) in p.iter_mut().zip(n) {
                        *c = v * 0.5 + 0.5;
                    }
                }
                _ => {}
            }
        }
        match info.channels {
            1 => codes.push(code(p[0])),
            2 => codes.extend([code(p[0]), code(p[3])]),
            3 => codes.extend([code(p[0]), code(p[1]), code(p[2])]),
            _ => codes.extend(p.map(code)),
        }
    }
    let bytes = encode_png(
        &codes,
        work.width,
        work.height,
        info.channels,
        info.bit_depth,
    )
    .map_err(|e| (3, format!("{}: {e}", dest.display())))?;
    std::fs::create_dir_all(&dest_dir).map_err(|e| (3, format!("{}: {e}", dest_dir.display())))?;
    engine::dds::write_atomic(&dest, &bytes)
        .map_err(|e| (3, format!("{}: {e}", dest.display())))?;
    let filled = valid.iter().filter(|v| !**v).count();
    Ok(format!(
        "wrote {}: {what}, mask {}, filled {filled} of {} texels",
        dest.display(),
        mask_path.display(),
        valid.len()
    ))
}

/// Exit codes: 0 every input written; 1 some input wrote nothing (no mask, size mismatch, float, exists without
/// `--overwrite`); 2 a usage error, including patterns that match no image; 3 a write failed.
pub fn run(a: DilateArgs) -> Result<i32, EngineError> {
    let mut found = BTreeSet::new();
    for p in &a.patterns {
        expand(p, a.recursive, &mut found);
    }
    found.retain(|p| !is_skipped(p));
    if found.is_empty() {
        return Err(EngineError::Usage(format!(
            "no image matches {}: give image files, folders or globs; _matid and _uvmask images and the dilated \
             folder are never inputs",
            a.patterns.join(" ")
        )));
    }
    let reg = PresetRegistry::builtin().map_err(|e| EngineError::Fatal(e.to_string()))?;
    let mut code = 0;
    for src in found {
        match process(&src, &a, &reg) {
            Ok(line) => out!("{line}"),
            Err((c, m)) => {
                err!("{m}");
                code = code.max(c);
            }
        }
    }
    Ok(code)
}
