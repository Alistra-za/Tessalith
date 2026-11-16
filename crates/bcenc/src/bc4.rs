//! The BC4 block search (docs/research/bc4-family.md): a port of `rgbcx::encode_bc4_hq` (rgbcx from bc7enc_rdo commit
//! b943862, MIT or public domain) with Tessalith's `BC4_SDC_SEARCH` and `BC4_SDC_GUARD128` patches, which are the
//! measured additions (6-value search on the inner range, least-squares refits, trimmed inner ranges, a local search).
//! Everything starts from the upstream result and replaces it only on a strictly lower SSE, so ties keep its bytes.
//! The vendored C++ is patched to equal [`search`] byte for byte, so the tie rules here are the contract: the first
//! minimum per texel, a strict `<` to replace the best, and candidates tried in the order written.
//! The test inputs `Rng` and `blocks` live in `tests/support/gen.rs`.
//!
//! Paths: [`search_on`] runs the search on the path it is given. `Base` runs it as plain code; `Avx2` runs it inside
//! `search_avx2`, a `#[target_feature(enable = "avx2")]` frame, where the search (`#[inline(always)]`, with the token
//! first) is compiled again with AVX2 (Note HY21). Each loop hands its candidates to a `Batch` where it called
//! `try_pair` before: on `Base` that is `try_pair`; on `Avx2` the batch scores two pairs at once in one 256-bit vector
//! (`avx2::pair2`) and decides them in order (`apply`), and a lone last pair goes through `try_pair` when the loop
//! flushes. `Avx512` runs the search inside `search_avx512`, a frame with the AVX-512 set, where the batch scores
//! four pairs at once in one 512-bit vector (`avx512::pair4`) and `flush` sends the last two through `pair2` and a
//! lone one through `try_pair`. No byte changes: each pair still gets `try_pair`'s verdict against the best of its
//! moment (a strict `<` on the full total, which is what `pair` returning `None` means), in the order written, and no
//! candidate of a loop is computed from the best inside that loop, so scoring later pairs before the earlier ones are
//! decided changes neither the pairs nor their order. Each loop flushes before anything reads its best.
// Index loops mirror the C++ they port.
#![allow(clippy::needless_range_loop)]

// `hit!` expands to nothing outside the unit tests, which leaves `Hit` unused there.
#[cfg_attr(not(test), allow(unused_imports))]
use crate::hits::{hit, Hit};
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
use crate::path::{Avx2, Avx512};
use crate::path::{Base, Isa, Path};

/// The bcdec / patched-rgbcx BC4 palette (`get_block_values`); checked against the FFI for all 65536 pairs.
#[inline(always)]
pub fn pal(e0: u8, e1: u8) -> [u8; 8] {
    let (l, h) = (e0 as u32, e1 as u32);
    let w = |a: u32, b: u32| ((a * l + b * h + 32768) >> 16) as u8;
    if l > h {
        [
            e0,
            e1,
            w(56173, 9363),
            w(46812, 18724),
            w(37450, 28086),
            w(28086, 37450),
            w(18724, 46812),
            w(9363, 56173),
        ]
    } else {
        [
            e0,
            e1,
            w(52429, 13107),
            w(39321, 26215),
            w(26215, 39321),
            w(13107, 52429),
            0,
            255,
        ]
    }
}

#[derive(Clone, Copy)]
pub struct Best {
    pub err: u32,
    pub e0: u8,
    pub e1: u8,
    pub sels: [u8; 16],
}

impl Default for Best {
    fn default() -> Self {
        Self::new()
    }
}

impl Best {
    pub fn new() -> Self {
        Best {
            err: u32::MAX,
            e0: 0,
            e1: 0,
            sels: [0; 16],
        }
    }
    pub fn bytes(&self) -> [u8; 8] {
        let mut bits = 0u64;
        for (i, &s) in self.sels.iter().enumerate() {
            bits |= (s as u64) << (3 * i);
        }
        let mut o = [0u8; 8];
        o[0] = self.e0;
        o[1] = self.e1;
        o[2..].copy_from_slice(&bits.to_le_bytes()[..6]);
        o
    }
    /// Try one endpoint pair exactly as rgbcx does (first minimum per texel, early out, strict improvement).
    #[inline(always)]
    pub fn try_pair(&mut self, v: &[u8; 16], e0: u8, e1: u8) {
        let p = pal(e0, e1);
        let Some((err, sels)) = pair(v, &p, self.err) else {
            hit!(Hit::PairEarlyOut);
            return;
        };
        hit!(Hit::PairImproved);
        *self = Best { err, e0, e1, sels };
    }
    /// `try_pair` for the test references: the scalar loop and no branch counters, so the references add nothing to
    /// what `every_ported_branch_is_taken` counts.
    #[cfg(test)]
    fn try_pair_reference(&mut self, v: &[u8; 16], e0: u8, e1: u8) {
        if let Some((err, sels)) = pair_scalar(v, &pal(e0, e1), self.err) {
            *self = Best { err, e0, e1, sels };
        }
    }
}

/// The block's error under palette `p` with the nearest palette value per texel (the first nearest one), and the
/// selectors; `None` when the error is not below `best`. The SSE2 kernel where the target has it, the scalar loop
/// elsewhere; in the unit tests every call also runs the scalar loop and asserts the same result.
///
/// Why the kernel equals the loop: squaring keeps the order of absolute differences, so the first least square is the
/// first least difference; the loop returns early exactly when its running total reaches `best`, because the total only
/// grows and ends at the block's total, so "not below `best`" is the same test on the full total.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline(always)]
fn pair(v: &[u8; 16], p: &[u8; 8], best: u32) -> Option<(u32, [u8; 16])> {
    let r = sse2::pair(v, p, best);
    #[cfg(test)]
    assert_eq!(r, pair_scalar(v, p, best), "{v:?} {p:?} {best}");
    r
}

#[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
#[inline(always)]
fn pair(v: &[u8; 16], p: &[u8; 8], best: u32) -> Option<(u32, [u8; 16])> {
    pair_scalar(v, p, best)
}

/// rgbcx's loop: per texel the first nearest palette value, the running total, an early return once it reaches `best`.
/// The kernel's test reference, and what a target without SSE2 runs.
#[cfg(any(test, not(all(target_arch = "x86_64", target_feature = "sse2"))))]
fn pair_scalar(v: &[u8; 16], p: &[u8; 8], best: u32) -> Option<(u32, [u8; 16])> {
    let mut err = 0u32;
    let mut sels = [0u8; 16];
    for i in 0..16 {
        let (mut be, mut bj) = (u32::MAX, 0u8);
        for (j, &pj) in p.iter().enumerate() {
            let d = pj as i32 - v[i] as i32;
            let e = (d * d) as u32;
            if e < be {
                be = e;
                bj = j as u8;
                if e == 0 {
                    break;
                }
            }
        }
        err += be;
        if err >= best {
            return None;
        }
        sels[i] = bj;
    }
    Some((err, sels))
}

