//! Pins the BC4 reference search (docs/research/bc4-family.md): `upstream` to the upstream rgbcx path, `search` to its
//! never-worse rule and to the patched C++ (BC3/BC4/BC5 entry points). The engine's end-to-end check is in
//! `crates/engine/tests/encode.rs`.
use encoders_sys::{bc3_encode_block, bc4_encode_block, bc5_encode_block, RGBCX_MAX_LEVEL};
use support::bc4ref::{blocks, search, upstream};

mod support {
    pub mod bc4ref;
}
fn px(v: &[u8; 16], ch: usize) -> [[u8; 4]; 16] {
    let mut p = [[7u8; 4]; 16];
    for i in 0..16 {
        p[i][ch] = v[i];
    }
    p
}

fn sse(b: &[u8; 8], v: &[u8; 16]) -> u64 {
    let mut d = [0u8; 16];
    bcdec_rs::bc4(b, &mut d, 4, false);
    d.iter()
        .zip(v)
        .map(|(&x, &y)| (x as i64 - y as i64).pow(2) as u64)
        .sum()
}

fn flips(b: &[u8; 8], v: &[u8; 16]) -> u32 {
    let mut d = [0u8; 16];
    bcdec_rs::bc4(b, &mut d, 4, false);
    d.iter()
        .zip(v)
        .filter(|(&x, &y)| (x >= 128) != (y >= 128))
        .count() as u32
}

#[test]
fn upstream_port_equals_ffi() {
    for v in blocks(20_000) {
        for ch in 0..4 {
            let p = px(&v, ch);
            for r in [4, 5, 10] {
                assert_eq!(
                    upstream(&v, r),
                    encoders_sys::bc4_encode_block_flags(
                        &p,
                        ch,
                        r,
                        encoders_sys::BC4_USE_ALL_MODES
                    ),
                    "{v:?} ch {ch} r {r}"
                );
            }
        }
    }
}

#[test]
fn search_never_worse() {
    for v in blocks(20_000) {
        for r in [4, 5, 10] {
            let s = upstream(&v, r);
            let a = search(&v, r, false);
            assert!(sse(&a, &v) <= sse(&s, &v), "{v:?} r {r}");
            let g = search(&v, r, true);
            assert!(sse(&g, &v) <= sse(&s, &v), "{v:?} r {r} guard");
            assert!(flips(&g, &v) <= flips(&s, &v), "{v:?} r {r} guard");
        }
    }
}

#[test]
fn const_and_binary_blocks_unchanged() {
    let mut checker = [0u8; 16];
    for (i, x) in checker.iter_mut().enumerate() {
        *x = if (i + i / 4) % 2 == 0 { 0 } else { 255 };
    }
    for v in [[0u8; 16], [255; 16], [77; 16], checker] {
        assert_eq!(search(&v, 5, false), upstream(&v, 5), "{v:?}");
        assert_eq!(search(&v, 5, true), upstream(&v, 5), "{v:?}");
    }
}

#[test]
fn trimmed_range_beats_upstream_on_near_extremes() {
    let v = [
        0, 0, 1, 2, 40, 80, 120, 160, 200, 253, 254, 255, 255, 255, 128, 64,
    ];
    assert!(sse(&search(&v, 5, false), &v) < sse(&upstream(&v, 5), &v));
}

// The vendored C++ (shim flags BC4_SDC_SEARCH, plus BC4_SDC_GUARD128 for BC3 alpha) equals the reference byte for
// byte; radius 10 is `max`.
#[test]
fn ffi_equals_reference() {
    let bs = blocks(20_000);
    for (i, v) in bs.iter().enumerate() {
        let w = &bs[(i + 1) % bs.len()];
        for r in [4, 5, 10] {
            for ch in 0..4 {
                let p = px(v, ch);
                assert_eq!(
                    bc4_encode_block(&p, ch, r),
                    search(v, r, false),
                    "{v:?} ch {ch} r {r}"
                );
            }
            assert_eq!(
                bc3_encode_block(&px(v, 3), RGBCX_MAX_LEVEL, r)[..8],
                search(v, r, true),
                "{v:?} bc3 r {r}"
            );
            let mut p = px(v, 0);
            for t in 0..16 {
                p[t][1] = w[t];
            }
            let e = bc5_encode_block(&p, r);
            assert_eq!(e[..8], search(v, r, false), "{v:?} bc5 r half r {r}");
            assert_eq!(e[8..], search(w, r, false), "{w:?} bc5 g half r {r}");
        }
    }
}

/// The flag constants in `encoders-sys` reach the C++ as the reference expects (they are only mirrored by hand).
#[test]
fn flag_constants_select_the_reference_arms() {
    use encoders_sys::{
        bc4_encode_block_flags, BC4_SDC_GUARD128, BC4_SDC_SEARCH, BC4_USE_ALL_MODES,
    };
    for v in blocks(400) {
        let p = px(&v, 0);
        for r in [4, 5, 10] {
            let f = BC4_USE_ALL_MODES | BC4_SDC_SEARCH;
            assert_eq!(
                bc4_encode_block_flags(&p, 0, r, f),
                search(&v, r, false),
                "{v:?} r {r}"
            );
            let g = f | BC4_SDC_GUARD128;
            assert_eq!(
                bc4_encode_block_flags(&p, 0, r, g),
                search(&v, r, true),
                "{v:?} r {r} guard"
            );
        }
    }
}

#[test]
fn bc5_halves_equal_bc4_per_channel() {
    let bs = blocks(4_000);
    for (i, v) in bs.iter().enumerate() {
        let w = &bs[(i + 1) % bs.len()];
        let mut p = px(v, 0);
        for t in 0..16 {
            p[t][1] = w[t];
        }
        let e = bc5_encode_block(&p, 5);
        assert_eq!(e[..8], bc4_encode_block(&p, 0, 5), "{v:?}");
        assert_eq!(e[8..], bc4_encode_block(&p, 1, 5), "{w:?}");
    }
}

#[test]
fn guard_only_on_bc3() {
    // First block of `blocks(20_000)` where the guard changes the reference result.
    let v = [
        101, 185, 152, 193, 87, 248, 130, 84, 241, 174, 246, 200, 103, 217, 60, 5,
    ];
    let (v4, v5) = (search(&v, 5, false), search(&v, 5, true));
    assert_ne!(v4, v5);
    assert_eq!(bc4_encode_block(&px(&v, 0), 0, 5), v4);
    assert_eq!(bc5_encode_block(&px(&v, 0), 5)[..8], v4);
    assert_eq!(bc3_encode_block(&px(&v, 3), RGBCX_MAX_LEVEL, 5)[..8], v5);
}
