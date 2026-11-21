//! The header (layout v2 A4): one row, Open ▾ · Undo · Redo | MO2… · Preset · Tools | the view tabs · Settings…, then
//! Export to ▾ · Save · Export at the right edge; then the status line and banner. Drawn in both views (C7).
use super::meshes::UndoTarget;
use crate::look::Combo036 as _;
use crate::model::code_text::counted;
use crate::model::picker::{self, PickerPurpose};
use crate::model::{also, AppModel};
use eframe::egui;
use engine::config::Target;
use engine::presets::AlsoTarget;

/// The Tools entry that opens the PGPatcher JSON editor (R14).
pub const JSON_EDITOR: &str = "PGPatcher JSON editor…";
/// The marker cube command in Tools, or More before a project opens.
pub const MAKE_MARKER: &str = "Make marker cube…";
/// Export's width; Note CD1 puts it beside Save at the right edge.
const EXPORT_WIDTH: f32 = 240.0;
/// What the export menu's button says with no project open (Note FI5, was Note FB1's "no folder chosen").
const MORE_TEXT: &str = "More ▾";
/// What it says with a project open and no export folder chosen for it (Note FI5).
const NOT_CHOSEN_TEXT: &str = "Export to: not chosen ▾";
/// Character budget after overflow, when the drive and last folder no longer fit (K1).
const PATH_MIN_CHARS: usize = 12;

pub struct HeaderWidths {
    /// Fixed controls with the narrower of the full and readable destinations (Note LC1).
    pub fixed: f32,
    pub tools: f32,
    pub mo2: f32,
    pub preset: f32,
    pub more: f32,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Overflow {
    pub tools: bool,
    pub mo2: bool,
    pub preset: bool,
}

impl Overflow {
    fn any(&self) -> bool {
        self.tools || self.mo2 || self.preset
    }
}

/// Move optional controls in order. Each width includes its following gap.
pub fn overflow(avail: f32, w: &HeaderWidths) -> Overflow {
    let all = w.fixed + w.tools + w.mo2 + w.preset;
    if all <= avail {
        return Overflow::default();
    }
    Overflow {
        tools: w.tools > 0.0,
        mo2: w.mo2 > 0.0 && w.fixed + w.more + w.mo2 + w.preset > avail,
        preset: w.preset > 0.0 && w.fixed + w.more + w.preset > avail,
    }
}
/// Why the export folder cannot change now.
pub const EXPORT_RUNNING_HOVER: &str = "An export is running: change the folder after it ends";
/// The Tools item that makes the next export encode every recorded output again (Notes JA11, JA12).
pub const REENCODE_TEXT: &str = "Re-encode every output";
const REENCODE_HOVER: &str =
    "Encode every output again, even when nothing changed. Files Tessalith has no record of are kept.";

pub(super) use crate::model::code_text::target_name;

pub(super) fn target_help(t: Target) -> &'static str {
    match t {
        Target::Cs => "Community Shaders: TruePBR sets and its packed maps",
        Target::Vanilla => {
            "The base game's shaders and texture layout, plus complex material and complex grass"
        }
    }
}

/// `Tessalith — <folder>`, plus ` ●` when there are unsaved edits.
pub fn window_title(model: &AppModel) -> String {
    let mut t = match &model.root {
        Some(r) => format!("{} — {}", crate::PRODUCT_NAME, r.display()),
        None => crate::PRODUCT_NAME.to_string(),
    };
    // Note FI4: automatic types alone are not marked.
    if model.unsaved() {
        t.push_str(" ●");
    }
    t
}

/// Send the title only when it changes.
fn set_title(ctx: &egui::Context, title: String) {
    let id = egui::Id::unique("sdc-window-title");
    let same = ctx.data(|d| d.get_temp::<String>(id).is_some_and(|t| t == title));
    if !same {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
        ctx.data_mut(|d| d.insert_temp(id, title));
    }
}

pub fn draw(
    ui: &mut egui::Ui,
    model: &mut AppModel,
    scan: Option<&mut crate::model::meshes::MeshesModel>,
) {
    set_title(ui.ctx(), window_title(model));
    super::panel_ids::top(ui, "header").show(ui, |ui| {
        // AG126: the row wraps in a narrow window instead of drawing controls on top of each other. A group that is one
        // nested layout (a combo, a disabled scope) is moved whole by `fit`.
        ui.horizontal_wrapped(|ui| row(ui, model, scan));
    });
    status(ui, model);
}

