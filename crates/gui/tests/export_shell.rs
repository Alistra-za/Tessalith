//! Review E export shell: the per-project "Export without review" key in `sdc-project.toml` (Task 1).
mod support;
use eframe::egui;
use egui_kittest::Harness;
use engine::presets::AlsoTarget;
use sdc_gui::model::{AppModel, Dialog};
use std::time::Duration;

use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use sdc_gui::settings::GuiSettings;

use sdc_gui::model::bottom::{cached_tab_counts, flagged, tab_label, BottomTab, ProblemsFilter};
use sdc_gui::model::meshes::{NifLevel, NifProblem, NifTab};

const NIF_HEADING: &str = "From the NIF you are editing";
const NIF_HOVER: &str = "2 of these come from the NIF you are editing.";

fn editing_nif(dir: &std::path::Path) -> (Harness<'static, AppModel>, usize) {
    let mut m = support::opened(dir);
    support::decide_all(&mut m);
    m.skipped_user_presets = vec![("broken.toml".into(), "invalid preset".into())];
    m.unmatched_keys = vec!["gone.png".into()]; // A project note with its own action.
    m.invalidate_views();
    let ready = flagged(&m).ready_rows;
    m.set_nif_problems(
        vec![
            support::nif_problem_warning("Missing texture"),
            NifProblem {
                level: NifLevel::Blocks,
                ..support::nif_problem_warning("Invalid shader")
            },
            NifProblem {
                level: NifLevel::Info,
                tab: NifTab::Alpha,
                field: Some(nif::fields::Field::AlphaFlags),
                ..support::nif_problem_warning("Alpha flags changed")
            },
        ]
        .into(),
    );
    m.show_tab(BottomTab::Problems);
    let mut h = harness(m);
    settle(&mut h);
    (h, ready)
}

#[test]
fn nif_edit_problems_are_named_as_such() {
    let d = tempfile::tempdir().unwrap();
    let (mut h, ready) = editing_nif(d.path());
    assert_eq!(flagged(h.state()).ready_rows, ready + 2);
    let block = h.get_by_label("Invalid shader").rect();
    let warning = h.get_by_label("Missing texture").rect();
    let other = h
        .get_by_label("Preset file broken.toml was skipped: invalid preset")
        .rect();
    assert!(
        h.query_by_label(&format!("{NIF_HEADING} (2)"))
            .is_some_and(|header| {
                header.rect().bottom() <= block.top()
                    && block.bottom() <= warning.top()
                    && warning.bottom() <= other.top()
            }),
        "the warning follows the block under the two-row NIF heading"
    );
    let row = "meshes/rock.nif: Missing texture";
    h.get_by_label(row).focus();
    h.run();
    h.state_mut()
        .set_nif_problems(vec![support::nif_problem_warning("Missing texture")].into());
    settle(&mut h);
    assert!(h.get_by_label(row).accesskit_node().is_focused());
    assert!(warning.bottom() <= h.get_by_label("Missing texture").rect().top());
}

#[test]
fn nif_edit_problems_keep_a_lone_blocker_ahead_of_project_warnings_and_checks() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    for normal in ["a_n.png", "u_n.png"] {
        support::give_a_check_at(&mut m, &d.path().join("textures/g").join(normal));
    }
    m.skipped_user_presets = ["first.toml", "second.toml"]
        .map(|name| (name.into(), "invalid preset".into()))
        .into();
    m.set_nif_problems(
        vec![NifProblem {
            level: NifLevel::Blocks,
            ..support::nif_problem_warning("Invalid shader")
        }]
        .into(),
    );
    m.show_tab(BottomTab::Problems);
    let mut h = harness(m);
    settle(&mut h);
    let block = h.get_by_label("Invalid shader").rect();
    assert!(
        h.get_by_label(&format!("{NIF_HEADING} (1)"))
            .rect()
            .bottom()
            <= block.top()
    );
    for name in ["first.toml", "second.toml"] {
        let row = h
            .get_by_label(&format!("Preset file {name} was skipped: invalid preset"))
            .rect();
        assert!(block.bottom() <= row.top(), "blocker before {name}");
    }
    let check = sdc_gui::model::code_text::code_line(engine::codes::Code::ChkZSign);
    for key in ["g/a_n.png", "g/u_n.png"] {
        let row = h.get_by_label(&format!("{key}: {check}")).rect();
        assert!(
            block.bottom() <= row.top(),
            "blocker before the check on {key}"
        );
    }
}

#[test]
fn nif_edit_problems_include_a_block_on_its_own() {
    let d = tempfile::tempdir().unwrap();
    let (mut h, _) = editing_nif(d.path());
    h.get_by_label("Mark as intended: meshes/rock.nif: Missing texture")
        .click();
    settle(&mut h);
    assert!(h.query_by_label("Missing texture").is_none());
    let block = h.get_by_label("Invalid shader").rect();
    assert!(
        h.query_by_label(&format!("{NIF_HEADING} (1)"))
            .is_some_and(|n| n.rect().bottom() <= block.top()),
        "block under its counted heading"
    );
}

#[test]
fn nif_edit_problems_leave_info_among_the_actionable_notes() {
    let d = tempfile::tempdir().unwrap();
    let (mut h, _) = editing_nif(d.path());
    let note = h.get_by_label("Alpha flags changed").rect();
    let project = h
        .get_by_label_contains("Project: 1 saved setting names no file")
        .rect();
    assert!(h.get_by_label("Missing texture").rect().bottom() <= project.top());
    assert!(project.bottom() <= note.top());
    h.get_by_label("Open the Alpha tab").click();
    h.run();
    assert_eq!(
        h.state().nif_tab_request,
        Some((
            "meshes/rock.nif".into(),
            NifTab::Alpha,
            Some(nif::fields::Field::AlphaFlags)
        ))
    );
    assert!(
        h.query_by_label(&format!("{NIF_HEADING} (2)")).is_some(),
        "the Info note is outside the two counted problems"
    );
}

fn hover_problems(h: &mut Harness<'static, AppModel>) {
    let label = tab_label(cached_tab_counts(h.state()));
    h.get_by_label(&label).hover();
    h.run();
}

#[test]
fn nif_edit_problems_build_the_collapsed_list_only_on_hover() {
    use sdc_gui::model::bottom::filtered_builds;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    m.collapse_bottom();
    let mut h = harness(m);
    settle(&mut h);
    let before = filtered_builds(h.state());
    h.state_mut()
        .set_nif_problems(vec![support::nif_problem_warning("Missing texture")].into());
    settle(&mut h);
    assert_eq!(
        filtered_builds(h.state()),
        before,
        "collapsed without hover"
    );
    hover_problems(&mut h);
    h.get_by_label("1 of these comes from the NIF you are editing.");
    assert_eq!(filtered_builds(h.state()), before + 1);
}

#[test]
fn nif_edit_problems_explain_the_tab_count_on_hover() {
    let d = tempfile::tempdir().unwrap();
    let (mut h, _) = editing_nif(d.path());
    hover_problems(&mut h);
    assert!(
        h.query_by_label(NIF_HOVER).is_some(),
        "two NIF problems on tab hover"
    );
    let key = sdc_gui::model::sets::set_views(h.state())[0].key.clone();
    let scope = ProblemsFilter::Set(key);
    h.state_mut().open_problems(Some(scope.clone()));
    settle(&mut h);
    assert!(h.query_by_label_contains(NIF_HEADING).is_none());
    hover_problems(&mut h);
    assert!(h.query_by_label(NIF_HOVER).is_some());
    assert_eq!(h.state().bottom.filter.as_ref(), Some(&scope));
}

#[test]
fn nif_edit_problems_hide_the_heading_and_hover_at_zero() {
    let d = tempfile::tempdir().unwrap();
    let (mut h, ready) = editing_nif(d.path());
    let before = h.query_all_by_label_contains(NIF_HEADING).count();
    h.state_mut().set_nif_problems(
        vec![NifProblem {
            level: NifLevel::Info,
            ..support::nif_problem_warning("Only a note")
        }]
        .into(),
    );
    settle(&mut h);
    assert_eq!(flagged(h.state()).ready_rows, ready);
    h.get_by_label("Only a note");
    let after = h.query_all_by_label_contains(NIF_HEADING).count();
    assert_eq!(
        (before, after),
        (1, 0),
        "removing the last NIF problems removes their heading"
    );
    hover_problems(&mut h);
    assert!(
        h.query_by_label_contains("from the NIF you are editing.")
            .is_none(),
        "no zero-count hover"
    );
    h.state_mut()
        .set_nif_problems(vec![support::nif_problem_warning("Missing texture")].into());
    settle(&mut h);
    assert!(
        h.query_by_label(&format!("{NIF_HEADING} (1)")).is_some(),
        "the heading returns for one warning"
    );
}

#[test]
fn the_header_number_is_the_progress_total_and_the_result_sum() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(support::opened_chosen_decided(d.path()));
    for (written, current) in [(4, 0), (0, 4)] {
        let m = h.state_mut();
        assert_eq!(m.export_files(), 4);
        m.open_review();
        m.export_from_review();
        assert_eq!(m.export.progress(), (0, 4));
        idle(m);
        assert_eq!(m.export.progress(), (4, 4));
        let line = format!("{written} written, {current} up to date, 0 failed");
        assert!(m
            .messages
            .entries()
            .any(|e| e.text == format!("Export finished: {line}")));
        settle(&mut h);
        assert!(h.query_by_label(&line).is_some(), "{line}");
    }
}

#[test]
fn review_splits_will_be_written_from_up_to_date() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    export_and_compare(&mut m, d.path());
    support::colour(&d.path().join("textures/g/a.png"), 160);
    m.check_sources();
    m.reload_changed();
    idle(&mut m);
    use engine::config::Subtype;
    use sdc_gui::model::Answer;
    let normal = "g/a_n.png";
    m.decide(normal, Answer::Subtype(Subtype::Msn));
    idle(&mut m);
    m.clear_field("g/u_n.png", "subtype");
    idle(&mut m);
    m.decide(normal, Answer::Subtype(Subtype::Normal));
    idle(&mut m);
    let sets = m.sets_doc.as_mut().unwrap();
    assert!(!sets.auto_normal().contains_key(normal));
    assert!(sdc_gui::sets_doc::SetsDoc::open(m.root.as_deref().unwrap())
        .auto_normal()
        .contains_key(normal));
    sets.set_dirty(false);
    let sets_path = sets.path().to_owned();
    m.set_field("g/a.png", "subtype", serde_json::json!("normal"));
    idle(&mut m);
    m.undo();
    idle(&mut m);
    let files = m.project.as_ref().unwrap();
    assert!(files.user_dirty());
    let files_path = files.path().to_owned();
    // A save that rewrites identical bytes also removes this document's recovery file.
    std::fs::write(sdc_gui::crash_log::recovery_path(&files_path), "restore").unwrap();
    m.open_review();
    idle(&mut m);
    let writes = m.review_writes();
    assert_eq!(writes.outputs.len(), 1);
    assert!(writes.outputs[0].path.ends_with("g/a.dds") && writes.outputs[0].marked);
    assert_eq!(writes.current.len(), 3);
    // With the same source change and an empty destination, the replacement is definite.
    std::fs::remove_file(&writes.outputs[0].path).unwrap();
    m.close_review();
    m.open_review();
    idle(&mut m);
    assert_eq!(m.written_text(), "Will be written (1) · Up to date (3)");
    let writes = m.review_writes();
    assert_eq!(writes.outputs.len(), 1);
    assert!(writes.outputs[0].path.ends_with("g/a.dds"));
    assert!(!writes.outputs[0].marked);
    let root = m.output_root().join("textures/g");
    let expected = ["a_n.dds", "u.dds", "u_n.dds"].map(|name| root.join(name));
    assert_eq!(
        writes
            .current
            .iter()
            .collect::<std::collections::BTreeSet<_>>(),
        expected.iter().collect()
    );
    for path in [&files_path, &sets_path] {
        assert!(writes
            .also_updated
            .iter()
            .any(|w| &w.path == path && !w.marked));
    }
    export_and_compare(&mut m, d.path());
}

#[test]
fn the_up_to_date_count_never_outlives_its_plan() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    export_and_compare(&mut m, d.path());
    m.open_review();
    idle(&mut m);
    m.replan();
    assert_eq!(m.written_text(), "4 files · checking which changed…");
    idle(&mut m);
    assert_eq!(m.written_text(), "Will be written (0) · Up to date (4)");
    assert!(m.review_gate().is_ok());
}

fn held_currency(m: &mut AppModel) -> sdc_gui::worker::WorkerMsg {
    let until = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        m.poll();
        if let Some(reply) = m.take_held().pop() {
            return reply;
        }
        assert!(std::time::Instant::now() < until, "no currency reply");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_stale_preview_result_is_dropped() {
    use sdc_gui::model::StaleKind;
    use sdc_gui::worker::WorkerMsg;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    export_and_compare(&mut m, d.path());
    for change in ["plan", "re-encode", "close"] {
        m.set_reencode_all(false);
        m.hold_plan_results = true;
        m.open_review();
        let held = held_currency(&mut m);
        let WorkerMsg::Currency {
            generation,
            request,
            ..
        } = &held
        else {
            panic!("expected currency")
        };
        let error = WorkerMsg::Currency {
            generation: *generation,
            request: *request,
            result: Err("stale".into()),
        };
        m.hold_plan_results = false;
        match change {
            "plan" => m.replan(),
            "re-encode" => m.set_reencode_all(true),
            _ => {
                m.close_review();
                m.open_review();
            }
        }
        assert_eq!(m.generation() == *generation, change != "plan");
        let drops = m.stale_replies_dropped(StaleKind::Currency);
        m.handle_held(error);
        assert_eq!(m.written_text(), "4 files · checking which changed…");
        idle(&mut m);
        m.handle_held(held);
        assert_eq!(m.stale_replies_dropped(StaleKind::Currency), drops + 2);
        let expected = if change == "re-encode" {
            (4, 0)
        } else {
            (0, 4)
        };
        m.set_reencode_all(change == "re-encode");
        assert_eq!(
            m.written_text(),
            format!("Will be written (0) · Up to date ({})", expected.1)
        );
        let writes = m.review_writes();
        assert_eq!(writes.outputs.len(), expected.0);
        assert!(writes.outputs.iter().all(|o| o.marked));
        assert!(m.review_gate().is_ok());
        m.close_review();
    }
    m.open_review();
    m.set_reencode_all(true);
    assert!(m.written_text().contains("checking which changed"));
    m.export_from_review();
    assert!(m.export.running.is_some() && !m.reencode_all);
    idle(&mut m);
    assert_eq!(m.export.result.as_ref().unwrap().primary.written, 4);
}

#[test]
fn review_replans_for_a_new_ready_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    support::add_ready_set(d.path(), "b");
    m.open_review();
    idle(&mut m);
    assert_eq!(m.export_files(), 6);
    let writes = m.review_writes();
    for name in ["b.dds", "b_n.dds"] {
        assert!(writes.outputs.iter().any(|o| o.path.ends_with(name)));
    }
    export_and_compare(&mut m, d.path());
    assert_eq!(m.export.progress(), (6, 6));
}

