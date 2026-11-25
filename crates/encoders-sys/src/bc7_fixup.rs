//! Post-encode alpha fixes on BC7 blocks (spec §5.6). Mode 6 needs none for opaque input: the vendored bc7e carries the native exact-opaque flag (DECISIONS Note FM).

/// If `block` is BC7 mode 4 or 5 with rotation 1-3, set both endpoints of the colour channel that
/// decodes as alpha to all-ones, so alpha decodes to 255. Lossless for RGB: the decoder swaps that
/// channel with the stored alpha field after decoding, so no output RGB value reads the patched bits.
/// Equal endpoints interpolate to a constant, so the indices and the mode-4 index-selection bit do
/// not matter. Returns true if the block is one of those modes (the patch is then applied).
///
/// Layout (bit 0 = LSB of byte 0): mode bits (mode 4: `10000`, bits 0-4; mode 5: `100000`, bits 0-5),
/// rotation (2 bits), mode 4 only: index-selection bit, then one subset of endpoints in the order
/// R0 R1 G0 G1 B0 B1 A0 A1 from bit 8 (colour 5 bits / alpha 6 in mode 4, colour 7 / alpha 8 in
/// mode 5; no p-bits). Rotation r swaps alpha with channel r-1 (1 = R, 2 = G, 3 = B).
pub fn force_opaque_rotated(block: &mut [u8; 16]) -> bool {
    let mut v = u128::from_le_bytes(*block);
    let (rot, bits) = if block[0] & 0x1f == 0x10 {
        ((v >> 5) & 3, 5)
    } else if block[0] & 0x3f == 0x20 {
        ((v >> 6) & 3, 7)
    } else {
        return false;
    };
    if rot == 0 {
        return false;
    }
    let pair_bits = 2 * bits;
    v |= ((1u128 << pair_bits) - 1) << (8 + pair_bits * (rot as u32 - 1));
    *block = v.to_le_bytes();
    true
}

/// `n` bits of `v` from bit `at`.
fn bits(v: u128, at: u32, n: u32) -> u32 {
    ((v >> at) & ((1u128 << n) - 1)) as u32
}

/// bcdec's `interpolate`: endpoints `a` and `b` (8-bit) at a 64-scale index weight.
fn lerp(a: u32, b: u32, w: u32) -> u8 {
    ((a * (64 - w) + b * w + 32) >> 6) as u8
}

