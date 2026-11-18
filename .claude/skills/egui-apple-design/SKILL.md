---
name: egui-apple-design
description: Applies Apple's approach to interface design and fluid, physical motion to Rust desktop apps built with egui/eframe 0.37. Use when building or reviewing gesture-driven egui UI, spring animations, drag/swipe/sheet interactions, momentum and interruptible transitions, translucent floating panels and depth, typography (fonts, tracking, leading, weight), reduced motion, or the design foundations (feedback, spatial consistency, restraint) behind Apple-style interfaces in an egui app. For a one-off transition use egui-animate. Not for web, CSS, SwiftUI or UIKit work.
---

# egui Apple Design

How Apple builds interfaces that stop feeling like a computer and start feeling like an extension of you. This knowledge comes from Apple's WWDC design talks, chiefly *Designing Fluid Interfaces* (WWDC 2018), translated into egui 0.37: `Sense::drag`, `Response::drag_delta`, `pointer.velocity()`, `stable_dt`, and the `Spring` helper.

API facts and the shared `motion.rs` helpers (`Spring`, `ease_out_quint`, `reduce_motion`, `open_t`, `pop_in_layer`, `dur`): [EGUI-MOTION.md](EGUI-MOTION.md). All snippets below assume `use crate::motion::*;`.

The through-line: **an interface feels alive when motion starts from the current on-screen value, inherits the user's velocity, projects momentum forward, and can be grabbed and reversed at any instant.** Springs are the tool that makes this natural, because they are interruptible and velocity-aware. egui has no spring of its own; `Spring` in `motion.rs` is it.

## The Core Idea

An interface is fluid when it behaves like the physical world: things respond instantly, move continuously, carry momentum, resist at boundaries, and can be redirected mid-motion. Everything below is a way to get closer to that.

Apple frames design as serving four human needs: **safety/predictability, understanding, achievement, and joy.** Every rule here serves one of them.

## 1. Response: kill latency

The moment lag appears, the feeling of directness falls off a cliff. Response is the foundation everything else is built on.

- **Respond on pointer-down, not on release.** `response.clicked()` fires on release. Show press feedback from `response.is_pointer_button_down_on()`, which is true from the first frame the button goes down.
- **Audit every latency.** Artificial delays, `request_repaint_after` used as a debounce, work in `ui()` that stalls the frame (see `egui-animation-performance`). Anything on the input path that isn't essential is a regression.
- **Feedback is continuous during the interaction.** For a drag, slider, or sheet, update 1:1 with the pointer every frame. Never animate only when the gesture completes.

```rust
fn press_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::TextStyle::Button.resolve(ui.style()),
        ui.visuals().text_color(),
    );
    let size = galley.size() + egui::vec2(24.0, 12.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    // Down = 1 the instant the pointer goes down, not on release.
    // open_t, not raw animate_bool_*: a quick tap reverses mid-flight without a jump.
    let down = open_t(ui.ctx(), resp.id.with("press"), resp.is_pointer_button_down_on(), move_dur(ui.ctx(), dur::PRESS));
    let r = egui::Rect::from_center_size(rect.center(), rect.size() * egui::lerp(1.0..=0.97, down));
    let v = ui.style().interact(&resp);
    ui.painter().rect_filled(r, 8.0, v.bg_fill);
    ui.painter().galley(r.center() - galley.size() / 2.0, galley, v.text_color());
    resp
}
```

Scale 0.97 over `dur::PRESS` (160ms; anything in 100–160ms reads as instant), scaled around the centre, drawn only (the allocated rect never changes, so no layout shift).

## 2. Direct manipulation: 1:1 tracking

Touch and content should move together. When the user drags something it stays glued to the pointer, and respects the offset from *where they grabbed it*. Snapping its centre to the pointer on grab breaks the illusion.

