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
        hidden_kinds: b.hidden_kinds,
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
        hidden_kinds: 0,
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
        hidden_kinds: 0,
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

/// Review E18: the hidden kinds' first line, as the list's line above its rows.
fn hidden_line(kinds: usize, rows: usize) -> String {
    use sdc_gui::model::code_text::counted;
    format!(
        "{} hidden in this project ({}). Hidden rows still count for Ready.",
        counted(kinds, "kind", "kinds"),
        counted(rows, "row", "rows")
    )
}

const ALL_HIDDEN: &str = "Every problem here is hidden. Open Kinds ▾ to show them.";

/// Review E18: with every problem kind hidden the list names the scope (this project) and Ready is unchanged.
#[test]
fn hiding_every_problem_kind_names_the_scope_and_keeps_ready() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = with_warnings(&[
        Warning::new(Code::ChanNotGray, "a"),
        Warning::new(Code::ValMipError, "b"),
        Warning::new(Code::ValMipError, "c"),
    ]);
    let ready = sdc_gui::model::bottom::flagged(&m).ready_rows;
    let header = m.header_export_text();
    let clean = m.export_is_clean();
    let (b, _) = baseline();
    m.set_kind_hidden(Code::ChanNotGray, true);
    m.set_kind_hidden(Code::ValMipError, true);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_label(&label(b, 0, 3, 0));
    h.get_by_label("2 kinds hidden in this project (3 rows). Hidden rows still count for Ready.");
    assert_eq!(sdc_gui::model::bottom::flagged(h.state()).ready_rows, ready);
    assert_eq!(h.state().header_export_text(), header);
    assert_eq!(h.state().export_is_clean(), clean);
}

/// Review E18 (Task 2's review, Minor 2): every problem hidden and a note with a button still listed. The list is not
/// empty and problems exist, so "No problems." is not drawn; the hidden line is.
#[test]
fn with_every_problem_hidden_a_listed_note_never_reads_no_problems() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    m.unmatched_keys = vec!["gone.png".into()];
    m.invalidate_views();
    m.set_kind_hidden(Code::ChanNotGray, true);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Problems (0 · 1 hidden) · Notes (1)");
    assert_eq!(
        h.query_all_by_label_contains("No problems").count(),
        0,
        "problems exist, hidden"
    );
    h.get_by_label(&hidden_line(1, 1));
    assert_eq!(
        h.query_all_by_label(ALL_HIDDEN).count(),
        0,
        "a row is listed"
    );
}

/// Review E18 (controller ruling 6): a list empty because its rows are hidden says how to show them; a list empty
/// with nothing hidden keeps "No problems".
#[test]
fn an_empty_list_says_why_only_when_rows_are_hidden() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    m.set_kind_hidden(Code::ChanNotGray, true);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_label(&hidden_line(1, 1));
    h.get_by_label(ALL_HIDDEN);
    assert_eq!(h.query_all_by_label("No problems").count(), 0);

    let (_d, mut m) = only(&[]);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_label("No problems");
    assert_eq!(h.query_all_by_label(ALL_HIDDEN).count(), 0);
    assert_eq!(
        h.query_all_by_label_contains("hidden in this project")
            .count(),
        0,
        "nothing hidden"
    );
}

/// Review E18 (controller ruling 5): under a Set filter the line keeps the project's numbers ("in this project"),
/// never the set's. Here the set's row is hidden, and one more of the kind's rows is a File row in no set.
#[test]
fn under_a_set_filter_the_hidden_line_gives_the_projects_numbers() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{cached_tab_counts, ProblemsFilter};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ChanNotGray, "not grey")]);
    let (b, _) = baseline();
    let set = rock01(&m);
    // A last-export File row of a job in no set.
    m.export.rows = vec![export_row(
        "derived/x",
        vec![Warning::new(Code::ChanNotGray, "not grey")],
    )];
    m.invalidate_views();
    m.set_kind_hidden(Code::ChanNotGray, true);
    let project = cached_tab_counts(&m).hidden;
    assert_eq!(project, b.hidden + 2, "rock01's row and the File row");
    m.open_problems(Some(ProblemsFilter::Set(set.key)));
    let mut h = harness(m);
    h.run();
    h.get_by_label(&hidden_line(1, project));
    assert_eq!(
        h.query_all_by_label(&hidden_line(1, 1)).count(),
        0,
        "not the set's own number"
    );
}

/// Review E18 (controller ruling 10): at a narrow window the hidden line wraps inside the window; its node keeps the
/// full text.
#[test]
fn the_hidden_line_wraps_at_a_narrow_window() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    m.set_kind_hidden(Code::ChanNotGray, true);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    let one_line = h.get_by_label(&hidden_line(1, 1)).rect().height();
    // The line is about 400 px on one line.
    let w = 360.0;
    h.set_size(egui::vec2(w, 900.0));
    h.run();
    let line = h.get_by_label(&hidden_line(1, 1)).rect();
    let panel = support::panel(&h.ctx, "bottom");
    assert!(
        line.max.x <= panel.max.x.min(w),
        "the line ends at {} past the panel {panel:?}",
        line.max.x
    );
    assert!(
        line.height() > 1.5 * one_line,
        "the line wraps: {line:?}, one line is {one_line}"
    );
}

/// Review E18 fix round 1, item 2: `k` counts the kinds that hide a problem row, so `k` kinds account for the `r`
/// rows. A hidden kind with no row now is not in it.
#[test]
fn a_hidden_kind_with_no_row_is_not_counted_in_the_line() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    m.set_kind_hidden(Code::ChanNotGray, true);
    m.set_kind_hidden(Code::ValMipError, true);
    assert_eq!(
        m.hidden_kinds().len(),
        2,
        "two kinds hidden, one with a row"
    );
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    assert_eq!(
        h.query_all_by_label(&hidden_line(2, 1)).count(),
        0,
        "a kind with no row is not counted"
    );
    h.get_by_label(&hidden_line(1, 1));
}

/// Review E18 fix round 1, item 4: when only a hidden note empties the default list there is no problem row hidden
/// (`r` = 0), so no hidden line and the plain hidden-rows text.
#[test]
fn only_a_hidden_note_draws_no_hidden_line() {
    use engine::codes::Code;
    let d = tempfile::tempdir().unwrap();
    let mut m = many(d.path(), 1);
    m.unmatched_keys = vec!["gone.png".into()];
    m.invalidate_views();
    m.set_kind_hidden(Code::CfgFileUnmatched, true);
    assert_eq!(
        sdc_gui::model::bottom::cached_tab_counts(&m).hidden,
        0,
        "a note is no hidden problem row"
    );
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Every row here is hidden — see Kinds");
    assert_eq!(
        h.query_all_by_label_contains("hidden in this project")
            .count(),
        0
    );
    assert_eq!(h.query_all_by_label(ALL_HIDDEN).count(), 0);
}

/// Review E18 fix round 1, item 4: under a Set filter, a set with nothing of its own hidden keeps its true line while
/// a row elsewhere in the project is hidden.
#[test]
fn a_set_with_nothing_hidden_keeps_no_problems_and_its_notes() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{cached_tab_counts, ProblemsFilter};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::DimNpot, "npot")]);
    let set = rock01(&m);
    m.export.rows = vec![export_row(
        "derived/x",
        vec![Warning::new(Code::ChanNotGray, "not grey")],
    )];
    m.invalidate_views();
    m.set_kind_hidden(Code::ChanNotGray, true);
    assert!(cached_tab_counts(&m).hidden > 0, "the project hides a row");
    m.open_problems(Some(ProblemsFilter::Set(set.key)));
    let mut h = harness(m);
    h.run();
    h.get_by_label("No problems. Notes (1) lists what Tessalith did for you.");
    assert_eq!(
        h.query_all_by_label_contains("hidden in this project")
            .count(),
        0,
        "nothing of the set's is hidden"
    );
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

/// A NIF edit's problem of `level` on `path`, as the Meshes view hands them over.
fn nif(level: sdc_gui::model::meshes::NifLevel, path: &str) -> sdc_gui::model::meshes::NifProblem {
    sdc_gui::model::meshes::NifProblem {
        level,
        path: path.into(),
        text: format!("{path} needs a look"),
        tab: sdc_gui::model::meshes::NifTab::Shader,
        field: None,
    }
}

/// A last-export File row source: job `job` with `ws`.
fn export_row(job: &str, ws: Vec<Warning>) -> sdc_gui::model::export::Row {
    sdc_gui::model::export::Row {
        job_key: job.into(),
        path: job.into(),
        state: sdc_gui::model::export::RowState::Done { passed: true },
        warnings: ws,
    }
}

/// Review E2 fix round 1 (the E2 acceptance): every markable row has its mark, and no other row has one. Walks the
/// widest fixtures: item, set and project warnings, codeless rows of every built kind, NIF rows of each level, last-export
/// File rows, and a real PGPatcher pitfall.
#[test]
fn every_markable_row_has_its_mark_and_no_other_row_has_one() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{markable, problems};
    use sdc_gui::model::meshes::NifLevel;
    let check = |m: &AppModel, what: &str| {
        let rows = problems(m);
        assert!(!rows.is_empty(), "{what}: no rows");
        for p in rows.iter() {
            assert_eq!(
                markable(p.level, p.code),
                p.mark.is_some(),
                "{what}: {:?} {:?} {:?}",
                p.level,
                p.code,
                p.text
            );
        }
    };
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
    assert!(
        problems(&m)
            .iter()
            .any(|p| p.carried && p.level == sdc_gui::model::bottom::Level::Warning),
        "precondition: a pitfall row"
    );
    check(&m, "pbr_opened with a pitfall");
    let (_d, mut m) = with_warnings(&[
        Warning::new(Code::ChanNotGray, "not grey"),
        Warning::new(Code::ValMipError, "mip 3"),
        Warning::new(Code::SugContradiction, "name vs content"),
        Warning::new(Code::ChanNormalLength, "short"),
        Warning::new(Code::DimNpot, "npot"),
    ]);
    let root = m.root.clone().unwrap();
    m.read_only_flags = vec![root.join("sdc.toml")];
    m.skipped_user_presets = vec![
        (
            "p/x.toml".into(),
            "preset parse error: p/x.toml: bad".into(),
        ),
        (
            "p/y.toml".into(),
            "preset 'y': suffix alias 'zz' is already declared by preset 'a'".into(),
        ),
    ];
    m.unreadable = vec![Warning::new(
        Code::InUnreadable,
        r"skipped: C:\mods\a\locked: Access is denied. (os error 5)",
    )];
    m.set_nif_problems(std::sync::Arc::new(vec![
        nif(NifLevel::Blocks, "meshes/a.nif"),
        nif(NifLevel::Warning, "meshes/b.nif"),
        nif(NifLevel::Info, "meshes/c.nif"),
    ]));
    m.export.rows = vec![export_row(
        "derived/x",
        vec![
            Warning::new(Code::DeriveNifSuggest, "flags 4845"),
            Warning::new(Code::ValHeader, "bad header"),
            Warning::new(Code::ChkLowDetail, "low detail"),
            Warning::new(Code::ValMipError, "mip 1"),
        ],
    )];
    m.invalidate_views();
    check(&m, "item, project, NIF and File rows");
    let (_d, m) = with_set_warnings(&[
        Warning::new(Code::ShaderMissingMap, "no _n")
            .with_data(serde_json::json!({"map": "_n", "shader": "pbr"})),
        Warning::new(Code::ShaderExtraMap, "m")
            .with_data(serde_json::json!({"member": "rock01_x.png", "shader": "pbr"})),
        Warning::new(Code::PackConflict, "two sources"),
    ]);
    check(&m, "set rows");
}

