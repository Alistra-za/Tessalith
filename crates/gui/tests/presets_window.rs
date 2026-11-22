//! Note AM: the preset creator, a centre tab since Note FB7 (kittest). No presets folder: nothing can be written.
mod support;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1400.0, 1000.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    h
}

#[test]
fn presets_opens_the_creator_and_a_builtin_is_read_only() {
    let mut h = harness(AppModel::placeholder(GuiSettings::default()));
    h.run();
    // R14, K5: Presets… ends the Preset popup, which opens with no folder too.
    h.query_all_by_role(eframe::egui::accesskit::Role::ComboBox)
        .find(|n| {
            n.accesskit_node()
                .value()
                .or(n.accesskit_node().label())
                .is_some_and(|v| v.ends_with(" · default"))
        })
        .expect("the Preset combo")
        .click();
    h.run();
    h.get_by_label("Presets…").click();
    h.run();
    assert!(h.state().presets_open);
    h.state_mut().presets_model().select("cs-quality");
    h.run();
    h.get_by_label_contains("Built-in presets are read-only");
    assert!(
        h.query_by_label("Save preset").is_none(),
        "a built-in offers only Duplicate"
    );
    h.get_by_label("Duplicate").click();
    h.run();
    assert_eq!(h.state_mut().presets_model().id(), "cs-quality-copy");
    h.query_by_label_contains("Built-in presets are read-only")
        .is_none()
        .then_some(())
        .expect("the copy is editable");
}

#[test]
fn the_orm_example_is_shown_read_only_and_never_written() {
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    m.presets_model().duplicate();
    let mut h = harness(m);
    h.run();
    h.get_by_label_contains("PBR RMAOS —").click();
    h.run();
    h.get_by_label_contains("ORM → RMAOS");
    assert!(!h.state_mut().presets_model().toml_text().contains("orm"));
}

#[test]
fn the_reset_arrow_drops_an_override() {
    use engine::config::Subtype;
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    m.presets_model().duplicate();
    m.presets_model()
        .set_field(Subtype::Diffuse, "resize", serde_json::json!("half"));
    let mut h = harness(m);
    h.run();
    collapse_pbr(&mut h);
    h.get_by_label("Diffuse — BC7 · linear").click();
    h.run();
    h.get_by_label("Suffixes");
    // Colour rows have no Channel remap or Packing fills.
    assert!(h.query_by_label("Packing fills").is_none());
    h.get_by_label("Alpha");
    h.get_by_label_contains("base: None");
    h.get_by_label("↺").click();
    h.run();
    assert_eq!(
        h.state_mut()
            .presets_model()
            .field(Subtype::Diffuse, "resize"),
        None
    );
    assert!(h.query_by_label("↺").is_none());
}

/// Note FM20: the preset editor offers the 1-bit alpha box for a BC1 sub-type only, and ticking it sets the field.
#[test]
fn the_one_bit_alpha_box_shows_for_a_bc1_row_only() {
    use eframe::egui::accesskit::Role;
    use engine::config::Subtype;
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    m.presets_model().duplicate();
    let mut h = harness(m);
    h.run();
    collapse_pbr(&mut h);
    h.get_by_label("Diffuse — BC7 · linear").click();
    h.run();
    h.get_by_label("Alpha");
    assert!(
        h.query_by_label("1-bit alpha").is_none(),
        "no box under BC7"
    );
    h.state_mut().presets_model().set_field(
        Subtype::Diffuse,
        "format",
        serde_json::json!(["bc1", "unorm"]),
    );
    h.run();
    h.get_by_role_and_label(Role::CheckBox, "1-bit alpha")
        .click();
    h.run();
    assert_eq!(
        h.state_mut()
            .presets_model()
            .field(Subtype::Diffuse, "encoder.bc1_alpha"),
        Some(serde_json::json!(true))
    );
}

/// Final review (GUI M2): the preset editor has a Perceptual box in Compression for BC1, BC3 and BC7 (vanilla-quality
/// sets it, FM42/FM43); ticking it sets `encoder.perceptual` and ↺ drops only that key.
#[test]
fn the_perceptual_box_sets_and_clears_the_field() {
    use eframe::egui::accesskit::Role;
    use engine::config::Subtype;
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    m.presets_model().duplicate();
    m.presets_model()
        .set_field(Subtype::Diffuse, "resize", serde_json::json!("half"));
    let mut h = harness(m);
    h.run();
    collapse_pbr(&mut h);
    h.get_by_label("Diffuse — BC7 · linear").click();
    h.run();
    h.get_by_role_and_label(Role::CheckBox, "Perceptual")
        .click();
    h.run();
    let field = |h: &mut egui_kittest::Harness<'_, AppModel>, f: &str| {
        h.state_mut().presets_model().field(Subtype::Diffuse, f)
    };
    assert_eq!(
        field(&mut h, "encoder.perceptual"),
        Some(serde_json::json!(true))
    );
    // Two overrides, two ↺: click the one on the Perceptual row's line.
    let tick = h.get_by_role_and_label(Role::CheckBox, "Perceptual").rect();
    let reset = h
        .query_all_by_label("↺")
        .find(|n| (n.rect().center().y - tick.center().y).abs() < 4.0)
        .expect("the Perceptual row's ↺");
    reset.click();
    h.run();
    assert_eq!(field(&mut h, "encoder.perceptual"), None);
    assert_eq!(field(&mut h, "resize"), Some(serde_json::json!("half")));
}

