//! The Problems tab (layout v2 A1, A12): one row per problem with its fix in place; the list is rebuilt once per
//! change and only the rows on screen are drawn. Note FC3: every result the user must act on is a row, and its action
//! is the row's button (`Problem::fix`).
use super::review::{apply, fix_buttons, RowPick};
use super::status::{dot, Status};
use crate::model::bottom::{
    can_hide, filtered, flagged, kinds, problems, At, Fix, Level, Problem, ProblemsFilter,
};
use crate::model::code_text;
use crate::model::items::{alpha_asked, Decision};
use crate::model::meshes::NifTab;
use crate::model::sets::Missing;
use crate::model::shell::ShellRequest;
use crate::model::{Answer, AppModel};
use crate::pgp::settings::feature_key;
use eframe::egui;
use engine::codes::Code;
use serde_json::json;
use std::sync::Arc;

/// The fix button on a CHAN-NORMAL-LENGTH row (Note DP).
pub const REBUILD_Z: &str = "Rebuild Z";
/// The Undo of an Info row (R-fills-2, Z11); not the header's `Undo`, so the two have distinct names (rule 8.4).
pub const UNDO_ROW: &str = "Undo this change";
/// The `Checks` filter pill (Note FC3: Problems shows one count; checks are a filter of it, rule 6.5).
pub const CHECKS_PILL: &str = "Checks";
/// Note FC3: `Fix path` asks the PGPatcher field `(set id, key)` for focus; the field takes it when drawn.
pub const FOCUS_FIELD: &str = "fix-path-focus";

pub fn status_of(l: Level) -> Status {
    match l {
        Level::Blocks => Status::Blocks,
        Level::Decide | Level::Warning => Status::Warning,
        // Note FC3 (amends CL18): checks are not amber; Info rows are not a status either.
        Level::Check => Status::Check,
        Level::Info => Status::Info,
    }
}

/// A row's focus id (Note FB9): by what the row is, not where it is, so focus stays on a row while the rows above it
/// leave the list.
pub fn row_id(p: &Problem) -> egui::Id {
    egui::Id::new(("problem-row", p.level, &p.at, &p.text))
}

/// A row's accessible name: where, then what (Note FB9).
fn row_name(p: &Problem) -> String {
    let at = match &p.at {
        At::Texture(k) | At::File(k) | At::Nif(k) => k.as_str(),
        At::Set { id, .. } => id.as_str(),
        At::Project => "Project",
    };
    format!("{at}: {}", p.shown())
}

type Rows = Arc<Vec<Problem>>;

/// What the tab draws from one problems build.
#[derive(Clone)]
struct Listed {
    all: Rows,
    filter: Option<ProblemsFilter>,
    hidden_kinds: Vec<Code>,
    rows: Rows,
    checks: usize,
    suggestions: usize,
    /// Rows the hidden kinds take out of this filter's list (Note HC4).
    hidden: usize,
    /// Any row of the project can be hidden, so `Kinds ▾` shows.
    hideable: bool,
}

/// `filtered` at most once per change: keyed on the (cached) problems build, the filter and the hidden kinds. Also
/// whether the `Checks` pill shows (the set's check rows while a Set filter is on, else the project's; K6, review M2),
/// and the project's rows with a suggestion, which "Accept all suggestions" decides (Note FB9).
fn rows(ui: &egui::Ui, model: &AppModel) -> Listed {
    let all = problems(model);
    let f = model.bottom.filter.clone();
    let hidden_kinds = model.hidden_kinds();
    let id = egui::Id::new("problems-rows");
    if let Some(l) = ui.data(|d| d.get_temp::<Listed>(id)) {
        if Arc::ptr_eq(&l.all, &all) && l.filter == f && l.hidden_kinds == hidden_kinds {
            return l;
        }
    }
    let out: Rows = filtered(model, f.as_ref());
    let checks = match &f {
        Some(ProblemsFilter::Set(_)) => out.iter().filter(|p| p.level == Level::Check).count(),
        _ => flagged(model).check_rows,
    };
    let is_hidden = |p: &Problem| can_hide(p) && p.code.is_some_and(|c| hidden_kinds.contains(&c));
    let hidden = out.iter().filter(|p| is_hidden(p)).count();
    let rows: Rows = if hidden == 0 {
        out
    } else {
        Arc::new(out.iter().filter(|p| !is_hidden(p)).cloned().collect())
    };
    let l = Listed {
        suggestions: all.iter().filter(|p| p.suggested.is_some()).count(),
        hideable: all.iter().any(can_hide),
        all,
        filter: f,
        hidden_kinds: hidden_kinds.clone(),
        rows,
        checks,
        hidden,
    };
    ui.data_mut(|d| d.insert_temp(id, l.clone()));
    l
}

