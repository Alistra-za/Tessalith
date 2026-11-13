//! The BC4 block search (docs/research/bc4-family.md): a port of `rgbcx::encode_bc4_hq` (rgbcx from bc7enc_rdo commit
//! b943862, MIT or public domain) with Tessalith's `BC4_SDC_SEARCH` and `BC4_SDC_GUARD128` patches, which are the
//! measured additions (6-value search on the inner range, least-squares refits, trimmed inner ranges, a local search).
//! Everything starts from the upstream result and replaces it only on a strictly lower SSE, so ties keep its bytes.
//! The vendored C++ is patched to equal [`search`] byte for byte, so the tie rules here are the contract: the first
//! minimum per texel, a strict `<` to replace the best, and candidates tried in the order written.
//! The test inputs `Rng` and `blocks` live in `tests/support/gen.rs`.
// Index loops mirror the C++ they port.
#![allow(clippy::needless_range_loop)]

// `hit!` expands to nothing outside the unit tests, which leaves `Hit` unused there.
#[cfg_attr(not(test), allow(unused_imports))]
use crate::hits::{hit, Hit};

/// The bcdec / patched-rgbcx BC4 palette (`get_block_values`); checked against the FFI for all 65536 pairs.
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
    pub fn try_pair(&mut self, v: &[u8; 16], e0: u8, e1: u8) {
        let p = pal(e0, e1);
        let Some((err, sels)) = pair(v, &p, self.err) else {
            hit!(Hit::PairEarlyOut);
            return;
        };
        hit!(Hit::PairImproved);
        *self = Best { err, e0, e1, sels };
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
fn pair(v: &[u8; 16], p: &[u8; 8], best: u32) -> Option<(u32, [u8; 16])> {
    let r = sse2::pair(v, p, best);
    #[cfg(test)]
    assert_eq!(r, pair_scalar(v, p, best), "{v:?} {p:?} {best}");
    r
}

#[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
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

    pub(super) fn pair(v: &[u8; 16], p: &[u8; 8], best: u32) -> Option<(u32, [u8; 16])> {
        // SAFETY: this module is compiled only where SSE2 is enabled for the target (the `cfg` on `mod sse2`).
        unsafe { pair_sse2(v, p, best) }
    }
}

pub fn minmax(v: &[u8; 16]) -> (u8, u8) {
    (*v.iter().min().unwrap(), *v.iter().max().unwrap())
}

pub fn clampi(x: i32) -> u8 {
    x.clamp(0, 255) as u8
}

/// rgbcx's candidates (`rgbcx::encode_bc4_hq(.., rad, BC4_USE_ALL_MODES)`) searched per mode, each on its own fresh
/// best: `[0]` the 8-value mode (e0 > e1), `[1]` the 6-value mode. For a solid block `[0]` is the exact block and `[1]`
/// is `Best::new()`.
pub fn v0_modes(v: &[u8; 16], rad: i32) -> [Best; 2] {
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
                b.try_pair(v, e0, e1);
            }
        }
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
pub fn v0(v: &[u8; 16], rad: i32) -> Best {
    v0_pick(&v0_modes(v, rad))
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
                b.try_pair(v, e0, e1);
            }
        }
    }
    b
}

/// The 6-value-mode search rgbcx cannot reach: endpoints around the inner range, so 0 and 255 come from the palette.
pub fn inner6(v: &[u8; 16], b: &mut Best, rad: i32) {
    if !v.iter().any(|&x| x == 0 || x == 255) {
        hit!(Hit::Inner6NoExtreme);
        return;
    }
    let inner: Vec<u8> = v.iter().copied().filter(|&x| x != 0 && x != 255).collect();
    let (Some(&imn), Some(&imx)) = (inner.iter().min(), inner.iter().max()) else {
        hit!(Hit::Inner6NoInner);
        return;
    };
    for lo in -rad..=rad {
        for hi in -rad..=rad {
            let (e0, e1) = (clampi(imn as i32 + lo), clampi(imx as i32 + hi));
            if e0 <= e1 {
                hit!(Hit::Inner6Pair);
                b.try_pair(v, e0, e1);
            } else {
                hit!(Hit::Inner6Skip);
            }
        }
    }
}

/// Fraction of e1 in texel value for selector `s` (None = fixed 0/255 in the 6-value mode).
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
pub fn ls_refine(v: &[u8; 16], b: &mut Best) {
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
        for da in -1..=1 {
            for dc in -1..=1 {
                let (e0, e1) = (clampi(a.round() as i32 + da), clampi(c.round() as i32 + dc));
                if (e0 > e1) == mode8 {
                    hit!(Hit::LsPair);
                    b.try_pair(v, e0, e1);
                } else {
                    hit!(Hit::LsModeSkip);
                }
            }
        }
        if b.err >= before {
            hit!(Hit::LsNoGain);
            return;
        }
    }
    hit!(Hit::LsRoundsExhausted);
}

