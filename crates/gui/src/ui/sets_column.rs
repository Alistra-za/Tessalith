//! The editable Sets column (M3b spec §3.1): set pills in project order with badges, drag and drop
//! of textures onto sets (and onto an empty area for a new set), drag-to-reorder by the ≡ handle, and a context
//! menu. Drawing only: every change is a model call, made after drawing.
use super::columns::{
    bulk_menu, member, status_text, AMBER, DOCK_GAP_X, DOCK_GAP_Y, DOCK_INSET, RED,
};
use super::slot_commands::SlotCmdState;
use super::status::{shared_stroke, Status};
use super::thumbs::ThumbCache;
use crate::model::bottom::{check_listed, counts_for, no_files_yet, scope_key, At, Flagged};
use crate::model::bulk::MenuAction;
use crate::model::helpers::{docked, fill_text, Docked};
use crate::model::items::{helper_tag_text, helper_target_files, ItemKind, TextureItem};
use crate::model::labels::subtype_name;
use crate::model::pbr_export::export_blocks;
use crate::model::picker::{PickerPurpose, PickerSpec};
use crate::model::set_groups::GroupTarget;
use crate::model::sets::{
    badge_text, chip_letter, fill_proposals, is_shareable_helper, kind_text, member_tail,
    shader_options, slot_shown, view_row, BorrowOccupant, BorrowOutcome, DropChoice, DropOutcome,
    FillProposal, Missing, SetView, SlotFill, SlotKind, SlotLink, SlotShown,
};
use crate::model::target_mod::{FileBase, MeshPath};
use crate::model::txst_map::TargetPick;
use crate::model::{AppModel, Selection, SetTarget};
use crate::sets_doc::TxstMap;
use eframe::egui;
use engine::codes::Warning;
use engine::sets::fold_id;
use engine::shader::{Shader, ShaderOrigin};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Drag payload: a texture, by item key.
#[derive(Debug, Clone)]
pub struct DragItem(pub String);

/// Note JW4: a shared chip carries the item and the set that owns it at the end of the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragBorrow {
    pub item: String,
    pub from: String,
}

/// The texture to paint under the pointer during an image drag.
pub fn dragged_item_key(ctx: &egui::Context) -> Option<String> {
    egui::DragAndDrop::payload::<DragItem>(ctx)
        .map(|p| p.0.clone())
        .or_else(|| egui::DragAndDrop::payload::<DragBorrow>(ctx).map(|p| p.item.clone()))
}

#[derive(Clone, Copy)]
enum ChipDrag<'a> {
    None,
    Item,
    Borrow(&'a str),
}

enum Dropped {
    Item(Arc<DragItem>),
    Borrow(Arc<DragBorrow>),
}

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
    BorrowReplace {
        item: String,
        from: String,
        set: String,
        kind: SlotKind,
        existing: BorrowOccupant,
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
    /// Note HF6: the rename dialog of group `name`: the name being typed and why the last try was refused.
    RenameGroup {
        name: String,
        text: String,
        error: Option<String>,
    },
    /// Note HF6: the name dialog of a new group for sets `ids`.
    NewGroup {
        ids: Vec<String>,
        text: String,
        error: Option<String>,
    },
    /// Note CF5: Move to…, the set, the typed number or name, and the highlighted row.
    MoveTo {
        id: String,
        query: String,
        sel: usize,
    },
    /// Notes HG, HL4: Map to target…, the set, the typed query, the list `rows` (built for `seen`'s scan and target
    /// stamps), the hits of `seen` as indexes into `rows`, the used-by counts of the rows drawn so far, and the chosen
    /// row. Nothing is written until Map.
    MapTarget(Box<MapTargetState>),
}

/// What the rows of Map to target… were built from: the scan, the read target and the stored target path.
type TargetStamp = (usize, usize, Option<std::path::PathBuf>);

/// What [`Pending::MapTarget`] holds (boxed: it is the largest variant by far).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapTargetState {
    id: String,
    query: String,
    rows: Vec<TargetPick>,
    hits: Vec<usize>,
    seen: Option<(String, TargetStamp)>,
    used: HashMap<usize, u32>,
    sel: Option<TargetPick>,
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
    /// Note JW1: document id, clipped strip and its hit rectangle across the gap to the card.
    pub strip: Option<(String, egui::Rect, egui::Rect)>,
    /// Note JW6: the last answer for this run, kept when opening another folder.
    pub last_choice: Option<DropChoice>,
    pub pending: Option<Pending>,
    pub(super) choice_options: Option<ChoiceOptions>,
    pub(super) folded: FoldedCache,
    /// Item 17: a file drop not yet taken by a slot card this frame (`ui::draw_with_scan` opens its folder after).
    pub file: Option<FileDrop>,
    pub slot_note: Option<SlotNote>,
    /// R4: each set card's height when last laid out, by set id; a card off screen allocates this instead.
    pub card_h: HashMap<String, f32>,
    /// Review I1: frames a reveal has re-aimed at a card not yet laid out (capped by `REVEAL_TRIES`).
    pub reveal_tries: u8,
    /// Note HF6: the set a filter change or a re-flow scrolls to. It scrolls only: it never opens the set's group.
    pub quiet_reveal: Option<crate::model::SetTarget>,
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
    pub reveal_seen: Option<crate::model::SetTarget>,
    /// Note FM32: the widget with the keyboard focus when the last frame's list ended. A stand-in that holds the focus
    /// and was not the holder then took it this frame (Tab), as opposed to a focused card the wheel scrolled away.
    pub focus: Option<egui::Id>,
    /// Note HP: the Sets filter last frame, to tell a change from the first frame (`None`).
    pub filter: Option<crate::model::sets_filter::SetsFilter>,
    /// Note HF6: the selection last frame (`None` on the first). A selection that moved opens the group of what it
    /// moved to: a click on a texture, Go to set.
    pub selection: Option<Selection>,
}

/// The answers for this opening. A weak reference lets the model reuse its views on a replan.
#[derive(Debug)]
pub(super) struct ChoiceOptions {
    item: String,
    set: String,
    generation: u64,
    views: std::sync::Weak<Vec<SetView>>,
    editable: bool,
    buttons: Vec<(DropChoice, Option<String>)>,
}

impl DropState {
    fn choice_buttons(
        &mut self,
        model: &AppModel,
        item: &str,
        set: &str,
    ) -> &[(DropChoice, Option<String>)] {
        let views = crate::model::sets::views(model);
        let generation = model.generation();
        let editable = model.sets_editable();
        if self.choice_options.as_ref().is_some_and(|cached| {
            cached.item != item
                || cached.set != set
                || cached.generation != generation
                || cached.views.as_ptr() != Arc::as_ptr(&views)
                || cached.editable != editable
        }) {
            self.choice_options = None;
        }
        let options = self.choice_options.get_or_insert_with(|| ChoiceOptions {
            item: item.to_owned(),
            set: set.to_owned(),
            generation,
            views: Arc::downgrade(&views),
            editable,
            buttons: model
                .drop_choices(item)
                .into_iter()
                .map(|c| (c, model.choice_refused(item, set, c)))
                .collect(),
        });
        &options.buttons
    }
}

/// Review I1: a reveal re-aims at most this many frames, so a key that names no visible card cannot loop.
const REVEAL_TRIES: u8 = 8;

/// R4: the height of a card never laid out, until any card has been measured (then their mean).
const CARD_EST: f32 = 110.0;

/// Note FM32: the rows `v`'s chips wrap into at column width `width` (`frame_x` is the card's side margins and
/// the chip rows' inset, Note HP). A count from the set's slot row, its
/// missing entries and its extra members, not a layout; the chips sit `DOCK_GAP_X` apart.
// shortcut: a docked or folded helper still counts as an extra member here, and a helper tile counts as a full chip,
// so an unmeasured card may be estimated a row too tall (drawing corrects it, FM32); count them apart if a scroll jump shows.
fn chip_rows(v: &SetView, width: f32, frame_x: f32) -> usize {
    let chips = view_row(v).len() - v.empty.len()
        + v.missing.len()
        + v.members.len().saturating_sub(v.slots.len());
    chips.div_ceil(chips_per_row(width, frame_x))
}

/// The chips one row of a card `width` wide holds (at least one).
fn chips_per_row(width: f32, frame_x: f32) -> usize {
    ((width - frame_x + DOCK_GAP_X) / (CHIP + DOCK_GAP_X))
        .floor()
        .max(1.0) as usize
}

/// Note FM32: a new column width moves every remembered card height by the chip rows it adds or removes, so a card off
/// screen is no longer the height of the old width (the scroll bar length stays right without drawing it). A card not
/// in `views` any more is forgotten. The estimate leaves out the name row, which wraps below the pill when narrow;
/// drawing the card measures it exactly. Returns whether the width changed from a known one.
fn reflow_heights(st: &mut DropState, views: &[SetView], width: f32, frame_x: f32) -> bool {
    let old = std::mem::replace(&mut st.card_w, width);
    if old <= 0.0 || (old - width).abs() < 0.5 {
        return false;
    }
    let by_id: HashMap<&str, &SetView> = views.iter().map(|v| (v.id.as_str(), v)).collect();
    st.card_h.retain(|id, h| {
        let Some(v) = by_id.get(id.as_str()) else {
            return false;
        };
        let rows = |w: f32| chip_rows(v, w, frame_x) as f32;
        *h = (*h + (rows(width) - rows(old)) * (CHIP + DOCK_GAP_Y)).max(0.0);
        true
    });
    true
}

/// The warning tooltip shows this many lines, then "… and K more".
const TIP_LINES: usize = 10;

enum Act {
    /// The warning badge: open Problems filtered to this set key.
    Problems(String),
    SelectItem(String),
    Expand(String),
    Drop(String, String, bool),
    ShareBorrow {
        item: String,
        from: String,
        set: String,
    },
    NewSet(String),
    Reorder(String, usize),
    OpenMoveTo(String),
    /// Notes HG, HL4: open the Map to target… dialog on a set.
    MapTarget(String),
    /// Note HG: remove a set's mapping.
    UnmapRecord(String),
    OpenRename(String),
    Delete(String),
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
    /// Note HF6: a click on a group's header folds or opens it.
    ToggleGroup(String),
    GroupExcluded(String, bool),
    OpenRenameGroup(String),
    Ungroup(String),
    /// "Select all sets" on a header.
    SelectGroup(String),
    /// A set (id) let go on a group's header (name).
    JoinGroup(String, String),
    /// The set menu's pick: these sets (ids) go to a group or none.
    MoveToGroup(Vec<String>, GroupTarget),
    /// The set menu's New group…: ask for a name.
    OpenNewGroup(Vec<String>),
    /// Note HF9: Export group (name).
    ExportGroup(String),
    /// Note HF9: Export set (key, id).
    ExportSet(String, String),
}

/// The card's heading is one row only when the name keeps this much beside the pill and badges; otherwise the pill
/// and badges go under the name (Note FI13).
const NAME_MIN: f32 = 80.0;
/// The pill frame's horizontal margin (`status_pill`) and the status dot's width (`status::dot`, 8 px).
const PILL_PAD_X: f32 = 6.0;
const STATUS_DOT: f32 = 8.0;
/// The size of a member thumbnail (`columns::member`), which an empty chip matches.
const CHIP: f32 = 32.0;

/// Note JW1: the empty strip's inner margin and gap from the card.
const STRIP_PAD: f32 = 4.0;
const STRIP_GAP: f32 = 2.0;

fn strip_size(n: usize, per_row: usize) -> egui::Vec2 {
    let cols = n.min(per_row);
    let rows = n.div_ceil(per_row);
    egui::vec2(
        cols as f32 * CHIP + cols.saturating_sub(1) as f32 * DOCK_GAP_X,
        rows as f32 * CHIP + rows.saturating_sub(1) as f32 * DOCK_GAP_Y,
    ) + egui::Vec2::splat(2.0 * STRIP_PAD)
}

fn strip_rect(card: egui::Rect, size: egui::Vec2, clip: egui::Rect) -> egui::Rect {
    let below = card.bottom() + STRIP_GAP;
    let above = card.top() - STRIP_GAP - size.y;
    let y = if below + size.y <= clip.bottom() {
        below
    } else if above >= clip.top() {
        above
    } else {
        clip.bottom() - size.y
    };
    let x = card.left().min(clip.right() - size.x).max(clip.left());
    egui::Rect::from_min_size(egui::pos2(x, y), size)
}

/// Note JW1: paint only. The clip can cut through a box, without moving its letter.
fn paint_strip(
    p: &egui::Painter,
    style: &egui::Style,
    v: &SetView,
    rect: egui::Rect,
    per_row: usize,
) {
    let vis = &style.visuals;
    p.rect(
        rect,
        2.0,
        vis.window_fill(),
        vis.window_stroke(),
        egui::StrokeKind::Inside,
    );
    // The view has already formatted these letters in row order.
    for (i, letter) in v
        .empty_line
        .trim_start_matches("Empty: ")
        .split(", ")
        .enumerate()
    {
        let offset = egui::vec2(
            (i % per_row) as f32 * (CHIP + DOCK_GAP_X),
            (i / per_row) as f32 * (CHIP + DOCK_GAP_Y),
        );
        let cell = egui::Rect::from_min_size(
            rect.min + egui::Vec2::splat(STRIP_PAD) + offset,
            egui::Vec2::splat(CHIP),
        );
        p.rect(
            cell,
            2.0,
            vis.extreme_bg_color,
            vis.widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        paint_letter_on(p, style, cell, letter, true, f32::INFINITY);
    }
}

/// An empty slot (or a missing entry, outlined): a dim box with the letter. K11: named by the letter.
/// `filled` (a linked slot with no thumbnail to show): the box in the filled colours, named by what it returns (built
/// when read).
fn letter_box(
    ui: &mut egui::Ui,
    letter: &str,
    outline: Option<egui::Color32>,
    filled: Option<&dyn Fn() -> String>,
) -> egui::Response {
    let r = letter_box_painted(ui, letter, outline, filled.is_some());
    // an empty chip's name is "<letter>, empty"; a missing entry's outlined "?" keeps its own
    r.widget_info(|| {
        let name = match (outline, filled) {
            (_, Some(f)) => f(),
            (None, None) => format!("{letter}, empty"),
            (Some(_), None) => letter.to_string(),
        };
        egui::WidgetInfo::labeled(egui::Role::Label, true, name)
    });
    r
}

/// `letter_box`'s drawing with no AccessKit info, for a caller that registers its own (a cleared chip, a typed path's
/// box): a Label's name would stay behind as the node's value under the caller's Button (final wave Group 5).
fn letter_box_painted(
    ui: &mut egui::Ui,
    letter: &str,
    outline: Option<egui::Color32>,
    filled: bool,
) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(egui::Vec2::splat(CHIP), egui::Sense::hover());
    let vis = ui.visuals();
    let stroke = match outline {
        Some(c) => egui::Stroke::new(2.0, c),
        None if filled => vis.widgets.inactive.bg_stroke,
        None => vis.widgets.noninteractive.bg_stroke,
    };
    let bg = if filled {
        vis.widgets.inactive.bg_fill
    } else {
        vis.extreme_bg_color
    };
    let p = ui.painter();
    p.rect_filled(rect, 2.0, bg);
    p.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
    if outline.is_none() {
        paint_letter(ui, rect, letter, !filled, f32::INFINITY);
    } else {
        // The missing entry's "?" stays centred.
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            letter,
            egui::TextStyle::Small.resolve(ui.style()),
            dim_letter_colour(ui.visuals()),
        );
    }
    r
}

/// The slot lock's cell on a member chip (`columns::paint_locks`, the one copy of the geometry).
const LOCK_W: f32 = super::columns::LOCK_GLYPH;
const LOCK_PAD: f32 = super::columns::LOCK_INSET;
/// The link mark's top edge below the chip's top.
const MARK_TOP: f32 = 2.0;

/// Review E1: the mark of a linked slot, two overlapping rings (a chain) in `r`. No glyph: no emoji font is loaded.
pub(crate) fn link_mark(p: &egui::Painter, r: egui::Rect, c: egui::Color32) {
    let radius = r.height() * 0.28;
    let dx = r.width() * 0.2;
    let stroke = egui::Stroke::new(1.5, c);
    for sign in [-1.0, 1.0] {
        p.circle_stroke(r.center() + egui::vec2(sign * dx, 0.0), radius, stroke);
    }
}

/// A linked slot's chip name: "{letter}, from set {short}", or "{letter}, path {file}" for a typed path.
fn linked_chip_name(letter: &str, l: &SlotLink) -> String {
    match &l.from_short {
        Some(s) => format!("{letter}, from set {s}"),
        None => {
            // a path ending in a separator has no file part: name the whole path
            let file = match l.path.rsplit(['\\', '/']).next() {
                Some(f) if !f.is_empty() => f,
                _ => &l.path,
            };
            format!("{letter}, path {file}")
        }
    }
}

/// Note HP: the dimmed letter's colour on an empty chip (read at 4.5:1 on `extreme_bg_color` in both themes).
pub fn dim_letter_colour(vis: &egui::Visuals) -> egui::Color32 {
    vis.widgets.inactive.text_color()
}

/// Note HP: a tile's label in its bottom-left corner. `dim` (an empty chip, no thumbnail under it) is weak text with
/// no backing; otherwise white on a dark backing so it reads over any thumbnail. Text wider than the chip is cut with `…`.
/// `max_w` cuts the text with `…` (trailing-row tags); chip letters pass `f32::INFINITY` and are never cut.
fn paint_letter(ui: &egui::Ui, rect: egui::Rect, text: &str, dim: bool, max_w: f32) {
    paint_letter_on(ui.painter(), ui.style(), rect, text, dim, max_w);
}

fn paint_letter_on(
    p: &egui::Painter,
    style: &egui::Style,
    rect: egui::Rect,
    text: &str,
    dim: bool,
    max_w: f32,
) {
    let font = egui::TextStyle::Small.resolve(style);
    let colour = if dim {
        dim_letter_colour(&style.visuals)
    } else {
        egui::Color32::WHITE
    };
    let lay = |s: String| p.layout_no_wrap(s, font.clone(), colour);
    let mut g = lay(text.to_string());
    let mut shown = text.to_string();
    while g.size().x > max_w && shown.pop().is_some() {
        g = lay(format!("{}…", shown.trim_end()));
    }
    let at = rect.left_bottom() + egui::vec2(1.0, -1.0 - g.size().y);
    if !dim {
        p.rect_filled(
            egui::Rect::from_min_size(at, g.size()).expand(1.0),
            2.0,
            egui::Color32::from_black_alpha(170),
        );
    }
    p.galley(at, g, colour);
}

