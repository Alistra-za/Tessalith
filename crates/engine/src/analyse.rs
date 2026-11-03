//! Analyse (spec §5.1): statistics used by suggestions, warnings and validation.
//!
//! One pass over the float image, no allocation beyond the result. Every
//! "code" comparison is made on the nearest 8-bit code ([`code8`]) so the
//! thresholds match what an 8-bit viewer of the same data would see.
use crate::checks::CheckStats;
use crate::image::{Rgba32F, SourceInfo};
use crate::presets::AlphaContent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Minimum, maximum and mean of one channel, in the source's own value domain.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ChannelStats {
    pub min: f32,
    pub max: f32,
    pub mean: f64,
}

/// Pixel counts by 8-bit alpha code. `le5` and `ge250` are cumulative bands
/// (they include the 0 and 255/254 codes respectively).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AlphaHistogram {
    pub eq255: u64,
    pub eq0: u64,
    pub le5: u64,
    pub ge250: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Analysis {
    pub width: u32,
    pub height: u32,
    pub is_pot: bool,
    /// R, G, B, A.
    pub channels: [ChannelStats; 4],
    /// Alpha class. `Constant255` includes near-opaque alpha and a few stray low texels (see [`NEAR_OPAQUE_ALPHA`],
    /// [`ALPHA_SPECK_SHARE`]).
    pub alpha: AlphaContent,
    pub alpha_hist: AlphaHistogram,
    /// R, G and B agree within 4/255 for every pixel.
    pub grayscale: bool,
    /// Share of pixels whose alpha code is not 255.
    pub non_white_share: f64,
    /// Predicted 1x1 alpha (box average of the base).
    pub mean_alpha: f64,
    /// Mean |length - 1| of RGB decoded as `*2 - 1`.
    pub normal_len_err: f64,
    /// Largest Rec. 709 luma over the image (not clamped, so HDR sources report above 1).
    pub peak_luminance: f32,
    /// DXGI format of a DDS input (DECISIONS S6, spec §5.1); `None` for images. Set by the caller from `SourceMeta`.
    pub dds_dxgi: Option<u32>,
    /// The input is a single cube DDS, loaded as a 6:1 strip (DECISIONS DS7). Set by the caller from `SourceMeta`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dds_cube: bool,
    /// NaN and ±∞ samples per channel (R, G, B, A). They are left out of every statistic above; Preprocess replaces
    /// them (DECISIONS S9).
    pub nonfinite: [u64; 4],
    /// Complex grass facts (design §4, §5), for an image whose height is twice its width; `None` otherwise.
    pub grass: Option<crate::grass::GrassFacts>,
    /// `|L - 1|` of the bottom-left texel `(0, h - 1)`, the one CS's complex-grass detector reads, for every shape
    /// (PBR grass design R1); infinite for an empty image.
    pub corner_err: f32,
    /// Artist check stats (DECISIONS Note CL); `None` unless [`AnalyseHints::bands`] asked for them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<CheckStats>,
}

/// Which artist check stats [`analyse_with`] computes besides the §5.1 statistics. All off by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnalyseHints {
    /// Band and luma counts; without them no [`CheckStats`] is made.
    pub bands: bool,
    /// The normal map's curl, as stored and flipped.
    pub curl: bool,
    /// The steep-step share of this channel.
    pub steps: Option<usize>,
}

/// DECISIONS Note L: source alpha whose minimum is at least 251/255 (no texel below 251) is opaque within BC
/// tolerance (BC7 re-exports leave 251..=254). The comparison is on the stored float, so an 8-bit source (exact
/// k/255) compares on its codes and a 16-bit or float source against 251/255 itself; [`NEAR_OPAQUE_EPS`] absorbs
/// only the float rounding of the division.
pub const NEAR_OPAQUE_ALPHA: f32 = CS_COMPLEX_GATE_CODE as f32 / 255.0;
const NEAR_OPAQUE_EPS: f32 = 1e-6;

