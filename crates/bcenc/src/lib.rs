//! `bcenc`: rgbcx's block encoders in Rust. A port of rgbcx from bc7enc_rdo commit b943862 with Tessalith's patches
//! (MIT or public domain, `crates/encoders-sys/vendor/bc7enc_rdo/LICENSE`). The vendored C++ in `encoders-sys` is the
//! specification and the reference every test compares with. No dependencies, no mutable global state (two sets of
//! tables are built once in a `OnceLock`). `unsafe` has one use: calling a `#[target_feature(enable = "sse2")]`
//! function of a module compiled only where the target has SSE2 (`bc1/sels/sse2.rs`, `bc1/order_keys/sse2.rs` and
//! `mod sse2` in `bc4.rs`), each call with a `// SAFETY:` comment; Rust asks for the block even though the feature is
//! always on there. A target without SSE2 runs portable code instead, which the unit tests run on every target.
//! The engine encodes BC1, BC3, BC4 and BC5 with it (Note HY7). The BC1 colour search runs at rgbcx level 18 by
//! default; the `_with` functions take a [`Bc1Search`], whose `Fast` is a lower level (Note HY16).
//! The `_max` functions run a search the C++ does not have (Note LH); their reference is
//! `bc4::tests::search_max_reference`, a literal port of the bench measurement, and `tests/bc4.rs` pins bytes taken
//! from the bench code itself.

use std::num::NonZeroU8;

mod hits;

#[doc(hidden)]
pub mod bc4;

#[doc(hidden)]
pub mod bc1;

// The test inputs of `tests/support/gen.rs`, for every unit test (a `#[path]` in a nested module would resolve against
// another folder).
#[cfg(test)]
#[path = "../tests/support/gen.rs"]
pub(crate) mod gen;

#[cfg(test)]
mod coverage_tests;

/// How hard the BC1 colour search works: the rgbcx level of BC1, of one-bit alpha with no bit set and of BC3's colour
/// half. BC4, BC5, BC3's alpha half and the masked path of one-bit alpha do not depend on it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Bc1Search {
    /// rgbcx level 18, the bytes of [`bc1_encode_block`], [`bc1_encode_block_alpha`] and [`bc3_encode_block`].
    #[default]
    Default,
    /// rgbcx level [`BC1_FAST_LEVEL`]: the fastest level whose mean RGB PSNR still beats NVTT 3 `-highest` on both
    /// decoders (Note HY16).
    Fast,
}

/// The rgbcx level of [`Bc1Search::Fast`] (Note HY16).
#[doc(hidden)]
pub const BC1_FAST_LEVEL: u32 = 12;

// The fast level is one `bcenc` encodes at, and not the default's.
const _: () = {
    let mut found = false;
    let mut i = 0;
    while i < bc1::LEVELS.len() {
        found |= bc1::LEVELS[i] == BC1_FAST_LEVEL;
        i += 1;
    }
    assert!(found, "BC1_FAST_LEVEL is not in bc1::LEVELS");
    assert!(
        BC1_FAST_LEVEL != 18,
        "BC1_FAST_LEVEL is the default's level"
    );
};

/// `rgbcx::encode_bc4_hq(.., BC4_USE_ALL_MODES | BC4_SDC_SEARCH)` for one channel of a 4x4 block.
/// Panics if `channel >= 4`.
pub fn bc4_encode_block(px: &[[u8; 4]; 16], channel: usize, search_radius: u32) -> [u8; 8] {
    bc4::search(&channel_of(px, channel), search_radius, None)
}

/// `rgbcx::encode_bc5_hq`: channel 0 then channel 1, each as [`bc4_encode_block`].
pub fn bc5_encode_block(px: &[[u8; 4]; 16], search_radius: u32) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&bc4_encode_block(px, 0, search_radius));
    out[8..].copy_from_slice(&bc4_encode_block(px, 1, search_radius));
    out
}

/// The radius [`bc4::search_max`] starts from: the engine's `BC45_RADIUS_MAX`.
pub const MAX_SEARCH_RADIUS: u32 = 10;

/// [`bc4_encode_block`] with the Max search ([`bc4::search_max`], no guard). Panics if `channel >= 4`.
pub fn bc4_encode_block_max(px: &[[u8; 4]; 16], channel: usize) -> [u8; 8] {
    bc4::search_max(&channel_of(px, channel), None)
}

