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

const NORMAL: &str = "landscape/rock01_normal.png";

/// B3: rock01 on plain `pbr` with its own normal decided as vanilla `normal`: the engine's real Info, no injection.
fn plain_normal_in_pbr(dir: &std::path::Path) -> AppModel {
    let mut m = pbr_opened_plain(dir);
    m.decide(
        NORMAL,
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Normal),
    );
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m
}

/// N-8, N-9 (Note JS4): rock01 on plain `pbr` with its own normal decided `normal` has no counted row and reads Ready
/// exactly when the same set with a `pbr_normal` does (baseline `pbr_opened_plain`).
#[test]
fn a_plain_normal_in_a_pbr_set_counts_nothing_and_the_set_can_be_ready() {
    use sdc_gui::model::bottom::{counts_for, set_ready, At};
    let base_dir = tempfile::tempdir().unwrap();
    let plain_dir = tempfile::tempdir().unwrap();
    let base = pbr_opened_plain(base_dir.path());
    let plain = plain_normal_in_pbr(plain_dir.path());
    let counted = |m: &AppModel| {
        let v = rock01(m);
        let at = At::Set {
            key: v.key.clone(),
            id: v.id.clone(),
        };
        v.set_warnings
            .iter()
            .filter(|w| counts_for(m, &at, w))
            .count()
    };
    assert!(
        set_ready(&base, &rock01(&base)),
        "precondition: the pbr_normal baseline is Ready"
    );
    assert_eq!(counted(&plain), counted(&base), "needs _n no longer counts");
    assert_eq!(counted(&plain), 0, "no counted row is left");
    assert_eq!(
        set_ready(&plain, &rock01(&plain)),
        set_ready(&base, &rock01(&base))
    );
    assert!(
        !rock01(&plain)
            .set_warnings
            .iter()
            .any(|w| w.code == engine::codes::Code::ShaderExtraMap
                || (w.code == engine::codes::Code::ShaderMissingMap && w.data["map"] == "_n")),
        "{:?}",
        rock01(&plain).set_warnings
    );
}

/// `textures/test/wall.png` decided `pbr_albedo` beside a kept `wall_n.dds` (no job: typed by its `_n` suffix as a
/// vanilla normal), opened as the app opens a folder, with the set's shader picked `pbr`.
fn kept_dds_normal_wall(dir: &std::path::Path) -> AppModel {
    let r = dir.join("textures/test");
    std::fs::create_dir_all(&r).unwrap();
    image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb([120u8, 90, 60]))
        .save(r.join("wall.png"))
        .unwrap();
    bc1_dds(&r.join("wall_n.dds"));
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    let id = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.ends_with("wall"))
        .expect("the set")
        .id
        .clone();
    m.set_shader(&id, Some(engine::shader::Shader::Pbr));
    m.decide(
        "test/wall.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::PbrAlbedo),
    );
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m
}

/// N-9 (Note JS4), a pin: a kept DDS normal in a PBR set fills its `_n`: no EXTRA row and no "needs _n" row. (The
/// set's own "needs _rmaos" is a SHADER-MISSING-MAP row too, so the match names the `_n`.)
#[test]
fn a_kept_dds_normal_in_a_pbr_set_has_no_shader_row() {
    use engine::codes::Code;
    let d = tempfile::tempdir().unwrap();
    let m = kept_dds_normal_wall(d.path());
    let rows = sdc_gui::model::bottom::problems(&m);
    assert!(
        !rows.iter().any(|p| p.code == Some(Code::ShaderExtraMap)
            || (p.code == Some(Code::ShaderMissingMap) && p.text.contains("needs _n"))),
        "{rows:?}"
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

/// Controller ruling (2026-11-18): the code sentence no longer sends the user to the recipe combo, which is gone (A1-1).
#[test]
fn the_unused_input_sentence_names_the_shader_and_the_type() {
    assert_eq!(
        sdc_gui::model::code_text::code_sentence(engine::codes::Code::PackUnusedInput),
        "A packing input isn't used by this set — remove it, or change the set's shader or the file's type"
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
    // Final fix wave B2: no pill of any count; the tab ("Problems · Notes (1)") is the one label that names it.
    use sdc_gui::ui::problems::NOTES_PILL;
    let pills: Vec<String> = h
        .query_all_by_label_contains(&format!("{NOTES_PILL} ("))
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.starts_with(NOTES_PILL))
        .collect();
    assert!(pills.is_empty(), "no pill: {pills:?}");
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

/// Final review Minor 3: a `texture:` or `file:` subject folds its key as `[files]` keys fold (Note BF), so a mark made
/// on `Rock_N.png` still matches once the file's key differs only in case; the stored text stays as written.
#[test]
fn a_file_mark_matches_its_key_in_any_case() {
    let (_d, mut doc) = doc_with("[project]\n");
    for (made, now) in [
        (
            "texture:Landscape/Rock_N.png",
            "texture:landscape/rock_n.png",
        ),
        ("file:Landscape/Rock_N", "file:landscape/rock_n"),
    ] {
        doc.set_intended(&key("CHAN-NOT-GRAY", made, "s"), true);
        assert!(doc.text().contains(made), "stored as written");
        assert!(
            doc.is_intended(&key("CHAN-NOT-GRAY", now, "s")),
            "{made} matches {now}"
        );
        doc.set_intended(&key("CHAN-NOT-GRAY", now, "s"), false);
        assert!(doc.intended().is_empty(), "{now} takes {made}'s mark off");
    }
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

/// Final fix wave A5: only a removed first entry tidies the new first one. A hand-written first entry keeps its
/// leading space when a later mark goes.
#[test]
fn removing_a_later_mark_keeps_the_first_entry_as_written() {
    let (_d, mut doc) = doc_with(concat!(
        "[project]\nintended = [ 42, ",
        "{ code = \"C1\", subject = \"texture:landscape/rock01.png\", sentence = \"a\" }]\n",
    ));
    doc.set_intended(&key("C1", T, "a"), false);
    assert!(doc.text().contains("intended = [ 42]"), "{}", doc.text());
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

/// Final fix wave R2 (final review Minor 1): the set editor's status line scans its reasons once per change: frames
/// with no change scan none; a mark, a hidden kind or another set scan once each.
#[test]
fn the_status_line_scans_its_reasons_once_per_change() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::reason_scans;
    let (_d, mut m) = with_warnings(&[
        Warning::new(Code::ValMipError, "mip 3"),
        Warning::new(Code::ValCoverage, "coverage 0.4"),
    ]);
    let key = rock01(&m).key;
    let other = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key != key && !v.key.is_empty())
        .expect("another set")
        .key
        .clone();
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let steps = |h: &mut Harness<'static, AppModel>| {
        for _ in 0..5 {
            h.step();
        }
        reason_scans(h.state())
    };
    let n = steps(&mut h);
    assert!(n >= 1, "the status line read its reasons");
    assert_eq!(steps(&mut h), n, "five frames, no change: no scan");
    mark_first(h.state_mut(), Code::ValMipError);
    assert_eq!(steps(&mut h), n + 1, "a mark");
    h.state_mut().set_kind_hidden(Code::ValCoverage, true);
    assert_eq!(steps(&mut h), n + 2, "a hidden kind");
    h.state_mut().select_set(&other);
    assert_eq!(steps(&mut h), n + 3, "another set");
}

/// Final wave Group 7 (ledger 253, Q6 rank 1): the chip of a set filter resolves its set once per change of the
/// filter or the views, not on every frame it is drawn.
#[test]
fn the_set_filter_chip_resolves_its_set_once_per_change() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{scope_label_lookups, ProblemsFilter};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    let rock = rock01(&m);
    let other = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key != rock.key && !v.key.is_empty())
        .expect("another set")
        .clone();
    m.open_problems(Some(ProblemsFilter::Set(rock.key.clone())));
    let mut h = harness(m);
    h.run();
    h.get_by_label(&format!("Set: {} ×", rock.id));
    let steps = |h: &mut Harness<'static, AppModel>| {
        for _ in 0..5 {
            h.step();
        }
        scope_label_lookups(h.state())
    };
    let n = steps(&mut h);
    assert!(n >= 1, "the chip resolved its set");
    assert_eq!(steps(&mut h), n, "five frames, no change: no lookup");
    h.state_mut()
        .open_problems(Some(ProblemsFilter::Set(other.key.clone())));
    assert_eq!(steps(&mut h), n + 1, "another filter");
    h.get_by_label(&format!("Set: {} ×", other.id));
    h.state_mut().invalidate_views();
    assert_eq!(steps(&mut h), n + 2, "new views");
    h.get_by_label(&format!("Set: {} ×", other.id));
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
        problems(&m).iter().any(|p| p.carried
            && p.level == sdc_gui::model::bottom::Level::Warning
            && p.text.contains("Delete wins")),
        "precondition: the Delete-wins pitfall row"
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
            (At::Nif(_), _) => sdc_gui::model::bottom::kind::NIF_WARNING,
            (_, Some(c)) => c.as_str(),
            _ => "",
        };
        doc.set_intended(&intended_key(&p.at, kind, &p.shown()), true);
    }
    assert_eq!(doc.intended().len(), rows.len(), "every key stored");
    // Final fix wave B4: drop the cached rows, so the checks below read rows built with the stored keys.
    m.invalidate_views();
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

/// Final fix wave A6: a mark is read only for a code that can be marked (Warning or Info, `markable`). A hand-written
/// mark on an Error code is not one: the warning still counts.
#[test]
fn a_hand_written_mark_on_an_error_code_still_counts() {
    use engine::codes::{Code, Severity, Warning};
    use sdc_gui::model::bottom::{counts_for, intended_key, problems};
    assert_eq!(
        Code::TagSrgbVanilla.severity(),
        Severity::Error,
        "precondition"
    );
    let (_d, mut m) = with_warnings(&[Warning::new(Code::TagSrgbVanilla, "vanilla sRGB")]);
    let p = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::TagSrgbVanilla))
        .cloned()
        .expect("the row");
    assert!(
        p.mark.is_none(),
        "precondition: the row has no Mark as intended"
    );
    let key = rock01(&m).members[0].clone();
    let w = m.item(&key).unwrap().warnings[0].clone();
    assert!(counts_for(&m, &p.at, &w), "precondition: it counts");
    m.sets_doc.as_mut().unwrap().set_intended(
        &intended_key(&p.at, Code::TagSrgbVanilla.as_str(), &p.shown()),
        true,
    );
    m.invalidate_views();
    assert!(
        counts_for(&m, &p.at, &w),
        "a mark on an Error code is not read"
    );
}

/// Final fix wave A6: the tile hover lists a marked warning under "Marked as intended:", after the unmarked ones; with
/// none unmarked, only that line and the marked ones.
#[test]
fn the_tile_hover_lists_a_marked_warning_under_its_line() {
    use engine::codes::{Code, Warning};
    use sdc_gui::ui::columns::tile_tooltip;
    let (_d, mut m) = with_warnings(&[
        Warning::new(Code::ValMipError, "mip 3"),
        Warning::new(Code::ValCoverage, "coverage 0.4"),
    ]);
    let key = rock01(&m).members[0].clone();
    let tip = |m: &AppModel| tile_tooltip(m, m.item(&key).unwrap(), false);
    assert!(tip(&m).ends_with("\nmip 3\ncoverage 0.4"), "{}", tip(&m));
    mark_first(&mut m, Code::ValMipError);
    assert!(
        tip(&m).ends_with("\ncoverage 0.4\nMarked as intended:\nmip 3"),
        "{}",
        tip(&m)
    );
    mark_first(&mut m, Code::ValCoverage);
    assert_eq!(
        tip(&m),
        format!("{key}\nMarked as intended:\nmip 3\ncoverage 0.4")
    );
}

/// Final fix wave A6: a Files row's "n warnings" hover lists a marked warning under "Marked as intended:", after the
/// unmarked ones.
#[test]
fn the_files_rows_warning_hover_lists_a_marked_warning_under_its_line() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::export::warning_line;
    use sdc_gui::ui::drawer::warnings_hover;
    let (_d, mut m) = with_warnings(&[]);
    let (mip, cov) = (
        Warning::new(Code::ValMipError, "mip 3"),
        Warning::new(Code::ValCoverage, "coverage 0.4"),
    );
    m.export.rows = vec![sdc_gui::model::export::Row {
        job_key: "derived/x".into(),
        path: "derived/x".into(),
        state: sdc_gui::model::export::RowState::Done { passed: true },
        warnings: vec![mip.clone(), cov.clone()],
    }];
    m.invalidate_views();
    let hover = |m: &AppModel| warnings_hover(m, &m.export.rows[0]);
    assert_eq!(
        hover(&m),
        format!("{}\n{}", warning_line(&mip), warning_line(&cov))
    );
    mark_first(&mut m, Code::ValMipError);
    assert_eq!(
        hover(&m),
        format!(
            "{}\nMarked as intended:\n{}",
            warning_line(&cov),
            warning_line(&mip)
        )
    );
}

/// Final fix wave A6: a Files row's warning that its texture also has is listed, and marked, on the texture's row
/// (the File row is left out); the Files hover reads that mark.
#[test]
fn the_files_hover_reads_the_mark_of_the_textures_row() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::export::warning_line;
    use sdc_gui::ui::drawer::warnings_hover;
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    let key = rock01(&m).members[0].clone();
    let job = m.item(&key).unwrap().job_key.clone();
    let file = Warning::new(Code::ValMipError, "mip 3 in the file");
    m.export.rows = vec![sdc_gui::model::export::Row {
        job_key: job.clone(),
        path: job,
        state: sdc_gui::model::export::RowState::Done { passed: true },
        warnings: vec![file.clone()],
    }];
    m.invalidate_views();
    mark_first(&mut m, Code::ValMipError);
    assert_eq!(
        warnings_hover(&m, &m.export.rows[0]),
        format!("Marked as intended:\n{}", warning_line(&file))
    );
}

/// Plan 3c Task 17a (F3): a Files row's "n warnings" and the Has warnings filter leave a warning marked as intended
/// out, as its hover and the card badge do; a mark made while the filter is on takes the row out at once.
#[test]
fn a_marked_warning_leaves_the_files_count_and_filter() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::BottomTab;
    use sdc_gui::model::export::{warning_line, Row, RowState};
    let (_d, mut m) = with_warnings(&[]);
    let (mip, cov) = (
        Warning::new(Code::ValMipError, "mip 3"),
        Warning::new(Code::ValCoverage, "coverage 0.4"),
    );
    m.export.rows = vec![Row {
        job_key: "derived/x".into(),
        path: "derived/x".into(),
        state: RowState::Done { passed: true },
        warnings: vec![mip.clone(), cov.clone()],
    }];
    m.invalidate_views();
    m.show_tab(BottomTab::Files);
    mark_first(&mut m, Code::ValMipError);
    let mut h = harness(m);
    h.run();
    h.run();
    assert_eq!(
        h.query_all_by_label("2 warnings").count(),
        0,
        "the marked warning is not counted"
    );
    h.get_by_label("1 warning");
    h.get_by_label("Has warnings").click();
    h.run();
    h.run();
    h.get_by_label("derived/x"); // one warning still open: listed
    mark_first(h.state_mut(), Code::ValCoverage);
    h.run();
    h.run();
    assert!(
        h.query_by_label("derived/x").is_none(),
        "every warning marked: gone from Has warnings"
    );
    // Ruling 3c-11: with the filter off, the row shows a weak "2 marked" whose hover lists both under their line.
    h.get_by_label("Has warnings").click();
    h.run();
    h.run();
    for open in ["1 warning", "2 warnings"] {
        assert_eq!(
            h.query_all_by_label(open).count(),
            0,
            "no count of open warnings"
        );
    }
    h.get_by_label("2 marked").hover();
    h.run();
    h.run();
    h.get_by_label(&format!(
        "Marked as intended:\n{}\n{}",
        warning_line(&mip),
        warning_line(&cov)
    ));
}

