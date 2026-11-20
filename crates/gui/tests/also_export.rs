//! Track A Task 11 (DECISIONS Note CN): the GUI model of "Also export as" Vanilla / CM: the ticks, Review's gate,
//! the worker and the Files rows (C1 under layout v2: Review opens while a tick is on; its Export is the only way out).
mod support;
use eframe::egui::{accesskit::Role, Key, Modifiers};
use egui_kittest::kittest::{NodeT as _, Queryable};
use engine::codes::{Code, Warning};
use engine::events::{EngineEvent, EventKind, EVENT_SCHEMA};
use engine::presets::{AlsoTarget, FolderNames};
use image::{ImageBuffer, Rgb, Rgba};
use sdc_gui::model::bottom::{problems, At};
use sdc_gui::model::export::{cli_command, RowState};
use sdc_gui::model::review::DiskState;
use sdc_gui::model::{AppModel, SetTarget};
use std::path::{Path, PathBuf};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(120);

fn png(path: &Path, px: [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    ImageBuffer::from_pixel(16, 16, Rgba(px))
        .save(path)
        .unwrap();
}

/// `<tmp>/mods/Mod`: a PBR set `textures/pbr/arm/steel`, a plain set `textures/clutter/cup` and a complex-material
/// candidate `textures/clutter/iron` (+ `_m`); exports go to `<tmp>/out`. Opened as the app opens a folder.
fn project(d: &Path) -> AppModel {
    let m = d.join("mods/Mod");
    let t = m.join("textures");
    png(&t.join("pbr/arm/steel.png"), [200, 200, 200, 255]);
    png(&t.join("pbr/arm/steel_n.png"), [128, 128, 255, 255]);
    png(&t.join("pbr/arm/steel_rmaos.png"), [128, 255, 200, 255]);
    let c = t.join("clutter");
    std::fs::create_dir_all(&c).unwrap();
    ImageBuffer::from_fn(16, 16, |x, y| {
        Rgb([(x * 16) as u8, (y * 16) as u8, ((x + y) * 8) as u8])
    })
    .save(c.join("cup.png"))
    .unwrap();
    png(&c.join("cup_n.png"), [128, 128, 255, 255]);
    support::opened_as_the_app(m, d.join("out"))
}

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(WAIT), "the model did not settle");
}

/// Export as a user would through Review: Export, acknowledge, Review's Export, wait.
fn export(m: &mut AppModel) {
    support::export_through(m);
    idle(m);
}

fn tick(m: &mut AppModel, t: AlsoTarget) {
    m.set_also_tick(t, true);
    idle(m);
    assert!(m.also_on(), "the tick took: {}", m.status_line);
}

fn sdc_toml(m: &AppModel) -> PathBuf {
    m.root.clone().unwrap().join("sdc.toml")
}

fn steel_id(m: &AppModel) -> String {
    sdc_gui::model::sets::views(m)
        .iter()
        .find(|v| v.key.ends_with("steel"))
        .expect("the steel set")
        .id
        .clone()
}

/// Every row ended (item 5: no row stays Queued or Running after an export ends).
fn all_rows_ended(m: &AppModel) {
    for r in &m.export.rows {
        assert!(
            !matches!(r.state, RowState::Queued | RowState::Running),
            "{} is {:?}",
            r.job_key,
            r.state
        );
    }
}

#[test]
fn a_tick_opens_review_and_reviews_export_is_the_only_way_out() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    m.export_clicked();
    assert!(m.review.is_some());
    assert_eq!(m.dialog, None);
    assert!(
        m.export.running.is_none(),
        "the header's Export never exports"
    );
    // A plan in flight: the click waits, and the landed plan still opens Review, not the export.
    m.close_review();
    m.replan();
    m.export_clicked();
    idle(&mut m);
    assert!(m.review.is_some() && m.export.summary.is_none());
    m.export_from_review();
    idle(&mut m);
    assert_eq!(m.dialog, None, "{:?}", m.export.notes);
    let [p, v, c] = m.also_folders();
    assert!(p.join("textures/pbr/arm/steel.dds").is_file());
    assert!(v.join("textures/arm/steel.dds").is_file());
    assert!(!c.exists(), "Vanilla only: no CM folder");
    // The GUI names the folders as the engine does (also_file and the JSON root rely on it).
    assert_eq!(
        m.also_folders()[..2],
        m.also_folders_written().unwrap()[..2]
    );
    all_rows_ended(&m);
}

#[test]
fn nothing_ticked_exports_exactly_as_today() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    assert!(!m.also_on());
    assert_eq!(m.output_root(), m.export_root());
    assert_eq!(m.request().unwrap().skip_root, None);
    m.export_clicked();
    if m.review.is_some() {
        m.export_from_review();
    }
    idle(&mut m);
    let out = d.path().join("out");
    assert!(out.join("textures/pbr/arm/steel.dds").is_file());
    let names: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().all(|n| !n.starts_with("Mod ")),
        "no Also folder: {names:?}"
    );
    assert!(!std::fs::read_to_string(sdc_toml(&m))
        .unwrap_or_default()
        .contains("[export"));
}

#[test]
fn a_tick_is_one_undo_step_and_exports_three_folders() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let before = m.project.as_ref().unwrap().text();
    tick(&mut m, AlsoTarget::Cm);
    assert_eq!(m.also_ticks(), (false, true));
    assert_eq!(m.undo_label(), Some("Also export as"));
    m.undo();
    idle(&mut m);
    assert!(!m.also_on());
    assert_eq!(m.project.as_ref().unwrap().text(), before);
    tick(&mut m, AlsoTarget::Cm);
    export(&mut m);
    let [p, v, c] = m.also_folders();
    assert!(p.join("textures/pbr/arm/steel.dds").is_file());
    assert!(
        v.join("textures/arm/steel.dds").is_file(),
        "CM writes Vanilla too"
    );
    assert!(v.join("textures/clutter/cup.dds").is_file());
    assert!(c.join("textures/arm/steel_m.dds").is_file());
    let result = m.export.result.as_ref().unwrap();
    let records = engine::manifest::Manifest::at(&v).read().unwrap();
    let copies: Vec<_> = records
        .iter()
        .filter(|(_, rec)| rec.raw.get("copied_from").is_some())
        .collect();
    assert_eq!(result.copied, 1);
    assert_eq!(copies.len(), result.copied as usize);
    for (path, _) in copies {
        assert_eq!(
            std::fs::read(p.join(path)).unwrap(),
            std::fs::read(v.join(path)).unwrap()
        );
    }
    assert_eq!(
        result.phases[0].1.written + result.copied as usize,
        support::dds_under(&v)
    );
    assert!(
        p.join("PBRNIFPatcher")
            .read_dir()
            .is_ok_and(|mut d| d.next().is_some()),
        "the PGPatcher JSON is in the PBR folder"
    );
    assert!(m
        .export
        .rows
        .iter()
        .any(|r| r.job_key.starts_with("Mod Vanilla/textures/")));
    assert!(m
        .export
        .notes
        .iter()
        .all(|n| !n.contains("conversion-report")));
    all_rows_ended(&m);
    let (done, total) = m.export.progress();
    assert_eq!(done, total);
}

#[test]
fn a_read_only_sdc_toml_disables_reviews_export_with_the_reason() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    m.save().unwrap();
    let path = sdc_toml(&m);
    let bytes = std::fs::read(&path).unwrap();
    let set_ro = |on: bool| {
        let mut p = std::fs::metadata(&path).unwrap().permissions();
        p.set_readonly(on);
        std::fs::set_permissions(&path, p).unwrap();
    };
    set_ro(true);
    m.export_clicked();
    let reason = "sdc.toml is read-only: Also export settings could not be saved";
    assert_eq!(m.review_gate(), Err(reason.to_string()));
    m.export_from_review();
    idle(&mut m);
    assert!(m.export.summary.is_none(), "nothing exported");
    assert!(m.status_line.contains(reason), "{}", m.status_line);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    // Taken once (Review open, Export click), not per frame: clearing the flag shows only at the next click.
    set_ro(false);
    assert_eq!(m.review_gate(), Err(reason.to_string()));
    m.export_from_review();
    idle(&mut m);
    assert!(m.export.summary.is_some());
}

