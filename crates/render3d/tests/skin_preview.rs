//! Note GN: the Meshes view's Advanced Skin card (`Renderer::set_skin_preview`) on WARP with the stand-in shader.
//! `STANDIN_SKIN`: pixels = 0.25·diffuse + skinMaterialFlags.x × t71 + skinMaterialFlags.y × t74, plus
//! skinPerGeometry.x in blue when `WETNESS_EFFECTS` is compiled.
mod common;
use common::{channel, dds_rgba8, fixture, renderer};
use nif::material::{MaterialState, SkinPreview};
use render3d::scene::Scene;
use render3d::{EditKind, Renderer, ShapeId};
use std::path::Path;

const SKIN: &[&str] = &[
    "STANDIN_OPAQUE_OK",
    "STANDIN_FLAT",
    "STANDIN_SKIN",
    "CS_SKIN",
];
const CYL1: ShapeId = ShapeId(9);
const GUESS: &str = "textures/standin/light/head_rfaos.dds";
const WET_GUESS: &str = "textures/standin/light/head_wet.dds";
const PICK: &str = "textures/picked/other_rfaos.dds";
const WET_PICK: &str = "textures/picked/other_wet.dds";

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn frame(r: &mut Renderer) -> Vec<u8> {
    r.render(&Scene::neutral(), 64, 64).unwrap()
}

fn cb(r: &Renderer, var: &str) -> Vec<f32> {
    r.draw_cb_floats(CYL1, var)
        .unwrap_or_else(|| panic!("{var} reflected"))
}

/// Every test file the resolver returns: 1×1 colours by canonical path.
fn files() -> Vec<(&'static str, Vec<u8>)> {
    [
        (GUESS, RED),
        (PICK, GREEN),
        (WET_GUESS, WHITE),
        (WET_PICK, WHITE),
    ]
    .into_iter()
    .map(|(p, c)| (p, dds_rgba8(1, 1, &[c])))
    .collect()
}

fn resolve(p: &str) -> Option<Vec<u8>> {
    files().into_iter().find(|f| f.0 == p).map(|f| f.1)
}

/// cylinder_1 of the furniture fixture as a shape of `shader_type` with `_n` and `_s` slots (its RFAOS guess red).
fn skin(cache: &Path, shader_type: u32) -> Renderer {
    let mut r = renderer(cache, SKIN);
    let rep = r.load(&fixture("TestNifFile_Furniture_Col_SE.nif"), &|_| None);
    assert_eq!(rep.shapes, 2, "{rep:?}");
    let mut m = material(&r);
    m.shader_type = shader_type;
    m.set_slot_raw(1, r"textures\standin\head_n.dds");
    m.set_slot_raw(7, r"textures\standin\light\head_s.dds");
    assert!(r.set_material(CYL1, m, &resolve).is_ok());
    r
}

fn material(r: &Renderer) -> MaterialState {
    r.shapes()
        .into_iter()
        .find(|s| s.id == CYL1)
        .unwrap()
        .material
}

fn preview(rfaos: Option<&str>, wet_map: Option<&str>, wet: f32) -> SkinPreview {
    SkinPreview {
        rfaos: rfaos.map(String::from),
        wet_map: wet_map.map(String::from),
        wet,
    }
}

#[test]
fn a_picked_rfaos_replaces_the_guess_and_auto_brings_the_guess_back() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = skin(cache.path(), 5);
    let px = frame(&mut r);
    assert!(
        channel(&px, 0) > channel(&px, 1) + 1000,
        "the guess (red) first"
    );
    let k = r.set_skin_preview(CYL1, preview(Some(PICK), None, 0.0), &resolve);
    assert_eq!(k, Ok(EditKind::Textures));
    assert_eq!(cb(&r, "skinMaterialFlags")[0], 1.0);
    let px = frame(&mut r);
    assert!(
        channel(&px, 1) > channel(&px, 0) + 1000,
        "t71 is the pick (green)"
    );
    r.set_skin_preview(CYL1, SkinPreview::default(), &resolve)
        .unwrap();
    let px = frame(&mut r);
    assert!(
        channel(&px, 0) > channel(&px, 1) + 1000,
        "Auto: the guess again"
    );
}

