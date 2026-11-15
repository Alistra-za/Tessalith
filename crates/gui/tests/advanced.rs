mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use sdc_gui::model::AppModel;
use support::*;

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 1000.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

#[test]
fn the_strip_opens_the_drawer_and_close_collapses_it() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    assert!(!h.state().settings.advanced_open);
    assert!(h.query_by_label("Close").is_none());
    h.get_by_label("Advanced").click();
    h.run();
    assert!(h.state().settings.advanced_open);
    h.get_by_label("Close").click();
    h.run();
    assert!(!h.state().settings.advanced_open);
}
