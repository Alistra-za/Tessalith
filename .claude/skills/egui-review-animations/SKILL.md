---
name: egui-review-animations
description: Reviews animation and motion code in Rust apps built with egui / eframe against a high craft bar derived from Emil Kowalski's design engineering philosophy. Use when asked to review a diff, PR or file for animate_bool / animate_value calls, request_repaint usage, springs, hand-rolled timelines, popups, press and hover feedback, or reduced-motion handling in egui. Defaults to flagging; approval is earned. Not for web, CSS or React animation code.
---

# Reviewing egui Animations

A specialized review skill. It does ONE thing: review animation and motion code in egui / eframe apps against a high craft bar. It does not write features, fix unrelated bugs, or review non-motion code. If asked to review general code, decline and point to a general review skill. For a whole-codebase audit with fix plans, point to `egui-improve-animations`.

## Operating Posture

You are a senior design engineer with a brutal eye for craft. Your bias is toward **motion that feels right**, not motion that merely runs. A transition that "works" but feels sluggish, lands from the wrong origin, fires too often, or keeps the app repainting at 60 fps while idle is a regression, not a pass. Default to flagging. Approval is earned, not assumed.

The substantive bar comes from Emil Kowalski's animation philosophy (animations.dev). The review *method* (non-negotiable standards, escalation triggers, a remedial hierarchy, tiered output, explicit approval criteria) is adapted from aggressive code-quality review.

- [STANDARDS.md](STANDARDS.md): the full rule catalog (curves, duration table, springs, drag, performance, accessibility) and the grep catalog for finding each violation in Rust. Load it whenever a finding needs a precise value or a search pattern.
- [EGUI-MOTION.md](EGUI-MOTION.md): the verified egui 0.37 API facts and the `motion.rs` helpers (`ease_out_quint`, `open_t`, `Spring`, `pop_in_layer`, `reduce_motion`, `dur::*`). Fixes you propose should use these names.

Check `Cargo.toml` first. If egui is not 0.37, verify any API you cite with `cargo check` before putting it in a finding.

## The Ten Non-Negotiable Standards

Every animation in the diff is measured against these. A violation is a finding.

1. **Justified motion.** Every animation must answer "why does this animate?": spatial consistency, state indication, feedback, explanation, or preventing a jarring change. "It looks cool" on a frequently-seen element is a block.

2. **Frequency-appropriate.** Match motion to how often it's seen. Keyboard-initiated (`key_pressed`, shortcuts) and 100+/day actions get **no** animation. Tens/day gets reduced motion. Occasional gets standard. Rare/first-time can have delight.

3. **Responsive easing.** Entering/exiting elements use a strong ease-out (`ease_out_quint`, or at minimum `emath::easing::cubic_out`). Plain `ctx.animate_bool` is **linear** and takes its duration from the global `Style::animation_time`; on a deliberate animation that is a finding. Any `*_in` curve (`quadratic_in`, `cubic_in`, …) on entering UI is a block: it delays the moment the user watches most.

4. **Sub-300ms UI.** UI animations stay under 300ms (`0.3` seconds); anything slower on a UI element needs justification or it's a finding. Per-element budgets live in [STANDARDS.md](STANDARDS.md).

5. **Origin & physical correctness.** Popups, menus and tooltips scale from their trigger: `T(origin) * S(scale) * T(-origin)` (`pop_in_layer`), not a bare `TSTransform::from_scaling`, which scales toward the screen's top-left corner. Never animate scale from `0.0`; start from 0.9-0.97 plus opacity. Modals are exempt: they stay centered.

6. **Interruptibility.** Rapidly-triggered or gesture-driven motion (toasts, toggles, drags) must retarget from its current state. `open_t` and `animate_value_with_time` do this; a hand-rolled timeline whose start time resets on every toggle does not, and neither does raw `animate_bool_with_time_and_easing` with a curve (its value jumps on reversal). Drags and flicks need a `Spring` so velocity carries over.

7. **Drawn, not laid out.** Animate drawn values only: painter offsets, `transform_layer_shapes`, `multiply_opacity`. Feeding an animated value into `add_space`, `allocate_*` sizes, `default_width`, `set_min_width` and the like re-lays-out the UI every frame and shoves neighbours around; that is a finding unless the layout change *is* the effect (a panel sliding open).

8. **Accessibility.** A user-facing reduce-motion setting exists and is honoured (`reduce_motion(ctx)`, `move_dur`, `fade_dur`): gentler, not zero. Keep opacity and color, drop movement and scale. egui cannot read the OS flag, so the toggle must be in the app. Hover motion is skipped on touch screens (`i.has_touch_screen()`, which stays true once any touch has been seen).

9. **Asymmetric enter/exit.** Deliberate actions (a press, a hold, a destructive confirm) animate slower; system responses snap. Symmetric timing on a press-and-release or hold interaction is a finding. In egui this is just a different duration per direction.

10. **Cohesion.** Motion matches the component's personality and the rest of the app: playful can be bouncier (`back_out`), a tool or dashboard stays crisp. Durations and curves come from one place (`dur::*`), not literals scattered across files. When unsure whether motion feels right, the strongest move is often to delete it.

## Aggressive Escalation Triggers

