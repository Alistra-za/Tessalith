//! The Community Shaders settings the preview uses (03 §3, Note GT3): the pinned CS's `SettingsDefault.json` and
//! its feature list ([`defaults`]), and the feature defines the preview compiles. The user's own CS files are
//! never read.
use crate::cbuffer::CbWriter;
use crate::compile::{CbLayout, VarKind};
use crate::scene::{FEATURE_FLOATS, FEATURE_UINTS};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq)]
pub struct CsSettings {
    /// Short names of the features CS loaded.
    pub loaded: BTreeSet<String>,
    /// `"Linear Lighting".enableLinearLighting != 0`; true when absent (the P0 default).
    pub linear_lighting: bool,
    /// Every section, nested objects included.
    pub merged: Map<String, Value>,
}

impl Default for CsSettings {
    /// Built-in values, Linear Lighting on, no feature loaded (the tests' blank slate; the preview uses [`defaults`]).
    fn default() -> Self {
        CsSettings {
            loaded: BTreeSet::new(),
            linear_lighting: true,
            merged: Map::new(),
        }
    }
}

/// Material-local CS features the preview compiles when loaded and not switched off (03 §3.2, C8): Info.ini
/// name → define. A row whose installed-CS gate test (T-S5) fails is deleted here (C9).
pub const PREVIEW_FEATURES: &[(&str, &str)] = &[
    ("VanillaFresnel", "VANILLA_FRESNEL"),
    ("HairSpecular", "CS_HAIR"),
    ("Skin", "CS_SKIN"),
    ("ExtendedMaterials", "EXTENDED_MATERIALS"),
    ("ExtendedTranslucency", "EXTENDED_TRANSLUCENCY"),
    // No master switch (Features/DynamicCubemaps.ini has only [Info]): loaded means on. Lighting and the deferred
    // composite get it together (D5 R1).
    ("DynamicCubemaps", "DYNAMIC_CUBEMAPS"),
];

/// The `Shaders/Features/*.ini` stems of the pinned CS (1.9.0 (AIO build 2026-10-03)): every feature counts as loaded in the
/// bundled defaults. The test `feature_names_equal_the_pinned_features_folder` compares it with the folder.
pub const FEATURE_NAMES: &[&str] = &[
    "CloudShadows",
    "CSEditor",
    "DynamicCubemaps",
    "Effects11",
    "ExponentialHeightFog",
    "ExtendedMaterials",
    "ExtendedTranslucency",
    "GrassCollision",
    "GrassLighting",
    "GrassOptimizations",
    "HairSpecular",
    "HDRDisplay",
    "HorizonFix",
    "ImageBasedLighting",
    "InteriorSun",
    "InverseSquareLighting",
    "LightLimitFix",
    "LinearLighting",
    "LODBlending",
    "NRD",
    "PerformanceOverlay",
    "PhysicalSky",
    "PostProcessing",
    "PseudoSunBounce",
    "RemoteControl",
    "RenderDoc",
    "SceneManager",
    "Screenshot",
    "ScreenSpaceGI",
    "ScreenSpacePointLightShadows",
    "ScreenSpaceReflections",
    "ScreenSpaceShadows",
    "Skin",
    "Skylighting",
    "SkySync",
    "SubsurfaceScattering",
    "TerrainBlending",
    "TerrainHelper",
    "TerrainShadows",
    "TerrainVariation",
    "TruePBR",
    "UnifiedWater",
    "Upscaling",
    "VanillaFresnel",
    "VolumetricLighting",
    "VolumetricShadows",
    "WaterEffects",
    "WetnessEffects",
];

