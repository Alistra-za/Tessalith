//! Every `hit!` branch of the ported code is taken by the class generators.
use crate::bc1::encode::encode4;
use crate::bc1::palette::{dual_palette, single_tables};
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
        }
    }
    // The BC1 palettes and the single-colour tables (built once, on first use).
    for three in [false, true] {
        dual_palette(three, 3, 40, 5, 20, 9, 31);
    }
    single_tables();
    // BC1 four-colour: the colour classes, plus every grey and the solid colours at the ends of the range.
    // Every texel on one selector at the total-ordering lookup is rare: 1500 per class reach all three ported arms.
    for px in gen::colour_classes(0xC0DE_0002, 1500) {
        encode4(&px, [1, 1, 1]);
    }
    for v in [0u8, 1, 2, 3, 4, 127, 128, 251, 252, 253, 254, 255] {
        for g in [0u8, 1, 2, 3, 4, 127, 128, 251, 252, 253, 254, 255] {
            encode4(&[[v, g, v, 255]; 16], [1, 1, 1]);
        }
    }
    let never: Vec<_> = counts()
        .into_iter()
        .filter(|&(_, n)| n == 0)
        .map(|(h, _)| h)
        .collect();
    assert!(never.is_empty(), "branches never taken: {never:?}");
}
