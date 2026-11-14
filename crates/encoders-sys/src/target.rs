use std::sync::OnceLock;

/// ISPC code path of the vendored objects (bc7e and the BC6H texture compressor). The dispatcher picks the best one
/// the CPU runs, and the paths do not all encode bit-identically, so a pinned output hash is only meaningful per
/// target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IspcTarget {
    Sse2 = 0,
    Sse4 = 1,
    Avx = 2,
    Avx2 = 3,
    Avx512 = 4,
}

impl IspcTarget {
    pub const ALL: [IspcTarget; 5] = [Self::Sse2, Self::Sse4, Self::Avx, Self::Avx2, Self::Avx512];

    /// The target ISPC's dispatcher picks on this CPU (see the `shortcut:` note at `forced`).
    pub fn best() -> IspcTarget {
        *Self::ALL.iter().rev().find(|t| t.supported()).unwrap()
    }

    /// Whether this CPU can run the target (ISPC 1.31: sse4 is SSE4.2; avx2 also needs FMA, F16C, BMI1/2;
    /// avx512skx needs AVX-512 F, DQ, CD, BW and VL, as ISPC's dispatcher checks).
    pub fn supported(self) -> bool {
        match self {
            Self::Sse2 => true,
            Self::Sse4 => std::arch::is_x86_feature_detected!("sse4.2"),
            Self::Avx => std::arch::is_x86_feature_detected!("avx"),
            Self::Avx2 => {
                std::arch::is_x86_feature_detected!("avx2")
                    && std::arch::is_x86_feature_detected!("fma")
                    && std::arch::is_x86_feature_detected!("f16c")
                    && std::arch::is_x86_feature_detected!("bmi1")
                    && std::arch::is_x86_feature_detected!("bmi2")
            }
            Self::Avx512 => {
                Self::Avx2.supported()
                    && std::arch::is_x86_feature_detected!("avx512f")
                    && std::arch::is_x86_feature_detected!("avx512dq")
                    && std::arch::is_x86_feature_detected!("avx512cd")
                    && std::arch::is_x86_feature_detected!("avx512bw")
                    && std::arch::is_x86_feature_detected!("avx512vl")
            }
        }
    }

    /// The name recorded for the target (`identity.json`, the manifest).
    pub fn name(self) -> &'static str {
        match self {
            Self::Sse2 => "sse2",
            Self::Sse4 => "sse4",
            Self::Avx => "avx",
            Self::Avx2 => "avx2",
            Self::Avx512 => "avx512",
        }
    }
}

/// The target every plain encode runs on: `IspcTarget::best()`, computed once. A test build (feature `test-hooks`)
/// caps it at AVX2, so the pins hold on every CPU; `SDC_TEST_AVX512=1` lifts the cap.
pub fn ispc_target() -> IspcTarget {
    static TARGET: OnceLock<IspcTarget> = OnceLock::new();
    #[cfg(feature = "test-hooks")]
    let init = || {
        capped(
            IspcTarget::best(),
            std::env::var("SDC_TEST_AVX512").as_deref() == Ok("1"),
        )
    };
    #[cfg(not(feature = "test-hooks"))]
    let init = IspcTarget::best;
    *TARGET.get_or_init(init)
}

/// `best`, or at most AVX2 unless `lift`.
#[cfg(feature = "test-hooks")]
fn capped(best: IspcTarget, lift: bool) -> IspcTarget {
    if lift {
        best
    } else {
        best.min(IspcTarget::Avx2)
    }
}

/// The dispatch rule, stated once: the shim gets `-1` (ISPC's dispatcher) for the best target, whose per-target
/// init the dispatcher already ran, and the target's number otherwise (the shim inits that target once).
// shortcut: the rule assumes `best()` is the target ISPC's dispatcher runs. On a CPU that reports AVX2, FMA and F16C
// without BMI1/2 they differ (`best()` is Avx, the dispatcher runs AVX2); no such hardware is known; upgrade if one
// appears. The same can happen one level up: `Avx512.supported()` also requires `Avx2.supported()` (BMI1/2
// included), which ISPC's avx512skx check is not known to require, so a CPU with AVX-512 but no BMI (none known)
// would get `best()` Avx while the dispatcher runs AVX-512.
pub(crate) fn forced(target: IspcTarget) -> i32 {
    if target == IspcTarget::best() {
        -1
    } else {
        target as i32
    }
}

#[cfg(all(test, feature = "test-hooks"))]
mod tests {
    use super::*;

    #[test]
    fn the_cap_stops_at_avx2_unless_lifted() {
        for t in IspcTarget::ALL {
            assert_eq!(capped(t, true), t);
            assert_eq!(capped(t, false), t.min(IspcTarget::Avx2));
        }
        assert_eq!(capped(IspcTarget::Avx512, false), IspcTarget::Avx2);
    }
}
