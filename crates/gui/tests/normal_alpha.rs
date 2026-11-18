//! Note JS (N-17, N-18, N-19, N-10): a normal's alpha follows where it is used.
mod support;
use engine::config::{AlphaPolicy, Subtype};
use engine::shader::Shader;
use image::{ImageBuffer, Rgb, Rgba};
use sdc_gui::model::{Answer, AppModel};
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use support::*;

const WALL_N: &str = "test/wall_n.png";
const ROCK_N: &str = "landscape/rock_n.png";

/// `textures/landscape/rock.png`, `rock_n.png` and `rock_rmaos.png`: an automatic PBR Landscape set.
fn rock(dir: &Path) {
    let t = dir.join("textures/landscape");
    std::fs::create_dir_all(&t).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 60u8]))
        .save(t.join("rock.png"))
        .unwrap();
    bumpy_n(&t.join("rock_n.png"));
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(t.join("rock_rmaos.png"))
        .unwrap();
}

fn add_rmaos(t: &Path, m: &mut AppModel) {
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(t.join("wall_rmaos.png"))
        .unwrap();
    // A plan reads the folder again (the engine discovers the inputs each plan).
    m.replan();
    settle(m);
}

/// An `_n` like `normal`, with a specular alpha that changes per texel.
fn bumpy_n(path: &Path) {
    ImageBuffer::from_fn(64, 64, |x, y| {
        Rgba([
            if x % 2 == 0 { 204u8 } else { 51 },
            128,
            230,
            (x * 3 + y) as u8,
        ])
    })
    .save(path)
    .unwrap();
}

/// `textures/test/wall.png` and `wall_n.png`; returns the folder.
fn wall(dir: &Path) -> std::path::PathBuf {
    let t = dir.join("textures/test");
    std::fs::create_dir_all(&t).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(t.join("wall.png"))
        .unwrap();
    bumpy_n(&t.join("wall_n.png"));
    t
}

fn listed(m: &AppModel, key: &str) -> Option<String> {
    m.sets_doc.as_ref().unwrap().auto_normal().remove(key)
}

fn held(m: &AppModel, key: &str, field: &str) -> Option<serde_json::Value> {
    m.project.as_ref().unwrap().file_value(key, field)
}

fn settle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

/// The folder opened as the app opens it, with no export folder chosen yet (so nothing is saved for it).
fn opened_without_export(dir: &Path) -> AppModel {
    let mut m = AppModel::new(sdc_gui::settings::GuiSettings::default(), None, session());
    m.open_folder(dir.join("textures"));
    settle(&mut m);
    m
}

/// N-26 (FI4): the listing alone, on a folder opened with no export folder chosen yet, asks nothing and leaves no
/// recovery text; choosing the export folder saves it with `sdc-project.toml`.
#[test]
fn the_listing_alone_is_an_automatic_change() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_without_export(d.path());
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    assert!(
        m.sets_doc.as_ref().unwrap().is_dirty(),
        "the listing is unsaved"
    );
    assert!(m.only_automatic_unsaved());
    assert!(!m.prompts_unsaved());
    assert!(
        m.recovery_snapshot().is_empty(),
        "{:?}",
        m.recovery_snapshot()
    );
    m.set_export_folder(d.path().join("export"));
    settle(&mut m);
    assert!(!m.sets_doc.as_ref().unwrap().is_dirty());
    let saved = std::fs::read_to_string(d.path().join("textures/sdc-project.toml")).unwrap();
    assert!(saved.contains("auto_normal"), "{saved}");
    assert!(m.only_automatic_unsaved());
}

/// N-26, review 2 Important 1, review 3 Minor 2: the listing snapshot ends at a save. With no export folder chosen
/// (so the Undo returns to the very text the snapshot holds): Save, a user edit to `sdc-project.toml`, Save, Undo. The
/// document is back at the text the recording left, but that is now the user's unsaved change: it is asked about,
/// named in the dialog's file list (`files_hint` reads `sets_user_dirty`), and no save-if-clean site writes it.
#[test]
fn a_save_ends_the_listing_snapshot() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_without_export(d.path());
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    assert!(
        m.sets_listing_only(),
        "precondition: the snapshot holds the listing's text"
    );
    assert!(m.save().is_ok());
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert!(m.save().is_ok());
    let file = d.path().join("textures/sdc-project.toml");
    let on_disk = std::fs::read_to_string(&file).unwrap();
    m.undo();
    settle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().is_dirty());
    assert!(!m.sets_listing_only());
    assert!(m.sets_user_dirty(), "the dialog names sdc-project.toml");
    assert!(m.prompts_unsaved());
    assert!(!m.only_automatic_unsaved());
    m.set_export_folder(d.path().join("export"));
    settle(&mut m);
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        on_disk,
        "no save-if-clean write"
    );
}