/// `with_warnings(ws)`, saved, with `sdc-project.toml` then rewritten to `text` and both documents reloaded; `ws`
/// injected again after the plan. Returns the file's path.
fn reloaded_with(ws: &[Warning], text: &str) -> (tempfile::TempDir, AppModel, std::path::PathBuf) {
    let (d, mut m) = with_warnings(ws);
    assert!(m.save().is_ok());
    let file = m.root.clone().unwrap().join("sdc-project.toml");
    std::fs::write(&file, text).unwrap();
    m.reload_documents();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    inject(&mut m, ws);
    (d, m, file)
}

/// Review E2 fix round 1: `mark_intended` itself writes nothing on a read-only document or a non-array `intended`.
/// The guard in `mark_intended`, `set_intended`'s `intended_editable` check and `edit_docs`' equal-snapshot check
/// refuse the same cases, so this pins the behaviour, not one line.
#[test]
fn mark_intended_writes_nothing_where_mark_is_off() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::problems;
    let ws = [Warning::new(Code::ChanNotGray, "not grey")];
    for text in ["[project\n", "[project]\nintended = \"x\"\n"] {
        let (_d, mut m, file) = reloaded_with(&ws, text);
        assert!(m.mark_off_reason().is_some(), "{text:?}");
        let p = problems(&m)
            .iter()
            .find(|p| p.code == Some(Code::ChanNotGray))
            .cloned()
            .expect("the row");
        let before = (
            std::fs::read(&file).unwrap(),
            m.sets_doc.as_ref().unwrap().text(),
        );
        m.mark_intended(p.mark.clone().unwrap(), true, "row");
        assert_eq!(
            (
                std::fs::read(&file).unwrap(),
                m.sets_doc.as_ref().unwrap().text()
            ),
            before,
            "{text:?}"
        );
        assert!(!m.can_undo(), "{text:?}: nothing to undo");
    }
}

/// Review E2 fix round 1 (controller ruling 7): a stored key equal to what a Blocks, an Error File or a Decide row
/// would have never takes it out of the worklist or of Ready.
#[test]
fn a_stored_key_never_unblocks_a_row_that_is_not_markable() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{
        counts_as_ready_problem, filtered, flagged, intended_key, problems, At, Level, Problem,
    };
    use sdc_gui::model::meshes::NifLevel;
    let (_d, mut m) = with_warnings(&[]);
    m.set_nif_problems(std::sync::Arc::new(vec![nif(
        NifLevel::Blocks,
        "meshes/a.nif",
    )]));
    m.export.rows = vec![export_row(
        "derived/x",
        vec![Warning::new(Code::ValHeader, "bad header")],
    )];
    m.invalidate_views();
    let pick = |m: &AppModel| -> Vec<Problem> {
        problems(m)
            .iter()
            .filter(|p| {
                matches!(p.at, At::Nif(_))
                    || p.code == Some(Code::ValHeader)
                    || p.level == Level::Decide
            })
            .cloned()
            .collect()
    };
    let rows = pick(&m);
    assert!(
        rows.iter().any(|p| p.level == Level::Blocks),
        "precondition: a NIF block"
    );
    assert!(
        rows.iter().any(|p| p.level == Level::Decide),
        "precondition: a Decide row"
    );
    assert!(
        rows.iter().any(|p| p.code == Some(Code::ValHeader)),
        "precondition: an Error File row"
    );
    let ready = flagged(&m).ready_rows;
    // The kind each would be marked under: a NIF row's, its code, or (a Decide row has none) an empty one.
    let doc = m.sets_doc.as_mut().unwrap();
    for p in &rows {
        let kind = match (&p.at, p.code) {
            (At::Nif(_), _) => "NIF-WARNING",
            (_, Some(c)) => c.as_str(),
            _ => "",
        };
        doc.set_intended(&intended_key(&p.at, kind, &p.shown()), true);
    }
    assert_eq!(doc.intended().len(), rows.len(), "every key stored");
    let list = filtered(&m, None);
    for p in pick(&m) {
        assert!(!p.intended && p.mark.is_none(), "{:?}", p.text);
        assert!(counts_as_ready_problem(&p), "{:?}", p.text);
        assert!(
            list.iter().any(|r| r.text == p.text),
            "{:?} left the worklist",
            p.text
        );
    }
    assert_eq!(flagged(&m).ready_rows, ready, "Ready unchanged");
}

/// Marks the first Problems row of `code` as intended, as its **Mark as intended** button does.
fn mark_first(m: &mut AppModel, code: engine::codes::Code) {
    let p = sdc_gui::model::bottom::problems(m)
        .iter()
        .find(|p| p.code == Some(code))
        .cloned()
        .unwrap();
    m.mark_intended(p.mark.clone().unwrap(), true, "row");
}

/// Review E4: one readiness rule. A marked row takes "ready" away from neither its tile nor its set.
#[test]
fn a_set_tile_and_badge_agree_once_a_row_is_marked() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{set_ready, tile_ready};
    use sdc_gui::model::items::ItemStatus;
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    let v = rock01(&m);
    let key = v.members[0].clone();
    assert_eq!(
        m.item(&key).unwrap().status,
        ItemStatus::Ready,
        "precondition"
    );
    assert!(!set_ready(&m, &v));
    assert!(!tile_ready(&m, m.item(&key).unwrap()));
    mark_first(&mut m, Code::ValMipError);
    let v = rock01(&m);
    assert_eq!(
        set_ready(&m, &v),
        baseline().1,
        "the set is as ready as with no warning"
    );
    assert!(tile_ready(&m, m.item(&key).unwrap()), "the tile is ready");
    let w = &m.item(&key).unwrap().warnings[0];
    assert!(!sdc_gui::model::bottom::counts_for(
        &m,
        &sdc_gui::model::bottom::At::Texture(key.clone()),
        w
    ));
}

/// Review E4: the Textures column's "Has warnings" pill, its filter and the Status sort drop a marked warning.
#[test]
fn a_marked_warning_leaves_the_warnings_pill_filter_and_status_sort() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::texture_view::SortKey;
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    let key = rock01(&m).members[0].clone();
    let idx = m.items.iter().position(|i| i.key == key).unwrap();
    let warned = m.filter_counts().warnings;
    assert!(warned >= 1);
    m.filter.warnings = true;
    assert!(
        m.visible_items().contains(&idx),
        "listed under Has warnings"
    );
    m.filter.warnings = false;
    m.prefs.sort = SortKey::Status;
    assert_eq!(m.visible_items()[0], idx, "a warned texture sorts first");
    mark_first(&mut m, Code::ValMipError);
    assert_eq!(m.filter_counts().warnings, warned - 1, "the pill's count");
    let undecided = m
        .items
        .iter()
        .position(|i| i.status == sdc_gui::model::items::ItemStatus::NeedsDecision)
        .expect("the fixture's undecided texture");
    let at = |n: usize| m.visible_items().iter().position(|&i| i == n).unwrap();
    assert!(
        at(idx) > at(undecided),
        "sorted after an undecided texture, as a clean one"
    );
    m.filter.warnings = true;
    assert!(!m.visible_items().contains(&idx), "gone from Has warnings");
}

/// Review E4: in the set editor, the status dot of the slot whose only warning is marked turns from "warning" to
/// "ready" (`tile_ready`). Counted in the centre only: the side columns and the bottom panel draw dots of their own.
#[test]
fn the_set_editors_dot_is_ready_once_its_warning_is_marked() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    let key = rock01(&m).key;
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let centre = |h: &Harness<'static, AppModel>, label: &str| {
        let sides: Vec<egui::Rect> = ["header", "sets", "textures", "bottom"]
            .into_iter()
            .filter_map(|n| sdc_gui::ui::panel_ids::panel_rect(&h.ctx, n))
            .collect();
        h.query_all_by_label(label)
            .filter(|n| !sides.iter().any(|r| r.contains(n.rect().center())))
            .count()
    };
    let ready = centre(&h, "ready");
    // Task 9: the status line's dot is the second; it turns ready with the slot's.
    assert_eq!(
        centre(&h, "warning"),
        2,
        "the slot's dot and the status line's before the mark"
    );
    mark_first(h.state_mut(), Code::ValMipError);
    h.run();
    assert_eq!(centre(&h, "warning"), 0, "no warning dot once marked");
    assert_eq!(
        centre(&h, "ready"),
        ready + 2,
        "the slot's dot and the status line's are ready"
    );
}

/// `checks.rs::pbr_wall(dir, true, false)`, copied: one PBR set under `textures/test` whose flat `wall_p.png` raises
/// one CHK-PARALLAX check row, opened as the app opens a folder, shader Pbr.
fn flat_parallax_wall(dir: &std::path::Path) -> AppModel {
    use image::{ImageBuffer, Rgb};
    let r = dir.join("textures/test");
    std::fs::create_dir_all(&r).unwrap();
    let flat = |rgb: [u8; 3], name: &str| {
        ImageBuffer::from_fn(64, 64, |_, _| Rgb(rgb))
            .save(r.join(name))
            .unwrap()
    };
    flat([128, 128, 128], "wall.png");
    flat([128, 128, 128], "wall_p.png");
    flat([128, 10, 200], "wall_rmaos.png");
    flat([128, 128, 255], "wall_n.png");
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    let id = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.ends_with("wall"))
        .expect("the set")
        .id
        .clone();
    m.set_shader(&id, Some(engine::shader::Shader::Pbr));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m
}