#[test]
fn a_drive_root_input_makes_the_also_gate_show_the_engines_problem() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    m.set_also_location(&d.path().join("mods"));
    idle(&mut m);
    assert_eq!(m.also_gate(), Ok(()));
    m.root = Some(PathBuf::from(r"C:\"));
    let why = m.also_gate().unwrap_err();
    assert!(why.contains("no folder name"), "{why}");
}

#[test]
fn the_also_gate_with_no_project_says_to_open_a_folder() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    m.root = None;
    assert_eq!(m.also_gate(), Err("Open a folder first".to_string()));
}

#[test]
fn a_bad_location_is_the_reviews_reason() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    let root = m.root.clone().unwrap();
    m.set_also_location(&root);
    idle(&mut m);
    m.export_clicked();
    let why = m.review_gate().unwrap_err();
    assert!(why.contains("must not be the opened folder"), "{why}");
    // The MO2 layout (the location holds the opened folder) is fine.
    m.set_also_location(&d.path().join("mods"));
    idle(&mut m);
    m.export_from_review();
    idle(&mut m);
    assert!(m.export.summary.is_some(), "{}", m.status_line);
    assert!(d.path().join("mods/Mod Vanilla").is_dir());
}

#[test]
fn the_pbr_json_in_the_pbr_folder_is_ours_on_the_next_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    export(&mut m);
    let json = sdc_gui::model::pbr_export::prepare(&m).unwrap();
    assert!(
        json.path.starts_with(&m.also_folders()[0]),
        "{:?}",
        json.path
    );
    assert!(json.path.is_file());
    m.export_clicked();
    assert!(m.review_json().iter().all(|l| l.state != DiskState::Edited));
    assert!(!m.pgp_json_edited());
    m.export_from_review();
    idle(&mut m);
    // Note FC5: nothing waits for a Keep/Overwrite answer, so the export ran.
    assert!(m.export.summary.is_some(), "{}", m.status_line);
}

#[test]
fn an_also_file_row_reveals_its_file_instead_of_selecting() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    export(&mut m);
    let key = "Mod Vanilla/textures/arm/steel.dds";
    assert_eq!(
        m.also_file(key),
        Some(d.path().join("out/Mod Vanilla/textures/arm/steel.dds"))
    );
    assert!(m.also_file(key).unwrap().is_file());
    assert_eq!(m.also_file("textures/pbr/arm/steel.dds"), None);
    // A warning on a Vanilla row is an ordinary File problem; a DERIVE-* Info never is a row.
    let ev = |w: Warning| EngineEvent {
        schema: EVENT_SCHEMA,
        seq: 0,
        ts: String::new(),
        batch: "b".into(),
        job: Some(key.into()),
        set: None,
        path: None,
        kind: EventKind::Warning(w),
    };
    m.export
        .on_event(ev(Warning::new(Code::OutUnknownExisting, "kept")));
    m.export
        .on_event(ev(Warning::new(Code::DeriveNotConverted, "no scan")));
    m.invalidate_views();
    let rows = problems(&m);
    assert!(rows.iter().any(|p| p.at == At::File(key.into())));
    assert!(!rows
        .iter()
        .any(|p| p.code == Some(Code::DeriveNotConverted)));
}

#[test]
fn reviews_button_names_the_folders() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let plain = m.review_button_text();
    assert!(!plain.contains("folder"), "{plain}");
    tick(&mut m, AlsoTarget::Vanilla);
    let n = m.export_files();
    assert_eq!(
        m.review_button_text(),
        format!("Export {n} files + Vanilla folder")
    );
    tick(&mut m, AlsoTarget::Cm);
    assert_eq!(
        m.review_button_text(),
        format!("Export {n} files + Vanilla and CM folders")
    );
}

/// Note JF1: the number on Review's Export button, under What will be written and on the header hover is the number
/// of DDS files the export writes into the PBR folder (a first export into an empty location). The Vanilla and CM
/// folders are named and get no number: "5 files to 2 folders" stood for an export that wrote 9.
#[test]
fn the_count_is_the_pbr_folders_and_the_other_folders_are_named() {
    for (also, named) in [
        (AlsoTarget::Vanilla, "Vanilla folder"),
        (AlsoTarget::Cm, "Vanilla and CM folders"),
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut m = project(d.path());
        tick(&mut m, also);
        m.export_clicked();
        assert!(m.review.is_some());
        let button = m.review_button_text();
        let counted = m.export_files();
        assert_eq!(button, format!("Export {counted} files + {named}"));
        assert_eq!(
            m.written_text(),
            format!(
                "{counted} files · checking which changed… in the PBR folder, plus the {named}"
            )
        );
        let hover = sdc_gui::ui::header::export_hover(&m);
        assert!(
            hover.starts_with(&format!(
                "{} + {named}, ",
                sdc_gui::model::code_text::counted(counted, "file", "files")
            )),
            "{hover}"
        );
        m.export_from_review();
        idle(&mut m);
        assert_eq!(m.dialog, None, "{:?}", m.export.notes);
        let written: Vec<usize> = m
            .also_folders()
            .iter()
            .map(|f| support::dds_under(f))
            .collect();
        assert_eq!(
            counted, written[0],
            "{also:?}: {button}, written per folder {written:?}"
        );
        assert!(written[1] > 0, "the Vanilla folder is written: {written:?}");
    }
}

/// P8 (replaces "the progress never goes backwards"): one `drawer_epoch` for the three phases; every row done.
#[test]
fn one_epoch_for_the_whole_also_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Cm);
    let e = m.drawer_epoch;
    export(&mut m);
    assert_eq!(m.drawer_epoch, e + 1);
    let (done, total) = m.export.progress();
    assert!(total > 0 && done == total);
}

#[test]
fn with_an_also_tick_the_equality_holds_for_the_pbr_folder() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    tick(&mut m, AlsoTarget::Cm);
    let n = m.export_files();
    export(&mut m);
    let folders = m.also_folders();
    assert_eq!(support::dds_under(&folders[0]), n);
    assert_eq!(m.export.progress(), (n, n));
    let r = m.export.result.as_ref().unwrap();
    assert_eq!(r.primary.written, n);
    assert_eq!(r.phases.len(), 2);
    let vanilla = &r.phases[0].1;
    let cm = &r.phases[1].1;
    assert_eq!(
        vanilla.written + r.copied as usize,
        support::dds_under(&folders[1])
    );
    assert_eq!(cm.written, support::dds_under(&folders[2]));
    assert_eq!((n, vanilla.written, cm.written, r.copied), (5, 4, 1, 0));
    let line = "5 written, 0 up to date, 0 failed · Vanilla: 4 written, 0 up to date, 0 failed · CM: 1 written, 0 up to date, 0 failed, 1 skipped source";
    assert!(
        m.messages
            .entries()
            .any(|e| e.text == format!("Export finished: {line}")),
        "{r:?}"
    );
    let h = ui_harness(m, 1600.0);
    assert!(has_label(&h, line));
}

#[test]
fn vanilla_counts_the_unused_packing_source_in_its_own_events() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let path = m
        .root
        .as_ref()
        .unwrap()
        .join("textures/clutter/orphan_opacity.png");
    png(&path, [90, 90, 90, 255]);
    m.replan();
    idle(&mut m);
    tick(&mut m, AlsoTarget::Vanilla);
    export(&mut m);
    let r = m.export.result.as_ref().unwrap();
    assert_eq!(r.phases[0].1.skipped_sources, 1, "{r:?}");
}

#[test]
fn a_cm_failure_keeps_the_completed_vanilla_counts() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Cm);
    let folders = m.also_folders();
    std::fs::create_dir_all(folders[2].join("_sdc/manifest.jsonl")).unwrap();
    export(&mut m);
    assert_eq!(support::dds_under(&folders[1]), 4);
    let result = m.export.result.as_ref().unwrap();
    assert_eq!(result.phases.len(), 1);
    assert_eq!((result.phases[0].1.written, result.copied), (3, 1));
    let line = m
        .messages
        .entries()
        .rev()
        .find(|e| e.text.starts_with("Export finished: "))
        .unwrap()
        .text
        .clone();
    assert!(
        line.contains("Vanilla: 3 written, 0 up to date, 0 failed")
            && line.contains("CM phase failed")
            && line.contains("1 Vanilla copy"),
        "{line}"
    );
    let mut h = ui_harness(m, 1600.0);
    settle(&mut h);
    assert!(has_label(
        &h,
        line.strip_prefix("Export finished: ").unwrap()
    ));
}

