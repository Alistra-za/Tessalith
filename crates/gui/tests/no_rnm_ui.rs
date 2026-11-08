//! Chunk E Task 2: the GUI has no RNM section, outline or "Blends into" row, even when the engine still reads a layer file.
mod support;
use eframe::egui::{self, Color32};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use std::path::Path;
use std::time::Duration;
use support::session;

const N: &str = "m/iron_n.png";

fn fixture(dir: &Path, files: &[&str]) -> AppModel {
    let r = dir.join("m");
    std::fs::create_dir_all(&r).unwrap();
    for f in files {
        if *f == "iron.png" {
            ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
                .save(r.join(f))
                .unwrap();
        } else {
            ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
                .save(r.join(f))
                .unwrap();
        }
    }
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::High;
    m.open_folder(dir.to_path_buf());
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m
}

fn harness(m: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

/// The 2 px outline strokes painted in `colour` (the card outline is a 2 px outside stroke).
fn outlines(h: &Harness<'static, AppModel>, colour: Color32) -> usize {
    h.output()
        .shapes
        .iter()
        .filter(|c| {
            matches!(&c.shape, egui::Shape::Rect(r)
                if r.stroke.width == 2.0 && r.stroke.color == colour)
        })
        .count()
}

#[test]
fn a_normal_card_has_no_rnm_section_or_outline() {
    let d = tempfile::tempdir().unwrap();
    // An old project: the normal has a layer file the engine still reads (until the engine drops RNM).
    let mut m = fixture(
        d.path(),
        &[
            "iron.png",
            "iron_n.png",
            "iron_detailnorm2.png",
            "iron_g.png",
            "iron_m.png",
            "iron_p.png",
            "iron_s.png",
        ],
    );
    assert!(
        m.item(N).unwrap().job.as_ref().unwrap().rnm.is_some(),
        "the fixture's normal has a layer"
    );
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with("m/iron"))
        .unwrap()
        .key
        .clone();
    let accent = egui::Visuals::dark().selection.stroke.color;

    // The set editor: the normal card.
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Normal _n").hover();
    h.run();
    let card_drew = h.query_all_by_label_contains("iron_n").count();
    let card_hover_rnm = h.query_all_by_label_contains("RNM").count();
    let card_accent = outlines(&h, accent);
    // Control: the unused slots of this set are outlined by the same painter call, so the detector sees it.
    let amber = outlines(&h, AMBER);

    // The texture settings panel of the normal.
    let mut m = h.into_state();
    m.select_item(N);
    let mut h = harness(m);
    h.run();
    let panel_drew = h.query_all_by_label_contains("Normal").count();
    let panel_rnm = h.query_all_by_label("RNM").count();
    let panel_blends = h.query_all_by_label("Blends into").count();

    assert!(card_drew > 0, "the normal card drew");
    assert!(
        amber > 0,
        "the outline detector sees the outline code's strokes"
    );
    assert!(panel_drew > 0, "the settings panel drew");
    assert_eq!(card_hover_rnm, 0, "an RNM hover on the card");
    assert_eq!(card_accent, 0, "the normal card has an accent outline");
    assert_eq!(panel_rnm, 0, "an RNM section");
    assert_eq!(panel_blends, 0, "a Blends into row");
}

const AMBER: Color32 = Color32::from_rgb(230, 160, 30);
