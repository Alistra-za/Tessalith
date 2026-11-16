//! Note JI3: the preset inspector is a folded section at the bottom of the texture panel ("Where these settings come
//! from"), in the centre and the 3D settings window, not in the preview strip.
mod support;
use eframe::egui::{self, accesskit::Role, accesskit::Toggled, Rect};
use egui_kittest::kittest::{NodeT as _, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::inspector::*;
use sdc_gui::model::AppModel;
use std::time::Duration;
use support::*;

const HEAD: &str = "Where these settings come from";
const ONLY: &str = "Show only what differs from sdc-base";

fn leaf(dir: &std::path::Path) -> AppModel {
    let mut m = opened(dir);
    m.select_item("landscape/leaf.png");
    m
}

fn harness(m: AppModel, w: f32) -> Harness<'static, AppModel> {
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(w, 1400.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    h
}

fn centre(h: &Harness<'static, AppModel>) -> Rect {
    sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn")
}

/// Unfold the section.
fn open_section(h: &mut Harness<'static, AppModel>) {
    h.get_by_label(HEAD).scroll_to_me();
    h.run();
    h.get_by_label(HEAD).click();
    h.run();
}

#[test]
fn the_inspector_shows_provenance_and_differences_from_the_base() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    assert_eq!(m.batch_preset, "cs-quality");
    m.set_field(
        "landscape/leaf.png",
        "alpha.threshold",
        serde_json::json!(90),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    let job = m.item("landscape/leaf.png").unwrap().job.clone().unwrap();
    let reg = m.session.as_ref().unwrap().engine().registry().clone();
    let rows = inspector_rows(&job, &reg);
    let t = rows.iter().find(|r| r.field == "alpha.threshold").unwrap();
    assert_eq!(t.provenance, "project_file");
    assert!(t.differs_from_base);
    assert!(rows.iter().any(|r| !r.differs_from_base));
}

#[test]
fn the_inspector_is_folded_at_the_bottom_of_the_texture_panel() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(leaf(d.path()), 1280.0);
    let c = centre(&h);
    let head = h.get_by_label(HEAD);
    assert!(c.contains(head.rect().center()));
    assert!(head.rect().top() > h.get_by_label("Compression").rect().top());
    assert!(h.query_by_label(ONLY).is_none(), "closed by default");
    open_section(&mut h);
    h.get_by_label(ONLY);
    // The heading names the texture (D10, A18).
    h.get_by_label("How leaf.png's settings resolve");
    assert!(h.query_by_label_contains("Batch preset").is_none());
    assert!(h
        .query_all_by_label("Presets…")
        .all(|n| !c.contains(n.rect().center())));
}

#[test]
fn the_differences_tick_survives_a_new_selection() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(leaf(d.path()), 1280.0);
    open_section(&mut h);
    assert_eq!(
        h.get_by_label(ONLY).accesskit_node().toggled(),
        Some(Toggled::False)
    );
    h.get_by_label(ONLY).click();
    h.run();
    assert_eq!(
        h.get_by_label(ONLY).accesskit_node().toggled(),
        Some(Toggled::True)
    );
    h.state_mut().select_item("landscape/rock01.png");
    h.run();
    h.get_by_label("How rock01.png's settings resolve");
    assert_eq!(
        h.get_by_label(ONLY).accesskit_node().toggled(),
        Some(Toggled::True),
        "one value for the app"
    );
}

#[test]
fn the_preview_strip_has_no_inspector() {
    let d = tempfile::tempdir().unwrap();
    let mut m = leaf(d.path());
    m.expand("landscape/leaf.png");
    assert!(m.wait_idle(Duration::from_secs(60)));
    let h = harness(m, 1600.0);
    let strip = support::panel(&h.ctx, "preview-settings");
    // The strip is drawn: it holds the texture's Compression group, and no section.
    assert!(h
        .query_all_by_label("Compression")
        .any(|n| strip.contains(n.rect().center())));
    assert!(h.query_by_label(HEAD).is_none());
}

