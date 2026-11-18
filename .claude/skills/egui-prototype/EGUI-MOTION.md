# egui motion reference (egui / eframe 0.37)

Verified against egui 0.37.0 source with `cargo check`. If `Cargo.toml` pins another version, check these names with `cargo check` before trusting them.

## On egui 0.36.x

Read `Cargo.toml` first. Three names in this file and in the skills that use it do not exist in egui 0.36.2 (checked against its source; `motion.rs` below uses the first and third, fails `cargo check` there on both, and compiles once they are swapped):

| Written for 0.37 | On 0.36.x write | Why |
|---|---|---|
| `Id::unique("name")` | `Id::new("name")` | `Id::unique` is new in 0.37. `Id::new` is not deprecated on 0.36. |
| `ui.scope_id()` | `ui.id()` | Renamed in 0.37. `Ui::id` is not deprecated on 0.36. |
| `ctx.transform_layer_shapes_after_rounding(layer, xf)` | `ctx.transform_layer_shapes(layer, xf)` | The after-rounding variant is new in 0.37. |

On 0.36.x, `Id::new` and `ui.id()` are correct: never report them as deprecated in a review, audit or plan. Everything else named in this file (`App::ui`, `Panel`, `global_style_mut`, `with_visual_transform`, `set_transform_layer`, `multiply_opacity`, the `animate_*` calls, `has_touch_screen`) exists in both versions.

Contents: API facts · CSS-to-egui translation · Curves · Springs · Helpers (`motion.rs`) · Usage patterns

## API facts that change how you write motion

| Fact | Consequence |
|---|---|
| `ctx.animate_bool(id, on)` is **linear** and uses `Style::animation_time` (global) | Use `open_t` (below) with an explicit duration. |
| `animate_bool_with_time_and_easing` keeps a linear value `v` and returns `ease(v)` toward true, `1 - ease(1 - v)` toward false | One ease-out curve gives an ease-out enter **and** an ease-out exit (both decelerate). But the output **jumps** if the target flips mid-flight (a menu closed during its entrance vanishes in one frame). `animate_bool_responsive` (`cubic_out`) has the same jump. |
| `open_t` / `open_t_with` (below) | Ease-out both ways and continuous on reversal. Use them for anything a user can toggle quickly: popups, toasts, hover, press. Raw `animate_bool_*` with a curve is fine only for things that can't flip mid-flight. |
| The **first** `open_t` / `animate_bool_*` / `animate_value_with_time` call for an id returns the target immediately (no animation) | Something created already open (toast, popup spawned with `open = true`) pops in with no motion. Seed it: call once with `false` on the frame it is created, then `true` from the next frame on. |
| `animate_value_with_time` is **linear** and retargets from the current value (continuous, no velocity) | Fine for linear tweens; use a spring for drags and flicks. |
| All `animate_*` calls request a repaint while in flight | Don't add `request_repaint()` for them. `open_t` and `Spring` request their own; hand-rolled timelines must. |
| Easing params are `fn(f32) -> f32` | Custom curves must be named `fn`s, not closures. `emath::easing` has `cubic_out`, `quadratic_out`, `exponential_out`, `back_out`, `bounce_out`, … |
| No spring, no cubic-bezier, no reduced-motion API in egui | Use the `Spring`, curve and `reduce_motion` helpers below. |
| `Id::new` and `Ui::id` are **deprecated** | `Id::unique("name")` for app-global ids; `ui.scope_id().with("name")` for ids local to a widget (`Ui::id` was renamed `scope_id`). |
| `App::ui(&mut self, ui: &mut Ui, frame)` is the required method; `update` is gone | The old `fn update(&mut self, ctx, frame)` does not compile. |
| Style: `ctx.global_style()`, `global_style_mut`, `set_global_style`, `all_styles_mut`, `set_visuals` | `ctx.style()` / `set_style` examples are outdated. |
| `Style::animation_time` (default 0.2s) drives only `CollapsingHeader`, `Panel` collapse, scroll-bar fades, the `Window` close fade and `Area::fade_in` | Stock widget hover and press visuals (`Button`, `Checkbox`, …) change **instantly**. A hover fade needs `open_t(ctx, resp.id.with("hover"), resp.hovered(), dur::HOVER)` and custom painting. |
| Opacity: `ui.multiply_opacity(t)` / `painter.multiply_opacity(t)` | Fades with no layout work. |
| Transform a whole layer: `ctx.transform_layer_shapes(layer_id, TSTransform)`; `transform_layer_shapes_after_rounding` for one that ends at identity | Translate and uniform scale for an `Area`, `Window` or popup, with no layout change. The after-rounding variant avoids a 1px jump on the last frame. |
| `Area::fade_in(bool)` (on by default, 200ms) | Built-in fade for areas. Turn it off when you drive your own fade, or the two multiply. |
| Frame time: `ctx.input(\|i\| i.stable_dt)`; clock: `i.time` (f64 seconds) | Scale motion by `stable_dt`, never by a fixed frame count. Keep `time` as f64 until you subtract. |

