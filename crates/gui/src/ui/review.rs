//! The Review tab (layout v2 B2, C2, C5; R1–R4, R11): blockers and undecided textures side by side, what the export
//! writes, and one Export button. Not a dialog: Enter does nothing here (N3).
use super::settings_panel::{alpha_question, subtype_label};
use super::status::{AMBER, RED};
use super::type_picker::{self, TypeKind};
use crate::model::code_text::{
    ack_label, ack_label_for_texture, alpha_confirm_text, change_line, code_line, not_a_cube,
};
use crate::model::items::{alpha_asked, Decision};
use crate::model::picker::{self, PickerPurpose, PickerSpec};
use crate::model::review::{
    DiskState, JsonKind, JsonLine, ReviewWrites, NOT_CHOSEN_ROOT, NO_EXPORT_FOLDER,
};
use crate::model::{also, pbr_export, sets, Answer, AppModel};
use eframe::egui;
use engine::codes::Code;
use engine::config::{Subtype, Target};
use engine::presets::{AlsoTarget, ExportConfig};
use std::path::Path;
use std::sync::Arc;

/// Note FI5: what the Also location reads while no export folder is chosen (the fallback is never shown as a path).
const NOT_CHOSEN_YET: &str = "the export folder (not chosen yet)";

/// What a click on an undecided row asks for.
pub enum RowPick {
    Select(String),
    Decide(String, Subtype),
    Answer(String, Answer),
}

pub fn apply(model: &mut AppModel, pick: RowPick) {
    match pick {
        RowPick::Select(k) => model.select_item(&k),
        RowPick::Decide(k, st) => model.decide_undecided_row(&k, st),
        RowPick::Answer(k, a) => model.answer_undecided_row(&k, a),
    }
}

/// One undecided texture's row: the name link and the Problems row's fix buttons on one line (`Set type: X ✓ ▾`, the
/// alpha dropdown, "Acknowledge for this texture"), then the sentences of its questions on a wrapped line of their own
/// when it has any. The buttons are disabled with `off` as hover; the name link always selects.
pub fn undecided_row(
    ui: &mut egui::Ui,
    model: &AppModel,
    key: &str,
    off: Option<&str>,
) -> Option<RowPick> {
    let item = model.item(key)?;
    let mut pick = None;
    // Backlog L64: the buttons are the Problems row's, so both surfaces offer the same fixes. They stay on the name's
    // line (a wrapped layout would squeeze a button word by word), so a row is one line tall unless it has a sentence.
    ui.horizontal(|ui| {
        if ui.link(&item.name).clicked() {
            pick = Some(RowPick::Select(key.to_string()));
        }
        if let Some(p) = fix_buttons_on(ui, model, key, off, "Review") {
            pick = Some(p);
        }
    });
    // The sentences a Problems row words itself (Note FB8: the code on hover; Note FM35: the alpha line).
    let sentences: Vec<(String, Option<&str>)> = item
        .decisions
        .iter()
        .filter_map(|d| match d {
            Decision::AlphaMeaning => {
                alpha_asked(item).map(|shown| (alpha_confirm_text(shown).to_string(), None))
            }
            Decision::Acknowledge(code) => {
                Some((code_line(*code).to_string(), Some(code.as_str())))
            }
            Decision::Change(code) => Some((change_line(*code).to_string(), Some(code.as_str()))),
            // Note GU2: no menu, only what a cube source is; the type picker on the row is the way out.
            Decision::NotACubeSource => Some((not_a_cube(item.job.as_ref()), None)),
            Decision::Subtype { .. } => None,
        })
        .collect();
    if !sentences.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (text, hover) in &sentences {
                let r = ui.label(text);
                if let Some(h) = hover {
                    r.on_hover_text(*h);
                }
            }
        });
    }
    pick
}

/// The text of a Problems row's one-click accept (Note FB9).
fn accept_text(st: Subtype) -> String {
    format!("Set type: {} \u{2713}", subtype_label(st))
}

/// Two buttons drawn as one, `[text] ▾`, in reading order in either layout direction: (main, arrow).
fn split_button(ui: &mut egui::Ui, text: &str) -> (egui::Response, egui::Response) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if ui.layout().prefer_right_to_left() {
            let arrow = ui.button("\u{25BE}");
            (ui.button(text), arrow)
        } else {
            let main = ui.button(text);
            (main, ui.button("\u{25BE}"))
        }
    })
    .inner
}

/// A Problems row's fix buttons (A1), one set per open question of the texture `key`: the suggestion's one-click
/// accept with ▾ for the type picker, or a plain Set type… (Note FB9), the alpha dropdown and Confirm (Note FM35, HI8),
/// Acknowledge. All disabled with `off` as hover.
pub fn fix_buttons(
    ui: &mut egui::Ui,
    model: &AppModel,
    key: &str,
    off: Option<&str>,
) -> Option<RowPick> {
    fix_buttons_on(ui, model, key, off, "Problems")
}

