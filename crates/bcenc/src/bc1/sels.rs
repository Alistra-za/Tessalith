//! The selector searches. A port of rgbcx from bc7enc_rdo commit b943862 with Tessalith's patches (MIT or public
//! domain): `bc1_find_sels4` and `bc1_find_sels3_fullerr`, which under `cBC1Dual` return `bc1_find_sels4_dual` and
//! `bc1_find_sels3_dual` straight away, without forced selectors (Notes HY3, HY4). The R, G, B weights `w` reach the
//! error only through [`dual_err`], as the sdc patch's `bc1_dual_palette::w` does.

// `hit!` expands to nothing outside the unit tests, which leaves `Hit` unused there.
#[cfg_attr(not(test), allow(unused_imports))]
use crate::hits::{hit, Hit};

use super::palette::{dual_err, dual_palette};
use super::Ends;

/// `bc1_find_sels4` (sdc patch: `bc1_find_sels4_dual` under `cBC1Dual`): per texel the selector (rgbcx's linear
/// order) with the least NVIDIA plus AMD error; the first minimum wins except that selector 3 wins a tie, and a
/// texel stops at error 0. Returns the summed error. As soon as the running total reaches `cur_err` it returns that
/// partial total and leaves `sels` written only for the texels before it; callers only take `sels` when the total is
/// below `cur_err`.
pub(crate) fn find_sels4(
    px: &[[u8; 4]; 16],
    w: [i32; 3],
    e: &Ends,
    sels: &mut [u8; 16],
    cur_err: u32,
) -> u32 {
    let p = dual_palette(false, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);

    let mut total_err: u32 = 0;
    for (i, &c) in px.iter().enumerate() {
        let mut best_err = dual_err(&p, w, 0, c);
        let mut best_sel = 0u8;
        let mut j = 1;
        while j < 4 && best_err != 0 {
            let err = dual_err(&p, w, j, c);
            if err < best_err || (err == best_err && j == 3) {
                if err < best_err {
                    hit!(Hit::SelsBetter);
                } else {
                    hit!(Hit::SelsTieHigh);
                }
                best_err = err;
                best_sel = j as u8;
            }
            j += 1;
        }

        // uint32_t in the C++; 16 texel errors stay far below 2^32.
        total_err += best_err;
        if total_err >= cur_err {
            hit!(Hit::SelsEarlyOut);
            break;
        }

        sels[i] = best_sel;
    }
    total_err
}

/// `bc1_find_sels3_fullerr` (sdc patch: `bc1_find_sels3_dual` under `cBC1Dual`) with `use_black = false`: per texel the
/// 3-colour selector (0 low endpoint, 1 high endpoint, 2 midpoint; never 3, black) with the least NVIDIA plus AMD
/// error; a later selector replaces the best only when strictly better. `use_black` is false on every reachable call
/// (`try_3color_block` passes the literal `false`; the endpoint search passes `any_black_pixels &&
/// cEncodeBC1Use3ColorBlocksForBlackPixels`, a flag level 18 sets only when `allow_transparent_texels_for_black` is
/// true, which this call never passes), so the loop stops at selector 2. As soon as the running total reaches
/// `cur_err` it returns that partial total, with `sels` written only for the texels before it.
pub(crate) fn find_sels3(
    px: &[[u8; 4]; 16],
    w: [i32; 3],
    e: &Ends,
    sels: &mut [u8; 16],
    cur_err: u32,
) -> u32 {
    let p = dual_palette(true, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);

    let mut total_err: u32 = 0;
    for (i, &c) in px.iter().enumerate() {
        let mut best_err = dual_err(&p, w, 0, c);
        let mut best_sel = 0u8;
        for j in 1..=2 {
            let err = dual_err(&p, w, j, c);
            if err < best_err {
                hit!(Hit::Sels3Better);
                best_err = err;
                best_sel = j as u8;
            }
        }

        // uint32_t in the C++; 16 texel errors stay below 16 * 2 * 256 * 255^2 < 2^30 even with perceptual weights.
        total_err += best_err;
        if total_err >= cur_err {
            hit!(Hit::Sels3EarlyOut);
            return total_err;
        }

        sels[i] = best_sel;
    }
    total_err
}