#[test]
fn review_replans_only_once_for_two_differing_previews() {
    check_review_replans_once(false);
}

#[test]
fn review_replans_only_once_when_scan_starts_while_waiting_for_mesh_votes() {
    check_review_replans_once(true);
}

fn check_review_replans_once(start_scan: bool) {
    use sdc_gui::worker::WorkerMsg;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let dbdir = tempfile::tempdir().unwrap();
    if start_scan {
        let db = dbdir.path().join("scan.db");
        drop(modscan::store::Store::open(&db).unwrap());
        m.scan_db = Some(db);
    }
    m.hold_plan_results = true;
    support::add_ready_set(d.path(), "b");
    m.open_review();

    for settled in [4, 6] {
        let reply = held_currency(&mut m);
        assert_eq!(m.export_files(), settled);
        let WorkerMsg::Currency { result, .. } = &reply else {
            panic!("expected a preview of the {settled}-output plan")
        };
        let keys = result.as_ref().unwrap();
        assert_eq!(keys.len(), settled + 2, "preview {keys:?}");
        let generation = m.generation();
        m.handle_held(reply);
        if settled == 4 {
            assert_eq!(m.generation(), generation + 1, "first mismatch replans");
            assert!(m.full_plan_pending());
            if start_scan {
                for msg in held_plan(&mut m) {
                    m.handle_held(msg);
                }
                assert!(m.votes_pending(), "the reconciliation plan awaits votes");
                assert_eq!(m.generation(), generation + 1);
                m.scan_started();
                assert!(!m.votes_pending(), "scan delivered the retained votes");
                assert_eq!(m.generation(), generation + 2, "follow-up plan started");
                assert!(m.full_plan_pending());
            }
            while m.full_plan_pending() {
                for msg in held_plan(&mut m) {
                    m.handle_held(msg);
                }
            }
            // Finish recording b's automatic decisions before the next preview sees c.
            support::add_ready_set(d.path(), "c");
        } else {
            assert_eq!(m.generation(), generation, "second mismatch replanned");
        }
    }

    assert!(m.review.is_some());
    assert!(!m.full_plan_pending());
    assert!(!m.written_text().contains("checking which changed"));
    assert!(m.review_gate().is_ok());
    let writes = m.review_writes();
    let root = m.output_root().join("textures/g");
    let expected = ["a.dds", "a_n.dds", "b.dds", "b_n.dds", "u.dds", "u_n.dds"];
    let paths = expected.map(|name| root.join(name));
    assert!(writes.outputs.iter().map(|w| &w.path).eq(paths.iter()));

    m.hold_plan_results = false;
    let generation = m.generation();
    m.set_reencode_all(true);
    assert!(m.written_text().contains("checking which changed"));
    idle(&mut m);
    assert_eq!(
        m.generation(),
        generation,
        "Re-encode granted another replan"
    );
    assert_eq!(m.export_files(), 6);
    m.close_review();
    support::add_ready_set(d.path(), "e");
    m.open_review();
    idle(&mut m);
    assert_eq!(m.export_files(), 10);
}

#[test]
fn reload_from_disk_preserves_the_open_reviews_spent_reconcile() {
    use sdc_gui::model::DocKind;
    use sdc_gui::worker::WorkerMsg;

    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    support::add_ready_set(d.path(), "b");
    m.open_review();
    idle(&mut m);
    assert_eq!(m.export_files(), 6);
    m.save().unwrap();

    m.set_field("g/a_n.png", "subtype", serde_json::json!("msn"));
    idle(&mut m);
    support::touch_sdc_toml_by_hand(m.root.as_deref().unwrap(), "\n# external edit\n");
    assert!(m.save().is_err());
    assert_eq!(m.dialog, Some(Dialog::ExternalChange(DocKind::SdcToml)));
    m.hold_plan_results = true;
    m.resolve_external_change(DocKind::SdcToml, false);
    let plan = held_plan(&mut m);
    support::add_ready_set(d.path(), "c");
    for msg in plan {
        m.handle_held(msg);
    }
    let reply = held_currency(&mut m);
    assert!(matches!(&reply, WorkerMsg::Currency { result: Ok(keys), .. } if keys.len() == 8));
    assert_eq!(m.export_files(), 6);
    let generation = m.generation();
    m.handle_held(reply);
    assert_eq!(m.generation(), generation, "post-Reload mismatch replanned");
    assert!(m.review.is_some() && m.review_gate().is_ok());
    assert!(!m.full_plan_pending());
}

#[test]
fn changing_group_exclusion_refreshes_the_open_review_request() {
    use sdc_gui::model::set_groups::GroupTarget;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.move_to_group(&["g/a".into()], GroupTarget::New("Armor".into()))
        .unwrap();
    export_and_compare(&mut m, d.path());
    m.open_review();
    idle(&mut m);
    assert_eq!(m.review_writes().current.len(), 4);
    m.set_group_excluded("Armor", true).unwrap();
    assert_eq!(m.written_text(), "2 files · checking which changed…");
    assert!(m.review_gate().is_ok());
    idle(&mut m);
    m.set_group_excluded("Armor", true).unwrap();
    assert_eq!(m.written_text(), "Will be written (0) · Up to date (2)");
    let root = m.output_root().join("textures/g");
    assert_eq!(
        m.review_writes().current,
        [root.join("u.dds"), root.join("u_n.dds")]
    );
}

#[test]
fn export_during_preview_counts_new_writer_keys() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    support::add_ready_set(d.path(), "b");
    m.open_review();
    assert!(m.written_text().contains("checking which changed"));
    assert!(m.review_gate().is_ok());
    m.export_from_review();
    idle(&mut m);
    assert_eq!(support::dds_under(&m.output_root()), 6);
    assert_eq!(m.export.progress(), (6, 6));
    assert_eq!(
        m.export
            .rows
            .iter()
            .filter(|r| r.state.label() == "converted")
            .count(),
        6
    );
    assert!(m
        .messages
        .entries()
        .any(|e| e.text == "Export finished: 6 written, 0 up to date, 0 failed"));
}

#[test]
fn a_corrupt_source_counts_as_failed_alongside_ready_outputs() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    std::fs::write(d.path().join("textures/g/broken.png"), b"broken image").unwrap();
    m.open_review();
    m.export_from_review();
    idle(&mut m);
    assert!(m
        .export
        .rows
        .iter()
        .any(|r| r.path.ends_with("broken.png") && r.state.is_failed()));
    assert_eq!(m.export.progress(), (5, 5));
    let line = "4 written, 0 up to date, 1 failed";
    assert!(m
        .messages
        .entries()
        .any(|e| e.text == format!("Export finished: {line}")));
    let mut h = harness(m);
    settle(&mut h);
    assert!(h.query_by_label(line).is_some());
}

#[test]
fn an_unreadable_manifest_reports_the_files_result_in_messages() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    std::fs::create_dir_all(m.export_root().join("textures/_sdc/manifest.jsonl")).unwrap();
    m.open_review();
    m.export_from_review();
    idle(&mut m);
    let error = m.export.error.as_ref().unwrap();
    assert!(m
        .messages
        .entries()
        .any(|e| e.text == format!("Export failed: {error}")));
    let line = "0 written, 0 up to date, 4 failed";
    assert!(m.messages.entries().any(|e| e.text.ends_with(line)));
    let mut h = harness(m);
    settle(&mut h);
    assert!(h.query_by_label(line).is_some());
}

#[test]
fn a_file_in_place_of_the_output_directory_keeps_writes_conditional() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let parent = m.output_root().join("textures/g");
    std::fs::create_dir_all(parent.parent().unwrap()).unwrap();
    std::fs::write(&parent, b"occupied").unwrap();
    m.open_review();
    idle(&mut m);
    let writes = m.review_writes();
    assert_eq!(writes.outputs.len(), 4);
    assert!(writes.outputs.iter().all(|o| o.marked));
    m.export_from_review();
    idle(&mut m);
    assert_eq!(m.export.result.as_ref().unwrap().primary.failed, 4);
    assert_eq!(std::fs::read(parent).unwrap(), b"occupied");
}

#[test]
fn a_dangling_destination_is_conditional_beside_three_definite_writes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let parent = m.output_root().join("textures").join("g");
    std::fs::create_dir_all(&parent).unwrap();
    let link = parent.join("a.dds");
    dir_link(&link, &d.path().join("missing"));
    assert!(!link.exists() && link.symlink_metadata().is_ok());
    m.open_review();
    idle(&mut m);
    let writes = m.review_writes();
    assert_eq!(writes.outputs.len(), 4);
    for output in &writes.outputs {
        assert_eq!(
            output.marked,
            output.path == link,
            "{}",
            output.path.display()
        );
    }
    m.export_from_review();
    idle(&mut m);
    assert_eq!(m.export.result.as_ref().unwrap().primary.written, 3);
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink() || link.read_link().is_ok());
}

#[test]
fn progress_counts_each_new_terminal_key_once() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    export_and_compare(&mut m, d.path());
    let done = m
        .export
        .events
        .iter()
        .find(|e| matches!(e.kind, engine::events::EventKind::FileDone { .. }))
        .unwrap()
        .clone();
    let mut live = sdc_gui::model::export::ExportModel::default();
    live.on_event(done.clone());
    assert_eq!(live.progress(), (1, 1));
    live.on_event(done);
    assert_eq!(live.progress(), (1, 1));
}

#[test]
fn the_review_footer_shows_the_first_dds_with_space_available() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.open_review();
    idle(&mut m);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1200.0, 900.0))
        .build_ui_state(sdc_gui::ui::review::tab, m);
    h.run();
    let controls_top = h
        .get_by_label("Export without review next time")
        .rect()
        .top();
    for output in &h.state().review_writes().outputs {
        let path = output.path.display().to_string();
        let row = h
            .get_by_label(&format!("{path} (depends on this export)"))
            .rect();
        assert!(
            row.bottom() <= controls_top,
            "{path} is clipped at {controls_top}, {row:?}"
        );
    }
}

#[test]
fn timestamps_catch_identical_rewrites_of_current_and_kept_files() {
    use std::fs::{File, FileTimes};
    let d = tempfile::tempdir().unwrap();
    let mut w = sdc_gui::model::review::ReviewWrites::default();
    w.current.push(d.path().join("current.dds"));
    w.kept.push(d.path().join("kept.dds"));
    for path in w.current.iter().chain(&w.kept) {
        std::fs::write(path, b"same bytes").unwrap();
    }
    let before = support::write_snapshot(d.path(), &w);
    for path in w.current.iter().chain(&w.kept) {
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(
                FileTimes::new()
                    .set_modified(std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(100)),
            )
            .unwrap();
    }
    let after = support::write_snapshot(d.path(), &w);
    for path in w.current.iter().chain(&w.kept) {
        assert_ne!(before[path], after[path]);
        assert!(support::listed(&w).iter().all(|entry| entry.path != *path));
    }
}

#[test]
fn the_status_line_counts_files_not_plan_items() {
    let d = tempfile::tempdir().unwrap();
    let m = support::pbr_opened_chosen_decided(d.path());
    assert!(
        m.items
            .iter()
            .filter_map(|i| i.job.as_ref())
            .any(|j| j.bake.is_some()),
        "roughness has a normal dependency"
    );
    assert_eq!(
        m.status_line,
        "6 files → 5 textures (1 packed or generated) · 3 sets"
    );
}

#[test]
fn the_status_line_omits_packed_when_there_is_none() {
    let d = tempfile::tempdir().unwrap();
    let m = support::opened_chosen_decided(d.path());
    assert_eq!(m.status_line, "4 files → 4 textures · 2 sets");
}

#[test]
fn a_failed_currency_check_leaves_the_conditional_list_and_export_available() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let manifest = m.export_root().join("textures/_sdc/manifest.jsonl");
    std::fs::create_dir_all(&manifest).unwrap();
    m.hold_plan_results = true;
    m.open_review();
    assert_eq!(m.written_text(), "4 files · checking which changed…");
    let until = std::time::Instant::now() + Duration::from_secs(5);
    let reply = loop {
        m.poll();
        if let Some(reply) = m.take_held().pop() {
            break reply;
        }
        assert!(std::time::Instant::now() < until, "no preview failure");
        std::thread::yield_now();
    };
    assert!(
        matches!(&reply, sdc_gui::worker::WorkerMsg::Currency { generation, result: Err(error), .. }
        if *generation == m.generation() && error.contains("manifest"))
    );
    m.hold_plan_results = false;
    m.handle_held(reply);
    idle(&mut m);
    assert_eq!(m.written_text(), "4 DDS files");
    assert!(m.export.rows.is_empty());
    let conditional = m.review_writes();
    assert_eq!(conditional.outputs.len(), 4);
    assert!(conditional.outputs.iter().all(|o| o.marked));
    assert!(m.review_gate().is_ok());
    std::fs::remove_dir(&manifest).unwrap();
    m.export_from_review();
    idle(&mut m);
    assert_eq!(m.export.result.as_ref().unwrap().primary.written, 4);
}

#[test]
fn a_skipped_dds_input_has_a_files_row_outside_the_progress_total() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    std::fs::write(d.path().join("textures/g/kept.dds"), b"already a DDS").unwrap();
    m.replan();
    idle(&mut m);
    m.open_review();
    m.export_from_review();
    assert_eq!(m.export.progress(), (0, 4));
    idle(&mut m);
    assert_eq!(m.export.progress(), (4, 4));
    assert!(m.export.rows.iter().any(|r| r.path.ends_with("kept.dds")));
    let p = &m.export.result.as_ref().unwrap().primary;
    assert_eq!((p.written, p.failed, p.skipped_sources), (4, 0, 1));
    assert!(m
        .messages
        .entries()
        .any(|e| e.text == "Export finished: 4 written, 0 up to date, 0 failed, 1 skipped source"));
}

#[test]
fn cancelling_before_the_first_discovery_accounts_for_every_primary_key() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.open_review();
    m.before_export_worker = Some(|m| {
        assert!(m.export.events.is_empty());
        assert_eq!(m.export.progress(), (0, 4));
        m.export.running.as_ref().unwrap().cancel();
    });
    m.export_from_review();
    idle(&mut m);
    let p = &m.export.result.as_ref().unwrap().primary;
    assert_eq!(p.cancelled, 4);
    assert_eq!(p.cancelled + p.written, 4);
    assert_eq!(m.export.progress(), (4, 4));
}

