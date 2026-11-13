//! `bcenc`'s BC1 one-bit-alpha encode equals the vendored rgbcx (`encode_bc1_alpha(18, ..)` through `encoders_sys`,
//! plain or perceptual weights) on every mask class over the colour classes, and on one hand-built block per early
//! path of the C++.
use bcenc::{bc1_encode_block, bc1_encode_block_alpha};

#[path = "support/bc1_block.rs"]
mod bc1_block;
#[path = "support/gen.rs"]
mod gen;

use bc1_block::{colours, sel};

/// Both outputs, the mask, the flag and the texels on a mismatch, so the block can be replayed.
fn same(px: &[[u8; 4]; 16], mask: u16, perceptual: bool) -> [u8; 8] {
    let got = bc1_encode_block_alpha(px, mask, perceptual);
    let want = encoders_sys::bc1_encode_block_alpha(px, mask, perceptual);
    assert_eq!(
        got, want,
        "bc1 alpha, mask {mask:#06x}, perceptual {perceptual}: texels {px:?}\n  bcenc {got:?}\n  rgbcx {want:?}"
    );
    got
}

/// `px` with every masked texel replaced by random RGB and alpha 0, as a real cut-out texture holds them: nothing
/// under the mask may reach the encode.
fn garbage_under(px: &[[u8; 4]; 16], mask: u16, r: &mut gen::Rng) -> [[u8; 4]; 16] {
    let mut out = *px;
    for (i, t) in out.iter_mut().enumerate() {
        if mask & (1 << i) != 0 {
            *t = [r.byte(), r.byte(), r.byte(), 0];
        }
    }
    out
}

#[test]
fn bc1_alpha_equals_rgbcx_on_masks_and_classes() {
    let mut r = gen::Rng(0xA1FA_0003);
    let masks = gen::alpha_masks(0xA1FA_0001, 48);
    let blocks = gen::colour_classes(0xA1FA_0002, 3);
    for perceptual in [false, true] {
        for &mask in &masks {
            for px in &blocks {
                same(px, mask, perceptual);
                let g = garbage_under(px, mask, &mut r);
                same(&g, mask, perceptual);
            }
        }
    }
}

/// Garbage under the mask (any RGB, any alpha) changes no byte: rgbcx reads a masked texel nowhere.
#[test]
fn bc1_alpha_ignores_the_masked_texels() {
    let masks = gen::alpha_masks(0xA1FA_0004, 8);
    let blocks = gen::colour_classes(0xA1FA_0005, 2);
    let mut r = gen::Rng(0xA1FA_0006);
    for perceptual in [false, true] {
        for &mask in &masks {
            for px in &blocks {
                let base = same(px, mask, perceptual);
                for _ in 0..3 {
                    let g = garbage_under(px, mask, &mut r);
                    assert_eq!(
                        same(&g, mask, perceptual),
                        base,
                        "masked texels changed the block: mask {mask:#06x}, texels {g:?}"
                    );
                }
            }
        }
    }
}

/// The mask alone says which texels are transparent; the alpha channel is read nowhere (rgbcx.cpp 2056-2065 test the
/// mask only). So an opaque texel with alpha 0 still counts: 16 texels of the exact 5:6:5 colour (99, 150, 206) with
/// alpha 0 and only texel 0 unmasked give that colour on both endpoints (0x64B9), texel 0 on selector 0 and the rest on
/// 3. Then random alpha on every texel of the class blocks changes no byte.
#[test]
fn bc1_alpha_reads_the_mask_not_the_alpha_channel() {
    for perceptual in [false, true] {
        assert_eq!(
            same(&[[99, 150, 206, 0]; 16], 0xFFFE, perceptual),
            [0xB9, 0x64, 0xB9, 0x64, 0xFC, 0xFF, 0xFF, 0xFF]
        );
    }
    let mut r = gen::Rng(0xA1FA_000D);
    let masks = gen::alpha_masks(0xA1FA_000E, 4);
    let blocks = gen::colour_classes(0xA1FA_000F, 2);
    for perceptual in [false, true] {
        for (i, px) in blocks.iter().enumerate() {
            let mask = masks[i % masks.len()];
            let mut any = *px;
            any.iter_mut().for_each(|t| t[3] = r.byte());
            assert_eq!(
                same(&any, mask, perceptual),
                same(px, mask, perceptual),
                "alpha changed the block: mask {mask:#06x}, texels {any:?}"
            );
        }
    }
}

