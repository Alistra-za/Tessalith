//! The centre panel for two or more textures or sets (M6 Track F3a, design §3-4). Drawing only: every edit is a
//! model call, made after drawing.
use super::columns::AMBER;
use super::group::{collapsing_group, GroupHead};
use super::settings_panel::{
    editor, field_label, likely_buttons, offered_groups, own_name_after_link, reach_combo,
    set_group_open, subtype_label, RowEdit,
};
use crate::look::Combo036 as _;
use crate::model::bulk::{is_value_dependent, BulkField, BulkOp, GreenReach};
use crate::model::fields::{effort_field, field_help, FieldView, EFFORT_FIELDS};
use crate::model::labels::type_hover;
use crate::model::panel_groups::{self, PanelGroup};
use crate::model::sets::SlotKind;
use crate::model::AppModel;
use eframe::egui;
use engine::config::{GreenConvention, Subtype, Target};
use engine::policy::ResolvedJob;
use serde_json::Value;

/// Note JI4: the bulk Effort row's text when the selected formats read different search fields.
const FORMATS_DIFFER: &str = "Formats differ: edit one format at a time";

enum Act {
    Kind(Option<SlotKind>),
    Edit(&'static str, RowEdit),
    Reencode(Vec<String>),
    Decide(Vec<String>, Subtype),
    SetGroupOpen(PanelGroup, bool),
    Green(Vec<String>, GreenConvention),
}

/// "{m} other selected textures already have a type." (the card's line under Apply).
fn typed_peers_line(m: usize) -> String {
    if m == 1 {
        "1 other selected texture already has a type.".into()
    } else {
        format!("{m} other selected textures already have a type.")
    }
}

/// Task 10 (3d-R3): the selected textures that wait for a type, asked once for all of them. `typed` counts the
/// selected textures that already have one. Drawn only when some selected texture asks.
fn undecided_card(
    ui: &mut egui::Ui,
    model: &AppModel,
    keys: &[String],
    typed: usize,
    likely: &[(Subtype, String)],
) -> Option<Act> {
    if keys.is_empty() {
        return None;
    }
    let n = keys.len();
    ui.add_space(4.0);
    ui.label(if n == 1 {
        "What kind is this texture?".to_string()
    } else {
        format!("What kind are these {n} textures?")
    });
    let id = ui.scope_id().with("bulk-undecided-type");
    // The pick belongs to this set of textures and to a type every one of them is offered: another selection, a
    // changed target or a changed grass context shows no pick (and Apply stays off) rather than a stale one.
    let offered_to_all = |st: Subtype| keys.iter().all(|k| model.type_offered_to(k, st));
    let mut chosen: Option<Subtype> = ui
        .data(|d| d.get_temp::<(Vec<String>, Subtype)>(id))
        .filter(|(ks, st)| ks == keys && offered_to_all(*st))
        .map(|(_, st)| st);
    // `complex_grass` is the one type that depends on the texture, so the shared list starts from the grass one.
    let groups: Vec<(&str, Vec<Subtype>)> = offered_groups(model.target, true)
        .into_iter()
        .map(|(h, ms)| (h, ms.into_iter().filter(|st| offered_to_all(*st)).collect()))
        .filter(|(_, ms): &(&str, Vec<Subtype>)| !ms.is_empty())
        .collect();
    if let Some(st) = likely_buttons(ui, likely) {
        chosen = Some(st);
    }
    let name = format!("Type for {n} texture{}", if n == 1 { "" } else { "s" });
    let shown = chosen.map_or_else(|| "Choose a type".to_string(), subtype_label);
    ui.horizontal(|ui| {
        let r = egui::ComboBox::from_id_salt(id)
            .selected_text(&shown)
            .show_ui_036(ui, |ui| {
                for (heading, members) in &groups {
                    ui.label(egui::RichText::new(*heading).weak());
                    for &st in members {
                        ui.selectable_value(&mut chosen, Some(st), subtype_label(st))
                            .on_hover_text(type_hover(st));
                    }
                }
            })
            .response;
        super::helper_panel::named(&r, egui::Role::ComboBox, &name, Some(&shown));
    });
    ui.data_mut(|d| match chosen {
        Some(c) => {
            d.insert_temp(id, (keys.to_vec(), c));
        }
        None => d.remove::<(Vec<String>, Subtype)>(id),
    });
    let off = model.fix_off().or_else(|| {
        chosen
            .is_none()
            .then(|| "Pick a type from the list first".to_string())
    });
    let r = ui.add_enabled(off.is_none(), egui::Button::new(format!("Apply to {n}")));
    let r = match &off {
        Some(why) => r.on_disabled_hover_text(why),
        None => r,
    };
    let act = (r.clicked() && off.is_none())
        .then_some(chosen)
        .flatten()
        .map(|st| Act::Decide(keys.to_vec(), st));
    if typed > 0 {
        ui.weak(typed_peers_line(typed));
    }
    ui.add_space(4.0);
    if act.is_some() {
        ui.data_mut(|d| d.remove::<(Vec<String>, Subtype)>(id));
    }
    act
}

/// Note FI3: re-encode the selection's skipped DDS files (the S6 `reencode` selector, a plan-time field, so not a
/// `FIELDS` row). Files another input already writes are left out, with their names on hover.
fn reencode_row(
    ui: &mut egui::Ui,
    model: &AppModel,
    split: &(Vec<String>, Vec<String>),
    acts: &mut Vec<Act>,
) {
    let (ok, blocked) = split;
    let (k, j) = (ok.len(), blocked.len());
    if k == 0 && j == 0 {
        return;
    }
    let read_only = model.toml_read_only_reason();
    let left_out = format!("{j} left out: another file already writes them");
    ui.horizontal(|ui| {
        let (text, off) = if k == 0 {
            ("Re-encode DDS files".to_string(), Some(left_out.clone()))
        } else {
            let s = if k == 1 { "" } else { "s" };
            (format!("Re-encode {k} DDS file{s}"), read_only)
        };
        let r = ui
            .add_enabled(off.is_none(), egui::Button::new(text))
            .on_hover_text(
                "Decode them and encode them again with this project's settings. Block-compressed files lose some quality.",
            );
        let r = match &off {
            Some(why) => r.on_disabled_hover_text(why),
            None => r,
        };
        if r.clicked() {
            acts.push(Act::Reencode(ok.clone()));
        }
        if k > 0 && j > 0 {
            ui.weak(format!("({left_out})"))
                .on_hover_text(blocked.join("\n"));
        }
    });
}

pub fn draw(ui: &mut egui::Ui, model: &mut AppModel, to_top: bool) {
    let mut acts = vec![];
    // Chips and rows come from one cached view (F3a: nothing recomputed per frame).
    let view = model.bulk_view();
    let sets = model.selected_sets();
    if sets.len() >= 2 {
        ui.heading(format!("{} sets selected", sets.len()));
        let chips = &view.chips;
        ui.horizontal_wrapped(|ui| {
            for c in chips {
                let on = model.bulk.kind == c.kind;
                if ui
                    .selectable_label(on, format!("{} ({})", c.label, c.members.len()))
                    .clicked()
                {
                    acts.push(Act::Kind(c.kind));
                }
            }
        });
        if let Some(c) = chips
            .iter()
            .find(|c| c.kind == model.bulk.kind && c.sets_with < sets.len())
        {
            ui.weak(format!("applies to {} of {} sets", c.sets_with, sets.len()));
        }
    }
    let keys = &view.keys;
    // A kind chip of two sets can hold one texture (final fix wave A7).
    ui.heading(format!(
        "{} selected",
        crate::model::code_text::counted(keys.len(), "texture", "textures")
    ));
    if let Some(r) = &model.bulk.report {
        ui.weak(r);
    }
    reencode_row(ui, model, &view.reencode, &mut acts);
    if let Some(a) = undecided_card(ui, model, &view.undecided.0, view.undecided.1, &view.likely) {
        acts.push(a);
    }
    // Options come from the active texture's job, else the first target's (design §3.2).
    let job = model.bulk_job(keys).cloned();
    match job {
        None => {
            ui.weak("None of the selected textures has settings of its own.");
        }
        Some(job) => {
            let read_only = model.toml_read_only_reason();
            if let Some(why) = &read_only {
                ui.colored_label(AMBER, why);
            }
            let (target, busy) = (model.target, model.edit_busy());
            let rows = &view.rows;
            // Slider state is per selection, not shared by every bulk selection.
            let id = format!(
                "bulk-{}-{}",
                keys.len(),
                keys.first().map_or("", String::as_str)
            );
            // ui-rules 3.2 (Note JI7): a bar drawn without hover, so a panel taller than the window says so.
            ui.spacing_mut().scroll = crate::look::thin_scroll();
            let mut area = egui::ScrollArea::vertical()
                .id_salt("centre-bulk")
                .auto_shrink([false, false]);
            if to_top {
                area = area.vertical_scroll_offset(0.0);
            }
            // One row of the grid. Note JI4: the Effort row's choices are those of a texture that reads its field
            // (the BC3 row's Custom entry is that texture's pair).
            let draw_row = |ui: &mut egui::Ui, r: &BulkField, acts: &mut Vec<Act>| {
                let reader = EFFORT_FIELDS
                    .contains(&r.field)
                    .then(|| {
                        keys.iter().find_map(|k| {
                            model
                                .item(k)
                                .and_then(|i| i.job.as_ref())
                                .filter(|j| effort_field(&j.profile) == Some(r.field))
                        })
                    })
                    .flatten();
                let job = reader.unwrap_or(&job);
                if let Some(e) = row(ui, &id, job, target, busy, r) {
                    acts.push(Act::Edit(r.field, e));
                }
            };
            let read_only = read_only.is_some();
            area.show(ui, |ui| {
                // The first screen: rows in no group, greyed by a read-only `sdc.toml` (LM11).
                ui.add_enabled_ui(!read_only, |ui| {
                    egui::Grid::new("bulk-first-screen")
                        .num_columns(4)
                        .show(ui, |ui| {
                            for f in panel_groups::FIRST_SCREEN {
                                for r in rows.iter().filter(|r| r.field == f) {
                                    draw_row(ui, r, &mut acts);
                                }
                            }
                        });
                    green_row(ui, model, keys, &view.green, &mut acts);
                });
                // Each header sits outside the disabled scope (R1-1, FK3): only its rows are greyed.
                for (g, summary) in &view.groups {
                    let g = *g;
                    let open = panel_groups::is_open(&model.settings, g);
                    let head = GroupHead {
                        title: g.title(),
                        summary: &summary.text,
                        summary_hover: summary.hover.as_deref(),
                        changed: summary.changed,
                        problem: None,
                    };
                    let r = collapsing_group(ui, g.id(), &head, open, false, |ui| {
                        if read_only {
                            ui.disable();
                        }
                        egui::Grid::new(("bulk-grid", g.id()))
                            .num_columns(4)
                            .show(ui, |ui| {
                                for r in rows
                                    .iter()
                                    .filter(|r| panel_groups::panel_group(r.field) == Some(g))
                                {
                                    draw_row(ui, r, &mut acts);
                                }
                            });
                    });
                    if r.toggled {
                        acts.push(Act::SetGroupOpen(g, !open));
                    }
                }
            });
        }
    }
    for a in acts {
        match a {
            Act::Kind(k) => model.bulk.kind = k,
            Act::Reencode(ks) => model.set_reencode(&ks, true),
            Act::Decide(ks, st) => model.decide_many(&ks, st),
            // The row already shows why a refusal happened (its buttons are off with the reason).
            Act::Green(ks, g) => drop(model.decide_many_green(&ks, g)),
            Act::SetGroupOpen(g, open) => set_group_open(model, g, open),
            Act::Edit(f, RowEdit::Set(v)) => model.apply_bulk_edit(keys, f, BulkOp::Set(v)),
            Act::Edit(f, RowEdit::Clear) => model.apply_bulk_edit(keys, f, BulkOp::Clear),
        }
    }
}

/// A3: the Green row, right under Format. "Green: OpenGL", the two buttons that write a convention on every selected
/// texture that takes green, and the reach note with the skip reasons on hover. Drawn only when some texture takes
/// green; every string comes from the cached view (`GreenReach`).
fn green_row(
    ui: &mut egui::Ui,
    model: &AppModel,
    keys: &[String],
    green: &GreenReach,
    acts: &mut Vec<Act>,
) {
    if green.takes.is_empty() {
        return;
    }
    let off = model.fix_off();
    // Wrapped, not a grid row: two long buttons in the first-screen grid would widen its columns past the panel.
    ui.horizontal_wrapped(|ui| {
        ui.label(green.label.as_str());
        for (text, g) in [
            ("Treat as OpenGL (flip green)", GreenConvention::Opengl),
            ("Treat as DirectX", GreenConvention::Directx),
        ] {
            let r = ui.add_enabled(off.is_none(), egui::Button::new(text));
            let r = match &off {
                Some(why) => r.on_disabled_hover_text(why),
                None => r,
            };
            if r.clicked() && off.is_none() {
                acts.push(Act::Green(keys.to_vec(), g));
            }
        }
        ui.label(egui::RichText::new(green.note.as_str()).small().weak())
            .on_hover_text(green.hover.as_str());
    });
}

/// One bulk row: the label; the editor (its mixed state, disabled when the field applies to none); the note, with
/// the skip reasons on hover; and a reset when some texture sets the field itself.
fn row(
    ui: &mut egui::Ui,
    id: &str,
    job: &ResolvedJob,
    target: Target,
    busy: bool,
    r: &BulkField,
) -> Option<RowEdit> {
    let mut edit = None;
    let label = field_label(ui, false, r.label, field_help(r.field));
    // Note JI4 (spec 4.6): textures whose formats read different search fields get no Effort choices.
    if r.formats_differ {
        ui.weak(FORMATS_DIFFER);
    } else {
        let view = FieldView {
            field: r.field,
            label: r.label,
            value: r.value.clone().unwrap_or(Value::Null),
            provenance: String::new(),
            edited: r.edited > 0,
            lock: None,
        };
        let (set, control) = ui
            .add_enabled_ui(r.applies > 0, |ui| {
                let mixed = r.value.is_none() && r.applies > 0;
                // E14: a value-dependent row lists every option with how many textures a pick reaches.
                // With no option to list the combo is drawn off, never the editor with one texture's locks.
                if is_value_dependent(r.field) {
                    return ui
                        .add_enabled_ui(!r.option_reach.is_empty(), |ui| {
                            reach_combo(
                                ui,
                                egui::Id::unique((id, r.field)),
                                r.field,
                                &view.value,
                                &r.option_reach,
                                r.total,
                                (mixed, busy),
                            )
                        })
                        .inner;
                }
                editor(ui, id, target, job, busy, &view, mixed)
            })
            .inner;
        // Review finding 16: a combo's text is its value, so the control takes the row's label as its name; the
        // threshold slider keeps its own (E28).
        own_name_after_link(control.labelled_by(label.id), r.field);
        if let Some(v) = set {
            edit = Some(RowEdit::Set(v));
        }
    }
    // E14: the note names options and counts, so it can outgrow the column: truncated, the whole text on hover with
    // the skip reasons under it (rule 3.4).
    let text = r.note();
    let note = ui.add(
        egui::Label::new(egui::RichText::new(&text).small().weak())
            .truncate()
            .show_tooltip_when_elided(false),
    );
    if !text.is_empty() {
        let full = if r.skipped.is_empty() {
            text
        } else {
            format!("{text}\n{}", r.skipped_text())
        };
        note.on_hover_text(full);
    }
    if r.edited > 0
        && ui
            .small_button("↺")
            .on_hover_text("Use the inherited value on every selected texture")
            .clicked()
    {
        edit = Some(RowEdit::Clear);
    }
    ui.end_row();
    edit
}
