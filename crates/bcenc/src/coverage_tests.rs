//! Two roles. Every `hit!` branch of the ported code is taken by the class generators. And the code paths agree: each
//! hidden `_on` entry gives the base path's bytes on every path the machine has, and each public function is its `_on`
//! entry on `Path::best()`.
use crate::bc1::alpha::encode_alpha;
use crate::bc1::frames::{encode_alpha_on, encode_level_on, encode_on};
use crate::bc1::palette::{build_single_tables, dual_palette, single_tables};
use crate::bc1::{build_order_tables, order_tables, precise_round_565, to_5, to_6};
use crate::bc1::{params, Params, LEVELS, PERCEPTUAL, PLAIN};
use crate::gen;
use crate::hits::{counts, Hit};
use crate::path::{frame_runs, kernel_runs, tested_paths, Base, Wide};
use crate::{
    bc1_encode_block, bc1_encode_block_alpha, bc1_encode_block_alpha_on,
    bc1_encode_block_alpha_with, bc1_encode_block_level, bc1_encode_block_level_on,
    bc1_encode_block_on, bc1_encode_block_with, bc3_encode_block, bc3_encode_block_raw_cut,
    bc3_encode_block_raw_cut_on, bc3_encode_block_with, bc4_encode_alpha, bc4_encode_alpha_on,
    bc4_encode_block, bc4_encode_block_on, bc5_encode_block, bc5_encode_block_on, Bc1Search, Path,
};
use std::num::NonZeroU8;

/// The `(allow_3color, perceptual)` combinations.
const MODES: [(bool, bool); 4] = [(false, false), (false, true), (true, false), (true, true)];

/// Colour block `i` with the alpha of grey block `i % grey.len()`.
fn with_alpha(colour: &[[[u8; 4]; 16]], grey: &[[u8; 16]]) -> Vec<[[u8; 4]; 16]> {
    colour
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let a = &grey[i % grey.len()];
            std::array::from_fn(|k| [c[k][0], c[k][1], c[k][2], a[k]])
        })
        .collect()
}

/// Grey block `v` on channel 0, its reverse on channel 1, its complement on channels 2 and 3.
fn channels(v: &[u8; 16]) -> [[u8; 4]; 16] {
    std::array::from_fn(|k| [v[k], v[15 - k], !v[k], !v[(k + 5) % 16]])
}

/// `f` on every tested path equals `f(Path::Base)`.
fn as_on_base<const N: usize>(
    paths: &[Path],
    what: impl Fn() -> String,
    f: impl Fn(Path) -> [u8; N],
) {
    let base = f(Path::Base);
    for &p in paths.iter().filter(|&&p| p != Path::Base) {
        assert_eq!(f(p), base, "{} on path {}", what(), p.name());
    }
}

/// Every hidden `_on` entry gives, on every path the machine has, the bytes it gives on `Path::Base`: BC1 in its four
/// modes at both searches and at every level, one-bit alpha with the masks of `gen::alpha_masks`, BC3 at radii 4, 5
/// and 10 and cuts 1 and 128, BC4 (channels 0 and 3, and as BC3's alpha) and BC5 at each radius.
#[test]
fn every_path_gives_the_base_bytes() {
    let paths = tested_paths();
    let grey = gen::bc4_classes(0xE0_0002, 40);
    let blocks = with_alpha(&gen::colour_classes(0xE0_0001, 40), &grey);
    let masks = gen::alpha_masks(0xE0_0003, 4);
    for (i, px) in blocks.iter().enumerate() {
        let mask = masks[i % masks.len()];
        for search in [Bc1Search::Default, Bc1Search::Fast] {
            for (allow_3color, perceptual) in MODES {
                as_on_base(
                    &paths,
                    || {
                        format!("bc1 {search:?}, allow_3color {allow_3color}, perceptual {perceptual}: texels {px:?}")
                    },
                    |p| bc1_encode_block_on(p, px, allow_3color, perceptual, search),
                );
            }
            for perceptual in [false, true] {
                as_on_base(
                    &paths,
                    || {
                        format!("bc1 alpha {search:?}, mask {mask:#06x}, perceptual {perceptual}: texels {px:?}")
                    },
                    |p| bc1_encode_block_alpha_on(p, px, mask, perceptual, search),
                );
                for rad in [4, 5, 10] {
                    for cut in [1, 128] {
                        as_on_base(
                            &paths,
                            || {
                                format!("bc3 {search:?}, radius {rad}, cut {cut}, perceptual {perceptual}: texels {px:?}")
                            },
                            |p| bc3_encode_block_raw_cut_on(p, px, rad, cut, perceptual, search),
                        );
                    }
                }
            }
        }
        for level in LEVELS {
            for (allow_3color, perceptual) in MODES {
                as_on_base(
                    &paths,
                    || {
                        format!("bc1 level {level}, allow_3color {allow_3color}, perceptual {perceptual}: texels {px:?}")
                    },
                    |p| bc1_encode_block_level_on(p, px, level, allow_3color, perceptual),
                );
            }
        }
    }
    for v in &grey {
        let px = channels(v);
        for rad in [4, 5, 10] {
            for channel in [0, 3] {
                as_on_base(
                    &paths,
                    || format!("bc4 channel {channel}, radius {rad}: texels {px:?}"),
                    |p| bc4_encode_block_on(p, &px, channel, rad),
                );
            }
            for cut in [1, 128] {
                as_on_base(
                    &paths,
                    || format!("bc4 alpha radius {rad}, cut {cut}: values {v:?}"),
                    |p| bc4_encode_alpha_on(p, v, rad, cut),
                );
            }
            as_on_base(
                &paths,
                || format!("bc5 radius {rad}: texels {px:?}"),
                |p| bc5_encode_block_on(p, &px, rad),
            );
        }
    }
}

