//! The egui 0.37 behaviours the `egui-ui-rules` skill states, measured with bare egui, not Tessalith's UI. Each test
//! pins what egui does, so it is adjusted to whatever the egui in use does and cannot fail by design: it is evidence
//! for the skill, re-run on the next egui upgrade, not a guard on Tessalith. A failure after an upgrade means the
//! skill's entry needs checking. "G7" and so on are the rows of the 0.37 survey's section 11 (the skill's
//! `gotchas-egui-0.36.md`); source lines are egui 0.37.0's.
use eframe::egui::{self, accesskit::Role, containers::panel::PanelState};
use egui_kittest::kittest::{NodeT as _, Queryable};
use egui_kittest::Harness;
use sdc_gui::look;

fn harness<S: 'static>(
    size: egui::Vec2,
    state: S,
    app: impl FnMut(&mut egui::Ui, &mut S) + 'static,
) -> Harness<'static, S> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(size)
        .build_ui_state(app, state)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

// G7 -----------------------------------------------------------------------------------------------------------------

/// `ScrollBar` nodes for one 200 x 100 scroll area holding a block of `content`, under `thin_scroll()` or under the
/// harness's style (the pinned 0.36.2 look, which keeps egui's default floating bars).
fn bar_nodes(thin: bool, both: bool, content: egui::Vec2) -> usize {
    let mut h = harness(egui::vec2(400.0, 300.0), (), move |ui, _| {
        ui.allocate_ui(egui::vec2(200.0, 100.0), |ui| {
            if thin {
                ui.spacing_mut().scroll = look::thin_scroll();
            }
            let area = if both {
                egui::ScrollArea::both()
            } else {
                egui::ScrollArea::vertical()
            };
            area.id_salt("g7")
                .max_height(100.0)
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.allocate_exact_size(content, egui::Sense::hover());
                });
        });
    });
    h.run();
    h.query_all_by_role(Role::ScrollBar).count()
}

/// Pins egui (see the file comment). G7 said a scroll bar has no AccessKit node. On 0.37 each shown bar is a
/// `Role::ScrollBar` node (`scroll_area.rs:1471-1473`), with no name, and kittest's check does not ask for one. That
/// holds for the default floating bar too, whose dormant opacity is 0 (`style.rs:648-658`): the node is there while
/// nothing is drawn. Content that fits shows no bar and has no node.
#[test]
fn a_scroll_bar_has_an_accesskit_node() {
    for thin in [true, false] {
        let short = bar_nodes(thin, false, egui::vec2(150.0, 50.0));
        let tall = bar_nodes(thin, false, egui::vec2(150.0, 400.0));
        let both = bar_nodes(thin, true, egui::vec2(400.0, 400.0));
        assert_eq!(
            (short, tall, both),
            (0, 1, 2),
            "thin {thin}: bar nodes for content that fits, a tall vertical area, a wide and tall `both` area"
        );
    }
}

// G10 ----------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct WidePanel {
    both: bool,
    /// The panel's rect as `show` returns it, the screen, what is left of the screen after the panel, the scroll
    /// area's inner rect and the card's rect.
    panel: egui::Rect,
    screen: egui::Rect,
    rest: egui::Rect,
    area: egui::Rect,
    card: egui::Rect,
}

fn wide_panel(both: bool) -> WidePanel {
    let state = WidePanel {
        both,
        panel: egui::Rect::NOTHING,
        screen: egui::Rect::NOTHING,
        rest: egui::Rect::NOTHING,
        area: egui::Rect::NOTHING,
        card: egui::Rect::NOTHING,
    };
    let mut h = harness(egui::vec2(900.0, 400.0), state, |ui, s: &mut WidePanel| {
        s.screen = ui.max_rect();
        let r = egui::Panel::right("g10")
            .resizable(true)
            .default_size(300.0)
            .size_range(200.0..=300.0)
            .show(ui, |ui| {
                let area = if s.both {
                    egui::ScrollArea::both()
                } else {
                    egui::ScrollArea::vertical()
                };
                let out = area.id_salt("g10-scroll").show(ui, |ui| {
                    s.card = ui
                        .allocate_exact_size(egui::vec2(390.0, 40.0), egui::Sense::hover())
                        .0;
                });
                s.area = out.inner_rect;
            });
        s.panel = r.response.rect;
        s.rest = ui.available_rect_before_wrap();
    });
    h.run();
    *h.state()
}

