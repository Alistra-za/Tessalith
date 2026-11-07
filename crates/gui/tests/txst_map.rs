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
use sdc_gui::model::pbr_export::{export_blocks, path_collisions};
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
    assert!(
        export_blocks(&m)
            .iter()
            .any(|r| r.text.contains("same.dds")),
        "export is blocked"
    );
    let n = engine_inputs(&m)
        .1
        .values()
        .filter(|p| p.ends_with("same.dds"))
        .count();
    assert_eq!(n, 1, "the engine sees one of them");
}

#[test]
fn a_replaced_output_name_reserves_no_path() {
    let d = tempfile::tempdir().unwrap();
    let rec = |p: &str| {
        format!("[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"E\"\nslot1 = \"{p}\"\n")
    };
    let toml = format!(
        "{}{}{}{}",
        skin_set("textures/s/a", "output_name = \"steel\"\n"),
        rec("textures/s/body.dds"),
        skin_set("textures/s/b", ""),
        rec("textures/s/steel.dds")
    );
    let m = project_toml(d.path(), &["textures/s/a.png", "textures/s/b.png"], &toml);
    assert!(out(&m, "textures/s/a.png").ends_with("/textures/s/body.dds"));
    assert!(out(&m, "textures/s/b.png").ends_with("/textures/s/steel.dds"));
    assert!(path_collisions(&m).is_empty(), "{:?}", path_collisions(&m));
}

#[test]
fn a_spelling_variant_of_a_record_path_collides() {
    let d = tempfile::tempdir().unwrap();
    let rec = |p: &str| {
        format!("[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"E\"\nslot1 = \"{p}\"\n")
    };
    let toml = format!(
        "{}{}{}{}",
        skin_set("textures/s/a", ""),
        rec("textures/s/same.dds"),
        skin_set("textures/s/b", ""),
        rec("textures/s/./same.dds")
    );
    let m = project_toml(d.path(), &["textures/s/a.png", "textures/s/b.png"], &toml);
    assert!(!path_collisions(&m).is_empty());
    let toml = toml.replace("textures/s/./same.dds", "textures/s//same.dds");
    let d = tempfile::tempdir().unwrap();
    let m = project_toml(d.path(), &["textures/s/a.png", "textures/s/b.png"], &toml);
    assert!(!path_collisions(&m).is_empty());
}

#[test]
fn a_vanilla_collision_is_reported_and_stable() {
    let d = tempfile::tempdir().unwrap();
    let rec = "[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"E\"\nslot1 = \"textures/s/same.dds\"\n";
    let toml =
        format!("[[sets]]\nid = \"textures/s/a\"\n{rec}[[sets]]\nid = \"textures/s/b\"\n{rec}");
    let mut m = project_toml(d.path(), &["textures/s/a.png", "textures/s/b.png"], &toml);
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    assert!(!path_collisions(&m).is_empty());
    assert!(export_blocks(&m)
        .iter()
        .any(|r| r.text.contains("same.dds")));
    let before = engine_inputs(&m);
    m.replan();
    idle(&mut m);
    assert_eq!(engine_inputs(&m), before, "planning is stable");
    for k in ["textures/s/a.png", "textures/s/b.png"] {
        assert!(
            m.item(k).is_some_and(|i| i.job.is_some()),
            "{k} keeps its job"
        );
    }
    assert_eq!(
        before
            .1
            .values()
            .filter(|p| p.ends_with("same.dds"))
            .count(),
        1
    );
}

/// An `output_name` takes its folder from the file's default output, not from where a removed mapping put it.
#[test]
fn unmapping_restores_the_named_folder() {
    let d = tempfile::tempdir().unwrap();
    let id = "textures/a/x";
    let rec = "[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"E\"\nslot1 = \"textures/b/body.dds\"\n";
    let toml = skin_set(id, "output_name = \"steel\"\n") + rec;
    let mut m = project_toml(d.path(), &["textures/a/x.png"], &toml);
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/b/body.dds"));
    m.unmap_record(id).unwrap();
    idle(&mut m);
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/a/steel.dds"));
    // Through undo, too.
    m.undo();
    idle(&mut m);
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/b/body.dds"));
    m.unmap_record(id).unwrap();
    idle(&mut m);
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/a/steel.dds"));
    m.map_record(id, record(&[(1, "textures/b/body.dds")]))
        .unwrap();
    idle(&mut m);
    m.undo();
    idle(&mut m);
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/a/steel.dds"));
}

