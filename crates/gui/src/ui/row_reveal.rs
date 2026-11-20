//! Review E10: the outline Go to set draws on the slot row it lands on: full strength for `REVEAL_SECS`, then a short
//! fade, or none when motion is reduced (`ui::motion`). The set editor takes the reveal (`take`), each slot row asks
//! whether it is the one (`on_row`), and `end` asks for the next frame. The jump to the row is instant either way;
//! only the outline's opacity changes, and only when motion is not reduced.
use crate::model::nav::{Nav, SlotTarget};
use crate::model::sets::SlotKind;
use eframe::egui;

/// The outline holds at full strength this long.
pub const REVEAL_SECS: f64 = 2.0;
/// Then fades out over this long (not when motion is reduced).
pub const REVEAL_FADE_SECS: f64 = 0.12;

/// The reveal in progress: the set it is for, what it outlines, when it started, and whether its row was scrolled to.
#[derive(Clone)]
struct Reveal {
    set_id: String,
    target: SlotTarget,
    start: f64,
    scrolled: bool,
}

fn state_id() -> egui::Id {
    egui::Id::unique("set-editor-reveal")
}

/// What `on_row` outlined, its strength and the pass it drew in (`revealed`).
fn shown_id() -> egui::Id {
    egui::Id::unique("set-editor-revealed")
}

/// The outline's strength `elapsed` seconds after Go to set.
pub fn reveal_alpha(elapsed: f64, reduced: bool) -> f32 {
    if elapsed < REVEAL_SECS {
        return 1.0;
    }
    if reduced {
        return 0.0;
    }
    (1.0 - (elapsed - REVEAL_SECS) / REVEAL_FADE_SECS).clamp(0.0, 1.0) as f32
}

/// The target outlined in the last frame and its strength; `None` when nothing was outlined. For tests.
pub fn revealed(ctx: &egui::Context) -> Option<(SlotTarget, f32)> {
    let (target, alpha, pass) = ctx.data(|d| d.get_temp::<(SlotTarget, f32, u64)>(shown_id()))?;
    (pass + 1 == ctx.cumulative_pass_nr()).then_some((target, alpha))
}

/// Start a waiting Go to set on the matching target. Its drawn document id follows the outline until expiry.
pub(super) fn take(
    ctx: &egui::Context,
    nav: &mut Nav,
    target: Option<&crate::model::SetTarget>,
    drawn_id: Option<&str>,
) {
    let new = nav
        .reveal_slot
        .take()
        .filter(|r| Some(&r.set) == target)
        .zip(drawn_id);
    // Read before `data_mut`: the context is locked inside it.
    let start = ctx.input(|i| i.time);
    ctx.data_mut(|d| {
        if let Some((r, id)) = new {
            let reveal = Reveal {
                set_id: id.into(),
                target: r.target,
                start,
                scrolled: false,
            };
            d.insert_temp(state_id(), reveal);
        } else if drawn_id.is_none()
            || d.get_temp::<Reveal>(state_id())
                .is_some_and(|r| Some(r.set_id.as_str()) != drawn_id)
        {
            d.remove::<Reveal>(state_id());
        }
    });
}

/// A slot row of set `id` at `rect`, showing `slot` and holding `item`: if it is the reveal's, scroll to it once and
/// draw the outline outside the row's own (the unusable slot's amber one).
pub(super) fn on_row(
    ui: &egui::Ui,
    id: &str,
    slot: SlotKind,
    item: Option<&str>,
    rect: egui::Rect,
) {
    let Some(mut r) = ui.data(|d| d.get_temp::<Reveal>(state_id())) else {
        return;
    };
    let hit = r.set_id == id
        && match &r.target {
            // One card holds both kinds of slot 3 or 7, and shows one of them.
            SlotTarget::Slot(k) => k.shares_card(slot),
            SlotTarget::Item(i) => item == Some(i.as_str()),
        };
    if !hit {
        return;
    }
    if !r.scrolled {
        ui.scroll_to_rect_animation(
            rect,
            Some(egui::Align::Center),
            egui::style::ScrollAnimation::none(),
        );
        r.scrolled = true;
        ui.data_mut(|d| d.insert_temp(state_id(), r.clone()));
    }
    let elapsed = ui.input(|i| i.time) - r.start;
    let alpha = reveal_alpha(elapsed, super::motion::reduced(ui.ctx()));
    if alpha <= 0.0 {
        return;
    }
    let colour = ui.visuals().selection.stroke.color.gamma_multiply(alpha);
    ui.painter().rect_stroke(
        rect.expand(2.0),
        2.0,
        egui::Stroke::new(2.0, colour),
        egui::StrokeKind::Outside,
    );
    let pass = ui.ctx().cumulative_pass_nr();
    ui.data_mut(|d| d.insert_temp(shown_id(), (r.target, alpha, pass)));
}

/// After the slot rows: a reveal whose row was not drawn ends here (Go to set still selected the set), a finished
/// one is dropped, and a held one wakes the frame when it should change: once at the end of the hold, then each
/// frame of the fade. Nothing after that, so the frame goes idle.
pub(super) fn end(ctx: &egui::Context) {
    let Some(r) = ctx.data(|d| d.get_temp::<Reveal>(state_id())) else {
        return;
    };
    let elapsed = ctx.input(|i| i.time) - r.start;
    if !r.scrolled || reveal_alpha(elapsed, super::motion::reduced(ctx)) <= 0.0 {
        ctx.data_mut(|d| d.remove::<Reveal>(state_id()));
    } else if elapsed < REVEAL_SECS {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(
            (REVEAL_SECS - elapsed).max(0.0),
        ));
    } else {
        ctx.request_repaint();
    }
}
