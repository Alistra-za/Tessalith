//! A framed collapsing group whose header row carries a value right-aligned (Marmoset style, layout v2 A3). A group
//! can also carry a value only while it is collapsed ([`group_closed_value`]).
use eframe::egui;
use egui::collapsing_header::CollapsingState;

/// The group's persistent id under `ui`.
// shortcut: the Debug bound of egui ids is dodged by hashing to a u64 first; upgrade to `make_persistent_id(id)` if egui
// drops the Debug bound or every caller's id type derives Debug.
fn persistent_id(ui: &egui::Ui, id: impl std::hash::Hash) -> egui::Id {
    let hasher = std::hash::BuildHasherDefault::<std::hash::DefaultHasher>::default();
    ui.make_persistent_id(std::hash::BuildHasher::hash_one(&hasher, id))
}

/// `value` sits right-aligned in the header, styled by the caller (weak, or AMBER for a warning). K15: egui toggles
/// on the icon only, so the title toggles too.
pub fn group<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    title: &str,
    value: Option<egui::RichText>,
    default_open: bool,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    group_with(ui, id, title, value, default_open, false, body)
}

/// [`group`], opened now when `open_now` (Note FC3: a Problems row's `Fix path` reveals its field).
pub fn group_with<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    title: &str,
    value: Option<egui::RichText>,
    default_open: bool,
    open_now: bool,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    group_impl(ui, id, title, (value, None), default_open, open_now, body)
}

/// [`group`] for a group whose state shows only in a control inside its own body (C14, Task 10 review): `closed_value`
/// is drawn while the group is really collapsed (the loaded `CollapsingState`, not `default_open`) and not while it is
/// open, where the control is on screen. `value` shows in both states and wins over `closed_value`.
pub fn group_closed_value<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    title: &str,
    values: (Option<egui::RichText>, Option<egui::RichText>),
    default_open: bool,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    group_impl(ui, id, title, values, default_open, false, body)
}

/// The header row's height: the Textures strip's width, and at least `interact_size.y` (the row is `max` of the two).
pub const HEADER_H: f32 = 24.0;
const _: () = assert!(HEADER_H == super::columns::STRIP_WIDTH);

/// What one header shows. `summary` and `summary_hover` appear only while the group is closed (TPC-R1, Note JR6: an
/// open group's rows show the values). `changed` and `problem` are marks, not values, and show open or closed.
pub struct GroupHead<'a> {
    pub title: &'a str,
    pub summary: &'a str,
    pub summary_hover: Option<&'a str>,
    pub changed: bool,
    pub problem: Option<super::status::Status>,
}

pub struct GroupResponse<R> {
    pub header: egui::Response,
    /// The header was clicked, or took Space or Enter, this frame. The caller flips its open state.
    pub toggled: bool,
    /// The body's result while it is shown (open, or still closing).
    pub body: Option<R>,
}

/// The header's AccessKit name: `"{title}, {collapsed|expanded}[, changed][, has a problem][: {summary}]"`. The summary
/// part is written only while closed and not empty (TPC-R1).
pub fn header_name(head: &GroupHead<'_>, open: bool) -> String {
    let mut s = String::with_capacity(head.title.len() + head.summary.len() + 48);
    s.push_str(head.title);
    s.push_str(if open { ", expanded" } else { ", collapsed" });
    if head.changed {
        s.push_str(", changed");
    }
    if head.problem.is_some() {
        s.push_str(", has a problem");
    }
    if !open && !head.summary.is_empty() {
        s.push_str(": ");
        s.push_str(head.summary);
    }
    s
}