/// Note JW7: a non-helper trailing tile paints its slot letter, or its member tail.
/// `kind` is the slot the member owns in the set, if any. The tail is read with `v.skin || v.shared`, as the input
/// names are (`input_name.rs`).
fn tile_tag(model: &AppModel, v: &SetView, key: &str, kind: Option<SlotKind>) -> String {
    if let Some(k) = kind {
        return chip_letter(v, k);
    }
    let Some(item) = model.item(key) else {
        return String::new();
    };
    let stem = std::path::Path::new(&item.name)
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    tag_text(member_tail(&stem, v.tails(), &model.suffix_aliases))
}

fn tag_text(tail: &str) -> String {
    if tail.is_empty() { "—" } else { tail }.to_string()
}

/// Review E4 (controller ruling 10): what Ready means, on the Ready pill and a ready card's status hover.
pub const READY_MEANS: &str = "Ready: no problem row in the set or its textures. Notes, rows you marked as intended and checks don't count.";
/// Review E4 (controller ruling 10): what Not ready means, on the Not ready pill and an amber card's status hover.
pub const NOT_READY_MEANS: &str = "Not ready: a problem row in the set or one of its textures. Notes, rows you marked as intended and checks don't count.";
/// An amber card's status hover and the set editor's status line for a set with no engine key and no counted row
/// (`bottom::no_files_yet`, Note JP5).
pub const NO_FILES_YET: &str = "Not ready: this set has no files yet.";

/// The pill's hover (R15): the long badge, then what the status `s` means (review E4) or blocked, then one line each
/// for owns, missing entries and recipes. `flagged` is the frame's one fetch (`status_pill`).
pub fn pill_hover(model: &AppModel, flagged: &Flagged, v: &SetView, s: Status) -> String {
    let mut lines = vec![badge_text(model, v).to_string()];
    match s {
        Status::Blocks => lines.push("blocked".to_string()),
        Status::Ready => lines.push(READY_MEANS.to_string()),
        // Note JP5 (B3-17): a set with no key and no row of its own has nothing to point at.
        Status::Warning if no_files_yet(flagged, v) => lines.push(NO_FILES_YET.to_string()),
        Status::Warning => lines.push(NOT_READY_MEANS.to_string()),
        _ => {}
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
    if !v.empty_line.is_empty() {
        lines.push(v.empty_line.clone());
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
        acts.push(Act::Problems(scope_key(v).to_string()));
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

/// The longest form of a set id that `fits` and keeps `tail` (the view's `short`, the shortest tail no other set
/// shares): the id, its middle elided, then `…/<tail>`. Note HP (SC4): when the search lies inside one middle
/// segment, `…/<that segment>/<rest>` is tried right after the id. When nothing fits, `…/<tail>` is returned anyway
/// (the whole id when the tail is the id, or when `…/<tail>` would be no shorter than it: final wave Group 6, ledger
/// 187) and the title wraps; two cards never read the same (review E24). The bool says whether the title fits.
fn fit_title(
    id: &str,
    tail: &str,
    query: &str,
    mut fits: impl FnMut(&str) -> bool,
) -> (String, bool) {
    use super::header::elide_middle;
    let len = id.chars().count();
    let whole_fits = fits(id);
    if whole_fits || tail == id {
        return (id.to_string(), whole_fits);
    }
    let q = engine::sets::fold_id(query.trim());
    if !q.is_empty() {
        let segs: Vec<&str> = id.split('/').collect();
        let middle = 1..segs.len().saturating_sub(1);
        if let Some(i) = middle
            .into_iter()
            .find(|&i| engine::sets::fold_id(segs[i]).contains(&q))
        {
            let t = format!("…/{}", segs[i..].join("/"));
            if t.ends_with(tail) && fits(&t) {
                return (t, true);
            }
        }
    }
    let mut budget = len.saturating_sub(1);
    while budget > 0 {
        let t = elide_middle(id, budget);
        // Every intermediate budget gives the same component suffix.
        budget = budget.min(t.chars().count()).saturating_sub(1);
        if !t.contains('/') {
            break;
        }
        if t.ends_with(tail) && fits(&t) {
            return (t, true);
        }
    }
    let last = format!("…/{tail}");
    if last.chars().count() < len {
        let last_fits = fits(&last);
        (last, last_fits)
    } else {
        (id.to_string(), false)
    }
}

/// The preflight and the label can have different room after egui places the badges.
struct ShortTitles {
    forms: Vec<(Arc<egui::RichText>, f32)>,
    minimum: f32,
    fallback: Arc<egui::RichText>,
}

impl ShortTitles {
    fn new(ui: &egui::Ui, v: &SetView, query: &str, font: &egui::FontId) -> Self {
        #[cfg(feature = "test-hooks")]
        super::paint_counts::Work::ShortTitles.built();
        let mut forms = Vec::<(Arc<egui::RichText>, f32)>::new();
        // Visit the forms in display order, including the fallback when none fits.
        let (fallback, _) = fit_title(&v.id, &v.short, query, |text| {
            if text != v.id && !forms.iter().any(|(form, _)| form.text() == text) {
                forms.push((
                    Arc::new(egui::RichText::new(text).strong()),
                    text_width(ui, text, font),
                ));
            }
            false
        });
        let minimum = forms.iter().map(|(_, w)| *w).fold(f32::INFINITY, f32::min);
        Self {
            forms,
            minimum,
            fallback: Arc::new(egui::RichText::new(fallback).strong()),
        }
    }

    fn at(&self, room: f32) -> (Arc<egui::RichText>, bool) {
        self.forms
            .iter()
            .find(|(_, width)| *width <= room)
            .map_or_else(
                || (self.fallback.clone(), false),
                |(form, _)| (form.clone(), true),
            )
    }
}

/// The width of `text` in the small font, as a button or pill would paint it.
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

/// Shown only while a result is not marked (a marked one leaves the badge, review E4); its hover lists the `marked`
/// ones last, under `MARKED_LINE` (final fix wave A6).
fn check_badge(
    ui: &mut egui::Ui,
    v: &SetView,
    checks: &[(&str, &Warning)],
    marked: &[(&str, &Warning)],
    acts: &mut Vec<Act>,
) {
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
            egui::Role::Button,
            true,
            if n == 0 {
                "Check results: info only".to_string()
            } else {
                format!("Check results: {n}")
            },
        )
    });
    let r = r.on_hover_ui(|ui| {
        let done = (!marked.is_empty()).then(|| warning_tooltip(marked));
        let lines = crate::model::bottom::with_marked(
            vec![warning_tooltip(checks)],
            done.into_iter().collect(),
        );
        ui.label(lines.join("\n"));
    });
    if r.clicked() {
        acts.push(Act::Problems(scope_key(v).to_string()));
    }
}

/// The status pill: the status dot and the short kind (cut on a narrow card), in a rounded frame, the long text on
/// hover. The caller works out `s` from one `flagged` fetch per frame (Task 6) and passes that `flagged` in.
fn status_pill(ui: &mut egui::Ui, model: &AppModel, flagged: &Flagged, v: &SetView, s: Status) {
    use super::status::dot;
    egui::Frame::new()
        .corner_radius(8.0)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(egui::Margin::symmetric(PILL_PAD_X as i8, 1))
        .show(ui, |ui| {
            let kind = egui::Label::new(
                egui::RichText::new(kind_text(model, v))
                    .small()
                    .color(ui.visuals().widgets.inactive.text_color()),
            )
            .truncate();
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
        .on_hover_ui(|ui| {
            ui.set_max_width(ui.spacing().tooltip_width);
            ui.label(pill_hover(model, flagged, v, s));
        });
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
    remove: bool,
    drag: ChipDrag<'_>,
    note: Option<&dyn Fn() -> String>,
    sel_items: &HashSet<String>,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
) -> Option<egui::Response> {
    let item = model.item(mk)?;
    let (locked, pgp) = chip_locks(v, kind);
    let r = member(ui, item, sel_items.contains(mk), locked, pgp, thumbs, note);
    member_acts(model, v, mk, kind, remove, drag, &r, acts, member_menu);
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

/// Note HP: a helper badge is drawn 16 px, centred on its chip's top corner; its hit rect is 20 px before `clamp_hits`.
const BADGE: f32 = 16.0;
const BADGE_HIT: f32 = 20.0;

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

/// A dock cell on a chip: the drawn rect and the unclamped hit rect, both centred on the chip's top-right (position 0)
/// or top-left (1) corner.
fn badge_rects(chip: egui::Rect, pos: usize) -> (egui::Rect, egui::Rect) {
    let c = if pos == 0 {
        chip.right_top()
    } else {
        chip.left_top()
    };
    let at = |side: f32| egui::Rect::from_center_size(c, egui::Vec2::splat(side));
    (at(BADGE), at(BADGE_HIT))
}

/// Cuts `r` back along its thinner overlap with `o`: the edge on `o`'s side moves to `o`'s edge. No overlap, no change.
fn cut_from(r: &mut egui::Rect, o: egui::Rect) {
    let i = r.intersect(o);
    if i.width() <= 0.0 || i.height() <= 0.0 {
        return;
    }
    if i.width() <= i.height() {
        if r.center().x < o.center().x {
            r.max.x = o.min.x;
        } else {
            r.min.x = o.max.x;
        }
    } else if r.center().y < o.center().y {
        r.max.y = o.min.y;
    } else {
        r.min.y = o.max.y;
    }
}

/// Note HP: the badge hit rects (each with the index in `fixed` of its own chip) cut so none leaves `card` or meets a
/// `fixed` rect other than its own chip (pass 1), then split at the midline of every badge-badge overlap (pass 2).
/// Both passes cut along the thinner overlap, and the drawn 16 px rect always stays inside a hit.
// shortcut: three badges meeting in one spot are split pairwise in list order, not at a common point; the dock draws
// at most two per chip 16 px apart, so this does not arise now.
fn clamp_hits(
    hits: &[(egui::Rect, usize)],
    fixed: &[egui::Rect],
    card: egui::Rect,
) -> Vec<egui::Rect> {
    let mut out: Vec<egui::Rect> = hits
        .iter()
        .map(|&(h, own)| {
            let mut r = h.intersect(card);
            for (j, f) in fixed.iter().enumerate() {
                if j != own {
                    cut_from(&mut r, *f);
                }
            }
            r
        })
        .collect();
    for a in 0..out.len() {
        for b in a + 1..out.len() {
            let i = out[a].intersect(out[b]);
            if i.width() <= 0.0 || i.height() <= 0.0 {
                continue;
            }
            let (ra, rb) = (out[a], out[b]);
            let mid = i.center();
            if i.width() <= i.height() {
                let (left, right) = if ra.center().x < rb.center().x {
                    (a, b)
                } else {
                    (b, a)
                };
                out[left].max.x = mid.x;
                out[right].min.x = mid.x;
            } else {
                let (top, bottom) = if ra.center().y < rb.center().y {
                    (a, b)
                } else {
                    (b, a)
                };
                out[top].max.y = mid.y;
                out[bottom].min.y = mid.y;
            }
        }
    }
    out
}

/// The slot menu of card `k` of `v`, inside a chip's context menu.
fn slot_menu_ui(
    ui: &mut egui::Ui,
    model: &AppModel,
    v: &SetView,
    k: SlotKind,
    acts: &mut Vec<Act>,
) {
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

/// A chip with no member under it (a cleared slot, a linked typed path) gets the slot menu, so a clear or a typed link
/// can be undone from the card.
fn slot_box_menu(
    model: &AppModel,
    v: &SetView,
    k: SlotKind,
    r: &egui::Response,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
) {
    *member_menu |= r.secondary_clicked();
    let open = r.context_menu(|ui| slot_menu_ui(ui, model, v, k, acts));
    *member_menu |= open.is_some();
}

/// A member chip's menu, drag payload and clicks: click selects, double-click expands, a slot member has the slot
/// menu, and an own member can be removed.
#[allow(clippy::too_many_arguments)]
fn member_acts(
    model: &AppModel,
    v: &SetView,
    mk: &str,
    kind: Option<SlotKind>,
    remove: bool,
    drag: ChipDrag<'_>,
    r: &egui::Response,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
) {
    // Note JW4: only own members can leave the set through this menu.
    if kind.is_some() || remove {
        *member_menu |= r.secondary_clicked();
        let open = r.context_menu(|ui| {
            if let Some(k) = kind {
                slot_menu_ui(ui, model, v, k, acts);
            }
            if remove {
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
    if r.drag_started() {
        match drag {
            ChipDrag::Item => r.dnd_set_drag_payload(DragItem(mk.to_string())),
            ChipDrag::Borrow(from) => r.dnd_set_drag_payload(DragBorrow {
                item: mk.into(),
                from: from.into(),
            }),
            ChipDrag::None => {}
        }
    }
    if r.double_clicked() {
        acts.push(Act::Expand(mk.to_string()));
    } else if r.clicked() {
        acts.push(Act::SelectItem(mk.to_string()));
    }
}

/// Note JW7: docked badges and trailing helpers paint the same thumbnail and selection stroke.
fn paint_helper(
    ui: &egui::Ui,
    thumbs: &mut ThumbCache,
    item: &TextureItem,
    rect: egui::Rect,
    selected: bool,
) {
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
        let stroke = if selected {
            egui::Stroke::new(2.0, ui.visuals().selection.stroke.color)
        } else {
            egui::Stroke::new(1.0, ui.visuals().panel_fill)
        };
        ui.painter()
            .rect_stroke(rect, 1.0, stroke, egui::StrokeKind::Inside);
    }
}

/// Note JW7: a helper that has no dock gets a BADGE thumbnail in a BADGE_HIT square with no painted tag.
#[allow(clippy::too_many_arguments)]
fn helper_tile(
    ui: &mut egui::Ui,
    model: &AppModel,
    thumbs: &mut ThumbCache,
    v: &SetView,
    mk: &str,
    role: &str,
    sel_items: &HashSet<String>,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
) -> Option<egui::Response> {
    let item = model.item(mk)?;
    let (hit, r) =
        ui.allocate_exact_size(egui::Vec2::splat(BADGE_HIT), egui::Sense::click_and_drag());
    let rect = egui::Rect::from_center_size(hit.center(), egui::Vec2::splat(BADGE));
    paint_helper(ui, thumbs, item, rect, sel_items.contains(mk));
    r.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{}, {role}", item.name))
    });
    member_acts(
        model,
        v,
        mk,
        None,
        true,
        ChipDrag::Item,
        &r,
        acts,
        member_menu,
    );
    let text = || format!("{}\n{role}", item.name);
    ui.ctx()
        .accesskit_node_builder(r.id, |b| b.set_description(text()));
    Some(r.on_hover_ui(|ui| {
        ui.label(text());
    }))
}

/// Note JW7: one row per hidden helper on this chip. Its name lists its folded fills in dock order.
fn folded_row(
    ui: &mut egui::Ui,
    model: &AppModel,
    v: &SetView,
    helper: &FoldedHelper,
    name: &str,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
) {
    ui.horizontal(|ui| {
        let r = ui.add(egui::Button::new(name).sense(egui::Sense::click_and_drag()));
        member_acts(
            model,
            v,
            &helper.key,
            None,
            false,
            ChipDrag::Item,
            &r,
            acts,
            member_menu,
        );
        let off = model.sets_read_only_reason();
        let remove = ui.add_enabled(off.is_none(), egui::Button::new("✕"));
        remove.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                off.is_none(),
                format!("Remove {} from set", helper.file),
            )
        });
        let remove = match off {
            Some(why) => remove.on_disabled_hover_text(why),
            None => remove.on_hover_text("Remove from set"),
        };
        if remove.clicked() {
            acts.push(Act::RemoveMember(v.id.clone(), helper.key.clone()));
            ui.close();
        }
    });
}

#[derive(Debug)]
struct FoldedHelper {
    key: String,
    file: String,
    fills: Vec<engine::helper::FillTarget>,
}

/// Each set's slot and hidden range, for one revision of the column's views.
type FoldedBySlice = HashMap<(SlotKind, usize, usize), Arc<[FoldedHelper]>>;

#[derive(Debug, Default)]
pub(super) struct FoldedCache {
    revisions: (u64, u64),
    by_set: HashMap<usize, FoldedBySlice>,
}

fn group_hidden(model: &AppModel, hidden: &[&Docked]) -> Arc<[FoldedHelper]> {
    #[cfg(feature = "test-hooks")]
    super::paint_counts::Work::FoldedGroups.built();
    let mut groups: Vec<FoldedHelper> = Vec::new();
    for d in hidden {
        if let Some(group) = groups.iter_mut().find(|g| g.key == d.helper) {
            group.fills.push(d.target);
        } else {
            #[cfg(feature = "test-hooks")]
            super::paint_counts::Work::FoldedItemLookups.built();
            if let Some(item) = model.item(&d.helper) {
                groups.push(FoldedHelper {
                    key: item.key.clone(),
                    file: item.name.clone(),
                    fills: vec![d.target],
                });
            }
        }
    }
    groups.into()
}

enum PlannedCell<'a> {
    Badge(&'a Docked, &'a TextureItem),
    More(SlotKind, Arc<[FoldedHelper]>),
}

/// One chip's rect, its index among the card's fixed rects and its drawable cells.
struct DockPlan<'a> {
    chip: egui::Rect,
    own: usize,
    cells: Vec<(usize, PlannedCell<'a>)>,
}

/// Note JW7: only helpers represented by a badge or an overflow row can be skipped on the trailing row.
fn plan_dock<'a>(
    model: &'a AppModel,
    chip: egui::Rect,
    own: usize,
    entries: &[&'a Docked],
    locks: (bool, bool),
    folded: &mut FoldedBySlice,
) -> DockPlan<'a> {
    let mut cells = Vec::new();
    for (pos, cell) in dock_cells(locks.0, locks.1, entries.len()) {
        let cell = match cell {
            Cell::Badge(i) => {
                let d = entries[i];
                let Some(item) = model.item(&d.helper) else {
                    continue;
                };
                PlannedCell::Badge(d, item)
            }
            Cell::More(k) => {
                let first = entries.len() - k;
                let slot = entries[first].slot;
                let groups = folded
                    .entry((slot, first, k))
                    .or_insert_with(|| group_hidden(model, &entries[first..]));
                if groups.is_empty() {
                    continue;
                }
                PlannedCell::More(slot, groups.clone())
            }
        };
        cells.push((pos, cell));
    }
    DockPlan { chip, own, cells }
}

