//! BC1 four-colour encode at rgbcx level 18. A port of rgbcx from bc7enc_rdo commit b943862 with Tessalith's patches
//! (MIT or public domain): `encode_bc1(void*, const uint8_t*, flags, total_orderings_to_try, ..)` for level 18's flags
//! with 3-colour blocks off, `encode_bc1_solid_block` (4-colour result), `encode_bc1_endpoint_search` and
//! `bc1_encode4` (Note HY3).
//!
//! Level 18 is `cEncodeBC1TwoLeastSquaresPasses | cEncodeBC1UseFullMSEEval | cEncodeBC1UseLikelyTotalOrderings |
//! cEncodeBC1Use6PowerIters | cEncodeBC1Iterative | (256 << cEncodeBC1EndpointSearchRoundsShift) |
//! cEncodeBC1TryAllInitialEndponts` with `total_orderings_to_try = MAX_TOTAL_ORDERINGS4`; the engine encodes in
//! `cBC1Dual` mode and never passes forced selectors. Those flags are constants here: each flag test of the C++ is
//! resolved to the arm level 18 takes.

// `hit!` expands to nothing outside the unit tests, which leaves `Hit` unused there.
#[cfg_attr(not(test), allow(unused_imports))]
use crate::hits::{hit, Hit};

use super::fit::{
    ls_endpoints4, ls_endpoints4_ordered, order_tables, precise_round_565, Hist4, TOTAL_ORDER_4_16,
};
use super::initial::pick_initial;
use super::palette::single_tables;
use super::sels::find_sels4;
use super::tables::{BEST_TOTAL_ORDERINGS4, MAX_TOTAL_ORDERINGS4};
use super::{BlockStats, Ends};

/// `encode_bc1(18, .., allow_3color = false, allow_transparent_texels_for_black = false)` in `cBC1Dual` mode with
/// R, G, B error weights `weights`.
pub(crate) fn encode4(px: &[[u8; 4]; 16], weights: [i32; 3]) -> [u8; 8] {
    let (fr, fg, fb) = (px[0][0], px[0][1], px[0][2]);

    let mut j = 15;
    while j >= 1 {
        if px[j][0] != fr || px[j][1] != fg || px[j][2] != fb {
            break;
        }
        j -= 1;
    }

    if j == 0 {
        hit!(Hit::Bc1Solid);
        return solid_block(fr as usize, fg as usize, fb as usize);
    }

    // `any_black_pixels` is only read by the 3-colour paths, which level 18 without allow_3color never enters.
    let mut s = BlockStats {
        min: [fr as i32, fg as i32, fb as i32],
        max: [fr as i32, fg as i32, fb as i32],
        avg: [0; 3],
        total: [fr as i32, fg as i32, fb as i32],
        grayscale: fr == fg && fr == fb,
    };
    for t in &px[1..] {
        let (r, g, b) = (t[0] as i32, t[1] as i32, t[2] as i32);

        s.grayscale &= r == g && r == b;

        s.max = [s.max[0].max(r), s.max[1].max(g), s.max[2].max(b)];
        s.min = [s.min[0].min(r), s.min[1].min(g), s.min[2].min(b)];
        s.total = [s.total[0] + r, s.total[1] + g, s.total[2] + b];
    }

    s.avg = s.total.map(|t| (t + 8) >> 4);

    let mut ends = Ends {
        l: [0; 3],
        h: [0; 3],
    };
    let mut sels = [0u8; 16];

    // needs_block_error is true at level 18 (likely total orderings, full MSE, endpoint search).
    let mut cur_err = u32::MAX;

    // cEncodeBC1TryAllInitialEndponts: two rounds, the second with cEncodeBC1BoundingBox.
    for round in 0..2 {
        let mut round_ends = pick_initial(px, round == 1, &s);
        let mut round_sels = [0u8; 16];

        let mut round_err = find_sels4(px, weights, &round_ends, &mut round_sels, u32::MAX);

        // cEncodeBC1TwoLeastSquaresPasses.
        for _ls_pass in 0..2 {
            let trial = match ls_endpoints4(px, &round_sels, s.total) {
                // All selectors equal - treat it as a solid block which should always be equal or better.
                None => single_colour_ends(s.avg),
                Some((xl, xh)) => round_ls(&xl, &xh),
            };

            if round_ends == trial {
                hit!(Hit::LsPassSame);
                break;
            }

            let mut trial_sels = [0u8; 16];
            let trial_err = find_sels4(px, weights, &trial, &mut trial_sels, round_err);

            if trial_err < round_err {
                hit!(Hit::LsPassBetter);
                round_ends = trial;
                round_err = trial_err;
                round_sels = trial_sels;
            } else {
                hit!(Hit::LsPassWorse);
                break;
            }
        }

        if round_err <= cur_err {
            hit!(Hit::RoundTaken);
            cur_err = round_err;
            ends = round_ends;
            sels = round_sels;
        } else {
            hit!(Hit::RoundWorse);
        }
    }

    // cEncodeBC1UseLikelyTotalOrderings, with cEncodeBC1Iterative: up to two passes.
    if cur_err != 0 {
        hit!(Hit::OrderingsRun);
        for _iter_index in 0..2 {
            let orig_err = cur_err;
            total_orderings(px, weights, &s, &mut ends, &mut sels, &mut cur_err);
            if cur_err == 0 || cur_err == orig_err {
                if cur_err == 0 {
                    hit!(Hit::OrderingsExact);
                } else {
                    hit!(Hit::OrderingsNoGain);
                }
                break;
            }
        }
    } else {
        hit!(Hit::OrderingsSkippedExact);
    }

    // 256 endpoint search rounds.
    if cur_err != 0 {
        hit!(Hit::SearchRun);
        endpoint_search(px, weights, &mut ends, &mut sels, cur_err);
    }

    bc1_encode4(&ends, &sels)
}