## The CSS-to-egui translation

| Web rule | egui equivalent |
|---|---|
| Animate only `transform` / `opacity` | Animate **drawn** offset, scale and opacity (`painter` offsets, `transform_layer_shapes`, `multiply_opacity`). Never animate values that change layout (`add_space`, widget sizes, panel widths) unless the layout change *is* the effect, such as a panel sliding open. |
| `transition: … 200ms ease-out` | `open_t(ctx, id, on, 0.2)` |
| `@keyframes` / timeline | Store a start `time` in `ctx.data_mut`, compute `t = (now - start) / dur`, ease it, and `request_repaint()` until `t >= 1`. |
| Spring (Framer Motion, `motion`) | `Spring::step(ctx, target)`, stored in app state. |
| `transform-origin` | Build the transform around the trigger point: `T(origin) * S(scale) * T(-origin)`. |
| `prefers-reduced-motion` | The app setting `reduce_motion(ctx)`. egui can't read the OS flag. Default it from the OS through a platform crate if needed, and always offer the toggle. |
| `will-change`, GPU layers, compositor | Not applicable. egui re-tessellates every repainted frame, so cost is shapes per frame plus how often you repaint. |
| `:hover` transition | `response.hovered()` fed into `open_t`, then paint with the result. Stock widgets don't animate hover. |
| `:active` press scale | `response.is_pointer_button_down_on()` fed into `open_t` → draw the content scaled 0.97 around its centre. |

## Curves

One set for every skill. All are `fn(f32) -> f32`.

| Use | Curve | CSS it stands in for |
|---|---|---|
| Enter, exit, default | `ease_out_quint` (`motion.rs`) | `cubic-bezier(0.23, 1, 0.32, 1)` |
| On-screen move or morph A to B | `ease_in_out_quart` (`motion.rs`) | `cubic-bezier(0.77, 0, 0.175, 1)` |
| Hover / colour, toasts, anything soft | `emath::easing::quadratic_out` | `ease` (nearest stock curve) |
| Drawers and sheets | A `Spring` if draggable, else `ease_out_quint` | `cubic-bezier(0.32, 0.72, 0, 1)` (no exact match) |
| Constant motion (spinner, progress) | linear | `linear` |

Don't hand-roll a cubic-bezier solver per project; pick from this table.

## Springs

`Spring` takes stiffness `k` and damping `c` (mass 1). From a response time and bounce: `k = (2π / response)²`, `c = 2 · (1 − bounce) · √k`. Damping ratio `1 − bounce` = 1 means no overshoot.

| Feel | stiffness | damping |
|---|---|---|
| Snappy, no bounce (`Spring::new` default, response ~480ms) | 170 | 26 |
| Gesture release with a little bounce (Framer `{ duration: 0.5, bounce: 0.2 }`) | 158 | 20 |
| Playful, visible bounce (source `{ stiffness: 100, damping: 10 }`; decoration only) | 100 | 10 |

For Apple-style 300–400ms response, use the formula (response 0.35, bounce 0 gives 322 / 36).

## Helpers (`motion.rs`, compiles on eframe 0.37)

