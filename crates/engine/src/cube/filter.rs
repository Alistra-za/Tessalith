//! Cube yaw and mirror (DECISIONS DS14), and colour filters on linear-light faces (exposure, highlight clamp, display-space grading in the game's 1.6 gamma, DECISIONS DS11). Alpha is untouched.
use super::world::{to_cube, to_world};
use super::Cube;
use crate::config::CubemapSpec;
use crate::ops::{game_gamma_to_linear, linear_to_game_gamma};

/// The cube turned `degrees` about world up, positive clockwise seen from above (Skyrim's heading sense: 90 moves
/// east to south). 0 returns the input unchanged.
pub fn yaw(c: &Cube, degrees: f64) -> Cube {
    if degrees == 0.0 {
        return c.clone();
    }
    // out(d) = in(R d), R the counter-clockwise-from-above turn by `degrees` about world Z (up), in world space.
    let (s, co) = (degrees.to_radians() as f32).sin_cos();
    super::sample::resample(c, c.size, |d| {
        let w = to_world(d);
        to_cube([w[0] * co - w[1] * s, w[0] * s + w[1] * co, w[2]])
    })
}

/// The cube with world east and west swapped: `out(d) = in(to_cube(diag(-1, 1, 1) * to_world(d)))`.
pub fn mirror(c: &Cube) -> Cube {
    super::sample::resample(c, c.size, |d| {
        let w = to_world(d);
        to_cube([-w[0], w[1], w[2]])
    })
}

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Apply the pinned filter order; every step is skipped at its default, so a default spec is bit-identical.
pub fn colour(c: &mut Cube, s: &CubemapSpec) {
    let gain = 2f32.powf(s.exposure as f32);
    let exposure = s.exposure != 0.0;
    let clamp = s.highlight_clamp.map(|h| (h * h) as f32);
    let display = s.brightness != 0.0
        || s.contrast != 0.0
        || s.midtones != 1.0
        || s.saturation != 1.0
        || s.tint.is_some();
    if !exposure && clamp.is_none() && !display {
        return;
    }
    let (b, k, inv_m, sat) = (
        s.brightness as f32,
        1.0 + s.contrast as f32,
        1.0 / s.midtones as f32,
        s.saturation as f32,
    );
    let tint = s.tint.map(|t| t.map(|v| v as f32 / 255.0));
    for face in c.faces.iter_mut() {
        for p in face.px.iter_mut() {
            let mut rgb = [p[0], p[1], p[2]];
            if exposure {
                rgb = rgb.map(|v| v * gain);
            }
            if let Some(h2) = clamp {
                let l = luma(rgb);
                if l > 0.0 {
                    let l2 = (l * (1.0 + l / h2) / (1.0 + l)).min(1.0);
                    rgb = rgb.map(|v| v * (l2 / l));
                }
            }
            if display {
                let mut x = rgb.map(linear_to_game_gamma);
                if s.brightness != 0.0 || s.contrast != 0.0 {
                    x = x.map(|v| (v + b - 0.5) * k + 0.5);
                }
                if s.midtones != 1.0 {
                    x = x.map(|v| v.max(0.0).powf(inv_m));
                }
                if s.saturation != 1.0 {
                    let y = luma(x);
                    x = x.map(|v| y + (v - y) * sat);
                }
                if let Some(t) = tint {
                    x = [x[0] * t[0], x[1] * t[1], x[2] * t[2]];
                }
                rgb = x.map(|v| game_gamma_to_linear(v.clamp(0.0, 1.0)));
            }
            p[..3].copy_from_slice(&rgb);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Rgba32F;

    fn flat(rgb: [f32; 3]) -> Cube {
        let faces = std::array::from_fn(|_| Rgba32F::new(4, 4, [rgb[0], rgb[1], rgb[2], 0.7]));
        Cube { size: 4, faces }
    }

    #[test]
    fn default_spec_is_identity() {
        let mut c = flat([0.1, 0.2, 0.3]);
        for (i, f) in c.faces.iter_mut().enumerate() {
            let col = [0.05 * i as f32, 0.9, 3.5, 1.0];
            for y in 1..3 {
                for x in 0..4 {
                    f.px[y * 4 + x] = col;
                }
            }
        }
        let before = c.clone();
        colour(&mut c, &CubemapSpec::default());
        assert_eq!(c, before);
    }

    #[test]
    fn exposure_one_doubles_linear() {
        let mut c = flat([0.2, 0.2, 0.2]);
        let s = CubemapSpec {
            exposure: 1.0,
            ..Default::default()
        };
        colour(&mut c, &s);
        assert!((c.faces[0].px[0][0] - 0.4).abs() < 1e-6);
        assert_eq!(c.faces[0].px[0][3], 0.7);
    }

    #[test]
    fn highlight_clamp_maps_white_point_to_one() {
        let s = CubemapSpec {
            highlight_clamp: Some(4.0),
            ..Default::default()
        };
        let mut c = flat([4.0, 4.0, 4.0]);
        colour(&mut c, &s);
        assert!((c.faces[0].px[0][0] - 1.0).abs() < 1e-5);
        let mut c = flat([0.1, 0.1, 0.1]);
        colour(&mut c, &s);
        let v = c.faces[0].px[0][0];
        assert!(v < 0.1 && v > 0.08, "{v}");
    }

    #[test]
    fn saturation_zero_is_grey() {
        let mut c = flat([0.8, 0.2, 0.1]);
        let s = CubemapSpec {
            saturation: 0.0,
            ..Default::default()
        };
        colour(&mut c, &s);
        let p = c.faces[0].px[0];
        assert!((p[0] - p[1]).abs() < 1e-6 && (p[1] - p[2]).abs() < 1e-6);
    }

    #[test]
    fn tint_multiplies_in_display_space() {
        let mut c = flat([0.5, 0.5, 0.5]);
        let d = linear_to_game_gamma(0.5);
        let s = CubemapSpec {
            tint: Some([255, 128, 255]),
            ..Default::default()
        };
        colour(&mut c, &s);
        let p = c.faces[0].px[0];
        assert!((linear_to_game_gamma(p[1]) - d * 128.0 / 255.0).abs() < 1e-5);
        assert!((linear_to_game_gamma(p[0]) - d).abs() < 1e-5);
    }

    /// The first texel of a flat cube after `s`, in display space.
    fn graded(rgb: [f32; 3], s: &CubemapSpec) -> [f32; 3] {
        let mut c = flat(rgb);
        colour(&mut c, s);
        let p = c.faces[0].px[0];
        [p[0], p[1], p[2]].map(linear_to_game_gamma)
    }

    #[test]
    fn brightness_is_added_before_the_contrast_pivot() {
        let d = linear_to_game_gamma(0.2);
        let s = CubemapSpec {
            brightness: 0.1,
            contrast: 0.5,
            ..Default::default()
        };
        // Brightness after the pivot would give 0.05 less.
        let want = (d + 0.1 - 0.5) * 1.5 + 0.5;
        assert!((graded([0.2; 3], &s)[0] - want).abs() < 1e-5);
    }

    #[test]
    fn midtones_is_the_inverse_exponent() {
        let d = linear_to_game_gamma(0.2);
        let s = CubemapSpec {
            midtones: 2.0,
            ..Default::default()
        };
        assert!((graded([0.2; 3], &s)[0] - d.sqrt()).abs() < 1e-5);
    }

    #[test]
    fn a_channel_pushed_below_zero_is_zero_under_midtones_not_nan() {
        // Contrast takes blue below zero. A negative base under powf is NaN, and saturation's luma would spread it
        // to every channel (the cube would go black).
        let s = CubemapSpec {
            contrast: 0.5,
            midtones: 2.0,
            saturation: 0.5,
            ..Default::default()
        };
        let hi = ((linear_to_game_gamma(0.5) - 0.5) * 1.5 + 0.5).sqrt();
        let y = (0.2126 + 0.7152) * hi;
        let got = graded([0.5, 0.5, 0.0], &s);
        assert!((got[0] - (y + (hi - y) * 0.5)).abs() < 1e-5, "{got:?}");
        assert!((got[2] - y * 0.5).abs() < 1e-5, "{got:?}");
    }

    #[test]
    fn midtones_runs_before_saturation() {
        let s = CubemapSpec {
            midtones: 2.0,
            saturation: 0.5,
            ..Default::default()
        };
        let m = [0.8f32, 0.2, 0.1].map(|v| linear_to_game_gamma(v).sqrt());
        let y = 0.2126 * m[0] + 0.7152 * m[1] + 0.0722 * m[2];
        let got = graded([0.8, 0.2, 0.1], &s);
        for i in 0..3 {
            assert!((got[i] - (y + (m[i] - y) * 0.5)).abs() < 1e-5, "{i}");
        }
    }
}
