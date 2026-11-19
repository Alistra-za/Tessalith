//! Review E export shell: the per-project "Export without review" key in `sdc-project.toml` (Task 1).
mod support;
use eframe::egui;
use egui_kittest::Harness;
use engine::presets::AlsoTarget;
use sdc_gui::model::{AppModel, Dialog};
use std::time::Duration;

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_step_dt(0.25)
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        )
}

fn settle(h: &mut Harness<'static, AppModel>) {
    assert!(h.state_mut().wait_idle(Duration::from_secs(120)));
    h.run();
}

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

#[test]
fn the_header_click_opens_review_on_a_clean_project() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path()); // 4 outputs, no problem row
    assert_eq!(m.header_export_text(), "Review 4 files…");
    m.header_export_clicked();
    assert!(m.review.is_some(), "the header never writes by default");
    assert!(m.export.running.is_none());
}

#[test]
fn with_the_opt_out_a_clean_project_exports_now() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    assert_eq!(m.header_export_text(), "Export 4 files now");
    m.header_export_clicked();
    assert!(m.review.is_none() && m.export.running.is_some());
}

// Review Focus 1: each trigger, under the opt-out, opens Review and writes nothing.
#[test]
fn the_opt_out_never_skips_an_edited_json() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _json) = support::pbr_with_edited_json(d.path()); // moved from review.rs
    m.set_export_without_review(true);
    m.expire_review_json(); // no stale 1 s cache from before the edit
    assert!(m.header_export_text().starts_with("Review "));
    m.header_export_clicked();
    assert!(m.review.is_some() && m.export.running.is_none());
}

#[test]
fn the_opt_out_never_skips_an_edited_json_on_a_row_free_project() {
    // `pbr_with_edited_json`'s fixture also keeps warning rows, which alone keep Review. This one has none, so the
    // edited PGPatcher JSON is the only thing that can open Review.
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/pbr/arm");
    support::colour(&t.join("steel.png"), 200);
    support::flat(&t.join("steel_n.png"));
    support::colour(&t.join("steel_rmaos.png"), 30);
    let mut m = support::opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_export_without_review(true);
    assert!(
        !m.has_problem_rows() && m.undecided() == 0,
        "guard: row-free"
    );
    assert!(
        m.header_export_text().ends_with(" now"),
        "guard: {}",
        m.header_export_text()
    );
    m.header_export_clicked();
    assert!(m.wait_idle(Duration::from_secs(120)));
    let json = support::pgp_json_path(&m);
    assert!(json.is_file(), "{:?}", m.export.notes);
    std::fs::write(&json, "{ \"edited\": true }\n").unwrap();
    m.expire_review_json();
    assert!(
        m.header_export_text().starts_with("Review "),
        "{}",
        m.header_export_text()
    );
    // With Review open and the question answered, the gate lets the JSON pass (`unanswered_overwrites` is empty), so
    // only the rule's own JSON clause keeps the label on Review.
    m.open_review();
    m.answer_review_json(false);
    assert!(
        m.review_gate().is_ok(),
        "guard: answered, so the gate is open"
    );
    assert!(
        m.header_export_text().starts_with("Review "),
        "{}",
        m.header_export_text()
    );
    m.close_review();
    m.header_export_clicked();
    assert!(m.review.is_some() && m.export.running.is_none());
}

/// Fix round 1, finding 3 (ruling 3c-18): write dispatch reads the JSON state fresh. The label may use the 1 s cache,
/// the click may not: a JSON edited inside the cache's second still opens Review.
#[test]
fn the_click_reads_an_edited_json_fresh_inside_the_cache_second() {
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/pbr/arm");
    support::colour(&t.join("steel.png"), 200);
    support::flat(&t.join("steel_n.png"));
    support::colour(&t.join("steel_rmaos.png"), 30);
    let mut m = support::opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_export_without_review(true);
    m.header_export_clicked();
    assert!(m.wait_idle(Duration::from_secs(120)));
    let json = support::pgp_json_path(&m);
    assert!(json.is_file(), "{:?}", m.export.notes);
    m.expire_review_json();
    // The label warms the cache with the JSON clean...
    assert!(
        m.header_export_text().ends_with(" now"),
        "guard: {}",
        m.header_export_text()
    );
    // ...then the file is edited and the click comes inside the cache's second (no clock advance, no expiry).
    std::fs::write(&json, "{ \"edited\": true }\n").unwrap();
    assert!(
        m.header_export_text().ends_with(" now"),
        "guard: the label still reads the cache"
    );
    m.header_export_clicked();
    assert!(
        m.review.is_some() && m.export.running.is_none(),
        "the click wrote past an edited JSON"
    );
}

#[test]
fn the_opt_out_never_skips_an_also_tick() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    m.set_also_tick(AlsoTarget::Vanilla, true);
    // Guard: ticking is refused outside the cs mode (`also.rs`, NEEDS_CS). The fixture plans in the cs mode (the
    // model's default target is Cs; `also_export.rs` sets Vanilla by hand to leave it).
    assert!(m.also_on(), "the tick took: {}", m.status_line);
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.header_export_text(), "Review 4 files…");
    m.header_export_clicked();
    assert!(m.review.is_some() && m.export.running.is_none());
}

#[test]
fn the_opt_out_never_skips_a_nif_edit() {
    // The rule this pins is `!nif_rows_written()` (any row not blocked). A row with no foreign file and no question
    // proves it is the NIF-edit rule, not an overwrite question, that opens Review.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    m.set_nif_review_rows(vec![support::nif_row("meshes\\x.nif", None)]); // foreign None, answer None
    assert!(
        m.unanswered_overwrites().is_empty(),
        "no overwrite question in this case"
    );
    assert!(m.header_export_text().starts_with("Review "));
    m.header_export_clicked();
    assert!(m.review.is_some() && m.export.running.is_none());
}

#[test]
fn the_opt_out_ignores_check_rows() {
    // Note CL: a Check row counts in the tab, never as a problem row.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    support::give_a_check_at(&mut m, &d.path().join("textures/g/a_n.png"));
    m.set_export_without_review(true);
    assert_eq!(m.header_export_text(), "Export 4 files now");
}

// Review Focus 2 and controller ruling 1.
#[test]
fn the_header_label_holds_while_a_plan_is_pending() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    support::add_warn_set(d.path(), "w");
    m.replan(); // `check_sources` alone starts no plan for a new file
    assert!(m.full_plan_pending());
    // Fails before the fix: `export_is_clean()` is false while planning, so the label read "Review 4 files…".
    assert_eq!(m.header_export_text(), "Export 4 files now");
    // Ruling 2: the count holds too. A partial plan (fast_replan) lands before the full one and may already count
    // the new w set's file; the label must not move until the full plan settles.
    let end = std::time::Instant::now() + Duration::from_secs(60);
    while m.full_plan_pending() && std::time::Instant::now() < end {
        m.poll();
        if !m.full_plan_pending() {
            break; // this poll settled the plan: the label may now change
        }
        assert_eq!(
            m.header_export_text(),
            "Export 4 files now",
            "moved while pending"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        m.header_export_text().starts_with("Review "),
        "settled with an undecided alpha"
    );
}

/// The fixture's undecided diffuse brings a writable normal with it, so the export count goes from four to five and
/// the gate's count compare alone sends the click to Review. Removing the E3 eligibility read from the gate does NOT
/// fail this test (`export_stopped` has no undecided-texture check); removing the Review fallback does (round 1
/// mutation logs `task-2-fix1-mut-*`). The eligibility read's own test is
/// `a_queued_click_opens_review_when_the_landed_plan_needs_eyes_at_an_unchanged_count`.
#[test]
fn a_queued_now_click_opens_review_when_the_plan_brings_a_problem() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    support::add_warn_set(d.path(), "w");
    m.replan(); // `check_sources` alone starts no plan for a new file
    assert!(m.full_plan_pending());
    m.header_export_clicked();
    assert!(m.review.is_none(), "queued: nothing opens yet");
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        m.review.is_some(),
        "the landed plan has an undecided alpha: Review opens"
    );
    assert!(m.export.running.is_none() && !d.path().join("export/textures/g/a.dds").exists());
}

#[test]
fn a_queued_now_click_writes_when_the_plan_stays_clean() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    m.replan();
    assert!(m.full_plan_pending());
    m.header_export_clicked();
    assert!(m.wait_idle(Duration::from_secs(120)));
    assert!(m.review.is_none());
    assert!(d.path().join("export/textures/g/a.dds").is_file());
}

#[test]
fn a_queued_now_click_opens_review_when_the_count_changed() {
    // Review round 3, Important 1: a clean file added mid-plan changes the count. The label said 4; the landed plan
    // has 6 (b.png and b_n.png, both clean). Nothing the user was not shown is written.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    support::add_ready_set(d.path(), "b"); // colour + flat _n, nothing to decide
    m.replan(); // `check_sources` alone starts no plan for a new file
    assert!(m.full_plan_pending());
    assert_eq!(m.header_export_text(), "Export 4 files now");
    m.header_export_clicked();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.writes_without_review(), "guard: the landed plan is clean");
    assert_ne!(m.export_files(), 4, "guard: the count changed");
    assert!(m.review.is_some(), "a different count opens Review");
    assert!(m.export.running.is_none() && !d.path().join("export/textures/g/a.dds").exists());
}

#[test]
fn a_write_now_click_on_a_changed_source_plans_first() {
    // The click never writes on a stale plan: the changed source plans again, and the write waits for that plan.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    support::colour(&d.path().join("textures/g/a.png"), 91);
    m.header_export_clicked();
    assert!(
        m.export.running.is_none() && m.export_queued(),
        "queued behind a plan of the new source"
    );
    assert!(m.wait_idle(Duration::from_secs(120)));
    assert!(m.review.is_none(), "{:?}", m.dialog);
    assert!(d.path().join("export/textures/g/a.dds").is_file());
}

/// Fix round 1, finding 1: a header write queued behind a scan-finished read runs the source recheck Review's
/// continuation runs (`continue_review`) before it writes. A source that changed while the read was pending plans
/// again first; nothing is written against the old plan.
#[test]
fn a_queued_now_click_rechecks_the_sources_when_the_read_lands() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    drop(modscan::store::Store::open(&db).unwrap());
    m.scan_db = Some(db);
    m.refresh_mesh_votes(false);
    assert!(m.votes_pending(), "guard: a scan-finished read is pending");
    assert_eq!(m.header_export_text(), "Export 4 files now", "guard");
    m.header_export_clicked();
    assert!(m.review.is_none() && m.export.running.is_none(), "queued");
    support::colour(&d.path().join("textures/g/a.png"), 91);
    // The replan the landing starts has its result held, so the state right after the read is the test's to look at.
    m.hold_plan_results = true;
    let end = std::time::Instant::now() + Duration::from_secs(60);
    while m.votes_pending() {
        assert!(std::time::Instant::now() < end, "the read never landed");
        m.poll();
    }
    assert!(
        m.export.running.is_none() && m.export.summary.is_none(),
        "the read landed over a changed source: nothing may start on the old plan"
    );
    assert!(m.full_plan_pending(), "the changed source plans again");
    let msgs = held_plan(&mut m);
    land_plan(&mut m, msgs);
    assert!(m.review.is_none(), "{:?}", m.dialog);
    assert!(d.path().join("export/textures/g/a.dds").is_file());
}

