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
mod tests {
    use super::super::palette::dual_palette;
    use super::super::{PERCEPTUAL, PLAIN};
    use super::*;
    use crate::gen;

    fn blocks() -> Vec<[[u8; 4]; 16]> {
        gen::colour_classes(0x5E15_0001, 40)
    }

    /// The 5:6:5 code of a colour.
    fn code(c: [u8; 3]) -> [i32; 3] {
        [
            gen::round_code(c[0], 31),
            gen::round_code(c[1], 63),
            gen::round_code(c[2], 31),
        ]
    }

    /// 24 endpoint pairs for a block: the codes of its least and greatest colour, that pair with each code moved by
    /// -1 and +1 (clamped), the pair swapped, both ends equal (twice), black to white and white to black, and 6
    /// random pairs.
    fn ends_for(px: &[[u8; 4]; 16], rng: &mut gen::Rng) -> Vec<Ends> {
        let lo: [u8; 3] = std::array::from_fn(|c| px.iter().map(|t| t[c]).min().unwrap());
        let hi: [u8; 3] = std::array::from_fn(|c| px.iter().map(|t| t[c]).max().unwrap());
        let (l, h) = (code(lo), code(hi));
        let max = [31, 63, 31];
        let mut v = vec![Ends { l, h }];
        for k in 0..6 {
            for d in [-1, 1] {
                let mut e = Ends { l, h };
                let side = if k < 3 { &mut e.l } else { &mut e.h };
                side[k % 3] = (side[k % 3] + d).clamp(0, max[k % 3]);
                v.push(e);
            }
        }
        v.push(Ends { l: h, h: l });
        v.push(Ends { l, h: l });
        v.push(Ends { l: h, h });
        v.push(Ends { l: [0; 3], h: max });
        v.push(Ends { l: max, h: [0; 3] });
        for _ in 0..6 {
            let mut r = |c: usize| rng.below(max[c] as u64 + 1) as i32;
            v.push(Ends {
                l: [r(0), r(1), r(2)],
                h: [r(0), r(1), r(2)],
            });
        }
        assert_eq!(v.len(), 24);
        v
    }

