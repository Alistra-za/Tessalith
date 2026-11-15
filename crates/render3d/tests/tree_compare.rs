//! Note ID41: tree A (the CS 1.9 pin) against tree B (the CS 2.0 build) on the golden captures, report-only. Every
//! subject golden.rs runs (`golden::prepare`) is replayed deferred on both trees from the same capture: the captured
//! constants go to tree B rebuilt by variable name (`remap`), because tree B's FeatureData is laid out differently,
//! and with Terrain Blending off when the capture has no scene depth at t17 (`terrain_blending_off`). One row per
//! subject: B against A per MRT and on the composite (max, mean, over `diff_tol`), and for TRUE_PBR subjects a "B as
//! vanilla" pass with its own remap. Per group the summary lists the changed shader files the group's permutations
//! pull. The differences are rows, not assertions (pre-flight ruling): the test fails only when a subject does not
//! draw or a render fails on either tree (a composite error included; the MRTs are then rendered again without the
//! composite so the row is still written) or tree B's inputs cannot be built. Needs SDC_GOLDEN.
use render3d::compile::{compile, used_vars, CbLayout, CbVar, VarKind};
use render3d::golden::{self, Diff, Noise};
use render3d::permutation::Defines;
use render3d::replay::{cb_bytes, vs_defines};
use render3d::source::SourceSet;
use render3d::{Renderer, Replay, ReplayOut, ShaderSource, Tree};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use windows::core::PCSTR;
use windows::Win32::Graphics::Direct3D::Fxc::D3DPreprocess;
use windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D_SHADER_MACRO};

/// golden.rs's profiles: the set `golden-baseline.txt` checks on tree A.
const PROFILES: &[&str] = &["A", "A_RT", "A_RT2", "A_DC", "A_DC_N"];

/// Variables of `l` that are not a struct with members listed after it.
fn leaves(l: &CbLayout) -> Vec<&CbVar> {
    l.vars
        .iter()
        .filter(|v| {
            !l.vars.iter().any(|w| {
                w.name.len() > v.name.len()
                    && w.name.starts_with(&v.name)
                    && w.name.as_bytes()[v.name.len()] == b'.'
            })
        })
        .collect()
}

/// `raw`, laid out as `a`, rebuilt in layout `b`: each leaf of `b` takes the bytes of `a`'s leaf of the same name (the
/// shorter of the two sizes); everything else reads 0. Also returns `b`'s leaves `a` has no value for.
fn remap(raw: &[u8], a: &CbLayout, b: &CbLayout) -> (Vec<u8>, Vec<String>) {
    let src = cb_bytes(raw, a.size);
    let mut out = cb_bytes(&[], b.size);
    let from = leaves(a);
    let mut missing = vec![];
    for v in leaves(b) {
        let Some(w) = from.iter().find(|w| w.name == v.name) else {
            missing.push(v.name.clone());
            continue;
        };
        let n = w.size.min(v.size) as usize;
        let (s, d) = (w.offset as usize, v.offset as usize);
        if let (Some(bytes), Some(dst)) = (src.get(s..s + n), out.get_mut(d..d + n)) {
            dst.copy_from_slice(bytes);
        }
    }
    (out, missing)
}

fn to_defines(list: &[String]) -> Defines {
    Defines(
        list.iter()
            .map(|d| match d.split_once('=') {
                Some((k, v)) => (k.to_string(), v.to_string()),
                None => (d.clone(), "1".to_string()),
            })
            .collect(),
    )
}

