//! Note IH5: the Gradient background and its shadow-catching floor, on the bundled CS on WARP, forward and deferred
//! (the GUI default is deferred). The built-in sphere (radius 32 at the origin) hovers over a pebble (the same sphere
//! at 1 %) 40 units below it: the pebble is the model's lowest point, so the floor lies under the sphere. The sun
//! straight down unless a test says otherwise; the maps are 512 / 256 through `set_shadow_sizes`. A GPU test file: run
//! it alone.
mod common;
use common::test_env;
use nif::material::MaterialState;
use render3d::background::GRADIENT_FLOOR_SRGB;
use render3d::device::Gpu;
use render3d::scene::{Background, Camera, DirLight, Scene, View};
use render3d::{Builtin, ParsedNif, ParsedShape, Renderer, ShapeId};

const W: u32 = 256;
const H: u32 = 256;

/// The floor's height: the pebble's lowest point (centre -40, radius 0.32).
const FLOOR_Z: f32 = -40.32;

fn opaque() -> MaterialState {
    let mut m = MaterialState::builtin_default();
    m.slsf1 |= 1 << 31; // ZBuffer_Test
    m.slsf2 |= 1; // ZBuffer_Write
    m
}

/// The built-in sphere scaled by `scale` and moved to `z` on the Z axis.
fn ball(id: u32, scale: f32, z: f32) -> ParsedShape {
    ParsedShape {
        id: ShapeId(id),
        name: format!("ball{id}").into(),
        mesh: std::sync::Arc::new(render3d::builtin_mesh(Builtin::Sphere)),
        world: [
            [scale, 0.0, 0.0, 0.0],
            [0.0, scale, 0.0, 0.0],
            [0.0, 0.0, scale, z],
        ],
        material: opaque(),
    }
}

fn sphere_over_floor() -> Vec<ParsedShape> {
    vec![ball(0, 1.0, 0.0), ball(1, 0.01, -40.0)]
}

