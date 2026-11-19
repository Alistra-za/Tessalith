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

/// The files of two sets: `textures/pbr/iron.png`, `iron_n.png` (varying alpha), `iron_rmaos.png`: a PBR set;
/// `textures/armor/steel.png` with its own `steel_n.png`: a regular set.
fn two_sets_files(dir: &Path) {
    let pbr = dir.join("textures/pbr");
    let armor = dir.join("textures/armor");
    for f in [&pbr, &armor] {
        std::fs::create_dir_all(f).unwrap();
    }
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([120u8, 90, 60]))
        .save(pbr.join("iron.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([90u8, 90, 100]))
        .save(armor.join("steel.png"))
        .unwrap();
    bumpy_n(&pbr.join("iron_n.png"));
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(armor.join("steel_n.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(pbr.join("iron_rmaos.png"))
        .unwrap();
}

/// `two_sets_files` without `steel_n.png`: `armor/steel`'s Normal slot is empty, so a Share into it changes only its
/// borrow line. (N8-r: a Share that replaces `steel_n.png`, an automatic member, puts that file in no set, which
/// changes the borrower's membership, so that step writes nothing.)
fn open_slot_files(dir: &Path) {
    two_sets_files(dir);
    std::fs::remove_file(dir.join("textures/armor/steel_n.png")).unwrap();
}

/// `open_slot_files`, opened as the app opens them.
fn two_sets(dir: &Path) -> AppModel {
    open_slot_files(dir);
    let m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    let ids: Vec<String> = sdc_gui::model::sets::views(&m)
        .iter()
        .map(|v| v.id.clone())
        .collect();
    assert!(
        ids.iter().any(|i| i == "pbr/iron") && ids.iter().any(|i| i == "armor/steel"),
        "{ids:?}"
    );
    m
}

const IRON_N: &str = "pbr/iron_n.png";

/// Share `item` into the empty Normal slot of set `into` as the Sets column does (`drop_choice` returns `Joined`), and
/// let the plan land. A filled slot (`AskReplace`) is `share_replacing`'s.
fn share(m: &mut AppModel, item: &str, into: &str) {
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    match m.drop_choice(item, into, DropChoice::Share) {
        DropOutcome::Joined => {}
        other => panic!("not an empty slot: {other:?}"),
    }
    settle(m);
}

/// Share `item` over the filled Normal slot of set `into` and confirm Replace, as the Sets column does
/// (`drop_choice` returns `AskReplace`, sets.rs `drop_choice`), and let the plan land.
fn share_replacing(m: &mut AppModel, item: &str, into: &str) {
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    match m.drop_choice(item, into, DropChoice::Share) {
        DropOutcome::AskReplace { .. } => m
            .drop_replace(item, into, DropChoice::Share)
            .expect("shared"),
        other => panic!("not a filled slot: {other:?}"),
    }
    settle(m);
}

/// N8-m, N8-r (round 4; this test expected `data` and a list entry before): Share `iron_n` over `armor/steel`'s own
/// `steel_n`, an automatic member, and confirm Replace. The Replace puts `steel_n` in no set (`[project] no_set`), so
/// the borrower's membership changed in the step: it is not known, and the share writes and lists nothing in its step,
/// nor when its plan lands (N8-g). A later step that changes no membership and makes the share newly regular (here
/// the borrower picked PBR, then Default) writes it.
#[test]
fn a_share_replacing_an_automatic_normal_writes_nothing_in_its_step() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    let no_set = |m: &AppModel| m.sets_doc.as_ref().unwrap().no_set();
    assert!(no_set(&m).is_empty(), "precondition");
    share_replacing(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        no_set(&m),
        vec!["armor/steel_n.png".to_string()],
        "precondition: the Replace put the automatic member in no set"
    );
    assert!(
        m.undo_label().is_some_and(|l| l.starts_with("Share into")),
        "precondition: one step"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "a step that changed the borrower's membership writes nothing"
    );
    assert!(auto_data(&m).is_empty(), "and lists nothing");
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    m.set_shader("armor/steel", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
}

/// N8-r: the borrower's own diffuse goes to no set (its view is left with no file, and the listed `data` stays: the
/// step changed its membership); then Back to its automatic set takes it out of no set. The views still show the
/// borrower with no file, but it is not gone: the step that takes a file out of no set knows no set, so it clears
/// nothing, and once the plan lands the borrower reads the shared normal again.
#[test]
fn a_file_back_from_no_set_keeps_the_shared_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.remove_member("armor/steel", "armor/steel.png").unwrap();
    settle(&mut m);
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "armor/steel" && v.members.is_empty()),
        "precondition: the borrower has no file"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the remove's step changed the borrower's membership and kept it"
    );
    m.back_to_automatic_set(&["armor/steel.png".to_string()])
        .unwrap();
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a step that takes a file out of no set clears nothing"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "armor/steel" && !v.members.is_empty()),
        "the borrower has its file back"
    );
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N-10 (a): sharing a PBR set's normal into a regular set's empty Normal slot writes `alpha.policy = "data"` on it in
/// the share's step. (b): the regular set picked PBR takes it back in that step; a regular pick writes it again.
#[test]
fn a_shared_normal_keeps_its_alpha_while_a_regular_set_uses_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    assert_eq!(held(&m, IRON_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert!(
        m.undo_label().is_some_and(|l| l.starts_with("Share into")),
        "one step"
    );
    assert_eq!(
        m.sets_doc.as_ref().unwrap().auto_data(),
        vec![IRON_N.to_string()]
    );
    let j = m.item(IRON_N).and_then(|i| i.job.clone()).unwrap();
    assert_eq!(j.profile.alpha.policy, AlphaPolicy::Data);
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "no regular set uses it now"
    );
    m.set_shader("armor/steel", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N-10 (b), N8-p: the user's Remove on a listed key takes the entry off in the Remove's own step (its table no longer
/// holds `data`), and a value the user wrote over the step's is never taken back, Data included.
#[test]
fn the_users_alpha_on_a_shared_normal_is_never_taken_back() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the step wrote it"
    );
    m.set_alpha(IRON_N, AlphaPolicy::Remove);
    assert!(
        m.sets_doc.as_ref().unwrap().auto_data().is_empty(),
        "the entry leaves the list in the Remove's step"
    );
    assert_eq!(m.undo_label(), Some("Alpha"));
    settle(&mut m);
    m.set_alpha(IRON_N, AlphaPolicy::Data);
    settle(&mut m);
    assert!(
        m.sets_doc.as_ref().unwrap().auto_data().is_empty(),
        "the user's Data now"
    );
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N-34 (review 2 Minor 3): picking Data on a key the share step set to `data` makes it the user's: the key leaves
/// `auto_data` in the pick's step, and the share ending no longer takes it back.
#[test]
fn picking_data_on_a_listed_key_makes_it_the_users() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        m.sets_doc.as_ref().unwrap().auto_data(),
        vec![IRON_N.to_string()],
        "precondition: the share step listed it"
    );
    m.set_alpha(IRON_N, AlphaPolicy::Data);
    settle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().auto_data().is_empty());
    assert_eq!(m.undo_label(), Some("Alpha"));
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N-10: a share into another PBR set writes nothing (only ever in PBR sets: dropped, silently).
#[test]
fn a_share_between_pbr_sets_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
}

/// N-10: clearing the regular set's slot 2 ends the last share: `data` goes in that step, and one Undo brings it back.
#[test]
fn clearing_the_share_takes_data_back_in_one_step() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert_eq!(m.undo_label(), Some("Clear slot 2 of armor/steel"));
    m.undo();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N-10 (JN1): a two-hop chain (written as a project from before JN3 holds one) from a regular set through a PBR set
