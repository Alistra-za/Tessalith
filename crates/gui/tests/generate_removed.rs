//! Note GV: the GUI makes no generated maps. A `_generated` file is a plain row, and the old `[generated]` table and
//! `generated` sidecar key are read quietly and never rewritten.
mod support;
use eframe::egui::Rect;
use egui_kittest::kittest::Queryable;
use engine::config::Subtype;
use image::{ImageBuffer, Luma};
use sdc_gui::model::bottom::problems;
use sdc_gui::model::sets::SlotKind;
use sdc_gui::model::{Answer, AppModel};
use sdc_gui::sets_doc::SetsDoc;
use std::path::Path;
use std::time::Duration;
use support::*;

const ROCK_SET: &str = "landscape/rock01";
const KEY: &str = "landscape/rock01_p_generated.png";
/// What Generate once wrote: the normal's hash no longer matches, so the old code called this file stale.
const OLD_TABLE: &str = r#"
[generated."landscape/rock01_p_generated.png"]
from = "landscape/rock01_normal.png"
from_sha256 = "0000"
sha256 = "1111"
kind = "height"
m_alpha = false

[generated."landscape/rock01_p_generated.png".opts]
size = 64
tiling = "auto"
relief = 0.3
curvature = false

[generated."landscape/rock01_p_generated.png".opts.height]
strength = 1.0
neutral = "half"
"#;

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "the model did not settle"
    );
}

fn harness(m: AppModel) -> egui_kittest::Harness<'static, AppModel> {
    let h = egui_kittest::Harness::builder()
        .with_size(eframe::egui::vec2(1500.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

fn toml_path(d: &Path) -> std::path::PathBuf {
    d.join("textures/sdc-project.toml")
}

/// The PBR rock set with a `rock01_p_generated.png` on disk, decided Height, and the old `[generated]` table in
/// `sdc-project.toml`. Saved and reloaded, so the table came from the file.
fn rock_with_old_table(d: &Path) -> AppModel {
    let mut m = pbr_opened_plain(d);
    ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 1000) as u16]))
        .save(d.join("textures/landscape/rock01_p_generated.png"))
        .unwrap();
    m.replan();
    idle(&mut m);
    m.decide(KEY, Answer::Subtype(Subtype::Height));
    idle(&mut m);
    m.save().unwrap();
    let mut text = std::fs::read_to_string(toml_path(d)).unwrap();
    text.push_str(OLD_TABLE);
    std::fs::write(toml_path(d), text).unwrap();
    m.reload_documents();
    idle(&mut m);
    m
}

fn select_rock(m: &mut AppModel) {
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with(ROCK_SET))
        .expect("the rock set")
        .key
        .clone();
    m.select_set(&key);
}

/// Right-click set `id`'s card; the card menu opens.
fn card_menu(h: &mut egui_kittest::Harness<'static, AppModel>, id: &str) {
    let card = |h: &egui_kittest::Harness<'static, AppModel>| -> Rect {
        h.query_all_by_label(id)
            .find(|n| in_sets_column(h, n.rect()))
            .unwrap_or_else(|| panic!("{id}'s card"))
            .rect()
    };
    let r = card(h);
    h.hover_at(r.center());
    h.run();
    h.query_all_by_label(id)
        .find(|n| in_sets_column(h, n.rect()))
        .unwrap()
        .click_secondary();
    h.run();
}

const ENTRIES: [&str; 6] = [
    "Generate from normal map…",
    "Regenerate…",
    "Generate…",
    "Delete generated file…",
    "Delete generated file",
    "Delete generated file ⏵",
];

/// The Height and RMAOS rows' menus offer no Generate, Regenerate or Delete generated file (an entry that merely
/// names a `_generated` file, like "From this set (…)", is an ordinary one).
fn no_slot_entry(m: &AppModel) {
    for kind in [SlotKind::Height, SlotKind::Rmaos] {
        for e in m.slot_menu(ROCK_SET, kind) {
            let l = e.label.as_str();
            assert!(
                !["Generate", "Regenerate", "Delete generated"]
                    .iter()
                    .any(|p| l.starts_with(p)),
                "{kind:?} menu offers {l:?}"
            );
        }
    }
}

/// RED against the old code: the set card, the Height row's menu and the set editor offer no Generate, Regenerate or
/// Delete generated file, with or without a generated file in the set.
#[test]
fn the_generate_entries_are_gone() {
    let d = tempfile::tempdir().unwrap();
    // A set with a normal and no height, and one with a generated file: the old code offered Generate on the first
    // and Delete on the second.
    let m = pbr_opened_plain(d.path());
    no_slot_entry(&m);
    let mut h = harness(m);
    h.run();
    card_menu(&mut h, ROCK_SET);
    for l in ENTRIES {
        assert!(
            h.query_all_by_label(l).next().is_none(),
            "the plain set offers {l:?}"
        );
    }
    let d = tempfile::tempdir().unwrap();
    let mut m = rock_with_old_table(d.path());
    no_slot_entry(&m);
    select_rock(&mut m);
    let mut h = harness(m);
    h.run();
    for l in ENTRIES {
        assert!(
            h.query_all_by_label(l).next().is_none(),
            "the editor offers {l:?}"
        );
    }
    card_menu(&mut h, ROCK_SET);
    for l in ENTRIES {
        assert!(
            h.query_all_by_label(l).next().is_none(),
            "the card menu offers {l:?}"
        );
    }
}

