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

/// G4 (N-17): an explicit Advanced Skin pick with no skin evidence types the untouched normal `pbr_normal`. A pin: it
/// passes with or without the pick's own write, because the wall's diffuse triggers the diffuse Switch inside the
/// same step; `an_advanced_skin_pick_without_a_diffuse_types_the_normal_in_its_own_step` isolates the pick.
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
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("normal")), Some("normal".into()), false)
    );
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

/// The three facts of a key: the held `[files]` sub-type, its `auto_normal` word, and whether `auto_pbr` lists it.
fn facts(m: &AppModel, key: &str) -> (Option<serde_json::Value>, Option<String>, bool) {
    (
        held(m, key, "subtype"),
        listed(m, key),
        m.sets_doc
            .as_ref()
            .unwrap()
            .auto_pbr()
            .iter()
            .any(|k| k == key),
    )
}

/// The wall with its `_rmaos` added after the open: the recorder followed, so its normal is `pbr_normal` and listed.
fn followed_wall(dir: &Path) -> AppModel {
    let t = wall(dir);
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    add_rmaos(&t, &mut m);
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false)
    );
    m
}

/// N-17 (N6-c, N6-d): an Eye pick takes a followed `pbr_normal` to `msn` (Eye accepts nothing else) and leaves its
/// listed word; the way back from `msn` is the normal rule's write, listed, and never in `auto_pbr`.
#[test]
fn eye_from_a_followed_normal_is_msn_and_the_way_back_is_listed() {
    let word = |s: &str| (Some(json!(s)), Some(s.to_string()), false);
    for (leg, want) in [
        (Some(Shader::Default), word("normal")),
        (Some(Shader::Pbr), word("pbr_normal")),
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut m = followed_wall(d.path());
        m.set_shader("test/wall", Some(Shader::Eye));
        settle(&mut m);
        assert_eq!(
            facts(&m, WALL_N),
            (Some(json!("msn")), Some("pbr_normal".into()), false),
            "Eye"
        );
        m.set_shader("test/wall", leg);
        settle(&mut m);
        assert_eq!(facts(&m, WALL_N), want, "{leg:?}");
    }
}

/// N-17 (N6-d): Eye, then Auto: the Eye step leaves `msn` with the parked word; Auto clears once, the set is PBR by
/// its `_rmaos`, so the recorder then follows and the key ends `pbr_normal`, listed, off `auto_pbr`.
#[test]
fn eye_then_auto_leaves_the_lists_agreeing_with_the_key() {
    let d = tempfile::tempdir().unwrap();
    let mut m = followed_wall(d.path());
    m.set_shader("test/wall", Some(Shader::Eye));
    settle(&mut m);
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("msn")), Some("pbr_normal".into()), false)
    );
    m.set_shader("test/wall", None);
    assert_eq!(facts(&m, WALL_N), (None, None, false), "Auto clears once");
    settle(&mut m);
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false)
    );
}

/// B1 (N6-i): Eye, then Auto clears the key once: a sub-type the user wrote under the extension-less `[files]` spelling
/// is still there.
#[test]
fn auto_after_eye_keeps_the_users_extensionless_subtype() {
    let d = tempfile::tempdir().unwrap();
    let mut m = followed_wall(d.path());
    m.set_shader("test/wall", Some(Shader::Eye));
    settle(&mut m);
    m.edit_docs("user glow", None, true, |p, _| {
        p.set_file_value("test/wall_n", "subtype", &json!("glow"))
    });
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("msn")));
    m.set_shader("test/wall", None);
    assert_eq!(held(&m, "test/wall_n", "subtype"), Some(json!("glow")));
}

/// The key of the member that owns the set's Normal slot.
fn normal_owner(m: &AppModel, id: &str) -> Option<String> {
    let v = sdc_gui::model::sets::set_views(m)
        .into_iter()
        .find(|v| v.id == id)?;
    match v.slots.get(&sdc_gui::model::sets::SlotKind::Normal) {
        Some(sdc_gui::model::sets::SlotFill::Own { item }) => Some(item.clone()),
        _ => None,
    }
}