fn with_folder(dir: &std::path::Path) -> AppModel {
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.user_presets = Some(dir.to_path_buf());
    m.open_presets();
    m
}

#[test]
fn unsaved_edits_ask_before_new_and_esc_keeps_them() {
    use engine::config::Subtype;
    let d = tempfile::tempdir().unwrap();
    let mut m = with_folder(d.path());
    m.presets_model().select("cs-quality");
    m.presets_model().duplicate();
    m.presets_model().set_id("edited");
    m.presets_model()
        .set_field(Subtype::Diffuse, "resize", serde_json::json!("half"));
    let mut h = harness(m);
    h.run();
    h.get_by_label("New").click();
    h.run();
    h.get_by_label("Discard unsaved changes to edited?");
    // A7 / R18: the discard question has no Enter default; Enter keeps it open and the edits.
    h.run();
    h.key_press(eframe::egui::Key::Enter);
    h.run();
    h.get_by_label("Discard unsaved changes to edited?");
    assert_eq!(
        h.state_mut().presets_model().id(),
        "edited",
        "Enter kept the edits"
    );
    h.key_press(eframe::egui::Key::Escape);
    h.run();
    h.run();
    assert!(h
        .query_by_label("Discard unsaved changes to edited?")
        .is_none());
    assert_eq!(
        h.state_mut().presets_model().id(),
        "edited",
        "Esc kept the edits"
    );
    h.get_by_label("New").click();
    h.run();
    h.get_by_label("Discard").click();
    h.run();
    assert_eq!(h.state_mut().presets_model().id(), "", "a new preset");
}

#[test]
fn delete_names_the_file_and_broken_files_are_listed_with_open_folder() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("broken.toml"), "id = ").unwrap();
    std::fs::write(
        d.path().join("other-name.toml"),
        "schema_version = 1\nid = \"mine\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n",
    )
    .unwrap();
    let mut m = with_folder(d.path());
    m.presets_model().select("mine");
    let mut h = harness(m);
    h.run();
    h.get_by_label("1 preset files did not load:");
    h.get_by_label_contains("broken.toml: ");
    h.get_by_label("Open folder").click();
    h.run();
    assert_eq!(
        h.state_mut().take_shell(),
        [sdc_gui::model::shell::ShellRequest::Open(
            d.path().to_path_buf()
        )]
    );
    h.get_by_label("Delete…").click();
    h.run();
    h.get_by_label_contains("other-name.toml?");
}

/// Trace #22: the Delete confirmation belongs to the preset it was asked on, not to the next one picked.
#[test]
fn the_delete_question_is_per_preset() {
    let d = tempfile::tempdir().unwrap();
    for id in ["aa", "bb"] {
        std::fs::write(
            d.path().join(format!("{id}.toml")),
            format!(
                "schema_version = 1\nid = \"{id}\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n"
            ),
        )
        .unwrap();
    }
    let mut m = with_folder(d.path());
    m.presets_model().select("aa");
    let mut h = harness(m);
    h.run();
    h.get_by_label("Delete…").click();
    h.run();
    h.get_by_label_contains("aa.toml?");
    h.state_mut().presets_model().select("bb");
    h.run();
    h.run();
    assert!(
        h.query_by_label_contains("bb.toml?").is_none(),
        "armed for bb"
    );
    h.get_by_label("Delete…");
}

#[test]
fn the_orm_example_is_a_read_only_branch_under_rmaos() {
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    let mut h = harness(m);
    h.run();
    h.get_by_label_contains("PBR RMAOS —").click();
    h.run();
    h.get_by_label_contains("_orm → _rmaos — ORM → RMAOS");
    assert_eq!(h.query_all_by_label_contains("_rmaos — ").count(), 2);
}

#[test]
fn a_branch_shows_its_overrides_hints_the_sub_type_and_resets() {
    use engine::config::Subtype;
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    m.presets_model().duplicate();
    m.presets_model()
        .set_branch_field(Subtype::Diffuse, "d", "resize", serde_json::json!("half"));
    let mut h = harness(m);
    h.run();
    collapse_pbr(&mut h);
    h.get_by_label("Diffuse — BC7 · linear").click();
    h.run();
    h.get_by_label("_d — BC7 · linear (overrides 1)").click();
    h.run();
    h.get_by_label_contains("inherits: None");
    h.get_by_label("↺").click();
    h.run();
    assert_eq!(
        h.state_mut()
            .presets_model()
            .branch_field(Subtype::Diffuse, "d", "resize"),
        None
    );
    assert!(!h.state_mut().presets_model().toml_text().contains("rules"));
}