/// A packed output takes its named folder from its first source, not from where a removed mapping put it.
#[test]
fn unmapping_a_packed_output_restores_its_named_folder() {
    let d = tempfile::tempdir().unwrap();
    let id = "textures/a/q";
    let rec = "[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"E\"\nslot6 = \"textures/b/z_rfaos.dds\"\n";
    let toml = skin_set(id, "output_name = \"steel\"\n") + rec;
    let mut m = project_toml(
        d.path(),
        &["textures/a/q.png", "textures/a/q_roughness.png"],
        &toml,
    );
    let packed = |m: &AppModel| {
        let i = m
            .items
            .iter()
            .find(|i| i.kind == sdc_gui::model::items::ItemKind::Packed)
            .expect("a packed output");
        i.job
            .as_ref()
            .unwrap()
            .output
            .to_string_lossy()
            .replace('\\', "/")
    };
    assert!(
        packed(&m).ends_with("/textures/b/z_rfaos.dds"),
        "{}",
        packed(&m)
    );
    m.unmap_record(id).unwrap();
    idle(&mut m);
    assert!(
        packed(&m).ends_with("/textures/a/steel_rfaos.dds"),
        "{}",
        packed(&m)
    );
    m.undo();
    idle(&mut m);
    assert!(
        packed(&m).ends_with("/textures/b/z_rfaos.dds"),
        "{}",
        packed(&m)
    );
    m.unmap_record(id).unwrap();
    idle(&mut m);
    m.map_record(id, record(&[(6, "textures/b/z_rfaos.dds")]))
        .unwrap();
    idle(&mut m);
    m.undo();
    idle(&mut m);
    assert!(
        packed(&m).ends_with("/textures/a/steel_rfaos.dds"),
        "{}",
        packed(&m)
    );
}

/// An input renamed into another folder moves the named output with it, even when a mapping held it elsewhere.
#[test]
fn unmapping_after_an_input_rename_uses_the_new_folder() {
    use sdc_gui::model::rename::{RenameDialog, RenameMode};
    let d = tempfile::tempdir().unwrap();
    let toml = skin_set("textures/a/x", "output_name = \"steel\"\n");
    let mut m = project_toml(d.path(), &["textures/a/x.png"], &toml);
    let mut dlg = RenameDialog {
        mode: RenameMode::Inputs,
        keys: vec!["textures/a/x.png".into()],
        ..RenameDialog::default()
    };
    dlg.typed
        .insert("textures/a/x.png".into(), "textures/c/x.png".into());
    m.dialog = Some(sdc_gui::model::Dialog::Rename(dlg));
    assert!(m.apply_rename());
    idle(&mut m);
    assert_eq!(
        m.read_as_of("textures/a/x.png").as_deref(),
        Some("textures/c/x.png")
    );
    let id = set_views(&m)
        .into_iter()
        .find(|v| v.members.iter().any(|k| k == "textures/a/x.png"))
        .unwrap()
        .id;
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/c/steel.dds"));
    m.map_record(&id, record(&[(1, "textures/b/body.dds")]))
        .unwrap();
    idle(&mut m);
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/b/body.dds"));
    m.unmap_record(&id).unwrap();
    idle(&mut m);
    assert!(
        out(&m, "textures/a/x.png").ends_with("/textures/c/steel.dds"),
        "{}",
        out(&m, "textures/a/x.png")
    );
}

/// The engine refuses an alias onto another file's name (IN-ALIAS-INVALID); the file keeps its identity, so does its
/// named output.
#[test]
fn a_refused_alias_leaves_a_named_output_in_its_folder() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("sdc.toml"),
        "[inputs.\"textures/a/x.png\"]\nas = \"textures/c/x.png\"\n",
    )
    .unwrap();
    let toml = skin_set("textures/a/x", "output_name = \"steel\"\n");
    let m = project_toml(d.path(), &["textures/a/x.png", "textures/c/x.png"], &toml);
    let o = out(&m, "textures/a/x.png");
    assert!(o.ends_with("/textures/a/steel.dds"), "{o}");
}

