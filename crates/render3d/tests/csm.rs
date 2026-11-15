//! Note IE6: a shape whose shader property name is a `.csm` path takes its PBR material from that file, read through
//! the load's resolver (WARP, the bundled CS 2.0 shaders).
#[path = "../../nif/tests/builder/mod.rs"]
mod builder;
#[path = "common/mod.rs"]
mod common;
use builder::scenes::{lsp, texset, tri_shape};
use nif::material::MaterialState;
use render3d::device::Gpu;
use render3d::material::{
    own_pbr, pbr_mode, Pbr, PBR_FUZZ, PBR_HAS_FUZZ_ROUGHNESS_TEXTURE, PBR_SUBSURFACE,
};
use render3d::{parse, Builtin, EditKind, LoadReport, ParsedNif, Renderer, ShaderSource, ShapeId};

const SHAPE: ShapeId = ShapeId(0);

/// A one-shape NIF: triangle "S" (block 0) -> type-0 BSLightingShaderProperty (1) named `name` -> texture set (2)
/// with diffuse, normal, glow and RMAOS slots.
fn csm_nif(name: &str) -> Vec<u8> {
    let mut n = builder::Nif::new(100);
    let s = n.string("S");
    let name = n.string(name);
    n.block("BSTriShape", tri_shape(s, 1));
    let mut body = lsp(0, 2);
    body[4..8].copy_from_slice(&name.to_le_bytes()); // NiObjectNET name, after the shader type
    n.block("BSLightingShaderProperty", body);
    let mut slots = [""; 9];
    slots[0] = "textures\\a_d.dds";
    slots[1] = "textures\\a_n.dds";
    slots[2] = "textures\\a_g.dds";
    slots[5] = "textures\\a_rmaos.dds";
    n.block("BSShaderTextureSet", texset(&slots));
    n.bytes()
}

fn renderer(cache: &std::path::Path) -> Renderer {
    Renderer::with_gpu_source(
        Gpu::new_warp().unwrap(),
        ShaderSource::Bundled,
        cache.to_path_buf(),
    )
    .unwrap()
}

/// Loads [`csm_nif`] named `materials\t.csm`, the resolver serving `csm` at `materials/t.csm` and no textures.
fn load_with(r: &mut Renderer, csm: &str) -> LoadReport {
    let csm = csm.as_bytes().to_vec();
    let read = move |p: &str| (p == "materials/t.csm").then(|| csm.clone());
    r.load(&csm_nif("materials\\t.csm"), &read)
}

fn material(r: &Renderer) -> MaterialState {
    r.shapes().into_iter().next().unwrap().material
}

fn cb(r: &Renderer, var: &str) -> Vec<f32> {
    r.draw_cb_floats(SHAPE, var)
        .unwrap_or_else(|| panic!("no {var}"))
}

const FUZZ_CSM: &str = r#"{"Version": 1, "PBREnabled": true, "PBRRoughnessScale": 0.7, "PBRFuzzEnabled": true,
    "PBRFuzzRoughnessTexture": true, "PBRFuzzRoughness": 0.4}"#;

#[test]
fn csm_shape_is_pbr() {
    assert_eq!(
        parse(&csm_nif("Data\\Materials\\T.CSM")).shapes[0]
            .material
            .csm_path
            .as_deref(),
        Some("materials/t.csm"),
        "the name field, canonical"
    );
    assert_eq!(
        parse(&csm_nif("Materials\\t.json")).shapes[0]
            .material
            .csm_path,
        None
    );
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path());
    let rep = load_with(&mut r, FUZZ_CSM);
    assert!(
        rep.errors.is_empty() && rep.compile_errors.is_empty(),
        "{rep:?}"
    );
    let m = material(&r);
    assert_eq!(m.shader_type, 0);
    assert_eq!(own_pbr(&m), Pbr::V2);
    assert_eq!(m.csm_error, None);
    assert_eq!(cb(&r, "PBRParams1")[0], 0.7);
    let flags = cb(&r, "PBRFlags")[0].to_bits();
    assert_eq!(
        flags & (PBR_FUZZ | PBR_HAS_FUZZ_ROUGHNESS_TEXTURE),
        PBR_FUZZ | PBR_HAS_FUZZ_ROUGHNESS_TEXTURE,
        "{flags:#x}"
    );
    assert_eq!(cb(&r, "MaterialObjectRGBScale")[3], 0.4);
}

/// The GUI hands edits over as `parse` gives them (the `.csm` not applied yet): the renderer applies it again, so an
/// unchanged shape stays the same CS 2.0 material. The file was resolved at load: the edit's resolver is not asked.
#[test]
fn a_parsed_material_edit_keeps_the_csm() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path());
    load_with(&mut r, FUZZ_CSM);
    let parsed = parse(&csm_nif("materials\\t.csm")).shapes[0]
        .material
        .clone();
    assert_eq!(parsed.pbr, None, "parse does not read the file");
    assert_eq!(r.set_material(SHAPE, parsed, &|_| None), Ok(EditKind::Same));
    assert_eq!(own_pbr(&material(&r)), Pbr::V2);
    assert_eq!(cb(&r, "PBRParams1")[0], 0.7);
}