/// The files whose text survives preprocessing `src` (an expanded source whose `#line` names are root-relative) with
/// `defines`: what the permutation pulls in, guarded includes resolved.
fn pulled(src: &str, defines: &[String]) -> Result<BTreeSet<String>, String> {
    let d = to_defines(defines);
    let names: Vec<(std::ffi::CString, std::ffi::CString)> =
        d.0.iter()
            .map(|(k, v)| {
                Ok((
                    std::ffi::CString::new(k.as_str()).map_err(|e| e.to_string())?,
                    std::ffi::CString::new(v.as_str()).map_err(|e| e.to_string())?,
                ))
            })
            .collect::<Result<_, String>>()?;
    let mut macros: Vec<D3D_SHADER_MACRO> = names
        .iter()
        .map(|(k, v)| D3D_SHADER_MACRO {
            Name: PCSTR(k.as_ptr() as _),
            Definition: PCSTR(v.as_ptr() as _),
        })
        .collect();
    macros.push(D3D_SHADER_MACRO::default());
    let (mut text, mut errs): (Option<ID3DBlob>, Option<ID3DBlob>) = (None, None);
    // SAFETY: the source, the macro array (NULL-terminated) and its strings outlive the call.
    let r = unsafe {
        D3DPreprocess(
            src.as_ptr() as _,
            src.len(),
            PCSTR(c"Lighting.hlsl".as_ptr() as _),
            Some(macros.as_ptr()),
            None,
            &mut text,
            Some(&mut errs),
        )
    };
    let blob_text = |b: &ID3DBlob| {
        // SAFETY: the blob owns GetBufferSize() bytes at GetBufferPointer() while `b` lives.
        let s = unsafe {
            std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize())
        };
        String::from_utf8_lossy(s).into_owned()
    };
    if let Err(e) = r {
        return Err(format!(
            "D3DPreprocess: {e} {}",
            errs.as_ref().map(blob_text).unwrap_or_default()
        ));
    }
    let text = blob_text(text.as_ref().ok_or("D3DPreprocess returned no text")?);
    let mut file = "Lighting.hlsl".to_string();
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("#line") {
            if let Some(name) = rest.split('"').nth(1) {
                file = name.replace('\\', "/");
            }
        } else if !t.is_empty() {
            out.insert(file.clone());
        }
    }
    Ok(out)
}

/// Every file of a bundled tree, root-relative with `/`, mapped to its text with all whitespace removed.
fn tree_files(tree: &str) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if p.extension().is_some_and(|x| x == "hlsl" || x == "hlsli") {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = String::from_utf8_lossy(&std::fs::read(&p).unwrap())
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                out.insert(rel, text);
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("shaders")
        .join(tree);
    let mut out = BTreeMap::new();
    walk(&root, &root, &mut out);
    out
}

/// Report group of a PS define list (the capture's technique defines).
fn group(defs: &[String]) -> &'static str {
    let has = |k: &str| defs.iter().any(|d| d.split('=').next() == Some(k));
    if has("TRUE_PBR") {
        "v1 PBR"
    } else if has("HAIR") {
        "hair"
    } else if has("FACEGEN") || has("FACEGEN_RGB_TINT") {
        "skin"
    } else if [
        "MULTI_LAYER_PARALLAX",
        "PARALLAX",
        "EYE",
        "LANDSCAPE",
        "LODLANDSCAPE",
        "LODOBJECTS",
        "TREE_ANIM",
        "SNOW",
        "WORLD_MAP",
    ]
    .iter()
    .any(|k| has(k))
    {
        "other"
    } else {
        "vanilla"
    }
}

/// The technique defines of a PS list (the CS feature defines every subject shares left out), for the row.
fn technique(defs: &[String]) -> String {
    const SHOWN: &[&str] = &[
        "TRUE_PBR",
        "GLINT",
        "HAIR",
        "FACEGEN",
        "FACEGEN_RGB_TINT",
        "SKINNED",
        "MODELSPACENORMALS",
        "ENVMAP",
        "PARALLAX",
        "MULTI_LAYER_PARALLAX",
        "SPECULAR",
        "SOFT_LIGHTING",
        "RIM_LIGHTING",
        "BACK_LIGHTING",
        "ANISO_LIGHTING",
        "DO_ALPHA_TEST",
    ];
    let t: Vec<&str> = defs
        .iter()
        .map(|d| d.split('=').next().unwrap_or(d))
        .filter(|k| SHOWN.contains(k))
        .collect();
    if t.is_empty() {
        "-".into()
    } else {
        t.join(" ")
    }
}

/// Masked pixels of any RT whose 4 channels differ from the seeded before-values: the draw wrote something.
fn drew(out: &ReplayOut, r: &Replay, mask: &[bool]) -> usize {
    let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    (0..mask.len())
        .filter(|&p| {
            mask[p]
                && out
                    .mrt
                    .iter()
                    .zip(&r.before)
                    .any(|(o, b)| bits(&o[4 * p..4 * p + 4]) != bits(&b[4 * p..4 * p + 4]))
        })
        .count()
}

