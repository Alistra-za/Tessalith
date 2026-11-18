//! Review E export shell: the per-project "Export without review" key in `sdc-project.toml` (Task 1).
mod support;
use sdc_gui::model::Dialog;
use std::time::Duration;

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
        m.export_without_review(),
        "the dirty document holds the tick"
    );
    // An unchanged value leaves a status line that is already there alone.
    m.status_line = "an earlier note".into();
    m.set_export_without_review(true);
    assert_eq!(m.status_line, "an earlier note");
    assert!(
        !m.request_open(b.path().to_path_buf()),
        "the open waits for the question"
    );
    assert!(
        matches!(m.dialog, Some(Dialog::OpenUnsaved(_))),
        "{:?}",
        m.dialog
    );
    m.save().unwrap();
    let saved = std::fs::read_to_string(root.join("sdc-project.toml")).unwrap();
    assert!(saved.contains("export_without_review = true"), "{saved}");
}

#[test]
fn undo_and_redo_keep_the_opt_out() {
    let a = tempfile::tempdir().unwrap();
    let mut m = support::opened(a.path());
    let id = sdc_gui::model::sets::views(&m)[0].id.clone();
    m.rename_set(&id, &format!("{id}_x")).unwrap();
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_export_without_review(true);
    m.undo();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.export_without_review(), "undo keeps the preference");
    assert!(
        m.sets_doc.as_ref().unwrap().is_dirty(),
        "the tick is still unsaved"
    );
    m.redo();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.export_without_review(), "redo keeps the preference");
}

#[test]
fn the_opt_out_cannot_be_set_while_an_open_can_still_be_cancelled() {
    let a = tempfile::tempdir().unwrap();
    let od = tempfile::tempdir().unwrap();
    let root = support::mod_folder(a.path());
    let mut m = support::opened(a.path());
    // As `open_cancel.rs` holds an open in flight: open another folder and do not wait.
    m.open_folder(support::mod_folder(od.path()));
    assert!(m.can_cancel_open(), "guard: the open is still in flight");
    m.set_export_without_review(true);
    assert!(!m.export_without_review());
    let on_disk = std::fs::read_to_string(root.join("sdc-project.toml")).unwrap_or_default();
    assert!(!on_disk.contains("export_without_review"), "{on_disk}");
    assert_eq!(
        m.status_line,
        "Export without review can be set once the folder has finished opening."
    );
    m.cancel_open();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        !m.export_without_review(),
        "nothing was accepted, so nothing is lost"
    );
}
