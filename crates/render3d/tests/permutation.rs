//! Note ID33: the PBR mode and the define set per shader tree. Tree A draws v1 as TruePBR and type 21 as its `to_v1`
//! stand-in; tree B draws type 21 as CS 2.0 PBR and v1 as vanilla (CS 2.0 hands it to the vanilla reader).
use nif::material::MaterialState;
use nif::pbr2::{feat, PBR2_DEFAULTS};
use nif::props::LightingTrailer;
use render3d::material::{drawn, pbr_mode, perm_material, Pbr};
use render3d::permutation::{defines, Stage};
use render3d::Tree;

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

fn names(m: &MaterialState, tree: Tree, stage: Stage) -> Vec<String> {
    let pm = perm_material(&drawn(m, tree));
    assert_eq!(
        pm.pbr,
        pbr_mode(m, tree),
        "the drawn material carries the mode"
    );
    defines(&pm, stage, &[], false)
        .0
        .into_iter()
        .map(|(k, _)| k)
        .collect()
}

#[test]
fn defines_per_tree() {
    let mut mlp = v1(&[24]);
    mlp.shader_type = 11;
    let mut coat21 = t21(feat::TWO_LAYER);
    coat21.slsf2 |= 1 << 4;
    // (what, material, tree, mode, PS defines that must be set, PS defines that must not be)
    type Row = (
        &'static str,
        MaterialState,
        Tree,
        Pbr,
        &'static [&'static str],
        &'static [&'static str],
    );
    let rows: Vec<Row> = vec![
        (
            "v1 on A",
            v1(&[]),
            Tree::A,
            Pbr::V1,
            &["TRUE_PBR"],
            &["SPECULAR"],
        ),
        (
            "v1 on B is vanilla",
            v1(&[]),
            Tree::B,
            Pbr::None,
            &["SPECULAR"],
            &["TRUE_PBR"],
        ),
        (
            "v1 soft/rim/back on B are the vanilla lighting flags",
            v1(&[25, 26, 27]),
            Tree::B,
            Pbr::None,
            &["SOFT_LIGHTING", "RIM_LIGHTING", "BACK_LIGHTING"],
            &["TRUE_PBR"],
        ),
        (
            "v1 MLP type 11 on B keeps its technique",
            mlp.clone(),
            Tree::B,
            Pbr::None,
            &["MULTI_LAYER_PARALLAX", "ENVMAP"],
            &["TRUE_PBR"],
        ),
        (
            "v1 MLP type 11 on A drops it",
            mlp,
            Tree::A,
            Pbr::V1,
            &["TRUE_PBR"],
            &["MULTI_LAYER_PARALLAX", "ENVMAP"],
        ),
        (
            "21 on B",
            t21(0),
            Tree::B,
            Pbr::V2,
            &["TRUE_PBR"],
            &["SPECULAR", "GLINT"],
        ),
        (
            "21 glint on B",
            t21(feat::GLINT),
            Tree::B,
            Pbr::V2,
            &["TRUE_PBR", "GLINT"],
            &[],
        ),
        (
            "21 thin surface on B",
            t21(feat::THIN_SURFACE | feat::FOLIAGE),
            Tree::B,
            Pbr::V2,
            &["TRUE_PBR"],
            &["GLINT"],
        ),
        (
            "21 on A via to_v1",
            t21(0),
            Tree::A,
            Pbr::V1,
            &["TRUE_PBR"],
            &["SPECULAR", "GLINT"],
        ),
        (
            "21 glint on A: to_v1 sets Fit_Slope",
            t21(feat::GLINT),
            Tree::A,
            Pbr::V1,
            &["TRUE_PBR", "GLINT"],
            &[],
        ),
        (
            "21 two-layer on A: the type-11 stand-in, technique dropped",
            coat21,
            Tree::A,
            Pbr::V1,
            &["TRUE_PBR"],
            &["MULTI_LAYER_PARALLAX", "ENVMAP"],
        ),
        (
            "a foreign 21 with bit 23 is v2 on B",
            {
                let mut m = t21(0);
                m.slsf2 |= 1 << 23;
                m
            },
            Tree::B,
            Pbr::V2,
            &["TRUE_PBR"],
            &[],
        ),
    ];
    for (what, m, tree, mode, on, off) in rows {
        assert_eq!(pbr_mode(&m, tree), mode, "{what}");
        let d = names(&m, tree, Stage::Pixel);
        for n in on {
            assert!(d.iter().any(|x| x == n), "{what}: {n} missing from {d:?}");
        }
        for n in off {
            assert!(!d.iter().any(|x| x == n), "{what}: {n} set in {d:?}");
        }
    }
    // The vertex stage: the vanilla technique of a v1 type-11 shape on B.
    let mut mlp = v1(&[24]);
    mlp.shader_type = 11;
    let vs = names(&mlp, Tree::B, Stage::Vertex);
    assert!(vs.iter().any(|x| x == "MULTI_LAYER_PARALLAX"), "{vs:?}");
}

