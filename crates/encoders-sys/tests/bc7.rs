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
