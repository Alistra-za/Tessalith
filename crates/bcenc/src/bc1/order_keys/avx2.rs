//! The ordering kernel of `sse2.rs` in 256-bit vectors, for the AVX2 path: two orderings per vector, ordering `2k` of
//! the row in the low 128-bit half and ordering `2k + 1` in the high half, each half laid out as the SSE2 vector (R, G
//! and B in its lanes 0, 1 and 2, its lane 3 0 everywhere). The kernels are called only through the safe wrappers at
//! the end, which take the `Avx2` token.
//!
//! Each half runs the SSE2 kernel's per-lane steps on its own ordering, so each lane holds the SSE2 kernel's bits, and
//! the argument of `sse2.rs`'s module doc holds per lane unchanged (Note HY13): `q10` is the sum of the same prefix
//! sums (built by the SSE2 `prefix_vectors`, one `__m128i` per ordering, joined with `_mm256_set_m128i`) and `uq00`
//! the same multiple of the total minus it, with the same bounds (at most 12,240, no wrap, exact in `f32`); the fit is
//! the same two products and one sum per lane (`_mm256_mul_ps`, `_mm256_add_ps`, with each half's factors broadcast by
//! `_mm256_set_m128(_mm_set1_ps(b), _mm_set1_ps(a))`); the truncation (`_mm256_cvttps_epi32`) sees the same values
//! inside `i32`; the clamp is per lane; the rounding step compares with `_mm256_cmp_ps::<_CMP_GT_OS>`, the predicate
//! of `_mm_cmpgt_ps`, against thresholds gathered per half from that half's codes. These are the only float operations:
//! the 256-bit forms of the SSE2 kernel's, plus `_mm256_set_m128` and `_mm_set1_ps` to build the factors.
//!
//! The key. `_mm256_packs_epi32(codes(xh), codes(xl))` packs within each 128-bit half, which is safe here because each
//! ordering lives inside one half: the low half of the result is the SSE2 `both` of ordering `2k` and the high half
//! that of ordering `2k + 1`. `_mm256_madd_epi16` and the 64-bit shift work per half as well, so the key halves of
//! ordering `2k` are 32-bit lanes 0 and 2 and those of `2k + 1` lanes 4 and 6, each within the SSE2 bounds (a key half
//! at most 65,535).
//!
//! Every level's `orderings4` and `orderings3` is even (the `const` check below), so the pair loop leaves no ordering
//! alone.

use super::super::fit::{OrderTables, MIDPOINT5, MIDPOINT6};
use super::super::params::{params, LEVELS};
use super::super::tables::{MAX_TOTAL_ORDERINGS4, UNIQUE_TOTAL_ORDERS3, UNIQUE_TOTAL_ORDERS4};
use super::sse2::prefix_vectors;
use super::MAX_TOTAL_ORDERINGS3;
use crate::path::Avx2;
use core::arch::x86_64::*;

// The pair loop fits two orderings at a time: every level tries an even count of each.
const _: () = {
    let mut i = 0;
    while i < LEVELS.len() {
        let p = params(LEVELS[i]);
        assert!(p.orderings4.is_multiple_of(2) && p.orderings3.is_multiple_of(2));
        i += 1;
    }
};

/// `order_keys::keys4` with two orderings per vector.
#[target_feature(enable = "avx2")]
#[inline]
fn keys4<const LEVEL: u32>(
    total: [i32; 3],
    sums: &[[u32; 17]; 3],
    row: &[u16; MAX_TOTAL_ORDERINGS4],
    tables: &OrderTables,
    single_key: u32,
) -> [u32; MAX_TOTAL_ORDERINGS4] {
    let n = const { params(LEVEL).orderings4 };
    let v = prefix_vectors(sums);
    let t3 = _mm_set_epi32(0, total[2] * 3, total[1] * 3, total[0] * 3);
    let t3 = _mm256_set_m128i(t3, t3);

    let mut keys = [0u32; MAX_TOTAL_ORDERINGS4];
    let pairs = keys[..n].as_chunks_mut::<2>().0.iter_mut();
    for (pair, &[sa, sb]) in pairs.zip(row[..n].as_chunks::<2>().0) {
        let (sa, sb) = (sa as usize, sb as usize);
        let (oa, ob) = (UNIQUE_TOTAL_ORDERS4[sa], UNIQUE_TOTAL_ORDERS4[sb]);
        let q10 = |[o0, o1, o2, _]: [u8; 4]| {
            let (f1, f2, f3) = (o0 as usize, (o0 + o1) as usize, (o0 + o1 + o2) as usize);
            _mm_add_epi32(_mm_add_epi32(v[f1], v[f2]), v[f3])
        };
        let q10 = _mm256_set_m128i(q10(ob), q10(oa));
        let uq00 = _mm256_sub_epi32(t3, q10);
        let fitted = fit_key2(q10, uq00, tables.factors[sa], tables.factors[sb]);
        let single = |[o0, o1, o2, o3]: [u8; 4]| (o0 | o1 | o2 | o3) & 16 != 0;
        pair[0] = if single(oa) { single_key } else { fitted[0] };
        pair[1] = if single(ob) { single_key } else { fitted[1] };
    }
    keys
}