#[test]
fn copy_cli_command_names_the_location_when_also_is_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let loc = d.path().join("mods");
    m.set_also_location(&loc);
    tick(&mut m, AlsoTarget::Vanilla);
    let root = m.root.clone().unwrap();
    let slash = |p: &Path| p.to_string_lossy().replace('\\', "/");
    let c = cli_command(&root, &m.cli_output());
    assert!(c.contains(&format!("-o \"{}\"", slash(&loc))), "{c}");
    assert!(!c.contains(&slash(&d.path().join("out"))), "{c}");
}

#[test]
fn renamed_folders_are_used_and_saved_in_export_names() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Cm);
    m.set_also_settings(|c| {
        c.names = FolderNames {
            pbr: Some("A".into()),
            vanilla: Some("B".into()),
            cm: Some("C".into()),
        }
    });
    idle(&mut m);
    assert_eq!(m.undo_label(), Some("Also export settings"));
    export(&mut m);
    let out = d.path().join("out");
    for n in ["A", "B", "C"] {
        assert!(out.join(n).is_dir(), "{n}: {:?}", m.export.notes);
    }
    let cfg = engine::presets::ConfigFile::load(&sdc_toml(&m)).unwrap();
    assert_eq!(cfg.export.unwrap().names.vanilla.as_deref(), Some("B"));
}

#[test]
fn the_also_location_is_stored_per_project() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let one = m.root.clone().unwrap();
    let l1 = d.path().join("l1");
    m.set_also_location(&l1);
    idle(&mut m);
    assert_eq!(m.also_location(), l1);
    m.save().unwrap();
    let two = d.path().join("two");
    png(&two.join("a.png"), [1, 2, 3, 255]);
    m.open_folder(two);
    idle(&mut m);
    assert_eq!(m.also_location(), m.export_root());
    m.open_folder(one.clone());
    idle(&mut m);
    assert_eq!(m.also_location(), l1);
    let text = std::fs::read_to_string(one.join("sdc-project.toml")).unwrap();
    assert!(text.contains("also_location"), "{text}");
}

#[test]
fn renaming_a_skipped_set_keeps_its_opt_out() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let steel = steel_id(&m);
    m.set_skip_also(&steel, true);
    idle(&mut m);
    assert_eq!(m.also_config().skip, [steel.as_str()]);
    let iron = steel.replace("steel", "iron");
    m.rename_set(&steel, &iron).unwrap();
    idle(&mut m);
    assert_eq!(m.also_config().skip, [iron.as_str()]);
    m.undo();
    idle(&mut m);
    assert_eq!(m.also_config().skip, [steel], "one undo step");
}

#[test]
fn mesh_votes_are_asked_for_the_vanilla_key_of_a_pbr_colour_member() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    assert!(!m.mesh_vote_keys().contains("textures/arm/steel"));
    tick(&mut m, AlsoTarget::Vanilla);
    let keys = m.mesh_vote_keys();
    assert!(keys.contains("textures/arm/steel"), "{keys:?}");
    assert!(keys.contains("textures/pbr/arm/steel"), "{keys:?}");
}

/// P13: with a tick on, a complex `_m` set's CM JSON goes into the PBR folder (not "outside the export folder").
#[test]
fn a_cm_json_goes_into_the_pbr_folder_while_ticked() {
    let d = tempfile::tempdir().unwrap();
    let c = d.path().join("mods/Mod/textures/clutter");
    std::fs::create_dir_all(&c).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90]))
        .save(c.join("iron.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| {
        Rgba([(x * 4) as u8, (y * 4) as u8, 90, (40 + x * 2) as u8])
    })
    .save(c.join("iron_m.png"))
    .unwrap();
    let mut m = project(d.path());
    let iron = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.ends_with("clutter/iron"))
        .expect("the iron set")
        .id
        .clone();
    m.set_cm_export(&iron, true);
    tick(&mut m, AlsoTarget::Vanilla);
    let ts = sdc_gui::model::cm_json::targets(&m);
    assert!(
        ts.iter().all(|t| t.rel.is_ok()),
        "{:?}",
        ts.iter().map(|t| &t.rel).collect::<Vec<_>>()
    );
    export(&mut m);
    assert!(
        m.also_folders()[0]
            .join("textures/clutter/iron_m.json")
            .is_file(),
        "{:?}",
        m.export.notes
    );
}

/// P15: an export folder inside the opened folder, ticked: the next plan finds no output there as a DDS input.
#[test]
fn a_location_inside_the_opened_folder_is_not_planned_as_inputs() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let inside = m.root.clone().unwrap().join("out");
    m.set_also_location(&inside);
    tick(&mut m, AlsoTarget::Vanilla);
    assert_eq!(m.request().unwrap().skip_root, Some(inside.clone()));
    export(&mut m);
    assert!(inside.join("Mod Vanilla").is_dir());
    m.replan();
    idle(&mut m);
    assert!(
        m.items.iter().all(|i| !i.key.starts_with("out/")),
        "{:?}",
        m.items.iter().map(|i| &i.key).collect::<Vec<_>>()
    );
    export(&mut m);
    assert!(m.export.summary.as_ref().is_some_and(|s| s.failed == 0));
}

/// A tick is refused outside the cs mode (TA20), with the reason; nothing is edited.
#[test]
fn a_tick_is_refused_in_the_vanilla_mode() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    let before = m.project.as_ref().unwrap().text();
    m.set_also_tick(AlsoTarget::Vanilla, true);
    assert!(!m.also_on());
    assert_eq!(m.project.as_ref().unwrap().text(), before);
    assert_eq!(m.status_line, sdc_gui::model::also::NEEDS_CS);
}

/// Final review I2 (Note FM: no zip any more): the PGPatcher JSON the export wrote into the PBR folder is this export's,
/// from the first run, so the walk of the folders does not note it as a file it did not write. No zip, sums file or
/// report is written beside the folders.
#[test]
fn the_pgpatcher_json_is_not_noted_and_nothing_else_is_written_beside_the_folders() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    export(&mut m);
    all_rows_ended(&m);
    let json = sdc_gui::model::pbr_export::prepare(&m).unwrap().path;
    assert!(json.is_file(), "{:?}", m.export.notes);
    let pbr = m.also_folders()[0].clone();
    let name = pbr.file_name().unwrap().to_string_lossy().into_owned();
    let entry = format!(
        "{name}/{}",
        json.strip_prefix(&pbr)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    );
    assert!(
        !m.export.notes.iter().any(|n| n.starts_with(&entry)),
        "{:?}",
        m.export.notes
    );
    let beside: Vec<String> = std::fs::read_dir(m.also_location())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| [".zip", ".txt", ".md"].iter().any(|x| n.ends_with(x)))
        .collect();
    assert!(beside.is_empty(), "{beside:?}");
    assert!(m.export.running.is_none());
}
/// Every `[export]` field set at once survives the write and the parse (prune, tables after values), as an unsaved
/// edit; only the non-default fields are written.
#[test]
fn every_export_field_round_trips_through_sdc_toml() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    let steel = steel_id(&m);
    m.set_also_settings(|c| {
        c.also.push(AlsoTarget::Cm);
        c.skip = vec![steel.clone()];
        c.names.vanilla = Some("Plain".into());
        c.legacy.brightness = 0.8;
        c.legacy.keep_metal_brightness = true;
    });
    idle(&mut m);
    let p = m.project.as_ref().unwrap();
    assert!(p.is_dirty());
    let text = p.text();
    let parsed = engine::presets::ConfigFile::parse(&text, "sdc.toml")
        .unwrap()
        .export
        .unwrap();
    assert_eq!(parsed, m.also_config(), "{text}");
    assert_eq!(parsed.also, [AlsoTarget::Vanilla, AlsoTarget::Cm]);
    assert!(parsed.legacy.brightness == 0.8 && parsed.legacy.keep_metal_brightness);
    assert_eq!(parsed.names.vanilla.as_deref(), Some("Plain"));
    assert_eq!(parsed.skip, [steel.as_str()]);
    assert!(
        !text.contains("ao_strength"),
        "defaults are not written: {text}"
    );
}