/// A clean project with the opt-out on and a scan-finished read pending, the header write queued behind it.
fn queued_behind_a_read(dir: &std::path::Path) -> (AppModel, tempfile::TempDir) {
    let mut m = support::opened_chosen_decided(dir);
    m.set_export_without_review(true);
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    drop(modscan::store::Store::open(&db).unwrap());
    m.scan_db = Some(db);
    m.refresh_mesh_votes(false);
    assert!(m.votes_pending(), "guard: a scan-finished read is pending");
    assert_eq!(m.header_export_text(), "Export 4 files now", "guard");
    m.header_export_clicked();
    assert!(m.review.is_none() && m.export.running.is_none(), "queued");
    (m, dbdir)
}

fn assert_cancelled_with_the_note(m: &AppModel) {
    assert!(
        m.review.is_none() && m.dialog.is_none() && m.export.running.is_none(),
        "{:?}",
        m.dialog
    );
    assert!(
        m.status_line
            .contains("Export cancelled: the project changed while scan results were loading"),
        "{}",
        m.status_line
    );
    assert!(m
        .messages
        .entries()
        .any(|e| e.text.starts_with("Export cancelled: the project changed")));
}

/// Fix round 1, finding 2: the header write the opt-out queues is cancelled by an edit during a scan-finished read,
/// like the queued Export is (DECISIONS AF4), with the same note. Twin of
/// `sets_model::an_edit_during_a_scan_finished_read_drops_the_queued_export`.
#[test]
fn an_edit_during_a_scan_finished_read_drops_the_queued_header_write() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _db) = queued_behind_a_read(d.path());
    m.set_field("g/a.png", "header", serde_json::json!("dx10"));
    assert!(m.wait_idle(Duration::from_secs(120)));
    assert_cancelled_with_the_note(&m);
}

/// Fix round 1, finding 2: an undo cancels it the same way (twin of
/// `sets_model::an_undo_during_a_scan_finished_read_cancels_the_queued_export`).
#[test]
fn an_undo_during_a_scan_finished_read_cancels_the_queued_header_write() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_field("g/a.png", "header", serde_json::json!("dx10"));
    assert!(m.wait_idle(Duration::from_secs(120)));
    // Re-open the queue on top of the edited project.
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    drop(modscan::store::Store::open(&db).unwrap());
    m.scan_db = Some(db);
    m.set_export_without_review(true);
    m.refresh_mesh_votes(false);
    assert!(m.votes_pending(), "guard: a scan-finished read is pending");
    assert!(
        m.header_export_text().ends_with(" now"),
        "guard: {}",
        m.header_export_text()
    );
    m.header_export_clicked();
    assert!(m.review.is_none() && m.export.running.is_none(), "queued");
    m.undo();
    assert!(m.wait_idle(Duration::from_secs(120)));
    assert_cancelled_with_the_note(&m);
}

#[test]
fn a_write_now_click_still_runs_the_export_guards() {
    // `writes_without_review` does not look at the export folder's location; the click's guards (`export_stopped`)
    // do, as every export's. Here the export folder is the source folder itself.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    support::choose_export(&mut m, d.path().join("textures"));
    assert_eq!(m.header_export_text(), "Export 4 files now", "guard");
    m.header_export_clicked();
    assert!(
        matches!(m.dialog, Some(Dialog::Error(_))),
        "{:?} / {}",
        m.dialog,
        m.status_line
    );
    assert!(m.export.running.is_none());
}

#[test]
fn a_blocked_nif_row_does_not_stop_a_write_now_and_is_not_counted_as_written() {
    // Note FI7 (4a, round 2), E3: a blocked row is never written, so it keeps the project writing at once; the
    // write-now label names no NIF edit and no NIF export follows.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    let mut row = support::nif_row("meshes\\x.nif", None);
    row.blocked = Some("its NIF changed on disk".into());
    m.set_nif_review_rows(vec![row]);
    assert_eq!(m.header_export_text(), "Export 4 files now");
    m.header_export_clicked();
    assert!(m.review.is_none() && m.export.running.is_some());
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(!m.take_nif_export_due(), "no NIF edit was written");
}

/// The set key of `id` in `m`.
fn key_of(m: &AppModel, id: &str) -> String {
    sdc_gui::model::sets::set_views(m)
        .iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("no set {id}"))
        .key
        .clone()
}

// Ruling 3c-14: the header is always the whole project's, whatever scope an open Review was made for.
#[test]
fn the_header_is_the_whole_projects_while_a_scoped_review_is_open() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let u = key_of(&m, "g/u");
    m.export_sets([u].into(), "Export set g/u".into());
    assert!(
        m.review.is_some() && m.export_scope().is_some(),
        "a scoped Review is open"
    );
    assert_eq!(m.export_files(), 2, "guard: the scope has 2 of the 4 files");
    // Default setting: the label names the whole project, and a click opens the whole project's Review.
    assert_eq!(m.header_export_text(), "Review 4 files…");
    m.header_export_clicked();
    assert!(
        m.review.is_some() && m.export_scope().is_none(),
        "the open Review is now the whole project's"
    );
    assert_eq!(m.review_button_text(), "Export 4 files");
    assert!(m.export.running.is_none());
    m.close_review();
    // With the opt-out: the label and the click are the whole project's too.
    let u = key_of(&m, "g/u");
    m.export_sets([u].into(), "Export set g/u".into());
    m.set_export_without_review(true);
    assert!(
        m.review.is_some() && m.export_scope().is_some(),
        "guard: a scoped Review is open"
    );
    assert_eq!(m.header_export_text(), "Export 4 files now");
    m.header_export_clicked();
    assert!(m.export.running.is_some(), "the click writes");
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        m.review.is_none(),
        "an export that starts closes the open Review"
    );
    assert!(m.export_scope().is_none());
    for f in ["a", "a_n", "u", "u_n"] {
        assert!(
            d.path()
                .join(format!("export/textures/g/{f}.dds"))
                .is_file(),
            "{f}"
        );
    }
}

/// Round 2: wait until the held plan result has arrived (the plan stays pending until the test delivers it).
fn held_plan(m: &mut AppModel) -> Vec<sdc_gui::worker::WorkerMsg> {
    let end = std::time::Instant::now() + Duration::from_secs(60);
    let mut got = vec![];
    loop {
        m.poll();
        got.extend(m.take_held());
        if got
            .iter()
            .any(|x| matches!(x, sdc_gui::worker::WorkerMsg::Planned { .. }))
        {
            return got;
        }
        assert!(std::time::Instant::now() < end, "the plan never arrived");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Deliver the held plan messages, let every later message through and settle.
fn land_plan(m: &mut AppModel, msgs: Vec<sdc_gui::worker::WorkerMsg>) {
    for x in msgs {
        m.handle_held(x);
    }
    m.hold_plan_results = false;
    assert!(m.wait_idle(Duration::from_secs(120)));
}

fn nothing_written(d: &std::path::Path) -> bool {
    !d.join("export/textures/g/a.dds").exists() && !d.join("export/textures/g/u.dds").exists()
}

/// Round 2, ruling 3c-25 (finding 5, B5): a scoped click never arms a write. With Review open and a plan pending, a
/// click on Export set opens its scoped Review at once; pressing Close before the plan lands must not leave a
/// whole-project write behind.
#[test]
fn a_scoped_click_during_a_plan_never_leaves_a_write_armed_for_close() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    m.open_review();
    m.hold_plan_results = true;
    m.replan();
    assert!(m.full_plan_pending(), "guard: a plan is pending");
    let k = key_of(&m, "g/u");
    m.export_sets([k].into(), "Export set g/u".into());
    assert!(
        m.export_scope().is_some(),
        "guard: the scoped Review is open"
    );
    assert!(!m.export_queued(), "a scoped click queues nothing");
    m.close_review();
    let msgs = held_plan(&mut m);
    land_plan(&mut m, msgs);
    assert!(
        nothing_written(d.path()) && m.export.running.is_none() && m.export.summary.is_none(),
        "the landed plan wrote the whole project"
    );
}

/// Round 2, ruling 3c-25: a scoped click whose sources changed opens its scoped Review instead of queueing a write
/// and planning again.
#[test]
fn a_scoped_click_on_a_changed_source_opens_its_review() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    support::colour(&d.path().join("textures/g/a.png"), 91);
    let k = key_of(&m, "g/u");
    m.export_sets([k].into(), "Export set g/u".into());
    assert!(
        m.review.is_some() && m.export_scope().is_some(),
        "the scoped Review opens"
    );
    assert!(!m.export_queued(), "nothing is queued");
    assert!(m.export.running.is_none() && nothing_written(d.path()));
}

/// A row-free PBR project (steel) opted out and exported once, so its PGPatcher JSON is Tessalith's and clean; the
/// exported DDS files are removed again so a later write shows. Returns the model and the JSON's path.
fn exported_row_free_pbr(d: &std::path::Path) -> (AppModel, std::path::PathBuf) {
    let t = d.join("textures/pbr/arm");
    support::colour(&t.join("steel.png"), 200);
    support::flat(&t.join("steel_n.png"));
    support::colour(&t.join("steel_rmaos.png"), 30);
    let mut m = support::opened_as_the_app(d.join("textures"), d.join("export"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_export_without_review(true);
    m.header_export_clicked();
    assert!(m.wait_idle(Duration::from_secs(120)));
    let json = support::pgp_json_path(&m);
    assert!(json.is_file(), "{:?}", m.export.notes);
    for f in ["steel", "steel_n", "steel_rmaos"] {
        let _ = std::fs::remove_file(d.join(format!("export/textures/pbr/arm/{f}.dds")));
    }
    assert!(m.header_export_text().ends_with(" now"), "guard");
    (m, json)
}

fn steel_written(d: &std::path::Path) -> bool {
    d.join("export/textures/pbr/arm/steel.dds").exists()
}

/// Round 2, ruling 3c-23 (B1): a queued click's gate walks the sources BEFORE it reads eligibility. Here the landed
/// plan finds a changed source and an edited JSON together: the walk plans again (the click stays queued), nothing is
/// written on the old plan; with the JSON restored the second plan writes.
#[test]
fn a_queued_click_walks_the_sources_before_it_reads_eligibility() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, json) = exported_row_free_pbr(d.path());
    m.hold_plan_results = true;
    m.replan();
    assert!(m.full_plan_pending(), "guard");
    assert_eq!(m.header_export_text(), "Export 3 files now", "guard");
    m.header_export_clicked();
    assert!(m.export_queued(), "guard: queued behind the plan");
    let original = std::fs::read(&json).unwrap();
    // The plan has been computed and is parked: what changes now is newer than that plan.
    let msgs = held_plan(&mut m);
    support::colour(&d.path().join("textures/pbr/arm/steel.png"), 90);
    std::fs::write(&json, "{ \"edited\": true }\n").unwrap();
    for x in msgs {
        m.handle_held(x);
    }
    assert!(
        m.full_plan_pending() && m.export_queued(),
        "the landed plan met a changed source: it plans again, the click stays queued"
    );
    assert!(m.export.running.is_none() && !steel_written(d.path()));
    std::fs::write(&json, &original).unwrap();
    let msgs = held_plan(&mut m);
    land_plan(&mut m, msgs);
    assert!(m.review.is_none(), "{:?}", m.dialog);
    assert!(steel_written(d.path()), "the second plan writes");
}

