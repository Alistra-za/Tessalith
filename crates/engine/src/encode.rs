//! Compress (spec §5.6): one block row per rayon task; bc7e gets at most 64 blocks per call.
//!
//! Output is a pure function of the input: every block is encoded independently from its own 16
//! texels, so the bytes do not depend on the rayon thread count or on band scheduling.
use crate::config::*;
use crate::grass::{unit_err, CS_GRASS_THRESHOLD, GRASS_GUARD_MARGIN};
use crate::quantize::EncoderInput;
// BC1, BC3, BC4 and BC5 blocks come from `bcenc`, the Rust port of rgbcx, byte-equal to it (Note HY7). Its calls
// below are written `bcenc::..`: the `encoders_sys` glob still holds rgbcx's functions of the same names.
use encoders_sys::bc7_fixup::{force_opaque_rotated, force_zero_alpha};
use encoders_sys::*;
use rayon::prelude::*;
use std::borrow::Cow;
use std::num::NonZeroU8;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Blocks per bc7e call (what the bench used).
const BC7_BLOCKS_PER_CALL: usize = 64;
/// Texels per 4x4 block.
const BLOCK_TEXELS: usize = 16;
/// Bytes per BC7 block.
const BC7_BLOCK_BYTES: usize = 16;

#[derive(Debug, Clone, Default)]
pub struct EncodeCtx {
    /// When set to true (from any thread), the level in flight stops at the next band boundary.
    pub cancel: Option<Arc<AtomicBool>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("cancelled")]
pub struct Cancelled;

pub fn bc7_level(e: Effort) -> Bc7Level {
    match e {
        Effort::Ultrafast => Bc7Level::Ultrafast,
        Effort::Veryfast => Bc7Level::Veryfast,
        Effort::Fast => Bc7Level::Fast,
        Effort::Basic => Bc7Level::Basic,
        Effort::Veryslow => Bc7Level::Veryslow,
        Effort::Slowest => Bc7Level::Slowest,
        Effort::Max => Bc7Level::Max,
    }
}

pub fn bc6h_level(e: Effort) -> Bc6hLevel {
    match e {
        Effort::Ultrafast | Effort::Veryfast => Bc6hLevel::Veryfast,
        Effort::Fast => Bc6hLevel::Fast,
        Effort::Basic => Bc6hLevel::Basic,
        Effort::Veryslow => Bc6hLevel::Veryslow,
        // Note FF: the peak-preserving mode (Note FE).
        Effort::Slowest | Effort::Max => Bc6hLevel::VeryslowPeak,
    }
}

/// The least alpha that passes the texture's alpha test, the cutoff the BC3 alpha guard keeps texels on the right side
/// of (Note FM): `gt` t passes alpha >= t + 1, `ge` t passes alpha >= t. `threshold` is 1..=254 (policy refuses any other
/// value with CFG-INVALID, and a mesh threshold is clamped), so the result is 1..=255. The default (127, `gt`) gives 128.
pub fn alpha_cut(a: &AlphaSpec) -> u8 {
    let cut = match a.compare {
        AlphaCompare::Gt => a.threshold.saturating_add(1),
        AlphaCompare::Ge => a.threshold,
    };
    // rgbcx read cutoff 0 as "use 128", so a 0 here would silently guard the wrong test.
    debug_assert!(cut >= 1, "alpha threshold {} gives cutoff 0", a.threshold);
    cut
}

/// An [`alpha_cut`] as the BC3 encoder takes it (Note HY7). A 0, which policy rules out, stays the 128 that rgbcx
/// made of it, so no input changes its bytes.
fn bc3_cut(cut: u8) -> NonZeroU8 {
    NonZeroU8::new(cut).unwrap_or(NonZeroU8::new(128).unwrap())
}

/// The BC1 search depth `bcenc` encodes with (Note HY17).
fn bc1_search(s: Bc1Search) -> bcenc::Bc1Search {
    match s {
        Bc1Search::Default => bcenc::Bc1Search::Default,
        Bc1Search::Fast => bcenc::Bc1Search::Fast,
    }
}

const _: () = assert!(
    bcenc::MAX_SEARCH_RADIUS == BC45_RADIUS_MAX,
    "bcenc's Max starts at the engine's radius 10"
);