/// A4, left to right: Open ▾ · Undo · Redo | MO2… · Preset · Tools | the view tabs · Settings…, then Export to ▾ · Save ·
/// Export pushed to the right edge.
fn row(
    ui: &mut egui::Ui,
    model: &mut AppModel,
    mut scan: Option<&mut crate::model::meshes::MeshesModel>,
) {
    // Save and Export's height from the start, so every control centres on one line (not lower after the taller tabs).
    ui.set_row_height(2.0 * ui.spacing().interact_size.y);
    // Keep the compact spacing on narrow project headers.
    if model.root.is_some() && ui.available_width() < 1280.0 {
        ui.spacing_mut().item_spacing.x = crate::look::COMPACT_HEADER_GAP;
    }
    open_menu(ui, model);
    let edits = super::edit_controls_visible(ui.ctx(), model);
    if edits {
        undo_redo(ui, model);
    }
    let separator = ui.separator().rect.width();
    let gap = ui.spacing().item_spacing.x;
    let end = measure_export_end(ui, model, edits);
    let preset = preset_text(model);
    let combo = (button_width(ui, &preset) + ui.spacing().icon_width + ui.spacing().icon_spacing)
        .max(ui.spacing().combo_width);
    // Open and history already occupy their measured widths. Reserve the rest of the fixed row.
    let widths = HeaderWidths {
        fixed: separator
            + gap
            + view_tabs_width(ui)
            + button_width(ui, "Settings…")
            + gap
            + end.give_way_width,
        tools: if model.root.is_some() {
            button_width(ui, "Tools ▾") + gap
        } else {
            0.0
        },
        mo2: if scan.is_some() {
            button_width(ui, "MO2…") + gap
        } else {
            0.0
        },
        preset: button_width(ui, "Preset") - 2.0 * ui.spacing().button_padding.x
            + combo
            + 2.0 * gap,
        more: button_width(ui, "⋯") + gap,
    };
    let moved = overflow(room(ui), &widths);
    // Note JI1: the Mode control is in the Settings tab (FI16); MO2… stands alone, where the app has a scan model.
    if let Some(m) = scan.as_deref_mut().filter(|_| !moved.mo2) {
        fit(ui, widths.mo2);
        super::meshes::mo2_button(ui, m);
    }
    if !moved.preset {
        preset_combo(ui, model, preset);
    }
    if model.root.is_some() && !moved.tools {
        tools_menu(ui, model);
    }
    if moved.any() {
        overflow_menu(ui, model, scan, &moved);
    }
    ui.separator();
    view_toggle(ui, model);
    settings_button(ui, model);
    right_end(ui, model, edits, end);
}

fn overflow_menu(
    ui: &mut egui::Ui,
    model: &mut AppModel,
    scan: Option<&mut crate::model::meshes::MeshesModel>,
    moved: &Overflow,
) {
    let more = crate::look::menu_button_036(ui, "⋯", |ui| {
        if moved.tools {
            tools_menu(ui, model);
        }
        if let Some(scan) = scan.filter(|_| moved.mo2) {
            super::meshes::mo2_button(ui, scan);
        }
        if moved.preset {
            super::path_picker::menu(ui, "Preset", |ui| scroll_preset_menu(ui, model));
        }
    })
    .response
    .accessible_name("More header items")
    .on_hover_text("Show more header controls");
    if moved.preset {
        requested_preset_menu(ui, model, &more);
    }
}

/// Note JI1: "Settings…" where the column eyes were (they are the Settings tab's Columns group now), after the view
/// tabs. Drawn always, also with no folder and in the Meshes view (survey correction 2).
fn settings_button(ui: &mut egui::Ui, model: &mut AppModel) {
    const TEXT: &str = "Settings…";
    fit(ui, button_width(ui, TEXT));
    let r = ui
        .button(TEXT)
        .on_hover_text("App settings, and this project's mode, preset and export folder");
    // Note FC8, HS9: the tab hides the view in front, so a focused field commits first.
    super::commit_click(ui, &r, model, AppModel::open_settings_tab);
}

struct ExportEnd {
    lead: &'static str,
    full_path: Option<String>,
    buttons_width: f32,
    min_width: f32,
    normal_width: f32,
    give_way_width: f32,
    readable: String,
}

/// Reserve the narrower of full and readable for overflow. Allow a shorter tail before wrapping (Note LC1).
fn measure_export_end(ui: &egui::Ui, model: &AppModel, edits: bool) -> ExportEnd {
    let gap = ui.spacing().item_spacing.x;
    let full_path = model
        .root
        .as_ref()
        .and_then(|_| model.export_folder_chosen())
        .map(|p| p.display().to_string());
    let lead = match (model.root.is_some(), full_path.is_some()) {
        (false, _) => MORE_TEXT,
        (true, false) => NOT_CHOSEN_TEXT,
        (true, true) => "Export to:  ▾",
    };
    let buttons_width = if model.root.is_some() {
        gap + button_width(ui, "Save")
    } else {
        0.0
    } + if edits { gap + EXPORT_WIDTH } else { 0.0 };
    let (normal_width, give_way_width, min_width, readable) = if let Some(path) = &full_path {
        let width = |p: &str| button_width(ui, &format!("Export to: {p} ▾")) + buttons_width;
        let base = cut_base(path);
        let readable = readable_path(path);
        let full = width(path);
        let give_way = full.min(width(&readable));
        let min = give_way.min(width(&elide_middle(base, PATH_MIN_CHARS)));
        (full, give_way, min, readable)
    } else {
        let width = button_width(ui, lead) + buttons_width;
        (width, width, width, String::new())
    };
    ExportEnd {
        normal_width,
        lead,
        full_path,
        buttons_width,
        min_width,
        give_way_width,
        readable,
    }
}

/// Export to ▾, Save and Export against the row's right edge; Save and Export two rows tall (A4).
fn right_end(ui: &mut egui::Ui, model: &mut AppModel, edits: bool, end: ExportEnd) {
    let gap = ui.spacing().item_spacing.x;
    let h = 2.0 * ui.spacing().interact_size.y;
    let save_w = button_width(ui, "Save");
    fit(ui, end.min_width);
    // end_row resets the height. Centre the chip on Save and Export on the new row too.
    ui.set_row_height(h);
    let text = match end.full_path.as_ref() {
        None => end.lead.to_string(),
        Some(p) if end.normal_width <= room(ui) => format!("Export to: {p} ▾"),
        Some(p) => {
            let base = cut_base(p);
            let glyph = button_width(ui, "0") - 2.0 * ui.spacing().button_padding.x;
            let fixed = button_width(ui, end.lead) + end.buttons_width;
            let estimate = ((room(ui) - fixed - gap) / glyph).floor().max(0.0) as usize;
            let floor = if end.give_way_width <= room(ui) {
                end.readable
            } else {
                elide_middle(base, PATH_MIN_CHARS)
            };
            let stop = floor.chars().count();
            let mut chars = estimate;
            loop {
                if chars <= stop {
                    break format!("Export to: {floor} ▾");
                }
                let text = format!("Export to: {} ▾", elide_middle(base, chars));
                if button_width(ui, &text) + end.buttons_width <= room(ui) - 0.5 {
                    break text;
                }
                chars -= 1;
            }
        }
    };
    push_right(ui, button_width(ui, &text) + end.buttons_width);
    export_to_menu(ui, model, text, end.full_path);
    if model.root.is_some() {
        save_button(ui, model, egui::vec2(save_w, h));
    }
    if edits {
        export_button(ui, model, egui::vec2(EXPORT_WIDTH, h));
    }
}