/// Tree A's stand-in for type 21 is `nif::pbr2::to_v1`'s v1 shape (design, settled item 5).
#[test]
fn tree_a_draws_type_21_as_its_to_v1_shape() {
    let mut m = t21(feat::TWO_LAYER | feat::COAT_NORMAL);
    let LightingTrailer::Pbr2 { params, .. } = &mut m.params.trailer else {
        unreachable!()
    };
    params[0] = 0.7;
    params[1] = 0.05;
    params[2] = 0.3;
    params[11] = 0.4;
    params[12] = 0.06;
    let want = nif::pbr2::to_v1(feat::TWO_LAYER | feat::COAT_NORMAL, params);
    // v1 PBR bits and the multi-layer bit on the source go unless the stand-in sets them; other bits stay
    m.slsf2 |= nif::pbr2::V1_SLSF2 | 1 << 24 | 1 << 4;
    let kept = m.slsf2 & !(nif::pbr2::V1_SLSF2 | 1 << 24);
    assert_ne!(kept & 1 << 4, 0);
    let a = drawn(&m, Tree::A);
    assert_eq!(a.shader_type, 11, "two-layer needs the MLP trailer");
    assert_eq!(a.slsf2, kept | want.slsf2_set, "{:#x}", a.slsf2);
    assert!(a.is_pbr());
    assert_eq!(a.params.glossiness, want.glossiness);
    assert_eq!(a.params.specular_strength, want.specular_strength);
    assert_eq!(a.params.specular_color, want.specular_color);
    assert_eq!(a.params.lighting_effect_1, want.lighting_effect_1);
    assert_eq!(a.params.lighting_effect_2, want.lighting_effect_2);
    let mlp = want.mlp.unwrap();
    assert_eq!(
        a.params.trailer,
        LightingTrailer::MultiLayer {
            inner_thickness: mlp[0],
            refraction_scale: mlp[1],
            inner_texture_scale: [mlp[2], mlp[3]],
            // what a conversion to type 11 writes (Note ID40)
            envmap_strength: nif::edit::trailer_defaults(11)[4],
        }
    );
    // Glint: the stand-in's trailer[1] is p18, the log density (Note ID37).
    let mut g = t21(feat::GLINT);
    let LightingTrailer::Pbr2 { params, .. } = &mut g.params.trailer else {
        unreachable!()
    };
    params[17..=20].copy_from_slice(&[1.5, 12.0, 0.02, 3.0]);
    let LightingTrailer::MultiLayer {
        inner_thickness,
        refraction_scale,
        inner_texture_scale,
        ..
    } = drawn(&g, Tree::A).params.trailer
    else {
        panic!("glint needs the MLP trailer")
    };
    assert_eq!(
        [
            inner_thickness,
            refraction_scale,
            inner_texture_scale[0],
            inner_texture_scale[1]
        ],
        [1.5, 12.0, 0.02, 3.0]
    );
    // Without a trailer feature: type 0, no trailer.
    let plain = drawn(&t21(feat::SUBSURFACE), Tree::A).into_owned();
    assert_eq!(plain.shader_type, 0);
    assert_eq!(plain.params.trailer, LightingTrailer::None);
    // Tree B draws the material as it is; tree A leaves a v1 shape as it is.
    let b = t21(feat::SUBSURFACE);
    assert_eq!(*drawn(&b, Tree::B), b);
    assert_eq!(*drawn(&v1(&[26]), Tree::A), v1(&[26]));
}

