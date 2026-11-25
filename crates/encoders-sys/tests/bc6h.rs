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

/// AVX2 path, recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-25, from the ISPC TC patch 0007 objects
/// (Note LT).
const RECORDED: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "5d73a5f78753bc50c8551232d9da261249a06fe8b08357f839938d72b5f8026f",
    ),
    (
        Bc6hLevel::Fast,
        "4c36698b3ec1d07dfa8ede975d1f09e53a1fe096097c4c754f6a2c7bced07be5",
    ),
    (
        Bc6hLevel::Basic,
        "5386955b2f795c5b5119d80627b4d538db40bfd61e5165840ec1a5f2fbd4ba17",
    ),
    (
        Bc6hLevel::Slow,
        "29a4e63f509f24c5b0dd3d7094e8a3e5eeec932ecbda51a33b52dffe395f9192",
    ),
    (
        Bc6hLevel::Veryslow,
        "c24af20ef81fbe0b091d7fbb6b0b53e1f1fc21964caed31bd75bc680a414c54e",
    ),
];

/// The SSE2 path, recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-25, from the ISPC TC patch 0007 objects
/// (Note LT). The SSE4 and AVX paths, dropped by Note JB, wrote the same bytes before patch 0004.
const RECORDED_PRE_AVX2: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "3bc40ae2f904a3328391a8ac61675e76b2a43872c896a5b7ef818c265bcf6699",
    ),
    (
        Bc6hLevel::Fast,
        "be1c8043d19f1df4a57012f932185f95be2f168b178dcec70a0397de821ee3a8",
    ),
    (
        Bc6hLevel::Basic,
        "1ec0f623b9694c3c21217a40222f7939cb5bc4169e09ebe5b70ac6cd46292d6a",
    ),
    (
        Bc6hLevel::Slow,
        "c69407eda0d1d1e7947b69cf54402a433770cfa51572db85768d8952d7f5bda2",
    ),
    (
        Bc6hLevel::Veryslow,
        "aec88b604925d456a2d795057a62f1fdc10405997838fc2ff7228e0ea5a4ee92",
    ),
];

