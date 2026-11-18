# Worst-Case Catalog

Realistic values that break egui UI, grouped by what kind of value the widget renders. Use the rows that apply to the fields in your Phase 1 table. Each value is something a real user could produce; the note says what it tends to break in egui.

Use `example.com`, `example.org`, or `.test` domains for emails and URLs, so the fixture never points at a real inbox or site.

Contents:

- People and names
- Emails, URLs, identifiers
- Labels, titles, and copy from data
- Numbers and money
- Collections
- Time
- Images and media
- States
- egui text rendering
- Environment

---

## People and names

| Value | Breaks |
| --- | --- |
| `Aleksandra Wiśniewska-Kowalczyk` | Long, hyphenated, diacritics; Extends a horizontal row, or wraps to two lines (egui prefers the hyphen as a break point) |
| `Christopher Alexander Montgomery III` | Long with a suffix; naive first + last initials give `CI` |
| `Konstantin Oberhauser-Wettstein` | Long and compound; overflows single-line rows |
| `Jo` | Two letters; leaves the name column mostly empty, and naive initials give `J` |
| `J` | One letter; single-character initials, tiny click target if the name is the clickable part |
| `Ólafur Darri Ólafsson` | Leading accented capital; uppercase and sort logic, initials `ÓÓ` |
| `Đặng Thị Ngọc Hân` | Stacked Vietnamese diacritics; clipped by a tight clip rect or a fixed row height |
| `王秀英` | CJK, no spaces; boxes without a CJK font (see egui text rendering), "first + last word" initials finds one word |
| `نور الهدى عبد الرحمن` | RTL; needs a font with Arabic, and egui has no bidi, so joined with Latin text or digits it can read in the wrong order |
| `Seán O'Brien-Ó Súilleabháin` | Apostrophe and accents; initials, search, sorting |
| `María José de la Cruz y Fernández` | Lowercase particles; initials `Md` or `MF`, sorting by "last name" |
| `dana` | All lowercase; initials should still be uppercase |
| `🦊 Fox` | Emoji first; a box with egui's default fonts, and the initial is the emoji, not `F` |
| `\u{1F469}\u{1F3FD}\u{200D}\u{1F4BB} Priya` | Woman technologist, medium skin tone (written as escapes so the zero-width joiner is visible; use the escaped literal in the fixture): 4 `char`s, 15 bytes; `&name[..n]` byte slicing can panic on a non-char boundary, `chars().take(n)` splits the sequence |
| `  Sam   Lee ` | Leading, trailing, and repeated spaces; initials from empty words, odd gaps (`trim` and `split_whitespace`) |
| *(missing)* | `None` or empty name, only an email; the UI must fall back to something |

## Emails, URLs, identifiers

Unbreakable strings: they have no spaces. In Wrap mode egui breaks them at punctuation or anywhere; in Extend mode (the default in `ui.horizontal` and in a `Grid`) they push everything after them off the edge.

