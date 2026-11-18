# Components in egui: tokens, widgets, overlays

Verified against egui 0.37. Snippets compile on eframe 0.37 and use the `motion.rs` helpers from EGUI-MOTION.md.

## Contents

1. Style and Visuals tokens
2. Widget states
3. Frame
4. Hit targets
5. Focus and keyboard
6. Text
7. Icons
8. Tooltips
9. Popups, menus and modals
10. Toasts (hand-rolled)

## 1. Style and Visuals tokens

Set the look once, in one function, instead of tweaking widgets one by one. Good defaults matter more than options: most screens never override them.

| Token | Where | Notes |
| --- | --- | --- |
| Spacing between widgets | `style.spacing.item_spacing` | One rhythm everywhere |
| Button padding | `style.spacing.button_padding` | |
| Minimum widget size | `style.spacing.interact_size` | Your hit-target floor (section 4) |
| Window / menu padding | `spacing.window_margin`, `spacing.menu_margin` (`Margin`, `i8` fields) | |
| Corner radius | `WidgetVisuals::corner_radius`, `window_corner_radius`, `menu_corner_radius` (`CornerRadius`, `u8` fields) | |
| Borders | `WidgetVisuals::bg_stroke`, `window_stroke` (`Stroke::new(width, color)`) | |
| Shadows | `window_shadow`, `popup_shadow` (`Shadow { offset: [i8; 2], blur: u8, spread: u8, color }`) | |
| Surfaces | `panel_fill`, `window_fill`, `faint_bg_color`, `extreme_bg_color` | |
| Accent | `selection.bg_fill`, `selection.stroke` | Also used for focus and selection |
| Built-in collapse / panel / scroll-bar / Area fades | `style.animation_time` | Global; stock widget hover is instant. Components name their own durations |
| Tooltip timing | `style.interaction.tooltip_delay`, `tooltip_grace_time` | Section 8 |

Use `ctx.all_styles_mut` to change light and dark themes together (`global_style_mut` changes only the active one).

```rust
use eframe::egui::{self, Color32, CornerRadius, Shadow, Stroke, Vec2};

/// One place for the app's look. Call once from the eframe creation closure.
pub fn apply_theme(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        let s = &mut style.spacing;
        s.item_spacing = Vec2::new(8.0, 6.0);
        s.button_padding = Vec2::new(10.0, 5.0);
        s.interact_size.y = 28.0; // minimum hit height for every widget

        let v = &mut style.visuals;
        let r = CornerRadius::same(6);
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = r;
        }
        v.widgets.hovered.expansion = 0.0; // no size jump on hover
        v.widgets.active.expansion = 0.0;
        v.window_corner_radius = CornerRadius::same(10);
        v.menu_corner_radius = CornerRadius::same(8);
        v.window_stroke = Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color);
        v.popup_shadow = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(60) };
        v.window_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(70) };

        style.animation_time = 0.15; // collapse / panel / Area fades
        style.interaction.tooltip_delay = 0.5;
        style.interaction.tooltip_grace_time = 0.3; // next tooltip opens instantly
    });
}
```

## 2. Widget states

`Visuals::widgets` has one `WidgetVisuals` per state: `noninteractive`, `inactive`, `hovered`, `active` (pressed, and also focused) and `open` (a combo box or menu that is open). Each has `bg_fill`, `weak_bg_fill`, `bg_stroke`, `corner_radius`, `fg_stroke` and `expansion`. For a custom widget, `ui.style().interact(&response)` picks the right one.

- `expansion` grows the widget's drawn rect on hover/press. Non-zero values make widgets jump; the theme above sets 0.
- Stock widgets switch between these states **instantly**; `style.animation_time` only drives collapsing headers, panels, scroll bars and `Area`/`Window` fades. A hover fade on a custom widget is `open_t(ctx, resp.id.with("hover"), resp.hovered(), dur::HOVER)` plus your own painting. Keep it short (0.1-0.15s): hover runs tens of times a day.
- A focused widget uses the `active` visuals, so focus can look like "pressed". Add a ring (section 5).
- Disabled widgets (`ui.add_enabled(false, ...)`) are faded by `visuals.disabled_alpha`; explain why with `on_disabled_hover_text`.

