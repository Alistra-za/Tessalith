//! Note HJ1: Rename outputs is for PGPatcher PBR sets on Community Shaders only (an old `output_name` still works, HJ2).
mod support;
use engine::config::{Subtype, Target};
use engine::sets::OutputKey;
use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::rename::RenameMode;
use sdc_gui::model::sets::{engine_inputs, views};
use sdc_gui::model::{Answer, AppModel};
use std::path::PathBuf;
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01.png";
const LEAF: &str = "landscape/leaf.png";

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)));
}

fn has_outputs_entry(entries: Vec<sdc_gui::model::bulk::MenuEntry>) -> bool {
    entries
        .iter()
        .any(|e| matches!(e.action, MenuAction::RenameOutputs))
}

fn set_id_of(m: &AppModel, item: &str) -> String {
    views(m)
        .iter()
        .find(|v| v.members.iter().any(|k| k == item))
        .expect("the item's set")
        .id
        .clone()
}

#[test]
fn rename_outputs_is_absent_on_a_vanilla_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(LEAF, false, false);
    assert!(!has_outputs_entry(m.item_menu()));
    assert!(m.rename_keys(RenameMode::Outputs).is_empty());
    let leaf = set_id_of(&m, LEAF);
    let lk = views(&m).iter().find(|v| v.id == leaf).unwrap().key.clone();
    m.click_set(&lk, false, false);
    assert!(!has_outputs_entry(m.set_menu()));
}

#[test]
fn rename_outputs_is_absent_on_a_skin_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.decide(LEAF, Answer::Subtype(Subtype::SkinAlbedo));
    idle(&mut m);
    let leaf = set_id_of(&m, LEAF);
    m.set_shader(&leaf, Some(engine::shader::Shader::AdvancedSkin));
    idle(&mut m);
    let v = views(&m).iter().find(|v| v.id == leaf).unwrap().clone();
    assert!(!v.pbr, "a skin set is not a PBR set");
    m.click_item(LEAF, false, false);
    assert!(!has_outputs_entry(m.item_menu()));
    assert!(m.rename_keys(RenameMode::Outputs).is_empty());
}

#[test]
fn rename_outputs_is_absent_on_a_no_albedo_json_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    let leaf = views(&m)
        .iter()
        .find(|v| v.key.ends_with("leaf"))
        .expect("the leaf set")
        .clone();
    assert!(
        leaf.json && !leaf.pbr,
        "the fixture is a no-albedo JSON set"
    );
    m.click_item("arch/leaf_n.png", false, false);
    assert!(!has_outputs_entry(m.item_menu()));
    assert!(m.rename_keys(RenameMode::Outputs).is_empty());
    // The set with an albedo next to it still qualifies.
    m.click_item("arch/wall.png", false, false);
    assert!(has_outputs_entry(m.item_menu()));
}

#[test]
fn rename_outputs_is_absent_under_the_vanilla_target() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_target(Target::Vanilla);
    idle(&mut m);
    m.click_item(ROCK, false, false);
    assert!(!has_outputs_entry(m.item_menu()));
    assert!(m.rename_keys(RenameMode::Outputs).is_empty());
}

#[test]
fn rename_outputs_shows_on_a_pbr_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.click_item(ROCK, false, false);
    let e = m
        .item_menu()
        .into_iter()
        .find(|e| matches!(e.action, MenuAction::RenameOutputs))
        .expect("the entry");
    assert_eq!((e.label.as_str(), e.disabled), ("Rename output…", None));
}

#[test]
fn a_mixed_selection_renames_only_the_pbr_sets() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let (rock, leaf) = (set_id_of(&m, ROCK), set_id_of(&m, LEAF));
    let key_of = |m: &AppModel, id: &str| {
        views(m)
            .iter()
            .find(|v| v.id == id)
            .map(|v| v.key.clone())
            .unwrap()
    };
    let (rk, lk) = (key_of(&m, &rock), key_of(&m, &leaf));
    m.click_set(&lk, false, false);
    m.click_set(&rk, true, false);
    assert_eq!(m.rename_keys(RenameMode::Outputs), vec![rock]);
    let e = m
        .set_menu()
        .into_iter()
        .find(|e| matches!(e.action, MenuAction::RenameOutputs))
        .expect("the entry");
    assert!(e.label.starts_with("Rename output (1)…"), "{}", e.label);
}

#[test]
fn a_mixed_texture_selection_counts_the_pbr_sets() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let label = |m: &AppModel| {
        m.item_menu()
            .into_iter()
            .find(|e| matches!(e.action, MenuAction::RenameOutputs))
            .map(|e| e.label)
    };
    assert_eq!(label(&m).as_deref(), Some("Rename output (1)…"));
    // Two textures of the one PBR set: nothing is left out, so no count.
    m.click_item(ROCK, false, false);
    m.click_item("landscape/rock01_normal.png", true, false);
    assert_eq!(label(&m).as_deref(), Some("Rename output…"));
}

#[test]
fn f2_on_a_vanilla_set_opens_no_dialog_and_says_where_inputs_are_renamed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let leaf = set_id_of(&m, LEAF);
    let lk = views(&m).iter().find(|v| v.id == leaf).unwrap().key.clone();
    m.click_set(&lk, false, false);
    sdc_gui::ui::actions::run("rename", &mut m, &eframe::egui::Context::default());
    assert_eq!(m.dialog, None);
    assert_eq!(
        m.status_line,
        "Rename inputs in the Textures view: select the textures there and press F2"
    );
}

#[test]
fn an_old_output_name_still_exports_and_clears() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    seed_output_name(&mut m, "landscape/leaf", "maple");
    let leaf_key = m
        .items
        .iter()
        .find(|i| i.name == "leaf.png")
        .unwrap()
        .key
        .clone();
    assert_eq!(
        engine_inputs(&m).1.get(&OutputKey::File(leaf_key.clone())),
        Some(&PathBuf::from("textures/landscape/maple.dds"))
    );
    assert_eq!(m.output_name_of("landscape/leaf").as_deref(), Some("maple"));
    // A new name is refused with the HJ1 text; `None` still clears.
    assert_eq!(
        m.set_output_name("landscape/leaf", Some("oak")),
        Err("output names are for PGPatcher PBR sets".to_string())
    );
    m.clear_output_name("landscape/leaf");
    idle(&mut m);
    assert_eq!(m.output_name_of("landscape/leaf"), None);
    assert_eq!(engine_inputs(&m).1.get(&OutputKey::File(leaf_key)), None);
}
