//! The selector searches. A port of rgbcx from bc7enc_rdo commit b943862 with Tessalith's patches (MIT or public
//! domain): `bc1_find_sels4` and `bc1_find_sels3_fullerr`, which under `cBC1Dual` return `bc1_find_sels4_dual` and
//! `bc1_find_sels3_dual` straight away, without forced selectors (Notes HY3, HY4). The R, G, B weights `w` reach the
//! error only through [`Texels`], as the sdc patch's `bc1_dual_palette::w` does.
//!
//! The public searches ([`find_sels4`], [`find_sels3`]) score all 16 texels of a candidate at once and return the
//! block's full error in kernel units: the weighted error times `dec[0] + dec[1]`, the two decoder weights (twice
//! rgbcx's for [`DUAL`]). The encode only compares errors with each other, with 0 and with `u32::MAX` (which is only
//! ever a cutoff), so a common factor changes no decision. The search stops nowhere early: it returns the full total,
//! and writes `sels` only when the total is below `cutoff`; every caller only tests `total < cutoff` and reads `sels`
//! only then, and a full total is at least the partial one rgbcx returns, so the test gives the same answer.
//!
//! The tie rules are rgbcx's loops: per texel the first minimum wins, except that in a 4-colour block selector 3 wins
//! a tie with the least of selectors 0 to 2 unless that least is 0 (the C++ loop stops at error 0, so 3 never
//! replaces a 0); a 3-colour block has no such rule (a later selector replaces the best only when strictly better).
//!
//! Three private modules: `scalar` is rgbcx's loops (the reference of every test, with a variant that takes the
//! decoder weights and no cutoff), `portable` is the kernel's interface over those loops (correct, not tuned, with no
//! early exit; the unit tests run it on every target, and a target without SSE2 uses it), and `sse2` is the kernel.
//!
//! Why the kernel is a sum of products and not of squares (measured, survey section 5): with `W = dec[0] + dec[1]`,
//! palette values `a` (NVIDIA) and `b` (AMD) of a channel and `s = dec[0] * a + dec[1] * b`,
//! `W * (dec[0] * (c - a)^2 + dec[1] * (c - b)^2) = (W * c - s)^2 + dec[0] * dec[1] * (a - b)^2`, and the square
//! expands into a part of the texel alone, a part of the entry alone and one product per channel. Those products are
//! 16-bit by 16-bit whatever the channel weights, which SSE2 does two at a time (`pmaddwd`); squares need a 32-bit
//! multiply per texel under the perceptual weights, which SSE2 lacks, and plain loops left to the compiler's
//! vectoriser came out scalar for this form.

use super::Ends;
#[cfg(test)]
use super::DUAL;

/// A block's texels prepared for the searches, for one set of channel weights `w` and decoder weights `dec`.
pub(crate) struct Texels {
    k: imp::Texels,
    #[cfg(test)]
    px: [[u8; 4]; 16],
    #[cfg(test)]
    w: [i32; 3],
    #[cfg(test)]
    dec: [i32; 2],
    #[cfg(test)]
    mask: u32,
}

impl Texels {
    /// Panics unless every channel weight is in 1..=256 and their sum is at most 256, and both decoder weights are
    /// at least 1 and their sum is at most 4 (the kernel's integer bounds, written beside the kernel).
    pub(crate) fn new(px: &[[u8; 4]; 16], w: [i32; 3], dec: [i32; 2]) -> Self {
        Self::masked(px, w, dec, 0)
    }

    /// As `new`; texel i is transparent when bit i of `mask` is set (only [`find_sels3_masked`] reads the mask), and
    /// panics when a bit above the 16th is set. `new` is `masked` with mask 0.
    pub(crate) fn masked(px: &[[u8; 4]; 16], w: [i32; 3], dec: [i32; 2], mask: u32) -> Self {
        assert!(
            mask <= 0xFFFF,
            "transparent mask {mask:#x} has more than 16 bits"
        );
        assert!(
            w.iter().all(|&w| (1..=256).contains(&w)) && w.iter().sum::<i32>() <= 256,
            "channel weights {w:?} are outside the kernel's bounds"
        );
        assert!(
            dec.iter().all(|&d| d >= 1) && dec[0] + dec[1] <= 4,
            "decoder weights {dec:?} are outside the kernel's bounds"
        );
        Texels {
            k: imp::Texels::masked(px, w, dec, mask),
            #[cfg(test)]
            px: *px,
            #[cfg(test)]
            w,
            #[cfg(test)]
            dec,
            #[cfg(test)]
            mask,
        }
    }
}