## 3. Frame

`egui::Frame` draws a container: fill, stroke, corner radius, shadow, inner and outer margin.

- Start from a themed preset so surfaces stay consistent: `Frame::popup(style)`, `Frame::menu(style)`, `Frame::window(style)`, `Frame::group(style)`, `Frame::canvas(style)`.
- Adjust with builders: `.inner_margin(..)`, `.outer_margin(..)`, `.corner_radius(..)`, `.fill(..)`, `.stroke(..)`, `.shadow(..)`.
- Nested corners: an inner element's radius should be the outer radius minus the padding between them, or the corners look uneven.
- To fade a whole frame with its content, call `ui.multiply_opacity(t)` before showing it (or `Frame::multiply_with_opacity` for the frame alone).

## 4. Hit targets

- `style.spacing.interact_size` is the minimum size egui gives buttons, checkboxes, sliders and text edits. The default (40 x 20 points) is small for comfortable mouse use; a height of 24-28 is kinder. For touch, aim for 44 points.
- Custom widgets: allocate at least `ui.spacing().interact_size`, even if you draw something smaller inside it.
- `style.interaction.interact_radius` lets a near-miss still hit the closest widget. Raise it on touch screens.
- Keep the hit area still while the drawing animates. Scale or offset the drawing (`with_visual_transform`), never the allocated rect, so a press can't slip off a shrinking edge.

## 5. Focus and keyboard

egui has keyboard focus built in: Tab and Shift+Tab move between focusable widgets, Space/Enter activate the focused button.

| Need | API |
| --- | --- |
| Is this widget focused? | `response.has_focus()`, `gained_focus()`, `lost_focus()` |
| Put focus somewhere | `response.request_focus()`, or `ctx.memory_mut(\|m\| m.request_focus(id))` |
| Give it up | `response.surrender_focus()` |
| Move it programmatically | `ctx.memory_mut(\|m\| m.move_focus(FocusDirection::Next))` (also `Previous`, `Up`, `Down`, `Left`, `Right`) |
| What has focus now | `ctx.memory(\|m\| m.focused())` |
| Shortcut | `ctx.input_mut(\|i\| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::K)))` |
| Single key, consumed | `ctx.input_mut(\|i\| i.consume_key(Modifiers::NONE, Key::Escape))` |

Rules:

- Make focus visible and different from "pressed".
- When a palette, dialog or search opens, put focus in its field on the first frame.
- Escape closes the top-most overlay. `Popup` and menus close on Escape themselves; `Modal` only reports it through `ModalResponse::should_close()`, which you must check. Do the same for your own overlays.
- Keyboard-initiated actions don't animate. A shortcut used 100+ times a day toggles instantly.
- Anything gesture-only (hold-to-delete, swipe-to-dismiss) needs a keyboard path too.

```rust
use eframe::egui::{Key, KeyboardShortcut, Modifiers, Response, StrokeKind, Ui};

/// egui draws a focused widget with `widgets.active` visuals, which can look like "pressed".
/// Add a distinct ring for keyboard users.
pub fn focus_ring(ui: &Ui, resp: &Response) {
    if resp.has_focus() {
        let r = ui.visuals().widgets.active.corner_radius;
        ui.painter().rect_stroke(resp.rect.expand(2.0), r, ui.visuals().selection.stroke, StrokeKind::Outside);
    }
}

pub struct Palette {
    pub open: bool,
    pub query: String,
    focus_field: bool, // focus the field once, on the frame it opens
}

impl Palette {
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        // Used 100+ times a day and keyboard-initiated: toggle instantly, no animation.
        const TOGGLE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::K);
        if ctx.input_mut(|i| i.consume_shortcut(&TOGGLE)) {
            self.open = !self.open;
            self.focus_field = self.open;
        }
        if !self.open {
            return;
        }
        let resp = ui.text_edit_singleline(&mut self.query);
        if std::mem::take(&mut self.focus_field) {
            resp.request_focus(); // once: Tab can still move focus away afterwards
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.open = false;
            resp.surrender_focus();
        }
        focus_ring(ui, &resp);
    }
}
```

