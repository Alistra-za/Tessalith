//! The editable Sets column (M3b spec §3.1): set pills in project order with badges, drag and drop
//! of textures onto sets (and onto an empty area for a new set), drag-to-reorder by the ≡ handle, and a context
//! menu. Drawing only: every change is a model call, made after drawing.
use super::columns::{bulk_menu, member, status_text, AMBER, RED};
use super::slot_commands::SlotCmdState;
use super::status::Status;
use super::thumbs::ThumbCache;
use crate::model::bulk::MenuAction;
use crate::model::helpers::{docked, Docked};
use crate::model::items::helper_target_text;
use crate::model::known_paths::TxstRecord;
use crate::model::labels::subtype_name;
use crate::model::pbr_export::export_blocks;
use crate::model::sets::{
    badge_text, fill_proposals, kind_text, shader_options, shared_cells, slot_row, DropChoice,
    DropOutcome, FillProposal, Missing, SetView, SlotFill, SlotKind,
};
use crate::model::AppModel;
use crate::sets_doc::TxstMap;
use eframe::egui;
use engine::codes::Warning;
use engine::shader::{Shader, ShaderOrigin};
use std::collections::{HashMap, HashSet};

/// Drag payload: a texture, by item key.
#[derive(Debug, Clone)]
pub struct DragItem(pub String);
/// Drag payload: a set, by id (reordering).
#[derive(Debug, Clone)]
pub struct DragSet(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pending {
    Choice {
        item: String,
        set: String,
        from: String,
    },
    /// `choice` is what the replace answers: `drop_replace(item, set, choice)`.
    Replace {
        item: String,
        set: String,
        existing: String,
        choice: DropChoice,
    },
    Refused(String),
    /// The rename dialog: the set, the name being typed, and why the last try was refused.
    Rename {
        id: String,
        text: String,
        error: Option<String>,
    },
    /// Item 18: Fill from names, one tick per proposal (all ticked to start; a blocked row stays unticked).
    FillNames {
        set: String,
        rows: Vec<FillProposal>,
        ticks: Vec<bool>,
    },
    /// Note CF5: Move to…, the set, the typed number or name, and the highlighted row.
    MoveTo {
        id: String,
        query: String,
        sel: usize,
    },
    /// Note HG: Map to texture set record…, the set, the typed query, the hits of `seen` (the query and the scan they
    /// were filtered for) as indexes into `known.records`, and the chosen record. Nothing is written until Map.
    MapRecord {
        id: String,
        query: String,
        hits: Vec<usize>,
        seen: Option<(String, usize)>,
        sel: Option<TxstRecord>,
    },
}

/// Item 17: files dropped from Explorer this frame, and where (`None` when the position is unknown).
#[derive(Debug, Clone)]
pub struct FileDrop {
    pub paths: Vec<std::path::PathBuf>,
    pub pos: Option<egui::Pos2>,
}

/// Item 17: the line the set editor shows under the slot cards after a file drop: set id, text, is an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotNote {
    pub set: String,
    pub text: String,
    pub error: bool,
}

#[derive(Debug, Default)]
pub struct DropState {
    pub pending: Option<Pending>,
    /// Item 17: a file drop not yet taken by a slot card this frame (`ui::draw_with_scan` opens its folder after).
    pub file: Option<FileDrop>,
    pub slot_note: Option<SlotNote>,
    /// R4: each set card's height when last laid out, by set id; a card off screen allocates this instead.
    pub card_h: HashMap<String, f32>,
    /// Review I1: frames a reveal has re-aimed at a card not yet laid out (capped by `REVEAL_TRIES`).
    pub reveal_tries: u8,
    /// The Sets scroll offset and view height last frame, for a reveal's jump (review I1).
    pub view: Option<(f32, f32)>,
    /// Note FM32: the column width `card_h` is for. A change re-flows every remembered height (`reflow_heights`).
    pub card_w: f32,
    /// Note FM32: the first card in view last frame and where its top sat in the content. The next frame keeps it
    /// there while the heights of the cards above it change (estimates become measured).
    pub anchor: Option<(String, f32)>,
    /// Note FM32: the key of the reveal's target when it was laid out last frame, so this frame scrolls to its measured
    /// rect instead of jumping to the offset the heights give (the mean moves as cards are measured, and would re-aim
    /// each frame). Another target, or a frame with no reveal, takes the jump path again.
    pub reveal_seen: Option<String>,
    /// Note FM32: the widget with the keyboard focus when the last frame's list ended. A stand-in that holds the focus
    /// and was not the holder then took it this frame (Tab), as opposed to a focused card the wheel scrolled away.
    pub focus: Option<egui::Id>,
}

/// Review I1: a reveal re-aims at most this many frames, so a key that names no visible card cannot loop.
const REVEAL_TRIES: u8 = 8;

/// R4: the height of a card never laid out, until any card has been measured (then their mean).
const CARD_EST: f32 = 110.0;

/// Note FM32: the rows `v`'s chips wrap into at column width `width` (`frame_x` is the card's side margins, `gap` the
/// chip spacing). A count from the set's slot row, its missing entries and its extra members, not a layout.
// shortcut: a helper docked on a chip still counts as an extra member here, so an unmeasured card may be estimated
// a row too tall (drawing corrects it, FM32); subtract the docked helpers if a scroll jump shows.
fn chip_rows(v: &SetView, width: f32, frame_x: f32, gap: f32) -> usize {
    let chips = shared_cells(v, slot_row(v.shader.as_ref(), &v.slots)).len()
        + v.missing.len()
        + v.members.len().saturating_sub(v.slots.len());
    let per_row = ((width - frame_x + gap) / (CHIP + gap)).floor().max(1.0) as usize;
    chips.div_ceil(per_row)
}

/// Note FM32: a new column width moves every remembered card height by the chip rows it adds or removes, so a card off
/// screen is no longer the height of the old width (the scroll bar length stays right without drawing it). A card not
/// in `views` any more is forgotten. The estimate leaves out the name row, which wraps below the pill when narrow;
/// drawing the card measures it exactly.
fn reflow_heights(
    st: &mut DropState,
    views: &[SetView],
    width: f32,
    gap: egui::Vec2,
    frame_x: f32,
) {
    let old = std::mem::replace(&mut st.card_w, width);
    if old <= 0.0 || (old - width).abs() < 0.5 {
        return;
    }
    let by_id: HashMap<&str, &SetView> = views.iter().map(|v| (v.id.as_str(), v)).collect();
    st.card_h.retain(|id, h| {
        let Some(v) = by_id.get(id.as_str()) else {
            return false;
        };
        let rows = |w: f32| chip_rows(v, w, frame_x, gap.x) as f32;
        *h = (*h + (rows(width) - rows(old)) * (CHIP + gap.y)).max(0.0);
        true
    });
}

/// The warning tooltip shows this many lines, then "… and K more".
const TIP_LINES: usize = 10;

enum Act {
    Select(String),
    /// The warning badge: open Problems filtered to this set key.
    Problems(String),
    SelectItem(String),
    Expand(String),
    Drop(String, String),
    NewSet(String),
    Reorder(String, usize),
    OpenMoveTo(String),
    /// Note HG: open the texture set record picker on a set.
    MapRecord(String),
    /// Note HG: remove a set's record mapping.
    UnmapRecord(String),
    OpenRename(String),
    Delete(String, usize),
    /// Item 18: open Fill from names on a set.
    FillNames(String),
    RemoveMember(String, String),
    /// Note DU1: the "?" chip's Remove from set.
    RemoveMissing(String, Missing),
    /// The Shader dropdown: `None` is Auto.
    Shader(String, Option<Shader>),
    /// A pill click with its modifiers (F3a D1).
    Click(String, bool, bool),
    RightClick(String),
    Bulk(MenuAction),
    /// F3a Task 8: a member's slot menu pick.
    Slot(MenuAction),
}

/// The card's heading is one row only when the name keeps this much beside the pill and badges; otherwise the pill
/// and badges go under the name (Note FI13).
const NAME_MIN: f32 = 80.0;
/// The pill frame's horizontal margin (`status_pill`) and the status dot's width (`status::dot`, 8 px).
const PILL_PAD_X: f32 = 6.0;
const STATUS_DOT: f32 = 8.0;
/// The size of a member thumbnail (`columns::member`), which an empty chip matches.
const CHIP: f32 = 32.0;

/// A chip's letter (A3): the slot's suffix in capitals, "D" for the diffuse.
fn chip_letter(v: &SetView, k: SlotKind) -> String {
    match k.suffix() {
        "" => "D".to_string(),
        _ if k == SlotKind::Rmaos => v.slot_text(k).to_string(),
        s => s.to_uppercase(),
    }
}

/// An empty slot (or a missing entry, outlined): a dim box with the letter. K11: named by the letter.
fn letter_box(ui: &mut egui::Ui, letter: &str, outline: Option<egui::Color32>) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(egui::Vec2::splat(CHIP), egui::Sense::hover());
    let vis = ui.visuals();
    let stroke = match outline {
        Some(c) => egui::Stroke::new(2.0, c),
        None => vis.widgets.noninteractive.bg_stroke,
    };
    let p = ui.painter();
    p.rect_filled(rect, 2.0, vis.extreme_bg_color);
    p.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::TextStyle::Small.resolve(ui.style()),
        vis.weak_text_color(),
    );
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, letter));
    r
}

/// The letter in a filled chip's bottom-left corner, on a dark backing so it reads over any thumbnail.
fn paint_letter(ui: &egui::Ui, rect: egui::Rect, letter: &str) {
    let font = egui::TextStyle::Small.resolve(ui.style());
    let g = ui
        .painter()
        .layout_no_wrap(letter.to_string(), font, egui::Color32::WHITE);
    let at = rect.left_bottom() + egui::vec2(1.0, -1.0 - g.size().y);
    ui.painter().rect_filled(
        egui::Rect::from_min_size(at, g.size()).expand(1.0),
        2.0,
        egui::Color32::from_black_alpha(170),
    );
    ui.painter().galley(at, g, egui::Color32::WHITE);
}

/// The pill's hover (R15): the long badge, then one line each for blocked, owns, missing entries and recipes.
fn pill_hover(model: &AppModel, v: &SetView, blocked: bool) -> String {
    let mut lines = vec![badge_text(model, v).to_string()];
    if blocked {
        lines.push("blocked".to_string());
    }
    lines.extend(v.owns.iter().map(|s| format!("owns {}", v.slot_text(*s))));
    lines.extend(
        v.missing
            .iter()
            .map(|m| format!("no file matches {}", m.text())),
    );
    for r in &v.recipes {
        let mut l = format!("{}: {}", r.subtype.id(), status_text(r.status).0);
        if let Some(reason) = &r.reason {
            l.push_str(&format!(" ({reason})"));
        }
        lines.push(l);
    }
    lines.join("\n")
}

/// Task 1.6's "⚠ n" button: the set's and its members' warnings on hover; a click opens Problems on the set.
fn warn_button(ui: &mut egui::Ui, v: &SetView, rows: &[(&str, &Warning)], acts: &mut Vec<Act>) {
    if rows.is_empty() {
        return;
    }
    let warn = ui
        .add(egui::Button::new(
            egui::RichText::new(warn_text(rows.len()))
                .small()
                .color(AMBER),
        ))
        .on_hover_text(warning_tooltip(rows));
    if warn.clicked() {
        acts.push(Act::Problems(v.key.clone()));
    }
}

