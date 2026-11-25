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
/// (Note LO).
const RECORDED: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "a1d86ce13ea28a4845aaaf64ecdf948763f67c6d514cea8f598ed254e848ecdc",
    ),
    (
        Bc6hLevel::Fast,
        "8d00c0c10ee93e258239d55757965af02cd1a5431d44590b5979703e4a26aae0",
    ),
    (
        Bc6hLevel::Basic,
        "21a38f0cf43a75ae4ab867b5c87414d4d621c5d4038634e71133eb7551317add",
    ),
    (
        Bc6hLevel::Slow,
        "140a5cdb9359ff82d3f8d6cf9305a99be0b14d9d59ad971419b3d18ebc56929a",
    ),
    (
        Bc6hLevel::Veryslow,
        "6223e3309df86ec4d88be0fb872b0ac114d6466269a2811780fc2849da6fb788",
    ),
];

/// The SSE2 path, recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-22, from the ISPC TC patch 0004 objects
/// (Note LO). The SSE4 and AVX paths, dropped by Note JB, wrote the same bytes before patch 0004.
const RECORDED_PRE_AVX2: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "98215a3ca45c12c0f2ebc70fd0a14781b9368f159fc89ec1b3d098e1ac400d08",
    ),
    (
        Bc6hLevel::Fast,
        "5a27639f81f2781b265a8d7e74bd17da579b9a63b82f23dc39a4df919369a341",
    ),
    (
        Bc6hLevel::Basic,
        "2ceecbcd810ffc4bd599fb87e5f8bd39335d93c008128d5e19989e3b171fee41",
    ),
    (
        Bc6hLevel::Slow,
        "563d7eb914134fca56acf98f8925b8f72d6dd5f22183d4351026745f7812a5c9",
    ),
    (
        Bc6hLevel::Veryslow,
        "4e39ad0b15e856d80c6d7994892d202b3a9bfd3f6d5e309c6e5f492b30182597",
    ),
];

