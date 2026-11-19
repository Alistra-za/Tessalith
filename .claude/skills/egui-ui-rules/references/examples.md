# Examples (egui 0.37.0, from Tessalith `crates/gui/src/ui/`)

The code is trimmed to show the pattern. Names such as `AppModel`, `panel_ids` and `look` belong to the Tessalith app, so adapt them to yours. Tessalith builds every panel through `ui::panel_ids`, which records the scoped id egui 0.37 keys the panel's state by; outside Tessalith, take that id from the parent `Ui`: `parent.scope_id().with(salt)`.

## Contents
1. Panel order, a full-width bottom panel, and a strip with its own id
2. A resizable column: range, chosen width, truncation, blur on press
3. Skipping a panel without shifting ids
4. Columns above a footer
5. A visible scroll bar and the never-under-a-neighbour fallback
6. A problem row with its buttons always on screen
7. A collapsing group with a value clamped to the visible edge
8. One source for status colours, with a named dot
9. Accessible names for shortened or painted widgets
10. Exact-width combos in a popup
11. Drop zones and a card highlight
12. Enter and a focus guard
13. The safety gate of a review panel (shape)

## 1. Panel order, a full-width bottom panel, and a strip with its own id
```rust
// ui/mod.rs: header first, then the bottom panel, then the columns, then the centre.
header::draw(ui, model, None);
bottom::draw(ui, model, &mut state.drawer); // full window width because it comes before the side panels
// ... column_panel(Textures), column_panel(Sets), then the centre

// ui/bottom.rs
if !model.bottom.open {
    // Its own salt, so the strip's height never becomes the open panel's.
    panel_ids::bottom(ui, "bottom-strip").show(ui, |ui| { ui.horizontal(|ui| tab_buttons(ui, model)); });
    return;
}
panel_ids::bottom(ui, "bottom").resizable(true).default_size(240.0).show(ui, |ui| { /* tabs + body */ });
```

## 2. A resizable column: range, chosen width, truncation, blur on press
```rust
// Too narrow a window: the minimum wins.
let range = *range.start()..=range.end().max(*range.start());
use egui::containers::panel::PanelState;
// panel_ids::left records the id egui keys this panel by: parent.scope_id().with(id), not Id::unique(id).
let panel = panel_ids::left(ui, id).default_size(width).size_range(range);
let panel_id = panel_ids::panel_id(ui.ctx(), id).expect("the panel was built through panel_ids");
let chosen = panel_ids::chosen_width_id(id); // an app-owned key: Id::unique(id).with("chosen")
let resize = ui.ctx().read_response(panel_ids::resize_id(panel_id)); // panel_id.with("__resize")
let released = resize.as_ref().is_some_and(|r| r.drag_stopped());
if !released && !resize.is_some_and(|r| r.dragged()) {
    // egui stores the width it drew, clamped to the window; restore the one the user chose.
    let w = ui.data_mut(|d| d.get_persisted::<f32>(chosen)).unwrap_or(width);
    let outer_rect = egui::Rect::from_min_size(ui.available_rect_before_wrap().min,
                                               egui::vec2(w, ui.available_height()));
    ui.data_mut(|d| d.insert_persisted(panel_id, PanelState { outer_rect }));
}
panel.show(ui, |ui| {
    // Text that would not fit truncates instead of holding the column wider than the drag left it.
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
    // Blur text fields on the press, so commit-on-focus-loss runs before the release changes the selection.
    blur_on_press(ui, ui.max_rect());
    content(ui, model)
});
// egui stores the released width during `show`.
if let Some(s) = PanelState::load(ui.ctx(), panel_id).filter(|_| released) {
    ui.data_mut(|d| d.insert_persisted(chosen, s.outer_rect.width()));
}
```
The blur-on-press helper the column calls (`ui/mod.rs`):
```rust
pub(crate) fn blur_on_press(ui: &egui::Ui, rect: egui::Rect) {
    if ui.input(|i| i.pointer.primary_pressed()) && ui.rect_contains_pointer(rect) {
        ui.memory_mut(|m| m.stop_text_input());
    }
}
```
Each column's maximum leaves room for the columns after it and for the centre:
`let textures_max = ui.available_width() - SETS_MIN - CENTRE_MIN;`

## 3. Skipping a panel without shifting ids
```rust
if folder && !three_d && set_editor_shown(model, three_d) {
    // The drawer is one panel: keep the later widgets' ids (a context menu that is open) as if it were drawn.
    ui.skip_ahead_auto_ids(1);
} else if folder {
    advanced::draw(ui, model, &mut state.advanced);
}
```

## 4. Columns above a footer
```rust
panel_ids::bottom(ui, "review-footer").show(ui, |ui| { /* what will be written, Close, Export */ });
// `columns` sizes to max_rect, which still reaches under the footer; bound it to the space left above it.
ui.allocate_ui(ui.available_size(), |ui| {
    ui.columns(3, |cols| {
        column(&mut cols[0], "Blockers", |ui| { /* ... */ });
        column(&mut cols[1], "Undecided", |ui| { /* ... */ });
        column(&mut cols[2], "Warnings", |ui| { /* ... */ });
    });
});
```

