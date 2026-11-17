//! egui 0.36.2's look, kept on egui 0.37 (owner rulings 1 and 2 of the 0.37 upgrade): its spacing, its widget
//! visuals, its TextEdit margin and no rubber band past a scroll area's edge. `fonts::install_system_fonts` applies
//! `pin_036_style` to both themes, so the app and every kittest harness get it.
use eframe::egui;

/// 0.36.2's TextEdit margin (`R\egui-0.36.2\src\widgets\text_edit\builder.rs:136`). 0.37 takes `button_padding`
/// instead, which is 2 pt short under the pinned (4, 1). Every `TextEdit` in `ui` sets it.
pub const TEXT_EDIT_MARGIN: egui::Margin = egui::Margin::symmetric(4, 2);

/// Sets `style` to 0.36.2's defaults where 0.37 changed them (survey 4.2a): `button_padding`, `interact_size`, the
/// widget visuals of `style`'s theme, and no rubber band. 0.37's kinetic `decay_time`/`stop_distance` stay: 0.36.2's
/// hard-coded momentum cannot be written in them.
pub fn pin_036_style(style: &mut egui::Style) {
    style.spacing.button_padding = egui::vec2(4.0, 1.0);
    style.spacing.interact_size = egui::vec2(40.0, 18.0);
    style.spacing.scroll.kinetic.rubber_band = false;
    style.visuals.widgets = if style.visuals.dark_mode {
        widgets_036_dark()
    } else {
        widgets_036_light()
    };
}

/// Shows a `ComboBox` as 0.36.2 did: inside a `ui.horizontal` row of its own (`R\egui-0.36.2\src\containers\
/// combo_box.rs:234`). 0.37 draws it in the caller's layout (`R\egui-0.37.0\src\containers\combo_box.rs:265`), so a
/// long value wraps in a vertical layout (the row gave it `TextWrapMode::Extend`, `R\egui-0.36.2\src\ui.rs:589`) and
/// a combo beside a 19 pt button sits half a point higher. Every `ComboBox` in `ui` goes through it
/// (`look::tests::every_combo_box_is_shown_through_show_ui_036`).
pub trait Combo036 {
    fn show_ui_036<R>(
        self,
        ui: &mut egui::Ui,
        menu_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>>;
}

impl Combo036 for egui::ComboBox<'_> {
    fn show_ui_036<R>(
        self,
        ui: &mut egui::Ui,
        menu_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        ui.horizontal(|ui| self.show_ui(ui, menu_contents)).inner
    }
}

/// 0.36.2's submenu arrow, a text glyph (`R\egui-0.36.2\src\containers\menu.rs:344-347`).
pub const SUBMENU_ARROW_036: &str = "⏵";

/// `Ui::menu_button` as 0.36.2 drew it. Inside a menu, 0.37 paints the submenu arrow as a 14 pt triangle atom
/// (`R\egui-0.37.0\src\containers\menu.rs:492-511`), so the entry is 1 px wider than 0.36.2's 13 pt "⏵" glyph and the
/// arrow sits 2 px higher. This puts the glyph back through `SubMenuButton::from_button`, the hook 0.37 gives for it
/// (`:514-518`), and keeps the accessible name the entry's text, as J?4 records (0.36.2 read "Move to group ⏵").
/// Outside a menu it is `MenuButton::new`, as `Ui::menu_button` is (`R\egui-0.37.0\src\ui.rs:2927-2931`). Every
/// menu button in `ui` goes through it (`look::tests::every_menu_button_goes_through_menu_button_036`).
pub fn menu_button_036<R>(
    ui: &mut egui::Ui,
    text: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    let (response, inner) = if egui::menu::is_in_menu(ui) {
        let button = egui::Button::new(text).right_text(SUBMENU_ARROW_036);
        let (r, inner) = egui::menu::SubMenuButton::from_button(button).ui(ui, add_contents);
        (r.accessible_name(text), inner)
    } else {
        egui::menu::MenuButton::new(text).ui(ui, add_contents)
    };
    egui::InnerResponse::new(inner.map(|i| i.inner), response)
}

/// `ScrollStyle::thin()` without 0.37's rubber band, for the areas that scope a thin bar.
pub fn thin_scroll() -> egui::style::ScrollStyle {
    let mut s = egui::style::ScrollStyle::thin();
    s.kinetic.rubber_band = false;
    s
}

