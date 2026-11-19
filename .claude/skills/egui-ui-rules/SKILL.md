---
name: egui-ui-rules
description: A measured UI ruleset for Rust desktop apps built with egui/eframe, for everything except motion. Use it whenever you build, lay out, restructure, review or test egui/eframe UI. That covers Panel::top/bottom/left/right, column widths and minimums, truncation and hover text, scroll bars, information hierarchy, where problems, warnings and the export or save flow go, status colours, readable labels, dialogs and Enter defaults, destructive buttons, filters, non-modal review panels that gate a write, AccessKit names, per-frame cost, and egui_kittest tests. Use it even when the user only says "fix this panel", "the buttons go off-screen" or "write a kittest for this". The egui gotchas were measured on egui 0.37.0; check them again on other versions. Not for animation or motion (use egui-animate, egui-review-animations and the other egui-*animation* skills), and not for web, CSS or HTML. For visual taste and feel use egui-emil-design-eng. For stress-testing with worst-case data use egui-break-ui.
---

# egui UI rules

Rules for laying out, wiring and testing egui/eframe desktop UI. Most come from a real app, Tessalith's GUI. They were found by reviewers in its layout v2 work (first built on an earlier egui), reproduced as failing tests and fixed, then checked again in its egui 0.37 upgrade. A few come from the user's design-skills report, which collects web-sourced numbers. Measured findings take priority. Rules marked **(taste)** are judgement; follow them unless the project already decides otherwise.

**Out of scope: animation and motion.** For those, use egui-animate, egui-review-animations, egui-improve-animations, egui-find-animation-opportunities and egui-animation-performance.
For visual feel (Style and Visuals tokens, hover and press feel, tooltips, toasts), use **egui-emil-design-eng**. For worst-case data (long names, CJK, huge counts, Extend vs wrap vs truncate), use **egui-break-ui**. This skill doesn't repeat what those two cover.

## How to use this

1. **Read `Cargo.toml` first.** Note the exact egui, eframe and egui_kittest versions, and write code for those versions only. All the egui-* skills now target egui 0.37.0 (`App::ui`), and the gotchas here were measured on 0.37.0 in Tessalith. On another version, check each gotcha that matters against that version's egui source in the cargo registry. Run `cargo check` after each edit. [Report; E37 T11]
2. While building or reviewing, go through the groups below. The long gotcha list, with the egui-source reason for each item, is in `references/gotchas-egui-0.37.md`; its section 0 lists the names 0.37 changed. Read it before you write panel, id, scroll, collapsing, drag-and-drop, popup or accessibility code.
3. The code for each rule is in `references/examples.md`, taken from Tessalith's `crates/gui/src/ui/`.
4. Write tests by `references/kittest.md`. Layout bugs here were caught by RED-first pins and by screenshots, not by reading code.
5. Before you report done, capture the screen at about 1100, 1280 and 1600 px wide and check every rule in groups 2 and 3 against the images. [B2 final; Report]

**Source tags:**
- [Report]: the owner's report "Claude UI design skills and rules" (web-sourced numbers; kept outside this repo).
- [Spec]: the layout v2 design (`converter/docs/superpowers/specs/2026-10-17-gui-layout-v2-design.md`).
- [Rev-A n] / [Rev-B n]: designer reviews A and B, proposal n. Only A1-A12, B2 and the Sets filter were built.
- [B1/B2/B3 final In], [B1/B2/B3 pre Kn]: the final review or preflight scan of branch 1, 2 or 3.
- [CAn] / [CCn] / [CDn]: Tessalith `docs/DECISIONS.md` Notes CA, CC and CD.
- [E37 Tn]: Tessalith's egui 0.37 upgrade, task n (tests `crates/gui/tests/egui_gotchas.rs`, `accepted_037.rs`, `accessible_names.rs`).

## 1. Layout and information hierarchy

