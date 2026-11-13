//! `bcenc`'s BC1 encode equals the vendored rgbcx (`encode_bc1(18, .., allow_3color, false)` through `encoders_sys`,
//! plain or perceptual weights) on every block class and on solid colours. Solid blocks take rgbcx's
//! `encode_bc1_solid_block`, so the solid tests also prove the single-colour tables (`equals_1` with three-colour off,
//! `half` too with it on).
use bcenc::bc1::palette::{dual_err, dual_palette};
use bcenc::bc1::{PERCEPTUAL, PLAIN};
use bcenc::bc1_encode_block;

#[path = "support/bc1_block.rs"]
mod bc1_block;
#[path = "support/gen.rs"]
mod gen;

use bc1_block::{colours, sel};

/// The `(allow_3color, perceptual)` combinations.
const MODES: [(bool, bool); 4] = [(false, false), (false, true), (true, false), (true, true)];

/// Both outputs, the flags and the texels on a mismatch, so the block can be replayed.
fn same(px: &[[u8; 4]; 16], allow_3color: bool, perceptual: bool) -> [u8; 8] {
    let got = bc1_encode_block(px, allow_3color, perceptual);
    let want = encoders_sys::bc1_encode_block(px, allow_3color, false, perceptual);
    assert_eq!(
        got, want,
        "bc1, allow_3color {allow_3color}, perceptual {perceptual}: texels {px:?}\n  bcenc {got:?}\n  rgbcx {want:?}"
    );
    got
}

fn solid(r: u8, g: u8, b: u8) -> [[u8; 4]; 16] {
    [[r, g, b, 255]; 16]
}

#[test]
fn bc1_equals_rgbcx_on_block_classes() {
    let blocks = gen::colour_classes(0xBC10_0001, 300);
    for (allow_3color, perceptual) in MODES {
        for px in &blocks {
            same(px, allow_3color, perceptual);
        }
    }
}

/// rgbcx ignores alpha in both modes (its solid test and every error sum read RGB only): random alpha changes neither
/// side.
#[test]
fn bc1_ignores_alpha() {
    let mut rng = gen::Rng(0xBC10_0003);
    let mut blocks = gen::colour_classes(0xBC10_0004, 20);
    // Solid RGB with mixed alpha takes the solid path whatever the alpha.
    for _ in 0..200 {
        blocks.push(solid(rng.byte(), rng.byte(), rng.byte()));
    }
    for (allow_3color, perceptual) in MODES {
        for px in &blocks {
            let opaque = px.map(|t| [t[0], t[1], t[2], 255]);
            let mut mixed = *px;
            mixed.iter_mut().for_each(|t| t[3] = rng.byte());
            let got = same(&mixed, allow_3color, perceptual);
            assert_eq!(
                got,
                bc1_encode_block(&opaque, allow_3color, perceptual),
                "alpha changed the output (allow_3color {allow_3color}, perceptual {perceptual}): texels {mixed:?}"
            );
        }
    }
}

/// The block the task 3 review built (Note HY3): the principal-axis round ends on equal endpoints, the bounding-box
/// round ties it with every texel on the low endpoint, and the tie goes to the later round. So the total-ordering
/// lookup sees the histogram [16, 0, 0, 0] with a non-zero error (`hist4::lookup_total_ordering_index`'s first arm).
/// It runs in all four flag combinations; the four-colour pass is the one that reaches the arm.
#[test]
fn bc1_four_colour_block_with_every_texel_on_the_low_endpoint() {
    let (a, b, c, d) = (
        [20, 87, 33, 255],
        [13, 95, 33, 255],
        [14, 88, 29, 255],
        [18, 90, 36, 255],
    );
    let px = [a, b, c, c, c, c, c, c, c, d, d, d, d, d, d, d];
    for (allow_3color, perceptual) in MODES {
        same(&px, allow_3color, perceptual);
    }
}

