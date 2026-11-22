//! Sets menu: Copy JSON settings from one set to the selected sets (material values only, skips explained, one undo
//! step).
mod support;
use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::config::Subtype;
use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::cs_pbr::Kind;
use sdc_gui::model::set_json_copy::{ticked, JsonGroup, JsonRow};
use sdc_gui::model::sets::{is_json_set, is_pbr_grass, views, SetView};
use sdc_gui::model::{Answer, AppModel, Dialog};
use sdc_gui::sets_doc::{SetEntry, SetsDoc};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01";
const LEAF: &str = "landscape/leaf";
const ODD: &str = "landscape/odd";
const LEAF_FILE: &str = "landscape/leaf.png";

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

/// Rock on plain `Pbr`, leaf decided as a PBR albedo and picked as plain `Pbr` (both PGPatcher JSON sets), and odd,
/// which writes no PGPatcher JSON.
fn three_sets(dir: &std::path::Path) -> AppModel {
    let mut m = pbr_opened_plain(dir);
    m.decide(LEAF_FILE, Answer::Subtype(Subtype::PbrAlbedo));
    idle(&mut m);
    m.set_shader(LEAF, Some(engine::shader::Shader::Pbr));
    idle(&mut m);
    m
}

fn key_of(m: &AppModel, id: &str) -> String {
    sdc_gui::model::sets::views(m)
        .iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("{id} has a view"))
        .key
        .clone()
}

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(0.02)
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h
}

/// The card title node of set `id` in the Sets column (the leftmost label of that name).
fn card_title<'a>(h: &'a Harness<'static, AppModel>, id: &'a str) -> egui_kittest::Node<'a> {
    h.query_all_by_label(id)
        .min_by(|a, b| a.rect().min.x.total_cmp(&b.rect().min.x))
        .unwrap_or_else(|| panic!("{id}'s card"))
}

/// Right-click set `id`'s card: its menu opens.
fn open_card_menu(h: &mut Harness<'static, AppModel>, id: &str) {
    let at = card_title(h, id).rect().center();
    h.hover_at(at);
    h.run();
    card_title(h, id).click_secondary();
    h.run();
}

#[test]
fn copy_json_settings_from_the_set_menu_writes_the_other_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    assert!(
        is_json_set(&view(&m, ROCK), m.target)
            && is_json_set(&view(&m, LEAF), m.target)
            && !is_json_set(&view(&m, ODD), m.target),
        "premise: rock and leaf are JSON sets, odd is not"
    );
    m.set_json_value(ROCK, "roughness_scale", Some(json!(0.8)));
    idle(&mut m);
    let (rock, leaf) = (key_of(&m, ROCK), key_of(&m, LEAF));
    m.click_set(&leaf, false, false);
    m.click_set(&rock, true, false);
    let mut h = harness(m);
    h.run();
    open_card_menu(&mut h, ROCK);
    assert!(h
        .query_by_label_contains("Copy JSON settings from")
        .is_some());
    h.get_by_label_contains("Copy JSON settings from").click();
    h.run();
    h.get_by_label("Apply").click();
    h.run();
    idle(h.state_mut());
    let m = h.state();
    let leaf_value = m
        .sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|e| e.id == LEAF)
        .and_then(|e| e.settings.get("roughness_scale").cloned());
    assert_eq!(leaf_value, Some(json!(0.8)));
}

fn view(m: &AppModel, id: &str) -> SetView {
    views(m)
        .iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("no set {id}"))
        .clone()
}

fn entry(m: &AppModel, id: &str) -> Option<SetEntry> {
    let doc = m.sets_doc.as_ref().unwrap();
    doc.entries().iter().find(|e| e.id == id).cloned()
}

fn doc_text(m: &AppModel) -> String {
    m.sets_doc.as_ref().unwrap().text()
}

fn pairs(rows: &[JsonRow]) -> Vec<(JsonGroup, String)> {
    rows.iter().map(|r| (r.group, r.key.clone())).collect()
}

/// The rows of `rows` whose key is in `keys`, in the rows' own order.
fn pick(rows: Vec<JsonRow>, keys: &[&str]) -> Vec<JsonRow> {
    rows.into_iter()
        .filter(|r| keys.contains(&r.key.as_str()))
        .collect()
}

fn set(m: &mut AppModel, id: &str, key: &str, v: Value) {
    m.set_json_value(id, key, Some(v));
}

fn report_lines(m: &AppModel) -> Vec<String> {
    m.bulk
        .report
        .clone()
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn logged(m: &AppModel) -> Vec<String> {
    m.messages.entries().map(|e| e.text.clone()).collect()
}

fn select(m: &mut AppModel, ids: &[&str]) {
    for (n, id) in ids.iter().enumerate() {
        let key = key_of(m, id);
        m.click_set(&key, n > 0, false);
    }
}

fn row_of(group: JsonGroup, key: &str, label: &str, value: Value) -> JsonRow {
    JsonRow {
        group,
        key: key.into(),
        label: label.into(),
        value,
    }
}

#[test]
fn the_rows_are_material_values_only() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    for (k, v) in [
        ("roughness_scale", json!(0.8)),
        ("emissive", json!(true)),
        ("match2", json!("landscape/rock01_n")),
        ("rename", json!("stone")),
        ("lock_diffuse", json!(true)),
        ("cubemap", json!("textures\\cube.dds")),
        ("slot3", json!("textures\\x_g.dds")),
    ] {
        set(&mut m, ROCK, k, v);
    }
    idle(&mut m);
    assert_eq!(
        pairs(&m.json_rows(ROCK)),
        vec![
            (JsonGroup::Pgp, "emissive".to_string()),
            (JsonGroup::Pgp, "roughness_scale".to_string())
        ]
    );
}

#[test]
fn raw_glint_and_fuzz_switches_are_never_rows() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    m.edit_docs("seed a hand edit", None, true, |_, s| {
        s.set_value(ROCK, "glint", Some(json!(true)));
        s.set_value(ROCK, "fuzz", Some(json!(true)));
    });
    set(&mut m, ROCK, "glint_on", json!(true));
    set(&mut m, LEAF, "glint.screen_space_scale", json!(2.0));
    idle(&mut m);
    let e = entry(&m, ROCK).unwrap();
    assert!(
        e.settings.contains_key("glint") && e.settings.contains_key("fuzz"),
        "premise: the raw switches are in rock's settings: {:?}",
        e.settings
    );
    let rows = m.json_rows(ROCK);
    assert!(rows.iter().any(|r| r.key == "glint_on"), "{rows:?}");
    assert!(
        rows.iter().all(|r| r.key != "glint" && r.key != "fuzz"),
        "{rows:?}"
    );
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    let leaf = entry(&m, LEAF).unwrap();
    assert_eq!(
        leaf.settings.get("glint.screen_space_scale"),
        Some(&json!(2.0)),
        "{:?}",
        leaf.settings
    );
    let text = doc_text(&m);
    let leaf_table = text
        .split("[[sets]]")
        .find(|t| t.contains(&format!("id = \"{LEAF}\"")))
        .expect("leaf's table");
    assert!(
        !leaf_table.lines().any(|l| l.trim() == "glint = true"),
        "{leaf_table}"
    );
}

