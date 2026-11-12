//! Note DU: a set entry that matches no file (the card's red "?" chip) can be removed from the chip's menu and from its
//! Problems row; choosing the Advanced Skin shader makes the set's diffuse `skin_albedo`, and leaving it reverts that.
mod support;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::config::Subtype;
use engine::shader::Shader;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::bottom::{problems, Fix};
use sdc_gui::model::messages::Severity;
use sdc_gui::model::sets::{set_views, Missing};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use support::{pbr_opened, session};

const ROCK: &str = "landscape/rock01";
/// A file that is on disk but is no texture, so no plan lists it: its `add` line stays a red "?" chip (Note HE2).
const GONE: &str = "landscape/notes.txt";
/// A texture deleted from disk: its line clears itself (Note HE2).
const DELETED: &str = "landscape/deleted.png";

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

fn texts(m: &AppModel) -> (String, String) {
    (
        m.project.as_ref().unwrap().text(),
        m.sets_doc.as_ref().unwrap().text(),
    )
}

fn missing_of(m: &AppModel, id: &str) -> Vec<Missing> {
    set_views(m)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("set {id}"))
        .missing
}

/// rock01 with an `add` entry naming a file that is on disk but no plan lists.
fn with_gone(dir: &Path) -> AppModel {
    let mut m = pbr_opened(dir);
    std::fs::write(m.root.clone().unwrap().join(GONE), "notes").unwrap();
    m.edit_docs("t", None, true, |_, s| s.add_member(ROCK, GONE));
    idle(&mut m);
    assert_eq!(missing_of(&m, ROCK), vec![Missing::File(GONE.into())]);
    m
}

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    harness_at(model, 1600.0)
}

fn harness_at(model: AppModel, w: f32) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(egui::vec2(w, 1000.0))
        .with_step_dt(0.02)
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

fn settle(h: &mut Harness<'static, AppModel>) {
    h.run();
    idle(h.state_mut());
    h.run();
}

// ---- Remove a missing entry: the model ----

/// DU1: the `add` entry goes from the project file, as one undo step.
#[test]
fn a_missing_add_entry_is_removed_in_one_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_gone(d.path());
    let before = texts(&m);
    let steps = m.history.undo.len();
    m.remove_missing(ROCK, &Missing::File(GONE.into()))
        .expect("removed");
    idle(&mut m);
    assert!(missing_of(&m, ROCK).is_empty());
    assert!(!m.sets_doc.as_ref().unwrap().text().contains("notes.txt"));
    assert_eq!(m.history.undo.len(), steps + 1, "one undo step");
    m.undo();
    idle(&mut m);
    assert_eq!(texts(&m), before);
}

/// DU1: a `remove` entry naming no file goes the same way.
#[test]
fn a_missing_remove_entry_is_removed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    std::fs::write(m.root.clone().unwrap().join(GONE), "notes").unwrap();
    m.edit_docs("t", None, true, |_, s| s.add_remove_line(ROCK, GONE));
    idle(&mut m);
    assert_eq!(missing_of(&m, ROCK), vec![Missing::File(GONE.into())]);
    m.remove_missing(ROCK, &Missing::File(GONE.into())).unwrap();
    idle(&mut m);
    assert!(missing_of(&m, ROCK).is_empty());
}

// ---- A deleted file's line clears itself (Note HE2) ----

fn info_lines(m: &AppModel, file: &str) -> Vec<String> {
    m.messages
        .entries()
        .filter(|e| e.severity == Severity::Info && e.text.contains(file))
        .map(|e| e.text.clone())
        .collect()
}

/// A project with a list line (`add` or `remove`) naming `DELETED`, which is not on disk; the line is cleared by the
/// plan the edit starts, with no undo step of its own.
fn with_deleted(dir: &Path, remove: bool) -> (AppModel, usize) {
    let mut m = pbr_opened(dir);
    m.edit_docs("t", None, true, |_, s| {
        if remove {
            s.add_remove_line(ROCK, DELETED)
        } else {
            s.add_member(ROCK, DELETED)
        }
    });
    let steps = m.history.undo.len();
    idle(&mut m);
    (m, steps)
}