| egui | What it gives you |
| --- | --- |
| `Sense::drag()` on `ui.interact` / `allocate_exact_size` | Drag tracking that continues outside the widget's rect until release (egui captures it) |
| `response.drag_started()` / `dragged()` / `drag_stopped()` | The three phases |
| `response.drag_delta()` | Points moved this frame. **Accumulate it** into your position: the grab offset is preserved automatically |
| `response.interact_pointer_pos()` | Absolute pointer. Setting `pos = pointer` is what loses the grab offset |
| `ctx.input(\|i\| i.pointer.velocity())` | Smoothed pointer velocity in points/s, from a short position history. Available on the `drag_stopped()` frame |

## 3. Interruptibility: the single most important principle

The thought and the gesture happen in parallel. Every animation must be interruptible and redirectable at any moment. A user must be able to grab a moving element mid-flight and reverse it. A closing sheet the user grabs again follows the pointer; it doesn't finish closing first.

- **Never lock out input during a transition.** No `if animating { return; }`.
- **Always continue from the presentation (on-screen) value.** With `Spring`, `spring.value` *is* the on-screen value. On `drag_started()`, start tracking from it, never from the old target.
- **Prefer springs over fixed timelines for anything touchable.** `open_t` and `animate_value_with_time` retarget from the current value (raw eased `animate_bool_*` jumps instead), but they drop velocity, so a reversal hits a "brick wall". `Spring` keeps velocity through a retarget.
- **Decompose 2D motion into independent X and Y springs.** One spring on a 2D distance desyncs when X and Y have different velocities.

```rust
struct Pip {
    x: Spring,
    y: Spring,
}

impl Pip {
    fn release(&mut self, ctx: &egui::Context, corner: egui::Pos2) -> egui::Pos2 {
        egui::pos2(self.x.step(ctx, corner.x), self.y.step(ctx, corner.y))
    }
}
```

## 4. Behavior over animation: use springs

Think of animation as a conversation between you and the object, not something prescribed by the interface. A fixed-duration animation can't respond to new input. A spring can: new input just changes the target and the motion stays continuous.

Apple replaced mass/stiffness/damping with two designer-friendly knobs. Think in these:

- **Damping ratio**: overshoot. `1.0` = critically damped, no bounce. `< 1.0` overshoots; lower = bouncier.
- **Response**: how quickly the value reaches the target, in seconds. Lower = snappier. **Not a duration**: a spring has no fixed duration; settle time emerges from the parameters.

`Spring` takes raw `stiffness` and `damping` (mass 1). Convert:

```rust
/// Spring from Apple's two knobs. `response` in seconds, `damping_ratio` 1.0 = no overshoot.
fn apple_spring(value: f32, response: f32, damping_ratio: f32) -> Spring {
    let k = (std::f32::consts::TAU / response).powi(2); // mass = 1
    Spring { value, velocity: 0.0, stiffness: k, damping: 2.0 * damping_ratio * k.sqrt() }
}
```

`Spring::new` (stiffness 170, damping 26) is critically damped with a response of about 480ms, slower than Apple's 300–400ms default; use `apple_spring` for that. The shared spring table in EGUI-MOTION.md uses the same conversion.

**Defaults:**
- Most UI: **damping 1.0**, graceful and non-distracting.
- Add bounce (**damping ~0.8**) **only when the gesture carried momentum** (flick, throw, drag release). Overshoot on a menu that faded in feels wrong; on a card you flicked it feels right.

**Values Apple ships:**

| Interaction | Damping | Response | `apple_spring(v, …)` |
| --- | --- | --- | --- |
| Move / reposition (e.g. PiP) | 1.0 | 400ms | `0.4, 1.0` |
| Rotation | 0.8 | 400ms | `0.4, 0.8` |
| Drawer / sheet | 0.8 | 300ms | `0.3, 0.8` |

## 5. Velocity handoff: the seam between drag and animation

When a gesture ends, the animation must **continue at the pointer's exact velocity**, so there's no visible seam between dragging and animating. This is the detail that most separates "fluid" from "fine".

