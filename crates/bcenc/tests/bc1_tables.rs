//! The BC1 tables equal the vendored text, and the dual palettes equal the two decoders of `encoders_sys`.
use bcenc::bc1::palette::{dual_err, dual_palette, single_tables, DualPalette, MatchEntry};
use bcenc::bc1::tables::{
    BEST_TOTAL_ORDERINGS3, BEST_TOTAL_ORDERINGS4, MAX_TOTAL_ORDERINGS4, N3, N4,
    UNIQUE_TOTAL_ORDERS3, UNIQUE_TOTAL_ORDERS4,
};
use encoders_sys::Bc1Model;
use std::path::PathBuf;

fn vendor(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../encoders-sys/vendor/bc7enc_rdo")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// The text from the first `=` after `marker` up to the next `};`.
fn initialiser<'a>(text: &'a str, marker: &str) -> &'a str {
    let at = text.find(marker).unwrap_or_else(|| panic!("no {marker}"));
    let rest = &text[at..];
    let eq = rest.find('=').expect("no =");
    let rest = &rest[eq + 1..];
    &rest[..rest.find("};").expect("no };")]
}

/// Every innermost `{a,b,..}` group of an initialiser list, as numbers.
fn groups(text: &str) -> Vec<Vec<u32>> {
    let mut out = Vec::new();
    for piece in text.split('}') {
        let Some(open) = piece.rfind('{') else {
            continue;
        };
        let nums: Vec<u32> = piece[open + 1..]
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().unwrap_or_else(|_| panic!("not a number: {s:?}")))
            .collect();
        out.push(nums);
    }
    out
}

/// `const uint32_t <name> = <n>;`
fn constant(text: &str, name: &str) -> usize {
    let at = text.find(&format!("uint32_t {name} =")).expect(name);
    let rest = &text[at..];
    let rest = &rest[rest.find('=').unwrap() + 1..];
    rest[..rest.find(';').unwrap()].trim().parse().unwrap()
}

fn check<T: Copy + Into<u32>, const W: usize>(what: &str, got: &[[T; W]], want: &[Vec<u32>]) {
    assert_eq!(got.len(), want.len(), "{what}: row count");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        let g: Vec<u32> = g.iter().map(|&x| x.into()).collect();
        assert_eq!(&g, w, "{what}: row {i}");
    }
}

#[test]
fn total_ordering_tables_equal_the_vendored_text() {
    let cpp = vendor("rgbcx.cpp");
    let hdr = vendor("rgbcx.h");
    let table4 = vendor("rgbcx_table4.h");

    // The build uses the large table: RGBCX_USE_SMALLER_TABLES is 0 in rgbcx.h.
    assert!(hdr.contains("#define RGBCX_USE_SMALLER_TABLES 0"));
    let else_at = hdr
        .find("#else\n\tconst uint32_t MAX_TOTAL_ORDERINGS4")
        .or_else(|| hdr.find("#else\r\n\tconst uint32_t MAX_TOTAL_ORDERINGS4"));
    let max = constant(
        &hdr[else_at.expect("MAX_TOTAL_ORDERINGS4 else branch")..],
        "MAX_TOTAL_ORDERINGS4",
    );
    assert_eq!(MAX_TOTAL_ORDERINGS4, max);
    assert_eq!(N4, constant(&cpp, "NUM_UNIQUE_TOTAL_ORDERINGS4"));
    assert_eq!(N3, constant(&cpp, "NUM_UNIQUE_TOTAL_ORDERINGS3"));

    let u4 = groups(initialiser(&cpp, "g_unique_total_orders4["));
    let u3 = groups(initialiser(&cpp, "g_unique_total_orders3["));
    let b3 = groups(initialiser(&cpp, "g_best_total_orderings3["));
    let b4 = groups(&table4);
    assert_eq!(b4[0].len(), max, "rgbcx_table4.h row width");

    check("UNIQUE_TOTAL_ORDERS4", &UNIQUE_TOTAL_ORDERS4, &u4);
    check("UNIQUE_TOTAL_ORDERS3", &UNIQUE_TOTAL_ORDERS3, &u3);
    check("BEST_TOTAL_ORDERINGS3", &BEST_TOTAL_ORDERINGS3, &b3);
    check("BEST_TOTAL_ORDERINGS4", &BEST_TOTAL_ORDERINGS4, &b4);
}

/// Five settings of the two channels that are not swept: (first low, second low, first high, second high), as
/// fractions of the code range in 1/16ths.
const SETTINGS: [[usize; 4]; 5] = [
    [0, 0, 0, 0],
    [15, 15, 15, 15],
    [5, 9, 12, 3],
    [15, 0, 0, 15],
    [8, 14, 2, 11],
];

