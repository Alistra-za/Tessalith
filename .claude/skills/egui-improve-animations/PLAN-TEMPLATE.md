# Plan Template

Every plan written by `egui-improve-animations` follows this structure. The executor may be a less capable model with zero context and zero taste: the plan must contain everything, exactly. No references to "the audit above" or "the easing we discussed."

## Contents

- The template (Problem, Target, Repo conventions, Steps, Boundaries, Verification)
- Notes for the plan author

````markdown
# NNN: <Short imperative title>

- **Status**: TODO
- **Commit**: <output of `git rev-parse --short HEAD` when this plan was written>
- **egui version**: <from Cargo.lock, e.g. 0.37.0>
- **Severity**: HIGH | MEDIUM | LOW
- **Category**: <audit category>
- **Estimated scope**: <n files, rough size>

## Problem

What is wrong, where, and why it matters to how the app feels. Cite every
location as `src/path/file.rs:123` and include the current code verbatim:

```rust
// src/app.rs:88 (current): linear, global duration, and the popup's
// layout is animated with add_space, so its content re-flows every frame.
let t = ctx.animate_bool(ui.scope_id().with("dropdown"), self.dropdown_open);
if t > 0.0 {
    egui::Area::new(ui.scope_id().with("dropdown_area"))
        .fixed_pos(btn.rect.left_bottom())
        .show(&ctx, |ui| {
            ui.add_space((1.0 - t) * 40.0);
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label("Name");
                ui.label("Date");
            });
        });
}
```

## Target

The exact end state. Every value spelled out: easing fn, duration in seconds,
scale range, origin. Never "use a nicer easing":

```rust
// target: 180ms strong ease-out, fade + scale 0.96 -> 1 from the trigger,
// drawn only (no layout change), reduce-motion aware via fade_dur.
let t = open_t(&ctx, ui.scope_id().with("dropdown"), self.dropdown_open, fade_dur(&ctx, dur::POPUP));
if t > 0.0 {
    let area = egui::Area::new(ui.scope_id().with("dropdown_area"))
        .fixed_pos(btn.rect.left_bottom() + egui::vec2(0.0, 4.0))
        .order(egui::Order::Foreground)
        .fade_in(false) // we drive the fade ourselves
        .show(&ctx, |ui| {
            ui.multiply_opacity(t);
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label("Name");
                ui.label("Date");
            });
        });
    // Scale 0.96 -> 1 around the trigger, drawn only: no layout change.
    pop_in_layer(ui, area.response.layer_id, btn.rect.center_bottom(), t);
}
```

## Repo conventions to follow

How this codebase already does it, with one exemplar the executor should
imitate (module names, token names, id style):

- Motion helpers live in `src/motion.rs` (`dur::POPUP = 0.18`, `ease_out_quint`, `open_t`, `pop_in_layer`, `fade_dur`). If the file does not exist, plan 001 adds it; this plan depends on it.
- Ids use `ui.scope_id().with("…")`, never `Id::new`.
- <exemplar file:line that already does this correctly>

## Steps

1. <One concrete edit per step: file, what changes, resulting code.>
2. …

## Boundaries

- Do NOT touch <files/modules out of scope>.
- Do NOT change what the UI contains or how it is laid out: motion code only (unless a step says otherwise).
- Do NOT add dependencies or change `Cargo.toml`.
- If a step doesn't match the code you find (drift since the commit stamp), STOP and report instead of improvising.

## Verification

- **Mechanical**: `cargo check` and `cargo clippy` finish with zero errors and zero warnings (deprecations count).
- **Feel check**: `cargo run`, trigger <interaction>, and confirm:
  - <observable check, e.g. "the dropdown grows out of the Sort button, not from the window corner">
  - <e.g. "clicking Sort rapidly reverses the animation from where it is; it never restarts from zero or jumps">
  - Temporarily multiply `dur::POPUP` by 5, re-run, and confirm <detail, e.g. "it decelerates into place and never overshoots">. Revert the multiplier.
  - Turn on the app's reduce-motion setting and confirm movement and scale are gone but the fade remains.
  - With the app idle, CPU use drops to near zero (nothing is repainting).
- **Done when**: <machine- or eye-checkable completion criteria>.
````

## Notes for the plan author

- One plan per finding. If two findings share every file and the same fix pattern (e.g. the same `animate_bool` → `open_t` swap across widgets), they may merge into one plan.
- Pull every value from AUDIT.md; never approximate from memory. Durations go in code as seconds (`0.18`), in prose as ms (180ms).
- Every Rust excerpt in the Target section must compile against the project's egui version. If you can, check it in a scratch crate before writing the plan.
- The feel check is not optional. Motion can be mechanically correct and still feel wrong; give the executor (or the human reviewing the executor's diff) concrete things to watch for in slow motion.
- After writing plans, create or update `plans/README.md` with: a table of plans (number, title, severity, status), the recommended execution order, and any dependencies between plans.
