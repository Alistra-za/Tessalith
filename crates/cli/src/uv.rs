//! `sdc uv`: the UV islands of the meshes that use a texture, written as an 8-bit grayscale image (Note IB).
//! Island texels are 255, the recommended padding around an island 127, texels within padding of two different
//! islands 191, everything else 0.
use crate::args::UvArgs;
use engine::api::EngineError;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

/// The most tiles one triangle is drawn over along an axis; past it the triangle marks its whole UV box.
const TILE_CAP: i64 = 16;
/// Island, padding and too-close values of the output image.
const ISLAND: u8 = 255;
const TOO_CLOSE: u8 = 191;
const PADDING: u8 = 127;

macro_rules! out {
    ($($t:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout().lock(), $($t)*);
    }};
}

/// The padding a texture of this size wants, in texels: 2, or 1/256 of the longer side (512 to 2, 4096 to 16).
pub fn pad_for(w: u32, l: u32) -> u32 {
    (w.max(l) / 256).max(2)
}

// ---- the PATTERN ----

/// A texture path as a modder types it: `\` or `/`, with or without `textures\` and `.dds`, any case; `*` and `?`
/// match inside one path segment.
struct Pattern {
    segs: Vec<String>,
    recursive: bool,
}

impl Pattern {
    fn parse(s: &str, recursive: bool) -> Result<Pattern, String> {
        let mut p = s.trim().replace('\\', "/").to_ascii_lowercase();
        if let Some(x) = p.strip_suffix(".dds") {
            p = x.to_string();
        }
        let mut segs: Vec<String> = p
            .split('/')
            .filter(|x| !x.is_empty())
            .map(String::from)
            .collect();
        if segs.is_empty() {
            return Err("the texture pattern is empty".into());
        }
        if segs[0] != "textures" {
            segs.insert(0, "textures".into());
        }
        Ok(Pattern { segs, recursive })
    }

    /// For a canonical texture path that matches: how many of its leading segments are the pattern's literal folder
    /// part, which the output does not mirror. A pattern matches a file, or the textures in a folder (all below it
    /// with `--recursive`).
    fn matches(&self, tex: &str) -> Option<usize> {
        let t: Vec<&str> = tex.split('/').collect();
        let last = t.last()?.strip_suffix(".dds")?;
        let (n, m) = (t.len(), self.segs.len());
        let fits = |k: usize| (0..k).all(|i| glob(&self.segs[i], t[i]));
        if n == m && fits(m - 1) && glob(&self.segs[m - 1], last) {
            return Some(self.literal(m - 1));
        }
        if n > m && (self.recursive || n == m + 1) && fits(m) {
            return Some(self.literal(m));
        }
        None
    }

    /// The count of leading segments (of the first `k`) without a wildcard.
    fn literal(&self, k: usize) -> usize {
        self.segs[..k]
            .iter()
            .take_while(|s| !s.contains(['*', '?']))
            .count()
    }
}

/// `*` (any run of characters) and `?` (one character) on one segment.
pub(crate) fn glob(pat: &str, s: &str) -> bool {
    let (p, s): (Vec<char>, Vec<char>) = (pat.chars().collect(), s.chars().collect());
    let (mut pi, mut si, mut star, mut mark) = (0, 0, None, 0);
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) && p[pi] != '*' {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = si;
            pi += 1;
        } else if let Some(st) = star {
            pi = st + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

// ---- the meshes ----

enum Source {
    Loose(PathBuf),
    Bsa { archive: PathBuf, norm: String },
}

/// One NIF and the blocks of its shapes that use a texture.
struct ShapeRef {
    source: Source,
    label: String,
    blocks: Vec<u32>,
}

enum Meshes {
    /// Every NIF below a folder, read up front.
    Folder {
        nifs: Vec<(PathBuf, String)>,
        by_tex: BTreeMap<String, BTreeMap<usize, BTreeSet<u32>>>,
    },
    /// The game-winning copies of the GUI's scan.
    Scan(modscan::store::Store),
}

fn nifs_below(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut es: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        if p.is_dir() {
            nifs_below(&p, out)?;
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("nif")) {
            out.push(p);
        }
    }
    Ok(())
}

/// A panic inside a NIF reader is that NIF's error, not the run's.
fn guarded<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err("the NIF reader failed on this file".into()))
}