/// Every public function is its `_on` entry on `Path::best()`, argument for argument: the differential tests go
/// through the `_on` entries, so this is what ties the public functions to rgbcx. The arguments differ from each
/// other and from the defaults (radius 4 and 10, cuts 0, 1 and 201, channels 0, 2 and 3, both searches, every level),
/// so a wrapper that drops or swaps one gives other bytes.
#[test]
fn the_public_functions_run_the_best_path() {
    let best = Path::best();
    let grey = gen::bc4_classes(0xE0_0005, 4);
    let blocks = with_alpha(&gen::colour_classes(0xE0_0004, 4), &grey);
    let masks = gen::alpha_masks(0xE0_0006, 4);
    for (i, px) in blocks.iter().enumerate() {
        let mask = masks[i % masks.len()];
        for (allow_3color, perceptual) in MODES {
            let what =
                || format!("allow_3color {allow_3color}, perceptual {perceptual}: texels {px:?}");
            assert_eq!(
                bc1_encode_block(px, allow_3color, perceptual),
                bc1_encode_block_on(best, px, allow_3color, perceptual, Bc1Search::Default),
                "bc1_encode_block, {}",
                what()
            );
            for search in [Bc1Search::Default, Bc1Search::Fast] {
                assert_eq!(
                    bc1_encode_block_with(px, allow_3color, perceptual, search),
                    bc1_encode_block_on(best, px, allow_3color, perceptual, search),
                    "bc1_encode_block_with {search:?}, {}",
                    what()
                );
            }
            for level in LEVELS {
                assert_eq!(
                    bc1_encode_block_level(px, level, allow_3color, perceptual),
                    bc1_encode_block_level_on(best, px, level, allow_3color, perceptual),
                    "bc1_encode_block_level {level}, {}",
                    what()
                );
            }
        }
        for perceptual in [false, true] {
            let what = || format!("mask {mask:#06x}, perceptual {perceptual}: texels {px:?}");
            assert_eq!(
                bc1_encode_block_alpha(px, mask, perceptual),
                bc1_encode_block_alpha_on(best, px, mask, perceptual, Bc1Search::Default),
                "bc1_encode_block_alpha, {}",
                what()
            );
            for search in [Bc1Search::Default, Bc1Search::Fast] {
                assert_eq!(
                    bc1_encode_block_alpha_with(px, mask, perceptual, search),
                    bc1_encode_block_alpha_on(best, px, mask, perceptual, search),
                    "bc1_encode_block_alpha_with {search:?}, {}",
                    what()
                );
            }
            for (rad, cut) in [(4, 1u8), (10, 201)] {
                let what =
                    || format!("radius {rad}, cut {cut}, perceptual {perceptual}: texels {px:?}");
                let nz = NonZeroU8::new(cut).unwrap();
                assert_eq!(
                    bc3_encode_block(px, rad, nz, perceptual),
                    bc3_encode_block_raw_cut_on(best, px, rad, cut, perceptual, Bc1Search::Default),
                    "bc3_encode_block, {}",
                    what()
                );
                for search in [Bc1Search::Default, Bc1Search::Fast] {
                    assert_eq!(
                        bc3_encode_block_with(px, rad, nz, perceptual, search),
                        bc3_encode_block_raw_cut_on(best, px, rad, cut, perceptual, search),
                        "bc3_encode_block_with {search:?}, {}",
                        what()
                    );
                }
                for raw in [0, cut] {
                    assert_eq!(
                        bc3_encode_block_raw_cut(px, rad, raw, perceptual),
                        bc3_encode_block_raw_cut_on(
                            best,
                            px,
                            rad,
                            raw,
                            perceptual,
                            Bc1Search::Default
                        ),
                        "bc3_encode_block_raw_cut, raw cut {raw}, {}",
                        what()
                    );
                }
            }
        }
    }
    for v in &grey {
        let px = channels(v);
        for (channel, rad) in [(0, 4), (2, 10), (3, 5)] {
            assert_eq!(
                bc4_encode_block(&px, channel, rad),
                bc4_encode_block_on(best, &px, channel, rad),
                "bc4_encode_block channel {channel}, radius {rad}: texels {px:?}"
            );
        }
        for (rad, cut) in [(4, 0), (10, 1), (5, 201)] {
            assert_eq!(
                bc4_encode_alpha(v, rad, cut),
                bc4_encode_alpha_on(best, v, rad, cut),
                "bc4_encode_alpha radius {rad}, cut {cut}: values {v:?}"
            );
        }
        for rad in [4, 10] {
            assert_eq!(
                bc5_encode_block(&px, rad),
                bc5_encode_block_on(best, &px, rad),
                "bc5_encode_block radius {rad}: texels {px:?}"
            );
        }
    }
}