/// A group of the texture panel: a full-width header row (arrow, title, summary while closed, marks) over a body. The
/// widget keeps no open state: the caller passes `open` and flips it on `toggled`. The header is drawn first, so in the
/// frame of a toggle the body already follows it (`shown = open != toggled`). Draw it outside any disabled `Ui` and
/// disable only the body: a widget takes its `Ui`'s enabled state (egui 0.37.0 `ui.rs:966`), and navigation is never
/// gated (FK3). `instant`, reduced motion, or a toggle from Space or Enter open and close the body without sliding.
///
/// Runs once per header per frame (Note LH7): the AccessKit name is built inside `widget_info`, which runs only while
/// AccessKit is on, and the summary is the caller's `&str`.
pub fn collapsing_group<R>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    head: &GroupHead<'_>,
    open: bool,
    instant: bool,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> GroupResponse<R> {
    let id = persistent_id(ui, id_salt);
    let height = HEADER_H.max(ui.spacing().interact_size.y);
    let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), height));
    let header = ui.interact(rect, id, egui::Sense::click());
    let toggled = header.clicked();
    // egui turns Space or Enter on a focused click widget into a click (`context.rs`), so a key toggle is a click with
    // the key down.
    let by_key = toggled
        && header.has_focus()
        && ui.input(|i| i.key_pressed(egui::Key::Space) || i.key_pressed(egui::Key::Enter));
    let shown = open != toggled;
    let ctx = ui.ctx().clone();
    let mut state = CollapsingState::load_with_default_open(&ctx, id, shown);
    state.set_open(shown);
    if instant || crate::ui::motion::reduced(&ctx) || by_key {
        // Zero time: the animation lands on `shown` now, so `openness` below (the arrow's and the body's) is final.
        ctx.animate_bool_with_time_and_easing(
            state.id(),
            shown,
            0.0,
            egui::emath::easing::cubic_out,
        );
    }
    paint_header(ui, &header, rect, head, shown, state.openness(&ctx));
    let body = state.show_body_unindented(ui, body).map(|r| r.inner);
    GroupResponse {
        header,
        toggled,
        body,
    }
}

