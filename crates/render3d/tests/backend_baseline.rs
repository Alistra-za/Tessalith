//! Measurements for the render3d graphics-backend research (2026-11-16). They print numbers and assert only that
//! each scene loads; nothing here pins a speed. Both tests are ignored and read owner data read-only.
//! - `render_backend_baseline` (`SDC_ACHERUS` = the MO2 instance): NIF parse, cold and warm load, and frame timings of
//!   [`SCENES`] on the hardware GPU and on WARP. `SDC_R3D_FRAMES` (default 200) sets the timed frames per resolution;
//!   `SDC_R3D_WARP_BUDGET_S` (default 150) caps the seconds one WARP resolution may take (fewer frames, reported as
//!   `n`). `SDC_R3D_PROBE=<path;path>` (NIFs relative to the MO2 instance) prints only their scene stats.
//!   `SDC_R3D_FRAME_OUT=<dir>` writes each scene's last warm-up frame at the first resolution as raw RGBA8
//!   (`<scene>-<device>-<w>x<h>.rgba`), to compare one D3D11 runtime with another (DXVK beside the test exe).
//! - `dump_permutations` (`SDC_R3D_DUMP=<dir>`, with `SDC_GOLDEN` and/or `SDC_ACHERUS`; `SDC_R3D_SHADERS=<folder>`
//!   reads another CS tree instead of the bundled one): writes every distinct (source, target, entry, defines) that
//!   render3d compiles as `<n>.hlsl` (include-expanded, what D3DCompile sees) and `<n>.json`, for
//!   tools\r3d-dxc-trial.ps1.
//!
//! `cargo test -p render3d -p sdc --release --offline --test backend_baseline -- --ignored --nocapture --test-threads=1`
use render3d::cs_settings::{defaults, preview_defines};
use render3d::device::Gpu;
use render3d::material::{perm_material, pipe};
use render3d::permutation::{defines, Defines, Stage};
use render3d::scene::Scene;
use render3d::source::SourceSet;
use render3d::{golden, parse, replay, ParsedNif, Renderer, ShaderSource, ShapeStatus};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// (label, mod folder under `mods`, NIF under that folder).
const SCENES: [(&str, &str, &str); 3] = [
    (
        "static",
        "SkyrimMeshesExtract",
        r"meshes\architecture\whiterun\wrinteriors\wrinthousedrygoods01.nif",
    ),
    (
        "skinned",
        "SkyrimMeshesExtract",
        r"meshes\armor\ebony\f\cuirass_1.nif",
    ),
    (
        "pbr",
        "Faultier's PBR Windhelm",
        r"meshes\srider\srex\statics\srexmarkarthfrontwall.nif",
    ),
];
const RES: [(u32, u32); 4] = [(1280, 720), (1600, 900), (1920, 1080), (3840, 2160)];
const WARMUP: usize = 20;
/// The golden profiles `tests/golden.rs` replays.
const PROFILES: &[&str] = &["A", "A_RT", "A_RT2", "A_DC", "A_DC_N"];

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn env_num(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Nearest-rank quantile of `v` (sorted in place).
fn q(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// Texture bytes by canonical path, read once from loose files: the scene's mod, then the vanilla extract, then every
/// other mod folder in name order (MO2 priority and BSAs are not modelled). Misses are remembered too, so a timed
/// load after the first only clones bytes out of memory.
struct Textures {
    roots: Vec<PathBuf>,
    memo: RefCell<HashMap<String, Option<Vec<u8>>>>,
}

impl Textures {
    fn new(acherus: &Path, first: &str) -> Textures {
        let mods = acherus.join("mods");
        let mut roots = vec![mods.join(first), mods.join("SkyrimMeshesExtract")];
        let mut rest: Vec<PathBuf> = std::fs::read_dir(&mods)
            .map(|d| {
                d.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir() && !roots.contains(p))
                    .collect()
            })
            .unwrap_or_default();
        rest.sort();
        roots.extend(rest);
        Textures {
            roots,
            memo: RefCell::new(HashMap::new()),
        }
    }

    fn get(&self, norm: &str) -> Option<Vec<u8>> {
        if let Some(v) = self.memo.borrow().get(norm) {
            return v.clone();
        }
        let rel = norm.replace('/', "\\");
        let hit = self
            .roots
            .iter()
            .find_map(|r| std::fs::read(r.join(&rel)).ok());
        self.memo.borrow_mut().insert(norm.to_string(), hit.clone());
        hit
    }

    /// (found, asked but not found, bytes of the found ones).
    fn summary(&self) -> (usize, usize, u64) {
        let m = self.memo.borrow();
        let found = m.values().filter(|v| v.is_some()).count();
        let bytes = m.values().flatten().map(|b| b.len() as u64).sum();
        (found, m.len() - found, bytes)
    }
}

fn scene_stats(p: &ParsedNif) -> String {
    let tris: usize = p.shapes.iter().map(|s| s.mesh.triangles.len()).sum();
    let verts: usize = p.shapes.iter().map(|s| s.mesh.positions.len()).sum();
    let skinned = p.shapes.iter().filter(|s| s.material.skinned).count();
    let pbr = p
        .shapes
        .iter()
        .filter(|s| s.material.slsf2 & (1 << 23) != 0)
        .count();
    let blended = p.shapes.iter().filter(|s| s.material.blend()).count();
    let hidden = p.shapes.iter().filter(|s| s.material.hidden).count();
    let types: BTreeSet<u32> = p.shapes.iter().map(|s| s.material.shader_type).collect();
    format!(
        "shapes {} (hidden {hidden}), triangles {tris}, vertices {verts}, skinned shapes {skinned}, TruePBR shapes {pbr}, \
         blended shapes {blended}, shader types {types:?}, parse errors {}, warnings {}",
        p.shapes.len(),
        p.errors.len(),
        p.warnings.len()
    )
}

/// The renderer's `old_ll` test: an old Linear Lighting build gets no `ENABLE_LL` define on Lighting.hlsl.
fn old_ll(set: &SourceSet) -> bool {
    set.get(Path::new("Lighting.hlsl"))
        .is_ok_and(|s| s.text.contains("isDirLightLinear"))
}

/// The (VS, PS) define sets `Renderer::wanted` makes for every drawn shape of `p` (no projected layer, no skin
/// preview, Lit debug view), distinct pairs only.
fn scene_defines(
    p: &ParsedNif,
    features: &[String],
    old_ll: bool,
    deferred: bool,
) -> Vec<(Defines, Defines)> {
    let f: Vec<&str> = features
        .iter()
        .map(String::as_str)
        .filter(|d| !(old_ll && *d == "ENABLE_LL"))
        .collect();
    let mut out: Vec<(Defines, Defines)> = vec![];
    for s in p.shapes.iter().filter(|s| !s.material.hidden) {
        let pm = perm_material(&s.material);
        let d = deferred && pipe(&s.material).blend.is_none();
        let pair = (
            defines(&pm, Stage::Vertex, &f, d),
            defines(&pm, Stage::Pixel, &f, d),
        );
        if !out.contains(&pair) {
            out.push(pair);
        }
    }
    out
}

fn files_in(dir: &Path) -> usize {
    std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0)
}