impl Meshes {
    fn folder(dir: &Path) -> Result<Meshes, EngineError> {
        if !dir.is_dir() {
            return Err(EngineError::Usage(format!(
                "--meshes {}: not a folder",
                dir.display()
            )));
        }
        let mut files = vec![];
        nifs_below(dir, &mut files)
            .map_err(|e| EngineError::Usage(format!("--meshes {}: {e}", dir.display())))?;
        let mut nifs = vec![];
        let mut by_tex: BTreeMap<String, BTreeMap<usize, BTreeSet<u32>>> = BTreeMap::new();
        for f in files {
            let label = f.strip_prefix(dir).unwrap_or(&f).display().to_string();
            let scanned = guarded(|| {
                let b = std::fs::read(&f).map_err(|e| e.to_string())?;
                nif::scan::scan(&b).map_err(|e| e.to_string())
            });
            let s = match scanned {
                Ok(s) if s.le_mesh => {
                    err!("{label}: a Skyrim LE mesh, skipped");
                    continue;
                }
                Ok(s) => s,
                Err(e) => {
                    err!("{label}: {e}, skipped");
                    continue;
                }
            };
            nifs.push((f, label));
            for row in &s.shapes {
                let texs: Vec<&str> = match &row.shader {
                    nif::scan::Shader::Lighting(_, slots) => {
                        slots.iter().map(|t| t.norm.as_str()).collect()
                    }
                    nif::scan::Shader::Effect(e) => vec![e.source.norm.as_str()],
                    _ => vec![],
                };
                for t in texs.into_iter().filter(|t| !t.is_empty()) {
                    by_tex
                        .entry(t.to_string())
                        .or_default()
                        .entry(nifs.len() - 1)
                        .or_default()
                        .insert(row.block);
                }
            }
        }
        Ok(Meshes::Folder { nifs, by_tex })
    }

    fn scan(db: Option<&Path>) -> Result<Meshes, EngineError> {
        if let Some(p) = db.filter(|p| !p.is_file()) {
            return Err(EngineError::Usage(format!(
                "--scan {}: no such file",
                p.display()
            )));
        }
        let default = modscan::store::default_path();
        let Some(path) = db
            .map(Path::to_path_buf)
            .or(default)
            .filter(|p| p.is_file())
        else {
            return Err(EngineError::Usage(
                "no scan database: run the Meshes scan in the GUI, or name a mesh folder with --meshes \
                 (a database elsewhere: --scan <db>)"
                    .into(),
            ));
        };
        let store = modscan::store::Store::open_read_only(&path)
            .map_err(|e| EngineError::Usage(format!("{}: {e}", path.display())))?;
        Ok(Meshes::Scan(store))
    }

    fn textures(&self) -> Result<Vec<String>, EngineError> {
        match self {
            Meshes::Folder { by_tex, .. } => Ok(by_tex.keys().cloned().collect()),
            Meshes::Scan(s) => s
                .used_texture_paths()
                .map_err(|e| EngineError::Usage(e.to_string())),
        }
    }

    /// The NIFs to read for `tex`.
    fn shapes(&self, tex: &str) -> Result<Vec<ShapeRef>, EngineError> {
        match self {
            Meshes::Folder { nifs, by_tex } => Ok(by_tex.get(tex).map_or_else(Vec::new, |m| {
                m.iter()
                    .map(|(&i, blocks)| ShapeRef {
                        source: Source::Loose(nifs[i].0.clone()),
                        label: nifs[i].1.clone(),
                        blocks: blocks.iter().copied().collect(),
                    })
                    .collect()
            })),
            Meshes::Scan(s) => {
                let usage = |e: modscan::ScanError| EngineError::Usage(e.to_string());
                let mut by_copy: BTreeMap<i64, Vec<u32>> = BTreeMap::new();
                for (copy, _, block) in s.winning_shapes_of_texture(tex).map_err(usage)? {
                    by_copy.entry(copy).or_default().push(block);
                }
                let mut out = vec![];
                for (copy, blocks) in by_copy {
                    let Some((p, kind, root, le)) = s.copy_location(copy).map_err(usage)? else {
                        continue;
                    };
                    if le {
                        err!("{p}: a Skyrim LE mesh, skipped");
                        continue;
                    }
                    let source = if kind == modscan::store::KIND_BSA {
                        Source::Bsa {
                            archive: root.into(),
                            norm: p.clone(),
                        }
                    } else {
                        Source::Loose(p.split('/').fold(PathBuf::from(root), |a, x| a.join(x)))
                    };
                    out.push(ShapeRef {
                        source,
                        label: p,
                        blocks,
                    });
                }
                Ok(out)
            }
        }
    }
}

