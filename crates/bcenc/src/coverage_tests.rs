//! Every `hit!` branch of the ported code is taken by the class generators.
use crate::bc1::alpha::encode_alpha;
use crate::bc1::encode::encode;
use crate::bc1::palette::{build_single_tables, dual_palette};
use crate::bc1::{build_order_tables, precise_round_565, to_5, to_6};
use crate::bc1::{PERCEPTUAL, PLAIN};
use crate::gen;
use crate::hits::{counts, Hit};
use crate::{bc4_encode_alpha, bc4_encode_block, bc5_encode_block};

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

#[test]
fn every_ported_branch_is_taken() {
    let before = counts();
    for v in gen::bc4_classes(0xC0DE_0001, 600) {
        let px = v.map(|a| [a, a ^ 0x5A, 0, a]);
        for rad in [4, 5, 10] {
            bc4_encode_block(&px, 0, rad);
            bc5_encode_block(&px, rad);
            for cut in gen::CUTS {
                bc4_encode_alpha(&v, rad, cut);
            }
            // rgbcx reads a cut of 0 as 128.
            bc4_encode_alpha(&v, rad, 0);
        }
    }
    // The BC1 palettes, and the single-colour and total-ordering tables: the encoder builds those once, on whichever
    // thread asks first, so the test builds them here, on its own.
    for three in [false, true] {
        dual_palette(three, 3, 40, 5, 20, 9, 31);
    }
    build_single_tables();
    build_order_tables();
    // BC1 in both modes with both weight sets: the colour classes, plus every grey and the solid colours at the ends
    // of the range. Every texel on one selector at a total-ordering lookup is rare: 1500 per class reach every arm
    // (the review-shaped narrow clusters take the 4-colour selector 0 arm, the half-step near-solid blocks the
    // 3-colour selector 1 arm).
    let blocks = gen::colour_classes(0xC0DE_0002, 1500);
    for allow_3color in [false, true] {
        for weights in [PLAIN, PERCEPTUAL] {
            for px in &blocks {
                encode(px, allow_3color, weights);
            }
            for v in [0u8, 1, 2, 3, 4, 127, 128, 251, 252, 253, 254, 255] {
                for g in [0u8, 1, 2, 3, 4, 127, 128, 251, 252, 253, 254, 255] {
                    encode(&[[v, g, v, 255]; 16], allow_3color, weights);
                }
            }
        }
    }
    // BC1 one-bit alpha: every mask class over a sample of the colour classes, both weight sets, and the block whose
    // endpoint search runs all 64 rounds (no class reaches that bound).
    let masks = gen::alpha_masks(0xC0DE_0003, 24);
    for weights in [PLAIN, PERCEPTUAL] {
        for px in blocks.iter().step_by(7) {
            for &mask in &masks {
                encode_alpha(px, mask, weights);
            }
        }
    }
    encode_alpha(&gen::LONG_WALK.0, gen::LONG_WALK.1, PLAIN);
    let never: Vec<_> = since(&before)
        .into_iter()
        .filter(|&(_, n)| n == 0)
        .map(|(h, _)| h)
        .collect();
    assert!(never.is_empty(), "branches never taken: {never:?}");
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
                    encode_alpha(&px, mask, PLAIN);
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
    encode_alpha(&px, mask, PLAIN);
    encode_alpha(&swapped, mask, PERCEPTUAL);
    let grew = since(&before);
    assert_eq!(taken(&grew, Hit::AlphaAxisDefault), 2);
    assert_eq!(taken(&grew, Hit::AlphaAxisScaled), 0);
}