/// [`bc5_encode_block`] with the Max search: channel 0 then channel 1, each as [`bc4_encode_block_max`].
pub fn bc5_encode_block_max(px: &[[u8; 4]; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&bc4_encode_block_max(px, 0));
    out[8..].copy_from_slice(&bc4_encode_block_max(px, 1));
    out
}

/// `rgbcx::encode_bc1(18, .., allow_3color, false)` in `cBC1Dual` mode for one 4x4 block of RGBA texels (alpha is
/// ignored). `allow_3color` lets the encoder pick a three-colour block; `perceptual` weights the colour error by
/// Rec.709 luma (rgbcx's `set_bc1_perceptual`). [`bc1_encode_block_with`] at [`Bc1Search::Default`].
pub fn bc1_encode_block(px: &[[u8; 4]; 16], allow_3color: bool, perceptual: bool) -> [u8; 8] {
    bc1_encode_block_with(px, allow_3color, perceptual, Bc1Search::Default)
}

/// [`bc1_encode_block`] at the rgbcx level `search` stands for: `encode_bc1(18, ..)` for [`Bc1Search::Default`],
/// `encode_bc1(BC1_FAST_LEVEL, ..)` for [`Bc1Search::Fast`].
pub fn bc1_encode_block_with(
    px: &[[u8; 4]; 16],
    allow_3color: bool,
    perceptual: bool,
    search: Bc1Search,
) -> [u8; 8] {
    let w = bc1::weights(perceptual);
    match search {
        Bc1Search::Default => bc1::encode::encode::<18>(px, allow_3color, w),
        Bc1Search::Fast => bc1::encode::encode::<BC1_FAST_LEVEL>(px, allow_3color, w),
    }
}

/// [`bc1_encode_block`] at rgbcx level `level`, one of [`bc1::LEVELS`] (`rgbcx::encode_bc1(level, .., allow_3color,
/// false)`, Note HY15); the argument order of `encoders_sys::bc1_encode_block_level`. Panics on any other level. For
/// the tests of every level: rgbcx's one-bit alpha with no bit set is this with `allow_3color`, and rgbcx's BC3 colour
/// half this without.
#[doc(hidden)]
pub fn bc1_encode_block_level(
    px: &[[u8; 4]; 16],
    level: u32,
    allow_3color: bool,
    perceptual: bool,
) -> [u8; 8] {
    bc1::encode::encode_level(px, level, allow_3color, bc1::weights(perceptual))
}

/// The alpha half of `rgbcx::encode_bc3_hq`: the BC4 search with the alpha-test guard at `cut` (BC4_SDC_GUARD128).
/// A `cut` of 0 is 128, as `encode_bc4_hq` reads bits 8..15 of its flags (sdc patch "bc4 guard cut").
pub fn bc4_encode_alpha(alpha: &[u8; 16], search_radius: u32, cut: u8) -> [u8; 8] {
    bc4::search(alpha, search_radius, Some(read_cut(cut)))
}

/// A cut as rgbcx's flags carry it: 0 is 128.
fn read_cut(cut: u8) -> u8 {
    if cut != 0 {
        hits::hit!(hits::Hit::CutGiven);
        cut
    } else {
        hits::hit!(hits::Hit::CutDefault);
        128
    }
}

/// The texels of one channel of a block. Panics if `channel >= 4`.
fn channel_of(px: &[[u8; 4]; 16], channel: usize) -> [u8; 16] {
    assert!(channel < 4, "channel {channel} is not 0..4");
    std::array::from_fn(|i| px[i][channel])
}

/// [`bc4_encode_alpha`] with the Max search ([`bc4::search_max`] with the guard at `cut`); a `cut` of 0 is 128.
pub fn bc4_encode_alpha_max(alpha: &[u8; 16], cut: u8) -> [u8; 8] {
    bc4::search_max(alpha, Some(read_cut(cut)))
}