fn save_button(ui: &mut egui::Ui, model: &mut AppModel, size: egui::Vec2) {
    // Note FC8: a focused text field may hold the only unsaved edit, so Save is live while one has the focus.
    let unsaved = model.unsaved();
    let r = ui.add_enabled(
        unsaved || super::actions::text_field_focused(ui.ctx()),
        egui::Button::new("Save").min_size(size),
    );
    let r = if unsaved {
        r
    } else {
        r.on_hover_text("Nothing to save.")
            .on_disabled_hover_text("Nothing to save.")
    };
    // Note FC8 (supersedes BS7's refusal): the field commits first, then this click saves. That relies on the header
    // and the bottom panel drawing before the central fields (`blur_on_press` runs first); `after_commit` covers a
    // press and release in one frame.
    super::commit_click(ui, &r, model, |m| {
        if m.unsaved() {
            // A failed save raises its own dialog.
            let _ = m.save();
        }
    });
}

/// `path` cut in the middle to at most `max_chars` characters: the first component (`C:`), `…`, then as many whole
/// trailing components as fit. A path that fits is returned as it is; when even the last component does not fit,
/// `…` and its last `max_chars - 1` characters.
pub fn elide_middle(path: &str, max_chars: usize) -> String {
    let len = |s: &str| s.chars().count();
    if len(path) <= max_chars {
        return path.to_string();
    }
    let parts: Vec<&str> = path.split(['\\', '/']).collect();
    let sep = path
        .chars()
        .find(|c| *c == '\\' || *c == '/')
        .unwrap_or('\\');
    let last = parts.last().copied().unwrap_or(path);
    let head = format!("{}{sep}…", parts[0]);
    let mut tail = String::new();
    for p in parts[1..].iter().rev() {
        let next = format!("{sep}{p}{tail}");
        if len(&head) + len(&next) > max_chars {
            break;
        }
        tail = next;
    }
    if tail.is_empty() {
        let keep = max_chars.saturating_sub(1);
        let skip = len(last).saturating_sub(keep);
        return format!("…{}", last.chars().skip(skip).collect::<String>());
    }
    format!("{head}{tail}")
}

/// Keep the drive and the whole last folder. Cut the middle only when the ellipsis saves characters.
pub fn readable_path(path: &str) -> String {
    let base = cut_base(path);
    let Some((drive, rest)) = base.split_once(['\\', '/']) else {
        return base.to_owned();
    };
    let Some((middle, leaf)) = rest.rsplit_once(['\\', '/']) else {
        return base.to_owned();
    };
    if middle.chars().count() <= 1 {
        return base.to_owned();
    }
    let sep = &base[drive.len()..drive.len() + 1];
    format!("{drive}{sep}…{sep}{leaf}")
}

fn cut_base(path: &str) -> &str {
    let trimmed = path.trim_end_matches(['\\', '/']);
    if trimmed.contains(['\\', '/']) {
        trimmed
    } else {
        path // Keep a root's separator.
    }
}

/// What is left of this line of a wrapped row.
fn room(ui: &egui::Ui) -> f32 {
    ui.max_rect().right() - ui.cursor().min.x
}

/// Moves the cursor so a `w`-wide widget ends at the row's right edge.
fn push_right(ui: &mut egui::Ui, w: f32) {
    let s = room(ui) - w - 0.5;
    if s > 0.0 {
        ui.add_space(s);
    }
}

