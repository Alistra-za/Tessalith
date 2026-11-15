//! NIF material → CS Lighting.hlsl permutation defines (plan 04 §1.2, corrected by real captures:
//! tools/rdc-out/p04/answers.md Q4, controller ruling RB7).

/// How a drawn material uses TruePBR (Note ID33): not at all, the v1 form (SLSF2 bit 23; `material::drawn` clears it,
/// Note IE) or CS 2.0's, from a `.csm`. [`crate::material::pbr_mode`] picks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pbr {
    None,
    V1,
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub shader_type: u32,
    pub slsf1: u32,
    pub slsf2: u32,
    /// TRUE_PBR and its technique and define rules; set from the drawn material ([`crate::material::perm_material`]).
    pub pbr: Pbr,
    /// With `pbr == V2`: the Glint feature bit is set, so the pixel shader compiles `GLINT`.
    pub pbr2_glint: bool,
    pub skinned: bool,
    /// The shape has an NiAlphaProperty (blend or test): CS sets DoAlphaTest for both (answers.md Q4.2).
    pub alpha_test: bool,
    pub has_vertex_colors: bool,
    /// NIF specular strength; the Specular lighting flag is dropped when it is 0 (answers.md Q4.2, Arch_Mouth).
    pub specular_strength: f32,
    /// The MATO projected layer applies to this shape (Note GG): `PROJECTED_UV` on both stages. Set by the
    /// renderer from its layer and `material::projected_eligible`; [`crate::material::perm_material`] leaves it false.
    pub projected: bool,
    /// With `projected`: the layer has projected glint, so a PBR pixel shader compiles `GLINT` (capture 18019).
    pub projected_glint: bool,
    /// The skin card's wetness is on for a skin shape (Note GN, `SkinPreview::wet_on`): the pixel shader compiles
    /// `WETNESS_EFFECTS` when CS_SKIN is a feature. Only these shapes: on every other shape CS's rain and puddle
    /// code would change pixels the card never touched.
    pub wetness: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stage {
    Vertex,
    Pixel,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Defines(pub Vec<(String, String)>);

const fn bit(v: u32, b: u32) -> bool {
    v & (1 << b) != 0
}

fn technique(shader_type: u32) -> &'static [&'static str] {
    match shader_type {
        1 => &["ENVMAP"],
        3 => &["PARALLAX"],
        4 => &["FACEGEN"],
        5 => &["FACEGEN_RGB_TINT"],
        6 => &["HAIR"],
        11 => &["MULTI_LAYER_PARALLAX", "ENVMAP"],
        16 => &["EYE"],
        // 2 = Glowmap: runtime technique (cb4 PixelShaderDescriptor >> 24, Lighting.hlsl:2598).
        _ => &[],
    }
}

