//! The BC1 colour classes ([`colour_classes`]) and their builders, a submodule of `gen.rs`, which re-exports
//! `colour_classes` and `COLOUR_CLASSES`. Deterministic in the seed, no crate dependencies.
#![allow(dead_code)]

use super::Rng;

/// The number of classes [`colour_classes`] returns `per_class` blocks of.
pub const COLOUR_CLASSES: usize = 30;

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
/// colour); colours of one BC1 palette that both decoders agree on. Then six classes that aim at paths a reasoned
/// block reached and random ones missed: narrow clusters of 2..=4 colours with random multiplicities, grouped or
/// shuffled; two-value grey blocks, shuffled; near-grey blocks; texels that differ only perpendicular to the
/// principal axis; r, g, b all varying with a zero covariance term; endpoints on the 565 rounding boundaries. Last, four
/// classes for three-colour blocks: black and near-black texels; two exact 565 colours with their midpoint; texels on a
/// short line near its thirds and half; near-solid exact 565 colours with texels moved by half a code. Then two
/// colours equal in red and green, apart in blue.
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
    for i in 0..per_class {
        out.push(narrow_cluster(&mut r, i));
    }
    for _ in 0..per_class {
        out.push(two_value_grey(&mut r));
    }
    for _ in 0..per_class {
        out.push(near_grey(&mut r));
    }
    for _ in 0..per_class {
        out.push(perpendicular_ties(&mut r));
    }
    for i in 0..per_class {
        out.push(zero_covariance(&mut r, i));
    }
    let (b5, b6) = (rounding_boundaries(31), rounding_boundaries(63));
    for i in 0..per_class {
        out.push(midpoint_ends(&mut r, i, &b5, &b6));
    }
    // The three-colour classes (Note HY4), after the four-colour ones so their blocks stay as they were.
    for i in 0..per_class {
        out.push(with_black(&mut r, i));
    }
    for i in 0..per_class {
        out.push(ends_and_midpoint(&mut r, i));
    }
    for i in 0..per_class {
        out.push(along_short_line(&mut r, i));
    }
    for _ in 0..per_class {
        out.push(near_565_half_step(&mut r));
    }
    // Classes from the task 3 to 5 reviews (Note HY6), after the others so their blocks stay as they were.
    for i in 0..per_class {
        out.push(blue_pair(&mut r, i));
    }
    out
}

/// Two colours equal in red and green and apart in blue, 1..=15 texels of the second, grouped (even `i`) or shuffled
/// (odd `i`). The packed order of two such endpoints rests on blue alone, so one red or green step of the
/// three-colour endpoint search can reverse it (`bc1_three_colour_search_step_crosses_the_packed_order`); about one
/// block in forty takes such a step, against about one in three thousand over the other classes.
fn blue_pair(r: &mut Rng, i: usize) -> [[u8; 4]; 16] {
    let (red, green) = (r.byte(), r.byte());
    let (b1, b2) = (r.byte(), r.byte());
    let k = r.below(15) as usize + 1;
    let mut v: Vec<[u8; 4]> = (0..16)
        .map(|t| [red, green, if t < k { b2 } else { b1 }, 255])
        .collect();
    if i % 2 == 1 {
        shuffle(r, &mut v);
    }
    std::array::from_fn(|t| v[t])
}

/// Two 565 codes 0..=6 codes apart per channel (low, high).
fn ends_pair(r: &mut Rng) -> ([i32; 3], [i32; 3]) {
    let lo: [i32; 3] = [r.below(26) as i32, r.below(58) as i32, r.below(26) as i32];
    let hi: [i32; 3] = std::array::from_fn(|c| lo[c] + r.below(7) as i32);
    (lo, hi)
}

/// The midpoint of two 565 codes as AMD decodes it (NVIDIA's differs by one on some pairs).
fn midpoint_colour(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    let (ea, eb) = (
        expand565(a[0], a[1], a[2]).map(i32::from),
        expand565(b[0], b[1], b[2]).map(i32::from),
    );
    std::array::from_fn(|c| (ea[c] + eb[c] + 1) >> 1)
}

