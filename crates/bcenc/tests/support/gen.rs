//! Test inputs for the BC4-family and BC1 tests: a deterministic xorshift `Rng`, the mixed `blocks` set and the class
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

/// The number of classes [`colour_classes`] returns `per_class` blocks of.
pub const COLOUR_CLASSES: usize = 19;

fn rgb(r: &mut Rng) -> [u8; 3] {
    [r.byte(), r.byte(), r.byte()]
}

fn texel(c: [u8; 3]) -> [u8; 4] {
    [c[0], c[1], c[2], 255]
}

fn clamp8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// `n` (2..=16) points evenly spread from `a` to `b`, each texel one of them at random.
fn on_line(r: &mut Rng, a: [i32; 3], b: [i32; 3], n: usize) -> [[u8; 4]; 16] {
    let pts: Vec<[u8; 3]> = (0..n)
        .map(|k| std::array::from_fn(|c| clamp8(a[c] + (b[c] - a[c]) * k as i32 / (n as i32 - 1))))
        .collect();
    std::array::from_fn(|_| texel(pts[r.below(n as u64) as usize]))
}

fn pick(r: &mut Rng, colours: &[[u8; 3]]) -> [[u8; 4]; 16] {
    std::array::from_fn(|_| texel(colours[r.below(colours.len() as u64) as usize]))
}