/// Shapes by status after a load: (drawing, failed, pending).
fn statuses(r: &Renderer) -> (usize, usize, usize) {
    let s = r.shapes();
    let n = |f: fn(&ShapeStatus) -> bool| s.iter().filter(|x| f(&x.status)).count();
    (
        n(|x| *x == ShapeStatus::Drawing),
        n(|x| matches!(x, ShapeStatus::Failed { .. })),
        n(|x| matches!(x, ShapeStatus::Pending { .. })),
    )
}

#[derive(Default)]
struct Series {
    draw: Vec<f64>,
    gpu: Vec<f64>,
    readback: Vec<f64>,
    total: Vec<f64>,
}

fn cell(v: &mut [f64]) -> String {
    format!("{:.3} / {:.3}", q(v, 0.5), q(v, 0.95))
}

/// One device's numbers for one scene, as Markdown lines.
fn run_device(
    label: &str,
    device: &str,
    make_gpu: &dyn Fn() -> Result<Gpu, String>,
    parsed: &ParsedNif,
    tex: &Textures,
    frames: usize,
    warp_budget_s: f64,
) -> Result<Vec<String>, String> {
    let mut out = vec![];
    let resolve = |p: &str| tex.get(p);
    let scene = Scene::neutral();
    let cache = tempfile::tempdir().map_err(|e| e.to_string())?;

    // Cold: empty cache dir. The async split: `load_parsed` resolves, decodes and uploads every texture and queues
    // the programs; `settle` compiles them (FXC, both stages) and builds the draws (vertex buffers, cbuffers).
    let t = Instant::now();
    let gpu = make_gpu()?;
    let t_gpu = ms(t);
    let t = Instant::now();
    let mut r = Renderer::with_gpu(gpu, cache.path().to_path_buf())?;
    let t_new = ms(t);
    let adapter = r.gpu().adapter.clone();
    r.set_async_compile(true);
    let t = Instant::now();
    let rep = r.load_parsed(parsed, &resolve);
    let t_tex = ms(t);
    let t = Instant::now();
    r.settle();
    let t_settle = ms(t);
    let (drawing, failed, pending) = statuses(&r);
    let cold_files = files_in(cache.path());
    let t = Instant::now();
    r.render(&scene, RES[0].0, RES[0].1)?;
    let t_first_cold = ms(t);
    let first_files = files_in(cache.path());
    drop(r);

    // Warm: the same cache dir, a new renderer (programs from disk, textures again).
    let t = Instant::now();
    let mut r = Renderer::with_gpu(make_gpu()?, cache.path().to_path_buf())?;
    let t_new_warm = ms(t);
    let t = Instant::now();
    let rep_warm = r.load_parsed_sync(parsed, &resolve);
    let t_warm = ms(t);
    let t = Instant::now();
    r.render(&scene, RES[0].0, RES[0].1)?;
    let t_first_warm = ms(t);

    // Warm, no textures: every register gets its default (no DDS decode or upload).
    let mut r0 = Renderer::with_gpu(make_gpu()?, cache.path().to_path_buf())?;
    let t = Instant::now();
    r0.load_parsed_sync(parsed, &|_| None);
    let t_notex = ms(t);
    drop(r0);

    out.push(format!("### {device}: {adapter}\n"));
    out.push(format!(
        "- shapes drawing {drawing}, failed {failed}, pending {pending}; load report: missing textures {}, errors {}, \
         compile errors {}, warnings {}; features {:?}; deferred {}",
        rep.missing_textures.len(),
        rep.errors.len(),
        rep_warm.compile_errors.len(),
        rep.warnings.len(),
        r.features(),
        r.deferred()
    ));
    for e in rep.errors.iter().chain(&rep_warm.compile_errors).take(5) {
        out.push(format!("  - error: {e}"));
    }
    out.push(format!(
        "- cache files after the cold load {cold_files}, after its first frame {first_files} (the env prefilter CS)\n"
    ));
    out.push("| step | ms | how |".into());
    out.push("|---|---:|---|".into());
    let rows = [
        ("device create (`Gpu::new*`)", t_gpu, "wall, once"),
        ("`Renderer::with_gpu` (empty cache)", t_new, "wall, once"),
        (
            "cold `load_parsed` (async): textures decode+upload, program lookups",
            t_tex,
            "wall, once",
        ),
        (
            "cold `settle`: FXC compile of every permutation + draw build",
            t_settle,
            "wall, once",
        ),
        (
            "**cold load total** (load_parsed + settle)",
            t_tex + t_settle,
            "sum",
        ),
        (
            "cold first `render` 1280x720 (env prefilter CS compile, env build)",
            t_first_cold,
            "wall, once",
        ),
        (
            "`Renderer::with_gpu` (warm cache)",
            t_new_warm,
            "wall, once",
        ),
        (
            "**warm load** (`load_parsed_sync`, programs from disk cache)",
            t_warm,
            "wall, once",
        ),
        ("warm first `render` 1280x720", t_first_warm, "wall, once"),
        (
            "warm load with no textures (resolver returns None)",
            t_notex,
            "wall, once",
        ),
        (
            "≈ texture decode+upload (warm − no-textures)",
            t_warm - t_notex,
            "difference",
        ),
    ];
    for (k, v, how) in rows {
        out.push(format!("| {k} | {v:.1} | {how} |"));
    }

    out.push(String::new());
    out.push(format!(
        "Frames (`render()` on the warm renderer, `Scene::neutral()`, forward; ms median / p95; {WARMUP} warm-up frames):\n"
    ));
    out.push(
        "| res | n | draw_ms | gpu_ms | readback_ms | whole render() | covered px % | bytes/frame |".into(),
    );
    out.push("|---|---:|---|---|---|---|---:|---:|".into());
    for (w, h) in RES {
        let mut last = vec![];
        let mut warm_t = vec![];
        for i in 0..WARMUP {
            let t = Instant::now();
            last = r.render(&scene, w, h)?;
            warm_t.push(ms(t));
            // A slow WARP frame: three warm-ups are enough to estimate.
            if i >= 2 && warm_t[i] > 1000.0 {
                break;
            }
        }
        let est = q(&mut warm_t.clone(), 0.5);
        let n = if r.gpu().warp {
            frames.min(((warp_budget_s * 1e3 / est) as usize).max(10))
        } else {
            frames
        };
        let mut s = Series::default();
        for _ in 0..n {
            let t = Instant::now();
            let px = r.render(&scene, w, h)?;
            s.total.push(ms(t));
            let ft = r.last_timing();
            s.draw.push(ft.draw_ms);
            s.gpu.push(ft.gpu_ms);
            s.readback.push(ft.readback_ms);
            black_box(&px);
        }
        if let (Ok(dir), true) = (std::env::var("SDC_R3D_FRAME_OUT"), (w, h) == RES[0]) {
            let file = Path::new(&dir).join(format!("{label}-{device}-{w}x{h}.rgba"));
            std::fs::write(&file, &last).map_err(|e| format!("{}: {e}", file.display()))?;
        }
        let bg = [last[0], last[1], last[2]];
        let covered = last.chunks(4).filter(|p| p[..3] != bg).count();
        out.push(format!(
            "| {w}x{h} | {n} | {} | {} | {} | {} | {:.1} | {} |",
            cell(&mut s.draw),
            cell(&mut s.gpu),
            cell(&mut s.readback),
            cell(&mut s.total),
            100.0 * covered as f64 / (w * h) as f64,
            w * h * 4
        ));
    }
    out.push(String::new());
    Ok(out)
}

