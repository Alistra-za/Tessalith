// BC1 perceptual weighting (README-VENDOR.md patch "rgbcx BC1 perceptual", DECISIONS FM42).
use encoders_sys::*;
use sha2::{Digest, Sha256};

/// 64x64 noisy colour (the bc1_dual.rs image) with a scattered alpha cut for the 1-bit and BC3 paths.
fn blocks() -> Vec<[[u8; 4]; 16]> {
    let (w, h) = (64u32, 64u32);
    let mut seed = 12345u32;
    let mut rnd = move || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (seed >> 24) as u8
    };
    let px: Vec<[u8; 4]> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            [
                (x * 4) as u8 / 2 + rnd() / 8,
                (y * 3) as u8 + rnd() / 4,
                if (x / 8 + y / 8) % 3 == 0 {
                    rnd()
                } else {
                    (x * y) as u8
                },
                if (x * 7 + y * 5) % 11 < 3 { 0 } else { 255 },
            ]
        })
        .collect();
    gather_blocks(&px, w, h)
        .chunks(16)
        .map(|b| b.try_into().unwrap())
        .collect()
}

fn mask(b: &[[u8; 4]; 16]) -> u16 {
    b.iter()
        .enumerate()
        .fold(0, |m, (i, t)| m | (u16::from(t[3] < 128) << i))
}

/// BC1, BC1 1-bit alpha and BC3 encodes of every block, concatenated.
fn all(perceptual: bool) -> Vec<u8> {
    let mut out = vec![];
    for b in blocks() {
        out.extend(bc1_encode_block(&b, true, false, perceptual));
        out.extend(bc1_encode_block_alpha(&b, mask(&b), perceptual));
        out.extend(bc3_encode_block(&b, BC45_RADIUS_DEFAULT, 128, perceptual));
    }
    out
}

#[test]
fn perceptual_bc1_differs_and_off_is_byte_identical() {
    init();
    let off = all(false);
    // Recorded before the patch (the three calls without the flag).
    assert_eq!(
        hex::encode(Sha256::digest(&off)),
        "684fdc6bc03dbad2b37c6aa2d0c236f99735f70a2ade7a172f3c04a412923d6d"
    );
    let on = all(true);
    let n = blocks().len();
    let differ = |at: usize, len: usize| {
        (0..n)
            .filter(|i| {
                let s = i * 32 + at;
                off[s..s + len] != on[s..s + len]
            })
            .count()
    };
    // Each path gets the flag: BC1, BC1 with 1-bit alpha, and BC3's colour half (its alpha half never changes).
    assert!(differ(0, 8) > n / 4, "BC1: {} of {n}", differ(0, 8));
    assert!(differ(8, 8) > n / 4, "BC1 alpha: {} of {n}", differ(8, 8));
    assert!(
        differ(24, 8) > n / 4,
        "BC3 colour: {} of {n}",
        differ(24, 8)
    );
    assert_eq!(differ(16, 8), 0, "BC3 alpha");
}

/// The weights live per thread: a perceptual encode on one thread never leaks into a plain one on another.
#[test]
fn perceptual_is_per_thread() {
    init();
    let off = all(false);
    let on = all(true);
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..8)
            .map(|t| {
                s.spawn(move || {
                    (
                        t % 2 == 1,
                        (0..4).map(|_| all(t % 2 == 1)).collect::<Vec<_>>(),
                    )
                })
            })
            .collect();
        for h in hs {
            let (p, runs) = h.join().unwrap();
            for r in runs {
                assert!(&r == if p { &on } else { &off }, "perceptual {p}");
            }
        }
    });
}
