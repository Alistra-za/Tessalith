//! Note JS (N-17, N-18, N-19, N-10): a normal's alpha follows where it is used.
mod support;
use engine::config::{AlphaPolicy, Subtype};
use engine::shader::Shader;
use image::{ImageBuffer, Luma, Rgb, Rgba};
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

/// G2, G3 (N-17): an explicit PBR pick types the untouched normal `pbr_normal` in the pick's own step, a regular pick
/// types it back to `normal`, and each is one Undo. No fuzz map is involved.
#[test]
fn a_pick_moves_an_untouched_normal_in_and_out_of_pbr() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    assert_eq!(m.undo_label(), Some("Shader"), "the pick's own step");
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.undo();
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
}

/// N-17: back to Auto after an explicit PBR pick clears the listed `pbr_normal`; Auto (a regular set) then reads
/// `normal`.
#[test]
fn auto_after_a_pbr_pick_takes_the_normal_back() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    m.set_shader("test/wall", None);
    settle(&mut m);
    let j = m.item(WALL_N).and_then(|i| i.job.clone()).unwrap();
    assert_eq!(j.profile.subtype, Subtype::Normal);
    assert_ne!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
}

/// G4 (N-17): an explicit Advanced Skin pick with no skin evidence types the untouched normal `pbr_normal`. The
/// type alone is a pin (the recorder's follow pass reaches it too); the undo label tells the pick's own step from the
/// recorder's.
#[test]
fn an_advanced_skin_pick_types_the_untouched_normal() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.set_shader("test/wall", Some(Shader::AdvancedSkin));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(m.undo_label(), Some("Shader"), "the pick's own step");
}

/// N-17 (N6-a): the pick's rule owns tangent-space values only. An Eye pick still takes an untouched `normal` to `msn`
/// (FC1 R5), a regular pick takes it back, and the explicit PBR pick then types and lists `pbr_normal`.
#[test]
fn an_eye_pick_then_a_regular_pick_then_pbr_keeps_the_normal_moving() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    m.set_shader("test/wall", Some(Shader::Eye));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("msn")));
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
}

/// N-19: an explicit regular pick takes an untouched landscape `_n` back to the engine's type, `landscape_normal`.
#[test]
fn a_regular_pick_takes_a_landscape_n_back_to_landscape_normal() {
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("pbr_normal")));
    m.set_shader("landscape/rock", Some(Shader::Landscape));
    settle(&mut m);
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
    let j = m.item(ROCK_N).and_then(|i| i.job.clone()).unwrap();
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Data);
}

/// N-17, N-19: a pick never retypes a normal the user typed (plain or landscape), nor one whose alpha the user wrote.
#[test]
fn a_pick_leaves_the_users_normal_alone() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));

    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.set_alpha(WALL_N, AlphaPolicy::Data);
    settle(&mut m);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));

    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(ROCK_N, Answer::Subtype(Subtype::LandscapeNormal));
    settle(&mut m);
    m.set_shader("landscape/rock", Some(Shader::PbrLandscape));
    settle(&mut m);
    assert_eq!(held(&m, ROCK_N, "subtype"), Some(json!("landscape_normal")));
}

/// N-20: a recorded `normal` from before this change (in `sdc.toml`, not listed) reads as typed: a PBR pick keeps it.
#[test]
fn an_unlisted_recorded_normal_survives_a_pbr_pick() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"test/wall_n.png\"]\nsubtype = \"normal\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N), None);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
}

/// N-17: in a set holding a fuzz map (read Glow, as Fuzz-27's set is), a `normal` the user typed equal to the
/// recorder's is never retyped by an explicit PBR pick (`untouched_for_shader`'s `pbr_pick` arm) nor back.
#[test]
fn a_typed_normal_in_a_fuzz_map_set_stays() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    ImageBuffer::from_fn(64, 64, |x, _| Rgb([(x * 4) as u8, 200, 40]))
        .save(t.join("wall_g.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
        .save(t.join("wall_fuzz.png"))
        .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
}

/// N-28: a typed `normal` stays when the diffuse is decided `pbr_albedo` (`pbr_follow`'s Switch), and when the set is
/// picked Advanced Skin (its diffuse step runs the same Switch).
#[test]
fn the_diffuse_switch_leaves_a_typed_normal() {
    for skin in [false, true] {
        let d = tempfile::tempdir().unwrap();
        wall(d.path());
        let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
        m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
        settle(&mut m);
        if skin {
            m.set_shader("test/wall", Some(Shader::AdvancedSkin));
        } else {
            m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
        }
        settle(&mut m);
        assert_eq!(
            held(&m, WALL_N, "subtype"),
            Some(json!("normal")),
            "skin {skin}"
        );
        assert!(!m
            .sets_doc
            .as_ref()
            .unwrap()
            .auto_pbr()
            .iter()
            .any(|k| k == WALL_N));
    }
}

/// N-28: the Switch types an untouched listed normal `pbr_normal`, keeps it in `auto_normal` (never `auto_pbr`), and
/// the diffuse leaving PBR takes it back.
#[test]
fn the_diffuse_switch_moves_an_untouched_normal_and_takes_it_back() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
    assert!(!m
        .sets_doc
        .as_ref()
        .unwrap()
        .auto_pbr()
        .iter()
        .any(|k| k == WALL_N));
    m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
}

/// N-17: an explicit regular pick takes a followed `pbr_normal` back to `normal` and clears the follow's marker, so
/// the set returning to PBR on Auto in the same session follows again.
#[test]
fn a_regular_pick_clears_the_follow_marker() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    add_rmaos(&t, &mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.set_shader("test/wall", None);
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("pbr_normal"));
}