/// Note HP: clamps every dock hit rect of one card at once (they must not meet each other, a chip, a tile or the
/// combo, nor leave the card), then draws the docks. Returns after the chips, so badges sit on top.
#[allow(clippy::too_many_arguments)]
fn draw_docks(
    ui: &mut egui::Ui,
    model: &AppModel,
    thumbs: &mut ThumbCache,
    v: &SetView,
    plans: &[DockPlan],
    fixed: &[egui::Rect],
    card: egui::Rect,
    sel_items: &HashSet<String>,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
) {
    let hits: Vec<(egui::Rect, usize)> = plans
        .iter()
        .flat_map(|p| {
            p.cells
                .iter()
                .map(move |(pos, _)| (badge_rects(p.chip, *pos).1, p.own))
        })
        .collect();
    let mut clamped = clamp_hits(&hits, fixed, card).into_iter();
    for p in plans {
        let hits: Vec<egui::Rect> = clamped.by_ref().take(p.cells.len()).collect();
        draw_dock(ui, model, thumbs, v, p, &hits, sel_items, acts, member_menu);
    }
}

/// Note HN: the helpers that fill one chip, drawn on its top corners. A badge is the helper's thumbnail with the
/// member behaviours (click selects, double-click expands, drag, the member menu). `+K` lists the folded helpers.
/// `hits` are the clamped hit rects of `plan.cells`, in order.
#[allow(clippy::too_many_arguments)]
fn draw_dock(
    ui: &mut egui::Ui,
    model: &AppModel,
    thumbs: &mut ThumbCache,
    v: &SetView,
    plan: &DockPlan,
    hits: &[egui::Rect],
    sel_items: &HashSet<String>,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
) {
    for ((pos, cell), &hit) in plan.cells.iter().zip(hits) {
        let (rect, _) = badge_rects(plan.chip, *pos);
        let pos = *pos;
        match cell {
            PlannedCell::Badge(d, item) => {
                let id = ui
                    .scope_id()
                    .with(("dock", &v.id, &d.helper, d.slot, d.channel));
                let r = ui.interact(hit, id, egui::Sense::click_and_drag());
                paint_helper(ui, thumbs, item, rect, sel_items.contains(&d.helper));
                let name = format!("{}, fills {}", item.name, fill_text(d.target));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
                member_acts(
                    model,
                    v,
                    &d.helper,
                    None,
                    true,
                    ChipDrag::Item,
                    &r,
                    acts,
                    member_menu,
                );
                let target = item
                    .helper
                    .as_ref()
                    .map(|h| helper_target_files(h, v.tails()));
                r.on_hover_text(match target {
                    Some(t) => format!("{}\n{t}", item.name),
                    None => item.name.clone(),
                });
            }
            PlannedCell::More(kind, groups) => {
                let id = ui.scope_id().with(("dock-more", &v.id, kind, pos));
                let names = std::cell::LazyCell::new(|| {
                    #[cfg(feature = "test-hooks")]
                    super::paint_counts::Work::FoldedNames.built();
                    groups
                        .iter()
                        .map(|group| {
                            let fills = group
                                .fills
                                .iter()
                                .copied()
                                .map(fill_text)
                                .collect::<Vec<_>>();
                            format!("{}, fills {}", group.file, fills.join(", "))
                        })
                        .collect::<Vec<_>>()
                });
                let k = groups.len();
                let r = ui.interact(hit, id, egui::Sense::click());
                if ui.is_rect_visible(rect) {
                    let font = egui::TextStyle::Small.resolve(ui.style());
                    ui.painter()
                        .rect_filled(rect, 2.0, egui::Color32::from_black_alpha(170));
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        // "+10" is wider than the 16 px cell: from ten up the cell says "+" and the hover lists them
                        if k < 10 {
                            format!("+{k}")
                        } else {
                            "+".to_string()
                        },
                        font,
                        egui::Color32::WHITE,
                    );
                }
                r.widget_info(|| {
                    let slot = chip_letter(v, *kind);
                    let fills = if k == 1 {
                        "helper fills"
                    } else {
                        "helpers fill"
                    };
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        format!("{k} more {fills} {slot} in {}", v.id),
                    )
                });
                const LIST_HINT: &str = "Click to list them.";
                ui.ctx().accesskit_node_builder(r.id, |node| {
                    node.set_description(format!("{}\n{LIST_HINT}", names.join("\n")));
                });
                egui::Popup::menu(&r)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        for (group, name) in groups.iter().zip(names.iter()) {
                            folded_row(ui, model, v, group, name, acts, member_menu);
                        }
                    });
                r.on_hover_ui(|ui| {
                    for name in names.iter() {
                        ui.label(name);
                    }
                    ui.label(LIST_HINT);
                });
            }
        }
    }
}

/// A member chip's name: its letter, and when helpers fill it, its file and their files ("D, crate.png, filled by x").
fn chip_name(model: &AppModel, letter: &str, member: &str, entries: &[&Docked]) -> String {
    match model.item(member) {
        Some(item) if !entries.is_empty() => {
            let mut files: Vec<&str> = vec![];
            for d in entries {
                let f = model.item(&d.helper).map_or(&*d.helper, |i| &*i.name);
                if !files.contains(&f) {
                    files.push(f);
                }
            }
            format!("{letter}, {}, filled by {}", item.name, files.join(", "))
        }
        _ => letter.to_string(),
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
    /// An export runs: Export set / group are off (Note HF10).
    exporting: bool,
}

/// Why the ≡ handle and Move up / Move down are off while the Sets filter is on.
const CLEAR_TO_REORDER: &str = "Clear the filter to reorder sets, or use Move to…";

/// R17: the search box, the pills with their counts and the shader popup, under the heading.
fn filter_rows(ui: &mut egui::Ui, model: &mut AppModel) {
    let search = ui.add(
        egui::TextEdit::singleline(&mut model.sets_filter.search)
            .margin(crate::look::TEXT_EDIT_MARGIN)
            .hint_text("Search sets")
            .desired_width(f32::INFINITY),
    );
    // A hint is only a placeholder, so the box gets an accessible name too (after `ui.add`, rule 8.3).
    let text = model.sets_filter.search.as_str();
    search.widget_info(|| egui::WidgetInfo {
        label: Some("Search sets".into()),
        ..egui::WidgetInfo::text_edit(true, text, text, "Search sets")
    });
    let c = model.set_counts();
    ui.horizontal_wrapped(|ui| {
        let f = &mut model.sets_filter;
        // The hover is (lead, what ready means), joined only when it shows.
        for (on, text, n, hover) in [
            (
                &mut f.attention,
                format!("Not ready {}", c.attention),
                c.attention,
                ("Only sets that are not ready.", Some(NOT_READY_MEANS)),
            ),
            (
                &mut f.ready,
                format!("Ready {}", c.ready),
                c.ready,
                ("Only sets that are ready.", Some(READY_MEANS)),
            ),
            (
                &mut f.no_json,
                format!("No JSON {}", c.no_json),
                c.no_json,
                ("Only sets that write no JSON", None),
            ),
        ] {
            // Note HP SC5: a pill that would show nothing is weak, and stays clickable.
            let text = if n == 0 && !*on {
                egui::RichText::new(text).color(ui.visuals().weak_text_color())
            } else {
                egui::RichText::new(text)
            };
            if ui
                .add(
                    egui::Button::selectable(*on, text)
                        .corner_radius(10)
                        .frame_when_inactive(true),
                )
                .on_hover_ui(|ui| {
                    match hover {
                        (lead, Some(means)) => ui.label(format!("{lead} {means}")),
                        (lead, None) => ui.label(lead),
                    };
                })
                .clicked()
            {
                *on = !*on;
            }
        }
        let (text, name) = match f.shader {
            None => ("Filter ▾", "Filter, Sets shader filter".to_string()),
            Some(s) => (
                "Filter (1) ▾",
                format!("Filter (1), Sets shader filter: {}", s.label()),
            ),
        };
        // A9: a saved shader the Mode does not offer is listed too, so it is never invisible.
        let mut shaders = shader_options(model.target);
        if let Some(s) = f.shader.filter(|s| !shaders.contains(s)) {
            shaders.push(s);
        }
        let r = crate::ui::path_picker::menu(ui, text, |ui| {
            if ui
                .selectable_label(model.sets_filter.shader.is_none(), "Any")
                .clicked()
            {
                model.sets_filter.shader = None;
                ui.close();
            }
            let counts = model.sets_with_shaders(&shaders);
            for (s, n) in shaders.into_iter().zip(counts) {
                // A9: a shader with no sets stays pickable, in the weak colour.
                let label = format!("{} {n}", s.label());
                let label = if n == 0 {
                    egui::RichText::new(label).color(ui.visuals().weak_text_color())
                } else {
                    egui::RichText::new(label)
                };
                if ui
                    .selectable_label(model.sets_filter.shader == Some(s), label)
                    .clicked()
                {
                    model.sets_filter.shader = Some(s);
                    ui.close();
                }
            }
        })
        .response;
        // The Textures column has its own "Filter ▾", so this one is named apart.
        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
        r.on_hover_text("Only sets with this shader");
    });
}

/// Note FC13: one line when the filter hides the selected set (CD13), one when a set made under it is shown (CD16).
/// Returns whether its own **Clear filter** button is drawn, so the "No sets match" line adds no second one.
fn filter_strips(
    ui: &mut egui::Ui,
    model: &mut AppModel,
    views: &[crate::model::sets::SetView],
    visible: &[usize],
) -> bool {
    let mut button = false;
    if let Some(i) = model.hidden_set_index(visible) {
        let id = &views[i].id;
        button = true;
        let text = format!("{id} is hidden by the Sets filter");
        // Note HN F2: the sentence wraps in full, the button sits on the next row.
        ui.add(egui::Label::new(&text).wrap());
        if ui.button("Clear filter").clicked() {
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
    if !v.empty_line.is_empty() {
        n.set_description(v.empty_line.clone());
    }
    n.set_selected(selected);
    n.add_action(egui::accesskit::Action::Focus);
    n.add_action(egui::accesskit::Action::ScrollIntoView);
    n.set_bounds(egui::accesskit::Rect {
        x0: rect.min.x.into(),
        y0: rect.min.y.into(),
        x1: rect.max.x.into(),
        y1: rect.max.y.into(),
    });
}

/// Note FM32 (R4): a card outside the clip rect allocates `size` and stands in with the card's widget id: a Tab stop
/// and an AccessKit node named for the set. Returns its rect and whether the card is wanted in view: it took the
/// focus this frame (`held` is the holder last frame) or got an AccessKit ScrollIntoView request. K9: `allocate_space` takes one auto id, as the card's scope does, so later cards keep theirs.
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
    let mut wanted = ctx.memory_mut(|m| {
        m.interested_in_focus(id, layer);
        m.focused() == Some(id) && held != Some(id)
    });
    // egui consumes ScrollIntoView only in a registered widget; a stand-in has none, so read the request here.
    wanted |=
        ctx.input(|i| i.has_accesskit_action_request(id, egui::accesskit::Action::ScrollIntoView));
    ctx.accesskit_node_builder(id, |n| card_node(n, v, at, selected, rect));
    (rect, wanted)
}

/// Note HF6: a group's members sit this far in from the header's left (owner: indented; the number is mine).
const GROUP_INDENT: f32 = 12.0;

/// What the column draws, top to bottom: a group's header (an index into the heads) or a set's card (an index into
/// the views).
enum Row {
    Header(usize),
    Card(usize),
}

/// A group's header this frame, read from the views: `total` counts its members that have a card, `shown` those that
/// pass the filter, `status` is the worst of the shown ones.
struct Head {
    name: String,
    total: usize,
    shown: usize,
    status: Status,
    excluded: bool,
    collapsed: bool,
}

impl Head {
    /// `7`, or `2 of 7` while a filter hides some.
    fn count_text(&self) -> String {
        if self.shown < self.total {
            format!("{} of {}", self.shown, self.total)
        } else {
            self.total.to_string()
        }
    }

    /// The header's AccessKit name: `Group Armor, 7 sets, warning, collapsed`.
    fn node_name(&self) -> String {
        let sets = if self.total == 1 && self.shown == 1 {
            "1 set".to_string()
        } else {
            format!("{} sets", self.count_text())
        };
        let state = if self.collapsed {
            "collapsed"
        } else {
            "expanded"
        };
        let tag = if self.excluded { ", excluded" } else { "" };
        format!(
            "Group {}, {sets}, {}, {state}{tag}",
            self.name,
            self.status.label()
        )
    }
}

/// Blocks, then warning, then ready: the worst of a group's sets is the header's dot.
fn rank(s: Status) -> u8 {
    match s {
        Status::Blocks => 2,
        Status::Warning => 1,
        _ => 0,
    }
}

/// Note HF6: the rows of the column, the headers they point at, and each view's group (an index into the heads; empty
/// when there are no groups). A header comes before its first shown member and only when some member passes; a
/// collapsed group draws no member. The views are already in group order (`pull_groups`).
fn build_rows(
    model: &AppModel,
    views: &[SetView],
    visible: &[usize],
    status_of: &dyn Fn(&SetView) -> Status,
) -> (Vec<Row>, Vec<Head>, Vec<Option<usize>>) {
    let stored = model
        .sets_doc
        .as_ref()
        .map(|d| d.groups())
        .unwrap_or_default();
    if stored.is_empty() {
        return (
            visible.iter().map(|&i| Row::Card(i)).collect(),
            vec![],
            vec![],
        );
    }
    // A set named by two groups belongs to the first, as in the view order.
    let mut of: HashMap<String, usize> = HashMap::new();
    for (g, group) in stored.iter().enumerate() {
        for s in &group.sets {
            of.entry(fold_id(s)).or_insert(g);
        }
    }
    let grouped: Vec<Option<usize>> = views
        .iter()
        .map(|v| of.get(&fold_id(&v.id)).copied())
        .collect();
    let mut heads: Vec<Head> = stored
        .into_iter()
        .map(|g| Head {
            collapsed: model.group_collapsed(&g.name),
            name: g.name,
            total: 0,
            shown: 0,
            status: Status::Ready,
            excluded: g.excluded,
        })
        .collect();
    for g in grouped.iter().flatten() {
        heads[*g].total += 1;
    }
    for &i in visible {
        if let Some(g) = grouped[i] {
            let s = status_of(&views[i]);
            let h = &mut heads[g];
            h.shown += 1;
            if rank(s) > rank(h.status) {
                h.status = s;
            }
        }
    }
    let mut rows = Vec::with_capacity(visible.len() + heads.len());
    let mut placed = vec![false; heads.len()];
    for &i in visible {
        match grouped[i] {
            Some(g) => {
                if !std::mem::replace(&mut placed[g], true) {
                    rows.push(Row::Header(g));
                }
                if !heads[g].collapsed {
                    rows.push(Row::Card(i));
                }
            }
            None => rows.push(Row::Card(i)),
        }
    }
    (rows, heads, grouped)
}

/// Note HF6: what a reveal (`nav.reveal_set`: Move to…, the palette) or a selection that moved (a click on a texture,
/// Go to set) leads to is shown, so the group it sits in opens. A filter that hides the set leaves the group as it
/// is. `seen` is last frame's selection: the first frame only records it.
fn open_groups_of(
    model: &mut AppModel,
    views: &[SetView],
    visible: &[usize],
    seen: &mut Option<Selection>,
    reveal: Option<usize>,
) {
    let moved = seen.as_ref().is_some_and(|s| *s != model.selection);
    if seen.as_ref() != Some(&model.selection) {
        *seen = Some(model.selection.clone());
    }
    let moved = moved.then(|| model.resolved_selection().index).flatten();
    for i in [reveal, moved]
        .into_iter()
        .flatten()
        .filter(|i| visible.binary_search(i).is_ok())
    {
        let v = &views[i];
        if let Some(g) = model.group_of(&v.id) {
            if model.group_collapsed(&g.name) {
                model.set_group_collapsed(&g.name, false);
            }
        }
    }
}

/// The chevron: right while the group is folded, down while it is open. Painted, so no font has to carry it.
fn chevron(ui: &mut egui::Ui, collapsed: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(10.0), egui::Sense::hover());
    let (c, colour) = (rect.center(), ui.visuals().widgets.inactive.fg_stroke.color);
    let points = if collapsed {
        vec![
            egui::pos2(c.x - 2.5, c.y - 4.5),
            egui::pos2(c.x + 3.5, c.y),
            egui::pos2(c.x - 2.5, c.y + 4.5),
        ]
    } else {
        vec![
            egui::pos2(c.x - 4.5, c.y - 2.5),
            egui::pos2(c.x + 4.5, c.y - 2.5),
            egui::pos2(c.x, c.y + 3.5),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        colour,
        egui::Stroke::NONE,
    ));
}

/// Note HF6: a group's header: chevron, name, then the `Excluded` tag, the count and the worst status dot on the
/// right. A click on the row folds or opens the group; the right-click menu holds its edits. Returns the set let go
/// on it this frame (a dropped texture does nothing: the header takes only sets).
fn group_header(
    ui: &mut egui::Ui,
    h: &Head,
    height: f32,
    editing: &Editing,
    acts: &mut Vec<Act>,
) -> Option<String> {
    // The widget id is the group's name, so a fold above never moves it (an open menu stays open).
    let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), height));
    let resp = ui.interact(
        rect,
        ui.scope_id().with(("set-group", &h.name)),
        egui::Sense::click(),
    );
    let over_set = editing.on
        && egui::DragAndDrop::has_payload_of_type::<DragSet>(ui.ctx())
        && resp.contains_pointer();
    let (fill, accent) = {
        let v = ui.visuals();
        let hot = resp.hovered() || over_set;
        (
            if hot {
                v.widgets.hovered.weak_bg_fill
            } else {
                v.faint_bg_color
            },
            v.selection.stroke,
        )
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    if over_set {
        ui.painter()
            .rect_stroke(rect, 6.0, accent, egui::StrokeKind::Inside);
    }
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(PILL_PAD_X, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    row.style_mut().interaction.selectable_labels = false;
    egui::Sides::new().shrink_left().truncate().show(
        &mut row,
        |ui| {
            chevron(ui, h.collapsed);
            // Cut short, the whole name is on hover and in the AccessKit name.
            let name = ui.add(egui::Label::new(egui::RichText::new(&h.name).strong()).truncate());
            name.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &h.name));
        },
        |ui| {
            super::status::dot(ui, h.status);
            ui.add(
                egui::Label::new(egui::RichText::new(h.count_text()).weak())
                    .wrap_mode(egui::TextWrapMode::Extend),
            );
            if h.excluded {
                egui::Frame::NONE
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::symmetric(4, 0))
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new("Excluded").small())
                                .wrap_mode(egui::TextWrapMode::Extend),
                        )
                    });
            }
        },
    );
    let name = h.node_name();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    ui.ctx()
        .accesskit_node_builder(resp.id, |n| n.set_expanded(!h.collapsed));
    if resp.clicked() {
        acts.push(Act::ToggleGroup(h.name.clone()));
    }
    resp.context_menu(|ui| group_menu(ui, h, editing, acts));
    let hovered_by_set = editing.on && resp.dnd_hover_payload::<DragSet>().is_some();
    hovered_by_set
        .then(|| resp.dnd_release_payload::<DragSet>())
        .flatten()
        .map(|p| p.0.clone())
}

