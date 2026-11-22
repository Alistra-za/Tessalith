//! The endpoint pairs of one total-ordering pass, fitted all at once (Note HY13). A 4-colour pass of `total_orderings`
//! tries the level's first `orderings4` (at most 128) orderings of one row of `BEST_TOTAL_ORDERINGS4`, a 3-colour pass
//! of `try_3color_block` the first `orderings3` (at most 32) of one row of `BEST_TOTAL_ORDERINGS3` (Note HY15);
//! [`keys4`] and [`keys3`] return, for each, the pair the trial scores, as its [`Seen`] key: the single-colour pair's
//! key for an ordering with all 16 texels on one selector (`TOTAL_ORDER_4_16`, `TOTAL_ORDER_3_16`), else the key of
//! the rounded least-squares fit of the ordering.
//!
//! Why fitting the whole row before scoring it changes nothing: a trial's pair depends only on the ordering index, the
//! pass's prefix sums, the block's totals and its average (through the single-colour pair). All four are fixed before
//! the pass starts, and nothing the scoring loop does (the set of scored pairs, the error to beat, the taken endpoints
//! and selectors) feeds back into them. So each trial gets the pair it had when it was fitted inside the loop, and the
//! loop scores the pairs in the same order with the same `Seen` and error as before.

use super::fit::OrderTables;
use super::tables::{BEST_TOTAL_ORDERINGS3, MAX_TOTAL_ORDERINGS4};
use crate::path::Isa;

/// `MAX_TOTAL_ORDERINGS3` (rgbcx.h): the width of the generated table's rows.
pub(super) const MAX_TOTAL_ORDERINGS3: usize = BEST_TOTAL_ORDERINGS3[0].len();
const _: () = assert!(MAX_TOTAL_ORDERINGS3 == 32);

/// The [`Seen`] key of the endpoint pair of each ordering `row[q]` of one pass, for `q` below the level's
/// `orderings4`: `single_key` when the ordering is one of `TOTAL_ORDER_4_16`, else the key of `round_ls` of
/// `ls_endpoints4_ordered(total, tables.factors[row[q]], row[q], sums)`. The entries from `orderings4` on are 0 and
/// are not keys: the caller reads `..orderings4`. `total` is the block's channel sums and `sums` the pass's prefix sums
/// (`sorted_sums`).
///
/// The row's first `orderings4` entries are the orderings the level tries: the C++ loops to
/// `clampi(total_orderings_to_try, MIN_TOTAL_ORDERINGS, MAX_TOTAL_ORDERINGS4)`, which is `orderings4` at every level
/// (`params.rs` asserts it is inside the clamp). The level is a const generic so that the count is a constant in each
/// instance.
///
/// The kernel is the stamp compiled for `isa`'s path (as in [`keys3`]); the scalar code where the target lacks SSE2.
///
/// [`Seen`]: super::seen::Seen
#[inline(always)]
pub(crate) fn keys4<const LEVEL: u32>(
    isa: impl Isa,
    total: [i32; 3],
    sums: &[[u32; 17]; 3],
    row: &[u16; MAX_TOTAL_ORDERINGS4],
    tables: &OrderTables,
    single_key: u32,
) -> [u32; MAX_TOTAL_ORDERINGS4] {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    let keys = {
        let keys = if let Some(_t) = isa.avx2() {
            #[cfg(test)]
            crate::path::kernel_ran(crate::path::Wide::Keys4);
            // SAFETY: `_t` exists, so `Avx2::require` found AVX2.
            unsafe { sse2::keys4_avx2::<LEVEL>(total, sums, row, tables, single_key) }
        } else {
            // SAFETY: this arm is compiled only where the target has SSE2 (the cfg above).
            unsafe { sse2::keys4::<LEVEL>(total, sums, row, tables, single_key) }
        };
        // Shadow check: every real call in the unit tests also runs the code the kernel replaces.
        #[cfg(test)]
        assert_eq!(
            keys,
            scalar::keys4::<LEVEL>(total, sums, row, tables, single_key)
        );
        keys
    };
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    let keys = {
        // The scalar code serves every path here.
        let _ = isa;
        scalar::keys4::<LEVEL>(total, sums, row, tables, single_key)
    };
    keys
}

