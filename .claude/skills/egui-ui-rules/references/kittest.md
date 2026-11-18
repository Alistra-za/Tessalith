# Testing egui UI with egui_kittest (0.37.0)

These patterns are the ones that caught real layout and safety bugs in Tessalith. Write each layout test RED-first: run it against the broken code, record the failure message, then fix. File and line references are into the egui_kittest 0.37.0 and kittest 0.5.0 sources in the cargo registry.

## Contents
0. egui_kittest 0.37: what the harness does for you
1. Finding nodes without duplicate-label panics
2. Layout pins (RED-first)
3. Clipped rects and scrolling
4. Things kittest cannot see
5. Frames, idleness and tooltips
6. Isolation: app-data folders
7. Making tests bite
8. Cache and per-frame-cost pins

## 0. egui_kittest 0.37: what the harness does for you
- **The unnamed-widget check is on by default** (`builder.rs:51`, `:150-158`); its panic lists each unnamed widget's role, rect and closest named ancestor (`accessibility.rs:26-43`).
  - **Which roles:** every visible, not read-only node whose role is an input in `egui::accessibility::is_input` (egui `accessibility.rs:11-45`): Button, CheckBox, ColorWell, ComboBox, DisclosureTriangle, Link, ListBoxOption, MenuItem*, MultilineTextInput, NumberInput, RadioButton, SearchInput, Slider, SpinButton, Splitter, Tab, TextInput and more. ScrollBar and ProgressIndicator are not inputs, so the check skips them; test a progress bar's name yourself (Tessalith `accessible_names.rs::unnamed_progress`).
  - **What counts as a name:** a non-blank label or a non-blank placeholder (`accessibility.rs:46-55`). A TextEdit with only a hint passes.
  - **When it runs:** at the end of `run()`, `try_run()` and `run_ok()` (`lib.rs:518-521`), and before every snapshot (`snapshot.rs:741`). Not after `step()` or `run_steps()`. The builder calls `run_ok()` once (`lib.rs:298`), so an unnamed widget on the first screen fails the build itself.
  - **Never switch it off: it is the acceptance test for "every interactive widget has an AccessKit name".** Name the widget instead; `it::kittest_checks_are_never_switched_off` fails on `with_accessibility_check(` or `allow_missing_glyphs(` in `crates/gui/tests` or `crates/gui/src` (a line scan; a wrapped harness builder is not seen).
- **Fonts go in `with_context_setup`.** The harness runs one frame inside `build_*` (`lib.rs:247-251`), before the test can call anything, and the default `MissingGlyphPolicy` is `Panic` (`builder.rs:50`; `allow_missing_glyphs()` switches to tofu, `:99-100`). With egui's `default_fonts` off, a harness without fonts panics at build. `with_context_setup` runs before the first frame and before the harness's own theme and settings (`builder.rs:244`, run at `lib.rs:225-227`):
  ```rust
  Harness::builder()
      .with_context_setup(|ctx| { sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts"); })
      .with_size(egui::vec2(1600.0, 1000.0))
      .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m)
  ```
  In Tessalith `install_system_fonts` also applies `look::pin_036_style`, so every harness gets the app's look.
- **`build_ui*` still wraps the closure** in `ui.scope_builder(..)` + `Frame::central_panel` (`app_kind.rs:60-87`). The closure's `Ui` is not the root, so a panel's id can't be derived from the context alone. **`Harness::ui_id()`** (`lib.rs:618-628`) returns that inner `Ui`'s `scope_id`: a panel built directly in the closure has the id `h.ui_id().with(salt)`. It panics for an eframe-app harness. An app's own panels are read back through the app's id records (Tessalith `support::panel`).
- **`Event::MouseWheel` needs `source`.** egui 0.37 added `source: MouseWheelSource` (egui `data/input/event.rs:191-195`). A wheel turn is spread over a few frames, so step after it:
  ```rust
  h.event(egui::Event::MouseWheel {
      unit: egui::MouseWheelUnit::Point,
      delta: egui::vec2(0.0, dy),
      source: egui::MouseWheelSource::Wheel,
      phase: egui::TouchPhase::Move,
      modifiers: Default::default(),
  });
  h.run_steps(10);
  ```
