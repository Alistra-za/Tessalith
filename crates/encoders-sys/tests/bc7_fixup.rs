use encoders_sys::bc7_fixup::force_opaque_rotated;
use encoders_sys::{bc7e_compress_blocks, gather_blocks, Bc7Level, Bc7Params};

fn mode6_block(rgb: u128, alpha: u128, p0: u128, p1: u128) -> [u8; 16] {
    let mut v: u128 = 1 << 6; // mode 6
    for (i, c) in [rgb, rgb, rgb, rgb, rgb, rgb, alpha, alpha]
        .iter()
        .enumerate()
    {
        v |= c << (7 + 7 * i);
    }
    v |= p0 << 63 | p1 << 64; // indices stay 0
    v.to_le_bytes()
}

fn decode(b: &[u8; 16]) -> [u8; 64] {
    let mut o = [0u8; 64];
    bcdec_rs::bc7(b, &mut o, 16);
    o
}

fn texel(d: &[u8; 64], i: usize) -> [u8; 4] {
    [d[4 * i], d[4 * i + 1], d[4 * i + 2], d[4 * i + 3]]
}

/// Mode 4: mode bits 0-4 (`10000`), rotation 5-6, index selection 7, then R0 R1 G0 G1 B0 B1 (5 bits each,
/// from bit 8) and A0 A1 (6 bits each, from 38). Mode 5: mode bits 0-5 (`100000`), rotation 6-7, then R0..B1
/// (7 bits each, from 8) and A0 A1 (8 bits each, from 50). Returns (header bits 0-7, colour bits, alpha bits).
fn header(mode: u32, rot: u128, isel: u128) -> (u128, u32, u32) {
    match mode {
        4 => (1 << 4 | rot << 5 | isel << 7, 5, 6),
        5 => (1 << 5 | rot << 6, 7, 8),
        _ => unreachable!(),
    }
}

/// Bits `force_opaque_rotated` may set: both endpoints of colour channel `rot - 1`.
fn rotated_mask(mode: u32, rot: u128) -> u128 {
    let pair = if mode == 4 { 10 } else { 14 };
    ((1u128 << pair) - 1) << (8 + pair * (rot as u32 - 1))
}

/// All indices 0, so every texel decodes endpoint 0: checks the layout above and the rotation swap against bcdec_rs.
#[test]
fn mode4_mode5_layout_and_rotation_agree_with_independent_decoder() {
    let expand = |x: u128, bits: u32| ((x << (8 - bits)) | (x >> (2 * bits - 8))) as u8;
    for (mode, isel) in [(4, 0), (4, 1), (5, 0)] {
        for rot in 0..4u128 {
            let (mut v, cb, ab) = header(mode, rot, isel);
            let (rgb0, a0) = ([3u128, 9, 17], 21u128);
            for (c, e) in rgb0.iter().enumerate() {
                v |= e << (8 + 2 * cb * c as u32); // endpoint 0 of channel c; endpoint 1 stays 0
            }
            v |= a0 << (8 + 6 * cb);
            let d = decode(&v.to_le_bytes());
            let mut want = [
                expand(rgb0[0], cb),
                expand(rgb0[1], cb),
                expand(rgb0[2], cb),
                expand(a0, ab),
            ];
            if rot != 0 {
                want.swap(rot as usize - 1, 3);
            }
            for i in 0..16 {
                assert_eq!(texel(&d, i), want, "mode {mode} isel {isel} rot {rot}");
            }
        }
    }
}

