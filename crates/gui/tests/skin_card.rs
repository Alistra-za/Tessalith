//! Task 10b: the Advanced skin card (owner, 2026-10-14). Per-set, non-inheritable CS SkinProfile values kept in
//! the set's `skin` table of `sdc-project.toml`; the export writes them as the `.csskin.json`.
mod support;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::config::Subtype;
use sdc_gui::model::skin::FIELDS;
use sdc_gui::model::{Answer, AppModel};
use sdc_gui::sets_doc::SetsDoc;
use serde_json::json;
use std::time::Duration;
use support::*;

const SKIN: &str = "landscape/rock01";
const TITLE: &str = "Advanced skin (Community Shaders)";

fn skin_opened(dir: &std::path::Path) -> AppModel {
    let mut m = opened(dir);
    m.decide("landscape/rock01.png", Answer::Subtype(Subtype::SkinAlbedo));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

fn set_key(m: &AppModel) -> String {
    sdc_gui::model::sets::set_views(m)
        .into_iter()
        .find(|v| v.id == SKIN)
        .unwrap()
        .key
}

fn harness(m: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 1400.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

#[test]
fn the_field_list_is_the_twenty_two_cs_keys() {
    let keys: Vec<&str> = FIELDS.iter().map(|f| f.key).collect();
    assert_eq!(
        keys,
        [
            "Roughness",
            "SecondaryRoughness",
            "SpecularTextureMultiplier",
            "SecondarySpecularStrength",
            "Reflectance",
            "BaseColorMultiplier",
            "PhysicalMainRoughnessMultiplier",
            "PhysicalSecondRoughnessMultiplier",
            "PhysicalSpecularStrength",
            "ExtraEdgeRoughness",
            "Fuzz",
            "FuzzRoughness",
            "FuzzF0",
            "TransmissionEnabled",
            "Transmission",
            "TransmissionDepth",
            "SSSAmount",
            "DetailEnabled",
            "DetailStrength",
            "DetailTiling",
            "BodyTilingMultiplier",
            "WetResponse",
        ]
    );
    assert!(sdc_gui::model::skin::skin_field("Reflectance").is_some());
    assert!(sdc_gui::model::skin::skin_field("F0").is_none());
}

#[test]
fn the_card_shows_for_a_skin_set_and_not_for_a_plain_one() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    let k = set_key(&m);
    m.select_set(&k);
    let mut h = harness(m);
    h.run();
    h.get_by_label_contains(TITLE);
    h.get_by_label_contains(".csskin.json");
    assert!(
        h.query_by_label("Export JSON…").is_none(),
        "the card has no export button"
    );

    let d2 = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d2.path());
    let k = set_key(&m);
    m.select_set(&k);
    let mut h = harness(m);
    h.run();
    assert!(h.query_by_label_contains(TITLE).is_none());
}

/// Audit lead 24: a read-only `sdc-project.toml` greys the card and says why on hover.
#[test]
fn a_read_only_project_greys_the_card_with_its_reason() {
    use egui_kittest::kittest::NodeT;
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    let bad = d.path().join("bad");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("sdc-project.toml"), "[[sets]\nbroken").unwrap();
    let k = set_key(&m);
    m.sets_doc = Some(SetsDoc::open(&bad));
    m.select_set(&k);
    let mut h = harness(m);
    h.run();
    const WHY: &str = "sdc-project.toml is read-only";
    assert!(
        h.query_by_label_contains(WHY).is_none(),
        "the reason is not shown before the hover"
    );
    let set = h.get_by_role_and_label(Role::Button, "Set Roughness");
    assert!(set.accesskit_node().is_disabled());
    set.hover();
    h.run();
    h.run();
    assert!(
        h.query_by_label_contains(WHY).is_some(),
        "the reason shows on hover"
    );
}

#[test]
fn an_unset_field_shows_its_default_and_stores_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    assert!(m.skin_values(SKIN).is_empty());
    let k = set_key(&m);
    m.select_set(&k);
    let mut h = harness(m);
    h.run();
    h.get_by_label("default 0.7");
    h.get_by_label("default 0.0278");
    h.get_by_label("Reflectance");
    h.get_by_label("Secondary roughness");
    h.get_by_label("SSS amount");
    assert!(h.state().skin_values(SKIN).is_empty());
}