/// Export, `size` big. While an export runs its lower half is the progress bar (done/total; the file on hover).
/// E3: it reads `Review {n} files…` and a click opens Review, unless the project opted out of Review and nothing
/// needs the user's eyes: then it reads `Export {n} files now` and a click writes. A clean project keeps the accent
/// fill either way (Note FB1).
fn export_button(ui: &mut egui::Ui, model: &mut AppModel, size: egui::Vec2) {
    let can_export = model.can_export();
    let running = model.export.running.is_some();
    let gap = ui.spacing().item_spacing.y;
    let button_h = if running {
        (size.y - gap) / 2.0
    } else {
        size.y
    };
    // Once per frame (ruling 3c-28): the text, the accent and the hover all read the same answer.
    let answer = model.header_answer();
    let mut export = egui::Button::new(egui::RichText::new(model.export_text(&answer)).strong())
        .min_size(egui::vec2(size.x, button_h));
    if answer.accent {
        export = export.fill(ui.visuals().selection.bg_fill);
    }
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Center), |ui| {
        // A12: the hover counts the files and the problems. FB1: and, while it reads Review, what is still open.
        // Built lazily, only while hovered (ui-rules 9.3).
        let hover = |ui: &mut egui::Ui| {
            // UI-D final review Minor 5: with no folder the button is off; Review opens from the NIF edit panel.
            if model.root.is_none() && !model.nif_review_rows().is_empty() {
                ui.label("Open Review from the NIF edit panel (Review…)");
                return;
            }
            // Note FC5 (supersedes CZ6's hover): NIF edits are Review rows, so the Meshes view's line is gone.
            ui.label(export_hover_for(model, answer));
            if !answer.now {
                if let Some(why) = model.header_review_reason() {
                    ui.label(format!("Review first: {why}"));
                }
            }
        };
        let r = ui
            .add_enabled(can_export, export)
            .on_hover_ui(hover)
            .on_disabled_hover_ui(hover);
        // Export saves first: a focused field commits before it runs (Note FC8, supersedes BS14's refusal). Ruling
        // 3c-38: the click carries what the button saw when it fired, in case it waits a frame.
        let press = model.export_press();
        super::commit_click(ui, &r, model, move |m| m.header_export_pressed(press));
        if running {
            let (done, total) = model.export.progress();
            let frac = if total == 0 {
                0.0
            } else {
                done as f32 / total as f32
            };
            let hover = match model.export.current_file() {
                Some(f) => format!("Exporting {f}. Cancel is in the Files tab."),
                None => "Exporting. Cancel is in the Files tab.".to_string(),
            };
            ui.add(
                egui::ProgressBar::new(frac)
                    .desired_width(size.x)
                    .desired_height(size.y - gap - button_h)
                    .text(format!("{done}/{total}"))
                    .animate(false),
            )
            .on_hover_text(hover);
            // The drawer also asks; the bar must move even with the drawer closed.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(500));
        }
    });
}

/// Export's hover (A12): "{n} files, {m} problems" ("1 file, 1 problem" for one, `counted`); with an "Also export
/// as" tick on, "{n} files + Vanilla folder, {m} problems" (Note JF1: the count is the PBR folder's).
pub fn export_hover(model: &AppModel) -> String {
    export_hover_for(model, model.header_answer())
}

/// [`export_hover`] for the write-now answer the header already has this frame.
fn export_hover_for(model: &AppModel, answer: crate::model::review::HeaderAnswer) -> String {
    let n = model.header_problem_count();
    let files = counted(answer.files, "file", "files");
    let n = counted(n, "problem", "problems");
    let mut text = if model.also_on() {
        format!("{files} + {}, {n}", model.also_extra_folders())
    } else {
        format!("{files}, {n}")
    };
    // E3: the button reads "Export {n} files now": say why a click writes.
    if answer.now {
        text.push_str(
            "\nWrites at once: Review is off for this project. Turn it back on in Settings.",
        );
    }
    text
}

/// The "Also export as" ticks: target, text, hover. Export to ▾ and the set editor's Also export tab (Note JI2) show
/// them.
pub(super) const ALSO_ITEMS: [(AlsoTarget, &str, &str); 2] = [
    (
        AlsoTarget::Vanilla,
        "Also export as Vanilla",
        "Also write a Vanilla folder: the PBR sets converted to vanilla textures, the rest copied",
    ),
    (
        AlsoTarget::Cm,
        "Also export as Complex material",
        "Also write a Complex material folder (and the Vanilla folder it installs on)",
    ),
];

/// Export to ▾'s two "Also export as" items (owner answer 5): checkable, and the menu stays open on a tick. Ticking
/// needs the cs mode (TA20); a tick left on stays clickable so it can be cleared (the model allows the untick).
fn also_items(
    ui: &mut egui::Ui,
    model: &AppModel,
    busy: Option<&str>,
) -> Option<(AlsoTarget, bool)> {
    let (v, c) = model.also_ticks();
    let mut out = None;
    for ((t, text, hover), on) in ALSO_ITEMS.into_iter().zip([v, c]) {
        let why = also::tick_off(model, on, busy);
        let mut now = on;
        let r = ui.add_enabled(why.is_none(), egui::Checkbox::new(&mut now, text));
        let r = match &why {
            Some(w) => r.on_disabled_hover_text(w),
            None => r.on_hover_text(hover),
        };
        if r.changed() {
            out = Some((t, now));
        }
    }
    out
}

/// The first unmet condition for editing the project's PGPatcher JSON.
pub(super) fn shows_json_editor(model: &AppModel) -> Result<(), &'static str> {
    if model.root.is_none() {
        Err("Open a folder first.")
    } else if model.target != Target::Cs {
        Err("Switch to Community Shaders mode in Settings.")
    } else if model.sets_doc.is_none() {
        Err("Load the project's sets document first.")
    } else {
        Ok(())
    }
}

fn marker_item(ui: &mut egui::Ui, model: &mut AppModel) {
    if ui
        .add_enabled(!model.picker.busy(), egui::Button::new(MAKE_MARKER))
        .on_hover_text("Write a 1×1 marker cube (a solid F0 colour and roughness) that Community Shaders reads")
        .on_disabled_hover_text(picker::BUSY_HOVER)
        .clicked()
    {
        model.open_marker_dialog();
        ui.close();
    }
}

fn tools_menu(ui: &mut egui::Ui, model: &mut AppModel) {
    let config = egui::menu::MenuConfig::find(ui)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    let menu = crate::look::menu_button_036_with_config(ui, "Tools ▾", config, |ui| {
        let available = shows_json_editor(model);
        let r = ui.add_enabled(available.is_ok(), egui::Button::new(JSON_EDITOR));
        let r = match available {
            Ok(()) => r.on_hover_text(
                "Edit the whole PGPatcher JSON: its file name, every set's entry and the delete sections",
            ),
            Err(why) => r.on_disabled_hover_text(why),
        };
        if r.clicked() {
            model.open_json_editor();
            ui.close();
        }
        marker_item(ui, model);
        let mut reencode = model.reencode_all;
        if ui
            .checkbox(&mut reencode, REENCODE_TEXT)
            .on_hover_text(REENCODE_HOVER)
            .changed()
        {
            model.set_reencode_all(reencode);
        }
    });
    menu.response.accessible_name("Tools");
}