/// The 4-colour single-colour match of the block average (`g_bc1_match*_equals_1[avg]`): `m_hi` as the low
/// endpoint, `m_lo` as the high one, as `encode_bc1` stores them.
fn single_colour_ends(avg: [i32; 3]) -> Ends {
    let t = single_tables();
    let (r, g, b) = (
        &t.match5_equals_1[avg[0] as usize],
        &t.match6_equals_1[avg[1] as usize],
        &t.match5_equals_1[avg[2] as usize],
    );
    Ends {
        l: [r.hi as i32, g.hi as i32, b.hi as i32],
        h: [r.lo as i32, g.lo as i32, b.lo as i32],
    }
}

/// Rounds a least-squares result to endpoint codes. The C++ calls `precise_round_565(xl, xh, trial_h*, trial_l*)`:
/// the low fit becomes the high endpoint and the high fit the low one.
fn round_ls(xl: &[f32; 3], xh: &[f32; 3]) -> Ends {
    let (from_xl, from_xh) = precise_round_565(xl, xh);
    Ends {
        l: from_xh,
        h: from_xl,
    }
}

/// One iteration of `encode_bc1`'s total-ordering loop: sort the texels along the current endpoints' axis, then try
/// the `MAX_TOTAL_ORDERINGS4` orderings `g_best_total_orderings4` lists for the current selector histogram, keeping
/// a strictly better one.
#[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
fn total_orderings(
    px: &[[u8; 4]; 16],
    weights: [i32; 3],
    s: &BlockStats,
    ends: &mut Ends,
    sels: &mut [u8; 16],
    cur_err: &mut u32,
) {
    let mut h = Hist4::default();
    for i in 0..16 {
        h.0[sels[i] as usize] += 1;
    }

    let orig_total_order_index = h.lookup_total_ordering_index();

    let (lr, lg, lb) = (ends.l[0], ends.l[1], ends.l[2]);
    let (hr, hg, hb) = (ends.h[0], ends.h[1], ends.h[2]);
    let (r0, g0, b0) = (
        (lr << 3) | (lr >> 2),
        (lg << 2) | (lg >> 4),
        (lb << 3) | (lb >> 2),
    );
    let (r3, g3, b3) = (
        (hr << 3) | (hr >> 2),
        (hg << 2) | (hg >> 4),
        (hb << 3) | (hb >> 2),
    );

    let (ar, ag, ab) = (r3 - r0, g3 - g0, b3 - b0);

    // d is 0x1000000 plus a dot product of at most 3 * 255^2 in size, so (d << 4) + i stays positive in i32.
    let mut dots = [0i32; 16];
    for i in 0..16 {
        let (r, g, b) = (px[i][0] as i32, px[i][1] as i32, px[i][2] as i32);
        let d = 0x1000000 + (r * ar + g * ag + b * ab);
        dots[i] = (d << 4) + i as i32;
    }

    // The keys are distinct (the texel index is in the low 4 bits), so any sort gives std::sort's order.
    dots.sort_unstable();

    let mut sums = [[0u32; 17]; 3];
    let mut acc = [0u32; 3];
    for i in 0..16 {
        let p = (dots[i] & 15) as usize;
        for c in 0..3 {
            sums[c][i] = acc[c];
            acc[c] += px[p][c] as u32;
        }
    }

    for c in 0..3 {
        sums[c][16] = s.total[c] as u32;
    }

    let tables = order_tables();
    // clampi(total_orderings_to_try, MIN_TOTAL_ORDERINGS, MAX_TOTAL_ORDERINGS4) with MAX_TOTAL_ORDERINGS4 passed.
    let q_total = MAX_TOTAL_ORDERINGS4.clamp(1, MAX_TOTAL_ORDERINGS4);
    for q in 0..q_total {
        let s_index = BEST_TOTAL_ORDERINGS4[orig_total_order_index][q] as usize;

        let trial = if TOTAL_ORDER_4_16.contains(&s_index) {
            hit!(Hit::OrderingSingle);
            single_colour_ends(s.avg)
        } else {
            hit!(Hit::OrderingFit);
            let (xl, xh) = ls_endpoints4_ordered(s.total, tables.factors[s_index], s_index, &sums);
            round_ls(&xl, &xh)
        };

        let mut trial_sels = [0u8; 16];
        let trial_err = find_sels4(px, weights, &trial, &mut trial_sels, *cur_err);

        if trial_err < *cur_err {
            hit!(Hit::OrderingBetter);
            *cur_err = trial_err;
            *ends = trial;
            *sels = trial_sels;
        }
    }
}

