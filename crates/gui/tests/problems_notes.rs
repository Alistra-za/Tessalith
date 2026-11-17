//! Review E: Problems and Notes. Every code is a Problem or a Note once; a Note row says why it is fine.
mod support;
use eframe::egui;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::codes::Warning;
use sdc_gui::model::AppModel;
use support::*;

fn harness_at(model: AppModel, size: egui::Vec2) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(size)
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    h
}

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    harness_at(model, egui::vec2(1400.0, 900.0))
}

/// `problems.rs::many`, copied: `n` ready textures with no set.
fn many(dir: &std::path::Path, n: usize) -> AppModel {
    let mut m = opened(dir);
    let base = m.items.iter().find(|i| i.job.is_some()).unwrap().clone();
    m.items = (0..n)
        .map(|n| {
            let mut i = base.clone();
            i.key = format!("t/t{n:05}");
            i.name = format!("t{n:05}.png");
            i.status = sdc_gui::model::items::ItemStatus::Ready;
            i.warnings = vec![];
            i.set_key = None;
            i
        })
        .collect();
    m.sets = vec![];
    m.invalidate_views();
    m
}

/// `many(dir, 1)` with `ws` on its one texture: a list with nothing but these rows.
fn only(ws: &[Warning]) -> (tempfile::TempDir, AppModel) {
    let d = tempfile::tempdir().unwrap();
    let mut m = many(d.path(), 1);
    m.items[0].warnings = ws.to_vec();
    m.invalidate_views();
    (d, m)
}

/// Only `ws` on rock01's first member: every item and set warning of the PBR fixture is cleared first.
fn inject(m: &mut AppModel, ws: &[Warning]) {
    for i in m.items.iter_mut() {
        i.warnings = vec![];
    }
    for s in m.sets.iter_mut() {
        s.warnings.clear();
    }
    let key = rock01(m).members[0].clone();
    m.items.iter_mut().find(|i| i.key == key).unwrap().warnings = ws.to_vec();
    m.invalidate_views();
}

/// A PBR project whose only item warnings are `ws` (`bottom_panel.rs`'s `with_only_warnings`).
fn with_warnings(ws: &[Warning]) -> (tempfile::TempDir, AppModel) {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    inject(&mut m, ws);
    (d, m)
}

/// `inject`, with `ws` on rock01's set instead of a member (`SetItem` has `key`, not `id`).
fn inject_set(m: &mut AppModel, ws: &[Warning]) {
    for i in m.items.iter_mut() {
        i.warnings = vec![];
    }
    for s in m.sets.iter_mut() {
        s.warnings.clear();
    }
    let key = rock01(m).key;
    m.sets.iter_mut().find(|s| s.key == key).unwrap().warnings = ws.to_vec();
    m.invalidate_views();
}

/// A PBR project whose only warnings are `ws`, on rock01's set.
fn with_set_warnings(ws: &[Warning]) -> (tempfile::TempDir, AppModel) {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    inject_set(&mut m, ws);
    (d, m)
}

fn rock01(m: &AppModel) -> sdc_gui::model::sets::SetView {
    sdc_gui::model::sets::set_views(m)
        .into_iter()
        .find(|v| v.id == "landscape/rock01")
        .unwrap()
}

/// The node whose label contains `label` on the row that shows `sentence` (centre y within 4 px).
fn on_row<'a>(
    h: &'a Harness<'static, AppModel>,
    sentence: &str,
    label: &'a str,
) -> egui_kittest::Node<'a> {
    let y = h.get_by_label(sentence).rect().center().y;
    h.query_all_by_label_contains(label)
        .find(|n| (n.rect().center().y - y).abs() <= 4.0)
        .unwrap_or_else(|| panic!("no {label:?} on the row of {sentence:?}"))
}

/// The fixture's own rows (see the plan-specific rules): its tab counts and rock01's readiness with no warning.
fn baseline() -> (sdc_gui::model::bottom::TabCounts, bool) {
    let (_d, m) = with_warnings(&[]);
    (
        sdc_gui::model::bottom::cached_tab_counts(&m),
        sdc_gui::model::bottom::set_ready(&m, &rock01(&m)),
    )
}

fn label(b: sdc_gui::model::bottom::TabCounts, p: usize, h: usize, n: usize) -> String {
    use sdc_gui::model::bottom::{tab_label, TabCounts};
    tab_label(TabCounts {
        problems: b.problems + p,
        hidden: b.hidden + h,
        notes: b.notes + n,
    })
}

#[test]
fn every_code_has_one_class_and_the_counts_match_the_plan() {
    use sdc_gui::model::code_text::{row_class, RowClass};
    let notes = engine::codes::Code::ALL
        .iter()
        .filter(|c| row_class(**c) == RowClass::Note)
        .count();
    assert_eq!(engine::codes::Code::ALL.len(), 93);
    assert_eq!(notes, 22);
    assert_eq!(
        row_class(engine::codes::Code::ShaderUnavailable),
        RowClass::Problem
    );
}

#[test]
fn a_defaulted_channel_reads_as_a_note_that_says_it_is_fine() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{problems, Level};
    let w = Warning::new(
        Code::PackDefaulted,
        "x_rmaos.dds G: no metallic source; neutral default 0",
    )
    .with_data(serde_json::json!({"output": "x_rmaos.dds", "channel": "G", "slot": "metallic", "value": 0}));
    let (_d, m) = with_warnings(&[w]);
    let rows = problems(&m);
    let r = rows
        .iter()
        .find(|p| p.code == Some(Code::PackDefaulted))
        .unwrap();
    assert_eq!(r.level, Level::Info);
    assert_eq!(
        r.shown(),
        "x_rmaos.dds G: no metallic map, so the default 0 (not metal) is used. Fine unless part of it is metal."
    );
    assert!(
        !sdc_gui::model::bottom::counts_as_ready_problem(r),
        "a note never blocks Ready"
    );
}