## 5. A visible scroll bar and the never-under-a-neighbour fallback
```rust
// The rows' column is narrower than the rows: scroll them sideways, clipped to the column.
// The default floating bar is invisible until hovered (dormant opacity 0); thin is always drawn.
// look::thin_scroll() is ScrollStyle::thin() with egui 0.37's rubber band turned off.
ui.scope(|ui| {
    ui.spacing_mut().scroll = crate::look::thin_scroll();
    egui::ScrollArea::horizontal()
        .id_salt(("slot-rows-scroll", &v.id))
        .show(ui, |ui| {
            egui::Grid::new(("slot-rows", &v.id)).num_columns(6).striped(true).show(ui, |ui| { /* rows */ });
        });
});
```
The same applies to a side panel with children wider than it: `ScrollArea::both()` inside the same scoped style.

## 6. A problem row with its buttons always on screen
```rust
egui::ScrollArea::vertical().auto_shrink([false, false])
    .show_rows(ui, row_h, list.len(), |ui, range| {
        for p in &list[range] {
            // The right side is laid out first, so the sentence is cut to the room left.
            let (l, r) = egui::Sides::new().shrink_left().truncate().show(
                ui,
                |ui| {
                    dot(ui, status_of(p.level));
                    let a = ui.link(&p.where_).clicked().then(|| Act::Go(p.key.clone()));
                    ui.add(egui::Label::new(&p.text).truncate()); // truncate() already shows the full text on hover
                    a
                },
                |ui| {
                    // Right to left: add the buttons in reverse reading order.
                    ui.button("Acknowledge").clicked().then(|| Act::Ack(p.key.clone()))
                },
            );
            if let Some(a) = l.or(r) { act = Some(a); }
        }
    });
```
(Tessalith also added `.on_hover_text(&p.text)`. The final review noted that the tooltip then shows the sentence twice, so leave it out.)

## 7. A collapsing group with a value clamped to the visible edge
```rust
pub fn group<R>(ui: &mut egui::Ui, id: impl std::hash::Hash, title: &str,
                value: Option<egui::RichText>, default_open: bool,
                body: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        // shortcut: egui ids need Debug (AsIdSalt); hash to u64 first. Upgrade if egui drops the bound.
        let hasher = std::hash::BuildHasherDefault::<std::hash::DefaultHasher>::default();
        let id = ui.make_persistent_id(std::hash::BuildHasher::hash_one(&hasher, id));
        let state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
        let mut clicked = false;
        let mut h = state.show_header(ui, |ui| {
            // A wider row above widens max_rect (egui unions it); bound to the visible edge instead.
            let right = ui.max_rect().right().min(ui.clip_rect().right() - ui.spacing().item_spacing.x);
            // Placer::set_min_width measures from the cursor, which is past the collapser here.
            ui.set_min_width((right - ui.cursor().min.x).max(0.0));
            // show_header toggles on the icon only; make the title toggle too.
            clicked = ui.add(egui::Label::new(egui::RichText::new(title).strong())
                             .sense(egui::Sense::click())).clicked();
            if let Some(v) = value.filter(|_| ui.cursor().min.x < right) {
                let mut r = ui.available_rect_before_wrap();
                r.max.x = right;
                ui.scope_builder(egui::UiBuilder::new().max_rect(r)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| ui.label(v));
            }
        });
        if clicked { h.toggle(); }
        h.body(body).2.map(|r| r.inner)
    }).inner
}
```
To make a group open when its feature turns on, put the on/off state in its id: `group(ui, ("pgp", g, on, set), ..)`.

## 8. One source for status colours, with a named dot
```rust
//! ui/status.rs: red blocks the write, amber warns or needs a decision, green is ready.
//! Teal (visuals.selection) is the only non-status colour.
pub const AMBER: egui::Color32 = egui::Color32::from_rgb(230, 160, 30);
pub const RED: egui::Color32 = egui::Color32::from_rgb(210, 60, 60);
pub const GREEN: egui::Color32 = egui::Color32::from_rgb(90, 170, 90);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status { Blocks, Warning, Ready }

pub fn dot(ui: &mut egui::Ui, s: Status) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(egui::Vec2::splat(8.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, s.color());
    // Not colour alone: the dot has a name ("ready", "warning", "blocks export").
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, s.label()));
    r
}
```

