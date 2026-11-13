//! `bcenc`'s BC1 encode equals the vendored rgbcx (`encode_bc1(18, .., allow_3color, false)` through `encoders_sys`,
//! plain or perceptual weights) on every block class and on solid colours. Solid blocks take rgbcx's
//! `encode_bc1_solid_block`, so the solid tests also prove the single-colour tables (`equals_1` with three-colour off,
//! `half` too with it on).
use bcenc::bc1_encode_block;

#[path = "support/gen.rs"]
mod gen;

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

/// 100 million class blocks over 16 threads, four per `(allow_3color, perceptual)` combination, generated in batches
/// so memory stays small; each thread takes exactly its share (the last batch cut short). Run with `-- --ignored`; a
/// mismatch stops the thread with the replay data.
#[test]
#[ignore = "100 million blocks, about 15 minutes on 16 threads"]
fn bc1_equals_rgbcx_on_100_million_blocks() {
    const THREADS: u64 = 16;
    const PER_THREAD: usize = 6_250_000; // 16 threads x 6.25 million = 100 million, 25 million per combination
    const PER_CLASS: usize = 1_000;
    let start = std::time::Instant::now();
    let totals: Vec<usize> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                let (allow_3color, perceptual) = MODES[(t % 4) as usize];
                s.spawn(move || {
                    let mut n = 0;
                    let mut batch = 0u64;
                    while n < PER_THREAD {
                        let seed = 0x2_0000_0000 + (t * 10_000 + batch) * 0x9E37_79B9;
                        for px in gen::colour_classes(seed, PER_CLASS) {
                            if n == PER_THREAD {
                                break;
                            }
                            same(&px, allow_3color, perceptual);
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
    for (m, mode) in MODES.iter().enumerate() {
        let per_mode: usize = totals.iter().skip(m).step_by(4).sum();
        assert_eq!(per_mode, 25_000_000, "blocks for {mode:?}");
    }
    let total: usize = totals.iter().sum();
    assert_eq!(total, 100_000_000);
    eprintln!(
        "{total} blocks (25 million per combination), 0 mismatches, {:.1} s",
        start.elapsed().as_secs_f64()
    );
}
