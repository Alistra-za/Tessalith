//! Note IH4: shadows from the sun and the point light through CS's own screen-space shadow mask (t14), on the
//! bundled CS on WARP. The built-in sphere (radius 32 at the origin) above the built-in plane turned to face +Z at
//! z = -40 and scaled to ±160; the sun straight down. The maps are 512 (sun) and 256 (cube face) through
//! `set_shadow_sizes`, so WARP stays fast. A GPU test file: run it alone.
use nif::material::MaterialState;
use render3d::device::Gpu;
use render3d::scene::{Background, Camera, DirLight, PointLight, Scene, View};
use render3d::{Builtin, ParsedNif, ParsedShape, Renderer, ShapeId};

const W: u32 = 256;
const H: u32 = 256;

/// A plane point under the sphere, seen past it from the camera (not hidden by it): inside the sun's shadow disc
/// (radius 32) and inside the point light's.
const UNDER: [f32; 3] = [0.0, -20.0, -40.0];

/// Opaque, depth test and write on.
fn opaque() -> MaterialState {
    let mut m = MaterialState::builtin_default();
    m.slsf1 |= 1 << 31; // ZBuffer_Test
    m.slsf2 |= 1; // ZBuffer_Write
    m
}

/// Alpha-blended (`0x1001`: blend on, One / One, which the engine draws as SrcAlpha / One), z-write as given.
fn blended(z_write: bool) -> MaterialState {
    let mut m = opaque();
    m.alpha = Some(nif::props::Alpha {
        flags: 0x1001,
        threshold: 0,
    });
    if !z_write {
        m.slsf2 &= !1;
    }
    m
}

fn sphere(id: u32, m: MaterialState) -> ParsedShape {
    ParsedShape {
        id: ShapeId(id),
        name: format!("sphere{id}").into(),
        mesh: std::sync::Arc::new(render3d::builtin_mesh(Builtin::Sphere)),
        world: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        material: m,
    }
}

/// The built-in plane (±32 in X and Z, normal +Y) turned so its normal is +Z, scaled 5×, at height `z`.
fn plane(id: u32, m: MaterialState, z: f32) -> ParsedShape {
    ParsedShape {
        id: ShapeId(id),
        name: format!("plane{id}").into(),
        mesh: std::sync::Arc::new(render3d::builtin_mesh(Builtin::Plane)),
        world: [
            [5.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, -5.0, 0.0],
            [0.0, 5.0, 0.0, z],
        ],
        material: m,
    }
}

