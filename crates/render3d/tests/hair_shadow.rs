//! Note GX: CS hair self-shadow (`Hair::HairSelfShadow`, Hair.hlsli:264-299) raymarches the scene depth at t17. The
//! preview draws a depth pre-pass and binds its copy there; `render_*` on the bundled CS, run alone.
use nif::material::MaterialState;
use nif::props::LightingTrailer;
use render3d::scene::{Camera, DirLight, Scene};
use render3d::{ParsedNif, ParsedShape, Renderer, ShapeId};
use serde_json::json;

const W: u32 = 256;
const H: u32 = 256;

/// The built-in plane (±32 in X and Z, normal +Y) scaled by `sx`, `sz` and moved by `t`.
fn card(id: u32, m: MaterialState, sx: f32, sz: f32, t: [f32; 3]) -> ParsedShape {
    ParsedShape {
        id: ShapeId(id),
        name: format!("card{id}").into(),
        mesh: std::sync::Arc::new(render3d::builtin_mesh(render3d::Builtin::Plane)),
        world: [
            [sx, 0.0, 0.0, t[0]],
            [0.0, 1.0, 0.0, t[1]],
            [0.0, 0.0, sz, t[2]],
        ],
        material: m,
    }
}

/// Opaque, depth test and write on (`m.blend()` false), of `shader_type`.
fn opaque(shader_type: u32) -> MaterialState {
    let mut m = MaterialState::builtin_default();
    m.shader_type = shader_type;
    m.slsf1 |= 1 << 31; // ZBuffer_Test
    m.slsf2 |= 1; // ZBuffer_Write
    m
}

/// A CS hair card (shader type 6 → HAIR; the bundled settings compile CS_HAIR) with a brown tint: x ±9.6, z ±3.2
/// at y = 0, facing the camera.
fn hair() -> ParsedShape {
    let mut m = opaque(6);
    m.params.trailer = LightingTrailer::HairTint([0.6, 0.45, 0.3]);
    card(0, m, 0.3, 0.1, [0.0; 3])
}

/// A plain opaque card 2 units in front of the hair (toward the camera and the light) over x < 0 only: inside the
/// raymarch's reach (SelfShadowScale 2.5 × GAME_UNIT_TO_CM 1.428 = 3.57 units, Hair.hlsli:275).
fn occluder() -> ParsedShape {
    card(1, opaque(0), 0.3, 0.15, [-9.6, 2.0, 0.0])
}

/// Camera on the +Y side looking at the origin (screen right = world −X), the sun on the camera side and toward
/// −X, dim ambient so the direct light dominates.
fn scene() -> Scene {
    let l: [f32; 3] = [-1.0, 0.4, 0.0];
    let n = (l[0] * l[0] + l[1] * l[1]).sqrt();
    Scene {
        camera: Camera::Orbit {
            target: [0.0; 3],
            yaw: std::f32::consts::PI,
            pitch: 0.0,
            distance: 16.0,
        },
        dir_light: DirLight {
            direction: [l[0] / n, l[1] / n, 0.0],
            color: [2.0; 3],
        },
        ambient6: [[0.05; 3]; 6],
        ..Scene::neutral()
    }
}

/// The bundled CS, forward or deferred (the two targets' depths are copied the same way), with `shapes` loaded.
fn renderer(cache: &std::path::Path, deferred: bool, shapes: Vec<ParsedShape>) -> Renderer {
    let mut r =
        Renderer::with_source(render3d::ShaderSource::Bundled, cache.to_path_buf()).unwrap();
    r.set_deferred(deferred).unwrap();
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
    r
}

/// One frame with `hairSpecularSettings.EnableSelfShadow` = `on` (a cbuffer edit of the bundled settings).
fn frame(r: &mut Renderer, on: bool) -> Vec<u8> {
    let mut s = render3d::cs_settings::defaults().clone();
    s.merged["Hair Specular"]["EnableSelfShadow"] = json!(u8::from(on));
    r.set_settings(s);
    r.render(&scene(), W, H).unwrap()
}

/// Mean over the hair pixels (`mask`) of the RGB sum, and of |on − off| per channel, both in 1/255 steps.
fn stats(on: &[u8], off: &[u8], mask: &[usize]) -> (f64, f64) {
    let n = mask.len().max(1) as f64;
    let rgb = |px: &[u8], i: usize| (0..3).map(|c| f64::from(px[i * 4 + c])).sum::<f64>();
    let drop = mask.iter().map(|&i| rgb(off, i) - rgb(on, i)).sum::<f64>() / n / 3.0;
    let diff = mask
        .iter()
        .map(|&i| {
            (0..3)
                .map(|c| f64::from(on[i * 4 + c].abs_diff(off[i * 4 + c])))
                .sum::<f64>()
        })
        .sum::<f64>()
        / n
        / 3.0;
    (drop, diff)
}