#[test]
fn six_faces_one_cube_and_a_source_dds_have_separate_units() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    for face in ["px", "nx", "py", "ny", "pz", "nz"] {
        support::colour(&root.join(format!("a/chrome_e_{face}.png")), 90);
    }
    std::fs::write(root.join("a/old.dds"), b"already a DDS").unwrap();
    let m = support::opened_as_the_app(root, d.path().join("export"));
    assert!(
        m.status_line.starts_with("7 files → 1 texture ·"),
        "{}",
        m.status_line
    );
}

#[test]
fn a_foreign_dds_is_protected_in_review_and_in_the_result() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let path = m
        .item("g/a.png")
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .output
        .clone();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"foreign").unwrap();
    m.open_review();
    idle(&mut m);
    assert_eq!(
        m.written_text(),
        "Will be written (3) · Up to date (0) · Protected (1)"
    );
    export_and_compare(&mut m, d.path());
    assert_eq!(std::fs::read(path).unwrap(), b"foreign");
    assert!(m
        .messages
        .entries()
        .any(|e| e.text == "Export finished: 3 written, 0 up to date, 0 failed, 1 protected"));
}

#[test]
fn an_unused_helper_and_shared_pack_input_count_once_each() {
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    for name in [
        "a/rock",
        "a/rock_roughness",
        "pbr/c/slab",
        "a/orphan_opacity",
    ] {
        support::colour(&root.join(format!("{name}.png")), 90);
    }
    let mut m = support::opened_as_the_app(root, d.path().join("export"));
    assert_eq!(
        m.drop_choice("a/rock_roughness.png", "pbr/c/slab", DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    assert!(
        m.status_line
            .starts_with("4 files → 4 textures (2 packed or generated)"),
        "{}",
        m.status_line
    );
}

#[test]
fn validation_failure_is_one_failed_output() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    export_and_compare(&mut m, d.path());
    let mut event = m
        .export
        .events
        .iter()
        .find(|e| matches!(e.kind, engine::events::EventKind::FileDone { .. }))
        .unwrap()
        .clone();
    if let engine::events::EventKind::FileDone { validation, .. } = &mut event.kind {
        validation.passed = false;
    }
    m.export.on_event(event);
    let mut summary = m.export.summary.clone().unwrap();
    summary.validation_failed = 1;
    m.end_export_with(Ok(summary));
    assert!(m
        .messages
        .entries()
        .any(|e| e.text == "Export finished: 3 written, 0 up to date, 1 failed"));
}

#[test]
fn write_project_files_now_puts_the_automatic_types_on_disk() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(support::auto_project(d.path()));
    settle(&mut h);
    assert!(
        !d.path().join("sdc.toml").exists(),
        "nothing written at open"
    );
    h.get_by_role_and_label(Role::Button, "Write project files now")
        .click();
    settle(&mut h);
    let toml = std::fs::read_to_string(d.path().join("sdc.toml")).unwrap();
    assert!(
        toml.contains("teeth_n.png") && toml.contains("subtype"),
        "{toml}"
    );
    assert!(!sdc_gui::ui::header::window_title(h.state()).ends_with(" ●"));
    assert!(!h.state().history.can_undo());
    assert_eq!(
        h.query_all_by_label("Write project files now").count(),
        0,
        "the line is gone"
    );
    assert_eq!(
        h.query_all_by_label_contains("New project: nothing saved yet.")
            .count(),
        0
    );
}

#[test]
fn no_line_when_there_is_nothing_to_write() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("textures")).unwrap();
    image::RgbImage::new(4, 4)
        .save(d.path().join("textures/mystery_unknown.png"))
        .unwrap(); // no rule types it
    let mut m = AppModel::new(GuiSettings::default(), None, support::session());
    m.open_folder(d.path().to_path_buf());
    assert!(m.wait_idle(Duration::from_secs(60)));
    let mut h = harness(m);
    settle(&mut h);
    assert_eq!(
        h.query_all_by_label_contains("New project: nothing saved yet.")
            .count(),
        0
    );
}

#[test]
fn the_new_project_line_and_the_next_step_both_read_at_1100_px() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness_sized(support::auto_project(d.path()), egui::vec2(1100.0, 800.0));
    settle(&mut h);
    let line = h
        .get_by_label_contains("New project: nothing saved yet.")
        .rect();
    let next = h
        .get_by_label_contains("1 texture needs a type: start with Undecided.")
        .rect();
    assert!(!line.intersects(next), "{line:?} {next:?}");
    for r in [line, next] {
        assert!(r.max.x <= 1100.0 && r.min.x >= 0.0, "{r:?} is cut");
    }
}

#[test]
fn opening_a_folder_leaves_save_undo_and_the_dot_off() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::auto_project(d.path());
    assert!(
        m.project
            .as_ref()
            .unwrap()
            .file_value("textures/teeth_n.png", "subtype")
            .is_some(),
        "the plan decided a type"
    );
    assert!(!m.unsaved());
    assert!(!m.history.can_undo());
    assert!(!sdc_gui::ui::header::window_title(&m).ends_with(" ●"));
    assert!(m.request_close(), "close does not ask");
    assert!(m.needs_write(), "the automatic types are not on disk yet");
}

#[test]
fn a_type_edit_then_undo_stays_unsaved() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::auto_project(d.path());
    support::set_type(&mut m, "other_unknown.png", "diffuse");
    assert!(m.unsaved() && m.history.can_undo());
    m.undo();
    assert!(m.unsaved() && !m.history.can_undo());
}

#[test]
fn the_next_save_or_export_writes_the_automatic_types() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::auto_project(d.path());
    support::choose_export(&mut m, d.path().join("out"));
    support::export_through(&mut m);
    assert!(m.wait_idle(Duration::from_secs(120)));
    let toml = std::fs::read_to_string(d.path().join("sdc.toml")).unwrap();
    assert!(toml.contains("subtype"), "{toml}");
    assert!(!m.needs_write());
}

#[test]
fn a_hand_edit_reloads_over_automatic_values_without_asking() {
    // Review Focus 3.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::auto_project(d.path());
    support::touch_sdc_toml_by_hand(
        d.path(),
        "[files.\"textures/other_unknown.png\"]\nsubtype = \"diffuse\"\n",
    );
    m.check_sources();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.dialog.is_none(), "{:?}", m.dialog);
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value("textures/other_unknown.png", "subtype"),
        Some(serde_json::json!("diffuse"))
    );
    assert!(!m.unsaved());
}

#[test]
fn undo_of_an_edit_made_before_the_automatic_values_keeps_them() {
    // Undo snapshots hold whole documents: an edit before the automatic values land must not undo them.
    let d = tempfile::tempdir().unwrap();
    support::write_auto_decidable(d.path());
    let mut m = AppModel::new(GuiSettings::default(), None, support::session());
    m.open_folder(d.path().to_path_buf());
    support::set_type(&mut m, "other_unknown.png", "diffuse"); // before the plan settles
    assert!(
        m.unsaved() && m.can_undo(),
        "the user step precedes the automatic record"
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.undo();
    assert!(m.unsaved() && !m.history.can_undo());
    assert!(
        m.project
            .as_ref()
            .unwrap()
            .file_value("textures/teeth_n.png", "subtype")
            .is_some(),
        "the automatic type is still in the document"
    );
}

#[test]
fn a_direct_save_moves_the_baseline() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::auto_project(d.path());
    support::choose_export(&mut m, d.path().join("out")); // set_export_folder saves sdc-project.toml at once
    m.set_export_without_review(true);
    assert!(!m.unsaved(), "a direct save never lights the dot");
    support::set_type(&mut m, "other_unknown.png", "diffuse");
    m.save().unwrap();
    m.undo();
    assert!(m.unsaved(), "Undo differs from the newly saved baseline");
    m.redo();
    assert!(m.unsaved(), "Redo of the type still prompts");
}

#[test]
fn an_undone_type_edit_across_a_later_automatic_record_still_prompts() {
    // The user step comes first, the automatic record second; the Undo must keep the automatic value, with no
    // wait for a plan: `unsaved()` is read straight after `undo()`.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::auto_project(d.path());
    support::set_type(&mut m, "other_unknown.png", "diffuse");
    assert!(m.wait_idle(Duration::from_secs(60)));
    support::write_auto_png(d.path(), "teeth2_n.png"); // a second file the plan types automatically (High)
    m.check_sources();
    m.replan(); // A directory rescan discovers the new file.
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m
        .project
        .as_ref()
        .unwrap()
        .file_value("textures/teeth2_n.png", "subtype")
        .is_some());
    m.undo();
    assert!(m.unsaved(), "type edit marked at once, no plan awaited");
    assert!(
        m.project
            .as_ref()
            .unwrap()
            .file_value("textures/teeth2_n.png", "subtype")
            .is_some(),
        "Undo kept it"
    );
    m.redo();
    assert!(m.unsaved());
    assert!(
        m.project
            .as_ref()
            .unwrap()
            .file_value("textures/teeth2_n.png", "subtype")
            .is_some(),
        "Redo kept it"
    );
}

#[test]
fn choosing_an_export_folder_keeps_the_new_project_line() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::auto_project(d.path());
    support::choose_export(&mut m, d.path().join("out"));
    let mut h = harness(m);
    settle(&mut h);
    h.get_by_label_contains("New project: nothing saved yet.");
    assert!(!d.path().join("sdc.toml").exists());
}

#[test]
fn a_real_save_clears_the_new_project_line() {
    let d = tempfile::tempdir().unwrap();
    support::colour(&d.path().join("textures/mystery_unknown.png"), 90);
    let mut m = AppModel::new(GuiSettings::default(), None, support::session());
    m.open_folder(d.path().to_path_buf());
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(!m.needs_write());
    let set = m.drop_new_set("textures/mystery_unknown.png").unwrap();
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.rename_set(&set, "first").unwrap();
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.save().unwrap();
    assert!(d.path().join("sdc-project.toml").is_file());
    assert!(!d.path().join("sdc.toml").exists());
    m.rename_set("first", "second").unwrap();
    assert!(m.needs_write());
    let mut h = harness(m);
    settle(&mut h);
    assert_eq!(
        h.query_all_by_label_contains("New project: nothing saved yet.")
            .count(),
        0
    );
    assert!(!h.state().never_saved);
}

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    harness_sized(model, egui::vec2(1400.0, 900.0))
}

fn harness_sized(model: AppModel, size: egui::Vec2) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_step_dt(0.25)
        .with_size(size)
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

#[test]
fn scoped_frames_keep_their_views_across_a_whole_disk_refresh() {
    use sdc_gui::model::{bottom::export_files_builds, review::whole_disk_reads, sets};
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    // Vanilla's borrower-copy preparation asks which sets are in the whole run at each disk refresh.
    m.set_target(engine::config::Target::Vanilla);
    assert!(m.wait_idle(Duration::from_secs(60)));
    scoped_review_with_the_opt_out(&mut m, "g/u");
    let frame = || {
        assert!(m.header_answer().now);
        assert_eq!(m.export_files(), 2);
        sets::views(&m);
    };
    frame();
    let builds = || (export_files_builds(&m), sets::view_builds(&m));
    let before = builds();
    let reads = whole_disk_reads(&m);
    m.expire_review_json();
    for _ in 0..4 {
        frame();
    }
    assert!(whole_disk_reads(&m) > reads);
    assert_eq!(
        builds(),
        before,
        "stable scoped frames rebuilt the file count or set views"
    );
}

#[test]
fn scoped_review_header_hover_counts_the_whole_project() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let expected = sdc_gui::ui::header::export_hover(&m);
    let key = key_of(&m, "g/u");
    m.export_sets([key].into(), "Export set".into());
    assert_eq!(m.export_files(), 2);
    assert_eq!(sdc_gui::ui::header::export_hover(&m), expected);
}

#[test]
fn project_history_over_meshes_names_both_steps_in_the_status_line() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::auto_project(d.path());
    support::set_type(&mut m, "other_unknown.png", "diffuse");
    m.centre_over_meshes = true;
    let label = m.undo_label().unwrap().to_string();
    m.undo();
    let undone = m.status_line.clone();
    m.redo();
    assert!(
        undone.starts_with(&format!("Undone: {label}"))
            && m.status_line.starts_with(&format!("Redone: {label}")),
        "{undone} / {}",
        m.status_line
    );
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

#[test]
fn the_opening_step_counts_the_other_sets_problem_during_scoped_review() {
    let d = tempfile::tempdir().unwrap();
    support::add_ready_set(d.path(), "clean");
    let root = d.path().join("textures");
    let arm = root.join("pbr/arm");
    support::colour(&arm.join("steel.png"), 200);
    support::flat(&arm.join("steel_n.png"));
    support::colour(&arm.join("steel_rmaos.png"), 30);
    let mut doc = sdc_gui::sets_doc::SetsDoc::open(&root);
    doc.set_value("pbr/arm/steel", "parallax", Some(serde_json::json!(true)));
    doc.set_export_folder(&d.path().join("export"));
    doc.save(false).unwrap();

    let mut m = AppModel::new(GuiSettings::default(), None, support::session());
    m.hold_plan_results = true;
    m.open_folder(root);
    while !m.needs_write() {
        for msg in held_plan(&mut m) {
            m.handle_held(msg);
        }
    }
    let follow_up = held_plan(&mut m);
    assert!(
        m.status_note.is_none(),
        "the recorder's plan has yet to land"
    );
    assert_eq!(m.undecided(), 0);
    assert_eq!(flagged(&m).ready_rows, 1);
    m.export_sets([key_of(&m, "g/clean")].into(), "Export set g/clean".into());
    assert!(m.review.is_some() && m.export_scope().is_some());
    assert!(!m.has_problem_rows(), "the set in Review is clean");
    let hover = sdc_gui::ui::header::export_hover(&m);
    assert!(hover.ends_with(", 1 problem"), "{hover}");
    land_plan(&mut m, follow_up);
    assert!(m.review.is_some() && m.export_scope().is_some());
    let line = format!(
        "1 problem to look at: open Problems. {}",
        m.status_line_counts()
    );
    let h = harness(m);
    assert_eq!(
        h.query_all_by_label(&line).count(),
        1,
        "{}",
        h.state().status_line
    );
}

#[test]
fn changing_export_without_review_retires_the_opening_step() {
    let d = tempfile::tempdir().unwrap();
    support::opened_chosen_decided(d.path()).save().unwrap();
    let root = d.path().join("textures");
    let mut m = AppModel::new(GuiSettings::default(), None, support::session());
    m.open_folder(root.clone());
    let mut h = harness(m);
    settle(&mut h);
    assert!(!h.state().needs_write());
    let ready = "Ready: Review 4 files… shows what will be written.";
    let counts = h.state().status_line_counts();
    h.get_by_label(&format!("{ready} {counts}"));

    h.state_mut().open_settings_tab();
    h.run();
    h.get_by_role_and_label(Role::CheckBox, "Export without review")
        .click();
    h.run();
    assert!(sdc_gui::sets_doc::SetsDoc::open(&root).export_without_review());
    assert_eq!(h.state().header_export_text(), "Export 4 files now");
    assert_eq!(
        h.query_all_by_label_contains(ready).count(),
        0,
        "{}",
        h.state().status_line
    );
    h.get_by_label(&counts);
    assert!(h.state().status_note.is_none());
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
        Between::ReencodeAllTicked => m.set_reencode_all(true),
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
    try_dir_link(link, target).unwrap();
}