/// The radius of the radius search. `Max` starts with that radius-10 search and then runs the pair search
/// (`bcenc::*_max`), so it reads 10 too; not a rank (`api::radius_rank` orders the four values).
pub fn radius(r: SearchRadius) -> u32 {
    match r {
        SearchRadius::Default => BC45_RADIUS_DEFAULT,
        SearchRadius::Slowest | SearchRadius::Max => BC45_RADIUS_MAX,
        SearchRadius::Fast => BC45_RADIUS_FAST,
    }
}

/// The bc7e tune a BC7 profile's encoder settings give (spec D5; Note LR, LR8), or `None` for the plain call:
/// - weights: Note CP: perceptual uses bc7e's own weights, so it wins over `bc7_weights`; an `alphaN` passes its
///   `[1, 1, 1, N]` (x1-form: bc7e ranks every linear call as x16 would, LR15), unset and `linear` keep the level's.
/// - `exact_constants`: the channel guard's R, G and B (`None` when none is guarded); A is `Bc7Params::exact_opaque`.
/// - `flat_tolerance`: `Some(Data)` for `data`; `default` keeps the level's T_d.
///
/// The guard and the tolerance apply under perceptual too (BC7-O16, O20): they are constraints, not weightings, and
/// the tuned entry starts from the level's perceptual params.
fn bc7_tune(e: &EncoderSpec) -> Option<Bc7Tune> {
    let mask = e.channel_guard.mask();
    let t = Bc7Tune {
        weights: e
            .bc7_weights
            .filter(|w| !e.perceptual && *w != Bc7Weights::Linear)
            .map(Bc7Weights::weights),
        exact_constants: (mask != 0).then_some(mask),
        flat_tolerance: match e.bc7_flat_tolerance {
            crate::config::Bc7FlatTolerance::Default => None,
            crate::config::Bc7FlatTolerance::Data => Some(encoders_sys::Bc7FlatTolerance::Data),
        },
    };
    (t != Bc7Tune::default()).then_some(t)
}

