# egui Animation Standards Reference

The precise values, curves and rules behind the review, plus what to grep for in Rust. Cite these in findings instead of approximating. Distilled from Emil Kowalski's design engineering philosophy, translated to egui / eframe 0.37. Helper names (`ease_out_quint`, `ease_in_out_quart`, `open_t`, `open_t_with`, `Spring`, `pop_in_layer`, `reduce_motion`, `move_dur`, `fade_dur`, `dur::*`) come from EGUI-MOTION.md, which also holds the shared curve and spring tables.

## Contents

- Grep catalog
- Should it animate? (frequency table)
- Easing
- Duration
- Physicality
- Springs
- Interruptibility
- Asymmetric timing
- Performance (repaints and layout)
- Clip rects and layer transforms
- Gestures & drag
- Crossfades
- Stagger
- Accessibility
- Debugging
- Cohesion

## Grep catalog

Run these over `src/` first; every hit is a candidate finding, then judge it against the sections below.

| Pattern | What it usually means | Rule |
| --- | --- | --- |
| `animate_bool(` | Linear curve, global `Style::animation_time` | Easing |
| `animate_bool_with_time(` | Explicit time but still linear | Easing (fine for hold-to-confirm fills) |
| `animate_bool_with_easing(` | Curve chosen, duration still global | Duration |
| `animate_value_with_time(` | Linear, no velocity | Springs, if a drag or flick feeds it |
| `easing::\w+_in\b`, `quadratic_in(`, `cubic_in(` | Ease-in curve on UI | Easing: block |
| `\|t\|` next to an `animate_` call | Easing written as a closure | Name it as a `fn`; capturing closures don't compile |
| `animation_time =` | Global tuning of the built-in container animations (collapse, panels, scroll bars, area fades) | Cohesion: check it stays ~0.1-0.2 |
| `request_repaint()` | Continuous repaint, or redundant with `animate_*` | Performance |
| `request_repaint_after` | Polling loop; fine for clocks, wrong for motion | Performance |
| `frame_count`, `frame +=`, `+= 0.0` constants per frame | Speed tied to frame rate | Performance |
| `stable_dt`, `i.time` | Correct clocks; check the timeline stops repainting | Performance |
| `from_scaling(` | Scale around screen origin unless wrapped in translates | Physicality |
| `0.0..=1.0` near a scale | Scale from 0 | Physicality: block |
| `add_space(`, `allocate_exact_size(`, `default_width(`, `min_size`, `set_width(` with an animated value | Layout animated every frame | Performance |
| `key_pressed(`, `consume_shortcut(` driving an `animate_*` target | Keyboard-initiated motion | Frequency: block |
| `insert_temp(` / `start = now` inside a `clicked()` branch | Timeline that restarts on every toggle | Interruptibility |
| `reduce_motion` (absent) | No reduced-motion handling | Accessibility |
| `hovered()` feeding motion without `has_touch_screen` | Ungated hover | Accessibility |
| `fade_in(` | Built-in Area fade; double fade if you also `multiply_opacity` | Cohesion |
| `Id::new(` , `ui.id()` | Deprecated in 0.37; use `Id::unique("…")` / `ui.scope_id().with("…")` | Hygiene |
| `fn update(&mut self, ctx` in `impl eframe::App` | Pre-0.36 API; won't compile on 0.36 or 0.37 | Hygiene |

## Should it animate? (frequency table)

| Frequency | Decision |
| --- | --- |
| 100+ times/day (keyboard shortcuts, command palette toggle) | No animation. Ever. |
| Tens of times/day (hover effects, list navigation) | Remove or drastically reduce |
| Occasional (modals, side panels, toasts) | Standard animation |
| Rare / first-time (onboarding, feedback, celebrations) | Can add delight |

**Never animate keyboard-initiated actions.** They repeat hundreds of times daily; animation makes them feel slow and disconnected. (Raycast has no open/close animation, which is correct for something used hundreds of times a day.) In egui: if the `bool` passed to `animate_bool_*` flips on `i.key_pressed(…)` or `consume_shortcut`, show the end state directly.

