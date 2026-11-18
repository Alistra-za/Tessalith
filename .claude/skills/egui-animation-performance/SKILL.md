---
name: egui-animation-performance
description: Keeps motion in Rust egui/eframe 0.37 apps inside the frame budget and finds the cause when it isn't. Covers egui's reactive repaint model, request_repaint vs request_repaint_after, repaint storms from animations that never settle, per-frame tessellation and text-layout cost, heavy work in App::ui moved to threads, texture upload, stable_dt, NativeOptions present settings, release builds, and measuring with repaint_causes, cpu_usage, paint stats and the profiling crate. Use when an egui app animates choppily, stutters, burns CPU or GPU while idle, never stops repainting, or slows down as content grows. For choosing easing, duration, or whether to animate at all use egui-animate; for reviewing motion as a whole use egui-review-animations. Not for web, CSS or JavaScript animation.
---

# egui Animation Performance

Source: Emil Kowalski's design engineering philosophy, adapted to egui.

A diagnostic skill. It does ONE thing: make motion land every frame, and make the app stop drawing frames when nothing moves. It does not pick easing or duration (that's `egui-animate`) or judge whether the motion should exist (that's `egui-review-animations`). The question here is never "does it look right?" but "does each frame finish in time, and does the app go quiet afterwards?".

API facts and the shared `motion.rs` helpers (`Spring`, `ease_out_quint`, `reduce_motion`): [EGUI-MOTION.md](EGUI-MOTION.md).

## Why This Matters

A toast that slides in at 30fps feels broken even with perfect easing. A settings panel that keeps the GPU at 100% while idle drains the laptop battery and gets the app uninstalled. Performance isn't a polish pass; it's what lets good motion register at all.

## How egui Spends a Frame

egui is immediate mode. There is no retained scene, no compositor and no GPU layer to promote. Every frame it:

1. **Runs your `App::ui`**: all widget code, layout, and text layout (cached, see below).
2. **Tessellates**: turns every shape (rects, circles, text glyphs, shadows) into triangles on the CPU.
3. **Uploads and draws**: sends the meshes and any changed textures to the GPU.

And it does this **only when something asks for a frame**: input (mouse move, key, resize), an in-flight `animate_*` call, or your `request_repaint*`. With nothing pending, eframe sleeps. This reactive model is the single most important fact in this skill: an idle egui app should use ~0% CPU.

So cost = **(work per frame) x (frames you cause)**. Both halves matter.

## Operating Posture

You have shipped egui tools that ran fine on a dev workstation and pegged a laptop fan, or stuttered on a 4K screen. You know the developer's machine is the least representative one the app will run on.

Two failure modes, and the first is worse:

1. **"It's smooth on my machine."** A fast desktop hides almost everything here. Test on a low-power laptop and a high-DPI screen; on a 120Hz display the budget is half what you think.
2. **Calling `request_repaint()` every frame and calling it fixed.** It makes animation smooth by making the app never sleep. It hides the real cause and burns power forever.

## Hard Rules

1. **Never call `request_repaint()` unconditionally.** Request a frame only while something is actually moving, and stop when it settles.
2. **Name the cause before changing code.** Every fix you propose names which of the causes below it addresses. "Added request_repaint, seems smoother" is not a diagnosis.
3. **Measure frame time only in an optimized build.** Tessellation and layout run several times slower without optimizations. `cargo run --release`, or set `[profile.dev] opt-level = 1` and `[profile.dev.package."*"] opt-level = 2`.
4. **Never conclude "it's fine" from a fast machine.** Say which findings you verified from code and which need a measurement on target hardware.
5. **Don't trade quality for a number.** Turning off vsync or feathering (anti-aliasing) to "hit 144fps" is not a fix. Fix what you draw and how often.
6. **Never block `ui()`.** No file I/O, network, `recv()`, `sleep`, or long computation inside it. One 200ms stall is 12 missed frames.

## The Symptom Table

Start here. Match what the user is seeing, then read the matching section.

