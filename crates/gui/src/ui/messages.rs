//! Item 20 (DECISIONS AG112-AG114): the toasts at the bottom right and the Messages tab. The toasts are drawn once a
//! frame by `SdcApp` for both views; the store is `model::messages`.
use super::status::{AMBER, RED};
use crate::model::messages::{Severity, Toast, ToastAction};
use crate::model::AppModel;
use eframe::egui;
use std::collections::HashSet;
use std::time::Instant;

fn color(s: Severity) -> Option<egui::Color32> {
    match s {
        Severity::Info => None,
        Severity::Warning => Some(AMBER),
        Severity::Error => Some(RED),
    }
}

/// The toasts of the last frame: id and rect (the pointer over one pauses its time).
#[derive(Clone, Default)]
struct Rects(Vec<(u64, egui::Rect)>);

/// Advances and draws the toasts. The Messages log is the bottom panel's tab (`tab`).
pub fn draw(ctx: &egui::Context, model: &mut AppModel) {
    draw_at(ctx, model, Instant::now());
}

/// `draw`'s toast half at clock `now`. A toast shows whole until its time runs out, then goes (Note FM25).
pub fn draw_at(ctx: &egui::Context, model: &mut AppModel, now: Instant) {
    let id = egui::Id::new("toast-rects");
    let last: Rects = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    let pointer = ctx.pointer_hover_pos();
    let hovered: HashSet<u64> = last
        .0
        .iter()
        .filter(|(_, r)| pointer.is_some_and(|p| r.contains(p)))
        .map(|(i, _)| *i)
        .collect();
    model.messages.tick(now, &hovered);
    let toasts: Vec<Toast> = model.messages.toasts().to_vec();
    let mut rects = Rects::default();
    let mut clicked: Option<(u64, ToastAction)> = None;
    if !toasts.is_empty() {
        // shortcut: the anchored Area places itself from last frame's size, so a new toast's first visible frame
        // draws the stack one toast low; `request_discard` would fix it but can handle a click twice. Upgrade if
        // egui sizes anchored Areas in the same pass.
        // Note FC8 (supersedes CA10's anchor): over an open bottom panel they sit above it, off Review's Export.
        let lift =
            super::bottom::open_top(ctx).map_or(0.0, |top| ctx.content_rect().bottom() - top);
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -12.0 - lift))
            .order(egui::Order::Foreground)
            // Clicks pass through, unless a toast has a button (Note FB5).
            .interactable(toasts.iter().any(|t| t.action.is_some()))
            // A toast appears whole: no fade-in.
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_max_width(360.0);
                for t in &toasts {
                    let r = egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let text = |ui: &mut egui::Ui| match color(t.severity) {
                            Some(c) => ui.colored_label(c, &t.text),
                            None => ui.label(&t.text),
                        };
                        let Some(action) = t.action else {
                            text(ui);
                            return;
                        };
                        ui.horizontal(|ui| {
                            text(ui);
                            let b = ui.small_button("Undo");
                            // The header has an Undo too: this one names what it takes back.
                            let name = match action {
                                ToastAction::UndoCrashRestore => "Undo the restored crash edits",
                                ToastAction::UndoDeleteSet(..) => "Undo the deleted set",
                            };
                            b.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name)
                            });
                            if b.clicked() {
                                clicked = Some((t.id, action));
                            }
                        });
                    });
                    rects.0.push((t.id, r.response.rect));
                }
            });
    }
    ctx.data_mut(|d| d.insert_temp(id, rects));
    if let Some((toast, action)) = clicked {
        model.messages.dismiss(toast);
        match action {
            ToastAction::UndoCrashRestore => model.undo_crash_restore(),
            ToastAction::UndoDeleteSet(depth, pushes) => model.undo_delete_set(depth, pushes),
        }
    }
    // A resting toast wakes once, when its time is due. A hovered toast's clock is stopped, so it needs no timer:
    // the pointer leaving is an input event.
    let next = toasts
        .iter()
        .filter(|t| !hovered.contains(&t.id))
        .map(|t| t.left)
        .min();
    if let Some(d) = next {
        ctx.request_repaint_after(d);
    }
}

/// The Messages tab: time, severity and text of the last entries, newest first, and a Clear button.
pub fn tab(ui: &mut egui::Ui, model: &mut AppModel) {
    ui.horizontal(|ui| {
        let n = model.messages.len();
        ui.label(format!("{n} message{}", super::review::plural(n)));
        if ui
            .add_enabled(!model.messages.is_empty(), egui::Button::new("Clear"))
            .clicked()
        {
            model.messages.clear();
        }
    });
    ui.separator();
    egui::ScrollArea::vertical().show(ui, |ui| {
        for e in model.messages.entries().rev() {
            ui.horizontal_wrapped(|ui| {
                ui.weak(&e.time);
                match color(e.severity) {
                    Some(c) => ui.colored_label(c, e.severity.label()),
                    None => ui.label(e.severity.label()),
                };
                ui.label(&e.text);
            });
        }
    });
}

/// The Messages tab's text: "Messages", plus the unread count.
pub fn button_text(model: &AppModel) -> String {
    match model.messages.unread() {
        0 => "Messages".to_string(),
        n => format!("Messages ({n})"),
    }
}