/// Mean finite difference per channel of an RGBA target over the mask (non-finite differences left out of the count).
fn channel_means(b: &[f32], a: &[f32], mask: &[bool]) -> [f32; 4] {
    let mut sum = [0.0f32; 4];
    let mut n = [0usize; 4];
    for (p, _) in mask.iter().enumerate().filter(|(_, m)| **m) {
        for c in 0..4 {
            let d = (b[4 * p + c] - a[4 * p + c]).abs();
            if d.is_finite() {
                sum[c] += d;
                n[c] += 1;
            }
        }
    }
    std::array::from_fn(|c| sum[c] / n[c].max(1) as f32)
}

/// The channel (x, y, z, w) of an RGBA target with the largest mean difference over the mask.
fn channel(b: &[f32], a: &[f32], mask: &[bool]) -> char {
    let m = channel_means(b, a, mask);
    let c = (0..4).fold(0, |w, c| if m[c] > m[w] { c } else { w });
    ['x', 'y', 'z', 'w'][c]
}

fn cell(d: &Diff) -> String {
    if d.max_abs == 0.0 {
        "0".into()
    } else {
        format!(
            "{:.2e}/{:.1e}{}",
            d.max_abs,
            d.mean_abs,
            if d.over > 0.0 { "!" } else { "" }
        )
    }
}

/// B against A per RT and on the composite (when both have one), each with A's format tolerance and the subject's
/// noise limit.
fn compare(
    b: &ReplayOut,
    a: &ReplayOut,
    r: &Replay,
    mask: &[bool],
    noise: &Noise,
    name: &str,
) -> Vec<Diff> {
    let mut out: Vec<Diff> = (0..a.mrt.len())
        .map(|i| {
            let max = noise
                .limit(name, &format!("mrt{i}.f32"))
                .map_or(0.0, |l| l.1);
            golden::diff_tol(
                &b.mrt[i],
                &a.mrt[i],
                mask,
                golden::format_tol(&r.formats[i]).unwrap(),
                max,
            )
        })
        .collect();
    if let (Some(mb), Some(ma)) = (&b.main, &a.main) {
        let max = noise.limit(name, "mainrw_after.f32").map_or(0.0, |l| l.1);
        out.push(golden::diff_tol(
            mb,
            ma,
            mask,
            golden::format_tol("R16G16B16A16_FLOAT").unwrap(),
            max,
        ));
    }
    out
}

/// One compiled stage of a replay: entry file, define list, target, and the stage prefix of the capture's cbuffer files.
struct Stage<'a> {
    rel: &'a str,
    defs: &'a [String],
    target: &'a str,
    stage: &'a str,
}

/// Reflected cbuffers keyed as the capture's files (`ps_b1`), and the `cbuffer.var` names the bytecode reads.
type Layouts = (Vec<(String, CbLayout)>, BTreeSet<String>);

/// Captured cbuffer bytes keyed as the capture's files (`ps_b6`).
type CbMap = std::collections::HashMap<String, Vec<u8>>;

/// The reflected cbuffers of `st` on one tree.
fn layouts(src: &SourceSet, st: &Stage, cache: &Path) -> Result<Layouts, String> {
    let s = src.get(Path::new(st.rel))?;
    let c = compile(&s, &to_defines(st.defs), st.target, cache)?;
    let stage = st.stage;
    let used = used_vars(&c.bytecode)?
        .into_iter()
        .map(|(cb, v)| format!("{cb}.{v}"))
        .collect();
    Ok((
        c.reflection
            .cbuffers
            .into_iter()
            .map(|l| (format!("{stage}_b{}", l.slot), l))
            .collect(),
        used,
    ))
}

