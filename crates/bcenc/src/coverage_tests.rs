//! Every `hit!` branch of the ported code is taken by the class generators.
use crate::bc1::alpha::encode_alpha;
use crate::bc1::encode::encode;
use crate::bc1::palette::{dual_palette, single_tables};
use crate::bc1::{PERCEPTUAL, PLAIN};
use crate::hits::counts;
use crate::{bc4_encode_alpha, bc4_encode_block, bc5_encode_block};

#[path = "../tests/support/gen.rs"]
mod gen;

#[test]
fn every_ported_branch_is_taken() {
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
    // The BC1 palettes and the single-colour tables (built once, on first use).
    for three in [false, true] {
        dual_palette(three, 3, 40, 5, 20, 9, 31);
    }
    single_tables();
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
    let never: Vec<_> = counts()
        .into_iter()
        .filter(|&(_, n)| n == 0)
        .map(|(h, _)| h)
        .collect();
    assert!(never.is_empty(), "branches never taken: {never:?}");
}