#[test]
fn setting_resetting_and_one_undo_step_each() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    m.set_skin_value(SKIN, "Reflectance", Some(json!(0.04)));
    m.set_skin_value(SKIN, "TransmissionEnabled", Some(json!(false)));
    assert_eq!(m.skin_values(SKIN)["Reflectance"], json!(0.04));
    m.undo();
    assert!(!m.skin_values(SKIN).contains_key("TransmissionEnabled"));
    assert_eq!(m.skin_values(SKIN)["Reflectance"], json!(0.04));
    m.set_skin_value(SKIN, "Reflectance", None);
    assert!(m.skin_values(SKIN).is_empty(), "reset clears it");
    m.undo();
    assert_eq!(m.skin_values(SKIN)["Reflectance"], json!(0.04));
    m.undo();
    assert!(
        m.skin_values(SKIN).is_empty(),
        "the set itself was one step"
    );
}

#[test]
fn the_skin_table_round_trips_and_unknown_keys_are_removed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    m.set_skin_value(SKIN, "Reflectance", Some(json!(0.04)));
    m.sets_doc
        .as_mut()
        .unwrap()
        .set_value(SKIN, "skin.Future", Some(json!("x")));
    m.set_skin_value(SKIN, "SSSAmount", Some(json!(0.3)));
    let root = m.root.clone().unwrap();
    m.sets_doc.as_mut().unwrap().save(true).unwrap();
    let s = SetsDoc::open(&root);
    let e = s.entries().iter().find(|&e| e.id == SKIN).cloned().unwrap();
    assert_eq!(e.skin["Reflectance"], json!(0.04));
    assert_eq!(e.skin["SSSAmount"], json!(0.3));
    assert!(
        !e.skin.contains_key("Future"),
        "an unknown key goes with the next edit"
    );
    assert!(
        e.settings.keys().all(|k| !k.starts_with("skin")),
        "skin values are not PGPatcher settings: {:?}",
        e.settings
    );
}

#[test]
fn the_set_button_stores_the_default_and_the_reset_clears_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    let k = set_key(&m);
    m.select_set(&k);
    let mut h = harness(m);
    h.run();
    // Note FB2: the card is below the slot rows in the scrolling set editor; bring the button into view.
    // Every unset row has its own named button; none is a bare "set".
    assert!(h.query_by_label("set").is_none());
    h.get_by_role_and_label(Role::Button, "Set Roughness")
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::Button, "Set Roughness")
        .click();
    h.run();
    assert_eq!(h.state().skin_values(SKIN)["Roughness"], json!(0.7));
    // The value input and the reset are named after the same row.
    h.get_by_role_and_label(Role::SpinButton, "Roughness");
    assert!(h.query_by_label("↺").is_none());
    h.get_by_role_and_label(Role::Button, "Reset Roughness")
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::Button, "Reset Roughness")
        .click();
    h.run();
    assert!(h.state().skin_values(SKIN).is_empty());
}

#[test]
fn a_flag_row_has_a_named_checkbox_that_stores_its_value() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    let k = set_key(&m);
    m.select_set(&k);
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(Role::Button, "Set Transmission enabled")
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::Button, "Set Transmission enabled")
        .click();
    h.run();
    assert_eq!(
        h.state().skin_values(SKIN)["TransmissionEnabled"],
        json!(true)
    );
    h.get_by_role_and_label(Role::CheckBox, "Transmission enabled")
        .click();
    h.run();
    assert_eq!(
        h.state().skin_values(SKIN)["TransmissionEnabled"],
        json!(false)
    );
}

#[test]
fn unknown_skin_entries_of_any_shape_are_removed_on_edit() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    m.set_skin_value(SKIN, "Reflectance", Some(json!(0.04)));
    let root = m.root.clone().unwrap();
    m.sets_doc.as_mut().unwrap().save(true).unwrap();
    let path = root.join(sdc_gui::sets_doc::FILE_NAME);
    let text = std::fs::read_to_string(&path).unwrap().replacen(
        "skin.Reflectance = 0.04",
        "skin.Reflectance = 0.04\nskin.\"Future.option\" = 1\nskin.Empty = {}\nskin.Nested.deep = 2",
        1,
    );
    assert!(text.contains("Future.option"), "{text}");
    std::fs::write(&path, &text).unwrap();
    m.open_folder(root.clone());
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_skin_value(SKIN, "Roughness", Some(json!(0.6)));
    m.sets_doc.as_mut().unwrap().save(true).unwrap();
    let out = std::fs::read_to_string(&path).unwrap();
    for gone in ["Future", "Empty", "Nested", "deep"] {
        assert!(!out.contains(gone), "{gone} survived: {out}");
    }
    assert!(out.contains("Roughness = 0.6"), "{out}");
    assert!(out.contains("Reflectance = 0.04"), "{out}");
}

// ---- Task 10 fix round 1: the 10b review carry-over and the owner's no-JSON ruling for skin sets (AG88-AG90)

