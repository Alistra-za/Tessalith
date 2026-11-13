//! BC1: the total-ordering tables and the two-decoder ("dual", NVIDIA plus AMD) palettes of rgbcx's `cBC1Dual` mode
//! (Note HY2). `tables` is generated from the vendored text by `tools/gen-bcenc-tables.py`; `palette` ports the
//! palette and single-colour code.

pub mod palette;
pub mod tables;