/// An open archive and its entries by canonical name.
type IndexedBsa = (vfs::bsa::Bsa, HashMap<String, usize>);

/// Reads NIF bytes from disk or from a BSA; each archive is indexed once.
#[derive(Default)]
struct Reader {
    bsas: RefCell<HashMap<PathBuf, Option<IndexedBsa>>>,
}

impl Reader {
    fn read(&self, src: &Source) -> Result<Vec<u8>, String> {
        match src {
            Source::Loose(p) => std::fs::read(p).map_err(|e| format!("{}: {e}", p.display())),
            Source::Bsa { archive, norm } => {
                let mut map = self.bsas.borrow_mut();
                let idx = map.entry(archive.clone()).or_insert_with(|| {
                    vfs::bsa::Bsa::open(archive).ok().map(|b| {
                        let names = b
                            .entries
                            .iter()
                            .enumerate()
                            .map(|(i, e)| (e.norm.clone(), i))
                            .collect();
                        (b, names)
                    })
                });
                let Some((bsa, names)) = idx else {
                    return Err(format!("{}: cannot open the archive", archive.display()));
                };
                let i = *names
                    .get(norm)
                    .ok_or_else(|| format!("{norm}: not in {}", archive.display()))?;
                bsa.read(&bsa.entries[i])
                    .map_err(|e| format!("{norm} in {}: {e}", archive.display()))
            }
        }
    }
}

// ---- the raster ----

/// A triangle's UVs, or `None` for an index past `uvs` or a non-finite UV.
fn tri_uv(uvs: &[[f32; 2]], t: &[u16; 3]) -> Option<[[f64; 2]; 3]> {
    let mut p = [[0.0; 2]; 3];
    for (q, &k) in p.iter_mut().zip(t) {
        let uv = uvs.get(k as usize)?;
        if !uv[0].is_finite() || !uv[1].is_finite() {
            return None;
        }
        *q = [uv[0] as f64, uv[1] as f64];
    }
    Some(p)
}

/// The texels of one axis (`n` of them) whose square overlaps [lo, hi] with nonzero length, wrapped into [0, 1)
/// (every texel with `all`).
fn axis(n: u32, lo: f64, hi: f64, all: bool) -> Vec<u32> {
    if all || hi - lo >= 1.0 {
        return (0..n).collect();
    }
    let nf = n as f64;
    let mut on = vec![false; n as usize];
    for k in [lo.floor(), hi.floor()] {
        let first = ((lo - k) * nf).floor().max(0.0);
        let last = ((hi - k) * nf).ceil().min(nf) - 1.0;
        let mut x = first;
        while x <= last {
            on[x as usize] = true;
            x += 1.0;
        }
    }
    (0..n).filter(|&x| on[x as usize]).collect()
}

/// Whether triangle `t` (texel units) and the square [x, x+1] x [y, y+1] overlap with nonzero area: a separating
/// axis test over the box axes and the three edge normals. Touching only is no overlap.
fn overlaps(t: &[[f64; 2]; 3], x: f64, y: f64) -> bool {
    let sq = [[x, y], [x + 1.0, y], [x + 1.0, y + 1.0], [x, y + 1.0]];
    let span = |pts: &[[f64; 2]], a: [f64; 2]| {
        pts.iter()
            .map(|p| p[0] * a[0] + p[1] * a[1])
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), d| {
                (lo.min(d), hi.max(d))
            })
    };
    let axes = [
        [1.0, 0.0],
        [0.0, 1.0],
        [t[1][1] - t[0][1], t[0][0] - t[1][0]],
        [t[2][1] - t[1][1], t[1][0] - t[2][0]],
        [t[0][1] - t[2][1], t[2][0] - t[0][0]],
    ];
    axes.iter().all(|&a| {
        let ((tl, th), (bl, bh)) = (span(t, a), span(&sq, a));
        th > bl && bh > tl
    })
}

