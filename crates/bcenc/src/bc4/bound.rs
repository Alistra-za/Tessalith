//! A lower bound on a BC4 endpoint pair's error, from four per-block tables (Note HY24 records the skip and its
//! figures). With the block's 16 values x:
//!
//! - `A[e]` = sum over x < e of (e - x)^2, `B[e]` = sum over x > e of (x - e)^2;
//! - `C[e]` = sum over x < e of min(e - x, x)^2, `D[e]` = sum over x > e of min(x - e, 255 - x)^2.
//!
//! `lb(e0, e1)` is `A[e1] + B[e0]` for `e0 > e1` (8-value palette) and `C[e0] + D[e1]` otherwise (6-value palette, where
//! slots 6 and 7 are 0 and 255). Every palette value lies in `[min(e0, e1), max(e0, e1)]`, apart from the 6-value
//! mode's 0 and 255, so a texel outside that cover costs at least what the tables count for it.
//!
//! The tables are exact over a window `[min - pad, max + pad]` clamped to 0..=255 and a pair with an end outside it has
//! bound 0 (the bound never prunes it; only a best of error 0 does). The fill is one sum of 16 squared saturating
//! differences per entry: SSE2 where the target has it, a scalar loop elsewhere; in the unit tests every fill also runs
//! the scalar definition and asserts the same tables.

use super::minmax;

/// The tables of one block, on the stack of that block's search.
pub(crate) struct Bound {
    /// The window, both ends included; `lo > hi` for [`Bound::empty`].
    lo: u8,
    hi: u8,
    a: [u32; 256],
    b: [u32; 256],
    c: [u32; 256],
    d: [u32; 256],
}

impl Bound {
    /// Exact tables over `[mn - pad, mx + pad]` clamped to 0..=255, for any block, solid ones included. `pad` is the
    /// caller's: 0..=255 (callers clamp before calling).
    pub(crate) fn new(v: &[u8; 16], pad: u8) -> Bound {
        let (mn, mx) = minmax(v);
        let mut t = Bound {
            lo: mn.saturating_sub(pad),
            hi: mx.saturating_add(pad),
            ..Bound::empty()
        };
        fill(v, &mut t);
        t
    }

    /// A bound that never prunes on its own: `lb` is 0 for every pair.
    pub(crate) fn empty() -> Bound {
        Bound {
            lo: 1,
            hi: 0,
            a: [0; 256],
            b: [0; 256],
            c: [0; 256],
            d: [0; 256],
        }
    }

    /// A lower bound on the error of pair (e0, e1) under `pal(e0, e1)`; 0 when e0 or e1 is outside the window.
    #[inline(always)]
    pub(crate) fn lb(&self, e0: u8, e1: u8) -> u32 {
        let inside = |e: u8| (self.lo..=self.hi).contains(&e);
        if !inside(e0) || !inside(e1) {
            return 0;
        }
        if e0 > e1 {
            self.a[e1 as usize] + self.b[e0 as usize]
        } else {
            self.c[e0 as usize] + self.d[e1 as usize]
        }
    }
}

/// Fills the window's entries: the SSE2 kernel where the target has it (in the unit tests also compared with the scalar
/// definition), the scalar loop elsewhere.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
fn fill(v: &[u8; 16], t: &mut Bound) {
    sse2::fill(v, t);
    #[cfg(test)]
    {
        let mut s = Bound::empty();
        (s.lo, s.hi) = (t.lo, t.hi);
        fill_scalar(v, &mut s);
        assert!(
            t.a == s.a && t.b == s.b && t.c == s.c && t.d == s.d,
            "{v:?} window {}..={}",
            t.lo,
            t.hi
        );
    }
}

#[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
fn fill(v: &[u8; 16], t: &mut Bound) {
    fill_scalar(v, t);
}

