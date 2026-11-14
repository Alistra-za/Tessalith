// crates/nif/src/pbr2.rs
//! CS TruePBR 2.0 (shader type 21): the 25 PBR parameters of the payload (spec section 4, in file order).

/// Default of each of the 25 parameters.
pub const PBR2_DEFAULTS: [f32; 25] = [
    1.0, 0.04, 1.0, // roughness scale, specular level, displacement scale
    1.0, 1.0, 1.0, 0.0, // subsurface colour, opacity
    1.0, 1.0, 1.0, 1.0, 1.0, 0.04, // coat colour, strength, roughness, specular level
    1.0, 1.0, 1.0, 0.0, // fuzz colour, weight
    1.5, 40.0, 0.015, 2.0,  // glint scale, log density, roughness, randomization
    0.75, // subsurface balance
    0.5, 0.5, 1.0, // thin surface: front, back specular weight, back roughness scale
];

/// Name of each of the 25 parameters, in file order.
pub const PARAM_NAMES: [&str; 25] = [
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