/// The preview's settings (Note GT3): the pinned CS's `SettingsDefault.json` from the embedded tree, with every
/// [`FEATURE_NAMES`] feature loaded. Its `"Disable at Boot"` is null, so none is switched off.
pub fn defaults() -> &'static CsSettings {
    static D: std::sync::OnceLock<CsSettings> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        let bytes =
            crate::bundled::read("SettingsDefault.json").expect("bundled SettingsDefault.json");
        let Ok(Value::Object(merged)) = serde_json::from_slice(strip_bom(bytes)) else {
            panic!("bundled SettingsDefault.json is not a JSON object");
        };
        let loaded: BTreeSet<String> = FEATURE_NAMES.iter().map(|n| n.to_string()).collect();
        CsSettings {
            linear_lighting: setting(&merged, "Linear Lighting", "enableLinearLighting")
                .is_none_or(|x| x != 0.0),
            loaded,
            merged,
        }
    })
}

/// The shaders' `CS_GAME_GAMMA` when none is passed (TransferFunctions.hlsli:7).
const SHADER_GAME_GAMMA: f32 = 1.6;

/// The feature defines for these settings (C8), sorted, so equal lists mean "defines unchanged".
pub fn preview_defines(s: &CsSettings, features_on: bool) -> Vec<String> {
    if !features_on {
        return vec![];
    }
    let mut out: Vec<String> = PREVIEW_FEATURES
        .iter()
        .filter(|(name, _)| s.loaded.contains(*name) && !switched_off(s, name))
        .map(|(_, define)| define.to_string())
        .collect();
    if s.linear_lighting {
        out.push("ENABLE_LL".into());
        // The shaders read CS_GAME_GAMMA only under ENABLE_LL (TransferFunctions.hlsli:12-16), and only a value off
        // their built-in one changes anything, so default users keep today's defines. The files hold the f32; CS
        // passes it to 9 decimals (`1.600000024`).
        if let Some(g) = setting(&s.merged, "Linear Lighting", "gameGamma").map(|g| g as f32) {
            if g != SHADER_GAME_GAMMA && g.is_finite() && g > 0.0 {
                out.push(format!("CS_GAME_GAMMA={:.9}", f64::from(g)));
            }
        }
    }
    // shortcut: ENABLE_ACESCG is not compiled; the readback has no ACEScg conversion. Upgrade when a user reports
    // ACEScg on.
    out.sort();
    out
}

/// The feature's own master switch is 0. Skin, ExtendedMaterials and ExtendedTranslucency have none.
fn switched_off(s: &CsSettings, feature: &str) -> bool {
    let (section, key) = match feature {
        "VanillaFresnel" => ("Vanilla Fresnel", "Enable"),
        "HairSpecular" => ("Hair Specular", "Enabled"),
        _ => return false,
    };
    setting(&s.merged, section, key).is_some_and(|x| x == 0.0)
}

fn setting(m: &Map<String, Value>, section: &str, key: &str) -> Option<f64> {
    m.get(section).and_then(|s| s.get(key)).and_then(num)
}

/// A JSON number, or a bool as 1/0 (CS writes enables as integers and some flags as bools).
fn num(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_bool().map(|b| f64::from(u8::from(b))))
}

fn strip_bom(b: &[u8]) -> &[u8] {
    b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)
}

/// JSON section → FeatureData struct prefix, with the keys whose struct field is named differently (C5).
type Section = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);
/// Reflected only by the new Linear Lighting (AIO 1.9.0): π on the CPU, LL switched by a compile-time define.
pub const NEW_LL: &str = "linearLightingSettings.diffuseCurve";
/// Reflected only by the old Linear Lighting (1.6, 1.8, Nexus 1.9.1): π in the shader, LL switched by a cbuffer flag.
pub const OLD_LL: &str = "linearLightingSettings.isDirLightLinear";

