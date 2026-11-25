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

/// Exact constants cover R, G and B only: alpha keeps bc7e's exact-opaque flag (BC7-O14), so an alpha bit is refused
/// rather than dropped, even with no block to encode.
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
