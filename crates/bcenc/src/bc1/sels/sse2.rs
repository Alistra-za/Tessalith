//! The selector kernel: all 16 texels of a candidate at once, in SSE2 integer intrinsics (the maths, and why it is a sum
//! of products, are in the module doc of `sels.rs`). Compiled only where the target has SSE2 (the `cfg` on `mod sse2`),
//! so the `target_feature(enable = "sse2")` functions here are always callable; Rust still wants an `unsafe` block for
//! the call from plain code, and `Texels::masked` and the three searches of this module carry them. Integer code only.
//!
//! Intrinsics and `as i16` wrap silently, so the bounds below are what keep the integers exact. They hold for channel
//! weights `w[c]` in 1..=256 summing to at most 256 and decoder weights summing to `W <= 4`, which `sels::Texels::masked`
//! enforces:
//! - `|x| <= (256 * 255 + 1) / 2 = 32,640`, so `x` fits `i16`; the three `|x|` of one texel sum to at most 32,642.
//! - `s = dec[0] * a + dec[1] * b <= 4 * 255 = 1,020`, so `|m| = 2 * W * s <= 8,160`, which fits `i16`.
//! - `|x[0]*m[0] + x[1]*m[1] + x[2]*m[2]| <= 32,642 * 8,160 = 266,358,720`.
//! - `k_j` lies in `-266,358,720 ..= 332,928,000` (`256 * (1,020^2 + 4 * 255^2)`), so every `rest` stays inside `i32`.
//! - `t[i]` and every texel error are at most `16 * 256 * 255^2 = 266,342,400`; four of them fit `i32`; all 16 sum to
//!   at most 4,261,478,400, below 2^32, so the last additions wrap as `i32` and are read as `u32` exactly.
//!
//! `debug_assert!`s guard each narrowing cast, and `the_largest_errors_do_not_overflow` compares the kernel with the
//! scalar code at the largest inputs.

use super::super::palette::{dual_palette, DualPalette};
use super::super::Ends;
use core::arch::x86_64::*;

type V4 = [__m128i; 4];

/// Row `v` of each field holds texels `4v..4v+3`.
pub(super) struct Texels {
    xa: V4,   // per texel the i16 pair (x[0], x[1])
    xb: V4,   // per texel the i16 pair (x[2], 0)
    t: V4,    // per texel t as i32
    keep: V4, // per texel all ones when opaque, zero when transparent (the masked search)
    off: [i32; 3],
    w: [i32; 3],
    dec: [i32; 2],
}

impl Texels {
    /// `x[c][i] = w[c] * c_i - off[c]` with `off[c] = (w[c] * 255 + 1) / 2` (centred, so it fits `i16`), and
    /// `t[i] = W^2 * sum over c of w[c] * c_i^2`; `keep[i]` is zero when bit i of `mask` is set. A transparent
    /// texel's `x` and `t` are those of its colour, within the same bounds, and the masked search drops its error.
    pub(super) fn masked(px: &[[u8; 4]; 16], w: [i32; 3], dec: [i32; 2], mask: u32) -> Self {
        let ws = dec[0] + dec[1];
        let off = [
            (w[0] * 255 + 1) / 2,
            (w[1] * 255 + 1) / 2,
            (w[2] * 255 + 1) / 2,
        ];
        let (mut xa, mut xb, mut t) = ([[0i16; 8]; 4], [[0i16; 8]; 4], [[0i32; 4]; 4]);
        let mut keep = [[0i32; 4]; 4];
        // The index is the texel of several arrays at once.
        #[allow(clippy::needless_range_loop)]
        for i in 0..16 {
            let (v, l) = (i / 4, i % 4);
            let c = [px[i][0] as i32, px[i][1] as i32, px[i][2] as i32];
            let x = [
                w[0] * c[0] - off[0],
                w[1] * c[1] - off[1],
                w[2] * c[2] - off[2],
            ];
            debug_assert!(x.iter().all(|&x| i16::try_from(x).is_ok()));
            (xa[v][2 * l], xa[v][2 * l + 1], xb[v][2 * l]) =
                (x[0] as i16, x[1] as i16, x[2] as i16);
            t[v][l] = ws * ws * (w[0] * c[0] * c[0] + w[1] * c[1] * c[1] + w[2] * c[2] * c[2]);
            keep[v][l] = if mask & (1 << i) == 0 { -1 } else { 0 };
        }
        // SAFETY: this module is compiled only where SSE2 is enabled for the target (the `cfg` on `mod sse2`).
        let (xa, xb, t, keep) = unsafe { load(&xa, &xb, &t, &keep) };
        Texels {
            xa,
            xb,
            t,
            keep,
            off,
            w,
            dec,
        }
    }
}

