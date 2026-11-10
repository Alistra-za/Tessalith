//! Note HJ3 (owner, 2026-11-09): a rename on a set with more than one match row or a `path_contains` writes slot
//! commands naming the renamed files; a plain rename keeps `match_diffuse` + `rename` (HJ4).
mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use sdc_gui::model::pbr_export::json_entries;
use sdc_gui::model::rename::{plan_rename, RenameDialog, RenameMode, RenamePlan, RenameRow};
use sdc_gui::model::sets::{set_views, DropChoice, DropOutcome, SlotFill, SlotKind};
use sdc_gui::model::{AppModel, Dialog};
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
    let mut m = renamed(d.path(), &[]);
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

// ---- Note HJ5: whole path renames ----

const PLATE: &str = r"armor\steel\plate";

/// `pbr_opened` with rock01's `rename` set to `rename`.
fn moved(dir: &std::path::Path, rename: &str) -> AppModel {
    let mut m = pbr_opened(dir);
    m.set_json_value(SET, "rename", Some(json!(rename)));
    idle(&mut m);
    m
}

/// The explicit output paths the engine gets, `/`, lower case.
fn engine_paths(m: &AppModel) -> Vec<String> {
    sdc_gui::model::sets::engine_inputs(m)
        .1
        .values()
        .map(|p| p.to_string_lossy().replace('\\', "/").to_lowercase())
        .collect()
}

/// Every job's output, `/`, lower case.
fn job_outputs(m: &AppModel) -> Vec<String> {
    m.items
        .iter()
        .filter_map(|i| i.job.as_ref())
        .map(|j| j.output.to_string_lossy().replace('\\', "/").to_lowercase())
        .collect()
}

#[test]
fn whole_path_moves_the_outputs() {
    let d = tempfile::tempdir().unwrap();
    let m = moved(d.path(), PLATE);
    let paths = engine_paths(&m);
    let export = d
        .path()
        .join("export")
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    for f in ["plate.dds", "plate_n.dds", "plate_rmaos.dds"] {
        let rel = format!("textures/armor/steel/{f}");
        assert!(paths.contains(&rel), "{rel} not in {paths:?}");
        let full = format!("{export}/{rel}");
        assert!(
            job_outputs(&m).contains(&full),
            "{full} not in {:?}",
            job_outputs(&m)
        );
    }
}

#[test]
fn whole_path_writes_slots() {
    let d = tempfile::tempdir().unwrap();
    let m = moved(d.path(), PLATE);
    let e = entry(&m, SET);
    assert_eq!(e["match1"], json!(r"landscape\rock01.dds"), "{e}");
    assert!(
        e.get("rename").is_none() && e.get("match_diffuse").is_none(),
        "{e}"
    );
    assert_eq!(e["slot1"], json!(r"textures\armor\steel\plate.dds"), "{e}");
    assert_eq!(
        e["slot2"],
        json!(r"textures\armor\steel\plate_n.dds"),
        "{e}"
    );
    assert_eq!(
        e["slot6"],
        json!(r"textures\armor\steel\plate_rmaos.dds"),
        "{e}"
    );
}

#[test]
fn pgpatcher_lands_on_a_moved_file() {
    let d = tempfile::tempdir().unwrap();
    let m = moved(d.path(), PLATE);
    let e = entry(&m, SET);
    let mut source = pgpsim::MaterialState::builtin_default();
    source.set_slot_raw(0, r"textures\landscape\rock01.dds");
    source.set_slot_raw(1, r"textures\landscape\rock01_n.dds");
    let ours = [
        r"textures\armor\steel\plate.dds",
        r"textures\armor\steel\plate_n.dds",
        r"textures\armor\steel\plate_rmaos.dds",
    ];
    let exists = |p: &str| ours.iter().any(|o| o.eq_ignore_ascii_case(p));
    match simulate(&e, &source, &exists) {
        pgpsim::SimResult::Patched { material, .. } => {
            assert_eq!(material.slots[0].raw.to_lowercase(), ours[0]);
            assert_eq!(material.slots[1].raw.to_lowercase(), ours[1]);
            assert_eq!(material.slots[5].raw.to_lowercase(), ours[2]);
        }
        other => panic!("{other:?}: {e}"),
    }
}