/// Refit the best of each mode, not only the overall best: `ls_refine` on `b`, then a fresh search of rgbcx's own
/// candidates in the other mode, `ls_refine` on that, keeping the strictly better.
///
/// The other mode's search is the one `v0_modes` already ran: the same candidates, in the same order, with the same
/// skip of equal ends, on a fresh best, so its result is `modes[1]` (6-value) when `b` is in the 8-value mode and
/// `modes[0]` otherwise. `ls_refine` of `b` changes `b` only, and the mode is read after it, as before. In the unit
/// tests `rad` lets the old search run and be compared.
pub fn refit_both(v: &[u8; 16], b: &mut Best, modes: &[Best; 2], #[cfg(test)] rad: i32) {
    ls_refine(v, b);
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
        #[cfg(test)]
        {
            let r = other_mode_reference(v, mode8, rad);
            assert_eq!(
                (o.err, o.e0, o.e1, o.sels),
                (r.err, r.e0, r.e1, r.sels),
                "{v:?} {mode8} {rad}"
            );
        }
        ls_refine(v, &mut o);
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

/// Today's second loop of `refit_both`: rgbcx's candidates in the mode opposite to `mode8`, on a fresh best. The
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
            o.try_pair(v, e0, e1);
        }
    }
    o
}

/// 6-value mode with near-extreme texels left to the palette's 0/255: inner range = values in (t, 255 - t).
pub fn trimmed6(v: &[u8; 16], b: &mut Best) {
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
        for lo in -2..=2 {
            for hi in -2..=2 {
                let (e0, e1) = (clampi(imn as i32 + lo), clampi(imx as i32 + hi));
                if e0 <= e1 {
                    hit!(Hit::TrimPair);
                    c.try_pair(v, e0, e1);
                } else {
                    hit!(Hit::TrimSkip);
                }
            }
        }
        if c.e0 <= c.e1 {
            hit!(Hit::TrimRefit);
            ls_refine(v, &mut c);
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
pub fn local(v: &[u8; 16], b: &mut Best) {
    let (c0, c1, m8) = (b.e0 as i32, b.e1 as i32, b.e0 > b.e1);
    for d0 in -3..=3 {
        for d1 in -3..=3 {
            let (e0, e1) = (clampi(c0 + d0), clampi(c1 + d1));
            if (e0 > e1) == m8 {
                hit!(Hit::LocalPair);
                b.try_pair(v, e0, e1);
            } else {
                hit!(Hit::LocalModeSkip);
            }
        }
    }
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
    v0(v, radius as i32).bytes()
}

/// The measured search (probe variant v4; v5 with `guard = Some(cut)`, which falls back to the upstream block when it
/// would flip more texels across the alpha-test cutoff `cut`, 128 by default). Never worse than [`upstream`] in SSE.
/// `cut` is the cutoff itself: reading a 0 as 128, as rgbcx's flags do, is `bc4_encode_alpha`'s job.
pub fn search(v: &[u8; 16], radius: u32, guard: Option<u8>) -> [u8; 8] {
    let rad = radius as i32;
    let modes = v0_modes(v, rad);
    let base = v0_pick(&modes);
    let mut b = base;
    inner6(v, &mut b, rad);
    #[cfg(test)]
    refit_both(v, &mut b, &modes, rad);
    #[cfg(not(test))]
    refit_both(v, &mut b, &modes);
    trimmed6(v, &mut b);
    local(v, &mut b);
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
                let (a, b) = (v0(&v, rad), v0_reference(&v, rad));
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
        let (mut eight, mut six) = (0usize, 0usize);
        for v in gen::bc4_classes(0xB4B4_0007, 200) {
            if v.iter().all(|&x| x == v[0]) {
                continue;
            }
            for rad in [0, 1, 4, 5, 10] {
                let m = v0_modes(&v, rad);
                for mode8 in [false, true] {
                    // `other_mode_reference(.., mode8, ..)` searches the mode opposite to `mode8`.
                    let (a, b) = (m[usize::from(mode8)], other_mode_reference(&v, mode8, rad));
                    assert_eq!(
                        (a.err, a.e0, a.e1, a.sels),
                        (b.err, b.e0, b.e1, b.sels),
                        "{v:?} radius {rad} {mode8}"
                    );
                    if mode8 {
                        six += 1;
                    } else {
                        eight += 1;
                    }
                }
            }
        }
        assert!(eight > 0 && six > 0, "8-value {eight}, 6-value {six}");
    }

    #[test]
    fn a_tie_between_the_modes_keeps_the_8_value_block() {
        // Values 0 and 255 only: both modes reach error 0; `v0` must return e0 > e1.
        let v: [u8; 16] = std::array::from_fn(|i| if i % 2 == 0 { 0 } else { 255 });
        let b = v0(&v, 5);
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
        let mut largest = 0;
        for v in &blocks {
            for (e0, e1) in [
                (255, 254),
                (254, 255),
                (1, 0),
                (0, 1),
                (255, 0),
                (0, 255),
                (7, 7),
            ] {
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
    }
}
