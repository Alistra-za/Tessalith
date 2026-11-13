//! Decode one DDS level for validation and inspection (`bcdec_rs`; BC1 at the NVIDIA decoder model with a Rust
//! port of rgbcx's decoder, spec §5.8, Notes FM17, HX1).
//!
//! The output size is derived from `data.len()`: every function checks that `data` holds the whole level (with
//! checked arithmetic) before allocating, so a hostile `w`/`h` cannot force a large allocation.
use crate::dds::{DdsError, DdsPixelFormat};

// Port of rgbcx's `interp_*_nv` helpers (bc7enc_rdo/rgbcx.cpp, MIT or public domain). C's `int` division
// truncates toward zero, and so does Rust's `/` on `i32`.
fn interp_5_nv(v0: i32, v1: i32) -> i32 {
    ((2 * v0 + v1) * 22) / 8
}
fn interp_6_nv(c0: i32, c1: i32) -> i32 {
    let gdiff = c1 - c0;
    (256 * c0 + (gdiff / 4) + 128 + gdiff * 80) / 256
}
fn interp_half_5_nv(v0: i32, v1: i32) -> i32 {
    ((v0 + v1) * 33) / 8
}
fn interp_half_6_nv(c0: i32, c1: i32) -> i32 {
    let gdiff = c1 - c0;
    (256 * c0 + gdiff / 4 + 128 + gdiff * 128) / 256
}

