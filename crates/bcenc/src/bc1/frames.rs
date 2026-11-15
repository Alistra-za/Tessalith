//! The frame entries of the BC1 encode: one dispatch per call, on the path the caller names. `Base` runs the encode as
//! plain code; `Avx2` runs it inside a `#[target_feature(enable = "avx2")]` function, where every hot function
//! (`#[inline(always)]`) is compiled again with AVX2 and calls the kernels' AVX2 stamps (Note HY21). The bytes are the
//! same on every path. Each `_on` entry of `lib.rs` has asserted that its path is available before it calls these.

use super::alpha::encode_alpha;
use super::encode::encode;
use super::LEVELS;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
use crate::path::Avx2;
use crate::path::{Base, Path};

/// [`encode`] at a level given at run time, one of [`LEVELS`], on `path`. Panics on any other level.
pub(crate) fn encode_level_on(
    path: Path,
    px: &[[u8; 4]; 16],
    level: u32,
    allow_3color: bool,
    weights: [i32; 3],
) -> [u8; 8] {
    // A const generic needs a literal per level; `every_level_is_rgbcx` fails when `LEVELS` gains one without an arm.
    match level {
        10 => encode_on::<10>(path, px, allow_3color, weights),
        12 => encode_on::<12>(path, px, allow_3color, weights),
        13 => encode_on::<13>(path, px, allow_3color, weights),
        15 => encode_on::<15>(path, px, allow_3color, weights),
        18 => encode_on::<18>(path, px, allow_3color, weights),
        _ => panic!("BC1 level {level} is not one of {LEVELS:?}"),
    }
}

/// [`encode`] on `path`.
pub(crate) fn encode_on<const LEVEL: u32>(
    path: Path,
    px: &[[u8; 4]; 16],
    allow_3color: bool,
    weights: [i32; 3],
) -> [u8; 8] {
    match path {
        Path::Base => {
            #[cfg(test)]
            crate::path::frame_ran(Path::Base);
            encode::<LEVEL>(Base, px, allow_3color, weights)
        }
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        Path::Avx2 => {
            let t = Avx2::require();
            // SAFETY: `t` exists, so `Avx2::require` found AVX2.
            unsafe { encode_avx2::<LEVEL>(t, px, allow_3color, weights) }
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        Path::Avx2 => {
            unreachable!("path avx2 is not built here; the _on entry asserted availability")
        }
    }
}

/// [`encode_alpha`] on `path`.
pub(crate) fn encode_alpha_on<const LEVEL: u32>(
    path: Path,
    px: &[[u8; 4]; 16],
    transparent: u16,
    weights: [i32; 3],
) -> [u8; 8] {
    match path {
        Path::Base => {
            #[cfg(test)]
            crate::path::frame_ran(Path::Base);
            encode_alpha::<LEVEL>(Base, px, transparent, weights)
        }
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        Path::Avx2 => {
            let t = Avx2::require();
            // SAFETY: `t` exists, so `Avx2::require` found AVX2.
            unsafe { encode_alpha_avx2::<LEVEL>(t, px, transparent, weights) }
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
        Path::Avx2 => {
            unreachable!("path avx2 is not built here; the _on entry asserted availability")
        }
    }
}

/// The AVX2 frame of [`encode`]: the whole encode compiled with AVX2.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[target_feature(enable = "avx2")]
fn encode_avx2<const LEVEL: u32>(
    t: Avx2,
    px: &[[u8; 4]; 16],
    allow_3color: bool,
    weights: [i32; 3],
) -> [u8; 8] {
    #[cfg(test)]
    crate::path::frame_ran(Path::Avx2);
    encode::<LEVEL>(t, px, allow_3color, weights)
}

/// The AVX2 frame of [`encode_alpha`].
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
#[target_feature(enable = "avx2")]
fn encode_alpha_avx2<const LEVEL: u32>(
    t: Avx2,
    px: &[[u8; 4]; 16],
    transparent: u16,
    weights: [i32; 3],
) -> [u8; 8] {
    #[cfg(test)]
    crate::path::frame_ran(Path::Avx2);
    encode_alpha::<LEVEL>(t, px, transparent, weights)
}