/// DECISIONS Note FM34: texels below [`NEAR_OPAQUE_ALPHA`] that are fewer than this share of the image, with no
/// 4-connected group over [`ALPHA_SPECK_MAX_GROUP`] texels, are noise and the alpha counts as opaque. The smallest
/// real cutout in the survey is 0.31 %; the group cap keeps a small cutout on a large texture asking.
pub const ALPHA_SPECK_SHARE: f64 = 0.001;
pub const ALPHA_SPECK_MAX_GROUP: usize = 16;

/// A stored alpha value that counts as "low": below [`NEAR_OPAQUE_ALPHA`]. Every low-alpha test uses this one.
pub fn is_low_alpha(v: f32) -> bool {
    v < NEAR_OPAQUE_ALPHA - NEAR_OPAQUE_EPS
}

/// Note FM34's stray-texel rule over a width x height alpha plane: `low_count` texels satisfy `is_low` (index
/// `y * width + x`), they are fewer than [`ALPHA_SPECK_SHARE`] of the plane, and no 4-connected group of them is
/// larger than [`ALPHA_SPECK_MAX_GROUP`]. The group walk runs only under the share limit and stops at the first
/// group over the cap, so its work is bounded by the low count.
pub fn low_alpha_is_speck_noise(
    width: usize,
    height: usize,
    low_count: u64,
    is_low: impl Fn(usize) -> bool,
) -> bool {
    let n = width * height;
    if (low_count as f64) >= ALPHA_SPECK_SHARE * n as f64 {
        return false;
    }
    let mut seen = vec![0u64; n.div_ceil(64)];
    // True when `i` was not marked before.
    let mut mark = |i: usize| {
        let (word, bit) = (i / 64, 1u64 << (i % 64));
        let fresh = seen[word] & bit == 0;
        seen[word] |= bit;
        fresh
    };
    for start in (0..n).filter(|&i| is_low(i)) {
        if !mark(start) {
            continue;
        }
        let (mut stack, mut size) = (vec![start], 0);
        while let Some(i) = stack.pop() {
            size += 1;
            if size > ALPHA_SPECK_MAX_GROUP {
                return false;
            }
            let (x, y) = (i % width, i / width);
            let near = [
                (x > 0).then(|| i - 1),
                (x + 1 < width).then(|| i + 1),
                (y > 0).then(|| i - width),
                (y + 1 < height).then(|| i + width),
            ];
            for j in near.into_iter().flatten() {
                if is_low(j) && mark(j) {
                    stack.push(j);
                }
            }
        }
    }
    true
}
/// Community Shaders' complex-material alpha gate, as an 8-bit code (spike S8, DECISIONS Note L): an `_m` whose
/// smallest-mip alpha is at or above it is a plain env mask, and source alpha whose minimum reaches it is opaque.
pub const CS_COMPLEX_GATE_CODE: u8 = 251;

/// Round a stored value to the nearest 8-bit code, clamping to `[0, 1]` first.
/// NaN maps to 0.
pub fn code8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The widest spread, in 8-bit codes, that still counts as one value: R, G and B within it are grey
/// ([`Analysis::grayscale`]), and a channel whose range is within it is flat ([`is_flat`], Note FM36).
pub const FLAT_CODES: i32 = 4;

/// Whether a channel that stores `min..=max` holds one value, to [`FLAT_CODES`].
pub fn is_flat(min: f32, max: f32) -> bool {
    i32::from(code8(max)) - i32::from(code8(min)) <= FLAT_CODES
}

/// Compute the §5.1 statistics for `img`. `info` says whether the source file
/// actually carried an alpha channel (the float image always has four).
pub fn analyse(img: &Rgba32F, info: &SourceInfo) -> Analysis {
    analyse_with(img, info, AnalyseHints::default())
}