| Problem | Solution |
| --- | --- |
| CPU/GPU busy while nothing visibly moves | Find the repaint storm: `ctx.repaint_causes()` |
| Animation never quite stops; value creeps | Snap to target when close; stop requesting frames |
| Animation speed depends on refresh rate | Scale by `stable_dt`, never per-frame constants |
| Hitch when a panel opens or data loads | Move the work to a thread; `try_recv` in `ui()` |
| Hitch the first time an image appears | Load the texture once, keep the `TextureHandle` |
| Scrolling a long list gets slower as it grows | `ScrollArea::show_rows`; only build visible rows |
| Animated text is expensive | Scale the drawn text with a transform; don't animate font size |
| Clock/status text repaints at 60fps | `request_repaint_after` with the real update interval |
| Many animated shapes or big shadows stutter | Fewer shapes, smaller blur; check paint stats |
| Drag feels a frame behind the pointer | `SurfaceConfig::LOW_LATENCY` |
| Everything slow, profile shows nothing obvious | You are in a debug build |

## The Frame Budget

60fps gives **16.7ms** per frame (1000/60). On a 120Hz screen it's **8ms**, and eframe follows the display's vsync. Design against the smaller number. That budget covers your `ui()` code, tessellation, and the GPU submit together.

For scale: laying out 100,000 labels in one frame takes far longer than 8ms. That isn't one missed frame, it's a visible freeze.

## Cause 1: How Often You Repaint

### Repaint storms

The most common egui performance bug: something requests a frame every frame, forever. The app never sleeps, and any animation you add looks smooth only because the whole app is spinning.

| Source | Settles on its own? |
| --- | --- |
| Input events (mouse move, keys) | Yes, frames stop when input stops |
| `ctx.animate_bool_*`, `animate_value_with_time` | Yes, they request frames only while in flight |
| `ui.spinner()` | No, by design. Remove it when loading ends |
| Your `request_repaint()` | Only if your code stops calling it |
| `Options::repaint_on_widget_change = true` | Can cause endless repaints; egui recommends keeping it off |

The classic storm is exponential smoothing that never arrives:

```rust
// Repaint storm: this never reaches `target`, so it repaints forever.
fn follow_bad(ctx: &egui::Context, x: &mut f32, target: f32) {
    *x += (target - *x) * 0.1;
    ctx.request_repaint();
}

// Fix: frame-rate independent, snaps when close, and stops asking for frames.
fn follow(ctx: &egui::Context, x: &mut f32, target: f32) {
    let dt = ctx.input(|i| i.stable_dt).min(0.1);
    *x += (target - *x) * (1.0 - (-12.0 * dt).exp());
    if (target - *x).abs() < 0.5 {
        *x = target; // settled: no more repaints
    } else {
        ctx.request_repaint();
    }
}
```

The bad version is also frame-rate dependent: `0.1` per frame moves twice as fast at 120Hz as at 60Hz. The `Spring` helper in [EGUI-MOTION.md](EGUI-MOTION.md) already sub-steps by `stable_dt` and snaps at rest; prefer it.

Hand-rolled timelines follow the same rule: request a frame only while `t < 1`.

```rust
fn flash_alpha(ctx: &egui::Context, started_at: f64, dur: f32) -> f32 {
    let t = ((ctx.input(|i| i.time) - started_at) as f32 / dur).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint(); // only while in flight
    }
    1.0 - t
}
```

Don't add `request_repaint()` next to `animate_*` calls; they already request frames while running.

### `request_repaint` vs `request_repaint_after`

| Call | Use for |
| --- | --- |
| `request_repaint()` | Something moves this frame and next frame (animation in flight, background result arrived) |
| `request_repaint_after(Duration)` / `request_repaint_after_secs(f32)` | Something changes later at a known time: clocks, "3s ago" timestamps, toast auto-dismiss, cursor blink |

```rust
fn clock_label(ui: &mut egui::Ui) {
    let now = ui.ctx().input(|i| i.time);
    ui.label(format!("Uptime {:.0}s", now));
    // The text changes once a second, so ask for one frame a second, not 60.
    ui.ctx().request_repaint_after(Duration::from_secs(1));
}
```