const WEIGHT2: [u32; 4] = [0, 21, 43, 64];
const WEIGHT3: [u32; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const WEIGHT4: [u32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

/// Mode 7's 64 two-subset partitions (BC7 specification; the same shapes as `bcdec_rs`'s table): bit t is set when
/// texel t belongs to subset 1.
const PARTITION2_SUBSET1: [u16; 64] = [
    0xcccc, 0x8888, 0xeeee, 0xecc8, 0xc880, 0xfeec, 0xfec8, 0xec80, 0xc800, 0xffec, 0xfe80, 0xe800,
    0xffe8, 0xff00, 0xfff0, 0xf000, 0xf710, 0x008e, 0x7100, 0x08ce, 0x008c, 0x7310, 0x3100, 0x8cce,
    0x088c, 0x3110, 0x6666, 0x366c, 0x17e8, 0x0ff0, 0x718e, 0x399c, 0xaaaa, 0xf0f0, 0x5a5a, 0x33cc,
    0x3c3c, 0x55aa, 0x9696, 0xa55a, 0x73ce, 0x13c8, 0x324c, 0x3bdc, 0x6996, 0xc33c, 0x9966, 0x0660,
    0x0272, 0x04e4, 0x4e40, 0x2720, 0xc936, 0x936c, 0x39c6, 0x639c, 0x9336, 0x9cc6, 0x817e, 0xe718,
    0xccf0, 0x0fcc, 0x7744, 0xee22,
];

/// The texel that holds subset 1's anchor (its index has one bit less) in each two-subset partition; subset 0's
/// anchor is texel 0.
const PARTITION2_ANCHOR: [u8; 64] = [
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 2, 8, 2, 2, 8, 8, 15, 2, 8,
    2, 2, 8, 8, 2, 2, 15, 15, 6, 8, 2, 8, 15, 15, 2, 8, 2, 2, 2, 15, 15, 6, 6, 2, 6, 8, 15, 15, 2,
    2, 15, 15, 15, 15, 15, 2, 2, 15,
];

/// The index of texel `t` in an index set of `width` bits that starts at bit `at`, where only texel 0 (and `anchor2`,
/// when it is not 16) is stored with one bit less.
fn index_at(v: u128, at: u32, width: u32, anchor2: u32, t: u32) -> usize {
    // Texel 0's index takes one bit less, and so does the second anchor's: every later texel starts that much earlier.
    let shorter = u32::from(t > 0) + u32::from(anchor2 < t);
    let pos = at + width * t - shorter;
    // Texel 0 and the second anchor store `width - 1` bits.
    let n = if t == 0 || t == anchor2 {
        width - 1
    } else {
        width
    };
    bits(v, pos, n) as usize
}

/// The 16 alphas the BC7 decoder gives block `v` (a mode 4 to 7 block, else `None`), read by hand from the layout
/// below and equal to `bcdec_rs`'s (checked by `the_gate_decodes_alpha_as_bcdec_does`). No allocation: this runs for
/// every all-0 source block of every mip of an alpha-0 `_m`.
///
/// Mode 4's A at rotation 0 is the scalar slot (its endpoints at 38 and the alpha index set), and which index set
/// that is depends on the index-selector bit (bit 7): with 0 the alpha reads the 3-bit set (from bit 81) and the
/// colour the 2-bit set (from bit 50), with 1 the other way round. So the selector is read before the alpha indices.
/// At rotation r > 0 the decoder's A is the interpolation of colour channel r - 1 (its endpoints from bit
/// 8 + 10 (r - 1) in mode 4, 8 + 14 (r - 1) in mode 5) with the colour index set. Mode 5 stores the colour indices
/// from bit 66 and the alpha indices from bit 97, 2 bits each. Mode 6: alpha 7-bit x2 at 49, p-bits 63 and 64,
/// 4-bit indices from 65. Mode 7: partition at bit 8, alpha 5-bit x4 at 74, p-bits 94 to 97, 2-bit indices from 98.
fn decode_alpha(v: u128) -> Option<[u8; 16]> {
    let mut out = [0u8; 16];
    let first = v as u8;
    if first & 0x1f == 0x10 {
        let (rot, isel) = (bits(v, 5, 2), bits(v, 7, 1));
        // Index set 1 (2 bits, from 50) and set 2 (3 bits, from 81).
        let set1 = |t| WEIGHT2[index_at(v, 50, 2, 16, t)];
        let set2 = |t| WEIGHT3[index_at(v, 81, 3, 16, t)];
        let (a0, a1) = {
            let (x, y) = if rot == 0 {
                (bits(v, 38, 6), bits(v, 44, 6))
            } else {
                let at = 8 + 10 * (rot - 1);
                (bits(v, at, 5), bits(v, at + 5, 5))
            };
            if rot == 0 {
                ((x << 2) | (x >> 4), (y << 2) | (y >> 4))
            } else {
                ((x << 3) | (x >> 2), (y << 3) | (y >> 2))
            }
        };
        for (t, o) in out.iter_mut().enumerate() {
            let t = t as u32;
            // The set the alpha slot reads: rotation 0 is the scalar slot; else it is the colour slot's.
            let w = match (rot == 0, isel) {
                (true, 0) | (false, 1) => set2(t),
                _ => set1(t),
            };
            *o = lerp(a0, a1, w);
        }
        Some(out)
    } else if first & 0x3f == 0x20 {
        let rot = bits(v, 6, 2);
        let (a0, a1) = if rot == 0 {
            (bits(v, 50, 8), bits(v, 58, 8))
        } else {
            let at = 8 + 14 * (rot - 1);
            let (x, y) = (bits(v, at, 7), bits(v, at + 7, 7));
            ((x << 1) | (x >> 6), (y << 1) | (y >> 6))
        };
        // Colour indices from 66, alpha indices from 97; both 2 bits with texel 0 at one.
        let from = if rot == 0 { 97 } else { 66 };
        for (t, o) in out.iter_mut().enumerate() {
            *o = lerp(a0, a1, WEIGHT2[index_at(v, from, 2, 16, t as u32)]);
        }
        Some(out)
    } else if first & 0x7f == 0x40 {
        let a0 = (bits(v, 49, 7) << 1) | bits(v, 63, 1);
        let a1 = (bits(v, 56, 7) << 1) | bits(v, 64, 1);
        for (t, o) in out.iter_mut().enumerate() {
            *o = lerp(a0, a1, WEIGHT4[index_at(v, 65, 4, 16, t as u32)]);
        }
        Some(out)
    } else if first == 0x80 {
        let part = bits(v, 8, 6) as usize;
        let mut ep = [0u32; 4];
        for (i, e) in ep.iter_mut().enumerate() {
            let x = (bits(v, 74 + 5 * i as u32, 5) << 1) | bits(v, 94 + i as u32, 1);
            *e = (x << 2) | (x >> 4);
        }
        let anchor2 = u32::from(PARTITION2_ANCHOR[part]);
        for (t, o) in out.iter_mut().enumerate() {
            let s = usize::from(PARTITION2_SUBSET1[part] >> t & 1 == 1);
            let w = WEIGHT2[index_at(v, 98, 2, anchor2, t as u32)];
            *o = lerp(ep[2 * s], ep[2 * s + 1], w);
        }
        Some(out)
    } else {
        None
    }
}

/// The 16 alphas [`force_zero_alpha`]'s gate decodes for `block`, or `None` for a mode that stores no alpha (0 to 3).
/// Test builds only (feature `test-hooks`), so `tests/bc7_fixup.rs` can compare the gate with `bcdec_rs`.
#[cfg(feature = "test-hooks")]
pub fn gate_alpha(block: &[u8; 16]) -> Option<[u8; 16]> {
    decode_alpha(u128::from_le_bytes(*block))
}

/// Exact alpha 0 (DECISIONS AH1): only valid on blocks whose source alpha is 0 throughout. Sets both alpha endpoints
/// of every subset to 0 (and, in modes 6 and 7, the p-bits they share with RGB to 0), so alpha decodes to exactly 0.
/// RGB endpoints that had p-bit 1 move down (unoptimised): one LSB in mode 6 (7-bit + p, 1/255), one step of the
/// 6-bit value in mode 7 (5-bit + p, about 4/255). Modes 4 and 5 are lossless for RGB. Modes 0-3 store no alpha (it decodes as 255) and are left alone.
/// A block whose alpha already decodes 0 at every texel keeps its bytes (the gate, Note LR, LR18): the rewrite would
/// only move RGB (a cleared p-bit can undo a guarded G of 255). Returns true if the block was rewritten.
///
/// Layouts (bit 0 = LSB of byte 0): mode 4 alpha 6-bit x2 at 38, colour 5-bit; mode 5 alpha 8-bit x2 at 50, colour
/// 7-bit; with rotation r the alpha sits in colour pair r-1 from bit 8. Mode 6 alpha 7-bit x2 at 49, p-bits 63, 64.
/// Mode 7 (two subsets) alpha 5-bit x4 at 74, p-bits 94..98.
pub fn force_zero_alpha(block: &mut [u8; 16]) -> bool {
    let mut v = u128::from_le_bytes(*block);
    let field = |at: u32, len: u32| ((1u128 << len) - 1) << at;
    let mask = if block[0] & 0x1f == 0x10 {
        match (v >> 5) & 3 {
            0 => field(38, 12),
            r => field(8 + 10 * (r as u32 - 1), 10),
        }
    } else if block[0] & 0x3f == 0x20 {
        match (v >> 6) & 3 {
            0 => field(50, 16),
            r => field(8 + 14 * (r as u32 - 1), 14),
        }
    } else if block[0] & 0x7f == 0x40 {
        field(49, 16)
    } else if block[0] == 0x80 {
        field(74, 24)
    } else {
        return false;
    };
    // Nothing to clear, or the alpha already decodes 0 at every texel: the rewrite would only move RGB. The decode
    // runs only for a block with a bit to clear.
    if v & mask == 0 || decode_alpha(v).is_none_or(|a| a == [0u8; 16]) {
        return false;
    }
    v &= !mask;
    *block = v.to_le_bytes();
    true
}
