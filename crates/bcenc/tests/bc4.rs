//! `bcenc`'s BC4 and BC5 equal the vendored rgbcx (through `encoders_sys`) on every block class.
use bcenc::{bc4_encode_alpha, bc4_encode_block, bc5_encode_block};

#[path = "support/gen.rs"]
mod gen;

const RADII: [u32; 3] = [4, 5, 10];

fn px(v: &[u8; 16], ch: usize) -> [[u8; 4]; 16] {
    let mut p = [[7u8; 4]; 16];
    for i in 0..16 {
        p[i][ch] = v[i];
    }
    p
}

/// Both outputs and the replay data on a mismatch.
fn same<const N: usize>(got: [u8; N], want: [u8; N], what: &str, input: &dyn std::fmt::Debug) {
    assert_eq!(
        got, want,
        "{what}: texels {input:?}\n  bcenc {got:?}\n  rgbcx {want:?}"
    );
}

fn check_bc4(v: &[u8; 16]) {
    let p = px(v, 0);
    for rad in RADII {
        same(
            bcenc::bc4_encode_block(&p, 0, rad),
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
                bc5_encode_block(&p, rad),
                encoders_sys::bc5_encode_block(&p, rad),
                &format!("bc5 radius {rad}"),
                &(v, w),
            );
        }
    }
}

#[test]
fn bc3_alpha_half_equals_rgbcx_for_each_cut() {
    // bc3_encode_block's first 8 bytes are the alpha block; colour texels stay mid-grey.
    for v in gen::bc4_classes(0xB4B4_0003, 300) {
        let mut p = [[128u8; 4]; 16];
        for i in 0..16 {
            p[i][3] = v[i];
        }
        for cut in gen::CUTS {
            for rad in RADII {
                let want: [u8; 8] = encoders_sys::bc3_encode_block(&p, rad, cut, false)[..8]
                    .try_into()
                    .unwrap();
                same(
                    bc4_encode_alpha(&v, rad, cut),
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
            let got = bc4_encode_block(&p, ch, 5);
            assert_eq!(
                got,
                encoders_sys::bc4_encode_block(&p, ch, 5),
                "channel {ch} of {p:?}"
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
                            bc4_encode_block(&p, 0, 5),
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