Valid purposes for motion: spatial consistency, state indication, explanation, feedback, preventing jarring change. "It looks cool" on a frequently-seen element is not valid.

## Easing

Decision order:

| Motion | Curve |
| --- | --- |
| Entering or exiting | `ease_out_quint` (strong ease-out; `cubic_out` is the weaker fallback) |
| Moving / morphing on screen | `ease_in_out_quart` (`motion.rs`, shown below) |
| Hover / color change, toasts | `emath::easing::quadratic_out` |
| Constant motion (spinner, progress) | `emath::easing::linear` |
| Default | `ease_out_quint` |

**Never an ease-in curve on UI.** It starts slow, delaying the exact moment the user is watching. Ease-out at 200ms *feels* faster than ease-in at 200ms.

`ctx.animate_bool(id, on)` is linear, and so is `animate_value_with_time`. Linear on a deliberate entrance is too weak. `animate_bool_with_time_and_easing` returns `ease(v)` toward true and `1 - ease(1 - v)` toward false: one ease-out call gives an ease-out enter **and** an ease-out exit (both decelerate), but the value **jumps** when the target flips mid-flight. `open_t` gives the same curve both ways without the jump.

egui has no cubic-bezier. The CSS curves map like this:

| CSS curve | egui |
| --- | --- |
| `cubic-bezier(0.23, 1, 0.32, 1)` strong ease-out | `ease_out_quint` (in `motion.rs`) |
| `cubic-bezier(0.77, 0, 0.175, 1)` strong ease-in-out | `ease_in_out_quart` (below) |
| `cubic-bezier(0.32, 0.72, 0, 1)` drawer | No exact match: `ease_out_quint`, or a `Spring` if the drawer is draggable |

```rust
/// Strong ease-in-out for on-screen movement, close to CSS `cubic-bezier(0.77, 0, 0.175, 1)`.
pub fn ease_in_out_quart(t: f32) -> f32 {
    if t < 0.5 { 8.0 * t.powi(4) } else { 1.0 - (-2.0 * t + 2.0).powi(4) / 2.0 }
}
```

Curves are `fn(f32) -> f32`. Define them once as named `fn`s next to the `dur` tokens; don't hand-roll a new one per call site.

## Duration

| Element | Duration | egui |
| --- | --- | --- |
| Button press feedback | 100-160ms | `dur::PRESS` (0.16) |
| Hover highlight | 150ms | `dur::HOVER` (0.15) |
| Tooltips, small popups | 125-200ms | `dur::POPUP` (0.18) |
| Menus, combo boxes | 150-250ms | `dur::POPUP` (0.18) |
| Modals, side panels | 200-500ms | `dur::PANEL` (0.25) |
| Full-view transitions, explanatory | Can be longer | `dur::PAGE` (0.35) |

**Rule: UI animations stay under 300ms.** A 180ms menu feels more responsive than a 400ms one. Faster spinners make load feel faster (same actual time). Instant tooltips after the first (skip delay and animation) make a toolbar feel faster.

`Style::animation_time` (default 0.2) drives egui's own animations (collapsing headers, panels, scroll bars, window and area fades). Stock widget hover and press visuals change instantly; a hover fade needs `open_t` and custom painting. Tuning `animation_time` changes them all; flag values above 0.3.

## Physicality

- **Never scale from 0.** Start from 0.9-0.97 plus `multiply_opacity(t)`. Nothing in the real world appears from nothing. `pop_in_layer` uses 0.96.
- **Origin-aware popups.** Scale from the trigger, not the screen corner: `T(origin) * S(scale) * T(-origin)`, as `pop_in_layer` does with `btn.rect.center_bottom()` as origin. A bare `TSTransform::from_scaling(s)` scales toward (0, 0) and makes the popup slide from the top-left.
  **Modals are exempt**: they appear centered, so the origin is the modal's own centre.
- **Press feedback.** Draw the pressable at 0.97 scale while `response.is_pointer_button_down_on()`, over `dur::PRESS`. Keep it subtle (0.95-0.98). Applies to any pressable element. The allocation stays fixed; only the drawing shrinks:

```rust
/// Pressable with a 0.97 press scale around its centre. Layout never changes: only the drawing shrinks.
fn press_button(ui: &mut Ui, label: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(120.0, 32.0), egui::Sense::click());
    let down = resp.is_pointer_button_down_on();
    let t = open_t(ui.ctx(), resp.id.with("press"), down, move_dur(ui.ctx(), dur::PRESS));
    // Text is laid out once at 14pt; the transform scales the drawn shapes, not the font size.
    let c = rect.center().to_vec2();
    let xf = egui::emath::TSTransform::from_translation(c)
        * egui::emath::TSTransform::from_scaling(egui::lerp(1.0..=0.97, t))
        * egui::emath::TSTransform::from_translation(-c);
    ui.with_visual_transform(xf, |ui| {
        ui.painter().rect_filled(rect, 6.0, ui.visuals().widgets.inactive.bg_fill);
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label,
            egui::FontId::proportional(14.0), ui.visuals().text_color());
    });
    resp
}
```

## Springs

Springs feel natural because they simulate physics; they have no fixed duration and settle on their parameters. Use them for drag with momentum, "alive" elements, interruptible gestures, and decorative pointer-following. egui has none built in; use `Spring` from `motion.rs`, stored in app state and stepped once per frame.

| Feel | `stiffness` | `damping` |
| --- | --- | --- |
| Snappy UI, no overshoot (`Spring::new` default) | 170 | 26 |
| Gesture release, a little bounce (Framer `bounce: 0.2`) | 158 | 20 |
| Playful, visible bounce (decoration only) | 100 | 10 |

This is the shared table from EGUI-MOTION.md (`k = (2π / response)²`, `c = 2 · (1 − bounce) · √k`); flag configs that drift from it without a reason.

Keep bounce subtle; avoid it in most UI. Springs keep velocity when the target changes (a fixed timeline restarts from zero), so they suit gestures the user may reverse mid-motion.

Pointer-following: step a `Spring` toward the pointer rather than drawing at the raw pointer position (direct = artificial, no momentum). Only when the motion is decorative.

## Interruptibility

`open_t` and `animate_value_with_time` retarget from the current value when the target changes; a hand-rolled timeline restarts from zero if its start time is reset, and raw eased `animate_bool_*` jumps. For anything triggered rapidly (toasts, toggles, press, hover), use `open_t`, a linear `animate_*`, or a `Spring`.

| Pattern | Interruptible? |
| --- | --- |
| `open_t(ctx, id, open, secs)` / `open_t_with` | Yes, continues from the value on screen |
| `animate_bool_with_time_and_easing` / `animate_bool_responsive` | No: the eased value jumps when the target flips mid-flight |
| `animate_value_with_time(id, target, secs)` | Yes, but linear and no velocity |
| `Spring::step(ctx, target)` | Yes, keeps velocity |
| Timeline with `start = now` set on every click | No: flag it |

Hand-rolled timelines are fine for one-shot sequences. Set the start time once, when the event happens, and repaint only while running:

```rust
/// One-shot timeline: repaint only while it runs. `start` is set once, when the event happens.
fn timeline_t(ctx: &Context, start: f64, secs: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    let t = ((now - start) as f32 / secs.max(1e-3)).clamp(0.0, 1.0);
    if t < 1.0 { ctx.request_repaint(); }
    ease_out_quint(t)
}
```

Entry animation on first show: `open_t` and `animate_bool_*` return the target value on their first call, so a popup that is open on the frame it is created does not animate in. Create it closed, or call `open_t` with `false` once before it opens. `Area::fade_in(true)` is the built-in alternative for areas; turn it off (`fade_in(false)`) when you drive the fade yourself.

## Asymmetric timing

Slow where the user is deciding, fast where the system responds. With linear `animate_bool_with_time` the duration is a rate, and `open_t` stores each leg's duration, so changing it per direction works mid-flight in both:

```rust
/// Fill takes 2s while held (deliberate), drains in 200ms on release (the system snaps back).
/// Returns true on the one frame the fill completes.
fn hold_to_delete(ui: &mut Ui) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(160.0, 32.0), egui::Sense::click());
    let held = resp.is_pointer_button_down_on();
    let secs = if held { 2.0 } else { 0.2 };
    // Same (linear) curve both ways, so switching duration mid-hold can't make the fill jump.
    let t = ui.ctx().animate_bool_with_time(resp.id.with("hold"), held, secs);
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, ui.visuals().widgets.inactive.bg_fill);
    // Clip-rect reveal: the fill eats in from the left, like `clip-path: inset()`.
    let fill = egui::Rect::from_min_max(rect.min, egui::pos2(egui::lerp(rect.left()..=rect.right(), t), rect.bottom()));
    painter.with_clip_rect(fill).rect_filled(rect, 6.0, egui::Color32::from_rgb(200, 60, 60));
    painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Hold to delete",
        egui::FontId::proportional(14.0), ui.visuals().strong_text_color());
    // Report completion once, not on every frame the button stays held after filling.
    let done = t >= 1.0;
    let was = ui.ctx().data_mut(|d| std::mem::replace(d.get_temp_mut_or_insert_with(resp.id.with("fired"), || false), done));
    done && !was
}
```

Switching the *curve* per direction mid-flight makes the drawn value jump; switch only the duration.

## Performance (repaints and layout)

egui has no compositor and no GPU-only properties: it re-tessellates every repainted frame. Cost is shapes per frame times how often you repaint. So the two performance rules are:

- **Repaint only while something moves.** All `animate_*` calls already request a repaint while in flight; adding `request_repaint()` next to them is redundant. An unconditional `ctx.request_repaint()` at the top of `fn ui` keeps the app at full frame rate while idle, burning CPU and battery: a block. Hand-rolled timelines and springs repaint only while `t < 1.0` / not settled.
- **Animate drawn values, not layout.** Painter offsets, `transform_layer_shapes` and `multiply_opacity` change only drawing. Animated values fed to `add_space`, `allocate_exact_size`, `default_width`, `min_size` or panel widths re-lay-out every frame and push neighbouring widgets around. Allowed only when the layout change is the effect (a side panel sliding open).

Also flag:

- **Frame-count timing.** `x += 0.05` per frame or a frame counter runs twice as fast at 120 Hz and stalls when egui skips frames (it only repaints on input or request). Use `stable_dt` or `i.time`.
- **Springs or timelines stepped for every row** of a long list, every frame, even when off-screen. Step only visible rows, or drive the whole group from one `t`.
- **`request_repaint_after` loops** used to fake an animation. They are for clocks and polling, not motion.

## Clip rects and layer transforms

- **Clip-rect reveals** replace `clip-path: inset()`: `painter.with_clip_rect(rect)` or `ui.shrink_clip_rect(rect)`, with the rect's edge animated. Uses: reveal-on-open, hold-to-delete fill (above), comparison sliders.
- **`ctx.transform_layer_shapes(layer_id, TSTransform)`** translates and uniformly scales a whole `Area`, `Window` or popup layer without layout. It does not rotate and has no 3D.
- **Offsets relative to size**: offset by a fraction of the element's own rect (`rect.height() * (1.0 - t)`) rather than hard-coded points, so it works at any size.

## Gestures & drag

- **Momentum dismissal**: don't require crossing a distance threshold alone. Read `ctx.input(|i| i.pointer.velocity())` (points per second) on `drag_stopped()` and dismiss above ~0.11 px/ms = 110 points/s. A flick should be enough.
- **Damping at boundaries**: dragging past a natural edge moves less the further you go.
- **Pointer capture** is built in: once a widget is dragged, egui keeps sending it the drag outside its rect. Don't re-implement it with hover checks.
- **Multi-touch protection**: ignore input while `i.multi_touch().is_some()` so a second finger can't make the element jump.
- **Friction over hard stops**: allow over-drag with rising resistance rather than an invisible wall.

