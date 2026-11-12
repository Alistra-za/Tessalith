//! Push-pull dilation (spec §5.4 step 4): colour under α = 0 is filled before any filtering [MIP].
//!
//! Texels whose alpha rounds to 8-bit code 0 carry no meaningful colour, but mip filtering and
//! bilinear sampling would otherwise blend that garbage into visible neighbours. They are given
//! colour pulled from a push-pull pyramid built only from visible texels. Alpha is never touched.
use crate::image::Rgba32F;

/// One coarser pyramid level; `rgb[i]` is meaningful only where `filled[i]`.
struct Level {
    w: usize,
    h: usize,
    rgb: Vec<[f32; 3]>,
    filled: Vec<bool>,
}

/// A texel is visible when its alpha rounds to 8-bit code 1 or more. NaN and negative alpha are invisible.
fn visible(a: f32) -> bool {
    (a * 255.0).round() >= 1.0
}

/// Mean RGB of the filled texels among the (up to) 2x2 children of coarse texel `(x, y)`.
/// Children beyond the edge of an odd-sized parent are skipped, so every parent texel is
/// covered exactly once and no source texel is dropped.
fn push_texel(
    x: usize,
    y: usize,
    pw: usize,
    ph: usize,
    child: impl Fn(usize) -> Option<[f32; 3]>,
) -> Option<[f32; 3]> {
    let (mut sum, mut n) = ([0.0f32; 3], 0.0f32);
    for sy in (y * 2)..(y * 2 + 2).min(ph) {
        for sx in (x * 2)..(x * 2 + 2).min(pw) {
            if let Some(c) = child(sy * pw + sx) {
                for (s, v) in sum.iter_mut().zip(c) {
                    *s += v;
                }
                n += 1.0;
            }
        }
    }
    (n > 0.0).then(|| sum.map(|v| v / n))
}

/// Replace the RGB of every texel whose alpha rounds to code 0 with colour from the nearest
/// coarser level of a push-pull pyramid built from visible texels. Alpha is never changed.
///
/// An image with no visible texel (including an empty one) is left unchanged: there is no
/// colour to propagate, and inventing one would be a guess. Any size is accepted, including
/// non-power-of-two and 1x1.
///
/// Memory: level 0 is read and written in place in `img`; only the coarser levels (about a
/// third of the image, 13 bytes per texel) are allocated.
pub fn dilate_under_zero_alpha(img: &mut Rgba32F) {
    if !img.px.iter().any(|p| visible(p[3])) {
        return;
    }
    let (w0, h0) = (img.width as usize, img.height as usize);

    // push: mean of the visible children, down to 1x1
    let mut levels: Vec<Level> = Vec::new();
    let (mut pw, mut ph) = (w0, h0);
    while pw > 1 || ph > 1 {
        let (w, h) = (pw.div_ceil(2), ph.div_ceil(2));
        let mut rgb = vec![[0.0f32; 3]; w * h];
        let mut filled = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let c = match levels.last() {
                    None => push_texel(x, y, pw, ph, |i| {
                        let p = img.px[i];
                        visible(p[3]).then_some([p[0], p[1], p[2]])
                    }),
                    Some(prev) => {
                        push_texel(x, y, pw, ph, |i| prev.filled[i].then_some(prev.rgb[i]))
                    }
                };
                if let Some(c) = c {
                    rgb[y * w + x] = c;
                    filled[y * w + x] = true;
                }
            }
        }
        levels.push(Level { w, h, rgb, filled });
        (pw, ph) = (w, h);
    }

    // pull: fill empty texels from the parent, coarse to fine
    // (the 1x1 top is filled because at least one texel is visible)
    for l in (0..levels.len().saturating_sub(1)).rev() {
        let (lo, hi) = levels.split_at_mut(l + 1);
        let (cur, parent) = (&mut lo[l], &hi[0]);
        for y in 0..cur.h {
            for x in 0..cur.w {
                let i = y * cur.w + x;
                if !cur.filled[i] {
                    cur.rgb[i] = parent.rgb[(y / 2) * parent.w + x / 2];
                    cur.filled[i] = true;
                }
            }
        }
    }
    // A 1x1 image has no pyramid: its single texel is visible here, so there is nothing to fill.
    if let Some(first) = levels.first() {
        for y in 0..h0 {
            for x in 0..w0 {
                let p = &mut img.px[y * w0 + x];
                if !visible(p[3]) {
                    let c = first.rgb[(y / 2) * first.w + x / 2];
                    p[..3].copy_from_slice(&c);
                }
            }
        }
    }
}