/// Marks every texel of the `w` x `l` mask that a triangle overlaps. A triangle is drawn once per integer tile its
/// UV box touches, shifted into [0, 1), so UVs outside 0..1 wrap. A triangle with a bad index, a non-finite UV or
/// no area is skipped; the count of those comes back.
// shortcut: a per-texel separating-axis test over each triangle's bounding box, so a huge triangle at 16384 is slow;
// upgrade to a scanline span fill if big atlases make `sdc uv` slow.
fn draw_mesh(mask: &mut [bool], w: u32, l: u32, uvs: &[[f32; 2]], tris: &[[u16; 3]]) -> usize {
    let (wf, lf) = (w as f64, l as f64);
    let mut skipped = 0;
    for tri in tris {
        let Some(p) = tri_uv(uvs, tri) else {
            skipped += 1;
            continue;
        };
        let area =
            (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
        if area == 0.0 {
            skipped += 1;
            continue;
        }
        let min = |a: usize| p.iter().map(|q| q[a]).fold(f64::INFINITY, f64::min);
        let max = |a: usize| p.iter().map(|q| q[a]).fold(f64::NEG_INFINITY, f64::max);
        let (umin, umax, vmin, vmax) = (min(0), max(0), min(1), max(1));
        let (u0, u1, v0, v1) = (umin.floor(), umax.floor(), vmin.floor(), vmax.floor());
        let wide = u1 - u0 + 1.0 > TILE_CAP as f64;
        let tall = v1 - v0 + 1.0 > TILE_CAP as f64;
        if wide || tall {
            // shortcut: a triangle repeating more than 16 times along an axis marks its whole UV box (the full axis
            // where it repeats past the cap), wrapped; upgrade if trim sheets need the exact shape.
            for y in axis(l, vmin, vmax, tall) {
                for x in axis(w, umin, umax, wide) {
                    mask[y as usize * w as usize + x as usize] = true;
                }
            }
            continue;
        }
        for ty in v0 as i64..=v1 as i64 {
            for tx in u0 as i64..=u1 as i64 {
                // The triangle in this tile, in texel units.
                let s = |q: [f64; 2]| [(q[0] - tx as f64) * wf, (q[1] - ty as f64) * lf];
                let t = [s(p[0]), s(p[1]), s(p[2])];
                let lo = |a: usize| t.iter().map(|q| q[a]).fold(f64::INFINITY, f64::min);
                let hi = |a: usize| t.iter().map(|q| q[a]).fold(f64::NEG_INFINITY, f64::max);
                let (x0, x1) = (lo(0).floor().max(0.0), (hi(0).ceil() - 1.0).min(wf - 1.0));
                let (y0, y1) = (lo(1).floor().max(0.0), (hi(1).ceil() - 1.0).min(lf - 1.0));
                let mut y = y0;
                while y <= y1 {
                    let mut x = x0;
                    while x <= x1 {
                        if overlaps(&t, x, y) {
                            mask[y as usize * w as usize + x as usize] = true;
                        }
                        x += 1.0;
                    }
                    y += 1.0;
                }
            }
        }
    }
    skipped
}

/// Labels the 8-connected components of the mask 1..=k (0 off the mask); returns k.
// shortcut: an island across the 0/1 edge of a tiling texture is two islands, as the padding window is not wrapped
// either; upgrade if wrapped layouts need it.
fn label(mask: &[bool], w: usize, l: usize) -> (Vec<u32>, u32) {
    let mut labels = vec![0u32; mask.len()];
    let (mut k, mut stack) = (0u32, vec![]);
    for start in 0..mask.len() {
        if !mask[start] || labels[start] != 0 {
            continue;
        }
        k += 1;
        labels[start] = k;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = ((i % w) as i64, (i / w) as i64);
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= l as i64 {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if mask[j] && labels[j] == 0 {
                        labels[j] = k;
                        stack.push(j);
                    }
                }
            }
        }
    }
    (labels, k)
}

/// In place: every `stride`-th element of `v` (a row or a column) becomes `op` over its neighbours within `r`
/// (clamped at the ends). van Herk / Gil-Werman, so linear in the elements whatever `r`.
fn window(v: &mut [u32], stride: usize, r: usize, neutral: u32, op: fn(u32, u32) -> u32) {
    let len = v.len().div_ceil(stride);
    let w = 2 * r + 1;
    let padded = len + 2 * r;
    let at = |i: usize| {
        if i < r || i >= r + len {
            neutral
        } else {
            v[(i - r) * stride]
        }
    };
    // g: op from each block's start up to i; h: op from i to its block's end.
    let mut g = vec![neutral; padded];
    let mut h = vec![neutral; padded];
    for i in 0..padded {
        g[i] = if i % w == 0 {
            at(i)
        } else {
            op(g[i - 1], at(i))
        };
    }
    for i in (0..padded).rev() {
        h[i] = if i % w == w - 1 || i + 1 == padded {
            at(i)
        } else {
            op(h[i + 1], at(i))
        };
    }
    for i in 0..len {
        v[i * stride] = op(h[i], g[i + 2 * r]);
    }
}