#[test]
fn has_warnings_tracks_the_texture_now_owning_an_export_warning() {
    use engine::codes::Code;
    use sdc_gui::model::bottom::BottomTab;
    use sdc_gui::model::export::{Row, RowState};

    let warning = Warning::new(Code::ValMipError, "mip 3");
    let (_d, mut m) = with_warnings(std::slice::from_ref(&warning));
    let key = rock01(&m).members[0].clone();
    mark_first(&mut m, warning.code);
    m.export.rows = vec![Row {
        job_key: "derived/x".into(),
        path: "derived/x".into(),
        state: RowState::Done { passed: true },
        warnings: vec![warning],
    }];
    m.show_tab(BottomTab::Files);
    let mut h = harness(m);
    h.get_by_label("Has warnings").click();
    h.run();
    h.get_by_label("derived/x");

    let m = h.state_mut();
    m.items.iter_mut().find(|i| i.key == key).unwrap().job_key = "derived/x".into();
    m.invalidate_views();
    h.run();
    assert!(
        h.query_by_label("derived/x").is_none(),
        "the same export warning now belongs to the marked texture"
    );
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
    let (k, member) = normal_member(&m);
    inject_set(&mut m, &[extra_map(&member)]);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    let note = extra_note(&member);
    let remove = format!("Remove {member} from landscape/rock01");
    // Controller ruling 2: left to right, the fix, the second fix, then Go to set.
    let xs: Vec<f32> = [
        remove.as_str(),
        "Change shader for landscape/rock01 (SHADER-EXTRA-MAP)",
        "Go to set",
    ]
    .iter()
    .map(|l| on_row(&h, &note, l).rect().center().x)
    .collect();
    assert!(xs[0] < xs[1] && xs[1] < xs[2], "row order: {xs:?}");
    // Final fix wave A8: the hover says where the file goes (Note JN5).
    tooltips_now(&h);
    on_row(&h, &note, &remove).hover();
    h.run();
    h.get_by_label(&remove_hover(&member));
    on_row(&h, &note, &remove).click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert!(!rock01(h.state())
        .members
        .iter()
        .any(|k| h.state().item(k).unwrap().name == member));
    // Final fix wave A8: no set shares it here, so it is in no set, and nothing is toasted.
    assert!(
        sdc_gui::model::sets::set_views(h.state())
            .iter()
            .all(|v| !v.members.contains(&k)),
        "in no set"
    );
    let texts: Vec<&str> = h
        .state()
        .messages
        .entries()
        .map(|e| e.text.as_str())
        .collect();
    assert!(
        texts.iter().all(|t| !t.contains("went to")),
        "no hand-over message: {texts:?}"
    );
}

/// The Problems row's Remove from set hover for the file `name` (final fix wave A8, Note JN5).
fn remove_hover(name: &str) -> String {
    format!("Take {name} out of this set. It goes to No set, or to the first set that shares it.")
}

/// Final fix wave A8 (Note JN5): the Problems row's Remove from set on a file another set shares hands it to that set
/// and raises the hand-over toast, as the set card's removal does.
#[test]
fn remove_from_set_on_a_shared_file_hands_it_over_with_a_toast() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"))
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let (k, member) = normal_member(&m);
    inject_set(&mut m, &[extra_map(&member)]);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    let remove = format!("Remove {member} from landscape/rock01");
    on_row(&h, &extra_note(&member), &remove).click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    let views = sdc_gui::model::sets::set_views(h.state());
    let heir = views
        .iter()
        .find(|v| v.members.contains(&k))
        .unwrap_or_else(|| panic!("{member} is in a set"));
    assert_eq!(heir.id, "landscape/leaf", "the set that shared it");
    let toast = format!("{member} went to landscape/leaf, which shared it.");
    let texts: Vec<&str> = h
        .state()
        .messages
        .entries()
        .map(|e| e.text.as_str())
        .collect();
    assert!(texts.contains(&toast.as_str()), "{texts:?}");
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
    let opener = h.get_by_label("Change shader for landscape/rock01 (SHADER-MISSING-MAP)");
    let at = opener.rect();
    opener.click();
    h.run();
    nearest(&h, first.label(), at).click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert_eq!(rock01(h.state()).shader_choice, Some(first));
    // Fix round 1: the menu marks the set's shader, as the sets column's combo does. The pick planned again, which
    // replaced the injected warning. Final fix wave B5: no "picking it again is no edit" check, because `edit_docs`
    // records nothing for an unchanged edit, so it could not fail; the menu's guard is `(!on).then_some(s)`.
    inject_set(
        h.state_mut(),
        &[Warning::new(Code::ShaderMissingMap, "no _n")
            .with_data(serde_json::json!({"map": "_n", "shader": "pbr"}))],
    );
    h.run();
    let opener = h.get_by_label("Change shader for landscape/rock01 (SHADER-MISSING-MAP)");
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
    // under the default normal base the row is a note with no button (Note JP6), and its sentence says what the code
    // means.
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

/// Final fix wave A3 (L116): GRASS-DETECT-SOURCE's sentence follows the job. Under the default normal base the
/// written file is renormalised and Community Shaders reads it as grass; with "Keep as authored" it is not, and the row
/// says so. A mark on the kept row's sentence takes the warning out of the tile's count too.
#[test]
fn the_grass_source_row_says_what_the_normal_base_does() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{counts_for, problems, At};
    let w = Warning::new(Code::GrassDetectSource, "g");
    let (_d, mut m) = with_warnings(std::slice::from_ref(&w));
    let k = rock01(&m).members[0].clone();
    let row = |m: &AppModel| {
        problems(m)
            .iter()
            .find(|p| p.code == Some(Code::GrassDetectSource))
            .cloned()
            .expect("the grass source row")
    };
    assert_eq!(
        row(&m).shown(),
        "This grass source's corner normal isn't unit length — the written file is renormalised, so Community \
         Shaders still reads it as grass"
    );
    set_normal_base(&mut m, &k, engine::config::NormalBase::Keep);
    let kept = row(&m);
    assert_eq!(
        kept.shown(),
        "This grass source's corner normal isn't unit length, and its normal base level is kept as authored — \
         Community Shaders won't read the written file as grass"
    );
    assert_eq!((kept.fix.clone(), kept.second_fix.clone()), (None, None));
    let at = At::Texture(k);
    assert!(counts_for(&m, &at, &w), "precondition: it counts");
    m.mark_intended(kept.mark.clone().expect("markable"), true, "row");
    assert!(row(&m).intended, "the row is marked");
    assert!(!counts_for(&m, &at, &w), "the tile reads the same mark");
}

/// Sets the normal base of item `k`'s job and returns the one it had.
fn set_normal_base(
    m: &mut AppModel,
    k: &str,
    nb: engine::config::NormalBase,
) -> engine::config::NormalBase {
    let job = m
        .items
        .iter_mut()
        .find(|i| i.key == k)
        .and_then(|i| i.job.as_mut())
        .expect("a job");
    let old = std::mem::replace(&mut job.profile.mips.normal_base, nb);
    m.invalidate_views();
    old
}

/// B3-18 (Note JP6; F1): with the default normal base the written file is renormalised, so the grass source's row is
/// a note: no button, no Mark, listed under Notes only. It takes nothing from the tile, the set or the tab's count.
#[test]
fn a_grass_source_row_is_a_note_when_the_file_is_renormalised() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{
        cached_tab_counts, counts_for, filtered, item_warned, problems, set_ready, tab_label,
        tile_ready, warning_class, At, Level, ProblemsFilter,
    };
    use sdc_gui::model::code_text::RowClass;
    use sdc_gui::model::items::ItemStatus;
    let (b, ready) = baseline();
    let w = Warning::new(Code::GrassDetectSource, "g");
    let (_d, m) = with_warnings(std::slice::from_ref(&w));
    let k = rock01(&m).members[0].clone();
    let item = m.item(&k).unwrap();
    let job = item.job.as_ref().expect("a job");
    assert_ne!(
        job.profile.mips.normal_base,
        engine::config::NormalBase::Keep,
        "precondition: the default renormalises"
    );
    assert_eq!(item.status, ItemStatus::Ready, "precondition");
    assert_eq!(warning_class(Some(job), &w), RowClass::Note);
    let rows = problems(&m);
    let row = rows
        .iter()
        .find(|p| p.code == Some(Code::GrassDetectSource))
        .expect("the row");
    assert_eq!(row.level, Level::Info);
    assert!(row.is_note() && !row.in_worklist());
    assert_eq!(
        (row.fix.clone(), row.second_fix.clone(), row.mark.clone()),
        (None, None, None)
    );
    let grass = |rows: &[sdc_gui::model::bottom::Problem]| {
        rows.iter().any(|p| p.code == Some(Code::GrassDetectSource))
    };
    assert!(!grass(&filtered(&m, None)), "not in the main list");
    assert!(grass(&filtered(&m, Some(&ProblemsFilter::Notes))));
    assert_eq!(tab_label(cached_tab_counts(&m)), label(b, 0, 0, 1));
    assert!(!counts_for(&m, &At::Texture(k.clone()), &w));
    assert!(!item_warned(&m, item));
    assert!(tile_ready(&m, item));
    assert_eq!(
        set_ready(&m, &rock01(&m)),
        ready,
        "as ready as with no warning"
    );
}

/// B3-18 (Note JP6): under `mips.normal_base = keep` the written file is not read as grass, so the row stays a
/// Problem with Mark, and it counts on the tile, the set and the tab.
#[test]
fn a_grass_source_row_is_a_problem_when_the_normal_base_is_kept() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{
        cached_tab_counts, counts_for, filtered, item_warned, problems, set_ready, tab_label,
        tile_ready, At, Level,
    };
    let (b, _) = baseline();
    let w = Warning::new(Code::GrassDetectSource, "g");
    let (_d, mut m) = with_warnings(std::slice::from_ref(&w));
    let k = rock01(&m).members[0].clone();
    set_normal_base(&mut m, &k, engine::config::NormalBase::Keep);
    let rows = problems(&m);
    let row = rows
        .iter()
        .find(|p| p.code == Some(Code::GrassDetectSource))
        .expect("the row");
    assert_eq!(row.level, Level::Warning);
    assert!(row.mark.is_some() && !row.is_note());
    assert!(filtered(&m, None)
        .iter()
        .any(|p| p.code == Some(Code::GrassDetectSource)));
    assert_eq!(tab_label(cached_tab_counts(&m)), label(b, 1, 0, 0));
    assert!(counts_for(&m, &At::Texture(k.clone()), &w));
    let item = m.item(&k).unwrap();
    assert!(item_warned(&m, item) && !tile_ready(&m, item));
    assert!(!set_ready(&m, &rock01(&m)));
}

/// B3-18 (Note JP6): the set card's "⚠ n" and the tile's ⚠ follow the job: none while renormalised, one when kept.
#[test]
fn the_grass_source_class_follows_the_job_on_the_card() {
    use engine::codes::{Code, Warning};
    let (_d, m) = with_warnings(&[Warning::new(Code::GrassDetectSource, "g")]);
    let k = rock01(&m).members[0].clone();
    let mut h = harness(m);
    h.run();
    assert_eq!(
        h.query_all_by_label_contains("⚠").count(),
        0,
        "renormalised: no amber badge"
    );
    set_normal_base(h.state_mut(), &k, engine::config::NormalBase::Keep);
    h.run();
    h.get_by_label("⚠ 1");
}

/// B3-18 (Note JP6): a last-export File row of the code reads the live item's job, as its sentence does.
#[test]
fn a_grass_source_file_row_follows_the_live_job() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{problems, At, Level};
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.export_clicked();
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    let jk = m.export.rows[0].job_key.clone();
    let live: Vec<String> = m
        .items
        .iter()
        .filter(|i| i.job_key == jk)
        .map(|i| i.key.clone())
        .collect();
    assert_eq!(live.len(), 1, "precondition: one live item");
    let nb = m
        .item(&live[0])
        .and_then(|i| i.job.as_ref())
        .expect("a live job");
    assert_ne!(
        nb.profile.mips.normal_base,
        engine::config::NormalBase::Keep,
        "precondition: the default renormalises"
    );
    m.export.rows[0]
        .warnings
        .push(Warning::new(Code::GrassDetectSource, "g"));
    m.invalidate_views();
    let file_row = |m: &AppModel| {
        problems(m)
            .iter()
            .find(|p| p.code == Some(Code::GrassDetectSource))
            .cloned()
            .expect("a File row")
    };
    let r = file_row(&m);
    assert!(matches!(r.at, At::File(_)), "{:?}", r.at);
    assert_eq!(r.level, Level::Info, "renormalised: a note");
    set_normal_base(&mut m, &live[0], engine::config::NormalBase::Keep);
    let r = file_row(&m);
    assert_eq!(r.level, Level::Warning, "kept: a problem");
    assert_eq!(r.shown(), sdc_gui::model::code_text::GRASS_SOURCE_KEPT);
}

/// B3-25 (Note JP6): a last-export File row whose job key several items share has no job, so its grass row is a Note
/// and counts nowhere, even when every one of those items keeps its normal base (neither the row nor `class_at` takes
/// the first item's job).
#[test]
fn a_grass_source_file_row_with_a_shared_job_key_is_a_note() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{class_at, counts_for, problems, At, Level};
    use sdc_gui::model::code_text::RowClass;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.export_clicked();
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    let jk = m.export.rows[0].job_key.clone();
    let live: Vec<String> = m
        .items
        .iter()
        .filter(|i| i.job_key == jk)
        .map(|i| i.key.clone())
        .collect();
    assert_eq!(live.len(), 1, "precondition: one live item");
    set_normal_base(&mut m, &live[0], engine::config::NormalBase::Keep);
    let w = Warning::new(Code::GrassDetectSource, "g");
    m.export.rows[0].warnings.push(w.clone());
    m.invalidate_views();
    let at = At::File(jk.clone());
    let file_row = |m: &AppModel| {
        problems(m)
            .iter()
            .find(|p| p.code == Some(Code::GrassDetectSource))
            .cloned()
            .expect("a File row")
    };
    assert_eq!(
        file_row(&m).level,
        Level::Warning,
        "precondition: one item that keeps it, a problem"
    );
    // A second item that maps to the same job key and keeps its normal base too.
    let mut twin = m.item(&live[0]).unwrap().clone();
    twin.key = format!("{}-twin", twin.key);
    twin.warnings.clear();
    m.items.push(twin);
    m.invalidate_views();
    let r = file_row(&m);
    assert!(matches!(r.at, At::File(_)), "{:?}", r.at);
    assert_eq!(r.level, Level::Info, "no single job: a note");
    assert_eq!(class_at(&m, &at, &w), RowClass::Note);
    assert!(!counts_for(&m, &at, &w));
}