/// The artist checks' "ⓘ n" badge (DECISIONS Note CL): `n` counts the Warning-level results, in the weak text colour
/// because a check never changes "ready" (owner answer 1); the hover lists every result, Info ones included (owner
/// answer 2); a click opens Problems on the set (ui-rules 1.3).
fn check_badge_text(checks: &[(&str, &Warning)]) -> (usize, String) {
    let n = checks
        .iter()
        .filter(|(_, w)| super::counts_toward_badge(w))
        .count();
    (
        n,
        if n == 0 {
            "ⓘ".into()
        } else {
            format!("ⓘ {n}")
        },
    )
}

/// The width of `text` in the small font, as a button or pill would paint it (a few short strings per card per frame).
fn small_width(ui: &egui::Ui, text: &str) -> f32 {
    text_width(ui, text, &egui::TextStyle::Small.resolve(ui.style()))
}

fn text_width(ui: &egui::Ui, text: &str, font: &egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::PLACEHOLDER)
        .size()
        .x
}

/// The "⚠ n" button's text, shared by the button and the heading's width measure.
fn warn_text(n: usize) -> String {
    format!("⚠ {n}")
}

fn check_badge(ui: &mut egui::Ui, v: &SetView, checks: &[(&str, &Warning)], acts: &mut Vec<Act>) {
    if checks.is_empty() {
        return;
    }
    let (n, text) = check_badge_text(checks);
    let weak = ui.visuals().weak_text_color();
    let r = ui.add(egui::Button::new(
        egui::RichText::new(text).small().color(weak),
    ));
    r.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            if n == 0 {
                "Check results: info only".to_string()
            } else {
                format!("Check results: {n}")
            },
        )
    });
    let r = r.on_hover_ui(|ui| {
        ui.label(warning_tooltip(checks));
    });
    if r.clicked() {
        acts.push(Act::Problems(v.key.clone()));
    }
}

/// The status pill: the status dot and the short kind (cut on a narrow card), in a rounded frame, the long text on
/// hover. The caller works out `s` from one `flagged` fetch per frame (Task 6).
fn status_pill(ui: &mut egui::Ui, model: &AppModel, v: &SetView, s: Status) {
    use super::status::dot;
    egui::Frame::new()
        .corner_radius(8.0)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(egui::Margin::symmetric(PILL_PAD_X as i8, 1))
        .show(ui, |ui| {
            let kind =
                egui::Label::new(egui::RichText::new(kind_text(model, v)).small()).truncate();
            // In the card row's right-to-left layout the first widget lands rightmost: the dot stays left.
            if ui.layout().prefer_right_to_left() {
                ui.add(kind);
                dot(ui, s);
            } else {
                dot(ui, s);
                ui.add(kind);
            }
        })
        .response
        .on_hover_text(pill_hover(model, v, s == Status::Blocks));
}

/// A member's thumbnail on the card with today's behaviours: click selects, double-click expands, a slot member has
/// the slot menu, and `drag` offers it for dragging; `note` is a line added to its hover. `None` when the item is
/// not in the model.
#[allow(clippy::too_many_arguments)]
fn member_chip(
    ui: &mut egui::Ui,
    model: &AppModel,
    thumbs: &mut ThumbCache,
    v: &SetView,
    mk: &str,
    kind: Option<SlotKind>,
    drag: bool,
    note: Option<&str>,
    sel_items: &HashSet<String>,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
) -> Option<egui::Response> {
    let item = model.item(mk)?;
    let (locked, pgp) = chip_locks(v, kind);
    let r = member(ui, item, sel_items.contains(mk), locked, pgp, thumbs, note);
    member_acts(model, v, mk, kind, drag, &r, acts, member_menu);
    Some(r)
}

/// Whether a chip shows the slot 🔒 (top right) and the PGPatcher 🔒 (top left).
fn chip_locks(v: &SetView, kind: Option<SlotKind>) -> (bool, bool) {
    let n = kind.and_then(SlotKind::slot_number);
    let locked = n.is_some_and(|n| v.locks.contains(&n));
    let pgp = kind
        .and_then(SlotKind::game_slot)
        .is_some_and(|n| crate::pgp::prefix::pgp_locked(&v.settings, n));
    (locked, pgp)
}

/// The side of a helper badge on a chip's top corner (Note HN).
const BADGE: f32 = 12.0;

/// What one cell of a chip's dock holds: the i-th helper's badge, or `+K` for K helpers not drawn.
#[derive(Debug, PartialEq, Eq)]
enum Cell {
    Badge(usize),
    More(usize),
}

/// Note HN: the dock's cells for `n` helpers on a chip: position 0 is the top right cell, 1 the top left. A 🔒 keeps
/// its corner, so only free cells are used; the last free cell folds the helpers it cannot show into one `+K`.
fn dock_cells(locked: bool, pgp: bool, n: usize) -> Vec<(usize, Cell)> {
    let free: Vec<usize> = [(0, locked), (1, pgp)]
        .into_iter()
        .filter(|(_, taken)| !taken)
        .map(|(p, _)| p)
        .collect();
    match (free.as_slice(), n) {
        (_, 0) | ([], _) => vec![],
        ([p, ..], 1) => vec![(*p, Cell::Badge(0))],
        ([p], n) => vec![(*p, Cell::More(n))],
        ([p, q], 2) => vec![(*p, Cell::Badge(0)), (*q, Cell::Badge(1))],
        ([p, q], n) => vec![(*p, Cell::Badge(0)), (*q, Cell::More(n - 1))],
        _ => vec![],
    }
}

/// A dock cell's rectangle on a chip.
fn cell_rect(chip: egui::Rect, pos: usize) -> egui::Rect {
    let min = if pos == 0 {
        chip.right_top() - egui::vec2(BADGE, 0.0)
    } else {
        chip.left_top()
    };
    egui::Rect::from_min_size(min, egui::Vec2::splat(BADGE))
}

/// A member chip's menu, drag payload and clicks: click selects, double-click expands, a slot member has the slot
/// menu, and `drag` offers it for dragging and a "Remove from set" entry.
#[allow(clippy::too_many_arguments)]
fn member_acts(
    model: &AppModel,
    v: &SetView,
    mk: &str,
    kind: Option<SlotKind>,
    drag: bool,
    r: &egui::Response,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
) {
    // Note DU5: an own member (`drag`) can leave the set from its chip; a borrowed one has the slot's Clear.
    if kind.is_some() || drag {
        *member_menu |= r.secondary_clicked();
        let open = r.context_menu(|ui| {
            if let Some(k) = kind {
                let entries = model.slot_menu(&v.id, k);
                // The XPMF maps' slot menu is empty: the chip keeps only "Remove from set".
                if !entries.is_empty() {
                    let title = match k.game_slot() {
                        Some(n) => format!("Slot {n}: {}", v.slot_text(k)),
                        None => v.slot_text(k).to_string(),
                    };
                    if let Some(a) = bulk_menu(ui, &title, &entries) {
                        acts.push(Act::Slot(a));
                    }
                }
            }
            if drag {
                let off = model.sets_read_only_reason();
                let b = ui.add_enabled(off.is_none(), egui::Button::new("Remove from set"));
                let b = match &off {
                    Some(why) => b.on_disabled_hover_text(why.as_str()),
                    None => b,
                };
                if b.clicked() {
                    acts.push(Act::RemoveMember(v.id.clone(), mk.to_string()));
                    ui.close();
                }
            }
        });
        *member_menu |= open.is_some();
    }
    if drag {
        r.dnd_set_drag_payload(DragItem(mk.to_string()));
    }
    if r.double_clicked() {
        acts.push(Act::Expand(mk.to_string()));
    } else if r.clicked() {
        acts.push(Act::SelectItem(mk.to_string()));
    }
}

/// Note HN: the helpers that fill one chip, drawn in its top cells. A badge is the helper's thumbnail with the member
/// behaviours (click selects, double-click expands, drag, the member menu); `+K` is a hover-only count. A helper
/// with a badge is added to `drawn`, so the trailing row skips it.
#[allow(clippy::too_many_arguments)]
fn draw_dock<'a>(
    ui: &mut egui::Ui,
    model: &AppModel,
    thumbs: &mut ThumbCache,
    v: &SetView,
    chip: egui::Rect,
    entries: &[&'a Docked],
    locks: (bool, bool),
    sel_items: &HashSet<String>,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
    drawn: &mut HashSet<&'a str>,
) {
    for (pos, cell) in dock_cells(locks.0, locks.1, entries.len()) {
        let rect = cell_rect(chip, pos);
        match cell {
            Cell::Badge(i) => {
                let d = entries[i];
                let Some(item) = model.item(&d.helper) else {
                    continue;
                };
                let id = ui.id().with(("dock", &v.id, &d.helper, d.slot, d.channel));
                let r = ui.interact(rect, id, egui::Sense::click_and_drag());
                drawn.insert(d.helper.as_str());
                if ui.is_rect_visible(rect) {
                    match thumbs.get(ui.ctx(), item) {
                        Some(t) => egui::Image::new(&t)
                            .fit_to_exact_size(rect.size())
                            .paint_at(ui, rect),
                        None => {
                            ui.painter().rect_filled(
                                rect,
                                1.0,
                                ui.visuals().widgets.noninteractive.weak_bg_fill,
                            );
                        }
                    }
                    let stroke = if sel_items.contains(&d.helper) {
                        ui.visuals().selection.stroke
                    } else {
                        egui::Stroke::new(1.0, ui.visuals().panel_fill)
                    };
                    ui.painter()
                        .rect_stroke(rect, 1.0, stroke, egui::StrokeKind::Inside);
                }
                let name = format!("{}, fills {} {}", item.name, v.slot_text(d.slot), d.channel);
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
                member_acts(model, v, &d.helper, None, true, &r, acts, member_menu);
                let target = item.helper.as_ref().map(|h| helper_target_text(h));
                r.on_hover_text(match target {
                    Some(t) => format!("{}\n{t}", item.name),
                    None => item.name.clone(),
                });
            }
            Cell::More(k) => {
                let hidden = &entries[entries.len() - k..];
                let slot = v.slot_text(hidden[0].slot);
                let id = ui.id().with(("dock-more", &v.id, hidden[0].slot, pos));
                let r = ui.interact(rect, id, egui::Sense::hover());
                if ui.is_rect_visible(rect) {
                    let font = egui::TextStyle::Small.resolve(ui.style());
                    ui.painter()
                        .rect_filled(rect, 2.0, egui::Color32::from_black_alpha(170));
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("+{k}"),
                        font,
                        egui::Color32::WHITE,
                    );
                }
                let name = format!("{k} more helpers fill {slot}");
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
                let lines: Vec<String> = hidden
                    .iter()
                    .map(|d| {
                        let file = model.item(&d.helper).map_or(d.helper.as_str(), |i| &i.name);
                        format!("{file} → {} {}", v.slot_text(d.slot), d.channel)
                    })
                    .collect();
                r.on_hover_text(lines.join("\n"));
            }
        }
    }
}

/// The warning tooltip: `<file>: <code> <message>` per warning, at most `TIP_LINES`, then "… and K more".
fn warning_tooltip(rows: &[(&str, &Warning)]) -> String {
    let mut lines: Vec<String> = rows
        .iter()
        .take(TIP_LINES)
        .map(|(file, w)| format!("{file}: {} {}", w.code.as_str(), w.message))
        .collect();
    if rows.len() > TIP_LINES {
        lines.push(format!("… and {} more", rows.len() - TIP_LINES));
    }
    lines.join("\n")
}

/// Whether the sets document can be changed (P8), and the banner's reason when it cannot.
struct Editing<'a> {
    on: bool,
    why: &'a str,
    /// Editable and no filter on: sets can be reordered (R17).
    reorder: bool,
}