fn floor_and_ball() -> Vec<ParsedShape> {
    vec![sphere(0, opaque()), plane(1, opaque(), -40.0)]
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

/// The camera on the −Y side, 0.5 rad above the plane's centre; the sun straight down at 0.6, dim ambient, so the
/// lit plane does not clip and its shadow is far darker.
fn scene(shadows: bool) -> Scene {
    Scene {
        camera: Camera::Orbit {
            target: [0.0; 3],
            yaw: 0.0,
            pitch: 0.5,
            distance: 300.0,
        },
        dir_light: DirLight {
            direction: [0.0, 0.0, 1.0],
            color: [0.6; 3],
        },
        ambient6: [[0.03; 3]; 6],
        shadows,
        ..Scene::neutral()
    }
}

/// [`scene`] lit by the point light alone at `at`, reaching 400 units.
fn point_scene(shadows: bool, at: [f32; 3]) -> Scene {
    Scene {
        sun_brightness: 0.0,
        point_light: PointLight {
            brightness: 0.5,
            radius: 400.0,
            position: at,
        },
        ..scene(shadows)
    }
}

/// The pixel world point `p` lands on in a `w × h` frame of `s` (the bounds the renderer frames with).
fn px(r: &Renderer, s: &Scene, w: u32, h: u32, p: [f32; 3]) -> (u32, u32) {
    let v = View::new(&s.camera, r.bounds(), w, h);
    let c = [p[0] - v.eye[0], p[1] - v.eye[1], p[2] - v.eye[2], 1.0];
    let clip: Vec<f32> = (0..4)
        .map(|k| (0..4).map(|j| v.view_proj[k][j] * c[j]).sum())
        .collect();
    let (x, y) = (clip[0] / clip[3], clip[1] / clip[3]);
    (
        ((x + 1.0) * 0.5 * w as f32) as u32,
        ((1.0 - y) * 0.5 * h as f32) as u32,
    )
}

fn rgb(frame: &[u8], w: u32, (x, y): (u32, u32)) -> [u8; 3] {
    let i = ((y * w + x) * 4) as usize;
    [frame[i], frame[i + 1], frame[i + 2]]
}

fn lum(frame: &[u8], w: u32, at: (u32, u32)) -> f64 {
    let [r, g, b] = rgb(frame, w, at).map(f64::from);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Every channel within `tol` steps.
fn close(a: [u8; 3], b: [u8; 3], tol: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tol)
}

/// The bottom-left frame corner: on the plane, far outside every shadow.
const CORNER: (u32, u32) = (2, H - 3);

/// The shadow under the sphere is much darker than the lit plane; the frame corner is unchanged. `what` names the case.
fn shadowed_under_and_corner_unchanged(r: &Renderer, s: &Scene, off: &[u8], on: &[u8], what: &str) {
    let under = px(r, s, W, H, UNDER);
    let (l_off, l_on) = (lum(off, W, under), lum(on, W, under));
    println!("{what}: under {under:?} off {l_off:.1} on {l_on:.1}");
    assert!(
        l_off > 20.0,
        "{what}: the plane under the sphere is lit: {l_off:.1}"
    );
    assert!(
        l_on < 0.6 * l_off,
        "{what}: the shadow darkens it: {l_on:.1} vs {l_off:.1}"
    );
    assert!(
        lum(off, W, CORNER) > 10.0,
        "{what}: the corner is the lit plane"
    );
    assert!(
        close(rgb(off, W, CORNER), rgb(on, W, CORNER), 1),
        "{what}: the corner is unchanged: {:?} vs {:?}",
        rgb(off, W, CORNER),
        rgb(on, W, CORNER)
    );
}

#[test]
fn the_sun_shadows_the_plane_under_the_sphere() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path(), false, floor_and_ball());
    let off = r.render(&scene(false), W, H).unwrap();
    let on = r.render(&scene(true), W, H).unwrap();
    shadowed_under_and_corner_unchanged(&r, &scene(true), &off, &on, "sun");
    // A cbuffer edit rewrites the plane's material: its shadow bits must come back with it.
    let mut m = opaque();
    m.params.uv_scale = [2.0, 2.0];
    assert_eq!(
        r.set_material(ShapeId(1), m, &|_| None),
        Ok(render3d::EditKind::Cbuffer)
    );
    let edited = r.render(&scene(true), W, H).unwrap();
    shadowed_under_and_corner_unchanged(&r, &scene(true), &off, &edited, "sun after an edit");
}

/// Off after on draws the bytes of a renderer that never had shadows on (point light on, so its shadow fields go back
/// too), and leaves no map behind.
#[test]
fn shadows_off_is_byte_identical_to_today() {
    let at = [0.0, 0.0, 80.0];
    let (ca, cb) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut a = renderer(ca.path(), false, floor_and_ball());
    a.render(&point_scene(true, at), W, H).unwrap();
    assert!(a.has_shadow_maps(), "a shadow frame makes the maps");
    let a_off = a.render(&point_scene(false, at), W, H).unwrap();
    assert!(!a.has_shadow_maps(), "an off frame drops them");
    let mut b = renderer(cb.path(), false, floor_and_ball());
    let b_off = b.render(&point_scene(false, at), W, H).unwrap();
    assert!(!b.has_shadow_maps());
    assert!(a_off == b_off, "off after on equals never on");
}

