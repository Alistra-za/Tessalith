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

/// `rgbcx::encode_bc1(18, .., allow_3color = false, false)` with plain weights, for the tests until the public BC1
/// entry point exists.
#[doc(hidden)]
pub fn bc1_encode4_for_tests(px: &[[u8; 4]; 16]) -> [u8; 8] {
    bc1::encode::encode4(px, [1, 1, 1])
}

/// The alpha half of `rgbcx::encode_bc3_hq`: the BC4 search with the alpha-test guard at `cut` (BC4_SDC_GUARD128).
pub fn bc4_encode_alpha(alpha: &[u8; 16], search_radius: u32, cut: u8) -> [u8; 8] {
    bc4::search(alpha, search_radius, Some(cut))
}