/// Pins egui (see the file comment). G10: a 390 px card in a scroll area inside a right panel whose range tops out at
/// 300 px. In a `ScrollArea::vertical` the card widens the panel's frame past the range (the non-scrolled axis follows
/// the content, `scroll_area.rs:1291-1296`). egui then cuts the reported rect to the range's width from the frame's
/// right edge (`panel.rs:863-866`, `Right => rect.min.x = rect.max.x - size`, `:157-160`), so the rect runs off the
/// screen, and the parent's cursor stops at its left edge (`:869-883`): the centre is left room that the panel's
/// content draws over. `run()` settles. In a `ScrollArea::both` the panel stays 300 px at the screen edge.
#[test]
fn a_vertical_scroll_area_never_widens_its_panel_past_the_range() {
    // `ScrollArea::vertical`: the reported rect keeps the range's width but is pushed off the screen's right edge,
    // and what is left for the centre ends at its left edge, past where the panel's content starts.
    let v = wide_panel(false);
    assert!(near(v.card.width(), 390.0), "the card is 390 wide");
    assert!(v.panel.width() <= 300.5, "vertical: panel {:?}", v.panel);
    assert!(
        v.panel.right() > v.screen.right() + 50.0,
        "vertical: the panel's rect {:?} runs off the screen {:?}",
        v.panel,
        v.screen
    );
    assert!(near(v.rest.right(), v.panel.left()));
    assert!(
        v.rest.right() > v.area.left() + 50.0,
        "vertical: the centre {:?} reaches over the panel's content {:?}",
        v.rest,
        v.area
    );
    // `ScrollArea::both`, the control: the panel sits at the screen's right edge, 300 wide, beside the centre.
    let b = wide_panel(true);
    assert!(near(b.card.width(), 390.0), "the card is 390 wide");
    assert!(
        near(b.panel.right(), b.screen.right()),
        "both: {:?}",
        b.panel
    );
    assert!(near(b.panel.width(), 300.0), "both: {:?}", b.panel);
    assert!(near(b.rest.right(), b.panel.left()));
    assert!(
        b.rest.right() <= b.area.left(),
        "both: the centre ends before the content"
    );
}

// G11 ----------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct FullRow {
    /// The area's inner width last frame: the row is sized from it (Tessalith's CC2 File cell, without its margin).
    inner: Option<f32>,
    row: f32,
}

const ROW_FIXED: f32 = 100.0;