#[test]
fn every_group_copies_material_values_and_keeps_the_targets_own() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    // Rock: a texture-set block, a material object, a complex material block and the PGPatcher values.
    m.add_cs_block(ROCK, Kind::Txst);
    m.set_cs_edids(ROCK, Kind::Txst, &["RockTXST".to_string()]);
    m.set_cs_value(ROCK, Kind::Txst, "fuzzWeight", Some(json!(0.5)));
    m.add_cs_block(ROCK, Kind::Mato);
    m.set_cs_value(ROCK, Kind::Mato, "roughness", Some(json!(0.4)));
    m.set_cm_export(ROCK, true);
    m.set_cm_value(ROCK, "glossiness", Some(json!(50)));
    m.set_cm_value(ROCK, "disabled", Some(json!(true)));
    for (k, v) in [
        ("roughness_scale", json!(0.8)),
        ("emissive", json!(true)),
        ("match2", json!("landscape/rock01_n")),
        ("rename", json!("stone")),
        ("lock_diffuse", json!(true)),
        ("cubemap", json!("textures\\cube.dds")),
        ("slot3", json!("textures\\x_g.dds")),
    ] {
        set(&mut m, ROCK, k, v);
    }
    // Leaf: its own identity and controls, and the blocks a copy fills.
    m.add_cs_block(LEAF, Kind::Txst);
    m.set_cs_edids(LEAF, Kind::Txst, &["LeafTXST".to_string()]);
    m.add_cs_block(LEAF, Kind::Mato);
    m.set_cm_export(LEAF, false);
    for (k, v) in [
        ("match2", json!("landscape/leaf_n")),
        ("rename", json!("maple")),
        ("lock_diffuse", json!(false)),
        ("cubemap", json!("textures\\leafcube.dds")),
        ("slot3", json!("textures\\leaf_g.dds")),
    ] {
        set(&mut m, LEAF, k, v);
    }
    idle(&mut m);
    let rows = m.json_rows(ROCK);
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    m.save().unwrap();
    // The persisted document, not the model.
    let doc = SetsDoc::open(&d.path().join("textures"));
    let leaf = doc
        .entries()
        .iter()
        .find(|e| e.id == LEAF)
        .cloned()
        .expect("leaf's entry");
    assert_eq!(leaf.settings.get("roughness_scale"), Some(&json!(0.8)));
    assert_eq!(leaf.settings.get("emissive"), Some(&json!(true)));
    let txst = leaf.cs_txst.clone().unwrap();
    assert_eq!(txst.values.get("fuzzWeight"), Some(&json!(0.5)));
    assert_eq!(txst.edids, ["LeafTXST"], "leaf keeps its own EDID");
    assert_eq!(
        leaf.cs_mato.unwrap().values.get("roughness"),
        Some(&json!(0.4))
    );
    let cm = leaf.cm.unwrap();
    assert_eq!(cm.values.get("glossiness"), Some(&json!(50)));
    assert_eq!(cm.values.get("disabled"), Some(&json!(true)));
    assert!(!cm.export, "leaf keeps its own export choice");
    for (k, v) in [
        ("match2", json!("landscape/leaf_n")),
        ("rename", json!("maple")),
        ("lock_diffuse", json!(false)),
        ("cubemap", json!("textures\\leafcube.dds")),
        ("slot3", json!("textures\\leaf_g.dds")),
    ] {
        assert_eq!(leaf.settings.get(k), Some(&v), "{k} is leaf's own");
    }
}

fn skin_colour(dir: &Path, name: &str) {
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
    })
    .save(dir.join(name))
    .unwrap();
}

/// Advanced skin sets `bases` (diffuse, `_rfaos`, `_wet`, `_detail_n`) under `<dir>/textures/x`, exported to
/// `<dir>/export` (the fixture of `cs_skin_json.rs`).
fn skin_project(dir: &Path, bases: &[&str]) -> AppModel {
    let r = dir.join("textures/x");
    std::fs::create_dir_all(&r).unwrap();
    for base in bases {
        for s in ["", "_rfaos", "_wet", "_detail_n"] {
            skin_colour(&r, &format!("{base}{s}.png"));
        }
    }
    opened_skin(dir)
}

fn opened_skin(dir: &Path) -> AppModel {
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(dir.join("export")),
        ..sdc_gui::settings::GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, support::session());
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    support::choose_export(&mut m, dir.join("export"));
    assert_eq!(m.target, engine::config::Target::Cs);
    m
}

fn skin_id(m: &AppModel, key_end: &str) -> String {
    views(m)
        .iter()
        .find(|v| v.key.ends_with(key_end))
        .unwrap_or_else(|| panic!("no set {key_end}"))
        .id
        .clone()
}