/// Review M1: a tick left from the cs mode does not move the plan or the NIF edits (P16) into a PBR folder no export
/// can fill while the project is vanilla; Review still names TA20.
#[test]
fn a_stale_tick_in_the_vanilla_mode_changes_no_root() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    assert_eq!(
        m.nif_export_root(),
        Some(m.also_folders()[0].clone()),
        "P16"
    );
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    assert!(m.also_on(), "the tick stays");
    assert_eq!(m.output_root(), m.export_root());
    assert_eq!(m.nif_export_root(), Some(m.export_root()));
    let req = m.request().unwrap();
    assert_eq!((req.output_root, req.skip_root), (m.export_root(), None));
    m.export_clicked();
    assert_eq!(
        m.review_gate(),
        Err(sdc_gui::model::also::NEEDS_CS.to_string())
    );
}

/// Review M3: a custom folder name equal to another folder's default is refused at Review, naming its key.
#[test]
fn a_folder_name_collision_is_the_reviews_reason() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    m.set_also_settings(|c| c.names.vanilla = Some("Mod PBR".into()));
    idle(&mut m);
    m.export_clicked();
    let why = m.review_gate().unwrap_err();
    assert!(why.contains("export.names.vanilla"), "{why}");
}

// ---- Task 12: the controls (kittest) ----

type H = egui_kittest::Harness<'static, AppModel>;

fn ui_harness(m: AppModel, w: f32) -> H {
    let mut h = egui_kittest::Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_step_dt(0.25)
        .with_size(eframe::egui::vec2(w, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    h
}

fn settle(h: &mut H) {
    assert!(h.state_mut().wait_idle(WAIT), "the model did not settle");
    h.run();
}

fn checkbox<'a>(h: &'a H, label: &'a str) -> egui_kittest::Node<'a> {
    h.get_by_role_and_label(Role::CheckBox, label)
}

fn has_label(h: &H, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

/// Click as a pointer does: settle on the widget first (egui hit-tests against the previous frame).
fn press(h: &mut H, node: impl Fn(&H) -> egui_kittest::Node<'_>) {
    node(h).scroll_to_me();
    h.run();
    node(h).hover();
    h.run();
    node(h).click();
    h.run();
}

/// `project` with Vanilla ticked and Review open (Export), drawn at `w`.
fn reviewing(d: &Path, w: f32) -> H {
    let mut m = project(d);
    tick(&mut m, AlsoTarget::Vanilla);
    for job in m.items.iter().filter_map(|i| i.job.as_ref()) {
        std::fs::create_dir_all(job.output.parent().unwrap()).unwrap();
    }
    m.export_clicked();
    idle(&mut m);
    assert!(m.review.is_some());
    m.review.as_mut().unwrap().options_open = true;
    let mut h = ui_harness(m, w);
    settle(&mut h);
    h
}

/// Note JF1: under What will be written the count is the PBR folder's and the Vanilla folder is named, at a narrow
/// window too (a cut line keeps its whole text as its name).
#[test]
fn reviews_footer_counts_the_pbr_folder_and_names_the_vanilla_folder() {
    for w in [1100.0, 1600.0] {
        let d = tempfile::tempdir().unwrap();
        let h = reviewing(d.path(), w);
        let n = h.state().export_files();
        let line = format!(
            "Will be written ({n}) · Up to date (0) in the PBR folder, plus the Vanilla folder"
        );
        assert!(has_label(&h, &line), "{w}: {line}");
        assert!(has_label(&h, &format!("Export {n} files + Vanilla folder")));
    }
}

#[test]
fn the_export_to_menu_toggles_the_ticks() {
    let d = tempfile::tempdir().unwrap();
    let mut h = ui_harness(project(d.path()), 1600.0);
    h.get_by_label_contains("Export to: ").click();
    h.run();
    press(&mut h, |h| checkbox(h, "Also export as Vanilla"));
    settle(&mut h);
    assert_eq!(h.state().also_ticks(), (true, false));
    assert!(
        has_label(&h, "Also export as Complex material"),
        "the menu stays open after a tick"
    );
    assert_eq!(h.state().undo_label(), Some("Also export as"));
    // Untick, then the vanilla mode: both items are off, with TA20's reason on hover.
    press(&mut h, |h| checkbox(h, "Also export as Vanilla"));
    settle(&mut h);
    assert!(!h.state().also_on());
    h.key_press(Key::Escape);
    h.run();
    h.state_mut().set_target(engine::config::Target::Vanilla);
    settle(&mut h);
    h.get_by_label_contains("Export to: ").click();
    h.run();
    for l in ["Also export as Vanilla", "Also export as Complex material"] {
        assert!(checkbox(&h, l).accesskit_node().is_disabled(), "{l}");
    }
    checkbox(&h, "Also export as Vanilla").hover();
    h.run();
    assert!(
        h.query_all_by_label_contains("needs the cs mode")
            .next()
            .is_some(),
        "TA20's reason on hover"
    );
}

#[test]
fn reviews_also_section_shows_the_ticks_location_and_skip_list() {
    let d = tempfile::tempdir().unwrap();
    let mut h = reviewing(d.path(), 1600.0);
    for l in ["Vanilla", "Complex material"] {
        checkbox(&h, l);
    }
    let loc = h.state().also_location().display().to_string();
    let node = h.get_by_label_contains("Also folders go in: ");
    // egui gives a Label's text to AccessKit as its value, which is its accessible name.
    let a = node.accesskit_node();
    assert_eq!(
        a.label().or(a.value()).unwrap_or_default(),
        format!("Also folders go in: {loc}")
    );
    // No recipe drags in Review: a link to a set's Also export tab instead (Note JI2).
    assert!(!has_label(&h, "Diffuse brightness"));
    assert!(has_label(&h, "Recipe settings…"));
    let steel = steel_id(h.state());
    let ends = |n: &egui_kittest::Node<'_>, end: &str| {
        n.accesskit_node().label().is_some_and(|l| l.ends_with(end))
    };
    // Only PBR sets are listed: the plain cup set is not.
    assert!(!h.query_all_by_role(Role::CheckBox).any(|n| ends(&n, "cup")));
    press(&mut h, |h| {
        h.query_all_by_role(Role::CheckBox)
            .find(|n| ends(n, "steel"))
            .expect("a Skip these sets row for steel")
    });
    settle(&mut h);
    assert_eq!(h.state().also_config().skip, [steel.as_str()]);
    assert_eq!(h.state().undo_label(), Some("Skip vanilla/CM for this set"));
    h.state_mut().undo();
    settle(&mut h);
    assert!(h.state().also_config().skip.is_empty(), "one undo step");
    // Note JI2: from an expanded texture of the PBR set, the link closes the preview and opens the set's Also export
    // tab, where the recipe is.
    let (steel_key, steel_tex) = {
        let all = sdc_gui::model::sets::views(h.state());
        let v = all.iter().find(|v| v.key.ends_with("steel")).unwrap();
        let tex = v
            .members
            .iter()
            .find(|k| k.ends_with("steel.png"))
            .expect("the steel diffuse")
            .clone();
        (v.key.clone(), tex)
    };
    h.state_mut().expand(&steel_tex);
    settle(&mut h);
    assert!(h.state().expanded.is_some());
    press(&mut h, |h| h.get_by_label("Recipe settings…"));
    settle(&mut h);
    assert!(h.state().expanded.is_none(), "the preview closed");
    assert_eq!(
        h.state().selection,
        sdc_gui::model::Selection::Set(steel_key)
    );
    let centre = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    let spin = h
        .get_by_role_and_label(Role::SpinButton, "Diffuse brightness")
        .rect();
    assert!(
        centre.contains(spin.center()),
        "the recipe {spin:?} is in the centre {centre:?}"
    );
}

#[test]
fn reviews_name_field_commits_on_focus_loss() {
    let d = tempfile::tempdir().unwrap();
    let mut h = reviewing(d.path(), 1600.0);
    fn field(h: &H) -> egui_kittest::Node<'_> {
        h.get_by_role_and_label(Role::TextInput, "Vanilla folder name")
    }
    field(&h).focus();
    h.run();
    field(&h).type_text("Plain");
    h.run();
    assert_eq!(
        h.state().also_config().names.vanilla,
        None,
        "not while typing"
    );
    h.key_press(Key::Tab);
    settle(&mut h);
    assert_eq!(
        h.state().also_config().names.vanilla.as_deref(),
        Some("Plain")
    );
    assert_eq!(h.state().undo_label(), Some("Also export settings"));
}

#[test]
fn reviews_export_stays_disabled_with_the_also_reason() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    m.save().unwrap();
    let path = sdc_toml(&m);
    let set_ro = |on: bool| {
        let mut p = std::fs::metadata(&path).unwrap().permissions();
        p.set_readonly(on);
        std::fs::set_permissions(&path, p).unwrap();
    };
    set_ro(true);
    m.export_clicked();
    idle(&mut m);
    let reason = sdc_gui::model::also::TOML_READ_ONLY;
    assert_eq!(m.review_gate(), Err(reason.to_string()));
    let mut h = ui_harness(m, 1600.0);
    settle(&mut h);
    let button = h
        .query_all_by_role(Role::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Export ") && l.ends_with(" + Vanilla folder"))
        })
        .expect("Review's Export");
    assert!(button.accesskit_node().is_disabled());
    // The reason is drawn beside the button and in Blockers.
    assert!(h.query_all_by_label(reason).count() >= 2);
    set_ro(false);
}

