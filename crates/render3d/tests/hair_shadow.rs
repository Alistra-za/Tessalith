//! Note GX: CS hair self-shadow (`Hair::HairSelfShadow`, Hair.hlsli:264-299) raymarches the scene depth at t17. The
//! preview draws a depth pre-pass and binds its copy there; `render_*` on the bundled CS, run alone.
use nif::material::MaterialState;
use nif::props::LightingTrailer;
use render3d::device::Gpu;
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

/// A plain opaque card 2 units in front of the hair (toward the camera) over x < 0 only. The raymarch steps toward
/// the light in screen space and compares each step with t17: from hair near the card's edge the steps pass behind
/// the card's projection inside the reach (SelfShadowScale 2.5 × GAME_UNIT_TO_CM 1.428 = 3.57 units, Hair.hlsli:275)
/// and count as blocked. The world-space light ray from there meets y = 2 only after about 5.4 units, so the shadow
/// is the screen-space test's, as in game; moving the card to put y = 2 inside the reach is not needed.
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

/// The bundled CS on the default GPU, forward or deferred (the two targets' depths are copied the same way), with
/// `shapes` loaded.
fn renderer(cache: &std::path::Path, deferred: bool, shapes: Vec<ParsedShape>) -> Renderer {
    renderer_on(Gpu::new().unwrap(), cache, deferred, shapes)
}