#[test]
fn textures_prefix_is_not_doubled() {
    let d = tempfile::tempdir().unwrap();
    let m = moved(d.path(), r"textures\armor\plate");
    let paths = engine_paths(&m);
    assert!(
        paths.contains(&"textures/armor/plate.dds".to_string()),
        "{paths:?}"
    );
    assert!(!paths.iter().any(|p| p.contains("textures/textures")));
}

/// The Outputs dialog over `ids`, opened as the user opens it (the sets selected, Rename outputs).
fn open_outputs(m: &mut AppModel, ids: &[&str]) {
    let keys: Vec<String> = set_views(m)
        .iter()
        .filter(|v| ids.contains(&v.id.as_str()))
        .map(|v| v.key.clone())
        .collect();
    for (i, k) in keys.iter().enumerate() {
        m.click_set(k, i > 0, false);
    }
    m.open_rename(RenameMode::Outputs);
    assert!(
        matches!(&m.dialog, Some(Dialog::Rename(d)) if d.keys.len() == ids.len()),
        "{:?}",
        m.dialog
    );
}

fn dlg(m: &mut AppModel) -> &mut RenameDialog {
    match m.dialog.as_mut() {
        Some(Dialog::Rename(d)) => d,
        other => panic!("no rename dialog: {other:?}"),
    }
}

fn plan(m: &AppModel) -> RenamePlan {
    match &m.dialog {
        Some(Dialog::Rename(d)) => plan_rename(m, d),
        _ => panic!("no rename dialog"),
    }
}

fn row_of(p: &RenamePlan, id: &str) -> RenameRow {
    p.rows
        .iter()
        .find(|r| r.key == id)
        .cloned()
        .unwrap_or_else(|| panic!("no row {id}: {:?}", p.rows))
}

#[test]
fn a_whole_path_under_an_inner_root_lands_in_the_export_folder() {
    use sdc_gui::model::Answer;
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("Mod/textures/clutter");
    std::fs::create_dir_all(&root).unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
    })
    .save(root.join("cup.png"))
    .unwrap();
    image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb([128u8, 128, 255]))
        .save(root.join("cup_n.png"))
        .unwrap();
    let mut m = opened_as_the_app(root, d.path().join("export"));
    m.decide(
        "cup.png",
        Answer::Subtype(engine::config::Subtype::PbrAlbedo),
    );
    idle(&mut m);
    let id = set_views(&m)
        .iter()
        .find(|v| v.members.iter().any(|k| k == "cup.png"))
        .expect("cup's set")
        .id
        .clone();
    open_outputs(&mut m, &[&id]);
    dlg(&mut m).typed.insert(id.clone(), r"armor\plate".into());
    let r = row_of(&plan(&m), &id);
    assert_eq!(r.problem, None);
    assert!(r.output.starts_with("textures/armor/plate.dds"), "{r:?}");
    assert!(m.apply_rename());
    idle(&mut m);
    let export = d.path().join("export");
    let want = export
        .join("textures/armor/plate.dds")
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    assert!(job_outputs(&m).contains(&want), "{:?}", job_outputs(&m));
    support::export_through(&mut m);
    idle(&mut m);
    assert!(export.join("textures/armor/plate.dds").is_file());
    let inner = export.join("textures/clutter");
    let under: Vec<_> = std::fs::read_dir(&inner)
        .map(|r| r.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(
        !under.iter().any(|p| p.to_string_lossy().contains("plate")),
        "{under:?}"
    );
}

#[test]
fn a_path_outside_data_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let derived = r"landscape\rock01";
    for bad in [r"..\x", r"C:\x"] {
        let mut s = sdc_gui::pgp::settings::Settings::new();
        s.insert("rename".into(), json!(bad));
        assert!(
            sdc_gui::pgp::prefix::prefix_of(&s, derived).is_err(),
            "{bad}"
        );
        open_outputs(&mut m, &[SET]);
        dlg(&mut m).typed.insert(SET.into(), bad.into());
        let p = plan(&m);
        assert!(row_of(&p, SET).problem.is_some(), "{bad}: {:?}", p.rows);
        assert!(!p.can_apply(), "{bad}");
        assert!(m.set_output_name(SET, Some(bad)).is_err(), "{bad}");
        m.dialog = None;
    }
}

