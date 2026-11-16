//! v2 Phase D, D6c: sdc's own NIF material against the capture (extraction_matches_capture asserts that every CS 2.0
//! group is swapped in and that none of its MRTs is over the limit; the v1 and vanilla rows are report-only). A hybrid
//! replay: geometry, scene cbuffers, engine textures, samplers and depth from the capture; the material (PS b1 used
//! variables, the PS b2 material fields, VS b1 TexcoordOffset, material textures) from sdc's own `Renderer::load`.
//! Differences isolate material mapping (04 Q4-Q8). Needs SDC_GOLDEN and SDC_ACHERUS (the CS PBR2Test1 root, which
//! the bundled CS 2.0 shaders replay, Note IE5). The CS 2.0 subjects are their own v1 meshes pointed at a
//! `.csm` built here (Note IE7).
mod common;

#[path = "common/golden_nif.rs"]
mod golden_nif;

use golden_nif::{env, json, key, load_sdc, renderer, subject_of, Env, SdcShape, M, PBR2};
use render3d::compile::{compile, used_vars, CbVar, SrvKind, VarKind};
use render3d::golden::{self, Noise};
use render3d::permutation::Defines;
use render3d::replay::{cb_bytes, vs_defines};
use render3d::source::SourceSet;
use render3d::{load_fixture, DebugOutput, ParsedNif, Renderer, Replay, ShaderSource};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;

/// The subjects (the CS PBR2Test1 root, Note IE5): the CS 1.9 root's v1 subjects in the A1, A2 and A2S
/// captures (no ShaderSpheres02 there), the type-21 subjects of T1 and T2, and T2L's CookingStone01 (PBRParams2
/// differs between its T2 and T2L captures).
const EXTRACTION: &[&str] = &[
    "Skyrim_AE_1.6.1170_20261114_201422_frame67390/ev26892_Coffin",
    "Skyrim_AE_1.6.1170_20261114_201422_frame67390/ev28399_Object174",
    "Skyrim_AE_1.6.1170_20261114_201422_frame67390/ev28469_Object174",
    "Skyrim_AE_1.6.1170_20261114_201422_frame67390/ev28570_dibella_L.004_3",
    "Skyrim_AE_1.6.1170_20261114_201422_frame78910/ev34101_BlackreachCrystalL01_10",
    "Skyrim_AE_1.6.1170_20261114_201422_frame78910/ev32136_NorRubblePiece01_0",
    "Skyrim_AE_1.6.1170_20261114_201422_frame83415/ev31771_SOVStairsPiece01_6",
    "Skyrim_AE_1.6.1170_20261114_201422_frame212763/ev24951_HelgenStoneBlock01_12",
    "Skyrim_AE_1.6.1170_20261114_201422_frame212763/ev25009_Corpse04_0",
    "Skyrim_AE_1.6.1170_20261114_201422_frame212763/ev24916_AzuraSmallCrystal05_2",
    "Skyrim_AE_1.6.1170_20261114_201422_frame212763/ev24973_Rug01Rolled_0",
    "Skyrim_AE_1.6.1170_20261114_201422_frame223565/ev28774_LoadScreenMRaltar01_1",
    "Skyrim_AE_1.6.1170_20261114_201422_frame223565/ev28753_LoadScreenMRaltar01_2",
    "Skyrim_AE_1.6.1170_20261114_201422_frame223565/ev26916_CookingStone01_1",
    "Skyrim_AE_1.6.1170_20261114_201422_frame230321/ev26900_CookingStone01_1",
    "Skyrim_AE_1.6.1170_20261114_201422_frame223565/ev26998_HandCartCanopy_0",
    "Skyrim_AE_1.6.1170_20261114_201422_frame223565/ev26977_HandCartCanopy_1",
    "Skyrim_AE_1.6.1170_20261114_201422_frame223565/ev26941_CaveWormtubeLarge_0",
];

/// The PS b2 (PerGeometry) fields that come from the material (Lighting.hlsl:564-582).
const B2_MATERIAL: &[&str] = &["MaterialData", "AlphaTestRef", "EmitColor"];

/// Engine textures: bound by the game whatever the material (noise, shadow mask, and the scene depth that CS 2.0's
/// Terrain Blending reads, `SharedData::DepthTexture`, Note ID53), and CS's glint noise
/// map at t20, which CS generates once for every GLINT draw (sdc's own copy is `render3d::glint_noise`; Note IE8).
// shortcut: t20 keeps the game's glint noise, so the glint subject's pixels never test sdc's `glint_noise` port against
// CS's map (`tests/glint_noise.rs` checks it against an independent computation only). Swap in sdc's map once the
// replay can bind an RGBA32F texture from the renderer.
const ENGINE_REGS: &[u32] = &[11, 14, 17, 20];

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

/// One compared cbuffer field.
struct Field {
    cb: &'static str,
    var: String,
    offset: u32,
    capture: Vec<f32>,
    /// What sdc should hold: the capture, with a `.csm` colour's x, y and z through `#rrggbb` (`csm::hex`, then
    /// `csm::rgb`), as a `.csm` stores a colour (Note IE7).
    expected: Vec<f32>,
    /// A `.csm` colour field ([`SdcShape::colours`]).
    colour: bool,
    sdc: Option<Vec<f32>>,
    ok: bool,
}