/// `order_keys::keys3` with two orderings per vector: the same fit with two positions, `f1 = o0` and `f2 = o0 + o2`,
/// `q10 = S[f1] + S[f2]` and `uq00 = 2 total - q10` (as the SSE2 `keys3`).
#[target_feature(enable = "avx2")]
#[inline]
fn keys3<const LEVEL: u32>(
    total: [i32; 3],
    sums: &[[u32; 17]; 3],
    row: &[u8; MAX_TOTAL_ORDERINGS3],
    tables: &OrderTables,
    single_key: u32,
) -> [u32; MAX_TOTAL_ORDERINGS3] {
    let n = const { params(LEVEL).orderings3 };
    let v = prefix_vectors(sums);
    let t2 = _mm_set_epi32(0, total[2] * 2, total[1] * 2, total[0] * 2);
    let t2 = _mm256_set_m128i(t2, t2);

    let mut keys = [0u32; MAX_TOTAL_ORDERINGS3];
    let pairs = keys[..n].as_chunks_mut::<2>().0.iter_mut();
    for (pair, &[sa, sb]) in pairs.zip(row[..n].as_chunks::<2>().0) {
        let (sa, sb) = (sa as usize, sb as usize);
        let (oa, ob) = (UNIQUE_TOTAL_ORDERS3[sa], UNIQUE_TOTAL_ORDERS3[sb]);
        let q10 = |[o0, _, o2]: [u8; 3]| _mm_add_epi32(v[o0 as usize], v[(o0 + o2) as usize]);
        let q10 = _mm256_set_m128i(q10(ob), q10(oa));
        let uq00 = _mm256_sub_epi32(t2, q10);
        let fitted = fit_key2(q10, uq00, tables.factors3[sa], tables.factors3[sb]);
        let single = |[o0, o1, o2]: [u8; 3]| (o0 | o1 | o2) & 16 != 0;
        pair[0] = if single(oa) { single_key } else { fitted[0] };
        pair[1] = if single(ob) { single_key } else { fitted[1] };
    }
    keys
}

/// The keys of the rounded fits of two orderings, from their `q10` and `uq00` (ordering `a` in the low half, `b` in
/// the high half) and their factors `iz_a`, `iz_b` (iz00, iz10, iz11).
#[target_feature(enable = "avx2")]
#[inline]
fn fit_key2(q10: __m256i, uq00: __m256i, iz_a: [f32; 3], iz_b: [f32; 3]) -> [u32; 2] {
    // Factor `i` of `a` in the low half, of `b` in the high half.
    let f = |i: usize| _mm256_set_m128(_mm_set1_ps(iz_b[i]), _mm_set1_ps(iz_a[i]));
    let (iz00, iz10, iz11) = (f(0), f(1), f(2));
    let iz01 = iz10;
    let (q, uq) = (_mm256_cvtepi32_ps(q10), _mm256_cvtepi32_ps(uq00));
    let xl = _mm256_add_ps(_mm256_mul_ps(iz00, uq), _mm256_mul_ps(iz01, q));
    let xh = _mm256_add_ps(_mm256_mul_ps(iz10, uq), _mm256_mul_ps(iz11, q));
    round_key2(xl, xh)
}

