// BC1 encodes for the summed NVIDIA + AMD decoder error (cBC1Dual; README-VENDOR.md patch "rgbcx BC1 dual").
use encoders_sys::*;

fn image() -> Vec<[u8; 4]> {
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
                255,
            ]
        })
        .collect();
    gather_blocks(&px, w, h)
}

/// SSE under the NVIDIA decoder plus SSE under the AMD decoder.
fn dual_sse(blk: &[u8; 8], src: &[[u8; 4]]) -> u64 {
    [Bc1Model::Nvidia, Bc1Model::Amd]
        .iter()
        .flat_map(|&m| bc1_decode_block(blk, m).into_iter().zip(src))
        .map(|(a, b)| {
            (0..3)
                .map(|c| (a[c] as i64 - b[c] as i64).pow(2) as u64)
                .sum::<u64>()
        })
        .sum()
}

/// Sum over the blocks of the 64x64 generated image of NV SSE + AMD SSE of the encoded block.
fn summed_sse() -> u64 {
    image()
        .chunks(16)
        .map(|b| {
            let b: &[[u8; 4]; 16] = b.try_into().unwrap();
            dual_sse(&bc1_encode_block(b, true, false, false), b)
        })
        .sum()
}

#[test]
fn dual_summed_vendor_error_is_pinned() {
    init();
    // The value the research prototype measured (reports/bc1-dual/REPORT-dual-mode.md).
    assert_eq!(summed_sse(), 4_998_895);
}

#[test]
fn nvidia_and_amd_decode_a_dual_block() {
    init();
    let blk = bc1_encode_block(&[[10, 200, 30, 255]; 16], true, false, false);
    for m in [Bc1Model::Nvidia, Bc1Model::Amd] {
        let px = bc1_decode_block(&blk, m);
        assert_eq!(px.len(), 16);
        assert!(px.iter().all(|p| p[3] == 255), "{m:?}");
    }
}