/// The finished image: pixels (row 0 at v = 0), the island count and the number of too-close texels.
struct Image {
    pixels: Vec<u8>,
    islands: u32,
    too_close: u64,
}

/// Islands 255; a texel off every island within `pad` (Chebyshev) of one island 127, of two or more 191.
// shortcut: the mask, labels, window min and max and pixels cost about 12 bytes per texel (several GB at 16384 x
// 16384); upgrade to tiles or u16 labels if the largest sizes are wanted on small machines.
fn paint(mask: &[bool], w: u32, l: u32, pad: u32) -> Image {
    let (w, l, pad) = (w as usize, l as usize, pad as usize);
    let (labels, islands) = label(mask, w, l);
    // Window min and max of the labels over rows, then columns: the minimum ignores empty texels (u32::MAX) and the
    // maximum has 0 for them, so a differing minimum and maximum means two or more islands.
    let mut lo: Vec<u32> = labels
        .iter()
        .map(|&x| if x == 0 { u32::MAX } else { x })
        .collect();
    let mut hi = labels.clone();
    for y in 0..l {
        window(&mut lo[y * w..(y + 1) * w], 1, pad, u32::MAX, u32::min);
        window(&mut hi[y * w..(y + 1) * w], 1, pad, 0, u32::max);
    }
    for x in 0..w {
        window(&mut lo[x..], w, pad, u32::MAX, u32::min);
        window(&mut hi[x..], w, pad, 0, u32::max);
    }
    let mut too_close = 0;
    let pixels = (0..labels.len())
        .map(|i| {
            if labels[i] != 0 {
                ISLAND
            } else if hi[i] == 0 {
                0
            } else if lo[i] != hi[i] {
                too_close += 1;
                TOO_CLOSE
            } else {
                PADDING
            }
        })
        .collect();
    Image {
        pixels,
        islands,
        too_close,
    }
}

// ---- the command ----

fn png(img: &Image, w: u32, l: u32) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let mut out = vec![];
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&img.pixels, w, l, image::ExtendedColorType::L8)
        .map_err(|e| e.to_string())?;
    Ok(out)
}