#[test]
fn the_dialog_free_cm_lines_follow_keep_metal_brightness() {
    use engine::presets::ExportConfig;
    use sdc_gui::ui::review::also_cm_lines;
    let mut c = ExportConfig {
        also: vec![AlsoTarget::Vanilla, AlsoTarget::Cm],
        ..ExportConfig::default()
    };
    let off = also_cm_lines(&c, false);
    assert!(off[0].contains("on top of the Vanilla folder"), "{off:?}");
    assert!(off.iter().any(|l| l.contains("only the `_m` files")));
    c.legacy.keep_metal_brightness = true;
    let on = also_cm_lines(&c, false);
    assert_eq!(on.len(), 1, "{on:?}");
    c.also = vec![AlsoTarget::Cm];
    assert!(also_cm_lines(&c, true)
        .iter()
        .any(|l| l.contains("the Vanilla folder is written too")));
}

/// The steel set's key (the set editor's selection).
fn steel_key(m: &AppModel) -> String {
    sdc_gui::model::sets::views(m)
        .iter()
        .find(|v| v.key.ends_with("steel"))
        .expect("the steel set")
        .key
        .clone()
}

/// `project` (Vanilla ticked when `ticked`) with the steel set selected and its Also export tab open (Note JI2).
fn also_tab(d: &Path, ticked: bool) -> H {
    also_tab_at(d, ticked, 1600.0)
}

/// `also_tab` drawn `w` wide.
fn also_tab_at(d: &Path, ticked: bool, w: f32) -> H {
    let mut m = project(d);
    if ticked {
        tick(&mut m, AlsoTarget::Vanilla);
    }
    let k = steel_key(&m);
    m.select_set(&k);
    let mut h = ui_harness(m, w);
    settle(&mut h);
    press(&mut h, |h| {
        h.get_by_role_and_label(Role::Button, "Also export")
    });
    h
}

fn skip_tick(h: &H) -> egui_kittest::Node<'_> {
    checkbox(h, "Skip vanilla/CM for this set")
}

fn brightness(h: &H) -> egui_kittest::Node<'_> {
    h.get_by_role_and_label(Role::SpinButton, "Diffuse brightness")
}

/// Note JI2 (correction 5): with both ticks off the Skip tick is drawn greyed and says to turn a tick on; with a tick
/// on it writes `[export] skip`, one undo step.
#[test]
fn the_skip_tick_is_greyed_while_both_ticks_are_off() {
    let d = tempfile::tempdir().unwrap();
    let mut h = also_tab(d.path(), false);
    assert!(skip_tick(&h).accesskit_node().is_disabled());
    assert!(hover_says(
        &mut h,
        skip_tick,
        "Turn on Also export as Vanilla or as Complex material below first"
    ));
    h.state_mut().set_also_tick(AlsoTarget::Vanilla, true);
    settle(&mut h);
    assert!(!skip_tick(&h).accesskit_node().is_disabled());
    let steel = steel_id(h.state());
    press(&mut h, skip_tick);
    settle(&mut h);
    assert_eq!(h.state().also_config().skip, [steel.as_str()]);
    assert_eq!(h.state().undo_label(), Some("Skip vanilla/CM for this set"));
}

/// Note JI2 (spec 2.3): the recipe is greyed while both ticks are off, and its hover says why; either tick turns it on.
#[test]
fn the_recipe_is_greyed_while_both_ticks_are_off() {
    let d = tempfile::tempdir().unwrap();
    let mut h = also_tab(d.path(), false);
    brightness(&h).scroll_to_me();
    h.run();
    assert!(brightness(&h).accesskit_node().is_disabled());
    assert!(hover_says(
        &mut h,
        brightness,
        "Turn on Also export as Vanilla or as Complex material to use these"
    ));
    h.state_mut().set_also_tick(AlsoTarget::Cm, true);
    settle(&mut h);
    assert!(!brightness(&h).accesskit_node().is_disabled());
}

/// Note JK1: under the recipe one sentence names its scope. It is not greyed with the boxes (a blind reader on
/// 2026-11-15 could not tell who the greyed boxes apply to), sits below the last box, and stays inside the centre at
/// 1100 px.
#[test]
fn the_recipe_says_where_it_is_saved_and_when_it_is_used() {
    const SCOPE: &str =
        "Saved in this project's sdc.toml; used when Vanilla or Complex material is ticked";
    for (ticked, w) in [(false, 1600.0), (false, 1100.0), (true, 1600.0)] {
        let d = tempfile::tempdir().unwrap();
        let mut h = also_tab_at(d.path(), ticked, w);
        let sentence = |h: &H| h.get_by_role_and_label(Role::Label, SCOPE).rect();
        h.get_by_role_and_label(Role::Label, SCOPE).scroll_to_me();
        h.run();
        let node = h.get_by_role_and_label(Role::Label, SCOPE);
        assert!(
            !node.accesskit_node().is_disabled(),
            "the sentence is not greyed (ticked {ticked}, {w} px)"
        );
        let last = h
            .get_by_role_and_label(Role::SpinButton, "Dielectric env from gloss")
            .rect();
        let s = sentence(&h);
        assert!(
            s.min.y >= last.max.y,
            "the sentence {s:?} is under the last box {last:?} ({w} px)"
        );
        let centre = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
        assert!(
            s.max.x <= centre.max.x + 0.5,
            "the sentence {s:?} fits the centre {centre:?} ({w} px)"
        );
    }
}

/// Note JI2: the recipe moved from the drawer to the Also export tab. A typed value and a drag of a box each end in
/// one edit (one undo step), not one per frame.
#[test]
fn recipe_drags_in_the_also_export_tab_write_export_legacy() {
    use eframe::egui::{pos2, Event, PointerButton};
    let d = tempfile::tempdir().unwrap();
    let mut h = also_tab(d.path(), true);
    brightness(&h).scroll_to_me();
    h.run();
    brightness(&h).focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    h.event(eframe::egui::Event::Text("0.7".into()));
    h.run();
    assert_eq!(
        h.state().also_config().legacy.brightness,
        0.9,
        "not while typing"
    );
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(h.state().also_config().legacy.brightness, 0.7);
    assert_eq!(h.state().undo_label(), Some("Also export settings"));
    h.state_mut().undo();
    settle(&mut h);
    assert_eq!(
        h.state().also_config().legacy.brightness,
        0.9,
        "one undo step"
    );
    // P17: Keep metal brightness is only in Review; Dielectric env is one number box.
    assert!(!has_label(&h, "Keep metal brightness"));
    h.get_by_role_and_label(Role::SpinButton, "Dielectric env from gloss");
    // A drag follows the pointer and ends in one edit.
    brightness(&h).scroll_to_me();
    h.run();
    let at = brightness(&h).rect().center();
    let ev = |h: &mut H, e: Event| {
        h.input_mut().events.push(e);
        h.step();
    };
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    ev(&mut h, Event::PointerMoved(at));
    ev(&mut h, button(at, true));
    for i in 1..=10 {
        ev(
            &mut h,
            Event::PointerMoved(pos2(at.x + 2.0 * i as f32, at.y)),
        );
    }
    assert_eq!(
        h.state().also_config().legacy.brightness,
        0.9,
        "nothing written during the drag"
    );
    ev(&mut h, button(pos2(at.x + 20.0, at.y), false));
    settle(&mut h);
    let b = h.state().also_config().legacy.brightness;
    assert!(b > 1.0, "the drag moved the value: {b}");
    assert_eq!(h.state().undo_label(), Some("Also export settings"));
    h.state_mut().undo();
    settle(&mut h);
    assert_eq!(
        h.state().also_config().legacy.brightness,
        0.9,
        "one undo step"
    );
}