/// Old-LL fields the 1.9 settings files have no key for, at the 1.6/1.8 `SettingsDefault.json` values (identical in
/// both builds).
const OLD_LL_DEFAULTS: [(&str, f32); 15] = [
    ("ambientGamma", 1.8),
    ("colorGamma", 1.8),
    ("effectAlphaGamma", 1.55),
    ("effectGamma", 1.4),
    ("emitColorGamma", 1.8),
    ("emitColorMult", 1.0),
    ("enableACEScg", 0.0),
    ("fogAlphaGamma", 1.8),
    ("fogGamma", 1.97),
    ("glowmapGamma", 1.8),
    ("lightGamma", 1.8),
    ("skyGamma", 1.8),
    ("vanillaDiffuseColorMult", 1.0),
    ("vlGamma", 1.8),
    ("waterGamma", 1.8),
];

/// Whether Linear Lighting is on in the preview. Old LL: the setting, with no key = off (the shader's flag reads the
/// written 0, Nexus before CS's first run). New LL: `CsSettings::linear_lighting` (no key = the P0 default, on).
pub fn linear_lighting_on(s: &CsSettings, old_ll: bool, features_on: bool) -> bool {
    features_on
        && if old_ll {
            setting(&s.merged, "Linear Lighting", "enableLinearLighting").is_some_and(|x| x != 0.0)
        } else {
            s.linear_lighting
        }
}

const SECTIONS: &[Section] = &[
    ("Linear Lighting", "linearLightingSettings", &[]),
    ("Hair Specular", "hairSpecularSettings", &[]),
    ("Vanilla Fresnel", "vanillaFresnelSettings", &[]),
    ("True PBR", "truePBRSettings", &[]),
    (
        "Extended Materials",
        "extendedMaterialSettings",
        &[("EnableTerrain", "EnableTerrainParallax")],
    ),
    (
        "Extended Translucency",
        "extendedTranslucencySettings",
        &[
            ("AlphaMode", "MaterialModel"),
            ("AlphaReduction", "Reduction"),
            ("AlphaSoftness", "Softness"),
            ("AlphaStrength", "Strength"),
        ],
    ),
];

/// The struct field a section key lands in (C5 renames).
fn renamed<'a>(renames: &[(&str, &'a str)], key: &'a str) -> &'a str {
    renames.iter().find(|r| r.0 == key).map_or(key, |r| r.1)
}

/// Linear Lighting's `diffuseCurve` (C4, no settings key): the inverse of CS's DiffuseToWorking curve at mid
/// grey. `x = 0.5^gamma; mid·(1−x) / (x·(white−mid))`. Without a white-mid gap, or with a non-finite result, the
/// [`FEATURE_FLOATS`] default (trace 2026-10-28 #27).
pub fn diffuse_curve(gamma: f64, mid: f64, white: f64) -> f64 {
    let x = 0.5f64.powf(gamma);
    let c = mid * (1.0 - x) / (x * (white - mid));
    if white - mid > 1e-6 && c.is_finite() {
        return c;
    }
    FEATURE_FLOATS
        .iter()
        .find(|(n, _)| *n == "linearLightingSettings.diffuseCurve")
        .map_or(1.0, |(_, v)| f64::from(v[0]))
}

/// Every cbuffer variable name of `layouts` without its namespace (`SharedData::linearLightingSettings.diffuseCurve`
/// → `linearLightingSettings.diffuseCurve`): the shape evidence [`feature_data`] keys its exceptions on (P1-D).
pub fn reflected_fields<'a>(layouts: impl IntoIterator<Item = &'a CbLayout>) -> BTreeSet<String> {
    layouts
        .into_iter()
        .flat_map(|l| &l.vars)
        .map(|v| unqualified(&v.name).to_string())
        .collect()
}

/// `ns::a.b` → `a.b`. Only the part before the first `.` can carry a namespace (struct members are dotted).
fn unqualified(name: &str) -> &str {
    let head = name.split('.').next().unwrap_or(name);
    head.rfind("::").map_or(name, |i| &name[i + 2..])
}

