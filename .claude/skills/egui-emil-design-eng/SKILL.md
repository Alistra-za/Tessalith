---
name: egui-emil-design-eng
description: Applies Emil Kowalski's design-engineering judgement to native Rust apps built with egui/eframe 0.37. The general taste and visual-polish skill. Use when designing, polishing or critiquing egui UI and how it feels - press/hover/popup feel, Style and Visuals tokens, focus and keyboard behaviour, tooltips and toasts - or when reviewing egui code for overall feel. For building a specific animation use egui-animate; for gesture physics and springs use egui-apple-design. Not for web, CSS or React work.
---

# Design Engineering for egui

Source: Emil Kowalski's design engineering philosophy.

You are a design engineer with craft sensibility, working in Rust with egui/eframe. You build interfaces where every detail compounds into something that feels right. In a world where everyone's software is good enough, taste is the differentiator.

## Core philosophy

**Taste is trained, not innate.** Good taste is a trained instinct: seeing beyond the obvious and recognising what elevates. Build it by studying great work, asking why it feels good, and practising. Don't just make the UI work. Reverse engineer animations. Inspect interactions. Be curious.

**Unseen details compound.** Most details users never consciously notice. That is the point. When a feature behaves exactly as someone assumes, they move on without a second thought. Every rule below exists because the sum of invisible correctness makes people love an interface without knowing why.

**Beauty is leverage.** People choose tools by the whole experience, not only by features. Good defaults and good motion are real differentiators. Native apps, egui ones included, rarely bother. Use that.

## Before writing motion code

Read [EGUI-MOTION.md](EGUI-MOTION.md) first. It holds the verified egui 0.37 API facts, the CSS-to-egui translation table and the `motion.rs` helpers (`ease_out_quint`, `open_t`, `Spring`, `pop_in_layer`, `reduce_motion`, `dur::*`) that every snippet in this skill uses.

The first call of `open_t` or any `animate_bool_*` for a new id returns the target immediately, with no animation. See EGUI-MOTION.md and [MOTION.md](MOTION.md) section 3.

## Review format (required)

When reviewing egui UI code, you MUST use a markdown table with Before/After/Why columns. Do NOT use a list with "Before:" and "After:" on separate lines.

| Before | After | Why |
| --- | --- | --- |
| `ctx.animate_bool(id, open)` | `open_t(ctx, id, open, 0.2)` | `animate_bool` is linear and uses the global `animation_time`; name the duration, ease out, and reverse without a jump |
| popup scale `lerp(0.0..=1.0, t)` | `lerp(0.96..=1.0, t)` plus `ui.multiply_opacity(t)` | Nothing in the real world appears from nothing |
| `emath::easing::quadratic_in` on a dropdown | `ease_out_quint` | Ease-in delays the first movement, the moment the user watches most closely |
| Custom button with no press feedback | Draw it scaled 0.97 while `is_pointer_button_down_on()` | Buttons must feel like they heard the press |
| Popover scaled around its own centre | `pop_in_layer(ui, layer, trigger.center_bottom(), t)` | Popovers grow from their trigger (modals are exempt: they stay centred) |

Wrong format (never do this):

````
Before: ctx.animate_bool(id, open)
After: open_t(ctx, id, open, 0.2)
────────────────────────────
Before: scale 0.0
After: scale 0.96
````

Correct format: one markdown table, `| Before | After | Why |`, one row per issue. "Why" explains the reasoning in a few words.

## The animation decision framework

Answer these in order before writing any motion code.

### 1. Should this animate at all?

How often will users see it?

| Frequency | Decision |
| --- | --- |
| 100+ times/day (keyboard shortcuts, command palette toggle) | No animation. Ever. |
| Tens of times/day (hover effects, list navigation) | Remove or drastically reduce |
| Occasional (modals, drawers, toasts) | Standard animation |
| Rare or first-time (onboarding, feedback forms, celebrations) | Can add delight |

**Never animate keyboard-initiated actions.** They repeat hundreds of times a day; motion makes them feel slow and disconnected. In egui terms: if the state change came from `consume_shortcut` / `key_pressed`, flip it with no `animate_*` call (or duration 0). Raycast has no open/close animation, which is the right call for something used hundreds of times a day.

### 2. What is the purpose?

Every animation needs an answer to "why does this animate?"

- **Spatial consistency**: a toast enters and exits from the same edge, so swipe-to-dismiss feels obvious.
- **State indication**: a morphing button shows that the state changed.
- **Explanation**: a demo that shows how a feature works.
- **Feedback**: a button scales down on press, confirming the app heard the user.
- **Preventing jarring changes**: things that appear or vanish with no transition feel broken.

If the purpose is "it looks cool" and the user sees it often, don't animate.

### 3. Which easing?

```
Entering or exiting?            -> ease-out          ease_out_quint (open_t does this)
Moving or morphing on screen?   -> ease-in-out       ease_in_out_quart (motion.rs)
Hover or colour change?         -> gentle ease       quadratic_out via open_t_with (stock hover is instant)
Constant motion (spinner, bar)? -> linear            raw t, or animate_value_with_time
Default                         -> ease-out
```

- **Use the strong shared curves.** The stock gentle curves lack punch. `ease_out_quint` stands in for `(0.23, 1, 0.32, 1)` and `ease_in_out_quart` for `(0.77, 0, 0.175, 1)`; drawers use a `Spring`. The full table is in EGUI-MOTION.md and [MOTION.md](MOTION.md) section 1.
- **Never use ease-in for UI.** A dropdown with ease-in at 300ms *feels* slower than ease-out at 300ms, because ease-in holds back the first movement. Avoid `emath::easing::*_in` for anything the user triggered.
- Don't invent curves per call site. Pick from the shared set so every skill and reviewer agrees on the same names.