/// Each frame entry runs its path's frame once per call: on every tested path, 4 blocks (one from each of the first 4
/// colour classes) through `encode_on` and `encode_alpha_on` (with texels 0 and 15 transparent, so the alpha encode
/// does not hand on to the plain one) count 8 frames on that path and none on another. `Base` counts too: its entry is
/// the match arm.
#[test]
fn the_frames_run_on_their_path() {
    let blocks = &gen::colour_classes(0xE0_0007, 1)[..4];
    for p in tested_paths() {
        let before = frame_runs();
        for px in blocks {
            encode_on::<18>(p, px, true, PLAIN);
            encode_alpha_on::<18>(p, px, 0x8001, PLAIN);
        }
        let after = frame_runs();
        for q in Path::ALL {
            let want = if q == p { 8 } else { 0 };
            assert_eq!(
                after[q as usize] - before[q as usize],
                want,
                "frames run on path {} while encoding on path {}",
                q.name(),
                p.name()
            );
        }
    }
}

/// Each wide kernel runs in its frame: on every tested path, the blocks of `gen::colour_classes(0xE0_0003, 4)`
/// through `bc1_encode_block_on` (3-colour blocks allowed) and through `bc1_encode_block_alpha_on` with every mask of
/// `gen::alpha_masks(0xE0_0004, 8)`. On `Avx2` every kernel of [`Wide::ALL`] runs; on `Base` none does.
#[test]
fn the_wide_kernels_run_in_their_frame() {
    let blocks = gen::colour_classes(0xE0_0003, 4);
    let masks = gen::alpha_masks(0xE0_0004, 8);
    for p in tested_paths() {
        let before = kernel_runs();
        for px in &blocks {
            bc1_encode_block_on(p, px, true, false, Bc1Search::Default);
            for &mask in &masks {
                bc1_encode_block_alpha_on(p, px, mask, false, Bc1Search::Default);
            }
        }
        let after = kernel_runs();
        for k in Wide::ALL {
            let runs = after[k as usize] - before[k as usize];
            match p {
                Path::Base => assert_eq!(runs, 0, "{k:?} ran on path base"),
                Path::Avx2 => assert!(runs > 0, "{k:?} never ran on path avx2"),
            }
        }
    }
}

/// How often this thread took each branch since `before` (an earlier [`counts`]). The counters are per thread, so
/// the tests running beside this one add nothing; taking the difference also leaves out a test that ran earlier on
/// the same thread.
fn since(before: &[(Hit, u64)]) -> Vec<(Hit, u64)> {
    counts()
        .into_iter()
        .zip(before)
        .map(|((h, now), &(_, then))| (h, now - then))
        .collect()
}