fn try_dir_link(link: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let ok = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()?;
        if !ok.status.success() {
            return Err(std::io::Error::other(format!("mklink /J failed: {ok:?}")));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    std::os::unix::fs::symlink(target, link)
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

#[test]
fn a_scoped_overlap_refusal_refreshes_the_whole_project_header() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    let out = d.path().join("safe");
    let target = d.path().join("target mod");
    let link = d.path().join("export link");
    std::fs::create_dir(&out).unwrap();
    std::fs::create_dir(&target).unwrap();
    dir_link(&link, &out);
    support::choose_export(&mut m, link.clone());
    m.set_target_mod(Some(target.clone())).unwrap();
    assert!(m.wait_idle(Duration::from_secs(60)));
    scoped_review_with_the_opt_out(&mut m, "g/u");
    assert_eq!(m.header_export_text(), "Export 4 files now");

    std::fs::rename(&link, d.path().join("old link")).unwrap();
    dir_link(&link, &target);
    let a = key_of(&m, "g/a");
    m.export_sets([a.clone()].into(), "Export set g/a".into());
    assert!(m.review.is_some() && m.export.running.is_none());
    assert!(m.export_scope().unwrap().keys.contains(&a));
    assert!(m.review_gate().is_err());
    let label = m.header_export_text();
    let hover = sdc_gui::ui::header::export_hover(&m);
    assert_eq!(
        (label.as_str(), hover.as_str()),
        ("Review 4 files…", "4 files, 1 problem")
    );
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
    let mut h = harness(m);
    let label = h.state().header_export_text();
    h.get_by_role_and_label(Role::Button, &label).hover();
    h.run_steps(6);
    assert!(
        h.query_by_label_contains("Review first: Fix 1 blocker first")
            .is_some(),
        "the hover must explain the whole project's blocker"
    );
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

/// One export run seen from outside: what Review listed when its Export was pressed, and what changed on disk.
struct Seen {
    listed: std::collections::BTreeSet<std::path::PathBuf>,
    changed: std::collections::BTreeSet<std::path::PathBuf>,
    w: sdc_gui::model::review::ReviewWrites,
    removals: support::RemovalCredits,
}

/// The acceptance helper: open Review if it is not open, take its list, press its Export and hold the files that
/// changed under `dir` (the union of both snapshots, so a removed or renamed file is a change) to the listing rules
/// (`support::listing_violations`, rulings 3c-50 to 3c-52): every changed file is listed or credited to one listed
/// entry, and every listed entry that did not change is marked run-dependent.
fn export_and_compare(m: &mut AppModel, dir: &std::path::Path) -> Seen {
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    if m.review.is_none() {
        m.header_export_clicked();
    }
    assert!(m.review.is_some(), "the header opens Review");
    assert!(m.review_gate().is_ok(), "{:?}", m.review_gate());
    idle(m);
    let w = (*m.review_writes()).clone();
    let pending = support::PendingRemovals::before_export(m);
    let listed: BTreeSet<PathBuf> = support::listed(&w).into_iter().map(|e| e.path).collect();
    let before = support::write_snapshot(dir, &w);
    m.export_from_review();
    assert!(m.wait_idle(Duration::from_secs(120)));
    assert!(
        pending.started(m) && m.export.summary.is_some(),
        "{:?}",
        m.export.notes
    );
    let removals = pending.after_export(m);
    let after = support::write_snapshot(dir, &w);
    // No scan store in these projects: no autosave can be credited.
    let support::ListingComparison {
        changed,
        violations,
    } = support::compare_listing(&w, &before, &after, None, &removals);
    assert!(violations.is_empty(), "{violations:#?}");
    Seen {
        listed,
        changed,
        w,
        removals,
    }
}

/// The first export of the acceptance test, checked under the listing rules (controller ruling).
fn assert_every_changed_file_is_listed(mut m: AppModel, dir: &std::path::Path) {
    let out = m.output_root();
    let root = m.root.clone().unwrap();
    let Seen {
        listed, changed, w, ..
    } = export_and_compare(&mut m, dir);
    // A file under a listed Also folder is that folder's entry.
    let folders = w.also_folders();
    let changed: std::collections::BTreeSet<std::path::PathBuf> = changed
        .into_iter()
        .map(|p| {
            folders
                .iter()
                .find(|f| p.starts_with(f))
                .cloned()
                .unwrap_or(p)
        })
        .collect();
    // A first export that really writes files: every listed file is new, so the two sets are equal.
    assert!(!changed.is_empty());
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

/// Capture Review before the click, then inspect the synchronous save before any export finisher runs.
fn export_and_get_project_list(m: &mut AppModel) -> Vec<std::path::PathBuf> {
    m.header_export_clicked();
    assert!(m.review.is_some() && m.review_gate().is_ok());
    assert!(
        m.review_json().is_empty(),
        "this run has no JSON hash to record"
    );
    let list = m.review_writes();
    m.export_from_review();
    assert!(m.export.running.is_some(), "{:?}", m.dialog);
    list.also_updated.iter().map(|e| e.path.clone()).collect()
}

#[test]
fn review_lists_save_first_rewrite_after_undoing_a_saved_type_edit() {
    use std::time::SystemTime;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.save().unwrap();
    let file = m.project.as_ref().unwrap().path().to_owned();
    let saved = std::fs::read(&file).unwrap();
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(86_400);
    std::fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(old)
        .unwrap();
    m.reload_documents();
    idle(&mut m);
    let key = "g/a.png";
    assert_eq!(
        m.project.as_ref().unwrap().file_value(key, "subtype"),
        Some(serde_json::json!("diffuse"))
    );
    m.set_field(key, "subtype", serde_json::json!("normal"));
    idle(&mut m);
    assert_eq!(
        m.project.as_ref().unwrap().file_value(key, "subtype"),
        Some(serde_json::json!("normal"))
    );
    m.undo();
    idle(&mut m);
    let doc = m.project.as_ref().unwrap();
    assert_eq!(doc.text().as_bytes(), saved);
    assert!(!doc.is_dirty() && doc.user_dirty());
    let list = export_and_get_project_list(&mut m);
    assert_ne!(std::fs::metadata(&file).unwrap().modified().unwrap(), old);
    assert_eq!(std::fs::read(&file).unwrap(), saved);
    idle(&mut m);
    assert!(
        list.contains(&file),
        "Review omitted the rewritten sdc.toml"
    );
}

#[test]
fn review_lists_save_first_write_of_a_quiet_sidecar_difference() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_chosen_decided(d.path());
    m.save().unwrap();
    let doc = m.sets_doc.as_ref().unwrap();
    let file = doc.path().to_owned();
    let side = doc.sidecar_path();
    std::fs::create_dir_all(side.parent().unwrap()).unwrap();
    let hash_record = r#"{"last_json_sha256":"side"}"#;
    std::fs::write(&side, hash_record).unwrap();
    let before = std::fs::read_to_string(&file).unwrap();
    m.reload_documents();
    idle(&mut m);
    let doc = m.sets_doc.as_ref().unwrap();
    let expected = doc.text();
    assert!(!doc.is_dirty() && !m.unsaved());
    assert_ne!(before, expected);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    let list = export_and_get_project_list(&mut m);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), expected);
    assert!(!m
        .review_writes()
        .also_updated
        .iter()
        .any(|e| e.path == file));
    idle(&mut m);
    assert_eq!(std::fs::read_to_string(&side).unwrap(), hash_record);
    assert!(
        list.contains(&file),
        "Review omitted the reconciled sdc-project.toml"
    );
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

// ---- Task 4 fix round 1 (rulings 3c-46, 3c-47): repeat exports, each from a project exported once ----

/// `pbr_opened_chosen_decided`, exported once through Review (checked by the helper) and settled.
fn pbr_exported_once(dir: &std::path::Path) -> AppModel {
    let mut m = support::pbr_opened_chosen_decided(dir);
    export_and_compare(&mut m, dir);
    assert!(m.review.is_none());
    m
}

/// The listed entry with `hover`, if any.
fn listed_with(s: &Seen, hover: &str) -> Vec<std::path::PathBuf> {
    s.w.also_updated
        .iter()
        .filter(|e| e.hover == hover)
        .map(|e| e.path.clone())
        .collect()
}

/// A vanilla project where a new set `leaf` borrows rock01's normal (as `pbr_export.rs::vanilla_borrow`): the model,
/// the owner's source image and the copy's path.
fn vanilla_borrow(d: &std::path::Path) -> (AppModel, std::path::PathBuf, std::path::PathBuf) {
    use sdc_gui::model::sets::{set_views, DropChoice, DropOutcome, SlotFill, SlotKind};
    use serde_json::json;
    let mut m = support::pbr_ready(d);
    let idle = |m: &mut AppModel| assert!(m.wait_idle(Duration::from_secs(60)));
    let leaf = m
        .items
        .iter()
        .find(|i| i.name == "leaf.png")
        .unwrap()
        .key
        .clone();
    m.set_field(&leaf, "subtype", json!("pbr_albedo"));
    idle(&mut m);
    let id = m.drop_new_set(&leaf).unwrap();
    idle(&mut m);
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    let normal = set_views(&m)
        .iter()
        .find(|v| v.id == "landscape/rock01")
        .and_then(|v| match v.slots.get(&SlotKind::Normal) {
            Some(SlotFill::Own { item }) => Some(item.clone()),
            _ => None,
        })
        .expect("rock01 owns a normal map");
    assert!(matches!(
        m.drop_choice(&normal, &id, DropChoice::Share),
        DropOutcome::Joined
    ));
    idle(&mut m);
    m.set_field(&normal, "subtype", json!("landscape_normal"));
    idle(&mut m);
    let views = set_views(&m);
    let borrower = views.iter().find(|v| v.id == id).unwrap();
    let Some(SlotFill::Own { item: d_item }) = borrower.slots.get(&SlotKind::Diffuse) else {
        panic!("the new set owns its diffuse: {:?}", borrower.slots);
    };
    let dout = m.item(d_item).unwrap().job.as_ref().unwrap().output.clone();
    let stem = dout.file_stem().unwrap().to_string_lossy().into_owned();
    let want: std::path::PathBuf = dout
        .with_file_name(format!("{stem}_n.dds"))
        .components()
        .collect();
    let source = m.item(&normal).unwrap().input.clone();
    (m, source, want)
}

#[test]
fn undoing_a_rename_after_the_first_copy_and_cm_export_is_clean() {
    let d = tempfile::tempdir().unwrap();
    let iron = d.path().join("textures/m");
    std::fs::create_dir_all(&iron).unwrap();
    image::RgbImage::from_pixel(64, 64, image::Rgb([80, 90, 100]))
        .save(iron.join("iron.png"))
        .unwrap();
    image::RgbaImage::from_fn(64, 64, |x, _| {
        image::Rgba([80, 90, 100, (40 + x * 2) as u8])
    })
    .save(iron.join("iron_m.png"))
    .unwrap();
    let (mut m, _, copy) = vanilla_borrow(d.path());
    m.set_field(
        "m/iron_m.png",
        "subtype",
        serde_json::json!("complex_material"),
    );
    m.set_cm_export("m/iron", true);
    idle(&mut m);
    m.save().unwrap();
    let doc = m.sets_doc.as_ref().unwrap();
    assert!(doc.copy_shas().is_empty() && doc.cm_json_shas().is_empty());
    let saved = std::fs::read(doc.path()).unwrap();
    support::export_through(&mut m);
    assert!(m.export.running.is_some(), "{:?}", m.review_gate());
    m.rename_set("landscape/rock01", "landscape/renamed")
        .unwrap();
    idle(&mut m);
    let doc = m.sets_doc.as_ref().unwrap();
    assert!(copy.is_file());
    let hashes = (doc.copy_shas(), doc.cm_json_shas());
    assert!(
        !hashes.0.is_empty() && !hashes.1.is_empty(),
        "{:?}",
        m.export.notes
    );
    assert!(doc.text().contains("landscape/renamed") && m.unsaved());
    assert_eq!(std::fs::read(doc.path()).unwrap(), saved);
    m.undo();
    let doc = m.sets_doc.as_ref().unwrap();
    assert!(!doc.text().contains("landscape/renamed"));
    assert_eq!((doc.copy_shas(), doc.cm_json_shas()), hashes);
    assert!(doc.text().contains("copy_sha256") && doc.text().contains("cm_json_sha256"));
    assert!(doc.is_dirty());
    assert!(
        m.request_close(),
        "only the two automatic export hashes remain"
    );
}

/// (1) Nothing changed since the last export: everything Review lists is decided by this run (ruling 3c-50): the DDS
/// outputs (until Task 8), the PGPatcher JSON built from the run's rows, the two record files its hash goes to and the
/// manifest. Each is marked; the JSON comes out as it was.
#[test]
fn a_repeat_export_with_nothing_changed_lists_only_marked_entries() {
    use sdc_gui::model::review::{
        HOVER_GENERATED, HOVER_MANIFEST, HOVER_MARKED, HOVER_PROJECT_TOML,
    };
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_exported_once(d.path());
    m.open_review();
    assert!(m.written_text().contains("checking which changed…"));
    idle(&mut m);
    assert_eq!(
        m.written_text(),
        format!("Will be written (0) · Up to date ({})", m.export_files())
    );
    let json = support::pgp_json_path(&m);
    let s = export_and_compare(&mut m, d.path());
    let also: Vec<_> =
        s.w.also_updated
            .iter()
            .map(|e| (e.hover, e.marked))
            .collect();
    assert_eq!(
        also,
        [
            (HOVER_PROJECT_TOML, true),
            (HOVER_GENERATED, true),
            (HOVER_MANIFEST, true)
        ],
        "{:?}",
        s.w.also_updated
    );
    assert!(s.w.outputs.iter().all(|o| o.marked), "{:?}", s.w.outputs);
    assert!(s.listed.contains(&json) && !s.changed.contains(&json));
    // A marked entry's hover says so after what the file is.
    for (e, l) in s.w.also_updated.iter().zip(&s.w.labels) {
        assert_eq!(l.hover, format!("{} {HOVER_MARKED}", e.hover));
    }
    assert_eq!(
        HOVER_MARKED,
        "It changes only if this export's new files change it."
    );
}

/// (2) The owner of a borrowed slot changed: the borrower's copy is replaced from the DDS this run writes, and its hash
/// goes into both record files. All three are listed, and all three change.
#[test]
fn a_changed_owner_lists_the_borrowed_copy_and_both_record_files() {
    use sdc_gui::model::review::{HOVER_COPY, HOVER_GENERATED, HOVER_PROJECT_TOML};
    let d = tempfile::tempdir().unwrap();
    let (mut m, source, want) = vanilla_borrow(d.path());
    let first = export_and_compare(&mut m, d.path());
    assert!(first.changed.contains(&want), "the first export copies");
    image::ImageBuffer::from_fn(64, 64, |x, _| image::Rgb([120u8, x as u8, 255]))
        .save(&source)
        .unwrap();
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
    let s = export_and_compare(&mut m, d.path());
    assert_eq!(listed_with(&s, HOVER_COPY), std::slice::from_ref(&want));
    for f in [
        want,
        listed_with(&s, HOVER_PROJECT_TOML)[0].clone(),
        listed_with(&s, HOVER_GENERATED)[0].clone(),
    ] {
        assert!(s.changed.contains(&f), "{f:?} changes: {:?}", s.changed);
    }
}

/// (3) A saved set edit changes the next PGPatcher JSON while the old file is Tessalith's: the JSON and
/// `sdc-project.toml` (its new hash) are listed and change.
#[test]
fn a_saved_edit_that_changes_our_json_lists_it_and_the_project_record() {
    use sdc_gui::model::review::HOVER_PROJECT_TOML;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_exported_once(d.path());
    let json = support::pgp_json_path(&m);
    m.set_json_value(
        "landscape/rock01",
        "specular_level",
        Some(serde_json::json!(0.07)),
    );
    m.save().unwrap();
    assert!(m.wait_idle(Duration::from_secs(60)));
    let s = export_and_compare(&mut m, d.path());
    let project = listed_with(&s, HOVER_PROJECT_TOML);
    assert_eq!(project.len(), 1, "{:?}", s.w.also_updated);
    for f in [&json, &project[0]] {
        assert!(s.listed.contains(f) && s.changed.contains(f), "{f:?}");
    }
}

/// (4) A JSON with no record that already holds the bytes the export would write: the writer adopts it (same bytes,
/// so the JSON does not change) and records its hash in both record files. Those two are listed; the JSON is not.
#[test]
fn an_unrecorded_json_with_the_same_bytes_lists_only_the_record_files() {
    use sdc_gui::model::review::{HOVER_GENERATED, HOVER_PROJECT_TOML};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_exported_once(d.path());
    let json = support::pgp_json_path(&m);
    // Forget the records: the JSON on disk is now one Tessalith has no hash for.
    let project = m.sets_doc.as_ref().unwrap().path().to_path_buf();
    let text = std::fs::read_to_string(&project).unwrap();
    let kept: String = text
        .lines()
        .filter(|l| !l.contains("json_sha256"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_ne!(kept, text, "guard: the record was there");
    std::fs::write(&project, kept).unwrap();
    std::fs::remove_file(m.root.clone().unwrap().join("_sdc").join("generated.json")).unwrap();
    m.reload_documents();
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.header_export_clicked();
    // Unrecorded: Review asks; Keep is the answer that leaves the file as it is.
    m.answer_review_json(false);
    let s = export_and_compare(&mut m, d.path());
    assert!(
        !s.listed.contains(&json) && !s.changed.contains(&json),
        "{json:?}"
    );
    for f in [
        listed_with(&s, HOVER_PROJECT_TOML),
        listed_with(&s, HOVER_GENERATED),
    ] {
        assert_eq!(f.len(), 1, "{:?}", s.w.also_updated);
        assert!(s.changed.contains(&f[0]), "{:?} changes", f[0]);
    }
}

/// (5) A file Tessalith did not write sits where a borrowed copy goes: the writer replaces it only with the same bytes,
/// so the copy is not listed. Whether those are the owner's new bytes is this run's (ruling 3c-50), so the two record
/// files its hash would go to are listed, marked; here they stay as they are.
#[test]
fn a_foreign_copy_destination_is_not_listed_and_its_records_are_marked() {
    use sdc_gui::model::review::{HOVER_COPY, HOVER_GENERATED, HOVER_PROJECT_TOML};
    let d = tempfile::tempdir().unwrap();
    let (mut m, _, want) = vanilla_borrow(d.path());
    export_and_compare(&mut m, d.path());
    std::fs::write(&want, b"hand made").unwrap();
    let s = export_and_compare(&mut m, d.path());
    assert!(
        listed_with(&s, HOVER_COPY).is_empty(),
        "{:?}",
        s.w.also_updated
    );
    for hover in [HOVER_PROJECT_TOML, HOVER_GENERATED] {
        let e: Vec<_> =
            s.w.also_updated
                .iter()
                .filter(|e| e.hover == hover)
                .collect();
        assert!(e.len() == 1 && e[0].marked, "{:?}", s.w.also_updated);
    }
    assert!(!s.changed.contains(&want));
    assert_eq!(std::fs::read(&want).unwrap(), b"hand made");
}

/// (6) A NIF whose export path holds a file Tessalith did not write, answered Keep: the NIF export skips it, so
/// Review lists neither the NIF nor its record.
#[test]
fn a_foreign_nif_answered_keep_is_not_listed() {
    use sdc_gui::model::meshes::export::GuardChoice;
    use sdc_gui::model::review::{HOVER_NIF, HOVER_NIF_RECORD};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_exported_once(d.path());
    let mut row = support::nif_row("meshes\\x.nif", Some([7; 32]));
    row.answer = Some(GuardChoice::KeepTheirs);
    m.set_nif_review_rows(vec![row]);
    let s = export_and_compare(&mut m, d.path());
    assert!(
        listed_with(&s, HOVER_NIF).is_empty(),
        "{:?}",
        s.w.also_updated
    );
    assert!(listed_with(&s, HOVER_NIF_RECORD).is_empty());
    assert!(!s
        .changed
        .iter()
        .any(|p| p.to_string_lossy().contains("x.nif")));
}

// ---- Task 4 fix round 1 (ruling 3c-48): what a listed write replaces or moves counts under it ----

/// (a) Saving a listed document removes its crash `.recovery` sibling; the removal counts under the document.
#[test]
fn a_recovery_file_removed_by_a_listed_save_counts_under_the_document() {
    use engine::config::AlphaPolicy;
    use sdc_gui::model::review::HOVER_SDC_TOML;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_exported_once(d.path());
    m.decide(
        "landscape/leaf.png",
        sdc_gui::model::Answer::Alpha(AlphaPolicy::Blend),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    let toml = m.project.as_ref().unwrap().path().to_path_buf();
    let recovery = sdc_gui::crash_log::recovery_path(&toml);
    std::fs::write(&recovery, "# edits from a crash\n").unwrap();
    let s = export_and_compare(&mut m, d.path());
    assert_eq!(listed_with(&s, HOVER_SDC_TOML), [toml]);
    // Decided before the run: not marked, and its hover is the file's alone.
    let i =
        s.w.also_updated
            .iter()
            .position(|e| e.hover == HOVER_SDC_TOML)
            .unwrap();
    assert!(!s.w.also_updated[i].marked);
    assert_eq!(s.w.labels[i].hover, HOVER_SDC_TOML);
    assert!(!recovery.exists(), "the save removed it");
    assert!(!s.listed.contains(&recovery), "not an entry of its own");
}

/// The complex-material fixture of `cm_json.rs`: `iron` (diffuse + `_m`) under `<dir>/src/m`, its JSON export on,
/// exported once. Returns the model and the JSON's path.
fn cm_exported_once(dir: &std::path::Path) -> (AppModel, std::path::PathBuf) {
    let mut m = cm_project(dir, "iron");
    export_and_compare(&mut m, dir);
    let json = m.export_root().join("m").join("iron_m.json");
    assert!(json.is_file(), "{:?}", m.export.notes);
    (m, json)
}

/// (b) A JSON of ours renamed on disk to another case: the next export writes it under the name it has now and removes
/// the old spelling. The JSON is listed (its name on disk changes); the old spelling counts under it.
#[test]
fn a_case_only_rename_lists_the_json_and_counts_the_old_spelling_under_it() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, json) = cm_exported_once(d.path());
    let old = json.with_file_name("IRON_M.json");
    std::fs::rename(&json, &old).unwrap();
    let s = export_and_compare(&mut m, d.path());
    assert!(s.listed.contains(&json), "{:?}", s.w.outputs);
    assert!(!s.listed.contains(&old));
    let names: Vec<String> = std::fs::read_dir(json.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".json"))
        .collect();
    assert_eq!(names, ["iron_m.json"], "written under its new spelling");
}

/// A snapshot as `support::tree_hashes` returns it, from (path, bytes' stand-in) pairs.
fn snapshot(
    files: &[(&std::path::Path, &str)],
) -> std::collections::BTreeMap<std::path::PathBuf, String> {
    files
        .iter()
        .map(|(p, h)| (p.to_path_buf(), h.to_string()))
        .collect()
}

/// Isolate one listed writer and simulate one additional removal across its export.
fn unexpected_removal(
    w: &sdc_gui::model::review::ReviewWrites,
    removals: &support::RemovalCredits,
    target: &std::path::Path,
    gone: &std::path::Path,
    old_hash: &str,
) -> Vec<String> {
    let mut w = w.clone();
    w.outputs.retain(|e| e.path == target);
    w.also_updated.retain(|e| e.path == target);
    assert_eq!(support::listed(&w).len(), 1, "{target:?}");
    support::compare_listing(
        &w,
        &snapshot(&[(target, "before"), (gone, old_hash)]),
        &snapshot(&[(target, "after")]),
        None,
        removals,
    )
    .violations
}

#[test]
fn a_skipped_cm_finisher_cannot_credit_its_owned_twins_removal() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, json) = cm_exported_once(d.path());
    let old = json.with_file_name("IRON_M.json");
    std::fs::rename(&json, &old).unwrap();
    let hash = sdc_gui::model::history::sha256_of(&old).unwrap();
    // Fail a texture write while the required iron_m.dds and the owned JSON still exist.
    let diffuse = json.with_file_name("iron.dds");
    std::fs::remove_file(&diffuse).unwrap();
    std::fs::create_dir(&diffuse).unwrap();
    m.set_reencode_all(true);
    let s = export_and_compare(&mut m, d.path());
    assert!(m.export.summary.as_ref().unwrap().failed > 0);
    assert!(m
        .export
        .notes
        .iter()
        .any(|n| n.contains("Complex material JSON not written")));
    assert_eq!(
        sdc_gui::model::history::sha256_of(&old).as_deref(),
        Some(hash.as_str())
    );
    let bad = unexpected_removal(&s.w, &s.removals, &json, &old, &hash);
    let missing = format!("changed, not listed: {old:?} (before true, after false)");
    assert_eq!(bad, [missing]);
}

