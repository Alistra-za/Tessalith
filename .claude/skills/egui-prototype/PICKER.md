# The Picker

The picker's appearance is **not a design decision**: it is this spec. Copy the code below verbatim; the only values that change per run are the variant names and count. It stays identical across every project so it always reads as harness chrome, never as part of the design being judged. Do not restyle it with the project's `Visuals`, fonts, or colours.

It is a floating dark pill in an `egui::Area` anchored to the bottom-right corner. A dark translucent pill works on top of any panel, light or dark, which is why it is not theme-aware.

Contents:

- Spec values
- Rules
- Behavior contract
- Reference code

## Spec values

| Part | Value |
| --- | --- |
| Placement | `Area` anchored `Align2::RIGHT_BOTTOM`, offset `(-24, -24)`, `Order::Foreground`, `fade_in(false)` |
| Pill | fill `rgba(10, 10, 10, 209)` (82%), 1 pt stroke white at alpha 20 (8%), fully rounded (`CornerRadius::same(255)`), inner margin 4 |
| Shadow | offset `[0, 8]`, blur 24, spread 0, black at alpha 61 (24%). egui frames take one shadow; the web spec's second, smaller shadow and its backdrop blur have no egui equivalent and are dropped |
| Items | 28 pt tall, 12 pt side padding, 2 pt gap, 13 pt proportional font |
| Item text | white at 55%; 85% on hover (150ms, `ease_out_quint`); 100% when active |
| Press | text drawn at scale 0.97 around its centre while the pointer is down |
| Keyboard focus | 2 pt outline, white at alpha 102 (40%), 2 pt outside the item |
| Highlight | white at alpha 31 (12%), 28 pt tall, fully rounded, under the active item |
| Highlight slide | 250ms, `ease_out_quint`, x and width; instant under reduced motion; first position never animates |
| Divider | 1×16 pt, white at alpha 31, 4 pt each side |
| Replay | 10 pt side padding, 14 pt font, label "Replay" (the default egui fonts have no circular-arrow glyph), hover text "Replay animation (R)" |

## Rules

- **Verbatim.** These values are the spec. No project fonts, no brand colours, no theme switching, no extra shadows or strokes.
- **The highlight slides; the variant swap stays instant.** The active pill animates between items (250ms, strong ease-out) as spatial feedback on the picker itself, but the variant being previewed still switches with no transition. The highlight is painted into a slot reserved before the items (`painter.add(Shape::Noop)`, then `painter.set`), so it sits under them without a second pass, and it is drawn, not laid out, so animating its width costs nothing.
- **One allowed modification:** if a variant occupies the bottom-right of the window (a toast stack, a bottom sheet, a status bar action), anchor the picker `Align2::RIGHT_TOP` with offset `(-24, 24)` so it never covers the work. Nothing else about it may move or change.
- **Replay is conditional.** Pass `with_replay = true` only when at least one variant has an entrance or state animation worth re-triggering; a static comparison gets a shorter pill without the divider and the Replay item.

## Behavior contract

The contract is fixed regardless of how the harness renders:

- Number keys `1`–`N` (up to 9) and `←`/`→` switch variants; `R` replays. Keys are ignored while a `TextEdit` has focus (`ctx.text_edit_focused()`) or while Ctrl, Cmd or Alt is held. Handle them at the top of `App::ui`, before any widget, with `ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key))`, so the keys are consumed and nothing else in the frame reacts to them.
- Clicking an item switches to it; exactly one item is active at all times, and the highlight slides to it.
- The starting variant comes from the `PROTO_V` environment variable (`PROTO_V=2 cargo run --example proto_<slug>`), falling back to variant 1, so relaunching keeps your place. The highlight takes its initial position without animating.
- Switching re-mounts the variant (new `push_id` salt and a new `mounted_at`, so entrance animations re-run); Replay re-mounts without switching.

## Reference code

Verbatim. It compiles on eframe 0.37 and uses `ease_out_quint`, `open_t`, `dur` and `move_dur` from `motion.rs` (linked from SKILL.md). Imports:

```rust
use crate::motion::*;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Key, Modifiers, Sense, Shape, Stroke, vec2};
```

State and input:

```rust
pub struct Picker {
    pub current: usize,
    pub count: usize,
    /// Bumped on switch and replay: the variant is "re-mounted".
    pub mount: u64,
    /// Time of the last mount, for entrance animations.
    pub mounted_at: f64,
    /// Highlight x/width relative to the pill, and the tween between them.
    hl_from: (f32, f32),
    hl_to: Option<(f32, f32)>,
    hl_start: f64,
}
```

