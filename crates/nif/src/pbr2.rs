// crates/nif/src/pbr2.rs
//! CS TruePBR 2.0 PBR values: the feature bits and the 25 parameters (the PBR 2.0 spec's section 4 order) a `.csm`
//! material sets, and the bridge from a v1 PBR shape (Notes ID and IE).
use crate::csm::{self, Csm, Value};
use crate::props::{LightingParams, LightingTrailer};
use std::collections::BTreeMap;

/// Number of PBR parameters.
pub const PARAM_COUNT: usize = 25;

/// The PBR feature bits (spec section 3), as masks.
pub mod feat {
    pub const SUBSURFACE: u32 = 1;
    pub const TWO_LAYER: u32 = 2;
    pub const COLORED_COAT: u32 = 4;
    pub const INTERLAYER_PARALLAX: u32 = 8;
    pub const COAT_NORMAL: u32 = 16;
    pub const FUZZ: u32 = 32;
    pub const HAIR: u32 = 64;
    pub const GLINT: u32 = 128;
    pub const FOLIAGE: u32 = 256;
    pub const THIN_SURFACE: u32 = 512;
    /// Not a payload bit: the `.csm` switch `PBRFuzzRoughnessTexture` (fuzz roughness from the displacement
    /// texture's green channel), carried in `PbrValues::features` (Note IE3). `feature_errors` reports it as a bit
    /// above 9, so a payload check masks with [`PAYLOAD_FEATURES`] first.
    pub const FUZZ_ROUGHNESS_TEXTURE: u32 = 1024;
    /// The payload's feature bits 0-9: `features & PAYLOAD_FEATURES` is what `feature_errors` and the payload
    /// writers take from `PbrValues::features`.
    pub const PAYLOAD_FEATURES: u32 = 0x3ff;
}

/// The v1 PBR flags (SLSF2 12, 23, 25, 26, 27, 30). CS 2.0 doesn't use them; a clean v2 shape has them off.
pub const V1_SLSF2: u32 = 1 << 12 | 1 << 23 | 1 << 25 | 1 << 26 | 1 << 27 | 1 << 30;

/// Default of each of the 25 parameters.
pub const PBR2_DEFAULTS: [f32; PARAM_COUNT] = [
    1.0, 0.04, 1.0, // roughness scale, specular level, displacement scale
    1.0, 1.0, 1.0, 0.0, // subsurface colour, opacity
    1.0, 1.0, 1.0, 1.0, 1.0, 0.04, // coat colour, strength, roughness, specular level
    1.0, 1.0, 1.0, 0.0, // fuzz colour, weight
    1.5, 40.0, 0.015, 2.0,  // glint scale, log density, roughness, randomization
    0.75, // subsurface balance
    0.5, 0.5, 1.0, // thin surface: front, back specular weight, back roughness scale
];

/// The first parameter of each colour (three parameters, R G B).
pub const COLOR_PARAMS: [usize; 3] = [3, 7, 13];

/// Name of each of the 25 parameters, in file order.
pub const PARAM_NAMES: [&str; PARAM_COUNT] = [
    "Roughness scale",
    "Specular level",
    "Displacement scale",
    "Subsurface colour R",
    "Subsurface colour G",
    "Subsurface colour B",
    "Subsurface opacity",
    "Coat colour R",
    "Coat colour G",
    "Coat colour B",
    "Coat strength",
    "Coat roughness",
    "Coat specular level",
    "Fuzz colour R",
    "Fuzz colour G",
    "Fuzz colour B",
    "Fuzz weight",
    "Glint screen-space scale",
    "Glint log microfacet density",
    "Glint microfacet roughness",
    "Glint density randomization",
    "Subsurface balance",
    "Thin-surface front specular weight",
    "Thin-surface back specular weight",
    "Thin-surface back roughness scale",
];

const INF: f32 = f32::INFINITY;