/// Note JI2 (final review I1): a recipe value outside its box's range, hand-written in `sdc.toml`, is left as it is:
/// opening the tab and drawing the box write nothing (no clamp, no undo step, nothing to save).
#[test]
fn an_out_of_range_recipe_value_survives_opening_the_tab() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("mods/Mod");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("sdc.toml"),
        "[export]\nalso = [\"vanilla\"]\n\n[export.legacy]\nlandscape_spec = 6.0\n",
    )
    .unwrap();
    let mut m = project(d.path());
    assert!(m.also_on(), "fixture: Vanilla ticked");
    assert_eq!(
        m.also_config().legacy.landscape_spec,
        6.0,
        "fixture: the value is read"
    );
    let k = steel_key(&m);
    m.select_set(&k);
    // Opening the folder may already leave edits to save (an accepted suggestion); the tab must add none.
    let before = m.project.as_ref().unwrap().text();
    let mut h = ui_harness(m, 1600.0);
    settle(&mut h);
    press(&mut h, |h| {
        h.get_by_role_and_label(Role::Button, "Also export")
    });
    fn spec(h: &H) -> egui_kittest::Node<'_> {
        h.get_by_role_and_label(Role::SpinButton, "Landscape spec factor")
    }
    spec(&h).scroll_to_me();
    h.run();
    assert!(!spec(&h).accesskit_node().is_disabled(), "the box is live");
    settle(&mut h);
    assert_eq!(h.state().also_config().legacy.landscape_spec, 6.0);
    assert_ne!(h.state().undo_label(), Some("Also export settings"));
    assert_eq!(
        h.state().project.as_ref().unwrap().text(),
        before,
        "sdc.toml's text is unchanged"
    );
}

/// Note JI2 (final review I2): with the Settings tab in front of the centre, Review's "Recipe settings…" brings the
/// selected set's Also export tab up, and its one-shot scroll request is used.
#[test]
fn recipe_link_over_the_settings_tab_shows_the_also_export_tab() {
    use sdc_gui::model::CentreTab;
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    let k = steel_key(&m);
    m.select_set(&k);
    m.open_settings_tab();
    m.open_review();
    m.review.as_mut().unwrap().options_open = true;
    let mut h = ui_harness(m, 1600.0);
    settle(&mut h);
    assert_eq!(h.state().centre_front(), CentreTab::Settings, "fixture");
    assert!(h
        .query_by_role_and_label(Role::SpinButton, "Diffuse brightness")
        .is_none());
    press(&mut h, |h| h.get_by_label("Recipe settings…"));
    settle(&mut h);
    assert_eq!(h.state().centre_front(), CentreTab::Selection);
    assert_eq!(h.state().selection, sdc_gui::model::Selection::Set(k));
    brightness(&h);
    let left = h
        .ctx
        .data(|d| d.get_temp::<bool>(eframe::egui::Id::unique("set-editor-tab-scroll")));
    assert_eq!(left, None, "the scroll request was used");
}

/// Note JI2 (rule 8.4): the tab's project ticks and Export to ▾'s items are on screen together, so the tab's carry
/// "All sets: …" names; a click on one writes `[export] also` as the header's does.
#[test]
fn all_sets_ticks_have_their_own_names() {
    let d = tempfile::tempdir().unwrap();
    let mut h = also_tab(d.path(), false);
    h.get_by_label_contains("Export to: ").click();
    h.run();
    for t in ["Vanilla", "Complex material"] {
        checkbox(&h, &format!("Also export as {t}"));
        checkbox(&h, &format!("All sets: also export as {t}"));
    }
    h.key_press(Key::Escape);
    h.run();
    press(&mut h, |h| checkbox(h, "All sets: also export as Vanilla"));
    settle(&mut h);
    assert_eq!(h.state().also_ticks(), (true, false));
    assert_eq!(h.state().undo_label(), Some("Also export as"));
    // The tick moves the output root, and so the set keys: the set stays selected, its tab open, its Skip tick on.
    assert_eq!(
        h.state().selection,
        sdc_gui::model::Selection::Set(steel_key(h.state()))
    );
    assert!(!skip_tick(&h).accesskit_node().is_disabled());
}

/// Note JI2 (spec 2.2): the Material tab's "Also export" group is gone; the Skip tick is only on the Also export tab.
#[test]
fn the_material_tab_has_no_also_export_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    let k = steel_key(&m);
    m.select_set(&k);
    let mut h = ui_harness(m, 1600.0);
    settle(&mut h);
    assert_eq!(
        h.query_all_by_label("Also export").count(),
        1,
        "one node named Also export"
    );
    assert!(
        h.query_all_by_label("Also export")
            .all(|n| n.accesskit_node().role() == Role::Button),
        "only the tab is named Also export"
    );
    assert!(!has_label(&h, "Skip vanilla/CM for this set"));
    assert!(!has_label(&h, "converted"));
}

/// Note JI2: from 3D, where the set editor is not drawn, the link switches the view to 2D.
#[test]
fn recipe_link_from_3d_switches_to_2d() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    m.settings.set_view_3d = true;
    m.open_review();
    m.review.as_mut().unwrap().options_open = true;
    let steel = steel_key(&m);
    let mut h = ui_harness(m, 1600.0);
    settle(&mut h);
    press(&mut h, |h| h.get_by_label("Recipe settings…"));
    settle(&mut h);
    assert!(!h.state().settings.set_view_3d);
    assert_eq!(h.state().selection, sdc_gui::model::Selection::Set(steel));
    h.get_by_role_and_label(Role::SpinButton, "Diffuse brightness");
}

/// Note JI2 (correction 6): the link's set is the selected set if it is PBR, else the first PBR set not skipped, else
/// the first PBR set.
#[test]
fn recipe_set_prefers_the_selected_pbr_set_then_the_first_unskipped() {
    let d = tempfile::tempdir().unwrap();
    let arm = d.path().join("mods/Mod/textures/pbr/arm");
    png(&arm.join("brass.png"), [200, 180, 100, 255]);
    png(&arm.join("brass_n.png"), [128, 128, 255, 255]);
    png(&arm.join("brass_rmaos.png"), [128, 255, 200, 255]);
    let mut m = project(d.path());
    let all = sdc_gui::model::sets::views(&m);
    let pbr: Vec<(SetTarget, String)> = all
        .iter()
        .filter(|v| sdc_gui::model::also::is_pbr_set(v))
        .map(|v| (SetTarget::view(v), v.id.clone()))
        .collect();
    assert_eq!(pbr.len(), 2, "brass and steel are PBR sets");
    let cup = all
        .iter()
        .find(|v| v.key.ends_with("cup"))
        .unwrap()
        .key
        .clone();

    m.select_set(&cup);
    assert_eq!(
        m.recipe_set().as_ref(),
        Some(&pbr[0].0),
        "not PBR: the first"
    );
    m.set_skip_also(&pbr[0].1, true);
    idle(&mut m);
    assert_eq!(
        m.recipe_set().as_ref(),
        Some(&pbr[1].0),
        "the first not skipped"
    );
    m.set_skip_also(&pbr[1].1, true);
    idle(&mut m);
    assert_eq!(
        m.recipe_set().as_ref(),
        Some(&pbr[0].0),
        "all skipped: the first"
    );
    m.select_target(pbr[1].0.clone());
    assert_eq!(
        m.recipe_set().as_ref(),
        Some(&pbr[1].0),
        "the selected PBR set"
    );

    let target = add_keyless_pbr(&mut m);
    m.select_target(target.clone());
    m.set_skip_also(&steel_id(&m), false);
    m.set_skip_also(&steel_key(&m), true);
    idle(&mut m);
    assert_eq!(m.recipe_set(), Some(target.clone()), "selected by id");
    m.select_set(&cup);
    m.set_skip_also(&steel_id(&m), true);
    m.set_skip_also(&steel_key(&m), false);
    idle(&mut m);
    assert_eq!(m.recipe_set(), Some(target), "the unskipped key-less set");
}

// The new card's document id is also the steel card's engine key.
fn add_keyless_pbr(m: &mut AppModel) -> SetTarget {
    let id = steel_key(m);
    let source = steel_id(m);
    m.edit_docs("Create PBR borrower", None, true, |_, doc| {
        doc.create_set(&id);
        doc.set_borrow(&id, 1, Some(&source));
    });
    idle(m);
    let all = sdc_gui::model::sets::views(m);
    assert!(all.iter().any(|v| v.key == id && v.id == source));
    assert!(all.iter().any(|v| v.key.is_empty() && v.id == id && v.pbr));
    SetTarget::Id(id)
}

