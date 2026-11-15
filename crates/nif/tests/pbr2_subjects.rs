//! Task 15's type-21 golden subjects (Note ID48): an owner v1 PBR mesh becomes CS 2.0 type 21 by this branch's own
//! conversion (`SetField ShaderType 21`, then the feature bits and parameters a subject needs), gets its PBRFuzz or
//! PBRThinSurface extra data, and is refused when CS 2.0 would not draw it. The writer reads owner meshes, so it is
//! `#[ignore]` and takes its paths from `SDC_PBR2_SUBJECTS_MANIFEST` and `SDC_PBR2_SUBJECTS_OUT`.
mod builder;
use builder::{scenes, Nif, W};
use nif::edit::validate::Warning;
use nif::edit::validate::{always_blocks, validate, Level};
use nif::edit::{anchor_role, apply, Op, Outcome, Role};
use nif::extra::{encode_extra, ExtraValue};
use nif::fields::{locate, Field, Value};
use nif::header::Header;
use nif::material::SLSF2_TRUE_PBR as PBR;
use nif::model::{read_model, NifModel};
use nif::net::encode_net;
use nif::pbr2::feat::*;
use nif::props::LightingTrailer;
use nif::write::NifDoc;

/// What one subject needs beyond the plain conversion.
#[derive(Debug, Default, Clone, PartialEq)]
struct Subject {
    /// Replaces the feature bits the v1 flags gave.
    features: Option<u32>,
    /// Bits switched on beside the v1 (or replaced) ones, per shader.
    add: u32,
    /// Parameter overrides, as (index, value).
    params: Vec<(u8, f32)>,
    /// PBRFuzz: fuzz roughness, texture switch.
    fuzz: Option<[f32; 2]>,
    /// PBRThinSurface: subsurface balance, front and back specular weight, back roughness scale.
    thin: Option<[f32; 4]>,
}

/// The converted bytes and what the conversion said (op notes, then any Warn text). Every v1 PBR shader (type 0 or 11
/// with SLSF2 23) becomes type 21; other shaders stay as they are. Err when no shader is v1 PBR, an op does not apply,
/// or the result has a finding `refused` names (any type-21 Error, or a warning CS 2.0 always blocks on).
fn convert(src: &[u8], s: &Subject) -> Result<(Vec<u8>, Vec<String>), String> {
    let m = read_model(src).map_err(|e| e.to_string())?;
    let mut shaders: Vec<(u32, u32)> = vec![]; // (shader, a shape using it)
    for sh in &m.shapes {
        let Some(b) = sh.shader else { continue };
        let v1 = m.lighting.get(&b).is_some_and(|l| {
            matches!(l.lighting.shader_type, 0 | 11) && l.lighting.slsf2 & PBR != 0
        });
        if v1 && !shaders.iter().any(|&(x, _)| x == b) {
            shaders.push((b, sh.block));
        }
    }
    if shaders.is_empty() {
        return Err("no v1 PBR shape".into());
    }
    let mut notes = vec![];
    let to_21 = |m: &NifModel| -> Result<Vec<Op>, String> {
        shaders
            .iter()
            .map(|&(b, shape)| {
                let at = anchor_role(m, Some(shape), Role::Shader).ok_or("no shader anchor")?;
                let base = Value::U32(m.lighting[&b].lighting.shader_type);
                Ok(Op::SetField {
                    at,
                    field: Field::ShaderType,
                    value: Value::U32(21),
                    base,
                })
            })
            .collect()
    };
    let mut bytes = run(src, &to_21(&m)?, &mut notes)?;
    // the feature bits and parameters, on the type-21 blocks (same block numbers)
    let m = read_model(&bytes).map_err(|e| e.to_string())?;
    let h = Header::parse(&bytes).map_err(|e| e.to_string())?;
    let mut ops = vec![];
    for &(b, shape) in &shaders {
        let at = anchor_role(&m, Some(shape), Role::Shader).ok_or("no shader anchor")?;
        let now = |field| locate(&h, &bytes, b as usize, field).map(|x| x.1);
        let mut set = |field, value| -> Result<(), String> {
            let base = now(field).map_err(|e| e.to_string())?;
            ops.push(Op::SetField {
                at: at.clone(),
                field,
                value,
                base,
            });
            Ok(())
        };
        if s.features.is_some() || s.add != 0 {
            let v1 = now(Field::PbrFeatures)
                .map_err(|e| e.to_string())?
                .as_u32()
                .unwrap_or(0);
            set(
                Field::PbrFeatures,
                Value::U32(s.features.unwrap_or(v1) | s.add),
            )?;
        }
        for &(i, v) in &s.params {
            set(Field::PbrParam(i), Value::F32(v.to_bits()))?;
        }
    }
    bytes = run(&bytes, &ops, &mut notes)?;
    for &(b, _) in &shaders {
        for (name, v) in [
            ("PBRFuzz", s.fuzz.map(|f| f.to_vec())),
            ("PBRThinSurface", s.thin.map(|t| t.to_vec())),
        ] {
            if let Some(v) = v {
                bytes = attach_floats(&bytes, b, name, v)?;
            }
        }
    }
    let m = read_model(&bytes).map_err(|e| e.to_string())?;
    let (bad, rest): (Vec<Warning>, Vec<Warning>) =
        validate(&m, None).into_iter().partition(refused);
    if !bad.is_empty() {
        let texts: Vec<String> = bad.into_iter().map(|w| w.text).collect();
        return Err(texts.join("; "));
    }
    notes.extend(
        rest.into_iter()
            .map(|w| format!("{:?}: {}", w.level, w.text)),
    );
    Ok((bytes, notes))
}

