//! Review E: Problems and Notes. Every code is a Problem or a Note once; a Note row says why it is fine.
mod support;
use eframe::egui;
use egui_kittest::kittest::Queryable;
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
    m.unmatched_keys = vec!["gone.png".into()];
    m.invalidate_views();
    let ready = sdc_gui::model::bottom::flagged(&m).ready_rows;
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