#[test]
fn every_default_group_and_range_is_the_briefs_table() {
    use sdc_gui::model::skin::Kind;
    let want: [(&str, &str, Kind); 22] = [
        ("Roughness", "Specular", n(0.7, 0.0, 1.0)),
        ("SecondaryRoughness", "Specular", n(0.35, 0.0, 1.0)),
        ("SpecularTextureMultiplier", "Specular", n(1.0, 0.0, 10.0)),
        ("SecondarySpecularStrength", "Specular", n(0.15, 0.0, 1.0)),
        ("Reflectance", "Specular", n(0.0278, 0.0, 0.1)),
        ("BaseColorMultiplier", "Physical", n(1.0, 0.0, 2.0)),
        (
            "PhysicalMainRoughnessMultiplier",
            "Physical",
            n(1.3, 0.0, 2.0),
        ),
        (
            "PhysicalSecondRoughnessMultiplier",
            "Physical",
            n(0.75, 0.0, 2.0),
        ),
        ("PhysicalSpecularStrength", "Physical", n(1.0, 0.0, 2.0)),
        ("ExtraEdgeRoughness", "Edge & detail", n(0.25, 0.0, 1.0)),
        ("Fuzz", "Fuzz", n(1.0, 0.0, 2.0)),
        ("FuzzRoughness", "Fuzz", n(0.35, 0.1, 1.0)),
        ("FuzzF0", "Fuzz", n(0.045, 0.0, 0.5)),
        (
            "TransmissionEnabled",
            "Transmission",
            Kind::Flag { default: true },
        ),
        ("Transmission", "Transmission", n(0.1, 0.0, 1.0)),
        ("TransmissionDepth", "Transmission", n(0.2, 0.0, 1.0)),
        ("SSSAmount", "Transmission", n(1.0, 0.0, 1.0)),
        (
            "DetailEnabled",
            "Edge & detail",
            Kind::Flag { default: true },
        ),
        ("DetailStrength", "Edge & detail", n(0.25, -2.0, 2.0)),
        ("DetailTiling", "Edge & detail", n(10.0, 0.1, 100.0)),
        ("BodyTilingMultiplier", "Edge & detail", n(2.0, 0.1, 5.0)),
        ("WetResponse", "Wet", n(1.0, 0.0, 1.0)),
    ];
    assert_eq!(
        sdc_gui::model::skin::GROUPS,
        [
            "Specular",
            "Physical",
            "Edge & detail",
            "Transmission",
            "Fuzz",
            "Wet"
        ]
    );
    for (f, (key, group, kind)) in FIELDS.iter().zip(want) {
        assert_eq!((f.key, f.group, f.kind), (key, group, kind), "{key}");
    }
}

fn n(default: f64, lo: f64, hi: f64) -> sdc_gui::model::skin::Kind {
    sdc_gui::model::skin::Kind::Num { default, lo, hi }
}

#[test]
fn a_drag_on_one_field_is_one_undo_step_and_is_rounded() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    for x in [0.70, 0.705, 0.7100000000000001] {
        m.set_skin_value(SKIN, "Roughness", Some(json!(x)));
    }
    assert_eq!(m.skin_values(SKIN)["Roughness"], json!(0.71));
    m.undo();
    assert!(m.skin_values(SKIN).is_empty(), "the drag was one step");
}

#[test]
fn unknown_skin_keys_drop_and_comments_survive_a_real_load_and_save() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    m.set_skin_value(SKIN, "Reflectance", Some(json!(0.04)));
    let root = m.root.clone().unwrap();
    m.sets_doc.as_mut().unwrap().save(true).unwrap();
    let path = root.join(sdc_gui::sets_doc::FILE_NAME);
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.replacen(
        "skin.Reflectance = 0.04",
        "skin.Future = \"x\"\n# kept by hand\nskin.Reflectance = 0.04",
        1,
    );
    assert!(text.contains("Future"), "{text}");
    std::fs::write(&path, &text).unwrap();
    // The real load path: open the folder again.
    m.open_folder(root.clone());
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        !m.skin_values(SKIN).contains_key("Future"),
        "unknown keys are not skin values: {:?}",
        m.skin_values(SKIN)
    );
    m.set_skin_value(SKIN, "Reflectance", Some(json!(0.05)));
    m.sets_doc.as_mut().unwrap().save(true).unwrap();
    let out = std::fs::read_to_string(&path).unwrap();
    assert!(out.contains("# kept by hand"), "{out}");
    assert!(!out.contains("Future"), "dropped on the edit: {out}");
    assert!(out.contains("Reflectance = 0.05"), "{out}");
    // Not a PGPatcher setting: no JSON entry carries it.
    for (_, e) in sdc_gui::model::pbr_export::json_entries(&m) {
        assert!(
            e.settings.keys().all(|k| !k.contains("Future")),
            "{:?}",
            e.settings
        );
    }
}