/// Starts a new line of a wrapped row unless `width` fits in what is left of this one. A nested layout (a combo, a
/// scope) is placed where the cursor is; only single widgets wrap by themselves.
fn fit(ui: &mut egui::Ui, width: f32) {
    if ui.max_rect().right() - ui.cursor().min.x < width {
        ui.end_row();
    }
}

/// The width a button with `text` takes.
fn button_width(ui: &egui::Ui, text: &str) -> f32 {
    #[cfg(test)]
    if matches!(text, "Undo" | "Redo" | "Undo NIF edit" | "Redo NIF edit") {
        tests::HISTORY_MEASURES.set(tests::HISTORY_MEASURES.get() + 1);
    }
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    galley.size().x + 2.0 * ui.spacing().button_padding.x
}

/// Open ▾ (R14): Open folder… (off while a file dialog is open), then the recent folders.
fn open_menu(ui: &mut egui::Ui, model: &mut AppModel) {
    let mut open = None;
    let mut pick = false;
    crate::look::menu_button_036(ui, "Open", |ui| {
        if picker::button(ui, &model.picker, egui::Button::new("Open folder…")) {
            pick = true;
            ui.close();
        }
        ui.separator();
        if model.settings.recent.is_empty() {
            ui.add_enabled(false, egui::Button::new("No recent folders"));
        }
        for p in &model.settings.recent {
            if ui.button(p.display().to_string()).clicked() {
                open = Some(p.clone());
                ui.close();
            }
        }
    });
    if pick {
        model.pick(
            ui.ctx(),
            PickerPurpose::OpenFolder,
            model.open_folder_spec(),
        );
    }
    if let Some(p) = open {
        model.request_open(p);
    }
}

/// One header Undo or Redo button as drawn: its text, its accessible name (E28: `Undo: {step}` for the Textures
/// history, `Undo (NIF edit history): {step}` while the NIF history is the target, without the step when there is
/// none), its hover (which holds the whole step).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoButton {
    pub text: String,
    pub name: String,
    pub hover: String,
    pub enabled: bool,
}

/// The header's Undo (`undo`) or Redo button for the history it acts on (Note FC12, CI23 labelled): `Undo` for the
/// Textures one, whose next step is `textures`, or `Undo NIF edit` while a NIF edit is the target (`nif`).
pub fn undo_button(nif: Option<&UndoTarget>, undo: bool, textures: Option<&str>) -> UndoButton {
    let verb = if undo { "Undo" } else { "Redo" };
    let keys = super::actions::keys_text(if undo { "undo" } else { "redo" });
    // (the button's full text, what the hover says it does, enabled, the history, the accessible name). E28: the name
    // says the step, unshortened; while the NIF history is the target it also says whose history it is.
    let (text, does, enabled, history, name) = match nif {
        Some(t) => {
            let (can, label) = if undo {
                (t.can_undo, &t.undo_label)
            } else {
                (t.can_redo, &t.redo_label)
            };
            let (text, name) = match label.as_deref().filter(|_| can) {
                Some(l) => (
                    format!("{verb} NIF edit: {l}"),
                    format!("{verb} (NIF edit history): {l}"),
                ),
                None => (
                    format!("{verb} NIF edit"),
                    format!("{verb} (NIF edit history)"),
                ),
            };
            (
                format!("{verb} NIF edit"),
                text,
                can,
                "the NIF edit history; the Textures history is untouched",
                name,
            )
        }
        None => {
            let does = textures.map_or_else(String::new, |s| format!("{verb}: {s}"));
            let name = if textures.is_some() {
                does.clone()
            } else {
                verb.to_string()
            };
            (
                verb.to_string(),
                does,
                textures.is_some(),
                "the Textures history (sdc.toml and sdc-project.toml)",
                name,
            )
        }
    };
    let hover = match (enabled, nif.is_some()) {
        (false, true) => format!("Nothing to {} in the NIF edit history", verb.to_lowercase()),
        (false, false) => format!("Nothing to {}", verb.to_lowercase()),
        (true, _) => format!("{does}  ({keys}). Acts on {history}."),
    };
    UndoButton {
        text,
        name,
        hover,
        enabled,
    }
}

fn history_slot_width(ui: &egui::Ui) -> f32 {
    let (undo_text, redo_text) = if meshes_view_active(ui.ctx()) {
        ("Undo NIF edit", "Redo NIF edit")
    } else {
        ("Undo", "Redo")
    };
    let font = egui::FontSelection::default()
        .resolve_with_fallback(ui.style(), egui::TextStyle::Button.into());
    let key = egui::Id::unique((
        undo_text,
        redo_text,
        font,
        ui.ctx().pixels_per_point().to_bits(),
        ui.spacing().button_padding.x.to_bits(),
    ));
    let id = egui::Id::unique("header-history-width");
    if let Some((_, width)) = ui
        .data(|d| d.get_temp::<(egui::Id, f32)>(id))
        .filter(|(saved, _)| *saved == key)
    {
        return width;
    }
    let width = button_width(ui, undo_text).max(button_width(ui, redo_text));
    ui.data_mut(|d| d.insert_temp(id, (key, width)));
    width
}

