//! The set editor (M3b spec §4): one scrolling column (Note FB2), the name box, the slot rows (layout v2 A2) at full
//! width, then the Material / Also export tabs and groups. Drawing only: every change is a model call, made after drawing.
use super::columns::{border_of, bulk_menu, status_text, AMBER, RED};
use super::sets_column::{DropState, Pending, SlotNote};
use super::settings_panel::alpha_choice;
use super::slot_commands::SlotCmdState;
use super::status::{dot, Status};
use super::thumbs::ThumbCache;
use crate::look::Combo036 as _;
use crate::model::bottom::{
    cached_set_reasons, flagged, hidden_reason_kinds, is_hidden, no_files_yet, scope_key,
    tile_ready, ProblemsFilter, ReasonGo,
};
use crate::model::bulk::MenuAction;
use crate::model::fields::{
    channel_text, encoding_options, field_hidden, field_lock, field_view, format_label,
    PARALLAX_HOVER,
};
use crate::model::items::{self, ItemKind, ItemStatus, TextureItem};
use crate::model::nav::SlotTarget;
use crate::model::out_size::{human, SizeEst};
use crate::model::pack_fills::{channel_helpers, channel_roles, fill_rows, FillRow};
use crate::model::pbr_export::export_blocks;
use crate::model::sets::{
    editor_cells, is_json_set, set_outputs, shows_json_panel, slot_clash_text, slot_of, slot_shown,
    views, SetView, SlotCell, SlotCommand, SlotFill, SlotKind, SlotLink, SlotShown,
    DETAIL_NORMAL_HOVER,
};
use crate::model::AppModel;
use crate::pgp::prefix::{active_matches, lock_key, match_toggle, pgp_locked, slot_match_path};
use eframe::egui;
use engine::config::{AlphaPolicy, ColorTag, Format, RawMap, Target};
use engine::events::RecipeReport;
use engine::pack::RecipeKind;
use engine::shader::Shader;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Note AN: the caption across a PGPatcher-locked slot card.
/// Review M2: a lock key on a set that writes no JSON.
pub const PGP_LOCK_UNWRITTEN: &str = "lock stored, not written (no PGPatcher entry)";

/// Note FB2: the centre's floor while the 2D set editor is shown, so its slot rows fit without clipping. Measured at
/// 1280 px on the fixture's PBR set (system fonts): Slot 151 ("Coat normal / Fuzz (slot 7)"), thumbnail 56, Format 185
/// (an RMAOS row's fill rows), Flags 21, Status 8, File at `FILE_MIN`, five gaps and the scroll bar reserve make 511;
/// plus the centre panel's 16 px of margin and a little slack make 540. Note JI5 adds the diffuse row's alpha dropdown
/// (`ALPHA_W`), Confirm and their two gaps (about 184 px), rounded up: 724.
pub const ROWS_MIN: f32 = 724.0;

/// Note JI5: the slot row's alpha dropdown, wide enough for `Cutout + Blend` (the longest `alpha_label`) uncut plus
/// the combo's arrow. Measured by `slot_alpha::alpha_w_fits_the_longest_label`, which fails when the fonts say otherwise.
pub const ALPHA_W: f32 = 114.0;

const THUMB: f32 = 56.0;
/// Owner A1-5 (Note DQ2): the hover of the packed row's title on an Advanced Skin set that keeps `_rmaos` because
/// it has no map `_rfaos` could be built from.
pub const KEEPS_RMAOS: &str =
    "RMAOS, not RFAOS: RFAOS needs a roughness, glossiness or AO map, and this set has none.";

/// The format combo (A5).
pub(super) const COMBO_W: f32 = 96.0;
/// The File cell is the stretchy one (it truncates); a Grid column sizes itself from its content, so it gets what the
/// row's other cells leave of the column, and at least this.
const FILE_MIN: f32 = 60.0;
/// Between the row cells.
const SPACING: f32 = 4.0;
/// Seconds a single click waits for its second click (a little over egui's double-click delay).
const DOUBLE_CLICK_WAIT: f64 = 0.35;

enum Act {
    Select(String),
    Expand(String),
    Format(String, Format, ColorTag),
    /// Note JI5: the diffuse slot row's alpha type (item key, type), and its reset.
    Alpha(String, AlphaPolicy),
    ClearAlpha(String),
    /// "Enable parallax" on a complex `_m` (DECISIONS AH2).
    Parallax(String, bool),
    /// Clear the `_m`'s own `alpha.parallax` (the reset next to a locked box).
    ParallaxReset(String),
    Fill(String, RawMap, Option<u8>),
    /// Note DY18: a packed channel's source (packed key, channel, helper item key; `None` = a value).
    ChannelHelper(String, u8, Option<String>),
    /// A Match on this click: (set id, `matchN`, value; `None` removes it), one Undo step each (`toggle_match`).
    Match(String, String, Option<Value>),
    /// Note AN: read a locked slot's original textures (set id, slot) on a worker.
    AskOriginal(String, u8),
    /// F3a Task 8: a slot menu pick.
    Slot(MenuAction),
    /// Review E1: open the set a linked slot's file comes from (set id).
    OpenSet(String),
    /// Item 17: files from Explorer dropped on this set's slot card.
    FileDrop(String, SlotKind),
    /// Note AO7: take a member out of the set (set id, item), from a clashing slot card.
    RemoveMember(String, String),
    /// Note HH: the set stops sharing a helper (set id, item).
    StopSharing(String, String),
    /// Review E4: a status line reason's button.
    Reason(ReasonGo),
    /// Review E4: the status line's "+k more in Problems".
    Problems,
}

/// Review E4: a Slot reason asks the empty slot's **Assign…** (by its id salt, `assign_id`) for focus; the row takes
/// it on its next draw, and a draw that does not take it drops it.
const ASSIGN_FOCUS: &str = "set-editor-assign-focus";

/// The reasons the status line names; the rest are "+k more in Problems".
const STATUS_REASONS: usize = 3;

/// A slot row's id: by slot number, so the two kinds of a shared card are one row; the XPMF maps (no number) by kind.
fn slot_row_id(set_id: &str, slot: SlotKind) -> egui::Id {
    match slot.slot_number() {
        Some(n) => egui::Id::unique(("slot-row", set_id, n)),
        None => egui::Id::unique(("slot-row", set_id, slot)),
    }
}

/// Review E4: the editor's first line, **Ready** or "Not ready:" and up to three reasons, each a button to its fix.
fn status_line(ui: &mut egui::Ui, model: &AppModel, v: &SetView) -> Option<Act> {
    let reasons = cached_set_reasons(model, v);
    // A set the export blocks on has the card's red dot (`sets_column`), whatever its words say.
    let blocked = export_blocks(model).iter().any(|i| i.set_id == v.id);
    let mut act = None;
    ui.horizontal_wrapped(|ui| {
        // Note JP5 (B3-17): a set with no key and no row of its own; amber, as its card. A key-less set with a row
        // reads its reasons below.
        if no_files_yet(&flagged(model), v) {
            dot(
                ui,
                if blocked {
                    Status::Blocks
                } else {
                    Status::Warning
                },
            );
            ui.label(super::sets_column::NO_FILES_YET);
            return;
        }
        if reasons.is_empty() {
            dot(
                ui,
                if blocked {
                    Status::Blocks
                } else {
                    Status::Ready
                },
            );
            ui.label("Ready: nothing in this set needs you.");
            return;
        }
        dot(
            ui,
            if blocked {
                Status::Blocks
            } else {
                Status::Warning
            },
        );
        ui.label("Not ready:");
        let hidden = model.hidden_kinds();
        let shown = &reasons[..reasons.len().min(STATUS_REASONS)];
        for (i, r) in shown.iter().enumerate() {
            if i > 0 {
                ui.weak("·");
            }
            // Two shown reasons with one text are told apart by their row's subject, in the words, the name and the
            // hover; a unique text keeps its plain name. A Slot reason has no row: its twin (the same set's same slot)
            // is the same place.
            let subject = match &r.go {
                ReasonGo::Row(p) if shown.iter().filter(|o| o.text == r.text).count() > 1 => {
                    Some(super::problems::row_subject(p))
                }
                _ => None,
            };
            let with_subject = |t: &str| match subject {
                Some(s) => format!("{t} ({s})"),
                None => t.to_string(),
            };
            let b = ui.button(with_subject(&r.text));
            // Rule 3.4: a cut text is named by the whole one.
            let name = format!(
                "{} — go to the fix",
                with_subject(r.full.as_deref().unwrap_or(&r.text))
            );
            b.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
            // Hiding never changes readiness, so a hidden kind's row is still a reason.
            let tip = match &r.go {
                ReasonGo::Row(p) if is_hidden(p, &hidden) => {
                    "Go to the control that fixes this.\nThis kind is hidden; this shows it again."
                }
                _ => "Go to the control that fixes this.",
            };
            let b = b.on_hover_ui(|ui| {
                if let Some(full) = &r.full {
                    ui.label(full);
                }
                if let Some(s) = subject {
                    ui.label(s);
                }
                ui.label(tip);
            });
            if b.clicked() {
                act = Some(Act::Reason(r.go.clone()));
            }
        }
        let more = reasons.len().saturating_sub(STATUS_REASONS);
        if more > 0 && ui.link(format!("+{more} more in Problems")).clicked() {
            act = Some(Act::Problems);
        }
    });
    act
}

/// Review E4: what a status line click asks for, applied after drawing. `three_d`: the editor shows the 3D view.
fn status_act(ui: &egui::Ui, model: &mut AppModel, v: &SetView, act: Act, three_d: bool) {
    match act {
        Act::Reason(ReasonGo::Slot(kind)) => {
            // The slot rows are 2D only: switch back as the 2D | 3D switch does; the next frame draws the rows, the
            // reveal and the Assign… focus.
            if three_d && model.settings.set_view_3d {
                model.settings.set_view_3d = false;
                model.save_settings();
            }
            // Slot reasons come from warnings on a keyed plan set.
            model.go_to_set(&v.key, Some(SlotTarget::Slot(kind)));
            let id = slot_row_id(&v.id, kind).with("assign");
            ui.data_mut(|d| d.insert_temp(egui::Id::unique(ASSIGN_FOCUS), id));
        }
        Act::Reason(ReasonGo::Row(p)) => {
            // The row it focuses must be in the list.
            if let Some(code) = p.code.filter(|_| is_hidden(&p, &model.hidden_kinds())) {
                model.set_kind_hidden(code, false);
            }
            model.open_set_problems(crate::model::SetTarget::view(v));
            super::problems::focus_row(ui.ctx(), super::problems::row_id(&p));
        }
        Act::Problems => {
            // Show the editor's counted kinds before opening the ordinary JP5 filter.
            for code in hidden_reason_kinds(model, v) {
                model.set_kind_hidden(code, false);
            }
            model.open_problems(Some(ProblemsFilter::Set(scope_key(v).to_string())));
        }
        // `status_line` makes no other act.
        _ => {}
    }
}