/// (min, max) of each parameter (spec section 4); every value is also finite and at least 0. `max` is infinity when
/// the spec gives none.
pub const PARAM_RANGE: [(f32, f32); PARAM_COUNT] = [
    (0.0, INF), // 0 roughness scale
    (0.0, 1.0), // 1 specular level
    (0.0, INF), // 2 displacement scale
    (0.0, INF), // 3-5 subsurface colour
    (0.0, INF),
    (0.0, INF),
    (0.0, 1.0), // 6 subsurface opacity
    (0.0, INF), // 7-9 coat colour
    (0.0, INF),
    (0.0, INF),
    (0.0, 1.0), // 10 coat strength
    (0.0, 1.0), // 11 coat roughness
    (0.0, 1.0), // 12 coat specular level
    (0.0, INF), // 13-15 fuzz colour
    (0.0, INF),
    (0.0, INF),
    (0.0, 1.0),   // 16 fuzz weight
    (1.0, INF),   // 17 glint screen-space scale
    (1.0, 40.0),  // 18 glint log density
    (0.005, 0.3), // 19 glint roughness
    (0.0, 5.0),   // 20 glint randomization
    (0.0, 1.0),   // 21 subsurface balance
    (0.0, 1.0),   // 22 thin front specular weight
    (0.0, 1.0),   // 23 thin back specular weight
    (0.0, 3.0),   // 24 thin back roughness scale
];

/// Why parameter `i` can't be `v`: not finite, negative, or outside [`PARAM_RANGE`]. `None` when it is fine.
pub fn param_error(i: usize, v: f32) -> Option<String> {
    let (name, &(lo, hi)) = (PARAM_NAMES.get(i)?, PARAM_RANGE.get(i)?);
    if !v.is_finite() {
        Some(format!("{name} {v} is not a finite number"))
    } else if v < 0.0 || v < lo || v > hi {
        let range = if hi.is_finite() {
            format!("{lo}-{hi}")
        } else {
            format!("at least {lo}")
        };
        Some(format!("{name} {v} is outside {range}"))
    } else {
        None
    }
}

/// `v` brought into parameter `i`'s range; the default when it is not finite. The value a fix writes.
pub fn clamp_param(i: usize, v: f32) -> f32 {
    match (PARAM_RANGE.get(i), PBR2_DEFAULTS.get(i)) {
        (Some(&(lo, hi)), Some(&d)) => {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                d
            }
        }
        _ => v,
    }
}

pub const BITS_ABOVE_9: &str = "feature bits above 9 are invalid";
pub const HAIR_ALONE: &str = "hair must be the only feature";
pub const COAT_NEEDS_TWO_LAYER: &str =
    "colored coat, interlayer parallax and coat normal need two-layer coat";
pub const TWO_LAYER_EXCLUDES: &str =
    "two-layer coat can't be combined with subsurface, fuzz, glint, foliage or thin surface";
pub const FUZZ_GLINT: &str = "fuzz and glint exclude each other";

/// The spec section 3 rules `features` breaks, one text per broken rule.
pub fn feature_errors(features: u32) -> Vec<&'static str> {
    use feat::*;
    let has = |m: u32| features & m != 0;
    let rules = [
        (features & !PAYLOAD_FEATURES != 0, BITS_ABOVE_9),
        (has(HAIR) && features != HAIR, HAIR_ALONE),
        (
            has(COLORED_COAT | INTERLAYER_PARALLAX | COAT_NORMAL) && !has(TWO_LAYER),
            COAT_NEEDS_TWO_LAYER,
        ),
        (
            has(TWO_LAYER) && has(SUBSURFACE | FUZZ | GLINT | FOLIAGE | THIN_SURFACE),
            TWO_LAYER_EXCLUDES,
        ),
        (has(FUZZ) && has(GLINT), FUZZ_GLINT),
    ];
    rules
        .into_iter()
        .filter_map(|(broken, text)| broken.then_some(text))
        .collect()
}

/// Can feature mask `bit` be switched on with `features` already set: it breaks no rule that `features` doesn't
/// already break. The GUI greys a checkbox when this is false.
pub fn allowed_with(features: u32, bit: u32) -> bool {
    let now = feature_errors(features);
    feature_errors(features | bit)
        .iter()
        .all(|e| now.contains(e))
}