/// The bundled CS on WARP with `shapes` loaded, forward or deferred, maps at 512 / 256.
fn renderer(cache: &std::path::Path, deferred: bool, shapes: Vec<ParsedShape>) -> Renderer {
    let mut r = Renderer::with_gpu_source(
        Gpu::new_warp().unwrap(),
        render3d::ShaderSource::Bundled,
        cache.to_path_buf(),
    )
    .unwrap();
    r.set_deferred(deferred).unwrap();
    r.set_shadow_sizes(512, 256);
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

/// The camera on the −Y side at `pitch` above the horizon, 300 units from the origin; the sun straight down at 0.6,
/// dim ambient, so the lit floor does not clip and its shadow is far darker.
fn scene(pitch: f32, shadows: bool) -> Scene {
    Scene {
        camera: Camera::Orbit {
            target: [0.0; 3],
            yaw: 0.0,
            pitch,
            distance: 300.0,
        },
        background: Background::Gradient,
        dir_light: DirLight {
            direction: [0.0, 0.0, 1.0],
            color: [0.6; 3],
        },
        ambient6: [[0.03; 3]; 6],
        shadows,
        ..Scene::neutral()
    }
}

/// The pixel world point `p` lands on in a `w × h` frame of `s`.
fn px(r: &Renderer, s: &Scene, p: [f32; 3]) -> (u32, u32) {
    let v = View::new(&s.camera, r.bounds(), W, H);
    let c = [p[0] - v.eye[0], p[1] - v.eye[1], p[2] - v.eye[2], 1.0];
    let clip: Vec<f32> = (0..4)
        .map(|k| (0..4).map(|j| v.view_proj[k][j] * c[j]).sum())
        .collect();
    let (x, y) = (clip[0] / clip[3], clip[1] / clip[3]);
    (
        ((x + 1.0) * 0.5 * W as f32) as u32,
        ((1.0 - y) * 0.5 * H as f32) as u32,
    )
}

fn rgb(frame: &[u8], (x, y): (u32, u32)) -> [u8; 3] {
    let i = ((y * W + x) * 4) as usize;
    [frame[i], frame[i + 1], frame[i + 2]]
}

fn lum(frame: &[u8], at: (u32, u32)) -> f64 {
    let [r, g, b] = rgb(frame, at).map(f64::from);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn srgb8(c: f32) -> f32 {
    render3d::scene::linear_to_srgb(c) * 255.0
}

#[test]
fn gradient_is_warm_grey_at_the_bottom_and_black_from_the_middle_up() {
    for deferred in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let mut r = renderer(cache.path(), deferred, vec![]);
        let f = r.render(&scene(0.5, true), W, H).unwrap();
        let [pr, pg, pb] = rgb(&f, (W / 2, H - 1));
        println!("deferred {deferred}: bottom row {pr} {pg} {pb}");
        for (got, want) in [(pr, 107.0), (pg, 102.0), (pb, 99.0)] {
            assert!(
                (f32::from(got) - want).abs() <= 3.0,
                "deferred {deferred}: bottom {pr} {pg} {pb}"
            );
        }
        assert!(pr > pg && pg >= pb, "warm: {pr} {pg} {pb}");
        // The constant the bottom row is checked against is the floor colour.
        assert!(
            (srgb8(render3d::scene::srgb_to_linear(GRADIENT_FLOOR_SRGB[0])) - 107.0).abs() < 1.0
        );
        for y in 0..H / 2 {
            for x in 0..W {
                assert_eq!(
                    rgb(&f, (x, y)),
                    [0, 0, 0],
                    "deferred {deferred}: ({x}, {y}) is black"
                );
            }
        }
    }
}

#[test]
fn the_floor_catches_the_sun_shadow() {
    for deferred in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let mut r = renderer(cache.path(), deferred, sphere_over_floor());
        let s = scene(0.5, true);
        let f = r.render(&s, W, H).unwrap();
        // Under the sphere, in the sun's shadow disc (radius 32) and in front of the sphere; beside it, outside the disc
        // and inside the floor's solid part.
        let under = px(&r, &s, [0.0, -20.0, FLOOR_Z]);
        let beside = px(&r, &s, [60.0, -20.0, FLOOR_Z]);
        let (l_under, l_beside) = (lum(&f, under), lum(&f, beside));
        println!(
            "deferred {deferred}: under {under:?} {l_under:.1}, beside {beside:?} {l_beside:.1}"
        );
        assert!(
            l_beside > 20.0,
            "deferred {deferred}: the lit floor: {l_beside:.1}"
        );
        assert!(
            l_under < 0.6 * l_beside,
            "deferred {deferred}: shadow {l_under:.1} vs lit {l_beside:.1}"
        );
        // The same frame without shadows lights the floor under the sphere as the lit floor beside it.
        let off = r.render(&scene(0.5, false), W, H).unwrap();
        assert!(
            lum(&off, under) > 0.9 * lum(&off, beside),
            "deferred {deferred}: no shadows, no shade"
        );
    }
}

#[test]
fn the_floor_has_no_edge() {
    for deferred in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let mut r = renderer(cache.path(), deferred, sphere_over_floor());
        // A bright sun, so the floor is far lighter than the backdrop behind its rim; far enough back that a row of
        // the frame spans the whole quad (half-size about 145).
        let s = Scene {
            camera: Camera::Orbit {
                target: [0.0; 3],
                yaw: 0.0,
                pitch: 0.5,
                distance: 600.0,
            },
            dir_light: DirLight {
                direction: [0.0, 0.0, 1.0],
                color: [2.0; 3],
            },
            ..scene(0.5, true)
        };
        let f = r.render(&s, W, H).unwrap();
        let c = tempfile::tempdir().unwrap();
        let backdrop = renderer(c.path(), deferred, vec![])
            .render(&s, W, H)
            .unwrap();
        // A row in front of the sphere, from one side of the frame to the other, crosses the floor's rim twice.
        let (_, y) = px(&r, &s, [0.0, -90.0, FLOOR_Z]);
        assert!(
            lum(&f, (W / 2, y)) > lum(&backdrop, (W / 2, y)) + 15.0,
            "deferred {deferred}: the floor is there: {} vs {}",
            lum(&f, (W / 2, y)),
            lum(&backdrop, (W / 2, y))
        );
        // Both ends of the row are past the rim: the backdrop, untouched.
        for x in [0, W - 1] {
            assert_eq!(
                rgb(&f, (x, y)),
                rgb(&backdrop, (x, y)),
                "deferred {deferred}: the row's end {x} is past the floor"
            );
        }
        let mut worst = 0u8;
        let mut at = 0;
        for x in 1..W {
            let (a, b) = (rgb(&f, (x - 1, y)), rgb(&f, (x, y)));
            let d = (0..3).map(|k| a[k].abs_diff(b[k])).max().unwrap();
            if d > worst {
                (worst, at) = (d, x);
            }
        }
        println!("deferred {deferred}: row {y}, largest step {worst} at x {at}");
        assert!(
            worst <= 6,
            "deferred {deferred}: a step of {worst} at x {at}"
        );
    }
}