/// How the set editor shares the view with the 3D preview panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct View {
    /// The 2D | 3D switch is offered (the app has a 3D preview).
    pub switch: bool,
    /// The 3D panel shows this set: the slot rows are left out, the TruePBR settings stay.
    pub three_d: bool,
    /// The 3D preview's picked MATO record's `noiseUVScale`, where the XPMF card's Override starts (M8).
    pub noise_uv: Option<f32>,
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    model: &mut AppModel,
    active: &crate::model::ResolvedSelection,
    thumbs: &mut ThumbCache,
    cmds: &mut SlotCmdState,
    drops: &mut DropState,
    view: View,
) {
    let selected = active.view();
    super::row_reveal::take(
        ui.ctx(),
        &mut model.nav,
        active.target.as_ref(),
        selected.filter(|_| !view.three_d).map(|v| v.id.as_str()),
    );
    let Some(v) = selected else {
        ui.weak("This set is no longer in the plan.");
        return;
    };
    // Review E4: the first line, in both views, above the scrolling column so it stays in sight.
    let status = status_line(ui, model, v);
    let assign_focus = egui::Id::unique(ASSIGN_FOCUS);
    if view.three_d {
        // 3D (AG132): the set's 3D view takes the rest of this pane (`ui::centre`); the TruePBR settings and the skin
        // card are in the TruePBR settings… window, as are the CS PBR section and the complex material card, so an edit
        // still shows live in 3D.
        // No Output files group here either: the viewport's share of the centre is pinned (`set_preview_layout`).
        head(ui, model, v, view);
        // No slot rows here: a waiting Assign… focus is dropped.
        ui.data_mut(|d| d.remove::<egui::Id>(assign_focus));
        if let Some(a) = status {
            status_act(ui, model, v, a, true);
        }
        return;
    }
    // `size_of` needs the model mutably, which the drawing below does not have.
    let sizes: HashMap<String, SizeEst> = v
        .members
        .iter()
        .filter_map(|k| model.size_of(k).map(|e| (k.clone(), e)))
        .collect();
    let mut acts: Vec<Act> = status.into_iter().collect();
    // Item 17: where a file from Explorer was dropped this frame, if one waits for a slot card.
    let drop_at = drops.file.as_ref().and_then(|f| f.pos);
    let off = model.sets_read_only_reason();
    let editable = off.is_none();
    let why = off.unwrap_or_default();
    // Note FB2 (amendment C): one column, as the texture settings panel: the name box, the slot rows at full width,
    // then the tabs and groups. Nothing sits beside the rows, so nothing can cover them. ui-rules 3.2 (CC21, Note JI7):
    // a bar drawn without hover.
    ui.spacing_mut().scroll = crate::look::thin_scroll();
    let mut area = egui::ScrollArea::vertical()
        .id_salt("centre-set")
        .auto_shrink([false, false]);
    // Go to set's scroll to its row (`row_reveal`) still wins: egui applies this offset as the area begins and a
    // scroll request as it ends.
    if super::take_scroll_reset(ui.ctx()) {
        area = area.vertical_scroll_offset(0.0);
    }
    let out = area.show(ui, |ui| {
        head(ui, model, v, view);
        outputs(ui, model, v);
        rows(ui, model, v, thumbs, &mut acts, drop_at, &sizes);
        slot_note(ui, drops, &v.id);
        unslotted_packed(ui, model, v, &mut acts);
        shared_rows(ui, model, v, thumbs, &why, &mut acts);
        ui.add_space(6.0);
        // A2: a normal button, not stretched across the justified column.
        ui.horizontal(|ui| {
            ui.add_enabled_ui(editable, |ui| {
                if ui.button("Slot commands…").clicked() {
                    cmds.open = Some(v.id.clone());
                }
            })
            .response
            .on_disabled_hover_text(&why);
        });
        under_grid(ui, v);
        ui.add_space(6.0);
        tabs(ui, model, v, view.noise_uv);
    });
    #[cfg(feature = "test-hooks")]
    ui.data_mut(|d| {
        d.insert_temp(
            egui::Id::unique("set-editor-observed-offset"),
            out.state.clamped_offset().y,
        )
    });
    #[cfg(not(feature = "test-hooks"))]
    let _ = out;
    super::row_reveal::end(ui.ctx());
    // The slot rows took a waiting Assign… focus, or it has no Assign… to go to (a filled slot): either way it is gone.
    ui.data_mut(|d| d.remove::<egui::Id>(assign_focus));
    let (mut select, mut expand) = (None, None);
    for a in acts {
        match a {
            Act::Select(k) => select = Some(k),
            Act::Expand(k) => expand = Some(k),
            Act::Format(k, f, t) => model.set_format(&k, f, t),
            Act::Alpha(k, p) => model.set_alpha(&k, p),
            Act::ClearAlpha(k) => model.clear_alpha(&k),
            Act::Parallax(k, on) => model.set_field(&k, "alpha.parallax", serde_json::json!(on)),
            Act::ParallaxReset(k) => model.clear_field(&k, "alpha.parallax"),
            Act::Fill(k, role, v) => model.set_pack_fill(&k, role, v),
            Act::ChannelHelper(k, c, h) => model.set_channel_helper(&k, c, h.as_deref()),
            Act::Match(set, k, v) => model.toggle_match(&set, &k, v),
            Act::AskOriginal(set, n) => model.ask_original_textures(&set, n),
            Act::Slot(a) => super::slot_commands::run(model, cmds, a),
            Act::OpenSet(id) => {
                // The link source owns a planned member output; a key-less view has no members.
                if let Some(key) = views(model)
                    .iter()
                    .find(|s| s.id == id)
                    .map(|s| s.key.clone())
                {
                    model.select_set(&key);
                }
            }
            Act::FileDrop(set, slot) => file_drop(model, drops, &set, slot),
            Act::StopSharing(set, item) => {
                if let Err(why) = model.stop_sharing(&set, &item) {
                    drops.slot_note = Some(SlotNote {
                        set,
                        text: why,
                        error: true,
                    });
                }
            }
            Act::RemoveMember(set, item) => {
                if let Err(why) = model.remove_member(&set, &item) {
                    drops.slot_note = Some(SlotNote {
                        set,
                        text: why,
                        error: true,
                    });
                }
            }
            a @ (Act::Reason(_) | Act::Problems) => status_act(ui, model, v, a, false),
        }
    }
    select_or_expand(ui, model, &v.id, select, expand);
}

/// The 2D | 3D switch, the input name box (the set's one name, C14), the output prefix, the output name and the
/// delete-section warnings, then a separator above the 2D slot rows.
fn head(ui: &mut egui::Ui, model: &mut AppModel, v: &SetView, view: View) {
    ui.horizontal_wrapped(|ui| {
        if view.switch {
            super::set_preview::view_switch(ui, model);
        }
        // The set's path and its input name box share the row where the width allows (AO5).
        super::input_name::picker(ui, model, v, !view.three_d);
        if view.three_d {
            super::set_preview::settings_toggle(ui, model);
        }
        // The prefix error is a PGPatcher JSON fact: any JSON set shows it. The arrow names where files go, so only a
        // PBR set (whose textures are written under the prefix) draws it (review E13, C14).
        if shows_json_panel(v, model.target) {
            match (&v.prefix, &v.prefix_error) {
                (Some(p), _) if v.pbr => {
                    ui.weak(format!("→ {p}"));
                }
                (Some(_), _) => {}
                (_, Some(e)) => {
                    ui.colored_label(RED, e);
                }
                _ => {
                    ui.weak("");
                }
            };
        }
    });
    // F3b: the set's output name, or why it is ignored.
    let mut clear_name = false;
    if let Some(n) = &v.output_name {
        ui.horizontal_wrapped(|ui| {
            if model.renames_outputs(v) {
                ui.colored_label(
                    AMBER,
                    format!("Output name {n:?} is ignored on a PBR set; use Rename"),
                );
            } else {
                ui.weak(format!("Outputs named {n}"));
            }
            clear_name = ui.small_button("Clear").clicked();
        });
    }
    if clear_name {
        model.clear_output_name(&v.id);
    }
    // G6: a delete section that matches this set's textures deletes the shapes the set is for.
    for h in crate::model::sections::delete_hits(model)
        .iter()
        .filter(|h| engine::sets::same_id(&h.set_id, &v.id))
    {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(
                AMBER,
                format!(
                    "⚠ Delete section {} deletes this set's shapes: {}",
                    h.section + 1,
                    h.why
                ),
            );
            if ui.small_button("Open the JSON editor").clicked() {
                model.open_json_editor();
            }
        });
    }
    if !view.three_d {
        ui.separator();
    }
}

/// The Match toggle's label; its AccessKit name adds the slot: "Match on this (Diffuse)" (final wave Group 5).
pub const MATCH_ON_THIS: &str = "Match on this";

/// The first line of an open Output files group: what its header counts (final wave Group 2, a proposed string).
pub const OUTPUTS_COUNT_NOTE: &str =
    "The count is of the files at these paths now, whichever export wrote them.";

struct OutputLine {
    visible: Arc<egui::RichText>,
    full: Arc<egui::RichText>,
    name: String,
    reveal: String,
}

/// Review E13: the files this set's own members write, in a group closed by default. Its header counts the files at
/// these paths now, whichever export wrote them (`stat_is_file` is cached with a TTL, so the closed header costs no
/// stat per frame); an open group says so, then lists each file with a Show in Explorer button once it exists. The
/// lines are built only while the group is open.
fn outputs(ui: &mut egui::Ui, model: &mut AppModel, v: &SetView) {
    let files = set_outputs(v);
    let n = files.len();
    let on_disk = files
        .iter()
        .filter(|(_, _, p)| model.stat_is_file(p))
        .count();
    let summary_key = egui::Id::unique(("output-summary", n, on_disk));
    let summary = super::paint_cached(ui.ctx(), summary_key, || {
        #[cfg(feature = "test-hooks")]
        super::paint_counts::Work::OutputSummary.built();
        let text = if n == 0 {
            "none".to_string()
        } else if on_disk == n {
            format!("{n}, all on disk")
        } else if on_disk == 0 {
            format!("{n}, none on disk")
        } else {
            format!("{on_disk} of {n} on disk")
        };
        Arc::new(egui::RichText::new(text).weak())
    });
    let id = ui.scope_id().with(("set-outputs", &v.id));
    let mut reveal = None;
    super::group::group_impl(
        ui,
        id,
        "Output files",
        (Some(summary), None),
        false,
        false,
        |ui| {
            if files.is_empty() {
                ui.weak("Nothing is written for this set.");
                return;
            }
            ui.weak(OUTPUTS_COUNT_NOTE);
            let root_key = egui::Id::unique((
                "output-root",
                model.root.as_ref().map(|p| p.as_os_str()),
                model.export_folder_ref().map(|p| p.as_os_str()),
            ));
            let root = super::paint_cached(ui.ctx(), root_key, || Arc::new(model.export_root()));
            for (kind, item, abs) in files {
                let member_name = kind
                    .is_none()
                    .then(|| model.item(item).map_or("File", |i| i.name.as_str()));
                let key = root_key.with((
                    abs.as_os_str(),
                    kind,
                    item,
                    member_name,
                    v.rfaos,
                    v.shader.as_ref().map(|s| s.shader),
                ));
                let line = super::paint_cached(ui.ctx(), key, || {
                    #[cfg(feature = "test-hooks")]
                    super::paint_counts::Work::OutputLine.built();
                    let label = kind.map_or_else(
                        || member_name.unwrap_or("File"),
                        |k| v.row_title(k, Some(item)),
                    );
                    let rel = abs.strip_prefix(root.as_path()).unwrap_or(abs);
                    let full = abs.display().to_string();
                    let visible = format!("{label}: {}", rel.to_string_lossy().replace('/', "\\"));
                    Arc::new(OutputLine {
                        visible: Arc::new(egui::RichText::new(visible)),
                        name: format!("{label}: {full}"),
                        reveal: format!("Show {full} in Explorer"),
                        full: Arc::new(egui::RichText::new(full)),
                    })
                });
                egui::Sides::new().shrink_left().truncate().show(
                    ui,
                    |ui| {
                        let r = ui
                            .add(
                                egui::Label::new(line.visible.clone())
                                    .truncate()
                                    .show_tooltip_when_elided(false),
                            )
                            .on_hover_text(line.full.clone());
                        // The name keeps the whole path when the line is cut short.
                        r.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Label, true, &line.name)
                        });
                    },
                    |ui| {
                        if model.stat_is_file(abs) {
                            let scroll = ui.spacing().scroll;
                            ui.add_space(
                                scroll.bar_width
                                    + scroll.bar_outer_margin
                                    + ui.spacing().item_spacing.x,
                            );
                            let r = ui.small_button("Show in Explorer");
                            // Named by the whole path: two outputs can share a file name (final wave Group 5).
                            r.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::Role::Button, true, &line.reveal)
                            });
                            if r.clicked() {
                                reveal = Some(abs.clone());
                            }
                        }
                    },
                );
            }
        },
    );
    if let Some(p) = reveal {
        if let Err(e) = model.reveal(&p) {
            model.status_line = e;
        }
    }
}