/// A block that reaches `hist3::lookup_total_ordering_index`'s selector 1 arm (Note HY4), found by a search over the
/// half-step near-solid class and explained from the C++: 14 texels on the exact 565 colour (27, 18, 26), one 4 lower
/// in blue, one 4 lower in red. The bounding-box round's inset is negative (range 4 < 8), so its low endpoint rounds
/// a whole code down in red and blue, to (26, 18, 25); its high endpoint is (27, 18, 26). That round wins (its first
/// 4-colour search scores 38, the principal-axis round, with both endpoints on the exact colour, 64), so
/// `try_3color_block` starts from those endpoints: the 14 texels sit on the high endpoint, and each moved texel is
/// 4 from the high endpoint and from the midpoint (218, 73, 210) but 8 from the low one, so the strict `<` keeps it
/// on selector 1. The least-squares pass is singular and its single-colour trial (the exact colour on both
/// endpoints) only ties, so the 3-colour orderings see the histogram [0, 16, 0] with error 64.
#[test]
fn bc1_three_colour_block_with_every_texel_on_the_high_endpoint() {
    let (a, b, c) = (
        [222, 73, 214, 255],
        [222, 73, 210, 255],
        [218, 73, 214, 255],
    );
    let px = [a, b, a, a, a, a, a, c, a, a, a, a, a, a, a, a];
    for (allow_3color, perceptual) in MODES {
        same(&px, allow_3color, perceptual);
    }
}

/// An accepted three-colour endpoint-search step that reverses the packed colour order (task 4 Codex review, section
/// 5), traced and explained from the C++. Eleven texels P = (236, 3, 85), five Q = (236, 3, 148). The four-colour
/// rounds and orderings end at error 463; `try_3color_block` (rgbcx.cpp 1713-1881) starts from round 1's initial
/// endpoints and its orderings reach low (29, 1, 3), high (29, 1, 18) with error 342: Q on the high endpoint, P on the
/// midpoint, blue 86 against 85 (both decoders). 342 < 463, so the block is three-colour. Packed, low 0xE823 is below
/// high 0xE832. The endpoint search (2589-2652, `m_3color`) takes round 3, step (-1, 0, 0) on the high endpoint
/// (2617-2619): high (28, 1, 18) packs to 0xE032, now below low. The midpoint's red falls from 239 to 235, one from P's
/// 236, so each P texel's error drops from 22 to 6, and each Q texel's rises from 20 to 52 (red 231): 342 - 176 + 160
/// = 326 < 342, accepted (2636). No later step improves, so `bc1_encode3` (1493-1529) swaps the colours and selectors 0
/// and 1: colour 0 is (28, 1, 18), Q reads selector 0, P stays on 2. Without the crossing step the colours would be
/// 0xE823, 0xE832 with Q on selector 1. The class `blue_pair` varies this shape.
#[test]
fn bc1_three_colour_search_step_crosses_the_packed_order() {
    let (p, q) = ([236, 3, 85, 255], [236, 3, 148, 255]);
    let px = [p, q, p, p, q, q, p, p, p, p, p, p, q, p, q, p];
    for (allow_3color, perceptual) in MODES {
        same(&px, allow_3color, perceptual);
    }
    let b = same(&px, true, false);
    assert_eq!(colours(&b), (0xE032, 0xE823));
    for (i, t) in px.iter().enumerate() {
        assert_eq!(sel(&b, i), if *t == q { 0 } else { 2 }, "texel {i}");
    }
}

/// A non-solid block whose first three-colour selectors are all the midpoint, then a singular fit whose single-colour
/// fallback ties (task 4 Codex review, section 5), traced and explained from the C++. Fifteen texels (111, 89, 122),
/// texel 4 (111, 90, 122). Round 1 (bounding box, range under 8, so the inset is negative) starts at low (13, 22, 15),
/// high (14, 22, 15): red 107 and 115 around 111, so every texel is nearest the midpoint (111, 89, 123) and
/// `try_3color_block`'s first search (rgbcx.cpp 1725) puts all 16 on selector 2. One selector makes the fit singular
/// (903), and the fallback (1736-1742) is the `half` match of the average: low (27, 22, 15), high (0, 22, 15). Its
/// midpoint is the same colour (red (222 + 0) / 2 in both decoders), so its error equals the first one (34 plain, 974
/// perceptual): the search with that cutoff (1753) reaches the cutoff exactly at texel 15 and returns, and the strict
/// `<` (1755) rejects it. With perceptual weights the three-colour block wins (974 < 1534), keeping the first
/// endpoints and every texel on the midpoint; accepting the tie would have taken texel 15's unwritten selector.
#[test]
fn bc1_three_colour_midpoint_block_with_a_tying_single_colour_fit() {
    let mut px = [[111, 89, 122, 255]; 16];
    px[4] = [111, 90, 122, 255];
    for (allow_3color, perceptual) in MODES {
        same(&px, allow_3color, perceptual);
    }
    let b = same(&px, true, true);
    // (13, 22, 15) and (14, 22, 15), every texel on the midpoint.
    assert_eq!(colours(&b), (0x6ACF, 0x72CF));
    assert!(
        (0..16).all(|i| sel(&b, i) == 2),
        "selectors {:02x?}",
        &b[4..]
    );
}

