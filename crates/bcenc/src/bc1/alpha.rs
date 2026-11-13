//! BC1 with one-bit alpha. A port of rgbcx from bc7enc_rdo commit b943862 with Tessalith's patches (MIT or public
//! domain): `encode_bc1_alpha` and `bc1_alpha_find_sels` (sdc patch "bc1 alpha"), at level 18 (Note HY5).
//!
//! `level` reaches only the mask-0 call of `encode_bc1(level, .., true, false)`, which is [`encode`] with three-colour
//! blocks. The R, G, B weights enter only through [`dual_err`] in [`find_sels`] (rgbcx.cpp 980 and 1012, reached from
//! `bc1_alpha_find_sels` 2009 and 2019-2022): the statistics, the principal axis, the least-squares fits and the
//! endpoint steps read no weights.

// `hit!` expands to nothing outside the unit tests, which leaves `Hit` unused there.
#[cfg_attr(not(test), allow(unused_imports))]
use crate::hits::{hit, Hit};

use super::encode::{bc1_encode3, encode, round_ls, single_colour_ends3};
use super::fit::{ls_endpoints3, to_5, to_6};
use super::initial::maximum;
use super::palette::{dual_err, dual_palette};
use super::Ends;

/// `bc1_alpha_find_sels`: per opaque texel the 3-colour selector (0 low endpoint, 1 high endpoint, 2 midpoint) with
/// the least NVIDIA plus AMD error, a later selector replacing the best only when strictly better; every texel whose
/// bit is set in `mask` gets selector 3 (transparent black) and adds no error. No cutoff: all 16 selectors are written.
/// Returns the summed error of the opaque texels.
#[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
fn find_sels(px: &[[u8; 4]; 16], mask: u32, w: [i32; 3], e: &Ends, sels: &mut [u8; 16]) -> u32 {
    let p = dual_palette(true, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);

    let mut total_err: u32 = 0;
    for i in 0..16 {
        if mask & (1u32 << i) != 0 {
            hit!(Hit::AlphaSelTransparent);
            sels[i] = 3;
            continue;
        }
        let mut best_err = dual_err(&p, w, 0, px[i]);
        let mut best_sel = 0u8;
        for j in 1..3 {
            let err = dual_err(&p, w, j, px[i]);
            if err < best_err {
                hit!(Hit::AlphaSelBetter);
                best_err = err;
                best_sel = j as u8;
            }
        }
        sels[i] = best_sel;
        // uint32_t in the C++; 16 texel errors stay below 16 * 2 * 256 * 255^2 < 2^30.
        total_err += best_err;
    }
    total_err
}