1. **Give each step of the user's main loop one obvious place.** Tessalith had warnings on six surfaces and four modals that could interrupt Export, and none of them worked as a worklist. [Rev-A §1; Spec Intent]
2. **Put problems in one list, one row per issue.** Each row has a severity dot, a clickable location, one plain sentence and the fix as a button (`Decide…`, `Acknowledge`, `Go to set`). The user fixes things where they read about them. [Rev-A 1; CA11]
3. **Badges and counts are buttons that open that list, filtered.** A "⚠ 2" that does nothing, or a count with no target, makes the user hunt. [Rev-A 1; CA18]
4. **The export or save review lives in a non-modal bottom panel, not a chain of modals.** The columns stay usable, so a fix happens in place. Keep modals only for "a write is running" and for questions that aren't about the export. [Spec goal 1; Rev-B 2; CA1]
5. **Keep the commit actions together and visible in every view.** Put Save next to Export at the same height. Show the export path's tail, not its prefix, with the full path on hover. Keep the header when the view changes. [Rev-A 4; CD1, CD4]
6. **Never let a floating window cover the header or the controls needed to act on what it says.** Dock the window into a panel or tab, or centre it over the centre column. [Rev-A 9; Spec A9]
7. **Every data view has an empty, a loading and an error state, and the empty state has one call to action.** Write "Not exported yet", not five zeros. The start screen is one large "Open folder…" button plus Recent, with the column chrome hidden. [Rev-A 11; Rev-B 10, 12; CD18; Report]
8. **Copy the best panel in the app.** In Tessalith that was the texture settings panel: a back link, a name, collapsing groups and a Grid with a hint column. One section-header style for one idea, not four. **(taste)** [Rev-A §1 Consistency]

## 2. Panels

1. **Draw panels in a fixed order: top header, then the full-width bottom panel, then the side columns, then the centre.** A bottom panel drawn after the side panels spans only the centre. Drawn before them, it spans the window, in both views. [CA9; `ui/mod.rs`, `ui/bottom.rs`]
2. **A panel's salt is a fixed string, its id is scoped to the parent `Ui`, and a collapsed strip gets its own salt.** egui 0.37 keys a panel's state by `parent.scope_id().with(salt)`, never by `Id::new(salt)`: read and write a panel's width only through that id (Tessalith: `ui::panel_ids`). Never pass an `Id` as the salt; it is hashed again. egui keeps one size per panel id, so a 24 px strip that shares the salt becomes the column's width. [CA9; B2 pre A10; `column_panel`; E37 T6, T11]
3. **Compute each column's maximum so that every later column keeps its minimum and the centre keeps `CENTRE_MIN`.** Write `range = min..=(avail - others).max(min)`: when the room left is too small, the minimum wins. An inverted range is silently flipped by egui, and then neither bound holds. This holds while the panel's parent is at least `min` wide; in a narrower parent egui 0.37 lays the content out at `min` but cuts the panel's rect to the parent, so both bounds break (gotchas, "An inverted `size_range`"). [B2 pre K20; CC2; E37 T11]
4. **Keep the width the user dragged to apart from `PanelState`.** egui stores the width it drew, clamped to the window, so narrowing the window and widening it again loses the user's width. Store a separate "chosen" width on drag release and write it back into `PanelState`, under the panel's scoped id, before `show`. [`column_panel`; CD18; E37 T6]
5. **A panel whose children may be wider than it gets `ScrollArea::both`.** With a vertical-only area, a 390 px card in a 300 px panel pushed the panel's rect off the window edge and its content over the centre (on 0.36.2 the panel widened past its range and the layout never settled). [CC2; E37 T11]
6. **When you skip drawing a panel this frame, call `ui.skip_ahead_auto_ids(1)`.** Otherwise every later auto id shifts, and open menus and focus in the columns break. A set's context menu closed on the next frame. egui does the same for a collapsed panel. [CC3; B2 final Strengths]
7. **Don't put `ui.columns` inside a panel that has a footer sub-panel without bounding it first.** `columns` sizes to the bottom of the whole panel, so its widgets draw over the footer and take its clicks. Wrap it in `ui.allocate_ui(ui.available_size(), ..)`. [B1 final I1; CA25]

