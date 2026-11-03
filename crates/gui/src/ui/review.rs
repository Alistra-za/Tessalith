//! The Review tab (layout v2 B2, C2, C5; R1–R4, R11): blockers and undecided textures side by side, what the export
//! writes, and one Export button. Not a dialog: Enter does nothing here (N3).
use super::settings_panel::{alpha_dropdown, offered_groups, subtype_label, unclear_layouts};
use super::status::{AMBER, RED};
use super::type_picker::{self, TypeKind};
use crate::model::code_text::{ack_label, ack_label_for_texture, alpha_confirm_text, code_line};
use crate::model::items::{alpha_asked, Decision};
use crate::model::picker::{self, PickerPurpose, PickerSpec};
use crate::model::review::{DiskState, JsonKind, JsonLine, NO_EXPORT_FOLDER};
use crate::model::{also, pbr_export, sets, Answer, AppModel};
use eframe::egui;
use engine::config::{CubeLayout, Subtype, Target};
use engine::presets::{AlsoTarget, ExportConfig};
use std::path::Path;

/// Note FI5: what the Also location reads while no export folder is chosen (the fallback is never shown as a path).
const NOT_CHOSEN_YET: &str = "the export folder (not chosen yet)";
/// Note FI5: what a JSON line's path starts with while no export folder is chosen.
const NOT_CHOSEN_ROOT: &str = "‹export folder›";

/// What a click on an undecided row asks for.
pub enum RowPick {
    Select(String),
    Decide(String, Subtype),
    Answer(String, Answer),
    /// Note FC3 (DT35): the cube layout picked from the row's `Set layout ▾`.
    Layout(String, CubeLayout),
}

pub fn apply(model: &mut AppModel, pick: RowPick) {
    match pick {
        RowPick::Select(k) => model.select_item(&k),
        RowPick::Decide(k, st) => model.decide_undecided_row(&k, st),
        RowPick::Answer(k, a) => model.answer_undecided_row(&k, a),
        RowPick::Layout(k, l) => model.set_field(&k, "cubemap.layout", serde_json::json!(l)),
    }
}

/// Note FC3 (supersedes DT35's hint): the cube layout question answered on the row, `Set layout ▾`, with the layouts
/// the settings panel's picker offers for an unclear image.
pub const SET_LAYOUT: &str = "Set layout ▾";

fn layout_menu(ui: &mut egui::Ui, key: &str, why: &str) -> Option<RowPick> {
    let mut pick = None;
    ui.menu_button(SET_LAYOUT, |ui| {
        for (l, label) in unclear_layouts() {
            if ui.button(*label).clicked() {
                pick = Some(RowPick::Layout(key.to_string(), *l));
                ui.close();
            }
        }
    })
    .response
    .on_disabled_hover_text(why);
    pick
}

