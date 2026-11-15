//! Note JG (owner, 2026-11-15): a grey map (every sub-type of the Grayscale category) is BC4 or R8, UNORM, in the
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

/// The owner's project: `_sk` typed `skin_sk` and pinned BC7. Opening it resolves the pin away (BC4 from the type),
/// says so as an Info row, and leaves the project unsaved so Save writes it.
#[test]
fn a_bc7_pin_on_a_skin_sk_resolves_to_bc4_on_open() {
    let d = tempfile::tempdir().unwrap();
    let m = opened_with(
        d.path(),
        Some(
            "[files.\"landscape/body_sk.png\"]\nsubtype = \"skin_sk\"\nformat = \"bc7\"\ncolor.tag = \"unorm\"\n",
        ),
    );
    assert_eq!(
        pair(&m, SK),
        (Subtype::SkinSk, Format::Bc4, ColorTag::Unorm)
    );
    assert_eq!(own(&m, SK, "format"), None, "the pin is dropped");
    assert_eq!(own(&m, SK, "color.tag"), None);
    let (line, _) = m.encoding_note.clone().expect("an info note");
    assert!(line.contains("body_sk.png"), "{line}");
    assert!(line.contains("BC4"), "{line}");
    assert!(m.prompts_unsaved(), "Save writes the fix");
    // No user step caused it, so its Info row offers no Undo: the rule would only fix it again.
    let rows = sdc_gui::model::bottom::problems(&m);
    let row = rows
        .iter()
        .find(|p| p.text.contains("body_sk.png"))
        .expect("an Info row");
    assert_eq!(row.level, sdc_gui::model::bottom::Level::Info);
    assert!(row.fix.is_none(), "{:?}", row.fix);
}

/// A project preset layer that gives `skin_sk` BC7 has no `[files]` pin to drop: BC4 is pinned on the texture.
#[test]
fn a_layer_that_gives_a_grey_map_bc7_is_answered_with_a_bc4_pin() {
    let d = tempfile::tempdir().unwrap();
    let m = opened_with(
        d.path(),
        Some(
            "[subtypes.skin_sk]\nformat = \"bc7\"\n\n[files.\"landscape/body_sk.png\"]\nsubtype = \"skin_sk\"\n",
        ),
    );
    assert_eq!(
        pair(&m, SK),
        (Subtype::SkinSk, Format::Bc4, ColorTag::Unorm)
    );
    assert_eq!(own(&m, SK, "format"), Some(json!("bc4")));
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

/// A texture pinned BC7 as a colour map, then typed `skin_sk`: the pin goes and the type's BC4 applies.
#[test]
fn retyping_a_bc7_texture_to_a_grey_type_drops_the_pin() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_with(d.path(), None);
    m.set_field(SK, "subtype", json!("diffuse"));
    idle(&mut m);
    m.set_format(SK, Format::Bc7, ColorTag::Unorm);
    idle(&mut m);
    assert_eq!(own(&m, SK, "format"), Some(json!("bc7")));
    let steps = m.history.undo.len();
    m.set_field(SK, "subtype", json!("skin_sk"));
    idle(&mut m);
    assert_eq!(
        pair(&m, SK),
        (Subtype::SkinSk, Format::Bc4, ColorTag::Unorm)
    );
    assert_eq!(own(&m, SK, "format"), None);
    assert_eq!(m.history.undo.len(), steps + 1, "one step with the edit");
}

/// A pair the type does not allow resolves on any type, not only grey ones: a normal pinned BC4 on disk.
#[test]
fn a_disallowed_pair_on_any_type_resolves_on_open() {
    let d = tempfile::tempdir().unwrap();
    let m = opened_with(
        d.path(),
        Some("[files.\"landscape/rock01_normal.png\"]\nsubtype = \"normal\"\nformat = \"bc4\"\n"),
    );
    let (st, f, t) = pair(&m, NORMAL);
    assert_eq!(st, Subtype::Normal);
    assert!(
        engine::policy::allowed_encodings_for(st, m.target, false).contains(&(f, t)),
        "{f:?} {t:?}"
    );
    assert_eq!(own(&m, NORMAL, "format"), None);
    assert!(m.encoding_note.is_some());
}

#[test]
fn the_presets_tab_never_keeps_bc7_on_a_grey_type() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("presets");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mine.toml"),
        "schema_version = 1\nid = \"mine\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n\n[subtypes.skin_sk]\nformat = \"bc7\"\n",
    )
    .unwrap();
    let mut pm = sdc_gui::model::presets::PresetsModel::new(Some(dir), Target::Cs);
    pm.select("mine");
    let row = pm.row(Subtype::SkinSk).expect("a row");
    let p = row.profile.as_ref().expect("a profile");
    assert!(
        GREY_OK.contains(&(p.format, p.color.tag)),
        "{:?}",
        (p.format, p.color.tag)
    );
    assert!(pm.dirty(), "Save writes the fix");
    assert!(
        pm.note().is_some_and(|n| n.contains("BC7")),
        "{:?}",
        pm.note()
    );
    pm.set_field(Subtype::SkinSk, "format", json!(["bc7", "unorm"]));
    let p = pm.row(Subtype::SkinSk).unwrap().profile.clone().unwrap();
    assert_ne!(
        p.format,
        Format::Bc7,
        "the editor never writes BC7 on a grey type"
    );
}