## 6. Text

- Text sizes come from `style.text_styles` (`TextStyle::Small`, `Body`, `Monospace`, `Button`, `Heading`). Change sizes there, not per label.
- Secondary text: `ui.weak(..)` or `RichText::weak()`, controlled by `visuals.weak_text_alpha`. Don't invent a third grey.
- Numbers that update in place (counters, timers, tables): use `TextStyle::Monospace` or right-align, so digits don't jitter.
- Don't animate font size. It re-shapes text every frame and shifts layout. Scale the drawing (`with_visual_transform`) instead.
- Decide on purpose whether labels are selectable (`style.interaction.selectable_labels`). Selectable text in buttons and toolbars feels broken.

## 7. Icons

egui has no icon component. Icons are text glyphs or images:

- The default fonts (Ubuntu-Light, Hack, egui-icons) have no emoji. Enable egui's `monochrome_emoji_fonts` feature for a monochrome set, or bundle an icon font; mixed glyph styles in production look cheap.
- For a consistent set, load an icon font into `FontDefinitions` and use its glyphs in `RichText`, or draw SVG/PNG with `egui::Image` (needs an image loader, e.g. from `egui_extras`).
- Size icons from the text style they sit next to, and align them to the text, not to the widget's edge.
- `spacing.icon_width`, `icon_width_inner` and `icon_spacing` size the checkbox and radio indicators. Keep them in proportion to body text.
- Icon-only buttons need a tooltip that names the action.

## 8. Tooltips

- `response.on_hover_text("...")` or `on_hover_ui(|ui| ...)`; `on_disabled_hover_text` for disabled widgets.
- **Delay the first, open the rest instantly.** `tooltip_delay` stops accidental tooltips; `tooltip_grace_time` opens the next tooltip right away if the user moves to a neighbour within that time. This is the "skip delay on subsequent hovers" rule, built in. Defaults are sensible; set them in the theme function so they are explicit.
- `show_tooltips_only_when_still` (default true) waits for the pointer to stop.
- Keep tooltips short and never put essential information only in a tooltip: touch users can't hover.

## 9. Popups, menus and modals

Built-ins first:

| Need | egui |
| --- | --- |
| Menu from a button | `ui.menu_button("File", \|ui\| ...)` |
| Dropdown | `egui::ComboBox` |
| Anchored popup | `egui::Popup::from_toggle_button_response(&resp).show(\|ui\| ...)` (also `Popup::menu`, `Popup::context_menu`) |
| Free-floating layer | `egui::Area` |
| Modal dialog | `egui::Modal::new(id).show(ctx, \|ui\| ...)` |

- `Area` (and `Window`) fade in by default (`fade_in(true)`). For origin-aware motion, turn that off and drive your own: `open_t` for progress, `ui.multiply_opacity(t)` inside, `pop_in_layer(ui, layer_id, trigger_point, t)` after. EGUI-MOTION.md has a complete example.
- Popovers grow from the trigger; modals stay centred and scale around their own centre.
- Popups close on Escape and on outside click (`PopupCloseBehavior`). A popup with a text field should use `CloseOnClickOutside`.
- Menu durations: 150-250ms, or none for menus opened from the keyboard.

## 10. Toasts (hand-rolled)

egui has no toast. This one is the minimum that still handles the invisible edge cases:

- enters from below by its own height and leaves the same way (spatial consistency)
- 400ms open/close (`dur::TOAST`) with `quadratic_out`, slower and softer than other UI, to feel calm
- hovering anywhere over the stack, gaps included, pauses every timer
- timers pause while the window is unfocused
- stack positions retarget, so remaining toasts glide down when one leaves
- click to dismiss; waits with `request_repaint_after_secs`, not a repaint every frame