/// B3-18 (Note JP6): a note reads no mark. A mark stored on the renormalised sentence (Plan 3a let a user store one),
/// and a mark stored while the job kept the normal base, hold nothing once the job renormalises: the row has no Mark,
/// the tile's hover lists no marked line, and both keys stay in sdc-project.toml (ruling 9), so going back to `keep`
/// reads the kept one again.
#[test]
fn a_mark_stored_while_the_grass_row_was_a_problem_holds_nothing_once_it_is_a_note() {
    use engine::codes::{Code, Warning};
    use engine::config::NormalBase;
    use sdc_gui::model::bottom::{intended_key, marked, problems, At, Level, MARKED_LINE};
    let w = Warning::new(Code::GrassDetectSource, "g");
    let (_d, mut m) = with_warnings(std::slice::from_ref(&w));
    let k = rock01(&m).members[0].clone();
    let at = At::Texture(k.clone());
    let grass = |m: &AppModel| {
        problems(m)
            .iter()
            .find(|p| p.code == Some(Code::GrassDetectSource))
            .cloned()
            .expect("the row")
    };
    let renormalised = grass(&m).shown().into_owned();
    m.mark_intended(
        intended_key(&at, Code::GrassDetectSource.as_str(), &renormalised),
        true,
        "row",
    );
    assert!(!marked(&m, &at, &w), "a note reads no mark");
    let before = set_normal_base(&mut m, &k, NormalBase::Keep);
    mark_first(&mut m, Code::GrassDetectSource);
    assert!(
        grass(&m).intended && marked(&m, &at, &w),
        "precondition: marked as a Problem"
    );
    set_normal_base(&mut m, &k, before);
    let row = grass(&m);
    assert_eq!(row.level, Level::Info);
    assert!(!row.intended && row.mark.is_none());
    assert!(!marked(&m, &at, &w));
    let tip = sdc_gui::ui::columns::tile_tooltip(&m, m.item(&k).unwrap(), false);
    assert!(!tip.contains(MARKED_LINE), "{tip}");
    let stored = m.sets_doc.as_ref().unwrap().intended_keys();
    assert_eq!(
        stored
            .iter()
            .filter(|s| s.code == "GRASS-DETECT-SOURCE")
            .count(),
        2,
        "both marks stay (ruling 9)"
    );
    set_normal_base(&mut m, &k, NormalBase::Keep);
    assert!(grass(&m).intended, "the kept mark holds again");
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
        Some(Some(Fix::RebuildZ(nk.clone()))),
        "precondition: the check ran"
    );
    m.open_problems(None);
    let name = rebuild_z_for(&m.item(&nk).unwrap().name);
    let mut h = harness(m);
    h.run();
    // The sentence names "Rebuild Z" too, so the button is found by its role on the row.
    let y = h
        .get_by_label(sdc_gui::model::code_text::code_sentence(Code::ChkZSign))
        .rect()
        .center()
        .y;
    h.query_all_by_role_and_label(egui::accesskit::Role::Button, &name)
        .find(|n| (n.rect().center().y - y).abs() <= 4.0)
        .expect("Rebuild Z on the row")
        .click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert_eq!(has(h.state()), None, "Rebuild Z is on: the row is gone");
}

/// The accessible name of the Rebuild Z button of the file `name` (final review Minor 4, rule 8.4).
fn rebuild_z_for(name: &str) -> String {
    format!("{} for {name}", sdc_gui::ui::problems::REBUILD_Z)
}

/// `rebuild_z_for` the member `inject` (and so `with_warnings`) puts its warnings on.
fn injected_z(m: &AppModel) -> String {
    rebuild_z_for(&m.item(&rock01(m).members[0]).unwrap().name)
}

/// Final review Minor 4: each Rebuild Z button is named for its file, so two on screen are told apart; the visible
/// text stays "Rebuild Z".
#[test]
fn each_rebuild_z_button_is_named_for_its_file() {
    use engine::codes::{Code, Warning};
    let w = Warning::new(Code::ChanNormalLength, "short");
    let (_d, mut m) = with_warnings(std::slice::from_ref(&w));
    let v = rock01(&m);
    let (a, b) = (v.members[0].clone(), v.members[1].clone());
    m.items.iter_mut().find(|i| i.key == b).unwrap().warnings = vec![w];
    m.invalidate_views();
    m.open_problems(None);
    let names: Vec<String> = [&a, &b]
        .iter()
        .map(|k| m.item(k).unwrap().name.clone())
        .collect();
    assert_ne!(names[0], names[1], "precondition");
    let mut h = harness(m);
    h.run();
    for n in &names {
        assert_eq!(
            h.query_all_by_role_and_label(egui::accesskit::Role::Button, &rebuild_z_for(n))
                .count(),
            1,
            "{n}'s button"
        );
    }
    assert_eq!(
        h.query_all_by_role_and_label(
            egui::accesskit::Role::Button,
            sdc_gui::ui::problems::REBUILD_Z
        )
        .count(),
        0,
        "no bare name left"
    );
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

/// Final wave Group 3 (review Important 4, ledger line 124): a SHADER-EXTRA-MAP row that names a packed output other
/// than the RMAOS slot's (here vanilla's complex `_m`) keeps that member as its Go to set target.
#[test]
fn an_extra_packed_output_row_keeps_its_member_target() {
    use sdc_gui::model::items::ItemKind;
    use sdc_gui::model::nav::SlotTarget;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_target(engine::config::Target::Vanilla);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    support::complex_material(&mut m, "landscape/rock01");
    let (key, file) = rock01(&m)
        .members
        .iter()
        .find_map(|k| {
            let i = m.item(k).filter(|i| i.kind == ItemKind::Packed)?;
            let f = i.job.as_ref()?.output.file_name()?.to_string_lossy();
            f.contains("_m.").then(|| (k.clone(), f.into_owned()))
        })
        .expect("premise: rock01 packs a complex _m");
    inject_set(&mut m, &[extra_map(&file)]);
    let rows = sdc_gui::model::bottom::problems(&m);
    let row = rows
        .iter()
        .find(|p| p.code == Some(engine::codes::Code::ShaderExtraMap))
        .expect("the SHADER-EXTRA-MAP row");
    assert_eq!(row.target, Some(SlotTarget::Item(key)));
}

/// Final wave Group 3 (ledger line 255): a Problems set filter belongs to the project it was set in, so opening
/// another folder drops it.
#[test]
fn opening_another_folder_drops_the_set_filter() {
    use sdc_gui::model::bottom::ProblemsFilter;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let key = rock01(&m).key;
    let root = m.root.clone().expect("premise: a folder is open");
    for f in [
        ProblemsFilter::Set(key.clone()),
        ProblemsFilter::SetNotes(key.clone()),
        ProblemsFilter::SetChecks(key.clone()),
    ] {
        m.bottom.filter = Some(f.clone());
        let e = tempfile::tempdir().unwrap();
        let other = e.path().join("textures");
        std::fs::create_dir_all(&other).unwrap();
        m.open_folder(other);
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        assert_eq!(m.bottom.filter, None, "{f:?} is the old project's");
        m.open_folder(root.clone());
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        // Reopening the same folder keeps it: its sets are the same sets.
        m.bottom.filter = Some(f.clone());
        m.open_folder(root.clone());
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        assert_eq!(m.bottom.filter, Some(f.clone()), "{f:?}: same folder");
    }
    // A filter that names no set is not the folder's.
    m.bottom.filter = Some(ProblemsFilter::Notes);
    let e = tempfile::tempdir().unwrap();
    m.open_folder(e.path().to_path_buf());
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert_eq!(m.bottom.filter, Some(ProblemsFilter::Notes));
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

/// Final fix wave A2: Go to set closes an expanded texture preview first, so the set editor draws at once and takes
/// the reveal.
#[test]
fn go_to_set_closes_an_expanded_preview_first() {
    use sdc_gui::model::nav::SlotTarget;
    use sdc_gui::model::sets::SlotKind;
    let (_d, m) = with_set_warnings(&[]);
    let set = rock01(&m).key;
    let (item, _) = normal_member(&m);
    let mut h = harness(m);
    h.state_mut().expand(&item);
    h.step();
    h.step();
    assert_eq!(
        h.state().expanded.as_deref(),
        Some(item.as_str()),
        "precondition"
    );
    h.state_mut()
        .go_to_set(&set, Some(SlotTarget::Slot(SlotKind::Normal)));
    h.step();
    h.step();
    assert!(h.state().expanded.is_none(), "the preview closed");
    assert_eq!(
        sdc_gui::ui::row_reveal::revealed(&h.ctx).map(|r| r.0),
        Some(SlotTarget::Slot(SlotKind::Normal)),
        "the set editor took the reveal"
    );
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
    let z = injected_z(&m);
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
        .query_all_by_role_and_label(Role::Button, &z)
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

/// Final fix wave A11: a slot whose label has its own parentheses is named without nesting them, in the words, the
/// name and the hover.
#[test]
fn slot_3s_reason_has_no_nested_parentheses() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::set_reasons;
    use sdc_gui::model::sets::{editor_cells, SlotKind};
    const SLOT3: &str = "no Emissive / Skin, slot 3 (Assign…)";
    // A PBR set outside landscape/ draws a slot 3 card (rock01, PBR Landscape, draws none).
    let d = tempfile::tempdir().unwrap();
    let mut m = flat_parallax_wall(d.path());
    let key = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.ends_with("wall"))
        .unwrap()
        .key
        .clone();
    m.sets.iter_mut().find(|s| s.key == key).unwrap().warnings =
        vec![Warning::new(Code::ShaderMissingMap, "no _g")
            .with_data(serde_json::json!({"map": "_g", "shader": "pbr"}))];
    m.invalidate_views();
    let v = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key == key)
        .cloned()
        .unwrap();
    assert!(
        editor_cells(&v)
            .iter()
            .any(|c| c.kind == SlotKind::Emissive),
        "precondition: the editor draws slot 3"
    );
    let texts: Vec<String> = set_reasons(&m, &key).into_iter().map(|r| r.text).collect();
    assert_eq!(texts, [SLOT3]);
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(
        egui::accesskit::Role::Button,
        &format!("{SLOT3} — go to the fix"),
    );
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
/// reason's button name. Final fix wave B10: no set in this fixture shares rock01's normal, so removing the member
/// goes to no set and no hand-over (Note JN5) takes part.
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
    thumbs: sdc_gui::ui::thumbs::ThumbCache,
    cmds: sdc_gui::ui::slot_commands::SlotCmdState,
    drops: sdc_gui::ui::sets_column::DropState,
    column: egui::Rect,
}

fn editor_harness(mut m: AppModel, key: String, width: f32) -> Harness<'static, Editor> {
    if !matches!(&m.selection, sdc_gui::model::Selection::Set(k) if k == &key) {
        m.select_set(&key);
    }
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
                    let active = e.m.resolved_selection();
                    sdc_gui::ui::set_editor::draw(
                        ui,
                        &mut e.m,
                        &active,
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
    use sdc_gui::model::sets::{editor_cells, SlotKind};
    assert!(
        baseline().1,
        "precondition: rock01 is ready with no injected warning"
    );
    let (_d, mut m) = with_set_warnings(&[Warning::new(Code::ShaderMissingMap, "no _g")
        .with_data(serde_json::json!({"map": "_g", "shader": "pbr"}))]);
    let v = rock01(&m);
    // Final fix wave B6: the rows the editor draws, the list `set_reasons` decides by.
    assert!(
        !editor_cells(&v)
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
    let z = injected_z(&m);
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
        .query_all_by_label(&z)
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

/// Fix round 2 item 1: "Turn parallax off" takes focus from its reason but never runs on Enter or Space; a click
/// still runs it.
#[test]
fn turn_parallax_off_focused_by_a_reason_ignores_enter_and_space() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::AlphaParallaxNoAlpha, "p")]);
    let v = rock01(&m);
    let k = v.members[0].clone();
    let file = m.item(&k).unwrap().name.clone();
    let parallax = |m: &AppModel| m.project.as_ref().unwrap().file_value(&k, "alpha.parallax");
    let before = parallax(&m);
    assert_ne!(before, Some(serde_json::json!(false)), "precondition");
    let r = sdc_gui::model::bottom::set_reasons(&m, &v.key);
    let name = format!("{} — go to the fix", r[0].text);
    m.select_set(&v.key);
    let mut h = harness(m);
    h.run();
    click_and_settle(&mut h, &name);
    let button = format!("Turn parallax off for {file}");
    assert!(
        h.get_by_label(&button).accesskit_node().is_focused(),
        "focus shows where the fix is"
    );
    for key in [egui::Key::Enter, egui::Key::Space] {
        h.key_press(key);
        h.run();
        assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
        h.run();
        assert_eq!(parallax(h.state()), before, "{key:?} wrote nothing");
    }
    click_and_settle(&mut h, &button);
    assert_eq!(parallax(h.state()), Some(serde_json::json!(false)));
}

/// Fix round 2 item 2: "+k more in Problems" shows a hidden kind behind a slot reason past the third too.
#[test]
fn more_in_problems_shows_a_hidden_slot_reasons_row() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::{problems, set_reasons, ReasonGo};
    let (_d, mut m) = with_warnings(&[
        Warning::new(Code::ValMipError, "mip 3"),
        Warning::new(Code::ValCoverage, "coverage"),
        Warning::new(Code::LossReencode, "again"),
    ]);
    let key = rock01(&m).key;
    // Set rows come after the item rows, so the missing `_p` (Height, an empty slot row) is the fourth reason.
    m.sets.iter_mut().find(|s| s.key == key).unwrap().warnings =
        vec![Warning::new(Code::ShaderMissingMap, "no _p")
            .with_data(serde_json::json!({"map": "_p", "shader": "pbr"}))];
    m.invalidate_views();
    let r = set_reasons(&m, &key);
    let slot = r
        .iter()
        .position(|r| matches!(r.go, ReasonGo::Slot(_)))
        .unwrap_or_else(|| panic!("a slot reason: {r:?}"));
    assert!(slot >= 3, "past the third: {r:?}");
    let shown = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::ShaderMissingMap))
        .unwrap()
        .shown()
        .to_string();
    m.set_kind_hidden(Code::ShaderMissingMap, true);
    assert_eq!(
        m.hidden_kinds(),
        vec![Code::ShaderMissingMap],
        "precondition"
    );
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    click_and_settle(&mut h, &format!("+{} more in Problems", r.len() - 3));
    assert!(h.state().hidden_kinds().is_empty());
    assert!(
        h.query_all_by_label_contains(&shown).count() >= 1,
        "the row is listed"
    );
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
    use sdc_gui::model::bottom::flagged;
    use sdc_gui::ui::sets_column::{pill_hover, NOT_READY_MEANS, READY_MEANS};
    use sdc_gui::ui::status::Status;
    let (_d, m) = with_warnings(&[Warning::new(Code::ValMipError, "mip 3")]);
    let v = rock01(&m);
    assert!(pill_hover(&m, &flagged(&m), &v, Status::Warning).contains(NOT_READY_MEANS));
    assert!(pill_hover(&m, &flagged(&m), &v, Status::Ready).contains(READY_MEANS));
    assert!(!pill_hover(&m, &flagged(&m), &v, Status::Blocks).contains(READY_MEANS));
    assert!(!pill_hover(&m, &flagged(&m), &v, Status::Blocks).contains(NOT_READY_MEANS));
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

