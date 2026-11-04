mod support;
use sdc_gui::model::history::{Command, History, Snapshot};
use serde_json::json;
use support::*;

fn snap(a: &str, b: &str) -> Snapshot {
    Snapshot {
        sdc_toml: a.into(),
        project: b.into(),
    }
}

fn cmd(label: &str, before: Snapshot, after: Snapshot, merge: Option<&str>) -> Command {
    Command {
        label: label.into(),
        before,
        after,
        merge: merge.map(str::to_string),
        created_file: None,
    }
}

#[test]
fn history_undoes_redoes_merges_and_caps() {
    let mut h = History::default();
    h.push(cmd("a", snap("0", ""), snap("1", ""), None));
    h.push(cmd("slider", snap("1", ""), snap("2", ""), Some("x")));
    h.push(cmd("slider", snap("2", ""), snap("3", ""), Some("x")));
    assert_eq!(h.undo.len(), 2, "two steps with the same merge key are one");
    let c = h.undo_step().unwrap();
    assert_eq!(
        (c.before.sdc_toml.as_str(), c.after.sdc_toml.as_str()),
        ("1", "3")
    );
    assert!(h.can_redo());
    h.push(cmd("b", snap("1", ""), snap("9", ""), None));
    assert!(!h.can_redo(), "a new edit drops the redo list");
    for i in 0..(History::LIMIT + 10) {
        h.push(cmd("n", snap(&i.to_string(), ""), snap("x", ""), None));
    }
    assert_eq!(h.undo.len(), History::LIMIT);
}

#[test]
fn every_kind_of_edit_undoes_and_redoes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let item = m
        .items
        .iter()
        .find(|i| i.name == "rock01.png")
        .unwrap()
        .key
        .clone();
    // A [files] edit (sdc.toml).
    m.set_field(&item, "mips.levels", json!(3));
    assert_eq!(
        m.project.as_ref().unwrap().file_value(&item, "mips.levels"),
        Some(json!(3))
    );
    m.undo();
    assert_eq!(
        m.project.as_ref().unwrap().file_value(&item, "mips.levels"),
        None
    );
    m.redo();
    assert_eq!(
        m.project.as_ref().unwrap().file_value(&item, "mips.levels"),
        Some(json!(3))
    );
    // Mode.
    m.set_target(engine::config::Target::Vanilla);
    assert_eq!(
        m.project.as_ref().unwrap().target(),
        Some(engine::config::Target::Vanilla)
    );
    m.undo();
    assert_eq!(m.project.as_ref().unwrap().target(), None);
    // A sets-doc edit.
    m.edit_docs("hair", None, false, |_, s| {
        s.set_value("landscape/rock01", "hair", Some(json!(true)))
    });
    assert!(m
        .sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .any(|e| e.settings.contains_key("hair")));
    m.undo();
    assert!(!m
        .sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .any(|e| e.settings.contains_key("hair")));
    assert!(
        m.is_dirty(),
        "undo back past the save point stays an unsaved change"
    );
}

#[test]
fn undo_after_an_outside_edit_never_overwrites_silently() {
    use sdc_gui::model::{Dialog, DocKind};
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let root = m.root.clone().unwrap();
    m.edit_docs("hair", None, false, |_, s| {
        s.set_value("landscape/rock01", "hair", Some(json!(true)))
    });
    m.save().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(root.join("sdc-project.toml"), "# edited elsewhere\n").unwrap();
    m.undo();
    assert!(m.save().is_err());
    assert_eq!(
        m.dialog,
        Some(Dialog::ExternalChange(DocKind::Project)),
        "Review focus 4"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("sdc-project.toml")).unwrap(),
        "# edited elsewhere\n"
    );
}

#[test]
fn opening_another_folder_clears_the_history() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_target(engine::config::Target::Vanilla);
    assert!(m.can_undo());
    m.save().unwrap();
    m.open_folder(mod_folder(&d.path().join("b")));
    assert!(!m.can_undo() && !m.can_redo());
}

#[test]
fn an_edit_that_changes_nothing_is_not_a_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.edit_docs("noop", None, false, |_, _| {});
    assert!(!m.can_undo());
}