/// 1..=8 black or near-black texels (every channel 0..=3, a third of them pure black) in a two-colour (even `i`) or
/// noisy (odd `i`) block, half of the time dark: a three-colour block's fourth palette entry is black, so these are
/// the texels that entry would fit but may not be given (no transparent black).
fn with_black(r: &mut Rng, i: usize) -> [[u8; 4]; 16] {
    let dark = r.next() & 1 == 0;
    let colour = |r: &mut Rng| -> [u8; 3] {
        if dark {
            std::array::from_fn(|_| r.below(64) as u8)
        } else {
            rgb(r)
        }
    };
    let mut v = if i.is_multiple_of(2) {
        let c = [colour(r), colour(r)];
        pick(r, &c)
    } else {
        std::array::from_fn(|_| texel(colour(r)))
    };
    for _ in 0..r.below(8) + 1 {
        let black: [u8; 3] = if r.below(3) == 0 {
            [0, 0, 0]
        } else {
            std::array::from_fn(|_| r.below(4) as u8)
        };
        v[r.below(16) as usize] = texel(black);
    }
    v
}

/// Two exact 565 colours and the colour on their midpoint, where a three-colour block can be exact and a four-colour
/// one cannot: `i % 3` 0 the three colours, 1 the midpoint off by one in one channel, 2 the midpoint only with one
/// endpoint (the three-colour fit then has to find the other).
fn ends_and_midpoint(r: &mut Rng, i: usize) -> [[u8; 4]; 16] {
    let (lo, hi) = ends_pair(r);
    let (ea, eb) = (
        expand565(lo[0], lo[1], lo[2]),
        expand565(hi[0], hi[1], hi[2]),
    );
    let mut m = midpoint_colour(lo, hi);
    if i % 3 == 1 {
        let c = r.below(3) as usize;
        m[c] += if r.next() & 1 == 0 { 1 } else { -1 };
    }
    let m = m.map(clamp8);
    let colours: Vec<[u8; 3]> = if i % 3 == 2 {
        vec![ea, m]
    } else {
        vec![ea, eb, m]
    };
    pick(r, &colours)
}

/// Texels on a short line between two exact 565 colours at fractions near the thirds and the half (1/3, 2/5, 1/2,
/// 3/5, 2/3), plus the endpoints, so the three- and four-colour errors are close: the three-colour block wins on some
/// and loses narrowly on others. Odd `i` add +-1 noise.
fn along_short_line(r: &mut Rng, i: usize) -> [[u8; 4]; 16] {
    const FRACTIONS: [(i32, i32); 7] = [(0, 1), (1, 3), (2, 5), (1, 2), (3, 5), (2, 3), (1, 1)];
    let (lo, hi) = ends_pair(r);
    let (ea, eb) = (
        expand565(lo[0], lo[1], lo[2]).map(i32::from),
        expand565(hi[0], hi[1], hi[2]).map(i32::from),
    );
    let used: Vec<(i32, i32)> = (0..r.below(3) + 2)
        .map(|_| FRACTIONS[r.below(7) as usize])
        .collect();
    std::array::from_fn(|_| {
        let (n, d) = used[r.below(used.len() as u64) as usize];
        texel(std::array::from_fn(|c| {
            let v = ea[c] + ((eb[c] - ea[c]) * n + d / 2) / d;
            let noise = if i % 2 == 1 { r.below(3) as i32 - 1 } else { 0 };
            clamp8(v + noise)
        }))
    })
}

/// A near-solid block on an exact 565 colour with 1..=3 texels moved by about half a code (3..=5 in red or blue, 2 or
/// 3 in green) in one channel each, all the same way. The bounding-box round's inset is negative there (range < 8), so
/// its far endpoint lands a whole code away, and every texel can end on the other endpoint in a three-colour block
/// (`bc1_three_colour_block_with_every_texel_on_the_high_endpoint`).
fn near_565_half_step(r: &mut Rng) -> [[u8; 4]; 16] {
    let base = expand565(
        r.below(30) as i32 + 1,
        r.below(62) as i32 + 1,
        r.below(30) as i32 + 1,
    );
    let mut v = [texel(base); 16];
    let sign = if r.next() & 1 == 0 { 1 } else { -1 };
    for _ in 0..r.below(3) + 1 {
        let t = &mut v[r.below(16) as usize];
        let c = r.below(3) as usize;
        let d = if c == 1 {
            r.below(2) as i32 + 2
        } else {
            r.below(3) as i32 + 3
        };
        t[c] = clamp8(t[c] as i32 + sign * d);
    }
    v
}