#[test]
fn recipe_link_opens_the_keyless_pbr_cards_also_tab() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    let target = add_keyless_pbr(&mut m);
    m.set_skip_also(&steel_key(&m), true);
    idle(&mut m);
    m.select_target(target.clone());
    m.settings.set_view_3d = true;
    m.open_review();
    m.review.as_mut().unwrap().options_open = true;
    let mut h = ui_harness(m, 1600.0);
    press(&mut h, |h| h.get_by_label("Recipe settings…"));
    settle(&mut h);
    assert_eq!(h.state().selection.set_target(), Some(target));
    assert!(!h.state().settings.set_view_3d);
    h.get_by_role_and_label(Role::SpinButton, "Diffuse brightness");
}

/// Note JI2 (Review Focus 1): with no PBR set there is no set to open, so the link is not drawn.
#[test]
fn the_recipe_link_is_not_drawn_without_a_pbr_set() {
    let d = tempfile::tempdir().unwrap();
    // Only `project`'s plain cup set (support's `opened` has an auto PBR Landscape set).
    let c = d.path().join("mods/Mod/textures/clutter");
    png(&c.join("cup.png"), [120, 90, 60, 255]);
    png(&c.join("cup_n.png"), [128, 128, 255, 255]);
    let mut m = support::opened_as_the_app(d.path().join("mods/Mod"), d.path().join("out"));
    assert!(!sdc_gui::model::sets::views(&m).is_empty());
    assert!(sdc_gui::model::sets::views(&m)
        .iter()
        .all(|v| !sdc_gui::model::also::is_pbr_set(v)));
    assert_eq!(m.recipe_set(), None);
    m.open_review();
    m.review.as_mut().unwrap().options_open = true;
    let mut h = ui_harness(m, 1600.0);
    settle(&mut h);
    assert!(has_label(&h, "Also export as"), "Review's Also column");
    assert!(!has_label(&h, "Recipe settings…"));
}

#[test]
fn an_also_file_click_in_problems_reveals_the_file() {
    use sdc_gui::model::shell::ShellRequest;
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    export(&mut m);
    let key = "Mod Vanilla/textures/arm/steel.dds";
    m.export.on_event(EngineEvent {
        schema: EVENT_SCHEMA,
        seq: 0,
        ts: String::new(),
        batch: "b".into(),
        job: Some(key.into()),
        set: None,
        path: None,
        kind: EventKind::Warning(Warning::new(Code::OutUnknownExisting, "kept")),
    });
    m.invalidate_views();
    m.open_problems(None);
    m.take_shell();
    let sel = m.selection.clone();
    let mut h = ui_harness(m, 1600.0);
    press(&mut h, |h| h.get_by_label(key));
    let file = d.path().join("out/Mod Vanilla/textures/arm/steel.dds");
    assert_eq!(h.state_mut().take_shell(), [ShellRequest::Reveal(file)]);
    assert_eq!(h.state().selection, sel, "nothing selected");
}

#[test]
fn the_header_export_hover_names_the_folders() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let plain = sdc_gui::ui::header::export_hover(&m);
    assert!(!plain.contains("folders"), "{plain}");
    tick(&mut m, AlsoTarget::Cm);
    let t = sdc_gui::ui::header::export_hover(&m);
    let files = sdc_gui::model::code_text::counted(m.export_files(), "file", "files");
    assert!(
        t.contains(&format!("{files} + Vanilla and CM folders, ")),
        "{t}"
    );
}

// ---- Task 12 fix round 1 ----

fn ui_harness_sized(m: AppModel, w: f32, hgt: f32) -> H {
    let mut h = egui_kittest::Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_step_dt(0.25)
        .with_size(eframe::egui::vec2(w, hgt))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    h
}

/// Hover `node` and say whether a tooltip containing `text` shows.
fn hover_says(h: &mut H, node: impl Fn(&H) -> egui_kittest::Node<'_>, text: &str) -> bool {
    node(h).hover();
    h.run_steps(6);
    h.query_all_by_label_contains(text).next().is_some()
}

/// I1 (review ruling): in the cs mode Review has the Also column without a tick (unchecked, no reason line);
/// in the vanilla mode it has none.
#[test]
fn cs_review_without_a_tick_has_the_also_column() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    assert!(!m.also_on());
    m.open_review();
    m.review.as_mut().unwrap().options_open = true;
    let mut h = ui_harness(m, 1600.0);
    settle(&mut h);
    assert!(has_label(&h, "Also export as"));
    let v = checkbox(&h, "Vanilla");
    assert_ne!(
        v.accesskit_node().toggled(),
        Some(eframe::egui::accesskit::Toggled::True)
    );
    assert!(!v.accesskit_node().is_disabled());
    assert!(
        h.query_all_by_label_contains("Also export needs")
            .next()
            .is_none(),
        "no reason line without a tick"
    );
    h.state_mut().set_target(engine::config::Target::Vanilla);
    settle(&mut h);
    assert!(!has_label(&h, "Also export as"));
}

/// CN76: a tick left from the cs mode stays clickable in the vanilla mode, in Export to ▾ and in Review.
#[test]
fn a_stale_tick_can_be_cleared_in_the_vanilla_mode() {
    for from_review in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let mut m = project(d.path());
        tick(&mut m, AlsoTarget::Vanilla);
        m.set_target(engine::config::Target::Vanilla);
        idle(&mut m);
        assert!(m.also_on());
        let label = if from_review {
            m.open_review();
            m.review.as_mut().unwrap().options_open = true;
            "Vanilla"
        } else {
            "Also export as Vanilla"
        };
        let mut h = ui_harness(m, 1600.0);
        settle(&mut h);
        if !from_review {
            h.get_by_label_contains("Export to: ").click();
            h.run();
        }
        assert!(
            !checkbox(&h, label).accesskit_node().is_disabled(),
            "{label}"
        );
        press(&mut h, |h| checkbox(h, label));
        settle(&mut h);
        assert!(!h.state().also_on(), "{label} cleared the tick");
    }
}

/// While an export runs every Also control is off, with the export as the reason (M1: "Choose…" too).
#[test]
fn the_also_controls_are_off_while_an_export_runs() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    let steel_key = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.ends_with("steel"))
        .unwrap()
        .key
        .clone();
    m.select_set(&steel_key);
    m.open_review();
    m.review.as_mut().unwrap().options_open = true;
    m.export.running = Some(Default::default());
    let mut h = ui_harness(m, 1600.0);
    h.run_steps(4);
    // Note JI2: the Skip tick, the project's ticks and the recipe are on the set's Also export tab.
    h.get_by_role_and_label(Role::Button, "Also export")
        .scroll_to_me();
    h.run_steps(4);
    h.get_by_role_and_label(Role::Button, "Also export").click();
    h.run_steps(4);
    for l in [
        "Vanilla",
        "Skip vanilla/CM for this set",
        "All sets: also export as Vanilla",
    ] {
        assert!(checkbox(&h, l).accesskit_node().is_disabled(), "{l}");
    }
    assert!(brightness(&h).accesskit_node().is_disabled());
    let running = sdc_gui::model::also::EXPORT_RUNNING;
    assert!(hover_says(&mut h, skip_tick, running));
    assert!(hover_says(
        &mut h,
        |h| h.get_by_label("Choose where the Also folders go…"),
        running
    ));
    // M1: the export is the reason, not a file dialog.
    assert!(
        h.query_all_by_label_contains(sdc_gui::model::picker::BUSY_HOVER)
            .next()
            .is_none(),
        "Choose… blames a file dialog"
    );
    h.get_by_label_contains("Export to: ").click();
    h.run_steps(4);
    for l in ["Also export as Vanilla", "Also export as Complex material"] {
        assert!(checkbox(&h, l).accesskit_node().is_disabled(), "{l}");
    }
    assert!(hover_says(
        &mut h,
        |h| checkbox(h, "Also export as Complex material"),
        sdc_gui::ui::header::EXPORT_RUNNING_HOVER
    ));
    h.state_mut().export.running = None;
}