#[test]
fn an_ao_default_prints_one_not_one_point_zero() {
    use engine::codes::{Code, Warning};
    let w = Warning::new(Code::PackDefaulted, "m").with_data(
        serde_json::json!({"output": "a_rmaos.dds", "channel": "B", "slot": "ao", "value": 1.0}),
    );
    assert_eq!(
        sdc_gui::model::code_text::note_sentence(&w),
        "a_rmaos.dds B: no AO map, so the default 1 (no extra shading) is used. Fine; creases just get no extra shadow."
    );
}

#[test]
fn a_npot_texture_without_mips_is_a_note() {
    use engine::codes::{Code, Warning};
    let (_d, m) = with_warnings(&[Warning::new(Code::DimNpot, "npot")]);
    let r = sdc_gui::model::bottom::problems(&m);
    let r = r.iter().find(|p| p.code == Some(Code::DimNpot)).unwrap();
    assert_eq!(
        r.shown(),
        "Keeps a size that isn't a power of two. That's fine: it has no mips."
    );
    assert!(!sdc_gui::model::counts_toward_badge(&Warning::new(
        Code::DimNpot,
        "npot"
    )));
}

/// The PACK-UNUSED-INPUT note parses the engine message `"{name}: {reason}; not converted"`
/// (`engine/src/pack/plan.rs:1254-1255`).
#[test]
fn an_unused_input_note_names_the_file_and_the_reason() {
    use engine::codes::{Code, Warning};
    let w = Warning::new(
        Code::PackUnusedInput,
        "x_h.png: the recipe has no height channel; not converted",
    );
    assert_eq!(
        sdc_gui::model::code_text::note_sentence(&w),
        "x_h.png isn't used: the recipe has no height channel. Nothing else changes."
    );
    let odd = Warning::new(Code::PackUnusedInput, "no colon here");
    assert_eq!(
        sdc_gui::model::code_text::note_sentence(&odd),
        "no colon here",
        "a message that doesn't parse shows unchanged"
    );
}

/// A Note-class code on a last-export File row gets the same note row as a live one (the File loop's Note rule).
#[test]
fn a_note_from_the_last_export_is_a_note_row() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{problems, At, Level};
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.export_clicked();
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    m.export.rows[0]
        .warnings
        .push(Warning::new(Code::DimNpot, "odd size"));
    m.invalidate_views();
    let rows = problems(&m);
    let r = rows
        .iter()
        .find(|p| p.code == Some(Code::DimNpot))
        .expect("a File row");
    assert!(matches!(r.at, At::File(_)), "{:?}", r.at);
    assert_eq!(r.level, Level::Info);
    assert_eq!(
        r.shown(),
        "Keeps a size that isn't a power of two. That's fine: it has no mips."
    );
}

/// Controller ruling 4: the Also-only case runs before the general Note rule and keeps today's row.
#[test]
fn an_also_only_input_keeps_its_row_text() {
    use engine::codes::{Code, Warning};
    let msg = format!("x_curves.png: {}; not converted", engine::pack::ALSO_ONLY);
    let (_d, m) = with_warnings(&[Warning::new(Code::PackUnusedInput, msg)]);
    let rows = sdc_gui::model::bottom::problems(&m);
    assert!(rows
        .iter()
        .any(|p| p.text == "Read only by Also export; not written on its own"));
    assert!(!rows.iter().any(|p| p.shown().contains("isn't used")));
}

/// Controller ruling 4 on a last-export File row: the Also-only case runs before the Note rule there too.
#[test]
fn an_also_only_note_from_the_last_export_keeps_its_row_text() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{problems, At, Level};
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.export_clicked();
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    let msg = format!("x_curves.png: {}; not converted", engine::pack::ALSO_ONLY);
    m.export.rows[0]
        .warnings
        .push(Warning::new(Code::PackUnusedInput, msg));
    m.invalidate_views();
    let rows = problems(&m);
    let r = rows
        .iter()
        .find(|p| p.text == "Read only by Also export; not written on its own")
        .expect("the Also-only row");
    assert!(matches!(r.at, At::File(_)), "{:?}", r.at);
    assert_eq!(
        (r.level, r.code, r.fix.is_none()),
        (Level::Info, None, true)
    );
    assert!(!rows.iter().any(|p| p.shown().contains("isn't used")));
}

/// The SHADER-EXTRA-MAP note reads the engine's `data` (`engine/src/shader.rs`: `member`, `shader`).
#[test]
fn an_extra_map_note_names_the_file_and_the_shader() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::code_text::note_sentence;
    let bare = Warning::new(Code::ShaderExtraMap, "m");
    let w = bare
        .clone()
        .with_data(serde_json::json!({"member": "lamp_p.dds", "shader": "glow"}));
    assert_eq!(
        note_sentence(&w),
        "lamp_p.dds isn't read by the glow shader. It is still converted, so nothing breaks."
    );
    assert_eq!(note_sentence(&bare), "m", "no data: the message unchanged");
}

/// A Note-class warning on a set (SHADER-EXTRA-MAP arrives there) is a note row too.
#[test]
fn a_set_note_is_a_note_row() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{counts_as_ready_problem, problems, At, Level};
    let w = Warning::new(Code::ShaderExtraMap, "m")
        .with_data(serde_json::json!({"member": "rock01_x.png", "shader": "pbr"}));
    let (_d, m) = with_set_warnings(&[w]);
    let rows = problems(&m);
    let r = rows
        .iter()
        .find(|p| p.code == Some(Code::ShaderExtraMap))
        .expect("the set's row");
    assert!(matches!(r.at, At::Set { .. }), "{:?}", r.at);
    assert_eq!(r.level, Level::Info);
    assert_eq!(
        r.shown(),
        "rock01_x.png isn't read by the pbr shader. It is still converted, so nothing breaks."
    );
    assert!(!counts_as_ready_problem(r));
}