/// to the owner is a share into a regular set; clearing the regular set's slot ends it.
#[test]
fn a_chain_into_a_regular_set_keeps_alpha() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mid = d.path().join("textures/mid");
    let plate = d.path().join("textures/plate");
    for f in [&mid, &plate] {
        std::fs::create_dir_all(f).unwrap();
    }
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([70u8, 70, 70]))
        .save(mid.join("bolt.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(mid.join("bolt_rmaos.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([60u8, 60, 80]))
        .save(plate.join("plate.png"))
        .unwrap();
    let mut m = two_sets(d.path());
    m.edit_docs("seed", None, true, |_, s| {
        s.set_borrow("mid/bolt", 2, Some("pbr/iron"));
        s.set_borrow("plate/plate", 2, Some("mid/bolt"));
    });
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    m.set_slot_command("plate/plate", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "only a PBR set shares it now"
    );
}

/// N-10: the owner picked regular ends the PBR-into-regular share: `data` goes in the pick's step (and the owner's
/// untouched `pbr_normal` goes back to `normal`, Task 6).
#[test]
fn the_owner_turning_regular_takes_data_back() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.set_shader("pbr/iron", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert_eq!(held(&m, IRON_N, "subtype"), Some(json!("normal")));
}

/// Review 2 Important 3: a regular owner whose untouched `normal` a regular set shares; picking PBR on the owner
/// retypes it `pbr_normal` in that step (Task 6), so "already `data`" is read from the step's new type, not the last
/// plan's: `data` is written in the "Shader" step.
#[test]
fn the_owner_turning_pbr_writes_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.set_shader("pbr/iron", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "subtype"), Some(json!("normal")));
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "a regular owner's normal is not written"
    );
    m.set_shader("pbr/iron", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "subtype"), Some(json!("pbr_normal")));
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(
        m.sets_doc.as_ref().unwrap().auto_data(),
        vec![IRON_N.to_string()]
    );
    assert_eq!(m.undo_label(), Some("Shader"));
}

/// Review 2 Important 2: removing the owner's Normal-slot file from its set hands it over to the set that shares it
/// (Note JN5), a regular set, so no regular set shares a PBR owner's normal: `data` goes in the remove's step. (The
/// handed `pbr_normal` stays `pbr_normal`: spec limit 2.)
#[test]
fn removing_the_owners_normal_takes_data_back() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.remove_member("pbr/iron", IRON_N).unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert!(m.sets_doc.as_ref().unwrap().auto_data().is_empty());
}

/// Review 1 Minor 7: an owner normal already planned `data` (a typed `normal`, whose built-in alpha is `data`) gets no
/// write.
#[test]
fn an_owner_normal_already_data_gets_no_write() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.decide(IRON_N, Answer::Subtype(Subtype::Normal));
    settle(&mut m);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert!(m.sets_doc.as_ref().unwrap().auto_data().is_empty());
}

/// Review 4 Important 1: an entry whose files are all gone (no files; it borrows `pbr/iron`'s slot 2) reads no
/// texture, so it neither shares the normal nor holds back the clear when the last regular set stops sharing it.
#[test]
fn a_set_with_no_files_does_not_hold_back_a_clear() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.edit_docs("seed", None, true, |_, s| {
        s.set_borrow("ghost/ghost", 2, Some("pbr/iron"))
    });
    settle(&mut m);
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "ghost/ghost" && v.members.is_empty()),
        "precondition: the entry is shown with no files"
    );
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
}

/// N-40 (review 5 Minor 1): while a plan is out, a set it does not know yet (here a rename's new id) has no files in
/// its view, but it is not gone: a second user step made before that plan lands keeps the listed `data` that the set's
/// pair holds back.
#[test]
fn a_set_the_running_plan_does_not_know_is_not_gone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    m.rename_set("armor/steel", "armor/plate").unwrap();
    assert!(
        m.full_plan_pending(),
        "precondition: the rename's plan is still out"
    );
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "armor/plate" && v.members.is_empty()),
        "precondition: the new id has no files until the plan lands"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the rename's own step holds it back"
    );
    m.set_shader("pbr/iron", Some(Shader::Pbr));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a set the running plan does not know yet is not gone"
    );
    settle(&mut m);
}

/// N-39: a shared owner normal whose analysis is known and whose alpha does not vary has no specular to keep: the
/// share writes nothing.
#[test]
fn a_share_of_a_normal_with_no_varying_alpha_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(d.path().join("textures/pbr/iron_n.png"))
        .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        m.item(IRON_N)
            .and_then(|i| i.analysis.as_ref())
            .map(|a| a.alpha != engine::presets::AlphaContent::Varying),
        Some(true),
        "precondition: the analysis is known and the alpha does not vary"
    );
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert!(m.sets_doc.as_ref().unwrap().auto_data().is_empty());
}

/// N-37, a pin of the `edit_docs` read of `automatic_step`: the fuzz switch on open is an automatic step through
/// `edit_docs` (auto_record.rs:234). With accept None the recorder writes nothing, so no batch runs at open and only
/// that read is reached. A saved `auto_data` key that no regular set shares survives the switch; the user's next step
/// clears it.
#[test]
fn an_automatic_step_never_clears_a_leftover() {
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/m");
    std::fs::create_dir_all(&t).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(t.join("rock01.png"))
        .unwrap();
    bumpy_n(&t.join("rock01_n.png"));
    ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
        .save(t.join("rock01_roughness.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, _| Rgb([(x * 4) as u8, 200, 40]))
        .save(t.join("rock01_fuzz.png"))
        .unwrap();
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n[files.\"m/rock01_n.png\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"m/rock01_n.png\"]\n",
    )
    .unwrap();
    let mut m = AppModel::new(sdc_gui::settings::GuiSettings::default(), None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().join("textures"));
    settle(&mut m);
    // The switch plans again after its step.
    settle(&mut m);
    assert_eq!(
        held(&m, "m/rock01.png", "subtype"),
        None,
        "precondition: the switch ran"
    );
    assert_eq!(
        held(&m, "m/rock01_n.png", "alpha.policy"),
        Some(json!("data")),
        "no automatic step clears it"
    );
    assert_eq!(
        m.sets_doc.as_ref().unwrap().auto_data(),
        vec!["m/rock01_n.png".to_string()]
    );
    m.set_shader("m/rock01", Some(Shader::Pbr));
    settle(&mut m);
    assert_eq!(
        held(&m, "m/rock01_n.png", "alpha.policy"),
        None,
        "the user's next step re-checks every listed key"
    );
}

fn auto_data(m: &AppModel) -> Vec<String> {
    m.sets_doc.as_ref().unwrap().auto_data()
}

/// `sdc-project.toml` as saved: `pbr/iron_n.png` listed, and `armor/steel` borrowing `pbr/iron`'s slot 2. Written as a
/// file, not by a seed step: since N8-p a step that runs the follow-on drops an entry whose own table holds no `data`.
const LISTED_AND_BORROWED: &str = "[project]\nauto_data = [\"pbr/iron_n.png\"]\n\n[[sets]]\nid = \"armor/steel\"\nborrow = { slot2 = \"pbr/iron\" }\n";

/// N8-a (Task 8a round 1, finding 1): the user's `alpha.policy = "remove"` under the extension-less spelling and the
/// `subtype` under the full name. The engine applies both tables, so the alpha is the user's: the share writes no
/// `data` in either and lists nothing.
#[test]
fn a_share_writes_no_data_over_the_users_alpha_under_another_spelling() {
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n.png\"]\nsubtype = \"pbr_normal\"\n\n[files.\"pbr/iron_n\"]\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        Some(json!("remove")),
        "precondition: the user's alpha"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "precondition: none in the full name's table"
    );
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None, "no data written");
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        Some(json!("remove"))
    );
    assert!(auto_data(&m).is_empty());
}

