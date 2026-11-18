# Animation Recipes (egui / eframe 0.37)

Ready-to-build implementations for the cases that come up most. Start from the recipe, then adapt; don't rebuild from scratch.

Contents:
- Shared imports and `scale_around`
- Button press
- Menu, popover, dropdown
- Tooltip
- Modal
- Side panel
- Toast
- Accordion / collapse
- Stagger a group entrance
- Hold to confirm
- Tab indicator with a colour transition
- Drag to dismiss, spring release
- Number and progress change
- Not available in egui

Every recipe compiles against eframe 0.37 with the `motion.rs` helpers (`dur`, `ease_out_quint`, `ease_in_out_quart`, `reduce_motion`, `move_dur`, `fade_dur`, `open_t`, `open_t_with`, `Spring`, `pop_in_layer`) in the same crate. Curves and spring numbers are the shared set in EGUI-MOTION.md.

---

## Shared imports and `scale_around`

```rust
use crate::motion::*;
use eframe::egui::{self, emath::TSTransform, Id, Pos2, Ui};

/// Scale `s` around `origin` (the trigger, or the centre for modals).
pub fn scale_around(origin: Pos2, s: f32) -> TSTransform {
    TSTransform::from_translation(origin.to_vec2())
        * TSTransform::from_scaling(s)
        * TSTransform::from_translation(-origin.to_vec2())
}
```

---

## Button press

Any pressable element. Instant feedback that the interface heard the user.

```rust
/// `ui.button` that scales to 0.97 while held. Keeps egui's own button (focus, keyboard, a11y).
/// `id_salt` must be unique among siblings (two "OK" buttons need different salts).
pub fn press_button(ui: &mut Ui, id_salt: impl egui::AsIdSalt, text: &str) -> egui::Response {
    let ctx = ui.ctx().clone();
    let id = ui.scope_id().with(id_salt).with("press");
    // The rect and press state exist only after the button is added, so read last frame's.
    let (down, center) = ctx.data(|d| d.get_temp::<(bool, Pos2)>(id)).unwrap_or((false, Pos2::ZERO));
    let t = open_t(&ctx, id.with("t"), down, move_dur(&ctx, dur::PRESS)); // a quick tap reverses mid-flight
    let resp = ui.with_visual_transform(scale_around(center, egui::lerp(1.0..=0.97, t)), |ui| ui.button(text)).inner;
    let now_down = resp.is_pointer_button_down_on();
    if now_down != down {
        ctx.request_repaint(); // start the animation next frame even if the pointer stays still
    }
    ctx.data_mut(|d| d.insert_temp(id, (now_down, resp.rect.center())));
    resp
}
```

`with_visual_transform` scales everything drawn inside, so the label comes along, which is what makes it read as a physical press. It moves pixels only; the hit area stays put, which is what you want for a 3% squeeze. The one-frame lag is invisible.

No hover gating needed: a press is a real press on touch.

---

## Menu, popover, dropdown

Scales out of its trigger, not out of thin air. Prefer egui's own `ui.menu_button` / `ComboBox` / `Popup` unless you need the pop-in; this is the hand-built version for when you do.

```rust
#[derive(Default)]
pub struct Menu {
    open: bool,
}
impl Menu {
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let btn = ui.button("Edit");
        if btn.clicked() {
            self.open = !self.open;
        }
        // Call every frame, even while closed: the first call snaps straight to the target.
        let t = open_t(&ctx, ui.scope_id().with("menu_t"), self.open, fade_dur(&ctx, dur::POPUP));
        if t == 0.0 {
            return;
        }
        let area = egui::Area::new(ui.scope_id().with("menu"))
            .fixed_pos(btn.rect.left_bottom() + egui::vec2(0.0, 4.0))
            .order(egui::Order::Foreground)
            .fade_in(false) // we drive opacity ourselves
            .interactable(self.open) // a closing menu can't be clicked
            .show(&ctx, |ui| {
                ui.multiply_opacity(t);
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    if ui.button("Copy").clicked() || ui.button("Paste").clicked() {
                        self.open = false;
                    }
                });
            });
        let esc = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if self.open && (esc || (area.response.clicked_elsewhere() && !btn.clicked())) {
            self.open = false;
        }
        // Grow out of the trigger: scale 0.96 -> 1 around the button's bottom centre.
        pop_in_layer(ui, area.response.layer_id, btn.rect.center_bottom(), t);
    }
}
```

The origin is the whole point: the panel should look like it came out of the thing you clicked. Keyboard-opened menus (a shortcut, 100+ times a day) skip this entirely.

---

## Tooltip

egui's built-in tooltip (`response.on_hover_text(..)`) already has the detail most implementations miss: `Style::interaction.tooltip_delay` (default 500ms) prevents accidental activation, and `tooltip_grace_time` (default 200ms) makes neighbouring tooltips open instantly once one is open. Keep both. If you hand-build a tooltip, use the menu recipe with a shorter duration (125ms) and scale 0.97.