/// One undecided texture's controls: name link, sub-type combo + ✓ when it asks its sub-type, the alpha dropdown,
/// "Acknowledge for this texture". The controls are disabled with `off` as hover; the name link always selects.
pub fn undecided_row(
    ui: &mut egui::Ui,
    model: &AppModel,
    key: &str,
    off: Option<&str>,
) -> Option<RowPick> {
    let item = model.item(key)?;
    let fallback = item.job.as_ref().map(|j| j.profile.subtype);
    let question = model.subtype_question(key);
    let why = off.unwrap_or_default();
    let mut pick = None;
    ui.horizontal(|ui| {
        if ui.link(&item.name).clicked() {
            pick = Some(RowPick::Select(key.to_string()));
        }
        let Some(suggested) = question else {
            return;
        };
        let Some(default) = suggested.or(fallback) else {
            return;
        };
        ui.add_enabled_ui(off.is_none(), |ui| {
            let id = egui::Id::new(("undecided-row", key));
            let mut chosen: Subtype = ui.data(|d| d.get_temp(id)).unwrap_or(default);
            egui::ComboBox::from_id_salt(id)
                .selected_text(subtype_label(chosen))
                .show_ui(ui, |ui| {
                    let grass = item.job.as_ref().is_some_and(engine::policy::grass_context);
                    for (heading, members) in offered_groups(model.target, grass) {
                        ui.label(egui::RichText::new(heading).weak());
                        for st in members {
                            ui.selectable_value(&mut chosen, st, subtype_label(st))
                                .on_hover_text(st.id());
                        }
                    }
                })
                .response
                .on_disabled_hover_text(why);
            ui.data_mut(|d| d.insert_temp(id, chosen));
            if ui
                .button("\u{2713}")
                .on_hover_text("Decide this texture")
                .on_disabled_hover_text(why)
                .clicked()
            {
                pick = Some(RowPick::Decide(key.to_string(), chosen));
            }
        });
    });
    // The questions that are not a sub-type: a small fixed set of choices each, answered inline.
    ui.add_enabled_ui(off.is_none(), |ui| {
        for d in &item.decisions {
            ui.horizontal_wrapped(|ui| match d {
                // Note FM35: one line and one dropdown on the resolved type.
                Decision::AlphaMeaning => {
                    if let Some((shown, mesh)) = alpha_asked(item) {
                        ui.label(alpha_confirm_text(shown, mesh));
                        let id = egui::Id::new(("undecided-alpha", key));
                        if let Some(p) = alpha_dropdown(ui, id, shown, why) {
                            pick = Some(RowPick::Answer(key.to_string(), Answer::Alpha(p)));
                        }
                    }
                }
                Decision::Acknowledge(code) => {
                    // Note FB8: the sentence, with the code on its hover.
                    ui.label(code_line(*code)).on_hover_text(code.as_str());
                    if ui
                        .button(ack_label_for_texture(*code))
                        .on_hover_text(code_line(*code))
                        .on_disabled_hover_text(why)
                        .clicked()
                    {
                        pick = Some(RowPick::Answer(key.to_string(), Answer::Acknowledge(*code)));
                    }
                }
                Decision::CubeLayout => {
                    if let Some(p) = layout_menu(ui, key, why) {
                        pick = Some(p);
                    }
                }
                Decision::Subtype { .. } => {}
            });
        }
    });
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
/// accept with ▾ for the type picker, or a plain Set type… (Note FB9), the alpha dropdown (Note FM35), Acknowledge. All
/// disabled with `off` as hover.
pub fn fix_buttons(
    ui: &mut egui::Ui,
    model: &AppModel,
    key: &str,
    off: Option<&str>,
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
                Decision::Subtype { suggested } => {
                    let opener = match suggested {
                        Some(st) => {
                            let (accept, arrow) = split_button(ui, &accept_text(*st));
                            if accept
                                .on_hover_text("Use the suggested type")
                                .on_disabled_hover_text(why)
                                .clicked()
                            {
                                pick = Some(RowPick::Decide(key.to_string(), *st));
                            }
                            // ▾ is what the plain button was: the picker, so it keeps its name.
                            arrow.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    arrow.enabled(),
                                    "Set type…",
                                )
                            });
                            arrow.on_hover_text("Pick another type")
                        }
                        None => ui.button("Set type…"),
                    }
                    .on_disabled_hover_text(why);
                    let id = egui::Id::new(("set-type-picker", key));
                    let kind = TypeKind::SetType;
                    let grass = item.job.as_ref().is_some_and(engine::policy::grass_context);
                    let (t, s) = (model.target, *suggested);
                    if let Some(st) = type_picker::show(ui, id, &opener, None, s, kind, t, grass) {
                        pick = Some(RowPick::Decide(key.to_string(), st));
                    }
                }
                // Note FM35 (supersedes FI11's Decide… menu): the row's line names the type, and this dropdown opens
                // on it; Enter on the row confirms it.
                Decision::AlphaMeaning => {
                    if let Some((shown, _)) = alpha_asked(item) {
                        let id = egui::Id::new(("problems-alpha", key));
                        if let Some(p) = alpha_dropdown(ui, id, shown, why) {
                            pick = Some(RowPick::Answer(key.to_string(), Answer::Alpha(p)));
                        }
                    }
                }
                Decision::CubeLayout => {
                    if let Some(p) = layout_menu(ui, key, why) {
                        pick = Some(p);
                    }
                }
                Decision::Acknowledge(code) => {
                    if ui
                        .button(ack_label(*code))
                        .on_hover_text(code_line(*code))
                        .on_disabled_hover_text(why)
                        .clicked()
                    {
                        pick = Some(RowPick::Answer(key.to_string(), Answer::Acknowledge(*code)));
                    }
                }
            }
        }
    });
    pick
}