/// Rebuilds `cbs` (laid out for tree A) for tree B by name. Returns the B leaves the B bytecode reads whose value a
/// CS 1.9 capture cannot supply (zero, except `MaterialObjectRGBScale.w`: -1, "no fuzz roughness").
fn to_tree_b(
    cbs: &mut CbMap,
    st: &Stage,
    a: &SourceSet,
    b: &SourceSet,
    cache: &Path,
) -> Result<BTreeSet<String>, String> {
    let (la, _) = layouts(a, st, cache)?;
    let (lb, used) = layouts(b, st, cache)?;
    let mut unsupplied = BTreeSet::new();
    for (key, l) in &lb {
        let (Some((_, from)), Some(raw)) = (la.iter().find(|(k, _)| k == key), cbs.get(key)) else {
            continue;
        };
        if from == l {
            continue;
        }
        let (mut bytes, missing) = remap(raw, from, l);
        let top = |name: &str| format!("{}.{}", l.name, name.split('.').next().unwrap_or(name));
        unsupplied.extend(
            missing
                .iter()
                .filter(|m| used.contains(&top(m)))
                .map(|m| format!("{key} {}.{m}", l.name)),
        );
        // CS 2.0's float4 MaterialObjectRGBScale: .w is the fuzz roughness, negative = none (Lighting.hlsl:570).
        let (va, vb) = (
            from.vars
                .iter()
                .find(|v| v.name == "MaterialObjectRGBScale"),
            l.vars.iter().find(|v| v.name == "MaterialObjectRGBScale"),
        );
        if let (Some(va), Some(vb)) = (va, vb) {
            if va.size < vb.size && vb.size >= 16 {
                let at = vb.offset as usize + 12;
                bytes[at..at + 4].copy_from_slice(&(-1.0f32).to_le_bytes());
                unsupplied.insert(format!("{key} {}.MaterialObjectRGBScale.w = -1", l.name));
            }
        }
        cbs.insert(key.clone(), bytes);
    }
    Ok(unsupplied)
}

/// Sets tree B's `terrainBlendingSettings.Enabled` in the PS FeatureData of `cbs` to 0; true when it was on.
fn terrain_blending_off(
    cbs: &mut CbMap,
    ps: &[String],
    b: &SourceSet,
    cache: &Path,
) -> Result<bool, String> {
    let st = Stage {
        rel: "Lighting.hlsl",
        defs: ps,
        target: "ps_5_0",
        stage: "ps",
    };
    let (lb, _) = layouts(b, &st, cache)?;
    for (key, l) in &lb {
        let Some(v) = l
            .vars
            .iter()
            .find(|v| v.name.ends_with("terrainBlendingSettings.Enabled"))
        else {
            continue;
        };
        let at = v.offset as usize;
        if let Some(w) = cbs.get_mut(key).and_then(|c| c.get_mut(at..at + 4)) {
            let on = w.iter().any(|x| *x != 0);
            w.fill(0);
            return Ok(on);
        }
    }
    Ok(false)
}

/// Tree B's inputs for `r`, built from the capture's tree-A constants `cb_a` (draw) and `ccb_a` (composite) for the
/// current `r.defines`: every cbuffer rebuilt by name, and Terrain Blending off when the capture has no t17 depth.
/// Returns the B inputs a CS 1.9 capture cannot supply, and whether Terrain Blending was switched off.
fn to_b_inputs(
    r: &mut Replay,
    cb_a: &CbMap,
    ccb_a: Option<&CbMap>,
    a: &SourceSet,
    b: &SourceSet,
    cache: &Path,
) -> Result<(BTreeSet<String>, bool), String> {
    r.cbuffers = cb_a.clone();
    let ps = r.defines[0].clone();
    let ps_full: Vec<String> = std::iter::once("PSHADER".to_string())
        .chain(ps.iter().cloned())
        .collect();
    let vs = vs_defines(&ps, &r.mesh);
    let mut gaps = BTreeSet::new();
    for (defs, target, stage) in [(&ps_full, "ps_5_0", "ps"), (&vs, "vs_5_0", "vs")] {
        let st = Stage {
            rel: "Lighting.hlsl",
            defs,
            target,
            stage,
        };
        gaps.extend(
            to_tree_b(&mut r.cbuffers, &st, a, b, cache)
                .map_err(|e| format!("{stage} remap: {e}"))?,
        );
    }
    // Tree B's objects read the scene depth at t17 for Terrain Blending (Lighting.hlsl:1989-2007), which a CS 1.9
    // capture never bound: unbound it reads 0, every pixel counts as buried and its normal bends up. With the real
    // depth an object above the terrain is not buried, so the feature is switched off for the B pass instead.
    let off = !r.textures.contains_key(&17)
        && terrain_blending_off(&mut r.cbuffers, &ps_full, b, cache)
            .map_err(|e| format!("Terrain Blending: {e}"))?;
    if let (Some(rc), Some(c)) = (r.composite.as_mut(), ccb_a) {
        rc.cbuffers = c.clone();
        // The define list render_replay compiles the composite with (replay.rs replay_composite).
        let mut cd = vec!["CSHADER".to_string()];
        cd.extend(rc.defines_inferred.iter().cloned());
        cd.extend(
            ps.iter()
                .filter(|d| *d == "ENABLE_LL" || d.starts_with("CS_GAME_GAMMA"))
                .cloned(),
        );
        let st = Stage {
            rel: "DeferredCompositeCS.hlsl",
            defs: &cd,
            target: "cs_5_0",
            stage: "cs",
        };
        gaps.extend(
            to_tree_b(&mut rc.cbuffers, &st, a, b, cache)
                .map_err(|e| format!("composite remap: {e}"))?,
        );
    }
    Ok((gaps, off))
}