/// `fix_buttons` on the surface `surface` names (the accessible name of the alpha dropdown).
fn fix_buttons_on(
    ui: &mut egui::Ui,
    model: &AppModel,
    key: &str,
    off: Option<&str>,
    surface: &str,
) -> Option<RowPick> {
    let item = model.item(key)?;
    let why = off.unwrap_or_default();
    let mut pick = None;
    let mut decisions: Vec<_> = item.decisions.iter().collect();
    // A Problems row adds its buttons right to left; keep them in reading order.
    if ui.layout().prefer_right_to_left() {
        decisions.reverse();
    }
    ui.add_enabled_ui(off.is_none(), |ui| {
        for d in decisions {
            match d {
                // Note FB9 (owner amendment A): `[Set type: {suggestion} ✓] ▾`, the main part accepts, ▾ opens the
                // picker; without a suggestion the plain "Set type…" opens it.
                // A not-a-cube source has only this answer too: a different sub-type (no suggestion). Note JF3: so
                // has a type the mode does not have and gives no fallback for.
                d @ (Decision::Subtype { .. }
                | Decision::NotACubeSource
                | Decision::Change(Code::CfgUnavailable)) => {
                    let suggested = match d {
                        Decision::Subtype { suggested } => *suggested,
                        _ => None,
                    };
                    if let Some(p) = set_type_button(ui, model, key, suggested, why) {
                        pick = Some(p);
                    }
                }
                // Note FM35 (supersedes FI11's Decide… menu): the row's line names the type, and this dropdown opens
                // on it; Enter on the row confirms it, as Confirm does (Note HI8).
                Decision::AlphaMeaning => {
                    if let Some(shown) = alpha_asked(item) {
                        let id = egui::Id::unique(("problems-alpha", key));
                        if let Some(p) = alpha_question(ui, id, shown, why, surface) {
                            pick = Some(RowPick::Answer(key.to_string(), Answer::Alpha(p)));
                        }
                    }
                }
                Decision::Acknowledge(code) => {
                    if ui
                        .button(if surface == "Review" {
                            ack_label_for_texture(*code)
                        } else {
                            ack_label(*code).to_string()
                        })
                        .on_hover_text(code_line(*code))
                        .on_disabled_hover_text(why)
                        .clicked()
                    {
                        pick = Some(RowPick::Answer(key.to_string(), Answer::Acknowledge(*code)));
                    }
                }
                // Note JF3: any other code Acknowledge cannot clear has no button: its sentence names the fix.
                Decision::Change(_) => {}
            }
        }
    });
    pick
}

/// Note FB9 (owner amendment A): `[Set type: {suggestion} ✓] ▾`, the main part accepts, ▾ opens the type picker;
/// without a suggestion the plain "Set type…" opens it. `why` is the disabled hover. Review E10: the Problems rows of
/// SUG-CONTRADICTION use it too, on a decided texture, with no suggestion.
pub(super) fn set_type_button(
    ui: &mut egui::Ui,
    model: &AppModel,
    key: &str,
    suggested: Option<Subtype>,
    why: &str,
) -> Option<RowPick> {
    let item = model.item(key)?;
    let mut pick = None;
    let opener = match suggested {
        Some(st) => {
            let (accept, arrow) = split_button(ui, &accept_text(st));
            if accept
                .on_hover_text("Use the suggested type")
                .on_disabled_hover_text(why)
                .clicked()
            {
                pick = Some(RowPick::Decide(key.to_string(), st));
            }
            // ▾ is what the plain button was: the picker, so it keeps its name.
            arrow.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, arrow.enabled(), "Set type…")
            });
            arrow.on_hover_text("Pick another type")
        }
        None => ui.button("Set type…"),
    }
    .on_disabled_hover_text(why);
    let id = egui::Id::unique(("set-type-picker", key));
    let kind = TypeKind::SetType;
    let grass = item.job.as_ref().is_some_and(engine::policy::grass_context);
    let (t, s) = (model.target, suggested);
    if let Some(st) = type_picker::show(ui, id, &opener, None, s, kind, t, grass) {
        pick = Some(RowPick::Decide(key.to_string(), st));
    }
    pick
}

enum Act {
    AlsoTick(AlsoTarget, bool),
    AlsoSet(Box<dyn FnOnce(&mut ExportConfig)>),
    AlsoLocation,
    ChooseExport,
    ChoosePreset,
    Skip(String, bool),
    Recipe,
    GoTo(String),
    Row(RowPick),
    AcceptAll,
    Json(bool),
    Nif(String, bool, [u8; 32]),
    NifDiff(String),
    Export,
    Close,
    OptOut(bool),
}

/// Review's footer tick; the Settings tab's box is "Export without review" (two inputs never share a name).
const OPT_OUT_TEXT: &str = "Export without review next time";

pub(super) fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// R6: one line per JSON file under What will be written.
/// Note FI5: with `unchosen` (the export folder's fallback name) the path is shown under `‹export folder›`.
fn json_line_text(l: &JsonLine, unchosen: Option<&Path>) -> String {
    let path = match unchosen.and_then(|r| l.path.strip_prefix(r).ok()) {
        Some(rel) => format!("{NOT_CHOSEN_ROOT}\\{}", rel.display()),
        None => l.path.display().to_string(),
    };
    match (l.state, l.kind) {
        (DiskState::New, _) => format!("{path}: will be written"),
        (DiskState::Ours, _) => format!("{path}: will be replaced (written by Tessalith)"),
        (DiskState::Edited, JsonKind::PgPatcher) => format!("{path}: edited outside Tessalith"),
        (DiskState::Edited, _) => format!("{path}: edited outside Tessalith — kept, with a note"),
        (DiskState::Unreadable, _) => format!("{path}: cannot be read; kept"),
    }
}

/// Note FC5: one overwrite question in Blockers: the sentence (red until answered), then Review's two radios, neither
/// preselected. Returns Some(overwrite) on a click.
fn overwrite_row(ui: &mut egui::Ui, text: &str, answer: Option<bool>) -> Option<bool> {
    let mut pick = None;
    ui.horizontal_wrapped(|ui| {
        match answer {
            None => ui.colored_label(RED, text),
            Some(_) => ui.label(text),
        };
        if ui
            .radio(answer == Some(false), "Keep the edited file")
            .clicked()
        {
            pick = Some(false);
        }
        if ui.radio(answer == Some(true), "Overwrite it").clicked() {
            pick = Some(true);
        }
    });
    pick
}