/// Note HF10: Export set / Export group, off with the reason `off` (an export runs: a scope set then would change what
/// the running export's lists count; a set with no files).
fn export_button(ui: &mut egui::Ui, off: Option<&str>, label: &str, hover: &str) -> egui::Response {
    ui.add_enabled(off.is_none(), egui::Button::new(label))
        .on_hover_text(hover)
        .on_disabled_hover_text(off.unwrap_or_default())
}

/// Note HF6: a header's right-click menu. The edits are off while `sdc-project.toml` is read-only, with the reason;
/// Select all sets only reads.
fn group_menu(ui: &mut egui::Ui, h: &Head, editing: &Editing, acts: &mut Vec<Act>) {
    let edit = |ui: &mut egui::Ui, label: &str| {
        let r = ui.add_enabled(editing.on, egui::Button::new(label));
        if editing.on {
            r
        } else {
            r.on_disabled_hover_text(editing.why)
        }
    };
    // Note HF9: exports only this group's sets (and the sets they borrow from), even when it is excluded.
    if export_button(
        ui,
        editing
            .exporting
            .then_some(crate::model::also::EXPORT_RUNNING),
        "Export group",
        "Export only this group's sets; the rest of the export folder stays as it is",
    )
    .clicked()
    {
        acts.push(Act::ExportGroup(h.name.clone()));
        ui.close();
    }
    let (flip, hover) = if h.excluded {
        ("Include in export", "Export writes this group's sets again")
    } else {
        (
            "Exclude from export",
            "Export skips this group's sets and leaves their files in the export folder alone",
        )
    };
    if edit(ui, flip).on_hover_text(hover).clicked() {
        acts.push(Act::GroupExcluded(h.name.clone(), !h.excluded));
        ui.close();
    }
    if edit(ui, "Rename…").clicked() {
        acts.push(Act::OpenRenameGroup(h.name.clone()));
        ui.close();
    }
    if edit(ui, "Ungroup")
        .on_hover_text("Remove the group; its sets stay")
        .clicked()
    {
        acts.push(Act::Ungroup(h.name.clone()));
        ui.close();
    }
    if ui.button("Select all sets").clicked() {
        acts.push(Act::SelectGroup(h.name.clone()));
        ui.close();
    }
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
        exporting: model.export.running.is_some(),
    };
    let views = crate::model::sets::views(model);
    let revisions = model.set_content_revisions();
    if st.folded.revisions != revisions {
        st.folded.by_set.clear();
        st.folded.revisions = revisions;
    }
    let visible = model.visible_sets();
    let has_clear = filter_strips(ui, model, &views, &visible);
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
    // A card's dot, and (worst of its members) a header's.
    let status_of = |v: &SetView| {
        if blocked.contains(&v.id) {
            Status::Blocks
        } else if !crate::model::bottom::set_ready_in(&flagged, v) {
            Status::Warning
        } else {
            Status::Ready
        }
    };
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
    let frame_x = egui::Frame::group(ui.style()).total_margin().sum().x + 2.0 * DOCK_INSET;
    // ui-rules 3.2 (Note JJ2): a bar drawn without hover. Its 6 px gutter keeps the cards out from under the bar at
    // rest; hovered, the 10 px bar reaches only the cards' frame margin.
    ui.spacing_mut().scroll = crate::look::thin_scroll();
    // The cards' width beside the bar's gutter, whether or not the bar shows: a card is estimated only off screen, and
    // then the list overflows and the bar shows. A width that followed the bar would make its appearing a re-flow,
    // which scrolls to the open set.
    let card_w = ui.available_width() - ui.spacing().scroll.allocated_width();
    let reflowed = reflow_heights(st, &views, card_w, frame_x);
    // A new width moves the open card away from the scroll offset: bring it back into view (Note HN).
    let active = model.resolved_selection();
    let est = if st.card_h.is_empty() {
        CARD_EST
    } else {
        st.card_h.values().sum::<f32>() / st.card_h.len() as f32
    };
    let card_h = |id: &str| st.card_h.get(id).copied().unwrap_or(est);
    // A set the palette picked scrolls into view (item 19); `reveal_at` is its place in `visible`.
    // Note HP (SC3): a changed filter, search or shader starts the list at its top, unless the one open set still
    // shows: that one is revealed. The first frame has no earlier filter, so it counts as no change.
    // Compare first and clone only on a change: this runs every frame.
    let changed = match &st.filter {
        Some(f) if *f == model.sets_filter => false,
        last => {
            let was_known = last.is_some();
            st.filter = Some(model.sets_filter.clone());
            was_known
        }
    };
    let mut to_top = false;
    let single = (reflowed || changed)
        && matches!(active.selection, Selection::Set(_) | Selection::SetId(_))
        && !model.in_bulk();
    if reflowed && model.nav.reveal_set.is_none() && single {
        if let Some(k) = &active.target {
            st.quiet_reveal = Some(k.clone());
            model.nav.reveal_set = Some(k.clone());
        }
    }
    if changed {
        match &active.target {
            Some(k)
                if single
                    && active
                        .index
                        .is_some_and(|i| visible.binary_search(&i).is_ok()) =>
            {
                st.quiet_reveal = Some(k.clone());
                model.nav.reveal_set = Some(k.clone());
            }
            _ => to_top = model.nav.reveal_set.is_none(),
        }
        st.reveal_tries = 0;
    }
    let reveal = model.nav.reveal_set.take();
    let reveal_index = reveal.as_ref().and_then(|target| {
        if active.target.as_ref() == Some(target) {
            active.index
        } else {
            views.iter().position(|v| target.matches(v))
        }
    });
    // Note HF6: what a reveal or a moved selection leads to is shown: its group opens before the rows are built.
    open_groups_of(
        model,
        &views,
        &visible,
        &mut st.selection,
        reveal_index.filter(|_| st.quiet_reveal != reveal),
    );
    let (rows, heads, grouped) = build_rows(model, &views, &visible, &status_of);
    let head_h = ui
        .text_style_height(&egui::TextStyle::Body)
        .max(ui.spacing().interact_size.y)
        + 6.0;
    let reveal_at = reveal_index.and_then(|index| {
        rows.iter()
            .position(|r| matches!(r, Row::Card(i) if *i == index))
    });
    let row_h = |r: &Row| match r {
        Row::Header(_) => head_h,
        Row::Card(i) => card_h(&views[*i].id),
    };
    // Review I1: a target outside last frame's view jumps to the offset the known heights give. `scroll_to_rect` on
    // an estimated rect glided to the wrong card, and its relative target double-counted when aimed again. Note FM32:
    // the target lands at the top of the view, a little below it (the sum of the rounded allocations above it is not
    // quite the sum of the heights), so it is whole on screen whatever its true height; bottom-aligning an estimated
    // height left it cut off, and a second scroll moved it.
    let jump = reveal_at
        .filter(|_| st.reveal_seen != reveal)
        .and_then(|n| {
            let (off, view_h) = st.view?;
            let top: f32 = rows[..n].iter().map(|r| row_h(r) + gap.y).sum();
            let h = row_h(&rows[n]);
            let outside = top + h <= off || top >= off + view_h;
            outside.then(|| (top - 2.0 * gap.y).max(0.0))
        });
    // Note FM32: the first card in view last frame keeps its place: when the heights above it changed since (the
    // mean of the measured ones moves, a resize re-flowed them), the offset moves by the same amount.
    let anchored = || {
        let ((id, was), (off, _)) = st.anchor.as_ref().zip(st.view)?;
        let n = rows
            .iter()
            .position(|r| matches!(r, Row::Card(i) if views[*i].id == *id))?;
        let now: f32 = rows[..n].iter().map(|r| row_h(r) + gap.y).sum();
        ((now - was).abs() > 0.5).then(|| (off + now - was).max(0.0))
    };
    let target = if to_top {
        st.anchor = None;
        Some(0.0)
    } else {
        jump.or_else(anchored)
    };
    let mut resized = false;
    let mut reveal_drawn = false;
    // The first card in view (id, top in the content) and the off-screen card that took the focus, if any.
    let (mut anchor, mut focused) = (None::<(String, f32)>, None::<String>);
    let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
    if let Some(y) = target {
        area = area.vertical_scroll_offset(y);
    }
    let image = egui::DragAndDrop::has_payload_of_type::<DragItem>(&ctx)
        || egui::DragAndDrop::has_payload_of_type::<DragBorrow>(&ctx);
    let mut next_strip = None;
    // Test last frame's painted area and its bridge once, before any zone can take the release.
    let strip_owner = st
        .strip
        .as_ref()
        .filter(|(_, painted, hit)| {
            image
                && ctx
                    .pointer_latest_pos()
                    .is_some_and(|p| painted.contains(p) || hit.contains(p))
        })
        .map(|(id, ..)| id.as_str());
    let out = area.show(ui, |ui| {
        let view_top = ui.clip_rect().top();
        // `top` runs on the remembered heights, not on the laid-out rects: egui rounds each allocation, and the sum of
        // those roundings drifts from the heights over hundreds of cards.
        let mut top = 0.0;
        // `index` is the set's place among all sets: the reorder and Move up / Move down use it (K19).
        for (pos, row) in rows.iter().enumerate() {
            // Note HF6: a header is always drawn (one short row); a dropped set joins its group.
            let index = match *row {
                Row::Header(g) => {
                    let out = group_header(ui, &heads[g], head_h, &editing, &mut acts);
                    if let Some(set) = out {
                        acts.push(Act::JoinGroup(set, heads[g].name.clone()));
                    }
                    top += head_h + gap.y;
                    continue;
                }
                Row::Card(i) => i,
            };
            let v = &views[index];
            let h = st.card_h.get(&v.id).copied().unwrap_or(est);
            let id = ui.scope_id().with(("set-card", &v.key, &v.id));
            let selected = sel_sets.contains(&v.id);
            let mut card = |ui: &mut egui::Ui| {
                let probe =
                    egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), h));
                if ui.is_rect_visible(probe) {
                    let look = PillLook {
                        image: image && next_strip.is_none(),
                        strip_owner,
                        frame_x,
                        dragging: dragged.as_deref() == Some(v.id.as_str()),
                        selected,
                        bulk: sel_sets.len() >= 2,
                        id,
                    };
                    let out = pill(
                        ui,
                        model,
                        &flagged,
                        thumbs,
                        v,
                        (index, views.len()),
                        st.folded.by_set.entry(index).or_default(),
                        &editing,
                        status_of(v),
                        look,
                        sel_items,
                        &mut acts,
                    );
                    if let Some((painted, hit)) = out.strip {
                        next_strip = Some((index, painted, hit));
                    }
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
                    // R4: a card outside the clip rect stands in for its last height. The focus it takes scrolls the
                    // real card into view (Note FM32).
                    let (rect, wanted) = stand_in(
                        ui,
                        id,
                        probe.size(),
                        st.focus,
                        v,
                        (index, views.len()),
                        selected,
                    );
                    if wanted {
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
                }
            };
            // Note HF6: a member sits GROUP_INDENT in from the left.
            let (rect, used) = if grouped.get(index).copied().flatten().is_some() {
                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: GROUP_INDENT as i8,
                        ..Default::default()
                    })
                    .show(ui, card)
                    .inner
            } else {
                card(ui)
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
            new_set_area(ui, &editing, strip_owner.is_some(), &mut acts);
        }
    });
    st.strip = next_strip.map(|(index, painted, hit)| {
        let id = match st.strip.take() {
            Some((id, ..)) if id == views[index].id => id,
            _ => views[index].id.clone(),
        };
        (id, painted, hit)
    });

    st.view = Some((out.state.clamped_offset().y, out.inner_rect.height()));
    st.anchor = anchor;
    st.reveal_seen = reveal.clone().filter(|_| reveal_drawn);
    let go_to = reveal.is_some();
    st.focus = ctx.memory(|m| m.focused());
    // Review I1: aim again next frame until the target is laid out with every height around it measured.
    if reveal_at.is_some() && !(reveal_drawn && !resized) && st.reveal_tries < REVEAL_TRIES {
        st.reveal_tries += 1;
        model.nav.reveal_set = reveal;
        ctx.request_repaint();
    } else {
        if reveal.is_some() {
            st.reveal_tries = 0;
        }
        st.quiet_reveal = None;
    }
    // Note FM32: Tab reached an off-screen card: the next frame scrolls it in and draws it, keeping the focus.
    // A Go to set in the same frame wins: the card keeps its focus, and the Tab does not pull the view off the target.
    if let Some(k) = focused.filter(|_| !go_to) {
        st.reveal_tries = 0;
        model.nav.reveal_set = Some(crate::model::SetTarget::Key(k));
        ctx.request_repaint();
    }
    // A measured height moved the cards below it: one more frame lays them out where they now are (settles).
    if resized {
        ctx.request_repaint();
    }
    let ids: Vec<&str> = views.iter().map(|v| v.id.as_str()).collect();
    if editing.reorder {
        let (dropped, target) = dropped.unzip();
        if let Some((id, to)) = reorder_target(dropped.as_deref(), target, &ids, &grouped) {
            acts.push(Act::Reorder(id, to));
        }
    }
    for a in acts {
        match a {
            // Note FC8 (BN3): a new selection waits a frame for a focused field to commit.
            Act::SelectItem(k) => super::after_commit(&ctx, model, move |m| m.select_item(&k)),
            Act::Click(id, ctrl, shift) => after_card_commit(&ctx, model, id, move |m, target| {
                m.click_set(target, ctrl, shift)
            }),
            Act::RightClick(id) => after_card_commit(&ctx, model, id, AppModel::right_click_set),
            // Note HF6: New group… asks for a name first; the other picks go to the model.
            Act::Bulk(MenuAction::ToNewGroup) => {
                st.pending = Some(Pending::NewGroup {
                    ids: model.selected_set_ids(),
                    text: String::new(),
                    error: None,
                })
            }
            Act::Bulk(a) => model.menu_picked(a),
            Act::Problems(k) => {
                model.open_problems(Some(crate::model::bottom::ProblemsFilter::Set(k)))
            }
            Act::Slot(a) => super::slot_commands::run(model, cmds, a),
            Act::Expand(k) => super::after_commit(&ctx, model, move |m| m.expand(&k)),
            Act::Drop(item, set, shift) => match model.drop_on_set(&item, &set) {
                DropOutcome::Joined => {}
                DropOutcome::AskChoice { from } => {
                    let usable = |c| {
                        model.drop_choices(&item).contains(&c)
                            && model.choice_refused(&item, &set, c).is_none()
                    };
                    let skip = st.last_choice.filter(|&c| shift && usable(c));
                    st.pending = match skip {
                        Some(c) => choose(model, item, set, c),
                        None => Some(Pending::Choice { item, set, from }),
                    };
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
            Act::ShareBorrow { item, from, set } => match model.drop_borrow(&item, &from, &set) {
                Ok(BorrowOutcome::AskReplace { kind, existing }) => {
                    st.pending = Some(Pending::BorrowReplace {
                        item,
                        from,
                        set,
                        kind,
                        existing,
                    })
                }
                Ok(BorrowOutcome::Refused(why)) => st.pending = Some(Pending::Refused(why)),
                Ok(BorrowOutcome::Joined) | Err(_) => {}
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
            Act::MapTarget(id) => {
                model.target_pick_error = None;
                // A mapped set opens on its current record, mesh path or file base, when the scan or target still has it.
                let sel = crate::model::sets::views(model)
                    .iter()
                    .find(|v| engine::sets::same_id(&v.id, &id))
                    .and_then(|v| v.txst.as_ref())
                    .and_then(|t| match &t.base {
                        None => model.record_of(t).cloned().map(TargetPick::Record),
                        Some(b) => model.target().and_then(|tm| {
                            let same = |x: &str| x.eq_ignore_ascii_case(b);
                            let mesh = tm.meshes.iter().find(|m| same(&m.base));
                            let files = tm.files.iter().find(|f| same(&f.base));
                            mesh.cloned()
                                .map(TargetPick::Mesh)
                                .or(files.cloned().map(TargetPick::Files))
                        }),
                    });
                st.pending = Some(Pending::MapTarget(Box::new(MapTargetState {
                    id,
                    query: String::new(),
                    rows: vec![],
                    hits: vec![],
                    seen: None,
                    used: HashMap::new(),
                    sel,
                })));
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
            Act::Delete(id) => model.delete_set_with_toast(&id),
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
            // Note HF6: the group edits. A fold is no edit; the others say why when the file refuses.
            Act::ToggleGroup(name) => {
                let on = !model.group_collapsed(&name);
                model.set_group_collapsed(&name, on);
            }
            Act::GroupExcluded(name, on) => {
                if let Err(why) = model.set_group_excluded(&name, on) {
                    st.pending = Some(Pending::Refused(why));
                }
            }
            Act::OpenRenameGroup(name) => {
                st.pending = Some(Pending::RenameGroup {
                    text: name.clone(),
                    name,
                    error: None,
                })
            }
            Act::Ungroup(name) => {
                if let Err(why) = model.ungroup(&name) {
                    st.pending = Some(Pending::Refused(why));
                }
            }
            Act::SelectGroup(name) => {
                model.select_group(&name);
                // Selecting is no reveal: a folded group stays folded.
                st.selection = Some(model.selection.clone());
            }
            Act::JoinGroup(id, name) => {
                if let Err(why) = model.move_to_group(&[id], GroupTarget::Existing(name)) {
                    st.pending = Some(Pending::Refused(why));
                }
            }
            Act::MoveToGroup(ids, to) => {
                if let Err(why) = model.move_to_group(&ids, to) {
                    st.pending = Some(Pending::Refused(why));
                }
            }
            Act::OpenNewGroup(ids) => {
                st.pending = Some(Pending::NewGroup {
                    ids,
                    text: String::new(),
                    error: None,
                })
            }
            Act::ExportGroup(name) => model.export_group(&name),
            Act::ExportSet(key, id) => {
                model.export_sets([key].into(), format!("Export set {id}"));
            }
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
        model.nav.reveal_set = key
            .filter(|k| !k.is_empty())
            .map(crate::model::SetTarget::Key);
    }
}

/// The plan can land while the old editor commits. Find the clicked card by its document id in that plan.
fn after_card_commit(
    ctx: &egui::Context,
    model: &mut AppModel,
    id: String,
    click: impl FnOnce(&mut AppModel, SetTarget) + 'static,
) {
    super::after_commit(ctx, model, move |m| {
        let views = crate::model::sets::views(m);
        if let Some(v) = views.iter().find(|v| engine::sets::same_id(&v.id, &id)) {
            click(m, SetTarget::view(v));
        } else {
            m.clear_selection();
        }
    });
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
        egui::WidgetInfo::labeled(egui::Role::Label, true, format!("Position {n} of {total}"))
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
fn new_set_area(ui: &mut egui::Ui, editing: &Editing, covered: bool, acts: &mut Vec<Act>) {
    // Note HN F6: weak text, the selection accent while last frame's zone had a dragged texture over it.
    let hot_id = ui.scope_id().with("new_set_hint_hot");
    let hot = ui.data(|d| d.get_temp::<bool>(hot_id)).unwrap_or(false);
    let colour = if hot && !covered {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().weak_text_color()
    };
    let borrow_in_flight = egui::DragAndDrop::has_payload_of_type::<DragBorrow>(ui.ctx());
    let (zone, payload) = item_zone(
        ui,
        editing.on && !borrow_in_flight && !covered,
        egui::Frame::group(ui.style()).corner_radius(12.0),
        |ui| {
            ui.set_width(ui.available_width());
            // A one-off hint: it wraps rather than elides (the column's style truncates).
            ui.add(
                egui::Label::new(egui::RichText::new("Drop here to start a new set").color(colour))
                    .wrap(),
            );
        },
    );
    let over = editing.on
        && !covered
        && zone.contains_pointer()
        && egui::DragAndDrop::has_payload_of_type::<DragItem>(ui.ctx());
    ui.data_mut(|d| d.insert_temp(hot_id, over));
    if !editing.on {
        zone.on_hover_text(editing.why);
    }
    if let Some(Dropped::Item(p)) = payload {
        acts.push(Act::NewSet(p.0.clone()));
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
) -> (egui::Response, Option<Dropped>) {
    if editing && !egui::DragAndDrop::has_payload_of_type::<DragSet>(ui.ctx()) {
        if egui::DragAndDrop::has_payload_of_type::<DragBorrow>(ui.ctx()) {
            let (zone, p) = ui.dnd_drop_zone::<DragBorrow, ()>(frame, add);
            return (zone.response, p.map(Dropped::Borrow));
        }
        let (zone, p) = ui.dnd_drop_zone::<DragItem, ()>(frame, add);
        (zone.response, p.map(Dropped::Item))
    } else {
        (frame.show(ui, add).response, None)
    }
}

/// Where a dragged set goes: `released` is the set let go this frame, `hovered` the index of the pill it was let go
/// on, `grouped` each id's group (empty without groups). Nothing happens when there is no target or the set is
/// dropped on itself.
fn reorder_target(
    released: Option<&str>,
    hovered: Option<usize>,
    ids: &[&str],
    grouped: &[Option<usize>],
) -> Option<(String, usize)> {
    let (id, mut to) = (released?, hovered?);
    if *ids.get(to)? == id {
        return None;
    }
    let from = ids.iter().position(|i| *i == id)?;
    // Note HS14: a group's first member takes its block along, and `moved_order` counts `to` without the block.
    let g = grouped.get(from).copied().flatten();
    let first = g.is_some() && (from == 0 || grouped[from - 1] != g);
    let rest = if first {
        grouped[from + 1..].iter().take_while(|x| **x == g).count()
    } else {
        0
    };
    if to > from {
        if to <= from + rest {
            return None;
        }
        to -= rest;
    }
    Some((id.to_string(), to))
}

/// What a pill reports back: the set dropped on it this frame (R1: detected on the target, since the source card
/// may not be laid out when the drag ends), and whether a dragged set is over it.
struct PillOut {
    strip: Option<(egui::Rect, egui::Rect)>,
    dropped_set: Option<String>,
    hovered_by_set: bool,
    rect: egui::Rect,
}

/// How a pill is drawn this frame.
#[derive(Clone, Copy)]
struct PillLook<'a> {
    image: bool,
    strip_owner: Option<&'a str>,
    frame_x: f32,
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
    flagged: &Flagged,
    thumbs: &mut ThumbCache,
    v: &SetView,
    (index, total): (usize, usize),
    folded: &mut FoldedBySlice,
    editing: &Editing,
    status: Status,
    look: PillLook<'_>,
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
    let card_margin = card.total_margin();
    let builder = egui::UiBuilder::new()
        .scope_id(look.id)
        .sense(egui::Sense::click());
    let scoped = ui.scope_builder(builder, |ui| {
        if look.dragging {
            ui.set_opacity(0.5);
        }
        let owns = look.strip_owner == Some(v.id.as_str());
        let covered = look.strip_owner.is_some() && !owns;
        let (zone, mut dropped) = item_zone(
            ui,
            editing.on && !covered,
            egui::Frame::NONE.corner_radius(12.0),
            |ui| {
                card.show(ui, |ui| {
                    ui.style_mut().interaction.selectable_labels = false;
                    ui.set_width(ui.available_width());
                    // Note HP: the card's left, right and top, for the badge hit rects to stay inside. The bottom is
                    // not known until the card is laid out, and no badge reaches it, so it is left open.
                    let card_rect = {
                        let (r, m) = (ui.max_rect(), card_margin);
                        egui::Rect::from_min_max(
                            r.min - egui::vec2(m.left, m.top),
                            egui::pos2(r.max.x + m.right, f32::MAX),
                        )
                    };
                    // Review E4: `counts_for`, as the tiles; each `At` is built only for a card member or set
                    // with warnings.
                    let set_at = (!v.set_warnings.is_empty()).then(|| At::Set {
                        key: v.key.clone(),
                        id: v.id.clone(),
                    });
                    let rows: Vec<(&str, &Warning)> = v
                        .members
                        .iter()
                        .filter_map(|k| model.item(k))
                        .filter(|i| !i.warnings.is_empty())
                        .flat_map(|i| {
                            let at = At::Texture(i.key.clone());
                            i.warnings
                                .iter()
                                .filter(move |w| counts_for(model, &at, w))
                                .map(move |w| (i.name.as_str(), w))
                        })
                        .chain(set_at.iter().flat_map(|at| {
                            v.set_warnings
                                .iter()
                                .filter(|w| counts_for(model, at, w))
                                .map(|w| (v.id.as_str(), w))
                        }))
                        .collect();
                    // Kept apart from `rows`: the "⚠ n" never counts check results. Undecided members are left out,
                    // as in Problems (review I1).
                    // Review E4: a marked result leaves the badge, as its row leaves the Checks pill.
                    let set_checks = model.set_check_warnings(&v.key);
                    let set_check_at = (!set_checks.is_empty()).then(|| At::Set {
                        key: v.key.clone(),
                        id: v.id.clone(),
                    });
                    // Final fix wave A6: the marked ones stay for the badge's hover, listed last.
                    let mut checks: Vec<(&str, &Warning)> = vec![];
                    let mut marked_checks: Vec<(&str, &Warning)> = vec![];
                    let results = v
                        .members
                        .iter()
                        .filter_map(|k| model.item(k))
                        .filter(|i| i.status != crate::model::items::ItemStatus::NeedsDecision)
                        .flat_map(|i| {
                            let found = model.check_warnings(&i.key);
                            let at = (!found.is_empty()).then(|| At::Texture(i.key.clone()));
                            found.iter().map(move |w| {
                                let listed = at.as_ref().is_some_and(|a| check_listed(model, a, w));
                                (listed, (i.name.as_str(), w))
                            })
                        })
                        .chain(set_check_at.iter().flat_map(|at| {
                            set_checks
                                .iter()
                                .map(|w| (check_listed(model, at, w), (v.id.as_str(), w)))
                        }));
                    for (listed, row) in results {
                        if listed {
                            &mut checks
                        } else {
                            &mut marked_checks
                        }
                        .push(row);
                    }
                    // One row only when the name keeps NAME_MIN px (or its whole width) beside the pill and badges
                    // (Notes FI13, HP12).
                    let body = egui::TextStyle::Body.resolve(ui.style());
                    let query = &model.sets_filter.search;
                    let title_key = egui::Id::unique((
                        "card-title",
                        &v.id,
                        &v.short,
                        query,
                        &body,
                        ui.pixels_per_point().to_bits(),
                    ));
                    let (whole, full_width) = super::paint_cached(ui.ctx(), title_key, || {
                        #[cfg(feature = "test-hooks")]
                        super::paint_counts::Work::Title.built();
                        (
                            Arc::new(egui::RichText::new(&v.id).strong()),
                            text_width(ui, &v.id, &body),
                        )
                    });
                    // Only measure shorter forms if the whole title cannot fit.
                    let shorter = |ui: &egui::Ui| {
                        super::paint_cached(ui.ctx(), title_key.with("short"), || {
                            Arc::new(ShortTitles::new(ui, v, query, &body))
                        })
                    };
                    let fits =
                        |ui: &egui::Ui, room| full_width <= room || shorter(ui).minimum <= room;
                    let name_min = NAME_MIN.min(full_width);
                    let sp = ui.spacing().item_spacing.x;
                    let btn = 2.0 * ui.spacing().button_padding.x + sp;
                    let stroke = ui.visuals().widgets.noninteractive.bg_stroke.width;
                    let kind = kind_text(model, v);
                    let count = checks
                        .iter()
                        .filter(|(_, w)| super::counts_toward_badge(w))
                        .count();
                    let heading_key = title_key.with((
                        total,
                        kind,
                        rows.len(),
                        checks.is_empty(),
                        count,
                        egui::TextStyle::Small.resolve(ui.style()),
                        sp.to_bits(),
                        btn.to_bits(),
                        stroke.to_bits(),
                    ));
                    let need = super::paint_cached(ui.ctx(), heading_key, || {
                        let lead =
                            text_width(ui, &total.to_string(), &egui::FontId::monospace(body.size))
                                + text_width(ui, "≡", &body)
                                + 2.0 * sp;
                        let pill = small_width(ui, kind)
                            + 2.0 * PILL_PAD_X
                            + 2.0 * stroke
                            + STATUS_DOT
                            + sp;
                        let mut need = lead + name_min + pill + sp;
                        if !rows.is_empty() {
                            need += small_width(ui, &warn_text(rows.len())) + btn;
                        }
                        if !checks.is_empty() {
                            need += small_width(ui, &check_badge_text(&checks).1) + btn;
                        }
                        need
                    });
                    // The wrap decision and displayed form use the same measured widths.
                    // One row only when the title that will be shown also fits beside the pill and badges
                    // (Task 9 review); otherwise it takes its own line at the card's full width.
                    let wide = ui.available_width() >= need
                        && fits(ui, ui.available_width() - (need - name_min));
                    let name = |ui: &mut egui::Ui| {
                        let room = ui.available_width();
                        let (shown, fits) =
                            super::paint_cached(ui.ctx(), title_key.with(room.to_bits()), || {
                                #[cfg(feature = "test-hooks")]
                                super::paint_counts::Work::TitleFit.built();
                                if full_width <= room {
                                    (whole.clone(), true)
                                } else {
                                    shorter(ui).at(room)
                                }
                            });
                        // Extend: the shown text already fits, so egui never end-cuts the tail. When even the
                        // tail does not fit, the title wraps instead of cutting further (E24).
                        let wrap = if fits {
                            egui::TextWrapMode::Extend
                        } else {
                            egui::TextWrapMode::Wrap
                        };
                        let r = ui.add(egui::Label::new(shown.clone()).wrap_mode(wrap));
                        r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &v.id));
                        if shown.text() != v.id {
                            let hover =
                                super::paint_cached(ui.ctx(), title_key.with("hover"), || {
                                    Arc::new(egui::RichText::new(&v.id))
                                });
                            r.on_hover_text(hover);
                        }
                    };
                    let own_line = ui.horizontal(|ui| {
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
                                    check_badge(ui, v, &checks, &marked_checks, acts);
                                    warn_button(ui, v, &rows, acts);
                                    status_pill(ui, model, flagged, v, status);
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        name,
                                    );
                                },
                            );
                            false
                        } else {
                            // Beside the position and handle the tail would not fit: give the title its own line.
                            let own = !fits(ui, ui.available_width());
                            if !own {
                                name(ui);
                            }
                            own
                        }
                    });
                    if own_line.inner {
                        ui.horizontal(name);
                    }
                    if !wide {
                        ui.horizontal_wrapped(|ui| {
                            status_pill(ui, model, flagged, v, status);
                            warn_button(ui, v, &rows, acts);
                            check_badge(ui, v, &checks, &marked_checks, acts);
                        });
                    }
                    if let Some(map) = &v.txst {
                        record_line(ui, v, map, model.renames_outputs(v));
                    }
                    // Note HP: the rects a badge's hit area must keep clear of: the combo, then every chip and tile.
                    let mut fixed: Vec<egui::Rect> = vec![];
                    if !v.key.is_empty() {
                        fixed.push(shader_combo(ui, model, v, editing, acts));
                    }
                    if v.key.is_empty() && v.created && v.missing.is_empty() {
                        ui.weak("Drop a texture here to start");
                    }
                    // Note HN: helpers dock on the chips they fill.
                    let dock = docked(model, v);
                    let side = DOCK_INSET as i8;
                    let top = (DOCK_GAP_Y - ui.spacing().item_spacing.y) as i8;
                    egui::Frame::NONE
                        .inner_margin(egui::Margin {
                            left: side,
                            right: side,
                            top,
                            bottom: 0,
                        })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(DOCK_GAP_X, DOCK_GAP_Y);
                            ui.horizontal_wrapped(|ui| {
                                let (plans, fixed) = chip_row(
                                    ui,
                                    model,
                                    thumbs,
                                    v,
                                    &dock,
                                    folded,
                                    editing,
                                    sel_items,
                                    acts,
                                    &mut member_menu,
                                    std::mem::take(&mut fixed),
                                );
                                // Badges last: they register after every chip and sit on top of it.
                                draw_docks(
                                    ui,
                                    model,
                                    thumbs,
                                    v,
                                    &plans,
                                    &fixed,
                                    card_rect,
                                    sel_items,
                                    acts,
                                    &mut member_menu,
                                );
                            });
                        });
                });
            },
        );
        // Final review I1: the zone's hover stroke is under the card frame, so outline the drop target on top.
        let over = look.image && !covered && (zone.contains_pointer() || owns);
        if editing.on && over {
            ui.painter().rect_stroke(
                zone.rect,
                12.0,
                ui.visuals().widgets.active.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        let strip = (over && !v.empty.is_empty()).then(|| {
            let per_row = chips_per_row(zone.rect.width(), look.frame_x);
            let clip = ui.clip_rect();
            let rect = strip_rect(zone.rect, strip_size(v.empty.len(), per_row), clip);
            let layer = egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::unique("sets-empty-strip"),
            );
            let p = ui.ctx().layer_painter(layer).with_clip_rect(clip);
            paint_strip(&p, ui.style(), v, rect, per_row);
            let painted = rect.intersect(clip);
            let mut hit = painted;
            if rect.top() >= zone.rect.bottom() {
                hit.min.y = zone.rect.bottom();
            } else if rect.bottom() <= zone.rect.top() {
                hit.max.y = zone.rect.top();
            }
            // The bridge reaches only the part of the strip that faces the card.
            hit.min.x = hit.min.x.max(zone.rect.left());
            hit.max.x = hit.max.x.min(zone.rect.right());
            (painted, hit.intersect(clip))
        });
        if owns && !zone.contains_pointer() && editing.on && ui.input(|i| i.pointer.any_released())
        {
            if egui::DragAndDrop::has_payload_of_type::<DragBorrow>(ui.ctx()) {
                dropped =
                    egui::DragAndDrop::take_payload::<DragBorrow>(ui.ctx()).map(Dropped::Borrow);
            } else if egui::DragAndDrop::has_payload_of_type::<DragItem>(ui.ctx()) {
                dropped = egui::DragAndDrop::take_payload::<DragItem>(ui.ctx()).map(Dropped::Item);
            }
        }
        (dropped, strip)
    });
    let (dropped, strip) = scoped.inner;
    let resp = scoped.response;
    // Note FM32: named as its stand-in is, so a card keeps its name when it scrolls in (a Tab stop for the card itself).
    ui.ctx().accesskit_node_builder(resp.id, |n| {
        card_node(n, v, (index, total), selected, resp.rect)
    });
    if !editing.on {
        resp.clone().on_hover_text(editing.why);
    }
    if dropped.is_none() && resp.clicked() && acts.is_empty() {
        let m = ui.input(|i| i.modifiers);
        acts.push(Act::Click(v.id.clone(), m.command, m.shift));
    }
    match dropped {
        Some(Dropped::Item(p)) => acts.push(Act::Drop(
            p.0.clone(),
            v.id.clone(),
            ui.input(|i| {
                let mut releases = i.events.iter().filter_map(|event| match event {
                    egui::Event::PointerButton {
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        pos,
                        modifiers,
                    } => Some(modifiers.shift && resp.interact_rect.contains(*pos)),
                    _ => None,
                });
                releases.next() == Some(true) && releases.next().is_none()
            }),
        )),
        Some(Dropped::Borrow(p)) => acts.push(Act::ShareBorrow {
            item: p.item.clone(),
            from: p.from.clone(),
            set: v.id.clone(),
        }),
        None => {}
    }
    let hovered_by_set = editing.reorder && resp.dnd_hover_payload::<DragSet>().is_some();
    // K9: only while a set hovers, since `take_payload` would take a dragged texture too.
    let dropped_set = hovered_by_set
        .then(|| resp.dnd_release_payload::<DragSet>())
        .flatten()
        .map(|p| p.0.clone());
    // F3a D4: a right-click on an unselected pill selects it alone first; among two or more selected sets the pill
    // shows the bulk menu, otherwise its own.
    if resp.secondary_clicked() && !member_menu {
        acts.push(Act::RightClick(v.id.clone()));
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
        strip,
        hovered_by_set,
        rect: resp.rect,
    }
}