fn assert_cleared(m: &AppModel, steps: usize) {
    assert!(missing_of(m, ROCK).is_empty(), "the chip is gone");
    assert!(!m.sets_doc.as_ref().unwrap().text().contains("deleted.png"));
    let info = info_lines(m, "deleted.png");
    assert_eq!(info.len(), 1, "{info:?}");
    assert!(info[0].contains(ROCK), "names the set: {}", info[0]);
    assert_eq!(m.history.undo.len(), steps, "the clear is no undo step");
}

#[test]
fn a_deleted_add_line_is_cleared_with_an_info_line() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, steps) = with_deleted(d.path(), false);
    assert_cleared(&m, steps);
    // The clear is an unsaved edit like any other: dirty, and Save writes it.
    assert!(m.sets_doc.as_ref().unwrap().is_dirty());
    m.save().expect("saved");
    let on_disk =
        std::fs::read_to_string(m.root.clone().unwrap().join("sdc-project.toml")).unwrap();
    assert!(!on_disk.contains("deleted.png"), "{on_disk}");
}

#[test]
fn a_deleted_remove_line_is_cleared() {
    let d = tempfile::tempdir().unwrap();
    let (m, steps) = with_deleted(d.path(), true);
    assert_cleared(&m, steps);
}

/// Undo keeps its redo step through the clear; redo brings the line back and the next plan clears it again, once.
#[test]
fn undo_brings_the_line_back_and_it_is_cleared_again() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, steps) = with_deleted(d.path(), false);
    m.undo();
    idle(&mut m);
    assert!(!m.sets_doc.as_ref().unwrap().text().contains("deleted.png"));
    assert_eq!(m.history.redo.len(), 1, "the clear loses no redo");
    m.redo();
    idle(&mut m);
    assert!(missing_of(&m, ROCK).is_empty(), "cleared again");
    assert!(!m.sets_doc.as_ref().unwrap().text().contains("deleted.png"));
    assert_eq!(info_lines(&m, "deleted.png").len(), 2, "one line per clear");
    assert_eq!(m.history.undo.len(), steps, "still no step of its own");
}

/// A project file that does not parse takes no write and says nothing: the file on disk is left as the user has it.
/// (A read-only document holds no entries, so no chip can exist here; the guard is a second line of defence.)
#[test]
fn a_read_only_project_keeps_the_line_and_the_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_gone(d.path());
    let root = m.root.clone().unwrap();
    let broken = "[[sets]\nbroken";
    std::fs::write(root.join("sdc-project.toml"), broken).unwrap();
    m.reload_documents();
    idle(&mut m);
    std::fs::remove_file(root.join(GONE)).unwrap();
    m.replan();
    idle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().read_only(), "read-only");
    assert!(info_lines(&m, "notes.txt").is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("sdc-project.toml")).unwrap(),
        broken
    );
    let e = m
        .remove_missing(ROCK, &Missing::File(GONE.into()))
        .unwrap_err();
    assert!(e.contains("read-only"), "{e}");
}

/// A file on disk that no plan lists (not a texture this project reads) keeps its chip, Problems row and menu.
#[test]
fn a_file_on_disk_outside_the_plan_keeps_its_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_gone(d.path());
    assert!(info_lines(&m, "notes.txt").is_empty());
    assert!(problems(&m)
        .iter()
        .any(|p| p.fix == Some(Fix::RemoveMissing(Missing::File(GONE.into())))));
    m.remove_missing(ROCK, &Missing::File(GONE.into()))
        .expect("removed");
    idle(&mut m);
    assert!(missing_of(&m, ROCK).is_empty());
}

/// A renamed or unplugged folder must not lose the lines under it: nothing is cleared, the chips stay and work, and
/// with the folder back the set reads as before (Note HE3).
#[test]
fn a_missing_folder_keeps_its_lines() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let root = m.root.clone().unwrap();
    let sub = root.join("sub");
    let names = ["sub/a.txt", "sub/b.txt"];
    std::fs::create_dir_all(&sub).unwrap();
    for n in names {
        std::fs::write(root.join(n), "x").unwrap();
    }
    m.edit_docs("t", None, true, |_, s| {
        for n in names {
            s.add_member(ROCK, n)
        }
    });
    idle(&mut m);
    let want: Vec<Missing> = names.iter().map(|n| Missing::File((*n).into())).collect();
    assert_eq!(missing_of(&m, ROCK), want);
    let text = m.sets_doc.as_ref().unwrap().text();
    let moved = d.path().join("sub-away");
    std::fs::rename(&sub, &moved).unwrap();
    m.replan();
    idle(&mut m);
    assert_eq!(missing_of(&m, ROCK), want, "the chips stay");
    assert_eq!(m.sets_doc.as_ref().unwrap().text(), text, "no line lost");
    assert!(info_lines(&m, "sub/").is_empty());
    std::fs::rename(&moved, &sub).unwrap();
    m.replan();
    idle(&mut m);
    assert_eq!(missing_of(&m, ROCK), want, "the set reads as before");
    assert_eq!(m.sets_doc.as_ref().unwrap().text(), text);
    // The chip works while the folder is away too.
    std::fs::rename(&sub, &moved).unwrap();
    m.replan();
    idle(&mut m);
    m.remove_missing(ROCK, &want[0]).expect("removed");
    idle(&mut m);
    assert_eq!(missing_of(&m, ROCK), want[1..].to_vec());
}