/// The names `#ifdef`, `#ifndef` and `defined(...)` test in the directive lines of `text`.
fn tested(text: &str) -> BTreeSet<String> {
    let ident = |s: &str| -> String {
        s.trim_start()
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect()
    };
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let Some(d) = line.trim_start().strip_prefix('#') else {
            continue;
        };
        let d = d.trim_start();
        if let Some(rest) = d.strip_prefix("ifndef").or_else(|| d.strip_prefix("ifdef")) {
            out.insert(ident(rest));
        }
        for (i, _) in d.match_indices("defined") {
            let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
            if d[..i].chars().next_back().is_some_and(word) {
                continue;
            }
            let rest = d[i + "defined".len()..].trim_start();
            out.insert(ident(rest.strip_prefix('(').unwrap_or(rest)));
        }
    }
    out.remove("");
    out
}

/// Replays `r` on the renderer's tree; on a composite error retries without the composite and says why.
fn replay(ren: &mut Renderer, r: &mut Replay) -> Result<(ReplayOut, Option<String>), String> {
    match ren.render_replay(r, true) {
        Ok(o) => Ok((o, None)),
        Err(e) if r.composite.is_some() => {
            let rc = r.composite.take();
            let out = ren.render_replay(r, true);
            r.composite = rc;
            Ok((out?, Some(e)))
        }
        Err(e) => Err(e),
    }
}

fn short(e: &str) -> String {
    let e = e.replace('\n', " ");
    if e.chars().count() > 160 {
        format!("{}...", e.chars().take(160).collect::<String>())
    } else {
        e
    }
}

#[derive(Default)]
struct Group {
    subjects: usize,
    same: usize,
    within: usize,
    differs: usize,
    worst: Option<(String, f32)>,
    /// Subjects over the tolerance per target (the MRTs, then main).
    over: Vec<usize>,
    changed: BTreeSet<String>,
    removed: BTreeSet<String>,
    inert: BTreeSet<String>,
}