/// The pair kernel: all 16 texels against all 8 palette values at once, in SSE2 integer intrinsics. Compiled only where
/// the target has SSE2, so its `target_feature(enable = "sse2")` function is always callable; Rust still wants an
/// `unsafe` block for the call from plain code. Integer code only.
///
/// Bounds (intrinsics wrap silently): a distance is at most 255, so as an `i16` lane at most 255; a `pmaddwd` lane
/// adds two squares, at most 2 x 255^2 = 130,050; adding the low and the high half gives four `i32` lanes of four
/// squares each, at most 260,100; their sum is at most 16 x 255^2 = 1,040,400, far below 2^31, so every addition is
/// exact and the `as u32` reads it back exactly.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod sse2 {
    use core::arch::x86_64::*;

    #[target_feature(enable = "sse2")]
    #[inline]
    fn pair_sse2(v: &[u8; 16], p: &[u8; 8], best: u32) -> Option<(u32, [u8; 16])> {
        let half = |a: &[u8]| i64::from_le_bytes([a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]]);
        let x = _mm_set_epi64x(half(&v[8..]), half(&v[..8]));
        // |x - q| per byte: one of the two saturating differences is 0, the other is the distance.
        let dist = |j: usize| {
            let q = _mm_set1_epi8(p[j] as i8);
            _mm_or_si128(_mm_subs_epu8(x, q), _mm_subs_epu8(q, x))
        };
        let d = [
            dist(0),
            dist(1),
            dist(2),
            dist(3),
            dist(4),
            dist(5),
            dist(6),
            dist(7),
        ];
        let mut m = d[0];
        for dj in &d[1..] {
            m = _mm_min_epu8(m, *dj);
        }
        // Sum of squares: the 16 distances (0..=255, so non-negative as i16) squared and added in pairs (at most
        // 130,050 a lane), low and high half added (four lanes of at most 260,100), then the four lanes added
        // together: the total is at most 1,040,400.
        let zero = _mm_setzero_si128();
        let (lo, hi) = (_mm_unpacklo_epi8(m, zero), _mm_unpackhi_epi8(m, zero));
        let s = _mm_add_epi32(_mm_madd_epi16(lo, lo), _mm_madd_epi16(hi, hi));
        let s = _mm_add_epi32(s, _mm_shuffle_epi32::<0b01_00_11_10>(s));
        let s = _mm_add_epi32(s, _mm_shuffle_epi32::<0b00_00_00_01>(s));
        debug_assert!((0..=1_040_400).contains(&_mm_cvtsi128_si32(s)));
        let err = _mm_cvtsi128_si32(s) as u32;
        if err >= best {
            return None;
        }
        // From the last palette value to the first, so the first nearest one is what stays.
        let mut sel = zero;
        for j in (0..8).rev() {
            let eq = _mm_cmpeq_epi8(d[j], m);
            sel = _mm_or_si128(
                _mm_and_si128(eq, _mm_set1_epi8(j as i8)),
                _mm_andnot_si128(eq, sel),
            );
        }
        let mut sels = [0u8; 16];
        sels[..8].copy_from_slice(&(_mm_cvtsi128_si64(sel) as u64).to_le_bytes());
        sels[8..].copy_from_slice(
            &(_mm_cvtsi128_si64(_mm_unpackhi_epi64(sel, sel)) as u64).to_le_bytes(),
        );
        Some((err, sels))
    }

    #[inline(always)]
    pub(super) fn pair(v: &[u8; 16], p: &[u8; 8], best: u32) -> Option<(u32, [u8; 16])> {
        // SAFETY: this module is compiled only where SSE2 is enabled for the target (the `cfg` on `mod sse2`).
        unsafe { pair_sse2(v, p, best) }
    }
}

/// Two endpoint pairs scored at once, for the AVX2 path: palette `pa` in the low 128-bit half of each vector and `pb`
/// in the high half, the block's 16 texels in both. Each half runs exactly the SSE2 kernel's steps (`mod sse2`): the
/// saturating differences or-ed into distances, their least per texel, the squares summed with `vpmaddwd` and the
/// adds, and the selectors blended from the last palette value to the first. The 256-bit `unpack`, `madd` and
/// `shuffle` work within each half, so neither half sees the other's lanes, each lane holds the SSE2 kernel's bits and
/// the bounds of that kernel's doc hold per half (a total at most 1,040,400). Unlike the SSE2 kernel it returns both
/// full totals and both selector sets; `apply` decides them.
///
/// Exactness: `pair` returns `None` exactly when the full total is not below the best (HY11), so two totals computed
/// first and then decided in order, each against the best of its moment, give each pair its old verdict; a tie is not
/// strictly lower, so the first pair keeps it. Integer code only; called only through the safe wrapper `pair2`, which
/// takes the `Avx2` token.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod avx2 {
    use crate::path::Avx2;
    use core::arch::x86_64::*;

    #[target_feature(enable = "avx2")]
    #[inline]
    fn pair2_kernel(v: &[u8; 16], pa: &[u8; 8], pb: &[u8; 8]) -> [(u32, [u8; 16]); 2] {
        let half = |a: &[u8]| i64::from_le_bytes([a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]]);
        let (lo8, hi8) = (half(&v[..8]), half(&v[8..]));
        let x = _mm256_set_epi64x(hi8, lo8, hi8, lo8);
        // |x - q| per byte, q holding palette a's value j in the low half and palette b's in the high half.
        let dist = |j: usize| {
            let q = _mm256_set_m128i(_mm_set1_epi8(pb[j] as i8), _mm_set1_epi8(pa[j] as i8));
            _mm256_or_si256(_mm256_subs_epu8(x, q), _mm256_subs_epu8(q, x))
        };
        let d = [
            dist(0),
            dist(1),
            dist(2),
            dist(3),
            dist(4),
            dist(5),
            dist(6),
            dist(7),
        ];
        let mut m = d[0];
        for dj in &d[1..] {
            m = _mm256_min_epu8(m, *dj);
        }
        // The SSE2 kernel's sum per half: `unpack` takes each half's own texels 0..8 and 8..16.
        let zero = _mm256_setzero_si256();
        let (lo, hi) = (_mm256_unpacklo_epi8(m, zero), _mm256_unpackhi_epi8(m, zero));
        let s = _mm256_add_epi32(_mm256_madd_epi16(lo, lo), _mm256_madd_epi16(hi, hi));
        let s = _mm256_add_epi32(s, _mm256_shuffle_epi32::<0b01_00_11_10>(s));
        let s = _mm256_add_epi32(s, _mm256_shuffle_epi32::<0b00_00_00_01>(s));
        let (sa, sb) = (_mm256_castsi256_si128(s), _mm256_extracti128_si256::<1>(s));
        debug_assert!((0..=1_040_400).contains(&_mm_cvtsi128_si32(sa)));
        debug_assert!((0..=1_040_400).contains(&_mm_cvtsi128_si32(sb)));
        // From the last palette value to the first, so the first nearest one is what stays.
        let mut sel = zero;
        for j in (0..8).rev() {
            let eq = _mm256_cmpeq_epi8(d[j], m);
            sel = _mm256_or_si256(
                _mm256_and_si256(eq, _mm256_set1_epi8(j as i8)),
                _mm256_andnot_si256(eq, sel),
            );
        }
        let bytes = |h: __m128i| {
            let mut sels = [0u8; 16];
            sels[..8].copy_from_slice(&(_mm_cvtsi128_si64(h) as u64).to_le_bytes());
            sels[8..].copy_from_slice(
                &(_mm_cvtsi128_si64(_mm_unpackhi_epi64(h, h)) as u64).to_le_bytes(),
            );
            sels
        };
        [
            (
                _mm_cvtsi128_si32(sa) as u32,
                bytes(_mm256_castsi256_si128(sel)),
            ),
            (
                _mm_cvtsi128_si32(sb) as u32,
                bytes(_mm256_extracti128_si256::<1>(sel)),
            ),
        ]
    }

    /// The full total and the selectors of the block under palette `pa` (`[0]`) and under `pb` (`[1]`), on the AVX2
    /// path. Counts its runs in the unit tests.
    #[inline]
    pub(super) fn pair2(
        _t: Avx2,
        v: &[u8; 16],
        pa: &[u8; 8],
        pb: &[u8; 8],
    ) -> [(u32, [u8; 16]); 2] {
        #[cfg(test)]
        crate::path::kernel_ran(crate::path::Wide::Pair2);
        // SAFETY: `_t` exists, so `Avx2::require` found AVX2.
        unsafe { pair2_kernel(v, pa, pb) }
    }
}

