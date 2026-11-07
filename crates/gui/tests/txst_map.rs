//! Note HG: a set the user marked Advanced skin that holds only `_n`, `_sk` and `_wet` is a shared set: the maps other
//! skin sets use. The GUI shows it as shared, with only those maps, and writes no `.csskin.json` for it.
mod support;
use eframe::egui;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use image::{ImageBuffer, Rgb};
use sdc_gui::model::bottom::problems;
use sdc_gui::model::cs_skin_json::skin_json_plan;
use sdc_gui::model::pbr_export::json_entries;
use sdc_gui::model::sets::{
    kind_text, set_views, shared_cells, slot_row, DropChoice, DropOutcome, SetView, SlotKind,
};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use std::path::Path;
use std::time::Duration;
use support::session;

pub const X: &str = "textures/s/x";

fn colour(dir: &Path, rel: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(p)
        .unwrap();
}

/// A project at `dir` holding `files` (PNGs), `textures/s/x` marked Advanced skin in `sdc-project.toml`, exported to
/// `<dir>/export`. Tasks 4-6 extend this.
pub fn project(dir: &Path, files: &[&str]) -> AppModel {
    project_toml(
        dir,
        files,
        &format!("[[sets]]\nid = \"{X}\"\nshader = \"advanced_skin\"\n"),
    )
}

/// `project` with the whole text of `sdc-project.toml` given.
pub fn project_toml(dir: &Path, files: &[&str], toml: &str) -> AppModel {
    for f in files {
        colour(dir, f);
    }
    std::fs::write(dir.join("sdc-project.toml"), toml).unwrap();
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

/// Layout F of the engine's `shared_skin` tests: `x_normal`, `x_sk`, `x_wet` under `textures/s`.
pub fn shared(dir: &Path) -> AppModel {
    project(
        dir,
        &[
            "textures/s/x_normal.png",
            "textures/s/x_sk.png",
            "textures/s/x_wet.png",
        ],
    )
}

pub fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

pub fn view(m: &AppModel, id: &str) -> SetView {
    set_views(m)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("no set {id}"))
}

fn kinds(v: &SetView) -> Vec<SlotKind> {
    shared_cells(v, slot_row(v.shader.as_ref(), &v.slots))
        .iter()
        .map(|c| c.kind)
        .collect()
}

#[test]
fn a_shared_set_reads_shared() {
    let d = tempfile::tempdir().unwrap();
    let m = shared(d.path());
    let v = view(&m, X);
    assert!(v.shared, "{:?}", v.slots);
    assert!(!v.skin && !v.pbr);
    assert_eq!(kind_text(&m, &v), "shared");
    assert_eq!(
        sdc_gui::model::sets::badge_text(&m, &v),
        "shared skin maps (no JSON)"
    );
    assert!(json_entries(&m).is_empty());
    assert!(skin_json_plan(&m).0.is_empty());
}

#[test]
fn a_shared_set_has_no_skin_skip_row() {
    let d = tempfile::tempdir().unwrap();
    let m = shared(d.path());
    assert!(view(&m, X).shared);
    assert!(
        skin_json_plan(&m).1.iter().all(|(id, ..)| id != X),
        "{:?}",
        skin_json_plan(&m).1
    );
    let rows = problems(&m);
    assert!(
        rows.iter().all(|p| !p.text.contains("Advanced skin needs")),
        "{rows:?}"
    );
}

#[test]
fn a_shared_set_shows_only_its_maps() {
    let d = tempfile::tempdir().unwrap();
    let m = shared(d.path());
    let v = view(&m, X);
    let full = slot_row(v.shader.as_ref(), &v.slots);
    assert!(full.iter().any(|c| c.kind == SlotKind::Diffuse), "{full:?}");
    let mut got = kinds(&v);
    got.sort();
    let mut want = vec![
        SlotKind::Normal,
        SlotKind::SkinSk,
        SlotKind::SkinWet,
        SlotKind::SkinDetail,
    ];
    want.sort();
    assert_eq!(got, want);
}