/// The sets document's pick differs from the last plan's: the plan has not seen it yet.
fn assert_plan_pending(m: &AppModel, id: &str, choice: Shader) {
    let v = sdc_gui::model::sets::set_views(m)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap();
    assert_eq!(v.shader_choice, Some(choice), "the live choice");
    assert!(
        !v.shader
            .as_ref()
            .is_some_and(|p| p.provenance == engine::shader::ShaderOrigin::Set),
        "the last plan has not seen the pick: {:?}",
        v.shader
    );
}

/// B1 (N6-h): PBR, then Auto before the PBR plan lands: the Auto leg reads the live pick, so the key is cleared and
/// unlisted, never held and listed `pbr_normal`.
#[test]
fn auto_right_after_a_pbr_pick_clears_the_normal() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(listed(&m, WALL_N).as_deref(), Some("normal"));
    m.set_shader("test/wall", Some(Shader::Pbr));
    assert_plan_pending(&m, "test/wall", Shader::Pbr);
    m.set_shader("test/wall", None);
    assert_eq!(facts(&m, WALL_N), (None, None, false));
}

/// B4 (N6-h): an automatically PBR set with a PBR diffuse and a listed `pbr_normal`: PBR is picked, then the diffuse
/// is decided `diffuse` before the plan lands. The Revert reads the live pick and keeps the normal.
#[test]
fn a_diffuse_revert_before_the_plan_lands_reads_the_live_pick() {
    let d = tempfile::tempdir().unwrap();
    let t = wall(d.path());
    ImageBuffer::from_fn(64, 64, |x, _| Rgb([(x * 4) as u8, 200, 40]))
        .save(t.join("wall_g.png"))
        .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    add_rmaos(&t, &mut m);
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    // The Revert has something to do: a glow map an earlier Switch typed `pbr_emissive`, listed in `auto_pbr`.
    m.edit_docs("switched", None, true, |p, s| {
        p.set_file_value("test/wall_g.png", "subtype", &json!("pbr_emissive"));
        s.set_auto_pbr(&["test/wall_g.png".to_string()]);
    });
    m.set_shader("test/wall", Some(Shader::Pbr));
    assert_plan_pending(&m, "test/wall", Shader::Pbr);
    assert_eq!(
        held(&m, "test/wall_g.png", "subtype"),
        Some(json!("pbr_emissive"))
    );
    m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
    assert_ne!(
        held(&m, "test/wall_g.png", "subtype"),
        Some(json!("pbr_emissive")),
        "the Revert ran"
    );
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false)
    );
}

/// N6-l: a key in both lists is the normal rule's alone in a manual Revert: under a settled explicit PBR pick it stays
/// `pbr_normal`, listed, off `auto_pbr`; without the pick it goes back to `tangent_back`, relisted, off `auto_pbr`.
#[test]
fn a_revert_leaves_a_key_in_both_lists_to_the_normal_rule() {
    for explicit in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let mut m = followed_wall(d.path());
        m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
        settle(&mut m);
        if explicit {
            m.set_shader("test/wall", Some(Shader::Pbr));
            settle(&mut m);
        }
        m.edit_docs("both lists", None, true, |_, s| {
            s.set_auto_pbr(&[WALL_N.to_string()])
        });
        m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
        let want = if explicit { "pbr_normal" } else { "normal" };
        assert_eq!(
            facts(&m, WALL_N),
            (Some(json!(want)), Some(want.into()), false),
            "explicit {explicit}"
        );
    }
}

