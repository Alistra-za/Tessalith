# egui 0.37.0 gotchas found in Tessalith

Each entry gives the symptom, the cause in egui, the fix, and the source. File and line references point into the egui 0.37.0 sources in the cargo registry (`egui-0.37.0/src/`), unless an entry names another crate or version. On another egui version, open that version's source and check the entry before you rely on it.

The entries were first found on egui 0.36.2 (Tessalith layout v2) and checked again on 0.37.0, by source read and by the tests in Tessalith's `crates/gui/tests/egui_gotchas.rs`, `accepted_037.rs` and `accessible_names.rs`. Source tags are explained in `SKILL.md`.

## Contents
0. Names that changed in 0.37
1. Panels and columns
2. Widths and scroll areas
3. Collapsing headers and ids
4. Drag and drop
5. Windows, menus and popups
6. Labels, text and AccessKit
7. Layout helpers
8. The default look

## 0. Names that changed in 0.37

| 0.36.2 | 0.37.0 | Source |
|---|---|---|
| `egui::Id::new(x)` | `Id::unique(x)` for a global id (same value: `new` calls `unique`); `parent.with(x)` or `IdSalt::new(x)` for a local one | `id.rs:91-104` |
| `Ui::id()` | `Ui::scope_id()` (same field) | `ui.rs:363-372` |
| `UiBuilder::id(..)` | `UiBuilder::scope_id(..)` | `ui_builder.rs:73-81` |
| `egui::WidgetType::X` | `egui::Role::X` (`WidgetType` is a deprecated alias of `accesskit::Role`). Renamed: `Checkbox` → `CheckBox`, `TextEdit` → `TextInput`, `DragValue` → `SpinButton`, `ColorButton` → `ColorWell`, `CollapsingHeader` → `DisclosureTriangle`, `SelectableLabel` → `Button`, `Panel` → `Pane`, `ResizeHandle` → `Splitter`, `Other` → `Unknown` | `lib.rs:642-659`; `WidgetInfo::labeled(role: Role, ..)` at `data/output.rs:694` |
| `Panel::left/right/top/bottom(id: impl Into<Id>)` | `(id_salt: impl AsIdSalt)`: the panel's id is scoped to its parent `Ui` (section 1) | `containers/panel.rs:252-279` |
| `scroll_area::State::offset` (field) | `clamped_offset()`; `offset()` is deprecated | `containers/scroll_area.rs:121,146` |
| `Event::MouseWheel { unit, delta, phase, modifiers }` | adds `source: MouseWheelSource` | `data/input/event.rs:191-195` (0.36.2: `:150-172`) |

## 1. Panels and columns

**A panel's id is scoped to the `Ui` it is shown in.**
- Symptom: code that reads or writes a panel's `PanelState` under `Id::new("sets")` (its width, its resize handle `Id::new("sets").with("__resize")`) compiles on 0.37 and silently finds nothing. A width restored before `show` is ignored, a "dragged width" never comes back, and a test's `PanelState::load(.., Id::new(name))` returns `None` (an `is_none()` assert then passes vacuously).
- Cause: `Panel::left(id_salt: impl AsIdSalt)` (`containers/panel.rs:252`); the panel's id is `parent_ui.scope_id().with_salt(self.id_salt)` (`:974-976`), and that id is the `PanelState` key (`:907-922`). The same salt in two parents is two panels with a state each. The panel's child `Ui` is built with `.id_salt(id)` (`:833-839`), so every id made inside a panel moves with it. An `Id` passed as the salt is hashed again (`IdSalt::new`, `id_salt.rs:43-50`), so `Panel::top(Id::unique("x"))` is not stored under `Id::unique("x")`. Tuples now work as salts (the old "takes no tuple" limit is gone).
- Fix (generic): take the id from the `Ui` the panel is shown in: `parent.scope_id().with(salt)`, which is `parent.make_persistent_id(salt)` (`ui.rs:914-916`); the resize handle is `that_id.with("__resize")` (`containers/panel.rs:36-38`). Pass a plain string or tuple salt, never an `Id`.
- To let a dragged width carry over between items (one settings panel shown for every selected item), give every item's panel the same salt and show it in the same parent `Ui`: then it is one id with one stored state. The same salt under another parent is another panel with its own width.
- Fix (Tessalith): build every panel through `ui::panel_ids::{left, right, top, bottom}(parent, salt)`, which records the id egui uses. Read it back with `panel_id(ctx, salt)`, `panel_rect(ctx, salt)` and `resize_id(panel_id)`; a size restored before the first build goes through `restore_size` / `pending_size`. `panel_ids::tests::no_panel_is_built_outside_panel_ids` fails on any other `Panel::` constructor (a line scan of `crates/gui/src`, comment lines skipped). Tests use `support::panel(ctx, name)` and `support::panel_never_built(ctx, name)` (pair the latter with a panel the same frame does build).
- In a `build_ui*` harness the closure runs inside a `scope_builder` + `Frame` (egui_kittest `app_kind.rs:60-87`), so a panel built directly in the closure is `h.ui_id().with(salt)` (`lib.rs:618-628`). That does not cover an app harness, whose panels are built by the app's own `Ui`s.
- Source: [E37 T6]; [E37 T11] `a_panel_is_stored_under_its_parents_scope`.

