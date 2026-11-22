//! `bcenc`'s BC4 and BC5 equal the vendored rgbcx (through `encoders_sys`) on every block class; the `_max` functions,
//! which rgbcx does not have, are pinned to bytes taken from the bench code (Note LI). Every comparison and pin runs
//! on every code path the machine has (`support/paths.rs`), through the hidden `_on` entries.
use bcenc::{
    bc3_encode_block_max_on, bc3_encode_block_raw_cut_on, bc4_encode_alpha_max_on,
    bc4_encode_alpha_on, bc4_encode_block, bc4_encode_block_max, bc4_encode_block_max_on,
    bc4_encode_block_on, bc5_encode_block_max_on, bc5_encode_block_on, Bc1Search, Path,
};
use std::num::NonZeroU8;

#[path = "support/gen.rs"]
mod gen;
#[path = "support/paths.rs"]
mod paths;

const RADII: [u32; 3] = [4, 5, 10];

fn px(v: &[u8; 16], ch: usize) -> [[u8; 4]; 16] {
    let mut p = [[7u8; 4]; 16];
    for i in 0..16 {
        p[i][ch] = v[i];
    }
    p
}

/// `got` on every path against `want`; both outputs, the path and the replay data on a mismatch.
fn same<const N: usize>(
    got: impl Fn(Path) -> [u8; N],
    want: [u8; N],
    what: &str,
    input: &dyn std::fmt::Debug,
) -> [u8; N] {
    for path in paths::paths() {
        let got = got(path);
        assert_eq!(
            got,
            want,
            "{what} on path {}: texels {input:?}\n  bcenc {got:?}\n  rgbcx {want:?}",
            path.name()
        );
    }
    want
}

fn check_bc4(v: &[u8; 16]) {
    let p = px(v, 0);
    for rad in RADII {
        same(
            |path| bc4_encode_block_on(path, &p, 0, rad),
            encoders_sys::bc4_encode_block(&p, 0, rad),
            &format!("bc4 radius {rad}"),
            v,
        );
    }
}

#[test]
fn bc4_equals_rgbcx_on_block_classes() {
    for v in gen::bc4_classes(0xB4B4_0001, 400) {
        check_bc4(&v);
    }
}

#[test]
fn bc5_equals_rgbcx_on_block_classes() {
    let classes = gen::bc4_classes(0xB4B4_0002, 400);
    for (i, v) in classes.iter().enumerate() {
        // Channel 1 gets a different class block, so the two halves are independent.
        let w = &classes[(i * 7 + 13) % classes.len()];
        let mut p = px(v, 0);
        for t in 0..16 {
            p[t][1] = w[t];
        }
        for rad in RADII {
            same(
                |path| bc5_encode_block_on(path, &p, rad),
                encoders_sys::bc5_encode_block(&p, rad),
                &format!("bc5 radius {rad}"),
                &(v, w),
            );
        }
    }
}

#[test]
fn bc3_alpha_half_equals_rgbcx_for_each_cut() {
    // bc3_encode_block's first 8 bytes are the alpha block; colour texels stay mid-grey. Cut 0 is rgbcx's "use 128"
    // (rgbcx.cpp 3488).
    for v in gen::bc4_classes(0xB4B4_0003, 300) {
        let mut p = [[128u8; 4]; 16];
        for i in 0..16 {
            p[i][3] = v[i];
        }
        for cut in gen::CUTS.into_iter().chain([0]) {
            for rad in RADII {
                let want: [u8; 8] = encoders_sys::bc3_encode_block(&p, rad, cut, false)[..8]
                    .try_into()
                    .unwrap();
                same(
                    |path| bc4_encode_alpha_on(path, &v, rad, cut),
                    want,
                    &format!("bc3 alpha radius {rad} cut {cut}"),
                    &v,
                );
            }
        }
    }
}

