//! The CS 2.0 rules (spec sections 3 and 4) as pure functions.
use nif::pbr2::{
    allowed_with, clamp_param, feat::*, feature_errors, param_error, BITS_ABOVE_9,
    COAT_NEEDS_TWO_LAYER, FUZZ_GLINT, HAIR_ALONE, PARAM_COUNT, PARAM_RANGE, PBR2_DEFAULTS,
    TWO_LAYER_EXCLUDES,
};

#[test]
fn clean_feature_sets_have_no_error() {
    for f in [
        0,
        SUBSURFACE,
        HAIR,
        TWO_LAYER | COLORED_COAT | INTERLAYER_PARALLAX | COAT_NORMAL,
        FUZZ | THIN_SURFACE,
        FOLIAGE | THIN_SURFACE | GLINT,
    ] {
        assert!(feature_errors(f).is_empty(), "{f:#x}");
    }
}

#[test]
fn hair_must_be_alone() {
    assert_eq!(feature_errors(HAIR | SUBSURFACE), [HAIR_ALONE]);
    assert_eq!(
        feature_errors(HAIR | GLINT | FUZZ | THIN_SURFACE),
        [HAIR_ALONE, FUZZ_GLINT]
    ); // + fuzz excludes glint
}

#[test]
fn coat_children_need_two_layer() {
    for f in [COLORED_COAT, INTERLAYER_PARALLAX, COAT_NORMAL] {
        assert_eq!(feature_errors(f), [COAT_NEEDS_TWO_LAYER], "{f:#x}");
        assert!(feature_errors(f | TWO_LAYER).is_empty());
    }
    // all three without two-layer is still the one rule
    assert_eq!(
        feature_errors(COLORED_COAT | COAT_NORMAL),
        [COAT_NEEDS_TWO_LAYER]
    );
}

#[test]
fn two_layer_excludes_sss_fuzz_glint_foliage_thin() {
    for f in [SUBSURFACE, FUZZ, GLINT, FOLIAGE, THIN_SURFACE] {
        assert_eq!(
            feature_errors(TWO_LAYER | f),
            [TWO_LAYER_EXCLUDES],
            "{f:#x}"
        );
    }
}

#[test]
fn fuzz_excludes_glint() {
    assert_eq!(feature_errors(FUZZ | GLINT), [FUZZ_GLINT]);
}

#[test]
fn bits_above_9_invalid() {
    assert_eq!(feature_errors(1 << 10), [BITS_ABOVE_9]);
    assert_eq!(feature_errors(0xFFFF_FC00), [BITS_ABOVE_9]);
}

#[test]
fn allowed_with_follows_the_rules() {
    assert!(allowed_with(0, SUBSURFACE));
    assert!(!allowed_with(0, COAT_NORMAL)); // needs two-layer first
    assert!(allowed_with(TWO_LAYER, COAT_NORMAL));
    assert!(!allowed_with(TWO_LAYER, FUZZ));
    assert!(!allowed_with(SUBSURFACE, HAIR));
    assert!(!allowed_with(FUZZ, GLINT));
    // a bit already on stays allowed
    assert!(allowed_with(FUZZ, FUZZ));
}

#[test]
fn defaults_are_in_range() {
    for (i, &d) in PBR2_DEFAULTS.iter().enumerate() {
        assert_eq!(param_error(i, d), None, "{i}");
    }
    assert_eq!(PARAM_RANGE.len(), PARAM_COUNT);
}

#[test]
fn param_bounds() {
    assert!(param_error(18, 0.5).is_some());
    assert!(param_error(19, 0.4).is_some());
    assert_eq!(param_error(19, 0.005), None);
    assert!(param_error(17, 0.5).is_some());
    assert!(param_error(0, -0.1).is_some());
    assert!(param_error(0, f32::NAN).is_some());
    assert!(param_error(0, f32::INFINITY).is_some());
    assert!(param_error(2, 1e9).is_none()); // no upper bound
    assert!(param_error(24, 3.1).is_some());
    assert!(param_error(6, 1.1).is_some());
}

#[test]
fn clamp_gives_a_value_param_error_accepts() {
    for i in 0..PARAM_COUNT {
        for v in [-5.0, 0.0, 0.5, 100.0, f32::NAN, f32::INFINITY] {
            assert_eq!(param_error(i, clamp_param(i, v)), None, "{i} {v}");
        }
    }
}

// --- v1 <-> v2 conversion (Task 5) ---
#[path = "common/v1_fixtures.rs"]
mod v1_fixtures;
use nif::pbr2::{from_v1, to_v1, ToV1};
use nif::props::LightingParams;
use v1_fixtures::{fixture, v1, PBR};

/// `from_v1` on the v1 fixture `name`.
fn from_fixture(name: &str) -> nif::pbr2::ToV2 {
    let (t, f2, p) = fixture(name);
    from_v1(t, f2, &p)
}