/// N-27: a user's sub-type edit of a listed normal to another value leaves `sdc-project.toml` as it was (the fast
/// partial plan is kept); the key no longer counts as untouched.
#[test]
fn a_users_edit_of_a_listed_normal_is_files_only() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    assert!(
        !m.sets_doc.as_ref().unwrap().is_dirty(),
        "saved with the export folder"
    );
    let rev = m.sets_doc.as_ref().unwrap().rev();
    m.set_field(WALL_N, "subtype", json!("pbr_normal"));
    settle(&mut m);
    assert_eq!(
        m.sets_doc.as_ref().unwrap().rev(),
        rev,
        "no write to sdc-project.toml"
    );
    assert_eq!(
        listed(&m, WALL_N).as_deref(),
        Some("normal"),
        "stale, not untouched"
    );
    assert_eq!(m.untouched_normal(WALL_N), None);
}

/// N-27: typing the listed word itself makes the value the user's: the key leaves the list.
#[test]
fn typing_the_listed_word_takes_the_key_off() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    assert_eq!(listed(&m, WALL_N), None);
    assert_eq!(m.untouched_normal(WALL_N), None);
}

/// G1: the recorder wrote `normal` while the set was regular, and listed it; an `_rmaos` arrives and the set is PBR by
/// evidence, so the listed normal becomes `pbr_normal` and its alpha is dropped silently, in the recorder's own step.
#[test]
fn a_recorded_normal_follows_its_set_into_pbr() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        held(&m, WALL_N, "subtype"),
        Some(json!("normal")),
        "FC1 recorded it"
    );
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    add_rmaos(&t, &mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    let j = m.item(WALL_N).and_then(|i| i.job.clone()).expect("job");
    assert_eq!(j.profile.subtype, Subtype::PbrNormal);
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Remove);
    assert!(!j
        .warnings
        .iter()
        .any(|w| w.code == engine::codes::Code::LossAlphaData));
    assert!(
        m.undo_label().is_some_and(|l| l.ends_with("automatically")),
        "{:?}",
        m.undo_label()
    );
}

/// An Undo of the follow step stays undone: the next settled plan does not retype the normal again (`normal-follow`
/// marker in `History::auto_written`).
#[test]
fn an_undone_follow_stays_undone() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    add_rmaos(&t, &mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    m.undo();
    settle(&mut m);
    m.replan();
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    assert!(m.can_redo(), "the undone step is still offered again");
}

/// A `normal` the user typed is never retyped (the owner's "data treatment" by type), a pin.
#[test]
fn a_typed_normal_stays_when_its_set_turns_pbr() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    add_rmaos(&t, &mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
}

/// N-19: a never-recorded `_n` in an automatic PBR Landscape set is recorded `pbr_normal` and listed, in the
/// recording's own step; its alpha is dropped silently.
#[test]
fn a_landscape_n_in_a_pbr_landscape_set_is_recorded_pbr_normal() {
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    let m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, ROCK_N).as_deref(), Some("pbr_normal"));
    let j = m.item(ROCK_N).and_then(|i| i.job.clone()).expect("job");
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Remove);
    assert!(!j
        .warnings
        .iter()
        .any(|w| w.code == engine::codes::Code::LossAlphaData));
}

/// N-19, a pin: a landscape `_n` whose alpha policy the user wrote is recorded as the engine suggests and stays.
#[test]
fn a_landscape_n_with_the_users_alpha_keeps_its_type() {
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"landscape/rock_n.png\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
    m.replan();
    settle(&mut m);
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
}

/// A pin: with `sdc-project.toml` read-only the type could not be listed, so the recorder writes the engine's own
/// type, as before.
#[test]
fn a_read_only_sets_document_gets_no_pbr_normal() {
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    std::fs::write(d.path().join("textures/sdc-project.toml"), "[project\n").unwrap();
    let m = opened_without_export(d.path());
    assert!(
        m.sets_doc.as_ref().unwrap().read_only(),
        "precondition: unreadable sdc-project.toml"
    );
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
}
