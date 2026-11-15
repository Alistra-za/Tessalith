//! Note ID33: the PBR mode and the define set on the CS 2.0 shaders (Note IE): a `.csm` shape draws as CS 2.0 PBR
//! and v1 as vanilla (CS 2.0 hands it to the vanilla reader).
use nif::material::MaterialState;
use nif::pbr2::feat;
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

/// The builtin material as a `.csm` shape with `features` (Note IE7: a `.csm` is the only source of PBR values).
fn csm(features: u32) -> MaterialState {
    common::as_csm(MaterialState::builtin_default(), features)
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
    // SOVStairsPiece01's type-11 glint shapes: no SLSF2.24 (Note IE8).
    let mut t11 = v1(&[12]);
    t11.shader_type = 11;
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
            &[],
            &["TRUE_PBR", "SPECULAR"],
        ),
        (
            "v1 soft/rim/back are the vanilla lighting flags, at run time",
            v1(&[25, 26, 27]),
            Pbr::None,
            &[],
            &["TRUE_PBR", "SOFT_LIGHTING", "RIM_LIGHTING", "BACK_LIGHTING"],
        ),
        (
            "v1 MLP type 11 keeps its technique",
            mlp,
            Pbr::None,
            &["MULTI_LAYER_PARALLAX", "ENVMAP"],
            &["TRUE_PBR"],
        ),
        (
            "v1 type 11 without SLSF2.24 draws the Default technique",
            t11.clone(),
            Pbr::None,
            &[],
            &["MULTI_LAYER_PARALLAX", "ENVMAP", "TRUE_PBR"],
        ),
        (
            ".csm",
            csm(0),
            Pbr::V2,
            &["TRUE_PBR"],
            &["SPECULAR", "GLINT"],
        ),
        (
            ".csm glint",
            csm(feat::GLINT),
            Pbr::V2,
            &["TRUE_PBR", "GLINT"],
            &[],
        ),
        (
            ".csm thin surface",
            csm(feat::THIN_SURFACE | feat::FOLIAGE),
            Pbr::V2,
            &["TRUE_PBR"],
            &["GLINT"],
        ),
        (
            "a .csm shape with bit 23 is v2",
            {
                let mut m = csm(0);
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
    let vs = names(&t11, Stage::Vertex);
    assert!(!vs.iter().any(|x| x == "MULTI_LAYER_PARALLAX"), "{vs:?}");
}

/// Note IE8: CS 2.0 reads a vanilla shape's Specular, Soft, Rim and Back lighting from cb4 `PixelShaderDescriptor`, so
/// a v1 shape (drawn vanilla) carries them there; a `.csm` shape carries none.
#[test]
fn lighting_flags_are_runtime_bits() {
    use render3d::permutation::{
        runtime_bits, LIGHTING_BACK, LIGHTING_RIM, LIGHTING_SOFT, LIGHTING_SPECULAR,
    };
    let bits = |m: &MaterialState| runtime_bits(&perm_material(&drawn(m)));
    assert_eq!(bits(&v1(&[])), LIGHTING_SPECULAR);
    assert_eq!(
        bits(&v1(&[25, 26, 27])),
        LIGHTING_SPECULAR | LIGHTING_SOFT | LIGHTING_RIM | LIGHTING_BACK
    );
    let mut dull = v1(&[26]);
    dull.params.specular_strength = 0.0;
    assert_eq!(bits(&dull), LIGHTING_RIM);
    let mut b = csm(feat::SUBSURFACE);
    b.slsf2 |= (1 << 25) | (1 << 26) | (1 << 27);
    assert_eq!(bits(&b), 0);
}

/// The shaders draw a `.csm` shape as it is, and a v1 shape without bit 23.
#[test]
fn drawn_clears_only_the_v1_bit() {
    let b = csm(feat::SUBSURFACE);
    assert_eq!(*drawn(&b), b);
    let mut want = v1(&[26]);
    want.slsf2 &= !(1 << 23);
    assert_eq!(*drawn(&v1(&[26])), want);
}

#[path = "common/mod.rs"]
mod common;
use render3d::device::Gpu;
use render3d::material::{EditKind, PBR_GLINT};
use render3d::permutation::{LIGHTING_RIM, LIGHTING_SPECULAR};
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

/// The `.csm` of [`glint`], which [`files`] serves.
const GLINT: &str = "materials/test/glint.csm";
/// The `.csm` of [`thin`], which [`files`] serves.
const THIN: &str = "materials/test/thin.csm";

/// Glint with thin surface (so the file holds p21-p24), parameters away from the defaults.
fn glint() -> nif::pbr2::PbrValues {
    let mut v = common::csm_values(feat::GLINT | feat::THIN_SURFACE);
    v.params[..3].copy_from_slice(&[0.7, 0.2, 1.3]);
    v.params[17..=20].copy_from_slice(&[1.5, 12.0, 0.02, 3.0]);
    v.params[21..=24].copy_from_slice(&[0.11, 0.22, 0.33, 0.44]);
    v
}

/// Thin surface with subsurface balance 0.9.
fn thin() -> nif::pbr2::PbrValues {
    let mut v = common::csm_values(feat::THIN_SURFACE);
    v.params[21] = 0.9;
    v
}

/// The resolver: the `.csm` files of `common::as_csm`, [`GLINT`] and [`THIN`].
fn files(p: &str) -> Option<Vec<u8>> {
    match p {
        GLINT => Some(common::csm_text(&glint())),
        THIN => Some(common::csm_text(&thin())),
        _ => common::csm_read(p),
    }
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
        &files,
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

/// Note ID33, on the draws the renderer builds (sync compile, WARP): a `.csm` glint shape gets its CS 2.0 block; a v1
/// shape is vanilla (no PBRFlags).
#[test]
fn the_renderer_writes_the_cs2_pbr_block() {
    let (_c, mut r) = renderer(true);
    let g = common::csm_shape(MaterialState::builtin_default(), GLINT, &glint());
    let glint = [1.5, 40.0 - 12.0, 0.02, 3.0];
    load(&mut r, vec![g, v1(&[26])]);

    assert_ne!(flags(&r, 1) & PBR_GLINT, 0, "{:#x}", flags(&r, 1));
    assert_eq!(cb(&r, 1, "PBRParams1"), [0.7, 1.3, 0.2]);
    assert_eq!(cb(&r, 1, "MultiLayerParallaxData"), glint, "glint");
    assert_eq!(cb(&r, 1, "LightingEffectParams"), [0.11, 0.22, 0.33, 0.44]);
    assert_eq!(cb(&r, 2, "LightingEffectParams").len(), 4, "shape 2 draws");
    assert_eq!(flags(&r, 2), 0, "v1 draws as vanilla");
    // Note IE8: the vanilla lighting flags reach cb4 (Specular on in the builtin material, Rim from SLSF2.26).
    let desc = |id| cb(&r, id, "PixelShaderDescriptor")[0].to_bits();
    assert_eq!(desc(2), LIGHTING_SPECULAR | LIGHTING_RIM, "{:#x}", desc(2));
    assert_eq!(desc(1), 0, "{:#x}", desc(1));
    // The stored material stays the source.
    assert_eq!(r.shapes()[0].material.shader_type, 0);
    assert_eq!(r.shapes()[1].material.slsf2 & 1 << 23, 1 << 23);
}

/// Note IE5: clearing SLSF2 bit 23 on a v1 shape changes nothing the shaders draw, so the edit is `Same`; the stored
/// material still follows the edit (the `Same && !rebuild` branch of `set_material`).
#[test]
fn a_v1_bit_edit_is_same_and_kept() {
    let (_c, mut r) = renderer(true);
    load(&mut r, vec![v1(&[])]);
    let mut m = r.shapes()[0].material.clone();
    m.slsf2 &= !(1 << 23);
    assert_eq!(
        r.set_material(ShapeId(1), m.clone(), &|_| None),
        Ok(EditKind::Same)
    );
    assert_eq!(r.shapes()[0].material, m);
    assert_eq!(status(&r, 1), ShapeStatus::Drawing);
}

/// An edit that points a `.csm` shape at a file differing only in a parameter is a cbuffer edit that reaches the draw.
#[test]
fn a_csm_parameter_edit_reaches_the_draw() {
    let (_c, mut r) = renderer(true);
    load(&mut r, vec![csm(feat::THIN_SURFACE)]);
    let m = common::csm_shape(r.shapes()[0].material.clone(), THIN, &thin());
    let kind = r.set_material(ShapeId(1), m.clone(), &files).unwrap();
    assert_eq!(kind, EditKind::Cbuffer);
    assert_eq!(r.shapes()[0].material, m);
    assert_eq!(cb(&r, 1, "LightingEffectParams")[0], 0.9);
}