/// [`keys4`] for the 3-colour pass of `try_3color_block`: the [`Seen`] key of the endpoint pair of each ordering
/// `row[q]`, for `q` below the level's `orderings3` (the rest 0, not read): `single_key` when the ordering is one of
/// `TOTAL_ORDER_3_16`, else the key of `round_ls` of `ls_endpoints3_ordered(total, tables.factors3[row[q]], row[q],
/// sums)`.
///
/// The row's first `orderings3` entries are the orderings the level tries: the C++ loops to
/// `std::min(total_orderings_to_try, MAX_TOTAL_ORDERINGS3)`, which is `orderings3` (8 or 32, `MAX_TOTAL_ORDERINGS3`
/// 32) (`cEncodeBC1Exhaustive` is level 19 only, so the `std::min` bound is what the ported levels use).
///
/// [`Seen`]: super::seen::Seen
#[inline(always)]
pub(crate) fn keys3<const LEVEL: u32>(
    isa: impl Isa,
    total: [i32; 3],
    sums: &[[u32; 17]; 3],
    row: &[u8; MAX_TOTAL_ORDERINGS3],
    tables: &OrderTables,
    single_key: u32,
) -> [u32; MAX_TOTAL_ORDERINGS3] {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    let keys = {
        let keys = if let Some(_t) = isa.avx2() {
            #[cfg(test)]
            crate::path::kernel_ran(crate::path::Wide::Keys3);
            // SAFETY: `_t` exists, so `Avx2::require` found AVX2.
            unsafe { sse2::keys3_avx2::<LEVEL>(total, sums, row, tables, single_key) }
        } else {
            // SAFETY: this arm is compiled only where the target has SSE2 (the cfg above).
            unsafe { sse2::keys3::<LEVEL>(total, sums, row, tables, single_key) }
        };
        // Shadow check: every real call in the unit tests also runs the code the kernel replaces.
        #[cfg(test)]
        assert_eq!(
            keys,
            scalar::keys3::<LEVEL>(total, sums, row, tables, single_key)
        );
        keys
    };
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    let keys = {
        // The scalar code serves every path here.
        let _ = isa;
        scalar::keys3::<LEVEL>(total, sums, row, tables, single_key)
    };
    keys
}

// The kernel the encode uses where the target has SSE2 (`x86_64-pc-windows-msvc`, the workspace's target, always).
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod sse2;