/// Hand-built blocks per mode, rotation and (mode 4) index selection. The rotated colour channel holds
/// endpoints 0 and 13, so every texel decodes alpha < 255 before the fix and 255 after; RGB is unchanged.
#[test]
fn rotated_mode4_mode5_blocks_decode_opaque_with_rgb_unchanged() {
    for (mode, isel) in [(4, 0), (4, 1), (5, 0)] {
        for rot in 1..4u128 {
            let (mut v, cb, ab) = header(mode, rot, isel);
            for c in 0..3u32 {
                let (e0, e1) = if c as u128 == rot - 1 {
                    (0u128, 13u128)
                } else {
                    (4 + 5 * c as u128, 30 - 3 * c as u128)
                };
                v |= e0 << (8 + 2 * cb * c) | e1 << (8 + 2 * cb * c + cb);
            }
            v |= 7 << (8 + 6 * cb) | 40 << (8 + 6 * cb + ab); // stored alpha = the swapped colour channel
            v |= 0x5A5A_5A5A_5A5A_5A5A_5A5Au128 << (8 + 6 * cb + 2 * ab); // mixed indices
            let mut b = v.to_le_bytes();
            let before = decode(&b);
            assert!(
                before.chunks(4).all(|t| t[3] < 255),
                "mode {mode} rot {rot}: must start non-opaque"
            );
            assert!(force_opaque_rotated(&mut b));
            let after = decode(&b);
            for (p, q) in before.chunks(4).zip(after.chunks(4)) {
                assert_eq!(q[3], 255, "mode {mode} isel {isel} rot {rot}");
                assert_eq!(
                    p[..3],
                    q[..3],
                    "mode {mode} isel {isel} rot {rot}: RGB changed"
                );
            }
            assert_eq!(
                u128::from_le_bytes(b),
                v | rotated_mask(mode, rot),
                "only the rotated endpoints may change"
            );
        }
    }
}

fn xorshift(mut s: u64) -> impl FnMut() -> u64 {
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    }
}

#[test]
fn rotated_fix_leaves_rotation_0_and_other_modes_alone() {
    let mut next = xorshift(0x2545_F491_4F6C_DD1D);
    for mode in 0..8u32 {
        for _ in 0..200 {
            let r = (next() as u128) << 64 | next() as u128;
            // Mode bits: `1 << mode` in the low mode+1 bits; modes 4 and 5 get rotation 0.
            let mut v = (r & !((1u128 << (mode + 1)) - 1)) | 1 << mode;
            match mode {
                4 => v &= !(3 << 5),
                5 => v &= !(3 << 6),
                _ => {}
            }
            let mut b = v.to_le_bytes();
            assert!(!force_opaque_rotated(&mut b), "mode {mode}");
            assert_eq!(u128::from_le_bytes(b), v, "mode {mode}");
        }
    }
    let mut reserved = [0u8; 16]; // no mode bit set
    assert!(!force_opaque_rotated(&mut reserved));
    assert_eq!(reserved, [0u8; 16]);
}

/// Random rotated mode 4/5 blocks: alpha decodes 255 everywhere, RGB is bit-identical, and only the
/// rotated channel's endpoint bits are set.
#[test]
fn random_rotated_blocks_decode_opaque_with_rgb_unchanged() {
    let mut next = xorshift(0x9E37_79B9_7F4A_7C15);
    for i in 0..20_000u32 {
        let r = (next() as u128) << 64 | next() as u128;
        let (mode, rot) = (4 + i % 2, 1 + (i / 2 % 3) as u128);
        let (h, _, _) = header(mode, rot, (r >> 7) & 1);
        let v = (r & !0xff) | h; // both headers fill bits 0-7
        let mut b = v.to_le_bytes();
        let before = decode(&b);
        assert!(force_opaque_rotated(&mut b));
        assert_eq!(u128::from_le_bytes(b), v | rotated_mask(mode, rot));
        let after = decode(&b);
        for (p, q) in before.chunks(4).zip(after.chunks(4)) {
            assert_eq!(q[3], 255, "block {v:#034x}");
            assert_eq!(p[..3], q[..3], "block {v:#034x}");
        }
    }
}