/// The Undo and Redo buttons (item 2): disabled with no history, the step's label in the hover text.
fn undo_redo(ui: &mut egui::Ui, model: &mut AppModel) {
    // A20: the NIF history while a NIF edit is open in the Meshes view.
    let nif = super::actions::nif_history(ui.ctx(), model);
    let slot = egui::vec2(history_slot_width(ui), ui.spacing().interact_size.y);
    for undo in [true, false] {
        let textures = if undo {
            model.undo_label()
        } else {
            model.redo_label()
        };
        let b = undo_button(nif.as_ref(), undo, textures);
        let r = ui.add_enabled(b.enabled, egui::Button::new(&b.text).min_size(slot));
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, b.enabled, &b.name));
        let r = r
            .on_hover_text(b.hover.clone())
            .on_disabled_hover_text(b.hover);
        // Backlog T20: a focused text field commits first, so its step is the one undone and Redo is not cleared.
        let ctx = ui.ctx().clone();
        super::commit_click(ui, &r, model, move |m| {
            super::actions::history_step(&ctx, m, undo);
        });
    }
}

/// The project's preset, or the tagged target default when the project follows it.
pub(super) fn preset_text(model: &AppModel) -> String {
    model
        .project_preset()
        .unwrap_or_else(|| default_text(engine::presets::default_preset_for(model.target)))
}

fn default_text(id: &str) -> String {
    format!("{id} · default")
}

/// The Preset menu's entries (AM12): the tagged default, then each other preset of this target (a user preset appears
/// once saved); off with no project. The header's combo and the Settings tab's (Note JI1) draw it. The pick (`None` =
/// the default), only when it differs from the project's preset.
pub(super) fn preset_entries(ui: &mut egui::Ui, model: &mut AppModel) -> Option<Option<String>> {
    let default = engine::presets::default_preset_for(model.target);
    let cur = model.project_preset();
    let choices = model.preset_choices();
    let mut picked = None;
    ui.add_enabled_ui(model.project.is_some(), |ui| {
        let entry = |ui: &mut egui::Ui, on: bool, text: &str| {
            ui.selectable_label(on, text)
                .on_disabled_hover_text("Open a folder first")
                .clicked()
        };
        if entry(
            ui,
            cur.as_deref().is_none_or(|id| id == default),
            &default_text(default),
        ) {
            picked = Some(None);
        }
        for id in choices.iter().filter(|id| *id != default) {
            if entry(ui, cur.as_deref() == Some(id.as_str()), id) {
                picked = Some(Some(id.clone()));
            }
        }
    });
    if picked.is_some() {
        ui.close();
    }
    picked.filter(|p| *p != cur)
}

/// The project's preset (AM12), and Presets….
fn preset_combo(ui: &mut egui::Ui, model: &mut AppModel, text: String) {
    let mode = target_name(model.target);
    let gap = ui.spacing().item_spacing.x;
    fit(
        ui,
        button_width(ui, "Preset") + gap + ui.spacing().combo_width,
    );
    ui.label("Preset");
    // The combo's inner horizontal row must reserve the button's text height before it centres itself.
    let interact_h = ui.spacing().interact_size.y;
    ui.spacing_mut().interact_size.y = interact_h
        .max(ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y);
    let combo = egui::ComboBox::from_id_salt("preset")
        .selected_text(text)
        .show_ui_036(ui, |ui| preset_menu(ui, model))
        .response;
    ui.spacing_mut().interact_size.y = interact_h;
    let combo = super::name_input(combo, "Preset").on_hover_ui(|ui| {
        ui.label(format!(
            "The preset the folder converts with. Saved in sdc.toml as preset. Mode: {mode}; the list shows this mode's presets. Change the mode in Settings…"
        ));
    });
    requested_preset_menu(ui, model, &combo);
}

/// FC1: open the same choices under whichever header control currently holds Preset.
fn requested_preset_menu(ui: &egui::Ui, model: &mut AppModel, anchor: &egui::Response) {
    let now = ui.ctx().cumulative_pass_nr();
    let request = model.choose_preset.take().is_some_and(|at| now <= at + 2);
    egui::Popup::menu(anchor)
        .id(egui::Id::unique("preset_choose_menu"))
        .open_memory(request.then_some(egui::SetOpenCommand::Bool(true)))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| scroll_preset_menu(ui, model));
}

fn scroll_preset_menu(ui: &mut egui::Ui, model: &mut AppModel) {
    ui.spacing_mut().scroll = crate::look::thin_scroll();
    let height = ui
        .spacing()
        .combo_height
        .min(ui.ctx().content_rect().height() - ui.spacing().menu_margin.sum().y);
    egui::ScrollArea::vertical()
        .max_height(height)
        .show(ui, |ui| preset_menu(ui, model));
}

fn preset_menu(ui: &mut egui::Ui, model: &mut AppModel) {
    if let Some(p) = preset_entries(ui, model) {
        model.set_preset(p);
    }
    ui.separator();
    if ui
        .button("Presets…")
        .on_hover_text("Make and edit your own presets (saved in %APPDATA%\\sdc\\presets)")
        .clicked()
    {
        model.open_presets();
        ui.close();
    }
}