/// Note FC5: one NIF edit under `NIF edits (n)`: `path · mod · n unused blocks removed`, or a Warning
/// `NIF edit skipped: {reason}` for a blocked one (it never blocks the texture export).
fn nif_row_text(
    r: &crate::model::meshes::NifReviewRow,
    folder: Option<&std::path::Path>,
) -> String {
    let mut t = r.path.clone();
    if !r.mod_name.is_empty() {
        t.push_str(&format!(" · {}", r.mod_name));
    }
    if let Some(n) = r.compaction.filter(|n| *n > 0) {
        t.push_str(&format!(" · {n} unused block{} removed", plural(n)));
    }
    if let Some(f) = folder {
        t.push_str(&format!(" · written to {}", f.display()));
    }
    t
}

/// Note FC14: one row of the footer's shared list (NIF edits, skipped NIF edits, what will be written).
enum Footer<'a> {
    Heading(String),
    Nif(&'a crate::model::meshes::NifReviewRow),
    NifWaiting(&'a crate::model::meshes::NifReviewRow),
    NifSkipped(&'a crate::model::meshes::NifReviewRow),
    Written(String),
    File(&'a Path, bool),
    Json(&'a JsonLine),
    /// E7: the files besides the outputs the export saves or records (`AppModel::review_writes`).
    Also(&'a Arc<ReviewWrites>),
}

#[cfg(test)]
#[test]
fn a_missing_footer_row_draws_an_empty_heading() {
    let row = std::panic::catch_unwind(
        || matches!(output_row(&ReviewWrites::default(), 0), Footer::Heading(text) if text.is_empty()),
    );
    assert!(
        row.is_ok_and(|empty| empty),
        "missing footer row must be harmless to draw"
    );
}

/// Index the cached groups directly so scrolling to the end of a large project lays out only visible paths.
fn output_row(w: &ReviewWrites, mut index: usize) -> Footer<'_> {
    if !w.outputs.is_empty() {
        if index == 0 {
            return Footer::Heading("Will be written".into());
        }
        if let Some(o) = w.outputs.get(index - 1) {
            return Footer::File(&o.path, o.marked);
        }
        index -= w.outputs.len() + 1;
    }
    for (name, paths) in [
        ("Up to date", &w.current),
        ("Kept", &w.kept),
        ("Protected", &w.protected),
    ] {
        if paths.is_empty() {
            continue;
        }
        if index == 0 {
            return Footer::Heading(name.into());
        }
        if let Some(path) = paths.get(index - 1) {
            return Footer::File(path, false);
        }
        index -= paths.len() + 1;
    }
    Footer::Heading(String::new())
}

/// E7 (ruling 3c-49): the width of each entry's text and of the `…`, measured once per list and font (kept in the
/// context beside the list they were measured from), so a frame lays out no text to measure it.
fn also_widths(ui: &egui::Ui, w: &Arc<ReviewWrites>, font: &egui::FontId) -> Arc<Vec<f32>> {
    type Kept = (Arc<ReviewWrites>, egui::FontId, Arc<Vec<f32>>);
    let id = ui.scope_id().with("also-updated-widths");
    let kept: Option<Kept> = ui.data(|d| d.get_temp(id));
    if let Some((from, f, widths)) = kept {
        if Arc::ptr_eq(&from, w) && f == *font {
            return widths;
        }
    }
    let color = ui.visuals().text_color();
    let width = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_string(), font.clone(), color)
            .size()
            .x
    };
    let widths: Arc<Vec<f32>> = Arc::new(
        w.labels
            .iter()
            .map(|l| width(&l.text))
            .chain(std::iter::once(width("…")))
            .collect(),
    );
    ui.data_mut(|d| d.insert_temp(id, (w.clone(), font.clone(), widths.clone())));
    widths
}

/// E7: "Also updated:" and the files' names, each a label with its full path and what it is on hover. A line that
/// does not fit is cut in the middle: names from both ends, then `…`, whose accessible name and hover hold all of them.
/// The texts come with the list (`ReviewWrites::labels`), and their widths are measured once per list.
fn also_updated_line(ui: &mut egui::Ui, w: &Arc<ReviewWrites>) {
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
    ui.strong("Also updated:");
    let list = &w.also_updated;
    let labels = &w.labels;
    let n = labels.len().min(list.len());
    let font = egui::TextStyle::Body.resolve(ui.style());
    let widths = also_widths(ui, w, &font);
    let gap = ui.spacing().item_spacing.x;
    let avail = ui.available_width();
    let marker = widths[widths.len() - 1] + gap;
    let (mut lo, mut hi, mut used) = (0, n, 0.0_f32);
    let mut from_head = true;
    while lo < hi {
        // The other end is tried when this one does not fit: a short name there may still.
        let fit = [from_head, !from_head].into_iter().find_map(|head| {
            let i = if head { lo } else { hi - 1 };
            let w_i = widths[i] + gap;
            let reserve = if hi - lo == 1 { 0.0 } else { marker };
            (used + w_i + reserve <= avail).then_some((head, w_i))
        });
        let Some((head, w_i)) = fit else {
            break;
        };
        used += w_i;
        if head {
            lo += 1;
        } else {
            hi -= 1;
        }
        from_head = !head;
    }
    let one = |ui: &mut egui::Ui, i: usize| {
        let l = &labels[i];
        let r = ui.label(l.text.as_str());
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &l.name));
        r.on_hover_ui(|ui| {
            ui.label(l.hover.as_str());
            ui.weak(l.shown.as_str());
        });
    };
    for i in 0..lo {
        one(ui, i);
    }
    if lo < hi {
        let r = ui.label("…");
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &w.whole));
        r.on_hover_ui(|ui| {
            for l in labels.iter().take(n) {
                ui.label(l.text.trim_end_matches(','));
                ui.weak(format!("{} {}", l.hover, l.shown));
            }
        });
    }
    for i in hi..n {
        one(ui, i);
    }
}

