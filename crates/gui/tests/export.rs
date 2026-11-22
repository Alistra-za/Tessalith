mod support;
use engine::codes::{Code, Warning};
use engine::events::{EngineEvent, EventKind, SkipKind, EVENT_SCHEMA};
use sdc_gui::model::export::*;
use sdc_gui::model::*;
use std::path::Path;
use std::time::Duration;
use support::*;

#[test]
fn undecided_textures_open_review_first_and_its_export_skips_them() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let n = m.undecided();
    assert!(n > 0);
    m.export_clicked();
    assert!(m.review.is_some() && m.dialog.is_none(), "{:?}", m.dialog);
    assert_eq!(m.undecided_keys().len(), n);
    assert!(m.export.running.is_none(), "nothing runs before the answer");
    m.export_from_review();
    assert!(m.wait_idle(Duration::from_secs(120)));
    let s = m.export.summary.clone().unwrap();
    assert!(
        s.converted >= 1,
        "the packed _rmaos is decided and converts"
    );
    let skipped: Vec<&Row> = m
        .export
        .rows
        .iter()
        .filter(|r| {
            matches!(
                r.state,
                RowState::Skipped(SkipKind::Unresolved | SkipKind::Blocked, _)
            )
        })
        .collect();
    assert_eq!(skipped.len(), n);
    assert!(d
        .path()
        .join("export/textures/landscape/rock01_rmaos.dds")
        .exists());
    assert!(!d.path().join("export/textures/landscape/leaf.dds").exists());
}

#[test]
fn a_decided_folder_exports_and_the_report_groups_warnings() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.export_clicked();
    assert_eq!(m.dialog, None);
    assert!(m.bottom.open && m.bottom.tab == sdc_gui::model::bottom::BottomTab::Files);
    assert!(m.wait_idle(Duration::from_secs(120)));
    let s = m.export.summary.clone().unwrap();
    assert_eq!(s.failed, 0);
    assert!(
        m.root.clone().unwrap().join("sdc.toml").exists(),
        "export saves first"
    );
    assert!(m.export.rows.iter().all(|r| !r.job_key.is_empty()));
    assert!(
        m.export.rows.iter().all(|r| r.state != RowState::Queued),
        "no row is left queued"
    );
    let codes = m.export.by_code();
    assert!(codes
        .get(&Code::DimSmall)
        .is_some_and(|keys| keys.contains(&"landscape/odd.dds".to_string())));
    let md = m.export.to_markdown();
    assert!(md.contains("DIM-SMALL") && md.contains("landscape/odd.dds"));
    assert_eq!(m.export.to_jsonl().lines().count(), m.export.events.len());
    m.select_job("landscape/odd.dds");
    assert_eq!(m.selection, Selection::Item("landscape/odd.png".into()));
}

#[test]
fn export_refuses_the_input_folder_or_an_ancestor() {
    let root = Path::new("C:/mods/x/textures");
    assert!(export_folder_problem(root, root).is_some());
    assert!(export_folder_problem(root, Path::new("C:/mods/x")).is_some());
    assert!(
        export_folder_problem(root, Path::new("C:/mods/X/Textures")).is_some(),
        "case-insensitive"
    );
    assert!(export_folder_problem(root, Path::new("C:/mods/x/textures/sdc-out")).is_none());
    assert!(export_folder_problem(root, Path::new("D:/out")).is_none());
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let root = m.root.clone().unwrap();
    choose_export(&mut m, root);
    m.export_clicked();
    assert!(matches!(m.dialog, Some(Dialog::Error(ref e)) if e.contains("export folder")));
    assert!(m.export.running.is_none());
}

#[test]
fn export_stops_when_the_save_fails() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    // A directory where sdc.toml should be makes the save fail.
    let toml = m.root.clone().unwrap().join("sdc.toml");
    std::fs::create_dir(&toml).unwrap();
    m.export_clicked();
    assert!(matches!(
        m.dialog,
        Some(Dialog::Error(_)) | Some(Dialog::ExternalChange(_))
    ));
    assert!(m.export.running.is_none());
    assert!(m.export.summary.is_none());
    assert!(m.is_dirty());
}

