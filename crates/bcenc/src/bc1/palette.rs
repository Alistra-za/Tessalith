//! The dual palettes and the dual single-colour tables. A port of rgbcx from bc7enc_rdo commit b943862 with
//! Tessalith's patches (MIT or public domain): `interp_5_nv`, `interp_6_nv`, `interp_half_5_nv`, `interp_half_6_nv`,
//! `interp_5_6_amd`, `interp_half_5_6_amd`, `bc1_get_dual_palette`, `bc1_dual_err` and
//! `prepare_bc1_single_color_table_dual` (as `init` calls it for the four single-colour tables).

// `hit!` expands to nothing outside the unit tests, which leaves `Hit` unused there.
#[cfg_attr(not(test), allow(unused_imports))]
use crate::hits::{hit, Hit};
use std::sync::OnceLock;

/// `interp_5_nv`: the NVIDIA 2/3 interpolation of two 5-bit codes.
pub(crate) fn interp_5_nv(v0: i32, v1: i32) -> i32 {
    ((2 * v0 + v1) * 22) / 8
}

/// `interp_6_nv`: the NVIDIA 2/3 interpolation of two expanded 6-bit values.
pub(crate) fn interp_6_nv(c0: i32, c1: i32) -> i32 {
    let gdiff = c1 - c0;
    (256 * c0 + (gdiff / 4) + 128 + gdiff * 80) / 256
}

/// `interp_half_5_nv`: the NVIDIA midpoint of two 5-bit codes.
pub(crate) fn interp_half_5_nv(v0: i32, v1: i32) -> i32 {
    ((v0 + v1) * 33) / 8
}

/// `interp_half_6_nv`: the NVIDIA midpoint of two expanded 6-bit values.
pub(crate) fn interp_half_6_nv(c0: i32, c1: i32) -> i32 {
    let gdiff = c1 - c0;
    (256 * c0 + gdiff / 4 + 128 + gdiff * 128) / 256
}

/// `interp_5_6_amd`: the AMD 2/3 interpolation of two expanded values.
pub(crate) fn interp_5_6_amd(c0: i32, c1: i32) -> i32 {
    (c0 * 43 + c1 * 21 + 32) >> 6
}

/// `interp_half_5_6_amd`: the AMD midpoint of two expanded values.
pub(crate) fn interp_half_5_6_amd(c0: i32, c1: i32) -> i32 {
    (c0 + c1 + 1) >> 1
}

/// `bc1_dual_palette` without the weights (callers pass those to [`dual_err`]): both palettes of a 4- or 3-colour
/// block, `[0]` NVIDIA and `[1]` AMD, in rgbcx's linear order. Entry 0 is the low endpoint, entry 3 of a 4-colour
/// block the high one, entries 1 and 2 the thirds; a 3-colour block has the high endpoint at 1, the midpoint at 2 and
/// black at 3. In block terms a 4-colour block's entries 0, 1, 2, 3 are palette entries 0, 3, 1, 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DualPalette {
    pub r: [[i32; 4]; 2],
    pub g: [[i32; 4]; 2],
    pub b: [[i32; 4]; 2],
}

/// `bc1_get_dual_palette`: `l*` and `h*` are the unexpanded 5/6/5-bit endpoint codes.
#[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
pub fn dual_palette(
    three: bool,
    lr: i32,
    lg: i32,
    lb: i32,
    hr: i32,
    hg: i32,
    hb: i32,
) -> DualPalette {
    let mut p = DualPalette {
        r: [[0; 4]; 2],
        g: [[0; 4]; 2],
        b: [[0; 4]; 2],
    };
    let (r0, g0, b0) = (
        (lr << 3) | (lr >> 2),
        (lg << 2) | (lg >> 4),
        (lb << 3) | (lb >> 2),
    );
    let (r3, g3, b3) = (
        (hr << 3) | (hr >> 2),
        (hg << 2) | (hg >> 4),
        (hb << 3) | (hb >> 2),
    );
    for k in 0..2 {
        p.r[k][0] = r0;
        p.g[k][0] = g0;
        p.b[k][0] = b0;
    }
    if three {
        hit!(Hit::PaletteThree);
        for k in 0..2 {
            p.r[k][1] = r3;
            p.g[k][1] = g3;
            p.b[k][1] = b3;
            p.r[k][3] = 0;
            p.g[k][3] = 0;
            p.b[k][3] = 0;
        }
        p.r[0][2] = interp_half_5_nv(lr, hr);
        p.g[0][2] = interp_half_6_nv(g0, g3);
        p.b[0][2] = interp_half_5_nv(lb, hb);
        p.r[1][2] = interp_half_5_6_amd(r0, r3);
        p.g[1][2] = interp_half_5_6_amd(g0, g3);
        p.b[1][2] = interp_half_5_6_amd(b0, b3);
    } else {
        hit!(Hit::PaletteFour);
        for k in 0..2 {
            p.r[k][3] = r3;
            p.g[k][3] = g3;
            p.b[k][3] = b3;
        }
        p.r[0][1] = interp_5_nv(lr, hr);
        p.g[0][1] = interp_6_nv(g0, g3);
        p.b[0][1] = interp_5_nv(lb, hb);
        p.r[0][2] = interp_5_nv(hr, lr);
        p.g[0][2] = interp_6_nv(g3, g0);
        p.b[0][2] = interp_5_nv(hb, lb);
        p.r[1][1] = interp_5_6_amd(r0, r3);
        p.g[1][1] = interp_5_6_amd(g0, g3);
        p.b[1][1] = interp_5_6_amd(b0, b3);
        p.r[1][2] = interp_5_6_amd(r3, r0);
        p.g[1][2] = interp_5_6_amd(g3, g0);
        p.b[1][2] = interp_5_6_amd(b3, b0);
    }
    p
}