```rust
struct Sheet { offset: Spring, dismissed: bool }

fn sheet(ui: &mut Ui, sheet: &mut Sheet) {
    let ctx = ui.ctx().clone();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(300.0, 200.0), egui::Sense::drag());
    // Ignore extra fingers once a drag is running, so the sheet can't jump.
    let multi = ctx.input(|i| i.multi_touch().is_some());
    if resp.drag_stopped() {
        let vy = ctx.input(|i| i.pointer.velocity().y); // points per second
        // A flick is enough: ~0.11 px/ms = 110 points/s. Distance is only the fallback.
        sheet.dismissed = vy > 110.0 || sheet.offset.value > rect.height() * 0.5;
        sheet.offset.velocity = vy; // the spring carries the flick's momentum
    } else if resp.dragged() && !multi {
        let mut dy = resp.drag_delta().y;
        if sheet.offset.value < 0.0 {
            // Past the top edge: rising resistance instead of a hard stop.
            dy /= 1.0 + sheet.offset.value.abs() / 50.0;
        }
        sheet.offset.value += dy;
        sheet.offset.velocity = 0.0;
    } else {
        let target = if sheet.dismissed { rect.height() } else { 0.0 };
        sheet.offset.step(&ctx, target);
    }
    let r = rect.translate(egui::vec2(0.0, sheet.offset.value));
    ui.painter().rect_filled(r, 12.0, ui.visuals().window_fill);
}
```

## Crossfades

egui has no blur filter, so the web trick of blurring a crossfade to hide double exposure is not available. If two states visibly overlap during a crossfade, shorten it, fade the outgoing state out faster than the incoming one fades in, or add a small shared offset so it reads as one movement. Don't fake blur with extra layers.

## Stagger

Stagger group entrances; 30-80ms between items, capped so long lists don't drag (the usage pattern in EGUI-MOTION.md uses 40ms capped at 200ms). Longer delays feel slow. Stagger is decorative: never ignore clicks on a row while its entrance plays.

## Accessibility

egui cannot read the OS reduced-motion flag. The app must offer a setting (`set_reduce_motion`) and every movement must branch on it. Reduced motion means fewer and gentler animations, not zero: keep fades that aid comprehension (`fade_dur` keeps them, capped at 120ms), drop movement and scale (`move_dur` returns 0).

```rust
/// Slide-in content: under reduced motion the fade stays, the movement goes.
fn slide_in(ui: &mut Ui, open: bool, add: impl FnOnce(&mut Ui)) {
    let ctx = ui.ctx().clone();
    let t = open_t(&ctx, ui.scope_id().with("panel"), open, fade_dur(&ctx, dur::PANEL));
    if t == 0.0 { return; }
    let dx = if reduce_motion(&ctx) { 0.0 } else { (1.0 - t) * 24.0 };
    let rect = ui.max_rect().translate(egui::vec2(dx, 0.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.multiply_opacity(t);
        add(ui);
    });
}
```

Gate hover motion on touch screens, where a tap fires a hover:

```rust
/// Hover motion only for a real pointer: touch screens fire a hover on tap.
fn hover_t(ctx: &Context, resp: &egui::Response) -> f32 {
    let touch = ctx.input(|i| i.has_touch_screen());
    open_t(ctx, resp.id.with("hover"), resp.hovered() && !touch, dur::HOVER)
}
```

A reduce-motion implementation that zeroes every duration, including fades and press feedback, is itself a finding.

## Debugging (recommend in reviews when feel is uncertain)

- **Slow motion**: multiply the `dur::*` values by 2-5 in a debug build. Check colors fade cleanly, the curve doesn't stop abruptly, the origin is right, coordinated values stay in sync.
- **Frame-by-frame**: log `t` per frame with `i.time`, or step the app with a fixed `stable_dt` in an `egui_kittest` test if the project uses it.
- **Real hardware** for gestures: test drags on a touch screen or trackpad, not only a mouse.
- **Fresh eyes next day**: imperfections invisible during development surface later.

## Cohesion

Match motion to the component's personality: playful can be bouncier (`emath::easing::back_out`, a softer spring); a professional tool or dashboard should be crisp and fast. Durations and curves belong in one module (`dur::*` and named easing `fn`s); five near-identical literals across files is a finding. Opacity plus offset in entering/exiting lists is trial and error; there's no formula, adjust until it feels right.