fn paint_header(
    ui: &mut egui::Ui,
    header: &egui::Response,
    rect: egui::Rect,
    head: &GroupHead<'_>,
    shown: bool,
    openness: f32,
) {
    header.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::DisclosureTriangle,
            ui.is_enabled(),
            header_name(head, shown),
        )
    });
    ui.ctx().accesskit_node_builder(header.id, |node| {
        node.set_expanded(shown);
    });
    if !ui.is_rect_visible(rect) {
        return;
    }
    let visuals = ui.visuals();
    if header.hovered() {
        let w = &visuals.widgets.hovered;
        ui.painter()
            .rect_filled(rect, w.corner_radius, w.weak_bg_fill);
    }
    // The arrow, as `columns::strip_sensing` draws its own, turned a quarter turn when open (pointing down).
    let color = if header.hovered() {
        visuals.strong_text_color()
    } else {
        visuals.text_color()
    };
    let galley =
        ui.painter()
            .layout_no_wrap("▶".to_owned(), egui::FontId::proportional(12.0), color);
    let top_left = egui::pos2(rect.left() + HEADER_H / 2.0, rect.center().y) - galley.size() / 2.0;
    ui.painter().add(
        egui::epaint::TextShape::new(top_left, galley, color).with_angle_and_anchor(
            openness * std::f32::consts::FRAC_PI_2,
            egui::Align2::CENTER_CENTER,
        ),
    );
    // The visible edge, as `group_impl` bounds its value: a wider row above widens max_rect, not the panel.
    let inset = ui.spacing().item_spacing.x;
    let left = rect.left() + HEADER_H;
    let right = rect.right().min(ui.clip_rect().right() - inset).max(left);
    let content = egui::Rect::from_min_max(
        egui::pos2(left, rect.top()),
        egui::pos2(right, rect.bottom()),
    );
    let strong = ui.visuals().strong_text_color();
    ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
        // The right side lays out first, so "changed" is never cut; the left side takes what is left and truncates.
        egui::Sides::new()
            .height(rect.height())
            .shrink_left()
            .truncate()
            .show(
                ui,
                |ui| {
                    if let Some(status) = head.problem {
                        super::status::dot(ui, status);
                    }
                    ui.add(
                        egui::Label::new(egui::RichText::new(head.title).strong())
                            .selectable(false),
                    );
                    if !shown && !head.summary.is_empty() {
                        // A truncated label shows its full text on hover by itself; `summary_hover` replaces that.
                        let label = egui::Label::new(egui::RichText::new(head.summary).weak())
                            .truncate()
                            .selectable(false);
                        match head.summary_hover {
                            // TPC-T2-1: an elided summary keeps its full text on hover, above the hover text.
                            Some(hover) => {
                                let r = ui.add(label.show_tooltip_when_elided(false));
                                let width = r.rect.width();
                                r.on_hover_ui(|ui| {
                                    let natural = egui::WidgetText::from(head.summary)
                                        .into_galley(
                                            ui,
                                            Some(egui::TextWrapMode::Extend),
                                            f32::INFINITY,
                                            egui::TextStyle::Body,
                                        )
                                        .size()
                                        .x;
                                    if natural > width + 0.5 {
                                        ui.label(format!("{}\n{hover}", head.summary));
                                    } else {
                                        ui.label(hover);
                                    }
                                });
                            }
                            None => {
                                ui.add(label);
                            }
                        }
                    }
                },
                |ui| {
                    if head.changed {
                        ui.add(
                            egui::Label::new(egui::RichText::new("● changed").color(strong))
                                .selectable(false),
                        );
                    }
                },
            );
    });
    if header.has_focus() {
        ui.painter().rect_stroke(
            rect,
            2.0,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
}

/// The one header drawing: `values` is (`value`, `closed_value`).
pub(super) fn group_impl<R, T: Into<egui::WidgetText>>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    title: &str,
    values: (Option<T>, Option<T>),
    default_open: bool,
    open_now: bool,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    egui::Frame::group(ui.style())
        .show(ui, |ui| {
            let id = persistent_id(ui, id);
            let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
            if open_now {
                state.set_open(true);
            }
            // A click this frame shows from the next one (egui repaints after a toggle).
            let (value, closed_value) = values;
            let value = value.or_else(|| closed_value.filter(|_| !state.is_open()));
            let mut clicked = false;
            let mut h = state.show_header(ui, |ui| {
                // The visible edge: a wider row above widens max_rect (egui unions it into the content), not the panel.
                let inset = ui.spacing().item_spacing.x; // the clip edge cuts the last glyph otherwise
                let right = ui.max_rect().right().min(ui.clip_rect().right() - inset);
                // Placer::set_min_width measures from the cursor, which is past the collapser here.
                ui.set_min_width((right - ui.cursor().min.x).max(0.0));
                let label = egui::Label::new(egui::RichText::new(title).strong())
                    .sense(egui::Sense::click());
                clicked = ui.add(label).clicked();
                // A title wider than the visible width leaves no room: skip the value rather than draw it over the title.
                if let Some(v) = value.filter(|_| ui.cursor().min.x < right) {
                    let mut r = ui.available_rect_before_wrap();
                    r.max.x = right;
                    let layout = egui::Layout::right_to_left(egui::Align::Center);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(r).layout(layout), |ui| {
                        ui.label(v)
                    });
                }
            });
            if clicked {
                h.toggle();
            }
            h.body(body).2.map(|r| r.inner)
        })
        .inner
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    /// (content width, view width) of a 300 px horizontal ScrollArea holding one group, after a few frames.
    /// `closed_only`: the value goes through `group_closed_value` (the group is closed, so it is drawn).
    fn widths(value: bool, closed_only: bool) -> (f32, f32) {
        let ctx = egui::Context::default();
        let mut out = (0.0, 0.0);
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 400.0),
                )),
                ..Default::default()
            };
            let full = ctx.run_ui(input, |ui| {
                let o = egui::ScrollArea::horizontal().show(ui, |ui| {
                    let v = value.then(|| "off".into());
                    if closed_only {
                        super::group_closed_value(ui, "g", "Coat", (None, v), false, |_| {});
                    } else {
                        super::group(ui, "g", "Coat", v, false, |_| {});
                    }
                });
                out = (o.content_size.x, o.inner_rect.width());
            });
            full.drop_without_applying_deltas();
        }
        out
    }

    /// Re-review of I1: the header row filled from the frame's left edge, not from the cursor after the collapser,
    /// so every group was ~18 px wider than its panel and the panel scrolled sideways.
    #[test]
    fn a_group_header_never_widens_its_scroll_area() {
        for (value, closed_only) in [(false, false), (true, false), (true, true)] {
            let (content, view) = widths(value, closed_only);
            assert!(
                content <= view,
                "value {value}, closed only {closed_only}: content {content} > view {view}"
            );
        }
    }
}