/// The per-trial code `total_orderings` and `try_3color_block` ran before the kernel, one ordering at a time: the
/// kernel's reference, and what a target without SSE2 runs.
#[cfg(any(test, not(all(target_arch = "x86_64", target_feature = "sse2"))))]
mod scalar {
    #[cfg_attr(
        not(test),
        allow(
            unused_imports,
            reason = "`hit!` expands to nothing outside the unit tests, so `Hit` is unused there; this module builds outside them only for a target without SSE2"
        )
    )]
    use crate::hits::{hit, Hit};

    use super::super::encode::round_ls;
    use super::super::fit::{
        ls_endpoints3_ordered, ls_endpoints4_ordered, OrderTables, TOTAL_ORDER_3_16,
        TOTAL_ORDER_4_16,
    };
    use super::super::params::params;
    use super::super::seen::Seen;
    use super::super::tables::MAX_TOTAL_ORDERINGS4;
    use super::MAX_TOTAL_ORDERINGS3;

    /// [`super::keys4`], one ordering at a time.
    pub(super) fn keys4<const LEVEL: u32>(
        total: [i32; 3],
        sums: &[[u32; 17]; 3],
        row: &[u16; MAX_TOTAL_ORDERINGS4],
        tables: &OrderTables,
        single_key: u32,
    ) -> [u32; MAX_TOTAL_ORDERINGS4] {
        let n = const { params(LEVEL).orderings4 };
        let mut keys = [0u32; MAX_TOTAL_ORDERINGS4];
        for (key, &s_index) in keys.iter_mut().zip(row).take(n) {
            let s_index = s_index as usize;
            *key = if TOTAL_ORDER_4_16.contains(&s_index) {
                hit!(Hit::OrderingSingle);
                single_key
            } else {
                hit!(Hit::OrderingFit);
                let (xl, xh) = ls_endpoints4_ordered(total, tables.factors[s_index], s_index, sums);
                Seen::key(&round_ls(&xl, &xh))
            };
        }
        keys
    }

    /// [`super::keys3`], one ordering at a time.
    pub(super) fn keys3<const LEVEL: u32>(
        total: [i32; 3],
        sums: &[[u32; 17]; 3],
        row: &[u8; MAX_TOTAL_ORDERINGS3],
        tables: &OrderTables,
        single_key: u32,
    ) -> [u32; MAX_TOTAL_ORDERINGS3] {
        let n = const { params(LEVEL).orderings3 };
        let mut keys = [0u32; MAX_TOTAL_ORDERINGS3];
        for (key, &s_index) in keys.iter_mut().zip(row).take(n) {
            let s_index = s_index as usize;
            *key = if TOTAL_ORDER_3_16.contains(&s_index) {
                hit!(Hit::Ordering3Single);
                single_key
            } else {
                hit!(Hit::Ordering3Fit);
                let (xl, xh) =
                    ls_endpoints3_ordered(total, tables.factors3[s_index], s_index, sums);
                Seen::key(&round_ls(&xl, &xh))
            };
        }
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::super::encode::sorted_sums;
    use super::super::fit::{order_tables, TOTAL_ORDER_3_16, TOTAL_ORDER_4_16};
    use super::super::tables::{N3, N4};
    use super::super::Ends;
    use super::*;
    use crate::path::Base;
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    use crate::path::{avx2_tested, Avx2};

    /// Rows that together hold every 4-colour ordering index.
    fn rows4() -> Vec<[u16; MAX_TOTAL_ORDERINGS4]> {
        (0..N4)
            .collect::<Vec<_>>()
            .chunks(MAX_TOTAL_ORDERINGS4)
            .map(|c| std::array::from_fn(|q| c.get(q).map_or(0, |&s| s as u16)))
            .collect()
    }

    /// The named blocks plus generated ones: all 0, all 255, half 0 and half 255, a ramp, and 40 blocks of each
    /// colour class.
    fn blocks() -> Vec<[[u8; 4]; 16]> {
        let mut b = vec![
            [[0, 0, 0, 255]; 16],
            [[255, 255, 255, 255]; 16],
            std::array::from_fn(|i| if i < 8 { [0, 0, 0, 255] } else { [255; 4] }),
            std::array::from_fn(|i| {
                let v = (i * 17) as u8;
                [v, 255 - v, v / 2, 255]
            }),
        ];
        b.extend(crate::gen::colour_classes(0xB2_0001, 40));
        b
    }

    /// The block's channel sums, as `encode` builds `BlockStats::total`.
    fn total(px: &[[u8; 4]; 16]) -> [i32; 3] {
        std::array::from_fn(|c| px.iter().map(|t| t[c] as i32).sum())
    }

    /// Three axes to sort the texels along.
    fn axes() -> [Ends; 3] {
        [
            Ends {
                l: [0, 0, 0],
                h: [31, 63, 31],
            },
            Ends {
                l: [31, 63, 31],
                h: [0, 0, 0],
            },
            Ends {
                l: [31, 0, 0],
                h: [0, 63, 31],
            },
        ]
    }

    /// Rows that together hold every 3-colour ordering index.
    fn rows3() -> Vec<[u8; MAX_TOTAL_ORDERINGS3]> {
        (0..N3)
            .collect::<Vec<_>>()
            .chunks(MAX_TOTAL_ORDERINGS3)
            .map(|c| std::array::from_fn(|q| c.get(q).map_or(0, |&s| s as u8)))
            .collect()
    }

    #[test]
    fn every_3_colour_ordering_matches_the_reference() {
        // Meaningful only where the SSE2 kernel exists: on a target without SSE2 both sides are the scalar code, and
        // the scalar path is then covered by the differential tests against `encoders_sys`.
        const SINGLE: u32 = 0xDEAD_BEEF;
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        let avx2 = avx2_tested();
        let tables = order_tables();
        let rows = rows3();
        for px in blocks() {
            let total = total(&px);
            for axis in &axes() {
                let sums = sorted_sums(&px, axis, total);
                for row in &rows {
                    // Level 18 fits the whole row.
                    let keys = keys3::<18>(Base, total, &sums, row, tables, SINGLE);
                    assert_eq!(
                        keys,
                        scalar::keys3::<18>(total, &sums, row, tables, SINGLE),
                        "{px:?} along {axis:?}"
                    );
                    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
                    if avx2 {
                        assert_eq!(
                            keys3::<18>(Avx2::require(), total, &sums, row, tables, SINGLE),
                            keys,
                            "AVX2 stamp, {px:?} along {axis:?}"
                        );
                    }
                    for (q, &s) in row.iter().enumerate() {
                        if TOTAL_ORDER_3_16.contains(&(s as usize)) {
                            assert_eq!(keys[q], SINGLE, "ordering {s}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_ordering_matches_the_reference() {
        // Meaningful only where the SSE2 kernel exists: on a target without SSE2 both sides are the scalar code, and
        // the scalar path is then covered by the differential tests against `encoders_sys`.
        const SINGLE: u32 = 0xDEAD_BEEF;
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        let avx2 = avx2_tested();
        let axes = axes();
        let tables = order_tables();
        let rows = rows4();
        for px in blocks() {
            let total = total(&px);
            for axis in &axes {
                let sums = sorted_sums(&px, axis, total);
                for row in &rows {
                    // Level 18 fits the whole row.
                    let keys = keys4::<18>(Base, total, &sums, row, tables, SINGLE);
                    assert_eq!(
                        keys,
                        scalar::keys4::<18>(total, &sums, row, tables, SINGLE),
                        "{px:?} along {axis:?}"
                    );
                    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
                    if avx2 {
                        assert_eq!(
                            keys4::<18>(Avx2::require(), total, &sums, row, tables, SINGLE),
                            keys,
                            "AVX2 stamp, {px:?} along {axis:?}"
                        );
                    }
                    for (q, &s) in row.iter().enumerate() {
                        if TOTAL_ORDER_4_16.contains(&(s as usize)) {
                            assert_eq!(keys[q], SINGLE, "ordering {s}");
                        }
                    }
                }
            }
        }
    }

    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    mod sse2_kernel {
        use super::super::super::encode::round_ls;
        use super::super::super::fit::{MIDPOINT5, MIDPOINT6};
        use super::super::super::seen::Seen;
        use super::super::keys4;
        use super::super::sse2::round_key_of;
        use super::{avx2_tested, Avx2, MAX_TOTAL_ORDERINGS4, N3, N4};
        use crate::bc1::fit::OrderTables;

        // 41,500 is above the largest real fit: |iz| is at most 3/255 x 144 and the sums at most 12,240, two terms.
        const CLAMP_VALUES: [f32; 10] = [
            -41_500.0,
            -1.0,
            -1e-6,
            -0.0,
            0.0,
            1.0,
            1.0f32.next_up(),
            1.02,
            2.5,
            41_500.0,
        ];

        /// The kernel's rounding of (`xl`, `xh`) against the scalar code's.
        fn check(xl: [f32; 3], xh: [f32; 3]) {
            assert_eq!(
                round_key_of(xl, xh),
                Seen::key(&round_ls(&xl, &xh)),
                "xl {xl:?} xh {xh:?}"
            );
        }

        /// `x` in each of the six positions in turn (three of `xl`, three of `xh`), the other five at `rest`.
        fn check_each_position(x: f32, rest: f32) {
            for i in 0..6 {
                let mut v = [rest; 6];
                v[i] = x;
                check([v[0], v[1], v[2]], [v[3], v[4], v[5]]);
            }
        }

        #[test]
        fn the_rounding_steps_exactly_at_each_threshold() {
            // The last threshold of each table (1e37) is left out: no fit reaches it.
            for &m in MIDPOINT5[..31].iter().chain(&MIDPOINT6[..63]) {
                for x in [m.next_down(), m, m.next_up()] {
                    check_each_position(x, 0.5);
                }
            }
            for c in 0..=31 {
                check_each_position(c as f32 / 31.0, 0.5);
            }
            for c in 0..=63 {
                check_each_position(c as f32 / 63.0, 0.5);
            }
        }

        /// The threshold, grid and clamp probes of the tests above and below, as one list.
        fn probes() -> Vec<f32> {
            let mut out = Vec::new();
            for &m in MIDPOINT5[..31].iter().chain(&MIDPOINT6[..63]) {
                out.extend([m.next_down(), m, m.next_up()]);
            }
            out.extend((0..=31).map(|c| c as f32 / 31.0));
            out.extend((0..=63).map(|c| c as f32 / 63.0));
            out.extend(CLAMP_VALUES);
            out
        }

        /// The same probes through the AVX2 stamp of the ordering kernel (`keys4_avx2`, which `keys4` picks for the
        /// AVX2 token), against the scalar code's rounding. Ordering 0 is [0, 8, 2, 6], so it reads the prefix sums at
        /// positions 0, 8 and 10: with the sums of channel `c` 1 from position 10 on (a real, non-decreasing prefix
        /// sum) and `total[c]` 1, channel `c` has q = 1 and uq = 2, and the other channels 0. Factors (0, a, b) then
        /// give xl = a and xh = 2a + b, exactly: (0, x, -x) puts x in both, (0, x, -2x) in `xl` only, and (0, 0, x) in
        /// `xh` only.
        #[test]
        fn the_avx2_stamp_rounds_the_probes_as_the_scalar_code() {
            if !avx2_tested() {
                return;
            }
            let t = Avx2::require();
            let mut tables = OrderTables {
                hash: [0; 4096],
                factors: [[0.0; 3]; N4],
                hash3: [0; 256],
                factors3: [[0.0; 3]; N3],
            };
            let row = [0u16; MAX_TOTAL_ORDERINGS4];
            for x in probes() {
                for c in 0..3 {
                    let mut sums = [[0u32; 17]; 3];
                    sums[c][10..].fill(1);
                    let mut total = [0; 3];
                    total[c] = 1;
                    let modes = [
                        (x, 0.0, [0.0, x, -2.0 * x]),
                        (0.0, x, [0.0, 0.0, x]),
                        (x, x, [0.0, x, -x]),
                    ];
                    for (xl, xh, iz) in modes {
                        tables.factors[0] = iz;
                        let key = keys4::<18>(t, total, &sums, &row, &tables, 0xDEAD_BEEF)[0];
                        let (mut l, mut h) = ([0.0f32; 3], [0.0f32; 3]);
                        l[c] = xl;
                        h[c] = xh;
                        assert_eq!(
                            key,
                            Seen::key(&round_ls(&l, &h)),
                            "AVX2 stamp: xl {l:?} xh {h:?}"
                        );
                    }
                }
            }
        }

        #[test]
        fn out_of_range_fits_clamp_as_the_scalar_code() {
            let values = CLAMP_VALUES;
            // Positions 0..3 are xl's r, g, b and 3..6 xh's.
            let groups: [&[usize]; 9] = [
                &[0],
                &[1],
                &[2],
                &[3],
                &[4],
                &[5],
                &[1, 4],
                &[0, 2, 3, 5],
                &[0, 1, 2, 3, 4, 5],
            ];
            for x in values {
                for group in groups {
                    let mut v = [0.25f32; 6];
                    for &i in group {
                        v[i] = x;
                    }
                    check([v[0], v[1], v[2]], [v[3], v[4], v[5]]);
                }
            }
        }
    }
}