/// The block's error under endpoints `e` with the best 4-colour selector per texel, in kernel units. Always the full
/// total. `sels` is written only when the total is below `cutoff`.
pub(crate) fn find_sels4(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
    let total = imp::find_sels4(&t.k, e, sels, cutoff);
    #[cfg(test)]
    assert_eq!(t.mask, 0, "a masked block goes to find_sels3_masked");
    #[cfg(test)]
    if t.dec == DUAL {
        // rgbcx's search with the same cutoff in its units: `2r < cutoff` exactly when `r < ceil(cutoff / 2)`, and
        // `u32::MAX` stays above every total.
        let mut want = [0xEEu8; 16];
        let r = scalar::find_sels4(&t.px, t.w, e, &mut want, cutoff.div_ceil(2));
        check_against_scalar(total, cutoff, r, cutoff.div_ceil(2), sels, &want);
    }
    total
}

/// [`find_sels4`] for a 3-colour block (selectors 0 low endpoint, 1 high endpoint, 2 midpoint).
pub(crate) fn find_sels3(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
    let total = imp::find_sels3(&t.k, e, sels, cutoff);
    #[cfg(test)]
    assert_eq!(t.mask, 0, "a masked block goes to find_sels3_masked");
    #[cfg(test)]
    if t.dec == DUAL {
        let mut want = [0xEEu8; 16];
        let r = scalar::find_sels3(&t.px, t.w, e, &mut want, cutoff.div_ceil(2));
        check_against_scalar(total, cutoff, r, cutoff.div_ceil(2), sels, &want);
    }
    total
}

/// [`find_sels3`] with no cutoff, for a block built with [`Texels::masked`]: all 16 selectors are written, and a
/// transparent texel gets selector 3 and adds no error. Returns the opaque texels' error in kernel units.
pub(crate) fn find_sels3_masked(t: &Texels, e: &Ends, sels: &mut [u8; 16]) -> u32 {
    let total = imp::find_sels3_masked(&t.k, e, sels);
    #[cfg(test)]
    if t.dec == DUAL {
        let mut want = [0xEEu8; 16];
        let r = scalar::find_sels3_masked(&t.px, t.mask, t.w, e, &mut want);
        assert_eq!(total, 2 * r, "kernel total");
        assert_eq!(sels, &want, "kernel selectors");
    }
    total
}

/// The shadow check: the kernel's answer against the scalar search's, below the cutoff or not, and when below the
/// same total (twice) and the same selectors. Unit tests only, so every block they encode checks every real call.
#[cfg(test)]
fn check_against_scalar(
    total: u32,
    cutoff: u32,
    r: u32,
    scalar_cutoff: u32,
    sels: &[u8; 16],
    want: &[u8; 16],
) {
    assert_eq!(
        total < cutoff,
        r < scalar_cutoff,
        "kernel total {total} and scalar total {r} disagree about cutoff {cutoff}"
    );
    if total < cutoff {
        assert_eq!(total, 2 * r, "kernel total");
        assert_eq!(sels, want, "kernel selectors");
    }
}

// The implementation the encode uses: the SSE2 kernel where the target has SSE2 (`x86_64-pc-windows-msvc`, the
// workspace's target, always), the portable one elsewhere.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod sse2;
#[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
use portable as imp;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
use sse2 as imp;

/// rgbcx's loops, the reference of every test and what `portable` runs.
#[cfg(any(test, not(all(target_arch = "x86_64", target_feature = "sse2"))))]
mod scalar {
    #[cfg(test)]
    use super::super::palette::dual_err;
    use super::super::palette::{dual_palette, DualPalette};
    use super::super::Ends;