#[test]
fn the_tab_label_grammar() {
    use sdc_gui::model::bottom::{tab_label, TabCounts as C};
    let c = |problems, hidden, notes| C {
        problems,
        hidden,
        notes,
    };
    assert_eq!(tab_label(c(0, 0, 0)), "Problems");
    assert_eq!(tab_label(c(2, 0, 0)), "Problems (2)");
    assert_eq!(tab_label(c(2, 0, 67)), "Problems (2) · Notes (67)");
    assert_eq!(tab_label(c(0, 0, 67)), "Problems (0) · Notes (67)");
    assert_eq!(tab_label(c(12, 60, 0)), "Problems (12 · 60 hidden)");
    assert_eq!(
        tab_label(c(12, 60, 5)),
        "Problems (12 · 60 hidden) · Notes (5)"
    );
}

#[test]
fn notes_leave_the_problems_list_and_show_under_the_notes_pill() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = only(&[
        Warning::new(Code::ChanNotGray, "not grey"),
        Warning::new(Code::DimNpot, "npot"),
    ]);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Problems (1) · Notes (1)");
    let grey = sdc_gui::model::code_text::code_sentence(Code::ChanNotGray);
    let npot = "Keeps a size that isn't a power of two. That's fine: it has no mips.";
    assert_eq!(h.query_all_by_label(grey).count(), 1);
    assert_eq!(h.query_all_by_label(npot).count(), 0);
    h.get_by_label("Notes (1)").click();
    h.run();
    assert_eq!(h.query_all_by_label(npot).count(), 1);
    assert_eq!(h.query_all_by_label(grey).count(), 0);
}

/// Controller ruling 5: a note with a button stays in the default list, after the problems, and never counts.
#[test]
fn a_note_with_a_button_stays_in_the_list_and_never_counts() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    // The same fixture without the note: the note must leave this count as it is.
    let ready = sdc_gui::model::bottom::flagged(&m).ready_rows;
    m.unmatched_keys = vec!["gone.png".into()];
    m.invalidate_views();
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Problems (1) · Notes (1)");
    let grey = h
        .get_by_label(sdc_gui::model::code_text::code_sentence(Code::ChanNotGray))
        .rect();
    let mode = sdc_gui::model::code_text::target_name(h.state().target);
    let note = h
        .get_by_label(&format!(
            "1 saved setting names no file in this mode ({mode})"
        ))
        .rect();
    assert!(note.top() > grey.top(), "the note comes after the problem");
    assert_eq!(
        sdc_gui::model::bottom::flagged(h.state()).ready_rows,
        ready,
        "the note changes no ready count"
    );
}

#[test]
fn with_only_notes_the_list_says_no_problems_and_names_the_notes() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = only(&[Warning::new(Code::DimNpot, "npot")]);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Problems (0) · Notes (1)");
    h.get_by_label("No problems. Notes (1) lists what Tessalith did for you.");
}

/// Controller ruling 12: the cache is counted in row scans, not in `problems()` lookups.
#[test]
fn the_tab_counts_are_cached_at_10k() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::tab_count_scans;
    let d = tempfile::tempdir().unwrap();
    let mut m = many(d.path(), 10_000);
    for (n, i) in m.items.iter_mut().enumerate() {
        let code = if n % 2 == 0 {
            Code::ChanNotGray
        } else {
            Code::DimNpot
        };
        i.warnings = vec![Warning::new(code, "w")];
    }
    m.invalidate_views();
    m.open_problems(None);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .with_max_steps(16)
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    let settled = tab_count_scans(h.state());
    assert!(settled >= 1);
    for _ in 0..10 {
        h.step();
    }
    assert_eq!(
        tab_count_scans(h.state()),
        settled,
        "ten settled frames scan no row"
    );
    h.state_mut().set_kind_hidden(Code::ChanNotGray, true);
    h.step();
    assert_eq!(
        tab_count_scans(h.state()),
        settled + 1,
        "one change, one scan"
    );
}

/// The cache key's other half: a new `problems()` build with the same hidden kinds is counted again. The build is
/// made under the same view key (only `evidence_rev` moves), so the view cache keeps the old counts beside it.
#[test]
fn a_new_problems_build_is_counted_again() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{cached_tab_counts, TabCounts};
    let (_d, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    let one = TabCounts {
        problems: 1,
        hidden: 0,
        notes: 0,
    };
    assert_eq!(cached_tab_counts(&m), one);
    m.items[0]
        .warnings
        .push(Warning::new(Code::DimNpot, "npot"));
    m.evidence_rev += 1;
    assert_eq!(cached_tab_counts(&m), TabCounts { notes: 1, ..one });
}

/// Review E2 fix 1: under a set filter the Notes pill and the first line count the set's notes, which the pill opens;
/// the tab keeps the project's count. Here: one note on rock01's member, one on the project.
#[test]
fn under_a_set_filter_the_notes_pill_counts_the_sets_notes() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{cached_tab_counts, ProblemsFilter};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::DimNpot, "npot")]);
    m.unmatched_keys = vec!["gone.png".into()];
    m.invalidate_views();
    assert_eq!(cached_tab_counts(&m).notes, 2, "the project's notes");
    let key = rock01(&m).key;
    m.open_problems(Some(ProblemsFilter::Set(key)));
    let mut h = harness(m);
    h.run();
    h.get_by_label("Notes (1)");
    assert_eq!(h.query_all_by_label("Notes (2)").count(), 0);
    h.get_by_label("No problems. Notes (1) lists what Tessalith did for you.");
}

