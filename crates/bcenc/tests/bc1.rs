//! `bcenc`'s BC1 four-colour encode equals the vendored rgbcx (`encode_bc1(18, .., allow_3color = false, false)`
//! through `encoders_sys`) on every block class and on solid colours. Solid blocks take rgbcx's
//! `encode_bc1_solid_block` 4-colour path, so the solid tests also prove the single-colour tables.
use bcenc::bc1_encode4_for_tests;

#[path = "support/gen.rs"]
mod gen;

/// Both outputs and the texels on a mismatch, so the block can be replayed.
fn same4(px: &[[u8; 4]; 16]) {
    let got = bc1_encode4_for_tests(px);
    let want = encoders_sys::bc1_encode_block(px, false, false, false);
    assert_eq!(
        got, want,
        "bc1 four-colour, plain weights: texels {px:?}\n  bcenc {got:?}\n  rgbcx {want:?}"
    );
}

fn solid(r: u8, g: u8, b: u8) -> [[u8; 4]; 16] {
    [[r, g, b, 255]; 16]
}

#[test]
fn bc1_four_colour_equals_rgbcx_on_block_classes() {
    for px in gen::colour_classes(0xBC10_0001, 300) {
        same4(&px);
    }
}

/// rgbcx ignores alpha in four-colour mode (its solid test reads RGB only): random alpha changes neither side.
#[test]
fn bc1_four_colour_ignores_alpha() {
    let mut rng = gen::Rng(0xBC10_0003);
    let mut blocks = gen::colour_classes(0xBC10_0004, 40);
    // Solid RGB with mixed alpha takes the solid path whatever the alpha.
    for _ in 0..200 {
        let mut px = solid(rng.byte(), rng.byte(), rng.byte());
        px.iter_mut().for_each(|t| t[3] = rng.byte());
        blocks.push(px);
    }
    for px in blocks {
        let opaque = px.map(|t| [t[0], t[1], t[2], 255]);
        let mut mixed = px;
        mixed.iter_mut().for_each(|t| t[3] = rng.byte());
        let got = bc1_encode4_for_tests(&mixed);
        let want = encoders_sys::bc1_encode_block(&mixed, false, false, false);
        assert_eq!(
            got, want,
            "texels {mixed:?}\n  bcenc {got:?}\n  rgbcx {want:?}"
        );
        assert_eq!(
            got,
            bc1_encode4_for_tests(&opaque),
            "alpha changed the output: texels {mixed:?}"
        );
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
    same4(&[a, b, c, c, c, c, c, c, c, d, d, d, d, d, d, d]);
}

#[test]
fn bc1_four_colour_solid_blocks_equal_rgbcx_on_a_sample() {
    // Every grey, every colour with channels in the edge set, then random colours up to 20 000.
    let mut n = 0;
    for g in 0..=255u8 {
        same4(&solid(g, g, g));
        n += 1;
    }
    const EDGES: [u8; 10] = [0, 1, 7, 8, 127, 128, 247, 248, 254, 255];
    for r in EDGES {
        for g in EDGES {
            for b in EDGES {
                same4(&solid(r, g, b));
                n += 1;
            }
        }
    }
    let mut rng = gen::Rng(0xBC10_0002);
    while n < 20_000 {
        same4(&solid(rng.byte(), rng.byte(), rng.byte()));
        n += 1;
    }
}

/// All 2^24 solid colours over 16 threads (one per top nibble of red). Run with `-- --ignored`.
#[test]
#[ignore = "all 2^24 solid colours"]
fn bc1_four_colour_solid_blocks_equal_rgbcx_on_every_colour() {
    let start = std::time::Instant::now();
    let total: u32 = std::thread::scope(|s| {
        let handles: Vec<_> = (0..16u32)
            .map(|t| {
                s.spawn(move || {
                    let mut n = 0;
                    for c in t << 20..(t + 1) << 20 {
                        same4(&solid((c >> 16) as u8, (c >> 8) as u8, c as u8));
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
        "{total} solid colours, 0 mismatches, {:.1} s",
        start.elapsed().as_secs_f64()
    );
}

/// 100 million class blocks over 16 threads, generated in batches so memory stays small. Run with `-- --ignored`;
/// a mismatch stops the thread with the replay data.
#[test]
#[ignore = "100 million blocks, about 11 minutes on 16 threads"]
fn bc1_four_colour_equals_rgbcx_on_100_million_blocks() {
    const THREADS: u64 = 16;
    const PER_CLASS: usize = 1_000; // 25 classes x 1000 = 25 000 blocks per batch
    const BATCHES: u64 = 250; // 16 threads x 250 batches x 25 000 = 100 million blocks
    assert_eq!(gen::COLOUR_CLASSES, 25);
    let start = std::time::Instant::now();
    let total: usize = std::thread::scope(|s| {
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                s.spawn(move || {
                    let mut n = 0;
                    for batch in 0..BATCHES {
                        let seed = 0x1_0000_0000 + (t * BATCHES + batch) * 0x9E37_79B9;
                        for px in gen::colour_classes(seed, PER_CLASS) {
                            same4(&px);
                            n += 1;
                        }
                    }
                    n
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    });
    assert_eq!(
        total,
        THREADS as usize * BATCHES as usize * PER_CLASS * gen::COLOUR_CLASSES
    );
    assert_eq!(total, 100_000_000);
    eprintln!(
        "{total} blocks, 0 mismatches, {:.1} s",
        start.elapsed().as_secs_f64()
    );
}
