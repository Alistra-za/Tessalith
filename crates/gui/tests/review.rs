//! Layout v2 Task 1.3 (B2, C2, C5; rulings R1–R4): Review replaces the undecided, blocked and pitfall modals. Since
//! Note FM it gates on blockers, undecided textures and the overwrite questions only.
mod support;
use eframe::egui;
use egui_kittest::kittest::NodeT as _;
use egui_kittest::Harness;
use engine::codes::Code;
use engine::config::{AlphaPolicy, GreenConvention, Subtype};
use sdc_gui::model::bottom::{problems, BottomTab, Level};
use sdc_gui::model::review::{DiskState, JsonKind};
use sdc_gui::model::*;
use serde_json::json;
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01.png";
const LEAF: &str = "landscape/leaf.png";
const ODD: &str = "landscape/odd.png";
const NORMAL: &str = "landscape/rock01_normal.png";

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(120)),
        "model did not settle"
    );
}

#[test]
fn a_clean_project_exports_without_review() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.export_clicked();
    assert!(m.review.is_none() && m.dialog.is_none(), "{:?}", m.dialog);
    idle(&mut m);
    assert!(m.export.summary.is_some());
}

#[test]
fn undecided_textures_open_review_not_a_dialog() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    assert!(m.review.is_some());
    assert_eq!(m.dialog, None);
    assert_eq!(m.bottom.tab, BottomTab::Review);
    assert!(m.bottom.open);
    let (_, skipped) = m
        .review_gate()
        .expect("undecided textures never disable Export");
    assert_eq!(skipped, m.undecided());
    assert!(
        m.review_button_text()
            .contains(&format!("(skip {} undecided)", m.undecided())),
        "{}",
        m.review_button_text()
    );
}

#[test]
fn a_blocker_disables_review_export_with_its_reason() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_ready(d.path());
    m.set_json_value("landscape/rock01", "emissive", Some(json!(true)));
    m.export_clicked();
    assert!(m.review.is_some());
    assert_eq!(m.review_gate(), Err("Fix 1 blocker first".to_string()));
    m.export_from_review();
    idle(&mut m);
    assert!(m.export.summary.is_none() && m.review.is_some());
}

/// Was pbr_export.rs::blocks_never_offer_proceed_anyway: a block and a pitfall, the block wins.
#[test]
fn a_blocker_wins_over_a_pitfall() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_ready(d.path());
    m.set_json_value("landscape/rock01", "match8", Some(json!(r"x\leaf_s.dds")));
    m.set_json_value("landscape/rock01", "emissive", Some(json!(true)));
    idle(&mut m);
    assert!(!pbr_export::export_asks(&m).is_empty());
    m.export_clicked();
    assert!(m.review.is_some());
    assert_eq!(m.review_gate(), Err("Fix 1 blocker first".to_string()));
}

/// Was pbr_export.rs::pitfalls_ask_and_proceeding_continues_to_the_undecided_check: a pitfall gates nothing, so Export
/// skips the undecided textures.
#[test]
fn a_pitfall_does_not_stop_export_skipping_the_undecided_textures() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_ready(d.path());
    m.set_json_value("landscape/rock01", "match8", Some(json!(r"x\leaf_s.dds")));
    idle(&mut m);
    assert!(m.undecided() > 0);
    m.export_clicked();
    assert!(m.review.is_some() && m.dialog.is_none(), "{:?}", m.dialog);
    let (_, skipped) = m.review_gate().unwrap();
    assert_eq!(skipped, m.undecided());
}

/// Was pbr_export.rs::proceeding_with_warnings_waits_for_a_plan_in_flight (audit 08f-2).
#[test]
fn an_export_with_a_pitfall_waits_for_a_plan_in_flight() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_ready(d.path());
    m.set_json_value("landscape/rock01", "match8", Some(json!(r"x\leaf_s.dds")));
    idle(&mut m);
    m.export_clicked();
    m.replan();
    m.export_from_review();
    assert!(m.export_queued() && m.export.running.is_none());
    idle(&mut m);
    assert!(m.export.summary.is_some());
}