/// Review E2 fix 1: a set with no note shows no Notes pill and the plain empty line, whatever the project has.
#[test]
fn a_set_with_no_notes_shows_no_notes_pill() {
    use sdc_gui::model::bottom::{cached_tab_counts, ProblemsFilter};
    let (_d, mut m) = with_warnings(&[]);
    m.unmatched_keys = vec!["gone.png".into()];
    m.invalidate_views();
    assert_eq!(cached_tab_counts(&m).notes, 1, "the project's note");
    let key = rock01(&m).key;
    m.open_problems(Some(ProblemsFilter::Set(key)));
    let mut h = harness(m);
    h.run();
    assert_eq!(h.query_all_by_label("Notes (1)").count(), 0, "no pill");
    h.get_by_label("No problems");
}

/// Review E2 fix 2: with the set's notes listed, the Checks pill counts the set's checks, which its click opens. The
/// project's one check is a last-export File row, in no set.
#[test]
fn under_a_sets_notes_the_checks_pill_counts_the_sets_checks() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{filtered, flagged, ProblemsFilter};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    decide_all(&mut m);
    m.export_clicked();
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    m.export.rows[0]
        .warnings
        .push(Warning::new(Code::ChkLowDetail, "low detail"));
    m.invalidate_views();
    let key = rock01(&m).key;
    assert!(flagged(&m).check_rows > 0, "the project has a check");
    let set_checks = ProblemsFilter::SetChecks(key.clone());
    assert!(
        filtered(&m, Some(&set_checks)).is_empty(),
        "rock01 has none"
    );
    m.open_problems(Some(ProblemsFilter::SetNotes(key)));
    let mut h = harness(m);
    h.run();
    assert!(h
        .query_by_label(sdc_gui::ui::problems::CHECKS_PILL)
        .is_none());
}

/// Review E2 fix 6 and the pill ruling: an empty Notes list says so, and its pill stays to turn the filter off.
#[test]
fn an_empty_notes_list_says_no_notes_and_its_pill_turns_it_off() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::ProblemsFilter;
    let (_d, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    m.open_problems(Some(ProblemsFilter::Notes));
    let mut h = harness(m);
    h.run();
    h.get_by_label("No notes");
    h.get_by_label("Notes (0)").click();
    h.run();
    assert_eq!(h.state().bottom.filter, None);
}

/// A `SetsDoc` over `sdc-project.toml` holding `text`.
fn doc_with(text: &str) -> (tempfile::TempDir, sdc_gui::sets_doc::SetsDoc) {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("sdc-project.toml"), text).unwrap();
    let doc = sdc_gui::sets_doc::SetsDoc::open(d.path());
    (d, doc)
}

fn key(code: &str, subject: &str, sentence: &str) -> sdc_gui::sets_doc::IntendedKey {
    sdc_gui::sets_doc::IntendedKey {
        code: code.into(),
        subject: subject.into(),
        sentence: sentence.into(),
    }
}

#[test]
fn a_mark_is_written_as_an_inline_table_in_project() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let doc = m.sets_doc.as_mut().unwrap();
    let k = key("SHADER-MISSING-MAP", "set:landscape/rock01", "no _n");
    doc.set_intended(&k, true);
    let text = doc.text();
    assert!(
        text.contains(r#"intended = [{ code = "SHADER-MISSING-MAP", subject = "set:landscape/rock01", sentence = "no _n" }]"#),
        "{text}"
    );
    assert!(doc.intended_keys().contains(&k));
    doc.set_intended(&k, false);
    assert!(!doc.text().contains("intended"));
    assert_eq!(
        sdc_gui::sets_doc::normalise_sentence("texture:x.png", "  a \t b\n c "),
        "a b c"
    );
}

/// Ruling 1 (spec 7a, Controller rulings for Plan 3a): a set row's own id in front of its sentence is left out of
/// the key, so the mark follows a rename. The PGPatcher pitfall rows are written `"{set id}: {pitfall}"`
/// (`model/pbr_export.rs:606`).
#[test]
fn a_set_rows_own_id_prefix_is_left_out_of_the_key() {
    use sdc_gui::sets_doc::normalise_sentence as n;
    let s = "landscape/rock01: Delete wins; the other alpha keys of this entry do nothing";
    assert_eq!(
        n("set:landscape/rock01", s),
        "Delete wins; the other alpha keys of this entry do nothing"
    );
    assert_eq!(
        n("set:Landscape/Rock01", s),
        "Delete wins; the other alpha keys of this entry do nothing",
        "ids compare as `same_id`"
    );
    assert_eq!(n("set:landscape/rock02", s), s, "another set's id stays");
    assert_eq!(
        n("texture:landscape/rock01.png", s),
        s,
        "only a set subject drops a prefix"
    );
    assert_eq!(
        n(
            "set:landscape/rock01",
            "alphaprop_flags: only changes shapes"
        ),
        "alphaprop_flags: only changes shapes",
        "a sentence that starts with another word is untouched"
    );
    assert_eq!(
        n("set:landscape/rock01", "landscape/rock01 is ready"),
        "landscape/rock01 is ready",
        "the id without \": \" stays"
    );
}

#[test]
fn a_mark_follows_a_renamed_set_and_leaves_with_a_deleted_one() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let doc = m.sets_doc.as_mut().unwrap();
    doc.set_intended(
        &key(
            "SET-COMPANION-DDS",
            "set:landscape/rock01",
            "a companion is a DDS",
        ),
        true,
    );
    doc.rename_set("landscape/rock01", "landscape/rock02");
    assert!(doc.intended_keys().contains(&key(
        "SET-COMPANION-DDS",
        "set:landscape/rock02",
        "a companion is a DDS"
    )));
    doc.delete_set("landscape/rock02", &[]);
    assert!(doc.intended().is_empty());
}