/// `c` through `#rrggbb`: what a `.csm` holds for a colour of the float `c`.
fn quantised(c: &[f32]) -> [f32; 3] {
    nif::csm::rgb(nif::csm::hex([c[0], c[1], c[2]]))
}

fn floats(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect()
}

/// uint/int/bool exact; floats within 1e-5·max(1, |capture|).
fn same(kind: VarKind, a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| match kind {
            VarKind::Float => (x - y).abs() <= 1e-5 * y.abs().max(1.0),
            _ => x.to_bits() == y.to_bits(),
        })
}

/// The capture's used material fields (PS b1 PerMaterial, the PS b2 material fields, VS b1 TexcoordOffset), each
/// with its reflected layout var and cbuffer size, keyed by fixture cbuffer stem.
fn material_fields(env: &Env, r: &Replay) -> Result<Vec<(&'static str, CbVar, u32)>, String> {
    let src = SourceSet::new(ShaderSource::Bundled).get(Path::new("Lighting.hlsl"))?;
    let ps_list = &r.defines[0];
    let ps_d: Vec<String> = std::iter::once("PSHADER".to_string())
        .chain(ps_list.iter().cloned())
        .collect();
    let mut out = Vec::new();
    for (stage, target, defs) in [
        ("ps", "ps_5_0", ps_d),
        ("vs", "vs_5_0", vs_defines(ps_list, &r.mesh)),
    ] {
        let c = compile(&src, &to_defines(&defs), target, env.cache.path())?;
        let used: BTreeSet<(String, String)> = used_vars(&c.bytecode)?.into_iter().collect();
        for l in &c.reflection.cbuffers {
            let stem: &'static str = match (stage, l.slot) {
                ("ps", 1) => "ps_b1",
                ("ps", 2) => "ps_b2",
                ("vs", 1) => "vs_b1",
                _ => continue,
            };
            for v in &l.vars {
                let wanted = match stem {
                    "ps_b1" => true,
                    "ps_b2" => B2_MATERIAL.contains(&v.name.as_str()),
                    _ => v.name == "TexcoordOffset",
                };
                if wanted && used.contains(&(l.name.clone(), v.name.clone())) {
                    out.push((stem, v.clone(), l.size));
                }
            }
        }
    }
    out.sort_by_key(|(s, v, _)| (*s, v.offset));
    Ok(out)
}

fn compare_fields(
    ren: &Renderer,
    sdc_shape: &SdcShape,
    r: &Replay,
    fields: &[(&'static str, CbVar, u32)],
) -> Vec<Field> {
    let id = sdc_shape.id;
    fields
        .iter()
        .map(|(stem, v, size)| {
            let raw = cb_bytes(&r.cbuffers[*stem], *size);
            let capture = floats(&raw[v.offset as usize..(v.offset + v.size) as usize]);
            let sdc = if *stem == "vs_b1" {
                ren.draw_vs_cb_floats(id, &v.name)
            } else {
                ren.draw_cb_floats(id, &v.name)
            };
            let colour = *stem == "ps_b1"
                && capture.len() >= 3
                && sdc_shape.colours.contains(&v.name.as_str());
            let mut expected = capture.clone();
            if colour {
                expected[..3].copy_from_slice(&quantised(&capture));
            }
            let ok = sdc.as_ref().is_some_and(|s| same(v.kind, s, &expected));
            Field {
                cb: stem,
                var: v.name.clone(),
                offset: v.offset,
                capture,
                expected,
                colour,
                sdc,
                ok,
            }
        })
        .collect()
}

/// 04 Q4-Q8 group of a material field.
fn group(var: &str) -> &'static str {
    match var {
        "SpecularColor" => "Q5 specular",
        "EnvmapData" | "MaterialData" => "Q6 envmap/LOD",
        "EmitColor" => "Q7 emissive",
        "PBRFlags" | "PBRParams1" | "PBRParams2" | "MultiLayerParallaxData" => "Q8 PBR",
        _ => "other",
    }
}

fn dxgi_name(f: i32) -> String {
    match f {
        2 => "R32G32B32A32_FLOAT",
        10 => "R16G16B16A16_FLOAT",
        28 => "R8G8B8A8_UNORM",
        29 => "R8G8B8A8_UNORM_SRGB",
        71 => "BC1_UNORM",
        72 => "BC1_SRGB",
        74 => "BC2_UNORM",
        75 => "BC2_SRGB",
        77 => "BC3_UNORM",
        78 => "BC3_SRGB",
        80 => "BC4_UNORM",
        83 => "BC5_UNORM",
        87 => "B8G8R8A8_UNORM",
        91 => "B8G8R8A8_UNORM_SRGB",
        95 => "BC6H_UF16",
        98 => "BC7_UNORM",
        99 => "BC7_SRGB",
        _ => return format!("DXGI {f}"),
    }
    .to_string()
}

/// The texel data of a DDS file: past the 128-byte header, and the DX10 extension when present.
fn dds_data(b: &[u8]) -> Option<&[u8]> {
    let skip = if b.get(84..88)? == b"DX10" { 148 } else { 128 };
    b.get(skip..)
}

fn dds_wh(b: &[u8]) -> Option<(u32, u32)> {
    let u = |o: usize| Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?));
    Some((u(16)?, u(12)?))
}