/// A created set's id does not follow an input alias; its named output still does.
#[test]
fn unmapping_a_created_set_after_a_rename_uses_the_new_folder() {
    use sdc_gui::model::rename::{RenameDialog, RenameMode};
    let d = tempfile::tempdir().unwrap();
    let mut m = project_toml(d.path(), &["textures/a/x.png"], "");
    let id = m.drop_new_set("textures/a/x.png").unwrap();
    idle(&mut m);
    m.set_output_name(&id, Some("steel")).unwrap();
    idle(&mut m);
    m.map_record(&id, record(&[(1, "textures/b/body.dds")]))
        .unwrap();
    idle(&mut m);
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/b/body.dds"));
    let mut dlg = RenameDialog {
        mode: RenameMode::Inputs,
        keys: vec!["textures/a/x.png".into()],
        ..RenameDialog::default()
    };
    dlg.typed
        .insert("textures/a/x.png".into(), "textures/c/x.png".into());
    m.dialog = Some(sdc_gui::model::Dialog::Rename(dlg));
    assert!(m.apply_rename());
    idle(&mut m);
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/b/body.dds"));
    m.unmap_record(&id).unwrap();
    idle(&mut m);
    let o = out(&m, "textures/a/x.png");
    assert!(o.ends_with("/textures/c/steel.dds"), "{o}");
}
/// A project opened with a mapping and an output name, then unmapped.
#[test]
fn a_reopened_mapped_and_named_set_unmaps_to_its_named_path() {
    let d = tempfile::tempdir().unwrap();
    let id = "textures/a/x";
    let rec = "[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"E\"\nslot1 = \"textures/b/body.dds\"\n";
    let toml = skin_set(id, "output_name = \"steel\"\n") + rec;
    drop(project_toml(d.path(), &["textures/a/x.png"], &toml));
    let mut m = project_toml(d.path(), &["textures/a/x.png"], &toml);
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/b/body.dds"));
    m.unmap_record(id).unwrap();
    idle(&mut m);
    assert!(out(&m, "textures/a/x.png").ends_with("/textures/a/steel.dds"));
}

/// The derivation reproduces the engine's own default output for files and packed outputs it is not moved from.
#[test]
fn the_default_output_matches_the_plan_when_nothing_moves_it() {
    let d = tempfile::tempdir().unwrap();
    let m = mapped_free(d.path());
    let defaults = sdc_gui::model::sets::DefaultOutputs::new(&m).unwrap();
    let mut n = 0;
    for it in m
        .items
        .iter()
        .filter(|i| i.job.is_some() && !i.job_key.is_empty())
    {
        assert_eq!(
            defaults.key(it).as_deref(),
            Some(it.job_key.as_str()),
            "{}",
            it.key
        );
        n += 1;
    }
    assert!(n >= 4, "{n} items compared");
}