/// The engine's automatic retarget moves a mark too. `SetsDoc::retarget_auto(from, to)`: its two branches that
/// change the id go through `rename_set` or `rename_refs`. This call takes the third branch: `opened` writes no
/// `[[sets]]` entry (`support/mod.rs` `opened` only plans the folder and chooses the export folder), so there is no
/// entry for the first branch (needs `renamed_from`) or the second (needs an entry with id `from`), and none created
/// or renamed at `from`, so it calls `rename_refs("landscape/rock01", "landscape/rock09")`.
#[test]
fn a_mark_follows_an_automatic_retarget() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let doc = m.sets_doc.as_mut().unwrap();
    doc.set_intended(
        &key("SHADER-MISSING-MAP", "set:landscape/rock01", "s"),
        true,
    );
    doc.retarget_auto("landscape/rock01", "landscape/rock09");
    assert_eq!(
        doc.intended(),
        vec![key("SHADER-MISSING-MAP", "set:landscape/rock09", "s")]
    );
}

/// Controller ruling 9: a rewrite keeps entries the app can't read, in place, and never drops a stale mark.
#[test]
fn hand_written_and_stale_marks_survive_a_rewrite() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("sdc-project.toml"),
        concat!(
            "[project]\nintended = [\n",
            "  42,\n",
            "  { code = \"X\" },\n",
            "  { code = \"VAL-MIP-ERROR\", subject = \"texture:gone.png\", sentence = \"s\" },\n",
            "]\n",
        ),
    )
    .unwrap();
    let mut doc = sdc_gui::sets_doc::SetsDoc::open(d.path());
    assert!(!doc.read_only());
    assert_eq!(
        doc.intended(),
        vec![key("VAL-MIP-ERROR", "texture:gone.png", "s")],
        "unreadable entries are skipped on read"
    );
    let k = key("CHAN-NOT-GRAY", "texture:landscape/rock01.png", "t");
    doc.set_intended(&k, true);
    assert!(
        doc.text()
            .contains("sentence = \"s\" },\n  { code = \"CHAN-NOT-GRAY\""),
        "a new mark takes its own line in a one-per-line array:\n{}",
        doc.text()
    );
    doc.set_intended(&k, false);
    let t = doc.text();
    let at = |s: &str| t.find(s).unwrap_or_else(|| panic!("{s} lost:\n{t}"));
    assert!(at("42") < at("{ code = \"X\" }"));
    assert!(
        at("{ code = \"X\" }") < at("texture:gone.png"),
        "kept in place, stale mark kept"
    );
}

/// A mark set as it already is makes no edit: no new revision, the same text.
#[test]
fn a_mark_set_as_it_already_is_is_no_edit() {
    let (_d, mut doc) = doc_with("[project]\n");
    let k = key("VAL-MIP-ERROR", "texture:a.png", "m");
    let (rev, text) = (doc.rev(), doc.text());
    doc.set_intended(&k, false);
    assert_eq!((doc.rev(), doc.text()), (rev, text), "unmark with no key");
    doc.set_intended(&k, true);
    let (rev, text) = (doc.rev(), doc.text());
    doc.set_intended(&k, true);
    assert_eq!((doc.rev(), doc.text()), (rev, text), "mark twice");
}

#[test]
fn a_read_only_document_takes_no_mark() {
    let (_d, mut doc) = doc_with("[[sets]\nbroken");
    assert!(doc.read_only());
    assert!(!doc.intended_editable());
    let text = doc.text();
    doc.set_intended(&key("VAL-MIP-ERROR", "texture:a.png", "m"), true);
    assert_eq!(doc.text(), text);
    assert!(doc.intended().is_empty());
}

/// Review E2 fix round 1: an `intended` that is not an array is left as written, and `intended_editable` says so
/// (Task 4 disables its button on it).
#[test]
fn an_intended_that_is_not_an_array_is_not_editable() {
    let (_d, mut doc) = doc_with("[project]\nintended = \"x\"\n");
    assert!(!doc.intended_editable());
    let text = doc.text();
    doc.set_intended(&key("VAL-MIP-ERROR", "texture:a.png", "m"), true);
    assert_eq!(doc.text(), text);
    assert!(doc_with("[project]\n").1.intended_editable(), "no key");
    assert!(
        doc_with("[project]\nintended = []\n").1.intended_editable(),
        "an array"
    );
}

const A: &str = "set:landscape/rock01";
const B: &str = "set:landscape/rock02";
const T: &str = "texture:landscape/rock01.png";

fn three_marks() -> (tempfile::TempDir, sdc_gui::sets_doc::SetsDoc) {
    let (d, mut doc) = doc_with("[project]\n");
    for s in [A, B, T] {
        doc.set_intended(&key("SHADER-MISSING-MAP", s, "s"), true);
    }
    (d, doc)
}

/// Deleting or renaming one set touches its own marks only; the others stay, in place.
#[test]
fn delete_and_rename_touch_only_that_sets_marks() {
    let (_d, mut doc) = three_marks();
    doc.delete_set("landscape/rock01", &[]);
    assert_eq!(
        doc.intended(),
        vec![
            key("SHADER-MISSING-MAP", B, "s"),
            key("SHADER-MISSING-MAP", T, "s")
        ]
    );
    let (_d, mut doc) = three_marks();
    doc.rename_set("landscape/rock01", "landscape/rock07");
    assert_eq!(
        doc.intended(),
        vec![
            key("SHADER-MISSING-MAP", "set:landscape/rock07", "s"),
            key("SHADER-MISSING-MAP", B, "s"),
            key("SHADER-MISSING-MAP", T, "s")
        ]
    );
}

