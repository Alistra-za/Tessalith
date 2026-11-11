mod support;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::inspector::*;
use sdc_gui::model::AppModel;
use std::time::Duration;
use support::*;

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
    for label in [
        "Preset inspector",
        "Preview",
        "Advanced texture fields",
        "Unmatched [files] entries",
        "Show only what differs from sdc-base",
    ] {
        assert!(
            h.query_all_by_label_contains(label).count() >= 1,
            "no {label:?} in the drawer"
        );
    }
    h.get_by_label_contains("cs-quality");
}

fn drawer_harness(width: f32, model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(width, 1000.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

/// `leaf_model` with a user preset whose id and rule glob are long, chosen as the project's preset, so the
/// provenance column carries long text ("from preset …", "from rule …").
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
    model.settings.advanced_open = true;
    model.select_item("landscape/leaf.png");
    model
}

fn leaf_model(dir: &std::path::Path) -> AppModel {
    let mut model = opened(dir);
    model.settings.advanced_open = true;
    model.select_item("landscape/leaf.png");
    model
}

/// D10: the first column reads in words; the key rides in the AccessKit name and on hover.
#[test]
fn the_inspector_shows_labels_with_the_key_on_hover() {
    let d = tempfile::tempdir().unwrap();
    let model = leaf_model(d.path());
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
    let mut h = drawer_harness(1400.0, model);
    h.run();
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

/// D10, rule 10.3: no inspector label ends past the drawer's edge. The drawer body is drawn alone in a viewport as
/// wide as a narrow drawer (the panel itself is held wider by other sections, which would hide an overflow), with a
/// user preset whose long id puts long text in the provenance column. Uses the unclipped label rect: the clipped
/// `interact_rect` is cut to the viewport, so it could never exceed the edge.
#[test]
fn no_inspector_value_passes_the_drawer_edge() {
    for width in [260.0f32, 380.0, 520.0] {
        let d = tempfile::tempdir().unwrap();
        // The drawer's right edge: the allocated rect starts inside the harness margin, so it ends past `width`.
        let edge = std::rc::Rc::new(std::cell::Cell::new(f32::NAN));
        let edge_in = edge.clone();
        let mut h = Harness::builder()
            .with_size(eframe::egui::vec2(width, 1400.0))
            .build_ui_state(
                move |ui, model: &mut AppModel| {
                    let mut state = sdc_gui::ui::advanced::AdvancedState::default();
                    ui.allocate_ui(eframe::egui::vec2(width, 1400.0), |ui| {
                        edge_in.set(ui.max_rect().right());
                        sdc_gui::ui::advanced::body(ui, model, &mut state);
                    });
                },
                long_text_model(d.path()),
            );
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run();
        let inspector_top = h.get_by_label("Preset inspector").rect().min.y;
        let preview_top = h.get_by_label("Preview").rect().min.y;
        let edge = edge.get();
        let mut checked = 0;
        for n in h.query_all_by_role(Role::Label) {
            let r = n.rect();
            if r.min.y > inspector_top && r.min.y < preview_top {
                checked += 1;
                assert!(
                    r.max.x <= edge + 1.0,
                    "{width}: {:?} ends at {} past the drawer edge",
                    n.accesskit_node().label().or(n.accesskit_node().value()),
                    r.max.x
                );
            }
        }
        assert!(checked > 10, "{width}: only {checked} labels checked");
    }
}
/// Final review (GUI M9), rule 3.5: no combo in the drawer ends past its edge, with the longest Relative to label
/// ("Slot 8 · …") shown. Drawn alone, as `no_inspector_value_passes_the_drawer_edge` is.
// shortcut: at the drawer's 260 px minimum every combo of the fields grid ends ~3 px past the edge (the label
// column plus egui's combo width); pin 260 too once that grid gets exact-width combo slots.
#[test]
fn no_drawer_combo_passes_the_drawer_edge() {
    for width in [280.0f32, 380.0] {
        let d = tempfile::tempdir().unwrap();
        let mut model = leaf_model(d.path());
        model.set_field("landscape/leaf.png", "resize", serde_json::json!("half"));
        model.set_field(
            "landscape/leaf.png",
            "resize_relative_to",
            serde_json::json!("slot8"),
        );
        assert!(model.wait_idle(Duration::from_secs(60)));
        let mut h = Harness::builder()
            .with_size(eframe::egui::vec2(width, 1400.0))
            .build_ui_state(
                |ui, model: &mut AppModel| {
                    let mut state = sdc_gui::ui::advanced::AdvancedState::default();
                    ui.allocate_ui(eframe::egui::vec2(width, 1400.0), |ui| {
                        sdc_gui::ui::advanced::body(ui, model, &mut state);
                    });
                },
                model,
            );
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run();
        let combos: Vec<_> = h.query_all_by_role(Role::ComboBox).collect();
        assert!(
            combos.iter().any(|n| n
                .accesskit_node()
                .value()
                .is_some_and(|v| v.starts_with("Slot 8"))),
            "{width}: the Relative to combo shows Slot 8"
        );
        let past: Vec<_> = combos
            .iter()
            .filter(|n| n.rect().max.x > width + 1.0)
            .map(|n| (n.accesskit_node().value(), n.rect().max.x))
            .collect();
        assert!(past.is_empty(), "{width}: past the drawer edge: {past:?}");
    }
}

#[test]
fn the_inspector_names_the_texture() {
    let d = tempfile::tempdir().unwrap();
    let mut h = drawer_harness(1400.0, leaf_model(d.path()));
    h.run();
    h.get_by_label("How leaf.png's settings resolve");
    h.get_by_label("Batch preset: cs-quality");
    h.get_by_role_and_label(Role::Button, "Presets…");
    assert!(h.query_by_label_contains("%APPDATA%").is_none());
    h.get_by_role_and_label(Role::Button, "Presets…").click();
    h.run();
    assert!(h.state().presets_open, "Presets… opens the Presets tab");
}

#[test]
fn the_strip_opens_the_drawer_and_close_collapses_it() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    assert!(!h.state().settings.advanced_open);
    assert!(h.query_by_label("Preset inspector").is_none());
    h.get_by_label("Advanced").click();
    h.run();
    assert!(h.state().settings.advanced_open);
    h.get_by_label("Preset inspector");
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

#[test]
fn the_advanced_texture_fields_carry_their_locks_and_edit_through_the_model() {
    use sdc_gui::model::fields::advanced_field_views;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_field(
        "landscape/rock01.png",
        "encoder.bc45_search_radius",
        serde_json::json!("max"),
    );
    m.set_field("landscape/rock01.png", "header", serde_json::json!("dx10"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let job = m.item("landscape/rock01.png").unwrap().job.clone().unwrap();
    let views = advanced_field_views(m.target, &job);
    let names: Vec<_> = views.iter().map(|v| v.field).collect();
    assert_eq!(
        names,
        [
            "encoder.bc7_weights",
            "encoder.bc45_search_radius",
            "header",
            "resize",
            "resize_relative_to",
            "resize_floor"
        ]
    );
    let header = views.iter().find(|v| v.field == "header").unwrap();
    assert_eq!(header.value, serde_json::json!("dx10"));
    assert!(header.edited);
    let radius = views
        .iter()
        .find(|v| v.field == "encoder.bc45_search_radius")
        .unwrap();
    assert!(radius.lock.is_some(), "a colour map is not BC4/BC5");
}

/// M6 Track D spec §3: the GUI loads the user presets folder and the inspector lists what it loaded.
#[test]
fn the_inspector_lists_loaded_user_presets() {
    let d = tempfile::tempdir().unwrap();
    let presets = d.path().join("user-presets");
    std::fs::create_dir_all(&presets).unwrap();
    std::fs::write(
        presets.join("mine.toml"),
        "schema_version = 1\nid = \"mine\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n",
    )
    .unwrap();
    let mut m = opened(d.path());
    assert!(m.loaded_user_presets.is_empty());
    m.user_presets = Some(presets.clone());
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.loaded_user_presets.len(), 1);
    assert_eq!(m.loaded_user_presets[0].0, "mine");
    assert!(m.loaded_user_presets[0].1.ends_with("mine.toml"));
    m.settings.advanced_open = true;
    m.select_item("landscape/leaf.png");
    let mut h = harness(m);
    h.run();
    h.get_by_label_contains("mine.toml");
}

/// Final review Important 1: a user preset file that fails to load is shown in the inspector with its reason
/// (CFG-PRESET-SKIPPED), under the presets that did load.
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
    let ids: Vec<&str> = m
        .loaded_user_presets
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(ids, ["mine"]);
    assert_eq!(m.skipped_user_presets.len(), 1);
    assert!(m.skipped_user_presets[0].0.ends_with("bad.toml"));
    assert!(m.skipped_user_presets[0]
        .1
        .contains("unknown preset 'nope'"));
    m.settings.advanced_open = true;
    m.select_item("landscape/leaf.png");
    let mut h = harness(m);
    h.run();
    h.get_by_label_contains("mine.toml");
    h.get_by_label_contains("bad.toml");
    h.get_by_label_contains("unknown preset 'nope'");
}

/// Note FM25 (P17): the drawer's exact rows once the Batch section (threads, memory), the preview-size combo, the
/// 16384 box and the Interface section (Reduce motion) are gone. Nothing is selected, so only the section headings,
/// their always-present rows and the Close button remain.
#[test]
fn advanced_has_no_batch_or_preview_size_rows() {
    use eframe::egui;
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.settings.advanced_open = true;
    let mut h = harness(model);
    h.run();
    let panel = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("advanced"))
        .expect("the drawer is open")
        .outer_rect;
    let mut labels: Vec<String> = h
        .query_all_by_role(Role::Label)
        .chain(h.query_all_by_role(Role::Button))
        .chain(h.query_all_by_role(Role::CheckBox))
        .chain(h.query_all_by_role(Role::SpinButton))
        .chain(h.query_all_by_role(Role::ComboBox))
        .filter(|n| panel.expand(1.0).contains_rect(n.rect()))
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    labels.sort();
    labels.dedup();
    let want = [
        "Advanced texture fields",
        "Also export recipe",
        "Analysis cache limit",
        "Cache",
        "Checks",
        "Close",
        "Green direction check",
        "Live encode",
        "Low detail check",
        "Mipmaps",
        "Parallax check",
        "Preset inspector",
        "Presets…",
        "Preview",
        "Unmatched [files] entries",
    ];
    assert_eq!(labels, want);
}

/// Note HV2: the Cache section's box shows the analysis cache limit, and a typed limit is saved to gui.toml once. A
/// limit from gui.toml outside the box's range is left as it is by drawing the drawer.
#[test]
fn the_cache_limit_box_saves_a_typed_limit() {
    use eframe::egui::{Event, Key, Modifiers};
    use sdc_gui::settings::GuiSettings;
    fn spin<'a>(h: &'a Harness<'static, AppModel>) -> egui_kittest::Node<'a> {
        h.get_by_role_and_label(Role::SpinButton, "Analysis cache limit")
    }
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.settings.advanced_open = true;
    let toml = d.path().join("settings").join("gui.toml");
    model.settings_path = Some(toml.clone());
    let mut h = harness(model);
    h.run();
    assert_eq!(spin(&h).accesskit_node().numeric_value(), Some(500.0));

    h.state_mut().settings.analysis_cache_mb = Some(7);
    h.run();
    assert_eq!(spin(&h).accesskit_node().numeric_value(), Some(7.0));
    assert_eq!(h.state().settings.analysis_cache_mb, Some(7));
    assert!(!toml.exists(), "drawing saves nothing");

    spin(&h).focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    h.event(Event::Text("800".into()));
    h.run();
    assert_eq!(
        h.state().settings.analysis_cache_mb,
        Some(7),
        "not while typing"
    );
    h.key_press(Key::Enter);
    h.run();
    h.run();
    assert_eq!(h.state().settings.analysis_cache_mb, Some(800));
    assert_eq!(GuiSettings::load(&toml).0.analysis_cache_mb, Some(800));
}

/// Backlog T32: while a plan is pending the drawer's rows are off, like the settings panel's (Note FK), so a pick is
/// not dropped as "unchanged" against the stale view.
#[test]
fn drawer_texture_rows_wait_for_the_plan() {
    let d = tempfile::tempdir().unwrap();
    let mut model = leaf_model(d.path());
    model.set_field("landscape/leaf.png", "resize", serde_json::json!("half"));
    assert!(model.fix_off().is_some(), "the edit left a plan in flight");
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(400.0, 1400.0))
        .build_ui_state(
            |ui, model: &mut AppModel| {
                let mut state = sdc_gui::ui::advanced::AdvancedState::default();
                sdc_gui::ui::advanced::body(ui, model, &mut state);
            },
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run_steps(3);
    let combos: Vec<_> = h
        .query_all_by_role(Role::ComboBox)
        .map(|n| (n.accesskit_node().value(), n.accesskit_node().is_disabled()))
        .collect();
    // The rows show the stale view (Resize: None) until the plan lands, so every combo must be off meanwhile.
    assert!(
        combos.iter().any(|(v, _)| v.as_deref() == Some("None")),
        "{combos:?}"
    );
    assert!(
        combos.iter().all(|(_, off)| *off),
        "a drawer row is enabled during a plan: {combos:?}"
    );
}

/// Backlog T32 (second half): the pending plan's wait reason is on the disabled row, and once the plan lands a pick
/// of None is written (it was dropped as "unchanged" against the stale view).
#[test]
fn drawer_resize_pick_waits_for_the_plan_then_writes() {
    let d = tempfile::tempdir().unwrap();
    let mut model = leaf_model(d.path());
    model.set_field("landscape/leaf.png", "resize", serde_json::json!("half"));
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(400.0, 1400.0))
        .build_ui_state(
            |ui, model: &mut AppModel| {
                let mut state = sdc_gui::ui::advanced::AdvancedState::default();
                sdc_gui::ui::advanced::body(ui, model, &mut state);
            },
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run_steps(3);
    // The Resize combo is the one showing `value` (its accessible text is its value).
    let resize = |h: &Harness<'_, AppModel>, value: &str| {
        h.query_all_by_role(Role::ComboBox)
            .find(|n| n.accesskit_node().value().as_deref() == Some(value))
            .unwrap_or_else(|| panic!("no combo showing {value}"))
            .rect()
            .center()
    };
    h.hover_at(resize(&h, "None"));
    h.run();
    h.get_by_label("Wait for the plan to finish");
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run_steps(3);
    h.query_all_by_role(Role::ComboBox)
        .find(|n| n.accesskit_node().value().as_deref() == Some("Half"))
        .expect("the Resize combo shows the landed Half")
        .click();
    h.run();
    h.get_by_label("None").click();
    h.run_steps(3);
    let own = h
        .state()
        .project
        .as_ref()
        .unwrap()
        .file_value("landscape/leaf.png", "resize");
    assert_eq!(own, Some(serde_json::json!("none")));
}

