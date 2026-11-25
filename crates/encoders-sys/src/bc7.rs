use crate::target::{forced, ispc_target, IspcTarget};
use crate::{ffi, init};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bc7Level {
    Ultrafast = 0,
    Veryfast = 1,
    Fast = 2,
    Basic = 3,
    Slow = 4,
    Veryslow = 5,
    Slowest = 6,
    /// sdc patch 0002: veryslow's search plus a wider, fully refined partition ranking, uber 4, two least-squares
    /// passes and a quantised endpoint search (docs/research/bc7e-tuning.md). Needs the patch-0002 objects.
    Max = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bc7Params {
    pub level: Bc7Level,
    pub perceptual: bool,
    /// The channel guard's A: bit 3 of bc7e's exact-constants mask (patch 0005, Note LR, LR17). On a block whose alpha
    /// is 255 throughout, mode 6 takes p-bits 1, so alpha decodes exactly 255 (a rotated mode 4/5 winner needs
    /// [`crate::bc7_fixup::force_opaque_rotated`], LR6); in an alpha block, a subset whose alpha is 0 or 255 on every
    /// texel decodes it exactly wherever the kernel finds such a solution, above any guarded R, G or B
    /// ([`Bc7Tune::exact_constants`]).
    pub exact_opaque: bool,
}

/// Compress `pixels.len() / 16` blocks. `pixels` holds 16 texels per block (see `gather_blocks`).
pub fn bc7e_compress_blocks(pixels: &[[u8; 4]], out: &mut [u8], p: Bc7Params) {
    bc7e_compress_blocks_on(pixels, out, p, ispc_target());
}

/// `bc7e_compress_blocks` on one ISPC code path instead of the CPU dispatch. Every plain encode goes through it with `ispc_target()`.
/// The best supported target goes through the dispatcher, which already runs that path; forcing it would
/// re-run its table init under concurrent encodes. Panics when the CPU cannot run `target`.
pub fn bc7e_compress_blocks_on(
    pixels: &[[u8; 4]],
    out: &mut [u8],
    p: Bc7Params,
    target: IspcTarget,
) {
    assert!(
        target.supported(),
        "this CPU cannot run the {target:?} bc7e path"
    );
    compress(pixels, out, p, forced(target));
}

fn compress(pixels: &[[u8; 4]], out: &mut [u8], p: Bc7Params, target: i32) {
    assert_eq!(pixels.len() % 16, 0, "pixels must hold whole 4x4 blocks");
    let n = pixels.len() / 16;
    assert_eq!(out.len(), n * 16, "out must hold 16 bytes per block");
    if n == 0 {
        return;
    }
    init();
    // Copy into u32/u64 buffers so the ISPC code sees naturally aligned data.
    let src: Vec<u32> = pixels.iter().map(|c| u32::from_le_bytes(*c)).collect();
    let mut dst = vec![0u64; n * 2];
    // SAFETY: `src` holds 16 * n texels and `dst` 2 * n u64 (16 bytes per block); `level` comes
    // from the closed `Bc7Level` enum (0..=7), which the shim indexes without a range check; a forced
    // `target` was checked against the CPU by `bc7e_compress_blocks_on`.
    unsafe {
        ffi::sdc_bc7e_compress(
            n as u32,
            dst.as_mut_ptr(),
            src.as_ptr(),
            p.level as i32,
            p.perceptual as i32,
            p.exact_opaque as i32,
            target,
        )
    };
    for (i, w) in dst.iter().enumerate() {
        out[i * 8..i * 8 + 8].copy_from_slice(&w.to_le_bytes());
    }
}

/// The default flat tolerance of bc7e patch 0005 (Q2t, T_d), in the kernel's 0..1 domain: every level's
/// `m_flat_tolerance` (DECISIONS Note LR).
pub const BC7_FLAT_TOLERANCE_DEFAULT: f32 = 0.5 / 255.0;
/// The flat tolerance for data maps guarded on G (T_data, Note LR).
pub const BC7_FLAT_TOLERANCE_DATA: f32 = 2.0 / 255.0;

/// The flat tolerance a call sets (patch 0005 Q2t): `fixDegenerateEndpoints` spreads a channel only when its float
/// endpoints differ by more than it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bc7FlatTolerance {
    /// [`BC7_FLAT_TOLERANCE_DEFAULT`], the levels' own value.
    Default,
    /// [`BC7_FLAT_TOLERANCE_DATA`].
    Data,
}

impl Bc7FlatTolerance {
    pub fn value(self) -> f32 {
        match self {
            Bc7FlatTolerance::Default => BC7_FLAT_TOLERANCE_DEFAULT,
            Bc7FlatTolerance::Data => BC7_FLAT_TOLERANCE_DATA,
        }
    }
}