/// Writes sdc's material into the capture's replay: the compared fields sdc has, and sdc's slot textures at its
/// registers (capture-only material registers are dropped: unbound, reads 0). A `.csm` colour component whose captured
/// value is outside 0-1 keeps the capture's: `#rrggbb` cannot hold it (Note IE7), and the field check already holds sdc
/// to the clamped value. Returns notes.
fn swap_material(
    ren: &Renderer,
    r: &mut Replay,
    fields: &[Field],
    sizes: &HashMap<&str, u32>,
    sdc: &SdcShape,
) -> Vec<String> {
    let mut notes = Vec::new();
    for f in fields {
        let Some(v) = &f.sdc else {
            notes.push(format!("{}.{}: sdc has none, capture kept", f.cb, f.var));
            continue;
        };
        let b = r.cbuffers.get_mut(f.cb).unwrap();
        *b = cb_bytes(b, sizes[f.cb]);
        for (i, x) in v.iter().enumerate() {
            let x = match f.capture.get(i) {
                Some(c) if f.colour && i < 3 && !(0.0..=1.0).contains(c) => {
                    notes.push(format!(
                        "{}.{}[{i}]: capture {c} kept (sdc {x})",
                        f.cb, f.var
                    ));
                    c
                }
                _ => x,
            };
            let o = f.offset as usize + 4 * i;
            b[o..o + 4].copy_from_slice(&x.to_le_bytes());
        }
    }
    let sdc_regs: BTreeSet<u32> = sdc.slots.iter().map(|s| s.0).collect();
    // A material register the NIF leaves empty gets sdc's 1x1 default, as sdc's own draw binds it.
    let others: Vec<u32> = r
        .textures
        .keys()
        .copied()
        .filter(|reg| !ENGINE_REGS.contains(reg) && !sdc_regs.contains(reg))
        .collect();
    for reg in others {
        match ren.draw_default_at(sdc.id, reg) {
            Some((SrvKind::Cube, c)) => {
                r.textures.insert(reg, common::dds_cube_rgba8(c));
            }
            Some((SrvKind::Tex2D, c)) => {
                r.textures.insert(reg, common::dds_rgba8(1, 1, &[c]));
            }
            d => {
                r.textures.remove(&reg);
                notes.push(format!("t{reg}: sdc default {d:?}, left unbound"));
            }
        }
    }
    for (reg, path, bytes) in &sdc.slots {
        match bytes {
            Some(b) => {
                r.textures.insert(*reg, b.clone());
            }
            None => {
                r.textures.remove(reg);
                notes.push(format!("t{reg} {path}: not found, unbound"));
            }
        }
    }
    notes
}

