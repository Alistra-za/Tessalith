//! BC1: the total-ordering tables and the two-decoder ("dual", NVIDIA plus AMD) palettes of rgbcx's `cBC1Dual` mode
//! (Note HY2), and the encode at level 18 with four- and three-colour blocks and plain or perceptual weights (Notes HY3,
//! HY4). `tables` is generated from the vendored text by `tools/gen-bcenc-tables.py`; `palette` ports the palette and
//! single-colour code; `encode` drives the encode, `initial` picks the starting endpoints, `fit` holds the
//! least-squares fits and rounding, `sels` the selector searches.

pub(crate) mod encode;
mod fit;
mod initial;
pub mod palette;
mod sels;
pub mod tables;

/// The R, G, B error weights with perceptual off: plain squared error.
pub(crate) const PLAIN: [i32; 3] = [1, 1, 1];
/// The weights `set_bc1_perceptual(true)` stores (sdc patch "bc1 perceptual"): Rec.709 luma scaled to 256.
pub(crate) const PERCEPTUAL: [i32; 3] = [54, 183, 19];

/// A block's two endpoints as 5:6:5 codes, r, g, b: `l` is rgbcx's `lr, lg, lb` (linear selector 0), `h` its
/// `hr, hg, hb` (linear selector 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ends {
    pub l: [i32; 3],
    pub h: [i32; 3],
}

/// What `encode_bc1` gathers from the texels before it picks endpoints.
pub(crate) struct BlockStats {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub avg: [i32; 3],
    pub total: [i32; 3],
    pub grayscale: bool,
}