#[test]
fn the_point_light_shadows_too() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path(), false, floor_and_ball());
    let above = [0.0, 0.0, 80.0];
    let off = r.render(&point_scene(false, above), W, H).unwrap();
    let on = r.render(&point_scene(true, above), W, H).unwrap();
    assert!(r.has_point_shadow_map());
    shadowed_under_and_corner_unchanged(&r, &point_scene(true, above), &off, &on, "point");
    // Off-axis, so a mirrored cube face shows: from (60, 0, 80) the sphere's shadow falls around (-30, 0, -40). The
    // ray to (-35, -10, -40) passes 7 units from the centre, the ray to (35, -10, -40) 43 (outside the radius 32).
    let side = [60.0, 0.0, 80.0];
    let s = point_scene(true, side);
    let off = r.render(&point_scene(false, side), W, H).unwrap();
    let on = r.render(&s, W, H).unwrap();
    let (near, far) = (
        px(&r, &s, W, H, [-35.0, -10.0, -40.0]),
        px(&r, &s, W, H, [35.0, -10.0, -40.0]),
    );
    println!(
        "off-axis: shadowed {:.1} -> {:.1}, lit {:.1} -> {:.1}",
        lum(&off, W, near),
        lum(&on, W, near),
        lum(&off, W, far),
        lum(&on, W, far)
    );
    assert!(lum(&off, W, near) > 20.0 && lum(&off, W, far) > 20.0);
    assert!(
        lum(&on, W, near) < 0.6 * lum(&off, W, near),
        "the shadow side"
    );
    assert!(
        close(rgb(&off, W, far), rgb(&on, W, far), 1),
        "the mirrored side stays lit"
    );
}

/// Review Focus 2: a point light at brightness 0 makes no cube, and the frame is the sun-only frame.
#[test]
fn point_off_means_no_point_shadow() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path(), false, floor_and_ball());
    let sun_only = r.render(&scene(true), W, H).unwrap();
    assert!(r.has_shadow_maps() && !r.has_point_shadow_map());
    let mut dark = scene(true);
    dark.point_light = PointLight {
        brightness: 0.0,
        radius: 400.0,
        position: [0.0, 0.0, 80.0],
    };
    let frame = r.render(&dark, W, H).unwrap();
    assert!(!r.has_point_shadow_map(), "brightness 0: no cube");
    assert!(frame == sun_only, "the sun-only frame");
    let mut lit = dark.clone();
    lit.point_light.brightness = 0.5;
    r.render(&lit, W, H).unwrap();
    assert!(r.has_point_shadow_map(), "the light on makes the cube");
    let back = r.render(&dark, W, H).unwrap();
    assert!(!r.has_point_shadow_map(), "and off drops it");
    assert!(back == sun_only, "the sun-only frame again");
}

/// Review Focus 4: a blended sphere that does not write depth casts nothing.
#[test]
fn blended_shapes_cast_no_shadow() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(
        cache.path(),
        false,
        vec![sphere(0, blended(false)), plane(1, opaque(), -40.0)],
    );
    let off = r.render(&scene(false), W, H).unwrap();
    let on = r.render(&scene(true), W, H).unwrap();
    let under = px(&r, &scene(true), W, H, UNDER);
    assert!(lum(&off, W, under) > 20.0);
    assert!(
        close(rgb(&off, W, under), rgb(&on, W, under), 1),
        "{:?} vs {:?}",
        rgb(&off, W, under),
        rgb(&on, W, under)
    );
}

/// Review Focus 3: the mask is made again at the new size and the shadow still sits under the sphere.
#[test]
fn mask_follows_a_resize() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path(), false, floor_and_ball());
    for (w, h) in [(256, 256), (320, 200)] {
        let off = r.render(&scene(false), w, h).unwrap();
        let on = r.render(&scene(true), w, h).unwrap();
        let (mw, mh, mask) = r.read_shadow_mask().unwrap();
        assert_eq!((mw, mh), (w, h), "the mask is the frame's size");
        assert_eq!(mask.len(), (w * h * 4) as usize);
        let under = px(&r, &scene(true), w, h, UNDER);
        let (l_off, l_on) = (lum(&off, w, under), lum(&on, w, under));
        assert!(
            l_off > 20.0 && l_on < 0.6 * l_off,
            "{w}x{h}: {l_on:.1} vs {l_off:.1}"
        );
        assert!(
            mask[((under.1 * w + under.0) * 4) as usize] < 128,
            "{w}x{h}: the mask's sun channel is dark under the sphere"
        );
    }
}