/// The flat tolerance bc7e's level holds (the shim's copy of the level's params), for the test that pins
/// [`BC7_FLAT_TOLERANCE_DEFAULT`] against the kernel. Test builds only (feature `test-hooks`).
#[cfg(feature = "test-hooks")]
pub fn bc7e_level_flat_tolerance(p: Bc7Params) -> f32 {
    init();
    // SAFETY: `level` comes from the closed `Bc7Level` enum (0..=7), which the shim indexes without a range check.
    unsafe {
        ffi::sdc_bc7e_level_flat_tolerance(
            p.level as i32,
            p.perceptual as i32,
            p.exact_opaque as i32,
        )
    }
}

/// An override of a level's bc7e parameters (`encoder.bc7_weights`, DECISIONS Note CP). `None` keeps the level's
/// value. The shim's other tuning slots (the BC7 tuning study's) are always sent unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bc7Tune {
    /// Linear per-channel error weights (used with `perceptual = false`), as passed. Since patch 0005 (L16) bc7e ranks
    /// every linear call as the same weights x16 would, so pass x1-form weights (`[1, 1, 1, N]` for Alpha xN; weights
    /// x16 would rank at x256). Each weight must be 1 to 65535, else the call panics: the shim reads a weight as `i32`
    /// (2^31 and above read as "unset"), the kernel's weighted square is a `uint32` (safe up to 66,051) and a weight of
    /// 0 makes the estimators divide by a zero sum. Note CP, Note LR (LR25).
    pub weights: Option<[u32; 4]>,
    /// The flat tolerance of patch 0005 (Q2t); `None` keeps the level's value, [`BC7_FLAT_TOLERANCE_DEFAULT`]
    /// (Note LR).
    pub flat_tolerance: Option<Bc7FlatTolerance>,
    /// Exact constants (patch 0005 XC): bit 0 R, bit 1 G, bit 2 B. A subset whose masked channel is 0 on every texel,
    /// or 255 on every texel, decodes that channel exactly wherever the kernel finds such a solution. `None` keeps the
    /// level's value, 0, which writes the bytes the plain call writes (Note LR, LR12). Where masked channels cannot all
    /// be kept, the higher of G, B and R is kept first, across every subset (LR14). Alpha is the mask's bit 3, set from
    /// [`Bc7Params::exact_opaque`] and never from here, so an alpha bit is refused; with both set, alpha is kept above
    /// R, G and B (LR17).
    pub exact_constants: Option<u8>,
}

/// `bc7e_compress_blocks` with `tune` applied over the level's parameters.
pub fn bc7e_compress_blocks_tuned(pixels: &[[u8; 4]], out: &mut [u8], p: Bc7Params, tune: Bc7Tune) {
    bc7e_compress_blocks_tuned_on(pixels, out, p, tune, ispc_target());
}

/// `bc7e_compress_blocks_tuned` on one ISPC code path (see `bc7e_compress_blocks_on`). Panics when the CPU cannot
/// run `target`.
pub fn bc7e_compress_blocks_tuned_on(
    pixels: &[[u8; 4]],
    out: &mut [u8],
    p: Bc7Params,
    tune: Bc7Tune,
    target: IspcTarget,
) {
    assert!(
        target.supported(),
        "this CPU cannot run the {target:?} bc7e path"
    );
    assert_eq!(pixels.len() % 16, 0, "pixels must hold whole 4x4 blocks");
    let n = pixels.len() / 16;
    assert_eq!(out.len(), n * 16, "out must hold 16 bytes per block");
    // The tune is checked before the empty-input return, so a refused value is refused whatever the input size.
    // -1 leaves a slot at the level's value (sdc_shim.cpp).
    let mut t = [-1i32; 16];
    if let Some(w) = tune.weights {
        assert!(
            w.iter().all(|&x| (1..=65535).contains(&x)),
            "Bc7Tune::weights must be 1..=65535, got {w:?} (Note LR)"
        );
        for (slot, x) in t[4..8].iter_mut().zip(w) {
            *slot = x as i32;
        }
    }
    if let Some(v) = tune.flat_tolerance {
        // A finite tolerance at or above 0 has a non-negative bit pattern, so it never reads as "unset".
        t[14] = v.value().to_bits() as i32;
    }
    if let Some(m) = tune.exact_constants {
        assert!(
            m & !0b111 == 0,
            "Bc7Tune::exact_constants is R, G and B only (alpha keeps exact opaque, BC7-O14)"
        );
        t[15] = m as i32;
    }
    if n == 0 {
        return;
    }
    init();
    let src: Vec<u32> = pixels.iter().map(|c| u32::from_le_bytes(*c)).collect();
    let mut dst = vec![0u64; n * 2];
    // SAFETY: as in `bc7e_compress_blocks`; `t` is 16 ints, which is what the shim reads; `target` was checked above.
    unsafe {
        ffi::sdc_bc7e_compress_tuned(
            n as u32,
            dst.as_mut_ptr(),
            src.as_ptr(),
            p.level as i32,
            p.perceptual as i32,
            p.exact_opaque as i32,
            t.as_ptr(),
            forced(target),
        )
    };
    for (i, w) in dst.iter().enumerate() {
        out[i * 8..i * 8 + 8].copy_from_slice(&w.to_le_bytes());
    }
}
