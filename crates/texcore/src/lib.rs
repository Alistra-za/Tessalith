//! Tessalith's encoder-free base crate: the types and per-texel helpers that engine, the DDS reader and the
//! 3D preview share. It must never depend on `engine` or `encoders-sys` (Note HX).

pub mod dds;
pub mod decode;
pub mod image;
pub mod transfer;