## 3. Widths, truncation and scrolling

The basics of Extend vs wrap vs truncate, `Label::truncate()`'s free tooltip, `Grid::max_col_width` and right-to-left-first layout are in egui-break-ui. These are the additions:

1. **A widget never draws under its neighbour.** If rows don't fit their column, put them in a horizontal `ScrollArea` clipped to that column. Slot rows drawn under the material panel lost their clicks to it. [B2 progress 2.2 ruling; CC2]
2. **Scope `ScrollStyle::thin()` (or `solid()`) with the rubber band off.** The default floating style has dormant opacity 0, so `ScrollBarVisibility::AlwaysVisible` still draws an invisible bar. At 1280 px the Format combo was unreachable with no cue. Every 0.37 preset turns rubber-banding on; Tessalith scopes `look::thin_scroll()`, which is `thin()` with `kinetic.rubber_band = false`. A bar whose own paint is fully transparent with no pointer in the window fails the `ui_proof` walk (`invariants::scroll_bars_are_visible_and_clear`, Note JU); it reads vertical bars only, and any alpha above 0 passes, however faint. [B2 final I2; CC21; E37 T5]
3. **A row with text and buttons uses `egui::Sides::new().shrink_left().truncate()`.** The buttons lay out first on the right. The sentence takes what's left, with `.truncate()` and the full text on hover. Without this, long problem rows pushed their fix buttons off-screen at 1280×800. [B1 final I2; CA11]
4. **Every truncation keeps the full value reachable: on hover, and in the AccessKit name.** A cut text, a middle cut such as `C:\…\mods\My Mod` too, fails the `ui_proof` walk (`invariants::cut_text_keeps_its_value`, Note JU) when no name, value or description of its node holds the longer text; the walk does not see the tooltip, and a label the author ends with an ellipsis goes in its `AUTHORED` list. [CD4; B3 pre K8]
5. **`ComboBox::width` is only a minimum.** Give each combo an exact-width slot with `ui.allocate_ui(vec2(W, h), ..)`, or a truncated combo takes all the room left. [AG124 via `columns.rs::filter_popup`]
6. **Inside a resizable column, set `ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate)`.** Otherwise long text holds the column wider than the user dragged it. [`column_panel`]
7. **Leave a few pixels of margin when you size a cell from the column's width.** With none, a row exactly as wide as its column flipped the scroll bar on and off every frame, and `run()` never settled. That was measured in Tessalith on 0.36.2; a bare probe on 0.37 did not reproduce it, but egui's bar test is unchanged, so keep the margin. [CC2: 8 px; E37 T11]
8. **Bound a right-aligned value to the visible edge, not to the content width.** A wide row earlier in a scroll area widens every later row's `max_rect`, so right-to-left values land past the clip. Clamp to `min(max_rect.right, clip_rect.right - item_spacing.x)`. [B2 final I1; CC20]

## 4. Status colours and labels

1. **Keep status colours in one module: red blocks the write, amber is a warning or a needed decision, green means ready.** Every other file imports them; none defines its own (`look::tests::status_colours_are_defined_in_ui_status_only`, a line scan of `crates/gui/src` for `const RED`, `AMBER`, `GREEN` and their three RGB values). [CA20; `ui/status.rs`]
2. **Use one non-status accent, for selection and the one primary action, and for nothing else.** When orange meant warning, undecided, not-in-a-set and the active frame at once, it meant nothing. [Rev-A 12; Spec A12]
3. **Show "ready" too.** A green dot on clean items lets the user see "done", not only "broken". Before this, a clean set looked the same as an unchecked one. [Rev-A 12; CA20]
4. **Colour is never the only carrier.** A status dot has an AccessKit label ("ready", "warning", "blocks export") and a hover that gives the reason. A circle in `status::RED`, `AMBER` or `GREEN` whose innermost node has no name fails the `ui_proof` walk (`invariants::status_dots_have_names`, Note JU); the walk does not check what the name says, the hover, or a status in another colour. [`status::dot`; Report]
5. **Use readable labels, with the raw key only on hover or as a small grey suffix.** Write "Specular enabled", not `specular_enabled`. Keep an initialism list (AO, UV, RGB, SSS, F0) so you don't produce "Sss". [Rev-A 8; CC7]
6. **Use one vocabulary per concept.** Tessalith had "Diffuse", "Slot 1 (Diffuse)" and an unlabelled swatch for the same slot. Get the plurals right too ("1 record"). **(taste)** [Rev-A §1; B2 final Minor 3]
7. **The Report's web-sourced defaults:** a 4 px spacing grid, text contrast of at least 4.5:1, hit targets of at least 24 px, and one dominant colour plus one accent. Nothing in Tessalith measured these. **(taste)** [Report]

