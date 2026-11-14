// The `params` column holds `serde_json` of `LightingParams`; a v1 shape's JSON must not change when the type grows
// a field for type 21 (Note ID2).
use nif::props::{LightingParams, LightingTrailer};

#[test]
fn v1_params_serialize_unchanged() {
    let p = LightingParams {
        uv_offset: [0.0; 2],
        uv_scale: [1.0; 2],
        emissive_color: [0.0; 3],
        emissive_multiple: 1.0,
        texture_clamp_mode: 3,
        alpha: 1.0,
        refraction_strength: 0.0,
        glossiness: 80.0,
        specular_color: [1.0; 3],
        specular_strength: 1.0,
        lighting_effect_1: 0.3,
        lighting_effect_2: 2.0,
        trailer: LightingTrailer::None,
        pbr2_extra: None,
    };
    // taken from a run on 9b9cd3c4, before the field existed
    let before = r#"{"uv_offset":[0.0,0.0],"uv_scale":[1.0,1.0],"emissive_color":[0.0,0.0,0.0],"emissive_multiple":1.0,"texture_clamp_mode":3,"alpha":1.0,"refraction_strength":0.0,"glossiness":80.0,"specular_color":[1.0,1.0,1.0],"specular_strength":1.0,"lighting_effect_1":0.3,"lighting_effect_2":2.0,"trailer":"None"}"#;
    assert_eq!(serde_json::to_string(&p).unwrap(), before);
}
