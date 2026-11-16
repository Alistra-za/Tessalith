//! E28 (egui 0.37): every input widget has an accessible name. egui_kittest's own check runs at every `run()` and
//! sees only the input roles on screen, so these tests open the screens other tests may not reach, check the one role
//! it skips (`ProgressIndicator`), and pin the short names: a hover sentence would otherwise become the name.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::rename::RenameMode;
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use support::*;

fn harness(m: AppModel) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 1000.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m)
}

/// Every progress node (a spinner, a bar) with no name: its value and rect.
fn unnamed_progress<S>(h: &Harness<'_, S>) -> Vec<String> {
    h.query_all_by_role(Role::ProgressIndicator)
        .map(|n| n.accesskit_node())
        .filter(|n| n.label().is_none_or(|l| l.trim().is_empty()))
        .map(|n| format!("{:?} {:?}", n.value(), n.raw_bounds()))
        .collect()
}

/// Opens the Textures column's Filter ▾ popup.
fn open_filters(h: &mut Harness<'static, AppModel>) {
    h.query_all_by_label("Filter ▾")
        .find(|n| n.accesskit_node().role() == Role::Button)
        .expect("the Filter button")
        .click();
    h.run();
    h.run();
}

#[test]
fn every_input_has_a_name_in_the_textures_view() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.select_item("landscape/rock01.png");
    let mut h = harness(m);
    h.run();
    open_filters(&mut h);
    h.get_by_role_and_label(Role::TextInput, "Search textures");
    h.get_by_role_and_label(Role::ComboBox, "Sort order");
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
}

#[test]
fn every_input_has_a_name_in_the_set_editor_and_json_editor() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with("rock01"))
        .unwrap()
        .key
        .clone();
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(Role::ComboBox, "Packed texture recipe");
    h.get_by_role_and_label(Role::TextInput, "EDID to add");
    // The JSON editor is a centre tab: it takes the set editor's place.
    h.state_mut().open_json_editor();
    h.run();
    h.get_by_label(sdc_gui::ui::json_editor::NAME_LABEL);
    h.get_by_role_and_label(Role::TextInput, "Search sets and paths");
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
}

#[test]
fn every_input_has_a_name_in_presets_and_rename() {
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    m.presets_model().duplicate();
    let mut h = harness(m);
    h.run();
    h.get_by_label("diffuse — BC7 · linear").click();
    h.run();
    h.get_by_label_contains("rmaos —").click();
    h.run();
    h.get_by_label("TOML preview").click();
    h.run();
    for n in ["Base preset", "Red source"] {
        h.get_by_role_and_label(Role::ComboBox, n);
    }
    h.get_by_role_and_label(Role::TextInput, "Preset name");
    h.get_by_role_and_label(Role::MultilineTextInput, "TOML preview");
    h.get_by_role_and_label(Role::SpinButton, "AO fill value");

    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item("landscape/leaf.png", false, false);
    m.open_rename(RenameMode::Inputs);
    assert!(m.dialog.is_some(), "the rename dialog is open");
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(Role::CheckBox, "Rename landscape/leaf.png");
    // Tied to the visible labels (`labelled_by`), so no hover sentence takes their place.
    for n in ["Find", "Replace", "Prefix", "Suffix"] {
        h.get_by_role_and_label(Role::TextInput, n);
    }
    h.get_by_role_and_label(Role::ComboBox, "Letter case");
}

/// The 3D view's busy spinner (survey row 48): a `ProgressIndicator`, which kittest's check does not cover.
#[test]
fn a_busy_3d_view_names_its_spinner() {
    use sdc_gui::model::viewport3d::{self as v3, Viewport3d};
    let mut v = Viewport3d::new();
    let _cmds = v.attach_test_thread();
    v.set_mesh(Some(v3::MeshSpec {
        key: 1,
        nif: v3::FileSource::Loose(std::path::PathBuf::from("C:/nope/1.nif")),
        textures: Default::default(),
        le_mesh: false,
        head_part: false,
        txst: Default::default(),
    }));
    v.request_frame(64, 64);
    assert!(v.busy());
    // Busy since well past the spinner's delay.
    let long_ago = std::time::Instant::now() - 2 * v3::SPINNER_DELAY;
    assert!(!v.show_spinner(long_ago));
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(800.0, 100.0))
        .build_ui_state(
            |ui, v: &mut Viewport3d| {
                ui.horizontal(|ui| sdc_gui::ui::viewport3d::toolbar(ui, v));
            },
            v,
        );
    // The spinner never goes idle: steps, not `run()`.
    h.run_steps(2);
    h.get_by_role_and_label(Role::ProgressIndicator, "Working");
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
}

#[test]
fn the_textures_filters_have_short_names() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    open_filters(&mut h);
    for n in [
        "Sub-type filter",
        "Output format filter",
        "Set membership filter",
        "Sort order",
    ] {
        h.get_by_role_and_label(Role::ComboBox, n);
    }
}

#[test]
fn the_header_preset_and_the_search_boxes_are_named() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    h.get_by_role_and_label(Role::ComboBox, "Preset");
    h.get_by_role_and_label(Role::TextInput, "Search textures");
}