/// `s_adjacent_voxels`: a step in r, g, b, and the index of the opposite step.
const ADJACENT_VOXELS: [[i8; 4]; 16] = [
    [1, 0, 0, 3],
    [0, 1, 0, 4],
    [0, 0, 1, 5],
    [-1, 0, 0, 0],
    [0, -1, 0, 1],
    [0, 0, -1, 2],
    [1, 1, 0, 9],
    [1, 0, 1, 10],
    [0, 1, 1, 11],
    [-1, -1, 0, 6],
    [-1, 0, -1, 7],
    [0, -1, -1, 8],
    [-1, 1, 0, 13],
    [1, -1, 0, 12],
    [0, -1, 1, 15],
    [0, 1, -1, 14],
];

/// `encode_bc1_endpoint_search` for a 4-colour block with 256 rounds: step one endpoint by one code in one of 16
/// directions (rounds 0..15 the high endpoint, 16..31 the low, repeating), keep a strictly better result, never
/// undo the last improving step, and stop 33 rounds after the last improvement. A skipped round skips that stop
/// test too, as in the C++.
fn endpoint_search(
    px: &[[u8; 4]; 16],
    weights: [i32; 3],
    ends: &mut Ends,
    sels: &mut [u8; 16],
    mut cur_err: u32,
) {
    let mut prev_improvement_index = 0i32;
    let mut forbidden_direction = -1i32;

    for i in 0..256i32 {
        if forbidden_direction == (i & 31) {
            hit!(Hit::SearchForbidden);
            continue;
        }

        let v = &ADJACENT_VOXELS[(i & 15) as usize];
        let delta = [v[0] as i32, v[1] as i32, v[2] as i32];

        let mut trial = *ends;

        if (i >> 4) & 1 != 0 {
            hit!(Hit::SearchLow);
            trial.l[0] = (trial.l[0] + delta[0]).clamp(0, 31);
            trial.l[1] = (trial.l[1] + delta[1]).clamp(0, 63);
            trial.l[2] = (trial.l[2] + delta[2]).clamp(0, 31);
        } else {
            hit!(Hit::SearchHigh);
            trial.h[0] = (trial.h[0] + delta[0]).clamp(0, 31);
            trial.h[1] = (trial.h[1] + delta[1]).clamp(0, 63);
            trial.h[2] = (trial.h[2] + delta[2]).clamp(0, 31);
        }

        let mut trial_sels = [0u8; 16];
        let trial_err = find_sels4(px, weights, &trial, &mut trial_sels, cur_err);

        if trial_err < cur_err {
            hit!(Hit::SearchBetter);
            cur_err = trial_err;

            forbidden_direction = v[3] as i32 | (i & 16);

            *ends = trial;
            *sels = trial_sels;

            prev_improvement_index = i;
        }

        if i - prev_improvement_index > 32 {
            hit!(Hit::SearchStop);
            break;
        }
    }
}