fn mapped_free(dir: &Path) -> AppModel {
    let mut files = M_FILES.to_vec();
    files.extend(["textures/s/q.png", "textures/s/q_roughness.png"]);
    let toml = skin_set(M, "") + &skin_set("textures/s/q", "");
    project_toml(dir, &files, &toml)
}
/// A set of raw packing maps with no diffuse builds `_rmaos`; slot 6 of its record moves it.
#[test]
fn a_packed_rmaos_follows_slot_six() {
    let d = tempfile::tempdir().unwrap();
    let toml = "[[sets]]\nid = \"textures/s/q\"\n[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"Q\"\n\
        slot6 = \"textures/s/z_rmaos.dds\"\n";
    let m = project_toml(
        d.path(),
        &["textures/s/q_roughness.png", "textures/s/q_ao.png"],
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
    assert!(o.ends_with("/textures/s/z_rmaos.dds"), "{o}");
    assert!(!view(&m, "textures/s/q").pbr);
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

// ---- Task 5: record slots fill from the project (Note HG) ----

use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::sets::SlotFill;

const R_FILES: &[&str] = &[
    "textures/s/x_normal.png",
    "textures/s/x_sk.png",
    "textures/s/x_wet.png",
    "textures/s/e/x.png",
    "textures/s/e/x_rfaos.png",
];
const OTHER: &str = "textures/s/o";
const BODY_N: &str = "textures/s/body_n.dds";

fn txst(slots: &[(u8, &str)]) -> String {
    let mut s = "[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"E\"\n".to_string();
    for (n, p) in slots {
        s += &format!("slot{n} = \"{p}\"\n");
    }
    s
}

/// Fixture R: the shared-map set `H` (`X`) and the skin set `C` (`M`), both mapped to the same body record, `C` with
/// slot 8 as well. `h` and `c` are extra lines for each set's table.
fn r_toml(h: &str, c: &str) -> String {
    let h_rec = txst(&[
        (1, "textures/s/e/femalebody.dds"),
        (2, BODY_N),
        (3, "textures/s/body_sk.dds"),
    ]);
    let c_rec = txst(&[
        (1, "textures/s/e/femalebody.dds"),
        (2, BODY_N),
        (3, "textures/s/body_sk.dds"),
        (8, "textures/s/e/body_s.dds"),
    ]);
    format!(
        "[[sets]]\nid = \"{X}\"\n{h}{h_rec}[[sets]]\nid = \"{M}\"\nshader = \"advanced_skin\"\n{c}{c_rec}"
    )
}

fn r_with(dir: &Path, files: &[&str], h: &str, c: &str) -> AppModel {
    project_toml(dir, files, &r_toml(h, c))
}

fn r(dir: &Path) -> AppModel {
    r_with(dir, R_FILES, "", "")
}

fn borrowed_from(v: &SetView, k: SlotKind) -> Option<String> {
    match v.slots.get(&k) {
        Some(SlotFill::Borrowed { from, .. }) => Some(from.clone()),
        _ => None,
    }
}

#[test]
fn a_record_slot_another_set_writes_is_borrowed() {
    let d = tempfile::tempdir().unwrap();
    let m = r(d.path());
    let c = view(&m, M);
    assert_eq!(borrowed_from(&c, SlotKind::Normal).as_deref(), Some(X));
    assert_eq!(borrowed_from(&c, SlotKind::SkinSk).as_deref(), Some(X));
    assert!(c.filled.contains_key(&2) && c.filled.contains_key(&3));
}

#[test]
fn a_fill_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = r(d.path());
    let before = std::fs::read(d.path().join("sdc-project.toml")).unwrap();
    assert!(!view(&m, M).filled.is_empty());
    m.save().unwrap();
    assert_eq!(
        std::fs::read(d.path().join("sdc-project.toml")).unwrap(),
        before
    );
    assert_eq!(before, r_toml("", "").as_bytes());
}

#[test]
fn one_normal_is_written() {
    let d = tempfile::tempdir().unwrap();
    let m = r(d.path());
    let paths = engine_inputs(&m).1;
    let n = paths
        .values()
        .filter(|p| p.to_string_lossy().replace('\\', "/") == BODY_N)
        .count();
    assert_eq!(n, 1, "{paths:?}");
    // C's rfaos bakes from the borrowed normal, so the engine is told about the borrow.
    assert_eq!(
        sdc_gui::model::sets::borrowed_normal(&m)
            .get(M)
            .map(String::as_str),
        Some(X)
    );
    let mut outs = m.items.iter().filter_map(|i| i.job.as_ref()).filter(|j| {
        j.output
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("/textures/s/body_n.dds")
    });
    assert!(outs.next().is_some() && outs.next().is_none());
}

#[test]
fn a_record_slot_nothing_writes_is_supplied() {
    let d = tempfile::tempdir().unwrap();
    let m = r(d.path());
    let c = view(&m, M);
    assert_eq!(
        c.record_supplied.get(&8).map(String::as_str),
        Some("textures/s/e/body_s.dds")
    );
    assert!(!c.filled.contains_key(&8));
    assert!(c.slots.keys().all(|k| k.slot_number() != Some(8)));
}

/// A vanilla set with no `_n` and a record naming a slot-2 path: the missing-map warning goes with no scan.
#[test]
fn a_supplied_slot_clears_its_missing_map() {
    let d = tempfile::tempdir().unwrap();
    let plain = project_toml(
        d.path(),
        &["textures/v/a.png"],
        "[[sets]]\nid = \"textures/v/a\"\nshader = \"pbr_landscape\"\n",
    );
    let warned = |m: &AppModel| {
        view(m, "textures/v/a")
            .set_warnings
            .iter()
            .any(|w| w.code == engine::codes::Code::ShaderMissingMap && w.data["map"] == "_n")
    };
    assert!(
        warned(&plain),
        "{:?}",
        view(&plain, "textures/v/a").set_warnings
    );
    let d2 = tempfile::tempdir().unwrap();
    let toml = format!(
        "[[sets]]\nid = \"textures/v/a\"\nshader = \"pbr_landscape\"\n{}",
        txst(&[(2, "textures/black.dds")])
    );
    let m = project_toml(d2.path(), &["textures/v/a.png"], &toml);
    let v = view(&m, "textures/v/a");
    assert!(v.record_supplied.contains_key(&2));
    assert!(!warned(&m), "{:?}", v.set_warnings);
}

#[test]
fn colour_is_never_borrowed() {
    let d = tempfile::tempdir().unwrap();
    let m = r(d.path());
    let h = view(&m, X);
    assert!(!h.slots.contains_key(&SlotKind::Diffuse));
    assert!(!h.filled.contains_key(&1) && !h.record_supplied.contains_key(&1));
    assert!(h.shared);
}

#[test]
fn a_shared_map_set_of_a_skin_set_is_skin() {
    let d = tempfile::tempdir().unwrap();
    let m = r(d.path());
    let h = view(&m, X);
    assert!(h.shared, "{:?}", h.slots);
    let job = |k: &str| {
        m.item(k)
            .and_then(|i| i.job.as_ref())
            .map(|j| j.profile.subtype)
    };
    assert_eq!(
        job("textures/s/x_normal.png"),
        Some(engine::config::Subtype::PbrNormal)
    );
    assert!(h.slots.contains_key(&SlotKind::SkinWet));
    // R7 is a request to the engine, not a line in the project file.
    assert_eq!(
        sdc_gui::model::sets::set_shaders(&m).get(X),
        Some(&engine::shader::Shader::AdvancedSkin)
    );
}

#[test]
fn a_chosen_shader_on_it_wins() {
    let d = tempfile::tempdir().unwrap();
    let m = r_with(d.path(), R_FILES, "shader = \"skin\"\n", "");
    assert!(!view(&m, X).shared);
    assert_eq!(
        sdc_gui::model::sets::set_shaders(&m).get(X),
        Some(&engine::shader::Shader::Skin)
    );
}

#[test]
fn wet_comes_with_the_normal() {
    let d = tempfile::tempdir().unwrap();
    let m = r(d.path());
    let c = view(&m, M);
    assert_eq!(borrowed_from(&c, SlotKind::SkinWet).as_deref(), Some(X));
    assert!(c.filled.contains_key(&9));
    let rows = skin_json_plan(&m).0;
    let row = rows.iter().find(|r| r.0 == M).expect("a plan row");
    assert_eq!(row.3.wet.as_deref(), Some("textures/s/x_wet.dds"));
}

#[test]
fn its_own_wet_wins() {
    let d = tempfile::tempdir().unwrap();
    let mut files = R_FILES.to_vec();
    files.push("textures/s/e/x_wet.png");
    let m = r_with(d.path(), &files, "", "");
    let c = view(&m, M);
    assert!(matches!(
        c.slots.get(&SlotKind::SkinWet),
        Some(SlotFill::Own { .. })
    ));
    assert!(!c.filled.contains_key(&9));
}

#[test]
fn an_explicit_borrow_of_wet_wins() {
    let d = tempfile::tempdir().unwrap();
    let mut files = R_FILES.to_vec();
    files.extend([
        "textures/s/o.png",
        "textures/s/o_rfaos.png",
        "textures/s/o_wet.png",
    ]);
    let toml = r_toml("", &format!("borrow = {{ slot9 = \"{OTHER}\" }}\n"))
        + &format!("[[sets]]\nid = \"{OTHER}\"\nshader = \"advanced_skin\"\n");
    let m = project_toml(d.path(), &files, &toml);
    let c = view(&m, M);
    assert_eq!(borrowed_from(&c, SlotKind::SkinWet).as_deref(), Some(OTHER));
    assert!(!c.filled.contains_key(&9));
}

#[test]
fn a_host_with_no_wet_fills_nothing() {
    let d = tempfile::tempdir().unwrap();
    let m = r_with(
        d.path(),
        &[
            "textures/s/x_normal.png",
            "textures/s/x_sk.png",
            "textures/s/e/x.png",
            "textures/s/e/x_rfaos.png",
        ],
        "",
        "",
    );
    let c = view(&m, M);
    assert!(!c.slots.contains_key(&SlotKind::SkinWet) && !c.filled.contains_key(&9));
    assert!(c.missing.is_empty());
}

#[test]
fn an_explicit_borrow_wins() {
    let d = tempfile::tempdir().unwrap();
    let mut files = R_FILES.to_vec();
    files.extend(["textures/s/o.png", "textures/s/o_n.png"]);
    let m = r_with(
        d.path(),
        &files,
        "",
        &format!("borrow = {{ slot2 = \"{OTHER}\" }}\n"),
    );
    let c = view(&m, M);
    assert_eq!(borrowed_from(&c, SlotKind::Normal).as_deref(), Some(OTHER));
    assert!(!c.filled.contains_key(&2));
}

#[test]
fn a_pinned_empty_slot_stays_empty() {
    let d = tempfile::tempdir().unwrap();
    let m = r_with(d.path(), R_FILES, "", "lock = [\"slot2\"]\n");
    let c = view(&m, M);
    assert!(!c.slots.contains_key(&SlotKind::Normal) && !c.filled.contains_key(&2));
    assert!(c.filled.contains_key(&3), "the other slot still fills");
}

#[test]
fn its_own_file_wins() {
    let d = tempfile::tempdir().unwrap();
    let mut files = R_FILES.to_vec();
    files.push("textures/s/e/x_n.png");
    let m = r_with(d.path(), &files, "", "");
    let c = view(&m, M);
    assert!(matches!(
        c.slots.get(&SlotKind::Normal),
        Some(SlotFill::Own { .. })
    ));
    assert!(!c.filled.contains_key(&2));
    // H also writes body_n: the existing duplicate row.
    assert!(
        path_collisions(&m)
            .iter()
            .any(|c| format!("{c:?}").contains("body_n.dds")),
        "{:?}",
        path_collisions(&m)
    );
}

#[test]
fn clear_turns_a_fill_off() {
    let d = tempfile::tempdir().unwrap();
    let mut m = r(d.path());
    m.set_slot_command(M, 2, sdc_gui::model::sets::SlotCommand::Clear)
        .unwrap();
    idle(&mut m);
    assert!(entry(&m, M).no_fill.contains(&2));
    assert!(!entry(&m, M).settings.contains_key("slot2"));
    let c = view(&m, M);
    assert!(!c.slots.contains_key(&SlotKind::Normal) && !c.filled.contains_key(&2));
    assert_eq!(c.fill_off.get(&2).map(String::as_str), Some(X));
    m.save().unwrap();
    let text = std::fs::read_to_string(d.path().join("sdc-project.toml")).unwrap();
    assert!(
        text.contains("no_fill") && !text.contains("slot2 = \"\""),
        "{text}"
    );
}

#[test]
fn clear_on_a_wet_fill_turns_it_off() {
    let d = tempfile::tempdir().unwrap();
    let mut m = r(d.path());
    assert!(view(&m, M).filled.contains_key(&9));
    m.clear_slot_borrow(M, 9).unwrap();
    idle(&mut m);
    assert!(entry(&m, M).no_fill.contains(&9));
    let c = view(&m, M);
    assert!(!c.slots.contains_key(&SlotKind::SkinWet));
    assert_eq!(c.fill_off.get(&9).map(String::as_str), Some(X));
}

#[test]
fn a_turned_off_fill_stays_off_after_reopen() {
    let d = tempfile::tempdir().unwrap();
    let mut m = r(d.path());
    m.set_slot_command(M, 2, sdc_gui::model::sets::SlotCommand::Clear)
        .unwrap();
    idle(&mut m);
    m.save().unwrap();
    let mut m2 = AppModel::new(
        GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..GuiSettings::default()
        },
        None,
        session(),
    );
    m2.accept = engine::policy::AcceptSuggestions::None;
    m2.open_folder(d.path().to_path_buf());
    idle(&mut m2);
    assert!(view(&m2, M).fill_off.contains_key(&2));
    assert!(!view(&m2, M).slots.contains_key(&SlotKind::Normal));
}