/// Round 2, evidence rule (B6): the E3 eligibility read alone sends a queued click to Review. The count the label
/// showed (4) is unchanged at landing, so the count compare cannot save it; a NIF edit that would be written appears
/// meanwhile. Removing the eligibility read from the gate fails this test (mutation log
/// `task-2-fix2-mut-eligibility-removed.log`).
#[test]
fn a_queued_click_opens_review_when_the_landed_plan_needs_eyes_at_an_unchanged_count() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    m.hold_plan_results = true;
    m.replan();
    assert_eq!(m.header_export_text(), "Export 4 files now", "guard");
    m.header_export_clicked();
    assert!(m.export_queued(), "guard: queued behind the plan");
    let msgs = held_plan(&mut m);
    m.set_nif_review_rows(vec![support::nif_row("meshes\\x.nif", None)]);
    land_plan(&mut m, msgs);
    assert_eq!(m.export_files(), 4, "guard: the count is unchanged");
    assert!(m.review.is_some(), "a NIF edit to write opens Review");
    assert!(m.export.running.is_none() && nothing_written(d.path()));
}

/// Round 2, ruling 3c-23 (B3): the eligibility read comes AFTER the source walk. The JSON is edited while the walk
/// runs (a hook between the walk and the read): Review opens, nothing is written.
#[test]
fn the_eligibility_read_comes_after_the_source_walk() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, json) = exported_row_free_pbr(d.path());
    m.set_after_walk(move || std::fs::write(&json, "{ \"edited\": true }\n").unwrap());
    m.header_export_clicked();
    assert!(
        m.review.is_some() && m.export.running.is_none() && !steel_written(d.path()),
        "the click wrote on an answer read before the walk"
    );
}

/// Round 2, ruling 3c-23 (B1): a click that already waited through its own source replan does not replan again when
/// a source changes once more: Review opens, nothing is written, no third plan starts.
#[test]
fn a_click_that_waited_through_its_own_replan_never_replans_twice() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    m.hold_plan_results = true;
    support::colour(&d.path().join("textures/g/a.png"), 91);
    m.header_export_clicked();
    assert!(
        m.full_plan_pending() && m.export_queued(),
        "guard: the changed source plans first and the click waits"
    );
    let msgs = held_plan(&mut m);
    support::colour(&d.path().join("textures/g/a.png"), 92);
    for x in msgs {
        m.handle_held(x);
    }
    assert!(!m.full_plan_pending(), "no third plan");
    assert!(
        m.review.is_some() && !m.export_queued(),
        "the second change opens Review"
    );
    assert!(m.export.running.is_none() && nothing_written(d.path()));
}

/// Ruling 3c-26: what a waiting whole-project click does with ONE thing done between the click and the landing.
#[derive(Debug, Clone, Copy)]
enum Between {
    Nothing,
    ExportSetThenClose,
    ExportSetLeftOpen,
    ExportFolderMoved,
    /// Round 4 (3c-33): the export folder is a junction to A, retargeted to B (outside the target mod): the same path.
    ExportJunctionRetargeted,
    TargetModSet,
    ReencodeAllTicked,
    ReviewOpenedByHand,
    DialogOpened,
}

/// Arm a write-now click on a clean pending plan (its result held), do `between`, deliver the plan, and list what is
/// wrong with the outcome (empty: right). Nothing but the positive row may write, and nothing may stay armed.
fn after_a_waiting_click(between: Between) -> Vec<String> {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    let (link, a, b) = (
        d.path().join("export link"),
        d.path().join("out a"),
        d.path().join("out b"),
    );
    if matches!(between, Between::ExportJunctionRetargeted) {
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        dir_link(&link, &a);
        support::choose_export(&mut m, link.clone());
    }
    m.hold_plan_results = true;
    m.replan();
    assert!(
        m.full_plan_pending(),
        "{between:?}: guard: a plan is pending"
    );
    assert_eq!(m.header_export_text(), "Export 4 files now", "{between:?}");
    m.header_export_clicked();
    assert!(m.export_queued(), "{between:?}: guard: the click is armed");
    // The click's plan is computed and parked; what happens now is newer than it.
    let msgs = held_plan(&mut m);
    let moved = d.path().join("export2");
    match between {
        Between::Nothing => {}
        Between::ExportSetThenClose => {
            let k = key_of(&m, "g/u");
            m.export_sets([k].into(), "Export set g/u".into());
            m.close_review();
        }
        Between::ExportSetLeftOpen => {
            let k = key_of(&m, "g/u");
            m.export_sets([k].into(), "Export set g/u".into());
        }
        // Another folder, the same four files: only the destination differs.
        Between::ExportFolderMoved => m.set_export_folder(moved.clone()),
        Between::ExportJunctionRetargeted => {
            std::fs::remove_dir(&link).unwrap();
            dir_link(&link, &b);
        }
        Between::TargetModSet => {
            let t = d.path().join("target mod");
            std::fs::create_dir_all(&t).unwrap();
            m.set_target_mod(Some(t)).unwrap();
        }
        // The header's Re-encode all tick: a plain field, no plan, the same count.
        Between::ReencodeAllTicked => m.reencode_all = true,
        Between::ReviewOpenedByHand => m.open_review(),
        Between::DialogOpened => m.dialog = Some(Dialog::Error("a question".into())),
    }
    // The click's plan lands (a plan the action started supersedes it and lands through the channel).
    land_plan(&mut m, msgs);
    let mut wrong = vec![];
    let wrote = !nothing_written(d.path())
        || moved.join("textures/g/a.dds").exists()
        || a.join("textures/g/a.dds").exists()
        || b.join("textures/g/a.dds").exists()
        || m.export.summary.is_some();
    if m.export_queued() {
        wrong.push("a click is still armed".to_string());
    }
    match between {
        Between::Nothing => {
            if !d.path().join("export/textures/g/a.dds").is_file() || m.review.is_some() {
                wrong.push(format!("the untouched click did not write: {:?}", m.dialog));
            }
        }
        Between::DialogOpened => {
            if wrote {
                wrong.push("wrote under an open dialog".into());
            }
            if !m.status_line.contains("answer the open dialog") {
                wrong.push(format!("no warning: {}", m.status_line));
            }
        }
        _ => {
            if wrote {
                wrong.push("wrote what the user was not shown".into());
            }
            if m.review.is_none() {
                wrong.push("no Review is open".into());
            }
        }
    }
    if matches!(between, Between::ExportSetLeftOpen) && m.export_scope().is_none() {
        wrong.push("the open scoped Review lost its scope".into());
    }
    wrong
        .into_iter()
        .map(|w| format!("{between:?}: {w}"))
        .collect()
}

/// Round 3, ruling 3c-26: a waiting click is checked when it lands, against what it was made for (the scope, the
/// destination), and never lands into an open Review or an open dialog. One row per thing done while it waits.
#[test]
fn a_waiting_click_writes_only_when_nothing_changed_what_it_was_made_for() {
    let wrong: Vec<String> = [
        Between::Nothing,
        Between::ExportSetThenClose,
        Between::ExportSetLeftOpen,
        Between::ExportFolderMoved,
        Between::ExportJunctionRetargeted,
        Between::TargetModSet,
        Between::ReencodeAllTicked,
        Between::ReviewOpenedByHand,
        Between::DialogOpened,
    ]
    .into_iter()
    .flat_map(after_a_waiting_click)
    .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Round 3, ruling 3c-27 (re-review B1): the dialog check is the gate's first check for a click that did not wait
/// too. A header click deferred one frame for a focused text field (Note FC8, `ui::after_commit`) runs first thing
/// next frame, in `actions::keys`, before its modal guard; a dialog that appeared in between drops it with the queued
/// click's warning.
#[test]
fn a_deferred_click_never_writes_over_a_dialog_that_appeared_meanwhile() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    let mut h = harness(m);
    h.run();
    assert_eq!(
        h.state().header_export_text(),
        "Export 4 files now",
        "guard"
    );
    // What `ui::after_commit` leaves when the click lands while a field has the focus.
    h.state_mut()
        .after_commit
        .push(Box::new(AppModel::header_export_clicked));
    h.state_mut().dialog = Some(Dialog::Error("a question".into()));
    h.step();
    let m = h.state_mut();
    assert!(m.after_commit.is_empty(), "guard: the deferred click ran");
    assert!(
        m.export.running.is_none() && m.export.summary.is_none() && nothing_written(d.path()),
        "the deferred click wrote over an open dialog"
    );
    assert!(
        m.status_line.contains("answer the open dialog"),
        "{}",
        m.status_line
    );
    assert!(!m.export_queued());
}

/// Round 4, ruling 3c-32 (re-review B1/B4): the dialog check comes before EVERY dispatch, a pending one included. A
/// deferred click that meets an error dialog while a plan is pending (the held answer is "now") is dropped with the
/// warning, once, before anything is armed and before an open scoped Review loses its scope; the dialog answered
/// and the plan landed, nothing is written.
#[test]
fn a_deferred_click_on_a_pending_plan_never_survives_a_dialog() {
    for scoped in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let mut m = support::opened_chosen_decided(d.path());
        if scoped {
            scoped_review_with_the_opt_out(&mut m, "g/u");
        } else {
            m.set_export_without_review(true);
        }
        let mut h = harness(m);
        h.run();
        let m = h.state_mut();
        m.hold_plan_results = true;
        m.replan();
        assert!(m.full_plan_pending(), "scoped {scoped}: guard: pending");
        assert_eq!(
            m.header_export_text(),
            "Export 4 files now",
            "scoped {scoped}: guard: the held answer writes"
        );
        m.after_commit
            .push(Box::new(AppModel::header_export_clicked));
        m.dialog = Some(Dialog::Error("a question".into()));
        h.step();
        let m = h.state_mut();
        assert!(m.after_commit.is_empty(), "guard: the deferred click ran");
        assert!(
            !m.export_queued(),
            "scoped {scoped}: a click that met a dialog is armed"
        );
        assert_eq!(
            m.export_scope().is_some(),
            scoped,
            "scoped {scoped}: the dropped click changed the scope"
        );
        // The dialog is answered, then the plan lands.
        m.dialog = None;
        let msgs = held_plan(m);
        land_plan(m, msgs);
        assert!(
            m.export.running.is_none() && m.export.summary.is_none() && nothing_written(d.path()),
            "scoped {scoped}: the click wrote after the dialog was answered"
        );
        let warned = m
            .messages
            .entries()
            .filter(|e| {
                e.text
                    .starts_with("Export not started: answer the open dialog")
            })
            .count();
        assert_eq!(warned, 1, "scoped {scoped}: the warning, once");
        assert_eq!(m.export_scope().is_some(), scoped, "scoped {scoped}");
    }
}