/// A finding that refuses a subject: any Error, or a warning CS 2.0 always blocks on (`WONT_DRAW`).
fn refused(w: &Warning) -> bool {
    w.level == Level::Error || always_blocks(w)
}

/// `ops` on `src`; Err unless every op applied. Its notes go to `notes`.
fn run(src: &[u8], ops: &[Op], notes: &mut Vec<String>) -> Result<Vec<u8>, String> {
    let r = apply(src, ops).map_err(|e| e.to_string())?;
    if let Some(o) = r.outcomes.iter().find(|o| **o != Outcome::Applied) {
        return Err(format!("{o:?}"));
    }
    notes.extend(r.notes.into_iter().map(|(_, n)| n));
    Ok(r.bytes)
}

/// Appends a NiFloatsExtraData named `name` to the extra-data list of shader property `shader`. No edit op adds one
/// (AddExtra writes the editable kinds on a shape or the root), so this re-emits the property's NiObjectNET itself.
fn attach_floats(src: &[u8], shader: u32, name: &str, v: Vec<f32>) -> Result<Vec<u8>, String> {
    let m = read_model(src).map_err(|e| e.to_string())?;
    let net = m
        .lighting
        .get(&shader)
        .ok_or("not a lighting shader")?
        .prop
        .net
        .clone();
    if m.extras_of(&net)
        .iter()
        .any(|e| e.name.as_deref() == Some(name.as_bytes()))
    {
        return Err(format!("the shader already has a {name}"));
    }
    let mut doc = NifDoc::open(src).map_err(|e| e.to_string())?;
    let nm = doc.string(name.as_bytes()).map_err(|e| e.to_string())?;
    let (kind, body) = encode_extra(nm, &ExtraValue::Floats(v), -1);
    let new = doc
        .append(kind.as_bytes(), body)
        .map_err(|e| e.to_string())?;
    let base = doc.header().offsets()[shader as usize];
    let old = doc.block(shader);
    let mut extra = net.extra.clone();
    extra.push(new as i32);
    let mut out = old[..net.start - base].to_vec();
    out.extend(encode_net(net.name, &extra, net.controller));
    out.extend(&old[net.end - base..]);
    doc.set_block(shader, out);
    Ok(doc.bytes())
}

// ---- the writer ----

/// Model-space box (min, max) of the BSTriShapes, each moved by its own transform (parent nodes ignored): for laying
/// out the capture plan. `None` when no shape decodes.
fn bounds(b: &[u8]) -> Option<([f32; 3], [f32; 3])> {
    let (h, m) = (Header::parse(b).ok()?, read_model(b).ok()?);
    let mut r: Option<([f32; 3], [f32; 3])> = None;
    for s in &m.shapes {
        let Ok(mesh) = nif::geom::read_shape(&h, b, s.block as usize) else {
            continue;
        };
        let t = mesh.transform;
        for p in &mesh.positions {
            let q: [f32; 3] =
                std::array::from_fn(|i| t[i][0] * p[0] + t[i][1] * p[1] + t[i][2] * p[2] + t[i][3]);
            let (lo, hi) = r.get_or_insert((q, q));
            for i in 0..3 {
                lo[i] = lo[i].min(q[i]);
                hi[i] = hi[i].max(q[i]);
            }
        }
    }
    r
}

