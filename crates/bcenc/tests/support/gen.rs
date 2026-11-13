//! Test inputs for the BC4-family tests: a deterministic xorshift `Rng`, the mixed `blocks` set and the class
//! generators. Shared by `crates/bcenc/tests`, the crate's coverage unit test (through `#[path]`) and the
//! encoders-sys and engine BC4 reference tests. No crate dependencies.
#![allow(dead_code)]

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