/// Why the ≡ handle and Move up / Move down are off while the Sets filter is on.
const CLEAR_TO_REORDER: &str = "Clear the filter to reorder sets, or use Move to…";

/// R17: the search box, the pills with their counts and the shader popup, under the heading.
fn filter_rows(ui: &mut egui::Ui, model: &mut AppModel) {
    ui.add(
        egui::TextEdit::singleline(&mut model.sets_filter.search)
            .hint_text("Search sets")
            .desired_width(f32::INFINITY),
    );
    let c = model.set_counts();
    ui.horizontal_wrapped(|ui| {
        let f = &mut model.sets_filter;
        for (on, text, hover) in [
            (
                &mut f.attention,
                format!("Not ready {}", c.attention),
                "Only sets that are not ready: a problem in the set or one of its textures",
            ),
            (
                &mut f.ready,
                format!("Ready {}", c.ready),
                "Only sets with no problem",
            ),
            (
                &mut f.no_json,
                format!("No JSON {}", c.no_json),
                "Only sets that write no JSON",
            ),
        ] {
            if ui
                .add(
                    egui::Button::selectable(*on, text)
                        .corner_radius(10)
                        .frame_when_inactive(true),
                )
                .on_hover_text(hover)
                .clicked()
            {
                *on = !*on;
            }
        }
        let text = if f.shader.is_none() {
            "Filter ▾"
        } else {
            "Filter (1) ▾"
        };
        // A9: a saved shader the Mode does not offer is listed too, so it is never invisible.
        let mut shaders = shader_options(model.target);
        if let Some(s) = f.shader.filter(|s| !shaders.contains(s)) {
            shaders.push(s);
        }
        crate::ui::path_picker::menu(ui, text, |ui| {
            if ui.selectable_label(f.shader.is_none(), "Any").clicked() {
                f.shader = None;
                ui.close();
            }
            for s in shaders {
                if ui
                    .selectable_label(f.shader == Some(s), s.label())
                    .clicked()
                {
                    f.shader = Some(s);
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("Only sets with this shader");
    });
}

/// Note FC13: one line when the filter hides the selected set (CD13), one when a set made under it is shown (CD16).
/// Returns whether its own **Clear filter** button is drawn, so the "No sets match" line adds no second one.
fn filter_strips(
    ui: &mut egui::Ui,
    model: &mut AppModel,
    views: &[crate::model::sets::SetView],
) -> bool {
    let mut button = false;
    if let Some(id) = model.selected_set_hidden_by_filter() {
        button = true;
        let text = format!("{id} is hidden by the Sets filter");
        let clear = egui::Sides::new()
            .shrink_left()
            .truncate()
            .show(
                ui,
                |ui| {
                    ui.add(egui::Label::new(&text).truncate())
                        .on_hover_text(&text);
                },
                |ui| ui.button("Clear filter").clicked(),
            )
            .1;
        if clear {
            model.clear_sets_filter();
        }
    }
    let new = model.new_sets_shown();
    let n = views.iter().filter(|v| new.contains(&v.id)).count();
    if n > 0 {
        let s = if n == 1 { "" } else { "s" };
        ui.weak(format!("{n} new set{s} shown"))
            .on_hover_text("A set made while a filter is on stays shown until the filter changes");
    }
    button
}

/// Note FM32: a set card's AccessKit node, the same for the drawn card and its stand-in: one stop named for the set,
/// its place in the order, and whether it is selected. The role is Button, as the card is click-sensing.
fn card_node(
    n: &mut egui::accesskit::Node,
    v: &SetView,
    (index, total): (usize, usize),
    selected: bool,
    rect: egui::Rect,
) {
    n.set_role(egui::accesskit::Role::Button);
    n.set_label(format!("Set {}, position {} of {total}", v.id, index + 1));
    n.set_selected(selected);
    n.add_action(egui::accesskit::Action::Focus);
    n.set_bounds(egui::accesskit::Rect {
        x0: rect.min.x.into(),
        y0: rect.min.y.into(),
        x1: rect.max.x.into(),
        y1: rect.max.y.into(),
    });
}

/// Note FM32 (R4): a card outside the clip rect allocates `size` and stands in with the card's widget id: a Tab stop
/// and an AccessKit node named for the set. Returns its rect and whether it took the focus this frame (`held` is the
/// holder last frame). K9: `allocate_space` takes one auto id, as the card's scope does, so later cards keep theirs.
fn stand_in(
    ui: &mut egui::Ui,
    id: egui::Id,
    size: egui::Vec2,
    held: Option<egui::Id>,
    v: &SetView,
    at: (usize, usize),
    selected: bool,
) -> (egui::Rect, bool) {
    let rect = ui.allocate_space(size).1;
    let layer = ui.layer_id();
    let ctx = ui.ctx();
    // Registered by hand: a `Response` per card cost 15% of a frame at 300 sets.
    let gained = ctx.memory_mut(|m| {
        m.interested_in_focus(id, layer);
        m.focused() == Some(id) && held != Some(id)
    });
    ctx.accesskit_node_builder(id, |n| card_node(n, v, at, selected, rect));
    (rect, gained)
}

pub fn draw(
    ui: &mut egui::Ui,
    model: &mut AppModel,
    thumbs: &mut ThumbCache,
    st: &mut DropState,
    cmds: &mut SlotCmdState,
) {
    ui.horizontal_wrapped(|ui| {
        super::columns::collapse_arrow(ui, model, crate::ui::Column::Sets);
        ui.heading("Sets");
        if let Some(s) = model.selection_summary(true) {
            ui.label(egui::RichText::new(s).strong());
        }
    });
    filter_rows(ui, model);
    ui.separator();
    // R17: while a filter is on, the handle, Move up / Move down and the insertion line are off (K19).
    let filtering = model.sets_filter_active();
    // One set per frame keeps the per-pill and per-member tests O(1) however many are selected.
    let stats = super::columns::column_stats(ui.ctx(), model);
    let (sel_sets, sel_items) = (&stats.sets, &stats.chosen);
    let off = model.sets_read_only_reason();
    let editable = off.is_none();
    let why = off.unwrap_or_default();
    let editing = Editing {
        on: editable,
        why: &why,
        reorder: editable && !filtering,
    };
    let views = crate::model::sets::views(model);
    let visible = model.visible_sets();
    let has_clear = filter_strips(ui, model, &views);
    if views.is_empty() && model.root.is_some() && !model.busy() {
        ui.weak("No sets.");
    } else if visible.is_empty() && filtering && !views.is_empty() {
        ui.label(format!("No sets match the filter ({} hidden)", views.len()));
        // One Clear filter in the column (ui-rules 8.4): the strip above already has it.
        if !has_clear && ui.button("Clear filter").clicked() {
            model.clear_sets_filter();
        }
    }
    let blocked: std::collections::HashSet<String> = export_blocks(model)
        .iter()
        .map(|i| i.set_id.clone())
        .collect();
    // One fetch per frame: each drawn card asks `set_ready_in`, not `problems()` (Task 6).
    let flagged = crate::model::bottom::flagged(model);
    let mut acts = vec![];
    // `dropped`: the set let go over a card this frame, with that card's index (R1).
    let (mut dropped, mut hovered) = (None, None);
    let ctx = ui.ctx().clone();
    // The set being dragged (a peek: the payload stays for the reorder).
    let dragged: Option<String> = egui::DragAndDrop::payload::<DragSet>(&ctx).map(|p| p.0.clone());
    let dragged_index = dragged
        .as_deref()
        .and_then(|d| views.iter().position(|v| v.id == d));
    // Laid-out cards by full index, for the insertion line.
    let mut rects: HashMap<usize, egui::Rect> = HashMap::new();
    let gap = ui.spacing().item_spacing;
    let frame_x = egui::Frame::group(ui.style()).total_margin().sum().x;
    reflow_heights(st, &views, ui.available_width(), gap, frame_x);
    let est = if st.card_h.is_empty() {
        CARD_EST
    } else {
        st.card_h.values().sum::<f32>() / st.card_h.len() as f32
    };
    let card_h = |id: &str| st.card_h.get(id).copied().unwrap_or(est);
    // A set the palette picked scrolls into view (item 19); `reveal_at` is its place in `visible`.
    let reveal = model.nav.reveal_set.take();
    let reveal_at = reveal.as_deref().and_then(|k| {
        visible
            .iter()
            .position(|&i| !views[i].key.is_empty() && views[i].key == k)
    });
    // Review I1: a target outside last frame's view jumps to the offset the known heights give. `scroll_to_rect` on
    // an estimated rect glided to the wrong card, and its relative target double-counted when aimed again. Note FM32:
    // the target lands at the top of the view, a little below it (the sum of the rounded allocations above it is not
    // quite the sum of the heights), so it is whole on screen whatever its true height; bottom-aligning an estimated
    // height left it cut off, and a second scroll moved it.
    let jump = reveal_at
        .filter(|_| st.reveal_seen != reveal)
        .and_then(|n| {
            let (off, view_h) = st.view?;
            let top: f32 = visible[..n]
                .iter()
                .map(|&i| card_h(&views[i].id) + gap.y)
                .sum();
            let h = card_h(&views[visible[n]].id);
            let outside = top + h <= off || top >= off + view_h;
            outside.then(|| (top - 2.0 * gap.y).max(0.0))
        });
    // Note FM32: the first card in view last frame keeps its place: when the heights above it changed since (the
    // mean of the measured ones moves, a resize re-flowed them), the offset moves by the same amount.
    let anchored = || {
        let ((id, was), (off, _)) = st.anchor.as_ref().zip(st.view)?;
        let n = visible.iter().position(|&i| views[i].id == *id)?;
        let now: f32 = visible[..n]
            .iter()
            .map(|&i| card_h(&views[i].id) + gap.y)
            .sum();
        ((now - was).abs() > 0.5).then(|| (off + now - was).max(0.0))
    };
    let target = jump.or_else(anchored);
    let mut resized = false;
    let mut reveal_drawn = false;
    // The first card in view (id, top in the content) and the off-screen card that took the focus, if any.
    let (mut anchor, mut focused) = (None::<(String, f32)>, None::<String>);
    let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
    if let Some(y) = target {
        area = area.vertical_scroll_offset(y);
    }
    let out = area.show(ui, |ui| {
        let view_top = ui.clip_rect().top();
        // `top` runs on the remembered heights, not on the laid-out rects: egui rounds each allocation, and the sum of
        // those roundings drifts from the heights over hundreds of cards.
        let mut top = 0.0;
        // `index` is the set's place among all sets: the reorder and Move up / Move down use it (K19).
        for (pos, &index) in visible.iter().enumerate() {
            let v = &views[index];
            let h = st.card_h.get(&v.id).copied().unwrap_or(est);
            let probe =
                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), h));
            let id = ui.id().with(("set-card", &v.key, &v.id));
            let selected = !v.key.is_empty() && sel_sets.contains(&v.key);
            let (rect, used) = if ui.is_rect_visible(probe) {
                let look = PillLook {
                    dragging: dragged.as_deref() == Some(v.id.as_str()),
                    selected,
                    bulk: sel_sets.len() >= 2,
                    id,
                };
                let out = pill(
                    ui,
                    model,
                    thumbs,
                    v,
                    (index, views.len()),
                    &editing,
                    if blocked.contains(&v.id) {
                        Status::Blocks
                    } else if !crate::model::bottom::set_ready_in(&flagged, v) {
                        Status::Warning
                    } else {
                        Status::Ready
                    },
                    look,
                    sel_items,
                    &mut acts,
                );
                resized |= (out.rect.height() - h).abs() > 0.5;
                st.card_h.insert(v.id.clone(), out.rect.height());
                if let Some(d) = out.dropped_set {
                    dropped = Some((d, index));
                }
                if out.hovered_by_set {
                    hovered = Some(index);
                }
                rects.insert(index, out.rect);
                (out.rect, out.rect.height())
            } else {
                // R4: a card outside the clip rect stands in for its last height. The focus it takes scrolls the real
                // card into view (Note FM32).
                let (rect, gained) = stand_in(
                    ui,
                    id,
                    probe.size(),
                    st.focus,
                    v,
                    (index, views.len()),
                    selected,
                );
                if gained {
                    if v.key.is_empty() {
                        ui.scroll_to_rect_animation(
                            rect,
                            None,
                            egui::style::ScrollAnimation::none(),
                        );
                    } else {
                        focused = Some(v.key.clone());
                    }
                }
                (rect, h)
            };
            if anchor.is_none() && rect.bottom() > view_top {
                anchor = Some((v.id.clone(), top));
            }
            top += used + gap.y;
            if reveal_at == Some(pos) {
                reveal_drawn = rects.contains_key(&index);
                // A target in or across last frame's view lands at once (Note FM25).
                if jump.is_none() {
                    ui.scroll_to_rect_animation(rect, None, egui::style::ScrollAnimation::none());
                }
            }
        }
        if let (Some(from), Some(to)) = (dragged_index, hovered) {
            if let Some((r, y)) = rects
                .get(&to)
                .and_then(|r| insertion_y(from, to, *r).map(|y| (*r, y)))
            {
                let accent = ui.visuals().selection.stroke.color;
                ui.painter().line_segment(
                    [egui::pos2(r.left(), y), egui::pos2(r.right(), y)],
                    egui::Stroke::new(3.0, accent),
                );
            }
        }
        if model.root.is_some() {
            new_set_area(ui, &editing, &mut acts);
        }
    });
    st.view = Some((out.state.offset.y, out.inner_rect.height()));
    st.anchor = anchor;
    st.reveal_seen = reveal.clone().filter(|_| reveal_drawn);
    let go_to = reveal.is_some();
    st.focus = ctx.memory(|m| m.focused());
    // Review I1: aim again next frame until the target is laid out with every height around it measured.
    if reveal_at.is_some() && !(reveal_drawn && !resized) && st.reveal_tries < REVEAL_TRIES {
        st.reveal_tries += 1;
        model.nav.reveal_set = reveal;
        ctx.request_repaint();
    } else if reveal.is_some() {
        st.reveal_tries = 0;
    }
    // Note FM32: Tab reached an off-screen card: the next frame scrolls it in and draws it, keeping the focus.
    // A Go to set in the same frame wins: the card keeps its focus, and the Tab does not pull the view off the target.
    if let Some(k) = focused.filter(|_| !go_to) {
        st.reveal_tries = 0;
        model.nav.reveal_set = Some(k);
        ctx.request_repaint();
    }
    // A measured height moved the cards below it: one more frame lays them out where they now are (settles).
    if resized {
        ctx.request_repaint();
    }
    let ids: Vec<&str> = views.iter().map(|v| v.id.as_str()).collect();
    if editing.reorder {
        let (dropped, target) = dropped.unzip();
        if let Some((id, to)) = reorder_target(dropped.as_deref(), target, &ids) {
            acts.push(Act::Reorder(id, to));
        }
    }
    for a in acts {
        match a {
            // Note FC8 (BN3): a new selection waits a frame for a focused field to commit.
            Act::Select(k) => super::after_commit(&ctx, model, move |m| m.select_set(&k)),
            Act::SelectItem(k) => super::after_commit(&ctx, model, move |m| m.select_item(&k)),
            Act::Click(k, ctrl, shift) => {
                super::after_commit(&ctx, model, move |m| m.click_set(&k, ctrl, shift))
            }
            Act::RightClick(k) => model.right_click_set(&k),
            Act::Bulk(a) => model.menu_picked(a),
            Act::Problems(k) => {
                model.open_problems(Some(crate::model::bottom::ProblemsFilter::Set(k)))
            }
            Act::Slot(a) => super::slot_commands::run(model, cmds, a),
            Act::Expand(k) => super::after_commit(&ctx, model, move |m| m.expand(&k)),
            Act::Drop(item, set) => match model.drop_on_set(&item, &set) {
                DropOutcome::Joined => {}
                DropOutcome::AskChoice { from } => {
                    st.pending = Some(Pending::Choice { item, set, from })
                }
                DropOutcome::AskReplace { existing } => {
                    st.pending = Some(Pending::Replace {
                        item,
                        set,
                        existing,
                        choice: DropChoice::Move,
                    })
                }
                DropOutcome::Refused(why) => st.pending = Some(Pending::Refused(why)),
            },
            Act::NewSet(item) => {
                // Nothing was done (a packed map, a locked slot): say why.
                if let Err(why) = model.drop_new_set(&item) {
                    st.pending = Some(Pending::Refused(why));
                }
            }
            Act::Reorder(id, to) => apply_reorder(model, &id, to),
            Act::OpenMoveTo(id) => {
                st.pending = Some(Pending::MoveTo {
                    id,
                    query: String::new(),
                    sel: 0,
                })
            }
            Act::MapRecord(id) => {
                // A mapped set opens on its current record, when the scan still has it.
                let sel = crate::model::sets::views(model)
                    .iter()
                    .find(|v| engine::sets::same_id(&v.id, &id))
                    .and_then(|v| v.txst.as_ref())
                    .and_then(|t| model.record_of(t))
                    .cloned();
                st.pending = Some(Pending::MapRecord {
                    id,
                    query: String::new(),
                    hits: vec![],
                    seen: None,
                    sel,
                });
            }
            Act::UnmapRecord(id) => {
                if let Err(why) = model.unmap_record(&id) {
                    st.pending = Some(Pending::Refused(why));
                }
            }
            Act::OpenRename(id) => {
                st.pending = Some(Pending::Rename {
                    text: id.clone(),
                    id,
                    error: None,
                })
            }
            // Note FC5: no question; one undo step and a toast with Undo.
            Act::Delete(id, members) => model.delete_set_with_toast(&id, members),
            Act::FillNames(set) => {
                let rows = fill_proposals(model, &set);
                st.pending = Some(if rows.is_empty() {
                    Pending::Refused(format!(
                        "No other file in {set}'s folder has its base name and a suffix Tessalith knows."
                    ))
                } else {
                    Pending::FillNames {
                        set,
                        ticks: rows.iter().map(|r| r.blocked.is_none()).collect(),
                        rows,
                    }
                });
            }
            Act::RemoveMember(id, item) => {
                if let Err(why) = model.remove_member(&id, &item) {
                    st.pending = Some(Pending::Refused(why));
                }
            }
            Act::RemoveMissing(id, m) => {
                if let Err(why) = model.remove_missing(&id, &m) {
                    st.pending = Some(Pending::Refused(why));
                }
            }
            Act::Shader(id, shader) => model.set_shader(&id, shader),
        }
    }
}

/// K5: every reorder (drag, Move up / Move down, Move to…): the moved set scrolls into view (CF5).
fn apply_reorder(model: &mut AppModel, id: &str, to: usize) {
    let key = crate::model::sets::views(model)
        .iter()
        .find(|v| engine::sets::same_id(&v.id, id))
        .map(|v| v.key.clone());
    if model.reorder_set(id, to) {
        model.nav.reveal_set = key.filter(|k| !k.is_empty());
    }
}

/// CF4: the card's place in the full order, a body-size number in a slot as wide as `total`'s, so names line up.
fn position_label(ui: &mut egui::Ui, n: usize, total: usize) {
    let font = egui::FontId::monospace(egui::TextStyle::Body.resolve(ui.style()).size);
    // The ≡ handle's colour (a sensing Label): 5.3:1 on the card; `text_color()` would be about 3.3:1.
    let color = ui.visuals().widgets.inactive.fg_stroke.color;
    let (w, galley) = ui.fonts_mut(|f| {
        let w = f
            .layout_no_wrap(total.to_string(), font.clone(), color)
            .size()
            .x;
        (w, f.layout_no_wrap(n.to_string(), font, color))
    });
    // A fixed slot painted right-aligned: no child Ui, so the header row's width never depends on the number.
    let (rect, r) = ui.allocate_exact_size(egui::vec2(w, galley.size().y), egui::Sense::hover());
    let at = egui::pos2(rect.right() - galley.size().x, rect.top());
    ui.painter().galley(at, galley, color);
    // K6: the number carries the name; the card's own response has none.
    r.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Label,
            true,
            format!("Position {n} of {total}"),
        )
    });
}