/// Every set with a deleted file in a folder that is still there is cleared, not only the first.
#[test]
fn a_clear_reaches_every_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let other = "landscape/leaf";
    m.edit_docs("t", None, true, |_, s| {
        s.add_member(ROCK, DELETED);
        s.add_member(other, "landscape/deleted2.png");
    });
    idle(&mut m);
    for (id, file) in [(ROCK, "deleted.png"), (other, "deleted2.png")] {
        assert!(missing_of(&m, id).is_empty(), "{id} still has a chip");
        assert_eq!(info_lines(&m, file).len(), 1, "{file}");
    }
    assert!(!m.sets_doc.as_ref().unwrap().text().contains("deleted"));
}

/// An entry with settings and no files keeps its "no files left" chip (Note HE3): it holds the user's settings.
#[test]
fn an_entry_left_empty_keeps_its_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.edit_docs("t", None, true, |_, s| {
        s.set_shader("mine/ghost", Some(Shader::Pbr))
    });
    idle(&mut m);
    assert_eq!(missing_of(&m, "mine/ghost"), vec![Missing::NoFilesLeft]);
    assert!(m.sets_doc.as_ref().unwrap().text().contains("mine/ghost"));
    assert!(info_lines(&m, "ghost").is_empty());
}

/// A plan with no items (a drive briefly gone) must not wipe the lines.
#[test]
fn no_clear_when_the_plan_has_no_items() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.edit_docs("t", None, true, |_, s| {
        s.add_member(ROCK, "landscape/leaf.png")
    });
    idle(&mut m);
    let root = m.root.clone().unwrap();
    for f in std::fs::read_dir(root.join("landscape")).unwrap() {
        std::fs::remove_file(f.unwrap().path()).unwrap();
    }
    m.replan();
    idle(&mut m);
    assert!(m.items.is_empty(), "the plan has no items");
    let text = m.sets_doc.as_ref().unwrap().text();
    assert!(text.contains("leaf.png"), "the line stays: {text}");
    assert!(info_lines(&m, "leaf.png").is_empty());
}

/// DU2: a borrow from a set that does not exist is dropped.
#[test]
fn a_borrow_from_a_missing_set_is_removed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow(ROCK, 6, Some("landscape/nowhere"))
    });
    idle(&mut m);
    let miss = missing_of(&m, ROCK);
    assert_eq!(miss.len(), 1, "{miss:?}");
    assert_eq!(miss[0].text(), "slot6 from landscape/nowhere");
    m.remove_missing(ROCK, &miss[0]).unwrap();
    idle(&mut m);
    assert!(missing_of(&m, ROCK).is_empty());
    assert!(!m.sets_doc.as_ref().unwrap().text().contains("nowhere"));
}

/// DU4: a read-only sdc-project.toml refuses, with the reason.
#[test]
fn removing_a_missing_entry_is_refused_when_read_only() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_gone(d.path());
    let root = m.root.clone().unwrap();
    std::fs::write(root.join("sdc-project.toml"), "[[sets]\nbroken").unwrap();
    m.reload_documents();
    idle(&mut m);
    let e = m
        .remove_missing(ROCK, &Missing::File(GONE.into()))
        .unwrap_err();
    assert!(e.contains("read-only"), "{e}");
}

// ---- Remove a missing entry: the Problems row ----

