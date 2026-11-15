//! Note JD (owner, 2026-11-15): a grey map (every sub-type of the Grayscale category) is BC4 or R8, UNORM, in the
//! GUI. No picker offers another format, no GUI write lands one, and one already in the project or a preset resolves
//! itself on the settled plan, with an Info row. More generally, a (format, tag) pair the type does not allow is never
//! kept: an own pin is dropped so the type's default applies, else the best allowed format is pinned.
mod support;
use engine::config::{Category, ColorTag, Format, Subtype, Target};
use engine::presets::PresetRegistry;
use sdc_gui::model::bulk::BulkOp;
use sdc_gui::model::fields::encoding_options;
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use serde_json::json;
use std::path::Path;
use std::time::Duration;

const SK: &str = "landscape/body_sk.png";
const NORMAL: &str = "landscape/rock01_normal.png";
const GREY_OK: [(Format, ColorTag); 2] = [
    (Format::Bc4, ColorTag::Unorm),
    (Format::R8, ColorTag::Unorm),
];

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

fn grey_types() -> impl Iterator<Item = Subtype> {
    Subtype::ALL
        .into_iter()
        .filter(|s| s.category() == Category::Grayscale)
}

/// The fixture folder plus a grey `body_sk.png`, with `sdc_toml` written first when given.
fn opened_with(dir: &Path, sdc_toml: Option<&str>) -> AppModel {
    let root = support::mod_folder(dir);
    image::ImageBuffer::from_fn(32, 32, |x, _| image::Luma([100u8 + x as u8]))
        .save(root.join(SK))
        .unwrap();
    if let Some(t) = sdc_toml {
        std::fs::write(root.join("sdc.toml"), t).unwrap();
    }
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, support::session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    idle(&mut m);
    m
}

fn pair(m: &AppModel, k: &str) -> (Subtype, Format, ColorTag) {
    let p = &m
        .item(k)
        .and_then(|i| i.job.as_ref())
        .expect("a job")
        .profile;
    (p.subtype, p.format, p.color.tag)
}

fn own(m: &AppModel, k: &str, f: &str) -> Option<serde_json::Value> {
    m.project.as_ref().unwrap().file_value(k, f)
}

/// `body_sk.png` typed `skin_sk` by hand, no format of its own: BC4 from the built-in preset.
fn typed_sk(dir: &Path) -> AppModel {
    opened_with(
        dir,
        Some("[files.\"landscape/body_sk.png\"]\nsubtype = \"skin_sk\"\n"),
    )
}

#[test]
fn every_grey_picker_lists_bc4_and_r8_only() {
    let reg = PresetRegistry::builtin().unwrap();
    for t in [Target::Cs, Target::Vanilla] {
        for st in grey_types() {
            let p = reg.base_profile(t, st);
            let engine = engine::policy::allowed_encodings_for(st, t, true);
            let gui: Vec<_> = encoding_options(t, &p)
                .into_iter()
                .map(|(e, _)| e)
                .collect();
            assert!(
                gui.iter().all(|e| GREY_OK.contains(e)),
                "{st:?} under {t:?} offers {gui:?}"
            );
            if !engine.is_empty() {
                assert!(
                    gui.contains(&(Format::Bc4, ColorTag::Unorm)),
                    "{st:?} {t:?}"
                );
            }
        }
    }
}

#[test]
fn a_direct_format_write_of_bc7_on_a_grey_map_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let mut m = typed_sk(d.path());
    m.set_format(SK, Format::Bc7, ColorTag::Unorm);
    m.set_field(SK, "format", json!("bc7"));
    m.set_field(SK, "color.tag", json!("srgb"));
    idle(&mut m);
    assert_eq!(own(&m, SK, "format"), None);
    assert_eq!(own(&m, SK, "color.tag"), None);
    assert_eq!(pair(&m, SK).1, Format::Bc4);
    // R8 is the other grey format: it is written.
    m.set_format(SK, Format::R8, ColorTag::Unorm);
    idle(&mut m);
    assert_eq!(own(&m, SK, "format"), Some(json!("r8")));
}

#[test]
fn a_bulk_bc7_skips_the_grey_map() {
    let d = tempfile::tempdir().unwrap();
    let mut m = typed_sk(d.path());
    let keys = vec![SK.to_string(), NORMAL.to_string()];
    m.apply_bulk_edit(&keys, "format", BulkOp::Set(json!(["bc7", "unorm"])));
    idle(&mut m);
    assert_eq!(own(&m, SK, "format"), None);
    assert_eq!(pair(&m, SK).1, Format::Bc4);
    assert_eq!(own(&m, NORMAL, "format"), Some(json!("bc7")));
}

/// The owner's other suspect: Copy settings from a normal pinned BC7 ticks Format; the grey target skips it.
#[test]
fn copy_settings_never_lands_bc7_on_a_grey_map() {
    let d = tempfile::tempdir().unwrap();
    let mut m = typed_sk(d.path());
    m.set_format(NORMAL, Format::Bc7, ColorTag::Unorm);
    idle(&mut m);
    m.apply_copy_settings(NORMAL, &[SK.to_string()], &["format".to_string()]);
    idle(&mut m);
    assert_eq!(own(&m, SK, "format"), None);
    assert_eq!(pair(&m, SK).1, Format::Bc4);
}