/// Move to…'s list: indices into `views`, in the full order (CF5).
#[derive(Debug, PartialEq, Eq)]
struct MoveHits {
    rows: Vec<usize>,
    /// The query is a number outside 1..=N.
    out_of_range: bool,
}

/// Blank: every set. All digits: that position only, never a name (I1), and no row when it is outside 1..=N.
/// Anything else: the sets whose folded id or label holds the folded query.
fn move_to_hits(views: &[SetView], query: &str) -> MoveHits {
    let q = engine::sets::fold_id(query.trim());
    if q.is_empty() || !q.chars().all(|c| c.is_ascii_digit()) {
        let rows = (0..views.len())
            .filter(|&i| engine::sets::fold_id(&views[i].id).contains(&q))
            .collect();
        return MoveHits {
            rows,
            out_of_range: false,
        };
    }
    // A number too big for usize is out of range too.
    let at = q
        .parse::<usize>()
        .ok()
        .filter(|n| (1..=views.len()).contains(n))
        .map(|n| n - 1);
    MoveHits {
        rows: at.into_iter().collect(),
        out_of_range: at.is_none(),
    }
}

/// The dashed area at the bottom: a texture dropped here starts a new set.
fn new_set_area(ui: &mut egui::Ui, editing: &Editing, acts: &mut Vec<Act>) {
    let (zone, payload) = item_zone(
        ui,
        editing.on,
        egui::Frame::group(ui.style()).corner_radius(12.0),
        |ui| {
            ui.set_width(ui.available_width());
            // A one-off hint: it wraps rather than elides (the column's style truncates).
            ui.add(
                egui::Label::new(egui::RichText::new("Drop here to start a new set").color(AMBER))
                    .wrap(),
            );
        },
    );
    if !editing.on {
        zone.on_hover_text(editing.why);
    }
    if let Some(p) = payload {
        if editing.on {
            acts.push(Act::NewSet(p.0.clone()));
        }
    }
}

/// A frame that takes dropped textures. `egui::Ui::dnd_drop_zone` takes whatever payload is in flight when it
/// is released (even one of another type), so while a set is dragged for reordering, or when editing is off, a
/// plain frame is drawn instead and the set payload survives for the reorder.
fn item_zone(
    ui: &mut egui::Ui,
    editing: bool,
    frame: egui::Frame,
    add: impl FnOnce(&mut egui::Ui),
) -> (egui::Response, Option<std::sync::Arc<DragItem>>) {
    if editing && !egui::DragAndDrop::has_payload_of_type::<DragSet>(ui.ctx()) {
        let (zone, p) = ui.dnd_drop_zone::<DragItem, ()>(frame, add);
        (zone.response, p)
    } else {
        (frame.show(ui, add).response, None)
    }
}

