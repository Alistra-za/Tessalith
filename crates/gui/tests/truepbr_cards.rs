//! Note FM33: the TruePBR editors show a feature's card only while the feature is on, or while the JSON still holds its
//! values (then with "kept, not used"). Parallax has no card: its Displacement scale is a row of Material. Each rule
//! runs against the set editor's TruePBR panel and the PGPatcher JSON editor.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::config::Subtype;
use sdc_gui::model::AppModel;
use sdc_gui::ui::{json_editor, pgp_fields};
use serde_json::{json, Value};
use support::*;

const SET: &str = "landscape/rock01";
const KEPT: &str = "kept, not used";
/// The cards a feature gets while it is on (Parallax has none).
const FEATURE_CARDS: [&str; 5] = [
    "Coat",
    "Emissive settings",
    "Glint settings",
    "Fuzz settings",
    "Parallax settings",
];

fn model(dir: &std::path::Path, edits: &[(&str, Value)]) -> AppModel {
    let mut m = pbr_opened(dir);
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with(SET))
        .unwrap()
        .key
        .clone();
    m.select_set(&key);
    for (k, v) in edits {
        m.set_json_value(SET, k, Some(v.clone()));
    }
    m
}

fn panel(m: AppModel) -> Harness<'static, AppModel> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(700.0, 1400.0))
        .build_ui_state(|ui, m: &mut AppModel| pgp_fields::panel(ui, m, SET), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h
}

/// The JSON editor with one entry (`rock01`; the other set is a skin set, which has none).
fn editor(dir: &std::path::Path, edits: &[(&str, Value)]) -> Harness<'static, AppModel> {
    let mut m = pbr_opened(dir);
    m.decide(
        "landscape/leaf.png",
        sdc_gui::model::Answer::Subtype(Subtype::SkinAlbedo),
    );
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.json_editor_open = true;
    for (k, v) in edits {
        m.set_json_value(SET, k, Some(v.clone()));
    }
    let mut h = Harness::builder()
        .with_size(egui::vec2(1200.0, 1800.0))
        .build_ui_state(|ui, m: &mut AppModel| json_editor::view(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    assert_eq!(h.query_all_by_label("+ key").count(), 1, "one open entry");
    h
}

fn stored(h: &Harness<'static, AppModel>, key: &str) -> Option<Value> {
    h.state()
        .sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|&e| e.id == SET)
        .cloned()?
        .settings
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
}