---

## Modal

The one popover that stays centred.

```rust
pub fn confirm_modal(ctx: &egui::Context, open: &mut bool) {
    let t = open_t(ctx, Id::unique("confirm_t"), *open, fade_dur(ctx, dur::PANEL));
    if t == 0.0 {
        return;
    }
    let scale = if reduce_motion(ctx) { 1.0 } else { egui::lerp(0.96..=1.0, t) };
    let id = Id::unique("confirm");
    let resp = egui::Modal::new(id)
        .area(egui::Modal::default_area(id).fade_in(false))
        .backdrop_color(egui::Color32::from_black_alpha((120.0 * t) as u8)) // backdrop fades with it
        .frame(egui::Frame::NONE) // drawn inside the transform below instead
        .show(ctx, |ui| {
            ui.multiply_opacity(t);
            // Modals scale from the centre: they are not anchored to a trigger.
            let centre = ui.ctx().content_rect().center();
            ui.with_visual_transform(scale_around(centre, scale), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label("Archive 3 files?");
                    ui.button("Archive").clicked()
                })
                .inner
            })
            .inner
        });
    // `Modal` only reports Escape and backdrop clicks; closing is up to you.
    if resp.inner || resp.should_close() {
        *open = false;
    }
}
```

The backdrop alpha follows `t` so the dialog and backdrop read as one surface. The frame is moved inside the transform; otherwise `Modal` draws it outside your scale and fade.

---

## Side panel

Built in. Don't hand-roll it.

```rust
pub fn side_panel(ui: &mut Ui, open: &mut bool) {
    egui::Panel::left("nav").show_collapsible(ui, open, |ui| {
        ui.label("Navigation");
    });
}
```

`show_collapsible` slides the panel and reflows its neighbours, which is the one place a layout change is the effect. It uses `animate_bool_responsive` (cubic ease-out) with the global `Style::animation_time`; raising that also slows collapsing headers, scroll-bar fades and area fades, so leave it near the default. Use `show_switched` to animate between a thin and a wide panel.

For a sheet that overlaps content instead of pushing it, use the toast pattern (translate by `(1.0 - t) * height`), or the drag recipe below if it can be dragged.

---

## Toast

```rust
pub struct Toast {
    pub text: String,
    pub open: bool,
    pub shown_at: f64,
}
pub fn toast_ui(ctx: &egui::Context, toast: &mut Toast) {
    let now = ctx.input(|i| i.time);
    if toast.open {
        let left = 4.0 - (now - toast.shown_at);
        if left <= 0.0 {
            toast.open = false;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(left)); // wake up to dismiss
        }
    }
    // Softer curve and slower than UI chrome: a toast is not on the critical path.
    let t = open_t_with(ctx, Id::unique("toast_t"), toast.open, fade_dur(ctx, dur::TOAST), egui::emath::easing::quadratic_out);
    if t == 0.0 {
        return;
    }
    let area = egui::Area::new(Id::unique("toast"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
        .order(egui::Order::Foreground)
        .fade_in(false)
        .interactable(toast.open)
        .show(ctx, |ui| {
            ui.multiply_opacity(t);
            egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(&toast.text));
        });
    // Enter from below, leave the same way: offset by the hidden part of its own height + margin.
    let dy = if reduce_motion(ctx) { 0.0 } else { (1.0 - t) * (area.response.rect.height() + 16.0) };
    ctx.transform_layer_shapes(area.response.layer_id, TSTransform::from_translation(egui::vec2(0.0, dy)));
}
```

- 400ms (`dur::TOAST`) with `quadratic_out`, softer and slower than the generic UI budget: a toast's motion is tuned to its personality. `quadratic_out` is the nearest stock egui curve to CSS `ease`; there is no exact match.
- `open_t_with` retargets from the value on screen, so a toast re-triggered mid-exit turns around instead of restarting or jumping.
- Call `toast_ui` every frame from app start, so the first `open_t_with` call happens while closed and the entrance plays.

When toasts stack and the list reflows, the fade has to work against the height change. There's no formula for that pair: adjust until it feels right, then check it again the next day.

---

## Accordion / collapse

Built in: `egui::CollapsingHeader::new("Details").show(ui, |ui| { … })`, or `egui::collapsing_header::CollapsingState` for a custom header. Both animate the open height with `Style::animation_time`.

This is one of the few animations that costs layout on every frame (everything below it moves), so keep it short. Don't hand-roll one by animating `add_space` or `set_max_height`.

---

## Stagger a group entrance

For a list or grid the user sees occasionally, not one they scroll past all day.

