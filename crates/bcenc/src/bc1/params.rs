//! BC1's search depth per rgbcx level (Note HY15): the six numbers in which `encode_bc1(level, ..)`'s flags and
//! orderings differ at the levels `bcenc` ports. Every other flag test of the C++ resolves the same at these levels:
//! `cEncodeBC1TwoLeastSquaresPasses` and `cEncodeBC1UseLikelyTotalOrderings` are set at all of them, and
//! `cEncodeBC1UseFullMSEEval` (13, 15, 18) has no effect under `cBC1Dual`, whose selector searches ignore it.

use super::order_keys::MAX_TOTAL_ORDERINGS3;
use super::tables::MAX_TOTAL_ORDERINGS4;

/// One level's search depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Params {
    /// Power iterations of the principal-axis initial endpoints: 6 with `cEncodeBC1Use6PowerIters`, else 4.
    pub power_iters: usize,
    /// Initial-endpoint rounds: 2 with `cEncodeBC1TryAllInitialEndponts` (round 1 is the bounding box), else 1.
    pub rounds: usize,
    /// 4-colour total-ordering passes: 2 with `cEncodeBC1Iterative`, else 1.
    pub passes: usize,
    /// 4-colour orderings tried per pass, the first of a `g_best_total_orderings4` row (`total_orderings_to_try`).
    pub orderings4: usize,
    /// 3-colour orderings tried, the first of a `g_best_total_orderings3` row (`total_orderings_to_try3`).
    pub orderings3: usize,
    /// Endpoint-search rounds (`flags >> cEncodeBC1EndpointSearchRoundsShift`); 0 runs no search.
    pub search_rounds: i32,
}

/// The rgbcx levels `bcenc` encodes BC1 at, byte-equal to rgbcx at each.
#[doc(hidden)]
pub const LEVELS: [u32; 5] = [10, 12, 13, 15, 18];

/// The search depth of rgbcx level `level` (rgbcx.cpp `encode_bc1(uint32_t level, ..)`, cases 10, 12, 13, 15 and 18).
/// Panics on any other level, so `encode::<11>` does not compile.
pub(crate) const fn params(level: u32) -> Params {
    let (power_iters, rounds, passes, orderings4, orderings3, search_rounds) = match level {
        10 => (4, 1, 1, 20, 8, 0),
        12 => (4, 1, 1, 32, 32, 0),
        13 => (6, 2, 1, 32, 32, 20),
        // ((((32 + MAX_TOTAL_ORDERINGS4) / 2) + 32) / 2) with MAX_TOTAL_ORDERINGS4 128.
        15 => (6, 2, 1, 56, 32, 32),
        18 => (6, 2, 2, MAX_TOTAL_ORDERINGS4, 32, 256),
        _ => panic!("bcenc encodes BC1 at rgbcx levels 10, 12, 13, 15 and 18 only"),
    };
    Params {
        power_iters,
        rounds,
        passes,
        orderings4,
        orderings3,
        search_rounds,
    }
}

/// The most endpoint pairs one block can score for the 4-colour block type at `p`: each round scores its start and
/// up to two least-squares trials, then `passes` ordering passes of `orderings4` trials and the search steps.
pub(crate) const fn most(p: Params) -> usize {
    3 * p.rounds + p.passes * p.orderings4 + p.search_rounds as usize
}

/// The most pairs one block can score for the 3-colour block type at `p`: `try_3color_block`'s start, its two
/// least-squares trials and `orderings3` ordering trials, then the search steps.
pub(crate) const fn most3(p: Params) -> usize {
    3 + p.orderings3 + p.search_rounds as usize
}

// rgbcx clamps the orderings it reads from a row: `clampi(total_orderings_to_try, MIN_TOTAL_ORDERINGS (1),
// MAX_TOTAL_ORDERINGS4)` and `std::min(total_orderings_to_try3, MAX_TOTAL_ORDERINGS3)` (a 0 there turns the 3-colour
// orderings off, which no level here does). Every level's values are inside both, so the clamps change nothing.
const _: () = {
    let mut i = 0;
    while i < LEVELS.len() {
        let p = params(LEVELS[i]);
        assert!(p.orderings4 >= 1 && p.orderings4 <= MAX_TOTAL_ORDERINGS4);
        assert!(p.orderings3 >= 1 && p.orderings3 <= MAX_TOTAL_ORDERINGS3);
        i += 1;
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        power_iters: usize,
        rounds: usize,
        passes: usize,
        o4: usize,
        o3: usize,
        search: i32,
    ) -> Params {
        Params {
            power_iters,
            rounds,
            passes,
            orderings4: o4,
            orderings3: o3,
            search_rounds: search,
        }
    }

    /// rgbcx.cpp `encode_bc1(level, ..)`: case 10 `TwoLeastSquaresPasses | UseLikelyTotalOrderings`, 20 and 8
    /// orderings; case 12 the same flags, 32 and 32; case 13 adds `UseFullMSEEval | Use6PowerIters |
    /// TryAllInitialEndponts` and 20 search rounds, 32 and 32; case 15 32 search rounds, 56 and 32; case 18 adds
    /// `Iterative`, 256 search rounds, `MAX_TOTAL_ORDERINGS4` (128) and 32.
    #[test]
    fn the_table_is_rgbcx() {
        assert_eq!(LEVELS, [10, 12, 13, 15, 18]);
        assert_eq!(params(10), row(4, 1, 1, 20, 8, 0));
        assert_eq!(params(12), row(4, 1, 1, 32, 32, 0));
        assert_eq!(params(13), row(6, 2, 1, 32, 32, 20));
        assert_eq!(params(15), row(6, 2, 1, 56, 32, 32));
        assert_eq!(params(18), row(6, 2, 2, 128, 32, 256));
        assert_eq!(MAX_TOTAL_ORDERINGS4, 128);
        assert_eq!(MAX_TOTAL_ORDERINGS3, 32);
        for level in LEVELS {
            let p = params(level);
            assert!(
                (1..=MAX_TOTAL_ORDERINGS4).contains(&p.orderings4),
                "level {level}"
            );
            assert!(
                (1..=MAX_TOTAL_ORDERINGS3).contains(&p.orderings3),
                "level {level}"
            );
        }
    }

    /// 2 round starts, 4 least-squares trials, 2 x 128 ordering trials and 256 search steps.
    #[test]
    fn most_is_518_at_level_18() {
        assert_eq!(most(params(18)), 518);
        assert_eq!(most3(params(18)), 291);
    }

    #[test]
    #[should_panic(expected = "levels 10, 12, 13, 15 and 18 only")]
    fn another_level_panics() {
        params(11);
    }
}