/// Set ids compare folded (Note BF): a mark stored with another case follows a rename and is found.
#[test]
fn a_set_mark_matches_its_id_in_any_case() {
    let (_d, mut doc) = doc_with(
        "[project]\nintended = [{ code = \"C\", subject = \"set:Landscape/Rock01\", sentence = \"s\" }]\n",
    );
    assert!(doc.intended_keys().contains(&key("C", A, "s")));
    doc.rename_set("landscape/rock01", "landscape/rock05");
    assert_eq!(doc.intended(), vec![key("C", "set:landscape/rock05", "s")]);
}

/// Taking the first mark off a one-line array leaves no space after `[`; a one-per-line array keeps its lines.
#[test]
fn removing_the_first_mark_leaves_a_tidy_array() {
    let (_d, mut doc) = doc_with("[project]\n");
    let (k1, k2) = (key("C1", T, "a"), key("C2", T, "b"));
    doc.set_intended(&k1, true);
    doc.set_intended(&k2, true);
    doc.set_intended(&k1, false);
    assert!(
        doc.text().contains("intended = [{ code = \"C2\""),
        "{}",
        doc.text()
    );
    let (_d, mut doc) = doc_with(concat!(
        "[project]\nintended = [\n",
        "  { code = \"C1\", subject = \"texture:landscape/rock01.png\", sentence = \"a\" },\n",
        "  { code = \"C2\", subject = \"texture:landscape/rock01.png\", sentence = \"b\" },\n",
        "]\n",
    ));
    doc.set_intended(&k1, false);
    assert!(
        doc.text().contains("intended = [\n  { code = \"C2\""),
        "{}",
        doc.text()
    );
}

/// Review E2 acceptance: every counted row has a button that removes it from the count.
#[test]
fn every_problem_row_has_a_fix_or_mark_as_intended_and_marks_reach_zero() {
    use engine::codes::{Code, Warning};
    let ws = [
        Warning::new(Code::SugContradiction, "name vs content"),
        Warning::new(Code::ValMipError, "mip 3"),
        Warning::new(Code::DimNpot, "npot"),
    ];
    let (_d, mut m) = with_warnings(&ws);
    let (b, ready) = baseline();
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_label(&label(b, 2, 0, 1));
    for code in [Code::SugContradiction, Code::ValMipError] {
        let sentence = sdc_gui::model::code_text::code_sentence(code);
        on_row(&h, sentence, "Mark as intended: ").click();
        h.run();
    }
    h.get_by_label(&label(b, 0, 0, 3));
    assert_eq!(
        sdc_gui::model::bottom::set_ready(h.state(), &rock01(h.state())),
        ready,
        "back to the fixture's own readiness"
    );
    h.state_mut().undo();
    // `undo` plans again, and a plan replaces injected warnings: wait for it, then inject them again.
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    inject(h.state_mut(), &ws);
    h.run();
    h.get_by_label(&label(b, 1, 0, 2));
    h.get_by_label(&format!("Notes ({})", b.notes + 2)).click();
    h.run();
    h.query_all_by_label_contains("Undo mark: ")
        .next()
        .unwrap()
        .click();
    h.run();
    h.get_by_label(&label(b, 2, 0, 1));
}

/// Controller ruling 5: a marked row leaves the default list even when it has a fix.
#[test]
fn a_marked_row_with_a_fix_leaves_the_default_list() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{filtered, problems};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ChanNormalLength, "short")]);
    let p = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::ChanNormalLength))
        .cloned()
        .unwrap();
    assert!(p.fix.is_some(), "Rebuild Z");
    m.mark_intended(p.mark.clone().unwrap(), true, "row");
    assert!(!filtered(&m, None)
        .iter()
        .any(|r| r.code == Some(Code::ChanNormalLength)));
    let notes = filtered(&m, Some(&sdc_gui::model::bottom::ProblemsFilter::Notes));
    assert!(notes
        .iter()
        .any(|r| r.code == Some(Code::ChanNormalLength) && r.intended));
}

#[test]
fn two_rows_of_one_code_are_marked_one_at_a_time() {
    use engine::codes::{Code, Warning};
    use serde_json::json;
    let (_d, mut m) = with_set_warnings(&[
        Warning::new(Code::ShaderMissingMap, "no _n")
            .with_data(json!({"map": "_n", "shader": "pbr"})),
        Warning::new(Code::ShaderMissingMap, "no _rmaos")
            .with_data(json!({"map": "_rmaos", "shader": "pbr"})),
    ]);
    let rows = |m: &AppModel| {
        sdc_gui::model::bottom::problems(m)
            .iter()
            .filter(|p| p.code == Some(Code::ShaderMissingMap))
            .map(|p| p.intended)
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(&m), [false, false]);
    let p = sdc_gui::model::bottom::problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::ShaderMissingMap))
        .cloned()
        .unwrap();
    m.mark_intended(p.mark.clone().unwrap(), true, "row");
    let mut now = rows(&m);
    now.sort();
    assert_eq!(now, [false, true], "the other sentence is still a problem");
}

#[test]
fn mark_as_intended_is_off_on_a_read_only_project() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ChanNotGray, "not grey")]);
    // The reload reads `sdc.toml` again too: saved, rock01's members stay decided and keep their warning rows.
    assert!(m.save().is_ok());
    let file = m.root.clone().unwrap().join("sdc-project.toml");
    // An unreadable file makes SetsDoc read-only (`sets_doc.rs:375`, P8).
    std::fs::write(&file, "[project\n").unwrap();
    m.reload_documents();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    inject(&mut m, &[Warning::new(Code::ChanNotGray, "not grey")]);
    assert!(m.sets_read_only_reason().is_some());
    let before = std::fs::read(&file).unwrap();
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    let b = h
        .query_all_by_label_contains("Mark as intended: ")
        .next()
        .unwrap();
    assert!(b.accesskit_node().is_disabled());
    b.click();
    h.run();
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

