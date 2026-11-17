//! AG125: a popup that holds a filter box or a list stays open while it is used, and closes on a click outside, Esc
//! or a pick.
mod support;
use eframe::egui;
use egui_kittest::kittest::{by, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::pbr_export::json_entries;
use sdc_gui::model::AppModel;
use sdc_gui::ui::State;
use support::*;

type H = Harness<'static, (AppModel, State)>;

const HEADING: &str = "Read this set's textures as";

fn harness() -> (tempfile::TempDir, H) {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let key = json_entries(&m)
        .into_iter()
        .find(|(v, _)| v.id == "landscape/rock01")
        .unwrap()
        .0
        .key;
    m.select_set(&key);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 900.0))
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, State::default()),
        );
    h.run();
    // Note FC12: the selector starts at the set's lowest unused match; the slot-1 list is the one under test.
    h.get_by_value("Match 2").click();
    h.run();
    h.get_by_label("Match 1 · Diffuse").click();
    h.run();
    h.get_by_label(sdc_gui::ui::input_name::DROPDOWN).click();
    h.run();
    h.get_by_label(HEADING);
    (d, h)
}

fn filter_box(h: &H) -> egui::Rect {
    h.get(by().predicate(|n| n.placeholder() == Some("Filter: words of the path")))
        .rect()
}

fn filter_box_or_value(h: &H) -> egui::Pos2 {
    h.get(by().role(egui::accesskit::Role::TextInput).value("rock"))
        .rect()
        .center()
}

fn click_at(h: &mut H, p: egui::Pos2) {
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run();
}

#[test]
fn typing_in_the_input_name_filter_keeps_the_popup_open() {
    let (_d, mut h) = harness();
    let r = filter_box(&h);
    click_at(&mut h, r.center());
    h.get_by_label(HEADING);
    h.input_mut().events.push(egui::Event::Text("rock".into()));
    h.run();
    h.get_by_label(HEADING);
    // The typed text reached the box, which now lists only the matches.
    h.get(by().role(egui::accesskit::Role::TextInput).value("rock"));
    // A click on the popup's own text, not a control, keeps it too.
    let heading = h.get_by_label(HEADING).rect().center();
    click_at(&mut h, heading);
    h.get_by_label(HEADING);
    // Esc with the box focused closes it as well.
    let r = filter_box_or_value(&h);
    click_at(&mut h, r);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.query_by_label(HEADING).is_none(), "Esc in the box");
}

#[test]
fn a_click_outside_or_esc_closes_the_input_name_popup() {
    let (_d, mut h) = harness();
    // A spot on the background (not on a slot card, whose click opens its own window).
    let outside = egui::pos2(1500.0, 300.0);
    assert_eq!(
        h.ctx.layer_id_at(outside).map(|l| l.order),
        Some(egui::Order::Background)
    );
    click_at(&mut h, outside);
    assert!(h.query_by_label(HEADING).is_none(), "click outside");
    h.get_by_label(sdc_gui::ui::input_name::DROPDOWN).click();
    h.run();
    h.get_by_label(HEADING);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h.query_by_label(HEADING).is_none(), "Esc");
}

/// Accepted for the owner to overrule (spec 7a, Plan 1 round 1, ruling 6): egui 0.37 moves between a menu's rows with
/// the arrow keys (`menu.rs:232-288`). The input-name dropdown is a real menu (`path_picker::menu`), and with its
/// filter box focused every arrow key stays in the box: no key the filter needs is taken, and Down does not move to a
/// row. After the keys, a click on a row still picks: the set's own name is refused with its reason and the menu stays
/// open (T35), and a free path is written as the input name and closes the menu.
#[test]
fn arrow_keys_and_typing_stay_in_the_input_name_filter() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::NodeT;
    let (_d, mut h) = harness();
    let r = filter_box(&h);
    click_at(&mut h, r.center());
    let typed = |h: &mut H, text: &str| {
        h.input_mut().events.push(egui::Event::Text(text.into()));
        h.run();
    };
    let key = |h: &mut H, k: egui::Key| {
        h.key_press(k);
        h.run();
    };
    let rock_focused = |h: &H, step: &str| {
        let b = h.get(by().role(Role::TextInput).value("rock"));
        assert!(
            b.accesskit_node().is_focused(),
            "{step}: the box keeps the focus"
        );
    };
    typed(&mut h, "rck");
    key(&mut h, egui::Key::ArrowLeft);
    key(&mut h, egui::Key::ArrowLeft);
    typed(&mut h, "o");
    rock_focused(&h, "Left, Left, o");
    key(&mut h, egui::Key::Home);
    key(&mut h, egui::Key::End);
    key(&mut h, egui::Key::ArrowRight);
    key(&mut h, egui::Key::Backspace);
    typed(&mut h, "k");
    rock_focused(&h, "Home, End, Right, Backspace, k");
    for k in [
        egui::Key::ArrowDown,
        egui::Key::ArrowDown,
        egui::Key::ArrowUp,
    ] {
        key(&mut h, k);
    }
    rock_focused(&h, "Down, Down, Up");
    h.get_by_label(HEADING);
    // The one listed path is the set's own name. A click on it reaches the row: the pick is refused (T35, a pick of
    // the set's own name) and the menu stays open to say why, with the filter cleared by the pick.
    const PATH: &str = "textures/landscape/rock01";
    h.get_by_label("1 of 3 paths");
    h.query_all_by_role(Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some(PATH))
        .expect("the listed path")
        .click();
    h.run();
    h.get_by_label("the set already has this name");
    h.get_by_label("3 of 3 paths");
    h.get_by_label(HEADING);
    let name = h.get_by_role_and_label(Role::TextInput, "Input name");
    assert_eq!(name.accesskit_node().value().as_deref(), Some(PATH));
    // The fixture's other two paths name other sets, so they are refused too: a scan adds one no set has. A pick of it
    // writes the input name and closes the menu.
    const FREE: &str = "textures/landscape/rock02";
    h.state_mut()
        .0
        .set_known(sdc_gui::model::known_paths::KnownPaths {
            textures: [(format!("{FREE}.dds"), 1u16)].into_iter().collect(),
            mesh_dirs: vec![],
            ..Default::default()
        });
    h.run();
    h.query_all_by_role(Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some(FREE))
        .expect("the scanned path is listed")
        .click();
    h.run();
    assert!(
        h.query_by_label(HEADING).is_none(),
        "the pick closes the menu"
    );
    // The pick renames the set, whose id follows its new input name (as `input_name_edit` reads it).
    assert!(h
        .state_mut()
        .0
        .wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert_eq!(
        h.state().0.input_name_of("landscape/rock02").as_deref(),
        Some(FREE)
    );
}