fn taken(grew: &[(Hit, u64)], hit: Hit) -> u64 {
    grew.iter().find(|&&(h, _)| h == hit).unwrap().1
}

/// On every tested path, through the `_on` entries, so each path's code takes every branch (the counters start again
/// per path).
#[test]
fn every_ported_branch_is_taken() {
    let grey = gen::bc4_classes(0xC0DE_0001, 600);
    // Every texel on one selector at a total-ordering lookup is rare: 1500 per class reach every arm (the review-shaped
    // narrow clusters take the 4-colour selector 0 arm, the half-step near-solid blocks the 3-colour selector 1 arm).
    let blocks = gen::colour_classes(0xC0DE_0002, 1500);
    let masks = gen::alpha_masks(0xC0DE_0003, 24);
    for p in tested_paths() {
        let before = counts();
        for v in &grey {
            let px = v.map(|a| [a, a ^ 0x5A, 0, a]);
            for rad in [4, 5, 10] {
                bc4_encode_block_on(p, &px, 0, rad);
                bc5_encode_block_on(p, &px, rad);
                for cut in gen::CUTS {
                    bc4_encode_alpha_on(p, v, rad, cut);
                }
                // rgbcx reads a cut of 0 as 128.
                bc4_encode_alpha_on(p, v, rad, 0);
            }
        }
        // The BC1 palettes, and the single-colour and total-ordering tables: the encoder builds those once, on
        // whichever thread asks first, so the test builds them here, on its own.
        for three in [false, true] {
            dual_palette(three, 3, 40, 5, 20, 9, 31);
        }
        build_single_tables();
        build_order_tables();
        // BC1 in both modes with both weight sets: the colour classes, plus every grey and the solid colours at the
        // ends of the range.
        for allow_3color in [false, true] {
            for weights in [PLAIN, PERCEPTUAL] {
                for px in &blocks {
                    encode_on::<18>(p, px, allow_3color, weights);
                }
                for px in &solids() {
                    encode_on::<18>(p, px, allow_3color, weights);
                }
            }
        }
        // BC1 one-bit alpha: every mask class over a sample of the colour classes, both weight sets, and the block
        // whose endpoint search runs all 64 rounds (no class reaches that bound).
        for weights in [PLAIN, PERCEPTUAL] {
            for px in blocks.iter().step_by(7) {
                for &mask in &masks {
                    encode_alpha_on::<18>(p, px, mask, weights);
                }
            }
        }
        encode_alpha_on::<18>(p, &gen::LONG_WALK.0, gen::LONG_WALK.1, PLAIN);
        let never: Vec<_> = since(&before)
            .into_iter()
            .filter(|&(_, n)| n == 0)
            .map(|(h, _)| h)
            .collect();
        assert!(
            never.is_empty(),
            "path {}: branches never taken: {never:?}",
            p.name()
        );
    }
}

/// The greys and the solid colours at the ends of the range, green apart from red and blue.
fn solids() -> Vec<[[u8; 4]; 16]> {
    const ENDS: [u8; 12] = [0, 1, 2, 3, 4, 127, 128, 251, 252, 253, 254, 255];
    ENDS.iter()
        .flat_map(|&v| ENDS.map(|g| [[v, g, v, 255]; 16]))
        .collect()
}