#[test]
fn skin_values_copy_exactly_without_pruning() {
    use sdc_gui::model::cs_skin_json::skin_json_plan;
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), &["a", "b"]);
    let (a, b) = (skin_id(&m, "x/a"), skin_id(&m, "x/b"));
    m.edit_docs("seed skin values", None, false, |_, s| {
        s.set_value(&a, "skin.Roughness", Some(json!(0.71234)));
        s.set_value(&b, "skin.OldKey", Some(json!(1)));
    });
    let rows = m.json_rows(&a);
    assert_eq!(
        pairs(&rows),
        vec![(JsonGroup::Skin, "Roughness".to_string())]
    );
    m.apply_copy_set_json("a", false, std::slice::from_ref(&b), &rows);
    idle(&mut m);
    let skin = entry(&m, &b).unwrap().skin;
    assert_eq!(skin.get("Roughness"), Some(&json!(0.71234)));
    assert_eq!(skin.get("OldKey"), Some(&json!(1)), "an unknown key stays");
    // The writer's own output holds the same value for both sets.
    let plan = skin_json_plan(&m).0;
    let rough = |id: &str| {
        plan.iter()
            .find(|p| p.0 == id)
            .unwrap_or_else(|| panic!("{id} writes a skin JSON: {plan:?}"))
            .3
            .parameters
            .get("Roughness")
            .cloned()
    };
    assert_eq!(rough(&b), Some(json!(0.71234)));
    assert_eq!(rough(&b), rough(&a));
    // Persisted.
    m.save().unwrap();
    let doc = SetsDoc::open(d.path());
    let saved = doc.entries().iter().find(|e| e.id == b).cloned().unwrap();
    assert_eq!(saved.skin.get("OldKey"), Some(&json!(1)));
    assert_eq!(saved.skin.get("Roughness"), Some(&json!(0.71234)));
}
#[test]
fn xpmf_values_copy_within_bounds_and_each_profile_keeps_its_own() {
    use sdc_gui::model::xpmf_json::{is_landscape, prepare, profile};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.decide(LEAF_FILE, Answer::Subtype(Subtype::PbrAlbedo));
    idle(&mut m);
    assert!(
        is_landscape(&view(&m, ROCK)) && is_landscape(&view(&m, LEAF)),
        "premise: both sets are landscape sets"
    );
    let block = |v: Value| v.as_object().cloned();
    m.edit_docs("seed XPMF blocks", None, false, |_, s| {
        s.set_xpmf(
            ROCK,
            block(
                json!({"shelterFade": 32, "shelterRandomness": 500, "editorIds": ["Rock*"],
                "name": "rock", "diffuseTexture": "textures\\r.dds", "pbr": false}),
            ),
        );
        s.set_xpmf(
            LEAF,
            block(json!({"editorIds": ["Leaf*"], "name": "leaf",
                "diffuseTexture": "textures\\l.dds", "pbr": true})),
        );
    });
    idle(&mut m);
    let rows = m.json_rows(ROCK);
    let xpmf: Vec<&str> = rows
        .iter()
        .filter(|r| r.group == JsonGroup::Xpmf)
        .map(|r| r.key.as_str())
        .collect();
    assert_eq!(
        xpmf,
        ["shelterFade"],
        "neither the out-of-range number nor pbr"
    );
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    let leaf = entry(&m, LEAF).unwrap();
    let p = profile(&m, &leaf).unwrap_or_else(|e| panic!("leaf's profile is refused: {e}"));
    assert_eq!(p["shelterFade"].as_f64(), Some(32.0));
    assert_eq!(p["editorIds"], json!(["Leaf*"]));
    assert_eq!(p["name"], json!("leaf"));
    assert_eq!(p["diffuseTexture"], json!("textures\\l.dds"));
    assert_eq!(p["pbr"], json!(true));
    let pending = prepare(&m).expect("profiles to write");
    let file = |n: &str| format!("SKSE/Plugins/XPMF/tessalith_{n}.json");
    assert!(
        pending.files.iter().any(|f| f.0 == file("leaf")),
        "{pending:?}"
    );
    assert!(!pending.refused.contains(&file("leaf")), "{pending:?}");
    assert!(
        pending.refused.contains(&file("rock01")),
        "rock's own profile is refused over its 500: {pending:?}"
    );
}

