# egui Animation Audit Playbook

The recon sweep, the eight audit categories, what to look for in each, and the exact target values to cite in findings and plans. Distilled from Emil Kowalski's design engineering philosophy ([emilkowal.ski](https://emilkowal.ski/)), translated to egui / eframe 0.37. Never approximate a value that appears here: copy it. Helper names (`ease_out_quint`, `ease_in_out_quart`, `open_t`, `open_t_with`, `Spring`, `pop_in_layer`, `reduce_motion`, `move_dur`, `fade_dur`, `dur::*`) are defined in EGUI-MOTION.md, next to the shared curve and spring tables.

## Contents

- Recon sweep
- 1. Purpose & frequency
- 2. Easing & duration
- 3. Physicality & origin
- 4. Interruptibility
- 5. Performance (repaints and layout)
- 6. Accessibility
- 7. Cohesion & tokens
- 8. Missed opportunities

## Recon sweep

| Grep | Why |
| --- | --- |
| `^egui\|^eframe` in `Cargo.toml`, `name = "egui"` in `Cargo.lock` | Version; 0.37 is what EGUI-MOTION.md was verified against |
| `impl eframe::App`, `fn ui(`, `fn update(` | App entry; `update` means before 0.36 |
| `global_style_mut\|set_global_style\|all_styles_mut\|set_visuals\|set_style\|animation_time` | Style setup and global animation time |
| `animate_bool\|animate_value` | Every built-in animation |
| `request_repaint` | Every repaint request |
| `stable_dt\|i\.time\|frame_nr\|frame_count` | Hand-rolled clocks |
| `transform_layer_shapes\|multiply_opacity\|fade_in\|with_clip_rect\|shrink_clip_rect` | Drawn motion |
| `easing::\|fn ease` | Curves in use |
| `velocity\|stiffness\|damping` | Springs and gestures |
| `key_pressed\|consume_shortcut` | Keyboard-driven state that may be animated |
| `Id::new\|\.id()` | Deprecated id APIs |

## 1. Purpose & frequency

Every animation must answer "why does this animate?": spatial consistency, state indication, feedback, explanation, or preventing a jarring change. "It looks cool" on a frequently-seen element is not a purpose.

| Frequency | Decision |
| --- | --- |
| 100+ times/day (keyboard shortcuts, command palette toggle) | No animation. Ever. |
| Tens of times/day (hover effects, list navigation) | Remove or drastically reduce |
| Occasional (modals, side panels, toasts) | Standard animation |
| Rare / first-time (onboarding, feedback, celebrations) | Can add delight |

Hunt for: an `animate_*` whose target flips on `key_pressed` / `consume_shortcut`, command palettes with open/close transitions (Raycast has none, which is correct), decorative motion on list rows or hover states hit constantly. The strongest fix is often **delete the animation**: draw the end state directly.

## 2. Easing & duration

Decision order for easing:

| Motion | Curve |
| --- | --- |
| Entering or exiting | `ease_out_quint` (strong ease-out) |
| Moving / morphing on screen | `ease_in_out_quart` |
| Hover / color change, toasts | `emath::easing::quadratic_out` |
| Constant motion (spinner, progress) | `emath::easing::linear` |
| Default | `ease_out_quint` |

**Ease-in on UI is always a finding** (`quadratic_in`, `cubic_in`, any `*_in`): it starts slow, delaying the exact moment the user is watching. **Plain `ctx.animate_bool` is linear** and uses the global `Style::animation_time`; `animate_value_with_time` is linear too. Both are too weak for deliberate motion. Plans should introduce named curves next to the duration tokens, matching repo conventions (both are in `motion.rs`; the full shared curve table is in EGUI-MOTION.md):

```rust
/// Strong ease-out, close to CSS `cubic-bezier(0.23, 1, 0.32, 1)`.
pub fn ease_out_quint(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(5)
}

/// Strong ease-in-out for on-screen movement, close to CSS `cubic-bezier(0.77, 0, 0.175, 1)`.
pub fn ease_in_out_quart(t: f32) -> f32 {
    if t < 0.5 { 8.0 * t.powi(4) } else { 1.0 - (-2.0 * t + 2.0).powi(4) / 2.0 }
}
```