## 5. Dialogs, buttons and keys

1. **Use a dialog only when the user must answer before anything else can happen.** Everything else is a panel or a tab. [Spec goal 1; CA1]
2. **Enter triggers the safe default. It never overwrites, deletes or exports.** It does nothing while a widget has keyboard focus, and nothing on the frame the dialog opened, so the Enter that opened it doesn't also answer it. Enter, Escape or Space with nothing focused that changes a file or starts an export fails the `ui_proof` walk (`invariants::press_keys`, hits `keys_never_write`, Note JU), on every screen it lists, at 1280x800; it does not press a key inside a focused field or with a modifier. `dialog_keys.rs` (the `it` binary) tests the dialogs' Enter rows, the frame a dialog opens and focused destructive buttons; its comment names the files that hold the other dialogs' Enter rows. [CA2, CA19; `ui/dialogs.rs`]
3. **Draw the Enter default filled with the accent, and outline destructive buttons in red.** Use one fixed button order for every dialog: plain buttons, then destructive ones, then the default at the right. Tessalith's `ui::dialogs::dialog` draws its footer from one `dialogs::Role` per button; `dialog_keys::the_enter_default_is_rightmost_and_filled` and `dialog_keys::the_footer_order_is_plain_then_destructive_then_the_default` (the `it` binary) test it. [Spec A7; Rev-A 7; Rev-B 5]
4. **Button text names the action and the object.** Write "Overwrite mod.json" / "Keep the edited mod.json" and "Stop the export and close" / "Keep exporting", not "Keep it" or "Cancel export and continue". [Rev-B 5]
5. **Instructions match the widgets.** "Click one to select its set" over plain text lines is a bug: make the lines buttons. [Rev-A §1 Dialog wording]
6. **Record a changed key.** When a dialog becomes a panel, Esc and Enter change meaning. Write that down, and add a test. [B1 final M1; CA19]

## 6. Filters and selection

1. **Filters hide and never deselect.** A filter that hides the open item keeps the selection, the editor and the bulk panel. Don't route the editor through the filtered list. [Spec goal 7; CD13]
2. **Show the hidden part of a selection as a fact:** "2 selected (1 hidden by filter)". [CD13]
3. **A select-all over an empty visible list is a no-op.** It never deselects. [CD14]
4. **Turn reordering off while any filter is on, and say why on hover.** Otherwise the list reorders against positions the user can't see. [CD15]
5. **Make filters pill toggles with counts, and put the rare ones in a `Filter ▾` popup.** Draw the pills with `.frame_when_inactive(true)`, or off pills read as plain text. [Rev-A 6; CD10; B3 final (a)]
6. **Don't show "No matches" before the data exists.** A "Clear filter" button drawn during the first load wiped filters restored from settings. [CD17]

## 7. Safety in a non-modal review panel that gates a write

