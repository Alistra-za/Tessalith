//! The endpoint pairs of one 4-colour total-ordering pass, fitted all at once (Note HY13). A pass of
//! `total_orderings` tries the 128 orderings of one row of `BEST_TOTAL_ORDERINGS4`; [`keys4`] returns, for each, the
//! pair the trial scores, as its [`Seen`] key: the single-colour pair's key for an ordering with all 16 texels on one
//! selector (`TOTAL_ORDER_4_16`), else the key of the rounded least-squares fit of the ordering.
//!
//! Why fitting the whole row before scoring it changes nothing: a trial's pair depends only on the ordering index, the
//! pass's prefix sums, the block's totals and its average (through the single-colour pair). All four are fixed before
//! the pass starts, and nothing the scoring loop does (the set of scored pairs, the error to beat, the taken endpoints
//! and selectors) feeds back into them. So each trial gets the pair it had when it was fitted inside the loop, and the
//! loop scores the pairs in the same order with the same `Seen` and error as before.

use super::encode::MAX_TOTAL_ORDERINGS3;
use super::fit::OrderTables;
use super::tables::MAX_TOTAL_ORDERINGS4;

/// The [`Seen`] key of the endpoint pair of each ordering `row[q]` of one pass: `single_key` when the ordering is one
/// of `TOTAL_ORDER_4_16`, else the key of `round_ls` of `ls_endpoints4_ordered(total, tables.factors[row[q]], row[q],
/// sums)`. `total` is the block's channel sums and `sums` the pass's prefix sums (`sorted_sums`).
///
/// The row's 128 entries are the 128 orderings level 18 tries: the C++ loops to
/// `clampi(total_orderings_to_try, MIN_TOTAL_ORDERINGS, MAX_TOTAL_ORDERINGS4)` with `MAX_TOTAL_ORDERINGS4` passed.
///
/// [`Seen`]: super::seen::Seen
pub(crate) fn keys4(
    total: [i32; 3],
    sums: &[[u32; 17]; 3],
    row: &[u16; MAX_TOTAL_ORDERINGS4],
    tables: &OrderTables,
    single_key: u32,
) -> [u32; MAX_TOTAL_ORDERINGS4] {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    let keys = {
        // SAFETY: this arm is compiled only where the target has SSE2 (the cfg above).
        let keys = unsafe { sse2::keys4(total, sums, row, tables, single_key) };
        // Shadow check: every real call in the unit tests also runs the code the kernel replaces.
        #[cfg(test)]
        assert_eq!(keys, scalar::keys4(total, sums, row, tables, single_key));
        keys
    };
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    let keys = scalar::keys4(total, sums, row, tables, single_key);
    keys
}

/// [`keys4`] for the 3-colour pass of `try_3color_block`: the [`Seen`] key of the endpoint pair of each ordering
/// `row[q]`: `single_key` when the ordering is one of `TOTAL_ORDER_3_16`, else the key of `round_ls` of
/// `ls_endpoints3_ordered(total, tables.factors3[row[q]], row[q], sums)`.
///
/// The row's 32 entries are the 32 orderings level 18 tries: the C++ loops to
/// `std::min(total_orderings_to_try, MAX_TOTAL_ORDERINGS3)` with 32 passed and `MAX_TOTAL_ORDERINGS3` 32.
///
/// [`Seen`]: super::seen::Seen
pub(crate) fn keys3(
    total: [i32; 3],
    sums: &[[u32; 17]; 3],
    row: &[u8; MAX_TOTAL_ORDERINGS3],
    tables: &OrderTables,
    single_key: u32,
) -> [u32; MAX_TOTAL_ORDERINGS3] {
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    let keys = {
        // SAFETY: this arm is compiled only where the target has SSE2 (the cfg above).
        let keys = unsafe { sse2::keys3(total, sums, row, tables, single_key) };
        // Shadow check: every real call in the unit tests also runs the code the kernel replaces.
        #[cfg(test)]
        assert_eq!(keys, scalar::keys3(total, sums, row, tables, single_key));
        keys
    };
    #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
    let keys = scalar::keys3(total, sums, row, tables, single_key);
    keys
}

// The kernel the encode uses where the target has SSE2 (`x86_64-pc-windows-msvc`, the workspace's target, always).
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod sse2;

/// The per-trial code `total_orderings` ran before the kernel, one ordering at a time: the kernel's reference, and
/// what a target without SSE2 runs.
#[cfg(any(test, not(all(target_arch = "x86_64", target_feature = "sse2"))))]
mod scalar {
    // `hit!` expands to nothing outside the unit tests, which leaves `Hit` unused there.
    #[cfg_attr(not(test), allow(unused_imports))]
    use crate::hits::{hit, Hit};

    use super::super::encode::round_ls;
    use super::super::encode::MAX_TOTAL_ORDERINGS3;
    use super::super::fit::{
        ls_endpoints3_ordered, ls_endpoints4_ordered, OrderTables, TOTAL_ORDER_3_16,
        TOTAL_ORDER_4_16,
    };
    use super::super::seen::Seen;
    use super::super::tables::MAX_TOTAL_ORDERINGS4;

    /// [`super::keys4`], one ordering at a time.
    pub(super) fn keys4(
        total: [i32; 3],
        sums: &[[u32; 17]; 3],
        row: &[u16; MAX_TOTAL_ORDERINGS4],
        tables: &OrderTables,
        single_key: u32,
    ) -> [u32; MAX_TOTAL_ORDERINGS4] {
        let mut keys = [0u32; MAX_TOTAL_ORDERINGS4];
        for (key, &s_index) in keys.iter_mut().zip(row) {
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
    pub(super) fn keys3(
        total: [i32; 3],
        sums: &[[u32; 17]; 3],
        row: &[u8; MAX_TOTAL_ORDERINGS3],
        tables: &OrderTables,
        single_key: u32,
    ) -> [u32; MAX_TOTAL_ORDERINGS3] {
        let mut keys = [0u32; MAX_TOTAL_ORDERINGS3];
        for (key, &s_index) in keys.iter_mut().zip(row) {
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
        const SINGLE: u32 = 0xDEAD_BEEF;
        let tables = order_tables();
        let rows = rows3();
        for px in blocks() {
            let total = total(&px);
            for axis in &axes() {
                let sums = sorted_sums(&px, axis, total);
                for row in &rows {
                    let keys = keys3(total, &sums, row, tables, SINGLE);
                    assert_eq!(
                        keys,
                        scalar::keys3(total, &sums, row, tables, SINGLE),
                        "{px:?} along {axis:?}"
                    );
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
        const SINGLE: u32 = 0xDEAD_BEEF;
        let axes = axes();
        let tables = order_tables();
        let rows = rows4();
        for px in blocks() {
            let total = total(&px);
            for axis in &axes {
                let sums = sorted_sums(&px, axis, total);
                for row in &rows {
                    let keys = keys4(total, &sums, row, tables, SINGLE);
                    assert_eq!(
                        keys,
                        scalar::keys4(total, &sums, row, tables, SINGLE),
                        "{px:?} along {axis:?}"
                    );
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
        use super::super::sse2::round_key_of;

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

        #[test]
        fn out_of_range_fits_clamp_as_the_scalar_code() {
            // 41,500 is above the largest real fit: |iz| is at most 3/255 x 144 and the sums at most 12,240, two terms.
            let values = [
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