/// Where a dragged set goes: `released` is the set let go this frame, `hovered` the index of the pill it was let go
/// on. Nothing happens when there is no target or the set is dropped on itself.
fn reorder_target(
    released: Option<&str>,
    hovered: Option<usize>,
    ids: &[&str],
) -> Option<(String, usize)> {
    let (id, to) = (released?, hovered?);
    (*ids.get(to)? != id).then(|| (id.to_string(), to))
}

/// What a pill reports back: the set dropped on it this frame (R1: detected on the target, since the source card
/// may not be laid out when the drag ends), and whether a dragged set is over it.
struct PillOut {
    dropped_set: Option<String>,
    hovered_by_set: bool,
    rect: egui::Rect,
}

/// How a pill is drawn this frame.
#[derive(Clone, Copy)]
struct PillLook {
    dragging: bool,
    selected: bool,
    /// Two or more sets are selected: the pills of selected ones show the bulk menu.
    bulk: bool,
    /// The card's widget id: the stand-in drawn while it is off screen has the same one, so focus survives the swap.
    id: egui::Id,
}

/// The y of the insertion line (None over the dragged pill itself, where nothing moves) when set `from` is dragged over the pill at `to` (whose rect is `r`): moving
/// down lands after that pill, moving up before it (the same rule as `reorder_set`).
fn insertion_y(from: usize, to: usize, r: egui::Rect) -> Option<f32> {
    match to.cmp(&from) {
        std::cmp::Ordering::Greater => Some(r.bottom()),
        std::cmp::Ordering::Less => Some(r.top()),
        std::cmp::Ordering::Equal => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn pill(
    ui: &mut egui::Ui,
    model: &AppModel,
    thumbs: &mut ThumbCache,
    v: &SetView,
    (index, total): (usize, usize),
    editing: &Editing,
    status: Status,
    look: PillLook,
    sel_items: &HashSet<String>,
    acts: &mut Vec<Act>,
) -> PillOut {
    #[cfg(feature = "test-hooks")]
    model.set_cards_drawn.set(model.set_cards_drawn.get() + 1);
    // A member's slot menu, not the pill's, answers a right-click on that member.
    let mut member_menu = false;
    let selected = look.selected;
    let accent = ui.visuals().selection.stroke.color;
    let stroke = if look.dragging {
        egui::Stroke::new(2.0, accent)
    } else if selected {
        ui.visuals().selection.stroke
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke
    };
    // A5: the open set and the selected sets are filled with the selection colour (`selected_sets` holds the open one).
    let fill = if selected {
        ui.visuals().selection.bg_fill.gamma_multiply(0.35)
    } else {
        egui::Color32::TRANSPARENT
    };
    // The drop zone repaints its own frame's fill and stroke, so the card's look is an inner frame.
    let card = egui::Frame::group(ui.style())
        .corner_radius(12.0)
        .stroke(stroke)
        .fill(fill);
    let builder = egui::UiBuilder::new()
        .id(look.id)
        .sense(egui::Sense::click());
    let scoped = ui.scope_builder(builder, |ui| {
        if look.dragging {
            ui.set_opacity(0.5);
        }
        let (zone, dropped) = item_zone(
            ui,
            editing.on,
            egui::Frame::NONE.corner_radius(12.0),
            |ui| {
                card.show(ui, |ui| {
                    ui.style_mut().interaction.selectable_labels = false;
                    ui.set_width(ui.available_width());
                    let rows: Vec<(&str, &Warning)> = v
                        .members
                        .iter()
                        .filter_map(|k| model.item(k))
                        .flat_map(|i| {
                            i.warnings
                                .iter()
                                .filter(|w| super::counts_toward_badge(w))
                                .map(move |w| (i.name.as_str(), w))
                        })
                        .chain(
                            v.set_warnings
                                .iter()
                                .filter(|w| super::counts_toward_badge(w))
                                .map(|w| (v.id.as_str(), w)),
                        )
                        .collect();
                    // Kept apart from `rows`: the "⚠ n" never counts check results. Undecided members are left out,
                    // as in Problems (review I1).
                    let checks: Vec<(&str, &Warning)> = v
                        .members
                        .iter()
                        .filter_map(|k| model.item(k))
                        .filter(|i| i.status != crate::model::items::ItemStatus::NeedsDecision)
                        .flat_map(|i| {
                            model
                                .check_warnings(&i.key)
                                .iter()
                                .map(move |w| (i.name.as_str(), w))
                        })
                        .chain(
                            model
                                .set_check_warnings(&v.key)
                                .iter()
                                .map(|w| (v.id.as_str(), w)),
                        )
                        .collect();
                    // One row only when the name keeps NAME_MIN px beside the pill and badges (Note FI13).
                    let sp = ui.spacing().item_spacing.x;
                    let btn = 2.0 * ui.spacing().button_padding.x + sp;
                    let body = egui::TextStyle::Body.resolve(ui.style());
                    // position slot (as wide as the last number), the ≡ handle, and the gaps around them
                    let lead =
                        text_width(ui, &total.to_string(), &egui::FontId::monospace(body.size))
                            + text_width(ui, "≡", &body)
                            + 2.0 * sp;
                    let stroke = ui.visuals().widgets.noninteractive.bg_stroke.width;
                    let pill = small_width(ui, kind_text(model, v))
                        + 2.0 * PILL_PAD_X
                        + 2.0 * stroke
                        + STATUS_DOT
                        + sp;
                    let mut need = lead + NAME_MIN + pill + sp;
                    if !rows.is_empty() {
                        need += small_width(ui, &warn_text(rows.len())) + btn;
                    }
                    if !checks.is_empty() {
                        need += small_width(ui, &check_badge_text(&checks).1) + btn;
                    }
                    let wide = ui.available_width() >= need;
                    let name = |ui: &mut egui::Ui| {
                        ui.add(egui::Label::new(egui::RichText::new(&v.id).strong()).truncate());
                    };
                    ui.horizontal(|ui| {
                        position_label(ui, index + 1, total);
                        let sense = if editing.reorder {
                            egui::Sense::drag()
                        } else {
                            egui::Sense::hover()
                        };
                        let handle = ui.add(egui::Label::new("≡").sense(sense));
                        if editing.reorder {
                            handle.dnd_set_drag_payload(DragSet(v.id.clone()));
                        } else if editing.on {
                            handle.on_hover_text(CLEAR_TO_REORDER);
                        }
                        if wide {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Right to left: reads pill, ⚠, ⓘ, as the narrow row does (review M1).
                                    check_badge(ui, v, &checks, acts);
                                    warn_button(ui, v, &rows, acts);
                                    status_pill(ui, model, v, status);
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        name,
                                    );
                                },
                            );
                        } else {
                            name(ui);
                        }
                    });
                    if !wide {
                        ui.horizontal_wrapped(|ui| {
                            status_pill(ui, model, v, status);
                            warn_button(ui, v, &rows, acts);
                            check_badge(ui, v, &checks, acts);
                        });
                    }
                    if let Some(map) = &v.txst {
                        record_line(ui, v, map, model.record_ignored(v));
                    }
                    if !v.key.is_empty() {
                        shader_combo(ui, model, v, editing, acts);
                    }
                    if v.key.is_empty() && v.created && v.missing.is_empty() {
                        ui.weak("Drop a texture here to start");
                    }
                    // Note HN: helpers dock on the chips they fill.
                    let dock = docked(model, v);
                    ui.horizontal_wrapped(|ui| {
                        let row = shared_cells(v, slot_row(v.shader.as_ref(), &v.slots));
                        let mut chipped: HashSet<&str> = HashSet::new();
                        let mut badged: HashSet<&str> = HashSet::new();
                        for cell in &row {
                            let k = cell.kind;
                            let letter = chip_letter(v, k);
                            let (mk, from) = match &cell.fill {
                                SlotFill::Own { item } => (item, None),
                                SlotFill::Borrowed { from, item } => (item, Some(from)),
                                SlotFill::Empty => {
                                    letter_box(ui, &letter, None)
                                        .on_hover_text(format!("{} (empty)", v.slot_text(k)));
                                    continue;
                                }
                            };
                            chipped.insert(mk);
                            let note = from.map(|f| format!("borrowed from {f}"));
                            // K13: a borrowed item belongs to another set: no drag payload from here.
                            let Some(r) = member_chip(
                                ui,
                                model,
                                thumbs,
                                v,
                                mk,
                                Some(k),
                                from.is_none(),
                                note.as_deref(),
                                sel_items,
                                acts,
                                &mut member_menu,
                            ) else {
                                letter_box(ui, &letter, None);
                                continue;
                            };
                            paint_letter(ui, r.rect, &letter);
                            if from.is_some() {
                                ui.painter().rect_stroke(
                                    r.rect,
                                    2.0,
                                    egui::Stroke::new(2.0, AMBER),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            let entries: Vec<&Docked> =
                                dock.iter().filter(|d| d.slot == k).collect();
                            let name = match model.item(mk) {
                                Some(item) if !entries.is_empty() => {
                                    let mut files: Vec<&str> = vec![];
                                    for d in &entries {
                                        let f =
                                            model.item(&d.helper).map_or(&*d.helper, |i| &*i.name);
                                        if !files.contains(&f) {
                                            files.push(f);
                                        }
                                    }
                                    format!(
                                        "{letter}, {}, filled by {}",
                                        item.name,
                                        files.join(", ")
                                    )
                                }
                                _ => letter.clone(),
                            };
                            r.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                            });
                            draw_dock(
                                ui,
                                model,
                                thumbs,
                                v,
                                r.rect,
                                &entries,
                                chip_locks(v, Some(k)),
                                sel_items,
                                acts,
                                &mut member_menu,
                                &mut badged,
                            );
                        }
                        // K12: an add/remove entry that matches no file.
                        // Note DU1: its menu removes the entry (an entry with no files left: Delete set).
                        for m in &v.missing {
                            let r = letter_box(ui, "?", Some(RED)).interact(egui::Sense::click());
                            let text = m.text();
                            r.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    format!("Missing: {text}"),
                                )
                            });
                            let r = r.on_hover_text(format!("no file matches {text}"));
                            member_menu |= r.secondary_clicked();
                            let open = r.context_menu(|ui| {
                                let (label, act) = match m {
                                    Missing::NoFilesLeft => {
                                        ("Delete set", Act::Delete(v.id.clone(), v.members.len()))
                                    }
                                    _ => (
                                        "Remove from set",
                                        Act::RemoveMissing(v.id.clone(), m.clone()),
                                    ),
                                };
                                let b = ui.add_enabled(editing.on, egui::Button::new(label));
                                if b.on_disabled_hover_text(editing.why).clicked() {
                                    acts.push(act);
                                    ui.close();
                                }
                            });
                            member_menu |= open.is_some();
                        }
                        // Members in no chip (an env mask `_m`, a clashing second kind) follow as plain thumbnails.
                        for mk in &v.members {
                            if chipped.contains(mk.as_str()) || badged.contains(mk.as_str()) {
                                continue;
                            }
                            // F3a Task 8: the slot the member fills in this set, if any, has a slot menu.
                            let kind = v
                                .slots
                                .iter()
                                .find(|(_, f)| matches!(f, SlotFill::Own { item } if item == mk))
                                .map(|(k, _)| *k);
                            let Some(item) = model.item(mk) else {
                                continue;
                            };
                            // Named by its file and why it is here (REPORT M1); an unused helper is dimmed.
                            let role = match (kind, &item.helper, &item.job) {
                                (Some(k), ..) => format!("{} (its slot is taken)", v.slot_text(k)),
                                (None, Some(h), _) => helper_target_text(h),
                                (None, None, Some(j)) => subtype_name(j.profile.subtype),
                                _ => "not converted".to_string(),
                            };
                            let dim = item.helper.as_ref().is_some_and(|h| h.fills.is_empty());
                            let was = ui.opacity();
                            if dim {
                                ui.multiply_opacity(ui.visuals().disabled_alpha());
                            }
                            let r = member_chip(
                                ui,
                                model,
                                thumbs,
                                v,
                                mk,
                                kind,
                                true,
                                Some(&role),
                                sel_items,
                                acts,
                                &mut member_menu,
                            );
                            ui.set_opacity(was);
                            if let Some(r) = r {
                                let name = format!("{}, {role}", item.name);
                                r.widget_info(|| {
                                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name)
                                });
                            }
                        }
                    });
                });
            },
        );
        // Final review I1: the zone's hover stroke is under the card frame, so outline the drop target on top.
        if editing.on
            && egui::DragAndDrop::has_payload_of_type::<DragItem>(ui.ctx())
            && zone.contains_pointer()
        {
            ui.painter().rect_stroke(
                zone.rect,
                12.0,
                ui.visuals().widgets.active.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        dropped
    });
    let dropped = scoped.inner;
    let resp = scoped.response;
    // Note FM32: named as its stand-in is, so a card keeps its name when it scrolls in (a Tab stop for the card itself).
    ui.ctx().accesskit_node_builder(resp.id, |n| {
        card_node(n, v, (index, total), selected, resp.rect)
    });
    if !editing.on {
        resp.clone().on_hover_text(editing.why);
    }
    if resp.clicked() && !v.key.is_empty() && acts.is_empty() {
        let m = ui.input(|i| i.modifiers);
        acts.push(Act::Click(v.key.clone(), m.command, m.shift));
    }
    if let Some(p) = dropped {
        if editing.on {
            acts.push(Act::Drop(p.0.clone(), v.id.clone()));
        }
    }
    let hovered_by_set = editing.reorder && resp.dnd_hover_payload::<DragSet>().is_some();
    // K9: only while a set hovers, since `take_payload` would take a dragged texture too.
    let dropped_set = hovered_by_set
        .then(|| resp.dnd_release_payload::<DragSet>())
        .flatten()
        .map(|p| p.0.clone());
    // F3a D4: a right-click on an unselected pill selects it alone first; among two or more selected sets the pill
    // shows the bulk menu, otherwise its own.
    if resp.secondary_clicked() && !v.key.is_empty() && !member_menu {
        acts.push(Act::RightClick(v.key.clone()));
    }
    let bulk = look.selected && look.bulk;
    if !member_menu {
        resp.context_menu(|ui| {
            if bulk {
                if let Some(a) = bulk_menu(ui, &model.menu_title(true), &model.set_menu()) {
                    acts.push(Act::Bulk(a));
                }
            } else {
                menu(ui, v, index, editing, model, acts)
            }
        });
    }
    PillOut {
        dropped_set,
        hovered_by_set,
        rect: resp.rect,
    }
}