#[test]
fn the_cli_command_reproduces_the_export() {
    let c = cli_command(
        Path::new("C:/mods/x/textures"),
        Path::new("D:/out dir"),
        false,
    );
    assert_eq!(
        c,
        r#"sdc convert "C:/mods/x/textures" --recursive -o "D:/out dir" --accept-suggestions high"#
    );
    assert!(!c.contains("--validate"));
    assert!(
        !c.contains("--jobs"),
        "the engine's defaults are not written out"
    );
}

/// FM: the copied command carries none of the flags 2.0 removed.
#[test]
fn copy_cli_command_uses_no_removed_flag() {
    let c = cli_command(Path::new("C:/a"), Path::new("C:/b"), false);
    for gone in ["--sidecar", "--fill", "--strict-cubemaps"] {
        assert!(!c.contains(gone), "{gone}: {c}");
    }
}

/// Note LG: the copied command validates when the switch is on.
#[test]
fn the_cli_command_names_validate_when_it_is_on() {
    let c = cli_command(Path::new("C:/a"), Path::new("C:/b"), true);
    assert!(c.ends_with(" --validate"), "{c}");
}

fn ev(seq: u64, job: &str, path: &str, kind: EventKind) -> EngineEvent {
    EngineEvent {
        schema: EVENT_SCHEMA,
        seq,
        ts: "2026-10-10T00:00:00.000Z".into(),
        batch: "b".into(),
        set: None,
        path: Some(path.into()),
        job: Some(job.into()),
        kind,
    }
}

/// Audit lead 10: the per-mip Progress events are not kept, so the JSON Lines report has none.
#[test]
fn progress_events_are_not_kept_for_the_report() {
    let mut x = ExportModel::default();
    x.on_event(ev(0, "a/b.dds", "a/b.png", EventKind::FileStarted));
    let progress = EventKind::Progress {
        stage: engine::events::Stage::Compress,
        done: 1,
        total: 11,
    };
    x.on_event(ev(1, "a/b.dds", "a/b.png", progress));
    assert_eq!(x.events.len(), 1);
    assert!(!x.to_jsonl().contains("progress"), "{}", x.to_jsonl());
}

#[test]
fn a_later_run_overrides_an_earlier_pack_conflict_skip_of_the_same_job() {
    let mut x = ExportModel::default();
    let skip = EventKind::FileSkipped {
        kind: SkipKind::PackConflict,
        code: Some(Code::PackConflict),
        reason: "raw replaces rock_rmaos.png".into(),
    };
    x.on_event(ev(0, "a/rock_rmaos.dds", "a/rock_rmaos.png", skip));
    assert!(matches!(
        x.rows[0].state,
        RowState::Skipped(SkipKind::PackConflict, _)
    ));
    x.on_event(ev(
        1,
        "a/rock_rmaos.dds",
        "a/rock_roughness.png",
        EventKind::FileStarted,
    ));
    assert_eq!(x.rows.len(), 1, "one row per job key");
    assert_eq!(x.rows[0].state, RowState::Running);
    x.on_event(ev(
        2,
        "a/rock_rmaos.dds",
        "a/rock_roughness.png",
        EventKind::FileFailed {
            stage: engine::events::Stage::Write,
            error: "disk full".into(),
        },
    ));
    assert_eq!(x.rows[0].state, RowState::Failed("disk full".into()));
    assert_eq!(x.events.len(), 3);
}

#[test]
fn a_skip_after_a_run_does_not_undo_it() {
    let mut x = ExportModel::default();
    x.on_event(ev(0, "a/b.dds", "a/b.png", EventKind::FileStarted));
    let skip = EventKind::FileSkipped {
        kind: SkipKind::PackConflict,
        code: Some(Code::PackConflict),
        reason: "late".into(),
    };
    x.on_event(ev(1, "a/b.dds", "a/b.png", skip));
    assert_eq!(x.rows[0].state, RowState::Running);
}