pub fn defines(m: &Material, stage: Stage, features: &[&str], deferred: bool) -> Defines {
    let ps = stage == Stage::Pixel;
    let pbr_material = m.pbr != Pbr::None;
    let pbr = ps && pbr_material;
    let mut out: Vec<&str> = vec![if ps { "PSHADER" } else { "VSHADER" }];
    let vs_cleared = matches!(m.shader_type, 2 | 3 | 4 | 5 | 6 | 13 | 14 | 15);
    if ps || !vs_cleared {
        // CS drops the EnvMap/Parallax/MLP technique for TruePBR on both stages (captured Arch_Eye_Moisture:
        // type 11 + SLSF2.23 → PS key 100009 and VS desc 2, technique 0). Keeping them collides with the PBR
        // samplers (Lighting.hlsl:360-371: s4/s5 overlap, SampParallaxSampler redefinition).
        out.extend(technique(m.shader_type).iter().filter(|t| {
            !(pbr_material && matches!(**t, "ENVMAP" | "PARALLAX" | "MULTI_LAYER_PARALLAX"))
        }));
    }
    if m.skinned {
        out.push("SKINNED");
    }
    if bit(m.slsf1, 12) {
        out.push("MODELSPACENORMALS");
    }
    if bit(m.slsf1, 23) || m.projected {
        out.push("PROJECTED_UV");
    }
    if !ps && bit(m.slsf2, 5) && m.has_vertex_colors {
        out.push("VC");
    }
    // CS pairs VC with PROJECTED_UV (captures 8019/18019). The game sets the vertex-colour flag itself when the mesh has
    // colours (owner, Note GG21), so VC follows the mesh, not SLSF2.5; a colour-less mesh gets none (vertexAlpha = 1).
    if m.projected && m.has_vertex_colors {
        out.push("VC");
    }
    if pbr {
        out.push("TRUE_PBR");
    }
    // PS only: the VS never reads it, and captured VS descriptors lack bit 20 (answers.md Q4.2).
    if ps && m.alpha_test {
        out.push("DO_ALPHA_TEST");
    }
    // HAIR + Decal/Dynamic_Decal (SLSF1.26/27) → DepthWriteDecals (answers.md Q4.2, Arch_Scalp/Lashes).
    // PS only: read at Lighting.hlsl:884, 892, 3036 (all in the PSHADER section).
    if ps && m.shader_type == 6 && (bit(m.slsf1, 26) || bit(m.slsf1, 27)) {
        out.push("DEPTH_WRITE_DECALS");
    }
    // TruePBR: CS enables glint from its remapped Glint flag (Fit_Slope without MLP/Soft/Back; 01 §3.1, P1 spec
    // §2.2); SLSF2.21 is cleared at load. Vanilla: Anisotropic_Lighting (SLSF2.21) → ANISO_LIGHTING.
    // Projected glint (CS PBR::Flags::ProjectedGlint) needs the same GLINT code (capture 18019). A `.csm` has its own
    // Glint feature bit (Note ID33).
    let own_glint = match m.pbr {
        Pbr::V1 => crate::material::remapped_glint(m.slsf2),
        Pbr::V2 => m.pbr2_glint,
        Pbr::None => false,
    };
    if pbr && (own_glint || (m.projected && m.projected_glint)) {
        out.push("GLINT");
    } else if ps && !pbr && bit(m.slsf2, 21) {
        out.push("ANISO_LIGHTING");
    }
    // Specular, Soft, Rim and Back lighting are no defines on CS 2.0: they reach the shader at run time (Note IE8,
    // `runtime_bits`).
    // Every CS skin wetness line is under it (Lighting.hlsl:1658, 1695, 2169), all in the PS.
    if ps && m.wetness && features.contains(&"CS_SKIN") {
        out.push("WETNESS_EFFECTS");
    }
    if deferred {
        out.push("DEFERRED");
    }
    let mut d: Vec<(String, String)> = vec![];
    for n in out.into_iter().chain(features.iter().copied()) {
        if !d.iter().any(|(k, _)| k == n) {
            d.push((n.to_string(), "1".to_string()));
        }
    }
    Defines(d)
}

/// cb4 `PixelShaderDescriptor` Specular, Soft, Rim and Back lighting (cs2 Common/Permutation.hlsli `LightingFlags`).
pub const LIGHTING_SPECULAR: u32 = 1 << 9;
pub const LIGHTING_SOFT: u32 = 1 << 10;
pub const LIGHTING_RIM: u32 = 1 << 11;
pub const LIGHTING_BACK: u32 = 1 << 12;

/// The bits cb4 `PixelShaderDescriptor` carries for this material: `engine PS desc & ~CS compile key` (answers.md
/// Q4.1), with CharacterLight/DefShadow/ShadowDir 0 in the preview. CS 2.0 keeps Specular, Soft, Rim and Back out of
/// the compile key and reads them here (cs2 Lighting.hlsl `HasLightingFlag`; Note IE8): Specular for SLSF1.0 with a
/// specular strength above 0 (Arch_Mouth), Soft, Rim, Back for SLSF2.25-27, none of them under TruePBR. The Glowmap
/// technique is read here too (`(PixelShaderDescriptor >> 24) & 0x3F == Glowmap`), so type 2 adds `2 << 24`.
pub fn runtime_bits(m: &Material) -> u32 {
    let technique = if m.shader_type == 2 { 2 << 24 } else { 0 };
    if m.pbr != Pbr::None {
        return technique;
    }
    [
        (
            bit(m.slsf1, 0) && m.specular_strength > 0.0,
            LIGHTING_SPECULAR,
        ),
        (bit(m.slsf2, 25), LIGHTING_SOFT),
        (bit(m.slsf2, 26), LIGHTING_RIM),
        (bit(m.slsf2, 27), LIGHTING_BACK),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .fold(technique, |b, (_, flag)| b | flag)
}
