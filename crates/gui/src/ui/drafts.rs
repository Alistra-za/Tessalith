//! Buffered editors keep their own values. A replacement prompt settles them through their normal
//! commit paths only after its answer, including fields drawn in another immediate viewport.
use crate::model::{AfterCommit, AppModel, Dialog};
use eframe::egui;

#[derive(Clone, Copy, Default, PartialEq)]
pub(crate) enum Mode {
    #[default]
    Editing,
    Resume,
    Hold,
    Save,
    Discard,
}

#[derive(Clone, Default)]
struct Drafts {
    mode: Mode,
    frame: u64,
    fields: std::collections::HashMap<(egui::ViewportId, egui::Id), u64>,
}

fn id() -> egui::Id {
    egui::Id::unique("project-editor-drafts")
}

pub(crate) fn begin(ctx: &egui::Context, mode: Mode) {
    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<Drafts>(id());
        state.frame += 1;
        state.mode = if state.mode == Mode::Hold && mode == Mode::Editing {
            Mode::Resume
        } else {
            mode
        };
    });
}

pub(crate) fn hold(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<Drafts>(id());
        if matches!(state.mode, Mode::Editing | Mode::Resume) {
            state.mode = Mode::Hold;
        }
    });
}

pub(crate) fn pending(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<Drafts>(id());
        state.fields.retain(|_, seen| *seen == state.frame);
        !state.fields.is_empty()
    })
}

/// Some(true) commits, Some(false) drops the editor's buffer. An untouched field never asks.
pub(super) fn finish(r: &egui::Response, changed: bool, settled: bool) -> Option<bool> {
    if !changed {
        return settled.then(|| !r.ctx.input(|i| i.key_pressed(egui::Key::Escape)));
    }
    let key = (r.ctx.viewport_id(), r.id);
    let focused = r.has_focus();
    let dragged = r.dragged();
    let escape = r.ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let mut resume = false;
    let answer = r.ctx.data_mut(|d| {
        let state = d.get_temp_mut_or_default::<Drafts>(id());
        let was_pending = state.fields.remove(&key).is_some();
        match state.mode {
            Mode::Save => return Some(true),
            Mode::Discard => return Some(false),
            Mode::Resume if was_pending => resume = !focused,
            Mode::Resume if settled => return Some(!escape),
            Mode::Editing if settled || (was_pending && !focused && !dragged) => {
                return Some(!escape);
            }
            _ => {}
        }
        state.fields.insert(key, state.frame);
        None
    });
    if resume {
        r.request_focus();
    }
    answer
}

pub(super) fn text(r: &egui::Response, changed: bool) -> Option<bool> {
    finish(r, changed, r.lost_focus())
}

pub(crate) fn asking(model: &AppModel) -> bool {
    matches!(
        model.dialog,
        Some(
            Dialog::CloseUnsaved
                | Dialog::ReloadUnsaved
                | Dialog::OpenUnsaved(_)
                | Dialog::ExportRunning(_)
        )
    )
}

pub(super) fn answer(ctx: &egui::Context, model: &mut AppModel, save: bool, then: AfterCommit) {
    if model.pending_draft {
        model.draft_answer = Some((save, then));
        ctx.request_repaint();
    } else {
        then(model);
    }
}