#[test]
fn deferred_and_forward_shadow_alike() {
    let (ca, cb) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut f = renderer(ca.path(), false, floor_and_ball());
    let mut d = renderer(cb.path(), true, floor_and_ball());
    let off = d.render(&scene(false), W, H).unwrap();
    let (fwd, def) = (
        f.render(&scene(true), W, H).unwrap(),
        d.render(&scene(true), W, H).unwrap(),
    );
    let under = px(&f, &scene(true), W, H, UNDER);
    println!(
        "under: forward {:?} deferred {:?} deferred off {:?}",
        rgb(&fwd, W, under),
        rgb(&def, W, under),
        rgb(&off, W, under)
    );
    assert!(
        lum(&def, W, under) < 0.6 * lum(&off, W, under),
        "deferred is shadowed"
    );
    assert!(close(rgb(&fwd, W, under), rgb(&def, W, under), 4));
}

/// Where no geometry is drawn (depth at the far plane) the mask is 1 in both light channels.
#[test]
fn the_mask_is_unshadowed_where_nothing_is_drawn() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = renderer(cache.path(), false, floor_and_ball());
    let mut s = point_scene(true, [60.0, 0.0, 80.0]);
    s.sun_brightness = 1.0;
    s.background = Background::Solid([0.0, 0.0, 1.0]);
    let frame = r.render(&s, W, H).unwrap();
    let (mw, mh, mask) = r.read_shadow_mask().unwrap();
    assert_eq!((mw, mh), (W, H));
    let background: Vec<usize> = (0..(W * H) as usize)
        .filter(|&i| frame[i * 4..i * 4 + 3] == [0, 0, 255])
        .collect();
    assert!(background.len() > 1000, "{}", background.len());
    let bad = background
        .iter()
        .filter(|&&i| mask[i * 4] != 255 || mask[i * 4 + 1] != 255)
        .count();
    assert_eq!(bad, 0, "background pixels with a shadow in R or G");
    let shadowed = |c: usize| (0..(W * H) as usize).any(|i| mask[i * 4 + c] < 128);
    assert!(
        shadowed(0) && shadowed(1),
        "both lights cast a shadow somewhere"
    );
}