/// The set's Shader dropdown (M6 spec §7): the engine's pick with its provenance, then "Auto" and the mode's
/// shaders. Disabled when sets are not editable.
fn shader_combo(
    ui: &mut egui::Ui,
    model: &AppModel,
    v: &SetView,
    editing: &Editing,
    acts: &mut Vec<Act>,
) {
    let text = v.shader.as_ref().map_or(String::new(), |p| {
        let tag = match p.provenance {
            ShaderOrigin::Set => "",
            ShaderOrigin::Auto(_) => " (auto)",
            ShaderOrigin::Fallback => " (fallback)",
            ShaderOrigin::Derived => " (derived)",
        };
        format!("{}{tag}", p.shader.label())
    });
    // A5: its own row, the card's width (a ComboBox never wraps, so it is never wider than the row).
    ui.add_enabled_ui(editing.on, |ui| {
        egui::ComboBox::from_id_salt(("shader", &v.id))
            .width(ui.available_width())
            .truncate()
            .selected_text(text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(v.shader_choice.is_none(), "Auto")
                    .clicked()
                {
                    acts.push(Act::Shader(v.id.clone(), None));
                }
                for s in shader_options(model.target) {
                    if ui
                        .selectable_label(v.shader_choice == Some(s), s.label())
                        .clicked()
                    {
                        acts.push(Act::Shader(v.id.clone(), Some(s)));
                    }
                }
            });
    });
}

