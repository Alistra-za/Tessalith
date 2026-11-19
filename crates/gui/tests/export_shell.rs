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
    m.set_also_tick(AlsoTarget::Vanilla, false);
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.header_export_text(), "Export 4 files now");
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