#[test]
fn plain_v1_maps_to_defaults_plus_three() {
    let r = from_fixture("plain");
    let mut want = PBR2_DEFAULTS;
    (want[0], want[1], want[2]) = (0.8, 0.05, 0.5);
    assert_eq!((r.features, r.params), (0, want));
    assert!(r.clamped.is_empty());
}

#[test]
fn two_layer_type_11_maps_coat() {
    let r = from_fixture("two_layer_11");
    assert_eq!(
        r.features,
        TWO_LAYER | INTERLAYER_PARALLAX | COAT_NORMAL | COLORED_COAT
    );
    assert_eq!(r.params[7..=10], [0.1, 0.2, 0.3, 0.6]);
    assert_eq!(r.params[11..=12], [0.3, 0.06]);
    // type 0 has no trailer: the coat roughness and level stay default
    let r = from_fixture("two_layer_0");
    assert_eq!(r.features, TWO_LAYER);
    assert_eq!(r.params[11..=12], PBR2_DEFAULTS[11..=12]);
    // 24 wins over 26 and 12
    let r = from_fixture("two_layer_wins");
    assert_eq!(r.features, TWO_LAYER);
}

#[test]
fn subsurface_maps_colour_and_opacity() {
    let r = from_fixture("subsurface");
    assert_eq!(r.features, SUBSURFACE);
    assert_eq!(r.params[3..=6], [0.9, 0.5, 0.4, 0.35]);
    assert_eq!(r.params[7..=10], PBR2_DEFAULTS[7..=10]);
}

#[test]
fn fuzz_from_trailer() {
    let r = from_fixture("fuzz");
    assert_eq!(r.features, FUZZ);
    assert_eq!(r.params[13..=16], [0.2, 0.4, 0.6, 0.8]);
    // 25 wins over 12, and subsurface can sit beside it
    let r = from_fixture("fuzz_subsurface");
    assert_eq!(r.features, FUZZ | SUBSURFACE);
    assert_eq!(r.params[3..=6], [0.5; 4]);
}

#[test]
/// Note ID37: v1 trailer[1] is the log microfacet density itself (CS 1.x BSLightingShaderMaterialPBR.cpp:305), and so
/// is p18 (spec section 4, default 40); only the shader packing takes 40 - it.
fn glint_keeps_log_density() {
    let r = from_fixture("glint");
    assert_eq!(r.features, GLINT);
    assert_eq!(r.params[17..=20], [1.5, 10.0, 0.02, 3.0]);
    let mut p = PBR2_DEFAULTS;
    p[17..=20].copy_from_slice(&[1.5, 10.0, 0.02, 3.0]);
    assert_eq!(to_v1(GLINT, &p).mlp, Some([1.5, 10.0, 0.02, 3.0]));
}

#[test]
fn hair_alone() {
    let r = from_fixture("hair");
    assert_eq!(r.features, HAIR);
    assert_eq!(r.params[3..], PBR2_DEFAULTS[3..]);
}

#[test]
fn out_of_range_is_clamped_and_reported() {
    let r = from_v1(0, PBR, &v1(1.0, 2.0, [1.0; 3], 0.0, 1.0, None));
    assert_eq!(r.params[1], 1.0);
    assert_eq!(r.clamped, [(1, 2.0, 1.0)]);
    // a glint trailer of zeros (scale 0 and log density 0 are below 1, roughness 0 below 0.005)
    let r = from_v1(
        11,
        PBR | 1 << 12,
        &v1(1.0, 0.04, [1.0; 3], 0.0, 1.0, Some([0.0; 4])),
    );
    assert_eq!(r.params[17..=20], [1.0, 1.0, 0.005, 0.0]);
    assert_eq!(
        r.clamped,
        [(17, 0.0, 1.0), (18, 0.0, 1.0), (19, 0.0, 0.005)]
    );
    // a NaN takes the default
    let r = from_v1(0, PBR, &v1(f32::NAN, 0.04, [1.0; 3], 0.0, 1.0, None));
    assert_eq!(r.params[0], PBR2_DEFAULTS[0]);
    assert_eq!(r.clamped.len(), 1);
}

fn params_for(features: u32) -> [f32; PARAM_COUNT] {
    let mut p = PBR2_DEFAULTS;
    (p[0], p[1], p[2]) = (0.7, 0.2, 1.3);
    for (bit, rows) in [
        (TWO_LAYER, 7..=12),
        (SUBSURFACE, 3..=6),
        (FUZZ, 13..=16),
        (GLINT, 17..=20),
    ] {
        if features & bit != 0 {
            for i in rows {
                p[i] = 0.1 + 0.05 * i as f32;
            }
        }
    }
    if features & GLINT != 0 {
        (p[17], p[18], p[19]) = (1.5, 25.0, 0.1); // inside the glint ranges
    }
    p
}
fn lp(r: &ToV1) -> LightingParams {
    v1(
        r.specular_strength,
        r.glossiness,
        r.specular_color,
        r.lighting_effect_1,
        r.lighting_effect_2,
        r.mlp,
    )
}