#[test]
#[cfg(windows)]
fn a_failed_initial_save_cannot_credit_the_previous_cm_exports_twin() {
    use std::os::windows::fs::OpenOptionsExt;
    let d = tempfile::tempdir().unwrap();
    let (mut m, json) = cm_exported_once(d.path());
    let old = json.with_file_name("IRON_M.json");
    std::fs::rename(&json, &old).unwrap();
    let hash = sdc_gui::model::history::sha256_of(&old).unwrap();
    m.set_field("m/iron.png", "mips.filter", serde_json::json!("tent"));
    idle(&mut m);
    let settings = m.project.as_ref().unwrap();
    assert!(settings.is_dirty());
    let lock = std::fs::File::options()
        .read(true)
        .share_mode(1)
        .open(settings.path())
        .unwrap();
    m.header_export_clicked();
    m.expire_review_json();
    assert!(m.review_gate().is_ok(), "{:?}", m.review_gate());
    let w = m.review_writes();
    let pending = support::PendingRemovals::before_export(&m);
    m.export_from_review();
    idle(&mut m);
    drop(lock);
    assert!(matches!(m.dialog, Some(Dialog::Error(_))), "{:?}", m.dialog);
    assert!(m.project.as_ref().unwrap().is_dirty());
    assert_eq!(sdc_gui::model::history::sha256_of(&old).unwrap(), hash);
    let bad = unexpected_removal(&w, &pending.after_export(&m), &json, &old, &hash);
    assert_eq!(
        bad,
        [format!(
            "changed, not listed: {old:?} (before true, after false)"
        )]
    );
}

#[test]
#[cfg(windows)]
fn a_failed_second_save_credits_only_the_completed_settings_save() {
    use std::os::windows::fs::OpenOptionsExt;
    let d = tempfile::tempdir().unwrap();
    let (mut m, json) = cm_exported_once(d.path());
    let old = json.with_file_name("IRON_M.json");
    std::fs::rename(&json, &old).unwrap();
    m.set_field("m/iron.png", "mips.filter", serde_json::json!("tent"));
    idle(&mut m);
    let settings = m.project.as_ref().unwrap().path().to_owned();
    let recovery = sdc_gui::crash_log::recovery_path(&settings);
    std::fs::write(&recovery, "recover settings").unwrap();
    let doc = m.sets_doc.as_mut().unwrap();
    doc.set_json_name(Some("iron-project"));
    assert!(doc.is_dirty());
    let lock = std::fs::File::options()
        .read(true)
        .share_mode(1)
        .open(doc.path())
        .unwrap();
    m.open_review();
    assert!(m.review_gate().is_ok());
    let w = m.review_writes();
    let pending = support::PendingRemovals::before_export(&m);
    let before = support::tree_hashes(&[d.path()]);
    m.export_from_review();
    idle(&mut m);
    drop(lock);
    let after = support::tree_hashes(&[d.path()]);
    assert!(matches!(m.dialog, Some(Dialog::Error(_))));
    assert!(!pending.started(&m));
    assert!(!m.project.as_ref().unwrap().is_dirty() && m.sets_doc.as_ref().unwrap().is_dirty());
    assert!(before.contains_key(&recovery) && !after.contains_key(&recovery));
    assert_ne!(before[&settings], after[&settings]);
    assert_eq!(before[&old], after[&old]);
    let credits = pending.after_export(&m);
    assert_eq!(credits.initial_saves, [settings].into());
    assert!(credits.guarded_twins.is_empty());
    let comparison = support::compare_listing(&w, &before, &after, None, &credits);
    assert!(!comparison
        .violations
        .iter()
        .any(|v| v.contains(".recovery")));
}

#[test]
fn the_listing_rules_reject_a_foreign_json_twin() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, json) = cm_exported_once(d.path());
    let old = json.with_file_name("IRON_M.json");
    std::fs::rename(&json, &old).unwrap();
    let s = export_and_compare(&mut m, d.path());
    assert_eq!(
        unexpected_removal(&s.w, &s.removals, &json, &old, "foreign").len(),
        1
    );
}

