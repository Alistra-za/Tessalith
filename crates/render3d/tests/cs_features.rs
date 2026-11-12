//! T-S5 (03 §8, C9): each preview feature define (`PREVIEW_FEATURES`) alone with `ENABLE_LL`, and all of them as
//! the bundled default settings turn them on, on the bundled Community Shaders. A face-tint head and a skin-tint
//! body (both SKIN) and a hair (HAIR) mesh must compile, draw, and add no NaN pixels. A row that fails is deleted
//! from `PREVIEW_FEATURES` and the plan records it. Ignored: reads the vanilla meshes from the MO2 instance
//! read-only; panics without it.
//! `$env:SDC_ACHERUS='<your MO2 instance>'; cargo test -p render3d --release --test cs_features -- --ignored --nocapture`
use render3d::cs_settings::{defaults, preview_defines, CsSettings, PREVIEW_FEATURES};
use render3d::device::Gpu;
use render3d::scene::{Background, Scene};
use render3d::{parse, Renderer};
use std::path::{Path, PathBuf};

/// Vanilla meshes, loose in the owner's (disabled) extract mod: read by path, no BSA, no scan.
const ASSETS: &str = "mods/SkyrimMeshesExtract/meshes/actors/character/character assets";
/// (file under ASSETS, shader type that must be present: 4 FaceGen (face tint) and 5 FaceGen RGB tint
/// (skin tint) → SKIN, 6 HairTint → HAIR)
const MESHES: [(&str, u32); 3] = [
    ("femalehead.nif", 4),
    ("femalebody_1.nif", 5),
    ("hair/femaledarkelfhair01.nif", 6),
];
const N: u32 = 128;

struct Shot {
    drawn: usize,
    sum: u64,
    black: usize,
}

fn acherus() -> PathBuf {
    PathBuf::from(std::env::var("SDC_ACHERUS").expect("set SDC_ACHERUS to the MO2 instance root"))
}

/// The bundled default settings (Note GT3): the settings the preview uses, whoever's CS is installed.
fn bundled_settings() -> CsSettings {
    defaults().clone()
}

/// Whether the settings turn feature `name` on: CS loaded it and its own master switch (Vanilla Fresnel's
/// `Enable`, Hair Specular's `Enabled`) is not 0.
fn enables(s: &CsSettings, name: &str) -> bool {
    let switch = match name {
        "VanillaFresnel" => Some(("Vanilla Fresnel", "Enable")),
        "HairSpecular" => Some(("Hair Specular", "Enabled")),
        _ => None,
    };
    let off = switch.is_some_and(|(section, key)| {
        s.merged
            .get(section)
            .and_then(|v| v.get(key))
            .is_some_and(|v| v.as_f64() == Some(0.0) || v.as_bool() == Some(false))
    });
    s.loaded.contains(name) && !off
}

/// One mesh, one define set, one frame on a grey background, in a fresh renderer.
fn shoot(cache: &Path, s: &CsSettings, defines: &[String], nif: &[u8]) -> Result<Shot, String> {
    let mut r = Renderer::with_gpu(Gpu::new_warp()?, cache.to_path_buf())?;
    r.set_settings(s.clone()); // the bundled FeatureData values (and LL encode)
    r.set_features(defines.to_vec()); // then exactly this define set
    let rep = r.load(nif, &|_| None);
    if rep.shapes == 0
        || rep.pending > 0
        || !rep.errors.is_empty()
        || !rep.compile_errors.is_empty()
    {
        return Err(format!("load: {rep:?}"));
    }
    let mut scene = Scene::neutral();
    scene.background = Background::Solid([0.5; 3]);
    let px = r.render(&scene, N, N)?;
    let bg = [px[0], px[1], px[2]]; // a corner: the camera frames the bounds with a margin
    let (mut drawn, mut sum, mut black) = (0, 0u64, 0);
    for p in px.chunks(4) {
        if p[..3] == [0, 0, 0] {
            black += 1; // NaN reads back as 0 (device.rs srgb8 / unorm8)
        }
        if p[..3] != bg {
            drawn += 1;
            sum += p[..3].iter().map(|&c| c as u64).sum::<u64>();
        }
    }
    Ok(Shot { drawn, sum, black })
}

#[test]
#[ignore = "needs SDC_ACHERUS (MO2 instance root) for the vanilla meshes"]
fn each_preview_feature_compiles_and_draws_a_head_and_hair_on_the_bundled_cs() {
    let root = acherus();
    let s = bundled_settings();
    let cache = tempfile::tempdir().unwrap(); // shared by every run below: each permutation compiles once
    let ll = "ENABLE_LL".to_string();
    let all = preview_defines(&s, true);
    // "All" is what the bundled settings enable, each feature as read; every feature still compiles and draws
    // alone below.
    let mut want: Vec<String> = PREVIEW_FEATURES
        .iter()
        .filter(|(name, _)| enables(&s, name))
        .map(|(_, define)| define.to_string())
        .collect();
    if s.linear_lighting {
        want.push(ll.clone());
    }
    want.sort();
    assert_eq!(all, want, "the defines follow the bundled settings");
    let mut cases: Vec<(String, Vec<String>)> = vec![("ENABLE_LL only".into(), vec![ll.clone()])];
    for (name, define) in PREVIEW_FEATURES {
        cases.push((name.to_string(), vec![ll.clone(), define.to_string()]));
    }
    cases.push(("all (bundled settings)".into(), all));
    let mut failures = vec![];
    for (file, shader_type) in MESHES {
        let path = root.join(ASSETS).join(file);
        let nif = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            parse(&nif)
                .shapes
                .iter()
                .any(|x| x.material.shader_type == shader_type),
            "{}: no shape of shader type {shader_type}; pick a fallback mesh (plan Task 6)",
            path.display()
        );
        let mut baseline_black = None;
        for (label, defines) in &cases {
            let t0 = std::time::Instant::now();
            match shoot(cache.path(), &s, defines, &nif) {
                Ok(shot) => {
                    println!(
                        "{file} / {label}: drawn {} px, rgb sum {}, black {} ({:.1} s)",
                        shot.drawn,
                        shot.sum,
                        shot.black,
                        t0.elapsed().as_secs_f64()
                    );
                    if label == "ENABLE_LL only" {
                        baseline_black = Some(shot.black);
                    }
                    // No baseline = the LL-only run failed: that is a P0 regression, already listed above.
                    let base = baseline_black.unwrap_or(0);
                    if shot.drawn == 0 || shot.sum == 0 {
                        failures.push(format!("{file} / {label}: nothing drawn (black frame)"));
                    } else if shot.black > base {
                        failures.push(format!(
                            "{file} / {label}: {} exact-black (NaN) pixels, baseline {base}",
                            shot.black
                        ));
                    }
                }
                Err(e) => failures.push(format!("{file} / {label}: {e}")),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "delete the failing rows from PREVIEW_FEATURES (C9):\n{}",
        failures.join("\n")
    );
}