/// One manifest line, tab separated: label, source NIF, game path (`meshes\...`), features (hex to replace, `+hex` to
/// add, `-` for the v1 conversion's), parameters (`i=v,i=v` or `-`), PBRFuzz (`a,b` or `-`), PBRThinSurface (`a,b,c,d` or `-`).
fn parse_line(line: &str) -> Result<(String, String, String, Subject), String> {
    let c: Vec<&str> = line.split('\t').collect();
    let [label, src, game, feat, params, fuzz, thin] = c[..] else {
        return Err(format!("{} columns, not 7: {line}", c.len()));
    };
    let floats = |t: &str| -> Result<Vec<f32>, String> {
        t.split(',')
            .map(|x| x.trim().parse::<f32>().map_err(|e| format!("{x}: {e}")))
            .collect()
    };
    fn opt(t: &str) -> Option<&str> {
        (t != "-").then_some(t)
    }
    let hex =
        |f: &str| u32::from_str_radix(f.trim_start_matches("0x"), 16).map_err(|e| e.to_string());
    let (features, add) = match opt(feat) {
        None => (None, 0),
        Some(f) => match f.strip_prefix('+') {
            Some(a) => (None, hex(a)?),
            None => (Some(hex(f)?), 0),
        },
    };
    let params = match opt(params) {
        None => vec![],
        Some(p) => p
            .split(',')
            .map(|kv| {
                let (k, v) = kv.split_once('=').ok_or(format!("{kv}: not i=v"))?;
                let k = k.trim().parse::<u8>().map_err(|e| e.to_string())?;
                Ok((k, v.trim().parse::<f32>().map_err(|e| e.to_string())?))
            })
            .collect::<Result<_, String>>()?,
    };
    let fuzz = opt(fuzz)
        .map(|t| {
            floats(t)?
                .try_into()
                .map_err(|_| "PBRFuzz takes 2 floats".to_string())
        })
        .transpose()?;
    let thin = opt(thin)
        .map(|t| {
            floats(t)?
                .try_into()
                .map_err(|_| "PBRThinSurface takes 4 floats".to_string())
        })
        .transpose()?;
    let s = Subject {
        features,
        add,
        params,
        fuzz,
        thin,
    };
    Ok((label.into(), src.into(), game.into(), s))
}

/// Writes each manifest subject to `SDC_PBR2_SUBJECTS_OUT\<game path>`. A file already there is never replaced
/// (`create_new`). Every subject is converted and every destination checked absent before the first write, so a
/// refused subject or an existing file writes nothing and fails the run.
#[test]
#[ignore = "owner meshes: set SDC_PBR2_SUBJECTS_MANIFEST and SDC_PBR2_SUBJECTS_OUT"]
fn write_pbr2_subjects() {
    use std::io::Write;
    let (Ok(man), Ok(out)) = (
        std::env::var("SDC_PBR2_SUBJECTS_MANIFEST"),
        std::env::var("SDC_PBR2_SUBJECTS_OUT"),
    ) else {
        panic!("set SDC_PBR2_SUBJECTS_MANIFEST and SDC_PBR2_SUBJECTS_OUT");
    };
    let text = std::fs::read_to_string(&man).unwrap();
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .collect();
    let mut made = vec![];
    for line in &lines {
        let (label, src, game, s) = parse_line(line).unwrap();
        let src = std::fs::read(&src).unwrap_or_else(|e| panic!("{label}: {src} {e}"));
        let (b, notes) = convert(&src, &s).unwrap_or_else(|e| panic!("{label}: {e}"));
        made.push((label, game, b, notes));
    }
    let taken = existing(&out, made.iter().map(|m| m.1.as_str()));
    assert!(
        taken.is_empty(),
        "already there, nothing written: {taken:?}"
    );
    for (label, game, b, notes) in made {
        let dst = std::path::Path::new(&out).join(&game);
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dst)
            .unwrap_or_else(|e| panic!("{label}: {} {e}", dst.display()));
        f.write_all(&b).unwrap();
        f.sync_all().unwrap();
        let m = read_model(&b).unwrap();
        let shaders: Vec<String> = m
            .lighting
            .values()
            .filter_map(|l| match &l.lighting.params.trailer {
                LightingTrailer::Pbr2 { features, .. } => Some(format!(
                    "21 features {features:#x} extra {:?}",
                    l.lighting.params.pbr2_extra
                )),
                _ => None,
            })
            .collect();
        println!(
            "{label}\t{}\t{} bytes\tbox {:?}\t{shaders:?}\t{}",
            dst.display(),
            b.len(),
            bounds(&b),
            notes.join(" | ")
        );
    }
}