/// Top-level view toggle (plan A Task 12), kept in egui memory for the session. Note BH8: two tabs, taller than the
/// row, rounded on top only; the selected one is filled and bold. R21: they show "Textures" / "Meshes" and keep the
/// accessible names "Textures view" / "Meshes view".
fn view_toggle(ui: &mut egui::Ui, model: &mut AppModel) {
    let meshes = meshes_view_active(ui.ctx());
    let tab = |ui: &mut egui::Ui, on: bool, text: &str, name: &str| {
        let rich = egui::RichText::new(text);
        let w = button_width(ui, "Textures") + 12.0;
        let h = ui.spacing().interact_size.y + 6.0;
        let r = ui.add(
            egui::Button::selectable(on, if on { rich.strong() } else { rich })
                .frame_when_inactive(true)
                .corner_radius(egui::CornerRadius {
                    nw: 6,
                    ne: 6,
                    sw: 0,
                    se: 0,
                })
                .min_size(egui::vec2(w, h)),
        );
        let enabled = ui.is_enabled();
        r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, enabled, on, name));
        r
    };
    fit(ui, view_tabs_width(ui));
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (target, text, name) in [
            (false, "Textures", "Textures view"),
            (true, "Meshes", "Meshes view"),
        ] {
            let r = tab(ui, meshes == target, text, name);
            super::blur_on_press(ui, r.rect);
            if r.clicked() {
                super::navigate(ui.ctx(), model, move |ctx, m| {
                    m.centre_over_meshes = false;
                    set_meshes_view(ctx, target);
                });
            }
        }
    });
}

/// The width `view_toggle` takes: two tabs and the spacing after them.
fn view_tabs_width(ui: &egui::Ui) -> f32 {
    2.0 * (button_width(ui, "Textures") + 12.0) + 2.0 + ui.spacing().item_spacing.x
}

/// Switch the view (also the saved one coming back at start, AG17).
pub fn set_meshes_view(ctx: &egui::Context, meshes: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("sdc-meshes-view"), meshes));
}

/// True when the Meshes view is shown instead of the Textures view.
pub fn meshes_view_active(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::unique("sdc-meshes-view")))
        .unwrap_or(false)
}

/// Why the export folder cannot be chosen now, or `None`: a file dialog is open, or an export runs (AG129: the
/// drawer's links point at the export folder, so it stays put meanwhile). Export to ▾ and the Settings tab (Note JI1).
pub(super) fn export_folder_off(model: &AppModel) -> Option<&'static str> {
    if model.picker.busy() {
        Some(picker::BUSY_HOVER)
    } else if model.export.running.is_some() {
        Some(EXPORT_RUNNING_HOVER)
    } else {
        None
    }
}

/// Export to ▾ (A4): `text` shows the folder cut in the middle; the hover and the accessible name keep all of it (K8).
/// With no project the button reads "More ▾" and also offers the standalone marker tool.
fn export_to_menu(ui: &mut egui::Ui, model: &mut AppModel, text: String, full: Option<String>) {
    let mut choose = false;
    let mut open = false;
    let mut tick = None;
    // A click inside keeps it open (egui's default closes on any click), so the ticks can be set without reopening;
    // commands that open a folder or dialog close it themselves.
    let r = super::path_picker::menu(ui, &text, |ui| {
        let off = export_folder_off(model);
        let r = ui.add_enabled(off.is_none(), egui::Button::new("Choose export folder…"));
        let r = match off {
            Some(why) => r.on_disabled_hover_text(why),
            None => r.on_hover_text("Choose where converted textures are written"),
        };
        choose = r.clicked();
        // A2: stat only while the menu is open.
        let state = model.export_folder_state();
        let r = ui.add_enabled(state.folder.is_ok(), egui::Button::new(state.label));
        open = match &state.folder {
            Ok(p) => r.on_hover_text(format!("Show {} in Explorer", p.display())),
            Err(why) => super::disabled_reason(r, why),
        }
        .clicked();
        let running = model
            .export
            .running
            .is_some()
            .then_some(EXPORT_RUNNING_HOVER);
        tick = also_items(ui, model, running);
        if model.root.is_none() {
            marker_item(ui, model);
        }
        if choose || open {
            ui.close();
        }
    });
    let name = match (&full, model.root.is_some()) {
        (Some(f), _) => format!("Export to: {f}"),
        (None, true) => "Export to: not chosen".to_string(),
        (None, false) => "More export options".to_string(),
    };
    let enabled = ui.is_enabled();
    r.response
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &name));
    let root_open = model.root.is_some();
    r.response.on_hover_text(match full {
        Some(f) => f,
        None if root_open => {
            "No export folder chosen yet: choose one here or in Review".to_string()
        }
        None => "Open a folder to choose where its textures are written".to_string(),
    });
    if choose {
        model.pick_export_folder(ui.ctx());
    }
    if open {
        // As the Files tab's: the entry is enabled only when the folder is there.
        let _ = model.open_export_folder();
    }
    if let Some((t, on)) = tick {
        model.set_also_tick(t, on);
    }
}

