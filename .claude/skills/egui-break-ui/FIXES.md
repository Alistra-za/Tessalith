# Fix reference

Compiled against eframe 0.37. Each block is copied from a checked crate; adapt names to the project, keep the mechanism.

- A row that survives the worst case
- Clamp, huge lists, wrapping Grid
- Fallback fonts

## A row that survives the worst case

Fixed avatar, trailing action reserved first, text truncates with a tooltip.

```rust
/// Avatar fixed, trailing action reserved first, text takes what is left and truncates.
pub fn member_row(ui: &mut egui::Ui, member: &Member) {
    ui.horizontal(|ui| {
        let (avatar, _) = ui.allocate_exact_size(vec2(28.0, 28.0), egui::Sense::hover());
        ui.painter().circle_filled(avatar.center(), 14.0, ui.visuals().widgets.inactive.bg_fill);
        ui.painter().text(avatar.center(), Align2::CENTER_CENTER, initial(&member.name), egui::FontId::proportional(13.0), ui.visuals().text_color());

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let _ = ui.button("•••");
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.add(egui::Label::new(&member.name).truncate());
                ui.add(egui::Label::new(egui::RichText::new(middle_truncate(&member.email, 40)).weak()).truncate())
                    .on_hover_text(&member.email);
                if let Some(role) = &member.role {
                    ui.add(egui::Label::new(role).truncate()); // omitted, not "—", when missing
                }
            });
        });
    });
}

/// First character, uppercased. `chars()` never splits a surrogate pair, but it does split
/// ZWJ emoji and combining marks; use grapheme clusters if the project already has a crate for them.
pub fn initial(name: &str) -> String {
    name.trim().chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into())
}

/// Keep the start and the end, for values that differ at the end (emails, file names, paths).
pub fn middle_truncate(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars || max_chars < 3 {
        return s.to_owned();
    }
    let keep = max_chars - 1;
    let head: String = s.chars().take(keep / 2).collect();
    let tail: String = s.chars().skip(n - (keep - keep / 2)).collect();
    format!("{head}…{tail}")
}
```

## Clamp, huge lists, wrapping Grid

Clamp to two lines, a huge list with `show_rows`, a `Grid` that wraps:

```rust
/// Clamp a preview to two lines with an ellipsis (CSS `line-clamp: 2`).
pub fn clamped(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        egui::FontId::proportional(14.0),
        ui.visuals().text_color(),
        ui.available_width(),
    );
    job.wrap.max_rows = 2;
    job.wrap.break_anywhere = true;
    ui.label(job)
}

/// 1,000+ rows: only the visible ones are laid out. Rows must share one height.
pub fn member_list(ui: &mut egui::Ui, members: &[Member]) {
    if members.is_empty() {
        ui.weak("No members yet. Invite someone to get started.");
        return;
    }
    // Tallest row: name + email + role, three body lines plus the spacing between them.
    let row_h = 3.0 * ui.text_style_height(&egui::TextStyle::Body) + 2.0 * ui.spacing().item_spacing.y;
    egui::ScrollArea::vertical().auto_shrink(false).show_rows(ui, row_h, members.len(), |ui, range| {
        for member in &members[range] {
            ui.allocate_ui(vec2(ui.available_width(), row_h), |ui| {
                ui.set_height(row_h); // rows without a role keep the same height
                member_row(ui, member);
            });
        }
    });
}

/// Grid labels Extend by default; a max column width makes them wrap instead of pushing columns off.
pub fn members_grid(ui: &mut egui::Ui, members: &[Member]) {
    egui::Grid::new("members_grid").num_columns(2).striped(true).max_col_width(220.0).show(ui, |ui| {
        for member in members {
            ui.label(&member.name);
            ui.label(&member.email);
            ui.end_row();
        }
    });
}
```

## Fallback fonts

A bundled fallback font, so glyphs don't depend on the user's machine, and the options to test without system fonts:

```rust
/// Bundle a fallback font so CJK/Arabic/emoji never depend on the user's machine.
/// Call once from the `run_native` creation closure: `install_fallback_font(&cc.egui_ctx)`.
pub fn install_fallback_font(ctx: &egui::Context) {
    // Ship it with `FontData::from_static(include_bytes!("../assets/NotoSansCJKsc-Regular.otf"))`.
    let Ok(bytes) = std::fs::read("assets/NotoSansCJKsc-Regular.otf") else { return };
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("noto_cjk".to_owned(), Arc::new(FontData::from_owned(bytes)));
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        // push = last fallback: Latin keeps the default font, missing glyphs come from Noto.
        fonts.families.entry(family).or_default().push("noto_cjk".to_owned());
    }
    ctx.set_fonts(fonts);
}

/// Simulate a machine without system fonts (the web, or a bare Linux box).
pub fn options_without_system_fonts() -> eframe::NativeOptions {
    eframe::NativeOptions {
        system_font_fallback: false,
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 640.0]),
        ..Default::default()
    }
}
```

`push` makes the font the last fallback, so Latin keeps the default look. `insert(0, ..)` would make it primary. `ctx.add_font(FontInsert::new(name, data, families))` adds one font without rebuilding `FontDefinitions`. Fonts change at the start of the next pass, so install them once at startup, not every frame.

These assume `use eframe::egui::{self, Align, Align2, FontData, FontDefinitions, FontFamily, Layout, vec2};` and `use std::sync::Arc;`.
