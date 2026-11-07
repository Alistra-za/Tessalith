//! P1-C (03 §3, T-S1/T-S3): the CS settings the preview uses (Note GT3) → preview defines and FeatureData values.
//! The owner-shaped fixture files (capture 11178) are read here and merged by the test itself; no GPU.
use render3d::cs_settings::{
    defaults, diffuse_curve, feature_data, linear_lighting_on, preview_defines, CsSettings,
};
use render3d::scene::{FEATURE_FLOATS, FEATURE_UINTS};
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/cs_settings"
    ))
}

fn from_json(v: Value) -> CsSettings {
    let merged = v.as_object().unwrap().clone();
    let linear_lighting = merged
        .get("Linear Lighting")
        .and_then(|l| l.get("enableLinearLighting"))
        .and_then(Value::as_f64)
        .is_none_or(|x| x != 0.0);
    CsSettings {
        merged,
        linear_lighting,
        ..CsSettings::default()
    }
}

/// `top` over `base` per key, nested objects per key too.
fn overlay(base: &mut Map<String, Value>, top: Map<String, Value>) {
    for (k, v) in top {
        match (base.get_mut(&k), v) {
            (Some(Value::Object(b)), Value::Object(t)) => overlay(b, t),
            (Some(slot), v) => *slot = v,
            (None, v) => {
                base.insert(k, v);
            }
        }
    }
}

/// The owner-shaped fixture (capture 11178): `SettingsUser.json` over `SettingsDefault.json` when `user`, else
/// `SettingsDefault.json` alone.
fn owner_shaped(user: bool) -> CsSettings {
    let read = |n: &str| match serde_json::from_slice(&std::fs::read(fixtures().join(n)).unwrap()) {
        Ok(Value::Object(m)) => m,
        _ => panic!("{n} is not a JSON object"),
    };
    let mut merged = read("SettingsDefault.json");
    if user {
        overlay(&mut merged, read("SettingsUser.json"));
    }
    from_json(Value::Object(merged))
}

/// The installed CS 1.9.0's reflected FeatureData/b7 names (P1-D fixture): the build these tests describe.
fn aio190() -> BTreeSet<String> {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/cs_versions/aio190/fields.txt"
    ))
    .unwrap()
    .lines()
    .filter(|l| !l.is_empty())
    .map(String::from)
    .collect()
}

/// Note GT3: the preview's settings are the bundled `SettingsDefault.json` of the pinned CS with every feature loaded.
#[test]
fn bundled_defaults_give_the_expected_defines() {
    let s = defaults();
    assert_eq!(
        preview_defines(s, true),
        [
            "CS_HAIR",
            "CS_SKIN",
            "DYNAMIC_CUBEMAPS",
            "ENABLE_LL",
            "EXTENDED_MATERIALS",
            "EXTENDED_TRANSLUCENCY",
            "VANILLA_FRESNEL"
        ]
    );
    assert!(linear_lighting_on(s, false, true));
    assert_eq!(
        s.merged["Linear Lighting"]["gameGamma"].as_f64().unwrap() as f32,
        1.6
    );
    assert!(preview_defines(s, false).is_empty());
    assert!(
        s.merged["Disable at Boot"].is_null(),
        "no feature is switched off"
    );
}

