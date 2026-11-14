use encoders_sys::{bc6h_compress_rows, bc6h_compress_rows_on, ispc_target, Bc6hLevel, IspcTarget};
use half::f16;
use sha2::{Digest, Sha256};

#[test]
fn constant_hdr_block_round_trips_within_two_percent() {
    let c = [1.5f32, 0.25, 0.0];
    let px = vec![
        [
            f16::from_f32(c[0]).to_bits(),
            f16::from_f32(c[1]).to_bits(),
            f16::from_f32(c[2]).to_bits(),
            f16::ONE.to_bits()
        ];
        16
    ];
    let mut out = [0u8; 16];
    bc6h_compress_rows(&px, 4, 4, &mut out, Bc6hLevel::Veryslow);
    let mut dec = [0f32; 48];
    bcdec_rs::bc6h_float(&out, &mut dec, 12, false);
    for t in 0..16 {
        for ch in 0..3 {
            let d = dec[t * 3 + ch];
            assert!(
                (d - c[ch]).abs() <= 0.02 * c[ch].max(0.05),
                "texel {t} ch {ch}: {d}"
            );
        }
    }
}

/// The 4x4 block that holds venice_sunset's sun (bench corpus, CC0): blue is exactly 0 wherever the sun is, as an
/// RGBE source stores it, and veryslow decodes the 6464 peak as 1464.
const SUN: [[f32; 3]; 16] = [
    [12.44, 3.19, 0.69],
    [16.0, 3.62, 0.5],
    [17.75, 3.75, 0.38],
    [14.06, 3.38, 0.56],
    [17.12, 3.5, 0.38],
    [308.0, 40.0, 0.0],
    [968.0, 144.0, 0.0],
    [29.0, 5.25, 0.12],
    [20.38, 3.88, 0.25],
    [3280.0, 480.0, 0.0],
    [6464.0, 960.0, 0.0],
    [186.0, 27.0, 0.0],
    [15.19, 3.0, 0.38],
    [258.0, 40.0, 0.0],
    [732.0, 112.0, 0.0],
    [29.25, 4.88, 0.12],
];

/// A deterministic 64x64 HDR image: a dim warm sky ramp with noise, venice_sunset's sun block and a bright neutral
/// highlight, so the peak-preserving path has work to do.
fn hdr_image() -> Vec<[u16; 4]> {
    let mut s = 0x2545_f491u32;
    let mut px = Vec::with_capacity(64 * 64);
    for y in 0..64u32 {
        for x in 0..64u32 {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let n = 1.0 + ((s >> 8) & 0xff) as f32 / 2048.0;
            let mut c = [
                (0.2 + x as f32 * 0.3) * n,
                (0.1 + y as f32 * 0.05) * n,
                0.02 + (63 - y) as f32 * 0.01,
            ];
            if (20..24).contains(&x) && (40..44).contains(&y) {
                c = SUN[((y - 40) * 4 + x - 20) as usize];
            }
            if (x, y) == (50, 10) {
                c = [900.0, 850.0, 800.0];
            }
            px.push([
                f16::from_f32(c[0]).to_bits(),
                f16::from_f32(c[1]).to_bits(),
                f16::from_f32(c[2]).to_bits(),
                f16::ONE.to_bits(),
            ]);
        }
    }
    px
}

/// AVX2 path, recorded on Windows, ISPC 1.31.0, 2026-10-31.
const RECORDED: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "f803b49eba0cad2880b811f039f65567133a4b7e8e281d0df42541522afb57a5",
    ),
    (
        Bc6hLevel::Fast,
        "fc9954432343d094bca7a2e7bddad47486fc3c00a14c5335a1bfe4810b14fcb1",
    ),
    (
        Bc6hLevel::Basic,
        "5b5dd790d10d4ebe36482cd6a5e4f956fe7ec9429cc9e58b8fb4405a61b030f8",
    ),
    (
        Bc6hLevel::Slow,
        "49804dcc003a5cb1a8eab90dcbbd1151e0f7ebfd68c070181fe17cb7226b1e6f",
    ),
    (
        Bc6hLevel::Veryslow,
        "cc6fbdc096496cd67c9b728eb3809f6899c6e73ea634498925afbf3d2b85ecb6",
    ),
];

/// SSE2, SSE4 and AVX write the same BC6H bytes.
const RECORDED_PRE_AVX2: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "2429f2f010d831a5e8392d7452e8b7e5defb334e943a97076fae0786d76c248b",
    ),
    (
        Bc6hLevel::Fast,
        "c8461152e349acb1cb294667a7e501a9536a1ba22687546dd1912432eac63319",
    ),
    (
        Bc6hLevel::Basic,
        "f0f5ba2c1fd6abb65db043cfd1e1b7b9373805b5383d1b595bc7e9c785c5e1fe",
    ),
    (
        Bc6hLevel::Slow,
        "c428819ef65ec8801744c9b4324ad1c4d46808540dfd0c37a298559a35b9e340",
    ),
    (
        Bc6hLevel::Veryslow,
        "ef6c0258b03cbd489f8ae998b3f9890f4e6b8866e0d2fc1a9ec6fd403e2fa997",
    ),
];