#[path = "common/mod.rs"]
mod common;
use render3d::device::Gpu;
use render3d::material::{EditKind, PBR_GLINT, PBR_SUBSURFACE};
use render3d::{ParsedNif, Renderer, ShaderSource, ShapeId, ShapeStatus};

/// A WARP renderer on tree A; `sync` compiles every program before a call returns, so the draws' cbuffers are the
/// real ones (`draw_cb_floats`).
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

/// Note ID33, on the draws the renderer builds (sync compile, WARP): tree A writes a type-21 glint shape's v1
/// stand-in block and tree B its CS 2.0 block; a v1 shape is TruePBR on A and vanilla (no PBRFlags) on B.
#[test]
fn each_tree_writes_its_own_pbr_block() {
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

    assert_ne!(flags(&r, 1) & PBR_GLINT, 0, "A: {:#x}", flags(&r, 1));
    assert_eq!(cb(&r, 1, "MultiLayerParallaxData"), glint, "A glint");
    assert_eq!(
        cb(&r, 1, "LightingEffectParams"),
        [0.0, 1.3, 0.0, 0.0],
        "A: v1 lighting effects"
    );
    assert_ne!(flags(&r, 2) & PBR_SUBSURFACE, 0, "A: v1 subsurface");

    r.set_tree(Tree::B);
    assert_ne!(flags(&r, 1) & PBR_GLINT, 0, "B: {:#x}", flags(&r, 1));
    assert_eq!(cb(&r, 1, "PBRParams1"), [0.7, 1.3, 0.2]);
    assert_eq!(cb(&r, 1, "MultiLayerParallaxData"), glint, "B glint");
    assert_eq!(cb(&r, 1, "LightingEffectParams"), [0.11, 0.22, 0.33, 0.44]);
    assert_eq!(cb(&r, 2, "LightingEffectParams").len(), 4, "shape 2 draws");
    assert_eq!(flags(&r, 2), 0, "B draws v1 as vanilla");
    // The stored material stays the source: type 21, not the stand-in.
    assert_eq!(r.shapes()[0].material.shader_type, 21);
}

/// An edit tree A cannot see (p21 on a type-21 shape without subsurface or thin surface) is `Same` there, but the
/// renderer keeps it, so tree B draws it after a switch.
#[test]
fn an_edit_tree_a_cannot_see_reaches_tree_b() {
    let (_c, mut r) = renderer(true);
    load(&mut r, vec![t21(0)]);
    let mut m = r.shapes()[0].material.clone();
    let LightingTrailer::Pbr2 { params, .. } = &mut m.params.trailer else {
        unreachable!()
    };
    params[21] = 0.9;
    let kind = r.set_material(ShapeId(1), m.clone(), &|_| None).unwrap();
    assert_eq!(kind, EditKind::Same);
    assert_eq!(r.shapes()[0].material, m);
    r.set_tree(Tree::B);
    assert_eq!(cb(&r, 1, "LightingEffectParams")[0], 0.9);
}

