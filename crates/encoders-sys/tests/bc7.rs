use encoders_sys::{
    bc7e_compress_blocks, bc7e_compress_blocks_on, gather_blocks, ispc_target, Bc7Level, Bc7Params,
    IspcTarget,
};

const U5: Bc7Params = Bc7Params {
    level: Bc7Level::Veryslow,
    perceptual: false,
    exact_opaque: false,
};

fn decode(block: &[u8]) -> Vec<[u8; 4]> {
    let mut out = [0u8; 64];
    bcdec_rs::bc7(block, &mut out, 16);
    out.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()
}

const LEVELS: [Bc7Level; 8] = [
    Bc7Level::Ultrafast,
    Bc7Level::Veryfast,
    Bc7Level::Fast,
    Bc7Level::Basic,
    Bc7Level::Slow,
    Bc7Level::Veryslow,
    Bc7Level::Slowest,
    Bc7Level::Max,
];

/// Every ISPC path this CPU runs up to `ispc_target()` (Note HZ3): SSE2 and AVX2 in a test build, AVX-512 too with
/// `SDC_TEST_AVX512=1`.
fn paths() -> Vec<IspcTarget> {
    IspcTarget::ALL
        .into_iter()
        .filter(|t| t.supported() && *t <= ispc_target())
        .collect()
}

/// 128x128 opaque white noise: `bc7_fixup.rs`'s xorshift seeded 0x1234_5678_9ABC_DEF1, alpha 255.
fn opaque_white_noise() -> Vec<[u8; 4]> {
    let mut s = 0x1234_5678_9ABC_DEF1u64;
    let px: Vec<[u8; 4]> = (0..128 * 128)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            [s as u8, (s >> 8) as u8, (s >> 16) as u8, 255]
        })
        .collect();
    gather_blocks(&px, 128, 128)
}

/// Raw bc7e bytes (no post-fix), in 64-block calls as the engine makes them.
fn raw_on(blocks: &[[u8; 4]], p: Bc7Params, target: IspcTarget) -> Vec<u8> {
    let mut out = vec![0u8; blocks.len()];
    for (o, b) in out.chunks_mut(64 * 16).zip(blocks.chunks(64 * 16)) {
        bc7e_compress_blocks_on(b, o, p, target);
    }
    out
}

/// `fixDegenerateEndpoints` spread a constant channel when the refit's float endpoints differed by noise. In a
/// rotated mode-4 block of an opaque block that channel holds the alpha, and mode 4's double nudge took 31 to 29,
/// which decodes 239 (Note LR; DECISIONS-1.x line 52's fingerprint). With the flat tolerance no such block is written.
#[test]
fn opaque_noise_never_double_nudges_the_rotated_slot() {
    let blocks = opaque_white_noise();
    let mut found = vec![];
    let mut total = 0;
    for level in [Bc7Level::Basic, Bc7Level::Veryslow] {
        for exact_opaque in [false, true] {
            let p = Bc7Params {
                level,
                perceptual: false,
                exact_opaque,
            };
            for target in paths() {
                let n = raw_on(&blocks, p, target)
                    .chunks(16)
                    .filter(|b| {
                        let v = u128::from_le_bytes((*b).try_into().unwrap());
                        b[0] & 0x1f == 0x10
                            && (v >> 5) & 3 != 0
                            && decode(b).iter().any(|t| t[3] <= 239)
                    })
                    .count();
                total += n;
                found.push(format!("{level:?} exact={exact_opaque} {target:?}: {n}"));
            }
        }
    }
    assert_eq!(
        total,
        0,
        "rotated mode-4 blocks decoding alpha 239 or below:\n{}",
        found.join("\n")
    );
}

#[test]
fn gather_blocks_orders_blocks_row_major_and_replicates_edges() {
    // 5x3 image, texel value = (x, y, 0, 255)
    let px: Vec<[u8; 4]> = (0..3)
        .flat_map(|y| (0..5).map(move |x| [x as u8, y as u8, 0, 255]))
        .collect();
    let b = gather_blocks(&px, 5, 3);
    assert_eq!(b.len(), 2 * 16); // 2x1 blocks
    assert_eq!(b[0], [0, 0, 0, 255]);
    assert_eq!(b[5], [1, 1, 0, 255]); // block 0, row 1, col 1
    assert_eq!(b[15], [3, 2, 0, 255]); // block 0, row 3 replicates row 2
    assert_eq!(b[16], [4, 0, 0, 255]); // block 1 starts at x = 4
    assert_eq!(b[17], [4, 0, 0, 255]); // x = 5 replicates x = 4
}

#[test]
fn solid_block_round_trips_exactly() {
    let px = vec![[200u8, 100, 50, 255]; 16];
    let mut out = [0u8; 16];
    bc7e_compress_blocks(&px, &mut out, U5);
    assert!(decode(&out).iter().all(|p| *p == [200, 100, 50, 255]));
}

#[test]
fn opaque_gradient_decodes_alpha_at_least_254() {
    let px: Vec<[u8; 4]> = (0..16)
        .map(|i| [i as u8 * 16, 255 - i as u8 * 10, 7 * i as u8, 255])
        .collect();
    let mut out = [0u8; 16];
    bc7e_compress_blocks(&px, &mut out, U5);
    assert!(decode(&out).iter().all(|p| p[3] >= 254));
}

#[test]
#[should_panic(expected = "16 bytes per block")]
fn rejects_mismatched_output_length() {
    let px = vec![[0u8; 4]; 16];
    let mut out = [0u8; 8];
    bc7e_compress_blocks(&px, &mut out, U5);
}

#[test]
fn exact_opaque_decodes_alpha_exactly_255_where_upstream_gives_254() {
    // A dark opaque ramp that upstream bc7e u5 encodes in mode 6 with a p-bit 0 endpoint (alpha 254).
    let px: Vec<[u8; 4]> = (0..16u8).map(|i| [i, i / 2, 100 + i, 255]).collect();
    let (mut off, mut on) = ([0u8; 16], [0u8; 16]);
    bc7e_compress_blocks(&px, &mut off, U5);
    bc7e_compress_blocks(
        &px,
        &mut on,
        Bc7Params {
            exact_opaque: true,
            ..U5
        },
    );
    assert!(
        decode(&off).iter().any(|p| p[3] == 254),
        "flag off: {off:02x?}"
    );
    assert_eq!(on[0] & 0x7f, 0x40, "flag on: still mode 6: {on:02x?}");
    assert!(
        decode(&on).iter().all(|p| p[3] == 255),
        "flag on: {on:02x?}"
    );
}

/// Patch 0005 (Q2t): the Rust constants are the kernel's. Every level's default is `BC7_FLAT_TOLERANCE_DEFAULT` bit for
/// bit; T_data has no C default, so its constant is pinned against the literal the measured arms compared with
/// (`2.0f / 255.0f`), and its kernel-side proof is the measurement match (Note LR, LR3).
#[test]
fn flat_tolerance_constants_match_the_kernel() {
    use encoders_sys::{
        bc7e_level_flat_tolerance, BC7_FLAT_TOLERANCE_DATA, BC7_FLAT_TOLERANCE_DEFAULT,
    };
    for level in LEVELS {
        for perceptual in [false, true] {
            for exact_opaque in [false, true] {
                let p = Bc7Params {
                    level,
                    perceptual,
                    exact_opaque,
                };
                assert_eq!(
                    bc7e_level_flat_tolerance(p).to_bits(),
                    BC7_FLAT_TOLERANCE_DEFAULT.to_bits(),
                    "{p:?}"
                );
            }
        }
    }
    assert_eq!(
        BC7_FLAT_TOLERANCE_DATA.to_bits(),
        (2.0f32 / 255.0).to_bits()
    );
}

/// Patch 0005 (Q2t): `Bc7Tune::flat_tolerance` reaches the kernel. `Default` is the level's own value, so it writes the
/// plain call's bytes; `Data` writes other bytes on some blocks of step A's noise (3 of 1024 on each path when
/// written, Note LR).
#[test]
fn flat_tolerance_slot_reaches_the_kernel() {
    use encoders_sys::{bc7e_compress_blocks_tuned_on, Bc7FlatTolerance, Bc7Tune};
    let blocks = opaque_white_noise();
    let tuned = |t, target| {
        let mut out = vec![0u8; blocks.len()];
        for (o, b) in out.chunks_mut(64 * 16).zip(blocks.chunks(64 * 16)) {
            let tune = Bc7Tune {
                flat_tolerance: Some(t),
                ..Bc7Tune::default()
            };
            bc7e_compress_blocks_tuned_on(b, o, U5, tune, target);
        }
        out
    };
    for target in paths() {
        let plain = raw_on(&blocks, U5, target);
        assert!(
            tuned(Bc7FlatTolerance::Default, target) == plain,
            "{target:?}: Default is the level's tolerance"
        );
        let data = tuned(Bc7FlatTolerance::Data, target);
        let changed = plain
            .chunks(16)
            .zip(data.chunks(16))
            .filter(|(a, b)| a != b)
            .count();
        assert!(changed > 0, "{target:?}: Data wrote the plain call's bytes");
    }
}

/// `Bc7Tune::exact_constants` covers R, G and B only: alpha is the mask's bit 3, set from `Bc7Params::exact_opaque`
/// (Note LR, LR17), so an alpha bit is refused rather than dropped, even with no block to encode.
#[test]
#[should_panic(expected = "R, G and B only")]
fn exact_constants_refuse_an_alpha_bit() {
    use encoders_sys::{bc7e_compress_blocks_tuned, Bc7Tune};
    let tune = Bc7Tune {
        exact_constants: Some(0b1000),
        ..Bc7Tune::default()
    };
    bc7e_compress_blocks_tuned(&[], &mut [], U5, tune);
}

/// The seed of `exact_constant_fixture` (Note LR, LR12).
const EXACT_SEED: u32 = 0x5C0C_2026;

/// A 64x64 opaque image, row-major, whose G is `g` on every texel, from an LCG seeded with `seed`
/// (`s = s * 1664525 + 1013904223`, each draw `s >> 24`). Each block k (row-major, 16 per row) draws four centres (R and
/// B of two clusters) whose p-bit is the other one G needs: `255 - 2 * (d % 16)` (odd, 225 to 255) when `g` is 0,
/// `2 * (d % 16)` (even, 0 to 30) when it is 255. Then per texel, by k % 4: 0 two clusters (texel columns 0-1 and 2-3),
/// the centre plus `d % 5 - 2` on R, then on B; 1 the same, but columns 0-1 are cluster 0's centre without noise (a
/// solid subset of BC7's first two-subset partition beside a noisy one); 2 near-solid (cluster 0's centre, R and B both
/// plus 1 when `d % 3 == 0`); 3 solid (cluster 0's centre; bc7e's mode 5 solid-block path). Values are clamped to 0-255.
fn exact_constant_fixture(g: u8, seed: u32) -> Vec<[u8; 4]> {
    let mut s = seed;
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 24) as i32
    };
    let centre = |v: i32| {
        if g == 0 {
            255 - 2 * (v % 16)
        } else {
            2 * (v % 16)
        }
    };
    let mut px = vec![[0u8, g, 0, 255]; 64 * 64];
    for k in 0..256 {
        let (bx, by) = (k % 16 * 4, k / 16 * 4);
        let kind = k % 4;
        let c = [
            [centre(next()), centre(next())],
            [centre(next()), centre(next())],
        ];
        for t in 0..16 {
            let (x, y) = (t % 4, t / 4);
            let [r, b] = match kind {
                1 if x < 2 => c[0],
                0 | 1 => {
                    let [r, b] = c[x / 2];
                    [r + next() % 5 - 2, b + next() % 5 - 2]
                }
                2 => {
                    let up = i32::from(next() % 3 == 0);
                    [c[0][0] + up, c[0][1] + up]
                }
                _ => c[0],
            };
            px[(by + y) * 64 + bx + x] = [r.clamp(0, 255) as u8, g, b.clamp(0, 255) as u8, 255];
        }
    }
    px
}