/// Pins egui (see the file comment). G11: a row exactly as wide as its column, sized from last frame's width (CC2's
/// File cell without its 8 px margin), in a `ScrollArea::both` with `thin_scroll()`. The skill said `run()` never
/// settles; this bare shape settles with the row at the area's inner width and no bar shown (`content_is_too_large`
/// compares `limit_rect.width().ceil() < content_size.x`, `scroll_area.rs:1310-1313`, the same code as 0.36.2's
/// `:1203-1205`). So the flip is not reproduced by this shape, which does not prove it gone.
#[test]
fn a_row_as_wide_as_its_column_settles() {
    let mut h = harness(
        egui::vec2(400.0, 300.0),
        FullRow::default(),
        |ui, s: &mut FullRow| {
            ui.allocate_ui(egui::vec2(300.0, 200.0), |ui| {
                ui.spacing_mut().scroll = look::thin_scroll();
                let out = egui::ScrollArea::both()
                    .id_salt("g11")
                    .max_height(200.0)
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        let gap = ui.spacing().item_spacing.x;
                        let col = s.inner.unwrap_or_else(|| ui.available_width());
                        let flex = (col - ROW_FIXED - gap).max(0.0);
                        let r = ui.horizontal(|ui| {
                            ui.allocate_exact_size(
                                egui::vec2(ROW_FIXED, 20.0),
                                egui::Sense::hover(),
                            );
                            ui.allocate_exact_size(egui::vec2(flex, 20.0), egui::Sense::hover());
                        });
                        s.row = r.response.rect.width();
                        // Enough below the row that the area needs its vertical bar only once the horizontal one
                        // takes its height.
                        ui.allocate_exact_size(egui::vec2(10.0, 175.0), egui::Sense::hover());
                    });
                s.inner = Some(out.inner_rect.width());
            });
        },
    );
    h.run();
    let s = h.state();
    assert_eq!(
        Some(s.row),
        s.inner,
        "the row is exactly as wide as the area"
    );
    assert_eq!(h.query_all_by_role(Role::ScrollBar).count(), 0, "no bar");
}

// G22 ----------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Combos {
    a: u8,
    b: u8,
}

fn combos_in_a_menu() -> Harness<'static, Combos> {
    harness(
        egui::vec2(600.0, 400.0),
        Combos::default(),
        |ui, s: &mut Combos| {
            egui::containers::menu::MenuButton::new("Options")
                .config(
                    egui::containers::menu::MenuConfig::new()
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                )
                .ui(ui, |ui| {
                    egui::ComboBox::from_label("First")
                        .selected_text(format!("A{}", s.a))
                        .show_ui(ui, |ui| {
                            for i in 0..3 {
                                ui.selectable_value(&mut s.a, i, format!("A{i}"));
                            }
                        });
                    egui::ComboBox::from_label("Second")
                        .selected_text(format!("B{}", s.b))
                        .show_ui(ui, |ui| {
                            for i in 0..3 {
                                ui.selectable_value(&mut s.b, i, format!("B{i}"));
                            }
                        });
                });
        },
    )
}

fn menu_open(h: &Harness<'static, Combos>) -> bool {
    h.query_by_role_and_label(Role::ComboBox, "First").is_some()
}

fn open_menu(h: &mut Harness<'static, Combos>) {
    h.get_by_label("Options").click();
    h.run();
    assert!(menu_open(h), "the menu opened");
}

/// Pins egui (see the file comment). G22: two `ComboBox`es in a `MenuButton` that closes on a click outside. Held as
/// the skill states it: egui keeps one open popup per viewport (`memory/mod.rs:1126-1129`, `open_popup` replaces the
/// one entry), so a click that opens either combo closes the menu, the combo's list never shows and nothing can be
/// picked. A click outside and Esc still close the menu. Tessalith's own `combo_menu` is the way round it.
#[test]
fn two_combos_in_a_menu_open_and_close_in_order() {
    let mut h = combos_in_a_menu();
    h.run();
    for (combo, option) in [("First", "A2"), ("Second", "B1")] {
        open_menu(&mut h);
        h.get_by_role_and_label(Role::ComboBox, combo).click();
        h.run();
        assert!(!menu_open(&h), "opening {combo} closed the menu");
        assert!(h.query_by_label(option).is_none(), "{combo}: no option");
    }
    assert_eq!((h.state().a, h.state().b), (0, 0), "nothing was picked");
    open_menu(&mut h);
    let outside = egui::pos2(500.0, 300.0);
    h.hover_at(outside);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: outside,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run();
    assert!(!menu_open(&h), "a click outside closed the menu");
    open_menu(&mut h);
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!menu_open(&h), "Esc closed the menu");
}

// G23 ----------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct FilterMenu {
    filter: String,
    picked: Option<usize>,
}

const ROWS: usize = 3;