/// N8-a: a file whose only `[files]` table is spelled without the extension. The share step's `data` would land in
/// that table, where no later step could tell it from the user's and take it back: the share writes nothing.
#[test]
fn a_share_writes_no_data_into_an_extensionless_table() {
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n\"]\nsubtype = \"pbr_normal\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table under the full name"
    );
    assert_eq!(
        held(&m, IRON_N, "subtype"),
        Some(json!("pbr_normal")),
        "precondition: read through the extension-less spelling"
    );
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        None,
        "no data written"
    );
    assert!(auto_data(&m).is_empty());
}

/// N8-a (finding 1): a listed key whose own `[files]` table is gone, with the user's `data` under the extension-less
/// spelling. `file_value` falls back to that table, but it is not the one the share step writes: ending the share
/// leaves the user's `data`.
#[test]
fn ending_a_share_leaves_the_users_data_under_another_spelling() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        LISTED_AND_BORROWED,
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table under the full name"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: read through the extension-less spelling"
    );
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "precondition: listed"
    );
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        Some(json!("data")),
        "the user's data stays"
    );
}

/// N8-b (finding 2, JN1 own-first): regular `armor/steel` borrows slot 2 from PBR `pbr/iron`, which owns `iron_n` and
/// also borrows `pbr/gold`'s. Steel reads iron's own normal, as the set resolver fills it: `iron_n` gets `data` and is
/// listed, `gold_n` is untouched.
#[test]
fn an_own_first_chain_writes_the_file_the_regular_set_reads() {
    use sdc_gui::model::sets::{SlotFill, SlotKind};
    const GOLD_N: &str = "pbr/gold_n.png";
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let pbr = d.path().join("textures/pbr");
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([200u8, 160, 40]))
        .save(pbr.join("gold.png"))
        .unwrap();
    bumpy_n(&pbr.join("gold_n.png"));
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(pbr.join("gold_rmaos.png"))
        .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.edit_docs("seed", None, true, |_, s| {
        s.set_borrow("pbr/iron", 2, Some("pbr/gold"));
        s.set_borrow("armor/steel", 2, Some("pbr/iron"));
    });
    settle(&mut m);
    let views = sdc_gui::model::sets::set_views(&m);
    let normal = |id: &str| {
        views
            .iter()
            .find(|v| v.id == id)
            .and_then(|v| v.slots.get(&SlotKind::Normal).cloned())
    };
    assert_eq!(
        normal("armor/steel"),
        Some(SlotFill::Borrowed {
            from: "pbr/iron".into(),
            item: IRON_N.into()
        }),
        "precondition: steel reads iron's own normal"
    );
    assert_eq!(
        normal("pbr/iron"),
        Some(SlotFill::Borrowed {
            from: "pbr/gold".into(),
            item: GOLD_N.into()
        }),
        "precondition: iron's own card shows its borrow"
    );
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    assert_eq!(held(&m, GOLD_N, "alpha.policy"), None);
}

/// N8-b (finding 3): renaming the LENDER retargets the regular set's borrow to an id the running plan does not know
/// yet, so the step cannot tell which file the borrow reads: it clears nothing. The `data` and its list entry survive
/// the rename's step and the plan that lands after it.
#[test]
fn renaming_the_lender_keeps_the_shared_normals_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    m.rename_set("pbr/iron", "pbr/plate").unwrap();
    assert!(
        m.full_plan_pending(),
        "precondition: the rename's plan is still out"
    );
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "pbr/plate" && v.members.is_empty()),
        "precondition: the new id has no files until the plan lands"
    );
    assert!(
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .any(|e| e.id == "armor/steel"
                && e.borrow.get(&2).map(String::as_str) == Some("pbr/plate")),
        "precondition: the borrow follows the rename"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the rename's step keeps it"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "and its entry");
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "after the plan lands"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
}

/// N8-b (finding 3, the automatic rename): a new input name renames the lender's automatic set. `retarget_auto` moves
/// the regular set's borrow to the new id, which has no entry and no view until the plan lands. A user step made
/// before then cannot tell which file the borrow reads: it clears nothing, and after the plan the share holds again.
#[test]
fn an_automatic_rename_of_the_lender_keeps_the_shared_normals_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    m.set_input_name("pbr/iron", "textures/pbr/plate").unwrap();
    assert!(
        m.full_plan_pending(),
        "precondition: the rename's plan is still out"
    );
    let entries = m.sets_doc.as_ref().unwrap().entries();
    assert!(
        entries
            .iter()
            .any(|e| e.id == "armor/steel"
                && e.borrow.get(&2).map(String::as_str) == Some("pbr/plate")),
        "precondition: the borrow follows the automatic id"
    );
    assert!(
        !entries.iter().any(|e| e.id == "pbr/plate"),
        "precondition: no entry under the new id"
    );
    assert!(
        !sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "pbr/plate"),
        "precondition: no view under the new id until the plan lands"
    );
    m.set_shader("armor/steel", Some(Shader::Default));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a step before the plan lands keeps it"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    m.set_shader("armor/steel", None);
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "after the plan the share holds"
    );
}

/// N8-b (review Q6): an explicit PBR owner and an explicit PBR borrower. The owner's `pbr_normal` is retyped `height`,
/// then the borrower is picked Default before the type's plan lands. The last plan still puts the file in the Normal
/// slot, but the document says it is a height map now: it gets no `data`.
#[test]
fn a_normal_retyped_before_its_plan_lands_gets_no_data() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.set_shader("pbr/iron", Some(Shader::Pbr));
    settle(&mut m);
    m.set_shader("armor/steel", Some(Shader::Pbr));
    settle(&mut m);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "precondition: a share between PBR sets writes nothing"
    );
    m.set_field(IRON_N, "subtype", json!("height"));
    assert_eq!(
        m.item(IRON_N)
            .and_then(|i| i.job.as_ref())
            .map(|j| j.profile.subtype),
        Some(Subtype::PbrNormal),
        "precondition: the type's plan has not landed"
    );
    assert_eq!(
        normal_owner(&m, "pbr/iron").as_deref(),
        Some(IRON_N),
        "precondition: the last plan still fills the Normal slot with it"
    );
    m.set_shader("armor/steel", Some(Shader::Default));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "the retyped file gets no data"
    );
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
}

/// N8-c (finding 5): as saved, a listed key with no alpha (reset), retyped as a diffuse (so no set's Normal slot holds
/// it), which `armor/steel` borrows; the user's first step confirms Data on it: that `data` is the user's. It stays
/// after the step and after a later step that ends the share, and the key is off the list. (Written as files: since
/// N8-p a step that runs the follow-on drops such an entry, so only a project opened so reaches the Data pick with it.)
#[test]
fn a_confirmed_data_on_a_listed_key_is_the_users() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.\"pbr/iron_n.png\"]\nsubtype = \"diffuse\"\n",
    )
    .unwrap();
    std::fs::write(t.join("sdc-project.toml"), LISTED_AND_BORROWED).unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "precondition: reset"
    );
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "precondition: listed"
    );
    assert_ne!(
        normal_owner(&m, "pbr/iron").as_deref(),
        Some(IRON_N),
        "precondition: the file left the Normal slot"
    );
    m.decide(IRON_N, Answer::Alpha(AlphaPolicy::Data));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the Data the user confirmed"
    );
    assert!(auto_data(&m).is_empty(), "the user's now");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "ending the share leaves it"
    );
}