/// AVX-512 path, recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-22, from the ISPC TC patch 0004 objects built
/// with ISPC 1.32.0 (Note LO). A test build encodes on it only with the AVX2 cap lifted (`SDC_TEST_AVX512=1`).
const RECORDED_AVX512: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "593df3a2389f7fb381458647461d28ad7a7b7602cbc37d9809ce019bba023cff",
    ),
    (
        Bc6hLevel::Fast,
        "ad1505e9e6d1c49b582c6bcb2fadff07be08696eebae8504fc13d63147a29cd8",
    ),
    (
        Bc6hLevel::Basic,
        "610dcf00562c7bfb07dbcda1981aa749c4db05b0cfcb37d1995bb3500142260f",
    ),
    (
        Bc6hLevel::Slow,
        "cf1d4c07d29e1c2ca0d72e787718291d168db71728ec0fdfe8d02ca41f447bd5",
    ),
    (
        Bc6hLevel::Veryslow,
        "58f8016368c8cd211e6cd17cf60bdcb58a0bcba2de9bde3fac2d0c48a43a8607",
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

/// The five shipped levels encode byte-identically to the recorded objects (the patch 0004 objects, Note LO), on each
/// path the run reads (SSE2 and AVX2; AVX-512 only with the cap lifted). The paths differ, so each has its own hashes.
#[test]
fn shipped_levels_are_byte_identical_to_the_recorded_objects() {
    check_pins(|t| recorded(t).to_vec());
}

/// The peak mode (`VeryslowPeak`) per path: SSE2, AVX2, AVX-512 (AVX-512 read only with the cap lifted).
/// Recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-22, from the ISPC TC patch 0004 objects (Note LO).
const RECORDED_PEAK: [&str; 3] = [
    "a2b6930bf3296db9b4caeff929581d32ed18da0ea2cbe4a27805dafff1336252",
    "303e36d9d646693463545040a814a900cc45bf9506b4101d2066400d1a502a7f",
    "6ca34e50b1565c7c9020d998cd109f47e46e5a634adfdbe29d16473d642ae52f",
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

/// `block_quant` stored each texel's float error in an `int`; above 2^31 (a black texel beside one brighter than about
/// 1344.0) x86 gives `INT_MIN`, which beat every honest candidate (survey #1).
#[test]
fn black_beside_bright_keeps_both() {
    const BRIGHT: f32 = 8192.0;
    let black = neutral(0.0);
    let bright = neutral(BRIGHT);
    // (i) half black, half bright (guarded in peak mode); (ii) one black texel among fifteen bright (unguarded).
    let blocks: [(&str, [[u16; 4]; 16]); 2] = [
        (
            "half black",
            std::array::from_fn(|t| if t < 8 { black } else { bright }),
        ),
        (
            "one black",
            std::array::from_fn(|t| if t == 5 { black } else { bright }),
        ),
    ];
    let mut failures = Vec::new();
    for (name, px) in &blocks {
        for target in paths() {
            for level in [
                Bc6hLevel::Veryfast,
                Bc6hLevel::Fast,
                Bc6hLevel::Basic,
                Bc6hLevel::Slow,
                Bc6hLevel::Veryslow,
                Bc6hLevel::VeryslowPeak,
            ] {
                let dec = decode_halves(&encode_block(px, level, target));
                for (t, texel) in dec.iter().enumerate() {
                    let is_black = px[t][0] == black[0];
                    for (c, &h) in texel.iter().enumerate() {
                        let d = f16::from_bits(h).to_f32();
                        let ok = if is_black {
                            d < 0.01 * BRIGHT
                        } else {
                            (d - BRIGHT).abs() <= 0.1 * BRIGHT
                        };
                        if !ok {
                            failures.push(format!(
                                "{name} {target:?} {level:?} texel {t} ch {c}: decoded {d}"
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} texel channels off:\n{}",
        failures.len(),
        failures.join("\n")
    );
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
    let strips = random_strips();
    let levels = RANDOM_STRIP_LEVELS;
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

/// The levels the random strips are encoded at, in the order `random_strip_hashes` prints them.
const RANDOM_STRIP_LEVELS: [Bc6hLevel; 6] = [
    Bc6hLevel::Veryfast,
    Bc6hLevel::Fast,
    Bc6hLevel::Basic,
    Bc6hLevel::Slow,
    Bc6hLevel::Veryslow,
    Bc6hLevel::VeryslowPeak,
];

/// Prints `<path> <strip> <level> <sha256 hex of the encoded strip>` for the random strips, one line per path in
/// `paths()`, strip (a-d) and level. It asserts nothing: run it before and after a kernel change that must keep every
/// byte and compare the two outputs (`-- --ignored --exact random_strip_hashes --nocapture`).
#[test]
#[ignore = "prints the random strips' output hashes: compare them before and after a byte-neutral kernel change"]
fn random_strip_hashes() {
    let strips = random_strips();
    for target in paths() {
        for (name, px) in ["a", "b", "c", "d"].into_iter().zip(&strips) {
            for level in RANDOM_STRIP_LEVELS {
                let mut out = vec![0u8; STRIP_BLOCKS * 16];
                bc6h_compress_rows_on(px, (STRIP_BLOCKS * 4) as u32, 4, &mut out, level, target);
                println!(
                    "{target:?} {name} {level:?} {}",
                    hex::encode(Sha256::digest(&out))
                );
            }
        }
    }
}

/// The four random strips of `random_blocks_encode_at_every_level_on_every_path`: (a) every channel of every texel a
/// random half; (b) two clusters, each one random colour plus noise of up to 64 half codes, a random 8/8 texel split;
/// (c) narrow spans; (d) black beside bright neutral texels.
fn random_strips() -> [Vec<[u16; 4]>; 4] {
    let mut r = Lcg(0x1234_5678);
    [
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
    ]
}

/// Bounds (E_lo, E_hi) on the encoder's own error for one encoded block, read from the output bytes alone, in half-code
/// units (the encoder's UF16 error over the common factor (64/31)^2). The encoder scores the squared error of the
/// decoded UF16 interpolant u against `h_src * 64 / 31`, unweighted over R, G and B. The decoder writes
/// `h = (u * 31) >> 6`, so `u * 31 / 64` lies in [h, h + 1): with e = h - h_src a channel's error lies between e^2
/// and (e + 1)^2 (survey #3).
fn err_bounds(src: &[[u16; 4]; 16], dec: &[[u16; 3]; 16]) -> (f64, f64) {
    let (mut lo, mut hi) = (0i64, 0i64);
    for (s, d) in src.iter().zip(dec) {
        for c in 0..3 {
            let e = d[c] as i64 - s[c] as i64;
            let (a, b) = (e * e, (e + 1) * (e + 1));
            lo += a.min(b);
            hi += a.max(b);
        }
    }
    (lo as f64, hi as f64)
}

/// `err_bounds` of one block encoded at `level` on `target`.
fn level_bounds(px: &[[u16; 4]; 16], level: Bc6hLevel, target: IspcTarget) -> (f64, f64) {
    err_bounds(px, &decode_halves(&encode_block(px, level, target)))
}

/// A deterministic HDR block from index `i` (half codes in 0x2000..=0x7000, alpha 1.0), for the refinement search.
/// Half the blocks are a gradient along a random direction across the block, with a random colour direction and up
/// to 64 half codes of noise; the other half draw each channel from a random range. One block in eight gets one
/// outlier texel. The drawn span of the widest channel is 600..=12288 half codes (an outlier can widen a block to
/// 20480), so the one-region modes in play are 10 to 12.
/// Integer arithmetic only, so the blocks are the same on every machine.
fn candidate(i: u32) -> [[u16; 4]; 16] {
    const LO: i64 = 0x2000;
    const HI: i64 = 0x7000;
    let mut r = Lcg(i.wrapping_mul(0x9E37_79B9) ^ 0x2C1B_3C6D);
    r.next();
    let one = f16::ONE.to_bits();
    let mut px = [[0, 0, 0, one]; 16];
    let span = r.range(600, 0x3000) as i64;
    let widest = (r.next() % 3) as usize;
    if r.next().is_multiple_of(2) {
        // A gradient: t = the texel's position along (dx, dy), 0..=1 over the block, times each channel's amplitude.
        let (mut dx, dy) = (r.range(0, 6) as i64 - 3, r.range(0, 6) as i64 - 3);
        if dx == 0 && dy == 0 {
            dx = 1;
        }
        let pos: [i64; 16] = std::array::from_fn(|t| (t % 4) as i64 * dx + (t / 4) as i64 * dy);
        let (pmin, pmax) = (*pos.iter().min().unwrap(), *pos.iter().max().unwrap());
        let amp: [i64; 3] = std::array::from_fn(|c| {
            if c == widest {
                span
            } else {
                span * (r.range(0, 200) as i64 - 100) / 100
            }
        });
        let noise = r.range(0, 64) as i64;
        for c in 0..3 {
            // The base keeps base + amp * t inside LO..=HI for every t in 0..=1.
            let (lo, hi) = (LO - amp[c].min(0), HI - amp[c].max(0));
            let base = r.range(lo as u32, hi as u32) as i64;
            for (t, p) in pos.iter().enumerate() {
                let v =
                    base + amp[c] * (p - pmin) / (pmax - pmin) + r.range(0, noise as u32) as i64
                        - noise / 2;
                px[t][c] = v.clamp(LO, HI) as u16;
            }
        }
    } else {
        // A random range per channel; the widest channel spans `span`, the others up to it.
        for c in 0..3 {
            let s = if c == widest {
                span
            } else {
                r.range(0, span as u32) as i64
            };
            let lo = r.range(LO as u32, (HI - s) as u32);
            for texel in px.iter_mut() {
                texel[c] = r.range(lo as u32, lo as u32 + s as u32);
            }
        }
    }
    if r.next().is_multiple_of(8) {
        let t = (r.next() % 16) as usize;
        for v in &mut px[t][..3] {
            *v = r.range(LO as u32, HI as u32);
        }
    }
    px
}

/// The candidates of `lo..hi` (at most `STRIP_BLOCKS`) whose Basic or Veryslow encode on `target` ends certainly worse
/// than Veryfast's: (index, level, the level's E_lo, Veryfast's E_hi).
fn refine_violations(lo: u32, hi: u32, target: IspcTarget) -> Vec<(u32, Bc6hLevel, f64, f64)> {
    let blocks: Vec<[[u16; 4]; 16]> = (lo..hi).map(candidate).collect();
    let mut n = 0;
    let px = strip(|| {
        // A short last chunk repeats its first block to fill the strip.
        let c = blocks.get(n).unwrap_or(&blocks[0]);
        n += 1;
        std::array::from_fn(|t| [c[t][0], c[t][1], c[t][2]])
    });
    let encode = |level| {
        let mut out = vec![0u8; STRIP_BLOCKS * 16];
        bc6h_compress_rows_on(&px, (STRIP_BLOCKS * 4) as u32, 4, &mut out, level, target);
        out
    };
    let bounds = |out: &[u8], b: usize| {
        let bytes: [u8; 16] = out[b * 16..b * 16 + 16].try_into().unwrap();
        err_bounds(&blocks[b], &decode_halves(&bytes))
    };
    let veryfast = encode(Bc6hLevel::Veryfast);
    let others = [
        (Bc6hLevel::Basic, encode(Bc6hLevel::Basic)),
        (Bc6hLevel::Veryslow, encode(Bc6hLevel::Veryslow)),
    ];
    let mut found = Vec::new();
    for b in 0..blocks.len() {
        let (_, vf_hi) = bounds(&veryfast, b);
        for (level, out) in &others {
            let (e_lo, _) = bounds(out, b);
            if e_lo > vf_hi {
                found.push((lo + b as u32, *level, e_lo, vf_hi));
            }
        }
    }
    found
}

/// Indices of `candidate` blocks where a pre-fix kernel ended Basic or Veryslow certainly worse than Veryfast on the
/// AVX2 path (printed by `search_refine_witnesses`).
const REFINE_WITNESSES: &[u32] = &[25, 95, 292, 364, 395, 432, 643, 678];

/// Prints up to eight `candidate` indices in `0..1 << 20` where Basic or Veryslow ends certainly worse than Veryfast
/// on the AVX2 path (`E_lo(level) > E_hi(Veryfast)`), each re-checked through a one-block encode, then the count
/// scanned and the time. Each hit is a witness for `REFINE_WITNESSES`.
#[test]
#[ignore = "search for refine witnesses: run with --ignored --nocapture after a kernel change"]
fn search_refine_witnesses() {
    const END: u32 = 1 << 20;
    const CHUNK: u32 = STRIP_BLOCKS as u32;
    let target = IspcTarget::Avx2;
    if !target.supported() {
        eprintln!("skipped: the AVX2 path is not supported on this CPU");
        return;
    }
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8)) as u32;
    let start = std::time::Instant::now();
    let mut hits = Vec::new();
    let mut scanned = 0;
    while scanned < END && hits.len() < 8 {
        let mut batch: Vec<_> = std::thread::scope(|s| {
            let jobs: Vec<_> = (0..threads)
                .map(|t| scanned + t * CHUNK)
                .filter(|&lo| lo < END)
                .map(|lo| s.spawn(move || refine_violations(lo, (lo + CHUNK).min(END), target)))
                .collect();
            jobs.into_iter().flat_map(|j| j.join().unwrap()).collect()
        });
        scanned = (scanned + threads * CHUNK).min(END);
        batch.sort_by_key(|h| h.0);
        for (i, level, e_lo, vf_hi) in batch {
            if hits.len() == 8 {
                break;
            }
            let px = candidate(i);
            let (lo1, _) = level_bounds(&px, level, target);
            let (_, hi1) = level_bounds(&px, Bc6hLevel::Veryfast, target);
            if lo1 > hi1 {
                println!("witness {i} {level:?}: E_lo {e_lo} > Veryfast E_hi {vf_hi}");
                if !hits.contains(&i) {
                    hits.push(i);
                }
            } else {
                println!("{i} {level:?}: the strip encode violates, the one-block encode does not");
            }
        }
    }
    println!(
        "scanned {scanned} candidates in {:.1} s on {target:?}; witnesses {hits:?}",
        start.elapsed().as_secs_f64()
    );
}

/// Survey #3. Veryfast writes the first one-region candidate that Basic and Veryslow also make: the same mode choice,
/// the same `qbounds`, unrefined (Veryfast has no refinement and nothing runs before its one-region encode). Basic
/// keeps the better of its two-region result and its submitted one-region result, and Veryslow the best of all its
/// candidates. So Basic and Veryslow can end worse than Veryfast only if a refinement pass made the one-region result
/// worse and that pass was submitted. The bounds come from the output bytes (`err_bounds`); they could fail falsely
/// only if the encoder's f32 error sat within f32 rounding of Veryfast's E_hi, so there is no tolerance.
#[test]
fn refinement_never_ends_worse_than_veryfast() {
    let mut failures = Vec::new();
    for target in paths() {
        for &i in REFINE_WITNESSES {
            let px = candidate(i);
            let (vf_lo, vf_hi) = level_bounds(&px, Bc6hLevel::Veryfast, target);
            for level in [Bc6hLevel::Basic, Bc6hLevel::Veryslow] {
                let (lo, hi) = level_bounds(&px, level, target);
                if lo > vf_hi {
                    failures.push(format!(
                        "witness {i} {target:?} {level:?}: E_lo {lo} > Veryfast E_hi {vf_hi} \
                         ({level:?} E_hi {hi}, Veryfast E_lo {vf_lo})"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} ends worse than Veryfast:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// A copy of the kernel's `unpack_to_uf16`: the UF16 value the decoder rebuilds from code `q` of a `bits`-bit endpoint.
fn unpack_to_uf16(q: u32, bits: u32) -> u32 {
    if bits >= 15 {
        q
    } else if q == 0 {
        0
    } else if q == (1 << bits) - 1 {
        0xFFFF
    } else {
        (2 * q + 1) << (15 - bits)
    }
}

/// The `bits`-bit code whose rebuilt value is nearest to `v` (the lower one on a tie), in constant time: the code
/// whose cell holds `v` and its two neighbours are the only candidates (`nearest_code_equals_the_exhaustive_search`
/// checks it against every code).
fn nearest_code(v: f64, bits: u32) -> u32 {
    let levels = 1i64 << bits;
    let q0 = ((v * levels as f64 / 65536.0).floor() as i64).clamp(0, levels - 1);
    let mut best = (f64::MAX, 0u32);
    for q in (q0 - 1).max(0)..=(q0 + 1).min(levels - 1) {
        let dist = (unpack_to_uf16(q as u32, bits) as f64 - v).abs();
        if dist < best.0 {
            best = (dist, q as u32);
        }
    }
    best.1
}

/// The `bits`-bit rounding the kernel shipped (in f32), used only to pick inputs where it missed the nearest code.
fn shipped_code(v: f32, bits: u32) -> u32 {
    let top = (1i32 << bits) - 1;
    ((v / 65535.0 * top as f32 + 0.5) as i32).clamp(0, top) as u32
}

/// The UF16 value `h` (a half code) becomes in the kernel's `bc6h_setup`, in f32 and in f64.
fn uf16_of_half(h: u16) -> (f32, f64) {
    ((h as f32 / 31.0) * 64.0, h as f64 * 64.0 / 31.0)
}

/// The half codes whose `bits`-bit code under the shipped rule is not the nearest code, as `(h, nearest, shipped)`.
/// Only codes `margin` UF16 units clear of every edge between two codes under both rules (both are monotone step
/// functions, so equal codes `margin` either side mean no edge in between), so the nearest code is strict.
fn mismatch_inputs(bits: u32, margin: f32) -> Vec<(u16, u32, u32)> {
    let mut found = Vec::new();
    for h in 0..=MAX_UF16 as u16 {
        let (v32, v64) = uf16_of_half(h);
        let (s, n) = (shipped_code(v32, bits), nearest_code(v64, bits));
        let m = margin as f64;
        let clear = shipped_code(v32 - margin, bits) == s
            && shipped_code(v32 + margin, bits) == s
            && nearest_code(v64 - m, bits) == n
            && nearest_code(v64 + m, bits) == n;
        if clear && s != n {
            found.push((h, n, s));
        }
    }
    found
}

#[test]
fn nearest_code_equals_the_exhaustive_search() {
    // Pure check of the test helper: every code, at the values where two codes meet and a few around them.
    for bits in [10, 11, 12] {
        let levels = 1u32 << bits;
        let rebuilt: Vec<f64> = (0..levels)
            .map(|q| unpack_to_uf16(q, bits) as f64)
            .collect();
        let exhaustive = |v: f64| {
            let mut best = (f64::MAX, 0u32);
            for (q, r) in rebuilt.iter().enumerate() {
                if (r - v).abs() < best.0 {
                    best = ((r - v).abs(), q as u32);
                }
            }
            best.1
        };
        let mut values: Vec<f64> = (0..=65535).step_by(97).map(f64::from).collect();
        for pair in rebuilt.windows(2) {
            let mid = (pair[0] + pair[1]) / 2.0;
            values.extend([mid - 0.25, mid, mid + 0.25]);
        }
        values.extend([0.0, 1.0, 65534.0, 65535.0]);
        for v in values {
            assert_eq!(nearest_code(v, bits), exhaustive(v), "{bits} bits, v {v}");
        }
    }
}

/// The kernel's `ep_quant_bc6h` for one endpoint, mirrored line by line in f32: below 15 bits the interior code from one
/// multiply (`ceil(e * levels / 65536) - 1`, clamped to 1..levels-2), then one compare with an end code when the
/// interior code is next to it; 15 bits and more keep upstream's rounding.
fn kernel_ep_quant(ep: f32, bits: u32) -> u32 {
    let levels = 1i32 << bits;
    let v = if bits >= 15 {
        (ep / (256.0 * 256.0 - 1.0) * (levels - 1) as f32 + 0.5) as i32
    } else {
        let e = ep.clamp(0.0, 65535.0);
        let mut v = ((e * levels as f32 / 65536.0).ceil() as i32 - 1).clamp(1, levels - 2);
        if v == 1 {
            if e <= (unpack_to_uf16(1, bits) as f32 - e).abs() {
                v = 0;
            }
        } else if v == levels - 2 {
            let d_top = (unpack_to_uf16((levels - 1) as u32, bits) as f32 - e).abs();
            if d_top < (unpack_to_uf16(v as u32, bits) as f32 - e).abs() {
                v = levels - 1;
            }
        }
        v
    };
    v.clamp(0, levels - 1) as u32
}

/// The kernel's one-multiply endpoint rule (`kernel_ep_quant`) picks the nearest code, the lower on a tie, for every
/// integer UF16 value at every precision a BC6H mode uses (6 bits in mode 9 to 16 in mode 13). The integer values
/// include every tie between two interior codes (multiples of 65536 / 2^bits), where truncating instead of `ceil - 1`
/// would take the higher code, and the tie between code 0 and code 1 (0.75 * 65536 / 2^bits). The tie between the top
/// code (65535) and the one below it, at 65535.5 - 0.75 * 65536 / 2^bits, is a half-integer, so it is checked on its
/// own, with values a quarter either side of it and of every other tie.
#[test]
fn the_one_multiply_rule_takes_the_nearest_code_for_every_value() {
    for bits in [6, 7, 8, 9, 10, 11, 12, 16] {
        for e in 0..=65535u32 {
            assert_eq!(
                kernel_ep_quant(e as f32, bits),
                nearest_code(e as f64, bits),
                "{bits} bits, e {e}"
            );
        }
        if bits >= 15 {
            continue;
        }
        let s = 65536.0 / (1u32 << bits) as f64;
        let top_tie = 65535.5 - 0.75 * s;
        let mut ties: Vec<f64> = (1..1u32 << bits).map(|k| k as f64 * s).collect();
        ties.extend([0.75 * s, top_tie]);
        for tie in ties {
            for v in [tie - 0.25, tie, tie + 0.25] {
                if !(0.0..=65535.0).contains(&v) {
                    continue;
                }
                assert_eq!(
                    kernel_ep_quant(v as f32, bits),
                    nearest_code(v, bits),
                    "{bits} bits, v {v}"
                );
            }
        }
        let below_top = (1u32 << bits) - 2;
        assert_eq!(
            kernel_ep_quant(top_tie as f32, bits),
            below_top,
            "{bits} bits: the top tie keeps the lower code"
        );
    }
}

/// Survey #2. The decoder rebuilds an interior code q of a `bits`-bit endpoint at (q + 0.5) * 65536 / 2^bits
/// (`unpack_to_uf16`, the D3D formula), but `ep_quant_bc6h` rounded by `ep * (2^bits - 1) / 65535`. That misses the
/// nearest code near both ends of the range: one too high near 0, one too low near the top.
///
/// Only G and B are tested. A channel that is constant over the block gets endpoints close to the channel's mean, not
/// exactly at it: the PCA axis comes from an f32 covariance built from raw moments (`covar_from_stats`), whose rounding
/// tilts the axis so it gains a small component in a constant channel (and the renormalisation with a fast-math
/// reciprocal square root can move the endpoints by several UF16 units along R). So R varies only to force mode 10 (its
/// span is far beyond mode 11's 7782.3 UF16 units, so Veryfast writes mode 10: 10-bit absolute endpoints), and G and B
/// are each constant. The test holds because that mode's 10-bit step is 64 UF16 units, and every input is at least 4
/// units from an edge between two codes, so a small shift of the endpoint cannot change the nearest code.
///
/// The inputs are half codes h (UF16 value v = h * 64 / 31) where the shipped rule and the nearest code differ, the
/// nearest code is strict, and v is at least 4 UF16 units from both edges of that mismatch window: G from nearest codes
/// 8..=150 (the shipped rule picks one too high), B from 870..=1015 (one too low). Four more blocks take the end
/// codes: G with nearest code 0 (shipped 1) and B with nearest 1023 (shipped 1022).
///
/// Not covered here, on purpose: a value midway between two codes (the kernel takes the lower, as `nearest_code`, but a
/// tie is a knife edge that the encoder's float endpoint can fall either side of) and the delta modes 11 and 12 (an
/// endpoint pair can carry different codes for a channel that is constant over the block).
#[test]
fn one_region_endpoints_take_the_nearest_code() {
    let (mut g_list, mut b_list) = (Vec::new(), Vec::new());
    let (mut g_end, mut b_end) = (Vec::new(), Vec::new());
    for (h, n, s) in mismatch_inputs(10, 4.0) {
        if (8..=150).contains(&n) && s == n + 1 {
            g_list.push(h);
        } else if (870..=1015).contains(&n) && s + 1 == n {
            b_list.push(h);
        } else if n == 0 && s == 1 {
            g_end.push(h);
        } else if n == 1023 && s == 1022 {
            b_end.push(h);
        }
    }
    assert!(
        !g_list.is_empty() && !b_list.is_empty() && !g_end.is_empty() && !b_end.is_empty(),
        "no inputs: G {g_list:?} B {b_list:?} G end {g_end:?} B end {b_end:?}"
    );
    let mut r = Lcg(0x5678_9abc);
    let mut blocks: Vec<(u16, u16)> = (0..16)
        .map(|_| {
            let g = g_list[r.next() as usize % g_list.len()];
            let b = b_list[r.next() as usize % b_list.len()];
            (g, b)
        })
        .collect();
    // The end codes, which the candidate window clamps: G where code 0 is nearest (shipped: 1), B where code 1023 is
    // (shipped: 1022).
    blocks.extend((0..4).map(|_| {
        let g = g_end[r.next() as usize % g_end.len()];
        let b = b_end[r.next() as usize % b_end.len()];
        (g, b)
    }));
    let one = f16::ONE.to_bits();
    let mut failures = Vec::new();
    for target in paths() {
        for (k, &(g, b)) in blocks.iter().enumerate() {
            let px: [[u16; 4]; 16] =
                std::array::from_fn(|t| [if t < 8 { 0x2000 } else { 0x7000 }, g, b, one]);
            let out = encode_block(&px, Bc6hLevel::Veryfast, target);
            assert_eq!(
                field(&out, 0, 5),
                3,
                "block {k} {target:?}: not mode 10, the fixture assumption broke"
            );
            let codes = [
                field(&out, 15, 10),
                field(&out, 45, 10),
                field(&out, 25, 10),
                field(&out, 55, 10),
            ];
            let (vg, vb) = (uf16_of_half(g).1, uf16_of_half(b).1);
            let (ng, nb) = (nearest_code(vg, 10), nearest_code(vb, 10));
            if codes != [ng, ng, nb, nb] {
                let (sg, sb) = (
                    shipped_code(uf16_of_half(g).0, 10),
                    shipped_code(uf16_of_half(b).0, 10),
                );
                failures.push(format!(
                    "block {k} {target:?} (G half {g:#06x}, B half {b:#06x}): G codes {} {}, B codes {} {}; \
                     nearest G {ng} B {nb}; shipped rule G {sg} B {sb}",
                    codes[0], codes[1], codes[2], codes[3]
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} blocks miss the nearest code:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Eight texels of half bits `(7936, 7936, 0)` and eight of `(0, 0, 15872)` give UF16 colours `(16384, 16384, 0)` and
/// `(0, 0, 32768)`. The scaled covariance has every row summing to 0 exactly and absorbs the `1e-6` eps, so the first
/// power iteration gives the zero vector and `RSQRT(0) * 0` is NaN (survey section 11). Whether the shipped objects
/// reach it depends on how ISPC lowers `/stats[14]`, which differs by CPU target. Only the AVX-512 object reaches the
/// zero vector, so only a run with `SDC_TEST_AVX512=1` sees a regression.
///
/// The two colours go in alternate texels and in two halves of the block. At every level on every path, every
/// decoded channel of every texel lies within 256 half codes of its source. Mode 10's interior codes rebuild at
/// `(q + 0.5) * 64`, so each colour lands about 32 UF16 units (some 16 half codes) from its source: 256 is a loose
/// bound that only a black or near-white NaN decode breaks; do not tighten it.
#[test]
fn pca_survives_a_zero_covariance_row_sum() {
    let a = [7936u16, 7936, 0];
    let b = [0u16, 0, 15872];
    let alternate: [[u16; 3]; 16] = std::array::from_fn(|t| if t % 2 == 0 { a } else { b });
    let halves: [[u16; 3]; 16] = std::array::from_fn(|t| if t < 8 { a } else { b });
    let mut failures = Vec::new();
    for target in paths() {
        for (name, src) in [("alternate", &alternate), ("halves", &halves)] {
            let px: [[u16; 4]; 16] =
                std::array::from_fn(|t| [src[t][0], src[t][1], src[t][2], f16::ONE.to_bits()]);
            for level in RANDOM_STRIP_LEVELS {
                let dec = decode_halves(&encode_block(&px, level, target));
                let mut bad = Vec::new();
                for t in 0..16 {
                    for c in 0..3 {
                        if dec[t][c].abs_diff(src[t][c]) > 256 {
                            bad.push(format!("texel {t} ch {c}: {} -> {}", src[t][c], dec[t][c]));
                        }
                    }
                }
                eprintln!("{target:?} {level:?} {name}: {} channels off", bad.len());
                if !bad.is_empty() {
                    eprintln!("  decoded texels (half bits): {dec:?}");
                    failures.push(format!(
                        "{target:?} {level:?} {name}: {} channels off by more than 256 half codes, first {}",
                        bad.len(),
                        bad[0]
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The decoder writes `(u * 31) >> 6`, so the UF16 values that decode to half `h` are `[64h/31, 64(h+1)/31)`. The
/// kernel aims at `(h / 31) * 64`, the bottom edge, so a constant block's 16-bit code `round(64h/31)` decodes to
/// `h - 1` whenever the fraction is below 0.5 (survey section 1; h = 1 decodes to 0).
///
/// A constant neutral block (R = G = B = h) for every half `h` in `0x0001..=0x7BFF` stepping by 7, plus `0x7BFF`:
/// at every level on every path every decoded channel of every texel equals `h`. The failure message names `h`, the
/// level, the path and the decoded half, and the counts per level and path are printed.
#[test]
#[ignore = "RED for BC6H-Q5 (Note LT): passes once the Q5 rounding fix (patch 0007) lands"]
fn constant_blocks_decode_to_their_half_code() {
    let mut hs: Vec<u16> = (1..=0x7BFFu16).step_by(7).collect();
    hs.push(0x7BFF);
    let mut failures = Vec::new();
    for target in paths() {
        for level in RANDOM_STRIP_LEVELS {
            let (mut count, mut minus_one, mut first) = (0usize, 0usize, String::new());
            for chunk in hs.chunks(STRIP_BLOCKS) {
                let mut it = chunk.iter().copied();
                let px = strip(|| {
                    let h = it.next().unwrap_or(0x7BFF);
                    [[h; 3]; 16]
                });
                let mut out = vec![0u8; STRIP_BLOCKS * 16];
                bc6h_compress_rows_on(&px, (STRIP_BLOCKS * 4) as u32, 4, &mut out, level, target);
                for (i, &h) in chunk.iter().enumerate() {
                    let b: [u8; 16] = out[i * 16..i * 16 + 16].try_into().unwrap();
                    let dec = decode_halves(&b);
                    if let Some(&d) = dec.iter().flatten().find(|&&d| d != h) {
                        count += 1;
                        // Q5's premise: the failure is exactly one half code low in every channel that misses.
                        minus_one +=
                            usize::from(dec.iter().flatten().all(|&c| c == h || c == h - 1));
                        if first.is_empty() {
                            first = format!("h = {h:#06x} decodes to {d:#06x}");
                        }
                    }
                }
            }
            eprintln!(
                "{target:?} {level:?}: {count} of {} constant blocks do not decode to h, {minus_one} of them to exactly h - 1",
                hs.len()
            );
            if count > 0 {
                failures.push(format!(
                    "{target:?} {level:?}: {count} of {} blocks ({minus_one} exactly h - 1), first {first}",
                    hs.len()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
#[should_panic(expected = "multiple of 4")]
fn rejects_unaligned_width() {
    let px = vec![[0u16; 4]; 6 * 4];
    let mut out = [0u8; 32];
    bc6h_compress_rows(&px, 6, 4, &mut out, Bc6hLevel::Veryslow);
}