/// Note FB10: under this window width the Review sections stack in one scroll area.
const STACK_BELOW: f32 = 1200.0;

/// Each column scrolls independently. Its key stays the same as the heading's count changes.
fn column(ui: &mut egui::Ui, key: &str, body: impl FnOnce(&mut egui::Ui)) {
    // ui-rules 3.2: a bar drawn without hover, so a column that overflows says so.
    ui.spacing_mut().scroll = crate::look::thin_scroll();
    egui::ScrollArea::vertical()
        .id_salt(("review", key))
        .auto_shrink([false, true])
        .show(ui, body);
}

/// P2 / TA30: the CM lines Review shows: the engine's install text (the report prints the same; `grass`: a PBR grass
/// set is not skipped), plus "the Vanilla folder is written too" when only CM is ticked.
pub fn also_cm_lines(cfg: &ExportConfig, grass: bool) -> Vec<&'static str> {
    let mut v = engine::also::cm_lines(cfg, grass);
    if !cfg.also.contains(&AlsoTarget::Vanilla) {
        v.push(engine::also::CM_NEEDS_VANILLA);
    }
    v
}

/// One `[export.names]` field (PBR, Vanilla or CM), committed on focus loss (ui-rules 7.5); Esc cancels. Empty means
/// the default name, shown as the hint.
fn name_field(
    ui: &mut egui::Ui,
    root: Option<&std::path::Path>,
    which: usize,
    saved: Option<&str>,
    hint: &str,
    act: &mut Option<Act>,
) {
    // Keyed by the opened folder too: a draft never carries into another project (review M2).
    let id = egui::Id::unique(("also-name", which, root));
    let saved = saved.unwrap_or_default().to_string();
    let mut t: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| saved.clone());
    let r = ui.add(
        egui::TextEdit::singleline(&mut t)
            .margin(crate::look::TEXT_EDIT_MARGIN)
            .hint_text(hint)
            .desired_width(f32::INFINITY),
    );
    let name = format!("{} folder name", ["PBR", "Vanilla", "CM"][which]);
    let enabled = ui.is_enabled();
    r.widget_info(|| {
        let mut i = egui::WidgetInfo::text_edit(enabled, saved.clone(), t.clone(), hint);
        i.label = Some(name.clone());
        i
    });
    if r.changed() {
        ui.data_mut(|d| d.insert_temp(id, t.clone()));
    }
    if r.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
        if t != saved && !ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            let v = (!t.is_empty()).then_some(t);
            *act = Some(Act::AlsoSet(Box::new(move |c| match which {
                0 => c.names.pbr = v,
                1 => c.names.vanilla = v,
                _ => c.names.cm = v,
            })));
        }
    }
}

/// The Also settings under Options (Note CN, owner answers 5 and 6). The recipe link opens a set's Also export tab (JI2).
fn also_section(ui: &mut egui::Ui, m: &AppModel, act: &mut Option<Act>) {
    let cfg = m.also_config();
    let (v, c) = m.also_ticks();
    let busy = m.export.running.is_some().then_some(also::EXPORT_RUNNING);
    let off = busy
        .map(str::to_string)
        .or_else(|| m.toml_read_only_reason());
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
    ui.add_enabled_ui(off.is_none(), |ui| {
        ui.horizontal(|ui| {
            for (t, on, text) in [
                (AlsoTarget::Vanilla, v, "Vanilla"),
                (AlsoTarget::Cm, c, "Complex material"),
            ] {
                // A tick left from the cs mode stays clickable, so it can be cleared.
                let why = also::tick_off(m, on, busy);
                let mut now = on;
                let r = ui.add_enabled(why.is_none(), egui::Checkbox::new(&mut now, text));
                if r
                    .on_disabled_hover_text(why.as_deref().unwrap_or_default())
                    .changed()
                {
                    *act = Some(Act::AlsoTick(t, now));
                }
            }
        });
        ui.weak("Also folders go in");
        // Note FI5: with no export folder chosen and no own location, the fallback is a name, never a `sdc-out` path.
        let unchosen = m.export_folder_chosen().is_none()
            && m.sets_doc.as_ref().and_then(|d| d.also_location()).is_none();
        let loc = if unchosen {
            NOT_CHOSEN_YET.to_string()
        } else {
            m.also_location().display().to_string()
        };
        egui::Sides::new().shrink_left().truncate().show(
            ui,
            |ui| {
                let glyph = ui.fonts_mut(|f| {
                    f.glyph_width(&egui::TextStyle::Body.resolve(ui.style()), '0')
                });
                let chars = (ui.available_width() / glyph).floor().max(8.0) as usize;
                let r = ui.add(egui::Label::new(super::header::elide_middle(&loc, chars)));
                let name = format!("Also folders go in: {loc}");
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
                r.on_hover_text(&loc);
            },
            |ui| {
                let r = ui.add_enabled(
                    !m.picker.busy(),
                    egui::Button::new("Choose where the Also folders go…"),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        r.enabled(),
                        "Choose where the Also folders go…",
                    )
                });
                let r = r
                    .on_hover_text("Where the PBR, Vanilla and CM folders are written")
                    .on_disabled_hover_text(off.as_deref().unwrap_or(picker::BUSY_HOVER));
                if r.clicked() {
                    *act = Some(Act::AlsoLocation);
                }
            },
        );
        let folders = m.also_folders();
        let hint = |i: usize| {
            folders[i]
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        egui::Grid::new("also-names").num_columns(2).show(ui, |ui| {
            let names = [&cfg.names.pbr, &cfg.names.vanilla, &cfg.names.cm];
            for (i, label) in ["PBR", "Vanilla", "CM"].into_iter().enumerate() {
                ui.label(label);
                name_field(ui, m.root.as_deref(), i, names[i].as_deref(), &hint(i), act);
                ui.end_row();
            }
        });
        if c {
            let mut keep = cfg.legacy.keep_metal_brightness;
            if ui
                .checkbox(&mut keep, "Keep metal brightness")
                .on_hover_text("The CM diffuse keeps the metals' brightness instead of reusing the Vanilla diffuse")
                .changed()
            {
                *act = Some(Act::AlsoSet(Box::new(move |c| {
                    c.legacy.keep_metal_brightness = keep
                })));
            }
            ui.add_enabled_ui(keep, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Strength");
                    let now = cfg.legacy.keep_metal_strength;
                    // One edit (one undo step) per drag, not per frame.
                    let (r, done) =
                        super::number_input::held_value(ui, "also-keep-strength", now, |ui, s| {
                            let r = ui.add(egui::Slider::new(s, 0.0..=1.0));
                            let v = *s;
                            r.widget_info(|| {
                                egui::WidgetInfo::slider(
                                    r.enabled(),
                                    v,
                                    "Keep metal brightness strength",
                                )
                            });
                            r
                        });
                    r.on_hover_text("How much of the metal darkening it undoes");
                    if let Some(s) = done {
                        *act = Some(Act::AlsoSet(Box::new(move |c| {
                            c.legacy.keep_metal_strength = s
                        })));
                    }
                });
            });
            for l in also_cm_lines(&cfg, m.also_grass()) {
                // The engine's text is Markdown; no backticks on screen.
                ui.add(egui::Label::new(egui::RichText::new(l.replace('`', "")).weak()).wrap());
            }
        }
        skip_list(ui, m, &cfg, act);
    })
    .response
    .on_disabled_hover_text(off.as_deref().unwrap_or_default());
    // Note JI2 (correction 6): the recipe is on a PBR set's Also export tab, so with no PBR set there is no link.
    if m.recipe_set().is_some()
        && ui
            .link("Recipe settings…")
            .on_hover_text(
                "Brightness, AO, specular and the other [export.legacy] values, in a set's Also export tab",
            )
            .clicked()
    {
        *act = Some(Act::Recipe);
    }
}