#[test]
fn add_alias_opens_its_empty_branch() {
    const TEXT: eframe::egui::accesskit::Role = eframe::egui::accesskit::Role::TextInput;
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    m.presets_model().duplicate();
    let mut h = harness(m);
    h.run();
    collapse_pbr(&mut h);
    h.get_by_label("Diffuse — BC7 · linear").click();
    h.run();
    h.get_by_role_and_label(TEXT, "New alias _").focus();
    h.run();
    h.get_by_role_and_label(TEXT, "New alias _")
        .type_text("foo");
    h.run();
    h.get_by_label("Add alias").click();
    h.run();
    h.run();
    h.get_by_label("_foo → _d — BC7 · linear");
    h.get_by_label("Remove alias");
}

#[test]
fn the_families_show_in_order_with_their_hints() {
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    let h = harness(m);
    let y = |label: &str| h.get_by_label(label).rect().top();
    let (pbr, van) = (y("PBR / Community Shaders"), y("Vanilla"));
    assert!(pbr < van, "{pbr} {van}");
    h.get_by_label("PBR / Community Shaders — used under the cs target");
    // pbr_emissive under PBR / CS; glow and complex material under Vanilla (Note AZ).
    let emissive = h.get_by_label_contains("emissive —").rect().top();
    let glow = h.get_by_label_contains("Glow —").rect().top();
    let complex = h.get_by_label_contains("Complex material —").rect().top();
    assert!(pbr < emissive && emissive < van, "{emissive}");
    assert!(van < glow && van < complex, "{glow} {complex}");
}

/// Note JI1: the Presets tab is about presets only; the project's Mode moved to the Settings tab.
#[test]
fn the_presets_tab_has_no_project_block() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    m.open_presets();
    let mut h = harness(m);
    h.run();
    h.get_by_label("Presets tab");
    h.get_by_label("Built-in presets are read-only: Duplicate makes an editable copy.");
    assert!(h.query_by_label_contains("This project").is_none());
    assert!(h.query_by_label_contains("Mode (this project)").is_none());
    assert!(h
        .query_by_label("This beats the Target set in a preset.")
        .is_none());
}

/// Note FM43: the editor shows the effort a BC6H sub-type encodes at (Slowest) and says it is the default, without
/// storing it; a stored Veryslow shows as itself.
#[test]
fn the_editor_shows_the_bc6h_default_effort_and_says_why() {
    use engine::config::{Effort, Subtype};
    use sdc_gui::ui::settings_panel::subtype_label;
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    m.presets_model().duplicate();
    let pm = m.presets_model();
    let before = pm.toml_text();
    for st in [Subtype::PbrEmissive, Subtype::HdrSky] {
        let row = pm.row(st).unwrap();
        let (p, base) = (row.profile.as_ref().unwrap(), row.base.as_ref().unwrap());
        assert_eq!(p.encoder.effort, Effort::Slowest, "{st:?}");
        assert_eq!(base.encoder.effort, Effort::Slowest, "{st:?}");
        assert!(row.peak && row.base_peak, "{st:?}");
        assert_eq!(pm.field(st, "encoder.effort"), None, "stored value");
    }
    let d = pm.row(Subtype::Diffuse).unwrap();
    assert_eq!(d.profile.as_ref().unwrap().encoder.effort, Effort::Veryslow);
    assert!(!d.peak && !d.base_peak);
    assert_eq!(pm.toml_text(), before, "showing the default stores nothing");
    // A stored effort is the user's choice, not the default.
    pm.set_field(
        Subtype::PbrEmissive,
        "encoder.effort",
        serde_json::json!("veryslow"),
    );
    let row = pm.row(Subtype::PbrEmissive).unwrap();
    assert_eq!(
        row.profile.as_ref().unwrap().encoder.effort,
        Effort::Veryslow
    );
    assert!(!row.peak && row.base_peak);
    pm.clear_field(Subtype::PbrEmissive, "encoder.effort");
    assert!(pm.row(Subtype::PbrEmissive).unwrap().peak);
    // The row says why.
    let mut h = harness(m);
    h.run();
    h.get_by_label_contains(&format!("{} —", subtype_label(Subtype::PbrEmissive)))
        .click();
    h.run();
    h.get_by_label_contains("base: Slowest (BC6H default)");
}

/// The PBR / CS family comes first and is long: fold it so the Vanilla rows are on screen.
fn collapse_pbr(h: &mut Harness<'static, AppModel>) {
    h.get_by_label("PBR / Community Shaders").click();
    h.run();
}