**`ui.columns` ignores a sibling footer panel.**
- Symptom: inside a bottom panel with a `Panel::bottom("footer")` sub-panel, the three columns' widgets drew over the footer and took its clicks (Export, Close, a JSON keep/overwrite choice).
- Cause: `Panel::show` reserves the footer only by moving the parent's cursor (`containers/panel.rs:872-883`, `parent_ui.set_cursor`). `Ui::columns` sizes each column to `self.max_rect().right_bottom().y` (`ui.rs:2675`, `:2732` for the const version), which is the bottom of the whole panel.
- Fix: `ui.allocate_ui(ui.available_size(), |ui| ui.columns(3, ..))`. Passing `.max_height(h)` to each column's ScrollArea doesn't work, because a heading outside the scroll area still pushes the column one line over the footer.
- Source: B1 final I1; Note CA25; `ui/review.rs`. 0.37 source read.

**A panel's size is stored per id.**
- A collapsed strip drawn with the column's salt in the same parent turns the strip's 24 px into the column's width (`PanelState::load(ui, self.id(ui))`, `containers/panel.rs:1117`). Give the strip its own salt (`"textures-strip"`, `"bottom-strip"`).
- Source: CA9; `ui/mod.rs::column_panel`, `ui/bottom.rs`. 0.37 source read.

**`PanelState` stores the drawn width, clamped to the window.**
- Narrow the window and widen it again, and the user's width is gone: egui clamps the size to the range and the available size (`containers/panel.rs:784-785`) and stores the drawn rect (`:919-922`).
- Fix: keep a persisted "chosen" width of your own. Write it on drag release (`ctx.read_response(panel_id.with("__resize"))`, `drag_stopped`) and write it back into `PanelState { outer_rect }` under the panel's scoped id before `show` while no drag is in progress. Tessalith: `ui::chosen_width_panel` with `panel_ids::panel_id`, `resize_id` and `chosen_width_id`.
- A panel is also only as wide as its content (at least its minimum): `show` stores the frame's response rect. A width test needs content that fills the panel.
- Source: `ui/mod.rs::chosen_width_panel`; [E37 T6].

**An inverted `size_range` is flipped silently.**
- When `avail - CENTRE_MIN/2` drops below the minimum, egui flips the range (`as_positive`, `containers/panel.rs:706`, again in `clamp_to_range` at `:1425`), so neither bound holds.
- 0.37 also caps the range's max at the available size right after the flip (`:707-710`). When the parent is narrower than the minimum, that leaves the range inverted again (min 300, max 136 in a measured case). The panel's reported rect is then cut to that max (`:863-866`) and displaced: with a 300 px `exact_size` panel in a narrower window it was 136 wide and lay wholly outside the window.
- Fix, for a parent at least `min` wide: `min..=(avail - x).max(min)`. The range is then never inverted, and when the room left for the column is under its minimum, the minimum wins.
- A parent narrower than `min` breaks both bounds whatever range you pass: the content is still laid out at least `min` wide (`set_min_width(panel_axis_min)`, `containers/panel.rs:847-858`), but the reported rect is cut to the available size (`:707-710`, `:863-866`) and displaced, so the panel's content overflows its parent. Keep the parent at least `min` wide (a minimum window size, or collapse the panel to a strip first).
- Source: B2 pre K20; CC2; [E37 T11] side note of `a_nested_panel_is_clipped_to_its_parent`.

**Skipping a panel shifts every later auto id.**
- Symptom: when the Advanced drawer wasn't drawn, a set's context menu closed on the next frame.
- Cause: one `Panel::show` makes one `new_child` (`containers/panel.rs:833`), which takes one auto id. egui calls `skip_ahead_auto_ids(1)` itself for a collapsed panel (`:470`).
- Fix: `ui.skip_ahead_auto_ids(1)` in the branch that skips the panel.
- Source: B2 task 2.3 report; CC3; `ui/mod.rs`. 0.37 source read.

