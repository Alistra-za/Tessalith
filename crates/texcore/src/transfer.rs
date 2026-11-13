//! Transfer curves: the exact piecewise sRGB pair and the game gamma for static cubemaps.

/// Exact piecewise sRGB EOTF (IEC 61966-2-1), not a 2.2 power.
#[inline]
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Exact piecewise sRGB OETF, the inverse of [`srgb_to_linear`].
#[inline]
pub fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// The game's display gamma for static cubemaps (DECISIONS DS11).
pub const GAME_GAMMA: f32 = 1.6;

/// Community Shaders' `TransferFunctions::GameGammaToLinear`, `pow(c, 1.6)` (`Common/TransferFunctions.hlsli:13-15`,
/// used on cube samples at `Lighting.hlsl:2126-2131`, CS build 2026-10-03). Cubes only; 2D keeps sRGB.
#[inline]
pub fn game_gamma_to_linear(v: f32) -> f32 {
    v.max(0.0).powf(GAME_GAMMA)
}

/// The inverse of [`game_gamma_to_linear`] on 0..1 (input clamped).
#[inline]
pub fn linear_to_game_gamma(v: f32) -> f32 {
    v.clamp(0.0, 1.0).powf(1.0 / GAME_GAMMA)
}