/// "Skip these sets" (C2, owner answer 6): one row per PBR set, checked = left out of Vanilla and CM. Reachable from
/// the 3D view too, where the set panel is not. At most 8 rows tall; only the rows on screen are drawn.
fn skip_list(ui: &mut egui::Ui, m: &AppModel, cfg: &ExportConfig, act: &mut Option<Act>) {
    ui.label("Skip these sets")
        .on_hover_text("Checked sets are left out of the Vanilla and CM folders");
    let views = sets::views(m);
    let pbr: Vec<_> = views.iter().filter(|v| also::is_pbr_set(v)).collect();
    if pbr.is_empty() {
        ui.weak("No PBR sets");
        return;
    }
    let rows = |ui: &mut egui::Ui, range: std::ops::Range<usize>, act: &mut Option<Act>| {
        for v in &pbr[range] {
            let on = cfg.skip.iter().any(|s| engine::sets::same_id(s, &v.id));
            let mut now = on;
            let r = ui.checkbox(&mut now, super::dialogs::short_name(&v.id));
            r.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::CheckBox, r.enabled(), now, &v.id)
            });
            if r.on_hover_text(&v.id).changed() {
                *act = Some(Act::Skip(v.id.clone(), now));
            }
        }
    };
    // Up to 8 rows are drawn inline, so the column's own scroll (and scroll-to-focus) reaches them: a nested area
    // takes every scroll target set inside it (ui-rules gotcha, CC18).
    if pbr.len() <= 8 {
        rows(ui, 0..pbr.len(), act);
        return;
    }
    let row_h = ui.spacing().interact_size.y;
    ui.scope(|ui| {
        ui.spacing_mut().scroll = crate::look::thin_scroll();
        egui::ScrollArea::vertical()
            .id_salt("also-skip")
            .max_height(8.0 * (row_h + ui.spacing().item_spacing.y))
            .auto_shrink([false, true])
            .show_rows(ui, row_h, pbr.len(), |ui, range| rows(ui, range, act));
    });
}

