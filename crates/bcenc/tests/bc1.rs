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
    const PER_CLASS: usize = 1_000; // 19 classes x 1000 = 19 000 blocks per batch
    const BATCHES: u64 = 329; // 16 threads x 329 batches x 19 000 = 100.02 million blocks
    assert_eq!(gen::COLOUR_CLASSES, 19);
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
    eprintln!(
        "{total} blocks, 0 mismatches, {:.1} s",
        start.elapsed().as_secs_f64()
    );
}