#[test]
fn a_set_with_only_an_rfaos_is_not_shared() {
    let d = tempfile::tempdir().unwrap();
    let m = project(d.path(), &["textures/s/x_rfaos.png"]);
    let v = view(&m, X);
    assert!(!v.shared, "{:?}", v.slots);
    assert!(kinds(&v).contains(&SlotKind::Diffuse), "all its cards stay");
    assert!(
        skin_json_plan(&m).1.iter().any(|(id, _, why)| id == X
            && matches!(why, sdc_gui::model::cs_skin_json::SkinSkip::NoDiffuse)),
        "{:?}",
        skin_json_plan(&m).1
    );
}

#[test]
fn a_wet_member_of_a_shared_set_keeps_its_tail() {
    let d = tempfile::tempdir().unwrap();
    let m = shared(d.path());
    use sdc_gui::model::rename::{plan_rename, RenameDialog, RenameMode};
    let v = view(&m, X);
    let mut dlg = RenameDialog {
        mode: RenameMode::Inputs,
        keys: v.members.clone(),
        ..RenameDialog::default()
    };
    dlg.recipe.suffix = "2".into();
    let p = plan_rename(&m, &dlg);
    let r = p
        .rows
        .iter()
        .find(|r| r.key == "textures/s/x_wet.png")
        .unwrap_or_else(|| panic!("{:?}", p.rows));
    assert_eq!(r.new, "textures/s/x2_wet.png");
    assert_eq!(r.kind, Some(("_wet".to_string(), "_wet".to_string())));
    // Apply: the saved input mapping and the re-planned output keep the tail.
    let mut m = m;
    m.dialog = Some(sdc_gui::model::Dialog::Rename(dlg));
    assert!(m.apply_rename());
    idle(&mut m);
    assert_eq!(
        m.read_as_of("textures/s/x_wet.png").as_deref(),
        Some("textures/s/x2_wet.png")
    );
    let out = m
        .item("textures/s/x_wet.png")
        .and_then(|i| i.job.as_ref())
        .map(|j| j.output.to_string_lossy().replace('\\', "/"))
        .expect("the wet job");
    assert!(out.ends_with("x2_wet.dds"), "{out}");
}

#[test]
fn a_wet_can_be_dropped_on_a_shared_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(
        d.path(),
        &[
            "textures/s/x_normal.png",
            "textures/s/x_sk.png",
            "textures/s/rock.png",
            "textures/s/rock_rfaos.png",
            "textures/s/rock_wet.png",
        ],
    );
    assert!(view(&m, X).shared);
    assert!(view(&m, "textures/s/rock").skin);
    // The shared set has no `_wet` of its own, so the share lands in the empty slot.
    let out = m.drop_choice("textures/s/rock_wet.png", X, DropChoice::Share);
    assert!(matches!(out, DropOutcome::Joined), "{out:?}");
    idle(&mut m);
    assert!(view(&m, X).slots.contains_key(&SlotKind::SkinWet));
}

/// A shared set `x` (with its own `_wet`), a skin set `rock` and an ordinary set `plain`.
fn with_neighbours(dir: &Path) -> AppModel {
    project(
        dir,
        &[
            "textures/s/x_normal.png",
            "textures/s/x_sk.png",
            "textures/s/x_wet.png",
            "textures/s/rock.png",
            "textures/s/rock_rfaos.png",
            "textures/s/rock_wet.png",
            "textures/s/plain.png",
        ],
    )
}

#[test]
fn a_wet_is_still_refused_on_an_ordinary_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_neighbours(d.path());
    assert!(!view(&m, "textures/s/plain").skin && !view(&m, "textures/s/plain").shared);
    let out = m.drop_choice(
        "textures/s/rock_wet.png",
        "textures/s/plain",
        DropChoice::Share,
    );
    assert!(matches!(out, DropOutcome::Refused(_)), "{out:?}");
}

#[test]
fn a_wet_on_an_occupied_shared_slot_asks_to_replace() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_neighbours(d.path());
    assert!(view(&m, X).shared);
    let out = m.drop_choice("textures/s/rock_wet.png", X, DropChoice::Share);
    assert!(matches!(out, DropOutcome::AskReplace { .. }), "{out:?}");
}

#[test]
fn a_set_given_a_height_map_is_no_longer_shared_and_shows_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = shared(d.path());
    colour(d.path(), "textures/s/rock_p.png");
    m.replan();
    idle(&mut m);
    m.edit_docs("add", None, true, |_, s| {
        s.add_member(X, "textures/s/rock_p.png")
    });
    idle(&mut m);
    let v = view(&m, X);
    assert!(
        v.members.iter().any(|k| k.ends_with("rock_p.png")),
        "{:?}",
        v.members
    );
    assert!(!v.shared, "{:?}", v.slots);
    assert!(kinds(&v).contains(&SlotKind::Height), "{:?}", kinds(&v));
}