**A panel inside a narrower parent is clipped to the parent.**
- 0.37 intersects the panel's clip rect with its parent's (`shrink_clip_rect`, `containers/panel.rs:844`; the central panel at `:1409`). 0.36.2 replaced it (`set_clip_rect`). A `Panel::right(..).exact_size(300.0)` inside a 150 px `Window` paints only inside the window.
- Its `max_rect` and reported `response.rect` still reach past the parent (the rect can lie wholly outside the window), so don't hit-test or lay out by them.
- Source: [E37 T11] `a_nested_panel_is_clipped_to_its_parent`.

**A panel's resize handle is an input with a name.**
- Each resizable panel's handle has `Role::Splitter`, named "Resize panel", with the size as a fraction for its value (`containers/panel.rs:1201-1205`). Splitter is an input role, so kittest's unnamed-widget check covers it; egui names it. Panels themselves are `Role::Pane`.
- Source: 0.37 source read.

## 2. Widths and scroll areas

**Floating scroll bars are invisible until hovered, and `AlwaysVisible` doesn't change that.**
- Cause: the default `ScrollStyle::floating()` has `dormant_handle_opacity: 0.0`, `dormant_background_opacity: 0.0` and `floating_allocated_width: 0.0` (`style.rs:589-592`, `:648-658`). The dormant opacities apply whatever `ScrollBarVisibility` is (`containers/scroll_area.rs:1607-1632`).
- Fix: in a scope, set the scroll style to `ScrollStyle::thin()` (6 px allocated, dormant opacity 1, `style.rs:624-643`) or `solid()`. In Tessalith assign `look::thin_scroll()` (`thin()` with the rubber band off): `ui.spacing_mut().scroll = crate::look::thin_scroll();`, and `sdc_gui::look::thin_scroll()` in tests. `look::tests::scroll_bars_use_thin_scroll` (a text scan of `crates/gui/src`) fails on a preset outside `look.rs` and on a whole scroll style set to anything but `look::thin_scroll()` or `crate::look::thin_scroll()`. A horizontal bar takes height, not width.
- Every shown bar has an AccessKit node with `Role::ScrollBar` and no name (`containers/scroll_area.rs:1471-1473`), under the thin style and under the default floating one alike, even while the floating bar is drawn at opacity 0. A bar is shown only when needed (`:1383-1387`): content that fits has none, a tall vertical area has 1, a wide and tall `both` area has 2. So kittest can see whether a bar exists, not whether it is visible. ScrollBar is not an input role, so the unnamed-widget check ignores it.
- Source: B2 final I2; CC21; [E37 T11] `a_scroll_bar_has_an_accesskit_node`.

**Widgets widen the content of later groups.**
- Symptom: after one group wider than the 300 px panel (a colour row, a 390 px card), every later group's right-aligned header value was drawn past the clip. Six of nine values vanished.
- Cause: `Region::expand_to_include_rect` unions `max_rect` as well as `min_rect` (`layout.rs:52-55`). Inside a scroll area, the wide child widens `max_rect` for everything after it.
- Fix: bound right-aligned content by `ui.max_rect().right().min(ui.clip_rect().right() - item_spacing.x)`. The inset keeps the last glyph off the clip edge. Then put the value in a `scope_builder` with a right-to-left layout and that `max_rect`.
- Source: B2 final I1; CC20; `ui/group.rs`. 0.37 source read.