fn shuffle<T>(r: &mut Rng, v: &mut [T]) {
    for i in (1..v.len()).rev() {
        v.swap(i, r.below(i as u64 + 1) as usize);
    }
}

/// `n` (1..=16) positive counts that sum to 16, uniform over the ways to cut 16 texels into `n` runs.
fn split16(r: &mut Rng, n: usize) -> Vec<usize> {
    let mut cuts = vec![0usize, 16];
    while cuts.len() < n + 1 {
        let c = r.below(15) as usize + 1;
        if !cuts.contains(&c) {
            cuts.push(c);
        }
    }
    cuts.sort_unstable();
    cuts.windows(2).map(|w| w[1] - w[0]).collect()
}

/// The 8-bit value of a 5-bit code (or 6-bit, for `max == 63`) as both decoders expand it.
fn expand_ch(code: i32, max: i32) -> i32 {
    if max == 63 {
        (code << 2) | (code >> 4)
    } else {
        (code << 3) | (code >> 2)
    }
}

/// 2..=4 distinct colours, each within +-8 per channel of a base (random, an exact 565 colour, or one step from the
/// midpoint of two adjacent codes), with random multiplicities that sum to 16. Even `i`: the texels grouped by
/// colour; odd `i`: shuffled.
fn narrow_cluster(r: &mut Rng, i: usize) -> [[u8; 4]; 16] {
    // Three shapes in rotation: random multiplicities; skewed (one or two heavy colours and single-texel outliers);
    // and the shape of the review's counterexample (see `review_shaped`).
    let shape = (i / 6) % 3;
    if shape == 2 {
        return review_shaped(r, i);
    }
    let base: [i32; 3] = match i % 3 {
        0 => rgb(r).map(i32::from),
        1 => expand565(r.below(32) as i32, r.below(64) as i32, r.below(32) as i32).map(i32::from),
        _ => std::array::from_fn(|c| {
            let max = if c == 1 { 63 } else { 31 };
            let code = r.below(max as u64) as i32;
            (expand_ch(code, max) + expand_ch(code + 1, max)) / 2 + r.below(3) as i32 - 1
        }),
    };
    let n = r.below(3) as usize + 2;
    // Skewed blocks: one or two heavy colours (within +-4 of the base, so they share a 565 cell with it) and
    // single-texel outliers (+-8); the others cut 16 texels into random runs.
    let skewed = shape == 1;
    let heavy = if skewed { r.below(2) as usize + 1 } else { 0 };
    let mut colours: Vec<[u8; 3]> = Vec::new();
    for k in 0..n {
        let range = if k < heavy { 9 } else { 17 };
        for _ in 0..20 {
            let c: [u8; 3] = std::array::from_fn(|ch| {
                clamp8(base[ch] + r.below(range) as i32 - range as i32 / 2)
            });
            if !colours.contains(&c) {
                colours.push(c);
                break;
            }
        }
    }
    let counts = if skewed {
        let n = colours.len();
        let h = heavy.min(n - 1);
        let mut counts = vec![1; n];
        // The heavy colours (the first `h`) share the remaining texels, 1.. each.
        let rest = 16 - n;
        let first = r.below(rest as u64 + 1) as usize;
        counts[0] += if h == 1 { rest } else { first };
        if h == 2 {
            counts[1] += rest - first;
        }
        counts
    } else {
        split16(r, n)
    };
    let mut v: Vec<[u8; 4]> = Vec::with_capacity(16);
    for (c, &k) in colours.iter().zip(&counts) {
        v.extend(std::iter::repeat_n(texel(*c), k));
    }
    // A repeated colour (rare, from clamping) leaves fewer than `n`; fill with the first.
    v.resize(16, texel(colours[0]));
    if i % 2 == 1 {
        shuffle(r, &mut v);
    }
    std::array::from_fn(|k| v[k])
}