#[test]
fn dual_palette_equals_the_two_decoders() {
    let mut checked = 0;
    for three in [false, true] {
        for swept in 0..3 {
            // Code range of each channel: 5, 6, 5 bits.
            let range = [32usize, 64, 32];
            let others: Vec<usize> = (0..3).filter(|&c| c != swept).collect();
            for s in SETTINGS {
                for lo in 0..range[swept] {
                    for hi in 0..range[swept] {
                        let mut l = [0usize; 3];
                        let mut h = [0usize; 3];
                        l[swept] = lo;
                        h[swept] = hi;
                        l[others[0]] = s[0] * (range[others[0]] - 1) / 15;
                        l[others[1]] = s[1] * (range[others[1]] - 1) / 15;
                        h[others[0]] = s[2] * (range[others[0]] - 1) / 15;
                        h[others[1]] = s[3] * (range[others[1]] - 1) / 15;
                        let c0 = (l[0] << 11 | l[1] << 5 | l[2]) as u16;
                        let c1 = (h[0] << 11 | h[1] << 5 | h[2]) as u16;
                        // The decoder reads c0 > c1 as 4-colour and c0 <= c1 as 3-colour.
                        if three != (c0 <= c1) {
                            continue;
                        }
                        let mut block = [0u8; 8];
                        block[..2].copy_from_slice(&c0.to_le_bytes());
                        block[2..4].copy_from_slice(&c1.to_le_bytes());
                        block[4..].fill(0xE4); // texels 0..=3 of every row show entries 0..=3
                        let p = dual_palette(
                            three,
                            l[0] as i32,
                            l[1] as i32,
                            l[2] as i32,
                            h[0] as i32,
                            h[1] as i32,
                            h[2] as i32,
                        );
                        for (k, model) in [Bc1Model::Nvidia, Bc1Model::Amd].into_iter().enumerate()
                        {
                            let px = encoders_sys::bc1_decode_block(&block, model);
                            // rgbcx's palette order is linear: 0, 3, 1, 2 are the block's entries 0, 1, 2, 3
                            // in a 4-colour block; a 3-colour block is the block's order.
                            let order = if three { [0, 1, 2, 3] } else { [0, 3, 1, 2] };
                            for (entry, &idx) in order.iter().enumerate() {
                                let want = px[entry];
                                let got = [p.r[k][idx], p.g[k][idx], p.b[k][idx]];
                                assert_eq!(
                                    got,
                                    [want[0] as i32, want[1] as i32, want[2] as i32],
                                    "three={three} model={model:?} c0={c0:#06x} c1={c1:#06x} entry {entry}"
                                );
                            }
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 30_000, "only {checked} endpoint pairs checked");
}

/// The stored error of every single-colour entry is what `dual_err` says for it, and the entry is the one the C++
/// loop order and tie rule pick: the lowest error over all (lo, hi), a later equal-endpoint pair winning ties.
#[test]
fn single_colour_entries_equal_dual_err() {
    let t = single_tables();
    let cases: [(&str, &[MatchEntry; 256], usize, bool); 4] = [
        ("match5_equals_1", &t.match5_equals_1, 32, false),
        ("match5_half", &t.match5_half, 32, true),
        ("match6_equals_1", &t.match6_equals_1, 64, false),
        ("match6_half", &t.match6_half, 64, true),
    ];
    for (name, table, size, half) in cases {
        // 5-bit tables are the red channel, 6-bit tables the green channel; the others stay 0.
        let (chan, w) = if size == 32 {
            (0, [1, 0, 0])
        } else {
            (1, [0, 1, 0])
        };
        let err = |i: usize, hi: usize, lo: usize| -> u32 {
            // hi is the palette's first endpoint (weight 2 in entry 1), lo the second.
            let mut e = [0i32; 6];
            e[chan] = hi as i32;
            e[3 + chan] = lo as i32;
            let p: DualPalette = dual_palette(half, e[0], e[1], e[2], e[3], e[4], e[5]);
            let mut c = [0u8; 4];
            c[chan] = i as u8;
            dual_err(&p, w, if half { 2 } else { 1 }, c)
        };
        for (i, &got) in table.iter().enumerate() {
            let mut lowest = u32::MAX;
            let (mut best_hi, mut best_lo) = (0, 0);
            for lo in 0..size {
                for hi in 0..size {
                    let e = err(i, hi, lo);
                    if e < lowest || (e == lowest && lo == hi) {
                        lowest = e;
                        best_hi = hi;
                        best_lo = lo;
                    }
                }
            }
            // The C++ clamps the stored error to 255; the port does not, so no error may exceed it.
            assert!(
                lowest <= 255,
                "{name}[{i}]: error {lowest} would be clamped"
            );
            assert_eq!(
                (got.hi as usize, got.lo as usize, got.e as u32),
                (best_hi, best_lo, lowest),
                "{name}[{i}]"
            );
            assert_eq!(
                err(i, got.hi as usize, got.lo as usize),
                got.e as u32,
                "{name}[{i}] stored error"
            );
        }
    }
}