#[test]
fn the_statics_choice_is_off_on_a_shared_set() {
    let d = tempfile::tempdir().unwrap();
    statics_are_off(shared(d.path()));
}

/// A kept `x_wet.dds` (not re-encoded, so no job and no slot) leaves the set shared.
#[test]
fn a_kept_dds_member_leaves_a_set_shared() {
    let d = tempfile::tempdir().unwrap();
    support::bc1_dds(&d.path().join("textures/s/x_wet.dds"));
    let m = project(
        d.path(),
        &["textures/s/x_normal.png", "textures/s/x_sk.png"],
    );
    let v = view(&m, X);
    let wet = v
        .members
        .iter()
        .find(|k| k.ends_with("x_wet.dds"))
        .unwrap_or_else(|| panic!("{:?}", v.members));
    assert!(m.item(wet).is_some_and(|i| i.job.is_none()), "no job");
    assert!(v.shared, "{:?}", v.slots);
    assert_eq!(kind_text(&m, &v), "shared");
    let diffuse = shared_cells(&v, slot_row(v.shader.as_ref(), &v.slots))
        .iter()
        .filter(|c| matches!(c.kind, SlotKind::Diffuse | SlotKind::Rmaos))
        .count();
    assert_eq!(diffuse, 0);
    assert!(skin_json_plan(&m).1.iter().all(|(id, ..)| id != X));
    statics_are_off(m);
}

/// A kept `x_zz9.dds` named by a preset's `[suffix_aliases] zz9 = "n"` leaves the set shared: the engine groups with
/// the loaded aliases, so the GUI reads the member by them too.
#[test]
fn an_aliased_kept_dds_member_leaves_a_set_shared() {
    let d = tempfile::tempdir().unwrap();
    support::bc1_dds(&d.path().join("textures/s/x_zz9.dds"));
    let presets = d.path().join("user-presets");
    std::fs::create_dir_all(&presets).unwrap();
    std::fs::write(
        presets.join("mine.toml"),
        "schema_version = 1\nid = \"mine\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n\n\
         [suffix_aliases]\nzz9 = \"n\"\n",
    )
    .unwrap();
    let mut m = project(d.path(), &["textures/s/x_sk.png"]);
    m.user_presets = Some(presets);
    m.replan();
    idle(&mut m);
    let v = view(&m, X);
    let kept = v
        .members
        .iter()
        .find(|k| k.ends_with("x_zz9.dds"))
        .unwrap_or_else(|| panic!("{:?}", v.members));
    assert!(m.item(kept).is_some_and(|i| i.job.is_none()), "no job");
    assert!(v.shared, "{:?}", v.slots);
    assert!(skin_json_plan(&m).1.iter().all(|(id, ..)| id != X));
}

fn statics_are_off(mut m: AppModel) {
    use sdc_gui::model::cs_pbr::{Kind, Statics};
    m.add_cs_block(X, Kind::Txst);
    m.set_cs_statics(X, Statics::Always);
    let key = view(&m, X).key;
    m.select_set(&key);
    let mut h = harness(m, 1280.0);
    h.run();
    h.get_by_label("CS PBR records").click();
    h.run();
    for name in ["auto", "always", "never"] {
        let n = h
            .query_all_by_role_and_label(egui::accesskit::Role::Button, name)
            .next()
            .unwrap_or_else(|| panic!("no {name} button"));
        assert!(n.accesskit_node().is_disabled(), "{name}");
    }
    assert_eq!(
        h.query_all_by_label_contains("PGPatcher entry written")
            .count(),
        0
    );
    h.get_by_label(sdc_gui::ui::cs_pbr::SHARED_NO_PGP);
}