/// Encode one band (one block row) of an LDR image. `b` holds 16 texels per block (see
/// `gather_blocks`), `o` the matching output bytes.
///
/// BC7 blocks whose input alpha is 255 throughout (spec §5.6, §5.8):
/// - Rotated mode 4/5 blocks always get the lossless rotated-slot fix, whatever `alpha.exact_opaque`
///   and the alpha policy (fix batch 41), as the guarantee behind `remove`. Before bc7e patch 0005 bc7e could decode
///   such a block's alpha well below 254 (239 on white noise); since 0005 (Q2t) the four probe inputs give none (0 of
///   1210 to 2310 rotated blocks at veryslow, basic, slowest and max on SSE2, AVX2 and AVX-512; Note LR6).
/// - `alpha.exact_opaque`, the channel guard's A (bc7e's exact-constants mask bit 3, Note LR, LR17), reaches these
///   blocks only through mode 6, whose p-bits it forces so alpha decodes 255: the part that costs RGB. With it off,
///   mode 6 is the only source of alpha 254.
///
/// In blocks with alpha, A keeps a subset whose alpha is 0 or 255 throughout exact wherever bc7e finds such a
/// solution, above any guarded R, G or B ([`bc7_tune`]).
///
/// A complex `_m` written with alpha 0 (`complex_material`, parallax off, alpha kept: `ops::complex_alpha_zero`) takes A
/// whatever `alpha.exact_opaque` says (BC7-O29 (e2), Note LR, LR18): the file has no opaque block, so A only ranks
/// alpha in its alpha blocks, and bc7e keeps alpha 0 itself. The exact-0 fix after the encode then rewrites only a
/// block whose alpha still decodes above 0 ([`bc7_opaque_fix`]). The route reads the profile only, so it holds for a
/// parallax-off profile; a parallax-on `_m` whose source has no alpha channel (alpha 0 written, a policy warning) keeps
/// the profile's A and has no fix, as before.
fn encode_band(b: &[[u8; 4]], o: &mut [u8], p: &Profile) {
    let e = &p.encoder;
    match p.format {
        Format::Bc7 => {
            // The channel guard's A (Note LR, LR17), or the alpha-0 `_m` route.
            // shortcut: reads the profile, not the source, so a parallax-on `_m` whose source has no alpha channel
            // (alpha 0 written, AlphaParallaxNoAlpha) keeps the profile's A and gets no fix; thread `has_alpha` through
            // `EncodeCtx` to cover it (BC7-C36). The pipeline always resolves `alpha.parallax` for an `_m` (policy sets it
            // from the analysis unless explicit), so `zero_fix`'s `!= Some(true)` and this `== Some(false)` agree there.
            let params = Bc7Params {
                level: bc7_level(e.effort),
                perceptual: e.perceptual,
                exact_opaque: p.alpha.exact_opaque || crate::ops::complex_alpha_zero(p, true),
            };
            // Resolved once per band, before the 64-block calls.
            let tune = bc7_tune(e);
            for (oc, bc) in o
                .chunks_mut(BC7_BLOCKS_PER_CALL * BC7_BLOCK_BYTES)
                .zip(b.chunks(BC7_BLOCKS_PER_CALL * BLOCK_TEXELS))
            {
                match tune {
                    Some(t) => bc7e_compress_blocks_tuned(bc, oc, params, t),
                    None => bc7e_compress_blocks(bc, oc, params),
                }
            }
            bc7_opaque_fix(o, b, zero_fix(p));
        }
        f => {
            let bs = f.block_bytes().expect("block format");
            let max = e.bc45_search_radius == SearchRadius::Max;
            let bc1s = bc1_search(e.bc1_search);
            // A closure: `alpha_cut` asserts a cut of 1 or more, which only BC3 needs.
            let cut = || bc3_cut(alpha_cut(&p.alpha));
            for (blk, src) in o.chunks_mut(bs).zip(b.chunks(BLOCK_TEXELS)) {
                let src: &[[u8; 4]; 16] = src.try_into().unwrap();
                match f {
                    // Note FM: with `bc1_alpha` quantise cut the alpha to 0/255; texels below 128 go transparent.
                    // Without it rgbcx ignores alpha.
                    Format::Bc1 if e.bc1_alpha => {
                        // Precondition: `quantize::to_encoder_input` already cut the alpha, so 128 only splits 0 from 255.
                        debug_assert!(
                            src.iter().all(|t| matches!(t[3], 0 | 255)),
                            "BC1 1-bit alpha input not cut to 0/255"
                        );
                        let mask = src
                            .iter()
                            .enumerate()
                            .fold(0u16, |m, (i, t)| m | (u16::from(t[3] < 128) << i));
                        blk.copy_from_slice(&bcenc::bc1_encode_block_alpha_with(
                            src,
                            mask,
                            e.perceptual,
                            bc1s,
                        ))
                    }
                    // FM42: `perceptual` weights the BC1 colour error by Rec.709 luma (BC3's colour block too).
                    Format::Bc1 => blk.copy_from_slice(&bcenc::bc1_encode_block_with(
                        src,
                        true,
                        e.perceptual,
                        bc1s,
                    )),
                    Format::Bc3 if max => blk.copy_from_slice(&bcenc::bc3_encode_block_max_with(
                        src,
                        cut(),
                        e.perceptual,
                        bc1s,
                    )),
                    Format::Bc3 => blk.copy_from_slice(&bcenc::bc3_encode_block_with(
                        src,
                        radius(e.bc45_search_radius),
                        cut(),
                        e.perceptual,
                        bc1s,
                    )),
                    // Every single-channel sub-type (envmask, height, msn_specular) keeps its map in R.
                    Format::Bc4 if max => blk.copy_from_slice(&bcenc::bc4_encode_block_max(src, 0)),
                    Format::Bc4 => blk.copy_from_slice(&bcenc::bc4_encode_block(
                        src,
                        0,
                        radius(e.bc45_search_radius),
                    )),
                    Format::Bc5 if max => blk.copy_from_slice(&bcenc::bc5_encode_block_max(src)),
                    Format::Bc5 => blk.copy_from_slice(&bcenc::bc5_encode_block(
                        src,
                        radius(e.bc45_search_radius),
                    )),
                    Format::Bc7
                    | Format::Bc6hUf16
                    | Format::Rgba8
                    | Format::Rgb8
                    | Format::R8
                    | Format::Rg8
                    | Format::Rgba16f => unreachable!("handled elsewhere"),
                }
            }
        }
    }
}

/// The exact-0 alpha fix is for the complex `_m` written with alpha 0 only; parallax on keeps the source alpha as
/// data, byte for byte as before (DECISIONS AH1).
fn zero_fix(p: &Profile) -> bool {
    p.subtype == Subtype::ComplexMaterial && p.alpha.parallax != Some(true)
}

