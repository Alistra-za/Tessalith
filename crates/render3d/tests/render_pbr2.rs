//! Note ID33: the CS 2.0 shaders draw type 21 with thin-surface shading. WARP, bundled shaders (fxc compiles).
#[path = "common/mod.rs"]
mod common;
use nif::material::MaterialState;
use nif::pbr2::{feat, PBR2_DEFAULTS};
use nif::props::LightingTrailer;
use render3d::device::Gpu;
use render3d::scene::{Camera, DirLight, Scene};
use render3d::{Builtin, Renderer, ShaderSource, ShapeId};

const N: u32 = 64;

/// A double-sided type-21 plane: subsurface (opacity 0.5, white) with `extra` features.
fn plane21(extra: u32) -> MaterialState {
    let mut m = MaterialState::builtin_default();
    m.shader_type = 21;
    m.slsf1 = 1 << 31; // Z test
    m.slsf2 = 1 | (1 << 4); // Z write, Double_Sided
    let mut params = PBR2_DEFAULTS;
    params[6] = 0.5;
    m.params.trailer = LightingTrailer::Pbr2 {
        version: 2,
        features: feat::SUBSURFACE | extra,
        params,
    };
    m
}

/// The plane (in the XZ plane) seen face-on from −Y, lit only by the sun from +Y: from behind.
fn render(r: &mut Renderer, m: MaterialState) -> Vec<u8> {
    let rep = r.load_builtin(Builtin::Plane, m, &|_| None);
    assert!(
        rep.errors.is_empty() && rep.compile_errors.is_empty(),
        "{rep:?}"
    );
    let (centre, radius) = r.bounds().unwrap();
    let mut s = Scene::neutral();
    s.camera = Camera::Orbit {
        target: centre,
        yaw: 0.0,
        pitch: 0.0,
        distance: 3.0 * radius,
    };
    s.dir_light = DirLight {
        direction: [0.0, 1.0, 0.0],
        color: [3.0; 3],
    };
    s.ambient6 = [[0.0; 3]; 6];
    r.render(&s, N, N).unwrap()
}

fn mean(px: &[u8]) -> f64 {
    let sum: u64 = (0..3).map(|c| common::channel(px, c)).sum();
    sum as f64 / (px.len() / 4 * 3) as f64
}

#[test]
fn render_pbr2_thin_surface_plane() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = Renderer::with_gpu_source(
        Gpu::new_warp().unwrap(),
        ShaderSource::Bundled,
        cache.path().to_path_buf(),
    )
    .unwrap();
    let thin = render(&mut r, plane21(feat::THIN_SURFACE));
    let flags = r.draw_cb_floats(ShapeId(0), "PBRFlags").unwrap()[0].to_bits();
    assert_ne!(
        flags & render3d::material::PBR_THIN_SURFACE,
        0,
        "PBRFlags {flags:#x}"
    );
    let thin_again = render(&mut r, plane21(feat::THIN_SURFACE));
    let solid = render(&mut r, plane21(0));
    // The noise floor: the same material drawn twice.
    let floor = (mean(&thin) - mean(&thin_again)).abs();
    let tol = floor + 1.0;
    let (t, s) = (mean(&thin), mean(&solid));
    println!(
        "covered {} | mean thin {t:.3}, again {:.3}, solid {s:.3} | floor {floor:.3}, tolerance {tol:.3}",
        common::covered(&thin),
        mean(&thin_again)
    );
    assert!(common::covered(&thin) > 0, "the back-lit thin plane shows");
    assert!(
        (t - s).abs() > tol,
        "thin {t:.3} vs solid {s:.3}: within the tolerance {tol:.3}"
    );
}