/// 1e56e4b: with no folder open the menu items are off and say why.
#[test]
fn the_menu_items_need_an_open_folder() {
    let m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    let mut h = ui_harness(m, 1600.0);
    // Note FB1: with no folder chosen the menu is a plain "More".
    h.get_by_label("More export options").click();
    h.run();
    for l in ["Also export as Vanilla", "Also export as Complex material"] {
        assert!(checkbox(&h, l).accesskit_node().is_disabled(), "{l}");
    }
    assert!(hover_says(
        &mut h,
        |h| checkbox(h, "Also export as Vanilla"),
        "Open a folder first"
    ));
}

/// More than 8 PBR sets: the list scrolls in its own area, and its last row is reachable and writes `skip`.
#[test]
fn a_long_skip_list_scrolls_and_its_last_row_works() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("mods/Many");
    for i in 0..10 {
        let t = root.join(format!("textures/pbr/arm/set{i:02}"));
        png(&t.with_extension("png"), [200, 200, 200, 255]);
        png(
            &root.join(format!("textures/pbr/arm/set{i:02}_n.png")),
            [128, 128, 255, 255],
        );
        png(
            &root.join(format!("textures/pbr/arm/set{i:02}_rmaos.png")),
            [128, 255, 200, 255],
        );
    }
    let mut m = support::opened_as_the_app(root, d.path().join("out"));
    tick(&mut m, AlsoTarget::Vanilla);
    m.open_review();
    m.review.as_mut().unwrap().options_open = true;
    let last = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.id.ends_with("set09"))
        .expect("set09")
        .id
        .clone();
    let mut h = ui_harness_sized(m, 1600.0, 1400.0);
    settle(&mut h);
    // Only the rows on screen are drawn (show_rows): set09 has no node until the list scrolls.
    let ends = |h: &H, end: &str| {
        h.query_all_by_role(Role::CheckBox)
            .any(|n| n.accesskit_node().label().is_some_and(|l| l.ends_with(end)))
    };
    assert!(ends(&h, "set00") && !ends(&h, "set09"), "8 rows, not 10");
    // A wheel over `label`'s widget, as a user scrolls: first the column, then the list inside it.
    let wheel = |h: &mut H, label: &str| {
        let at = h
            .query_all_by_role(Role::CheckBox)
            .find(|n| {
                n.accesskit_node()
                    .label()
                    .is_some_and(|l| l.ends_with(label))
            })
            .unwrap()
            .rect()
            .center();
        h.input_mut()
            .events
            .push(eframe::egui::Event::PointerMoved(at));
        h.run();
        h.input_mut().events.push(eframe::egui::Event::MouseWheel {
            unit: eframe::egui::MouseWheelUnit::Point,
            delta: eframe::egui::vec2(0.0, -400.0),
            source: eframe::egui::MouseWheelSource::Wheel,
            phase: eframe::egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        h.run_steps(10);
    };

    wheel(&mut h, "Vanilla");
    wheel(&mut h, "set00");
    assert!(ends(&h, "set09"), "the inner list scrolled to its end");
    fn row(h: &H) -> egui_kittest::Node<'_> {
        h.query_all_by_role(Role::CheckBox)
            .find(|n| {
                n.accesskit_node()
                    .label()
                    .is_some_and(|l| l.ends_with("set09"))
            })
            .expect("the set09 row")
    }
    press(&mut h, row);
    settle(&mut h);
    assert_eq!(h.state().also_config().skip, [last.as_str()]);
}

/// Esc cancels a name field; a draft never carries into another project (M2).
#[test]
fn a_name_draft_is_cancelled_by_esc_and_stays_with_its_project() {
    fn field(h: &H) -> egui_kittest::Node<'_> {
        h.get_by_role_and_label(Role::TextInput, "Vanilla folder name")
    }
    let d = tempfile::tempdir().unwrap();
    let mut h = reviewing(d.path(), 1600.0);
    field(&h).focus();
    h.run();
    field(&h).type_text("Plain");
    h.run();
    h.key_press(Key::Escape);
    settle(&mut h);
    assert_eq!(h.state().also_config().names.vanilla, None, "Esc cancels");
    // A draft left in the field while another folder opens.
    field(&h).focus();
    h.run();
    field(&h).type_text("Plain");
    h.run();
    let other = tempfile::tempdir().unwrap();
    let mut b = project(other.path());
    tick(&mut b, AlsoTarget::Vanilla);
    b.save().unwrap();
    let b_root = b.root.clone().unwrap();
    drop(b);
    h.state_mut().open_folder(b_root);
    settle(&mut h);
    h.state_mut().export_clicked();
    settle(&mut h);
    assert!(h.state().review.is_some());
    h.state_mut().review.as_mut().unwrap().options_open = true;
    settle(&mut h);
    assert_ne!(
        field(&h).accesskit_node().value().as_deref(),
        Some("Plain"),
        "the other project's draft is not shown"
    );
    field(&h).focus();
    h.run();
    h.key_press(Key::Tab);
    settle(&mut h);
    assert_eq!(h.state().also_config().names.vanilla, None);
}

/// Note FC5: with Also on, the NIF edits are due once, after the Also run has fully ended (its zips too).
#[test]
fn nif_export_is_due_once_after_an_also_run_ends() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    m.set_nif_review_rows(vec![sdc_gui::model::meshes::NifReviewRow {
        path: r"meshes\x.nif".into(),
        mod_name: "Mod".into(),
        blocked: None,
        foreign: None,
        answer: None,
        compaction: None,
    }]);
    export(&mut m);
    assert!(m.export.summary.is_some(), "{}", m.status_line);
    assert!(m.export.running.is_none());
    assert!(m.take_nif_export_due(), "due after the run");
    assert!(!m.take_nif_export_due(), "once");
}

/// Backlog T29 (Note FP1), E3: a tick leaves the header on Review and takes `export_is_clean` away (the
/// write-now label under the opt-out is pinned in export_shell.rs on a row-free project; this fixture keeps warnings).
#[test]
fn a_tick_makes_the_header_read_review() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    assert!(m.export_is_clean(), "clean before the tick");
    assert!(
        m.header_export_text().starts_with("Review "),
        "{}",
        m.header_export_text()
    );
    tick(&mut m, AlsoTarget::Vanilla);
    assert!(!m.export_is_clean());
    assert!(
        m.header_export_text().starts_with("Review "),
        "{}",
        m.header_export_text()
    );
}

/// Backlog T31 (Note FP5): clearing the read-only flag from the app refreshes Review's cached `sdc.toml` check, so
/// Export is not left disabled with a reason that no longer holds.
#[test]
fn clearing_the_read_only_flag_reopens_reviews_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    tick(&mut m, AlsoTarget::Vanilla);
    m.save().unwrap();
    let path = sdc_toml(&m);
    let mut p = std::fs::metadata(&path).unwrap().permissions();
    p.set_readonly(true);
    std::fs::set_permissions(&path, p).unwrap();
    // An unsaved edit: the flag is cleared without a reload, and Save writes it.
    m.set_field(&m.items[0].key.clone(), "format", serde_json::json!("bc7"));
    idle(&mut m);
    m.open_review();
    assert!(m.review_gate().is_err());
    m.clear_read_only_flag(&path).unwrap();
    m.save().unwrap();
    idle(&mut m);
    assert_eq!(m.review_gate().map(|_| ()), Ok(()));
}

/// Note FC7 (CN3), moved from the removed Generate tests: a `_curves_generated` file on disk (an old Generate's
/// output, or one made by hand) that no recipe reads is Also export's input. Its Problems row is Info with no button,
/// not the generic "remove it" warning.
#[test]
fn a_curves_generated_row_is_also_only() {
    use sdc_gui::model::bottom::Level;
    const KEY: &str = "textures/pbr/arm/steel_curves_generated.png";
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let file = d.path().join("mods/Mod").join(KEY);
    ImageBuffer::from_fn(16, 16, |x, _| image::Luma([(x * 4000) as u16]))
        .save(&file)
        .unwrap();
    m.replan();
    idle(&mut m);
    let item = m.item(KEY).expect("the generated file is a source");
    assert!(
        item.warnings.iter().any(|w| w.code == Code::PackUnusedInput
            && w.message
                .contains("read only by Also export as (Vanilla / Complex material)")),
        "{:?}",
        item.warnings
    );
    let rows: Vec<_> = problems(&m)
        .iter()
        .filter(|p| p.at == At::Texture(KEY.into()))
        .cloned()
        .collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        (rows[0].level, rows[0].text.as_str(), rows[0].fix.is_none()),
        (
            Level::Info,
            "Read only by Also export; not written on its own",
            true
        )
    );
}