#[test]
fn sdc_toml_edits_undo_while_the_sets_doc_is_read_only() {
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    std::fs::write(root.join("sdc-project.toml"), "this is = = not toml\n").unwrap();
    let mut m =
        sdc_gui::model::AppModel::new(sdc_gui::settings::GuiSettings::default(), None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(m.sets_doc.as_ref().unwrap().read_only());
    m.set_target(engine::config::Target::Vanilla);
    assert_eq!(
        m.project.as_ref().unwrap().target(),
        Some(engine::config::Target::Vanilla)
    );
    assert!(m.can_undo());
    m.undo();
    assert_eq!(m.project.as_ref().unwrap().target(), None);
    m.redo();
    assert_eq!(
        m.project.as_ref().unwrap().target(),
        Some(engine::config::Target::Vanilla)
    );
    // The read-only sets doc was never dirtied or made writable by undo/redo.
    let s = m.sets_doc.as_ref().unwrap();
    assert!(s.read_only() && !s.is_dirty());
}

#[test]
fn undo_of_an_sdc_toml_edit_leaves_the_sets_doc_clean_and_save_writes_no_project_file() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_unchosen(d.path());
    let root = m.root.clone().unwrap();
    m.set_target(engine::config::Target::Vanilla);
    m.undo();
    assert!(!m.sets_doc.as_ref().unwrap().is_dirty());
    m.save().unwrap();
    assert!(!root.join("sdc-project.toml").exists());
    assert_eq!(m.dialog, None);
}

#[test]
fn undo_of_a_sets_only_edit_leaves_sdc_toml_clean() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.edit_docs("hair", None, false, |_, s| {
        s.set_value("landscape/rock01", "hair", Some(json!(true)))
    });
    m.undo();
    assert!(!m.project.as_ref().unwrap().is_dirty());
}

#[test]
fn a_plan_rejection_keeps_the_history() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let item = m
        .items
        .iter()
        .find(|i| i.name == "rock01.png")
        .unwrap()
        .key
        .clone();
    m.set_field(&item, "alpha.threshold", json!(1000));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(
        m.project.as_ref().unwrap().read_only(),
        "the engine rejected it"
    );
    // Note FC12 (supersedes BN1's clear): the rejected step stays, so Undo can take it back.
    assert!(m.can_undo() && !m.can_redo());
}

fn export_and_wait(m: &mut sdc_gui::model::AppModel) {
    export_through(m);
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
}

/// Final review I2: the JSON hash records the file on disk, which undo and redo do not change; restoring an older
/// `sdc-project.toml` text must not bring back an older hash (or none), or the next export asks to overwrite a JSON
/// sdc wrote itself.
#[test]
fn undo_and_redo_after_an_export_keep_the_json_hash() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_json_value("landscape/rock01", "specular_level", Some(json!(0.05)));
    export_and_wait(&mut m);
    assert!(m.dialog.is_none(), "{:?}", m.dialog);
    let sha = m.sets_doc.as_ref().unwrap().last_json_sha256();
    assert!(sha.is_some());

    m.undo();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert_eq!(m.sets_doc.as_ref().unwrap().last_json_sha256(), sha);
    export_and_wait(&mut m);
    assert!(m.dialog.is_none(), "after undo: {:?}", m.dialog);
    let sha = m.sets_doc.as_ref().unwrap().last_json_sha256();

    m.redo();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert_eq!(m.sets_doc.as_ref().unwrap().last_json_sha256(), sha);
    export_and_wait(&mut m);
    assert!(m.dialog.is_none(), "after redo: {:?}", m.dialog);
}

/// Undoing an `sdc.toml` edit made before an export leaves `sdc-project.toml` alone: only the hash differs from the
/// step's text, and that is kept, so the sets document stays saved.
#[test]
fn undoing_an_sdc_toml_edit_after_an_export_leaves_the_sets_doc_clean() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_field("landscape/rock01.png", "alpha.policy", json!("remove"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    export_and_wait(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().last_json_sha256().is_some());
    assert!(!m.sets_doc.as_ref().unwrap().is_dirty());
    m.undo();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(!m.sets_doc.as_ref().unwrap().is_dirty());
    assert!(m.sets_doc.as_ref().unwrap().last_json_sha256().is_some());
}

/// Audit 10 (DECISIONS Note Y8): an edit after an undo starts a new step, even with the merge key now on top.
#[test]
fn an_edit_after_undo_never_merges_into_the_step_below() {
    let mut h = History::default();
    h.push(cmd("name", snap("", "0"), snap("", "x"), Some("json_name")));
    h.push(cmd("shader", snap("", "x"), snap("", "xs"), None));
    h.push(cmd(
        "name",
        snap("", "xs"),
        snap("", "xsy"),
        Some("json_name"),
    ));
    h.undo_step();
    h.undo_step();
    h.push(cmd(
        "name",
        snap("", "x"),
        snap("", "xz"),
        Some("json_name"),
    ));
    assert_eq!(h.undo.len(), 2, "z is a step of its own");
    assert_eq!(h.undo[0].after.project, "x", "the x state is still a step");
    // Redo also ends a run.
    let mut h = History::default();
    h.push(cmd("name", snap("", "0"), snap("", "a"), Some("k")));
    h.undo_step();
    h.redo_step();
    h.push(cmd("name", snap("", "a"), snap("", "ab"), Some("k")));
    assert_eq!(h.undo.len(), 2);
}

/// Audit REPORT 2026-10-16 #2 (Note BD2): a set edit made while an export runs is not written by the hash record;
/// it stays unsaved (the doc dirty) until the user saves or discards it.
#[test]
fn an_edit_during_an_export_is_not_saved_by_the_hash_record() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_json_value("landscape/rock01", "specular_level", Some(json!(0.05)));
    export_through(&mut m);
    m.set_json_value("landscape/rock01", "specular_level", Some(json!(0.37)));
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    let doc = m.sets_doc.as_ref().unwrap();
    assert!(
        doc.last_json_sha256().is_some(),
        "the hash is recorded in memory"
    );
    assert!(doc.is_dirty(), "the edit is still unsaved");
    let disk = std::fs::read_to_string(doc.path()).unwrap();
    assert!(!disk.contains("0.37"), "{disk}");
    assert!(
        m.export
            .notes
            .iter()
            .any(|n| n.contains("written with the next Save")),
        "{:?}",
        m.export.notes
    );
    // The next Save writes the edit and the recorded hash.
    let sha = m.sets_doc.as_ref().unwrap().last_json_sha256().unwrap();
    m.save().unwrap();
    let doc = m.sets_doc.as_ref().unwrap();
    assert!(!doc.is_dirty());
    let disk = std::fs::read_to_string(doc.path()).unwrap();
    assert!(disk.contains("0.37") && disk.contains(&sha), "{disk}");
}

