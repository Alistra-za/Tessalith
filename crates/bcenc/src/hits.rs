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
    // bc1 palette
    PaletteThree,
    PaletteFour,
    // bc1 single-colour tables
    SingleHalf,
    SingleFull,
    Single5,
    Single6,
    SingleImproved,
    SingleTieEqualEnds,
    // search guard
    NoGuard,
    GuardPassed,
    GuardFallback,
    // bc1 encode4
    Bc1Solid,
    LsPassSame,
    LsPassBetter,
    LsPassWorse,
    RoundTaken,
    RoundWorse,
    OrderingsRun,
    OrderingsSkippedExact,
    OrderingsExact,
    OrderingsNoGain,
    OrderingSingle,
    OrderingFit,
    OrderingBetter,
    SearchRun,
    // bc1 endpoint_search
    SearchForbidden,
    SearchLow,
    SearchHigh,
    SearchBetter,
    SearchStop,
    // bc1_encode4
    PackEqual,
    PackEqualBlack,
    PackSwap,
    PackInOrder,
    // bc1 solid_block
    SolidEqual,
    SolidEqualBlack,
    SolidSwap,
    // bc1 pick_initial
    InitGreyNarrow,
    InitGreyRange,
    InitBox,
    InitBoxSwapR,
    InitBoxSwapG,
    InitAxis,
    InitAxisFlipR,
    InitAxisFlipG,
    InitAxisScaled,
    InitAxisDefault,
    // bc1 fit
    Hist16Sel0,
    Hist16Sel1,
    Hist16Sel2,
    Hist16Sel3,
    HistHashed,
    FactorsSingular,
    FactorsSolved,
    LsSelsSingular,
    LsSelsSolved,
    RoundClampRb,
    RoundClampG,
    // bc1 find_sels4
    SelsBetter,
    SelsTieHigh,
    SelsEarlyOut,
    // bc1 three-colour: solid_block
    SolidThree,
    SolidThreeSwap,
    SolidThreeLoses,
    // encode
    Try3Run,
    Try3SkippedExact,
    // try_3color_block
    Try3Fit,
    Try3StartExact,
    Ls3PassSame,
    Ls3PassBetter,
    Ls3PassWorse,
    Orderings3Run,
    Orderings3SkippedExact,
    Ordering3Single,
    Ordering3Fit,
    Ordering3Better,
    Try3Taken,
    Try3Rejected,
    // endpoint_search
    SearchThree,
    SearchFour,
    // bc1_encode3
    Pack3Swap,
    Pack3InOrder,
    // fit
    Hist3Sel0,
    Hist3Sel1,
    Hist3Sel2,
    Hist3Hashed,
    Factors3Singular,
    Factors3Solved,
    LsSels3Singular,
    LsSels3Solved,
    // find_sels3
    Sels3Better,
    Sels3EarlyOut,
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