/// N8-d (finding 6): `sdc-project.toml` holds the valid but unusable `project = "bad"`, so `auto_data` cannot be
/// written. The value and its entry go together or not at all: the share writes no `data` and lists nothing.
#[test]
fn a_share_that_cannot_list_writes_no_data() {
    use sdc_gui::model::sets::{SlotFill, SlotKind};
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "project = \"bad\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert!(
        !m.sets_doc.as_ref().unwrap().read_only(),
        "precondition: the file opens for editing"
    );
    share(&mut m, IRON_N, "armor/steel");
    assert!(
        matches!(
            sdc_gui::model::sets::set_views(&m)
                .iter()
                .find(|v| v.id == "armor/steel")
                .and_then(|v| v.slots.get(&SlotKind::Normal)),
            Some(SlotFill::Borrowed { item, .. }) if item == IRON_N
        ),
        "precondition: the share happened"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "no data without its entry"
    );
    assert!(auto_data(&m).is_empty());
}

/// N8-d (finding 7): with an unshared listed `data` present, a user's edit that writes the value already held changes
/// no document: it runs no cleanup and adds no Undo step.
#[test]
fn an_edit_that_writes_the_held_value_clears_nothing() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n.png\"]\nalpha.policy = \"data\"\n\n[files.\"armor/steel.png\"]\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the leftover"
    );
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "precondition: listed"
    );
    let label = m.undo_label().map(str::to_string);
    m.set_alpha("armor/steel.png", AlphaPolicy::Remove);
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "an equal write clears nothing"
    );
    assert_eq!(
        m.undo_label().map(str::to_string),
        label,
        "and adds no Undo step"
    );
}

/// `two_sets_files` with the user's `[files]` table `table` holding `alpha.policy = "data"` for `IRON_N`, which is
/// listed although its own lower-case table is absent; `armor/steel` borrows it, then the share ends.
fn ending_the_share_over(table: &str) -> (tempfile::TempDir, AppModel) {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        format!("[files.'{table}']\nalpha.policy = \"data\"\n"),
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        LISTED_AND_BORROWED,
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table spelled as the list entry"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    (d, m)
}

/// N8-e (round 2, B1): the list entry names the exact table the share step wrote. A case twin of it holding the
/// user's `data` is not that table: ending the share clears nothing.
#[test]
fn ending_a_share_leaves_the_users_data_in_a_case_twin() {
    let (_d, m) = ending_the_share_over("PBR/IRON_N.PNG");
    assert_eq!(
        held(&m, "PBR/IRON_N.PNG", "alpha.policy"),
        Some(json!("data")),
        "the user's data in the case twin stays"
    );
}

/// N8-e (B1): as above, with a twin that differs from the listed spelling only by Unicode case.
#[test]
fn ending_a_share_leaves_the_users_data_in_a_unicode_twin() {
    use sdc_gui::model::sets::SlotCommand;
    const ARZ_N: &str = "pbr/ärz_n.png";
    let d = tempfile::tempdir().unwrap();
    let pbr = d.path().join("textures/pbr");
    let armor = d.path().join("textures/armor");
    for f in [&pbr, &armor] {
        std::fs::create_dir_all(f).unwrap();
    }
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([120u8, 90, 60]))
        .save(pbr.join("ärz.png"))
        .unwrap();
    bumpy_n(&pbr.join("ärz_n.png"));
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(pbr.join("ärz_rmaos.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([90u8, 90, 100]))
        .save(armor.join("steel.png"))
        .unwrap();
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.'PBR/ÄRZ_N.PNG']\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/ärz_n.png\"]\n\n[[sets]]\nid = \"armor/steel\"\nborrow = { slot2 = \"pbr/ärz\" }\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(ARZ_N),
        "precondition: no table spelled as the list entry"
    );
    assert_eq!(auto_data(&m), vec![ARZ_N.to_string()], "precondition");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(
        held(&m, "PBR/ÄRZ_N.PNG", "alpha.policy"),
        Some(json!("data")),
        "the user's data in the Unicode twin stays"
    );
}

/// N8-e (B1), a pin: a `[files]` key with a literal backslash names no file (the engine matches `/` only), so the
/// share writes and lists the slash-form table, and ending the share clears that one and leaves the other alone.
#[test]
fn a_backslash_table_beside_the_shares_table_is_left_alone() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.'pbr\\iron_n.png']\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), None);
    assert_eq!(
        held(&m, "pbr\\iron_n.png", "alpha.policy"),
        Some(json!("remove"))
    );
}

/// N8-e (B5, amends N6-j): a listed key whose own table is gone, with the user's `data` in the extension-less table:
/// that `data` is the user's for Task 6 too, so a shader pick neither retypes nor clears the normal's sub-type.
#[test]
fn a_users_stem_data_keeps_the_normal_out_of_shader_picks() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let typed = held(&m, IRON_N, "subtype");
    m.set_shader("pbr/iron", Some(Shader::Default));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "subtype"),
        typed,
        "the pick leaves the sub-type"
    );
    assert_eq!(held(&m, "pbr/iron_n", "alpha.policy"), Some(json!("data")));
}

/// Drop `item` on set `into` as the Sets column does, without waiting for the plan.
fn drop_now(m: &mut AppModel, item: &str, into: &str, choice: sdc_gui::model::sets::DropChoice) {
    use sdc_gui::model::sets::DropOutcome;
    match m.drop_choice(item, into, choice) {
        DropOutcome::AskReplace { .. } => m.drop_replace(item, into, choice).expect("dropped"),
        DropOutcome::Joined => {}
        other => panic!("{other:?}"),
    }
}

/// `two_sets` plus a populated PBR set `pbr/gold` (`gold.png`, `gold_rmaos.png`, and `gold_n.png` when `with_n`);
/// `iron_n` is shared into `armor/steel` and listed.
fn shared_with_gold(dir: &Path, with_n: bool) -> AppModel {
    open_slot_files(dir);
    let pbr = dir.join("textures/pbr");
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([200u8, 160, 40]))
        .save(pbr.join("gold.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([100u8, 0, 255]))
        .save(pbr.join("gold_rmaos.png"))
        .unwrap();
    if with_n {
        bumpy_n(&pbr.join("gold_n.png"));
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    m
}

/// N8-f (round 2, B2): the lent normal moves into another populated PBR set whose Normal slot is empty, and the
/// regular set's borrow follows it. The last plan does not have the file in that set yet, so the step cannot name
/// what the borrow reads: the listed `data` stays, and the share holds after the plan.
#[test]
fn moving_a_lent_normal_between_pbr_sets_keeps_its_data() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let mut m = shared_with_gold(d.path(), false);
    drop_now(&mut m, IRON_N, "pbr/gold", DropChoice::Move);
    assert!(
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .any(|e| e.id == "armor/steel"
                && e.borrow.get(&2).map(String::as_str) == Some("pbr/gold")),
        "precondition: the borrow follows the file"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the move's step keeps it"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N8-f (B2): as above with Replace: `pbr/gold` has its own normal, which the move displaces. The listed `data` stays
/// and the displaced normal gets none.
#[test]
fn replacing_a_normal_with_a_lent_one_writes_and_clears_nothing() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let mut m = shared_with_gold(d.path(), true);
    drop_now(&mut m, IRON_N, "pbr/gold", DropChoice::Move);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the replace's step keeps it"
    );
    assert_eq!(
        held(&m, "pbr/gold_n.png", "alpha.policy"),
        None,
        "the displaced normal gets no data"
    );
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(held(&m, "pbr/gold_n.png", "alpha.policy"), None);
}

/// N8-f (round 2, fresh): after the move, and before its plan lands, the user makes an unrelated `[files]` edit. The
/// views still put `iron_n` in `pbr/iron`, and nothing in that step changes membership, so only the stale-plan check
/// stops the step from reading "`pbr/gold` has no normal" and clearing the shared normal's `data`.
#[test]
fn a_step_before_the_moves_plan_lands_keeps_the_data() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let mut m = shared_with_gold(d.path(), false);
    drop_now(&mut m, IRON_N, "pbr/gold", DropChoice::Move);
    m.edit_docs("unrelated", None, true, |p, _| {
        p.set_file_value("pbr/gold.png", "alpha.policy", &json!("remove"))
    });
    assert_eq!(
        held(&m, "pbr/gold.png", "alpha.policy"),
        Some(json!("remove")),
        "precondition: the edit landed"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a step on a stale plan keeps it"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N8-f (B2): one step makes `iron_n` shared by a known regular set and by a set the step creates (no files yet, so
/// unsure). A key any borrower makes unsure is neither written nor cleared.
#[test]
fn an_unsure_key_is_not_written() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    m.edit_docs("seed", None, true, |_, s| {
        s.set_borrow("ghost/new", 2, Some("pbr/iron"));
        s.set_borrow("armor/steel", 2, Some("pbr/iron"));
    });
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "an unsure key gets no data"
    );
    settle(&mut m);
}