There is no "continuous mode" switch in eframe 0.37. Continuous repaint is exactly what you get by calling `request_repaint()` every frame. Reserve it for things that truly change every frame (a live plot, a game view), and stop it when the window is hidden: check `ctx.input(|i| i.viewport().minimized)` and `i.focused`.

### Time, not frames

Scale all motion by `ctx.input(|i| i.stable_dt)` (or derive it from `i.time`). Clamp with `.min(0.1)` so a long stall (window was hidden, debugger break) doesn't teleport the value. Never step a fixed amount per frame: a 120Hz display would run it at double speed, and egui's reactive frames are not evenly spaced anyway.

## Cause 2: How Much Each Frame Costs

### Heavy work in `ui()`

`ui()` runs on the UI thread every frame. Anything slow in it stalls the animation that plays alongside it, which is exactly when the user is watching: a panel opening while it loads its data, a tab switch that parses a file.

```rust
struct Loader {
    rx: Option<mpsc::Receiver<Vec<String>>>,
    rows: Vec<String>,
}

impl Loader {
    fn start(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone(); // Context is Send + Sync
        std::thread::spawn(move || {
            let rows = (0..100_000).map(|i| format!("row {i}")).collect(); // slow work
            let _ = tx.send(rows);
            ctx.request_repaint(); // wake the UI; it may be idle
        });
        self.rx = Some(rx);
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        if let Some(rx) = &self.rx {
            if let Ok(rows) = rx.try_recv() { // never `recv()` here: it blocks the frame
                self.rows = rows;
                self.rx = None;
            } else {
                ui.spinner(); // spinner requests its own repaints
            }
        }
        // Only lay out the rows that are visible.
        let row_h = ui.text_style_height(&egui::TextStyle::Body);
        egui::ScrollArea::vertical().show_rows(ui, row_h, self.rows.len(), |ui, range| {
            for row in &self.rows[range] {
                ui.label(row);
            }
        });
    }
}
```