#[target_feature(enable = "sse2")]
#[inline]
fn load(
    xa: &[[i16; 8]; 4],
    xb: &[[i16; 8]; 4],
    t: &[[i32; 4]; 4],
    keep: &[[i32; 4]; 4],
) -> (V4, V4, V4, V4) {
    let l16 = |a: &[i16; 8]| _mm_set_epi16(a[7], a[6], a[5], a[4], a[3], a[2], a[1], a[0]);
    let l32 = |a: &[i32; 4]| _mm_set_epi32(a[3], a[2], a[1], a[0]);
    (
        [l16(&xa[0]), l16(&xa[1]), l16(&xa[2]), l16(&xa[3])],
        [l16(&xb[0]), l16(&xb[1]), l16(&xb[2]), l16(&xb[3])],
        [l32(&t[0]), l32(&t[1]), l32(&t[2]), l32(&t[3])],
        [l32(&keep[0]), l32(&keep[1]), l32(&keep[2]), l32(&keep[3])],
    )
}

/// Palette entry `j`: the multiplier pair (m[0], m[1]) as two i16 in one i32, the multiplier m[2], and `k_j`
/// (`m[c] = -2 * W * s`, `k_j = sum over c of ( w[c] * (s^2 + dec[0] * dec[1] * (a - b)^2) - 2 * W * off[c] * s )`, for
/// `a`, `b` the entry's NVIDIA and AMD values of channel `c`).
#[inline(always)]
fn entry(t: &Texels, p: &DualPalette, j: usize) -> (i32, i32, i32) {
    let ws = t.dec[0] + t.dec[1];
    let pal = [&p.r, &p.g, &p.b];
    let (mut m, mut k) = ([0i32; 3], 0i32);
    for c in 0..3 {
        let (a, b) = (pal[c][0][j], pal[c][1][j]);
        let s = t.dec[0] * a + t.dec[1] * b;
        m[c] = -2 * ws * s;
        debug_assert!(i16::try_from(m[c]).is_ok());
        k += t.w[c] * (s * s + t.dec[0] * t.dec[1] * (a - b) * (a - b)) - 2 * ws * t.off[c] * s;
    }
    (
        ((m[1] as u16 as u32) << 16 | (m[0] as u16 as u32)) as i32,
        (m[2] as u16 as u32) as i32,
        k,
    )
}

/// For one palette entry: `rest` of every texel (`k_j` plus the three products). The error of a texel is `t + rest`.
#[target_feature(enable = "sse2")]
#[inline]
fn rest(t: &Texels, e: (i32, i32, i32)) -> V4 {
    let (ma, mb, k) = (
        _mm_set1_epi32(e.0),
        _mm_set1_epi32(e.1),
        _mm_set1_epi32(e.2),
    );
    let f = |v: usize| {
        _mm_add_epi32(
            k,
            _mm_add_epi32(_mm_madd_epi16(t.xa[v], ma), _mm_madd_epi16(t.xb[v], mb)),
        )
    };
    [f(0), f(1), f(2), f(3)]
}

/// The lower of each i32 pair (SSE2 has no `pminsd`).
#[target_feature(enable = "sse2")]
#[inline]
fn min32(a: __m128i, b: __m128i) -> __m128i {
    let gt = _mm_cmpgt_epi32(a, b);
    _mm_or_si128(_mm_and_si128(gt, b), _mm_andnot_si128(gt, a))
}

/// The four i32 lanes summed, wrapping, read as u32.
#[target_feature(enable = "sse2")]
#[inline]
fn hsum(v: __m128i) -> u32 {
    let s = _mm_add_epi32(v, _mm_shuffle_epi32::<0b01_00_11_10>(v));
    let s = _mm_add_epi32(s, _mm_shuffle_epi32::<0b00_00_00_01>(s));
    _mm_cvtsi128_si32(s) as u32
}

/// Four rows of selectors (i32 lanes holding 0..=3) as 16 bytes.
#[target_feature(enable = "sse2")]
#[inline]
fn store(q: V4, sels: &mut [u8; 16]) {
    let b = _mm_packus_epi16(_mm_packs_epi32(q[0], q[1]), _mm_packs_epi32(q[2], q[3]));
    sels[..8].copy_from_slice(&(_mm_cvtsi128_si64(b) as u64).to_le_bytes());
    sels[8..].copy_from_slice(&(_mm_cvtsi128_si64(_mm_unpackhi_epi64(b, b)) as u64).to_le_bytes());
}

