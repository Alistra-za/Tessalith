//! Note HF6: groups in the Sets column. A header per group (chevron, name, count, worst status), members indented,
//! collapse kept in `gui.toml`, a filter that only hides, a reveal that opens the group, a set dropped on a header,
//! and the menus. Everything is found by the AccessKit name the header and the cards carry.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use image::{ImageBuffer, Rgb};
use sdc_gui::model::set_groups::{GroupTarget, SetGroup};
use sdc_gui::model::sets::set_views;
use sdc_gui::model::AppModel;
use std::path::Path;
use std::time::Duration;
use support::*;

type H = Harness<'static, AppModel>;

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "model did not settle");
}

fn harness_at(m: AppModel, w: f32, h: f32) -> H {
    let h = Harness::builder()
        .with_size(egui::vec2(w, h))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

fn harness(m: AppModel) -> H {
    harness_at(m, 1500.0, 900.0)
}

fn colour(g: &Path, file: &str) {
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(g.join(file))
        .unwrap();
}

fn flat(g: &Path, file: &str) {
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(g.join(file))
        .unwrap();
}

/// `textures/g`: a ready set `g/<n>` for each of `ready` (a colour map and a flat `_n` normal), a warning set for
/// each of `warn` (an alpha that needs a decision), and a blocked one for each of `blocked` (`_cnr` and `_f` both
/// want slot 7).
fn fixture(dir: &Path, ready: &[&str], warn: &[&str], blocked: &[&str]) -> AppModel {
    let g = dir.join("textures/g");
    std::fs::create_dir_all(&g).unwrap();
    for n in ready {
        colour(&g, &format!("{n}.png"));
        flat(&g, &format!("{n}_n.png"));
    }
    for n in warn {
        ImageBuffer::from_fn(64, 64, |x, y| {
            image::Rgba([
                (x * 4) as u8,
                (y * 4) as u8,
                90u8,
                if x < 32 { 255 } else { 0 },
            ])
        })
        .save(g.join(format!("{n}.png")))
        .unwrap();
        flat(&g, &format!("{n}_n.png"));
    }
    for n in blocked {
        colour(&g, &format!("{n}.png"));
        flat(&g, &format!("{n}_cnr.png"));
        flat(&g, &format!("{n}_f.png"));
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    idle(&mut m);
    m
}

fn sets_named(dir: &Path, names: &[&str]) -> AppModel {
    fixture(dir, names, &[], &[])
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn group(m: &mut AppModel, name: &str, sets: &[&str]) {
    m.move_to_group(&s(sets), GroupTarget::New(name.into()))
        .expect("grouped");
}

fn stored(m: &AppModel) -> Vec<SetGroup> {
    m.sets_doc.as_ref().unwrap().groups()
}

/// The text of `sdc-project.toml`.
fn doc(m: &AppModel) -> String {
    m.sets_doc.as_ref().unwrap().text()
}

/// A node of the Sets column whose AccessKit name contains `part`.
fn node<'a>(h: &'a H, part: &str) -> egui_kittest::Node<'a> {
    let part: &'a str = Box::leak(part.to_string().into_boxed_str());
    let mut n: Vec<_> = h
        .query_all_by_label_contains(part)
        .filter(|n| in_sets_column(h, n.rect()))
        .collect();
    assert_eq!(n.len(), 1, "one {part:?} in the Sets column");
    n.pop().unwrap()
}

fn maybe<'a>(h: &'a H, part: &str) -> Option<egui_kittest::Node<'a>> {
    let part: &'a str = Box::leak(part.to_string().into_boxed_str());
    h.query_all_by_label_contains(part)
        .find(|n| in_sets_column(h, n.rect()))
}

/// The header of group `name`, if it is drawn.
fn header_opt<'a>(h: &'a H, name: &str) -> Option<egui_kittest::Node<'a>> {
    maybe(h, &format!("Group {name},"))
}

fn header<'a>(h: &'a H, name: &str) -> egui_kittest::Node<'a> {
    node(h, &format!("Group {name},"))
}

