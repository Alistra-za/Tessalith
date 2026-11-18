---
name: egui-prototype
description: Builds multiple genuinely different versions of a Rust egui / eframe UI piece you describe, as an enum of variants rendered one at a time behind an egui picker overlay (bottom-right, number and arrow keys to switch), so you can flip through them live and promote the one that feels right. Only runs when explicitly invoked; it does not trigger on its own. Not for web or HTML prototypes.
disable-model-invocation: true
---

# Prototyping egui Variants

A divergence skill. It does ONE thing: take a described piece of egui UI ("a toast", "the settings panel", "a hold-to-delete button"), build several genuinely different versions of it, and put them behind a picker so the user can flip through them live and choose a winner. It does not review existing UI (that's `egui-review-animations`) or plan fixes for it (that's `egui-improve-animations`).

## Operating Posture

You are a senior design engineer running a design exploration. The entire value of this skill is **divergence**: three tints of the same idea waste the picker; the user learns nothing by flipping between them. Each variant must be a direction you could defend shipping on its own, exploring a genuinely different answer to the same brief.

Divergence is not an excuse to drop the craft bar. Every variant individually meets Emil Kowalski's standards, expressed in egui: strong ease-out on entrances (`ease_out_quint`, never an ease-in), sub-300ms UI motion, scale from the trigger (never from 0), only drawn offset, scale and opacity animated, reduced motion handled through `move_dur` / `fade_dur`, and no repaint loop left running once motion settles. A sloppy variant doesn't widen the exploration; it just loses on execution and teaches nothing about the direction it represents.

[EGUI-MOTION.md](EGUI-MOTION.md) has the verified egui 0.37 API facts and the `motion.rs` helpers every variant uses. Check `Cargo.toml` first; if egui is not 0.37, `cargo check` the harness before trusting the names here.

## Hard Rules

1. **Never touch production code during exploration.** Everything lives in an isolated prototype surface (see Phase 4). Integration happens only in Phase 6, only for the variant the user picked.
2. **Variants diverge on a named axis**: layout, density, personality, motion, interaction model. Before building, you must be able to state each variant's axis in a phrase. Sharing the project's style (`Visuals`, spacing, fonts) is not convergence; variants *should* feel native to the product.
3. **Every variant fully works.** Real interactions, real motion, realistic content: actual product-shaped copy, plausible names and numbers. No lorem ipsum, no buttons that do nothing, no "imagine this part".
4. **The picker is chrome, not a contestant.** Its exact look and behavior are specified in [PICKER.md](PICKER.md); copy them verbatim. Its look is not a design decision and never adapts to the project's theme.
5. **Clean up after the choice.** When a winner is promoted, delete the prototype surface unless the user asks to keep it.

## Workflow

### Phase 1: Scope

One thing per run. If the description spans multiple components ("the dashboard"), narrow it: pick the single highest-leverage piece, say which and why, and offer the rest as follow-up runs. Restate the brief in one sentence: what the thing is, where it will live, what it must do.

### Phase 2: Recon

Before designing anything, map the ground the variants must stand on:

- **Stack**: egui/eframe version, renderer, any widget crates already in `Cargo.toml` (`egui_extras`, `egui_plot`, …), and whether a `motion.rs` or equivalent exists.
- **Style**: the app's `Visuals` and `Style` (custom `set_visuals`, `global_style_mut`, fonts installed in the creation closure, spacing, corner radii, accent colour), and any duration/easing constants. Variants use these: every variant should look like it could ship in this app tomorrow.
- **Personality**: playful consumer app or crisp tool? This bounds how far the boldest variant may go.
- **Context**: where the piece renders: which panel, against what fill, beside what neighbours, at what window sizes.

If there is no project (empty directory, or the user is just exploring), skip to the standalone branch in Phase 4 and use egui's default dark theme with one accent colour.

### Phase 3: Choose directions

Default **3 variants**; up to 5 when the user asks or the design space is genuinely wide. More than 5 dilutes the comparison.

Before writing any code, list the set: a name and an axis for each. Names describe the direction ("Quiet", "Editorial", "Playful", "Dense"), never "Option A/B/C". If two proposed directions would differ only in accent colour or copy, they are one direction; replace one with a real alternative (different layout, different interaction model, different motion story).

**Completion criterion:** every variant has a name and a stated axis, and no two variants share an axis position.

### Phase 4: Build the picker harness

Two branches, by what exists:

- **In an existing app**: a separate binary or example (`examples/proto_<slug>.rs`, run with `cargo run --example proto_<slug>`) that renders the piece inside a faithful copy of its real surroundings, one module per variant if they are long. Nothing in the app's own modules imports from the prototype.
- **No project**: a new single-file eframe app (`src/main.rs`) with the harness and every variant in it.

The variants are one enum and one `match`. Each arm renders one complete variant; the picker only changes which arm runs.

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Quiet,
    Editorial,
    Playful,
}

impl Variant {
    pub const ALL: [Variant; 3] = [Variant::Quiet, Variant::Editorial, Variant::Playful];
    pub fn name(self) -> &'static str {
        match self {
            Variant::Quiet => "Quiet",
            Variant::Editorial => "Editorial",
            Variant::Playful => "Playful",
        }
    }
}

pub struct Proto {
    picker: Picker,
    email: String,
}

impl Proto {
    pub fn new(ctx: &egui::Context) -> Self {
        Self { picker: Picker::new(ctx, Variant::ALL.len()), email: String::new() }
    }
}

