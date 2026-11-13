//! The 4-colour selector search. A port of rgbcx from bc7enc_rdo commit b943862 with Tessalith's patches (MIT or
//! public domain): `bc1_find_sels4`, which under `cBC1Dual` returns `bc1_find_sels4_dual` straight away, without
//! forced selectors (Note HY3).

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