#[test]
#[ignore]
fn render_tree_b_vs_a_report() {
    let Ok(root) = std::env::var("SDC_GOLDEN") else {
        println!("skipped: set SDC_GOLDEN");
        return;
    };
    let root = Path::new(&root);
    let cache = tempfile::tempdir().unwrap();
    let mut ren = Renderer::with_source(ShaderSource::Bundled, cache.path().to_path_buf()).unwrap();
    let (src_a, src_b) = (
        SourceSet::new(ShaderSource::Bundled),
        SourceSet::new(ShaderSource::BundledB),
    );
    let lighting = |s: &SourceSet| s.get(Path::new("Lighting.hlsl")).unwrap().text.clone();
    let (text_a, text_b) = (lighting(&src_a), lighting(&src_b));
    let noise = Noise::load(
        &root.join("noise_Skyrim_AE_1.6.1170_frame207657_Skyrim_AE_1.6.1170_frame207658.json"),
    )
    .unwrap();

    // Which files differ: content (whitespace ignored), whitespace only, A only, B only.
    let (fa, fb) = (tree_files("cs"), tree_files("cs2"));
    let raw = |t: &str, f: &str| {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("shaders")
                .join(t)
                .join(f),
        )
        .unwrap()
    };
    let changed: BTreeSet<String> = fa
        .iter()
        .filter(|(f, t)| fb.get(*f).is_some_and(|u| u != *t))
        .map(|(f, _)| f.clone())
        .collect();
    let ws_only: Vec<&String> = fa
        .iter()
        .filter(|(f, t)| fb.get(*f) == Some(*t) && raw("cs", f) != raw("cs2", f))
        .map(|(f, _)| f)
        .collect();
    let a_only: BTreeSet<String> = fa
        .keys()
        .filter(|f| !fb.contains_key(*f))
        .cloned()
        .collect();
    let b_only: BTreeSet<String> = fb
        .keys()
        .filter(|f| !fa.contains_key(*f))
        .cloned()
        .collect();
    let mut t = format!(
        "shader files (.hlsl/.hlsli): A {}, B {}; {} shared files differ in bytes: {} changed in content, {} whitespace only {ws_only:?}; A only {a_only:?}, B only {b_only:?}\nchanged: {changed:?}\n\n",
        fa.len(),
        fb.len(),
        changed.len() + ws_only.len(),
        changed.len(),
        ws_only.len()
    );
    t.push_str("capture | subject | profile | group | technique | masked px | A vs capture (RTs over) | B vs A RT0..RT7 max/mean (! = over tol; x-w = channel with the largest mean difference) | main | max | mean | verdict | B as vanilla (TRUE_PBR) | notes\n");

    let mut groups: BTreeMap<&str, Group> = BTreeMap::new();
    let mut unsupplied: BTreeMap<String, usize> = BTreeMap::new();
    let mut b_warnings: BTreeMap<String, usize> = BTreeMap::new();
    let mut skipped: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    // Pre-flight ruling: each is a subject that did not draw, a render error on either tree (composite included) or
    // tree B inputs that could not be built.
    let mut failures: Vec<String> = vec![];
    let (mut run, mut blending_off) = (0usize, 0usize);
    let (tested_a, tested_b) = (tested(&text_a), tested(&text_b));
    for s in golden::subjects(root, PROFILES).unwrap() {
        let name = s.dir.file_name().unwrap().to_string_lossy().into_owned();
        let who = format!("{}/{name}", s.capture);
        // golden.rs's skip rules: nothing to replay.
        let golden::Ready {
            replay: mut r,
            mask,
            masked,
        } = match golden::prepare(&s.dir) {
            Ok(p) => p,
            Err(k) => {
                let detail = k
                    .error
                    .map_or(who.clone(), |e| format!("{who}: {}", short(&e)));
                skipped.entry(k.code).or_default().push(detail);
                continue;
            }
        };
        let n = mask.len();
        run += 1;
        let ps: Vec<String> = r.defines[0].clone();
        let ps_full: Vec<String> = std::iter::once("PSHADER".to_string())
            .chain(ps.iter().cloned())
            .collect();
        let vs = vs_defines(&ps, &r.mesh);
        let grp = group(&ps);
        let mut notes = vec![];
        // The capture's constants, laid out for tree A: every tree B pass rebuilds its own from these.
        let (cb_a, ccb_a) = (
            r.cbuffers.clone(),
            r.composite.as_ref().map(|c| c.cbuffers.clone()),
        );

        // Tree A, and A against the capture.
        ren.set_tree(Tree::A);
        let a = match replay(&mut ren, &mut r) {
            Ok((o, ce)) => {
                if let Some(e) = ce {
                    failures.push(format!("{who}: A composite error {}", short(&e)));
                    notes.push("A composite failed (MRTs only)".to_string());
                }
                o
            }
            Err(e) => {
                failures.push(format!("{who}: A replay error {}", short(&e)));
                continue;
            }
        };
        let rts = a.mrt.len();
        let cap_over = (0..rts)
            .filter(|&i| {
                let file = format!("mrt{i}.f32");
                let after = golden::read_f32(&s.dir.join(&file), n * 4).unwrap();
                let max = noise.limit(&name, &file).map_or(0.0, |l| l.1);
                let tol = golden::format_tol(&r.formats[i]).unwrap();
                golden::diff_tol(&a.mrt[i], &after, &mask, tol, max).over > 0.0
            })
            .count();

        // Tree B: the same captured values, rebuilt in tree B's layouts.
        let (gaps, off) =
            match to_b_inputs(&mut r, &cb_a, ccb_a.as_ref(), &src_a, &src_b, cache.path()) {
                Ok(x) => x,
                Err(e) => {
                    failures.push(format!("{who}: tree B inputs: {}", short(&e)));
                    continue;
                }
            };
        blending_off += off as usize;
        for g in gaps {
            *unsupplied.entry(g).or_default() += 1;
        }
        ren.set_tree(Tree::B);
        let b = match replay(&mut ren, &mut r) {
            Ok((o, ce)) => {
                if let Some(e) = ce {
                    failures.push(format!("{who}: B composite error {}", short(&e)));
                    notes.push("B composite failed (MRTs only)".to_string());
                }
                o
            }
            Err(e) => {
                failures.push(format!("{who}: B replay error {}", short(&e)));
                continue;
            }
        };
        let (da, db) = (drew(&a, &r, &mask), drew(&b, &r, &mask));
        if da == 0 || db == 0 {
            failures.push(format!("{who}: drew A {da} px, B {db} px of {masked}"));
        }
        for w in b.warnings.iter().filter(|w| !a.warnings.contains(w)) {
            *b_warnings.entry(w.clone()).or_default() += 1;
        }
        let d = compare(&b, &a, &r, &mask, &noise, &name);

        // TRUE_PBR subjects again on tree B with TRUE_PBR and GLINT dropped: the capture's inputs, remapped for the
        // vanilla permutation's layouts, through B's vanilla path (an approximation of CS 2.0 handing a v1 shape to
        // the vanilla reader, Note ID33).
        let vanilla = if grp == "v1 PBR" {
            let keep = r.defines[0].clone();
            r.defines[0].retain(|d| d != "TRUE_PBR" && d != "GLINT");
            let v = match to_b_inputs(&mut r, &cb_a, ccb_a.as_ref(), &src_a, &src_b, cache.path())
                .and_then(|_| replay(&mut ren, &mut r))
            {
                Ok((o, ce)) => {
                    if let Some(e) = ce {
                        failures.push(format!("{who}: B as vanilla composite error {}", short(&e)));
                    }
                    let dv = compare(&o, &a, &r, &mask, &noise, &name);
                    let max = dv.iter().map(|x| x.max_abs).fold(0.0, f32::max);
                    let mean = dv[..rts].iter().map(|x| x.mean_abs).sum::<f32>() / rts as f32;
                    format!("{max:.2e}/{mean:.1e} (RT0 {})", cell(&dv[0]))
                }
                Err(e) => {
                    failures.push(format!("{who}: B as vanilla: {}", short(&e)));
                    format!("error {}", short(&e))
                }
            };
            r.defines[0] = keep;
            v
        } else {
            "-".into()
        };

        let max = d.iter().map(|x| x.max_abs).fold(0.0, f32::max);
        let mean = d[..rts].iter().map(|x| x.mean_abs).sum::<f32>() / rts as f32;
        let verdict = if max == 0.0 {
            "same"
        } else if d.iter().all(|x| x.over == 0.0) {
            "within tol"
        } else {
            "DIFFERS"
        };
        let mut row = format!(
            "{} | {name} | {} | {grp} | {} | {masked} | {cap_over} |",
            s.capture,
            s.profile,
            technique(&ps)
        );
        for (i, x) in d[..rts].iter().enumerate() {
            let _ = write!(row, " {}", cell(x));
            if x.max_abs > 0.0 {
                row.push(channel(&b.mrt[i], &a.mrt[i], &mask));
            }
        }
        let _ = writeln!(
            row,
            " | {} | {max:.2e} | {mean:.1e} | {verdict} | {vanilla} | {}",
            d.get(rts).map_or("-".into(), cell),
            notes.join("; ")
        );
        print!("{row}");
        t.push_str(&row);

        // What this permutation pulls from the changed files (tree B) and from the files tree B lacks (tree A).
        let g = groups.entry(grp).or_default();
        g.subjects += 1;
        match verdict {
            "same" => g.same += 1,
            "within tol" => g.within += 1,
            _ => g.differs += 1,
        }
        if g.worst.as_ref().is_none_or(|w| max > w.1) {
            g.worst = Some((who.clone(), max));
        }
        if g.over.len() < d.len() {
            g.over.resize(d.len(), 0);
        }
        for (o, x) in g.over.iter_mut().zip(&d) {
            *o += (x.over > 0.0) as usize;
        }
        for defs in [&ps_full, &vs] {
            g.changed.extend(
                pulled(&text_b, defs)
                    .unwrap()
                    .into_iter()
                    .filter(|f| changed.contains(f) || b_only.contains(f)),
            );
            g.removed.extend(
                pulled(&text_a, defs)
                    .unwrap()
                    .into_iter()
                    .filter(|f| a_only.contains(f)),
            );
        }
        // Defines this subject sets that tree A's source tests and tree B's never does.
        g.inert.extend(
            ps.iter()
                .map(|d| d.split('=').next().unwrap_or(d))
                .filter(|k| tested_a.contains(*k) && !tested_b.contains(*k))
                .map(str::to_string),
        );
    }

    let mut summary = format!(
        "\nsubjects run {run}; skipped {}\n",
        skipped.values().map(Vec::len).sum::<usize>()
    );
    for (why, list) in &skipped {
        let _ = writeln!(summary, "skipped {why}: {}", list.len());
        for w in list {
            let _ = writeln!(summary, "  {w}");
        }
    }
    for (name, g) in &groups {
        let _ = writeln!(
            summary,
            "\ngroup {name}: {} subjects; same {}, within tol {}, differs {}; worst {:?}\n  over tol per target RT0..RT7, main: {:?}\n  changed files pulled (tree B): {:?}\n  files only tree A has, pulled (tree A): {:?}\n  defines tree A tests and tree B ignores: {:?}",
            g.subjects,
            g.same,
            g.within,
            g.differs,
            g.worst,
            g.over,
            g.changed,
            g.removed,
            g.inert
        );
    }
    let _ = writeln!(
        summary,
        "\ntree B inputs a CS 1.9 capture cannot supply (read by B; subjects):"
    );
    for (k, c) in &unsupplied {
        let _ = writeln!(summary, "  {k}: {c}");
    }
    let _ = writeln!(
        summary,
        "tree B Terrain Blending switched off (it was on; the capture has no t17 depth): {blending_off} subjects"
    );
    let _ = writeln!(
        summary,
        "tree B replay warnings tree A did not have (subjects):"
    );
    for (k, c) in &b_warnings {
        let _ = writeln!(summary, "  {k}: {c}");
    }
    let _ = writeln!(
        summary,
        "failures (no draw, render or composite error on either tree, tree B inputs not built): {failures:#?}"
    );
    print!("{summary}");
    t.push_str(&summary);
    let out = std::env::temp_dir().join("tree-b-vs-a.txt");
    std::fs::write(&out, &t).unwrap();
    println!("table written to {}", out.display());

    // Pre-flight ruling: every subject renders and draws on both trees; the differences are rows.
    assert!(run > 0, "no golden subjects found under SDC_GOLDEN");
    assert!(
        failures.is_empty(),
        "subjects that failed to render or draw: {failures:#?}"
    );
}