/// B3 (N6-j): `alpha.policy` under the extension-less spelling `wall_n` and another field under `wall_n.png`: the
/// engine applies both, so the alpha is the user's. The Switch leaves an inferred normal alone (acceptance off), and a
/// listed `normal` is not untouched.
#[test]
fn an_alpha_under_the_extensionless_spelling_is_the_users() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"test/wall_n\"]\nalpha.policy = \"data\"\n\n[files.\"test/wall_n.png\"]\nformat = \"bc7\"\n",
    )
    .unwrap();
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..sdc_gui::settings::GuiSettings::default()
        },
        None,
        session(),
    );
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().join("textures"));
    settle(&mut m);
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), (None, None, false));

    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::Normal));
    m.edit_docs("alpha", None, true, |p, _| {
        p.set_file_value("test/wall_n", "alpha.policy", &json!("data"))
    });
    assert_eq!(m.untouched_normal(WALL_N), None);
}

/// B2 (N6-e): a fuzz-map set where `wall_msn.png` sorts before `wall_n.png`: `normal` typed by the user on `_n` is
/// unchanged by an explicit PBR pick and by a regular pick. A pin of the guard on a real pick. The positive legacy
/// exception: a `pbr_normal` an older pick listed in `auto_pbr` is cleared when the set leaves PBR.
#[test]
fn a_typed_normal_beside_an_msn_survives_picks_and_an_older_listed_one_is_cleared() {
    let build = |d: &Path| {
        let t = fuzz_wall(d);
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
            .save(t.join("wall_msn.png"))
            .unwrap();
        let mut m = opened_as_the_app(d.join("textures"), d.join("export"));
        m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
        settle(&mut m);
        assert_eq!(
            normal_owner(&m, "test/wall").as_deref(),
            Some("test/wall_msn.png"),
            "the `_msn` owns the Normal slot; `_n` is the secondary member"
        );
        m
    };
    let d = tempfile::tempdir().unwrap();
    let mut m = build(d.path());
    let typed = facts(&m, WALL_N);
    assert_eq!(typed, (Some(json!("normal")), None, false));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), typed, "explicit PBR pick");
    m.set_shader("test/wall", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), typed, "regular pick");

    let d = tempfile::tempdir().unwrap();
    let mut m = build(d.path());
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    m.edit_docs("older pick", None, true, |p, s| {
        p.set_file_value(WALL_N, "subtype", &json!("pbr_normal"));
        s.set_auto_pbr(&[WALL_N.to_string()]);
    });
    m.set_shader("test/wall", Some(Shader::Default));
    assert_eq!(facts(&m, WALL_N), (None, None, false), "the leave step");
}