/// DU3: each missing entry is a Problems row on its set, carrying the entry.
#[test]
fn a_missing_entry_is_a_problems_row() {
    let d = tempfile::tempdir().unwrap();
    let m = with_gone(d.path());
    let rows = problems(&m);
    let row = rows
        .iter()
        .find(|p| p.fix == Some(Fix::RemoveMissing(Missing::File(GONE.into()))))
        .unwrap_or_else(|| panic!("no row: {rows:#?}"));
    assert!(row.text.contains(GONE), "{}", row.text);
}

#[test]
fn the_problems_row_removes_the_entry() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_gone(d.path());
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(
        egui::accesskit::Role::Button,
        &format!("Remove {GONE} from {ROCK}"),
    )
    .click();
    settle(&mut h);
    assert!(missing_of(h.state(), ROCK).is_empty());
    assert!(problems(h.state())
        .iter()
        .all(|p| !matches!(p.fix, Some(Fix::RemoveMissing(_)))));
}

// ---- Remove a missing entry: the "?" chip's menu ----

fn missing_chip(h: &Harness<'static, AppModel>) -> egui::Rect {
    h.get_by_role_and_label(egui::accesskit::Role::Button, &format!("Missing: {GONE}"))
        .rect()
}

#[test]
fn the_question_mark_chip_menu_removes_the_entry() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(with_gone(d.path()));
    h.run();
    missing_chip(&h);
    h.get_by_role_and_label(egui::accesskit::Role::Button, &format!("Missing: {GONE}"))
        .click_secondary();
    h.run();
    // The chip's own menu, not the card's (the card's has "Rename").
    assert!(h.query_by_label("Rename").is_none(), "the card menu opened");
    h.get_by_label("Remove from set").click();
    settle(&mut h);
    assert!(missing_of(h.state(), ROCK).is_empty());
}

/// DU5: an own member's chip menu also offers Remove from set.
#[test]
fn a_member_chip_menu_offers_remove_from_set() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    h.run();
    // The Sets column is leftmost: its card's N chip, not the set editor's.
    let n = h
        .query_all_by_role_and_label(egui::accesskit::Role::Button, "N")
        .min_by(|a, b| a.rect().min.x.total_cmp(&b.rect().min.x))
        .expect("the N chip");
    n.click_secondary();
    h.run();
    let normal = "landscape/rock01_normal.png";
    let in_rock = |m: &AppModel| {
        set_views(m)
            .iter()
            .any(|v| v.id == ROCK && v.members.iter().any(|k| k == normal))
    };
    assert!(in_rock(h.state()));
    h.get_by_label("Remove from set").click();
    settle(&mut h);
    assert!(!in_rock(h.state()), "the member left the set");
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(&mut h);
    assert!(in_rock(h.state()), "Ctrl+Z brings it back");
}

// ---- Advanced Skin sets skin_albedo ----

const DIFFUSE: &str = "m/rock01.png";
const RMAOS: &str = "m/rock01_rmaos";
const RFAOS: &str = "m/rock01_rfaos";
const SET: &str = "m/rock01";

fn grey(dir: &Path, name: &str, v: u8) {
    ImageBuffer::from_fn(64, 64, |x, _| Luma([v.wrapping_add(x as u8)]))
        .save(dir.join(name))
        .unwrap();
}

/// A cs set: diffuse, roughness (packed `_rmaos`) and `_sk`, as in packing_fills.
fn skin_fixture(dir: &Path) -> AppModel {
    let r = dir.join("m");
    std::fs::create_dir_all(&r).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(r.join("rock01.png"))
        .unwrap();
    grey(&r, "rock01_roughness.png", 120);
    grey(&r, "rock01_sk.png", 0);
    let m = skin_fixture_reopen(dir);
    assert!(m.item(RMAOS).is_some());
    m
}

fn subtype(m: &AppModel) -> Option<serde_json::Value> {
    m.project.as_ref().unwrap().file_value(DIFFUSE, "subtype")
}

fn resolved(m: &AppModel) -> Subtype {
    m.item(DIFFUSE)
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .profile
        .subtype
}

/// DU6: Advanced Skin makes the diffuse `skin_albedo` (and so the packed output `_rfaos`, Note DQ), one undo step.
#[test]
fn advanced_skin_makes_the_diffuse_skin_albedo_in_one_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_fixture(d.path());
    let before = texts(&m);
    let steps = m.history.undo.len();
    m.set_shader(SET, Some(Shader::AdvancedSkin));
    idle(&mut m);
    assert_eq!(resolved(&m), Subtype::SkinAlbedo);
    assert_eq!(subtype(&m), Some(json!(Subtype::SkinAlbedo)));
    assert!(m.item(RFAOS).is_some(), "the packed output is _rfaos");
    assert_eq!(m.history.undo.len(), steps + 1, "one undo step");
    m.undo();
    idle(&mut m);
    assert_eq!(texts(&m), before);
    assert!(m.item(RMAOS).is_some());
}

