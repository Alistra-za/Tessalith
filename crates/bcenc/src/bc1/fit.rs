//! The BC1 endpoint fits: histograms and their total-ordering index, the least-squares endpoints (from selectors and
//! from a total ordering's prefix sums) and the rounding to 5:6:5. A port of rgbcx from bc7enc_rdo commit b943862
//! with Tessalith's patches (MIT or public domain): `to_5`, `to_6`, `hist4`, `compute_selector_factors4`, the 4-colour
//! half of `init` (`g_total_ordering4_hash`, `g_selector_factors4`), both `compute_least_squares_endpoints4_rgb` and
//! `precise_round_565` (Note HY3).
//!
//! Float maths follows the C++ operation by operation in `f32` (MSVC `/fp:precise`, x64 SSE2, no FMA). No float here
//! can be NaN or out of `i32` range where it is cast: see [`precise_round_565`].

// `hit!` expands to nothing outside the unit tests, which leaves `Hit` unused there.
#[cfg_attr(not(test), allow(unused_imports))]
use crate::hits::{hit, Hit};
use std::sync::OnceLock;

use super::tables::{N4, UNIQUE_TOTAL_ORDERS4};

/// `to_5`: an 8-bit value to the nearest 5-bit code.
pub(crate) fn to_5(v: u32) -> i32 {
    let v = v * 31 + 128;
    ((v + (v >> 8)) >> 8) as u8 as i32
}

/// `to_6`: an 8-bit value to the nearest 6-bit code.
pub(crate) fn to_6(v: u32) -> i32 {
    let v = v * 63 + 128;
    ((v + (v >> 8)) >> 8) as u8 as i32
}

/// `TOTAL_ORDER_4_0_16` .. `TOTAL_ORDER_4_3_16`: the orderings whose histogram has all 16 texels on one selector.
pub(crate) const TOTAL_ORDER_4_16: [usize; 4] = [15, 700, 753, 515];

/// `g_weight_vals4`: per selector, the packed (z00, z10, z11) contributions, times 9.
const WEIGHT_VALS4: [u32; 4] = [0x000009, 0x010204, 0x040201, 0x090000];

/// `hist4`: how many texels use each selector (rgbcx's linear order).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Hist4(pub [u8; 4]);

impl Hist4 {
    /// `hist4::any_16`.
    fn any_16(&self) -> bool {
        let h = &self.0;
        h[0] == 16 || h[1] == 16 || h[2] == 16 || h[3] == 16
    }

    /// `hist4::lookup_total_ordering_index`.
    pub(crate) fn lookup_total_ordering_index(&self) -> usize {
        let h = &self.0;
        // Not ported: the C++ first tests m_hist[0] == 16 (TOTAL_ORDER_4_0_16). Every texel on the low endpoint with a
        // non-zero error never reaches this lookup at level 18 (no generated block takes it, Note HY3); were it
        // reached, the hash below would give a wrong ordering, which the assertion catches in test builds.
        debug_assert_ne!(h[0], 16, "unported arm: every texel on selector 0");
        if h[1] == 16 {
            hit!(Hit::Hist16Sel1);
            TOTAL_ORDER_4_16[1]
        } else if h[2] == 16 {
            hit!(Hit::Hist16Sel2);
            TOTAL_ORDER_4_16[2]
        } else if h[3] == 16 {
            hit!(Hit::Hist16Sel3);
            TOTAL_ORDER_4_16[3]
        } else {
            hit!(Hit::HistHashed);
            // Must sum to 16, so m_hist[3] isn't needed.
            order_tables().hash[h[0] as usize | ((h[1] as usize) << 4) | ((h[2] as usize) << 8)]
                as usize
        }
    }
}

/// `compute_selector_factors4`: the inverse of the least-squares normal matrix of a histogram, scaled by 3/255;
/// zero when the matrix is singular. Returns (iz00, iz10, iz11).
#[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
fn compute_selector_factors4(h: &Hist4) -> [f32; 3] {
    let mut weight_accum: u32 = 0;
    for sel in 0..4 {
        weight_accum += WEIGHT_VALS4[sel] * h.0[sel] as u32;
    }

    let z00 = ((weight_accum >> 16) & 0xFF) as f32;
    let z10 = ((weight_accum >> 8) & 0xFF) as f32;
    let z11 = (weight_accum & 0xFF) as f32;
    let z01 = z10;

    let mut det = z00 * z11 - z01 * z10;
    // The C++ calls `fabs` on a float; |det| < 1e-8f compares the same in float and double.
    if det.abs() < 1e-8 {
        hit!(Hit::FactorsSingular);
        det = 0.0;
    } else {
        hit!(Hit::FactorsSolved);
        det = (3.0f32 / 255.0f32) / det;
    }

    [z11 * det, -z10 * det, z00 * det]
}

/// The tables `init` builds for 4-colour blocks.
pub(crate) struct OrderTables {
    /// `g_total_ordering4_hash`: histogram (h0 | h1 << 4 | h2 << 8) to its total-ordering index.
    pub hash: [u16; 4096],
    /// `g_selector_factors4`: [`compute_selector_factors4`] of every total ordering.
    pub factors: [[f32; 3]; N4],
}