/// The review's probe: bc7e (perceptual off, exact-opaque flag as built) on opaque noise emitted rotated mode 4/5
/// blocks whose alpha decoded below 255 until bc7e patch 0005 (Q2t), which leaves none on this input at veryslow,
/// basic, slowest and max (Note LR6). The fix still makes every rotated block it touches decode 255 with RGB
/// bit-identical.
#[test]
fn bc7e_opaque_noise_rotated_blocks_fixed_losslessly() {
    if encoders_sys::ispc_target() == encoders_sys::IspcTarget::Avx512 {
        eprintln!("skipped: AVX2-path probe");
        return;
    }
    let mut next = xorshift(0x1234_5678_9ABC_DEF1);
    let px: Vec<[u8; 4]> = (0..128 * 128)
        .map(|_| {
            let s = next();
            [s as u8, (s >> 8) as u8, (s >> 16) as u8, 255]
        })
        .collect();
    let blocks = gather_blocks(&px, 128, 128);
    let mut out = vec![0u8; blocks.len()];
    let params = Bc7Params {
        level: Bc7Level::Veryslow,
        perceptual: false,
        exact_opaque: true,
    };
    for (o, b) in out.chunks_mut(64 * 16).zip(blocks.chunks(64 * 16)) {
        bc7e_compress_blocks(b, o, params);
    }
    for blk in out.chunks(16) {
        let orig: [u8; 16] = blk.try_into().unwrap();
        let mut b = orig;
        if !force_opaque_rotated(&mut b) {
            continue;
        }
        let (before, after) = (decode(&orig), decode(&b));
        for (p, q) in before.chunks(4).zip(after.chunks(4)) {
            assert_eq!(q[3], 255);
            assert_eq!(p[..3], q[..3]);
        }
    }
}

/// DECISIONS AH1: a mode-6 block with alpha endpoints 0 but p-bit 1 decodes alpha 1; the fix makes it exactly 0.
#[test]
fn zero_alpha_fix_clears_mode6_pbits() {
    let mut b = mode6_block(10, 0, 1, 1);
    assert_eq!(decode(&b)[3], 1);
    assert!(encoders_sys::bc7_fixup::force_zero_alpha(&mut b));
    let d = decode(&b);
    assert!(d.chunks(4).all(|p| p[3] == 0));
    assert_eq!(d[0], 20); // RGB endpoint shifts down one LSB: (10 << 1 | 0)
}

/// Every bc7e mode choice on noisy RGB with alpha 0 throughout decodes to exactly 0 after the fix.
#[test]
fn zero_alpha_fix_gives_exact_0_on_bc7e_output() {
    let mut seed = 12345u32;
    let mut rnd = || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) as u8
    };
    let texels: Vec<[u8; 4]> = (0..64 * 64).map(|_| [rnd(), rnd(), rnd(), 0]).collect();
    let blocks = gather_blocks(&texels, 64, 64);
    // Blocks bc7e itself decoded above 0: the fix must have something to do, or the test proves nothing.
    let mut needed = 0;
    for level in [Bc7Level::Ultrafast, Bc7Level::Basic, Bc7Level::Slow] {
        let mut out = vec![0u8; blocks.len() / 16 * 16];
        bc7e_compress_blocks(
            &blocks,
            &mut out,
            Bc7Params {
                level,
                perceptual: false,
                exact_opaque: false,
            },
        );
        for blk in out.chunks_mut(16) {
            let blk: &mut [u8; 16] = blk.try_into().unwrap();
            needed += decode(blk).chunks(4).any(|p| p[3] != 0) as usize;
            encoders_sys::bc7_fixup::force_zero_alpha(blk);
            assert!(
                decode(blk).chunks(4).all(|p| p[3] == 0),
                "{level:?}: {blk:?}"
            );
        }
    }
    assert!(needed > 0, "no bc7e block decoded alpha above 0");
}

