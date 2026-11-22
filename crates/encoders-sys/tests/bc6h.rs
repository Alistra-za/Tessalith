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

/// AVX2 path, recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-22, from the ISPC TC patch 0004 objects
/// (Note LM).
const RECORDED: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "f803b49eba0cad2880b811f039f65567133a4b7e8e281d0df42541522afb57a5",
    ),
    (
        Bc6hLevel::Fast,
        "6e863a5eb712ddd98f113ae993cd35bd6811f6faf6e23bcfd32ba3e73ca8b420",
    ),
    (
        Bc6hLevel::Basic,
        "77f47d088a051ee81193bbde671958d3ce11452ddc7f1d6a9c21fe8e38361a8e",
    ),
    (
        Bc6hLevel::Slow,
        "5cff76d6f54850151c3859ebbe25476bd014c83c85c97dffa6f48e39bb8c8732",
    ),
    (
        Bc6hLevel::Veryslow,
        "27cd50278c489eddbe355e00d270a00f24930bae01acddb7d96808dcd3f045f1",
    ),
];

/// The SSE2 path, recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-22, from the ISPC TC patch 0004 objects
/// (Note LM). The SSE4 and AVX paths, dropped by Note JB, wrote the same bytes before patch 0004.
const RECORDED_PRE_AVX2: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "2429f2f010d831a5e8392d7452e8b7e5defb334e943a97076fae0786d76c248b",
    ),
    (
        Bc6hLevel::Fast,
        "b557c183267ca7205902aa27ad831bd3961d9691b5b44755094d81ef2fc3c455",
    ),
    (
        Bc6hLevel::Basic,
        "40490aad6ef3be2d6b4012023c76ade0fa8e4bdc4a3ca94e99f3ee0be00b731f",
    ),
    (
        Bc6hLevel::Slow,
        "0931a03ce5cb1b95461b162cfa56643a668351303d67bd89530b1697efe47ad8",
    ),
    (
        Bc6hLevel::Veryslow,
        "815cd489e79d4bea56815d93a7c4be6483251d11c945aa2d7d4940df0f370978",
    ),
];

/// AVX-512 path, recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-22, from the ISPC TC patch 0004 objects built
/// with ISPC 1.32.0 (Note LM). A test build encodes on it only with the AVX2 cap lifted (`SDC_TEST_AVX512=1`).
const RECORDED_AVX512: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "67d2ab6a029a5e5b9427d3dfb7fe3c53191e5f027aa007f6259ff06a8c664e0d",
    ),
    (
        Bc6hLevel::Fast,
        "d75be99d5b345c3653a23464859b534020e9450b7324b5550016ce4c0ee9d8d3",
    ),
    (
        Bc6hLevel::Basic,
        "3aec6443b998926eaf19c6609716b69089a19649524636cdb2ae44c140988765",
    ),
    (
        Bc6hLevel::Slow,
        "082cf8a89f677a03cf1c926dd9983fab11773a85c0e11d972918489ba3de6ef0",
    ),
    (
        Bc6hLevel::Veryslow,
        "a81ae4aaa7f9603d105dd5dbb78d0f39936f29e8f65a2b961329a2d592b90fca",
    ),
];

