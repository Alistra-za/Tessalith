//! The ordering kernel: the rounded least-squares fit of each of a pass's orderings (the level's `orderings4`, at most
//! 128, in the 4-colour pass, its `orderings3`, at most 32, in the 3-colour pass), R, G and B in lanes 0, 1 and 2
//! of one SSE2 vector (lane 3 is 0 everywhere: 0 sums, scale 0, maximum 0, so it adds nothing to the key). Compiled only
//! where the target has SSE2 (the `cfg` on `mod sse2`), so the `target_feature(enable = "sse2")` functions here are
//! always callable; Rust still wants an `unsafe` block for the call from plain code, and `order_keys::keys4` and
//! `keys3` carry it (as does the test-only `round_key_of`). This is the crate's one float kernel: the scalar code it
//! replaces (`order_keys::scalar`, which runs `ls_endpoints4_ordered` or `ls_endpoints3_ordered` and
//! `precise_round_565` through `round_ls`) is its reference, and every real call in the unit tests also runs that code
//! and asserts the same keys (all `MAX_TOTAL_ORDERINGS4` of `keys4`, `MAX_TOTAL_ORDERINGS3` of `keys3`, the unfitted
//! tail 0 on both sides).
//!
//! Why it gives the scalar code's bits, step by step:
//! - Integers. With `S` the prefix sums of one channel, `S[16]` the block total (`sorted_sums` stores it there) and
//!   `f1 = o0`, `f2 = o0 + o1`, `f3 = o0 + o1 + o2` for the ordering's histogram `o`, the scalar
//!   `uq00 = (S[f2] - S[f1]) + 2 (S[f3] - S[f2]) + 3 (S[16] - S[f3])` expands to `3 total - (S[f1] + S[f2] + S[f3])`,
//!   and its `q10 = 3 total - uq00` is `S[f1] + S[f2] + S[f3]`. The kernel builds `q10` as that sum of three prefix
//!   sums and `uq00` as three times the total minus it. Each prefix sum is at most `16 x 255 = 4,080`, so `q10`, `uq00`
//!   and `3 total` are at most `12,240`, and no 32-bit lane wraps. Only integers are rearranged. The 3-colour fit
//!   (`keys3`) has two positions, `f1 = o0` and `f2 = o0 + o2` (BC1's 3-colour order along the axis is selector 0, 2,
//!   1): the scalar `uq00 = 2 (S[16] - S[f2]) + (S[f2] - S[f1])` expands to `2 total - (S[f1] + S[f2])`, and its
//!   `q10 = 2 total - uq00` is `S[f1] + S[f2]`, at most `8,160`. `q10` is two prefix sums and `uq00` is two times the
//!   total minus it.
//! - Conversions. Every integer below `2^24` converts to `f32` exactly, from `u32` (the scalar `as f32`) and from `i32`
//!   (`_mm_cvtepi32_ps`) alike, so both sides start from the same floats.
//! - The fit. `xl = iz00 * uq00 + iz01 * q10` and `xh = iz10 * uq00 + iz11 * q10` (`iz01 = iz10`) are two
//!   `_mm_mul_ps` and one `_mm_add_ps` each, the same two products and one sum per lane as the scalar `mulss` and
//!   `addss` (IEEE products and sums are commutative, and Rust never fuses them into an FMA). The code is one more
//!   product, with the scale (31, 63, 31, 0), as the scalar `x * 31.0` and `x * 63.0`.
//! - Truncation. `|iz|` is at most `3/255 x 144` (a non-zero determinant is an integer at least 1, an entry of the
//!   matrix at most 144), so each product is at most `3/255 x 144 x 12,240 = 20,736`, `|x|` at most 41,472 and
//!   `|x x 63|` at most 2,612,736: far inside `i32`, where `_mm_cvttps_epi32` truncates toward zero exactly as `as i32`
//!   does (the doc of `precise_round_565` bounds the same values). No value is NaN. The 3-colour fit is inside the same
//!   limit: its matrix entries (`4 h0 + h2`, `h2`, `4 h1 + h2` for `WEIGHT_VALS3`, with `h0 + h1 + h2 = 16`) are at
//!   most 64, so `|iz|` is at most `2/255 x 64` (the scale is 2/255 and a non-zero determinant is an integer at least
//!   1; `precise_round_565` states the tighter `/ 4`). Its inputs are `q10` and `uq00`, at most `2 x 16 x 255 = 8,160`,
//!   so each product is at most `2/255 x 64 x 8,160 = 4,096`, `|x|` at most 8,192 and `|x x 63|` at most 516,096,
//!   under the 4-colour limit above.
//! - Clamp. Per lane, a negative code becomes 0 and one above the maximum the maximum. `precise_round_565` clamps a
//!   group (the four red and blue codes, or the two green ones) when any of it is out of range, but `clamp_code` leaves
//!   a code in range alone, so its result is the per-lane clamp.
//! - Rounding step. The thresholds are gathered by code from `MIDPOINT5` and `MIDPOINT6` into one vector, compared
//!   with `_mm_cmpgt_ps` (the scalar `>`), and the mask (-1 or 0) subtracted, then masked with the maximum as `& 31`
//!   and `& 63` do. The codes index the tables at most at 31 and 63. Lane 3 is 0.0 against a 0.0 threshold: no step.
//! - Key. The six codes (each at most 63) are packed to 16-bit lanes, codes of `xh` first (they are the low endpoint,
//!   as `round_ls` stores them) and codes of `xl` second, and one `_mm_madd_epi16` against (1, 32, 2048, 0) per half
//!   gives `r + 32 g` and `2048 b` (at most 63,488); their sum is a key half `r | g << 5 | b << 11`, at most 65,535,
//!   below `2^16`. The key is the low half plus the high half shifted by 16: `Seen::key` of `round_ls`'s pair.
//!
//! The kernel also fits the single-colour orderings (`TOTAL_ORDER_4_16`, the orderings with all 16 texels on one
//! selector) and throws the result away. Their factors are all zero (`compute_selector_factors4` stores 0 for a
//! singular matrix), so that fit is 0.0 in every lane, finite, and its codes index the `MIDPOINT` tables at 0. An
//! ordering is single-colour exactly when one of its four counts is 16; the counts sum to 16, so that is when bit 4 of
//! their OR is set. That this picks exactly the orderings of `TOTAL_ORDER_4_16` is checked over all 969 orderings by
//! `every_ordering_matches_the_reference`, which also compares the kernel with the reference at the largest sums (a
//! block of 255s). `keys3` does the same with `TOTAL_ORDER_3_16` (`compute_selector_factors3` stores 0 likewise), three
//! counts, and `every_3_colour_ordering_matches_the_reference` over all 153 orderings.

