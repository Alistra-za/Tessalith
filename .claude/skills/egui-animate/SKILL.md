---
name: egui-animate
description: "Builds an animation from scratch in a Rust egui / eframe app, making the decisions in the order that determines whether it feels right: should it animate at all, what purpose, which egui mechanism, which drawn properties, which curve and duration, how it interrupts, how it exits. Writes the Rust implementation. Use when asked to animate something, add motion, make a widget feel alive, or build a transition in egui or eframe. Not for web, CSS or React motion. For critiquing existing egui motion use egui-review-animations; for auditing a whole egui codebase use egui-improve-animations."
---

# Building Animations in egui

A construction skill. It does ONE thing: turn a request for motion into an egui implementation that would survive a strict review. It does not audit a codebase (that's `egui-improve-animations`), critique a diff (that's `egui-review-animations`), or hunt for places that could animate (that's `egui-find-animation-opportunities`).

API facts, the CSS-to-egui table and the shared `motion.rs` helpers (`dur`, `ease_out_quint`, `ease_in_out_quart`, `reduce_motion`, `move_dur`, `fade_dur`, `open_t`, `open_t_with`, `Spring`, `pop_in_layer`), plus the shared curve and spring tables, are in [EGUI-MOTION.md](EGUI-MOTION.md). Read it before writing code. Copy `motion.rs` into the app if it isn't there yet.

## Operating Posture

You are a senior design engineer building the animation yourself. The bar is Emil Kowalski's animation philosophy, the same bar `egui-review-animations` enforces. Write it so it passes that review the first time.

Two failure modes, and the first is worse:

1. **Animating something that shouldn't animate.** The gate below exists to produce zero lines of code sometimes. That's a success, not a dodge.
2. **Animating the right thing with the wrong ingredients**: the default linear `animate_bool` on an entrance, a scale from 0, a hand-rolled timeline on a toast that restarts on every click, a duration that makes a menu feel sluggish.

Never present motion options as a menu. Make the call, state the reasoning in one line, write the code.

## Hard Rules

1. **Run the sequence in order.** Steps 1 and 2 gate everything. Don't reach for a curve before you know whether it animates at all.
2. **No approximated values.** Every curve, duration and spring config comes from the tables below or `motion.rs`. Don't invent a curve because it looks familiar.
3. **Extend the codebase's tokens, don't fork them.** If `motion.rs` (or another duration/easing module) exists, use it. Adding a parallel system is a defect.
4. **Reduced motion and hover gating ship with the animation**, not as a follow-up.
5. **Cheapest mechanism that works.** Don't hand-roll a timeline for what `open_t` already does, and don't add a crate for a fade.

## The Build Sequence

### 1. Should this animate at all?

| Frequency | Decision |
| --- | --- |
| 100+ times/day (keyboard shortcuts, command palette toggle) | **No animation. Ever.** Stop here. |
| Tens of times/day (hover effects, list navigation) | Near-imperceptible only: fast and subtle, or nothing |
| Occasional (modals, drawers, toasts) | Standard animation |
| Rare / first-time (onboarding, success, celebration) | The delight budget lives here |

**Keyboard-initiated actions are a disqualifier, not a judgment call.** Raycast has no open/close animation, which is correct for something opened hundreds of times a day.

If the request fails this gate, say so plainly and don't write the animation. Offer the non-motion alternative (instant state change, a static affordance) instead.

### 2. What is the purpose?

Name it in one of these words before continuing:

- **Feedback**: confirming the interface heard the user
- **Spatial consistency**: showing where something came from or went
- **State indication**: making a state change legible
- **Preventing a jarring change**: bridging content that would otherwise teleport
- **Explanation**: demonstrating how something works (onboarding only)
- **Delight**: allowed *only* at the rare/first-time tier

Can't name it? Don't build it. "It looks cool" on a frequently seen element is a reason to stop.

Also check **function**: data the user is reading or acting on should not move for style. A decorative pointer-tracking effect belongs on a splash screen, not on a plot in a finance tool.

### 3. Pick the mechanism, cheapest that works

Walk down; stop at the first that fits.

| Need | Mechanism |
| --- | --- |
| Collapsing sections, side panels | **Built in.** `CollapsingHeader` and `egui::Panel::show_collapsible` slide for free via `Style::animation_time`. Write nothing. Stock widget hover and press colours change instantly; animating them means custom painting (next rows). |
| A floating thing: menu, combo box, tooltip, modal | **egui's own container first** (`ui.menu_button`, `ComboBox`, `on_hover_text`, `egui::Modal`, `egui::Popup`). They handle focus, Escape and click-outside. Add motion on top only if the gate says so. |
| A two-state toggle you control (open/closed, on/off, hovered, pressed) | `open_t` (or `open_t_with` for another curve) from `motion.rs`. It reverses smoothly mid-flight; raw `animate_bool_with_time_and_easing` with a curve jumps when the target flips. |
| A predetermined one-shot sequence (stagger, intro) | Hand-rolled timeline: start time in `ctx.data_mut`, ease `t`, `request_repaint()` until done |
| A number that changes (progress, counter) | `ctx.animate_value_with_time` (linear, retargets from where it is) |
| Drag release, flicks, follow-the-pointer, anything reversed mid-motion | `Spring` from `motion.rs`, stored in app state |

Every `animate_*` call requests its own repaints. Hand-rolled timelines and springs must call `request_repaint()` while moving, and stop calling it when they settle.

Call `open_t` every frame, including while closed. Its first call snaps straight to the target, so if you only start calling it when something opens, there is no entrance.

### 4. Pick the properties

- **Animate drawn offset, scale and opacity only.** `ui.multiply_opacity`, `ui.with_visual_transform`, `ctx.transform_layer_shapes`, painter offsets. They change pixels, not layout. Never animate `add_space`, widget sizes, `set_min_width` or panel widths: the whole layout reflows every frame and neighbours jitter. The exceptions are where the layout change *is* the effect: a side panel opening (`Panel::show_collapsible`) or a collapsing section.
- **`with_visual_transform` and `transform_layer_shapes` move pixels, not hit areas.** Fine for short pop-ins and press scale. For something the user grabs while it is offset (a dragged sheet), use `ctx.set_transform_layer`, which moves input too.
- **Never scale from 0.** Start from 0.95–0.97 plus opacity 0. Nothing in the real world appears from nothing.
- **Origin at the trigger** for menus, popovers, combo lists, tooltips: build the transform around the trigger point (`pop_in_layer(ui, layer, btn.rect.center_bottom(), t)`). **Modals are exempt**: they're not anchored to a trigger, so they scale from the centre.
- **Offsets relative to the element's own size.** Slide a toast by `(1.0 - t) * rect.height()`, not by a hardcoded 40px, so it works whatever the content.
- `TSTransform` is translate plus *uniform* scale only. No rotation, skew or 3D in egui's layer transforms; if you need rotation, rotate the shapes you paint yourself.

### 5. Easing and duration, or a spring

**Easing**, in decision order:

| Situation | Easing |
| --- | --- |
| Entering or exiting | `ease_out_quint` (`motion.rs`) |
| Moving / morphing on screen | `ease_in_out_quart` (`motion.rs`) |
| Hover / colour change | `emath::easing::quadratic_out` |
| Constant motion (spinner, progress) | linear (`animate_value_with_time`, or `t` unchanged) |
| Default | `ease_out_quint` |

**Never an ease-in curve on UI** (`*_in` in `emath::easing`). It starts slow, delaying the exact moment the user is watching. Ease-out at 200ms *feels* faster than ease-in at 200ms. `open_t` eases out in both directions: the entrance and the exit both decelerate, and a reversal continues from the value on screen.

The stock `ctx.animate_bool` is **linear** and uses the global `animation_time`. Don't use it for anything deliberate.

The strong curves are both in `motion.rs`: `ease_out_quint` matches `cubic-bezier(0.23, 1, 0.32, 1)`, `ease_in_out_quart` matches `cubic-bezier(0.77, 0, 0.175, 1)`. The full curve table, shared by every egui skill, is in [EGUI-MOTION.md](EGUI-MOTION.md).

egui easing parameters are `fn(f32) -> f32`: named functions, not closures. There is no cubic-bezier in egui and the iOS drawer curve has no exact equivalent; use a `Spring` for drawers and sheets. Need another curve? Use one from `emath::easing` (`cubic_out`, `quadratic_out`, `exponential_out`, `back_out`, …). Don't hand-roll one.

**Duration** (prose in ms, code in seconds; `motion::dur` holds the tokens):

| Element | Duration | `dur` token |
| --- | --- | --- |
| Button press feedback | 100–160ms | `PRESS` (0.16) |
| Hover highlight | ~150ms | `HOVER` (0.15) |
| Tooltips, small popovers | 125–200ms | `POPUP` (0.18) |
| Menus, combo lists | 150–250ms | `POPUP` (0.18) |
| Modals, drawers, side panels | 200–500ms | `PANEL` (0.25) |
| Onboarding / explanatory | Can be longer | `PAGE` (0.35) or custom |

**UI animations stay under 300ms.** A 180ms menu feels more responsive than a 400ms one.

**Reach for a spring instead** when the motion is a drag with momentum, something that should feel alive, a gesture the user can interrupt or reverse, or decorative pointer-tracking. `Spring::new` defaults to stiffness 170, damping 26: critically damped, no overshoot. For a little bounce on a gesture release use 158 / 20; playful decoration 100 / 10 (the spring table in EGUI-MOTION.md). Keep bounce subtle and reserve it for drag-to-dismiss and playful interactions; most UI wants none.

### 6. Interruption and exit

- **Retargetable values for anything triggered rapidly** (toasts, toggles, anything a user can fire twice in a second). `open_t`, `animate_value_with_time` and `Spring` continue from the current value when the target flips; raw `animate_bool_with_time_and_easing` with a curve jumps on reversal. A hand-rolled timeline that resets its start time restarts from zero: don't use one for these.
- **Springs for gestures**, because they carry velocity through an interruption. Seed `velocity` from `ctx.input(|i| i.pointer.velocity())` on release.
- **Exit the way it entered.** A toast that slides in from the bottom leaves through the bottom. Keep drawing the element while `t > 0.0` so the exit is visible, and make it non-interactable while it leaves (`Area::interactable(open)`).
- **Asymmetric timing where the user is deciding.** Slow on the deliberate phase (hold-to-confirm: 2s linear), snappy on the system response (release: ~200ms).

### 7. Reduced motion and pointer gating

Ships with the animation, every time. egui can't read the OS reduced-motion flag, so the app offers a setting: `reduce_motion(ctx)` / `set_reduce_motion` in `motion.rs`. Use `move_dur` for movement (drops to 0) and `fade_dur` for opacity (kept, shortened to ≤120ms). `Spring::step` and `pop_in_layer` already honour it.

Reduced motion means **fewer and gentler** animations, not zero: keep fades that aid comprehension, remove movement and position changes.

Touch screens report a hover on tap. Gate hover motion:

```rust
/// Hover lift, gated: touch screens report a hover on tap.
fn hover_lift(ui: &Ui, resp: &egui::Response) -> f32 {
    let ctx = ui.ctx();
    if reduce_motion(ctx) {
        return 0.0; // movement, so it goes entirely under reduced motion
    }
    let can_hover = !ctx.input(|i| i.has_touch_screen());
    let t = open_t(ctx, resp.id.with("hover"), can_hover && resp.hovered(), dur::HOVER);
    -2.0 * t // y offset in points: drawn, not laid out
}
```

## Recipes

For ready-to-build implementations of the common cases (button press, menu/popover pop-in, tooltip, modal, side panel, toast, accordion, stagger, hold-to-confirm, tab indicator, drag with spring release, number and progress change) see [RECIPES.md](RECIPES.md). Load it whenever the request matches one of those; start from the recipe rather than from a blank file.

## Never Ship

Self-check before you finish. Each of these is an automatic block in `egui-review-animations`:

| Never | Instead |
| --- | --- |
| Stock `ctx.animate_bool` (linear, global time) on a deliberate animation | `open_t` with an explicit duration |
| Scale from 0 | 0.95–0.97 plus opacity 0 |
| An ease-in curve on a UI element | `ease_out_quint` or `ease_in_out_quart` |
| Animation on a keyboard shortcut or 100+/day action | No animation |
| UI duration over 300ms with no reason | 150–250ms |
| Popover scaled around its own corner or centre | Transform around the trigger point (modals exempt) |
| Restart-from-zero timeline, or raw eased `animate_bool_*`, on toasts, toggles, rapidly triggered elements | `open_t` / `animate_value_with_time` / `Spring` |
| Animating `add_space`, widget sizes or panel widths by hand | Drawn offset, scale, opacity |
| `request_repaint()` every frame unconditionally | Only while a hand-rolled timeline or spring is moving |
| Hand-rolled timeline with no `request_repaint()` | Repaint until `t >= 1` |
| Motion scaled by frame count | Scale by `stable_dt` or `i.time` |
| Ungated hover motion | `!ctx.input(\|i\| i.has_touch_screen())` |
| No reduced-motion path | `move_dur` / `fade_dur`: gentler, not zero |
| Everything entering at once | 30–80ms stagger |
| `Id::new` (deprecated), `ui.id()` (deprecated) | `Id::unique("name")`, `ui.scope_id().with("name")` |

## Output

Write the code. Then, in at most a few lines:

- **The gate result**: frequency tier and the named purpose. If something in the request was rejected, say which and why.
- **The ingredients**: mechanism, properties, curve, duration or spring config, one line each.
- **What to feel-check**: if the result depends on feel you can't judge from code (a crossfade, a spring's bounce, the fade/height balance in a collapsing list), say so and point at the check: temporarily multiply durations by 2–5 (or raise `Style::animation_time`) to watch it slowly, test drags on a real touch screen if the app targets one, and look again the next day with fresh eyes.

Don't pad this into a report. The code is the deliverable.

## Tone

Opinionated and brief. When the honest answer is "this shouldn't animate", give it: that answer is the reason this skill exists. When feel genuinely can't be settled from code, say so instead of guessing at a value.