/// Review E1: the chip of a slot that holds a link: the linked item's thumbnail, or a filled letter box when no item
/// is known (a typed path); `link_mark` sits at its top centre. `cell` is the slot's kind, letter and link.
/// Returns the chip's rect.
#[allow(clippy::too_many_arguments)]
fn linked_chip<'a>(
    ui: &mut egui::Ui,
    model: &AppModel,
    thumbs: &mut ThumbCache,
    v: &SetView,
    cell: (SlotKind, &str, &'a SlotLink),
    sel_items: &HashSet<String>,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
    chipped: &mut HashSet<&'a str>,
) -> egui::Rect {
    let (k, letter, l) = cell;
    // names and hovers are built when read, not per frame
    let name = || linked_chip_name(letter, l);
    let hover = || format!("{} linked to {}", v.slot_text(k), l.path);
    let thumb = l.item.as_deref().and_then(|mk| {
        chipped.insert(mk);
        // Note JW4: an explicit link has no drag payload.
        member_chip(
            ui,
            model,
            thumbs,
            v,
            mk,
            Some(k),
            false,
            ChipDrag::None,
            Some(&hover),
            sel_items,
            acts,
            member_menu,
        )
    });
    let rect = match thumb {
        Some(r) => {
            paint_letter(ui, r.rect, letter, false, f32::INFINITY);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name()));
            r.rect
        }
        None => {
            // a typed path: no item to show, so the box takes the slot menu a thumbnail would have, and paints the
            // slot's locks as a thumbnail does (final wave Group 5)
            let r = letter_box_painted(ui, letter, None, true).interact(egui::Sense::click());
            let (locked, pgp) = chip_locks(v, Some(k));
            super::columns::paint_locks(ui, r.rect, locked, pgp);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name()));
            ui.ctx()
                .accesskit_node_builder(r.id, |b| b.set_description(hover()));
            let r = r.on_hover_ui(|ui| {
                ui.label(hover());
            });
            slot_box_menu(model, v, k, &r, acts, member_menu);
            r.rect
        }
    };
    chip_link_mark(ui, rect);
    rect
}