fn header_label(h: &H, name: &str) -> String {
    header(h, name).accesskit_node().label().unwrap()
}

/// The card of set `id`, if it is drawn.
fn card<'a>(h: &'a H, id: &str) -> Option<egui_kittest::Node<'a>> {
    maybe(h, &format!("Set {id}, position"))
}

fn card_rect(h: &H, id: &str) -> egui::Rect {
    card(h, id)
        .unwrap_or_else(|| panic!("the card of {id}"))
        .rect()
}

fn sets_panel(h: &H) -> egui::Rect {
    use egui::containers::panel::PanelState;
    PanelState::load(&h.ctx, egui::Id::new("sets"))
        .expect("the Sets panel")
        .outer_rect
}

/// Click a header (the row toggles).
fn click_header(h: &mut H, name: &str) {
    header(h, name).click();
    h.run();
}

/// Right-click set `id`'s card on its name (the middle of a card can be a chip or the shader box).
fn right_click_card(h: &mut H, id: &str) {
    let name = h
        .query_all_by_label(id)
        .find(|n| in_sets_column(h, n.rect()))
        .unwrap_or_else(|| panic!("{id}'s name"))
        .rect();
    h.hover_at(name.center());
    h.run();
    h.query_all_by_label(id)
        .find(|n| in_sets_column(h, n.rect()))
        .unwrap()
        .click_secondary();
    h.run();
}

/// Right-click a header and click `entry` in the menu.
fn menu_pick(h: &mut H, name: &str, entry: &str) {
    header(h, name).click_secondary();
    h.run();
    h.get_by_label(entry).click();
    h.run();
}

fn instant_tooltips(h: &H) {
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
}

fn key_of(m: &AppModel, id: &str) -> String {
    set_views(m)
        .iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("set {id}"))
        .key
        .clone()
}

fn press(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

/// Press the ≡ handle of set `id`'s card, carry it over `to` and let go there.
fn drag_set_to(h: &mut H, id: &str, to: egui::Pos2) {
    let card = card_rect(h, id);
    let at = h
        .query_all_by_label("≡")
        .map(|n| n.rect())
        .find(|r| in_sets_column(h, *r) && card.contains(r.center()))
        .expect("the card's handle")
        .center();
    h.hover_at(at);
    h.step();
    h.event(press(at, true));
    h.step();
    h.event(egui::Event::PointerMoved(at + egui::vec2(0.0, 10.0)));
    h.step();
    assert!(
        egui::DragAndDrop::has_payload_of_type::<sdc_gui::ui::sets_column::DragSet>(&h.ctx),
        "a set is in flight"
    );
    for k in 1..=4 {
        let p = at.lerp(to, k as f32 / 4.0);
        h.event(egui::Event::PointerMoved(p));
        h.step();
    }
    h.step();
    h.event(press(to, false));
    h.run_steps(3);
}

/// The dialog's name field.
fn name_field<'a>(h: &'a H) -> egui_kittest::Node<'a> {
    h.get_by_role_and_label(Role::TextInput, "Group name")
}

/// Replace the dialog field's text with `text`.
fn type_name(h: &mut H, text: &str) {
    name_field(h).focus();
    h.run();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run();
    name_field(h).type_text(text);
    h.run();
}

// ---- The header ----

#[test]
fn a_group_header_shows_name_count_and_worst_status() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path(), &["a", "b", "c", "d", "e", "f"], &["g"], &[]);
    group(
        &mut m,
        "Armor",
        &["g/a", "g/b", "g/c", "g/d", "g/e", "g/f", "g/g"],
    );
    m.set_group_collapsed("Armor", true);
    let mut h = harness(m);
    h.run();
    assert_eq!(
        header_label(&h, "Armor"),
        "Group Armor, 7 sets, warning, collapsed"
    );
    // The row shows the name and the count, inside the header.
    let rect = header(&h, "Armor").rect();
    for text in ["Armor", "7"] {
        let r = h
            .query_all_by_label(text)
            .map(|n| n.rect())
            .find(|r| rect.contains_rect(*r))
            .unwrap_or_else(|| panic!("{text:?} inside the header {rect:?}"));
        assert!(r.width() > 0.0);
    }
    // Opened, it says so, and the members are below it.
    click_header(&mut h, "Armor");
    assert_eq!(
        header_label(&h, "Armor"),
        "Group Armor, 7 sets, warning, expanded"
    );
    assert!(card_rect(&h, "g/a").top() > rect.bottom());
}

