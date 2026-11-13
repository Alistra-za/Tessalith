use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Float RGBA image, row-major. Values are in the stage's own domain (stored, linear light, vector, …).
#[derive(Debug, Clone, PartialEq)]
pub struct Rgba32F {
    pub width: u32,
    pub height: u32,
    pub px: Vec<[f32; 4]>,
}

impl Rgba32F {
    #[inline]
    pub fn new(width: u32, height: u32, fill: [f32; 4]) -> Self {
        Rgba32F {
            width,
            height,
            px: vec![fill; (width as usize) * (height as usize)],
        }
    }
    #[inline]
    pub fn at(&self, x: u32, y: u32) -> [f32; 4] {
        self.px[(y * self.width + x) as usize]
    }
    #[inline]
    pub fn at_mut(&mut self, x: u32, y: u32) -> &mut [f32; 4] {
        &mut self.px[(y * self.width + x) as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceInfo {
    pub bit_depth: u8,
    pub channels: u8,
    pub has_alpha: bool,
    pub is_float: bool,
}

/// Rec. 709 luma of a texel (a packed channel's `LUMA_CHANNEL`, `helper.read = luma`, `channels.grayscale_source`).
#[inline]
pub fn luma(q: &[f32; 4]) -> f32 {
    0.2126 * q[0] + 0.7152 * q[1] + 0.0722 * q[2]
}

/// The largest power of two ≤ `v` (`v` ≥ 1).
fn floor_pow2(v: u64) -> u64 {
    1 << v.ilog2()
}

/// The power-of-two size of a `w`×`h` image (spec B4). The longer side L rounds up to the next power of two when it
/// is within 5 % of it, else down. The shorter side is first scaled with it (Ss = S × L' / L), then rounds up when
/// within 15 % of the next power of two, else down (owner, 2026-11-02: 1024×768 becomes 1024×512, 2000×900 becomes
/// 2048×1024); never below 1. Axes keep their orientation.
pub fn pot_dims((w, h): (u32, u32)) -> (u32, u32) {
    let (l, s) = (u64::from(w.max(h).max(1)), u64::from(w.min(h).max(1)));
    let lp = if l.is_power_of_two() {
        l
    } else {
        let down = floor_pow2(l);
        if 100 * l >= 95 * 2 * down {
            2 * down
        } else {
            down
        }
    };
    // Ss = num / den, compared in integers.
    let (num, den) = (s * lp, l);
    let sp = if num < den {
        1
    } else {
        let down = floor_pow2(num / den);
        if down * den == num || 100 * num < 85 * 2 * down * den {
            down
        } else {
            2 * down
        }
    };
    let (lp, sp) = (lp as u32, sp as u32);
    if w >= h {
        (lp, sp)
    } else {
        (sp, lp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexing_is_row_major() {
        let mut img = Rgba32F::new(3, 2, [0.0; 4]);
        *img.at_mut(2, 1) = [1.0, 0.5, 0.25, 1.0];
        assert_eq!(img.px[5], [1.0, 0.5, 0.25, 1.0]);
        assert_eq!(img.at(2, 1)[1], 0.5);
    }

    /// Note FM29 (spec B4): the long side rounds up within 5 %, the short side at its scale within 15 %.
    #[test]
    fn pot_rule_examples() {
        for ((w, h), out) in [
            ((1000, 1000), (1024, 1024)),
            ((2000, 1000), (2048, 1024)),
            ((2000, 900), (2048, 1024)),
            ((4096, 1000), (4096, 1024)),
            ((900, 900), (512, 512)),
            ((973, 973), (1024, 1024)),
            ((972, 972), (512, 512)),
            ((1024, 768), (1024, 512)),
            ((768, 1024), (512, 1024)),
            ((1024, 880), (1024, 1024)),
            ((1024, 860), (1024, 512)),
            ((3, 5), (2, 4)),
            ((4096, 1), (4096, 1)),
            ((1, 1), (1, 1)),
            ((512, 256), (512, 256)),
        ] {
            assert_eq!(pot_dims((w, h)), out, "{w}x{h}");
        }
        // The short side scales below 1: it stays 1.
        assert_eq!(pot_dims((4000, 1)), (4096, 1));
        assert_eq!(pot_dims((900, 1)), (512, 1));
    }
}