/// Review E4: a marked check leaves the card's ⓘ badge, the Checks pill (project and set) and the Checks lists, and
/// moves one row from Problems to Notes in the tab; Undo mark brings all of it back.
#[test]
fn a_marked_check_leaves_the_card_badge_and_the_checks_pill() {
    use engine::codes::Code;
    use sdc_gui::model::bottom::{
        cached_tab_counts, filtered, problems, Level, ProblemsFilter, TabCounts,
    };
    use sdc_gui::ui::problems::CHECKS_PILL;
    let d = tempfile::tempdir().unwrap();
    let m = flat_parallax_wall(d.path());
    let set = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.ends_with("wall"))
        .unwrap()
        .key
        .clone();
    let mut h = harness_at(m, egui::vec2(1600.0, 1000.0));
    h.run();
    let check = problems(h.state())
        .iter()
        .find(|p| p.level == Level::Check)
        .cloned()
        .unwrap();
    assert_eq!(check.code, Some(Code::ChkParallax), "precondition");
    let before = cached_tab_counts(h.state());
    // (card badges, Checks pill shown on the project's list, on the set's list, the Checks list, the set's Checks list)
    let seen = |h: &mut Harness<'static, AppModel>| {
        let card = h.query_all_by_label_contains("Check results").count();
        let mut pill = vec![];
        for f in [None, Some(ProblemsFilter::Set(set.clone()))] {
            h.state_mut().open_problems(f);
            h.run();
            pill.push(h.query_by_label(CHECKS_PILL).is_some());
        }
        let listed = |f| filtered(h.state(), Some(&f)).len();
        (
            card,
            pill,
            listed(ProblemsFilter::Checks),
            listed(ProblemsFilter::SetChecks(set.clone())),
        )
    };
    assert_eq!(seen(&mut h), (1, vec![true, true], 1, 1), "before");
    h.state_mut()
        .mark_intended(check.mark.clone().unwrap(), true, "row");
    h.run();
    assert_eq!(seen(&mut h), (0, vec![false, false], 0, 0), "marked");
    assert_eq!(
        cached_tab_counts(h.state()),
        TabCounts {
            problems: before.problems - 1,
            notes: before.notes + 1,
            ..before
        }
    );
    h.state_mut()
        .mark_intended(check.mark.clone().unwrap(), false, "row");
    h.run();
    assert_eq!(seen(&mut h), (1, vec![true, true], 1, 1), "mark undone");
    assert_eq!(cached_tab_counts(h.state()), before);
}

/// Review E4: after a mark, the tile's ⚠, the set card's "⚠ n" and the pill's ⚠ all go, for a member's warning and
/// for the set's own.
#[test]
fn a_marked_warning_leaves_every_amber_badge() {
    use engine::codes::{Code, Warning};
    let ws = [Warning::new(Code::ValMipError, "mip 3")];
    for (what, (_d, m)) in [
        ("member", with_warnings(&ws)),
        ("set", with_set_warnings(&ws)),
    ] {
        let mut h = harness(m);
        h.run();
        h.get_by_label("⚠ 1");
        mark_first(h.state_mut(), Code::ValMipError);
        h.run();
        assert_eq!(
            h.query_all_by_label_contains("⚠").count(),
            0,
            "{what}: no amber badge"
        );
    }
}

/// The Normal member: `pbr_opened` decides rock01_normal.png as PbrNormal. `remove_member` refuses only the packed
/// output, so any raw member would do; members are picked by slot, never by index.
fn normal_member(m: &AppModel) -> (String, String) {
    use sdc_gui::model::sets::{slot_of, SlotKind};
    let k = rock01(m)
        .members
        .iter()
        .find(|k| m.item(k).and_then(slot_of) == Some(SlotKind::Normal))
        .cloned()
        .unwrap();
    let name = m.item(&k).unwrap().name.clone();
    (k, name)
}

/// The note sentence `extra_map(name)`'s row shows (the table's SHADER-EXTRA-MAP row).
fn extra_note(name: &str) -> String {
    format!("{name} isn't read by the pbr shader. It is still converted, so nothing breaks.")
}

fn extra_map(name: &str) -> engine::codes::Warning {
    engine::codes::Warning::new(
        engine::codes::Code::ShaderExtraMap,
        format!("{name} is not used by the pbr shader; it is still converted"),
    )
    .with_data(serde_json::json!({"member": name, "shader": "pbr"}))
}

/// The button labelled `label` whose centre is nearest `to`'s: an open popup's item, not a same-named widget elsewhere
/// (a popup's nodes follow its opener in tree order, so `last()` is not the popup's).
fn nearest<'a>(
    h: &'a Harness<'static, AppModel>,
    label: &'a str,
    to: egui::Rect,
) -> egui_kittest::Node<'a> {
    h.query_all_by_role_and_label(egui::accesskit::Role::Button, label)
        .min_by(|a, b| {
            let d = |n: &egui_kittest::Node<'_>| n.rect().center().distance(to.center());
            d(a).total_cmp(&d(b))
        })
        .expect("a match")
}

#[test]
fn an_unused_map_row_offers_remove_from_set_and_it_clears_the_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let (_k, member) = normal_member(&m);
    inject_set(&mut m, &[extra_map(&member)]);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    let note = extra_note(&member);
    let remove = format!("Remove {member} from landscape/rock01");
    // Controller ruling 2: left to right, the fix, the second fix, then Go to set.
    let xs: Vec<f32> = [
        remove.as_str(),
        "Change shader for landscape/rock01",
        "Go to set",
    ]
    .iter()
    .map(|l| on_row(&h, &note, l).rect().center().x)
    .collect();
    assert!(xs[0] < xs[1] && xs[1] < xs[2], "row order: {xs:?}");
    on_row(&h, &note, &remove).click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert!(!rock01(h.state())
        .members
        .iter()
        .any(|k| h.state().item(k).unwrap().name == member));
}

/// Controller ruling: SUG-CONTRADICTION offers the fix its sentence names. A diffuse whose contents are a
/// tangent-space normal gets Set type…; choosing the type the contents suggest removes the row.
/// Trigger: the engine's policy raises SUG-CONTRADICTION when a Color file's contents read as a normal map
/// (`normal_len_err < NORMAL_LEN_TOL` and a blue mean of at least 0.75), which a flat (128, 128, 255) image meets.
#[test]
fn a_type_contradiction_row_offers_set_type_and_the_right_type_clears_it() {
    use engine::codes::Code;
    use engine::config::Subtype;
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    image::ImageBuffer::from_fn(32, 32, |_, _| image::Rgb([128u8, 128, 255]))
        .save(root.join("landscape/wall.png"))
        .unwrap();
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(d.path().join("export")),
        ..Default::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let key = "landscape/wall.png";
    m.decide(key, sdc_gui::model::Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let has = |m: &AppModel| {
        sdc_gui::model::bottom::problems(m).iter().any(|p| {
            p.code == Some(Code::SugContradiction)
                && matches!(&p.at, sdc_gui::model::bottom::At::Texture(k) if k == key)
        })
    };
    assert!(has(&m), "the contents look like a normal map");
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    let sentence = sdc_gui::model::code_text::code_sentence(Code::SugContradiction);
    let opener = on_row(&h, sentence, "Set type…");
    let at = opener.rect();
    opener.click();
    h.run();
    let normal = sdc_gui::ui::settings_panel::subtype_label(Subtype::Normal);
    // The picker's list scrolls: bring the item into view before the pointer click.
    nearest(&h, &normal, at).scroll_to_me();
    h.run();
    nearest(&h, &normal, at).click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert!(
        !has(h.state()),
        "Normal is what the contents suggest; the row is gone"
    );
}

#[test]
fn a_parallax_row_offers_turn_parallax_off() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::AlphaParallaxNoAlpha, "p")]);
    let file = m.item(&rock01(&m).members[0]).unwrap().name.clone();
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    h.get_by_label(&format!("Turn parallax off for {file}"))
        .click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    let key = rock01(h.state()).members[0].clone();
    let v = h
        .state()
        .project
        .as_ref()
        .unwrap()
        .file_value(&key, "alpha.parallax");
    assert_eq!(v, Some(serde_json::json!(false)));
}

#[test]
fn a_missing_map_row_offers_change_shader() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = with_set_warnings(&[Warning::new(Code::ShaderMissingMap, "no _n")
        .with_data(serde_json::json!({"map": "_n", "shader": "pbr"}))]);
    let target = m.target;
    let current = rock01(&m).shader_choice;
    // Pick a shader the set does not have, so the test cannot pass by leaving the choice alone.
    let first = sdc_gui::model::sets::shader_options(target)
        .into_iter()
        .find(|s| current.as_ref() != Some(s))
        .expect("precondition: the target offers a shader other than the set's");
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    let opener = h.get_by_label("Change shader for landscape/rock01");
    let at = opener.rect();
    opener.click();
    h.run();
    nearest(&h, first.label(), at).click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert_eq!(rock01(h.state()).shader_choice, Some(first));
    // Fix round 1: the menu marks the set's shader, as the sets column's combo does, and picking it again is no edit.
    // The pick planned again, which replaced the injected warning.
    inject_set(
        h.state_mut(),
        &[Warning::new(Code::ShaderMissingMap, "no _n")
            .with_data(serde_json::json!({"map": "_n", "shader": "pbr"}))],
    );
    h.run();
    let opener = h.get_by_label("Change shader for landscape/rock01");
    let at = opener.rect();
    opener.click();
    h.run();
    let toggled = |h: &Harness<'static, AppModel>, s: engine::shader::Shader| {
        nearest(h, s.label(), at).accesskit_node().toggled()
    };
    assert_eq!(
        toggled(&h, first),
        Some(egui::accesskit::Toggled::True),
        "the current shader"
    );
    let other = sdc_gui::model::sets::shader_options(target)
        .into_iter()
        .find(|s| *s != first)
        .expect("precondition: a second shader");
    assert_eq!(toggled(&h, other), Some(egui::accesskit::Toggled::False));
    let pushes = h.state().history.pushes;
    nearest(&h, first.label(), at).click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert_eq!(
        h.state().history.pushes,
        pushes,
        "the shader it has: no undo step"
    );
}