/// The set editor's tab (Note JI2): kept in egui memory only, never in gui.toml (spec 2.6, review finding 9).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum SetTab {
    #[default]
    Main,
    AlsoExport,
}

const TAB_ID: &str = "set-editor-tab";
/// Set with the tab by Review's link: the tab row scrolls to the top once, so the recipe is in view.
const SCROLL_ID: &str = "set-editor-tab-scroll";

pub(super) fn show_main_tab(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::unique(TAB_ID), SetTab::Main);
        d.remove::<bool>(egui::Id::unique(SCROLL_ID));
    });
}

/// Open the set editor on its Also export tab, scrolled to it (Review's "Recipe settings…", Note JI2).
pub fn show_also_tab(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::unique(TAB_ID), SetTab::AlsoExport);
        d.insert_temp(egui::Id::unique(SCROLL_ID), true);
    });
}

/// The tabs under the slot rows (C6, R12; Note FB2): Material on a JSON set or Set on any other, then Also export
/// (Note JI2).
fn tabs(ui: &mut egui::Ui, model: &mut AppModel, v: &SetView, noise_uv: Option<f32>) {
    let json = shows_json_panel(v, model.target);
    let id = egui::Id::unique(TAB_ID);
    let was = ui.data(|d| d.get_temp::<SetTab>(id)).unwrap_or_default();
    let mut tab = was;
    let row = ui.horizontal(|ui| {
        ui.selectable_value(
            &mut tab,
            SetTab::Main,
            if json { "Material" } else { "Set" },
        );
        ui.selectable_value(&mut tab, SetTab::AlsoExport, "Also export");
        // C14 (Note FB2): the name box above names the set, so the Material tab has no name line; its JSON editor
        // button sits on the tab row. G3: the whole file, every set and the delete sections, in a centre tab (Note FB7).
        if json && tab == SetTab::Main {
            ui.add_space(8.0);
            if ui
                .small_button("PGPatcher JSON editor…")
                .accessible_name("Material: PGPatcher JSON editor…")
                .on_hover_text(
                    "Every set's entry and the delete sections of the PGPatcher JSON, in one view",
                )
                .clicked()
            {
                model.open_json_editor();
            }
        }
    });
    // A request made on the frame the editor opens wins over `View::to_top`'s offset (the area applies it last).
    if ui
        .data_mut(|d| d.remove_temp::<bool>(egui::Id::unique(SCROLL_ID)))
        .is_some()
    {
        row.response.scroll_to_me(Some(egui::Align::TOP));
    }
    if tab != was {
        ui.data_mut(|d| d.insert_temp(id, tab));
    }
    ui.separator();
    if tab == SetTab::AlsoExport {
        super::also_tab::body(ui, model, v);
        return;
    }
    super::cs_pbr::card(ui, model, v);
    super::xpmf::card(ui, model, v, noise_uv);
    // K12: on a JSON set that is not PBR Landscape the CS PBR records sit between Matching and Advanced TruePBR
    // settings; `pgp_panel` (a PBR Landscape set's) draws both halves itself.
    let split = json && !(model.target == Target::Cs && crate::model::sets::is_pbr_landscape(v));
    if split {
        super::pgp_fields::panel_main(
            ui,
            model,
            &super::pgp_fields::context_of(model, &v.id, Some(v)),
        );
        super::cs_pbr::section(ui, model, v);
        super::pgp_fields::advanced_of(ui, model, v);
    } else if json {
        super::cs_pbr::pgp_panel(ui, model, v);
    }
    if v.skin {
        super::skin_card::card(ui, model, v);
    }
    super::cm_card::section(ui, model, v);
    if !split {
        super::cs_pbr::section(ui, model, v);
    }
}

/// Item 17: a file from Explorer dropped on slot `slot` of `set`. The drop is taken (its folder is not opened); a
/// filled slot asks Replace / Cancel as a texture dragged onto the set does, anything refused is the error line.
fn file_drop(model: &mut AppModel, drops: &mut DropState, set: &str, slot: SlotKind) {
    let Some(f) = drops.file.take() else { return };
    let r = model.drop_file_on_slot(set, slot, &f.paths);
    let line = |text: String, error: bool| SlotNote {
        set: set.to_string(),
        text,
        error,
    };
    drops.slot_note = match r.outcome {
        crate::model::sets::DropOutcome::Refused(why) => Some(line(why, true)),
        crate::model::sets::DropOutcome::AskReplace { existing } => {
            if let Some(item) = r.item {
                drops.pending = Some(Pending::Replace {
                    item,
                    set: set.to_string(),
                    existing,
                    choice: crate::model::sets::DropChoice::Move,
                });
            }
            r.note.map(|n| line(n.into(), false))
        }
        // `drop_choice` with Move never asks the Share / Duplicate choice.
        _ => r.note.map(|n| line(n.into(), false)),
    };
}

/// Item 17: the last file drop's error or note, under the slot cards of the set it was for.
fn slot_note(ui: &mut egui::Ui, drops: &DropState, set: &str) {
    if let Some(n) = drops.slot_note.as_ref().filter(|n| n.set == set) {
        if n.error {
            ui.colored_label(RED, &n.text);
        } else {
            ui.weak(&n.text);
        }
    }
}

/// The recipe lines and what matches nothing (the set's problems are on the status line, review E4).
fn under_grid(ui: &mut egui::Ui, v: &SetView) {
    if v.key.is_empty() {
        return;
    }
    for r in &v.recipes {
        ui.add_space(4.0);
        recipe(ui, r);
    }
    for m in &v.missing {
        ui.colored_label(AMBER, format!("no file matches {}", m.text()));
    }
}

fn recipe(ui: &mut egui::Ui, r: &RecipeReport) {
    let (word, color) = status_text(r.status);
    let file = r.output.rsplit('/').next().unwrap_or(&r.output);
    let line = egui::RichText::new(format!("{file}: {word}"));
    ui.label(match color {
        Some(c) => line.color(c),
        None => line,
    });
    if let Some(reason) = &r.reason {
        ui.label(egui::RichText::new(reason).small().weak());
    }
    for (name, p) in ["R", "G", "B", "A"].iter().zip(&r.channels) {
        if !p.is_empty() {
            ui.label(egui::RichText::new(format!("{name}: {}", channel_text(p))).small());
        }
    }
}

/// Layout v2 A2: one row per slot, in PGPatcher slot order: Slot · thumbnail · File · Format · Flags · Status.
fn rows(
    ui: &mut egui::Ui,
    model: &AppModel,
    v: &SetView,
    thumbs: &mut ThumbCache,
    acts: &mut Vec<Act>,
    drop_at: Option<egui::Pos2>,
    sizes: &HashMap<String, SizeEst>,
) {
    // The File cell takes what the other columns leave: their widest content last frame, plus the spacing.
    let others_id = egui::Id::unique(("slot-rows-others", &v.id));
    let others = ui.data(|d| d.get_temp::<[f32; 5]>(others_id));
    // The column's floating scroll bar senses its full width over the content's right edge: the Status dot keeps
    // clear of it, or its hover would go to the bar.
    let bar = ui.spacing().scroll.bar_width + ui.spacing().scroll.bar_outer_margin;
    let file_w = others.map_or(FILE_MIN, |w| {
        (ui.available_width() - w.iter().sum::<f32>() - 5.0 * SPACING - bar).max(FILE_MIN)
    });
    // The format cells' base widths (the combo at `COMBO_W`) of last frame: what a combo may widen into never reads
    // the widths a widened combo made, so a frame's widths do not feed back into themselves.
    let base_id = egui::Id::unique(("slot-rows-base", &v.id));
    let base = ui.data(|d| d.get_temp::<[f32; 5]>(base_id));
    let free = base.map_or(0.0, |b| {
        (ui.available_width() - b.iter().sum::<f32>() - 5.0 * SPACING - bar - FILE_MIN).max(0.0)
    });
    let format_base = base.map_or(0.0, |b| b[2]);
    let mut widths = [0.0f32; 5];
    let mut base_widths = [0.0f32; 5];
    // The skin `_wet` and `_detail_n` cards belong to an Advanced skin set only (`editor_cells`).
    let cells = editor_cells(v);
    egui::Grid::new(("slot-rows", &v.id))
        .num_columns(6)
        .striped(true)
        .spacing([SPACING, 4.0])
        .min_col_width(0.0)
        .show(ui, |ui| {
            // The row takes the right-click; its labels are not selectable, so they do not take it.
            ui.style_mut().interaction.selectable_labels = false;
            // E17: the labels of the columns, in one line above the rows. No row with a format combo, no heading.
            let has_job = |c: &SlotCell| own_item(model, v, c).is_some_and(|i| i.job.is_some());
            if cells.iter().any(has_job) {
                let head = |ui: &mut egui::Ui, text: &str| {
                    if text.is_empty() {
                        ui.allocate_space(egui::Vec2::ZERO);
                    } else {
                        ui.label(egui::RichText::new(text).small().weak());
                    }
                };
                head(ui, "Slot");
                head(ui, "");
                head(ui, "File");
                head(ui, "Format");
                // The Flags cell's alpha combo is the Diffuse row's (`row`).
                let alpha = cells
                    .iter()
                    .any(|c| c.kind == SlotKind::Diffuse && has_job(c));
                if alpha && shows_json_panel(v, model.target) {
                    // The diffuse row draws the Match toggle before its alpha combo: "Alpha" starts at the combo.
                    let span = match_button_width(ui) + ui.spacing().item_spacing.x;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.add_space(span);
                        head(ui, "Alpha");
                    });
                } else {
                    head(ui, if alpha { "Alpha" } else { "" });
                }
                head(ui, "");
                ui.end_row();
            }
            for cell in &cells {
                let size = cell.fill.item().and_then(|i| sizes.get(i));
                let w = row(
                    ui,
                    model,
                    v,
                    cell,
                    thumbs,
                    acts,
                    drop_at,
                    size,
                    RowRoom {
                        file_w,
                        free,
                        format_base,
                        flags_w: others.map_or(0.0, |w| w[3]),
                    },
                );
                for (a, b) in widths.iter_mut().zip(w.widths) {
                    *a = a.max(b);
                }
                for (a, b) in base_widths.iter_mut().zip(w.base_widths) {
                    *a = a.max(b);
                }
            }
        });
    ui.data_mut(|d| {
        d.insert_temp(others_id, widths);
        d.insert_temp(base_id, base_widths);
    });
}