```rust
pub fn staggered_list(ui: &mut Ui, items: &[String]) {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    // Remove this key to replay the entrance for a new list.
    let start = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(ui.scope_id().with("stagger_start"), || now));
    for (i, item) in items.iter().enumerate() {
        let delay = (i as f64 * 0.04).min(0.2); // 40ms apart, capped so item 50 isn't waiting
        let k = (now - start - delay) as f32;
        // Reduced motion keeps the fade (shorter) and drops the rise.
        let fade = ease_out_quint((k / fade_dur(&ctx, 0.3)).clamp(0.0, 1.0));
        let rise = if reduce_motion(&ctx) { 1.0 } else { ease_out_quint((k / 0.3).clamp(0.0, 1.0)) };
        if fade < 1.0 || rise < 1.0 {
            ctx.request_repaint(); // hand-rolled timeline: egui won't repaint for us
        }
        let lift = TSTransform::from_translation(egui::vec2(0.0, (1.0 - rise) * 8.0));
        ui.with_visual_transform(lift, |ui| {
            ui.multiply_opacity(fade);
            ui.label(item); // still clickable while it fades in: stagger never blocks input
        });
    }
}
```

Each item fades in and rises 8px over 300ms, 40ms apart (stay within 30–80ms). Time stays `f64` until the subtraction so precision holds in a long-running app. Stagger is decorative: it must never block interaction while it plays.

---

## Hold to confirm

For destructive actions where a plain click is too easy to fire by accident.

```rust
/// Returns true once per hold, when it completes. Keep `progress` and `fired` in app state.
pub fn hold_to_confirm(ui: &mut Ui, progress: &mut f32, fired: &mut bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(160.0, 32.0), egui::Sense::click());
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    let held = resp.is_pointer_button_down_on();
    if held && !*fired {
        *progress = (*progress + dt / 2.0).min(1.0); // press: 2s linear, progress doesn't ease
    } else if !held {
        *fired = false; // re-arm on release
        *progress *= (-25.0 * dt).exp(); // release: decays out in ~200ms, fast start
        if *progress < 0.001 {
            *progress = 0.0;
        }
    }
    // Fire once; the fill stays full until release instead of looping while held.
    let done = !*fired && *progress >= 1.0;
    if done {
        *fired = true;
    }
    if *progress > 0.0 && !*fired {
        ui.ctx().request_repaint();
    }
    let press = open_t(ui.ctx(), resp.id.with("press"), held, move_dur(ui.ctx(), dur::PRESS));
    let p = *progress;
    ui.with_visual_transform(scale_around(rect.center(), egui::lerp(1.0..=0.97, press)), |ui| {
        let painter = ui.painter();
        let font = egui::TextStyle::Button.resolve(ui.style());
        painter.rect_filled(rect, 6.0, ui.visuals().widgets.inactive.bg_fill);
        let fill = egui::Rect::from_min_max(rect.min, egui::pos2(egui::lerp(rect.x_range(), p), rect.max.y));
        painter.with_clip_rect(fill).rect_filled(rect, 6.0, ui.visuals().error_fg_color);
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Hold to delete", font, ui.visuals().text_color());
    });
    done
}
```

Linear is correct for the hold: the fill is a progress indicator, and progress shouldn't ease. The clip rect is egui's `clip-path: inset(…)`. The `fired` latch makes it fire once per hold: the fill stays full until the pointer is released, then drains.

---

## Tab indicator with a colour transition

Timing a background slide and a separate text-colour change never quite lands. Clip instead: draw the active style (fill and text) once, clipped to the moving pill, so both change in perfect sync.

```rust
#[derive(Default)]
pub struct Tabs {
    active: usize,
    pill: Option<(Spring, Spring)>, // left edge, width
}
impl Tabs {
    pub fn ui(&mut self, ui: &mut Ui, labels: &[&str]) {
        let ctx = ui.ctx().clone();
        let rects: Vec<egui::Rect> = ui
            .horizontal(|ui| {
                let mut rects = Vec::new();
                for (i, label) in labels.iter().enumerate() {
                    let r = ui.add(egui::Button::new(*label).frame(false));
                    if r.clicked() {
                        self.active = i;
                    }
                    rects.push(r.rect);
                }
                rects
            })
            .inner;
        let target = rects[self.active];
        // Start at the active tab, not at x = 0.
        let (x, w) = self.pill.get_or_insert_with(|| (Spring::new(target.left()), Spring::new(target.width())));
        let pill = egui::Rect::from_min_size(
            egui::pos2(x.step(&ctx, target.left()), target.top()),
            egui::vec2(w.step(&ctx, target.width()), target.height()),
        );
        // Clip trick: draw the active style once, clipped to the pill, so fill and text colour move together.
        let painter = ui.painter().with_clip_rect(pill);
        let font = egui::TextStyle::Button.resolve(ui.style());
        painter.rect_filled(pill, 6.0, ui.visuals().selection.bg_fill);
        for (r, label) in rects.iter().zip(labels) {
            painter.text(r.center(), egui::Align2::CENTER_CENTER, *label, font.clone(), ui.visuals().selection.stroke.color);
        }
    }
}
```

