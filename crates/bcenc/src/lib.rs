//! `bcenc`: rgbcx's block encoders in Rust. A port of rgbcx from bc7enc_rdo commit b943862 with Tessalith's patches
//! (MIT or public domain, `crates/encoders-sys/vendor/bc7enc_rdo/LICENSE`). The vendored C++ in `encoders-sys` is the
//! specification and the reference every test compares with. No dependencies, no `unsafe`, no global state.
//! The engine still encodes through rgbcx (Note HY1).

mod hits;

pub mod bc4;

#[doc(hidden)]
pub mod bc1;

#[cfg(test)]
mod coverage_tests;

/// `rgbcx::encode_bc4_hq(.., BC4_USE_ALL_MODES | BC4_SDC_SEARCH)` for one channel of a 4x4 block.
/// Panics if `channel >= 4`.
pub fn bc4_encode_block(px: &[[u8; 4]; 16], channel: usize, search_radius: u32) -> [u8; 8] {
    assert!(channel < 4, "channel {channel} is not 0..4");
    let v: [u8; 16] = std::array::from_fn(|i| px[i][channel]);
    bc4::search(&v, search_radius, None)
}

/// `rgbcx::encode_bc5_hq`: channel 0 then channel 1, each as [`bc4_encode_block`].
pub fn bc5_encode_block(px: &[[u8; 4]; 16], search_radius: u32) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&bc4_encode_block(px, 0, search_radius));
    out[8..].copy_from_slice(&bc4_encode_block(px, 1, search_radius));
    out
}

/// `rgbcx::encode_bc1(18, .., allow_3color, false)` in `cBC1Dual` mode for one 4x4 block of RGBA texels (alpha is
/// ignored). `allow_3color` lets the encoder pick a three-colour block; `perceptual` weights the colour error by
/// Rec.709 luma (rgbcx's `set_bc1_perceptual`).
pub fn bc1_encode_block(px: &[[u8; 4]; 16], allow_3color: bool, perceptual: bool) -> [u8; 8] {
    let weights = if perceptual {
        bc1::PERCEPTUAL
    } else {
        bc1::PLAIN
    };
    bc1::encode::encode(px, allow_3color, weights)
}

/// The alpha half of `rgbcx::encode_bc3_hq`: the BC4 search with the alpha-test guard at `cut` (BC4_SDC_GUARD128).
/// A `cut` of 0 is 128, as `encode_bc4_hq` reads bits 8..15 of its flags (sdc patch "bc4 guard cut").
pub fn bc4_encode_alpha(alpha: &[u8; 16], search_radius: u32, cut: u8) -> [u8; 8] {
    let cut = if cut != 0 {
        hits::hit!(hits::Hit::CutGiven);
        cut
    } else {
        hits::hit!(hits::Hit::CutDefault);
        128
    };
    bc4::search(alpha, search_radius, Some(cut))
}

/// `rgbcx::encode_bc1_alpha(18, ..)` in `cBC1Dual` mode: BC1 with one-bit alpha, bit i of `transparent` marking
/// texel i transparent. No bit set is [`bc1_encode_block`]`(px, true, perceptual)`; every bit set is the
/// all-transparent block; otherwise a 3-colour block fitted to the opaque texels, masked texels on selector 3. The
/// RGB of a masked texel and every alpha value are never read. `perceptual` as in [`bc1_encode_block`].
pub fn bc1_encode_block_alpha(px: &[[u8; 4]; 16], transparent: u16, perceptual: bool) -> [u8; 8] {
    let weights = if perceptual {
        bc1::PERCEPTUAL
    } else {
        bc1::PLAIN
    };
    bc1::alpha::encode_alpha(px, transparent, weights)
}

/// `rgbcx::encode_bc3_hq(18, .., search_radius, BC4_USE_ALL_MODES | BC4_SDC_SEARCH | BC4_SDC_GUARD128 | cut << 8)`
/// with the shim's weights: bytes 0..8 are [`bc4_encode_alpha`] of the alpha channel, bytes 8..16
/// [`bc1_encode_block`]`(px, false, perceptual)` (`encode_bc1(level, .., false, false)`; `perceptual` reaches only
/// this half).
pub fn bc3_encode_block(
    px: &[[u8; 4]; 16],
    search_radius: u32,
    cut: u8,
    perceptual: bool,
) -> [u8; 16] {
    let alpha: [u8; 16] = std::array::from_fn(|i| px[i][3]);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&bc4_encode_alpha(&alpha, search_radius, cut));
    out[8..].copy_from_slice(&bc1_encode_block(px, false, perceptual));
    out
}