/// What [`from_v1`] makes of a v1 shape.
#[derive(Debug, Clone, PartialEq)]
pub struct ToV2 {
    /// The feature bits (`feat`) CS's load-time remap of the v1 flags gives.
    pub features: u32,
    pub params: [f32; PARAM_COUNT],
    /// Each value that was outside its range, as `(parameter, from, to)`.
    pub clamped: Vec<(usize, f32, f32)>,
}

// The SLSF2 bits of the v1 PBR remap, as `render3d::pbr_flags` reads them; all are in `V1_SLSF2` except 24.
const SLSF2_FIT_SLOPE: u32 = 1 << 12;
const SLSF2_PBR: u32 = 1 << 23;
const SLSF2_MLP: u32 = 1 << 24;
const SLSF2_SOFT: u32 = 1 << 25;
const SLSF2_SUBSURFACE: u32 = 1 << 26;
const SLSF2_BACK: u32 = 1 << 27;
const SLSF2_COLORED: u32 = 1 << 30;
// They can't drift from the v1 flag mask: all but 24 are in it, and 24 is the one it leaves out.
const _: () = assert!(
    (SLSF2_FIT_SLOPE | SLSF2_PBR | SLSF2_SOFT | SLSF2_SUBSURFACE | SLSF2_BACK | SLSF2_COLORED)
        & !V1_SLSF2
        == 0
        && V1_SLSF2 & SLSF2_MLP == 0
);

/// The v2 features and parameters a v1 PBR shape means (design: v1 to v2 conversion table). The flag precedence is
/// `render3d::pbr_flags`'s: two-layer (SLSF2 24), else hair (27), else subsurface (26) with fuzz (25) or glint (12).
/// A shape without SLSF2 23 is not PBR: no features, and the shared three parameters only. The type-11 trailer
/// feeds the coat, fuzz and glint rows; any other shape keeps their defaults.
pub fn from_v1(shader_type: u32, slsf2: u32, p: &LightingParams) -> ToV2 {
    use feat::*;
    let has = |m: u32| slsf2 & m != 0;
    let mut v = PBR2_DEFAULTS;
    let mut features = 0;
    (v[0], v[1], v[2]) = (p.specular_strength, p.glossiness, p.lighting_effect_2);
    let t = match (shader_type, &p.trailer) {
        (
            11,
            LightingTrailer::MultiLayer {
                inner_thickness,
                refraction_scale,
                inner_texture_scale,
                ..
            },
        ) => Some([
            *inner_thickness,
            *refraction_scale,
            inner_texture_scale[0],
            inner_texture_scale[1],
        ]),
        _ => None,
    };
    if has(SLSF2_PBR) {
        let c = p.specular_color;
        if has(SLSF2_MLP) {
            features = TWO_LAYER;
            features |= if has(SLSF2_SOFT) {
                INTERLAYER_PARALLAX
            } else {
                0
            };
            features |= if has(SLSF2_BACK) { COAT_NORMAL } else { 0 };
            features |= if has(SLSF2_COLORED) { COLORED_COAT } else { 0 };
            v[7..=9].copy_from_slice(&c);
            v[10] = p.lighting_effect_1;
            if let Some(t) = t {
                (v[11], v[12]) = (t[0], t[1]);
            }
        } else if has(SLSF2_BACK) {
            features = HAIR;
        } else {
            if has(SLSF2_SUBSURFACE) {
                features |= SUBSURFACE;
                v[3..=5].copy_from_slice(&c);
                v[6] = p.lighting_effect_1;
            }
            if has(SLSF2_SOFT) {
                features |= FUZZ;
                if let Some(t) = t {
                    v[13..=16].copy_from_slice(&t);
                }
            } else if has(SLSF2_FIT_SLOPE) {
                features |= GLINT;
                if let Some(t) = t {
                    // trailer[1] is the log density itself, as p18 is (Note ID37)
                    v[17..=20].copy_from_slice(&t);
                }
            }
        }
    }
    let mut clamped = Vec::new();
    for (i, x) in v.iter_mut().enumerate() {
        let to = clamp_param(i, *x);
        if to != *x && !(x.is_nan() && to.is_nan()) {
            clamped.push((i, *x, to));
            *x = to;
        }
    }
    ToV2 {
        features,
        params: v,
        clamped,
    }
}