/// Four endpoint pairs scored at once, for the AVX-512 path: palette `p[k]` in the 128-bit quarter `k` of each
/// vector, the block's 16 texels in every quarter. Each quarter runs exactly the SSE2 kernel's steps (`mod sse2`) in
/// the `_mm512_*` byte and word forms: the saturating differences or-ed into distances, their least per texel, the
/// squares summed with `vpmaddwd` and the adds, and the selectors blended from the last palette value to the first
/// (`vpcmpeqb` into a mask and `vpblendmb`, where the SSE2 kernel ands and or-s with the compare's bytes; the same
/// bytes stay). The 512-bit `unpack`, `madd` and `shuffle` work within each 128-bit quarter, so no quarter sees
/// another's lanes, each lane holds the SSE2 kernel's bits and the bounds of that kernel's doc hold per quarter (a
/// total at most 1,040,400). Like `pair2` it returns every full total and selector set; `apply` decides them.
///
/// Exactness: as `pair2`'s (HY11): four totals computed first and then decided in order, each against the best of its
/// moment, give each pair its old verdict. Integer code only; called only through the safe wrapper `pair4`, which takes
/// the `Avx512` token.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod avx512 {
    use crate::path::Avx512;
    use core::arch::x86_64::*;

    #[target_feature(enable = "avx2,avx512f,avx512bw,avx512vl,avx512dq")]
    #[inline]
    fn pair4_kernel(v: &[u8; 16], p: [&[u8; 8]; 4]) -> [(u32, [u8; 16]); 4] {
        let half = |a: &[u8]| i64::from_le_bytes([a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]]);
        let (lo8, hi8) = (half(&v[..8]), half(&v[8..]));
        let x = _mm512_set_epi64(hi8, lo8, hi8, lo8, hi8, lo8, hi8, lo8);
        // |x - q| per byte, q holding palette k's value j in quarter k.
        let dist = |j: usize| {
            let b = |k: usize| i32::from_ne_bytes([p[k][j]; 4]);
            let (b0, b1, b2, b3) = (b(0), b(1), b(2), b(3));
            let q = _mm512_set_epi32(
                b3, b3, b3, b3, b2, b2, b2, b2, b1, b1, b1, b1, b0, b0, b0, b0,
            );
            _mm512_or_si512(_mm512_subs_epu8(x, q), _mm512_subs_epu8(q, x))
        };
        let d = [
            dist(0),
            dist(1),
            dist(2),
            dist(3),
            dist(4),
            dist(5),
            dist(6),
            dist(7),
        ];
        let mut m = d[0];
        for dj in &d[1..] {
            m = _mm512_min_epu8(m, *dj);
        }
        // The SSE2 kernel's sum per quarter: `unpack` takes each quarter's own texels 0..8 and 8..16.
        let zero = _mm512_setzero_si512();
        let (lo, hi) = (_mm512_unpacklo_epi8(m, zero), _mm512_unpackhi_epi8(m, zero));
        let s = _mm512_add_epi32(_mm512_madd_epi16(lo, lo), _mm512_madd_epi16(hi, hi));
        let s = _mm512_add_epi32(s, _mm512_shuffle_epi32::<0b01_00_11_10>(s));
        let s = _mm512_add_epi32(s, _mm512_shuffle_epi32::<0b00_00_00_01>(s));
        // From the last palette value to the first, so the first nearest one is what stays.
        let mut sel = zero;
        for j in (0..8).rev() {
            let eq = _mm512_cmpeq_epi8_mask(d[j], m);
            sel = _mm512_mask_blend_epi8(eq, sel, _mm512_set1_epi8(j as i8));
        }
        let quarters = |a: __m512i| {
            [
                _mm512_castsi512_si128(a),
                _mm512_extracti32x4_epi32::<1>(a),
                _mm512_extracti32x4_epi32::<2>(a),
                _mm512_extracti32x4_epi32::<3>(a),
            ]
        };
        let (sq, selq) = (quarters(s), quarters(sel));
        let bytes = |h: __m128i| {
            let mut sels = [0u8; 16];
            sels[..8].copy_from_slice(&(_mm_cvtsi128_si64(h) as u64).to_le_bytes());
            sels[8..].copy_from_slice(
                &(_mm_cvtsi128_si64(_mm_unpackhi_epi64(h, h)) as u64).to_le_bytes(),
            );
            sels
        };
        std::array::from_fn(|k| {
            debug_assert!((0..=1_040_400).contains(&_mm_cvtsi128_si32(sq[k])));
            (_mm_cvtsi128_si32(sq[k]) as u32, bytes(selq[k]))
        })
    }

    /// The full total and the selectors of the block under each palette of `p`, in order, on the AVX-512 path.
    /// Counts its runs in the unit tests.
    #[inline]
    pub(super) fn pair4(_t: Avx512, v: &[u8; 16], p: [&[u8; 8]; 4]) -> [(u32, [u8; 16]); 4] {
        #[cfg(test)]
        crate::path::kernel_ran(crate::path::Wide::Pair4);
        // SAFETY: `_t` exists, so `Avx512::require` found the AVX-512 set.
        unsafe { pair4_kernel(v, p) }
    }
}

/// One scored candidate as [`apply`] takes it: the ends, the full total and the selectors.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
type Scored = (u8, u8, u32, [u8; 16]);

/// Scored pairs decided in order, as `try_pair` calls decide them: each replaces `best` only when its total is
/// strictly below `best.err` at that moment, after the ones before it, so a tie keeps the earlier block.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline(always)]
fn apply<const N: usize>(best: &mut Best, outcomes: [Scored; N]) {
    for (e0, e1, err, sels) in outcomes {
        if err < best.err {
            hit!(Hit::PairImproved);
            *best = Best { err, e0, e1, sels };
        } else {
            hit!(Hit::PairEarlyOut);
        }
    }
}

/// Two pairs scored in one vector (`pair2`) and decided in order (`apply`): the AVX2 path's batch, and the last two
/// of a flushed batch on the AVX-512 path.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[inline(always)]
fn decide2(t: Avx2, best: &mut Best, v: &[u8; 16], (a0, a1): (u8, u8), (b0, b1): (u8, u8)) {
    let [ra, rb] = avx2::pair2(t, v, &pal(a0, a1), &pal(b0, b1));
    #[cfg(test)]
    let start = *best;
    apply(best, [(a0, a1, ra.0, ra.1), (b0, b1, rb.0, rb.1)]);
    #[cfg(test)]
    check_batch(v, start, [(a0, a1), (b0, b1)], best);
}

/// The shadow check of a batch: the pairs `ends`, in order, through the scalar reference from `start`, must leave
/// what `apply` left in `best`. Unit tests only, so every batch they search is checked.
#[cfg(all(test, target_arch = "x86_64", target_feature = "sse2"))]
fn check_batch<const N: usize>(v: &[u8; 16], start: Best, ends: [(u8, u8); N], best: &Best) {
    let mut want = start;
    for (e0, e1) in ends {
        want.try_pair_reference(v, e0, e1);
    }
    assert_eq!(
        (best.err, best.e0, best.e1, best.sels),
        (want.err, want.e0, want.e1, want.sels),
        "{v:?} {ends:?} from {}",
        start.err
    );
}

