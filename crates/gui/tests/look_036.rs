//! Note J?2: egui 0.36.2's geometry where the doc shots showed a widget moving on egui 0.37. The numbers come from a
//! probe that drew the same ui on egui 0.36.2 and 0.37.0 with the app's font and pinned spacing.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use sdc_gui::look::Combo036 as _;
use sdc_gui::model::AppModel;
use support::*;

fn bare(size: egui::Vec2, ui_fn: impl FnMut(&mut egui::Ui) + 'static) -> Harness<'static> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(size)
        .build_ui(ui_fn)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

/// A combo shown beside a 19 pt button sits half a point lower and the row is 19.5 tall, as 0.36.2 drew it: its own
/// `ui.horizontal` (18 pt high, centred) held the 19 pt combo (`egui-0.36.2 containers/combo_box.rs:234`).
#[test]
fn a_combo_beside_a_button_sits_as_on_036() {
    let mut h = bare(egui::vec2(400.0, 200.0), |ui| {
        ui.horizontal(|ui| {
            let _ = ui.button("Go to set");
            egui::ComboBox::from_id_salt("look-036-row")
                .selected_text("Cutout")
                .width(96.0)
                .show_ui_036(ui, |_| {})
                .response
                .accessible_name("Alpha");
        });
        ui.label("below");
    });
    h.run();
    let button = h.get_by_role_and_label(Role::Button, "Go to set").rect();
    let combo = h.get_by_role_and_label(Role::ComboBox, "Alpha").rect();
    let below = h.get_by_label("below").rect();
    assert!(
        near(combo.min.y - button.min.y, 0.5) && near(combo.height(), 19.0),
        "combo {combo:?} beside button {button:?}"
    );
    assert!(
        near(below.min.y - button.min.y, 22.5),
        "the next row starts 19.5 + 3 below: {below:?} vs {button:?}"
    );
}

/// A combo whose value is wider than its width, in a Grid cell's vertical layout: one line, as wide as its text, as
/// on 0.36.2 (its own horizontal row made the text `Extend`, `egui-0.36.2 ui.rs:589`). 0.37 wrapped it to 3 lines.
#[test]
fn a_long_combo_value_in_a_grid_cell_stays_on_one_line_as_on_036() {
    let mut h = bare(egui::vec2(600.0, 200.0), |ui| {
        egui::Grid::new("look-036-grid")
            .num_columns(3)
            .min_col_width(0.0)
            .show(ui, |ui| {
                ui.label("cobble.png");
                ui.vertical(|ui| {
                    egui::ComboBox::from_id_salt("look-036-grid-combo")
                        .selected_text("BC4 · linear (one channel)")
                        .width(96.0)
                        .show_ui_036(ui, |_| {})
                        .response
                        .accessible_name("Format");
                });
                ui.label("L");
                ui.end_row();
            });
    });
    h.run();
    let combo = h.get_by_role_and_label(Role::ComboBox, "Format").rect();
    assert!(
        near(combo.height(), 19.0) && near(combo.width(), 171.4),
        "{combo:?}"
    );
}

fn texts(s: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
    match s {
        egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
        egui::Shape::Text(t) => out.push((
            t.galley.text().to_owned(),
            egui::Rect::from_min_size(t.pos, t.galley.size()),
        )),
        _ => {}
    }
}

/// S5 of the egui 0.37 doc shots: a submenu entry ends in 0.36.2's "⏵" glyph (13 pt wide, on the label's text row,
/// `egui-0.36.2 containers/menu.rs:347`), not 0.37's 14 pt painted triangle, which made the set card's menu 1 px
/// wider and the arrow 2 px higher. Its accessible name stays the entry's text (J?4).
#[test]
fn a_submenu_entry_ends_in_036s_arrow_glyph() {
    let mut h = bare(egui::vec2(400.0, 300.0), |ui| {
        let _ = egui::menu::MenuButton::new("Set").ui(ui, |ui| {
            let _ = sdc_gui::look::menu_button_036(ui, "Move to group", |ui| {
                let _ = ui.button("Group A");
            });
            let _ = ui.button("Delete set");
        });
    });
    h.get_by_role_and_label(Role::Button, "Set").click();
    h.run();
    let entry = h
        .get_by_role_and_label(Role::Button, "Move to group")
        .rect();
    let mut shown = vec![];
    h.output()
        .shapes
        .iter()
        .for_each(|c| texts(&c.shape, &mut shown));
    let arrow = shown
        .iter()
        .find(|(t, r)| t == "⏵" && entry.contains_rect(*r))
        .unwrap_or_else(|| panic!("no ⏵ glyph in the entry {entry:?}: {shown:?}"));
    let label = shown.iter().find(|(t, _)| t == "Move to group").unwrap().1;
    assert!(
        near(arrow.1.min.y, label.min.y)
            && near(arrow.1.width(), 13.0)
            && near(entry.max.x - arrow.1.max.x, label.min.x - entry.min.x),
        "glyph {:?} beside label {label:?} in entry {entry:?}",
        arrow.1
    );
}

/// S1 of the egui 0.37 doc shots: the PBR set's Height _p format combo ("BC4 · linear (one channel)") is one line and
/// wider than `COMBO_W`, so the Format column keeps its 0.36.2 width and place.
#[test]
fn the_set_editors_long_format_combo_is_one_line() {
    // The doc shots' PBR cobble set: diffuse, normal, height and a ready `_rmaos` (no fill rows in the Format column).
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/architecture/stone");
    std::fs::create_dir_all(&t).unwrap();
    let rgb = |n: &str, c: [u8; 3]| {
        image::ImageBuffer::from_fn(64, 64, |x, _| {
            image::Rgb([c[0], c[1], c[2].wrapping_add(x as u8)])
        })
        .save(t.join(n))
        .unwrap()
    };
    rgb("cobble.png", [90, 88, 84]);
    rgb("cobble_n.png", [128, 128, 255]);
    rgb("cobble_rmaos.png", [200, 30, 255]);
    image::ImageBuffer::from_fn(64, 64, |x, _| image::Luma([x as u8 * 4]))
        .save(t.join("cobble_p.png"))
        .unwrap();
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..Default::default()
        },
        None,
        session(),
    );
    m.open_folder(d.path().join("textures"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.set_shader(
        "architecture/stone/cobble",
        Some(engine::shader::Shader::Pbr),
    );
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with("architecture/stone/cobble"))
        .unwrap()
        .key
        .clone();
    m.select_set(&key);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 900.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    h.run();
    let values: Vec<(String, egui::Rect)> = h
        .query_all_by_role(Role::ComboBox)
        .filter_map(|n| n.value().map(|v| (v, n.rect())))
        .collect();
    let (_, bc4) = values
        .iter()
        .find(|(v, _)| v.starts_with("BC4"))
        .unwrap_or_else(|| panic!("no BC4 format combo: {values:?}"));
    assert!(
        // 96: `set_editor::COMBO_W`, the combo's minimum width.
        near(bc4.height(), 19.0) && bc4.width() > 96.0,
        "{bc4:?}"
    );
}