/// AVX-512 path, recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-25, from the ISPC TC patch 0007 objects built
/// with ISPC 1.32.0 (Note LT). A test build encodes on it only with the AVX2 cap lifted (`SDC_TEST_AVX512=1`).
const RECORDED_AVX512: [(Bc6hLevel, &str); 5] = [
    (
        Bc6hLevel::Veryfast,
        "c6f7a89f24c95746c05d18f4ee52fa0b70951ceaa5e652bd63edc0613062083c",
    ),
    (
        Bc6hLevel::Fast,
        "b0b48d3ac80f31f91dacdffee6dde490a6d57d4ef68fa167f1a5dc367ab89b24",
    ),
    (
        Bc6hLevel::Basic,
        "73b31868a6a14c2c327ca2359faed67f5697cb3ec4b993d595413f5f6a3a9c56",
    ),
    (
        Bc6hLevel::Slow,
        "0044a11248c11f80533da77d4ed8be51f27db06b0f00714db77bd397a00ea334",
    ),
    (
        Bc6hLevel::Veryslow,
        "9cdb7c28f4c9b359238bad48959907280caab8cc66197333720175833f93df18",
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

/// The five shipped levels encode byte-identically to the recorded objects (the patch 0007 objects, Note LT), on each
/// path the run reads (SSE2 and AVX2; AVX-512 only with the cap lifted). The paths differ, so each has its own hashes.
#[test]
fn shipped_levels_are_byte_identical_to_the_recorded_objects() {
    check_pins(|t| recorded(t).to_vec());
}

/// The peak mode (`VeryslowPeak`) per path: SSE2, AVX2, AVX-512 (AVX-512 read only with the cap lifted).
/// Recorded on Windows on an AMD Ryzen 7 9800X3D, 2026-11-25, from the ISPC TC patch 0007 objects (Note LT).
const RECORDED_PEAK: [&str; 3] = [
    "d90d462f34cc4f1133ded65f3248319c6e71f6cb64d9971d0ac1f6fab9bce91c",
    "d91bb9afbdbd103d7ddbef9c80ec794aec9e95ad0579af94053a2c360601b8e9",
    "6564a066b322aa71c40f8e0ffc5e6268418902c1541e0df7c915f49ee2d192e6",
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

/// Modes 2-4 and 6-8 give the widest channel twice the window but were gated on the single span, so a block with R span
/// 6000 and G, B 2000 fell to mode 1 at Fast although mode 6's R window of 7372.6 fits (fable Q4, opus's trace).
#[test]
fn a_wide_channel_block_takes_the_doubled_window() {
    // R spans 2906 half codes (about 6000 UF16 units), G and B 968 (about 2000). The left two columns (one region of
    // BC6H partition 0) vary R alone; the right two vary G and B together with R fixed: two clusters, two directions.
    const BASE: u16 = 0x3C00;
    const R_SPAN: u32 = 2906;
    const GB_SPAN: u32 = 968;
    let step = |span: u32, i: usize| BASE + (span * i as u32 / 7) as u16;
    let one = f16::ONE.to_bits();
    let mut px = [[BASE, BASE, BASE, one]; 16];
    for row in 0..4usize {
        for col in 0..2usize {
            px[row * 4 + col][0] = step(R_SPAN, row * 2 + col);
        }
        for col in 2..4usize {
            let v = step(GB_SPAN, row * 2 + col - 2);
            px[row * 4 + col][1] = v;
            px[row * 4 + col][2] = v;
        }
    }
    // Fast and Basic are the levels that run the gate; both fell to mode 1 on this block.
    let modes: Vec<(IspcTarget, Bc6hLevel, u32, u32)> = paths()
        .flat_map(|target| [Bc6hLevel::Fast, Bc6hLevel::Basic].map(|level| (target, level)))
        .map(|(target, level)| {
            let b = encode_block(&px, level, target);
            let two = field(&b, 0, 2);
            let mode = if two < 2 { two } else { field(&b, 0, 5) };
            eprintln!("{target:?} {level:?} mode {mode}");
            (target, level, two, mode)
        })
        .collect();
    for (target, level, two, mode) in modes {
        assert_ne!(
            two, 1,
            "{target:?} {level:?}: mode 1 kept the block out of mode 6"
        );
        let family = matches!(mode, 18 | 22 | 26);
        if level == Bc6hLevel::Basic {
            assert!(
                family,
                "{target:?} {level:?}: not in the mode 6 family: {mode}"
            );
        } else {
            let one_region = matches!(mode, 3 | 7 | 11 | 15);
            assert!(
                one_region || family,
                "{target:?} {level:?}: a two-region mode outside the mode 6 family: {mode}"
            );
        }
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
/// decoded UF16 interpolant u against `(h_src + 0.5) * 64 / 31`, the centre of the source half code's decode range
/// (Q5), unweighted over R, G and B. The decoder writes `h = (u * 31) >> 6`, so `u * 31 / 64` lies in [h, h + 1):
/// with e = h - h_src a channel's term (u * 31 / 64 - (h_src + 0.5))^2 lies in [e - 0.5, e + 0.5) squared. The lower
/// bound is 0 when e = 0 (the interval holds 0), else the smaller of (e - 0.5)^2 and (e + 0.5)^2; the upper bound is
/// the larger (survey #3).
fn err_bounds(src: &[[u16; 4]; 16], dec: &[[u16; 3]; 16]) -> (f64, f64) {
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for (s, d) in src.iter().zip(dec) {
        for c in 0..3 {
            let e = d[c] as f64 - s[c] as f64;
            let (a, b) = ((e - 0.5) * (e - 0.5), (e + 0.5) * (e + 0.5));
            if e != 0.0 {
                lo += a.min(b);
            }
            hi += a.max(b);
        }
    }
    (lo, hi)
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

/// The UF16 value `h` (a half code) becomes in the kernel's `bc6h_setup` (the centre of its decode range, Q5), in f32
/// and in f64.
fn uf16_of_half(h: u16) -> (f32, f64) {
    (
        (h as f32 + 0.5) / 31.0 * 64.0,
        (h as f64 + 0.5) * 64.0 / 31.0,
    )
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
/// The inputs are half codes h (UF16 value v = (h + 0.5) * 64 / 31, the centre of h's decode range) where the shipped
/// rule and the nearest code differ, the nearest code is strict, and v is at least 4 UF16 units from both edges of that
/// mismatch window: G from nearest codes 8..=150 (the shipped rule picks one too high), B from 870..=1015 (one too
/// low). Four more blocks take the end codes: G with nearest code 0 (shipped 1) and B with nearest 1023 (shipped
/// 1022).
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
/// kernel aimed at `(h / 31) * 64`, the bottom edge, so a constant block's 16-bit code `round(64h/31)` decoded to
/// `h - 1` whenever the fraction was below 0.5 (survey section 1; h = 1 decoded to 0); it now aims at the centre,
/// `(h + 0.5) / 31 * 64` (Q5).
///
/// A constant neutral block (R = G = B = h) for every half `h` in `0x0001..=0x7BFF` stepping by 7, plus `0x7BFF`:
/// at every level on every path every decoded channel of every texel equals `h`. The failure message names `h`, the
/// level, the path and the decoded half, and the counts per level and path are printed.
#[test]
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

/// The 32 two-region partitions as 16-bit texel masks (bit t set: texel t is in the second region), the high halves of
/// the kernel's `pattern_mask_table`.
const TWO_REGION_MASKS: [u16; 32] = [
    0xCCCC, 0x8888, 0xEEEE, 0xECC8, 0xC880, 0xFEEC, 0xFEC8, 0xEC80, 0xC800, 0xFFEC, 0xFE80, 0xE800,
    0xFFE8, 0xFF00, 0xFFF0, 0xF000, 0xF710, 0x008E, 0x7100, 0x08CE, 0x008C, 0x7310, 0x3100, 0x8CCE,
    0x088C, 0x3110, 0x6666, 0x366C, 0x17E8, 0x0FF0, 0x718E, 0x399C,
];

/// A two-cluster block from index `i` (half codes in 0x0400..=0x7000, alpha 1.0): one of the 32 two-region partitions,
/// each region its own colour gradient (a random base, a random direction across the block, each channel's amplitude
/// up to 4096 half codes either way) plus up to 32 half codes of noise. Integer arithmetic only, so the blocks are the
/// same on every machine. Tasks 15, 17 and 24 search these.
fn two_cluster(i: u32) -> [[u16; 4]; 16] {
    const LO: i64 = 0x0400;
    const HI: i64 = 0x7000;
    let mut r = Lcg(i.wrapping_mul(0x85EB_CA6B) ^ 0x6A09_E667);
    r.next();
    let mask = TWO_REGION_MASKS[(r.next() % 32) as usize];
    let mut planes = [[0u16; 16]; 3];
    for region in 0..2u16 {
        let in_region = |t: &usize| (mask >> t) & 1 == region;
        let (mut dx, dy) = (r.range(0, 6) as i64 - 3, r.range(0, 6) as i64 - 3);
        if dx == 0 && dy == 0 {
            dx = 1;
        }
        let pos = |t: usize| (t % 4) as i64 * dx + (t / 4) as i64 * dy;
        let pmin = (0..16).filter(in_region).map(pos).min().unwrap();
        let pmax = (0..16).filter(in_region).map(pos).max().unwrap();
        let noise = r.range(0, 32) as i64;
        for plane in &mut planes {
            let amp = r.range(0, 8192) as i64 - 4096;
            // The base keeps base + amp * t inside LO..=HI for every t in 0..=1.
            let (lo, hi) = (LO - amp.min(0), HI - amp.max(0));
            let base = r.range(lo as u32, hi as u32) as i64;
            for t in (0..16).filter(in_region) {
                let v = base
                    + amp * (pos(t) - pmin) / (pmax - pmin).max(1)
                    + r.range(0, noise as u32) as i64
                    - noise / 2;
                plane[t] = v.clamp(LO, HI) as u16;
            }
        }
    }
    texels_of(&planes)
}

/// Texels (alpha 1.0) from three channel planes.
fn texels_of(planes: &[[u16; 16]; 3]) -> [[u16; 4]; 16] {
    std::array::from_fn(|t| [planes[0][t], planes[1][t], planes[2][t], f16::ONE.to_bits()])
}

/// Task 15's seeded blocks: odd `i` is `two_cluster(i / 2)`; even `i` is a one-region ramp with clustered texels, like
/// the audit's grey block (four texels at half code 46, eight at 1038, four at 15918): 1 to 6 texels at the low end, 1
/// to 6 at the high end and the rest 2/64 to 62/64 of the way along, in a shuffled texel order, with up to 16 half
/// codes of noise. One ramp in two is grey (R = G = B); the others give each channel its own ends (low end 0..=0x2000,
/// span 512..=0x4000). Integer arithmetic only.
fn refit_blocks(i: u32) -> [[u16; 4]; 16] {
    if i % 2 == 1 {
        return two_cluster(i / 2);
    }
    let mut r = Lcg(i.wrapping_mul(0xC2B2_AE35) ^ 0x3C6E_F372);
    r.next();
    let (n_lo, n_hi) = (r.range(1, 6) as usize, r.range(1, 6) as usize);
    let along = r.range(2, 62) as i64;
    let grey = r.next().is_multiple_of(2);
    let noise = r.range(0, 16) as i64;
    let mut order: [usize; 16] = std::array::from_fn(|t| t);
    for t in (1..16).rev() {
        order.swap(t, r.next() as usize % (t + 1));
    }
    let mut planes = [[0u16; 16]; 3];
    for plane in planes.iter_mut().take(if grey { 1 } else { 3 }) {
        let lo = r.range(0, 0x2000) as i64;
        let hi = lo + r.range(512, 0x4000) as i64;
        for (rank, &t) in order.iter().enumerate() {
            let v = if rank < n_lo {
                lo
            } else if rank < n_lo + n_hi {
                hi
            } else {
                lo + (hi - lo) * along / 64
            };
            let v = v + r.range(0, noise as u32) as i64 - noise / 2;
            plane[t] = v.clamp(0, MAX_UF16 as i64) as u16;
        }
    }
    if grey {
        planes[1] = planes[0];
        planes[2] = planes[0];
    }
    texels_of(&planes)
}

/// Writes `<path> <level> <i> <hex bytes> <E_lo> <E_hi>` (`err_bounds` of the output) for `blocks(i)`, `i in 0..end`,
/// encoded one block at a time (the witness tests' call) at each of `levels` on every path in `paths()`, to the file
/// `SDC_BC6H_WITNESS_OUT` names; with the variable unset it says so and writes nothing. The first line,
/// `# commit <git describe --dirty> input <name> 0..<end> levels <levels>`, names the objects and the input set, and
/// each path and level's lines are appended as soon as they are done, so an interrupted run keeps them.
fn record_witness_search(
    name: &str,
    blocks: fn(u32) -> [[u16; 4]; 16],
    end: u32,
    levels: &[Bc6hLevel],
) {
    use std::fmt::Write as _;
    use std::io::Write as _;
    let Some(path) = std::env::var_os("SDC_BC6H_WITNESS_OUT") else {
        eprintln!("skipped: SDC_BC6H_WITNESS_OUT is not set");
        return;
    };
    let mut file = std::fs::File::create(&path).unwrap();
    writeln!(
        file,
        "# commit {} input {name} 0..{end} levels {levels:?}",
        git_describe()
    )
    .unwrap();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8)) as u32;
    let start = std::time::Instant::now();
    for target in paths() {
        for &level in levels {
            let chunks: Vec<String> = std::thread::scope(|s| {
                let jobs: Vec<_> = (0..threads)
                    .map(|t| {
                        s.spawn(move || {
                            let mut part = String::new();
                            for i in (t..end).step_by(threads as usize) {
                                let px = blocks(i);
                                let b = encode_block(&px, level, target);
                                let (lo, hi) = err_bounds(&px, &decode_halves(&b));
                                writeln!(
                                    part,
                                    "{target:?} {level:?} {i} {} {lo} {hi}",
                                    hex::encode(b)
                                )
                                .unwrap();
                            }
                            part
                        })
                    })
                    .collect();
                jobs.into_iter().map(|j| j.join().unwrap()).collect()
            });
            file.write_all(chunks.concat().as_bytes()).unwrap();
            file.flush().unwrap();
            eprintln!(
                "{target:?} {level:?}: {end} blocks written, {:.1} s since the start",
                start.elapsed().as_secs_f64()
            );
        }
    }
}

/// `git describe --always --dirty --abbrev=8` of the crate's checkout (the objects a search ran on), or "unknown".
fn git_describe() -> String {
    std::process::Command::new("git")
        .args(["describe", "--always", "--dirty", "--abbrev=8"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map_or("unknown".to_string(), |o| {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        })
}

/// The witness tests' check: for each witness `(i, level, bytes before the fix on SSE2, AVX2 and AVX-512)` and on every
/// path in `paths()`, the encode of `blocks(i)` is certainly better than the recorded bytes (`err_bounds`: E_hi now <
/// E_lo before). Before the fix the encode is those bytes, so the check fails there.
fn assert_witnesses_certainly_better(
    witnesses: &[(u32, Bc6hLevel, [[u8; 16]; 3])],
    blocks: fn(u32) -> [[u16; 4]; 16],
) {
    let mut failures = Vec::new();
    for target in paths() {
        let slot = IspcTarget::ALL.iter().position(|t| *t == target).unwrap();
        for (i, level, before) in witnesses {
            let px = blocks(*i);
            let now = encode_block(&px, *level, target);
            let (lo, hi) = err_bounds(&px, &decode_halves(&now));
            let (before_lo, before_hi) = err_bounds(&px, &decode_halves(&before[slot]));
            if hi >= before_lo {
                failures.push(format!(
                    "witness {i} {target:?} {level:?}: E_hi {hi} >= E_lo {before_lo} before \
                     (now [{lo}, {hi}], before [{before_lo}, {before_hi}], bytes {})",
                    if now == before[slot] {
                        "unchanged"
                    } else {
                        "changed"
                    }
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} witnesses not certainly better:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Records Task 15's search (`refit_blocks`, `i in 0..1 << 16`, Fast, Basic and Veryslow) for `REFIT_WITNESSES`.
#[test]
#[ignore = "record refit witnesses: run on the objects before the change, then after"]
fn search_refit_witnesses() {
    let levels = [Bc6hLevel::Fast, Bc6hLevel::Basic, Bc6hLevel::Veryslow];
    record_witness_search("refit_blocks", refit_blocks, 1 << 16, &levels);
}

/// Records Task 17's search (`two_cluster`, `i in 0..1 << 16`, Basic and Veryslow) for `TWO_REGION_REFINE_WITNESSES`.
#[test]
#[ignore = "record two-region refine witnesses: run on the objects before the change, then after"]
fn search_two_region_refine_witnesses() {
    let levels = [Bc6hLevel::Basic, Bc6hLevel::Veryslow];
    record_witness_search("two_cluster", two_cluster, 1 << 16, &levels);
}

/// Witnesses from `search_refit_witnesses`: (`refit_blocks` index, level, the bytes the objects before the refit fix
/// wrote on SSE2, AVX2 and AVX-512, in `IspcTarget::ALL` order). Recorded on Windows on an AMD Ryzen 7 9800X3D,
/// 2026-11-25. Before the fix 58198, 28394 and 48010 wrote a one-region mode (11) and the other five a two-region mode;
/// after it 58198 and 28394 stay one-region (the one-region refit) and 48010 writes a two-region mode, so both refits
/// are covered, at Fast, Basic and Veryslow.
#[rustfmt::skip]
const REFIT_WITNESSES: &[(u32, Bc6hLevel, [[u8; 16]; 3])] = &[
    (40309, Bc6hLevel::Basic, [
        [0x15, 0xb6, 0xb2, 0x20, 0x8e, 0x4f, 0x1e, 0x7b, 0x58, 0x32, 0xf1, 0x5f, 0x6d, 0x25, 0x09, 0x00],
        [0x15, 0xb6, 0xb2, 0x20, 0x8e, 0x4f, 0x1e, 0x7b, 0x58, 0x32, 0xf1, 0x5f, 0x6d, 0x25, 0x09, 0x00],
        [0x15, 0xb6, 0xb2, 0x20, 0x8e, 0x4f, 0x1e, 0x7b, 0x58, 0x32, 0xf1, 0x5f, 0x6d, 0x25, 0x09, 0x00],
    ]),
    (46762, Bc6hLevel::Basic, [
        [0xfe, 0xf4, 0x93, 0xcf, 0x4a, 0x34, 0xb9, 0x44, 0xb5, 0x33, 0xbc, 0xfd, 0xdf, 0xb1, 0x63, 0x3b],
        [0xfe, 0xf4, 0x93, 0xcf, 0x4a, 0x34, 0xb9, 0x44, 0xb5, 0x33, 0xbc, 0xfd, 0xdf, 0xb1, 0x63, 0x3b],
        [0xfe, 0xf4, 0x93, 0xcf, 0x4a, 0x34, 0xb9, 0x44, 0xb5, 0x33, 0xbc, 0xfd, 0xdf, 0xb1, 0x63, 0x3b],
    ]),
    (58198, Bc6hLevel::Basic, [
        [0xe3, 0xbc, 0xf3, 0xce, 0xa3, 0x82, 0x0a, 0x2a, 0x7e, 0x07, 0x77, 0x77, 0xe7, 0x77, 0x77, 0x07],
        [0xe3, 0xbc, 0xf3, 0xce, 0xa3, 0x82, 0x0a, 0x2a, 0x7e, 0x07, 0x77, 0x77, 0xe7, 0x77, 0x77, 0x07],
        [0xe3, 0xbc, 0xf3, 0xce, 0xa3, 0x82, 0x0a, 0x2a, 0x7e, 0x07, 0x77, 0x77, 0xe7, 0x77, 0x77, 0x07],
    ]),
    (4339, Bc6hLevel::Fast, [
        [0x5e, 0xa0, 0xf7, 0xaa, 0x53, 0x90, 0xae, 0x07, 0x69, 0x96, 0xdd, 0x17, 0xef, 0xde, 0xa9, 0x13],
        [0x5e, 0xa0, 0xf7, 0xaa, 0x53, 0x90, 0xae, 0x07, 0x69, 0x96, 0xdd, 0x17, 0xef, 0xde, 0xa9, 0x13],
        [0x5e, 0xa0, 0xf7, 0xaa, 0x53, 0x90, 0xae, 0x07, 0x69, 0x96, 0xdd, 0x17, 0xef, 0xde, 0xa9, 0x13],
    ]),
    (49260, Bc6hLevel::Fast, [
        [0xd1, 0x05, 0x97, 0x5c, 0xbb, 0xe1, 0xe6, 0x1b, 0x00, 0x56, 0xf0, 0xfd, 0xc3, 0x3e, 0xfc, 0xdb],
        [0xd1, 0x05, 0x97, 0x5c, 0xbb, 0xe1, 0xe6, 0x1b, 0x00, 0x56, 0xf0, 0xfd, 0xc3, 0x3e, 0xfc, 0xdb],
        [0xd1, 0x05, 0x97, 0x5c, 0xbb, 0xe1, 0xe6, 0x1b, 0x00, 0x56, 0xf0, 0xfd, 0xc3, 0x3e, 0xfc, 0xdb],
    ]),
    (8625, Bc6hLevel::Veryslow, [
        [0xde, 0x12, 0x81, 0x3a, 0x89, 0x08, 0xc9, 0xee, 0x14, 0x08, 0x21, 0x5a, 0xeb, 0x47, 0x9f, 0x86],
        [0xde, 0x12, 0x81, 0x3a, 0x89, 0x08, 0xc9, 0xee, 0x14, 0x08, 0x21, 0x5a, 0xeb, 0x47, 0x9f, 0x86],
        [0xde, 0x12, 0x81, 0x3a, 0x89, 0x08, 0xc9, 0xee, 0x14, 0x08, 0x21, 0x5a, 0xeb, 0x47, 0x9f, 0x86],
    ]),
    (28394, Bc6hLevel::Veryslow, [
        [0xe3, 0xcd, 0x37, 0xdf, 0x4c, 0x27, 0x9d, 0x74, 0x70, 0x70, 0x07, 0x77, 0x7e, 0x07, 0x07, 0x70],
        [0xe3, 0xcd, 0x37, 0xdf, 0x4c, 0x27, 0x9d, 0x74, 0x70, 0x70, 0x07, 0x77, 0x7e, 0x07, 0x07, 0x70],
        [0xe3, 0xcd, 0x37, 0xdf, 0x4c, 0x27, 0x9d, 0x74, 0x70, 0x70, 0x07, 0x77, 0x7e, 0x07, 0x07, 0x70],
    ]),
    (48010, Bc6hLevel::Veryslow, [
        [0xc3, 0x1a, 0x6b, 0xac, 0x11, 0x54, 0x50, 0x41, 0xfd, 0xf6, 0x06, 0x6f, 0xf6, 0x60, 0x60, 0x06],
        [0xc3, 0x1a, 0x6b, 0xac, 0x11, 0x54, 0x50, 0x41, 0xfd, 0xf6, 0x06, 0x6f, 0xf6, 0x60, 0x60, 0x06],
        [0xc3, 0x1a, 0x6b, 0xac, 0x11, 0x54, 0x50, 0x41, 0xfd, 0xf6, 0x06, 0x6f, 0xf6, 0x60, 0x60, 0x06],
    ]),
];

/// astra #4 (Note LT): the least-squares refit fits index q at the decoder's weight `table[q] / 64`, not at
/// `q / (levels - 1)`. On each witness the encode is certainly better than the bytes the objects before the fix wrote
/// (`err_bounds`: E_hi now < E_lo before), on every path in `paths()`. Before the fix the encode is those bytes, so
/// the test fails there. Skipped on an Intel CPU, like the pins: the bytes are the Ryzen's (Note JC).
#[test]
fn refit_uses_the_decoder_weights() {
    if encoders_sys::skip_byte_pins() {
        return;
    }
    assert_witnesses_certainly_better(REFIT_WITNESSES, refit_blocks);
}

/// Witnesses from `search_two_region_refine_witnesses`: (`two_cluster` index, level, the bytes the objects before the
/// Q1 fix wrote on SSE2, AVX2 and AVX-512, in `IspcTarget::ALL` order). Recorded on Windows on an AMD Ryzen 7 9800X3D,
/// 2026-11-25. All eight write a two-region mode before and after; the three paths agree on them.
#[rustfmt::skip]
const TWO_REGION_REFINE_WITNESSES: &[(u32, Bc6hLevel, [[u8; 16]; 3])] = &[
    (29886, Bc6hLevel::Basic, [
        [0x7e, 0x6e, 0x22, 0xe2, 0xb5, 0xbb, 0x80, 0x5a, 0xcb, 0xcf, 0xa2, 0x35, 0x63, 0x59, 0xdd, 0xf7],
        [0x7e, 0x6e, 0x22, 0xe2, 0xb5, 0xbb, 0x80, 0x5a, 0xcb, 0xcf, 0xa2, 0x35, 0x63, 0x59, 0xdd, 0xf7],
        [0x7e, 0x6e, 0x22, 0xe2, 0xb5, 0xbb, 0x80, 0x5a, 0xcb, 0xcf, 0xa2, 0x35, 0x63, 0x59, 0xdd, 0xf7],
    ]),
    (32448, Bc6hLevel::Basic, [
        [0x9e, 0x4c, 0x9a, 0x35, 0xec, 0xc4, 0x95, 0xcb, 0x5c, 0x56, 0xe0, 0x52, 0xa5, 0x4a, 0xe5, 0x0a],
        [0x9e, 0x4c, 0x9a, 0x35, 0xec, 0xc4, 0x95, 0xcb, 0x5c, 0x56, 0xe0, 0x52, 0xa5, 0x4a, 0xe5, 0x0a],
        [0x9e, 0x4c, 0x9a, 0x35, 0xec, 0xc4, 0x95, 0xcb, 0x5c, 0x56, 0xe0, 0x52, 0xa5, 0x4a, 0xe5, 0x0a],
    ]),
    (57014, Bc6hLevel::Basic, [
        [0x7e, 0x8c, 0xd0, 0x2d, 0x3a, 0xf1, 0xe3, 0x89, 0x23, 0xcb, 0x22, 0x36, 0xa3, 0x59, 0x8d, 0xf7],
        [0x7e, 0x8c, 0xd0, 0x2d, 0x3a, 0xf1, 0xe3, 0x89, 0x23, 0xcb, 0x22, 0x36, 0xa3, 0x59, 0x8d, 0xf7],
        [0x7e, 0x8c, 0xd0, 0x2d, 0x3a, 0xf1, 0xe3, 0x89, 0x23, 0xcb, 0x22, 0x36, 0xa3, 0x59, 0x8d, 0xf7],
    ]),
    (62547, Bc6hLevel::Basic, [
        [0x7e, 0x9c, 0x52, 0x33, 0xde, 0x7e, 0xec, 0xaf, 0x4d, 0x10, 0xc9, 0x5e, 0xa3, 0x23, 0x1e, 0xc2],
        [0x7e, 0x9c, 0x52, 0x33, 0xde, 0x7e, 0xec, 0xaf, 0x4d, 0x10, 0xc9, 0x5e, 0xa3, 0x23, 0x1e, 0xc2],
        [0x7e, 0x9c, 0x52, 0x33, 0xde, 0x7e, 0xec, 0xaf, 0x4d, 0x10, 0xc9, 0x5e, 0xa3, 0x23, 0x1e, 0xc2],
    ]),
    (63204, Bc6hLevel::Basic, [
        [0x9e, 0x34, 0x4a, 0x26, 0x2b, 0x6d, 0xc2, 0xcd, 0x53, 0x14, 0xb2, 0xfe, 0x58, 0xcb, 0x8d, 0x13],
        [0x9e, 0x34, 0x4a, 0x26, 0x2b, 0x6d, 0xc2, 0xcd, 0x53, 0x14, 0xb2, 0xfe, 0x58, 0xcb, 0x8d, 0x13],
        [0x9e, 0x34, 0x4a, 0x26, 0x2b, 0x6d, 0xc2, 0xcd, 0x53, 0x14, 0xb2, 0xfe, 0x58, 0xcb, 0x8d, 0x13],
    ]),
    (64598, Bc6hLevel::Basic, [
        [0x7e, 0x64, 0xda, 0x38, 0x36, 0x95, 0x46, 0x2c, 0x58, 0x98, 0x92, 0x11, 0xed, 0x1a, 0x3b, 0xf6],
        [0x7e, 0x64, 0xda, 0x38, 0x36, 0x95, 0x46, 0x2c, 0x58, 0x98, 0x92, 0x11, 0xed, 0x1a, 0x3b, 0xf6],
        [0x7e, 0x64, 0xda, 0x38, 0x36, 0x95, 0x46, 0x2c, 0x58, 0x98, 0x92, 0x11, 0xed, 0x1a, 0x3b, 0xf6],
    ]),
    (46831, Bc6hLevel::Veryslow, [
        [0x3e, 0x7b, 0xc3, 0x3d, 0x96, 0x6e, 0x19, 0xb1, 0x5e, 0x98, 0xc1, 0xa3, 0x53, 0xcc, 0xe9, 0xe6],
        [0x3e, 0x7b, 0xc3, 0x3d, 0x96, 0x6e, 0x19, 0xb1, 0x5e, 0x98, 0xc1, 0xa3, 0x53, 0xcc, 0xe9, 0xe6],
        [0x3e, 0x7b, 0xc3, 0x3d, 0x96, 0x6e, 0x19, 0xb1, 0x5e, 0x98, 0xc1, 0xa3, 0x53, 0xcc, 0xe9, 0xe6],
    ]),
    (49584, Bc6hLevel::Veryslow, [
        [0x1e, 0x33, 0x51, 0x48, 0xfe, 0xf0, 0x6c, 0xd5, 0x95, 0x46, 0x92, 0x31, 0x63, 0x67, 0xfd, 0xe1],
        [0x1e, 0x33, 0x51, 0x48, 0xfe, 0xf0, 0x6c, 0xd5, 0x95, 0x46, 0x92, 0x31, 0x63, 0x67, 0xfd, 0xe1],
        [0x1e, 0x33, 0x51, 0x48, 0xfe, 0xf0, 0x6c, 0xd5, 0x95, 0x46, 0x92, 0x31, 0x63, 0x67, 0xfd, 0xe1],
    ]),
];

/// Q1 (Note LT): the two-region refine refits each pass from the previous pass's indices, as the one-region refine
/// does, and keeps the best pass. On each witness the encode is certainly better than the bytes the objects before the
/// fix wrote (`err_bounds`: E_hi now < E_lo before), on every path in `paths()`. Before the fix the encode is those
/// bytes, so the test fails there. Skipped on an Intel CPU, like the pins: the bytes are the Ryzen's (Note JC).
#[test]
fn two_region_refine_keeps_its_best_pass() {
    if encoders_sys::skip_byte_pins() {
        return;
    }
    assert_witnesses_certainly_better(TWO_REGION_REFINE_WITNESSES, two_cluster);
}

/// Q2's two block generators (`collapsed_block`); the witnesses name theirs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CollapseKind {
    OneRegionRamp,
    TwoRegion,
}

/// Q2's seeded blocks from index `i` (alpha 1.0), with the texels of the region that must not decode flat.
///
/// `OneRegionRamp`: one smooth ramp along a random direction across the block in all three channels, each channel
/// spanning 4 to 7 half codes in total (8.3 to 14.5 UF16 units: over mode 13's 6-unit gate and under mode 12's 16-unit
/// code cell), every channel's UF16 range `(h + 0.5) * 64 / 31` inside one mode-12 cell `(16q, 16q + 16]` with a
/// quarter unit to spare at each edge. That is the range `ep_quant_bc6h` maps to code q: `ceil(e / 16) - 1`, so a tie at
/// 16q + 16 keeps q and 16q itself goes to q - 1. Odd `i` has one channel falling where the other two rise. The region is the
/// whole block.
///
/// `TwoRegion` (fable's scenario A): one of the 32 two-region partitions; one region is dim (half codes 0x0400 to
/// 0x2400 plus a ramp of 8 to 120 half codes per channel), the other bright (1000 to 0x5000 half codes above the dim
/// base, plus a ramp of up to 256). The region is the dim one.
///
/// Integer arithmetic only, so the blocks are the same on every machine.
fn collapsed_block(i: u32, kind: CollapseKind) -> ([[u16; 4]; 16], u16) {
    // A ramp's position along (dx, dy), for a random direction.
    fn direction(r: &mut Lcg) -> impl Fn(usize) -> i64 {
        let (mut dx, dy) = (r.range(0, 6) as i64 - 3, r.range(0, 6) as i64 - 3);
        if dx == 0 && dy == 0 {
            dx = 1;
        }
        move |t| (t % 4) as i64 * dx + (t / 4) as i64 * dy
    }
    let salt = match kind {
        CollapseKind::OneRegionRamp => 0x1656_67B1,
        CollapseKind::TwoRegion => 0x7FEB_352D,
    };
    let mut r = Lcg(i.wrapping_mul(0x27D4_EB2F) ^ salt);
    r.next();
    let mut planes = [[0u16; 16]; 3];
    match kind {
        CollapseKind::OneRegionRamp => {
            // 8/31 UF16 units: u(h) = (64h + 32) / 31 must lie in (16q + 8/31, 16q + 16 - 8/31].
            const MARGIN: i64 = 8;
            let pos = direction(&mut r);
            let pmin = (0..16).map(&pos).min().unwrap();
            let pmax = (0..16).map(&pos).max().unwrap();
            let falling = (i % 2 == 1).then(|| (r.next() % 3) as usize);
            for (c, plane) in planes.iter_mut().enumerate() {
                let s = r.range(4, 7) as i64;
                let h = loop {
                    let q = r.range(0x80, 0xE00) as i64;
                    let lo = (496 * q + MARGIN - 32).div_euclid(64) + 1;
                    let hi = (496 * q + 464 - MARGIN).div_euclid(64) - s;
                    if lo <= hi {
                        break r.range(lo as u32, hi as u32) as i64;
                    }
                };
                for (t, v) in plane.iter_mut().enumerate() {
                    let step = s * (pos(t) - pmin) / (pmax - pmin);
                    *v = if falling == Some(c) {
                        h + s - step
                    } else {
                        h + step
                    } as u16;
                }
            }
            (texels_of(&planes), 0xFFFF)
        }
        CollapseKind::TwoRegion => {
            let mask = TWO_REGION_MASKS[(r.next() % 32) as usize];
            let dim_side = (r.next() % 2) as u16;
            let dim_base: [i64; 3] = std::array::from_fn(|_| r.range(0x0400, 0x2400) as i64);
            for region in 0..2u16 {
                let in_region = |t: &usize| (mask >> t) & 1 == region;
                let pos = direction(&mut r);
                let pmin = (0..16).filter(in_region).map(&pos).min().unwrap();
                let pmax = (0..16).filter(in_region).map(&pos).max().unwrap();
                for (c, plane) in planes.iter_mut().enumerate() {
                    let (base, s) = if region == dim_side {
                        (dim_base[c], r.range(8, 120) as i64)
                    } else {
                        (
                            dim_base[c] + r.range(1000, 0x5000) as i64,
                            r.range(0, 256) as i64,
                        )
                    };
                    for t in (0..16).filter(in_region) {
                        plane[t] = (base + s * (pos(t) - pmin) / (pmax - pmin).max(1)) as u16;
                    }
                }
            }
            let dim = if dim_side == 1 { mask } else { !mask };
            (texels_of(&planes), dim)
        }
    }
}

/// The texels of `region` (bit t set: texel t).
fn region_texels(region: u16) -> Vec<usize> {
    (0..16).filter(|t| (region >> t) & 1 == 1).collect()
}

/// Whether `v` holds at least two distinct values over the texels of `region`.
fn varies(region: u16, v: impl Fn(usize) -> u16) -> bool {
    let texels = region_texels(region);
    texels.iter().any(|&t| v(t) != v(texels[0]))
}

/// Per channel, whether the source varies over the texels of `region`.
fn source_varies(px: &[[u16; 4]; 16], region: u16) -> [bool; 3] {
    std::array::from_fn(|c| varies(region, |t| px[t][c]))
}

/// The covariance sum of channels `a` and `b` of `v` over the texels of `region`: Σ (v_a - mean_a)(v_b - mean_b).
fn region_covariance(v: impl Fn(usize, usize) -> u16, region: u16, a: usize, b: usize) -> f64 {
    let texels = region_texels(region);
    let mean = |c: usize| texels.iter().map(|&t| v(t, c) as f64).sum::<f64>() / texels.len() as f64;
    let (ma, mb) = (mean(a), mean(b));
    texels
        .iter()
        .map(|&t| (v(t, a) as f64 - ma) * (v(t, b) as f64 - mb))
        .sum()
}

/// Q2's levels: Veryfast and Fast write mode 12 for a one-region ramp; Basic and Veryslow search more modes.
const COLLAPSE_LEVELS: [Bc6hLevel; 4] = [
    Bc6hLevel::Veryfast,
    Bc6hLevel::Fast,
    Bc6hLevel::Basic,
    Bc6hLevel::Veryslow,
];

/// For each generator and level in `COLLAPSE_LEVELS`, finds the `collapsed_block` indices in `0..1 << 16` whose region
/// varies in all three source channels and decodes flat in all three (one value per channel over the region) on every
/// path in `paths()` (run it with `SDC_TEST_AVX512=1` so AVX-512 is included), writes every hit as `<kind> <level> <i>`
/// to the file `SDC_BC6H_WITNESS_OUT` names (the first line names the objects), and prints the count and the first
/// four hits with and without a falling channel (odd and even `i`). With the variable unset it says so and returns.
#[test]
#[ignore = "search for collapsed-region witnesses: run on the objects before the Q2 fix, with SDC_TEST_AVX512=1"]
fn search_collapsed_witnesses() {
    use std::io::Write as _;
    const END: u32 = 1 << 16;
    let Some(path) = std::env::var_os("SDC_BC6H_WITNESS_OUT") else {
        eprintln!("skipped: SDC_BC6H_WITNESS_OUT is not set");
        return;
    };
    let targets: Vec<IspcTarget> = paths().collect();
    let mut file = std::fs::File::create(&path).unwrap();
    writeln!(
        file,
        "# commit {} input collapsed_block 0..{END} paths {targets:?}",
        git_describe()
    )
    .unwrap();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8)) as u32;
    for kind in [CollapseKind::OneRegionRamp, CollapseKind::TwoRegion] {
        for level in COLLAPSE_LEVELS {
            let mut hits: Vec<u32> = std::thread::scope(|s| {
                let jobs: Vec<_> = (0..threads)
                    .map(|t| {
                        let targets = &targets;
                        s.spawn(move || {
                            (t..END)
                                .step_by(threads as usize)
                                .filter(|&i| {
                                    let (px, region) = collapsed_block(i, kind);
                                    source_varies(&px, region).iter().all(|&v| v)
                                        && targets.iter().all(|&target| {
                                            let dec =
                                                decode_halves(&encode_block(&px, level, target));
                                            (0..3).all(|c| !varies(region, |t| dec[t][c]))
                                        })
                                })
                                .collect::<Vec<u32>>()
                        })
                    })
                    .collect();
                jobs.into_iter().flat_map(|j| j.join().unwrap()).collect()
            });
            hits.sort_unstable();
            for i in &hits {
                writeln!(file, "{kind:?} {level:?} {i}").unwrap();
            }
            file.flush().unwrap();
            let first = |odd: bool| -> Vec<u32> {
                hits.iter()
                    .copied()
                    .filter(|i| (i % 2 == 1) == odd)
                    .take(4)
                    .collect()
            };
            println!(
                "{kind:?} {level:?}: {} of {END} decode flat on {targets:?}; rising {:?}, falling {:?}",
                hits.len(),
                first(false),
                first(true)
            );
        }
    }
}

/// Witnesses from `search_collapsed_witnesses`: (`collapsed_block` index, level, generator). On the objects before the
/// Q2 fix each decoded flat on SSE2, AVX2 and AVX-512 (found on Windows on an AMD Ryzen 7 9800X3D, 2026-11-25). Per
/// level the first four hits with a falling channel (odd) and without (even); Veryslow had hits with a falling channel
/// only (185 of 65,536). The two-region hit at Fast is the first one that decodes non-flat after the fix on every path
/// (the first hit, 0, still decodes flat there).
const COLLAPSED_WITNESSES: &[(u32, Bc6hLevel, CollapseKind)] = &[
    (1, Bc6hLevel::Veryfast, CollapseKind::OneRegionRamp),
    (2, Bc6hLevel::Veryfast, CollapseKind::OneRegionRamp),
    (3, Bc6hLevel::Veryfast, CollapseKind::OneRegionRamp),
    (5, Bc6hLevel::Veryfast, CollapseKind::OneRegionRamp),
    (7, Bc6hLevel::Veryfast, CollapseKind::OneRegionRamp),
    (12, Bc6hLevel::Veryfast, CollapseKind::OneRegionRamp),
    (18, Bc6hLevel::Veryfast, CollapseKind::OneRegionRamp),
    (20, Bc6hLevel::Veryfast, CollapseKind::OneRegionRamp),
    (1, Bc6hLevel::Fast, CollapseKind::OneRegionRamp),
    (2, Bc6hLevel::Fast, CollapseKind::OneRegionRamp),
    (3, Bc6hLevel::Fast, CollapseKind::OneRegionRamp),
    (5, Bc6hLevel::Fast, CollapseKind::OneRegionRamp),
    (7, Bc6hLevel::Fast, CollapseKind::OneRegionRamp),
    (12, Bc6hLevel::Fast, CollapseKind::OneRegionRamp),
    (18, Bc6hLevel::Fast, CollapseKind::OneRegionRamp),
    (20, Bc6hLevel::Fast, CollapseKind::OneRegionRamp),
    (4, Bc6hLevel::Fast, CollapseKind::TwoRegion),
    (1, Bc6hLevel::Basic, CollapseKind::OneRegionRamp),
    (2, Bc6hLevel::Basic, CollapseKind::OneRegionRamp),
    (3, Bc6hLevel::Basic, CollapseKind::OneRegionRamp),
    (5, Bc6hLevel::Basic, CollapseKind::OneRegionRamp),
    (7, Bc6hLevel::Basic, CollapseKind::OneRegionRamp),
    (12, Bc6hLevel::Basic, CollapseKind::OneRegionRamp),
    (18, Bc6hLevel::Basic, CollapseKind::OneRegionRamp),
    (20, Bc6hLevel::Basic, CollapseKind::OneRegionRamp),
    (621, Bc6hLevel::Veryslow, CollapseKind::OneRegionRamp),
    (2345, Bc6hLevel::Veryslow, CollapseKind::OneRegionRamp),
    (2895, Bc6hLevel::Veryslow, CollapseKind::OneRegionRamp),
    (3187, Bc6hLevel::Veryslow, CollapseKind::OneRegionRamp),
    (4939, Bc6hLevel::Veryslow, CollapseKind::OneRegionRamp),
    (5155, Bc6hLevel::Veryslow, CollapseKind::OneRegionRamp),
    (5325, Bc6hLevel::Veryslow, CollapseKind::OneRegionRamp),
    (5747, Bc6hLevel::Veryslow, CollapseKind::OneRegionRamp),
];

/// Q2 (Note LT): a region whose span is shorter than one code cell got both endpoints on one code and decoded flat at
/// the cell centre, and refinement could not leave (every texel takes one index, so the refit is singular and
/// flattens to the mean, on the same code) (survey section 1, fable Q2). On each witness every source-varying channel
/// of the region holds at least two distinct decoded values, at the witness's level on every path in `paths()`. Only
/// a source-constant channel may decode flat (opus: a varying channel can regain variation through indices driven by
/// the others). Before the fix every witness decoded flat in all three channels.
///
/// The pair is oriented: where two source-varying channels run against each other in the region (a negative source
/// covariance, the falling channel of an odd `OneRegionRamp` against the two rising ones), their decoded covariance over
/// the region is negative too. A componentwise low-to-high pair would make it positive. Skipped on an Intel CPU, like
/// the pins: the witnesses were selected on the Ryzen's output (Note JC).
#[test]
fn collapsed_regions_take_a_bracketing_pair() {
    if encoders_sys::skip_byte_pins() {
        return;
    }
    let mut failures = Vec::new();
    for target in paths() {
        for &(i, level, kind) in COLLAPSED_WITNESSES {
            let (px, region) = collapsed_block(i, kind);
            let dec = decode_halves(&encode_block(&px, level, target));
            let source = source_varies(&px, region);
            let flat: Vec<usize> = (0..3)
                .filter(|&c| source[c] && !varies(region, |t| dec[t][c]))
                .collect();
            if !flat.is_empty() {
                failures.push(format!(
                    "witness {i} {kind:?} {target:?} {level:?}: channels {flat:?} vary in the source and decode flat"
                ));
            }
            for (a, b) in [(0, 1), (0, 2), (1, 2)] {
                if !(source[a] && source[b]) {
                    continue;
                }
                let src = region_covariance(|t, c| px[t][c], region, a, b);
                let got = region_covariance(|t, c| dec[t][c], region, a, b);
                if src < 0.0 && got >= 0.0 {
                    failures.push(format!(
                        "witness {i} {kind:?} {target:?} {level:?}: channels {a} and {b} run against each other \
                         in the source (covariance {src}) but not in the decode ({got})"
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} witness failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Q6's seeded blocks from index `i` (alpha 1.0): 4 to 8 exact-black texels (all channels 0) and the others one
/// coloured value of luminance 0.5 to 20, scaled per texel by 0.6 to 1.0, a third of them with one channel exactly 0
/// (as RGBE skies and suns have, so the lift in `bc6h_peak_retry` changes the block). Integer arithmetic and exact
/// float scalings only, so the blocks are the same on every machine.
fn black_gate_block(i: u32) -> [[u16; 4]; 16] {
    let mut r = Lcg(i.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B);
    r.next();
    let blacks = r.range(4, 8) as usize;
    let mut order: [usize; 16] = std::array::from_fn(|t| t);
    for t in (1..16).rev() {
        order.swap(t, r.next() as usize % (t + 1));
    }
    let lum = 0.5 + r.range(0, 1000) as f32 * 0.0195;
    let w: [f32; 3] = std::array::from_fn(|_| 0.1 + r.range(0, 900) as f32 * 0.001);
    let norm = lum / (0.2126 * w[0] + 0.7152 * w[1] + 0.0722 * w[2]);
    let mut px = [[0u16, 0, 0, f16::ONE.to_bits()]; 16];
    for &t in &order[blacks..] {
        let s = 0.6 + r.range(0, 8) as f32 * 0.05;
        let zero = r.next().is_multiple_of(3).then(|| (r.next() % 3) as usize);
        for c in 0..3 {
            if zero != Some(c) {
                px[t][c] = f16::from_f32(w[c] * norm * s).to_bits();
            }
        }
    }
    px
}

/// The gate value of `bc6h_setup`, `ln(peak) - mean(ln(floored luminance))`, in f32. The old floor is `1e-6`; with
/// `relative` each texel is floored at `max(peak * 2^-12, 1e-6)` (Q6).
fn black_gate_value(px: &[[u16; 4]; 16], relative: bool) -> f32 {
    let l: [f32; 16] = std::array::from_fn(|t| {
        let v = |c: usize| f16::from_bits(px[t][c]).to_f32();
        0.2126 * v(0) + 0.7152 * v(1) + 0.0722 * v(2)
    });
    let peak = l.iter().copied().fold(-1.0f32, f32::max);
    let floor = if relative {
        (peak * (1.0 / 4096.0)).max(1e-6)
    } else {
        1e-6
    };
    let log_sum: f32 = l.iter().map(|&v| v.max(floor).ln()).sum();
    peak.max(1e-6).ln() - log_sum / 16.0
}

/// Whether `black_gate_block(i)` guards under the old floor, not under the relative one, and neither gate value lies
/// within 1e-3 (relative) of `ln 32` (Task 2's tie band: the kernel's fast-math `log` may decide those differently).
fn black_gate_candidate(px: &[[u16; 4]; 16]) -> bool {
    let limit = 32.0f32.ln();
    let (old, new) = (black_gate_value(px, false), black_gate_value(px, true));
    let clear = |g: f32| (g - limit).abs() > 1e-3 * limit;
    clear(old) && clear(new) && old >= limit && new < limit
}

/// Finds the `black_gate_block` indices in `0..1 << 16` that `black_gate_candidate` accepts and whose VeryslowPeak
/// bytes differ from Veryslow's on every path in `paths()` (run it with `SDC_TEST_AVX512=1` so AVX-512 is included),
/// writes each as `<i> <black texels>` to the file `SDC_BC6H_WITNESS_OUT` names, and prints the count and the first
/// hits per black-texel count. With the variable unset it says so and returns.
#[test]
#[ignore = "search for black-gate witnesses: run on the objects before the Q6 fix, with SDC_TEST_AVX512=1"]
fn search_black_gate_witnesses() {
    use std::io::Write as _;
    const END: u32 = 1 << 16;
    let Some(path) = std::env::var_os("SDC_BC6H_WITNESS_OUT") else {
        eprintln!("skipped: SDC_BC6H_WITNESS_OUT is not set");
        return;
    };
    let targets: Vec<IspcTarget> = paths().collect();
    let mut file = std::fs::File::create(&path).unwrap();
    writeln!(
        file,
        "# commit {} input black_gate_block 0..{END} paths {targets:?}",
        git_describe()
    )
    .unwrap();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8)) as u32;
    let mut hits: Vec<u32> = std::thread::scope(|s| {
        let jobs: Vec<_> = (0..threads)
            .map(|t| {
                let targets = &targets;
                s.spawn(move || {
                    (t..END)
                        .step_by(threads as usize)
                        .filter(|&i| {
                            let px = black_gate_block(i);
                            black_gate_candidate(&px)
                                && targets.iter().all(|&target| {
                                    encode_block(&px, Bc6hLevel::VeryslowPeak, target)
                                        != encode_block(&px, Bc6hLevel::Veryslow, target)
                                })
                        })
                        .collect::<Vec<u32>>()
                })
            })
            .collect();
        jobs.into_iter().flat_map(|j| j.join().unwrap()).collect()
    });
    hits.sort_unstable();
    let blacks = |i: u32| {
        black_gate_block(i)
            .iter()
            .filter(|p| p[..3] == [0; 3])
            .count()
    };
    for &i in &hits {
        writeln!(file, "{i} {}", blacks(i)).unwrap();
    }
    file.flush().unwrap();
    println!("{} of {END} are witnesses on {targets:?}", hits.len());
    for n in 4..=8 {
        let first: Vec<u32> = hits
            .iter()
            .copied()
            .filter(|&i| blacks(i) == n)
            .take(4)
            .collect();
        println!("{n} black texels: {first:?}");
    }
}

/// Witnesses from `search_black_gate_witnesses`: `black_gate_block` indices. On the objects before the Q6 fix each
/// guarded under the `1e-6` floor and not under the relative one, and VeryslowPeak's bytes differed from Veryslow's on
/// SSE2, AVX2 and AVX-512 (found on Windows on an AMD Ryzen 7 9800X3D, 2026-11-25).
/// Per black-texel count (4, 5, 6) the first hits: 19,872 of 65,536 blocks qualified, none with 7 or 8 black texels.
const BLACK_GATE_WITNESSES: &[u32] = &[25, 27, 17, 30, 49, 11, 13, 28];

/// Q6 (Note LT): the peak gate should fire on a highlight, not on exact black. Each exact-black texel was floored at
/// `1e-6` in the geometric mean and pulled it down by 13.8 / 16, so four to eight of them guarded a block with no
/// highlight; an unguarded block encodes exactly as Veryslow (`peak_k = -1` returns early from every peak path). On
/// each witness VeryslowPeak's bytes equal Veryslow's on every path in `paths()`, and the replication of the gate
/// guards the block under the old floor and not under the new one (so a witness cannot pass because it is no longer
/// guarded by the replication's own count). Skipped on an Intel CPU, like the pins: the witnesses were selected on
/// the Ryzen's output (Note JC).
#[test]
fn exact_black_alone_does_not_guard_a_block() {
    if encoders_sys::skip_byte_pins() {
        return;
    }
    let mut failures = Vec::new();
    for &i in BLACK_GATE_WITNESSES {
        let px = black_gate_block(i);
        if !black_gate_candidate(&px) {
            failures.push(format!("witness {i}: the gate replication no longer fits"));
        }
        for target in paths() {
            let peak = encode_block(&px, Bc6hLevel::VeryslowPeak, target);
            let plain = encode_block(&px, Bc6hLevel::Veryslow, target);
            if peak != plain {
                failures.push(format!(
                    "witness {i} {target:?}: VeryslowPeak {} differs from Veryslow {}",
                    hex::encode(peak),
                    hex::encode(plain)
                ));
            }
        }
    }
    assert!(
        failures.is_empty() && !BLACK_GATE_WITNESSES.is_empty(),
        "{} witness failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// astra #6's scenario from index `i` (alpha 1.0): two channels whose spans are within 10 % of each other, both over
/// the single window of an asymmetric family and under its doubled one, and a third channel near constant, each
/// centred near half 0x3800 (0.5). Even `i` takes mode 2's family, odd `i` mode 6's: `get_span` is 204.8 and 3686.3
/// UF16 units, about 99 and 1786 half codes (times 31 / 64), doubled about 198 and 3571. The wider channel has 2 to 4
/// texels at its two ends and the rest within a sixteenth of its span of the middle; the other is split evenly, eight
/// texels at each end, on a two-region partition with eight texels a side. Widening the wider channel (the only
/// variant before astra #6) leaves all 16 texels of the split channel outside its window; widening the split channel
/// leaves only the outliers outside. Up to 4 half codes of noise, inward from each end. Integer arithmetic only, so
/// the blocks are the same on every machine.
fn wide_pair(i: u32) -> [[u16; 4]; 16] {
    let mut r = Lcg(i.wrapping_mul(0x9E37_79B9) ^ 0x510E_527F);
    r.next();
    // The wider span is over 10 / 9 of the single window, so the other (at least 90 % of it) is over it too.
    let (lo_span, hi_span) = if i.is_multiple_of(2) {
        (112, 196)
    } else {
        (1986, 3569)
    };
    let span_a = r.range(lo_span, hi_span) as i64;
    let span_b = span_a * r.range(90, 99) as i64 / 100;
    let even = TWO_REGION_MASKS.iter().filter(|m| m.count_ones() == 8);
    let mask = *even.clone().nth(r.next() as usize % even.count()).unwrap();
    // The channels' roles: outliers, split, near constant.
    let roles = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ][r.range(0, 5) as usize];
    let n_out = r.range(2, 4) as usize;
    let mut order: [usize; 16] = std::array::from_fn(|t| t);
    for t in (1..16).rev() {
        order.swap(t, r.next() as usize % (t + 1));
    }
    let noise = r.range(0, 4) as i64;
    let mut planes = [[0u16; 16]; 3];
    for (role, &channel) in roles.iter().enumerate() {
        let mid = 0x3800 + r.range(0, 0x200) as i64 - 0x100;
        let plane = &mut planes[channel];
        match role {
            0 => {
                let (lo, hi) = (mid - span_a / 2, mid - span_a / 2 + span_a);
                for (rank, &t) in order.iter().enumerate() {
                    plane[t] = match rank {
                        0 => lo,
                        1 => hi,
                        _ if rank < n_out => {
                            if r.next().is_multiple_of(2) {
                                lo + r.range(0, noise as u32) as i64
                            } else {
                                hi - r.range(0, noise as u32) as i64
                            }
                        }
                        _ => mid + r.range(0, (span_a / 8) as u32) as i64 - span_a / 16,
                    } as u16;
                }
            }
            1 => {
                let lo = mid - span_b / 2;
                for (t, v) in plane.iter_mut().enumerate() {
                    let dither = r.range(0, noise as u32) as i64;
                    *v = if (mask >> t) & 1 == 1 {
                        lo + span_b - dither
                    } else {
                        lo + dither
                    } as u16;
                }
            }
            _ => {
                for v in plane.iter_mut() {
                    *v = (mid + r.range(0, noise as u32) as i64) as u16;
                }
            }
        }
    }
    texels_of(&planes)
}

/// Records Task 22's search (`wide_pair`, `i in 0..1 << 16`, Veryslow and VeryslowPeak) for `WIDE_CHANNEL_WITNESSES`.
#[test]
#[ignore = "record wide-channel witnesses: run on the objects before the change, then after"]
fn search_wide_channel_witnesses() {
    let levels = [Bc6hLevel::Veryslow, Bc6hLevel::VeryslowPeak];
    record_witness_search("wide_pair", wide_pair, 1 << 16, &levels);
}

/// Witnesses from `search_wide_channel_witnesses`: (`wide_pair` index, level, the bytes the objects before the astra #6
/// fix wrote on SSE2, AVX2 and AVX-512, in `IspcTarget::ALL` order). Recorded on Windows on an AMD Ryzen 7 9800X3D,
/// 2026-11-25. Five are mode 2's family (even index) and three mode 6's (odd); after the fix each writes a variant of
/// its family that widens a channel other than the widest. The three paths agree on them.
#[rustfmt::skip]
const WIDE_CHANNEL_WITNESSES: &[(u32, Bc6hLevel, [[u8; 16]; 3])] = &[
    (1038, Bc6hLevel::Veryslow, [
        [0xa0, 0x38, 0xe7, 0x8e, 0x03, 0x63, 0x70, 0x00, 0x08, 0x62, 0x97, 0xe2, 0x24, 0x52, 0xe2, 0x24],
        [0xa0, 0x38, 0xe7, 0x8e, 0x03, 0x63, 0x70, 0x00, 0x08, 0x62, 0x97, 0xe2, 0x24, 0x52, 0xe2, 0x24],
        [0xa0, 0x38, 0xe7, 0x8e, 0x03, 0x63, 0x70, 0x00, 0x08, 0x62, 0x97, 0xe2, 0x24, 0x52, 0xe2, 0x24],
    ]),
    (19911, Bc6hLevel::Veryslow, [
        [0x01, 0x07, 0x1d, 0x6c, 0x01, 0x80, 0x27, 0x00, 0x01, 0xe0, 0x07, 0xf0, 0x03, 0x00, 0x00, 0x20],
        [0x01, 0x07, 0x1d, 0x6c, 0x01, 0x80, 0x27, 0x00, 0x01, 0xe0, 0x07, 0xf0, 0x03, 0x00, 0x00, 0x20],
        [0x01, 0x07, 0x1d, 0x6c, 0x01, 0x80, 0x27, 0x00, 0x01, 0xe0, 0x07, 0xf0, 0x03, 0x00, 0x00, 0x20],
    ]),
    (29508, Bc6hLevel::Veryslow, [
        [0xa2, 0x72, 0xc5, 0x39, 0xcf, 0x0c, 0x30, 0x00, 0xba, 0x6c, 0x40, 0x84, 0x3b, 0x76, 0x88, 0x10],
        [0xa2, 0x72, 0xc5, 0x39, 0xcf, 0x0c, 0x30, 0x00, 0xba, 0x6c, 0x40, 0x84, 0x3b, 0x76, 0x88, 0x10],
        [0xa2, 0x72, 0xc5, 0x39, 0xcf, 0x0c, 0x30, 0x00, 0xba, 0x6c, 0x40, 0x84, 0x3b, 0x76, 0x88, 0x10],
    ]),
    (29807, Bc6hLevel::Veryslow, [
        [0xa1, 0x86, 0x1c, 0x6a, 0x01, 0x00, 0x00, 0x82, 0x10, 0x84, 0x83, 0xe3, 0x01, 0x3a, 0x9e, 0xff],
        [0xa1, 0x86, 0x1c, 0x6a, 0x01, 0x00, 0x00, 0x82, 0x10, 0x84, 0x83, 0xe3, 0x01, 0x3a, 0x9e, 0xff],
        [0xa1, 0x86, 0x1c, 0x6a, 0x01, 0x00, 0x00, 0x82, 0x10, 0x84, 0x83, 0xe3, 0x01, 0x3a, 0x9e, 0xff],
    ]),
    (48084, Bc6hLevel::Veryslow, [
        [0x20, 0x39, 0xe4, 0xa0, 0x13, 0x08, 0x20, 0x00, 0x00, 0x0f, 0x80, 0x04, 0xc8, 0x01, 0x00, 0x4f],
        [0x20, 0x39, 0xe4, 0xa0, 0x13, 0x08, 0x20, 0x00, 0x00, 0x0f, 0x80, 0x04, 0xc8, 0x01, 0x00, 0x4f],
        [0x20, 0x39, 0xe4, 0xa0, 0x13, 0x08, 0x20, 0x00, 0x00, 0x0f, 0x80, 0x04, 0xc8, 0x01, 0x00, 0x4f],
    ]),
    (59874, Bc6hLevel::Veryslow, [
        [0xc8, 0x39, 0xe4, 0xa2, 0x03, 0x08, 0x20, 0xd1, 0x01, 0x60, 0x07, 0x06, 0xee, 0xf8, 0xf0, 0x01],
        [0xc8, 0x39, 0xe4, 0xa2, 0x03, 0x08, 0x20, 0xd1, 0x01, 0x60, 0x07, 0x06, 0xee, 0xf8, 0xf0, 0x01],
        [0xc8, 0x39, 0xe4, 0xa2, 0x03, 0x08, 0x20, 0xd1, 0x01, 0x60, 0x07, 0x06, 0xee, 0xf8, 0xf0, 0x01],
    ]),
    (37930, Bc6hLevel::VeryslowPeak, [
        [0x70, 0xb9, 0xe5, 0xa4, 0x03, 0x00, 0x00, 0x11, 0x48, 0xb2, 0x93, 0x1c, 0x01, 0x38, 0x10, 0x00],
        [0x70, 0xb9, 0xe5, 0xa4, 0x03, 0x00, 0x00, 0x11, 0x48, 0xb2, 0x93, 0x1c, 0x01, 0x38, 0x10, 0x00],
        [0x70, 0xb9, 0xe5, 0xa4, 0x03, 0x00, 0x00, 0x11, 0x48, 0xb2, 0x93, 0x1c, 0x01, 0x38, 0x10, 0x00],
    ]),
    (59765, Bc6hLevel::VeryslowPeak, [
        [0x72, 0x8e, 0x35, 0xe8, 0x48, 0x1c, 0x70, 0x00, 0x84, 0x5c, 0x13, 0x9e, 0x28, 0xb9, 0x02, 0x05],
        [0x72, 0x8e, 0x35, 0xe8, 0x48, 0x1c, 0x70, 0x00, 0x84, 0x5c, 0x13, 0x9e, 0x28, 0xb9, 0x02, 0x05],
        [0x72, 0x8e, 0x35, 0xe8, 0x48, 0x1c, 0x70, 0x00, 0x84, 0x5c, 0x13, 0x9e, 0x28, 0xb9, 0x02, 0x05],
    ]),
];

/// astra #6 (Note LT): at the slow levels an asymmetric family (modes 2-4, 6-8) also tries widening the channel whose
/// single window clips the block least, when that clips less than widening the widest channel. On each witness the
/// encode is certainly better than the bytes the objects before the fix wrote (`err_bounds`: E_hi now < E_lo before),
/// on every path in `paths()`. Before the fix the encode is those bytes, so the test fails there. Skipped on an Intel
/// CPU, like the pins: the bytes are the Ryzen's (Note JC).
#[test]
fn asymmetric_modes_try_the_better_wide_channel() {
    if encoders_sys::skip_byte_pins() {
        return;
    }
    assert_witnesses_certainly_better(WIDE_CHANNEL_WITNESSES, wide_pair);
}

#[test]
#[should_panic(expected = "multiple of 4")]
fn rejects_unaligned_width() {
    let px = vec![[0u16; 4]; 6 * 4];
    let mut out = [0u8; 32];
    bc6h_compress_rows(&px, 6, 4, &mut out, Bc6hLevel::Veryslow);
}