/// Pixels of the self-shadow-off frame brighter than the black clear, in `cols`.
fn lit(off: &[u8], cols: std::ops::Range<u32>) -> Vec<usize> {
    (0..H)
        .flat_map(|y| cols.clone().map(move |x| (y * W + x) as usize))
        .filter(|&i| {
            off[i * 4..i * 4 + 3]
                .iter()
                .map(|&c| u32::from(c))
                .sum::<u32>()
                > 6
        })
        .collect()
}

/// A lone hair card with nothing between it and the light is not self-shadowed. The light is on the camera side,
/// so each raymarch step moves toward the camera and lands in front of the card's own depth; with the light behind
/// the card its own neighbouring pixels would block the ray, which is game-correct and would fail this test. Before
/// Note GX t17 was a black default (raw depth 0 = the near plane), every step counted as blocked and the hair got
/// ambient light only.
#[test]
fn render_hair_with_nothing_toward_the_light_is_not_self_shadowed() {
    for deferred in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let mut r = renderer(cache.path(), deferred, vec![hair()]);
        let off = frame(&mut r, false);
        let on = frame(&mut r, true);
        let mask = lit(&off, 0..W);
        assert!(
            mask.len() > 10_000,
            "the card fills the frame: {}",
            mask.len()
        );
        let (drop, diff) = stats(&on, &off, &mask);
        println!(
            "deferred {deferred}: hair px {} drop {drop:.3} mean |diff| {diff:.3}",
            mask.len()
        );
        assert!(
            diff <= 1.0,
            "deferred {deferred}: mean |on - off| {diff:.3} / 255"
        );
    }
}

/// An opaque card between the hair and the light, inside the raymarch's reach, darkens the hair next to it with
/// self-shadow on; the hair beyond the reach is unchanged. A white (far) t17 would shadow nothing and a black one
/// everything, so this shows the real scene depth is bound.
#[test]
fn render_a_card_toward_the_light_shadows_the_hair() {
    for deferred in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let mut r = renderer(cache.path(), deferred, vec![hair(), occluder()]);
        let off = frame(&mut r, false);
        let on = frame(&mut r, true);
        assert_eq!(
            r.last_prepass_draws(),
            2,
            "both opaque cards are in the pre-pass"
        );
        let targets = if deferred { 10 } else { 2 };
        assert_eq!(r.target_dims().len(), targets, "deferred {deferred}");
        // The occluder covers the right half; 17.2 px per unit on the hair plane (fov 50°, 256 px, 16 units away).
        let near = lit(&off, W / 2 - 18..W / 2 - 2); // hair 0.1-1.0 units from the occluder's edge
        let far = lit(&off, 0..W / 2 - 85); // more than 4.9 units: beyond the 4.5-unit reach with the jitter
        assert!(
            near.len() > 1000 && far.len() > 1000,
            "{} {}",
            near.len(),
            far.len()
        );
        let (near_drop, near_diff) = stats(&on, &off, &near);
        let (far_drop, far_diff) = stats(&on, &off, &far);
        println!(
            "deferred {deferred}: near px {} drop {near_drop:.3} |diff| {near_diff:.3}",
            near.len()
        );
        println!(
            "deferred {deferred}: far px {} drop {far_drop:.3} |diff| {far_diff:.3}",
            far.len()
        );
        assert!(
            near_drop > 8.0,
            "deferred {deferred}: the hair next to the card darkens: {near_drop:.3} / 255"
        );
        assert!(
            far_diff <= 1.0,
            "deferred {deferred}: the hair out of reach is unchanged: {far_diff:.3} / 255"
        );
    }
}

/// A frame whose shapes do not read t17 draws no pre-pass and copies nothing (Note GX1), so it renders as before.
#[test]
fn render_a_frame_without_a_depth_reader_draws_no_prepass() {
    let cache = tempfile::tempdir().unwrap();
    let mut blended = opaque(0);
    blended.alpha = Some(nif::props::Alpha {
        flags: 0x00ED, // blend on, SrcAlpha / InvSrcAlpha
        threshold: 0,
    });
    let glass = card(2, blended, 0.1, 0.1, [4.0, 3.0, 0.0]);
    let mut r = renderer(cache.path(), false, vec![occluder(), glass]);
    r.render(&scene(), W, H).unwrap();
    assert_eq!(r.last_prepass_draws(), 0);
    let mut r = renderer(cache.path(), false, vec![hair(), occluder()]);
    r.render(&scene(), W, H).unwrap();
    assert_eq!(r.last_prepass_draws(), 2, "a hair card turns it on");
}