```rust
use crate::motion::*;
use eframe::egui::{self, emath::TSTransform, Align2, Context, Id, Order, Rect, Sense};

struct Toast {
    id: Id,
    text: String,
    elapsed: f32, // visible time so far; paused while hovered or the window is unfocused
    closing: bool,
    seeded: bool, // has its animation started from 0?
    t: f32,     // 0..1 open progress, last frame
    rect: Rect, // last frame's rect, for stack hover
}

/// Minimal toast stack, bottom-right. Keep one in app state; call `show` every frame.
#[derive(Default)]
pub struct Toasts {
    list: Vec<Toast>,
    next: u64,
    last_time: Option<f64>,
    was_paused: bool,
}

impl Toasts {
    const TTL: f32 = 4.0;
    const GAP: f32 = 8.0;

    pub fn push(&mut self, text: impl Into<String>) {
        self.next += 1;
        let id = Id::unique("toast").with(self.next);
        self.list.push(Toast { id, text: text.into(), elapsed: 0.0, closing: false, seeded: false, t: 0.0, rect: Rect::NOTHING });
    }

    pub fn show(&mut self, ctx: &Context) {
        // Hovering anywhere over the stack (gaps included) pauses every timer.
        let stack = self.list.iter().fold(Rect::NOTHING, |r, t| r.union(t.rect)).expand(Self::GAP);
        let hovered = ctx.pointer_hover_pos().is_some_and(|p| stack.contains(p));
        let paused = hovered || !ctx.input(|i| i.focused);

        let now = ctx.input(|i| i.time);
        // The first frame after a pause must not count the paused time.
        let dt = if self.was_paused { 0.0 } else { (now - self.last_time.unwrap_or(now)) as f32 };
        self.last_time = Some(now);
        self.was_paused = paused;

        let screen = ctx.content_rect();
        let mut bottom = screen.bottom() - 16.0;
        for toast in self.list.iter_mut().rev() {
            if !toast.seeded {
                // The first open_t call snaps to the target. Seed it closed so it animates in.
                open_t(ctx, toast.id, false, dur::TOAST);
                toast.seeded = true;
            }
            // Slightly slower and softer than other UI, like Sonner: 400ms.
            toast.t = open_t_with(ctx, toast.id, !toast.closing, fade_dur(ctx, dur::TOAST), egui::emath::easing::quadratic_out);
            // Retargetable stack position: when a toast leaves, the rest glide down.
            let y = ctx.animate_value_with_time(toast.id.with("y"), bottom, move_dur(ctx, dur::PANEL));
            let area = egui::Area::new(toast.id)
                .order(Order::Foreground)
                .pivot(Align2::RIGHT_BOTTOM)
                .fixed_pos(egui::pos2(screen.right() - 16.0, y))
                .fade_in(false)
                .sense(Sense::click())
                .show(ctx, |ui| {
                    ui.multiply_opacity(toast.t);
                    egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(&toast.text));
                });
            toast.rect = area.response.rect;
            // Enter from below by its own height (translateY(100%)); exit leaves the same way.
            let slide = if reduce_motion(ctx) { 0.0 } else { (1.0 - toast.t) * toast.rect.height() };
            ctx.transform_layer_shapes(area.response.layer_id, TSTransform::from_translation(egui::vec2(0.0, slide)));

            if area.response.clicked() {
                toast.closing = true;
            }
            if !toast.closing && !paused {
                toast.elapsed += dt;
                if toast.elapsed >= Self::TTL {
                    toast.closing = true;
                } else {
                    ctx.request_repaint_after_secs(Self::TTL - toast.elapsed);
                }
            }
            bottom -= toast.rect.height() + Self::GAP;
        }
        self.list.retain(|t| !(t.closing && t.t <= 0.0));
    }
}
```

Use it: keep `toasts: Toasts` in the app, call `self.toasts.push("Saved")` anywhere, and `self.toasts.show(ui.ctx())` once per frame at the end of `ui`.