#[test]
fn a_path_ending_in_a_suffix_is_refused() {
    let derived = r"landscape\rock01";
    for bad in [r"armor\plate_n", r"armor\plate_detail"] {
        let mut s = sdc_gui::pgp::settings::Settings::new();
        s.insert("rename".into(), json!(bad));
        assert!(
            sdc_gui::pgp::prefix::prefix_of(&s, derived).is_err(),
            "{bad}"
        );
    }
    // Through the model and the dialog too.
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    for bad in [r"armor\plate_n", r"armor\plate_detail"] {
        assert!(m.set_output_name(SET, Some(bad)).is_err(), "{bad}");
        open_outputs(&mut m, &[SET]);
        dlg(&mut m).typed.insert(SET.into(), bad.into());
        let p = plan(&m);
        assert!(row_of(&p, SET).problem.is_some(), "{bad}: {:?}", p.rows);
        assert!(!p.can_apply(), "{bad}");
        m.dialog = None;
    }
}

/// An empty folder name in a whole path is refused (final review 6).
#[test]
fn an_empty_folder_in_a_path_is_refused() {
    for bad in [r"a\\b", "a//b", r"armor\\steel\plate"] {
        let mut s = sdc_gui::pgp::settings::Settings::new();
        s.insert("rename".into(), json!(bad));
        assert!(
            sdc_gui::pgp::prefix::prefix_of(&s, r"landscape\rock01").is_err(),
            "{bad}"
        );
    }
}

/// HJ4 over a Case change: Whole path + UPPER CASE with the folder unchanged keeps the `Key` form (final review 3).
#[test]
fn a_case_change_over_the_whole_path_keeps_the_key_form() {
    use sdc_gui::model::rename::Case;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    open_outputs(&mut m, &[SET]);
    let r = dlg(&mut m);
    r.recipe.path = true;
    r.recipe.case = Case::Upper;
    assert!(row_of(&plan(&m), SET).writes());
    assert!(m.apply_rename());
    idle(&mut m);
    assert_eq!(settings(&m).get("rename"), Some(&json!("ROCK01")));
    let e = entry(&m, SET);
    assert!(
        e.get("match_diffuse").is_some() && e.get("slot1").is_none(),
        "{e}"
    );
}

/// A set whose stored `rename` is refused now shows why, and a new name repairs it (final review 4).
#[test]
fn a_refused_rename_can_be_repaired_in_the_dialog() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_json_value(SET, "rename", Some(json!(r"..\x")));
    idle(&mut m);
    assert!(set_views(&m)
        .iter()
        .any(|v| v.id == SET && v.prefix.is_none()));
    open_outputs(&mut m, &[SET]);
    let r = row_of(&plan(&m), SET);
    assert_eq!(r.skipped, None, "{r:?}");
    assert_eq!(r.old, "../x");
    assert!(
        r.warning.as_deref().is_some_and(|w| w.contains("refused")),
        "{r:?}"
    );
    dlg(&mut m).typed.insert(SET.into(), "steel".into());
    let r = row_of(&plan(&m), SET);
    assert_eq!(r.problem, None, "{r:?}");
    assert!(
        r.output.starts_with("textures/pbr/landscape/steel.dds"),
        "{r:?}"
    );
    assert!(m.apply_rename());
    idle(&mut m);
    assert_eq!(settings(&m).get("rename"), Some(&json!("steel")));
}

/// A typed path that resolves to the set's own prefix is no change (final review 5).
#[test]
fn a_path_to_the_same_prefix_is_no_change() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    open_outputs(&mut m, &[SET]);
    for same in ["pbr/landscape/rock01", r"textures\pbr\landscape\rock01"] {
        dlg(&mut m).typed.insert(SET.into(), same.into());
        let r = row_of(&plan(&m), SET);
        assert_eq!(r.new, "textures/pbr/landscape/rock01", "{same}");
        assert!(!r.writes(), "{same}: {r:?}");
    }
}

