//! Tessalith's encoder-free base crate: what the engine, the 3D preview (`render3d`) and other programs share.
//!
//! It holds the DDS container ([`dds`]: reader, writer, `Format`, the atomic file write), the pixel decoders
//! ([`decode`]), the float image type and size rule ([`image`]), the transfer curves ([`transfer`]), the cube-map
//! maths ([`cube`]) and, with the `load` feature, image loading (`load`). It has no C or C++ code and must never
//! depend on `engine` or `encoders-sys` (Note HX): `crates/render3d/tests/deps.rs` fails if `render3d` reaches
//! either again. The engine re-exports every item here at its old `engine::…` path.

pub mod cube;
pub mod dds;
pub mod decode;
pub mod image;
#[cfg(feature = "load")]
pub mod load;
#[cfg(feature = "load")]
mod psd;
pub mod transfer;