#[test]
fn the_status_dot_takes_the_worst_member() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path(), &["a", "b", "c", "d"], &["w1", "w2"], &["h"]);
    group(&mut m, "Fine", &["g/a", "g/b"]);
    group(&mut m, "Mixed", &["g/c", "g/w1"]);
    group(&mut m, "Bad", &["g/w2", "g/h"]);
    let mut h = harness(m);
    h.run();
    for (name, status) in [
        ("Fine", "ready"),
        ("Mixed", "warning"),
        ("Bad", "blocks export"),
    ] {
        let label = header_label(&h, name);
        assert!(
            label.contains(&format!(" sets, {status}, ")),
            "{name}: {label}"
        );
    }
}

#[test]
fn members_are_indented_by_12() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c", "d"]);
    group(&mut m, "Armor", &["g/a", "g/b"]);
    let mut h = harness(m);
    h.run();
    let (member, plain) = (card_rect(&h, "g/a"), card_rect(&h, "g/c"));
    assert!(
        ((member.min.x - plain.min.x) - 12.0).abs() < 0.5,
        "member at {}, plain card at {}",
        member.min.x,
        plain.min.x
    );
    assert!(
        (member.max.x - plain.max.x).abs() < 0.5,
        "the right edges line up"
    );
    assert!(
        (header(&h, "Armor").rect().min.x - plain.min.x).abs() < 0.5,
        "the header sits at the plain cards' left"
    );
}

// ---- Collapse ----

#[test]
fn collapse_hides_the_members_and_survives_a_reopen() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c", "d"]);
    let gui_toml = d.path().join("settings/gui.toml");
    m.settings_path = Some(gui_toml.clone());
    group(&mut m, "Armor", &["g/a", "g/b"]);
    let (steps, text) = (m.history.undo.len(), doc(&m));
    let mut h = harness(m);
    h.run();
    assert!(card(&h, "g/a").is_some() && card(&h, "g/b").is_some());
    click_header(&mut h, "Armor");
    assert!(
        card(&h, "g/a").is_none() && card(&h, "g/b").is_none(),
        "hidden"
    );
    assert!(card(&h, "g/c").is_some(), "the others stay");
    assert!(header_label(&h, "Armor").ends_with("collapsed"));
    // Not an undo step, and the project file is not touched.
    assert_eq!(
        (h.state().history.undo.len(), doc(h.state())),
        (steps, text)
    );
    assert!(h.state().group_collapsed("Armor"));
    h.state_mut().save().expect("saved");
    // The next session: a new model on the same folder, with the settings read back from `gui.toml`.
    let (settings, err) = sdc_gui::settings::GuiSettings::load(&gui_toml);
    assert!(err.is_none(), "{err:?}");
    let mut m2 = AppModel::new(settings, None, session());
    m2.open_folder(d.path().join("textures"));
    idle(&mut m2);
    let mut h2 = harness(m2);
    h2.run();
    assert!(header_label(&h2, "Armor").ends_with("collapsed"));
    assert!(card(&h2, "g/a").is_none(), "still folded away");
    click_header(&mut h2, "Armor");
    assert!(card(&h2, "g/a").is_some(), "and opens again");
}