fn harness(m: AppModel, w: f32) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(egui::vec2(w, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

#[test]
fn the_editor_of_a_shared_set_has_rows_for_its_maps_only() {
    let d = tempfile::tempdir().unwrap();
    let mut m = shared(d.path());
    let key = view(&m, X).key;
    m.select_set(&key);
    let mut h = harness(m, 1100.0);
    h.run();
    for label in [
        "Normal _n",
        "Skin _sk",
        "Skin wet _wet",
        "Skin detail _detail_n",
    ] {
        support::slot_rect(&h, label);
    }
    assert_eq!(h.query_all_by_label("Diffuse").count(), 0);
    assert_eq!(h.query_all_by_label("RMAOS").count(), 0);
}

// ---- Task 4: a mapped set writes to its record's paths ----

use engine::sets::OutputKey;
use sdc_gui::model::pbr_export::path_collisions;
use sdc_gui::model::sets::engine_inputs;
use sdc_gui::sets_doc::TxstMap;

pub const M: &str = "textures/s/e/x";
const REC: &str = "[sets.txst]\nplugin = \"Skyrim.esm\"\nformid = \"000800\"\nedid = \"MyBody\"\n\
    slot1 = \"textures/s/e/femalebody.dds\"\nslot2 = \"textures/s/body_n.dds\"\n";
const M_FILES: &[&str] = &[
    "textures/s/e/x.png",
    "textures/s/e/x_n.png",
    "textures/s/e/x_rfaos.png",
    "textures/s/e/x_wet.png",
];

fn skin_set(id: &str, extra: &str) -> String {
    format!("[[sets]]\nid = \"{id}\"\nshader = \"advanced_skin\"\n{extra}")
}

fn mapped(dir: &Path, extra: &str) -> AppModel {
    project_toml(dir, M_FILES, &(skin_set(M, extra) + REC))
}

fn out(m: &AppModel, key: &str) -> String {
    m.item(key)
        .and_then(|i| i.job.as_ref())
        .map(|j| j.output.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| panic!("no job for {key}"))
}

fn record(slots: &[(u8, &str)]) -> TxstMap {
    TxstMap {
        plugin: "Skyrim.esm".into(),
        formid: "000800".into(),
        edid: "MyBody".into(),
        slots: slots.iter().map(|(n, p)| (*n, p.to_string())).collect(),
    }
}

fn entry(m: &AppModel, id: &str) -> sdc_gui::sets_doc::SetEntry {
    m.sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|e| engine::sets::same_id(&e.id, id))
        .cloned()
        .unwrap_or_else(|| panic!("no entry {id}"))
}

#[test]
fn a_file_writes_to_its_records_path() {
    let d = tempfile::tempdir().unwrap();
    let m = mapped(d.path(), "");
    assert!(out(&m, "textures/s/e/x.png").ends_with("/textures/s/e/femalebody.dds"));
    assert!(out(&m, "textures/s/e/x_n.png").ends_with("/textures/s/body_n.dds"));
    assert_eq!(
        engine_inputs(&m)
            .1
            .get(&OutputKey::File("textures/s/e/x.png".into())),
        Some(&std::path::PathBuf::from("textures/s/e/femalebody.dds"))
    );
}

#[test]
fn the_material_file_follows_the_diffuse() {
    let d = tempfile::tempdir().unwrap();
    let m = mapped(d.path(), "");
    let rows = skin_json_plan(&m).0;
    let row = rows.iter().find(|r| r.0 == M).expect("a plan row");
    let json = row.2.to_string_lossy().replace('\\', "/");
    assert!(
        json.ends_with("/textures/s/e/femalebody.csskin.json"),
        "{json}"
    );
}

#[test]
fn a_file_with_no_record_slot_keeps_its_path() {
    let d = tempfile::tempdir().unwrap();
    let m = mapped(d.path(), "");
    assert!(out(&m, "textures/s/e/x_rfaos.png").ends_with("/textures/s/e/x_rfaos.dds"));
    assert!(out(&m, "textures/s/e/x_wet.png").ends_with("/textures/s/e/x_wet.dds"));
    let d2 = tempfile::tempdir().unwrap();
    let plain = project_toml(d2.path(), M_FILES, &skin_set(M, ""));
    let texts =
        |m: &AppModel| -> Vec<String> { problems(m).iter().map(|p| p.text.clone()).collect() };
    assert_eq!(texts(&m), texts(&plain), "no new warning");
}

#[test]
fn a_packed_output_follows_slot_six() {
    let d = tempfile::tempdir().unwrap();
    let toml = "[[sets]]\nid = \"textures/s/q\"\nshader = \"advanced_skin\"\n[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"Q\"\n\
        slot6 = \"textures/s/z_rfaos.dds\"\n";
    let m = project_toml(
        d.path(),
        &["textures/s/q.png", "textures/s/q_roughness.png"],
        toml,
    );

    let packed = m
        .items
        .iter()
        .find(|i| i.kind == sdc_gui::model::items::ItemKind::Packed)
        .expect("a packed output");
    let o = packed
        .job
        .as_ref()
        .unwrap()
        .output
        .to_string_lossy()
        .replace('\\', "/");
    assert!(o.ends_with("/textures/s/z_rfaos.dds"), "{o}");
}

#[test]
fn the_record_beats_output_name() {
    let d = tempfile::tempdir().unwrap();
    // `output_name` belongs to the set table, so it goes before `[sets.txst]`.
    let toml = skin_set(M, "output_name = \"steel\"\n") + REC;
    let m = project_toml(d.path(), M_FILES, &toml);
    assert!(out(&m, "textures/s/e/x.png").ends_with("/textures/s/e/femalebody.dds"));
    assert!(out(&m, "textures/s/e/x_rfaos.png").ends_with("/textures/s/e/steel_rfaos.dds"));
}

#[test]
fn an_input_name_is_untouched() {
    use sdc_gui::model::rename::{plan_rename, RenameDialog, RenameMode};
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path(), M_FILES);
    let mut dlg = RenameDialog {
        mode: RenameMode::Inputs,
        keys: view(&m, M).members.clone(),
        ..RenameDialog::default()
    };
    dlg.recipe.suffix = "2".into();
    let _ = plan_rename(&m, &dlg);
    m.dialog = Some(sdc_gui::model::Dialog::Rename(dlg));
    assert!(m.apply_rename());
    idle(&mut m);
    let key = "textures/s/e/x_rfaos.png";
    let alias = m.read_as_of(key);
    assert_eq!(alias.as_deref(), Some("textures/s/e/x2_rfaos.png"));
    let id = set_views(&m)
        .into_iter()
        .find(|v| v.members.iter().any(|k| k == key))
        .unwrap()
        .id;
    m.map_record(&id, record(&[(1, "textures/s/e/femalebody.dds")]))
        .unwrap();
    idle(&mut m);
    assert_eq!(m.read_as_of(key), alias);
    assert!(out(&m, "textures/s/e/x.png").ends_with("femalebody.dds"));
}

