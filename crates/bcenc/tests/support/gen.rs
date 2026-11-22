//! Test inputs for the BC4-family and BC1 tests: a deterministic xorshift `Rng`, the mixed `blocks` set, the BC4 and
//! BC3-alpha classes and the BC1 transparency masks; the BC1 colour classes live in `bc1_classes.rs` and are
//! re-exported here. Shared by `crates/bcenc/tests`, the crate's unit tests (the crate-level `gen` module, through
//! `#[path]`) and the encoders-sys and engine BC4 reference tests. No crate dependencies.
#![allow(dead_code)]

#[path = "bc1_classes.rs"]
mod bc1_classes;
#[allow(unused_imports)]
pub use bc1_classes::{colour_classes, round_code, round_code_midpoint, COLOUR_CLASSES};

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn byte(&mut self) -> u8 {
        (self.next() >> 24) as u8
    }
    /// Uniform in `0..n` (`n > 0`).
    pub fn below(&mut self, n: u64) -> u64 {
        (self.next() >> 11) % n
    }
}

/// Uniform, two-level 0/255 plus noise, ramps and near-constant blocks, in rotation.
pub fn blocks(n: usize) -> Vec<[u8; 16]> {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    (0..n)
        .map(|i| {
            let mut v = [0u8; 16];
            match i % 4 {
                0 => v.iter_mut().for_each(|x| *x = r.byte()),
                1 => v.iter_mut().for_each(|x| {
                    let base = if r.next() & 1 == 0 { 0i32 } else { 255 };
                    let noise = if r.next().is_multiple_of(3) {
                        (r.byte() % 9) as i32
                    } else {
                        0
                    };
                    *x = (if base == 0 {
                        base + noise
                    } else {
                        base - noise
                    }) as u8;
                }),
                2 => {
                    let (a, b) = (r.byte() as i32, r.byte() as i32);
                    for (k, x) in v.iter_mut().enumerate() {
                        *x = (a + (b - a) * k as i32 / 15) as u8;
                    }
                }
                _ => {
                    let c = r.byte();
                    v.iter_mut()
                        .for_each(|x| *x = c.saturating_add(r.byte() % 3).saturating_sub(1))
                }
            }
            v
        })
        .collect()
}

/// The alpha-test cutoffs the classes probe.
pub const CUTS: [u8; 4] = [1, 128, 201, 255];

/// Eleven classes of single-channel blocks, `per_class` of each, deterministic in `seed`: solid; two values; three
/// values; linear ramps (both directions, step sizes 1..=16); noise in a narrow range (width 1..=12 at random
/// offsets); full-range noise; values within +-2 of each cut in `CUTS` (four classes: pure, and mixed with 0/255
/// texels); blocks holding both 0 and 255 with a narrow middle band.
pub fn bc4_classes(seed: u64, per_class: usize) -> Vec<[u8; 16]> {
    let mut r = Rng(seed | 1);
    let mut out = Vec::with_capacity(per_class * 11);
    for _ in 0..per_class {
        out.push([r.byte(); 16]);
    }
    for _ in 0..per_class {
        let (a, b) = (r.byte(), r.byte());
        out.push(std::array::from_fn(
            |_| if r.next() & 1 == 0 { a } else { b },
        ));
    }
    for _ in 0..per_class {
        let c = [r.byte(), r.byte(), r.byte()];
        out.push(std::array::from_fn(|_| c[r.below(3) as usize]));
    }
    for i in 0..per_class {
        // All 16 steps in both directions need `per_class >= 32`.
        let step = (i % 16 + 1) as i32;
        let up = (i / 16) % 2 == 0;
        let start = r.below(256) as i32;
        out.push(std::array::from_fn(|k| {
            let d = step * k as i32;
            (if up { start + d } else { start - d }).clamp(0, 255) as u8
        }));
    }
    for _ in 0..per_class {
        let width = r.below(12) as u8 + 1;
        let base = r.below(256 - width as u64) as u8;
        out.push(std::array::from_fn(|_| {
            base + r.below(width as u64 + 1) as u8
        }));
    }
    for _ in 0..per_class {
        out.push(std::array::from_fn(|_| r.byte()));
    }
    for cut in CUTS {
        for i in 0..per_class {
            let mixed = i % 2 == 1;
            out.push(std::array::from_fn(|_| {
                if mixed && r.below(4) == 0 {
                    [0u8, 255][r.below(2) as usize]
                } else {
                    (cut as i32 + r.below(5) as i32 - 2).clamp(0, 255) as u8
                }
            }));
        }
    }
    for _ in 0..per_class {
        let width = r.below(13) as u8;
        let base = r.below(254 - width as u64) as u8 + 1;
        let mut v: [u8; 16] = std::array::from_fn(|_| base + r.below(width as u64 + 1) as u8);
        let (i, j) = (r.below(16) as usize, r.below(15) as usize);
        v[i] = 0;
        v[(i + 1 + j) % 16] = 255;
        out.push(v);
    }
    out
}