#[test]
fn to_v1_round_trips_reversible_subset() {
    for f in [
        0,
        SUBSURFACE,
        FUZZ,
        GLINT,
        FUZZ | SUBSURFACE,
        GLINT | SUBSURFACE,
        HAIR,
        TWO_LAYER,
        TWO_LAYER | COLORED_COAT,
        TWO_LAYER | INTERLAYER_PARALLAX | COAT_NORMAL | COLORED_COAT,
    ] {
        let p = params_for(f);
        let r = to_v1(f, &p);
        assert!(r.dropped.is_empty());
        assert_eq!(r.slsf2_set & PBR, PBR);
        let back = from_v1(if r.mlp.is_some() { 11 } else { 0 }, r.slsf2_set, &lp(&r));
        assert_eq!((back.features, back.params), (f, p), "{f:#x}");
        assert!(back.clamped.is_empty());
    }
}

#[test]
fn to_v1_drops_thin_and_foliage() {
    let r = to_v1(THIN_SURFACE | FOLIAGE | SUBSURFACE, &PBR2_DEFAULTS);
    assert_eq!(r.dropped, ["Foliage", "Thin surface"]);
    assert_eq!(r.slsf2_set, PBR | 1 << 26);
    let r = to_v1(THIN_SURFACE, &PBR2_DEFAULTS);
    assert_eq!(
        (r.dropped.as_slice(), r.slsf2_set),
        (&["Thin surface"][..], PBR)
    );
}

#[test]
fn to_v1_names_every_feature_its_form_cannot_carry() {
    for (f, want) in [
        (TWO_LAYER | SUBSURFACE, &["Subsurface"][..]),
        (TWO_LAYER | FUZZ, &["Fuzz"]),
        (TWO_LAYER | GLINT | SUBSURFACE, &["Subsurface", "Glint"]),
        (HAIR | SUBSURFACE | FUZZ, &["Subsurface", "Fuzz"]),
        (HAIR | TWO_LAYER, &["Hair"]),
        (FUZZ | GLINT, &["Glint"]),
        (FUZZ | GLINT | SUBSURFACE, &["Glint"]),
        (COLORED_COAT | HAIR, &["Coloured coat"]),
        (
            TWO_LAYER | COAT_NORMAL | FOLIAGE | THIN_SURFACE,
            &["Foliage", "Thin surface"],
        ),
    ] {
        let p = to_v1(f, &PBR2_DEFAULTS);
        assert_eq!(p.dropped, want, "{f:#x}");
    }
}

#[test]
fn v1_sub_flags_without_the_pbr_bit_give_no_features() {
    let r = from_fixture("no_pbr_bit");
    assert_eq!(r.features, 0);
    assert_eq!(r.params[3..], PBR2_DEFAULTS[3..]);
}

/// Note ID40: the v1 body of a type-21 shape, which `set_shader_type`'s from-21 conversion writes.
#[test]
fn v1_body_is_the_from_21_conversion() {
    use nif::pbr2::{v1_body, V1_SLSF2};
    let mut p = PBR2_DEFAULTS;
    p[17..=20].copy_from_slice(&[1.5, 10.0, 0.02, 3.0]);
    let slsf2 = V1_SLSF2 | 1 << 24 | 1 << 4;
    let v = to_v1(GLINT, &p);
    let b = v1_body(slsf2, GLINT, &p, 11);
    assert_eq!(b.slsf2, 1 << 4 | v.slsf2_set);
    assert_eq!(
        b.trailer,
        [1.5, 10.0, 0.02, 3.0, nif::edit::trailer_defaults(11)[4]]
    );
    assert_eq!(
        (b.glossiness, b.specular_strength, b.specular_color),
        (v.glossiness, v.specular_strength, v.specular_color)
    );
    assert_eq!(
        (b.lighting_effect_1, b.lighting_effect_2),
        (v.lighting_effect_1, v.lighting_effect_2)
    );
    assert!(b.dropped.is_empty());
    // type 0 keeps only the flags it shares with the type-11 form and names what the trailer carried
    let b0 = v1_body(slsf2, GLINT | SUBSURFACE, &p, 0);
    assert_eq!(b0.slsf2, 1 << 4 | PBR | 1 << 26);
    assert!(b0.trailer.is_empty());
    assert_eq!(b0.dropped, ["Glint"]);
    assert_eq!(v1_body(slsf2, SUBSURFACE, &p, 0).trailer, Vec::<f32>::new());
}

#[test]
fn negative_zero_is_not_a_clamp() {
    let r = from_v1(0, PBR, &v1(-0.0, 0.04, [1.0; 3], 0.0, 1.0, None));
    assert!(r.clamped.is_empty());
}
