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
    /// Force mode-6 p-bits to 1 on all-255 blocks (needs the patched bc7e objects, Task 14).
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

/// An override of a level's bc7e parameters (`encoder.bc7_weights`, DECISIONS Note CP). `None` keeps the level's
/// value. The shim's other tuning slots (the BC7 tuning study's) are always sent unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bc7Tune {
    /// Linear per-channel error weights (used with `perceptual = false`).
    pub weights: Option<[u32; 4]>,
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
    if n == 0 {
        return;
    }
    init();
    // -1 leaves a slot at the level's value (sdc_shim.cpp).
    let mut t = [-1i32; 14];
    if let Some(w) = tune.weights {
        for (slot, x) in t[4..8].iter_mut().zip(w) {
            *slot = x as i32;
        }
    }
    let src: Vec<u32> = pixels.iter().map(|c| u32::from_le_bytes(*c)).collect();
    let mut dst = vec![0u64; n * 2];
    // SAFETY: as in `bc7e_compress_blocks`; `t` is 14 ints, which is what the shim reads; `target` was checked above.
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