/// Note IE3: the CS `PBRTextureSets` JSON key of each parameter (`glintParameters.*` nested), by its first index; a
/// colour key fills three ([`COLOR_PARAMS`]).
pub const TXST_PARAMS: &[(&str, usize)] = &[
    ("roughnessScale", 0),
    ("specularLevel", 1),
    ("displacementScale", 2),
    ("subsurfaceColor", 3),
    ("subsurfaceOpacity", 6),
    ("coatColor", 7),
    ("coatStrength", 10),
    ("coatRoughness", 11),
    ("coatSpecularLevel", 12),
    ("fuzzColor", 13),
    ("fuzzWeight", 16),
    ("glintParameters.screenSpaceScale", 17),
    ("glintParameters.logMicrofacetDensity", 18),
    ("glintParameters.microfacetRoughness", 19),
    ("glintParameters.densityRandomization", 20),
    ("subsurfaceBalance", 21),
    ("thinSurfaceSpecularWeight", 22),
    ("thinSurfaceBackSpecularWeight", 23),
    ("thinSurfaceBackRoughnessScale", 24),
];

/// The PBR values of a `.csm` material (Note IE3): the feature bits (`feat`, with `FUZZ_ROUGHNESS_TEXTURE`), the 25
/// parameters, and the fuzz roughness (-1: use the base roughness).
#[derive(Debug, Clone, PartialEq)]
pub struct PbrValues {
    pub features: u32,
    pub params: [f32; PARAM_COUNT],
    pub fuzz_roughness: f32,
}

/// Each `.csm` switch and its feature bit.
pub const CSM_SWITCHES: [(&str, u32); 11] = [
    ("PBRSubsurfaceEnabled", feat::SUBSURFACE),
    ("PBRTwoLayerEnabled", feat::TWO_LAYER),
    ("PBRTwoLayerColoredCoat", feat::COLORED_COAT),
    ("PBRTwoLayerInterlayerParallax", feat::INTERLAYER_PARALLAX),
    ("PBRTwoLayerCoatNormal", feat::COAT_NORMAL),
    ("PBRFuzzEnabled", feat::FUZZ),
    ("PBRHairMarschner", feat::HAIR),
    ("PBRGlintEnabled", feat::GLINT),
    ("PBRFoliageEnabled", feat::FOLIAGE),
    ("PBRThinSurfaceEnabled", feat::THIN_SURFACE),
    ("PBRFuzzRoughnessTexture", feat::FUZZ_ROUGHNESS_TEXTURE),
];

/// Each `.csm` PBR number or colour key and its parameter; a colour fills three ([`COLOR_PARAMS`]).
pub const CSM_PARAMS: [(&str, usize); 19] = [
    ("PBRRoughnessScale", 0),
    ("PBRSpecularLevel", 1),
    ("PBRDisplacementTextureScale", 2),
    ("PBRSubsurfaceColor", 3),
    ("PBRSubsurfaceOpacity", 6),
    ("PBRTwoLayerCoatColor", 7),
    ("PBRTwoLayerCoatStrength", 10),
    ("PBRTwoLayerCoatRoughness", 11),
    ("PBRTwoLayerCoatSpecularLevel", 12),
    ("PBRFuzzColor", 13),
    ("PBRFuzzWeight", 16),
    ("PBRGlintScreenSpaceScale", 17),
    ("PBRGlintLogMicrofacetDensity", 18),
    ("PBRGlintMicrofacetRoughness", 19),
    ("PBRGlintDensityRandomization", 20),
    ("PBRThinSurfaceSubsurfaceBalance", 21),
    ("PBRThinSurfaceSpecularWeight", 22),
    ("PBRThinSurfaceBackSpecularWeight", 23),
    ("PBRThinSurfaceBackRoughnessScale", 24),
];