/// The 4-colour half of `init`, built once.
pub(crate) fn order_tables() -> &'static OrderTables {
    static TABLES: OnceLock<OrderTables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut t = OrderTables {
            hash: [0; 4096],
            factors: [[0.0; 3]; N4],
        };
        for (i, o) in UNIQUE_TOTAL_ORDERS4.iter().enumerate() {
            let h = Hist4(*o);
            if !h.any_16() {
                let index = h.0[0] as usize | ((h.0[1] as usize) << 4) | ((h.0[2] as usize) << 8);
                t.hash[index] = i as u16;
            }
            t.factors[i] = compute_selector_factors4(&h);
        }
        t
    })
}

/// The table form of `compute_least_squares_endpoints4_rgb`: the least-squares low and high colours (0..1 scale) of
/// total ordering `s`, from the prefix sums of the texels sorted along the current axis. `iz` is
/// `g_selector_factors4[s]`. Returns (xl, xh).
pub(crate) fn ls_endpoints4_ordered(
    total: [i32; 3],
    iz: [f32; 3],
    s: usize,
    sums: &[[u32; 17]; 3],
) -> ([f32; 3], [f32; 3]) {
    let [iz00, iz10, iz11] = iz;
    let iz01 = iz10;

    let o = &UNIQUE_TOTAL_ORDERS4[s];
    let f1 = o[0] as usize;
    let f2 = o[0] as usize + o[1] as usize;
    let f3 = o[0] as usize + o[1] as usize + o[2] as usize;
    let mut xl = [0.0f32; 3];
    let mut xh = [0.0f32; 3];
    for c in 0..3 {
        let sum = &sums[c];
        // uint32_t in the C++. The prefix sums never decrease, so no difference wraps.
        let uq00 = (sum[f2] - sum[f1]) + (sum[f3] - sum[f2]) * 2 + (sum[16] - sum[f3]) * 3;
        // `(float)(total * 3 - uq00)`: int * 3 converted to uint32_t for the subtraction, then to float. uq00 is at
        // most 3 * total, so it never wraps; wrapping_sub keeps the C++ type either way.
        let q10 = ((total[c] * 3) as u32).wrapping_sub(uq00) as f32;
        xl[c] = iz00 * uq00 as f32 + iz01 * q10;
        xh[c] = iz10 * uq00 as f32 + iz11 * q10;
    }
    (xl, xh)
}

/// The selector form of `compute_least_squares_endpoints4_rgb`: the least-squares low and high colours (0..1
/// scale, as the 3/255 factor puts it) for selectors `sels` (rgbcx's linear order). `None` when every texel has one selector (the C++ returns
/// false). Returns (xl, xh).
#[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
pub(crate) fn ls_endpoints4(
    px: &[[u8; 4]; 16],
    sels: &[u8; 16],
    total: [i32; 3],
) -> Option<([f32; 3], [f32; 3])> {
    let mut uq00 = [0u32; 3];
    let mut weight_accum: u32 = 0;
    for i in 0..16 {
        let sel = sels[i] as u32;
        weight_accum += WEIGHT_VALS4[sel as usize];
        for c in 0..3 {
            uq00[c] += sel * px[i][c] as u32;
        }
    }

    // `int q10 = total * 3 - uq00`: the uint32_t difference converted to int; uq00 <= 3 * total, so it is the plain
    // difference.
    let q10: [i32; 3] =
        std::array::from_fn(|c| ((total[c] * 3) as u32).wrapping_sub(uq00[c]) as i32);

    let z00 = ((weight_accum >> 16) & 0xFF) as f32;
    let z10 = ((weight_accum >> 8) & 0xFF) as f32;
    let z11 = (weight_accum & 0xFF) as f32;
    let z01 = z10;

    let mut det = z00 * z11 - z01 * z10;
    if det.abs() < 1e-8 {
        hit!(Hit::LsSelsSingular);
        return None;
    }
    hit!(Hit::LsSelsSolved);

    det = (3.0f32 / 255.0f32) / det;

    let iz00 = z11 * det;
    let iz01 = -z01 * det;
    let iz10 = -z10 * det;
    let iz11 = z00 * det;

    let xl = std::array::from_fn(|c| iz00 * uq00[c] as f32 + iz01 * q10[c] as f32);
    let xh = std::array::from_fn(|c| iz10 * uq00[c] as f32 + iz11 * q10[c] as f32);
    Some((xl, xh))
}

/// `g_midpoint5`: the rounding thresholds between adjacent 5-bit codes (0..1 scale); the last never rounds up.
#[rustfmt::skip]
const MIDPOINT5: [f32; 32] = [
    0.015686, 0.047059, 0.078431, 0.111765, 0.145098, 0.176471, 0.207843, 0.241176, 0.274510, 0.305882, 0.337255,
    0.370588, 0.403922, 0.435294, 0.466667, 0.5, 0.533333, 0.564706, 0.596078, 0.629412, 0.662745, 0.694118, 0.725490,
    0.758824, 0.792157, 0.823529, 0.854902, 0.888235, 0.921569, 0.952941, 0.984314, 1e+37,
];