/// Two blocks that reach a total-ordering lookup with every texel on one selector through round 0's least-squares
/// passes, so a level with one round reaches it too (the classes reach those arms only through round 1, found by a
/// search over clusters of 2..=4 nearby colours and traced through the port):
/// - `hist4_all_sel0`, perceptual weights: the principal-axis start (29, 59, 27) to (30, 60, 27) scores 82,176 with
///   the 13 texels of (247, 239, 222) on selector 1. Their least-squares fit puts both endpoints near that colour and
///   rounds to (30, 59, 27) and (31, 60, 27); the low one is exactly (247, 239, 222), and green weighted 183 keeps the
///   three other texels nearest it too, so all 16 sit on selector 0 at 29,208 and the trial is taken. The next pass
///   sees one selector, fits the single-colour pair of the average (both endpoints (30, 59, 27)), and that only ties.
///   So the 4-colour lookup sees [16, 0, 0, 0] with a non-zero error (`Hist16Sel0`), with or without 3-colour blocks.
/// - `hist3_all_sel1`, plain weights, 3-colour blocks: `try_3color_block` starts from round 0's (11, 41, 5) to
///   (12, 43, 5) at 3,040; its first least-squares trial, (10, 42, 5) to (11, 44, 5), scores 1,412, and the second,
///   (10, 42, 6) to (11, 43, 5), 1,156 with every texel on selector 1, the endpoint (90, 174, 41), and is taken. So
///   the 3-colour lookup sees [0, 16, 0] with a non-zero error (`Hist3Sel1`).
fn one_round_blocks() -> [[[u8; 4]; 16]; 2] {
    let (a, b, c) = (
        [247, 239, 222, 255],
        [241, 239, 222, 255],
        [244, 243, 222, 255],
    );
    let hist4_all_sel0 = [a, a, a, a, b, a, a, a, a, a, a, a, a, a, c, b];
    let (p, q, r, s) = (
        [88, 174, 43, 255],
        [90, 174, 37, 255],
        [96, 176, 41, 255],
        [90, 167, 41, 255],
    );
    let hist3_all_sel1 = [p, p, q, p, r, p, p, q, p, q, p, s, p, p, r, r];
    [hist4_all_sel0, hist3_all_sel1]
}

/// The branches a level's search depth rules out, each with the reason from the code.
fn never_at(p: Params) -> Vec<Hit> {
    let mut never = Vec::new();
    if p.rounds == 1 {
        // One round: it starts with `cur_err` at `u32::MAX`, so `round_err <= cur_err` always holds; round 1 (the
        // bounding box) never runs. A least-squares trial is a repeat only of the round's start (the first trial
        // differs from it, and a second trial equal to the first breaks as `LsPassSame`), and the start's error is
        // above `round_err` once a trial was taken, so the repeat is never taken.
        never.extend([
            Hit::RoundWorse,
            Hit::LsRepeatTaken,
            Hit::InitBox,
            Hit::InitBoxSwapR,
            Hit::InitBoxSwapG,
        ]);
    }
    if p.search_rounds == 0 {
        // No endpoint search: its gate needs `search_rounds != 0`.
        never.extend([
            Hit::SearchRun,
            Hit::SearchForbidden,
            Hit::SearchLow,
            Hit::SearchHigh,
            Hit::SearchBetter,
            Hit::SearchStop,
            Hit::SearchThree,
            Hit::SearchFour,
            Hit::SearchRepeat,
        ]);
    } else if p.search_rounds <= 32 {
        // `i` is below `search_rounds` and `prev_improvement_index` at least 0, so `i - prev_improvement_index` is at
        // most 31 and the stop test (`> 32`) never fires.
        never.push(Hit::SearchStop);
    }
    never
}

