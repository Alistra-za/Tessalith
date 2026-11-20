//! The status row names the next step once, after a folder's opening plans settle.
mod support;
use eframe::egui;
use egui_kittest::{kittest::Queryable, Harness};
use engine::policy::AcceptSuggestions;
use sdc_gui::{model::AppModel, sets_doc::SetsDoc, settings::GuiSettings};
use std::time::Duration;

const TWELVE: &str = "12 textures need a type: start with Undecided.";
const ONE: &str = "1 texture needs a type: start with Undecided.";
const DESTINATION: &str = "Next: choose where to export (Export to ▾).";

fn model() -> AppModel {
    AppModel::new(GuiSettings::default(), None, support::session())
}

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_step_dt(0.25)
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), model)
}

fn settle(h: &mut Harness<'static, AppModel>) {
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run();
}

#[track_caller]
fn assert_step(h: &Harness<'_, AppModel>, step: &str) {
    let line = format!("{step} {}", h.state().status_line_counts());
    assert_eq!(h.query_all_by_label(&line).count(), 1, "{line}");
    assert_eq!(h.state().status_note.as_deref(), Some(step));
}

#[test]
fn after_open_one_line_says_the_next_step() {
    let d = tempfile::tempdir().unwrap();
    support::write_undecided_pngs(d.path(), 12);
    let mut m = model();
    m.accept = AcceptSuggestions::None;
    m.open_folder(d.path().into());
    let mut h = harness(m);
    settle(&mut h);
    assert_step(&h, TWELVE);

    support::set_type(h.state_mut(), "mystery_0.png", "diffuse");
    settle(&mut h);
    assert_eq!(h.state().undecided(), 11);
    assert_eq!(
        h.query_all_by_label_contains("start with Undecided.")
            .count(),
        0
    );
    assert!(h.state().status_note.is_none());
    h.state_mut().undo();
    settle(&mut h);
    assert_eq!(h.state().undecided(), 12);
    assert_eq!(h.query_all_by_label_contains("Undid:").count(), 1);
    assert_eq!(h.query_all_by_label_contains(TWELVE).count(), 0);
}

fn open_with_problems(rows: usize, expected: &str) {
    let d = tempfile::tempdir().unwrap();
    support::colour(&d.path().join("textures/g/rock.png"), 90);
    support::flat(&d.path().join("textures/g/rock_n.png"));
    let mut m = model();
    m.open_folder(d.path().into());
    m.set_nif_problems(
        (0..rows)
            .map(|i| support::nif_problem_warning(&format!("Missing texture {i}")))
            .collect::<Vec<_>>()
            .into(),
    );
    let mut h = harness(m);
    settle(&mut h);
    assert_eq!(h.state().undecided(), 0);
    assert_eq!(sdc_gui::model::bottom::flagged(h.state()).ready_rows, rows);
    assert_step(&h, expected);
}

#[test]
fn one_problem_precedes_the_destination() {
    open_with_problems(1, "1 problem to look at: open Problems.");
}

#[test]
fn two_problems_precede_the_destination() {
    open_with_problems(2, "2 problems to look at: open Problems.");
}

#[test]
fn a_clean_project_without_a_destination_names_the_picker() {
    open_with_problems(0, DESTINATION);
}

fn ready_step(without_review: bool, step: &str) {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    for name in ["a", "u"] {
        support::colour(&root.join(format!("g/{name}.png")), 90);
        support::flat(&root.join(format!("g/{name}_n.png")));
    }
    let mut doc = SetsDoc::open(&root);
    doc.set_export_folder(&d.path().join("export"));
    doc.set_export_without_review(without_review);
    doc.save(false).unwrap();
    let mut m = model();
    m.open_folder(root);
    let mut h = harness(m);
    settle(&mut h);
    assert_eq!(h.state().export_files(), 4);
    assert!(h.state().needs_write(), "the opening recorder ran");
    assert_step(&h, step);
}

#[test]
fn the_ready_step_names_the_header_button() {
    ready_step(false, "Ready: Review 4 files… shows what will be written.");
}

#[test]
fn the_ready_step_names_the_opted_out_header_button() {
    ready_step(true, "Ready: Export 4 files now writes them.");
}

#[test]
fn cancelling_an_open_does_not_repeat_the_previous_folders_step() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    support::write_undecided_pngs(a.path(), 1);
    support::write_undecided_pngs(b.path(), 12);
    let mut m = model();
    m.open_folder(a.path().into());
    let mut h = harness(m);
    settle(&mut h);
    assert_step(&h, ONE);
    h.state_mut().open_folder(b.path().into());
    h.state_mut().cancel_open();
    settle(&mut h);
    assert_eq!(h.state().root.as_deref(), Some(a.path()));
    assert_eq!(
        h.query_all_by_label_contains("start with Undecided.")
            .count(),
        0
    );
    assert!(h.state().status_note.is_none());
}

#[test]
fn a_replacement_open_names_only_the_folder_that_settled() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    support::write_undecided_pngs(a.path(), 12);
    support::write_undecided_pngs(b.path(), 1);
    let mut m = model();
    m.open_folder(a.path().into());
    m.open_folder(b.path().into());
    let mut h = harness(m);
    settle(&mut h);
    assert_step(&h, ONE);
    assert_eq!(h.query_all_by_label_contains(TWELVE).count(), 0);
}

#[test]
fn a_failed_open_does_not_publish_a_step_on_the_fallback_plan() {
    let d = tempfile::tempdir().unwrap();
    support::write_undecided_pngs(d.path(), 1);
    std::fs::write(
        d.path().join("sdc.toml"),
        "[files.'textures/mystery_0.png']\nalpha.threshold = 1000\n",
    )
    .unwrap();
    let mut m = model();
    m.open_folder(d.path().into());
    let mut h = harness(m);
    settle(&mut h);
    assert!(h.state().project.as_ref().unwrap().read_only());
    assert!(h.state().status_note.is_none());
    assert_eq!(h.query_all_by_label_contains(ONE).count(), 0);
}

#[test]
fn retiring_the_step_preserves_a_newer_status_note() {
    let d = tempfile::tempdir().unwrap();
    support::write_undecided_pngs(d.path(), 1);
    let mut m = model();
    m.open_folder(d.path().into());
    let mut h = harness(m);
    settle(&mut h);
    assert_step(&h, ONE);
    let note = "Export cancelled: the project changed.";
    h.state_mut().status_note = Some(note.into());
    h.state_mut().replan();
    settle(&mut h);
    assert_step(&h, note);
}