/// A loop's candidates on their way to its best: [`Batch::push`] where the loop called `try_pair`, [`Batch::flush`] at
/// the loop's end. One per loop (per mode, round or cut), on the stack.
struct Batch<I: Isa> {
    // Read only by the wide branches of `push` and `flush`, which a build without the wide paths does not have.
    #[cfg_attr(
        not(all(target_arch = "x86_64", target_feature = "sse2")),
        allow(dead_code)
    )]
    isa: I,
    /// The pairs waiting, in order, in `pending[..waiting]`: at most one on the AVX2 path (the first of two), at most
    /// three on the AVX-512 path (the first three of four), none on the base path.
    pending: [(u8, u8); 3],
    waiting: usize,
}

impl<I: Isa> Batch<I> {
    #[inline(always)]
    fn new(isa: I) -> Self {
        Batch {
            isa,
            pending: [(0, 0); 3],
            waiting: 0,
        }
    }

    /// `best.try_pair(v, e0, e1)` on the base path. On the AVX2 path the first pair of two waits; the second scores
    /// both in one vector and `apply` decides them in order. On the AVX-512 path the first three of four wait; the
    /// fourth scores all four in one vector, decided in order the same way.
    #[inline(always)]
    fn push(&mut self, best: &mut Best, v: &[u8; 16], e0: u8, e1: u8) {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        if let Some(t) = self.isa.avx512() {
            if self.waiting < 3 {
                self.pending[self.waiting] = (e0, e1);
                self.waiting += 1;
                return;
            }
            self.waiting = 0;
            let [a, b, c] = self.pending;
            let ends = [a, b, c, (e0, e1)];
            // Written out: `ends.map(..)` stayed a call in the frame.
            let p = [pal(a.0, a.1), pal(b.0, b.1), pal(c.0, c.1), pal(e0, e1)];
            let r = avx512::pair4(t, v, p.each_ref());
            #[cfg(test)]
            let start = *best;
            apply::<4>(
                best,
                std::array::from_fn(|k| (ends[k].0, ends[k].1, r[k].0, r[k].1)),
            );
            #[cfg(test)]
            check_batch(v, start, ends, best);
            return;
        }
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        if let Some(t) = self.isa.avx2() {
            if self.waiting == 0 {
                self.pending[0] = (e0, e1);
                self.waiting = 1;
                return;
            }
            self.waiting = 0;
            decide2(t, best, v, self.pending[0], (e0, e1));
            return;
        }
        best.try_pair(v, e0, e1);
    }

    /// The pairs still waiting, in order: on the AVX-512 path the first two of two or three through `pair2`, then a
    /// lone one through `try_pair`; on the AVX2 path a lone pair through `try_pair`; nothing on the base path, where
    /// no pair waits.
    #[inline(always)]
    fn flush(&mut self, best: &mut Best, v: &[u8; 16]) {
        let pending = self.pending;
        let rest = &pending[..std::mem::take(&mut self.waiting)];
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        let rest = match (self.isa.avx512(), self.isa.avx2(), rest) {
            (Some(_), Some(t), [a, b, more @ ..]) => {
                decide2(t, best, v, *a, *b);
                more
            }
            _ => rest,
        };
        for &(e0, e1) in rest {
            best.try_pair(v, e0, e1);
        }
    }
}

#[inline]
pub fn minmax(v: &[u8; 16]) -> (u8, u8) {
    (*v.iter().min().unwrap(), *v.iter().max().unwrap())
}

pub fn clampi(x: i32) -> u8 {
    x.clamp(0, 255) as u8
}

/// rgbcx's candidates (`rgbcx::encode_bc4_hq(.., rad, BC4_USE_ALL_MODES)`) searched per mode, each on its own fresh
/// best: `[0]` the 8-value mode (e0 > e1), `[1]` the 6-value mode. For a solid block `[0]` is the exact block and `[1]`
/// is `Best::new()`.
#[inline(always)]
pub(crate) fn v0_modes(isa: impl Isa, v: &[u8; 16], rad: i32) -> [Best; 2] {
    let (mn, mx) = minmax(v);
    if mn == mx {
        hit!(Hit::V0Solid);
        let exact = Best {
            err: 0,
            e0: mn,
            e1: mn,
            sels: [0; 16],
        };
        return [exact, Best::new()];
    }
    let mut m = [Best::new(); 2];
    for (mode, b) in m.iter_mut().enumerate() {
        let mut batch = Batch::new(isa);
        for lo in -rad..=rad {
            for hi in -rad..=rad {
                let (mut e0, mut e1) = (clampi(mx as i32 + hi), clampi(mn as i32 + lo));
                if e0 == e1 {
                    hit!(Hit::V0EqualEnds);
                    continue;
                }
                if (mode == 0 && e0 <= e1) || (mode == 1 && e0 > e1) {
                    if mode == 0 {
                        hit!(Hit::V0SwapMode0);
                    } else {
                        hit!(Hit::V0SwapMode1);
                    }
                    std::mem::swap(&mut e0, &mut e1);
                }
                batch.push(b, v, e0, e1);
            }
        }
        batch.flush(b, v);
    }
    m
}

/// What rgbcx returns: `modes[1]` when its error is strictly lower, else `modes[0]`.
///
/// Why this equals one loop over both modes' candidates on a single best (the reference `v0_reference`): that loop runs
/// the 8-value candidates on a fresh best, then the 6-value ones on the same best, and takes a pair only when strictly
/// lower. While the 6-value candidates run, the best is the 8-value one until a 6-value pair is strictly below it, and
/// from then on only strictly lower pairs replace it, so the loop ends on the first 6-value pair that reaches the
/// 6-value mode's least error, which is also `modes[1]` (a fresh best takes the first pair reaching the mode's least
/// error). If no 6-value pair is strictly below the 8-value best, the 8-value best stays, which is `modes[0]`. A tie
/// keeps `modes[0]`.
#[inline(always)]
fn v0_pick(modes: &[Best; 2]) -> Best {
    if modes[1].err < modes[0].err {
        hit!(Hit::V0SixValueWins);
        modes[1]
    } else {
        hit!(Hit::V0EightValueKept);
        modes[0]
    }
}

/// Port of `rgbcx::encode_bc4_hq(.., rad, BC4_USE_ALL_MODES)`.
#[inline(always)]
pub(crate) fn v0(isa: impl Isa, v: &[u8; 16], rad: i32) -> Best {
    v0_pick(&v0_modes(isa, v, rad))
}

/// Today's `v0`: both modes' candidates on one best. The reference for `v0_pick`.
#[cfg(test)]
fn v0_reference(v: &[u8; 16], rad: i32) -> Best {
    let (mn, mx) = minmax(v);
    if mn == mx {
        return Best {
            err: 0,
            e0: mn,
            e1: mn,
            sels: [0; 16],
        };
    }
    let mut b = Best::new();
    for mode in 0..2 {
        for lo in -rad..=rad {
            for hi in -rad..=rad {
                let (mut e0, mut e1) = (clampi(mx as i32 + hi), clampi(mn as i32 + lo));
                if e0 == e1 {
                    continue;
                }
                if (mode == 0 && e0 <= e1) || (mode == 1 && e0 > e1) {
                    std::mem::swap(&mut e0, &mut e1);
                }
                b.try_pair_reference(v, e0, e1);
            }
        }
    }
    b
}

