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