/// The preview's own case: a built-in (`load_builtin`, so `Pipe::FIXED`) under `Scene::neutral`'s oblique sun, with the
/// point light around it, at the test map sizes and at the defaults. A lone convex sphere hides no part of itself that
/// faces a light, so shadows on leave its pixels as they were wherever |N·L| is at least 0.3 for both lights (each
/// pixel's normal from the ray-sphere hit): the biases keep its curved surface free of self-shadowing, and only the
/// PCF edge next to a terminator may move. And the built-in receives: its `PixelShaderDescriptor` carries ShadowDir
/// and DefShadow.
#[test]
fn a_lone_builtin_sphere_does_not_shadow_itself() {
    for sizes in [Some((512, 256)), None] {
        let cache = tempfile::tempdir().unwrap();
        let mut r = Renderer::with_gpu_source(
            Gpu::new_warp().unwrap(),
            render3d::ShaderSource::Bundled,
            cache.path().to_path_buf(),
        )
        .unwrap();
        if let Some((sun, point)) = sizes {
            r.set_shadow_sizes(sun, point);
        }
        let rep = r.load_builtin(Builtin::Sphere, MaterialState::builtin_default(), &|_| None);
        assert!(
            rep.errors.is_empty() && rep.compile_errors.is_empty(),
            "{rep:?}"
        );
        let bits = |r: &Renderer| {
            r.draw_cb_floats(ShapeId(0), "PixelShaderDescriptor")
                .unwrap()[0]
                .to_bits()
                & ((1 << 13) | (1 << 14))
        };
        let mut s = Scene::neutral();
        s.point_light = PointLight::around(r.bounds().unwrap(), 1.0, 3.0);
        let off = r.render(&s, W, H).unwrap();
        assert_eq!(bits(&r), 0);
        s.shadows = true;
        let on = r.render(&s, W, H).unwrap();
        assert_eq!(bits(&r), (1 << 13) | (1 << 14), "the built-in receives");
        let (c, radius) = r.bounds().unwrap();
        let v = View::new(&s.camera, r.bounds(), W, H);
        let inv = render3d::scene::inverse4(&v.view_proj);
        let centre = [c[0] - v.eye[0], c[1] - v.eye[1], c[2] - v.eye[2]];
        let light = [
            s.point_light.position[0] - v.eye[0],
            s.point_light.position[1] - v.eye[1],
            s.point_light.position[2] - v.eye[2],
        ];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let unit = |a: [f32; 3]| a.map(|x| x / dot(a, a).sqrt());
        let (mut away, mut changed, mut worst) = (0, 0, 0);
        for y in 0..H {
            for x in 0..W {
                let ndc = [
                    (x as f32 + 0.5) / W as f32 * 2.0 - 1.0,
                    1.0 - (y as f32 + 0.5) / H as f32 * 2.0,
                    1.0,
                    1.0,
                ];
                let h: Vec<f32> = (0..4)
                    .map(|k| (0..4).map(|j| inv[k][j] * ndc[j]).sum())
                    .collect();
                let d = unit([h[0] / h[3], h[1] / h[3], h[2] / h[3]]);
                // The ray from the eye (origin) meets the sphere at t: |t d - centre| = radius.
                let b = dot(d, centre);
                let disc = b * b - dot(centre, centre) + radius * radius;
                if disc < 0.0 {
                    continue;
                }
                let t = b - disc.sqrt();
                let p = d.map(|k| k * t);
                let n = unit([p[0] - centre[0], p[1] - centre[1], p[2] - centre[2]]);
                let sun = dot(n, unit(s.dir_light.direction));
                let pt = dot(n, unit([light[0] - p[0], light[1] - p[1], light[2] - p[2]]));
                // The PCF edge of a terminator may darken the few pixels next to it, which a light barely reaches.
                if sun.abs().min(pt.abs()) < 0.3 {
                    continue;
                }
                away += 1;
                let i = ((y * W + x) * 4) as usize;
                let diff = (0..3)
                    .map(|k| on[i + k].abs_diff(off[i + k]))
                    .max()
                    .unwrap();
                if diff > 1 {
                    changed += 1;
                    worst = worst.max(diff);
                }
            }
        }
        println!("sizes {sizes:?}: {away} px away from both terminators, {changed} changed, worst {worst}");
        assert!(away > 10_000, "the sphere fills the frame: {away}");
        assert_eq!(
            changed, 0,
            "sizes {sizes:?}: self-shadowed px, worst {worst}"
        );
    }
}

/// The shadow bits (ShadowDir, DefShadow) in shape `id`'s `PixelShaderDescriptor`.
fn shadow_bits(r: &Renderer, id: u32) -> u32 {
    r.draw_cb_floats(ShapeId(id), "PixelShaderDescriptor")
        .unwrap()[0]
        .to_bits()
        & ((1 << 13) | (1 << 14))
}

/// Final review: a shape that writes depth but does not test it is drawn with the camera pre-pass's depth test off, so
/// the mask under it holds whatever is behind; it receives no shadow bits. The plane (testing and writing) does.
#[test]
fn a_shape_that_does_not_test_depth_gets_no_shadow_bits() {
    let cache = tempfile::tempdir().unwrap();
    let mut untested = opaque();
    untested.slsf1 &= !(1 << 31); // ZBuffer_Test clear, ZBuffer_Write still set
    let mut r = renderer(
        cache.path(),
        false,
        vec![sphere(0, untested), plane(1, opaque(), -40.0)],
    );
    r.render(&scene(true), W, H).unwrap();
    assert_eq!(
        shadow_bits(&r, 1),
        (1 << 13) | (1 << 14),
        "the plane receives"
    );
    assert_eq!(shadow_bits(&r, 0), 0, "the untested sphere does not");
}