/// `bottom_panel.rs::short_normal_model`, copied: a mod folder with one short normal (decodes to length 0.3, R != G)
/// and a model opened on it.
fn short_normal_model(dir: &std::path::Path, toml: Option<&str>) -> AppModel {
    let root = mod_folder(dir);
    image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb([153u8, 140, 153]))
        .save(root.join("landscape/x_n.png"))
        .unwrap();
    if let Some(t) = toml {
        std::fs::write(root.join("sdc.toml"), t).unwrap();
    }
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(dir.join("export")),
        ..Default::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    choose_export(&mut m, dir.join("export"));
    m.open_problems(None);
    m
}

/// `short_normal_model` with every file decided, x_n.png a DirectX normal without Rebuild Z, and exported: its
/// exported file is short, so the export's row carries CHAN-NORMAL-LENGTH.
fn exported_short_normal(dir: &std::path::Path) -> AppModel {
    let mut m = short_normal_model(dir, None);
    decide_all(&mut m);
    m.decide(
        "landscape/x_n.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Normal),
    );
    m.decide(
        "landscape/x_n.png",
        sdc_gui::model::Answer::Green(engine::config::GreenConvention::Directx),
    );
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.export_clicked();
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    m
}

const QUEUED_Z: &str = "Rebuild Z is on for this file: export again to rewrite it";
/// x_n.png's Rebuild Z button (`rebuild_z_for`).
const X_N_Z: &str = "Rebuild Z for x_n.png";

/// The last export's CHAN-NORMAL-LENGTH row of x_n.png (an `At::File` row).
fn exported_z_row(m: &AppModel) -> Option<sdc_gui::model::bottom::Problem> {
    use sdc_gui::model::bottom::{problems, At};
    problems(m)
        .iter()
        .find(|p| {
            matches!(p.at, At::File(_)) && p.code == Some(engine::codes::Code::ChanNormalLength)
        })
        .cloned()
}

/// Review E25 acceptance: the row text changes after the press.
#[test]
fn after_rebuild_z_the_exported_row_is_a_queued_note() {
    use sdc_gui::model::bottom::{set_reasons, Level, ProblemsFilter};
    let d = tempfile::tempdir().unwrap();
    let mut m = exported_short_normal(d.path());
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    // The click lands on the next frame: run, wait for the plan it starts, then draw its result.
    click_and_settle(&mut h, X_N_Z);
    let r = exported_z_row(h.state()).expect("the last export's row");
    assert_eq!(r.level, Level::Queued);
    assert_eq!(r.text, QUEUED_Z);
    assert!(r.is_note() && r.fix.is_none() && r.second_fix.is_none() && r.mark.is_none());
    // Final fix wave B7: the note takes no "ready" away, and the Export dot's count is the count without the row.
    assert!(
        !sdc_gui::model::bottom::counts_as_ready_problem(&r),
        "{r:?}"
    );
    let ready = sdc_gui::model::bottom::flagged(h.state()).ready_rows;
    // The queued note is no reason on the set editor's status line (a File row never is one): x_n.png's set reads
    // "Not ready: no Diffuse (Assign…)", from its missing diffuse alone.
    let v = sdc_gui::model::sets::set_views(h.state())
        .into_iter()
        .find(|v| v.members.iter().any(|k| k == "landscape/x_n.png"))
        .expect("x_n.png is in a set");
    let reasons = set_reasons(h.state(), &v.key);
    let texts: Vec<&str> = reasons.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(texts, ["no Diffuse (Assign…)"]);
    h.state_mut().open_problems(Some(ProblemsFilter::Notes));
    h.run();
    h.get_by_label("queued");
    // Exact: the row's own name ("<file>: <sentence>", Note FB9) also contains the sentence.
    h.get_by_label(QUEUED_Z);
    assert!(h.query_by_label(X_N_Z).is_none());
    // B7: the same state with the row taken out has the same count.
    let m = h.state_mut();
    for row in &mut m.export.rows {
        row.warnings
            .retain(|w| w.code != engine::codes::Code::ChanNormalLength);
    }
    m.invalidate_views();
    assert!(exported_z_row(m).is_none(), "precondition: no row");
    assert_eq!(
        sdc_gui::model::bottom::flagged(m).ready_rows,
        ready,
        "the queued note changes no ready count"
    );
}

/// Review E25: without Rebuild Z on, the last export's short normal stays a warning with the Rebuild Z fix (the live
/// item's warning is cleared so the File row is not dropped as its copy, Note DP4).
#[test]
fn an_exported_short_normal_without_rebuild_z_stays_a_warning() {
    use sdc_gui::model::bottom::{Fix, Level};
    let d = tempfile::tempdir().unwrap();
    let mut m = exported_short_normal(d.path());
    m.items
        .iter_mut()
        .find(|i| i.key == "landscape/x_n.png")
        .unwrap()
        .warnings
        .clear();
    m.invalidate_views();
    let r = exported_z_row(&m).expect("the last export's row");
    assert_eq!(r.level, Level::Warning);
    assert_eq!(r.fix, Some(Fix::RebuildZ("landscape/x_n.png".into())));
}

/// Controller ruling: a queued note is a reminder that goes with the next export, so a hidden kind never hides it
/// (pins `can_hide`, which lets only Warning and Info rows hide).
#[test]
fn a_hidden_kind_never_hides_the_queued_z_note() {
    use engine::codes::Code;
    use sdc_gui::model::bottom::{is_hidden, ProblemsFilter};
    let d = tempfile::tempdir().unwrap();
    let mut m = exported_short_normal(d.path());
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    click_and_settle(&mut h, X_N_Z);
    // Kinds lists no CHAN-NORMAL-LENGTH once the only row is queued, so the kind is hidden as an earlier choice would.
    // The order does not matter: `is_hidden` reads the row and the hidden kinds as they are now, so hiding before
    // the press gives the same row.
    h.state_mut().set_kind_hidden(Code::ChanNormalLength, true);
    let r = exported_z_row(h.state()).expect("the last export's row");
    assert!(!is_hidden(&r, &h.state().hidden_kinds()));
    h.state_mut().open_problems(Some(ProblemsFilter::Notes));
    h.run();
    h.get_by_label(QUEUED_Z);
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

/// Review E29: on a wide window a row's button follows its sentence, not the panel's right edge.
#[test]
fn at_1600_a_short_row_keeps_its_button_within_40_px_of_the_sentence() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    m.open_problems(None);
    let mut h = harness_at(m, egui::vec2(1600.0, 900.0));
    h.run();
    h.run();
    let sentence = h
        .get_by_label(sdc_gui::model::code_text::code_sentence(Code::ChanNotGray))
        .rect();
    let button = h.get_by_label_contains("Mark as intended: ").rect();
    assert!(
        button.left() - sentence.right() <= 40.0,
        "gap {}",
        button.left() - sentence.right()
    );
}

/// Review E29: a row with a fix and Mark as intended keeps them in that order, the second one just after the first.
#[test]
fn at_1600_a_row_with_two_controls_keeps_both_after_the_sentence() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ChanNormalLength, "short")]);
    m.open_problems(None);
    let z = injected_z(&m);
    let mut h = harness_at(m, egui::vec2(1600.0, 900.0));
    h.run();
    let text = sdc_gui::model::code_text::code_sentence(Code::ChanNormalLength);
    let sentence = h.get_by_label(text).rect();
    // By role: the row's own name contains the sentence, which names Rebuild Z.
    let fix = h
        .query_all_by_role_and_label(egui::accesskit::Role::Button, &z)
        .find(|n| (n.rect().center().y - sentence.center().y).abs() <= 4.0)
        .expect("Rebuild Z on the row")
        .rect();
    let mark = on_row(&h, text, "Mark as intended: ").rect();
    assert!(
        sentence.right() < fix.left() && fix.right() < mark.left(),
        "sentence {sentence:?}, fix {fix:?}, mark {mark:?}"
    );
    assert!(
        mark.left() - sentence.right() <= 40.0 + fix.width(),
        "gap {}",
        mark.left() - sentence.right()
    );
}

/// Review E29: a row with no button (a note) still shows its whole sentence on a wide window.
#[test]
fn at_1600_a_row_with_no_button_keeps_its_whole_sentence() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::ProblemsFilter;
    let (_d, mut m) = only(&[Warning::new(Code::DimNpot, "npot")]);
    m.open_problems(Some(ProblemsFilter::Notes));
    let mut h = harness_at(m, egui::vec2(1600.0, 900.0));
    h.run();
    let text = "Keeps a size that isn't a power of two. That's fine: it has no mips.";
    let sentence = h.get_by_label(text).rect();
    let full = whole_width(&h, text);
    assert!(
        (sentence.width() - full).abs() < 1.0,
        "the sentence is {} px of {full}",
        sentence.width()
    );
}

/// The width of `text` drawn whole in the body font, to tell a whole sentence from a cut one.
fn whole_width<S>(h: &Harness<'_, S>, text: &str) -> f32 {
    let font = egui::TextStyle::Body.resolve(&h.ctx.global_style());
    h.ctx.fonts_mut(|f| {
        f.layout_no_wrap(text.to_string(), font, egui::Color32::WHITE)
            .size()
            .x
    })
}