/// FeatureData values by cbuffer name, raw numbers (the writer types them from the reflection): the
/// capture-11178 constants, overwritten by every number the settings have (C7: a missing file or key keeps the
/// constant). Names this CS build lacks are dropped at write time. `fields` = the reflected names of the program
/// the values are for ([`reflected_fields`]); `head` = the draw is a head (per-geometry skin tiling, P1-D).
pub fn feature_data(
    s: &CsSettings,
    fields: &BTreeSet<String>,
    features_on: bool,
    head: bool,
) -> Vec<(String, Vec<f64>)> {
    let mut out: Vec<(String, Vec<f64>)> = FEATURE_FLOATS
        .iter()
        .map(|(n, v)| (n.to_string(), v.iter().map(|x| f64::from(*x)).collect()))
        .chain(
            FEATURE_UINTS
                .iter()
                .map(|(n, u)| (n.to_string(), vec![f64::from(*u)])),
        )
        .collect();
    for (section, prefix, renames) in SECTIONS {
        let Some(Value::Object(m)) = s.merged.get(*section) else {
            continue;
        };
        for (key, v) in m {
            let field = renamed(renames, key);
            let vals: Vec<f64> = match v {
                Value::Array(a) => a.iter().filter_map(num).collect(),
                v => num(v).into_iter().collect(),
            };
            if !vals.is_empty() {
                put(&mut out, &format!("{prefix}.{field}"), vals);
            }
        }
    }
    let ll = |k: &str| setting(&s.merged, "Linear Lighting", k);
    if let (Some(gamma), Some(mid)) = (ll("diffuseGamma"), ll("diffuseMidReflectance")) {
        let white = ll("diffuseWhiteReflectance").unwrap_or(1.0);
        put(
            &mut out,
            "linearLightingSettings.diffuseCurve",
            vec![diffuse_curve(gamma, mid, white)],
        );
    }
    let ll_on = linear_lighting_on(s, fields.contains(OLD_LL), features_on);
    if fields.contains(NEW_LL) {
        // New LL: the CPU uploads π × pointLightMult and the shader no longer multiplies (dev LinearLighting.cpp:
        // 228-229; capture 11178: setting 1 → π). Directional stays raw: the capture shows 1.0.
        let point = f64::from(std::f32::consts::PI) * ll("pointLightMult").unwrap_or(1.0);
        put(
            &mut out,
            "linearLightingSettings.pointLightMult",
            vec![point],
        );
        if !ll_on {
            // From dev source only (LinearLighting.cpp:207-225, 2026-10-09), not verified against the installed
            // DLL: with LL inactive CS uploads every multiplier and fireEffectCurve as 1.0; the 1.9 shader reads
            // them ungated (ColorManagement.hlsli:61-137).
            for f in fields.iter().filter(|f| {
                f.strip_prefix("linearLightingSettings.")
                    .is_some_and(|m| m.ends_with("Mult") || m == "fireEffectCurve")
            }) {
                put(&mut out, f, vec![1.0]);
            }
        }
    } else if fields.contains(OLD_LL) {
        // Keys the 1.9 files lack (the capture has no such fields): a 0 gamma or diffuse/emit multiplier draws black.
        for (k, v) in OLD_LL_DEFAULTS {
            let f = format!("linearLightingSettings.{k}");
            if fields.contains(&f) && ll(k).is_none() {
                put(&mut out, &f, vec![f64::from(v)]);
            }
        }
        // Old LL: raw multipliers, the shader applies π itself (Color.hlsli:270-278). isDirLightLinear and
        // dirLightMult are engine-derived (advanced-skin LinearLighting.cpp:125-133); 0 would black out the sun
        // (1.8 Lighting.hlsl:2730). The flag is the LL switch (Color.hlsli:7).
        put(
            &mut out,
            "linearLightingSettings.pointLightMult",
            vec![ll("pointLightMult").unwrap_or(1.0)],
        );
        put(
            &mut out,
            "linearLightingSettings.enableLinearLighting",
            vec![f64::from(u8::from(ll_on))],
        );
        put(
            &mut out,
            "linearLightingSettings.isDirLightLinear",
            vec![0.0],
        );
        put(&mut out, "linearLightingSettings.dirLightMult", vec![1.0]);
        // b8 LLPerGeometry (Color.hlsli:10-14): 0 turns emissive black (Color.hlsli:283).
        put(&mut out, "emissiveMult", vec![1.0]);
    }
    put(
        &mut out,
        "linearLightingSettings.isMainOrLoadingMenu",
        vec![0.0],
    );
    // New packing iff the build reflects the per-geometry profile (AIO 1.9.0); always packed, so an old build with
    // no skin section still gets the un-multiplied tiling.
    let new_skin = fields.contains("skinPerGeometryProfile.skinParams");
    let empty = Map::new();
    let skin = match s.merged.get("Advanced Skin") {
        Some(Value::Object(m)) => m,
        _ => &empty,
    };
    let tiling = pack_skin(skin, new_skin, &mut out);
    // C2: the PS reads the profile from `SkinPerGeometry` (b7), not FeatureData (Skin.hlsli:21-31). No skin
    // extra / wetness / sk textures in the preview: skinMaterialFlags 0 here is the FeatureData/static default; the per-shape value (RFAOS bound at t71) is written per draw (material.rs); per-geometry wetness 0 (Skin.hlsli:254 returns 0).
    let profile: Vec<(String, Vec<f64>)> = out
        .iter()
        .filter_map(|(n, v)| {
            n.strip_prefix("skinData.")
                .map(|f| (format!("skinPerGeometryProfile.{f}"), v.clone()))
        })
        .collect();
    out.extend(profile);
    if head && new_skin {
        // CS: bodyTiling = 1 on a head (advanced-skin Renderer.cpp:263-264); FeatureData keeps the body value
        // (GetCommonBufferData passes false).
        if let Some((_, v)) = out
            .iter_mut()
            .find(|(n, _)| n == "skinPerGeometryProfile.skinDetailParams")
        {
            v[0] = tiling;
            v[1] = 1.0;
        }
    }
    out.push(("skinPerGeometry".into(), vec![0.0; 4]));
    out.push(("skinMaterialFlags".into(), vec![0.0; 4]));
    out
}