/// Median of 200 copies of `w*h*4` bytes: into a reused buffer, and into a new `Vec` (what `render` returns).
fn copy_floor() -> Vec<String> {
    let mut out = vec![
        "| res | bytes | copy into reused buffer, ms median | `to_vec` (alloc + copy), ms median |"
            .into(),
        "|---|---:|---:|---:|".into(),
    ];
    for (w, h) in RES {
        let n = (w * h * 4) as usize;
        let src = vec![7u8; n];
        let mut dst = vec![0u8; n];
        let (mut a, mut b) = (vec![], vec![]);
        for _ in 0..200 {
            let t = Instant::now();
            dst.copy_from_slice(black_box(&src));
            black_box(&dst);
            a.push(ms(t));
            let t = Instant::now();
            black_box(black_box(&src).to_vec());
            b.push(ms(t));
        }
        out.push(format!(
            "| {w}x{h} | {n} | {:.3} | {:.3} |",
            q(&mut a, 0.5),
            q(&mut b, 0.5)
        ));
    }
    out
}

/// Standalone FXC compile of the scene's forward permutations into an empty dir, then a renderer loading the scene
/// on that dir: if it compiles nothing new, these define sets are the renderer's.
fn compile_check(parsed: &ParsedNif, tex: &Textures) -> Result<Vec<String>, String> {
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let set = SourceSet::new(ShaderSource::Bundled);
    let t = Instant::now();
    let src = set.get(Path::new("Lighting.hlsl"))?;
    let t_expand = ms(t);
    let features = preview_defines(defaults(), true);
    let pairs = scene_defines(parsed, &features, old_ll(&set), false);
    let (mut vs_ms, mut ps_ms) = (vec![], vec![]);
    for (vs, ps) in &pairs {
        let t = Instant::now();
        render3d::compile::compile(&src, vs, "vs_5_0", dir.path())?;
        vs_ms.push(ms(t));
        let t = Instant::now();
        render3d::compile::compile(&src, ps, "ps_5_0", dir.path())?;
        ps_ms.push(ms(t));
    }
    let before = files_in(dir.path());
    let mut r = Renderer::with_gpu(Gpu::new_warp()?, dir.path().to_path_buf())?;
    r.load_parsed_sync(parsed, &|p| tex.get(p));
    let after = files_in(dir.path());
    let sum = |v: &[f64]| v.iter().sum::<f64>();
    Ok(vec![
        format!(
            "- standalone `compile::compile` (FXC via D3DCompile, OPTIMIZATION_LEVEL3, one thread): {} distinct forward \
             programs; expand Lighting.hlsl {t_expand:.1} ms (once per renderer); VS total {:.0} ms (median {:.0}); \
             PS total {:.0} ms (median {:.0}, max {:.0}); sum {:.0} ms",
            pairs.len(),
            sum(&vs_ms),
            q(&mut vs_ms.clone(), 0.5),
            sum(&ps_ms),
            q(&mut ps_ms.clone(), 0.5),
            q(&mut ps_ms.clone(), 1.0),
            sum(&vs_ms) + sum(&ps_ms)
        ),
        format!(
            "- cross-check: cache files after the standalone compile {before}, after a renderer load on that dir \
             {after} ({})",
            if before == after {
                "no new compile: the define sets match the renderer's"
            } else {
                "the renderer compiled more: the define sets do NOT match"
            }
        ),
    ])
}

