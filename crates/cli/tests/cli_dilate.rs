//! `sdc dilate`: push-pull fill outside a mask, masked texels bit-exact (Note IB). Images are generated here.
mod common;

use image::{ExtendedColorType as E, ImageEncoder};
use std::path::{Path, PathBuf};

fn run(args: &[&str]) -> (i32, String, String) {
    let o = common::sdc().arg("dilate").args(args).output().unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// A PNG from sample codes (`channels` per texel, 8 or 16 bits).
fn write_png(p: &Path, w: u32, h: u32, channels: u8, depth: u8, codes: &[u16]) {
    let ct = match (channels, depth) {
        (1, 8) => E::L8,
        (2, 8) => E::La8,
        (3, 8) => E::Rgb8,
        (4, 8) => E::Rgba8,
        (1, 16) => E::L16,
        (2, 16) => E::La16,
        (3, 16) => E::Rgb16,
        _ => E::Rgba16,
    };
    let bytes: Vec<u8> = if depth == 8 {
        codes.iter().map(|&c| c as u8).collect()
    } else {
        codes.iter().flat_map(|c| c.to_ne_bytes()).collect()
    };
    let mut out = vec![];
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&bytes, w, h, ct)
        .unwrap();
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, out).unwrap();
}

/// (channels, depth, codes) of a PNG.
fn read_png(p: &Path) -> (u8, u8, Vec<u16>) {
    let img = image::open(p).unwrap();
    let ct = img.color();
    let ch = ct.channel_count();
    let depth = (ct.bits_per_pixel() / u16::from(ch)) as u8;
    let bytes = img.into_bytes();
    let codes = if depth == 8 {
        bytes.iter().map(|&b| u16::from(b)).collect()
    } else {
        bytes
            .chunks(2)
            .map(|c| u16::from_ne_bytes([c[0], c[1]]))
            .collect()
    };
    (ch, depth, codes)
}

/// A mask PNG (RGB8): 255 where `on(x, y)`.
fn write_mask(p: &Path, w: u32, h: u32, on: impl Fn(u32, u32) -> bool) {
    let mut c = vec![];
    for y in 0..h {
        for x in 0..w {
            c.extend([if on(x, y) { 255 } else { 0 }; 3]);
        }
    }
    write_png(p, w, h, 3, 8, &c);
}

