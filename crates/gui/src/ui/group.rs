//! A framed collapsing group whose header row carries a value right-aligned (Marmoset style, layout v2 A3). A group
//! can also carry a value only while it is collapsed ([`group_closed_value`]).
use eframe::egui;
use egui::collapsing_header::CollapsingState;

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

/// The one header drawing: `values` is (`value`, `closed_value`).
fn group_impl<R>(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    title: &str,
    values: (Option<egui::RichText>, Option<egui::RichText>),
    default_open: bool,
    open_now: bool,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    egui::Frame::group(ui.style())
        .show(ui, |ui| {
            // shortcut: the Debug bound of egui ids is dodged by hashing to a u64 first; upgrade to `make_persistent_id(id)`
            // if egui drops the Debug bound or every caller's id type derives Debug.
            let hasher = std::hash::BuildHasherDefault::<std::hash::DefaultHasher>::default();
            let id = ui.make_persistent_id(std::hash::BuildHasher::hash_one(&hasher, id));
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