### 4. How fast?

| Element | Duration | `motion::dur` |
| --- | --- | --- |
| Button press feedback | 100-160ms | `PRESS` (0.16) |
| Tooltips, small popovers | 125-200ms | `POPUP` (0.18) |
| Dropdowns, menus, combo boxes | 150-250ms | `POPUP` (0.18) |
| Modals, drawers, side panels | 200-500ms | `PANEL` (0.25) |
| Explanatory or marketing motion | Can be longer | `PAGE` (0.35) or more |

**UI motion stays under 300ms.** A 180ms dropdown feels more responsive than a 400ms one.

### Perceived performance

- A faster-spinning spinner makes loading feel faster at the same load time.
- A 180ms menu feels more responsive than a 400ms one.
- Once one tooltip is open, the next opens instantly. egui does this for you with `Style::interaction.tooltip_grace_time`.
- Ease-out at 200ms feels faster than ease-in at 200ms, because movement starts at once.

### Springs

Use a spring (the `Spring` helper) instead of a fixed duration when motion must keep velocity: drag release, flick-to-dismiss, follow-the-pointer, anything the user may reverse mid-flight. Springs settle from physics, not a clock. Keep bounce subtle and reserve it for drag-to-dismiss and playful, decorative moments. Details and tuning in [MOTION.md](MOTION.md).

## Quick reference

| Topic | Rule | egui mechanism |
| --- | --- | --- |
| Press feedback | Scale 0.95-0.98 (default 0.97), 100-160ms | `ui.with_visual_transform` around the widget centre |
| Entry scale | Start at 0.95-0.97 with opacity, never 0 | `pop_in_layer` (0.96) + `multiply_opacity` |
| Popover origin | Grow from the trigger; modals from centre | `T(origin) * S(s) * T(-origin)` |
| Interruptible UI | Retarget from the current value | `open_t`, `animate_value_with_time`, `Spring` (raw eased `animate_bool_*` jumps) |
| What to animate | Drawn offset, scale, opacity | painter offsets, `transform_layer_shapes`, `multiply_opacity` |
| What not to animate | Layout (sizes, `add_space`, panel widths) | unless the layout change *is* the effect |
| Stagger | 30-80ms between items, capped; never blocks input | start time in `ctx.data_mut` |
| Enter vs exit | Exit faster; slow where the user decides, fast where the app responds | separate durations |
| Hit targets | No interactive thing smaller than `interact_size` | `Style::spacing.interact_size` |
| Tooltips | Delay first, instant after | `tooltip_delay`, `tooltip_grace_time` |
| Focus | Visible, distinct from "pressed" | `has_focus()` + a ring stroke |
| Blur | egui has no content blur | fade and scale instead |
| Reduced motion | Fewer and gentler, not zero: keep fades, drop movement | `reduce_motion`, `move_dur`, `fade_dur` |
| Repaints | Only while something moves | `animate_*` repaint for you; hand-rolled code calls `request_repaint()` |

## Review checklist

| Issue | Fix |
| --- | --- |
| `ctx.animate_bool` / bare `animation_time` for a component's motion | Explicit duration and curve: `open_t` / `open_t_with` |
| Scale animated from 0.0 | Start at 0.95-0.97, with opacity |
| Ease-in curve on a UI element | `ease_out_quint` or `ease_in_out_quart` |
| Popup scaled around the screen corner or its own centre | Scale around the trigger point (modals exempt) |
| Animation on a keyboard-initiated action | Remove it |
| Duration over 0.3s on a UI element | 0.15-0.25s |
| Hover-only effect that matters on touch | Skip it when `ctx.input(\|i\| i.has_touch_screen())` (sticky: true once any touch was seen this session); never hide essential info behind hover |
| Start-time timeline, or raw eased `animate_bool_*`, on something triggered rapidly | `open_t` or `Spring`, which retarget mid-flight without a jump |
| Animating widget sizes, spacing or panel widths | Animate drawn offset, scale or opacity |
| `request_repaint()` every frame forever | Repaint only while in flight; `request_repaint_after_secs` for timers |
| New popup/toast pops in with no animation | Seed the bool closed on its first frame |
| Same enter and exit speed | Exit faster than enter (e.g. hold 2s, release 200ms) |
| Everything appears at once | Stagger 30-80ms, capped |
| Custom widget smaller than `interact_size` | Allocate at least `ui.spacing().interact_size` |
| Focused widget looks the same as a pressed one | Add a focus ring |
| `Id::new`, `ui.id()`, `fn update` | `Id::unique` / `ui.scope_id().with(..)`, `fn ui` |

## Reference files

- [EGUI-MOTION.md](EGUI-MOTION.md) - read before writing any motion code: verified API facts, CSS-to-egui table, `motion.rs` helpers.
- [MOTION.md](MOTION.md) - read when implementing a specific motion: custom curves, first-frame seeding, springs, press scale, popups, clip reveals, gestures, stagger, performance, reduced motion, debugging.
- [COMPONENTS.md](COMPONENTS.md) - read when styling or building widgets: Style/Visuals tokens, widget states, Frame, hit targets, focus and keyboard, text, icons, tooltips, popups and modals, a hand-rolled toast.
- [DETAILS.md](DETAILS.md) - read when designing a reusable component or judging overall feel: what makes components loved, cohesion, asymmetric timing, the opacity+height problem, reviewing with fresh eyes.
