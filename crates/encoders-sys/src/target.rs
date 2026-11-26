use std::sync::OnceLock;

/// ISPC code path of the vendored objects (bc7e and the BC6H texture compressor). The dispatcher picks the best one
/// the CPU runs, and the paths do not all encode bit-identically, so a pinned output hash is only meaningful per
/// target. The numbers are the shim's (`sdc_shim.cpp`); 1 and 2 are unused since Note JB dropped the SSE4 and AVX
/// objects. A CPU without AVX2 runs SSE2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IspcTarget {
    Sse2 = 0,
    Avx2 = 3,
    Avx512 = 4,
}

impl IspcTarget {
    pub const ALL: [IspcTarget; 3] = [Self::Sse2, Self::Avx2, Self::Avx512];

    /// The target ISPC's dispatcher picks on this CPU (see the `shortcut:` note at `forced`).
    pub fn best() -> IspcTarget {
        *Self::ALL.iter().rev().find(|t| t.supported()).unwrap()
    }

    /// Whether this CPU can run the target: avx2 needs AVX2, FMA, F16C, BMI1 and BMI2; avx512skx needs that and
    /// AVX-512 F, DQ, CD, BW and VL. ISPC's dispatcher (1.31.0 and 1.32.0 alike) tests other features: avx2 is AVX, OS
    /// YMM state and either AVX-VNNI or (F16C, RDRAND and AVX2), with no FMA or BMI check; avx512skx is AVX2, AVX-512
    /// F, DQ, CD, BW and VL and OS ZMM state, with no FMA, F16C or BMI check. See the `shortcut:` at `forced`.
    pub fn supported(self) -> bool {
        match self {
            Self::Sse2 => true,
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

/// Whether the BC7 and BC6H bytes recorded in the tests are out of reach on this CPU: they were recorded on an AMD CPU,
/// and an Intel CPU encodes other bytes on the same ISPC path (Note JC). True on an Intel CPU, and on any CPU with
/// `SDC_TEST_AS_INTEL=1`, to test that path.
#[cfg(feature = "test-hooks")]
pub fn intel_cpu() -> bool {
    cpu_is_intel() || std::env::var("SDC_TEST_AS_INTEL").as_deref() == Ok("1")
}

/// `intel_cpu()`, printing the reason when it is true: for a test that skips whole on an Intel CPU.
#[cfg(feature = "test-hooks")]
pub fn skip_byte_pins() -> bool {
    let intel = intel_cpu();
    if intel {
        eprintln!("skipped: the recorded BC7/BC6H hashes are an AMD CPU's; this Intel CPU encodes other bytes (Note JC)");
    }
    intel
}

/// The CPUID vendor string is "GenuineIntel" (leaf 0: EBX, EDX, ECX).
#[cfg(all(feature = "test-hooks", target_arch = "x86_64"))]
fn cpu_is_intel() -> bool {
    let r = std::arch::x86_64::__cpuid(0);
    let mut vendor = [0u8; 12];
    vendor[..4].copy_from_slice(&r.ebx.to_le_bytes());
    vendor[4..8].copy_from_slice(&r.edx.to_le_bytes());
    vendor[8..].copy_from_slice(&r.ecx.to_le_bytes());
    &vendor == b"GenuineIntel"
}

#[cfg(all(feature = "test-hooks", not(target_arch = "x86_64")))]
fn cpu_is_intel() -> bool {
    false
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
// shortcut: the rule assumes `best()` is the target ISPC's dispatcher runs, but `supported()` and the dispatcher test
// other features (see `supported()`), so they can differ in both directions. (a) A CPU with AVX2 and RDRAND (or
// AVX-VNNI) but no FMA, BMI1 or BMI2, and an AVX-512 CPU without them: `best()` is `Sse2` (or `Avx2`) while the
// dispatcher runs the AVX2 (or AVX-512) object, so `forced(Sse2)` returns -1 and an explicit SSE2 encode gets AVX2
// bytes labelled `sse2`. (b) A CPU with AVX2 but neither RDRAND nor AVX-VNNI (a hypervisor can hide the bit):
// `best()` is `Avx2` while the dispatcher ranks it as AVX and runs the SSE2 object; the bytes are valid, but the
// label (`isa`, `ispc_target()`) is wrong and the encode is slower. No such CPU is known; upgrade when one is
// reported, by giving `supported()` the dispatcher's conditions.
pub(crate) fn forced(target: IspcTarget) -> i32 {
    if target == IspcTarget::best() {
        -1
    } else {
        target as i32
    }
}

/// Test hook: how an encode reaches the shim. `Dispatcher` is ISPC's dispatcher whatever the CPU; `Direct(t)` is
/// `t`'s own object, even when `t` is `best()`. The shim has no direct AVX-512 entry.
#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShimRoute {
    Dispatcher,
    Direct(IspcTarget),
}

/// -1 for `Dispatcher`; `t as i32` for `Direct(t)`. Panics for `Direct(Avx512)` and for a target the CPU cannot run.
#[cfg(feature = "test-hooks")]
pub(crate) fn shim_number(route: ShimRoute) -> i32 {
    match route {
        ShimRoute::Dispatcher => -1,
        ShimRoute::Direct(t) => {
            assert!(
                t != IspcTarget::Avx512,
                "the shim has no direct AVX-512 entry"
            );
            assert!(t.supported(), "this CPU cannot run the {t:?} path");
            t as i32
        }
    }
}

#[cfg(all(test, feature = "test-hooks"))]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "no direct AVX-512 entry")]
    fn a_direct_avx512_route_panics() {
        shim_number(ShimRoute::Direct(IspcTarget::Avx512));
    }

    #[test]
    fn the_route_numbers_are_the_shims() {
        assert_eq!(shim_number(ShimRoute::Dispatcher), -1);
        assert_eq!(shim_number(ShimRoute::Direct(IspcTarget::Sse2)), 0);
    }

    #[test]
    fn the_cap_stops_at_avx2_unless_lifted() {
        for t in IspcTarget::ALL {
            assert_eq!(capped(t, true), t);
            assert_eq!(capped(t, false), t.min(IspcTarget::Avx2));
        }
        assert_eq!(capped(IspcTarget::Avx512, false), IspcTarget::Avx2);
    }
}