/// Survey 4.4's probe: a `menu_button` holding a filter box and three rows, and a second one without the box.
fn filter_menus() -> Harness<'static, FilterMenu> {
    harness(
        egui::vec2(600.0, 400.0),
        FilterMenu::default(),
        |ui, s: &mut FilterMenu| {
            ui.horizontal(|ui| {
                ui.menu_button("Filtered", |ui| {
                    ui.add(egui::TextEdit::singleline(&mut s.filter).hint_text("Filter"));
                    for i in 0..ROWS {
                        if ui.button(format!("Row {i}")).clicked() {
                            s.picked = Some(i);
                        }
                    }
                });
                ui.menu_button("Plain", |ui| {
                    for i in 0..ROWS {
                        if ui.button(format!("Row {i}")).clicked() {
                            s.picked = Some(i);
                        }
                    }
                });
            });
        },
    )
}

fn filter_box<'h>(h: &'h Harness<'static, FilterMenu>) -> egui_kittest::Node<'h> {
    h.get_by_role(Role::TextInput)
}

fn rows_shown(h: &Harness<'static, FilterMenu>) -> bool {
    h.query_by_label("Row 0").is_some()
}

/// Pins egui (see the file comment). G23: the menu's arrow-key navigation (`menu.rs:232-288`) and a focused filter
/// box in it. A focused TextEdit's event filter takes both arrow axes (`text_edit/builder.rs:148-151`), and
/// `Memory` turns an arrow key into a focus move only when the focused widget's filter does not take it
/// (`memory/mod.rs:581-607`), so the menu never moves the focus out of the box. With nothing focused, Down goes to
/// the menu's first focusable widget (`menu.rs:257,264-268`). The menu's AccessKit node is a child of the button
/// that opened it (`popup.rs:642-644`). Task 9's `popups::arrow_keys_and_typing_stay_in_the_input_name_filter`
/// checks the same on Tessalith's input-name menu.
#[test]
fn arrow_keys_stay_in_a_menus_filter_box() {
    let mut h = filter_menus();
    h.run();
    h.get_by_label("Filtered").click();
    h.run();
    let opener = h.get_by_label("Filtered").accesskit_node().id();
    let row = h.get_by_label("Row 0").accesskit_node();
    assert!(
        std::iter::successors(row.parent(), |n| n.parent()).any(|n| n.id() == opener),
        "the menu's rows sit under the button that opened it"
    );
    filter_box(&h).focus();
    h.run();
    let typed = |h: &mut Harness<'static, FilterMenu>, t: &str| {
        h.event(egui::Event::Text(t.into()));
        h.run();
    };
    let key = |h: &mut Harness<'static, FilterMenu>, k: egui::Key| {
        h.key_press(k);
        h.run();
    };
    let in_box = |h: &Harness<'static, FilterMenu>, text: &str, step: &str| {
        assert_eq!(h.state().filter, text, "{step}");
        assert!(
            filter_box(h).is_focused(),
            "{step}: the box keeps the focus"
        );
        assert!(rows_shown(h), "{step}: the menu stays open");
    };
    typed(&mut h, "nor");
    key(&mut h, egui::Key::ArrowLeft);
    typed(&mut h, "x");
    in_box(&h, "noxr", "nor, Left, x");
    key(&mut h, egui::Key::Home);
    typed(&mut h, "H");
    key(&mut h, egui::Key::End);
    key(&mut h, egui::Key::Backspace);
    in_box(&h, "Hnox", "Home, H, End, Backspace");
    key(&mut h, egui::Key::ArrowRight);
    in_box(&h, "Hnox", "Right");
    for k in [
        egui::Key::ArrowDown,
        egui::Key::ArrowDown,
        egui::Key::ArrowUp,
    ] {
        key(&mut h, k);
    }
    in_box(&h, "Hnox", "Down, Down, Up");
    h.get_by_label("Row 1").click();
    h.run();
    assert_eq!(h.state().picked, Some(1), "the click picked row 1");
    assert!(!rows_shown(&h), "the pick closed the menu");

    // Nothing focused, the menu opened by a click: Down goes into the filter box first.
    h.state_mut().filter.clear();
    h.get_by_label("Filtered").click();
    h.run();
    assert_eq!(h.ctx.memory(|m| m.focused()), None, "nothing focused");
    key(&mut h, egui::Key::ArrowDown);
    in_box(&h, "", "Down from nothing focused");

    // The same menu without the box: Down walks the rows and Enter picks.
    key(&mut h, egui::Key::Escape);
    assert!(!rows_shown(&h), "Esc closed the menu");
    h.state_mut().picked = None;
    h.get_by_label("Plain").click();
    h.run();
    assert_eq!(h.ctx.memory(|m| m.focused()), None, "nothing focused");
    key(&mut h, egui::Key::ArrowDown);
    assert!(h.get_by_label("Row 0").is_focused(), "Down: row 0");
    key(&mut h, egui::Key::ArrowDown);
    assert!(h.get_by_label("Row 1").is_focused(), "Down, Down: row 1");
    key(&mut h, egui::Key::Enter);
    assert_eq!(h.state().picked, Some(1), "Enter picked row 1");
    assert!(rows_shown(&h), "the menu stays open after Enter");
}