/// The definition: per entry, the four sums over the 16 texels. The SSE2 fill's test reference, and what a target
/// without SSE2 runs.
#[cfg(any(test, not(all(target_arch = "x86_64", target_feature = "sse2"))))]
fn fill_scalar(v: &[u8; 16], t: &mut Bound) {
    for e in t.lo..=t.hi {
        let (mut a, mut b, mut c, mut d) = (0u32, 0u32, 0u32, 0u32);
        for &x in v {
            let (e, x) = (e as i32, x as i32);
            if x < e {
                a += ((e - x) * (e - x)) as u32;
                let m = (e - x).min(x);
                c += (m * m) as u32;
            }
            if x > e {
                b += ((x - e) * (x - e)) as u32;
                let m = (x - e).min(255 - x);
                d += (m * m) as u32;
            }
        }
        let i = e as usize;
        (t.a[i], t.b[i], t.c[i], t.d[i]) = (a, b, c, d);
    }
}

/// The SSE2 fill. Compiled only where the target has SSE2, so its `target_feature(enable = "sse2")` function is always
/// callable; Rust still wants an `unsafe` block for the call from plain code. Integer code only.
///
/// Bounds: every term is a saturating difference or a minimum of two bytes, so a byte 0..=255; the squares are summed
/// as in `bc4::sse2::pair` (a total of at most 16 x 255^2 = 1,040,400), so every addition is exact.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod sse2 {
    use super::Bound;
    use core::arch::x86_64::*;

    /// The sum of the squares of the 16 bytes of `m`.
    #[target_feature(enable = "sse2")]
    #[inline]
    fn sumsq(m: __m128i) -> u32 {
        let zero = _mm_setzero_si128();
        let (lo, hi) = (_mm_unpacklo_epi8(m, zero), _mm_unpackhi_epi8(m, zero));
        let s = _mm_add_epi32(_mm_madd_epi16(lo, lo), _mm_madd_epi16(hi, hi));
        let s = _mm_add_epi32(s, _mm_shuffle_epi32::<0b01_00_11_10>(s));
        let s = _mm_add_epi32(s, _mm_shuffle_epi32::<0b00_00_00_01>(s));
        debug_assert!((0..=1_040_400).contains(&_mm_cvtsi128_si32(s)));
        _mm_cvtsi128_si32(s) as u32
    }

    #[target_feature(enable = "sse2")]
    fn fill_sse2(v: &[u8; 16], t: &mut Bound) {
        let half = |a: &[u8]| i64::from_le_bytes([a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]]);
        let x = _mm_set_epi64x(half(&v[8..]), half(&v[..8]));
        // 255 - x per byte.
        let inv = _mm_xor_si128(x, _mm_set1_epi8(-1));
        for e in t.lo..=t.hi {
            let q = _mm_set1_epi8(e as i8);
            // e - x where x < e, else 0; and x - e where x > e, else 0.
            let below = _mm_subs_epu8(q, x);
            let above = _mm_subs_epu8(x, q);
            let i = e as usize;
            t.a[i] = sumsq(below);
            t.b[i] = sumsq(above);
            t.c[i] = sumsq(_mm_min_epu8(below, x));
            t.d[i] = sumsq(_mm_min_epu8(above, inv));
        }
    }

    pub(super) fn fill(v: &[u8; 16], t: &mut Bound) {
        // SAFETY: this module is compiled only where SSE2 is enabled for the target (the `cfg` on `mod sse2`).
        unsafe { fill_sse2(v, t) }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{pair_scalar, pal};
    use super::*;
    use crate::gen;

    /// The generated blocks plus the flat extremes and a block alternating 0 and 255.
    fn blocks() -> Vec<[u8; 16]> {
        let mut out = gen::bc4_classes(0xB4B4_0099, 12);
        out.push([0; 16]);
        out.push([255; 16]);
        out.push(std::array::from_fn(|i| if i % 2 == 0 { 0 } else { 255 }));
        out
    }

    #[test]
    fn the_fill_equals_the_definition() {
        for v in blocks() {
            for pad in [0u8, 16, 255] {
                // `Bound::new` has also compared its fill with `fill_scalar` (its shadow check); this is the check on a
                // target without the kernel, and compares with sums written out independently.
                let t = Bound::new(&v, pad);
                let (mn, mx) = minmax(&v);
                assert_eq!(
                    (t.lo, t.hi),
                    (mn.saturating_sub(pad), mx.saturating_add(pad))
                );
                for e in t.lo..=t.hi {
                    let sq = |d: i32| (d * d) as u32;
                    let sum = |f: &dyn Fn(i32, i32) -> Option<u32>| -> u32 {
                        v.iter().filter_map(|&x| f(e as i32, x as i32)).sum()
                    };
                    let i = e as usize;
                    assert_eq!(
                        t.a[i],
                        sum(&|e, x| (x < e).then(|| sq(e - x))),
                        "A {v:?} {e}"
                    );
                    assert_eq!(
                        t.b[i],
                        sum(&|e, x| (x > e).then(|| sq(x - e))),
                        "B {v:?} {e}"
                    );
                    assert_eq!(
                        t.c[i],
                        sum(&|e, x| (x < e).then(|| sq((e - x).min(x)))),
                        "C {v:?} {e}"
                    );
                    assert_eq!(
                        t.d[i],
                        sum(&|e, x| (x > e).then(|| sq((x - e).min(255 - x)))),
                        "D {v:?} {e}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_bound_never_exceeds_the_error() {
        for v in blocks() {
            let t = Bound::new(&v, 255);
            let solid = v.iter().all(|&x| x == v[0]);
            let mut tight = false;
            for e0 in 0..=255u8 {
                for e1 in 0..=255u8 {
                    let (err, _) = pair_scalar(&v, &pal(e0, e1), u32::MAX).unwrap();
                    let lb = t.lb(e0, e1);
                    assert!(
                        lb <= err,
                        "{v:?} ({e0}, {e1}): bound {lb} above error {err}"
                    );
                    tight |= lb == err && err > 0;
                }
            }
            assert!(solid || tight, "{v:?}: the bound is never tight above 0");
        }
    }

    #[test]
    fn the_bound_is_zero_outside_its_window() {
        let mut v = [100u8; 16];
        v[0] = 120;
        let t = Bound::new(&v, 16);
        assert_eq!(t.lb(83, 100), 0);
        assert_eq!(t.lb(100, 137), 0);
        // The window's edge is inside: 84..=136.
        let want = t.c[136] + t.d[136];
        // Fifteen texels of 100 and one of 120 sit below 136, and none above: min(36, 100)^2 and min(16, 120)^2.
        assert_eq!(want, 15 * 36 * 36 + 16 * 16);
        assert!(want > 0);
        assert_eq!(t.lb(136, 136), want);
        // Above the window with the other end inside: both modes, and the sums are positive, so only the window check
        // makes them 0 (C[120] and A[120] are 15 x 20^2).
        assert_eq!(t.lb(120, 137), 0);
        assert_eq!(t.lb(137, 120), 0);
        // The lower edge is inside too. Texels 100 x15 and 120 lie above 99: 15 x 1^2 + 21^2. (The window's lower end
        // has no texel below it, so its own table entry is 0; the other end of the pair carries the sum.)
        let above = 15 + 21 * 21;
        assert_eq!(t.lb(84, 99), above);
        assert_eq!(t.lb(99, 84), above);
        let e = Bound::empty();
        for (e0, e1) in [
            (0, 0),
            (0, 255),
            (255, 0),
            (100, 120),
            (120, 100),
            (255, 255),
        ] {
            assert_eq!(e.lb(e0, e1), 0, "({e0}, {e1})");
        }
    }

    #[test]
    fn palette_values_lie_in_the_cover() {
        for e0 in 0..=255u8 {
            for e1 in 0..=255u8 {
                let p = pal(e0, e1);
                let (lo, hi) = (e0.min(e1), e0.max(e1));
                for (j, &q) in p.iter().enumerate() {
                    let extreme = e0 <= e1 && ((j == 6 && q == 0) || (j == 7 && q == 255));
                    assert!(
                        (lo..=hi).contains(&q) || extreme,
                        "({e0}, {e1}) slot {j} = {q}"
                    );
                }
            }
        }
    }
}