#[test]
fn two_sets_on_one_record_path_are_a_collision() {
    let d = tempfile::tempdir().unwrap();
    let rec = "[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"E\"\nslot1 = \"textures/s/same.dds\"\n";
    let toml = format!(
        "{}{rec}{}{rec}",
        skin_set("textures/s/a", ""),
        skin_set("textures/s/b", "")
    );
    let m = project_toml(d.path(), &["textures/s/a.png", "textures/s/b.png"], &toml);
    let rows = path_collisions(&m);
    assert!(rows.iter().any(|r| r.text.contains("same.dds")), "{rows:?}");
    let n = engine_inputs(&m)
        .1
        .values()
        .filter(|p| p.ends_with("same.dds"))
        .count();
    assert_eq!(n, 1, "the engine sees one of them");
}

#[test]
fn a_pbr_set_ignores_its_mapping() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened(d.path());
    let before = engine_inputs(&m).1;
    let out_before = out(&m, "landscape/rock01.png");
    let map = record(&[(1, "textures/landscape/zzz.dds")]);
    m.edit_docs("map", None, true, |_, s| {
        s.set_txst("landscape/rock01", Some(&map))
    });
    idle(&mut m);
    assert!(entry(&m, "landscape/rock01").txst.is_some());
    assert_eq!(engine_inputs(&m).1, before);
    assert_eq!(out(&m, "landscape/rock01.png"), out_before);
}

#[test]
fn a_bad_record_path_is_never_written() {
    let d = tempfile::tempdir().unwrap();
    let toml = skin_set(M, "") + "[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"E\"\nslot2 = \"meshes/x.nif\"\n";
    let m = project_toml(d.path(), M_FILES, &toml);
    assert!(out(&m, "textures/s/e/x_n.png").ends_with("/textures/s/e/x_n.dds"));
}

/// Pin: the mapping is read from the project file, not from a scan.
#[test]
fn a_mapping_survives_an_empty_scan() {
    let d = tempfile::tempdir().unwrap();
    let m = mapped(d.path(), "");
    assert!(out(&m, "textures/s/e/x.png").ends_with("femalebody.dds"));
}