    /// `bc1_find_sels4` (sdc patch: `bc1_find_sels4_dual` under `cBC1Dual`): per texel the selector (rgbcx's linear
    /// order) with the least NVIDIA plus AMD error; the first minimum wins except that selector 3 wins a tie, and a
    /// texel stops at error 0. Returns the summed error. As soon as the running total reaches `cur_err` it returns
    /// that partial total and leaves `sels` written only for the texels before it; callers only take `sels` when the
    /// total is below `cur_err`. Tests only (`portable` runs the `_dec` searches).
    #[cfg(test)]
    pub(super) fn find_sels4(
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
                    best_err = err;
                    best_sel = j as u8;
                }
                j += 1;
            }

            // uint32_t in the C++; 16 texel errors stay far below 2^32.
            total_err += best_err;
            if total_err >= cur_err {
                break;
            }

            sels[i] = best_sel;
        }
        total_err
    }

    /// `bc1_find_sels3_fullerr` (sdc patch: `bc1_find_sels3_dual` under `cBC1Dual`) with `use_black = false`: per
    /// texel the 3-colour selector (0 low endpoint, 1 high endpoint, 2 midpoint; never 3, black) with the least
    /// NVIDIA plus AMD error; a later selector replaces the best only when strictly better. `use_black` is false on
    /// every reachable call (`try_3color_block` passes the literal `false`; the endpoint search passes
    /// `any_black_pixels && cEncodeBC1Use3ColorBlocksForBlackPixels`, a flag level 18 sets only when
    /// `allow_transparent_texels_for_black` is true, which this call never passes), so the loop stops at selector 2.
    /// As soon as the running total reaches `cur_err` it returns that partial total, with `sels` written only for
    /// the texels before it. Tests only.
    #[cfg(test)]
    pub(super) fn find_sels3(
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
                    best_err = err;
                    best_sel = j as u8;
                }
            }

            // uint32_t in the C++; 16 texel errors stay below 16 * 2 * 256 * 255^2 < 2^30 even with perceptual
            // weights.
            total_err += best_err;
            if total_err >= cur_err {
                return total_err;
            }

            sels[i] = best_sel;
        }
        total_err
    }

    /// `bc1_alpha_find_sels` (sdc patch "bc1 alpha"): per opaque texel the 3-colour selector (0 low endpoint, 1 high
    /// endpoint, 2 midpoint) with the least NVIDIA plus AMD error, a later selector replacing the best only when
    /// strictly better; every texel whose bit is set in `mask` gets selector 3 (transparent black) and adds no error.
    /// No cutoff: all 16 selectors are written. Returns the summed error of the opaque texels. Tests only.
    #[cfg(test)]
    #[allow(clippy::needless_range_loop)] // Index loops mirror the C++ they port.
    pub(super) fn find_sels3_masked(
        px: &[[u8; 4]; 16],
        mask: u32,
        w: [i32; 3],
        e: &Ends,
        sels: &mut [u8; 16],
    ) -> u32 {
        let p = dual_palette(true, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);

        let mut total_err: u32 = 0;
        for i in 0..16 {
            if mask & (1u32 << i) != 0 {
                sels[i] = 3;
                continue;
            }
            let mut best_err = dual_err(&p, w, 0, px[i]);
            let mut best_sel = 0u8;
            for j in 1..3 {
                let err = dual_err(&p, w, j, px[i]);
                if err < best_err {
                    best_err = err;
                    best_sel = j as u8;
                }
            }
            sels[i] = best_sel;
            // uint32_t in the C++; 16 texel errors stay below 16 * 2 * 256 * 255^2 < 2^30.
            total_err += best_err;
        }
        total_err
    }

    /// A texel's weighted error under palette entry `s` with the decoders counted `dec[0]` and `dec[1]` times:
    /// `sum over c of w[c] * (dec[0] * (nv - c)^2 + dec[1] * (amd - c)^2)`. At most `256 * 4 * 255^2`, inside `i32`.
    fn dec_err(p: &DualPalette, w: [i32; 3], dec: [i32; 2], s: usize, c: [u8; 4]) -> u32 {
        let sq = |a: i32| a * a;
        let ch = |pal: &[[i32; 4]; 2], w: i32, v: u8| {
            w * (dec[0] * sq(pal[0][s] - v as i32) + dec[1] * sq(pal[1][s] - v as i32))
        };
        (ch(&p.r, w[0], c[0]) + ch(&p.g, w[1], c[1]) + ch(&p.b, w[2], c[2])) as u32
    }

    /// [`find_sels4`]'s loop with no cutoff and the decoder weights `dec`: the full total and every selector.
    pub(super) fn find_sels4_dec(
        px: &[[u8; 4]; 16],
        w: [i32; 3],
        dec: [i32; 2],
        e: &Ends,
        sels: &mut [u8; 16],
    ) -> u32 {
        let p = dual_palette(false, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);

        let mut total_err: u32 = 0;
        for (i, &c) in px.iter().enumerate() {
            let mut best_err = dec_err(&p, w, dec, 0, c);
            let mut best_sel = 0u8;
            let mut j = 1;
            while j < 4 && best_err != 0 {
                let err = dec_err(&p, w, dec, j, c);
                if err < best_err || (err == best_err && j == 3) {
                    best_err = err;
                    best_sel = j as u8;
                }
                j += 1;
            }
            total_err += best_err;
            sels[i] = best_sel;
        }
        total_err
    }

    /// [`find_sels3`]'s loop with no cutoff and the decoder weights `dec`.
    pub(super) fn find_sels3_dec(
        px: &[[u8; 4]; 16],
        w: [i32; 3],
        dec: [i32; 2],
        e: &Ends,
        sels: &mut [u8; 16],
    ) -> u32 {
        let p = dual_palette(true, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);

        let mut total_err: u32 = 0;
        for (i, &c) in px.iter().enumerate() {
            let mut best_err = dec_err(&p, w, dec, 0, c);
            let mut best_sel = 0u8;
            for j in 1..=2 {
                let err = dec_err(&p, w, dec, j, c);
                if err < best_err {
                    best_err = err;
                    best_sel = j as u8;
                }
            }
            total_err += best_err;
            sels[i] = best_sel;
        }
        total_err
    }

    /// [`find_sels3_masked`]'s loop with the decoder weights `dec`.
    #[allow(clippy::needless_range_loop)] // As `find_sels3_masked`.
    pub(super) fn find_sels3_masked_dec(
        px: &[[u8; 4]; 16],
        mask: u32,
        w: [i32; 3],
        dec: [i32; 2],
        e: &Ends,
        sels: &mut [u8; 16],
    ) -> u32 {
        let p = dual_palette(true, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);

        let mut total_err: u32 = 0;
        for i in 0..16 {
            if mask & (1u32 << i) != 0 {
                sels[i] = 3;
                continue;
            }
            let mut best_err = dec_err(&p, w, dec, 0, px[i]);
            let mut best_sel = 0u8;
            for j in 1..3 {
                let err = dec_err(&p, w, dec, j, px[i]);
                if err < best_err {
                    best_err = err;
                    best_sel = j as u8;
                }
            }
            sels[i] = best_sel;
            total_err += best_err;
        }
        total_err
    }
}