/// The destinations under `out` that already exist (or cannot be checked).
fn existing<'a>(out: &str, games: impl Iterator<Item = &'a str>) -> Vec<String> {
    games
        .map(|g| std::path::Path::new(out).join(g))
        .filter(|p| !matches!(p.try_exists(), Ok(false)))
        .map(|p| p.display().to_string())
        .collect()
}

#[test]
fn existing_lists_only_taken_destinations() {
    let dir = std::env::temp_dir().join(format!("sdc-pbr2-subjects-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("meshes")).unwrap();
    std::fs::write(dir.join("meshes").join("a.nif"), b"x").unwrap();
    let out = dir.to_str().unwrap();
    let taken = existing(out, ["meshes\\a.nif", "meshes\\b.nif"].into_iter());
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(taken.len(), 1, "{taken:?}");
    assert!(taken[0].ends_with("a.nif"), "{taken:?}");
}

#[test]
fn manifest_line_parses() {
    let (l, src, game, s) = parse_line(
        "fuzz\tC:\\m\\a.nif\tmeshes\\a.nif\t0x220\t21=0.6,0=0.9\t0.35,1\t0.3,0.7,0.2,1.5",
    )
    .unwrap();
    assert_eq!(
        (l.as_str(), src.as_str(), game.as_str()),
        ("fuzz", "C:\\m\\a.nif", "meshes\\a.nif")
    );
    assert_eq!(
        s,
        Subject {
            features: Some(FUZZ | THIN_SURFACE),
            add: 0,
            params: vec![(21, 0.6), (0, 0.9)],
            fuzz: Some([0.35, 1.0]),
            thin: Some([0.3, 0.7, 0.2, 1.5]),
        }
    );
    let (_, _, _, s) = parse_line("b\ts\tg\t-\t-\t-\t-").unwrap();
    assert_eq!(s, Subject::default());
    let (_, _, _, s) = parse_line("b\ts\tg\t+0x100\t-\t-\t-").unwrap();
    assert_eq!((s.features, s.add), (None, FOLIAGE));
    assert!(parse_line("b\ts\tg\t-\t-\t1\t-").is_err()); // one PBRFuzz float
    assert!(parse_line("b\ts\tg").is_err());
}

// ---- tests of the conversion ----

/// A one-shape SE NIF: triangle shape "S" (0) -> BSLightingShaderProperty of `shader_type` with SLSF2 `slsf2` (1)
/// -> texture set (2) with `slots` filled (0-based indices: index 5 is slot 6, 1-based, the RMAOS).
fn v1_scene(shader_type: u32, slsf2: u32, trailer: &[f32], slots: &[usize]) -> Vec<u8> {
    let mut n = Nif::new(100);
    let s = n.string("S");
    n.block("BSTriShape", scenes::tri_shape(s, 1));
    let body = W::default()
        .u32(shader_type)
        .raw(&nif::net::encode_net(-1, &[], -1))
        .u32(0x8000_0000) // slsf1
        .u32(slsf2)
        .fs(&[0.0, 0.0, 1.0, 1.0]) // uv offset, scale
        .i32(2) // texture set
        .fs(&[0.0, 0.0, 0.0, 1.0]) // emissive colour, multiple
        .u32(3) // clamp
        .fs(&[1.0, 0.0, 0.04]) // alpha, refraction, glossiness (= specular level)
        .fs(&[0.9, 0.5, 0.4, 0.8]) // specular colour, strength (= roughness scale)
        .fs(&[0.35, 1.0]) // lighting effect 1, 2
        .fs(trailer)
        .0;
    n.block("BSLightingShaderProperty", body);
    let names = [
        "a_d", "a_n", "a_g", "a_p", "a_e", "a_rmaos", "a_f", "a_s", "x",
    ];
    let mut paths = [""; 9].map(String::from);
    for &i in slots {
        paths[i] = format!("textures\\{}.dds", names[i]);
    }
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    n.block("BSShaderTextureSet", scenes::texset(&refs));
    n.bytes()
}

const SLOTS: [usize; 3] = [0, 1, 5];

/// (shader type, features, params, extra, SLSF2) of the scene's one shader.
fn type21_of(b: &[u8]) -> (u32, u32, [f32; 25], Option<nif::pbr2::Pbr2Extra>, u32) {
    let m = read_model(b).unwrap();
    let l = &m.lighting[&1].lighting;
    let (f, p) = match &l.params.trailer {
        LightingTrailer::Pbr2 {
            features, params, ..
        } => (*features, *params),
        _ => (0, [0.0; 25]),
    };
    (l.shader_type, f, p, l.params.pbr2_extra.clone(), l.slsf2)
}

fn errors(b: &[u8]) -> Vec<String> {
    validate(&read_model(b).unwrap(), None)
        .into_iter()
        .filter(refused)
        .map(|w| w.text)
        .collect()
}

#[test]
fn fuzz_subject_is_type_21_with_its_pbrfuzz() {
    let src = v1_scene(11, 1 | PBR | 1 << 25, &[0.5, 0.4, 0.3, 0.6, 1.0], &SLOTS);
    let s = Subject {
        fuzz: Some([0.35, 0.0]),
        ..Subject::default()
    };
    let (b, notes) = convert(&src, &s).unwrap();
    let (t, f, p, x, s2) = type21_of(&b);
    assert_eq!((t, f), (21, FUZZ));
    assert_eq!(p[13..=16], [0.5, 0.4, 0.3, 0.6]); // the v1 trailer, by from_v1
    assert_eq!(x.unwrap().fuzz, Some(vec![0.35, 0.0]));
    assert_eq!(s2 & nif::pbr2::V1_SLSF2, 0);
    assert!(errors(&b).is_empty(), "{:?}", errors(&b));
    assert!(
        notes
            .iter()
            .any(|n| n.starts_with("Converted from v1 PBR: Fuzz")),
        "{notes:?}"
    );
}

#[test]
fn feature_bits_and_thin_surface_extra() {
    let src = v1_scene(0, 1 | PBR | 1 << 26, &[], &SLOTS);
    let s = Subject {
        features: Some(SUBSURFACE | THIN_SURFACE),
        params: vec![(21, 0.6)],
        thin: Some([0.3, 0.7, 0.2, 1.5]),
        ..Subject::default()
    };
    let (b, _) = convert(&src, &s).unwrap();
    let (t, f, p, x, _) = type21_of(&b);
    assert_eq!((t, f), (21, SUBSURFACE | THIN_SURFACE));
    assert_eq!(p[3..=6], [0.9, 0.5, 0.4, 0.35]); // subsurface colour and opacity from v1
    assert_eq!(p[21], 0.6);
    let x = x.unwrap();
    assert_eq!((x.thin, x.fuzz), (Some(vec![0.3, 0.7, 0.2, 1.5]), None));
    assert!(errors(&b).is_empty(), "{:?}", errors(&b));
}

#[test]
fn added_bits_keep_the_v1_features() {
    let src = v1_scene(0, 1 | PBR | 1 << 26, &[], &SLOTS);
    let s = Subject {
        add: FOLIAGE,
        ..Subject::default()
    };
    let (b, _) = convert(&src, &s).unwrap();
    assert_eq!(type21_of(&b).1, SUBSURFACE | FOLIAGE);
}

#[test]
fn hair_from_a_plain_v1_shape() {
    let src = v1_scene(0, 1 | PBR, &[], &SLOTS);
    let s = Subject {
        features: Some(HAIR),
        ..Subject::default()
    };
    let (b, _) = convert(&src, &s).unwrap();
    assert_eq!(type21_of(&b).1, HAIR);
    assert_eq!(type21_of(&b).0, 21);
}

#[test]
fn refuses_what_cs2_would_not_draw() {
    // no RMAOS: index 5 (slot 6, 1-based) left empty
    let src = v1_scene(0, 1 | PBR, &[], &[0, 1]);
    let e = convert(&src, &Subject::default()).unwrap_err();
    assert!(e.contains(nif::edit::validate::WONT_DRAW), "{e}");
    // hair beside subsurface
    let src = v1_scene(0, 1 | PBR, &[], &SLOTS);
    let s = Subject {
        features: Some(HAIR | SUBSURFACE),
        ..Subject::default()
    };
    let e = convert(&src, &s).unwrap_err();
    assert!(e.contains(nif::pbr2::HAIR_ALONE), "{e}");
}

#[test]
fn a_mesh_without_v1_pbr_is_refused() {
    let src = v1_scene(0, 1, &[], &SLOTS);
    let e = convert(&src, &Subject::default()).unwrap_err();
    assert!(e.contains("no v1 PBR shape"), "{e}");
}