/// `link` becomes a directory junction (Windows; a symlink elsewhere) to `target`. Neither needs admin rights.
fn dir_link(link: &std::path::Path, target: &std::path::Path) {
    #[cfg(windows)]
    {
        let ok = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .expect("cmd runs");
        assert!(ok.status.success(), "mklink /J failed: {ok:?}");
    }
    #[cfg(not(windows))]
    std::os::unix::fs::symlink(target, link).unwrap();
}

/// Round 3, ruling 3c-27: the write decision drops the view-keyed `export_blocks` list, whose target-mod overlap check
/// resolves the folders on disk. The export folder is a junction; retargeted into the target mod, every path string
/// and every revision stay the same, so the list cached by the label still says "no blocker". The click must read it
/// fresh: Review opens and nothing is written into the target mod.
#[test]
fn the_click_reads_the_export_blockers_fresh() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    let (out, target) = (d.path().join("out"), d.path().join("target mod"));
    let inside = target.join("textures out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::create_dir_all(&inside).unwrap();
    let link = d.path().join("export link");
    dir_link(&link, &out);
    support::choose_export(&mut m, link.clone());
    m.set_target_mod(Some(target.clone())).unwrap();
    assert!(m.wait_idle(Duration::from_secs(60)));
    // The label reads the blocker list now, and the cache keeps it.
    assert_eq!(m.header_export_text(), "Export 4 files now", "guard");
    std::fs::remove_dir(&link).unwrap();
    dir_link(&link, &inside);
    m.header_export_clicked();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        !inside.join("textures/g/a.dds").exists() && m.export.summary.is_none(),
        "the click wrote into the target mod on a cached blocker list"
    );
    assert!(m.review.is_some(), "the overlap blocker opens Review");
}

/// Round 4, ruling 3c-36 (re-review B5): the header's whole-project disk read (the PGPatcher line and the
/// `sdc.toml` writability, which prepares the whole project's JSON) comes only after the gate's cheap checks pass. A
/// scoped Review on a project with no export folder chosen fails at that check and reads nothing; a clean one reads
/// once.
#[test]
fn the_header_reads_the_disk_only_after_the_cheap_gate_checks() {
    use sdc_gui::model::review::whole_disk_reads;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_unchosen(d.path());
    let k = sdc_gui::model::sets::set_views(&m)[0].key.clone();
    m.export_sets([k].into(), "Export set".into());
    assert!(
        m.review.is_some() && m.export_scope().is_some(),
        "guard: scoped Review"
    );
    assert!(
        m.export_folder_chosen().is_none(),
        "guard: no export folder"
    );
    m.expire_review_json();
    let before = whole_disk_reads(&m);
    assert!(!m.header_answer().accent, "guard: the gate is closed");
    assert_eq!(
        whole_disk_reads(&m),
        before,
        "a closed cheap check read the disk"
    );
    // Control: a clean scoped project reads it once.
    let d2 = tempfile::tempdir().unwrap();
    let mut m2 = support::opened_chosen_decided(d2.path());
    scoped_review_with_the_opt_out(&mut m2, "g/u");
    m2.expire_review_json();
    let before = whole_disk_reads(&m2);
    assert!(m2.header_answer().now, "guard");
    assert_eq!(whole_disk_reads(&m2), before + 1);
}

/// Round 4, ruling 3c-35 (re-review, out-of-scope observation 2): Review's Export, queued behind a plan, was
/// confirmed for one scope. The header's "Review…" pressed before the plan lands clears the scope (the open Review
/// becomes the whole project's); the queued export must not run for the whole project: nothing is written, the
/// continuation is dropped and the user is told.
#[test]
fn a_queued_scoped_review_export_never_widens_to_the_whole_project() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let k = key_of(&m, "g/u");
    m.export_sets([k].into(), "Export set g/u".into());
    assert!(
        m.review.is_some() && m.export_scope().is_some(),
        "guard: scoped Review"
    );
    m.hold_plan_results = true;
    m.replan();
    m.export_from_review();
    assert!(
        m.export_queued(),
        "guard: Review's Export waits for the plan"
    );
    assert_eq!(m.header_export_text(), "Review 4 files…", "guard");
    m.header_export_clicked();
    assert!(
        m.review.is_some() && m.export_scope().is_none(),
        "guard: the header's Review made the open Review the whole project's"
    );
    let msgs = held_plan(&mut m);
    land_plan(&mut m, msgs);
    assert!(
        m.export.running.is_none() && m.export.summary.is_none() && nothing_written(d.path()),
        "the export confirmed for set g/u ran for the whole project"
    );
    assert!(!m.export_queued(), "the continuation is dropped");
    assert!(
        m.status_line.contains("press Export again"),
        "{}",
        m.status_line
    );
    assert!(m
        .messages
        .entries()
        .any(|e| e.text.contains("press Export again")));
}

/// Round 4, ruling 3c-34 (re-review B1): a fresh blocker read drops what was derived from the old one. The export
/// folder junction overlaps the target mod while Problems is built (its overlap blocker is a counted row); then the
/// junction is retargeted to a safe folder with no key moving. The click reads the blockers fresh, and must not answer
/// "Problems" from the old list: it writes.
#[test]
fn the_click_does_not_read_problems_built_from_old_blockers() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    let (out, target) = (d.path().join("out"), d.path().join("target mod"));
    let inside = target.join("textures out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::create_dir_all(&inside).unwrap();
    let link = d.path().join("export link");
    dir_link(&link, &out);
    support::choose_export(&mut m, link.clone());
    m.set_target_mod(Some(target.clone())).unwrap();
    assert!(m.wait_idle(Duration::from_secs(60)));
    // The junction moves into the target mod; a key moving afterwards (any edit would) builds blockers and Problems
    // from it.
    std::fs::remove_dir(&link).unwrap();
    dir_link(&link, &inside);
    m.invalidate_views();
    assert!(
        m.has_problem_rows(),
        "guard: the overlap is a counted Problems row"
    );
    assert!(m.header_export_text().starts_with("Review "), "guard");
    // Back to a safe folder, no key moving.
    std::fs::remove_dir(&link).unwrap();
    dir_link(&link, &out);
    m.header_export_clicked();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        m.review.is_none() && out.join("textures/g/a.dds").is_file(),
        "the click answered from Problems built on the old blocker list: {:?}",
        m.review_gate()
    );
    assert!(!inside.join("textures/g/a.dds").exists());
}

/// Round 3, ruling 3c-28 (re-review B5): the header computes its answer once per frame, also on the clean, settled,
/// write-now frame under a scope where it used to evaluate the rule three times (label, accent count, label count).
#[test]
fn the_header_evaluates_the_write_now_rule_once_per_frame() {
    use sdc_gui::model::review::rule_reads;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    scoped_review_with_the_opt_out(&mut m, "g/u");
    let mut h = harness(m);
    h.run();
    assert_eq!(
        h.state().header_export_text(),
        "Export 4 files now",
        "guard"
    );
    for scoped in [true, false] {
        let before = rule_reads(h.state());
        h.step();
        assert_eq!(
            rule_reads(h.state()) - before,
            1,
            "one evaluation per frame (scoped: {scoped})"
        );
        h.state_mut().close_review();
        h.run();
    }
}

/// A file made read-only for one test; writable again on drop, so the temp dir can go.
struct ReadOnlyFile(std::path::PathBuf);

impl ReadOnlyFile {
    fn set(path: std::path::PathBuf) -> Self {
        let mut p = std::fs::metadata(&path).unwrap().permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&path, p).unwrap();
        Self(path)
    }
}

impl Drop for ReadOnlyFile {
    fn drop(&mut self) {
        if let Ok(m) = std::fs::metadata(&self.0) {
            let mut p = m.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(false);
            let _ = std::fs::set_permissions(&self.0, p);
        }
    }
}

/// Round 3, ruling 3c-28 (re-review B3): the whole project's Also check is scope-blind in every part. With a tick and
/// a read-only `sdc.toml`, whole-project Review's gate is closed; the header under a scoped Review shows the same (no
/// accent), though the scoped Review took no writability reading (a tick does not run on a scoped export).
#[test]
fn a_scoped_review_header_reads_the_sdc_toml_writability_of_the_whole_project() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_also_tick(AlsoTarget::Vanilla, true);
    assert!(m.also_on(), "guard: the tick took: {}", m.status_line);
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.save().unwrap();
    let toml = m.root.clone().unwrap().join("sdc.toml");
    assert!(toml.is_file(), "guard: sdc.toml is on disk");
    let k = key_of(&m, "g/u");
    m.export_sets([k].into(), "Export set g/u".into());
    assert!(
        m.header_accent(),
        "guard: writable, the whole project's gate is open"
    );
    m.close_review();
    let _ro = ReadOnlyFile::set(toml);
    m.open_review();
    assert_eq!(
        m.review_gate(),
        Err(sdc_gui::model::also::TOML_READ_ONLY.to_string()),
        "guard: whole-project Review is closed by the read-only sdc.toml alone"
    );
    m.close_review();
    let k = key_of(&m, "g/u");
    m.export_sets([k].into(), "Export set g/u".into());
    assert!(m.export_scope().is_some(), "guard: scoped");
    m.expire_review_json(); // the 1 s drawing cache passing
    assert!(
        !m.header_accent(),
        "the header shows what whole-project Review decides: closed"
    );
}

/// `text` appended to `sdc.toml` by hand (a comment: the project's settings stay the same).
fn edit_sdc_toml_by_hand(toml: &std::path::Path, text: &str) {
    let mut s = std::fs::read_to_string(toml).unwrap_or_default();
    s.push_str(text);
    std::fs::write(toml, s).unwrap();
}