#[target_feature(enable = "sse2")]
fn sels4(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
    let p = dual_palette(false, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);
    let r = [
        rest(t, entry(t, &p, 0)),
        rest(t, entry(t, &p, 1)),
        rest(t, entry(t, &p, 2)),
        rest(t, entry(t, &p, 3)),
    ];
    let zero = _mm_setzero_si128();
    let (mut least, mut sum) = ([zero; 4], zero);
    // The index is the row of several arrays at once.
    #[allow(clippy::needless_range_loop)]
    for v in 0..4 {
        least[v] = min32(min32(r[0][v], r[1][v]), min32(r[2][v], r[3][v]));
        sum = _mm_add_epi32(sum, _mm_add_epi32(t.t[v], least[v]));
    }
    let total = hsum(sum);
    if total < cutoff {
        let (one, three) = (_mm_set1_epi32(1), _mm_set1_epi32(3));
        let mut q = [zero; 4];
        #[allow(clippy::needless_range_loop)]
        for v in 0..4 {
            let eq = |j: usize| _mm_cmpeq_epi32(r[j][v], least[v]);
            let (eq0, eq1, eq2, eq3) = (eq(0), eq(1), eq(2), eq(3));
            // The first of entries 0, 1, 2 holding the least, else 3. A true compare is -1: 3 + eq2 is 2 or 3.
            let s23 = _mm_add_epi32(three, eq2);
            let s123 = _mm_or_si128(_mm_and_si128(eq1, one), _mm_andnot_si128(eq1, s23));
            let first = _mm_andnot_si128(eq0, s123);
            // Selector 3 wins a tie unless the error is 0.
            let exact = _mm_cmpeq_epi32(_mm_add_epi32(t.t[v], least[v]), zero);
            let take3 = _mm_andnot_si128(exact, eq3);
            q[v] = _mm_or_si128(_mm_and_si128(take3, three), _mm_andnot_si128(take3, first));
        }
        store(q, sels);
    }
    total
}

#[target_feature(enable = "sse2")]
fn sels3(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
    let p = dual_palette(true, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);
    let r = [
        rest(t, entry(t, &p, 0)),
        rest(t, entry(t, &p, 1)),
        rest(t, entry(t, &p, 2)),
    ];
    let zero = _mm_setzero_si128();
    let (mut least, mut sum) = ([zero; 4], zero);
    #[allow(clippy::needless_range_loop)]
    for v in 0..4 {
        least[v] = min32(min32(r[0][v], r[1][v]), r[2][v]);
        sum = _mm_add_epi32(sum, _mm_add_epi32(t.t[v], least[v]));
    }
    let total = hsum(sum);
    if total < cutoff {
        let two = _mm_set1_epi32(2);
        let mut q = [zero; 4];
        #[allow(clippy::needless_range_loop)]
        for v in 0..4 {
            let eq = |j: usize| _mm_cmpeq_epi32(r[j][v], least[v]);
            // 0 where entry 0 holds the least, else 1 where entry 1 does, else 2 (a true compare is -1).
            q[v] = _mm_andnot_si128(eq(0), _mm_add_epi32(two, eq(1)));
        }
        store(q, sels);
    }
    total
}

/// [`sels3`] with no cutoff and a block's transparent texels dropped: `keep` zeroes a transparent texel's error and
/// puts selector 3 in its lane. The total only gets smaller than `sels3`'s, so the bounds of the module doc hold.
#[target_feature(enable = "sse2")]
fn sels3_masked(t: &Texels, e: &Ends, sels: &mut [u8; 16]) -> u32 {
    let p = dual_palette(true, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);
    let r = [
        rest(t, entry(t, &p, 0)),
        rest(t, entry(t, &p, 1)),
        rest(t, entry(t, &p, 2)),
    ];
    let (zero, two, three) = (_mm_setzero_si128(), _mm_set1_epi32(2), _mm_set1_epi32(3));
    let mut sum = zero;
    let mut q = [zero; 4];
    #[allow(clippy::needless_range_loop)]
    for v in 0..4 {
        let least = min32(min32(r[0][v], r[1][v]), r[2][v]);
        sum = _mm_add_epi32(sum, _mm_and_si128(t.keep[v], _mm_add_epi32(t.t[v], least)));
        let eq = |j: usize| _mm_cmpeq_epi32(r[j][v], least);
        // As `sels3`: 0 where entry 0 holds the least, else 1 where entry 1 does, else 2; a transparent texel is 3.
        let first = _mm_andnot_si128(eq(0), _mm_add_epi32(two, eq(1)));
        q[v] = _mm_or_si128(
            _mm_and_si128(t.keep[v], first),
            _mm_andnot_si128(t.keep[v], three),
        );
    }
    store(q, sels);
    hsum(sum)
}

pub(super) fn find_sels3_masked(t: &Texels, e: &Ends, sels: &mut [u8; 16]) -> u32 {
    // SAFETY: this module is compiled only where SSE2 is enabled for the target (the `cfg` on `mod sse2`).
    unsafe { sels3_masked(t, e, sels) }
}

pub(super) fn find_sels4(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
    // SAFETY: this module is compiled only where SSE2 is enabled for the target (the `cfg` on `mod sse2`).
    unsafe { sels4(t, e, sels, cutoff) }
}

pub(super) fn find_sels3(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
    // SAFETY: this module is compiled only where SSE2 is enabled for the target (the `cfg` on `mod sse2`).
    unsafe { sels3(t, e, sels, cutoff) }
}