/// The `Kinds ▾` menu (Note HC4): one tick per kind of row the project has, unticked when hidden. The menu stays open,
/// so several kinds go in one visit.
fn kinds_menu(ui: &mut egui::Ui, model: &AppModel, hidden_rows: usize) -> Option<Act> {
    use egui::containers::menu::{MenuButton, MenuConfig};
    let label = match hidden_rows {
        0 => "Kinds ▾".to_string(),
        n => format!("Kinds ▾ ({n} hidden)"),
    };
    let config = MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    let mut act = None;
    let (button, _) = MenuButton::new(label).config(config).ui(ui, |ui| {
        let hidden = model.hidden_kinds();
        for (code, n) in kinds(model) {
            let mut shown = !hidden.contains(&code);
            // The sentence's first half names the kind; its second half is the fix for one row.
            let what = code_text::code_line(code)
                .split(" — ")
                .next()
                .unwrap_or_default();
            let what = match what {
                "" | "Tessalith reported a problem" => code.as_str(),
                w => w,
            };
            if ui
                .checkbox(&mut shown, format!("{what} ({n})"))
                .on_hover_text(code.as_str())
                .changed()
            {
                act = Some(Act::HideKind(code, !shown));
            }
        }
    });
    button.on_hover_text(
        "Hide the kinds of warning and info row you don't need. Blocks and questions always show, and Export's review \
         still lists every warning.",
    );
    act
}

/// The row that had keyboard focus and its index: when it leaves the list (its question was answered), focus goes to
/// the row now at that index, or the last one (Review Focus 5).
const ROW_FOCUS: &str = "problems-row-focus";

/// The Problems tab is not drawn: forget the focused row, so coming back never moves focus into the list.
pub fn forget_focus(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<(egui::Id, usize)>(egui::Id::new(ROW_FOCUS)));
}

enum Act {
    Texture(String),
    Set(String),
    File(String),
    Pick(RowPick),
    Apply(Fix, At),
    Filter(Option<ProblemsFilter>),
    AcceptAll,
    HideKind(Code, bool),
}

/// The pill's state and where a click takes the filter: on with a set keeps the set (review M2).
fn checks_pill(f: &Option<ProblemsFilter>) -> (bool, Option<ProblemsFilter>) {
    match f {
        None | Some(ProblemsFilter::Undecided) => (false, Some(ProblemsFilter::Checks)),
        Some(ProblemsFilter::Checks) => (true, None),
        Some(ProblemsFilter::Set(k)) => (false, Some(ProblemsFilter::SetChecks(k.clone()))),
        Some(ProblemsFilter::SetChecks(k)) => (true, Some(ProblemsFilter::Set(k.clone()))),
    }
}

/// A button with a hover, disabled with `off` as its hover (rule 1.2: the fix in place).
fn button(ui: &mut egui::Ui, text: &str, tip: &str, off: Option<&str>) -> bool {
    let b = ui.add_enabled(off.is_none(), egui::Button::new(text));
    match off {
        Some(why) => b.on_disabled_hover_text(why),
        None => b.on_hover_text(tip),
    }
    .clicked()
}