/// Exit codes: 0 every matched texture written; 1 some wrote nothing (no triangles, or the file exists without
/// `--overwrite`); 2 a usage error, including a pattern that matches no texture; 3 a write failed.
pub fn run(a: UvArgs) -> Result<i32, EngineError> {
    let (w, l) = match (a.s, a.w, a.l) {
        (Some(s), ..) => (s, s),
        (None, Some(w), Some(l)) => (w, l),
        _ => return Err(EngineError::Usage("give --s N, or --w W --l L".into())),
    };
    let pat = Pattern::parse(&a.pattern, a.recursive).map_err(EngineError::Usage)?;
    let meshes = match &a.meshes {
        Some(dir) => Meshes::folder(dir)?,
        None => Meshes::scan(a.scan.as_deref())?,
    };
    let matched: Vec<(String, usize)> = meshes
        .textures()?
        .into_iter()
        .filter_map(|t| pat.matches(&t).map(|b| (t, b)))
        .collect();
    if matched.is_empty() {
        return Err(EngineError::Usage(format!(
            "no texture matches {}: the pattern is a texture path, with or without textures\\ and .dds; \
             a folder needs --recursive for what is below it",
            a.pattern
        )));
    }
    let out_root = a.out.clone().unwrap_or_else(|| PathBuf::from("."));
    let pad = pad_for(w, l);
    let reader = Reader::default();
    let mut code = 0;
    for (tex, base) in matched {
        let segs: Vec<&str> = tex.split('/').collect();
        let stem = segs[segs.len() - 1].trim_end_matches(".dds");
        let mut dest = out_root.clone();
        dest.extend(&segs[base..segs.len() - 1]);
        dest.push(format!("{stem}_uvmask.png"));
        if dest.exists() && !a.overwrite {
            err!("{}: exists, kept (--overwrite replaces it)", dest.display());
            code = code.max(1);
            continue;
        }
        let mut mask = vec![false; w as usize * l as usize];
        let (mut nifs, mut drawn) = (0, 0);
        // shortcut: each texture re-reads and re-parses its NIFs (no parse cache); cache the parsed NIFs if a
        // `--recursive` run over many textures proves slow.
        for s in meshes.shapes(&tex)? {
            let read = reader.read(&s.source).and_then(|b| {
                let h = nif::header::Header::parse(&b).map_err(|e| e.to_string())?;
                Ok((b, h))
            });
            let (bytes, header) = match read {
                Ok(x) => x,
                Err(e) => {
                    err!("{}: {e}, skipped", s.label);
                    continue;
                }
            };
            nifs += 1;
            for &b in &s.blocks {
                match guarded(|| {
                    nif::geom::read_shape(&header, &bytes, b as usize).map_err(|e| e.to_string())
                }) {
                    Ok(m) => {
                        let skipped = draw_mesh(&mut mask, w, l, &m.uvs, &m.triangles);
                        drawn += m.triangles.len() - skipped;
                        if skipped > 0 {
                            err!(
                                "{}: block {b}: {skipped} triangles with no UVs, bad UVs or no area skipped",
                                s.label
                            );
                        }
                    }
                    Err(e) => err!("{}: block {b}: {e}, skipped", s.label),
                }
            }
        }
        if drawn == 0 {
            err!("{tex}: no meshes with UV triangles, nothing written");
            code = code.max(1);
            continue;
        }
        let img = paint(&mask, w, l, pad);
        // A write failure is this texture's (exit 3); the rest are still written, as `sdc dilate` does.
        let written = png(&img, w, l).and_then(|bytes| {
            if let Some(dir) = dest.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            engine::dds::write_atomic(&dest, &bytes).map_err(|e| e.to_string())
        });
        if let Err(e) = written {
            err!("{}: {e}", dest.display());
            code = code.max(3);
            continue;
        }
        out!(
            "wrote {}: islands {}, pad {pad}, too close {}, meshes {nifs}",
            dest.display(),
            img.islands,
            img.too_close
        );
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask_of(w: u32, l: u32, uvs: &[[f32; 2]], tris: &[[u16; 3]]) -> Vec<bool> {
        let mut m = vec![false; (w * l) as usize];
        draw_mesh(&mut m, w, l, uvs, tris);
        m
    }

    /// Texels x0..x1 by y0..y1 of a 512 square as a two-triangle quad (all multiples of 1/512).
    fn quad(x0: u32, x1: u32, y0: u32, y1: u32) -> (Vec<[f32; 2]>, Vec<[u16; 3]>) {
        let f = |v: u32| v as f32 / 512.0;
        (
            vec![
                [f(x0), f(y0)],
                [f(x1), f(y0)],
                [f(x1), f(y1)],
                [f(x0), f(y1)],
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    }

    fn at(img: &Image, w: u32, x: u32, y: u32) -> u8 {
        img.pixels[(y * w + x) as usize]
    }

    #[test]
    fn the_pad_follows_the_longer_side() {
        assert_eq!(
            [512, 1024, 2048, 4096, 8192].map(|s| pad_for(s, s)),
            [2, 4, 8, 16, 32]
        );
        assert_eq!(pad_for(2048, 4096), 16);
        assert_eq!(pad_for(64, 64), 2);
    }

    #[test]
    fn a_sliver_narrower_than_a_texel_is_still_drawn() {
        // 0.3 texels wide, 100 tall, inside texel column 10: the old centre test marked nothing.
        let (a, b) = (10.2 / 512.0, 10.5 / 512.0);
        let m = mask_of(
            512,
            512,
            &[[a, 0.0], [b, 0.0], [a, 100.0 / 512.0]],
            &[[0, 1, 2]],
        );
        assert!(m[10] && m[50 * 512 + 10], "the column is marked");
        assert_eq!(m.iter().filter(|&&t| t).count(), 100, "one texel per row");
    }

    #[test]
    fn a_triangle_only_touching_a_texel_does_not_mark_it() {
        // The right triangle (0,0) (2,0) (0,2) in texels: the texel (1,1) touches the hypotenuse along an edge only.
        let f = |v: f32| v / 512.0;
        let m = mask_of(
            512,
            512,
            &[[0.0, 0.0], [f(2.0), 0.0], [0.0, f(2.0)]],
            &[[0, 1, 2]],
        );
        let on: Vec<_> = (0..512 * 3)
            .filter(|&i| m[i])
            .map(|i| (i % 512, i / 512))
            .collect();
        assert_eq!(on, [(0, 0), (1, 0), (0, 1)]);
    }

    #[test]
    fn uvs_past_1_wrap_and_bad_triangles_are_skipped() {
        let (uvs, tris) = quad(128, 256, 128, 256);
        let shifted: Vec<[f32; 2]> = uvs.iter().map(|p| [p[0] + 1.0, p[1] - 2.0]).collect();
        assert_eq!(
            mask_of(512, 512, &uvs, &tris),
            mask_of(512, 512, &shifted, &tris)
        );
        let mut m = vec![false; 512 * 512];
        let skipped = draw_mesh(
            &mut m,
            512,
            512,
            &[[0.0, 0.0], [f32::NAN, 0.0], [0.0, 1.0], [0.5, 0.5]],
            &[[0, 1, 2], [0, 2, 9], [3, 3, 3]],
        );
        assert_eq!((skipped, m.iter().any(|&t| t)), (3, false));
    }

    #[test]
    fn eight_connected_texels_are_one_island() {
        let mut m = vec![false; 16];
        for (x, y) in [(0, 0), (1, 1), (3, 3)] {
            m[y * 4 + x] = true;
        }
        assert_eq!(label(&m, 4, 4).1, 2);
    }

    #[test]
    fn one_island_has_a_padding_band_of_exactly_pad_texels() {
        let (uvs, tris) = quad(128, 256, 128, 256);
        let img = paint(&mask_of(512, 512, &uvs, &tris), 512, 512, 2);
        assert_eq!((img.islands, img.too_close), (1, 0));
        // Inside 255, the two texels around it 127, then 0; corners follow the Chebyshev square.
        assert_eq!(at(&img, 512, 128, 128), 255);
        assert_eq!(at(&img, 512, 255, 255), 255);
        for x in [126, 127, 256, 257] {
            assert_eq!(at(&img, 512, x, 200), 127, "x {x}");
            assert_eq!(at(&img, 512, 200, x), 127, "y {x}");
        }
        assert_eq!(at(&img, 512, 257, 257), 127);
        for x in [125, 258] {
            assert_eq!(at(&img, 512, x, 200), 0, "x {x}");
        }
        assert_eq!(at(&img, 512, 258, 258), 0);
        assert!(img.pixels.iter().all(|&p| [0, 127, 255].contains(&p)));
    }

    #[test]
    fn islands_closer_than_two_pads_get_191_between_and_far_ones_do_not() {
        let build = |gap: u32| {
            let (u1, t1) = quad(128, 256, 128, 256);
            let (u2, t2) = quad(256 + gap, 384 + gap, 128, 256);
            let mut m = vec![false; 512 * 512];
            draw_mesh(&mut m, 512, 512, &u1, &t1);
            draw_mesh(&mut m, 512, 512, &u2, &t2);
            paint(&m, 512, 512, 2)
        };
        // A gap of 3 texels (256..259): texel 257 is 2 from both islands.
        let near = build(3);
        assert_eq!(near.islands, 2);
        assert_eq!(at(&near, 512, 256, 200), 127);
        assert_eq!(at(&near, 512, 257, 200), 191);
        assert_eq!(at(&near, 512, 258, 200), 127);
        // Rows 126..=257 (the islands' rows 128..256 widened by the pad), one column.
        assert_eq!(near.too_close, 132);
        // A gap of 4 is two pads: no texel is within pad of both.
        let far = build(4);
        assert_eq!(far.too_close, 0);
        assert!(!far.pixels.contains(&191));
    }

    #[test]
    fn the_window_is_clamped_at_the_image_edge() {
        // An island at the left edge: no padding appears on the right edge (no wrap).
        let (uvs, tris) = quad(0, 8, 100, 108);
        let img = paint(&mask_of(512, 512, &uvs, &tris), 512, 512, 2);
        assert_eq!(at(&img, 512, 8, 104), 127);
        assert_eq!(at(&img, 512, 511, 104), 0);
    }

    #[test]
    fn the_window_is_the_min_over_the_neighbourhood() {
        let src = [5u32, 1, 7, 3, 9, 2, 8, 6, 4];
        // r past the row length too
        for r in 0..12 {
            let mut v = src;
            window(&mut v, 1, r, u32::MAX, u32::min);
            for i in 0..src.len() {
                let want = src[i.saturating_sub(r)..=(i + r).min(src.len() - 1)]
                    .iter()
                    .copied()
                    .min()
                    .unwrap();
                assert_eq!(v[i], want, "r {r} i {i}");
            }
        }
        // A column: stride 3 over a 3x3 grid touches only column 0.
        let mut g = [9u32, 0, 0, 1, 0, 0, 5, 0, 0];
        window(&mut g, 3, 1, u32::MAX, u32::min);
        assert_eq!(g, [1, 0, 0, 1, 0, 0, 1, 0, 0]);
        // The same for the maximum, with a column longer than the radius at stride 2.
        let mut m = [1u32, 9, 4, 9, 2, 9, 8, 9, 3, 9];
        window(&mut m, 2, 5, 0, u32::max);
        assert_eq!(m, [8, 9, 8, 9, 8, 9, 8, 9, 8, 9]);
    }

    #[test]
    fn the_axis_wraps_into_the_mask() {
        assert_eq!(axis(8, 0.9, 1.2, false), vec![0, 1, 7]);
        assert_eq!(axis(8, 0.0, 0.25, false), vec![0, 1]);
        // a sliver inside one texel marks it; a span ending on a texel edge does not mark the next
        assert_eq!(axis(8, 0.1, 0.12, false), vec![0]);
        assert_eq!(axis(8, 0.25, 0.5, false), vec![2, 3]);
        assert_eq!(axis(4, 3.0, 3.1, true), vec![0, 1, 2, 3]);
        assert_eq!(axis(4, 0.0, 1.0, false), vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_trim_strip_repeating_in_u_marks_only_its_band() {
        // 4 tiles wide, texel rows 51.2..102.4 of 512: rows 51..=102
        let uvs = [[0.0, 0.1], [4.0, 0.1], [4.0, 0.2], [0.0, 0.2]];
        let m = mask_of(512, 512, &uvs, &[[0, 1, 2], [0, 2, 3]]);
        assert_eq!(m.iter().filter(|&&t| t).count(), 52 * 512);
        assert!(m[60 * 512 + 100] && !m[200 * 512 + 100]);
    }

    #[test]
    fn a_long_trim_strip_past_the_cap_marks_its_band_not_the_whole_mask() {
        // 301 tiles in u (past TILE_CAP), one in v
        let uvs = [[0.0, 0.1], [300.0, 0.1], [300.0, 0.2], [0.0, 0.2]];
        let m = mask_of(512, 512, &uvs, &[[0, 1, 2], [0, 2, 3]]);
        assert_eq!(m.iter().filter(|&&t| t).count(), 52 * 512);
        assert!(m[60 * 512 + 100] && !m[200 * 512 + 100]);
    }

    #[test]
    fn a_tiling_triangle_past_the_cap_on_both_axes_fills_the_image() {
        let m = mask_of(
            512,
            512,
            &[[0.0, 0.0], [40.0, 0.0], [0.0, 40.0]],
            &[[0, 1, 2]],
        );
        assert!(m.iter().all(|&t| t));
    }

    #[test]
    fn patterns_match_files_and_folders() {
        let p = |s: &str, r| Pattern::parse(s, r).unwrap();
        let t = "textures/harry2135/fantasyseries6/fs6_arrow_ir.dds";
        for s in [
            "Harry2135\\FantasySeries6\\FS6_Arrow_IR",
            "textures/harry2135/fantasyseries6/fs6_arrow_ir.dds",
            "harry2135/fantasyseries6/fs6_*",
            "harry2135/fantasyseries6/fs6_arrow_i?",
            "Harry2135\\FantasySeries6",
            "harry2135/fantasy*/",
        ] {
            assert!(p(s, false).matches(t).is_some(), "{s}");
        }
        assert_eq!(
            p("harry2135/fantasyseries6/fs6_arrow_ir", false).matches(t),
            Some(3)
        );
        assert_eq!(p("harry2135", false).matches(t), None);
        assert_eq!(p("harry2135", true).matches(t), Some(2));
        assert_eq!(
            p("harry2135/fantasyseries6/fs6_*", false).matches(t),
            Some(3)
        );
        assert_eq!(p("harry2135/other", true).matches(t), None);
        assert_eq!(p("fs6_arrow_ir", false).matches(t), None);
        assert!(Pattern::parse("  ", false).is_err());
    }
}