/// [`analyse`], plus the artist check stats `hints` asks for.
pub fn analyse_with(img: &Rgba32F, info: &SourceInfo, hints: AnalyseHints) -> Analysis {
    let count = img.px.len();
    let n = count.max(1) as f64;
    let mut ch = [ChannelStats {
        min: f32::MAX,
        max: f32::MIN,
        mean: 0.0,
    }; 4];
    let mut hist = AlphaHistogram::default();
    let mut low_alpha = 0u64;
    let mut nonfinite = [0u64; 4];
    let mut finite = [0u64; 4];
    let mut len_n = 0u64;
    let (mut gray, mut len_err, mut peak) = (true, 0.0f64, 0f32);
    let mut codes = hints.bands.then(|| Box::new([[0u64; 256]; 4]));
    for p in &img.px {
        for (c, (s, &v)) in ch.iter_mut().zip(p).enumerate() {
            if !v.is_finite() {
                nonfinite[c] += 1;
                continue;
            }
            s.min = s.min.min(v);
            s.max = s.max.max(v);
            s.mean += f64::from(v);
            finite[c] += 1;
        }
        let a = code8(p[3]);
        match a {
            255 => hist.eq255 += 1,
            0 => hist.eq0 += 1,
            _ => {}
        }
        hist.le5 += u64::from(a <= 5);
        hist.ge250 += u64::from(a >= 250);
        low_alpha += u64::from(is_low_alpha(p[3]));
        let (r, g, b) = (
            i32::from(code8(p[0])),
            i32::from(code8(p[1])),
            i32::from(code8(p[2])),
        );
        gray &= r.max(g).max(b) - r.min(g).min(b) <= FLAT_CODES;
        if let Some(h) = &mut codes {
            CheckStats::hist_add(h, [r as u8, g as u8, b as u8, a]);
        }
        if p[..3].iter().all(|v| v.is_finite()) {
            let v = [p[0] * 2.0 - 1.0, p[1] * 2.0 - 1.0, p[2] * 2.0 - 1.0];
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            len_err += f64::from((len - 1.0).abs());
            len_n += 1;
            peak = peak.max(0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]);
        }
    }
    for (c, s) in ch.iter_mut().enumerate() {
        if finite[c] == 0 {
            // No finite sample: report zeros rather than the fold identities.
            (s.min, s.max) = (0.0, 0.0);
        }
        s.mean /= finite[c].max(1) as f64;
    }
    let mut checks = codes.map(|h| CheckStats::from_hist(info.channels == 2, &h));
    if let Some(s) = &mut checks {
        s.curl = hints.curl.then(|| crate::checks::curl(img));
        s.steep = hints.steps.map(|c| crate::checks::steep_share(img, c));
    }
    let ln = len_n.max(1) as f64;
    // A NaN texel has code 0 but no value: it must not turn all-255 into near-opaque (trace #6).
    let all_255 = hist.eq255 > 0 && (hist.eq255 + nonfinite[3]) as usize == count;
    let near_opaque = info.has_alpha && !is_low_alpha(ch[3].min);
    let speckled = info.has_alpha
        && !(all_255 || near_opaque)
        && low_alpha_is_speck_noise(img.width as usize, img.height as usize, low_alpha, |i| {
            is_low_alpha(img.px[i][3])
        });
    let alpha = if !info.has_alpha {
        AlphaContent::None
    } else if all_255 || near_opaque || speckled {
        AlphaContent::Constant255
    } else {
        AlphaContent::Varying
    };
    Analysis {
        width: img.width,
        height: img.height,
        is_pot: img.width.is_power_of_two() && img.height.is_power_of_two(),
        channels: ch,
        alpha,
        alpha_hist: hist,
        grayscale: gray,
        non_white_share: (count as u64 - hist.eq255) as f64 / n,
        mean_alpha: ch[3].mean,
        normal_len_err: len_err / ln,
        peak_luminance: peak,
        dds_dxgi: None,
        dds_cube: false,
        nonfinite,
        grass: crate::grass::facts(img),
        corner_err: img
            .px
            .get(img.height.saturating_sub(1) as usize * img.width as usize)
            .map_or(f32::INFINITY, |c| {
                crate::grass::unit_err([c[0], c[1], c[2]])
            }),
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{Rgba32F, SourceInfo};
    use AlphaContent as Alpha;

    const RGBA8: SourceInfo = SourceInfo {
        bit_depth: 8,
        channels: 4,
        has_alpha: true,
        is_float: false,
    };

    /// PBR grass design R1: `corner_err` reads texel `(0, h - 1)` of any shape, ignores alpha, and is infinite for an
    /// empty image.
    #[test]
    fn corner_err_reads_the_bottom_left_texel_of_any_shape() {
        const UNIT: [f32; 4] = [0.5, 0.5, 1.0, 1.0];
        const OFF: [f32; 4] = [0.5, 0.5, 0.5, 1.0];
        let corner = |w, h, at: (u32, u32), px: [f32; 4]| {
            let mut img = Rgba32F::new(w, h, OFF);
            *img.at_mut(at.0, at.1) = px;
            analyse(&img, &RGBA8).corner_err
        };
        for (w, h) in [(3, 5), (1, 4), (4, 1), (1, 1)] {
            assert!(corner(w, h, (0, h - 1), UNIT) < 1e-6, "{w}x{h}");
            assert!(
                (corner(w, h, (0, h - 1), [0.5, 0.5, 1.0, 0.0]) < 1e-6),
                "{w}x{h} alpha"
            );
            for at in [(w - 1, h - 1), (0, 0), (w - 1, 0)] {
                if at != (0, h - 1) {
                    assert!(corner(w, h, at, UNIT) > 0.9, "{w}x{h} {at:?}");
                }
            }
        }
        assert_eq!(
            analyse(&Rgba32F::new(0, 0, OFF), &RGBA8).corner_err,
            f32::INFINITY
        );
        assert_eq!(
            analyse(&Rgba32F::new(4, 0, OFF), &RGBA8).corner_err,
            f32::INFINITY
        );
    }

    #[test]
    fn alpha_classification() {
        let opaque = Rgba32F::new(4, 4, [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(analyse(&opaque, &RGBA8).alpha, AlphaContent::Constant255);
        let no_alpha = SourceInfo {
            channels: 3,
            has_alpha: false,
            ..RGBA8
        };
        assert_eq!(analyse(&opaque, &no_alpha).alpha, AlphaContent::None);
        let mut cut = opaque.clone();
        cut.px[0][3] = 0.0;
        let a = analyse(&cut, &RGBA8);
        assert_eq!(a.alpha, AlphaContent::Varying);
        assert_eq!(
            (a.alpha_hist.eq0, a.alpha_hist.eq255, a.alpha_hist.le5),
            (1, 15, 1)
        );
        assert!((a.non_white_share - 1.0 / 16.0).abs() < 1e-12);
        assert!(a.is_pot && a.grayscale);
    }

    #[test]
    fn grayscale_tolerance_is_4_codes() {
        let mut img = Rgba32F::new(2, 1, [0.5, 0.5, 0.5, 1.0]);
        img.px[1] = [0.5, 0.5 + 4.0 / 255.0, 0.5, 1.0];
        assert!(analyse(&img, &RGBA8).grayscale);
        img.px[1] = [0.5, 0.5 + 6.0 / 255.0, 0.5, 1.0];
        assert!(!analyse(&img, &RGBA8).grayscale);
    }

    /// Pairwise tolerance is not transitive: the whole R/G/B spread must be within 4 codes.
    #[test]
    fn grayscale_spread_covers_red_to_blue() {
        let px = |c: [f32; 3]| Rgba32F::new(2, 2, [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0, 1.0]);
        assert!(!analyse(&px([100.0, 104.0, 108.0]), &RGBA8).grayscale);
        assert!(analyse(&px([100.0, 102.0, 104.0]), &RGBA8).grayscale);
    }

    #[test]
    fn normal_length_error() {
        let flat = Rgba32F::new(4, 4, [0.5, 0.5, 1.0, 1.0]); // (0, 0, 1) encoded
        assert!(analyse(&flat, &RGBA8).normal_len_err < 0.01);
        let short = Rgba32F::new(4, 4, [0.75, 0.5, 0.5, 1.0]); // decodes to (0.5, 0, 0)
        assert!((analyse(&short, &RGBA8).normal_len_err - 0.5).abs() < 1e-6);
    }

    #[test]
    fn code8_rounds_and_clamps() {
        assert_eq!(code8(0.0), 0);
        assert_eq!(code8(1.0), 255);
        assert_eq!(code8(-3.0), 0);
        assert_eq!(code8(7.5), 255);
        assert_eq!(code8(f32::NAN), 0);
        assert_eq!(code8(127.4 / 255.0), 127);
        assert_eq!(code8(127.6 / 255.0), 128);
    }

    #[test]
    fn nan_and_infinite_samples_are_counted_and_left_out_of_the_statistics() {
        let mut img = Rgba32F::new(4, 1, [0.5, 0.5, 1.0, 1.0]);
        img.px[0] = [f32::NAN, 0.5, 1.0, 1.0];
        img.px[1] = [0.5, f32::INFINITY, 1.0, f32::NAN];
        img.px[2] = [f32::NEG_INFINITY, 0.5, 1.0, 1.0];
        let a = analyse(&img, &RGBA8);
        assert_eq!(a.nonfinite, [2, 1, 0, 1]);
        assert_eq!((a.channels[0].min, a.channels[0].max), (0.5, 0.5));
        assert!(
            (a.channels[0].mean - 0.5).abs() < 1e-12,
            "mean over finite samples"
        );
        assert!(a.channels[1].mean.is_finite() && a.channels[1].max == 0.5);
        assert!((a.mean_alpha - 1.0).abs() < 1e-12);
        // Only the fully finite pixel (index 3) is measured: (0, 0, 1) has unit length.
        assert!(a.normal_len_err < 1e-6);
        assert!(a.peak_luminance.is_finite());
    }

    #[test]
    fn an_all_nan_channel_reports_zeros_not_nan() {
        let img = Rgba32F::new(2, 2, [f32::NAN, 0.5, 0.5, 1.0]);
        let a = analyse(&img, &RGBA8);
        assert_eq!(a.nonfinite[0], 4);
        assert_eq!(
            (a.channels[0].min, a.channels[0].max, a.channels[0].mean),
            (0.0, 0.0, 0.0)
        );
        assert_eq!(a.normal_len_err, 0.0);
    }

    #[test]
    fn channel_stats_alpha_bands_and_peak() {
        let mut img = Rgba32F::new(3, 1, [0.0, 0.0, 0.0, 1.0]);
        img.px[0] = [0.2, 0.4, 0.6, 254.0 / 255.0];
        img.px[1] = [1.0, 1.0, 1.0, 250.0 / 255.0];
        img.px[2] = [0.0, 0.0, 0.0, 5.0 / 255.0];
        let a = analyse(&img, &RGBA8);
        assert_eq!((a.channels[0].min, a.channels[0].max), (0.0, 1.0));
        assert!((a.channels[1].mean - 1.4 / 3.0).abs() < 1e-6);
        assert_eq!(
            a.alpha_hist,
            AlphaHistogram {
                eq255: 0,
                eq0: 0,
                le5: 1,
                ge250: 2
            }
        );
        assert!((a.mean_alpha - a.channels[3].mean).abs() < 1e-12);
        assert!((a.peak_luminance - 1.0).abs() < 1e-6);
        assert!(!a.is_pot);
    }

    #[test]
    fn hdr_peak_is_not_clamped_and_empty_image_is_safe() {
        let hdr = Rgba32F::new(2, 2, [4.0, 4.0, 4.0, 1.0]);
        assert!(analyse(&hdr, &RGBA8).peak_luminance > 3.9);
        let empty = Rgba32F::new(0, 0, [0.0; 4]);
        let a = analyse(&empty, &RGBA8);
        assert_eq!(a.channels[0].min, 0.0);
        assert!(!a.is_pot && a.normal_len_err == 0.0 && a.non_white_share == 0.0);
    }

    /// DECISIONS Note L: a minimum of 251/255 or more is opaque within BC tolerance; one code below is varying.
    #[test]
    fn near_opaque_alpha_is_constant_at_min_251_and_varying_at_250() {
        let alpha = |codes: &[u8]| {
            let mut img = Rgba32F::new(codes.len() as u32, 1, [0.5, 0.5, 0.5, 1.0]);
            for (p, &c) in img.px.iter_mut().zip(codes) {
                p[3] = f32::from(c) / 255.0;
            }
            analyse(&img, &RGBA8)
        };
        let a = alpha(&[251, 252, 253, 254, 255]);
        assert_eq!(a.alpha, AlphaContent::Constant255);
        assert_eq!(alpha(&[250, 255, 255]).alpha, AlphaContent::Varying);
        assert_eq!(alpha(&[255, 255]).alpha, AlphaContent::Constant255);
        // No alpha channel in the source: never near-opaque.
        let none = SourceInfo {
            channels: 3,
            has_alpha: false,
            ..RGBA8
        };
        let img = Rgba32F::new(2, 1, [0.5, 0.5, 0.5, 251.0 / 255.0]);
        assert_eq!(analyse(&img, &none).alpha, AlphaContent::None);
    }

    #[test]
    fn band_counts_match_a_per_texel_count_and_are_absent_without_hints() {
        use crate::checks::BandCounts;
        let mut img = Rgba32F::new(4, 1, [0.0; 4]);
        for (p, c) in img.px.iter_mut().zip([0u8, 25, 230, 255]) {
            *p = [c as f32 / 255.0; 4];
        }
        let a = analyse_with(
            &img,
            &RGBA8,
            AnalyseHints {
                bands: true,
                ..Default::default()
            },
        );
        let s = a.checks.unwrap();
        assert_eq!(
            s.bands[0],
            BandCounts {
                eq0: 1,
                le4: 1,
                ge251: 1,
                eq255: 1
            }
        );
        assert_eq!(s.bands[3], s.bands[0]);
        assert_eq!(s.texels, 4);
        assert!(!s.two_channel);
        assert_eq!((s.curl, s.steep), (None, None));
        assert_eq!(analyse(&img, &RGBA8).checks, None);
        // Everything else is unchanged by the hints.
        let mut plain = a.clone();
        plain.checks = None;
        assert_eq!(plain, analyse(&img, &RGBA8));
        // A 0x0 image: texels 0, every count 0, no panic.
        let e = analyse_with(
            &Rgba32F::new(0, 0, [0.0; 4]),
            &RGBA8,
            AnalyseHints {
                bands: true,
                curl: true,
                steps: Some(0),
            },
        );
        let e = e.checks.unwrap();
        assert_eq!(e.texels, 0);
        assert_eq!(e.bands, [BandCounts::default(); 4]);
        // Two-channel sources (BC5 / RG8) are flagged; curl and steps run only when hinted.
        let rg = SourceInfo {
            channels: 2,
            has_alpha: false,
            ..RGBA8
        };
        let s = analyse_with(
            &img,
            &rg,
            AnalyseHints {
                bands: true,
                curl: true,
                steps: Some(0),
            },
        )
        .checks
        .unwrap();
        assert!(s.two_channel && s.curl.is_some() && s.steep.is_some());
    }

    /// 16-bit sources: the cutoff is 251/255 in float, so 250.9/255 is varying and 251/255 is not.
    #[test]
    fn near_opaque_cutoff_is_251_over_255_in_float() {
        let src = SourceInfo {
            bit_depth: 16,
            ..RGBA8
        };
        let mut img = Rgba32F::new(2, 1, [0.5, 0.5, 0.5, 1.0]);
        img.px[0][3] = 250.9 / 255.0;
        assert_eq!(analyse(&img, &src).alpha, AlphaContent::Varying);
        img.px[0][3] = 251.0 / 255.0;
        assert_eq!(analyse(&img, &src).alpha, AlphaContent::Constant255);
    }

    /// Trace-debug #6: one NaN alpha texel on an otherwise opaque float source is not "near-opaque" (its code is 0,
    /// which broke the all-255 test while the finite minimum stayed 1.0).
    #[test]
    fn a_nan_alpha_texel_on_an_opaque_source_is_not_near_opaque() {
        let src = SourceInfo {
            is_float: true,
            ..RGBA8
        };
        let mut img = Rgba32F::new(2, 1, [0.5, 0.5, 0.5, 1.0]);
        img.px[0][3] = f32::NAN;
        assert_eq!(analyse(&img, &src).alpha, AlphaContent::Constant255);
    }

    /// An opaque `w` x `h` image with the texels `low` (x, y) at 8-bit alpha code `code`.
    fn specked(
        w: u32,
        h: u32,
        code: u8,
        low: impl IntoIterator<Item = (u32, u32)>,
    ) -> AlphaContent {
        let mut img = Rgba32F::new(w, h, [0.5, 0.5, 0.5, 1.0]);
        for (x, y) in low {
            img.at_mut(x, y)[3] = f32::from(code) / 255.0;
        }
        analyse(&img, &RGBA8).alpha
    }

    /// Note FM34 boundaries: one texel at 250 or at 251 on a large image is opaque; a region at 250 that is 0.5 % of
    /// the image (well over the share, and over the group cap) stays varying.
    #[test]
    fn code_250_is_low_and_251_is_not_at_the_speck_boundaries() {
        assert_eq!(specked(512, 512, 250, [(9, 9)]), Alpha::Constant255);
        assert_eq!(specked(512, 512, 251, [(9, 9)]), Alpha::Constant255);
        // 18 x 18 = 324 texels, 0.5 % of 512 x 512.
        let block = (0..18u32).flat_map(|y| (0..18u32).map(move |x| (100 + x, 100 + y)));
        assert_eq!(specked(512, 512, 250, block.clone()), Alpha::Varying);
        assert_eq!(specked(512, 512, 251, block), Alpha::Constant255);
    }

    /// Note FM34: texels at 250 or below that are fewer than 0.1 % of the image and form no group over 16 are noise.
    #[test]
    fn a_few_low_texels_are_opaque_but_a_real_cutout_or_gradient_is_not() {
        // One texel at 200 on 64 x 64: opaque.
        assert_eq!(specked(64, 64, 200, [(7, 9)]), Alpha::Constant255);
        // The share limit on 64 x 64 (4096 texels, 0.1 % is 4.096): four spread texels are noise, five are not.
        let spread = |n: u32| (0..n).map(|i| (i * 9, i * 9));
        assert_eq!(specked(64, 64, 0, spread(4)), Alpha::Constant255);
        assert_eq!(specked(64, 64, 0, spread(5)), Alpha::Varying);
        // A real cutout (10 % at 0) and a soft gradient.
        let tenth = (0..64u32 * 64 / 10).map(|i| (i % 64, i / 64));
        assert_eq!(specked(64, 64, 0, tenth), Alpha::Varying);
        let mut ramp = Rgba32F::new(64, 64, [0.5, 0.5, 0.5, 1.0]);
        for (i, p) in ramp.px.iter_mut().enumerate() {
            p[3] = (i % 64) as f32 / 63.0;
        }
        assert_eq!(analyse(&ramp, &RGBA8).alpha, Alpha::Varying);
    }

    /// Note FM34: the group cap. 0.1 % of a large image is many texels, so a small real cutout must still ask: a 4 x 4
    /// block (16 texels) is noise, 17 connected texels are not, and a diagonal chain is not connected.
    #[test]
    fn a_connected_group_over_16_texels_is_not_noise() {
        let block =
            |w: u32, h: u32| (0..h).flat_map(move |y| (0..w).map(move |x| (100 + x, 100 + y)));
        assert_eq!(specked(512, 512, 0, block(4, 4)), Alpha::Constant255);
        assert_eq!(specked(512, 512, 0, block(5, 4)), Alpha::Varying);
        let row = |n: u32| (0..n).map(|x| (x, 40));
        assert_eq!(specked(512, 512, 0, row(16)), Alpha::Constant255);
        assert_eq!(specked(512, 512, 0, row(17)), Alpha::Varying);
        // Diagonal neighbours are separate groups.
        assert_eq!(
            specked(512, 512, 0, (0..40).map(|i| (i, i))),
            Alpha::Constant255
        );
        // A 5 x 5 block (25 texels) on a 2048 x 2048 image is under 0.1 % (4194 texels) and still asks.
        assert_eq!(specked(2048, 2048, 0, block(5, 5)), Alpha::Varying);
    }
}