/// Review E10: each code's row carries the fix the table gives it, and SHADER-EXTRA-MAP's member is matched by the
/// name the engine writes (the member's output file, `api.rs` `shader_members`) as well as by its source name.
#[test]
fn each_row_carries_the_tables_fix() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{problems, Fix};
    let fixes = |m: &AppModel, code: Code| {
        let rows = problems(m);
        let r = rows
            .iter()
            .find(|p| p.code == Some(code))
            .unwrap_or_else(|| panic!("a {code:?} row"));
        (r.fix.clone(), r.second_fix.clone())
    };
    let (_d, m) = with_warnings(&[
        Warning::new(Code::AlphaParallaxNoAlpha, "p"),
        Warning::new(Code::GrassDetectSource, "g"),
        Warning::new(Code::SugContradiction, "s"),
    ]);
    let k = rock01(&m).members[0].clone();
    assert_eq!(
        fixes(&m, Code::AlphaParallaxNoAlpha),
        (Some(Fix::ParallaxOff(k.clone())), None)
    );
    // Fix round 1: the engine raises it only on a texture already tagged grass, so Tag as grass would change nothing;
    // the row keeps only Mark, and its sentence says what the code means.
    assert_eq!(fixes(&m, Code::GrassDetectSource), (None, None));
    assert_eq!(
        sdc_gui::model::code_text::code_sentence(Code::GrassDetectSource),
        "This grass source's corner normal isn't unit length — the written file is renormalised, so Community \
         Shaders still reads it as grass"
    );
    assert_eq!(
        fixes(&m, Code::SugContradiction),
        (Some(Fix::SetType(k)), None)
    );
    let shader = || Some(Fix::ChangeShader("landscape/rock01".into()));
    for code in [Code::ShaderUnavailable, Code::ShaderMissingMap] {
        let (_d, m) = with_set_warnings(&[Warning::new(code, "m")]);
        assert_eq!(fixes(&m, code), (shader(), None), "{code:?}");
    }
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let (nk, name) = normal_member(&m);
    let out = m.item(&nk).unwrap().job.as_ref().unwrap().output.clone();
    let out = out.file_name().unwrap().to_string_lossy().into_owned();
    assert_ne!(
        out, name,
        "precondition: the output name is not the source's"
    );
    let remove = Some(Fix::RemoveMember {
        set_id: "landscape/rock01".into(),
        item: nk,
    });
    for member in [&name, &out] {
        inject_set(&mut m, &[extra_map(member)]);
        assert_eq!(
            fixes(&m, Code::ShaderExtraMap),
            (remove.clone(), shader()),
            "{member}"
        );
    }
    // No member of that name, or no `data`: Change shader… is the row's one button.
    inject_set(&mut m, &[extra_map("nowhere.png")]);
    assert_eq!(fixes(&m, Code::ShaderExtraMap), (shader(), None));
    inject_set(&mut m, &[Warning::new(Code::ShaderExtraMap, "m")]);
    assert_eq!(fixes(&m, Code::ShaderExtraMap), (shader(), None));
}

/// Review E10: a CHK-Z-SIGN check row offers Rebuild Z (`give_a_check` plans one on rock01's normal), and the click
/// clears the row: the check reads `channels.reconstruct_z`, as CHK-GREEN-GUESS reads `flip_green` (fix round 1).
#[test]
fn a_z_sign_check_offers_rebuild_z_and_it_clears_the_row() {
    use engine::codes::Code;
    use sdc_gui::model::bottom::{problems, Fix};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    give_a_check(&mut m, d.path());
    let (nk, _) = normal_member(&m);
    let has = |m: &AppModel| {
        problems(m)
            .iter()
            .find(|p| p.code == Some(Code::ChkZSign))
            .map(|p| p.fix.clone())
    };
    assert_eq!(
        has(&m),
        Some(Some(Fix::RebuildZ(nk))),
        "precondition: the check ran"
    );
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    // The sentence names "Rebuild Z" too, so the button is found by its role on the row.
    let y = h
        .get_by_label(sdc_gui::model::code_text::code_sentence(Code::ChkZSign))
        .rect()
        .center()
        .y;
    h.query_all_by_role_and_label(egui::accesskit::Role::Button, "Rebuild Z")
        .find(|n| (n.rect().center().y - y).abs() <= 4.0)
        .expect("Rebuild Z on the row")
        .click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert_eq!(has(h.state()), None, "Rebuild Z is on: the row is gone");
}

/// Fix round 1: Remove from set is off for `remove_member`'s own refusal, here a locked Normal slot, with that
/// refusal as its hover.
#[test]
fn remove_from_set_is_off_with_the_refusal_for_a_locked_slot() {
    use sdc_gui::model::sets::SlotKind;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let (k, member) = normal_member(&m);
    let n = SlotKind::Normal.slot_number().unwrap();
    m.set_slot_lock("landscape/rock01", n, true).unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    inject_set(&mut m, &[extra_map(&member)]);
    let why = m
        .remove_member_refusal("landscape/rock01", &k)
        .expect("precondition: a locked slot is refused");
    m.open_problems(None);
    let mut h = harness(m);
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.run();
    let remove = format!("Remove {member} from landscape/rock01");
    let b = on_row(&h, &extra_note(&member), &remove);
    assert!(b.accesskit_node().is_disabled(), "refused: off");
    b.hover();
    h.run();
    h.run();
    h.get_by_label(&why);
}

/// Review E18 (Task 6 review, made live by Task 7): under a Set filter whose only hidden row is a note with a button
/// (SHADER-EXTRA-MAP's Remove from set), while a problem row elsewhere in the project is hidden, the empty text is
/// about this list, which hid no problem. The other hidden problem is a last-export File row in no set: `pbr_opened`
/// has one set with members (leaf.png and odd.png stay undecided, in none), and a File row is the cheap way to give the
/// project a hidden problem outside rock01.
#[test]
fn a_sets_hidden_note_with_a_button_keeps_the_hidden_rows_text() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{cached_tab_counts, ProblemsFilter};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let (_k, member) = normal_member(&m);
    inject_set(&mut m, &[extra_map(&member)]);
    m.export.rows = vec![export_row(
        "derived/x",
        vec![Warning::new(Code::ChanNotGray, "not grey")],
    )];
    m.invalidate_views();
    m.set_kind_hidden(Code::ShaderExtraMap, true);
    m.set_kind_hidden(Code::ChanNotGray, true);
    let project = cached_tab_counts(&m).hidden;
    assert!(project > 0, "the project hides a problem row");
    m.open_problems(Some(ProblemsFilter::Set(rock01(&m).key)));
    let mut h = harness(m);
    h.run();
    h.get_by_label("Every row here is hidden — see Kinds");
    assert_eq!(h.query_all_by_label(ALL_HIDDEN).count(), 0);
    h.get_by_label_contains("hidden in this project");
}

#[test]
fn reveal_holds_two_seconds_then_fades_unless_motion_is_reduced() {
    use sdc_gui::ui::row_reveal::reveal_alpha;
    assert_eq!(reveal_alpha(0.0, false), 1.0);
    assert_eq!(reveal_alpha(1.99, false), 1.0);
    assert!(reveal_alpha(2.06, false) > 0.0 && reveal_alpha(2.06, false) < 1.0);
    assert_eq!(reveal_alpha(2.12, false), 0.0);
    assert_eq!(reveal_alpha(1.99, true), 1.0);
    assert_eq!(reveal_alpha(2.0, true), 0.0, "reduced: no fade");
}

#[test]
fn reduced_follows_the_style_and_the_test_seam() {
    let ctx = egui::Context::default();
    assert!(!sdc_gui::ui::motion::reduced(&ctx));
    sdc_gui::ui::motion::force_reduced(&ctx, true);
    assert!(sdc_gui::ui::motion::reduced(&ctx));
    let ctx = egui::Context::default();
    ctx.global_style_mut(|s| s.animation_time = 0.0);
    assert!(sdc_gui::ui::motion::reduced(&ctx));
}

/// The brief's labels, and those `map_slot` knows beyond them (controller ruling for Task 8).
#[test]
fn a_shader_map_label_names_its_slot_card() {
    use sdc_gui::model::sets::{slot_for_shader_map, SlotKind};
    for (label, want) in [
        ("albedo", Some(SlotKind::Diffuse)),
        ("diffuse", Some(SlotKind::Diffuse)),
        ("skin_albedo", Some(SlotKind::Diffuse)),
        ("_n", Some(SlotKind::Normal)),
        ("_n/_msn", Some(SlotKind::Normal)),
        ("_n (model-space)", Some(SlotKind::Normal)),
        ("_rmaos", Some(SlotKind::Rmaos)),
        ("_p", Some(SlotKind::Height)),
        ("_g", Some(SlotKind::Emissive)),
        ("_i", None),
        ("complex_grass", None),
        ("no such map", None),
    ] {
        assert_eq!(slot_for_shader_map(label), want, "{label}");
    }
}

/// Each set row that names a slot or a file carries it for Go to set; the others carry nothing.
#[test]
fn set_rows_carry_their_go_to_set_target() {
    use engine::codes::Code;
    use sdc_gui::model::{nav::SlotTarget, sets::SlotKind};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let (key, name) = normal_member(&m);
    inject_set(
        &mut m,
        &[
            extra_map(&name),
            Warning::new(Code::ShaderMissingMap, "no _rmaos")
                .with_data(serde_json::json!({"map": "_rmaos", "shader": "pbr"})),
            Warning::new(Code::DeriveNoAlbedo, "no albedo"),
            Warning::new(Code::DeriveDdsSource, "a dds source"),
        ],
    );
    let rows = sdc_gui::model::bottom::problems(&m);
    let target = |c: Code| {
        rows.iter()
            .find(|p| p.code == Some(c))
            .unwrap_or_else(|| panic!("a {c:?} row"))
            .target
            .clone()
    };
    assert_eq!(target(Code::ShaderExtraMap), Some(SlotTarget::Item(key)));
    assert_eq!(
        target(Code::ShaderMissingMap),
        Some(SlotTarget::Slot(SlotKind::Rmaos))
    );
    assert_eq!(
        target(Code::DeriveNoAlbedo),
        Some(SlotTarget::Slot(SlotKind::Diffuse))
    );
    assert_eq!(target(Code::DeriveDdsSource), None);
}