#[test]
fn a_filter_reads_2_of_7_and_keeps_collapse() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["ax1", "ax2", "b1", "b2", "b3", "b4", "b5"]);
    let all = ["g/ax1", "g/ax2", "g/b1", "g/b2", "g/b3", "g/b4", "g/b5"];
    group(&mut m, "Armor", &all);
    m.set_group_collapsed("Armor", true);
    let mut h = harness(m);
    h.run();
    assert!(header_label(&h, "Armor").contains(" 7 sets, "));
    h.state_mut().sets_filter.search = "ax".into();
    h.run();
    assert_eq!(
        header_label(&h, "Armor"),
        "Group Armor, 2 of 7 sets, ready, collapsed"
    );
    let rect = header(&h, "Armor").rect();
    assert!(
        h.query_all_by_label("2 of 7")
            .any(|n| rect.contains_rect(n.rect())),
        "the row reads 2 of 7"
    );
    assert!(
        card(&h, "g/ax1").is_none(),
        "still collapsed under a filter"
    );
    // Opened, only the passing members show; clearing the filter reads 7 again and keeps the state.
    click_header(&mut h, "Armor");
    assert!(card(&h, "g/ax1").is_some() && card(&h, "g/ax2").is_some());
    assert!(card(&h, "g/b1").is_none(), "hidden by the filter");
    h.state_mut().sets_filter.search.clear();
    h.run();
    assert_eq!(
        header_label(&h, "Armor"),
        "Group Armor, 7 sets, ready, expanded"
    );
}

#[test]
fn a_filter_change_scrolls_to_the_open_set_but_keeps_its_group_folded() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["ax1", "ax2", "b1", "b2"]);
    group(&mut m, "Armor", &["g/ax1", "g/ax2", "g/b1"]);
    m.set_group_collapsed("Armor", true);
    let k = key_of(&m, "g/ax1");
    m.select_set(&k);
    let mut h = harness(m);
    h.run();
    assert!(
        card(&h, "g/ax1").is_none(),
        "folded, though its set is open"
    );
    // The open set still passes: the filter reveals it for the scroll, and the fold stays as it is.
    h.state_mut().sets_filter.search = "ax".into();
    for _ in 0..4 {
        h.run();
    }
    assert!(h.state().group_collapsed("Armor"), "still folded");
    assert!(card(&h, "g/ax1").is_none());
    assert_eq!(
        header_label(&h, "Armor"),
        "Group Armor, 2 of 3 sets, ready, collapsed"
    );
}

#[test]
fn a_header_with_no_passing_member_hides() {
    let d = tempfile::tempdir().unwrap();
    // zz_bump waits for a type: no card (Note GY5).
    std::fs::create_dir_all(d.path().join("textures/g")).unwrap();
    colour(&d.path().join("textures/g"), "zz_bump.png");
    let mut m = sets_named(d.path(), &["ax1", "ax2", "b1"]);
    group(&mut m, "Armor", &["g/ax1", "g/ax2"]);
    group(&mut m, "Ghost", &["g/zz_bump"]);
    idle(&mut m);
    let stored_before = stored(&m);
    let text = doc(&m);
    assert!(text.contains("Ghost") && text.contains("g/zz_bump"));
    let mut h = harness(m);
    h.run();
    assert!(header_opt(&h, "Armor").is_some());
    assert!(
        header_opt(&h, "Ghost").is_none(),
        "its only member has no card"
    );
    h.state_mut().sets_filter.search = "b1".into();
    h.run();
    assert!(header_opt(&h, "Armor").is_none(), "no member passes");
    assert!(card(&h, "g/b1").is_some());
    h.state_mut().sets_filter.search.clear();
    h.run();
    assert!(header_opt(&h, "Armor").is_some(), "back with the filter");
    assert_eq!(stored(h.state()), stored_before, "[[groups]] unchanged");
    assert_eq!(doc(h.state()), text);
}

#[test]
fn a_filter_never_deselects() {
    use sdc_gui::model::Selection;
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c"]);
    group(&mut m, "Armor", &["g/b", "g/c"]);
    let key = key_of(&m, "g/b");
    m.select_set(&key);
    m.set_group_collapsed("Armor", true);
    let mut h = harness(m);
    h.run();
    assert_eq!(
        h.state().selection,
        Selection::Set(key.clone()),
        "collapsed"
    );
    h.state_mut().sets_filter.search = "zzz".into();
    h.run();
    assert!(header_opt(&h, "Armor").is_none());
    assert_eq!(h.state().selection, Selection::Set(key.clone()), "filtered");
    assert_eq!(h.state().selected_sets(), vec![key]);
}