/// Final review: a shape whose edit is still compiling keeps drawing its previous draw; a shadows flip meanwhile
/// writes that draw's shadow bits (from the draw's own blend and depth state), and off clears them again.
#[test]
fn a_shadow_flip_reaches_a_draw_whose_edit_is_still_compiling() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = Renderer::with_gpu_source(
        Gpu::new_warp().unwrap(),
        render3d::ShaderSource::Bundled,
        cache.path().to_path_buf(),
    )
    .unwrap();
    r.set_shadow_sizes(512, 256);
    r.set_async_compile(true);
    r.load_parsed(
        &ParsedNif {
            shapes: floor_and_ball(),
            ..Default::default()
        },
        &|_| None,
    );
    let drain = |r: &mut Renderer| {
        for j in r.take_jobs() {
            r.apply_compiled(render3d::run_job(j));
        }
    };
    drain(&mut r);
    r.render(&scene(false), W, H).unwrap();
    assert_eq!(shadow_bits(&r, 1), 0);
    // A permutation edit (alpha test on): the plane keeps its old draw while the new program compiles.
    let mut m = opaque();
    m.alpha = Some(nif::props::Alpha {
        flags: 0x200,
        threshold: 0,
    });
    assert_eq!(
        r.set_material(ShapeId(1), m, &|_| None),
        Ok(render3d::EditKind::Permutation)
    );
    assert_eq!(
        r.shapes()[1].status,
        render3d::ShapeStatus::Pending {
            showing_previous: true
        }
    );
    r.render(&scene(true), W, H).unwrap();
    assert_eq!(
        shadow_bits(&r, 1),
        (1 << 13) | (1 << 14),
        "the retained draw receives"
    );
    r.render(&scene(false), W, H).unwrap();
    assert_eq!(shadow_bits(&r, 1), 0, "and off clears it");
    // The edit lands with the bits of the current frame's setting.
    r.render(&scene(true), W, H).unwrap();
    drain(&mut r);
    assert_eq!(r.shapes()[1].status, render3d::ShapeStatus::Drawing);
    assert_eq!(shadow_bits(&r, 1), (1 << 13) | (1 << 14));
}

/// A blended plane under the opaque sphere keeps its shadows-off colour. It blends SrcAlpha / InvSrcAlpha at alpha 1,
/// so it hides the opaque plane below it; that plane is shadowed at the same pixel, so the mask there is dark: a
/// blended shape that read it would darken.
#[test]
fn blended_shapes_are_not_shadowed() {
    let cache = tempfile::tempdir().unwrap();
    let mut over = blended(true);
    over.alpha = Some(nif::props::Alpha {
        flags: 0x00ED, // blend on, SrcAlpha / InvSrcAlpha
        threshold: 0,
    });
    let mut r = renderer(
        cache.path(),
        false,
        vec![
            sphere(0, opaque()),
            plane(1, over, -40.0),
            plane(2, opaque(), -60.0),
        ],
    );
    let off = r.render(&scene(false), W, H).unwrap();
    let on = r.render(&scene(true), W, H).unwrap();
    let under = px(&r, &scene(true), W, H, UNDER);
    let (_, _, mask) = r.read_shadow_mask().unwrap();
    assert!(
        mask[((under.1 * W + under.0) * 4) as usize] < 128,
        "the opaque plane below is in the sun's shadow there"
    );
    assert!(lum(&off, W, under) > 20.0);
    assert!(
        close(rgb(&off, W, under), rgb(&on, W, under), 1),
        "{:?} vs {:?}",
        rgb(&off, W, under),
        rgb(&on, W, under)
    );
}