/// Round 3, ruling 3c-29 (re-review B1): the replan guarantee for a hand-edited `sdc.toml`. The first hand edit arms
/// the click and plans (the walk's reload plans, then the gate plans again for the click). A second hand edit while the
/// click waits ends in Review: nothing is written and nothing is armed. The plan `check_sources` starts for the
/// reload is the reload's own; the gate requests none.
#[test]
fn a_click_that_waited_through_a_hand_edited_sdc_toml_never_replans_for_a_second_one() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    let toml = m.root.clone().unwrap().join("sdc.toml");
    // On disk and taken, so the edits below are the only changes.
    edit_sdc_toml_by_hand(&toml, "# start\n");
    assert_eq!(m.check_sources(), 1, "guard: the hand edit is seen");
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.header_export_text(), "Export 4 files now", "guard");
    m.hold_plan_results = true;
    edit_sdc_toml_by_hand(&toml, "# by hand 1\n");
    m.header_export_clicked();
    assert!(
        m.full_plan_pending() && m.export_queued() && m.review.is_none(),
        "guard: the first change arms the click and plans"
    );
    let mut msgs = held_plan(&mut m);
    edit_sdc_toml_by_hand(&toml, "# by hand 2\n");
    // Deliver until the click's plan lands (the reload's own first plan is superseded by the gate's).
    loop {
        for x in msgs {
            m.handle_held(x);
        }
        if !(m.export_queued() && m.full_plan_pending()) {
            break;
        }
        msgs = held_plan(&mut m);
    }
    assert!(
        m.review.is_some() && !m.export_queued(),
        "the second change opens Review and leaves nothing armed"
    );
    assert!(m.export.running.is_none() && nothing_written(d.path()));
    // The reload's own plan lands; it carries no click.
    m.hold_plan_results = false;
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        !m.export_queued() && m.export.summary.is_none() && nothing_written(d.path()),
        "nothing was written after the reload's plan either"
    );
    assert!(m.review.is_some());
}

/// A scoped Review for set `id` (the opt-out is on afterwards, so opening it did not write).
fn scoped_review_with_the_opt_out(m: &mut AppModel, id: &str) {
    let k = key_of(m, id);
    m.export_sets([k].into(), format!("Export set {id}"));
    assert!(
        m.review.is_some() && m.export_scope().is_some(),
        "guard: a scoped Review is open"
    );
    m.set_export_without_review(true);
}

// Ruling 3c-24, one test per input of the header's answer that flips without moving the view key or the Problems key,
// each with a scoped Review open: the label and the accent follow in the same frame.
#[test]
fn a_scoped_review_header_follows_a_foreign_nif_row_going_away() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    scoped_review_with_the_opt_out(&mut m, "g/u");
    m.set_nif_review_rows(vec![support::nif_row("meshes\\x.nif", Some([7; 32]))]);
    assert!(
        m.header_export_text().starts_with("Review ") && !m.header_accent(),
        "guard: the unanswered foreign NIF closes the gate: {}",
        m.header_export_text()
    );
    m.set_nif_review_rows(vec![]);
    assert_eq!(
        m.header_export_text(),
        "Export 4 files now",
        "the label follows the row"
    );
    assert!(m.header_accent(), "the accent follows the row");
}

#[test]
fn a_scoped_review_header_follows_an_overwrite_answer_given_in_review() {
    use sdc_gui::model::meshes::export::GuardChoice;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    scoped_review_with_the_opt_out(&mut m, "g/u");
    m.set_export_without_review(false);
    m.set_nif_review_rows(vec![support::nif_row("meshes\\x.nif", Some([7; 32]))]);
    assert!(
        !m.header_accent(),
        "guard: the unanswered overwrite closes the whole project's gate"
    );
    let mut row = support::nif_row("meshes\\x.nif", Some([7; 32]));
    row.answer = Some(GuardChoice::KeepTheirs);
    m.set_nif_review_rows(vec![row]);
    assert!(m.header_accent(), "the answer opens the gate: accent on");
    // Round 3 (evidence rule): the label too. A NIF edit to write keeps it on Review, with or without the opt-out.
    assert_eq!(m.header_export_text(), "Review 4 files + 1 NIF edit…");
    m.set_export_without_review(true);
    assert_eq!(m.header_export_text(), "Review 4 files + 1 NIF edit…");
    assert!(m.header_accent(), "the answered gate stays open");
}

#[test]
fn a_scoped_review_header_follows_the_also_tick() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    scoped_review_with_the_opt_out(&mut m, "g/u");
    assert_eq!(m.header_export_text(), "Export 4 files now", "guard");
    m.set_also_tick(AlsoTarget::Vanilla, true);
    assert!(m.also_on(), "guard: the tick took: {}", m.status_line);
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.export_scope().is_some(), "guard: still scoped");
    assert!(
        m.header_export_text().starts_with("Review "),
        "the whole project with an Also tick opens Review: {}",
        m.header_export_text()
    );
    // Round 3 (evidence rule): a valid Also tick leaves the whole project's gate open, so the accent stays on.
    assert!(m.header_accent(), "the valid tick keeps the gate open");
    m.set_also_tick(AlsoTarget::Vanilla, false);
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.header_export_text(), "Export 4 files now");
    assert!(m.header_accent());
}

#[test]
fn a_scoped_review_header_follows_a_restored_json() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, json) = exported_row_free_pbr(d.path());
    let original = std::fs::read(&json).unwrap();
    m.set_export_without_review(false);
    let id = sdc_gui::model::sets::set_views(&m)[0].id.clone();
    let k = key_of(&m, &id);
    m.export_sets([k].into(), format!("Export set {id}"));
    assert!(m.review.is_some() && m.export_scope().is_some(), "guard");
    std::fs::write(&json, "{ \"edited\": true }\n").unwrap();
    m.expire_review_json();
    assert!(
        !m.header_accent(),
        "guard: the edited JSON's unanswered overwrite closes the whole project's gate"
    );
    std::fs::write(&json, &original).unwrap();
    m.expire_review_json();
    assert!(m.header_accent(), "the restored JSON opens the gate");
    // Round 3 (evidence rule): with the opt-out the label follows the same flips as the accent.
    m.set_export_without_review(true);
    assert!(m.export_scope().is_some(), "guard: still scoped");
    assert_eq!(m.header_export_text(), "Export 3 files now", "guard");
    std::fs::write(&json, "{ \"edited\": true }\n").unwrap();
    m.expire_review_json();
    assert!(
        m.header_export_text().starts_with("Review ") && !m.header_accent(),
        "the edited JSON: {}",
        m.header_export_text()
    );
    std::fs::write(&json, &original).unwrap();
    m.expire_review_json();
    assert_eq!(m.header_export_text(), "Export 3 files now");
    assert!(m.header_accent());
}

// Ruling 3c-19: the whole-project answer under a scope keeps its volatile terms live (the JSON recheck, the NIF
// rows, the pending plan), so none of them can leave a stale "now" or a stale Review behind.
#[test]
fn a_scoped_review_header_follows_the_json_going_edited() {
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/pbr/arm");
    support::colour(&t.join("steel.png"), 200);
    support::flat(&t.join("steel_n.png"));
    support::colour(&t.join("steel_rmaos.png"), 30);
    let mut m = support::opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_export_without_review(true);
    m.header_export_clicked();
    assert!(m.wait_idle(Duration::from_secs(120)));
    let json = support::pgp_json_path(&m);
    assert!(json.is_file(), "{:?}", m.export.notes);
    m.set_export_without_review(false);
    let id = sdc_gui::model::sets::set_views(&m)[0].id.clone();
    scoped_review_with_the_opt_out(&mut m, &id);
    assert!(
        m.header_export_text().ends_with(" now"),
        "guard: {}",
        m.header_export_text()
    );
    std::fs::write(&json, "{ \"edited\": true }\n").unwrap();
    m.expire_review_json(); // the 1 s recheck passing
    assert!(
        m.header_export_text().starts_with("Review "),
        "{}",
        m.header_export_text()
    );
}

/// Round 3, evidence rule (re-review B4): the stale state a held whole-project answer would leave IS reachable. A
/// scan-finished read whose reply changes nothing clears `votes_pending` and settles at once, moving neither the view
/// key nor the Problems key. The answer taken when the read started was "Review" (a NIF edit to write); the row goes
/// while the read is pending, and the header must read "now" once the reply lands. A cached rule answer (the
/// stale-answer mutation, `task-2-fix3-mut-votes-stale-answer.log`) stays on Review and fails the last asserts.
#[test]
fn a_scoped_review_header_follows_an_unchanged_scan_reply() {
    use sdc_gui::model::bottom::export_files_builds;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    // The database is part of the view key: set it, and let its first read land, before the scope opens.
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    drop(modscan::store::Store::open(&db).unwrap());
    m.scan_db = Some(db);
    m.refresh_mesh_votes(false);
    assert!(m.wait_idle(Duration::from_secs(60)));
    scoped_review_with_the_opt_out(&mut m, "g/u");
    // The frame after the opt-out: its `sdc-project.toml` change reruns the checks (`refresh_checks`, keyed on the
    // document's revision), which moves the view key. Done here, so the reply's own poll moves nothing.
    m.poll();
    m.set_nif_review_rows(vec![support::nif_row("meshes\\x.nif", None)]);
    // A scan finished: the read goes to the worker, and its reply waits in the channel until the next poll.
    m.refresh_mesh_votes(false);
    assert!(
        m.votes_pending() && m.full_plan_pending(),
        "guard: the read is pending"
    );
    assert!(
        m.header_export_text().starts_with("Review "),
        "guard: the answer held at the read's start is Review: {}",
        m.header_export_text()
    );
    let builds = export_files_builds(&m);
    m.set_nif_review_rows(vec![]); // moves neither the view key nor the Problems key
    let end = std::time::Instant::now() + Duration::from_secs(60);
    while m.votes_pending() {
        assert!(std::time::Instant::now() < end, "the read never landed");
        m.poll();
    }
    assert!(
        !m.full_plan_pending(),
        "guard: the unchanged reply settled without a plan"
    );
    assert!(m.export_scope().is_some(), "guard: still scoped");
    assert_eq!(m.header_export_text(), "Export 4 files now");
    assert!(m.header_accent());
    assert_eq!(
        export_files_builds(&m),
        builds,
        "guard: no key moved (no file count was rebuilt)"
    );
}