/// Gated on the pinned CS Shaders folder (read-only owner data): `FEATURE_NAMES` is its `Features/*.ini` stems.
#[test]
fn feature_names_equal_the_pinned_features_folder() {
    let Ok(root) = std::env::var("SDC_CS_SHADERS") else {
        eprintln!("SDC_CS_SHADERS unset: skipped");
        return;
    };
    let mut stems: Vec<String> = std::fs::read_dir(Path::new(&root).join("Features"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "ini"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    stems.sort();
    let mut names: Vec<&str> = render3d::cs_settings::FEATURE_NAMES.to_vec();
    names.sort();
    assert_eq!(stems, names);
}

/// T-S1: a key the settings lack is left out of `merged`, and `feature_data` fills it from the built-in table.
#[test]
fn a_key_the_settings_lack_keeps_the_builtin_constant() {
    let s = from_json(
        serde_json::json!({"Linear Lighting": {"glowmapMult": 0.5, "ambientMult": 2.0}, "Version": "1-9-0-0"}),
    );
    assert!(s.merged["Linear Lighting"]
        .get("directionalLightMult")
        .is_none());
    let fd = feature_data(&s, &aio190(), true, false);
    assert_eq!(value(&fd, "linearLightingSettings.glowmapMult"), [0.5]);
    assert_eq!(value(&fd, "linearLightingSettings.ambientMult"), [2.0]);
    assert_eq!(
        value(&fd, "linearLightingSettings.directionalLightMult"),
        [1.0],
        "built-in (capture 11178) when the settings lack it"
    );
}

/// C8: defines from loaded features and their own switches; LL from the setting; off toggle → none.
#[test]
fn preview_defines_follow_loaded_and_switches() {
    let s = defaults();
    assert!(preview_defines(s, false).is_empty());
    // A blank slate: exactly the Linear Lighting default (the renderer's `set_features_on` relies on it).
    assert_eq!(preview_defines(&CsSettings::default(), true), ["ENABLE_LL"]);
    let mut off = s.clone();
    off.merged.get_mut("Hair Specular").unwrap()["Enabled"] = 0.into();
    off.merged.get_mut("Vanilla Fresnel").unwrap()["Enable"] = 0.into();
    off.linear_lighting = false;
    assert_eq!(
        preview_defines(&off, true),
        [
            "CS_SKIN",
            "DYNAMIC_CUBEMAPS",
            "EXTENDED_MATERIALS",
            "EXTENDED_TRANSLUCENCY"
        ]
    );
    let ll_off = from_json(serde_json::json!({"Linear Lighting": {"enableLinearLighting": 0}}));
    assert!(!ll_off.linear_lighting);
    assert!(preview_defines(&ll_off, true).is_empty());
}

/// D5 (R1): Dynamic Cubemaps has no master switch (its ini has only [Info]): loaded means DYNAMIC_CUBEMAPS.
#[test]
fn dynamic_cubemaps_loaded_adds_the_define() {
    let mut s = defaults().clone();
    assert!(preview_defines(&s, true)
        .iter()
        .any(|d| d == "DYNAMIC_CUBEMAPS"));
    s.loaded.remove("DynamicCubemaps");
    assert!(!preview_defines(&s, true)
        .iter()
        .any(|d| d == "DYNAMIC_CUBEMAPS"));
    assert!(preview_defines(&s, false).is_empty());
}

/// Trace 2026-11-04 #45: with Linear Lighting on, a Game Gamma other than 1.6 compiles as CS_GAME_GAMMA (the f32
/// to 9 decimals, as CS passes it: the composite test's `1.600000024`); 1.6 (the files hold the f32), no key, an
/// unusable value or LL off adds nothing.
#[test]
fn game_gamma_from_settings_is_a_define() {
    let with = |json: &str| from_json(serde_json::from_str(json).unwrap());
    assert_eq!(
        preview_defines(&with(r#"{"Linear Lighting": {"gameGamma": 2.2}}"#), true),
        ["CS_GAME_GAMMA=2.200000048", "ENABLE_LL"]
    );
    assert!(preview_defines(&with(r#"{"Linear Lighting": {"gameGamma": 2.2}}"#), false).is_empty());
    for json in [
        r#"{"Linear Lighting": {"gameGamma": 1.600000023841858}}"#,
        r#"{"Linear Lighting": {"gameGamma": 1.6}}"#,
        r#"{"Linear Lighting": {}}"#,
    ] {
        assert_eq!(preview_defines(&with(json), true), ["ENABLE_LL"], "{json}");
    }
    // Not a usable gamma (0, negative, past f32): the shaders keep their 1.6.
    for g in ["0", "-1", "1e39"] {
        let json = format!(r#"{{"Linear Lighting": {{"gameGamma": {g}}}}}"#);
        assert_eq!(preview_defines(&with(&json), true), ["ENABLE_LL"], "{json}");
    }
    // Linear Lighting off: the shaders ignore CS_GAME_GAMMA, so the defines (and program keys) do not change.
    assert_eq!(
        preview_defines(
            &with(r#"{"Linear Lighting": {"enableLinearLighting": 0, "gameGamma": 2.2}}"#),
            true
        ),
        preview_defines(
            &with(r#"{"Linear Lighting": {"enableLinearLighting": 0}}"#),
            true
        )
    );
    // The bundled defaults (gameGamma 1.600000023841858): no CS_GAME_GAMMA.
    assert!(!preview_defines(defaults(), true)
        .iter()
        .any(|d| d.starts_with("CS_GAME_GAMMA")));
}

fn value<'a>(fd: &'a [(String, Vec<f64>)], name: &str) -> &'a [f64] {
    &fd.iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("{name} missing"))
        .1
}

fn bits(v: &[f64]) -> Vec<u32> {
    v.iter().map(|x| (*x as f32).to_bits()).collect()
}

/// Every FEATURE_FLOATS / FEATURE_UINTS value, bit-exact as written to the cbuffer.
fn assert_constants(fd: &[(String, Vec<f64>)]) {
    for (n, v) in FEATURE_FLOATS {
        let want: Vec<u32> = v.iter().map(|x| x.to_bits()).collect();
        assert_eq!(bits(value(fd, n)), want, "{n}");
    }
    for (n, u) in FEATURE_UINTS {
        assert_eq!(value(fd, n), [f64::from(*u)], "{n}");
    }
}

/// T-S3 (C4): the derived diffuseCurve from the owner's diffuseGamma / mid / white reflectance.
#[test]
fn diffuse_curve_derivation() {
    let c = diffuse_curve(2.200000047683716, 0.28717461228370667, 1.0);
    assert!((c - 1.448_227_7).abs() < 1e-6, "{c}");
}

/// trace 2026-10-28 #27: `diffuseMidReflectance: 1.0` with no white reflectance (1.0) divides by zero; a mid above
/// white gives a negative curve. Both keep the CS default curve.
#[test]
fn diffuse_curve_without_a_white_mid_gap_keeps_the_default() {
    let default = FEATURE_FLOATS
        .iter()
        .find(|(n, _)| *n == "linearLightingSettings.diffuseCurve")
        .map(|(_, v)| f64::from(v[0]))
        .unwrap();
    for (mid, white) in [(1.0, 1.0), (1.5, 1.0), (f64::NAN, 1.0)] {
        let c = diffuse_curve(2.2, mid, white);
        assert_eq!(c, default, "mid {mid}, white {white}");
    }
}

/// T-S3: the owner-shaped files reproduce every capture-11178 value; pointLightMult stays π although the file says 1.
#[test]
fn owner_settings_reproduce_capture_values() {
    let s = owner_shaped(true);
    assert_eq!(
        s.merged["Linear Lighting"]["pointLightMult"].as_f64(),
        Some(1.0)
    );
    let fd = feature_data(&s, &aio190(), true, false);
    assert_constants(&fd);
    assert_eq!(
        bits(value(&fd, "linearLightingSettings.pointLightMult")),
        [std::f32::consts::PI.to_bits()]
    );
}

/// No files: exactly today's constants, plus the C2 per-geometry copy and zeros.
#[test]
fn default_settings_reproduce_todays_constants() {
    let fd = feature_data(&CsSettings::default(), &aio190(), true, false);
    assert_constants(&fd);
    for f in [
        "skinParams",
        "skinParams2",
        "skinDetailParams",
        "sssParams",
        "fuzzParams",
        "physicalParams",
        "wetParams",
    ] {
        assert_eq!(
            value(&fd, &format!("skinPerGeometryProfile.{f}")),
            value(&fd, &format!("skinData.{f}")),
            "{f}"
        );
    }
    assert_eq!(value(&fd, "skinPerGeometry"), [0.0; 4]);
    assert_eq!(value(&fd, "skinMaterialFlags"), [0.0; 4]);
    assert_eq!(
        value(&fd, "linearLightingSettings.isMainOrLoadingMenu"),
        [0.0]
    );
}

/// C5: section → struct prefix with the renamed keys; skin packing from the profile.
#[test]
fn owner_section_keys_map_to_struct_fields() {
    let fd = feature_data(&owner_shaped(true), &aio190(), true, false);
    assert_eq!(
        value(&fd, "extendedTranslucencySettings.MaterialModel"),
        [3.0]
    );
    assert_eq!(
        bits(value(&fd, "extendedTranslucencySettings.Reduction")),
        [0.15f32.to_bits()]
    );
    assert_eq!(value(&fd, "extendedTranslucencySettings.Strength"), [0.0]);
    assert_eq!(
        value(&fd, "extendedMaterialSettings.EnableTerrainParallax"),
        [0.0]
    );
    assert_eq!(value(&fd, "extendedMaterialSettings.EnableParallax"), [1.0]);
    assert!(!fd.iter().any(|(n, _)| n.ends_with("AlphaMode")));
    assert_eq!(
        value(&fd, "skinData.skinDetailParams"),
        [20.0, 2.0, 0.25, 0.0]
    );
    // Default alone has EnableSkinDetail true: detail on.
    let d = feature_data(&owner_shaped(false), &aio190(), true, false);
    assert_eq!(value(&d, "skinData.skinDetailParams")[3], 1.0);
}