// G28 ----------------------------------------------------------------------------------------------------------------

/// Pins egui (see the file comment). G28: a TextEdit's hint is an AccessKit placeholder, not a label
/// (`response.rs:1044-1045`), so `query_by_label(hint)` finds nothing. On the kittest side, a placeholder counts as a
/// name for the unnamed-widget check (egui_kittest `accessibility.rs:46-55`): a box with only a hint passes `run()`.
#[test]
fn a_hint_is_not_a_label() {
    let mut h = harness(egui::vec2(300.0, 100.0), String::new(), |ui, s| {
        ui.add(egui::TextEdit::singleline(s).hint_text("Search"));
    });
    // `run()` runs kittest's unnamed-widget check: the box has no label, only the hint.
    h.run();
    assert!(h.query_by_label("Search").is_none(), "the hint is no label");
    let b = h.get_by_role(Role::TextInput);
    assert_eq!(b.accesskit_node().label(), None);
    assert_eq!(b.accesskit_node().placeholder(), Some("Search"));
}

// Panel ids ----------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Scopes {
    first: Option<egui::Id>,
    second: Option<egui::Id>,
}

/// Pins egui (see the file comment). The skill's examples key a panel's `PanelState` by `Id::new(salt)`. On 0.37 a
/// panel's id is `parent_ui.scope_id().with_salt(salt)` (`panel.rs:971-976`), so one salt shown in two parents is two
/// states and none is under `Id::new(salt)`, which is `Id::unique(salt)` (`id.rs:102-104`, deprecated).
/// `ui::panel_ids` records the id egui uses.
#[test]
fn a_panel_is_stored_under_its_parents_scope() {
    let mut h = harness(
        egui::vec2(600.0, 300.0),
        Scopes::default(),
        |ui, s: &mut Scopes| {
            ui.push_id("first", |ui| {
                s.first = Some(ui.scope_id());
                sdc_gui::ui::panel_ids::left(ui, "g-scoped").show(ui, |ui| ui.label("One"));
            });
            ui.push_id("second", |ui| {
                s.second = Some(ui.scope_id());
                egui::Panel::left("g-scoped").show(ui, |ui| ui.label("Two"));
            });
        },
    );
    h.run();
    #[allow(deprecated)]
    let new = egui::Id::new("g-scoped");
    assert_eq!(new, egui::Id::unique("g-scoped"));
    assert!(
        PanelState::load(&h.ctx, new).is_none(),
        "nothing under Id::new"
    );
    let (first, second) = (h.state().first.unwrap(), h.state().second.unwrap());
    let (first, second) = (first.with("g-scoped"), second.with("g-scoped"));
    assert_ne!(first, second);
    assert!(
        PanelState::load(&h.ctx, first).is_some(),
        "the first parent's"
    );
    assert!(PanelState::load(&h.ctx, second).is_some(), "the second's");
    assert_eq!(
        sdc_gui::ui::panel_ids::panel_id(&h.ctx, "g-scoped"),
        Some(first)
    );
}

