//! The right-hand Advanced drawer (M3a spec §2.5): the `[files]` entries that match nothing (Note JI1 moved the
//! live-encode switch, the checks and the cache limit to the Settings tab). Drawing only: every edit is a call on the
//! model, applied once the drawing is over.
use super::columns::{strip_button, SETS_MIN, STRIP_WIDTH, TEXTURES_MIN};
use crate::model::AppModel;
use eframe::egui;

/// The drawer's narrowest width.
const ADV_MIN: f32 = 260.0;

/// The drawer keeps no state of its own now (Note JI3); the struct goes with the drawer in Task 9.
#[derive(Debug, Default)]
pub struct AdvancedState {}

/// What a control asks of the model.
enum Action {
    RemoveUnmatched(String),
}

/// The drawer: a 24 px "Advanced" strip while closed, the panel while `settings.advanced_open`.
pub fn draw(ui: &mut egui::Ui, model: &mut AppModel, _state: &mut AdvancedState) {
    if !model.settings.advanced_open {
        egui::Panel::right("advanced-strip")
            .resizable(false)
            .exact_size(STRIP_WIDTH)
            .frame(egui::Frame::new().fill(ui.visuals().panel_fill))
            .show(ui, |ui| {
                if strip_button(ui, "Advanced")
                    .on_hover_text("Open the Advanced drawer")
                    .clicked()
                {
                    model.settings.advanced_open = true;
                    model.save_settings();
                }
            });
        return;
    }
    let mut close = false;
    // Drawn before the columns: it leaves them their minimums and the centre its floor (K8, rule §2.3).
    let max = ui.available_width() - TEXTURES_MIN - SETS_MIN - super::centre_floor(ui.ctx(), model);
    let panel = egui::Panel::right("advanced")
        .default_size(380.0)
        .size_range(ADV_MIN..=max.max(ADV_MIN));
    // The user's width survives the cap (a narrow window, an expanded preview) and is what gui.toml saves.
    super::chosen_width_panel(ui, "advanced", 380.0, panel, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Advanced");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close = ui.button("Close").clicked();
            });
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| body(ui, model));
    });
    if close {
        model.settings.advanced_open = false;
        model.save_settings();
    }
}

/// The drawer's sections (also the set panel's Advanced tab, layout v2 C6); applies its edits once they are drawn.
pub fn body(ui: &mut egui::Ui, model: &mut AppModel) {
    let mut actions = vec![];
    // Note JI3, JI4: the texture fields are the texture panel's now (the search fields through its Effort row); Note
    // JI1: the preview, checks and cache settings are the Settings tab's.
    unmatched(ui, model, &mut actions);
    for a in actions {
        match a {
            Action::RemoveUnmatched(k) => model.remove_unmatched(&k),
        }
    }
}

fn section(ui: &mut egui::Ui, name: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(name).strong())
        .default_open(true)
        .show(ui, body);
}

/// A number box named `name`; true once its value is settled and differs from what came in: a drag ended, or typed
/// text was accepted by leaving the box (Enter, Tab or a click away; Esc keeps the old value). Typing "200" is one
/// edit, not three (Note FN2): the text is applied only when the box loses focus (`update_while_editing(false)`).
/// The in-progress value of a drag, or of arrow keys on the focused box, is held in egui memory, so a caller that
/// rebuilds `value` from the model each frame still gets the final value on release or when the box is left. While
/// `busy` (a re-plan is out) a settled value stays on show until the model hands it back (Backlog R2); `scope` (the
/// texture or set the box edits) keys the held values, so they never show in another item's box. The match with what
/// came in is exact `f64` equality: a caller whose model rounds the value would hold it until the plan is idle.
pub(super) fn settled_drag<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    speed: f64,
    range: std::ops::RangeInclusive<T>,
    name: &str,
    busy: bool,
    scope: &str,
) -> bool {
    let id = ui.next_auto_id().with(("settled_drag", scope));
    let came_in = *value;
    let mut shown = came_in;
    let (out, _) = held(ui, id, came_in, busy, true, |ui, v| {
        let r = ui.add(
            egui::DragValue::new(v)
                .speed(speed)
                .range(range)
                .update_while_editing(false),
        );
        let x = v.to_f64();
        r.widget_info(|| {
            let mut i = egui::WidgetInfo::drag_value(r.enabled(), x);
            i.label = Some(name.to_string());
            i
        });
        shown = *v;
        r
    });
    *value = shown;
    out.is_some()
}