#[test]
fn the_listing_rules_reject_twins_of_the_sidecar_and_pgpatcher_json() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_exported_once(d.path());
    let side = m.sets_doc.as_ref().unwrap().sidecar_path();
    let pgp = support::pgp_json_path(&m);
    let mut twins = vec![];
    for path in [side, pgp.clone()] {
        let old = path.with_file_name(path.file_name().unwrap().to_ascii_uppercase());
        let sha = sdc_gui::model::history::sha256_of(&path).unwrap();
        std::fs::rename(&path, &old).unwrap();
        twins.push((path, old, sha));
    }
    m.header_export_clicked();
    m.expire_review_json();
    let upper = pgp.with_file_name("TEXTURES.JSON");
    let before = support::tree_hashes(&[pgp.parent().unwrap()]);
    assert!(before.contains_key(&upper) && !before.contains_key(&pgp));
    let s = export_and_compare(&mut m, d.path());
    assert!(m.export.summary.as_ref().is_some_and(|s| s.failed == 0));
    let after = support::tree_hashes(&[pgp.parent().unwrap()]);
    assert!(after.contains_key(&pgp) && !after.contains_key(&upper));
    assert!(s.changed.contains(&pgp) && !s.changed.contains(&upper));
    for (path, old, sha) in twins {
        assert_eq!(
            unexpected_removal(&s.w, &s.removals, &path, &old, &sha).len(),
            1,
            "{path:?}"
        );
    }
}

#[test]
fn the_listing_rules_reject_recovery_removal_by_a_record_only_save() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_exported_once(d.path());
    m.set_json_value(
        "landscape/rock01",
        "specular_level",
        Some(serde_json::json!(0.07)),
    );
    m.save().unwrap();
    idle(&mut m);
    let doc = m.sets_doc.as_ref().unwrap().path().to_path_buf();
    let recovery = sdc_gui::crash_log::recovery_path(&doc);
    std::fs::write(&recovery, "crash").unwrap();
    let s = export_and_compare(&mut m, d.path());
    assert!(s.changed.contains(&doc) && recovery.is_file());
    assert_eq!(
        unexpected_removal(&s.w, &s.removals, &doc, &recovery, "crash").len(),
        1
    );
}

/// A list of `also_updated` entries alone: (path, hover, marked).
fn list_of(
    entries: &[(&std::path::Path, &'static str, bool)],
) -> sdc_gui::model::review::ReviewWrites {
    use sdc_gui::model::review::{ReviewWrite, ReviewWrites};
    ReviewWrites {
        also_updated: entries
            .iter()
            .map(|(p, hover, marked)| ReviewWrite {
                kind: sdc_gui::model::review::WriteKind::Other,
                path: p.to_path_buf(),
                hover,
                marked: *marked,
            })
            .collect(),
        ..Default::default()
    }
}

/// (c) A NIF edit record moved aside to `<stem>.<seconds>.toml` (Note GD25) counts under the listed record beside it
/// when it is created holding the record's bytes from before, and under nothing else (ruling 3c-52: the direction is
/// checked). The move itself runs in `nif_edit.rs`'s tests; this pins the rule on snapshots.
#[test]
fn a_nif_record_moved_aside_counts_under_the_listed_record() {
    use sdc_gui::model::review::HOVER_NIF_RECORD;
    use std::path::Path;
    let rec = Path::new(r"C:\m\out\.sdc\nif-edits\meshes\x.nif.sdc-nifedit.toml");
    let w = list_of(&[(rec, HOVER_NIF_RECORD, false)]);
    let moved = rec.with_file_name("x.nif.sdc-nifedit.1763500000.toml");
    let check = |b: &[(&Path, &str)], a: &[(&Path, &str)]| {
        support::listing_violations(&w, &snapshot(b), &snapshot(a), None)
    };
    assert_eq!(
        check(&[(rec, "old")], &[(rec, "new"), (&moved, "old")]),
        Vec::<String>::new()
    );
    assert_eq!(
        check(&[(rec, "old")], &[(rec, "new"), (&moved, "other")]).len(),
        1,
        "other bytes are not the record moved aside"
    );
    assert_eq!(
        check(&[(rec, "old"), (&moved, "old")], &[(rec, "new")]).len(),
        1,
        "a removal is not a move aside"
    );
    let other = rec.with_file_name("x.nif.sdc-nifedit.backup.toml");
    assert_eq!(
        check(&[(rec, "old")], &[(rec, "new"), (&other, "old")]).len(),
        1,
        "only `<stem>.<seconds>.toml`"
    );
    let elsewhere = Path::new(r"C:\m\x.nif.sdc-nifedit.1763500000.toml");
    assert_eq!(
        check(&[(rec, "old")], &[(rec, "new"), (elsewhere, "old")]).len(),
        1,
        "only beside the record"
    );
    // Ruling 3c-57 (c): one move aside per record; a second file with the old bytes is a change of its own.
    let second = rec.with_file_name("x.nif.sdc-nifedit.1763500001.toml");
    let two = check(
        &[(rec, "old")],
        &[(rec, "new"), (&moved, "old"), (&second, "old")],
    );
    assert_eq!(two.len(), 1, "{two:?}");
}

/// Ruling 3c-57 (a): a removed `.recovery` counts only beside a listed document the export saves (`sdc.toml`,
/// `sdc-project.toml`), not beside any listed file.
#[test]
fn the_listing_rules_credit_a_recovery_file_only_beside_a_saved_document() {
    use sdc_gui::model::review::HOVER_PROJECT_TOML;
    use std::path::Path;
    let dds = Path::new(r"C:\e\a.dds");
    let doc = Path::new(r"C:\p\sdc-project.toml");
    let w = sdc_gui::model::review::ReviewWrites {
        outputs: vec![sdc_gui::model::review::Output {
            path: dds.to_path_buf(),
            marked: false,
        }],
        ..list_of(&[(doc, HOVER_PROJECT_TOML, false)])
    };
    let mut removals = support::RemovalCredits::default();
    removals.initial_saves.insert(doc.to_path_buf());
    let check = |b: &[(&Path, &str)], a: &[(&Path, &str)]| {
        support::compare_listing(&w, &snapshot(b), &snapshot(a), None, &removals).violations
    };
    let beside_doc = Path::new(r"C:\p\sdc-project.toml.recovery");
    assert_eq!(
        check(
            &[(dds, "1"), (doc, "1"), (beside_doc, "crash")],
            &[(dds, "2"), (doc, "2")]
        ),
        Vec::<String>::new()
    );
    let beside_dds = Path::new(r"C:\e\a.dds.recovery");
    let bad = check(
        &[(dds, "1"), (doc, "1"), (beside_dds, "crash")],
        &[(dds, "2"), (doc, "2")],
    );
    assert_eq!(bad.len(), 1, "{bad:?}");
}

/// Ruling 3c-57 (b): a removed old case spelling counts only for a listed JSON, and only in that JSON's own folder.
#[test]
fn the_listing_rules_credit_an_old_spelling_only_of_a_json_in_its_folder() {
    use std::path::Path;
    let json = Path::new(r"C:\e\m\iron_m.json");
    let dds = Path::new(r"C:\e\m\iron.dds");
    let twin = Path::new(r"C:\e\m\IRON_M.json");
    let mut removals = support::RemovalCredits::default();
    removals
        .guarded_twins
        .insert(json.to_path_buf(), (twin.to_path_buf(), "old".into()));
    let w = sdc_gui::model::review::ReviewWrites {
        outputs: [json, dds]
            .iter()
            .map(|p| sdc_gui::model::review::Output {
                path: p.to_path_buf(),
                marked: false,
            })
            .collect(),
        ..Default::default()
    };
    let check = |gone: &Path| {
        support::compare_listing(
            &w,
            &snapshot(&[(json, "1"), (dds, "1"), (gone, "old")]),
            &snapshot(&[(json, "2"), (dds, "2")]),
            None,
            &removals,
        )
        .violations
    };
    assert_eq!(check(twin), Vec::<String>::new());
    assert_eq!(
        check(Path::new(r"C:\e\M\iron_m.json")).len(),
        1,
        "another folder's spelling"
    );
    assert_eq!(check(Path::new(r"C:\e\m\IRON.dds")).len(), 1, "not a JSON");
}

/// Ruling 3c-52: a `.recovery` beside a listed document counts under it only when the save removed it; one created or
/// changed by the export is a change of its own.
#[test]
fn the_listing_rules_credit_a_recovery_file_only_when_it_is_removed() {
    use sdc_gui::model::review::HOVER_SDC_TOML;
    use std::path::Path;
    let doc = Path::new(r"C:\p\sdc.toml");
    let rec = Path::new(r"C:\p\sdc.toml.recovery");
    let w = list_of(&[(doc, HOVER_SDC_TOML, false)]);
    let mut removals = support::RemovalCredits::default();
    removals.initial_saves.insert(doc.to_path_buf());
    let check = |b: &[(&Path, &str)], a: &[(&Path, &str)]| {
        support::compare_listing(&w, &snapshot(b), &snapshot(a), None, &removals).violations
    };
    assert_eq!(
        check(&[(doc, "old"), (rec, "crash")], &[(doc, "new")]),
        Vec::<String>::new()
    );
    let created = check(&[(doc, "old")], &[(doc, "new"), (rec, "crash")]);
    assert_eq!(created.len(), 1, "{created:?}");
    let changed = check(&[(doc, "old"), (rec, "a")], &[(doc, "new"), (rec, "b")]);
    assert_eq!(changed.len(), 1, "{changed:?}");
}

/// Ruling 3c-51 (b): the scan-store autosave of a listed NIF record's data path counts under that record when the
/// export removed it; one created, or one of another data path, is a change of its own.
#[test]
fn the_listing_rules_credit_a_removed_nif_autosave_to_its_record() {
    use sdc_gui::model::meshes::edit_doc::autosave_path;
    use sdc_gui::model::review::HOVER_NIF_RECORD;
    use std::path::Path;
    let rec = Path::new(r"C:\m\out\.sdc\nif-edits\meshes\x.nif.sdc-nifedit.toml");
    let store = Path::new(r"C:\appdata\sdc");
    let mine = autosave_path(store, r"meshes\x.nif").unwrap();
    let other = autosave_path(store, r"meshes\y.nif").unwrap();
    let w = list_of(&[(rec, HOVER_NIF_RECORD, false)]);
    let check = |b: &[(&Path, &str)], a: &[(&Path, &str)]| {
        support::listing_violations(&w, &snapshot(b), &snapshot(a), Some(store))
    };
    assert_eq!(
        check(&[(rec, "old"), (&mine, "ops")], &[(rec, "new")]),
        Vec::<String>::new()
    );
    let created = check(&[(rec, "old")], &[(rec, "new"), (&mine, "ops")]);
    assert_eq!(created.len(), 1, "{created:?}");
    let theirs = check(&[(rec, "old"), (&other, "ops")], &[(rec, "new")]);
    assert_eq!(theirs.len(), 1, "{theirs:?}");
    // Ruling 3c-57 (d): only under the session's own scan store.
    let elsewhere = autosave_path(Path::new(r"C:\other\sdc"), r"meshes\x.nif").unwrap();
    let foreign = check(&[(rec, "old"), (&elsewhere, "ops")], &[(rec, "new")]);
    assert_eq!(foreign.len(), 1, "{foreign:?}");
}

/// Ruling 3c-52: no blanket exemption. Of two listed copies, one changes; the other, unchanged, passes only when it is
/// itself marked run-dependent.
#[test]
fn the_listing_rules_need_each_unchanged_entry_marked() {
    use sdc_gui::model::review::HOVER_COPY;
    use std::path::Path;
    let (a, b) = (Path::new(r"C:\e\a_n.dds"), Path::new(r"C:\e\b_n.dds"));
    let before = snapshot(&[(a, "1"), (b, "1")]);
    let after = snapshot(&[(a, "2"), (b, "1")]);
    let bad = support::listing_violations(
        &list_of(&[(a, HOVER_COPY, true), (b, HOVER_COPY, false)]),
        &before,
        &after,
        None,
    );
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(bad[0].contains("b_n.dds"), "{bad:?}");
    let ok = support::listing_violations(
        &list_of(&[(a, HOVER_COPY, false), (b, HOVER_COPY, true)]),
        &before,
        &after,
        None,
    );
    assert_eq!(ok, Vec::<String>::new());
}

#[test]
fn review_writes_is_cached() {
    use sdc_gui::model::meshes::export::GuardChoice;
    use std::sync::Arc;
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    // Ruling 3c-49: with an Also tick, `output_root()` asks the disk, so a key that read it would on every lookup.
    m.set_also_tick(AlsoTarget::Vanilla, true);
    assert!(m.wait_idle(Duration::from_secs(120)));
    let rows = vec![support::nif_row("meshes\\x.nif", Some([7; 32]))];
    m.set_nif_review_rows(rows.clone());
    m.header_export_clicked();
    let a = m.review_writes();
    let b = m.review_writes();
    assert!(Arc::ptr_eq(&a, &b), "two calls in a row share one list");
    assert_eq!(
        a.labels.len(),
        a.also_updated.len(),
        "the footer's text comes with the list"
    );
    let (builds, queries) = (m.review_writes_builds(), m.review_writes_fs_queries());
    assert!(queries > 0, "guard: a build asks the disk");
    for _ in 0..50 {
        // app.rs hands the NIF rows over every frame.
        m.set_nif_review_rows(rows.clone());
        let _ = m.review_writes();
    }
    assert_eq!(m.review_writes_builds(), builds, "a frame builds no list");
    assert_eq!(
        m.review_writes_fs_queries(),
        queries,
        "a frame asks the disk nothing"
    );
    // The NIF answer is in the key (ruling 3c-46): Keep takes the NIF off the list.
    let mut keep = rows[0].clone();
    keep.answer = Some(GuardChoice::KeepTheirs);
    m.set_nif_review_rows(vec![keep]);
    assert!(
        !Arc::ptr_eq(&a, &m.review_writes()),
        "a new answer builds a new list"
    );
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

// ---- Task 4 fix round 2 (rulings 3c-50 to 3c-52): one row per finding of the re-review ----

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(120)));
}