/// What a slot row has to size its cells by, from last frame (`rows`).
#[derive(Clone, Copy)]
struct RowRoom {
    /// The File cell's width.
    file_w: f32,
    /// What the row's cells can still take beyond their base widths before the File cell goes under `FILE_MIN`.
    free: f32,
    /// The Format column's base width.
    format_base: f32,
    flags_w: f32,
}

/// The widths a row drew: its cells' (`widths`, the Format cell with its combo as drawn) and the same with every
/// format combo at `COMBO_W` (`base_widths`).
struct RowWidths {
    widths: [f32; 5],
    base_widths: [f32; 5],
}

/// The slot's own item when its row draws that item's controls: not a link, not a borrowed file (the one rule for the
/// heading, the format combo and the alpha combo).
fn own_item<'a>(model: &'a AppModel, v: &SetView, cell: &SlotCell) -> Option<&'a TextureItem> {
    if matches!(slot_shown(v, cell.kind), SlotShown::Linked(_)) {
        return None;
    }
    match &cell.fill {
        SlotFill::Own { item } => model.item(item),
        _ => None,
    }
}

/// The link mark's square: the toggle is as wide as the glyph button it replaces (`look_036`'s pin on the Format column).
const MARK_W: f32 = 13.0;

/// Width reserved for the Match button on its row and before the Alpha heading.
fn match_button_width(ui: &egui::Ui) -> f32 {
    MARK_W + 2.0 * ui.spacing().button_padding.x
}

/// A Flags toggle painted as the link mark (two rings, no emoji glyph): its accessible name is the full `label`, which
/// also leads its hover. True when clicked.
fn flag(
    ui: &mut egui::Ui,
    enabled: bool,
    on: bool,
    (label, name): (&str, &dyn Fn() -> String),
    hover: &str,
    why: &str,
) -> bool {
    let mark = egui::IdSalt::new("flag-mark");
    let mut button = None;
    ui.add_enabled_ui(enabled, |ui| {
        let a = egui::Button::selectable(on, egui::Atom::custom(mark, egui::Vec2::splat(MARK_W)))
            .min_size(egui::vec2(match_button_width(ui), 0.0))
            .atom_ui(ui);
        if let Some(rect) = a.rect(mark) {
            let c = ui.style().interact_selectable(&a.response, on).text_color();
            super::sets_column::link_mark(ui.painter(), rect, c);
        }
        button = Some(a.response);
    });
    let r = button.expect("the toggle is drawn");
    // Named before its hover is attached (egui-ui-rules 8.8), by `name` (it tells two toggles apart), with the hover's
    // sentence as its description; the hover is built when it shows (final wave Groups 5 and 7).
    ui.ctx().accesskit_node_builder(r.id, |b| {
        b.set_label(name());
        b.set_description(hover);
    });
    r.on_hover_ui(|ui| {
        ui.label(format!("{label}. {hover}"));
    })
    .on_disabled_hover_text(why)
    .clicked()
}