/// D10: the first column reads in words; the key rides in the AccessKit name and on hover.
#[test]
fn the_inspector_shows_labels_with_the_key_on_hover() {
    let d = tempfile::tempdir().unwrap();
    let model = leaf(d.path());
    let job = model
        .item("landscape/leaf.png")
        .unwrap()
        .job
        .clone()
        .unwrap();
    let reg = model.session.as_ref().unwrap().engine().registry().clone();
    let rows = inspector_rows(&job, &reg);
    let labelled: Vec<_> = rows
        .iter()
        .filter(|r| sdc_gui::model::bulk::field_label(&r.field) != r.field)
        .collect();
    assert!(!labelled.is_empty(), "the inspector lists a labelled field");
    let mut h = harness(model, 1400.0);
    open_section(&mut h);
    for r in &labelled {
        let label = sdc_gui::model::bulk::field_label(&r.field);
        h.get_by_label(&format!("{label} ({})", r.field));
        assert!(
            h.query_by_label(&r.field).is_none(),
            "the raw key {} is shown as text",
            r.field
        );
    }
    // A dotted key with no label shows the readable form, never the key.
    let unlabelled: Vec<_> = rows
        .iter()
        .filter(|r| r.field.contains('.') && sdc_gui::model::bulk::field_label(&r.field) == r.field)
        .collect();
    assert!(
        !unlabelled.is_empty(),
        "the inspector lists an unlabelled dotted key"
    );
    for r in &unlabelled {
        let words = sdc_gui::model::labels::readable(&r.field.replace('.', "_"));
        h.get_by_label(&format!("{words} ({})", r.field));
        assert!(
            h.query_by_label(&r.field).is_none(),
            "{} shown raw",
            r.field
        );
    }
    // The hover carries the key, once: a tooltip node with the name appears beside the row's own.
    let r = labelled[0];
    let name = format!(
        "{} ({})",
        sdc_gui::model::bulk::field_label(&r.field),
        r.field
    );
    assert_eq!(h.query_all_by_label(&name).count(), 1);
    h.get_by_label(&name).hover();
    h.run();
    assert_eq!(
        h.query_all_by_label(&name).count(),
        2,
        "one tooltip with the key"
    );
}

/// `leaf` with a user preset whose id and rule glob are long, chosen as the project's preset, so the provenance
/// column carries long text ("from preset …", "from rule …").
fn long_text_model(dir: &std::path::Path) -> AppModel {
    // A long folder makes the shown preset path long on every machine, not only where the temp dir is (CI's is longer).
    let presets = dir.join("a-user-presets-folder-with-a-long-name-on-purpose");
    std::fs::create_dir_all(&presets).unwrap();
    std::fs::write(
        presets.join("long.toml"),
        "schema_version = 1\nid = \"a-preset-with-a-very-long-identifier-indeed\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n\n[[rules]]\nmatch = { glob = \"**/landscape/leaf.png\" }\noverrides = { resize = \"half\" }\n",
    )
    .unwrap();
    let mut model = opened(dir);
    model.user_presets = Some(presets);
    model.replan();
    assert!(model.wait_idle(Duration::from_secs(60)));
    model.set_preset(Some("a-preset-with-a-very-long-identifier-indeed".into()));
    assert!(model.wait_idle(Duration::from_secs(60)));
    model.select_item("landscape/leaf.png");
    model
}

/// D10, rule 10.3: no inspector label ends past the centre's edge, at 1100 px, with a long preset id in the
/// provenance column. Uses the unclipped label rect (AccessKit's), against `centre_rect`.
#[test]
fn no_inspector_value_passes_the_centre_edge() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(long_text_model(d.path()), 1100.0);
    open_section(&mut h);
    let edge = centre(&h).max.x;
    let top = h
        .get_by_label("How leaf.png's settings resolve")
        .rect()
        .min
        .y;
    let mut checked = 0;
    for n in h.query_all_by_role(Role::Label) {
        let r = n.rect();
        if r.min.y > top && r.min.x < edge {
            checked += 1;
            assert!(
                r.max.x <= edge + 1.0,
                "{:?} ends at {} past the centre edge {edge}",
                n.accesskit_node().label().or(n.accesskit_node().value()),
                r.max.x
            );
        }
    }
    assert!(checked > 10, "only {checked} labels checked");
}

/// Final review Important 1: a user preset file that fails to load is shown in the inspector with its reason
/// (CFG-PRESET-SKIPPED). The list of loaded presets is gone (Note JI3): the Presets tab lists them.
#[test]
fn the_inspector_lists_skipped_user_presets_with_their_reason() {
    let d = tempfile::tempdir().unwrap();
    let presets = d.path().join("user-presets");
    std::fs::create_dir_all(&presets).unwrap();
    std::fs::write(
        presets.join("mine.toml"),
        "schema_version = 1\nid = \"mine\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n",
    )
    .unwrap();
    std::fs::write(
        presets.join("bad.toml"),
        "schema_version = 1\nid = \"bad\"\nversion = \"1.0.0\"\nbase = \"nope\"\n",
    )
    .unwrap();
    let mut m = opened(d.path());
    m.user_presets = Some(presets.clone());
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.loaded_user_presets.len(), 1);
    assert_eq!(m.skipped_user_presets.len(), 1);
    assert!(m.skipped_user_presets[0].0.ends_with("bad.toml"));
    assert!(m.skipped_user_presets[0]
        .1
        .contains("unknown preset 'nope'"));
    m.select_item("landscape/leaf.png");
    let mut h = harness(m, 1400.0);
    open_section(&mut h);
    h.get_by_label_contains("bad.toml");
    h.get_by_label_contains("unknown preset 'nope'");
    assert!(
        h.query_by_label_contains("mine.toml").is_none(),
        "the loaded list is gone"
    );
}