#[test]
fn a_skin_set_has_no_json_entry_no_truepbr_card_and_no_export_button() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    assert!(sdc_gui::model::pbr_export::json_entries(&m).is_empty());
    assert!(sdc_gui::model::pbr_export::export_blocks(&m)
        .iter()
        .all(|b| !b.set_id.contains("rock01")));
    let k = set_key(&m);
    m.select_set(&k);
    let mut h = harness(m);
    h.run();
    h.get_by_label_contains(TITLE);
    assert!(
        h.query_by_label("Specular level").is_none(),
        "no TruePBR card"
    );
    // Layout v2 A5 (K14): the card's pill says "skin" (its hover "Advanced skin").
    h.get_by_label("skin");
    assert!(h.query_by_label("Export JSON…").is_none());

    // A PBR set is unchanged.
    let d2 = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d2.path());
    assert_eq!(sdc_gui::model::pbr_export::json_entries(&m).len(), 1);
    let k = set_key(&m);
    m.select_set(&k);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Specular level");
    h.get_by_label("PBR → JSON");
}

#[test]
fn an_old_skin_entry_drops_at_the_next_export_under_the_keep_rules() {
    use sdc_gui::model::export::{Row, RowState};
    use sdc_gui::model::pbr_export::{finish, prepare, Finish};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.decide("landscape/leaf.png", Answer::Subtype(Subtype::SkinAlbedo));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let views = sdc_gui::model::sets::set_views(&m);
    assert!(views.iter().any(|v| v.skin), "leaf is a skin set");
    let p = prepare(&m).expect("the PBR set is written");
    assert_eq!(p.entries.len(), 1, "only the PBR set");
    std::fs::create_dir_all(p.path.parent().unwrap()).unwrap();
    std::fs::write(&p.path, "{\"entries\": [\"landscape\\\\leaf skin\"]}").unwrap();
    let rows: Vec<Row> = p
        .entries
        .iter()
        .flat_map(|(_, deps, _)| deps.clone())
        .map(|job_key| Row {
            job_key,
            path: String::new(),
            state: RowState::Done { passed: true },
            warnings: vec![],
        })
        .collect();
    // Edited outside the GUI: the overwrite question, with text that has no skin entry.
    let mut notes = vec![];
    match finish(&p, &rows, None, &mut notes) {
        Finish::Overwrite(t) => assert!(!t.contains("leaf"), "{t}"),
        r => panic!("{r:?} {notes:?}"),
    }
    // Written by us last time: replaced, and the skin entry is gone.
    let last = sdc_gui::model::history::sha256_of(&p.path);
    assert!(matches!(
        finish(&p, &rows, last.as_deref(), &mut notes),
        Finish::Written(_)
    ));
    assert!(!std::fs::read_to_string(&p.path).unwrap().contains("leaf"));
}

/// Fix round 2 (re-review minors): a skin-only project writes no JSON, so an old file stays (keep rule AF1), and the
/// set preview has no JSON entry for a skin set (it draws it as a vanilla set).
#[test]
fn a_skin_only_project_keeps_the_old_json_and_previews_without_an_entry() {
    let d = tempfile::tempdir().unwrap();
    let m = skin_opened(d.path());
    assert!(
        sdc_gui::model::pbr_export::prepare(&m).is_none(),
        "nothing to write"
    );
    let fp = sdc_gui::model::set_preview::fingerprint(&m, &set_key(&m)).unwrap();
    assert!(fp.entry.is_none());
}

#[test]
fn old_skin_keys_are_dropped_on_edit_and_never_exported() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_opened(d.path());
    let d_ = m.sets_doc.as_mut().unwrap();
    d_.set_value(SKIN, "skin.SkinMainRoughness", Some(json!(0.5)));
    d_.set_value(SKIN, "skin.F0", Some(json!(0.03)));
    assert!(m.skin_values(SKIN).is_empty(), "old keys are not values");
    let k = set_key(&m);
    m.select_set(&k);
    let mut h = harness(m);
    h.run();
    h.get_by_label("default 0.7");
    let m = h.state_mut();
    m.set_skin_value(SKIN, "Roughness", Some(json!(0.6)));
    let e = m
        .sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|e| e.id == SKIN)
        .cloned()
        .unwrap();
    assert_eq!(e.skin.len(), 1, "{:?}", e.skin);
    assert_eq!(e.skin["Roughness"], json!(0.6));
    m.undo();
    assert!(m.skin_values(SKIN).is_empty());
    let raw = m
        .sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|e| e.id == SKIN)
        .cloned()
        .unwrap();
    assert_eq!(raw.skin.len(), 2, "one undo step restores the old keys");
}
