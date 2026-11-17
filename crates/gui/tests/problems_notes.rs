//! Review E: Problems and Notes. Every code is a Problem or a Note once; a Note row says why it is fine.
mod support;
use engine::codes::Warning;
use sdc_gui::model::AppModel;
use support::*;

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