/// The extra-map fixture of Task 7 (`normal_member`, `extra_map`) at 1280x800.
fn extra_map_at_1280(
    reduced: bool,
) -> (
    tempfile::TempDir,
    Harness<'static, AppModel>,
    String,
    String,
) {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let (key, name) = normal_member(&m);
    inject_set(&mut m, &[extra_map(&name)]);
    m.open_problems(None);
    let mut h = Harness::builder()
        .with_context_setup(move |ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
            sdc_gui::ui::motion::force_reduced(ctx, reduced);
        })
        .with_size(egui::vec2(1280.0, 800.0))
        .with_step_dt(0.25)
        .with_max_steps(64)
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    if !reduced {
        // kittest turns egui's animations off (`animation_time` 0, after the context setup), which
        // `motion::reduced` reads as reduced motion: give them back.
        let on = egui::Style::default().animation_time;
        h.ctx.all_styles_mut(|s| s.animation_time = on);
    }
    h.run();
    (d, h, key, name)
}

/// The frame just drawn asked for the next one at once (`requested_repaint_last_pass` reports the frame before).
fn asks_now(h: &Harness<'static, AppModel>) -> bool {
    wake(h) == std::time::Duration::ZERO
}

/// When the frame just drawn asked to be drawn again.
fn wake(h: &Harness<'static, AppModel>) -> std::time::Duration {
    h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay
}

/// A node labelled `label` lies inside the centre panel (`assert_in_centre`, scoped: the file name is on a Textures
/// tile too).
fn shown_in_centre(h: &Harness<'static, AppModel>, label: &str) {
    let c = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    let rects: Vec<egui::Rect> = h.query_all_by_label(label).map(|n| n.rect()).collect();
    assert!(
        rects.iter().any(|r| c.expand(0.5).contains_rect(*r)),
        "no {label} inside the centre {c:?}: {rects:?}"
    );
}

/// Review E10 acceptance: after Go to set the named file's row is visible and outlined, and a button on the
/// Problems row removes that row. Not reduced: it fades and is gone after 2 s, and the frame goes idle.
#[test]
fn go_to_set_shows_and_outlines_the_file_the_row_names() {
    use sdc_gui::model::nav::SlotTarget;
    use sdc_gui::ui::row_reveal::revealed;
    let (_d, mut h, key, name) = extra_map_at_1280(false);
    on_row(&h, &extra_note(&name), "Go to set").click();
    h.step();
    h.step();
    assert_eq!(revealed(&h.ctx), Some((SlotTarget::Item(key.clone()), 1.0)));
    shown_in_centre(&h, &name);
    on_row(
        &h,
        &extra_note(&name),
        &format!("Remove {name} from landscape/rock01"),
    );
    // The Sets column settles its own scroll to the selected card first (see the reduced-motion test).
    h.run_steps(3); // 1.25 s
    for _ in 0..3 {
        // Up to 1.75 s: held at full strength. The only wake asked for is the timed one at the end of the hold.
        assert_eq!(revealed(&h.ctx).map(|r| r.1), Some(1.0));
        assert!(!asks_now(&h), "no repaint loop while held");
        assert!(wake(&h).as_secs_f64() <= sdc_gui::ui::row_reveal::REVEAL_SECS);
        h.step();
    }
    // 2 s to 2.75 s. The fade's frame shows the outline and asks for the next frame at once (reduced motion never does).
    let mut fading = revealed(&h.ctx).is_some() && asks_now(&h);
    for _ in 0..3 {
        h.step();
        fading |= revealed(&h.ctx).is_some() && asks_now(&h);
    }
    assert!(fading, "a fade frame");
    assert_eq!(revealed(&h.ctx), None);
    h.step();
    assert!(
        !h.ctx.requested_repaint_last_pass(),
        "idle after the outline"
    );
}

/// Spec section 6, E10 ("no motion if reduced motion is set"); full strength with no fade is the planner's call:
/// reduced motion draws the outline at full strength, no fade and no repaint loop.
#[test]
fn with_reduced_motion_the_outline_holds_full_then_goes_without_a_fade() {
    use sdc_gui::ui::row_reveal::revealed;
    let (_d, mut h, _key, name) = extra_map_at_1280(true);
    on_row(&h, &extra_note(&name), "Go to set").click();
    // The click's last frame starts the reveal, a step after the release, so each later step is 0.25 s past it.
    h.step();
    h.step(); // 0.5 s
    assert_eq!(revealed(&h.ctx).map(|r| r.1), Some(1.0));
    // The Sets column scrolls the newly selected set's card into view over these frames, as any selection does (its
    // own reveal and scroll animation); the outline asks for nothing then either.
    h.run_steps(3); // 1.25 s
    for _ in 0..3 {
        // up to 1.75 s
        assert_eq!(revealed(&h.ctx).map(|r| r.1), Some(1.0));
        assert!(!asks_now(&h), "no repaint loop while held");
        assert!(
            !h.ctx.requested_repaint_last_pass(),
            "no repaint loop while held"
        );
        h.step();
    }
    for _ in 0..3 {
        // past 2 s: the outline goes in one step, with no fade frame
        assert!(revealed(&h.ctx).is_none_or(|_| !asks_now(&h)), "no fade");
        h.step();
    }
    assert_eq!(revealed(&h.ctx), None);
}

/// The jump: with the set's editor already open and scrolled past its slot rows, Go to set brings the row back.
#[test]
fn go_to_set_scrolls_an_editor_scrolled_away_back_to_the_row() {
    let (_d, mut h, _key, name) = extra_map_at_1280(true);
    let set = rock01(h.state()).key;
    h.state_mut().select_set(&set);
    h.run();
    let c = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    h.event(egui::Event::PointerMoved(c.center()));
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -1500.0),
        source: egui::MouseWheelSource::Wheel,
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
    });
    h.run_steps(4);
    assert!(
        !h.query_all_by_label(&name)
            .any(|n| c.expand(0.5).contains_rect(n.rect())),
        "the editor is scrolled past {name}'s row"
    );
    on_row(&h, &extra_note(&name), "Go to set").click();
    // egui draws the jump two frames after the frame that asked for it (the area stores the target, then moves to it
    // at once); no frame shows a position in between.
    h.step();
    h.step();
    h.step();
    shown_in_centre(&h, &name);
}

#[test]
fn a_missing_map_go_to_set_outlines_the_empty_slot() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::{nav::SlotTarget, sets::SlotKind};
    let (_d, mut m) = with_set_warnings(&[Warning::new(Code::ShaderMissingMap, "no _n")
        .with_data(serde_json::json!({"map": "_n", "shader": "pbr"}))]);
    // empty the Normal slot: take its member out of the set (this plans again, so inject after it)
    let (n, _) = normal_member(&m);
    m.remove_member(&rock01(&m).id, &n).unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    inject_set(
        &mut m,
        &[Warning::new(Code::ShaderMissingMap, "no _n")
            .with_data(serde_json::json!({"map": "_n", "shader": "pbr"}))],
    );
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    on_row(&h, "no _n", "Go to set").click();
    h.step();
    h.step();
    assert_eq!(
        sdc_gui::ui::row_reveal::revealed(&h.ctx).map(|r| r.0),
        Some(SlotTarget::Slot(SlotKind::Normal))
    );
}

/// Controller ruling for Task 8: a target the set editor draws no row for selects the set, outlines nothing, and
/// leaves no reveal waiting.
#[test]
fn a_target_with_no_row_selects_the_set_and_outlines_nothing() {
    use sdc_gui::model::nav::SlotTarget;
    let (_d, m) = with_set_warnings(&[]);
    let key = rock01(&m).key;
    let mut h = harness(m);
    h.run();
    h.state_mut()
        .go_to_set(&key, Some(SlotTarget::Item("no/such/item".into())));
    h.step();
    h.step();
    assert_eq!(
        h.state().selection,
        sdc_gui::model::Selection::Set(key.clone())
    );
    assert!(h.state().nav.reveal_slot.is_none(), "the reveal was taken");
    assert_eq!(sdc_gui::ui::row_reveal::revealed(&h.ctx), None);
    h.run();
    assert_eq!(sdc_gui::ui::row_reveal::revealed(&h.ctx), None);
    // Nothing waits on it: a reveal left pending would wake the frame at the end of its hold.
    let next = wake(&h);
    assert!(
        next.as_secs_f64() >= sdc_gui::ui::row_reveal::REVEAL_SECS,
        "{next:?}"
    );
}

/// Review fix 1: a set opened by Go to set opens at its top, also when its target has no row (the centre's set editor
/// keeps one scroll offset for every set).
#[test]
fn go_to_set_with_no_row_opens_the_set_at_its_top() {
    use sdc_gui::model::nav::SlotTarget;
    let (_d, mut h, key, name) = extra_map_at_1280(true);
    let set = rock01(h.state()).key;
    h.state_mut().select_set(&set);
    h.run();
    let c = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    h.event(egui::Event::PointerMoved(c.center()));
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -1500.0),
        source: egui::MouseWheelSource::Wheel,
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
    });
    h.run_steps(4);
    assert!(
        !h.query_all_by_label(&name)
            .any(|n| c.expand(0.5).contains_rect(n.rect())),
        "the editor is scrolled past {name}'s row"
    );
    // Another view in the centre, then Go to set back: the set editor opens again.
    h.state_mut().select_item(&key);
    h.run();
    h.state_mut()
        .go_to_set(&set, Some(SlotTarget::Item("no/such/item".into())));
    h.run();
    shown_in_centre(&h, &name);
}

/// Review fix 2: a Go to set the editor has not drawn yet ends with the next selection, so it never fires later.
/// Pinned on the model: the editor is not drawn while a texture preview fills the centre.
#[test]
fn another_selection_drops_a_waiting_go_to_set() {
    use sdc_gui::model::nav::SlotTarget;
    use sdc_gui::model::sets::SlotKind;
    let (_d, mut m) = with_set_warnings(&[]);
    let set = rock01(&m).key;
    let (item, _) = normal_member(&m);
    m.go_to_set(&set, Some(SlotTarget::Slot(SlotKind::Normal)));
    assert!(m.nav.reveal_slot.is_some());
    m.select_item(&item);
    assert!(m.nav.reveal_slot.is_none(), "a texture selected");
    m.go_to_set(&set, Some(SlotTarget::Slot(SlotKind::Normal)));
    m.select_set(&set);
    assert!(m.nav.reveal_slot.is_none(), "a set selected");
}