/// The BC1 colour classes, `per_class` of each, deterministic in `seed` (alpha is 255 throughout): solid; two random
/// colours; two colours one 565 step apart; three colours; four colours; points on an axis-aligned line, a diagonal
/// line and a line in a random direction (2..=16 distinct points); a line plus noise of up to +-3; grey ramps (a
/// quarter of them one step wide); one channel varying; 1..=15 pure-black texels in an otherwise random block;
/// saturated colours (channels 0 or 255); narrow clusters (range 1..=8 per channel); full noise; two clusters;
/// smooth 2D gradients; near-solid blocks (1..=8 changes of one step on a random, exact-565, near-black or black
/// colour); colours of
/// one BC1 palette that both decoders agree on.
pub fn colour_classes(seed: u64, per_class: usize) -> Vec<[[u8; 4]; 16]> {
    let mut r = Rng(seed | 1);
    let mut out = Vec::with_capacity(per_class * COLOUR_CLASSES);
    for _ in 0..per_class {
        out.push([texel(rgb(&mut r)); 16]);
    }
    for _ in 0..per_class {
        let c = [rgb(&mut r), rgb(&mut r)];
        out.push(pick(&mut r, &c));
    }
    for _ in 0..per_class {
        // Two 565 codes one step apart in one channel, expanded as the decoders expand them.
        let code = [r.below(32) as i32, r.below(64) as i32, r.below(32) as i32];
        let ch = r.below(3) as usize;
        let max = if ch == 1 { 63 } else { 31 };
        let mut other = code;
        other[ch] = if code[ch] == max {
            max - 1
        } else {
            code[ch] + 1
        };
        let expand = |c: [i32; 3]| expand565(c[0], c[1], c[2]);
        out.push(pick(&mut r, &[expand(code), expand(other)]));
    }
    for _ in 0..per_class {
        let c = [rgb(&mut r), rgb(&mut r), rgb(&mut r)];
        out.push(pick(&mut r, &c));
    }
    for _ in 0..per_class {
        let c = [rgb(&mut r), rgb(&mut r), rgb(&mut r), rgb(&mut r)];
        out.push(pick(&mut r, &c));
    }
    for _ in 0..per_class {
        let a = rgb(&mut r).map(i32::from);
        let mut b = a;
        b[r.below(3) as usize] = r.byte() as i32;
        let n = r.below(15) as usize + 2;
        out.push(on_line(&mut r, a, b, n));
    }
    for _ in 0..per_class {
        let a = rgb(&mut r).map(i32::from);
        let len = r.below(255) as i32 + 1;
        let b = std::array::from_fn(|c| a[c] + if r.next() & 1 == 0 { len } else { -len });
        let n = r.below(15) as usize + 2;
        out.push(on_line(&mut r, a, b, n));
    }
    for _ in 0..per_class {
        let (a, b) = (rgb(&mut r).map(i32::from), rgb(&mut r).map(i32::from));
        let n = r.below(15) as usize + 2;
        out.push(on_line(&mut r, a, b, n));
    }
    for _ in 0..per_class {
        let (a, b) = (rgb(&mut r).map(i32::from), rgb(&mut r).map(i32::from));
        let n = r.below(15) as usize + 2;
        let mut v = on_line(&mut r, a, b, n);
        for t in v.iter_mut() {
            for ch in &mut t[..3] {
                *ch = clamp8(*ch as i32 + r.below(7) as i32 - 3);
            }
        }
        out.push(v);
    }
    for i in 0..per_class {
        let g0 = r.byte() as i32;
        let g1 = if i % 4 == 0 {
            if g0 == 255 {
                254
            } else {
                g0 + 1
            }
        } else {
            r.byte() as i32
        };
        out.push(std::array::from_fn(|k| {
            let g = clamp8(g0 + (g1 - g0) * k as i32 / 15);
            [g, g, g, 255]
        }));
    }
    for _ in 0..per_class {
        let base = rgb(&mut r);
        let ch = r.below(3) as usize;
        out.push(std::array::from_fn(|_| {
            let mut c = base;
            c[ch] = r.byte();
            texel(c)
        }));
    }
    for i in 0..per_class {
        let c = [rgb(&mut r), rgb(&mut r)];
        let mut v = if i % 2 == 0 {
            pick(&mut r, &c)
        } else {
            std::array::from_fn(|_| texel(rgb(&mut r)))
        };
        let blacks = r.below(15) as usize + 1;
        for _ in 0..blacks {
            v[r.below(16) as usize] = [0, 0, 0, 255];
        }
        out.push(v);
    }
    for _ in 0..per_class {
        out.push(std::array::from_fn(|_| {
            texel(std::array::from_fn(
                |_| if r.next() & 1 == 0 { 0 } else { 255 },
            ))
        }));
    }
    for _ in 0..per_class {
        let width: [u8; 3] = std::array::from_fn(|_| r.below(8) as u8 + 1);
        let base: [u8; 3] = std::array::from_fn(|c| r.below(256 - width[c] as u64) as u8);
        out.push(std::array::from_fn(|_| {
            texel(std::array::from_fn(|c| {
                base[c] + r.below(width[c] as u64 + 1) as u8
            }))
        }));
    }
    for _ in 0..per_class {
        out.push(std::array::from_fn(|_| texel(rgb(&mut r))));
    }
    for _ in 0..per_class {
        let c = [rgb(&mut r), rgb(&mut r)];
        out.push(std::array::from_fn(|_| {
            let centre = c[r.below(2) as usize];
            texel(std::array::from_fn(|ch| {
                clamp8(centre[ch] as i32 + r.below(13) as i32 - 6)
            }))
        }));
    }
    for _ in 0..per_class {
        let c00 = rgb(&mut r).map(i32::from);
        let dx: [i32; 3] = std::array::from_fn(|_| r.below(33) as i32 - 16);
        let dy: [i32; 3] = std::array::from_fn(|_| r.below(33) as i32 - 16);
        out.push(std::array::from_fn(|k| {
            let (x, y) = ((k % 4) as i32, (k / 4) as i32);
            texel(std::array::from_fn(|c| {
                clamp8(c00[c] + dx[c] * x + dy[c] * y)
            }))
        }));
    }
    for i in 0..per_class {
        // A seventh on black, some on a near-black colour (channels 0..=7), a third on an exact 565 colour.
        let base = if i % 7 == 0 {
            [0, 0, 0]
        } else if i % 5 == 0 {
            std::array::from_fn(|_| r.below(8) as u8)
        } else if i % 3 == 0 {
            expand565(r.below(32) as i32, r.below(64) as i32, r.below(32) as i32)
        } else {
            rgb(&mut r)
        };
        let mut v = [texel(base); 16];
        for _ in 0..r.below(8) + 1 {
            let t = &mut v[r.below(16) as usize];
            let c = r.below(3) as usize;
            t[c] = if t[c] == 255 { 254 } else { t[c] + 1 };
        }
        out.push(v);
    }
    for i in 0..per_class {
        // Colours both BC1 decoders (NVIDIA and AMD) give for one pair of endpoints; every other block leaves out the
        // endpoints themselves.
        let codes: [[i32; 3]; 2] =
            std::array::from_fn(|_| [r.below(32) as i32, r.below(64) as i32, r.below(32) as i32]);
        let ends = codes.map(|c| expand565(c[0], c[1], c[2]).map(i32::from));
        let mut colours = Vec::new();
        if i % 2 == 0 {
            colours.extend(ends.map(|e| e.map(|x| x as u8)));
        }
        // The 2/3 entry from each end: NVIDIA's formulas (on codes for red and blue), then AMD's.
        for (a, b) in [(0, 1), (1, 0)] {
            let gdiff = ends[b][1] - ends[a][1];
            let nv = [
                ((2 * codes[a][0] + codes[b][0]) * 22) / 8,
                (256 * ends[a][1] + (gdiff / 4) + 128 + gdiff * 80) / 256,
                ((2 * codes[a][2] + codes[b][2]) * 22) / 8,
            ];
            let amd: [i32; 3] =
                std::array::from_fn(|c| (ends[a][c] * 43 + ends[b][c] * 21 + 32) >> 6);
            if nv == amd {
                colours.push(nv.map(|x| x as u8));
            }
        }
        if colours.is_empty() {
            colours.extend(ends.map(|e| e.map(|x| x as u8)));
        }
        out.push(pick(&mut r, &colours));
    }
    out
}

/// A 5:6:5 code expanded to 8 bits per channel, as both decoders expand it.
fn expand565(r: i32, g: i32, b: i32) -> [u8; 3] {
    [
        ((r << 3) | (r >> 2)) as u8,
        ((g << 2) | (g >> 4)) as u8,
        ((b << 3) | (b >> 2)) as u8,
    ]
}