/// The `shared` fixture of `txst_map.rs`: `x_normal`, `x_sk` and `x_wet` under `textures/s`, `textures/s/x` marked
/// Advanced skin.
fn shared_skin(dir: &Path) -> AppModel {
    for f in ["x_normal", "x_sk", "x_wet"] {
        let p = dir.join(format!("textures/s/{f}.png"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        skin_colour(p.parent().unwrap(), &format!("{f}.png"));
    }
    std::fs::write(
        dir.join("sdc-project.toml"),
        "[[sets]]\nid = \"textures/s/x\"\nshader = \"advanced_skin\"\n",
    )
    .unwrap();
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(dir.join("export")),
        ..sdc_gui::settings::GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, support::session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    m
}

fn share(m: &mut AppModel, item: &str, to: &str) {
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    let o = m.drop_choice(item, to, DropChoice::Share);
    if let DropOutcome::AskReplace { kind, existing } = o {
        m.drop_replace(item, to, DropChoice::Share, kind, &existing)
            .unwrap();
    }
    idle(m);
}

/// Apply `rows` to `[to]` and check that nothing was written, nothing is on the undo stack and the report says why.
fn skipped_everywhere(m: &mut AppModel, to: &str, rows: &[JsonRow], reason: &str) {
    let (undo, text) = (m.history.undo.len(), doc_text(m));
    m.apply_copy_set_json("source", false, &[to.to_string()], rows);
    idle(m);
    assert_eq!(m.history.undo.len(), undo, "no undo step");
    assert_eq!(doc_text(m), text, "the document is unchanged");
    let line = format!("skipped on 1 of 1: {reason}");
    assert!(
        report_lines(m).iter().any(|l| l.ends_with(&line)),
        "{:?}",
        report_lines(m)
    );
    assert!(
        logged(m).iter().any(|l| l.ends_with(&line)),
        "Messages: {:?}",
        logged(m)
    );
}

#[test]
fn skin_skip_reasons_follow_the_skin_writer() {
    let skin_row = [row_of(
        JsonGroup::Skin,
        "Roughness",
        "Roughness",
        json!(0.5),
    )];
    // A shared skin set: it has no diffuse, so the "not an Advanced skin set" check must not come first.
    let d = tempfile::tempdir().unwrap();
    let mut m = shared_skin(d.path());
    let x = "textures/s/x";
    assert!(view(&m, x).shared, "premise: a shared set");
    skipped_everywhere(&mut m, x, &skin_row, "is a shared skin set");
    // A borrowed diffuse: the skin writer skips the borrower.
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/x");
    std::fs::create_dir_all(&r).unwrap();
    for s in ["", "_rfaos", "_wet", "_detail_n"] {
        skin_colour(&r, &format!("a{s}.png"));
    }
    skin_colour(&r, "b_rfaos.png");
    let mut m = opened_skin(d.path());
    let b = skin_id(&m, "x/b");
    share(&mut m, "textures/x/a.png", &b);
    let v = view(&m, &b);
    assert!(
        matches!(
            v.slots.get(&sdc_gui::model::sets::SlotKind::Diffuse),
            Some(sdc_gui::model::sets::SlotFill::Borrowed { .. })
        ) && v.skin,
        "premise: b borrows its diffuse and is a skin set: {:?}",
        v.slots
    );
    skipped_everywhere(&mut m, &b, &skin_row, "borrows its diffuse");
}

#[test]
fn a_hand_edited_value_in_the_way_is_skipped_not_counted() {
    let way = "has a hand-edited value in the way";
    // An intermediate parent holding a plain value, and a final item that is a table.
    for (seed, label) in [
        (("skin", json!("legacy")), "parent"),
        (("skin.Roughness.old", json!(1)), "final item"),
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut m = skin_project(d.path(), &["a", "b"]);
        let (a, b) = (skin_id(&m, "x/a"), skin_id(&m, "x/b"));
        m.edit_docs("seed", None, false, |_, s| {
            s.set_value(&a, "skin.Roughness", Some(json!(0.5)));
            s.set_value(&b, seed.0, Some(seed.1.clone()));
        });
        let rows = m.json_rows(&a);
        assert_eq!(
            pairs(&rows),
            vec![(JsonGroup::Skin, "Roughness".to_string())],
            "{label}"
        );
        assert!(
            !m.sets_doc.as_ref().unwrap().path_open(&b, "skin.Roughness"),
            "premise ({label}): the path is blocked"
        );
        skipped_everywhere(&mut m, &b, &rows, way);
    }
    // The root: a top-level `sets` that is not an array of tables, on an automatic JSON target with no entry.
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.decide(LEAF_FILE, Answer::Subtype(Subtype::PbrAlbedo));
    idle(&mut m);
    let root = d.path().join("textures");
    std::fs::write(root.join("sdc-project.toml"), "sets = \"hand\"\n").unwrap();
    m.sets_doc = Some(SetsDoc::open(&root));
    assert!(!m.sets_doc.as_ref().unwrap().read_only());
    assert!(m.sets_doc.as_ref().unwrap().entries().is_empty());
    assert!(
        is_json_set(&view(&m, LEAF), m.target),
        "premise: leaf is a JSON set"
    );
    assert!(
        !m.sets_doc
            .as_ref()
            .unwrap()
            .path_open(LEAF, "roughness_scale"),
        "premise: the root blocks the write"
    );
    let rows = [row_of(
        JsonGroup::Pgp,
        "roughness_scale",
        "Roughness scale",
        json!(0.8),
    )];
    skipped_everywhere(&mut m, LEAF, &rows, way);
}

#[test]
fn glint_ownership_is_decided_once_per_target() {
    use sdc_gui::model::cs_pbr::mato_synced;
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    // An older project: the two blocks differ only in `enabled`, no `unsynced`.
    m.edit_docs("seed leaf's blocks", None, true, |_, s| {
        for (kind, enabled) in [(Kind::Txst, false), (Kind::Mato, true)] {
            s.set_cs_value(LEAF, kind, "glintParameters.enabled", Some(json!(enabled)));
            s.set_cs_value(
                LEAF,
                kind,
                "glintParameters.screenSpaceScale",
                Some(json!(1.0)),
            );
        }
    });
    idle(&mut m);
    let (txst, mato) = (
        m.cs_block(LEAF, Kind::Txst).unwrap(),
        m.cs_block(LEAF, Kind::Mato).unwrap(),
    );
    assert!(mato.unsynced.is_empty());
    assert!(
        !mato_synced(Some(&txst), &mato, "glintParameters"),
        "premise: leaf's material object does not follow its texture set"
    );
    set(&mut m, ROCK, "glint_on", json!(true));
    set(&mut m, ROCK, "glint.screen_space_scale", json!(3.0));
    idle(&mut m);
    let rows = pick(m.json_rows(ROCK), &["glint_on", "glint.screen_space_scale"]);
    assert_eq!(rows.len(), 2, "{rows:?}");
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    let glint = |k: Kind| m.cs_block(LEAF, k).unwrap().values["glintParameters"].clone();
    assert_eq!(glint(Kind::Txst)["enabled"], json!(true));
    assert_eq!(glint(Kind::Txst)["screenSpaceScale"], json!(3.0));
    assert_eq!(glint(Kind::Mato)["enabled"], json!(true));
    assert_eq!(
        glint(Kind::Mato)["screenSpaceScale"],
        json!(1.0),
        "the unticked, independent material object is not pulled along"
    );
}

#[test]
fn a_material_object_alone_takes_glint_rows() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    m.add_cs_block(LEAF, Kind::Mato);
    m.add_cs_block(ROCK, Kind::Mato);
    m.set_cs_value(
        ROCK,
        Kind::Mato,
        "glintParameters.enabled",
        Some(json!(true)),
    );
    idle(&mut m);
    assert!(
        m.cs_block(LEAF, Kind::Txst).is_none(),
        "premise: no texture set"
    );
    let rows = pick(m.json_rows(ROCK), &["glintParameters.enabled"]);
    assert_eq!(rows.len(), 1, "{rows:?}");
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    let mato = m.cs_block(LEAF, Kind::Mato).unwrap();
    assert_eq!(mato.values["glintParameters"]["enabled"], json!(true));
    assert!(
        report_lines(&m)[0].ends_with("written to 1 of 1"),
        "{:?}",
        report_lines(&m)
    );
    assert!(
        m.cs_block(LEAF, Kind::Txst).is_none(),
        "no block is created"
    );
}

#[test]
fn pbr_grass_takes_no_ignored_key_even_a_mapped_one() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    m.set_shader(LEAF, Some(engine::shader::Shader::PbrGrass));
    idle(&mut m);
    let v = view(&m, LEAF);
    assert!(is_pbr_grass(&v) && is_json_set(&v, m.target));
    assert!(m.cs_block(LEAF, Kind::Txst).is_none());
    set(&mut m, ROCK, "displacement_scale", json!(0.3));
    idle(&mut m);
    let rows = pick(m.json_rows(ROCK), &["displacement_scale"]);
    assert_eq!(rows.len(), 1, "{rows:?}");
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    assert!(
        report_lines(&m)[0].ends_with("skipped on 1 of 1: PBR grass does not use this key"),
        "{:?}",
        report_lines(&m)
    );
    assert!(entry(&m, LEAF).is_none_or(|e| !e.settings.contains_key("displacement_scale")));
}