/// Note JW3: linked and borrowed chips share the mark between their two top lock cells.
fn chip_link_mark(ui: &egui::Ui, rect: egui::Rect) {
    let side = CHIP - 2.0 * (LOCK_PAD + LOCK_W);
    let at = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(LOCK_PAD + LOCK_W, MARK_TOP),
        egui::Vec2::splat(side),
    );
    // the panel's fill backs the mark so the strong text colour reads over any thumbnail in both themes
    ui.painter().rect_filled(at, 2.0, ui.visuals().panel_fill);
    link_mark(ui.painter(), at, ui.visuals().strong_text_color());
}

/// Note JW1: the chip row of a card: one chip per displayed fill or clear, the missing entries, then the
/// members that fill no chip. Returns each filled chip's dock plan and every rect a badge's hit must keep clear of
/// (`fixed` comes in holding the combo's).
#[allow(clippy::too_many_arguments)]
fn chip_row<'a>(
    ui: &mut egui::Ui,
    model: &'a AppModel,
    thumbs: &mut ThumbCache,
    v: &SetView,
    dock: &'a [Docked],
    folded: &mut FoldedBySlice,
    editing: &Editing,
    sel_items: &HashSet<String>,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
    mut fixed: Vec<egui::Rect>,
) -> (Vec<DockPlan<'a>>, Vec<egui::Rect>) {
    let row = view_row(v);
    let mut represented: HashSet<&str> = HashSet::new();
    let mut plans: Vec<DockPlan> = vec![];
    for cell in &row {
        let k = cell.kind;
        let letter = || chip_letter(v, k);
        let (mk, from) = match slot_shown(v, k) {
            SlotShown::Linked(l) => {
                let rect = linked_chip(
                    ui,
                    model,
                    thumbs,
                    v,
                    (k, &letter(), l),
                    sel_items,
                    acts,
                    member_menu,
                    &mut represented,
                );
                fixed.push(rect);
                continue;
            }
            // the JSON leaves the slot empty: dim like an empty chip, named for what it is
            SlotShown::Cleared { item } => {
                let letter = letter();
                // a click-sensed box with the slot menu, so the clear can be undone from the card
                let r = letter_box_painted(ui, &letter, None, false).interact(egui::Sense::click());
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        format!("{letter}, cleared"),
                    )
                });
                let hover = || {
                    let file = model.item(item).map_or(item, |i| i.name.as_str());
                    format!("{} (cleared): {file}", v.slot_text(k))
                };
                ui.ctx()
                    .accesskit_node_builder(r.id, |b| b.set_description(hover()));
                let r = r.on_hover_ui(|ui| {
                    ui.label(hover());
                });
                slot_box_menu(model, v, k, &r, acts, member_menu);
                fixed.push(r.rect);
                continue;
            }
            SlotShown::Own { item } => (item, None),
            SlotShown::Borrowed { from, item } => (item, Some(from)),
            SlotShown::Empty => continue,
            SlotShown::Record { path } => {
                let letter = letter();
                let name = || format!("{letter}, record {path}");
                fixed.push(
                    letter_box(ui, &letter, None, Some(&name))
                        .on_hover_text(path)
                        .rect,
                );
                continue;
            }
            SlotShown::Replaced { .. } => {
                let r = letter_box(ui, &letter(), None, None)
                    .on_hover_text(format!("{} (empty)", v.slot_text(k)));
                fixed.push(r.rect);
                continue;
            }
        };
        let letter = letter();
        represented.insert(mk);
        let note = from.map(|f| move || format!("borrowed from {f}"));
        // Note JW4: a displayed borrow shares its source, including at the end of a chain.
        let Some(r) = member_chip(
            ui,
            model,
            thumbs,
            v,
            mk,
            Some(k),
            from.is_none(),
            from.map_or(ChipDrag::Item, ChipDrag::Borrow),
            note.as_ref().map(|n| n as &dyn Fn() -> String),
            sel_items,
            acts,
            member_menu,
        ) else {
            fixed.push(letter_box(ui, &letter, None, None).rect);
            continue;
        };
        let own = fixed.len();
        fixed.push(r.rect);
        paint_letter(ui, r.rect, &letter, false, f32::INFINITY);
        if from.is_some() {
            ui.painter().rect_stroke(
                r.rect,
                2.0,
                shared_stroke(ui.visuals(), 2.0),
                egui::StrokeKind::Inside,
            );
            chip_link_mark(ui, r.rect);
        }
        let entries: Vec<&Docked> = dock.iter().filter(|d| d.slot == k).collect();
        r.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                true,
                chip_name(model, &letter, mk, &entries),
            )
        });
        plans.push(plan_dock(
            model,
            r.rect,
            own,
            &entries,
            chip_locks(v, Some(k)),
            folded,
        ));
    }
    // K12: an add/remove entry that matches no file.
    // Note DU1: its menu removes the entry (an entry with no files left: Delete set).
    for m in &v.missing {
        let r = letter_box(ui, "?", Some(RED), None).interact(egui::Sense::click());
        fixed.push(r.rect);
        let text = m.text();
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Missing: {text}"))
        });
        let r = r.on_hover_text(format!("no file matches {text}"));
        *member_menu |= r.secondary_clicked();
        let open = r.context_menu(|ui| {
            let (label, act) = match m {
                Missing::NoFilesLeft => ("Delete set", Act::Delete(v.id.clone())),
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
        *member_menu |= open.is_some();
    }
    for (_, cell) in plans.iter().flat_map(|plan| &plan.cells) {
        match cell {
            PlannedCell::Badge(d, _) => {
                represented.insert(&d.helper);
            }
            PlannedCell::More(_, groups) => {
                represented.extend(groups.iter().map(|g| g.key.as_str()))
            }
        }
    }
    // Members in no chip (an env mask `_m`, a clashing second kind) follow as plain thumbnails.
    for mk in &v.members {
        if represented.contains(mk.as_str()) {
            continue;
        }
        // F3a Task 8: the slot the member fills in this set, if any, has a slot menu.
        let owns = |f: &SlotFill| matches!(f, SlotFill::Own { item } if item == mk);
        let kind = v.slots.iter().find(|(_, f)| owns(f)).map(|(k, _)| *k);
        // A file the JSON does not write (its slot holds a link, or is cleared) draws no thumbnail: the slot's own
        // chip speaks for it (review E1). `chip_rows` does not count it either: it fills a slot.
        if kind.is_some_and(|k| {
            matches!(
                slot_shown(v, k),
                SlotShown::Replaced { .. } | SlotShown::Linked(_) | SlotShown::Cleared { .. }
            )
        }) {
            continue;
        }
        let Some(item) = model.item(mk) else {
            continue;
        };
        // Named by its file and why it is here (REPORT M1); an unused helper is dimmed.
        let role = match (kind, &item.helper, &item.job) {
            (Some(k), ..) => {
                format!("{} (its slot is taken)", v.slot_text(k))
            }
            // Note JW7: the small tile keeps the unused reason in its name and hover.
            (None, Some(h), _) => helper_tag_text(h),
            (None, None, Some(j)) => subtype_name(j.profile.subtype),
            _ => "not converted".to_string(),
        };
        let r = if item.kind == ItemKind::Helper && kind.is_none() {
            let was = ui.opacity();
            if item.helper.as_ref().is_some_and(|h| h.fills.is_empty()) {
                ui.multiply_opacity(ui.visuals().disabled_alpha());
            }
            let r = helper_tile(
                ui,
                model,
                thumbs,
                v,
                mk,
                &role,
                sel_items,
                acts,
                member_menu,
            );
            ui.set_opacity(was);
            r
        } else {
            let r = member_chip(
                ui,
                model,
                thumbs,
                v,
                mk,
                kind,
                true,
                ChipDrag::Item,
                Some(&|| role.clone()),
                sel_items,
                acts,
                member_menu,
            );
            if let Some(r) = &r {
                paint_letter(ui, r.rect, &tile_tag(model, v, mk, kind), false, CHIP - 2.0);
                r.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        true,
                        format!("{}, {role}", item.name),
                    )
                });
            }
            r
        };
        if let Some(r) = r {
            fixed.push(r.rect);
        }
    }
    (plans, fixed)
}