/// Controller resolution for Task 4: an `intended` that is not an array turns Mark off with its own reason, and a
/// click changes nothing (`SetsDoc::intended_editable`).
#[test]
fn mark_as_intended_is_off_when_intended_is_not_a_list() {
    use engine::codes::{Code, Warning};
    const WHY: &str =
        "sdc-project.toml's intended entry isn't a list: fix or delete it in the file first";
    let ws = [Warning::new(Code::ChanNotGray, "not grey")];
    let (_d, mut m) = with_warnings(&ws);
    // As above: saved, so the reload keeps rock01's members decided.
    assert!(m.save().is_ok());
    let file = m.root.clone().unwrap().join("sdc-project.toml");
    std::fs::write(&file, "[project]\nintended = \"x\"\n").unwrap();
    m.reload_documents();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    inject(&mut m, &ws);
    assert_eq!(m.sets_read_only_reason(), None, "the file reads");
    assert_eq!(m.mark_off_reason().as_deref(), Some(WHY));
    let before = std::fs::read(&file).unwrap();
    let text = m.sets_doc.as_ref().unwrap().text();
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    let sentence = sdc_gui::model::code_text::code_sentence(Code::ChanNotGray);
    let b = on_row(&h, sentence, "Mark as intended: ");
    assert!(b.accesskit_node().is_disabled());
    assert_eq!(h.query_all_by_label(WHY).count(), 0, "no hover yet");
    b.hover();
    h.run();
    assert!(h.query_all_by_label(WHY).count() > 0, "the disabled hover");
    on_row(&h, sentence, "Mark as intended: ").click();
    h.run();
    assert_eq!(std::fs::read(&file).unwrap(), before);
    assert_eq!(h.state().sets_doc.as_ref().unwrap().text(), text);
    assert!(!h.state().can_undo(), "no undo step");
}

/// Controller ruling 7: Mark is for warnings only. An Error-severity code on a last-export File row is at
/// `Level::Warning` today and still gets no Mark.
#[test]
fn an_error_row_cannot_be_marked() {
    use engine::codes::Code;
    use sdc_gui::model::bottom::{markable, Level};
    assert!(!markable(Level::Warning, Some(Code::ValHeader)));
    assert!(!markable(Level::Warning, Some(Code::ValAlphaGuarantee)));
    assert!(markable(Level::Warning, Some(Code::ValMipError)));
    assert!(markable(Level::Warning, None));
    assert!(markable(Level::Check, Some(Code::ChkLowDetail)));
    assert!(!markable(Level::Blocks, None));
    assert!(!markable(Level::Info, Some(Code::DimNpot)));
}

/// Ruling 1 (spec 7a, Controller rulings for Plan 3a): a mark follows a rename. Run on the real planned warning: rock01 as plain
/// `pbr` with its normal taken out raises SHADER-MISSING-MAP, whose engine text names the shader, not the set.
#[test]
fn a_mark_survives_a_rename() {
    use engine::codes::Code;
    use sdc_gui::model::bottom::{problems, At};
    use sdc_gui::model::sets::{set_views, slot_of, SlotKind};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    let n = rock01(&m)
        .members
        .iter()
        .find(|k| m.item(k).and_then(slot_of) == Some(SlotKind::Normal))
        .cloned()
        .unwrap();
    m.remove_member("landscape/rock01", &n).unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let missing = |m: &AppModel, id: &str| {
        problems(m)
            .iter()
            .find(|p| {
                p.code == Some(Code::ShaderMissingMap)
                    && matches!(&p.at, At::Set { id: i, .. } if i == id)
            })
            .cloned()
    };
    let p = missing(&m, "landscape/rock01")
        .expect("precondition: plain pbr with no normal raises SHADER-MISSING-MAP");
    assert!(
        !p.shown().contains("landscape/rock01"),
        "precondition: the engine text does not name the set id"
    );
    m.mark_intended(p.mark.clone().unwrap(), true, "row");
    m.rename_set("landscape/rock01", "landscape/rock02")
        .unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(set_views(&m).iter().any(|v| v.id == "landscape/rock02"));
    let row = missing(&m, "landscape/rock02").expect("the planned row is back after the rename");
    assert_eq!(row.shown(), p.shown(), "same sentence");
    assert!(
        row.intended && row.is_note(),
        "the mark followed the rename"
    );
}