#[test]
fn cs_values_reach_a_block_even_without_pgpatcher_json_and_glint_stays_synced() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    // Rock: a texture set with a roughness scale and glint on, and a material object with glint off.
    m.add_cs_block(ROCK, Kind::Txst);
    m.set_cs_value(ROCK, Kind::Txst, "roughnessScale", Some(json!(0.8)));
    m.set_cs_value(
        ROCK,
        Kind::Txst,
        "glintParameters.enabled",
        Some(json!(true)),
    );
    m.add_cs_block(ROCK, Kind::Mato);
    m.set_cs_value(
        ROCK,
        Kind::Mato,
        "glintParameters.enabled",
        Some(json!(false)),
    );
    // Odd writes no PGPatcher JSON, but its texture set takes the values.
    m.add_cs_block(ODD, Kind::Txst);
    idle(&mut m);
    assert!(
        !is_json_set(&view(&m, ODD), m.target),
        "premise: odd writes no JSON"
    );
    let rock_rows = m.json_rows(ROCK);
    let pgp = pick(rock_rows.clone(), &["roughness_scale", "glint_on"]);
    assert_eq!(pgp.len(), 2, "{rock_rows:?}");
    m.apply_copy_set_json("rock", false, &[ODD.to_string()], &pgp);
    idle(&mut m);
    let block = m.cs_block(ODD, Kind::Txst).unwrap();
    assert_eq!(block.values["roughnessScale"], json!(0.8));
    assert_eq!(block.values["glintParameters"]["enabled"], json!(true));
    // Leaf: a texture set and a material object that follows it.
    m.add_cs_block(LEAF, Kind::Txst);
    m.add_mato_synced(LEAF);
    idle(&mut m);
    assert!(
        m.glint_follows(LEAF),
        "premise: leaf's material object follows"
    );
    let mato_row = pick(rock_rows, &["glintParameters.enabled"]);
    let rows: Vec<JsonRow> = pick(m.json_rows(ROCK), &["glint_on"])
        .into_iter()
        .chain(mato_row)
        .collect();
    assert_eq!(rows.len(), 2, "{rows:?}");
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    for k in [Kind::Txst, Kind::Mato] {
        assert_eq!(
            m.cs_block(LEAF, k).unwrap().values["glintParameters"]["enabled"],
            json!(true),
            "{k:?}"
        );
    }
    assert!(
        report_lines(&m)[1].ends_with("skipped on 1 of 1: follows its texture set"),
        "{:?}",
        report_lines(&m)
    );
}

#[test]
fn a_value_of_the_wrong_type_is_not_a_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), &["a", "b"]);
    let a = skin_id(&m, "x/a");
    m.edit_docs("seed", None, false, |_, s| {
        s.set_value(&a, "skin.Roughness", Some(json!("textures\\r.dds")))
    });
    assert!(m.json_rows(&a).is_empty(), "{:?}", m.json_rows(&a));
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    m.edit_docs("seed", None, true, |_, s| {
        s.set_value(ROCK, "roughness_scale", Some(json!("textures\\r.dds")))
    });
    idle(&mut m);
    assert!(m.json_rows(ROCK).is_empty(), "{:?}", m.json_rows(ROCK));
}

#[test]
fn copy_skips_with_reasons_in_one_undo_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    m.decide(LEAF_FILE, Answer::Subtype(Subtype::PbrAlbedo));
    idle(&mut m);
    set(&mut m, ROCK, "roughness_scale", json!(0.8));
    idle(&mut m);
    assert!(
        is_json_set(&view(&m, LEAF), m.target),
        "premise: leaf is a JSON set"
    );
    assert!(entry(&m, LEAF).is_none(), "premise: leaf has no entry");
    assert!(!is_json_set(&view(&m, ODD), m.target));
    select(&mut m, &[LEAF, ODD, ROCK]);
    let short = view(&m, ROCK).short;
    m.run_menu(MenuAction::CopyJsonFromActive);
    let Some(Dialog::CopySetJson {
        source,
        paste,
        to,
        rows,
    }) = m.dialog.clone()
    else {
        panic!("the dialog opens: {:?}", m.dialog);
    };
    assert_eq!((source.as_str(), paste), (short.as_str(), false));
    assert_eq!(to, [LEAF, ODD]);
    assert_eq!(
        rows,
        vec![(
            row_of(
                JsonGroup::Pgp,
                "roughness_scale",
                "Roughness scale",
                json!(0.8)
            ),
            true
        )]
    );
    let (before, undo) = (doc_text(&m), m.history.undo.len());
    let toasts = m.messages.toasts().len() + m.messages.queued();
    m.apply_copy_set_json(&source, paste, &to, &ticked(&rows));
    // SM-T3-1: the plan the write starts rewrites the status line from its counts; the result stays in front of them.
    idle(&mut m);
    let summary = format!("Copy JSON settings from {short}: 1 written, 1 skipped (see Messages)");
    assert!(
        m.status_line.starts_with(&summary),
        "after the replan: {:?}",
        m.status_line
    );
    assert_eq!(
        m.messages.toasts().len() + m.messages.queued(),
        toasts + 1,
        "one summary toast"
    );
    let leaf = entry(&m, LEAF).expect("leaf gets an entry, as any edit in its panel does");
    assert_eq!(
        leaf.settings.into_iter().collect::<Vec<_>>(),
        vec![("roughness_scale".to_string(), json!(0.8))]
    );
    assert_eq!(m.history.undo.len(), undo + 1);
    let line = "PGPatcher JSON: Roughness scale: written to 1 of 2; skipped on 1 of 2: writes no PGPatcher JSON";
    assert_eq!(report_lines(&m), [line]);
    assert!(logged(&m).contains(&line.to_string()), "{:?}", logged(&m));
    m.undo();
    assert_eq!(doc_text(&m), before);
    assert!(entry(&m, LEAF).is_none(), "leaf has no entry again");
}

#[test]
fn one_apply_is_one_undo_step_for_every_write() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    set(&mut m, ROCK, "roughness_scale", json!(0.8));
    set(&mut m, ROCK, "emissive", json!(true));
    idle(&mut m);
    let rows = m.json_rows(ROCK);
    assert_eq!(rows.len(), 2, "{rows:?}");
    let (before, undo) = (doc_text(&m), m.history.undo.len());
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    let leaf = entry(&m, LEAF).unwrap();
    assert_eq!(leaf.settings.get("roughness_scale"), Some(&json!(0.8)));
    assert_eq!(leaf.settings.get("emissive"), Some(&json!(true)));
    assert_eq!(m.history.undo.len(), undo + 1, "one step for two writes");
    // SM-T3-1 on the common path: one target, every row written, and the line still says so after the replan.
    assert!(
        m.status_line
            .starts_with("Copy JSON settings from rock: 2 written, 0 skipped (see Messages)"),
        "{:?}",
        m.status_line
    );
    m.undo();
    assert_eq!(doc_text(&m), before);
    let leaf = entry(&m, LEAF);
    assert!(
        leaf.is_none_or(|e| e.settings.is_empty()),
        "leaf holds neither value"
    );
}