```rust
//! Shared egui 0.37 motion helpers. Copy into your app as `motion.rs`.
use eframe::egui::{self, Context, Id, Ui};
use eframe::egui::emath::TSTransform;

/// Durations in seconds. One place to tune motion for the whole app.
#[allow(dead_code)] // tokens: use the ones you need
pub mod dur {
    pub const PRESS: f32 = 0.16;   // button press / toggle feedback
    pub const HOVER: f32 = 0.15;   // hover highlight
    pub const POPUP: f32 = 0.18;   // menus, tooltips, dropdowns
    pub const PANEL: f32 = 0.25;   // side panels, sheets, modals
    pub const PAGE: f32 = 0.35;    // full-view transitions (rare)
    pub const TOAST: f32 = 0.40;   // toasts: softer and slower than UI chrome
}

/// Strong ease-out, close to CSS `cubic-bezier(0.23, 1, 0.32, 1)`.
/// egui easing params are `fn(f32) -> f32`, so curves must be named fns, not closures.
pub fn ease_out_quint(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(5)
}

/// Strong ease-in-out for on-screen moves A to B, close to CSS `cubic-bezier(0.77, 0, 0.175, 1)`.
#[allow(dead_code)]
pub fn ease_in_out_quart(t: f32) -> f32 {
    if t < 0.5 { 8.0 * t.powi(4) } else { 1.0 - (-2.0 * t + 2.0).powi(4) / 2.0 }
}

/// User-facing "reduce motion" switch, stored in egui memory so any code can read it.
pub fn reduce_motion(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(Id::unique("reduce_motion")).unwrap_or(false))
}
pub fn set_reduce_motion(ctx: &Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(Id::unique("reduce_motion"), on));
}

/// Duration to use: 0 when motion is reduced (opacity fades should use `fade_dur`).
pub fn move_dur(ctx: &Context, secs: f32) -> f32 {
    if reduce_motion(ctx) { 0.0 } else { secs }
}
/// Fades stay under reduced motion, just shorter.
pub fn fade_dur(ctx: &Context, secs: f32) -> f32 {
    if reduce_motion(ctx) { secs.min(0.12) } else { secs }
}

/// 0..1 progress toward `open`, ease-out in both directions (enter and exit both decelerate).
/// Reversing mid-flight continues from the value on screen; egui's `animate_bool_*` jumps instead.
pub fn open_t(ctx: &Context, id: Id, open: bool, secs: f32) -> f32 {
    open_t_with(ctx, id, open, secs, ease_out_quint)
}

/// `open_t` with your own curve (e.g. `emath::easing::quadratic_out` for a soft toast).
/// Like egui's, the first call for an `id` returns the target at once.
pub fn open_t_with(ctx: &Context, id: Id, open: bool, secs: f32, ease: fn(f32) -> f32) -> f32 {
    let target = if open { 1.0 } else { 0.0 };
    let (now, dt) = ctx.input(|i| (i.time, i.stable_dt as f64));
    // State: (value at retarget, target, retarget time, duration of that leg). Storing the
    // duration lets each direction use its own (`if held { 2.0 } else { 0.2 }`) without a jump.
    let tween = |(from, to, start, secs): (f32, f32, f64, f32)| {
        let k = if secs <= 0.0 { 1.0 } else { ((now - start) as f32 / secs).clamp(0.0, 1.0) };
        (egui::lerp(from..=to, ease(k)), k)
    };
    let mut s = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || (target, target, f64::NEG_INFINITY, secs)));
    if s.1 != target {
        // Start one frame in, so the toggle frame already moves.
        s = (tween(s).0, target, now - dt, secs);
        ctx.data_mut(|d| d.insert_temp(id, s));
    }
    let (shown, k) = tween(s);
    if k < 1.0 { ctx.request_repaint(); }
    shown
}

/// Spring for values that must keep velocity when retargeted (drag release,
/// follow-the-pointer). Store it in your app state, call `step` each frame.
/// stiffness/damping: 170/26 snappy, no bounce (default); 158/20 a little bounce; 100/10 playful.
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub value: f32,
    pub velocity: f32,
    pub stiffness: f32,
    pub damping: f32,
}
impl Spring {
    /// Default: stiffness 170, damping 26 (critically damped, response ~480ms, no overshoot).
    pub fn new(value: f32) -> Self {
        Self { value, velocity: 0.0, stiffness: 170.0, damping: 26.0 }
    }
    /// Advance toward `target`. Returns the new value and requests a repaint while moving.
    pub fn step(&mut self, ctx: &Context, target: f32) -> f32 {
        if reduce_motion(ctx) {
            self.value = target;
            self.velocity = 0.0;
            return target;
        }
        // Sub-step so a long frame (e.g. after the window was hidden) can't explode.
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let steps = (dt / (1.0 / 240.0)).ceil().max(1.0) as usize;
        let h = dt / steps as f32;
        for _ in 0..steps {
            let force = -self.stiffness * (self.value - target) - self.damping * self.velocity;
            self.velocity += force * h;
            self.value += self.velocity * h;
        }
        if (self.value - target).abs() < 0.01 && self.velocity.abs() < 0.01 {
            self.value = target;
            self.velocity = 0.0;
        } else {
            ctx.request_repaint();
        }
        self.value
    }
}

/// Pop-in for a floating layer (Area/Window/popup): scale 0.96 -> 1 around `origin`
/// plus fade. Never scale from 0. Call after the layer is drawn, with its LayerId.
pub fn pop_in_layer(ui: &Ui, layer: egui::LayerId, origin: egui::Pos2, t: f32) {
    let scale = if reduce_motion(ui.ctx()) { 1.0 } else { egui::lerp(0.96..=1.0, t) };
    // Scale around `origin` (e.g. the trigger button), not the screen corner.
    let xf = TSTransform::from_translation(origin.to_vec2())
        * TSTransform::from_scaling(scale)
        * TSTransform::from_translation(-origin.to_vec2());
    // After-rounding variant: the last frame lands exactly on the untransformed pixels.
    ui.ctx().transform_layer_shapes_after_rounding(layer, xf);
}
```