/// RED against the old code: with a recorded, stale `[generated]` entry the Height row had a "generated" marker, a
/// "Generated from" hover and a Problems row with a Regenerate button. Now it is a row like any other file.
#[test]
fn a_generated_file_is_a_plain_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock_with_old_table(d.path());
    assert!(m.item(KEY).is_some(), "the generated file is a source");
    select_rock(&mut m);
    for p in problems(&m).iter() {
        let said = format!("{} {:?}", p.text, p.fix);
        assert!(
            !said.contains("earlier") && !said.contains("Regenerate"),
            "a stale row: {p:?}"
        );
    }
    let mut h = harness(m);
    h.run();
    let row = slot_rect(&h, "Height _p");
    assert!(
        h.query_all_by_label("generated").next().is_none(),
        "the marker"
    );
    assert!(h
        .query_all_by_label_contains("Generated from")
        .next()
        .is_none());
    let file = h
        .query_all_by_label("rock01_p_generated.png")
        .map(|n| n.rect())
        .filter(|r| r.min.x > row.max.x)
        .max_by(|a, b| a.min.x.total_cmp(&b.min.x))
        .expect("the File cell");
    h.hover_at(file.center());
    h.run();
    assert!(
        h.query_all_by_label_contains("Generated from")
            .next()
            .is_none(),
        "the File cell's hover"
    );
    assert!(h.query_all_by_label_contains("stale").next().is_none());
}

/// Pin (passes on the old code too): an old `[generated]` table loads with no message and a later save leaves its
/// text as it was.
#[test]
fn an_old_generated_table_loads_and_stays() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock_with_old_table(d.path());
    assert!(m.dialog.is_none());
    assert_eq!(m.status_line, "8 textures, 3 sets", "no message about it");
    assert!(!m.sets_doc.as_ref().unwrap().is_dirty(), "loads quietly");
    let tail = |d: &Path| {
        let t = std::fs::read_to_string(toml_path(d)).unwrap();
        t[t.find("[generated.").expect("the table")..].to_string()
    };
    let before = tail(d.path());
    m.sets_doc
        .as_mut()
        .unwrap()
        .set_order(&[ROCK_SET.to_string()]);
    assert!(m.sets_doc.as_ref().unwrap().is_dirty());
    m.save().unwrap();
    idle(&mut m);
    assert_eq!(tail(d.path()), before, "the table text is unchanged");
}

fn sidecar(d: &Path) -> serde_json::Map<String, serde_json::Value> {
    let b = std::fs::read(d.join("_sdc/generated.json")).unwrap();
    serde_json::from_slice(&b).unwrap()
}

fn old_sidecar(d: &Path) -> SetsDoc {
    std::fs::create_dir_all(d.join("_sdc")).unwrap();
    std::fs::write(
        d.join("_sdc/generated.json"),
        r#"{"generated":{"a_p_generated.png":{"from":"a_n.png"}},
            "pgp_json_sha256":{"a.json":"keep"},
            "cs_pbr_sha256":{"b.json":"keep too"},
            "last_json_sha256":"last"}"#,
    )
    .unwrap();
    SetsDoc::open(d)
}

/// Pin (passes on the old code too): a sidecar with the old `generated` key keeps every hash across a write.
#[test]
fn an_old_generated_key_keeps_the_hashes() {
    let d = tempfile::tempdir().unwrap();
    let mut doc = old_sidecar(d.path());
    doc.set_cm_json_shas(&[("c.json".to_string(), "new".to_string())].into())
        .unwrap();
    let side = sidecar(d.path());
    assert_eq!(side["pgp_json_sha256"]["a.json"], "keep");
    assert_eq!(side["cs_pbr_sha256"]["b.json"], "keep too");
    assert_eq!(side["last_json_sha256"], "last");
    assert_eq!(side["cm_json_sha256"]["c.json"], "new");
    assert_eq!(doc.pgp_json_shas()["a.json"], "keep");
}

/// RED against the old code: the next write of the sidecar drops the old `generated` key.
#[test]
fn the_next_sidecar_write_drops_the_old_generated_key() {
    let d = tempfile::tempdir().unwrap();
    let mut doc = old_sidecar(d.path());
    assert!(sidecar(d.path()).contains_key("generated"), "the fixture");
    doc.set_cm_json_shas(&[("c.json".to_string(), "new".to_string())].into())
        .unwrap();
    assert!(!sidecar(d.path()).contains_key("generated"));
}