/// Patch 0005, XC: with G in `Bc7Tune::exact_constants`, a subset whose G is 0 on every texel, or 255 on every texel,
/// decodes G exactly. `exact_constant_fixture`'s R and B pull the p-bits away from the value G needs in its clusters,
/// solid-beside-noisy and near-solid blocks (its solid blocks take bc7e's mode 5 solid path). Raw bytes, 64-block
/// calls, the data maps' tune (T_data, exact G) at weights x16 and x1, every level, exact opaque off and on, every
/// path. Without the one-colour p guard only ultrafast, veryfast and fast fail, so every level runs. With exact opaque
/// on, alpha decodes 255 after the post-fix in every cell, and wins over G where both cannot hold (ruling BC7-C4):
/// mode 6's forced p-bits cannot decode G 0, so G 0 is not asserted in `exact_g_waived`'s cells (Note LR, LR12).
#[test]
fn exact_constants_keep_a_flat_channel() {
    use encoders_sys::bc7_fixup::force_opaque_rotated;
    use encoders_sys::{bc7e_compress_blocks_tuned_on, Bc7FlatTolerance, Bc7Tune};
    // BC7-C4's waiver: exact opaque on, G 0, at ultrafast, where opaque blocks are mode 6 alone (no exact choice left).
    let exact_g_waived = |g: u8, level: Bc7Level, exact_opaque: bool| {
        exact_opaque && g == 0 && level == Bc7Level::Ultrafast
    };
    let mut bad = vec![];
    for g in [0u8, 255] {
        let blocks = gather_blocks(&exact_constant_fixture(g, EXACT_SEED), 64, 64);
        for (level, exact_opaque) in LEVELS.into_iter().flat_map(|l| [(l, false), (l, true)]) {
            let p = Bc7Params {
                level,
                exact_opaque,
                ..U5
            };
            for weights in [Some([16; 4]), None] {
                let tune = Bc7Tune {
                    weights,
                    flat_tolerance: Some(Bc7FlatTolerance::Data),
                    exact_constants: Some(0b010),
                };
                for target in paths() {
                    let mut out = vec![0u8; blocks.len()];
                    for (o, b) in out.chunks_mut(64 * 16).zip(blocks.chunks(64 * 16)) {
                        bc7e_compress_blocks_tuned_on(b, o, p, tune, target);
                    }
                    let what = format!(
                        "G {g} {level:?} exact={exact_opaque} weights {weights:?} {target:?}"
                    );
                    let errs: Vec<u8> = out
                        .chunks(16)
                        .flat_map(decode)
                        .map(|t| t[1].abs_diff(g))
                        .filter(|e| *e > 0)
                        .collect();
                    if let Some(worst) = errs.iter().max() {
                        if !exact_g_waived(g, level, exact_opaque) {
                            bad.push(format!("{what}: {} texels off, worst {worst}", errs.len()));
                        }
                    }
                    if exact_opaque {
                        // The engine's opaque guarantee is after the rotated-block post-fix.
                        let below: Vec<u8> = out
                            .chunks_mut(16)
                            .flat_map(|b| {
                                force_opaque_rotated(b.try_into().unwrap());
                                decode(b)
                            })
                            .map(|t| t[3])
                            .filter(|a| *a != 255)
                            .collect();
                        if let Some(lowest) = below.iter().min() {
                            bad.push(format!(
                                "{what}: {} texels with alpha below 255, lowest {lowest}",
                                below.len()
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "flat G or exact-opaque alpha decoded off:\n{}",
        bad.join("\n")
    );
}

/// Note CP: a tune that sets only the weights leaves every other level parameter alone: unit weights give the plain
/// call's bytes at each level, and other weights reach bc7e.
#[test]
fn tuned_with_unit_weights_matches_the_plain_call() {
    use encoders_sys::{bc7e_compress_blocks_tuned, Bc7Tune};
    let mut s = 0x9e37_79b9_u32;
    let px: Vec<[u8; 4]> = (0..64 * 16)
        .map(|i| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let b = s.to_le_bytes();
            // Every fourth block opaque, so the exact-opaque tables are exercised too.
            let a = if (i / 16) % 4 == 0 { 255 } else { b[3] };
            [b[0], b[1], b[2], a]
        })
        .collect();
    let tune = |weights| Bc7Tune {
        weights: Some(weights),
        ..Bc7Tune::default()
    };
    let mut checked = 0;
    for level in LEVELS {
        for exact_opaque in [false, true] {
            let p = Bc7Params {
                level,
                exact_opaque,
                ..U5
            };
            let what = format!("{level:?} exact={exact_opaque}");
            let mut plain = vec![0u8; 64 * 16];
            bc7e_compress_blocks(&px, &mut plain, p);
            let mut tuned = vec![0u8; 64 * 16];
            bc7e_compress_blocks_tuned(&px, &mut tuned, p, tune([1, 1, 1, 1]));
            assert_eq!(plain, tuned, "{what}");
            bc7e_compress_blocks_tuned(&px, &mut tuned, p, tune([1, 1, 1, 8]));
            assert_ne!(plain, tuned, "{what}: the weights reach bc7e");
            checked += 1;
        }
    }
    // The vendored objects carry both patches: every level, both exact-opaque settings.
    assert_eq!(checked, 16);
}

/// `g_bc7_partition2` in `bc7e.ispc`, copied: the subset (0 or 1) of each texel of BC7's 64 two-subset partitions.
const PARTITION2: [[u8; 16]; 64] = [
    [0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
    [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1],
    [0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1],
    [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1],
    [0, 0, 1, 1, 0, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1],
    [0, 0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1],
    [0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1],
    [0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1, 1],
    [0, 1, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 1, 0],
    [0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0],
    [0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0],
    [0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0, 1],
    [0, 0, 1, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 0, 0],
    [0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0],
    [0, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0, 0],
    [0, 0, 0, 1, 0, 1, 1, 1, 1, 1, 1, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
    [0, 1, 1, 1, 0, 0, 0, 1, 1, 0, 0, 0, 1, 1, 1, 0],
    [0, 0, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0],
    [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1],
    [0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1],
    [0, 1, 0, 1, 1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0],
    [0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0],
    [0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0],
    [0, 1, 0, 1, 0, 1, 0, 1, 1, 0, 1, 0, 1, 0, 1, 0],
    [0, 1, 1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 1],
    [0, 1, 0, 1, 1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 1],
    [0, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0],
    [0, 0, 0, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 0, 0, 0],
    [0, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 1, 0, 0],
    [0, 0, 1, 1, 1, 0, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0],
    [0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0],
    [0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 1, 1],
    [0, 1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 1],
    [0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0],
    [0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 0],
    [0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 0, 0],
    [0, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 1],
    [0, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1],
    [0, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0],
    [0, 0, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 0],
    [0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 0, 0, 1],
    [0, 1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1],
    [0, 1, 1, 1, 1, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1],
    [0, 0, 0, 1, 1, 0, 0, 0, 1, 1, 1, 0, 0, 1, 1, 1],
    [0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1],
    [0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0],
    [0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0],
    [0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 1, 1, 0, 1, 1, 1],
];

/// The seed of the mode-7 candidates (`mode7_candidate`).
const MODE7_SEED: u32 = 0x7007_2026;

/// Candidate `i` for bc7e's mode-7 one-colour path, from an LCG seeded with `MODE7_SEED ^ i * 0x9E37_79B9`: a
/// two-subset partition from `PARTITION2`; subset 0 one solid colour (R, G and B random, alpha 16 to 255); subset 1 a
/// gradient between two random colours along the texel order (alpha 16 to 239) plus noise of -2 to 2 per channel.
/// Returns the block and the mask of its solid texels (bit t = texel t).
fn mode7_candidate(i: u32) -> ([[u8; 4]; 16], u16) {
    let mut s = MODE7_SEED ^ i.wrapping_mul(0x9E37_79B9);
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 24) as i32
    };
    let part = PARTITION2[(next() % 64) as usize];
    let solid = [next(), next(), next(), 16 + next() % 240];
    let ends = [
        [next(), next(), next(), 16 + next() % 224],
        [next(), next(), next(), 16 + next() % 224],
    ];
    let n = part.iter().filter(|&&s| s == 1).count() as i32;
    let (mut k, mut mask) = (0, 0u16);
    let block = std::array::from_fn(|t| {
        if part[t] == 0 {
            mask |= 1 << t;
            return solid.map(|v| v as u8);
        }
        let mut px = [0u8; 4];
        for c in 0..4 {
            let v = ends[0][c] + (ends[1][c] - ends[0][c]) * k / (n - 1) + next() % 5 - 2;
            let (lo, hi) = if c == 3 { (16, 239) } else { (0, 255) };
            px[c] = v.clamp(lo, hi) as u8;
        }
        k += 1;
        px
    });
    (block, mask)
}

/// The largest per-channel error over the solid texels of each candidate, each encoded alone (one block per call, so a
/// candidate's bytes do not depend on its neighbours in an ISPC gang), raw (no post-fix).
fn mode7_solid_errors(ids: &[u32], p: Bc7Params, target: IspcTarget) -> Vec<u8> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = ids.len().div_ceil(threads).max(1);
    let mut errs = vec![0u8; ids.len()];
    std::thread::scope(|sc| {
        for (ids, errs) in ids.chunks(chunk).zip(errs.chunks_mut(chunk)) {
            sc.spawn(move || {
                for (i, e) in ids.iter().zip(errs) {
                    let (block, mask) = mode7_candidate(*i);
                    let mut out = [0u8; 16];
                    bc7e_compress_blocks_on(&block, &mut out, p, target);
                    *e = decode(&out)
                        .iter()
                        .zip(block)
                        .enumerate()
                        .filter(|(t, _)| (mask >> t) & 1 == 1)
                        .flat_map(|(_, (d, s))| (0..4).map(move |c| d[c].abs_diff(s[c])))
                        .max()
                        .unwrap_or(0);
                }
            });
        }
    });
    errs
}

/// The `mode7_candidate` indices whose solid texels decoded more than 3 off at veryslow on the AVX2 path before patch
/// 0005's Q1 and Q4, and within 3 at basic and veryslow on every path after: the first 32 such candidates found by
/// `search_mode7_witnesses` (indices 0 to 56), kept where the fixed kernel passes (Note LR, LR7).
const MODE7_WITNESSES: &[u32] = &[1, 4, 8, 23, 41, 48, 49];

/// Every witness, level (basic and veryslow) and path whose solid texels decode more than 3 off in some channel.
fn mode7_solid_failures(witnesses: &[u32]) -> Vec<String> {
    let mut bad = vec![];
    for level in [Bc7Level::Basic, Bc7Level::Veryslow] {
        let p = Bc7Params { level, ..U5 };
        for target in paths() {
            let errs = mode7_solid_errors(witnesses, p, target);
            for (i, e) in witnesses.iter().zip(errs) {
                if e > 3 {
                    bad.push(format!("candidate {i} {level:?} {target:?}: {e}"));
                }
            }
        }
    }
    bad
}

/// Patch 0005, Q1 and Q4: mode 7's one-colour candidate (`pack_mode7_to_one_color`) was scored by interpolating the
/// 6-bit endpoint codes as if they were 8-bit values (Q1), so its error was wrong (far too high except near black) and
/// a solid subset could lose to a worse fit; and its table kept the best pair found under earlier p-bits (Q4), so an
/// entry could hold endpoints that decode off under its own p-bits. Each witness's solid subset now decodes within 3
/// per channel at basic and veryslow on every path. These witnesses pass with Q1 alone, so they guard Q1; Q4 has its
/// own witnesses below (Note LR, LR7).
#[test]
fn mode7_takes_its_solid_subset() {
    let bad = mode7_solid_failures(MODE7_WITNESSES);
    assert!(
        bad.is_empty(),
        "solid texels decoded more than 3 off:\n{}",
        bad.join("\n")
    );
}

/// The `mode7_candidate` indices whose solid texels decode more than 3 off at veryslow on the AVX2 path before patch
/// 0005's Q1 and Q4 and with Q1 alone, and within 3 at basic and veryslow with both: the first 8 of the 6516 such
/// indices below 2^20 (a scan on AVX2 of each kernel). With Q1 alone each also fails on every path, and with both each
/// passes on every path (Note LR, LR7).
const MODE7_Q4_WITNESSES: &[u32] = &[332, 442, 722, 891, 1008, 1135, 1220, 1307];

/// Patch 0005, Q4: with Q1 in, mode 7's one-colour candidate wins more often, and a table entry holding a pair found
/// under other p-bits decodes a solid subset more than 3 off. These witnesses fail with Q1 alone and pass with Q4 too,
/// at basic and veryslow on every path (Note LR, LR7).
#[test]
fn mode7_table_keeps_each_p_bit_pairs_own_best() {
    let bad = mode7_solid_failures(MODE7_Q4_WITNESSES);
    assert!(
        bad.is_empty(),
        "solid texels decoded more than 3 off:\n{}",
        bad.join("\n")
    );
}

/// Step A0 of patch 0005's Q1 and Q4 (Note LR): scans `mode7_candidate` at veryslow on the AVX2 path and prints the
/// candidates whose solid texels decode more than 3 off in some channel, up to 32.
#[test]
#[ignore = "search for mode-7 witnesses: run with --ignored --nocapture after a kernel change"]
fn search_mode7_witnesses() {
    assert!(
        IspcTarget::Avx2.supported(),
        "the search runs on the AVX2 path"
    );
    let p = Bc7Params {
        level: Bc7Level::Veryslow,
        ..U5
    };
    let start = std::time::Instant::now();
    let (mut scanned, mut found) = (0u32, 0);
    'scan: for base in (0..1u32 << 20).step_by(1 << 14) {
        let ids: Vec<u32> = (base..base + (1 << 14)).collect();
        for (i, e) in ids
            .iter()
            .zip(mode7_solid_errors(&ids, p, IspcTarget::Avx2))
        {
            scanned += 1;
            if e > 3 {
                println!("candidate {i}: solid error {e}");
                found += 1;
                if found == 32 {
                    break 'scan;
                }
            }
        }
    }
    println!(
        "scanned {scanned}, found {found}, in {:.1} s",
        start.elapsed().as_secs_f64()
    );
}

/// The seed of the Q3 candidates (`q3_candidate`).
const Q3_SEED: u32 = 0x5133_2026;

/// Candidate `i` for mode 6's alpha-block selectors, from an LCG seeded with `Q3_SEED ^ i * 0x9E37_79B9`
/// (`s = s * 1664525 + 1013904223`, each draw `s >> 24`): two random colours (R, G, B), then two random alphas (16 to
/// 239); per texel in order, R, G and B on the ramp between the colours along the texel order plus noise of -2 to 2, then
/// alpha on its own ramp between the alphas plus noise of -24 to 24, clamped to 16-239. So alpha is only loosely tied to
/// the colour.
fn q3_candidate(i: u32) -> [[u8; 4]; 16] {
    let mut s = Q3_SEED ^ i.wrapping_mul(0x9E37_79B9);
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 24) as i32
    };
    let ends = [[next(), next(), next()], [next(), next(), next()]];
    let alpha = [16 + next() % 224, 16 + next() % 224];
    std::array::from_fn(|t| {
        let t = t as i32;
        let mut px = [0u8; 4];
        for c in 0..3 {
            let v = ends[0][c] + (ends[1][c] - ends[0][c]) * t / 15 + next() % 5 - 2;
            px[c] = v.clamp(0, 255) as u8;
        }
        let a = alpha[0] + (alpha[1] - alpha[0]) * t / 15 + next() % 49 - 24;
        px[3] = a.clamp(16, 239) as u8;
        px
    })
}

/// BC7's 4-bit index weights (mode 6's palette).
const BC7_WEIGHTS4: [u32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

/// For a mode-6 block, whether every texel's index has the smallest weighted error `sum w[c] * (pal[c] - src[c])^2`
/// over the 16 palette entries (a tie passes). The endpoints (7 bits plus each endpoint's p-bit) and the indices (3 bits
/// for texel 0, 4 for the rest) are read by the BC7 layout, and the palette is checked against the decoder. Any other
/// mode returns true.
fn mode6_indices_are_weighted_nearest(block: &[u8; 16], src: &[[u8; 4]; 16], w: [u32; 4]) -> bool {
    if block[0] & 0x7f != 0x40 {
        return true;
    }
    let v = u128::from_le_bytes(*block);
    let bits = |at: u32, n: u32| ((v >> at) & ((1 << n) - 1)) as u32;
    let p = [bits(63, 1), bits(64, 1)];
    let ends: [[u32; 4]; 2] = std::array::from_fn(|k| {
        std::array::from_fn(|c| (bits(7 + 14 * c as u32 + 7 * k as u32, 7) << 1) | p[k])
    });
    let pal: [[u32; 4]; 16] = std::array::from_fn(|j| {
        let wj = BC7_WEIGHTS4[j];
        std::array::from_fn(|c| ((64 - wj) * ends[0][c] + wj * ends[1][c] + 32) >> 6)
    });
    let err = |j: usize, s: &[u8; 4]| -> u64 {
        (0..4)
            .map(|c| u64::from(w[c]) * u64::from(pal[j][c].abs_diff(u32::from(s[c]))).pow(2))
            .sum()
    };
    let decoded = decode(block);
    (0..16).all(|t| {
        let idx = if t == 0 {
            bits(65, 3)
        } else {
            bits(64 + 4 * t as u32, 4)
        } as usize;
        assert_eq!(
            pal[idx].map(|c| c as u8),
            decoded[t],
            "hand decode of texel {t}: {block:02x?}"
        );
        let best = (0..16).map(|j| err(j, &src[t])).min().unwrap();
        err(idx, &src[t]) == best
    })
}

/// Each `q3_candidate` of `ids` encoded alone (one block per call, so its bytes do not depend on gang neighbours) with
/// linear weights `w`, raw (no post-fix).
fn q3_outputs(ids: &[u32], p: Bc7Params, w: [u32; 4], target: IspcTarget) -> Vec<[u8; 16]> {
    use encoders_sys::{bc7e_compress_blocks_tuned_on, Bc7Tune};
    let tune = Bc7Tune {
        weights: Some(w),
        ..Bc7Tune::default()
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = ids.len().div_ceil(threads).max(1);
    let mut outs = vec![[0u8; 16]; ids.len()];
    std::thread::scope(|sc| {
        for (ids, outs) in ids.chunks(chunk).zip(outs.chunks_mut(chunk)) {
            sc.spawn(move || {
                for (i, o) in ids.iter().zip(outs) {
                    bc7e_compress_blocks_tuned_on(&q3_candidate(*i), o, p, tune, target);
                }
            });
        }
    });
    outs
}

/// The `q3_candidate` indices and weights whose mode-6 output broke `mode6_indices_are_weighted_nearest` at veryslow on
/// the AVX2 path before patch 0005's Q3, and that stay mode 6 and keep it at basic and veryslow on every path after: the
/// first 8 per weight set of the 32 `search_q3_witnesses` printed (all 64 qualified; Note LR, LR13).
const Q3_WITNESSES: [(u32, [u32; 4]); 16] = [
    (4, [16, 16, 16, 128]),
    (7, [16, 16, 16, 128]),
    (23, [16, 16, 16, 128]),
    (25, [16, 16, 16, 128]),
    (33, [16, 16, 16, 128]),
    (34, [16, 16, 16, 128]),
    (57, [16, 16, 16, 128]),
    (59, [16, 16, 16, 128]),
    (4, [16, 16, 16, 256]),
    (7, [16, 16, 16, 256]),
    (23, [16, 16, 16, 256]),
    (25, [16, 16, 16, 256]),
    (34, [16, 16, 16, 256]),
    (43, [16, 16, 16, 256]),
    (59, [16, 16, 16, 256]),
    (61, [16, 16, 16, 256]),
];

/// Patch 0005, Q3: with linear weights that are not all equal (a user's alpha weight), mode 6's alpha-block selectors
/// were chosen by a projection in the unweighted metric and scored on the two indices around it, so a texel could keep
/// an index the weights rank below another; `evaluate_solution` now scores all sixteen palette entries with the weighted
/// error. Equal weights run the unchanged code, so every level's default bytes stay. Each witness stays mode 6 (a block
/// that left mode 6 proves nothing, so that fails too) and keeps the weighted nearest index on every texel at basic and
/// veryslow on every path (Note LR, LR13).
#[test]
fn mode6_alpha_selectors_follow_the_weights() {
    let mut bad = vec![];
    let mut checked = 0;
    for level in [Bc7Level::Basic, Bc7Level::Veryslow] {
        let p = Bc7Params { level, ..U5 };
        for target in paths() {
            for (i, w) in Q3_WITNESSES {
                let out = q3_outputs(&[i], p, w, target)[0];
                checked += 1;
                if out[0] & 0x7f != 0x40 {
                    bad.push(format!(
                        "witness {i} weights {w:?} {level:?} {target:?} left mode 6 (mode {}): re-run \
                         search_q3_witnesses",
                        out[0].trailing_zeros()
                    ));
                } else if !mode6_indices_are_weighted_nearest(&out, &q3_candidate(i), w) {
                    bad.push(format!("candidate {i} weights {w:?} {level:?} {target:?}"));
                }
            }
        }
    }
    println!(
        "checked {checked} witness encodes (16 witnesses, 2 levels, {} paths)",
        paths().len()
    );
    assert!(
        bad.is_empty(),
        "mode-6 texels not at the weighted nearest index:\n{}",
        bad.join("\n")
    );
}

/// Patch 0005, DG: four divisions are now guarded: `evaluate_solution`'s linear `f`, opaque and alpha, whose denominator
/// is 0 whenever the trial's decoded endpoints coincide (a flat subset, and some subsets of low spread), and the
/// estimators' `scale`, 0 on a flat subset. The numerator is then 0 too, so the guard gives the index the clamp gave and
/// no byte changes. A unit check of that path, not a RED: an opaque block of (100, 100, 100) with one texel (101, 100, 100), and
/// an alpha block of two solid halves (texel columns 0-1 and 2-3), linear at basic, decode within 1 of the source in
/// every channel on every path (Note LR, LR19).
#[test]
fn flat_subsets_decode_sane() {
    let mut opaque = [[100u8, 100, 100, 255]; 16];
    opaque[5] = [101, 100, 100, 255];
    let alpha: [[u8; 4]; 16] = std::array::from_fn(|t| {
        if t % 4 < 2 {
            [60, 120, 180, 200]
        } else {
            [200, 90, 30, 80]
        }
    });
    let p = Bc7Params {
        level: Bc7Level::Basic,
        ..U5
    };
    let mut bad = vec![];
    for (name, block) in [("opaque", opaque), ("alpha", alpha)] {
        for target in paths() {
            let mut out = [0u8; 16];
            bc7e_compress_blocks_on(&block, &mut out, p, target);
            let worst = decode(&out)
                .iter()
                .zip(block)
                .flat_map(|(d, s)| (0..4).map(move |c| d[c].abs_diff(s[c])))
                .max()
                .unwrap();
            if worst > 1 {
                bad.push(format!("{name} {target:?}: worst {worst}, {out:02x?}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "flat subsets decoded more than 1 off:\n{}",
        bad.join("\n")
    );
}

/// Step A0 of patch 0005's Q3 (Note LR): scans `q3_candidate` at veryslow on the AVX2 path with each alpha-weighted set
/// and prints the candidates whose mode-6 output breaks `mode6_indices_are_weighted_nearest`, up to 32 per set.
#[test]
#[ignore = "search for Q3 witnesses: run with --ignored --nocapture after a kernel change"]
fn search_q3_witnesses() {
    assert!(
        IspcTarget::Avx2.supported(),
        "the search runs on the AVX2 path"
    );
    let p = Bc7Params {
        level: Bc7Level::Veryslow,
        ..U5
    };
    for w in [[16, 16, 16, 128], [16, 16, 16, 256]] {
        let start = std::time::Instant::now();
        let (mut scanned, mut found) = (0u32, 0);
        'scan: for base in (0..1u32 << 20).step_by(1 << 14) {
            let ids: Vec<u32> = (base..base + (1 << 14)).collect();
            for (i, out) in ids.iter().zip(q3_outputs(&ids, p, w, IspcTarget::Avx2)) {
                scanned += 1;
                if !mode6_indices_are_weighted_nearest(&out, &q3_candidate(*i), w) {
                    println!("weights {w:?} candidate {i}");
                    found += 1;
                    if found == 32 {
                        break 'scan;
                    }
                }
            }
        }
        println!(
            "weights {w:?}: scanned {scanned}, found {found}, in {:.1} s",
            start.elapsed().as_secs_f64()
        );
    }
}

/// `g_bc7_partition3` in `bc7e.ispc`, copied: the subset (0, 1 or 2) of each texel of BC7's 64 three-subset partitions
/// (mode 0 reads the first 16).
const PARTITION3: [[u8; 16]; 64] = [
    [0, 0, 1, 1, 0, 0, 1, 1, 0, 2, 2, 1, 2, 2, 2, 2],
    [0, 0, 0, 1, 0, 0, 1, 1, 2, 2, 1, 1, 2, 2, 2, 1],
    [0, 0, 0, 0, 2, 0, 0, 1, 2, 2, 1, 1, 2, 2, 1, 1],
    [0, 2, 2, 2, 0, 0, 2, 2, 0, 0, 1, 1, 0, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 2, 1, 1, 2, 2],
    [0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 2, 2, 0, 0, 2, 2],
    [0, 0, 2, 2, 0, 0, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1],
    [0, 0, 1, 1, 0, 0, 1, 1, 2, 2, 1, 1, 2, 2, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2],
    [0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2],
    [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2],
    [0, 0, 1, 2, 0, 0, 1, 2, 0, 0, 1, 2, 0, 0, 1, 2],
    [0, 1, 1, 2, 0, 1, 1, 2, 0, 1, 1, 2, 0, 1, 1, 2],
    [0, 1, 2, 2, 0, 1, 2, 2, 0, 1, 2, 2, 0, 1, 2, 2],
    [0, 0, 1, 1, 0, 1, 1, 2, 1, 1, 2, 2, 1, 2, 2, 2],
    [0, 0, 1, 1, 2, 0, 0, 1, 2, 2, 0, 0, 2, 2, 2, 0],
    [0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 2, 1, 1, 2, 2],
    [0, 1, 1, 1, 0, 0, 1, 1, 2, 0, 0, 1, 2, 2, 0, 0],
    [0, 0, 0, 0, 1, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2, 2],
    [0, 0, 2, 2, 0, 0, 2, 2, 0, 0, 2, 2, 1, 1, 1, 1],
    [0, 1, 1, 1, 0, 1, 1, 1, 0, 2, 2, 2, 0, 2, 2, 2],
    [0, 0, 0, 1, 0, 0, 0, 1, 2, 2, 2, 1, 2, 2, 2, 1],
    [0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 2, 2, 0, 1, 2, 2],
    [0, 0, 0, 0, 1, 1, 0, 0, 2, 2, 1, 0, 2, 2, 1, 0],
    [0, 1, 2, 2, 0, 1, 2, 2, 0, 0, 1, 1, 0, 0, 0, 0],
    [0, 0, 1, 2, 0, 0, 1, 2, 1, 1, 2, 2, 2, 2, 2, 2],
    [0, 1, 1, 0, 1, 2, 2, 1, 1, 2, 2, 1, 0, 1, 1, 0],
    [0, 0, 0, 0, 0, 1, 1, 0, 1, 2, 2, 1, 1, 2, 2, 1],
    [0, 0, 2, 2, 1, 1, 0, 2, 1, 1, 0, 2, 0, 0, 2, 2],
    [0, 1, 1, 0, 0, 1, 1, 0, 2, 0, 0, 2, 2, 2, 2, 2],
    [0, 0, 1, 1, 0, 1, 2, 2, 0, 1, 2, 2, 0, 0, 1, 1],
    [0, 0, 0, 0, 2, 0, 0, 0, 2, 2, 1, 1, 2, 2, 2, 1],
    [0, 0, 0, 0, 0, 0, 0, 2, 1, 1, 2, 2, 1, 2, 2, 2],
    [0, 2, 2, 2, 0, 0, 2, 2, 0, 0, 1, 2, 0, 0, 1, 1],
    [0, 0, 1, 1, 0, 0, 1, 2, 0, 0, 2, 2, 0, 2, 2, 2],
    [0, 1, 2, 0, 0, 1, 2, 0, 0, 1, 2, 0, 0, 1, 2, 0],
    [0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 0, 0, 0, 0],
    [0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 2, 0],
    [0, 1, 2, 0, 2, 0, 1, 2, 1, 2, 0, 1, 0, 1, 2, 0],
    [0, 0, 1, 1, 2, 2, 0, 0, 1, 1, 2, 2, 0, 0, 1, 1],
    [0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 0, 0, 0, 0, 1, 1],
    [0, 1, 0, 1, 0, 1, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 2, 1, 2, 1, 2, 1],
    [0, 0, 2, 2, 1, 1, 2, 2, 0, 0, 2, 2, 1, 1, 2, 2],
    [0, 0, 2, 2, 0, 0, 1, 1, 0, 0, 2, 2, 0, 0, 1, 1],
    [0, 2, 2, 0, 1, 2, 2, 1, 0, 2, 2, 0, 1, 2, 2, 1],
    [0, 1, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2, 0, 1, 0, 1],
    [0, 0, 0, 0, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1],
    [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 2, 2, 2, 2],
    [0, 2, 2, 2, 0, 1, 1, 1, 0, 2, 2, 2, 0, 1, 1, 1],
    [0, 0, 0, 2, 1, 1, 1, 2, 0, 0, 0, 2, 1, 1, 1, 2],
    [0, 0, 0, 0, 2, 1, 1, 2, 2, 1, 1, 2, 2, 1, 1, 2],
    [0, 2, 2, 2, 0, 1, 1, 1, 0, 1, 1, 1, 0, 2, 2, 2],
    [0, 0, 0, 2, 1, 1, 1, 2, 1, 1, 1, 2, 0, 0, 0, 2],
    [0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 0, 2, 2, 2, 2],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 1, 2, 2, 1, 1, 2],
    [0, 1, 1, 0, 0, 1, 1, 0, 2, 2, 2, 2, 2, 2, 2, 2],
    [0, 0, 2, 2, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 2, 2],
    [0, 0, 2, 2, 1, 1, 2, 2, 1, 1, 2, 2, 0, 0, 2, 2],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 1, 2],
    [0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 1],
    [0, 2, 2, 2, 1, 2, 2, 2, 0, 2, 2, 2, 1, 2, 2, 2],
    [0, 1, 0, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2],
    [0, 1, 1, 1, 2, 0, 1, 1, 2, 2, 0, 1, 2, 2, 2, 0],
];

/// The subset of each texel of a BC7 block, from its mode and partition bits (one subset for modes 4, 5 and 6).
fn block_subsets(block: &[u8; 16]) -> [u8; 16] {
    let v = u128::from_le_bytes(*block);
    let part = |at: u32| ((v >> at) & 0x3f) as usize;
    match block[0].trailing_zeros() {
        0 => PARTITION3[((v >> 1) & 0xf) as usize],
        1 => PARTITION2[part(2)],
        2 => PARTITION3[part(3)],
        3 => PARTITION2[part(4)],
        7 => PARTITION2[part(8)],
        _ => [0; 16],
    }
}

/// One block encoded alone with `tune` (so its bytes do not depend on gang neighbours), raw (no post-fix).
fn tuned_block(
    block: &[[u8; 4]; 16],
    p: Bc7Params,
    tune: encoders_sys::Bc7Tune,
    target: IspcTarget,
) -> [u8; 16] {
    let mut out = [0u8; 16];
    encoders_sys::bc7e_compress_blocks_tuned_on(block, &mut out, p, tune, target);
    out
}

/// `Bc7Tune` with only the exact-constants mask set.
fn guard(mask: u8) -> encoders_sys::Bc7Tune {
    encoders_sys::Bc7Tune {
        exact_constants: Some(mask),
        ..encoders_sys::Bc7Tune::default()
    }
}

/// The channel names of the guard's messages.
const CHANNEL: [char; 3] = ['R', 'G', 'B'];

/// The channel guard's order below A (owner, BC7-O26), as (higher, lower) pairs of R (0), G (1) and B (2).
const PRIORITY_PAIRS: [(usize, usize); 3] = [(1, 2), (1, 0), (2, 0)];

/// The levels with no p-bit-free opaque mode (R5 survey 2): every mode they try for an opaque block has p-bits, so a
/// subset whose two guarded channels are 0 and 255 cannot keep both.
const PBIT_LEVELS: [Bc7Level; 3] = [Bc7Level::Ultrafast, Bc7Level::Veryfast, Bc7Level::Fast];

/// The conflict blocks: channel `hi` at `hv` and channel `lo` at `255 - hv` on every texel, alpha `a`, and the third
/// channel 128 (`solid`: bc7e's mode-5 solid-block path, which keeps every channel) or a ramp 0 to 240 in texel order
/// (`ramp`).
fn conflict_blocks(hi: usize, lo: usize, hv: u8, a: u8) -> [(&'static str, [[u8; 4]; 16]); 2] {
    let other = 3 - hi - lo;
    let block = |third: fn(usize) -> u8| -> [[u8; 4]; 16] {
        std::array::from_fn(|t| {
            let mut px = [0, 0, 0, a];
            px[hi] = hv;
            px[lo] = 255 - hv;
            px[other] = third(t);
            px
        })
    };
    [("solid", block(|_| 128)), ("ramp", block(|t| 16 * t as u8))]
}

/// The partition of the across-subsets case, `PARTITION3[0]` (subset 0 is texels 0, 1, 4, 5 and 8), which modes 0 and
/// 2 can both use.
const ACROSS_PARTITION: usize = 0;

/// The across-subsets block: channel `lo` at `255 - hv` on every texel (flat in all three subsets of
/// `ACROSS_PARTITION`), channel `hi` at `hv` in subset 0 and a ramp in subsets 1 and 2, the third channel a ramp.
fn across_block(hi: usize, lo: usize, hv: u8) -> [[u8; 4]; 16] {
    let part = PARTITION3[ACROSS_PARTITION];
    std::array::from_fn(|t| {
        let mut px = [0, 0, 0, 255];
        px[lo] = 255 - hv;
        px[hi] = if part[t] == 0 { hv } else { 40 + 12 * t as u8 };
        px[3 - hi - lo] = 200 - 10 * t as u8;
        px
    })
}

/// Patch 0005, XC priority: when two guarded channels are flat at 0 and 255 in one subset and no solution keeps both,
/// the higher one of G > B > R decodes exactly (spec D7). At ultrafast, veryfast and fast every opaque mode has p-bits,
/// and 0 needs p-bit 0 where 255 needs 1, so every solution loses one of the two. Each pair, both ways round (higher
/// 255 and lower 0, then higher 0 and lower 255, which passed before the priority too: the first p tried is 0), on the
/// conflict blocks, every path. The ramp blocks are mode 6; a solid block takes bc7e's mode-5 solid path, which keeps
/// both, and must stay on it (a pin, not a RED). Then a guard at basic and veryslow: `across_block`'s higher channel
/// decodes exactly in its subset. Those outputs are mode 4 (one subset, in which the higher channel is not flat) except
/// higher 0 at veryslow (mode 2 on `ACROSS_PARTITION`, no p-bits), so no conflict is decided there: the order across
/// subsets rests on the penalties' spacing (Note LR, LR14).
#[test]
fn guarded_channels_conflict_by_priority() {
    let mut bad = vec![];
    let mut seen = vec![];
    for (hi, lo) in PRIORITY_PAIRS {
        let tune = guard((1 << hi) | (1 << lo));
        for hv in [255u8, 0] {
            let what = format!("{} {hv} over {} {}", CHANNEL[hi], CHANNEL[lo], 255 - hv);
            for (name, block) in conflict_blocks(hi, lo, hv, 255) {
                for level in PBIT_LEVELS {
                    let p = Bc7Params { level, ..U5 };
                    for target in paths() {
                        let out = tuned_block(&block, p, tune, target);
                        let mode = out[0].trailing_zeros();
                        let off = decode(&out).iter().filter(|d| d[hi] != hv).count();
                        let cell = format!("{what}: {name} {level:?} {target:?}: mode {mode}");
                        if name == "solid" && mode != 5 {
                            bad.push(format!("{cell}: left the solid-block path"));
                        }
                        if off > 0 {
                            bad.push(format!("{cell}: {off} texels off"));
                        }
                        seen.push(cell);
                    }
                }
            }
            let block = across_block(hi, lo, hv);
            let part = PARTITION3[ACROSS_PARTITION];
            for level in [Bc7Level::Basic, Bc7Level::Veryslow] {
                let p = Bc7Params { level, ..U5 };
                for target in paths() {
                    let out = tuned_block(&block, p, tune, target);
                    let mode = out[0].trailing_zeros();
                    let off = decode(&out)
                        .iter()
                        .zip(part)
                        .filter(|(d, s)| *s == 0 && d[hi] != hv)
                        .count();
                    let cell = format!(
                        "{what}: across {level:?} {target:?}: mode {mode} subsets {:?}",
                        block_subsets(&out)
                    );
                    if off > 0 {
                        bad.push(format!("{cell}: {off} texels off in subset 0"));
                    }
                    seen.push(cell);
                }
            }
        }
    }
    println!("{}", seen.join("\n"));
    assert!(
        bad.is_empty(),
        "the higher guarded channel decoded off:\n{}",
        bad.join("\n")
    );
}

/// Exact opaque keeps alpha 255 above every guarded channel (BC7-C4, BC7-O26: A first): the conflict blocks of
/// `guarded_channels_conflict_by_priority`, opaque, with exact opaque on, decode alpha 255 after the engine's
/// rotated-block post-fix at every level with no p-bit-free opaque mode, on every path. A guard of Task 5c's
/// behaviour, passing before the priority too (Note LR, LR14).
#[test]
fn exact_opaque_outranks_every_guard() {
    use encoders_sys::bc7_fixup::force_opaque_rotated;
    let mut bad = vec![];
    for (hi, lo) in PRIORITY_PAIRS {
        let tune = guard((1 << hi) | (1 << lo));
        for hv in [255u8, 0] {
            for (name, block) in conflict_blocks(hi, lo, hv, 255) {
                for level in PBIT_LEVELS {
                    let p = Bc7Params {
                        level,
                        exact_opaque: true,
                        ..U5
                    };
                    for target in paths() {
                        let mut out = tuned_block(&block, p, tune, target);
                        force_opaque_rotated(&mut out);
                        let below = decode(&out).iter().filter(|d| d[3] != 255).count();
                        if below > 0 {
                            bad.push(format!(
                                "{} {hv} over {} {}: {name} {level:?} {target:?}: {below} texels below 255",
                                CHANNEL[hi],
                                CHANNEL[lo],
                                255 - hv
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "exact-opaque alpha decoded below 255:\n{}",
        bad.join("\n")
    );
}

/// `exact_constant_fixture` generalised (Note LR, LR14): a channel with `Some(v)` is v on every texel, and the other
/// channels follow its cluster rule, drawn in channel order, with each centre's p-bit the other one the flat value
/// needs. Every `Some` holds the same value, 0 or 255. `[None, Some(g), None]` is `exact_constant_fixture(g, seed)`.
fn xc_fixture(seed: u32, flat: [Option<u8>; 3]) -> Vec<[u8; 4]> {
    let v = flat.into_iter().flatten().next().expect("a flat channel");
    assert!(
        flat.into_iter().flatten().all(|x| x == v) && (v == 0 || v == 255),
        "{flat:?}"
    );
    let free: Vec<usize> = (0..3).filter(|&c| flat[c].is_none()).collect();
    let mut s = seed;
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 24) as i32
    };
    let centre = |d: i32| {
        if v == 0 {
            255 - 2 * (d % 16)
        } else {
            2 * (d % 16)
        }
    };
    let mut px = vec![[v, v, v, 255]; 64 * 64];
    for k in 0..256 {
        let (bx, by) = (k % 16 * 4, k / 16 * 4);
        let kind = k % 4;
        let c: [Vec<i32>; 2] =
            std::array::from_fn(|_| free.iter().map(|_| centre(next())).collect());
        for t in 0..16 {
            let (x, y) = (t % 4, t / 4);
            let vals: Vec<i32> = match kind {
                1 if x < 2 => c[0].clone(),
                0 | 1 => c[x / 2].iter().map(|m| m + next() % 5 - 2).collect(),
                2 => {
                    let up = i32::from(next() % 3 == 0);
                    c[0].iter().map(|m| m + up).collect()
                }
                _ => c[0].clone(),
            };
            let p = &mut px[(by + y) * 64 + bx + x];
            for (&ch, m) in free.iter().zip(vals) {
                p[ch] = m.clamp(0, 255) as u8;
            }
        }
    }
    px
}

/// Every masked channel stays exact when the masked channels agree (Task 5c's rule on several channels): each
/// non-empty mask of R, G and B, its channels all 0 or all 255 on every texel of `xc_fixture` (the rest Task 5c's
/// clusters), at basic and veryslow, T_data at weights x16 and x1, on every path. With no conflict the priority never
/// decides, so this guards Task 5c's behaviour on masks it never ran (Note LR, LR14).
#[test]
fn exact_constants_keep_every_masked_channel() {
    use encoders_sys::{Bc7FlatTolerance, Bc7Tune};
    for g in [0u8, 255] {
        assert!(
            xc_fixture(EXACT_SEED, [None, Some(g), None]) == exact_constant_fixture(g, EXACT_SEED),
            "xc_fixture is not Task 5c's G {g} fixture"
        );
    }
    let mut bad = vec![];
    for mask in 1u8..8 {
        for v in [0u8, 255] {
            let flat: [Option<u8>; 3] =
                std::array::from_fn(|c| ((mask >> c) & 1 == 1).then_some(v));
            let blocks = gather_blocks(&xc_fixture(EXACT_SEED, flat), 64, 64);
            for level in [Bc7Level::Basic, Bc7Level::Veryslow] {
                let p = Bc7Params { level, ..U5 };
                for weights in [Some([16; 4]), None] {
                    let tune = Bc7Tune {
                        weights,
                        flat_tolerance: Some(Bc7FlatTolerance::Data),
                        exact_constants: Some(mask),
                    };
                    for target in paths() {
                        let mut out = vec![0u8; blocks.len()];
                        for (o, b) in out.chunks_mut(64 * 16).zip(blocks.chunks(64 * 16)) {
                            encoders_sys::bc7e_compress_blocks_tuned_on(b, o, p, tune, target);
                        }
                        let off = out
                            .chunks(16)
                            .flat_map(decode)
                            .filter(|d| (0..3).any(|c| (mask >> c) & 1 == 1 && d[c] != v))
                            .count();
                        if off > 0 {
                            bad.push(format!(
                                "mask {mask:03b} at {v} {level:?} weights {weights:?} {target:?}: {off} texels off"
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "a masked channel decoded off:\n{}",
        bad.join("\n")
    );
}

/// Round 6 (A1): a lost channel's penalty is added to the solution's real error, and penalties count subsets, so among
/// solutions that lose the same channels in the same number of subsets the closest wins. R 0 and G 255 on every
/// texel, B a ramp 0 to 240: at ultrafast, veryfast and fast no opaque mode keeps both (p-bits), so every G keeper
/// loses R. Guarding R and G must keep G exactly and write what guarding G alone writes (G's keepers ranked by their
/// real error), unless its output loses R in strictly fewer subsets than G alone's (ruling BC7-C20: at veryfast and
/// fast mode 6, one subset, beats G alone's mode 0 or 1, three or two). With one score for every solution that lost
/// a guarded channel, the first solution evaluated won instead and kept R, losing G. The premise, that the R|G output
/// does not keep both, is asserted first (Note LR, LR14).
#[test]
fn a_lost_channel_keeps_the_closest_solution() {
    let block: [[u8; 4]; 16] = std::array::from_fn(|t| [0, 255, 16 * t as u8, 255]);
    // The subsets of a block's partition in which R decodes off 0 on some texel.
    let r_lost = |out: &[u8; 16]| {
        let (d, sub) = (decode(out), block_subsets(out));
        (0..3u8)
            .filter(|&s| (0..16).any(|t| sub[t] == s && d[t][0] != 0))
            .count()
    };
    let mut bad = vec![];
    for level in PBIT_LEVELS {
        let p = Bc7Params { level, ..U5 };
        for target in paths() {
            let rg = tuned_block(&block, p, guard(0b011), target);
            let g = tuned_block(&block, p, guard(0b010), target);
            let what = format!(
                "{level:?} {target:?}: R|G mode {} R lost in {} subsets, G mode {} in {}",
                rg[0].trailing_zeros(),
                r_lost(&rg),
                g[0].trailing_zeros(),
                r_lost(&g)
            );
            let decoded = decode(&rg);
            if decoded.iter().all(|d| d[0] == 0 && d[1] == 255) {
                bad.push(format!(
                    "{what}: R|G keeps both, so the fixture has a keeper"
                ));
            } else if decoded.iter().any(|d| d[1] != 255) {
                bad.push(format!("{what}: R|G lost G: {rg:02x?}"));
            } else if rg != g && r_lost(&rg) >= r_lost(&g) {
                bad.push(format!("{what}: {rg:02x?} != {g:02x?}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "guarding R and G did not keep G at G alone's bytes or in fewer losing subsets:\n{}",
        bad.join("\n")
    );
}

/// Task 6m2's `witness-A1.txt` blocks (class A1, `a1_block`): block k, the level, the RGB part of the mask it ran with
/// (A was in the mask too, which is exact opaque here) and the mode it took (Note LR, LR14). The ultrafast six take
/// mode 6 with the penalty replacing the error too, and lose the property there; the mode 3 two leave mode 3 then.
const A1_WITNESSES: &[(usize, Bc7Level, u8, u32)] = &[
    (13, Bc7Level::Ultrafast, 0b011, 6),
    (21, Bc7Level::Ultrafast, 0b011, 6),
    (13, Bc7Level::Ultrafast, 0b101, 6),
    (24, Bc7Level::Ultrafast, 0b101, 6),
    (13, Bc7Level::Ultrafast, 0b110, 6),
    (21, Bc7Level::Ultrafast, 0b110, 6),
    (14, Bc7Level::Veryfast, 0b011, 3),
    (18, Bc7Level::Fast, 0b110, 3),
];

/// The channels of `bits` that decode off their source at some texel of `block`, and the number of subsets of the
/// output's partition, summed over those channels, in which they do.
fn lost_in(out: &[u8; 16], block: &[[u8; 4]; 16], bits: u8) -> (u8, usize) {
    let (d, sub) = (decode(out), block_subsets(out));
    let mut lost = 0;
    let mut subsets = 0;
    for c in (0..3).filter(|c| (bits >> c) & 1 == 1) {
        let off: Vec<u8> = (0..3u8)
            .filter(|&s| (0..16).any(|t| sub[t] == s && d[t][c] != block[t][c]))
            .collect();
        if !off.is_empty() {
            lost |= 1 << c;
            subsets += off.len();
        }
    }
    (lost, subsets)
}

/// Round 6 (A1) on Task 6m2's witnesses: with exact opaque on and a mask whose flat channels cannot all be kept, the
/// output loses some (the premise) and must write the bytes of the same mask without the lost channels, where every
/// solution is ranked by its real error among those that keep the rest, unless it loses them in strictly fewer subsets.
/// With the penalty replacing the error (Task 6m2's `xA1` arm) the first solution evaluated won instead, and this test
/// fails. Each witness must still take the mode it took there, so a later kernel cannot pass it by leaving that path
/// (Note LR, LR14).
#[test]
fn a1_witnesses_pick_the_closest_loser() {
    let mut bad = vec![];
    for &(k, level, mask, mode) in A1_WITNESSES {
        let block = a1_block(k);
        let flat: u8 = (0..3)
            .filter(|&c| (mask >> c) & 1 == 1 && block.iter().all(|px| px[c] == block[0][c]))
            .map(|c| 1u8 << c)
            .sum();
        let p = Bc7Params {
            level,
            exact_opaque: true,
            ..U5
        };
        for target in paths() {
            let out = tuned_block(&block, p, guard(mask), target);
            let (lost, subsets) = lost_in(&out, &block, flat);
            let what = format!("witness {k} {level:?} mask {mask:03b} {target:?}: lost {lost:03b}");
            if out[0].trailing_zeros() != mode {
                bad.push(format!(
                    "{what}: left mode {mode} (mode {}): re-pick from witness-A1.txt",
                    out[0].trailing_zeros()
                ));
            } else if lost == 0 {
                bad.push(format!(
                    "{what}: keeps every flat channel, so it has no loser to pick"
                ));
            } else {
                let reduced = tuned_block(&block, p, guard(mask & !lost), target);
                let (_, reduced_subsets) = lost_in(&reduced, &block, lost);
                if out != reduced && subsets >= reduced_subsets {
                    bad.push(format!(
                        "{what} in {subsets} subsets, without them {reduced_subsets}: {out:02x?} != {reduced:02x?}"
                    ));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "a witness did not pick the closest loser:\n{}",
        bad.join("\n")
    );
}

/// The draw of Task 6m2's synthetic-class generator (copied; the seeds and rules are in the research folder's
/// `gen6.txt`): an LCG `x = 6364136223846793005 x + 1442695040888963407` (mod 2^64), `u(n) = (x >> 33) % n`. A class's
/// block k is its rule's (k + 1)th output from the class's seed.
fn gen6_u(x: &mut u64, n: u64) -> i32 {
    *x = x
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*x >> 33) % n) as i32
}

/// Block `k` of Task 6m2's synthetic class A1 (`gen6_u`, seed 0x6A2A1): the ramped channel rc = u(3), then each of R, G
/// and B flat at u(2) * 255, then a ramp v0 + s * t (v0 = u(200), s = 1 + u(4), t the texel, capped at 255) replaces
/// channel rc; alpha 255. So two channels are 0 or 255 throughout.
fn a1_block(k: usize) -> [[u8; 4]; 16] {
    let mut x = 0x6A2A1u64;
    let mut u = move |n: u64| gen6_u(&mut x, n);
    let mut block = [[0u8; 4]; 16];
    for _ in 0..=k {
        let rc = u(3) as usize;
        let flat = [u(2) * 255, u(2) * 255, u(2) * 255];
        let (v0, s) = (u(200), 1 + u(4));
        for (t, px) in block.iter_mut().enumerate() {
            let mut v = flat;
            v[rc] = (v0 + s * t as i32).min(255);
            *px = [v[0] as u8, v[1] as u8, v[2] as u8, 255];
        }
    }
    block
}

/// Block `k` of Task 6m2's synthetic class A3c (`gen6_u`, seed 0x6A2A5). Texel columns 0-1 are near-solid with a
/// near-255 G: R = r0 + u(5) - 2 (r0 = 60 + u(120)), G = 255 - u(3), B = 255 - u(4); columns 2-3 are textured:
/// R = r0 + u(32), G = 200 + u(56), B = 200 + u(56); alpha 255.
fn a3c_block(k: usize) -> [[u8; 4]; 16] {
    let mut x = 0x6A2A5u64;
    let mut u = move |n: u64| gen6_u(&mut x, n);
    let mut block = [[0u8; 4]; 16];
    for _ in 0..=k {
        let r0 = 60 + u(120);
        for (t, px) in block.iter_mut().enumerate() {
            *px = if t % 4 < 2 {
                [(r0 + u(5) - 2).clamp(0, 255), 255 - u(3), 255 - u(4), 255]
            } else {
                [(r0 + u(32)).min(255), 200 + u(56), 200 + u(56), 255]
            }
            .map(|c| c as u8);
        }
    }
    block
}

/// Whether channel `c` of `block` is 0 on every texel, or 255 on every texel, of some subset of `subsets`.
fn flagged_in(block: &[[u8; 4]; 16], subsets: &[u8; 16], c: usize) -> bool {
    (0..3u8).any(|s| {
        let vals: Vec<u8> = (0..16)
            .filter(|&t| subsets[t] == s)
            .map(|t| block[t][c])
            .collect();
        !vals.is_empty() && (vals.iter().all(|&v| v == 0) || vals.iter().all(|&v| v == 255))
    })
}

/// The A3c blocks of Task 6m2's `witness-A3.txt` (index k = image x 4096 + block) with the level at which the guard
/// keyed on the packed value moved them on AVX2: the 8 of its 12 that, each encoded alone, failed the test below on
/// every path before Round 6 (A3) and pass after. Of the others, 20193 leaves the mean-colour path on SSE2 at veryslow
/// and slowest, and 13371 and 13629 (found on AVX-512) pass on SSE2 and AVX2 either way (Note LR, LR14).
const PACKER_FLAG_WITNESSES: &[(usize, Bc7Level)] = &[
    (160, Bc7Level::Veryfast),
    (330, Bc7Level::Veryfast),
    (558, Bc7Level::Basic),
    (2356, Bc7Level::Basic),
    (558, Bc7Level::Slow),
    (2974, Bc7Level::Slow),
    (38501, Bc7Level::Veryslow),
    (3228, Bc7Level::Slowest),
];

/// Round 6 (A3): the one-colour packers charge a guarded channel only where the subset's source holds it constant, not
/// where the packed value is 0 or 255. Where A3c's near-255 G is constant in no subset of the unguarded output's
/// partition (asserted), that solution pays no penalty and every other one pays at least its own unguarded score, so
/// guarding G must write the unguarded bytes (x16 weights, as Task 6m2 ran them). Keyed on the packed value, the mean
/// colour's G rounded to 255 was forced and the subset decoded farther from its source. Each witness's unguarded
/// output must still be mode 0 with a subset packed as one colour from a source that is not one colour (the mean-colour
/// candidate), so a later kernel cannot pass it by leaving that path (Note LR, LR14).
#[test]
fn one_colour_packers_key_on_source_constants() {
    let tune = |mask| encoders_sys::Bc7Tune {
        weights: Some([16; 4]),
        exact_constants: Some(mask),
        ..encoders_sys::Bc7Tune::default()
    };
    let mut bad = vec![];
    for &(k, level) in PACKER_FLAG_WITNESSES {
        let block = a3c_block(k);
        let p = Bc7Params { level, ..U5 };
        for target in paths() {
            let guarded = tuned_block(&block, p, tune(0b010), target);
            let plain = tuned_block(&block, p, tune(0), target);
            let what = format!("witness {k} {level:?} {target:?}");
            // The premise and the path are read from the unguarded bytes, which the guard's rule cannot move.
            let decoded = decode(&plain);
            let subsets = block_subsets(&plain);
            if flagged_in(&block, &subsets, 1) {
                bad.push(format!(
                    "{what}: G is 0 or 255 throughout a subset of the unguarded output"
                ));
                continue;
            }
            let mean_packed = (0..3u8).any(|s| {
                let ts: Vec<usize> = (0..16).filter(|&t| subsets[t] == s).collect();
                !ts.is_empty()
                    && ts.iter().all(|&t| decoded[t] == decoded[ts[0]])
                    && ts.iter().any(|&t| block[t] != block[ts[0]])
            });
            if plain[0] & 1 == 0 || !mean_packed {
                bad.push(format!(
                    "{what}: left the mean-colour path (mode {}): re-pick from witness-A3.txt",
                    plain[0].trailing_zeros()
                ));
            } else if guarded != plain {
                bad.push(format!(
                    "{what}: guarded {guarded:02x?} != plain {plain:02x?}"
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "guarding an unflaggable G changed the bytes:\n{}",
        bad.join("\n")
    );
}

/// The seed of `flat_alpha_blocks`.
const FLAT_ALPHA_SEED: u32 = 0xA17A_2026;

/// 256 blocks (16 texels each, in block order) whose alpha is `a` on every texel, from an LCG seeded with `seed`
/// (`s = s * 1664525 + 1013904223`, each draw `s >> 24`): even blocks a ramp along the texel order between two random
/// colours, odd blocks random colour noise. With `g = Some(v)`, G is v on every texel.
fn flat_alpha_blocks(seed: u32, a: u8, g: Option<u8>) -> Vec<[u8; 4]> {
    let mut s = seed;
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 24) as i32
    };
    let mut px = Vec::with_capacity(256 * 16);
    for k in 0..256 {
        let ends = [[next(), next(), next()], [next(), next(), next()]];
        for t in 0..16i32 {
            let mut c: [u8; 4] = if k % 2 == 0 {
                std::array::from_fn(|c| {
                    if c == 3 {
                        a
                    } else {
                        (ends[0][c] + (ends[1][c] - ends[0][c]) * t / 15) as u8
                    }
                })
            } else {
                [next() as u8, next() as u8, next() as u8, a]
            };
            if let Some(v) = g {
                c[1] = v;
            }
            px.push(c);
        }
    }
    px
}

/// `blocks` encoded with `tune` in 64-block calls, as the engine makes them, raw (no post-fix).
fn tuned_on(
    blocks: &[[u8; 4]],
    p: Bc7Params,
    tune: encoders_sys::Bc7Tune,
    target: IspcTarget,
) -> Vec<u8> {
    // 16 texels in, 16 bytes out per block: 64 blocks are 64 * 16 texels and 64 * 16 output bytes.
    let n = blocks.len() / 16;
    let mut out = vec![0u8; n * 16];
    for (o, b) in out.chunks_mut(64 * 16).zip(blocks.chunks(64 * 16)) {
        encoders_sys::bc7e_compress_blocks_tuned_on(b, o, p, tune, target);
    }
    out
}

/// The texels of `out` whose decoded channel `c` is not `v`, and the blocks that hold them.
fn off_count(out: &[u8], c: usize, v: u8) -> (usize, usize) {
    out.chunks(16).fold((0, 0), |(texels, blocks), b| {
        let n = decode(b).iter().filter(|d| d[c] != v).count();
        (texels + n, blocks + usize::from(n > 0))
    })
}

/// Patch 0005, XC (A): exact opaque is the channel guard's alpha, bit 3 of the mask, so in an alpha block a subset whose
/// alpha is 0 on every texel decodes it exactly wherever the kernel finds such a solution, above G, B and R. On blocks
/// whose alpha is 0 throughout (`flat_alpha_blocks`' ramps and noise) mode 4 keeps alpha 0 exactly and a rotated trial
/// that loses it pays A's tier (A2), so every texel decodes alpha 0 at every level, linear and perceptual, raw, on every
/// path. Before, exact opaque guarded opaque blocks only, and these blocks decoded alpha above 0 at every level. Then the
/// same blocks with G 255 throughout and G guarded at ultrafast, veryfast and fast: alpha 0 still decodes exactly (A
/// over G; before, guarding G lost alpha 0 in 57 to 94 of the 256 blocks) (Note LR, LR17).
#[test]
fn channel_guard_a_keeps_flat_alpha() {
    let mut bad = vec![];
    let blocks = flat_alpha_blocks(FLAT_ALPHA_SEED, 0, None);
    let with_g = flat_alpha_blocks(FLAT_ALPHA_SEED, 0, Some(255));
    for level in LEVELS {
        for perceptual in [false, true] {
            let p = Bc7Params {
                level,
                perceptual,
                exact_opaque: true,
            };
            for target in paths() {
                let (texels, n) = off_count(&raw_on(&blocks, p, target), 3, 0);
                if texels > 0 {
                    bad.push(format!(
                        "{level:?} perceptual={perceptual} {target:?}: {texels} texels in {n} blocks"
                    ));
                }
                if PBIT_LEVELS.contains(&level) {
                    let out = tuned_on(&with_g, p, guard(0b010), target);
                    let (texels, n) = off_count(&out, 3, 0);
                    if texels > 0 {
                        bad.push(format!(
                            "G 255 guarded {level:?} perceptual={perceptual} {target:?}: {texels} texels in {n} \
                             blocks"
                        ));
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "alpha 0 throughout decoded off 0:\n{}",
        bad.join("\n")
    );
}

/// Exact opaque keeps alpha 255 on opaque blocks at every level, linear and perceptual, raw, on every path:
/// `flat_alpha_blocks` at alpha 255 (these blocks need no rotated-block post-fix). Bit 3 drives the opaque handlers'
/// forced mode-6 p-bits as the flag did, so the property holds before and after patch 0005 (A); that opaque bytes are
/// unchanged was checked by the pins when A was added (Note LR, LR17).
#[test]
fn exact_opaque_keeps_opaque_alpha_255() {
    let blocks = flat_alpha_blocks(FLAT_ALPHA_SEED, 255, None);
    let mut bad = vec![];
    for level in LEVELS {
        for perceptual in [false, true] {
            let p = Bc7Params {
                level,
                perceptual,
                exact_opaque: true,
            };
            for target in paths() {
                let (texels, n) = off_count(&raw_on(&blocks, p, target), 3, 255);
                if texels > 0 {
                    bad.push(format!(
                        "{level:?} perceptual={perceptual} {target:?}: {texels} texels in {n} blocks"
                    ));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "exact-opaque alpha decoded below 255:\n{}",
        bad.join("\n")
    );
}

/// Round 6 (A2), a guard: a rotated mode 4/5 trial of an alpha block holds source alpha in a colour slot and charges
/// its loss at A's tier, so a guarded G cannot outrank it there. Alpha x16 linear (weights 16, 16, 16, 256), alpha 0
/// and G 255 on every texel, R and B noise, exact opaque on with G guarded, at basic and veryslow (rotations on): every
/// texel decodes alpha 0 and G 255, raw, on every path. Task 6m2 found no block where A2 alone changes the output
/// (`witness-A2.txt`: 0 of 65,536 at every level), so this holds without A2 too (Note LR, LR17).
#[test]
fn a_outranks_g_in_rotated_trials() {
    let mut s = FLAT_ALPHA_SEED ^ 0x0A02;
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 24) as u8
    };
    let blocks: Vec<[u8; 4]> = (0..256 * 16).map(|_| [next(), 255, next(), 0]).collect();
    let tune = encoders_sys::Bc7Tune {
        weights: Some([16, 16, 16, 256]),
        exact_constants: Some(0b010),
        ..encoders_sys::Bc7Tune::default()
    };
    let mut bad = vec![];
    for level in [Bc7Level::Basic, Bc7Level::Veryslow] {
        let p = Bc7Params {
            level,
            exact_opaque: true,
            ..U5
        };
        for target in paths() {
            let out = tuned_on(&blocks, p, tune, target);
            for (c, v) in [(3, 0), (1, 255)] {
                let (texels, n) = off_count(&out, c, v);
                if texels > 0 {
                    bad.push(format!(
                        "{level:?} {target:?}: channel {c} off {v} on {texels} texels in {n} blocks"
                    ));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "alpha 0 or guarded G 255 decoded off:\n{}",
        bad.join("\n")
    );
}

/// Block `k` of Task 6m2's synthetic class wp (`gen6_u`, seed 0x6A2B1): texel columns 0-1 (subset 0 of partition 0 in
/// modes 1, 3 and 7) solid at (u(4), u(4), u(4), 1 + u(3)); columns 2-3 an RGBA ramp, each channel in turn v0 + s * t
/// (v0 = u(200), s = 1 + u(4), t the texel, capped at 255).
fn wp_block(k: usize) -> [[u8; 4]; 16] {
    let mut x = 0x6A2B1u64;
    let mut u = move |n: u64| gen6_u(&mut x, n);
    let mut block = [[0u8; 4]; 16];
    for _ in 0..=k {
        let solid = [u(4), u(4), u(4), 1 + u(3)];
        let mut ramps = [(0, 0); 4];
        for r in &mut ramps {
            *r = (u(200), 1 + u(4));
        }
        for (t, px) in block.iter_mut().enumerate() {
            for c in 0..4 {
                px[c] = if t % 4 < 2 {
                    solid[c]
                } else {
                    (ramps[c].0 + ramps[c].1 * t as i32).min(255)
                } as u8;
            }
        }
    }
    block
}

/// The smallest squared error bc7e's mode-7 one-colour table holds for value `v` at p-bit pair `p` (bit 1 the high
/// endpoint's p-bit, bit 0 the low's), built as the kernel builds it: every pair of 5-bit endpoints with those p-bits,
/// expanded to 8 bits, interpolated at the table's selector, index 1 (weight 21 of 64).
fn mode7_one_colour_error(v: u8, p: u32) -> u32 {
    let expand = |e: u32, pbit: u32| {
        let x = ((e << 1) | pbit) << 2;
        x | (x >> 6)
    };
    (0..32u32)
        .flat_map(|l| (0..32u32).map(move |h| (l, h)))
        .map(|(l, h)| {
            let k = (expand(l, p & 1) * 43 + expand(h, p >> 1) * 21 + 32) >> 6;
            k.abs_diff(u32::from(v)).pow(2)
        })
        .min()
        .unwrap()
}

/// The smallest weighted error `sum w[c] * e[c]` that colour `col` packed by mode 7's one-colour table reaches over the
/// four p-bit pairs: what the packer's p choice gives when it follows the weights.
fn mode7_one_colour_best(col: [u8; 4], w: [u32; 4]) -> u32 {
    (0..4)
        .map(|p| {
            (0..4)
                .map(|c| w[c] * mode7_one_colour_error(col[c], p))
                .sum()
        })
        .min()
        .unwrap()
}

/// `sum w[c] * (d[c] - s[c])^2` for one texel.
fn weighted_error(d: [u8; 4], s: [u8; 4], w: [u32; 4]) -> u32 {
    (0..4)
        .map(|c| w[c] * u32::from(d[c].abs_diff(s[c])).pow(2))
        .sum()
}

/// Linear weights with alpha at 64 times the colour (an `alpha64` texture's weights at x1).
const WP_WEIGHTS: [u32; 4] = [1, 1, 1, 64];

/// The wp blocks (`wp_block`) whose output at basic with `WP_WEIGHTS`, each encoded alone, is mode 7 with the solid
/// columns as subset 0 (partition 0), and whose solid texels decoded above the weighted best of the four p-bit pairs
/// before patch 0005's WP, on every path: the first 8 such blocks of the class (3,159 of its 16,384 on AVX2; the same
/// first 8 at x16, `[16, 16, 16, 1024]`). Their solid alpha is 1 or 3 (Note LR, LR20).
const WP_MODE7_WITNESSES: &[usize] = &[2, 6, 19, 21, 22, 24, 30, 37];

/// Patch 0005, WP: with linear weights that are not all equal, mode 7's one-colour packer chose its p-bits by the
/// unweighted sum of the table entries' errors and weighed only the chosen pair, so with alpha at 64 times the colour
/// the packer's pick for a solid subset at (0, 0, 0, 1) decoded (0, 0, 0, 0), weighted error 64 per texel, where p =
/// 2's (1, 1, 1, 1) costs 3; witness 37's solid (0, 1, 0, 1) decoded (0, 0, 0, 0) at 65 against a best pair's 2.
/// The packer now picks the pair with the smallest weighted error. Each witness must still be mode 7 with the solid
/// columns as its own subset (a block that left that path proves nothing, so that fails too), and every solid texel
/// decodes at weighted error at most the best p-bit pair's (`mode7_one_colour_best`), at basic on every path. Mode 6's
/// packer got the same change, but no block was found where it shows, so it has no witness here (Note LR, LR20).
#[test]
fn one_colour_p_bits_follow_the_weights() {
    let tune = encoders_sys::Bc7Tune {
        weights: Some(WP_WEIGHTS),
        ..encoders_sys::Bc7Tune::default()
    };
    let p = Bc7Params {
        level: Bc7Level::Basic,
        ..U5
    };
    let mut bad = vec![];
    for &k in WP_MODE7_WITNESSES {
        let block = wp_block(k);
        let best = mode7_one_colour_best(block[0], WP_WEIGHTS);
        for target in paths() {
            let out = tuned_block(&block, p, tune, target);
            let what = format!("wp block {k} {target:?}");
            if out[0].trailing_zeros() != 7 || block_subsets(&out) != PARTITION2[0] {
                bad.push(format!(
                    "{what}: left mode 7 partition 0 (mode {}): re-pick from the wp class",
                    out[0].trailing_zeros()
                ));
                continue;
            }
            let d = decode(&out);
            let worst = (0..16)
                .filter(|t| t % 4 < 2)
                .map(|t| weighted_error(d[t], block[t], WP_WEIGHTS))
                .max()
                .unwrap();
            if worst > best {
                bad.push(format!(
                    "{what}: solid {:?} decodes {:?}, weighted error {worst}, best p-bit pair {best}",
                    block[0], d[0]
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "solid subsets not at the weighted best p-bit pair:\n{}",
        bad.join("\n")
    );
}

/// BC7's 2-bit and 3-bit index weights (mode 4's palettes).
const BC7_WEIGHTS2: [u32; 4] = [0, 21, 43, 64];
const BC7_WEIGHTS3: [u32; 8] = [0, 9, 18, 27, 37, 46, 55, 64];

/// Patch 0005 (KS)'s tables, built as the kernel builds them: `[width][v]` is the smallest squared error at which mode
/// 4's scalar decodes `v` from a pair of 6-bit codes (expanded as the decoder does) and one selector, with 2-bit indices
/// (`[0]`) or 3-bit (`[1]`). Every v has an exact pair at 3 bits; at 2 bits v = 2, 6, 10, 249 and 253 have none (1 off).
fn ks_best_errors() -> [[u32; 256]; 2] {
    [&BC7_WEIGHTS2[..], &BC7_WEIGHTS3[..]].map(|w| {
        let mut decoded = vec![];
        for l in 0..64u32 {
            let lo = (l << 2) | (l >> 4);
            for h in 0..64u32 {
                let hi = (h << 2) | (h >> 4);
                decoded.extend(w.iter().map(|&x| (lo * (64 - x) + hi * x + 32) >> 6));
            }
        }
        // A value no pair decodes takes the error of the closest decoded value.
        std::array::from_fn(|v| {
            let d = decoded.iter().map(|&k| k.abs_diff(v as u32)).min().unwrap();
            d * d
        })
    })
}

/// The outputs (raw) of one constant-alpha cell: the block indices that are mode 4 at rotation 0 (alpha in the scalar
/// slot), those of them whose alpha decodes farther from `v` than `ks_best_errors` allows for the block's alpha index
/// width (index-mode bit set: 2 bits), and the number of other blocks.
fn constant_alpha_cell(
    out: &[u8],
    v: u8,
    best: &[[u32; 256]; 2],
) -> (Vec<usize>, Vec<usize>, usize) {
    let (mut taken, mut missed, mut other) = (vec![], vec![], 0);
    for (k, b) in out.chunks(16).enumerate() {
        if b[0].trailing_zeros() != 4 || (b[0] >> 5) & 3 != 0 {
            other += 1;
            continue;
        }
        taken.push(k);
        let width = usize::from(b[0] >> 7 == 0);
        if decode(b)
            .iter()
            .any(|d| u32::from(d[3].abs_diff(v)).pow(2) > best[width][usize::from(v)])
        {
            missed.push(k);
        }
    }
    (taken, missed, other)
}

/// `ks_best_errors` pins the tables' claim that LR22 and README-VENDOR repeat: every v decodes exactly at 3-bit
/// indices, and at 2-bit indices every v but 2, 6, 10, 249 and 253, which come 1 off (Note LR, LR22).
#[test]
fn ks_tables_are_exact_but_five_two_bit_values() {
    let best = ks_best_errors();
    let off2: Vec<(usize, u32)> = (0..256)
        .filter(|&v| best[0][v] != 0)
        .map(|v| (v, best[0][v]))
        .collect();
    assert_eq!(off2, [(2, 1), (6, 1), (10, 1), (249, 1), (253, 1)]);
    assert!(best[1].iter().all(|&e| e == 0), "3-bit table not exact");
}

/// The params and tune of one constant-alpha cell: linear weights `[1, 1, 1, 16]`, or the level's perceptual weights.
fn ks_cell_params(level: Bc7Level, perceptual: bool) -> (Bc7Params, encoders_sys::Bc7Tune) {
    let p = Bc7Params {
        level,
        perceptual,
        exact_opaque: false,
    };
    let tune = encoders_sys::Bc7Tune {
        weights: (!perceptual).then_some([1, 1, 1, 16]),
        ..encoders_sys::Bc7Tune::default()
    };
    (p, tune)
}

/// The blocks of `flat_alpha_blocks(FLAT_ALPHA_SEED, 252, None)` (in its 64-block calls) that were mode 4 at rotation 0
/// and decoded alpha off 252 at veryfast and fast, perceptual, on every path before patch 0005's KS, and stay mode 4 at
/// rotation 0 there after it: the first 8 of 14 (all 14 stay; Note LR, LR22).
const KS_WITNESSES: &[usize] = &[19, 29, 47, 69, 99, 101, 131, 171];

/// Patch 0005, KS: a block whose mode-4 scalar is one value v started mode 4's alpha search at la = ha = (v + 2) >> 2,
/// which the refit cannot move (every selector 0), so without the uber grid (ultrafast, veryfast and fast) alpha 252
/// decoded 255, though codes 62 and 63 at index weight 21 decode 252. Mode 4 now also scores the closest code pair from
/// a table. "Exactly" means at the table's best error for the output's index width (`ks_best_errors`: exact but for
/// five values at 2-bit indices). On `flat_alpha_blocks` at alpha 252, at veryfast and fast, raw, on every path, every
/// output that is mode 4 at rotation 0 decodes alpha within that error, and `KS_WITNESSES` must still be mode 4 at
/// rotation 0 in the perceptual cells. The linear cells (weights 1, 1, 1, 16) are counted only: none of their outputs
/// is mode 4 at rotation 0, so they assert nothing today (ruling BC7-C26). Outputs in another mode or rotated are
/// counted, not asserted: KS adds a mode-4 candidate, it does not decide the mode (Note LR, LR22).
#[test]
fn a_constant_alpha_decodes_exactly() {
    let best = ks_best_errors();
    let blocks = flat_alpha_blocks(FLAT_ALPHA_SEED, 252, None);
    let mut bad = vec![];
    for level in [Bc7Level::Veryfast, Bc7Level::Fast] {
        for perceptual in [false, true] {
            let (p, tune) = ks_cell_params(level, perceptual);
            for target in paths() {
                let out = tuned_on(&blocks, p, tune, target);
                let (taken, missed, other) = constant_alpha_cell(&out, 252, &best);
                let what = format!("{level:?} perceptual={perceptual} {target:?}");
                println!("{what}: mode 4 rotation 0 {taken:?}, other {other}; missed {missed:?}");
                if !missed.is_empty() {
                    bad.push(format!("{what}: alpha off 252 in {} blocks", missed.len()));
                }
                if perceptual {
                    for k in KS_WITNESSES.iter().filter(|k| !taken.contains(k)) {
                        bad.push(format!("{what}: witness {k} left mode 4 rotation 0"));
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "constant alpha 252 off its best code pair:\n{}",
        bad.join("\n")
    );
}

/// The fewest blocks of `every_constant_alpha_value_decodes_exactly`'s perceptual cells that must be mode 4 at rotation
/// 0 on each path, so the test cannot pass by leaving that path: 4,123 of 65,024 were with patch 0005's KS (Note LR,
/// LR22).
const KS_PERCEPTUAL_FLOOR: usize = 4_000;

/// Patch 0005, KS, for every constant: `flat_alpha_blocks` at each alpha v from 1 to 254, at veryfast, raw, on every
/// path. Every output that is mode 4 at rotation 0 decodes alpha within the table's best error for its index width
/// ("exactly", as in `a_constant_alpha_decodes_exactly`), and each perceptual cell holds at least `KS_PERCEPTUAL_FLOOR`
/// such outputs. The linear cells (weights 1, 1, 1, 16) are counted only: none of their outputs is mode 4 at rotation
/// 0 (ruling BC7-C26). Each cell prints its counts and the values none of whose blocks is mode 4 at rotation 0 (Note
/// LR, LR22).
#[test]
fn every_constant_alpha_value_decodes_exactly() {
    let best = ks_best_errors();
    let mut bad = vec![];
    for perceptual in [false, true] {
        let (p, tune) = ks_cell_params(Bc7Level::Veryfast, perceptual);
        for target in paths() {
            let (mut taken, mut other, mut missed) = (0, 0, 0);
            let (mut off_values, mut none_taken) = (vec![], vec![]);
            for v in 1..=254u8 {
                let out = tuned_on(
                    &flat_alpha_blocks(FLAT_ALPHA_SEED, v, None),
                    p,
                    tune,
                    target,
                );
                let (t, m, o) = constant_alpha_cell(&out, v, &best);
                taken += t.len();
                other += o;
                missed += m.len();
                if !m.is_empty() {
                    off_values.push(v);
                }
                if t.is_empty() {
                    none_taken.push(v);
                }
            }
            let what = format!("perceptual={perceptual} {target:?}");
            println!(
                "{what}: mode 4 rotation 0 {taken}, other {other}, missed {missed}; {} values with no mode 4 \
                 rotation 0 block: {none_taken:?}",
                none_taken.len()
            );
            if missed > 0 {
                bad.push(format!(
                    "{what}: {missed} blocks off at {} values, first {:?}",
                    off_values.len(),
                    &off_values[..off_values.len().min(12)]
                ));
            }
            if perceptual && taken < KS_PERCEPTUAL_FLOOR {
                bad.push(format!(
                    "{what}: only {taken} blocks mode 4 at rotation 0 (floor {KS_PERCEPTUAL_FLOOR})"
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "constant alpha off its best code pair at veryfast:\n{}",
        bad.join("\n")
    );
}

/// The seed of `ks3_block`.
const KS3_SEED: u32 = 0x3B17_2026;

/// Block `k` (from 0) of a seeded class whose mode-4 outputs use 3-bit alpha indices: an LCG seeded with `KS3_SEED`
/// (`s = s * 1664525 + 1013904223`, each draw `s >> 24`); per block an alpha 1 + d % 254, then two colours whose R,
/// G and B are 5-bit codes expanded as the decoder does, then per texel one of the two (d % 2). Mode 4's colour then
/// needs no more than its 2-bit indices, so the 3-bit alpha half (tried first) keeps the tie.
fn ks3_block(k: usize) -> [[u8; 4]; 16] {
    let mut s = KS3_SEED;
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        s >> 24
    };
    let e5 = |c: u32| ((c << 3) | (c >> 2)) as u8;
    let mut block = [[0u8; 4]; 16];
    for _ in 0..=k {
        let a = 1 + (next() % 254) as u8;
        let cols: [[u8; 3]; 2] = std::array::from_fn(|_| std::array::from_fn(|_| e5(next() % 32)));
        block = std::array::from_fn(|_| {
            let c = cols[(next() % 2) as usize];
            [c[0], c[1], c[2], a]
        });
    }
    block
}

/// The `ks3_block` indices of `three_bit_alpha_takes_its_exact_pair`: blocks 0 to 7.
const KS3_WITNESSES: std::ops::Range<usize> = 0..8;

/// Patch 0005, KS, at 3-bit alpha indices (the `g_ks_mode4_pair3` table; `flat_alpha_blocks` reaches only the 2-bit
/// one): each `ks3_block` witness, encoded alone at ultrafast, veryfast and fast (no uber grid, so KS is the only route
/// to the pair), linear (the plain call and weights 1, 1, 1, 16) and perceptual, raw, on every path, must be mode 4 at
/// rotation 0 with 3-bit alpha indices (index-mode bit clear; a block that left that path proves nothing, so that fails
/// too) and decode its alpha exactly, as every value has an exact pair at 3 bits (Note LR, LR22).
#[test]
fn three_bit_alpha_takes_its_exact_pair() {
    let mut bad = vec![];
    for k in KS3_WITNESSES {
        let block = ks3_block(k);
        let v = block[0][3];
        for level in [Bc7Level::Ultrafast, Bc7Level::Veryfast, Bc7Level::Fast] {
            for (perceptual, weights) in [(false, None), (false, Some([1, 1, 1, 16])), (true, None)]
            {
                let p = Bc7Params {
                    level,
                    perceptual,
                    exact_opaque: false,
                };
                let tune = encoders_sys::Bc7Tune {
                    weights,
                    ..encoders_sys::Bc7Tune::default()
                };
                for target in paths() {
                    let out = tuned_block(&block, p, tune, target);
                    let what = format!(
                        "ks3 block {k} (alpha {v}) {level:?} perceptual={perceptual} {weights:?} \
                         {target:?}"
                    );
                    if out[0].trailing_zeros() != 4 || out[0] >> 5 != 0 {
                        bad.push(format!(
                            "{what}: left mode 4 rotation 0 with 3-bit alpha (byte 0 {:#04x})",
                            out[0]
                        ));
                        continue;
                    }
                    let a: Vec<u8> = decode(&out).iter().map(|d| d[3]).collect();
                    if a.iter().any(|&x| x != v) {
                        bad.push(format!("{what}: alpha decodes {a:?}"));
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "3-bit alpha off its exact code pair:\n{}",
        bad.join("\n")
    );
}
