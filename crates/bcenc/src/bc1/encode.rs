//! BC1 encode at rgbcx level 18. A port of rgbcx from bc7enc_rdo commit b943862 with Tessalith's patches (MIT or
//! public domain): `encode_bc1(void*, const uint8_t*, flags, total_orderings_to_try, total_orderings_to_try3, ..)` for
//! level 18's flags, `encode_bc1_solid_block`, `try_3color_block`, `encode_bc1_endpoint_search`, `bc1_encode4` and
//! `bc1_encode3` (Notes HY3, HY4).
//!
//! Level 18 is `cEncodeBC1TwoLeastSquaresPasses | cEncodeBC1UseFullMSEEval | cEncodeBC1UseLikelyTotalOrderings |
//! cEncodeBC1Use6PowerIters | cEncodeBC1Iterative | (256 << cEncodeBC1EndpointSearchRoundsShift) |
//! cEncodeBC1TryAllInitialEndponts`, plus `cEncodeBC1Use3ColorBlocks` when `allow_3color`, with
//! `total_orderings_to_try = MAX_TOTAL_ORDERINGS4` and `total_orderings_to_try3 = 32`. The engine encodes in `cBC1Dual`
//! mode, never passes forced selectors and never sets `allow_transparent_texels_for_black`, so
//! `cEncodeBC1Use3ColorBlocksForBlackPixels` is never set. Those flags are constants here: each flag test of the C++ is
//! resolved to the arm level 18 takes; `allow_3color` is the one flag left.

// `hit!` expands to nothing outside the unit tests, which leaves `Hit` unused there.
#[cfg_attr(not(test), allow(unused_imports))]
use crate::hits::{hit, Hit};

use super::fit::{
    ls_endpoints3, ls_endpoints3_ordered, ls_endpoints4, ls_endpoints4_ordered, order_tables,
    precise_round_565, Hist3, Hist4, TOTAL_ORDER_3_16, TOTAL_ORDER_4_16,
};
use super::initial::pick_initial;
use super::palette::single_tables;
use super::seen::Seen;
use super::sels::{find_sels3, find_sels4};
use super::tables::{BEST_TOTAL_ORDERINGS3, BEST_TOTAL_ORDERINGS4, MAX_TOTAL_ORDERINGS4};
use super::{BlockStats, Ends};

/// `MAX_TOTAL_ORDERINGS3` (rgbcx.h): the width of the generated table's rows.
const MAX_TOTAL_ORDERINGS3: usize = BEST_TOTAL_ORDERINGS3[0].len();
const _: () = assert!(MAX_TOTAL_ORDERINGS3 == 32);
/// Level 18's `total_orderings3`.
const TOTAL_ORDERINGS3: usize = 32;

