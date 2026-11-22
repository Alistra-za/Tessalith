//! Texture type words: the table's labels and descriptions, and where the card and the pills show them.
mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use image::{ImageBuffer, Rgb};
use sdc_gui::model::AppModel;
use std::path::Path;
use std::time::Duration;
use support::*;

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        )
}

fn key_of(m: &AppModel, name: &str) -> String {
    m.items
        .iter()
        .find(|i| i.key.ends_with(name))
        .expect("an item with that name")
        .key
        .clone()
}

/// `rock_bump.png` alone waits for a type (copied from `unknown_suffix.rs`).
fn waiting_folder(dir: &Path) -> AppModel {
    let c = dir.join("textures/clutter");
    std::fs::create_dir_all(&c).unwrap();
    let grad = |x: u32, y: u32| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]);
    ImageBuffer::from_fn(64, 64, grad)
        .save(c.join("rock_bump.png"))
        .unwrap();
    opened_as_the_app(dir.join("textures"), dir.join("export"))
}

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "model did not settle");
}

fn entry_below<'a, S>(
    h: &'a Harness<'_, S>,
    label: &'a str,
    combo: eframe::egui::Rect,
) -> egui_kittest::Node<'a> {
    h.query_all_by_label(label)
        .find(|n| n.rect().min.y > combo.max.y)
        .unwrap_or_else(|| panic!("{label} in the open combo"))
}
#[test]
fn every_type_has_a_label_a_description_and_a_suffix() {
    for st in engine::config::Subtype::ALL {
        // config.rs:360, [Subtype; 34]
        let i = sdc_gui::model::labels::type_info(st);
        assert!(
            !i.label.is_empty() && !i.description.is_empty() && !i.suffix.is_empty(),
            "{st:?}"
        );
        assert!(
            !i.label.contains('_'),
            "{st:?} label has an underscore: {}",
            i.label
        );
        assert!(i.description.ends_with('.'), "{st:?}");
        assert!(
            !i.description.contains('`') && !i.suffix.contains('`'),
            "{st:?} ships Markdown backticks"
        ); // round 3
    }
}

#[test]
fn the_table_reads_like_the_wiki() {
    use engine::config::Subtype::*;
    use sdc_gui::model::labels::{subtype_name, type_info};
    assert_eq!(subtype_name(SkinSk), "Skin subsurface mask");
    assert_eq!(subtype_name(Msn), "Model-space normal");
    assert_eq!(subtype_name(Envmask), "Environment mask");
    assert_eq!(subtype_name(GenericTwoChannel), "Two-channel data");
    assert_eq!(subtype_name(Rmaos), "PBR RMAOS");
    assert_eq!(type_info(Rmaos).suffix, "_rmaos");
    assert_eq!(
        type_info(Normal).description,
        "The surface bumps of an ordinary material (tangent space)."
    );
    assert_eq!(
        sdc_gui::model::labels::type_hover(Normal),
        "The surface bumps of an ordinary material (tangent space).\nFile name: _n\nA file alone under its base name becomes a set of its own once it has a type."
    );
    let gray = sdc_gui::model::labels::type_hover(Gray);
    assert!(
        gray.contains("No file ending gives this type (picked by hand)")
            && !gray.contains("File name: none ("),
        "{gray}"
    );
}

#[test]
fn the_slot_labels_read_like_table_a() {
    use sdc_gui::model::labels::slot_type_label as l;
    use sdc_gui::model::sets::SlotKind::*;
    assert_eq!(l(Normal, false, false, false), "Normal map");
    assert_eq!(l(Normal, true, false, false), "PBR normal");
    assert_eq!(l(Diffuse, false, true, false), "Skin albedo");
    assert_eq!(l(Normal, false, true, false), "PBR normal");
    assert_eq!(l(Emissive, false, false, false), "Glow");
    assert_eq!(
        l(Subsurface, true, false, false),
        "PBR subsurface or coat colour"
    );
    assert_eq!(
        l(Rmaos, true, false, true),
        "Skin RFAOS (roughness, fuzz, AO, specular)"
    );
    assert_eq!(l(Height, true, false, false), "Height (parallax)");
}

#[test]
fn the_undecided_card_hovers_the_description_not_the_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = waiting_folder(dir.path());
    idle(&mut m);
    m.select_item(&key_of(&m, "rock_bump.png"));
    let mut h = harness(m);
    instant_tips(&h.ctx);
    h.run();
    h.run();
    assert!(h.query_by_label("What kind of texture is this?").is_some());
    // The combo announces its current value: the label of the type it starts on.
    let shown = h.get_by_label("Type for this texture").value();
    assert_eq!(
        shown.as_deref(),
        Some(sdc_gui::model::labels::subtype_name(engine::config::Subtype::GenericData).as_str())
    );
    let combo = h.get_by_label("Type for this texture").rect();
    h.get_by_label("Type for this texture").click();
    h.run();
    // The popup starts clipped: scroll the entry into view, then re-query it.
    entry_below(&h, "Environment mask", combo).scroll_to_me();
    h.run();
    entry_below(&h, "Environment mask", combo).hover();
    h.run();
    assert!(h
        .query_by_label_contains("A grey mask that sets where the cubemap shines.")
        .is_some());
    assert!(h.query_by_label("envmask").is_none());
}