/// Kept from round 2 as a pin: the post-plan read's landing moves the view key, so it cannot catch a held answer (the
/// test above does).
#[test]
fn a_scoped_review_header_is_not_stuck_on_an_answer_made_while_votes_were_pending() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    drop(modscan::store::Store::open(&db).unwrap());
    m.scan_db = Some(db);
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
    scoped_review_with_the_opt_out(&mut m, "g/u");
    // The held answer starts as "Review" (a NIF edit waits when the plan starts), so the label reads the live
    // whole-project answer while the plan's post-plan read is pending.
    m.set_nif_review_rows(vec![support::nif_row("meshes\\x.nif", None)]);
    // Another database: the post-plan read cannot be the kept evidence, it goes to the worker.
    let db2 = dbdir.path().join("scan2.db");
    drop(modscan::store::Store::open(&db2).unwrap());
    m.scan_db = Some(db2);
    // The plan's result is held and delivered by hand, so the state between the plan and its read lasts as long as
    // the test wants (no polling for a window).
    m.hold_plan_results = true;
    m.replan();
    for x in held_plan(&mut m) {
        m.handle_held(x);
    }
    assert!(
        m.votes_pending() && m.full_plan_pending(),
        "guard: the post-plan read is pending"
    );
    // The header is drawn while it is pending: the whole-project answer is computed now (a pending plan and a NIF
    // edit both answer false).
    let _ = m.header_export_text();
    m.set_nif_review_rows(vec![]); // moves neither the view key nor the problems key
    m.hold_plan_results = false;
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.export_scope().is_some(), "guard: still scoped");
    assert_eq!(m.header_export_text(), "Export 4 files now");
}
#[test]
fn a_scoped_review_header_follows_a_nif_row_that_will_be_written() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    scoped_review_with_the_opt_out(&mut m, "g/u");
    assert_eq!(m.header_export_text(), "Export 4 files now", "guard");
    m.set_nif_review_rows(vec![support::nif_row("meshes\\x.nif", None)]);
    assert!(
        m.header_export_text().starts_with("Review "),
        "{}",
        m.header_export_text()
    );
}

/// Fix round 1, Minor 7 (ruling 3c-21): the header's accent reads the whole project's gate, as its label does. A clean
/// scoped Review with a blocker elsewhere (here a parallax flag no height map backs, in the other set) leaves it
/// without the accent.
#[test]
fn the_header_accent_reads_the_whole_projects_gate_under_a_scope() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened(d.path());
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_json_value(
        "landscape/rock01",
        "parallax",
        Some(serde_json::json!(true)),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    let k = key_of(&m, "landscape/leaf");
    m.export_sets([k].into(), "Export set landscape/leaf".into());
    assert!(m.review.is_some() && m.export_scope().is_some(), "guard");
    assert!(
        m.review_gate().is_ok(),
        "guard: the scope alone is clean: {:?}",
        m.review_gate()
    );
    m.close_review();
    m.header_export_clicked();
    assert!(
        m.review_gate().is_err(),
        "guard: the whole project has a blocker"
    );
    m.close_review();
    let k = key_of(&m, "landscape/leaf");
    m.export_sets([k].into(), "Export set landscape/leaf".into());
    assert!(!m.header_accent(), "the whole project's gate is closed");
}
// Ruling 3c-17: the accent shows only when a click has something to write or review.
#[test]
fn the_header_has_no_accent_with_nothing_to_write() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    std::fs::create_dir_all(&root).unwrap();
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    m.open_folder(root);
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_export_folder(d.path().join("out"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.header_export_text(), "Review 0 files…");
    assert!(
        m.review_gate().is_ok(),
        "guard: the gate alone would light it"
    );
    assert!(!m.header_accent());
}

#[test]
fn the_header_has_the_accent_when_there_is_something_to_write() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    assert!(m.header_accent(), "a clean project with files");
    m.set_export_without_review(true);
    assert!(m.header_accent(), "under the opt-out");
}

#[test]
fn the_hover_says_why_a_click_writes_at_once() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let plain = sdc_gui::ui::header::export_hover(&m);
    assert!(!plain.contains("Writes at once"), "{plain}");
    m.set_export_without_review(true);
    let now = sdc_gui::ui::header::export_hover(&m);
    assert!(now.starts_with(&plain), "the counted form stays: {now}");
    assert!(
        now.contains(
            "Writes at once: Review is off for this project. Turn it back on in Settings."
        ),
        "{now}"
    );
}

// The brief's `ctrl_e_follows_the_header` pressed Ctrl+E, but no key is bound to the "export" action in this tree
// (`ui/actions.rs`: `&[]`); the action runs from the Ctrl+K palette, so the test runs the action as the palette does.
#[test]
fn the_export_action_follows_the_header() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(support::opened_chosen_decided(d.path()));
    h.run();
    let ctx = h.ctx.clone();
    sdc_gui::ui::actions::run("export", h.state_mut(), &ctx);
    settle(&mut h);
    assert!(h.state().review.is_some());
    assert!(h.state().export.running.is_none());
}

/// Round 5, ruling 3c-37: every function that can arm, queue or start a texture export.
#[derive(Debug, Clone, Copy)]
enum Entry {
    /// `header_export_clicked`; the palette's "export" action calls it too.
    Header,
    /// The header click deferred a frame for a focused field (`ui::after_commit`), drained by the next frame.
    HeaderDeferred,
    /// `export_from_review`.
    ReviewExport,
    /// Review's Export deferred a frame, drained by the next frame.
    ReviewExportDeferred,
    ExportSet,
    ExportGroup,
    /// Export N sets, the sets menu's entry.
    ExportNSets,
    /// The legacy model entry `export_clicked` (`export_go`).
    Legacy,
}

/// Round 5, ruling 3c-37: what waits for a landing, delivered with a dialog open.
#[derive(Debug, Clone, Copy)]
enum Queued {
    /// `export_after_plan` (the legacy entry, queued).
    Legacy,
    /// `Continue::Review` (Review's Export, queued).
    Review,
    /// The armed write (a header "now" click, queued).
    Armed,
    /// Review's Export queued, then a header "now" click armed: both wait for the same landing.
    ArmedAndReview,
}

fn dialog_warnings(m: &AppModel) -> usize {
    m.messages
        .entries()
        .filter(|e| {
            e.text
                .starts_with("Export not started: answer the open dialog")
        })
        .count()
}

/// The setup of every 3c-37 row: a clean project, opted out, a scoped Review for g/u open (so a re-scope shows), in a
/// harness so a deferred click can be drained by a frame.
fn dialog_row_setup(d: &std::path::Path, entry: Option<Entry>) -> Harness<'static, AppModel> {
    let mut m = support::opened_chosen_decided(d);
    match entry {
        Some(Entry::ExportGroup) => {
            m.move_to_group(
                &["g/a".to_string()],
                sdc_gui::model::set_groups::GroupTarget::New("Pair".into()),
            )
            .expect("grouped");
            assert!(m.wait_idle(Duration::from_secs(60)));
        }
        Some(Entry::ExportNSets) => {
            let (a, u) = (key_of(&m, "g/a"), key_of(&m, "g/u"));
            m.click_set(&a, false, false);
            m.click_set(&u, true, false);
        }
        _ => {}
    }
    scoped_review_with_the_opt_out(&mut m, "g/u");
    assert!(m.wait_idle(Duration::from_secs(60)));
    let mut h = harness(m);
    h.run();
    h
}

/// The outcome checks shared by every 3c-37 row, once the dialog is answered and everything landed.
fn dialog_row_wrong(
    m: &AppModel,
    d: &std::path::Path,
    scope: Option<sdc_gui::model::export_scope::ExportScope>,
    warned: usize,
) -> Vec<String> {
    let mut wrong = vec![];
    if !nothing_written(d) || m.export.running.is_some() || m.export.summary.is_some() {
        wrong.push("wrote after the dialog was answered".to_string());
    }
    if m.export_queued() {
        wrong.push("something is still queued or armed".into());
    }
    if m.export_scope().cloned() != scope {
        wrong.push(format!(
            "the scope changed: {:?} -> {:?}",
            scope,
            m.export_scope()
        ));
    }
    if dialog_warnings(m) != warned + 1 {
        wrong.push(format!("{} warnings", dialog_warnings(m) - warned));
    }
    wrong
}

/// One entry run with an error dialog open, the dialog answered, then the pending plan landed (`pending`) or nothing
/// more (a settled clean plan). Lists what is wrong (empty: right).
fn entry_under_a_dialog(entry: Entry, pending: bool) -> Vec<String> {
    let d = tempfile::tempdir().unwrap();
    let mut h = dialog_row_setup(d.path(), Some(entry));
    let m = h.state_mut();
    if pending {
        m.hold_plan_results = true;
        m.replan();
        assert!(m.full_plan_pending(), "{entry:?}: guard: pending");
    }
    assert!(
        m.review.is_some() && m.export_scope().is_some(),
        "{entry:?}: guard: the scoped Review is open"
    );
    let scope = m.export_scope().cloned();
    let warned = dialog_warnings(m);
    m.dialog = Some(Dialog::Error("a question".into()));
    match entry {
        Entry::Header => m.header_export_clicked(),
        Entry::HeaderDeferred => m
            .after_commit
            .push(Box::new(AppModel::header_export_clicked)),
        Entry::ReviewExport => m.export_from_review(),
        Entry::ReviewExportDeferred => m.after_commit.push(Box::new(AppModel::export_from_review)),
        Entry::ExportSet => {
            let k = key_of(m, "g/a");
            m.export_sets([k].into(), "Export set g/a".into());
        }
        Entry::ExportGroup => m.export_group("Pair"),
        Entry::ExportNSets => {
            let entry = m
                .set_menu()
                .into_iter()
                .find(|e| e.action == sdc_gui::model::bulk::MenuAction::ExportSets)
                .expect("the Export N sets entry");
            assert!(m.run_menu(entry.action).is_none());
        }
        Entry::Legacy => m.export_clicked(),
    }
    if matches!(entry, Entry::HeaderDeferred | Entry::ReviewExportDeferred) {
        h.step();
    }
    let m = h.state_mut();
    assert!(m.after_commit.is_empty(), "{entry:?}: guard: the click ran");
    let mut wrong = vec![];
    if m.export_queued() || m.export.running.is_some() {
        wrong.push("armed, queued or started under the dialog".to_string());
    }
    m.dialog = None;
    if pending {
        let msgs = held_plan(m);
        land_plan(m, msgs);
    } else {
        assert!(m.wait_idle(Duration::from_secs(60)));
    }
    wrong.extend(dialog_row_wrong(m, d.path(), scope, warned));
    wrong
        .into_iter()
        .map(|w| format!("{entry:?} (pending {pending}): {w}"))
        .collect()
}