A critically damped spring (the `Spring::new` default) instead of a 250ms ease-in-out: same feel, and rapid tab clicks retarget smoothly. Tabs switched by keyboard shortcut many times a day can skip the slide: snap the springs with `x.value = target.left()`.

---

## Drag to dismiss, spring release

The gesture recipe. Springs, not durations, because the user can reverse mid-motion.

```rust
const SHEET_H: f32 = 240.0;
pub struct Sheet {
    pub open: bool,
    y: Spring, // 0 = fully open, SHEET_H = off-screen
}
impl Default for Sheet {
    fn default() -> Self {
        Self { open: false, y: Spring::new(SHEET_H) }
    }
}
pub fn sheet_ui(ctx: &egui::Context, s: &mut Sheet) {
    if !s.open && s.y.value >= SHEET_H {
        return; // fully closed
    }
    let area = egui::Area::new(Id::unique("sheet"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::Vec2::ZERO)
        .order(egui::Order::Foreground)
        .fade_in(false)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .show(ui, |ui| {
                    ui.set_min_size(egui::vec2(360.0, SHEET_H));
                    ui.add(egui::Label::new("Drag down to close").sense(egui::Sense::drag()))
                })
                .inner
        });
    let handle = area.inner;
    // A second finger is a pinch, not a new drag: ignore it so the sheet doesn't jump.
    if handle.dragged() && ctx.input(|i| i.multi_touch().is_none()) {
        // Past the open edge: resistance rises the further you pull (rubber band, not a wall).
        let k = if s.y.value < 0.0 { 1.0 / (1.0 - s.y.value / 40.0) } else { 1.0 };
        s.y.value += handle.drag_delta().y * k;
        s.y.velocity = 0.0;
    } else {
        if handle.drag_stopped() {
            let vy = ctx.input(|i| i.pointer.velocity().y); // points per second
            // Decide both ways: a flick (0.11 px/ms = 110 pt/s) wins, else distance.
            // A closing sheet grabbed and flicked back up stays open.
            s.open = if vy > 110.0 { false } else if vy < -110.0 { true } else { s.y.value < SHEET_H * 0.5 };
            s.y.velocity = vy; // the spring keeps the flick's speed
        }
        s.y.step(ctx, if s.open { 0.0 } else { SHEET_H });
    }
    // set_transform_layer moves input too, so the handle stays under the finger.
    ctx.set_transform_layer(area.response.layer_id, TSTransform::from_translation(egui::vec2(0.0, s.y.value)));
}
```

Four details that separate a good drag from a bad one:

- **Capture**: egui keeps delivering `dragged()` to the widget that started the drag even when the pointer leaves it. Nothing to add.
- **Multi-touch protection**: ignore the drag while `multi_touch()` is `Some`, or switching fingers mid-drag makes the element jump.
- **Damping past boundaries**: dragging beyond the natural edge moves the element less the further it goes. Real things slow before they stop.
- **Friction, not a wall**: allow the over-drag with rising resistance rather than refusing it.

Open it by setting `open = true`; the same spring slides it up. Dismiss on a flick, not just on distance: a short fast swipe is a clear intent. The release decides in both directions, so a closing sheet the user grabs and flicks back up stays open.

---

## Number and progress change

```rust
pub fn progress_and_count(ui: &mut Ui, value: f32, count: u32) {
    let ctx = ui.ctx().clone();
    // Progress is linear (it's a measurement) and retargets from where it is.
    let shown = ctx.animate_value_with_time(ui.scope_id().with("progress"), value, move_dur(&ctx, 0.3));
    ui.add(egui::ProgressBar::new(shown));
    let n = ctx.animate_value_with_time(ui.scope_id().with("count"), count as f32, move_dur(&ctx, 0.4));
    ui.monospace(format!("{:>6}", n.round() as u32)); // fixed-width digits: no jitter while counting
}
```

Monospace is egui's tabular numbers: digits keep their width so the value doesn't shift while it counts. Only tick numbers the user is watching change (a total after an action); a value refreshed every second should just update.

---

## Not available in egui

| Web recipe | egui status |
| --- | --- |
| Blur to mask a crossfade that won't settle | No blur or filters in egui. Shorten the overlap instead: fade the old content out over the first ~40% of the duration and the new one in over the rest. |
| Scroll reveal (`IntersectionObserver`) | No observer. If you really need it (onboarding only), check `ui.is_rect_visible(rect)` and store a "revealed" flag so it fires once. |
| Programmatic animation without a library (WAAPI) | Everything is programmatic in egui; use the mechanism table in SKILL.md. |
