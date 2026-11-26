use crate::target::{forced, ispc_target, IspcTarget};
#[cfg(feature = "test-hooks")]
use crate::target::{shim_number, ShimRoute};
use crate::{ffi, init};

/// ISPC TC BC6H effort levels. The discriminants index `g_bc6` in the shim, which has no range check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bc6hLevel {
    Veryfast = 0,
    Fast = 1,
    Basic = 2,
    Slow = 3,
    Veryslow = 4,
    // Veryslow plus the peak-preserving mode (kernel.ispc sdc patch 0001): in a block whose brightest texel is at
    // least 32x the block's geometric-mean luminance, candidates that decode that texel below 0.9 of its source
    // luminance rank last, and a second search on a lifted block competes. Other blocks encode as Veryslow.
    // The engine's Slowest and Max efforts map here; Slowest is the BC6H default unless a preset, config, rule or
    // `--set` names an effort (Notes FF and FM43).
    VeryslowPeak = 5,
}

/// BC6H UF16 compression of a strip of `rows` rows (a multiple of 4) of a `width`-wide RGBA16F image.
/// Pixels are row-major raw IEEE half bits; output blocks are in raster order.
/// Panics when `rows * width * 8` exceeds `i32::MAX` (2 GiB per strip).
pub fn bc6h_compress_rows(
    rgba_f16: &[[u16; 4]],
    width: u32,
    rows: u32,
    out: &mut [u8],
    level: Bc6hLevel,
) {
    bc6h_compress_rows_on(rgba_f16, width, rows, out, level, ispc_target());
}

/// Whether the kernel's 32-bit byte offsets fit for a strip. The input offset
/// `(rows - 1) * stride + 8 * (width - 1) + 4` dominates the output offset
/// `(rows / 4 - 1) * width * 4 + 15`, so one product covers both.
fn strip_offsets_fit_i32(width: u32, rows: u32) -> bool {
    (rows as u64) * (width as u64 * 8) <= i32::MAX as u64
}

/// `bc6h_compress_rows` on one ISPC code path instead of the CPU dispatch. Every plain encode goes through it with `ispc_target()`.
/// Panics when the CPU cannot run `target`, and when `rows * width * 8` exceeds `i32::MAX` (2 GiB per strip).
pub fn bc6h_compress_rows_on(
    rgba_f16: &[[u16; 4]],
    width: u32,
    rows: u32,
    out: &mut [u8],
    level: Bc6hLevel,
    target: IspcTarget,
) {
    assert!(
        target.supported(),
        "this CPU cannot run the {target:?} BC6H path"
    );
    compress_rows(rgba_f16, width, rows, out, level, forced(target));
}

/// Test only. `bc6h_compress_rows` through `route`, whatever the CPU's best target is. `Direct(best())` re-runs that
/// object's table init, so call it only while no other thread encodes.
#[cfg(feature = "test-hooks")]
pub fn bc6h_compress_rows_routed(
    rgba_f16: &[[u16; 4]],
    width: u32,
    rows: u32,
    out: &mut [u8],
    level: Bc6hLevel,
    route: ShimRoute,
) {
    compress_rows(rgba_f16, width, rows, out, level, shim_number(route));
}

/// The shim number is already validated: by `forced` after `supported()`, or by `shim_number`.
fn compress_rows(
    rgba_f16: &[[u16; 4]],
    width: u32,
    rows: u32,
    out: &mut [u8],
    level: Bc6hLevel,
    shim: i32,
) {
    assert!(
        width > 0 && width.is_multiple_of(4) && rows.is_multiple_of(4),
        "width and rows must be a multiple of 4"
    );
    let (w, r) = (width as usize, rows as usize);
    assert_eq!(
        rgba_f16.len(),
        w * r,
        "pixel count does not match width*rows"
    );
    assert_eq!(
        out.len(),
        (w / 4) * (r / 4) * 16,
        "out must hold 16 bytes per block"
    );
    if rows == 0 {
        return;
    }
    // Before the conversions below: with `rows, width >= 1` this bound implies each of them fits.
    assert!(
        strip_offsets_fit_i32(width, rows),
        "strip exceeds the kernel's 32-bit offsets: pass fewer rows"
    );
    let stride = i32::try_from(w * 8).expect("row stride exceeds i32");
    let rows_i = i32::try_from(rows).expect("rows exceed i32");
    let width_i = i32::try_from(width).expect("width exceeds i32");
    init();
    // SAFETY: `rgba_f16` holds `width * rows` RGBA16F texels (8 bytes each) and `stride` is the
    // row pitch in bytes, so the shim reads only within the slice; the byte offsets the kernel
    // forms in 32-bit `int` (`(yy * 4 + y) * stride`, `yy * width * data_size`) fit, by the
    // assert above; `out` holds `(width/4)*(rows/4)` 16-byte blocks, the number ISPC TC writes;
    // `level` comes from the closed `Bc6hLevel` enum (0..=5), which the shim indexes without a
    // range check; a forced `target` was checked by `bc6h_compress_rows_on` or `shim_number`.
    unsafe {
        ffi::sdc_bc6h_compress(
            rgba_f16.as_ptr() as *const u16,
            width_i,
            rows_i,
            stride,
            out.as_mut_ptr(),
            level as i32,
            shim,
        )
    };
}

#[cfg(test)]
mod tests {
    use super::strip_offsets_fit_i32;

    #[test]
    fn a_four_row_strip_fits() {
        assert!(strip_offsets_fit_i32(4, 4));
    }

    #[test]
    fn a_very_tall_narrow_strip_does_not_fit() {
        // (1 << 26) rows * 32 bytes = 2 GiB
        assert!(!strip_offsets_fit_i32(4, 1 << 26));
    }

    #[test]
    fn two_gib_total_does_not_fit() {
        // stride 8 MiB: 256 rows = 2 GiB
        assert!(!strip_offsets_fit_i32(1 << 20, 256));
    }

    #[test]
    fn just_under_two_gib_fits() {
        // stride 8 MiB: 252 rows = 1.97 GiB
        assert!(strip_offsets_fit_i32(1 << 20, 252));
    }

    #[test]
    fn a_stride_that_fits_alone_is_not_enough() {
        // w * 8 = 2^30 fits i32, rows * stride = 2^32 does not
        assert!(!strip_offsets_fit_i32(1 << 27, 4));
    }
}