fn visuals(
    weak_bg_fill: egui::Color32,
    bg_fill: egui::Color32,
    bg_stroke: egui::Stroke,
    fg_stroke: egui::Stroke,
    corner_radius: u8,
) -> egui::style::WidgetVisuals {
    egui::style::WidgetVisuals {
        weak_bg_fill,
        bg_fill,
        bg_stroke,
        fg_stroke,
        corner_radius: egui::CornerRadius::same(corner_radius),
        expansion: 0.0,
    }
}

/// 0.36.2's `Widgets::dark()`, field by field (`R\egui-0.36.2\src\style.rs:1680-1724`).
fn widgets_036_dark() -> egui::style::Widgets {
    use egui::{Color32, Stroke};
    let grey = Color32::from_gray;
    egui::style::Widgets {
        noninteractive: visuals(
            grey(27),
            grey(27),
            Stroke::new(1.0, grey(60)),
            Stroke::new(1.0, grey(140)),
            2,
        ),
        inactive: visuals(
            grey(60),
            grey(60),
            Stroke::default(),
            Stroke::new(1.0, grey(180)),
            2,
        ),
        hovered: visuals(
            grey(70),
            grey(70),
            Stroke::new(1.0, grey(150)),
            Stroke::new(1.5, grey(240)),
            3,
        ),
        active: visuals(
            grey(55),
            grey(55),
            Stroke::new(1.0, Color32::WHITE),
            Stroke::new(2.0, Color32::WHITE),
            2,
        ),
        open: visuals(
            grey(45),
            grey(27),
            Stroke::new(1.0, grey(60)),
            Stroke::new(1.0, grey(210)),
            2,
        ),
    }
}