use super::super::fit::{OrderTables, MIDPOINT5, MIDPOINT6};
use super::super::params::params;
use super::super::tables::{MAX_TOTAL_ORDERINGS4, UNIQUE_TOTAL_ORDERS3, UNIQUE_TOTAL_ORDERS4};
use super::MAX_TOTAL_ORDERINGS3;
use core::arch::x86_64::*;

/// `order_keys::keys4` on SSE2 vectors.
#[target_feature(enable = "sse2")]
pub(super) fn keys4<const LEVEL: u32>(
    total: [i32; 3],
    sums: &[[u32; 17]; 3],
    row: &[u16; MAX_TOTAL_ORDERINGS4],
    tables: &OrderTables,
    single_key: u32,
) -> [u32; MAX_TOTAL_ORDERINGS4] {
    let n = const { params(LEVEL).orderings4 };
    let v = prefix_vectors(sums);
    let t3 = _mm_set_epi32(0, total[2] * 3, total[1] * 3, total[0] * 3);

    let mut keys = [0u32; MAX_TOTAL_ORDERINGS4];
    for (key, &s) in keys.iter_mut().zip(row).take(n) {
        let s = s as usize;
        let [o0, o1, o2, o3] = UNIQUE_TOTAL_ORDERS4[s];
        let (f1, f2, f3) = (o0 as usize, (o0 + o1) as usize, (o0 + o1 + o2) as usize);
        let q10 = _mm_add_epi32(_mm_add_epi32(v[f1], v[f2]), v[f3]);
        let uq00 = _mm_sub_epi32(t3, q10);
        let fitted = fit_key(q10, uq00, tables.factors[s]);
        *key = if (o0 | o1 | o2 | o3) & 16 != 0 {
            single_key
        } else {
            fitted
        };
    }
    keys
}

/// `order_keys::keys3` on SSE2 vectors: the same fit with two positions. `f1 = o0` and `f2 = o0 + o2` (BC1's 3-colour
/// order along the axis is selector 0, 2, 1), `q10 = S[f1] + S[f2]` and `uq00 = 2 total - q10`.
#[target_feature(enable = "sse2")]
pub(super) fn keys3<const LEVEL: u32>(
    total: [i32; 3],
    sums: &[[u32; 17]; 3],
    row: &[u8; MAX_TOTAL_ORDERINGS3],
    tables: &OrderTables,
    single_key: u32,
) -> [u32; MAX_TOTAL_ORDERINGS3] {
    let n = const { params(LEVEL).orderings3 };
    let v = prefix_vectors(sums);
    let t2 = _mm_set_epi32(0, total[2] * 2, total[1] * 2, total[0] * 2);

    let mut keys = [0u32; MAX_TOTAL_ORDERINGS3];
    for (key, &s) in keys.iter_mut().zip(row).take(n) {
        let s = s as usize;
        let [o0, o1, o2] = UNIQUE_TOTAL_ORDERS3[s];
        let (f1, f2) = (o0 as usize, (o0 + o2) as usize);
        let q10 = _mm_add_epi32(v[f1], v[f2]);
        let uq00 = _mm_sub_epi32(t2, q10);
        let fitted = fit_key(q10, uq00, tables.factors3[s]);
        *key = if (o0 | o1 | o2) & 16 != 0 {
            single_key
        } else {
            fitted
        };
    }
    keys
}