/// The SSE2 `round_key` of each half: `Seen::key(&round_ls(&xl, &xh))` for lanes 0, 1, 2 (low half) and 4, 5, 6 (high
/// half) of `xl` and `xh` (lanes 3 and 7 must be 0.0).
#[target_feature(enable = "avx2")]
#[inline]
fn round_key2(xl: __m256, xh: __m256) -> [u32; 2] {
    let scale = _mm256_set_ps(0.0, 31.0, 63.0, 31.0, 0.0, 31.0, 63.0, 31.0);
    let max = _mm256_set_epi32(0, 31, 63, 31, 0, 31, 63, 31);
    let codes = |x: __m256| {
        let t = _mm256_cvttps_epi32(_mm256_mul_ps(x, scale));
        // A negative code becomes 0 (and-not of its sign), one above the maximum the maximum.
        let t = _mm256_andnot_si256(_mm256_srai_epi32::<31>(t), t);
        let over = _mm256_cmpgt_epi32(t, max);
        let t = _mm256_or_si256(_mm256_and_si256(over, max), _mm256_andnot_si256(over, t));
        // The codes are 0..=63 now, so the low 16 bits of each 32-bit lane are the code.
        let (r0, g0, b0) = (
            _mm256_extract_epi16::<0>(t) as usize,
            _mm256_extract_epi16::<2>(t) as usize,
            _mm256_extract_epi16::<4>(t) as usize,
        );
        let (r1, g1, b1) = (
            _mm256_extract_epi16::<8>(t) as usize,
            _mm256_extract_epi16::<10>(t) as usize,
            _mm256_extract_epi16::<12>(t) as usize,
        );
        let threshold = _mm256_set_ps(
            0.0,
            MIDPOINT5[b1],
            MIDPOINT6[g1],
            MIDPOINT5[r1],
            0.0,
            MIDPOINT5[b0],
            MIDPOINT6[g0],
            MIDPOINT5[r0],
        );
        let step = _mm256_castps_si256(_mm256_cmp_ps::<_CMP_GT_OS>(x, threshold));
        _mm256_and_si256(_mm256_sub_epi32(t, step), max)
    };
    // Per half, 16-bit lanes: the low endpoint (codes of `xh`) r, g, b, 0, then the high endpoint (codes of `xl`).
    let both = _mm256_packs_epi32(codes(xh), codes(xl));
    let m = _mm256_madd_epi16(
        both,
        _mm256_set_epi16(
            0, 2048, 32, 1, 0, 2048, 32, 1, 0, 2048, 32, 1, 0, 2048, 32, 1,
        ),
    );
    // Per half, 32-bit lanes: r + 32 g and 2048 b of the low endpoint, then of the high one; add each pair.
    let halves = _mm256_add_epi32(m, _mm256_srli_epi64::<32>(m));
    let key = |low: i32, high: i32| low as u32 | (high as u32) << 16;
    [
        key(
            _mm256_extract_epi32::<0>(halves),
            _mm256_extract_epi32::<2>(halves),
        ),
        key(
            _mm256_extract_epi32::<4>(halves),
            _mm256_extract_epi32::<6>(halves),
        ),
    ]
}

// The safe wrappers: the token proves the CPU has AVX2. Each counts its runs in the unit tests.

/// [`keys4`] on the AVX2 path.
#[inline]
pub(super) fn fit_keys4<const LEVEL: u32>(
    _tok: Avx2,
    total: [i32; 3],
    sums: &[[u32; 17]; 3],
    row: &[u16; MAX_TOTAL_ORDERINGS4],
    tables: &OrderTables,
    single_key: u32,
) -> [u32; MAX_TOTAL_ORDERINGS4] {
    #[cfg(test)]
    crate::path::kernel_ran(crate::path::Wide::Keys4);
    // SAFETY: `_tok` exists, so `Avx2::require` found AVX2.
    unsafe { keys4::<LEVEL>(total, sums, row, tables, single_key) }
}

/// [`keys3`] on the AVX2 path.
#[inline]
pub(super) fn fit_keys3<const LEVEL: u32>(
    _tok: Avx2,
    total: [i32; 3],
    sums: &[[u32; 17]; 3],
    row: &[u8; MAX_TOTAL_ORDERINGS3],
    tables: &OrderTables,
    single_key: u32,
) -> [u32; MAX_TOTAL_ORDERINGS3] {
    #[cfg(test)]
    crate::path::kernel_ran(crate::path::Wide::Keys3);
    // SAFETY: `_tok` exists, so `Avx2::require` found AVX2.
    unsafe { keys3::<LEVEL>(total, sums, row, tables, single_key) }
}

/// [`round_key2`] of two pairs of colours (lanes 3 and 7 0.0): pair `i` is (`xl[i]`, `xh[i]`), pair 0 in the low half.
/// For the tests; panics without AVX2.
#[cfg(test)]
pub(super) fn round_key2_of(xl: [[f32; 3]; 2], xh: [[f32; 3]; 2]) -> [u32; 2] {
    let _tok = Avx2::require();
    let v = |x: [[f32; 3]; 2]| {
        [
            x[0][0], x[0][1], x[0][2], 0.0, x[1][0], x[1][1], x[1][2], 0.0,
        ]
    };
    // SAFETY: `_tok` exists, so `Avx2::require` found AVX2.
    unsafe { round_key2_arrays(v(xl), v(xh)) }
}

/// [`round_key2`] of lanes given as arrays (lane 0 first), for [`round_key2_of`].
#[cfg(test)]
#[target_feature(enable = "avx2")]
fn round_key2_arrays(xl: [f32; 8], xh: [f32; 8]) -> [u32; 2] {
    let v = |x: [f32; 8]| _mm256_set_ps(x[7], x[6], x[5], x[4], x[3], x[2], x[1], x[0]);
    round_key2(v(xl), v(xh))
}