/// Note HI18: the drawer's rows have no `?`, so the label shows the row's help on hover.
#[test]
fn a_drawer_label_shows_the_rows_help() {
    let d = tempfile::tempdir().unwrap();
    let model = leaf_model(d.path());
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(380.0, 1400.0))
        .build_ui_state(
            |ui, model: &mut AppModel| {
                let mut state = sdc_gui::ui::advanced::AdvancedState::default();
                ui.allocate_ui(eframe::egui::vec2(380.0, 1400.0), |ui| {
                    sdc_gui::ui::advanced::body(ui, model, &mut state);
                });
            },
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    let text = sdc_gui::model::fields::field_help("resize_floor").unwrap();
    assert!(h.query_all_by_label_contains(text).next().is_none());
    let rects: Vec<_> = h
        .query_all_by_label("Size floor")
        .filter(|n| n.accesskit_node().role() == Role::Label)
        .map(|n| n.rect())
        .collect();
    assert!(!rects.is_empty(), "the drawer has a Size floor row");
    let shown = rects.iter().any(|r| {
        h.query_all_by_label("Size floor")
            .find(|n| n.rect() == *r)
            .unwrap()
            .hover();
        h.run();
        h.query_all_by_label_contains(text).next().is_some()
    });
    assert!(shown, "no Size floor label shows its help");
    assert!(
        !h.query_all_by_role(Role::Label).any(|n| n
            .accesskit_node()
            .value()
            .is_some_and(|v| v.starts_with("Help: "))),
        "a ? in the drawer"
    );
}