Two things in that example matter: the worker calls `ctx.request_repaint()` after sending (otherwise a sleeping UI won't see the result until the mouse moves), and `show_rows` builds only the visible rows. For rows of varying height use `ScrollArea::show_viewport` and skip what's outside the rect.

### Text layout

egui caches laid-out text (galleys) keyed by the full `LayoutJob`: text, font, size, color, wrap width. A galley not used in the previous frame is evicted. Large multi-line jobs are cached per paragraph, so editing one line of a big document re-lays out only that line.

What defeats the cache:

| Pattern | Why it costs | Fix |
| --- | --- | --- |
| Animating `FontId.size` / `RichText::size` | New job every frame = full layout every frame | Lay out once at final size; scale the drawn shapes |
| Animating text color through the job | New job every frame | `ui.multiply_opacity(t)` for fades |
| `format!("{}", value)` with a value that jitters every frame | New string every frame | Round what you show (`{:.1}`) |
| Huge text that disappears for a frame | Evicted, re-laid-out on return | Keep showing it, or hold the `Arc<Galley>` yourself |

```rust
fn grow_heading(ui: &mut egui::Ui, t: f32) {
    // Bad: a new FontId size every frame = a new LayoutJob = fresh text layout every frame.
    //   ui.label(egui::RichText::new("Done").size(egui::lerp(14.0..=20.0, t)));
    // Good: lay out once at the final size, keep its place in the layout, scale the drawn shape.
    let color = ui.visuals().strong_text_color();
    let galley = ui.painter().layout_no_wrap("Done".to_owned(), egui::FontId::proportional(20.0), color);
    let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
    let c = rect.center().to_vec2();
    // 0.7 = 14/20, the same visual growth as the bad version.
    let xf = egui::emath::TSTransform::from_translation(c)
        * egui::emath::TSTransform::from_scaling(egui::lerp(0.7..=1.0, t))
        * egui::emath::TSTransform::from_translation(-c);
    let mut shape = egui::Shape::galley(rect.min, galley, color);
    shape.transform(xf);
    ui.painter().add(shape);
}
```

Same rule as the web's "animate transform, not width": animate **drawn** offset, scale and opacity, not values that change layout.

### Tessellation

Every repainted frame re-tessellates every visible shape on the CPU. There is no "this didn't change, skip it". Cost grows with:

| Driver | Notes |
| --- | --- |
| Shape count | Thousands of tiny circles/rects per frame add up. Batch into one `Mesh` or cull off-screen items. |
| Glyph count | Every visible character is a textured quad. A wall of text costs more than a few labels. |
| Shadows | Each `Shadow` is a blurred rect mesh. Large `blur` on many frames, or animating blur, costs every frame. |
| Feathering | egui's anti-aliasing adds a thin outer ring of triangles. Keep it on; it's cheap relative to the shapes themselves. |
| Big filled paths, curves | Bezier tolerance and polygon size. Rare in UI, common in plots. |

Check counts in `ctx.inspection_ui(ui)` under "Paint stats": shapes, vertices, indices, and texture memory. If one animated element multiplies those numbers, that's your target. `TessellationOptions` (via `ctx.tessellation_options_mut`) holds feathering and culling settings; `coarse_tessellation_culling` should stay on.

### Textures

Uploading an image is expensive; drawing an uploaded one is cheap. Upload once, keep the handle.

```rust
struct Thumb {
    tex: Option<egui::TextureHandle>,
}

impl Thumb {
    fn ui(&mut self, ui: &mut egui::Ui, rgba: &[u8], size: [usize; 2]) {
        // Upload once; the handle keeps the GPU texture alive. Dropping it frees it.
        let tex = self.tex.get_or_insert_with(|| {
            let img = egui::ColorImage::from_rgba_unmultiplied(size, rgba);
            ui.ctx().load_texture("thumb", img, egui::TextureOptions::LINEAR)
        });
        ui.image((tex.id(), tex.size_vec2()));
    }
}
```

Calling `load_texture` inside `ui()` every frame re-uploads every frame. Decoding the image file belongs on a worker thread (see above). `ctx.texture_ui(ui)` lists live textures and their sizes.

## Presenting: vsync and latency

eframe paces frames by vsync. The settings that exist in 0.37:

| Backend | Field | Default | Notes |
| --- | --- | --- | --- |
| wgpu (default) | `NativeOptions::wgpu_options.surface` | `SurfaceConfig::HIGH_THROUGHPUT` (2 queued frames, `AutoVsync`) | `LOW_LATENCY` queues 1 frame: drags track the pointer more tightly |
| wgpu | `surface.present_mode` | `AutoVsync` | Leave it. `AutoNoVsync` burns power for frames no one sees |
| glow (feature `glow`) | `NativeOptions::glow_options.vsync` | `true` | No effect on Wayland, which paces by compositor |
| both | `NativeOptions::multisampling` | `0` | MSAA is for embedded 3D; egui already anti-aliases by feathering |

```rust
fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        wgpu_options: eframe::WgpuConfiguration {
            // Default is HIGH_THROUGHPUT (2 queued frames). LOW_LATENCY (1) makes drags feel tighter.
            surface: eframe::SurfaceConfig::LOW_LATENCY,
            ..Default::default()
        },
        ..Default::default()
    }
}
```

Switch to `LOW_LATENCY` when direct manipulation feels a frame behind. Stay on the default if you also draw heavy custom GPU content.

## Measuring

Confirm with numbers, not by feel.

| Question | Tool |
| --- | --- |
| Why is it repainting at all? | `ctx.repaint_causes()`: `file:line` and reason of every request that caused this frame. Also shown in `ctx.inspection_ui`. |
| Did anything ask for a frame last pass? | `ctx.requested_repaint_last_pass()` |
| How long does a frame take? | `frame.info().cpu_usage`: seconds of CPU for the previous frame, `ui()` plus rendering, excluding vsync wait. `None` on the first frame. |
| Is it spinning? | `ctx.cumulative_frame_nr()`: if it climbs while you don't touch anything, you have a storm. |
| What is being drawn? | `ctx.inspection_ui(ui)` paint stats; `ctx.texture_ui(ui)` |
| Where inside the frame? | A profiler through the `profiling` crate (below) |

A drop-in overlay:

```rust
fn perf_overlay(ui: &mut egui::Ui, frame: &eframe::Frame) {
    let ctx = ui.ctx().clone();
    egui::Window::new("perf").show(&ctx, |ui| {
        if let Some(cpu) = frame.info().cpu_usage {
            ui.label(format!("CPU per frame: {:.2} ms", cpu * 1000.0)); // excludes vsync wait
        }
        ui.label(format!("Frame #{}", ctx.cumulative_frame_nr()));
        for cause in ctx.repaint_causes() {
            ui.monospace(cause.to_string()); // file:line of whoever asked for this frame
        }
        ui.collapsing("Inspection (paint stats)", |ui| ctx.inspection_ui(ui));
    });
}
```

Note: the overlay itself changes every frame while visible (its text changes), so hide it when checking whether the app goes idle; watch `cumulative_frame_nr` via a log instead.

**Profilers.** egui and eframe 0.37 have no `puffin` feature of their own. They are instrumented with the [`profiling`](https://crates.io/crates/profiling) crate (`profiling::scope!`, `function_scope!`), which compiles to nothing unless a backend feature is on. Enable one in your own `Cargo.toml` and Cargo turns it on for egui too:

```toml
[dependencies]
profiling = { version = "1", features = ["profile-with-tracy"] } # or profile-with-puffin, -superluminal, -optick, -tracing
```

Then start that backend as its own docs describe (for Tracy, run the Tracy app and connect; for puffin, add a puffin server or viewer). Wrap your own heavy functions with `profiling::function_scope!()` so they show up next to egui's scopes. Profile release builds.

## Workflow

### Phase 1: Find what animates and what repaints

List every animation in scope with its driver and what it changes:

| Element | Driver | Changes | Settles? | Plays during |
| --- | --- | --- | --- | --- |
| Side panel | `animate_bool_with_time_and_easing` | panel width (layout) | yes | file load |
| Drag preview | `Spring::step` | painter offset | yes (snaps) | drag |
| Status dot | `request_repaint()` every frame | color | **no** | always |

Expand helper calls into what they actually do: does it request repaints? Does it change layout, font size, or only drawn offset/scale/opacity?

### Phase 2: Diagnose

Work down this list. It's ordered by how often each one is the cause:

1. **Is it a debug build?** Measure release before anything else.
2. **Does the app ever go idle?** Leave it untouched; watch `cumulative_frame_nr` and `repaint_causes()`. Any unconditional or never-settling `request_repaint()` is the first suspect.
3. **Is there heavy or blocking work in `ui()`?** I/O, parsing, `recv()`, building huge lists. Does the jank coincide with loading or switching views? Move it to a thread.
4. **Does it get worse as content grows?** Non-virtualized lists (use `show_rows`), thousands of shapes, big text that re-lays out.
5. **Is text animated through its layout?** Font size, color in the job, or jittering numbers.
6. **Are textures uploaded in `ui()`?** `load_texture` every frame, or images decoded on the UI thread.
7. **Is motion stepped per frame instead of per second?** Speed varies across 60/120Hz screens.
8. **Many or large shadows, or animated blur?** Check paint stats.
9. **Still a frame behind the pointer?** Now try `SurfaceConfig::LOW_LATENCY`.

Profile on the weakest target machine and a high-DPI screen (pixel count multiplies fill cost, and more glyphs get rasterized at higher scale).

### Phase 3: Report

For each finding:

````
**[Element]**: `src/panels.rs:42`
Cause: [which of the causes above]
Before: `x += (target - x) * 0.1; ctx.request_repaint();` (never settles, repaints forever)
After: `Spring::step` (sub-stepped by stable_dt, snaps and stops at rest)
Verified: from code / needs a measurement on target hardware
````

Order findings by cost: repaint storms and blocking work before per-frame cost, per-frame cost before latency tuning. Then stop. Apply fixes when asked.
