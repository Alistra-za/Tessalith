//! Note HJ3 (owner, 2026-11-09): a rename on a set with more than one match row or a `path_contains` writes slot
//! commands naming the renamed files; a plain rename keeps `match_diffuse` + `rename` (HJ4).
mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use sdc_gui::model::pbr_export::json_entries;
use sdc_gui::model::sets::{set_views, DropChoice, DropOutcome, SlotFill, SlotKind};
use sdc_gui::model::AppModel;
use sdc_gui::pgp::writer::{document, entry_node, to_text, EntryInput};
use serde_json::{json, Value};
use std::time::Duration;
use support::*;

const SET: &str = "landscape/rock01";
const STEEL: &str = r"textures\pbr\landscape\steel.dds";
const STEEL_N: &str = r"textures\pbr\landscape\steel_n.dds";
const STEEL_RMAOS: &str = r"textures\pbr\landscape\steel_rmaos.dds";

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)));
}

fn input(m: &AppModel, id: &str) -> EntryInput {
    json_entries(m)
        .into_iter()
        .find(|(v, _)| v.id == id)
        .unwrap_or_else(|| panic!("no entry for {id}"))
        .1
}

fn entry(m: &AppModel, id: &str) -> Value {
    document(std::slice::from_ref(&input(m, id))).to_value()["entries"][0].clone()
}

/// rock01 (albedo, normal, roughness + AO packing `_rmaos`) renamed to `steel`, with `pairs` set on it first.
fn renamed(dir: &std::path::Path, pairs: &[(&str, Value)]) -> AppModel {
    let mut m = pbr_opened(dir);
    for (k, v) in pairs {
        m.set_json_value(SET, k, Some(v.clone()));
    }
    m.set_json_value(SET, "rename", Some(json!("steel")));
    idle(&mut m);
    m
}

fn assert_slot_form(e: &Value) {
    assert!(
        e.get("rename").is_none() && e.get("match_diffuse").is_none(),
        "{e}"
    );
    assert_eq!(e["slot1"], json!(STEEL), "{e}");
    assert_eq!(e["slot2"], json!(STEEL_N), "{e}");
    assert_eq!(e["slot6"], json!(STEEL_RMAOS), "{e}");
}

/// HJ4: one match row and a file name: the entry PGPatcher sees is the one 2.x wrote, byte for byte.
#[test]
fn a_plain_rename_keeps_its_json() {
    let d = tempfile::tempdir().unwrap();
    let m = renamed(d.path(), &[]);
    let text = to_text(&entry_node(&input(&m, SET)));
    let want = "{\n  \"match_diffuse\": \"landscape\\\\rock01\",\n  \"rename\": \"landscape\\\\steel\"\n}\n";
    assert_eq!(text, want);
}

#[test]
fn a_rename_with_match2_writes_slots() {
    let d = tempfile::tempdir().unwrap();
    let m = renamed(d.path(), &[("match2", json!(r"landscape\rock01_n"))]);
    let e = entry(&m, SET);
    assert_eq!(e["match2"], json!(r"landscape\rock01_n.dds"), "{e}");
    assert_slot_form(&e);
}

#[test]
fn a_rename_with_path_contains_writes_slots() {
    let d = tempfile::tempdir().unwrap();
    let m = renamed(d.path(), &[("path_contains", json!("rock01"))]);
    let e = entry(&m, SET);
    assert_eq!(e["path_contains"], json!("rock01"), "{e}");
    assert_slot_form(&e);
}

fn simulate(
    e: &Value,
    source: &pgpsim::MaterialState,
    exists: &dyn Fn(&str) -> bool,
) -> pgpsim::SimResult {
    let default = document(&[]).to_value()["default"].clone();
    pgpsim::simulate(&pgpsim::SimInput {
        entry: e,
        default: &default,
        nif_path: r"meshes\landscape\boulder.nif",
        source,
        exists,
    })
}

