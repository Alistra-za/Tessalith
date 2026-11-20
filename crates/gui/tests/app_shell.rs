mod app_harness;
#[allow(dead_code)] // fields only the doc shots read
#[path = "doc_shots/manifest.rs"]
mod manifest;
mod support;

use eframe::egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};
use sdc_gui::{model::AppModel, ui::header};
use std::sync::OnceLock;
use tempfile::TempDir;

const BANNER_OPEN: &str = "Open a texture folder from the Meshes banner";

fn app_data() {
    static DATA: OnceLock<TempDir> = OnceLock::new();
    DATA.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        app_harness::redirect_app_data(dir.path());
        dir
    });
}

fn model() -> AppModel {
    app_data();
    AppModel::new(
        support::settings_with_meshes_view(),
        None,
        support::session(),
    )
}

#[test]
fn settings_opens_in_the_meshes_view_and_back_returns() {
    app_data();
    let d = tempfile::tempdir().unwrap();
    let mut h = app_harness::plain_harness(support::opened(d.path()));
    header::set_meshes_view(&h.ctx, true);
    app_harness::settle(&mut h, 3);
    h.get_by_label("Settings…").click();
    app_harness::settle(&mut h, 3);
    assert!(header::meshes_view_active(&h.ctx), "the view stays Meshes");
    h.get_by_label("Settings tab");
    h.get_by_label("Close Settings");
    h.get_by_label_contains("Back to meshes").click();
    app_harness::settle(&mut h, 3);
    assert!(!h.state().model.centre_over_meshes);
    h.get_by_label("NIFs");
}

#[test]
fn settings_over_meshes_with_no_project_keeps_the_banner() {
    app_data();
    let m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    let mut h = app_harness::plain_harness(m);
    header::set_meshes_view(&h.ctx, true);
    app_harness::settle(&mut h, 3);
    h.get_by_label("Settings…").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label_contains("No project open. This list");
    h.get_by_label_contains("Back to meshes");
    h.get_by_label_contains("App settings: these apply to every project");
    assert!(h.query_by_label("Export without review").is_none());
    assert!(header::meshes_view_active(&h.ctx));
    h.get_by_label_contains("Back to meshes").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label_contains("No project open.");
    assert_eq!(h.query_all_by_label_contains("Back to meshes").count(), 0);
}

#[test]
fn settings_and_other_docked_tools_yield_to_meshes_or_view_navigation() {
    use sdc_gui::ui::json_editor::WINDOW_TITLE;

    app_data();
    let d = tempfile::tempdir().unwrap();
    let mut h = app_harness::plain_harness(support::opened(d.path()));
    h.get_by_label("Meshes view").click();
    app_harness::settle(&mut h, 3);
    h.state_mut().model.open_presets();
    app_harness::settle(&mut h, 3);
    assert!(
        h.state().model.centre_over_meshes,
        "Presets opens over Meshes"
    );
    h.get_by_label("Close Presets");
    h.state_mut().model.open_json_editor();
    app_harness::settle(&mut h, 3);
    h.get_by_label("Presets tab");
    h.get_by_label(&format!("Pop out {WINDOW_TITLE}")).click();
    app_harness::settle(&mut h, 3);
    assert!(h.state().model.json_editor_popped);
    assert!(!h.state().model.centre_over_meshes);
    h.get_by_label("NIFs");
    h.get_by_label(&format!("Dock {WINDOW_TITLE}")).click();
    app_harness::settle(&mut h, 3);
    assert!(h.state().model.centre_over_meshes);
    h.get_by_label(&format!("Close {WINDOW_TITLE}")).click();
    app_harness::settle(&mut h, 3);
    h.state_mut().model.open_presets();
    app_harness::settle(&mut h, 3);
    h.get_by_label("Close Presets").click();
    app_harness::settle(&mut h, 3);
    assert!(!h.state().model.centre_over_meshes);
    h.get_by_label("NIFs");
    h.get_by_label("Settings…").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("Textures view").click();
    app_harness::settle(&mut h, 3);
    assert!(!h.state().model.centre_over_meshes);
    assert!(!header::meshes_view_active(&h.ctx));
    h.get_by_label("Close Settings");
    h.get_by_label("Meshes view").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("NIFs");
    h.get_by_label("Settings…").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("Meshes view").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("NIFs");
    assert!(!h.state().model.centre_over_meshes);
    h.get_by_label("Settings…").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("Selection tab").click();
    app_harness::settle(&mut h, 3);
    assert!(!header::meshes_view_active(&h.ctx));
    assert!(!h.state().model.centre_over_meshes);
}