/// `encode_bc1(18, .., allow_3color, allow_transparent_texels_for_black = false)` in `cBC1Dual` mode with R, G, B
/// error weights `weights`.
pub(crate) fn encode(px: &[[u8; 4]; 16], allow_3color: bool, weights: [i32; 3]) -> [u8; 8] {
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
        return solid_block(fr as usize, fg as usize, fb as usize, allow_3color);
    }

    // `any_black_pixels` is not computed: every read of it is ANDed with cEncodeBC1Use3ColorBlocksForBlackPixels
    // (never set here), or ORed into `needs_block_error`, which level 18's flags already make true.
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
    // `orig_l*`, `orig_h*`: the initial endpoints of the round taken, where `try_3color_block` starts.
    let mut orig = ends;
    let mut sels = [0u8; 16];

    // needs_block_error is true at level 18 (likely total orderings, full MSE, endpoint search).
    let mut cur_err = u32::MAX;

    // The endpoint pairs scored so far in this block, one set per block type (see `seen.rs` for why a repeat can be
    // skipped). The 3-colour set exists only when 3-colour blocks are allowed: BC3's colour half never clears it.
    let mut seen4 = Seen::new();
    let mut seen3 = allow_3color.then(Seen::new);

    // cEncodeBC1TryAllInitialEndponts: two rounds, the second with cEncodeBC1BoundingBox.
    for round in 0..2 {
        let mut round_ends = pick_initial(px, round == 1, &s);
        let orig_round = round_ends;
        let mut round_sels = [0u8; 16];

        // The start is recorded, never skipped: its result is needed.
        seen4.insert(&round_ends);
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

            // Recorded, never skipped: `round_err` starts fresh in each round, so a pair round 0 scored can be taken
            // here (measured on 16 textures: 39% of the blocks take such a pair).
            let repeat = seen4.insert(&trial);
            let mut trial_sels = [0u8; 16];
            let trial_err = find_sels4(px, weights, &trial, &mut trial_sels, round_err);

            if trial_err < round_err {
                hit!(Hit::LsPassBetter);
                if repeat {
                    hit!(Hit::LsRepeatTaken);
                }
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
            orig = orig_round;
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
            total_orderings(
                px,
                weights,
                &s,
                &mut seen4,
                &mut ends,
                &mut sels,
                &mut cur_err,
            );
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

    // `results.m_3color`.
    let mut three = false;

    // cEncodeBC1Use3ColorBlocks (cEncodeBC1Use3ColorBlocksForBlackPixels is never set, so
    // `try_3color_block_useblack` never runs).
    if let Some(seen3) = seen3.as_mut() {
        if cur_err != 0 {
            hit!(Hit::Try3Run);
            three = try_3color_block(
                px,
                weights,
                &s,
                seen3,
                orig,
                &mut cur_err,
                &mut ends,
                &mut sels,
            );
        } else {
            hit!(Hit::Try3SkippedExact);
        }
    }

    // 256 endpoint search rounds, over the set of the block type the search scores.
    if cur_err != 0 {
        hit!(Hit::SearchRun);
        let search_seen = match seen3.as_mut() {
            Some(seen3) if three => seen3,
            _ => &mut seen4,
        };
        endpoint_search(
            px,
            weights,
            three,
            search_seen,
            &mut ends,
            &mut sels,
            cur_err,
        );
    }

    if three {
        bc1_encode3(&ends, &sels)
    } else {
        bc1_encode4(&ends, &sels)
    }
}

/// Shadow check of a skipped repeat (unit tests only): scores the skipped 4-colour pair in full and asserts its error
/// is not below `bound`, the error it would have had to beat. This runs on every skip of every block the unit tests
/// encode, so the argument in `seen.rs` is checked on each.
#[cfg(test)]
fn shadow4(px: &[[u8; 4]; 16], weights: [i32; 3], trial: &Ends, bound: u32) {
    let mut scratch = [0u8; 16];
    let err = find_sels4(px, weights, trial, &mut scratch, u32::MAX);
    assert!(
        err >= bound,
        "skipped 4-colour repeat {trial:?} scores {err}, below {bound}, for {px:?}"
    );
}

/// [`shadow4`] for a 3-colour pair.
#[cfg(test)]
fn shadow3(px: &[[u8; 4]; 16], weights: [i32; 3], trial: &Ends, bound: u32) {
    let mut scratch = [0u8; 16];
    let err = find_sels3(px, weights, trial, &mut scratch, u32::MAX);
    assert!(
        err >= bound,
        "skipped 3-colour repeat {trial:?} scores {err}, below {bound}, for {px:?}"
    );
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

/// The 3-colour single-colour match of the block average (`g_bc1_match*_half[avg]`), stored as
/// [`single_colour_ends`] stores the 4-colour one.
pub(super) fn single_colour_ends3(avg: [i32; 3]) -> Ends {
    let t = single_tables();
    let (r, g, b) = (
        &t.match5_half[avg[0] as usize],
        &t.match6_half[avg[1] as usize],
        &t.match5_half[avg[2] as usize],
    );
    Ends {
        l: [r.hi as i32, g.hi as i32, b.hi as i32],
        h: [r.lo as i32, g.lo as i32, b.lo as i32],
    }
}

/// Rounds a least-squares result to endpoint codes. The C++ calls `precise_round_565(xl, xh, trial_h*, trial_l*)`:
/// the low fit becomes the high endpoint and the high fit the low one.
pub(super) fn round_ls(xl: &[f32; 3], xh: &[f32; 3]) -> Ends {
    let (from_xl, from_xh) = precise_round_565(xl, xh);
    Ends {
        l: from_xh,
        h: from_xl,
    }
}

/// The prefix sums `encode_bc1` and `try_3color_block` build for their total-ordering loops: the texels sorted by
/// their dot product with the current endpoints' axis (the texel index breaks ties), `sums[c][i]` the sum of channel
/// `c` over the first `i` of them and `sums[c][16]` the block total.
#[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
fn sorted_sums(px: &[[u8; 4]; 16], e: &Ends, total: [i32; 3]) -> [[u32; 17]; 3] {
    let (lr, lg, lb) = (e.l[0], e.l[1], e.l[2]);
    let (hr, hg, hb) = (e.h[0], e.h[1], e.h[2]);
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
        sums[c][16] = total[c] as u32;
    }
    sums
}

/// One iteration of `encode_bc1`'s total-ordering loop: sort the texels along the current endpoints' axis, then try
/// the `MAX_TOTAL_ORDERINGS4` orderings `g_best_total_orderings4` lists for the current selector histogram, keeping
/// a strictly better one.
#[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
fn total_orderings(
    px: &[[u8; 4]; 16],
    weights: [i32; 3],
    s: &BlockStats,
    seen: &mut Seen,
    ends: &mut Ends,
    sels: &mut [u8; 16],
    cur_err: &mut u32,
) {
    let mut h = Hist4::default();
    for &sel in sels.iter() {
        h.0[sel as usize] += 1;
    }

    let orig_total_order_index = h.lookup_total_ordering_index();

    let sums = sorted_sums(px, ends, s.total);

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

        // A pair scored before in this block cannot beat `*cur_err`: it was taken (its error is at least `*cur_err`,
        // which only falls) or rejected against an error that was at least `*cur_err`; and a trial is taken only
        // when strictly lower (see `seen.rs`). `*cur_err` at the start of the first pass is at most every earlier
        // 4-colour error of the block (the lower of the two rounds).
        if seen.insert(&trial) {
            hit!(Hit::OrderingRepeat);
            #[cfg(test)]
            shadow4(px, weights, &trial, *cur_err);
            continue;
        }

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

/// `try_3color_block` at level 18 without forced selectors: fit a 3-colour block from `start` (the 4-colour round's
/// initial endpoints) with two least-squares passes and the 32 likely total orderings, and take it into `ends`,
/// `sels` and `cur_err` when its error is strictly below `cur_err`. Returns whether it did (`results.m_3color`).
///
/// Every selector array this reads was written in full by [`find_sels3`]: the first search and the ordering searches
/// pass `UINT32_MAX` (a block's error is far below it), and a least-squares search's selectors are kept only when its
/// total is below the cutoff it was given, which means it never stopped early.
#[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
#[allow(clippy::too_many_arguments)] // The C++ call's own list, plus the block's set of scored pairs.
fn try_3color_block(
    px: &[[u8; 4]; 16],
    weights: [i32; 3],
    s: &BlockStats,
    seen: &mut Seen,
    start: Ends,
    cur_err: &mut u32,
    ends: &mut Ends,
    sels: &mut [u8; 16],
) -> bool {
    let mut e = start;
    seen.insert(&e);
    let mut trial_sels = [0u8; 16];
    let mut trial_err = find_sels3(px, weights, &e, &mut trial_sels, u32::MAX);

    if trial_err != 0 {
        hit!(Hit::Try3Fit);
        // cEncodeBC1TwoLeastSquaresPasses.
        for _trials in 0..2 {
            let trial = match ls_endpoints3(px, &trial_sels) {
                None => single_colour_ends3(s.avg),
                Some((xl, xh)) => round_ls(&xl, &xh),
            };

            if e == trial {
                hit!(Hit::Ls3PassSame);
                break;
            }

            // Recorded, never skipped (the pass's result is the next pass's input).
            seen.insert(&trial);
            let mut trial_sels2 = [0u8; 16];
            let trial_err2 = find_sels3(px, weights, &trial, &mut trial_sels2, trial_err);

            if trial_err2 < trial_err {
                hit!(Hit::Ls3PassBetter);
                trial_err = trial_err2;
                e = trial;
                trial_sels = trial_sels2;
            } else {
                hit!(Hit::Ls3PassWorse);
                break;
            }
        }
    } else {
        hit!(Hit::Try3StartExact);
    }

    // cEncodeBC1UseLikelyTotalOrderings is set and total_orderings_to_try3 (32) is not 0.
    if trial_err != 0 {
        hit!(Hit::Orderings3Run);
        let mut h = Hist3::default();
        for &sel in trial_sels.iter() {
            h.0[sel as usize] += 1;
        }

        let orig_total_order_index = h.lookup_total_ordering_index();

        let sums = sorted_sums(px, &e, s.total);

        let tables = order_tables();
        // std::min(total_orderings_to_try, MAX_TOTAL_ORDERINGS3); cEncodeBC1Exhaustive is level 19 only.
        let q_total = TOTAL_ORDERINGS3.min(MAX_TOTAL_ORDERINGS3);
        for q in 0..q_total {
            let s_index = BEST_TOTAL_ORDERINGS3[orig_total_order_index][q] as usize;

            let trial = if TOTAL_ORDER_3_16.contains(&s_index) {
                hit!(Hit::Ordering3Single);
                single_colour_ends3(s.avg)
            } else {
                hit!(Hit::Ordering3Fit);
                let (xl, xh) =
                    ls_endpoints3_ordered(s.total, tables.factors3[s_index], s_index, &sums);
                round_ls(&xl, &xh)
            };

            // A pair scored before in this block cannot be strictly below `trial_err`: `trial_err` only falls, it
            // started at most the error of every pair this function scored before (the start, the least-squares
            // trials), and each was taken or rejected against an error at least `trial_err` (see `seen.rs`).
            if seen.insert(&trial) {
                hit!(Hit::Ordering3Repeat);
                #[cfg(test)]
                shadow3(px, weights, &trial, trial_err);
                continue;
            }

            // UINT32_MAX here, unlike the 4-colour loop, which passes cur_err.
            let mut trial_sels2 = [0u8; 16];
            let trial_err2 = find_sels3(px, weights, &trial, &mut trial_sels2, u32::MAX);

            if trial_err2 < trial_err {
                hit!(Hit::Ordering3Better);
                trial_err = trial_err2;
                e = trial;
                trial_sels = trial_sels2;
            }
        }
    } else {
        hit!(Hit::Orderings3SkippedExact);
    }

    if trial_err < *cur_err {
        hit!(Hit::Try3Taken);
        *ends = e;
        *sels = trial_sels;
        *cur_err = trial_err;
        true
    } else {
        hit!(Hit::Try3Rejected);
        false
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

/// `encode_bc1_endpoint_search` with 256 rounds: step one endpoint by one code in one of 16 directions (rounds 0..15
/// the high endpoint, 16..31 the low, repeating), keep a strictly better result, never undo the last improving step,
/// and stop 33 rounds after the last improvement. A round in the forbidden direction (the undo of the last improving
/// step) skips that stop test too, as in the C++. A step to a pair in `seen` is not scored but runs the stop test, as
/// it did when it was scored and rejected. `three` is `results.m_3color`: the trials are then scored as 3-colour
/// blocks (`use_black` false, see [`find_sels3`]).
fn endpoint_search(
    px: &[[u8; 4]; 16],
    weights: [i32; 3],
    three: bool,
    seen: &mut Seen,
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

        // A pair scored before in this block (for this block type) cannot be strictly below `cur_err`, which only
        // falls and began at most the error of everything scored before (see `seen.rs`). The step is not scored,
        // but the stop test below still runs, as after a rejected trial.
        let scored = if seen.insert(&trial) {
            hit!(Hit::SearchRepeat);
            #[cfg(test)]
            if three {
                shadow3(px, weights, &trial, cur_err);
            } else {
                shadow4(px, weights, &trial, cur_err);
            }
            None
        } else if three {
            hit!(Hit::SearchThree);
            let mut trial_sels = [0u8; 16];
            let err = find_sels3(px, weights, &trial, &mut trial_sels, cur_err);
            Some((err, trial_sels))
        } else {
            hit!(Hit::SearchFour);
            let mut trial_sels = [0u8; 16];
            let err = find_sels4(px, weights, &trial, &mut trial_sels, cur_err);
            Some((err, trial_sels))
        };

        if let Some((trial_err, trial_sels)) = scored {
            if trial_err < cur_err {
                hit!(Hit::SearchBetter);
                cur_err = trial_err;

                forbidden_direction = v[3] as i32 | (i & 16);

                *ends = trial;
                *sels = trial_sels;

                prev_improvement_index = i;
            }
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

/// `bc1_encode3`: a 3-colour block (colour 0 <= colour 1) from the endpoints and 3-colour selectors; swapping the
/// endpoints swaps selectors 0 and 1 (selector 3, black or transparent, stays 3). Equal endpoints stay equal (a
/// 3-colour block).
pub(super) fn bc1_encode3(e: &Ends, sels: &[u8; 16]) -> [u8; 8] {
    let mut lc16 = pack_unscaled_color(&e.l);
    let mut hc16 = pack_unscaled_color(&e.h);

    let invert_flag = lc16 > hc16;
    if invert_flag {
        hit!(Hit::Pack3Swap);
        std::mem::swap(&mut lc16, &mut hc16);
    } else {
        hit!(Hit::Pack3InOrder);
    }

    let mut packed_sels = 0u32;
    if invert_flag {
        const SEL_TRANS_INV: [u32; 4] = [1, 0, 2, 3];
        for (i, &s) in sels.iter().enumerate() {
            packed_sels |= SEL_TRANS_INV[s as usize] << (i * 2);
        }
    } else {
        for (i, &s) in sels.iter().enumerate() {
            packed_sels |= (s as u32) << (i * 2);
        }
    }

    block_bytes(
        lc16,
        hc16,
        [
            packed_sels as u8,
            (packed_sels >> 8) as u8,
            (packed_sels >> 16) as u8,
            (packed_sels >> 24) as u8,
        ],
    )
}

/// `encode_bc1_solid_block`. With `allow_3color`, the 3-colour (`half`, every texel on the midpoint) match of each
/// channel when its summed table error is strictly lower, endpoints ordered for a 3-colour block; otherwise the
/// 4-colour (`equals_1`, every texel on the 2/3 entry) match, equal endpoints made distinct as in [`bc1_encode4`].
fn solid_block(fr: usize, fg: usize, fb: usize, allow_3color: bool) -> [u8; 8] {
    let t = single_tables();
    let mut mask = 0xAAu32;
    let mut max16 = -1i32;
    let mut min16 = 0i32;

    if allow_3color {
        let err4 = t.match5_equals_1[fr].e as u32
            + t.match6_equals_1[fg].e as u32
            + t.match5_equals_1[fb].e as u32;
        let err3 =
            t.match5_half[fr].e as u32 + t.match6_half[fg].e as u32 + t.match5_half[fb].e as u32;

        if err3 < err4 {
            hit!(Hit::SolidThree);
            let (r, g, b) = (&t.match5_half[fr], &t.match6_half[fg], &t.match5_half[fb]);
            max16 = ((r.hi as i32) << 11) | ((g.hi as i32) << 5) | b.hi as i32;
            min16 = ((r.lo as i32) << 11) | ((g.lo as i32) << 5) | b.lo as i32;

            if max16 > min16 {
                hit!(Hit::SolidThreeSwap);
                std::mem::swap(&mut max16, &mut min16);
            }
        } else {
            hit!(Hit::SolidThreeLoses);
        }
    }

    if max16 == -1 {
        let (r, g, b) = (
            &t.match5_equals_1[fr],
            &t.match6_equals_1[fg],
            &t.match5_equals_1[fb],
        );
        max16 = ((r.hi as i32) << 11) | ((g.hi as i32) << 5) | b.hi as i32;
        min16 = ((r.lo as i32) << 11) | ((g.lo as i32) << 5) | b.lo as i32;

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
    }

    block_bytes(max16 as u32, min16 as u32, [mask as u8; 4])
}