fn renderer_on(
    gpu: Gpu,
    cache: &std::path::Path,
    deferred: bool,
    shapes: Vec<ParsedShape>,
) -> Renderer {
    let mut r =
        Renderer::with_gpu_source(gpu, render3d::ShaderSource::Bundled, cache.to_path_buf())
            .unwrap();
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

/// Every Lighting.hlsl pixel program the preview builds (the only shape shader it compiles), for every NIF shader
/// type, forward and DEFERRED, plain and with the flag-driven defines on, under the default CS features: only CS
/// hair without TRUE_PBR (Hair.hlsli:289, `CS_HAIR_SHADING`) reflects the scene depth at t17 (Note GX1). Compiled
/// with fxc only, no device.
#[test]
fn only_vanilla_hair_reads_the_scene_depth() {
    use render3d::permutation::{defines, Material, Stage};
    let features = render3d::cs_settings::preview_defines(render3d::cs_settings::defaults(), true);
    let features: Vec<&str> = features.iter().map(String::as_str).collect();
    let plain = |shader_type| Material {
        shader_type,
        slsf1: 0,
        slsf2: 0,
        skinned: false,
        alpha_test: false,
        has_vertex_colors: false,
        specular_strength: 1.0,
        projected: false,
        projected_glint: false,
        wetness: false,
    };
    // (name, material, TRUE_PBR)
    let variants = |st: u32| {
        let all = Material {
            // Specular, Decal, Dynamic_Decal; aniso, soft, rim, back lighting
            slsf1: 1 | (1 << 26) | (1 << 27),
            slsf2: (1 << 21) | (1 << 25) | (1 << 26) | (1 << 27),
            skinned: true,
            alpha_test: true,
            has_vertex_colors: true,
            wetness: true,
            ..plain(st)
        };
        let msn = Material {
            slsf1: 1 << 12,
            skinned: true,
            ..plain(st)
        };
        let projected = Material {
            slsf1: 1 << 23,
            has_vertex_colors: true,
            projected: true,
            ..plain(st)
        };
        let pbr = Material {
            slsf2: (1 << 23) | (1 << 21),
            ..all
        };
        let pbr_projected = Material {
            slsf2: 1 << 23,
            projected_glint: true,
            ..projected
        };
        [
            ("plain", plain(st), false),
            ("flags", all, false),
            ("msn", msn, false),
            ("projected", projected, false),
            ("pbr", pbr, true),
            ("pbr projected", pbr_projected, true),
        ]
    };
    let mut jobs = std::collections::BTreeMap::new();
    for st in 0..20 {
        for (name, m, pbr) in variants(st) {
            for deferred in [false, true] {
                let d = defines(&m, Stage::Pixel, &features, deferred);
                jobs.entry(format!("{:?}", d.0))
                    .or_insert((d, Vec::new()))
                    .1
                    .push((st, name, pbr, deferred));
            }
        }
    }
    let jobs: Vec<_> = jobs.into_values().collect();
    let src = render3d::source::SourceSet::new(render3d::ShaderSource::Bundled)
        .get(std::path::Path::new("Lighting.hlsl"))
        .unwrap();
    let cache = tempfile::tempdir().unwrap();
    let next = std::sync::atomic::AtomicUsize::new(0);
    // None: fxc refuses the permutation (the preview reports a compile error and draws nothing for it).
    let reads: Vec<(usize, Option<bool>)> = std::thread::scope(|sc| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                sc.spawn(|| {
                    let mut out = vec![];
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((d, _)) = jobs.get(i) else {
                            return out;
                        };
                        let c = render3d::compile::compile(&src, d, "ps_5_0", cache.path());
                        out.push((
                            i,
                            c.ok().map(|c| render3d::draw::reads_depth(&c.reflection)),
                        ));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    });
    assert_eq!(reads.len(), jobs.len());
    let (mut readers, mut refused) = (vec![], vec![]);
    for (i, r) in reads {
        for &(st, name, pbr, deferred) in &jobs[i].1 {
            let at = format!("type {st} {name} deferred {deferred}");
            match r {
                Some(true) => readers.push(at.clone()),
                Some(false) => {}
                None => refused.push(at.clone()),
            }
            if let Some(r) = r {
                assert_eq!(r, st == 6 && !pbr, "{at}: reads t17 {r}");
            }
        }
    }
    readers.sort();
    refused.sort();
    println!(
        "{} programs; t17 readers: {readers:#?}\nrefused by fxc: {refused:#?}",
        jobs.len()
    );
    // The hair rows must have compiled, or the assert above proved nothing for them.
    assert!(readers.iter().any(|r| r.starts_with("type 6 plain")) && readers.len() >= 6);
}

/// No t17 reader: a plain card where the hair would be, the occluder overlapping it in front (so the depth test
/// decides pixels) and a blended card over both.
fn no_reader_shapes() -> Vec<ParsedShape> {
    let mut blended = opaque(0);
    blended.alpha = Some(nif::props::Alpha {
        flags: 0x00ED, // blend on, SrcAlpha / InvSrcAlpha
        threshold: 0,
    });
    vec![
        card(0, opaque(0), 0.3, 0.1, [0.0; 3]),
        occluder(),
        card(2, blended, 0.1, 0.1, [0.0, 3.0, 0.0]),
    ]
}

/// SHA-256 of the 128 × 128 RGBA8 readback of [`no_reader_shapes`] on WARP, captured on base f9639a84 (before
/// Note GX), forward and deferred. WARP, not the default GPU, so the bytes do not depend on the adapter or driver;
/// a Windows update that changes WARP's rasteriser changes them too, so re-capture on that base before re-pinning.
const NO_READER_SHA256: [(bool, &str); 2] = [
    (
        false,
        "d3cddc8212422ef19291924c8ce235ac0add7e70d8d5076d33629627befdf544",
    ),
    (
        true,
        "7d1cef7a9cb334306595af2940cb3c6c389de1ce6bc998c08285ab19889c547e",
    ),
];

/// Two pins for a frame with no t17 reader (Note GX1): its pixels are byte-identical to base f9639a84, forward and
/// deferred, with both targets' depth R32_TYPELESS (GX2); and it draws no pre-pass and its target holds no copy.
/// The last asserts show the hooks can fail: adding a hair card turns the pre-pass on and makes the copy.
#[test]
fn render_a_frame_without_a_depth_reader_is_unchanged_and_draws_no_prepass() {
    use sha2::Digest;
    for (deferred, want) in NO_READER_SHA256 {
        let cache = tempfile::tempdir().unwrap();
        let gpu = Gpu::new_warp().unwrap();
        let mut r = renderer_on(gpu, cache.path(), deferred, no_reader_shapes());
        let px = r.render(&scene(), 128, 128).unwrap();
        let shown = px
            .chunks(4)
            .filter(|p| p[..3].iter().map(|&c| u32::from(c)).sum::<u32>() > 6)
            .count();
        println!("deferred {deferred}: {shown} lit px");
        assert!(
            shown > 4000,
            "deferred {deferred}: the cards are drawn: {shown}"
        );
        let got = hex::encode(sha2::Sha256::digest(&px));
        assert_eq!(
            got, want,
            "deferred {deferred}: the frame's bytes changed. The pin depends on WARP's rasteriser and the system \
             d3dcompiler, so a Windows update can move it. To re-take it, check out the base commit (f9639a84) in a \
             temporary detached worktree, copy this test there without the last_prepass_draws and has_depth_copy \
             asserts, run it and pin the hashes its assert reports; never re-pin from this branch"
        );
        assert_eq!(r.last_prepass_draws(), 0, "deferred {deferred}");
        assert!(
            !r.has_depth_copy(),
            "deferred {deferred}: no reader, no copy"
        );
    }
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path(), false, vec![hair(), occluder()]);
    r.render(&scene(), W, H).unwrap();
    assert_eq!(r.last_prepass_draws(), 2, "a hair card turns it on");
    assert!(
        r.has_depth_copy(),
        "the first pre-pass frame makes the copy"
    );
}