fn put(out: &mut Vec<(String, Vec<f64>)>, name: &str, vals: Vec<f64>) {
    match out.iter_mut().find(|(n, _)| n == name) {
        Some(slot) => slot.1 = vals,
        None => out.push((name.to_string(), vals)),
    }
}

/// `"Advanced Skin"` → `skinData.*` (C5). The profile is `DefaultProfile`, else the section itself (CS's own
/// migration, advanced-skin Skin.cpp:22: 1.6/1.8 files are flat with the same key names). `new` (the build reflects
/// `skinPerGeometryProfile`): advanced-skin Renderer.cpp:260-268, the CPU pre-multiplies the tiling. Otherwise dev
/// Skin.cpp:267-273: the shader multiplies x·y itself (1.8 Lighting.hlsl:2049). Returns SkinDetailTiling (for the
/// head copy). A key the files lack keeps the capture-11178 component.
fn pack_skin(skin: &Map<String, Value>, new: bool, out: &mut [(String, Vec<f64>)]) -> f64 {
    let profile = match skin.get("DefaultProfile") {
        Some(Value::Object(d)) => d,
        _ => skin,
    };
    let p = |k: &str| profile.get(k).and_then(num);
    let top = |k: &str| skin.get(k).and_then(num);
    // Capture 11178: x = 20 = 10 × 2.
    let tiling = p("SkinDetailTiling").unwrap_or(10.0);
    let body = p("BodyTilingMultiplier").unwrap_or(2.0);
    let (detail_x, detail_on, sss_amount, wet_response) = if new {
        (
            tiling * body,
            p("EnableSkinDetail"),
            p("SSSAmount"),
            p("WetResponse"),
        )
    } else {
        // A missing EnableSkin falls back to the capture's skinParams.w before the AND.
        let skin_on = top("EnableSkin").or_else(|| {
            out.iter()
                .find(|(n, _)| n == "skinData.skinParams")
                .map(|(_, v)| v[3])
        });
        let on = p("EnableSkinDetail")
            .zip(skin_on)
            .map(|(d, e)| f64::from(u8::from(d != 0.0 && e != 0.0)));
        (tiling, on, Some(0.0), Some(0.0))
    };
    let packed = [
        // x roughness, y secondary roughness (:1969-1970), z × glossiness (:1969), w skin on (:1365).
        (
            "skinParams",
            [
                p("SkinMainRoughness"),
                p("SkinSecondRoughness"),
                p("SkinSpecularTexMultiplier"),
                top("EnableSkin"),
            ],
        ),
        // x secondary specular (:1977), y extra wetness (:1666, :2190), z F0 (:1979), w base colour × (:1555).
        (
            "skinParams2",
            [
                p("SecondarySpecularStrength"),
                top("ExtraSkinWetness"),
                p("F0"),
                p("BaseColorMultiplier"),
            ],
        ),
        // x detail UV tiling (new: × body, :1673; old: the shader × y), y body tiling, z detail strength (:1688),
        // w detail on (:1682).
        (
            "skinDetailParams",
            [
                Some(detail_x),
                Some(body),
                p("SkinDetailStrength"),
                detail_on,
            ],
        ),
        // x·w translucency (Skin.hlsli:166), y width (Skin.hlsli:162), z amount (:3159; new only, old 0).
        (
            "sssParams",
            [p("Translucency"), p("sssWidth"), sss_amount, p("UseSSS")],
        ),
        // x weight (:1982), y roughness (:1983), z colour (:1984), w edge roughness (:1968).
        (
            "fuzzParams",
            [
                p("FuzzStrength"),
                p("FuzzRoughness"),
                p("FuzzF0"),
                p("ExtraEdgeRoughness"),
            ],
        ),
        // x, y roughness multipliers (:1972-1973), z specular with a skin-extra map (:1979), w wet response
        // (new only, :2216; old 0).
        (
            "physicalParams",
            [
                p("PhysicalMainRoughnessMultiplier"),
                p("PhysicalSecondRoughnessMultiplier"),
                p("PhysicalSpecularStrength"),
                wet_response,
            ],
        ),
    ];
    for (field, comps) in packed {
        let name = format!("skinData.{field}");
        if let Some((_, v)) = out.iter_mut().find(|(n, _)| *n == name) {
            for (dst, c) in v.iter_mut().zip(comps) {
                if let Some(c) = c {
                    *dst = c;
                }
            }
        }
    }
    if let Some(Value::Array(a)) = skin.get("WetParams") {
        let w: Vec<f64> = a.iter().filter_map(num).collect();
        if w.len() == 4 {
            if let Some(slot) = out.iter_mut().find(|(n, _)| n == "skinData.wetParams") {
                slot.1 = w;
            }
        }
    }
    tiling
}

/// Writes `values` by name, each as its reflected type (C6): float (and unknown) fields as f32, uint and bool
/// fields as u32, int fields as i32 bits. Names this build's layout lacks are skipped.
pub fn write_feature_data(w: &mut CbWriter, values: &[(String, Vec<f64>)]) {
    for (name, v) in values {
        match w.kind(name) {
            None => {}
            Some(VarKind::Float | VarKind::Other) => {
                let f: Vec<f32> = v.iter().map(|x| *x as f32).collect();
                w.set(name, &f);
            }
            Some(VarKind::Uint | VarKind::Bool) => {
                let u: Vec<u32> = v.iter().map(|x| *x as u32).collect();
                w.set_u32(name, &u);
            }
            Some(VarKind::Int) => {
                let u: Vec<u32> = v.iter().map(|x| *x as i32 as u32).collect();
                w.set_u32(name, &u);
            }
        }
    }
}
