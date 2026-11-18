//! Review E export shell: the per-project "Export without review" key in `sdc-project.toml` (Task 1).
mod support;
use sdc_gui::model::Dialog;

#[test]
fn export_without_review_is_per_project_and_off_by_default() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let root = support::mod_folder(a.path());
    let mut m = support::opened(a.path());
    assert!(!m.export_without_review());
    m.set_export_without_review(true);
    let text = std::fs::read_to_string(root.join("sdc-project.toml")).unwrap();
    assert!(text.contains("export_without_review = true"), "{text}");
    m.open_folder(b.path().to_path_buf());
    assert!(
        !m.export_without_review(),
        "another project keeps the default"
    );
    m.open_folder(root.clone());
    assert!(m.export_without_review());
}

#[test]
fn the_opt_out_on_a_dirty_project_is_not_lost_silently() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let root = support::mod_folder(a.path());
    let mut m = support::opened(a.path());
    // One user edit to sdc-project.toml: a set rename (`model/sets.rs` `rename_set`).
    let id = sdc_gui::model::sets::views(&m)[0].id.clone();
    m.rename_set(&id, &format!("{id}_x")).unwrap();
    assert!(
        m.sets_doc.as_ref().unwrap().is_dirty(),
        "guard: the rename is an sdc-project.toml edit"
    );
    m.set_export_without_review(true);
    let on_disk = std::fs::read_to_string(root.join("sdc-project.toml")).unwrap_or_default();
    assert!(
        !on_disk.contains("export_without_review"),
        "kept for the next Save, not written alone"
    );
    assert!(
        m.status_line
            .contains("saved with your other changes at the next Save"),
        "{}",
        m.status_line
    );
    assert!(
        !m.request_open(b.path().to_path_buf()),
        "the open waits for the question"
    );
    assert!(
        matches!(m.dialog, Some(Dialog::OpenUnsaved(_))),
        "{:?}",
        m.dialog
    );
}