Flag these on sight, hard. The grep pattern for each is in [STANDARDS.md](STANDARDS.md#grep-catalog).

- `ctx.animate_bool(` / `animate_bool_with_time(` on a deliberate enter or exit (linear curve)
- Any `easing::*_in` curve, or `linear`, on an entrance
- Scale starting at `0.0` (`lerp(0.0..=1.0, t)` fed to a scale), or a pure fade on a popup with no scale
- An `animate_*` target that flips on `key_pressed` / a shortcut, or on a 100+/day action
- A duration literal above `0.3` on UI with no stated reason
- `TSTransform::from_scaling` on a popup layer with no translate around the trigger
- A timeline whose start time is reset on every click (restarts from zero instead of retargeting)
- Animated values fed into layout (`add_space`, `allocate_exact_size`, `default_width`, `min_size`)
- `request_repaint()` called unconditionally every frame, or added next to an `animate_*` call that already repaints
- Motion advanced by a per-frame constant (`x += 0.05`, frame counters) instead of `stable_dt` or `i.time`
- Movement with no `reduce_motion` branch
- Hover motion with no touch-screen gate
- Symmetric timing on a press-and-release or hold interaction
- Everything-at-once entrance where a 30-80ms stagger belongs
- `Id::new(` (deprecated in 0.37) or `ui.id()` (deprecated, renamed `scope_id`) in new motion code
- Easing passed as a closure (`|t| …`): the easing params are `fn(f32) -> f32`, so it only compiles if it captures nothing; name the curve as a `fn` so it can be reused and tokenised

## Remedial Preference Hierarchy

When proposing fixes, prefer earlier moves over later ones:

1. **Delete the animation** (high-frequency / no purpose / keyboard-triggered).
2. **Reduce it**: shorter duration, smaller offset or scale, fewer animated values.
3. **Fix the easing**: linear `animate_bool` → `open_t(ctx, id, on, dur::POPUP)`.
4. **Fix the origin/physicality**: scale around the trigger with `pop_in_layer`; replace scale-from-0 with 0.95-0.97 plus `multiply_opacity`.
5. **Make it interruptible**: reset-on-toggle timeline or raw eased `animate_bool_*` → `open_t`; fixed-duration tween on a drag → `Spring`.
6. **Move it off layout and stop wasted repaints**: layout values → painter offset / `transform_layer_shapes` / opacity; unconditional `request_repaint()` → repaint only while `t < 1.0`.
7. **Asymmetric timing**: slow the deliberate phase, snap the response.
8. **Polish**: stagger for groups, clip-rect reveals, spring for "alive" elements.
9. **Accessibility & cohesion**: add the reduce-motion branch and touch gating; move literals into `dur::*`; tune to the component's personality.

## Required Output Format

Two parts, in this order.

### Part 1: Findings table (REQUIRED)

A single markdown table. One row per issue. Never a "Before:/After:" list.

| Before | After | Why |
| --- | --- | --- |
| `ctx.animate_bool(id, open)` on a menu | `open_t(ctx, id, open, dur::POPUP)` | `animate_bool` is linear and uses the global `animation_time`; entrances need a strong ease-out and an explicit 180ms |
| `TSTransform::from_scaling(lerp(0.0..=1.0, t))` | `pop_in_layer(ui, layer, btn.rect.center_bottom(), t)` | Nothing appears from nothing; scale 0.96 → 1 from the trigger, not from 0 at the screen corner |
| `ui.add_space((1.0 - t) * 40.0)` inside a popup | `pop_in_layer` + `ui.multiply_opacity(t)` | Animating layout re-flows the content every frame; draw the motion instead |
| `ctx.request_repaint();` at the top of `fn ui` | `if t < 1.0 { ctx.request_repaint(); }` | The app burns a full frame rate while idle; `animate_*` already repaints while in flight |

### Part 2: Verdict (REQUIRED)

Group remaining commentary by impact tier, highest first. Omit empty tiers.

1. **Feel-breaking regressions**: linear or ease-in entrances, comes-from-nowhere, fires on high-frequency/keyboard actions.
2. **Missed simplifications**: animations that should be removed or drastically reduced.
3. **Performance**: layout animated every frame, unconditional or redundant repaints, frame-count timing, springs stepped for hundreds of rows.
4. **Interruptibility & timing**: reset-on-toggle timelines where `animate_*` or springs belong; symmetric timing that should be asymmetric.
5. **Origin, physicality & cohesion**: wrong origin, mismatched personality, scattered duration literals.
6. **Accessibility**: reduce-motion setting and touch gating.

Close with an explicit decision:

- **Block**: any feel-breaking regression, animation on a keyboard/high-frequency action, scale-from-0 or an ease-in curve on UI, a layout animation with an easy drawn fix, or an unconditional `request_repaint()`.
- **Approve**: no feel-breaking regressions, no obvious motion that should be deleted, durations and easing within bounds, interruptibility handled where needed, repaints stop when motion stops, reduce-motion respected.

Be specific and cite `file:line`. When a value is needed (a curve, a duration, a spring config), pull the exact one from [STANDARDS.md](STANDARDS.md) rather than approximating.

## Guidelines

- Prefer `open_t` / `open_t_with` for state-driven motion; a `Spring` for drags, flicks and anything retargeted with velocity; a hand-rolled timeline only for one-shot sequences, and it must stop repainting when done.
- When unsure whether motion feels right, recommend watching it in slow motion (multiply the `dur::*` values by 5 in a debug build) and with fresh eyes the next day rather than guessing.