fn has(h: &Harness<'static, AppModel>, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

/// The entry as the PGPatcher JSON writes it.
fn written(h: &Harness<'static, AppModel>) -> Value {
    let e = sdc_gui::model::pbr_export::json_entries(h.state())
        .into_iter()
        .find(|(v, _)| v.id == SET)
        .unwrap()
        .1;
    sdc_gui::pgp::writer::document(&[e]).to_value()["entries"][0].clone()
}

fn focus_spin(h: &mut Harness<'static, AppModel>, shown: &str) {
    h.query_all_by_role(Role::SpinButton)
        .find(|n| n.value().as_deref() == Some(shown))
        .unwrap_or_else(|| panic!("no number box shows {shown}"))
        .focus();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run();
}

fn type_value(h: &mut Harness<'static, AppModel>, shown: &str, text: &str) {
    focus_spin(h, shown);
    h.query_all_by_role(Role::SpinButton)
        .next()
        .unwrap()
        .type_text(text);
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
}

#[test]
fn only_active_feature_cards_show() {
    let d = tempfile::tempdir().unwrap();
    // The set editor: Parallax on, nothing else.
    let h = panel(model(d.path(), &[("parallax", json!(true))]));
    for t in FEATURE_CARDS {
        assert!(!has(&h, t), "no {t} card");
    }
    h.get_by_label("Material");
    h.get_by_label("Displacement scale");
    // Subsurface and NiAlphaProperty hold their own switch (the selector, the flag boxes): they stay, closed.
    h.get_by_label("Subsurface");
    h.get_by_label("NiAlphaProperty");
    assert!(!has(&h, "Override flags"), "NiAlphaProperty is closed");

    // The JSON editor draws no cards; it lists the keys the entry has.
    let d = tempfile::tempdir().unwrap();
    let h = editor(d.path(), &[("parallax", json!(true))]);
    for t in FEATURE_CARDS.into_iter().chain([
        "Subsurface",
        "NiAlphaProperty",
        "Glint",
        "Fuzz",
        "Emissive",
    ]) {
        assert!(!has(&h, t), "no {t} in the JSON editor");
    }
    h.get_by_label("Parallax");

    // Every feature off and nothing stored: no feature card in either editor (Parallax's row is absent too).
    let d = tempfile::tempdir().unwrap();
    let h = panel(model(d.path(), &[]));
    for t in FEATURE_CARDS {
        assert!(!has(&h, t), "no {t} card with every feature off");
    }
    assert!(!has(&h, "Displacement scale") && !has(&h, KEPT));
    let d = tempfile::tempdir().unwrap();
    let h = editor(d.path(), &[("roughness_scale", json!(0.5))]);
    for t in FEATURE_CARDS {
        assert!(
            !has(&h, t),
            "no {t} in the JSON editor with every feature off"
        );
    }
    assert!(!has(&h, "Displacement scale") && !has(&h, KEPT));
}

/// A feature that is on but blocked (Glint while Multilayer is on) keeps its card with the blocked header; stored
/// values show their rows and the line, and none show without values.
#[test]
fn a_blocked_feature_with_stored_values_shows_rows_and_the_line() {
    let d = tempfile::tempdir().unwrap();
    let mut h = panel(model(
        d.path(),
        &[
            ("multilayer", json!(true)),
            ("glint_on", json!(true)),
            ("glint.screen_space_scale", json!(2.0)),
        ],
    ));
    h.get_by_label("Glint settings");
    h.get_by_label(KEPT);
    h.get_by_label("Screen-space scale");
    assert_eq!(stored(&h, "glint.screen_space_scale"), Some(json!(2.0)));
    assert!(written(&h).get("glint").is_none(), "blocked: not written");
    h.state_mut()
        .set_json_value(SET, "glint.screen_space_scale", None);
    h.run();
    h.get_by_label("Glint settings");
    assert!(!has(&h, KEPT), "no stored value, no line");
}

#[test]
fn ticking_glint_shows_its_card() {
    let d = tempfile::tempdir().unwrap();
    let mut h = panel(model(d.path(), &[]));
    assert!(!has(&h, "Glint settings"));
    h.get_by_label("Glint").click();
    h.run();
    h.get_by_label("Glint settings");
    h.get_by_label("Screen-space scale");
    assert_eq!(stored(&h, "glint_on"), Some(json!(true)));
    h.get_by_label("Glint").click();
    h.run();
    assert!(!has(&h, "Glint settings"), "unticking hides it again");

    // The JSON editor: the + key menu adds the Glint key, which then has its own pill and offers the glint values.
    let d = tempfile::tempdir().unwrap();
    let mut h = editor(d.path(), &[("parallax", json!(true))]);
    h.get_by_label("+ key").click();
    h.run();
    h.get_by_label_contains("Glint").click();
    h.run();
    assert_eq!(stored(&h, "glint_on"), Some(json!(true)));
    assert!(!has(&h, "Glint settings"), "the editor has no cards");
    h.get_by_label("+ key").click();
    h.run();
    h.get_by_label("Screen-space scale");
}

#[test]
fn parallax_puts_displacement_scale_under_roughness_scale() {
    let d = tempfile::tempdir().unwrap();
    let mut h = panel(model(d.path(), &[]));
    assert!(
        !has(&h, "Displacement scale"),
        "no row while Parallax is off"
    );
    h.get_by_label("Parallax").click();
    h.run();
    assert!(!has(&h, "Parallax settings"), "Parallax has no card");
    let rough = h.get_by_label("Roughness scale").rect();
    let disp = h.get_by_label("Displacement scale").rect();
    let features = h.get_by_label("Features").rect();
    assert!(
        disp.min.y > rough.max.y - 1.0 && (disp.min.x - rough.min.x).abs() < 1.0,
        "under Roughness scale: {rough:?} {disp:?}"
    );
    assert!(disp.max.y < features.min.y, "inside the Material card");
    // The row edits the same key as the old card did, in the same number format.
    h.state_mut()
        .set_json_value(SET, "displacement_scale", Some(json!(0.4)));
    h.run();
    assert!(h.query_all_by_label("reset").next().is_some());
    type_value(&mut h, "0.4", "0.7");
    assert_eq!(stored(&h, "displacement_scale"), Some(json!(0.7)));
    assert_eq!(written(&h)["displacement_scale"], json!(0.7));
    h.get_by_label("Parallax").click();
    h.run();
    assert!(!has(&h, "Parallax settings"));

    // The JSON editor keeps its own pill for the key.
    let d = tempfile::tempdir().unwrap();
    let mut h = editor(
        d.path(),
        &[
            ("parallax", json!(true)),
            ("displacement_scale", json!(0.4)),
        ],
    );
    assert!(!has(&h, "Parallax settings"));
    h.get_by_label_contains("Displacement scale");
    type_value(&mut h, "0.4", "0.7");
    assert_eq!(stored(&h, "displacement_scale"), Some(json!(0.7)));
    assert_eq!(written(&h)["displacement_scale"], json!(0.7));
}

#[test]
fn a_feature_off_with_stored_values_says_kept_not_used() {
    // Glint: the card stays, with the line; the values stay as they were.
    let values = [
        ("glint.screen_space_scale", json!(2.0)),
        ("glint.microfacet_roughness", json!(0.3)),
    ];
    let mut edits = vec![("glint_on", json!(true))];
    edits.extend(values.clone());
    let d = tempfile::tempdir().unwrap();
    let mut h = panel(model(d.path(), &edits));
    assert!(!has(&h, KEPT));
    h.get_by_label("Glint").click();
    h.run();
    h.get_by_label("Glint settings");
    h.get_by_label(KEPT);
    h.get_by_label("Screen-space scale");
    h.get_by_label("Microfacet roughness");
    for (k, v) in &values {
        assert_eq!(stored(&h, k).as_ref(), Some(v), "{k} is kept as written");
    }
    assert!(written(&h).get("glint").is_none(), "off: nothing written");
    // The rows reset: the first reset removes its value only, and the writer still writes no glint.
    h.query_all_by_label("reset").next().unwrap().click();
    h.run();
    assert_eq!(stored(&h, "glint.screen_space_scale"), None);
    assert_eq!(
        stored(&h, "glint.microfacet_roughness"),
        Some(json!(0.3)),
        "the other value is untouched"
    );
    assert!(written(&h).get("glint").is_none());
    h.state_mut()
        .set_json_value(SET, "glint.screen_space_scale", Some(json!(2.0)));
    h.run();
    h.get_by_label("Glint").click();
    h.run();
    assert!(!has(&h, KEPT));
    h.get_by_label("Screen-space scale");

    // Parallax: the Displacement scale row stays, with the line.
    let d = tempfile::tempdir().unwrap();
    let mut h = panel(model(
        d.path(),
        &[
            ("parallax", json!(true)),
            ("displacement_scale", json!(0.4)),
        ],
    ));
    assert!(!has(&h, KEPT));
    h.get_by_label("Parallax").click();
    h.run();
    h.get_by_label("Displacement scale");
    h.get_by_label(KEPT);
    assert_eq!(stored(&h, "displacement_scale"), Some(json!(0.4)));
    assert!(written(&h).get("displacement_scale").is_none());

    // Subsurface (its colour is a field of the card) and a card that never had values: nothing to keep.
    let d = tempfile::tempdir().unwrap();
    let h = panel(model(
        d.path(),
        &[("subsurface_color", json!([0.5, 0.5, 0.5]))],
    ));
    h.get_by_label(KEPT);
    let d = tempfile::tempdir().unwrap();
    let h = panel(model(d.path(), &[("multilayer", json!(true))]));
    assert!(!has(&h, KEPT));

    // The JSON editor: the key's pill says so.
    let d = tempfile::tempdir().unwrap();
    let h = editor(d.path(), &[("displacement_scale", json!(0.4))]);
    h.get_by_label(KEPT);
    let d = tempfile::tempdir().unwrap();
    let h = editor(
        d.path(),
        &[
            ("parallax", json!(true)),
            ("displacement_scale", json!(0.4)),
        ],
    );
    assert!(!has(&h, KEPT));
}