/// The feature whose parameters include `i`; 0 for p0-p2, which every PBR material has.
fn param_owner(i: usize) -> u32 {
    match i {
        3..=6 => feat::SUBSURFACE,
        7..=12 => feat::TWO_LAYER,
        13..=16 => feat::FUZZ,
        17..=20 => feat::GLINT,
        21..=24 => feat::THIN_SURFACE,
        _ => 0,
    }
}

/// The PBR values of a (resolved) `.csm`; `None` unless `PBREnabled` is true. A switch is on when the file holds it
/// true; an absent number takes [`PBR2_DEFAULTS`], an absent `PBRFuzzRoughness` -1.
pub fn pbr_values(c: &Csm) -> Option<PbrValues> {
    if c.values.get("PBREnabled") != Some(&Value::Bool(true)) {
        return None;
    }
    let features = CSM_SWITCHES
        .iter()
        .filter(|(key, _)| c.values.get(key) == Some(&Value::Bool(true)))
        .fold(0, |f, (_, bit)| f | bit);
    let mut params = PBR2_DEFAULTS;
    for (key, i) in CSM_PARAMS {
        match c.values.get(key) {
            Some(Value::Num(n)) => params[i] = *n as f32,
            Some(Value::Color(rgb)) => params[i..i + 3].copy_from_slice(&csm::rgb(*rgb)),
            _ => {}
        }
    }
    let fuzz_roughness = match c.values.get("PBRFuzzRoughness") {
        Some(Value::Num(n)) => *n as f32,
        _ => -1.0,
    };
    Some(PbrValues {
        features,
        params,
        fuzz_roughness,
    })
}

/// The fuzz roughness a `.csm` gets for `v`: inside -1 (base roughness) to 1, and -1 when it is not finite. The
/// counterpart of [`clamp_param`], so a caller reports this clamp as it reports a parameter's.
pub fn clamp_fuzz_roughness(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        -1.0
    }
}

/// The `.csm` that stands for `v`: `PBREnabled`, each switch that is on, p0-p2 and every parameter of a feature that
/// is on. With fuzz on it also holds `PBRFuzzRoughness` and, when set, `PBRFuzzRoughnessTexture`; without fuzz,
/// neither. Parameters are clamped by [`clamp_param`] (a colour to 0-1 by `#rrggbb`) and the fuzz roughness by
/// [`clamp_fuzz_roughness`]; the caller reports clamps. No texture, generic key or root.
pub fn csm_from_values(v: &PbrValues) -> Csm {
    let mut values = BTreeMap::new();
    values.insert("PBREnabled", Value::Bool(true));
    let fuzz = v.features & feat::FUZZ != 0;
    for (key, bit) in CSM_SWITCHES {
        if v.features & bit != 0 && (bit != feat::FUZZ_ROUGHNESS_TEXTURE || fuzz) {
            values.insert(key, Value::Bool(true));
        }
    }
    let p: [f32; PARAM_COUNT] = std::array::from_fn(|i| clamp_param(i, v.params[i]));
    for (key, i) in CSM_PARAMS {
        let owner = param_owner(i);
        if owner != 0 && v.features & owner == 0 {
            continue;
        }
        let value = if COLOR_PARAMS.contains(&i) {
            Value::Color(csm::hex([p[i], p[i + 1], p[i + 2]]))
        } else {
            Value::Num(csm::num(p[i]))
        };
        values.insert(key, value);
    }
    if fuzz {
        let r = clamp_fuzz_roughness(v.fuzz_roughness);
        values.insert("PBRFuzzRoughness", Value::Num(csm::num(r)));
    }
    Csm {
        version: csm::VERSION,
        values,
    }
}

/// [`from_v1`] as [`PbrValues`] (fuzz roughness -1), with its clamps. v1 has no foliage flag, so no foliage bit.
pub fn v1_values(
    shader_type: u32,
    slsf2: u32,
    p: &LightingParams,
) -> (PbrValues, Vec<(usize, f32, f32)>) {
    let t = from_v1(shader_type, slsf2, p);
    let v = PbrValues {
        features: t.features,
        params: t.params,
        fuzz_roughness: -1.0,
    };
    (v, t.clamped)
}