With `Spring` this is one assignment: on `drag_stopped()`, set `spring.velocity` to the release velocity in points/s, then keep calling `step`. `Spring` uses absolute velocity, so Apple's normalization (`gestureVelocity / (target - current)`) does not apply.

`pointer.velocity()` is smoothed over a few frames and can read **zero when the frame rate is very low**. If that matters, keep your own short history of `(time, pos)` during the drag.

## 6. Momentum projection: animate to where the gesture is going

Take a small input and make a big output. Don't snap to the nearest boundary from the *release point*. Use velocity to **project the resting position**, like scroll deceleration, then snap to the target nearest that projection. This is what makes a flick throw the element.

Apple's projection (from the *Designing Fluid Interfaces* sample code), and rubber-banding (section 9):

```rust
/// Where a flick would come to rest. `v` in points/s. 0.998 = normal scroll, 0.99 = snappier.
fn project(v: f32, deceleration_rate: f32) -> f32 {
    (v / 1000.0) * deceleration_rate / (1.0 - deceleration_rate)
}

/// The further past the edge, the less the content follows. `dim` = size of the dragged axis.
fn rubberband(overshoot: f32, dim: f32) -> f32 {
    const C: f32 = 0.55;
    (overshoot * dim * C) / (dim + C * overshoot.abs())
}
```