/// At every level the encode equals rgbcx, with the shadow checks of every `Seen` skip running (HY8's argument holds
/// for any `Params`, Note HY15), and takes the branches level 18 takes except those its search depth rules out; on
/// every tested path, each path's counts compared with its own level 18.
#[test]
fn every_level_matches_rgbcx_with_its_skips_checked() {
    // The once-built tables, on this thread before the first count: a build inside the first pass would count there.
    for three in [false, true] {
        dual_palette(three, 3, 40, 5, 20, 9, 31);
    }
    build_single_tables();
    build_order_tables();
    single_tables();
    order_tables();
    let blocks = gen::colour_classes(0xC0DE_0002, 1500);
    let solids = solids();
    let named = one_round_blocks();
    for p in tested_paths() {
        let mut passes = Vec::new();
        for level in LEVELS {
            let before = counts();
            for allow_3color in [false, true] {
                for perceptual in [false, true] {
                    let weights = if perceptual { PERCEPTUAL } else { PLAIN };
                    for px in blocks.iter().chain(&solids).chain(&named) {
                        assert_eq!(
                            encode_level_on(p, px, level, allow_3color, weights),
                            encoders_sys::bc1_encode_block_level(px, level, allow_3color, perceptual),
                            "path {}, level {level}, allow_3color {allow_3color}, perceptual {perceptual}: texels {px:?}",
                            p.name()
                        );
                    }
                }
            }
            passes.push((level, since(&before)));
        }
        let h18 = &passes.iter().find(|(l, _)| *l == 18).unwrap().1;
        let mut wrong = Vec::new();
        for (level, hl) in &passes {
            let never = never_at(params(*level));
            for &(hit, n) in hl {
                if never.contains(&hit) {
                    if n > 0 {
                        wrong.push(format!(
                            "path {}, level {level} took {hit:?}, which its search depth rules out",
                            p.name()
                        ));
                    }
                } else if taken(h18, hit) > 0 && n == 0 {
                    wrong.push(format!(
                        "path {}, level {level} never took {hit:?}, which level 18 takes",
                        p.name()
                    ));
                }
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }
}

/// The class generators' restated rounding (`round_code`, `round_code_midpoint`, which find the 565 rounding
/// boundaries) is the port's own: `to_5`/`to_6`, and `precise_round_565` of `v / 255`, for every 8-bit value.
#[test]
fn the_class_rounding_is_the_ports() {
    for v in 0..=255u8 {
        assert_eq!(gen::round_code(v, 31), to_5(v as u32), "to_5({v})");
        assert_eq!(gen::round_code(v, 63), to_6(v as u32), "to_6({v})");
        let x = [v as f32 * (1.0f32 / 255.0f32); 3];
        let (codes, _) = precise_round_565(&x, &x);
        assert_eq!(
            gen::round_code_midpoint(v, 31),
            codes[0],
            "5-bit midpoint of {v}"
        );
        assert_eq!(
            gen::round_code_midpoint(v, 63),
            codes[1],
            "6-bit midpoint of {v}"
        );
    }
}

/// Exactly two distinct opaque colours never tie on `encode_bc1_alpha`'s default axis (306, 601, 117) while that
/// axis is in use (rgbcx.cpp 2105-2113 keep it when the power iteration's `k < 2`; 2122-2131 pick the extreme texels
/// with strict `<` and `>`). A tie needs `306 dr + 601 dg + 117 db == 0` for the colours' difference `d`, and `k`
/// rests on `d` and the two counts alone: the covariance is built from the texels' distances to the rounded average,
/// which the base colour and the texel order leave unchanged. So every such `d` (434 of them) with every pair of
/// counts that leaves a texel masked (105) is every case, and each takes the scaled axis, on which the two colours
/// cannot tie. With a third opaque colour the tie does happen: [`gen::DEFAULT_AXIS_TIE`].
#[test]
fn two_opaque_colours_never_tie_on_the_default_alpha_axis() {
    let before = counts();
    let (mut vectors, mut cases) = (0u64, 0u64);
    for dr in -255i32..=255 {
        for dg in -255i32..=255 {
            let rest = 306 * dr + 601 * dg;
            let db = -rest / 117;
            if rest % 117 != 0 || db.abs() > 255 || (dr, dg, db) == (0, 0, 0) {
                continue;
            }
            vectors += 1;
            let d = [dr, dg, db];
            let low = d.map(|x| (-x).max(0));
            let colour = |first: bool| -> [u8; 4] {
                let c: [u8; 3] =
                    std::array::from_fn(|c| (low[c] + if first { d[c] } else { 0 }) as u8);
                [c[0], c[1], c[2], 255]
            };
            for n_a in 1..=14usize {
                for n_b in 1..=15 - n_a {
                    let px: [[u8; 4]; 16] = std::array::from_fn(|i| colour(i < n_a));
                    // Texels n_a + n_b and up are transparent.
                    let mask = (0xFFFFu32 << (n_a + n_b)) as u16;
                    encode_alpha::<18>(Base, &px, mask, PLAIN);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!((vectors, cases), (434, 45_570));
    let grew = since(&before);
    assert_eq!(taken(&grew, Hit::AlphaAxisDefault), 0);
    assert_eq!(taken(&grew, Hit::AlphaAxisScaled), cases);
}

/// [`gen::DEFAULT_AXIS_TIE`] keeps the default axis (`k < 2`), in either order of its tied texels.
#[test]
fn the_default_axis_tie_block_keeps_the_default_axis() {
    let (px, mask) = gen::DEFAULT_AXIS_TIE;
    let mut swapped = px;
    swapped.swap(0, 1);
    let before = counts();
    encode_alpha::<18>(Base, &px, mask, PLAIN);
    encode_alpha::<18>(Base, &swapped, mask, PERCEPTUAL);
    let grew = since(&before);
    assert_eq!(taken(&grew, Hit::AlphaAxisDefault), 2);
    assert_eq!(taken(&grew, Hit::AlphaAxisScaled), 0);
}