// TextEdit margin ----------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Heights {
    with: Vec<f32>,
    without: Vec<f32>,
}

/// Pins egui (see the file comment). Spec section 4 item 7: a singleline TextEdit is 21.28 pt tall with the pinned
/// 0.36.2 margin (`look::TEXT_EDIT_MARGIN`) and 19.28 with 0.37's own, 2 pt short (`text_edit/builder.rs:143`
/// `margin: None` keeps the frame's theme margin, which `look.rs` reads as `button_padding`). Neither changes when the
/// box takes focus: a set margin leaves room for the focus stroke (`text_edit/builder.rs:696-701`). Segoe UI 14 pt
/// from `install_system_fonts`.
#[test]
fn text_edit_height_with_and_without_the_036_margin() {
    let mut h = harness(
        egui::vec2(400.0, 200.0),
        (Heights::default(), String::new(), String::new()),
        |ui, (s, a, b)| {
            let with = ui
                .add(egui::TextEdit::singleline(a).margin(look::TEXT_EDIT_MARGIN))
                .accessible_name("With the margin");
            s.with.push(with.rect.height());
            let without = ui
                .add(egui::TextEdit::singleline(b))
                .accessible_name("Without");
            s.without.push(without.rect.height());
        },
    );
    h.run();
    let (with, without) = {
        let s = &h.state().0;
        (*s.with.last().unwrap(), *s.without.last().unwrap())
    };
    assert!(near(with, 21.28), "with the margin, unfocused: {with}");
    assert!(near(without, 19.28), "without, unfocused: {without}");
    for (name, want) in [("With the margin", with), ("Without", without)] {
        h.state_mut().0 = Heights::default();
        h.get_by_label(name).focus();
        h.run();
        assert!(h.get_by_label(name).is_focused(), "{name} took the focus");
        let s = &h.state().0;
        let got = if name == "Without" {
            &s.without
        } else {
            &s.with
        };
        let got = *got.last().unwrap();
        assert!(near(got, want), "{name}, focused: {got}");
    }
}

// Nested panel -------------------------------------------------------------------------------------------------------

struct Nested {
    window_clip: egui::Rect,
    /// The panel's content `Ui`: its max rect and clip.
    panel_max: egui::Rect,
    panel_clip: egui::Rect,
}

impl Default for Nested {
    fn default() -> Self {
        Self {
            window_clip: egui::Rect::NOTHING,
            panel_max: egui::Rect::NOTHING,
            panel_clip: egui::Rect::NOTHING,
        }
    }
}

const MARK: egui::Color32 = egui::Color32::from_rgb(250, 0, 250);