/// The 6-value-mode search rgbcx cannot reach: endpoints around the inner range, so 0 and 255 come from the palette.
#[inline(always)]
pub(crate) fn inner6(isa: impl Isa, v: &[u8; 16], b: &mut Best, rad: i32) {
    if !v.iter().any(|&x| x == 0 || x == 255) {
        hit!(Hit::Inner6NoExtreme);
        return;
    }
    let inner: Vec<u8> = v.iter().copied().filter(|&x| x != 0 && x != 255).collect();
    let (Some(&imn), Some(&imx)) = (inner.iter().min(), inner.iter().max()) else {
        hit!(Hit::Inner6NoInner);
        return;
    };
    let mut batch = Batch::new(isa);
    for lo in -rad..=rad {
        for hi in -rad..=rad {
            let (e0, e1) = (clampi(imn as i32 + lo), clampi(imx as i32 + hi));
            if e0 <= e1 {
                hit!(Hit::Inner6Pair);
                batch.push(b, v, e0, e1);
            } else {
                hit!(Hit::Inner6Skip);
            }
        }
    }
    batch.flush(b, v);
}

/// Fraction of e1 in texel value for selector `s` (None = fixed 0/255 in the 6-value mode).
#[inline(always)]
fn frac(mode8: bool, s: u8) -> Option<f64> {
    match (mode8, s) {
        (_, 0) => Some(0.0),
        (_, 1) => Some(1.0),
        (true, s) => Some((s - 1) as f64 / 7.0),
        (false, 6 | 7) => None,
        (false, s) => Some((s - 1) as f64 / 5.0),
    }
}

/// Least-squares refit of the endpoints for the current selectors, then the +-1 neighbourhood in the same mode.
#[inline(always)]
pub(crate) fn ls_refine(isa: impl Isa, v: &[u8; 16], b: &mut Best) {
    for _ in 0..3 {
        let mode8 = b.e0 > b.e1;
        let (mut aa, mut ab, mut bb, mut av, mut bv) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for i in 0..16 {
            if let Some(t) = frac(mode8, b.sels[i]) {
                let (wa, wb, x) = (1.0 - t, t, v[i] as f64);
                aa += wa * wa;
                ab += wa * wb;
                bb += wb * wb;
                av += wa * x;
                bv += wb * x;
            } else {
                hit!(Hit::LsFixedTexel);
            }
        }
        let det = aa * bb - ab * ab;
        if det.abs() < 1e-9 {
            hit!(Hit::LsSingular);
            return;
        }
        let a = (av * bb - bv * ab) / det;
        let c = (bv * aa - av * ab) / det;
        let before = b.err;
        let mut batch = Batch::new(isa);
        for da in -1..=1 {
            for dc in -1..=1 {
                let (e0, e1) = (clampi(a.round() as i32 + da), clampi(c.round() as i32 + dc));
                if (e0 > e1) == mode8 {
                    hit!(Hit::LsPair);
                    batch.push(b, v, e0, e1);
                } else {
                    hit!(Hit::LsModeSkip);
                }
            }
        }
        batch.flush(b, v);
        if b.err >= before {
            hit!(Hit::LsNoGain);
            return;
        }
    }
    hit!(Hit::LsRoundsExhausted);
}

/// Refit the best of each mode, not only the overall best: `ls_refine` on `b`, then the other mode's best from
/// `modes` (what `v0_modes` returned for this block), `ls_refine` on that, keeping the strictly better.
///
/// The C++ searches rgbcx's candidates in the other mode again here. That search is the one `v0_modes` already ran:
/// the same candidates, in the same order, with the same skip of equal ends, on a fresh best, so its result is
/// `modes[1]` (6-value) when `b` is in the 8-value mode and `modes[0]` otherwise. `ls_refine` of `b` changes `b`
/// only, and the mode is read after it, as before. In the unit tests `search` compares both entries of `modes` with
/// the old search (`check_modes`).
#[inline(always)]
pub(crate) fn refit_both(isa: impl Isa, v: &[u8; 16], b: &mut Best, modes: &[Best; 2]) {
    ls_refine(isa, v, b);
    let (mn, mx) = minmax(v);
    if mn != mx {
        let mode8 = b.e0 > b.e1;
        let mut o = if mode8 {
            hit!(Hit::RefitOtherSixValue);
            modes[1]
        } else {
            hit!(Hit::RefitOtherEightValue);
            modes[0]
        };
        ls_refine(isa, v, &mut o);
        if o.err < b.err {
            hit!(Hit::RefitOtherBetter);
            *b = o;
        } else {
            hit!(Hit::RefitOtherKept);
        }
    } else {
        hit!(Hit::RefitSolid);
    }
}

/// The shadow check of `search`: each entry of `modes` against the old second loop of `refit_both`, which searched
/// the mode opposite to `mode8`. Unit tests only, so every block they search is checked. A solid block has no such
/// loop.
#[cfg(test)]
fn check_modes(v: &[u8; 16], modes: &[Best; 2], rad: i32) {
    let (mn, mx) = minmax(v);
    if mn == mx {
        return;
    }
    for mode8 in [false, true] {
        let (a, r) = (
            modes[usize::from(mode8)],
            other_mode_reference(v, mode8, rad),
        );
        assert_eq!(
            (a.err, a.e0, a.e1, a.sels),
            (r.err, r.e0, r.e1, r.sels),
            "{v:?} {mode8} {rad}"
        );
    }
}

/// The old second loop of `refit_both`: rgbcx's candidates in the mode opposite to `mode8`, on a fresh best. The
/// reference for the per-mode results of `v0_modes`.
#[cfg(test)]
fn other_mode_reference(v: &[u8; 16], mode8: bool, rad: i32) -> Best {
    let (mn, mx) = minmax(v);
    let mut o = Best::new();
    for lo in -rad..=rad {
        for hi in -rad..=rad {
            let (mut e0, mut e1) = (clampi(mx as i32 + hi), clampi(mn as i32 + lo));
            if e0 == e1 {
                continue;
            }
            if (e0 > e1) == mode8 {
                std::mem::swap(&mut e0, &mut e1);
            }
            o.try_pair_reference(v, e0, e1);
        }
    }
    o
}

/// 6-value mode with near-extreme texels left to the palette's 0/255: inner range = values in (t, 255 - t).
#[inline(always)]
pub(crate) fn trimmed6(isa: impl Isa, v: &[u8; 16], b: &mut Best) {
    for t in [2u8, 6, 12, 24, 48] {
        if !v.iter().any(|&x| x <= t || x >= 255 - t) {
            hit!(Hit::TrimNoExtreme);
            continue;
        }
        let inner: Vec<u8> = v
            .iter()
            .copied()
            .filter(|&x| x > t && x < 255 - t)
            .collect();
        let (Some(&imn), Some(&imx)) = (inner.iter().min(), inner.iter().max()) else {
            hit!(Hit::TrimNoInner);
            continue;
        };
        let mut c = *b;
        let mut batch = Batch::new(isa);
        for lo in -2..=2 {
            for hi in -2..=2 {
                let (e0, e1) = (clampi(imn as i32 + lo), clampi(imx as i32 + hi));
                if e0 <= e1 {
                    hit!(Hit::TrimPair);
                    batch.push(&mut c, v, e0, e1);
                } else {
                    hit!(Hit::TrimSkip);
                }
            }
        }
        batch.flush(&mut c, v);
        if c.e0 <= c.e1 {
            hit!(Hit::TrimRefit);
            ls_refine(isa, v, &mut c);
        } else {
            hit!(Hit::TrimNoRefit);
        }
        if c.err < b.err {
            hit!(Hit::TrimReplaced);
            *b = c;
        } else {
            hit!(Hit::TrimKept);
        }
    }
}

