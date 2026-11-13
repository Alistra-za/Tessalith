//! `bcenc`'s BC3 encode equals the vendored rgbcx (`encode_bc3_hq(18, .., rad, BC4_USE_ALL_MODES | BC4_SDC_SEARCH |
//! BC4_SDC_GUARD128 | cut << 8)` through `encoders_sys`, plain or perceptual weights) on the alpha classes paired with
//! the colour classes.
use bcenc::{bc1_encode_block, bc3_encode_block, bc4_encode_alpha};

#[path = "support/gen.rs"]
mod gen;

/// The search radii the engine uses.
const RADII: [u32; 3] = [4, 5, 10];
/// `CUTS` plus 0, which rgbcx reads as 128 (rgbcx.cpp 3488).
const CUTS: [u8; 5] = [0, 1, 128, 201, 255];

/// Both outputs, the arguments and the texels on a mismatch, so the block can be replayed.
fn same(px: &[[u8; 4]; 16], rad: u32, cut: u8, perceptual: bool) -> [u8; 16] {
    let got = bc3_encode_block(px, rad, cut, perceptual);
    let want = encoders_sys::bc3_encode_block(px, rad, cut, perceptual);
    assert_eq!(
        got, want,
        "bc3, radius {rad}, cut {cut}, perceptual {perceptual}: texels {px:?}\n  bcenc {got:?}\n  rgbcx {want:?}"
    );
    got
}

/// Alpha block `i` of `alpha` under colour block `i % colour.len()`.
fn paired(alpha: &[[u8; 16]], colour: &[[[u8; 4]; 16]]) -> Vec<[[u8; 4]; 16]> {
    alpha
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let c = &colour[i % colour.len()];
            std::array::from_fn(|k| [c[k][0], c[k][1], c[k][2], a[k]])
        })
        .collect()
}

#[test]
fn bc3_equals_rgbcx_on_block_classes() {
    let blocks = paired(
        &gen::alpha_classes(0xBC30_0001, 30),
        &gen::colour_classes(0xBC30_0002, 10),
    );
    for perceptual in [false, true] {
        for px in &blocks {
            for rad in RADII {
                for cut in CUTS {
                    same(px, rad, cut, perceptual);
                }
            }
        }
    }
}

/// Every cut from 1 to 255 is told apart from its neighbour. Most blocks encode to the same bytes at cut c and c + 1
/// (the guard only acts when the search would move more texels across the cut than the mode-loop block does), so a
/// wrong cut by one would pass a test on arbitrary blocks. For each c this draws blocks with 1 to 6 texels exactly on c
/// and the rest within a random range of it until three give different alpha bytes at c and c + 1 (radius 5), and
/// compares every drawn block with rgbcx at both cuts, the three at every radius and at cut 0 too.
#[test]
fn bc3_equals_rgbcx_for_every_cut() {
    const WANTED: usize = 3;
    const TRIES: usize = 4000;
    let colour = gen::colour_classes(0xBC30_0005, 1);
    let mut r = gen::Rng(0xBC30_0006);
    let mut n = 0usize;
    for cut in 1..=254u8 {
        let mut found = 0;
        for _ in 0..TRIES {
            let range = r.below(120) + 4;
            let on_cut = r.below(6) as usize + 1;
            let alpha: [u8; 16] = std::array::from_fn(|i| {
                if i < on_cut {
                    cut
                } else {
                    let v = cut as i32 + r.below(2 * range + 1) as i32 - range as i32;
                    v.clamp(0, 255) as u8
                }
            });
            let c = &colour[n % colour.len()];
            let px: [[u8; 4]; 16] = std::array::from_fn(|k| [c[k][0], c[k][1], c[k][2], alpha[k]]);
            let perceptual = n % 2 == 1;
            n += 1;
            same(&px, 5, cut, perceptual);
            same(&px, 5, cut + 1, perceptual);
            if bc4_encode_alpha(&alpha, 5, cut) != bc4_encode_alpha(&alpha, 5, cut + 1) {
                for rad in RADII {
                    same(&px, rad, cut, perceptual);
                    same(&px, rad, cut + 1, perceptual);
                    same(&px, rad, 0, perceptual);
                }
                found += 1;
                if found == WANTED {
                    break;
                }
            }
        }
        assert_eq!(
            found,
            WANTED,
            "blocks whose alpha bytes differ between cut {cut} and {}",
            cut + 1
        );
    }
}

/// The two halves are the BC1 colour encode without three-colour blocks and the BC4 alpha search, and a cut of 0 is a
/// cut of 128 (rgbcx.cpp 3527-3528, 3488).
#[test]
fn bc3_is_bc4_alpha_then_four_colour_bc1() {
    let blocks = paired(
        &gen::alpha_classes(0xBC30_0003, 4),
        &gen::colour_classes(0xBC30_0004, 4),
    );
    for perceptual in [false, true] {
        for px in &blocks {
            let alpha: [u8; 16] = std::array::from_fn(|k| px[k][3]);
            let b = same(px, 5, 0, perceptual);
            assert_eq!(b[..8], bc4_encode_alpha(&alpha, 5, 128), "texels {px:?}");
            assert_eq!(b, same(px, 5, 128, perceptual), "texels {px:?}");
            assert_eq!(
                b[8..],
                bc1_encode_block(px, false, perceptual),
                "texels {px:?}"
            );
        }
    }
}

/// 50 million blocks over 16 threads at radius 5, the cut and the weight set changing from block to block, in
/// batches so memory stays small. Run with `-- --ignored`.
#[test]
#[ignore = "50 million blocks, about 7 minutes on 16 threads"]
fn bc3_equals_rgbcx_on_50_million_blocks() {
    const THREADS: u64 = 16;
    const PER_THREAD: usize = 3_125_000; // 16 x 3.125 million = 50 million
    let start = std::time::Instant::now();
    let totals: Vec<usize> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                s.spawn(move || {
                    let mut n = 0;
                    let mut batch = 0u64;
                    while n < PER_THREAD {
                        let seed = 0x8_0000_0000 + (t * 10_000 + batch) * 0x9E37_79B9;
                        let blocks = paired(
                            &gen::alpha_classes(seed, 200),
                            &gen::colour_classes(seed ^ 0x55, 100),
                        );
                        for px in &blocks {
                            if n == PER_THREAD {
                                break;
                            }
                            same(px, 5, CUTS[n % CUTS.len()], (n / CUTS.len()) % 2 == 1);
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
        "{total} blocks at radius 5, 0 mismatches, {:.1} s",
        start.elapsed().as_secs_f64()
    );
}