#[test]
fn restore_turns_it_back_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = r(d.path());
    assert!(m.restore_fill(M, 2).is_err(), "nothing is off yet");
    m.set_slot_command(M, 2, sdc_gui::model::sets::SlotCommand::Clear)
        .unwrap();
    idle(&mut m);
    m.restore_fill(M, 2).unwrap();
    idle(&mut m);
    assert!(entry(&m, M).no_fill.is_empty());
    assert!(view(&m, M).filled.contains_key(&2));
    m.save().unwrap();
    let text = std::fs::read_to_string(d.path().join("sdc-project.toml")).unwrap();
    assert!(!text.contains("no_fill"), "{text}");
}

#[test]
fn the_menu_offers_restore() {
    let d = tempfile::tempdir().unwrap();
    let mut m = r(d.path());
    let restore = |m: &AppModel| -> Vec<String> {
        m.slot_menu(M, SlotKind::Normal)
            .into_iter()
            .filter(|e| matches!(e.action, MenuAction::RestoreFill { .. }))
            .map(|e| e.label)
            .collect()
    };
    assert!(restore(&m).is_empty());
    // The only command on a wet slot.
    m.clear_slot_borrow(M, 9).unwrap();
    idle(&mut m);
    let wet = m.slot_menu(M, SlotKind::SkinWet);
    assert!(wet
        .iter()
        .any(|e| matches!(e.action, MenuAction::RestoreFill { n: 9, .. })));
    m.restore_fill(M, 9).unwrap();
    idle(&mut m);
    m.set_slot_command(M, 2, sdc_gui::model::sets::SlotCommand::Clear)
        .unwrap();
    idle(&mut m);
    assert_eq!(restore(&m), [format!("From {X}")]);
    assert!(m
        .run_menu(MenuAction::RestoreFill {
            set: M.into(),
            n: 2
        })
        .is_none());
    idle(&mut m);
    assert!(restore(&m).is_empty());
}