#[test]
fn packed_source_skips_make_no_row_and_cancelled_inputs_mark_their_rows() {
    let mut x = ExportModel::default();
    let packed = EventKind::FileSkipped {
        kind: SkipKind::PackedSource,
        code: None,
        reason: "packed into a/rock_rmaos.dds".into(),
    };
    x.on_event(ev(0, "a/rock_rmaos.dds", "a/rock_roughness.png", packed));
    assert!(x.rows.is_empty());
    x.on_event(ev(
        1,
        "a/b.dds",
        "C:/in/a/b.png",
        EventKind::FileDiscovered {
            output: "a/b.dds".into(),
        },
    ));
    x.on_event(ev(
        2,
        "a/b.dds",
        "C:/in/a/b.png",
        EventKind::Warning(Warning::new(Code::DimNpot, "npot")),
    ));
    assert_eq!(x.rows[0].warnings.len(), 1);
    let mut done = ev(
        3,
        "",
        "",
        EventKind::BatchDone {
            converted: 0,
            up_to_date: 0,
            skipped: 0,
            skipped_intentional: 0,
            failed: 0,
            validation_failed: 0,
            checked: 0,
            checked_failed: 0,
            cancelled: vec!["C:/in/a/b.png".into()],
        },
    );
    done.job = None;
    done.path = None;
    x.on_event(done);
    assert_eq!(x.rows[0].state, RowState::Cancelled);
}

#[test]
fn the_report_is_written_as_json_lines_or_markdown_by_extension() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.export_clicked();
    assert!(m.wait_idle(Duration::from_secs(120)));
    let jsonl = d.path().join("r.jsonl");
    let md = d.path().join("r.MD");
    sdc_gui::ui::drawer::write_report(&m, &jsonl).unwrap();
    sdc_gui::ui::drawer::write_report(&m, &md).unwrap();
    assert_eq!(
        std::fs::read_to_string(&jsonl).unwrap(),
        m.export.to_jsonl()
    );
    assert_eq!(
        std::fs::read_to_string(&md).unwrap(),
        m.export.to_markdown()
    );
}

/// Audit left-out (DECISIONS Note Y7): rows are found by key; a packed-source drop keeps the index right.
#[test]
fn rows_stay_keyed_after_a_packed_source_drop() {
    let mut x = ExportModel::default();
    let found = |o: &str| EventKind::FileDiscovered { output: o.into() };
    x.on_event(ev(0, "a/r.dds", "a/r_roughness.png", found("a/r.dds")));
    x.on_event(ev(1, "a/b.dds", "a/b.png", found("a/b.dds")));
    x.on_event(ev(2, "a/c.dds", "a/c.png", found("a/c.dds")));
    let packed = EventKind::FileSkipped {
        kind: SkipKind::PackedSource,
        code: None,
        reason: "packed".into(),
    };
    x.on_event(ev(3, "", "a/r_roughness.png", packed));
    assert_eq!(x.rows.len(), 2);
    x.on_event(ev(4, "a/c.dds", "a/c.png", EventKind::FileStarted));
    let state = |k: &str| {
        x.rows
            .iter()
            .find(|r| r.job_key == k)
            .unwrap()
            .state
            .clone()
    };
    assert_eq!(state("a/c.dds"), RowState::Running);
    assert_eq!(state("a/b.dds"), RowState::Queued);
    assert_eq!(x.rows.len(), 2, "no duplicate row from a stale index");
}

/// Audit 08c-4, ruling 3c-10: the export notes (JSON not written, kept files, copies) are in the Markdown report,
/// under the Files tab's heading.
#[test]
fn the_markdown_report_lists_the_export_notes() {
    let mut x = ExportModel::default();
    x.notes = vec!["PGPatcher JSON not written: the export failed".into()];
    let md = x.to_markdown();
    assert!(
        md.contains("## Export details\n\n- PGPatcher JSON not written: the export failed\n"),
        "{md}"
    );
}

