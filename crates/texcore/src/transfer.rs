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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_gamma_round_trips_every_8bit_code() {
        for k in 0..=255u32 {
            let v = linear_to_game_gamma(game_gamma_to_linear(k as f32 / 255.0));
            assert_eq!((v * 255.0 + 0.5).floor() as u32, k);
        }
    }

    #[test]
    fn game_gamma_is_a_1_6_power() {
        let v = game_gamma_to_linear(0.5);
        assert!(
            (v - 0.5f32.powf(1.6)).abs() < 1e-6 && (v - 0.329877).abs() < 1e-6,
            "{v}"
        );
        assert!((srgb_to_linear(0.5) - 0.214041).abs() < 1e-6);
        assert!((v - srgb_to_linear(0.5)).abs() > 0.1);
    }

    #[test]
    fn srgb_curve_round_trips_every_8_bit_code() {
        for k in 0..=255u32 {
            let v = k as f32 / 255.0;
            let back = (linear_to_srgb(srgb_to_linear(v)) * 255.0 + 0.5).floor() as u32;
            assert_eq!(back, k);
        }
        assert!((srgb_to_linear(0.5) - 0.214_041).abs() < 1e-5);
    }
}