egui easing params are `fn(f32) -> f32`: curves must be named `fn`s. There is no cubic-bezier; for the iOS-like drawer curve use `ease_out_quint`, or a `Spring` if the drawer is draggable.

Duration budgets: **UI animations stay under 300ms**.

| Element | Duration | egui |
| --- | --- | --- |
| Button press feedback | 100-160ms | `dur::PRESS` (0.16) |
| Hover highlight | 150ms | `dur::HOVER` (0.15) |
| Tooltips, small popups | 125-200ms | `dur::POPUP` (0.18) |
| Menus, combo boxes | 150-250ms | `dur::POPUP` (0.18) |
| Modals, side panels | 200-500ms | `dur::PANEL` (0.25) |
| Full-view transitions, explanatory | Can be longer | `dur::PAGE` (0.35) |

Hunt for: `animate_bool(` on deliberate entrances, any `*_in` curve, duration literals above `0.3` on UI, `animation_time` set above 0.3, tooltip delay plus animation on every tooltip in a toolbar (after the first, they should be instant).

## 3. Physicality & origin

- **Never scale from 0.** Target: scale 0.9-0.97 plus `multiply_opacity(t)`. `pop_in_layer` uses 0.96.
- **Popups, menus and tooltips scale from their trigger**, not the screen corner: `T(origin) * S(scale) * T(-origin)` via `pop_in_layer(ui, area.response.layer_id, btn.rect.center_bottom(), t)`. A bare `TSTransform::from_scaling` scales toward (0, 0).
  **Modals are exempt**: they appear centered; scaling around the modal's own centre is correct. Do not report it.
- **Press feedback**: draw the pressable at 0.97 scale around its centre while `response.is_pointer_button_down_on()`, over `dur::PRESS` (160ms) with `open_t`. Keep it subtle (0.95-0.98). The allocation stays fixed; only the drawing shrinks.

Hunt for: `0.0..=1.0` feeding a scale, popups that only fade, `from_scaling(` with no surrounding translate, custom pressable widgets with no press feedback.

## 4. Interruptibility

`open_t` and `animate_value_with_time` retarget from the current value; a hand-rolled timeline restarts from zero if its start time is reset on each toggle. Raw `animate_bool_with_time_and_easing` (and `animate_bool_responsive`) with a non-linear curve **jumps** when the target flips mid-flight, because egui switches between `ease(v)` and `1 - ease(1 - v)`. Anything triggered rapidly or reversible mid-motion (toasts stacking, toggles, press, hover, expand/collapse) must use `open_t`, a linear `animate_*`, or a `Spring`.

- Entry on first show: `open_t` and `animate_bool_*` return the target on their first call, so a popup created already-open doesn't animate. Create it closed first. `Area::fade_in(true)` is the built-in fade for areas; set `fade_in(false)` when the code drives its own fade.
- Gesture-driven motion should use `Spring` (stored in app state, `step` once per frame): it carries velocity when retargeted. Configs (the shared table in EGUI-MOTION.md): stiffness 170 / damping 26 for snappy UI with no overshoot (default); 158 / 20 for a little bounce on a gesture release (Framer `bounce: 0.2`); 100 / 10 for visible, playful bounce on decoration only.
- **Asymmetric timing**: deliberate phases (press, hold, destructive confirm) animate slower; the system's response snaps. `open_t` keeps each leg's own duration, so `if held { 2.0 } else { 0.2 }` works mid-flight; linear `animate_bool_with_time` treats the duration as a rate and works too. Keep the same curve both ways (switching curves mid-flight makes the value jump, which is what raw eased `animate_bool_*` does on every reversal). Symmetric timing on press-and-release is a finding.