/// Review E29: a row's buttons change under the same row (Mark as intended becomes the narrower Undo mark, and back):
/// the width stored for it follows, so the button stays close and the sentence is never cut.
#[test]
fn a_row_whose_button_changes_follows_the_new_width() {
    use engine::codes::{Code, Warning};
    use sdc_gui::model::bottom::ProblemsFilter;
    let (_d, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    m.open_problems(None);
    let mut h = harness_at(m, egui::vec2(1600.0, 900.0));
    h.run();
    let text = sdc_gui::model::code_text::code_sentence(Code::ChanNotGray);
    let whole = whole_width(&h, text);
    for (click, list, now) in [
        (
            "Mark as intended: ",
            Some(ProblemsFilter::Notes),
            "Undo mark: ",
        ),
        ("Undo mark: ", None, "Mark as intended: "),
    ] {
        h.get_by_label_contains(click).click();
        h.run();
        h.state_mut().open_problems(list);
        h.run();
        let sentence = h.get_by_label(text).rect();
        let button = h.get_by_label_contains(now).rect();
        assert!(
            button.left() - sentence.right() <= 40.0,
            "{now} gap {}",
            button.left() - sentence.right()
        );
        assert!(
            (sentence.width() - whole).abs() < 1.0,
            "with {now} the sentence is {} px of {whole}",
            sentence.width()
        );
    }
}

/// Review E29: the row's width comes from the last frame, so the list must settle (`run` panics if it never goes idle),
/// wide and at 1280×800 with a row longer than the panel, whose button stays inside it. Final fix wave B8: both
/// halves assert, and once settled a frame is exactly one step of one pass: the row measures itself once, with no
/// repaint and no discarded pass after that.
#[test]
fn a_row_sized_from_the_last_frame_settles_wide_and_narrow() {
    use engine::codes::{Code, Warning};
    let settles = |m: AppModel, size: egui::Vec2| {
        let mut h = harness_at(m, size);
        h.run();
        let passes = h.ctx.cumulative_pass_nr();
        assert_eq!(h.run(), 1, "{size:?}: a settled frame asks for no repaint");
        assert_eq!(
            h.ctx.cumulative_pass_nr() - passes,
            1,
            "{size:?}: a settled frame discards no pass"
        );
        // Against the window, not the panel: a row wider than the window would widen the panel's rect with it.
        let panel = sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "bottom").expect("the bottom panel");
        let mark = h.get_by_label_contains("Mark as intended: ").rect();
        assert!(
            mark.right() <= panel.right().min(size.x) && mark.left() >= panel.left(),
            "{size:?}: Mark at {mark:?} is outside the panel {panel:?} or the window"
        );
    };
    let (_d, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    m.open_problems(None);
    settles(m, egui::vec2(1600.0, 900.0));
    let (_d2, mut m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    m.items[0].key = format!("t/{}", "a_very_long_folder_name/".repeat(40));
    m.invalidate_views();
    m.open_problems(None);
    settles(m, egui::vec2(1280.0, 800.0));
}

/// Final fix wave A1: a row listed for the first time has its buttons at their final x on its first painted frame:
/// the pass that measures them is discarded, as egui's `Grid` does on its first measure.
#[test]
fn a_new_rows_buttons_are_in_place_on_its_first_frame() {
    use engine::codes::{Code, Warning};
    let (_d, mut m) = only(&[]);
    m.open_problems(None);
    let mut h = harness_at(m, egui::vec2(1600.0, 900.0));
    h.run();
    h.state_mut().items[0].warnings = vec![Warning::new(Code::ChanNotGray, "not grey")];
    h.state_mut().invalidate_views();
    h.step();
    let first = h.get_by_label_contains("Mark as intended: ").rect();
    h.run();
    let settled = h.get_by_label_contains("Mark as intended: ").rect();
    assert!(
        (first.left() - settled.left()).abs() < 0.5,
        "first frame {first:?}, settled {settled:?}"
    );
}

/// Final fix wave A1 (pin): a reason's `focus_row` hand-off survives the discarded pass that first measures its row.
/// At 1600 px the row is narrower than the list, so the Problems tab's first draw is discarded.
#[test]
fn a_reason_focuses_its_fix_through_a_discarded_first_pass() {
    use egui::accesskit::Role;
    use engine::codes::{Code, Warning};
    let (_d, mut m) = with_warnings(&[Warning::new(Code::ChanNormalLength, "short")]);
    let key = rock01(&m).key;
    let z = injected_z(&m);
    m.select_set(&key);
    let mut h = harness_at(m, egui::vec2(1600.0, 900.0));
    h.run();
    let what = sdc_gui::model::code_text::code_line(Code::ChanNormalLength)
        .split(" — ")
        .next()
        .unwrap()
        .to_string();
    h.get_by_label(&format!("{what} — go to the fix")).click();
    h.step();
    h.step();
    let sentence = sdc_gui::model::code_text::code_sentence(Code::ChanNormalLength);
    let row = h.get_by_label(sentence).rect();
    let fix = h
        .query_all_by_role_and_label(Role::Button, &z)
        .find(|n| (n.rect().center().y - row.center().y).abs() <= 4.0)
        .expect("Rebuild Z on the row");
    assert!(fix.accesskit_node().is_focused(), "Rebuild Z has focus");
    assert!(
        fix.rect().left() - row.right() < 100.0,
        "the row is narrow: its fix follows the sentence ({row:?}, {:?})",
        fix.rect()
    );
}

/// Final review Important 1 (rule 3.4): a code-less reason longer than 60 characters is cut on screen, but its
/// accessible name and the first line of its hover hold the whole sentence.
#[test]
fn a_long_code_less_reason_keeps_its_whole_sentence_in_its_name_and_hover() {
    use sdc_gui::model::bottom::{set_reasons, ReasonGo};
    const GONE: &str = "landscape/a_notes_file_whose_name_is_long_enough_to_be_cut.txt";
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    // As `a_removal_focused_by_a_reason_ignores_enter_and_space`: a file that is no texture matches no entry.
    std::fs::write(m.root.clone().unwrap().join(GONE), "notes").unwrap();
    m.edit_docs("t", None, true, |_, s| {
        s.add_member("landscape/rock01", GONE)
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let key = rock01(&m).key;
    let r = set_reasons(&m, &key);
    let full = r
        .iter()
        .find_map(|r| match &r.go {
            ReasonGo::Row(p) if p.text.contains("a_notes_file") => Some(p.text.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a missing-entry reason: {r:?}"));
    assert!(full.chars().count() > 60, "precondition: {full}");
    let cut: String = full.chars().take(60).collect();
    m.select_set(&key);
    let mut h = harness(m);
    tooltips_now(&h);
    h.run();
    assert!(
        h.query_by_label(&format!("{cut}… — go to the fix"))
            .is_none(),
        "the cut text is not the button's name"
    );
    assert!(
        h.query_by_label(&full).is_none(),
        "precondition: the whole sentence is not on screen"
    );
    h.get_by_label(&format!("{full} — go to the fix")).hover();
    h.run();
    h.get_by_label(&full);
}

/// Final review Minor 2: a created set with no files yet has no engine key, so its card is amber with no problem row;
/// its hover says why instead of claiming a row.
#[test]
fn a_set_with_no_files_yet_says_so_on_its_card() {
    use sdc_gui::model::bottom::flagged;
    use sdc_gui::ui::sets_column::{pill_hover, NOT_READY_MEANS, NO_FILES_YET};
    use sdc_gui::ui::status::Status;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.edit_docs("t", None, true, |_, s| s.create_set("landscape/empty"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let v = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(|v| v.id == "landscape/empty")
        .expect("the created set has a card");
    assert!(
        v.key.is_empty() && v.members.is_empty(),
        "precondition: {v:?}"
    );
    assert!(!sdc_gui::model::bottom::set_ready(&m, &v), "amber");
    let hover = pill_hover(&m, &flagged(&m), &v, Status::Warning);
    assert!(hover.contains(NO_FILES_YET), "{hover}");
    assert!(!hover.contains(NOT_READY_MEANS), "{hover}");
    assert_eq!(NO_FILES_YET, "Not ready: this set has no files yet.");
    let ready = rock01(&m);
    assert!(pill_hover(&m, &flagged(&m), &ready, Status::Warning).contains(NOT_READY_MEANS));
}

/// Two sets with no engine key, each with one entry that matches no file (`.txt` is no texture).
fn key_less_sets(dir: &std::path::Path) -> AppModel {
    let mut m = pbr_opened(dir);
    let root = m.root.clone().unwrap();
    for (id, file) in [
        ("landscape/empty", "landscape/one.txt"),
        ("landscape/other", "landscape/two.txt"),
    ] {
        std::fs::write(root.join(file), "notes").unwrap();
        m.edit_docs("t", None, true, |_, s| {
            s.create_set(id);
            s.add_member(id, file)
        });
    }
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    for id in ["landscape/empty", "landscape/other"] {
        let v = sdc_gui::model::sets::set_views(&m)
            .into_iter()
            .find(|v| v.id == id)
            .expect("the created set has a card");
        assert!(
            v.key.is_empty() && !v.missing.is_empty(),
            "precondition: {v:?}"
        );
    }
    m
}

fn wall_cards(dir: &std::path::Path) -> (AppModel, String) {
    let root = dir.join("textures");
    std::fs::create_dir(&root).unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
    })
    .save(root.join("wall.png"))
    .unwrap();
    std::fs::write(root.join("wall-note.txt"), "notes").unwrap();
    let mut m = AppModel::new(Default::default(), None, session());
    m.open_folder(root);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    choose_export(&mut m, dir.join("export"));
    let id = engine::sets::set_key(&m.sets_root(), "wall");
    m.edit_docs("t", None, true, |_, s| {
        s.create_set(&id);
        s.add_member(&id, "wall-note.txt");
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert_eq!(sdc_gui::model::sets::views(&m).len(), 2);
    assert_eq!(view_by_id(&m, "wall").key, id);
    assert!(view_by_id(&m, &id).key.is_empty());
    (m, id)
}

fn card<'a>(h: &'a Harness<'static, AppModel>, id: &'a str) -> egui_kittest::Node<'a> {
    let panel = support::panel(&h.ctx, "sets");
    h.query_all_by_label(id)
        .find(|n| panel.contains(n.rect().center()))
        .expect("the card title in Sets")
}

#[test]
fn a_deferred_card_click_uses_the_card_that_survives_the_landing() {
    use sdc_gui::model::Selection;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(key_less_sets(d.path()));
    h.run();
    let id = "landscape/other";
    let file = "landscape/two.png";
    let rock = "landscape/rock01";
    let defer = |h: &mut Harness<'static, AppModel>| {
        card(h, rock).click();
        h.run();
        h.get_by_role_and_label(
            egui::accesskit::Role::TextInput,
            sdc_gui::ui::input_name::HINT,
        )
        .click();
        h.run();
        let pos = card(h, id).rect().center();
        h.input_mut().events.push(egui::Event::PointerMoved(pos));
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos,
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.step();
        assert_eq!(
            h.state().after_commit.len(),
            1,
            "the card click awaits the field"
        );
    };
    defer(&mut h);
    give_a_file(h.state_mut(), id, file);
    let gained = view_by_id(h.state(), id).key;
    assert!(!gained.is_empty());
    h.run();
    assert_eq!(h.state().selection, Selection::Set(gained.clone()));
    card(&h, rock).click_modifiers(egui::Modifiers::COMMAND);
    h.run();
    assert_eq!(
        h.state().selected_sets(),
        [gained, view_by_id(h.state(), rock).key]
    );

    defer(&mut h);
    std::fs::remove_file(h.state().root.as_ref().unwrap().join(file)).unwrap();
    h.state_mut().replan();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    assert!(view_by_id(h.state(), id).key.is_empty());
    h.run();
    assert_eq!(h.state().selection, id_selection(id));

    defer(&mut h);
    assert!(h.state_mut().delete_set(id));
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert_eq!(h.state().selection, Selection::None);
}

#[test]
fn a_click_on_a_key_less_card_selects_that_set_alone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    let texture = "landscape/rock01.png";
    m.expand(texture);
    let mut h = harness(m);
    h.run();
    assert_eq!(h.state().expanded.as_deref(), Some(texture));
    assert!(!sdc_gui::ui::set_editor_shown(h.state(), false));
    card(&h, "landscape/other").click();
    h.run();
    assert_eq!(h.state().selection, id_selection("landscape/other"));
    assert!(card_selected(&h, "landscape/other"));
    assert!(!card_selected(&h, "landscape/empty"));
    assert!(h.state().expanded.is_none());
    assert_eq!(
        h.state().centre_front(),
        sdc_gui::model::CentreTab::Selection
    );
    assert_editor(&h, "landscape/other");
}

#[test]
fn modifier_clicks_never_mix_a_key_less_set_into_a_multi_selection() {
    use sdc_gui::model::Selection;
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    let rock = view_by_id(&m, "landscape/rock01").key;
    m.select_set(&rock);
    let mut h = harness(m);
    h.run();
    for mods in [egui::Modifiers::COMMAND, egui::Modifiers::SHIFT] {
        card(&h, "landscape/other").click_modifiers(mods);
        h.run();
        assert_eq!(h.state().selection, id_selection("landscape/other"));
        assert!(!h.state().in_bulk());
        card(&h, "landscape/rock01").click_modifiers(mods);
        h.run();
        assert_eq!(h.state().selection, Selection::Set(rock.clone()));
        assert!(!h.state().in_bulk());
    }
}

#[test]
fn the_model_keeps_ids_out_of_modified_clicks_and_select_all() {
    use sdc_gui::model::{Selection, SetTarget};
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    let rock = view_by_id(&m, "landscape/rock01").key;
    let id = "landscape/other";
    for (ctrl, shift) in [(false, false), (true, false), (false, true), (true, true)] {
        m.select_set(&rock);
        m.click_set(SetTarget::Id(id.into()), ctrl, shift);
        assert_eq!(m.selection, id_selection(id));
        assert!(!m.in_bulk());
        m.click_set(&rock, ctrl, shift);
        assert_eq!(m.selection, Selection::Set(rock.clone()));
        assert!(!m.in_bulk());
    }
    pick_id(&mut m, id);
    let views = sdc_gui::model::sets::views(&m);
    let expected: Vec<_> = m
        .visible_sets()
        .iter()
        .map(|&i| &views[i])
        .filter(|v| !v.key.is_empty())
        .map(|v| v.key.clone())
        .collect();
    assert!(expected.contains(&rock));
    m.select_all_visible();
    assert_eq!(m.selected_sets(), expected);
    assert!(matches!(m.selection, Selection::Set(_)));
}

#[test]
fn a_right_click_on_a_key_less_card_selects_that_set_and_edit_match_closes_the_menu() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    let texture = "landscape/rock01.png";
    m.expand(texture);
    let mut h = harness(m);
    h.run();
    assert_eq!(h.state().expanded.as_deref(), Some(texture));
    card(&h, "landscape/empty").click_secondary();
    h.run();
    assert_eq!(h.state().selection, id_selection("landscape/empty"));
    assert!(h.state().expanded.is_none());
    assert_editor(&h, "landscape/empty");
    h.state_mut().expand(texture);
    h.run();
    assert_eq!(h.state().expanded.as_deref(), Some(texture));
    h.get_by_label("Edit match…").click();
    h.run();
    assert_eq!(h.state().selection, id_selection("landscape/empty"));
    assert!(h.query_by_label("Edit match…").is_none());
    assert!(h.state().expanded.is_none());
    assert_editor(&h, "landscape/empty");
}

#[test]
fn toggling_two_keyed_sets_down_to_one_opens_its_editor() {
    use sdc_gui::model::Selection;
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    let keys: Vec<_> = sdc_gui::model::sets::views(&m)
        .iter()
        .filter(|v| !v.key.is_empty())
        .map(|v| v.key.clone())
        .take(2)
        .collect();
    assert_eq!(keys.len(), 2);
    m.click_set(&keys[0], false, false);
    m.click_set(&keys[1], true, false);
    assert!(m.in_bulk());
    m.expanded = Some("arch/wall.png".into());
    m.click_set(&keys[1], true, false);
    assert_eq!(m.selection, Selection::Set(keys[0].clone()));
    assert!(!m.in_bulk());
    assert!(m.expanded.is_none());
    assert_eq!(m.centre_front(), sdc_gui::model::CentreTab::Selection);
    assert!(sdc_gui::ui::set_editor_shown(&m, false));
}

#[test]
fn go_to_set_on_a_key_less_sets_row_opens_that_set() {
    use sdc_gui::model::bottom::{filtered, At, ProblemsFilter};
    let id = "landscape/other";
    for click_link in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let mut m = key_less_sets(d.path());
        let filter = ProblemsFilter::Set(id.into());
        let rows = filtered(&m, Some(&filter));
        assert_eq!(rows.len(), 1);
        assert!(matches!(&rows[0].at, At::Set { key, id: row }
            if key.is_empty() && row == id));
        let texture = "landscape/rock01.png";
        m.expand(texture);
        m.open_problems(Some(filter));
        let mut h = harness(m);
        h.run();
        assert_eq!(h.state().expanded.as_deref(), Some(texture));
        if click_link {
            h.get_by_role_and_label(egui::accesskit::Role::Link, id)
                .click();
        } else {
            h.get_by_label("Go to set").click();
        }
        h.run();
        assert_eq!(h.state().selection, id_selection(id));
        assert!(h.state().expanded.is_none());
        assert_eq!(
            h.state().centre_front(),
            sdc_gui::model::CentreTab::Selection
        );
        assert_editor(&h, id);
    }
}

fn assert_editor(h: &Harness<'static, AppModel>, id: &str) {
    use egui::accesskit::Role;
    let centre = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
    if let Some(name) = h.state().input_name_of(id) {
        assert!(h
            .query_all_by_role(Role::TextInput)
            .any(|n| centre.contains(n.rect().center())
                && n.accesskit_node().value().as_deref() == Some(name.as_str())));
    } else {
        assert!(h
            .query_all_by_label(id)
            .any(|n| centre.contains(n.rect().center())));
    }
}

#[test]
fn a_wall_key_and_an_identical_document_id_are_two_different_cards() {
    use sdc_gui::model::Selection;
    for delete_document_card in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let (m, id) = wall_cards(d.path());
        let mut h = harness(m);
        h.run();
        let order = if delete_document_card {
            ["wall", id.as_str()]
        } else {
            [id.as_str(), "wall"]
        };
        for name in order {
            card(&h, name).click();
            h.run();
            let expected = if name == "wall" {
                Selection::Set(id.clone())
            } else {
                id_selection(&id)
            };
            assert_eq!(h.state().selection, expected);
            assert_eq!(card_selected(&h, "wall"), name == "wall");
            assert_eq!(card_selected(&h, &id), name == id.as_str());
            assert_editor(&h, name);
        }
        let [survivor, selected] = order;
        assert!(h.state_mut().delete_set(selected));
        assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
        h.run();
        let remaining = sdc_gui::model::sets::views(h.state());
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, survivor);
        assert_eq!(h.state().selection, Selection::None);
        assert!(!card_selected(&h, survivor));
    }
}

#[test]
fn a_click_with_an_absent_engine_key_keeps_the_selected_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    let rock = view_by_id(&m, "landscape/rock01").key;
    m.select_set(&rock);
    // A document with this id is present, but the caller supplied an engine key.
    m.click_set("landscape/other", true, false);
    assert_eq!(m.selection, sdc_gui::model::Selection::Set(rock));
    assert!(!m.in_bulk());
}

#[test]
fn a_right_click_preserves_a_selected_group_and_reopens_a_single_editor() {
    use sdc_gui::model::CentreTab;
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    let keys: Vec<_> = sdc_gui::model::sets::views(&m)
        .iter()
        .filter(|v| !v.key.is_empty())
        .map(|v| v.key.clone())
        .take(2)
        .collect();
    assert_eq!(keys.len(), 2);
    m.click_set(&keys[0], false, false);
    m.click_set(&keys[1], true, false);
    m.right_click_set(&keys[0]);
    assert_eq!(m.selected_sets(), keys);
    m.select_set(&keys[0]);
    m.open_settings_tab();
    assert_eq!(m.centre_front(), CentreTab::Settings);
    m.right_click_set(&keys[0]);
    assert_eq!(m.centre_front(), CentreTab::Selection);
}

fn card_selected(h: &Harness<'static, AppModel>, id: &str) -> bool {
    h.get_by_label_contains(&format!("Set {id}, position"))
        .accesskit_node()
        .is_selected()
        == Some(true)
}

#[test]
fn a_key_less_set_selected_by_its_id_opens_its_own_editor() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    pick_id(&mut m, "landscape/other");
    assert!(sdc_gui::ui::set_editor_shown(&m, false));
    let mut h = harness(m);
    h.run();
    let reasons: Vec<_> = h
        .query_all_by_label_contains("go to the fix")
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    assert!(reasons[0].contains("two.txt") && !reasons[0].contains("one.txt"));
    assert!(card_selected(&h, "landscape/other"));
    assert!(!card_selected(&h, "landscape/empty"));
}