**`Placer::set_min_width` measures from the cursor, not from the frame edge.**
- Symptom: after the fix above, every group was about 18 px (the collapser's width) too wide, so every PBR panel scrolled sideways and the frame borders were clipped.
- Cause: `set_min_width` asks for the next widget's space from the cursor (`placer.rs:263-270`).
- Fix: `ui.set_min_width((right - ui.cursor().min.x).max(0.0))`.
- Unit test: a group inside a 300 px `ScrollArea::horizontal` must give `content_size.x <= inner_rect.width()`.
- Source: B2 progress (final re-review); CC20; `ui/group.rs` tests. 0.37 source read.

**A vertical-only ScrollArea lets a wide child push its panel off the window.**
- Symptom (0.37): a 390 px card in a `ScrollArea::vertical` inside `Panel::right(..).size_range(200..=300)`. The panel's reported width stays 300, but its rect is pushed about 100 px past the window edge, and its content covers the centre by about 100 px. On 0.36.2 (CC2) the panel widened past its range and the layout never settled.
- Cause: the axis that does not scroll follows the content (`containers/scroll_area.rs:1291-1296`), so the frame grows. egui then cuts the reported rect to the range's max from the far edge (`containers/panel.rs:863-866`, with `set_rect_size` at `:157-162`), and the parent's cursor stops at the visible rect's edge (`:872-883`).
- Fix: use `ScrollArea::both` for a panel whose children can be wider than it. With `both` the panel stays 300 wide at the window edge and the centre ends before it.
- Source: CC2; [E37 T11] `a_vertical_scroll_area_keeps_the_panels_width_but_pushes_it_off_screen`.

**A row exactly as wide as its column flipped the scroll bar every frame.**
- Measured in Tessalith on 0.36.2: `run()` never settled. A bare probe on 0.37 (a 100 px cell, the gap and a flexible cell sized from last frame's inner width, in a 300 px `ScrollArea::both`) did not reproduce it: the row settled at exactly the inner width with no bar. The bar test is `limit_rect.width().ceil() < content_size.x` (`containers/scroll_area.rs:1310-1313`), the same code as 0.36.2, so the original flip needs a shape the probe did not rebuild. Treat it as live.
- Fix: size the flexible cell from the column width minus the other cells' width from the last frame, minus an 8 px margin.
- Source: B2 task 2.2 report; CC2; [E37 T11] `a_row_as_wide_as_its_column_settles` (not reproduced).

**`ComboBox::width` is a minimum.**
- "The combo box will always have at least this width" (`containers/combo_box.rs:386-388`). 0.37 builds the combo from atoms, and a truncated text still takes all the room left. Give it an exact slot with `ui.allocate_ui(vec2(W, interact_h), ..)`.
- Source: AG124 (cited in `ui/columns.rs::filter_popup`). 0.37 source read; measured equal to 0.36.2 in the upgrade survey.

**A ScrollArea takes every scroll target set inside it.**
- A `scroll_to_me` request made inside a nested horizontal area is consumed there (`containers/scroll_area.rs:1032-1034`, `mem::take` of the pass's `scroll_target`) and never reaches the outer vertical one. Tessalith re-issues it after the inner area ends.
- Source: CC18; B2 final Minor 12 (marked fragile). 0.37 source read.

**The wheel scrolls during a drag.**
- 0.36.2 ignored the mouse wheel while any widget was dragged. 0.37 scrolls a hovered area whatever is dragged (`containers/scroll_area.rs:1326-1329`; the `dragged_id().is_none()` test now gates only hover, `:1322-1324`). A drag can therefore reach rows that are off screen.
- Tessalith accepts this (owner ruling): the wheel scrolls the Sets column during a set drag, and a release on a card the wheel brought up reorders. Don't add code that reproduces the 0.36.2 limit.
- A nested scroll area at its end passes the wheel on to the area it sits in (`containers/scroll_physics.rs:213-225`: no scroll possible means the delta is not consumed).
- Source: [E37 T9] `set_order::the_wheel_scrolls_the_sets_during_a_drag`, `accepted_037::the_wheel_passes_to_the_outer_list_at_the_inner_lists_end`.

**Rubber-banding is on by default, and `ScrollStyle::thin()` carries it.**
- Symptom: a reveal or kinetic scroll keeps repainting, and kittest's `run()` exceeds `max_steps` ("It would have settled after 40 steps"; repaint cause `physics.step`). A drag past the end of a list draws the first row below its rest position.
- Cause: `ScrollStyle::kinetic` (`KineticScrollStyle`, `style.rs:851-884`) defaults to `decay_time 0.5`, `stop_distance 0.5`, `rubber_band true`. Rubber-banding applies to drag-to-scroll (touch) and trackpads, not wheels or bars (`:869-874`; `containers/scroll_physics.rs:216-221`). Every preset (`thin()`, `solid()`, `floating()`) keeps the default `kinetic`.
- Fix: set `style.spacing.scroll.kinetic.rubber_band = false` on the style, and again on every scoped scroll style. Tessalith: `look::pin_036_style` turns it off for both themes, and every scoped style is `look::thin_scroll()` (see "Floating scroll bars are invisible until hovered"; its scan does not see a field write such as `.kinetic.rubber_band = true`). The decay stays 0.37's (0.36.2's hard-coded momentum can't be expressed in it).
- Drag-to-scroll is `DragScroll::OnTouch` by default, so a mouse-only test must send one `Event::Touch` first to see it.
- Source: [E37 T5] `look::tests::a_drag_past_the_end_of_a_thin_scroll_list_stops_at_the_edge`.

## 3. Collapsing headers and ids

**`CollapsingState::show_header` toggles only on its icon.**
- Clicking the title does nothing (`containers/collapsing_header.rs:129-150`). Tests and users that click the title see no change.
- Fix: make the title `Label::new(..).sense(Sense::click())`, and call `toggle()` on the header response when it's clicked. The accessible label stays the title.
- Source: B2 pre K15; CC7. 0.37 source read.

**egui's `CollapsingHeader` reports `Role::DisclosureTriangle`, not `Button`.**
- 0.36.2 mapped `WidgetType::CollapsingHeader` to AccessKit `Button`. 0.37's header reports `DisclosureTriangle` with an expanded state (`containers/collapsing_header.rs:588-594`). A test that finds a header by `Role::Button` and its title finds nothing; find it by label, or by `Role::DisclosureTriangle`. DisclosureTriangle is an input role (`accessibility.rs:20`), so a header with no title text fails kittest's name check.
- A painted header you name yourself should say the same: `WidgetInfo::labeled(egui::Role::DisclosureTriangle, ..)`.
- Source: 0.37 source read (Tessalith's one named header is in `ui/meshes.rs`).

**`load_with_default_open` applies only before any state is stored.**
- A group drawn closed stays closed after its feature turns on (`containers/collapsing_header.rs:50-58`).
- Fix: put the on/off state in the id (`("pgp", group, on, set)`), so the "on" group is a new state and opens.
- Source: B2 pre K16; CC9. 0.37 source read.

**`AsIdSalt` (the bound on `id_salt` and `make_persistent_id`) needs `Debug`.**
- `AsIdSalt: Hash + Debug` (`id_salt.rs:7-9`). A helper that takes `id: impl Hash` can't pass it on.
- Fix: hash it to a `u64` first, using `BuildHasherDefault::<DefaultHasher>`, and mark that with a `shortcut:` comment.
- Source: B2 task 2.1 report; `ui/group.rs`. 0.37 source read.

**`make_persistent_id` inside a `Frame` child uses `new_child`'s `scope_id`** (`ui.rs:255-262`, `:914-916`).
- A group's open state doesn't shift when a sibling group is added or removed. That is safe, and was checked. (0.36.2 called the field `stable_id`.)
- Source: B2 final Strengths. 0.37 source read.

**Don't key a per-keystroke cache with egui `Id`s in a debug build.**
- In debug builds 0.37 records the source of every new `Id` in a map that is never cleared (`id.rs:94-95`, `Id::unique` calls `id_source::insert_root`; `with` does the same, `:124-125`). A cache keyed by `Id::unique(text)` grows one entry per keystroke.
- Fix: key such caches with a plain std hash (`BuildHasherDefault::<DefaultHasher>::hash_one`).
- Source: [E37 T10] (`ui/input_name.rs`, `cache_key`).

## 4. Drag and drop

**`dnd_drop_zone` repaints its frame while anything is dragged.**
- 0.37: a zone whose `Frame` has its own fill or stroke keeps them only while nothing is being dragged. While any payload is in flight, or always for a frame with no fill and no stroke, egui overwrites them with `widgets.inactive` or `widgets.active` (`ui.rs:2842-2865`). (0.36.2 overwrote them every frame.) So a selection highlight set on that frame vanishes during every drag.
- Fix: give the zone `Frame::NONE`, and draw the card look (fill, selection stroke) on an inner `Frame`.
- Trade-off: the inner frame covers the zone's 1 px white hover stroke, so the drop-target cue nearly disappears. Fix (verified by a shapes test): paint the hover stroke with `ui.painter()` after the inner frame, over `zone.rect`, while a matching payload is in flight and the pointer is over the zone.
- Source: B3 task 3.2 report; CD9; B3 final I1, fixed in 0d815a1 (CD20). 0.37 source read.

**`dnd_drop_zone` takes whatever payload is in flight when the pointer is released, even one of another type.**
- `dnd_release_payload` calls `DragAndDrop::take_payload` (`response.rs:588-596`), which takes the payload out before the type check (`drag_and_drop.rs:116`, `payload.take()?.downcast().ok()`), so a wrong-type payload is consumed and lost.
- While a different payload is being dragged (a set being reordered), draw a plain frame instead, so the other payload survives.
- Source: `ui/sets_column.rs::item_zone`. 0.37 source read.

**A Grid cell can't hold a row-wide click or drop.**
- A row-wide `ui.interact` registered after the cells takes their clicks. Within a layer, egui keeps widgets back to front in the order they were first registered (`widget_rect.rs:151-155`; a new id is pushed at the end, `:199-205`). When several click-sensing widgets contain the pointer, the hit test takes the last of them, "the one on top" (`hit_test.rs:208-212`, tie rule at `:430-433`). Registering the same id again later in a frame updates its rect and sense in place and does not move it up (`widget_rect.rs:206-215`).
- Fix: register `ui.interact(last_frame_row_rect, id, Sense::click())` before the row's cells, then store the union of the cell rects after `end_row`. This is what `UiBuilder::sense` does internally. On the first frame there is no rect, and during a scroll the rect is one frame stale.
- New in 0.37: `Response::container_clicked()` is true when a click lands anywhere in a container's rect, a child included (`response.rs:373-381`). It suits a row built in a `scope`, not a Grid row.
- Source: B2 pre K1; CC1. 0.37 source read.

## 5. Windows, menus and popups

**`Window::from_viewport` ignores `ViewportBuilder::position`.**
- It reads only title, sizes, resizable, decorations and the minimise button (`containers/window.rs:126-158`). An embedded fallback window is placed by egui, so you can't place it yourself.
- Source: global constraints R19 (branches 1-3). 0.37 source read.

**A popup closes only on a click whose press started while it was open.**
- 0.36.2 closed a popup on any click in a frame after it opened. 0.37 tracks whether the press started while the popup was open, and only such a click may close it (`containers/popup.rs:659-682`). A popup opened on a press (a right-press context menu) no longer closes on the release.
- A pick in a list still closes it: Tessalith's "Choose preset…" list closes on the pick.
- Source: [E37 T9] `problems_accept::choose_preset_opens_the_preset_list_and_a_pick_closes_it`.

**A popup combo inside a menu.**
- Symptom: in a `MenuButton` with `PopupCloseBehavior::CloseOnClickOutside` holding a `ComboBox`, a click on the combo closes the menu. The combo's list never shows, and no value can be picked. A click outside and Esc still close the menu after a reopen.
- Cause: egui keeps one open popup per viewport. `open_popup` replaces the single entry, and `is_popup_open` compares that one id (`memory/mod.rs:1115-1129`). Opening the combo's popup closes the menu's. (`CloseOnClickOutside` also counts a click on another layer as "elsewhere", `response.rs:279-296`.) This is the same on 0.36.2.
- Fix: draw the menu as your own small popup with its own open state (Tessalith `ui/columns.rs::combo_menu`), not a `MenuButton`. Hold the global keyboard shortcuts while it is open, as a menu would. A first click outside then closes only the combo, and a second closes the menu.
- Test: pick in one combo, then in a second without reopening, and assert both values. Then check that a click outside, reopening, and Esc still work (Tessalith `textures_filter::two_filters_are_picked_through_the_filter_popup_without_reopening_it`).
- Source: B3 final I2, fixed in 731be52 (CD21); [E37 T11] `two_combos_in_a_menu_open_and_close_in_order`.

**A menu that holds a filter box or a scrolled list must not close on a click inside.**
- Use `MenuConfig::new().close_behavior(PopupCloseBehavior::CloseOnClickOutside)` (`containers/menu.rs:85`, `:119-124`), and call `ui.close()` on a pick.
- 0.37 adds arrow-key navigation in menus (`containers/menu.rs:232-287`). It never takes a key from a focused text box: a TextEdit's event filter takes both arrow axes (`widgets/text_edit/builder.rs:148-151`), and `Memory` turns an arrow key into a focus move only when the focused widget's filter does not take it (`memory/mod.rs:581-607`). So with the filter box focused, typing, Left, Right, Home, End, Backspace, Down and Up all stay in the box and the menu stays open. With nothing focused, Down focuses the first focusable entry, which is the filter box when the menu starts with one; in a menu without a box, Down walks the rows and Enter picks one (the menu stays open after that Enter).
- Source: `ui/path_picker.rs::menu` (AG125); [E37 T9] `popups::arrow_keys_and_typing_stay_in_the_input_name_filter`; [E37 T11] `arrow_keys_stay_in_a_menus_filter_box`.

**A popup's AccessKit nodes sit under the widget that opened it.**
- `Popup` passes its anchor widget to `Area::accessibility_parent` (`containers/popup.rs:642-644`). In tree order a popup's nodes now come right after its opener, before the opener's later siblings. A test that picks "the last spin button" in tree order can land on a widget outside the popup.
- Fix: find popup widgets by role and name.
- Source: [E37 T9] (`view_menu`'s swatch test).

**`menu_button` draws no ▾ arrow.**
- `MenuButton::new` builds a plain `Button` from its atoms (`containers/menu.rs:404-406`). Put the arrow in the label text if you want one.
- Source: CD3. 0.37 source read.

## 6. Labels, text and AccessKit

**`Label::truncate()` changes the click target's size.**
- `Truncate` sets the wrap width to the available width and one row (`widgets/label.rs:239-242`). Tests that click a title by its exact label can miss. Shorten the string instead of truncating a clickable title.
- Source: B2 final I1 note. 0.37 source read.

**A label in `ui.horizontal` uses `TextWrapMode::Extend`.**
- `Ui::wrap_mode` gives `Extend` in a horizontal layout that doesn't wrap (`ui.rs:615-631`). One long title widens every row below it. Shorten the title, and move the hint into the body or a hover. (The general rule is in egui-break-ui.)
- Source: CC22. 0.37 source read.

**`response.widget_info(..)` after `ui.add` replaces the label egui set.**
- `widget_info` refills the node from the new `WidgetInfo` (`response.rs:951-976`, `fill_accesskit_node_from_widget_info` at `:1008-1047`). An empty label is no longer set (`:1020-1021`), so it doesn't hide a `labelled_by` name. For `Role::Label` the text goes into the node's value, not its label (`:1022-1023`).
- Use it to give a short visible text a full accessible name. To set only the name, `Response::accessible_name(name)` is new in 0.37 (`:1087-1093`); it changes the label and nothing else.
- Source: B3 pre A14; CD1. 0.37 source read.

**`on_hover_text` names a widget that has no name.**
- `on_hover_text` and `on_hover_text_at_pointer` always set the text as the node's description when it has none, and set it as the name only when the role is not `Label`, `labelled_by` is empty and the label is empty (`response.rs:766-768`, `:785-795`, `describe_for_accessibility` at `:799-816`). None of this needs a hover.
- So an icon button with only a tooltip is named by the whole tooltip sentence, and so is an unlabelled combo with a hover. A "Save" button keeps "Save"; a control linked with `labelled_by` keeps its label.
- Fix: name the widget explicitly (`accessible_name`, or Tessalith's `ui::name_input`) with a short name. It wins over the hover text whichever is set first.
- Source: [E37 T2]; [E37 T9] `texture_panel_layout::a_linked_control_keeps_its_label_under_a_hover`; [E37 T11] `hover_text_names_an_unnamed_button`.

**`Response::labelled_by` clears the widget's own name.**
- 0.37's `labelled_by` clears the node's label before it pushes the relation (`response.rs:1065-1071`); 0.36.2 only pushed the relation. A slider named "Alpha threshold" and linked to a row label "Threshold" is now announced "Threshold".
- Fix: where the own name says more than the label, name it again after the link: `r.labelled_by(l.id).accessible_name("Alpha threshold")`. That rebuilds the 0.36.2 node: its own label plus the relation. Tessalith: `settings_panel::own_name_after_link`.
- Source: [E37 T9].

**Widgets that name themselves.**
- A `Slider` with text links the slider and its value box to the text label; without text, the value box takes the slider's name through `labelled_by` (`widgets/slider.rs:633-652`). The slider and its own value box may share one name.
- A `DragValue` is named by its trimmed prefix and suffix, joined by a space (`widgets/drag_value.rs:433-441`, applied at `:704-709`); `labelled_by` replaces that name. A suffix such as "×" makes a poor name: link the box to its label.
- A `ComboBox` without a label has no name; its selected text is its value (`containers/combo_box.rs:437-442`). With a non-empty label it is `labelled_by` that label (`:477-484`). An unlabelled combo must be named, or kittest's check fails.
- Source: [E37 T9]; 0.37 source read.

**A TextEdit's hint text is an AccessKit placeholder, not a label** (`response.rs:1044-1045`).
- kittest's label filter matches a node's label (a `Label`'s value) and never the placeholder (kittest 0.5.0 `filter.rs:147-160`), so `query_by_label("Search").is_none()` passes even when the box is drawn. Find the box by `Role::TextInput`, or give it a name.
- egui_kittest 0.37's unnamed-widget check counts a non-blank placeholder as a name (egui_kittest `accessibility.rs:46-55`), so a box with only a hint passes `run()`. The check is not a guard against hint-only boxes.
- Source: B3 pre K21; [E37 T11] `a_hint_is_not_a_label`.

**kittest's label query returns the linked control, not its Label.**
- Once a control is `labelled_by` a Label, `get_by_label(text)` drops the Label node and returns the control (kittest 0.5.0 `query.rs:20-40`, the testing-library rule). A test that found the Label and then "the box to its right" now starts from the box itself.
- Fix: find the control by role and name: `get_by_role_and_label(Role::TextInput, "Find")`.
- Source: [E37 T9] (`dialog_keys`).

## 7. Layout helpers

**`horizontal_wrapped` doesn't re-centre widgets placed before a taller one** (`ui.rs:2489-2494`).
- Next to a two-row-tall Save button, earlier widgets sit about `interact_size.y / 2` higher. The offset moves with `interact_size.y` (18 under Tessalith's pin, 20 by 0.37's default).
- Test "one row" as "every rect contains the first widget's centre y, and x increases in order". A 2 px centre check is only valid for widgets of the same height.
- Source: B3 pre K2. 0.37 source read.

**`egui::Sides::new().shrink_left().truncate()` lays out the right side first.**
- The right side is laid out first and the left side gets what's left (`containers/sides.rs:88-93`, `:128-132`, `:170-180`). Inside the right side, right-to-left order reverses widgets, so reverse the order you add buttons in to keep their reading order.
- Source: B1 fix report I2; `ui/problems.rs`. 0.37 source read.

## 8. The default look

**0.37 changed the default spacing and widget visuals.**
- Symptom: after the upgrade, buttons are 8 px wider and 2 px taller (`button("Undo")` 39.6 × 19 became 47.6 × 21), combos and checkboxes 2 px taller, corners rounder, and a header that fit on one row at 1280 px wraps.
- Cause: `button_padding` (4, 1) → (8, 2) and `interact_size` (40, 18) → (40, 20) (`style.rs:1568`, `:1570`). `Widgets::dark()` and `light()` (`style.rs:1791-1880`) round corners 4 (noninteractive) and 6 (the rest) instead of 2 and 3, and the dark inactive and hovered greys moved. A `TextEdit`'s frame now comes from the theme, with `button_padding` as its inner margin and `interact_size.y` as its min height (`theme/default_style.rs:150-165`, the margin at `:155`); 0.36.2 used `Margin::symmetric(4, 2)`. Latin text in Segoe UI lays out the same; the sizes change through spacing and margin.
- Fix (Tessalith, which keeps 0.36.2's look): `look::pin_036_style(&mut Style)` sets 0.36.2's `button_padding`, `interact_size`, both themes' `Widgets` copied field by field, and turns the rubber band off. `fonts::install_system_fonts` applies it with `ctx.all_styles_mut(look::pin_036_style)`, so the app and every kittest harness get it. Every `TextEdit` sets `.margin(look::TEXT_EDIT_MARGIN)` (0.36.2's (4, 2); `look::tests::every_text_edit_carries_the_036_margin` checks `crates/gui/src`, with the one framed box in `ui/input_name.rs` exempt): without it a box under the pin is 19.28 pt tall, not 21.28. A set margin leaves room for the focus stroke (`widgets/text_edit/builder.rs:696-701`), so the height doesn't change on focus.
- Fix (generic): before you pin a pixel number, decide whether you keep the old look or re-measure. Read sizes from `ui.spacing()` instead of hard-coding them.
- Source: [E37 T5] `look::tests::*`; [E37 T11] `text_edit_height_with_and_without_the_036_margin`.

**The style pin doesn't cover two 0.37 changes: the ComboBox row and the submenu arrow.**
- ComboBox: 0.36.2 showed a combo inside a `ui.horizontal` of its own (0.36.2 `containers/combo_box.rs:234`), where its text got `TextWrapMode::Extend`. 0.37 calls `combo_box_dyn` in the caller's layout (`containers/combo_box.rs:265`). In a Grid cell or a vertical layout a long value now wraps (a Format combo went to 3 lines, 53 pt tall, and moved its column about 62 px), and beside a 19 pt button a combo sits 0.5 pt higher in a row 1 px shorter. The ▼ icon is unchanged.
- Submenu arrow: 0.36.2's submenu entry ended in the text glyph "⏵" (0.36.2 `containers/menu.rs:344-347`). 0.37 paints a triangle atom, 0.55 × 0.35 of an `icon_width` square (`containers/menu.rs:481-487`, `:492-511`), so the entry is 1 px wider and the arrow sits 2 px higher.
- Fix (Tessalith): every combo is shown with `look::Combo036::show_ui_036` (`ui.horizontal(|ui| combo.show_ui(ui, f)).inner`), and every menu button goes through `look::menu_button_036`. Inside a menu (`menu::is_in_menu`, `containers/menu.rs:62`) that builds `SubMenuButton::from_button(Button::new(text).right_text("⏵"))` (`:514-518`) and sets `accessible_name(text)`, so the glyph stays out of the name. `look::tests::every_combo_box_is_shown_through_show_ui_036` and `every_menu_button_goes_through_menu_button_036` fail on a plain `show_ui` or `ui.menu_button` anywhere in `crates/gui/src` (line scans).
- Source: [E37 T15a] `look_036::a_long_combo_value_in_a_grid_cell_stays_on_one_line_as_on_036`, `a_combo_beside_a_button_sits_as_on_036`, `a_submenu_entry_ends_in_036s_arrow_glyph`, `the_set_editors_format_column_starts_where_036_put_it`.