/// Review fix 3: a slot target outlines the card that holds it, also when the card shows the other kind of its slot
/// (3 or 7, `SlotKind::shares_card`).
#[test]
fn a_slot_target_outlines_the_card_its_pair_shows() {
    use sdc_gui::model::nav::SlotTarget;
    use sdc_gui::model::sets::{slot_row, SlotKind};
    let (_d, mut m) = with_set_warnings(&[]);
    // The PBR shader's optional maps give rock01 the emissive (slot 3) and coat (slot 7) cards.
    m.set_shader(&rock01(&m).id, Some(engine::shader::Shader::Pbr));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let v = rock01(&m);
    let shown = slot_row(v.shader.as_ref(), &v.slots)
        .into_iter()
        .map(|c| c.kind)
        .find(|k| matches!(k.slot_number(), Some(3 | 7)))
        .expect("rock01's editor draws a slot 3 or 7 card");
    let other = SlotKind::ALL
        .into_iter()
        .find(|k| *k != shown && k.shares_card(shown))
        .unwrap();
    let mut h = harness(m);
    h.run();
    h.state_mut()
        .go_to_set(&v.key, Some(SlotTarget::Slot(other)));
    h.step();
    h.step();
    assert_eq!(
        sdc_gui::ui::row_reveal::revealed(&h.ctx),
        Some((SlotTarget::Slot(other), 1.0)),
        "{other:?} on the {shown:?} card"
    );
}

/// Review fix 4: `slot_for_shader_map` inverts `SlotKind::slot_number` for every label `map_slot` knows, so the two
/// tables cannot drift apart.
#[test]
fn slot_for_shader_map_agrees_with_slot_number() {
    use engine::{config::Target, shader::Shader};
    use sdc_gui::model::sets::{map_slot, slot_for_shader_map, SlotKind};
    let mut labels = vec![
        "_n",
        "_n/_msn",
        "_n (model-space)",
        "_g",
        "_p",
        "_rmaos",
        "skin_rfaos",
        "_m complex",
        "_i",
    ];
    for s in Shader::ALL {
        for t in [Target::Cs, Target::Vanilla] {
            let spec = s.spec_for(t);
            labels.extend(spec.required.iter().chain(spec.optional).map(|r| r.label));
        }
    }
    for l in labels {
        let Some(n) = map_slot(l) else { continue };
        match slot_for_shader_map(l) {
            None => assert_eq!(n, 7, "{l}: only slot 7 (`_i`) has no card"),
            Some(k) => assert_eq!(SlotKind::slot_number(k), Some(n), "{l}"),
        }
    }
}

/// Review fix 5, 6: the OS value `install` reads stops egui's own animations and its animated scroll on both themes;
/// with motion on, the style is left alone.
#[test]
fn install_follows_the_os_value_on_both_themes() {
    use egui::Theme;
    use sdc_gui::ui::motion::{install_with, reduced};
    let ctx = egui::Context::default();
    install_with(&ctx, true);
    assert!(reduced(&ctx));
    for t in [Theme::Dark, Theme::Light] {
        let s = ctx.style_of(t);
        assert_eq!(s.animation_time, 0.0, "{t:?}");
        assert_eq!(s.scroll_animation.duration.max, 0.0, "{t:?}");
    }
    let ctx = egui::Context::default();
    let before = (ctx.style_of(Theme::Dark), ctx.style_of(Theme::Light));
    install_with(&ctx, false);
    assert!(!reduced(&ctx));
    assert_eq!(*ctx.style_of(Theme::Dark), *before.0);
    assert_eq!(*ctx.style_of(Theme::Light), *before.1);
}

/// Controller ruling for Task 9: a click, then `run`, `wait_idle`, `run` (`bottom_panel.rs`'s `idle`).
fn click_and_settle(h: &mut Harness<'static, AppModel>, label: &str) {
    h.get_by_label(label).click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
}

/// Tooltips at once, wherever the pointer is (`bottom_panel.rs`'s hover tests).
fn tooltips_now(h: &Harness<'static, AppModel>) {
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
}

const MIP_REASON: &str = "A mip level strays from its source — go to the fix";

/// Review E4 acceptance: an amber set's editor names a reason first, and the reason is a button that moves focus to
/// its fix.
#[test]
fn a_not_ready_set_says_why_on_its_first_line_and_each_reason_moves_focus() {
    use engine::codes::{Code, Warning};
    assert!(
        baseline().1,
        "precondition: rock01 is ready with no injected warning, so VAL-MIP-ERROR is its one reason"
    );
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    let key = rock01(&m).key;
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let line = h.get_by_label("Not ready:");
    let reason = h.get_by_label(MIP_REASON);
    assert!(
        (reason.rect().top() - line.rect().top()).abs() < 4.0,
        "on the same first line"
    );
    click_and_settle(&mut h, MIP_REASON);
    assert_eq!(
        h.state().bottom.filter,
        Some(sdc_gui::model::bottom::ProblemsFilter::Set(key))
    );
    let mark = on_row(
        &h,
        sdc_gui::model::code_text::code_sentence(Code::ValMipError),
        "Mark as intended: ",
    );
    assert!(
        mark.accesskit_node().is_focused(),
        "focus is on the row's fix control"
    );
}

/// A row with a fix: focus goes to the fix, the leftmost of the row's controls, not to Mark as intended.
#[test]
fn a_reason_with_a_fix_focuses_the_fix() {
    use egui::accesskit::Role;
    use engine::codes::{Code, Warning};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ChanNormalLength, "short")]);
    let key = rock01(&m).key;
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let what = sdc_gui::model::code_text::code_line(Code::ChanNormalLength)
        .split(" — ")
        .next()
        .unwrap()
        .to_string();
    click_and_settle(&mut h, &format!("{what} — go to the fix"));
    let sentence = sdc_gui::model::code_text::code_sentence(Code::ChanNormalLength);
    let y = h.get_by_label(sentence).rect().center().y;
    let fix = h
        .query_all_by_role_and_label(Role::Button, sdc_gui::ui::problems::REBUILD_Z)
        .find(|n| (n.rect().center().y - y).abs() <= 4.0)
        .expect("Rebuild Z on the row");
    assert!(fix.accesskit_node().is_focused(), "Rebuild Z has focus");
    assert!(!on_row(&h, sentence, "Mark as intended: ")
        .accesskit_node()
        .is_focused());
}

#[test]
fn a_hidden_reason_shows_its_kind_again_and_takes_focus() {
    use engine::codes::{Code, Warning};
    assert!(
        baseline().1,
        "precondition: rock01 is ready with no injected warning"
    );
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    m.set_kind_hidden(Code::ValMipError, true);
    let key = rock01(&m).key;
    m.select_set(&key);
    let mut h = harness(m);
    tooltips_now(&h);
    h.run();
    h.get_by_label(MIP_REASON).hover();
    h.run();
    h.get_by_label_contains("This kind is hidden; this shows it again.");
    click_and_settle(&mut h, MIP_REASON);
    assert!(h.state().hidden_kinds().is_empty());
    let mark = on_row(
        &h,
        sdc_gui::model::code_text::code_sentence(Code::ValMipError),
        "Mark as intended: ",
    );
    assert!(mark.accesskit_node().is_focused());
}

/// Controller ruling for Task 9: the Normal member leaves, not the diffuse (the set could dissolve); the empty slot's
/// Assign… takes focus.
#[test]
fn a_missing_map_reason_focuses_the_slot_assign_button() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::sets::SlotKind;
    let missing = || {
        Warning::new(Code::ShaderMissingMap, "no _n")
            .with_data(serde_json::json!({"map": "_n", "shader": "pbr"}))
    };
    let (_d, mut m) = with_set_warnings(&[missing()]);
    let (n, _) = normal_member(&m);
    m.remove_member(&rock01(&m).id, &n).unwrap();
    // remove_member plans again, which replaces injected warnings
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    inject_set(&mut m, &[missing()]);
    let v = rock01(&m);
    let text = format!("no {} (Assign…)", v.slot_label(SlotKind::Normal, ""));
    m.select_set(&v.key);
    let mut h = harness(m);
    h.run();
    click_and_settle(&mut h, &format!("{text} — go to the fix"));
    assign_on_normal_row(&h);
}

/// Fix round 1 item 13: the focused Assign… is the Normal row's, so another empty slot's (Height's, one row down)
/// fails. The row's cells are top-aligned with its thumbnail (the slot title is centred, 19 px lower).
fn assign_on_normal_row<S>(h: &Harness<'_, S>) {
    let assign = focused_assign(h);
    let normal = h.get_by_label("Normal _n thumbnail").rect();
    assert!(
        (assign.top() - normal.top()).abs() < 4.0,
        "the Normal row's Assign… has focus: {assign:?} vs {normal:?}"
    );
}

/// The one focused **Assign…** button's rect. The reason button's own label contains "Assign…" too; it is left out, so
/// a focus that stayed on it fails.
fn focused_assign<S>(h: &Harness<'_, S>) -> egui::Rect {
    let focused: Vec<_> = h
        .query_all_by_label_contains("Assign")
        .filter(|n| {
            !n.accesskit_node()
                .label()
                .is_some_and(|l| l.contains("go to the fix"))
        })
        .filter(|n| n.accesskit_node().is_focused())
        .map(|n| n.rect())
        .collect();
    assert_eq!(focused.len(), 1, "one Assign… has focus");
    focused[0]
}

/// The labels of the focused nodes.
fn focused_labels<S>(h: &Harness<'_, S>) -> Vec<String> {
    h.query_all_by_label_contains("")
        .filter(|n| n.accesskit_node().is_focused())
        .filter_map(|n| n.accesskit_node().label().map(|l| l.to_string()))
        .collect()
}