impl eframe::App for Proto {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.picker.handle_keys(ui.ctx());
        let variant = Variant::ALL[self.picker.current];
        let since = (ui.input(|i| i.time) - self.picker.mounted_at) as f32;

        // Realistic context: the page the card will live on.
        ui.heading("Workspace settings");
        ui.label("Acme Design · 12 members");
        ui.separator();

        // New id per mount: egui memory for the variant starts fresh on switch or replay.
        ui.push_id(self.picker.mount, |ui| match variant {
            Variant::Quiet => {
                ui.label("Invite teammates");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.email);
                    let _ = ui.button("Send invite");
                });
            }
            Variant::Editorial => {
                ui.label(egui::RichText::new("Bring your team in.").size(28.0).strong());
                ui.label("Invites expire after 7 days. You can resend from the members list.");
                ui.add_space(12.0);
                ui.text_edit_singleline(&mut self.email);
                let _ = ui.button("Send invite");
            }
            Variant::Playful => {
                // Entrance driven by mount time: 250ms, strong ease-out, scale 0.96 -> 1, never from 0.
                // Fade and scale are timed separately: reduced motion keeps the fade, drops the scale.
                let t = ease_out_quint((since / fade_dur(ui.ctx(), dur::PANEL)).clamp(0.0, 1.0));
                let m = move_dur(ui.ctx(), dur::PANEL);
                let grow = if m > 0.0 { ease_out_quint((since / m).clamp(0.0, 1.0)) } else { 1.0 };
                if t < 1.0 || grow < 1.0 {
                    ui.ctx().request_repaint();
                }
                // Visual-only transform (inputs unaffected), scaled from the top centre.
                let c = ui.available_rect_before_wrap().center_top().to_vec2();
                let s = egui::lerp(0.96..=1.0, grow);
                let xf = egui::emath::TSTransform::from_translation(c)
                    * egui::emath::TSTransform::from_scaling(s)
                    * egui::emath::TSTransform::from_translation(-c);
                ui.with_visual_transform(xf, |ui| {
                    ui.multiply_opacity(t);
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(egui::RichText::new("Who else should be here?").size(20.0));
                        ui.text_edit_singleline(&mut self.email);
                        let _ = ui.button("Invite");
                    });
                });
            }
        });

        self.picker.show(ui.ctx(), &Variant::ALL.map(Variant::name), true);
    }
}
```

Run it with `eframe::run_native("Prototype", options, Box::new(|cc| Ok(Box::new(Proto::new(&cc.egui_ctx)))))`. It assumes `use crate::motion::*;` and `use eframe::egui;`.

Rules the harness follows:

- **One variant at a time, full size, in realistic surrounding context.** A toast needs a window of content behind it, a card needs siblings, a button needs its form. Side-by-side thumbnails distort spacing and scale; don't evaluate UI at postage-stamp size. Open the window at the size the app really uses (`ViewportBuilder::with_inner_size`).
- **Switching is instant.** Flipping is a 100+/session action; by the frequency rule the variant swap gets no animation.
- **Switching re-mounts.** egui is immediate mode, so "re-mount" means: bump `mount` (the `push_id` salt, so the variant's egui memory such as scroll offsets and `animate_bool` state starts fresh) and reset `mounted_at`. Entrance animations must be driven from `mounted_at`, because `animate_bool_*` on a fresh id starts at its target and plays nothing.
- **State the variants share** (typed text, the selected item) lives on the harness struct, so it survives a switch and the comparison is fair.

The picker itself (look, keys, highlight) comes from [PICKER.md](PICKER.md), verbatim. Load it now and build exactly that.

### Phase 5: Verify and hand off

`cargo check` with zero warnings, then run the harness. Flip through every variant yourself with the keys and the mouse, confirm every interaction responds and no variant keeps the app repainting after its motion settles, before showing the user. If you can screenshot the window, capture each variant.

Then present the set and **stop: the choice belongs to the user**:

| # | Variant | Axis | When it's the right choice | Its cost |
| --- | --- | --- | --- | --- |
| 1 | Quiet | Minimal motion, strokes over shadows | The app is a daily-use tool | Least memorable |
| 2 | Editorial | Large type, generous spacing | The moment deserves weight | Eats vertical space |

Close with how to run the harness (`cargo run --example proto_<slug>`, `PROTO_V=2` to start on variant 2) and the keys to flip: `1`–`N`, `←`/`→`, `R` to replay.

**Completion criterion:** every variant is reachable from the picker and behaves correctly; `cargo check` is clean; the table names each variant's tradeoff honestly.

### Phase 6: Promote on selection

When the user picks: move that variant's arm into the app where it belongs, following the project's existing conventions (module layout, naming, style constants, `motion.rs` tokens), then delete the prototype example or binary per Hard Rule 5. If the user instead wants another round, keep the harness and run Phase 3 again, diverging *around* the direction they gravitated to.

## Invocation Variants

| Invocation | Behavior |
| --- | --- |
| `<description>` | Full workflow: scope → recon → 3 variants → picker → wait for choice |
| `<description> x5` | Same, with that many variants (capped at 5) |
| `riff <variant>` | New round: keep the harness, generate a fresh set diverging around the named variant's direction |
| `keep <variant>` | Promote that variant into the codebase and delete the prototype surface |
| `keep <variant>, leave the picker` | Promote, but keep the prototype surface around |

## Tone

Sell each variant honestly: one line on when it wins, one on what it costs. Never pre-pick a favourite in the table; if the user asks which you'd choose, answer with a reason rooted in the product's personality and frequency of use, not aesthetics alone. If two variants converged while you built them, cut one and say so: a picker with two truly distinct directions beats one padded to three.