#[test]
fn export_from_review_waits_for_a_plan_in_flight() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    // A plan starts.
    m.decide_undecided_row(ROCK, Subtype::Diffuse);
    m.export_from_review();
    assert!(m.export_queued() && m.export.running.is_none());
    idle(&mut m);
    let s = m
        .export
        .summary
        .clone()
        .expect("exported on the settled plan");
    assert!(
        s.skipped > 0,
        "the other undecided textures were skipped (N3)"
    );
    assert!(d.path().join("export/landscape/rock01.dds").exists());
}

#[test]
fn export_from_review_reruns_the_blocks() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    answer_all_but_subtypes(&mut m);
    // Emissive is on with no emissive texture: a block only once rock01 is a PBR set, which it is not yet.
    m.set_json_value("landscape/rock01", "emissive", Some(json!(true)));
    idle(&mut m);
    m.export_clicked();
    assert!(m.review.is_some());
    m.decide_undecided_row(ROCK, Subtype::PbrAlbedo);
    idle(&mut m);
    while let Some(k) = m.undecided_keys().first().cloned() {
        let st = m.subtype_question(&k).flatten().unwrap_or(Subtype::Diffuse);
        m.decide_undecided_row(&k, st);
        idle(&mut m);
    }
    m.export_from_review();
    idle(&mut m);
    assert!(m.export.running.is_none() && m.export.summary.is_none());
    assert!(m.review_gate().unwrap_err().starts_with("Fix "));
}

#[test]
fn export_from_review_plans_again_when_a_source_changed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    let p = m.root.clone().unwrap().join(LEAF);
    image::ImageBuffer::from_fn(32, 32, |_, _| image::Rgba([9u8, 9, 9, 255]))
        .save(&p)
        .unwrap();
    m.export_from_review();
    assert!(m.export_queued());
    idle(&mut m);
    assert!(m.export.summary.is_some());
    assert!(m.items.iter().all(|i| !i.changed_on_disk));
}

#[test]
fn deciding_the_last_undecided_texture_never_exports_by_itself() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    m.accept_all_suggestions();
    idle(&mut m);
    // Accept all leaves the questions that are not a sub-type.
    m.answer_undecided_row(LEAF, Answer::Alpha(AlphaPolicy::Cutout));
    m.answer_undecided_row(ODD, Answer::Acknowledge(Code::DimNpot));
    m.answer_undecided_row(NORMAL, Answer::Green(GreenConvention::Directx));
    idle(&mut m);
    assert_eq!(m.undecided(), 0, "{:?}", m.undecided_keys());
    assert!(m.export.summary.is_none() && m.review.is_some());
    assert_eq!(
        m.review_button_text(),
        format!("Export {} files", m.review_gate().unwrap().0)
    );
}

#[test]
fn opening_another_folder_closes_review_without_exporting() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    // A plan in flight, so the click below is queued.
    m.replan();
    m.export_from_review();
    assert!(m.export_queued());
    let d2 = tempfile::tempdir().unwrap();
    let root2 = mod_folder(d2.path());
    assert!(m.request_open(root2.clone()));
    idle(&mut m);
    assert!(m.review.is_none());
    assert!(!(m.bottom.open && m.bottom.tab == BottomTab::Review));
    assert!(m.export.summary.is_none());
    assert!(!root2.join("sdc.toml").exists());
}

/// K4: Reload drops a queued Review export and closes Review.
#[test]
fn reload_closes_review_and_drops_its_queued_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    m.replan();
    m.export_from_review();
    assert!(m.export_queued());
    m.reload_documents();
    assert!(!m.export_queued());
    idle(&mut m);
    assert!(m.review.is_none());
    assert!(m.export.summary.is_none() && m.export.running.is_none());
}

#[test]
fn undo_reverts_a_decision_made_in_review() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    m.decide_undecided_row(ROCK, Subtype::Diffuse);
    idle(&mut m);
    assert!(!m.undecided_keys().contains(&ROCK.to_string()));
    m.undo();
    idle(&mut m);
    assert!(m.undecided_keys().contains(&ROCK.to_string()));
    assert!(m.review.is_some() && m.export.summary.is_none());
}

#[test]
fn the_gate_names_a_read_only_project() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    assert!(m.review.is_some());
    assert_eq!(m.fix_off(), None);
    std::fs::write(m.root.clone().unwrap().join("sdc.toml"), "nonsense = [").unwrap();
    m.reload_documents();
    idle(&mut m);
    m.open_review();
    let why = m.review_gate().unwrap_err();
    assert!(why.starts_with("sdc.toml is read-only: "), "{why}");
    assert_eq!(m.fix_off(), Some(why));
}