/// Mask 0 is `encode_bc1(level, .., true, false)` (rgbcx.cpp 2040-2044): the plain encode with three-colour blocks.
#[test]
fn bc1_alpha_with_no_bit_equals_the_plain_encode() {
    for perceptual in [false, true] {
        for px in gen::colour_classes(0xA1FA_0007, 20) {
            assert_eq!(
                same(&px, 0, perceptual),
                bc1_encode_block(&px, true, perceptual),
                "texels {px:?}"
            );
        }
    }
}

/// Mask 0xFFFF (rgbcx.cpp 2047-2053): both colours 0 and every selector 3, whatever the texels.
#[test]
fn bc1_alpha_with_every_bit_is_all_transparent() {
    let mut r = gen::Rng(0xA1FA_0008);
    for perceptual in [false, true] {
        for px in gen::colour_classes(0xA1FA_0009, 2) {
            let g = garbage_under(&px, 0xFFFF, &mut r);
            assert_eq!(
                same(&g, 0xFFFF, perceptual),
                [0, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]
            );
        }
    }
}

/// The opaque texels all one colour that 5:6:5 cannot hold. The C++ has no solid path here: the extents are 0, so the
/// power iteration stays at 0 and `k < 2` keeps the default axis (rgbcx.cpp 2105-2113); every dot product ties, so the
/// first opaque texel is both the low and the high texel (strict `<` and `>`, 2122-2131) and the endpoints are equal.
/// The palette's three opaque entries are then the same colour, so every opaque texel stays on selector 0 (strict `<`,
/// 2023) with a non-zero error; the least-squares solve sees one selector only and is singular (det 0, 903), so the
/// pass falls back to the `half` single-colour match of the opaque average (2147-2153).
#[test]
fn bc1_alpha_solid_opaque_texels_off_the_565_grid() {
    let mut r = gen::Rng(0xA1FA_000A);
    for mask in [0x0001u16, 0x8000, 0x0F0F, 0x7FFE, 0xFFFE] {
        for colour in [
            [100u8, 150, 201],
            [1, 2, 3],
            [254, 253, 252],
            [128, 128, 128],
        ] {
            let px = garbage_under(&[[colour[0], colour[1], colour[2], 255]; 16], mask, &mut r);
            for perceptual in [false, true] {
                same(&px, mask, perceptual);
            }
        }
    }
}

/// The opaque texels all one exact 5:6:5 colour, (99, 150, 206) = codes (12, 37, 25): as above the endpoints are equal
/// and on the colour, so the first selector search scores 0 and both refinement loops stop on `err == 0` before their
/// first pass (rgbcx.cpp 2141, 2174). `bc1_encode3` then packs two equal colours (`lc16 > hc16` is false, 1499): a
/// 3-colour block with colour 0 == colour 1, opaque texels on selector 0 and masked ones on 3.
#[test]
fn bc1_alpha_solid_opaque_texels_on_the_565_grid_pack_equal_colours() {
    let mut r = gen::Rng(0xA1FA_000B);
    let mask = 0b0110_0000_0000_0110u16;
    let px = garbage_under(&[[99, 150, 206, 255]; 16], mask, &mut r);
    for perceptual in [false, true] {
        let b = same(&px, mask, perceptual);
        let c = (12u16 << 11) | (37 << 5) | 25;
        assert_eq!(colours(&b), (c, c));
        for i in 0..16 {
            let want = if mask & (1 << i) != 0 { 3 } else { 0 };
            assert_eq!(sel(&b, i), want, "texel {i}");
        }
    }
}

