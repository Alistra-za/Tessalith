//! Audit 7 (DECISIONS Note Y7): the set views are built once per change, and follow every change.
mod support;
use sdc_gui::model::sets::views;
use std::sync::Arc;
use std::time::Duration;
use support::*;

#[test]
fn set_views_are_cached_until_an_input_changes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let a = views(&m);
    assert!(Arc::ptr_eq(&a, &views(&m)), "no change, no rebuild");
    // A document edit.
    m.rename_set("landscape/rock01", "landscape/stone").unwrap();
    assert!(!Arc::ptr_eq(&a, &views(&m)));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(views(&m).iter().any(|v| v.id == "landscape/stone"));
    // Undo restores the document without an edit_docs call.
    m.undo();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(!views(&m).iter().any(|v| v.id == "landscape/stone"));
    // The export folder.
    let before = views(&m);
    m.set_export_folder(d.path().join("elsewhere"));
    assert!(!Arc::ptr_eq(&before, &views(&m)));
    // An in-place write, announced.
    let before = views(&m);
    m.invalidate_views();
    assert!(!Arc::ptr_eq(&before, &views(&m)));
}

#[test]
fn every_sets_doc_edit_changes_its_revision() {
    let d = tempfile::tempdir().unwrap();
    let mut s = sdc_gui::sets_doc::SetsDoc::open(d.path());
    let mut last = s.rev();
    let mut step = |s: &mut sdc_gui::sets_doc::SetsDoc, what: &str| {
        assert_ne!(s.rev(), last, "{what}");
        last = s.rev();
    };
    s.create_set("a/x");
    step(&mut s, "create_set");
    s.set_order(&["a/x".into()]);
    step(&mut s, "set_order");
    s.set_json_name(Some("n"));
    step(&mut s, "set_json_name");
    let t = s.text();
    s.restore(&t);
    step(&mut s, "restore");
    s.reload();
    step(&mut s, "reload");
    assert_ne!(
        sdc_gui::sets_doc::SetsDoc::open(d.path()).rev(),
        last,
        "a new document never reuses a revision"
    );
}

#[test]
fn reload_changes_the_revision_of_both_documents() {
    let d = tempfile::tempdir().unwrap();
    let mut p = sdc_gui::project::ProjectDoc::open(d.path());
    let mut s = sdc_gui::sets_doc::SetsDoc::open(d.path());
    let (pr, sr) = (p.rev(), s.rev());
    p.reload();
    s.reload();
    assert_ne!(p.rev(), pr, "sdc.toml");
    assert_ne!(s.rev(), sr, "sdc-project.toml");
}

/// The cached problem list is kept while nothing moved and rebuilt when any input of the view key moves, including
/// `output_root`'s own inputs, which `view_key_matches` reads from a memo (Task 5 fix round 3).
#[test]
fn the_problem_list_follows_every_input_of_the_view_key() {
    use sdc_gui::model::bottom::problems;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_target(engine::config::Target::Cs);
    assert!(m.wait_idle(Duration::from_secs(60)));
    let mut last = problems(&m);
    assert!(Arc::ptr_eq(&last, &problems(&m)), "no change, no rebuild");
    let mut moved = |m: &sdc_gui::model::AppModel, what: &str| {
        let now = problems(m);
        assert!(
            !Arc::ptr_eq(&last, &now),
            "{what}: the list was not rebuilt"
        );
        assert!(
            Arc::ptr_eq(&now, &problems(m)),
            "{what}: and it is kept after"
        );
        last = now;
    };
    // The export folder (output_root's own input).
    m.set_export_folder(d.path().join("elsewhere"));
    moved(&m, "export folder");
    // The scan database.
    m.scan_db = Some(d.path().join("scan.db"));
    moved(&m, "scan database");
    // An Also tick moves output_root through `sdc.toml`.
    m.set_also_tick(engine::presets::AlsoTarget::Vanilla, true);
    assert!(m.wait_idle(Duration::from_secs(60)));
    moved(&m, "also tick");
    // The target: the Also tick only counts in cs mode, so `output_root` moves with it.
    m.set_target(engine::config::Target::Vanilla);
    assert!(m.wait_idle(Duration::from_secs(60)));
    moved(&m, "target");
    // A read-only `sdc.toml` (`export_project` is then none, so the Also folder is not the output root).
    m.set_target(engine::config::Target::Cs);
    assert!(m.wait_idle(Duration::from_secs(60)));
    moved(&m, "target back");
    // (`sdc.toml` is read-only to the GUI while it does not parse.)
    m.save().unwrap();
    std::fs::write(m.root.clone().unwrap().join("sdc.toml"), "= = [").unwrap();
    m.project.as_mut().unwrap().reload();
    assert!(m.project.as_ref().unwrap().read_only(), "premise");
    moved(&m, "read-only flag");
    // The opened folder.
    let other = d.path().join("other");
    std::fs::create_dir_all(other.join("textures")).unwrap();
    m.open_folder(other);
    assert!(m.wait_idle(Duration::from_secs(60)));
    moved(&m, "opened folder");
}
