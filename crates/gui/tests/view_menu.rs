//! Note GK: the 3D View menu: the background colour popup keeps the menu open, and each light slider's label sits on
//! its slider's row.
use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::viewport3d::{V3State, Viewport3d};

const SLIDERS: [&str; 3] = [
    "Sun brightness",
    "Point light brightness",
    "Point light radius (x model size)",
];

fn menu<'a>(width: f32) -> Harness<'a, Viewport3d> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(width, 700.0))
        .build_ui_state(
            |ui, v: &mut Viewport3d| {
                sdc_gui::ui::viewport3d::show(ui, v);
            },
            Viewport3d::with_state(V3State::Ready),
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h.get_by_label(sdc_gui::ui::viewport3d::VIEW_MENU).click();
    h.run();
    h
}

fn bounds(n: &egui_kittest::Node<'_>) -> egui::accesskit::Rect {
    n.accesskit_node().bounding_box().expect("bounds")
}

fn menu_open(h: &Harness<'_, Viewport3d>) -> bool {
    h.query_by_role_and_label(Role::Slider, SLIDERS[0])
        .is_some()
}

/// Owner 2026-11-02: in a narrow 3D panel the row wrapped between a slider and its trailing label, so "Sun brightness"
/// sat beside the point-light slider and "Point light brightness" beside the radius one. Unwrapped, the one row ran
/// off a narrow window.
#[test]
fn each_light_label_sits_on_its_sliders_row_in_a_narrow_panel() {
    for width in [360.0, 480.0, 640.0, 1000.0] {
        let h = menu(width);
        for name in SLIDERS {
            let s = bounds(&h.get_by_role_and_label(Role::Slider, name));
            let l = h
                .query_all_by_role(Role::Label)
                .find(|n| n.accesskit_node().value().as_deref() == Some(name))
                .unwrap_or_else(|| panic!("label {name}"));
            let l = bounds(&l);
            assert!(
                l.x1 <= f64::from(width),
                "width {width}: label {name:?} ends off screen at x {}",
                l.x1
            );
            assert!(
                l.y0 < s.y1 && s.y0 < l.y1,
                "width {width}: label {name:?} at y {}..{} is not on its slider's row {}..{}",
                l.y0,
                l.y1,
                s.y0,
                s.y1
            );
        }
    }
}

/// Owner 2026-11-02: clicking the background swatch closed the View menu (egui 0.36 keeps one popup open, and the
/// colour popup took that slot from the menu).
#[test]
fn the_background_swatch_opens_its_picker_and_keeps_the_menu() {
    let mut h = menu(1000.0);
    let before = h.query_all_by_role(Role::SpinButton).count();
    h.get_by_role(Role::ColorWell).click();
    h.run();
    assert!(menu_open(&h), "the View menu stays open");
    let spins: Vec<_> = h.query_all_by_role(Role::SpinButton).collect();
    assert!(
        spins.len() > before,
        "the colour picker opened ({} spin buttons, {before} before)",
        spins.len()
    );
    // A click inside the picker keeps both; a click outside closes the picker first, then the menu.
    let inside = bounds(spins.last().unwrap());
    h.get_by_role(Role::ColorWell); // still drawn
    let p = egui::pos2(inside.x0 as f32 + 2.0, inside.y0 as f32 + 2.0);
    h.event(egui::Event::PointerMoved(p));
    h.event(egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.event(egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert!(menu_open(&h), "a click inside the picker keeps the menu");
    assert!(
        h.query_all_by_role(Role::SpinButton).count() > before,
        "and the picker"
    );
    let outside = egui::pos2(990.0, 690.0);
    for _ in 0..2 {
        h.event(egui::Event::PointerMoved(outside));
        h.event(egui::Event::PointerButton {
            pos: outside,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos: outside,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        h.run();
    }
    assert!(
        !menu_open(&h),
        "two clicks outside close the picker and the menu"
    );
}

#[test]
fn the_view_menu_still_closes_on_esc_and_on_its_button() {
    let mut h = menu(1000.0);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!menu_open(&h), "Esc closes it");
    h.get_by_label(sdc_gui::ui::viewport3d::VIEW_MENU).click();
    h.run();
    assert!(menu_open(&h), "the button reopens it");
    h.get_by_label(sdc_gui::ui::viewport3d::VIEW_MENU).click();
    h.run();
    assert!(!menu_open(&h), "and closes it");
}