#[test]
#[ignore = "owner data: SDC_ACHERUS; real GPU"]
fn render_backend_baseline() {
    let Some(acherus) = std::env::var_os("SDC_ACHERUS").map(PathBuf::from) else {
        println!("skipped: set SDC_ACHERUS");
        return;
    };
    if let Ok(list) = std::env::var("SDC_R3D_PROBE") {
        for rel in list.split(';').filter(|s| !s.is_empty()) {
            let path = acherus.join(rel);
            match std::fs::read(&path) {
                Ok(b) => println!("{rel}: {}", scene_stats(&parse(&b))),
                Err(e) => println!("{rel}: {e}"),
            }
        }
        return;
    }
    let frames = env_num("SDC_R3D_FRAMES", 200);
    let warp_budget_s = env_num("SDC_R3D_WARP_BUDGET_S", 150) as f64;
    let mut md = vec![
        format!(
            "## Measurement A (frames {frames}, WARP budget {warp_budget_s} s per resolution)\n"
        ),
        "### Readback floor (CPU copy of the frame bytes alone)\n".to_string(),
    ];
    md.extend(copy_floor());
    md.push(String::new());
    for (label, modname, rel) in SCENES {
        let path = acherus.join("mods").join(modname).join(rel);
        let nif = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut pt = vec![];
        for _ in 0..20 {
            let t = Instant::now();
            black_box(parse(black_box(&nif)));
            pt.push(ms(t));
        }
        let parsed = parse(&nif);
        let tex = Textures::new(&acherus, modname);
        // Untimed priming load: fills the texture memo from disk (and loads the compiler DLL), on its own cache dir.
        let prime = tempfile::tempdir().unwrap();
        let mut r =
            Renderer::with_gpu(Gpu::new_warp().unwrap(), prime.path().to_path_buf()).unwrap();
        let rep = r.load_parsed_sync(&parsed, &|p| tex.get(p));
        drop(r);
        let (found, missing, bytes) = tex.summary();
        md.push(format!("## Scene `{label}`: mods\\{modname}\\{rel}\n"));
        md.push(format!(
            "- {}; NIF {} bytes",
            scene_stats(&parsed),
            nif.len()
        ));
        md.push(format!(
            "- textures: {found} files found, {} MiB total; {missing} asked but not found (missing_textures {:?})",
            bytes as f64 / 1048576.0,
            rep.missing_textures
        ));
        md.push(format!(
            "- NIF parse (`render3d::parse` on bytes in memory): median {:.2} ms, p95 {:.2} ms of 20",
            q(&mut pt.clone(), 0.5),
            q(&mut pt, 0.95)
        ));
        let features = preview_defines(defaults(), true);
        let set = SourceSet::new(ShaderSource::Bundled);
        let fwd = scene_defines(&parsed, &features, old_ll(&set), false).len();
        let dfr = scene_defines(&parsed, &features, old_ll(&set), true).len();
        md.push(format!(
            "- distinct programs (VS+PS pairs): forward {fwd}, deferred {dfr}"
        ));
        match compile_check(&parsed, &tex) {
            Ok(lines) => md.extend(lines),
            Err(e) => md.push(format!("- compile check failed: {e}")),
        }
        md.push(String::new());
        for (device, make) in [
            ("hardware", &Gpu::new as &dyn Fn() -> Result<Gpu, String>),
            ("WARP", &Gpu::new_warp),
        ] {
            match run_device(label, device, make, &parsed, &tex, frames, warp_budget_s) {
                Ok(lines) => md.extend(lines),
                Err(e) => md.push(format!("### {device}: failed: {e}\n")),
            }
        }
        println!("{}", md.join("\n"));
        md.clear();
    }
}