- **`click()` sends real pointer events** (a hover, then press and release at the node's centre, `node.rs:61-79`), so popup close rules (a press that started while the popup was open) apply to test clicks too.

## 1. Finding nodes without duplicate-label panics
- **`get_by_label` and `get_by_*` panic on more than one match** (kittest 0.5.0 `query.rs:58-70`). Common duplicates: the same slot name in two panels ("Diffuse" in the slot rows and in Matching), a tab and a group that are both "Material", two "Filter ▾" buttons, four "Assign…" buttons, chips named "D" on every card. [B2 pre K5, K6, K11; B3 pre K11, K15]
- **Scope by the panel's rect.** Read the panel's rect from its `PanelState` under the panel's scoped id (egui 0.37 keys it by `parent.scope_id().with(salt)`; `Id::new(salt)`, now `Id::unique(salt)`, finds nothing). Tessalith's `support::panel` reads it through `ui::panel_ids`:
  ```rust
  let panel = support::panel(&h.ctx, "sets"); // panics "panel sets was never built"
  let d = h.query_all_by_label("D").find(|n| panel.contains(n.rect().center())).unwrap();
  ```
  To check that a panel is absent, use `support::panel_never_built(&h.ctx, name)`, and pair it with a panel the same frame does build, so a renamed panel can't make it pass. In a `build_ui*` harness with the panel built directly in the closure, the id is `h.ui_id().with(salt)`.
- **Scope by role** when a label is shared: `h.get_by_role_and_label(Role::Button, "Material")` finds the tab, not the group title. In 0.37 a bare label query also finds the Label beside a named control, a panel, a popup or a tooltip of that name. Tessalith's `support::widget(h, role, name)` returns the one node of that role and name and panics with every node of that name otherwise; `support::header_undo(h, verb)` finds the header's Undo or Redo by exact name.
- **A linked control hides its Label from label queries.** Once a box is `labelled_by` a Label, `get_by_label(text)` returns the box, not the Label (kittest 0.5.0 `query.rs:20-40`). Don't find the Label and then "the box to its right"; find the box by role and name.
- **Hover-only names.** egui 0.37 copies a hover sentence into the name of a widget that has none, so a `label_contains` search can match words that are on screen only while the tooltip shows. Tessalith's `support::shown_containing(h, text)` leaves those nodes out.
- **Leftmost or topmost helper.** When the same label appears in two columns, use one helper per suite:
  ```rust
  fn slot_rect(h: &Harness<'_, State>, label: &str) -> egui::Rect {
      h.query_all_by_label(label).map(|n| n.rect()).min_by(|a, b| a.min.x.total_cmp(&b.min.x)).unwrap()
  }
  ```
- **Pick one of several buttons by the row it's in:** filter `query_all_by_label("Assign…")` by the y-range of the "Height _p" row.
- **A TextEdit's hint is a placeholder, not a label.** Find the box by `Role::TextInput`, or by its name. A `query_by_label("Search").is_none()` assertion passes even when the box is there (kittest 0.5.0 `filter.rs:147-160` matches the label, or a Label's value). [B3 pre K21; E37 T11]
- **A combo's selected text is its value, not its name** ("Sub-type: Any"). egui 0.37 gives an unlabelled combo no name, so the app must name it, and the test finds it by role and name: `h.get_by_role_and_label(Role::ComboBox, "Sort order")`. To match the value, use `h.get_by_value("Sub-type: Any")` (kittest 0.5.0 `query.rs:210-219`), or `query_all_by_value(..)` filtered by role. [B3 task 3.3; E37 T2]
- **Painted widgets need `widget_info` before tests can find them at all** (letters, dots, icon toggles). Pass an `egui::Role` (`WidgetType` is a deprecated alias in 0.37). [B3 pre K11]