#[test]
fn the_wet_amount_compiles_wetness_effects_and_moves_per_geometry_wetness() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = skin(cache.path(), 4);
    let dry = channel(&frame(&mut r), 2);
    assert_eq!(cb(&r, "skinPerGeometry"), [0.0; 4]);
    let k = r.set_skin_preview(CYL1, preview(None, None, 0.5), &resolve);
    assert_eq!(k, Ok(EditKind::Permutation), "WETNESS_EFFECTS turns on");
    assert_eq!(cb(&r, "skinPerGeometry"), [0.5, 0.0, 0.0, 0.0]);
    let half = channel(&frame(&mut r), 2);
    assert!(half > dry + 1000, "wetness shows: {dry} -> {half}");
    let k = r.set_skin_preview(CYL1, preview(None, None, 1.0), &resolve);
    assert_eq!(k, Ok(EditKind::Cbuffer), "a slider move compiles nothing");
    assert!(channel(&frame(&mut r), 2) > half + 1000);
    let k = r.set_skin_preview(CYL1, preview(None, None, 0.0), &resolve);
    assert_eq!(
        k,
        Ok(EditKind::Permutation),
        "back to the shape's own program"
    );
    assert_eq!(channel(&frame(&mut r), 2), dry);
}

#[test]
fn a_wetness_map_is_bound_only_while_wetness_is_on() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = skin(cache.path(), 5);
    // Dry and no pick: the `_wet` guess is not even looked at, so the shape draws exactly as before the card.
    assert!(!r.draw_bound(CYL1).unwrap().contains(&74));
    assert_eq!(cb(&r, "skinMaterialFlags")[1], 0.0);
    r.set_skin_preview(CYL1, preview(None, None, 0.3), &resolve)
        .unwrap();
    assert!(r.draw_bound(CYL1).unwrap().contains(&74), "the guess");
    assert_eq!(cb(&r, "skinMaterialFlags")[1], 1.0);
    // A picked map turns wetness on by itself (slider at 0).
    r.set_skin_preview(CYL1, preview(None, Some(WET_PICK), 0.0), &resolve)
        .unwrap();
    assert!(r.draw_bound(CYL1).unwrap().contains(&74));
    assert_eq!(cb(&r, "skinMaterialFlags")[1], 1.0);
}

#[test]
fn a_material_edit_keeps_the_preview() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = skin(cache.path(), 5);
    r.set_skin_preview(CYL1, preview(Some(PICK), None, 0.4), &resolve)
        .unwrap();
    let mut m = material(&r);
    m.skin_preview = None; // what a NIF edit sends
    m.params.glossiness += 1.0;
    r.set_material(CYL1, m, &resolve).unwrap();
    assert_eq!(*material(&r).skin(), preview(Some(PICK), None, 0.4));
    assert_eq!(cb(&r, "skinPerGeometry")[0], 0.4);
    let px = frame(&mut r);
    assert!(channel(&px, 1) > channel(&px, 0) + 1000, "still the pick");
}

#[test]
fn a_non_skin_shape_ignores_the_preview() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = skin(cache.path(), 0);
    let before = frame(&mut r);
    r.set_skin_preview(CYL1, preview(Some(PICK), Some(WET_PICK), 1.0), &resolve)
        .unwrap();
    assert_eq!(cb(&r, "skinMaterialFlags"), [0.0; 4]);
    assert_eq!(frame(&mut r), before);
}