/// `bc1_block::pack_unscaled_color`.
fn pack_unscaled_color(c: &[i32; 3]) -> u32 {
    (c[2] | (c[1] << 5) | (c[0] << 11)) as u16 as u32
}

/// The 8 block bytes: colour 0 (`set_low_color`), colour 1 (`set_high_color`), then the four selector bytes.
fn block_bytes(low: u32, high: u32, sel_bytes: [u8; 4]) -> [u8; 8] {
    let (low, high) = (low as u16, high as u16);
    let [l0, l1] = low.to_le_bytes();
    let [h0, h1] = high.to_le_bytes();
    [
        l0,
        l1,
        h0,
        h1,
        sel_bytes[0],
        sel_bytes[1],
        sel_bytes[2],
        sel_bytes[3],
    ]
}

/// `bc1_encode4`: a 4-colour block (colour 0 > colour 1) from the endpoints and linear selectors. Equal endpoints
/// become a block with colour 1 one code lower and every texel on colour 0 (or, for black, colour 0 = 1 and every
/// texel on colour 1).
fn bc1_encode4(e: &Ends, sels: &[u8; 16]) -> [u8; 8] {
    let mut lc16 = pack_unscaled_color(&e.l);
    let mut hc16 = pack_unscaled_color(&e.h);

    // Always forbid 3 color blocks
    if lc16 == hc16 {
        let mut mask = 0u8;

        // Make l > h
        if hc16 > 0 {
            hit!(Hit::PackEqual);
            hc16 -= 1;
        } else {
            hit!(Hit::PackEqualBlack);
            hc16 = 0;
            lc16 = 1;
            mask = 0x55; // select hc16
        }

        block_bytes(lc16, hc16, [mask; 4])
    } else {
        let mut invert_mask = 0u8;
        if lc16 < hc16 {
            hit!(Hit::PackSwap);
            std::mem::swap(&mut lc16, &mut hc16);
            invert_mask = 0x55;
        } else {
            hit!(Hit::PackInOrder);
        }

        const SEL_TRANS: [u32; 4] = [0, 2, 3, 1];
        let mut packed_sels = 0u32;
        for (i, &s) in sels.iter().enumerate() {
            packed_sels |= SEL_TRANS[s as usize] << (i * 2);
        }

        block_bytes(
            lc16,
            hc16,
            [
                packed_sels as u8 ^ invert_mask,
                (packed_sels >> 8) as u8 ^ invert_mask,
                (packed_sels >> 16) as u8 ^ invert_mask,
                (packed_sels >> 24) as u8 ^ invert_mask,
            ],
        )
    }
}

/// `encode_bc1_solid_block` with `allow_3color = false`: the 4-colour single-colour match of each channel, every
/// texel on the 2/3 entry; equal endpoints are made distinct as in [`bc1_encode4`].
fn solid_block(fr: usize, fg: usize, fb: usize) -> [u8; 8] {
    let t = single_tables();
    let mut mask = 0xAAu32;

    let (r, g, b) = (
        &t.match5_equals_1[fr],
        &t.match6_equals_1[fg],
        &t.match5_equals_1[fb],
    );
    let mut max16 = ((r.hi as i32) << 11) | ((g.hi as i32) << 5) | b.hi as i32;
    let mut min16 = ((r.lo as i32) << 11) | ((g.lo as i32) << 5) | b.lo as i32;

    if min16 == max16 {
        // Always forbid 3 color blocks
        mask = 0;

        // Make l > h
        if min16 > 0 {
            hit!(Hit::SolidEqual);
            min16 -= 1;
        } else {
            hit!(Hit::SolidEqualBlack);
            // l = h = 0
            max16 = 1;
            min16 = 0;
            mask = 0x55;
        }
    }

    if max16 < min16 {
        hit!(Hit::SolidSwap);
        std::mem::swap(&mut max16, &mut min16);
        mask ^= 0x55;
    }

    block_bytes(max16 as u32, min16 as u32, [mask as u8; 4])
}