## 2. Layout pins (RED-first)
Each one names the edge it guards, so the failure message points at the bug:
- **"No widget past its column edge."** For every button in a row, assert `node.rect().max.x <= panel.max.x`. The RED message read: `Go to set ends at 1297.97`. [B1 fix I2]
- **"A header value stays inside the panel."** Turn on the features that widen a group (Coat, Emissive), then assert `value.rect().max.x <= panel.max.x` for every later group. The RED message read: `Subsurface: value at [1568.6..1585.7], panel [1084..1384]`. [B2 fix I1]
- **"Columns stay above the footer."** Every clickable rect in the columns ends at or above the top of the footer's first label. [B1 fix I1]
- **"One row."** Every listed control's rect contains the first control's centre y, and x increases in the listed order. A 2 px centre check is only valid for controls of the same height. [B3 pre K2]
- **"Exactly one of A or B is drawn per frame, in every state."** Enumerate the states (texture, set, several sets, expanded, stale selection, 3D, no folder) and assert the invariant for each. [CC5; CD19]
- **A unit-level width test, without a harness.** Run a bare `Context` for a few frames and compare the content size with the view size:
  ```rust
  let ctx = egui::Context::default();
  for _ in 0..3 {
      let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 400.0))), ..Default::default() };
      let full = ctx.run_ui(input, |ui| {
          let o = egui::ScrollArea::horizontal().show(ui, |ui| group(ui, "g", "Coat", Some("off".into()), false, |_| {}));
          out = (o.content_size.x, o.inner_rect.width());
      });
      full.drop_without_applying_deltas();
  }
  assert!(out.0 <= out.1, "content {} > view {}", out.0, out.1);
  ```
  [B2 re-review; `ui/group.rs` tests]
  With no renderer, each `FullOutput` must drop or clear its texture deltas (`drop_without_applying_deltas()` as above, or `full.textures_delta.clear()`): epaint 0.37 panics on a dropped, unapplied delta ("Dropped TexturesDelta with 1 unapplied deltas"). [E37 T5, T6]
- **Test at the sizes users have:** 1280×800 and 1100 wide, not only 1600. [B1 final I2]

## 3. Clipped rects and scrolling
- **AccessKit bounds aren't clipped.** To know whether a widget can be clicked, read its clipped `interact_rect` from the pass's widget list, matched by its unclipped rect:
  ```rust
  let clickable = h.ctx.viewport(|v| v.this_pass.widgets.layers().flat_map(|(_, ws)| ws)
      .find(|w| w.rect == node_rect).map(|w| w.interact_rect));
  ```
  Two widgets with identical rects would be ambiguous. [B1 fix I1]
- **After a clipping fix, a test that clicked a widget that was only visible over the footer will fail.** Call `scroll_to_me()` first, as a user would scroll. [B1 fix I1]

## 4. Things kittest cannot see
- **A scroll bar's opacity is paint-only.** Every shown bar has a `Role::ScrollBar` node with no name (egui `containers/scroll_area.rs:1471-1473`), with the thin style and with the default floating one, even while a floating bar is drawn at opacity 0. So assert that a bar exists by counting those nodes in the area's rect, but "is the bar visible" can't be asserted from the tree: use screenshots as the evidence, and say so in the decision record. [CC21; E37 T11]
- **Paint-only cues** (a highlight, a drop-target stroke, an outline count): assert on `h.output().shapes`, for example "a stroke of `widgets.active.bg_stroke` over the card's rect while a payload hovers". [B3 pre (3.2); B3 final I1]