#[test]
fn a_deleted_host_map_goes_quietly() {
    let d = tempfile::tempdir().unwrap();
    let mut m = r(d.path());
    assert!(view(&m, M).filled.contains_key(&9));
    std::fs::remove_file(d.path().join("textures/s/x_wet.png")).unwrap();
    m.replan();
    idle(&mut m);
    let c = view(&m, M);
    assert!(!c.filled.contains_key(&9) && !c.slots.contains_key(&SlotKind::SkinWet));
    assert!(c.missing.is_empty(), "{:?}", c.missing);
    assert!(
        problems(&m)
            .iter()
            .all(|p| p.level != sdc_gui::model::bottom::Level::Blocks),
        "{:?}",
        problems(&m)
    );
}

#[test]
fn a_case_difference_still_matches() {
    let d = tempfile::tempdir().unwrap();
    let toml = r_toml("", "").replace(
        "slot2 = \"textures/s/body_n.dds\"\nslot3 = \"textures/s/body_sk.dds\"\n[[sets]]",
        "slot2 = \"textures/s/Body_N.dds\"\nslot3 = \"textures/s/body_sk.dds\"\n[[sets]]",
    );
    assert_ne!(toml, r_toml("", ""), "the H record was edited");
    let m = project_toml(d.path(), R_FILES, &toml);
    assert!(
        view(&m, M).filled.contains_key(&2),
        "{:?}",
        view(&m, M).record_supplied
    );
}