/// The BC7 opaque fix-ups on blocks `o` whose source texels `b` (16 per
/// block) have alpha 255 throughout: the lossless rotated mode 4/5 fix. With `zero`, blocks whose source alpha is 0
/// throughout get the exact-0 fix (DECISIONS AH1: bc7e's mode-6 p-bit can decode them as 1), which rewrites a block
/// only if its alpha decodes above 0 at some texel (the gate in `force_zero_alpha`, Note LR, LR18). It runs once per
/// block of every mip, and only an all-0 source block pays for the gate's decode.
fn bc7_opaque_fix(o: &mut [u8], b: &[[u8; 4]], zero: bool) {
    for (blk, src) in o.chunks_mut(BC7_BLOCK_BYTES).zip(b.chunks(BLOCK_TEXELS)) {
        if zero && src.iter().all(|t| t[3] == 0) {
            force_zero_alpha(blk.try_into().unwrap());
        } else if src.iter().all(|t| t[3] == 255) {
            let blk: &mut [u8; 16] = blk.try_into().unwrap();
            // Rotated mode 4/5 carry alpha in a colour endpoint pair, which the native flag does not cover.
            // Lossless for RGB, so it runs with the flag off.
            force_opaque_rotated(blk);
        }
    }
}

/// Uncompressed level bytes (DECISIONS S4): `rgba8` as B8G8R8A8, `rgb8` as B8G8R8 (Note FM38), `r8` as R,
/// `rg8` as R then G, `rgba16f` as four little-endian halves per texel. Rows are tightly packed (pitch
/// `w * bytes_per_pixel`).
pub(crate) fn pack_uncompressed(input: &EncoderInput, format: Format) -> Vec<u8> {
    match (input, format) {
        (EncoderInput::Ldr(px), Format::Rgba8) => {
            px.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()
        }
        (EncoderInput::Ldr(px), Format::Rgb8) => {
            px.iter().flat_map(|p| [p[2], p[1], p[0]]).collect()
        }
        (EncoderInput::Ldr(px), Format::R8) => px.iter().map(|p| p[0]).collect(),
        (EncoderInput::Ldr(px), Format::Rg8) => px.iter().flat_map(|p| [p[0], p[1]]).collect(),
        (EncoderInput::Hdr(px), Format::Rgba16f) => px
            .iter()
            .flat_map(|p| p.iter().flat_map(|h| h.to_le_bytes()))
            .collect(),
        _ => unreachable!("encode_level_with checks the input kind against the format"),
    }
}

/// Pad a row-major half image to a multiple of 4 in both directions by edge replication.
/// Borrows `px` when both dimensions are already multiples of 4.
fn pad_hdr(px: &[[u16; 4]], w: u32, h: u32) -> (Cow<'_, [[u16; 4]]>, u32, u32) {
    assert!(w > 0 && h > 0, "empty image");
    assert_eq!(
        px.len(),
        w as usize * h as usize,
        "pixel count does not match width*height"
    );
    let (pw, ph) = (w.div_ceil(4) * 4, h.div_ceil(4) * 4);
    if (pw, ph) == (w, h) {
        return (Cow::Borrowed(px), w, h);
    }
    let mut out = Vec::with_capacity(pw as usize * ph as usize);
    for y in 0..ph {
        for x in 0..pw {
            out.push(px[(y.min(h - 1) as usize) * w as usize + x.min(w - 1) as usize]);
        }
    }
    (Cow::Owned(out), pw, ph)
}

/// Compress one `w` x `h` level to `p.format`: blocks in raster order,
/// `ceil(w/4) * ceil(h/4) * block_bytes` bytes. Levels smaller than a block (2x2, 1x1, or a
/// non-multiple-of-4 edge) are padded to whole 4x4 blocks by edge replication (`gather_blocks`
/// for LDR, `pad_hdr` for BC6H). Cancellation is checked before each band and once more at the end.
///
/// Panics if `input` does not match `p.format` (HDR input exactly for BC6H and RGBA16F; uncompressed
/// formats are packed, not encoded) or `w * h` pixels. Both are programmer-error invariants, not
/// user-reachable: `quantize::to_encoder_input` yields HDR input iff the format is HDR, and the level's
/// pixel count comes from the mip chain.
pub fn encode_level(
    input: &EncoderInput,
    w: u32,
    h: u32,
    p: &Profile,
    ctx: &EncodeCtx,
) -> Result<Vec<u8>, Cancelled> {
    let cancelled = || {
        ctx.cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
    };
    encode_level_with(input, w, h, p, &cancelled)
}