#[test]
fn the_menus_offer_copy_json_with_counts_and_reasons() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    set(&mut m, ROCK, "roughness_scale", json!(0.8));
    idle(&mut m);
    let (rock, odd) = (view(&m, ROCK).short, view(&m, ODD).short);
    select(&mut m, &[ROCK]);
    let e = m.copy_json_entry();
    assert_eq!(
        e.label,
        format!("Copy JSON settings from {rock} to 0 selected…")
    );
    assert_eq!(e.disabled.as_deref(), Some("select two or more sets"));
    select(&mut m, &[LEAF, ROCK]);
    let e = m.copy_json_entry();
    assert_eq!(
        e.label,
        format!("Copy JSON settings from {rock} to 1 selected…")
    );
    assert_eq!(e.disabled, None);
    assert_eq!(m.set_menu()[1].action, MenuAction::CopyJsonFromActive);
    select(&mut m, &[ROCK, ODD]);
    assert_eq!(
        m.copy_json_entry().disabled,
        Some(format!("{odd} has no JSON settings of its own"))
    );
    // A read-only sets document: its reason comes first.
    let bad = d.path().join("bad");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("sdc-project.toml"), "this is = = not toml\n").unwrap();
    m.sets_doc = Some(SetsDoc::open(&bad));
    assert!(m.sets_doc.as_ref().unwrap().read_only());
    select(&mut m, &[LEAF, ROCK]);
    let why = m.sets_read_only_reason();
    assert!(why.is_some());
    assert_eq!(m.copy_json_entry().disabled, why);
}

#[test]
fn the_dialog_names_each_row_with_its_value_and_applies_only_ticked_rows() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    set(&mut m, ROCK, "roughness_scale", json!(0.8));
    set(&mut m, ROCK, "emissive", json!(true));
    idle(&mut m);
    select(&mut m, &[LEAF, ROCK]);
    let mut h = harness(m);
    h.run();
    open_card_menu(&mut h, ROCK);
    h.get_by_label_contains("Copy JSON settings from").click();
    h.run();
    h.get_by_role_and_label(Role::CheckBox, "PGPatcher JSON: Roughness scale: 0.8");
    h.get_by_role_and_label(Role::CheckBox, "PGPatcher JSON: Emissive: true")
        .click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Apply").click();
    h.run();
    idle(h.state_mut());
    let leaf = entry(h.state(), LEAF).unwrap();
    assert_eq!(leaf.settings.get("roughness_scale"), Some(&json!(0.8)));
    assert!(
        !leaf.settings.contains_key("emissive"),
        "{:?}",
        leaf.settings
    );
}

/// `three_sets` where leaf also has a resolved subsurface slot (`leaf_s.png`, a `pbr_subsurface`), so its PGPatcher
/// entry can write `subsurface_opacity` and the project can be exported.
fn export_ready(dir: &Path) -> AppModel {
    let r = dir.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb([200u8, 100, 50]))
        .save(r.join("leaf_s.png"))
        .unwrap();
    let mut m = pbr_opened_plain(dir);
    m.decide(LEAF_FILE, Answer::Subtype(Subtype::PbrAlbedo));
    idle(&mut m);
    m.set_shader(LEAF, Some(engine::shader::Shader::Pbr));
    idle(&mut m);
    m.set_field("landscape/leaf_s.png", "subtype", json!("pbr_subsurface"));
    idle(&mut m);
    // The leaf has transparent pixels: its alpha is a decision the export waits for.
    m.decide(
        LEAF_FILE,
        Answer::Alpha(engine::config::AlphaPolicy::Cutout),
    );
    idle(&mut m);
    m
}

/// The CS record of texture-set EDID `edid` as an export writes it, parsed.
fn cs_record(m: &AppModel, edid: &str) -> Value {
    use sdc_gui::model::cs_pbr_export::{prepare, rel_path};
    let rel = rel_path(Kind::Txst, edid);
    let pending = prepare(m).expect("CS records");
    let (_, text) = pending
        .files
        .iter()
        .find(|f| f.0 == rel)
        .unwrap_or_else(|| panic!("{rel} is written: {:?}", pending.files));
    serde_json::from_str(text).unwrap()
}

/// Export the project (the gate must allow it) and return leaf's entry of the PGPatcher JSON, parsed.
fn leaf_json_entry(m: &mut AppModel) -> Value {
    idle(m);
    let path = sdc_gui::model::pbr_export::prepare(m)
        .expect("a PGPatcher JSON to write")
        .path;
    assert!(
        m.review_gate().is_ok(),
        "the project is ready to export: {:?}",
        m.review_gate()
    );
    export_through(m);
    idle(m);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; {:?}", path.display(), m.export.notes));
    let v: Value = serde_json::from_str(&text).unwrap();
    v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["match1"] == json!("landscape\\leaf.dds"))
        .unwrap_or_else(|| panic!("leaf's entry in {text}"))
        .clone()
}

#[test]
fn a_cs_source_value_copies_as_is_and_clamps_in_the_pgpatcher_output() {
    let d = tempfile::tempdir().unwrap();
    let mut m = export_ready(d.path());
    // Rock: a CS value outside the PGPatcher range. Odd: a texture set. Leaf: no block, subsurface on.
    m.add_cs_block(ROCK, Kind::Txst);
    m.set_cs_edids(ROCK, Kind::Txst, &["RockTXST".to_string()]);
    m.set_cs_value(ROCK, Kind::Txst, "subsurfaceOpacity", Some(json!(2.0)));
    m.add_cs_block(ODD, Kind::Txst);
    m.set_cs_edids(ODD, Kind::Txst, &["OddTXST".to_string()]);
    set(&mut m, LEAF, "subsurface", json!(true));
    idle(&mut m);
    assert_eq!(cs_record(&m, "RockTXST")["subsurfaceOpacity"], json!(2.0));
    assert!(is_json_set(&view(&m, LEAF), m.target));
    assert!(m.cs_block(LEAF, Kind::Txst).is_none());
    let rows = pick(m.json_rows(ROCK), &["subsurface_opacity"]);
    assert_eq!(
        rows,
        [row_of(
            JsonGroup::Pgp,
            "subsurface_opacity",
            "Subsurface opacity",
            json!(2.0)
        )]
    );
    m.apply_copy_set_json("rock", false, &[ODD.to_string(), LEAF.to_string()], &rows);
    idle(&mut m);
    assert_eq!(
        cs_record(&m, "OddTXST")["subsurfaceOpacity"],
        json!(2.0),
        "CS to CS is unchanged"
    );
    assert_eq!(
        entry(&m, LEAF).unwrap().settings.get("subsurface_opacity"),
        Some(&json!(1.0)),
        "converted at the PGPatcher destination"
    );
    let written = leaf_json_entry(&mut m);
    assert_eq!(written["subsurface_opacity"], json!(1.0));
}