## 5. Frames, idleness and tooltips
- **A viewport that never goes idle** (a spinner, a 3D view, a worker that keeps requesting repaints) hangs `run()`. Use `run_steps(n)`. [CLAUDE.md gotcha]
- **Disabled-hover and normal tooltips wait out egui's tooltip delay.** `run_steps(2)` is too short, so use `run()` after the hover. [B3 task 3.1]
- **With `run_steps`, a tooltip takes 4 steps on 0.37, one more than on 0.36.2.** The delay ends on step 3, but that frame is the tooltip area's invisible sizing pass (egui `containers/area.rs:691-692`, as on 0.36.2), and 0.37 leaves invisible widgets out of the AccessKit tree (egui `context.rs:655`). So the tooltip's text is first in the tree, and first painted, on step 4. Tessalith keeps the count in one constant, `TOOLTIP_STEPS = 4` (`tests/nif_edit.rs`). [E37 T10, T11]
- **Rubber-banding keeps repainting.** With egui 0.37's default `rubber_band: true`, a reveal or a kinetic scroll can exceed `run()`'s `max_steps` ("It would have settled after 40 steps"). Turn the rubber band off in the style (Tessalith's `look::pin_036_style` and `look::thin_scroll()` do) rather than raising `max_steps`. [E37 T5]
- **On 0.37, a frame that draws a new widget asks for one more pass.** `Ui::widget_style` requests a repaint for every widget that had no response in the previous pass (egui `theme/mod.rs:32-38`; 0.36.2 has no such repaint). So in a harness that polls a model every frame, a `run()` straight after an edit races the worker: when the result lands mid-run, the new widgets push the count past `max_steps` (4), and the test fails only when it runs alone or serially. After the click, `step()` once, wait for the model to be idle, then `run()`; assert that the edit landed before you wait. Run the test binary with `--test-threads=1` once before a merge. [E37 T17a: `resize_field::pick_in`]
- **Data from a worker** (texture sizes, scans) needs a bounded loop: `for _ in 0..N { h.run_steps(1); if ready() { break } }`. [B2 pre K7]
- **First-frame state:** anything that uses last frame's rect (row menus, drops) is absent on frame one. Step at least once before you assert. [CC1]

## 6. Isolation: app-data folders
- **Tests never read or write the user's real settings, logs or caches** (`%APPDATA%`, `%LOCALAPPDATA%`, or the platform's equivalent). Redirect them to a temp dir before you build the app. [CLAUDE.md; CA23]
- **The environment is process-wide, so a test that changes it is a `[[test]]` of its own, never a module of `it`**: otherwise it races the other tests in the process. `it::no_it_module_changes_process_state` fails on an `it` module that sets or removes an env var, swaps the panic hook or calls the crash-log or device-lost wrappers. [B3 pre K6]
- **Prefer passing paths into the app over reading env at startup.** Tessalith's `SdcApp::new` takes the scan DB from the model, so only `%APPDATA%` still needs redirecting. [CA23]
- **Generate fixtures in the test.** Don't commit real assets.

## 7. Making tests bite
- **A comment-only test passes.** Each planned test must assert something real. [B3 progress ruling]
- **Mutation check:** disable the fix (`from.is_none()` → `true`, a cache bypass with `false &&`) and confirm the test fails. Record that in the report. [B3 task 3.2; B1 task 1.6]
- **A test that passes before the code exists gives no RED.** Add the assertion that fails today, such as "the selected set is absent from `visible_sets()`". [B3 pre K23]
- **Keep a moved test's meaning.** When a label or placement changes, update the finder in the same commit, and never weaken the assertion. [Spec Constraints; B2 final Strengths]

## 8. Cache and per-frame-cost pins
- **A build counter:** a `#[doc(hidden)]` `Cell<usize>` that counts rebuilds. Assert that it is unchanged across `run_steps(3)` with nothing changed, and that it moves after a real change. [B1 task 1.6]
- **The 10k case:** build synthetic items (no real re-plan), call your invalidation hook, and assert one build per change plus `show_rows` drawing only the rows on screen. [B1 pre A2; CA11]
- **Stale-cache pin:** make a change that moves only a secondary input (an evidence revision, the export ending), and assert that the derived list updates. [B1 pre K9; B3 final Strengths]