/// The floor draws before the blended shapes: a blended sphere (SrcAlpha / One, no depth write) is added over the
/// floor behind it, not covered by it.
#[test]
fn blended_shapes_draw_over_the_floor() {
    for deferred in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let mut glass = ball(0, 1.0, 0.0);
        glass.material.alpha = Some(nif::props::Alpha {
            flags: 0x1001,
            threshold: 0,
        });
        glass.material.slsf2 &= !1; // no ZBuffer_Write
        let mut r = renderer(cache.path(), deferred, vec![glass, ball(1, 0.01, -40.0)]);
        let s = scene(0.5, true);
        let f = r.render(&s, W, H).unwrap();
        // The sphere's centre pixel has the floor behind it, lit as the floor beside the sphere is.
        let centre = px(&r, &s, [0.0, 0.0, 0.0]);
        let beside = px(&r, &s, [60.0, -20.0, FLOOR_Z]);
        println!(
            "deferred {deferred}: sphere {:.1}, floor {:.1}",
            lum(&f, centre),
            lum(&f, beside)
        );
        assert!(
            lum(&f, centre) > lum(&f, beside) + 10.0,
            "deferred {deferred}: the blended sphere is over the floor"
        );
    }
}

/// The built-in plane (±32 in X and Z, normal +Y) turned so its normal is +Z, scaled 3×, at height `z`.
fn flat(id: u32, z: f32) -> ParsedShape {
    ParsedShape {
        id: ShapeId(id),
        name: format!("flat{id}").into(),
        mesh: std::sync::Arc::new(render3d::builtin_mesh(Builtin::Plane)),
        world: [
            [3.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, -3.0, 0.0],
            [0.0, 3.0, 0.0, z],
        ],
        material: opaque(),
    }
}

