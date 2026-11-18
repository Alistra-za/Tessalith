# Motion in egui: detailed techniques

All snippets use the helpers from `motion.rs` in EGUI-MOTION.md (`ease_out_quint`, `ease_in_out_quart`, `open_t`, `open_t_with`, `Spring`, `move_dur`, `fade_dur`, `reduce_motion`, `dur::*`). Every snippet compiles on eframe 0.37.

## Contents

1. Curves
2. Interruptible motion: retarget, don't restart
3. First frame: seed the animation
4. Springs
5. Press feedback
6. Entry scale and origin
7. Moving by an element's own size
8. Clip reveals (the clip-path family)
9. Crossfades without blur
10. Drag and gestures
11. Stagger
12. Performance
13. Reduced motion
14. Debugging motion

## 1. Curves

egui ships `emath::easing` (`cubic_out`, `quadratic_out`, `exponential_out`, `back_out`, ...) but no cubic-bezier, and the gentle stock curves are often too weak. Every egui skill uses one shared curve set, defined in EGUI-MOTION.md and `motion.rs`:

| Use | Curve |
| --- | --- |
| Enter / exit, UI response | `ease_out_quint` (close to `cubic-bezier(0.23, 1, 0.32, 1)`) |
| On-screen movement or morph | `ease_in_out_quart` (close to `cubic-bezier(0.77, 0, 0.175, 1)`) |
| Drawers and sheets | A `Spring` if draggable, else `ease_out_quint` (the iOS drawer curve has no exact match) |
| Hover, colour, toasts | `emath::easing::quadratic_out` (nearest stock curve to CSS `ease`) |
| Constant motion | linear `t` |

Easing params are `fn(f32) -> f32`, so a curve must be a named `fn`, not a closure. Never use an ease-in curve for UI the user triggered.

## 2. Interruptible motion: retarget, don't restart

The web rule "transitions over keyframes" maps directly:

| Interruptible (use for dynamic UI) | Restarts (avoid for things triggered rapidly) |
| --- | --- |
| `open_t` - flipping the bool continues from the value on screen | A timeline from a stored start time: retriggering jumps back to 0 |
| `animate_value_with_time` - a new target starts from the current value | Raw `animate_bool_with_time_and_easing` with a curve: the eased value jumps on reversal |
| `Spring::step` - also keeps velocity | |

For anything that can be toggled quickly (toasts being added, menus opened and closed, an expanded item closed with Escape mid-animation), use the left column. Keep start-time timelines for one-shot sequences nobody can interrupt.

`animate_value_with_time` has no velocity: a retarget changes direction abruptly. If that shows (drag release, follow-the-pointer), use a spring.

## 3. First frame: seed the animation

The first call of `open_t` (or `animate_bool_*`) for an id has no history, so it returns the target value at once. A popup or toast created already open would pop in with no animation. This is egui's version of the web "mounted" problem. Fix it by calling the same animation with `false` once, on the frame the thing is created:

```rust
if !toast.seeded {
    // The first open_t call snaps to the target. Seed it closed so it animates in.
    open_t(ctx, toast.id, false, dur::TOAST);
    toast.seeded = true;
}
```

When the open bool starts `false` and later flips to `true` (the usual menu case), no seeding is needed.

## 4. Springs

Springs feel natural because they simulate physics and settle from parameters, not a fixed duration. Use them for:

- drags with momentum, and flick-to-dismiss
- things that should feel alive
- gestures the user may reverse mid-motion
- decorative pointer-tracking

Tying a visual directly to the pointer position feels artificial because nothing moves on its own. Feed the target through a spring instead:

```rust
use crate::motion::*;
use eframe::egui::{self, Sense, Ui};

/// Decorative: a card that leans toward the pointer through a soft spring, not 1:1.
pub fn tilt_card(ui: &mut Ui, spring: &mut Spring) {
    spring.stiffness = 100.0;
    spring.damping = 10.0; // bouncy on purpose: decoration only
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(200.0, 120.0), Sense::hover());
    let target = resp.hover_pos().map_or(0.0, |p| (p.x - rect.center().x) * 0.1);
    let dx = spring.step(ui.ctx(), target);
    ui.painter().rect_filled(rect.translate(egui::vec2(dx, 0.0)), 8.0, ui.visuals().widgets.inactive.bg_fill);
}
```

This works because it is **decorative**. In a functional view (a chart in a banking app) no motion would be better. Know when decoration helps and when it hinders.

**Tuning `Spring`.** egui has no duration/bounce spring config, only the helper's `stiffness` and `damping`. Use the shared table from EGUI-MOTION.md:

| Feel | stiffness | damping |
| --- | --- | --- |
| Snappy UI, no overshoot (`Spring::new` default) | 170 | 26 |
| Gesture release, a little bounce (Framer `bounce: 0.2`) | 158 | 20 |
| Playful, visible bounce (decoration only) | 100 | 10 |

Damping near `2 * sqrt(stiffness)` means no overshoot; lower damping means more bounce. Keep bounce subtle in most UI and reserve it for drag-to-dismiss and playful moments.

**Interruptibility.** Springs keep velocity when the target changes; timelines restart from zero. When the user opens an item and quickly presses Escape, a spring reverses smoothly from where it is.

## 5. Press feedback

Any pressable element should scale to about 0.97 while pressed (subtle: 0.95-0.98). It confirms the app heard the press. `Ui::with_visual_transform` scales only the drawing, so the hit area stays put. Scaling also shrinks the label and icon with the frame, which is what you want.

```rust
use crate::motion::*;
use eframe::egui::{self, emath::TSTransform, Pos2, Response, Ui};

/// A normal `ui.button` whose drawing scales to 0.97 while pressed.
/// Uses last frame's press state and centre, so the scale starts one frame late (invisible).
/// `id_salt` must be unique among siblings (two "OK" buttons need different salts).
pub fn press_button(ui: &mut Ui, id_salt: impl egui::AsIdSalt, text: &str) -> Response {
    let key = ui.scope_id().with(id_salt).with("press");
    let ctx = ui.ctx().clone();
    let (down, center) = ctx
        .data(|d| d.get_temp::<(bool, Pos2)>(key))
        .unwrap_or((false, ui.cursor().min));
    let t = open_t(&ctx, key.with("t"), down, move_dur(&ctx, dur::PRESS)); // a quick tap reverses smoothly
    let scale = egui::lerp(1.0..=0.97, t);
    let xf = TSTransform::from_translation(center.to_vec2())
        * TSTransform::from_scaling(scale)
        * TSTransform::from_translation(-center.to_vec2());
    // Visual only: the hit area stays put, so the press can't slip off the edge.
    let resp = ui.with_visual_transform(xf, |ui| ui.button(text)).inner;
    let now_down = resp.is_pointer_button_down_on();
    if now_down != down {
        ctx.request_repaint(); // start the animation next frame
    }
    ctx.data_mut(|d| d.insert_temp(key, (now_down, resp.rect.center())));
    resp
}
```

## 6. Entry scale and origin

- **Never scale from 0.** Nothing in the real world vanishes and reappears completely. Start at 0.95-0.97 (the helper uses 0.96) together with opacity, like a balloon that still has a shape when deflated.
- **Grow popovers from their trigger.** Build the transform around the trigger point: `T(origin) * S(scale) * T(-origin)`; `pop_in_layer` does this for an `Area`, popup or `Window`. Scaling around the layer's own centre or the screen corner is wrong for almost every popover.
- **Exception: modals** are not anchored to a trigger. Scale them around their own centre.

Users rarely notice the difference on a single popover. In the aggregate, unseen details become visible.

## 7. Moving by an element's own size

