//! The selector kernel of `sse2.rs` in 256-bit vectors, for the AVX2 path. Vector `w` of each [`Texels`] field holds
//! row `2w` (texels `8w..8w+3`) in its low half and row `2w + 1` (texels `8w+4..8w+7`) in its high half, so a block is
//! two vectors where the SSE2 kernel has four. The kernels are called only through the safe wrappers at the end, which
//! take the `Avx2` token. Integer code only.
//!
//! Each 32-bit lane runs the SSE2 kernel's steps on the same texel: `vpmaddwd`, the adds, compares and blends work per
//! lane, and `_mm256_min_epi32` is the signed minimum `min32` computes. So each lane holds the SSE2 kernel's bits, and
//! the bounds of `sse2.rs`'s module doc hold unchanged. The total adds the same 16 texel errors in another order (two
//! rows per lane, then the two halves, then `hsum`); 32-bit addition wraps, and the sum mod 2^32 does not depend on the
//! order, so with the true total below 2^32 (`sse2.rs`) it reads back as the same `u32`.
//!
//! The selectors are stored by splitting each vector back into its two rows and calling the SSE2 `store`: the 256-bit
//! `packs` and `packus` work within each 128-bit half, so packing whole vectors would interleave the rows.

use super::super::palette::dual_palette;
use super::super::Ends;
use super::sse2::{entry, hsum, store, Texels};
use crate::path::Avx2;
use core::arch::x86_64::*;

/// A block's 16 lanes as two vectors of 8 (the layout of the module doc).
type V2 = [__m256i; 2];

/// Rows `2w` and `2w + 1` of a `Texels` field as vector `w`.
#[target_feature(enable = "avx2")]
#[inline]
fn join(rows: &[__m128i; 4]) -> V2 {
    [
        _mm256_set_m128i(rows[1], rows[0]),
        _mm256_set_m128i(rows[3], rows[2]),
    ]
}

/// The SSE2 `rest` of one palette entry, 8 texels per vector.
#[target_feature(enable = "avx2")]
#[inline]
fn rest(xa: &V2, xb: &V2, e: (i32, i32, i32)) -> V2 {
    let (ma, mb, k) = (
        _mm256_set1_epi32(e.0),
        _mm256_set1_epi32(e.1),
        _mm256_set1_epi32(e.2),
    );
    let f = |w: usize| {
        _mm256_add_epi32(
            k,
            _mm256_add_epi32(_mm256_madd_epi16(xa[w], ma), _mm256_madd_epi16(xb[w], mb)),
        )
    };
    [f(0), f(1)]
}

/// The eight i32 lanes summed, wrapping, read as u32.
#[target_feature(enable = "avx2")]
#[inline]
fn hsum8(v: __m256i) -> u32 {
    hsum(_mm_add_epi32(
        _mm256_castsi256_si128(v),
        _mm256_extracti128_si256::<1>(v),
    ))
}

/// The selectors of two vectors as 16 bytes: each vector split back into its two rows, in texel order, for the SSE2
/// `store`.
#[target_feature(enable = "avx2")]
#[inline]
fn store8(q: V2, sels: &mut [u8; 16]) {
    store(
        [
            _mm256_castsi256_si128(q[0]),
            _mm256_extracti128_si256::<1>(q[0]),
            _mm256_castsi256_si128(q[1]),
            _mm256_extracti128_si256::<1>(q[1]),
        ],
        sels,
    );
}

/// The SSE2 `sels4`, 8 texels per vector.
#[target_feature(enable = "avx2")]
#[inline]
fn sels4(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
    let p = dual_palette(false, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);
    let (xa, xb, tt) = (join(&t.xa), join(&t.xb), join(&t.t));
    let r = [
        rest(&xa, &xb, entry(t, &p, 0)),
        rest(&xa, &xb, entry(t, &p, 1)),
        rest(&xa, &xb, entry(t, &p, 2)),
        rest(&xa, &xb, entry(t, &p, 3)),
    ];
    let zero = _mm256_setzero_si256();
    let (mut least, mut sum) = ([zero; 2], zero);
    for w in 0..2 {
        least[w] = _mm256_min_epi32(
            _mm256_min_epi32(r[0][w], r[1][w]),
            _mm256_min_epi32(r[2][w], r[3][w]),
        );
        sum = _mm256_add_epi32(sum, _mm256_add_epi32(tt[w], least[w]));
    }
    let total = hsum8(sum);
    if total < cutoff {
        let (one, three) = (_mm256_set1_epi32(1), _mm256_set1_epi32(3));
        let mut q = [zero; 2];
        for w in 0..2 {
            let eq = |j: usize| _mm256_cmpeq_epi32(r[j][w], least[w]);
            let (eq0, eq1, eq2, eq3) = (eq(0), eq(1), eq(2), eq(3));
            // As `sse2.rs`: the first of entries 0, 1, 2 holding the least, else 3; 3 + eq2 is 2 or 3.
            let s23 = _mm256_add_epi32(three, eq2);
            let s123 = _mm256_or_si256(_mm256_and_si256(eq1, one), _mm256_andnot_si256(eq1, s23));
            let first = _mm256_andnot_si256(eq0, s123);
            // Selector 3 wins a tie unless the error is 0.
            let exact = _mm256_cmpeq_epi32(_mm256_add_epi32(tt[w], least[w]), zero);
            let take3 = _mm256_andnot_si256(exact, eq3);
            q[w] = _mm256_or_si256(
                _mm256_and_si256(take3, three),
                _mm256_andnot_si256(take3, first),
            );
        }
        store8(q, sels);
    }
    total
}