/// N6-k: the diffuse's Switch governs every free member that is inferred or untouched-listed, not only the Normal
/// slot's: in the `_msn`/`_n` set a recorder-listed `normal` on the secondary `_n` switches to `pbr_normal` and stays
/// in `auto_normal` (never `auto_pbr`); with acceptance off the inferred one switches too. The Revert takes both back.
#[test]
fn the_switch_governs_every_free_member_not_only_the_normal_slot() {
    for accept_on in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let t = wall(d.path());
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
            .save(t.join("wall_msn.png"))
            .unwrap();
        let mut m = AppModel::new(
            sdc_gui::settings::GuiSettings {
                export_folder: Some(d.path().join("export")),
                ..sdc_gui::settings::GuiSettings::default()
            },
            None,
            session(),
        );
        if !accept_on {
            m.accept = engine::policy::AcceptSuggestions::None;
        }
        m.open_folder(d.path().join("textures"));
        settle(&mut m);
        assert_eq!(
            normal_owner(&m, "test/wall").as_deref(),
            Some("test/wall_msn.png"),
            "the `_msn` owns the Normal slot; `_n` is the secondary member"
        );
        m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
        let (switched, back) = if accept_on {
            (
                (
                    Some(json!("pbr_normal")),
                    Some("pbr_normal".to_string()),
                    false,
                ),
                (Some(json!("normal")), Some("normal".to_string()), false),
            )
        } else {
            ((Some(json!("pbr_normal")), None, true), (None, None, false))
        };
        assert_eq!(facts(&m, WALL_N), switched, "accept on {accept_on}");
        settle(&mut m);
        m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
        assert_eq!(facts(&m, WALL_N), back, "accept on {accept_on}");
    }
}
/// A fuzz-map wall: `wall_g` and `wall_fuzz` beside the normal, so an explicit PBR pick runs R5's fuzz step.
fn fuzz_wall(dir: &Path) -> std::path::PathBuf {
    let t = wall(dir);
    ImageBuffer::from_fn(64, 64, |x, _| Rgb([(x * 4) as u8, 200, 40]))
        .save(t.join("wall_g.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
        .save(t.join("wall_fuzz.png"))
        .unwrap();
    t
}

/// N-17 (N6-e): the typed-normal guard covers every member, not only the Normal slot's. A set whose second `_n`-like
/// file the user typed `normal` (no job: the two compete for the slot, so no real plan reaches R5 with it) keeps it:
/// the pick's resolution drops every R5 entry for it, but a legacy leave clear of a key in `auto_pbr`.
#[test]
fn the_typed_normal_guard_covers_every_member() {
    const SECOND: &str = "test/wall_normal.png";
    let d = tempfile::tempdir().unwrap();
    let t = fuzz_wall(d.path());
    bumpy_n(&t.join("wall_normal.png"));
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(SECOND, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    let r5 = vec![
        (SECOND.to_string(), Some(json!("pbr_normal")), true),
        (
            "test/wall_fuzz.png".to_string(),
            Some(json!("pbr_fuzz")),
            true,
        ),
        (SECOND.to_string(), None, true),
    ];
    let kept = m.normal_plan_probe("test/wall", Some(Shader::Pbr), r5);
    // The fuzz map's entry stands; the typed member's value entry goes; its clear stands only for a key an older
    // pick listed in `auto_pbr` (this one is not listed).
    assert_eq!(
        kept,
        vec![(
            "test/wall_fuzz.png".to_string(),
            Some(json!("pbr_fuzz")),
            true
        )]
    );
}
/// N-28 (N6-e): the Switch reads the stored value. A `_n` typed `glow`, then typed `normal`, then the diffuse decided
/// before the plan settles: the `_n` stays `normal`, in no list.
#[test]
fn the_switch_reads_the_stored_type_of_a_member_the_plan_has_not_seen_yet() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide(WALL_N, Answer::Subtype(Subtype::Glow));
    settle(&mut m);
    m.decide(WALL_N, Answer::Subtype(Subtype::Normal));
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), (Some(json!("normal")), None, false));
}

/// N-17, N-19 (N6-f): an inferred normal whose `alpha.policy` the user wrote is never switched or listed by the
/// diffuse's Switch.
#[test]
fn the_switch_leaves_an_inferred_normal_with_the_users_alpha() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"test/wall_n.png\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..sdc_gui::settings::GuiSettings::default()
        },
        None,
        session(),
    );
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().join("textures"));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), (None, None, false));
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), (None, None, false));
}

/// N-17 (N6-c): a saved fuzz-map set under an explicit PBR pick whose `pbr_normal` is in both lists (the two lists
/// are read independently): Advanced Skin keeps it `pbr_normal` listed in `auto_normal` only; Default types it back
/// and lists that word; Auto clears it and unlists it.
#[test]
fn a_normal_in_both_lists_resolves_to_one_final_value_per_pick() {
    let word = |s: &str| (Some(json!(s)), Some(s.to_string()), false);
    for (shader, want) in [
        (Some(Shader::AdvancedSkin), word("pbr_normal")),
        (Some(Shader::Default), word("normal")),
        (None, (None, None, false)),
    ] {
        let d = tempfile::tempdir().unwrap();
        fuzz_wall(d.path());
        let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
        m.set_shader("test/wall", Some(Shader::Pbr));
        settle(&mut m);
        m.edit_docs("both lists", None, true, |p, s| {
            p.set_file_value(WALL_N, "subtype", &json!("pbr_normal"));
            s.set_auto_pbr(&[WALL_N.to_string()]);
            s.set_auto_normal(&std::collections::BTreeMap::from([(
                WALL_N.to_string(),
                "pbr_normal".to_string(),
            )]));
        });
        settle(&mut m);
        assert_eq!(m.untouched_normal(WALL_N), Some(Subtype::PbrNormal));
        m.set_shader("test/wall", shader);
        // The pick alone, before the next plan records anything.
        assert_eq!(facts(&m, WALL_N), want, "{shader:?}");
    }
}