#[test]
fn undoing_a_mapping_restores_the_names() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path(), M_FILES);
    assert!(out(&m, "textures/s/e/x.png").ends_with("/textures/s/e/x.dds"));
    m.map_record(M, record(&[(1, "textures/s/e/femalebody.dds")]))
        .unwrap();
    idle(&mut m);
    assert!(out(&m, "textures/s/e/x.png").ends_with("femalebody.dds"));
    m.undo();
    idle(&mut m);
    assert!(out(&m, "textures/s/e/x.png").ends_with("/textures/s/e/x.dds"));
    assert!(entry_opt(&m, M).is_none_or(|e| e.txst.is_none()));
    m.redo();
    idle(&mut m);
    m.unmap_record(M).unwrap();
    idle(&mut m);
    assert!(out(&m, "textures/s/e/x.png").ends_with("/textures/s/e/x.dds"));
}

fn entry_opt(m: &AppModel, id: &str) -> Option<sdc_gui::sets_doc::SetEntry> {
    m.sets_doc
        .as_ref()?
        .entries()
        .iter()
        .find(|e| engine::sets::same_id(&e.id, id))
        .cloned()
}

#[test]
fn a_renamed_set_keeps_its_mapping() {
    let d = tempfile::tempdir().unwrap();
    let mut m = mapped(d.path(), "");
    m.rename_set(M, "textures/s/e/renamed").unwrap();
    idle(&mut m);
    let e = entry(&m, "textures/s/e/renamed");
    assert_eq!(e.txst.unwrap().edid, "MyBody");
}

#[test]
fn a_mapping_round_trips_the_file() {
    let d = tempfile::tempdir().unwrap();
    let mut s = sdc_gui::sets_doc::SetsDoc::open(d.path());
    let map = record(&[(1, "textures/a/b.dds"), (6, "textures/a/b_rmaos.dds")]);
    s.set_txst("a/x", Some(&map));
    s.save(false).unwrap();
    let text = std::fs::read_to_string(d.path().join("sdc-project.toml")).unwrap();
    let mut s = sdc_gui::sets_doc::SetsDoc::open(d.path());
    assert_eq!(s.entries()[0].txst.as_ref(), Some(&map));
    s.set_txst("a/x", Some(&map));
    s.save(false).unwrap();
    assert_eq!(
        std::fs::read_to_string(d.path().join("sdc-project.toml")).unwrap(),
        text
    );
}

#[test]
fn no_fill_round_trips() {
    let d = tempfile::tempdir().unwrap();
    let mut s = sdc_gui::sets_doc::SetsDoc::open(d.path());
    s.set_no_fill("a/x", 2, true);
    s.save(false).unwrap();
    let mut s = sdc_gui::sets_doc::SetsDoc::open(d.path());
    assert_eq!(
        s.entries()[0].no_fill.iter().copied().collect::<Vec<_>>(),
        [2]
    );
    s.set_no_fill("a/x", 2, false);
    s.save(false).unwrap();
    let text = std::fs::read_to_string(d.path().join("sdc-project.toml")).unwrap();
    assert!(!text.contains("no_fill"), "{text}");
}
/// `TXST_SHOTS_DIR=<dir> cargo test -p sdc-gui --test txst_map -- --ignored render_`: the card and the editor of the
/// shared set at 1100, 1280 and 1600 px wide.
#[test]
#[ignore]
fn render_shared_set_shots() {
    let out = std::path::PathBuf::from(std::env::var_os("TXST_SHOTS_DIR").expect("TXST_SHOTS_DIR"));
    std::fs::create_dir_all(&out).unwrap();
    for w in [1100.0f32, 1280.0, 1600.0] {
        let d = tempfile::tempdir().unwrap();
        let mut m = shared(d.path());
        let key = view(&m, X).key;
        m.select_set(&key);
        let mut h = Harness::builder()
            .with_size(egui::vec2(w, 700.0))
            .renderer(egui_kittest::wgpu::WgpuTestRenderer::new())
            .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        for _ in 0..8 {
            h.run_steps(1);
            std::thread::sleep(Duration::from_millis(40));
        }
        h.render()
            .unwrap()
            .save(out.join(format!("shared-{w}.png")))
            .unwrap();
    }
}
