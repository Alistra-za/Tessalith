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

/// T16: a skipped cube DDS previews as a cross (six faces), not its first face.
#[test]
fn a_cube_dds_previews_all_six_faces() {
    use engine::config::Format;
    use engine::dds::{dds_bytes_cube, DdsDesc, ResolvedHeader};
    let d = tempfile::tempdir().unwrap();
    let desc = DdsDesc {
        width: 8,
        height: 8,
        format: Format::Rgba8,
        srgb: false,
        header: ResolvedHeader::Legacy,
    };
    let faces: Vec<Vec<u8>> = (0..6u8).map(|f| [10 * f, 20, 30, 255].repeat(64)).collect();
    let p = d.path().join("cube.dds");
    std::fs::write(&p, dds_bytes_cube(&desc, &faces).unwrap()).unwrap();
    let out = dds_preview(&p, 4096).unwrap();
    assert_eq!(out.result.faces, 6);
    // The cross is 4 x 3 faces wide and high.
    assert_eq!(out.decoded[0].len(), 12 * 64);
}