#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut egui::Ui,
    model: &AppModel,
    v: &SetView,
    cell: &SlotCell,
    thumbs: &mut ThumbCache,
    acts: &mut Vec<Act>,
    drop_at: Option<egui::Pos2>,
    size: Option<&SizeEst>,
    room: RowRoom,
) -> RowWidths {
    let file_w = room.file_w;
    let slot = cell.kind;
    let fill = &cell.fill;
    // K1: a Grid cell cannot hold the whole row, so last frame's row rect takes the right-click (F3a Task 8) and the
    // Explorer drop (item 17). It is registered before the cells, so they keep their own clicks.
    let row_id = slot_row_id(&v.id, slot);
    if let Some(r) = ui.data(|d| d.get_temp::<egui::Rect>(row_id)) {
        let resp = ui.interact(r, row_id, egui::Sense::click());
        slot_menu(&resp, model, v, slot, acts);
        // The clipped rect: a row scrolled out of view takes no drop (trace-debug #51).
        if drop_at.is_some_and(|p| resp.interact_rect.contains(p)) {
            acts.push(Act::FileDrop(v.id.clone(), slot));
        }
    }
    let top = ui.cursor().min;
    let locked = slot.slot_number().is_some_and(|n| v.locks.contains(&n));
    let title = v.row_title(slot, fill.item());
    // Review E1: what the exported JSON carries. A linked texture belongs to the other set, so a `Linked` row has no
    // item: no thumbnail, format, alpha or status of its own, and no row to reveal.
    let shown = slot_shown(v, slot);
    let (item_key, borrowed) = match fill {
        _ if matches!(shown, SlotShown::Linked(_)) => (None, None),
        SlotFill::Own { item } => (Some(item.clone()), None),
        SlotFill::Borrowed { from, item } => (Some(item.clone()), Some(from.clone())),
        SlotFill::Empty => (None, None),
    };
    let item = item_key.as_deref().and_then(|k| model.item(k));

    // Slot: the title, dim when empty.
    let text = if locked {
        format!("🔒 {title}")
    } else {
        title.to_string()
    };
    let text = egui::RichText::new(text);
    let t = ui.label(if shown.is_filled() {
        text.strong()
    } else {
        text.weak()
    });
    let slot_w = t.rect.width();
    let t = if locked {
        t.on_hover_text("Slot pinned (sdc): right-click to unpin")
    } else {
        t
    };
    // Note GI (X16): CS does not draw the projected detail normal in this build.
    let landscape = v
        .shader
        .as_ref()
        .is_some_and(|p| matches!(p.shader, Shader::Landscape | Shader::PbrLandscape));
    if slot == SlotKind::DetailNormal && landscape {
        t.clone().on_hover_text(DETAIL_NORMAL_HOVER);
    }
    // Owner A1-5 (Note DQ2): an Advanced Skin set with no RFAOS source keeps RMAOS; the title says why (the disabled
    // RFAOS option's hover is gone with the combo), in its AccessKit description too (rule 8).
    if slot == SlotKind::Rmaos
        && model.target == Target::Cs
        && v.shader
            .as_ref()
            .is_some_and(|p| p.shader == Shader::AdvancedSkin)
        && item
            .and_then(|i| i.job.as_ref())
            .and_then(|j| j.pack.as_ref())
            .is_some_and(|pk| pk.kind == RecipeKind::Rmaos)
        && item_key
            .as_deref()
            .is_some_and(|k| model.rfaos_unbuildable(k).is_some())
    {
        t.clone().on_hover_text(KEEPS_RMAOS);
        ui.ctx()
            .accesskit_node_builder(t.id, |b| b.set_description(KEEPS_RMAOS));
    }
    if let (false, Some(p)) = (cell.usable, &v.shader) {
        let what = slot
            .game_slot()
            .map_or_else(|| "this map".to_string(), |n| format!("slot {n}"));
        t.on_hover_text(format!(
            "{title}: the {} shader does not use {what}",
            p.shader.label(),
        ));
    }

    // Thumbnail.
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(THUMB, THUMB), egui::Sense::click());
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Image, true, format!("{title} thumbnail"))
    });
    slot_menu(&resp, model, v, slot, acts);
    // A PGPatcher-locked slot has one hover, the lock's, in place of the thumbnail's path.
    let lock_cross = slot.game_slot().is_some_and(|n| pgp_locked(&v.settings, n))
        && is_json_set(v, model.target);
    let resp = if lock_cross {
        pgp_lock_hover(resp, model, v, slot, acts)
    } else {
        resp
    };
    if ui.is_rect_visible(rect) {
        match item.and_then(|i| thumbs.get(ui.ctx(), i)) {
            Some(t) => egui::Image::new(&t)
                .fit_to_exact_size(rect.size())
                .paint_at(ui, rect),
            None => {
                ui.painter().rect_filled(
                    rect,
                    2.0,
                    ui.visuals().widgets.noninteractive.weak_bg_fill,
                );
            }
        }
        if let Some(i) = item {
            ui.painter().rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.0, border_of(ui, &i.status)),
                egui::StrokeKind::Inside,
            );
        }
        if lock_cross {
            pgp_lock_mark(ui, rect, resp.id.with("pgp-lock"));
        }
    }
    if let Some(i) = item.filter(|_| borrowed.is_none()) {
        let resp = if lock_cross {
            resp
        } else {
            resp.on_hover_text(&i.key)
        };
        if resp.double_clicked() {
            acts.push(Act::Expand(i.key.clone()));
        } else if resp.clicked() {
            acts.push(Act::Select(i.key.clone()));
        }
    }

    // File: the name (truncated; full name and size on hover) and a weak size line, or "empty"; a clashing slot lists
    // both members under it.
    ui.vertical(|ui| {
        ui.set_width(file_w);
        match (&shown, item) {
            (SlotShown::Linked(link), _) => linked_cell(ui, model, v, slot, link, acts),
            (_, Some(i)) => {
                let size =
                    size.map(|e| format!("{}×{} · {}", e.width, e.height, human(e.file_bytes())));
                let hover = match &size {
                    Some(s) => format!("{}\n{s}", i.name),
                    None => i.name.clone(),
                };
                ui.add(egui::Label::new(egui::RichText::new(&i.name).small()).truncate())
                    .on_hover_text(hover);
                if let Some(s) = size {
                    ui.add(egui::Label::new(egui::RichText::new(s).small().weak()).truncate());
                }
                if matches!(shown, SlotShown::Cleared { .. }) {
                    let off = slot.game_slot().and_then(|n| model.slot_command_off(v, n));
                    cleared_cell(ui, v, slot, &i.name, off.as_deref(), acts);
                }
            }
            // Note HG: a slot the set's record names that no set in the project writes counts as supplied.
            (_, None) => match slot.slot_number().and_then(|n| v.record_supplied.get(&n)) {
                Some(path) => {
                    let name = path.rsplit('/').next().unwrap_or(path);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("From record: {name}")).weak(),
                        )
                        .truncate(),
                    )
                    .on_hover_text(path);
                }
                None => {
                    ui.weak("empty");
                }
            },
        }
        // Note AO7: both kinds of a shared slot (slot 7: `_cnr` and `_f`) are a conflict, not a silent pick: the row
        // names and shows both, and either can be taken out of the set here.
        if slot_clash_text(cell).is_some() {
            clash_rows(ui, model, v, cell, thumbs, acts);
        }
    });

    // Format, or Assign… on an empty slot (R13: the slot's own menu); a packed output's recipe and fills under it.
    let toml_read_only = model.toml_read_only_reason();
    let own = own_item(model, v, cell);
    let own_job = own.and_then(|i| i.job.as_ref().map(|j| (i, j)));
    // The widest thing in the Format cell that is not the format combo (`RowRoom::format_base`).
    let mut other_w = 0.0f32;
    let mut combo_drawn = false;
    let format_w = ui
        .vertical(|ui| {
            if let (SlotShown::Linked(_), Some(n)) = (&shown, slot.game_slot()) {
                // Review E1: the link is what the slot gives; Unlink removes the stored `slotN` and nothing else, so
                // what is under it shows again (ruling 3b-W1: a borrow line stays). Refused like the slot menu's
                // entries: a read-only `sdc-project.toml`, a pinned slot.
                let off = model.slot_command_off(v, n);
                let r = ui
                    .add_enabled(off.is_none(), egui::Button::new("Unlink"))
                    .on_hover_text("Remove the link. The slot goes back to what this set gives.")
                    .on_disabled_hover_text(off.as_deref().unwrap_or_default());
                other_w = other_w.max(r.rect.width());
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        r.enabled(),
                        format!("Unlink {}", v.slot_label(slot, "")),
                    )
                });
                if r.clicked() {
                    acts.push(Act::Slot(MenuAction::Slot {
                        set: v.id.clone(),
                        n,
                        cmd: SlotCommand::RemoveSlotKey,
                    }));
                }
            } else if item.is_none() {
                // Review E4: salted by the row, so a status line reason can ask this button for focus.
                let assign_id = row_id.with("assign");
                let menu = ui
                    .push_id(assign_id, |ui| {
                        crate::look::menu_button_036(ui, "Assign…", |ui| {
                            // C14: the slot label once ("Emissive / Skin (slot 3)").
                            bulk_menu(ui, v.slot_label(slot, ""), &model.slot_menu(&v.id, slot))
                        })
                    })
                    .inner;
                other_w = other_w.max(menu.response.rect.width());
                let focus_key = egui::Id::unique(ASSIGN_FOCUS);
                if ui.data(|d| d.get_temp::<egui::Id>(focus_key)) == Some(assign_id) {
                    ui.data_mut(|d| d.remove::<egui::Id>(focus_key));
                    menu.response.request_focus();
                }
                if let Some(Some(a)) = menu.inner {
                    acts.push(Act::Slot(a));
                }
            }
            let Some((i, job)) = own_job else { return };
            let (cur_f, cur_t) = (job.profile.format, job.profile.color.tag);
            // Format and Opaque write `sdc.toml`: a read-only one would drop the edit silently.
            let lock = toml_read_only
                .clone()
                .or(field_lock(model.target, &job.profile, "format").map(str::to_string));
            let mut choice = (cur_f, cur_t);
            // E17: a value reads in full where the row has room: the combo takes what its cell and the File cell
            // leave, up to the painted value. The values are short fixed strings; `truncate` is the floor's answer.
            let key = button_fit_key(ui).with(("slot-format", cur_f, cur_t));
            let (value, want) = super::paint_cached(ui.ctx(), key, || {
                #[cfg(feature = "test-hooks")]
                super::paint_counts::Work::Format.built();
                let text = format_label(cur_f, cur_t);
                let want = combo_want(ui, &text);
                (Arc::new(egui::RichText::new(text)), want)
            });
            // Its cell's slack: the column is `format_base` wide whatever this combo does.
            let slack = (room.format_base - COMBO_W).max(0.0);
            let w = combo_w(room.free + slack, want, COMBO_W);
            combo_drawn = true;
            ui.add_enabled_ui(lock.is_none(), |ui| {
                let h = ui.spacing().interact_size.y;
                let r = ui
                    .allocate_ui(egui::vec2(w, h), |ui| {
                        egui::ComboBox::from_id_salt(("slot-format", &v.id, slot.label()))
                            .selected_text(value.clone())
                            .width(w)
                            .truncate()
                            .show_ui_036(ui, |ui| {
                                for (e, label) in encoding_options(model.target, &job.profile) {
                                    ui.selectable_value(&mut choice, e, label);
                                }
                            })
                            .response
                    })
                    .inner;
                let r = super::name_input(r, &format!("{} format", slot.label()));
                // A value the row has no room for is cut: its full text is on hover (rule 3.4), and on a disabled
                // combo it leads the reason's line (as the pickers' hover does).
                if w + 0.01 < want {
                    // what the hover shows, for a screen reader too (final wave Group 5)
                    ui.ctx()
                        .accesskit_node_builder(r.id, |b| b.set_description(value.text()));
                    let why = lock.as_deref();
                    r.on_hover_ui(|ui| {
                        ui.label(value.clone());
                    })
                    .on_disabled_hover_ui(|ui| {
                        if let Some(why) = why {
                            ui.label(format!("{}\n{why}", value.text()));
                        } else {
                            ui.label(value.clone());
                        }
                    });
                }
            })
            .response
            .on_disabled_hover_text(lock.unwrap_or_default());
            if choice != (cur_f, cur_t) {
                acts.push(Act::Format(i.key.clone(), choice.0, choice.1));
            }
            if i.kind == ItemKind::Packed {
                // Last frame's Flags column: the lines run on across it (ruling A2-7: `rows` stores it).
                let used =
                    packed_controls(ui, model, i, toml_read_only.as_deref(), room.flags_w, acts);
                other_w = other_w.max(used);
            }
        })
        .response
        .rect
        .width();
    // The same cell with the combo at `COMBO_W` (a row with no combo has none to widen).
    let format_base_w = if combo_drawn {
        other_w.max(COMBO_W)
    } else {
        format_w
    };

    // Flags: Match on this (slots 1/2 of a JSON set), an icon toggle that keeps its full label as its accessible
    // name and leads its hover with it, and the diffuse's alpha type (Note JI5), on one line.
    let flags_w = ui
        .horizontal_top(|ui| {
            if let (Some(n), Some(_)) = (
                match slot {
                    _ if !shows_json_panel(v, model.target) => None,
                    SlotKind::Diffuse => Some(1),
                    SlotKind::Normal => Some(2),
                    _ => None,
                },
                item,
            ) {
                // F1 spec §3.2: the row's switch is the Matching header's slot 1/2 row.
                let derived = v.derived_match.as_deref().unwrap_or("");
                let on = active_matches(&v.settings, derived)
                    .iter()
                    .any(|(m, _)| *m == n);
                let path = slot_match_path(derived, slot.suffix());
                // Audit lead 17: a read-only sdc-project.toml is the reason.
                let refused = model.sets_read_only_reason();
                // The Diffuse and Normal toggles are told apart by the slot (egui-ui-rules 8.4).
                let name = || format!("{MATCH_ON_THIS} ({})", v.slot_label(slot, ""));
                if flag(
                    ui,
                    refused.is_none(),
                    on,
                    (MATCH_ON_THIS, &name),
                    "PGPatcher applies the entry to meshes with this exact texture in this slot",
                    refused.as_deref().unwrap_or_default(),
                ) {
                    let (k, val) = match_toggle(n, !on, &path);
                    acts.push(Act::Match(v.id.clone(), k, val));
                }
            }
            let Some((i, job)) = own_job.filter(|_| slot == SlotKind::Diffuse) else {
                return;
            };
            // Note JI5: the proposal while it asks, else the resolved type (a preset's shows too). An edit waits for
            // the plan, as the texture panel's rows do (Note FS1); read-only `sdc.toml` is the same reason.
            let proposal = items::alpha_asked(i);
            let shown = proposal.unwrap_or(job.profile.alpha.policy);
            let off = model.fix_off();
            let why = off.as_deref().unwrap_or_default();
            let mut picked = None;
            ui.add_enabled_ui(off.is_none(), |ui| {
                // The dropdown is as wide as its longest label.
                ui.spacing_mut().combo_width = ALPHA_W;
                picked = alpha_choice(
                    ui,
                    egui::Id::unique(("slot-alpha", &v.id)),
                    shown,
                    proposal.is_some(),
                    why,
                    "Slot",
                );
            });
            if let Some(p) = picked {
                acts.push(Act::Alpha(i.key.clone(), p));
            }
            let own_alpha = model
                .project
                .as_ref()
                .is_some_and(|p| p.file_value(&i.key, "alpha.policy").is_some());
            if own_alpha {
                let r = ui.add_enabled(off.is_none(), egui::Button::new("↺").small());
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, r.enabled(), "Reset slot alpha")
                });
                if r.on_hover_text("Go back to the inherited alpha")
                    .on_disabled_hover_text(why)
                    .clicked()
                {
                    acts.push(Act::ClearAlpha(i.key.clone()));
                }
            }
        })
        .response
        .rect
        .width();

    // Status: the texture's dot (K3), with the short lines (K4: the title's hovers keep the long forms) on its hover.
    let status_w = ui
        .horizontal(|ui| {
            let mut lines = vec![];
            if let Some(from) = &borrowed {
                lines.push(format!("borrowed from {from}"));
                if slot == SlotKind::Normal && model.target == Target::Cs {
                    lines.push("If that set has several normals, the roughness bake can use a different one from the file shown here.".into());
                }
            }
            if let (false, Some(p)) = (cell.usable, &v.shader) {
                lines.push(format!("not used by {}", p.shader.label()));
            }
            if let Some(i) = item {
                let s = match i.status {
                    ItemStatus::Failed(_) | ItemStatus::Error(_) => Status::Blocks,
                    _ if tile_ready(model, i) => Status::Ready,
                    _ => Status::Warning,
                };
                let r = dot(ui, s);
                if !lines.is_empty() {
                    r.on_hover_text(lines.join("\n"));
                }
            }
            // A4: only a set that writes JSON gives PGPatcher the lock (review M2); otherwise a note.
            if slot.game_slot().is_some_and(|n| pgp_locked(&v.settings, n)) && !lock_cross {
                let r = ui
                    .add(egui::Label::new(egui::RichText::new("ⓘ").weak()))
                    .on_hover_text(PGP_LOCK_UNWRITTEN);
                ui.ctx()
                    .accesskit_node_builder(r.id, |b| b.set_value(PGP_LOCK_UNWRITTEN));
            }
        })
        .response
        .rect
        .width();
    let rect = egui::Rect::from_min_max(top, ui.min_rect().max);
    ui.end_row();
    ui.data_mut(|d| d.insert_temp(row_id, rect));
    // A filled slot the shader cannot use stays at its place, outlined, and says why (F1 spec §5).
    if !cell.usable {
        ui.painter().rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(2.0, AMBER),
            egui::StrokeKind::Outside,
        );
    }
    super::row_reveal::on_row(ui, &v.id, slot, item_key.as_deref(), rect);
    RowWidths {
        widths: [slot_w, THUMB, format_w, flags_w, status_w],
        base_widths: [slot_w, THUMB, format_base_w, flags_w, status_w],
    }
}