    /// The kernel under test: [`imp`] through the public functions, or [`portable`] directly. Under SSE2 these are
    /// two implementations; elsewhere the same one twice.
    trait Kernel {
        type T;
        fn new(px: &[[u8; 4]; 16], w: [i32; 3], dec: [i32; 2]) -> Self::T;
        fn f4(t: &Self::T, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32;
        fn f3(t: &Self::T, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32;
        fn masked(px: &[[u8; 4]; 16], w: [i32; 3], dec: [i32; 2], mask: u32) -> Self::T;
        fn f3m(t: &Self::T, e: &Ends, sels: &mut [u8; 16]) -> u32;
    }
    struct Public;
    impl Kernel for Public {
        type T = Texels;
        fn new(px: &[[u8; 4]; 16], w: [i32; 3], dec: [i32; 2]) -> Texels {
            Texels::new(px, w, dec)
        }
        fn masked(px: &[[u8; 4]; 16], w: [i32; 3], dec: [i32; 2], mask: u32) -> Texels {
            Texels::masked(px, w, dec, mask)
        }
        fn f3m(t: &Texels, e: &Ends, sels: &mut [u8; 16]) -> u32 {
            find_sels3_masked(t, e, sels)
        }
        fn f4(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
            find_sels4(t, e, sels, cutoff)
        }
        fn f3(t: &Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
            find_sels3(t, e, sels, cutoff)
        }
    }
    struct Portable;
    impl Kernel for Portable {
        type T = portable::Texels;
        fn new(px: &[[u8; 4]; 16], w: [i32; 3], dec: [i32; 2]) -> portable::Texels {
            portable::Texels::masked(px, w, dec, 0)
        }
        fn masked(px: &[[u8; 4]; 16], w: [i32; 3], dec: [i32; 2], mask: u32) -> portable::Texels {
            portable::Texels::masked(px, w, dec, mask)
        }
        fn f3m(t: &portable::Texels, e: &Ends, sels: &mut [u8; 16]) -> u32 {
            portable::find_sels3_masked(t, e, sels)
        }
        fn f4(t: &portable::Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
            portable::find_sels4(t, e, sels, cutoff)
        }
        fn f3(t: &portable::Texels, e: &Ends, sels: &mut [u8; 16], cutoff: u32) -> u32 {
            portable::find_sels3(t, e, sels, cutoff)
        }
    }

    fn equal_the_scalar_searches<K: Kernel>() {
        let mut rng = gen::Rng(0x5E15_0002);
        for px in &blocks() {
            for w in [PLAIN, PERCEPTUAL] {
                let t = K::new(px, w, DUAL);
                for e in ends_for(px, &mut rng) {
                    let (mut want, mut got) = ([0xEEu8; 16], [0xEEu8; 16]);
                    let full = scalar::find_sels4(px, w, &e, &mut want, u32::MAX);
                    assert_eq!(
                        K::f4(&t, &e, &mut got, u32::MAX),
                        2 * full,
                        "{px:?} {e:?} {w:?}"
                    );
                    assert_eq!(got, want, "{px:?} {e:?} {w:?}");
                    let full3 = scalar::find_sels3(px, w, &e, &mut want, u32::MAX);
                    assert_eq!(
                        K::f3(&t, &e, &mut got, u32::MAX),
                        2 * full3,
                        "{px:?} {e:?} {w:?}"
                    );
                    assert_eq!(got, want, "{px:?} {e:?} {w:?}");
                }
            }
        }
    }

    #[test]
    fn the_kernels_equal_the_scalar_searches() {
        equal_the_scalar_searches::<Public>();
        equal_the_scalar_searches::<Portable>();
    }

    #[test]
    fn the_dec_search_with_equal_weights_is_todays_search() {
        let mut rng = gen::Rng(0x5E15_0003);
        for px in &blocks() {
            for w in [PLAIN, PERCEPTUAL] {
                for e in ends_for(px, &mut rng) {
                    let (mut want, mut got) = ([0xEEu8; 16], [0xEEu8; 16]);
                    let full = scalar::find_sels4(px, w, &e, &mut want, u32::MAX);
                    assert_eq!(scalar::find_sels4_dec(px, w, [1, 1], &e, &mut got), full);
                    assert_eq!(got, want, "{px:?} {e:?} {w:?}");
                    let full3 = scalar::find_sels3(px, w, &e, &mut want, u32::MAX);
                    assert_eq!(scalar::find_sels3_dec(px, w, [1, 1], &e, &mut got), full3);
                    assert_eq!(got, want, "{px:?} {e:?} {w:?}");
                }
            }
        }
    }

    fn take_the_decoder_weights<K: Kernel>() {
        let mut rng = gen::Rng(0x5E15_0004);
        for px in &blocks() {
            for w in [PLAIN, PERCEPTUAL] {
                for dec in [[2, 1], [1, 2], [3, 1], [1, 3], [2, 2]] {
                    let t = K::new(px, w, dec);
                    let factor = (dec[0] + dec[1]) as u32;
                    for e in ends_for(px, &mut rng) {
                        let (mut want, mut got) = ([0xEEu8; 16], [0xEEu8; 16]);
                        let full = scalar::find_sels4_dec(px, w, dec, &e, &mut want);
                        assert_eq!(
                            K::f4(&t, &e, &mut got, u32::MAX),
                            factor * full,
                            "{px:?} {e:?} {w:?} {dec:?}"
                        );
                        assert_eq!(got, want, "{px:?} {e:?} {w:?} {dec:?}");
                        let full3 = scalar::find_sels3_dec(px, w, dec, &e, &mut want);
                        assert_eq!(
                            K::f3(&t, &e, &mut got, u32::MAX),
                            factor * full3,
                            "{px:?} {e:?} {w:?} {dec:?}"
                        );
                        assert_eq!(got, want, "{px:?} {e:?} {w:?} {dec:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_kernels_take_the_decoder_weights() {
        take_the_decoder_weights::<Public>();
        take_the_decoder_weights::<Portable>();
    }

    fn decide_by_the_cutoff<K: Kernel>() {
        let mut rng = gen::Rng(0x5E15_0005);
        for px in &blocks() {
            for w in [PLAIN, PERCEPTUAL] {
                let t = K::new(px, w, DUAL);
                for e in ends_for(px, &mut rng) {
                    for four in [true, false] {
                        let mut sink = [0u8; 16];
                        let full = if four {
                            scalar::find_sels4(px, w, &e, &mut sink, u32::MAX)
                        } else {
                            scalar::find_sels3(px, w, &e, &mut sink, u32::MAX)
                        };
                        for c in [1, full / 2, full, full + 1] {
                            let (mut want, mut got) = ([0xEEu8; 16], [0xEEu8; 16]);
                            let (r, t2) = if four {
                                (
                                    scalar::find_sels4(px, w, &e, &mut want, c),
                                    K::f4(&t, &e, &mut got, 2 * c),
                                )
                            } else {
                                (
                                    scalar::find_sels3(px, w, &e, &mut want, c),
                                    K::f3(&t, &e, &mut got, 2 * c),
                                )
                            };
                            assert_eq!(t2 < 2 * c, r < c, "{px:?} {e:?} {w:?} cutoff {c}");
                            if t2 < 2 * c {
                                assert_eq!(t2, 2 * r);
                                assert_eq!(got, want, "{px:?} {e:?} {w:?} cutoff {c}");
                            } else {
                                assert_eq!(got, [0xEE; 16], "sels written at or above the cutoff");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_cutoff_decides_as_the_scalar_search_does() {
        decide_by_the_cutoff::<Public>();
        decide_by_the_cutoff::<Portable>();
    }

    /// The texel of entry `j`'s colour on both decoders, when the two decoders agree on it.
    fn entry_colour(p: &super::super::palette::DualPalette, j: usize) -> Option<[u8; 4]> {
        let pal = [&p.r, &p.g, &p.b];
        if pal.iter().any(|c| c[0][j] != c[1][j]) {
            return None;
        }
        Some([
            pal[0][0][j] as u8,
            pal[1][0][j] as u8,
            pal[2][0][j] as u8,
            255,
        ])
    }

    fn ties_and_exact<K: Kernel>() {
        let mut rng = gen::Rng(0x5E15_0006);
        let blocks = blocks();
        // How many cases reached each entry (index 0..4: 4-colour, 4..7: 3-colour), so no entry goes untested.
        let mut reached = [0usize; 7];
        for (bi, px) in blocks.iter().enumerate() {
            for w in [PLAIN, PERCEPTUAL] {
                for e in ends_for(px, &mut rng) {
                    for four in [true, false] {
                        let p = dual_palette(!four, e.l[0], e.l[1], e.l[2], e.h[0], e.h[1], e.h[2]);
                        let entries = if four { 4 } else { 3 };
                        for j in 0..entries {
                            let Some(c) = entry_colour(&p, j) else {
                                continue;
                            };
                            // Every texel of this colour, the others from the block: a texel equal to entry `j`'s
                            // colour on both decoders takes the first entry that is, whatever follows.
                            let mut b = *px;
                            for (i, t) in b.iter_mut().enumerate() {
                                if (i + bi) % 3 == 0 {
                                    *t = c;
                                }
                            }
                            let first = (0..entries)
                                .find(|&k| entry_colour(&p, k) == Some(c))
                                .unwrap();
                            let t = K::new(&b, w, DUAL);
                            let mut got = [0xEEu8; 16];
                            let total = if four {
                                K::f4(&t, &e, &mut got, u32::MAX)
                            } else {
                                K::f3(&t, &e, &mut got, u32::MAX)
                            };
                            let mut want = [0u8; 16];
                            let full = if four {
                                scalar::find_sels4(&b, w, &e, &mut want, u32::MAX)
                            } else {
                                scalar::find_sels3(&b, w, &e, &mut want, u32::MAX)
                            };
                            assert_eq!(total, 2 * full);
                            assert_eq!(got, want, "{b:?} {e:?} {w:?}");
                            for (i, &g) in got.iter().enumerate() {
                                if (i + bi) % 3 == 0 {
                                    assert_eq!(g, first as u8, "texel {i} of {b:?} {e:?}");
                                }
                            }
                            reached[if four { first } else { 4 + first }] += 1;
                        }
                    }
                }
            }
        }

        assert!(
            reached.iter().all(|&n| n > 0),
            "an entry was never the exact match: {reached:?}"
        );

        // Equal endpoints: every entry the same colour, so a texel of that colour gets selector 0 and any other
        // texel 3 (4-colour) or 0 (3-colour).
        let mut seen_flat = [0, 0];
        for px in &blocks {
            for k in 0..32 {
                let c = [(k * 7) % 32, (k * 5 + 3) % 64, (k * 11 + 1) % 32];
                let e = Ends { l: c, h: c };
                for four in [true, false] {
                    let p = dual_palette(!four, c[0], c[1], c[2], c[0], c[1], c[2]);
                    let entries = if four { 4 } else { 3 };
                    let Some(col) = entry_colour(&p, 0) else {
                        continue;
                    };
                    if (1..entries).any(|j| entry_colour(&p, j) != Some(col)) {
                        continue;
                    }
                    seen_flat[usize::from(four)] += 1;
                    let mut b = *px;
                    b[5] = col;
                    b[9] = col;
                    let t = K::new(&b, PERCEPTUAL, DUAL);
                    let mut got = [0xEEu8; 16];
                    if four {
                        K::f4(&t, &e, &mut got, u32::MAX);
                    } else {
                        K::f3(&t, &e, &mut got, u32::MAX);
                    }
                    for (i, &g) in got.iter().enumerate() {
                        let want = if b[i][..3] == col[..3] || !four { 0 } else { 3 };
                        assert_eq!(g, want, "texel {i} {b:?} {e:?} four {four}");
                    }
                }
            }
        }
        assert!(
            seen_flat.iter().all(|&n| n > 0),
            "3-colour or 4-colour equal endpoints never held their premise: {seen_flat:?}"
        );

        // Two-colour blocks against the scalar searches.
        for px in &blocks {
            let (a, b) = (px[0], px[15]);
            for w in [PLAIN, PERCEPTUAL] {
                let mut two = [a; 16];
                for (i, t) in two.iter_mut().enumerate() {
                    if rng.below(2) == 0 || i == 15 {
                        *t = b;
                    }
                }
                let t = K::new(&two, w, DUAL);
                for e in [
                    Ends {
                        l: code([a[0], a[1], a[2]]),
                        h: code([b[0], b[1], b[2]]),
                    },
                    Ends {
                        l: code([b[0], b[1], b[2]]),
                        h: code([a[0], a[1], a[2]]),
                    },
                ] {
                    let (mut want, mut got) = ([0xEEu8; 16], [0xEEu8; 16]);
                    let full = scalar::find_sels4(&two, w, &e, &mut want, u32::MAX);
                    assert_eq!(K::f4(&t, &e, &mut got, u32::MAX), 2 * full);
                    assert_eq!(got, want, "{two:?} {e:?}");
                    let full3 = scalar::find_sels3(&two, w, &e, &mut want, u32::MAX);
                    assert_eq!(K::f3(&t, &e, &mut got, u32::MAX), 2 * full3);
                    assert_eq!(got, want, "{two:?} {e:?}");
                }
            }
        }
    }

    #[test]
    fn ties_and_exact_texels() {
        ties_and_exact::<Public>();
        ties_and_exact::<Portable>();
    }

    /// The inputs that make the largest errors: endpoints, blocks and channel weights.
    type Largest = ([Ends; 4], [[[u8; 4]; 16]; 3], [[i32; 3]; 6]);
    fn largest_inputs() -> Largest {
        let max = [31, 63, 31];
        let ends = [
            Ends {
                l: [0; 3],
                h: [0; 3],
            },
            Ends { l: max, h: max },
            Ends { l: [0; 3], h: max },
            Ends { l: max, h: [0; 3] },
        ];
        let blocks: [[[u8; 4]; 16]; 3] = [
            [[0; 4]; 16],
            [[255; 4]; 16],
            std::array::from_fn(|i| if i % 2 == 0 { [0; 4] } else { [255; 4] }),
        ];
        let weights = [
            PERCEPTUAL,
            PLAIN,
            [254, 1, 1],
            [1, 254, 1],
            [1, 1, 254],
            [86, 85, 85],
        ];
        (ends, blocks, weights)
    }

    fn largest_errors<K: Kernel>() {
        let (ends, blocks, weights) = largest_inputs();
        for px in &blocks {
            for e in &ends {
                for (wi, w) in weights.iter().enumerate() {
                    for dec in [[1, 1], [2, 2], [3, 1], [1, 3]] {
                        let t = K::new(px, *w, dec);
                        let factor = (dec[0] + dec[1]) as u32;
                        let (mut want, mut got) = ([0xEEu8; 16], [0xEEu8; 16]);
                        let full = scalar::find_sels4_dec(px, *w, dec, e, &mut want);
                        assert_eq!(
                            K::f4(&t, e, &mut got, u32::MAX),
                            factor * full,
                            "{px:?} {e:?} {w:?} {dec:?}"
                        );
                        assert_eq!(got, want);
                        let full3 = scalar::find_sels3_dec(px, *w, dec, e, &mut want);
                        assert_eq!(
                            K::f3(&t, e, &mut got, u32::MAX),
                            factor * full3,
                            "{px:?} {e:?} {w:?} {dec:?}"
                        );
                        assert_eq!(got, want);
                        if dec == [1, 1] && wi < 2 {
                            let full = scalar::find_sels4(px, *w, e, &mut want, u32::MAX);
                            assert_eq!(K::f4(&t, e, &mut got, u32::MAX), 2 * full);
                            assert_eq!(got, want, "{px:?} {e:?} {w:?}");
                            let full3 = scalar::find_sels3(px, *w, e, &mut want, u32::MAX);
                            assert_eq!(K::f3(&t, e, &mut got, u32::MAX), 2 * full3);
                            assert_eq!(got, want, "{px:?} {e:?} {w:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_largest_errors_do_not_overflow() {
        largest_errors::<Public>();
        largest_errors::<Portable>();
    }

    /// The masks the masked tests run: those of `gen::alpha_masks`, none and all.
    fn masks() -> Vec<u32> {
        let mut m: Vec<u32> = gen::alpha_masks(0xA1FA_0002, 12)
            .into_iter()
            .map(u32::from)
            .collect();
        m.extend([0, 0xFFFF]);
        m
    }

    fn masked_equals_the_scalar_search<K: Kernel>() {
        let mut rng = gen::Rng(0xA1FA_0003);
        // Masks of no, some and all texels: every kind must be reached.
        let (mut none, mut some, mut all) = (0usize, 0usize, 0usize);
        for px in &gen::colour_classes(0xA1FA_0001, 20) {
            for mask in masks() {
                for w in [PLAIN, PERCEPTUAL] {
                    let t = K::masked(px, w, DUAL, mask);
                    let tds = [[2, 1], [1, 3]].map(|dec| (dec, K::masked(px, w, dec, mask)));
                    for e in ends_for(px, &mut rng) {
                        let (mut want, mut got) = ([0xEEu8; 16], [0xEEu8; 16]);
                        let full = scalar::find_sels3_masked(px, mask, w, &e, &mut want);
                        assert_eq!(
                            K::f3m(&t, &e, &mut got),
                            2 * full,
                            "{px:?} {e:?} {w:?} {mask:#x}"
                        );
                        assert_eq!(got, want, "{px:?} {e:?} {w:?} {mask:#x}");
                        for (i, &s) in got.iter().enumerate() {
                            assert!(
                                s <= 2 || mask & (1 << i) != 0,
                                "opaque texel {i} got selector 3: {px:?} {e:?} {mask:#x}"
                            );
                            assert!(
                                mask & (1 << i) == 0 || s == 3,
                                "transparent texel {i} got {s}"
                            );
                        }
                        match mask.count_ones() {
                            0 => none += 1,
                            16 => all += 1,
                            _ => some += 1,
                        }
                        for (dec, td) in &tds {
                            let full =
                                scalar::find_sels3_masked_dec(px, mask, w, *dec, &e, &mut want);
                            assert_eq!(
                                K::f3m(td, &e, &mut got),
                                (dec[0] + dec[1]) as u32 * full,
                                "{px:?} {e:?} {w:?} {dec:?} {mask:#x}"
                            );
                            assert_eq!(got, want, "{px:?} {e:?} {w:?} {dec:?} {mask:#x}");
                        }
                    }
                }
            }
        }
        assert!(
            none > 0 && some > 0 && all > 0,
            "a kind of mask was never reached: none {none}, some {some}, all {all}"
        );
    }

    #[test]
    fn the_masked_kernel_equals_the_scalar_search() {
        masked_equals_the_scalar_search::<Public>();
        masked_equals_the_scalar_search::<Portable>();
    }

    #[test]
    fn the_masked_dec_search_with_equal_weights_is_todays_search() {
        let mut rng = gen::Rng(0xA1FA_0004);
        for px in &gen::colour_classes(0xA1FA_0001, 20) {
            for mask in masks() {
                for w in [PLAIN, PERCEPTUAL] {
                    for e in ends_for(px, &mut rng) {
                        let (mut want, mut got) = ([0xEEu8; 16], [0xEEu8; 16]);
                        let full = scalar::find_sels3_masked(px, mask, w, &e, &mut want);
                        assert_eq!(
                            scalar::find_sels3_masked_dec(px, mask, w, [1, 1], &e, &mut got),
                            full
                        );
                        assert_eq!(got, want, "{px:?} {e:?} {w:?} {mask:#x}");
                    }
                }
            }
        }
    }

    fn masked_largest_errors<K: Kernel>() {
        let (ends, blocks, weights) = largest_inputs();
        let mut reached = [0usize; 3];
        for px in &blocks {
            for e in &ends {
                for w in &weights {
                    for (kind, mask) in [0u32, 0x5555, 0xFFFF].into_iter().enumerate() {
                        let t = K::masked(px, *w, [2, 2], mask);
                        let (mut want, mut got) = ([0xEEu8; 16], [0xEEu8; 16]);
                        let full =
                            scalar::find_sels3_masked_dec(px, mask, *w, [2, 2], e, &mut want);
                        assert_eq!(
                            K::f3m(&t, e, &mut got),
                            4 * full,
                            "{px:?} {e:?} {w:?} {mask:#x}"
                        );
                        assert_eq!(got, want, "{px:?} {e:?} {w:?} {mask:#x}");
                        reached[kind] += 1;
                    }
                }
            }
        }
        assert!(reached.iter().all(|&n| n > 0), "{reached:?}");
    }

    #[test]
    fn the_masked_largest_errors_do_not_overflow() {
        masked_largest_errors::<Public>();
        masked_largest_errors::<Portable>();
    }

    #[test]
    #[should_panic(expected = "decoder weights")]
    fn decoder_weights_above_the_bound_are_refused() {
        Texels::new(&[[0; 4]; 16], PLAIN, [3, 2]);
    }

    #[test]
    #[should_panic(expected = "channel weights")]
    fn channel_weights_above_the_bound_are_refused() {
        Texels::new(&[[0; 4]; 16], [200, 100, 1], DUAL);
    }

    #[test]
    #[should_panic(expected = "transparent mask")]
    fn a_mask_above_16_bits_is_refused() {
        Texels::masked(&[[0; 4]; 16], PLAIN, DUAL, 0x1_0000);
    }
}