/// N8-h (round 2, B4): Reset all removes a listed key's own table, which held only `remove`; the `data` that the
/// extension-less table already held shows through. The step wrote no `data`: the entry stays listed (N-27).
#[test]
fn a_reset_that_uncovers_data_leaves_the_entry() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"pbr/iron_n.png\"]\nalpha.policy = \"remove\"\n\n[files.\"pbr/iron_n\"]\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("remove")),
        "precondition"
    );
    m.reset_all_settings(&[IRON_N.to_string()]);
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: the reset removed the table"
    );
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "a reset is files-only"
    );
}

/// N8-h (B4), a pin of the batch compare: a bulk Data on a six-file cube is copied to its other face entries in the
/// same batch (DT31); a listed secondary face that the copy wrote `data` into leaves the list.
#[test]
fn a_six_face_copy_onto_a_listed_face_unlists_it() {
    use sdc_gui::model::bulk::BulkOp;
    const NX: &str = "a/chrome_e_nx.png";
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/a");
    std::fs::create_dir_all(&r).unwrap();
    for (i, f) in ["px", "nx", "py", "ny", "pz", "nz"].iter().enumerate() {
        image::RgbImage::from_pixel(16, 16, image::Rgb([30 + 40 * i as u8, 20, 10]))
            .save(r.join(format!("chrome_e_{f}.png")))
            .unwrap();
    }
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"a/chrome_e_nx.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let key = m
        .items
        .iter()
        .find(|i| i.job.is_some())
        .expect("the six-face item")
        .key
        .clone();
    assert_ne!(key, NX, "precondition: the face is a secondary one");
    m.decide(&key, Answer::Subtype(Subtype::Cubemap));
    settle(&mut m);
    m.apply_bulk_edit(
        std::slice::from_ref(&key),
        "alpha.policy",
        BulkOp::Set(json!("data")),
    );
    settle(&mut m);
    assert_eq!(
        held(&m, NX, "alpha.policy"),
        Some(json!("data")),
        "precondition: the copy reached the listed face"
    );
    assert!(auto_data(&m).is_empty(), "the user's batch wrote it");
}

fn aged(p: &Path) {
    std::fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(86_400))
        .unwrap();
}

/// N8-h (B3): a crash restore is no user alpha edit. Saved: a listed key with its alpha reset. Unsaved at the crash:
/// a later share wrote the automatic `data` back. The restore keeps the list as the snapshot had it, so ending the
/// share later takes the `data` back.
#[test]
fn a_crash_restore_keeps_the_list_it_restores() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    // As saved (written as a file: since N8-p a step that runs the follow-on drops such an entry).
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "precondition: reset"
    );
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "precondition: listed, reset"
    );
    m.save().unwrap();
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it again"
    );
    sdc_gui::crash_log::write_recovery(&m.recovery_changed().unwrap_or_default());
    drop(m);
    let t = d.path().join("textures");
    aged(&t.join("sdc.toml"));
    aged(&t.join("sdc-project.toml"));
    let mut m = opened_as_the_app(t, d.path().join("export"));
    assert_eq!(
        m.undo_label(),
        Some("Restore crash edits"),
        "precondition: restored"
    );
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "the restore keeps the record"
    );
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "ending the share takes it back"
    );
}

/// N8-i (round 2, B7): the share step's file write fails after the list was written: the list goes back to exactly
/// what it was, a key already listed after a reset and a stale entry included.
#[test]
fn a_failed_write_leaves_the_list_as_it_was() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let mut m = two_sets(d.path());
    share(&mut m, IRON_N, "armor/steel");
    m.clear_alpha(IRON_N);
    settle(&mut m);
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    let start = vec![IRON_N.to_string(), "x/stale.png".to_string()];
    let l = start.clone();
    // No plan, so no follow-on: since N8-p one would drop both entries (neither table holds `data`).
    m.edit_docs("seed", None, false, move |_, s| s.set_auto_data(&l));
    settle(&mut m);
    assert_eq!(auto_data(&m), start, "precondition");
    m.fail_shared_alpha_write = true;
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "precondition: the write failed"
    );
    assert_eq!(auto_data(&m), start, "the list as it was");
}

/// The share's table `pbr/iron_n.png` holds `data`, listed twice in `auto_data`, and the user's `twin` table (another
/// spelling of the same file) holds `remove`; `armor/steel` borrows `pbr/iron`'s normal. Then the share ends.
fn ending_a_twice_listed_share_beside(twin: &str) -> (tempfile::TempDir, AppModel) {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        format!(
            "[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n\n[files.'{twin}']\nalpha.policy = \"remove\"\n"
        ),
    )
    .unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\", \"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    m.edit_docs("seed", None, true, |_, s| {
        s.set_borrow("armor/steel", 2, Some("pbr/iron"))
    });
    settle(&mut m);
    // The reviewer's "leave only Data in the listed table": the user takes out the sub-type the recorder wrote, so
    // clearing the `data` empties the table and removes it.
    m.edit_docs("user", None, true, |p, _| {
        p.clear_file_value(IRON_N, "subtype")
    });
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "subtype"),
        None,
        "precondition: the share's table holds only its data"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share's table holds data"
    );
    assert_eq!(auto_data(&m).len(), 2, "precondition: listed twice");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    settle(&mut m);
    (d, m)
}

/// N8-k (round 3, B1): a repeated list entry is one key and its clear is exact and done once: the share's own table
/// loses its `data`, and the user's upper-case twin keeps its `remove`.
#[test]
fn a_twice_listed_clear_leaves_the_users_case_twin() {
    let (_d, m) = ending_a_twice_listed_share_beside("PBR/IRON_N.PNG");
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "the share's own table is cleared"
    );
    assert_eq!(
        held(&m, "PBR/IRON_N.PNG", "alpha.policy"),
        Some(json!("remove")),
        "the user's case twin keeps its value"
    );
}

/// N8-k (B1): as above, with the user's extension-less table.
#[test]
fn a_twice_listed_clear_leaves_the_users_stem_twin() {
    let (_d, m) = ending_a_twice_listed_share_beside("pbr/iron_n");
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "the share's own table is cleared"
    );
    assert_eq!(
        held(&m, "pbr/iron_n", "alpha.policy"),
        Some(json!("remove")),
        "the user's stem twin keeps its value"
    );
}

