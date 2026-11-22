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
        "a3815f73f1faa1991f89bd2d251f0c43d623f382c4e76aad0eb0ce405e001177",
    ),
    (
        Bc6hLevel::Slow,
        "4f5a52c6d602af099219a8a1461e96da250d3db076f6f1e5524baa2f1cf7a993",
    ),
    (
        Bc6hLevel::Veryslow,
        "27319321e56deb1d397198fe8dcafac45f704c3ac0f96b93f1246b22f00357cd",
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
        "1ac7db96d1a36c074cfa8e2339070880ee11ec9bed41c515ea39ef51990629d7",
    ),
    (
        Bc6hLevel::Slow,
        "f31409ce5530d9b9713b882207443b9fc1c5da53ad7b92f3508f25db605cdd17",
    ),
    (
        Bc6hLevel::Veryslow,
        "4daf6ac2b51011af4988808a54a67a7a1559f7de5e8928ec953f5187bf6ff7ff",
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
        "56266f916dbd71a7c5fe23639d29a5a9acef72d9dad5850061e32dba275ee9ef",
    ),
    (
        Bc6hLevel::Slow,
        "b84cd4a2bdbcfce6314d942a849de18c724f2e93eab4ab73e18aa4613ce1605d",
    ),
    (
        Bc6hLevel::Veryslow,
        "842b34375b4e0526d4387c6cab6fcd6515b716fb4a0e4c01ebfe9571debab3c7",
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
    "f382b224094a93524ef6594d352ef32a977b65bfbba46c619214797f426d6e3b",
    "a36d9aaa489ef3dc9d1bbbd5cb52aec4babca0f3584d8b8ad471d9a73e6ff74b",
    "62840b034aebaceeb13faf2765aac521873c5ea83f72baa5d68e5a3607725a9e",
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

/// Bounds (E_lo, E_hi) on the encoder's own error for one encoded block, read from the output bytes alone, in half-code
/// units (the encoder's UF16 error over the common factor (64/31)^2). The encoder scores the squared error of the
/// decoded UF16 interpolant u against `h_src * 64 / 31`, unweighted over R, G and B. The decoder writes
/// `h = (u * 31) >> 6`, so `u * 31 / 64` lies in [h, h + 1): with d = h - h_src a channel's error lies between d^2
/// and (d + 1)^2 (survey #3).
fn err_bounds(src: &[[u16; 4]; 16], dec: &[[u16; 3]; 16]) -> (f64, f64) {
    let (mut lo, mut hi) = (0i64, 0i64);
    for (s, d) in src.iter().zip(dec) {
        for c in 0..3 {
            let d = d[c] as i64 - s[c] as i64;
            let (a, b) = (d * d, (d + 1) * (d + 1));
            lo += a.min(b);
            hi += a.max(b);
        }
    }
    (lo as f64, hi as f64)
}

/// A deterministic HDR block from index `i` (half codes in 0x2000..=0x7000, alpha 1.0), for the refinement search.
/// Half the blocks are a gradient along a random direction across the block, with a random colour direction and up
/// to 64 half codes of noise; the other half draw each channel from a random range. One block in eight gets one
/// outlier texel. The widest channel spans 600..=12288 half codes, so the one-region modes in play are 10 to 12.
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
            let one = |l| err_bounds(&px, &decode_halves(&encode_block(&px, l, target)));
            let (lo1, _) = one(level);
            let (_, hi1) = one(Bc6hLevel::Veryfast);
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
            let bounds = |l| err_bounds(&px, &decode_halves(&encode_block(&px, l, target)));
            let (vf_lo, vf_hi) = bounds(Bc6hLevel::Veryfast);
            for level in [Bc6hLevel::Basic, Bc6hLevel::Veryslow] {
                let (lo, hi) = bounds(level);
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

#[test]
#[should_panic(expected = "multiple of 4")]
fn rejects_unaligned_width() {
    let px = vec![[0u16; 4]; 6 * 4];
    let mut out = [0u8; 32];
    bc6h_compress_rows(&px, 6, 4, &mut out, Bc6hLevel::Veryslow);
}