#[test]
fn the_key_shaped_document_cards_editor_shows_its_own_reason() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = wall_cards(d.path());
    m.select_set(&id);
    let mut h = harness(m);
    h.run();
    assert!(!h.query_all_by_label_contains("go to the fix").any(|n| n
        .accesskit_node()
        .label()
        .is_some_and(|s| s.contains("wall-note.txt"))));
    pick_id(h.state_mut(), &id);
    h.run();
    assert!(h.query_all_by_label_contains("go to the fix").any(|n| n
        .accesskit_node()
        .label()
        .is_some_and(|s| s.contains("wall-note.txt"))));
    // Warm the ordinary key-first list and chip with the same spelling.
    h.state_mut()
        .open_problems(Some(sdc_gui::model::bottom::ProblemsFilter::Set(
            id.clone(),
        )));
    h.run();
    h.get_by_label("Set: wall ×");
    let name = h
        .query_all_by_label_contains("go to the fix")
        .filter_map(|n| n.accesskit_node().label())
        .find(|s| s.contains("wall-note.txt"))
        .unwrap();
    click_and_settle(&mut h, &name);
    let remove = format!("Remove wall-note.txt from {id}");
    assert!(
        h.query_by_label(&remove)
            .is_some_and(|n| n.accesskit_node().is_focused()),
        "the reason must show and focus the document card's own row"
    );
    h.get_by_label(&format!("Set: {id} ×"));
    h.state_mut()
        .open_problems(Some(sdc_gui::model::bottom::ProblemsFilter::Set(id)));
    h.run();
    h.get_by_label("Set: wall ×");
    assert!(h.query_by_label(&remove).is_none());
}

#[test]
fn output_rename_uses_only_the_selected_borrower() {
    use sdc_gui::model::rename::RenameMode;
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_json_sets(d.path());
    for id in ["landscape/empty", "landscape/other"] {
        assert!(
            m.renames_outputs(&view_by_id(&m, id)),
            "{id} qualifies for Outputs"
        );
    }
    pick_id(&mut m, "landscape/other");
    assert_eq!(m.rename_keys(RenameMode::Outputs), ["landscape/other"]);
    assert!(m.rename_keys(RenameMode::Inputs).is_empty());
    assert_eq!(m.selected_set_total(), 1);
    let mut h = harness(m);
    h.run();
    h.key_press(egui::Key::F2);
    h.run();
    let Some(sdc_gui::model::Dialog::Rename(dlg)) = &h.state().dialog else {
        panic!("F2 on the second borrower must open Rename");
    };
    assert_eq!(dlg.mode, RenameMode::Outputs);
    assert_eq!(dlg.keys, ["landscape/other"]);
}

#[test]
fn f2_on_a_texture_in_an_output_eligible_set_renames_the_input() {
    use sdc_gui::model::{rename::RenameMode, Dialog};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let item = "landscape/rock01.png";
    m.select_item(item);
    assert!(!m.rename_keys(RenameMode::Outputs).is_empty());
    let mut h = harness(m);
    h.run();
    h.key_press(egui::Key::F2);
    h.run();
    let Some(Dialog::Rename(dlg)) = &h.state().dialog else {
        panic!("expected Rename")
    };
    assert_eq!(dlg.mode, RenameMode::Inputs);
    assert_eq!(dlg.keys, [item]);
}

#[test]
fn the_unnamed_sets_focused_problems_row_reopens_its_editor() {
    for click_link in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let mut m = empty_id_set(d.path());
        pick_id(&mut m, "");
        let mut h = harness(m);
        h.run();
        let reason = h
            .get_by_label_contains("go to the fix")
            .accesskit_node()
            .label()
            .unwrap();
        click_and_settle(&mut h, &reason);
        assert!(h
            .get_by_label("Remove landscape/empty-id.txt from ")
            .accesskit_node()
            .is_focused());
        h.state_mut().expand("landscape/rock01.png");
        h.run();
        if click_link {
            h.get_by_role_and_label(egui::accesskit::Role::Link, "Unnamed set")
                .click();
        } else {
            h.get_by_label("Go to set").click();
        }
        h.run();
        assert_eq!(h.state().selection, id_selection(""));
        assert!(h.state().expanded.is_none());
        assert_eq!(h.state().resolved_selection().view().unwrap().id, "");
        let centre = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
        assert!(centre.contains(h.get_by_label("Also export").rect().center()));
    }
}

fn empty_id_set(dir: &std::path::Path) -> AppModel {
    let mut m = key_less_sets(dir);
    let rel = "landscape/empty-id.txt";
    std::fs::write(m.root.as_ref().unwrap().join(rel), "notes").unwrap();
    m.edit_docs("t", None, true, |_, doc| {
        doc.create_set("");
        doc.add_member("", rel);
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(view_by_id(&m, "").key.is_empty());
    m
}

fn check_more_unhides_own_kind(mut m: AppModel, id: &str, other_id: &str) {
    use engine::codes::Code;
    use sdc_gui::model::bottom::{is_hidden, problems, At};

    let other_key = view_by_id(&m, other_id).key;
    m.sets
        .iter_mut()
        .find(|s| s.key == other_key)
        .unwrap()
        .warnings = vec![Warning::new(Code::ValCoverage, "other card")];
    sdc_gui::model::sets::inject_view_warnings(
        &mut m,
        id,
        vec![
            Warning::new(Code::ValMipError, "mip"),
            Warning::new(Code::LossReencode, "encode"),
            Warning::new(Code::InNonfinite, "input"),
            Warning::new(Code::ChanNotGray, "colour"),
        ],
    );
    m.set_kind_hidden(Code::ChanNotGray, true);
    m.set_kind_hidden(Code::ValCoverage, true);
    let all = problems(&m);
    let hidden_row = |id: &str, code| {
        all.iter()
            .find(|p| {
                matches!(&p.at, At::Set { id: row_id, .. } if row_id == id)
                    && p.code == Some(code)
                    && is_hidden(p, &m.hidden_kinds())
            })
            .unwrap()
    };
    let own = hidden_row(id, Code::ChanNotGray);
    let other = hidden_row(other_id, Code::ValCoverage);
    pick_id(&mut m, id);
    let mut h = harness(m);
    h.run();
    click_and_settle(&mut h, "+2 more in Problems");
    let hidden = h.state().hidden_kinds();
    assert!(
        !is_hidden(own, &hidden) && is_hidden(other, &hidden),
        "only the selected card's kind should be shown, hidden: {hidden:?}"
    );
    // The link opens an ordinary filter. Clear it to see both cards' row visibility together.
    h.state_mut().open_problems(None);
    h.run();
    h.get_by_label(own.shown().as_ref());
    assert!(h.query_by_label(other.shown().as_ref()).is_none());
}

#[test]
fn more_in_problems_unhides_own_kind_for_a_key_shaped_id() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = wall_cards(d.path());
    check_more_unhides_own_kind(m, &id, "wall");
}

#[test]
fn more_in_problems_unhides_own_kind_for_an_empty_id() {
    let d = tempfile::tempdir().unwrap();
    check_more_unhides_own_kind(empty_id_set(d.path()), "", "landscape/rock01");
}

#[test]
fn an_empty_id_has_editor_reasons_but_the_empty_filter_has_no_rows() {
    use sdc_gui::model::bottom::{filtered, problems, At, ProblemsFilter};
    let d = tempfile::tempdir().unwrap();
    let mut m = empty_id_set(d.path());
    assert!(problems(&m)
        .iter()
        .any(|p| matches!(&p.at, At::Set { key, id }
        if key.is_empty() && id.is_empty())
            && p.text.contains("empty-id.txt")));
    pick_id(&mut m, "");
    let mut h = harness(m);
    h.run();
    assert_eq!(h.state().selection, id_selection(""));
    assert!(filtered(h.state(), Some(&ProblemsFilter::Set(String::new()))).is_empty());
    let centre = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
    let reasons: Vec<_> = h
        .query_all_by_label_contains("go to the fix")
        .filter(|n| centre.contains(n.rect().center()))
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    assert!(reasons[0].contains("empty-id.txt"), "{reasons:?}");
    assert!(!h
        .query_all_by_label("Ready: nothing in this set needs you.")
        .any(|n| centre.contains(n.rect().center())));
    click_and_settle(&mut h, &reasons[0]);
    assert!(
        h.query_by_label("Remove landscape/empty-id.txt from ")
            .is_some_and(|n| n.accesskit_node().is_focused()),
        "the empty-id card must show and focus its own row"
    );
    h.get_by_role_and_label(egui::accesskit::Role::Link, "Unnamed set");
    assert!(filtered(h.state(), Some(&ProblemsFilter::Set(String::new()))).is_empty());
}

#[test]
fn a_key_gain_returns_the_scrolled_editor_to_its_top() {
    use sdc_gui::model::{CentreTab, Selection};
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_json_sets(d.path());
    pick_id(&mut m, "landscape/empty");
    let mut h = harness_at(m, egui::vec2(1400.0, 420.0));
    h.run();
    let offset = |h: &Harness<'static, AppModel>| {
        h.ctx
            .data(|d| d.get_temp::<f32>(egui::Id::unique("set-editor-observed-offset")))
            .expect("2D editor drew its scroll area")
    };
    assert_eq!(offset(&h), 0.0);
    let centre = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
    h.query_all_by_label("Advanced TruePBR settings")
        .find(|n| centre.x_range().contains(n.rect().center().x))
        .unwrap()
        .scroll_to_me();
    h.run();
    assert!(offset(&h) > 0.0, "precondition: the editor scrolled");
    h.state_mut().open_json_editor();
    h.run();
    assert_eq!(h.state().centre_tab, CentreTab::Json);
    give_a_file(h.state_mut(), "landscape/empty", "landscape/one.png");
    h.run();
    let v = view_by_id(h.state(), "landscape/empty");
    assert!(!v.key.is_empty());
    assert_eq!(h.state().selection, Selection::Set(v.key));
    assert_eq!(h.state().centre_tab, CentreTab::Selection);
    assert_eq!(offset(&h), 0.0);
}

#[test]
fn choosing_a_keyed_set_over_a_texture_opens_its_editor() {
    use sdc_gui::model::{CentreTab, Selection};
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    let rock = view_by_id(&m, "landscape/rock01").key;
    m.expand("landscape/rock01.png");
    m.select_set(&rock);
    assert!(m.expanded.is_none());
    assert_eq!(m.selection, Selection::Set(rock));
    assert_eq!(m.centre_front(), CentreTab::Selection);
    assert!(m.reveal_centre);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Also export");
}

#[test]
fn a_key_less_editor_resolves_once_per_view_revision() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_json_sets(d.path());
    pick_id(&mut m, "landscape/other");
    let mut h = harness(m);
    h.run();
    let before = h.state().resolved_selection();
    assert_eq!(before.view().unwrap().id, "landscape/other");
    assert!(before.view().unwrap().key.is_empty());
    let n = h.state().selection_builds.get();
    let work = sdc_gui::model::identity_work();
    h.run_steps(5);
    assert_eq!(
        h.state().selection_builds.get(),
        n,
        "idle editor rebuilt identity"
    );
    assert_eq!(
        sdc_gui::model::identity_work(),
        work,
        "editor copied or searched for its set"
    );
    assert!(std::sync::Arc::ptr_eq(
        &before,
        &h.state().resolved_selection()
    ));
    pick_id(h.state_mut(), "landscape/empty");
    h.run();
    let next = h.state().resolved_selection();
    assert_eq!(next.view().unwrap().id, "landscape/empty");
    assert_eq!(h.state().selection_builds.get(), n + 1);
    h.state_mut().invalidate_views();
    h.run_steps(5);
    let fresh = h.state().resolved_selection();
    assert!(!std::sync::Arc::ptr_eq(&next.views, &fresh.views));
    assert_eq!(
        h.state().selection_builds.get(),
        n + 2,
        "one build for replacement views"
    );
    pick_id(h.state_mut(), "missing-card");
    h.run_steps(5);
    assert!(h.state().resolved_selection().view().is_none());
    assert_eq!(
        h.state().selection_builds.get(),
        n + 3,
        "cache a missing target too"
    );
}

#[test]
fn the_selected_headers_output_name_tracks_document_edits() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_json_sets(d.path());
    let id = "landscape/other";
    m.edit_docs("t", None, true, |_, doc| {
        doc.set_output_name(id, Some("other-output"))
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    pick_id(&mut m, id);
    let mut h = harness(m);
    h.run();
    h.get_by_label_contains("Output name \"other-output\" is ignored");
    let work = sdc_gui::model::identity_work();
    h.run_steps(5);
    assert_eq!(sdc_gui::model::identity_work(), work);
    h.state_mut().clear_output_name(id);
    h.step();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert!(h.query_by_label_contains("other-output").is_none());
}

#[test]
fn the_records_header_counts_the_selected_borrowers_blocks() {
    use sdc_gui::model::cs_pbr::Kind;
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_json_sets(d.path());
    let id = "landscape/other";
    m.add_cs_block(id, Kind::Txst);
    m.add_cs_block(id, Kind::Mato);
    pick_id(&mut m, id);
    assert!(!m.resolved_selection().view().unwrap().shared);
    let mut h = harness_at(m, egui::vec2(1600.0, 4600.0));
    h.run();
    h.get_by_label("2 records");
    h.get_by_label(sdc_gui::ui::cs_pbr::SECTION).click();
    h.run();
    h.get_by_label("PGPatcher entry for statics:");
    let reads = h.state().doc_block_reads.get();
    let work = sdc_gui::model::identity_work();
    h.run_steps(5);
    assert_eq!(
        h.state().doc_block_reads.get(),
        reads,
        "the expanded records panel looked its blocks up again"
    );
    assert_eq!(
        sdc_gui::model::identity_work(),
        work,
        "the records header searched for its blocks"
    );
    pick_id(h.state_mut(), "landscape/empty");
    h.run();
    h.get_by_label("0 records");
}

#[test]
fn the_landscape_checkbox_uses_the_selected_views_txst() {
    use sdc_gui::model::cs_pbr::{Kind, PGP_BOX};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let id = "landscape/rock01";
    m.add_cs_block(id, Kind::Txst);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.select_set(&view_by_id(&m, id).key);
    let mut h = harness_at(m, egui::vec2(1600.0, 4600.0));
    h.run();
    h.get_by_label(PGP_BOX);
    let reads = h.state().doc_block_reads.get();
    h.run_steps(5);
    assert_eq!(
        h.state().doc_block_reads.get(),
        reads,
        "the Landscape checkbox looked its block up again"
    );
}

#[test]
fn a_warmed_partial_keeps_the_other_cards_member_storage() {
    use sdc_gui::worker::WorkerMsg;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.select_set(&view_by_id(&m, "landscape/rock01").key);
    let mut h = harness(m);
    h.run();
    h.state_mut().hold_plan_results = true;
    h.state_mut().set_field(
        "landscape/leaf.png",
        "alpha.policy",
        serde_json::json!("cutout"),
    );
    h.run_steps(2);
    let m = h.state_mut();
    let member_ptr = |m: &AppModel| m.resolved_selection().view().unwrap().members[0].as_ptr();
    let before = member_ptr(m);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let delivered = loop {
        m.poll();
        if let Some(msg @ WorkerMsg::Partial { seq, .. }) = m
            .take_held()
            .into_iter()
            .find(|msg| matches!(msg, WorkerMsg::Partial { seq, .. } if *seq > m.applied_seq))
        {
            m.handle_held(msg);
            break seq;
        }
        assert!(std::time::Instant::now() < until);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert!(!m.partial_pending);
    assert_eq!(
        m.applied_seq, delivered,
        "the delivered partial must have applied"
    );
    assert_eq!(before, member_ptr(m), "the splice cloned an unchanged set");
}

#[test]
fn advanced_reads_the_material_edit_in_the_same_frame() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_json_sets(d.path());
    let id = "landscape/other";
    pick_id(&mut m, id);
    let mut h = harness_at(m, egui::vec2(1600.0, 4600.0));
    h.run();
    h.get_by_label("Advanced TruePBR settings").click();
    h.run();
    let observed = |h: &Harness<'static, AppModel>| {
        h.ctx
            .data(|d| d.get_temp::<bool>(egui::Id::unique("advanced-observed-parallax")))
    };
    assert_eq!(observed(&h), Some(false));
    h.get_by_label("Parallax").click();
    for _ in 0..3 {
        h.step();
        if view_by_id(h.state(), id).settings.get("parallax") == Some(&serde_json::json!(true)) {
            assert_eq!(observed(&h), Some(true), "Advanced drew the pre-edit view");
            return;
        }
    }
    panic!("Material did not commit Parallax");
}