/// `encode_bc1_alpha(18, ..)` with R, G, B error weights `weights`: bit i of `transparent` marks texel i transparent.
/// Mask 0 is the plain encode with three-colour blocks, mask 0xFFFF the all-transparent block; otherwise a 3-colour
/// block fitted to the opaque texels (principal axis, extreme texels, up to 8 least-squares refits, then up to 64
/// rounds of +-1 endpoint steps), masked texels on selector 3.
#[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
pub(crate) fn encode_alpha(px: &[[u8; 4]; 16], transparent: u16, weights: [i32; 3]) -> [u8; 8] {
    // `transparent_mask &= 0xFFFF` is the u16 here.
    let mask = transparent as u32;
    if mask == 0 {
        hit!(Hit::AlphaNoMask);
        return encode(px, true, weights);
    }

    if mask == 0xFFFF {
        hit!(Hit::AlphaAllTransparent);
        // set_low_color(0), set_high_color(0), set_selector_bits(0xFFFFFFFF).
        return [0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF];
    }
    hit!(Hit::AlphaFit);

    let opaque = |i: usize| (mask & (1u32 << i)) == 0;

    let (mut total_r, mut total_g, mut total_b, mut total_pixels) = (0i32, 0i32, 0i32, 0i32);
    let (mut max_r, mut max_g, mut max_b) = (0i32, 0i32, 0i32);
    let (mut min_r, mut min_g, mut min_b) = (255i32, 255i32, 255i32);
    for i in 0..16 {
        if !opaque(i) {
            hit!(Hit::AlphaSkipStats);
            continue;
        }
        let (r, g, b) = (px[i][0] as i32, px[i][1] as i32, px[i][2] as i32);
        max_r = max_r.max(r);
        max_g = max_g.max(g);
        max_b = max_b.max(b);
        min_r = min_r.min(r);
        min_g = min_g.min(g);
        min_b = min_b.min(b);
        total_r += r;
        total_g += g;
        total_b += b;
        total_pixels += 1;
    }

    // total_pixels is 1..=15: the mask is neither 0 nor 0xFFFF. The totals are non-negative, so `/` truncates as the
    // C++ int division does.
    let half_total_pixels = total_pixels >> 1;
    let avg_r = (total_r + half_total_pixels) / total_pixels;
    let avg_g = (total_g + half_total_pixels) / total_pixels;
    let avg_b = (total_b + half_total_pixels) / total_pixels;

    let mut icov = [0i32; 6];
    for i in 0..16 {
        if !opaque(i) {
            hit!(Hit::AlphaSkipCov);
            continue;
        }
        let r = px[i][0] as i32 - avg_r;
        let g = px[i][1] as i32 - avg_g;
        let b = px[i][2] as i32 - avg_b;
        icov[0] += r * r;
        icov[1] += r * g;
        icov[2] += r * b;
        icov[3] += g * g;
        icov[4] += g * b;
        icov[5] += b * b;
    }

    // |icov| <= 15 * 255^2 < 2^24, so the conversion is exact.
    let cov: [f32; 6] = std::array::from_fn(|i| icov[i] as f32 * (1.0f32 / 255.0f32));

    let mut xr = (max_r - min_r) as f32;
    let mut xg = (max_g - min_g) as f32;
    let mut xb = (max_b - min_b) as f32;
    if icov[2] < 0 {
        hit!(Hit::AlphaAxisFlipR);
        xr = -xr;
    }
    if icov[4] < 0 {
        hit!(Hit::AlphaAxisFlipG);
        xg = -xg;
    }

    for _power_iter in 0..4 {
        let r = xr * cov[0] + xg * cov[1] + xb * cov[2];
        let g = xr * cov[1] + xg * cov[3] + xb * cov[4];
        let b = xr * cov[2] + xg * cov[4] + xb * cov[5];
        xr = r;
        xg = g;
        xb = b;
    }

    // Each power step grows the vector by at most 3 * 15 * 255^2 / 255 < 12000, so after 4 steps from at most 255,
    // k < 6e18 is finite, and |x * m| is about 1024 at most: the casts below never leave i32.
    let k = maximum(maximum(xr.abs(), xg.abs()), xb.abs());
    let (mut saxis_r, mut saxis_g, mut saxis_b) = (306i32, 601i32, 117i32);
    if k >= 2.0 {
        hit!(Hit::AlphaAxisScaled);
        let m = 1024.0f32 / k;
        saxis_r = (xr * m) as i32;
        saxis_g = (xg * m) as i32;
        saxis_b = (xb * m) as i32;
    } else {
        hit!(Hit::AlphaAxisDefault);
    }

    // The first opaque texel sets both (no dot reaches i32::MAX or i32::MIN: |dot| <= 3 * 255 * 1025).
    let (mut low_dot, mut high_dot) = (i32::MAX, i32::MIN);
    let (mut low_c, mut high_c) = (0usize, 0usize);
    for i in 0..16 {
        if !opaque(i) {
            hit!(Hit::AlphaSkipDot);
            continue;
        }
        let dot = px[i][0] as i32 * saxis_r + px[i][1] as i32 * saxis_g + px[i][2] as i32 * saxis_b;
        if dot < low_dot {
            hit!(Hit::AlphaDotLow);
            low_dot = dot;
            low_c = i;
        }
        if dot > high_dot {
            hit!(Hit::AlphaDotHigh);
            high_dot = dot;
            high_c = i;
        }
    }

    let (lo, hi) = (&px[low_c], &px[high_c]);
    let mut ends = Ends {
        l: [to_5(lo[0] as u32), to_6(lo[1] as u32), to_5(lo[2] as u32)],
        h: [to_5(hi[0] as u32), to_6(hi[1] as u32), to_5(hi[2] as u32)],
    };

    let mut sels = [0u8; 16];
    let mut err = find_sels(px, mask, weights, &ends, &mut sels);

    // Least-squares refits while they improve; the solve skips selector 3, so only opaque texels count.
    // `for (pass = 0; err && (pass < 8); pass++)`.
    let mut pass = 0u32;
    loop {
        if err == 0 {
            hit!(Hit::AlphaLsExact);
            break;
        }
        if pass >= 8 {
            hit!(Hit::AlphaLsPassesExhausted);
            break;
        }

        let trial = match ls_endpoints3(px, &sels) {
            None => {
                hit!(Hit::AlphaLsSingular);
                single_colour_ends3([avg_r, avg_g, avg_b])
            }
            // precise_round_565(xl, xh, hr2, hg2, hb2, lr2, lg2, lb2), as `round_ls` stores it.
            Some((xl, xh)) => {
                hit!(Hit::AlphaLsSolved);
                round_ls(&xl, &xh)
            }
        };

        if ends == trial {
            hit!(Hit::AlphaLsSame);
            break;
        }

        let mut sels2 = [0u8; 16];
        let err2 = find_sels(px, mask, weights, &trial, &mut sels2);
        if err2 >= err {
            hit!(Hit::AlphaLsWorse);
            break;
        }
        hit!(Hit::AlphaLsBetter);
        err = err2;
        ends = trial;
        sels = sels2;
        pass += 1;
    }

    // Step each endpoint channel by +-1 while any step improves (at most 64 rounds). Each step starts from the
    // endpoints as the steps before it in the round left them; a step out of the code range is skipped, not clamped.
    // `for (round = 0; err && (round < 64); round++)`.
    let mut round = 0u32;
    loop {
        if err == 0 {
            hit!(Hit::AlphaSearchExact);
            break;
        }
        if round >= 64 {
            hit!(Hit::AlphaSearchRoundsExhausted);
            break;
        }

        let mut improved = false;
        for step in 0..12u32 {
            let mut e = [
                ends.l[0], ends.l[1], ends.l[2], ends.h[0], ends.h[1], ends.h[2],
            ];
            let c = (step >> 1) as usize;
            let lim = if c == 1 || c == 4 { 63 } else { 31 };
            e[c] += if step & 1 != 0 { 1 } else { -1 };
            if e[c] < 0 || e[c] > lim {
                hit!(Hit::AlphaStepOutOfRange);
                continue;
            }

            let trial = Ends {
                l: [e[0], e[1], e[2]],
                h: [e[3], e[4], e[5]],
            };
            let mut sels2 = [0u8; 16];
            let err2 = find_sels(px, mask, weights, &trial, &mut sels2);
            if err2 < err {
                hit!(Hit::AlphaStepBetter);
                err = err2;
                ends = trial;
                sels = sels2;
                improved = true;
            }
        }
        if !improved {
            hit!(Hit::AlphaSearchNoGain);
            break;
        }
        round += 1;
    }

    bc1_encode3(&ends, &sels)
}