/// A widget over a model value that is written once per gesture (L29): the value in flight, and one already sent,
/// live in egui memory under `id`, so a caller that rebuilds `current` from the model each frame still sees the
/// final value. `add` draws the widget on a copy of `current` and returns its response. `Some(value)` on the frame it
/// settles and differs from `current`: a drag ended, or typed text was accepted by leaving the box (Enter, Tab or a
/// click away; Esc keeps the old value), so "200" is one edit, not three (Note FN2) and an arrow key on a focused box
/// is one too (`on_blur`: a number box, whose key presses settle when it is left). A slider (`on_blur` false) settles
/// on each key press, so arrow keys edit live. While `busy` (a re-plan is out) a settled value stays on show until the
/// model hands it back (Backlog R2). The match with `current` is exact `f64` equality: a caller whose model rounds the
/// value would hold it until the plan is idle.
pub(super) fn held<T: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    id: egui::Id,
    current: T,
    busy: bool,
    on_blur: bool,
    add: impl FnOnce(&mut egui::Ui, &mut T) -> egui::Response,
) -> (Option<T>, egui::Response) {
    let cur = current.to_f64();
    let mut pending: Option<(f64, bool)> = ui.data(|d| d.get_temp(id));
    if pending.is_some_and(|(v, sent)| v == cur || (sent && !busy)) {
        pending = None;
    }
    let mut x = pending.map_or(current, |p| T::from_f64(p.0));
    let r = add(ui, &mut x);
    if r.changed() {
        pending = Some((x.to_f64(), false));
    }
    let settled = r.drag_stopped()
        || r.lost_focus()
        || (r.changed() && !r.dragged() && !(on_blur && r.has_focus()));
    let mut out = None;
    if let Some((pv, false)) = pending {
        if settled && pv != cur {
            out = Some(T::from_f64(pv));
            pending = Some((pv, true));
        }
    }
    ui.data_mut(|d| match pending {
        Some(p) => {
            d.insert_temp(id, p);
        }
        None => d.remove::<(f64, bool)>(id),
    });
    (out, r)
}

/// A number widget over a model value that is only written once it settles: the value in flight lives in egui
/// memory under `id` while it is dragged, so the drag follows the pointer and ends in exactly one edit (one undo
/// step). `Some(value)` on the frame it settles (a drag ended, a click, a typed value accepted).
pub(super) fn held_value(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    current: f64,
    add: impl FnOnce(&mut egui::Ui, &mut f64) -> egui::Response,
) -> (egui::Response, Option<f64>) {
    let (out, r) = held(ui, egui::Id::new(id), current, false, false, add);
    (r, out)
}

fn unmatched(ui: &mut egui::Ui, model: &AppModel, actions: &mut Vec<Action>) {
    section(ui, "Unmatched [files] entries", |ui| {
        if model.unmatched_keys.is_empty() {
            ui.weak("Every [files] entry in sdc.toml matches a file.");
            return;
        }
        ui.label(
            egui::RichText::new(
                "These entries name no file under the current target and have no effect.",
            )
            .small()
            .weak(),
        );
        let editable = model.project.as_ref().is_some_and(|p| !p.read_only());
        for k in &model.unmatched_keys {
            ui.horizontal(|ui| {
                ui.label(k);
                if ui
                    .add_enabled(editable, egui::Button::new("Remove"))
                    .clicked()
                {
                    actions.push(Action::RemoveUnmatched(k.clone()));
                }
            });
        }
    });
}

/// Gesture drivers for the number-box wrappers over [`held`] (L29): a typed value and an arrow key each commit once.
#[cfg(test)]
pub(super) mod gesture {
    use eframe::egui::{self, accesskit::Role};
    use egui_kittest::kittest::Queryable as _;

    /// The model value the widget shows, and what it has committed.
    pub struct St {
        pub cur: f64,
        pub commits: Vec<f64>,
    }

    impl St {
        /// The model takes a commit at once, as an idle plan does.
        pub fn take(&mut self, v: Option<f64>) {
            if let Some(v) = v {
                self.cur = v;
                self.commits.push(v);
            }
        }
    }