#[test]
fn a_mapped_skin_project_settles() {
    let d = tempfile::tempdir().unwrap();
    let mut m = r(d.path());
    // The plan on screen was made from the inputs the model sends now, within the re-plan bound.
    assert!(m.inputs_settled());
    assert!(m.auto_replans() < 2);
    let marks = m.plan_marks();
    let inputs = engine_inputs(&m);
    m.replan();
    idle(&mut m);
    assert!(m.inputs_settled() && m.auto_replans() < 2);
    assert_eq!(
        m.plan_marks().0,
        marks.0,
        "no inputs changed on a fresh plan"
    );
    assert_eq!(engine_inputs(&m), inputs);
}

fn missing_n(v: &SetView) -> bool {
    v.set_warnings
        .iter()
        .any(|w| w.code == engine::codes::Code::ShaderMissingMap && w.data["map"] == "_n/_msn")
}

/// R2 and R9: a borrowed fill removes C's missing-`_n` warning, Clear brings it back, and restoring removes it again.
#[test]
fn a_fill_clears_the_missing_map_warning_and_clear_brings_it_back() {
    let d = tempfile::tempdir().unwrap();
    let mut m = r(d.path());
    assert!(view(&m, M).filled.contains_key(&2));
    assert!(!missing_n(&view(&m, M)), "{:?}", view(&m, M).set_warnings);
    m.set_slot_command(M, 2, sdc_gui::model::sets::SlotCommand::Clear)
        .unwrap();
    idle(&mut m);
    assert!(missing_n(&view(&m, M)), "{:?}", view(&m, M).set_warnings);
    m.restore_fill(M, 2).unwrap();
    idle(&mut m);
    assert!(!missing_n(&view(&m, M)), "{:?}", view(&m, M).set_warnings);
}

/// The index compares game paths: a host in another mod folder still writes the consumer's record path.
#[test]
fn a_host_in_another_mod_folder_is_borrowed() {
    let d = tempfile::tempdir().unwrap();
    let h = "mod_a/textures/s/x";
    let c = "mod_b/textures/s/e/x";
    let toml = format!(
        "[[sets]]\nid = \"{h}\"\n{}[[sets]]\nid = \"{c}\"\nshader = \"advanced_skin\"\n{}",
        txst(&[(1, "textures/s/e/femalebody.dds"), (2, BODY_N)]),
        txst(&[(1, "textures/s/e/femalebody.dds"), (2, BODY_N)]),
    );
    let m = project_toml(
        d.path(),
        &[
            "mod_a/textures/s/x_normal.png",
            "mod_b/textures/s/e/x.png",
            "mod_b/textures/s/e/x_rfaos.png",
        ],
        &toml,
    );
    let v = view(&m, c);
    assert_eq!(borrowed_from(&v, SlotKind::Normal).as_deref(), Some(h));
    assert!(v.record_supplied.is_empty(), "{:?}", v.record_supplied);
    assert!(!missing_n(&v));
}