/// Ruling 1 for a row whose text starts with its set id: a PGPatcher pitfall, `"{set id}: {pitfall}"`
/// (`model/pbr_export.rs:606`). The fixture raises it for real, as `json_errors.rs:27-35` does for the leaf:
/// `alphaprop_delete` with `alphaprop_thresh` gives "Delete wins; …" (`pgp/prefix.rs:440-446`). No injection.
#[test]
fn a_marked_pgpatcher_pitfall_survives_a_rename() {
    use sdc_gui::model::bottom::{problems, At};
    use sdc_gui::model::sets::set_views;
    const PITFALL: &str = "Delete wins; the other alpha keys of this entry do nothing";
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_json_value(
        "landscape/rock01",
        "alphaprop_delete",
        Some(serde_json::json!(true)),
    );
    m.set_json_value(
        "landscape/rock01",
        "alphaprop_thresh",
        Some(serde_json::json!(10)),
    );
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let pitfall = |m: &AppModel, id: &str| {
        problems(m)
            .iter()
            .find(|p| {
                p.code.is_none()
                    && p.text.ends_with(PITFALL)
                    && matches!(&p.at, At::Set { id: i, .. } if i == id)
            })
            .cloned()
    };
    let p = pitfall(&m, "landscape/rock01")
        .expect("precondition: rock01's JSON entry raises the Delete-wins pitfall");
    assert_eq!(
        p.shown(),
        format!("landscape/rock01: {PITFALL}"),
        "precondition: the row text starts with the set id"
    );
    assert_eq!(
        p.mark.as_ref().unwrap().sentence,
        PITFALL,
        "the key leaves the set id out"
    );
    m.mark_intended(p.mark.clone().unwrap(), true, "row");
    m.rename_set("landscape/rock01", "landscape/rock02")
        .unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(set_views(&m).iter().any(|v| v.id == "landscape/rock02"));
    let row =
        pitfall(&m, "landscape/rock02").expect("the JSON entry and its pitfall follow the rename");
    assert_eq!(
        row.shown(),
        format!("landscape/rock02: {PITFALL}"),
        "the shown text names the new id"
    );
    assert!(
        row.intended && row.is_note(),
        "the mark followed the rename"
    );
}

/// A row whose sentence really changes shows again (spec 7a.3).
#[test]
fn a_changed_sentence_shows_again() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::problems;
    let missing = |map: &str| {
        Warning::new(
            Code::ShaderMissingMap,
            format!("the pbr shader needs {map}; none is in the set (nothing is generated)"),
        )
        .with_data(serde_json::json!({"map": map, "shader": "pbr"}))
    };
    let (_d, mut m) = with_set_warnings(&[missing("_n")]);
    let p = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::ShaderMissingMap))
        .cloned()
        .unwrap();
    m.mark_intended(p.mark.clone().unwrap(), true, "row");
    inject_set(&mut m, &[missing("_rmaos")]);
    let row = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::ShaderMissingMap))
        .cloned()
        .unwrap();
    assert!(
        !row.intended && !row.is_note(),
        "a changed sentence shows again"
    );
}

/// Controller ruling 3 and review B: DERIVE-NIF-SUGGEST (Info severity) still makes its last-export File row, at
/// Warning level, with Mark.
#[test]
fn a_nif_suggestion_row_is_a_problem_with_mark() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{problems, At, Level};
    let (_d, mut m) = with_warnings(&[]);
    m.export.rows = vec![sdc_gui::model::export::Row {
        job_key: "derived/x".into(),
        path: "derived/x".into(),
        state: sdc_gui::model::export::RowState::Done { passed: true },
        warnings: vec![Warning::new(
            Code::DeriveNifSuggest,
            "NiAlphaProperty flags 4845",
        )],
    }];
    m.invalidate_views();
    let r = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::DeriveNifSuggest))
        .cloned()
        .expect("the row exists");
    assert!(matches!(r.at, At::File(_)));
    assert_eq!(r.level, Level::Warning);
    assert!(r.mark.is_some());
}

/// Review round 2: with no project open, Mark is off and says why.
#[test]
fn mark_is_off_with_no_project_open() {
    let m = AppModel::new(sdc_gui::settings::GuiSettings::default(), None, session());
    assert_eq!(
        m.mark_off_reason().as_deref(),
        Some("Open a project folder first")
    );
}

/// Controller ruling 12: one mark costs one tab-count scan.
#[test]
fn a_mark_costs_one_tab_count_scan() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{problems, tab_count_scans};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    let before = tab_count_scans(h.state());
    let p = problems(h.state())
        .iter()
        .find(|p| p.code == Some(Code::ValMipError))
        .cloned()
        .unwrap();
    h.state_mut()
        .mark_intended(p.mark.clone().unwrap(), true, "row");
    for _ in 0..5 {
        h.step();
    }
    assert_eq!(tab_count_scans(h.state()), before + 1);
}

/// `counts_for` (the tiles' and cards' predicate): a problem warning counts until it is marked, with the same sentence
/// its row shows; an artist check never counts against Ready (Note CL, spec 7a.3).
#[test]
fn counts_for_drops_a_marked_warning_and_every_check() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{counts_for, problems, At};
    let w = Warning::new(Code::ValMipError, "mip 3");
    let (_d, mut m) = with_warnings(std::slice::from_ref(&w));
    let k = rock01(&m).members[0].clone();
    let at = At::Texture(k.clone());
    assert!(counts_for(&m, &at, &w));
    assert!(!counts_for(
        &m,
        &At::Texture(k),
        &Warning::new(Code::ChkLowDetail, "x")
    ));
    let p = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::ValMipError))
        .cloned()
        .unwrap();
    m.mark_intended(p.mark.clone().unwrap(), true, "row");
    assert!(!counts_for(&m, &at, &w), "marked");
    assert!(
        counts_for(&m, &at, &Warning::new(Code::ChanNotGray, "not grey")),
        "another sentence still counts"
    );
}

/// Set ids compare folded (Note BF): a row whose set id differs only in case finds the set's mark.
#[test]
fn a_set_mark_is_found_under_its_id_in_any_case() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{counts_for, intended_key, At};
    let w = Warning::new(Code::ValMipError, "mip 3");
    let (_d, mut m) = with_warnings(&[]);
    let at = |id: &str| At::Set {
        key: "k".into(),
        id: id.into(),
    };
    let shown = sdc_gui::model::code_text::shown_for(Some(w.code), &w.message);
    let k = intended_key(&at("landscape/rock01"), w.code.as_str(), &shown);
    m.mark_intended(k, true, "row");
    assert!(!counts_for(&m, &at("landscape/rock01"), &w));
    assert!(!counts_for(&m, &at("Landscape/Rock01"), &w), "folded");
}
