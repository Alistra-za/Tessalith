//! The code paths `bcenc` can encode on, and the pick: the widest path the CPU has, read per public call from std's
//! cached CPU detection. Every path returns the same bytes. The hidden `_on` entries of `lib.rs` take a path, so the
//! tests and the speed example can run each one the machine has.

/// A code path: `Base` is the SSE2 code (portable code on a target without SSE2), `Avx2` the same encode for CPUs with
/// AVX2. [`Path::ALL`] lists them narrowest first.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Base,
    Avx2,
}

// Each path's place in `ALL` is its discriminant, which `Path::available` compares.
const _: () = {
    let mut i = 0;
    while i < Path::ALL.len() {
        assert!(
            Path::ALL[i] as usize == i,
            "Path::ALL is not in declaration order"
        );
        i += 1;
    }
};

/// Whether this build has the wide paths: an x86_64 target with SSE2 (the cfg of the SSE2 kernels).
const BUILT: bool = cfg!(all(target_arch = "x86_64", target_feature = "sse2"));

/// The CPU features the paths need, as std detects them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Detected {
    pub avx2: bool,
}

impl Detected {
    /// std's detection (cached after the first call); all `false` on a build without the wide paths.
    pub(crate) fn now() -> Detected {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            Detected {
                avx2: std::arch::is_x86_feature_detected!("avx2"),
            }
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            Detected::default()
        }
    }
}

impl Path {
    /// Every path, narrowest first; a path is available when it is at or below [`Path::best`] in this order.
    pub const ALL: [Path; 2] = [Path::Base, Path::Avx2];

    /// The name the speed example's `--path` and `SDC_BCENC_PATHS` use.
    pub fn name(self) -> &'static str {
        match self {
            Path::Base => "base",
            Path::Avx2 => "avx2",
        }
    }

    /// The widest path this build and CPU have: the one every public function runs.
    pub fn best() -> Path {
        Path::pick(Detected::now(), BUILT)
    }

    /// Whether this machine can run `self`. `Base` always can.
    pub fn available(self) -> bool {
        // `ALL` is in declaration order, so the discriminant is the place in it.
        self == Path::Base || self as u8 <= Path::best() as u8
    }

    /// Why this machine cannot run `self`, or `None` when it can.
    pub fn missing(self) -> Option<&'static str> {
        match self {
            Path::Base => None,
            Path::Avx2 if !BUILT => Some("not built for x86_64 with SSE2"),
            Path::Avx2 if !Detected::now().avx2 => Some("CPU lacks avx2"),
            Path::Avx2 => None,
        }
    }

    /// The path for these features: `Avx2` when the build has the wide paths and the CPU has AVX2.
    pub(crate) fn pick(d: Detected, built: bool) -> Path {
        if built && d.avx2 {
            Path::Avx2
        } else {
            Path::Base
        }
    }

    /// The `_on` entries' check: panics with the reason when this machine cannot run `self`.
    pub(crate) fn assert_available(self) {
        assert!(
            self.available(),
            "path {} is not available: {}",
            self.name(),
            self.missing().unwrap_or("")
        );
    }
}

/// The code a hot function is compiled for, passed first to every function between a frame entry and the kernels (the
/// frames are in `bc1/frames.rs`). Each instance has a concrete `Isa`, so a kernel choice on [`Isa::avx2`] folds at
/// compile time.
pub(crate) trait Isa: Copy {
    /// The path this code runs on.
    #[allow(dead_code)] // First read by a wide kernel's run counter, with the first AVX2 kernel.
    const PATH: Path;
    /// The token of the AVX2 path, when this is it.
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    fn avx2(self) -> Option<Avx2>;
}

/// The base path's code: the SSE2 kernels (the portable ones on a target without SSE2).
#[derive(Clone, Copy)]
pub(crate) struct Base;

impl Isa for Base {
    const PATH: Path = Path::Base;
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    fn avx2(self) -> Option<Avx2> {
        None
    }
}

/// Proof that std found AVX2 on this CPU: only [`Avx2::require`] builds one (the field is private), so code holding an
/// `Avx2` may call a `#[target_feature(enable = "avx2")]` function.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[derive(Clone, Copy)]
pub(crate) struct Avx2(());

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
impl Avx2 {
    /// The token. Panics unless std detects AVX2 (cached after the first call).
    pub(crate) fn require() -> Avx2 {
        assert!(
            Detected::now().avx2,
            "path avx2 is not available: {}",
            Path::Avx2.missing().unwrap_or("")
        );
        Avx2(())
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
impl Isa for Avx2 {
    const PATH: Path = Path::Avx2;
    fn avx2(self) -> Option<Avx2> {
        Some(self)
    }
}

#[cfg(test)]
thread_local! {
    /// How often this thread ran each path's frame, by place in [`Path::ALL`].
    static FRAME_RUNS: std::cell::Cell<[u64; Path::ALL.len()]> =
        const { std::cell::Cell::new([0; Path::ALL.len()]) };
}

/// Counts one run of path `p`'s frame on this thread (unit tests only; each frame entry calls it first).
#[cfg(test)]
pub(crate) fn frame_ran(p: Path) {
    FRAME_RUNS.with(|c| {
        let mut runs = c.get();
        runs[p as usize] += 1;
        c.set(runs);
    });
}

/// How often this thread ran each path's frame, by place in [`Path::ALL`].
#[cfg(test)]
pub(crate) fn frame_runs() -> [u64; Path::ALL.len()] {
    FRAME_RUNS.with(|c| c.get())
}

/// The paths the unit tests run: `tests/support/paths.rs`'s `paths()`, with the same line and the same
/// `SDC_BCENC_PATHS` checks.
#[cfg(test)]
pub(crate) fn tested_paths() -> Vec<Path> {
    crate::test_paths::paths()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pick_follows_detection() {
        for avx2 in [false, true] {
            for built in [false, true] {
                let want = if avx2 && built {
                    Path::Avx2
                } else {
                    Path::Base
                };
                assert_eq!(
                    Path::pick(Detected { avx2 }, built),
                    want,
                    "avx2 {avx2}, built {built}"
                );
            }
        }
    }

    #[test]
    fn the_best_path_is_the_detected_one() {
        assert_eq!(Path::best(), Path::pick(Detected::now(), BUILT));
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        assert_eq!(
            Path::best() == Path::Avx2,
            std::arch::is_x86_feature_detected!("avx2")
        );
    }

    #[test]
    fn every_unavailable_path_says_why() {
        for p in Path::ALL {
            assert_eq!(
                p.available(),
                p.missing().is_none(),
                "{p:?}: {:?}",
                p.missing()
            );
        }
        assert!(Path::Base.available());
    }
}