/// Final review: the floor never paints over the model. A flat mesh lying at the model's lowest Z shares the floor's
/// depth, and an opaque sphere that writes no depth leaves none for the floor to fail against; in both, forward and
/// deferred, every model pixel is what the same frame draws without the floor (its creation made to fail, so the
/// sun fit and everything else stay).
#[test]
fn the_floor_never_covers_the_model() {
    type Model = fn() -> Vec<ParsedShape>;
    let cases: [(&str, Model); 2] = [
        ("a flat mesh at the lowest Z", || {
            vec![ball(0, 1.0, 0.0), flat(1, -40.0)]
        }),
        ("an opaque sphere writing no depth", || {
            let mut s = ball(0, 1.0, 0.0);
            s.material.slsf2 &= !1; // ZBuffer_Write clear
            vec![s, ball(1, 0.01, -40.0)]
        }),
    ];
    let mut covering = vec![];
    for (what, shapes) in cases {
        for deferred in [false, true] {
            let (c1, c2, c3) = (
                tempfile::tempdir().unwrap(),
                tempfile::tempdir().unwrap(),
                tempfile::tempdir().unwrap(),
            );
            let s = scene(0.5, true);
            let mut with = renderer(c1.path(), deferred, shapes());
            let mut without = renderer(c2.path(), deferred, shapes());
            without.set_fault(Some((render3d::Fault::Floor, "no floor")));
            let (a, b) = (
                with.render(&s, W, H).unwrap(),
                without.render(&s, W, H).unwrap(),
            );
            assert_eq!(without.backdrop_error(), Some("no floor"));
            let empty = renderer(c3.path(), deferred, vec![])
                .render(&s, W, H)
                .unwrap();
            // The model's pixels: where the frame without the floor is not the bare background.
            let model: Vec<usize> = (0..(W * H) as usize)
                .filter(|&i| b[i * 4..i * 4 + 3] != empty[i * 4..i * 4 + 3])
                .collect();
            assert!(model.len() > 2000, "{what}: {} model pixels", model.len());
            let covered = model
                .iter()
                .filter(|&&i| a[i * 4..i * 4 + 3] != b[i * 4..i * 4 + 3])
                .count();
            println!(
                "{what}, deferred {deferred}: {covered} of {} model pixels changed",
                model.len()
            );
            if covered > 0 {
                covering.push(format!("{what}, deferred {deferred}: {covered}"));
            }
            // The floor is drawn: it shows beside the model.
            assert!(
                (0..(W * H) as usize).any(|i| a[i * 4..i * 4 + 3] != b[i * 4..i * 4 + 3]),
                "{what}, deferred {deferred}: the floor is drawn"
            );
        }
    }
    assert!(
        covering.is_empty(),
        "the floor covers model pixels: {covering:?}"
    );
}

/// Final review: a shape revealed after the load (hidden in the NIF, shown through `set_material`) counts for the
/// floor and the sun's fit: the floor moves down to its lowest point, and under Solid (where the fit is the model's
/// sphere alone) the fit grows to hold it. The camera's framing (`bounds`) stays.
#[test]
fn a_revealed_lower_shape_moves_the_floor_and_the_sun_fit() {
    let mut low = ball(2, 0.01, -80.0);
    low.material.hidden = true;
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(
        cache.path(),
        false,
        vec![ball(0, 1.0, 0.0), ball(1, 0.01, -40.0), low],
    );
    let gradient = scene(0.5, true);
    let solid = Scene {
        background: Background::Solid([0.0; 3]),
        ..scene(0.5, true)
    };
    let low_point = [0.0, 0.0, -80.32];
    let holds = |fit: ([f32; 3], f32)| {
        let d = [0, 1, 2].map(|k| fit.0[k] - low_point[k]);
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() <= fit.1
    };
    r.render(&gradient, W, H).unwrap();
    let floor = r.last_floor().expect("a floor");
    assert!((floor.centre[2] - FLOOR_Z).abs() < 1e-3, "{floor:?}");
    r.render(&solid, W, H).unwrap();
    assert!(
        !holds(r.last_sun_fit().unwrap()),
        "the hidden shape is outside"
    );
    let framing = r.bounds();
    assert_eq!(
        r.set_material(ShapeId(2), opaque(), &|_| None).map(|_| ()),
        Ok(())
    );
    r.render(&gradient, W, H).unwrap();
    let floor = r.last_floor().expect("a floor");
    assert!((floor.centre[2] - low_point[2]).abs() < 1e-3, "{floor:?}");
    r.render(&solid, W, H).unwrap();
    assert!(
        holds(r.last_sun_fit().unwrap()),
        "the fit holds the revealed shape"
    );
    assert_eq!(r.bounds(), framing, "the camera's framing stays");
}