// ---- Revealing ----

#[test]
fn a_click_on_a_texture_expands_its_sets_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c"]);
    group(&mut m, "Armor", &["g/a", "g/b"]);
    m.set_group_collapsed("Armor", true);
    let mut h = harness(m);
    h.run();
    assert!(card(&h, "g/a").is_none());
    h.state_mut().select_item("g/b.png");
    h.run();
    assert!(!h.state().group_collapsed("Armor"), "opened");
    assert!(card(&h, "g/b").is_some());
    // Folded by hand while its texture stays selected: it stays folded (nothing revealed anything).
    click_header(&mut h, "Armor");
    assert!(h.state().group_collapsed("Armor"));
    // A click on another texture of the group opens it again.
    h.state_mut().select_item("g/a.png");
    h.run();
    assert!(!h.state().group_collapsed("Armor"));
}

#[test]
fn problems_go_to_set_expands_its_sets_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path(), &["a"], &[], &["h"]);
    group(&mut m, "Armor", &["g/a", "g/h"]);
    m.set_group_collapsed("Armor", true);
    m.open_problems(None);
    let mut h = harness(m);
    h.run();
    assert!(card(&h, "g/h").is_none());
    let go = h
        .query_all_by_label("Go to set")
        .find(|n| !in_sets_column(&h, n.rect()))
        .expect("a Go to set button in Problems");
    go.click();
    h.run();
    assert!(!h.state().group_collapsed("Armor"), "opened");
    assert!(card(&h, "g/h").is_some());
}

/// Move to… and the palette ask the column to reveal a set through `nav.reveal_set`.
#[test]
fn a_reveal_expands_its_sets_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c"]);
    group(&mut m, "Armor", &["g/a", "g/b"]);
    m.set_group_collapsed("Armor", true);
    let key = key_of(&m, "g/b");
    let mut h = harness(m);
    h.run();
    assert!(card(&h, "g/b").is_none());
    h.state_mut().nav.reveal_set = Some(key);
    h.run();
    assert!(!h.state().group_collapsed("Armor"));
    assert!(card(&h, "g/b").is_some());
}