/// Exhaustive +-3 around the current best, same mode.
#[inline(always)]
pub(crate) fn local(isa: impl Isa, v: &[u8; 16], b: &mut Best) {
    let (c0, c1, m8) = (b.e0 as i32, b.e1 as i32, b.e0 > b.e1);
    let mut batch = Batch::new(isa);
    for d0 in -3..=3 {
        for d1 in -3..=3 {
            let (e0, e1) = (clampi(c0 + d0), clampi(c1 + d1));
            if (e0 > e1) == m8 {
                hit!(Hit::LocalPair);
                batch.push(b, v, e0, e1);
            } else {
                hit!(Hit::LocalModeSkip);
            }
        }
    }
    batch.flush(b, v);
}

/// Texels whose alpha-test result (pass when alpha >= `cut`) differs from the source.
pub fn flips(v: &[u8; 16], b: &Best, cut: u8) -> u32 {
    let p = pal(b.e0, b.e1);
    (0..16)
        .filter(|&i| (v[i] >= cut) != (p[b.sels[i] as usize] >= cut))
        .count() as u32
}

/// The upstream search: `rgbcx::encode_bc4_hq` with all modes, as bytes.
pub fn upstream(v: &[u8; 16], radius: u32) -> [u8; 8] {
    v0(Base, v, radius as i32).bytes()
}

/// The measured search (probe variant v4; v5 with `guard = Some(cut)`, which falls back to the upstream block when it
/// would flip more texels across the alpha-test cutoff `cut`, 128 by default). Never worse than [`upstream`] in SSE.
/// `cut` is the cutoff itself: reading a 0 as 128, as rgbcx's flags do, is `bc4_encode_alpha`'s job. Runs on
/// [`Path::best`].
pub fn search(v: &[u8; 16], radius: u32, guard: Option<u8>) -> [u8; 8] {
    search_on(Path::best(), v, radius, guard)
}

