//! The set editor's CS PBR section (design §4, §6).
mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use sdc_gui::model::cs_pbr::{Kind, Statics, NOT_CHECKED};
use sdc_gui::model::sets::set_views;
use sdc_gui::model::AppModel;
use support::*;

fn tall(model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 2400.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

fn has(h: &Harness<'static, AppModel>, s: &str) -> bool {
    h.query_all_by_label_contains(s).next().is_some()
}

/// Final review I1: the JSON editor window lists only the entries the written file gets.
#[test]
fn the_json_editor_leaves_out_a_set_the_statics_rule_drops() {
    let d = tempfile::tempdir().unwrap();
    let mut model = pbr_opened_plain(d.path());
    model.add_cs_block("landscape/rock01", Kind::Txst); // no scan: the entry is dropped
    model.open_json_editor();
    let mut h = tall(model);
    h.run();
    assert!(has(&h, "0 set entries"));
    assert!(has(&h, "No PBR set has an entry yet."));
    h.state_mut()
        .set_cs_statics("landscape/rock01", Statics::Always);
    h.run();
    assert!(has(&h, "1 set entry "));
}

/// Trace-debug 2026-11-04 #3 (Note FO3): drawing a card never rewrites a stored value outside its field range.
#[test]
fn a_draw_keeps_an_out_of_range_template_value() {
    let d = tempfile::tempdir().unwrap();
    let mut model = pbr_opened(d.path());
    let id = "landscape/rock01";
    model.set_shader(id, Some(engine::shader::Shader::Pbr));
    model.add_cs_block(id, Kind::Txst);
    model
        .apply_cs_template(id, Kind::Txst, "LandscapeMountainSlab")
        .unwrap();
    let key = set_views(&model)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap()
        .key;
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_set(&key);
    let (steps, dirty) = (model.history.undo.len(), model.is_dirty());
    let mut h = tall(model);
    h.run();
    h.get_by_label(sdc_gui::ui::cs_pbr::SECTION).click();
    for _ in 0..3 {
        h.run();
    }
    let label = sdc_gui::model::cs_pbr::field_label("densityRandomization");
    assert!(has(&h, label), "the glint row {label} is drawn");
    let m = h.state();
    assert_eq!(m.history.undo.len(), steps, "a draw is not an edit");
    assert_eq!(m.is_dirty(), dirty, "a draw does not dirty the project");
    let entries: Vec<_> = sdc_gui::model::pbr_export::json_entries(m)
        .into_iter()
        .map(|(_, e)| e)
        .collect();
    let text = sdc_gui::pgp::writer::to_text(&sdc_gui::pgp::writer::document(&entries));
    assert!(text.contains("\"density_randomization\": 300.0"), "{text}");
}

#[test]
fn the_section_shows_the_block_template_refusals_and_checks() {
    let d = tempfile::tempdir().unwrap();
    let mut model = pbr_opened(d.path());
    let id = "landscape/rock01";
    // The fixture is a PBR landscape set (Note BM); as plain PBR it keeps the Advanced records panel.
    model.set_shader(id, Some(engine::shader::Shader::Pbr));
    model.add_cs_block(id, Kind::Txst);
    model
        .apply_cs_template(id, Kind::Txst, "LandscapeMountainSlab")
        .unwrap();
    model.set_cs_edids(id, Kind::Txst, &["Bad:Name".into()]);
    let key = set_views(&model)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap()
        .key;
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_set(&key);
    let mut h = tall(model);
    h.run();
    h.get_by_label(sdc_gui::ui::cs_pbr::SECTION).click();
    h.run();
    assert!(has(&h, "PBR texture set"));
    assert!(has(&h, "Add PBR material object"));
    // The combo box shows the applied template as its value, not a label.
    assert!(
        h.query_all_by_value("LandscapeMountainSlab")
            .next()
            .is_some(),
        "the applied template"
    );
    assert!(has(&h, "Bad:Name"));
    assert!(has(&h, "not written: EDID: contains a character"));
    assert!(has(&h, NOT_CHECKED));
    assert!(
        has(&h, "No scan yet"),
        "the statics line says why there is no entry"
    );
    h.get_by_label("Add PBR material object").click();
    h.run();
    assert!(h.state().cs_block(id, Kind::Mato).is_some());
}