/// Review E1: the File cell of a slot the set's JSON entry fills with a path (`slotN`): the file, then the set it comes
/// from (a button that opens it) or "set by path"; under it, each own file of the same card the link replaces.
fn linked_cell(
    ui: &mut egui::Ui,
    model: &AppModel,
    v: &SetView,
    kind: SlotKind,
    link: &SlotLink,
    acts: &mut Vec<Act>,
) {
    let file = link.path.rsplit('\\').next().unwrap_or(&link.path);
    let sentence = || format!("PGPatcher reads {} for this slot.", link.path);
    let hover = |ui: &mut egui::Ui| {
        ui.label(sentence());
    };
    // The label shows the file name; its description holds the whole path the hover shows (final wave Group 5).
    let described = |r: &egui::Response| {
        r.ctx
            .accesskit_node_builder(r.id, |b| b.set_description(sentence()));
    };
    // The set link keeps its size on the right, but takes at most half of the cell and truncates (its full text on
    // hover); the file name truncates on the left in what is left and keeps its text on hover.
    let half = ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).floor();
    egui::Sides::new().shrink_left().truncate().show(
        ui,
        |ui| match (&link.from, &link.from_short) {
            (Some(_), Some(_)) => {
                let r = ui.add(egui::Label::new(egui::RichText::new(file).small()).truncate());
                described(&r);
                r.on_hover_ui(hover);
            }
            _ => {
                let r = ui.add(
                    egui::Label::new(egui::RichText::new(format!("{file} · set by path")).small())
                        .truncate(),
                );
                described(&r);
                r.on_hover_ui(hover);
            }
        },
        |ui| {
            if let (Some(from), Some(short)) = (&link.from, &link.from_short) {
                ui.set_max_width(half);
                // Sides lays its right side out right to left and `horizontal` keeps that direction, so the link is
                // added first and the separator lands on its left (final wave Group 8, ledger 68). The link truncates
                // in what the separator leaves of the half.
                ui.horizontal(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                    let sep = egui::RichText::new(" · ").small().weak();
                    let sep_w = egui::WidgetText::from(sep.clone())
                        .into_galley(
                            ui,
                            Some(egui::TextWrapMode::Extend),
                            f32::INFINITY,
                            egui::TextStyle::Small,
                        )
                        .size()
                        .x;
                    let room =
                        (ui.available_width() - sep_w - ui.spacing().item_spacing.x).max(0.0);
                    let r = ui
                        .scope(|ui| {
                            ui.set_max_width(room);
                            ui.add(egui::Link::new(
                                egui::RichText::new(format!("from set {short}")).small(),
                            ))
                        })
                        .inner;
                    let r = r.on_hover_ui(|ui| {
                        ui.label(format!("from set {short}"));
                        ui.label("Open the set this file comes from.");
                    });
                    // Two slots linked from one set name the slot, so their names differ (egui-ui-rules 8.4); the
                    // hover's sentence is its description (final wave Group 5).
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Link,
                            true,
                            format!("Open set {short} ({})", v.slot_label(kind, "")),
                        )
                    });
                    ui.ctx().accesskit_node_builder(r.id, |b| {
                        b.set_description("Open the set this file comes from.")
                    });
                    if r.clicked() {
                        acts.push(Act::OpenSet(from.clone()));
                    }
                    ui.label(sep);
                });
            }
        },
    );
    // An own `_f` under a link to a `_cnr` goes nowhere: the JSON's slot 7 is the link.
    let n = kind.game_slot().unwrap_or_default();
    for other in v
        .slots
        .keys()
        .filter(|k| **k != kind && k.shares_card(kind))
    {
        if let SlotShown::Replaced { item, .. } = slot_shown(v, *other) {
            let own = model.item(item).map_or_else(
                || item.rsplit('/').next().unwrap_or(item).to_string(),
                |i| i.name.clone(),
            );
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "{own} still converts to DDS. The JSON uses the linked file for slot {n}."
                    ))
                    .small()
                    .weak(),
                )
                .truncate(),
            )
            .on_hover_ui(|ui| {
                ui.label(format!("Unlink to use {own} in slot {n}."));
            });
        }
    }
}

/// Review E1: under the own file's name, a slot the set cleared (`slotN = ""`): the file is converted but the JSON
/// leaves it out. **Use this file** removes the stored `slotN = ""` and nothing else (`RemoveSlotKey`, ruling 3b-W1),
/// so the own or shared file it names fills the slot again; `off` is the slot menu's reason for refusing it.
fn cleared_cell(
    ui: &mut egui::Ui,
    v: &SetView,
    kind: SlotKind,
    name: &str,
    off: Option<&str>,
    acts: &mut Vec<Act>,
) {
    ui.add(
        egui::Label::new(
            egui::RichText::new("Left out of the JSON: you cleared this slot.")
                .small()
                .weak(),
        )
        .truncate(),
    );
    let Some(n) = kind.game_slot() else { return };
    let r = ui
        .add_enabled(off.is_none(), egui::Button::new("Use this file").small())
        .on_disabled_hover_text(off.unwrap_or_default());
    r.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            r.enabled(),
            format!("Use {name} for {}", v.slot_label(kind, "")),
        )
    });
    if r.clicked() {
        acts.push(Act::Slot(MenuAction::Slot {
            set: v.id.clone(),
            n,
            cmd: SlotCommand::RemoveSlotKey,
        }));
    }
}

/// What the PGPatcher lock reads and is named: the caption under the padlock and its AccessKit name.
const PGP_LOCK_CAPTION: &str = "kept by PGPatcher";

/// Note FC9 (AN6, supersedes the red ✕): a PGPatcher-locked slot is a grey padlock over a dimmed thumbnail with the
/// caption `kept by PGPatcher`. It is not an error, so it takes no status colour. The caption is painted (it must not
/// move the row), so a node of its own carries the name.
fn pgp_lock_mark(ui: &egui::Ui, rect: egui::Rect, id: egui::Id) {
    let grey = ui.visuals().weak_text_color();
    // Clipped to the thumbnail; below the width the caption needs only the padlock is drawn (the name stays).
    let p = ui.painter().with_clip_rect(rect);
    p.rect_filled(
        rect,
        2.0,
        ui.visuals().extreme_bg_color.gamma_multiply(0.45),
    );
    let caption = rect.width() >= 50.0;
    let lift = if caption { 9.0 } else { 0.0 };
    super::input_name::paint_lock(&p, rect.center() - egui::vec2(0.0, lift), grey);
    if caption {
        let font = egui::FontId::proportional(9.0);
        for (i, line) in ["kept by", "PGPatcher"].into_iter().enumerate() {
            p.text(
                rect.center() + egui::vec2(0.0, 1.0 + 10.0 * i as f32),
                egui::Align2::CENTER_TOP,
                line,
                font.clone(),
                grey,
            );
        }
    }
    ui.ctx().accesskit_node_builder(id, |b| {
        b.set_role(egui::accesskit::Role::Label);
        b.set_label(PGP_LOCK_CAPTION);
        b.set_value(PGP_LOCK_CAPTION);
    });
}

/// Note AN: the locked card's one hover: the lock, then (with a scan) the textures the matched meshes have in that slot,
/// read on a worker ("…" until it lands).
fn pgp_lock_hover(
    resp: egui::Response,
    model: &AppModel,
    v: &SetView,
    slot: SlotKind,
    acts: &mut Vec<Act>,
) -> egui::Response {
    // Only a PGPatcher slot is locked (`lock_cross`).
    let Some(n) = slot.game_slot() else {
        return resp;
    };
    resp.on_hover_ui(|ui| {
        ui.label(format!(
            "Kept by PGPatcher ({}): the mesh's original texture is used here",
            lock_key(n).unwrap_or_default()
        ));
        match model.original_textures(&v.id, n) {
            Some(Some(paths)) if !paths.is_empty() => {
                for p in paths.iter() {
                    ui.monospace(p);
                }
            }
            Some(Some(_)) => {
                ui.weak("The scan has no mesh with this set's diffuse");
            }
            // The read runs on a worker (AN6); it lands on a later frame.
            Some(None) => {
                ui.weak("…");
                acts.push(Act::AskOriginal(v.id.clone(), n));
            }
            None => {}
        }
    })
}

/// F3a Task 8: the slot menu of card `slot` (the model's list, drawn as the bulk menus are).
fn slot_menu(
    resp: &egui::Response,
    model: &AppModel,
    v: &SetView,
    slot: SlotKind,
    acts: &mut Vec<Act>,
) {
    // The XPMF maps have no slot commands, lock or borrow: no menu.
    if slot.slot_number().is_none() {
        return;
    }
    resp.context_menu(|ui| {
        if let Some(a) = bulk_menu(ui, v.slot_label(slot, ""), &model.slot_menu(&v.id, slot)) {
            acts.push(Act::Slot(a));
        }
    });
}

/// A click selects a texture, which replaces this view by its settings, so the second click of a double click
/// would land on something else. The selection therefore waits out the double-click delay; a double click
/// meanwhile expands the texture instead. The pending click is set `set`'s and lives only while its view is drawn
/// every pass: another set's view never sees it, and a pass without this view drops it (trace-debug #53).
fn select_or_expand(
    ui: &egui::Ui,
    model: &mut AppModel,
    set: &str,
    clicked: Option<String>,
    expand: Option<String>,
) {
    type Pending = (String, f64, u64);
    let id = egui::Id::unique(("set-editor-pending-select", set));
    let (now, pass) = (ui.input(|i| i.time), ui.ctx().cumulative_pass_nr());
    if let Some(k) = expand {
        ui.data_mut(|d| d.remove::<Pending>(id));
        model.expand(&k);
        return;
    }
    if let Some(k) = clicked {
        ui.data_mut(|d| d.insert_temp(id, (k, now, pass)));
    }
    let Some((k, at, seen)) = ui.data(|d| d.get_temp::<Pending>(id)) else {
        return;
    };
    let left = DOUBLE_CLICK_WAIT - (now - at);
    // A click left pending while this view was not drawn (another selection took over) is stale: drop it.
    if left <= 0.0 || seen + 1 < pass {
        ui.data_mut(|d| d.remove::<Pending>(id));
        if seen + 1 >= pass {
            model.select_item(&k);
        }
    } else {
        ui.data_mut(|d| d.insert_temp(id, (k, at, pass)));
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(left));
    }
}

/// `text` when `fits(text)`, else "…" and the longest end of `text` for which `fits` holds with the "…" (at least one
/// character, fitting or not): a file name keeps its suffix (ruling A2-1). `header::elide_middle` is the other cut.
pub fn front_cut(text: &str, fits: impl Fn(&str) -> bool) -> String {
    if fits(text) {
        return text.to_string();
    }
    let mut cut = String::new();
    // Every end, longest first; the last one is a single character.
    for (at, _) in text.char_indices().skip(1) {
        cut.clear();
        cut.push('…');
        cut.push_str(&text[at..]);
        if fits(&cut) {
            return cut;
        }
    }
    let last = text.chars().last().map(String::from).unwrap_or_default();
    format!("…{last}")
}

/// A format combo's width: its minimum `min`, plus the free room up to what the painted value `want`s.
pub(super) fn combo_w(free: f32, want: f32, min: f32) -> f32 {
    min + free.max(0.0).min((want - min).max(0.0))
}

/// What a `ComboBox` button spends beside its text: the arrow and the padding (probe P2b: 26.0).
fn combo_chrome(ui: &egui::Ui) -> f32 {
    let s = ui.spacing();
    s.icon_width + s.icon_spacing + 2.0 * s.button_padding.x
}