/// An unmapped host is indexed by its last plan's output path, folded: a mixed-case name still matches.
#[test]
fn an_unmapped_writer_matches_by_its_folded_output_path() {
    let d = tempfile::tempdir().unwrap();
    let host = "textures/s/Body";
    let toml = format!(
        "[[sets]]\nid = \"{host}\"\n[[sets]]\nid = \"{M}\"\nshader = \"advanced_skin\"\n{}",
        txst(&[(1, "textures/s/e/femalebody.dds"), (2, BODY_N)]),
    );
    let m = project_toml(
        d.path(),
        &[
            "textures/s/Body_n.png",
            "textures/s/e/x.png",
            "textures/s/e/x_rfaos.png",
        ],
        &toml,
    );
    let written = out(&m, "textures/s/Body_n.png");
    assert!(written.ends_with("/textures/s/Body_n.dds"), "{written}");
    let v = view(&m, M);
    // Set ids fold case; the file's own spelling is what the index folds.
    assert!(borrowed_from(&v, SlotKind::Normal).is_some_and(|f| f.eq_ignore_ascii_case(host)));
}

/// A PGPatcher PBR set on CS ignores its mapping, so it writes where its plan puts the file, and a record naming
/// that path fills from it.
#[test]
fn a_pbr_set_on_cs_is_a_writer_by_its_output_path() {
    use engine::config::{GreenConvention, Subtype};
    use sdc_gui::model::Answer;
    let pbr = |m: &mut AppModel| {
        m.decide("textures/v/a.png", Answer::Subtype(Subtype::PbrAlbedo));
        m.decide(
            "textures/v/a_normal.png",
            Answer::Subtype(Subtype::PbrNormal),
        );
        m.decide(
            "textures/v/a_normal.png",
            Answer::Green(GreenConvention::Directx),
        );
        idle(m);
    };
    let host = |extra: &str| format!("[[sets]]\nid = \"textures/v/a\"\n{extra}");
    let d = tempfile::tempdir().unwrap();
    let mut probe = project_toml(
        d.path(),
        &["textures/v/a.png", "textures/v/a_normal.png"],
        &host(""),
    );
    pbr(&mut probe);
    assert!(view(&probe, "textures/v/a").pbr, "a PBR set");
    let path = probe
        .item("textures/v/a_normal.png")
        .unwrap()
        .job_key
        .replace('\\', "/");
    assert!(path.ends_with(".dds"), "{path}");
    // The record on the PBR set names somewhere else; the consumer names where the file really goes.
    let d2 = tempfile::tempdir().unwrap();
    let toml =
        host(&txst(&[(2, "textures/elsewhere/zzz_n.dds")])) + &skin_set(M, &txst(&[(2, &path)]));
    let mut m = project_toml(
        d2.path(),
        &[
            "textures/v/a.png",
            "textures/v/a_normal.png",
            "textures/s/e/x.png",
            "textures/s/e/x_rfaos.png",
        ],
        &toml,
    );
    pbr(&mut m);
    assert!(view(&m, "textures/v/a").pbr);
    let v = view(&m, M);
    assert_eq!(
        borrowed_from(&v, SlotKind::Normal).as_deref(),
        Some("textures/v/a"),
        "{:?} {path}",
        v.record_supplied
    );
}
#[test]
fn restore_is_refused_on_a_pinned_slot_and_a_read_only_file() {
    let d = tempfile::tempdir().unwrap();
    let mut m = r(d.path());
    m.set_slot_command(M, 2, sdc_gui::model::sets::SlotCommand::Clear)
        .unwrap();
    idle(&mut m);
    m.set_slot_lock(M, 2, true).unwrap();
    idle(&mut m);
    assert_eq!(
        m.restore_fill(M, 2),
        Err(sdc_gui::model::sets::SLOT_LOCKED.into())
    );
    assert!(entry(&m, M).no_fill.contains(&2), "nothing was written");

    let d = tempfile::tempdir().unwrap();
    let mut m = project_toml(d.path(), R_FILES, "this is = = not toml\n");
    assert!(m.sets_doc.as_ref().unwrap().read_only());
    let e = m.restore_fill(M, 2).unwrap_err();
    assert!(e.contains("read-only"), "{e}");
}