/// rock01 with its Normal member gone and the SHADER-MISSING-MAP row for it (`a_missing_map_reason_…`), and the
/// reason's button name.
fn missing_normal() -> (tempfile::TempDir, AppModel, String) {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::sets::SlotKind;
    let missing = || {
        Warning::new(Code::ShaderMissingMap, "no _n")
            .with_data(serde_json::json!({"map": "_n", "shader": "pbr"}))
    };
    let (d, mut m) = with_set_warnings(&[missing()]);
    let (n, _) = normal_member(&m);
    m.remove_member(&rock01(&m).id, &n).unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    inject_set(&mut m, &[missing()]);
    let v = rock01(&m);
    let name = format!(
        "no {} (Assign…) — go to the fix",
        v.slot_label(SlotKind::Normal, "")
    );
    (d, m, name)
}

/// Fix round 1: the set editor alone, `width` wide (no GPU: `View::three_d` is read from the settings, as
/// `ui::draw` reads it when the 3D panel can show), and the column it was given.
struct Editor {
    m: AppModel,
    key: String,
    thumbs: sdc_gui::ui::thumbs::ThumbCache,
    cmds: sdc_gui::ui::slot_commands::SlotCmdState,
    drops: sdc_gui::ui::sets_column::DropState,
    column: egui::Rect,
}

fn editor_harness(m: AppModel, key: String, width: f32) -> Harness<'static, Editor> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(width + 40.0, 900.0))
        .build_ui_state(
            move |ui, e: &mut Editor| {
                let view = sdc_gui::ui::set_editor::View {
                    three_d: e.m.settings.set_view_3d,
                    ..Default::default()
                };
                let r = ui.allocate_ui(egui::vec2(width, ui.available_height()), |ui| {
                    ui.set_width(width);
                    sdc_gui::ui::set_editor::draw(
                        ui,
                        &mut e.m,
                        &e.key,
                        &mut e.thumbs,
                        &mut e.cmds,
                        &mut e.drops,
                        view,
                    );
                });
                e.column = r.response.rect;
            },
            Editor {
                m,
                key,
                thumbs: Default::default(),
                cmds: Default::default(),
                drops: Default::default(),
                column: egui::Rect::NOTHING,
            },
        )
}

/// Fix round 1 item 1: in 3D a slot reason switches the editor to 2D, outlines the slot and focuses its Assign….
#[test]
fn a_slot_reason_clicked_in_3d_shows_the_slot_in_2d() {
    use sdc_gui::model::nav::SlotTarget;
    use sdc_gui::model::sets::SlotKind;
    let (_d, mut m, name) = missing_normal();
    m.settings.set_view_3d = true;
    let key = rock01(&m).key;
    let mut h = editor_harness(m, key, 1200.0);
    h.run();
    assert!(
        h.query_by_label("Normal _n").is_none(),
        "precondition: 3D draws no slot rows"
    );
    h.get_by_label(&name).click();
    h.run();
    assert!(!h.state().m.settings.set_view_3d, "the editor is in 2D");
    h.run();
    assert_eq!(
        sdc_gui::ui::row_reveal::revealed(&h.ctx).map(|r| r.0),
        Some(SlotTarget::Slot(SlotKind::Normal))
    );
    assign_on_normal_row(&h);
}

/// Fix round 1 item 2: rock01 is PBR Landscape (CS), which draws no slot 3 card, so a missing `_g` (Emissive, slot
/// 3) has no Assign… to go to: its reason opens the Problems row instead.
#[test]
fn a_slot_reason_with_no_slot_row_opens_its_problems_row() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{set_reasons, ReasonGo};
    use sdc_gui::model::sets::{shared_cells, slot_row, SlotKind};
    assert!(
        baseline().1,
        "precondition: rock01 is ready with no injected warning"
    );
    let (_d, mut m) = with_set_warnings(&[Warning::new(Code::ShaderMissingMap, "no _g")
        .with_data(serde_json::json!({"map": "_g", "shader": "pbr"}))]);
    let v = rock01(&m);
    assert!(
        !shared_cells(&v, slot_row(v.shader.as_ref(), &v.slots))
            .iter()
            .any(|c| c.kind.shares_card(SlotKind::Emissive) || c.kind == SlotKind::Emissive),
        "precondition: the editor draws no slot 3 row"
    );
    let r = set_reasons(&m, &v.key);
    assert_eq!(r.len(), 1, "{r:?}");
    let ReasonGo::Row(p) = &r[0].go else {
        panic!("a slot the editor does not draw is a Row reason: {r:?}");
    };
    let shown = p.shown().to_string();
    let name = format!("{} — go to the fix", r[0].text);
    m.select_set(&v.key);
    let mut h = harness(m);
    h.run();
    click_and_settle(&mut h, &name);
    assert_eq!(
        h.state().bottom.filter,
        Some(sdc_gui::model::bottom::ProblemsFilter::Set(v.key.clone()))
    );
    assert!(
        h.query_all_by_label_contains(&shown).count() >= 1,
        "the row is listed"
    );
}

/// Fix round 1 item 3: two members with the same coded row: two buttons, each named for its file, each to its row.
#[test]
fn two_reasons_with_one_text_are_named_for_their_files() {
    use engine::codes::{Code, Warning};
    let w = Warning::new(Code::ValMipError, "mip 3");
    let (_d, mut m) = with_warnings(std::slice::from_ref(&w));
    let v = rock01(&m);
    let (a, b) = (v.members[0].clone(), v.members[1].clone());
    m.items.iter_mut().find(|i| i.key == b).unwrap().warnings = vec![w];
    m.invalidate_views();
    m.select_set(&v.key);
    let mut h = harness(m);
    h.run();
    let text = MIP_REASON.trim_end_matches(" — go to the fix");
    let sentence = sdc_gui::model::code_text::code_sentence(Code::ValMipError);
    for k in [&a, &b] {
        let name = format!("{text} ({k}) — go to the fix");
        h.get_by_label(&name);
        click_and_settle(&mut h, &name);
        let mark = format!("Mark as intended: {k}: {sentence}");
        assert!(
            h.get_by_label(&mark).accesskit_node().is_focused(),
            "{k}'s row has focus"
        );
    }
    assert!(h.query_by_label(MIP_REASON).is_none());
}

/// Fix round 1 item 4: a set the export blocks on has the red dot on its first line, as on its card.
#[test]
fn a_blocked_sets_status_line_has_the_red_dot() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_json_value(
        "landscape/rock01",
        "slot3",
        Some(serde_json::json!(r"..\g.dds")),
    );
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let blocks = sdc_gui::model::pbr_export::export_blocks(&m);
    assert!(
        blocks.iter().any(|i| i.set_id == "landscape/rock01"),
        "precondition: {blocks:?}"
    );
    let key = rock01(&m).key;
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let line = h.get_by_label("Not ready:").rect();
    let dots: Vec<egui::Rect> = h
        .query_all_by_label("blocks export")
        .map(|n| n.rect())
        .collect();
    assert!(
        dots.iter()
            .any(|r| (r.center().y - line.center().y).abs() < 4.0 && r.right() <= line.left()),
        "the status line's dot: {dots:?} beside {line:?}"
    );
    assert!(
        dots.iter().any(|r| in_sets_column(&h, *r)),
        "the card's dot: {dots:?}"
    );
}

/// Fix round 1 item 5: a removal takes focus from its reason but never runs on Enter or Space; a click still runs it.
#[test]
fn a_removal_focused_by_a_reason_ignores_enter_and_space() {
    use sdc_gui::model::bottom::{set_reasons, ReasonGo};
    const GONE: &str = "landscape/notes.txt";
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    std::fs::write(m.root.clone().unwrap().join(GONE), "notes").unwrap();
    m.edit_docs("t", None, true, |_, s| {
        s.add_member("landscape/rock01", GONE)
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let key = rock01(&m).key;
    let r = set_reasons(&m, &key);
    let reason = r
        .iter()
        .find(|r| matches!(&r.go, ReasonGo::Row(p) if format!("{:?}", p.fix).contains("RemoveMissing")))
        .unwrap_or_else(|| panic!("a missing-entry reason: {r:?}"));
    let name = format!("{} — go to the fix", reason.text);
    let listed = |m: &AppModel| m.sets_doc.as_ref().unwrap().text().contains("notes.txt");
    assert!(listed(&m));
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    click_and_settle(&mut h, &name);
    let remove = "Remove landscape/notes.txt from landscape/rock01";
    assert!(
        h.get_by_label(remove).accesskit_node().is_focused(),
        "focus shows where the fix is"
    );
    for k in [egui::Key::Enter, egui::Key::Space] {
        h.key_press(k);
        h.run();
        assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
        h.run();
        assert!(listed(h.state()), "{k:?} removed nothing");
    }
    click_and_settle(&mut h, remove);
    assert!(!listed(h.state()), "a click removes the entry");
}

/// Fix round 1 items 6 and 7: Rebuild Z is off (already on), so neither it nor Mark as intended takes focus; the row
/// does (Enter and Space on it write nothing: it has no suggestion and asks no alpha).
#[test]
fn a_disabled_fix_takes_no_focus_and_gives_none_to_mark() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{set_reasons, ReasonGo};
    let w = Warning::new(Code::ChanNormalLength, "short");
    let (_d, mut m) = with_warnings(std::slice::from_ref(&w));
    let k = rock01(&m).members[0].clone();
    m.set_field(&k, "channels.reconstruct_z", serde_json::json!(true));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    inject(&mut m, &[w]);
    let key = rock01(&m).key;
    let ReasonGo::Row(p) = set_reasons(&m, &key).remove(0).go else {
        panic!("a Row reason");
    };
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let what = sdc_gui::model::code_text::code_line(Code::ChanNormalLength)
        .split(" — ")
        .next()
        .unwrap()
        .to_string();
    click_and_settle(&mut h, &format!("{what} — go to the fix"));
    let sentence = sdc_gui::model::code_text::code_sentence(Code::ChanNormalLength);
    let y = h.get_by_label(sentence).rect().center().y;
    let fix = h
        .query_all_by_label(sdc_gui::ui::problems::REBUILD_Z)
        .find(|n| (n.rect().center().y - y).abs() <= 4.0)
        .expect("Rebuild Z on the row");
    assert!(fix.accesskit_node().is_disabled(), "precondition");
    assert!(!fix.accesskit_node().is_focused());
    assert!(!on_row(&h, sentence, "Mark as intended: ")
        .accesskit_node()
        .is_focused());
    assert_eq!(
        focused_labels(&h),
        vec![format!("{k}: {}", p.shown())],
        "the row has focus"
    );
}

/// Fix round 1 items 6 and 7: on a read-only project Mark as intended is off, the row's only control: it takes no
/// focus and the row does.
#[test]
fn a_reason_whose_only_control_is_off_focuses_the_row() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{set_reasons, ReasonGo};
    let w = Warning::new(Code::ChanNotGray, "not grey");
    let (_d, mut m) = with_warnings(std::slice::from_ref(&w));
    assert!(m.save().is_ok());
    let file = m.root.clone().unwrap().join("sdc-project.toml");
    std::fs::write(&file, "[project\n").unwrap();
    m.reload_documents();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    inject(&mut m, &[w]);
    let key = rock01(&m).key;
    let k = rock01(&m).members[0].clone();
    let ReasonGo::Row(p) = set_reasons(&m, &key).remove(0).go else {
        panic!("a Row reason");
    };
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let what = sdc_gui::model::code_text::code_line(Code::ChanNotGray)
        .split(" — ")
        .next()
        .unwrap()
        .to_string();
    click_and_settle(&mut h, &format!("{what} — go to the fix"));
    let sentence = sdc_gui::model::code_text::code_sentence(Code::ChanNotGray);
    let mark = on_row(&h, sentence, "Mark as intended: ");
    assert!(mark.accesskit_node().is_disabled(), "precondition");
    assert_eq!(
        focused_labels(&h),
        vec![format!("{k}: {}", p.shown())],
        "the row has focus"
    );
}

