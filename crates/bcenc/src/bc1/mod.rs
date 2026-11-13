//! BC1: the total-ordering tables and the two-decoder ("dual", NVIDIA plus AMD) palettes of rgbcx's `cBC1Dual` mode
//! (Note HY2), and the four-colour encode at level 18 (Note HY3). `tables` is generated from the vendored text by
//! `tools/gen-bcenc-tables.py`; `palette` ports the palette and single-colour code; `encode` drives the encode,
//! `initial` picks the starting endpoints, `fit` holds the least-squares fits and rounding, `sels` the selector
//! search.

pub(crate) mod encode;
mod fit;
mod initial;
pub mod palette;
mod sels;
pub mod tables;