/// Prefix sum `i` of the three channels as one vector. Every sum is at most 4,080, so `as i32` is exact.
#[target_feature(enable = "sse2")]
#[inline]
fn prefix_vectors(sums: &[[u32; 17]; 3]) -> [__m128i; 17] {
    std::array::from_fn(|i| {
        _mm_set_epi32(0, sums[2][i] as i32, sums[1][i] as i32, sums[0][i] as i32)
    })
}

/// The key of the rounded fit of one ordering with factors `iz` (iz00, iz10, iz11), from its `q10` and `uq00`.
#[target_feature(enable = "sse2")]
#[inline]
fn fit_key(q10: __m128i, uq00: __m128i, iz: [f32; 3]) -> u32 {
    let [iz00, iz10, iz11] = iz;
    let iz01 = iz10;
    let (q, uq) = (_mm_cvtepi32_ps(q10), _mm_cvtepi32_ps(uq00));
    let xl = _mm_add_ps(
        _mm_mul_ps(_mm_set1_ps(iz00), uq),
        _mm_mul_ps(_mm_set1_ps(iz01), q),
    );
    let xh = _mm_add_ps(
        _mm_mul_ps(_mm_set1_ps(iz10), uq),
        _mm_mul_ps(_mm_set1_ps(iz11), q),
    );
    round_key(xl, xh)
}

/// `Seen::key(&round_ls(&xl, &xh))` for lanes 0, 1, 2 of `xl` and `xh` (lane 3 must be 0.0).
#[target_feature(enable = "sse2")]
#[inline]
fn round_key(xl: __m128, xh: __m128) -> u32 {
    let scale = _mm_set_ps(0.0, 31.0, 63.0, 31.0);
    let max = _mm_set_epi32(0, 31, 63, 31);
    let codes = |x: __m128| {
        let t = _mm_cvttps_epi32(_mm_mul_ps(x, scale));
        // A negative code becomes 0 (and-not of its sign), one above the maximum the maximum.
        let t = _mm_andnot_si128(_mm_srai_epi32::<31>(t), t);
        let over = _mm_cmpgt_epi32(t, max);
        let t = _mm_or_si128(_mm_and_si128(over, max), _mm_andnot_si128(over, t));
        // The codes are 0..=63 now, so the low 16 bits of each 32-bit lane are the code.
        let (r, g, b) = (
            _mm_extract_epi16::<0>(t) as usize,
            _mm_extract_epi16::<2>(t) as usize,
            _mm_extract_epi16::<4>(t) as usize,
        );
        let threshold = _mm_set_ps(0.0, MIDPOINT5[b], MIDPOINT6[g], MIDPOINT5[r]);
        let step = _mm_castps_si128(_mm_cmpgt_ps(x, threshold));
        _mm_and_si128(_mm_sub_epi32(t, step), max)
    };
    // 16-bit lanes: the low endpoint (codes of `xh`) r, g, b, 0, then the high endpoint (codes of `xl`).
    let both = _mm_packs_epi32(codes(xh), codes(xl));
    let m = _mm_madd_epi16(both, _mm_set_epi16(0, 2048, 32, 1, 0, 2048, 32, 1));
    // 32-bit lanes: r + 32 g and 2048 b of the low endpoint, then of the high one; add each pair.
    let halves = _mm_add_epi32(m, _mm_srli_epi64::<32>(m));
    let low = _mm_cvtsi128_si32(halves) as u32;
    let high = _mm_cvtsi128_si32(_mm_srli_si128::<8>(halves)) as u32;
    low | high << 16
}

/// [`round_key`] of two colours (lane 3 0.0), for the tests.
#[cfg(test)]
pub(super) fn round_key_of(xl: [f32; 3], xh: [f32; 3]) -> u32 {
    // SAFETY: this module is compiled only where SSE2 is enabled for the target (the `cfg` on `mod sse2`).
    unsafe {
        round_key(
            _mm_set_ps(0.0, xl[2], xl[1], xl[0]),
            _mm_set_ps(0.0, xh[2], xh[1], xh[0]),
        )
    }
}
