//! Note ID33: the PBR mode and the define set on the CS 2.0 shaders (Note IE): type 21 draws as CS 2.0 PBR and v1 as
//! vanilla (CS 2.0 hands it to the vanilla reader).
use nif::material::MaterialState;
use nif::pbr2::{feat, PBR2_DEFAULTS};
use nif::props::LightingTrailer;
use render3d::material::{drawn, pbr_mode, perm_material, Pbr};
use render3d::permutation::{defines, Stage};

/// The builtin material (Specular on, strength 1) with the v1 TruePBR bit and `bits`.
fn v1(bits: &[u32]) -> MaterialState {
    let mut m = MaterialState::builtin_default();
    m.slsf2 |= 1 << 23;
    for b in bits {
        m.slsf2 |= 1 << b;
    }
    m
}

/// A type-21 material as `decode_lighting` gives it: the v1 lighting fields read 0, the payload in the trailer.
fn t21(features: u32) -> MaterialState {
    let mut m = MaterialState::builtin_default();
    m.shader_type = 21;
    m.params.glossiness = 0.0;
    m.params.specular_color = [0.0; 3];
    m.params.specular_strength = 0.0;
    m.params.lighting_effect_1 = 0.0;
    m.params.lighting_effect_2 = 0.0;
    m.params.trailer = LightingTrailer::Pbr2 {
        version: 2,
        features,
        params: PBR2_DEFAULTS,
    };
    m
}

fn names(m: &MaterialState, stage: Stage) -> Vec<String> {
    let pm = perm_material(&drawn(m));
    assert_eq!(pm.pbr, pbr_mode(m), "the drawn material carries the mode");
    defines(&pm, stage, &[], false)
        .0
        .into_iter()
        .map(|(k, _)| k)
        .collect()
}

#[test]
fn defines_per_material() {
    let mut mlp = v1(&[24]);
    mlp.shader_type = 11;
    // (what, material, mode, PS defines that must be set, PS defines that must not be)
    type Row = (
        &'static str,
        MaterialState,
        Pbr,
        &'static [&'static str],
        &'static [&'static str],
    );
    let rows: Vec<Row> = vec![
        (
            "v1 is vanilla",
            v1(&[]),
            Pbr::None,
            &["SPECULAR"],
            &["TRUE_PBR"],
        ),
        (
            "v1 soft/rim/back are the vanilla lighting flags",
            v1(&[25, 26, 27]),
            Pbr::None,
            &["SOFT_LIGHTING", "RIM_LIGHTING", "BACK_LIGHTING"],
            &["TRUE_PBR"],
        ),
        (
            "v1 MLP type 11 keeps its technique",
            mlp,
            Pbr::None,
            &["MULTI_LAYER_PARALLAX", "ENVMAP"],
            &["TRUE_PBR"],
        ),
        ("21", t21(0), Pbr::V2, &["TRUE_PBR"], &["SPECULAR", "GLINT"]),
        (
            "21 glint",
            t21(feat::GLINT),
            Pbr::V2,
            &["TRUE_PBR", "GLINT"],
            &[],
        ),
        (
            "21 thin surface",
            t21(feat::THIN_SURFACE | feat::FOLIAGE),
            Pbr::V2,
            &["TRUE_PBR"],
            &["GLINT"],
        ),
        (
            "a foreign 21 with bit 23 is v2",
            {
                let mut m = t21(0);
                m.slsf2 |= 1 << 23;
                m
            },
            Pbr::V2,
            &["TRUE_PBR"],
            &[],
        ),
    ];
    for (what, m, mode, on, off) in rows {
        assert_eq!(pbr_mode(&m), mode, "{what}");
        let d = names(&m, Stage::Pixel);
        for n in on {
            assert!(d.iter().any(|x| x == n), "{what}: {n} missing from {d:?}");
        }
        for n in off {
            assert!(!d.iter().any(|x| x == n), "{what}: {n} set in {d:?}");
        }
    }
    // The vertex stage: the vanilla technique of a v1 type-11 shape.
    let mut mlp = v1(&[24]);
    mlp.shader_type = 11;
    let vs = names(&mlp, Stage::Vertex);
    assert!(vs.iter().any(|x| x == "MULTI_LAYER_PARALLAX"), "{vs:?}");
}

/// The shaders draw type 21 as it is, and a v1 shape without bit 23.
#[test]
fn drawn_clears_only_the_v1_bit() {
    let b = t21(feat::SUBSURFACE);
    assert_eq!(*drawn(&b), b);
    let mut want = v1(&[26]);
    want.slsf2 &= !(1 << 23);
    assert_eq!(*drawn(&v1(&[26])), want);
}

#[path = "common/mod.rs"]
mod common;
use render3d::device::Gpu;
use render3d::material::{EditKind, PBR_GLINT};
use render3d::{ParsedNif, Renderer, ShaderSource, ShapeId, ShapeStatus};