#[test]
fn the_revealed_set_is_on_screen_with_headers_above_it() {
    let d = tempfile::tempdir().unwrap();
    let names: Vec<String> = (0..36).map(|i| format!("s{i:02}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let mut m = sets_named(d.path(), &refs);
    for (name, from) in [("One", 20), ("Two", 24), ("Three", 28)] {
        let ids: Vec<String> = (from..from + 4).map(|i| format!("g/s{i:02}")).collect();
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        group(&mut m, name, &ids);
        m.set_group_collapsed(name, true);
    }
    let key = key_of(&m, "g/s30");
    let mut h = harness(m);
    h.run();
    assert!(card(&h, "g/s30").is_none());
    h.state_mut().nav.reveal_set = Some(key);
    // One frame lands it at the top of the view, a little below it (Note FM32). The offset sums the heights above
    // the target, the three headers among them: without those it lands a header and a half too far down.
    h.step();
    let top = h.get_by_label("Filter, Sets shader filter").rect().bottom();
    let first = card_rect(&h, "g/s30");
    assert!(
        first.top() - top < 48.0,
        "one frame: the card is at {first:?}, the view starts near {top}"
    );
    // `run` settling is the pin that the re-aim ends.
    h.run();
    assert!(!h.state().group_collapsed("Three"));
    assert!(h.state().group_collapsed("One") && h.state().group_collapsed("Two"));
    let (panel, r) = (sets_panel(&h), card_rect(&h, "g/s30"));
    assert!(
        r.top() >= panel.top() && r.bottom() <= panel.bottom(),
        "the card {r:?} is whole in the column {panel:?}"
    );
}

// ---- Drops ----

#[test]
fn dropping_a_set_on_a_header_joins() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c", "d"]);
    group(&mut m, "Armor", &["g/a", "g/b"]);
    let order = doc(&m);
    let mut h = harness(m);
    h.run();
    assert!(h.state().group_of("g/d").is_none());
    let to = header(&h, "Armor").rect().center();
    drag_set_to(&mut h, "g/d", to);
    idle(h.state_mut());
    h.run();
    assert_eq!(
        h.state().group_of("g/d").map(|g| g.name),
        Some("Armor".into()),
        "joined"
    );
    assert_ne!(doc(h.state()), order);
    assert_eq!(stored(h.state())[0].sets, s(&["g/a", "g/b", "g/d"]));
}

#[test]
fn dropping_a_texture_on_a_header_does_nothing() {
    use sdc_gui::ui::sets_column::DragItem;
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c", "d"]);
    group(&mut m, "Armor", &["g/a", "g/b"]);
    let (text, steps) = (doc(&m), m.history.undo.len());
    let mut h = harness(m);
    h.run();
    let at = header(&h, "Armor").rect().center();
    let from = card_rect(&h, "g/d").center();
    h.hover_at(from);
    h.run();
    h.event(press(from, true));
    h.step();
    for k in 1..=4 {
        egui::DragAndDrop::set_payload(&h.ctx, DragItem("g/d.png".into()));
        h.event(egui::Event::PointerMoved(from.lerp(at, k as f32 / 4.0)));
        h.run_steps(2);
    }
    egui::DragAndDrop::set_payload(&h.ctx, DragItem("g/d.png".into()));
    h.event(press(at, false));
    h.run_steps(3);
    assert_eq!(doc(h.state()), text, "no edit");
    assert_eq!(h.state().history.undo.len(), steps);
    assert!(h.state().group_of("g/d").is_none());
    assert_eq!(
        h.state().set_of_item_probe("g/d.png").map(|v| v.id),
        Some("g/d".into()),
        "the texture stays in its set"
    );
}

// ---- Menus ----

#[test]
fn the_group_menu_has_its_entries() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c"]);
    group(&mut m, "Armor", &["g/a", "g/b"]);
    let mut h = harness(m);
    h.run();
    header(&h, "Armor").click_secondary();
    h.run();
    for entry in [
        "Rename…",
        "Ungroup",
        "Select all sets",
        "Exclude from export",
    ] {
        h.get_by_label(entry);
    }
    assert!(h.query_by_label("Include in export").is_none());
    // Exclude flips the tag and the entry.
    h.get_by_label("Exclude from export").click();
    h.run();
    assert!(stored(h.state())[0].excluded);
    assert!(
        header_label(&h, "Armor").ends_with(", excluded"),
        "{}",
        header_label(&h, "Armor")
    );
    let rect = header(&h, "Armor").rect();
    assert!(
        h.query_all_by_label("Excluded")
            .any(|n| rect.contains_rect(n.rect())),
        "the Excluded tag"
    );
    header(&h, "Armor").click_secondary();
    h.run();
    assert!(h.query_by_label("Exclude from export").is_none());
    h.get_by_label("Include in export").click();
    h.run();
    assert!(!stored(h.state())[0].excluded);
    assert!(h.query_all_by_label("Excluded").next().is_none());
    // Select all sets selects the members.
    menu_pick(&mut h, "Armor", "Select all sets");
    let mut keys = h.state().selected_sets();
    keys.sort();
    let mut want = vec![key_of(h.state(), "g/a"), key_of(h.state(), "g/b")];
    want.sort();
    assert_eq!(keys, want);
    // Ungroup takes the group, not the sets.
    menu_pick(&mut h, "Armor", "Ungroup");
    assert!(header_opt(&h, "Armor").is_none());
    assert!(stored(h.state()).is_empty());
    assert!(card(&h, "g/a").is_some() && card(&h, "g/b").is_some());
}