/// The shape of the review's counterexample: four colours about an exact 565 base, at offsets that sit on rounding
/// boundaries of the 565 codes (half a code step, 4 for red and blue, 2 for green, and their neighbours), two heavy
/// colours (7 and 7 texels, or another split of 14) and two single texels; 0..=2 offsets change by one step, so the
/// block stays near the shape without being the review's block. Even `i`: grouped; odd `i`: shuffled.
fn review_shaped(r: &mut Rng, i: usize) -> [[u8; 4]; 16] {
    // shortcut: this is one known block (the task 3 review's, which reaches hist4's all-on-selector-0 arm) with
    // jitter, not a general class of such blocks, so the arm's coverage rests on that one block's reasoning. Upgrade
    // to a generator built from the arm's conditions in the C++ when the initial rounds or the rounding change, or
    // when this class stops taking `Hist16Sel0`.
    const SHAPE: [[i32; 3]; 4] = [[4, -2, 0], [-3, 6, 0], [-2, -1, -4], [2, 1, 3]];
    // The shape reaches the all-texels-on-one-selector arm only on some bases (an exhaustive search over the exact
    // 565 colours found red codes = 1 or 2 mod 4, blue codes = 0 mod 4 and green codes 16, 22, 32, 48 or 58, where the
    // code spacing puts the offsets on the same side of the boundaries as in the review's block). Three blocks in four
    // use such a base, the rest any exact colour.
    const GREEN: [i32; 5] = [16, 22, 32, 48, 58];
    let base = if r.below(4) != 0 {
        expand565(
            r.below(7) as i32 * 4 + 1 + r.below(2) as i32,
            GREEN[r.below(5) as usize],
            r.below(7) as i32 * 4 + 4,
        )
    } else {
        expand565(
            r.below(29) as i32 + 1,
            r.below(61) as i32 + 1,
            r.below(29) as i32 + 1,
        )
    }
    .map(i32::from);
    let mut off = SHAPE;
    for _ in 0..r.below(3) {
        off[r.below(4) as usize][r.below(3) as usize] += r.below(2) as i32 * 2 - 1;
    }
    let colours: Vec<[u8; 3]> = off
        .iter()
        .map(|o| std::array::from_fn(|ch| clamp8(base[ch] + o[ch])))
        .collect();
    let c = if r.next() & 1 == 0 {
        7
    } else {
        r.below(13) as usize + 1
    };
    let counts = [1, 1, c, 14 - c];
    let mut v: Vec<[u8; 4]> = Vec::with_capacity(16);
    for (col, &k) in colours.iter().zip(&counts) {
        v.extend(std::iter::repeat_n(texel(*col), k));
    }
    if i % 2 == 1 {
        shuffle(r, &mut v);
    }
    std::array::from_fn(|k| v[k])
}

/// A grey block of two values `g` and `g + d` (`d` 1 or 2), 1..=15 texels on `g`, shuffled (texel 0 on either value).
fn two_value_grey(r: &mut Rng) -> [[u8; 4]; 16] {
    let d = r.below(2) as u8 + 1;
    let g = r.below(256 - d as u64) as u8;
    let low = r.below(15) as usize + 1;
    let mut v: Vec<u8> = (0..16).map(|k| if k < low { g } else { g + d }).collect();
    shuffle(r, &mut v);
    std::array::from_fn(|k| [v[k], v[k], v[k], 255])
}

/// A grey block (solid or a ramp) with one channel of 1..=3 distinct texels off by 1 or 2, so the block is never
/// grey. A texel drawn twice moves on to the next unchanged one, and a step that would leave 0..=255 goes the other
/// way, so the draws from `r` stay as they were before those two rules.
fn near_grey(r: &mut Rng) -> [[u8; 4]; 16] {
    let g0 = r.byte() as i32;
    let g1 = if r.next() & 1 == 0 {
        g0
    } else {
        r.byte() as i32
    };
    let mut v: [[u8; 4]; 16] = std::array::from_fn(|k| {
        let g = clamp8(g0 + (g1 - g0) * k as i32 / 15);
        [g, g, g, 255]
    });
    let ch = r.below(3) as usize;
    let mut changed = [false; 16];
    for _ in 0..r.below(3) + 1 {
        let mut k = r.below(16) as usize;
        while changed[k] {
            k = (k + 1) % 16;
        }
        changed[k] = true;
        let delta = r.below(2) as i32 + 1;
        let mut d = if r.next() & 1 == 0 { delta } else { -delta };
        let t = &mut v[k];
        if !(0..=255).contains(&(t[ch] as i32 + d)) {
            d = -d;
        }
        t[ch] = (t[ch] as i32 + d) as u8;
    }
    v
}