/// A queue set behind a pending plan (`read`: a scan-finished read instead), then an error dialog opens and the plan
/// (or the read) lands under it. The dialog is answered afterwards. Lists what is wrong (empty: right).
fn queue_delivered_under_a_dialog(queued: Queued, read: bool) -> Vec<String> {
    let d = tempfile::tempdir().unwrap();
    let dbdir = tempfile::tempdir().unwrap();
    let mut h = dialog_row_setup(d.path(), None);
    let m = h.state_mut();
    if read {
        let db = dbdir.path().join("scan.db");
        drop(modscan::store::Store::open(&db).unwrap());
        m.scan_db = Some(db);
        m.refresh_mesh_votes(false);
        assert!(m.votes_pending(), "{queued:?}: guard: a read is pending");
    } else {
        m.hold_plan_results = true;
        m.replan();
    }
    assert!(m.full_plan_pending(), "{queued:?}: guard: pending");
    match queued {
        Queued::Legacy => m.export_clicked(),
        Queued::Review => m.export_from_review(),
        Queued::Armed => m.header_export_clicked(),
        Queued::ArmedAndReview => {
            m.export_from_review();
            m.header_export_clicked();
        }
    }
    assert!(m.export_queued(), "{queued:?}: guard: queued");
    let scope = m.export_scope().cloned();
    let warned = dialog_warnings(m);
    m.dialog = Some(Dialog::Error("a question".into()));
    if read {
        let end = std::time::Instant::now() + Duration::from_secs(60);
        while m.votes_pending() {
            assert!(std::time::Instant::now() < end, "the read never landed");
            m.poll();
        }
    } else {
        let msgs = held_plan(m);
        for x in msgs {
            m.handle_held(x);
        }
        m.hold_plan_results = false;
    }
    assert!(
        m.dialog.is_some(),
        "{queued:?}: guard: delivered under the dialog"
    );
    m.dialog = None;
    assert!(m.wait_idle(Duration::from_secs(120)));
    dialog_row_wrong(m, d.path(), scope, warned)
        .into_iter()
        .map(|w| format!("{queued:?} (read {read}): {w}"))
        .collect()
}

/// Round 5, ruling 3c-37 (re-review: Review's Export dispatched through an open dialog): one dialog rule at every
/// entry. Each entry runs with an error dialog open, twice (a plan pending; a settled clean plan); the dialog is
/// answered before the plan lands. Each queue is delivered with the dialog open, by a plan and by a scan-finished read
/// (the "settled" case does not apply to a delivery). Every row: nothing written, nothing queued or armed, the scope
/// unchanged, the warning once.
#[test]
fn an_open_dialog_stops_every_export_entry() {
    let entries = [
        Entry::Header,
        Entry::HeaderDeferred,
        Entry::ReviewExport,
        Entry::ReviewExportDeferred,
        Entry::ExportSet,
        Entry::ExportGroup,
        Entry::ExportNSets,
        Entry::Legacy,
    ];
    let queues = [
        Queued::Legacy,
        Queued::Review,
        Queued::Armed,
        Queued::ArmedAndReview,
    ];
    let mut wrong: Vec<String> = vec![];
    for e in entries {
        for pending in [true, false] {
            wrong.extend(entry_under_a_dialog(e, pending));
        }
    }
    for q in queues {
        for read in [false, true] {
            wrong.extend(queue_delivered_under_a_dialog(q, read));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Round 5, ruling 3c-38: the Export buttons the UI can defer a frame for a focused field (`ui::after_commit`).
/// Export set, Export group, Export N sets and the palette call the model at once.
#[derive(Debug, Clone, Copy)]
enum Deferred {
    ReviewExport,
    Header,
}

/// A scoped Review for g/u is open, the input name box has the focus and a plan for another export folder is pending.
/// `button` is pressed and released in one frame (a tap), so its click waits a frame; the plan lands before that
/// frame (as `SdcApp`'s poll does) and re-keys the scope; then the frame runs the click. Lists what is wrong.
fn pressed_then_rekeyed(button: Deferred) -> Vec<String> {
    use egui_kittest::kittest::{by, Queryable as _};
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let u = key_of(&m, "g/u");
    m.select_set(&u);
    scoped_review_with_the_opt_out(&mut m, "g/u");
    assert!(m.wait_idle(Duration::from_secs(60)));
    let mut h = harness(m);
    h.run();
    h.get(
        by().role(egui::accesskit::Role::TextInput)
            .label(sdc_gui::ui::input_name::HINT),
    )
    .click();
    h.run();
    let moved = d.path().join("export2");
    let m = h.state_mut();
    m.hold_plan_results = true;
    m.set_export_folder(moved.clone());
    assert!(m.full_plan_pending(), "{button:?}: guard: pending");
    let scope = m.export_scope().cloned();
    let label = match button {
        Deferred::ReviewExport => m.review_button_text(),
        Deferred::Header => m.header_export_text(),
    };
    let p = h
        .query_all_by_label(&label)
        .next()
        .unwrap_or_else(|| panic!("{button:?}: no button {label}"))
        .rect()
        .center();
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    h.step();
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    let m = h.state_mut();
    assert_eq!(
        m.after_commit.len(),
        1,
        "{button:?}: guard: the click waits a frame"
    );
    // The poll at the next frame's start lands the plan under the new export folder: the scope is re-keyed.
    let msgs = held_plan(m);
    for x in msgs {
        m.handle_held(x);
    }
    m.hold_plan_results = false;
    assert!(
        m.export_scope().is_some() && m.export_scope().cloned() != scope,
        "{button:?}: guard: the scope was re-keyed"
    );
    h.step();
    let m = h.state_mut();
    assert!(
        m.after_commit.is_empty(),
        "{button:?}: guard: the click ran"
    );
    assert!(m.wait_idle(Duration::from_secs(120)));
    let mut wrong = vec![];
    if moved.join("textures/g/u.dds").exists()
        || moved.join("textures/g/a.dds").exists()
        || !nothing_written(d.path())
        || m.export.running.is_some()
        || m.export.summary.is_some()
    {
        wrong.push("wrote for a scope the press never saw".to_string());
    }
    if m.export_queued() {
        wrong.push("something is still queued or armed".into());
    }
    if !m.status_line.contains("press Export again") {
        wrong.push(format!("no message: {}", m.status_line));
    }
    wrong
        .into_iter()
        .map(|w| format!("{button:?}: {w}"))
        .collect()
}

/// Round 5, ruling 3c-38 (re-review: Review captures the scope one frame late): a click is judged by what was true
/// when the button fired. Through the real deferral, a plan that re-keys the scope between the press and the deferred
/// call drops the click with the 3c-35 message; nothing is written.
#[test]
fn a_deferred_export_click_keeps_the_scope_of_its_press() {
    let wrong: Vec<String> = [Deferred::ReviewExport, Deferred::Header]
        .into_iter()
        .flat_map(pressed_then_rekeyed)
        .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// The export folder is a junction to `out a`; a source changed, so the gate's walk plans again with the click armed.
/// While the walk runs (`set_after_walk`) the junction is retargeted to `out b`. `waited`: the click waited for a
/// pending plan and the walk is its first landing's; else the click met a settled plan and walked at once. Lists what
/// is wrong (empty: right).
fn retargeted_during_the_walk(waited: bool) -> Vec<String> {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    let (link, a, b) = (
        d.path().join("export link"),
        d.path().join("out a"),
        d.path().join("out b"),
    );
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    dir_link(&link, &a);
    support::choose_export(&mut m, link.clone());
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.header_export_text(), "Export 4 files now", "guard");
    let retarget = {
        let (link, b) = (link.clone(), b.clone());
        move || {
            std::fs::remove_dir(&link).unwrap();
            dir_link(&link, &b);
        }
    };
    if waited {
        m.hold_plan_results = true;
        m.replan();
        m.header_export_clicked();
        assert!(m.export_queued(), "waited {waited}: guard: armed");
        let msgs = held_plan(&mut m);
        support::colour(&d.path().join("textures/g/a.png"), 91);
        m.set_after_walk(retarget);
        for x in msgs {
            m.handle_held(x);
        }
        assert!(
            m.full_plan_pending() && m.export_queued(),
            "waited {waited}: guard: the landing walked, planned again and re-armed"
        );
        let msgs = held_plan(&mut m);
        land_plan(&mut m, msgs);
    } else {
        support::colour(&d.path().join("textures/g/a.png"), 91);
        m.set_after_walk(retarget);
        m.header_export_clicked();
        assert!(
            m.full_plan_pending() && m.export_queued(),
            "waited {waited}: guard: the click walked, planned again and armed"
        );
        assert!(m.wait_idle(Duration::from_secs(120)));
    }
    let mut wrong = vec![];
    let wrote = [&a, &b]
        .iter()
        .any(|r| r.join("textures/g/a.dds").exists() || r.join("textures/g/u.dds").exists())
        || m.export.summary.is_some();
    if wrote {
        wrong.push("wrote where the click was not confirmed".to_string());
    }
    if m.review.is_none() {
        wrong.push("no Review is open".into());
    }
    if m.export_queued() {
        wrong.push("still armed".into());
    }
    wrong
        .into_iter()
        .map(|w| format!("waited {waited}: {w}"))
        .collect()
}

/// Round 5, ruling 3c-39 (re-review: re-arming can replace destination A with B): an armed value never changes
/// except its `replanned` bit. The junction retargeted while the walk that re-arms the click runs: the landing of the
/// plan it starts opens Review and writes nothing, for a click that waited and for one that walked at once.
#[test]
fn a_rearmed_click_keeps_the_destination_of_its_click() {
    let wrong: Vec<String> = [true, false]
        .into_iter()
        .flat_map(retargeted_during_the_walk)
        .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Round 5, ruling 3c-40: a destination that does not exist yet, armed behind a pending plan.
#[derive(Debug, Clone, Copy)]
enum Missing {
    /// `link\new`: `new` exists under neither junction target; `link` is retargeted from `out a` to `out b` while the
    /// click waits.
    UnderARetargetedLink,
    /// A plain new folder, unchanged while the click waits: the ordinary first export writes at once.
    Unchanged,
    /// A plain new folder created while the click waits: the same parent, the same destination, so it writes.
    CreatedMeanwhile,
}

fn armed_into_a_missing_folder(case: Missing) -> Vec<String> {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.set_export_without_review(true);
    let (link, a, b) = (
        d.path().join("export link"),
        d.path().join("out a"),
        d.path().join("out b"),
    );
    let folder = match case {
        Missing::UnderARetargetedLink => {
            std::fs::create_dir_all(&a).unwrap();
            std::fs::create_dir_all(&b).unwrap();
            dir_link(&link, &a);
            link.join("new")
        }
        Missing::Unchanged | Missing::CreatedMeanwhile => d.path().join("first export"),
    };
    support::choose_export(&mut m, folder.clone());
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        !folder.exists(),
        "{case:?}: guard: the folder does not exist"
    );
    m.hold_plan_results = true;
    m.replan();
    assert_eq!(
        m.header_export_text(),
        "Export 4 files now",
        "{case:?}: guard"
    );
    m.header_export_clicked();
    assert!(m.export_queued(), "{case:?}: guard: armed");
    let msgs = held_plan(&mut m);
    match case {
        Missing::UnderARetargetedLink => {
            std::fs::remove_dir(&link).unwrap();
            dir_link(&link, &b);
        }
        Missing::CreatedMeanwhile => std::fs::create_dir_all(&folder).unwrap(),
        Missing::Unchanged => {}
    }
    land_plan(&mut m, msgs);
    let mut wrong = vec![];
    match case {
        Missing::UnderARetargetedLink => {
            if [&a, &b]
                .iter()
                .any(|r| r.join("new/textures/g/a.dds").exists())
                || m.export.summary.is_some()
            {
                wrong.push("wrote under the link's new target".to_string());
            }
            if m.review.is_none() {
                wrong.push("no Review is open".into());
            }
        }
        Missing::Unchanged | Missing::CreatedMeanwhile => {
            if !folder.join("textures/g/a.dds").is_file() || m.review.is_some() {
                wrong.push(format!("the first export did not write: {:?}", m.dialog));
            }
        }
    }
    if m.export_queued() {
        wrong.push("still armed".into());
    }
    wrong
        .into_iter()
        .map(|w| format!("{case:?}: {w}"))
        .collect()
}

/// Round 5, ruling 3c-40 (re-review: two failed resolutions compare equal): the recorded destination is the canonical
/// nearest existing ancestor plus the missing components, so `link\new` under a retargeted `link` is another
/// destination: Review opens and nothing is written. An unchanged missing folder still compares equal, so the ordinary
/// first export into a new folder writes, and so does one whose folder was created meanwhile under the same parent. (A
/// read that fails for another reason than "not found" is the unit test
/// `model::tests::an_indeterminate_destination_never_compares_equal`.)
#[test]
fn a_missing_destination_is_resolved_through_its_nearest_existing_parent() {
    let wrong: Vec<String> = [
        Missing::UnderARetargetedLink,
        Missing::Unchanged,
        Missing::CreatedMeanwhile,
    ]
    .into_iter()
    .flat_map(armed_into_a_missing_folder)
    .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

// ---- Task 4 (E7): Review lists every file the export touches ----

/// The acceptance test's body: the changed files under the project folder, against what Review listed when it
/// opened. Two listing rules and no other exemption: a changed path under a listed Also folder counts as listed, and
/// the manifest is one listed path whenever the export runs.
fn assert_every_changed_file_is_listed(mut m: AppModel, dir: &std::path::Path) {
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    let out = m.output_root();
    let root = m.root.clone().unwrap();
    m.header_export_clicked();
    assert!(m.review.is_some(), "the header opens Review");
    let listed: BTreeSet<PathBuf> = {
        let w = m.review_writes();
        w.outputs
            .iter()
            .cloned()
            .chain(w.also_updated.iter().map(|r| r.path.clone()))
            .collect()
    };
    let before = support::tree_hashes(&[dir]);
    m.export_from_review();
    assert!(m.wait_idle(Duration::from_secs(120)));
    let after = support::tree_hashes(&[dir]);
    let changed: BTreeSet<PathBuf> = after
        .iter()
        .filter(|(p, h)| before.get(*p) != Some(*h))
        .map(|(p, _)| p.clone())
        .collect();
    // A first export that really writes files: equality under the two listing rules only (controller ruling).
    assert!(!changed.is_empty());
    let also_dirs: Vec<PathBuf> = m.review_writes_at_open().also_folders();
    let changed: BTreeSet<PathBuf> = changed
        .into_iter()
        .map(|p| {
            also_dirs
                .iter()
                .find(|f| p.starts_with(f))
                .cloned()
                .unwrap_or(p)
        })
        .collect();
    assert_eq!(changed, listed, "changed vs listed");
    assert!(listed.contains(&root.join("_sdc").join("generated.json")));
    // The engine plans into `<export>\<root prefix>` (Note HQ1; here `textures`), so its manifest is there, not at
    // `<export>\_sdc` as the brief's text has it.
    let planned = engine::discover::effective_output_root(&root, &out);
    assert_ne!(planned, out, "guard: the fixture's root has a prefix");
    assert!(listed.contains(&planned.join("_sdc").join("manifest.jsonl")));
}

#[test]
fn review_lists_every_file_the_export_changes() {
    let d = tempfile::tempdir().unwrap();
    let m = support::pbr_opened_chosen_decided(d.path());
    assert_every_changed_file_is_listed(m, d.path());
}

#[test]
fn review_lists_every_file_with_an_also_tick() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    m.set_also_tick(AlsoTarget::Vanilla, true);
    assert!(m.wait_idle(Duration::from_secs(120)));
    assert!(m.review_gate().is_ok(), "{:?}", m.review_gate());
    assert_every_changed_file_is_listed(m, d.path());
}

#[test]
fn review_writes_is_cached() {
    use std::sync::Arc;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    m.header_export_clicked();
    let a = m.review_writes();
    let b = m.review_writes();
    assert!(Arc::ptr_eq(&a, &b), "two calls in a row share one list");
    let builds = m.review_writes_builds();
    for _ in 0..50 {
        let _ = m.review_writes();
    }
    assert_eq!(m.review_writes_builds(), builds, "a frame reads no file");
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(120)));
    let c = m.review_writes();
    assert!(!Arc::ptr_eq(&a, &c), "a new plan builds a new list");
    // The disk-state expiry: the same key, a second later, is read again.
    let d2 = m.review_writes();
    assert!(Arc::ptr_eq(&c, &d2));
    m.expire_review_json();
    assert!(!Arc::ptr_eq(&c, &m.review_writes()));
}

fn review_harness_at(m: AppModel, size: egui::Vec2) -> Harness<'static, AppModel> {
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(size)
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    h.run_steps(8);
    h
}