fn var(name: &str, offset: u32, size: u32) -> CbVar {
    CbVar {
        name: name.into(),
        offset,
        size,
        kind: VarKind::Float,
    }
}

fn layout(size: u32, vars: Vec<CbVar>) -> CbLayout {
    CbLayout {
        name: "FeatureData".into(),
        slot: 6,
        size,
        vars,
    }
}

#[test]
fn tested_reads_ifdef_ifndef_and_defined() {
    let text = "#ifdef A\n#\tif defined(B) && !defined C\n  #  ifndef D\nfloat defined(E);\n#if undefined(F)\n";
    let want: BTreeSet<String> = ["A", "B", "C", "D"].map(String::from).into();
    assert_eq!(tested(text), want);
}

#[test]
fn remap_copies_leaves_by_name() {
    // A: x (float4), then struct s { a; b (float3) }. B: s { a; new; b (float2) }, then x.
    let a = layout(
        32,
        vec![
            var("x", 0, 16),
            var("s", 16, 16),
            var("s.a", 16, 4),
            var("s.b", 20, 12),
        ],
    );
    let b = layout(
        32,
        vec![
            var("s", 0, 16),
            var("s.a", 0, 4),
            var("s.new", 4, 4),
            var("s.b", 8, 8),
            var("x", 16, 16),
        ],
    );
    let raw: Vec<u8> = (1..=32).collect();
    let (out, missing) = remap(&raw, &a, &b);
    let mut want = vec![0u8; 32];
    want[0..4].copy_from_slice(&raw[16..20]); // s.a
    want[8..16].copy_from_slice(&raw[20..28]); // s.b, cut to B's size
    want[16..32].copy_from_slice(&raw[0..16]); // x
    assert_eq!(out, want);
    assert_eq!(missing, vec!["s.new".to_string()]);
    // A short capture reads as zero-padded, as `cb_bytes` uploads it.
    let (out, _) = remap(&raw[..8], &a, &b);
    assert_eq!(&out[16..24], &raw[..8]);
    assert!(out[..16].iter().chain(&out[24..]).all(|x| *x == 0));
}