/// N-17 (N6-g): the recorder follows, the user undoes it, picks PBR, and undoes the pick: the key is `normal`, the
/// recorder does not follow again, and Redo is still offered.
#[test]
fn undoing_a_pick_after_an_undone_follow_does_not_follow_again() {
    let d = tempfile::tempdir().unwrap();
    let mut m = followed_wall(d.path());
    m.undo();
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    m.undo();
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("normal")));
    assert!(m.can_redo(), "Redo is still offered");
}

/// N-28 (N6-g): a followed normal on a wall whose diffuse was recorded `diffuse`: Advanced Skin, then PBR. The pick
/// types the normal `pbr_normal` and the diffuse's Revert leaves it (the explicit pick is a PBR-set shader), in the
/// pick's one step; a settle adds none.
#[test]
fn advanced_skin_then_pbr_keeps_a_followed_normal_in_one_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = followed_wall(d.path());
    m.set_shader("test/wall", Some(Shader::AdvancedSkin));
    settle(&mut m);
    let before = m.history.undo.len();
    m.set_shader("test/wall", Some(Shader::Pbr));
    let after_pick = m.history.undo.len();
    assert_eq!(after_pick, before + 1, "one Shader step");
    settle(&mut m);
    assert_eq!(m.history.undo.len(), after_pick, "a settle adds none");
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false)
    );
}

/// G4 (N-17): an explicit Advanced Skin pick on a set with no diffuse types the untouched normal `pbr_normal` in
/// the pick's own step (no diffuse Switch, no recorder follow), listed in `auto_normal` only.
#[test]
fn an_advanced_skin_pick_without_a_diffuse_types_the_normal_in_its_own_step() {
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/test");
    std::fs::create_dir_all(&t).unwrap();
    bumpy_n(&t.join("wall_n.png"));
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("normal")), Some("normal".into()), false)
    );
    m.set_shader("test/wall", Some(Shader::AdvancedSkin));
    assert_eq!(m.undo_label(), Some("Shader"));
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false),
        "the pick itself, before any plan"
    );
}

/// B3 (N6-j): the engine lowercases both sides, so `alpha.policy` under `TEST/WALL_N` beside a `test/wall_n.png` table
/// is the user's: the Switch leaves the inferred normal alone (acceptance off) and `auto_pbr` does not gain it.
#[test]
fn an_alpha_under_another_case_of_the_key_is_the_users() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"TEST/WALL_N\"]\nalpha.policy = \"data\"\n\n[files.\"test/wall_n.png\"]\nformat = \"bc7\"\n",
    )
    .unwrap();
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..sdc_gui::settings::GuiSettings::default()
        },
        None,
        session(),
    );
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().join("textures"));
    settle(&mut m);
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    assert_eq!(facts(&m, WALL_N), (None, None, false));
}