/// A dark three-colour block where black (palette entry 3) is the uniquely nearest entry for a texel, and the encode
/// does not use it (task 4 Codex review, section 5): every three-colour selector search runs `j <= last` with `last =
/// use_black ? 3 : 2` (rgbcx.cpp 1075); `try_3color_block` passes `false` (1725, 1753, 1844) and the endpoint search
/// passes `any_black_pixels && cEncodeBC1Use3ColorBlocksForBlackPixels` (2628), a flag this call never sets. Seven
/// texels A = (78, 65, 24), eight B = (15, 73, 56) and texel 8 K = (2, 1, 0), found by a search and traced:
/// `try_3color_block` ends at (17, 16, 0) and (2, 16, 6) (red 140 and 16) with A on the midpoint and B and K on
/// (2, 16, 6), error 15217 plain (1814827 perceptual) against the four-colour 15315 (1817179), so the block is
/// three-colour. Black would be far nearer K than (16, 65, 49) is. The test asserts a three-colour block, no texel on
/// selector 3, and K's error to black below its error to each of the three colours.
#[test]
fn bc1_three_colour_block_never_uses_black_for_a_black_texel() {
    let (a, b, k) = ([78, 65, 24, 255], [15, 73, 56, 255], [2, 1, 0, 255]);
    let px = [a, a, a, a, b, b, b, b, k, b, b, a, b, b, a, a];
    for (allow_3color, perceptual) in MODES {
        same(&px, allow_3color, perceptual);
    }
    for perceptual in [false, true] {
        let out = same(&px, true, perceptual);
        let (c0, c1) = colours(&out);
        assert!(c0 <= c1, "a three-colour block: {c0:#06x}, {c1:#06x}");
        assert!(
            (0..16).all(|i| sel(&out, i) != 3),
            "selectors {:02x?}",
            &out[4..]
        );
        let code = |c: u16| [(c >> 11) as i32, ((c >> 5) & 63) as i32, (c & 31) as i32];
        let (l, h) = (code(c0), code(c1));
        let p = dual_palette(true, l[0], l[1], l[2], h[0], h[1], h[2]);
        let w = if perceptual { PERCEPTUAL } else { PLAIN };
        let black = dual_err(&p, w, 3, k);
        for s in 0..3 {
            assert!(
                black < dual_err(&p, w, s, k),
                "entry {s} is as near as black"
            );
        }
    }
}

/// One block in sixteen positions: fifteen texels on the exact 5:6:5 colour A = (165, 105, 247) = codes (20, 26, 30),
/// one X = (177, 105, 247) at texel `k` (task 4 Codex review, section 5: the selector search's cutoff equality at
/// different texels, with zero-error texels after it). With three-colour blocks and perceptual weights,
/// `try_3color_block` starts from low A, high (22, 26, 30) (red 181): X is 4 from the high endpoint and from the
/// midpoint 173, so it stays on selector 1 with error 1728 (54 * 16 per decoder). The first fit gives low A, high
/// (21, 26, 30) (red 173): X again 4 away, error 1728. That search's cutoff is 1728 (rgbcx.cpp 1753), every A texel
/// is exact on the low endpoint, so the running total reaches the cutoff exactly at texel `k` and returns (1087-1088)
/// with the texels after `k` at error 0; `<` rejects it (1755). The orderings then reach high (23, 26, 30), whose
/// midpoint 177 is X itself: error 0, a three-colour block. A traced run showed the three-colour equality at texel `k`
/// for every `k`, and the four-colour ordering loop's equality (cutoff 108) at `k` too. The A texels are identical, so
/// no step before the search depends on where X is.
#[test]
fn bc1_selector_search_cutoff_equality_at_every_texel() {
    let a = [165, 105, 247, 255];
    for k in 0..16 {
        let mut px = [a; 16];
        px[k] = [177, 105, 247, 255];
        for (allow_3color, perceptual) in MODES {
            same(&px, allow_3color, perceptual);
        }
        let b = same(&px, true, true);
        assert_eq!(colours(&b), (0xA35E, 0xBB5E), "X at {k}");
        for i in 0..16 {
            assert_eq!(
                sel(&b, i),
                if i == k { 2 } else { 0 },
                "X at {k}, texel {i}"
            );
        }
    }
}