fn status(ui: &mut egui::Ui, model: &mut AppModel) {
    if model.status_line.is_empty() && model.banner.is_none() {
        return;
    }
    super::panel_ids::top(ui, "status").show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            // Item 13: a bar with Cancel while a folder opens; a re-plan after an edit keeps the plain line.
            match model.plan_progress.filter(|_| model.can_cancel_open()) {
                Some((done, total)) => {
                    let frac = if total == 0 {
                        0.0
                    } else {
                        done as f32 / total as f32
                    };
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .desired_width(320.0)
                            .text(&model.status_line),
                    );
                }
                None => {
                    ui.label(&model.status_line);
                }
            }
            if model.can_cancel_open()
                && ui
                    .button("Cancel")
                    .on_hover_text(model.cancel_open_hover())
                    .clicked()
            {
                model.cancel_open();
            }
            if !model.items.is_empty() {
                let stats = super::columns::column_stats(ui.ctx(), model);
                ui.weak(format!("· {}", stats.vram)).on_hover_text(
                    "File sizes of the DDS files an export writes; a compressed size is also its video memory",
                );
            }
        });
        if let Some(b) = model.banner.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(super::status::AMBER, format!("⚠ {b}"));
                if model.project.is_some() && ui.button("Reload").clicked() {
                    model.request_reload();
                }
                if model.kept_banner.is_some()
                    && ui
                        .button("Dismiss")
                        .on_hover_text("Hide the startup warning")
                        .clicked()
                {
                    model.dismiss_kept_banner();
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_tools_keeps_the_checkbox_open_and_closes_on_a_command() {
        use egui::Key;
        use egui_kittest::kittest::Queryable as _;
        use egui_kittest::Harness;

        for nested in [false, true] {
            let mut h = Harness::builder()
                .with_context_setup(|ctx| {
                    crate::fonts::install_system_fonts(ctx).expect("system fonts");
                })
                .build_ui_state(
                    move |ui, m| {
                        if nested {
                            crate::look::menu_button_036(ui, "More", |ui| tools_menu(ui, m));
                        } else {
                            tools_menu(ui, m);
                        }
                    },
                    AppModel::placeholder(Default::default()),
                );
            let press = |h: &mut Harness<'_, AppModel>, key| {
                h.key_press(key);
                h.run();
            };
            press(&mut h, Key::Tab);
            if nested {
                assert!(h.get_by_label("More").is_focused());
                press(&mut h, Key::Enter);
                press(&mut h, Key::ArrowDown);
            }
            assert!(h.get_by_label("Tools").is_focused());
            press(&mut h, if nested { Key::ArrowRight } else { Key::Enter });
            // The JSON command is disabled in this fixture.
            if !h.get_by_label(MAKE_MARKER).is_focused() {
                press(&mut h, Key::ArrowDown);
            }
            assert!(h.get_by_label(MAKE_MARKER).is_focused());
            press(&mut h, Key::ArrowDown);
            assert!(h.get_by_label(REENCODE_TEXT).is_focused());
            press(&mut h, Key::Space);
            assert!(h.state().reencode_all);
            assert!(h.get_by_label(REENCODE_TEXT).is_focused());
            press(&mut h, Key::ArrowUp);
            assert!(h.get_by_label(MAKE_MARKER).is_focused());
            press(&mut h, Key::Enter);
            assert!(matches!(
                h.state().dialog,
                Some(crate::model::Dialog::MakeMarker)
            ));
            assert!(h.query_by_label(REENCODE_TEXT).is_none());
            if nested {
                assert!(h.query_by_label("Tools").is_none());
            }
        }
    }

    thread_local! {
        pub(super) static HISTORY_MEASURES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    #[test]
    fn history_width_reuses_the_measured_slot_on_unchanged_frames() {
        use egui_kittest::kittest::Queryable as _;

        let mut h = egui_kittest::Harness::builder()
            .with_context_setup(|ctx| {
                crate::fonts::install_system_fonts(ctx).expect("system fonts");
            })
            .build_ui_state(
                |ui, (model, meshes, padding, size): &mut (AppModel, bool, f32, f32)| {
                    set_meshes_view(ui.ctx(), *meshes);
                    ui.spacing_mut().button_padding.x = *padding;
                    ui.style_mut().override_font_id = Some(egui::FontId::proportional(*size));
                    ui.horizontal(|ui| undo_redo(ui, model));
                },
                (AppModel::placeholder(Default::default()), false, 4.0, 13.0),
            );
        for (meshes, padding, size, scale) in [
            (false, 5.0, 13.0, 1.0),
            (true, 5.0, 13.0, 1.0),
            (true, 12.0, 13.0, 1.0),
            (true, 12.0, 20.0, 1.0),
            (true, 12.0, 20.0, 1.5),
        ] {
            h.state_mut().1 = meshes;
            h.state_mut().2 = padding;
            h.state_mut().3 = size;
            HISTORY_MEASURES.set(0);
            h.set_pixels_per_point(scale);
            h.run_steps(2);
            let font = egui::FontId::proportional(size);
            let labels = if meshes {
                ["Undo NIF edit", "Redo NIF edit"]
            } else {
                ["Undo", "Redo"]
            };
            let expected = h.ctx.fonts_mut(|f| {
                labels
                    .into_iter()
                    .map(|text| {
                        f.layout_no_wrap(text.into(), font.clone(), egui::Color32::WHITE)
                            .size()
                            .x
                    })
                    .fold(0.0, f32::max)
            }) + 2.0 * padding;
            for label in ["Undo", "Redo"] {
                let width = h.get_by_label(label).rect().width();
                assert!(
                    (width - expected).abs() < 0.5,
                    "{label}: {width} vs {expected}"
                );
            }
            assert!(
                HISTORY_MEASURES.get() >= 2,
                "sizing change must measure the pair"
            );
            HISTORY_MEASURES.set(0);
            h.run_steps(1);
            assert_eq!(
                HISTORY_MEASURES.get(),
                0,
                "unchanged frame measured history labels"
            );
        }
    }

    #[test]
    fn undo_names_the_step() {
        assert_eq!(
            undo_button(None, true, Some("set type of teeth_sss.png")).name,
            "Undo: set type of teeth_sss.png"
        );
        assert_eq!(
            undo_button(None, false, Some("set type of teeth_sss.png")).name,
            "Redo: set type of teeth_sss.png"
        );
        assert_eq!(undo_button(None, true, None).name, "Undo");
    }

    #[test]
    fn nif_undo_names_its_history_and_step() {
        let t = UndoTarget {
            can_undo: true,
            can_redo: false,
            undo_label: Some("move UV".into()),
            redo_label: None,
        };
        assert_eq!(
            undo_button(Some(&t), true, None).name,
            "Undo (NIF edit history): move UV"
        );
    }
}