/// DU7: leaving Advanced Skin reverts what it set: the sub-type goes back to inferred, the output to `_rmaos`.
#[test]
fn leaving_advanced_skin_reverts_the_auto_sub_type() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_fixture(d.path());
    let was = resolved(&m);
    m.set_shader(SET, Some(Shader::AdvancedSkin));
    idle(&mut m);
    let steps = m.history.undo.len();
    m.set_shader(SET, Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m), None, "back to inferred");
    assert_eq!(resolved(&m), was);
    assert!(m.item(RMAOS).is_some(), "the packed output is _rmaos again");
    assert!(m.item(RFAOS).is_none());
    assert_eq!(m.history.undo.len(), steps + 1, "one undo step");
}

/// DU7: back to Auto counts as leaving it too.
#[test]
fn auto_reverts_too() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_fixture(d.path());
    m.set_shader(SET, Some(Shader::AdvancedSkin));
    idle(&mut m);
    m.set_shader(SET, None);
    idle(&mut m);
    assert_eq!(subtype(&m), None);
    assert!(m.item(RMAOS).is_some());
}

/// DU8: a sub-type the user set after the switch stays when the shader changes.
#[test]
fn a_sub_type_the_user_set_since_is_not_reverted() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_fixture(d.path());
    m.set_shader(SET, Some(Shader::AdvancedSkin));
    idle(&mut m);
    // The user picks it by hand (the same value): it is theirs now.
    m.set_field(DIFFUSE, "subtype", json!(Subtype::SkinAlbedo));
    idle(&mut m);
    m.set_shader(SET, Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m), Some(json!(Subtype::SkinAlbedo)));
}

/// DU9: a diffuse pinned after the switch keeps `skin_albedo` when the shader changes (review issue 2).
#[test]
fn a_pinned_diffuse_is_not_reverted() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_fixture(d.path());
    m.set_shader(SET, Some(Shader::AdvancedSkin));
    idle(&mut m);
    m.set_slot_lock(SET, 1, true).unwrap();
    idle(&mut m);
    m.set_shader(SET, Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m), Some(json!(Subtype::SkinAlbedo)));
    assert!(m.item(RFAOS).is_some());
}

/// DU9: a hand-set sub-type other than plain diffuse is never overridden.
#[test]
fn a_hand_set_sub_type_is_kept() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_fixture(d.path());
    m.set_field(DIFFUSE, "subtype", json!(Subtype::PbrAlbedo));
    idle(&mut m);
    m.set_shader(SET, Some(Shader::AdvancedSkin));
    idle(&mut m);
    assert_eq!(subtype(&m), Some(json!(Subtype::PbrAlbedo)));
}

/// DU9: a hand-set plain `diffuse` is switched, and leaving restores it as written.
#[test]
fn a_hand_set_plain_diffuse_is_switched_and_restored() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_fixture(d.path());
    m.set_field(DIFFUSE, "subtype", json!(Subtype::Diffuse));
    idle(&mut m);
    m.set_shader(SET, Some(Shader::AdvancedSkin));
    idle(&mut m);
    assert_eq!(subtype(&m), Some(json!(Subtype::SkinAlbedo)));
    m.set_shader(SET, None);
    idle(&mut m);
    assert_eq!(subtype(&m), Some(json!(Subtype::Diffuse)));
    assert!(m.item(RMAOS).is_some());
}