/// The reviewer's N8-m fixture: a set the user created (New set from `armor/steel.png`, a diffuse only, its shader
/// chosen Default) borrows `pbr/iron`'s normal, which the share listed; then the diffuse is gone from disk, the project
/// is reopened and settles, and an unrelated diffuse `armor/bronze.png` is in the folder. The borrower is left with no
/// file of its own and still points at the listed normal. Returns the borrower's id.
fn emptied_borrower(dir: &Path) -> (AppModel, String) {
    emptied_borrower_with(dir, false)
}

/// `emptied_borrower`; with `twice`, a second `[[sets]]` entry carrying only the borrower's id is appended by hand
/// before the reopen (the GUI edits the first entry of an id, the engine reads every one).
fn emptied_borrower_with(dir: &Path, twice: bool) -> (AppModel, String) {
    two_sets_files(dir);
    let armor = dir.join("textures/armor");
    std::fs::remove_file(armor.join("steel_n.png")).unwrap();
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    let id = m.drop_new_set("armor/steel.png").expect("a new set");
    settle(&mut m);
    m.set_shader(&id, Some(Shader::Default));
    settle(&mut m);
    share(&mut m, IRON_N, &id);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: the share wrote it"
    );
    settle(&mut m);
    m.save().expect("saved");
    drop(m);
    std::fs::remove_file(armor.join("steel.png")).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([150u8, 110, 60]))
        .save(armor.join("bronze.png"))
        .unwrap();
    if twice {
        let f = dir.join("textures/sdc-project.toml");
        let mut text = std::fs::read_to_string(&f).unwrap();
        text.push_str(&format!("\n[[sets]]\nid = \"{id}\"\n"));
        std::fs::write(&f, text).unwrap();
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    settle(&mut m);
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == id && v.members.is_empty()),
        "precondition: the borrower has no file"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: still data"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    (m, id)
}

/// N8-m (round 3, B3): a diffuse duplicated into the emptied borrower gives it its first file in a step whose views
/// still show it empty. Its membership changed, so it is not gone: the listed `data` is not cleared in that step, and
/// the share is live once the plan lands.
#[test]
fn a_duplicate_into_an_emptied_borrower_keeps_the_data() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = emptied_borrower(d.path());
    drop_now(&mut m, "armor/bronze.png", &id, DropChoice::Duplicate);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a set whose membership the step changed is not gone"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N8-u (round 5, the reviewer's sequence): as `a_move_into_an_emptied_borrower_keeps_the_data`, with a second
/// `[[sets]]` entry under the borrower's id written by hand. The Move edits the first entry; the comparison keeps every
/// entry of an id, so the borrower is still a set whose membership the step changed, and the `data` stays.
#[test]
fn a_move_into_an_emptied_borrower_with_a_second_entry_keeps_the_data() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = emptied_borrower_with(d.path(), true);
    let rows = |m: &AppModel| {
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .filter(|e| e.id == id)
            .count()
    };
    assert_eq!(rows(&m), 2, "precondition: two entries under the id");
    drop_now(&mut m, "armor/bronze.png", &id, DropChoice::Move);
    assert!(
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .any(|e| e.id == id && e.add.iter().any(|f| f == "armor/bronze.png")),
        "precondition: the Move wrote an add line"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a set whose membership the step changed is not gone"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N8-m (B3): as `a_duplicate_into_an_emptied_borrower_keeps_the_data`, with the diffuse moved in.
#[test]
fn a_move_into_an_emptied_borrower_keeps_the_data() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = emptied_borrower(d.path());
    drop_now(&mut m, "armor/bronze.png", &id, DropChoice::Move);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "a set whose membership the step changed is not gone"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("data")));
}

/// N8-m control: an ordinary Share into the empty Normal slot of a populated borrower changes only its borrow line,
/// so the borrower stays known and the share writes `data` in its own step.
#[test]
fn an_ordinary_share_into_an_empty_slot_writes_in_its_step() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    std::fs::remove_file(d.path().join("textures/armor/steel_n.png")).unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    let lines = |m: &AppModel| {
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .find(|e| e.id == "armor/steel")
            .map(|e| (e.add.clone(), e.remove.clone()))
            .unwrap_or_default()
    };
    let before = lines(&m);
    share(&mut m, IRON_N, "armor/steel");
    assert_eq!(
        lines(&m),
        before,
        "precondition: only the borrow line changed"
    );
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the ordinary share writes in its step"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
}

/// N8-n (round 3, B2): `fresh` holds while a `[files]`-only edit's full plan is out, and that plan may give an empty
/// set its files back. The emptied borrower still points at the listed normal; `armor/bronze` shares it too. An
/// unrelated `[files]` edit starts a full plan, and while it is out the user ends `armor/bronze`'s share, the last one a
/// populated regular set holds. The empty borrower is not gone while a plan is out, so the normal stays unsure and its
/// `data` stays.
#[test]
fn an_empty_borrower_is_not_gone_while_a_files_plan_is_out() {
    use sdc_gui::model::sets::SlotCommand;
    let d = tempfile::tempdir().unwrap();
    let (mut m, _id) = emptied_borrower(d.path());
    share(&mut m, IRON_N, "armor/bronze");
    settle(&mut m);
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "precondition: still data"
    );
    m.edit_docs("unrelated", None, true, |p, _| {
        p.set_file_value("armor/bronze.png", "alpha.policy", &json!("remove"))
    });
    assert!(
        m.full_plan_pending(),
        "precondition: the files edit's full plan is out"
    );
    m.set_slot_command("armor/bronze", 2, SlotCommand::Clear)
        .unwrap();
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "an empty set is not gone while a plan is out"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()]);
    settle(&mut m);
}

/// N8-o (round 3, B5): one setting changed in a settled cs project with 4,000 `[files]` tables and 1,000 listed keys
/// walks the `[files]` keys at most once per document state (before the edit, after it): the list reader, the
/// unlisting and the follow-on share the two indexes. `armor/steel_n.png`'s table is spelled in upper case, so a
/// per-owner read of its sub-type through the document's resolver would miss the exact spelling and walk every key; it
/// goes through the index instead. The real step runs: the user's edit, then the follow-on, which clears the listed
/// keys no regular set shares.
#[test]
fn a_step_walks_the_files_keys_once_per_document_state() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let mut toml = String::from("[files.'ARMOR/STEEL_N.PNG']\nsubtype = \"normal\"\n\n");
    let mut listed = Vec::new();
    for i in 0..4_000 {
        toml.push_str(&format!(
            "[files.\"old/f{i:04}.png\"]\nalpha.policy = \"data\"\n\n"
        ));
        if i % 4 == 0 {
            listed.push(format!("\"old/f{i:04}.png\""));
        }
    }
    std::fs::write(d.path().join("textures/sdc.toml"), toml).unwrap();
    std::fs::write(
        d.path().join("textures/sdc-project.toml"),
        format!("[project]\nauto_data = [{}]\n", listed.join(", ")),
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    settle(&mut m);
    assert_eq!(auto_data(&m).len(), 1_000, "precondition: a long list");
    assert!(
        !m.project
            .as_ref()
            .unwrap()
            .has_file_key("armor/steel_n.png"),
        "precondition: the owner normal's table is spelled in another case"
    );
    let start = sdc_gui::project::key_walks();
    m.edit_docs("user", None, true, |p, _| {
        p.set_file_value("armor/steel.png", "alpha.policy", &json!("remove"))
    });
    let walks = sdc_gui::project::key_walks() - start;
    assert!(
        auto_data(&m).is_empty(),
        "precondition: the follow-on ran and cleared the listed keys"
    );
    assert!(walks <= 2, "{walks} walks of the [files] keys in one step");
}