/// Audit 08f-1: an export queued behind a plan does not start while the Open-unsaved question is up; the user's
/// answer would otherwise open the other folder in the middle of the export and drop its JSON.
#[test]
fn a_queued_export_waits_out_an_open_dialog() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_json_value(
        "landscape/rock01",
        "specular_level",
        Some(serde_json::json!(0.05)),
    );
    m.replan();
    m.export_clicked();
    assert!(m.export_queued());
    let other = d.path().join("other");
    std::fs::create_dir_all(&other).unwrap();
    assert!(!m.request_open(other.clone()));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        m.export.running.is_none() && m.export.rows.is_empty(),
        "the export started"
    );
    assert_eq!(m.dialog, Some(Dialog::OpenUnsaved(other)));
    assert!(m.status_line.contains("Export"), "{}", m.status_line);
}

/// Audit 08f-1: answering Open-unsaved while an export runs asks to stop the export first, as Open does.
#[test]
fn answering_open_unsaved_during_an_export_waits_for_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_json_value(
        "landscape/rock01",
        "specular_level",
        Some(serde_json::json!(0.05)),
    );
    export_through(&mut m);
    assert!(m.export.running.is_some(), "{:?}", m.dialog);
    let root = m.root.clone();
    let other = d.path().join("other");
    std::fs::create_dir_all(&other).unwrap();
    m.resolve_open_unsaved(other.clone(), false);
    assert_eq!(m.root, root, "not opened during the export");
    assert_eq!(
        m.dialog,
        Some(Dialog::ExportRunning(AfterExport::Open(other)))
    );
    m.cancel_export();
    assert!(m.wait_idle(Duration::from_secs(120)));
}

/// Re-check C3: an artist-check warning never opens Review and never reaches the blocks or asks.
#[test]
fn export_never_asks_about_check_warnings() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    give_a_check(&mut m, d.path());
    m.poll();
    let c: usize = m
        .items
        .iter()
        .map(|i| m.check_warnings(&i.key).len())
        .sum::<usize>();
    assert!(c > 0, "the fixture raises a check result (blue-0 normal)");
    for i in sdc_gui::model::pbr_export::export_blocks(&m)
        .iter()
        .chain(sdc_gui::model::pbr_export::export_asks(&m).iter())
    {
        assert!(!format!("{i:?}").contains("CHK-"), "{i:?}");
    }
    m.export_clicked();
    assert!(m.review.is_none(), "{:?}", m.review.is_some());
    assert!(m.export.running.is_some());
    assert!(m.wait_idle(Duration::from_secs(120)));
    assert!(m.export.summary.is_some());
}

// ---- the end-of-export checks summary: when it is logged (CL19) ----

const SUMMARY: &str = "Checks: 1 warning on 1 texture; see Problems";

/// A decided folder whose only check result is the Z sign of its `_n`.
fn with_one_check() -> (tempfile::TempDir, AppModel) {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    give_a_check(&mut m, d.path());
    m.poll();
    assert_eq!(
        sdc_gui::model::checks::checks_summary(&m).as_deref(),
        Some(SUMMARY)
    );
    (d, m)
}

fn summary_lines(m: &AppModel) -> usize {
    m.messages
        .entries()
        .filter(|e| e.text.starts_with("Checks:"))
        .count()
}

#[test]
fn a_completed_export_logs_the_summary_once() {
    let (_d, mut m) = with_one_check();
    m.end_export_with(Ok(Default::default()));
    assert_eq!(summary_lines(&m), 1);
}