/// `rgbcx::encode_bc1_alpha(18, ..)` in `cBC1Dual` mode: BC1 with one-bit alpha, bit i of `transparent` marking
/// texel i transparent. No bit set is [`bc1_encode_block`]`(px, true, perceptual)`; every bit set is the
/// all-transparent block; otherwise a 3-colour block fitted to the opaque texels, masked texels on selector 3. The
/// RGB of a masked texel and every alpha value are never read. `perceptual` as in [`bc1_encode_block`].
/// [`bc1_encode_block_alpha_with`] at [`Bc1Search::Default`].
pub fn bc1_encode_block_alpha(px: &[[u8; 4]; 16], transparent: u16, perceptual: bool) -> [u8; 8] {
    bc1_encode_block_alpha_with(px, transparent, perceptual, Bc1Search::Default)
}

/// [`bc1_encode_block_alpha`] at the rgbcx level `search` stands for (`encode_bc1_alpha(level, ..)`); only a
/// `transparent` of 0, the plain encode, depends on it.
pub fn bc1_encode_block_alpha_with(
    px: &[[u8; 4]; 16],
    transparent: u16,
    perceptual: bool,
    search: Bc1Search,
) -> [u8; 8] {
    let w = bc1::weights(perceptual);
    match search {
        Bc1Search::Default => bc1::alpha::encode_alpha::<18>(px, transparent, w),
        Bc1Search::Fast => bc1::alpha::encode_alpha::<BC1_FAST_LEVEL>(px, transparent, w),
    }
}

/// `rgbcx::encode_bc3_hq(18, .., search_radius, BC4_USE_ALL_MODES | BC4_SDC_SEARCH | BC4_SDC_GUARD128 | cut << 8)`
/// with the shim's weights: bytes 0..8 are [`bc4_encode_alpha`] of the alpha channel, bytes 8..16
/// [`bc1_encode_block`]`(px, false, perceptual)` (`encode_bc1(level, .., false, false)`; `perceptual` reaches only
/// this half). `cut` is the least alpha that passes the texture's alpha test; it cannot be 0, which rgbcx's flags
/// would read as 128 (Note HY7). [`bc3_encode_block_with`] at [`Bc1Search::Default`].
pub fn bc3_encode_block(
    px: &[[u8; 4]; 16],
    search_radius: u32,
    cut: NonZeroU8,
    perceptual: bool,
) -> [u8; 16] {
    bc3_encode_block_with(px, search_radius, cut, perceptual, Bc1Search::Default)
}

/// [`bc3_encode_block`] with its colour half at the rgbcx level `search` stands for (`encode_bc3_hq(level, ..)`); the
/// alpha half does not depend on it.
pub fn bc3_encode_block_with(
    px: &[[u8; 4]; 16],
    search_radius: u32,
    cut: NonZeroU8,
    perceptual: bool,
    search: Bc1Search,
) -> [u8; 16] {
    bc3_raw_cut_with(px, search_radius, cut.get(), perceptual, search)
}

/// [`bc3_encode_block_with`] with the Max search on the alpha half: bytes 0..8 are [`bc4_encode_alpha_max`] of the
/// alpha channel, bytes 8..16 `bc1_encode_block_with(px, false, perceptual, search)`.
pub fn bc3_encode_block_max_with(
    px: &[[u8; 4]; 16],
    cut: NonZeroU8,
    perceptual: bool,
    search: Bc1Search,
) -> [u8; 16] {
    let alpha = channel_of(px, 3);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&bc4_encode_alpha_max(&alpha, cut.get()));
    out[8..].copy_from_slice(&bc1_encode_block_with(px, false, perceptual, search));
    out
}

/// [`bc3_encode_block`] with the cut as rgbcx's flags carry it, where 0 is 128 ([`bc4_encode_alpha`]). For the tests,
/// which compare every value of the cut with rgbcx.
#[doc(hidden)]
pub fn bc3_encode_block_raw_cut(
    px: &[[u8; 4]; 16],
    search_radius: u32,
    cut: u8,
    perceptual: bool,
) -> [u8; 16] {
    bc3_raw_cut_with(px, search_radius, cut, perceptual, Bc1Search::Default)
}

/// [`bc3_encode_block_with`] with the cut as rgbcx's flags carry it.
fn bc3_raw_cut_with(
    px: &[[u8; 4]; 16],
    search_radius: u32,
    cut: u8,
    perceptual: bool,
    search: Bc1Search,
) -> [u8; 16] {
    let alpha = channel_of(px, 3);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&bc4_encode_alpha(&alpha, search_radius, cut));
    out[8..].copy_from_slice(&bc1_encode_block_with(px, false, perceptual, search));
    out
}
