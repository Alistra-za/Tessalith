//! Branch counters for the ported code. `hit!(Hit::X)` marks one branch; it adds 1 to a counter in the crate's own
//! unit tests (`#[cfg(test)]`) and expands to nothing everywhere else, so the engine and the integration tests carry
//! no counters. `coverage_tests` asserts every variant is taken, so a branch no generator reaches is found.
#![cfg_attr(not(test), allow(dead_code))]

/// Declares `Hit` and `Hit::ALL` from one list, so a new branch is named once.
macro_rules! hits {
    ($($name:ident,)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum Hit {
            $($name,)*
        }

        impl Hit {
            pub(crate) const ALL: &'static [Hit] = &[$(Hit::$name,)*];
            pub(crate) const COUNT: usize = Hit::ALL.len();
        }
    };
}

hits! {
    // Best::try_pair
    PairExactTexel,
    PairEarlyOut,
    PairImproved,
    // v0
    V0Solid,
    V0EqualEnds,
    V0SwapMode0,
    V0SwapMode1,
    // inner6
    Inner6NoExtreme,
    Inner6NoInner,
    Inner6Pair,
    Inner6Skip,
    // ls_refine
    LsFixedTexel,
    LsSingular,
    LsPair,
    LsModeSkip,
    LsNoGain,
    LsRoundsExhausted,
    // refit_both
    RefitSolid,
    RefitEqualEnds,
    RefitSwap,
    RefitOtherBetter,
    RefitOtherKept,
    // trimmed6
    TrimNoExtreme,
    TrimNoInner,
    TrimPair,
    TrimSkip,
    TrimRefit,
    TrimNoRefit,
    TrimReplaced,
    TrimKept,
    // local
    LocalPair,
    LocalModeSkip,
    // search guard
    NoGuard,
    GuardPassed,
    GuardFallback,
}

#[cfg(test)]
static COUNTS: [std::sync::atomic::AtomicU64; Hit::COUNT] =
    [const { std::sync::atomic::AtomicU64::new(0) }; Hit::COUNT];

#[cfg(test)]
pub(crate) fn bump(h: Hit) {
    COUNTS[h as usize].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// Every variant with its count so far.
#[cfg(test)]
pub(crate) fn counts() -> Vec<(Hit, u64)> {
    Hit::ALL
        .iter()
        .map(|&h| {
            (
                h,
                COUNTS[h as usize].load(std::sync::atomic::Ordering::Relaxed),
            )
        })
        .collect()
}

/// Counts one taken branch (tests of this crate only; nothing otherwise).
macro_rules! hit {
    ($h:expr) => {
        #[cfg(test)]
        {
            $crate::hits::bump($h);
        }
    };
}
pub(crate) use hit;