#[test]
fn rename_asks_for_a_name_and_refuses_a_taken_one() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c"]);
    group(&mut m, "Armor", &["g/a"]);
    group(&mut m, "Weapons", &["g/b"]);
    let mut h = harness(m);
    h.run();
    menu_pick(&mut h, "Armor", "Rename…");
    type_name(&mut h, "weapons");
    h.get_by_label("OK").click();
    h.run();
    h.get_by_label_contains("already exists");
    assert_eq!(
        stored(h.state())[0].name,
        "Armor",
        "refused: nothing changed"
    );
    // A free name goes through.
    type_name(&mut h, "Plate");
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(stored(h.state())[0].name, "Plate");
    assert!(header_opt(&h, "Plate").is_some());
}

#[test]
fn the_set_menu_has_move_to_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c"]);
    group(&mut m, "Armor", &["g/a"]);
    let mut h = harness(m);
    h.run();
    right_click_card(&mut h, "g/c");
    h.run();
    h.get_by_label("Move to group ⏵").click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Armor");
    h.get_by_label("New group…");
    h.get_by_label("No group");
    h.get_by_role_and_label(Role::Button, "Armor").click();
    h.run();
    assert_eq!(
        h.state().group_of("g/c").map(|g| g.name),
        Some("Armor".into())
    );
    // And out again. Its own group is off in the menu (nothing to move).
    right_click_card(&mut h, "g/c");
    h.run();
    h.get_by_label("Move to group ⏵").click();
    h.run();
    assert!(h
        .get_by_role_and_label(Role::Button, "Armor")
        .accesskit_node()
        .is_disabled());
    h.get_by_label("No group").click();
    h.run();
    assert!(h.state().group_of("g/c").is_none());
}

#[test]
fn the_bulk_menu_has_move_to_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c", "d"]);
    group(&mut m, "Armor", &["g/a"]);
    let (kc, kd) = (key_of(&m, "g/c"), key_of(&m, "g/d"));
    m.select_set(&kc);
    m.click_set(&kd, true, false);
    assert_eq!(m.selected_sets().len(), 2);
    let mut h = harness(m);
    h.run();
    right_click_card(&mut h, "g/d");
    h.run();
    h.get_by_label("Move to group ⏵").click();
    h.run();
    h.get_by_label("New group…");
    h.get_by_label("No group");
    h.get_by_role_and_label(Role::Button, "Armor").click();
    h.run();
    for id in ["g/c", "g/d"] {
        assert_eq!(
            h.state().group_of(id).map(|g| g.name),
            Some("Armor".into()),
            "{id}"
        );
    }
}

#[test]
fn new_group_asks_for_a_name_and_refuses_a_taken_one() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b", "c"]);
    group(&mut m, "Armor", &["g/a"]);
    let mut h = harness(m);
    h.run();
    right_click_card(&mut h, "g/c");
    h.run();
    h.get_by_label("Move to group ⏵").click();
    h.run();
    h.get_by_label("New group…").click();
    h.run();
    type_name(&mut h, "ARMOR");
    h.get_by_label("OK").click();
    h.run();
    h.get_by_label_contains("already exists");
    assert!(h.state().group_of("g/c").is_none(), "refused");
    type_name(&mut h, "Weapons");
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(
        h.state().group_of("g/c").map(|g| g.name),
        Some("Weapons".into())
    );
    assert!(header_opt(&h, "Weapons").is_some());
}

#[test]
fn a_long_group_name_truncates_with_hover() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b"]);
    let name = "Armour of the Long Forgotten Northern Kings, second set".repeat(2);
    group(&mut m, &name, &["g/a", "g/b"]);
    let mut h = harness_at(m, 1280.0, 800.0);
    instant_tooltips(&h);
    h.run();
    let (panel, rect) = (sets_panel(&h), header(&h, &name).rect());
    assert!(rect.right() <= panel.right() + 0.5, "{rect:?} in {panel:?}");
    let label = h
        .query_all_by_label(name.as_str())
        .find(|n| in_sets_column(&h, n.rect()) && rect.contains_rect(n.rect()))
        .expect("the name inside the header");
    let count = h
        .query_all_by_label("2")
        .map(|n| n.rect())
        .find(|r| rect.contains_rect(*r))
        .expect("the count inside the header");
    assert!(
        label.rect().right() <= count.left(),
        "the name {:?} stops before the count {count:?}",
        label.rect()
    );
    label.hover();
    h.run();
    h.run();
    assert!(
        h.query_all_by_label(name.as_str()).count() > 1,
        "the hover shows the whole name"
    );
}