/// Review Focus 5: from below the floor plane nothing of the floor is drawn: every pixel the sphere does not cover is
/// the backdrop (the frame of nothing loaded), and the sphere shows.
#[test]
fn floor_is_hidden_from_below() {
    for deferred in [false, true] {
        let (c1, c2) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let mut empty = renderer(c1.path(), deferred, vec![]);
        let mut r = renderer(c2.path(), deferred, sphere_over_floor());
        let s = scene(-0.5, true);
        let backdrop = empty.render(&s, W, H).unwrap();
        let f = r.render(&s, W, H).unwrap();
        let centre = px(&r, &s, [0.0, 0.0, 0.0]);
        assert!(
            rgb(&f, centre) != rgb(&backdrop, centre),
            "deferred {deferred}: the sphere is drawn"
        );
        let mut outside = 0;
        for y in 0..H {
            for x in 0..W {
                let (dx, dy) = (x as f32 - centre.0 as f32, y as f32 - centre.1 as f32);
                if (dx * dx + dy * dy).sqrt() > 40.0 {
                    outside += 1;
                    assert_eq!(
                        rgb(&f, (x, y)),
                        rgb(&backdrop, (x, y)),
                        "deferred {deferred}: ({x}, {y}) is the backdrop"
                    );
                }
            }
        }
        assert!(outside > 40_000);
    }
}

#[test]
fn solid_and_hdri_have_no_floor() {
    for deferred in [false, true] {
        let (c1, c2) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let mut empty = renderer(c1.path(), deferred, vec![]);
        let mut r = renderer(c2.path(), deferred, sphere_over_floor());
        for bg in [
            Background::Solid([0.0, 0.0, 1.0]),
            Background::Hdri(test_env()),
        ] {
            let s = Scene {
                background: bg,
                ..scene(0.5, true)
            };
            let backdrop = empty.render(&s, W, H).unwrap();
            let f = r.render(&s, W, H).unwrap();
            let centre = px(&r, &s, [0.0, 0.0, 0.0]);
            // Everywhere the sphere (radius 32 at distance 300) is not, the frame is the empty frame: no floor.
            let mut outside = 0;
            for y in 0..H {
                for x in 0..W {
                    let (dx, dy) = (x as f32 - centre.0 as f32, y as f32 - centre.1 as f32);
                    if (dx * dx + dy * dy).sqrt() > 40.0 {
                        outside += 1;
                        assert_eq!(
                            rgb(&f, (x, y)),
                            rgb(&backdrop, (x, y)),
                            "deferred {deferred}: ({x}, {y}) is the background"
                        );
                    }
                }
            }
            assert!(outside > 40_000);
        }
    }
}

/// The point light's cube shadows the floor too: lit by the point light alone from above the sphere, the floor under
/// the sphere is dark and the floor beside it is not.
#[test]
fn the_floor_catches_the_point_light_shadow() {
    for deferred in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let mut r = renderer(cache.path(), deferred, sphere_over_floor());
        let point = |shadows| Scene {
            sun_brightness: 0.0,
            point_light: render3d::scene::PointLight {
                brightness: 0.5,
                radius: 400.0,
                position: [0.0, 0.0, 80.0],
            },
            ..scene(0.5, shadows)
        };
        let s = point(true);
        let f = r.render(&s, W, H).unwrap();
        assert!(r.has_point_shadow_map(), "deferred {deferred}");
        let (under, beside) = (
            px(&r, &s, [0.0, -20.0, FLOOR_Z]),
            px(&r, &s, [90.0, -20.0, FLOOR_Z]),
        );
        let (l_under, l_beside) = (lum(&f, under), lum(&f, beside));
        println!("deferred {deferred}: under {l_under:.1}, beside {l_beside:.1}");
        assert!(
            l_beside > 20.0,
            "deferred {deferred}: the lit floor: {l_beside:.1}"
        );
        assert!(
            l_under < 0.6 * l_beside,
            "deferred {deferred}: shadow {l_under:.1} vs lit {l_beside:.1}"
        );
    }
}

