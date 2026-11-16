//! The code paths `bcenc` can encode on, and the pick: the widest path the CPU has, read per public call from std's
//! cached CPU detection. Every path returns the same bytes. The hidden `_on` entries of `lib.rs` take a path, so the
//! tests and the speed example can run each one the machine has.

/// A code path: `Base` is the SSE2 code (portable code on a target without SSE2), `Avx2` the same encode for CPUs with
/// AVX2, `Avx512` the same again for CPUs with the AVX-512 set (`avx2`, `avx512f`, `avx512bw`, `avx512vl`,
/// `avx512dq`). [`Path::ALL`] lists them narrowest first.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    Base,
    Avx2,
    Avx512,
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
    pub avx512f: bool,
    pub avx512bw: bool,
    pub avx512vl: bool,
    pub avx512dq: bool,
}

impl Detected {
    /// std's detection (cached after the first call); all `false` on a build without the wide paths.
    pub(crate) fn now() -> Detected {
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            use std::arch::is_x86_feature_detected as has;
            Detected {
                avx2: has!("avx2"),
                avx512f: has!("avx512f"),
                avx512bw: has!("avx512bw"),
                avx512vl: has!("avx512vl"),
                avx512dq: has!("avx512dq"),
            }
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        {
            Detected::default()
        }
    }

    /// The first feature of the AVX-512 set the CPU lacks, as [`Path::missing`] says it; `None` when it has them all.
    fn lacks_avx512(self) -> Option<&'static str> {
        [
            (self.avx2, "CPU lacks avx2"),
            (self.avx512f, "CPU lacks avx512f"),
            (self.avx512bw, "CPU lacks avx512bw"),
            (self.avx512vl, "CPU lacks avx512vl"),
            (self.avx512dq, "CPU lacks avx512dq"),
        ]
        .into_iter()
        .find_map(|(has, why)| (!has).then_some(why))
    }
}

impl Path {
    /// Every path, narrowest first; a path is available when it is at or below [`Path::best`] in this order.
    pub const ALL: [Path; 3] = [Path::Base, Path::Avx2, Path::Avx512];