1. **The review never writes by itself.** Resolving the last issue only relabels the button ("Export 20 files (skip 2 undecided)" becomes "Export 22 files"). The user's click on the review's own write button is the only way to start the write. Close leaves without writing, and Enter does nothing there; the `ui_proof` walk's key step (`invariants::press_keys`, hits `keys_never_write`) presses Enter on the review tab with nothing focused and fails a written file or a started export. [CA2, CA19]
2. **Never write on a stale plan.** A click while the plan is in flight is queued. When the plan lands, the gate runs again on the new plan, and a new blocker or warning stops the write with a status line. [CA3, CA13; B1 final Strengths]
3. **Never write past an unacknowledged warning.** Key each acknowledgement by the warning's exact text, so a changed warning needs a new tick. Clear the ticks when the write starts, when the review closes and when the project changes. [CA4]
4. **Check again at write time.** An answer given in the review ("overwrite the edited JSON") stores the file's hash. The writer applies the answer only if the file still has that hash; otherwise it asks again. The answer is taken on entry, so a cancelled or failed write never leaves it for the next one. [CA7, CA17]
5. **Run the focus guard before saving.** A click on a button that saves, while a text field still has focus, refuses with "leave the text field first", because the field's value isn't committed yet. Also blur text fields on the pointer press, so commit-on-focus-loss runs before the selection changes. [CA14; `column_panel`]
6. **Closing the review cancels anything it queued.** Opening another project, or reloading, closes the review and writes nothing. [CA13, CA17]
7. **Cache what the gate reads, keyed on every input it reads.** A stale cache would let a new warning past the gate. [B1 final Strengths; CA22]

## 8. Accessibility (AccessKit names)