/// B7 (N6-j): the user's `glow` and alpha under `TEST/WALL_N`, only `pbr_normal` under `test/wall_n.png`, the key in
/// both lists, an explicit PBR pick settled, a manual Revert: the key is not untouched, so the legacy loop clears the
/// `pbr_normal` (the user's `glow` shows through) and the normal rule keeps nothing.
#[test]
fn a_revert_clears_a_pbr_normal_whose_alpha_is_the_users_under_another_case() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"TEST/WALL_N\"]\nsubtype = \"glow\"\nalpha.policy = \"data\"\n\n[files.\"test/wall_n.png\"]\nsubtype = \"pbr_normal\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    settle(&mut m);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    m.edit_docs("both lists", None, true, |p, s| {
        assert_eq!(p.file_value(WALL_N, "subtype"), Some(json!("pbr_normal")));
        s.set_auto_pbr(&[WALL_N.to_string()]);
        s.set_auto_normal(&std::collections::BTreeMap::from([(
            WALL_N.to_string(),
            "pbr_normal".to_string(),
        )]));
    });
    assert_eq!(m.untouched_normal(WALL_N), None, "the alpha is the user's");
    m.decide("test/wall.png", Answer::Subtype(Subtype::Diffuse));
    assert_eq!(
        held(&m, WALL_N, "subtype"),
        Some(json!("glow")),
        "the legacy loop cleared the pbr_normal and the user's glow shows through"
    );
}

/// N-28 (the Switch keeps the module's promise): a `normal` persisted in both lists is `pbr_normal` after the Switch,
/// listed with that word, and off `auto_pbr`.
#[test]
fn the_switch_takes_an_untouched_key_off_auto_pbr() {
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.edit_docs("both lists", None, true, |_, s| {
        s.set_auto_pbr(&[WALL_N.to_string()])
    });
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("normal")), Some("normal".into()), true)
    );
    m.decide("test/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    assert_eq!(
        facts(&m, WALL_N),
        (Some(json!("pbr_normal")), Some("pbr_normal".into()), false)
    );
}

/// N-18: on a `pbr_normal` in a PBR set the texture panel's Alpha row offers Data; picking it writes
/// `alpha.policy = "data"`, raises no error, and survives retypes (the user's own, and shader picks).
#[test]
fn data_is_offered_on_a_pbr_normal_and_sticks() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    let d = tempfile::tempdir().unwrap();
    wall(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, WALL_N, "subtype"), Some(json!("pbr_normal")));
    m.select_item(WALL_N);
    let mut h = egui_kittest::Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    {
        let combos: Vec<_> = h
            .query_all_by_role(Role::ComboBox)
            .filter(|n| n.accesskit_node().value().as_deref() == Some("Remove"))
            .collect();
        assert_eq!(combos.len(), 1, "the Alpha combo shows Remove");
        assert!(
            !combos[0].accesskit_node().is_disabled(),
            "the Alpha row is on"
        );
        combos[0].click();
    }
    h.run();
    {
        let data: Vec<_> = h.query_all_by_label("Data").collect();
        assert_eq!(data.len(), 1, "one Data option, in the open Alpha list");
        data[0].click();
    }
    h.step();
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run();
    let m = h.state_mut();
    assert_eq!(held(m, WALL_N, "alpha.policy"), Some(json!("data")));
    let j = m.item(WALL_N).and_then(|i| i.job.clone()).unwrap();
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Data);
    assert!(
        j.warnings
            .iter()
            .all(|w| w.severity != engine::codes::Severity::Error),
        "{:?}",
        j.warnings
    );
    m.set_field(WALL_N, "subtype", json!("normal"));
    settle(m);
    m.set_field(WALL_N, "subtype", json!("pbr_normal"));
    settle(m);
    assert_eq!(
        held(m, WALL_N, "alpha.policy"),
        Some(json!("data")),
        "the user's retypes"
    );
    m.set_shader("test/wall", Some(Shader::Default));
    settle(m);
    m.set_shader("test/wall", Some(Shader::Pbr));
    settle(m);
    assert_eq!(
        held(m, WALL_N, "alpha.policy"),
        Some(json!("data")),
        "the picks"
    );
    let j = m.item(WALL_N).and_then(|i| i.job.clone()).unwrap();
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Data);
}