#[test]
fn bc4_channel_picks_the_named_channel() {
    let classes = gen::bc4_classes(0xB4B4_0004, 50);
    for chunk in classes.as_chunks::<4>().0 {
        let p: [[u8; 4]; 16] =
            std::array::from_fn(|t| [chunk[0][t], chunk[1][t], chunk[2][t], chunk[3][t]]);
        for (ch, own) in chunk.iter().enumerate() {
            let got = same(
                |path| bc4_encode_block_on(path, &p, ch, 5),
                encoders_sys::bc4_encode_block(&p, ch, 5),
                &format!("bc4 channel {ch}"),
                &p,
            );
            // The channel is the only input: other channels do not matter.
            assert_eq!(
                got,
                bc4_encode_block(&px(own, ch), ch, 5),
                "channel {ch} alone"
            );
        }
    }
}

#[test]
#[should_panic(expected = "channel 4")]
fn bc4_channel_out_of_range_panics() {
    bc4_encode_block(&[[0; 4]; 16], 4, 5);
}

/// The six pinned blocks of `gen` with the bench code's bytes (`gen::MAX_BENCH_BYTES`): slowest no guard, slowest cut
/// 128, max no guard, max cut 128. Independent of `bc4::search_max`.
fn bench_blocks() -> impl Iterator<Item = ([u8; 16], [[u8; 8]; 4])> {
    gen::MAX_BENCH_BYTES
        .into_iter()
        .map(|(v, [_, slow, slow128, max, max128])| (v, [slow, slow128, max, max128]))
}

#[test]
fn max_bytes_match_the_bench() {
    for (v, [slow, slow128, max, max128]) in bench_blocks() {
        let p = v.map(|a| [a, 0, 0, 255]);
        for path in paths::paths() {
            let n = path.name();
            assert_eq!(
                bc4_encode_block_max_on(path, &p, 0),
                max,
                "max {v:?} path {n}"
            );
            assert_eq!(
                bc4_encode_alpha_max_on(path, &v, 128),
                max128,
                "max cut 128 {v:?} path {n}"
            );
            let both = bc5_encode_block_max_on(path, &v.map(|a| [a, a, 0, 255]));
            assert_eq!(both[..8], max, "bc5 channel 0 {v:?} path {n}");
            assert_eq!(both[8..], max, "bc5 channel 1 {v:?} path {n}");
            assert_eq!(
                bc4_encode_block_on(path, &p, 0, 10),
                slow,
                "slowest {v:?} path {n}"
            );
            assert_eq!(
                bc4_encode_alpha_on(path, &v, 10, 128),
                slow128,
                "slowest cut 128 {v:?} path {n}"
            );
        }
    }
}

#[test]
fn bc5_max_is_two_bc4_max_channels() {
    let classes = gen::bc4_classes(0xB4B4_0014, 4);
    for (i, v) in classes.iter().enumerate() {
        let w = &classes[(i * 7 + 13) % classes.len()];
        let mut p = px(v, 0);
        for t in 0..16 {
            p[t][1] = w[t];
        }
        for path in paths::paths() {
            let n = path.name();
            let got = bc5_encode_block_max_on(path, &p);
            assert_eq!(
                got[..8],
                bc4_encode_block_max_on(path, &p, 0),
                "channel 0 of {p:?} path {n}"
            );
            assert_eq!(
                got[8..],
                bc4_encode_block_max_on(path, &p, 1),
                "channel 1 of {p:?} path {n}"
            );
        }
    }
}