fn fmt_v(v: &[f32]) -> String {
    format!(
        "[{}]",
        v.iter()
            // A tiny non-zero value (an integer's bits read as a float) is shown, not rounded to 0.
            .map(|x| if *x != 0.0 && x.abs() < 1e-4 {
                format!("{x:e}")
            } else {
                format!("{x:.6}")
            })
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[test]
#[ignore]
fn extraction_matches_capture() {
    let Some(env) = env() else { return };
    let noise =
        Noise::load(&golden::noise_path(&env.root).unwrap_or_else(|e| panic!("{e}"))).unwrap();
    let mut ren = renderer(&env);
    let mut t = String::new();
    let (mut reached, mut nonfinite, mut define_mismatch) = (0usize, Vec::new(), 0usize);
    let mut firsts: Vec<String> = Vec::new();
    // The counted CS 2.0 subjects (glint included, Note IE8): the events that reached the pixel swap, and each MRT over
    // the noise limit there.
    let (mut swapped, mut over) = (BTreeSet::new(), Vec::new());
    // Counted subjects whose shape did not draw from a resolved `.csm`: (name, PBR values taken, csm_error).
    let mut no_csm: Vec<String> = Vec::new();
    // `.csm` colours: counted subjects whose colour field is not the capture's through `#rrggbb`, and the largest
    // difference that leaves, for a captured colour inside 0-1 (rounding) and outside it (clamp).
    let mut colour_misses = Vec::new();
    let (mut rounding, mut clamp) = ((0.0f32, String::new()), (0.0f32, String::new()));
    for s in EXTRACTION {
        let dir = env.root.join(s);
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let counted = subject_of(&name);
        let _ = writeln!(t, "\n== {s}");
        let mut r = load_fixture(&dir).unwrap();
        // (1)-(2) sdc's load and the shape.
        let sdc = match load_sdc(&mut ren, &env, &dir, &r) {
            Ok(x) => x,
            Err(e) => {
                let _ = writeln!(t, "shape: ERROR {e}");
                continue;
            }
        };
        let _ = writeln!(
            t,
            "nif: {} | shape: {} | load errors: {:?}",
            sdc.nif_origin, sdc.rule, sdc.errors
        );
        // (3) Defines.
        let cap: BTreeSet<&str> = r.defines[0]
            .iter()
            .map(|d| key(d))
            .filter(|k| M.contains(k))
            .collect();
        let Some((sdc_defs, _, sdc_srvs)) = ren.draw_diag(sdc.id) else {
            let _ = writeln!(t, "defines: ERROR no draw_diag");
            continue;
        };
        let mine: BTreeSet<&str> = sdc_defs
            .iter()
            .map(|d| d.0.as_str())
            .filter(|k| M.contains(k))
            .collect();
        let defines_equal = cap == mine;
        if !defines_equal {
            define_mismatch += 1;
        }
        let _ = writeln!(
            t,
            "defines: {} capture-only {:?} sdc-only {:?} (common {:?})",
            if defines_equal { "EQUAL" } else { "DIFFER" },
            cap.difference(&mine).collect::<Vec<_>>(),
            mine.difference(&cap).collect::<Vec<_>>(),
            cap.intersection(&mine).collect::<Vec<_>>()
        );
        // (4) Cbuffer fields.
        let layout = match material_fields(&env, &r) {
            Ok(l) => l,
            Err(e) => {
                let _ = writeln!(t, "cbuffers: ERROR {e}");
                continue;
            }
        };
        reached += 1;
        let fields = compare_fields(&ren, &sdc, &r, &layout);
        for f in fields.iter().filter(|f| f.colour) {
            if counted.is_some() && !f.ok {
                colour_misses.push(format!("{name} {}", f.var));
            }
            for (c, e) in f.capture.iter().zip(&f.expected).take(3) {
                let d = (c - e).abs();
                let (max, what) = if (0.0..=1.0).contains(c) {
                    (&mut rounding, "rounding")
                } else {
                    (&mut clamp, "clamp")
                };
                if d > max.0 {
                    *max = (d, format!("{name} {} {what} {c} -> {e}", f.var));
                }
            }
        }
        let first = fields.iter().find(|f| !f.ok);
        match first {
            Some(f) => {
                let line = format!(
                    "{name}: {}.{} [{}] capture {} sdc {}",
                    f.cb,
                    f.var,
                    group(&f.var),
                    fmt_v(&f.capture),
                    f.sdc
                        .as_deref()
                        .map_or("not reflected by sdc".into(), fmt_v)
                );
                let _ = writeln!(t, "first differing field: {line}");
                firsts.push(line);
            }
            None => {
                let _ = writeln!(t, "first differing field: none");
            }
        }
        let _ = writeln!(t, "cb | var | offset | group | capture | sdc | match");
        for f in &fields {
            let note = if f.var == "MaterialData" && !f.ok {
                " (runtime LOD fade)"
            } else {
                ""
            };
            let cap = if !f.colour {
                fmt_v(&f.capture)
            } else {
                format!("{} (.csm {})", fmt_v(&f.capture), fmt_v(&f.expected))
            };
            let _ = writeln!(
                t,
                "{} | {} | {} | {} | {cap} | {} | {}{note}",
                f.cb,
                f.var,
                f.offset,
                group(&f.var),
                f.sdc
                    .as_deref()
                    .map_or("not reflected by sdc".into(), fmt_v),
                if f.ok { "ok" } else { "DIFF" }
            );
        }
        // (5) Texture registers.
        let tex = json(&dir.join("tex.json"));
        let cap_tex: HashMap<u32, &Value> = tex
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["isTexture"] == true && e["saved"] == true)
            .map(|e| (e["bind"].as_u64().unwrap() as u32, e))
            .collect();
        let cap_regs: BTreeSet<u32> = cap_tex
            .keys()
            .copied()
            .filter(|r| !ENGINE_REGS.contains(r))
            .collect();
        let sdc_regs: BTreeSet<u32> = sdc.slots.iter().map(|s| s.0).collect();
        let _ = writeln!(
            t,
            "textures: capture-only {:?} sdc-only {:?} (engine t11/t14/t17 not compared)",
            cap_regs.difference(&sdc_regs).collect::<Vec<_>>(),
            sdc_regs.difference(&cap_regs).collect::<Vec<_>>()
        );
        for (reg, path, bytes) in &sdc.slots {
            let Some(c) = cap_tex.get(reg) else { continue };
            let sdc_fmt = sdc_srvs
                .iter()
                .find(|s| s.0 == *reg)
                .map_or("unbound".to_string(), |s| dxgi_name(s.1));
            let wh = bytes.as_deref().and_then(dds_wh);
            let cap_wh = (
                c["w"].as_u64().unwrap_or(0) as u32,
                c["h"].as_u64().unwrap_or(0) as u32,
            );
            let cap_fmt = c["viewFormat"].as_str().unwrap_or("?");
            let ok = wh == Some(cap_wh) && sdc_fmt == cap_fmt;
            // Same pixels: equal bytes past each file's own header (the extractor may write DX10 where the file is legacy).
            let cap_bytes = std::fs::read(dir.join(format!("tex/t{reg}.dds"))).unwrap_or_default();
            let content = match bytes {
                Some(b) if dds_data(b).is_some() && dds_data(b) == dds_data(&cap_bytes) => {
                    "same data"
                }
                Some(_) => "OTHER DATA",
                None => "not found",
            };
            let _ = writeln!(
                t,
                "  t{reg} {path}: sdc {wh:?} {sdc_fmt} | capture {cap_wh:?} {cap_fmt} | {} | {content}",
                if ok { "ok" } else { "DIFF" }
            );
        }
        // (6) Pixels, only with equal material defines.
        if !defines_equal {
            let _ = writeln!(t, "pixels: skipped (material defines differ)");
            continue;
        }
        let sizes: HashMap<&str, u32> = layout.iter().map(|(s, _, z)| (*s, *z)).collect();
        let notes = swap_material(&ren, &mut r, &fields, &sizes, &sdc);
        let out = match ren.render_replay(&r, true) {
            Ok(o) => o,
            Err(e) => {
                let _ = writeln!(t, "pixels: replay ERROR {e}");
                continue;
            }
        };
        let _ = writeln!(
            t,
            "pixels: swap notes {notes:?}; replay warnings {:?}",
            out.warnings
        );
        let n = (r.crop[2] * r.crop[3]) as usize;
        let mask = golden::read_mask(&dir.join("mask.bin"), n).unwrap();
        let mut row = String::from("pixels RT0..RT7 over%/max:");
        for (i, rt) in out.mrt.iter().enumerate() {
            if rt.iter().any(|v| !v.is_finite()) {
                nonfinite.push(format!("{name} RT{i}"));
            }
            let file = format!("mrt{i}.f32");
            let after = golden::read_f32(&dir.join(&file), n * 4).unwrap();
            let (_, max) = noise.limit(&name, &file).unwrap();
            let d = golden::diff_tol(
                rt,
                &after,
                &mask,
                golden::format_tol(&r.formats[i]).unwrap(),
                max,
            );
            let _ = write!(row, " {:.2}%/{:.3e}", 100.0 * d.over, d.max_abs);
            if counted.is_some() && d.over > 0.0 {
                over.push(format!("{name} RT{i} {:.2}%", 100.0 * d.over));
            }
        }
        if counted.is_some() {
            // Counted only when the shape drew from its resolved `.csm`.
            match &sdc.csm {
                (Some(_), true, None) => {
                    swapped.insert(name.clone());
                }
                other => no_csm.push(format!("{name} {other:?}")),
            }
        }
        if let (Some(main), Some(rc)) = (&out.main, &r.composite) {
            if main.iter().any(|v| !v.is_finite()) {
                nonfinite.push(format!("{name} main"));
            }
            let after = golden::read_f32(&dir.join("mainrw_after.f32"), n * 4).unwrap();
            let (_, max) = noise.limit(&name, "mainrw_after.f32").unwrap();
            let mrt0 = golden::read_f32(&dir.join("mrt0.f32"), n * 4).unwrap();
            let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
            let clean: Vec<bool> = (0..n)
                .map(|p| {
                    mask[p] && bits(&mrt0[4 * p..4 * p + 4]) == bits(&rc.before[4 * p..4 * p + 4])
                })
                .collect();
            for (label, m) in [("mask", &mask), ("clean", &clean)] {
                let d = golden::diff_tol(
                    main,
                    &after,
                    m,
                    golden::format_tol("R16G16B16A16_FLOAT").unwrap(),
                    max,
                );
                let _ = write!(
                    row,
                    " | main [{label}] {:.2}%/{:.3e} ({}px)",
                    100.0 * d.over,
                    d.max_abs,
                    d.pixels
                );
            }
        }
        let _ = writeln!(t, "{row}");
    }
    let _ = writeln!(
        t,
        "\nsubjects {}: reached cbuffers {reached}; material define mismatches {define_mismatch}",
        EXTRACTION.len()
    );
    let _ = writeln!(t, "first differing fields:");
    for f in &firsts {
        let _ = writeln!(t, "  {f}");
    }
    let _ = writeln!(t, "non-finite: {nonfinite:?}");
    let groups: BTreeSet<&str> = swapped
        .iter()
        .filter_map(|n: &String| subject_of(n).map(|p| p.label))
        .collect();
    let unswapped: Vec<&str> = EXTRACTION
        .iter()
        .map(|s| s.rsplit('/').next().unwrap())
        .filter(|n| subject_of(n).is_some() && !swapped.contains(*n))
        .collect();
    let _ = writeln!(
        t,
        "CS 2.0 subjects swapped: {} groups {groups:?}; not swapped {unswapped:?}; without a \
         resolved .csm {no_csm:?}; MRTs over the limit {over:?}",
        groups.len()
    );
    let _ = writeln!(
        t,
        ".csm colours: fields off the capture's #rrggbb {colour_misses:?}; largest rounding {:.6} ({}); largest \
         clamp {:.6} ({})",
        rounding.0, rounding.1, clamp.0, clamp.1
    );
    print!("{t}");
    let out = std::env::temp_dir().join("golden-extraction.txt");
    std::fs::write(&out, &t).unwrap();
    println!("table written to {}", out.display());
    assert!(
        reached >= 5,
        "only {reached} subjects reached the cbuffer step"
    );
    assert!(nonfinite.is_empty(), "non-finite output: {nonfinite:?}");
    assert!(
        no_csm.is_empty(),
        "counted subjects without a resolved .csm: {no_csm:?}"
    );
    assert_eq!(
        groups.len(),
        PBR2.len(),
        "CS 2.0 subjects swapped: {groups:?}; not swapped {unswapped:?}"
    );
    assert!(unswapped.is_empty(), "not swapped: {unswapped:?}");
    assert!(
        colour_misses.is_empty(),
        "colour fields off the capture's #rrggbb: {colour_misses:?}"
    );
    assert!(
        over.is_empty(),
        "CS 2.0 subject MRTs over the limit: {over:?}"
    );
}

/// Part 3 mode B: the DBG subject with sdc's material swapped in (as in extraction_matches_capture); the NIF is
/// unloaded afterwards so debug-view changes recompile nothing but the replay.
fn hybrid(ren: &mut Renderer, env: &Env, dir: &Path) -> Result<(Replay, Vec<String>), String> {
    let mut r = load_fixture(dir)?;
    let sdc = load_sdc(ren, env, dir, &r)?;
    let layout = material_fields(env, &r)?;
    let fields = compare_fields(ren, &sdc, &r, &layout);
    let sizes: HashMap<&str, u32> = layout.iter().map(|(s, _, z)| (*s, *z)).collect();
    let mut notes = swap_material(ren, &mut r, &fields, &sizes, &sdc);
    notes.extend(
        fields
            .iter()
            .filter(|f| !f.ok)
            .map(|f| format!("{}.{} differs", f.cb, f.var)),
    );
    ren.load_parsed_sync(&ParsedNif::default(), &|_| None);
    Ok((r, notes))
}

const UNPARSED: &str = "unparsed var";

/// The value a debug view writes at the traced pixel, from the trace's final values (None = a null component).
/// Err(UNPARSED) when a traced var is not an array of numbers and nulls.
fn traced(view: DebugOutput, vars: &Value, pbr: bool) -> Result<[Option<f32>; 3], &'static str> {
    let comp = |k: &str, i: usize| -> Result<Option<f32>, &'static str> {
        match vars[k].as_array().and_then(|a| a.get(i)) {
            Some(Value::Null) => Ok(None),
            Some(v) => v.as_f64().map(|x| Some(x as f32)).ok_or(UNPARSED),
            None => Err(UNPARSED),
        }
    };
    let vec3 = |k: &str, f: fn(f32) -> f32| -> Result<[Option<f32>; 3], &'static str> {
        Ok([comp(k, 0)?.map(f), comp(k, 1)?.map(f), comp(k, 2)?.map(f)])
    };
    let scalar = |k: &str, f: fn(f32) -> f32| {
        if vars.get(k).is_none() {
            return Err("not traced");
        }
        let v = comp(k, 0)?.map(f);
        Ok([v, v, v])
    };
    match view {
        DebugOutput::NormalWorld if vars.get("worldNormal").is_some() => {
            vec3("worldNormal", |n| n * 0.5 + 0.5)
        }
        DebugOutput::Roughness if pbr => scalar("material.Roughness", |x| x),
        DebugOutput::Roughness => scalar("glossiness", |g| 1.0 - g),
        DebugOutput::Metallic if pbr => scalar("material.Metallic", |x| x),
        DebugOutput::Metallic => Err("non-PBR: no metallic term"),
        DebugOutput::Albedo if vars.get("material.BaseColor").is_some() => {
            vec3("material.BaseColor", |x| x)
        }
        _ => Err("not traced"),
    }
}

