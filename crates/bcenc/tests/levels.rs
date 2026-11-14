//! `bcenc`'s BC1 encode at every level it ports (`bcenc::bc1::LEVELS`) equals the vendored rgbcx at that level
//! (`encode_bc1(level, .., allow_3color, false)` through `encoders_sys`, plain or perceptual weights), and so do the
//! two callers rgbcx passes the level to: one-bit alpha with no bit set and BC3's colour half (Note HY15). The inputs
//! are the colour classes in the four modes and every grey, solid and as a two-value block.
use bcenc::bc1::LEVELS;
use bcenc::{bc1_encode_block_level, bc4_encode_alpha};

#[path = "support/gen.rs"]
mod gen;

/// The `(allow_3color, perceptual)` combinations.
const MODES: [(bool, bool); 4] = [(false, false), (false, true), (true, false), (true, true)];

/// The colour classes, every solid grey and every grey block of two adjacent values.
fn blocks() -> Vec<[[u8; 4]; 16]> {
    let mut b = gen::colour_classes(0xC0_0001, 300);
    for v in 0..=255u8 {
        b.push([[v, v, v, 255]; 16]);
        let w = v.saturating_add(1);
        b.push(std::array::from_fn(|i| {
            let g = if i % 3 == 0 { w } else { v };
            [g, g, g, 255]
        }));
    }
    b
}

#[test]
fn every_level_is_rgbcx() {
    let blocks = blocks();
    for level in LEVELS {
        for (allow_3color, perceptual) in MODES {
            for px in &blocks {
                let got = bc1_encode_block_level(px, level, allow_3color, perceptual);
                let want =
                    encoders_sys::bc1_encode_block_level(px, level, allow_3color, perceptual);
                assert_eq!(
                    got, want,
                    "bc1 level {level}, allow_3color {allow_3color}, perceptual {perceptual}: texels {px:?}\n  bcenc {got:?}\n  rgbcx {want:?}"
                );
            }
        }
    }
}

/// rgbcx's `encode_bc1_alpha(level, ..)` with no bit set is `encode_bc1(level, .., true, false)`.
#[test]
fn opaque_alpha_at_every_level_is_rgbcx() {
    let blocks = blocks();
    for level in LEVELS {
        for perceptual in [false, true] {
            for px in &blocks {
                let got = bc1_encode_block_level(px, level, true, perceptual);
                let want = encoders_sys::bc1_encode_block_alpha_level(px, 0, level, perceptual);
                assert_eq!(
                    got, want,
                    "bc1 alpha level {level}, perceptual {perceptual}: texels {px:?}\n  bcenc {got:?}\n  rgbcx {want:?}"
                );
            }
        }
    }
}

/// rgbcx's `encode_bc3_hq(level, ..)` is the BC4 alpha search, then `encode_bc1(level, .., false, false)`.
#[test]
fn bc3_at_every_level_is_rgbcx() {
    // Alpha blocks of every class over the colour classes, so the alpha half is not trivial.
    let alpha = gen::alpha_classes(0xC0_0002, 10);
    let blocks: Vec<[[u8; 4]; 16]> = gen::colour_classes(0xC0_0003, 30)
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let a = &alpha[i % alpha.len()];
            std::array::from_fn(|k| [c[k][0], c[k][1], c[k][2], a[k]])
        })
        .collect();
    for level in LEVELS {
        for perceptual in [false, true] {
            for px in &blocks {
                let a: [u8; 16] = std::array::from_fn(|k| px[k][3]);
                let colour = bc1_encode_block_level(px, level, false, perceptual);
                for rad in [4, 5, 10] {
                    for cut in [1u8, 128, 255] {
                        let want =
                            encoders_sys::bc3_encode_block_level(px, rad, cut, level, perceptual);
                        assert_eq!(
                            want[..8],
                            bc4_encode_alpha(&a, rad, cut),
                            "bc3 alpha, level {level}, radius {rad}, cut {cut}: texels {px:?}"
                        );
                        assert_eq!(
                            want[8..],
                            colour,
                            "bc3 colour, level {level}, perceptual {perceptual}: texels {px:?}"
                        );
                    }
                }
            }
        }
    }
}