#[test]
fn a_cancelled_export_logs_no_summary() {
    let (_d, mut m) = with_one_check();
    let s = engine::api::BatchSummary {
        cancelled: vec!["a.png".into()],
        ..Default::default()
    };
    m.end_export_with(Ok(s));
    assert_eq!(summary_lines(&m), 0);
}

#[test]
fn a_failed_export_logs_no_summary() {
    let (_d, mut m) = with_one_check();
    m.end_export_with(Err("disk full".into()));
    assert_eq!(summary_lines(&m), 0);
}

#[test]
fn an_export_that_ends_under_an_open_dialog_still_logs_the_summary_and_keeps_the_dialog() {
    let (_d, mut m) = with_one_check();
    m.dialog = Some(Dialog::ReloadUnsaved);
    m.end_export_with(Ok(Default::default()));
    assert_eq!(m.dialog, Some(Dialog::ReloadUnsaved));
    assert_eq!(summary_lines(&m), 1);
}

/// Backlog T25 (Note FP2): a plain export that ends with an error ends its rows too (the Also export already did).
#[test]
fn a_failed_plain_export_leaves_no_row_queued() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.export_clicked();
    assert!(m.export.running.is_some(), "the export started");
    // A job the worker announced and never reached (the run died after discovery).
    let found = EventKind::FileDiscovered {
        output: "zz/never.dds".into(),
    };
    m.export
        .on_event(ev(9_000, "zz/never.dds", "zz/never.png", found));
    assert!(
        m.export.rows.iter().any(|r| r.state == RowState::Queued),
        "a row is queued"
    );
    m.end_export_with(Err("cannot create the output folder".into()));
    let open: Vec<_> = m
        .export
        .rows
        .iter()
        .filter(|r| matches!(r.state, RowState::Queued | RowState::Running))
        .map(|r| r.job_key.clone())
        .collect();
    assert!(open.is_empty(), "still open: {open:?}");
}

// ---- Notes JA11, JA12: Tools ▾'s Re-encode every output item and the kept counts ----

fn ui_harness(m: AppModel) -> egui_kittest::Harness<'static, AppModel> {
    let mut h = egui_kittest::Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    h
}

fn export_and_wait(m: &mut AppModel) -> engine::api::BatchSummary {
    m.export_clicked();
    assert!(m.export.running.is_some(), "{:?}", m.dialog);
    assert!(m.wait_idle(Duration::from_secs(120)));
    m.export.summary.clone().unwrap()
}

/// The latest record of every output under the export's textures folder (the last line per output wins).
fn latest_records(m: &AppModel) -> Vec<(String, serde_json::Value)> {
    engine::manifest::Manifest::at(&m.export_root().join("textures"))
        .read()
        .unwrap()
        .into_iter()
        .map(|(k, e)| (k, e.raw))
        .collect()
}

fn skipped(rec: &serde_json::Value) -> bool {
    rec["validation"]["skipped"].as_bool().unwrap_or(false)
}

/// Note LG: an export validates only when Validate exported DDS is on.
#[test]
fn an_export_validates_only_when_the_setting_is_on() {
    for on in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let mut m = opened(d.path());
        decide_all(&mut m);
        m.settings.validate_exports = on;
        export_and_wait(&mut m);
        let records = latest_records(&m);
        assert!(!records.is_empty());
        for (k, rec) in &records {
            assert_eq!(skipped(rec), !on, "{k} with the setting {on}: {rec}");
        }
        let md = m.export.to_markdown();
        if on {
            assert!(md.contains("validation failed 0"), "{md}");
        } else {
            assert!(md.contains("not validated"), "{md}");
        }
    }
}

/// Note LG10: turning the setting on checks the last export's outputs where they are.
#[test]
fn turning_validation_on_checks_the_last_export_in_place() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.settings.validate_exports = false;
    export_and_wait(&mut m);
    assert!(latest_records(&m).iter().all(|(_, r)| skipped(r)));
    m.settings.validate_exports = true;
    let s = export_and_wait(&mut m);
    assert_eq!(s.converted, 0, "{s:?}");
    assert!(s.checked > 0, "{s:?}");
    assert_eq!(s.checked_failed, 0, "{s:?}");
    for (k, rec) in latest_records(&m) {
        assert!(!skipped(&rec), "{k}: {rec}");
    }
}