/// A mesh hit through `match2` alone (its diffuse is another texture) is patched onto the renamed files, and only
/// they exist: no `textures\pbr\` copy under the old name is needed.
#[test]
fn pgpatcher_lands_on_the_renamed_files_through_any_match() {
    let d = tempfile::tempdir().unwrap();
    let m = renamed(d.path(), &[("match2", json!(r"landscape\rock01_n"))]);
    let e = entry(&m, SET);
    let mut source = pgpsim::MaterialState::builtin_default();
    source.set_slot_raw(0, r"textures\other\boulder.dds");
    source.set_slot_raw(1, r"textures\landscape\rock01_n.dds");
    let ours = [STEEL, STEEL_N, STEEL_RMAOS];
    let exists = |p: &str| ours.iter().any(|o| o.eq_ignore_ascii_case(p));
    match simulate(&e, &source, &exists) {
        pgpsim::SimResult::Patched { material, .. } => {
            assert_eq!(material.slots[0].raw.to_lowercase(), STEEL);
            assert_eq!(material.slots[1].raw.to_lowercase(), STEEL_N);
            assert_eq!(material.slots[5].raw.to_lowercase(), STEEL_RMAOS);
        }
        other => panic!("{other:?}: {e}"),
    }
}

/// Review Focus 1: a slot-form set with no `_rmaos` fares in PGPatcher as the same set unrenamed does.
#[test]
fn a_slot_form_set_with_no_rmaos() {
    let kind = |rename: bool| {
        let d = tempfile::tempdir().unwrap();
        let mut m = pbr_opened(d.path());
        // Roughness and AO gone: no `_rmaos`.
        for f in ["rock01_roughness.png", "rock01_ao.png"] {
            std::fs::remove_file(d.path().join("textures/landscape").join(f)).unwrap();
        }
        m.replan();
        idle(&mut m);
        m.set_json_value(SET, "path_contains", Some(json!("rock01")));
        if rename {
            m.set_json_value(SET, "rename", Some(json!("steel")));
        }
        idle(&mut m);
        let e = entry(&m, SET);
        assert!(e.get("slot6").is_none(), "{e}");
        let name = if rename { "steel" } else { "rock01" };
        let ours = [
            format!(r"textures\pbr\landscape\{name}.dds"),
            format!(r"textures\pbr\landscape\{name}_n.dds"),
        ];
        let mut source = pgpsim::MaterialState::builtin_default();
        source.set_slot_raw(0, r"textures\landscape\rock01.dds");
        source.set_slot_raw(1, r"textures\landscape\rock01_n.dds");
        let exists = |p: &str| ours.iter().any(|o| o.eq_ignore_ascii_case(p));
        simulate(&e, &source, &exists)
    };
    // PGPatcher fills slot 6 from the matched prefix in both forms and skips the shape: the same outcome. Problems has
    // no row for it either way today.
    let skip = pgpsim::SimResult::Skip(r"textures\pbr\landscape\rock01_rmaos.dds missing".into());
    assert_eq!(kind(true), skip);
    assert_eq!(kind(false), skip);
}

/// The slot-form entry's scan key keeps its matches (the Meshes and preview queries see `match2`).
#[test]
fn the_scan_match_key_keeps_every_match_in_slot_form() {
    let d = tempfile::tempdir().unwrap();
    let m = renamed(d.path(), &[("match2", json!(r"landscape\rock01_n"))]);
    let v = set_views(&m).into_iter().find(|v| v.id == SET).unwrap();
    let key = sdc_gui::model::cs_pbr::set_key(&v).unwrap();
    assert!(
        matches!(&key, modscan::store::TexKey::Exact { slot: 1, path } if path == r"landscape\rock01_n"),
        "{key:?}"
    );
}

#[test]
fn a_locked_slot_stays_unnamed_in_slot_form() {
    let d = tempfile::tempdir().unwrap();
    let m = renamed(
        d.path(),
        &[
            ("match2", json!(r"landscape\rock01_n")),
            ("lock_normal", json!(true)),
        ],
    );
    let e = entry(&m, SET);
    assert!(e.get("slot2").is_none(), "{e}");
    assert_eq!(e["slot1"], json!(STEEL), "{e}");
}

#[test]
fn a_users_slot_wins_in_slot_form() {
    let d = tempfile::tempdir().unwrap();
    let mine = r"textures\mine\plate_n.dds";
    let m = renamed(
        d.path(),
        &[
            ("match2", json!(r"landscape\rock01_n")),
            ("slot2", json!(mine)),
        ],
    );
    let e = entry(&m, SET);
    assert_eq!(e["slot2"], json!(mine), "{e}");
    assert_eq!(e["slot1"], json!(STEEL), "{e}");
}