/// N8-k: `ProjectDoc::clear_exact_file_value` touches only the table spelled byte for byte as the key, and only while
/// it holds the value: no case twin, no extension-less table, no other value.
#[test]
fn the_exact_remover_touches_only_its_own_table_while_it_holds_the_value() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("sdc.toml"),
        "[files.'PBR/IRON_N.PNG']\nalpha.policy = \"data\"\n\n[files.'pbr/iron_n']\nalpha.policy = \"data\"\n\n[files.'pbr/iron_n.png']\nalpha.policy = \"remove\"\nsubtype = \"pbr_normal\"\n",
    )
    .unwrap();
    let mut p = sdc_gui::project::ProjectDoc::open(d.path());
    assert!(!p.read_only(), "{:?}", p.error());
    let start = p.text();
    let data = json!("data");
    assert!(
        !p.clear_exact_file_value("Pbr/Iron_N.png", "alpha.policy", &data),
        "no table spelled so: nothing, no twin"
    );
    assert!(
        !p.clear_exact_file_value("pbr/iron_n.png", "alpha.policy", &data),
        "the exact table holds another value: nothing"
    );
    assert_eq!(p.text(), start, "nothing changed");
    assert!(p.clear_exact_file_value("PBR/IRON_N.PNG", "alpha.policy", &data));
    assert!(
        !p.has_file_key("PBR/IRON_N.PNG"),
        "the emptied exact table goes"
    );
    assert_eq!(p.file_value("pbr/iron_n", "alpha.policy"), Some(data));
    assert_eq!(
        p.file_value("pbr/iron_n.png", "alpha.policy"),
        Some(json!("remove"))
    );
}

/// N8-p (round 4, the reviewer's sequence): the listed table was re-spelled by hand, so no table is spelled as the
/// entry. The next user step that runs the follow-on drops the entry, in that step's own Undo step. Later the user writes
/// their own `data` under the original spelling, between sessions: the next step leaves it, as nothing lists it.
#[test]
fn an_entry_whose_table_is_gone_leaves_the_list_in_the_next_step() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'PBR/IRON_N.PNG']\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    std::fs::write(
        t.join("sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(t.clone(), d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table spelled as the entry"
    );
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.set_alpha("armor/steel.png", AlphaPolicy::Remove);
    assert!(auto_data(&m).is_empty(), "the stale entry is dropped");
    assert_eq!(m.undo_label(), Some("Alpha"), "in the user's own step");
    m.undo();
    assert_eq!(
        auto_data(&m),
        vec![IRON_N.to_string()],
        "one Undo brings both back"
    );
    assert_eq!(held(&m, "armor/steel.png", "alpha.policy"), None);
    m.redo();
    settle(&mut m);
    m.save().expect("saved");
    drop(m);
    let mut text = std::fs::read_to_string(t.join("sdc.toml")).unwrap();
    text.push_str("\n[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n");
    std::fs::write(t.join("sdc.toml"), text).unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert!(
        m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: the user's own table under the original spelling"
    );
    m.set_alpha("armor/steel.png", AlphaPolicy::Data);
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(IRON_N, "alpha.policy"),
        Some(json!("data")),
        "the user's data stays"
    );
}

/// N8-q (round 4): two distinct case spellings of one file, both listed and both holding `data` in their own exact
/// table, as saved. Ending their last share clears both: neither is the other's protected twin in the read-back.
#[test]
fn ending_a_share_clears_two_listed_spellings_of_one_file() {
    use sdc_gui::model::sets::SlotCommand;
    const UPPER: &str = "PBR/IRON_N.PNG";
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n\n[files.'PBR/IRON_N.PNG']\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        t.join("sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\", \"PBR/IRON_N.PNG\"]\n\n[[sets]]\nid = \"armor/steel\"\nborrow = { slot2 = \"pbr/iron\" }\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    let exact_alpha = |m: &AppModel, k: &str| {
        let p = m.project.as_ref().unwrap();
        p.has_file_key(k)
            .then(|| p.file_value(k, "alpha.policy"))
            .flatten()
    };
    assert!(
        !m.project.as_ref().unwrap().read_only() && !m.sets_doc.as_ref().unwrap().read_only(),
        "precondition: both documents open for editing"
    );
    for k in [IRON_N, UPPER] {
        assert_eq!(exact_alpha(&m, k), Some(json!("data")), "precondition: {k}");
    }
    assert_eq!(auto_data(&m), vec![IRON_N.to_string(), UPPER.to_string()]);
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    for k in [IRON_N, UPPER] {
        assert_eq!(exact_alpha(&m, k), None, "{k} is cleared");
    }
    assert!(auto_data(&m).is_empty(), "both entries leave the list");
    assert_eq!(m.undo_label(), Some("Clear slot 2 of armor/steel"));
}