/// Transparency masks for BC1 one-bit alpha (bit i marks texel i transparent), deterministic in `seed`: 0 and 0xFFFF;
/// each single bit; each single cleared bit; each row and column of the 4x4 block and their complements; the two
/// checkerboards; then `n` random sparse masks (1..=4 bits), `n` random dense ones (12..=15 bits) and `n` random ones
/// with 5, 6, .., 11 bits in turn (11 down to 5 opaque texels; appended last, so the masks before them stay as they
/// were).
pub fn alpha_masks(seed: u64, n: usize) -> Vec<u16> {
    let mut r = Rng(seed | 1);
    let mut out = vec![0u16, 0xFFFF];
    out.extend((0..16).map(|i| 1u16 << i));
    out.extend((0..16).map(|i| !(1u16 << i)));
    for k in 0..4 {
        let row = 0xFu16 << (4 * k);
        let col = 0x1111u16 << k;
        out.extend([row, !row, col, !col]);
    }
    out.extend([0x5A5A, 0xA5A5]);
    let mut random = |bits: std::ops::RangeInclusive<u64>| {
        let k = r.below(bits.end() - bits.start() + 1) + bits.start();
        let mut m = 0u16;
        while (m.count_ones() as u64) < k {
            m |= 1 << r.below(16);
        }
        m
    };
    for _ in 0..n {
        out.push(random(1..=4));
    }
    for _ in 0..n {
        out.push(random(12..=15));
    }
    for i in 0..n {
        let bits = 5 + (i % 7) as u64;
        out.push(random(bits..=bits));
    }
    out
}

/// A block and transparency mask on which `encode_bc1_alpha`'s endpoint search runs all 64 rounds with the error still
/// falling (plain weights), found by a hill climb on the round count. One least-squares pass is taken; after the first
/// search round the endpoints are (6, 0, 5) and (30, 50, 31). From there the low endpoint's green climbs about a code
/// per round (to 55) while the high one's falls (to 3), the two greens crossing, and every round is strictly better
/// (error 505,470 after round 1, 267,192 after round 64).
pub const LONG_WALK: ([[u8; 4]; 16], u16) = (
    [
        [240, 240, 241, 255],
        [253, 13, 223, 255],
        [157, 16, 215, 255],
        [157, 13, 215, 255],
        [157, 22, 215, 255],
        [249, 6, 118, 255],
        [253, 13, 223, 255],
        [18, 235, 197, 255],
        [37, 0, 0, 255],
        [252, 6, 126, 255],
        [232, 0, 122, 255],
        [21, 255, 163, 255],
        [20, 228, 2, 255],
        [161, 16, 74, 255],
        [253, 13, 224, 255],
        [253, 13, 223, 255],
    ],
    0x6042,
);

// The six blocks below pin `bc4::search_max`. Their expected bytes (BC45-R10) come from the bench code itself
// (`bench:crates/bench/src/bc4ref.rs`, branch `bench` 87c5539f, compiled unchanged), not from `bcenc`.

/// The window's least error is 7 at the single pair (164, 103), and 164 = max + 16, so only pad 16 reaches it; at pad
/// 15 the least error is 8, at (102, 148) and (111, 157), which radius 10 already found.
pub const MAX_PAD_WITNESS: [u8; 16] = [
    120, 148, 120, 111, 139, 120, 120, 120, 139, 120, 111, 113, 120, 120, 120, 113,
];

/// 100 to 115. Four pairs share the window's least error 8: (114, 100), (114, 101), (115, 100), (115, 101). Radius 10
/// already holds (114, 100), so strict `<` keeps it.
pub const MAX_TIE_RAMP: [u8; 16] = [
    100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115,
];

/// Radius 10 ends at error 174 with (154, 45); two window pairs share the least error 172, (168, 61) and (171, 60).
/// Forward order (e0 outer, ascending) meets (168, 61) first.
pub const MAX_IMPROVING_TIE: [u8; 16] = [
    78, 150, 105, 72, 95, 89, 124, 55, 91, 97, 74, 111, 96, 95, 156, 136,
];

/// Two window pairs share the least error 998 with the same e0, (225, 24) and (225, 25); ascending e1 meets (225, 24)
/// first.
pub const MAX_IMPROVING_TIE_E0: [u8; 16] = [
    109, 174, 144, 115, 53, 5, 162, 71, 201, 219, 222, 63, 76, 31, 35, 88,
];

/// At cut 128 the window block flips 1 texel, the upstream block 0, so Max falls back. The guarded radius-10 block
/// (which passed its own guard, 0 flips) differs from the upstream block, so a fallback that returned upstream
/// directly is caught.
pub const MAX_GUARD_FALLBACK: [u8; 16] = [
    60, 238, 201, 32, 198, 232, 251, 14, 93, 229, 153, 238, 113, 130, 40, 173,
];