```rust
impl Picker {
    pub fn new(ctx: &egui::Context, count: usize) -> Self {
        // `PROTO_V=2 cargo run` starts on variant 2, so a relaunch keeps your place.
        let v = std::env::var("PROTO_V").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
        Self {
            current: v.clamp(1, count) - 1,
            count,
            mount: 0,
            mounted_at: ctx.input(|i| i.time),
            hl_from: (0.0, 0.0),
            hl_to: None,
            hl_start: 0.0,
        }
    }

    pub fn set_active(&mut self, ctx: &egui::Context, i: usize) {
        if i < self.count {
            self.current = i;
            self.replay(ctx);
        }
    }

    pub fn replay(&mut self, ctx: &egui::Context) {
        self.mount += 1;
        self.mounted_at = ctx.input(|i| i.time);
    }

    /// 1-N and Left/Right switch, R replays. Ignored while typing or with Ctrl/Cmd/Alt held.
    pub fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.text_edit_focused() {
            return;
        }
        let mods = ctx.input(|i| i.modifiers);
        if mods.ctrl || mods.command || mods.mac_cmd || mods.alt {
            return;
        }
        let n = self.count;
        let mut pick = None;
        let mut replay = false;
        ctx.input_mut(|i| {
            for k in 1..=n.min(9) {
                if let Some(key) = Key::from_name(&k.to_string())
                    && i.consume_key(Modifiers::NONE, key)
                {
                    pick = Some(k - 1);
                }
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowRight) {
                pick = Some((self.current + 1) % n);
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                pick = Some((self.current + n - 1) % n);
            }
            replay = i.consume_key(Modifiers::NONE, Key::R);
        });
        if let Some(i) = pick {
            self.set_active(ctx, i);
        } else if replay {
            self.replay(ctx);
        }
    }

    /// Dark pill anchored bottom-right. `with_replay`: only if a variant has motion to re-run.
    pub fn show(&mut self, ctx: &egui::Context, names: &[&str], with_replay: bool) {
        let pill = CornerRadius::same(255);
        let frame = egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(10, 10, 10, 209))
            .stroke(Stroke::new(1.0, Color32::from_white_alpha(20)))
            .corner_radius(pill)
            .inner_margin(4)
            .shadow(egui::Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(61) });

        egui::Area::new(egui::Id::unique("proto_picker"))
            .anchor(Align2::RIGHT_BOTTOM, vec2(-24.0, -24.0))
            .order(egui::Order::Foreground)
            .fade_in(false)
            .show(ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.horizontal(|ui| {
                        let origin = ui.cursor().min.x;
                        let hl_slot = ui.painter().add(Shape::Noop); // painted under the items
                        let mut active_rect = None;
                        for (i, name) in names.iter().enumerate() {
                            let resp = item(ui, name, 13.0, 12.0, i == self.current);
                            if resp.clicked() {
                                self.set_active(ctx, i);
                            }
                            if i == self.current {
                                active_rect = Some(resp.rect);
                            }
                        }
                        if with_replay {
                            let (r, _) = ui.allocate_exact_size(vec2(9.0, 16.0), Sense::hover());
                            ui.painter().rect_filled(egui::Rect::from_center_size(r.center(), vec2(1.0, 16.0)), 0.0, Color32::from_white_alpha(31));
                            // Default egui fonts have no "↻" glyph, so the button says what it does.
                            if item(ui, "Replay", 14.0, 10.0, false).on_hover_text("Replay animation (R)").clicked() {
                                self.replay(ctx);
                            }
                        }
                        if let Some(r) = active_rect {
                            let target = (r.min.x - origin, r.width());
                            let (x, w) = self.highlight(ctx, target);
                            let rect = egui::Rect::from_min_size(egui::pos2(origin + x, r.min.y), vec2(w, 28.0));
                            ui.painter().set(hl_slot, Shape::rect_filled(rect, pill, Color32::from_white_alpha(31)));
                        }
                    });
                });
            });
    }

    /// Slides 250ms with strong ease-out; takes its first position without animating.
    fn highlight(&mut self, ctx: &egui::Context, target: (f32, f32)) -> (f32, f32) {
        let now = ctx.input(|i| i.time);
        let secs = move_dur(ctx, 0.25);
        let Some(to) = self.hl_to else {
            self.hl_from = target;
            self.hl_to = Some(target);
            return target;
        };
        let t = if secs > 0.0 { ((now - self.hl_start) as f32 / secs).clamp(0.0, 1.0) } else { 1.0 };
        let e = ease_out_quint(t);
        let cur = (egui::lerp(self.hl_from.0..=to.0, e), egui::lerp(self.hl_from.1..=to.1, e));
        if to != target {
            // Retarget from where it is now, so rapid switching never jumps.
            self.hl_from = cur;
            self.hl_to = Some(target);
            self.hl_start = now;
            ctx.request_repaint();
            return cur;
        }
        if t < 1.0 {
            ctx.request_repaint();
        }
        cur
    }
}
```

One item:

```rust
/// One picker item: 28px tall pill, white text at 55% / 85% hover / 100% active.
fn item(ui: &mut egui::Ui, text: &str, size: f32, pad_x: f32, active: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(size), Color32::PLACEHOLDER);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 2.0 * pad_x, 28.0), Sense::click());
    let hover = open_t(ui.ctx(), resp.id.with("hover"), resp.hovered(), dur::HOVER); // no jump when the pointer leaves mid-fade
    let alpha = if active { 1.0 } else { egui::lerp(0.55..=0.85, hover) };
    let pos = rect.center() - galley.size() / 2.0;
    let mut shape = Shape::galley(pos, galley, Color32::WHITE.gamma_multiply(alpha));
    if resp.is_pointer_button_down_on() {
        let c = rect.center().to_vec2();
        shape.transform(
            egui::emath::TSTransform::from_translation(c)
                * egui::emath::TSTransform::from_scaling(0.97)
                * egui::emath::TSTransform::from_translation(-c),
        );
    }
    ui.painter().add(shape);
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(2.0), CornerRadius::same(255), Stroke::new(2.0, Color32::from_white_alpha(102)), egui::StrokeKind::Outside);
    }
    resp
}
```

Wiring in `App::ui`: call `self.picker.handle_keys(ui.ctx())` first, render `Variant::ALL[self.picker.current]` inside `ui.push_id(self.picker.mount, ..)`, and call `self.picker.show(ui.ctx(), &Variant::ALL.map(Variant::name), with_replay)` last so the `Area` is on top.
