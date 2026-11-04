//! Safe wrappers over the vendored BC encoders (bc7e, rgbcx, ISPC TC).
mod bc6h;
mod bc7;
pub mod bc7_fixup;
mod ffi;
mod rgbcx;

pub use bc6h::*;
pub use bc7::{
    bc7e_compress_blocks, bc7e_compress_blocks_on, bc7e_compress_blocks_tuned, Bc7Level, Bc7Params,
    Bc7Target, Bc7Tune,
};
pub use rgbcx::*;

use std::sync::Once;

/// The BC1 decoder an error is measured against; values equal `rgbcx::bc1_approx_mode`. Encoding always targets both
/// (`cBC1Dual = 4`, set by [`init`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum Bc1Model {
    Nvidia = 1,
    Amd = 2,
}

/// rgbcx's BC1 mode: score NVIDIA and AMD decoder error together (README-VENDOR.md, patch "rgbcx BC1 dual").
const BC1_DUAL: i32 = 4;

static INIT: Once = Once::new();

/// Initialise rgbcx and bc7e once per process. Safe to call from any thread, any number of times.
pub fn init() {
    // SAFETY: `Once` runs `sdc_init` once, as it requires (rgbcx::init writes global state).
    INIT.call_once(|| unsafe { ffi::sdc_init(BC1_DUAL) });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Versions {
    pub bc7enc_rdo: &'static str,
    pub ispctc: &'static str,
    pub ispc: &'static str,
    pub bc7e_patch: Option<&'static str>,
    /// The sdc patches in the vendored `rgbcx.h`/`rgbcx.cpp` (BC4 palette rounded to nearest, M6 Track D; BC4 block search; BC1 dual; BC3 alpha guard cutoff in `mode_flag` bits 8..15; BC1 1-bit alpha; BC1 perceptual weights); README-VENDOR.md.
    pub rgbcx_patch: Option<&'static str>,
}

pub fn versions() -> Versions {
    Versions {
        bc7enc_rdo: "b943862",
        rgbcx_patch: Some(
            "bc4-round-to-nearest+bc4-search+bc1-dual+bc4-guard-cut+bc1-alpha+bc1-perceptual",
        ),
        ispctc: "79ddbc9",
        ispc: "1.31.0",
        bc7e_patch: Some("0001-exact-opaque-alpha"),
    }
}

/// Re-arrange a row-major RGBA8 image into 4x4 blocks (row-major blocks, row-major texels inside a block).
/// Texels past the right or bottom edge replicate the nearest edge texel.
pub fn gather_blocks(px: &[[u8; 4]], width: u32, height: u32) -> Vec<[u8; 4]> {
    let (w, h) = (width as usize, height as usize);
    assert!(w > 0 && h > 0, "empty image");
    assert_eq!(px.len(), w * h, "pixel count does not match width*height");
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    let mut out = Vec::with_capacity(bw * bh * 16);
    for by in 0..bh {
        for bx in 0..bw {
            for y in 0..4 {
                let sy = (by * 4 + y).min(h - 1);
                for x in 0..4 {
                    let sx = (bx * 4 + x).min(w - 1);
                    out.push(px[sy * w + sx]);
                }
            }
        }
    }
    out
}
