mod app_harness;
#[allow(dead_code)] // fields only the doc shots read
#[path = "doc_shots/manifest.rs"]
mod manifest;
mod support;

use egui_kittest::kittest::Queryable;
use sdc_gui::{model::AppModel, ui::header};
use std::sync::OnceLock;
use tempfile::TempDir;

fn model() -> AppModel {
    static DATA: OnceLock<TempDir> = OnceLock::new();
    DATA.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        app_harness::redirect_app_data(dir.path());
        dir
    });
    AppModel::new(
        support::settings_with_meshes_view(),
        None,
        support::session(),
    )
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
    h.get_by_label("Open folder…");
    no_edit_controls(&h);
    let dir = tempfile::tempdir().unwrap();
    h.state()
        .model
        .picker
        .set_backend(std::sync::Arc::new(Folder(dir.path().to_owned())));
    h.get_by_label("Open folder…").click();
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
    assert!(h.query_by_label("Open export folder").is_none());
}