/// `bc1_dual_err`: a texel's weighted NVIDIA error plus its weighted AMD error under selector `s` (rgbcx's linear
/// order). `w` are the R, G, B weights; `c` is the texel as r, g, b, a.
pub fn dual_err(p: &DualPalette, w: [i32; 3], s: usize, c: [u8; 4]) -> u32 {
    let sq = |a: i32| a * a;
    let (wr, wg, wb) = (w[0], w[1], w[2]);
    let (cr, cg, cb) = (c[0] as i32, c[1] as i32, c[2] as i32);
    // The C++ adds in int and returns uint32_t; the sums stay far below 2^31.
    ((wr * sq(p.r[0][s] - cr) + wg * sq(p.g[0][s] - cg) + wb * sq(p.b[0][s] - cb))
        + (wr * sq(p.r[1][s] - cr) + wg * sq(p.g[1][s] - cg) + wb * sq(p.b[1][s] - cb))) as u32
}

/// `bc1_match_entry`: the best endpoint codes for one 8-bit value and the (clamped) error they leave.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchEntry {
    pub hi: u8,
    pub lo: u8,
    pub e: u8,
}

/// The four single-colour tables `init` builds under `cBC1Dual`: 5-bit (red, blue) and 6-bit (green), for the
/// 4-colour (`equals_1`, the 2/3 entry) and 3-colour (`half`, the midpoint entry) case.
pub struct SingleTables {
    pub match5_equals_1: [MatchEntry; 256],
    pub match6_equals_1: [MatchEntry; 256],
    pub match5_half: [MatchEntry; 256],
    pub match6_half: [MatchEntry; 256],
}

/// The tables, built once.
pub fn single_tables() -> &'static SingleTables {
    static TABLES: OnceLock<SingleTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut expand5 = [0u8; 32];
        for (i, e) in expand5.iter_mut().enumerate() {
            *e = ((i << 3) | (i >> 2)) as u8;
        }
        let mut expand6 = [0u8; 64];
        for (i, e) in expand6.iter_mut().enumerate() {
            *e = ((i << 2) | (i >> 4)) as u8;
        }
        SingleTables {
            match5_equals_1: single_colour_table_dual(&expand5, false),
            match6_equals_1: single_colour_table_dual(&expand6, false),
            match5_half: single_colour_table_dual(&expand5, true),
            match6_half: single_colour_table_dual(&expand6, true),
        }
    })
}

/// `prepare_bc1_single_color_table_dual`. Error = squared NVIDIA error + squared AMD error of the interpolated
/// value; equal endpoints still win ties. `expand.len()` is `size` (32 or 64).
#[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
fn single_colour_table_dual(expand: &[u8], half: bool) -> [MatchEntry; 256] {
    let size = expand.len();
    if half {
        hit!(Hit::SingleHalf);
    } else {
        hit!(Hit::SingleFull);
    }
    if size == 32 {
        hit!(Hit::Single5);
    } else {
        hit!(Hit::Single6);
    }
    let sq = |a: i32| a * a;
    let mut table = [MatchEntry::default(); 256];
    for (i, entry) in table.iter_mut().enumerate() {
        let mut lowest_e = i32::MAX;
        for lo in 0..size {
            let lo_e = expand[lo] as i32;
            for hi in 0..size {
                let hi_e = expand[hi] as i32;
                let (vn, va) = if half {
                    if size == 32 {
                        (
                            interp_half_5_nv(hi as i32, lo as i32),
                            interp_half_5_6_amd(hi_e, lo_e),
                        )
                    } else {
                        (
                            interp_half_6_nv(hi_e, lo_e),
                            interp_half_5_6_amd(hi_e, lo_e),
                        )
                    }
                } else if size == 32 {
                    (
                        interp_5_nv(hi as i32, lo as i32),
                        interp_5_6_amd(hi_e, lo_e),
                    )
                } else {
                    (interp_6_nv(hi_e, lo_e), interp_5_6_amd(hi_e, lo_e))
                };
                let e = sq(vn - i as i32) + sq(va - i as i32);
                if e < lowest_e || (e == lowest_e && lo == hi) {
                    if e < lowest_e {
                        hit!(Hit::SingleImproved);
                    } else {
                        hit!(Hit::SingleTieEqualEnds);
                    }
                    entry.hi = hi as u8;
                    entry.lo = lo as u8;
                    lowest_e = e;
                }
            }
        }
        // The C++ clamps this to UINT8_MAX; no entry reaches it (checked by
        // `single_colour_entries_equal_dual_err`), so the clamp is not ported.
        entry.e = lowest_e as u8;
    }
    table
}