/// The Rename submenu of a set's menu: label, one-line hover text, and what it does. F3b D5 for the last two.
fn rename_entries(set: &str) -> [(String, &'static str, Act); 3] {
    [
        (
            "Rename…".to_string(),
            "Change the set's name only",
            Act::OpenRename(set.to_string()),
        ),
        (
            format!("Rename outputs…  {}", super::actions::keys_text("rename")),
            "Change the names of the files this set writes",
            Act::Bulk(MenuAction::RenameOutputs),
        ),
        (
            "Rename inputs…".to_string(),
            "Read the source files under other names; the files on disk keep theirs",
            Act::Bulk(MenuAction::RenameInputs),
        ),
    ]
}

fn menu(
    ui: &mut egui::Ui,
    v: &SetView,
    index: usize,
    editing: &Editing,
    model: &AppModel,
    acts: &mut Vec<Act>,
) {
    // First: it only reads, so it works while sets are not editable (the disable below cannot be undone).
    ui.menu_button("Show in Explorer", |ui| {
        for mk in &v.members {
            let name = model.item(mk).map_or(mk.as_str(), |i| i.name.as_str());
            let off = model.reveal_problem(mk);
            let r = ui.add_enabled(off.is_none(), egui::Button::new(name));
            let r = match &off {
                Some(why) => r.on_disabled_hover_text(why),
                None => r,
            };
            if r.clicked() {
                acts.push(Act::Bulk(MenuAction::ShowInExplorer(mk.clone())));
                ui.close();
            }
        }
    });
    if !editing.on {
        ui.disable();
    }
    let tip = |r: egui::Response| {
        if editing.on {
            r
        } else {
            r.on_disabled_hover_text(editing.why)
        }
    };
    // Item 23: one "Rename" submenu; each entry says what it renames. F2 stays on Rename outputs.
    let rename = ui.menu_button("Rename", |ui| {
        for (label, hover, act) in rename_entries(&v.id) {
            if tip(ui.button(label).on_hover_text(hover)).clicked() {
                acts.push(act);
                ui.close();
            }
        }
    });
    tip(rename.response);
    if tip(ui.button("Edit match…")).clicked() && !v.key.is_empty() {
        acts.push(Act::Select(v.key.clone()));
        ui.close();
    }
    // Note CO (P19): the set's entry point, for a set whose shader shows no Height row.
    let generate = model.gen_entry(&v.id, None);
    let r = ui.add_enabled(
        generate.disabled.is_none(),
        egui::Button::new(&generate.label),
    );
    let r = match &generate.disabled {
        Some(why) if editing.on => r.on_disabled_hover_text(why),
        _ => tip(r),
    };
    if r.clicked() {
        acts.push(Act::Bulk(generate.action));
        ui.close();
    }
    // Note CO (P20): every generated file of the set, with its stale reason on hover.
    let gens = model.delete_generated_entries(&v.id);
    if !gens.is_empty() {
        let del = ui.menu_button("Delete generated file", |ui| {
            for e in gens {
                let key = match &e.action {
                    MenuAction::Generated {
                        op: crate::model::from_normal::GenOp::Delete(k),
                        ..
                    } => k.as_str(),
                    _ => "",
                };
                // A replaced file fills no slot row (review M2), so its rival is named here.
                let info = model.generated_info(key);
                let hover: Vec<String> = info
                    .iter()
                    .flat_map(|g| {
                        [
                            g.replaced_by.as_ref().map(|r| format!("replaced by {r}")),
                            g.stale.as_ref().map(|s| format!("stale: {s}")),
                        ]
                    })
                    .flatten()
                    .collect();
                let mut r = ui.add_enabled(e.disabled.is_none(), egui::Button::new(&e.label));
                if !hover.is_empty() {
                    r = r.on_hover_text(hover.join("\n"));
                }
                if let Some(why) = &e.disabled {
                    r = r.on_disabled_hover_text(why);
                }
                if r.clicked() {
                    acts.push(Act::Bulk(e.action));
                    ui.close();
                }
            }
        });
        tip(del.response);
    }
    let remove = ui.menu_button("Remove member", |ui| {
        for mk in &v.members {
            let name = model.item(mk).map_or(mk.as_str(), |i| i.name.as_str());
            if ui.button(name).clicked() {
                acts.push(Act::RemoveMember(v.id.clone(), mk.clone()));
                ui.close();
            }
        }
    });
    tip(remove.response);
    let del = ui.add_enabled(
        v.created || v.key.is_empty(),
        egui::Button::new("Delete set"),
    );
    let del = del.on_disabled_hover_text(if editing.on {
        "Only sets you created can be deleted; an automatic set comes back from the file names"
    } else {
        editing.why
    });
    if del.clicked() {
        acts.push(Act::Delete(v.id.clone(), v.members.len()));
        ui.close();
    }
    // K19: `index` is among all sets; while a filter is on neither moves.
    let move_button = |ui: &mut egui::Ui, label: &str| {
        let r = ui.add_enabled(editing.reorder || !editing.on, egui::Button::new(label));
        if editing.on && !editing.reorder {
            r.on_disabled_hover_text(CLEAR_TO_REORDER)
        } else {
            tip(r)
        }
    };
    if move_button(ui, "Move up").clicked() && index > 0 {
        acts.push(Act::Reorder(v.id.clone(), index - 1));
        ui.close();
    }
    if move_button(ui, "Move down").clicked() {
        acts.push(Act::Reorder(v.id.clone(), index + 1));
        ui.close();
    }
    // R2: it moves by the full order and names the target, so it stays on while a filter is on.
    let move_to = ui
        .button("Move to…")
        .on_hover_text("Move the set to a position you type, by number or by name");
    if tip(move_to).clicked() {
        acts.push(Act::OpenMoveTo(v.id.clone()));
        ui.close();
    }
    let fill = ui.button("Fill from names…").on_hover_text(
        "Move in the files of this folder named like the set with a known suffix (_n, _p, _rmaos, …)",
    );
    if tip(fill).clicked() {
        acts.push(Act::FillNames(v.id.clone()));
        ui.close();
    }
    // Note HG: one reason at a time; the scan comes first, as the PBR refusal can't be fixed by scanning.
    let why_not = if model.known.records.is_empty() {
        Some("Scan your mods first")
    } else if model.record_ignored(v) {
        Some("PGPatcher PBR sets name their outputs in the JSON; use Rename outputs")
    } else {
        None
    };
    let label = if v.txst.is_some() {
        "Change record…"
    } else {
        "Map to texture set record…"
    };
    let map = ui.add_enabled(why_not.is_none(), egui::Button::new(label));
    let map = match why_not {
        Some(why) if editing.on => map.on_disabled_hover_text(why),
        _ => tip(map),
    };
    if map.clicked() {
        acts.push(Act::MapRecord(v.id.clone()));
        ui.close();
    }
    if v.txst.is_some() && tip(ui.button("Remove record mapping")).clicked() {
        acts.push(Act::UnmapRecord(v.id.clone()));
        ui.close();
    }
}

/// What a button in the drop modal asked for; applied after the modal is drawn (the modal only reads the model).
#[derive(Clone, Copy)]
enum ModalAnswer {
    Choice(DropChoice),
    Replace,
    Close,
}

/// The drop, delete and rename dialogs. Drawn by `ui::draw_with_scan` whether or not this column shows: a file
/// dropped on a slot card asks its Replace here too (item 17).
pub fn pending_modal(ctx: &egui::Context, model: &mut AppModel, st: &mut DropState) {
    if matches!(st.pending, Some(Pending::Rename { .. })) {
        rename_modal(ctx, model, st);
        return;
    }
    if matches!(st.pending, Some(Pending::FillNames { .. })) {
        fill_modal(ctx, model, st);
        return;
    }
    if matches!(st.pending, Some(Pending::MoveTo { .. })) {
        move_to_modal(ctx, model, st);
        return;
    }
    if matches!(st.pending, Some(Pending::MapRecord { .. })) {
        map_record_modal(ctx, model, st);
        return;
    }
    let Some(p) = st.pending.clone() else { return };
    use super::dialogs::Role;
    // Per case: the title, the body line, and the footer as (label, role, what it answers).
    let (title, line, footer) = match &p {
        Pending::Choice { item, from, .. } => {
            let name = model.item(item).map_or(item.clone(), |i| i.name.clone());
            // K5: the three answers stay in the body; the footer has only Cancel.
            let footer = vec![("Cancel".to_string(), Role::Plain, ModalAnswer::Close)];
            (format!("{name} is in {from}"), None, footer)
        }
        Pending::Replace { set, existing, .. } => {
            let name = model
                .item(existing)
                .map_or(existing.clone(), |i| i.name.clone());
            // K1: Enter does nothing here, so Cancel is Plain and nothing is filled.
            let footer = vec![
                (
                    format!("Replace {}", super::dialogs::short_name(&name)),
                    Role::Destructive,
                    ModalAnswer::Replace,
                ),
                ("Cancel".to_string(), Role::Plain, ModalAnswer::Close),
            ];
            let line = format!("{set} already has {name} in that slot.");
            ("Replace?".to_string(), Some(line), footer)
        }
        Pending::Refused(why) => {
            let footer = vec![("OK".to_string(), Role::Default, ModalAnswer::Close)];
            ("Can't do that".to_string(), Some(why.clone()), footer)
        }
        Pending::Rename { .. }
        | Pending::FillNames { .. }
        | Pending::MoveTo { .. }
        | Pending::MapRecord { .. } => return,
    };
    let buttons: Vec<(&str, _, bool)> = footer.iter().map(|b| (b.0.as_str(), b.1, true)).collect();
    let mut answer = None;
    let (keys, clicked) =
        super::dialogs::dialog(ctx, "set-drop", true, 420.0, &title, &buttons, |ui| {
            if let Some(line) = &line {
                ui.label(line);
            }
            if matches!(p, Pending::Choice { .. }) {
                for (label, c) in [
                    ("Move it here", DropChoice::Move),
                    ("Share (borrow its finished output)", DropChoice::Share),
                    ("Duplicate the source for this set", DropChoice::Duplicate),
                ] {
                    if ui.button(label).clicked() {
                        answer = Some(ModalAnswer::Choice(c));
                    }
                }
            }
        });
    if let Some(i) = clicked {
        answer = Some(footer[i].2);
    }
    // Esc and a click outside cancel; Enter presses the OK of a refusal.
    let refusal = matches!(p, Pending::Refused(_));
    if answer.is_none() && (keys.cancel || (keys.enter && refusal)) {
        answer = Some(ModalAnswer::Close);
    }
    let Some(answer) = answer else { return };
    st.pending = match (answer, p) {
        (ModalAnswer::Choice(c), Pending::Choice { item, set, .. }) => {
            match model.drop_choice(&item, &set, c) {
                DropOutcome::AskReplace { existing } => Some(Pending::Replace {
                    item,
                    set,
                    existing,
                    choice: c,
                }),
                DropOutcome::Refused(why) => Some(Pending::Refused(why)),
                _ => None,
            }
        }
        // The replace answers the choice that led to it (Move, Share or Duplicate); a refusal shows its reason.
        (
            ModalAnswer::Replace,
            Pending::Replace {
                item, set, choice, ..
            },
        ) => model
            .drop_replace(&item, &set, choice)
            .err()
            .map(Pending::Refused),
        _ => None,
    };
}

/// Item 18: Fill from names. A row per file with its slot; a filled slot says what it replaces and a blocked row
/// (a locked slot) cannot be ticked. Esc or Cancel closes, Enter or Fill applies the ticked rows as one step. Ticks
/// are typed state, so a click outside does nothing (AG11).
fn fill_modal(ctx: &egui::Context, model: &mut AppModel, st: &mut DropState) {
    let Some(Pending::FillNames { set, rows, ticks }) = &mut st.pending else {
        return;
    };
    use super::dialogs::Role;
    let mut error = None;
    let title = format!("Fill {set} from names");
    let buttons = [("Cancel", Role::Plain, true), ("Fill", Role::Default, true)];
    let (keys, clicked) =
        super::dialogs::dialog(ctx, "fill-names", false, 420.0, &title, &buttons, |ui| {
            ui.label("Each ticked file moves into the set's slot; nothing is copied.");
            egui::Grid::new("fill-names-grid")
                .num_columns(2)
                .show(ui, |ui| {
                    for (r, tick) in rows.iter().zip(ticks.iter_mut()) {
                        let mut on = *tick && r.blocked.is_none();
                        let resp = ui.add_enabled(
                            r.blocked.is_none(),
                            egui::Checkbox::new(&mut on, &r.name),
                        );
                        if let Some(why) = &r.blocked {
                            resp.on_disabled_hover_text(why);
                        }
                        *tick = on;
                        let mut what = format!("→ {}", r.slot.label());
                        match (&r.blocked, &r.replaces) {
                            (Some(why), _) => what.push_str(&format!(" · {why}")),
                            (None, Some(cur)) => what.push_str(&format!(" · replaces {cur}")),
                            _ => {}
                        }
                        ui.label(what);
                        ui.end_row();
                    }
                });
        });
    if keys.cancel || clicked == Some(0) {
        st.pending = None;
        return;
    }
    if clicked == Some(1) || keys.enter {
        let picked: Vec<String> = rows
            .iter()
            .zip(ticks.iter())
            .filter(|(_, t)| **t)
            .map(|(r, _)| r.item.clone())
            .collect();
        if let Err(e) = model.fill_from_names(set, &picked) {
            error = Some(e);
        }
        st.pending = error.map(Pending::Refused);
    }
}

/// The rename dialog: Enter or OK renames, a refusal (a name already taken) is shown under the field, and
/// Cancel or Esc closes it.
fn rename_modal(ctx: &egui::Context, model: &mut AppModel, st: &mut DropState) {
    let Some(Pending::Rename { id, text, error }) = &mut st.pending else {
        return;
    };
    use super::dialogs::Role;
    let mut submit = false;
    let title = format!("Rename {id}");
    // The field's own Enter renames, so no button is the Default.
    let buttons = [("Cancel", Role::Plain, true), ("OK", Role::Plain, true)];
    // Only Esc cancels: a click outside could lose the typed name (AG11).
    let (keys, clicked) =
        super::dialogs::dialog(ctx, "set-rename", false, 420.0, &title, &buttons, |ui| {
            let field = ui.text_edit_singleline(text);
            if !field.has_focus() && !field.lost_focus() {
                field.request_focus();
            }
            if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                submit = true;
            }
            if let Some(e) = error.as_ref() {
                ui.colored_label(RED, e);
            }
        });
    submit |= clicked == Some(1);
    if keys.cancel || clicked == Some(0) {
        st.pending = None;
    } else if submit {
        match model.rename_set(id, text) {
            Ok(()) => st.pending = None,
            Err(e) => *error = Some(e),
        }
    }
}

/// Move to… (R3, CF5): type a position or a name, the arrows move the highlight, Enter or a click picks it. Esc or
/// Cancel closes; a click outside does not (it holds typed text). It moves in the full order, filter or not.
fn move_to_modal(ctx: &egui::Context, model: &mut AppModel, st: &mut DropState) {
    let Some(Pending::MoveTo { id, query, sel }) = &mut st.pending else {
        return;
    };
    use super::dialogs::Role;
    let views = crate::model::sets::views(model);
    let visible = model.visible_sets();
    let mut picked = None;
    let title = format!("Move {id} to…");
    let buttons = [("Cancel", Role::Plain, true)];
    let (keys, clicked) =
        super::dialogs::dialog(ctx, "set-move-to", false, 420.0, &title, &buttons, |ui| {
            // Before the field, so it does not move its cursor with them (as the palette does).
            let (up, down) = ui.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                )
            });
            let field = ui.add(
                egui::TextEdit::singleline(query)
                    .hint_text("Number or name")
                    .desired_width(f32::INFINITY),
            );
            // M5: a hint is only a placeholder, so the field gets an accessible name too.
            field.widget_info(|| egui::WidgetInfo {
                label: Some("Position or set name".into()),
                ..egui::WidgetInfo::text_edit(
                    true,
                    query.as_str(),
                    query.as_str(),
                    "Number or name",
                )
            });
            if !field.has_focus() && !field.lost_focus() {
                field.request_focus();
            }
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    field.id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
            // Never on the opening frame: the field had no focus to lose (BY2).
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if field.changed() {
                *sel = 0;
            }
            let hits = move_to_hits(&views, query);
            let last = hits.rows.len().saturating_sub(1);
            if up {
                *sel = sel.saturating_sub(1);
            }
            if down {
                *sel = (*sel + 1).min(last);
            }
            *sel = (*sel).min(last);
            if hits.out_of_range {
                ui.weak(format!("Positions are 1 to {}", views.len()));
            } else if hits.rows.is_empty() {
                ui.weak("Nothing matches");
            }
            let row_h = ui.spacing().interact_size.y;
            egui::ScrollArea::vertical()
                .max_height(240.0)
                .auto_shrink([false, true])
                .show_rows(ui, row_h, hits.rows.len(), |ui, range| {
                    for r in range {
                        let i = hits.rows[r];
                        ui.horizontal(|ui| {
                            let text = format!("{}  {}", i + 1, views[i].id);
                            let resp = ui.selectable_label(r == *sel, text);
                            if r == *sel && (up || down) {
                                resp.scroll_to_me(None);
                            }
                            if resp.clicked() {
                                picked = Some(i);
                            }
                            if !visible.contains(&i) {
                                ui.weak("(hidden by filter)");
                            }
                        });
                    }
                });
            if enter {
                picked = picked.or_else(|| hits.rows.get(*sel).copied());
            }
        });
    if keys.cancel || clicked == Some(0) {
        st.pending = None;
    } else if let Some(to) = picked {
        // The set's own position closes too: `reorder_set` refuses it, so nothing changes.
        let id = id.clone();
        st.pending = None;
        apply_reorder(model, &id, to);
    }
}

/// One picker row: EDID, FormID, plugin (and the winner when it differs) and the colour path, each cut to its column
/// with the whole text on hover (the label's own tooltip) and in the row's AccessKit name.
fn record_row(ui: &mut egui::Ui, r: &TxstRecord, selected: bool) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    let name = format!("{} ({}, {})", r.edid, r.formid, r.plugin);
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &name)
    });
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    if selected {
        ui.painter()
            .rect_filled(rect, 2.0, ui.visuals().selection.bg_fill);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 2.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let plugin = if r.winner.eq_ignore_ascii_case(&r.plugin) {
        r.plugin.clone()
    } else {
        format!("{} (won by {})", r.plugin, r.winner)
    };
    let colour = r.slots.get(&1).map_or("", String::as_str);
    // (text, start, end) as fractions of the row; the last gap keeps a cell off its neighbour.
    let cells = [
        (r.edid.as_str(), 0.0, 0.27),
        (r.formid.as_str(), 0.27, 0.37),
        (plugin.as_str(), 0.37, 0.65),
        (colour, 0.65, 1.0),
    ];
    for (text, a, b) in cells {
        let cell = egui::Rect::from_min_max(
            egui::pos2(rect.min.x + a * rect.width() + 4.0, rect.min.y),
            egui::pos2(rect.min.x + b * rect.width() - 4.0, rect.max.y),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(cell)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                ui.add(egui::Label::new(text).truncate().selectable(false));
            },
        );
    }
    resp
}