#[test]
fn csm_textures_replace_slots() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path());
    let rep = load_with(
        &mut r,
        r#"{"Version": 1, "PBREnabled": true, "PBRSubsurfaceEnabled": true,
            "RmaosTexture": "CSM\\T_rmaos.dds", "GlowTexture": ""}"#,
    );
    let m = material(&r);
    assert_eq!(m.slots[5].norm, "textures/csm/t_rmaos.dds");
    assert_eq!(m.slots[2].norm, "", "an empty key clears the slot");
    assert_eq!(m.slots[0].norm, "textures/a_d.dds", "a key the file lacks");
    assert_eq!(
        cb(&r, "PBRFlags")[0].to_bits() & PBR_SUBSURFACE,
        PBR_SUBSURFACE
    );
    // The textures are asked for after the file applied.
    let missing = &rep.missing_textures;
    assert!(
        missing.iter().any(|p| p == "textures/csm/t_rmaos.dds"),
        "{missing:?}"
    );
    assert!(
        !missing.iter().any(|p| p == "textures/a_rmaos.dds"),
        "{missing:?}"
    );
    assert!(
        !missing.iter().any(|p| p == "textures/a_g.dds"),
        "{missing:?}"
    );
}

#[test]
fn broken_csm_draws_vanilla() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path());
    let rep = load_with(&mut r, "{ not json");
    assert!(rep.errors.is_empty(), "{rep:?}");
    let m = material(&r);
    assert_eq!(own_pbr(&m), Pbr::None);
    assert!(m.csm_error.is_some());
    assert_eq!(m.slots[5].norm, "textures/a_rmaos.dds", "the NIF's slots");
    assert!(
        rep.warnings.iter().any(|w| w.contains("materials/t.csm")),
        "{:?}",
        rep.warnings
    );
    let (ps, _, _) = r.draw_diag(SHAPE).unwrap();
    assert!(!ps.iter().any(|(k, _)| k == "TRUE_PBR"), "{ps:?}");

    // A file the resolver lacks is the same.
    let mut r = renderer(cache.path());
    let rep = r.load(&csm_nif("materials\\gone.csm"), &|_| None);
    assert_eq!(own_pbr(&material(&r)), Pbr::None);
    assert!(
        rep.warnings
            .iter()
            .any(|w| w.contains("materials/gone.csm")),
        "{:?}",
        rep.warnings
    );
}

/// A `.csm` the resolver lacked at load is asked for again on the next edit (as a missing texture is), so a file
/// exported meanwhile shows without a reload.
#[test]
fn a_missing_csm_is_asked_again_on_edit() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path());
    r.load(&csm_nif("materials\\t.csm"), &|_| None);
    assert_eq!(own_pbr(&material(&r)), Pbr::None);
    let parsed = parse(&csm_nif("materials\\t.csm")).shapes[0]
        .material
        .clone();
    let csm = FUZZ_CSM.as_bytes().to_vec();
    let read = move |p: &str| (p == "materials/t.csm").then(|| csm.clone());
    r.set_material(SHAPE, parsed, &read).unwrap();
    let m = material(&r);
    assert_eq!(own_pbr(&m), Pbr::V2);
    assert_eq!(m.csm_error, None);
    assert_eq!(cb(&r, "PBRParams1")[0], 0.7);
}

/// A type-21 shape as a plane, named `materials\t.csm` when `csm` is set.
fn t21_plane(csm: bool) -> ParsedNif {
    let mut s = common::plane_shape(0, common::IDENT, [0.0; 3], 1);
    s.material = common::as_t21(s.material, nif::pbr2::feat::SUBSURFACE);
    if csm {
        s.material.csm_path = Some("materials/t.csm".into());
    }
    ParsedNif {
        shapes: vec![s],
        ..Default::default()
    }
}

/// A `.csm` that does not resolve leaves a type-21 shape its block's values (Note IE6).
#[test]
fn broken_csm_on_type_21_keeps_the_block() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path());
    let read = |p: &str| (p == "materials/t.csm").then(|| b"{ not json".to_vec());
    let rep = r.load_parsed(&t21_plane(true), &read);
    assert!(rep.compile_errors.is_empty(), "{rep:?}");
    let m = material(&r);
    assert!(m.csm_error.is_some());
    assert_eq!(m.pbr, m.type21_values());
    assert_eq!(own_pbr(&m), Pbr::V2);
    assert_eq!(
        cb(&r, "PBRFlags")[0].to_bits() & PBR_SUBSURFACE,
        PBR_SUBSURFACE
    );
}

/// An edit from type 21 to another type (no `.csm`) drops the PBR values: the shape draws vanilla.
#[test]
fn a_type_21_shape_retyped_draws_vanilla() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path());
    r.load_parsed(&t21_plane(false), &|_| None);
    assert_eq!(own_pbr(&material(&r)), Pbr::V2);
    let mut m = material(&r);
    m.shader_type = 0;
    r.set_material(SHAPE, m, &|_| None).unwrap();
    assert_eq!(pbr_mode(&material(&r)), Pbr::None);
    let (ps, _, _) = r.draw_diag(SHAPE).unwrap();
    assert!(!ps.iter().any(|(k, _)| k == "TRUE_PBR"), "{ps:?}");
}

/// Guard (Note IE5): a v1 shape (SLSF2 bit 23) draws as vanilla, so it compiles no TRUE_PBR.
#[test]
fn v1_shape_draws_vanilla() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path());
    let mut m = MaterialState::builtin_default();
    m.slsf2 |= nif::material::SLSF2_TRUE_PBR;
    let rep = r.load_builtin(Builtin::Plane, m, &|_| None);
    assert!(rep.compile_errors.is_empty(), "{rep:?}");
    let (ps, _, _) = r.draw_diag(SHAPE).unwrap();
    assert!(!ps.iter().any(|(k, _)| k == "TRUE_PBR"), "{ps:?}");
    assert_eq!(pbr_mode(&material(&r)), Pbr::None);
}