#[test]
fn fix_buttons_wait_for_a_plan_in_flight() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.replan();
    assert_eq!(m.fix_off().as_deref(), Some("Wait for the plan to finish"));
    idle(&mut m);
    assert_eq!(m.fix_off(), None);
}

#[test]
fn closing_review_collapses_the_panel_it_showed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    assert!(m.bottom.open);
    m.close_review();
    assert!(m.review.is_none() && !m.bottom.open);
}

// ---- ported from export_undecided.rs (the undecided list is Review's now)

/// Every non-sub-type question answered, so only the sub-types are still undecided.
fn answer_all_but_subtypes(m: &mut AppModel) {
    m.decide(LEAF, Answer::Alpha(AlphaPolicy::Cutout));
    m.decide(ODD, Answer::Acknowledge(Code::DimNpot));
    m.decide(NORMAL, Answer::Green(GreenConvention::Directx));
    idle(m);
}

#[test]
fn a_texture_without_a_sub_type_suggestion_stays_listed_and_undecided() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    // leaf's sub-type is decided; it now waits for its alpha meaning, which Accept all never answers.
    m.decide(LEAF, Answer::Subtype(Subtype::Diffuse));
    idle(&mut m);
    assert_eq!(m.subtype_question(LEAF), None);
    assert!(m.undecided_keys().contains(&LEAF.to_string()));
    m.export_clicked();
    assert!(m.review.is_some());
    m.accept_all_suggestions();
    assert!(m.undecided_keys().contains(&LEAF.to_string()));
    assert!(m.export.running.is_none());
    idle(&mut m);
    assert!(m.undecided_keys().contains(&LEAF.to_string()));
    assert!(!m.undecided_keys().contains(&ROCK.to_string()));
}

// ---- fix round 1

/// A scan read that lands before the plan in flight (no scan DB: it lands at once, unchanged) never starts a queued
/// Review export on the stale plan; the export waits for the plan.
#[test]
fn a_scan_read_landing_before_the_plan_never_starts_a_queued_review_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    assert!(m.review.is_some());
    m.replan();
    m.export_from_review();
    assert!(m.export_queued());
    m.refresh_mesh_votes(false);
    assert!(
        m.export.running.is_none(),
        "started while a plan was in flight"
    );
    assert!(m.export_queued());
    idle(&mut m);
    assert!(m.export.summary.is_some(), "exported on the settled plan");
}

/// Closing Review (Task 1.4's Close) drops its queued export.
#[test]
fn closing_review_drops_its_queued_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export_clicked();
    m.replan();
    m.export_from_review();
    assert!(m.export_queued());
    m.close_review();
    assert!(!m.export_queued());
    idle(&mut m);
    assert!(m.export.summary.is_none() && m.export.running.is_none());
}

/// Review's Export does nothing while Review is closed.
#[test]
fn export_from_review_needs_review_open() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    assert!(m.review.is_none());
    m.replan();
    m.export_from_review();
    assert!(!m.export_queued());
    idle(&mut m);
    assert!(m.export.summary.is_none() && m.export.running.is_none());
}

/// A8: `pbr_ready` with leaf and odd decided (not `decide_all`, which would make rock01 a plain diffuse), exported once
/// and settled; then the PGPatcher JSON is edited by hand. Returns the model and the JSON's path.
fn clean_pbr_with_edited_json(dir: &std::path::Path) -> (AppModel, std::path::PathBuf) {
    let mut m = pbr_ready(dir);
    m.decide(LEAF, Answer::Subtype(Subtype::Diffuse));
    m.decide(LEAF, Answer::Alpha(AlphaPolicy::Cutout));
    m.decide(ODD, Answer::Subtype(Subtype::Diffuse));
    m.decide(ODD, Answer::Acknowledge(Code::DimNpot));
    idle(&mut m);
    // C2: clean, so the first export starts without Review.
    m.export_clicked();
    assert!(m.review.is_none(), "{:?}", m.review_gate());
    idle(&mut m);
    let p = pgp_json_path(&m);
    assert!(p.is_file(), "{:?}", m.export.notes);
    std::fs::write(&p, "{ \"edited\": true }\n").unwrap();
    (m, p)
}