pub fn tab(ui: &mut egui::Ui, model: &mut AppModel) {
    let Some(review) = &model.review else {
        return;
    };
    let m = &*model;
    // R-b: the blocks are cached per change, so the gate below reads them again for free.
    let blocks = pbr_export::export_blocks(m);
    let undecided = m.undecided_keys();
    let off = m.fix_off();
    let any_suggestion = undecided
        .iter()
        .any(|k| matches!(m.subtype_question(k), Some(Some(_))));
    // Note FI11 (A11): Accept all takes type suggestions only; an alpha meaning needs the user's answer.
    let any_alpha = undecided.iter().any(|k| {
        m.item(k).is_some_and(|i| {
            i.decisions
                .iter()
                .any(|d| matches!(d, Decision::AlphaMeaning))
        })
    });
    let gate = m.review_gate();
    // Note FP8 (T30): the gate also refuses for a missing preset and a read-only `sdc.toml`, so they are blockers.
    let preset_row = m.missing_preset_text();
    let toml_row = m.toml_read_only_reason();
    let nif_folder = m.export_folder_chosen();
    let folder_chosen = nif_folder.is_some();
    let json = m.review_json();
    let writes = m.review_writes();
    let nif_rows = m.nif_review_rows();
    // Note CN85: Also settings are available in the cs mode or while a tick is on.
    // Note HF10: none for a scope, which Also never runs on.
    let also_on = (m.also_on() || m.target == Target::Cs) && m.export_scope().is_none();
    let also_err = if m.also_runs() {
        m.also_gate().err()
    } else {
        None
    };
    let nif_skipped: Vec<_> = nif_rows
        .iter()
        .filter(|r| r.blocked.as_deref().is_some_and(|b| b != NO_EXPORT_FOLDER))
        .collect();
    let mut act = None;
    let also_blocked = also_err.is_some();
    let mut options_open = review.options_open || (also_blocked && !review.also_blocked);
    // Note FC14: the footer's lists share one scroll area of at most a third of the tab, so the columns keep room.
    let footer_cap = ui.available_height() / 3.0;
    // The footer first, pinned to the bottom, so the columns scroll above it and it is never clipped.
    super::panel_ids::bottom(ui, "review-footer")
        .resizable(false)
        .show(ui, |ui| {
            // Note FC5: the NIF edits sit above the files rows; a blocked one is listed under its own heading.
            let nif_written: Vec<_> = nif_rows.iter().filter(|r| r.blocked.is_none()).collect();
            let mut list = vec![];
            if !nif_written.is_empty() {
                list.push(Footer::Heading(format!(
                    "NIF edits ({})",
                    nif_written.len()
                )));
                list.extend(nif_written.iter().map(|r| Footer::Nif(r)));
            }
            // Note FI7 (O5): the Blockers column already names the missing folder, so these are listed apart.
            let nif_waiting: Vec<_> = nif_rows
                .iter()
                .filter(|r| r.blocked.as_deref() == Some(NO_EXPORT_FOLDER))
                .collect();
            if !nif_waiting.is_empty() {
                list.push(Footer::Heading(format!(
                    "NIF edits skipped until an export folder is chosen ({})",
                    nif_waiting.len()
                )));
                list.extend(nif_waiting.iter().map(|r| Footer::NifWaiting(r)));
            }
            // A blocked NIF edit is skipped, never a gate: its reason is listed here.
            if !nif_skipped.is_empty() {
                list.push(Footer::Heading(format!(
                    "NIF edits skipped ({})",
                    nif_skipped.len()
                )));
                list.extend(nif_skipped.iter().map(|r| Footer::NifSkipped(r)));
            }
            list.push(Footer::Written(m.written_text()));
            // Note FC5: the Keep/Overwrite question moved to Blockers.
            list.extend(json.iter().map(Footer::Json));
            // E7: after the files of What will be written, the ones it saves and records besides.
            if !writes.also_updated.is_empty() {
                list.push(Footer::Also(&writes));
            }
            let paths = [writes.outputs.len(), writes.current.len(), writes.kept.len(), writes.protected.len()]
                .into_iter().filter(|&n| n > 0).map(|n| n + 1).sum::<usize>();
            let row_h = ui.spacing().interact_size.y;
            let stride = row_h + ui.spacing().item_spacing.y;
            let rows = list.len() + paths;
            let height = footer_cap.max(3.0 * stride).min(rows as f32 * stride);
            ui.scope(|ui| {
                ui.spacing_mut().scroll = crate::look::thin_scroll();
                egui::ScrollArea::vertical()
                    .id_salt("review-footer-lists")
                    .min_scrolled_height(height)
                    .max_height(height)
                    .auto_shrink([false, true])
                    // Rule 9.2: only the rows on screen are laid out, each exactly `row_h` tall.
                    .show_rows(ui, row_h, rows, |ui, range| {
                        for index in range {
                            let size = egui::vec2(ui.available_width(), row_h);
                            let layout = egui::Layout::left_to_right(egui::Align::Center);
                            let extra;
                            let row = match list.get(index) {
                                Some(row) => row,
                                None => {
                                    extra = output_row(&writes, index - list.len());
                                    &extra
                                }
                            };
                            ui.allocate_ui_with_layout(size, layout, |ui| match row {
                                Footer::Heading(t) => {
                                    ui.strong(t);
                                }
                                Footer::Nif(r) => {
                                    let t = nif_row_text(r, nif_folder.as_deref());
                                    ui.add(egui::Label::new(&t).truncate())
                                        .on_hover_text(format!("{}: {t}", r.path));
                                }
                                Footer::NifWaiting(r) => {
                                    ui.add(
                                        egui::Label::new(egui::RichText::new(&r.path).weak())
                                            .truncate(),
                                    )
                                    .on_hover_text(&r.path);
                                }
                                Footer::NifSkipped(r) => {
                                    let why = r.blocked.as_deref().unwrap_or_default();
                                    let t = format!("{}: {why}", r.path);
                                    ui.add(
                                        egui::Label::new(egui::RichText::new(&t).color(AMBER))
                                            .truncate(),
                                    )
                                    .on_hover_text(&t);
                                }
                                Footer::Written(text) => {
                                    ui.strong("What will be written");
                                    // Note JF1: with an Also tick the line is long; cut, it keeps its text on hover.
                                    ui.add(egui::Label::new(text).truncate());
                                }
                                Footer::File(path, marked) => {
                                    let shown = (!folder_chosen).then(|| m.export_root()).and_then(|root| path.strip_prefix(root).ok())
                                        .map_or_else(|| path.display().to_string(), |p| format!("{NOT_CHOSEN_ROOT}\\{}", p.display()));
                                    let text = if *marked { format!("{shown} (depends on this export)") } else { shown };
                                    ui.add(egui::Label::new(text).truncate());
                                }
                                Footer::Json(l) => {
                                    let fallback = (!folder_chosen).then(|| m.export_root());
                                    ui.add(egui::Label::new(json_line_text(l, fallback.as_deref())).truncate());
                                }
                                Footer::Also(w) => {
                                    also_updated_line(ui, w);
                                }
                            });
                        }
                    });
            });
            if m.reencode_all {
                ui.label("Re-encode every output is on: every file Tessalith recorded is written again.");
            }
            ui.horizontal(|ui| {
                // Ruling 3c-43: with no project (a meshes-only Review) there is no key to set, so no box.
                if m.project.is_some() {
                    if let Some(on) = super::settings_tab::opt_out_checkbox(
                        ui,
                        m,
                        OPT_OUT_TEXT,
                        "For this project only. The header's Export then writes at once while nothing needs a look. Turn it off in Settings.",
                    ) {
                        act = Some(Act::OptOut(on));
                    }
                }
                if ui.button("Close").clicked() {
                    act = Some(Act::Close);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let button = egui::Button::new(m.review_button_text());
                    match &gate {
                        Ok(_) => {
                            let r = ui.add(button.fill(ui.visuals().selection.bg_fill));
                            // Note FC8: a focused field commits on the press; the release exports.
                            super::blur_on_press(ui, r.rect);
                            if r.clicked() {
                                act = Some(Act::Export);
                            }
                        }
                        Err(why) => {
                            ui.add_enabled(false, button).on_disabled_hover_text(why);
                            // Note FP8: the Blockers column already lists these two reasons.
                            let listed = toml_row.as_deref() == Some(why.as_str())
                                || preset_row.as_ref().is_some_and(|(t, _)| t == why);
                            if !listed {
                                ui.colored_label(RED, why);
                            }
                        }
                    }
                    // Note FL: the rows may be an edit's partial plan; a click waits for the full plan (rule 7.2).
                    if m.full_plan_pending() {
                        ui.weak("Planning…")
                            .on_hover_text("Export waits for the plan, then checks again");
                    }
                });
            });
        });
    // Note FB4: the missing export folder counts, because the gate enforces it; Note FC5: so does an unanswered
    // overwrite question.
    let also_row = also_err
        .as_deref()
        .filter(|why| Some(*why) != toml_row.as_deref());
    let blockers = blocks.len()
        + usize::from(also_row.is_some())
        + usize::from(!folder_chosen)
        + usize::from(preset_row.is_some())
        + usize::from(toml_row.is_some())
        + m.unanswered_overwrites().len();
    let json_answer = review.json_answer.as_ref();
    let questions = json
        .iter()
        .any(|l| l.kind == JsonKind::PgPatcher && l.state == DiskState::Edited)
        || nif_rows
            .iter()
            .any(|r| r.blocked.is_none() && r.foreign.is_some());
    let n_cols = if also_on { 3 } else { 2 };
    let heading = |i: usize| match i {
        0 => ("Blockers", Some(format!("Blockers ({blockers})"))),
        1 => (
            "Undecided",
            Some(format!("Undecided ({})", undecided.len())),
        ),
        _ => ("Options", None),
    };
    // One body per column, drawn side by side or stacked (Note FB10).
    let mut body = |ui: &mut egui::Ui, i: usize| match i {
        0 => {
            if blockers == 0 && !questions {
                ui.weak("None");
            }
            // Note FB1: the export folder is a blocker until one is chosen for this project.
            if !folder_chosen {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(RED, NO_EXPORT_FOLDER);
                    // Rule 5.4: the visible text is the accessible name.
                    let r = ui.add_enabled(
                        !m.picker.busy(),
                        egui::Button::new("Choose the export folder…"),
                    );
                    let r = r
                        .on_hover_text("Choose where converted textures are written")
                        .on_disabled_hover_text(picker::BUSY_HOVER);
                    if r.clicked() {
                        act = Some(Act::ChooseExport);
                    }
                });
            }
            if let Some(why) = &toml_row {
                ui.colored_label(RED, why);
            }
            if let Some(why) = also_row {
                ui.add(egui::Label::new(egui::RichText::new(why).color(RED)).wrap());
            }
            if let Some((text, hover)) = &preset_row {
                ui.horizontal_wrapped(|ui| {
                    let l = ui.colored_label(RED, text);
                    if let Some(h) = hover {
                        l.on_hover_text(h);
                    }
                    // Rule 5.4: the visible text is the accessible name.
                    let r = ui.button("Choose preset…");
                    if r.on_hover_text("Open the Preset menu and pick an installed preset")
                        .clicked()
                    {
                        act = Some(Act::ChoosePreset);
                    }
                });
            }
            for b in blocks.iter() {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(RED, &b.text);
                    // A project-wide issue has no set to go to.
                    if !b.set_key.is_empty() && ui.button("Go to set").clicked() {
                        act = Some(Act::GoTo(b.set_key.clone()));
                    }
                });
            }
            // Note FC5 (C4): Export waits for an answer to each overwrite question; none is picked until clicked.
            for l in json
                .iter()
                .filter(|l| l.kind == JsonKind::PgPatcher && l.state == DiskState::Edited)
            {
                let answer = json_answer.filter(|a| a.answers(l)).map(|a| a.overwrite);
                let name = l.path.file_name().unwrap_or_default().to_string_lossy();
                let text = format!("{name} was edited outside Tessalith");
                if let Some(o) = overwrite_row(ui, &text, answer) {
                    act = Some(Act::Json(o));
                }
            }
            for r in nif_rows.iter().filter(|r| r.blocked.is_none()) {
                let Some(sha) = r.foreign else { continue };
                let answer = r
                    .answer
                    .map(|c| c == crate::model::meshes::export::GuardChoice::Overwrite);
                let text = format!("{} was not written by Tessalith", r.path);
                ui.horizontal(|ui| {
                    if let Some(o) = overwrite_row(ui, &text, answer) {
                        act = Some(Act::Nif(r.path.clone(), o, sha));
                    }
                    // Note GF2: the edit panel's Diff view of this file against what the export writes.
                    let diff = ui.button("Diff…");
                    // ui-rules 8.4: the edit panel's footer has a Diff… too; this one names its NIF.
                    let name = format!("Diff… {}", r.path);
                    diff.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
                    if diff
                        .on_hover_text(format!("Show the changed blocks of {}", r.path))
                        .clicked()
                    {
                        act = Some(Act::NifDiff(r.path.clone()));
                    }
                });
            }
        }
        1 => {
            if undecided.is_empty() {
                ui.weak("None");
            }
            for k in &undecided {
                if let Some(p) = undecided_row(ui, m, k, off.as_deref()) {
                    act = Some(Act::Row(p));
                }
            }
            let accept = ui
                .add_enabled(
                    any_suggestion && off.is_none(),
                    egui::Button::new("Accept all suggestions"),
                )
                .on_hover_text("Decide every texture that has a suggestion; the others stay listed")
                .on_disabled_hover_text(off.as_deref().unwrap_or(if any_alpha {
                    "Only type suggestions are accepted here; an alpha meaning needs your answer"
                } else {
                    "No undecided texture has a suggestion"
                }));
            if accept.clicked() {
                act = Some(Act::AcceptAll);
            }
        }
        _ => {
            let mut text = String::from("Options");
            if !options_open {
                let ((vanilla, cm), n) = m.also_summary();
                if cm {
                    text.push_str(" · Also export as Vanilla and CM");
                } else if vanilla {
                    text.push_str(" · Also export as Vanilla");
                }
                if (vanilla || cm) && n > 0 {
                    text.push_str(&format!(" · skipping {n} set{}", plural(n)));
                }
            }
            let response = egui::CollapsingHeader::new(text)
                .id_salt("review-options")
                .open(Some(options_open))
                .show_unindented(ui, |ui| {
                    ui.strong("Also export as");
                    also_section(ui, m, &mut act);
                });
            if response.header_response.clicked() {
                options_open = !options_open;
            }
        }
    };
    // Note HF9: a scoped review says what it exports above the columns.
    if let Some(s) = m.export_scope() {
        ui.add(egui::Label::new(egui::RichText::new(&s.label).strong()).truncate());
    }
    if ui.ctx().content_rect().width() < STACK_BELOW {
        // Note FB10: one scrolling column, the sections one under another.
        ui.spacing_mut().scroll = crate::look::thin_scroll();
        egui::ScrollArea::vertical()
            .id_salt(("review", "stacked"))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for i in 0..n_cols {
                    if let Some(text) = heading(i).1 {
                        ui.strong(text);
                    }
                    body(ui, i);
                    ui.add_space(6.0);
                }
            });
    } else {
        // `columns` sizes to max_rect, which still reaches under the footer; bound it to the space left above it.
        ui.allocate_ui(ui.available_size(), |ui| {
            ui.columns(n_cols, |cols| {
                for (i, c) in cols.iter_mut().enumerate() {
                    let (key, text) = heading(i);
                    if let Some(text) = text {
                        c.strong(text);
                    }
                    column(c, key, |ui| body(ui, i));
                }
            })
        });
    }
    if let Some(review) = &mut model.review {
        review.options_open = options_open;
        review.also_blocked = also_blocked;
    }
    match act {
        Some(Act::AlsoTick(t, on)) => model.set_also_tick(t, on),
        Some(Act::AlsoSet(f)) => model.set_also_settings(f),
        Some(Act::AlsoLocation) => {
            model.pick(ui.ctx(), PickerPurpose::AlsoLocation, PickerSpec::folder());
        }
        Some(Act::ChooseExport) => {
            model.pick_export_folder(ui.ctx());
        }
        // The header's Preset menu opens on the request (as Problems' `Choose preset…` does).
        Some(Act::ChoosePreset) => model.choose_preset = Some(ui.ctx().cumulative_pass_nr()),
        Some(Act::Skip(id, on)) => model.set_skip_also(&id, on),
        // Note JI2: the set's Also export tab, in 2D (the set editor is not drawn in 3D or over a preview).
        Some(Act::Recipe) => super::navigate(ui.ctx(), model, |ctx, model| {
            if let Some(target) = model.recipe_set() {
                model.collapse();
                model.select_target(target);
                super::set_editor::show_also_tab(ctx);
                if model.settings.set_view_3d {
                    model.settings.set_view_3d = false;
                    model.save_settings();
                }
                super::header::set_meshes_view(ctx, false);
            }
        }),
        Some(Act::GoTo(k)) => super::navigate(ui.ctx(), model, move |ctx, m| {
            m.select_set(&k);
            super::header::set_meshes_view(ctx, false);
        }),
        Some(Act::Row(p @ RowPick::Select(_))) => {
            super::navigate(ui.ctx(), model, move |_, m| apply(m, p))
        }
        Some(Act::Row(p)) => apply(model, p),
        Some(Act::AcceptAll) => model.accept_all_suggestions(),
        Some(Act::Json(overwrite)) => model.answer_review_json(overwrite),
        Some(Act::Nif(path, overwrite, sha)) => model.answer_review_nif(&path, overwrite, sha),
        Some(Act::NifDiff(path)) => super::navigate(ui.ctx(), model, move |ctx, m| {
            m.nif_diff_request = Some(path);
            super::header::set_meshes_view(ctx, true);
        }),
        // Export saves first: a focused field commits before it runs (Note FC8, supersedes CA14's refusal). Ruling
        // 3c-38: the click carries what the button saw when it fired, in case it waits a frame.
        Some(Act::Export) => {
            let press = model.export_press();
            super::after_commit(ui.ctx(), model, move |m| m.review_export_pressed(press));
        }
        Some(Act::Close) => model.close_review(),
        Some(Act::OptOut(on)) => model.set_export_without_review(on),
        None => {}
    }
}