    fn harness(draw: fn(&mut egui::Ui, &mut St)) -> egui_kittest::Harness<'static, St> {
        let st = St {
            cur: 0.0,
            commits: vec![],
        };
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(300.0, 100.0))
            .build_ui_state(draw, st);
        h.run();
        h
    }

    /// Click the box, type `text`, press Enter: what was committed.
    pub fn typed(draw: fn(&mut egui::Ui, &mut St), role: Role, text: &str) -> Vec<f64> {
        let mut h = harness(draw);
        h.get_by_role(role).click();
        h.run();
        // The cursor opens at the start of the text ("0", "0.00" in a slider's box): Delete clears it.
        for _ in 0..6 {
            h.key_press(egui::Key::Delete);
            h.run();
        }
        h.get_by_role(role).type_text(text);
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        h.run();
        h.state().commits.clone()
    }

    /// Focus the box and press the arrow key, without leaving it: what was committed so far.
    pub fn arrow_held(draw: fn(&mut egui::Ui, &mut St), role: Role, key: egui::Key) -> Vec<f64> {
        let mut h = harness(draw);
        h.get_by_role(role).focus();
        h.run();
        h.key_press(key);
        h.run();
        h.run();
        h.state().commits.clone()
    }

    /// Focus the box, press the arrow key, then leave it with Tab: what was committed.
    pub fn arrow(draw: fn(&mut egui::Ui, &mut St), role: Role, key: egui::Key) -> Vec<f64> {
        let mut h = harness(draw);
        h.get_by_role(role).focus();
        h.run();
        h.key_press(key);
        h.run();
        h.key_press(egui::Key::Tab);
        h.run();
        // A slider keeps the focus through Tab: a click on the draw function's "other" button takes it.
        if let Some(other) = h.query_by_label("other") {
            other.click();
        }
        h.run();
        h.run();
        h.state().commits.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::gesture::{arrow, typed, St};
    use super::*;
    use eframe::egui::accesskit::Role;

    fn settled(ui: &mut egui::Ui, s: &mut St) {
        let mut v = s.cur;
        let done = settled_drag(ui, &mut v, 1.0, 0.0..=255.0, "fill", false, "t");
        s.take(done.then_some(v));
    }

    fn held_box(ui: &mut egui::Ui, s: &mut St) {
        let (_, done) = held_value(ui, "t", s.cur, |ui, x| {
            ui.add(egui::DragValue::new(x).update_while_editing(false))
        });
        s.take(done);
    }

    #[test]
    fn settled_drag_commits_a_typed_value_and_an_arrow_key_once() {
        assert_eq!(typed(settled, Role::SpinButton, "25"), vec![25.0]);
        assert_eq!(arrow(settled, Role::SpinButton, egui::Key::ArrowUp), [1.0]);
    }

    /// A number box (`on_blur`) commits nothing while it is focused: the arrow key settles when the box is left.
    #[test]
    fn a_number_box_commits_nothing_before_it_is_left() {
        use super::gesture::arrow_held;
        assert!(arrow_held(settled, Role::SpinButton, egui::Key::ArrowUp).is_empty());
    }

    fn slider_box(ui: &mut egui::Ui, s: &mut St) {
        let (_, done) = held_value(ui, "t", s.cur, |ui, x| {
            ui.add(egui::Slider::new(x, 0.0..=100.0).show_value(false))
        });
        s.take(done);
    }

    /// FY1: a click on a slider settles it, and commits once.
    #[test]
    fn a_click_on_a_slider_commits_once() {
        use egui_kittest::kittest::Queryable as _;
        let st = St {
            cur: 0.0,
            commits: vec![],
        };
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(300.0, 100.0))
            .build_ui_state(slider_box, st);
        h.run();
        h.get_by_role(Role::Slider).click();
        h.run();
        h.run();
        let commits = &h.state().commits;
        assert_eq!(commits.len(), 1, "{commits:?}");
        assert!(commits[0] > 0.0, "{commits:?}");
    }

    #[test]
    fn held_value_commits_a_typed_value_and_an_arrow_key_once() {
        assert_eq!(typed(held_box, Role::SpinButton, "25"), vec![25.0]);
        assert_eq!(arrow(held_box, Role::SpinButton, egui::Key::ArrowUp), [1.0]);
    }
}