/// [`search`] on `path`: one dispatch per call. Panics if this machine cannot run `path`.
#[doc(hidden)]
pub fn search_on(path: Path, v: &[u8; 16], radius: u32, guard: Option<u8>) -> [u8; 8] {
    path.assert_available();
    match path {
        Path::Base => {
            #[cfg(test)]
            crate::path::frame_ran(Path::Base);
            search_in(Base, v, radius, guard)
        }
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        Path::Avx2 => {
            let t = Avx2::require();
            // SAFETY: `t` exists, so `Avx2::require` found AVX2.
            unsafe { search_avx2(t, v, radius, guard) }
        }
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        Path::Avx512 => {
            let t = Avx512::require();
            // SAFETY: `t` exists, so `Avx512::require` found the AVX-512 set.
            unsafe { search_avx512(t, v, radius, guard) }
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        Path::Avx2 => {
            unreachable!("path avx2 is not built here; the _on entry asserted availability")
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        Path::Avx512 => {
            unreachable!("path avx512 is not built here; the _on entry asserted availability")
        }
    }
}

/// The AVX2 frame of the search: the whole search compiled with AVX2, its loops scoring two pairs per vector.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[target_feature(enable = "avx2")]
fn search_avx2(t: Avx2, v: &[u8; 16], radius: u32, guard: Option<u8>) -> [u8; 8] {
    #[cfg(test)]
    crate::path::frame_ran(Path::Avx2);
    search_in(t, v, radius, guard)
}

/// The AVX-512 frame of the search: the whole search compiled with the AVX-512 set, its loops scoring four pairs per
/// vector.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[target_feature(enable = "avx2,avx512f,avx512bw,avx512vl,avx512dq")]
fn search_avx512(t: Avx512, v: &[u8; 16], radius: u32, guard: Option<u8>) -> [u8; 8] {
    #[cfg(test)]
    crate::path::frame_ran(Path::Avx512);
    search_in(t, v, radius, guard)
}

/// The body of [`search`], compiled for `isa`.
#[inline(always)]
fn search_in(isa: impl Isa, v: &[u8; 16], radius: u32, guard: Option<u8>) -> [u8; 8] {
    let rad = radius as i32;
    let modes = v0_modes(isa, v, rad);
    let base = v0_pick(&modes);
    let mut b = base;
    inner6(isa, v, &mut b, rad);
    #[cfg(test)]
    check_modes(v, &modes, rad);
    refit_both(isa, v, &mut b, &modes);
    trimmed6(isa, v, &mut b);
    local(isa, v, &mut b);
    if let Some(cut) = guard {
        if flips(v, &b, cut) > flips(v, &base, cut) {
            hit!(Hit::GuardFallback);
            b = base;
        } else {
            hit!(Hit::GuardPassed);
        }
    } else {
        hit!(Hit::NoGuard);
    }
    b.bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen;
    use crate::path::Base;
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    use crate::path::{avx2_tested, avx512_tested, Avx2, Avx512};

    /// The endpoint pairs tried for a block: every pair with both ends on a stride of 5, then every pair within 3 of
    /// the block's least and greatest value.
    fn pairs_for(v: &[u8; 16]) -> Vec<(u8, u8)> {
        let mut out = Vec::new();
        for e0 in (0..=255u8).step_by(5) {
            for e1 in (0..=255u8).step_by(5) {
                out.push((e0, e1));
            }
        }
        let (mn, mx) = minmax(v);
        let near = |c: u8| (c.saturating_sub(3)..=c.saturating_add(3)).collect::<Vec<u8>>();
        for e0 in near(mx) {
            for e1 in near(mn) {
                out.push((e0, e1));
                out.push((e1, e0));
            }
        }
        out
    }

    #[test]
    fn v0_from_the_two_modes_equals_the_shared_search() {
        let (mut six, mut eight, mut solid) = (0usize, 0usize, 0usize);
        for v in gen::bc4_classes(0xB4B4_0006, 200) {
            for rad in [0, 1, 4, 5, 10] {
                let (a, b) = (v0(Base, &v, rad), v0_reference(&v, rad));
                assert_eq!(
                    (a.err, a.e0, a.e1, a.sels),
                    (b.err, b.e0, b.e1, b.sels),
                    "{v:?} radius {rad}"
                );
                if minmax(&v).0 == minmax(&v).1 {
                    solid += 1;
                } else if a.e0 > a.e1 {
                    eight += 1;
                } else {
                    six += 1;
                }
            }
        }
        assert!(
            six > 0 && eight > 0 && solid > 0,
            "a kind was never reached: 6-value {six}, 8-value {eight}, solid {solid}"
        );
    }

    #[test]
    fn each_mode_equals_the_other_mode_search() {
        let mut blocks = 0usize;
        for v in gen::bc4_classes(0xB4B4_0007, 200) {
            if v.iter().all(|&x| x == v[0]) {
                continue;
            }
            for rad in [0, 1, 4, 5, 10] {
                let m = v0_modes(Base, &v, rad);
                for mode8 in [false, true] {
                    // `other_mode_reference(.., mode8, ..)` searches the mode opposite to `mode8`.
                    let (a, b) = (m[usize::from(mode8)], other_mode_reference(&v, mode8, rad));
                    assert_eq!(
                        (a.err, a.e0, a.e1, a.sels),
                        (b.err, b.e0, b.e1, b.sels),
                        "{v:?} radius {rad} {mode8}"
                    );
                }
            }
            blocks += 1;
        }
        assert!(
            blocks > 0,
            "the classes held no block of two or more values"
        );
    }

    #[test]
    fn a_tie_between_the_modes_keeps_the_8_value_block() {
        // Values 0 and 255 only: both modes reach error 0; `v0` must return e0 > e1.
        let v: [u8; 16] = std::array::from_fn(|i| if i % 2 == 0 { 0 } else { 255 });
        let b = v0(Base, &v, 5);
        assert!(b.err == 0 && b.e0 > b.e1);
    }

    #[test]
    fn the_pair_kernel_equals_the_scalar_one() {
        // Both palette kinds and both outcomes must be reached, or the test proves less than it looks.
        let (mut eight, mut six, mut taken, mut refused) = (0usize, 0usize, 0usize, 0usize);
        for v in gen::bc4_classes(0xB4B4_0005, 30) {
            let mut best = u32::MAX;
            // The best falls step by step, so later pairs meet a low `best`. Where the kernel is compiled in, `pair`
            // has already compared itself with `pair_scalar` (its shadow check); the assert below says what is proven
            // and is the check on a target without the kernel.
            for (e0, e1) in pairs_for(&v) {
                let p = pal(e0, e1);
                let (a, b) = (pair(&v, &p, best), pair_scalar(&v, &p, best));
                assert_eq!(a, b, "{v:?} {e0} {e1} {best}");
                if e0 > e1 {
                    eight += 1;
                } else {
                    six += 1;
                }
                match a {
                    Some((err, _)) => {
                        best = err;
                        taken += 1;
                    }
                    None => refused += 1,
                }
            }
        }
        assert!(
            eight > 0 && six > 0 && taken > 0 && refused > 0,
            "a kind was never reached: 8-value {eight}, 6-value {six}, taken {taken}, refused {refused}"
        );
    }

    #[test]
    fn the_pair_kernel_is_exact_at_the_largest_errors() {
        // Blocks of the extremes against palettes that sit far from them: distances up to 255 in every texel, totals
        // up to 16 x 254^2 = 1,032,256 and ties between palette values.
        let blocks: [[u8; 16]; 4] = [
            [0; 16],
            [255; 16],
            std::array::from_fn(|i| if i % 2 == 0 { 0 } else { 255 }),
            std::array::from_fn(|i| (i * 17) as u8),
        ];
        let ends: [(u8, u8); 7] = [
            (255, 254),
            (254, 255),
            (1, 0),
            (0, 1),
            (255, 0),
            (0, 255),
            (7, 7),
        ];
        let mut largest = 0;
        for v in &blocks {
            for (e0, e1) in ends {
                let p = pal(e0, e1);
                for best in [u32::MAX, 1_040_400, 1_032_256, 1_032_255, 1, 0] {
                    let (a, b) = (pair(v, &p, best), pair_scalar(v, &p, best));
                    assert_eq!(a, b, "{v:?} {e0} {e1} {best}");
                    if let Some((err, _)) = a {
                        largest = largest.max(err);
                    }
                }
            }
        }
        assert!(
            largest >= 1_000_000,
            "no case came near the largest total: {largest}"
        );
        // The two-pair kernel with a different palette of the list in each half: each half is the full scalar result.
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        if avx2_tested() {
            let t = Avx2::require();
            for v in &blocks {
                for (i, a) in ends.iter().enumerate() {
                    for (_, b) in ends.iter().enumerate().filter(|&(j, _)| j != i) {
                        let (pa, pb) = (pal(a.0, a.1), pal(b.0, b.1));
                        let [ra, rb] = avx2::pair2(t, v, &pa, &pb);
                        assert_eq!(
                            Some(ra),
                            pair_scalar(v, &pa, u32::MAX),
                            "{v:?} low {a:?}, high {b:?}"
                        );
                        assert_eq!(
                            Some(rb),
                            pair_scalar(v, &pb, u32::MAX),
                            "{v:?} low {a:?}, high {b:?}"
                        );
                    }
                }
            }
        }
        // The four-pair kernel with four different palettes of the list, one per quarter, each window of four in
        // turn: each quarter is the full scalar result.
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        if avx512_tested() {
            let t = Avx512::require();
            for v in &blocks {
                for i in 0..ends.len() {
                    let q: [(u8, u8); 4] = std::array::from_fn(|k| ends[(i + k) % ends.len()]);
                    let p = q.map(|(e0, e1)| pal(e0, e1));
                    let r = avx512::pair4(t, v, p.each_ref());
                    for k in 0..4 {
                        assert_eq!(
                            Some(r[k]),
                            pair_scalar(v, &p[k], u32::MAX),
                            "{v:?} quarters {q:?}, quarter {k}"
                        );
                    }
                }
            }
        }
    }

    /// A best of error `err` whose ends and selectors carry `tag`, so an assert shows which candidate stayed.
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    fn tagged(err: u32, tag: u8) -> Best {
        Best {
            err,
            e0: tag,
            e1: tag + 1,
            sels: [tag % 8; 16],
        }
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[test]
    fn a_batch_takes_the_second_only_against_the_first() {
        let start = tagged(500, 10);
        let cand = |b: Best| (b.e0, b.e1, b.err, b.sels);
        // (error of a, error of b, which one stays: 0 the start, 1 a, 2 b)
        for (ea, eb, stays) in [(100, 100, 1), (100, 99, 2), (600, 499, 2), (500, 600, 0)] {
            let (a, b) = (tagged(ea, 20), tagged(eb, 30));
            let mut best = start;
            apply(&mut best, [cand(a), cand(b)]);
            let want = [start, a, b][stays];
            assert_eq!(
                (best.err, best.e0, best.e1, best.sels),
                (want.err, want.e0, want.e1, want.sels),
                "a {ea}, b {eb}"
            );
        }
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[test]
    fn a_batch_of_four_takes_each_against_the_ones_before() {
        let start = tagged(500, 10);
        let cand = |b: Best| (b.e0, b.e1, b.err, b.sels);
        // (errors of the four outcomes, which one stays: 0 the start, 1..=4 the outcomes in order)
        let cases: [([u32; 4], usize); 8] = [
            ([100, 100, 100, 100], 1),
            ([400, 300, 200, 100], 4),
            ([600, 300, 400, 700], 2),
            ([600, 300, 300, 500], 2),
            ([600, 300, 250, 250], 3),
            ([500, 499, 499, 498], 4),
            ([500, 600, 700, 800], 0),
            ([700, 450, 450, 449], 4),
        ];
        for (errs, stays) in cases {
            let outs: [Best; 4] = std::array::from_fn(|i| tagged(errs[i], 20 + 10 * i as u8));
            let mut best = start;
            apply(&mut best, outs.map(cand));
            let want = [start, outs[0], outs[1], outs[2], outs[3]][stays];
            assert_eq!(
                (best.err, best.e0, best.e1, best.sels),
                (want.err, want.e0, want.e1, want.sels),
                "outcomes {errs:?}"
            );
        }
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[test]
    fn two_pairs_in_a_vector_equal_two_scalar_calls() {
        if !avx2_tested() {
            return;
        }
        let t = Avx2::require();
        // Per half (0 low, 1 high): palettes of the 8-value kind, then of the 6-value kind.
        let (mut kinds, mut taken, mut refused) = ([[0usize; 2]; 2], 0usize, 0usize);
        for v in gen::bc4_classes(0xE3_0001, 30) {
            // The best falls as in `the_pair_kernel_equals_the_scalar_one`, so later pairs meet a low best.
            let mut best = Best::new();
            let pairs = pairs_for(&v);
            let (twos, rest) = pairs.as_chunks::<2>();
            for &[a, b] in twos {
                let (pa, pb) = (pal(a.0, a.1), pal(b.0, b.1));
                let [ra, rb] = avx2::pair2(t, &v, &pa, &pb);
                let from = best.err;
                let what = || format!("{v:?} low {a:?}, high {b:?}, best {from}");
                assert_eq!(Some(ra), pair_scalar(&v, &pa, u32::MAX), "{}", what());
                assert_eq!(Some(rb), pair_scalar(&v, &pb, u32::MAX), "{}", what());
                // Each outcome in order: the scalar call against the best of that moment.
                let mut want = best;
                for ((e0, e1), p) in [(a, pa), (b, pb)] {
                    match pair_scalar(&v, &p, want.err) {
                        Some((err, sels)) => {
                            want = Best { err, e0, e1, sels };
                            taken += 1;
                        }
                        None => refused += 1,
                    }
                }
                for (half, (e0, e1)) in [a, b].into_iter().enumerate() {
                    kinds[half][usize::from(e0 <= e1)] += 1;
                }
                apply(&mut best, [(a.0, a.1, ra.0, ra.1), (b.0, b.1, rb.0, rb.1)]);
                assert_eq!(
                    (best.err, best.e0, best.e1, best.sels),
                    (want.err, want.e0, want.e1, want.sels),
                    "{}",
                    what()
                );
            }
            // `pairs_for` gives 52 x 52 = 2,704 grid pairs and pushes the near pairs two at a time (2,704 + 2k in all), so
            // no pair is left over here; a lone pair's `flush` is tested in `a_batch_flushes_every_pair_in_order`.
            assert!(rest.is_empty(), "{v:?}: {} pairs left over", rest.len());
        }
        assert!(
            kinds.iter().flatten().all(|&n| n > 0) && taken > 0 && refused > 0,
            "a kind was never reached: [low, high] x [8-value, 6-value] {kinds:?}, taken {taken}, refused {refused}"
        );
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[test]
    fn four_pairs_in_a_vector_equal_four_scalar_calls() {
        if !avx512_tested() {
            return;
        }
        let t = Avx512::require();
        // Per quarter: palettes of the 8-value kind, then of the 6-value kind; and the blocks whose last pairs went
        // through `pair2` in `flush`.
        let (mut kinds, mut taken, mut refused, mut flushed2) =
            ([[0usize; 2]; 4], 0usize, 0usize, 0usize);
        for v in gen::bc4_classes(0xE6_0001, 30) {
            // The best falls as in `the_pair_kernel_equals_the_scalar_one`, so later pairs meet a low best.
            let mut best = Best::new();
            let pairs = pairs_for(&v);
            let (fours, rest) = pairs.as_chunks::<4>();
            for ends in fours {
                let p = ends.map(|(e0, e1)| pal(e0, e1));
                let r = avx512::pair4(t, &v, p.each_ref());
                let from = best.err;
                let what = || format!("{v:?} quarters {ends:?}, best {from}");
                for k in 0..4 {
                    assert_eq!(
                        Some(r[k]),
                        pair_scalar(&v, &p[k], u32::MAX),
                        "quarter {k}: {}",
                        what()
                    );
                    kinds[k][usize::from(ends[k].0 <= ends[k].1)] += 1;
                }
                // Each outcome in order: the scalar call against the best of that moment.
                let mut want = best;
                for (&(e0, e1), pk) in ends.iter().zip(&p) {
                    match pair_scalar(&v, pk, want.err) {
                        Some((err, sels)) => {
                            want = Best { err, e0, e1, sels };
                            taken += 1;
                        }
                        None => refused += 1,
                    }
                }
                apply::<4>(
                    &mut best,
                    std::array::from_fn(|k| (ends[k].0, ends[k].1, r[k].0, r[k].1)),
                );
                assert_eq!(
                    (best.err, best.e0, best.e1, best.sels),
                    (want.err, want.e0, want.e1, want.sels),
                    "{}",
                    what()
                );
            }
            // The pairs after the last four wait in a batch and go out as `flush` sends them. `pairs_for` gives 2,704 grid
            // pairs (a multiple of 4) and pushes the near pairs two at a time, so here 0 or 2 wait, and the 2 go through
            // `pair2`; one or three waiting are tested in `a_batch_flushes_every_pair_in_order`.
            let mut want = best;
            for &(e0, e1) in rest {
                want.try_pair_reference(&v, e0, e1);
            }
            let mut batch = Batch::new(t);
            for &(e0, e1) in rest {
                batch.push(&mut best, &v, e0, e1);
            }
            batch.flush(&mut best, &v);
            flushed2 += usize::from(rest.len() >= 2);
            assert_eq!(
                (best.err, best.e0, best.e1, best.sels),
                (want.err, want.e0, want.e1, want.sels),
                "{v:?} last pairs {rest:?}"
            );
        }
        assert!(
            kinds.iter().flatten().all(|&n| n > 0) && taken > 0 && refused > 0 && flushed2 > 0,
            "a kind was never reached: quarters x [8-value, 6-value] {kinds:?}, taken {taken}, refused {refused}, \
             flushes through pair2 {flushed2}"
        );
    }

    /// A batch on the AVX-512 path sends every pair through, in order: n = 1..=9 pairs pushed, then flushed (0 to 3
    /// waiting at the flush, after up to two four-pair runs), leave what `try_pair_reference` in order leaves. The
    /// pairs come in order of falling error, so the last one is the best and a pair `flush` dropped would show.
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[test]
    fn a_batch_flushes_every_pair_in_order() {
        if !avx512_tested() {
            return;
        }
        let t = Avx512::require();
        // How often 0, 1, 2 and 3 pairs were waiting at the flush.
        let mut waiting = [0usize; 4];
        for v in gen::bc4_classes(0xE6_0002, 10) {
            // The block's pairs by full error, one per error, falling; the 9 best.
            let mut by_err: Vec<(u32, (u8, u8))> = pairs_for(&v)
                .into_iter()
                .filter_map(|(e0, e1)| {
                    pair_scalar(&v, &pal(e0, e1), u32::MAX).map(|(err, _)| (err, (e0, e1)))
                })
                .collect();
            by_err.sort_by_key(|&(err, _)| std::cmp::Reverse(err));
            by_err.dedup_by_key(|e| e.0);
            assert!(by_err.len() >= 9, "{v:?}: {} distinct errors", by_err.len());
            let ends: Vec<(u8, u8)> = by_err[by_err.len() - 9..].iter().map(|&(_, e)| e).collect();
            for n in 1..=9 {
                let pairs = &ends[9 - n..];
                let mut want = Best::new();
                for &(e0, e1) in pairs {
                    want.try_pair_reference(&v, e0, e1);
                }
                let (mut best, mut batch) = (Best::new(), Batch::new(t));
                for &(e0, e1) in pairs {
                    batch.push(&mut best, &v, e0, e1);
                }
                waiting[batch.waiting] += 1;
                batch.flush(&mut best, &v);
                assert_eq!(
                    (best.err, best.e0, best.e1, best.sels),
                    (want.err, want.e0, want.e1, want.sels),
                    "{v:?}, {n} pairs {pairs:?}"
                );
            }
        }
        assert!(
            waiting[1] > 0 && waiting[3] > 0,
            "flushes with 1 and 3 pairs waiting were not both reached: {waiting:?}"
        );
    }
}