/// Every grey, every colour with channels in the edge set, then random colours up to 20 000; each in both modes and
/// with both weight sets (the solid path reads no weights, so they give the same bytes).
#[test]
fn bc1_solid_blocks_equal_rgbcx_on_a_sample() {
    let mut colours = Vec::new();
    for g in 0..=255u8 {
        colours.push(solid(g, g, g));
    }
    const EDGES: [u8; 10] = [0, 1, 7, 8, 127, 128, 247, 248, 254, 255];
    for r in EDGES {
        for g in EDGES {
            for b in EDGES {
                colours.push(solid(r, g, b));
            }
        }
    }
    let mut rng = gen::Rng(0xBC10_0002);
    while colours.len() < 20_000 {
        colours.push(solid(rng.byte(), rng.byte(), rng.byte()));
    }
    for px in &colours {
        for allow_3color in [false, true] {
            let plain = same(px, allow_3color, false);
            assert_eq!(plain, same(px, allow_3color, true), "texels {px:?}");
        }
    }
}

/// All 2^24 solid colours in both modes over 16 threads (one per top nibble of red). Run with `-- --ignored`.
#[test]
#[ignore = "all 2^24 solid colours, both modes"]
fn bc1_solid_blocks_equal_rgbcx_on_every_colour() {
    let start = std::time::Instant::now();
    let total: u32 = std::thread::scope(|s| {
        let handles: Vec<_> = (0..16u32)
            .map(|t| {
                s.spawn(move || {
                    let mut n = 0;
                    for c in t << 20..(t + 1) << 20 {
                        let px = solid((c >> 16) as u8, (c >> 8) as u8, c as u8);
                        same(&px, true, false);
                        same(&px, false, false);
                        n += 1;
                    }
                    n
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    });
    assert_eq!(total, 1 << 24);
    eprintln!(
        "{total} solid colours in both modes, 0 mismatches, {:.1} s",
        start.elapsed().as_secs_f64()
    );
}

/// At least 100 million class blocks over 16 threads, four per `(allow_3color, perceptual)` combination, generated in
/// whole batches of `PER_CLASS` blocks per class so memory stays small and every class gets the same share. Run with
/// `-- --ignored`; a mismatch stops the thread with the replay data.
#[test]
#[ignore = "100 million blocks, about 11 minutes on 16 threads"]
fn bc1_equals_rgbcx_on_100_million_blocks() {
    const THREADS: u64 = 16;
    const PER_CLASS: usize = 1_000;
    const BATCH: usize = PER_CLASS * gen::COLOUR_CLASSES;
    // 6.25 million (100 million over 16 threads) rounded up to whole batches.
    const BATCHES: usize = 6_250_000usize.div_ceil(BATCH);
    const PER_THREAD: usize = BATCHES * BATCH;
    let start = std::time::Instant::now();
    let totals: Vec<usize> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                let (allow_3color, perceptual) = MODES[(t % 4) as usize];
                s.spawn(move || {
                    let mut n = 0;
                    for batch in 0..BATCHES as u64 {
                        let seed = 0x2_0000_0000 + (t * 10_000 + batch) * 0x9E37_79B9;
                        for px in gen::colour_classes(seed, PER_CLASS) {
                            same(&px, allow_3color, perceptual);
                            n += 1;
                        }
                    }
                    n
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (m, mode) in MODES.iter().enumerate() {
        let per_mode: usize = totals.iter().skip(m).step_by(4).sum();
        assert_eq!(per_mode, 4 * PER_THREAD, "blocks for {mode:?}");
    }
    let total: usize = totals.iter().sum();
    assert_eq!(total, 16 * PER_THREAD);
    assert!(total >= 100_000_000);
    eprintln!(
        "{total} blocks ({} per combination, {} per class), 0 mismatches, {:.1} s",
        4 * PER_THREAD,
        total / gen::COLOUR_CLASSES,
        start.elapsed().as_secs_f64()
    );
}