#[test]
fn repeated_selection_reads_do_not_construct_owned_view_keys() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_json_sets(d.path());
    pick_id(&mut m, "landscape/other");
    let active = m.resolved_selection();
    let start = sdc_gui::model::sets::root_reads();
    for _ in 0..5 {
        assert!(std::sync::Arc::ptr_eq(&active, &m.resolved_selection()));
    }
    assert_eq!(sdc_gui::model::sets::root_reads(), start);
}

fn pick_id(m: &mut AppModel, id: &str) {
    m.select_target(sdc_gui::model::SetTarget::Id(id.into()));
}

fn id_selection(id: &str) -> sdc_gui::model::Selection {
    sdc_gui::model::Selection::SetId(id.into())
}

fn give_a_file(m: &mut AppModel, id: &str, rel: &str) {
    let root = m.root.clone().unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
    })
    .save(root.join(rel))
    .unwrap();
    m.replan();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.edit_docs("t", None, true, |_, s| s.add_member(id, rel));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
}

fn view_by_id(m: &AppModel, id: &str) -> sdc_gui::model::sets::SetView {
    sdc_gui::model::sets::set_views(m)
        .into_iter()
        .find(|v| engine::sets::same_id(&v.id, id))
        .expect("the set has a card")
}

#[test]
fn a_full_plan_keeps_a_key_less_selection() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    pick_id(&mut m, "landscape/other");
    m.replan();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert_eq!(m.selection, id_selection("landscape/other"));
}

#[test]
fn a_key_less_selection_follows_the_set_once_it_has_a_key() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    pick_id(&mut m, "landscape/empty");
    give_a_file(&mut m, "landscape/empty", "landscape/one.png");
    let v = view_by_id(&m, "landscape/empty");
    assert!(
        !v.key.is_empty(),
        "precondition: the set has a key now: {v:?}"
    );
    assert_eq!(m.selection, sdc_gui::model::Selection::Set(v.key.clone()));
}

#[test]
fn a_mixed_case_key_less_selection_follows_the_set_once_it_has_a_key() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.edit_docs("t", None, true, |_, s| s.create_set("Landscape/Big"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let v = view_by_id(&m, "Landscape/Big");
    assert!(
        v.key.is_empty() && v.id == "Landscape/Big",
        "precondition: {v:?}"
    );
    pick_id(&mut m, "Landscape/Big");
    give_a_file(&mut m, "Landscape/Big", "landscape/big.png");
    let v = view_by_id(&m, "landscape/big");
    assert!(
        !v.key.is_empty(),
        "precondition: the set has a key now: {v:?}"
    );
    assert_eq!(m.selection, sdc_gui::model::Selection::Set(v.key.clone()));
}

#[test]
fn a_keyed_selection_follows_the_set_once_it_loses_its_key() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.edit_docs("t", None, true, |_, s| s.create_set("landscape/solo"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    give_a_file(&mut m, "landscape/solo", "landscape/solo.png");
    let v = view_by_id(&m, "landscape/solo");
    assert!(!v.key.is_empty(), "precondition: a keyed set: {v:?}");
    m.select_set(&v.key);
    std::fs::remove_file(m.root.clone().unwrap().join("landscape/solo.png")).unwrap();
    m.replan();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let v = view_by_id(&m, "landscape/solo");
    assert!(
        v.key.is_empty(),
        "precondition: the set lost its key: {v:?}"
    );
    assert_eq!(m.selection, id_selection("landscape/solo"));
}

fn two_selected_created_sets(dir: &std::path::Path) -> AppModel {
    let mut m = pbr_opened(dir);
    for (id, file) in [
        ("landscape/first", "landscape/one.png"),
        ("landscape/active", "landscape/two.png"),
    ] {
        m.edit_docs("t", None, true, |_, s| s.create_set(id));
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        give_a_file(&mut m, id, file);
    }
    let first = view_by_id(&m, "landscape/first");
    let active = view_by_id(&m, "landscape/active");
    m.click_set(&first.key, false, false);
    m.click_set(&active.key, true, false);
    assert_eq!(m.selected_sets(), [first.key, active.key.clone()]);
    assert_eq!(m.selection, sdc_gui::model::Selection::Set(active.key));
    m
}

#[test]
fn reload_keeps_the_active_set_when_both_selected_keys_disappear() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_selected_created_sets(d.path());
    let root = m.root.as_ref().unwrap();
    for file in ["landscape/one.png", "landscape/two.png"] {
        std::fs::remove_file(root.join(file)).unwrap();
    }
    m.reload_changed();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(view_by_id(&m, "landscape/first").key.is_empty());
    assert!(view_by_id(&m, "landscape/active").key.is_empty());
    assert_eq!(m.selection, id_selection("landscape/active"));
    assert!(m.multi.keys.is_empty());
}

#[test]
fn reload_keeps_the_surviving_key_when_the_active_set_loses_its_key() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_selected_created_sets(d.path());
    let first = view_by_id(&m, "landscape/first").key;
    std::fs::remove_file(m.root.as_ref().unwrap().join("landscape/two.png")).unwrap();
    m.reload_changed();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(view_by_id(&m, "landscape/active").key.is_empty());
    assert_eq!(m.selection, sdc_gui::model::Selection::Set(first));
    assert!(m.multi.keys.is_empty());
}

#[test]
fn a_selected_set_that_gains_its_key_brings_the_selection_tab_back() {
    use sdc_gui::model::CentreTab;
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    pick_id(&mut m, "landscape/empty");
    m.open_json_editor();
    let mut h = harness(m);
    h.run();
    assert_eq!(h.state().centre_tab, CentreTab::Json, "precondition");
    give_a_file(h.state_mut(), "landscape/empty", "landscape/one.png");
    h.run();
    let landed = view_by_id(h.state(), "landscape/empty");
    assert!(!landed.key.is_empty());
    assert_eq!(
        h.state().selection,
        sdc_gui::model::Selection::Set(landed.key)
    );
    assert_eq!(h.state().centre_tab, CentreTab::Selection);
}