/// `clean_pbr_with_edited_json`, a JSON edit (so the next export has something to write), Review open.
fn reviewing_an_edited_json(dir: &std::path::Path) -> (AppModel, std::path::PathBuf) {
    let (mut m, p) = clean_pbr_with_edited_json(dir);
    m.set_json_value("landscape/rock01", "specular_level", Some(json!(0.06)));
    idle(&mut m);
    m.open_review();
    (m, p)
}

fn exported_from_review(m: &mut AppModel) {
    m.export_from_review();
    idle(m);
    assert!(m.export.summary.is_some(), "{:?}", m.review_gate());
}

#[test]
fn review_shows_an_edited_pgpatcher_json() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, p) = clean_pbr_with_edited_json(d.path());
    m.open_review();
    let l = m
        .review_json()
        .into_iter()
        .find(|l| l.kind == JsonKind::PgPatcher)
        .unwrap();
    assert_eq!((l.path, l.state), (p, DiskState::Edited));
}

#[test]
fn a_pre_answer_overwrite_writes_without_asking() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, p) = reviewing_an_edited_json(d.path());
    m.answer_review_json(true);
    exported_from_review(&mut m);
    assert_eq!(m.dialog, None);
    assert!(std::fs::read_to_string(&p).unwrap().contains("0.06"));
    assert_eq!(
        m.sets_doc.as_ref().unwrap().last_json_sha256(),
        history::sha256_of(&p)
    );
}

#[test]
fn a_pre_answer_keep_keeps_the_file_with_a_note() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, p) = reviewing_an_edited_json(d.path());
    m.answer_review_json(false);
    exported_from_review(&mut m);
    assert_eq!(m.dialog, None);
    assert!(std::fs::read_to_string(&p).unwrap().contains("edited"));
    assert!(
        m.export
            .notes
            .iter()
            .any(|n| n.starts_with("PGPatcher JSON kept as edited")),
        "{:?}",
        m.export.notes
    );
}

/// C4: the answer was for the file as it was; an edit before Export is asked again and nothing is written (Note FC5;
/// an edit during the export is kept with a note: `model::tests::a_json_edited_during_the_export_is_kept_…`).
#[test]
fn a_json_changed_after_the_answer_is_not_overwritten() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, p) = reviewing_an_edited_json(d.path());
    m.answer_review_json(true);
    std::fs::write(&p, "{ \"edited\": 2 }\n").unwrap();
    m.export_from_review();
    idle(&mut m);
    assert!(
        m.status_line
            .starts_with("Export not started: Answer Keep or Overwrite"),
        "{}",
        m.status_line
    );
    assert_eq!(m.dialog, None);
    assert!(std::fs::read_to_string(&p)
        .unwrap()
        .contains("\"edited\": 2"));
}

/// Review Focus 2.
#[test]
fn a_pre_answer_does_not_survive_a_reload() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, p) = clean_pbr_with_edited_json(d.path());
    m.open_review();
    m.answer_review_json(true);
    m.reload_documents();
    assert!(m.review.is_none());
    idle(&mut m);
    m.set_json_value("landscape/rock01", "specular_level", Some(json!(0.06)));
    idle(&mut m);
    export_through(&mut m);
    idle(&mut m);
    // Note FC5: the reloaded Review has no answer, so its Export waits for one.
    assert!(
        m.status_line
            .starts_with("Export not started: Answer Keep or Overwrite"),
        "{}",
        m.status_line
    );
    assert!(m.review_gate().is_err());
    assert!(std::fs::read_to_string(&p).unwrap().contains("edited"));
}

/// R6, A1 (shot 41); Note FC5 (supersedes CA7's optional answer): Export waits for Keep or Overwrite.
#[test]
fn export_waits_for_the_json_answer() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, p) = clean_pbr_with_edited_json(d.path());
    m.export_clicked();
    assert!(m.review.is_some() && m.export.running.is_none());
    let name = p.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        m.review_gate(),
        Err(format!("Answer Keep or Overwrite for {name}"))
    );
    assert!(!m.export_is_clean());
    m.export_from_review();
    idle(&mut m);
    assert!(
        m.status_line
            .starts_with("Export not started: Answer Keep or Overwrite"),
        "nothing written before the answer: {}",
        m.status_line
    );
    m.answer_review_json(false);
    assert!(m.review_gate().is_ok());
    exported_from_review(&mut m);
    assert_eq!(m.dialog, None);
    assert!(std::fs::read_to_string(&p).unwrap().contains("edited"));
}