const VIEWS: [DebugOutput; 4] = [
    DebugOutput::NormalWorld,
    DebugOutput::Roughness,
    DebugOutput::Metallic,
    DebugOutput::Albedo,
];

/// Mode B subjects: (capture, subject folder name without the event).
const MODE_B: &[(&str, &str)] = &[
    ("Skyrim_AE_1.6.1170_frame26196", "Coffin"),
    ("Skyrim_AE_1.6.1170_frame26196", "Object174"),
    ("Skyrim_AE_1.6.1170_frame26196", "dibella_L.004_3"),
    ("Skyrim_AE_1.6.1170_frame61618", "BlackreachCrystalL01_10"),
    ("Skyrim_AE_1.6.1170_frame61618", "NorRubblePiece01_0"),
];

const WANTED: [&str; 5] = [
    "worldNormal",
    "material.Roughness",
    "material.Metallic",
    "material.BaseColor",
    "glossiness",
];

#[test]
#[ignore]
fn trace_values_match_unit_functions() {
    let Some(env) = env() else { return };
    let mut ren = renderer(&env);
    assert!(
        ren.debug_supported(),
        "the bundled Lighting.hlsl has no debug anchor"
    );
    // D-P24: a trace counts only when its fragment is the one the replay keeps (depth within 2 D24 ulps).
    ren.set_replay_depth_readback(true);
    let mut t = String::from(
        "mode | subject | trace | view | traced | replay RT0 | over | traced z / replay z\n",
    );
    let mut skipped: std::collections::BTreeMap<&str, Vec<String>> = Default::default();
    let (mut checked, mut failures, mut b_rows) = (0usize, Vec::new(), 0usize);
    // Compared Roughness + Metallic values; skipped occluded values whose colour was also over the limit.
    let (mut rough_metal, mut occluded_over) = (0usize, 0usize);
    let (mut occluded_dz, mut missing_z, mut unparsed) =
        (Vec::<f32>::new(), Vec::new(), Vec::new());
    for s in golden::subjects(&env.root, &["DBG"]).unwrap() {
        let name = s.dir.file_name().unwrap().to_string_lossy().into_owned();
        let who = format!("{}/{name}", s.capture);
        let mrt = json(&s.dir.join("mrt.json"));
        if mrt["maskSource"] == "overlay" {
            skipped.entry("overlay mask").or_default().push(who);
            continue;
        }
        let r = match load_fixture(&s.dir) {
            Ok(r) => r,
            Err(e) if e.contains("defines null") => {
                skipped.entry("defines null").or_default().push(who);
                continue;
            }
            Err(e) => {
                skipped
                    .entry("load error")
                    .or_default()
                    .push(format!("{who}: {e}"));
                continue;
            }
        };
        if r.blended {
            skipped.entry("blended").or_default().push(who);
            continue;
        }
        if r.two_sided_alpha_tested() && r.depth.is_none() {
            skipped
                .entry("two-sided without depth")
                .or_default()
                .push(who);
            continue;
        }
        let mut traces = Vec::new();
        for pick in ["lit", "mid", "dark"] {
            let p = s.dir.join(format!("trace_{pick}.json"));
            let Ok(text) = std::fs::read_to_string(&p) else {
                skipped
                    .entry("no trace file")
                    .or_default()
                    .push(format!("{who} {pick}"));
                continue;
            };
            let tr: Value =
                serde_json::from_str(&render3d::replay::nonfinite_to_null(&text)).unwrap();
            let why = if tr.get("error").is_some() {
                Some("error trace")
            } else if tr["debugInfo"] != true {
                Some("debugInfo not true")
            } else if !WANTED.iter().any(|k| tr["vars"].get(*k).is_some()) {
                Some("no wanted name")
            } else {
                None
            };
            match why {
                Some(w) => skipped.entry(w).or_default().push(format!("{who} {pick}")),
                None => traces.push((pick, tr)),
            }
        }
        if traces.is_empty() {
            continue;
        }
        let pbr = r.defines[0].iter().any(|d| d == "TRUE_PBR");
        let mode_b = MODE_B.iter().any(|(c, tag)| {
            *c == s.capture && name.split_once('_').is_some_and(|(_, rest)| rest == *tag)
        });
        let mut runs: Vec<(&str, Replay)> = vec![("A", r)];
        if mode_b {
            match hybrid(&mut ren, &env, &s.dir) {
                Ok((h, notes)) => {
                    let _ = writeln!(t, "B | {who} | notes {notes:?}");
                    runs.push(("B", h));
                }
                Err(e) => {
                    let _ = writeln!(t, "B | {who} | hybrid ERROR {e}");
                }
            }
        }
        for (mode, r) in &runs {
            let [cx, cy, cw, ch] = r.crop;
            for view in VIEWS {
                ren.set_debug_output(view).unwrap();
                let out = match ren.render_replay(r, false) {
                    Ok(o) => o,
                    Err(e) => {
                        skipped
                            .entry("replay error")
                            .or_default()
                            .push(format!("{mode} {who} {view:?}: {e}"));
                        continue;
                    }
                };
                for (pick, tr) in &traces {
                    let x = tr["x"].as_u64().unwrap() as u32;
                    let y = tr["y"].as_u64().unwrap() as u32;
                    if x < cx || y < cy || x >= cx + cw || y >= cy + ch {
                        skipped
                            .entry("traced pixel outside the crop")
                            .or_default()
                            .push(format!("{who} {pick}"));
                        continue;
                    }
                    let asserted = *mode == "A" && view != DebugOutput::Albedo;
                    let want = match traced(view, &tr["vars"], pbr) {
                        Ok(w) if w.iter().any(Option::is_some) => w,
                        Err(e) if e == UNPARSED => {
                            let line = format!("{mode} {who} {pick} {view:?}");
                            if asserted {
                                unparsed.push(line.clone());
                            }
                            skipped.entry(UNPARSED).or_default().push(line);
                            continue;
                        }
                        _ => continue,
                    };
                    let p = ((y - cy) * cw + (x - cx)) as usize;
                    let got: [f32; 4] = out.mrt[0][4 * p..4 * p + 4].try_into().unwrap();
                    // Untraced components and alpha compare with themselves.
                    let b: Vec<f32> = (0..4)
                        .map(|i| want.get(i).copied().flatten().unwrap_or(got[i]))
                        .collect();
                    let d = golden::diff_tol(
                        &got,
                        &b,
                        &[true],
                        golden::format_tol("R16G16B16A16_FLOAT").unwrap(),
                        0.0,
                    );
                    let zt = tr["vars"]["SV_POSITION"][2].as_f64().map(|z| z as f32);
                    let zr = out.depth.as_ref().unwrap()[p];
                    let visible = zt.is_some_and(|z| (z - zr).abs() <= 2.0 / 16_777_215.0);
                    let _ = writeln!(
                        t,
                        "{mode}{} | {who} | {pick} | {view:?} | {} | {} | {} | {zt:?} / {zr}{}",
                        if asserted { "" } else { " (report)" },
                        fmt_v(&b[..3]),
                        fmt_v(&got[..3]),
                        if d.over > 0.0 {
                            format!("OVER max {:.3e}", d.max_abs)
                        } else {
                            "0".into()
                        },
                        if visible { "" } else { " occluded" }
                    );
                    // A trace without a depth cannot be placed: a failure with its own reason, never an occlusion.
                    if asserted && zt.is_none() {
                        missing_z.push(format!("{who} {pick} {view:?}"));
                        continue;
                    }
                    if asserted && !visible {
                        occluded_dz.push((zt.unwrap() - zr).abs());
                        occluded_over += (d.over > 0.0) as usize;
                        skipped
                            .entry("occluded fragment")
                            .or_default()
                            .push(format!(
                                "{who} {pick} {view:?}: traced z {zt:?}, replay z {zr}{}",
                                if d.over > 0.0 { " (colour over)" } else { "" }
                            ));
                        continue;
                    }
                    if asserted {
                        checked += 1;
                        rough_metal +=
                            matches!(view, DebugOutput::Roughness | DebugOutput::Metallic) as usize;
                        if d.over > 0.0 {
                            failures.push(format!(
                                "{who} {pick} {view:?}: traced {} replay {}",
                                fmt_v(&b[..3]),
                                fmt_v(&got[..3])
                            ));
                        }
                    } else if *mode == "B" {
                        b_rows += 1;
                    }
                }
            }
        }
    }
    ren.set_debug_output(DebugOutput::Lit).unwrap();
    let _ = writeln!(
        t,
        "\nmode A compared values: {checked} (Roughness + Metallic {rough_metal}); failures: {}; occluded skips with colour over: {occluded_over}",
        failures.len()
    );
    for f in &failures {
        let _ = writeln!(t, "  {f}");
    }
    let _ = writeln!(
        t,
        "mode A traces: {} (compared {checked}, occluded {}); FAIL no SV_POSITION in trace: {missing_z:?}; unparsed compared vars: {unparsed:?}",
        checked + occluded_dz.len(),
        occluded_dz.len()
    );
    let _ = writeln!(t, "mode B rows (report-only): {b_rows}");
    for (why, list) in &skipped {
        let _ = writeln!(t, "skipped {why}: {}", list.len());
        for w in list {
            let _ = writeln!(t, "  {w}");
        }
    }
    print!("{t}");
    let out = std::env::temp_dir().join("golden-trace.txt");
    std::fs::write(&out, &t).unwrap();
    println!("table written to {}", out.display());
    // A value whose depth matches but whose colour differs is a failure, never a skip: every colour mismatch must
    // be an occluded fragment.
    assert!(
        failures.is_empty(),
        "mode A trace mismatches on the visible fragment: {failures:#?}"
    );
    assert!(checked >= 30, "only {checked} mode A values compared");
    assert!(
        missing_z.is_empty(),
        "no SV_POSITION in trace: {missing_z:#?}"
    );
    assert!(unparsed.is_empty(), "unparsed var: {unparsed:#?}");
    // Bounded: a replay regression must not turn into skips (10 of 90 at D-P24).
    let occluded = occluded_dz.len();
    assert!(
        occluded * 100 <= 12 * (checked + occluded),
        "{occluded} occluded of {} mode A traces (over 12%)",
        checked + occluded
    );
    assert!(
        occluded_dz.iter().all(|dz| *dz > 2.0 / 16_777_215.0),
        "an occluded trace within 2 D24 ulps: {occluded_dz:?}"
    );
    assert!(
        rough_metal >= 20,
        "only {rough_metal} Roughness + Metallic values compared"
    );
}

/// Fix round 2: a traced var that is not an array of numbers and nulls is "unparsed", never a silent skip.
#[test]
fn traced_reports_unparsed_vars() {
    let v = serde_json::json!({"worldNormal": [0.0, null, 1.0], "material.Roughness": 0.5, "glossiness": ["x"]});
    assert_eq!(
        traced(DebugOutput::NormalWorld, &v, false),
        Ok([Some(0.5), None, Some(1.0)])
    );
    assert_eq!(traced(DebugOutput::Roughness, &v, true), Err(UNPARSED));
    assert_eq!(traced(DebugOutput::Roughness, &v, false), Err(UNPARSED));
    assert_eq!(traced(DebugOutput::Metallic, &v, true), Err("not traced"));
}