#[test]
fn deleting_a_selected_key_less_set_selects_no_other_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    pick_id(&mut m, "landscape/other");
    assert!(m.delete_set("landscape/other"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert_eq!(m.selection, sdc_gui::model::Selection::None);
}

/// B3-16, B3-17 (Note JP5; F6 N1): a key-less set is named by its id. Each of two key-less sets lists and reasons
/// its own missing entry and not the other's, through the model, for both sets; the empty string names no set. The
/// editor opens the chosen key-less set by document id: its status line reads
/// "Not ready:" with its own reason, not "no files yet".
#[test]
fn a_key_less_set_lists_and_reasons_its_own_rows_only() {
    use sdc_gui::model::bottom::{filtered, scope_key, set_reasons, At, ProblemsFilter};
    use sdc_gui::ui::sets_column::NO_FILES_YET;
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    for (id, mine, theirs) in [
        ("landscape/empty", "one.txt", "two.txt"),
        ("landscape/other", "two.txt", "one.txt"),
    ] {
        let r = set_reasons(&m, id);
        assert_eq!(r.len(), 1, "{id}: {r:?}");
        assert!(
            r[0].text.contains(mine) && !r[0].text.contains(theirs),
            "{id}: {r:?}"
        );
        for f in [
            ProblemsFilter::Set(id.into()),
            ProblemsFilter::SetNotes(id.into()),
        ] {
            let rows = filtered(&m, Some(&f));
            let texts: Vec<&str> = rows.iter().map(|p| p.text.as_str()).collect();
            let want = usize::from(matches!(f, ProblemsFilter::Set(_)));
            assert_eq!(rows.len(), want, "{f:?}: {texts:?}");
            assert!(
                rows.iter().all(|p| p.text.contains(mine)),
                "{f:?}: {texts:?}"
            );
            assert!(rows.iter().all(|p| matches!(
                &p.at,
                At::Set { key, id: i } if key.is_empty() && i == id
            )));
        }
    }
    assert!(
        set_reasons(&m, "").is_empty(),
        "the empty string names no set"
    );
    assert!(filtered(&m, Some(&ProblemsFilter::Set(String::new()))).is_empty());
    let views = sdc_gui::model::sets::set_views(&m);
    for v in views.iter().filter(|v| v.key.is_empty()) {
        assert_eq!(scope_key(v), v.id, "a key-less set is named by its id");
    }
    let first = views.iter().find(|v| v.key.is_empty()).unwrap().id.clone();
    let (mine, theirs) = if first == "landscape/empty" {
        ("one.txt", "two.txt")
    } else {
        ("two.txt", "one.txt")
    };
    m.go_to_target(sdc_gui::model::SetTarget::Id(first.clone()), None);
    let mut h = harness(m);
    h.run();
    assert!(h.query_by_label(NO_FILES_YET).is_none(), "it has a row");
    let reasons: Vec<String> = h
        .query_all_by_label_contains("— go to the fix")
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    assert!(
        reasons[0].contains(mine) && !reasons[0].contains(theirs),
        "{reasons:?}"
    );
}

/// Re-review F1 (Note JP5, BF): a key-less set's id keeps the case it was typed in, while a keyed set's id is folded,
/// so the filter finds the set by its id compared by fold once a file lands in it and it has a key.
#[test]
fn a_key_less_sets_filter_follows_a_mixed_case_id() {
    use sdc_gui::model::bottom::{filtered, scope_key, ProblemsFilter};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let root = m.root.clone().unwrap();
    std::fs::write(root.join("landscape/one.txt"), "notes").unwrap();
    m.edit_docs("t", None, true, |_, s| {
        s.create_set("Landscape/Big");
        s.add_member("Landscape/Big", "landscape/one.txt")
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let same = |v: &sdc_gui::model::sets::SetView| v.id.eq_ignore_ascii_case("landscape/big");
    let v = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(same)
        .expect("the created set has a card");
    assert!(
        v.key.is_empty() && v.id == "Landscape/Big",
        "precondition: a key-less set with its typed id: {v:?}"
    );
    let f = ProblemsFilter::Set(scope_key(&v).to_string());
    m.open_problems(Some(f.clone()));
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
    })
    .save(root.join("landscape/one.png"))
    .unwrap();
    m.replan();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.edit_docs("t", None, true, |_, s| {
        s.add_member("Landscape/Big", "landscape/one.png")
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let v = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(same)
        .expect("the set still has a card");
    assert!(
        !v.key.is_empty(),
        "precondition: the set has a key now: {v:?}"
    );
    assert_eq!(
        m.bottom.filter,
        Some(f.clone()),
        "nothing resets the filter"
    );
    let rows = filtered(&m, Some(&f));
    let texts: Vec<&str> = rows.iter().map(|p| p.text.as_str()).collect();
    let by_key = filtered(&m, Some(&ProblemsFilter::Set(v.key.clone())));
    assert_eq!(
        rows.len(),
        by_key.len(),
        "the rows of a filter by its key: {texts:?}"
    );
    assert_eq!(*rows, *by_key);
    assert!(rows.iter().any(|p| p.text.contains("one.txt")), "{texts:?}");
}

/// Follow-up F7 (Note JR11): a filter opened by a keyed set's key follows the set once it loses every file and has no
/// key: it lists the key-less set's rows, the same rows as a filter by its id.
#[test]
fn a_keyed_sets_filter_follows_the_set_once_it_loses_its_key() {
    use sdc_gui::model::bottom::{filtered, scope_key, ProblemsFilter};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let root = m.root.clone().unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
    })
    .save(root.join("landscape/one.png"))
    .unwrap();
    // A text file no plan item matches: the entry stays and shows as "No file matches" once the image is gone.
    std::fs::write(root.join("landscape/one.txt"), "notes").unwrap();
    m.replan();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.edit_docs("t", None, true, |_, s| {
        s.create_set("landscape/big");
        s.add_member("landscape/big", "landscape/one.png");
        s.add_member("landscape/big", "landscape/one.txt")
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let find = |m: &sdc_gui::model::AppModel| {
        sdc_gui::model::sets::set_views(m)
            .into_iter()
            .find(|v| v.id == "landscape/big")
            .expect("the set has a card")
    };
    let v = find(&m);
    assert!(!v.key.is_empty(), "precondition: the set has a key: {v:?}");
    let f = ProblemsFilter::Set(v.key.clone());
    m.open_problems(Some(f.clone()));
    // The set's only real file goes; the other files of the folder stay, so the plan still has items.
    std::fs::remove_file(root.join("landscape/one.png")).unwrap();
    m.replan();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let v = find(&m);
    assert!(
        v.key.is_empty(),
        "precondition: every entry is missing, so no key: {v:?}"
    );
    // Final wave Group 6 (ledger 259): the rows are those of the filter the model holds, not of the local `f`.
    let installed = m.bottom.filter.clone();
    let rows = filtered(&m, installed.as_ref());
    let texts: Vec<&str> = rows.iter().map(|p| p.text.as_str()).collect();
    let by_id = filtered(&m, Some(&ProblemsFilter::Set(scope_key(&v).to_string())));
    assert!(
        rows.iter().any(|p| p.text.contains("one.txt")),
        "the missing entry's row: {texts:?}"
    );
    assert_eq!(*rows, *by_id, "the rows of a filter by its id");
    assert_eq!(installed, Some(f.clone()), "nothing resets the filter");
    // The chip resolves the key as the list does: it names the set, not the old output-root path.
    let mut h = harness(m);
    h.run();
    assert!(
        h.query_all_by_label_contains("Set: landscape/big").count() > 0,
        "the chip names the set by its id"
    );
}

/// A set filter's rows after the output root changes to `to` (an export folder), for a filter opened on `landscape/leaf`
/// under `<tmp>/out/textures/sub`. `also` creates a key-less set of that name first.
fn leaf_filter_after_a_root_change(
    d: &std::path::Path,
    to: std::path::PathBuf,
    also: Option<&str>,
) -> (Vec<String>, Vec<sdc_gui::model::bottom::Problem>) {
    use sdc_gui::model::bottom::{filtered, ProblemsFilter};
    let mut m = pbr_opened(d);
    let root = m.root.clone().unwrap();
    choose_export(&mut m, d.join("out/textures/sub"));
    if let Some(id) = also {
        std::fs::write(root.join("landscape/stray.txt"), "notes").unwrap();
        m.edit_docs("t", None, true, |_, s| {
            s.create_set(id);
            s.add_member(id, "landscape/stray.txt")
        });
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    }
    let views = sdc_gui::model::sets::set_views(&m);
    let leaf = views
        .iter()
        .find(|v| v.id == "landscape/leaf")
        .expect("the leaf set has a card");
    assert!(!leaf.key.is_empty(), "precondition: {leaf:?}");
    if let Some(id) = also {
        let v = views.iter().find(|v| v.id == id).expect("the key-less set");
        assert!(v.key.is_empty(), "precondition: {v:?}");
    }
    let f = ProblemsFilter::Set(leaf.key.clone());
    m.open_problems(Some(f.clone()));
    assert!(
        !filtered(&m, Some(&f)).is_empty(),
        "precondition: leaf has rows"
    );
    choose_export(&mut m, to);
    assert!(m.bottom.filter.is_some(), "nothing resets the filter");
    let ids = sdc_gui::model::sets::set_views(&m)
        .iter()
        .map(|v| v.id.clone())
        .collect();
    let f = m.bottom.filter.clone();
    (ids, filtered(&m, f.as_ref()).to_vec())
}

/// Review of Task 12a (plan-mandated): a filter opened by a set's key under one output root keeps listing that set's
/// rows after the root changes to an ancestor, even when a key-less set's id rebuilds the old key under the new root.
#[test]
fn a_set_filter_keeps_its_set_when_the_root_moves_to_an_ancestor() {
    use sdc_gui::model::bottom::At;
    let d = tempfile::tempdir().unwrap();
    let nested = "sub/textures/landscape/leaf";
    let (ids, rows) = leaf_filter_after_a_root_change(d.path(), d.path().join("out"), Some(nested));
    assert!(ids.iter().any(|i| i == nested), "{ids:?}");
    let texts: Vec<&str> = rows.iter().map(|p| p.text.as_str()).collect();
    assert!(!rows.is_empty(), "the leaf set's rows: {texts:?}");
    assert!(
        !rows.iter().any(|p| p.text.contains("stray.txt")),
        "another set's row: {texts:?}"
    );
    assert!(
        rows.iter().all(|p| match &p.at {
            At::Set { id, .. } => id == "landscape/leaf",
            At::Texture(t) => t == "landscape/leaf.png",
            _ => false,
        }),
        "{texts:?}"
    );
}

#[test]
fn a_root_move_keeps_the_original_bulk_reset_targets() {
    use engine::sets::set_key;
    use sdc_gui::model::{Dialog, Selection};
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("inputs");
    let root = d.path().join("R");
    let old = root.join("sub");
    std::fs::create_dir_all(input.join("sub/sub")).unwrap();
    for path in ["a.png", "sub/a.png", "sub/sub/a.png"] {
        image::RgbImage::from_pixel(16, 16, image::Rgb([90, 60, 30]))
            .save(input.join(path))
            .unwrap();
    }
    let mut m = opened_as_the_app(input, old.clone());
    m.click_set(&set_key(&old, "a"), false, false);
    m.click_set(&set_key(&old, "sub/a"), true, false);
    assert_eq!(
        m.selected_sets(),
        [set_key(&old, "a"), set_key(&old, "sub/a")]
    );
    choose_export(&mut m, root.clone());
    assert_eq!(m.selection, Selection::Set(set_key(&root, "sub/a")));
    m.ask_reset_all();
    let Some(Dialog::ConfirmResetAll(keys)) = m.dialog else {
        panic!("the selected sets have reset targets");
    };
    assert_eq!(keys, ["a.png", "sub/a.png"]);
}

/// The plain case of the same rekey: an ordinary root change leaves the open filter on its set.
#[test]
fn a_set_filter_keeps_its_set_when_the_root_changes() {
    let d = tempfile::tempdir().unwrap();
    let (_, rows) = leaf_filter_after_a_root_change(d.path(), d.path().join("elsewhere"), None);
    let texts: Vec<&str> = rows.iter().map(|p| p.text.as_str()).collect();
    assert!(!rows.is_empty(), "the leaf set's rows: {texts:?}");
}

/// A key-less entry's id is typed by the user and read as it stands: one that reads like a key under the old output
/// root (the folded root, a slash, `wall`) is still an id, so a root change leaves the filter on it and never on the
/// different set `wall` whose key the rewritten value would be.
#[test]
fn a_key_less_ids_filter_is_not_rekeyed_when_the_root_changes() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = move_root_with_id_filter(d.path(), None);
    assert_eq!(m.selection, id_selection(&id));
    let entry = view_by_id(&m, &id);
    assert!(entry.key.is_empty());
    assert!(entry.missing.iter().any(|v| matches!(v,
        sdc_gui::model::sets::Missing::File(path) if path.ends_with("stray.txt"))));
    assert_ne!(view_by_id(&m, "wall").key, id);
}

#[test]
fn a_keyed_selection_moves_when_the_filter_names_a_key_less_id() {
    let d = tempfile::tempdir().unwrap();
    let selected = "landscape/rock01";
    let (m, _) = move_root_with_id_filter(d.path(), Some(selected));
    assert_eq!(
        m.selection,
        sdc_gui::model::Selection::Set(engine::sets::set_key(&m.sets_root(), selected)),
        "the selected set moves even when the filter names an explicit id"
    );
}

fn move_root_with_id_filter(dir: &std::path::Path, keyed_set: Option<&str>) -> (AppModel, String) {
    use sdc_gui::model::bottom::{filtered, At, ProblemsFilter};
    let mut m = pbr_opened(dir);
    let root = m.root.clone().unwrap();
    std::fs::write(root.join("landscape/stray.txt"), "notes").unwrap();
    // The entry's id: what the key of a set `wall` is under the plan's output root now.
    let id = engine::sets::set_key(&m.sets_root(), "wall");
    m.replan();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.edit_docs("t", None, true, |_, s| {
        s.create_set(&id);
        s.add_member(&id, "landscape/stray.txt")
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let views = sdc_gui::model::sets::set_views(&m);
    let entry = views.iter().find(|v| v.id == id).expect("the entry's card");
    assert!(entry.key.is_empty(), "precondition: key-less: {entry:?}");
    assert!(
        !views.iter().any(|v| v.id == "wall"),
        "precondition: no set wall yet"
    );
    if let Some(selected) = keyed_set {
        m.select_set(&engine::sets::set_key(&m.sets_root(), selected));
    } else {
        pick_id(&mut m, &id);
    }
    let f = ProblemsFilter::Set(id.clone());
    m.open_problems(Some(f.clone()));
    let before = filtered(&m, Some(&f));
    assert!(
        before.iter().any(|p| p.text.contains("stray.txt")),
        "precondition: the entry's row"
    );
    // The different set `wall` comes with the plan the move starts: one file at the top of the folder, too small for
    // its format, so it has rows; its key under the new root is what a rewritten id would be.
    image::ImageBuffer::from_fn(3, 3, |x, y| {
        image::Rgb([(x * 40) as u8, (y * 40) as u8, 90u8])
    })
    .save(root.join("wall.png"))
    .unwrap();
    choose_export(&mut m, dir.join("elsewhere"));
    assert!(
        sdc_gui::model::sets::set_views(&m)
            .iter()
            .any(|v| v.id == "wall" && !v.key.is_empty()),
        "precondition: set wall has a key now"
    );
    let rows = filtered(&m, Some(&f));
    let texts: Vec<&str> = rows.iter().map(|p| p.text.as_str()).collect();
    assert!(
        rows.iter().any(|p| p.text.contains("stray.txt")),
        "the entry's row: {texts:?}"
    );
    assert!(
        rows.iter().all(|p| match &p.at {
            At::Set { key, id: i } => key.is_empty() && *i == id,
            _ => false,
        }),
        "another set's row: {texts:?}"
    );
    assert_eq!(m.bottom.filter, Some(f.clone()), "the id is left as it is");
    (m, id)
}

/// Final review I2 (Note JP5): a filter opened on a key-less set's id follows the set once a file lands in it and it
/// has a key: it lists the keyed set's rows (the same rows as a filter by its key), never another set's, and its chip
/// names the set.
#[test]
fn a_key_less_sets_filter_follows_the_set_once_it_has_a_key() {
    use sdc_gui::model::bottom::{filtered, At, ProblemsFilter};
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    let f = ProblemsFilter::Set("landscape/empty".into());
    m.open_problems(Some(f.clone()));
    // The fix a modder makes for "No file matches landscape/one.txt": put the texture in place and add it to the set.
    let root = m.root.clone().unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
    })
    .save(root.join("landscape/one.png"))
    .unwrap();
    m.replan();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.edit_docs("t", None, true, |_, s| {
        s.add_member("landscape/empty", "landscape/one.png")
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let v = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(|v| v.id == "landscape/empty")
        .expect("the created set has a card");
    assert!(
        !v.key.is_empty() && v.key != v.id,
        "precondition: the set has a key now: {v:?}"
    );
    assert!(
        v.members.iter().any(|k| k == "landscape/one.png"),
        "precondition: {v:?}"
    );
    assert_eq!(
        m.bottom.filter,
        Some(f.clone()),
        "nothing resets the filter"
    );
    let rows = filtered(&m, Some(&f));
    let texts: Vec<&str> = rows.iter().map(|p| p.text.as_str()).collect();
    let by_key = filtered(&m, Some(&ProblemsFilter::Set(v.key.clone())));
    assert_eq!(
        rows.len(),
        by_key.len(),
        "the rows of a filter by its key: {texts:?}"
    );
    assert_eq!(*rows, *by_key);
    assert!(!rows.is_empty(), "the set's rows: {texts:?}");
    assert!(rows.iter().any(|p| p.text.contains("one.txt")), "{texts:?}");
    assert!(
        !rows.iter().any(|p| p.text.contains("two.txt")),
        "{texts:?}"
    );
    assert!(
        rows.iter().all(|p| match &p.at {
            At::Set { key, .. } => *key == v.key,
            At::Texture(t) => v.members.contains(t),
            _ => false,
        }),
        "{texts:?}"
    );
    let mut h = harness(m);
    h.run();
    assert!(
        h.query_all_by_label_contains("Set: landscape/empty")
            .count()
            > 0,
        "the chip names the set"
    );
    assert!(
        h.query_all_by_label_contains("one.txt").count() > 0,
        "the row is listed"
    );
}

/// t0 Minor 2 (Note JP5): a key-less set's reason on the editor's status line opens a Problems filter that lists that
/// set's row and no other key-less set's (`status_act` names the set by `scope_key`, not by its empty key).
#[test]
fn a_key_less_sets_reason_opens_its_own_rows() {
    use sdc_gui::model::bottom::{filtered, At, ProblemsFilter};
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_sets(d.path());
    // Open the chosen document card by id.
    let first = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(|v| v.key.is_empty())
        .unwrap()
        .id;
    let (mine, theirs) = if first == "landscape/empty" {
        ("one.txt", "two.txt")
    } else {
        ("two.txt", "one.txt")
    };
    m.go_to_target(sdc_gui::model::SetTarget::Id(first.clone()), None);
    let mut h = harness(m);
    h.run();
    h.get_by_label_contains("— go to the fix").click();
    h.step();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    let f = h.state().bottom.filter.clone();
    assert_eq!(f, Some(ProblemsFilter::Set(first.clone())));
    let rows = filtered(h.state(), f.as_ref());
    let texts: Vec<&str> = rows.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(rows.len(), 1, "{texts:?}");
    assert!(
        rows[0].text.contains(mine) && !rows[0].text.contains(theirs),
        "{texts:?}"
    );
    assert!(matches!(&rows[0].at, At::Set { key, id } if key.is_empty() && *id == first));
}

/// B3-17 (Note JP5): a key-less set with a counted row is not ready like any set: its card's hover says what Ready
/// means, not "no files yet".
#[test]
fn a_key_less_set_with_rows_reads_not_ready_on_its_card() {
    use sdc_gui::model::bottom::{flagged, no_files_yet, set_ready};
    use sdc_gui::ui::sets_column::{pill_hover, NOT_READY_MEANS, NO_FILES_YET};
    use sdc_gui::ui::status::Status;
    let d = tempfile::tempdir().unwrap();
    let m = key_less_sets(d.path());
    let v = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(|v| v.id == "landscape/other")
        .unwrap();
    assert!(!set_ready(&m, &v), "amber");
    assert!(!no_files_yet(&flagged(&m), &v));
    let hover = pill_hover(&m, &flagged(&m), &v, Status::Warning);
    assert!(hover.contains(NOT_READY_MEANS), "{hover}");
    assert!(!hover.contains(NO_FILES_YET), "{hover}");
}

/// B3-17 (Note JP5): a created set with no entry has no row, so its status line still says it has no files yet,
/// with no reason buttons, as its card does (`a_set_with_no_files_yet_says_so_on_its_card`).
#[test]
fn a_set_with_no_entry_says_no_files_yet_on_its_status_line() {
    use sdc_gui::model::bottom::{flagged, no_files_yet, set_reasons};
    use sdc_gui::ui::sets_column::NO_FILES_YET;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.edit_docs("t", None, true, |_, s| s.create_set("landscape/empty"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let v = sdc_gui::model::sets::set_views(&m)
        .into_iter()
        .find(|v| v.id == "landscape/empty")
        .expect("the created set has a card");
    assert!(
        v.key.is_empty() && v.members.is_empty() && v.missing.is_empty(),
        "precondition: {v:?}"
    );
    assert!(no_files_yet(&flagged(&m), &v));
    assert!(set_reasons(&m, "landscape/empty").is_empty());
    m.go_to_target(
        sdc_gui::model::SetTarget::Id("landscape/empty".into()),
        None,
    );
    let mut h = harness(m);
    h.run();
    h.get_by_label(NO_FILES_YET);
    assert_eq!(h.query_all_by_label_contains("— go to the fix").count(), 0);
}

#[test]
fn one_problem_reads_singular_in_the_export_hover() {
    use engine::codes::{Code, Warning};
    let (_d, m) = only(&[Warning::new(Code::ChanNotGray, "not grey")]);
    let t = sdc_gui::ui::header::export_hover(&m);
    assert!(t.ends_with(", 1 problem"), "{t}");
}

/// Final fix wave B9: the 3D Details footer's notes header counts through `counted`: "1 note", "2 notes".
#[test]
fn the_3d_notes_header_reads_singular_and_plural() {
    use sdc_gui::model::viewport3d::Viewport3d;
    for (n, header) in [(1, "1 note"), (2, "2 notes")] {
        let mut v = Viewport3d::new();
        v.report = Some(render3d::LoadReport {
            warnings: (0..n).map(|i| format!("note {i}")).collect(),
            ..Default::default()
        });
        let mut h = Harness::builder()
            .with_context_setup(|ctx| {
                sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
            })
            .with_size(egui::vec2(600.0, 300.0))
            .build_ui_state(
                |ui, v: &mut Viewport3d| sdc_gui::ui::viewport3d::details(ui, v),
                v,
            );
        h.run();
        h.get_by_label(header);
    }
}

/// Final fix wave B3: the kinds rows with no code are marked under are stored in users' `sdc-project.toml`, so
/// each string is pinned; a changed one would orphan every mark stored under it.
#[test]
fn the_stored_mark_kinds_never_change() {
    use sdc_gui::model::bottom::kind;
    assert_eq!(
        kind::ALL,
        [
            "NIF-WARNING",
            "PGP-PITFALL",
            "SET-MISSING-ENTRY",
            "RECORD-CHANGED",
            "XPMF-SKIP",
            "SKIN-SKIP",
            "READ-ONLY-FLAG",
            "CFG-PRESET-SKIPPED",
        ]
    );
    assert_eq!(
        kind::CFG_PRESET_SKIPPED,
        engine::codes::Code::CfgPresetSkipped.as_str()
    );
}