/// The kernel's interface over the scalar loops: correct, not tuned, no early exit. No target the workspace builds
/// for uses it outside the tests.
#[cfg(any(test, not(all(target_arch = "x86_64", target_feature = "sse2"))))]
mod portable {
    use super::super::Ends;
    use super::scalar;

    pub(super) struct Texels {
        px: [[u8; 4]; 16],
        w: [i32; 3],
        dec: [i32; 2],
        mask: u32,
    }

    impl Texels {
        pub(super) fn masked(px: &[[u8; 4]; 16], w: [i32; 3], dec: [i32; 2], mask: u32) -> Self {
            Texels {
                px: *px,
                w,
                dec,
                mask,
            }
        }
    }

    // The products below fit `u32`: a `_dec` total is at most 16 texels x 256 (the channel weights' sum) x 4 (the
    // decoder weights' sum) x 255^2 = 1,065,369,600, and times the decoder weights' sum again 4,261,478,400 < 2^32.
    // `Texels::masked` refuses weights above those sums.
    pub(super) fn find_sels3_masked(t: &Texels, e: &Ends, sels: &mut [u8; 16]) -> u32 {
        scalar::find_sels3_masked_dec(&t.px, t.mask, t.w, t.dec, e, sels)
            * (t.dec[0] + t.dec[1]) as u32
    }

    pub(super) fn find_sels4(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
        let mut local = [0u8; 16];
        let total =
            scalar::find_sels4_dec(&t.px, t.w, t.dec, e, &mut local) * (t.dec[0] + t.dec[1]) as u32;
        if total < cutoff {
            *sels = local;
        }
        total
    }

    pub(super) fn find_sels3(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
        let mut local = [0u8; 16];
        let total =
            scalar::find_sels3_dec(&t.px, t.w, t.dec, e, &mut local) * (t.dec[0] + t.dec[1]) as u32;
        if total < cutoff {
            *sels = local;
        }
        total
    }
}

#[cfg(test)]
mod tests;