/// HJ3: in `Slots` form the entry names its files, so the AF8 "rename hits elsewhere" warning is silent.
#[test]
fn rename_hits_elsewhere_is_silent_in_slot_form() {
    use sdc_gui::pgp::prefix::rename_hits_elsewhere;
    let derived = r"landscape\rock01";
    let known = [r"textures\other\landscape\rock01.dds".to_string()];
    let mut s = sdc_gui::pgp::settings::Settings::new();
    s.insert("rename".into(), json!("steel"));
    assert!(
        rename_hits_elsewhere(&s, derived, &known).is_some(),
        "Key form warns"
    );
    s.insert("match2".into(), json!(r"landscape\rock01_n"));
    assert_eq!(rename_hits_elsewhere(&s, derived, &known), None);
}

/// `textures/armor/wall` (colour and `_n`) as a PBR set: its prefix is `textures\pbr\armor\wall`.
fn armor_wall(dir: &std::path::Path) -> (AppModel, String) {
    use sdc_gui::model::Answer;
    let r = dir.join("textures/armor");
    std::fs::create_dir_all(&r).unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
    })
    .save(r.join("wall.png"))
    .unwrap();
    image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb([128u8, 128, 255]))
        .save(r.join("wall_n.png"))
        .unwrap();
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    m.decide(
        "armor/wall.png",
        Answer::Subtype(engine::config::Subtype::PbrAlbedo),
    );
    idle(&mut m);
    let v = set_views(&m)
        .into_iter()
        .find(|v| v.members.iter().any(|k| k == "armor/wall.png"))
        .expect("wall's set");
    assert_eq!(v.prefix.as_deref(), Some(r"textures\pbr\armor\wall"));
    (m, v.id)
}

#[test]
fn whole_path_tick_works_in_outputs_mode() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = armor_wall(d.path());
    open_outputs(&mut m, &[&id]);
    let r = dlg(&mut m);
    r.recipe.path = true;
    r.recipe.regex = true;
    r.recipe.find = "^textures/pbr/armor/".into();
    r.recipe.replace = "armor/steel/".into();
    let row = row_of(&plan(&m), &id);
    assert_eq!(row.old, "textures/pbr/armor/wall");
    assert!(
        row.output.starts_with("textures/armor/steel/wall.dds"),
        "{row:?}"
    );
    assert_eq!(row.problem, None);
    let steps = m.history.undo.len();
    assert!(m.apply_rename());
    idle(&mut m);
    assert_eq!(m.history.undo.len(), steps + 1);
    let rename = |m: &AppModel| {
        set_views(m)
            .into_iter()
            .find(|v| v.id == id)
            .unwrap()
            .settings
            .get("rename")
            .cloned()
    };
    assert_eq!(rename(&m), Some(json!(r"textures\armor\steel\wall")));
    m.undo();
    idle(&mut m);
    assert_eq!(rename(&m), None);
}

/// Note FI2-style fixture: PBR sets `pbr/arm/steel` and `pbr/arm/iron` (colour, `_n`, `_rmaos`) and a plain colour map
/// `g/a` (mirrored, no path of its own).
fn arm(dir: &std::path::Path) -> AppModel {
    let t = dir.join("textures/pbr/arm");
    std::fs::create_dir_all(&t).unwrap();
    for (n, tint) in [("steel", 200u8), ("iron", 120)] {
        image::ImageBuffer::from_fn(64, 64, |x, y| {
            image::Rgb([(x * 4) as u8, (y * 4) as u8, tint])
        })
        .save(t.join(format!("{n}.png")))
        .unwrap();
        image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb([128u8, 128, 255]))
            .save(t.join(format!("{n}_n.png")))
            .unwrap();
        image::ImageBuffer::from_fn(64, 64, |x, _| image::Rgb([(x * 4) as u8, 30, 30]))
            .save(t.join(format!("{n}_rmaos.png")))
            .unwrap();
    }
    let g = dir.join("textures/g");
    std::fs::create_dir_all(&g).unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 9u8])
    })
    .save(g.join("a.png"))
    .unwrap();
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    idle(&mut m);
    m
}

const STEEL_SET: &str = "pbr/arm/steel";
const IRON_SET: &str = "pbr/arm/iron";