/// Note FC5: the answer is for the bytes it was given on; a file changed after it is asked again before the export.
#[test]
fn a_json_changed_after_the_answer_is_asked_again() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, p) = clean_pbr_with_edited_json(d.path());
    m.export_clicked();
    m.answer_review_json(true);
    assert!(m.review_gate().is_ok(), "{:?}", m.review_gate());
    std::fs::write(&p, "{\"edited\": \"again\"}").unwrap();
    // Past `review_json`'s 1 s cache, as the next refresh while Review is open.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let name = p.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        m.review_gate(),
        Err(format!("Answer Keep or Overwrite for {name}"))
    );
    m.answer_review_json(true);
    assert!(m.review_gate().is_ok(), "answered again for the new bytes");
}

/// C2 with a PBR set: Tessalith's own JSON reads as Ours, so the second export starts without Review or a question.
#[test]
fn our_own_pgpatcher_json_does_not_open_review() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_ready(d.path());
    m.decide(LEAF, Answer::Subtype(Subtype::Diffuse));
    m.decide(LEAF, Answer::Alpha(AlphaPolicy::Cutout));
    m.decide(ODD, Answer::Subtype(Subtype::Diffuse));
    m.decide(ODD, Answer::Acknowledge(Code::DimNpot));
    idle(&mut m);
    m.export_clicked();
    idle(&mut m);
    let p = pgp_json_path(&m);
    let l = m
        .review_json()
        .into_iter()
        .find(|l| l.kind == JsonKind::PgPatcher)
        .unwrap();
    assert_eq!((l.path, l.state), (p, DiskState::Ours));
    m.export_clicked();
    assert!(m.review.is_none() && m.export.running.is_some());
    idle(&mut m);
    assert_eq!(m.dialog, None);
}

/// `pbr_ready` with leaf and odd decided (not `decide_all`, which would make rock01 a plain diffuse): nothing is
/// undecided, so only a pitfall could open Review.
fn decided_pbr(dir: &std::path::Path) -> AppModel {
    let mut m = pbr_ready(dir);
    m.decide(LEAF, Answer::Subtype(Subtype::Diffuse));
    m.decide(LEAF, Answer::Alpha(AlphaPolicy::Cutout));
    m.decide(ODD, Answer::Subtype(Subtype::Diffuse));
    m.decide(ODD, Answer::Acknowledge(Code::DimNpot));
    idle(&mut m);
    m
}

/// Note FM (supersedes FC2, FC7, FC12 as to the ticks): a PGPatcher pitfall is an ordinary Warning row in Problems.
/// It opens no Review and gates nothing.
#[test]
fn a_pgpatcher_pitfall_is_a_problems_row_not_a_gate() {
    let d = tempfile::tempdir().unwrap();
    let mut m = decided_pbr(d.path());
    m.set_json_value("landscape/rock01", "match8", Some(json!(r"x\leaf_s.dds")));
    idle(&mut m);
    // match8 here raises two pitfalls: its `_s` list, and match slots that point at different textures.
    let asks = pbr_export::export_asks(&m);
    assert_eq!(asks.len(), 2, "{asks:?}");
    let rows = problems(&m);
    for a in &asks {
        let r = rows.iter().find(|p| p.text == a.text);
        assert_eq!(r.map(|p| p.level), Some(Level::Warning), "{}", a.text);
    }
    assert_eq!(m.review_gate().map(|_| ()), Ok(()));
    // With nothing else to ask, the click exports at once.
    m.export_clicked();
    assert!(m.review.is_none(), "a pitfall alone opens no Review");
    assert!(m.export.running.is_some(), "{:?}", m.status_line);
    idle(&mut m);
    assert!(m.export.summary.is_some());
    // Review open for another reason: its Export is enabled with the pitfalls listed in Problems.
    m.open_review();
    assert_eq!(m.review_gate().map(|_| ()), Ok(()));
}

