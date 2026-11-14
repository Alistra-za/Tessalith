//! The level-taking rgbcx wrappers: level 18 is the old wrapper, the lower levels differ from it, and the level
//! reaches only the colour half of BC1, one-bit alpha (mask 0 only) and BC3.

use encoders_sys::{
    bc1_encode_block, bc1_encode_block_alpha, bc1_encode_block_alpha_level, bc1_encode_block_level,
    bc3_encode_block, bc3_encode_block_level, RGBCX_MAX_LEVEL,
};

type Block = [[u8; 4]; 16];

const FAST_LEVELS: [u32; 4] = [10, 12, 13, 15];
const MODES: [(bool, bool); 4] = [(true, false), (true, true), (false, false), (false, true)];

fn blocks() -> Vec<Block> {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as u32
    };
    let mut v: Vec<Block> = (0..4000)
        .map(|_| {
            let mut b = [[0u8; 4]; 16];
            for p in b.iter_mut() {
                for c in p.iter_mut() {
                    *c = next() as u8;
                }
            }
            b
        })
        .collect();
    let mut ramp = [[0u8; 4]; 16];
    for (i, p) in ramp.iter_mut().enumerate() {
        let g = (i * 17) as u8;
        *p = [g, g / 2, 255 - g, 255];
    }
    v.push(ramp);
    let mut two = [[10, 200, 30, 255]; 16];
    for p in two.iter_mut().skip(8) {
        *p = [240, 20, 90, 255];
    }
    v.push(two);
    v.push([[128, 128, 128, 255]; 16]);
    v
}

#[test]
fn level_18_is_the_old_wrapper() {
    for b in blocks() {
        for (a3, p) in MODES {
            assert_eq!(
                bc1_encode_block_level(&b, 18, a3, p),
                bc1_encode_block(&b, a3, false, p)
            );
        }
        for mask in [0u16, 0x8001, 0xFFFF] {
            for p in [false, true] {
                assert_eq!(
                    bc1_encode_block_alpha_level(&b, mask, 18, p),
                    bc1_encode_block_alpha(&b, mask, p)
                );
            }
        }
        for p in [false, true] {
            assert_eq!(
                bc3_encode_block_level(&b, 5, 128, 18, p),
                bc3_encode_block(&b, 5, 128, p)
            );
        }
    }
}

#[test]
fn every_fast_level_changes_some_block() {
    let all = blocks();
    for level in FAST_LEVELS {
        let differs = all.iter().any(|b| {
            MODES.iter().any(|&(a3, p)| {
                bc1_encode_block_level(b, level, a3, p) != bc1_encode_block_level(b, 18, a3, p)
            })
        });
        assert!(
            differs,
            "level {level} gave level 18's bytes for every block"
        );
    }
}

#[test]
fn the_level_reaches_only_opaque_alpha() {
    for b in blocks() {
        for level in FAST_LEVELS {
            for p in [false, true] {
                assert_eq!(
                    bc1_encode_block_alpha_level(&b, 0, level, p),
                    bc1_encode_block_level(&b, level, true, p)
                );
                for mask in [0x8001u16, 0x00F0] {
                    assert_eq!(
                        bc1_encode_block_alpha_level(&b, mask, level, p),
                        bc1_encode_block_alpha_level(&b, mask, 18, p)
                    );
                }
            }
        }
    }
}

#[test]
fn the_bc3_level_moves_only_the_colour() {
    for b in blocks() {
        for level in FAST_LEVELS {
            for p in [false, true] {
                let at = bc3_encode_block_level(&b, 5, 128, level, p);
                let top = bc3_encode_block_level(&b, 5, 128, 18, p);
                assert_eq!(at[..8], top[..8]);
                assert_eq!(at[8..], bc1_encode_block_level(&b, level, false, p));
            }
        }
    }
}

#[test]
#[should_panic(expected = "level 19")]
fn a_level_above_18_panics() {
    assert_eq!(RGBCX_MAX_LEVEL, 18);
    bc1_encode_block_level(&[[0; 4]; 16], 19, true, false);
}