/// One pyramid level of `push_pull_fill`: all four channels, `filled[i]` marks meaningful texels.
struct Level4 {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
    filled: Vec<bool>,
}

/// Bilinear sample of a fully filled level at fractional texel coordinates, clamped to its edges.
fn sample_bilinear(l: &Level4, fx: f32, fy: f32) -> [f32; 4] {
    let (x0f, y0f) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0f, fy - y0f);
    let cl = |v: f32, n: usize| (v as isize).clamp(0, n as isize - 1) as usize;
    let (xa, xb, ya, yb) = (
        cl(x0f, l.w),
        cl(x0f + 1.0, l.w),
        cl(y0f, l.h),
        cl(y0f + 1.0, l.h),
    );
    let mut out = [0.0f32; 4];
    for (c, o) in out.iter_mut().enumerate() {
        let top = l.px[ya * l.w + xa][c] * (1.0 - tx) + l.px[ya * l.w + xb][c] * tx;
        let bot = l.px[yb * l.w + xa][c] * (1.0 - tx) + l.px[yb * l.w + xb][c] * tx;
        *o = top * (1.0 - ty) + bot * ty;
    }
    out
}

/// Fill every texel where `valid` is false (all four channels) by masked push-pull with a
/// smooth pull: each coarser level holds the mean of its valid 2x2 children (children past an
/// odd edge are skipped), and an unfilled texel takes the bilinear sample of its parent level at
/// `((x + 0.5) / 2 - 0.5, (y + 0.5) / 2 - 0.5)`, so the fill varies smoothly instead of in blocks.
///
/// Valid texels are never written. Returns false, leaving `img` unchanged, when no texel is valid.
/// Panics if `valid.len()` differs from the texel count.
pub fn push_pull_fill(img: &mut Rgba32F, valid: &[bool]) -> bool {
    assert_eq!(valid.len(), img.px.len(), "valid mask size mismatch");
    if !valid.iter().any(|&v| v) {
        return false;
    }
    let (w0, h0) = (img.width as usize, img.height as usize);

    let mut levels: Vec<Level4> = Vec::new();
    let (mut pw, mut ph) = (w0, h0);
    while pw > 1 || ph > 1 {
        let (w, h) = (pw.div_ceil(2), ph.div_ceil(2));
        let mut px = vec![[0.0f32; 4]; w * h];
        let mut filled = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let (mut sum, mut n) = ([0.0f32; 4], 0.0f32);
                for sy in (y * 2)..(y * 2 + 2).min(ph) {
                    for sx in (x * 2)..(x * 2 + 2).min(pw) {
                        let i = sy * pw + sx;
                        let c = match levels.last() {
                            None => valid[i].then_some(img.px[i]),
                            Some(prev) => prev.filled[i].then_some(prev.px[i]),
                        };
                        if let Some(c) = c {
                            for (s, v) in sum.iter_mut().zip(c) {
                                *s += v;
                            }
                            n += 1.0;
                        }
                    }
                }
                if n > 0.0 {
                    px[y * w + x] = sum.map(|v| v / n);
                    filled[y * w + x] = true;
                }
            }
        }
        levels.push(Level4 { w, h, px, filled });
        (pw, ph) = (w, h);
    }

    // pull, coarse to fine; the 1x1 top is filled because a texel is valid
    let parent_xy = |v: usize| (v as f32 + 0.5) / 2.0 - 0.5;
    for l in (0..levels.len().saturating_sub(1)).rev() {
        let (lo, hi) = levels.split_at_mut(l + 1);
        let (cur, parent) = (&mut lo[l], &hi[0]);
        for y in 0..cur.h {
            for x in 0..cur.w {
                let i = y * cur.w + x;
                if !cur.filled[i] {
                    cur.px[i] = sample_bilinear(parent, parent_xy(x), parent_xy(y));
                    cur.filled[i] = true;
                }
            }
        }
    }
    if let Some(first) = levels.first() {
        for y in 0..h0 {
            for x in 0..w0 {
                if !valid[y * w0 + x] {
                    img.px[y * w0 + x] = sample_bilinear(first, parent_xy(x), parent_xy(y));
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask_where(img: &Rgba32F, f: impl Fn(usize, usize) -> bool) -> Vec<bool> {
        (0..img.height as usize)
            .flat_map(|y| (0..img.width as usize).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y))
            .collect()
    }

    #[test]
    fn push_pull_keeps_valid_bit_exact_and_fills_invalid() {
        let mut img = Rgba32F::new(4, 4, [9.0, 9.0, 9.0, 9.0]);
        *img.at_mut(0, 0) = [0.1, 0.2, 0.3, 0.7];
        *img.at_mut(3, 3) = [0.9, 0.8, 0.7, 0.1];
        let valid = mask_where(&img, |x, y| (x, y) == (0, 0) || (x, y) == (3, 3));
        let before = img.px.clone();
        assert!(push_pull_fill(&mut img, &valid));
        for (i, v) in valid.iter().enumerate() {
            if *v {
                assert_eq!(img.px[i], before[i]);
            } else {
                assert!(
                    img.px[i].iter().all(|c| c.is_finite() && *c < 1.0),
                    "{:?}",
                    img.px[i]
                );
            }
        }
    }

    #[test]
    fn push_pull_blends_all_four_channels_between_two_regions() {
        let mut img = Rgba32F::new(8, 1, [0.0; 4]);
        let (a, b) = ([0.1, 0.3, 0.5, 0.7], [0.2, 0.4, 0.6, 0.8]);
        *img.at_mut(0, 0) = a;
        *img.at_mut(7, 0) = b;
        let valid = mask_where(&img, |x, _| x == 0 || x == 7);
        assert!(push_pull_fill(&mut img, &valid));
        for c in 0..4 {
            assert!(
                img.px[0][c] < img.px[3][c] && img.px[3][c] < img.px[7][c],
                "ch {c}"
            );
            assert!(img.px[3][c] > a[c] && img.px[3][c] < b[c], "ch {c}");
        }
        // channel gaps are 0.2 at both ends, so they are 0.2 at every blend weight: no channel leaked into another
        let m = img.px[3];
        assert!(
            m.windows(2).all(|w| (w[1] - w[0] - 0.2).abs() < 1e-4),
            "{m:?}"
        );
    }

    #[test]
    fn push_pull_all_invalid_is_unchanged_and_false() {
        let mut img = Rgba32F::new(3, 2, [0.5, 0.25, 0.125, 1.0]);
        let before = img.px.clone();
        assert!(!push_pull_fill(&mut img, &[false; 6]));
        assert_eq!(img.px, before);
    }

    #[test]
    fn push_pull_one_by_one() {
        let mut img = Rgba32F::new(1, 1, [0.5, 0.5, 0.5, 0.5]);
        assert!(push_pull_fill(&mut img, &[true]));
        assert_eq!(img.px[0], [0.5; 4]);
        assert!(!push_pull_fill(&mut img, &[false]));
    }

    #[test]
    fn push_pull_odd_and_npot_sizes_fill_every_texel_finite() {
        for (w, h) in [(5, 3), (7, 1), (1, 9), (6, 5), (13, 11)] {
            let mut img = Rgba32F::new(w, h, [f32::NAN; 4]);
            *img.at_mut(w - 1, h - 1) = [0.3, 0.4, 0.5, 0.6];
            let valid = mask_where(&img, |x, y| (x, y) == (w as usize - 1, h as usize - 1));
            assert!(push_pull_fill(&mut img, &valid));
            for p in &img.px {
                assert!(p.iter().all(|c| c.is_finite()), "{w}x{h} {p:?}");
            }
        }
    }

    #[test]
    fn push_pull_single_valid_texel_fills_everything_with_its_value() {
        let mut img = Rgba32F::new(9, 6, [0.0; 4]);
        *img.at_mut(4, 2) = [0.2, 0.4, 0.6, 0.8];
        let valid = mask_where(&img, |x, y| (x, y) == (4, 2));
        assert!(push_pull_fill(&mut img, &valid));
        for p in &img.px {
            for (c, e) in p.iter().zip([0.2, 0.4, 0.6, 0.8]) {
                assert!((c - e).abs() < 1e-6, "{p:?}");
            }
        }
    }

    #[test]
    fn push_pull_one_row_ramps_monotonically_between_ends() {
        let mut img = Rgba32F::new(16, 1, [0.0; 4]);
        *img.at_mut(15, 0) = [1.0; 4];
        let valid = mask_where(&img, |x, _| x == 0 || x == 15);
        assert!(push_pull_fill(&mut img, &valid));
        let r: Vec<f32> = img.px.iter().map(|p| p[0]).collect();
        assert!(r.windows(2).all(|p| p[1] >= p[0]), "{r:?}");
        // blocks would give equal neighbours across the interior; a smooth pull strictly rises
        assert!(r[1..15].windows(2).all(|p| p[1] > p[0]), "{r:?}");
    }

    #[test]
    #[should_panic(expected = "valid mask size mismatch")]
    fn push_pull_mismatched_mask_panics() {
        let mut img = Rgba32F::new(2, 2, [0.0; 4]);
        push_pull_fill(&mut img, &[true; 3]);
    }

    #[test]
    fn fills_invisible_rgb_from_visible_neighbours() {
        let mut img = Rgba32F::new(4, 4, [0.0, 1.0, 0.0, 0.0]); // green garbage, invisible
        for y in 0..4 {
            for x in 0..2 {
                *img.at_mut(x, y) = [1.0, 0.0, 0.0, 1.0]; // left half visible red
            }
        }
        dilate_under_zero_alpha(&mut img);
        for p in &img.px {
            assert!(p[0] > 0.99 && p[1] < 0.01, "{p:?}");
        }
        assert_eq!(img.at(3, 3)[3], 0.0);
    }

    #[test]
    fn all_invisible_is_unchanged() {
        let mut img = Rgba32F::new(2, 2, [0.2, 0.3, 0.4, 0.0]);
        let before = img.clone();
        dilate_under_zero_alpha(&mut img);
        assert_eq!(img, before);
    }

    #[test]
    fn all_invisible_non_square_and_empty_are_unchanged() {
        let mut img = Rgba32F::new(5, 3, [0.9, 0.1, 0.4, 0.0]);
        let before = img.clone();
        dilate_under_zero_alpha(&mut img);
        assert_eq!(img, before);
        let mut empty = Rgba32F::new(0, 0, [0.0; 4]);
        dilate_under_zero_alpha(&mut empty);
        assert!(empty.px.is_empty());
    }

    #[test]
    fn one_by_one_is_unchanged() {
        for px in [[0.3, 0.6, 0.9, 0.0], [0.3, 0.6, 0.9, 1.0]] {
            let mut img = Rgba32F::new(1, 1, px);
            dilate_under_zero_alpha(&mut img);
            assert_eq!(img.px, vec![px]);
        }
    }

    #[test]
    fn single_visible_texel_floods_whole_image_at_any_size() {
        for (w, h) in [(1, 7), (7, 1), (3, 3), (5, 3), (9, 6), (17, 5)] {
            for (vx, vy) in [(0, 0), (w - 1, h - 1), (w / 2, h / 2)] {
                let mut img = Rgba32F::new(w, h, [0.0, 1.0, 0.0, 0.0]);
                *img.at_mut(vx, vy) = [0.8, 0.2, 0.4, 1.0];
                dilate_under_zero_alpha(&mut img);
                for (i, p) in img.px.iter().enumerate() {
                    assert_eq!(
                        p[..3],
                        [0.8, 0.2, 0.4],
                        "{w}x{h} visible at ({vx},{vy}) texel {i}"
                    );
                    let want_a = if i == (vy * w + vx) as usize {
                        1.0
                    } else {
                        0.0
                    };
                    assert_eq!(p[3], want_a);
                }
            }
        }
    }

    #[test]
    fn odd_edge_column_is_not_dropped() {
        // Only the last (odd, unpaired) column is visible; it must still colour the rest.
        let mut img = Rgba32F::new(3, 2, [0.0; 4]);
        for y in 0..2 {
            *img.at_mut(2, y) = [0.1, 0.2, 0.9, 0.5];
        }
        dilate_under_zero_alpha(&mut img);
        for p in &img.px {
            assert_eq!(p[..3], [0.1, 0.2, 0.9]);
        }
    }

    #[test]
    fn alpha_and_visible_rgb_are_never_changed() {
        let (w, h) = (7u32, 5u32);
        let mut img = Rgba32F::new(w, h, [0.0; 4]);
        for y in 0..h {
            for x in 0..w {
                let a = if (x + 2 * y) % 3 == 0 {
                    0.0
                } else {
                    0.25 + 0.1 * ((x + y) % 5) as f32
                };
                *img.at_mut(x, y) = [x as f32 / 7.0, y as f32 / 5.0, 0.5, a];
            }
        }
        let before = img.clone();
        dilate_under_zero_alpha(&mut img);
        for (a, b) in img.px.iter().zip(&before.px) {
            assert_eq!(a[3], b[3]);
            if b[3] > 0.0 {
                assert_eq!(a, b);
            } else {
                assert_ne!(a[..3], b[..3]);
                assert!(a[..3].iter().all(|v| (0.0..=1.0).contains(v)));
            }
        }
    }

    #[test]
    fn alpha_threshold_is_the_8bit_code() {
        // 0.001 * 255 rounds to code 0 -> invisible; 1/255 is code 1 -> visible and untouched.
        let mut img = Rgba32F::new(3, 1, [0.0; 4]);
        *img.at_mut(0, 0) = [1.0, 0.0, 0.0, 1.0];
        *img.at_mut(1, 0) = [0.0, 1.0, 0.0, 0.001];
        *img.at_mut(2, 0) = [0.0, 0.0, 1.0, 1.0 / 255.0];
        dilate_under_zero_alpha(&mut img);
        assert_eq!(img.at(1, 0)[3], 0.001);
        assert_eq!(img.at(2, 0), [0.0, 0.0, 1.0, 1.0 / 255.0]);
        // the invisible middle texel gets the colour of its coarse block, not its own green
        assert_ne!(img.at(1, 0)[..3], [0.0, 1.0, 0.0]);
    }

    #[test]
    fn nearest_source_wins_on_a_long_strip() {
        // red far left, blue far right, gap between: each end stays closer to its own colour.
        let mut img = Rgba32F::new(16, 1, [0.0; 4]);
        *img.at_mut(0, 0) = [1.0, 0.0, 0.0, 1.0];
        *img.at_mut(15, 0) = [0.0, 0.0, 1.0, 1.0];
        dilate_under_zero_alpha(&mut img);
        assert!(img.at(1, 0)[0] > img.at(1, 0)[2]);
        assert!(img.at(14, 0)[2] > img.at(14, 0)[0]);
    }

    #[test]
    fn nan_alpha_is_treated_as_invisible() {
        let mut img = Rgba32F::new(2, 1, [0.0; 4]);
        *img.at_mut(0, 0) = [0.5, 0.5, 0.5, 1.0];
        *img.at_mut(1, 0) = [9.0, 9.0, 9.0, f32::NAN];
        dilate_under_zero_alpha(&mut img);
        assert_eq!(img.at(1, 0)[..3], [0.5, 0.5, 0.5]);
        assert!(img.at(1, 0)[3].is_nan());
    }
}