/// Pins egui (see the file comment). Spec section 4 item 7: a 300 px `Panel::right` inside a 150 px `Window`. The
/// panel's content `Ui` reaches past the window's right edge, but its clip is the window's
/// (`panel.rs:842-844` `shrink_clip_rect`; 0.36.2 replaced the clip with `set_clip_rect`, `panel.rs:824`), so a fill
/// over the panel's whole max rect is painted under a clip inside the window's. Measured from the clip rects and from
/// the painted shape's clip in the frame's output.
#[test]
fn a_nested_panel_is_clipped_to_its_parent() {
    let mut h = harness(
        egui::vec2(600.0, 400.0),
        Nested::default(),
        |ui, s: &mut Nested| {
            let ctx = ui.ctx().clone();
            egui::Window::new("Narrow")
                .fixed_pos(egui::pos2(200.0, 50.0))
                .fixed_size(egui::vec2(150.0, 120.0))
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    s.window_clip = ui.clip_rect();
                    egui::Panel::right("g-nested")
                        .exact_size(300.0)
                        .resizable(false)
                        .show(ui, |ui| {
                            s.panel_max = ui.max_rect();
                            s.panel_clip = ui.clip_rect();
                            ui.painter().rect_filled(ui.max_rect(), 0.0, MARK);
                        });
                });
        },
    );
    h.run();
    let s = h.state();
    assert!(
        s.panel_max.right() > s.window_clip.right() + 100.0,
        "the panel reaches past the window: panel {:?}, window clip {:?}",
        s.panel_max,
        s.window_clip
    );
    let inside = s.window_clip.expand(0.5);
    assert!(
        inside.contains_rect(s.panel_clip),
        "panel clip {:?} in window clip {:?}",
        s.panel_clip,
        s.window_clip
    );
    let marks: Vec<egui::Rect> = h
        .output()
        .shapes
        .iter()
        .filter(|c| matches!(&c.shape, egui::Shape::Rect(r) if r.fill == MARK))
        .map(|c| c.clip_rect)
        .collect();
    assert_eq!(marks.len(), 1, "the panel's fill was painted once");
    assert!(
        inside.contains_rect(marks[0]),
        "painted under clip {:?}, window clip {:?}",
        marks[0],
        s.window_clip
    );
}

// Hover text ---------------------------------------------------------------------------------------------------------

const ICON_TIP: &str = "Delete the set";
const SAVE_TIP: &str = "Writes the file";
const DRAG_TIP: &str = "From 0 to 1";

/// Pins egui (see the file comment). `on_hover_text` (`response.rs:785-795`) sets the hover text as the widget's
/// AccessKit description, and as its name only when the role is not Label, `labelled_by` is empty and the name is
/// empty (`response.rs:797-816`): an empty-text button is named by it, a button keeps its text, and a DragValue
/// `labelled_by` a label is named by the label. `labelled_by` clears the widget's own name (the DragValue's prefix,
/// `response.rs:1065-1071`). This needs no hover. The tooltip itself shows on the 4th kittest step after the pointer
/// moves onto the button, not before: it waits for a still pointer and `tooltip_delay` (`tooltip.rs:416-438`), and
/// its first frame is an invisible sizing pass (`area.rs:691-692`) that 0.37 leaves out of the AccessKit tree, so the
/// text is in the tree from the frame it is painted (`nif_edit.rs`'s `TOOLTIP_STEPS`).
#[test]
fn hover_text_names_an_unnamed_button() {
    let mut h = harness(egui::vec2(400.0, 200.0), 0.5_f32, |ui, v| {
        ui.add_sized([24.0, 20.0], egui::Button::new(""))
            .on_hover_text(ICON_TIP);
        ui.button("Save").on_hover_text(SAVE_TIP);
        ui.horizontal(|ui| {
            let l = ui.label("Strength");
            ui.add(egui::DragValue::new(v).prefix("Own: "))
                .labelled_by(l.id)
                .on_hover_text(DRAG_TIP);
        });
    });
    h.run();
    let icon = h.get_by_role_and_label(Role::Button, ICON_TIP);
    assert_eq!(icon.accesskit_node().description(), Some(ICON_TIP.into()));
    let save = h.get_by_role_and_label(Role::Button, "Save");
    assert_eq!(save.accesskit_node().description(), Some(SAVE_TIP.into()));
    let drag = h.get_by_role_and_label(Role::SpinButton, "Strength");
    assert_eq!(drag.accesskit_node().description(), Some(DRAG_TIP.into()));
    // The tooltip itself: absent after 3 frames, shown on the 4th.
    let at = save.rect().center();
    h.hover_at(at);
    let mut shown_on = None;
    for step in 1..=8 {
        h.step();
        if h.query_by_label(SAVE_TIP).is_some() {
            shown_on = Some(step);
            break;
        }
    }
    assert_eq!(shown_on, Some(4), "the tooltip's first frame");
}