/// Notes are rebuilt per export: a kept note of one export is gone from the next, which keeps nothing.
#[test]
fn a_kept_note_is_not_carried_into_the_next_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    export_and_wait(&mut m);
    // One record names an older encode version, so the next export keeps its output.
    let manifest = m.export_root().join("textures/_sdc/manifest.jsonl");
    let text: String = std::fs::read_to_string(&manifest)
        .unwrap()
        .lines()
        .enumerate()
        .map(|(i, l)| {
            let mut v: serde_json::Value = serde_json::from_str(l).unwrap();
            if i == 0 {
                v["encode_version"] = "0.9.0".into();
            }
            format!("{v}\n")
        })
        .collect();
    std::fs::write(&manifest, text).unwrap();
    m.open_review();
    assert!(m.wait_idle(Duration::from_secs(60)));
    let writes = m.review_writes();
    assert_eq!(writes.kept.len(), 1);
    assert!(writes.kept[0].is_file());
    assert_eq!(writes.current.len() + writes.kept.len(), m.export_files());
    assert!(m.written_text().ends_with(" · Kept (1)"));
    m.close_review();
    let s = export_and_wait(&mut m);
    assert_eq!(s.kept_older_encode, 1, "{s:?}");
    assert_eq!(m.export.result.as_ref().unwrap().primary.kept, 1);
    assert_eq!(
        m.export
            .rows
            .iter()
            .filter(|r| r.state.label() == "kept")
            .count(),
        1
    );
    assert!(m
        .export
        .notes
        .iter()
        .any(|n| n.starts_with("1 kept from an older encode version")));
    // The next export encodes it again, keeps nothing, and starts with no notes.
    m.set_reencode_all(true);
    let s = export_and_wait(&mut m);
    assert_eq!(s.kept_older_encode, 0, "{s:?}");
    assert!(
        !m.export.notes.iter().any(|n| n.contains(" kept ")),
        "{:?}",
        m.export.notes
    );
}