/// `encode_level` with the cancellation probe as a closure (called before each band and at the end).
fn encode_level_with(
    input: &EncoderInput,
    w: u32,
    h: u32,
    p: &Profile,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u8>, Cancelled> {
    assert_eq!(
        matches!(input, EncoderInput::Hdr(_)),
        p.format.is_hdr(),
        "encoder input kind does not match format {:?}",
        p.format
    );
    if !p.format.is_block() {
        let n = match input {
            EncoderInput::Ldr(v) => v.len(),
            EncoderInput::Hdr(v) => v.len(),
        };
        assert_eq!(
            n,
            w as usize * h as usize,
            "pixel count does not match width*height"
        );
        return if cancelled() {
            Err(Cancelled)
        } else {
            Ok(pack_uncompressed(input, p.format))
        };
    }
    // Set when any band is skipped, so its zero-filled output can never be returned, whatever the
    // probe reads at the end (a shared flag may be cleared by another holder).
    let skipped = AtomicBool::new(false);
    let go = || {
        let stop = cancelled();
        if stop {
            skipped.store(true, Ordering::Relaxed);
        }
        !stop
    };
    let out = match input {
        EncoderInput::Ldr(px) => {
            let blocks = gather_blocks(px, w, h);
            let (bw, bs) = (
                w.div_ceil(4) as usize,
                p.format.block_bytes().expect("block format"),
            );
            let mut out = vec![0u8; blocks.len() / BLOCK_TEXELS * bs];
            out.par_chunks_mut(bw * bs)
                .zip(blocks.par_chunks(bw * BLOCK_TEXELS))
                .for_each(|(o, b)| {
                    if go() {
                        encode_band(b, o, p);
                    }
                });
            if p.subtype == Subtype::ComplexGrass
                && p.format == Format::Bc7
                && !skipped.load(Ordering::Relaxed)
            {
                guard_grass_corner(&mut out, &blocks, w, h, p);
            }
            out
        }
        EncoderInput::Hdr(px) => {
            let (padded, pw, ph) = pad_hdr(px, w, h);
            let level = bc6h_level(p.encoder.effort);
            let mut out = vec![0u8; (pw / 4 * (ph / 4) * 16) as usize];
            out.par_chunks_mut((pw / 4 * 16) as usize)
                .zip(padded.par_chunks((pw * 4) as usize))
                .for_each(|(o, rows)| {
                    if go() {
                        bc6h_compress_rows(rows, pw, 4, o, level);
                    }
                });
            out
        }
    };
    if skipped.load(Ordering::Relaxed) || cancelled() {
        Err(Cancelled)
    } else {
        Ok(out)
    }
}

/// Design §3.6 (ruling R8): the decoded bottom-left texel of a complex-grass level must stay a unit vector within
/// `CS_GRASS_THRESHOLD - GRASS_GUARD_MARGIN`; when it does not, that one block is encoded again at bc7e's slowest
/// level (with the same opaque fix-ups). `validate` reports `VAL-GRASS-DETECT` when even that fails.
fn guard_grass_corner(out: &mut [u8], blocks: &[[u8; 4]], w: u32, h: u32, p: &Profile) {
    let i = (h.div_ceil(4) as usize - 1) * w.div_ceil(4) as usize; // column 0 of the last block row
    let texel = ((h - 1) % 4) as usize * 4; // x = 0 inside that block
    let blk = &mut out[i * BC7_BLOCK_BYTES..(i + 1) * BC7_BLOCK_BYTES];
    if grass_corner_ok(blk, texel) {
        return;
    }
    let mut best = p.clone();
    best.encoder.effort = Effort::Slowest;
    encode_band(
        &blocks[i * BLOCK_TEXELS..(i + 1) * BLOCK_TEXELS],
        blk,
        &best,
    );
}

/// Whether texel `t` (0..16, row-major) of BC7 block `blk` decodes within the complex-grass guard.
fn grass_corner_ok(blk: &[u8], t: usize) -> bool {
    let mut d = [0u8; 64];
    bcdec_rs::bc7(blk, &mut d, 16);
    let q = [d[t * 4], d[t * 4 + 1], d[t * 4 + 2]].map(|c| f32::from(c) / 255.0);
    unit_err(q) < CS_GRASS_THRESHOLD - GRASS_GUARD_MARGIN
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::PresetRegistry;

    #[test]
    fn bc3_cut_keeps_every_cut_and_reads_0_as_128() {
        for cut in 1..=255u8 {
            assert_eq!(bc3_cut(cut).get(), cut);
        }
        assert_eq!(bc3_cut(0).get(), 128);
    }

    #[test]
    fn alpha_cut_is_the_least_passing_alpha() {
        let spec = |threshold, compare| AlphaSpec {
            policy: AlphaPolicy::Cutout,
            threshold,
            compare,
            exact_opaque: false,
            parallax: None,
            exr_straight: false,
        };
        for (t, c, want) in [
            (127, AlphaCompare::Gt, 128),
            (200, AlphaCompare::Gt, 201),
            (254, AlphaCompare::Gt, 255),
            (1, AlphaCompare::Ge, 1),
            (200, AlphaCompare::Ge, 200),
        ] {
            assert_eq!(alpha_cut(&spec(t, c)), want, "{t} {c:?}");
        }
    }

    fn bc7_exact_profile() -> Profile {
        let mut p = PresetRegistry::builtin()
            .unwrap()
            .profile("cs-quality", Target::Cs, Subtype::ComplexMaterial)
            .unwrap();
        p.format = Format::Bc7;
        p.alpha.exact_opaque = true;
        p
    }

    /// A band skipped by cancellation must fail the level even if the probe reads false again
    /// afterwards (e.g. a shared flag reset by another holder).
    #[test]
    fn skipped_band_fails_even_if_cancel_flag_clears() {
        use std::sync::atomic::AtomicUsize;
        let px: Vec<[u8; 4]> = (0..16 * 16u32).map(|i| [i as u8, 7, 9, 255]).collect();
        let input = EncoderInput::Ldr(px);
        for fmt in [Format::Bc7, Format::Bc1] {
            let mut p = bc7_exact_profile();
            p.format = fmt;
            let calls = AtomicUsize::new(0);
            let first_only = || calls.fetch_add(1, Ordering::Relaxed) == 0;
            assert_eq!(
                encode_level_with(&input, 16, 16, &p, &first_only),
                Err(Cancelled),
                "{fmt:?}"
            );
        }
        let calls = AtomicUsize::new(0);
        let first_only = || calls.fetch_add(1, Ordering::Relaxed) == 0;
        let hdr = EncoderInput::Hdr(vec![[0x3C00; 4]; 8 * 8]);
        let mut p = bc7_exact_profile();
        p.format = Format::Bc6hUf16;
        assert_eq!(
            encode_level_with(&hdr, 8, 8, &p, &first_only),
            Err(Cancelled)
        );
    }

    #[test]
    fn pad_hdr_borrows_unless_padding_is_needed() {
        let px = vec![[0x3C00u16; 4]; 8 * 4];
        assert!(matches!(pad_hdr(&px, 8, 4), (Cow::Borrowed(_), 8, 4)));
        let px = vec![[0x3C00u16; 4]; 6 * 5];
        let (padded, pw, ph) = pad_hdr(&px, 6, 5);
        assert!(matches!(padded, Cow::Owned(_)));
        assert_eq!((padded.len(), pw, ph), (8 * 8, 8, 8));
    }

    #[test]
    fn uncompressed_levels_are_packed_as_their_dxgi_layout() {
        let mut p = bc7_exact_profile();
        let px = vec![[10u8, 20, 30, 40], [50, 60, 70, 80]];
        for (f, want) in [
            (Format::Rgba8, vec![30u8, 20, 10, 40, 70, 60, 50, 80]),
            // Note FM38: B, G, R with no alpha byte.
            (Format::Rgb8, vec![30, 20, 10, 70, 60, 50]),
            (Format::R8, vec![10, 50]),
            (Format::Rg8, vec![10, 20, 50, 60]),
        ] {
            p.format = f;
            assert_eq!(
                encode_level(
                    &EncoderInput::Ldr(px.clone()),
                    2,
                    1,
                    &p,
                    &EncodeCtx::default()
                )
                .unwrap(),
                want,
                "{f:?}"
            );
        }
        p.format = Format::Rgba16f;
        let h = [half::f16::from_f32(2.0).to_bits(), 0, 0x3c00, 0x3800];
        let out =
            encode_level(&EncoderInput::Hdr(vec![h]), 1, 1, &p, &EncodeCtx::default()).unwrap();
        assert_eq!(
            out,
            h.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()
        );
    }

    /// Design §3.6: a corner block that decodes off unit length is encoded again from its own texels.
    #[test]
    fn grass_guard_reencodes_a_failing_corner_block() {
        let p = PresetRegistry::builtin()
            .unwrap()
            .profile("cs-quality", Target::Cs, Subtype::ComplexGrass)
            .unwrap();
        // 4 x 8: top block = opaque colour, bottom block = flat +Z normal with specular 12.
        let px: Vec<[u8; 4]> = (0..32)
            .map(|i| {
                if i < 16 {
                    [200, 180, 90, 255]
                } else {
                    [128, 128, 255, 12]
                }
            })
            .collect();
        let blocks = gather_blocks(&px, 4, 8);
        let mut out = vec![0u8; 32];
        encode_band(&blocks, &mut out, &p);
        let top = out[..16].to_vec();
        assert!(
            grass_corner_ok(&out[16..], 12),
            "a flat normal encodes within the guard"
        );
        // A red block in the corner's place decodes to (1, -1, -1): far from unit length.
        let red = gather_blocks(&[[255, 0, 0, 12]; 16], 4, 4);
        encode_band(&red, &mut out[16..], &p);
        assert!(!grass_corner_ok(&out[16..], 12));
        guard_grass_corner(&mut out, &blocks, 4, 8, &p);
        assert!(grass_corner_ok(&out[16..], 12));
        assert_eq!(out[..16], top[..], "the top block is left alone");
    }

    /// A 64 x 64 image of noise on top and a gradient below, as the encoder's blocks. With `holes`, the texels of
    /// rows 0..32 with `(x + 2 y) % 5 == 0` are transparent (alpha 0, the rest 255): every noise block then holds
    /// some transparent texels, and the gradient blocks stay opaque.
    fn search_px(holes: bool) -> Vec<[u8; 4]> {
        let mut s = 0x9E37_79B9u32;
        let mut next = move || {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            (s >> 24) as u8
        };
        let px: Vec<[u8; 4]> = (0..64u32 * 64)
            .map(|i| {
                let (x, y) = (i % 64, i / 64);
                let a = if holes && y < 32 && (x + 2 * y) % 5 == 0 {
                    0
                } else {
                    255
                };
                if y < 32 {
                    [next(), next(), next(), a]
                } else {
                    [(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8, a]
                }
            })
            .collect();
        px
    }

    fn search_blocks(holes: bool) -> Vec<[u8; 4]> {
        gather_blocks(&search_px(holes), 64, 64)
    }

    fn search_profile(format: Format, search: Bc1Search) -> Profile {
        let mut p = bc7_exact_profile();
        p.format = format;
        p.encoder.perceptual = true;
        p.encoder.bc1_search = search;
        p
    }

    fn encode_blocks(b: &[[u8; 4]], p: &Profile) -> Vec<u8> {
        let mut out = vec![0u8; b.len() / BLOCK_TEXELS * p.format.block_bytes().unwrap()];
        encode_band(b, &mut out, p);
        out
    }

    fn texels(blk: &[[u8; 4]]) -> &[[u8; 4]; 16] {
        blk.try_into().unwrap()
    }

    /// The transparent-texel mask of a one-bit-alpha block, bit `j` for texel `j`.
    fn alpha_mask(px: &[[u8; 4]; 16]) -> u16 {
        px.iter()
            .enumerate()
            .fold(0u16, |m, (j, t)| m | (u16::from(t[3] < 128) << j))
    }

    /// The `holes` fixture holds opaque blocks and blocks with some, but not all, texels transparent.
    fn assert_mixed_masks(b: &[[u8; 4]]) {
        let masks: Vec<u16> = b
            .chunks(BLOCK_TEXELS)
            .map(|c| alpha_mask(texels(c)))
            .collect();
        assert!(masks.contains(&0), "no opaque block");
        assert!(
            masks.iter().filter(|&&m| m != 0 && m != 0xFFFF).count() > masks.len() / 4,
            "too few mixed blocks"
        );
    }

    #[test]
    fn default_bc1_search_bytes_are_unchanged() {
        let b = search_blocks(false);
        let bh = search_blocks(true);
        assert_mixed_masks(&bh);
        let p3 = search_profile(Format::Bc3, Bc1Search::Default);
        let cut = bc3_cut(alpha_cut(&p3.alpha));
        let rad = radius(p3.encoder.bc45_search_radius);
        let bc1 = encode_blocks(&b, &search_profile(Format::Bc1, Bc1Search::Default));
        let bc3 = encode_blocks(&b, &p3);
        let mut p1a = search_profile(Format::Bc1, Bc1Search::Default);
        p1a.encoder.bc1_alpha = true;
        let bc1a = encode_blocks(&bh, &p1a);
        for (i, blk) in b.chunks(BLOCK_TEXELS).enumerate() {
            let px = texels(blk);
            assert_eq!(
                bc1[i * 8..][..8],
                encoders_sys::bc1_encode_block(px, true, false, true),
                "bc1 {i}"
            );
            assert_eq!(
                bc3[i * 16..][..16],
                encoders_sys::bc3_encode_block(px, rad, cut.get(), true),
                "bc3 {i}"
            );
        }
        for (i, blk) in bh.chunks(BLOCK_TEXELS).enumerate() {
            let px = texels(blk);
            let mask = alpha_mask(px);
            assert_eq!(
                bc1a[i * 8..][..8],
                encoders_sys::bc1_encode_block_alpha(px, mask, true),
                "bc1 alpha {i} mask {mask:#06x}"
            );
        }
    }

    #[test]
    fn fast_bc1_and_bc3_are_rgbcx_at_the_fast_level() {
        let b = search_blocks(false);
        let lvl = bcenc::BC1_FAST_LEVEL;
        let p3 = search_profile(Format::Bc3, Bc1Search::Fast);
        let cut = bc3_cut(alpha_cut(&p3.alpha));
        let rad = radius(p3.encoder.bc45_search_radius);
        let bc1 = encode_blocks(&b, &search_profile(Format::Bc1, Bc1Search::Fast));
        let bc3 = encode_blocks(&b, &p3);
        let dflt = encode_blocks(&b, &search_profile(Format::Bc1, Bc1Search::Default));
        assert_ne!(bc1, dflt, "the fixture must tell the levels apart");
        for (i, blk) in b.chunks(BLOCK_TEXELS).enumerate() {
            let px = texels(blk);
            assert_eq!(
                bc1[i * 8..][..8],
                encoders_sys::bc1_encode_block_level(px, lvl, true, true),
                "bc1 {i}"
            );
            assert_eq!(
                bc3[i * 16..][..16],
                encoders_sys::bc3_encode_block_level(px, rad, cut.get(), lvl, true),
                "bc3 {i}"
            );
        }
    }

    #[test]
    fn fast_one_bit_alpha_is_rgbcx_at_the_fast_level() {
        let b = search_blocks(true);
        assert_mixed_masks(&b);
        let mut p = search_profile(Format::Bc1, Bc1Search::Fast);
        p.encoder.bc1_alpha = true;
        let out = encode_blocks(&b, &p);
        let mut d = p.clone();
        d.encoder.bc1_search = Bc1Search::Default;
        let dflt = encode_blocks(&b, &d);
        assert_ne!(out, dflt, "the fixture must tell the levels apart");
        for (i, blk) in b.chunks(BLOCK_TEXELS).enumerate() {
            let px = texels(blk);
            let mask = alpha_mask(px);
            assert_eq!(
                out[i * 8..][..8],
                encoders_sys::bc1_encode_block_alpha_level(px, mask, bcenc::BC1_FAST_LEVEL, true),
                "block {i} mask {mask:#06x}"
            );
        }
    }

    #[test]
    fn bc1_search_changes_nothing_on_other_formats() {
        let input = EncoderInput::Ldr(search_px(false));
        let enc = |f, s| {
            encode_level(&input, 64, 64, &search_profile(f, s), &EncodeCtx::default()).unwrap()
        };
        for f in [Format::Bc7, Format::Bc4, Format::Bc5, Format::Rgba8] {
            assert_eq!(enc(f, Bc1Search::Default), enc(f, Bc1Search::Fast), "{f:?}");
        }
    }
}