1. **The accessible name keeps its meaning when the visible text is shortened.** For a cut text that is 3.4, which the walk checks; a shorter word in place of a longer one it cannot see. An elided path's name is "Export to: {full path}". Tabs that show "Textures" are named "Textures view". [CD1, CD4; B3 pre K8]
2. **Every painted or icon widget gets `response.widget_info(..)` with a role and a name.** That covers painted letter chips, dots, icon toggles, and − / + buttons ("Smaller tiles"). [CD8; CC2; `columns.rs`]
3. **Call `widget_info` after `ui.add`. It replaces the label egui set.** [B3 pre A14; `header.rs::view_toggle`]
4. **Two controls on screen never share an accessible name**, for example "Textures filters" and "Sets shader filter". Otherwise a screen reader can't tell them apart, and tests must scope by rect. There are two exceptions, both names egui sets: a Slider and its own value box may share a name (egui gives the box the slider's name), and so may panel splitters (egui names every one "Resize panel", and the app cannot set it). `accessible_names.rs` fails on a new shared name on the screens it audits; the pairs it found are allowlisted (Note JT5). [B3 final Minor 5; E37 T2, T9]
5. **When you swap a widget kind, say that the role changed.** Flags went from CheckBox to a selectable Button with the same names. Record it. egui 0.37 changed some roles itself: its `CollapsingHeader` is now `DisclosureTriangle`, not `Button`. [CC2]
6. **Every input widget has a name (Tessalith's rules).** kittest's own check fails a `run()` on an unnamed visible input, and `it::kittest_checks_are_never_switched_off` keeps it on (a line scan; a wrapped harness builder is not seen). Name a widget with `ui::name_input(response, name)` (`Response::accessible_name`) when nothing visible beside it names it. A value box beside a control with the same label is named "{label} value" ("Specular level value"). A box whose only same-text neighbour is a plain Label is linked to it with `labelled_by`. [E37 T2]
7. **`labelled_by` clears the control's own name in 0.37.** Where the control's own name says more than its row label ("Alpha threshold" against "Threshold"), name it again after the link: `r.labelled_by(l.id)` then `name_input`. Where the own name is equal or poorer (a DragValue's "×" suffix, a combo's value), the label names it. [E37 T9]
8. **Name before hover text.** 0.37 copies an `on_hover_text` sentence into an unnamed widget's name (not a Label's, not a linked one's). Give every icon button and unlabelled combo a short name, so the tooltip sentence stays its description. `accessible_names.rs` fails on an input named only by its hover, on the screens it audits. [E37 T2, T11]

## 9. Per-frame cost

1. **Cache derived lists by revision, and key the cache on everything the builder reads.** For example: view key, evidence revision, export revision, whether a write is running. Build the list outside any `RefCell` borrow. Compare `Arc`s with `Arc::ptr_eq`. [B1 pre K9; CA11; CD (K17)]
2. **Draw only the rows on screen, with `ScrollArea::show_rows` and one fixed row height.** Pin it with a 10k-row test and a build counter. [CA11; `problems.rs`, `drawer.rs`]
3. **Build hover text lazily.** Use `on_hover_ui` or a closure, not a `format!` every frame. [B1 progress 1.1]
4. **Stat or hash files only while the menu or panel that shows the result is open,** and give the result a short TTL. [CD4 (A2); CA22]
5. **Don't call an uncached O(n) query once per card per frame.** Derive it from the cached list. [B1 pre R-a]

## 10. Testing with egui_kittest

The details and code are in `references/kittest.md`.

1. **Find by label or role, and scope by rect.** `get_by_*` panics on more than one match, so use `query_all_by_label(..)` filtered by the panel's `PanelState` rect, or `get_by_role_and_label(Role::Button, ..)`. [B2 pre K5, K11; B3 pre K15]
2. **Pin layout RED-first.** "No widget past its column edge", "the value's `rect.max.x <= panel.max.x`", "content width <= view width". Show the test failing before the fix. [B1 fix I1/I2; CC20]
3. **For clipped widgets, use the clipped `interact_rect` from `ctx.viewport(|v| v.this_pass.widgets)`.** AccessKit bounds aren't clipped. [B1 fix I1]
4. **Every shown scroll bar has a `Role::ScrollBar` node, but its opacity is paint-only.** This holds for the thin style and for egui's default floating one alike. Assert that a bar exists from its nodes; prove that it is visible with screenshots. [CC21; E37 T11]
5. **A viewport that never goes idle (a spinner, a 3D view) needs `run_steps(n)`, not `run()`.** A tooltip needs `run()`, to wait out egui's delay. With `run_steps`, a tooltip's text reaches the AccessKit tree one step later than on 0.36.2, because 0.37 leaves the tooltip's invisible sizing pass out of the tree (Tessalith's `TOOLTIP_STEPS` is 4). [CLAUDE.md; B3 3.1 report; E37 T10]
6. **Tests never touch real app-data folders.** Redirect `%APPDATA%` and `%LOCALAPPDATA%` to a temp dir, in a test binary of its own, because the environment is process-wide. [CA23; global constraints]
7. **A comment-only test passes.** Every test asserts something, and a mutation that disables the fix must fail it. [B3 progress; B3 final Strengths]
8. **A TextEdit's hint is a placeholder, not a label, and a combo's selected text is its value, not its name.** Find a box or a combo by role and name (`get_by_role_and_label(Role::ComboBox, "Sort order")`), or a combo by its value with `get_by_value(..)`. A label query never matches a hint. [B3 pre K21; B3 3.3 report; E37 T2, T11]
9. **Fonts go in `HarnessBuilder::with_context_setup`.** egui_kittest 0.37 draws a frame inside `build_*`, and its default `MissingGlyphPolicy` panics on a missing glyph. Never add `with_accessibility_check(false)` to get a test through: name the widget. [E37 T1, T2]
10. **A look change is proven by rendered screenshots compared with the committed ones, not by the kittest suite alone.** After the 0.37 upgrade the whole suite passed with a combo 62 px out of place; only the screenshot overlays (Tessalith: `tools\doc-shots.ps1`) showed it. [E37 T15a]

## Reference files

- `references/gotchas-egui-0.37.md`: the egui 0.37.0 traps found in this work, and the names 0.37 changed. Each entry gives the symptom, the egui cause and the fix. Read it before you write panel, scroll, collapsing, drag-and-drop, id, popup or accessibility code.
- `references/examples.md`: short code for the rules above, taken from Tessalith's GUI.
- `references/kittest.md`: test patterns (egui_kittest 0.37's harness facts, role-and-name finders, rect scoping, clipped rects, unit-level width tests, shapes tests, app-data redirect, build counters).