/// Fix round 1 item 7: an Undecided row (the `opened` fixture's rock01) has no fix and no Mark, and Enter or Space on
/// it accepts its suggested sub-type (a decision): nothing on the row takes focus.
#[test]
fn an_undecided_reason_gives_its_row_no_focus() {
    use sdc_gui::model::bottom::ReasonGo;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let v = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(|v| v.id == "landscape/rock01")
        .unwrap();
    let r = sdc_gui::model::bottom::set_reasons(&m, &v.key);
    let ReasonGo::Row(p) = &r[0].go else {
        panic!("a Row reason: {r:?}");
    };
    assert!(
        p.suggested.is_some() && p.fix.is_none() && p.mark.is_none(),
        "precondition: {p:?}"
    );
    let sdc_gui::model::bottom::At::Texture(k) = &p.at else {
        panic!("a texture row");
    };
    // Two members are undecided, so each reason names its file (item 3).
    let name = format!("{} ({k}) — go to the fix", r[0].text);
    m.select_set(&v.key);
    let mut h = harness(m);
    h.run();
    click_and_settle(&mut h, &name);
    let row = format!("{k}: {}", p.shown());
    let y = h.get_by_label(&row).rect().center().y;
    let on_it: Vec<String> = h
        .query_all_by_label_contains("")
        .filter(|n| n.accesskit_node().is_focused() && (n.rect().center().y - y).abs() <= 4.0)
        .filter_map(|n| n.accesskit_node().label().map(|l| l.to_string()))
        .collect();
    assert!(on_it.is_empty(), "nothing on the row has focus: {on_it:?}");
}

/// Fix round 1 item 8: a row focus asked for while Problems is not drawn is forgotten with the row focus.
#[test]
fn a_waiting_fix_focus_is_forgotten_while_problems_is_hidden() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{set_reasons, ProblemsFilter, ReasonGo};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    let key = rock01(&m).key;
    let ReasonGo::Row(p) = set_reasons(&m, &key).remove(0).go else {
        panic!("a Row reason");
    };
    m.bottom.open = false;
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    sdc_gui::ui::problems::focus_row(&h.ctx, sdc_gui::ui::problems::row_id(&p));
    h.run();
    h.state_mut().open_problems(Some(ProblemsFilter::Set(key)));
    h.run();
    let mark = on_row(
        &h,
        sdc_gui::model::code_text::code_sentence(Code::ValMipError),
        "Mark as intended: ",
    );
    assert!(!mark.accesskit_node().is_focused());
}

/// Fix round 1 item 11: "+k more in Problems" shows the hidden kinds of the rows it counts.
#[test]
fn more_in_problems_shows_the_hidden_rows_it_counts() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{set_reasons, ReasonGo};
    let (_d, mut m) = with_warnings(&[
        Warning::new(Code::ValMipError, "mip 3"),
        Warning::new(Code::ValCoverage, "coverage"),
        Warning::new(Code::LossReencode, "again"),
        Warning::new(Code::ChanNotGray, "colour"),
    ]);
    let key = rock01(&m).key;
    let r = set_reasons(&m, &key);
    let ReasonGo::Row(p) = &r.last().unwrap().go else {
        panic!("a Row reason: {r:?}");
    };
    let code = p.code.unwrap();
    let shown = p.shown().to_string();
    m.set_kind_hidden(code, true);
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let more = format!("+{} more in Problems", r.len() - 3);
    click_and_settle(&mut h, &more);
    assert!(h.state().hidden_kinds().is_empty());
    assert!(
        h.query_all_by_label_contains(&shown).count() >= 1,
        "the row is listed"
    );
}

/// Fix round 1 item 12: at the centre's floor width, long reasons stay inside the column, none taller than two lines.
#[test]
fn the_status_line_fits_the_centres_floor_width() {
    use engine::codes::{Code, Warning};
    let (_d, m) = with_warnings(&[
        Warning::new(Code::LossReencode, "again"),
        Warning::new(Code::TagContentMismatch, "tag"),
        Warning::new(Code::InNonfinite, "nan"),
    ]);
    let key = rock01(&m).key;
    let reasons = sdc_gui::model::bottom::set_reasons(&m, &key);
    assert!(reasons.len() >= 3, "{reasons:?}");
    assert!(
        reasons.iter().take(3).all(|r| r.text.chars().count() >= 40),
        "{reasons:?}"
    );
    let mut h = editor_harness(m, key, sdc_gui::ui::set_preview::CENTRE_MIN);
    h.run();
    let column = h.state().column;
    let line = h.get_by_label("Not ready:").rect().height();
    let buttons: Vec<egui::Rect> = h
        .query_all_by_label_contains(" — go to the fix")
        .map(|n| n.rect())
        .collect();
    assert_eq!(buttons.len(), 3);
    for b in &buttons {
        assert!(
            b.left() >= column.left() - 0.5 && b.right() <= column.right() + 0.5,
            "{b:?} inside {column:?}"
        );
        assert!(
            b.height() <= 2.0 * line + 8.0,
            "{b:?} at most two lines of {line}"
        );
    }
}

/// The DERIVE-NO-ALBEDO slot case: no production row reaches it today (the engine raises the code only from the Also
/// export planner), so the row is injected, as Task 8's target test does.
#[test]
fn a_no_albedo_set_row_is_a_slot_reason() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{set_reasons, ReasonGo};
    use sdc_gui::model::sets::SlotKind;
    let (_d, m) = with_set_warnings(&[Warning::new(Code::DeriveNoAlbedo, "no albedo")]);
    let r = set_reasons(&m, &rock01(&m).key);
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0].go, ReasonGo::Slot(SlotKind::Diffuse));
    assert_eq!(r[0].text, "no Diffuse (Assign…)");
}

/// Four reasons: three buttons, then a link to the rest.
#[test]
fn more_than_three_reasons_link_to_problems() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = with_warnings(&[
        Warning::new(Code::ValMipError, "mip 3"),
        Warning::new(Code::ValCoverage, "coverage"),
        Warning::new(Code::LossReencode, "again"),
        Warning::new(Code::ChanNotGray, "colour"),
    ]);
    let key = rock01(&m).key;
    assert_eq!(
        sdc_gui::model::bottom::set_reasons(&m, &key).len(),
        4 + usize::from(!baseline().1)
    );
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    assert_eq!(h.query_all_by_label_contains(" — go to the fix").count(), 3);
    let more = format!("+{} more in Problems", 1 + usize::from(!baseline().1));
    click_and_settle(&mut h, &more);
    assert_eq!(
        h.state().bottom.filter,
        Some(sdc_gui::model::bottom::ProblemsFilter::Set(key))
    );
}

#[test]
fn a_set_with_only_notes_reads_ready() {
    use engine::codes::{Code, Warning};
    assert!(
        baseline().1,
        "precondition: rock01 is ready with no injected warning"
    );
    let (_d, mut m) = with_warnings(&[Warning::new(Code::DimNpot, "npot")]);
    let key = rock01(&m).key;
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Ready: nothing in this set needs you.");
    assert!(
        h.query_by_label_contains("in this set — Show in Problems")
            .is_none(),
        "the old line is gone (CA18)"
    );
}

/// Controller ruling 10: the filter pills and the card's status hover both say what ready means.
#[test]
fn the_pills_and_the_card_say_what_ready_means() {
    use engine::codes::{Code, Warning};
    use sdc_gui::ui::sets_column::{pill_hover, NOT_READY_MEANS, READY_MEANS};
    use sdc_gui::ui::status::Status;
    let (_d, m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    let v = rock01(&m);
    assert!(pill_hover(&m, &v, Status::Warning).contains(NOT_READY_MEANS));
    assert!(pill_hover(&m, &v, Status::Ready).contains(READY_MEANS));
    assert!(!pill_hover(&m, &v, Status::Blocks).contains(READY_MEANS));
    assert!(!pill_hover(&m, &v, Status::Blocks).contains(NOT_READY_MEANS));
    let mut h = harness(m);
    tooltips_now(&h);
    h.run();
    h.get_by_label_contains("Not ready ").hover();
    h.run();
    h.get_by_label(&format!("Only sets that are not ready. {NOT_READY_MEANS}"));
    h.get_by_label_contains("Ready ").hover();
    h.run();
    h.get_by_label(&format!("Only sets that are ready. {READY_MEANS}"));
}

/// Review fix 5: the app reads the OS value at start.
#[test]
fn restore_ctx_reads_the_os_motion_setting() {
    let d = tempfile::tempdir().unwrap();
    let app = sdc_gui::app::SdcApp::new(opened(d.path()));
    let ctx = egui::Context::default();
    assert_eq!(sdc_gui::ui::motion::os_value(&ctx), None);
    app.restore_ctx(&ctx);
    assert!(sdc_gui::ui::motion::os_value(&ctx).is_some());
}