The textbook `v^2/(2*decel)` is *not* what Apple ships; use the exponential-decay form. Decide commit vs. reverse from the projection (effectively the velocity's **sign**), not from the release position.

### All of 2, 3, 5, 6 and 9 together: a draggable sheet

```rust
struct Sheet {
    y: Spring,     // presentation value: what is on screen right now
    raw_y: f32,    // finger position before rubber-banding
    target: f32,
}

const SNAPS: [f32; 2] = [0.0, 320.0]; // open, closed (offset from the top of the area)

impl Default for Sheet {
    fn default() -> Self {
        Self { y: apple_spring(320.0, 0.3, 0.8), raw_y: 320.0, target: 320.0 }
    }
}

impl Sheet {
    fn ui(&mut self, ui: &mut egui::Ui, area: egui::Rect) {
        let ctx = ui.ctx().clone();
        let sheet = egui::Rect::from_min_size(area.min + egui::vec2(0.0, self.y.value), area.size());
        let resp = ui.interact(sheet, ui.make_persistent_id("sheet"), egui::Sense::drag());

        if resp.drag_started() {
            self.raw_y = self.y.value; // grab mid-flight: start from what is on screen
        }
        if resp.dragged() {
            self.raw_y += resp.drag_delta().y; // accumulate deltas: keeps the grab offset
            let (lo, hi) = (SNAPS[0], SNAPS[1]);
            let shown = if self.raw_y < lo {
                lo - rubberband(lo - self.raw_y, area.height())
            } else if self.raw_y > hi {
                hi + rubberband(self.raw_y - hi, area.height())
            } else {
                self.raw_y
            };
            self.y.value = shown; // track 1:1, no spring while the finger is down
            self.y.velocity = 0.0;
        } else {
            if resp.drag_stopped() {
                // Read velocity on the release frame; it can be zero at very low frame rates.
                let v = ctx.input(|i| i.pointer.velocity().y);
                let rest = self.y.value + project(v, 0.998);
                self.target = *SNAPS
                    .iter()
                    .min_by(|a, b| (*a - rest).abs().total_cmp(&(*b - rest).abs()))
                    .unwrap();
                self.y.velocity = v; // hand the finger's speed to the spring: no seam
            }
            self.y.step(&ctx, self.target);
        }

        let t = 1.0 - self.y.value / SNAPS[1]; // 1 = open
        ui.painter().rect_filled(area, 0.0, egui::Color32::from_black_alpha((t.clamp(0.0, 1.0) * 80.0) as u8));
        ui.painter().rect_filled(sheet, 12.0, ui.visuals().window_fill);
    }
}
```

`Spring::step` requests repaints only while moving and snaps at rest, so the app goes idle once the sheet settles. Under reduced motion it jumps to the target.

## 7. Spatial consistency: symmetric paths, anchored origins

If something disappears one way, we expect it to emerge from where it came.

- **Enter and exit along the same path.** A panel that slides in from the right dismisses to the right. Driving both directions from one `open_t` / `Spring` value guarantees it.
- **Anchor to the source.** A menu or popover originates from the element that triggered it. Use `pop_in_layer(ui, layer_id, trigger_rect.center_bottom(), t)` so it scales from the trigger, never from the screen corner or its own centre.
- **Same curve both ways on reversible transitions, and no jump on reversal.** `open_t` eases out in both directions and continues from the on-screen value when the user reverses mid-flight. Raw `animate_bool_with_time_and_easing` also eases out both ways, but its value jumps on a mid-flight reversal.

## 8. Hint in the direction of the gesture

People predict a final state from a trajectory. Intermediate frames should telegraph where things are going: Control Center modules grow up and out toward the finger. Derive in-between visuals (scrim alpha, scale, corner radius) from the live spring value, as the sheet's scrim does above, so they point at the outcome instead of interpolating blindly to it.

## 9. Rubber-banding: soft boundaries

At an edge, resist progressively instead of stopping hard. A hard stop reads as "frozen"; continuous resistance reads as "responsive, but there's nothing more here". Use `rubberband` (above) on the overshoot past each bound, and let the spring bring it back on release.

egui 0.37's `ScrollArea` already does this for drag-to-scroll and trackpads (not mouse wheels or scroll bars): kinetic coasting with exponential decay, rubber-band past the edges, and an optional edge fade. Tune it in the style rather than rebuilding it:

```rust
fn scroll_feel(ctx: &egui::Context) {
    ctx.all_styles_mut(|s| {
        let k = &mut s.spacing.scroll.kinetic;
        k.rubber_band = true; // default: drag/trackpad past the edge, spring back
        k.decay_time = egui::style::KineticScrollStyle::decay_time_from_deceleration_rate_per_ms(0.998);
        s.spacing.scroll.fade.strength = 0.5; // soft edge instead of a hard divider
    });
}
```

Use `rubberband` for surfaces you drive yourself (sheets, carousels, draggable cards).

## 10. Gesture design details (the "feel" checklist)

- **Tap:** highlight on pointer-down (instant), commit on release (`clicked()`). egui cancels a click if the pointer moves more than `InputOptions::max_click_dist` (6 points by default, `ctx.options_mut(|o| o.input_options.max_click_dist = …)`).
- **Drag/swipe:** require a small movement threshold (~10 points) before committing to a direction, then track 1:1. Check `resp.total_drag_delta().map_or(0.0, |d| d.length()) > 10.0` before locking the axis.
- **Detect all plausible gestures in parallel from the first move**, then cancel the losers once intent is clear. `Sense::click_and_drag()` gives you both on one response.
- **Minimize disambiguation delays.** `double_clicked()` doesn't delay `clicked()` in egui, but if you suppress single-click until a double-click window passes, you pay that latency on every tap. Only do it where double-click truly exists.

## 11. Frame-level smoothness

Smoothness is about what's in the frames, not just the frame rate.

- Keep per-frame positional change below the perception threshold to avoid strobing. Very fast travel over a large distance: fade or shorten it.
- Step motion by time (`stable_dt`), never by frame count: egui frames are not evenly spaced and displays run at 60, 120 or 144Hz.
- Animate drawn offset, scale and opacity (`painter` offsets, `transform_layer_shapes`, `multiply_opacity`), not layout. For everything about frame cost, see `egui-animation-performance`.

## 12. Materials and depth: translucency conveys hierarchy

Apple uses translucent materials as a floating functional layer that brings structure without stealing focus.

**egui has no backdrop blur.** A `Frame` can be translucent, but what shows through is sharp. The honest alternative: a mostly opaque fill (80–90% alpha) plus a soft shadow and a faint light edge. It reads as a floating layer; it is not frosted glass, so keep the alpha high enough that text behind can't compete with text on top.

```rust
fn floating_toolbar(ctx: &egui::Context, reduce_transparency: bool) {
    egui::Area::new(egui::Id::unique("toolbar"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -16.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let base = ui.visuals().window_fill;
            let fill = if reduce_transparency { base } else { base.gamma_multiply(0.85) };
            egui::Frame::new()
                .fill(fill)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_white_alpha(40))) // light catching the edge
                .corner_radius(12)
                .inner_margin(10)
                .shadow(egui::Shadow { offset: [0, 6], blur: 24, spread: 0, color: egui::Color32::from_black_alpha(60) })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let _ = ui.button("Cut");
                        let _ = ui.button("Copy");
                        let _ = ui.button("Paste");
                    });
                });
        });
}
```

- **Float chrome over content instead of reserving a strip.** A `Panel::top` or `Panel::bottom` is an opaque bar that takes a fixed strip. For Apple-style chrome, put an `Area` with `Order::Foreground` over a `ScrollArea` that scrolls underneath.
- **Material weight encodes hierarchy:** heavier, more opaque fills separate structural regions (sidebars); lighter fills draw attention to interactive elements. **Never stack a translucent surface on another**: legibility collapses, and without blur it collapses faster than on Apple platforms.
- **Bigger surfaces read as thicker:** larger `Shadow::blur` and `offset` than small chips. `Shadow` fields are integers (`offset: [i8; 2]`, `blur: u8`, `spread: u8`).
- **Dim to focus, separate to keep flow.** A modal task gets a scrim (`painter.rect_filled(screen, 0.0, Color32::from_black_alpha(…))` on a layer below the modal). A parallel, non-blocking panel uses offset and shadow without a scrim.
- **Legibility over translucent fills:** use `strong_text_color()`, slightly larger or heavier text, and put colour on an opaque layer, not the translucent one.
- **Scroll edge effects, not hard dividers:** instead of a 1px separator under floating chrome, fade content where it meets the chrome. `ScrollArea` has a built-in edge fade (`spacing.scroll.fade`, section 9); for chrome floating over content, paint a short vertical gradient (a `Mesh` with per-vertex alpha).
- **Materialize, don't just fade:** on enter/exit animate scale (0.96 to 1) and opacity together with `pop_in_layer` + `multiply_opacity`. Don't animate shadow blur; it re-tessellates a blurred mesh every frame.

## 13. Multimodal feedback: motion + sound + haptics

egui has no audio or haptics API; desktop rarely has haptics at all. If you add sound through another crate, the three rules still hold:

1. **Causality**: trigger feedback on the causal event (the toggle flipping, the item snapping home), and match its character to the action.
2. **Harmony**: visual and sound start on the **same frame**. Fire the sound in the same `ui()` pass that starts the motion, not from a delayed callback.
3. **Utility**: reserve it for meaningful moments (success, error, commit, snap). Over-feedback trains users to ignore all of it.

## 14. Reduced motion and accessibility

Reduced motion doesn't mean no feedback; it means a gentler, non-vestibular equivalent. egui cannot read the OS accessibility flags, so expose them as app settings (the `reduce_motion` / `set_reduce_motion` pattern in `motion.rs`) and default them from the OS through a platform crate if you need to.

| Setting | Do |
| --- | --- |
| Reduce motion | Replace slides, springs and parallax with short opacity cross-fades: `move_dur` (0 for movement) and `fade_dur` (fades kept, capped at 120ms). `Spring::step` and `pop_in_layer` already jump to the end. Drop overshoot. |
| Reduce transparency | Same pattern, your own flag: opaque fills, as `floating_toolbar(ctx, true)` does |
| Increase contrast | Same pattern: opaque fills, a defined `Stroke` border, `strong_text_color()` |

Also: avoid full-window moving backgrounds, slow looping oscillations (near 0.2 Hz, one cycle per 5s), and abrupt brightness jumps (cross-fade dark and light themes rather than snapping `set_visuals`). Make large moving surfaces semi-transparent while they travel, and fade them out during a large reposition and back in once settled.

## 15. Typography: what egui can control

Apple designs type to change shape with size (*The Details of UI Typography*, WWDC 2020). egui gives you less than CSS, so know the knobs:

| Want | egui 0.37 |
| --- | --- |
| Custom font | `FontDefinitions::font_data` + `families`, then `ctx.set_fonts` (or `ctx.add_font`) |
| Size per role | `Style::text_styles: BTreeMap<TextStyle, FontId>` (Heading, Body, Button, Small, Monospace) |
| Tracking | `TextFormat::extra_letter_spacing`, in **points**, per `LayoutJob` section. No global setting |
| Leading | `TextFormat::line_height: Option<f32>`, absolute **points** (not a multiplier); text is centred in it |
| Weight | `TextFormat::coords: VariationCoords` (`wght` axis) on a **variable** font; otherwise register each weight file as its own family |
| Optical sizing | Not automatic. Use a variable font with an `opsz` axis via `coords`, or separate files |
| Text-size setting (Dynamic Type) | `ctx.set_zoom_factor` scales everything; egui sizes are in points so layout scales with text. Ctrl +/-/0 are built in (`Options::zoom_with_keyboard`) |

```rust
fn setup_type(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Inter".into(),
        Arc::new(egui::FontData::from_static(include_bytes!("../assets/Inter-Variable.ttf"))),
    );
    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "Inter".into());
    ctx.set_fonts(fonts);

    ctx.all_styles_mut(|s| {
        use egui::{FontId, TextStyle};
        s.text_styles.insert(TextStyle::Heading, FontId::proportional(28.0));
        s.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(11.0));
    });
}

fn display_text(ui: &mut egui::Ui, text: &str) {
    use egui::epaint::text::{LayoutJob, TextFormat, VariationCoords};
    let size = 34.0;
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id: egui::FontId::proportional(size),
            extra_letter_spacing: -0.02 * size,         // points, not em: -0.02em at this size
            line_height: Some((size * 1.1).round()),    // points, not a multiplier
            coords: VariationCoords::new([(*b"wght", 650.0)]), // needs a variable font
            color: ui.visuals().strong_text_color(),
            ..Default::default()
        },
    );
    ui.label(job);
}
```

Rules that carry over unchanged:

- **Tracking is size-specific, never one value for all sizes.** Large display text wants negative tracking (about -0.02em, so `-0.02 * size` points); small text slightly positive; body near 0.
- **Leading tracks size inversely.** Tight on large headings (~1.05–1.1x size), looser on body (~1.4–1.5x). Round `line_height` to whole pixels for crisp text.
- **Build hierarchy from weight + size + leading as a set**, not size alone. Emphasize with weight; it adds presence without taking space.
- **Respect the user's text size.** Offer a zoom setting and don't hard-code pixel positions that break when it changes.
- **Fonts:** egui ships its own default fonts, not the OS system font. eframe's `system_fonts` feature only *falls back* to system fonts for missing glyphs (CJK, Arabic, …). For a native look, bundle a well-hinted UI face deliberately rather than assuming the platform's.

## 16. Design foundations: the eight principles

The motion and craft above serve Apple's eight design principles (*Principles of Great Design*, WWDC 2026). Use these as the names you reason with:

1. **Purpose.** Make with intention; decide what *not* to build. Every feature asks for the user's time, attention, and trust; spend that only where it pays off.
2. **Agency.** Keep people in control: offer choices, don't force a single path. Back it with forgiveness: easy undo for slips, a confirmation dialog only for genuinely destructive, irreversible actions (used sparingly, or people click through).
3. **Responsibility.** Act in the user's interest. Privacy: ask at the right moment, only for what's needed. Safety: anticipate misuse and harm, especially with AI features. Cut a feature whose risk outweighs its value.
4. **Familiarity.** Build on what people already know. Metaphors neither too literal nor too abstract, and honour their physics. Things that look the same behave the same and live in the same place. Break a familiar pattern only when you can show it's better, then test it.
5. **Flexibility.** Design for different contexts, devices, and abilities. Desktop egui apps are for deep workflows with precise pointer control and keyboard; support both. When no single layout fits everyone, let people personalize.
6. **Simplicity, not minimalism.** Strip the unnecessary so the core purpose shines. Be concise and clear: hierarchy through order, spacing, contrast. Sometimes adding context simplifies. Show the common path first, advanced options one level deeper.
7. **Craft.** Attention to detail builds trust: typography, colours that work in light and dark `Visuals`, clear icons, responsive animation with immediate feedback. Every spacing, timing and alignment value is a deliberate choice you can defend. Jittery scroll and misaligned widgets read as carelessness.
8. **Delight.** The result of getting the other seven right, not confetti on top. Decide the emotion you want people to feel and reinforce it in every decision.

Tactical rules that serve these:

- **Feedback comes in four kinds:** status, completion, warning, error. Confirm meaningful actions, expose ongoing status, warn before problems, validate inline (not on submit).
- **Wayfinding.** Every screen answers: Where am I? Where can I go? What's there? How do I get out? Never trap the user.
- **Grouping and mapping.** Proximity implies relationship; place a control near what it affects. If you need a label to explain a control, the mapping is weak.
- **Direct, specific labels beat safe generic ones.** Name things for their contents ("Progress", "Library"), not vague umbrellas ("Home").

## 17. Process

- **Prototype interactively.** An interactive demo is worth a million static designs. egui is good at this: a throwaway `eframe` app with sliders for `response` and `damping_ratio` finds the right spring in minutes (see `egui-prototype`).
- **Design interaction and visuals together.** You shouldn't be able to tell where one ends and the other begins. Motion is not a layer added after the pixels.
- **Test with real people in real context**, and review motion with fresh eyes. Slow it down to catch what's invisible at full speed: temporarily multiply durations, or scale `stable_dt` in your `Spring` while reviewing.

## Quick Reference

| Need | Technique | Concrete value |
| --- | --- | --- |
| Default UI spring | Critically damped, no overshoot | `apple_spring(v, 0.3–0.4, 1.0)` (`Spring::new` is the slower ~480ms version) |
| Momentum / flick spring | Under-damped, slight bounce | `apple_spring(v, 0.3–0.4, 0.8)` |
| Gesture to spring velocity | Hand off release velocity | `spring.velocity = pointer.velocity()` (points/s) on `drag_stopped()` |
| Flick landing point | Project momentum | `current + (v/1000)*d/(1-d)`, `d = 0.998` |
| Interrupt cleanly | Start from presentation value | `raw = spring.value` on `drag_started()` |
| Avoid reversal "brick wall" | Carry velocity through retarget | `Spring`, not `animate_value_with_time` |
| Reversible transition | Same curve both ways, continuous on reversal | `open_t` |
| Decide reverse vs. commit | Velocity **sign** via projection | at release |
| 1:1 drag | `Sense::drag` + accumulate `drag_delta()` | keeps the grab offset |
| Press feedback | On pointer-down, continuous | `is_pointer_button_down_on()`, scale 0.97, `dur::PRESS` |
| Boundary | Rubber-band, don't hard-stop | `rubberband(overshoot, dim)`, c = 0.55 |
| Floating chrome | `Area` + `Frame` fill ~85% + `Shadow` | no backdrop blur in egui |
| Type tracking | Size-specific, never fixed | `extra_letter_spacing = -0.02 * size` on large text, body 0 |
| Reduced motion | Cross-fade, not slide/spring | `reduce_motion(ctx)`, `move_dur`, `fade_dur` |