/// Task 4: every control of the footer and the whole "Also updated:" line lie inside the footer's panel, with the
/// Export group in its three states (open, a blocker reason beside a disabled button, "Planning…").
fn assert_the_footer_holds_its_controls(h: &Harness<'static, AppModel>, state: &str) {
    use egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    let footer = support::panel(&h.ctx, "review-footer");
    let m = h.state();
    let mut seen: Vec<(String, egui::Rect)> = vec![];
    let lead = h.get_by_label("Also updated:").rect();
    seen.push(("Also updated:".into(), lead));
    // Every label on the line's row: the names and the `…`.
    for n in h.query_all_by_role(Role::Label) {
        let r = n.rect();
        if r.center().y > lead.top() && r.center().y < lead.bottom() && r.left() >= lead.left() {
            seen.push((format!("{:?}", n.accesskit_node().value()), r));
        }
    }
    seen.push((
        "Close".into(),
        h.get_by_role_and_label(Role::Button, "Close").rect(),
    ));
    seen.push((
        "Export without review next time".into(),
        h.get_by_role_and_label(Role::CheckBox, "Export without review next time")
            .rect(),
    ));
    let export = m.review_button_text();
    seen.push((
        export.clone(),
        h.get_by_role_and_label(Role::Button, &export).rect(),
    ));
    if let Err(why) = m.review_gate() {
        // A reason the Blockers column does not list is drawn beside the disabled button.
        let n = h
            .query_all_by_label_contains(&why)
            .find(|n| n.rect().top() >= footer.top())
            .unwrap_or_else(|| panic!("{state}: the reason is drawn in the footer"));
        seen.push((why.clone(), n.rect()));
    }
    if m.full_plan_pending() {
        let n = h
            .query_all_by_label("Planning…")
            .find(|n| n.rect().top() >= footer.top())
            .expect("Planning… is drawn in the footer");
        seen.push(("Planning…".into(), n.rect()));
    }
    assert!(seen.len() >= 5, "{state}: {seen:?}");
    for (name, r) in seen {
        assert!(
            footer.contains_rect(r),
            "{state}: {name} {r:?} outside the footer {footer:?}"
        );
    }
}

#[test]
fn reviews_footer_holds_the_also_updated_line_and_every_control_at_1100() {
    let size = egui::vec2(1100.0, 864.0);
    // Open: the Export button is live.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    m.open_review();
    let h = review_harness_at(m, size);
    assert!(h.state().review_gate().is_ok());
    assert_the_footer_holds_its_controls(&h, "open");
    // Blocked: undecided textures, a disabled button and its reason.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_unchosen(d.path());
    m.open_review();
    let h = review_harness_at(m, size);
    assert!(h.state().review_gate().is_err(), "guard: a blocker reason");
    assert_the_footer_holds_its_controls(&h, "blocked");
    // Planning: decisions just made, the plan still running.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened(d.path());
    m.decide(
        "landscape/leaf.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    m.open_review();
    let h = review_harness_at(m, size);
    if h.state().full_plan_pending() {
        assert_the_footer_holds_its_controls(&h, "planning");
    } else {
        eprintln!("the plan settled before the first frame: the Planning state is not covered");
    }
}

#[test]
fn a_line_that_does_not_fit_is_cut_in_the_middle_and_names_every_file() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    m.set_also_tick(AlsoTarget::Vanilla, true);
    m.set_also_tick(AlsoTarget::Cm, true);
    assert!(m.wait_idle(Duration::from_secs(120)));
    m.open_review();
    let w = m.review_writes();
    let names: Vec<String> = w
        .also_updated
        .iter()
        .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(names.len() >= 4, "{names:?}");
    // Wide: every file is its own label, named by its full path, and nothing is cut.
    let wide = review_harness_at(m, egui::vec2(2200.0, 864.0));
    assert!(
        wide.query_all_by_label_contains("Also updated: ")
            .next()
            .is_none(),
        "nothing is cut at 2200"
    );
    for e in &w.also_updated {
        wide.get_by_label_contains(&e.path.display().to_string());
    }
    // Narrow: cut in the middle; the `…` holds the whole line, the first and last names stay.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    m.set_also_tick(AlsoTarget::Vanilla, true);
    m.set_also_tick(AlsoTarget::Cm, true);
    assert!(m.wait_idle(Duration::from_secs(120)));
    m.open_review();
    let w = m.review_writes();
    let narrow = review_harness_at(m, egui::vec2(480.0, 864.0));
    let whole = narrow
        .query_all_by_label_contains("Also updated: ")
        .next()
        .expect("the cut line is named whole");
    let whole = whole.accesskit_node().value().unwrap_or_default();
    for n in &names {
        assert!(whole.contains(n.as_str()), "{whole} lacks {n}");
    }
    // Both ends stay as labels of their own, named by their full path; the middle is the ….
    let own = |e: &sdc_gui::model::review::ReviewWrite| {
        let name = e.path.file_name().unwrap().to_string_lossy().into_owned();
        let shown = if e.hover == sdc_gui::model::review::HOVER_ALSO_FOLDER {
            format!("{name}, all its files")
        } else {
            name.clone()
        };
        let named = format!("{shown} ({})", e.path.display());
        narrow
            .query_all_by_label_contains(&name)
            .filter(|n| {
                n.accesskit_node()
                    .value()
                    .is_some_and(|v| v.starts_with(&named))
            })
            .count()
    };
    assert_eq!(own(w.also_updated.first().unwrap()), 1);
    assert_eq!(own(w.also_updated.last().unwrap()), 1);
    let drawn: usize = w.also_updated.iter().map(own).sum();
    assert!(drawn < w.also_updated.len(), "something is cut at 480");
}
