//! Readers of an encoded BC1 block for the tests that assert a block's colours and selectors.
#![allow(dead_code)]

/// The two 16-bit colours of a block: (colour 0, colour 1).
pub fn colours(b: &[u8; 8]) -> (u16, u16) {
    (
        u16::from_le_bytes([b[0], b[1]]),
        u16::from_le_bytes([b[2], b[3]]),
    )
}

/// The 2-bit selector of texel `i` as the block stores it.
pub fn sel(b: &[u8; 8], i: usize) -> u32 {
    (u32::from_le_bytes([b[4], b[5], b[6], b[7]]) >> (2 * i)) & 3
}