/// Red against blue: the R-B covariance is negative, so red's extent is negated and the axis runs from red to blue
/// whatever the texel order (rgbcx.cpp 2092-2093, 2117-2132): red is the low endpoint. Both colours are exact, so the
/// error is 0 and neither loop runs; packed red (0xF800) is above blue (0x001F), so `bc1_encode3` swaps the colours
/// and selectors 0 and 1, 3 staying 3 (1499-1517): blue is colour 0, red texels read selector 1, blue ones 0. Black
/// against white has no negative covariance, black is the low endpoint (0x0000 below 0xFFFF) and the block stays in
/// order: black texels on selector 0, white on 1.
#[test]
fn bc1_alpha_orders_endpoints_for_three_colour_mode() {
    let mask = 0b0110_0110_0110_0110u16;
    const RED: [u8; 3] = [255, 0, 0];
    const BLUE: [u8; 3] = [0, 0, 255];
    const BLACK: [u8; 3] = [0, 0, 0];
    const WHITE: [u8; 3] = [255, 255, 255];
    // (first half, second half, colour 0, colour 1, selector of the first half, of the second half)
    let cases = [
        (RED, BLUE, 0x001F, 0xF800, 1, 0),
        (BLUE, RED, 0x001F, 0xF800, 0, 1),
        (BLACK, WHITE, 0x0000, 0xFFFF, 0, 1),
        (WHITE, BLACK, 0x0000, 0xFFFF, 1, 0),
    ];
    for (a, b, c0, c1, sa, sb) in cases {
        let px: [[u8; 4]; 16] = std::array::from_fn(|i| {
            let c = if i < 8 { a } else { b };
            [c[0], c[1], c[2], if mask & (1 << i) != 0 { 0 } else { 255 }]
        });
        for perceptual in [false, true] {
            let out = same(&px, mask, perceptual);
            assert_eq!(colours(&out), (c0, c1), "{a:?} then {b:?}");
            for i in 0..16 {
                let want = if mask & (1 << i) != 0 {
                    3
                } else if i < 8 {
                    sa
                } else {
                    sb
                };
                assert_eq!(sel(&out, i), want, "{a:?} then {b:?}, texel {i}");
            }
        }
    }
}

/// [`gen::LONG_WALK`]: the endpoint search is still improving when its 64-round bound stops it (rgbcx.cpp 2174), so
/// the bound decides the bytes.
#[test]
fn bc1_alpha_endpoint_search_runs_all_64_rounds() {
    let (px, mask) = gen::LONG_WALK;
    for perceptual in [false, true] {
        same(&px, mask, perceptual);
    }
}

/// [`gen::DEFAULT_AXIS_TIE`] (task 5 reviews): two distinct opaque colours tie on the default axis, so the texel order
/// picks the low endpoint (rgbcx.cpp 2122-2126, strict `<`). Texels 0 = (100, 100, 100) and 1 = (85, 109, 93) both dot
/// to 102,400; texel 2 = (93, 112, 104) dots to 107,938 and is the high endpoint, (11, 28, 13), either way. As given,
/// the low endpoint is texel 0's (12, 25, 12); with texels 0 and 1 exchanged it is texel 1's (10, 27, 11). The refits
/// and endpoint steps then start from different places and, with plain weights, end on different blocks. With
/// perceptual weights both orders end on the same colours.
#[test]
fn bc1_alpha_texel_order_breaks_a_tie_on_the_default_axis() {
    let (px, mask) = gen::DEFAULT_AXIS_TIE;
    let mut exchanged = px;
    exchanged.swap(0, 1);

    let as_given = same(&px, mask, false);
    assert_eq!(colours(&as_given), (0x5B6C, 0x632C));
    assert_eq!([0, 1, 2].map(|i| sel(&as_given, i)), [1, 0, 0]);
    let other = same(&exchanged, mask, false);
    assert_eq!(colours(&other), (0x4B8B, 0x634C));
    assert_eq!([0, 1, 2].map(|i| sel(&other, i)), [2, 1, 1]);

    assert_eq!(colours(&same(&px, mask, true)), (0x5B6C, 0x632C));
    assert_eq!(colours(&same(&exchanged, mask, true)), (0x5B6C, 0x632C));
}