#[test]
fn a_plain_pgpatcher_source_copies_its_exported_value_to_cs() {
    let d = tempfile::tempdir().unwrap();
    let mut m = export_ready(d.path());
    set(&mut m, LEAF, "subsurface", json!(true));
    // A hand edit the panel would keep in range.
    m.edit_docs("seed a hand edit", None, true, |_, s| {
        s.set_value(LEAF, "subsurface_opacity", Some(json!(2.0)));
    });
    m.add_cs_block(ODD, Kind::Txst);
    m.set_cs_edids(ODD, Kind::Txst, &["OddTXST".to_string()]);
    idle(&mut m);
    assert!(m.cs_block(LEAF, Kind::Txst).is_none());
    let written = leaf_json_entry(&mut m);
    assert_eq!(
        written["subsurface_opacity"],
        json!(1.0),
        "premise: the source exports 1.0"
    );
    let rows = pick(m.json_rows(LEAF), &["subsurface_opacity"]);
    assert_eq!(
        rows,
        [row_of(
            JsonGroup::Pgp,
            "subsurface_opacity",
            "Subsurface opacity",
            json!(1.0)
        )]
    );
    m.apply_copy_set_json("leaf", false, &[ODD.to_string()], &rows);
    idle(&mut m);
    assert_eq!(cs_record(&m, "OddTXST")["subsurfaceOpacity"], json!(1.0));
}

#[test]
fn a_mapped_colour_row_carries_the_source_writers_value() {
    let d = tempfile::tempdir().unwrap();
    let mut m = export_ready(d.path());
    let hot = json!([12.0, 0.5, 0.0]);
    // Rock keeps the colour in a CS texture set, leaf as a PGPatcher key; odd takes both in turn.
    m.add_cs_block(ROCK, Kind::Txst);
    m.set_cs_value(ROCK, Kind::Txst, "subsurfaceColor", Some(hot.clone()));
    set(&mut m, LEAF, "subsurface", json!(true));
    set(&mut m, LEAF, "subsurface_color", hot.clone());
    m.add_cs_block(ODD, Kind::Txst);
    m.set_cs_edids(ODD, Kind::Txst, &["OddTXST".to_string()]);
    idle(&mut m);
    // A CS-native colour keeps its value.
    let from_cs = pick(m.json_rows(ROCK), &["subsurface_color"]);
    assert_eq!(from_cs.len(), 1, "{from_cs:?}");
    assert_eq!(from_cs[0].value, hot);
    m.apply_copy_set_json("rock", false, &[ODD.to_string()], &from_cs);
    idle(&mut m);
    assert_eq!(
        cs_record(&m, "OddTXST")["subsurfaceColor"],
        json!([12.0, 0.5, 0.0])
    );
    // A PGPatcher colour is what its writer emits: clamped to 0 to 10.
    let from_pgp = pick(m.json_rows(LEAF), &["subsurface_color"]);
    assert_eq!(from_pgp.len(), 1, "{from_pgp:?}");
    assert_eq!(from_pgp[0].value, json!([10.0, 0.5, 0.0]));
    m.apply_copy_set_json("leaf", false, &[ODD.to_string()], &from_pgp);
    idle(&mut m);
    assert_eq!(
        cs_record(&m, "OddTXST")["subsurfaceColor"],
        json!([10.0, 0.5, 0.0])
    );
    assert_eq!(
        leaf_json_entry(&mut m)["subsurface_color"],
        json!([10.0, 0.5, 0.0]),
        "the source's own record says the same"
    );
}

/// SM-O2: a copy never creates a block the target lacks, and each skipped row says why.
#[test]
fn a_copy_never_creates_a_block_and_says_why() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    m.add_cs_block(ROCK, Kind::Txst);
    m.set_cs_value(ROCK, Kind::Txst, "fuzzWeight", Some(json!(0.5)));
    m.add_cs_block(ROCK, Kind::Mato);
    m.set_cs_value(ROCK, Kind::Mato, "roughness", Some(json!(0.4)));
    m.set_cm_value(ROCK, "glossiness", Some(json!(50)));
    m.edit_docs("seed an XPMF block", None, false, |_, s| {
        s.set_xpmf(ROCK, json!({"shelterFade": 32}).as_object().cloned())
    });
    idle(&mut m);
    let rows = m.json_rows(ROCK);
    for g in [
        JsonGroup::CsTxst,
        JsonGroup::CsMato,
        JsonGroup::Cm,
        JsonGroup::Xpmf,
    ] {
        assert!(rows.iter().any(|r| r.group == g), "a {g:?} row: {rows:?}");
    }
    let leaf = entry(&m, LEAF);
    assert!(
        leaf.is_none_or(|e| e.cs_txst.is_none()
            && e.cs_mato.is_none()
            && e.cm.is_none()
            && e.xpmf.is_none()),
        "premise: leaf has no block"
    );
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    let leaf = entry(&m, LEAF).unwrap();
    assert!(
        leaf.cs_txst.is_none()
            && leaf.cs_mato.is_none()
            && leaf.cm.is_none()
            && leaf.xpmf.is_none(),
        "no block was created: {leaf:?}"
    );
    let lines = report_lines(&m);
    for reason in [
        "has no CS PBR texture set",
        "has no CS PBR material object",
        "has no complex material JSON",
        "writes no XPMF profile",
    ] {
        let end = format!("written to 0 of 1; skipped on 1 of 1: {reason}");
        assert!(
            lines.iter().any(|l| l.ends_with(&end)),
            "{reason}: {lines:?}"
        );
    }
}