#[test]
fn a_view_click_commits_the_overlay_json_name_once() {
    use eframe::egui::{Event, PointerButton};

    app_data();
    for tab in ["Meshes view", "Textures view"] {
        let d = tempfile::tempdir().unwrap();
        let mut h = app_harness::plain_harness(support::pbr_opened(d.path()));
        header::set_meshes_view(&h.ctx, true);
        h.state_mut().model.open_json_editor();
        app_harness::settle(&mut h, 3);
        h.get_by_label("Back to meshes");
        let steps = h.state().model.history.undo.len();
        h.get_by_label("JSON file name").click();
        h.run_steps(2);
        h.event(Event::Text("Overlay pack".into()));
        h.run_steps(2);
        assert_eq!(h.state().model.history.undo.len(), steps);
        let pos = h.get_by_label(tab).rect().center();
        h.input_mut().events.push(Event::PointerMoved(pos));
        for pressed in [true, false] {
            h.input_mut().events.push(Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        app_harness::settle(&mut h, 3);
        let m = &h.state().model;
        assert_eq!(
            m.sets_doc.as_ref().unwrap().json_name().as_deref(),
            Some("Overlay pack"),
            "{tab} commits the focused JSON name"
        );
        assert_eq!(m.history.undo.len(), steps + 1);
        assert!(!m.centre_over_meshes);
        assert_eq!(header::meshes_view_active(&h.ctx), tab == "Meshes view");
    }
}

#[test]
fn invalid_header_mo2_pick_shows_the_error_above_the_open_tool() {
    app_data();
    let d = tempfile::tempdir().unwrap();
    let mut h = app_harness::plain_harness_no_mo2(model());
    header::set_meshes_view(&h.ctx, true);
    h.state_mut().model.open_settings_tab();
    app_harness::settle(&mut h, 3);
    let invalid = d.path().join("missing-instance");
    let (backend, requests) = support::recording_picker(Some(invalid.clone()));
    h.state().meshes.picker.set_backend(backend);
    h.get_by_label("MO2…").click();
    h.run_steps(3);
    h.get_by_label("Choose…").click();
    h.run_steps(2);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state().meshes.error.is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the MO2 pick did not return an error"
        );
        app_harness::settle(&mut h, 1);
    }
    assert_eq!(requests.lock().unwrap().len(), 1);
    h.key_press(Key::Escape);
    h.run_steps(3);
    let error = h.state().meshes.error.as_deref().unwrap();
    assert!(error.contains(invalid.to_str().unwrap()));
    let back = h.get_by_label("Back to meshes").rect();
    let message = h
        .query_by_label(error)
        .expect("the MO2 error must be visible over Settings");
    assert!(message.rect().bottom() <= back.top());
    h.get_by_label("Dismiss").click();
    h.run_steps(3);
    assert!(h.state().meshes.error.is_none());
    h.get_by_label("Back to meshes");
}

fn no_edit_controls(h: &app_harness::H) {
    for label in ["Undo", "Redo", "Save", "Files", "Problems", "Messages"] {
        assert!(
            h.query_all_by_label_contains(label).next().is_none(),
            "{label} is visible"
        );
    }
    assert_eq!(support::header_export_action(h), None);
}

#[test]
fn no_more_button_with_no_project() {
    let mut h = app_harness::plain_harness(model());
    h.set_size(eframe::egui::vec2(1100.0, 800.0));
    app_harness::settle(&mut h, 3);
    h.get_by_label("Settings…");
    assert_eq!(h.query_all_by_label("More header items").count(), 0);
}

#[test]
fn tools_is_hidden_with_no_project() {
    let mut h = app_harness::plain_harness(model());
    app_harness::settle(&mut h, 3);
    h.get_by_label("Settings…");
    assert_eq!(h.query_all_by_label("Tools").count(), 0);
    h.get_by_label("More export options").click();
    h.run_steps(3);
    h.get_by_label("Make marker cube…").click();
    h.run_steps(3);
    assert_eq!(
        h.state().model.dialog,
        Some(sdc_gui::model::Dialog::MakeMarker)
    );
    h.get_by_label("Marker colour");
    h.get_by_label("Save…");
}

#[test]
fn a_launch_with_no_project_shows_the_start_card_even_after_meshes() {
    let mut m = model();
    let dir = tempfile::tempdir().unwrap();
    m.settings.recent.push(dir.path().to_owned());
    let h = app_harness::restored_harness(m);
    assert!(
        !header::meshes_view_active(&h.ctx),
        "the first frame should show Textures"
    );
    for label in [
        "Open",
        "Open folder…",
        "Recent",
        "Settings…",
        "Textures view",
        "Meshes view",
    ] {
        h.get_by_label(label);
    }
    h.get_by_label(dir.path().display().to_string().as_str());
    no_edit_controls(&h);
}

#[test]
fn reopening_a_project_restores_the_saved_meshes_view() {
    let mut m = model();
    let dir = tempfile::tempdir().unwrap();
    m.open_folder(dir.path().to_owned());
    let h = app_harness::restored_harness(m);
    assert!(header::meshes_view_active(&h.ctx));
    h.get_by_label("NIFs");
    h.get_by_label("Save");
}

