//! Shared fixture for the F2 RNM tests: a PNG writer under `<dir>/in` (subfolders work).
use image::{ImageBuffer, Rgb};
use std::path::Path;

pub fn png(dir: &Path, name: &str) {
    let p = dir.join("in").join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(32, 32, |x, y| Rgb([(x * 8) as u8, (y * 8) as u8, 255u8]))
        .save(p)
        .unwrap();
}