/// A button that writes to disk: a pointer click only, never Enter or Space on focus (FC5, Note FC10).
fn click_only_button(
    ui: &mut egui::Ui,
    b: egui::Button,
    name: Option<&str>,
    tip: &str,
    off: Option<&str>,
) -> bool {
    let r = ui.add_enabled(off.is_none(), b);
    if let Some(name) = name {
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, r.enabled(), name));
    }
    let by_key = r.has_focus()
        && ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space));
    let r = match off {
        Some(why) => r.on_disabled_hover_text(why),
        None => r.on_hover_text(tip),
    };
    r.clicked() && !by_key
}

/// The fix a row's button asks for this frame (Note FC3), or `None`; nothing for a row whose fix is drawn from its
/// level. P8's row has two buttons (Note FC10), so the answer is the fix, not a click.
fn fix_button(
    ui: &mut egui::Ui,
    m: &AppModel,
    p: &Problem,
    off: Option<&str>,
    sets_off: Option<&str>,
) -> Option<Fix> {
    let fix = p.fix.as_ref()?;
    // Right to left: Reload is the rightmost. Named for its file: the banner has a Reload of its own (rule 8.4).
    if let Fix::OpenInEditor(path) = fix {
        let r = ui.button("Reload");
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Reload sdc-project.toml")
        });
        if r.on_hover_text("Read sdc-project.toml again (asks first when something is unsaved)")
            .clicked()
        {
            return Some(Fix::Reload);
        }
        let tip = format!("Open {} with its default program", path.display());
        return button(ui, "Open in editor", &tip, None).then(|| fix.clone());
    }
    // P26, owner 2026-11-04 (Note FN5): one button per feature, neither a default; right to left, so the row reads
    // in the sentence's order.
    if let Fix::FeatureClash(id, f, b) = fix {
        for g in [b, f] {
            let name = feature_key(*g).trim_end_matches("_on");
            let text = format!("Turn off {name}");
            if button(
                ui,
                &text,
                "Turn this feature off for the set",
                off.or(sets_off),
            ) {
                return Some(Fix::TurnFeatureOff(id.clone(), *g));
            }
        }
        return None;
    }
    let clicked = match fix {
        // Note DU3: a set entry that matches no file is removed in place.
        Fix::RemoveMissing(miss) => {
            let At::Set { id, .. } = &p.at else {
                return None;
            };
            let (label, name) = match miss {
                Missing::NoFilesLeft => ("Delete set", format!("Delete set {id}")),
                _ => (
                    "Remove from set",
                    format!("Remove {} from {id}", miss.text()),
                ),
            };
            let b = ui.add_enabled(sets_off.is_none(), egui::Button::new(label));
            b.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, sets_off.is_none(), &name)
            });
            match sets_off {
                Some(why) => b.on_disabled_hover_text(why),
                None => b.on_hover_text("Remove this line from sdc-project.toml"),
            }
            .clicked()
        }
        Fix::RebuildZ(k) => {
            let on = m
                .project
                .as_ref()
                .and_then(|d| d.file_value(k, "channels.reconstruct_z"))
                == Some(json!(true));
            // DP7d: after Rebuild Z the exported file is still short until the next export.
            let done = on.then_some("Rebuild Z is on for this file: export again to rewrite it");
            button(
                ui,
                REBUILD_Z,
                "Rebuild the blue (Z) channel from red and green for this file (channels.reconstruct_z). Only for \
                 normal maps: it replaces the file's blue channel.",
                off.or(done),
            )
        }
        Fix::OpenFolder(path) => button(ui, "Open folder", &path.display().to_string(), None),
        Fix::TreatAsOpenGl(_) => button(
            ui,
            "Treat as OpenGL",
            "Flip this normal map's green channel (channels.green_convention = opengl)",
            off,
        ),
        // Asked for by a `FeatureClash` row's buttons (above), never a row's own fix.
        Fix::TurnFeatureOff(..) | Fix::FeatureClash(..) => false,
        Fix::FixPath(..) => button(ui, "Fix path", "Go to the set and its path field", None),
        Fix::UndoLast => {
            let tip = format!("Undo: {}", m.undo_label().unwrap_or_default());
            button(ui, UNDO_ROW, &tip, off)
        }
        Fix::ShowNifTab(_, tab, _) => {
            // The Meshes edit panel's own tab names (`ui/meshes/edit_panel.rs` TABS).
            let name = match tab {
                NifTab::Shader => "Shader",
                NifTab::Alpha => "Alpha",
                NifTab::Controllers => "Controllers",
                NifTab::Extra => "Extra data",
            };
            button(
                ui,
                &format!("Open the {name} tab"),
                "Open this NIF on the tab that holds the field",
                None,
            )
        }
        Fix::ChoosePreset => button(
            ui,
            "Choose preset…",
            "Open the Preset menu and pick an installed preset",
            None,
        ),
        // Drawn above with Open in editor.
        Fix::OpenInEditor(_) => false,
        Fix::Reload => button(ui, "Reload", "Read the project files again", None),
        Fix::ClearReadOnly(path) => click_only_button(
            ui,
            egui::Button::new("Clear read-only flag"),
            None,
            &format!("Clear the read-only flag of {}", path.display()),
            None,
        ),
        Fix::TagAsGrass(_) => button(
            ui,
            "Tag as grass",
            "Set its sub-type to Complex grass (subtype = \"complex_grass\" in sdc.toml)",
            off,
        ),
        Fix::StraightAlpha(_) => button(
            ui,
            "Straight alpha",
            "Read this EXR's colour as stored (alpha.exr_straight = true in sdc.toml)",
            off,
        ),
        Fix::RemoveMatch(_, n) => button(
            ui,
            "Remove match",
            &format!("Remove match{n} from the set's PGPatcher settings"),
            sets_off,
        ),
        Fix::UseRecordPaths(_) => button(
            ui,
            "Use new paths",
            "Write the record's current paths to this set's mapping; its outputs follow them",
            sets_off,
        ),
        // Note FC14: the hover names the file it rewrites, since the skipped one may be the original.
        Fix::RemoveAliases(file, id) => click_only_button(
            ui,
            egui::Button::new(format!("Remove aliases from {id}")),
            None,
            &format!(
                "Rewrites {} without its suffix_aliases table; its comments and other keys stay",
                file.display()
            ),
            m.export
                .running
                .is_some()
                .then_some(crate::model::also::EXPORT_RUNNING),
        ),
    };
    clicked.then(|| fix.clone())
}

