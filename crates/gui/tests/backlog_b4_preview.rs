//! Backlog batch B4 (Note FT): a blocked preview's reason (T15), a cube DDS preview (T16).
mod support;
use sdc_gui::model::items::ItemStatus;
use sdc_gui::model::preview::{dds_preview, SourceState};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use std::path::Path;
use std::time::Duration;
use support::*;

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(120)),
        "planning did not finish"
    );
}

fn face(dir: &Path, f: &str, n: u32) {
    image::RgbImage::from_pixel(n, n, image::Rgb([5, 6, 7]))
        .save(dir.join(format!("chrome_e_{f}.png")))
        .unwrap();
}

/// T15: a cube blocked for one reason and then for another shows the new reason in the source pane.
#[test]
fn a_blocked_item_shows_its_new_reason() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/a");
    std::fs::create_dir_all(&r).unwrap();
    for f in ["px", "nx", "py", "ny", "pz"] {
        face(&r, f, 16);
    }
    let settings = GuiSettings {
        export_folder: Some(d.path().join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().join("textures"));
    idle(&mut m);
    let key = m
        .items
        .iter()
        .find(|i| matches!(i.status, ItemStatus::Error(_)))
        .expect("the incomplete cube is an error item")
        .key
        .clone();
    m.expand(&key);
    idle(&mut m);
    let SourceState::Failed(first) = &m.preview.source else {
        panic!("{:?}", m.preview.source)
    };
    assert!(first.contains("missing"), "{first}");
    face(&r, "nz", 32);
    m.replan();
    idle(&mut m);
    let SourceState::Failed(now) = &m.preview.source else {
        panic!("{:?}", m.preview.source)
    };
    assert!(now.contains("differ in size"), "{now}");
}

use engine::config::Format;
use engine::dds::{dds_bytes, dds_bytes_cube, DdsDesc, ResolvedHeader};

fn desc(size: u32, header: ResolvedHeader) -> DdsDesc {
    DdsDesc {
        width: size,
        height: size,
        format: Format::Rgba8,
        srgb: false,
        header,
    }
}

/// A cube of `size` with `mips` levels per face; face `f` is a solid grey of `10 * (f + 1)`.
fn cube_file(dir: &Path, size: u32, mips: u32) -> std::path::PathBuf {
    let mut levels = vec![];
    for f in 0..6u8 {
        let mut s = size;
        for _ in 0..mips {
            levels.push(
                {
                    let v = 10 * (f + 1);
                    [v, v, v, 255]
                }
                .repeat((s * s) as usize),
            );
            s = (s / 2).max(1);
        }
    }
    let p = dir.join("cube.dds");
    std::fs::write(
        &p,
        dds_bytes_cube(&desc(size, ResolvedHeader::Legacy), &levels).unwrap(),
    )
    .unwrap();
    p
}

fn reds(px: &[[f32; 4]]) -> std::collections::BTreeSet<u8> {
    px.iter().map(|p| (p[0] * 255.0).round() as u8).collect()
}

/// T16: a skipped cube DDS previews as a cross of its six different faces, not its first face.
#[test]
fn a_cube_dds_previews_all_six_faces() {
    let d = tempfile::tempdir().unwrap();
    let out = dds_preview(&cube_file(d.path(), 8, 1), 4096).unwrap();
    assert_eq!(out.result.faces, 6);
    // The cross is 4 x 3 faces wide and high.
    assert_eq!(out.decoded_mip(0).unwrap().len(), 12 * 64);
    for r in [10, 20, 30, 40, 50, 60] {
        assert!(
            reds(&out.decoded_mip(0).unwrap()).contains(&r),
            "face {r} is in the cross: {:?}",
            reds(&out.decoded_mip(0).unwrap())
        );
    }
}

/// T16: with mips left out (`first_level > 0`) each face still starts at its own chain.
#[test]
fn a_cube_dds_preview_from_a_lower_mip_keeps_each_faces_own_levels() {
    let d = tempfile::tempdir().unwrap();
    let out = dds_preview(&cube_file(d.path(), 8, 4), 4).unwrap();
    assert_eq!(out.result.source.first_level, 1);
    assert_eq!(out.result.levels.len(), 6 * 3);
    assert_eq!(out.decoded_mip(0).unwrap().len(), 12 * 16);
    for r in [10, 20, 30, 40, 50, 60] {
        assert!(
            reds(&out.decoded_mip(0).unwrap()).contains(&r),
            "face {r} is in the cross: {:?}",
            reds(&out.decoded_mip(0).unwrap())
        );
    }
}

/// T16: a cut-off cube is an error, never a panic.
#[test]
fn a_truncated_cube_dds_is_an_error() {
    let d = tempfile::tempdir().unwrap();
    let p = cube_file(d.path(), 8, 1);
    let b = std::fs::read(&p).unwrap();
    std::fs::write(&p, &b[..b.len() - 300]).unwrap();
    assert!(dds_preview(&p, 4096).is_err());
}

/// T16: an array DDS shows its first slice and says so.
#[test]
fn an_array_dds_says_which_slice_it_shows() {
    let d = tempfile::tempdir().unwrap();
    let mut b = dds_bytes(
        &desc(8, ResolvedHeader::Dx10),
        &[[1u8, 2, 3, 255].repeat(64)],
    )
    .unwrap();
    // DXGI header `array_size` (offset 140), and two more slices of data.
    b[140..144].copy_from_slice(&3u32.to_le_bytes());
    b.extend([[9u8, 9, 9, 255].repeat(64), [8u8, 8, 8, 255].repeat(64)].concat());
    let p = d.path().join("arr.dds");
    std::fs::write(&p, b).unwrap();
    let out = dds_preview(&p, 4096).unwrap();
    assert_eq!(out.result.faces, 1);
    let n = out.note.expect("a note");
    assert!(n.contains("Array of 3"), "{n}");
    // A plain texture says nothing.
    let q = d.path().join("one.dds");
    std::fs::write(
        &q,
        dds_bytes(
            &desc(8, ResolvedHeader::Dx10),
            &[[1u8, 2, 3, 255].repeat(64)],
        )
        .unwrap(),
    )
    .unwrap();
    assert!(dds_preview(&q, 4096).unwrap().note.is_none());
}
