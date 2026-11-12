//! The vanilla envmap (shader type 1, slot 4 → t4) reads cube +Z as world up (Note IC1, DS13), on the bundled CS
//! shaders (Note GT); run alone with --test-threads=1.
mod common;

use nif::material::MaterialState;
use nif::props::LightingTrailer;
use render3d::scene::{Camera, DirLight, Scene};
use render3d::{Renderer, ShapeId};

/// A 2×2 cube through the engine's writer, faces in DDS order +X −X +Y −Y +Z −Z. `Format::Rgba8` UNORM is
/// B8G8R8A8 (DXGI 87, Note S4), so each texel is given as RGBA and packed as BGRA (Note IC2).
fn cube(faces: [[u8; 4]; 6]) -> Vec<u8> {
    use engine::dds::{dds_bytes_cube, DdsDesc, ResolvedHeader};
    let desc = DdsDesc {
        width: 2,
        height: 2,
        format: engine::config::Format::Rgba8,
        srgb: false,
        header: ResolvedHeader::Dx10,
    };
    let levels: Vec<Vec<u8>> = faces
        .iter()
        .map(|[r, g, b, a]| [*b, *g, *r, *a].repeat(4))
        .collect();
    dds_bytes_cube(&desc, &levels).unwrap()
}

/// A black envmapped sphere (envmap scale 1, glossiness 1) at the origin.
fn ball() -> render3d::ParsedNif {
    let mut m = MaterialState::builtin_default();
    m.slsf1 |= 1 << 31; // ZBuffer_Test: the far side stays hidden

    m.shader_type = 1;
    m.params.trailer = LightingTrailer::EnvMap { scale: 1.0 };
    m.params.glossiness = 1.0;
    m.set_slot_raw(0, r"textures\test\black.dds");
    m.set_slot_raw(4, r"textures\test\cube.dds");
    render3d::ParsedNif {
        shapes: vec![render3d::ParsedShape {
            id: ShapeId(0),
            name: "ball".into(),
            mesh: std::sync::Arc::new(render3d::builtin_mesh(render3d::Builtin::Sphere)),
            world: common::IDENT,
            material: m,
        }],
        ..Default::default()
    }
}

/// No direct light and white ambient: CS adds the reflection through the ambient term, so a black ball shows
/// the cube's colour.
fn scene(pitch: f32) -> Scene {
    Scene {
        camera: Camera::Orbit {
            target: [0.0; 3],
            yaw: 0.0,
            pitch,
            distance: 150.0,
        },
        dir_light: DirLight {
            direction: [0.0, 0.0, 1.0],
            color: [0.0; 3],
        },
        ambient6: [[1.0; 3]; 6],
        ..Scene::neutral()
    }
}

#[test]
fn render_envmap_reads_cube_plus_z_as_up() {
    let cache = tempfile::tempdir().unwrap();
    let mut r =
        Renderer::with_source(render3d::ShaderSource::Bundled, cache.path().to_path_buf()).unwrap();
    let (red, green, blue) = ([255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]);
    let black = common::dds_rgba8(1, 1, &[[0, 0, 0, 255]]);
    let load = |r: &mut Renderer, c: Vec<u8>| {
        let rep = r.load_parsed(&ball(), &|p: &str| match p {
            "textures/test/cube.dds" => Some(c.clone()),
            "textures/test/black.dds" => Some(black.clone()),
            _ => None,
        });
        assert!(
            rep.errors.is_empty()
                && rep.compile_errors.is_empty()
                && rep.missing_textures.is_empty(),
            "{rep:?}"
        );
    };
    let (w, h) = (128u32, 128u32);
    let at = |px: &[u8], y: u32| -> [u8; 3] {
        let i = ((y * w + w / 2) * 4) as usize;
        [px[i], px[i + 1], px[i + 2]]
    };
    let red_wins = |c: [u8; 3]| c[0] as i32 - c[2] as i32 > 100;
    let blue_wins = |c: [u8; 3]| c[2] as i32 - c[0] as i32 > 100;
    // An all-red cube reads red, so `cube` packs the colour right whatever the orientation.
    load(&mut r, cube([red; 6]));
    let all_red = at(&r.render(&scene(0.0), w, h).unwrap(), h / 2);
    assert!(red_wins(all_red), "all-red cube: {all_red:?}");
    load(&mut r, cube([green, green, green, green, red, blue]));
    // From the side: the upper part of the ball reflects the sky (+Z), the lower part the ground (−Z).
    let side = r.render(&scene(0.0), w, h).unwrap();
    let (top, bottom) = (at(&side, 40), at(&side, 88));
    println!("side: top {top:?} bottom {bottom:?}");
    assert!(red_wins(top), "top of the ball reads +Z: {top:?}");
    assert!(blue_wins(bottom), "bottom of the ball reads -Z: {bottom:?}");
    // From above and below, the centre reflects straight back: +Z and −Z.
    let above = at(&r.render(&scene(1.3), w, h).unwrap(), h / 2);
    let below = at(&r.render(&scene(-1.3), w, h).unwrap(), h / 2);
    println!("above {above:?} below {below:?}");
    assert!(red_wins(above) && blue_wins(below), "{above:?} {below:?}");
}
