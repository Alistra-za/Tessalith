//! Note IA2: the "Baked in" row of a model-space normal (`_msn`) picks the baker; the engine applies its swap or
//! invert (Note IA1). Temp dirs only.
mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::config::{MsnSource, Subtype, Target};
use image::{ImageBuffer, Rgb};
use sdc_gui::model::fields::{field_hidden, field_view, group_of, MSN_SOURCE};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use support::session;

const MSN: &str = "skin/body_msn.png";
const DIFFUSE: &str = "skin/body.png";

fn fixture(dir: &Path) -> AppModel {
    let r = dir.join("skin");
    std::fs::create_dir_all(&r).unwrap();
    for name in ["body.png", "body_msn.png"] {
        ImageBuffer::from_fn(32, 32, |x, y| Rgb([(x * 8) as u8, 128u8, (y * 8) as u8]))
            .save(r.join(name))
            .unwrap();
    }
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::High;
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    m
}

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

fn job(m: &AppModel, key: &str) -> engine::policy::ResolvedJob {
    m.item(key).unwrap().job.clone().unwrap()
}

#[test]
fn the_baker_row_is_msn_only_and_writes_the_key() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    let msn = job(&m, MSN);
    assert_eq!(msn.profile.subtype, Subtype::Msn);
    assert_eq!(group_of(MSN_SOURCE), "Type");
    assert!(!field_hidden(&msn.profile, MSN_SOURCE));
    assert_eq!(
        field_view(Target::Cs, &msn, MSN_SOURCE).unwrap().value,
        json!("skyrim")
    );
    assert!(field_hidden(&job(&m, DIFFUSE).profile, MSN_SOURCE));
    m.set_field(MSN, MSN_SOURCE, json!("swap_gb"));
    idle(&mut m);
    assert_eq!(job(&m, MSN).profile.channels.msn_source, MsnSource::SwapGb);
    assert_eq!(
        m.project.as_ref().unwrap().file_value(MSN, MSN_SOURCE),
        Some(json!("swap_gb"))
    );
}

#[test]
fn the_panel_shows_baked_in_for_an_msn() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.select_item(MSN);
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run_steps(4);
    h.get_by_label("Baked in");
    h.state_mut().select_item(DIFFUSE);
    h.run_steps(4);
    assert!(h.query_by_label("Baked in").is_none());
}