/// A WARP renderer on the bundled shaders; `sync` compiles every program before a call returns, so the draws'
/// cbuffers are the real ones (`draw_cb_floats`).
fn renderer(sync: bool) -> (tempfile::TempDir, Renderer) {
    let cache = tempfile::tempdir().unwrap();
    let mut r = Renderer::with_gpu_source(
        Gpu::new_warp().unwrap(),
        ShaderSource::Bundled,
        cache.path().to_path_buf(),
    )
    .unwrap();
    r.set_async_compile(!sync);
    (cache, r)
}

/// Loads one plane per material, ids 1, 2, …
fn load(r: &mut Renderer, mats: Vec<MaterialState>) {
    let shapes = mats
        .into_iter()
        .enumerate()
        .map(|(i, m)| {
            let mut s = common::plane_shape(i as u32 + 1, common::IDENT, [1.0; 3], 0);
            s.material = m;
            s
        })
        .collect();
    let rep = r.load_parsed(
        &ParsedNif {
            shapes,
            ..Default::default()
        },
        &|_| None,
    );
    assert!(
        rep.errors.is_empty() && rep.compile_errors.is_empty(),
        "{rep:?}"
    );
}

fn cb(r: &Renderer, id: u32, var: &str) -> Vec<f32> {
    r.draw_cb_floats(ShapeId(id), var)
        .unwrap_or_else(|| panic!("shape {id} has no {var}"))
}

fn flags(r: &Renderer, id: u32) -> u32 {
    r.draw_cb_floats(ShapeId(id), "PBRFlags")
        .map_or(0, |f| f[0].to_bits())
}

fn status(r: &Renderer, id: u32) -> ShapeStatus {
    r.shapes()
        .into_iter()
        .find(|s| s.id == ShapeId(id))
        .unwrap()
        .status
}

/// Note ID33, on the draws the renderer builds (sync compile, WARP): a type-21 glint shape gets its CS 2.0 block; a v1
/// shape is vanilla (no PBRFlags).
#[test]
fn the_renderer_writes_the_cs2_pbr_block() {
    let (_c, mut r) = renderer(true);
    let mut g = t21(feat::GLINT);
    let LightingTrailer::Pbr2 { params, .. } = &mut g.params.trailer else {
        unreachable!()
    };
    params[..3].copy_from_slice(&[0.7, 0.2, 1.3]);
    params[17..=20].copy_from_slice(&[1.5, 12.0, 0.02, 3.0]);
    params[21..=24].copy_from_slice(&[0.11, 0.22, 0.33, 0.44]);
    let glint = [1.5, 40.0 - 12.0, 0.02, 3.0];
    load(&mut r, vec![g, v1(&[26])]);

    assert_ne!(flags(&r, 1) & PBR_GLINT, 0, "{:#x}", flags(&r, 1));
    assert_eq!(cb(&r, 1, "PBRParams1"), [0.7, 1.3, 0.2]);
    assert_eq!(cb(&r, 1, "MultiLayerParallaxData"), glint, "glint");
    assert_eq!(cb(&r, 1, "LightingEffectParams"), [0.11, 0.22, 0.33, 0.44]);
    assert_eq!(cb(&r, 2, "LightingEffectParams").len(), 4, "shape 2 draws");
    assert_eq!(flags(&r, 2), 0, "v1 draws as vanilla");
    // The stored material stays the source.
    assert_eq!(r.shapes()[0].material.shader_type, 21);
    assert_eq!(r.shapes()[1].material.slsf2 & 1 << 23, 1 << 23);
}

/// A parameter edit on a type-21 shape is a cbuffer edit that reaches the draw.
#[test]
fn a_type_21_parameter_edit_reaches_the_draw() {
    let (_c, mut r) = renderer(true);
    load(&mut r, vec![t21(0)]);
    let mut m = r.shapes()[0].material.clone();
    let LightingTrailer::Pbr2 { params, .. } = &mut m.params.trailer else {
        unreachable!()
    };
    params[21] = 0.9;
    let kind = r.set_material(ShapeId(1), m.clone(), &|_| None).unwrap();
    assert_eq!(kind, EditKind::Cbuffer);
    assert_eq!(r.shapes()[0].material, m);
    assert_eq!(cb(&r, 1, "LightingEffectParams")[0], 0.9);
}

