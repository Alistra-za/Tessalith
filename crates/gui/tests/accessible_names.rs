//! E28 (egui 0.37): every input widget has an accessible name. egui_kittest's own check runs at every `run()` and
//! sees only the input roles on screen, so these tests open the screens other tests may not reach, check the one role
//! it skips (`ProgressIndicator`), and pin the short names: a hover sentence would otherwise become the name.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::cs_pbr::Kind;
use sdc_gui::model::meshes::{edit_doc::EDIT_MOD, MeshesModel};
use sdc_gui::model::rename::RenameMode;
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use sdc_gui::settings::MeshesSettings;
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
    let s = m.sets.iter().find(|s| s.key.ends_with("rock01")).unwrap();
    let key = s.key.clone();
    let id = m.set_of_item_probe(&s.members[0]).unwrap().id;
    m.select_set(&key);
    // The CS texture set's value shows its number box beside the tick.
    m.add_cs_block(&id, Kind::Txst);
    m.set_cs_value(
        &id,
        Kind::Txst,
        "specularLevel",
        Some(serde_json::json!(0.02)),
    );
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(Role::ComboBox, "Packed texture recipe");
    h.get_by_role_and_label(Role::TextInput, "EDID to add");
    // Rule 8.4: a number box beside a named tick is "{label} value", never the tick's own name.
    h.get_by_role_and_label(Role::SpinButton, "Specular level value");
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
    // Rule 8.4: the header's "Preset" and the tab's own combos have distinct names.
    for n in [
        "Preset",
        "Preset to edit",
        "Preset target",
        "Base preset",
        "Red source",
    ] {
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

/// The Files tab's bar while an export runs (`drawer.rs`): a `ProgressIndicator`, which kittest's check does not
/// cover. A held `running` stands in for the export, as in `also_export.rs`.
#[test]
fn an_export_in_progress_names_its_bar() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export.running = Some(Default::default());
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 600.0))
        .build_ui_state(
            |ui, m: &mut AppModel| {
                let mut st = sdc_gui::ui::drawer::DrawerState::default();
                sdc_gui::ui::drawer::files_tab(ui, m, &mut st);
            },
            m,
        );
    // The bar asks for repaints while the export runs: steps, not `run()`.
    h.run_steps(3);
    h.get_by_role_and_label(Role::ProgressIndicator, "Export progress");
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
    h.state_mut().export.running = None;
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

type MeshesHarness = Harness<'static, MeshesModel>;

/// A Meshes model over a one-mod MO2 instance holding the Skinned SE fixture, with the scan requested and started.
fn scan_started() -> (tempfile::TempDir, MeshesModel) {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    let game = r.join("Game");
    std::fs::create_dir_all(game.join("Data")).unwrap();
    std::fs::write(
        r.join("ModOrganizer.ini"),
        format!(
            "[General]\ngamePath=@ByteArray({})\nselected_profile=@ByteArray(Default)\n",
            game.display().to_string().replace('\\', "\\\\")
        ),
    )
    .unwrap();
    std::fs::create_dir_all(r.join("overwrite")).unwrap();
    std::fs::create_dir_all(r.join("profiles/Default")).unwrap();
    std::fs::write(r.join("profiles/Default/modlist.txt"), "+A\n").unwrap();
    std::fs::write(
        r.join("profiles/Default/settings.ini"),
        "[General]\nLocalSettings=true\n",
    )
    .unwrap();
    let nif = std::fs::read(format!(
        "{}/../nif/tests/fixtures/TestNifFile_Skinned_SE.nif",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    std::fs::create_dir_all(r.join("mods/A/meshes")).unwrap();
    std::fs::write(r.join("mods/A/meshes/x.nif"), nif).unwrap();
    std::fs::create_dir_all(r.join("mods/A/textures")).unwrap();
    std::fs::write(r.join("mods/A/textures/new.dds"), b"dds").unwrap();
    let mut m = MeshesModel::new(Some(r.join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    m.activate_with(None);
    m.use_instance(r.to_path_buf(), "Default".into());
    m.request_scan();
    assert!(m.take_request());
    m.start();
    (d, m)
}

/// `scan_started`, run to its end, with A's x.nif selected.
fn scanned_meshes() -> (tempfile::TempDir, MeshesModel) {
    let (d, mut m) = scan_started();
    let t = std::time::Instant::now();
    m.poll();
    while m.scanning() {
        assert!(t.elapsed() < std::time::Duration::from_secs(60));
        std::thread::sleep(std::time::Duration::from_millis(10));
        m.poll();
    }
    assert_eq!(m.error, None);
    let a = m.mods.iter().find(|r| r.name == "A").unwrap().id;
    m.select_mod(a);
    let c = m.nifs[0].copy_id;
    m.select_nif(c);
    m.set_nif_export_root(Some(d.path().join("mods").join(EDIT_MOD)));
    (d, m)
}

fn meshes_harness(m: MeshesModel) -> MeshesHarness {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
            m,
        )
}

/// Clicks the first button whose name contains `part` and lets the popup draw.
fn open_browse(h: &mut MeshesHarness, part: &str) {
    h.query_all_by_label_contains(part)
        .find(|n| n.accesskit_node().role() == Role::Button)
        .unwrap_or_else(|| panic!("a button named {part:?}"))
        .click();
    h.run();
    h.run();
}

#[test]
fn every_input_has_a_name_in_the_meshes_view() {
    let (_d, m) = scanned_meshes();
    let mut h = meshes_harness(m);
    h.run();
    // The mod filter, and the filter box inside its open list.
    h.get_by_role_and_label(Role::ComboBox, "Mod filter")
        .click();
    h.run();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Filter mods by name");
    h.key_press(egui::Key::Escape);
    h.run();
    // A skin-tint shape for the Advanced Skin card, whose Browse popup has its own filter.
    let m = h.state_mut();
    m.shapes_col = true;
    let mut skin = m.shapes[0].clone();
    skin.block = 900;
    skin.index3d = Some(900);
    skin.shader_kind = 1;
    skin.shader_type = Some(5);
    skin.name = Some("Body".into());
    m.shapes.push(skin);
    h.run();
    open_browse(&mut h, "Browse for the ");
    h.get_by_role_and_label(Role::TextInput, "Filter textures");
    h.key_press(egui::Key::Escape);
    h.run();
    // Edit swaps the list for the edit model's cards and the edit panel; a slot's Browse has its own filter.
    h.state_mut().open_edit();
    h.run();
    h.get_by_label_contains("Select shape #0").click();
    h.run();
    h.run();
    open_browse(&mut h, "Browse for ");
    h.get_by_role_and_label(Role::TextInput, "Filter textures");
}

#[test]
fn a_running_scan_names_its_spinner() {
    // The scan is started and never polled, so it runs while the controls draw.
    let (_d, m) = scan_started();
    assert!(m.scanning());
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(800.0, 100.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::scan_controls(ui, m),
            m,
        );
    // The spinner never goes idle: steps, not `run()`.
    h.run_steps(3);
    h.get_by_role_and_label(Role::ProgressIndicator, "Scanning mods");
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
}

#[test]
fn the_meshes_search_is_named() {
    let (_d, m) = scanned_meshes();
    let mut h = meshes_harness(m);
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Search meshes and mods");
}
