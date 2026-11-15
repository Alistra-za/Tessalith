mod support;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::AppModel;
use std::time::Duration;
use support::*;

#[test]
fn unmatched_entries_are_listed_and_removable() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_field(
        "landscape/rock01_rmaos",
        "alpha.threshold",
        serde_json::json!(90),
    );
    m.set_target(engine::config::Target::Vanilla);
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.unmatched_keys, vec!["landscape/rock01_rmaos".to_string()]);
    m.remove_unmatched("landscape/rock01_rmaos");
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.unmatched_keys.is_empty());
    assert!(!m
        .project
        .as_ref()
        .unwrap()
        .file_keys()
        .contains(&"landscape/rock01_rmaos".to_string()));
}

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 1000.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

#[test]
fn the_open_drawer_shows_its_sections() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.settings.advanced_open = true;
    model.select_item("landscape/leaf.png");
    let mut h = harness(model);
    h.run();
    assert!(
        h.query_all_by_label_contains("Unmatched [files] entries")
            .count()
            >= 1,
        "no Unmatched [files] entries in the drawer"
    );
    // Note FM25 (P17): no Batch section (threads, memory), preview-size combo, 16384 box or Interface section. The
    // drawer's own labels, read as the old exact-list test read them (the header has its own "Batch…" words).
    let panel = eframe::egui::containers::panel::PanelState::load(
        &h.ctx,
        eframe::egui::Id::new("advanced"),
    )
    .expect("the drawer is open")
    .outer_rect;
    let labels: Vec<String> = h
        .query_all_by_role(Role::Label)
        .chain(h.query_all_by_role(Role::Button))
        .chain(h.query_all_by_role(Role::CheckBox))
        .chain(h.query_all_by_role(Role::SpinButton))
        .chain(h.query_all_by_role(Role::ComboBox))
        .filter(|n| panel.expand(1.0).contains_rect(n.rect()))
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    // The positive anchor: the absence asserts below cannot pass on an empty drawer (Note JI3 took the inspector, JI1
    // the Preview, Checks and Cache sections).
    assert!(
        labels.iter().any(|l| l == "Unmatched [files] entries"),
        "the anchor is in {labels:?}"
    );
    for label in [
        "Batch",
        "Worker threads",
        "Memory budget (MiB)",
        "Largest previewed side",
        "Allow 16384 previews",
        "Interface",
        "Reduce motion",
    ] {
        assert!(
            !labels.iter().any(|l| l == label),
            "{label:?} is gone (FM25): {labels:?}"
        );
    }
    // Note JI1: the app settings are the Settings tab's, not drawn here too.
    for label in [
        "Preview",
        "Live encode",
        "Checks",
        "Low detail check",
        "Green direction check",
        "Parallax check",
        "Cache",
        "Analysis cache limit",
    ] {
        assert!(
            !labels.iter().any(|l| l == label),
            "{label:?} is the Settings tab's (JI1): {labels:?}"
        );
    }
    // Note JI3, JI4: the rows the texture panel took (its Levels, Size, DDS header and Effort rows, the last over the
    // two search fields) and the duplicate BC7 weighting row are not drawn here too. Read by the drawer's columns only:
    // the last section lies below the window, and AccessKit rects are not clipped.
    let rows: Vec<String> = h
        .query_all_by_role(Role::Label)
        .filter(|n| n.rect().min.x >= panel.min.x - 1.0 && n.rect().max.x <= panel.max.x + 1.0)
        // A label's text is its AccessKit value.
        .filter_map(|n| n.accesskit_node().label().or(n.accesskit_node().value()))
        .collect();
    assert!(
        rows.iter()
            .any(|l| l == "Every [files] entry in sdc.toml matches a file."),
        "the reader sees the drawer's last section: {rows:?}"
    );
    for label in [
        "Levels",
        "Resize",
        "Relative to",
        "Size floor",
        "DDS header",
        "BC7 weighting",
        "BC4/BC5 search radius",
        "BC1/BC3 search",
    ] {
        assert!(
            !rows.iter().any(|l| l == label),
            "{label:?} is the texture panel's (JI3, JI4): {rows:?}"
        );
    }
    assert_eq!(
        h.query_all_by_label_contains("Advanced texture fields")
            .count(),
        0,
        "the drawer has no texture fields section (JI4)"
    );
}

#[test]
fn the_strip_opens_the_drawer_and_close_collapses_it() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    assert!(!h.state().settings.advanced_open);
    assert!(h.query_by_label("Unmatched [files] entries").is_none());
    h.get_by_label("Advanced").click();
    h.run();
    assert!(h.state().settings.advanced_open);
    h.get_by_label("Unmatched [files] entries");
    h.get_by_label("Close").click();
    h.run();
    assert!(!h.state().settings.advanced_open);
}

#[test]
fn remove_drops_an_unmatched_entry() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.set_field(
        "landscape/rock01_rmaos",
        "alpha.threshold",
        serde_json::json!(90),
    );
    model.set_target(engine::config::Target::Vanilla);
    assert!(model.wait_idle(Duration::from_secs(60)));
    model.settings.advanced_open = true;
    let mut h = harness(model);
    h.run();
    h.get_by_label("landscape/rock01_rmaos");
    // Note CN: the "Also export recipe" section above pushes the entry below the drawer's fold.
    h.get_by_label("Remove").scroll_to_me();
    h.run();
    h.get_by_label("Remove").click();
    h.run();
    h.state_mut().wait_idle(Duration::from_secs(60));
    h.run();
    assert!(h.state().unmatched_keys.is_empty());

    assert!(
        h.query_by_label("Packing fill values").is_none(),
        "the global fill panel is retired"
    );
}