#[test]
fn a_whole_path_collision_blocks() {
    let d = tempfile::tempdir().unwrap();
    let mut m = arm(d.path());
    open_outputs(&mut m, &[STEEL_SET]);
    // Onto another set's output.
    dlg(&mut m)
        .typed
        .insert(STEEL_SET.into(), r"pbr\arm\iron".into());
    let r = row_of(&plan(&m), STEEL_SET);
    assert_eq!(
        r.problem.as_deref(),
        Some("pbr/arm/iron already writes textures/pbr/arm/iron.dds"),
        "{r:?}"
    );
    // Onto the mirrored output of a texture with no path of its own.
    dlg(&mut m).typed.insert(STEEL_SET.into(), r"g\a".into());
    let r = row_of(&plan(&m), STEEL_SET);
    assert!(
        r.problem
            .as_deref()
            .is_some_and(|p| p.contains("already writes textures/g/a.dds")),
        "{r:?}"
    );
    assert!(!plan(&m).can_apply());
}

/// A mapped vanilla set's record path is taken too (the `record_entries` source; final review 8).
#[test]
fn a_record_path_collision_blocks() {
    let d = tempfile::tempdir().unwrap();
    let mut m = arm(d.path());
    let g = set_views(&m)
        .into_iter()
        .find(|v| v.members.iter().any(|k| k == "g/a.png"))
        .expect("g/a's set")
        .id;
    m.map_record(
        &g,
        sdc_gui::sets_doc::TxstMap {
            plugin: "Skyrim.esm".into(),
            formid: "000800".into(),
            edid: "E".into(),
            slots: [(1, "textures/rec/body.dds".to_string())]
                .into_iter()
                .collect(),
            base: None,
        },
    )
    .unwrap();
    idle(&mut m);
    open_outputs(&mut m, &[STEEL_SET]);
    dlg(&mut m)
        .typed
        .insert(STEEL_SET.into(), r"rec\body".into());
    let r = row_of(&plan(&m), STEEL_SET);
    assert_eq!(
        r.problem,
        Some(format!("{g} already writes textures/rec/body.dds")),
        "{r:?}"
    );
}