/// Decode one BC1 block to 16 RGBA texels (row-major) at the NVIDIA decoder model: a port of
/// `rgbcx::unpack_bc1(.., set_alpha = true, cBC1NVidia)` (`rgbcx.cpp`: `unpack_bc1_block_colors` and
/// `unpack_bc1`, the `cBC1NVidia` cases, plus the four `interp_*_nv` helpers). rgbcx is MIT or public domain.
/// Equal to `encoders_sys::bc1_decode_block(block, Bc1Model::Nvidia)` for every block (Note HX1).
#[inline]
pub fn bc1_block_nvidia(block: &[u8; 8]) -> [[u8; 4]; 16] {
    let l = u32::from(block[0]) | (u32::from(block[1]) << 8);
    let h = u32::from(block[2]) | (u32::from(block[3]) << 8);

    let cr0 = ((l >> 11) & 31) as i32;
    let cg0 = ((l >> 5) & 63) as i32;
    let cb0 = (l & 31) as i32;
    let r0 = (cr0 << 3) | (cr0 >> 2);
    let g0 = (cg0 << 2) | (cg0 >> 4);
    let b0 = (cb0 << 3) | (cb0 >> 2);

    let cr1 = ((h >> 11) & 31) as i32;
    let cg1 = ((h >> 5) & 63) as i32;
    let cb1 = (h & 31) as i32;
    let r1 = (cr1 << 3) | (cr1 >> 2);
    let g1 = (cg1 << 2) | (cg1 >> 4);
    let b1 = (cb1 << 3) | (cb1 >> 2);

    // `set_noclamp_rgba` stores each component as a `uint8_t`.
    let px = |r: i32, g: i32, b: i32, a: i32| [r as u8, g as u8, b as u8, a as u8];
    let mut c = [[0u8; 4]; 4];
    c[0] = px(r0, g0, b0, 255);
    c[1] = px(r1, g1, b1, 255);
    if l > h {
        c[2] = px(
            interp_5_nv(cr0, cr1),
            interp_6_nv(g0, g1),
            interp_5_nv(cb0, cb1),
            255,
        );
        c[3] = px(
            interp_5_nv(cr1, cr0),
            interp_6_nv(g1, g0),
            interp_5_nv(cb1, cb0),
            255,
        );
    } else {
        c[2] = px(
            interp_half_5_nv(cr0, cr1),
            interp_half_6_nv(g0, g1),
            interp_half_5_nv(cb0, cb1),
            255,
        );
        c[3] = px(0, 0, 0, 0);
    }

    let mut out = [[0u8; 4]; 16];
    for y in 0..4 {
        let sel = block[4 + y];
        for x in 0..4 {
            out[y * 4 + x] = c[usize::from((sel >> (x * 2)) & 3)];
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    Ldr(Vec<[u8; 4]>),
    Hdr(Vec<[f32; 3]>),
}

/// Validate `w`/`h` and that `data` holds a whole level; returns the level size in bytes.
fn level_len(fmt: DdsPixelFormat, w: u32, h: u32, data: &[u8]) -> Result<usize, DdsError> {
    if w == 0 || h == 0 {
        return Err(DdsError::Unsupported(format!("zero dimension ({w}x{h})")));
    }
    let need = fmt.checked_level_size(w, h).ok_or(DdsError::Truncated)?;
    if data.len() < need {
        return Err(DdsError::Truncated);
    }
    Ok(need)
}

/// Run `dec` over every `bs`-byte block and gather the `per`-element texels into a row-major `w` x `h` image.
/// `dec` fills a 4x4 scratch tile of `16 * per` elements.
fn blocks<T: Copy + Default>(
    w: u32,
    h: u32,
    data: &[u8],
    bs: usize,
    per: usize,
    mut dec: impl FnMut(&[u8], &mut [T]),
) -> Vec<T> {
    let (w, h) = (w as usize, h as usize);
    let bw = w.div_ceil(4);
    let mut img = vec![T::default(); w * h * per];
    let mut tmp = vec![T::default(); 16 * per];
    for (i, block) in data.chunks_exact(bs).take(bw * h.div_ceil(4)).enumerate() {
        dec(block, &mut tmp);
        let (bx, by) = (i % bw, i / bw);
        for y in 0..4.min(h - by * 4) {
            for x in 0..4.min(w - bx * 4) {
                let o = ((by * 4 + y) * w + bx * 4 + x) * per;
                img[o..o + per].copy_from_slice(&tmp[(y * 4 + x) * per..][..per]);
            }
        }
    }
    img
}

fn quads(v: Vec<u8>) -> Vec<[u8; 4]> {
    v.as_chunks::<4>().0.to_vec()
}

/// Decode a `w` x `h` level to RGBA8 (LDR) or RGB f32 (HDR).
///
/// BC1 decodes as an NVIDIA GPU does, with a Rust port of rgbcx's decoder (Notes FM17, HX1); the other block
/// formats use `bcdec_rs`. BC4 gives `[r, r, r, 255]` and BC5 `[r, g, 0, 255]` (unsigned), BC6H gives RGB floats,
/// half and float formats drop alpha. sRGB tags are not applied.
pub fn decode_level(fmt: DdsPixelFormat, w: u32, h: u32, data: &[u8]) -> Result<Decoded, DdsError> {
    decode(fmt, w, h, data, false)
}

/// `decode_level` for a DDS read as an input: BC1 decodes through `bcdec_rs` (exact interpolation), so a
/// re-encode does not bake one vendor's decoder error into the pixels (Note FM17).
pub fn decode_input_level(
    fmt: DdsPixelFormat,
    w: u32,
    h: u32,
    data: &[u8],
) -> Result<Decoded, DdsError> {
    decode(fmt, w, h, data, true)
}

fn decode(
    fmt: DdsPixelFormat,
    w: u32,
    h: u32,
    data: &[u8],
    bc1_exact: bool,
) -> Result<Decoded, DdsError> {
    use DdsPixelFormat::*;
    let need = level_len(fmt, w, h, data)?;
    let data = &data[..need];
    Ok(match fmt {
        Bc1 { .. } => Decoded::Ldr(quads(blocks(w, h, data, 8, 4, |b, o| {
            if bc1_exact {
                return bcdec_rs::bc1(b, o, 16);
            }
            let blk: &[u8; 8] = b.try_into().expect("8-byte block");
            for (i, p) in bc1_block_nvidia(blk).iter().enumerate() {
                o[i * 4..i * 4 + 4].copy_from_slice(p);
            }
        }))),
        Bc2 { .. } => Decoded::Ldr(quads(blocks(w, h, data, 16, 4, |b, o| {
            bcdec_rs::bc2(b, o, 16)
        }))),
        Bc3 { .. } => Decoded::Ldr(quads(blocks(w, h, data, 16, 4, |b, o| {
            bcdec_rs::bc3(b, o, 16)
        }))),
        Bc7 { .. } => Decoded::Ldr(quads(blocks(w, h, data, 16, 4, |b, o| {
            bcdec_rs::bc7(b, o, 16)
        }))),
        Bc4 => {
            let r = blocks(w, h, data, 8, 1, |b, o| bcdec_rs::bc4(b, o, 4, false));
            Decoded::Ldr(r.iter().map(|&v| [v, v, v, 255]).collect())
        }
        Bc5 => {
            let rg = blocks(w, h, data, 16, 2, |b, o| bcdec_rs::bc5(b, o, 8, false));
            Decoded::Ldr(
                rg.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| [c[0], c[1], 0, 255])
                    .collect(),
            )
        }
        Bc6hUf16 | Bc6hSf16 => {
            let signed = fmt == Bc6hSf16;
            let f = blocks(w, h, data, 16, 3, |b, o| {
                bcdec_rs::bc6h_float(b, o, 12, signed)
            });
            Decoded::Hdr(
                f.as_chunks::<3>()
                    .0
                    .iter()
                    .map(|c| [c[0], c[1], c[2]])
                    .collect(),
            )
        }
        Rgba8 { .. } => Decoded::Ldr(quads(data.to_vec())),
        Bgra8 { .. } | Bgrx8 => Decoded::Ldr(
            data.as_chunks::<4>()
                .0
                .iter()
                .map(|c| [c[2], c[1], c[0], if fmt == Bgrx8 { 255 } else { c[3] }])
                .collect(),
        ),
        Rgb8 => Decoded::Ldr(
            data.as_chunks::<3>()
                .0
                .iter()
                .map(|c| [c[2], c[1], c[0], 255])
                .collect(),
        ),
        La8 => Decoded::Ldr(
            data.as_chunks::<2>()
                .0
                .iter()
                .map(|c| [c[0], c[0], c[0], c[1]])
                .collect(),
        ),
        // Little-endian 16-bit grey; the LDR decode keeps the high byte (no 16-bit path exists here).
        R16 => Decoded::Ldr(
            data.as_chunks::<2>()
                .0
                .iter()
                .map(|c| [c[1], c[1], c[1], 255])
                .collect(),
        ),
        R8 => Decoded::Ldr(data.iter().map(|&v| [v, v, v, 255]).collect()),
        Rg8 => Decoded::Ldr(
            data.as_chunks::<2>()
                .0
                .iter()
                .map(|c| [c[0], c[1], 0, 255])
                .collect(),
        ),
        Rgba16f => Decoded::Hdr(
            data.as_chunks::<8>()
                .0
                .iter()
                .map(|c| {
                    [0, 1, 2].map(|k| half::f16::from_le_bytes([c[k * 2], c[k * 2 + 1]]).to_f32())
                })
                .collect(),
        ),
        Rgba32f => Decoded::Hdr(
            data.as_chunks::<16>()
                .0
                .iter()
                .map(|c| {
                    [0, 1, 2].map(|k| {
                        f32::from_le_bytes([c[k * 4], c[k * 4 + 1], c[k * 4 + 2], c[k * 4 + 3]])
                    })
                })
                .collect(),
        ),
    })
}

/// Decode a level to float RGBA: LDR formats as `code / 255`, BC6H as RGB with alpha 1, RGBA16F/RGBA32F with their
/// own alpha. For the DDS input loader (re-encode, DECISIONS S6) and the GUI preview.
pub fn decode_level_rgba(
    fmt: DdsPixelFormat,
    w: u32,
    h: u32,
    data: &[u8],
) -> Result<Vec<[f32; 4]>, DdsError> {
    rgba(fmt, w, h, data, false)
}

/// `decode_level_rgba` for a DDS read as an input (see [`decode_input_level`]).
pub fn decode_input_level_rgba(
    fmt: DdsPixelFormat,
    w: u32,
    h: u32,
    data: &[u8],
) -> Result<Vec<[f32; 4]>, DdsError> {
    rgba(fmt, w, h, data, true)
}

fn rgba(
    fmt: DdsPixelFormat,
    w: u32,
    h: u32,
    data: &[u8],
    bc1_exact: bool,
) -> Result<Vec<[f32; 4]>, DdsError> {
    use DdsPixelFormat::*;
    match fmt {
        Rgba16f | Rgba32f => {
            let need = level_len(fmt, w, h, data)?;
            let data = &data[..need];
            Ok(if fmt == Rgba16f {
                data.as_chunks::<8>()
                    .0
                    .iter()
                    .map(|c| {
                        [0, 1, 2, 3]
                            .map(|k| half::f16::from_le_bytes([c[k * 2], c[k * 2 + 1]]).to_f32())
                    })
                    .collect()
            } else {
                data.as_chunks::<16>()
                    .0
                    .iter()
                    .map(|c| {
                        [0, 1, 2, 3].map(|k| {
                            f32::from_le_bytes([c[k * 4], c[k * 4 + 1], c[k * 4 + 2], c[k * 4 + 3]])
                        })
                    })
                    .collect()
            })
        }
        _ => Ok(match decode(fmt, w, h, data, bc1_exact)? {
            Decoded::Ldr(v) => v.iter().map(|p| p.map(|c| f32::from(c) / 255.0)).collect(),
            Decoded::Hdr(v) => v.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect(),
        }),
    }
}