#[test]
fn a_read_only_project_disables_with_the_reason() {
    let d = tempfile::tempdir().unwrap();
    let mut m = sets_named(d.path(), &["a", "b"]);
    // A project file that does not parse is read-only (and holds no groups to show).
    let root = m.root.clone().unwrap();
    std::fs::write(root.join("sdc-project.toml"), "[[sets]\nbroken").unwrap();
    m.reload_documents();
    idle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().read_only());
    let mut h = harness(m);
    instant_tooltips(&h);
    h.run();
    right_click_card(&mut h, "g/a");
    h.run();
    let entry = h.get_by_label("Move to group ⏵");
    assert!(entry.accesskit_node().is_disabled(), "disabled");
    let before = h.query_all_by_label_contains("sdc-project.toml").count();
    entry.hover();
    h.run();
    h.run();
    assert!(
        h.query_all_by_label_contains("sdc-project.toml").count() > before,
        "the hover gives the reason"
    );
}

// ---- Screenshots ----

/// Renders the Sets column with groups at 1100, 1280 and 1600 px wide into `SDC_GROUP_SHOTS` (a folder), for the
/// review against the egui-ui-rules. Needs a GPU: `cargo test -p sdc-gui --test it set_groups_ui::render_ -- --ignored`.
#[test]
#[ignore]
fn render_the_groups() {
    let out = std::path::PathBuf::from(
        std::env::var("SDC_GROUP_SHOTS").unwrap_or_else(|_| "group-shots".into()),
    );
    std::fs::create_dir_all(&out).unwrap();
    let build = |w: f32, filter: &str| {
        let d = tempfile::tempdir().unwrap();
        let mut m = fixture(
            d.path(),
            &["ax1", "ax2", "b1", "b2", "b3", "c1", "c2"],
            &["w1", "w2"],
            &["h"],
        );
        group(&mut m, "Armor", &["g/ax1", "g/ax2", "g/w1"]);
        group(&mut m, "Weapons", &["g/b1", "g/b2", "g/h"]);
        group(
            &mut m,
            "Armour of the Long Forgotten Northern Kings",
            &["g/c1", "g/c2"],
        );
        group(&mut m, "Retired", &["g/b3", "g/w2"]);
        m.set_group_collapsed("Weapons", true);
        m.set_group_collapsed("Retired", true);
        m.set_group_excluded("Retired", true).unwrap();
        m.sets_filter.search = filter.into();
        let mut h = Harness::builder()
            .with_size(egui::vec2(w, 800.0))
            .wgpu()
            .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run();
        (h, d)
    };
    let crop = |h: &mut H, name: &str| {
        let panel = sets_panel(&*h);
        let img = h.render().expect("render");
        image::imageops::crop_imm(
            &img,
            panel.min.x.max(0.0) as u32,
            panel.min.y.max(0.0) as u32,
            panel.width() as u32,
            panel.height() as u32,
        )
        .to_image()
        .save(out.join(name))
        .expect("save png");
    };
    for w in [1100.0, 1280.0, 1600.0] {
        let (mut h, _d) = build(w, "");
        crop(&mut h, &format!("groups-{w}.png"));
    }
    let (mut h, _d) = build(1280.0, "c");
    crop(&mut h, "groups-filter-1280.png");
    let (mut h, _d) = build(1280.0, "");
    header(&h, "Armor").click_secondary();
    h.run();
    let img = h.render().expect("render");
    img.save(out.join("groups-menu-1280-full.png")).unwrap();
}