/// Finding 1: the borrower's copy already holds the owner's next DDS while its record names the old one. The writer
/// adopts it (same bytes) and records the new hash: both record files change and are listed.
#[test]
fn a_copy_already_holding_the_owners_next_dds_lists_both_record_files() {
    use sdc_gui::model::review::{HOVER_GENERATED, HOVER_PROJECT_TOML};
    let d = tempfile::tempdir().unwrap();
    let (mut m, source, want) = vanilla_borrow(d.path());
    let old = std::fs::read(&source).unwrap();
    let next = |p: &std::path::Path| {
        image::ImageBuffer::from_fn(64, 64, |x, _| image::Rgb([120u8, x as u8, 255]))
            .save(p)
            .unwrap()
    };
    // The owner's next DDS, as an export writes it.
    next(&source);
    m.replan();
    idle(&mut m);
    export_and_compare(&mut m, d.path());
    let n = std::fs::read(&want).unwrap();
    // Back to the old one, exported: the copy and its record are the old DDS again.
    std::fs::write(&source, &old).unwrap();
    m.replan();
    idle(&mut m);
    export_and_compare(&mut m, d.path());
    assert_ne!(std::fs::read(&want).unwrap(), n, "guard: the old DDS");
    // The next DDS put at the copy by hand, then the owner changed to it.
    std::fs::write(&want, &n).unwrap();
    next(&source);
    m.replan();
    idle(&mut m);
    let s = export_and_compare(&mut m, d.path());
    assert!(!s.changed.contains(&want), "adopted, not rewritten");
    for f in [
        listed_with(&s, HOVER_PROJECT_TOML),
        listed_with(&s, HOVER_GENERATED),
    ] {
        assert_eq!(f.len(), 1, "{:?}", s.w.also_updated);
        assert!(s.changed.contains(&f[0]), "{:?} changes", f[0]);
    }
}

/// Finding 2: an Advanced skin JSON on disk with the bytes the export writes, its hash records removed. The finisher
/// adopts it and records its hash: both record files change and are listed.
#[test]
fn an_unrecorded_skin_json_with_its_bytes_lists_both_record_files() {
    use sdc_gui::model::review::{HOVER_GENERATED, HOVER_PROJECT_TOML};
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/x");
    for s in ["", "_rfaos", "_wet", "_detail_n"] {
        support::colour(&r.join(format!("h{s}.png")), 90);
    }
    let mut m = support::opened_as_the_app(d.path().to_path_buf(), d.path().join("export"));
    assert_eq!(m.target, engine::config::Target::Cs);
    export_and_compare(&mut m, d.path());
    let json = m.export_root().join("textures/x/h.csskin.json");
    assert!(json.is_file(), "{:?}", m.export.notes);
    let project = m.sets_doc.as_ref().unwrap().path().to_path_buf();
    let text = std::fs::read_to_string(&project).unwrap();
    let kept: String = text
        .lines()
        .filter(|l| !l.contains("cs_skin_sha256"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_ne!(kept, text, "guard: the record was there");
    std::fs::write(&project, kept).unwrap();
    std::fs::remove_file(m.root.clone().unwrap().join("_sdc").join("generated.json")).unwrap();
    m.reload_documents();
    idle(&mut m);
    let s = export_and_compare(&mut m, d.path());
    assert!(!s.changed.contains(&json), "adopted, not rewritten");
    for f in [
        listed_with(&s, HOVER_PROJECT_TOML),
        listed_with(&s, HOVER_GENERATED),
    ] {
        assert_eq!(f.len(), 1, "{:?}", s.w.also_updated);
        assert!(s.changed.contains(&f[0]), "{:?} changes", f[0]);
    }
}

/// B1: two PBR sets exported, then one set's DDS dropped from the manifest. The engine keeps that DDS as unknown, its
/// set leaves the PGPatcher JSON, and the JSON is written with one set: it is listed and changes.
#[test]
fn a_two_set_json_with_one_manifest_entry_gone_lists_the_json() {
    use image::{ImageBuffer, Rgb};
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/test");
    std::fs::create_dir_all(&t).unwrap();
    for (n, g) in [("wall", 120u8), ("door", 60)] {
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([g, g, g]))
            .save(t.join(format!("{n}.png")))
            .unwrap();
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([200u8, 100, 50]))
            .save(t.join(format!("{n}_rmaos.png")))
            .unwrap();
    }
    let mut m = support::opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    for n in ["wall", "door"] {
        let id = sdc_gui::model::sets::views(&m)
            .iter()
            .find(|v| v.key.ends_with(n))
            .expect("the set")
            .id
            .clone();
        m.set_shader(&id, Some(engine::shader::Shader::Pbr));
        idle(&mut m);
    }
    export_and_compare(&mut m, d.path());
    let json = support::pgp_json_path(&m);
    let two = std::fs::read_to_string(&json).unwrap();
    let root = m.root.clone().unwrap();
    let manifest = engine::discover::effective_output_root(&root, &m.output_root())
        .join("_sdc")
        .join("manifest.jsonl");
    let text = std::fs::read_to_string(&manifest).unwrap();
    let kept: String = text
        .lines()
        .filter(|l| !l.contains("wall.dds"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_ne!(kept, text, "guard: wall.dds was in the manifest");
    std::fs::write(&manifest, kept).unwrap();
    let s = export_and_compare(&mut m, d.path());
    assert!(s.listed.contains(&json), "{:?}", s.w.outputs);
    assert!(s.changed.contains(&json), "the JSON keeps one set");
    assert_ne!(std::fs::read_to_string(&json).unwrap(), two);
}

/// `cm_exported_once` before its export: the project, its JSON export on.
fn cm_project(dir: &std::path::Path, stem: &str) -> AppModel {
    use image::{ImageBuffer, Rgb, Rgba};
    let r = dir.join("src/m");
    std::fs::create_dir_all(&r).unwrap();
    let rgb = |x: u32, y: u32| [(x * 4) as u8, (y * 4) as u8, 90u8];
    ImageBuffer::from_fn(64, 64, |x, y| Rgb(rgb(x, y)))
        .save(r.join(format!("{stem}.png")))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| {
        let [a, b, c] = rgb(x, y);
        Rgba([a, b, c, (40 + x * 2) as u8])
    })
    .save(r.join(format!("{stem}_m.png")))
    .unwrap();
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(dir.join("export")),
        ..Default::default()
    };
    let mut m = AppModel::new(settings, None, support::session());
    m.accept = engine::policy::AcceptSuggestions::High;
    m.open_folder(dir.join("src"));
    idle(&mut m);
    support::choose_export(&mut m, dir.join("export"));
    let set = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(|v| v.key.ends_with(&format!("m/{stem}")))
        .expect("the complex material set")
        .id;
    m.set_cm_export(&set, true);
    m.set_cm_value(&set, "glossiness", Some(serde_json::json!(80)));
    idle(&mut m);
    m
}

/// B1: the folder holds both the recorded JSON under its old spelling and a foreign JSON under the current one.
/// The writer removes its own old spelling first, then keeps the foreign file (ruling 3c-51 (a)): the
/// JSON is listed and the removed spelling counts under it.
#[test]
fn a_case_twin_beside_a_foreign_json_lists_the_json_and_counts_the_twin_under_it() {
    use std::io::Write;
    let d = tempfile::tempdir().unwrap();
    let mut m = cm_project(d.path(), "silk");
    let folder = m.export_root().join("m");
    export_and_compare(&mut m, d.path());
    let json = folder.join("silk_m.json");
    assert!(json.is_file(), "{:?}", m.export.notes);
    // The Kelvin sign lowercases to k in a JSON record key, but NTFS allows both names in an ordinary directory.
    let twin = folder.join("SIL\u{212A}_M.json");
    std::fs::rename(&json, &twin).unwrap();
    std::fs::File::create_new(&json)
        .expect("the foreign spelling must name a separate file")
        .write_all(b"{ \"theirs\": true }\n")
        .unwrap();
    assert!(twin.is_file(), "guard: both spellings are on disk");
    let s = export_and_compare(&mut m, d.path());
    assert!(!twin.exists(), "the writer removed its own old spelling");
    assert!(s.changed.contains(&twin) && !s.changed.contains(&json));
    assert_eq!(std::fs::read(&json).unwrap(), b"{ \"theirs\": true }\n");
    assert!(s.listed.contains(&json), "{:?}", s.w.outputs);
}

/// B7: new Tree Anim evidence lands while Review is open on an exported set with an automatic subsurface: the
/// PGPatcher JSON switches to foliage. Review's list is built again (ruling 3c-51 (c)) and lists the JSON.
#[test]
fn new_tree_anim_evidence_while_review_is_open_lists_the_json() {
    use std::sync::Arc;
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb([200u8, 100, 50]))
        .save(r.join("rock01_s.png"))
        .unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    m.set_field(
        "landscape/rock01_s.png",
        "subtype",
        serde_json::json!("pbr_subsurface"),
    );
    idle(&mut m);
    export_and_compare(&mut m, d.path());
    let json = support::pgp_json_path(&m);
    let before =
        std::fs::read_to_string(&json).unwrap_or_else(|e| panic!("{e}: {:?}", m.export.notes));
    assert!(
        before.contains("subsurface") && !before.contains("subsurface_foliage"),
        "{before}"
    );
    m.header_export_clicked();
    let a = m.review_writes();
    // The diffuse's mesh key, as `auto_flags::hint` looks it up.
    let key = m
        .item("landscape/rock01.png")
        .and_then(|i| i.job.as_ref())
        .and_then(|j| {
            engine::shader::colour_mesh_key(&j.game_path, &j.output, Some(j.profile.subtype))
        })
        .expect("rock01's mesh key");
    m.set_tree_anim([(key, true)].into());
    let rebuilt = !Arc::ptr_eq(&a, &m.review_writes());
    let s = export_and_compare(&mut m, d.path());
    assert!(
        std::fs::read_to_string(&json)
            .unwrap()
            .contains("subsurface_foliage"),
        "guard: the JSON switched"
    );
    assert!(s.listed.contains(&json) && s.changed.contains(&json));
    assert!(rebuilt, "new evidence builds a new list");
}

/// B7: the owner of a borrowed slot is in an excluded group, and its DDS is replaced by other bytes of the same length
/// and modified time after Review read it. Past the expiry the copy is read again (ruling 3c-51 (d)): listed, and it
/// changes.
#[test]
fn an_excluded_owner_replaced_with_the_same_length_and_time_lists_the_copy() {
    use sdc_gui::model::review::HOVER_COPY;
    use sdc_gui::model::set_groups::GroupTarget;
    let d = tempfile::tempdir().unwrap();
    let (mut m, _, want) = vanilla_borrow(d.path());
    export_and_compare(&mut m, d.path());
    let owner = m
        .items
        .iter()
        .find(|i| i.name == "rock01_normal.png")
        .and_then(|i| i.job.as_ref())
        .map(|j| j.output.clone())
        .expect("the owner's DDS");
    m.move_to_group(
        &["landscape/rock01".to_string()],
        GroupTarget::New("Owner".into()),
    )
    .unwrap();
    m.set_group_excluded("Owner", true).unwrap();
    idle(&mut m);
    m.header_export_clicked();
    let _ = m.review_writes();
    let mut bytes = std::fs::read(&owner).unwrap();
    let t = std::fs::metadata(&owner).unwrap().modified().unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    std::fs::write(&owner, &bytes).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&owner)
        .unwrap()
        .set_modified(t)
        .unwrap();
    m.expire_review_json();
    let s = export_and_compare(&mut m, d.path());
    assert_eq!(listed_with(&s, HOVER_COPY), std::slice::from_ref(&want));
    assert_eq!(std::fs::read(&want).unwrap(), bytes);
}

// ---- Task 4 fix round 3 (rulings 3c-55 to 3c-58) ----

/// A vanilla project where the run's copies feed each other: set `leaf-2` (leaf.png) borrows rock01's normal, so the
/// export copies it to `leaf_n.dds`; set `leaf-3` holds `leaf_normal.png`, whose own output is that same `leaf_n.dds`,
/// and is in an excluded group; set `odd` borrows `leaf_normal.png`, so its copy reads `leaf_n.dds` after the first copy
/// made it. `leaf-2` is a Landscape set with an XPMF profile naming `leaf_n.dds`. Returns the model, odd's copy and the
/// profile's path.
fn chained_copies(d: &std::path::Path) -> (AppModel, std::path::PathBuf, std::path::PathBuf) {
    use sdc_gui::model::set_groups::GroupTarget;
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    use serde_json::json;
    support::flat(&d.join("textures/landscape/leaf_normal.png"));
    let mut m = support::pbr_ready(d);
    m.set_field("landscape/leaf.png", "subtype", json!("pbr_albedo"));
    idle(&mut m);
    let leaf = m.drop_new_set("landscape/leaf.png").unwrap();
    idle(&mut m);
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    let out = m.drop_new_set("landscape/leaf_normal.png").unwrap();
    idle(&mut m);
    for (item, set) in [
        ("landscape/rock01_normal.png", leaf.as_str()),
        ("landscape/leaf_normal.png", "landscape/odd"),
    ] {
        assert_eq!(
            m.drop_choice(item, set, DropChoice::Share),
            DropOutcome::Joined
        );
        idle(&mut m);
    }
    m.move_to_group(&[out], GroupTarget::New("Out".into()))
        .unwrap();
    m.set_group_excluded("Out", true).unwrap();
    {
        use engine::codes::Code;
        use engine::config::{AlphaPolicy, Subtype};
        use sdc_gui::model::Answer;
        m.set_field(
            "landscape/rock01_normal.png",
            "subtype",
            json!("landscape_normal"),
        );
        m.decide("landscape/leaf.png", Answer::Subtype(Subtype::Diffuse));
        m.decide("landscape/leaf.png", Answer::Alpha(AlphaPolicy::Cutout));
        m.decide("landscape/odd.png", Answer::Subtype(Subtype::Diffuse));
        m.decide("landscape/odd.png", Answer::Acknowledge(Code::DimSmall));
    }
    m.set_shader(&leaf, Some(engine::shader::Shader::Landscape));
    idle(&mut m);
    m.set_xpmf_on(&leaf, true);
    m.set_xpmf_value(&leaf, "editorIds", Some(json!(["MatoA"])));
    m.set_xpmf_value(
        &leaf,
        "normalTexture",
        Some(json!("textures/landscape/leaf_n.dds")),
    );
    idle(&mut m);
    let land = m.export_root().join("textures").join("landscape");
    let copies: Vec<_> = sdc_gui::model::pbr_export::prepare(&m)
        .expect("the copies")
        .copies
        .into_iter()
        .map(|(src, dst, _)| (src, dst))
        .collect();
    assert_eq!(
        copies,
        [
            (land.join("rock01_n.dds"), land.join("leaf_n.dds")),
            (land.join("leaf_n.dds"), land.join("odd_n.dds")),
        ],
        "guard: the second copy reads the first one's file"
    );
    let profile = m
        .export_root()
        .join("SKSE/Plugins/XPMF/tessalith_leaf-2.json")
        .components()
        .collect();
    (m, land.join("odd_n.dds"), profile)
}

/// Ruling 3c-55: a borrower copy the run makes is a file the run produces. The second copy reads the first one's file
/// (its owner is excluded), and the XPMF profile names it: both are written by the export and both are listed.
#[test]
fn a_file_an_earlier_copy_makes_feeds_a_later_copy_and_a_profile() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, odd, profile) = chained_copies(d.path());
    let s = export_and_compare(&mut m, d.path());
    assert!(odd.is_file() && profile.is_file(), "{:?}", m.export.notes);
    let leaf = odd.with_file_name("leaf_n.dds");
    let entries = support::listed(&s.w);
    for f in [&leaf, &odd, &profile] {
        assert!(s.changed.contains(f), "{f:?}");
        assert!(entries.iter().any(|e| e.path == *f && e.marked), "{f:?}");
    }
}