/// Mode 7 and rotated mode 4/5 blocks: alpha endpoints (and mode 7's p-bits) cleared give alpha 0.
#[test]
fn zero_alpha_fix_covers_modes_4_5_and_7() {
    for mut b in [
        [
            0x80u8, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff,
        ],
        [
            0x10 | (2 << 5),
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
        [
            0x20 | (3 << 6),
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0xff,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
        [
            0x10, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
        [
            0x20, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0,
        ],
    ] {
        assert!(encoders_sys::bc7_fixup::force_zero_alpha(&mut b));
        assert!(decode(&b).chunks(4).all(|p| p[3] == 0), "{b:?}");
    }
}

/// The gate's per-texel alpha equals `bcdec_rs`'s for every mode `force_zero_alpha` rewrites: mode 4 at each rotation
/// and both index selectors (rotation 0 reads the scalar slot's index set, which the selector picks), mode 5 at each
/// rotation, modes 6 and 7. Random blocks with the header bits forced, so mode 7 meets every partition and anchor.
/// Modes 0 to 3 store no alpha: the gate has none (`None`).
#[test]
fn the_gate_decodes_alpha_as_bcdec_does() {
    let mut next = xorshift(0x51ED_270B_3C9F_A1D7);
    let mut check = |what: String, header: u128, keep: u128, n: u32| {
        for _ in 0..n {
            let r = (next() as u128) << 64 | next() as u128;
            let b = ((r & !keep) | header).to_le_bytes();
            let want: Vec<u8> = decode(&b).chunks(4).map(|t| t[3]).collect();
            let got = encoders_sys::bc7_fixup::gate_alpha(&b).expect("a mode that stores alpha");
            assert_eq!(&got[..], &want[..], "{what}: {b:02x?}");
        }
    };
    for rot in 0..4u128 {
        for isel in 0..2u128 {
            check(
                format!("mode 4 rot {rot} isel {isel}"),
                0x10 | rot << 5 | isel << 7,
                0xff,
                3000,
            );
        }
        check(format!("mode 5 rot {rot}"), 0x20 | rot << 6, 0xff, 3000);
    }
    check("mode 6".into(), 0x40, 0x7f, 6000);
    // 6 partition bits from bit 8 are random, so every one of the 64 partitions turns up.
    check("mode 7".into(), 0x80, 0xff, 20_000);
    let mut next = xorshift(0x0DDB_1A5E_5BAD_5EED);
    for mode in 0..4u32 {
        let r = (next() as u128) << 64 | next() as u128;
        let v = (r & !((1u128 << (mode + 1)) - 1)) | 1 << mode;
        assert!(encoders_sys::bc7_fixup::gate_alpha(&v.to_le_bytes()).is_none());
    }
}

/// `force_zero_alpha` as it was before the gate: the mask of every alpha field of the block, cleared. Only the
/// witness search below uses it, to find blocks the old rewrite changed.
fn ungated_zero(block: &[u8; 16]) -> [u8; 16] {
    let v = u128::from_le_bytes(*block);
    let field = |at: u32, len: u32| ((1u128 << len) - 1) << at;
    let mask = if block[0] & 0x1f == 0x10 {
        match (v >> 5) & 3 {
            0 => field(38, 12),
            r => field(8 + 10 * (r as u32 - 1), 10),
        }
    } else if block[0] & 0x3f == 0x20 {
        match (v >> 6) & 3 {
            0 => field(50, 16),
            r => field(8 + 14 * (r as u32 - 1), 14),
        }
    } else if block[0] & 0x7f == 0x40 {
        field(49, 16)
    } else if block[0] == 0x80 {
        field(74, 24)
    } else {
        return *block;
    };
    (v & !mask).to_le_bytes()
}

/// Every ISPC path this CPU runs up to `ispc_target()` (Note HZ3), as `tests/bc7.rs` does.
fn paths() -> Vec<encoders_sys::IspcTarget> {
    encoders_sys::IspcTarget::ALL
        .into_iter()
        .filter(|t| t.supported() && *t <= encoders_sys::ispc_target())
        .collect()
}

/// Seeded synthetic blocks, alpha 0: colour ramps between two random colours along a random direction with a little
/// noise, or plain noise (no corpus block, BC7-C13).
fn zero_alpha_blocks(seed: u64, blocks: usize) -> Vec<[u8; 4]> {
    let mut next = xorshift(seed);
    let mut px = Vec::with_capacity(blocks * 16);
    for _ in 0..blocks {
        let r = next();
        let c0 = next();
        // The far end of the ramp: anywhere, or within a few levels of the start (a near-flat block, which uses low
        // indices only).
        let kind = (r >> 16) % 3;
        let c1 = match kind {
            0 => next(),
            1 => {
                let d = next();
                (0..3).fold(0u64, |acc, ch| {
                    let near = ((c0 >> (8 * ch)) as u8).saturating_add((d >> (8 * ch)) as u8 & 3);
                    acc | u64::from(near) << (8 * ch)
                })
            }
            _ => c0,
        };
        let noise = if kind == 0 {
            [0u64, 3, 12, 255][(r % 4) as usize]
        } else {
            [0u64, 0, 1, 1][(r % 4) as usize]
        };
        for t in 0..16u64 {
            let along = match (r >> 8) % 3 {
                0 => t % 4,
                1 => t / 4,
                _ => (t % 4 + t / 4) / 2,
            };
            let mut px_t = [0u8, 0, 0, 0];
            for (ch, o) in px_t.iter_mut().take(3).enumerate() {
                let (a, b) = ((c0 >> (8 * ch)) as u8 as i64, (c1 >> (8 * ch)) as u8 as i64);
                let v = a + (b - a) * along as i64 / 3;
                let n = if noise == 0 {
                    0
                } else {
                    (next() % (2 * noise + 1)) as i64 - noise as i64
                };
                *o = (v + n).clamp(0, 255) as u8;
            }
            px.push(px_t);
        }
    }
    px
}

/// A block the encoder already keeps at alpha 0 keeps its bytes, though the old rewrite changed them (it cleared a
/// mode 6 or 7 p-bit, which moves RGB, or zeroed endpoints that decode 0 anyway). Blocks are encoded with A on, which
/// is where the engine's `_m` route sends an alpha-0 file. Needs at least one such block per path.
#[test]
fn the_zero_fix_leaves_a_block_already_at_zero() {
    // The settings `complex_material` ships: the data flat tolerance and the G guard (Note LR, LR8).
    let tune = encoders_sys::Bc7Tune {
        weights: None,
        flat_tolerance: Some(encoders_sys::Bc7FlatTolerance::Data),
        exact_constants: Some(0b010),
    };
    for target in paths() {
        let mut kept = 0;
        'search: for level in [
            Bc7Level::Ultrafast,
            Bc7Level::Basic,
            Bc7Level::Slow,
            Bc7Level::Veryslow,
        ] {
            let params = Bc7Params {
                level,
                perceptual: false,
                exact_opaque: true,
            };
            for chunk in 0..(1usize << 16) / 256 {
                let mut px = zero_alpha_blocks(0xA11C_E000 + chunk as u64, 256);
                // G is flat in every block, 255 and 0 in turn: the engine's `_m` guards G (the old fix cleared
                // p-bits, where a guarded 255 sits; the test checks bytes kept, not G).
                for (i, t) in px.iter_mut().enumerate() {
                    t[1] = [255, 0][i / 16 % 2];
                }
                let mut out = vec![0u8; px.len()];
                encoders_sys::bc7e_compress_blocks_tuned_on(&px, &mut out, params, tune, target);
                for blk in out.chunks(16) {
                    let blk: &[u8; 16] = blk.try_into().unwrap();
                    let at_zero = decode(blk).chunks(4).all(|t| t[3] == 0);
                    if !at_zero || ungated_zero(blk) == *blk {
                        continue;
                    }
                    let mut b = *blk;
                    assert!(
                        !encoders_sys::bc7_fixup::force_zero_alpha(&mut b),
                        "{target:?} {level:?}: rewrote a block that decodes alpha 0: {blk:02x?}"
                    );
                    assert_eq!(b, *blk, "{target:?} {level:?}: bytes changed");
                    kept += 1;
                    if kept == 8 {
                        break 'search;
                    }
                }
            }
        }
        eprintln!("{target:?}: {kept} blocks the old fix changed, kept");
        // The witnesses are a few blocks of AMD's bytes (Note JC); an Intel CPU writes other bytes and could find none.
        // Every witness found is asserted above whichever CPU runs.
        if encoders_sys::intel_cpu() {
            eprintln!("skipped: the witness count is AMD's bytes (Note JC)");
        } else {
            assert!(
                kept > 0,
                "{target:?}: no block found that the old fix changed"
            );
        }
    }
}