/// Note DV: a project saved with Advanced Skin on the set (and nothing else) opens skin: the inferred diffuse is
/// `skin_albedo` and the packed card shows RFAOS, with nothing written to either project file.
#[test]
fn an_existing_advanced_skin_set_opens_with_rfaos() {
    let d = tempfile::tempdir().unwrap();
    drop(skin_fixture(d.path()));
    let project = format!("[[sets]]\nid = \"{SET}\"\nshader = \"advanced_skin\"\n");
    std::fs::write(d.path().join("sdc-project.toml"), &project).unwrap();
    let mut m = skin_fixture_reopen(d.path());
    let before = texts(&m);
    assert_eq!(resolved(&m), Subtype::SkinAlbedo);
    assert_eq!(subtype(&m), None, "nothing written to [files]");
    assert!(m.item(RFAOS).is_some(), "the packed output is _rfaos");
    assert!(m.item(RMAOS).is_none());
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with(SET))
        .expect("the set")
        .key
        .clone();
    m.select_set(&key);
    let mut h = harness(m);
    settle(&mut h);
    assert!(
        h.query_all_by_role(egui::accesskit::Role::ComboBox)
            .any(|n| n.value().as_deref() == Some("RFAOS (skin) (auto)")),
        "the packed card shows RFAOS"
    );
    // DU6 does not apply again: choosing Advanced Skin once more writes no sub-type.
    h.state_mut().set_shader(SET, Some(Shader::AdvancedSkin));
    settle(&mut h);
    assert_eq!(texts(h.state()), before);
}

/// Note DW (supersedes DV4): a pin blocks the Advanced Skin reading: a pinned inferred diffuse gets no written
/// sub-type when Advanced Skin is chosen and reads as before, with `_rmaos`.
#[test]
fn a_pinned_diffuse_keeps_its_reading_under_advanced_skin() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_fixture(d.path());
    let was = resolved(&m);
    assert_ne!(was, Subtype::SkinAlbedo);
    m.set_slot_lock(SET, 1, true).unwrap();
    idle(&mut m);
    m.set_shader(SET, Some(Shader::AdvancedSkin));
    idle(&mut m);
    assert_eq!(subtype(&m), None, "nothing written into a pinned slot");
    assert_eq!(resolved(&m), was);
    assert!(m.item(RMAOS).is_some(), "the packed output stays _rmaos");
    assert!(m.item(RFAOS).is_none());
}

/// Note DW4: on a loaded Advanced Skin set, pinning the Diffuse slot re-plans to the pre-DV reading (`_rmaos`), and
/// unpinning flips it back to `skin_albedo` / `_rfaos`; neither writes a sub-type.
#[test]
fn pinning_the_diffuse_of_a_loaded_skin_set_flips_its_reading() {
    let d = tempfile::tempdir().unwrap();
    let was = {
        let m = skin_fixture(d.path());
        resolved(&m)
    };
    let project =
        format!("[[sets]]\nid = \"{SET}\"\nshader = \"advanced_skin\"\nlock = [\"slot1\"]\n");
    std::fs::write(d.path().join("sdc-project.toml"), &project).unwrap();
    let mut m = skin_fixture_reopen(d.path());
    assert!(m.slot_locked(SET, 1));
    assert_eq!(resolved(&m), was, "a pinned diffuse reads as before DV");
    assert!(m.item(RMAOS).is_some(), "the packed output is _rmaos");
    assert!(m.item(RFAOS).is_none());

    m.set_slot_lock(SET, 1, false).unwrap();
    idle(&mut m);
    assert_eq!(resolved(&m), Subtype::SkinAlbedo);
    assert!(
        m.item(RFAOS).is_some(),
        "unpinned: the packed output is _rfaos"
    );
    assert!(m.item(RMAOS).is_none());
    assert_eq!(subtype(&m), None, "nothing written to [files]");

    m.set_slot_lock(SET, 1, true).unwrap();
    idle(&mut m);
    assert_eq!(resolved(&m), was);
    assert!(m.item(RMAOS).is_some(), "pinned again: _rmaos");
}

/// The skin fixture's folder opened again (its files already there).
fn skin_fixture_reopen(dir: &Path) -> AppModel {
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    m
}

/// Screenshots for the report (not a check): the chip menu and the Problems row at 1100 and 1280 wide.
#[test]
#[ignore]
fn set_cleanup_screenshots() {
    let out = std::env::var("SDC_SHOT_DIR")
        .unwrap_or_else(|_| std::env::temp_dir().display().to_string());
    for w in [1100.0, 1280.0] {
        let d = tempfile::tempdir().unwrap();
        let mut m = with_gone(d.path());
        m.open_problems(None);
        let mut h = harness_at(m, w);
        h.run();
        h.get_by_role_and_label(egui::accesskit::Role::Button, &format!("Missing: {GONE}"))
            .click_secondary();
        h.run();
        let img = h.render().expect("render");
        img.save(Path::new(&out).join(format!("set_cleanup_{w}.png")))
            .unwrap();
    }
}