CSS `translateY(100%)` moves an element by its own height. In egui, use the height of the drawn rect (`area.response.rect.height()`, or last frame's rect if you need it before drawing) and translate the layer with `transform_layer_shapes`. Prefer this to hard-coded pixel offsets: it adapts to content. The toast in COMPONENTS.md enters from below by its own height this way.

## 8. Clip reveals (the clip-path family)

`painter.with_clip_rect(rect)` returns a painter that only draws inside `rect`. Animating that rect is the egui form of animating `clip-path: inset(...)`, and it is cheap.

**Hold-to-delete.** Clip a coloured overlay to a growing rect: 2s linear while held, 200ms ease-out back on release. Slow where the user is deciding, fast where the system responds. Combine with the press scale above.

```rust
use crate::motion::*;
use eframe::egui::{self, Align2, Sense, TextStyle, Ui};

#[derive(Clone, Copy, Default)]
struct Hold {
    p: f32,
    release: Option<(f32, f64)>, // (progress at release, time of release)
    done: bool,                  // fired; wait for release before arming again
}

/// Hold-to-delete: fill sweeps in over 2s linear while held, snaps back in 200ms on release.
/// Returns true once, when the hold completes. Pair it with a keyboard path (e.g. a confirm dialog).
pub fn hold_to_delete(ui: &mut Ui, label: &str) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(140.0, 32.0), Sense::click());
    let ctx = ui.ctx().clone();
    let (now, dt) = ctx.input(|i| (i.time, i.stable_dt.min(0.1)));
    let mut h = ctx.data(|d| d.get_temp::<Hold>(resp.id)).unwrap_or_default();
    let mut fired = false;
    if resp.is_pointer_button_down_on() {
        if !h.done {
            h.release = None;
            h.p = (h.p + dt / 2.0).min(1.0); // slow where the user is deciding
            if h.p >= 1.0 {
                fired = true;
                h = Hold { done: true, ..Hold::default() };
            }
        }
    } else if h.done {
        h.done = false;
    } else if h.p > 0.0 {
        let (from, t0) = *h.release.get_or_insert((h.p, now));
        let k = ((now - t0) as f32 / move_dur(&ctx, 0.2).max(1e-3)).min(1.0);
        h.p = from * (1.0 - ease_out_quint(k)); // fast where the system responds
        if k >= 1.0 {
            h = Hold::default();
        }
    }
    if h.p > 0.0 {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(resp.id, h));

    let v = ui.style().interact(&resp);
    let painter = ui.painter();
    painter.rect_filled(rect, v.corner_radius, v.bg_fill);
    // clip-path: inset(0 X 0 0) -> clip the overlay to the progress rect.
    let mut fill = rect;
    fill.max.x = egui::lerp(rect.min.x..=rect.max.x, h.p);
    painter.with_clip_rect(fill).rect_filled(rect, v.corner_radius, ui.visuals().error_fg_color);
    painter.text(rect.center(), Align2::CENTER_CENTER, label, TextStyle::Button.resolve(ui.style()), v.text_color());
    fired
}
```

**Tabs with a perfect colour switch.** Draw every label in the inactive colour, then draw them all again in the active colour clipped to the moving pill. The text colour flips exactly at the pill's edge, which timing per-label colour fades never achieves.

```rust
use crate::motion::*;
use eframe::egui::{self, Align2, Rect, Sense, TextStyle, Ui};

/// Segmented tabs whose pill slides between tabs, with text colour switching exactly at the pill edge:
/// draw every label in the inactive colour, then draw them all again in the active colour,
/// clipped to the pill rect.
pub fn tabs(ui: &mut Ui, labels: &[&str], selected: &mut usize) {
    let w = 90.0;
    let h = ui.spacing().interact_size.y;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w * labels.len() as f32, h), Sense::hover());
    let cell = |i: usize| Rect::from_min_size(rect.min + egui::vec2(w * i as f32, 0.0), egui::vec2(w, h));
    for i in 0..labels.len() {
        if ui.interact(cell(i), ui.scope_id().with(("tab", i)), Sense::click()).clicked() {
            *selected = i; // a click retargets the pill from wherever it is now
        }
    }
    let ctx = ui.ctx().clone();
    // A spring, not a linear tween: on-screen movement eases, and rapid clicks keep velocity.
    let id = ui.scope_id().with("tab_pill");
    let target = cell(*selected).min.x;
    let mut spring = ctx.data(|d| d.get_temp::<Spring>(id)).unwrap_or(Spring::new(target));
    let x = spring.step(&ctx, target);
    ctx.data_mut(|d| d.insert_temp(id, spring));
    let pill = Rect::from_min_size(egui::pos2(x, rect.min.y), egui::vec2(w, h));

    let vis = ui.visuals().clone();
    let font = TextStyle::Button.resolve(ui.style());
    let painter = ui.painter();
    painter.rect_filled(pill, vis.widgets.active.corner_radius, vis.selection.bg_fill);
    let clipped = painter.with_clip_rect(pill);
    for (i, label) in labels.iter().enumerate() {
        let c = cell(i).center();
        painter.text(c, Align2::CENTER_CENTER, label, font.clone(), vis.widgets.inactive.text_color());
        clipped.text(c, Align2::CENTER_CENTER, label, font.clone(), vis.selection.stroke.color);
    }
}
```

**Reveal on scroll.** Start a reveal when `ui.is_rect_visible(rect)` first turns true; remember that in `ctx.data_mut` so it plays once. Grow the clip rect from the top edge down.

**Comparison slider.** Paint both images over the same rect, the top one through `painter.with_clip_rect` whose right edge follows the drag position. No extra widgets needed.

## 9. Crossfades without blur

On the web, a 2px blur during a crossfade blends old and new states into one. egui has no content blur (`Shadow::blur` only blurs shadows), so don't fake one. When two overlapping states look like two objects:

- fade the old state out faster than the new one fades in, so they overlap less
- add a small scale (0.97 -> 1) to the incoming state
- or don't crossfade: swap instantly and animate only the container

## 10. Drag and gestures

- **Momentum dismissal.** Don't require a distance threshold alone. Check velocity at release: `ctx.input(|i| i.pointer.velocity())` is in points per second. The web threshold of 0.11 px/ms is 110 points/s. A quick flick should be enough.
- **Damping at boundaries.** Past a natural limit, move less the further the user drags. Things in real life slow down before they stop; never a hard wall.
- **Pointer capture is built in.** Once `dragged()` is true, egui keeps delivering the drag to that widget even when the pointer leaves its rect.
- **Hand velocity to the spring** on release so the motion continues the flick instead of restarting.

```rust
use crate::motion::*;
use eframe::egui::{self, Sense, Ui};

/// Rubber band: the further past the limit, the less it moves. Never a hard stop.
fn rubber_band(over: f32, limit: f32) -> f32 {
    over * limit / (over.abs() + limit)
}

/// A bottom sheet you can flick down to dismiss.
pub struct Sheet {
    pub open: bool,
    drag: f32,      // raw pointer travel while dragging
    offset: Spring, // drawn offset; keeps the flick's velocity on release
}
impl Default for Sheet {
    fn default() -> Self {
        Self { open: true, drag: 0.0, offset: Spring::new(0.0) }
    }
}

impl Sheet {
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let height = 200.0;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::drag());
        if resp.drag_started() {
            self.drag = self.offset.value; // grab it where it is, even mid-spring
        }
        if resp.dragged() {
            self.drag += resp.drag_delta().y;
            // Dragging up past the top: friction instead of a wall.
            self.offset.value = if self.drag < 0.0 { rubber_band(self.drag, 40.0) } else { self.drag };
            self.offset.velocity = 0.0;
        } else if resp.drag_stopped() {
            let v = ctx.input(|i| i.pointer.velocity().y); // points per second
            // Decide both ways: a flick (0.11 px/ms = 110 points/s) wins, else half the height.
            self.open = if v > 110.0 { false } else if v < -110.0 { true } else { self.drag < height * 0.5 };
            self.offset.velocity = v; // hand the flick's speed to the spring
            self.drag = 0.0;
        }
        let target = if self.open { 0.0 } else { height };
        let y = if resp.dragged() { self.offset.value } else { self.offset.step(&ctx, target) };

        let sheet = rect.translate(egui::vec2(0.0, y));
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(sheet, ui.visuals().window_corner_radius, ui.visuals().window_fill);
    }
}
```

## 11. Stagger

When several items enter together, delay each by 30-80ms. EGUI-MOTION.md shows the pattern: one start time in `ctx.data_mut`, `delay = (i * 0.04).min(0.2)` per item. Rules:

- keep the per-item delay short; long delays make the app feel slow
- cap the total so long lists don't drag
- stagger is decorative: never block input while it plays

## 12. Performance

The browser rules (compositor layers, `will-change`, hardware-accelerated transforms) don't apply. egui rebuilds and re-tessellates the frame on every repaint, so cost is shapes per frame times how often you repaint.

- Animate drawn values (offset, scale, opacity). Animating layout values (`add_space`, widget sizes, panel widths, font size) re-runs layout and text shaping every frame and makes neighbours jump.
- Repaint only while something moves. `animate_*` calls request repaints for you; hand-rolled timelines and springs must call `request_repaint()` while in flight and stop when settled.
- Timers that wait (toast lifetimes, tooltip delays) use `request_repaint_after_secs`, not a repaint every frame.
- Scale motion by time (`i.time`, `stable_dt`), never by frame count: egui is reactive and frame rate varies.

## 13. Reduced motion

Motion can cause motion sickness. Reduced motion means fewer and gentler animations, not zero: keep opacity and colour changes that aid comprehension, drop movement and scale. egui can't read the OS setting, so offer the toggle (`set_reduce_motion`) and route durations through `move_dur` (0 when reduced) and `fade_dur` (short fade kept). `Spring::step` and `pop_in_layer` already respect it.

## 14. Debugging motion

**Slow motion.** Play animations at 2-5x their duration to spot problems invisible at full speed. Route durations through a multiplier you can raise while debugging:

```rust
use eframe::egui::{Context, Id};

/// Debug: multiply every duration (2.0..=5.0 to inspect, 1.0 to ship). Route your durations through it.
pub fn slowmo(ctx: &Context, secs: f32) -> f32 {
    secs * ctx.data(|d| d.get_temp::<f32>(Id::unique("slowmo")).unwrap_or(1.0))
}
```

In slow motion, check:

- Do colours transition smoothly, or do two states visibly overlap?
- Does the curve feel right, or does it start or stop abruptly?
- Does the element scale from the right origin?
- Are opacity, offset and colour in sync?

**Real hardware.** Test drags and swipes on a real touch screen if the app supports touch; a mouse hides most gesture problems.