/// AVX-512 path, recorded on Windows with the ISPC 1.32.0 objects, on an AMD Zen 5 CPU, 2026-11-13. A test build
/// encodes on it only with the AVX2 cap lifted (`SDC_TEST_AVX512=1`).
const RECORDED_AVX512: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "67d2ab6a029a5e5b9427d3dfb7fe3c53191e5f027aa007f6259ff06a8c664e0d",
    ),
    (
        Bc6hLevel::Fast,
        "78ba22294b12f7c5007b8c4278b7e2b151585df449b340473b27d9574afa600f",
    ),
    (
        Bc6hLevel::Basic,
        "7692eb5302bce30784beab86b66a816c9f61fdff29711a585fc09dbd18a9a3d7",
    ),
    (
        Bc6hLevel::Slow,
        "d5dfe402130641019849baa55c36446db813a448651762b3d2412efcc2db45b0",
    ),
    (
        Bc6hLevel::Veryslow,
        "9070edcbe36791762996eacf2c9cbc8ff62f672ba66f17f13be70c1cda4e5fec",
    ),
];

fn recorded(target: IspcTarget) -> [(Bc6hLevel, &'static str); 5] {
    match target {
        IspcTarget::Sse2 | IspcTarget::Sse4 | IspcTarget::Avx => RECORDED_PRE_AVX2,
        IspcTarget::Avx2 => RECORDED,
        IspcTarget::Avx512 => RECORDED_AVX512,
    }
}

/// The five shipped levels encode byte-identically to the objects vendored before the peak-preserving patch.
/// The ISPC target paths differ (AVX-512, AVX2 and the three older ones), so each target has its own hashes; a CPU
/// checks the targets it supports, up to `ispc_target()` (AVX2 in a test build unless `SDC_TEST_AVX512=1`).
/// `SDC_RECORD_LEVEL_HASHES=1` prints them.
#[test]
fn shipped_levels_are_byte_identical_to_the_recorded_objects() {
    let px = hdr_image();
    let record = std::env::var_os("SDC_RECORD_LEVEL_HASHES").is_some();
    for target in IspcTarget::ALL
        .into_iter()
        .filter(|t| t.supported() && *t <= ispc_target())
    {
        for (level, want) in recorded(target) {
            let mut out = vec![0u8; 16 * 16 * 16];
            bc6h_compress_rows_on(&px, 64, 64, &mut out, level, target);
            let got = hex::encode(Sha256::digest(&out));
            if record {
                println!("{target:?} {level:?} {got}");
            } else {
                assert_eq!(got, want, "{target:?} {level:?}");
            }
        }
    }
}

#[test]
fn the_plain_call_encodes_on_the_process_target() {
    let px = hdr_image();
    for (level, _) in RECORDED {
        let mut plain = vec![0u8; 16 * 16 * 16];
        let mut on = vec![0u8; 16 * 16 * 16];
        bc6h_compress_rows(&px, 64, 64, &mut plain, level);
        bc6h_compress_rows_on(&px, 64, 64, &mut on, level, ispc_target());
        assert_eq!(plain, on, "{level:?}");
    }
}

/// Largest decoded luminance over the image divided by the source's.
fn peak_ratio(px: &[[u16; 4]], level: Bc6hLevel) -> f32 {
    let mut out = vec![0u8; 16 * 16 * 16];
    bc6h_compress_rows(px, 64, 64, &mut out, level);
    let lum = |c: &[f32]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let src = px
        .iter()
        .map(|p| {
            lum(&p[..3]
                .iter()
                .map(|&h| f16::from_bits(h).to_f32())
                .collect::<Vec<_>>())
        })
        .fold(0f32, f32::max);
    let mut dec = 0f32;
    for b in out.chunks(16) {
        let mut d = [0f32; 48];
        bcdec_rs::bc6h_float(b, &mut d, 12, false);
        dec = d.chunks(3).map(lum).fold(dec, f32::max);
    }
    dec / src
}

#[test]
fn peak_mode_keeps_a_blue_zero_sun() {
    let px = hdr_image();
    let r = peak_ratio(&px, Bc6hLevel::VeryslowPeak);
    eprintln!(
        "peak ratio {r}, veryslow {}",
        peak_ratio(&px, Bc6hLevel::Veryslow)
    );
    assert!(
        r >= 0.9,
        "peak ratio {r} (veryslow: {})",
        peak_ratio(&px, Bc6hLevel::Veryslow)
    );
}

#[test]
#[should_panic(expected = "multiple of 4")]
fn rejects_unaligned_width() {
    let px = vec![[0u16; 4]; 6 * 4];
    let mut out = [0u8; 32];
    bc6h_compress_rows(&px, 6, 4, &mut out, Bc6hLevel::Veryslow);
}