## 9. Accessible names for shortened or painted widgets
```rust
// The button text is elided in the middle; the name and the hover keep the whole path.
let name = format!("Export to: {full}");
let enabled = ui.is_enabled();
r.response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &name));
r.response.on_hover_text(full); // the name is set, so egui 0.37 keeps the hover as the description only

// The tab shows "Textures"; its name says what it is.
r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, "Textures view"));

// A painted letter chip or an icon button gets a role and a name.
let r = ui.add_enabled(enabled, egui::Button::new("+"));
r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, "Larger tiles"));

// ui/mod.rs: E28's helper. A short name for an input that nothing visible beside it names;
// it wins over the hover sentence egui 0.37 copies into an empty name, whichever is set first.
pub(crate) fn name_input(r: egui::Response, name: &str) -> egui::Response {
    r.accessible_name(name)
}

// ui/meshes.rs: the mod filter combo (no label of its own) and the filter box inside it, each
// named; the combo is named before its hover text. `show_ui_036` (with `use crate::look::Combo036 as _;`)
// shows it in a row of its own, as egui 0.36.2 did; every Tessalith combo goes through it.
let r = egui::ComboBox::from_id_salt("meshes-mod-picker")
    .selected_text(text)
    .width(size.x)
    .truncate()
    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
    .show_ui_036(ui, |ui| {
        let r = ui.add(
            egui::TextEdit::singleline(&mut q)
                .margin(crate::look::TEXT_EDIT_MARGIN)
                .hint_text("Filter: words of the mod name")
                .desired_width(f32::INFINITY),
        );
        let r = super::name_input(r, "Filter mods by name");
        // .. the rows
    })
    .response;
super::name_input(r, "Mod filter")
    .on_disabled_hover_text("Available after the scan finishes");

// ui/settings_panel.rs: egui 0.37's labelled_by clears a control's own name. Name again the one
// control whose own name says more than its row label.
const THRESHOLD_NAME: &str = "Alpha threshold";
pub(super) fn own_name_after_link(r: egui::Response, field: &str) -> egui::Response {
    match field {
        "alpha.threshold" => super::name_input(r, THRESHOLD_NAME),
        _ => r,
    }
}
// ui/bulk_panel.rs: called right after the link.
own_name_after_link(control.labelled_by(label.id), r.field);
```

## 10. Exact-width combos in a popup
```rust
// ComboBox::width is only a minimum, and a combo with truncated text takes all the room left.
const POPUP_COMBO_WIDTH: f32 = 220.0;
let h = ui.spacing().interact_size.y;
for n in 0..COMBOS {
    ui.allocate_ui(egui::vec2(POPUP_COMBO_WIDTH, h), |ui| combo_at(ui, model, &o, n));
}
// The menu that holds them keeps open on clicks inside:
let config = MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
```

## 11. Drop zones and a card highlight
```rust
// dnd_drop_zone takes any payload in flight on release, so draw a plain frame while another kind is dragged.
fn item_zone(ui: &mut egui::Ui, editing: bool, frame: egui::Frame, add: impl FnOnce(&mut egui::Ui))
    -> (egui::Response, Option<std::sync::Arc<DragItem>>) {
    if editing && !egui::DragAndDrop::has_payload_of_type::<DragSet>(ui.ctx()) {
        let (zone, p) = ui.dnd_drop_zone::<DragItem, ()>(frame, add);
        (zone.response, p)
    } else {
        (frame.show(ui, add).response, None)
    }
}
// While anything is dragged, the zone repaints its own frame: pass Frame::NONE and draw the card look on an inner Frame.
// Then repaint the hover cue yourself (B3 final I1, not yet landed when this was written):
// if DragAndDrop::has_payload_of_type::<DragItem>(ctx) && zone.contains_pointer() {
//     ui.painter().rect_stroke(zone.rect, 12.0, ui.visuals().widgets.active.bg_stroke, StrokeKind::Inside);
// }
```

## 12. Enter and a focus guard (the call site is a sketch)
```rust
// The dialog's Enter: only while no widget has focus, and never on the frame the dialog opened.
Keys {
    cancel: shown.should_close() && (outside || !outside_click),
    enter: opened_before(ctx, id)
        && ctx.memory(|m| m.focused().is_none())
        && ctx.input(|i| i.key_pressed(egui::Key::Enter)),
}
// The caller maps Enter to the safe default: `keep |= k.cancel || k.enter;`, never to Overwrite.

// A button that saves refuses while a text field holds an uncommitted value.
pub(super) fn field_focused(ui: &egui::Ui, model: &mut AppModel, what: &str) -> bool {
    if !ui.ctx().egui_wants_keyboard_input() { return false; }
    model.status_line = format!("{what}: leave the text field first");
    true
}
if ui.add_enabled(gate.is_ok(), export_button).clicked() && !field_focused(ui, model, "Export") {
    model.export_from_review();
}
```

## 13. The safety gate of a review panel (shape)
```rust
// Called on the click and again when a queued click's plan lands. Never called by anything else.
fn run_continue_review(&mut self) {
    if self.review.is_none() { return; }                    // closed: the queued click is dropped
    if self.plan_in_flight() { self.queue_review(); return; } // never on a stale plan
    if let Err(why) = self.export_stopped() { self.status(why); return; }
    if let Err(why) = self.review_gate() { self.status(format!("Export not started: {why}")); return; }
    self.start_export(); // moves the review's answers out; finish_json takes them on entry
}
// At write time: apply a stored "overwrite" answer only if the file's hash still equals the one stored with it.
```