/// The set's Shader dropdown (M6 spec §7): the engine's pick with its provenance, then "Auto" and the mode's
/// shaders. Disabled when sets are not editable.
fn shader_combo(
    ui: &mut egui::Ui,
    model: &AppModel,
    v: &SetView,
    editing: &Editing,
    acts: &mut Vec<Act>,
) -> egui::Rect {
    let text = v.shader.as_ref().map_or(String::new(), |p| {
        let tag = match p.provenance {
            ShaderOrigin::Set => "",
            ShaderOrigin::Auto(_) => " (auto)",
            ShaderOrigin::Fallback => " (fallback)",
            ShaderOrigin::Derived => " (derived)",
        };
        format!("{}{tag}", p.shader.label())
    });
    // Note HP: its own row under the title, right-aligned and as wide as its value (capped at the card's inner width
    // less COMBO_FREE). The row's free left part allocates nothing, so a click there reaches the card.
    let (w, natural) = combo_w(ui, &text);
    let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(
        row,
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.add_enabled_ui(editing.on, |ui| {
                let r = super::helper_panel::combo(ui, ("shader", &v.id), &text, w, |ui| {
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
                super::helper_panel::named(
                    &r,
                    egui::Role::ComboBox,
                    &format!("Shader for {}", v.id),
                    Some(&text),
                );
                if natural > w {
                    // a read-only card's combo is disabled: only the disabled hover reaches it
                    r.clone().on_hover_text(&text).on_disabled_hover_text(&text);
                }
                r.rect
            })
            .inner
        },
    )
    .inner
}

/// The free card area left of the shader combo, which still opens the set (Note HP12).
const COMBO_FREE: f32 = 48.0;

/// The shader combo's width: its shown text, the arrow and the padding, capped at the card's inner width less
/// `COMBO_FREE`; and the uncapped width, to tell a cut value.
fn combo_w(ui: &egui::Ui, shown: &str) -> (f32, f32) {
    let sp = ui.spacing();
    let natural = text_width(ui, shown, &egui::TextStyle::Button.resolve(ui.style()))
        + sp.icon_width
        + sp.icon_spacing
        + 2.0 * sp.button_padding.x;
    let floor = sp.icon_width + 2.0 * sp.button_padding.x;
    let cap = (ui.available_width() - COMBO_FREE).max(floor);
    (natural.min(cap), natural)
}

/// The Rename submenu of a set's menu: label, one-line hover text, and what it does. F3b D5 for the last two.
/// Rename outputs is absent, not greyed, on a set that does not rename its outputs (Note HJ1).
fn rename_entries(set: &str, outputs: bool) -> Vec<(String, &'static str, Act)> {
    let mut v = vec![(
        "Rename…".to_string(),
        "Change the set's name only",
        Act::OpenRename(set.to_string()),
    )];
    if outputs {
        v.push((
            format!("Rename outputs…  {}", super::actions::keys_text("rename")),
            "Change the names of the files this set writes",
            Act::Bulk(MenuAction::RenameOutputs),
        ));
    }
    v.push((
        "Rename inputs…".to_string(),
        "Read the source files under other names; the files on disk keep theirs",
        Act::Bulk(MenuAction::RenameInputs),
    ));
    v
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
    crate::look::menu_button_036(ui, "Show in Explorer", |ui| {
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
    // Note HF9: an export, not a set edit, so it works while sets are not editable.
    // Note HF10: a set whose files are all gone has no key, so nothing to export.
    let off = if v.key.is_empty() {
        Some("This set has no files to export")
    } else {
        editing
            .exporting
            .then_some(crate::model::also::EXPORT_RUNNING)
    };
    if export_button(
        ui,
        off,
        "Export set",
        "Export only this set (and any set it borrows from); the rest of the export folder stays as it is",
    )
    .clicked()
    {
        acts.push(Act::ExportSet(v.key.clone(), v.id.clone()));
        ui.close();
    }
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
    let rename = crate::look::menu_button_036(ui, "Rename", |ui| {
        for (label, hover, act) in rename_entries(&v.id, model.renames_outputs(v)) {
            if tip(ui.button(label).on_hover_text(hover)).clicked() {
                acts.push(act);
                ui.close();
            }
        }
    });
    tip(rename.response);
    if tip(ui.button("Edit match…")).clicked() {
        acts.push(Act::Click(v.id.clone(), false, false));
        ui.close();
    }
    let remove = crate::look::menu_button_036(ui, "Remove member", |ui| {
        for mk in &v.members {
            let name = model.item(mk).map_or(mk.as_str(), |i| i.name.as_str());
            if ui.button(name).clicked() {
                acts.push(Act::RemoveMember(v.id.clone(), mk.clone()));
                ui.close();
            }
        }
    });
    tip(remove.response);
    // Note HF6: the same entries as the bulk menu, read only while the menu is open.
    let ids = [v.id.clone()];
    let (to_group, picked) = super::columns::group_submenu(ui, &model.group_entries(&ids));
    if let Some(action) = picked {
        acts.push(match action {
            MenuAction::ToGroup(n) => Act::MoveToGroup(ids.to_vec(), GroupTarget::Existing(n)),
            MenuAction::ToNewGroup => Act::OpenNewGroup(ids.to_vec()),
            _ => Act::MoveToGroup(ids.to_vec(), GroupTarget::None),
        });
        ui.close();
    }
    tip(to_group);
    // Note HF1: any set; its textures go to no set.
    if tip(ui.button("Delete set")).clicked() {
        acts.push(Act::Delete(v.id.clone()));
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
    // Note HL4: a scan or a target mod gives the dialog something to list. A PGPatcher PBR set's record rows are
    // refused in the dialog; with a target mod its mesh and file rows still map, without one nothing could. The scan
    // reason comes first (HG8c, HL4).
    let has_target = model
        .sets_doc
        .as_ref()
        .is_some_and(|d| d.target_mod().is_some());
    let why_not = if model.known.records.is_empty() && !has_target {
        Some("Scan your mods first")
    } else if model.renames_outputs(v) && !has_target {
        Some("PGPatcher PBR sets name their outputs in the JSON; use Rename outputs")
    } else {
        None
    };
    let label = if v.txst.is_some() {
        "Change mapping…"
    } else {
        "Map to target…"
    };
    let map = ui.add_enabled(why_not.is_none(), egui::Button::new(label));
    let map = match why_not {
        Some(why) if editing.on => map.on_disabled_hover_text(why),
        _ => tip(map),
    };
    if map.clicked() {
        acts.push(Act::MapTarget(v.id.clone()));
        ui.close();
    }
    if v.txst.is_some() && tip(ui.button("Remove mapping")).clicked() {
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

/// Note JW6: accessible name, hover and keyboard hint for the remembered answer.
pub const DEFAULT_SUFFIX: &str = " (default, Enter)";
pub const DEFAULT_HOVER: &str =
    "Your last choice. Enter picks it; hold Shift as you drop to skip this question.";
pub const DEFAULT_HINT: &str =
    "Enter picks the highlighted choice. Hold Shift when you drop to skip this question.";

fn choose(model: &mut AppModel, item: String, set: String, c: DropChoice) -> Option<Pending> {
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

/// The drop, delete and rename dialogs. Drawn by `ui::draw_with_scan` whether or not this column shows: a file
/// dropped on a slot card asks its Replace here too (item 17).
pub fn pending_modal(ctx: &egui::Context, model: &mut AppModel, st: &mut DropState) {
    if !matches!(st.pending, Some(Pending::Choice { .. })) {
        st.choice_options = None;
    } else if model.dialog.is_some() {
        // The foreground dialog draws later this frame. Leave its input (including Escape) for it.
        return;
    }
    if matches!(st.pending, Some(Pending::Rename { .. })) {
        rename_modal(ctx, model, st);
        return;
    }
    if matches!(
        st.pending,
        Some(Pending::RenameGroup { .. } | Pending::NewGroup { .. })
    ) {
        group_name_modal(ctx, model, st);
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
    if matches!(st.pending, Some(Pending::MapTarget(_))) {
        map_target_modal(ctx, model, st);
        return;
    }
    let Some(p) = st.pending.clone() else { return };
    use super::dialogs::Role;
    let mut dragged = None;
    // Per case: the title, the body line, and the footer as (label, role, what it answers).
    let replace_prompt = |set: &str, name: &str| {
        // K1: Enter does nothing here, so Cancel is plain and nothing is filled.
        let footer = vec![
            (
                format!("Replace {}", super::dialogs::short_name(name)),
                Role::Destructive,
                ModalAnswer::Replace,
            ),
            ("Cancel".to_string(), Role::Plain, ModalAnswer::Close),
        ];
        (
            "Replace?".to_string(),
            Some(format!("{set} already has {name} in that slot.")),
            footer,
        )
    };
    let (title, line, footer) = match &p {
        Pending::Choice { item, from, .. } => {
            dragged = model.item(item);
            let name = dragged.map_or(item.as_str(), |i| i.name.as_str());
            // K5: the drop choices stay in the body; the footer has only Cancel.
            let footer = vec![("Cancel".to_string(), Role::Plain, ModalAnswer::Close)];
            (format!("{name} is in {from}"), None, footer)
        }
        Pending::Replace { set, existing, .. }
        | Pending::BorrowReplace {
            set,
            existing:
                BorrowOccupant::Own { item: existing } | BorrowOccupant::Borrowed { item: existing, .. },
            ..
        } => replace_prompt(
            set,
            model.item(existing).map_or(existing.as_str(), |i| &i.name),
        ),
        Pending::BorrowReplace {
            set,
            existing: BorrowOccupant::Record { path },
            ..
        } => replace_prompt(set, path),
        Pending::Refused(why) => {
            let footer = vec![("OK".to_string(), Role::Default, ModalAnswer::Close)];
            ("Can't do that".to_string(), Some(why.clone()), footer)
        }
        Pending::Rename { .. }
        | Pending::RenameGroup { .. }
        | Pending::NewGroup { .. }
        | Pending::FillNames { .. }
        | Pending::MoveTo { .. }
        | Pending::MapTarget(_) => return,
    };
    let buttons: Vec<(&str, _, bool)> = footer.iter().map(|b| (b.0.as_str(), b.1, true)).collect();
    let mut answer = None;
    let mut def = None;
    let (keys, clicked) =
        super::dialogs::dialog(ctx, "set-drop", true, 420.0, &title, &buttons, |ui| {
            if let Some(line) = &line {
                ui.label(line);
            }
            if let Pending::Choice { item, set, .. } = &p {
                // Note JW5: helpers have no Duplicate button; a raw helper shares the file itself.
                let raw_helper = dragged.is_some_and(is_shareable_helper);
                let share = if raw_helper {
                    "Share (use this file here too, no copy)"
                } else {
                    "Share (borrow its finished output)"
                };
                let last = st.last_choice;
                for &(c, ref refusal) in st.choice_buttons(model, item, set) {
                    let label = match c {
                        DropChoice::Move => "Move it here",
                        DropChoice::Share => share,
                        DropChoice::Duplicate => "Duplicate the source for this set",
                    };
                    let usable = refusal.is_none();
                    let remembered = usable && last == Some(c);
                    let mut button = egui::Button::new(label);
                    if remembered {
                        def = Some(c);
                        button = button.fill(ui.visuals().selection.bg_fill);
                    }
                    let mut r = ui.add_enabled(usable, button);
                    if remembered {
                        r.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::Button,
                                true,
                                format!("{label}{DEFAULT_SUFFIX}"),
                            )
                        });
                        r = r.on_hover_text(DEFAULT_HOVER);
                    }
                    if let Some(why) = refusal {
                        ui.ctx()
                            .accesskit_node_builder(r.id, |b| b.set_description(why.as_str()));
                        r.on_disabled_hover_text(why.as_str());
                    } else if r.clicked() {
                        answer = Some(ModalAnswer::Choice(c));
                    }
                }
                if def.is_some() {
                    ui.label(egui::RichText::new(DEFAULT_HINT).weak());
                }
            }
        });
    if let Some(i) = clicked {
        answer = Some(footer[i].2);
    }
    // Esc and a click outside cancel; Enter presses the OK of a refusal.
    let refusal = matches!(p, Pending::Refused(_));
    if keys.cancel {
        answer = Some(ModalAnswer::Close);
    } else if answer.is_none() && keys.enter {
        answer = if refusal {
            Some(ModalAnswer::Close)
        } else {
            def.map(ModalAnswer::Choice)
        };
    }
    let Some(answer) = answer else { return };
    st.choice_options = None;
    st.pending = match (answer, p) {
        (ModalAnswer::Choice(c), Pending::Choice { item, set, .. }) => {
            let next = choose(model, item, set, c);
            if !matches!(next, Some(Pending::Refused(_))) {
                st.last_choice = Some(c);
            }
            next
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
        (
            ModalAnswer::Replace,
            Pending::BorrowReplace {
                item,
                from,
                set,
                kind,
                existing,
            },
        ) => {
            let _ = model.replace_borrow(&item, &from, &set, kind, &existing);
            None
        }
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
            let field = super::name_input(
                ui.add(egui::TextEdit::singleline(text).margin(crate::look::TEXT_EDIT_MARGIN)),
                "New set name",
            );
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

/// Note HF6: the name dialog of Rename on a group's header and of New group… on a set's menu. A taken or empty name
/// stays open with the reason; nothing is written until a name is accepted.
fn group_name_modal(ctx: &egui::Context, model: &mut AppModel, st: &mut DropState) {
    let (title, text, error) = match &mut st.pending {
        Some(Pending::RenameGroup { name, text, error }) => {
            (format!("Rename group {name}"), text, error)
        }
        Some(Pending::NewGroup { text, error, .. }) => ("New group".to_string(), text, error),
        _ => return,
    };
    use super::dialogs::Role;
    let mut submit = false;
    // The field's own Enter accepts, so no button is the Default.
    let buttons = [("Cancel", Role::Plain, true), ("OK", Role::Plain, true)];
    // Only Esc cancels: a click outside could lose the typed name (AG11).
    let (keys, clicked) =
        super::dialogs::dialog(ctx, "group-name", false, 420.0, &title, &buttons, |ui| {
            let field =
                ui.add(egui::TextEdit::singleline(text).margin(crate::look::TEXT_EDIT_MARGIN));
            // A TextEdit takes no label from its text, so the box gets its name here.
            field.widget_info(|| egui::WidgetInfo {
                label: Some("Group name".into()),
                ..egui::WidgetInfo::text_edit(true, text.as_str(), text.as_str(), "")
            });
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
        let done = match st.pending.clone() {
            Some(Pending::RenameGroup { name, text, .. }) => model.rename_group(&name, &text),
            Some(Pending::NewGroup { ids, text, .. }) => {
                model.move_to_group(&ids, GroupTarget::New(text))
            }
            _ => return,
        };
        match done {
            Ok(()) => st.pending = None,
            Err(e) => {
                if let Some(Pending::RenameGroup { error, .. } | Pending::NewGroup { error, .. }) =
                    &mut st.pending
                {
                    *error = Some(e);
                }
            }
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
                    .margin(crate::look::TEXT_EDIT_MARGIN)
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

/// The kind column of a Map to target… row.
fn pick_kind(p: &TargetPick) -> &'static str {
    match p {
        TargetPick::Record(_) => "Record",
        TargetPick::Mesh(_) => "Mesh",
        TargetPick::Files(_) => "File",
    }
}

/// The path a row's "used by" count is about: its slot 1.
fn pick_slot1(p: &TargetPick) -> Option<&str> {
    let slots = match p {
        TargetPick::Record(r) => &r.slots,
        TargetPick::Mesh(m) => &m.slots,
        TargetPick::Files(f) => &f.slots,
    };
    slots.get(&1).map(String::as_str)
}

/// The list the dialog shows: the target's records (every scan record with no target mod), then its mesh paths, then
/// its file bases.
fn target_rows(model: &AppModel) -> Vec<TargetPick> {
    let mut rows: Vec<TargetPick> = model
        .target_records()
        .into_iter()
        .cloned()
        .map(TargetPick::Record)
        .collect();
    if let Some(t) = model.target() {
        rows.extend(t.meshes.iter().cloned().map(TargetPick::Mesh));
        rows.extend(t.files.iter().cloned().map(TargetPick::Files));
    }
    rows
}

/// One list row of Map to target…: the kind, the EDID or path, and `used by (N)`, each cut to its cell with the whole
/// text on hover and in the row's AccessKit name. `why`: the hover of a row that can't be picked (drawn disabled).
fn target_row(
    ui: &mut egui::Ui,
    pick: &TargetPick,
    used: u32,
    selected: bool,
    why: Option<&str>,
) -> egui::Response {
    const KIND_W: f32 = 56.0;
    const USED_W: f32 = 88.0;
    let h = ui.spacing().interact_size.y;
    let main = match pick {
        TargetPick::Record(r) => {
            let plugin = if r.winner.eq_ignore_ascii_case(&r.plugin) {
                r.plugin.clone()
            } else {
                format!("{} (won by {})", r.plugin, r.winner)
            };
            format!("{} ({}, {})", r.edid, r.formid, plugin)
        }
        TargetPick::Mesh(m) => m.base.clone(),
        TargetPick::Files(f) => f.base.clone(),
    };
    let kind = pick_kind(pick);
    let used_text = format!("used by ({used})");
    let name = format!("{kind} {main} · {used_text}");
    let resp = ui
        .add_enabled_ui(why.is_none(), |ui| {
            let (rect, resp) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
            resp.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::Button, why.is_none(), selected, &name)
            });
            if !ui.is_rect_visible(rect) {
                return resp;
            }
            if selected {
                ui.painter()
                    .rect_filled(rect, 2.0, ui.visuals().selection.bg_fill);
            } else if resp.hovered() && why.is_none() {
                ui.painter()
                    .rect_filled(rect, 2.0, ui.visuals().widgets.hovered.weak_bg_fill);
            }
            let (x0, x1) = (rect.min.x + 4.0, rect.max.x - 4.0);
            let cells = [
                (kind, x0, x0 + KIND_W - 4.0, !selected),
                (main.as_str(), x0 + KIND_W, x1 - USED_W, false),
                (used_text.as_str(), x1 - USED_W + 4.0, x1, !selected),
            ];
            for (text, a, b, weak) in cells {
                let cell = egui::Rect::from_min_max(
                    egui::pos2(a, rect.min.y),
                    egui::pos2(b.max(a), rect.max.y),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(cell)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        let t = egui::RichText::new(text);
                        let t = if weak { t.weak() } else { t };
                        ui.add(egui::Label::new(t).truncate().selectable(false));
                    },
                );
            }
            resp
        })
        .inner;
    match why {
        Some(w) => resp.on_disabled_hover_text(w),
        None => resp,
    }
}

const TARGET_ROW: &str = "Target mod (read only; Export writes to the export folder)";
const WHY_IGNORED: &str = "PGPatcher PBR sets name their outputs in the JSON; use Rename outputs";

/// Map to target… (Notes HG, HL4): the target mod row, one list of its records, mesh paths and file bases (every scan
/// record with no target mod), then what each file would write. It writes nothing before Map. Enter maps the chosen
/// row while no widget has focus; Esc or Cancel closes; a click outside does not (it holds the typed text). The list
/// is virtual: the scan holds thousands of records.
fn map_target_modal(ctx: &egui::Context, model: &mut AppModel, st: &mut DropState) {
    let Some(Pending::MapTarget(state)) = &mut st.pending else {
        return;
    };
    let MapTargetState {
        id,
        query,
        rows,
        hits,
        seen,
        used,
        sel,
    } = &mut **state;
    use super::dialogs::Role;
    let first = seen.is_none();
    let known = model.known.clone();
    // The stored path too: a chosen target has no read until it lands, and its rows are none, not every record.
    let stamp = (
        std::sync::Arc::as_ptr(&known) as usize,
        model
            .target()
            .map_or(0, |t| std::sync::Arc::as_ptr(t) as usize),
        model.sets_doc.as_ref().and_then(|d| d.target_mod()),
    );
    // Only a changed scan or target rebuilds the rows; a changed query filters them again.
    let stale = seen.as_ref().is_none_or(|(_, s)| *s != stamp);
    if stale {
        *rows = target_rows(model);
        used.clear();
        if sel.as_ref().is_some_and(|s| !rows.contains(s)) {
            *sel = None;
        }
    }
    if stale || seen.as_ref().is_none_or(|(q, _)| q != query) {
        *hits = crate::model::txst_map::pick_hits(rows, query);
        *seen = Some((query.clone(), stamp));
    }
    // A PGPatcher PBR set on Community Shaders ignores a record; its mesh and file rows still work.
    let (ignored, mapped) = crate::model::sets::views(model)
        .iter()
        .find(|v| engine::sets::same_id(&v.id, id))
        .map_or((false, false), |v| {
            (model.renames_outputs(v), v.txst.is_some())
        });
    let off = |p: &TargetPick| ignored && matches!(p, TargetPick::Record(_));
    let can_map = sel.as_ref().is_some_and(|p| !off(p));
    let target_path = model.sets_doc.as_ref().and_then(|d| d.target_mod());
    let no_records = model
        .target()
        .filter(|_| !rows.iter().any(|r| matches!(r, TargetPick::Record(_))))
        .map(|t| {
            if t.plugins.is_empty() {
                crate::model::target_mod::RECORDS_NO_PLUGIN
            } else {
                crate::model::target_mod::RECORDS_NOT_IN_SCAN
            }
        });
    let title = if mapped {
        "Change mapping…"
    } else {
        "Map to target…"
    };
    let buttons = [
        ("Cancel", Role::Plain, true),
        ("Map", Role::Default, can_map),
    ];
    let pick_first = if sel.is_some() {
        WHY_IGNORED
    } else {
        "Pick a row first"
    };
    let hovers = [None, Some(pick_first)];
    let (mut choose, mut clear, mut typed_enter) = (false, false, false);
    let (keys, clicked) = super::dialogs::dialog_hover(
        ctx,
        "set-map-target",
        false,
        640.0,
        title,
        &buttons,
        &hovers,
        |ui| {
            ui.add(egui::Label::new(egui::RichText::new(format!("Set: {id}")).weak()).truncate());
            ui.label(TARGET_ROW);
            egui::Sides::new().shrink_left().truncate().show(
                ui,
                |ui| match &target_path {
                    Some(p) => {
                        let full = p.display().to_string();
                        let r = ui.add(
                            egui::Label::new(super::header::elide_middle(&full, 60))
                                .truncate()
                                .show_tooltip_when_elided(false),
                        );
                        r.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::Label,
                                true,
                                format!("Target mod folder: {full}"),
                            )
                        });
                        r.on_hover_text(&full);
                    }
                    None => {
                        let text = "No target mod: the list shows every record of the scan";
                        ui.add(egui::Label::new(egui::RichText::new(text).weak()).truncate());
                    }
                },
                |ui| {
                    if target_path.is_some() && ui.button("Clear").clicked() {
                        clear = true;
                    }
                    let choose_button = egui::Button::new("Choose…");
                    choose = crate::model::picker::button(ui, &model.picker, choose_button);
                },
            );
            if let Some(e) = model.target_pick_error {
                ui.colored_label(RED, e);
            }
            if let Some(line) = no_records {
                ui.weak(line);
            }
            // Before the field, so it does not move its cursor with them (as Move to… does). They only move the
            // highlight.
            let (up, down) = ui.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                )
            });
            let field = ui.add(
                egui::TextEdit::singleline(query)
                    .margin(crate::look::TEXT_EDIT_MARGIN)
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
                label: Some("Search targets".into()),
                ..egui::WidgetInfo::text_edit(
                    true,
                    query.as_str(),
                    query.as_str(),
                    "EDID, plugin, FormID or path",
                )
            });
            // The field gives up its focus on Enter, so this frame's Enter was typed there and maps nothing.
            typed_enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if first && !field.has_focus() && !field.lost_focus() {
                field.request_focus();
            }
            let noun = if rows.iter().all(|r| matches!(r, TargetPick::Record(_))) {
                "records"
            } else {
                "rows"
            };
            ui.weak(format!("{} of {} {noun}", hits.len(), rows.len()));
            let row_h = ui.spacing().interact_size.y;
            let mut moved = None;
            if up || down {
                let at = sel
                    .as_ref()
                    .and_then(|s| hits.iter().position(|&i| rows.get(i) == Some(s)));
                let order: Vec<usize> = match (at, down) {
                    (None, _) => (0..hits.len()).collect(),
                    (Some(p), true) => (p + 1..hits.len()).collect(),
                    (Some(p), false) => (0..p).rev().collect(),
                };
                let row_at = |n: usize| hits.get(n).and_then(|&i| rows.get(i));
                let next = order
                    .into_iter()
                    .find(|&n| row_at(n).is_some_and(|r| !off(r)));
                if let Some((n, r)) = next.and_then(|n| Some((n, row_at(n)?))) {
                    *sel = Some(r.clone());
                    moved = Some(n);
                }
            }
            // `show_rows` builds only the rows on screen, so a moved highlight scrolls by offset, from last frame's
            // offset and view height.
            let view_id = egui::Id::unique("map-target-list-view");
            let (off_y, view) = ui
                .data(|d| d.get_temp::<(f32, f32)>(view_id))
                .unwrap_or((0.0, row_h * 9.0));
            // A bar that shows without hovering (egui-ui-rules 3.2); the dialog's own ui, so nothing else changes.
            ui.spacing_mut().scroll = crate::look::thin_scroll();
            let mut area = egui::ScrollArea::vertical()
                .id_salt("map-target-list")
                .max_height(row_h * 9.0)
                .auto_shrink([false, true]);
            if let Some(n) = moved {
                let top = n as f32 * (row_h + ui.spacing().item_spacing.y);
                if top < off_y {
                    area = area.vertical_scroll_offset(top);
                } else if top + row_h > off_y + view {
                    area = area.vertical_scroll_offset(top + row_h - view);
                }
            }
            let out = area.show_rows(ui, row_h, hits.len(), |ui, range| {
                for n in range {
                    let i = hits[n];
                    let Some(p) = rows.get(i) else { continue };
                    let count = *used
                        .entry(i)
                        .or_insert_with(|| pick_slot1(p).map_or(0, |s| model.used_by(s)));
                    let why = off(p).then_some(WHY_IGNORED);
                    if target_row(ui, p, count, sel.as_ref() == Some(p), why).clicked() {
                        *sel = Some(p.clone());
                    }
                }
            });
            ui.data_mut(|d| {
                d.insert_temp(
                    view_id,
                    (out.state.clamped_offset().y, out.inner_rect.height()),
                )
            });
            if let Some(p) = sel.as_ref() {
                ui.separator();
                let head = match p {
                    TargetPick::Record(r) => format!("{} ({}, {})", r.edid, r.formid, r.plugin),
                    TargetPick::Mesh(m) => m.base.clone(),
                    TargetPick::Files(f) => f.base.clone(),
                };
                ui.add(egui::Label::new(egui::RichText::new(head).strong()).truncate());
                egui::ScrollArea::vertical()
                    .id_salt("map-target-preview")
                    .max_height(row_h * 5.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| match p {
                        TargetPick::Record(r) => {
                            for (file, path) in model.record_preview(id, r) {
                                match path {
                                    Some(p) => {
                                        ui.add(egui::Label::new(format!("{file} → {p}")).truncate())
                                    }
                                    None => ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(format!(
                                                "{file}: keeps its own path"
                                            ))
                                            .weak(),
                                        )
                                        .truncate(),
                                    ),
                                };
                            }
                        }
                        TargetPick::Mesh(MeshPath { slots, .. })
                        | TargetPick::Files(FileBase { slots, .. }) => {
                            for (n, path) in slots {
                                ui.add(egui::Label::new(format!("slot{n}: {path}")).truncate());
                            }
                        }
                    });
            }
        },
    );
    if choose {
        model.pick(ctx, PickerPurpose::TargetMod, PickerSpec::folder());
    }
    if clear {
        model.target_pick_error = model.set_target_mod(None).err();
    }
    if keys.cancel || clicked == Some(0) {
        st.pending = None;
    } else if clicked == Some(1) || (keys.enter && can_map && !typed_enter) {
        if let Some(Pending::MapTarget(state)) = st.pending.take() {
            let MapTargetState { id, sel, .. } = *state;
            let Some(p) = sel else { return };
            if let Err(why) = model.map_target(&id, p) {
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
    use egui_kittest::kittest::{NodeT, Queryable};
    use engine::codes::Code;

    #[test]
    fn the_empty_strip_is_sized_by_its_slots() {
        assert_eq!(strip_size(1, 5), egui::vec2(40.0, 40.0));
        assert_eq!(strip_size(4, 5), egui::vec2(184.0, 40.0));
        assert_eq!(strip_size(6, 5), egui::vec2(232.0, 80.0));
    }

    #[test]
    fn the_empty_strip_sits_below_flips_above_and_clamps() {
        let clip = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(266.0, 600.0));
        let size = egui::vec2(40.0, 40.0);
        let at = |x0, y0, x1, y1| egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1));
        assert_eq!(
            strip_rect(at(10.0, 100.0, 250.0, 200.0), size, clip),
            egui::Rect::from_min_size(egui::pos2(10.0, 202.0), size)
        );
        assert_eq!(
            strip_rect(at(10.0, 500.0, 250.0, 590.0), size, clip),
            egui::Rect::from_min_size(egui::pos2(10.0, 458.0), size)
        );
        assert_eq!(
            strip_rect(at(10.0, 20.0, 250.0, 590.0), size, clip),
            egui::Rect::from_min_size(egui::pos2(10.0, 560.0), size)
        );
        assert_eq!(
            strip_rect(at(240.0, 100.0, 260.0, 200.0), size, clip).max.x,
            266.0
        );
    }

    /// Note JJ2: a fresh layout's Sets column still holds five chips a row beside the thin bar's gutter (260 held
    /// exactly five with no gutter; with it, four).
    #[test]
    fn the_default_sets_width_holds_five_chips_beside_the_bar() {
        // The app's style: `fonts::install_system_fonts` applies the pinned 0.36.2 look.
        let mut style = egui::Style::default();
        crate::look::pin_036_style(&mut style);
        let frame_x = egui::Frame::group(&style).total_margin().sum().x + 2.0 * DOCK_INSET;
        let card = crate::ui::SETS_W
            - super::super::columns::PANEL_CHROME
            - crate::look::thin_scroll().allocated_width();
        assert_eq!(chips_per_row(card, frame_x), 5, "a card {card} px wide");
    }

    /// Note HF6: the header menu's edits are off with the reason while `sdc-project.toml` is read-only, and Select all
    /// sets still works. A read-only document holds no groups, so no header shows then; the menu is drawn on its own.
    #[test]
    fn the_group_menu_edits_are_off_when_read_only() {
        let head = Head {
            name: "Armor".into(),
            total: 2,
            shown: 2,
            status: Status::Ready,
            excluded: false,
            collapsed: false,
        };
        let editing = Editing {
            on: false,
            why: "read-only because of the file",
            reorder: false,
            exporting: false,
        };
        let mut acts = vec![];
        let mut h = egui_kittest::Harness::builder()
            .with_context_setup(|ctx| {
                crate::fonts::install_system_fonts(ctx).expect("system fonts");
            })
            .build_ui(|ui| group_menu(ui, &head, &editing, &mut acts));
        h.run();
        for off in ["Exclude from export", "Rename…", "Ungroup"] {
            assert!(h.get_by_label(off).accesskit_node().is_disabled(), "{off}");
        }
        assert!(!h
            .get_by_label("Select all sets")
            .accesskit_node()
            .is_disabled());
    }

    fn link(path: &str, row: SlotKind) -> SlotLink {
        SlotLink {
            path: path.into(),
            from: None,
            from_short: None,
            item: None,
            kind: None,
            row: Some(row),
        }
    }

    /// How many chips `chip_row` draws for `v` (every chip pushes its rect on `fixed`), and what `chip_rows` counts
    /// for it when a row holds one chip.
    fn drawn_and_counted(v: &SetView) -> (usize, usize) {
        let session = std::sync::Arc::new(engine::session::Session::new().expect("a session"));
        let model = AppModel::new(crate::settings::GuiSettings::default(), None, session);
        let editing = Editing {
            on: true,
            why: "",
            reorder: false,
            exporting: false,
        };
        let drawn = std::rc::Rc::new(std::cell::Cell::new(0));
        let out = drawn.clone();
        let mut thumbs = ThumbCache::default();
        let mut h = egui_kittest::Harness::builder()
            .with_context_setup(|ctx| {
                crate::fonts::install_system_fonts(ctx).expect("system fonts");
            })
            .build_ui(move |ui| {
                let (_, fixed) = chip_row(
                    ui,
                    &model,
                    &mut thumbs,
                    v,
                    &[],
                    &mut FoldedBySlice::new(),
                    &editing,
                    &HashSet::new(),
                    &mut vec![],
                    &mut false,
                    vec![],
                );
                out.set(fixed.len());
            });
        h.run();
        (drawn.get(), chip_rows(v, 0.0, 0.0))
    }

    #[test]
    fn empty_slots_take_no_rows_in_the_height_estimate() {
        let mut v = SetView::new("k/a".into(), "a".into(), vec![]);
        v.shader = Some(engine::shader::ShaderPick {
            shader: Shader::Pbr,
            provenance: ShaderOrigin::Set,
            wanted: None,
        });
        v.empty = view_row(&v).iter().map(|c| c.kind).collect();
        assert!(!v.empty.is_empty());
        assert_eq!(drawn_and_counted(&v), (0, 0));
    }

    /// Review E1: `chip_rows` counts the chips `chip_row` draws for a linked slot, a cleared one, and a link over an
    /// own file of its kind. (A unit test has no items in its model, so the trailing members loop draws nothing here:
    /// that term is pinned by the drawn-equals-counted tests in `tests/set_cards.rs`.)
    #[test]
    fn chip_rows_counts_what_chip_row_draws_for_links_and_clears() {
        let mut linked = SetView::new("k/a".into(), "a".into(), vec!["a/d.png".into()]);
        linked.linked.insert(1, link("p\\x.dds", SlotKind::Diffuse));
        // a link over the set's own diffuse
        linked.slots.insert(
            SlotKind::Diffuse,
            SlotFill::Own {
                item: "a/d.png".into(),
            },
        );
        let mut cleared = SetView::new("k/b".into(), "b".into(), vec!["b/n.png".into()]);
        cleared.slots.insert(
            SlotKind::Normal,
            SlotFill::Own {
                item: "b/n.png".into(),
            },
        );
        cleared
            .settings
            .insert("slot2".into(), serde_json::Value::String(String::new()));
        let mut replaced = SetView::new("k/c".into(), "c".into(), vec!["c/f.png".into()]);
        replaced.slots.insert(
            SlotKind::Fuzz,
            SlotFill::Own {
                item: "c/f.png".into(),
            },
        );
        replaced.linked.insert(7, link("p\\c.dds", SlotKind::Coat));
        for v in [&linked, &cleared, &replaced] {
            let (drawn, counted) = drawn_and_counted(v);
            assert_eq!(drawn, counted, "{}", v.id);
            assert_eq!(drawn, 1, "one chip for {}", v.id);
        }
    }

    #[test]
    fn a_typed_path_names_its_file_and_a_trailing_separator_names_the_whole_path() {
        let l = link("textures\\x\\thing_d.dds", SlotKind::Diffuse);
        assert_eq!(linked_chip_name("D", &l), "D, path thing_d.dds");
        let l = link("textures\\x\\", SlotKind::Diffuse);
        assert_eq!(linked_chip_name("D", &l), "D, path textures\\x\\");
        let mut l = link("textures\\x\\thing_d.dds", SlotKind::Diffuse);
        l.from_short = Some("wall".into());
        assert_eq!(linked_chip_name("D", &l), "D, from set wall");
    }

    fn fit_at(id: &str, query: &str, n: usize) -> String {
        let tail = crate::model::sets::short_names(&[id]);
        fit_title(id, &tail[0], query, |t: &str| t.chars().count() <= n).0
    }

    #[test]
    fn fit_title_keeps_a_middle_match() {
        let id = "textures/architecture/stone/bricks/banner";
        assert_eq!(fit_at(id, "STONE", 21), "…/stone/bricks/banner");
        assert_ne!(fit_at(id, "", 21), fit_at(id, "STONE", 21));
        assert_eq!(fit_at("a/bb/stone/leaf", "ston", 13), "…/stone/leaf");
    }

    #[test]
    fn fit_title_without_a_middle_match_keeps_the_tail() {
        let id = "architecture/stone/banner";
        let plain = fit_at(id, "", 15);
        assert!(plain.ends_with("stone/banner"), "{plain}");
        for q in ["", "banner", "arch", "stone/ban"] {
            assert_eq!(fit_at(id, q, 15), plain, "query {q:?}");
        }
    }

    #[test]
    fn fit_title_with_a_match_that_does_not_fit_falls_through_to_the_tail() {
        let id = "architecture/stone/old/banner";
        assert_eq!(fit_at(id, "stone", 12), fit_at(id, "", 12));
        assert_eq!(fit_at(id, "stone", 12), "…/old/banner");
    }

    #[test]
    fn fit_title_keeps_the_unique_tail() {
        let narrow = |s: &str| s.chars().count() <= 8;
        assert_eq!(
            fit_title("textures/arch/eye/m", "eye/m", "", narrow),
            ("…/eye/m".into(), true)
        );
        assert_ne!(
            fit_title("textures/arch/eye/eye", "eye/eye", "", narrow),
            fit_title("textures/arch/ear/eye", "ear/eye", "", narrow)
        );
    }

    #[test]
    fn fit_title_never_merges_two_ids_under_a_search() {
        let narrow = |s: &str| s.chars().count() <= 10;
        // 18 characters each: neither fits whole, so both fall to the tail form, which wraps
        let a = fit_title("textures/a/stone/m", "a/stone/m", "stone", narrow);
        let b = fit_title("textures/b/stone/m", "b/stone/m", "stone", narrow);
        assert_ne!(a, b);
        assert!(a.0.ends_with("a/stone/m") && b.0.ends_with("b/stone/m"));
        assert!(!a.1 && !b.1);
    }

    #[test]
    fn a_tile_tag_is_the_slot_letter_or_the_tail_or_a_dash() {
        assert_eq!(tag_text("_m"), "_m");
        assert_eq!(tag_text(""), "—");
    }

    /// Final wave Group 6 (ledger 187): the last resort `…/<tail>` is never as long as the id it stands for.
    #[test]
    fn the_tail_form_is_never_longer_than_the_id() {
        let none = |_: &str| false;
        // "…/stone/m" is 9 characters, as many as the id: the id is shown
        assert_eq!(
            fit_title("x/stone/m", "stone/m", "", none),
            ("x/stone/m".into(), false)
        );
        assert_eq!(
            fit_title("textures/arch/eye/m", "eye/m", "", none),
            ("…/eye/m".into(), false)
        );
    }

    #[test]
    fn fit_title_shows_the_whole_id_when_it_fits() {
        let id = "architecture/stone/banner";
        let calls = std::cell::Cell::new(0);
        let result = fit_title(id, "stone/banner", "stone", |text| {
            calls.set(calls.get() + 1);
            text.chars().count() <= 40
        });
        assert_eq!(result, (id.to_string(), true));
        assert_eq!(calls.get(), 1, "the fitted title carries its measurement");
    }

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

    fn shared_area(a: egui::Rect, b: egui::Rect) -> f32 {
        let i = a.intersect(b);
        i.width().max(0.0) * i.height().max(0.0)
    }

    #[test]
    fn badge_rects_straddle_the_corner() {
        let chip = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::Vec2::splat(CHIP));
        let (drawn, hit) = badge_rects(chip, 0);
        assert_eq!(drawn.center(), chip.right_top());
        assert_eq!(drawn.size(), egui::Vec2::splat(16.0));
        assert_eq!(hit.center(), chip.right_top());
        assert_eq!(hit.size(), egui::Vec2::splat(20.0));
        let (drawn, hit) = badge_rects(chip, 1);
        assert_eq!(drawn.center(), chip.left_top());
        assert_eq!(hit.center(), chip.left_top());
    }

    #[test]
    fn clamp_hits_splits_and_stops_at_edges() {
        let s = egui::Vec2::splat(CHIP);
        let a = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), s);
        let b = a.translate(egui::vec2(CHIP + DOCK_GAP_X, 0.0));
        let c = a.translate(egui::vec2(0.0, CHIP + DOCK_GAP_Y));
        // 9 px = stroke 1 + margin 6 + inset 2 at the left; the card's top has the row's top margin as well
        let card = egui::Rect::from_min_max(
            egui::pos2(a.left() - 9.0, a.top() - 12.0),
            egui::pos2(b.right() + 9.0, c.bottom() + 9.0),
        );
        let fixed = [a, b, c];
        let hit = |chip: usize, pos: usize| (badge_rects(fixed[chip], pos), chip);
        // (hits as (rect, own chip), the drawn rects) for each case
        let cases = [vec![hit(0, 0), hit(1, 1)], vec![hit(0, 1)], vec![hit(2, 0)]];
        for case in cases {
            let hits: Vec<_> = case.iter().map(|((_, h), i)| (*h, *i)).collect();
            let out = clamp_hits(&hits, &fixed, card);
            for (k, (r, ((drawn, _), own))) in out.iter().zip(&case).enumerate() {
                assert!(
                    r.contains_rect(*drawn),
                    "case {case:?} #{k}: {r:?} holds {drawn:?}"
                );
                assert!(
                    r.width() >= 18.0 - 0.01 && r.height() >= 18.0 - 0.01,
                    "{r:?}"
                );
                assert!(card.contains_rect(*r), "{r:?} inside the card");
                for (j, f) in fixed.iter().enumerate() {
                    if j != *own {
                        assert_eq!(shared_area(*r, *f), 0.0, "{r:?} meets chip {j}");
                    }
                }
                for (j, o) in out.iter().enumerate() {
                    if j != k {
                        assert_eq!(shared_area(*r, *o), 0.0, "{r:?} meets badge {j} {o:?}");
                    }
                }
            }
        }
        let two = clamp_hits(
            &[(badge_rects(a, 0).1, 0), (badge_rects(b, 1).1, 1)],
            &fixed,
            card,
        );
        for r in &two {
            assert_eq!((r.width(), r.height()), (18.0, 20.0), "{r:?}");
        }
        assert_eq!(two[0].right(), a.right() + 8.0);
        assert_eq!(two[1].left(), a.right() + 8.0);
        let edge = clamp_hits(&[(badge_rects(a, 1).1, 0)], &fixed, card);
        assert_eq!((edge[0].left(), edge[0].width()), (card.left(), 19.0));
        let below = clamp_hits(&[(badge_rects(c, 0).1, 2)], &fixed, card);
        assert_eq!((below[0].top(), below[0].height()), (a.bottom(), 18.0));
        // a card whose top is under the badge's: the hit stops at the card's top
        let low = egui::Rect::from_min_max(egui::pos2(card.left(), a.top() - 4.0), card.max);
        let top = clamp_hits(&[(badge_rects(a, 0).1, 0)], &fixed, low);
        assert_eq!(top[0].top(), low.top(), "cut to the card's top");
        assert_eq!(fixed[0], a, "chips are not moved");
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
            reorder_target(Some("a"), Some(2), &v, &[]),
            Some(("a".to_string(), 2))
        );
        assert_eq!(
            reorder_target(Some("c"), Some(0), &v, &[]),
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
        assert_eq!(reorder_target(None, Some(1), &v, &[]), None);
        assert_eq!(reorder_target(Some("a"), None, &v, &[]), None);
        assert_eq!(reorder_target(Some("a"), Some(0), &v, &[]), None);
        assert_eq!(reorder_target(Some("a"), Some(9), &v, &[]), None);
    }

    #[test]
    fn a_groups_first_member_dropped_below_counts_without_its_block() {
        let v = ["a", "b", "c", "x", "d", "e"];
        let g = [None, Some(0), Some(0), Some(0), None, None];
        let t = |id, to| reorder_target(Some(id), Some(to), &v, &g);
        assert_eq!(t("b", 4), Some(("b".to_string(), 2)));
        assert_eq!(t("b", 2), None, "a drop on its own block does nothing");
        assert_eq!(t("b", 3), None, "a drop on its own block does nothing");
        assert_eq!(t("b", 0), Some(("b".to_string(), 0)));
        assert_eq!(
            t("c", 4),
            Some(("c".to_string(), 4)),
            "a later member moves alone"
        );
        assert_eq!(t("a", 4), Some(("a".to_string(), 4)));
    }
}