/// The SSE2 `sels3`, 8 texels per vector: per texel the least error over palette entries 0, 1 and 2 and its selector
/// (the first entry on a tie), each lane within the bounds of `sse2.rs`'s module doc.
#[target_feature(enable = "avx2")]
#[inline]
fn sels3(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
    let p = dual_palette(true, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);
    let (xa, xb, tt) = (join(&t.xa), join(&t.xb), join(&t.t));
    let r = [
        rest(&xa, &xb, entry(t, &p, 0)),
        rest(&xa, &xb, entry(t, &p, 1)),
        rest(&xa, &xb, entry(t, &p, 2)),
    ];
    let zero = _mm256_setzero_si256();
    let (mut least, mut sum) = ([zero; 2], zero);
    for w in 0..2 {
        least[w] = _mm256_min_epi32(_mm256_min_epi32(r[0][w], r[1][w]), r[2][w]);
        sum = _mm256_add_epi32(sum, _mm256_add_epi32(tt[w], least[w]));
    }
    let total = hsum8(sum);
    if total < cutoff {
        let two = _mm256_set1_epi32(2);
        let mut q = [zero; 2];
        for w in 0..2 {
            let eq = |j: usize| _mm256_cmpeq_epi32(r[j][w], least[w]);
            // 0 where entry 0 holds the least, else 1 where entry 1 does, else 2 (a true compare is -1).
            q[w] = _mm256_andnot_si256(eq(0), _mm256_add_epi32(two, eq(1)));
        }
        store8(q, sels);
    }
    total
}

/// The SSE2 `sels3_masked`, 8 texels per vector: `keep` zeroes a transparent texel's error and puts selector 3 in its
/// lane.
#[target_feature(enable = "avx2")]
#[inline]
fn sels3_masked(t: &Texels, e: &Ends, sels: &mut [u8; 16]) -> u32 {
    let p = dual_palette(true, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);
    let (xa, xb, tt, keep) = (join(&t.xa), join(&t.xb), join(&t.t), join(&t.keep));
    let r = [
        rest(&xa, &xb, entry(t, &p, 0)),
        rest(&xa, &xb, entry(t, &p, 1)),
        rest(&xa, &xb, entry(t, &p, 2)),
    ];
    let (zero, two, three) = (
        _mm256_setzero_si256(),
        _mm256_set1_epi32(2),
        _mm256_set1_epi32(3),
    );
    let mut sum = zero;
    let mut q = [zero; 2];
    for w in 0..2 {
        let least = _mm256_min_epi32(_mm256_min_epi32(r[0][w], r[1][w]), r[2][w]);
        sum = _mm256_add_epi32(
            sum,
            _mm256_and_si256(keep[w], _mm256_add_epi32(tt[w], least)),
        );
        let eq = |j: usize| _mm256_cmpeq_epi32(r[j][w], least);
        let first = _mm256_andnot_si256(eq(0), _mm256_add_epi32(two, eq(1)));
        q[w] = _mm256_or_si256(
            _mm256_and_si256(keep[w], first),
            _mm256_andnot_si256(keep[w], three),
        );
    }
    store8(q, sels);
    hsum8(sum)
}

// The safe wrappers: the token proves the CPU has AVX2. Each counts its runs in the unit tests.

/// [`sels4`] on the AVX2 path.
#[inline]
pub(super) fn find_sels4(
    _tok: Avx2,
    t: &Texels,
    e: &Ends,
    sels: &mut [u8; 16],
    cutoff: u32,
) -> u32 {
    #[cfg(test)]
    crate::path::kernel_ran(crate::path::Wide::Sels4);
    // SAFETY: `_tok` exists, so `Avx2::require` found AVX2.
    unsafe { sels4(t, e, sels, cutoff) }
}

/// [`sels3`] on the AVX2 path.
#[inline]
pub(super) fn find_sels3(
    _tok: Avx2,
    t: &Texels,
    e: &Ends,
    sels: &mut [u8; 16],
    cutoff: u32,
) -> u32 {
    #[cfg(test)]
    crate::path::kernel_ran(crate::path::Wide::Sels3);
    // SAFETY: `_tok` exists, so `Avx2::require` found AVX2.
    unsafe { sels3(t, e, sels, cutoff) }
}

/// [`sels3_masked`] on the AVX2 path.
#[inline]
pub(super) fn find_sels3_masked(_tok: Avx2, t: &Texels, e: &Ends, sels: &mut [u8; 16]) -> u32 {
    #[cfg(test)]
    crate::path::kernel_ran(crate::path::Wide::Sels3Masked);
    // SAFETY: `_tok` exists, so `Avx2::require` found AVX2.
    unsafe { sels3_masked(t, e, sels) }
}