/// `text`'s width in the `Button` font.
fn button_text_w(ui: &egui::Ui, text: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(
            text.to_string(),
            egui::TextStyle::Button.resolve(ui.style()),
            egui::Color32::PLACEHOLDER,
        )
        .size()
        .x
}

/// The width a combo wants to paint `text` in full: the text and the chrome.
fn combo_want(ui: &egui::Ui, text: &str) -> f32 {
    button_text_w(ui, text) + combo_chrome(ui)
}

fn button_fit_key(ui: &egui::Ui) -> egui::Id {
    egui::Id::unique((
        egui::TextStyle::Button.resolve(ui.style()),
        ui.pixels_per_point().to_bits(),
        combo_chrome(ui).to_bits(),
    ))
}

struct RoleLabel {
    role: RawMap,
    id: String,
    text: Arc<egui::RichText>,
    width: f32,
    source: String,
    fill: String,
}

fn role_label(ui: &egui::Ui, role: RawMap) -> Arc<RoleLabel> {
    let small = egui::TextStyle::Small.resolve(ui.style());
    let key = egui::Id::unique(("fill-role", role, &small, ui.pixels_per_point().to_bits()));
    super::paint_cached(ui.ctx(), key, || {
        let id = role.id();
        let text = if role == RawMap::Ao { "AO" } else { &id };
        let width = ui
            .painter()
            .layout_no_wrap(text.to_owned(), small, egui::Color32::PLACEHOLDER)
            .size()
            .x;
        Arc::new(RoleLabel {
            role,
            width,
            text: Arc::new(egui::RichText::new(text).small()),
            source: format!("{text} channel source"),
            fill: format!("{text} fill"),
            id,
        })
    })
}

/// A packed output's fill rows. They write `sdc.toml`, so they are disabled, with the reason on hover, while it is
/// read-only. The recipe has no control of its own (owner A1-1): the shader and the maps decide it, and a written
/// `packing.recipe` is cleared by its Problems row.
///
/// Ruling A2-3 (probe P2d): the channel lines are drawn in a child that is not bounded by the Format column, and the
/// cell takes only what the lines need beyond the Flags cell that follows (`flags_w`, last frame's Flags column,
/// which a packed row leaves empty): the lines run on across it, so the pickers keep their width at the floor. The
/// first frame has no `flags_w` (0.0): the cell takes the whole width for one frame. Returns the cell's width.
fn packed_controls(
    ui: &mut egui::Ui,
    model: &AppModel,
    item: &TextureItem,
    why: Option<&str>,
    flags_w: f32,
    acts: &mut Vec<Act>,
) -> f32 {
    let mut cell_w = 0.0;
    ui.add_enabled_ui(why.is_none(), |ui| {
        let top = ui.cursor().min;
        // Any rect larger than the window: the child's `min_rect` is what counts.
        let mut lines = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(egui::Rect::from_min_size(top, egui::vec2(2000.0, 2000.0)))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        fill_controls(&mut lines, model, &item.key, why, acts);
        let used = lines.min_rect();
        cell_w = (used.width() - SPACING - flags_w).max(0.0);
        ui.allocate_space(egui::vec2(cell_w, used.height()));
    })
    .response
    .on_disabled_hover_text(why.unwrap_or_default());
    cell_w
}

/// Ruling A2-4 (DY18 superseded; probe P2b): the width of a fill row's source picker, one for every channel line
/// (egui-ui-rules 3.5: `ComboBox::width` is only a minimum). `…_roughness.png` is 100.1 px of Button-font text, so
/// 126.1 px of combo (26.0 of arrow and padding); a pixel of fit margin and a few of rule 3.7's make 130. A longer
/// name is cut at the front (`front_cut`), full on hover.
const SOURCE_W: f32 = 130.0;

/// `channel_helpers` of `packed`, cached by the items' revision (review M1: an open popup draws every frame).
fn cached_helpers(
    ctx: &egui::Context,
    model: &AppModel,
    packed: &str,
) -> Vec<(String, String, engine::helper::HelperRole)> {
    type Slot = (
        String,
        u64,
        Vec<(String, String, engine::helper::HelperRole)>,
    );
    let id = egui::Id::unique("fill-source-helpers");
    let rev = model.items_rev();
    if let Some((k, r, v)) = ctx.data(|d| d.get_temp::<Slot>(id)) {
        if k == packed && r == rev {
            return v;
        }
    }
    let v = channel_helpers(model, packed);
    ctx.data_mut(|d| d.insert_temp(id, (packed.to_string(), rev, v.clone())));
    v
}

/// The source picker of one channel: "Source: <file>" or "Source: value", listing the set's greyscale helpers and
/// "Value". A pick other than the current source becomes `Act::ChannelHelper`.
fn source_picker(
    ui: &mut egui::Ui,
    model: &AppModel,
    packed: &str,
    label: &RoleLabel,
    current: Option<(&str, &str)>,
    why: Option<&str>,
    acts: &mut Vec<Act>,
) -> egui::Response {
    let channel = model
        .item(packed)
        .and_then(|i| i.job.as_ref()?.pack.as_ref())
        .and_then(|p| {
            channel_roles(p.kind)
                .iter()
                .position(|r| *r == Some(label.role))
        });
    let file = current.map_or("value", |(_, f)| f);
    // The AccessKit value keeps the whole name (A2-2); the painted text is the file alone, cut at the front so the
    // suffix shows when it does not fit (A2-1).
    let key = button_fit_key(ui).with(("fill-picker", file, current.is_some()));
    let (painted, shown) = super::paint_cached(ui.ctx(), key, || {
        #[cfg(feature = "test-hooks")]
        super::paint_counts::Work::Picker.built();
        let room = SOURCE_W - combo_chrome(ui) - 1.0;
        let text = if current.is_some() {
            front_cut(file, |s| button_text_w(ui, s) <= room)
        } else {
            file.to_string()
        };
        (
            Arc::new(egui::RichText::new(text)),
            Arc::new(format!("Source: {file}")),
        )
    });
    let h = ui.spacing().interact_size.y;
    let r = ui
        .allocate_ui(egui::vec2(SOURCE_W, h), |ui| {
            egui::ComboBox::from_id_salt(("fill-source", packed, &label.id))
                .width(SOURCE_W)
                .truncate()
                .selected_text(painted)
                .show_ui_036(ui, |ui| {
                    let now = current.map(|(k, _)| k);
                    let helpers = cached_helpers(ui.ctx(), model, packed);
                    // Review I1: only a helper's `helper.fills` can give the channel up; a file the engine does not
                    // report as a helper keeps it whatever the picker writes.
                    let stuck = current.filter(|(k, _)| !helpers.iter().any(|(h, ..)| h == k));
                    for (k, file, _) in helpers {
                        if ui
                            .selectable_label(now == Some(k.as_str()), &file)
                            .clicked()
                            && now != Some(k.as_str())
                        {
                            if let Some(c) = channel {
                                acts.push(Act::ChannelHelper(packed.into(), c as u8, Some(k)));
                            }
                        }
                    }
                    let why = stuck.map(|(_, f)| {
                        format!("{f} is not a helper; it feeds this channel by its name")
                    });
                    let value = ui
                        .add_enabled(
                            why.is_none(),
                            egui::Button::selectable(now.is_none(), "Value"),
                        )
                        .on_disabled_hover_text(why.clone().unwrap_or_default());
                    if value.clicked() && now.is_some() {
                        if let Some(c) = channel {
                            acts.push(Act::ChannelHelper(packed.into(), c as u8, None));
                        }
                    }
                })
                .response
        })
        .inner;
    super::helper_panel::named(&r, egui::Role::ComboBox, &label.source, Some(&shown));
    // Every picker has the whole name on hover; a disabled one says the name, then why it is off (A2-6).
    let name = if current.is_some() {
        file
    } else {
        "No file: the box sets this channel (0 to 255)."
    };
    r.ctx
        .accesskit_node_builder(r.id, |b| b.set_description(name));
    let r = r.on_hover_ui(|ui| {
        ui.set_max_width(ui.spacing().tooltip_width);
        ui.label(name);
    });
    match why {
        Some(why) => r.on_disabled_hover_ui(|ui| {
            ui.set_max_width(ui.spacing().tooltip_width);
            ui.label(format!("{name}\n{why}"));
        }),
        None => r,
    }
}

/// One row per channel that a fill can stand in for: its role, the source picker (Note DY18), and while the channel
/// has no source a value box with a reset.
fn fill_controls(
    ui: &mut egui::Ui,
    model: &AppModel,
    key: &str,
    why: Option<&str>,
    acts: &mut Vec<Act>,
) {
    let rows = fill_rows(model, key);
    // Ruling A2-3: every line starts with a role label of the width of the output's widest, so its pickers share an x.
    let lab_w = rows
        .iter()
        .map(|r| match r {
            FillRow::FromFile { role, .. } | FillRow::Value { role, .. } => {
                role_label(ui, *role).width
            }
        })
        .fold(0.0f32, f32::max)
        .ceil();
    // The label is drawn in a child of its own rect (a `scope_builder` advanced the cursor again, probe P2d).
    let role_cell = |ui: &mut egui::Ui, label: &RoleLabel| {
        let (r, _) = ui.allocate_exact_size(
            egui::vec2(lab_w, ui.spacing().interact_size.y),
            egui::Sense::hover(),
        );
        let mut cell = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(r)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        cell.label(label.text.clone());
    };
    for row in rows {
        match row {
            FillRow::FromFile {
                role,
                file,
                key: src,
                ..
            } => {
                ui.horizontal(|ui| {
                    let label = role_label(ui, role);
                    role_cell(ui, &label);
                    source_picker(ui, model, key, &label, Some((&src, &file)), why, acts);
                });
            }
            FillRow::Value {
                role,
                shown,
                own_key,
            } => {
                ui.horizontal(|ui| {
                    let label = role_label(ui, role);
                    role_cell(ui, &label);
                    source_picker(ui, model, key, &label, None, why, acts);
                    let mut v = shown;
                    // The lines run past the scope that `packed_controls` disables them under, so the box and Reset
                    // each answer for themselves with the reason (a scope's response covers the box, as a DragValue's
                    // `settled_drag` does not hand its own back).
                    let drag = ui.scope(|ui| {
                        super::number_input::settled_drag(
                            ui,
                            &mut v,
                            1.0,
                            0..=255,
                            &label.fill,
                            model.edit_busy(),
                            key,
                        )
                    });
                    if let Some(why) = why {
                        drag.response.on_disabled_hover_text(why);
                    }
                    if drag.inner {
                        acts.push(Act::Fill(key.to_string(), role, Some(v)));
                    }
                    if own_key
                        && ui
                            .small_button("Reset")
                            .on_hover_text("Remove this fill")
                            .on_disabled_hover_text(why.unwrap_or_default())
                            .clicked()
                    {
                        acts.push(Act::Fill(key.to_string(), role, None));
                    }
                });
            }
        }
    }
}

