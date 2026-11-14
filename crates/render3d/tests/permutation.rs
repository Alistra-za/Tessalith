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
    let a = drawn(&m, Tree::A);
    assert_eq!(a.shader_type, 11, "two-layer needs the MLP trailer");
    assert_eq!(a.slsf2 & want.slsf2_set, want.slsf2_set);
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
            envmap_strength: 0.0,
        }
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

/// The renderer resolves the drawn material with its own tree, at every (re)permutation: `set_tree` flips a v1 shape
/// between TruePBR and vanilla and keeps type 21 on TRUE_PBR. WARP device, async: nothing is compiled.
#[test]
fn the_renderer_draws_per_tree() {
    #[path = "common/mod.rs"]
    mod common;
    use render3d::device::Gpu;
    use render3d::{ParsedNif, Renderer, ShaderSource, ShapeId};
    let cache = tempfile::tempdir().unwrap();
    let mut r = Renderer::with_gpu_source(
        Gpu::new_warp().unwrap(),
        ShaderSource::Bundled,
        cache.path().to_path_buf(),
    )
    .unwrap();
    r.set_async_compile(true);
    let mut a = common::plane_shape(1, common::IDENT, [1.0; 3], 0);
    a.material.slsf2 |= 1 << 23;
    let mut b = common::plane_shape(2, common::IDENT, [1.0; 3], 0);
    b.material = t21(feat::GLINT);
    r.load_parsed(
        &ParsedNif {
            shapes: vec![a, b],
            ..Default::default()
        },
        &|_| None,
    );
    let ps = |r: &mut Renderer, id: u32| -> Vec<String> {
        r.draw_diag(ShapeId(id))
            .unwrap()
            .0
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    };
    let has = |d: &[String], n: &str| d.iter().any(|x| x == n);
    let (v1a, t21a) = (ps(&mut r, 1), ps(&mut r, 2));
    assert!(has(&v1a, "TRUE_PBR") && !has(&v1a, "SPECULAR"), "{v1a:?}");
    assert!(has(&t21a, "TRUE_PBR") && has(&t21a, "GLINT"), "{t21a:?}");
    r.set_tree(Tree::B);
    let (v1b, t21b) = (ps(&mut r, 1), ps(&mut r, 2));
    assert!(!has(&v1b, "TRUE_PBR") && has(&v1b, "SPECULAR"), "{v1b:?}");
    assert!(has(&t21b, "TRUE_PBR") && has(&t21b, "GLINT"), "{t21b:?}");
    // The stored material stays the source: type 21, not the stand-in.
    let shapes = r.shapes();
    assert_eq!(shapes[1].material.shader_type, 21);
}