/// `K` → `K=1`, as `compile::to_defines` reads a bare name.
fn kv(d: &str) -> String {
    if d.contains('=') {
        d.to_string()
    } else {
        format!("{d}=1")
    }
}

fn pairs(d: &Defines) -> Vec<String> {
    d.0.iter().map(|(k, v)| format!("{k}={v}")).collect()
}

/// The repo-owned backdrop shader as `background.rs` compiles it (the source is private there).
fn background_source() -> String {
    let file = include_str!("../src/background.rs");
    let start = file
        .find("const SOURCE: &str = r#\"")
        .expect("background SOURCE")
        + 24;
    let len = file[start..].find("\"#;").expect("end of SOURCE");
    format!(
        "#define YAW ({:.6})\n{}",
        render3d::hdri::YAW,
        &file[start..start + len]
    )
}

/// (rel, target, entry, sorted defines).
type PermKey = (String, String, String, Vec<String>);

#[derive(Default)]
struct Perms {
    /// Key → (groups, defines in compile order).
    map: BTreeMap<PermKey, (BTreeSet<String>, Vec<String>)>,
}

impl Perms {
    fn add(&mut self, group: &str, rel: &str, target: &str, entry: &str, defines: Vec<String>) {
        let mut sorted = defines.clone();
        sorted.sort();
        let e = self
            .map
            .entry((rel.into(), target.into(), entry.into(), sorted))
            .or_insert_with(|| (BTreeSet::new(), defines));
        e.0.insert(group.to_string());
    }