#[test]
fn revert_fixes_again_and_a_new_draft_drops_the_note() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("presets");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mine.toml"),
        "schema_version = 1\nid = \"mine\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n\n[subtypes.skin_sk]\nformat = \"bc7\"\n",
    )
    .unwrap();
    let mut pm = sdc_gui::model::presets::PresetsModel::new(Some(dir), Target::Cs);
    pm.select("mine");
    pm.revert();
    let p = pm.row(Subtype::SkinSk).unwrap().profile.clone().unwrap();
    assert!(
        GREY_OK.contains(&(p.format, p.color.tag)),
        "revert restores the file's BC7, and it is fixed again: {:?}",
        (p.format, p.color.tag)
    );
    assert!(pm.note().is_some(), "the fix is shown again");
    pm.new_preset(Target::Cs);
    assert_eq!(pm.note(), None, "a new preset has nothing fixed");
}

/// No built-in preset gives any sub-type a pair the GUI does not keep, so opening a healthy project fixes nothing.
#[test]
fn every_built_in_default_is_a_pair_the_gui_keeps() {
    let d = tempfile::tempdir().unwrap();
    let mut pm =
        sdc_gui::model::presets::PresetsModel::new(Some(d.path().join("none")), Target::Cs);
    let ids: Vec<String> = pm
        .list()
        .into_iter()
        .filter(|(_, b)| *b)
        .map(|(id, _)| id)
        .collect();
    assert!(!ids.is_empty());
    let mut bad = vec![];
    for id in ids {
        pm.select(&id);
        for st in Subtype::ALL {
            let Some(p) = pm.row(st).and_then(|r| r.profile.as_ref().ok()) else {
                continue;
            };
            let list = sdc_gui::model::format_rule::allowed(st, pm.target);
            if !list.is_empty() && !list.contains(&(p.format, p.color.tag)) {
                bad.push(format!("{id} {st:?}: {:?}", (p.format, p.color.tag)));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// A healthy project opens with no fix: no history step, no note, nothing unsaved.
#[test]
fn a_healthy_grey_map_is_left_alone() {
    let d = tempfile::tempdir().unwrap();
    let m = typed_sk(d.path());
    assert_eq!(
        pair(&m, SK),
        (Subtype::SkinSk, Format::Bc4, ColorTag::Unorm)
    );
    assert_eq!(m.encoding_note, None);
    assert!(!m.can_undo());
}

/// Round 2: the own BC7 pin is cleared, the layer still gives BC7, so BC4 is pinned. One step, one Info row.
#[test]
fn clear_then_pin_is_one_step_and_one_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_with(
        d.path(),
        Some(
            "[subtypes.skin_sk]\nformat = \"bc7\"\n\n[files.\"landscape/body_sk.png\"]\nsubtype = \"skin_sk\"\nformat = \"bc7\"\n",
        ),
    );
    assert_eq!(
        pair(&m, SK),
        (Subtype::SkinSk, Format::Bc4, ColorTag::Unorm)
    );
    assert_eq!(own(&m, SK, "format"), Some(json!("bc4")));
    let rows = sdc_gui::model::bottom::problems(&m);
    assert_eq!(
        rows.iter()
            .filter(|p| p.text.contains("body_sk.png"))
            .count(),
        1,
        "{:?}",
        rows.iter().map(|p| &p.text).collect::<Vec<_>>()
    );
    // One undo goes straight back to the file's BC7; two steps would stop at the cleared pin.
    m.undo();
    assert_eq!(
        own(&m, SK, "format"),
        Some(json!("bc7")),
        "the clear and the pin were one step"
    );
}

/// Undo of the open-time fix brings the BC7 pin back for a moment; the rule fixes it again at once.
#[test]
fn undoing_the_open_fix_fixes_it_again() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_with(
        d.path(),
        Some("[files.\"landscape/body_sk.png\"]\nsubtype = \"skin_sk\"\nformat = \"bc7\"\n"),
    );
    m.undo();
    idle(&mut m);
    assert_eq!(
        pair(&m, SK),
        (Subtype::SkinSk, Format::Bc4, ColorTag::Unorm)
    );
    assert_eq!(own(&m, SK, "format"), None);
    // The new fix is a step of its own, so Redo is gone (DECISIONS JG3).
    assert!(m.can_undo());
    assert!(!m.can_redo());
}