pub fn tab(ui: &mut egui::Ui, model: &mut AppModel) {
    // The count comes from the same cached build as the list (K6), not a count per frame.
    let Listed {
        rows: list,
        checks,
        suggestions,
        hidden,
        hideable,
        ..
    } = rows(ui, model);
    let off = model.fix_off();
    let sets_off = model.sets_read_only_reason();
    let mut act = None;
    let label = |k: &String| {
        crate::model::sets::views(model)
            .iter()
            .find(|v| v.key == *k)
            .map_or(k.clone(), |v| v.id.clone())
    };
    let (pill_on, pill_to) = checks_pill(&model.bottom.filter);
    if model.bottom.filter.is_some() || checks > 0 || suggestions > 0 || hideable {
        ui.horizontal(|ui| {
            // Note FB9 (owner amendment A): the project's suggestions, one undo step; written by Save.
            if suggestions > 0 {
                let b = ui
                    .add_enabled(
                        off.is_none(),
                        egui::Button::new(format!("Accept all suggestions ({suggestions})")),
                    )
                    .on_hover_text(if model.bottom.filter.is_some() {
                        "Set every texture in the whole project that has a suggested type to it, not only the \
                         listed ones; the others stay listed"
                    } else {
                        "Set every texture that has a suggested type to it; the others stay listed"
                    })
                    .on_disabled_hover_text(off.as_deref().unwrap_or_default());
                if b.clicked() {
                    act = Some(Act::AcceptAll);
                }
            }
            // The chip and what its × leaves (the Checks pill stays as it is).
            let chip = match &model.bottom.filter {
                Some(ProblemsFilter::Undecided) => Some(("Undecided only ×".to_string(), None)),
                Some(ProblemsFilter::Set(k)) => Some((format!("Set: {} ×", label(k)), None)),
                Some(ProblemsFilter::SetChecks(k)) => {
                    Some((format!("Set: {} ×", label(k)), Some(ProblemsFilter::Checks)))
                }
                None | Some(ProblemsFilter::Checks) => None,
            };
            if let Some((text, back)) = chip {
                if ui.button(text).clicked() {
                    act = Some(Act::Filter(back));
                }
            }
            // Note FC3 (CL18 amended): one count, on the tab; checks are a filter pill of it (rule 6.5).
            if checks > 0 || pill_on {
                let r = ui
                    .add(egui::Button::selectable(pill_on, CHECKS_PILL).frame_when_inactive(true))
                    .on_hover_text("Show only the artist checks' rows");
                if r.clicked() {
                    act = Some(Act::Filter(pill_to.clone()));
                }
            }
            if hideable {
                if let Some(a) = kinds_menu(ui, model, hidden) {
                    act = Some(a);
                }
            }
        });
    }
    if list.is_empty() {
        if hidden > 0 {
            ui.weak("Every row here is hidden — see Kinds");
        } else {
            ui.weak("No problems");
        }
    }
    let row_h = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
    let m = &*model;
    // Review Focus 5: the focused row left the list (its question was answered) and egui dropped the focus, so the
    // row now at its index takes it, or the last row.
    let focus_key = egui::Id::new(ROW_FOCUS);
    let last: Option<(egui::Id, usize)> = ui.data(|d| d.get_temp(focus_key));
    let focused = ui.memory(|mem| mem.focused());
    if let (None, Some((rid, i)), Some(end)) = (focused, last, list.len().checked_sub(1)) {
        if !list.iter().any(|p| row_id(p) == rid) {
            let to = row_id(&list[i.min(end)]);
            ui.memory_mut(|mem| mem.request_focus(to));
        }
    }
    let mut focus_now = None;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, row_h, list.len(), |ui, range| {
            for (i, p) in list[range.clone()].iter().enumerate() {
                let i = range.start + i;
                // Note FB9: the row is focusable (registered first, it takes no click from its cells); Enter or
                // Space accepts its suggestion and moves to the next row. A row without one ignores both, and no
                // row's fix runs from a key (Review Focus 3).
                let rect = egui::Rect::from_min_size(
                    ui.cursor().min,
                    egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                );
                let rid = row_id(p);
                let row = ui.interact(rect, rid, egui::Sense::focusable_noninteractive());
                row.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Other, true, row_name(p))
                });
                if row.has_focus() {
                    // Once a frame: the next row takes focus in this same frame and sees the same key press.
                    let first = focus_now.replace((rid, i)).is_none();
                    if row.gained_focus() {
                        row.scroll_to_me(None);
                    }
                    let stroke = ui.visuals().selection.stroke;
                    ui.painter()
                        .rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
                    // Note FM35: a row that asks only its alpha confirms the type it shows. With a sub-type question
                    // too, Enter must not answer just one of the two, so it does nothing there.
                    let alpha = match &p.at {
                        At::Texture(k) => m.item(k).filter(|i| {
                            !i.decisions
                                .iter()
                                .any(|d| matches!(d, Decision::Subtype { .. }))
                        }),
                        _ => None,
                    }
                    .and_then(alpha_asked);
                    // Consumed only when the row acts, so the Grid's Enter (open the preview) never sees it too.
                    let acting = matches!((first, &off, &p.at), (true, None, At::Texture(_)))
                        && (p.suggested.is_some() || alpha.is_some());
                    let pressed = acting
                        && ui.input_mut(|k| {
                            k.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                                || k.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                        });
                    if let (true, At::Texture(k)) = (pressed, &p.at) {
                        let k = k.clone();
                        act = p
                            .suggested
                            .map(|st| RowPick::Decide(k.clone(), st))
                            .or(alpha.map(|a| RowPick::Answer(k, Answer::Alpha(a))))
                            .map(Act::Pick);
                        // The last row keeps focus: no wrap to the top.
                        if let Some(next) = list.get(i + 1) {
                            ui.memory_mut(|mem| mem.request_focus(row_id(next)));
                        }
                    }
                }
                let status = status_of(p.level);
                // The fix buttons are laid out first, so the sentence is cut to the room left (I2).
                let (l, r) = egui::Sides::new().shrink_left().truncate().show(
                    ui,
                    |ui| {
                        let mut a = None;
                        dot(ui, status);
                        match &p.at {
                            At::Texture(k) => {
                                if ui.link(k).clicked() {
                                    a = Some(Act::Texture(k.clone()));
                                }
                            }
                            At::Set { key, id } => {
                                if ui.link(id).clicked() {
                                    a = Some(Act::Set(key.clone()));
                                }
                            }
                            At::File(k) => {
                                if ui.link(k).clicked() {
                                    a = Some(Act::File(k.clone()));
                                }
                            }
                            // A NIF's link opens it, as its fix does.
                            At::Nif(path) => {
                                if ui.link(path).clicked() {
                                    a = p.fix.clone().map(|f| Act::Apply(f, p.at.clone()));
                                }
                            }
                            At::Project => {
                                ui.label("Project");
                            }
                        }
                        // Note FB8: a sentence that names the action; the engine's text and the code are the hover.
                        let mut text = egui::RichText::new(&*p.shown());
                        // Note FC3: check and Info rows are drawn weak.
                        if status.weak() {
                            text = text.weak();
                        }
                        ui.add(egui::Label::new(text).truncate()).on_hover_ui(|ui| {
                            ui.label(&p.text);
                            if let Some(d) = &p.detail {
                                ui.label(d);
                            }
                            if let Some(c) = p.codes_text() {
                                ui.weak(c);
                            }
                        });
                        a
                    },
                    |ui| {
                        // Right to left: the last widget added is the leftmost.
                        // Note FL: a carried block or ask row's fix waits for the full plan.
                        let off = if p.carried {
                            m.fix_off_for(p)
                        } else {
                            off.clone()
                        };
                        if let Some(f) = fix_button(ui, m, p, off.as_deref(), sets_off.as_deref()) {
                            return Some(Act::Apply(f, p.at.clone()));
                        }
                        match &p.at {
                            At::Texture(k) if p.level == Level::Decide => {
                                fix_buttons(ui, m, k, off.as_deref()).map(Act::Pick)
                            }
                            // Fix round 1: a set row keeps Go to set beside its fix.
                            At::Set { key, .. } if ui.button("Go to set").clicked() => {
                                Some(Act::Set(key.clone()))
                            }
                            _ => None,
                        }
                    },
                );
                if let Some(a) = l.or(r) {
                    act = Some(a);
                }
            }
        });
    // Kept while a row has focus, and over the frame its row leaves the list (egui drops the focus at that frame's
    // end; the next frame moves it, so ask for that frame).
    let keep = focus_now.or(last.filter(|(rid, _)| focused == Some(*rid)));
    if keep.is_some() && focus_now.is_none() {
        ui.ctx().request_repaint();
    }
    ui.data_mut(|d| {
        if let Some(v) = keep {
            d.insert_temp(focus_key, v);
        } else {
            d.remove::<(egui::Id, usize)>(focus_key);
        }
    });
    // Every click-through lands in the Textures view.
    let to_textures = |ui: &egui::Ui| super::header::set_meshes_view(ui.ctx(), false);
    match act {
        Some(Act::Texture(k)) => {
            // Note FB12: a helper in a collapsed Helpers group opens it, as the palette does. Local to this click:
            // `select_item` also runs for restores, which must not reopen a group the user collapsed.
            if model
                .item(&k)
                .is_some_and(|i| i.kind == crate::model::items::ItemKind::Helper)
            {
                model.filter.hide_helpers = false;
                model.nav.reveal_item = Some(k.clone());
            }
            model.select_item(&k);
            to_textures(ui);
        }
        Some(Act::Set(k)) => {
            model.select_set(&k);
            to_textures(ui);
        }
        // Re-check A5: a Vanilla or CM row of the last Also export names no texture; its file is shown in Explorer.
        Some(Act::File(k)) => match model.also_file(&k) {
            Some(f) => {
                if let Err(e) = model.reveal(&f) {
                    model.status_line = e;
                }
            }
            None => {
                model.select_job(&k);
                to_textures(ui);
            }
        },
        Some(Act::Pick(r)) => apply(model, r),
        Some(Act::Apply(fix, at)) => apply_fix(ui, model, fix, at),
        Some(Act::Filter(f)) => model.bottom.filter = f,
        Some(Act::AcceptAll) => model.accept_all_suggestions(),
        Some(Act::HideKind(code, hide)) => model.set_kind_hidden(code, hide),
        None => {}
    }
}