## Usage patterns (compiled in the same crate)

```rust
impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let mut rm = reduce_motion(&ctx);
        if ui.checkbox(&mut rm, "Reduce motion").changed() { set_reduce_motion(&ctx, rm); }

        let btn = ui.button("Menu");
        if btn.clicked() { self.menu_open = !self.menu_open; }

        // Popup: fade + small scale from the trigger.
        let t = open_t(&ctx, egui::Id::unique("menu"), self.menu_open, fade_dur(&ctx, dur::POPUP));
        if t > 0.0 {
            let area = egui::Area::new(egui::Id::unique("menu_area"))
                .fixed_pos(btn.rect.left_bottom() + egui::vec2(0.0, 4.0))
                .order(egui::Order::Foreground)
                .fade_in(false) // we drive the fade; the built-in one would double it
                .show(&ctx, |ui| {
                    ui.multiply_opacity(t);
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label("Copy");
                        ui.label("Paste");
                    });
                });
            pop_in_layer(ui, area.response.layer_id, btn.rect.center_bottom(), t);
        }

        // Spring-driven value (keeps velocity when the target changes).
        if ui.button("Toggle panel").clicked() { self.panel_open = !self.panel_open; }
        let s = self.drag.get_or_insert(Spring::new(0.0));
        let x = s.step(&ctx, if self.panel_open { 200.0 } else { 0.0 });
        let (rect, _) = ui.allocate_exact_size(egui::vec2(260.0, 30.0), egui::Sense::hover());
        ui.painter().rect_filled(egui::Rect::from_min_size(rect.min + egui::vec2(x, 0.0), egui::vec2(60.0, 30.0)), 6.0, ui.visuals().selection.bg_fill);

        // Staggered list entrance: 40ms between items, capped so long lists don't drag.
        if self.items.is_empty() { self.items = (0..5).map(|i| format!("Item {i}")).collect(); }
        let now = ctx.input(|i| i.time); // keep f64 until the subtraction
        let start = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::unique("list_start"), || now));
        for (i, item) in self.items.iter().enumerate() {
            let delay = (i as f32 * 0.04).min(0.2);
            let k = (now - start) as f32 - delay;
            // Fade and movement are timed separately: reduced motion keeps the fade, drops the rise.
            let fade = ease_out_quint((k / fade_dur(&ctx, dur::POPUP)).clamp(0.0, 1.0));
            let m = move_dur(&ctx, dur::POPUP);
            let rise = if m > 0.0 { ease_out_quint((k / m).clamp(0.0, 1.0)) } else { 1.0 };
            if fade < 1.0 || rise < 1.0 { ctx.request_repaint(); }
            let xf = egui::emath::TSTransform::from_translation(egui::vec2(0.0, 8.0 * (1.0 - rise)));
            ui.with_visual_transform(xf, |ui| {
                ui.multiply_opacity(fade);
                ui.label(item);
            });
        }
    }
}
```

Every `run_native` app closure must return `Ok(Box::new(app))`. Window size and title live in `NativeOptions { viewport: egui::ViewportBuilder::default().with_inner_size([w, h]), .. }`.