/// Note FM: Review still gates on a blocker, on an undecided texture (it opens, and Export skips it) and on an
/// edited PGPatcher JSON nobody answered.
#[test]
fn review_still_gates_on_blockers_and_overwrites() {
    // A blocker disables Export.
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_ready(d.path());
    m.set_json_value("landscape/rock01", "emissive", Some(json!(true)));
    m.export_clicked();
    assert!(m.review.is_some());
    assert_eq!(m.review_gate(), Err("Fix 1 blocker first".to_string()));
    // An undecided texture opens Review instead of exporting; Export then skips it.
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    assert!(m.undecided() > 0);
    m.export_clicked();
    assert!(m.review.is_some() && m.export.running.is_none());
    assert_eq!(m.review_gate().map(|g| g.1), Ok(m.undecided()));
    // An edited PGPatcher JSON waits for Keep or Overwrite.
    let d = tempfile::tempdir().unwrap();
    let (mut m, p) = clean_pbr_with_edited_json(d.path());
    m.export_clicked();
    assert!(m.review.is_some() && m.export.running.is_none());
    let name = p.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        m.review_gate(),
        Err(format!("Answer Keep or Overwrite for {name}"))
    );
    m.answer_review_json(false);
    assert_eq!(m.review_gate().map(|_| ()), Ok(()));
}

/// The labels of the widgets Review draws, sorted: the tab's panel rect, every role that carries text. Paths under
/// `skip` (the test's temp folder) show it as `<tmp>`.
fn review_labels(h: &Harness<'static, AppModel>, skip: &str) -> Vec<String> {
    let panel = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("bottom-review"))
        .expect("the Review panel")
        .outer_rect;
    let mut out: Vec<String> = h
        .root()
        .children_recursive()
        .filter(|n| {
            n.accesskit_node().bounding_box().is_some_and(|b| {
                b.x0 >= f64::from(panel.min.x)
                    && b.y0 >= f64::from(panel.min.y)
                    && b.x1 <= f64::from(panel.max.x)
                    && b.y1 <= f64::from(panel.max.y)
            })
        })
        .filter(|n| n.accesskit_node().role() != egui::accesskit::Role::TextRun)
        .filter_map(|n| {
            let a = n.accesskit_node();
            a.label().or_else(|| a.value())
        })
        .filter(|l| !l.is_empty())
        .map(|l| l.replace(skip, "<tmp>"))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// P17: the EXACT groups and rows Review shows. An absence-only test passes after any rename, so this lists everything
/// drawn in the tab for one fixture: an undecided project with its export folder chosen, one NIF edit written and
/// one blocked. There is no Warnings group, no half-size question and no Stale outputs group.
#[test]
fn review_has_no_half_size_or_stale_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    choose_export(&mut m, d.path().join("export"));
    m.set_nif_review_rows(vec![
        sdc_gui::model::meshes::NifReviewRow {
            path: r"meshes\a.nif".into(),
            mod_name: "My Mod".into(),
            blocked: None,
            foreign: None,
            answer: None,
            compaction: None,
        },
        sdc_gui::model::meshes::NifReviewRow {
            path: r"meshes\b.nif".into(),
            mod_name: "My Mod".into(),
            blocked: Some("conflicts are open".into()),
            foreign: None,
            answer: None,
            compaction: None,
        },
    ]);
    m.open_review();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1500.0, 900.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    let skip = d.path().to_string_lossy().into_owned();
    let got = review_labels(&h, &skip);
    let expected: Vec<&str> = vec![
        "1 DDS file",
        "Accept all suggestions",
        "Acknowledge for this texture",
        "Also export as",
        "Also folders go in",
        "Also folders go in: <tmp>\\export",
        "Blockers (0)",
        "CM",
        "CM folder name",
        "Choose where the Also folders go…",
        "Close",
        "Complex material",
        "Export 1 file + 1 NIF edit (skip 4 undecided, 1 NIF edit)",
        "Files",
        "Messages",
        "NIF edits (1)",
        "NIF edits skipped (1)",
        "None",
        "PBR",
        "PBR folder name",
        "Problems (8)",
        "Recipe settings are in Advanced",
        "Review",
        "Skip these sets",
        "This texture's size isn't a power of two — resize it or acknowledge",
        "Undecided (4)",
        "Vanilla",
        "Vanilla folder name",
        "What will be written",
        "landscape diffuse",
        "landscape normal",
        "landscape/rock01",
        "leaf.png",
        "meshes\\a.nif · My Mod · written to <tmp>\\export",
        "meshes\\b.nif: conflicts are open",
        "odd.png",
        "rock01.png",
        "rock01_normal.png",
        "▼",
        "✓",
    ];
    assert_eq!(got, expected, "{got:#?}");
}