    /// The name the speed example's `--path` and `SDC_BCENC_PATHS` use.
    pub fn name(self) -> &'static str {
        match self {
            Path::Base => "base",
            Path::Avx2 => "avx2",
            Path::Avx512 => "avx512",
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
            Path::Avx2 | Path::Avx512 if !BUILT => Some("not built for x86_64 with SSE2"),
            Path::Avx2 => (!Detected::now().avx2).then_some("CPU lacks avx2"),
            Path::Avx512 => Detected::now().lacks_avx512(),
        }
    }

    /// The path for these features, when the build has the wide paths: `Avx512` when the CPU has the whole AVX-512
    /// set, else `Avx2` when it has AVX2.
    pub(crate) fn pick(d: Detected, built: bool) -> Path {
        if !built {
            Path::Base
        } else if d.lacks_avx512().is_none() {
            Path::Avx512
        } else if d.avx2 {
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
/// frames are in `bc1/frames.rs` and `bc4.rs`). Each instance has a concrete `Isa`, so a kernel choice on
/// [`Isa::avx2`] folds at compile time.
pub(crate) trait Isa: Copy {
    /// The path this code runs on.
    #[allow(dead_code)] // Nothing reads it yet: the run counters are keyed by frame (`Path`) and kernel (`Wide`).
    const PATH: Path;
    /// The token of the AVX2 kernels, when this path has AVX2 (the AVX2 path and the AVX-512 path).
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    fn avx2(self) -> Option<Avx2>;
    /// The token of the AVX-512 path, when this is it (BC4's `Batch::push` reaches the four-pair kernel through it).
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    fn avx512(self) -> Option<Avx512>;
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
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    fn avx512(self) -> Option<Avx512> {
        None
    }
}

/// Proof that std found AVX2 on this CPU: only [`Avx2::require`] and an [`Avx512`] token (whose set holds AVX2) build
/// one (the field is private), so code holding an `Avx2` may call a `#[target_feature(enable = "avx2")]` function.
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
    fn avx512(self) -> Option<Avx512> {
        None
    }
}

/// Proof that std found the AVX-512 set (`avx2`, `avx512f`, `avx512bw`, `avx512vl`, `avx512dq`) on this CPU: only
/// [`Avx512::require`] builds one (the field is private), so code holding an `Avx512` may call a function with
/// `#[target_feature(enable = "avx2,avx512f,avx512bw,avx512vl,avx512dq")]`, and through [`Isa::avx2`] one with the
/// AVX2 set.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[derive(Clone, Copy)]
pub(crate) struct Avx512(());

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
impl Avx512 {
    /// The token. Panics unless std detects the whole set (cached after the first call).
    pub(crate) fn require() -> Avx512 {
        assert!(
            Detected::now().lacks_avx512().is_none(),
            "path avx512 is not available: {}",
            Path::Avx512.missing().unwrap_or("")
        );
        Avx512(())
    }
}

#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
impl Isa for Avx512 {
    const PATH: Path = Path::Avx512;
    fn avx2(self) -> Option<Avx2> {
        // The set holds AVX2, which `Avx512::require` found.
        Some(Avx2(()))
    }
    fn avx512(self) -> Option<Avx512> {
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

/// A wide kernel: each one's safe wrapper counts its runs in the unit tests, so a test proves the wide code ran.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wide {
    Sels4,
    Sels3,
    Sels3Masked,
    Keys4,
    Keys3,
    Pair2,
    Pair4,
}

#[cfg(test)]
impl Wide {
    pub(crate) const ALL: [Wide; 7] = [
        Wide::Sels4,
        Wide::Sels3,
        Wide::Sels3Masked,
        Wide::Keys4,
        Wide::Keys3,
        Wide::Pair2,
        Wide::Pair4,
    ];
}

// Each kernel's place in `ALL` is its discriminant, which the counters index by.
#[cfg(test)]
const _: () = {
    let mut i = 0;
    while i < Wide::ALL.len() {
        assert!(
            Wide::ALL[i] as usize == i,
            "Wide::ALL is not in declaration order"
        );
        i += 1;
    }
};

#[cfg(test)]
thread_local! {
    /// How often this thread ran each wide kernel, by place in [`Wide::ALL`].
    static KERNEL_RUNS: std::cell::Cell<[u64; Wide::ALL.len()]> =
        const { std::cell::Cell::new([0; Wide::ALL.len()]) };
}

/// Counts one run of wide kernel `k` on this thread (unit tests only; the kernel's safe wrapper calls it). The wide
/// kernels exist only under the x86 cfg, and so does this.
#[cfg(all(test, target_arch = "x86_64", target_feature = "sse2"))]
pub(crate) fn kernel_ran(k: Wide) {
    KERNEL_RUNS.with(|c| {
        let mut runs = c.get();
        runs[k as usize] += 1;
        c.set(runs);
    });
}

/// How often this thread ran each wide kernel, by place in [`Wide::ALL`].
#[cfg(test)]
pub(crate) fn kernel_runs() -> [u64; Wide::ALL.len()] {
    KERNEL_RUNS.with(|c| c.get())
}

/// The paths the unit tests run: `tests/support/paths.rs`'s `paths()`, with the same line and the same
/// `SDC_BCENC_PATHS` checks.
#[cfg(test)]
pub(crate) fn tested_paths() -> Vec<Path> {
    crate::test_paths::paths()
}

/// Whether the unit tests run the AVX2 path ([`tested_paths`], which panics when `SDC_BCENC_PATHS` names a path this
/// machine lacks); prints the skip, once per process, when they do not.
#[cfg(all(test, target_arch = "x86_64", target_feature = "sse2"))]
pub(crate) fn avx2_tested() -> bool {
    path_tested(Path::Avx2)
}

/// [`avx2_tested`] for the AVX-512 path.
#[cfg(all(test, target_arch = "x86_64", target_feature = "sse2"))]
pub(crate) fn avx512_tested() -> bool {
    path_tested(Path::Avx512)
}

#[cfg(all(test, target_arch = "x86_64", target_feature = "sse2"))]
fn path_tested(p: Path) -> bool {
    static SKIP_LINES: [std::sync::Once; Path::ALL.len()] =
        [const { std::sync::Once::new() }; Path::ALL.len()];
    let tested = tested_paths().contains(&p);
    if !tested {
        SKIP_LINES[p as usize].call_once(|| {
            eprintln!(
                "{} path skipped: {}",
                p.name(),
                p.missing().unwrap_or("not named")
            )
        });
    }
    tested
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All 64 combinations of the five flags and `built`: `Avx512` exactly when all six are true, `Avx2` when `avx2`
    /// and `built` are but not all, else `Base`.
    #[test]
    fn the_pick_follows_detection() {
        for bits in 0u32..64 {
            let on = |i: u32| bits & (1 << i) != 0;
            let d = Detected {
                avx2: on(0),
                avx512f: on(1),
                avx512bw: on(2),
                avx512vl: on(3),
                avx512dq: on(4),
            };
            let built = on(5);
            let want = if bits == 63 {
                Path::Avx512
            } else if d.avx2 && built {
                Path::Avx2
            } else {
                Path::Base
            };
            assert_eq!(Path::pick(d, built), want, "{d:?}, built {built}");
        }
    }

    #[test]
    fn the_best_path_is_the_detected_one() {
        assert_eq!(Path::best(), Path::pick(Detected::now(), BUILT));
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            use std::arch::is_x86_feature_detected as has;
            let avx512 = has!("avx2")
                && has!("avx512f")
                && has!("avx512bw")
                && has!("avx512vl")
                && has!("avx512dq");
            assert_eq!(Path::best() == Path::Avx512, avx512);
            if !avx512 {
                assert_eq!(Path::best() == Path::Avx2, has!("avx2"));
            }
        }
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

    /// On a CPU without AVX2 the token cannot be had: `Avx2::require` panics and says why. Runs only on such a machine.
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[test]
    fn the_avx2_token_needs_avx2() {
        if Path::Avx2.available() {
            eprintln!("the_avx2_token_needs_avx2 skipped: this CPU has avx2");
            return;
        }
        let Err(payload) = std::panic::catch_unwind(Avx2::require) else {
            panic!("Avx2::require gave a token on a CPU without avx2");
        };
        let msg = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .unwrap_or_default();
        assert_eq!(msg, "path avx2 is not available: CPU lacks avx2");
    }

    /// On a CPU without the AVX-512 set the token cannot be had: `Avx512::require` panics and names the first feature
    /// missing. Runs only on such a machine.
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[test]
    fn the_avx512_token_needs_the_avx512_set() {
        let Some(why) = Path::Avx512.missing() else {
            eprintln!(
                "the_avx512_token_needs_the_avx512_set skipped: this CPU has the AVX-512 set"
            );
            return;
        };
        let Err(payload) = std::panic::catch_unwind(Avx512::require) else {
            panic!("Avx512::require gave a token on a CPU without the AVX-512 set");
        };
        let msg = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .unwrap_or_default();
        assert_eq!(msg, format!("path avx512 is not available: {why}"));
    }
}