| Value | Breaks |
| --- | --- |
| `bartholomew.fitzgerald@northwind-industries-holdings.example.com` | The classic. Pushes every sibling off the row in Extend mode |
| `a@b.co` | Shortest realistic; layout that assumed a long email looks empty |
| `first.last+billing-notifications@example.com` | Plus-addressing; validation that rejects `+`, end-truncation hides the meaningful part |
| `ops@sub.department.region.example.co.uk` | Many subdomains, two-part TLD; "domain" extraction logic |
| `https://example.com/workspaces/acme/projects/q3-launch/docs/9f8e7d6c5b4a?tab=comments&filter=unresolved` | Long URL; overflow, and end-truncation hides the part that differs |
| `9f8e7d6c-5b4a-4c3d-8e2f-1a0b9c8d7e6f` | UUID; set it in monospace, middle-truncation candidate |
| `Q3 Board Deck — FINAL (revised) v12 [approved by legal].pdf` | File name; end-truncation hides the version and extension |
| `IMG_20250914_183022_HDR_portrait_edited_edited.HEIC` | Camera file name; unbreakable, uppercase extension |
| `C:\Users\Aleksandra Wiśniewska-Kowalczyk\OneDrive - Northwind\Documents\exports\2025\` | Windows path with spaces and diacritics; desktop apps show paths, and end-truncation hides the folder that matters |
| `@a` / `@thisisaverylongusernamethatisallowed` | Handle extremes |

## Labels, titles, and copy from data

| Value | Breaks |
| --- | --- |
| `Senior Product Design Engineer, Platform Infrastructure` | Long job title; wraps to three lines in a secondary slot |
| `Invitation expired 12 days ago` | Long status badge; badge wraps or squeezes the name column |
| `Benachrichtigungseinstellungen` | German compound (30 chars, no spaces); label overflow, broken mid-word when wrapped |
| `Paramètres de confidentialité et de sécurité` | French expansion of "Privacy & security"; UI copy runs ~30% longer in translation, overflows fixed-width buttons |
| Twelve tags on one item: `design`, `frontend`, `q3`, `urgent`, `needs-review`, … | Tag rows that wrap into a wall (`horizontal_wrapped`) or Extend off-screen (`horizontal`); needs a `+8` overflow |
| A tag named `customer-feedback-from-enterprise-onboarding` | One tag wider than its container |
| `Untitled` / empty string / `   ` | Title missing; zero-height label, collapsed heading |
| `<script>alert(1)</script>` / `&amp;` / `**bold**` | egui renders these literally, so they usually pass; they break only through a markdown or HTML viewer crate |
| `Line one` + newline + `Line two` | `\n` starts a new line in Wrap and Extend modes (doubles row height in a single-line slot); with `.truncate()` everything after the first newline is elided (`Line one…`), so the rest is silently hidden. Strip or replace newlines in single-line fields |
| A 2,000-character pasted description | Clamping (`LayoutJob` `max_rows`), "show more", and `TextEdit::multiline` growth |

## Numbers and money

| Value | Breaks |
| --- | --- |
| `0` | Zero states: "0 members", empty progress bar, division by zero in percentages (`NaN` in a `ProgressBar`) |
| `1` | Plurals: "1 members", "1 days ago" |
| `1284` | Needs a thousands separator: `1,284`; Rust's `format!` has none |
| `1000000` | Width of the count badge; consider compact form `1M` where precision doesn't matter |
| `12345678.9` as currency | `$12,345,678.90`; overflows totals columns |
| `-42.5` | Negative sign, red colour logic, parentheses in accounting formats |
| `0.1 + 0.2` | `format!("{}", 0.1 + 0.2)` renders `0.30000000000000004`; use `{:.2}` or a money type |
| `142%` / `-3%` | `ProgressBar::new(1.42)` and custom meters past their bounds; clamp to `0.0..=1.0` |
| `None` / `NaN` / `inf` | Rendered literally through `{:?}` or `{}` |
| `1.284` in `de-DE` vs `1,284` in `en-US` | Locale formatting; hardcoded separators are wrong for half the world |
| A value that changes live (`99` → `100`) | Width jump and jitter with proportional digits; use `RichText::monospace()` or a fixed-width right-aligned cell |

## Collections

| Value | Breaks |
| --- | --- |
| 0 items | The empty state, and whether one exists at all |
| 1 item | Grids that look broken with one card; "1 of 1" |
| Exactly page size, and page size + 1 | Off-by-one in "Showing 40 of 40", pagination that shows an empty page 2 |
| 1,000+ items, unpaginated | Frame time: every row laid out every frame without `ScrollArea::show_rows`; also "Showing 40 of 1,284" copy |
| One item 10× the size of the others | `show_rows` needs one row height; variable heights need `show_viewport` or pagination |
| Items with identical names | Lists where the name is the only distinguishing field; also egui id clashes if ids come from the name (`ui.scope_id().with(&name)`): use a stable key |

## Time

| Value | Breaks |
| --- | --- |
| Now | "0 seconds ago" instead of "just now" |
| 12 days ago, 11 months ago, 3 years ago | Relative-time thresholds; switch to an absolute date after a week or so |
| A future date | "in 3 days" vs "-3 days ago" |
| `1970-01-01` | A zero timestamp shown as a real date |
| `2025-12-31T23:30:00-08:00` | Shows as a different day in UTC and the user's timezone |
| Very long duration (`1,284 hours`) | Duration formatting that never rolls up to days |

Format through the project's date/time crate and one shared helper, not hand-built strings at each call site. A relative time ("3 minutes ago") also needs a repaint to stay true: `ctx.request_repaint_after(..)`, not a repaint every frame.

## Images and media

| Value | Breaks |
| --- | --- |
| Avatar URI that fails to load | egui's error placeholder in the slot instead of the initials fallback |
| No avatar at all | The fallback itself: initials, colour, size parity with real avatars |
| 4000×200 panorama as avatar or cover | An `Image` that keeps aspect ratio becomes a thin strip; one that doesn't (`maintain_aspect_ratio(false)`) distorts |
| 200×4000 tall image | Same, other axis; can also blow out a card's height |
| Transparent PNG logo, dark logo on dark theme | Invisible on the panel fill |
| Slow-loading image | Spinner at one size, image at another: layout jumps unless the size is fixed (`fit_to_exact_size`) |

## States

| Value | Breaks |
| --- | --- |
| Loading | Spinners that shift content, placeholders that don't match the final layout |
| Error from the API | No error state, or a raw `Debug` error (`reqwest::Error { kind: Request, … }`) in a label |
| Partial data | Some optional fields filled, others not, in the same list; misaligned rows |
| Every status at once | All enum variants in one list (`Active`, `Invited`, `Expired`, `Suspended`); badge widths vary |
| No permission | Disabled actions (`add_enabled`); does the row still lay out the same? |
| The current user in the list | "You" labels, actions that shouldn't apply to yourself |

## egui text rendering

| Condition | Breaks |
| --- | --- |
| CJK, Arabic, Devanagari or Thai text with the default fonts | egui's bundled fonts cover Latin and Cyrillic; anything else comes from eframe's system font fallback (native only, on by default). Fine on your machine, boxes on the web build, with `default-features = false`, or with `system_font_fallback: false` |
| Emoji | No emoji in the default fonts unless egui's `monochrome_emoji_fonts` feature is on; never colour; ZWJ sequences may render as pieces |
| Arabic or Hebrew mixed with Latin or digits | egui has no bidi: the visual order can be wrong. Keep RTL values in their own label |
| A word longer than the column | Wrap mode breaks it anywhere; Extend mode overflows; Truncate elides it |
| Text in a hand-painted rect (`painter.text`) | Never wraps or truncates: the painter has no layout. Measure with `layout_no_wrap` and decide |

## Environment

Not data, but checked the same way: flip to the worst case, then change these.

| Condition | Breaks |
| --- | --- |
| Window at 320 px wide | Every overflow above, at once; side panels eating the central area |
| Narrow panel placement | Widgets designed full-width, reused in a 280 px `Panel::left` |
| 2560 px wide | Lines too long to read (cap with `ui.set_max_width`), content stranded on one side |
| Zoom 200% (`ctx.set_zoom_factor(2.0)`) or a 2× monitor | Fixed `allocate_exact_size` boxes and fixed-height areas clip growing text; dialogs taller than the window |
| Zoom 0.5 / a 1× monitor | 1-point strokes and gaps blur or vanish; `ctx.pixels_per_point()` tells you which you're on |
| Tiny window (below the min size) | Panics or `NaN` from negative available sizes in custom layout code; set `ViewportBuilder::with_min_inner_size` |
| Light and dark theme (`ctx.set_theme(..)`) | Hardcoded `Color32`s, invisible strokes and logos |
| Right-to-left layout (`Layout::right_to_left`) | Chevrons and trailing actions on the wrong side; `Grid` debug-asserts inside it |
| Touch (eframe web on a phone) | Hover-only actions (the ••• that appears on hover) are unreachable; `on_hover_text` never shows |