/// Colours on one axis `d` plus texels displaced only perpendicular to it (by multiples of `p = d x e`), so several
/// distinct colours project to the same point of `d`. `d` and `p` are never zero, and two pool entries are always
/// `base + t d + p` and `base + t d - p` (with `t` moved towards 0 until neither clamps), both placed in the block: an
/// exact tie on `d` between two distinct colours. The redraws and placements come from a second `Rng` forked from
/// `r`'s state, so the draws from `r` stay as they were before those rules.
fn perpendicular_ties(r: &mut Rng) -> [[u8; 4]; 16] {
    let mut fork = Rng((r.0.rotate_left(29) ^ 0xD1B5_4A32_D192_ED03) | 1);
    let cross = |d: [i32; 3], e: [i32; 3]| {
        [
            d[1] * e[2] - d[2] * e[1],
            d[2] * e[0] - d[0] * e[2],
            d[0] * e[1] - d[1] * e[0],
        ]
    };
    let base: [i32; 3] = std::array::from_fn(|_| r.below(100) as i32 + 78);
    let mut d: [i32; 3] = std::array::from_fn(|_| r.below(33) as i32 - 16);
    let mut e: [i32; 3] = std::array::from_fn(|_| r.below(3) as i32 - 1);
    while cross(d, e) == [0; 3] {
        d = std::array::from_fn(|_| fork.below(33) as i32 - 16);
        e = std::array::from_fn(|_| fork.below(3) as i32 - 1);
    }
    let p = cross(d, e);
    let at =
        |t: i32, s: i32| -> [i32; 3] { std::array::from_fn(|c| base[c] + t * d[c] + s * p[c]) };
    let n = r.below(6) as usize + 3;
    let mut pool: Vec<[u8; 3]> = Vec::with_capacity(n);
    let mut first_t = 0;
    for k in 0..n {
        let (t, s) = (r.below(7) as i32 - 3, r.below(3) as i32 - 1);
        if k == 0 {
            first_t = t;
        }
        pool.push(at(t, s).map(clamp8));
    }
    let mut t = first_t;
    let fits = |c: [i32; 3]| c.iter().all(|x| (0..=255).contains(x));
    while !(fits(at(t, 1)) && fits(at(t, -1))) {
        t -= t.signum();
    }
    pool[0] = at(t, 1).map(clamp8);
    pool[1] = at(t, -1).map(clamp8);
    let mut v = pick(r, &pool);
    let a = fork.below(16) as usize;
    let b = (a + 1 + fork.below(15) as usize) % 16;
    v[a] = texel(pool[0]);
    v[b] = texel(pool[1]);
    v
}