    fn count(&self, prefix: &str) -> usize {
        self.map
            .values()
            .filter(|(g, _)| g.iter().any(|g| g.starts_with(prefix)))
            .count()
    }
}

#[test]
#[ignore = "owner data: SDC_R3D_DUMP with SDC_GOLDEN and/or SDC_ACHERUS"]
fn dump_permutations() {
    let Some(out) = std::env::var_os("SDC_R3D_DUMP").map(PathBuf::from) else {
        println!("skipped: set SDC_R3D_DUMP");
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    let source = match std::env::var_os("SDC_R3D_SHADERS") {
        Some(p) => ShaderSource::Folder(PathBuf::from(p)),
        None => ShaderSource::Bundled,
    };
    println!("source: {source:?}");
    let set = SourceSet::new(source);
    let features = preview_defines(defaults(), true);
    let ll_old = old_ll(&set);
    println!("default features {features:?}; old LL {ll_old}");
    let mut perms = Perms::default();
    let lighting = "Lighting.hlsl";
    let composite = render3d::composite::SOURCE;
    let prefilter = render3d::composite::Prefilter::SOURCE;

    // (a) What the golden replay compiles for each subject (`render_replay`, `replay_composite`).
    if let Some(root) = std::env::var_os("SDC_GOLDEN").map(PathBuf::from) {
        let subjects = golden::subjects(&root, PROFILES).unwrap();
        let (mut loaded, mut failed) = (0, BTreeMap::<String, usize>::new());
        for s in &subjects {
            let r = match render3d::load_fixture(&s.dir) {
                Ok(r) => r,
                Err(e) => {
                    let code = if e.contains("defines null") {
                        "defines null".to_string()
                    } else {
                        e.replace(&s.dir.display().to_string(), "<subject>")
                            .chars()
                            .take(160)
                            .collect()
                    };
                    *failed.entry(code).or_default() += 1;
                    continue;
                }
            };
            loaded += 1;
            let Some(ps_list) = r.defines.first() else {
                continue;
            };
            let vs = replay::vs_defines(ps_list, &r.mesh);
            let ps: Vec<String> = std::iter::once("PSHADER".to_string())
                .chain(ps_list.iter().cloned())
                .collect();
            perms.add(
                "a",
                lighting,
                "vs_5_0",
                "main",
                vs.iter().map(|d| kv(d)).collect(),
            );
            perms.add(
                "a",
                lighting,
                "ps_5_0",
                "main",
                ps.iter().map(|d| kv(d)).collect(),
            );
            if let Some(rc) = &r.composite {
                let mut d = vec!["CSHADER".to_string()];
                d.extend(rc.defines_inferred.iter().cloned());
                d.extend(
                    ps_list
                        .iter()
                        .filter(|d| *d == "ENABLE_LL" || d.starts_with("CS_GAME_GAMMA"))
                        .cloned(),
                );
                perms.add(
                    "a",
                    composite,
                    "cs_5_0",
                    "main",
                    d.iter().map(|d| kv(d)).collect(),
                );
            }
        }
        println!(
            "(a) golden {}: subjects {} (profiles {PROFILES:?}), fixtures loaded {loaded}, not loaded {failed:?}",
            root.display(),
            subjects.len()
        );
    } else {
        println!("(a) skipped: set SDC_GOLDEN");
    }

    // (b) The renderer's define sets for every shape of the three scenes, forward and deferred.
    if let Some(acherus) = std::env::var_os("SDC_ACHERUS").map(PathBuf::from) {
        for (label, modname, rel) in SCENES {
            let path = acherus.join("mods").join(modname).join(rel);
            let parsed = parse(&std::fs::read(&path).unwrap());
            for deferred in [false, true] {
                for (vs, ps) in scene_defines(&parsed, &features, ll_old, deferred) {
                    let g = format!("b:{label}");
                    perms.add(&g, lighting, "vs_5_0", "main", pairs(&vs));
                    perms.add(&g, lighting, "ps_5_0", "main", pairs(&ps));
                }
            }
        }
    } else {
        println!("(b) skipped: set SDC_ACHERUS");
    }

    // (c) The built-in compute and backdrop shaders (builtin.rs and hdri.rs compile nothing).
    let cs: Vec<String> = std::iter::once("CSHADER".to_string())
        .chain(features.iter().cloned())
        .map(|d| kv(&d))
        .collect();
    perms.add("c", composite, "cs_5_0", "main", cs.clone());
    perms.add("c", prefilter, "cs_5_0", "main", cs);
    perms.add("c", "background.hlsl", "vs_5_0", "vs", vec![]);
    perms.add("c", "background.hlsl", "ps_5_0", "ps", vec![]);

    let mut texts: HashMap<String, Result<String, String>> = HashMap::new();
    let mut written = 0;
    let mut missing = BTreeMap::<String, usize>::new();
    for (n, ((rel, target, entry, _), (groups, defs))) in perms.map.iter().enumerate() {
        let text = texts
            .entry(rel.clone())
            .or_insert_with(|| {
                if rel == "background.hlsl" {
                    Ok(background_source())
                } else {
                    set.get(Path::new(rel)).map(|s| s.text.to_string())
                }
            })
            .clone();
        let Ok(text) = text else {
            *missing.entry(rel.clone()).or_default() += 1;
            continue;
        };
        std::fs::write(out.join(format!("{n}.hlsl")), text).unwrap();
        let json = serde_json::json!({
            "rel": rel, "target": target, "entry": entry, "defines": defs, "groups": groups,
        });
        std::fs::write(
            out.join(format!("{n}.json")),
            serde_json::to_string_pretty(&json).unwrap(),
        )
        .unwrap();
        written += 1;
    }
    println!(
        "distinct permutations {} (written {written}, source missing {missing:?}); with group a {}, b {}, c {}; \
         b:static {}, b:skinned {}, b:pbr {}; in a and b {}",
        perms.map.len(),
        perms.count("a"),
        perms.count("b"),
        perms.count("c"),
        perms.count("b:static"),
        perms.count("b:skinned"),
        perms.count("b:pbr"),
        perms
            .map
            .values()
            .filter(|(g, _)| g.contains("a") && g.iter().any(|g| g.starts_with("b")))
            .count()
    );
    for (rel, target) in [
        (lighting, "vs_5_0"),
        (lighting, "ps_5_0"),
        (composite, "cs_5_0"),
        (prefilter, "cs_5_0"),
    ] {
        let c = perms
            .map
            .keys()
            .filter(|k| k.0 == rel && k.1 == target)
            .count();
        println!("  {rel} {target}: {c}");
    }
}