/// 0.36.2's `Widgets::light()`, field by field (`R\egui-0.36.2\src\style.rs:1726-1770`).
fn widgets_036_light() -> egui::style::Widgets {
    use egui::{Color32, Stroke};
    let grey = Color32::from_gray;
    egui::style::Widgets {
        noninteractive: visuals(
            grey(248),
            grey(248),
            Stroke::new(1.0, grey(190)),
            Stroke::new(1.0, grey(80)),
            2,
        ),
        inactive: visuals(
            grey(230),
            grey(230),
            Stroke::default(),
            Stroke::new(1.0, grey(60)),
            2,
        ),
        hovered: visuals(
            grey(220),
            grey(220),
            Stroke::new(1.0, grey(105)),
            Stroke::new(1.5, Color32::BLACK),
            3,
        ),
        active: visuals(
            grey(165),
            grey(165),
            Stroke::new(1.0, Color32::BLACK),
            Stroke::new(2.0, Color32::BLACK),
            2,
        ),
        open: visuals(
            grey(220),
            grey(220),
            Stroke::new(1.0, grey(160)),
            Stroke::new(1.0, Color32::BLACK),
            2,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.05
    }

    fn screen() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        }
    }

    /// One pass with no renderer: the font texture's deltas are dropped on purpose.
    fn run(
        ctx: &egui::Context,
        input: egui::RawInput,
        ui: impl FnMut(&mut egui::Ui),
    ) -> egui::FullOutput {
        let mut out = ctx.run_ui(input, ui);
        out.textures_delta.clear();
        out
    }

    /// A dark-theme context with `setup` applied (the app's `install_system_fonts`).
    fn context(setup: fn(&egui::Context) -> Result<(), String>) -> egui::Context {
        let ctx = egui::Context::default();
        ctx.set_theme(egui::Theme::Dark);
        setup(&ctx).expect("system fonts");
        ctx
    }

    #[derive(Debug, Default)]
    struct Sizes {
        button: egui::Vec2,
        combo: egui::Vec2,
        check: egui::Vec2,
        edit: egui::Vec2,
        drag: egui::Vec2,
        drag_edit: egui::Vec2,
    }

    /// Each widget's size after 4 passes; then the DragValue is focused (edit mode) and 4 more passes run.
    fn sizes_after(setup: fn(&egui::Context) -> Result<(), String>) -> Sizes {
        let ctx = context(setup);
        let (mut text, mut on, mut value) = (String::from("x"), false, 0.0_f32);
        let mut s = Sizes::default();
        let mut drag_id = None;
        let mut pass = |ctx: &egui::Context, s: &mut Sizes, drag_id: &mut Option<egui::Id>| {
            run(ctx, screen(), |ui| {
                s.button = ui.button("Undo").rect.size();
                s.combo = egui::ComboBox::from_id_salt("look-combo")
                    .width(120.0)
                    .selected_text("A")
                    .show_ui_036(ui, |_| {})
                    .response
                    .rect
                    .size();
                s.check = ui.checkbox(&mut on, "Check").rect.size();
                s.edit = ui
                    .add(egui::TextEdit::singleline(&mut text).margin(TEXT_EDIT_MARGIN))
                    .rect
                    .size();
                let r = ui.add(egui::DragValue::new(&mut value));
                *drag_id = Some(r.id);
                if r.has_focus() {
                    s.drag_edit = r.rect.size();
                } else {
                    s.drag = r.rect.size();
                }
            });
        };
        for _ in 0..4 {
            pass(&ctx, &mut s, &mut drag_id);
        }
        ctx.memory_mut(|m| m.request_focus(drag_id.expect("a DragValue was drawn")));
        for _ in 0..4 {
            pass(&ctx, &mut s, &mut drag_id);
        }
        s
    }

    #[test]
    fn the_widgets_keep_their_036_sizes() {
        // Segoe UI 14 pt (fonts::install_system_fonts); 0.36.2 values from survey 4.2 and the 2026-11-16 probe (4.2a).
        let s = sizes_after(crate::fonts::install_system_fonts);
        assert!(
            near(s.button.x, 39.6) && near(s.button.y, 19.0),
            "button {:?}",
            s.button
        );
        assert!(
            near(s.combo.x, 120.0) && near(s.combo.y, 19.0),
            "combo .width(120) {:?}",
            s.combo
        );
        assert!(near(s.check.y, 18.0), "checkbox {:?}", s.check);
        assert!(
            near(s.edit.y, 21.28),
            "TextEdit with TEXT_EDIT_MARGIN {:?}",
            s.edit
        );
        assert!(
            near(s.drag.x, 40.0) && near(s.drag.y, 19.0),
            "DragValue {:?}",
            s.drag
        );
        assert!(
            near(s.drag_edit.y, 19.28),
            "DragValue in edit mode {:?}",
            s.drag_edit
        );
    }

    #[test]
    fn widget_visuals_are_036s_in_both_themes() {
        // 180: R\egui-0.36.2\src\style.rs Widgets::dark() inactive fg_stroke; 60: Widgets::light() inactive fg_stroke,
        // `:1740` ("fg_stroke: Stroke::new(1.0, Color32::from_gray(60)), // button text").
        for (theme, grey_inactive_fg) in [(egui::Theme::Dark, 180), (egui::Theme::Light, 60)] {
            let mut s = theme.default_style();
            pin_036_style(&mut s);
            let w = &s.visuals.widgets;
            assert_eq!(
                w.inactive.corner_radius,
                egui::CornerRadius::same(2),
                "{theme:?}"
            );
            assert_eq!(
                w.hovered.corner_radius,
                egui::CornerRadius::same(3),
                "{theme:?}"
            );
            assert_eq!(
                w.noninteractive.corner_radius,
                egui::CornerRadius::same(2),
                "{theme:?}"
            );
            assert_eq!(
                w.inactive.fg_stroke.color,
                egui::Color32::from_gray(grey_inactive_fg),
                "{theme:?}"
            );
        }
        // Survey 4.2a's pinned rows: active and open radii, and the dark hovered fill and text.
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            let mut s = theme.default_style();
            pin_036_style(&mut s);
            assert_eq!(
                s.visuals.widgets.active.corner_radius,
                egui::CornerRadius::same(2),
                "{theme:?}"
            );
            assert_eq!(
                s.visuals.widgets.open.corner_radius,
                egui::CornerRadius::same(2),
                "{theme:?}"
            );
        }
        let mut d = egui::Theme::Dark.default_style();
        pin_036_style(&mut d);
        let w = &d.visuals.widgets;
        assert_eq!(w.inactive.weak_bg_fill, egui::Color32::from_gray(60));
        assert_eq!(w.hovered.weak_bg_fill, egui::Color32::from_gray(70));
        assert_eq!(w.active.weak_bg_fill, egui::Color32::from_gray(55));
        assert_eq!(w.hovered.fg_stroke.color, egui::Color32::from_gray(240));
    }

    struct Painted {
        text_edit_focused: egui::epaint::RectShape,
        checkbox_box: egui::epaint::RectShape,
    }

    fn rects(shape: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
        match shape {
            egui::Shape::Rect(r) => out.push(r.clone()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| rects(s, out)),
            _ => {}
        }
    }

    /// The largest filled `Shape::Rect` painted inside `area` (grown by 1 pt for a stroke drawn outside). 0.37 also
    /// paints a larger unfilled rect with square corners in a checkbox's row; the box is the filled one.
    fn rect_in(shapes: &[egui::epaint::ClippedShape], area: egui::Rect) -> egui::epaint::RectShape {
        let mut all = Vec::new();
        shapes.iter().for_each(|c| rects(&c.shape, &mut all));
        all.into_iter()
            .filter(|r| {
                area.expand(1.0).contains_rect(r.rect) && r.fill != egui::Color32::TRANSPARENT
            })
            .max_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))
            .unwrap_or_else(|| panic!("no rect painted in {area:?}"))
    }

    /// Dark theme: 4 passes, then the TextEdit is focused and 4 more passes run; the painted rects of the last pass.
    fn painted_after(setup: fn(&egui::Context) -> Result<(), String>) -> Painted {
        let ctx = context(setup);
        let (mut text, mut on) = (String::new(), false);
        let edit_id = egui::Id::unique("look-edit");
        let mut last = None;
        for n in 0..8 {
            if n == 4 {
                ctx.memory_mut(|m| m.request_focus(edit_id));
            }
            let (mut edit, mut check) = (egui::Rect::NOTHING, egui::Rect::NOTHING);
            let out = run(&ctx, screen(), |ui| {
                edit = ui
                    .add(
                        egui::TextEdit::singleline(&mut text)
                            .id(edit_id)
                            .margin(TEXT_EDIT_MARGIN),
                    )
                    .rect;
                check = ui.checkbox(&mut on, "Check").rect;
            });
            last = Some((out.shapes, edit, check));
        }
        let (shapes, edit, check) = last.expect("8 passes ran");
        assert!(
            ctx.memory(|m| m.has_focus(edit_id)),
            "the TextEdit took focus"
        );
        Painted {
            text_edit_focused: rect_in(&shapes, edit),
            checkbox_box: rect_in(&shapes, check),
        }
    }

    #[test]
    fn a_focused_text_edit_and_a_checkbox_paint_as_in_036() {
        // Dark theme, install_system_fonts, Context::run_ui at 1280x800. Read the painted `Shape::Rect` that lies in each
        // widget's rect (as the survey 4.2a shape probe does). 0.36.2 values from that probe.
        let p = painted_after(crate::fonts::install_system_fonts);
        assert_eq!(
            p.text_edit_focused.stroke,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(0xC0, 0xDE, 0xFF))
        );
        assert_eq!(
            p.text_edit_focused.corner_radius,
            egui::CornerRadius::same(2)
        );
        assert_eq!(p.text_edit_focused.fill, egui::Color32::from_gray(10));
        assert_eq!(p.checkbox_box.corner_radius, egui::CornerRadius::same(2));
        assert_eq!(p.checkbox_box.fill, egui::Color32::from_gray(60));
        assert!(
            near(p.checkbox_box.rect.width(), 14.0),
            "{:?}",
            p.checkbox_box.rect
        );
    }

    /// A 200 pt tall `thin_scroll()` area over "row 0".."row 39": row 0's painted top at rest, then while the content
    /// is dragged 200 pt down from the top and held for 2 passes.
    fn first_row_top_while_dragging_past_top() -> (f32, Vec<f32>) {
        let ctx = context(crate::fonts::install_system_fonts);
        let mut time = 0.0;
        let mut pass = |events: Vec<egui::Event>| {
            time += 1.0 / 60.0;
            let mut top = f32::NAN;
            let mut input = screen();
            input.time = Some(time);
            input.events = events;
            run(&ctx, input, |ui| {
                ui.allocate_ui(egui::vec2(300.0, 200.0), |ui| {
                    ui.spacing_mut().scroll = thin_scroll();
                    egui::ScrollArea::vertical()
                        .id_salt("look-scroll")
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for n in 0..40 {
                                let r = ui.label(format!("row {n}"));
                                if n == 0 {
                                    top = r.rect.top();
                                }
                            }
                        });
                });
            });
            top
        };
        let start = egui::pos2(30.0, 30.0);
        let moved = |y: f32| egui::Event::PointerMoved(start + egui::vec2(0.0, y));
        let button = |pressed: bool, y: f32| egui::Event::PointerButton {
            pos: start + egui::vec2(0.0, y),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        // Drag-to-scroll is on only once egui has seen a touch screen (`DragScroll::OnTouch`, 0.36.2 and 0.37 alike).
        let touch = |phase| egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(1),
            phase,
            pos: egui::pos2(1000.0, 700.0),
            force: None,
        };
        pass(vec![
            touch(egui::TouchPhase::Start),
            touch(egui::TouchPhase::End),
        ]);
        pass(vec![moved(0.0)]);
        let rest = pass(vec![]);
        pass(vec![button(true, 0.0)]);
        for step in 1..=10 {
            pass(vec![moved(20.0 * step as f32)]);
        }
        let held = vec![pass(vec![]), pass(vec![])];
        pass(vec![button(false, 200.0)]);
        (rest, held)
    }

    #[test]
    fn a_drag_past_the_end_of_a_thin_scroll_list_stops_at_the_edge() {
        // Review Focus 5. Reads the painted row, not ScrollArea State: `clamped_offset()` hides any overscroll.
        let (rest, held) = first_row_top_while_dragging_past_top();
        assert!(
            held.iter().all(|&t| near(t, rest)),
            "rest {rest}, held {held:?}"
        );
        assert!(!thin_scroll().kinetic.rubber_band);
        assert_eq!(
            thin_scroll().bar_width,
            egui::style::ScrollStyle::thin().bar_width
        );
        // The pinned style turns it off for every other scroll area too.
        let mut s = egui::Style::default();
        pin_036_style(&mut s);
        assert!(!s.spacing.scroll.kinetic.rubber_band);
    }

    #[test]
    fn every_combo_box_is_shown_through_show_ui_036() {
        // `ComboBox::show_ui` and `show_index` are the only ways to show one (egui 0.37 `containers/combo_box.rs:237`,
        // `:298`), so a builder returned from a function is caught where it is shown.
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let re = regex::Regex::new(r"\.show_(ui|index)\(").unwrap();
        let mut files = vec![];
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let (mut hits, mut own, mut through) = (vec![], false, 0);
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap();
            through += text.matches(".show_ui_036(").count();
            for (n, line) in text.lines().enumerate() {
                if !re.is_match(line) {
                    continue;
                }
                if f.file_name().is_some_and(|n| n == "look.rs") && line.contains("self.show_ui(") {
                    own = true;
                } else {
                    hits.push(format!("{}:{}", f.display(), n + 1));
                }
            }
        }
        assert!(own, "the scan found no show_ui in Combo036 itself");
        assert!(through > 30, "the scan found {through} show_ui_036 calls");
        assert!(
            hits.is_empty(),
            "combo boxes shown without look::Combo036: {hits:#?}"
        );
    }

    #[test]
    fn every_menu_button_goes_through_menu_button_036() {
        // `Ui::menu_button` and `SubMenuButton` are the ways to draw a submenu entry (egui 0.37 `ui.rs:2927-2931`;
        // `menu_image_button` and `menu_image_text_button` are caught too). A top-level `MenuButton` draws no arrow.
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let re =
            regex::Regex::new(r"\.menu_(image_)?(text_)?button\(|SubMenuButton|SubMenu::").unwrap();
        let mut files = vec![];
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let (mut hits, mut through) = (vec![], 0);
        for f in &files {
            let text = std::fs::read_to_string(f).unwrap();
            if f.file_name().is_some_and(|n| n == "look.rs") {
                continue;
            }
            through += text.matches("look::menu_button_036(").count();
            for (n, line) in text.lines().enumerate() {
                if re.is_match(line) {
                    hits.push(format!("{}:{}", f.display(), n + 1));
                }
            }
        }
        assert!(
            through >= 12,
            "the scan found {through} menu_button_036 calls"
        );
        assert!(
            hits.is_empty(),
            "menu buttons drawn without look::menu_button_036: {hits:#?}"
        );
    }
}