/// Pin: a set borrowing the renamed set's normal names the moved file (borrowed slots are always named).
#[test]
fn a_borrower_names_the_moved_file() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    // The borrower sorts after rock01, so rock01 stays the normal's writer (first writer wins).
    let dir = d.path().join("textures/landscape");
    std::fs::copy(dir.join("rock01.png"), dir.join("zz.png")).unwrap();
    m.replan();
    idle(&mut m);
    let zz = m
        .items
        .iter()
        .find(|i| i.name == "zz.png")
        .unwrap()
        .key
        .clone();
    m.set_field(&zz, "subtype", json!("pbr_albedo"));
    idle(&mut m);
    let id = m.drop_new_set(&zz).unwrap();
    idle(&mut m);
    let normal = set_views(&m)
        .iter()
        .find(|v| v.id == SET)
        .and_then(|v| match v.slots.get(&SlotKind::Normal) {
            Some(SlotFill::Own { item }) => Some(item.clone()),
            _ => None,
        })
        .expect("rock01 owns a normal map");
    assert!(matches!(
        m.drop_choice(&normal, &id, DropChoice::Share),
        DropOutcome::Joined
    ));
    m.set_json_value(SET, "match2", Some(json!(r"landscape\rock01_n")));
    m.set_json_value(SET, "rename", Some(json!("steel")));
    idle(&mut m);
    assert_eq!(entry(&m, &id)["slot2"], json!(STEEL_N));
    assert_slot_form(&entry(&m, SET));
}

/// The refusals of Note AN2 are gone: the model takes a second match on a renamed set.
#[test]
fn adding_a_match_to_a_renamed_set_is_allowed() {
    let d = tempfile::tempdir().unwrap();
    let m = renamed(d.path(), &[]);
    let mut m = m;
    m.set_json_value(SET, "match2", Some(json!(r"landscape\rock01_n")));
    idle(&mut m);
    assert_eq!(entry(&m, SET)["match2"], json!(r"landscape\rock01_n.dds"));
}

fn settings(m: &AppModel) -> sdc_gui::pgp::settings::Settings {
    set_views(m)
        .into_iter()
        .find(|v| v.id == SET)
        .unwrap()
        .settings
}

/// The set editor's Match on this (normal card) is on offer on a renamed set and adds `match2`.
#[test]
fn the_set_editor_adds_a_match_to_a_renamed_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = renamed(d.path(), &[]);
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with(SET))
        .unwrap()
        .key
        .clone();
    m.select_set(&key);
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    let mut h = h;
    h.run();
    let toggle = || h.query_all_by_label("Match on this").nth(1).unwrap();
    toggle().scroll_to_me();
    h.run();
    h.query_all_by_label("Match on this")
        .nth(1)
        .unwrap()
        .click();
    h.run();
    assert_eq!(
        settings(h.state()).get("match2"),
        Some(&json!(r"landscape\rock01_n"))
    );
}

/// The JSON editor's + key › Match offers slot 2 on a renamed set; it opens a pending pill.
#[test]
fn the_json_editor_adds_a_match_to_a_renamed_set() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::NodeT as _;
    let d = tempfile::tempdir().unwrap();
    let mut m = renamed(d.path(), &[]);
    m.open_json_editor();
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 1200.0))
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, sdc_gui::ui::State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, sdc_gui::ui::State::default()),
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    let mut h = h;
    h.run();
    let pills = |h: &Harness<'static, (AppModel, sdc_gui::ui::State)>| {
        h.query_all_by_role(Role::ComboBox)
            .filter_map(|n| n.accesskit_node().value())
            .filter(|v| v.starts_with("Match: exact, slot"))
            .count()
    };
    let before = pills(&h);
    h.query_all_by_label("+ key").next().unwrap().click();
    h.run();
    h.query_all_by_label_contains("Match")
        .last()
        .unwrap()
        .click();
    h.run();
    h.query_all_by_label("Match: exact, slot 2 (normal)")
        .last()
        .unwrap()
        .click();
    h.run();
    h.run();
    assert_eq!(pills(&h), before + 1, "no pending pill for slot 2");
}