/// Note ID38: the renderer, not `parse`, decides that tree B skips a type-21 shape that breaks a CS 2.0 rule, at load,
/// on `set_tree` and on every edit; tree A draws it (with the approximation note). `hidden` stays the user's.
#[test]
fn an_invalid_21_is_skipped_on_tree_b_only() {
    let (_c, mut r) = renderer(false);
    let why = "CS 2.0 won't draw this shape: hair must be the only feature";
    let mut bad = t21(feat::HAIR | feat::SUBSURFACE);
    bad.pbr2_error = Some(why.into());
    let mut user_hidden = t21(0);
    user_hidden.hidden = true;
    load(&mut r, vec![bad.clone(), t21(0), user_hidden]);
    let approx = |id: u32| format!("plane{id}: {}", render3d::material::APPROXIMATE);
    assert_ne!(status(&r, 1), ShapeStatus::Hidden, "A draws its stand-in");
    assert_ne!(status(&r, 2), ShapeStatus::Hidden);
    assert_eq!(status(&r, 3), ShapeStatus::Hidden);
    // a shape the user hid gets no note
    assert_eq!(r.tree_notes(), [approx(1), approx(2)]);

    r.set_tree(Tree::B);
    assert_eq!(status(&r, 1), ShapeStatus::Hidden, "B skips it");
    assert_ne!(status(&r, 2), ShapeStatus::Hidden);
    assert_eq!(status(&r, 3), ShapeStatus::Hidden);
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

    r.set_tree(Tree::A);
    assert_ne!(status(&r, 1), ShapeStatus::Hidden, "A again");
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

/// A sync WARP renderer built on `tree`'s bundled shaders (the GUI's start with the box on or off, ID32).
fn sync_on(tree: Tree) -> (tempfile::TempDir, Renderer) {
    let cache = tempfile::tempdir().unwrap();
    let source = match tree {
        Tree::A => ShaderSource::Bundled,
        Tree::B => ShaderSource::BundledB,
    };
    let r = Renderer::with_gpu_source(Gpu::new_warp().unwrap(), source, cache.path().to_path_buf())
        .unwrap();
    assert_eq!(r.tree(), tree);
    (cache, r)
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

/// Note ID38: a shape tree B skipped from its load, or edited while skipped, comes back on `set_tree(A)` with the
/// same textures and constants as a renderer that loaded it on tree A (`begin_all` has no texture resolver).
#[test]
fn a_shape_skipped_on_b_returns_whole_on_a() {
    let (_ca, mut on_a) = sync_on(Tree::A);
    load_with_textures(&mut on_a, refused_with_slots());
    let want = draw_of(&on_a);
    assert!(
        want.0.as_ref().is_some_and(|b| b.len() >= 3),
        "{:?}",
        want.0
    );

    let (_cb, mut on_b) = sync_on(Tree::B);
    load_with_textures(&mut on_b, refused_with_slots());
    assert_eq!(status(&on_b, 1), ShapeStatus::Hidden);
    on_b.set_tree(Tree::A);
    assert_eq!(status(&on_b, 1), ShapeStatus::Drawing);
    assert_eq!(draw_of(&on_b), want, "loaded skipped on B, then A");

    // Edited while skipped: a parameter and a new diffuse path.
    let mut edited = refused_with_slots();
    let LightingTrailer::Pbr2 { params, .. } = &mut edited.params.trailer else {
        unreachable!()
    };
    params[0] = 0.5;
    params[2] = 0.7;
    edited.slots[0] = nif::path::canon_texture(b"textures\\b_d.dds");
    let (_ca2, mut on_a2) = sync_on(Tree::A);
    load_with_textures(&mut on_a2, edited.clone());
    let want = draw_of(&on_a2);

    let (_cb2, mut on_b2) = sync_on(Tree::B);
    load_with_textures(&mut on_b2, refused_with_slots());
    on_b2
        .set_material(ShapeId(1), edited, &any_texture)
        .unwrap();
    assert_eq!(status(&on_b2, 1), ShapeStatus::Hidden);
    on_b2.set_tree(Tree::A);
    assert_eq!(draw_of(&on_b2), want, "edited while skipped on B, then A");
}