/// r, g and b all vary. Kind 0: cov(r, b) = 0, kind 1: cov(g, b) = 0 (a symmetric cross of the two channels about a
/// centre, the third channel free); kind 2: b varies by +-1 while r and g vary widely, so var(b) is the smallest by far.
fn zero_covariance(r: &mut Rng, kind: usize) -> [[u8; 4]; 16] {
    let centre: [i32; 3] = std::array::from_fn(|_| r.below(160) as i32 + 48);
    let mut v: Vec<[u8; 4]> = Vec::with_capacity(16);
    if kind % 3 == 2 {
        for _ in 0..16 {
            v.push([
                r.byte(),
                clamp8(centre[1] + r.below(81) as i32 - 40),
                clamp8(centre[2] + r.below(3) as i32 - 1),
                255,
            ]);
        }
    } else {
        let (x, free) = if kind.is_multiple_of(3) {
            (0, 1)
        } else {
            (1, 0)
        };
        let (amp_x, amp_z) = (r.below(40) as i32 + 1, r.below(40) as i32 + 1);
        let nx = r.below(7) as usize + 1;
        let nz = r.below(8 - nx as u64) as usize + 1;
        let w = r.below(24) as i32 + 1;
        let mut dev: Vec<[i32; 2]> = Vec::new();
        for _ in 0..nx {
            dev.push([amp_x, 0]);
            dev.push([-amp_x, 0]);
        }
        for _ in 0..nz {
            dev.push([0, amp_z]);
            dev.push([0, -amp_z]);
        }
        dev.resize(16, [0, 0]);
        shuffle(r, &mut dev);
        for dv in dev {
            let mut c = centre;
            c[x] += dv[0];
            c[2] += dv[1];
            c[free] += r.below(2 * w as u64 + 1) as i32 - w;
            v.push(texel(c.map(clamp8)));
        }
    }
    std::array::from_fn(|k| v[k])
}

/// The port's `to_5` (`max` 31) and `to_6` (`max` 63) of an 8-bit value (`bc1/fit.rs`, rgbcx's `to_5`/`to_6`), which
/// pick the initial endpoints and the one-bit-alpha extremes. `coverage_tests` checks it against the port for every
/// value.
pub fn round_code(v: u8, max: i32) -> i32 {
    let v = v as u32 * max as u32 + 128;
    ((v + (v >> 8)) >> 8) as i32
}

/// The port's `precise_round_565` of an 8-bit value as the bounding-box round scales it (`v * (1 / 255)`): truncate
/// `x * max`, then round up past `g_midpoint5`/`g_midpoint6`, whose entries are the midpoint of two adjacent expanded
/// codes written to 6 decimals. `coverage_tests` checks it against the port for every value.
pub fn round_code_midpoint(v: u8, max: i32) -> i32 {
    let x = v as f32 * (1.0f32 / 255.0f32);
    let c = (x * max as f32) as i32;
    if c == max {
        return c;
    }
    let mid2 = (expand_ch(c, max) + expand_ch(c + 1, max)) as f64;
    let mid = ((mid2 / 510.0 * 1e6).round() / 1e6) as f32;
    c + (x > mid) as i32
}

/// The 8-bit values on either side of every place where the port's rounding to a code of `max` (31 or 63) changes:
/// `v` runs over 0..=255 through [`round_code`] and [`round_code_midpoint`], and each `v` whose code differs from
/// `v - 1`'s adds `v - 1` and `v`. Sorted, without repeats.
fn rounding_boundaries(max: i32) -> Vec<u8> {
    let mut out = Vec::new();
    for round in [round_code, round_code_midpoint] {
        for v in 1..=255u8 {
            if round(v, max) != round(v - 1, max) {
                out.extend([v - 1, v]);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Two-colour (even `i`) and gradient (odd `i`) blocks whose minimum or maximum in each channel sits at a rounding
/// boundary of its 5-bit or 6-bit code, the other end of the channel random.
fn midpoint_ends(r: &mut Rng, i: usize, b5: &[u8], b6: &[u8]) -> [[u8; 4]; 16] {
    let (mut a, mut b) = ([0i32; 3], [0i32; 3]);
    for c in 0..3 {
        let list = if c == 1 { b6 } else { b5 };
        let edge = list[r.below(list.len() as u64) as usize] as i32;
        let other = r.byte() as i32;
        if r.next() & 1 == 0 {
            (a[c], b[c]) = (edge, other);
        } else {
            (a[c], b[c]) = (other, edge);
        }
    }
    if i.is_multiple_of(2) {
        let pts = [a.map(clamp8), b.map(clamp8)];
        pick(r, &pts)
    } else {
        let n = r.below(15) as usize + 2;
        on_line(r, a, b, n)
    }
}

/// A 5:6:5 code expanded to 8 bits per channel, as both decoders expand it.
fn expand565(r: i32, g: i32, b: i32) -> [u8; 3] {
    [
        ((r << 3) | (r >> 2)) as u8,
        ((g << 2) | (g >> 4)) as u8,
        ((b << 3) | (b >> 2)) as u8,
    ]
}
