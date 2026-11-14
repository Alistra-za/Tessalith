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
}

impl IspcTarget {
    pub const ALL: [IspcTarget; 4] = [Self::Sse2, Self::Sse4, Self::Avx, Self::Avx2];

    /// The target ISPC's dispatcher picks on this CPU (see the `shortcut:` note at `forced`).
    pub fn best() -> IspcTarget {
        *Self::ALL.iter().rev().find(|t| t.supported()).unwrap()
    }

    /// Whether this CPU can run the target (ISPC 1.31: sse4 is SSE4.2; avx2 also needs FMA, F16C, BMI1/2).
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
        }
    }

    /// The name recorded for the target (`identity.json`, the manifest).
    pub fn name(self) -> &'static str {
        match self {
            Self::Sse2 => "sse2",
            Self::Sse4 => "sse4",
            Self::Avx => "avx",
            Self::Avx2 => "avx2",
        }
    }
}

/// The target every plain encode runs on: `IspcTarget::best()`, computed once.
pub fn ispc_target() -> IspcTarget {
    static TARGET: OnceLock<IspcTarget> = OnceLock::new();
    *TARGET.get_or_init(IspcTarget::best)
}

/// The dispatch rule, stated once: the shim gets `-1` (ISPC's dispatcher) for the best target, whose per-target
/// init the dispatcher already ran, and the target's number otherwise (the shim inits that target once).
// shortcut: the rule assumes `best()` is the target ISPC's dispatcher runs. On a CPU that reports AVX2, FMA and F16C
// without BMI1/2 they differ (`best()` is Avx, the dispatcher runs AVX2); no such hardware is known; upgrade if one
// appears.
pub(crate) fn forced(target: IspcTarget) -> i32 {
    if target == IspcTarget::best() {
        -1
    } else {
        target as i32
    }
}