#[test]
fn a_directory_at_a_copy_destination_cannot_feed_a_later_copy_or_profile() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, odd, profile) = chained_copies(d.path());
    let leaf = odd.with_file_name("leaf_n.dds");
    std::fs::create_dir_all(&leaf).unwrap();
    m.save().unwrap();
    let s = export_and_compare(&mut m, d.path());
    assert!(leaf.is_dir() && !odd.exists() && !profile.exists());
    let doc = m.sets_doc.as_ref().unwrap();
    for path in [&leaf, &odd, &profile, doc.path(), &doc.sidecar_path()] {
        assert!(!s.changed.contains(path), "{path:?}");
        assert!(!s.listed.contains(path), "{path:?} in {:?}", s.listed);
    }
}

#[test]
fn a_dangling_junction_marks_the_borrowed_copy_and_both_records() {
    check_dangling_borrowed_normal(false);
}

#[test]
fn a_dangling_file_link_marks_the_borrowed_copy_and_both_records() {
    check_dangling_borrowed_normal(true);
}

fn check_dangling_borrowed_normal(file_link: bool) {
    use sdc_gui::model::review::{HOVER_COPY, HOVER_GENERATED, HOVER_PROJECT_TOML};
    use sdc_gui::model::set_groups::GroupTarget;
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("textures");
    let included = if file_link { "stone" } else { "rock" };
    for (folder, name) in [("alias", "rock"), ("real", included)] {
        support::colour(&src.join(folder).join(format!("{name}.png")), 90);
        support::flat(&src.join(folder).join(format!("{name}_n.png")));
    }
    support::colour(&src.join("other/leaf.png"), 90);
    let mut m = support::opened_as_the_app(src, d.path().join("export"));
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    assert_eq!(
        m.drop_choice("alias/rock_n.png", "other/leaf", DropChoice::Share),
        DropOutcome::Joined
    );
    m.move_to_group(&["alias/rock".into()], GroupTarget::New("Owner".into()))
        .unwrap();
    m.set_group_excluded("Owner", true).unwrap();
    idle(&mut m);
    m.save().unwrap();
    let out = m.export_root().join("textures");
    std::fs::create_dir_all(&out).unwrap();
    let real = out.join("real");
    let job = real.join(format!("{included}_n.dds"));
    if file_link {
        std::fs::create_dir(out.join("alias")).unwrap();
        let link = out.join("alias/rock_n.dds");
        #[cfg(windows)]
        if let Err(err) = std::os::windows::fs::symlink_file(&job, &link) {
            if err.raw_os_error() == Some(1314) {
                eprintln!(
                    "File symlink privilege unavailable; see the final-file-link library test."
                );
                return;
            }
            panic!("creating {link:?}: {err}");
        }
        #[cfg(not(windows))]
        std::os::unix::fs::symlink(&job, &link).unwrap();
        assert!(link.is_symlink() && !link.exists());
    } else {
        dir_link(&out.join("alias"), &real);
    }
    assert!(!real.exists());
    assert!(m.sets_doc.as_ref().unwrap().copy_shas().is_empty());

    let s = export_and_compare(&mut m, d.path());
    let copy = out.join("other/leaf_n.dds");
    assert_eq!(std::fs::read(&copy).unwrap(), std::fs::read(job).unwrap());
    let doc = m.sets_doc.as_ref().unwrap();
    for (path, hover) in [
        (copy.as_path(), HOVER_COPY),
        (doc.path(), HOVER_PROJECT_TOML),
        (&doc.sidecar_path(), HOVER_GENERATED),
    ] {
        assert!(s.changed.contains(path), "{path:?}");
        assert!(
            s.w.also_updated
                .iter()
                .any(|e| e.hover == hover && e.path == path && e.marked),
            "{path:?}"
        );
    }
}

#[test]
fn a_job_writing_a_borrowers_destination_marks_the_copy_and_both_records() {
    check_job_at_borrower_destination("landscape/leaf_normal.png");
}

#[test]
fn a_job_writing_through_a_junction_marks_the_borrowed_copy_and_both_records() {
    check_job_at_borrower_destination("job/leaf_normal.png");
}

fn check_job_at_borrower_destination(key: &str) {
    use engine::config::GreenConvention;
    use sdc_gui::model::review::{HOVER_COPY, HOVER_GENERATED, HOVER_PROJECT_TOML};
    use sdc_gui::model::set_groups::GroupTarget;
    use sdc_gui::model::Answer;
    let d = tempfile::tempdir().unwrap();
    let leaf = d.path().join("textures").join(key);
    support::colour(&leaf, 255);
    let (mut m, owner_source, dst) = vanilla_borrow(d.path());
    m.set_field(key, "subtype", serde_json::json!("landscape_normal"));
    m.decide(key, Answer::Green(GreenConvention::Directx));
    idle(&mut m);
    let output = &m.item(key).unwrap().job.as_ref().unwrap().output;
    if key.starts_with("job/") {
        assert_ne!(output, &dst);
        let target = dst.parent().unwrap();
        std::fs::create_dir_all(target).unwrap();
        if let Err(e) = try_dir_link(output.parent().unwrap(), target) {
            eprintln!("Skipping junction export test: {e}");
            return;
        }
        assert_eq!(
            output.parent().unwrap().canonicalize().unwrap(),
            target.canonicalize().unwrap()
        );
    } else {
        assert_eq!(output, &dst);
    }
    let first = export_and_compare(&mut m, d.path());
    let owner = std::fs::read(dst.with_file_name("rock01_n.dds")).unwrap();
    assert_ne!(std::fs::read(&dst).unwrap(), owner);
    assert!(m.sets_doc.as_ref().unwrap().copy_shas().is_empty());

    // The leaf job's next DDS matches the excluded owner, so the copy writer adopts it.
    m.move_to_group(
        &["landscape/rock01".to_string()],
        GroupTarget::New("Owner".into()),
    )
    .unwrap();
    m.set_group_excluded("Owner", true).unwrap();
    std::fs::copy(owner_source, &leaf).unwrap();
    m.replan();
    idle(&mut m);
    m.save().unwrap();
    let s = export_and_compare(&mut m, d.path());
    assert_eq!(std::fs::read(&dst).unwrap(), owner);
    let doc = m.sets_doc.as_ref().unwrap();
    for (path, hover) in [
        (dst.as_path(), HOVER_COPY),
        (doc.path(), HOVER_PROJECT_TOML),
        (&doc.sidecar_path(), HOVER_GENERATED),
    ] {
        assert!(s.changed.contains(path), "{path:?}");
        for run in [&first, &s] {
            assert!(
                run.w
                    .also_updated
                    .iter()
                    .any(|e| e.hover == hover && e.path == path && e.marked),
                "{path:?} in {:?}",
                run.w.also_updated
            );
        }
    }
}

/// B7: an export scoped to a set outside the PGPatcher JSON, while an exported PBR set out of scope gets Tree Anim
/// evidence. That set's entry depends on nothing in the run, so the JSON's text is decided now, from `review_json`'s
/// prepared exports: their cache must see the evidence (ruling 3c-51 (c)).
#[test]
fn new_evidence_for_a_set_out_of_scope_lists_the_json_it_changes() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb([200u8, 100, 50]))
        .save(r.join("rock01_s.png"))
        .unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    m.set_field(
        "landscape/rock01_s.png",
        "subtype",
        serde_json::json!("pbr_subsurface"),
    );
    idle(&mut m);
    export_and_compare(&mut m, d.path());
    let json = support::pgp_json_path(&m);
    let odd = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(|v| v.id == "landscape/odd")
        .expect("the odd set")
        .key;
    m.export_sets([odd].into(), "Export set".into());
    assert!(m.review.is_some(), "the scoped export opens Review");
    let _ = m.review_writes();
    let key = m
        .item("landscape/rock01.png")
        .and_then(|i| i.job.as_ref())
        .and_then(|j| {
            engine::shader::colour_mesh_key(&j.game_path, &j.output, Some(j.profile.subtype))
        })
        .expect("rock01's mesh key");
    m.set_tree_anim([(key, true)].into());
    let s = export_and_compare(&mut m, d.path());
    assert!(
        std::fs::read_to_string(&json)
            .unwrap()
            .contains("subsurface_foliage"),
        "guard: the scoped export rewrote the JSON"
    );
    assert!(s.listed.contains(&json) && s.changed.contains(&json));
}

/// Ruling 3c-56: the complex-material JSON's `_m` is on disk and also planned in this run. The writer checks it after
/// the CM phase, which Review cannot know, so a changed JSON is listed and marked.
#[test]
fn a_cm_json_whose_m_this_run_writes_is_marked_even_when_the_m_is_on_disk() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, json) = cm_exported_once(d.path());
    let iron = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(|v| v.key.ends_with("m/iron"))
        .expect("the iron set")
        .id;
    m.set_cm_value(&iron, "glossiness", Some(serde_json::json!(40)));
    m.save().unwrap();
    idle(&mut m);
    let s = export_and_compare(&mut m, d.path());
    let out: Vec<_> = s.w.outputs.iter().filter(|o| o.path == json).collect();
    assert!(out.len() == 1 && out[0].marked, "{:?}", s.w.outputs);
    assert!(s.changed.contains(&json));
}

#[test]
fn an_xpmf_profile_is_marked_when_the_run_rewrites_a_required_dds_already_on_disk() {
    use serde_json::{json, Value};
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    let id = "landscape/rock01";
    m.set_xpmf_on(id, true);
    m.set_xpmf_value(id, "editorIds", Some(json!(["MatoA"])));
    idle(&mut m);
    export_and_compare(&mut m, d.path());
    let profile = m
        .output_root()
        .join("SKSE/Plugins/XPMF/tessalith_rock01.json");
    let first: Value = serde_json::from_slice(&std::fs::read(&profile).unwrap()).unwrap();
    let diffuse = m
        .output_root()
        .join(first["diffuseTexture"].as_str().unwrap());
    assert!(diffuse.is_file());

    support::colour(&d.path().join("textures/landscape/rock01.png"), 200);
    m.set_xpmf_value(id, "editorIds", Some(json!(["MatoB"])));
    m.save().unwrap();
    m.replan();
    idle(&mut m);
    let s = export_and_compare(&mut m, d.path());
    assert!(s.changed.contains(&diffuse) && s.changed.contains(&profile));
    assert!(
        s.w.outputs.iter().any(|o| o.path == profile && o.marked),
        "the required DDS is produced by the run: {:?}",
        s.w.outputs
    );
}

/// Ruling 3c-58: two borrowers of one excluded owner. One build of Review's list reads the owner's DDS once; each
/// borrower's destination still gets its own check (both copies are listed when the owner changed).
#[test]
fn borrowers_of_one_owner_share_one_read_of_it_per_build() {
    use sdc_gui::model::review::HOVER_COPY;
    use sdc_gui::model::set_groups::GroupTarget;
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    let d = tempfile::tempdir().unwrap();
    let (mut m, _, leaf_copy) = vanilla_borrow(d.path());
    assert_eq!(
        m.drop_choice(
            "landscape/rock01_normal.png",
            "landscape/odd",
            DropChoice::Share
        ),
        DropOutcome::Joined
    );
    idle(&mut m);
    export_and_compare(&mut m, d.path());
    let odd_copy = leaf_copy.with_file_name("odd_n.dds");
    assert!(odd_copy.is_file(), "{:?}", m.export.notes);
    let owner = m
        .items
        .iter()
        .find(|i| i.name == "rock01_normal.png")
        .and_then(|i| i.job.as_ref())
        .map(|j| j.output.clone())
        .expect("the owner's DDS");
    m.move_to_group(
        &["landscape/rock01".to_string()],
        GroupTarget::New("Owner".into()),
    )
    .unwrap();
    m.set_group_excluded("Owner", true).unwrap();
    idle(&mut m);
    // The owner changes on disk, so each borrower's own check lists its copy.
    let mut bytes = std::fs::read(&owner).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    std::fs::write(&owner, &bytes).unwrap();
    m.header_export_clicked();
    m.expire_review_json();
    let reads = m.review_writes_source_reads();
    let w = m.review_writes();
    assert_eq!(
        m.review_writes_source_reads() - reads,
        1,
        "one read per build"
    );
    let mut copies: Vec<_> = w
        .also_updated
        .iter()
        .filter(|e| e.hover == HOVER_COPY)
        .map(|e| e.path.clone())
        .collect();
    copies.sort();
    assert_eq!(copies, [leaf_copy, odd_copy]);
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
    // Planning: decisions just made, the plan held until the test lets it land, so this state is always checked.
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened(d.path());
    m.hold_plan_results = true;
    m.decide(
        "landscape/leaf.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    m.open_review();
    let mut h = review_harness_at(m, size);
    assert!(h.state().full_plan_pending(), "guard: the plan is held");
    assert_the_footer_holds_its_controls(&h, "planning");
    let held = held_plan(h.state_mut());
    land_plan(h.state_mut(), held);
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

#[test]
fn a_long_json_path_stays_inside_the_review_footer() {
    use sdc_gui::model::review::{DiskState, JsonKind};

    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened_chosen_decided(d.path());
    support::choose_export(
        &mut m,
        d.path().join("a rather long folder name for the export"),
    );
    m.set_also_tick(AlsoTarget::Vanilla, true);
    m.set_also_tick(AlsoTarget::Cm, true);
    assert!(m.wait_idle(Duration::from_secs(120)));
    m.open_review();
    let line = m
        .review_json()
        .into_iter()
        .find(|l| l.kind == JsonKind::PgPatcher)
        .expect("the PBR set writes a PGPatcher JSON");
    assert_eq!(line.state, DiskState::New);
    let json_text = format!("{}: will be written", line.path.display());
    let writes = m.review_writes();
    let whole = &writes.whole;

    let h = review_harness_at(m, egui::vec2(480.0, 864.0));
    let right = h.ctx.content_rect().right();
    let mut json = h
        .query_all_by_role(Role::Label)
        .filter(|n| n.accesskit_node().value().as_deref() == Some(json_text.as_str()));
    let row = json
        .next()
        .expect("the JSON label names the whole path")
        .rect();
    assert!(json.next().is_none(), "the JSON label is unique");
    assert!(
        row.right() <= right + 0.5,
        "JSON row {row:?} exceeds viewport {right}"
    );

    let also = h
        .query_all_by_label_contains("Also updated: ")
        .next()
        .expect("the Also updated line is cut in the middle");
    assert_eq!(
        also.accesskit_node().value().as_deref(),
        Some(whole.as_str())
    );
    let row = also.rect();
    assert!(
        row.right() <= right + 0.5,
        "Also updated ellipsis {row:?} exceeds viewport {right}"
    );
}