/// Backlog T37 (Note FP4): a step recorded while the engine's rejection blames `sdc.toml` keeps the stashed text, not
/// the empty document, so Undo and Redo through it never empty the file.
#[test]
fn a_step_recorded_while_sdc_toml_is_blamed_keeps_its_text() {
    let wait = std::time::Duration::from_secs(60);
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let item = m
        .items
        .iter()
        .find(|i| i.name == "rock01.png")
        .unwrap()
        .key
        .clone();
    m.set_field(&item, "alpha.threshold", json!(1000));
    assert!(m.wait_idle(wait));
    assert!(m.project.as_ref().unwrap().read_only(), "blamed");
    m.edit_docs("hair", None, false, |_, s| {
        s.set_value("landscape/rock01", "hair", Some(json!(true)))
    });
    let step = m.history.undo.last().unwrap();
    assert!(
        !step.before.sdc_toml.is_empty() && !step.after.sdc_toml.is_empty(),
        "before {:?} after {:?}",
        step.before.sdc_toml,
        step.after.sdc_toml
    );
    m.undo();
    assert!(m.wait_idle(wait));
    m.undo();
    assert!(m.wait_idle(wait));
    assert!(!m.project.as_ref().unwrap().read_only(), "undone");
    // Both redos before the plan lands: the second restores its step's text over a document not yet blamed again.
    m.redo();
    m.redo();
    assert!(m.wait_idle(wait));
    // Redone, the engine rejects the value again, so the text is the stashed one.
    assert!(
        m.project
            .as_ref()
            .unwrap()
            .snapshot_text()
            .contains("threshold"),
        "sdc.toml was emptied"
    );
}

/// Backlog T38 (Note FP7): while the engine's rejection of `sdc.toml` is still being retried, the unsaved edits the
/// blame stashed are what "Save" in the unsaved dialogs writes.
#[test]
fn save_in_the_open_dialog_writes_the_stashed_sdc_toml_edits() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let item = m
        .items
        .iter()
        .find(|i| i.name == "rock01.png")
        .unwrap()
        .key
        .clone();
    m.set_field(&item, "alpha.threshold", json!(1000));
    // The state while the retry plan is still running (held, not raced).
    m.blame_sdc_toml_while_retrying("rejected");
    let stashed = m.project.as_ref().unwrap().stashed_edits();
    let Some(stashed) = stashed else {
        panic!("the retry plan landed before the test could act");
    };
    assert!(m.is_dirty(), "the stashed edits count as unsaved");
    let toml = m.root.clone().unwrap().join("sdc.toml");
    let other = d.path().join("other");
    std::fs::create_dir_all(&other).unwrap();
    assert!(m.save().is_ok(), "Save succeeded");
    assert_eq!(std::fs::read_to_string(&toml).unwrap_or_default(), stashed);
    assert!(!m.is_dirty(), "nothing left unsaved");
    // The dialog's Save then goes on to open the other folder.
    m.resolve_open_unsaved(other.clone(), true);
    assert_eq!(m.dialog, None);
    assert_eq!(m.root.as_deref(), Some(other.as_path()));
}