#[test]
fn the_reencode_box_reencodes_recorded_outputs_only() {
    use egui_kittest::kittest::{NodeT as _, Queryable};
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    let first = export_and_wait(&mut m);
    assert!(first.converted >= 2 && first.failed == 0, "{first:?}");

    // Off at start: the second export finds every output up to date.
    assert!(!m.reencode_all);
    let s = export_and_wait(&mut m);
    assert_eq!((s.converted, s.up_to_date), (0, first.converted), "{s:?}");

    // `leaf.dds` loses its record and its bytes are replaced: a file Tessalith can't show it wrote.
    // The opened folder is a `textures` folder, so its outputs and their manifest are under `<export>/textures`.
    let base = m.export_root().join("textures");
    let manifest = base.join("_sdc/manifest.jsonl");
    let mut leaf = None;
    let lines: Vec<String> = std::fs::read_to_string(&manifest)
        .unwrap()
        .lines()
        .filter(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            let out = v["output"].as_str().unwrap().to_string();
            let is_leaf = out.ends_with("leaf.dds");
            if is_leaf {
                leaf = Some(out);
            }
            !is_leaf
        })
        .map(str::to_string)
        .collect();
    std::fs::write(&manifest, lines.join("\n") + "\n").unwrap();
    let leaf = base.join(leaf.expect("a record for leaf.dds"));
    assert!(leaf.exists(), "{}", leaf.display());
    std::fs::write(&leaf, b"not written by Tessalith").unwrap();

    // Tick the item as a user does: open Tools ▾, then click it by its label.
    m.open_review();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(!m.written_text().contains("checking which changed"));
    let mut h = ui_harness(m);
    let item = |h: &egui_kittest::Harness<'static, AppModel>| {
        h.get_by_role_and_label(
            eframe::egui::accesskit::Role::CheckBox,
            sdc_gui::ui::header::REENCODE_TEXT,
        )
        .accesskit_node()
        .toggled()
    };
    h.get_by_label("Tools").click();
    h.run();
    h.get_by_role_and_label(
        eframe::egui::accesskit::Role::CheckBox,
        sdc_gui::ui::header::REENCODE_TEXT,
    )
    .click();
    h.run();
    assert!(h.state().reencode_all);
    assert!(
        h.state().written_text().contains("checking which changed"),
        "the tick requests a new currency preview"
    );
    // The menu stays open and shows the new value.
    assert_eq!(
        item(&h),
        Some(eframe::egui::accesskit::Toggled::True),
        "the menu stays open, ticked"
    );
    let s = export_and_wait(h.state_mut());
    // JA12: one-shot, the pass that read it cleared it; the open menu shows it unticked.
    h.run();
    assert!(!h.state().reencode_all);
    assert_eq!(item(&h), Some(eframe::egui::accesskit::Toggled::False));
    let m = h.state_mut();
    // Every recorded output is encoded again; the unrecorded one is kept and reported.
    assert_eq!(
        (s.converted, s.up_to_date),
        (first.converted - 1, 0),
        "{s:?}"
    );
    assert_eq!(std::fs::read(&leaf).unwrap(), b"not written by Tessalith");
    let row = m
        .export
        .rows
        .iter()
        .find(|r| r.job_key.ends_with("leaf.dds"))
        .expect("a row for leaf.dds");
    assert!(
        matches!(row.state, RowState::Skipped(SkipKind::UnknownExisting, _)),
        "{row:?}"
    );
    assert!(
        row.warnings
            .iter()
            .any(|w| w.code == Code::OutUnknownExisting),
        "{row:?}"
    );
}

#[test]
fn the_export_summary_shows_kept_counts() {
    use egui_kittest::kittest::Queryable;
    const KEPT: &str = "2 kept from an older encode version, 1 kept at a higher quality target: tick Re-encode every output (Tools) to encode them again";
    // A zero count is not named.
    const LOWER_ONLY: &str =
        "1 kept at a higher quality target: tick Re-encode every output (Tools) to encode it again";
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    // Nothing kept: no line.
    m.end_export_with(Ok(engine::api::BatchSummary {
        up_to_date: 3,
        ..Default::default()
    }));
    assert!(
        !m.export.notes.iter().any(|n| n.contains("kept from")),
        "{:?}",
        m.export.notes
    );
    m.end_export_with(Ok(engine::api::BatchSummary {
        up_to_date: 3,
        kept_lower_target: 1,
        ..Default::default()
    }));
    assert!(
        m.export.notes.iter().any(|n| n == LOWER_ONLY),
        "{:?}",
        m.export.notes
    );
    m.end_export_with(Ok(engine::api::BatchSummary {
        up_to_date: 3,
        kept_older_encode: 2,
        kept_lower_target: 1,
        ..Default::default()
    }));
    assert!(
        m.export.notes.iter().any(|n| n == KEPT),
        "{:?}",
        m.export.notes
    );
    assert!(m.export.to_markdown().contains(KEPT));
    // One output may count for both reasons, so two counts of 1 name no object.
    let both = engine::api::BatchSummary {
        kept_older_encode: 1,
        kept_lower_target: 1,
        ..Default::default()
    };
    assert!(sdc_gui::model::export::kept_note(&both)
        .unwrap()
        .ends_with("(Tools) to encode again"));
    // GUI words: the line never names the CLI flag.
    assert!(!m.export.notes.iter().any(|n| n.contains("--ev")));
    // The Files tab shows it.
    m.bottom.open = true;
    m.bottom.tab = sdc_gui::model::bottom::BottomTab::Files;
    let h = ui_harness(m);
    h.get_by_label(KEPT);
}