/// What a row's fix button does (Note FC3). Only the writes named in Review Focus 3 write, and only from this click.
fn apply_fix(ui: &egui::Ui, model: &mut AppModel, fix: Fix, at: At) {
    let to_textures = |ui: &egui::Ui| super::header::set_meshes_view(ui.ctx(), false);
    match fix {
        Fix::RemoveMissing(miss) => {
            if let At::Set { id, .. } = at {
                if let Err(why) = model.remove_missing(&id, &miss) {
                    model.status_line = why;
                }
            }
        }
        Fix::RebuildZ(k) => model.set_field(&k, "channels.reconstruct_z", json!(true)),
        Fix::OpenFolder(p) => model.shell.push(ShellRequest::Open(p)),
        Fix::TreatAsOpenGl(k) => {
            model.decide(&k, Answer::Green(engine::config::GreenConvention::Opengl))
        }
        Fix::TurnFeatureOff(id, f) => model.turn_feature_off(&id, f),
        // Its buttons ask for `TurnFeatureOff` (`fix_button`).
        Fix::FeatureClash(..) => {}
        Fix::FixPath(key, field) => {
            let id = crate::model::sets::views(model)
                .iter()
                .find(|v| v.key == key)
                .map(|v| v.id.clone());
            model.select_set(&key);
            to_textures(ui);
            if let Some(id) = id {
                let at = ui.ctx().cumulative_pass_nr();
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::new(FOCUS_FIELD), (id, field, at)));
            }
        }
        Fix::UndoLast => model.undo(),
        Fix::ShowNifTab(p, t, f) => model.nif_tab_request = Some((p, t, f)),
        Fix::ChoosePreset => {
            to_textures(ui);
            model.choose_preset = Some(ui.ctx().cumulative_pass_nr());
        }
        // Note FC10: each writes from this click only (Review Focus 3); a refusal goes to the status line.
        Fix::OpenInEditor(p) => model.shell.push(ShellRequest::Open(p)),
        Fix::Reload => model.request_reload(),
        Fix::ClearReadOnly(p) => {
            if let Err(why) = model.clear_read_only_flag(&p) {
                model.status_line = why;
            }
        }
        Fix::TagAsGrass(k) => model.tag_as_grass(&k),
        Fix::StraightAlpha(k) => model.set_field(&k, "alpha.exr_straight", json!(true)),
        Fix::RemoveMatch(id, n) => model.set_json_value(&id, &format!("match{n}"), None),
        Fix::UseRecordPaths(id) => {
            if let Err(why) = model.use_record_paths(&id) {
                model.status_line = why;
            }
        }
        Fix::RemoveAliases(file, _) => {
            if let Err(why) = model.remove_preset_aliases(&file) {
                model.status_line = why;
            }
        }
    }
}