enum Act {
    AlsoTick(AlsoTarget, bool),
    AlsoSet(Box<dyn FnOnce(&mut ExportConfig)>),
    AlsoLocation,
    ChooseExport,
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
}

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
    Written(usize),
    Json(&'a JsonLine),
}

/// Note FB10: under this window width the Review sections stack in one scroll area.
const STACK_BELOW: f32 = 1200.0;

/// A column: its heading, then `body` in a scroll area of its own.
/// `key` names the scroll area, so a heading that carries a count keeps the column's scroll position.
fn column(ui: &mut egui::Ui, key: &str, heading: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.strong(heading);
    // ui-rules 3.2: a bar drawn without hover, so a column that overflows says so.
    ui.spacing_mut().scroll = egui::style::ScrollStyle::thin();
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
    let id = egui::Id::new(("also-name", which, root));
    let saved = saved.unwrap_or_default().to_string();
    let mut t: String = ui.data(|d| d.get_temp(id)).unwrap_or_else(|| saved.clone());
    let r = ui.add(
        egui::TextEdit::singleline(&mut t)
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

/// Review's "Also export as" column (Note CN, owner answers 5 and 6): the ticks, the location, the folder names, Keep
/// metal brightness and the CM lines, the sets to skip, and the gate's Also reason (`gate`). It names what
/// is written and why it cannot be yet, never that it will succeed (Task 11 M5). The recipe is in Advanced.
fn also_section(ui: &mut egui::Ui, m: &AppModel, gate: Option<&str>, act: &mut Option<Act>) {
    let cfg = m.also_config();
    let (v, c) = m.also_ticks();
    let off = m
        .export
        .running
        .is_some()
        .then(|| also::EXPORT_RUNNING.to_string())
        .or_else(|| m.toml_read_only_reason());
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
    ui.add_enabled_ui(off.is_none(), |ui| {
        ui.horizontal(|ui| {
            for (t, on, text) in [
                (AlsoTarget::Vanilla, v, "Vanilla"),
                (AlsoTarget::Cm, c, "Complex material"),
            ] {
                // A tick left from the cs mode stays clickable, so it can be cleared.
                let cs = (!on && m.target != Target::Cs).then_some(also::NEEDS_CS);
                let mut now = on;
                let r = ui.add_enabled(cs.is_none(), egui::Checkbox::new(&mut now, text));
                if r
                    .on_disabled_hover_text(off.as_deref().or(cs).unwrap_or_default())
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
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
                r.on_hover_text(&loc);
            },
            |ui| {
                let r = ui.add_enabled(
                    !m.picker.busy(),
                    egui::Button::new("Choose where the Also folders go…"),
                );
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
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
                        super::advanced::held_value(ui, "also-keep-strength", now, |ui, s| {
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
    if let Some(why) = gate {
        ui.add(egui::Label::new(egui::RichText::new(why).color(RED)).wrap());
    }
    if ui
        .link("Recipe settings are in Advanced")
        .on_hover_text("Brightness, AO, specular and the other [export.legacy] values")
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
                egui::WidgetInfo::selected(egui::WidgetType::Checkbox, r.enabled(), now, &v.id)
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
        ui.spacing_mut().scroll = egui::style::ScrollStyle::thin();
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
    let nif_folder = m.export_folder_chosen();
    let folder_chosen = nif_folder.is_some();
    let json = m.review_json();
    let nif_rows = m.nif_review_rows();
    // Note CN85: a fourth column in the cs mode or while a tick is on; the Also reason only with a tick, as the gate.
    let also_on = m.also_on() || m.target == Target::Cs;
    let also_err = if m.also_on() {
        m.also_gate().err()
    } else {
        None
    };
    let nif_skipped: Vec<_> = nif_rows
        .iter()
        .filter(|r| r.blocked.as_deref().is_some_and(|b| b != NO_EXPORT_FOLDER))
        .collect();
    let mut act = None;
    // Note FC14: the footer's lists share one scroll area of at most a third of the tab, so the columns keep room.
    let footer_cap = ui.available_height() / 3.0;
    // The footer first, pinned to the bottom, so the columns scroll above it and it is never clipped.
    egui::Panel::bottom("review-footer")
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
            list.push(Footer::Written(m.export_files()));
            // Note FC5: the Keep/Overwrite question moved to Blockers.
            list.extend(json.iter().map(Footer::Json));
            let row_h = ui.spacing().interact_size.y;
            let stride = row_h + ui.spacing().item_spacing.y;
            ui.scope(|ui| {
                ui.spacing_mut().scroll = egui::style::ScrollStyle::thin();
                egui::ScrollArea::vertical()
                    .id_salt("review-footer-lists")
                    .max_height(footer_cap.max(3.0 * stride))
                    .auto_shrink([false, true])
                    // Rule 9.2: only the rows on screen are laid out, each exactly `row_h` tall.
                    .show_rows(ui, row_h, list.len(), |ui, range| {
                        for row in &list[range] {
                            let size = egui::vec2(ui.available_width(), row_h);
                            let layout = egui::Layout::left_to_right(egui::Align::Center);
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
                                Footer::Written(files) => {
                                    ui.strong("What will be written");
                                    ui.label(format!("{files} DDS file{}", plural(*files)));
                                }
                                Footer::Json(l) => {
                                    let fallback = (!folder_chosen).then(|| m.export_root());
                                    ui.label(json_line_text(l, fallback.as_deref()));
                                }
                            });
                        }
                    });
            });
            ui.horizontal(|ui| {
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
                            ui.colored_label(RED, why);
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
    let blockers = blocks.len() + usize::from(!folder_chosen) + m.unanswered_overwrites().len();
    let json_answer = review.json_answer.as_ref();
    let questions = json
        .iter()
        .any(|l| l.kind == JsonKind::PgPatcher && l.state == DiskState::Edited)
        || nif_rows
            .iter()
            .any(|r| r.blocked.is_none() && r.foreign.is_some());
    let n_cols = if also_on { 3 } else { 2 };
    let heading = |i: usize| match i {
        0 => ("Blockers", format!("Blockers ({blockers})")),
        1 => ("Undecided", format!("Undecided ({})", undecided.len())),
        _ => ("Also export as", "Also export as".to_string()),
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
            for b in &blocks {
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
                    diff.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                    });
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
        _ => also_section(ui, m, also_err.as_deref(), &mut act),
    };
    if ui.ctx().content_rect().width() < STACK_BELOW {
        // Note FB10: one scrolling column, the sections one under another.
        ui.spacing_mut().scroll = egui::style::ScrollStyle::thin();
        egui::ScrollArea::vertical()
            .id_salt(("review", "stacked"))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for i in 0..n_cols {
                    ui.strong(heading(i).1);
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
                    column(c, key, &text, |ui| body(ui, i));
                }
            })
        });
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
        Some(Act::Skip(id, on)) => model.set_skip_also(&id, on),
        Some(Act::Recipe) => {
            super::advanced::open_recipe(ui.ctx());
            model.settings.advanced_open = true;
            model.save_settings();
        }
        Some(Act::GoTo(k)) => {
            model.select_set(&k);
            super::header::set_meshes_view(ui.ctx(), false);
        }
        Some(Act::Row(p)) => apply(model, p),
        Some(Act::AcceptAll) => model.accept_all_suggestions(),
        Some(Act::Json(overwrite)) => model.answer_review_json(overwrite),
        Some(Act::Nif(path, overwrite, sha)) => model.answer_review_nif(&path, overwrite, sha),
        Some(Act::NifDiff(path)) => {
            model.nif_diff_request = Some(path);
            super::header::set_meshes_view(ui.ctx(), true);
        }
        // Export saves first: a focused field commits before it runs (Note FC8, supersedes CA14's refusal).
        Some(Act::Export) => super::after_commit(ui.ctx(), model, AppModel::export_from_review),
        Some(Act::Close) => model.close_review(),
        None => {}
    }
}
