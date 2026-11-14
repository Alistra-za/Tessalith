// crates/nif/src/pbr2.rs
//! CS TruePBR 2.0 (shader type 21): the 25 PBR parameters of the payload (spec section 4, in file order).
use crate::props::{LightingParams, LightingTrailer};

/// What the payload's PBR Version word must be (spec section 2).
pub const VERSION: u32 = 2;
/// A type-21 body is this many bytes longer than a vanilla one (180 against 100).
pub const BLOCK_EXTRA: usize = 80;
/// Number of PBR parameters in the payload.
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
}

/// SLSF1 bits that must be off on a type-21 block (spec section 5): 7, 10, 11, 14, 17, 18, 21, 28.
pub const BANNED_SLSF1: u32 =
    1 << 7 | 1 << 10 | 1 << 11 | 1 << 14 | 1 << 17 | 1 << 18 | 1 << 21 | 1 << 28;
/// SLSF2 bits that must be off on a type-21 block (spec section 5): 1, 6, 9, 20, 24.
pub const BANNED_SLSF2: u32 = 1 << 1 | 1 << 6 | 1 << 9 | 1 << 20 | 1 << 24;
/// The v1 PBR flags (SLSF2 12, 23, 25, 26, 27, 30). CS 2.0 doesn't use them; a clean v2 shape has them off.
pub const V1_SLSF2: u32 = 1 << 12 | 1 << 23 | 1 << 25 | 1 << 26 | 1 << 27 | 1 << 30;

/// Float controller variables CS 2.0 acts on (spec section 8): refraction, emissive multiple, alpha, UV.
pub const FLOAT_VARS: [u32; 7] = [0, 11, 12, 20, 21, 22, 23];
/// Float controller variables CS 2.0 runs but shows no change for (glossiness, specular strength).
pub const NO_EFFECT_FLOAT_VARS: [u32; 2] = [9, 10];
/// Colour controller variables CS 2.0 acts on: emissive colour.
pub const COLOR_VARS: [u32; 1] = [1];
/// Colour controller variables CS 2.0 runs but shows no change for (specular colour).
pub const NO_EFFECT_COLOR_VARS: [u32; 1] = [0];

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

/// Name of each feature bit 0-9, in bit order.
pub const FEATURE_NAMES: [&str; 10] = [
    "Subsurface",
    "Two-layer coat",
    "Coloured coat",
    "Interlayer parallax",
    "Coat normal",
    "Fuzz",
    "Hair",
    "Glint",
    "Foliage",
    "Thin surface",
];

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
        (features >> 10 != 0, BITS_ABOVE_9),
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

/// The two optional NiFloatsExtraData blocks of a type-21 shader (spec section 7), found by name on the property's
/// extra-data list. The floats stay as written: a wrong count is the validator's to report (Note ID2).
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pbr2Extra {
    /// `PBRThinSurface`: expected 4 floats (subsurface balance, front and back specular weight, back roughness scale).
    pub thin: Option<Vec<f32>>,
    /// `PBRFuzz`: expected 2 floats (fuzz roughness, green-channel switch).
    pub fuzz: Option<Vec<f32>>,
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
                    v[17..=20].copy_from_slice(&[t[0], 40.0 - t[1], t[2], t[3]]);
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

/// What [`to_v1`] makes of a v2 shape: the SLSF2 bits to set, the lighting fields, and the type-11 trailer floats
/// (`None`: type 0, no trailer).
#[derive(Debug, Clone, PartialEq)]
pub struct ToV1 {
    pub slsf2_set: u32,
    pub glossiness: f32,
    pub specular_color: [f32; 3],
    pub specular_strength: f32,
    pub lighting_effect_1: f32,
    pub lighting_effect_2: f32,
    pub mlp: Option<[f32; 4]>,
    /// Set features the v1 shape can't carry, by [`FEATURE_NAMES`], in bit order.
    pub dropped: Vec<&'static str>,
}

/// The v1 PBR shape that stands for v2 `features` and `params`, by the same table in reverse. An approximation for
/// tree A's preview (design, settled item 5). Every set feature the chosen v1 form can't carry goes to `dropped`:
/// thin surface and foliage always, subsurface, fuzz and glint under two-layer, anything but hair under hair, and
/// glint beside fuzz (fuzz wins the trailer, as in `from_v1`).
pub fn to_v1(features: u32, params: &[f32; PARAM_COUNT]) -> ToV1 {
    use feat::*;
    let (has, p) = (|m: u32| features & m != 0, params);
    let mut r = ToV1 {
        slsf2_set: SLSF2_PBR,
        glossiness: p[1],
        specular_color: [1.0; 3],
        specular_strength: p[0],
        lighting_effect_1: 0.0,
        lighting_effect_2: p[2],
        mlp: None,
        dropped: Vec::new(),
    };
    if has(TWO_LAYER) {
        r.slsf2_set |= SLSF2_MLP;
        for (bit, flag) in [
            (INTERLAYER_PARALLAX, SLSF2_SOFT),
            (COAT_NORMAL, SLSF2_BACK),
            (COLORED_COAT, SLSF2_COLORED),
        ] {
            if has(bit) {
                r.slsf2_set |= flag;
            }
        }
        r.specular_color = [p[7], p[8], p[9]];
        r.lighting_effect_1 = p[10];
        r.mlp = Some([p[11], p[12], 0.0, 0.0]);
    } else if has(HAIR) {
        r.slsf2_set |= SLSF2_BACK;
    } else {
        if has(SUBSURFACE) {
            r.slsf2_set |= SLSF2_SUBSURFACE;
            r.specular_color = [p[3], p[4], p[5]];
            r.lighting_effect_1 = p[6];
        }
        if has(FUZZ) {
            r.slsf2_set |= SLSF2_SOFT;
            r.mlp = Some([p[13], p[14], p[15], p[16]]);
        } else if has(GLINT) {
            r.slsf2_set |= SLSF2_FIT_SLOPE;
            r.mlp = Some([p[17], 40.0 - p[18], p[19], p[20]]);
        }
    }
    let kept = if has(TWO_LAYER) {
        TWO_LAYER | INTERLAYER_PARALLAX | COAT_NORMAL | COLORED_COAT
    } else if has(HAIR) {
        HAIR
    } else {
        SUBSURFACE | if has(FUZZ) { FUZZ } else { GLINT }
    };
    r.dropped = FEATURE_NAMES
        .iter()
        .enumerate()
        .filter(|&(bit, _)| features & !kept & (1 << bit) != 0)
        .map(|(_, &name)| name)
        .collect();
    r
}
