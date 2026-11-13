//! The initial endpoints of a BC1 block. A port of rgbcx from bc7enc_rdo commit b943862 with Tessalith's patches
//! (MIT or public domain): `encode_bc1_pick_initial`, the arms level 18 reaches (Note HY3).

// `hit!` expands to nothing outside the unit tests, which leaves `Hit` unused there.
#[cfg_attr(not(test), allow(unused_imports))]
use crate::hits::{hit, Hit};

use super::encode::{BlockStats, Ends};
use super::fit::{precise_round_565, to_5, to_6};

/// `maximum(a, b)`: `(a > b) ? a : b`.
fn maximum(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

/// `clampf`.
fn clampf(value: f32, low: f32, high: f32) -> f32 {
    if value < low {
        low
    } else if value > high {
        high
    } else {
        value
    }
}

/// `encode_bc1_pick_initial` with level 18's flags: the grey arm for an all-grey block, else the principal-axis arm
/// with 6 power iterations (round 0) or the float bounding box (`bounding_box`, round 1, where `encode_bc1` adds
/// `cEncodeBC1BoundingBox`). The 2D least-squares and integer bounding-box arms need flags level 18 never sets.
pub(crate) fn pick_initial(px: &[[u8; 4]; 16], bounding_box: bool, s: &BlockStats) -> Ends {
    let (min, max, avg) = (s.min, s.max, s.avg);
    if s.grayscale {
        let fr = px[0][0] as u32;

        // Grayscale blocks are a common enough case to specialize.
        if (max[0] - min[0]) < 2 {
            hit!(Hit::InitGreyNarrow);
            let c5 = to_5(fr);
            let c6 = to_6(fr);
            Ends {
                l: [c5, c6, c5],
                h: [c5, c6, c5],
            }
        } else {
            hit!(Hit::InitGreyRange);
            let l5 = to_5(min[0] as u32);
            let h5 = to_5(max[0] as u32);
            Ends {
                l: [l5, to_6(min[0] as u32), l5],
                h: [h5, to_6(max[0] as u32), h5],
            }
        }
    } else if bounding_box {
        hit!(Hit::InitBox);
        // Algorithm from icbc.h compress_dxt1_fast()
        let mut l: [f32; 3] = std::array::from_fn(|c| min[c] as f32 * (1.0f32 / 255.0f32));
        let mut h: [f32; 3] = std::array::from_fn(|c| max[c] as f32 * (1.0f32 / 255.0f32));

        let bias = 8.0f32 / 255.0f32;
        let inset: [f32; 3] = std::array::from_fn(|c| (h[c] - l[c] - bias) * (1.0f32 / 16.0f32));

        for c in 0..3 {
            l[c] = clampf(l[c] + inset[c], 0.0, 1.0);
        }
        for c in 0..3 {
            h[c] = clampf(h[c] - inset[c], 0.0, 1.0);
        }

        let mut icov_xz = 0i32;
        let mut icov_yz = 0i32;
        for t in px {
            let r = t[0] as i32 - avg[0];
            let g = t[1] as i32 - avg[1];
            let b = t[2] as i32 - avg[2];
            icov_xz += r * b;
            icov_yz += g * b;
        }

        if icov_xz < 0 {
            hit!(Hit::InitBoxSwapR);
            std::mem::swap(&mut l[0], &mut h[0]);
        }

        if icov_yz < 0 {
            hit!(Hit::InitBoxSwapG);
            std::mem::swap(&mut l[1], &mut h[1]);
        }

        let (lc, hc) = precise_round_565(&l, &h);
        Ends { l: lc, h: hc }
    } else {
        hit!(Hit::InitAxis);
        // Select 2 colors along the principle axis. (There must be a faster/simpler way.)
        let mut icov = [0i32; 6];
        for t in px {
            let r = t[0] as i32 - avg[0];
            let g = t[1] as i32 - avg[1];
            let b = t[2] as i32 - avg[2];
            icov[0] += r * r;
            icov[1] += r * g;
            icov[2] += r * b;
            icov[3] += g * g;
            icov[4] += g * b;
            icov[5] += b * b;
        }

        let (mut saxis_r, mut saxis_g, mut saxis_b) = (306i32, 601i32, 117i32);

        let mut xr = (max[0] - min[0]) as f32;
        let mut xg = (max[1] - min[1]) as f32;
        let mut xb = (max[2] - min[2]) as f32;

        if icov[2] < 0 {
            hit!(Hit::InitAxisFlipR);
            xr = -xr;
        }

        if icov[4] < 0 {
            hit!(Hit::InitAxisFlipG);
            xg = -xg;
        }

        // |icov| <= 16 * 255^2 < 2^24, so the conversion is exact.
        let cov: [f32; 6] = std::array::from_fn(|i| icov[i] as f32 * (1.0f32 / 255.0f32));

        // cEncodeBC1Use6PowerIters is set at level 18.
        for _ in 0..6 {
            let r = xr * cov[0] + xg * cov[1] + xb * cov[2];
            let g = xr * cov[1] + xg * cov[3] + xb * cov[4];
            let b = xr * cov[2] + xg * cov[4] + xb * cov[5];
            xr = r;
            xg = g;
            xb = b;
        }

        // The power iteration grows the vector by at most the covariance trace (< 3 * 16 * 255^2 / 255) per step,
        // so k stays finite (< 1e27) and |x * m| <= 2048: the casts below never leave i32.
        let k = maximum(maximum(xr.abs(), xg.abs()), xb.abs());
        if k >= 2.0 {
            hit!(Hit::InitAxisScaled);
            let m = 2048.0f32 / k;
            saxis_r = (xr * m) as i32;
            saxis_g = (xg * m) as i32;
            saxis_b = (xb * m) as i32;
        } else {
            hit!(Hit::InitAxisDefault);
        }

        let mut low_dot = i32::MAX;
        let mut high_dot = i32::MIN;

        saxis_r = ((saxis_r as u32) << 4) as i32;
        saxis_g = ((saxis_g as u32) << 4) as i32;
        saxis_b = ((saxis_b as u32) << 4) as i32;

        for i in (0..16).step_by(4) {
            let dot = |k: usize| {
                let t = &px[k];
                ((t[0] as i32 * saxis_r + t[1] as i32 * saxis_g + t[2] as i32 * saxis_b) & !0xF)
                    + k as i32
            };
            let (dot0, dot1, dot2, dot3) = (dot(i), dot(i + 1), dot(i + 2), dot(i + 3));

            let min_d01 = dot0.min(dot1);
            let max_d01 = dot0.max(dot1);

            let min_d23 = dot2.min(dot3);
            let max_d23 = dot2.max(dot3);

            let min_d = min_d01.min(min_d23);
            let max_d = max_d01.max(max_d23);

            low_dot = low_dot.min(min_d);
            high_dot = high_dot.max(max_d);
        }
        let low_c = (low_dot & 15) as usize;
        let high_c = (high_dot & 15) as usize;

        let lo = &px[low_c];
        let hi = &px[high_c];
        Ends {
            l: [to_5(lo[0] as u32), to_6(lo[1] as u32), to_5(lo[2] as u32)],
            h: [to_5(hi[0] as u32), to_6(hi[1] as u32), to_5(hi[2] as u32)],
        }
    }
}