/// `g_midpoint6`: the same for 6-bit codes.
#[rustfmt::skip]
const MIDPOINT6: [f32; 64] = [
    0.007843, 0.023529, 0.039216, 0.054902, 0.070588, 0.086275, 0.101961, 0.117647, 0.133333, 0.149020, 0.164706,
    0.180392, 0.196078, 0.211765, 0.227451, 0.245098, 0.262745, 0.278431, 0.294118, 0.309804, 0.325490, 0.341176,
    0.356863, 0.372549, 0.388235, 0.403922, 0.419608, 0.435294, 0.450980, 0.466667, 0.482353, 0.500000, 0.517647,
    0.533333, 0.549020, 0.564706, 0.580392, 0.596078, 0.611765, 0.627451, 0.643137, 0.658824, 0.674510, 0.690196,
    0.705882, 0.721569, 0.737255, 0.754902, 0.772549, 0.788235, 0.803922, 0.819608, 0.835294, 0.850980, 0.866667,
    0.882353, 0.898039, 0.913725, 0.929412, 0.945098, 0.960784, 0.976471, 0.992157, 1e+37,
];

/// The C++ `x = ((uint32_t)x > max) ? (~x >> 31) & max : x`: a negative code becomes 0, one above `max` becomes `max`.
fn clamp_code(x: i32, max: i32) -> i32 {
    if (x as u32) > max as u32 {
        (!x >> 31) & max
    } else {
        x
    }
}

/// `precise_round_565`: 5:6:5 codes for two colours on the 0..1 scale. Returns (codes of `xl`, codes of `xh`) as
/// r, g, b; the callers that pass least-squares results store the first as the high endpoint, as the C++ does.
///
/// `(int)(x * 31.0f)` truncates toward zero like `as i32`. The values are finite and far inside `i32`: the
/// least-squares inputs are bounded by the factors (|iz| <= 3/255 * 144 / 1, as a non-zero det is an integer) times
/// the sums (<= 3 * 16 * 255), the bounding-box inputs are clamped to 0..1, so `as i32` never saturates and no NaN
/// reaches it (cvttss2si would give 0x80000000 there).
pub(crate) fn precise_round_565(xl: &[f32; 3], xh: &[f32; 3]) -> ([i32; 3], [i32; 3]) {
    let mut trial_lr = (xl[0] * 31.0) as i32;
    let mut trial_lg = (xl[1] * 63.0) as i32;
    let mut trial_lb = (xl[2] * 31.0) as i32;

    let mut trial_hr = (xh[0] * 31.0) as i32;
    let mut trial_hg = (xh[1] * 63.0) as i32;
    let mut trial_hb = (xh[2] * 31.0) as i32;

    if ((trial_lr | trial_lb | trial_hr | trial_hb) as u32) > 31 {
        hit!(Hit::RoundClampRb);
        trial_lr = clamp_code(trial_lr, 31);
        trial_hr = clamp_code(trial_hr, 31);

        trial_lb = clamp_code(trial_lb, 31);
        trial_hb = clamp_code(trial_hb, 31);
    }

    if ((trial_lg | trial_hg) as u32) > 63 {
        hit!(Hit::RoundClampG);
        trial_lg = clamp_code(trial_lg, 63);
        trial_hg = clamp_code(trial_hg, 63);
    }

    trial_lr = (trial_lr + (xl[0] > MIDPOINT5[trial_lr as usize]) as i32) & 31;
    trial_lg = (trial_lg + (xl[1] > MIDPOINT6[trial_lg as usize]) as i32) & 63;
    trial_lb = (trial_lb + (xl[2] > MIDPOINT5[trial_lb as usize]) as i32) & 31;

    trial_hr = (trial_hr + (xh[0] > MIDPOINT5[trial_hr as usize]) as i32) & 31;
    trial_hg = (trial_hg + (xh[1] > MIDPOINT6[trial_hg as usize]) as i32) & 63;
    trial_hb = (trial_hb + (xh[2] > MIDPOINT5[trial_hb as usize]) as i32) & 31;

    (
        [trial_lr, trial_lg, trial_lb],
        [trial_hr, trial_hg, trial_hb],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_single_selector_orderings_are_the_16_count_histograms() {
        for (sel, &s) in TOTAL_ORDER_4_16.iter().enumerate() {
            let mut want = [0u8; 4];
            want[sel] = 16;
            assert_eq!(UNIQUE_TOTAL_ORDERS4[s], want, "ordering {s}");
        }
    }

    /// Reads the hash directly, not through the counted lookup, so the coverage test sees only the encoder's calls.
    #[test]
    fn every_histogram_hashes_back_to_its_ordering() {
        let hash = &order_tables().hash;
        for (i, o) in UNIQUE_TOTAL_ORDERS4.iter().enumerate() {
            if !TOTAL_ORDER_4_16.contains(&i) {
                let index = o[0] as usize | ((o[1] as usize) << 4) | ((o[2] as usize) << 8);
                assert_eq!(hash[index] as usize, i, "histogram {o:?}");
            }
        }
    }
}