/// Outputs that fill no slot card get their controls here, under the cards: the complex `_m` (packed or loose)
/// its "Enable parallax" box (DECISIONS AH2), a packed output its fill rows.
fn unslotted_packed(ui: &mut egui::Ui, model: &AppModel, v: &SetView, acts: &mut Vec<Act>) {
    let why = model.toml_read_only_reason();
    for item in v
        .members
        .iter()
        .filter_map(|m| model.item(m))
        .filter(|i| slot_of(i).is_none())
    {
        let parallax = item
            .job
            .as_ref()
            .filter(|j| !field_hidden(&j.profile, "alpha.parallax"))
            .and_then(|j| field_view(model.target, j, "alpha.parallax"));
        let fills = item.kind == ItemKind::Packed && !fill_rows(model, &item.key).is_empty();
        if parallax.is_none() && !fills {
            continue;
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new(&item.name).strong());
        ui.add_enabled_ui(why.is_none(), |ui| {
            if let Some(f) = &parallax {
                ui.horizontal(|ui| {
                    let mut on = f.value.as_bool().unwrap_or(false);
                    let r = ui
                        .add_enabled(
                            f.lock.is_none(),
                            egui::Checkbox::new(&mut on, "Enable parallax"),
                        )
                        .on_hover_text(PARALLAX_HOVER)
                        .on_disabled_hover_text(f.lock.unwrap_or_default());
                    if r.changed() {
                        acts.push(Act::Parallax(item.key.clone(), on));
                    }
                    // A locked box the texture's own entry ticked (no source alpha) cannot be unticked: reset it.
                    if f.edited
                        && ui
                            .small_button("↺")
                            .on_hover_text("Use the inherited value")
                            .clicked()
                    {
                        acts.push(Act::ParallaxReset(item.key.clone()));
                    }
                });
            }
            if fills {
                fill_controls(ui, model, &item.key, why.as_deref(), acts)
            }
        })
        .response
        .on_disabled_hover_text(why.clone().unwrap_or_default());
    }
}

/// Note HH7: one row per helper the set shares (it belongs to another set, or to no set): its thumbnail and name,
/// outlined amber as a borrowed slot is, and a `✕` that stops sharing it. A click on the row selects the helper.
/// `why` is the read-only reason (empty when the project takes edits), which turns off only the ✕.
fn shared_rows(
    ui: &mut egui::Ui,
    model: &AppModel,
    v: &SetView,
    thumbs: &mut ThumbCache,
    why: &str,
    acts: &mut Vec<Act>,
) {
    for i in v.shared_helpers.iter().filter_map(|k| model.item(k)) {
        ui.add_space(4.0);
        // Keyed by the file, so the ✕'s id does not depend on what the rows above drew.
        // A click anywhere on the row selects the helper; the ✕ (registered later) keeps its own clicks.
        let row = egui::UiBuilder::new()
            .id_salt(("shared-helper", &i.key))
            .sense(egui::Sense::click());
        let row = ui.scope_builder(row, |ui| {
            // The editor's scroll bar floats over the right edge: keep the ✕ clear of it.
            let s = &ui.spacing().scroll;
            let gutter = s.bar_width + s.bar_outer_margin + ui.spacing().item_spacing.x;
            ui.set_max_width(ui.available_width() - gutter);
            egui::Sides::new().shrink_left().truncate().show(
                ui,
                |ui| {
                    if let Some(t) = thumbs.get(ui.ctx(), i) {
                        ui.add(egui::Image::new(&t).fit_to_exact_size(egui::vec2(20.0, 20.0)));
                    }
                    // truncate() shows the full name on hover; not selectable, so a click reaches the row.
                    ui.add(
                        egui::Label::new(format!("{} (shared)", i.name))
                            .truncate()
                            .selectable(false),
                    );
                },
                |ui| {
                    // Only the edit is off on a read-only project; selecting the helper is not an edit.
                    let x = ui
                        .add_enabled(why.is_empty(), egui::Button::new("✕").small())
                        .on_hover_text("Stop sharing: this set no longer reads the file")
                        .on_disabled_hover_text(why);
                    let label = format!("Stop sharing {}", i.name);
                    ui.ctx()
                        .accesskit_node_builder(x.id, |b| b.set_label(label));
                    x.clicked()
                },
            )
        });
        let (_, stop) = row.inner;
        if stop {
            acts.push(Act::StopSharing(v.id.clone(), i.key.clone()));
        } else if row.response.clicked() {
            acts.push(Act::Select(i.key.clone()));
        }
        ui.painter().rect_stroke(
            row.response.rect.expand(2.0),
            2.0,
            egui::Stroke::new(1.0, AMBER),
            egui::StrokeKind::Inside,
        );
    }
}

/// Note AO7: a row whose PGPatcher slot holds two members: a red line naming both files, then a line per member with
/// ✕, which takes it out of the set, its thumbnail and its file name (click selects, double-click expands).
fn clash_rows(
    ui: &mut egui::Ui,
    model: &AppModel,
    v: &SetView,
    cell: &SlotCell,
    thumbs: &mut ThumbCache,
    acts: &mut Vec<Act>,
) {
    let item_of = |fill: Option<&SlotFill>| model.item(fill?.item()?);
    let both: Vec<&TextureItem> = [Some(&cell.fill), cell.clash.and_then(|k| v.slots.get(&k))]
        .into_iter()
        .filter_map(item_of)
        .collect();
    let names: Vec<&str> = both.iter().map(|i| i.name.as_str()).collect();
    ui.colored_label(
        RED,
        format!(
            "{} both use slot {}: remove one",
            names.join(" and "),
            // A clash is two kinds of one PGPatcher slot, so it has a number.
            cell.kind.slot_number().unwrap_or_default()
        ),
    );
    for i in both {
        // The remove button first: the File cell is narrow, and the name truncates after it.
        ui.horizontal(|ui| {
            let x = ui
                .small_button("✕")
                .on_hover_text("Take it out of this set");
            let label = format!("Remove {}", i.name);
            ui.ctx()
                .accesskit_node_builder(x.id, |b| b.set_label(label));
            if x.clicked() {
                acts.push(Act::RemoveMember(v.id.clone(), i.key.clone()));
            }
            if let Some(t) = thumbs.get(ui.ctx(), i) {
                ui.add(egui::Image::new(&t).fit_to_exact_size(egui::vec2(20.0, 20.0)));
            }
            let r = ui
                .add(
                    egui::Label::new(egui::RichText::new(&i.name).small())
                        .truncate()
                        .sense(egui::Sense::click()),
                )
                .on_hover_text(&i.key);
            if r.double_clicked() {
                acts.push(Act::Expand(i.key.clone()));
            } else if r.clicked() {
                acts.push(Act::Select(i.key.clone()));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::model::sets::{views, SET_VIEWS_CLONES};
    use crate::model::{AppModel, Selection};
    use eframe::egui;
    use std::sync::Arc;
    use std::time::Duration;

    use super::{combo_w, front_cut};

    #[test]
    fn a_disabled_fill_reset_shows_its_reason() {
        use egui_kittest::kittest::{NodeT, Queryable};
        let d = tempfile::tempdir().unwrap();
        for name in ["rock.png", "rock_roughness.png"] {
            image::RgbImage::from_pixel(16, 16, image::Rgb([90, 90, 90]))
                .save(d.path().join(name))
                .unwrap();
        }
        let session = Arc::new(engine::session::Session::new().unwrap());
        let mut m = AppModel::new(Default::default(), None, session);
        m.open_folder(d.path().to_path_buf());
        assert!(m.wait_idle(Duration::from_secs(60)));
        let key = "rock_rmaos";
        m.set_pack_fill(key, super::RawMap::Ao, Some(99));
        assert!(m.wait_idle(Duration::from_secs(60)));
        // A read-only document drops its own fill keys. Exercise the row with the refusal and a saved fill together.
        let mut h = egui_kittest::Harness::builder()
            .with_context_setup(|ctx| {
                crate::fonts::install_system_fonts(ctx).expect("system fonts");
            })
            .build_ui(|ui| {
                super::packed_controls(
                    ui,
                    &m,
                    m.item(key).unwrap(),
                    Some("read-only fill"),
                    super::ALPHA_W,
                    &mut vec![],
                );
            });
        h.ctx.global_style_mut(|s| {
            s.interaction.tooltip_delay = 0.0;
            s.interaction.show_tooltips_only_when_still = false;
        });
        h.run();
        let reset = h.get_by_label("Reset");
        assert!(reset.accesskit_node().is_disabled());
        reset.hover();
        h.run_steps(4);
        h.get_by_label("read-only fill");
    }

    #[test]
    fn combo_w_takes_free_room_up_to_its_want() {
        assert_eq!(combo_w(0.0, 200.0, 96.0), 96.0);
        assert_eq!(combo_w(-5.0, 200.0, 96.0), 96.0, "no room: the minimum");
        assert_eq!(combo_w(50.0, 200.0, 96.0), 146.0, "part of the room");
        assert_eq!(combo_w(500.0, 200.0, 96.0), 200.0, "no wider than it wants");
        assert_eq!(combo_w(500.0, 80.0, 96.0), 96.0, "never under the minimum");
    }

    #[test]
    fn front_cut_keeps_the_end() {
        let at_most = |n: usize| move |s: &str| s.chars().count() <= n;
        assert_eq!(
            front_cut("rock01_roughness.png", at_most(40)),
            "rock01_roughness.png"
        );
        assert_eq!(
            front_cut("rock01_roughness.png", at_most(15)),
            "…_roughness.png"
        );
        assert_eq!(
            front_cut("rock01_roughness.png", at_most(16)),
            "…1_roughness.png",
            "the longest end that fits"
        );
        assert_eq!(front_cut("rock01_ao.png", at_most(13)), "rock01_ao.png");
        assert_eq!(
            front_cut("rock01_ao.png", at_most(1)),
            "…g",
            "at least one character"
        );
    }

    /// Trace fix 10: a set-editor frame (`set_editor_shown`, `set_editor::draw`, the input-name picker) reads the cached
    /// views and deep-clones none of them.
    #[test]
    fn a_set_editor_frame_does_not_clone_the_set_views() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("textures/landscape");
        std::fs::create_dir_all(&r).unwrap();
        for n in ["rock01.png", "rock01_normal.png"] {
            image::RgbImage::from_pixel(16, 16, image::Rgb([90, 90, 90]))
                .save(r.join(n))
                .unwrap();
        }
        let session = Arc::new(engine::session::Session::new().expect("a session"));
        let mut m = AppModel::new(crate::settings::GuiSettings::default(), None, session);
        m.open_folder(d.path().join("textures"));
        assert!(m.wait_idle(Duration::from_secs(60)));
        let key = views(&m).first().expect("a set").key.clone();
        m.selection = Selection::Set(key);
        let ctx = egui::Context::default();
        let mut state = crate::ui::State::default();
        let mut frame = |m: &mut AppModel| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 900.0),
                )),
                ..Default::default()
            };
            ctx.run_ui(input, |ui| crate::ui::draw_with(ui, m, &mut state))
                .drop_without_applying_deltas();
        };
        frame(&mut m);
        SET_VIEWS_CLONES.with(|c| c.set(0));
        frame(&mut m);
        frame(&mut m);
        assert_eq!(SET_VIEWS_CLONES.with(|c| c.get()), 0);
    }
}