/// The default sun is slanted (`Scene::neutral()`'s direction): the sphere's shadow falls on the floor away from the
/// sun, past the sphere, and the side towards the sun stays lit.
#[test]
fn the_default_slanted_sun_shadow_lands_on_the_floor() {
    let dir = Scene::neutral().dir_light.direction;
    for deferred in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let mut r = renderer(cache.path(), deferred, sphere_over_floor());
        let slanted = |shadows| Scene {
            dir_light: DirLight {
                direction: dir,
                color: [0.6; 3],
            },
            ..scene(0.5, shadows)
        };
        let (on_s, off_s) = (slanted(true), slanted(false));
        let on = r.render(&on_s, W, H).unwrap();
        let off = r.render(&off_s, W, H).unwrap();
        // The sphere's centre lands on the floor `height / dir.z` along the sun's rays: away from the sun.
        let height = 0.0 - FLOOR_Z;
        let away = [
            -dir[0] * height / dir[2],
            -dir[1] * height / dir[2],
            FLOOR_Z,
        ];
        let towards = [-away[0], -away[1], FLOOR_Z];
        let (a, t) = (px(&r, &on_s, away), px(&r, &on_s, towards));
        println!(
            "deferred {deferred}: away {a:?} {:.1} -> {:.1}, towards {t:?} {:.1} -> {:.1}",
            lum(&off, a),
            lum(&on, a),
            lum(&off, t),
            lum(&on, t)
        );
        assert!(lum(&off, a) > 20.0, "deferred {deferred}: the lit floor");
        assert!(
            lum(&on, a) < 0.6 * lum(&off, a),
            "deferred {deferred}: the shadow lands away from the sun"
        );
        // The darkest pixel the shadow made is near that point, not on the sun's side.
        let (mut best, mut at) = (f64::MAX, (0, 0));
        for y in 0..H {
            for x in 0..W {
                let r = lum(&on, (x, y)) / lum(&off, (x, y)).max(1.0);
                if lum(&off, (x, y)) > 20.0 && r < best {
                    (best, at) = (r, (x, y));
                }
            }
        }
        println!("deferred {deferred}: darkest ratio {best:.2} at {at:?}");
        assert!(best < 0.6);
        let (ax, ay) = (at.0 as f32 - a.0 as f32, at.1 as f32 - a.1 as f32);
        let (tx, ty) = (at.0 as f32 - t.0 as f32, at.1 as f32 - t.1 as f32);
        assert!(
            ax * ax + ay * ay < tx * tx + ty * ty,
            "deferred {deferred}: the darkest pixel {at:?} is on the sun's side"
        );
    }
}

/// The owner's look check (spec Decided 12): the Gradient with a built-in sphere and with the skinned fixture, the
/// preview's defaults (deferred, the default sun, shadows on), without and with the point light, as PNGs in the folder
/// `SDC_R3D_FRAME_OUT` names. Judge the colours by eye; nothing is asserted about the pixels.
#[test]
#[ignore = "writes the owner's look-check frames to SDC_R3D_FRAME_OUT"]
fn writes_look_check_frames() {
    let Some(out) = std::env::var_os("SDC_R3D_FRAME_OUT").map(std::path::PathBuf::from) else {
        eprintln!("skipped: set SDC_R3D_FRAME_OUT to the folder the PNGs go to");
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    let nif = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nif/tests/fixtures/TestNifFile_Skinned_SE.nif"
    ))
    .unwrap();
    let (w, h) = (768, 768);
    for (name, skinned) in [("sphere", false), ("skinned", true)] {
        let cache = tempfile::tempdir().unwrap();
        let mut r = Renderer::with_gpu_source(
            Gpu::new_warp().unwrap(),
            render3d::ShaderSource::Bundled,
            cache.path().to_path_buf(),
        )
        .unwrap();
        r.set_deferred(true).unwrap();
        let rep = if skinned {
            r.load(&nif, &|_| None)
        } else {
            r.load_builtin(Builtin::Sphere, opaque(), &|_| None)
        };
        assert!(
            rep.errors.is_empty() && rep.compile_errors.is_empty(),
            "{rep:?}"
        );
        let bounds = r.bounds().unwrap();
        for (point, brightness) in [("sun", 0.0), ("point", 1.0)] {
            let s = Scene {
                background: Background::Gradient,
                shadows: true,
                point_light: render3d::scene::PointLight::around(bounds, brightness, 2.0),
                ..Scene::neutral()
            };
            let f = r.render(&s, w, h).unwrap();
            let png = out.join(format!("gradient_{name}_{point}.png"));
            image::save_buffer(&png, &f, w, h, image::ExtendedColorType::Rgba8).unwrap();
            println!("saved {}", png.display());
        }
    }
}