/// Owner data, read-only: the Arch head on the bundled CS shaders with the bundled settings. `WETNESS_EFFECTS`
/// compiles for the skin shapes; on with nothing wet (a picked map, slider 0) it draws like the shape as loaded;
/// slider 1 changes the pixels. PNGs in %TEMP%\sdc-skin-preview.
#[test]
#[ignore]
fn bundled_cs_wetness_compiles_is_neutral_when_dry_and_shows_when_wet() {
    let root = std::path::PathBuf::from(std::env::var("SDC_ACHERUS").expect("SDC_ACHERUS"));
    let mods = root.join("mods");
    let arch = mods.join("Arch Resources");
    let settings = render3d::cs_settings::defaults().clone();
    let resolve = |norm: &str| std::fs::read(arch.join(norm.replace('/', "\\"))).ok();
    let nif = std::fs::read(arch.join(r"meshes\arch\head\Arch_head.nif")).unwrap();
    let out = std::env::temp_dir().join("sdc-skin-preview");
    std::fs::create_dir_all(&out).unwrap();
    let cache = tempfile::tempdir().unwrap();
    let mut r = Renderer::with_gpu(
        render3d::device::Gpu::new().unwrap(),
        cache.path().to_path_buf(),
    )
    .unwrap();
    r.set_settings(settings);
    r.set_features_on(true);
    let rep = r.load(&nif, &resolve);
    assert!(rep.compile_errors.is_empty(), "{rep:?}");
    let skins: Vec<(ShapeId, String)> = r
        .shapes()
        .into_iter()
        .filter(|s| matches!(s.material.shader_type, 4 | 5))
        .map(|s| (s.id, s.material.slots[1].norm.clone()))
        .collect();
    assert!(!skins.is_empty());
    let (w, h) = (512, 512);
    let shot = |r: &mut Renderer, name: &str| {
        let px = r.render(&Scene::neutral(), w, h).unwrap();
        let png = out.join(format!("arch_head_{name}.png"));
        image::save_buffer(&png, &px, w, h, image::ExtendedColorType::Rgba8).unwrap();
        println!("saved {}", png.display());
        px
    };
    let base = shot(&mut r, "as_loaded");
    let set = |r: &mut Renderer, wet_map: bool, wet: f32| {
        for (id, normal) in &skins {
            let p = preview(None, wet_map.then_some(normal.as_str()), wet);
            r.set_skin_preview(*id, p, &resolve).unwrap();
            let (defines, _, srvs) = r.draw_diag(*id).unwrap();
            assert_eq!(
                defines.iter().any(|d| d.0 == "WETNESS_EFFECTS"),
                wet_map || wet > 0.0,
                "{defines:?}"
            );
            println!("shape {} wet {wet} map {wet_map}: bound {srvs:?}", id.0);
            assert_eq!(
                r.shapes().iter().find(|s| s.id == *id).unwrap().status,
                render3d::ShapeStatus::Drawing
            );
        }
    };
    // A wetness map with the slider at 0 (the head's own normal map stands in for one).
    set(&mut r, true, 0.0);
    let dry = shot(&mut r, "map_dry");
    let diff = |a: &[u8], b: &[u8]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| u64::from(x.abs_diff(*y)))
            .sum::<u64>() as f64
            / a.len() as f64
    };
    println!("mean |dry - as loaded| = {:.4}", diff(&dry, &base));
    assert!(diff(&dry, &base) < 0.5, "WETNESS_EFFECTS alone is neutral");
    set(&mut r, false, 1.0);
    let wet = shot(&mut r, "wet_1");
    println!("mean |wet - as loaded| = {:.4}", diff(&wet, &base));
    // CS's wetness is sparse droplets (2026-11-03: mean 0.15, ~1% of the frame's pixels changed).
    assert!(diff(&wet, &base) > 0.05, "slider 1 shows");
}

#[test]
fn an_unknown_shape_is_an_error() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = skin(cache.path(), 5);
    assert!(r
        .set_skin_preview(ShapeId(9999), SkinPreview::default(), &resolve)
        .is_err());
}