/// A least-squares pass whose result is the current endpoints swapped (task 5 reviews), traced and explained from the
/// C++. Two opaque texels, 0 = (47, 20, 19) and 15 = (48, 20, 21) (mask 0x7FFE). The extents are 1, 0, 2, so the
/// power iteration stays far below 2 and the default axis (306, 601, 117) is kept (rgbcx.cpp 2105-2113); texel 0
/// dots lower, so the endpoints are low (6, 5, 2), high (6, 5, 3) (2134-2135). Both texels are nearest the midpoint
/// (49, 20, 20), error 14. One selector makes the fit singular, and the fallback is the `half` match of the opaque
/// average (48, 20, 20) (2147-2153): low (6, 5, 3), high (6, 5, 2), the same two endpoints the other way round. That
/// is not equal channel by channel (2160), its palette has the same midpoint, so its error is 14 again and `err2 >=
/// err` ends the passes (2165). No endpoint step improves, so the block keeps (6, 5, 2), (6, 5, 3) with both texels on
/// the midpoint.
#[test]
fn bc1_alpha_least_squares_pass_returns_the_endpoints_swapped() {
    let mut px = [[0, 0, 0, 0]; 16];
    px[0] = [47, 20, 19, 255];
    px[15] = [48, 20, 21, 255];
    for perceptual in [false, true] {
        same(&px, 0x7FFE, perceptual);
    }
    let b = same(&px, 0x7FFE, false);
    assert_eq!(colours(&b), (0x30A2, 0x30A3));
    for i in 0..16 {
        let want = if i == 0 || i == 15 { 2 } else { 3 };
        assert_eq!(sel(&b, i), want, "texel {i}");
    }
}

/// An improving +-1 endpoint step after which the packed colours change order (task 5 reviews), traced and explained
/// from the C++. Opaque texels 4 and 8 = (226, 21, 162) and 5 = (226, 21, 155) (mask 0xFECF). Only blue varies, so
/// the default axis is kept; texel 5 dots lowest and texel 4 highest (texel 8 ties it, strict `>`, 2127), giving low
/// (27, 5, 19), high (27, 5, 20): packed 0xD8B3 below 0xD8B4. Texel 5 sits on the low endpoint, 4 and 8 on the
/// midpoint, error 114; the least-squares pass scores 140 and stops (2165). Round 0, step 1 raises the low red code
/// (2177-2182): low (28, 5, 19), packed 0xE0B3, now above the high endpoint. The midpoint's red moves from 222 to
/// 226 (NVIDIA) and 227 (AMD), on the texels' 226, so 114 falls to 70 (2188); nothing improves after it.
/// `bc1_encode3` then swaps the colours and selectors 0 and 1: colour 0 is (27, 5, 20) and texel 5 reads selector 1.
#[test]
fn bc1_alpha_improving_step_reverses_the_packed_order() {
    let mut px = [[0, 0, 0, 0]; 16];
    px[4] = [226, 21, 162, 255];
    px[5] = [226, 21, 155, 255];
    px[8] = [226, 21, 162, 255];
    for perceptual in [false, true] {
        same(&px, 0xFECF, perceptual);
    }
    let b = same(&px, 0xFECF, false);
    assert_eq!(colours(&b), (0xD8B4, 0xE0B3));
    for i in 0..16 {
        let want = match i {
            4 | 8 => 2,
            5 => 1,
            _ => 3,
        };
        assert_eq!(sel(&b, i), want, "texel {i}");
    }
}