/// At cut 128 Max falls back and the guarded radius-10 search itself falls back to the upstream block, which differs
/// from the unguarded radius-10 block, so a fallback that returned the unguarded radius-10 block is caught.
pub const MAX_GUARD_FALLBACK_BASE: [u8; 16] = [
    60, 243, 14, 47, 233, 42, 240, 198, 103, 199, 208, 76, 22, 37, 127, 252,
];

/// The six blocks above with the bench code's bytes (taken 2026-11-21 from `bench:crates/bench/src/bc4ref.rs` at
/// 87c5539f, compiled unchanged; independent of `bcenc`). Per block: upstream r10, slowest no guard, slowest cut 128,
/// max no guard, max cut 128.
#[rustfmt::skip]
pub const MAX_BENCH_BYTES: [([u8; 16], [[u8; 8]; 5]); 6] = [
    (MAX_PAD_WITNESS, {
        let s = [0x66, 0x94, 0xcb, 0xd4, 0x6d, 0x9d, 0xb4, 0x4d];
        let m = [0xa4, 0x67, 0x9e, 0x4f, 0xdb, 0xf4, 0x6f, 0xfb];
        [s, s, s, m, m]
    }),
    (MAX_TIE_RAMP, {
        let s = [0x72, 0x64, 0xc9, 0xed, 0x96, 0xdc, 0x24, 0x00];
        [s, s, s, s, s]
    }),
    (MAX_IMPROVING_TIE, {
        let s = [0x9a, 0x2d, 0x06, 0xdd, 0xee, 0xad, 0xd9, 0x42];
        let m = [0xa8, 0x3d, 0x57, 0x6f, 0x33, 0xf6, 0x6b, 0x6b];
        [s, s, s, m, m]
    }),
    (MAX_IMPROVING_TIE_E0, {
        let u = [0xd4, 0x03, 0xd4, 0xe8, 0xcc, 0x00, 0xec, 0xbf];
        let s = [0xd3, 0x02, 0xd4, 0xe8, 0xcc, 0x00, 0xec, 0xbf];
        let m = [0xe1, 0x18, 0x1d, 0xfb, 0xcc, 0x02, 0xee, 0xc4];
        [u, s, s, m, m]
    }),
    (MAX_GUARD_FALLBACK, {
        let u = [0xf1, 0x05, 0x86, 0x2e, 0x20, 0x05, 0x51, 0x7e];
        let s = [0xea, 0x04, 0x86, 0x2e, 0x20, 0xc5, 0x50, 0x7e];
        let m = [0xed, 0x1e, 0x87, 0x22, 0x20, 0x06, 0xd1, 0x66];
        [u, s, s, m, s]
    }),
    (MAX_GUARD_FALLBACK_BASE, {
        let u = [0xf2, 0x0e, 0x47, 0x8e, 0x43, 0x95, 0x9c, 0x17];
        let s = [0xed, 0x0e, 0x47, 0x8e, 0x43, 0x95, 0x9c, 0x13];
        let m = [0xed, 0x09, 0x46, 0x8e, 0x43, 0x95, 0x9c, 0x13];
        [u, s, u, m, u]
    }),
];

/// A block and transparency mask on which two distinct opaque colours tie on `encode_bc1_alpha`'s default axis
/// (306, 601, 117) while that axis is in use, so the texel order picks the low endpoint (strict `<`, rgbcx.cpp
/// 2122-2126). Texels 0 and 1 differ by (15, -9, 7), and 306 * 15 - 601 * 9 + 117 * 7 = 0: both dot to 102,400,
/// texel 2 to 107,938. Two such colours alone always get a scaled axis that separates them
/// (`two_opaque_colours_never_tie_on_the_default_alpha_axis`); the third colour turns the green-blue covariance
/// positive (6), so green's extent is not negated, the start vector (15, 12, 11) points away from the principal axis
/// and four power steps leave `k` at 1.86, below the 2 that rgbcx asks for (2105-2113).
pub const DEFAULT_AXIS_TIE: ([[u8; 4]; 16], u16) = {
    let mut px = [[0, 0, 0, 0]; 16];
    px[0] = [100, 100, 100, 255];
    px[1] = [85, 109, 93, 255];
    px[2] = [93, 112, 104, 255];
    (px, 0xFFF8)
};

/// BC3 alpha blocks: [`bc4_classes`], then for each cut in `CUTS` `per_class` blocks of the values cut - 1, cut and
/// cut + 1 only (odd blocks with some texels at 0 or 255), where the alpha-test guard decides.
pub fn alpha_classes(seed: u64, per_class: usize) -> Vec<[u8; 16]> {
    let mut out = bc4_classes(seed, per_class);
    let mut r = Rng(seed.rotate_left(17) | 1);
    for cut in CUTS {
        for i in 0..per_class {
            let mixed = i % 2 == 1;
            out.push(std::array::from_fn(|_| {
                if mixed && r.below(4) == 0 {
                    [0u8, 255][r.below(2) as usize]
                } else {
                    (cut as i32 + r.below(3) as i32 - 1).clamp(0, 255) as u8
                }
            }));
        }
    }
    out
}