Hunt for: `start = now` / `insert_temp(id, now)` inside a `clicked()` branch, `animate_bool_with_time_and_easing` / `animate_bool_responsive` on something the user can reverse quickly, drags tweened with a fixed duration, drags without velocity-based dismissal (read `i.pointer.velocity()` on `drag_stopped()`, dismiss above ~0.11 px/ms = 110 points/s, not on distance alone), hard stops at drag boundaries instead of rising resistance, drags that react to a second finger (`i.multi_touch().is_some()` should be ignored).

## 5. Performance (repaints and layout)

egui has no compositor: every repainted frame is re-tessellated. Cost is shapes per frame times repaint rate.

- **Repaint only while something moves.** All `animate_*` calls request repaints while in flight; `request_repaint()` next to them is redundant. An unconditional `ctx.request_repaint()` in `fn ui` keeps the app at full frame rate while idle: HIGH. Hand-rolled timelines repaint only while `t < 1.0`; `Spring::step` stops when settled.
- **Animate drawn values, not layout.** Painter offsets, `transform_layer_shapes` and `multiply_opacity` don't touch layout. Animated values in `add_space`, `allocate_exact_size`, `default_width`, `min_size` or panel widths re-flow neighbours every frame. Exempt when the layout change is the effect (a side panel sliding open).
- **Time, not frames.** `x += 0.05` per frame or frame counters run at the speed of the monitor and stall when egui skips frames. Use `stable_dt` or `i.time`.
- **Scale with the list.** Springs or timelines stepped for every row of a long list, every frame, even off-screen: step visible rows only, or drive the group from one `t`.
- `request_repaint_after*` is for clocks and polling, not for faking an animation loop.

Hunt for: `request_repaint()` with no condition, `request_repaint()` beside `animate_*`, animated values in layout calls, per-frame constants, per-row springs.

## 6. Accessibility

egui cannot read the OS reduced-motion flag. The app needs a user-facing setting (`set_reduce_motion` / `reduce_motion(ctx)`), and every movement branches on it. Reduced motion means fewer and gentler animations, **not zero**: keep fades that aid comprehension (`fade_dur`, capped at 120ms), drop movement and scale (`move_dur` returns 0, or branch the offset to 0). Gate hover motion with `!ctx.input(|i| i.has_touch_screen())`, since a tap fires a hover on touch screens.

Hunt for: movement with no `reduce_motion` branch, no reduce-motion setting at all, hover motion with no touch gate, reduce-motion implementations that zero every duration including fades and press feedback.

## 7. Cohesion & tokens

- Motion should match the product's personality: playful can be bouncier (`emath::easing::back_out`, a softer spring); a tool or dashboard stays crisp. One bouncy component in a crisp app is a finding.
- Curves and durations should live in one module (`dur::*` and named easing `fn`s). Five hand-typed duration literals that almost match is a consolidation finding.
- Everything-at-once group entrances where a **30-80ms stagger** belongs (cap the total, e.g. 40ms per item capped at 200ms). Stagger is decorative: it must never block clicks.
- egui has no blur, so a crossfade that double-exposes can't be blur-masked. Fix it by shortening it, fading the outgoing state faster, or adding a small shared offset.
- `Area::fade_in` left on while the code also calls `multiply_opacity` gives a double fade.

Hunt for: duplicated near-identical durations and curves, one bouncy component in a crisp app, list entrances with no stagger, crossfades that visibly double-expose.

## 8. Missed opportunities

The additive category: places that don't animate but should.

- State changes that teleport (content swaps, a panel snapping open) where a brief transition would prevent a jarring change.
- Spatially-connected UI (a popup from a trigger) with no motion explaining where it came from.
- Rare, high-emotion moments (first-run, success, celebration) rendered with none of the delight budget they're allowed.
- Tools for these: offsets as a fraction of the element's own rect (`rect.height() * (1.0 - t)`) rather than hard-coded points, and clip-rect reveals (`painter.with_clip_rect`, `ui.shrink_clip_rect`) with an animated edge.

Report at most a handful, grounded in actual UX seams you observed, not a wishlist.