fn rng(seed: &mut u32) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed
}

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn masked_texels_stay_bit_exact_and_every_other_texel_is_filled() {
    const N: u32 = 16;
    for depth in [8u8, 16] {
        for ch in 1u8..=4 {
            let d = dir();
            let max = if depth == 8 { 255u32 } else { 65535 };
            let on = |x: u32, y: u32| x < 6 && y < 9;
            let mut seed = 0x1234_5678u32 + u32::from(ch) * 7 + u32::from(depth);
            let mut codes = vec![];
            for y in 0..N {
                for x in 0..N {
                    for _ in 0..ch {
                        // a masked texel takes any code but 0 (so 0 means "not filled" outside); the full range
                        // reaches 1 and the top code
                        codes.push(if on(x, y) {
                            (1 + rng(&mut seed) % max) as u16
                        } else {
                            0
                        });
                    }
                }
            }
            codes[0] = max as u16;
            let last = codes.len() - 1;
            codes[last.min(1)] = 1;
            let src = d.path().join("foo_p.png");
            write_png(&src, N, N, ch, depth, &codes);
            write_mask(&d.path().join("foo_matid.png"), N, N, on);
            let (code, out, err) = run(&[&s(&src)]);
            assert_eq!(code, 0, "{ch}ch {depth}bit: {err}");
            assert!(out.contains("wrote"), "{out}");
            let (c2, d2, got) = read_png(&d.path().join("dilated").join("foo_p.png"));
            assert_eq!((c2, d2), (ch, depth));
            assert_eq!(got.len(), codes.len());
            for t in 0..(N * N) as usize {
                let (x, y) = (t as u32 % N, t as u32 / N);
                let (a, b) = (
                    &codes[t * ch as usize..][..ch as usize],
                    &got[t * ch as usize..][..ch as usize],
                );
                if on(x, y) {
                    assert_eq!(a, b, "{ch}ch {depth}bit texel {x},{y} changed");
                } else {
                    assert!(
                        b.iter().all(|&v| v > 0),
                        "{ch}ch {depth}bit texel {x},{y} not filled: {b:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_mask_is_found_by_set_base_then_by_full_stem() {
    let d = dir();
    write_png(&d.path().join("foo_n.png"), 4, 4, 3, 8, &[100; 48]);
    write_mask(&d.path().join("foo_matid.png"), 4, 4, |x, _| x == 0);
    let (code, out, err) = run(&[&s(&d.path().join("foo_n.png"))]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("foo_matid.png"), "{out}");
    // a stem the engine cuts no suffix from names its own mask
    write_png(&d.path().join("rock_x.png"), 4, 4, 3, 8, &[100; 48]);
    write_mask(&d.path().join("rock_x_matid.png"), 4, 4, |x, _| x == 0);
    let (code, out, err) = run(&[&s(&d.path().join("rock_x.png"))]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("rock_x_matid.png"), "{out}");
}

#[test]
fn a_uvmask_counts_only_where_it_is_255() {
    let d = dir();
    let mut codes = vec![0u16; 8 * 8];
    for y in 0..8 {
        for x in 0..8 {
            codes[y * 8 + x] = 200;
        }
    }
    write_png(&d.path().join("foo_p.png"), 8, 8, 1, 8, &codes);
    // 255 on the left 3 columns, 127 (padding) on the next two, 0 beyond
    let m: Vec<u16> = (0..64)
        .map(|i| [255, 255, 255, 127, 127, 0, 0, 0][i % 8])
        .collect();
    write_png(&d.path().join("foo_uvmask.png"), 8, 8, 1, 8, &m);
    let (code, out, err) = run(&[&s(&d.path().join("foo_p.png"))]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("filled 40 of 64"), "{out}");
}

#[test]
fn an_explicit_mask_serves_every_input() {
    let d = dir();
    write_png(&d.path().join("a_p.png"), 4, 4, 1, 8, &[90; 16]);
    write_png(&d.path().join("b_p.png"), 4, 4, 1, 8, &[90; 16]);
    write_mask(&d.path().join("m.png"), 4, 4, |x, _| x < 2);
    let (code, out, err) = run(&[
        &s(&d.path().join("*_p.png")),
        "--mask",
        &s(&d.path().join("m.png")),
    ]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(out.matches("wrote").count(), 2, "{out}");
    assert!(d.path().join("dilated").join("a_p.png").is_file());
    assert!(d.path().join("dilated").join("b_p.png").is_file());
}

#[test]
fn no_mask_skips_the_input_with_a_message() {
    let d = dir();
    write_png(&d.path().join("foo_p.png"), 4, 4, 1, 8, &[90; 16]);
    let (code, out, err) = run(&[&s(&d.path().join("foo_p.png"))]);
    assert_eq!(code, 1);
    assert!(out.is_empty(), "{out}");
    assert!(err.contains("no mask"), "{err}");
    assert!(!d.path().join("dilated").exists());
}

#[test]
fn a_mask_of_another_size_is_an_error_for_that_input() {
    let d = dir();
    write_png(&d.path().join("foo_p.png"), 4, 4, 1, 8, &[90; 16]);
    write_mask(&d.path().join("foo_matid.png"), 8, 8, |x, _| x < 2);
    let (code, _, err) = run(&[&s(&d.path().join("foo_p.png"))]);
    assert_eq!(code, 1);
    assert!(err.contains("8x8") && err.contains("4x4"), "{err}");
    assert!(!d.path().join("dilated").exists());
}

#[test]
fn a_colour_map_fills_in_linear_light() {
    let d = dir();
    let mut codes = vec![0u16; 8];
    codes[7] = 255;
    let put = |name: &str| {
        write_png(&d.path().join(name), 8, 1, 1, 8, &codes);
    };
    put("c.png");
    put("c_p.png");
    write_mask(&d.path().join("c_matid.png"), 8, 1, |x, _| x == 0 || x == 7);
    let (code, _, err) = run(&[&s(&d.path().join("c.png")), &s(&d.path().join("c_p.png"))]);
    assert_eq!(code, 0, "{err}");
    let col = read_png(&d.path().join("dilated").join("c.png")).2;
    let dat = read_png(&d.path().join("dilated").join("c_p.png")).2;
    assert!(col.iter().zip(&dat).all(|(c, d)| c >= d), "{col:?} {dat:?}");
    for x in [3usize, 4] {
        assert!(
            col[x] > dat[x] + 20,
            "texel {x}: colour {} data {}",
            col[x],
            dat[x]
        );
    }
}

#[test]
fn the_filled_texels_of_a_normal_map_have_unit_length() {
    let d = dir();
    let mut codes = vec![];
    for y in 0..8u32 {
        for x in 0..8u32 {
            // two valid groups with opposite tilts (+-0.6, 0, 0.8); their plain mean has length 0.8
            codes.extend(match (x, y) {
                (0, _) => [204, 128, 230],
                (7, _) => [51, 128, 230],
                _ => [0, 0, 0],
            });
        }
    }
    write_png(&d.path().join("foo_n.png"), 8, 8, 3, 8, &codes);
    write_mask(&d.path().join("foo_matid.png"), 8, 8, |x, _| {
        x == 0 || x == 7
    });
    let (code, out, err) = run(&[&s(&d.path().join("foo_n.png"))]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("normal"), "{out}");
    let got = read_png(&d.path().join("dilated").join("foo_n.png")).2;
    for t in 0..64usize {
        if t % 8 == 0 || t % 8 == 7 {
            continue;
        }
        let n = [0, 1, 2].map(|c| f32::from(got[t * 3 + c]) / 127.5 - 1.0);
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 0.02, "texel {t}: {n:?} length {len}");
    }
}

#[test]
fn alpha_is_filled_as_data() {
    let d = dir();
    let mut codes = vec![];
    for x in 0..8u32 {
        codes.extend(if x == 0 {
            [10, 20, 30, 200]
        } else {
            [0, 0, 0, 0]
        });
    }
    write_png(&d.path().join("foo_p.png"), 8, 1, 4, 8, &codes);
    write_mask(&d.path().join("foo_matid.png"), 8, 1, |x, _| x == 0);
    assert_eq!(run(&[&s(&d.path().join("foo_p.png"))]).0, 0);
    let got = read_png(&d.path().join("dilated").join("foo_p.png")).2;
    assert!((0..8).all(|t| got[t * 4 + 3] == 200), "{got:?}");
}

#[test]
fn mask_images_and_the_dilated_folder_are_never_inputs() {
    let d = dir();
    write_png(&d.path().join("foo_p.png"), 4, 4, 1, 8, &[90; 16]);
    write_mask(&d.path().join("foo_matid.png"), 4, 4, |x, _| x < 2);
    write_png(&d.path().join("foo_uvmask.png"), 4, 4, 1, 8, &[255; 16]);
    write_png(
        &d.path().join("dilated").join("old_p.png"),
        4,
        4,
        1,
        8,
        &[90; 16],
    );
    let (code, out, err) = run(&[&s(d.path()), "--recursive"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(out.matches("wrote").count(), 1, "{out}");
    let mut names: Vec<_> = std::fs::read_dir(d.path().join("dilated"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["foo_p.png", "old_p.png"]);
    // nothing but a mask or a dilated file: no input at all
    let (code, _, err) = run(&[&s(&d.path().join("foo_matid.png"))]);
    assert_eq!(code, 2, "{err}");
}

#[test]
fn an_existing_output_is_kept_without_overwrite() {
    let d = dir();
    write_png(&d.path().join("foo_p.png"), 4, 4, 1, 8, &[90; 16]);
    write_mask(&d.path().join("foo_matid.png"), 4, 4, |x, _| x < 2);
    let dest: PathBuf = d.path().join("dilated").join("foo_p.png");
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::write(&dest, b"mine").unwrap();
    let src = s(&d.path().join("foo_p.png"));
    let (code, _, err) = run(&[&src]);
    assert_eq!(code, 1);
    assert!(err.contains("--overwrite"), "{err}");
    assert_eq!(std::fs::read(&dest).unwrap(), b"mine");
    assert_eq!(run(&[&src, "--overwrite"]).0, 0);
    assert_eq!(read_png(&dest).0, 1);
}

#[test]
fn out_puts_the_images_in_another_folder() {
    let d = dir();
    write_png(&d.path().join("foo_p.png"), 4, 4, 1, 8, &[90; 16]);
    write_mask(&d.path().join("foo_matid.png"), 4, 4, |x, _| x < 2);
    let o = d.path().join("elsewhere");
    let (code, _, err) = run(&[&s(&d.path().join("foo_p.png")), "--out", &s(&o)]);
    assert_eq!(code, 0, "{err}");
    assert!(o.join("foo_p.png").is_file());
    assert!(!d.path().join("dilated").exists());
}

#[test]
fn a_float_input_is_refused() {
    let d = dir();
    let p = d.path().join("foo_p.hdr");
    image::Rgb32FImage::from_pixel(4, 4, image::Rgb([0.5, 0.5, 0.5]))
        .save(&p)
        .unwrap();
    write_mask(&d.path().join("foo_matid.png"), 4, 4, |x, _| x < 2);
    let (code, _, err) = run(&[&s(&p)]);
    assert_eq!(code, 1);
    assert!(err.contains("float"), "{err}");
    assert!(!d.path().join("dilated").exists());
}

#[test]
fn a_pattern_that_names_no_image_is_a_usage_error() {
    let d = dir();
    let (code, _, err) = run(&[&s(&d.path().join("nope*.png"))]);
    assert_eq!(code, 2);
    assert!(err.contains("no image matches"), "{err}");
}