/// Map to texture set record… (Note HG): type to filter, click a row, read what each file would write, then Map. It
/// writes nothing before Map, and Enter does not map (there is no Default button). Esc or Cancel closes; a click
/// outside does not (it holds the typed text). The list is virtual: the scan holds thousands of records.
fn map_record_modal(ctx: &egui::Context, model: &mut AppModel, st: &mut DropState) {
    let Some(Pending::MapRecord {
        id,
        query,
        hits,
        seen,
        sel,
    }) = &mut st.pending
    else {
        return;
    };
    use super::dialogs::Role;
    let known = model.known.clone();
    let stamp = std::sync::Arc::as_ptr(&known) as usize;
    let title = format!("Map {id} to a texture set record");
    let buttons = [
        ("Cancel", Role::Plain, true),
        ("Map", Role::Plain, sel.is_some()),
    ];
    let hovers = [None, Some("Pick a record first")];
    let (keys, clicked) = super::dialogs::dialog_hover(
        ctx,
        "set-map-record",
        false,
        640.0,
        &title,
        &buttons,
        &hovers,
        |ui| {
            // Before the field, so it does not move its cursor with them (as Move to… does). They only move the
            // highlight; Enter still maps nothing.
            let (up, down) = ui.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                )
            });
            let field = ui.add(
                egui::TextEdit::singleline(query)
                    .hint_text("EDID, plugin, FormID or path")
                    .desired_width(f32::INFINITY),
            );
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    field.id,
                    egui::EventFilter {
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
            field.widget_info(|| egui::WidgetInfo {
                label: Some("Search texture set records".into()),
                ..egui::WidgetInfo::text_edit(
                    true,
                    query.as_str(),
                    query.as_str(),
                    "EDID, plugin, FormID or path",
                )
            });
            if !field.has_focus() && !field.lost_focus() && seen.is_none() {
                field.request_focus();
            }
            // Only a changed query (or a scan that landed) filters again.
            if !seen
                .as_ref()
                .is_some_and(|(q, s)| q == query && *s == stamp)
            {
                *hits = crate::model::txst_map::record_hits(&known.records, query);
                *seen = Some((query.clone(), stamp));
            }
            ui.weak(format!("{} of {} records", hits.len(), known.records.len()));
            let row_h = ui.spacing().interact_size.y;
            let mut moved = None;
            if up || down {
                let at = sel
                    .as_ref()
                    .and_then(|s| hits.iter().position(|&i| known.records.get(i) == Some(s)));
                let next = match (at, down) {
                    (None, _) => 0,
                    (Some(p), true) => (p + 1).min(hits.len().saturating_sub(1)),
                    (Some(p), false) => p.saturating_sub(1),
                };
                if let Some(r) = hits.get(next).and_then(|&i| known.records.get(i)) {
                    *sel = Some(r.clone());
                    moved = Some(next);
                }
            }
            // `show_rows` builds only the rows on screen, so a moved highlight scrolls by offset, from last frame's
            // offset and view height.
            let view_id = egui::Id::new("map-record-list-view");
            let (off, view) = ui
                .data(|d| d.get_temp::<(f32, f32)>(view_id))
                .unwrap_or((0.0, row_h * 9.0));
            let mut area = egui::ScrollArea::vertical()
                .id_salt("map-record-list")
                .max_height(row_h * 9.0)
                .auto_shrink([false, true]);
            if let Some(n) = moved {
                let top = n as f32 * (row_h + ui.spacing().item_spacing.y);
                if top < off {
                    area = area.vertical_scroll_offset(top);
                } else if top + row_h > off + view {
                    area = area.vertical_scroll_offset(top + row_h - view);
                }
            }
            let out = area.show_rows(ui, row_h, hits.len(), |ui, range| {
                for n in range {
                    let Some(r) = hits.get(n).and_then(|&i| known.records.get(i)) else {
                        continue;
                    };
                    if record_row(ui, r, sel.as_ref() == Some(r)).clicked() {
                        *sel = Some(r.clone());
                    }
                }
            });
            ui.data_mut(|d| d.insert_temp(view_id, (out.state.offset.y, out.inner_rect.height())));
            if let Some(r) = sel.as_ref() {
                ui.separator();
                let head = format!("{} ({}, {})", r.edid, r.formid, r.plugin);
                ui.add(egui::Label::new(egui::RichText::new(head).strong()).truncate());
                egui::ScrollArea::vertical()
                    .id_salt("map-record-preview")
                    .max_height(row_h * 5.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for (file, path) in model.record_preview(id, r) {
                            match path {
                                Some(p) => {
                                    ui.add(egui::Label::new(format!("{file} → {p}")).truncate())
                                }
                                None => ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("{file}: keeps its own path"))
                                            .weak(),
                                    )
                                    .truncate(),
                                ),
                            };
                        }
                    });
            }
        },
    );
    if keys.cancel || clicked == Some(0) {
        st.pending = None;
    } else if clicked == Some(1) {
        if let Some(Pending::MapRecord {
            id, sel: Some(r), ..
        }) = st.pending.take()
        {
            if let Err(why) = model.map_record(&id, r.to_map()) {
                st.pending = Some(Pending::Refused(why));
            }
        }
    }
}

/// The card's record line (Note HG); its hover lists the stored slots, marking the ones no set writes (`supplied`: the
/// Advanced skin editor has no `_s` row to say so). Small, not weak: it is the only card text that says where outputs go.
fn record_line(ui: &mut egui::Ui, v: &SetView, map: &TxstMap, ignored: bool) {
    let ignored = if ignored {
        " (ignored: PGPatcher PBR set)"
    } else {
        ""
    };
    let text = format!(
        "Record: {} ({}, {}){ignored}",
        map.edid, map.formid, map.plugin
    );
    let r = ui.add(
        egui::Label::new(egui::RichText::new(&text).small())
            .truncate()
            .show_tooltip_when_elided(false),
    );
    r.on_hover_ui(|ui| {
        ui.label(&text);
        for (n, p) in &map.slots {
            let from = if v.record_supplied.contains_key(n) {
                " (from the record)"
            } else {
                ""
            };
            ui.label(format!("slot{n}: {p}{from}"));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::codes::Code;

    #[test]
    fn dock_cells_fill_right_then_left_and_fold_the_rest() {
        use Cell::*;
        assert_eq!(dock_cells(false, false, 0), Vec::<(usize, Cell)>::new());
        assert_eq!(dock_cells(false, false, 1), [(0, Badge(0))]);
        assert_eq!(dock_cells(false, false, 2), [(0, Badge(0)), (1, Badge(1))]);
        assert_eq!(dock_cells(false, false, 4), [(0, Badge(0)), (1, More(3))]);
        assert_eq!(dock_cells(true, false, 1), [(1, Badge(0))]); // the slot 🔒 keeps the right cell
        assert_eq!(dock_cells(true, false, 2), [(1, More(2))]);
        assert_eq!(dock_cells(true, false, 3), [(1, More(3))]);
        assert_eq!(dock_cells(false, true, 2), [(0, More(2))]); // the PGPatcher 🔒 keeps the left cell
        assert_eq!(dock_cells(true, true, 2), Vec::<(usize, Cell)>::new());
    }

    fn warn(msg: &str) -> Warning {
        Warning::new(Code::LossAlphaData, msg)
    }

    #[test]
    fn the_warning_tooltip_lists_file_code_message_and_caps_at_ten() {
        let w = warn("alpha is lost");
        let few = [("a.png", &w), ("b.png", &w)];
        let t = warning_tooltip(&few);
        assert_eq!(t.lines().count(), 2);
        assert!(t.starts_with("a.png: LOSS-ALPHA-DATA alpha is lost"), "{t}");
        let many: Vec<(&str, &Warning)> = (0..13).map(|_| ("x.png", &w)).collect();
        let t = warning_tooltip(&many);
        assert_eq!(t.lines().count(), 11);
        assert_eq!(t.lines().last(), Some("… and 3 more"));
        assert!(!warning_tooltip(&many[..10]).contains("more"));
    }

    #[test]
    fn the_insertion_line_is_after_the_target_when_moving_down_and_before_it_when_moving_up() {
        let r = egui::Rect::from_min_max(egui::pos2(0.0, 10.0), egui::pos2(5.0, 30.0));
        assert_eq!(insertion_y(0, 2, r), Some(30.0));
        assert_eq!(insertion_y(2, 0, r), Some(10.0));
        assert_eq!(
            insertion_y(1, 1, r),
            None,
            "hovering the dragged pill itself shows no line"
        );
    }

    #[test]
    fn a_released_handle_over_another_pill_reorders_to_its_index() {
        let v = ["a", "b", "c"];
        assert_eq!(
            reorder_target(Some("a"), Some(2), &v),
            Some(("a".to_string(), 2))
        );
        assert_eq!(
            reorder_target(Some("c"), Some(0), &v),
            Some(("c".to_string(), 0))
        );
    }

    #[test]
    fn move_to_hits_reads_numbers_then_names() {
        let views: Vec<SetView> = ["a/Rock2", "a/leaf", "b/Moss", "c/armor_4", "c/s0"]
            .iter()
            .map(|id| SetView::new(format!("k/{id}"), id.to_string(), vec![]))
            .collect();
        let hits = |q: &str| move_to_hits(&views, q);
        // I1: digits are a position only, never a name ("2" is not also Rock2).
        assert_eq!(
            hits("2"),
            MoveHits {
                rows: vec![1],
                out_of_range: false
            }
        );
        assert_eq!(hits(" 1 ").rows, vec![0]);
        assert_eq!(
            hits("01").rows,
            vec![0],
            "a leading zero is still position 1"
        );
        // Out of range: no row, even where a name holds the digits (the 0 in s0).
        for q in ["0", "00", "6", "99999999999999999999999"] {
            assert_eq!(
                hits(q),
                MoveHits {
                    rows: vec![],
                    out_of_range: true
                },
                "{q:?}"
            );
        }
        assert_eq!(hits("").rows, vec![0, 1, 2, 3, 4]);
        assert_eq!(hits("   ").rows, vec![0, 1, 2, 3, 4]);
        assert!(!hits("").out_of_range);
        assert_eq!(hits("MOSS").rows, vec![2]);
        assert_eq!(hits("A\\L").rows, vec![1], "folded like set ids");
        assert_eq!(
            hits("armor_4").rows,
            vec![3],
            "a name with a digit is a name"
        );
        assert!(hits("zzz").rows.is_empty());
        assert!(!hits("zzz").out_of_range);
    }

    #[test]
    fn no_release_no_target_or_a_drop_on_itself_does_nothing() {
        let v = ["a", "b"];
        assert_eq!(reorder_target(None, Some(1), &v), None);
        assert_eq!(reorder_target(Some("a"), None, &v), None);
        assert_eq!(reorder_target(Some("a"), Some(0), &v), None);
        assert_eq!(reorder_target(Some("a"), Some(9), &v), None);
    }
}