/// Note ID38: the renderer, not `parse`, decides that a type-21 shape that breaks a CS 2.0 rule is skipped, at load and
/// on every edit, with the rule as its note. `hidden` stays the user's.
#[test]
fn an_invalid_21_is_skipped() {
    let (_c, mut r) = renderer(false);
    let why = "CS 2.0 won't draw this shape: hair must be the only feature";
    let mut bad = t21(feat::HAIR | feat::SUBSURFACE);
    bad.pbr2_error = Some(why.into());
    let mut user_hidden = t21(0);
    user_hidden.hidden = true;
    load(&mut r, vec![bad.clone(), t21(0), user_hidden]);
    assert_eq!(status(&r, 1), ShapeStatus::Hidden, "skipped");
    assert_ne!(status(&r, 2), ShapeStatus::Hidden);
    assert_eq!(status(&r, 3), ShapeStatus::Hidden);
    // a shape the user hid gets no note
    assert_eq!(r.tree_notes(), [format!("plane1: {why}")]);
    assert!(!r.shapes()[0].material.hidden, "hidden is the user's");

    // An edit that fixes it draws it; one that breaks it again skips it.
    let mut fixed = bad.clone();
    fixed.pbr2_error = None;
    r.set_material(ShapeId(1), fixed, &|_| None).unwrap();
    assert_ne!(status(&r, 1), ShapeStatus::Hidden);
    assert!(r.tree_notes().is_empty());
    r.set_material(ShapeId(1), bad, &|_| None).unwrap();
    assert_eq!(status(&r, 1), ShapeStatus::Hidden);
    assert_eq!(status(&r, 3), ShapeStatus::Hidden);
}

/// Every texture the resolver gives: a 4×4 RGBA8 DDS.
fn any_texture(_: &str) -> Option<Vec<u8>> {
    Some(common::dds_rgba8(4, 4, &[[128, 128, 255, 255]; 16]))
}

/// An invalid type-21 shape with diffuse, normal and RMAOS paths.
fn refused_with_slots() -> MaterialState {
    let mut m = t21(feat::HAIR | feat::SUBSURFACE);
    m.pbr2_error = Some("CS 2.0 won't draw this shape: hair must be the only feature".into());
    for (slot, path) in [(0, "a_d.dds"), (1, "a_n.dds"), (5, "a_rmaos.dds")] {
        m.slots[slot] = nif::path::canon_texture(format!("textures\\{path}").as_bytes());
    }
    m
}

/// `m` with its CS 2.0 error cleared.
fn fixed(mut m: MaterialState) -> MaterialState {
    m.pbr2_error = None;
    m
}

/// The draw as a test sees it: the registers bound to a real texture and the PBR block's constants.
fn draw_of(r: &Renderer) -> (Option<Vec<u32>>, Vec<Vec<f32>>) {
    let vars = [
        "PBRFlags",
        "PBRParams1",
        "LightingEffectParams",
        "MultiLayerParallaxData",
    ];
    (
        r.draw_bound(ShapeId(1)),
        vars.iter().map(|v| cb(r, 1, v)).collect(),
    )
}

fn load_with_textures(r: &mut Renderer, m: MaterialState) {
    let mut s = common::plane_shape(1, common::IDENT, [1.0; 3], 0);
    s.material = m;
    let rep = r.load_parsed(
        &ParsedNif {
            shapes: vec![s],
            ..Default::default()
        },
        &any_texture,
    );
    assert!(
        rep.errors.is_empty() && rep.compile_errors.is_empty(),
        "{rep:?}"
    );
}

/// Note ID38: a shape skipped from its load, or edited while skipped, is drawn whole by the edit that fixes it: the
/// same textures and constants as a renderer that loaded it valid (the edit's resolver reads its slots).
#[test]
fn a_skipped_shape_fixed_by_an_edit_draws_whole() {
    let (_cv, mut valid) = renderer(true);
    load_with_textures(&mut valid, fixed(refused_with_slots()));
    let want = draw_of(&valid);
    assert!(
        want.0.as_ref().is_some_and(|b| b.len() >= 3),
        "{:?}",
        want.0
    );

    let (_cs, mut skipped) = renderer(true);
    load_with_textures(&mut skipped, refused_with_slots());
    assert_eq!(status(&skipped, 1), ShapeStatus::Hidden);
    skipped
        .set_material(ShapeId(1), fixed(refused_with_slots()), &any_texture)
        .unwrap();
    assert_eq!(status(&skipped, 1), ShapeStatus::Drawing);
    assert_eq!(draw_of(&skipped), want, "loaded skipped, then fixed");

    // Edited while skipped: a parameter and a new diffuse path, then fixed.
    let mut edited = refused_with_slots();
    let LightingTrailer::Pbr2 { params, .. } = &mut edited.params.trailer else {
        unreachable!()
    };
    params[0] = 0.5;
    params[2] = 0.7;
    edited.slots[0] = nif::path::canon_texture(b"textures\\b_d.dds");
    let (_cv2, mut valid2) = renderer(true);
    load_with_textures(&mut valid2, fixed(edited.clone()));
    let want = draw_of(&valid2);

    let (_cs2, mut skipped2) = renderer(true);
    load_with_textures(&mut skipped2, refused_with_slots());
    skipped2
        .set_material(ShapeId(1), edited.clone(), &any_texture)
        .unwrap();
    assert_eq!(status(&skipped2, 1), ShapeStatus::Hidden);
    skipped2
        .set_material(ShapeId(1), fixed(edited), &any_texture)
        .unwrap();
    assert_eq!(draw_of(&skipped2), want, "edited while skipped, then fixed");
}
