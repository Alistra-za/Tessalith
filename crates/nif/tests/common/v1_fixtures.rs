// crates/nif/tests/common/v1_fixtures.rs
//! v1 PBR shapes for the v1 to v2 tests (`tests/pbr2.rs`) and the `.csm` value bridge (`tests/csm.rs`).
#![allow(dead_code)]
use nif::props::{LightingParams, LightingTrailer};

/// SLSF2 23: the v1 PBR flag.
pub const PBR: u32 = 1 << 23;

/// A v1 lighting block: specular strength, glossiness, specular colour, lighting effects 1 and 2, and the type-11
/// trailer's first four floats (`None`: no trailer).
pub fn v1(
    strength: f32,
    gloss: f32,
    colour: [f32; 3],
    le1: f32,
    le2: f32,
    t: Option<[f32; 4]>,
) -> LightingParams {
    LightingParams {
        uv_offset: [0.0; 2],
        uv_scale: [1.0; 2],
        emissive_color: [0.0; 3],
        emissive_multiple: 1.0,
        texture_clamp_mode: 3,
        alpha: 1.0,
        refraction_strength: 0.0,
        glossiness: gloss,
        specular_color: colour,
        specular_strength: strength,
        lighting_effect_1: le1,
        lighting_effect_2: le2,
        trailer: match t {
            Some(t) => LightingTrailer::MultiLayer {
                inner_thickness: t[0],
                refraction_scale: t[1],
                inner_texture_scale: [t[2], t[3]],
                envmap_strength: 0.0,
            },
            None => LightingTrailer::None,
        },
        pbr2_extra: None,
    }
}

/// The in-range v1 shapes, by name: (shader type, SLSF2, lighting block). Every value maps without a clamp.
pub fn fixtures() -> Vec<(&'static str, u32, u32, LightingParams)> {
    let fuzz_t = Some([0.2, 0.4, 0.6, 0.8]);
    vec![
        (
            "plain",
            0,
            PBR,
            v1(0.8, 0.05, [0.2, 0.3, 0.4], 0.7, 0.5, None),
        ),
        (
            "two_layer_11",
            11,
            PBR | 1 << 24 | 1 << 25 | 1 << 27 | 1 << 30,
            v1(
                1.0,
                0.04,
                [0.1, 0.2, 0.3],
                0.6,
                1.0,
                Some([0.3, 0.06, 0.0, 0.0]),
            ),
        ),
        (
            "two_layer_0",
            0,
            PBR | 1 << 24,
            v1(1.0, 0.04, [0.1, 0.2, 0.3], 0.6, 1.0, None),
        ),
        (
            "two_layer_wins",
            0,
            PBR | 1 << 24 | 1 << 26 | 1 << 12,
            v1(1.0, 0.04, [1.0; 3], 0.0, 1.0, None),
        ),
        (
            "subsurface",
            0,
            PBR | 1 << 26,
            v1(1.0, 0.04, [0.9, 0.5, 0.4], 0.35, 1.0, None),
        ),
        (
            "fuzz",
            11,
            PBR | 1 << 25,
            v1(1.0, 0.04, [1.0; 3], 0.0, 1.0, fuzz_t),
        ),
        (
            "fuzz_subsurface",
            11,
            PBR | 1 << 25 | 1 << 12 | 1 << 26,
            v1(1.0, 0.04, [0.5; 3], 0.5, 1.0, fuzz_t),
        ),
        (
            "glint",
            11,
            PBR | 1 << 12,
            v1(1.0, 0.04, [1.0; 3], 0.0, 1.0, Some([1.5, 10.0, 0.02, 3.0])),
        ),
        (
            "hair",
            11,
            PBR | 1 << 27 | 1 << 26 | 1 << 25,
            v1(0.5, 0.04, [1.0; 3], 0.0, 1.0, fuzz_t),
        ),
        (
            "no_pbr_bit",
            11,
            1 << 24 | 1 << 25 | 1 << 26 | 1 << 27 | 1 << 12 | 1 << 30,
            v1(0.8, 0.05, [0.2; 3], 0.7, 0.5, Some([0.3; 4])),
        ),
    ]
}

/// The fixture named `name`: (shader type, SLSF2, lighting block).
pub fn fixture(name: &str) -> (u32, u32, LightingParams) {
    let (_, t, f2, p) = fixtures()
        .into_iter()
        .find(|f| f.0 == name)
        .unwrap_or_else(|| panic!("no v1 fixture {name}"));
    (t, f2, p)
}