#[test]
fn exporting_another_set_keeps_a_moved_sets_entry() {
    let d = tempfile::tempdir().unwrap();
    let mut m = arm(d.path());
    let key = |m: &AppModel| {
        set_views(m)
            .into_iter()
            .find(|v| v.id == STEEL_SET)
            .unwrap()
            .key
    };
    let before = key(&m);
    m.set_json_value(STEEL_SET, "rename", Some(json!(r"armor\plate")));
    idle(&mut m);
    assert_eq!(key(&m), before, "a rename never changes a set key");
    support::export_through(&mut m);
    idle(&mut m);
    assert!(m.export_root().join("textures/armor/plate.dds").is_file());
    let iron = set_views(&m)
        .into_iter()
        .find(|v| v.id == IRON_SET)
        .unwrap()
        .key;
    m.export_sets([iron].into(), "Export set pbr/arm/iron".into());
    if m.review.is_some() {
        m.export_from_review();
    }
    idle(&mut m);
    let j = std::fs::read_to_string(pgp_json_path(&m)).unwrap();
    assert!(
        j.contains(r#""slot1": "textures\\armor\\plate.dds""#),
        "{j}\n{}",
        m.export.notes.join("\n")
    );
    assert!(j.contains("iron"), "{j}");
}

#[test]
fn the_outputs_leave_the_pbr_folder_safely() {
    let d = tempfile::tempdir().unwrap();
    let mut m = arm(d.path());
    support::export_through(&mut m);
    idle(&mut m);
    m.set_json_value(STEEL_SET, "rename", Some(json!(r"armor\plate")));
    idle(&mut m);
    support::export_through(&mut m);
    idle(&mut m);
    let root = m.export_root();
    for f in ["plate.dds", "plate_n.dds", "plate_rmaos.dds"] {
        assert!(root.join("textures/armor").join(f).is_file(), "{f}");
    }
    assert!(!root.join("textures/textures").exists());
    assert!(!root.join("textures/pbr/textures").exists());
    let j = std::fs::read_to_string(pgp_json_path(&m)).unwrap();
    assert!(
        j.contains(r#""slot1": "textures\\armor\\plate.dds""#),
        "{j}"
    );
    let notes = m.export.notes.join("\n").to_lowercase();
    assert!(!notes.contains("textures/textures") && !notes.contains(r"textures\textures"));
    // The old outputs stay where they were, and no note names them as written (no export reports a moved DDS as an
    // orphan; the JSON names the new path, above).
    assert!(root.join("textures/pbr/arm/steel.dds").is_file());
    assert!(
        !notes.contains("pbr/arm/steel") && !notes.contains(r"pbr\arm\steel"),
        "{notes}"
    );
    let v = set_views(&m)
        .into_iter()
        .find(|v| v.id == STEEL_SET)
        .unwrap();
    let slots = sdc_gui::model::set_preview::member_slots(&m, &v, Some(&input(&m, STEEL_SET)));
    assert!(
        slots.iter().any(|s| s
            .paths
            .iter()
            .any(|p| p.replace('/', "\\").to_lowercase() == r"textures\armor\plate.dds")),
        "{:?}",
        slots.iter().map(|s| &s.paths).collect::<Vec<_>>()
    );
}

/// HJ10: set `b/stone` shares `a/rock`'s AO helper; the `_rmaos` that helper feeds is b's.
fn shared_ao(dir: &std::path::Path) -> AppModel {
    let root = dir.join("textures");
    for f in [
        "a/rock.png",
        "a/rock_ao.png",
        "b/stone.png",
        "b/stone_roughness.png",
    ] {
        let p = root.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        if f.contains('_') {
            image::ImageBuffer::from_fn(64, 64, |x, y| image::Luma([(x * 3 + y) as u8]))
                .save(p)
                .unwrap();
        } else {
            image::ImageBuffer::from_fn(64, 64, |x, y| {
                image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
            })
            .save(p)
            .unwrap();
        }
    }
    let mut m = opened_as_the_app(root, dir.join("export"));
    idle(&mut m);
    assert_eq!(
        m.drop_choice("a/rock_ao.png", "b/stone", DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    m
}

#[test]
fn a_shared_helpers_recipe_follows_the_sharing_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = shared_ao(d.path());
    let a_before: Vec<String> = engine_paths(&m)
        .into_iter()
        .filter(|p| p.contains("/rock"))
        .collect();
    m.set_json_value("b/stone", "rename", Some(json!(r"x\stone2")));
    idle(&mut m);
    let paths = engine_paths(&m);
    assert!(
        paths.contains(&"textures/x/stone2_rmaos.dds".to_string()),
        "{paths:?}"
    );
    assert_eq!(
        entry(&m, "b/stone")["slot6"],
        json!(r"textures\x\stone2_rmaos.dds")
    );
    let a_after: Vec<String> = paths.into_iter().filter(|p| p.contains("/rock")).collect();
    assert_eq!(a_after, a_before, "a's outputs stay");
    // Renaming a moves none of b's outputs.
    m.set_json_value("b/stone", "rename", None);
    m.set_json_value("a/rock", "rename", Some(json!(r"y\rock2")));
    idle(&mut m);
    let paths = engine_paths(&m);
    assert!(
        set_views(&m).iter().any(|v| v.id == "a/rock" && v.pbr),
        "a/rock is a PBR set"
    );
    assert!(
        paths.contains(&"textures/y/rock2.dds".to_string()),
        "a's outputs move: {paths:?}"
    );
    assert!(
        paths
            .iter()
            .any(|p| p.ends_with("/stone_rmaos.dds") && !p.contains("textures/y/")),
        "{paths:?}"
    );
    assert!(
        !paths.iter().any(|p| p.contains("textures/y/stone")),
        "{paths:?}"
    );
}

/// Review pictures of the Outputs dialog with Whole path ticked at 1100, 1280 and 1600 px (egui-ui-rules step 5).
/// Needs a GPU and `SDC_SHOTS_DIR`: `cargo test -p sdc-gui --test rename_slots -- --ignored shots`.
#[test]
#[ignore]
fn shots() {
    let Ok(out) = std::env::var("SDC_SHOTS_DIR") else {
        return;
    };
    for (w, hgt) in [(1100.0, 800.0), (1280.0, 800.0), (1600.0, 900.0)] {
        let d = tempfile::tempdir().unwrap();
        let mut m = arm(d.path());
        open_outputs(&mut m, &[STEEL_SET, IRON_SET]);
        let r = dlg(&mut m);
        r.recipe.path = true;
        r.recipe.find = "pbr/arm".into();
        r.recipe.replace = "armor/plate".into();
        dlg(&mut m).typed.insert(IRON_SET.into(), r"..\x".into());
        let mut h = Harness::builder()
            .with_size(eframe::egui::vec2(w, hgt))
            .with_pixels_per_point(1.0)
            .wgpu()
            .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run_steps(10);
        let img = h.render().expect("render");
        img.save(std::path::Path::new(&out).join(format!("rename-outputs-{w}.png")))
            .expect("save png");
    }
}

/// The JSON editor and the set editor at 1280 px on a renamed set with two matches (final review 13). Needs a GPU and
/// `SDC_SHOTS_DIR`: `cargo test -p sdc-gui --test rename_slots -- --ignored matching_shots`.
#[test]
#[ignore]
fn matching_shots() {
    let Ok(out) = std::env::var("SDC_SHOTS_DIR") else {
        return;
    };
    for json in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let mut m = renamed(d.path(), &[("match2", json!(r"landscape\rock01_n"))]);
        if json {
            m.open_json_editor();
        } else {
            let key = m
                .sets
                .iter()
                .find(|s| s.key.ends_with(SET))
                .unwrap()
                .key
                .clone();
            m.select_set(&key);
        }
        let mut h = Harness::builder()
            .with_size(eframe::egui::vec2(1280.0, 1200.0))
            .with_pixels_per_point(1.0)
            .wgpu()
            .build_ui_state(
                |ui, (m, s): &mut (AppModel, sdc_gui::ui::State)| sdc_gui::ui::draw_with(ui, m, s),
                (m, sdc_gui::ui::State::default()),
            );
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run_steps(10);
        if !json {
            if let Some(n) = h
                .query_all_by_label_contains("The entry applies when any of these matches")
                .next()
            {
                n.scroll_to_me();
            }
            h.run_steps(5);
        }
        let img = h.render().expect("render");
        let name = if json { "json-editor" } else { "set-editor" };
        img.save(std::path::Path::new(&out).join(format!("{name}-1280.png")))
            .expect("save png");
    }
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

/// At 1280 px the JSON editor's match2 pill stays inside the window (it wraps to a new row), and the set editor's
/// Matching path field is wide enough to read (final review 13).
#[test]
fn the_match_fields_fit_at_1280() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::NodeT as _;
    for json in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let mut m = renamed(d.path(), &[("match2", json!(r"landscape\rock01_n"))]);
        if json {
            m.open_json_editor();
        } else {
            let key = m
                .sets
                .iter()
                .find(|s| s.key.ends_with(SET))
                .unwrap()
                .key
                .clone();
            m.select_set(&key);
        }
        let mut h = Harness::builder()
            .with_size(eframe::egui::vec2(1280.0, 1200.0))
            .build_ui_state(
                |ui, (m, s): &mut (AppModel, sdc_gui::ui::State)| sdc_gui::ui::draw_with(ui, m, s),
                (m, sdc_gui::ui::State::default()),
            );
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run_steps(5);
        if !json {
            if let Some(n) = h
                .query_all_by_label_contains("The entry applies when any of these matches")
                .next()
            {
                n.scroll_to_me();
            }
            h.run_steps(5);
        }
        let fields: Vec<eframe::egui::Rect> = h
            .query_all_by_role(Role::TextInput)
            .filter(|n| {
                n.accesskit_node()
                    .value()
                    .is_some_and(|v| v.contains("rock01_n"))
            })
            .map(|n| n.rect())
            .collect();
        assert!(!fields.is_empty(), "json {json}: no match2 field");
        for r in fields {
            assert!(r.max.x <= 1280.0, "json {json}: {r:?} past the edge");
            if !json {
                assert!(r.width() >= 120.0, "set editor field {r:?} too narrow");
            }
        }
    }
}