fn recorded(target: IspcTarget) -> [(Bc6hLevel, &'static str); 5] {
    match target {
        IspcTarget::Sse2 => RECORDED_PRE_AVX2,
        IspcTarget::Avx2 => RECORDED,
        IspcTarget::Avx512 => RECORDED_AVX512,
    }
}

/// Encodes `hdr_image()` on every target this CPU supports, up to `ispc_target()` (AVX2 in a test build unless
/// `SDC_TEST_AVX512=1`), at each level `pins` names for the target, and checks the SHA-256 against the pin.
/// `SDC_RECORD_LEVEL_HASHES=1` prints the hashes instead.
fn check_pins(pins: impl Fn(IspcTarget) -> Vec<(Bc6hLevel, &'static str)>) {
    if encoders_sys::skip_byte_pins() {
        return;
    }
    let px = hdr_image();
    let record = std::env::var_os("SDC_RECORD_LEVEL_HASHES").is_some();
    for target in IspcTarget::ALL
        .into_iter()
        .filter(|t| t.supported() && *t <= ispc_target())
    {
        for (level, want) in pins(target) {
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

/// The five shipped levels encode byte-identically to the recorded objects (the patch 0004 objects, Note LM), on each
/// path the run reads (SSE2 and AVX2; AVX-512 only with the cap lifted). The paths differ, so each has its own hashes.
#[test]
fn shipped_levels_are_byte_identical_to_the_recorded_objects() {
    check_pins(|t| recorded(t).to_vec());
}

/// The peak mode (`VeryslowPeak`) per path: SSE2, AVX2, AVX-512 (AVX-512 read only with the cap lifted).
/// Recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-22, from the ISPC TC patch 0004 objects (Note LM).
const RECORDED_PEAK: [&str; 3] = [
    "0d45a9e2ddf853c1b8a332590fc7100a95d24837d88e3526d9a17b976337af64",
    "7ef6b1145bc2887e96e1c841d4932961c757ae67cebbed49a63a53971a5cd8c8",
    "a76716efee2ffafc3780cc95f8611527cc0c8f4e62022c9896aca54cb9b0870b",
];

fn recorded_peak(target: IspcTarget) -> &'static str {
    match target {
        IspcTarget::Sse2 => RECORDED_PEAK[0],
        IspcTarget::Avx2 => RECORDED_PEAK[1],
        IspcTarget::Avx512 => RECORDED_PEAK[2],
    }
}

/// The peak mode encodes byte-identically to the recorded objects on every supported path.
#[test]
fn the_peak_mode_is_byte_identical_to_the_recorded_objects() {
    check_pins(|t| vec![(Bc6hLevel::VeryslowPeak, recorded_peak(t))]);
}

/// A peak pin equal to the veryslow pin would mean the peak mode never engaged on `hdr_image()`.
#[test]
fn the_peak_pin_is_not_the_veryslow_pin() {
    for t in IspcTarget::ALL {
        let veryslow = recorded(t)
            .iter()
            .find(|(l, _)| *l == Bc6hLevel::Veryslow)
            .unwrap()
            .1;
        assert_ne!(recorded_peak(t), veryslow, "{t:?}");
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

/// The paths the new tests walk, as `check_pins` does: every supported target up to `ispc_target()`. A default run
/// covers SSE2 and AVX2; `SDC_TEST_AVX512=1` adds AVX-512 (Note HZ3).
fn paths() -> impl Iterator<Item = IspcTarget> {
    IspcTarget::ALL
        .into_iter()
        .filter(|t| t.supported() && *t <= ispc_target())
}

/// Encodes one 4x4 block (row-major texels) on `target`.
fn encode_block(px: &[[u16; 4]; 16], level: Bc6hLevel, target: IspcTarget) -> [u8; 16] {
    let mut out = [0u8; 16];
    bc6h_compress_rows_on(px, 4, 4, &mut out, level, target);
    out
}

/// The block's decode as half bits (exact: every UF16 decode is a half).
fn decode_halves(b: &[u8; 16]) -> [[u16; 3]; 16] {
    let mut d = [0f32; 48];
    bcdec_rs::bc6h_float(b, &mut d, 12, false);
    std::array::from_fn(|t| std::array::from_fn(|c| f16::from_f32(d[t * 3 + c]).to_bits()))
}

/// `n` bits of the block starting at bit `lo`.
fn field(b: &[u8; 16], lo: u32, n: u32) -> u32 {
    ((u128::from_le_bytes(*b) >> lo) & ((1u128 << n) - 1)) as u32
}

/// A grey texel: R = G = B = `v`, alpha 1.0.
fn neutral(v: f32) -> [u16; 4] {
    let h = f16::from_f32(v).to_bits();
    [h, h, h, f16::ONE.to_bits()]
}

#[test]
fn neutral_blocks_encode_to_a_valid_mode_and_decode_close() {
    for target in paths() {
        for level in [
            Bc6hLevel::Veryfast,
            Bc6hLevel::Veryslow,
            Bc6hLevel::VeryslowPeak,
        ] {
            for v in [1.0f32, 100.0] {
                let b = encode_block(&[neutral(v); 16], level, target);
                let m5 = field(&b, 0, 5);
                let reserved = matches!(m5, 0b10011 | 0b10111 | 0b11011 | 0b11111);
                assert!(
                    !reserved,
                    "{target:?} {level:?} {v}: reserved mode {m5:#07b}"
                );
                for (t, px) in decode_halves(&b).iter().enumerate() {
                    for (c, &h) in px.iter().enumerate() {
                        let d = f16::from_bits(h).to_f32();
                        assert!(
                            (d - v).abs() <= 0.02 * v,
                            "{target:?} {level:?} {v} texel {t} ch {c}: {d}"
                        );
                    }
                }
            }
        }
    }
}

/// `get_span` truncated mode 2's 204.8 to 204, so a block whose widest channel spans 99 half codes (204.39 UF16 units)
/// was kept out of mode 2 at Fast (survey #5).
#[test]
fn mode_2_takes_a_span_inside_its_fractional_limit() {
    // R is the widest channel: exactly 99 half codes, 0x3C00..=0x3C63. The left two columns (one region of BC6H
    // partition 0) vary R with G fixed; the right two columns vary G with R fixed, so no single line fits the block.
    const BASE: u16 = 0x3C00;
    const R_STEPS: [u16; 8] = [0, 14, 28, 42, 56, 70, 84, 99];
    const G_STEPS: [u16; 8] = [0, 8, 16, 24, 32, 40, 48, 56];
    let one = f16::ONE.to_bits();
    let mut px = [[BASE, BASE, BASE, one]; 16];
    for row in 0..4usize {
        for col in 0..2usize {
            px[row * 4 + col][0] = BASE + R_STEPS[row * 2 + col];
        }
        for col in 2..4usize {
            px[row * 4 + col][1] = BASE + G_STEPS[row * 2 + col - 2];
        }
    }
    let modes: Vec<(IspcTarget, u32)> = paths()
        .map(|target| {
            let b = encode_block(&px, Bc6hLevel::Fast, target);
            let mode = if field(&b, 0, 2) < 2 {
                field(&b, 0, 2)
            } else {
                field(&b, 0, 5)
            };
            eprintln!("{target:?} mode {mode}");
            (target, mode)
        })
        .collect();
    for (target, mode) in modes {
        assert_ne!(mode, 0, "{target:?}: mode 0 kept the block out of mode 2");
        let one_region = matches!(mode, 3 | 7 | 11 | 15);
        assert!(
            one_region || mode == 2,
            "{target:?}: a two-region mode other than mode 2 with R widest: {mode}"
        );
    }
}

/// A seeded LCG (the one `hdr_image` uses).
struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 8
    }
    /// A value in `lo..=hi`.
    fn range(&mut self, lo: u32, hi: u32) -> u16 {
        (lo + self.next() % (hi - lo + 1)) as u16
    }
}

const MAX_UF16: u32 = 0x7BFF;
const STRIP_BLOCKS: usize = 2048;

/// A strip of `STRIP_BLOCKS` blocks (8192 px wide, 4 rows), each block's 16 texels from `block`.
fn strip(mut block: impl FnMut() -> [[u16; 3]; 16]) -> Vec<[u16; 4]> {
    let w = STRIP_BLOCKS * 4;
    let mut px = vec![[0u16; 4]; w * 4];
    for b in 0..STRIP_BLOCKS {
        for (t, c) in block().iter().enumerate() {
            px[(t / 4) * w + b * 4 + t % 4] = [c[0], c[1], c[2], f16::ONE.to_bits()];
        }
    }
    px
}

/// Every level on every path takes random HDR blocks without tripping an assert. The kernel is built with its asserts
/// on, and an assert aborts the process, so this test fails if any block trips one. It checks nothing else: a UF16
/// decode is at most 0x7BFF by construction, so a bound on the output could not fail.
///
/// Four strips: (a) every channel of every texel a random half; (b) two clusters, each one random colour plus noise
/// of up to 64 half codes, a random 8/8 texel split; (c) narrow spans; (d) black beside bright neutral texels.
#[test]
fn random_blocks_encode_at_every_level_on_every_path() {
    let mut r = Lcg(0x1234_5678);
    let strips = [
        strip(|| std::array::from_fn(|_| std::array::from_fn(|_| r.range(0, MAX_UF16)))),
        {
            let mut r = Lcg(0x2345_6789);
            strip(|| {
                // A random half/half split: a shuffled texel order, the first eight in cluster 0.
                let mut order: [usize; 16] = std::array::from_fn(|i| i);
                for i in (1..16).rev() {
                    order.swap(i, r.next() as usize % (i + 1));
                }
                let base: [[u16; 3]; 2] =
                    std::array::from_fn(|_| std::array::from_fn(|_| r.range(0, MAX_UF16)));
                let mut b = [[0u16; 3]; 16];
                for (rank, &t) in order.iter().enumerate() {
                    for c in 0..3 {
                        let v = base[rank / 8][c] as u32 + r.range(0, 64) as u32;
                        b[t][c] = v.min(MAX_UF16) as u16;
                    }
                }
                b
            })
        },
        {
            let mut r = Lcg(0x3456_789a);
            strip(|| {
                let base: [u32; 3] = std::array::from_fn(|_| r.range(0x0400, 0x7000) as u32);
                std::array::from_fn(|_| {
                    std::array::from_fn(|c| (base[c] + r.range(0, 2048) as u32) as u16)
                })
            })
        },
        {
            let mut r = Lcg(0x4567_89ab);
            strip(|| {
                let mut mask = r.range(0, 0xFFFF);
                if mask == 0 {
                    mask = 1;
                } else if mask == 0xFFFF {
                    mask = 0x7FFF;
                }
                std::array::from_fn(|t| {
                    if mask >> t & 1 == 0 {
                        [0; 3]
                    } else {
                        [r.range(0x6000, MAX_UF16); 3]
                    }
                })
            })
        },
    ];
    let levels = [
        Bc6hLevel::Veryfast,
        Bc6hLevel::Fast,
        Bc6hLevel::Basic,
        Bc6hLevel::Slow,
        Bc6hLevel::Veryslow,
        Bc6hLevel::VeryslowPeak,
    ];
    for target in paths() {
        for (i, px) in strips.iter().enumerate() {
            for level in levels {
                let mut out = vec![0u8; STRIP_BLOCKS * 16];
                bc6h_compress_rows_on(px, (STRIP_BLOCKS * 4) as u32, 4, &mut out, level, target);
                eprintln!("strip {i} {target:?} {level:?} survived");
            }
        }
    }
}

#[test]
#[should_panic(expected = "multiple of 4")]
fn rejects_unaligned_width() {
    let px = vec![[0u16; 4]; 6 * 4];
    let mut out = [0u8; 32];
    bc6h_compress_rows(&px, 6, 4, &mut out, Bc6hLevel::Veryslow);
}