/// N8-p: an entry whose exact table holds another value is dropped too, in a step that writes and clears nothing.
#[test]
fn an_entry_whose_table_holds_another_value_leaves_the_list() {
    let d = tempfile::tempdir().unwrap();
    two_sets_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'pbr/iron_n.png']\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    std::fs::write(
        t.join("sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.set_alpha("armor/steel.png", AlphaPolicy::Remove);
    assert!(auto_data(&m).is_empty(), "the stale entry is dropped");
    assert_eq!(held(&m, IRON_N, "alpha.policy"), Some(json!("remove")));
}

/// N8-s (round 4, the reviewer's pin): a real Share of an eligible normal that has no `[files]` table, into the empty
/// Normal slot of a populated regular set, on a project with 4,000 `[files]` tables, walks the keys at most once before
/// the edit and once after it: the write creates the table under the spelling the step's index chose, with no lookup.
/// The normal is `pbr_normal` with no table: the saved sets document chooses PBR grass for `landscape/rock`, so the
/// engine types its landscape `_n` itself, and with accept None nothing records a type.
#[test]
fn a_share_that_creates_the_normals_table_walks_the_keys_once_per_document_state() {
    use sdc_gui::model::sets::DropChoice;
    let d = tempfile::tempdir().unwrap();
    rock(d.path());
    let t = d.path().join("textures");
    std::fs::create_dir_all(t.join("armor")).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([90u8, 90, 100]))
        .save(t.join("armor/steel.png"))
        .unwrap();
    let mut toml = String::new();
    for i in 0..4_000 {
        toml.push_str(&format!(
            "[files.\"old/f{i:04}.png\"]\nalpha.policy = \"remove\"\n\n"
        ));
    }
    std::fs::write(t.join("sdc.toml"), toml).unwrap();
    std::fs::write(
        t.join("sdc-project.toml"),
        "[[sets]]\nid = \"landscape/rock\"\nshader = \"pbr_grass\"\n",
    )
    .unwrap();
    let mut m = AppModel::new(sdc_gui::settings::GuiSettings::default(), None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(t);
    settle(&mut m);
    let p = m.project.as_ref().unwrap();
    assert!(
        !p.has_file_key(ROCK_N) && p.file_value(ROCK_N, "subtype").is_none(),
        "precondition: no table for the normal"
    );
    let j = m.item(ROCK_N).and_then(|i| i.job.clone()).expect("job");
    assert_eq!(j.profile.subtype, Subtype::PbrNormal, "precondition");
    assert_ne!(j.profile.alpha.policy, AlphaPolicy::Data, "precondition");
    let start = sdc_gui::project::key_walks();
    drop_now(&mut m, ROCK_N, "armor/steel", DropChoice::Share);
    let walks = sdc_gui::project::key_walks() - start;
    let p = m.project.as_ref().unwrap();
    assert!(
        p.has_file_key(ROCK_N),
        "precondition: the share created the table"
    );
    assert_eq!(p.file_value(ROCK_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(auto_data(&m), vec![ROCK_N.to_string()]);
    assert!(walks <= 2, "{walks} walks of the [files] keys in the share");
    settle(&mut m);
}

/// N8-s (B4 Minor): the walk counter counts `[files]` walks only: an alias lookup that misses the exact `[inputs]`
/// spelling compares every `[inputs]` key, which is not a `[files]` walk.
#[test]
fn an_inputs_lookup_is_not_a_files_walk() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("sdc.toml"),
        "[inputs.'PBR/IRON.PNG']\nas = \"pbr/plate.png\"\n\n[files.'a/b.png']\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    let p = sdc_gui::project::ProjectDoc::open(d.path());
    assert!(!p.read_only(), "{:?}", p.error());
    let start = sdc_gui::project::key_walks();
    assert_eq!(
        p.read_as("pbr/iron.png").as_deref(),
        Some("pbr/plate.png"),
        "precondition: found under another case"
    );
    assert_eq!(sdc_gui::project::key_walks() - start, 0, "no [files] walk");
}

/// N8-s: `ProjectDoc::set_exact_file_value` writes the table spelled byte for byte as the key, created under that
/// spelling when absent, without walking the keys; a case twin and an extension-less table are left as they are.
#[test]
fn the_exact_setter_writes_only_its_own_spelling_without_a_walk() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("sdc.toml"),
        "[files.'PBR/IRON_N.PNG']\nalpha.policy = \"remove\"\n\n[files.'pbr/iron_n']\nsubtype = \"pbr_normal\"\n",
    )
    .unwrap();
    let mut p = sdc_gui::project::ProjectDoc::open(d.path());
    assert!(!p.read_only(), "{:?}", p.error());
    let twins = |p: &sdc_gui::project::ProjectDoc| {
        (
            p.exact_file_entry_text("PBR/IRON_N.PNG"),
            p.exact_file_entry_text("pbr/iron_n"),
        )
    };
    let before = twins(&p);
    let start = sdc_gui::project::key_walks();
    p.set_exact_file_value(IRON_N, "alpha.policy", &json!("data"));
    assert_eq!(sdc_gui::project::key_walks() - start, 0, "no walk");
    assert!(p.has_file_key(IRON_N), "the table under its own spelling");
    assert_eq!(p.file_value(IRON_N, "alpha.policy"), Some(json!("data")));
    assert_eq!(twins(&p), before, "the twins are left as they are");
    p.set_exact_file_value(IRON_N, "alpha.policy", &json!("remove"));
    assert_eq!(sdc_gui::project::key_walks() - start, 0, "no walk");
    assert_eq!(p.file_value(IRON_N, "alpha.policy"), Some(json!("remove")));
    assert_eq!(twins(&p), before);
}

/// N8-t (round 5, the reviewer's sequence): as saved, the listed table was re-spelled by hand (`PBR/IRON_N.PNG` holds
/// the `data` now, no table is spelled as the entry), `armor/steel` borrows the normal, and an unrelated texture is in
/// no set. The user's first step takes that texture back to its automatic set: a rejoin, which writes and clears no
/// file value, but the stale entry still leaves the list in that step. Later the user writes their own `data` under the
/// original spelling between sessions and ends the share: nothing lists it, so it stays.
#[test]
fn a_rejoin_step_drops_a_stale_entry_too() {
    use sdc_gui::model::sets::SlotCommand;
    const UPPER: &str = "PBR/IRON_N.PNG";
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let t = d.path().join("textures");
    std::fs::create_dir_all(t.join("misc")).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([70u8, 90, 110]))
        .save(t.join("misc/pebble.png"))
        .unwrap();
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'PBR/IRON_N.PNG']\nalpha.policy = \"data\"\n",
    )
    .unwrap();
    std::fs::write(
        t.join("sdc-project.toml"),
        "[project]\nauto_data = [\"pbr/iron_n.png\"]\nno_set = [\"misc/pebble.png\"]\n\n[[sets]]\nid = \"armor/steel\"\nborrow = { slot2 = \"pbr/iron\" }\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(t.clone(), d.path().join("export"));
    settle(&mut m);
    assert!(
        !m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: no table spelled as the entry"
    );
    assert!(m.is_no_set("misc/pebble.png"), "precondition: in no set");
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.back_to_automatic_set(&["misc/pebble.png".to_string()])
        .unwrap();
    assert!(!m.is_no_set("misc/pebble.png"), "precondition: rejoined");
    assert!(
        auto_data(&m).is_empty(),
        "the stale entry leaves the list in the rejoin step"
    );
    assert_eq!(
        held(&m, UPPER, "alpha.policy"),
        Some(json!("data")),
        "the rejoin step changes no file value"
    );
    settle(&mut m);
    m.save().expect("saved");
    drop(m);
    let mut text = std::fs::read_to_string(t.join("sdc.toml")).unwrap();
    text.push_str("\n[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n");
    std::fs::write(t.join("sdc.toml"), text).unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert!(
        m.project.as_ref().unwrap().has_file_key(IRON_N),
        "precondition: the user's own table under the original spelling"
    );
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        Some(json!("data")),
        "ending the share leaves the user's data"
    );
}

/// N8-v (round 5, a pin: it passes on the base): the read-back protects the tables of the intended targets' own file
/// only. Another file's table that the user changed by hand between runs is not read back, so ending the share still
/// clears the listed `data` and leaves the other table as the user wrote it.
#[test]
fn another_files_table_changed_by_hand_does_not_roll_the_step_back() {
    use sdc_gui::model::sets::SlotCommand;
    const OTHER: &str = "armor/other.png";
    let d = tempfile::tempdir().unwrap();
    open_slot_files(d.path());
    let t = d.path().join("textures");
    std::fs::write(
        t.join("sdc.toml"),
        "[files.'pbr/iron_n.png']\nalpha.policy = \"data\"\n\n[files.'armor/other.png']\nalpha.policy = \"remove\"\n",
    )
    .unwrap();
    std::fs::write(t.join("sdc-project.toml"), LISTED_AND_BORROWED).unwrap();
    let mut m = opened_as_the_app(t.clone(), d.path().join("export"));
    settle(&mut m);
    m.save().expect("saved");
    drop(m);
    let text = std::fs::read_to_string(t.join("sdc.toml")).unwrap();
    let edited = text.replace(
        "[files.'armor/other.png']\nalpha.policy = \"remove\"",
        "[files.'armor/other.png']\nalpha.policy = \"data\"",
    );
    assert_ne!(edited, text, "precondition: the hand edit");
    std::fs::write(t.join("sdc.toml"), edited).unwrap();
    let mut m = opened_as_the_app(t, d.path().join("export"));
    settle(&mut m);
    assert_eq!(auto_data(&m), vec![IRON_N.to_string()], "precondition");
    m.set_slot_command("armor/steel", 2, SlotCommand::Clear)
        .unwrap();
    assert_eq!(
        held(&m, IRON_N, "alpha.policy"),
        None,
        "the step's clear takes"
    );
    assert!(auto_data(&m).is_empty());
    assert_eq!(
        held(&m, OTHER, "alpha.policy"),
        Some(json!("data")),
        "the other file's table stays as the user wrote it"
    );
}