/// SM-T3-2: a row that would give the target two features that exclude each other is skipped, the reason names the
/// feature the target already uses, and nothing is written or counted as written.
fn clash_skipped(row_key: &str, target_key: &str, reason: &str) {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    set(&mut m, ROCK, row_key, json!(true));
    set(&mut m, LEAF, target_key, json!(true));
    idle(&mut m);
    let rows = pick(m.json_rows(ROCK), &[row_key]);
    assert_eq!(rows.len(), 1, "premise: rock has the {row_key} row");
    let (text, undo) = (doc_text(&m), m.history.undo.len());
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    assert_eq!(m.history.undo.len(), undo, "no undo step");
    assert_eq!(doc_text(&m), text, "the document is unchanged");
    let leaf = entry(&m, LEAF).unwrap();
    assert_eq!(leaf.settings.get(row_key), None);
    assert_eq!(leaf.settings.get(target_key), Some(&json!(true)));
    let end = format!("written to 0 of 1; skipped on 1 of 1: {reason}");
    assert!(
        report_lines(&m).iter().any(|l| l.ends_with(&end)),
        "{:?}",
        report_lines(&m)
    );
    assert!(
        m.status_line.contains("0 written, 1 skipped"),
        "{:?}",
        m.status_line
    );
}

#[test]
fn hair_is_skipped_beside_multilayer() {
    clash_skipped(
        "hair",
        "multilayer",
        "already uses multilayer, which excludes hair",
    );
}

#[test]
fn subsurface_is_skipped_beside_subsurface_foliage() {
    clash_skipped(
        "subsurface",
        "subsurface_foliage",
        "already uses subsurface_foliage, which excludes subsurface",
    );
}

#[test]
fn foliage_is_skipped_beside_subsurface_and_hair_beside_any_engine_pair() {
    clash_skipped(
        "subsurface_foliage",
        "subsurface",
        "already uses subsurface, which excludes subsurface_foliage",
    );
    // `blocked_by` names the pair: hair excludes emissive too.
    clash_skipped(
        "hair",
        "emissive",
        "already uses emissive, which excludes hair",
    );
}

/// SM-T3-2: the rows of one apply meet each other's switches, so two rows that exclude each other never both land.
#[test]
fn two_exclusive_rows_of_one_apply_do_not_both_land() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    set(&mut m, ROCK, "hair", json!(true));
    set(&mut m, ROCK, "multilayer", json!(true));
    idle(&mut m);
    let rows = pick(m.json_rows(ROCK), &["hair", "multilayer"]);
    assert_eq!(rows.len(), 2, "premise: {rows:?}");
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    let leaf = entry(&m, LEAF).unwrap();
    assert_eq!(leaf.settings.get("hair"), Some(&json!(true)));
    assert_eq!(leaf.settings.get("multilayer"), None);
    let lines = report_lines(&m);
    assert!(
        lines[0].ends_with("written to 1 of 1"),
        "hair lands first: {lines:?}"
    );
    assert!(
        lines[1].ends_with("skipped on 1 of 1: already uses hair, which excludes multilayer"),
        "{lines:?}"
    );
}

/// SM-T3-3 minor 2: a report line starts with its group, so rows of one label in two groups can be told apart.
#[test]
fn a_report_line_names_its_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    set(&mut m, ROCK, "roughness_scale", json!(0.8));
    idle(&mut m);
    let rows = m.json_rows(ROCK);
    m.apply_copy_set_json("rock", false, &[LEAF.to_string()], &rows);
    idle(&mut m);
    let line = "PGPatcher JSON: Roughness scale: written to 1 of 1";
    assert_eq!(report_lines(&m), [line]);
    assert!(logged(&m).contains(&line.to_string()), "{:?}", logged(&m));
}

/// SM-T3-3 minor 3: one toast per apply, however many rows skip; the per-row lines go to the log only.
#[test]
fn one_apply_raises_one_summary_toast_and_logs_each_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    set(&mut m, ROCK, "roughness_scale", json!(0.8));
    set(&mut m, ROCK, "emissive", json!(true));
    idle(&mut m);
    let rows = m.json_rows(ROCK);
    assert_eq!(rows.len(), 2, "{rows:?}");
    let (toasts, logs) = (
        m.messages.toasts().len() + m.messages.queued(),
        m.messages.entries().count(),
    );
    m.apply_copy_set_json("rock", false, &[LEAF.to_string(), ODD.to_string()], &rows);
    idle(&mut m);
    assert_eq!(
        m.messages.toasts().len() + m.messages.queued(),
        toasts + 1,
        "one toast"
    );
    assert_eq!(
        m.messages.entries().count(),
        logs + 3,
        "two row lines and the summary: {:?}",
        logged(&m)
    );
    let summary = "Copy JSON settings from rock: 2 written, 2 skipped (see Messages)";
    assert!(logged(&m).iter().any(|l| l == summary), "{:?}", logged(&m));
}

/// SM-T3-3 minor 9: a target with no view is reported and counted in "of n", never dropped silently.
#[test]
fn a_dropped_target_is_reported_and_counted() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_sets(d.path());
    set(&mut m, ROCK, "roughness_scale", json!(0.8));
    idle(&mut m);
    let rows = m.json_rows(ROCK);
    let to = [LEAF.to_string(), "landscape/gone".to_string()];
    m.apply_copy_set_json("rock", false, &to, &rows);
    idle(&mut m);
    assert_eq!(
        report_lines(&m),
        ["PGPatcher JSON: Roughness scale: written to 1 of 2; skipped on 1 of 2: is no longer in the project"]
    );
    assert_eq!(
        entry(&m, LEAF).unwrap().settings.get("roughness_scale"),
        Some(&json!(0.8))
    );
}

/// SM-T3-3 minor 8: a skin write and another group's write of one apply share the one undo step.
#[test]
fn a_skin_write_and_another_groups_write_share_one_undo_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), &["a", "b"]);
    let (a, b) = (skin_id(&m, "x/a"), skin_id(&m, "x/b"));
    m.edit_docs("seed", None, false, |_, s| {
        s.set_value(&a, "skin.Roughness", Some(json!(0.5)));
        s.set_xpmf(&a, json!({"shelterFade": 32}).as_object().cloned());
        s.set_xpmf(&b, json!({}).as_object().cloned());
    });
    idle(&mut m);
    let rows = m.json_rows(&a);
    for g in [JsonGroup::Skin, JsonGroup::Xpmf] {
        assert!(rows.iter().any(|r| r.group == g), "a {g:?} row: {rows:?}");
    }
    let (before, undo) = (doc_text(&m), m.history.undo.len());
    m.apply_copy_set_json("a", false, std::slice::from_ref(&b), &rows);
    idle(&mut m);
    let e = entry(&m, &b).unwrap();
    assert_eq!(e.skin.get("Roughness"), Some(&json!(0.5)));
    assert_eq!(
        e.xpmf.as_ref().and_then(|x| x.get("shelterFade")),
        Some(&json!(32))
    );
    assert_eq!(m.history.undo.len(), undo + 1, "one step for both groups");
    m.undo();
    assert_eq!(doc_text(&m), before);
}