#[test]
fn bc3_max_halves() {
    for v in gen::bc4_classes(0xB4B4_0015, 3) {
        let p: [[u8; 4]; 16] =
            std::array::from_fn(|i| [v[i], v[i] ^ 0x5A, 255 - v[i], v[(i * 5) % 16]]);
        let alpha: [u8; 16] = std::array::from_fn(|i| p[i][3]);
        for cut in [1u8, 128, 201] {
            let nz = NonZeroU8::new(cut).unwrap();
            for perceptual in [false, true] {
                for search in [Bc1Search::Default, Bc1Search::Fast] {
                    for path in paths::paths() {
                        let n = path.name();
                        let got = bc3_encode_block_max_on(path, &p, nz, perceptual, search);
                        assert_eq!(
                            got[..8],
                            bc4_encode_alpha_max_on(path, &alpha, cut),
                            "alpha {p:?} cut {cut} path {n}"
                        );
                        assert_eq!(
                            got[8..],
                            bc3_encode_block_raw_cut_on(path, &p, 5, cut, perceptual, search)[8..],
                            "colour {p:?} cut {cut} {perceptual} {search:?} path {n}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn bc4_max_channel_picks_the_named_channel() {
    let classes = gen::bc4_classes(0xB4B4_0016, 4);
    for chunk in classes.as_chunks::<4>().0 {
        let p: [[u8; 4]; 16] =
            std::array::from_fn(|t| [chunk[0][t], chunk[1][t], chunk[2][t], chunk[3][t]]);
        for (ch, own) in chunk.iter().enumerate() {
            for path in paths::paths() {
                let n = path.name();
                let got = bc4_encode_block_max_on(path, &p, ch);
                // The channel is the only input: other channels do not matter.
                assert_eq!(
                    got,
                    bc4_encode_block_max_on(path, &px(own, ch), ch),
                    "channel {ch} of {p:?} path {n}"
                );
                assert_eq!(
                    got,
                    bcenc::bc4::search_max_on(path, own, None),
                    "channel {ch} search path {n}"
                );
            }
        }
    }
}

#[test]
#[should_panic(expected = "channel 4")]
fn bc4_max_channel_out_of_range_panics() {
    bc4_encode_block_max(&[[0; 4]; 16], 4);
}

#[test]
fn bc4_encode_alpha_max_reads_cut_0_as_128() {
    let blocks = gen::bc4_classes(0xB4B4_0017, 3)
        .into_iter()
        .chain([gen::MAX_GUARD_FALLBACK, gen::MAX_GUARD_FALLBACK_BASE]);
    for v in blocks {
        for path in paths::paths() {
            assert_eq!(
                bc4_encode_alpha_max_on(path, &v, 0),
                bc4_encode_alpha_max_on(path, &v, 128),
                "{v:?} path {}",
                path.name()
            );
        }
    }
    // A cut of 0 is the guard at 128, not no guard: this block's unguarded Max block differs.
    let v = gen::MAX_GUARD_FALLBACK;
    for path in paths::paths() {
        assert_ne!(
            bc4_encode_alpha_max_on(path, &v, 0),
            bc4_encode_block_max_on(path, &v.map(|a| [a, 0, 0, 0]), 0),
            "path {}",
            path.name()
        );
    }
}

/// FNV-1a 64 over `bytes`, continuing from `h`.
fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h = (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// A regression pin of the Max search over more blocks than the bench table. Recorded from the implementation after
/// `max_bytes_match_the_bench` and `max_equals_the_bench_reference` passed, on 2026-11-21.
const MAX_PIN: u64 = 0xacc9_050d_69c8_9111;

#[test]
fn max_bytes_are_pinned() {
    let blocks = gen::bc4_classes(0xB4B4_0013, 16);
    for path in paths::paths() {
        let mut h = 0xCBF2_9CE4_8422_2325u64;
        for v in &blocks {
            let p = v.map(|a| [a, a ^ 0x5A, 0, a]);
            h = fnv(h, &bc4_encode_block_max_on(path, &p, 0));
            h = fnv(h, &bc5_encode_block_max_on(path, &p));
            for cut in gen::CUTS {
                h = fnv(h, &bc4_encode_alpha_max_on(path, v, cut));
            }
        }
        assert_eq!(h, MAX_PIN, "{h:#018x} path {}", path.name());
    }
}

/// 20 million blocks at radius 5, split over 16 threads. Run with `-- --ignored`; mismatches stop the thread.
#[test]
#[ignore = "20 million blocks, about a minute with 16 threads"]
fn bc4_equals_rgbcx_on_20_million_blocks() {
    const THREADS: u64 = 16;
    const PER_CLASS: usize = 113_750; // 11 classes x 113750 x 16 threads = 20.02 million blocks
    let start = std::time::Instant::now();
    let total: usize = std::thread::scope(|s| {
        let handles: Vec<_> = (0..THREADS)
            .map(|t| {
                s.spawn(move || {
                    let blocks = gen::bc4_classes(0x20_0000_0000 + t * 0x9E37, PER_CLASS);
                    for v in &blocks {
                        let p = px(v, 0);
                        same(
                            |path| bc4_encode_block_on(path, &p, 0, 5),
                            encoders_sys::bc4_encode_block(&p, 0, 5),
                            "bc4 radius 5",
                            v,
                        );
                    }
                    blocks.len()
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