#[test]
fn the_meshes_view_with_no_project_keeps_its_list_and_a_banner() {
    use sdc_gui::model::picker::{PickerBackend, PickerSpec};
    struct Folder(std::path::PathBuf);
    impl PickerBackend for Folder {
        fn pick(&self, _: &PickerSpec) -> Option<std::path::PathBuf> {
            Some(self.0.clone())
        }
    }
    let mut h = app_harness::plain_harness(model());
    h.get_by_label("Meshes view").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("NIFs");
    h.get_by_label(
        "No project open. This list is your MO2 instance; open a folder to edit its textures.",
    );
    no_edit_controls(&h);
    h.get_by_label("Open").click();
    h.run_steps(3);
    assert_eq!(
        h.query_all_by_label("Open folder…").count(),
        1,
        "only the open menu item should have the plain name"
    );
    h.get_by_label(BANNER_OPEN);
    h.key_press(Key::Escape);
    h.run_steps(2);
    let dir = tempfile::tempdir().unwrap();
    h.state()
        .model
        .picker
        .set_backend(std::sync::Arc::new(Folder(dir.path().to_owned())));
    h.get_by_label(BANNER_OPEN).click();
    h.run_steps(2);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state().model.picker.busy() {
        assert!(
            std::time::Instant::now() < deadline,
            "folder picker did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(app_harness::settle(&mut h, 3));
    assert_eq!(h.state().model.root.as_deref(), Some(dir.path()));
    assert!(h.query_by_label("Open folder…").is_none());
    assert!(h.query_by_label(BANNER_OPEN).is_none());
}

#[test]
fn selecting_a_nif_without_editing_it_keeps_edit_controls_hidden() {
    let mut h = app_harness::plain_harness(model());
    header::set_meshes_view(&h.ctx, true);
    h.state_mut().meshes.selected_nif = Some(1);
    h.run_steps(2);
    assert!(h.state().meshes.selected_nif.is_some());
    assert!(h.state().model.nif_review_rows().is_empty());
    no_edit_controls(&h);
}

#[test]
fn a_no_project_files_tab_stays_hidden_even_when_saved_open() {
    let mut m = model();
    m.show_tab(sdc_gui::model::bottom::BottomTab::Files);
    let h = app_harness::plain_harness(m);
    assert!(
        h.query_by_label("Files").is_none(),
        "the Files tab is visible"
    );
    assert!(h.query_by_label_contains("Open export folder").is_none());
}

const MESSAGES_REASON: &str = "Open a texture folder or edit a NIF in Meshes to see Messages.";

#[test]
fn a_hidden_messages_tab_stops_reading_new_entries() {
    let mut m = model();
    m.show_tab(sdc_gui::model::bottom::BottomTab::Messages);
    assert!(m.messages.is_open());
    let mut h = app_harness::plain_harness(m);
    assert!(h.query_by_label("Messages").is_none());
    let m = &mut h.state_mut().model;
    let unread = m.messages.unread();
    m.messages
        .log(sdc_gui::model::messages::Severity::Warning, "New entry");
    assert_eq!(m.messages.unread(), unread + 1);
}

fn start_card_with_unread_message() -> app_harness::H {
    let mut h = app_harness::restored_harness(model());
    h.state_mut().model.messages.clear();
    h.state_mut().model.messages.log(
        sdc_gui::model::messages::Severity::Warning,
        "Folder could not be opened",
    );
    h
}

#[test]
fn hidden_messages_shortcut_keeps_the_entry_unread() {
    let mut h = start_card_with_unread_message();
    h.key_press_modifiers(Modifiers::COMMAND, Key::M);
    h.run_steps(2);
    let m = &h.state().model;
    assert_eq!(m.messages.unread(), 1, "Ctrl+M read an unseen entry");
    assert!(!m.messages.is_open() && !m.bottom.open);
    h.get_by_label(MESSAGES_REASON);
}

#[test]
fn hidden_messages_palette_click_keeps_the_entry_unread() {
    let mut h = start_card_with_unread_message();
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    h.run_steps(2);
    h.state_mut().model.nav.palette.as_mut().unwrap().query = "Messages".into();
    h.run_steps(2);
    let row = "Messages   Ctrl+M";
    h.get_by_label(row).click();
    h.run_steps(2);
    assert_eq!(
        h.state().model.messages.unread(),
        1,
        "the palette read an unseen entry"
    );
    assert!(h.get_by_label(row).accesskit_node().is_disabled());
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.get_by_label(row).hover();
    h.run_steps(4);
    h.get_by_label(MESSAGES_REASON);
    h.key_press(Key::Enter);
    h.run_steps(2);
    let m = &h.state().model;
    assert_eq!(m.messages.unread(), 1);
    assert!(!m.messages.is_open() && !m.bottom.open);
    assert!(m.nav.palette.is_some());
}