/// A full-noise block with four texels masked: with perceptual weights every refit finds a strictly lower error up to
/// the eighth pass, the last the bound allows (rgbcx.cpp 2141), and that pass changes the bytes. Found among the
/// coverage test's inputs by setting the bound to 7; the six blocks that scan reported all use perceptual weights,
/// where the unweighted fit and the weighted selector search disagree.
#[test]
fn bc1_alpha_least_squares_runs_all_8_passes() {
    let px = [
        [38, 64, 131, 255],
        [194, 240, 228, 255],
        [217, 137, 237, 255],
        [187, 155, 181, 255],
        [157, 130, 138, 255],
        [33, 163, 68, 255],
        [6, 224, 151, 255],
        [183, 57, 52, 255],
        [0, 230, 88, 255],
        [245, 217, 157, 255],
        [138, 34, 176, 255],
        [127, 135, 34, 255],
        [144, 131, 225, 255],
        [172, 199, 144, 255],
        [128, 32, 124, 255],
        [199, 110, 217, 255],
    ];
    for perceptual in [false, true] {
        same(&px, 0x00B1, perceptual);
    }
}

/// The other side of the same bound: a full-noise block with three texels masked where, with perceptual weights, a
/// ninth least-squares pass would still lower the error, so stopping after eight (rgbcx.cpp 2141) decides the bytes.
/// Found by setting the bound to 9 and scanning the coverage test's inputs plus uniform masks against rgbcx.
#[test]
fn bc1_alpha_least_squares_stops_after_8_passes() {
    let px = [
        [35, 68, 24, 255],
        [223, 118, 227, 255],
        [218, 16, 230, 255],
        [186, 14, 205, 255],
        [59, 158, 21, 255],
        [114, 246, 57, 255],
        [0, 0, 0, 255],
        [231, 181, 214, 255],
        [4, 6, 47, 255],
        [162, 29, 188, 255],
        [182, 46, 82, 255],
        [57, 203, 18, 255],
        [180, 111, 175, 255],
        [235, 95, 136, 255],
        [84, 26, 4, 255],
        [114, 84, 53, 255],
    ];
    for perceptual in [false, true] {
        same(&px, 0x5200, perceptual);
    }
}

/// One opaque texel: low and high texel are the same, and the rest is as in the solid tests above.
#[test]
fn bc1_alpha_single_opaque_texel() {
    let mut r = gen::Rng(0xA1FA_000C);
    for i in 0..16 {
        let mask = !(1u16 << i);
        for _ in 0..8 {
            let px = garbage_under(&[[r.byte(), r.byte(), r.byte(), 255]; 16], mask, &mut r);
            for perceptual in [false, true] {
                same(&px, mask, perceptual);
            }
        }
    }
}

/// 50 million blocks over 16 threads, each block with a mask drawn from [`gen::alpha_masks`], garbage under it on odd
/// blocks, half with each weight set; generated in batches so memory stays small. Run with `-- --ignored`.
#[test]
#[ignore = "50 million blocks, about 20 seconds on 16 threads"]
fn bc1_alpha_equals_rgbcx_on_50_million_blocks() {
    const THREADS: u64 = 16;
    const PER_THREAD: usize = 3_125_000; // 16 x 3.125 million = 50 million
    const PER_CLASS: usize = 1_000;
    let start = std::time::Instant::now();
    let totals: Vec<usize> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                s.spawn(move || {
                    let perceptual = t % 2 == 1;
                    let mut r = gen::Rng(0x5_0000_0000 + t * 0x9E37_79B9);
                    let masks = gen::alpha_masks(0x6_0000_0000 + t, 4096);
                    let mut n = 0;
                    let mut batch = 0u64;
                    while n < PER_THREAD {
                        let seed = 0x7_0000_0000 + (t * 10_000 + batch) * 0x9E37_79B9;
                        for px in gen::colour_classes(seed, PER_CLASS) {
                            if n == PER_THREAD {
                                break;
                            }
                            let mask = masks[r.below(masks.len() as u64) as usize];
                            if n % 2 == 1 {
                                same(&garbage_under(&px, mask, &mut r), mask, perceptual);
                            } else {
                                same(&px, mask, perceptual);
                            }
                            n += 1;
                        }
                        batch += 1;
                    }
                    n
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let total: usize = totals.iter().sum();
    assert_eq!(total, 50_000_000);
    eprintln!(
        "{total} blocks (25 million per weight set), 0 mismatches, {:.1} s",
        start.elapsed().as_secs_f64()
    );
}
