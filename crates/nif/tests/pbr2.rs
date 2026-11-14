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
