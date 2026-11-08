//! The editable Sets column (M3b spec §3.1): set pills in project order with badges, drag and drop
//! of textures onto sets (and onto an empty area for a new set), drag-to-reorder by the ≡ handle, and a context
//! menu. Drawing only: every change is a model call, made after drawing.
use super::columns::{
    bulk_menu, member, status_text, AMBER, DOCK_GAP_X, DOCK_GAP_Y, DOCK_INSET, RED,
};
use super::slot_commands::SlotCmdState;
use super::status::Status;
use super::thumbs::ThumbCache;
use crate::model::bulk::MenuAction;
use crate::model::helpers::{docked, fill_text, Docked};
use crate::model::items::{helper_target_files, helper_target_text};
use crate::model::known_paths::TxstRecord;
use crate::model::labels::subtype_name;
use crate::model::pbr_export::export_blocks;
use crate::model::sets::{
    badge_text, fill_proposals, kind_text, member_tail, shader_options, shared_cells, slot_row,
    DropChoice, DropOutcome, FillProposal, Missing, SetView, SlotFill, SlotKind,
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
    /// Note HP: the Sets filter last frame, to tell a change from the first frame (`None`).
    pub filter: Option<crate::model::sets_filter::SetsFilter>,
}

/// Review I1: a reveal re-aims at most this many frames, so a key that names no visible card cannot loop.
const REVEAL_TRIES: u8 = 8;

/// R4: the height of a card never laid out, until any card has been measured (then their mean).
const CARD_EST: f32 = 110.0;

/// Note FM32: the rows `v`'s chips wrap into at column width `width` (`frame_x` is the card's side margins and
/// the chip rows' inset, Note HP). A count from the set's slot row, its
/// missing entries and its extra members, not a layout; the chips sit `DOCK_GAP_X` apart.
// shortcut: a helper docked on a chip still counts as an extra member here, so an unmeasured card may be estimated
// a row too tall (drawing corrects it, FM32); subtract the docked helpers if a scroll jump shows.
fn chip_rows(v: &SetView, width: f32, frame_x: f32) -> usize {
    let chips = shared_cells(v, slot_row(v.shader.as_ref(), &v.slots)).len()
        + v.missing.len()
        + v.members.len().saturating_sub(v.slots.len());
    let per_row = ((width - frame_x + DOCK_GAP_X) / (CHIP + DOCK_GAP_X))
        .floor()
        .max(1.0) as usize;
    chips.div_ceil(per_row)
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
    if outline.is_none() {
        paint_letter(ui, rect, letter, true, f32::INFINITY);
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
    // an empty chip's name is "<letter>, empty"; a missing entry's outlined "?" keeps its own
    let name = if outline.is_none() {
        format!("{letter}, empty")
    } else {
        letter.to_string()
    };
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
    r
}

/// Note HP: the dimmed letter's colour on an empty chip (read at 4.5:1 on `extreme_bg_color` in both themes).
pub fn dim_letter_colour(vis: &egui::Visuals) -> egui::Color32 {
    vis.widgets.inactive.text_color()
}

/// Note HP: a tile's label in its bottom-left corner. `dim` (an empty chip, no thumbnail under it) is weak text with
/// no backing; otherwise white on a dark backing so it reads over any thumbnail. Text wider than the chip is cut with `…`.
/// `max_w` cuts the text with `…` (trailing-row tags); chip letters pass `f32::INFINITY` and are never cut.
fn paint_letter(ui: &egui::Ui, rect: egui::Rect, text: &str, dim: bool, max_w: f32) {
    let font = egui::TextStyle::Small.resolve(ui.style());
    let colour = if dim {
        dim_letter_colour(ui.visuals())
    } else {
        egui::Color32::WHITE
    };
    let lay = |s: String| ui.painter().layout_no_wrap(s, font.clone(), colour);
    let mut g = lay(text.to_string());
    let mut shown = text.to_string();
    while g.size().x > max_w && shown.pop().is_some() {
        g = lay(format!("{}…", shown.trim_end()));
    }
    let at = rect.left_bottom() + egui::vec2(1.0, -1.0 - g.size().y);
    if !dim {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(at, g.size()).expand(1.0),
            2.0,
            egui::Color32::from_black_alpha(170),
        );
    }
    ui.painter().galley(at, g, colour);
}

/// Note HP (Decision 7): the trailing-row tile's tag. A member whose slot is taken: its slot letter; an unused helper:
/// "unused"; a helper whose fills land on no chip: its first fill; any other member: its `member_tail`, or "—".
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
    let fills = item.helper.as_ref().map(|h| h.fills.as_slice());
    tag_text(fills, member_tail(&stem, v.skin || v.shared))
}

/// `tile_tag` for a member with no slot: a helper's first fill, or "unused" with none; any other member's tail, or "—".
fn tag_text(helper_fills: Option<&[engine::helper::FillTarget]>, tail: &str) -> String {
    match (helper_fills, tail) {
        (Some(f), _) => f.first().map_or("unused".to_string(), |t| fill_text(*t)),
        (None, "") => "—".to_string(),
        (None, t) => t.to_string(),
    }
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

/// The longest form of a set id that `fits`: the id, its middle elided, `…/<leaf>`, then the end of the leaf.
/// Note HP (SC4): when the search lies inside one middle segment, `…/<that segment>/<rest>` is tried right after the id.
fn fit_title(id: &str, query: &str, fits: impl Fn(&str) -> bool) -> String {
    use super::header::elide_middle;
    let len = id.chars().count();
    if fits(id) {
        return id.to_string();
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
            if fits(&t) {
                return t;
            }
        }
    }
    for n in (1..len).rev() {
        let t = elide_middle(id, n);
        if t.contains('/') && fits(&t) {
            return t;
        }
    }
    let leaf = id.rsplit('/').next().unwrap_or(id);
    let leaf_len = leaf.chars().count();
    if leaf != id {
        let t = format!("…/{leaf}");
        if fits(&t) {
            return t;
        }
    }
    for keep in (1..=leaf_len).rev() {
        let t = format!(
            "…{}",
            leaf.chars().skip(leaf_len - keep).collect::<String>()
        );
        if fits(&t) {
            return t;
        }
    }
    "…".to_string()
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

/// One chip's dock, planned: its rect, the index of that rect among the card's fixed rects, the helpers that fill it
/// and the cells to draw (a badge whose item is gone has no cell).
struct DockPlan<'a> {
    chip: egui::Rect,
    own: usize,
    entries: Vec<&'a Docked>,
    cells: Vec<(usize, Cell)>,
}

/// Note HN: the cells of the helpers that fill the chip `chip` (index `own` among the fixed rects). A helper with a
/// badge is added to `badged`, so the trailing row skips it.
fn plan_dock<'a>(
    model: &AppModel,
    chip: egui::Rect,
    own: usize,
    entries: Vec<&'a Docked>,
    locks: (bool, bool),
    badged: &mut HashSet<&'a str>,
) -> DockPlan<'a> {
    let mut cells = dock_cells(locks.0, locks.1, entries.len());
    cells.retain(|(_, c)| match c {
        Cell::Badge(i) => model.item(&entries[*i].helper).is_some(),
        Cell::More(_) => true,
    });
    for (_, c) in &cells {
        if let Cell::Badge(i) = c {
            badged.insert(entries[*i].helper.as_str());
        }
    }
    DockPlan {
        chip,
        own,
        entries,
        cells,
    }
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
/// member behaviours (click selects, double-click expands, drag, the member menu); `+K` is a hover-only count.
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
    let entries = &plan.entries;
    for ((pos, cell), &hit) in plan.cells.iter().zip(hits) {
        let (rect, _) = badge_rects(plan.chip, *pos);
        let pos = *pos;
        match cell {
            Cell::Badge(i) => {
                let d = entries[*i];
                // `plan_dock` kept only cells whose item exists, so this is never None; it stays because the
                // item is needed and the model is borrowed immutably for the whole frame.
                let Some(item) = model.item(&d.helper) else {
                    continue;
                };
                let id = ui.id().with(("dock", &v.id, &d.helper, d.slot, d.channel));
                let r = ui.interact(hit, id, egui::Sense::click_and_drag());
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
                        egui::Stroke::new(2.0, ui.visuals().selection.stroke.color)
                    } else {
                        egui::Stroke::new(1.0, ui.visuals().panel_fill)
                    };
                    ui.painter()
                        .rect_stroke(rect, 1.0, stroke, egui::StrokeKind::Inside);
                }
                let name = format!("{}, fills {}", item.name, fill_text(d.target));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
                member_acts(model, v, &d.helper, None, true, &r, acts, member_menu);
                let target = item.helper.as_ref().map(|h| helper_target_files(h));
                r.on_hover_text(match target {
                    Some(t) => format!("{}\n{t}", item.name),
                    None => item.name.clone(),
                });
            }
            Cell::More(k) => {
                let k = *k;
                let hidden = &entries[entries.len() - k..];
                let slot = chip_letter(v, hidden[0].slot);
                let id = ui.id().with(("dock-more", &v.id, hidden[0].slot, pos));
                let r = ui.interact(hit, id, egui::Sense::hover());
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
                let name = format!("{k} more helpers fill {slot}");
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &name));
                let lines: Vec<String> = hidden
                    .iter()
                    .map(|d| {
                        let file = model.item(&d.helper).map_or(d.helper.as_str(), |i| &i.name);
                        format!("{file}, fills {}", fill_text(d.target))
                    })
                    .collect();
                r.on_hover_text(lines.join("\n"));
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
}

/// Why the ≡ handle and Move up / Move down are off while the Sets filter is on.
const CLEAR_TO_REORDER: &str = "Clear the filter to reorder sets, or use Move to…";

/// R17: the search box, the pills with their counts and the shader popup, under the heading.
fn filter_rows(ui: &mut egui::Ui, model: &mut AppModel) {
    let search = ui.add(
        egui::TextEdit::singleline(&mut model.sets_filter.search)
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
        for (on, text, n, hover) in [
            (
                &mut f.attention,
                format!("Not ready {}", c.attention),
                c.attention,
                "Only sets that are not ready: a problem in the set or one of its textures",
            ),
            (
                &mut f.ready,
                format!("Ready {}", c.ready),
                c.ready,
                "Only sets with no problem",
            ),
            (
                &mut f.no_json,
                format!("No JSON {}", c.no_json),
                c.no_json,
                "Only sets that write no JSON",
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
                .on_hover_text(hover)
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
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
        r.on_hover_text("Only sets with this shader");
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
    let mut gained = ctx.memory_mut(|m| {
        m.interested_in_focus(id, layer);
        m.focused() == Some(id) && held != Some(id)
    });
    // egui consumes ScrollIntoView only in a registered widget; a stand-in has none, so read the request here.
    gained |=
        ctx.input(|i| i.has_accesskit_action_request(id, egui::accesskit::Action::ScrollIntoView));
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
    let frame_x = egui::Frame::group(ui.style()).total_margin().sum().x + 2.0 * DOCK_INSET;
    let reflowed = reflow_heights(st, &views, ui.available_width(), frame_x);
    // A new width moves the open card away from the scroll offset: bring it back into view (Note HN).
    if reflowed && model.nav.reveal_set.is_none() {
        if let [k] = model.selected_sets().as_slice() {
            model.nav.reveal_set = Some(k.clone());
        }
    }
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
    if changed {
        match model.selected_sets().as_slice() {
            [k] if visible.iter().any(|&i| views[i].key == *k) => {
                model.nav.reveal_set = Some(k.clone());
            }
            _ => to_top = model.nav.reveal_set.is_none(),
        }
        st.reveal_tries = 0;
    }
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
    // Note HN F6: weak text, the selection accent while last frame's zone had a dragged texture over it.
    let hot_id = ui.id().with("new_set_hint_hot");
    let hot = ui.data(|d| d.get_temp::<bool>(hot_id)).unwrap_or(false);
    let colour = if hot {
        ui.visuals().selection.stroke.color
    } else {
        ui.visuals().weak_text_color()
    };
    let (zone, payload) = item_zone(
        ui,
        editing.on,
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
        && zone.contains_pointer()
        && egui::DragAndDrop::has_payload_of_type::<DragItem>(ui.ctx());
    ui.data_mut(|d| d.insert_temp(hot_id, over));
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
    let card_margin = card.total_margin();
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
                    // Note HP: the card's left, right and top, for the badge hit rects to stay inside. The bottom is
                    // not known until the card is laid out, and no badge reaches it, so it is left open.
                    let card_rect = {
                        let (r, m) = (ui.max_rect(), card_margin);
                        egui::Rect::from_min_max(
                            r.min - egui::vec2(m.left, m.top),
                            egui::pos2(r.max.x + m.right, f32::MAX),
                        )
                    };
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
                    // One row only when the name keeps NAME_MIN px (or its whole width) beside the pill and badges (Note FI13).
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
                    // Note HP12: a whole title shorter than NAME_MIN needs only its own width.
                    let name_min = NAME_MIN.min(text_width(ui, &v.id, &body));
                    let mut need = lead + name_min + pill + sp;
                    if !rows.is_empty() {
                        need += small_width(ui, &warn_text(rows.len())) + btn;
                    }
                    if !checks.is_empty() {
                        need += small_width(ui, &check_badge_text(&checks).1) + btn;
                    }
                    let wide = ui.available_width() >= need;
                    // Note HN F3: elide the middle so the leaf stays; the node's name is the full id.
                    let shown_for = |ui: &egui::Ui| {
                        let font = egui::TextStyle::Body.resolve(ui.style());
                        let room = ui.available_width();
                        fit_title(&v.id, &model.sets_filter.search, |t| {
                            text_width(ui, t, &font) <= room
                        })
                    };
                    let name = |ui: &mut egui::Ui| {
                        let shown = shown_for(ui);
                        // Extend: the shown text already fits, so egui never end-cuts the leaf.
                        let r = ui.add(
                            egui::Label::new(egui::RichText::new(&shown).strong())
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                        r.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &v.id)
                        });
                        if shown != v.id {
                            r.on_hover_text(&v.id);
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
                                    check_badge(ui, v, &checks, acts);
                                    warn_button(ui, v, &rows, acts);
                                    status_pill(ui, model, v, status);
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        name,
                                    );
                                },
                            );
                            false
                        } else {
                            // Beside the position and handle the leaf would be cut: give the title its own line.
                            let shown = shown_for(ui);
                            let own = shown != v.id
                                && !shown.ends_with(v.id.rsplit('/').next().unwrap_or(&v.id));
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
                            status_pill(ui, model, v, status);
                            warn_button(ui, v, &rows, acts);
                            check_badge(ui, v, &checks, acts);
                        });
                    }
                    if let Some(map) = &v.txst {
                        record_line(ui, v, map, model.record_ignored(v));
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

/// Note HP: the chip row of a card: one chip per slot (an empty one is a letter box), the missing entries, then the
/// members that fill no chip. Returns each filled chip's dock plan and every rect a badge's hit must keep clear of
/// (`fixed` comes in holding the combo's).
#[allow(clippy::too_many_arguments)]
fn chip_row<'a>(
    ui: &mut egui::Ui,
    model: &AppModel,
    thumbs: &mut ThumbCache,
    v: &SetView,
    dock: &'a [Docked],
    editing: &Editing,
    sel_items: &HashSet<String>,
    acts: &mut Vec<Act>,
    member_menu: &mut bool,
    mut fixed: Vec<egui::Rect>,
) -> (Vec<DockPlan<'a>>, Vec<egui::Rect>) {
    let row = shared_cells(v, slot_row(v.shader.as_ref(), &v.slots));
    let mut chipped: HashSet<&str> = HashSet::new();
    let mut badged: HashSet<&str> = HashSet::new();
    let mut plans: Vec<DockPlan> = vec![];
    for cell in &row {
        let k = cell.kind;
        let letter = chip_letter(v, k);
        let (mk, from) = match &cell.fill {
            SlotFill::Own { item } => (item, None),
            SlotFill::Borrowed { from, item } => (item, Some(from)),
            SlotFill::Empty => {
                let r = letter_box(ui, &letter, None)
                    .on_hover_text(format!("{} (empty)", v.slot_text(k)));
                fixed.push(r.rect);
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
            member_menu,
        ) else {
            fixed.push(letter_box(ui, &letter, None).rect);
            continue;
        };
        let own = fixed.len();
        fixed.push(r.rect);
        paint_letter(ui, r.rect, &letter, false, f32::INFINITY);
        if from.is_some() {
            ui.painter().rect_stroke(
                r.rect,
                2.0,
                egui::Stroke::new(2.0, AMBER),
                egui::StrokeKind::Inside,
            );
        }
        let entries: Vec<&Docked> = dock.iter().filter(|d| d.slot == k).collect();
        let name = chip_name(model, &letter, mk, &entries);
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
        plans.push(plan_dock(
            model,
            r.rect,
            own,
            entries,
            chip_locks(v, Some(k)),
            &mut badged,
        ));
    }
    // K12: an add/remove entry that matches no file.
    // Note DU1: its menu removes the entry (an entry with no files left: Delete set).
    for m in &v.missing {
        let r = letter_box(ui, "?", Some(RED)).interact(egui::Sense::click());
        fixed.push(r.rect);
        let text = m.text();
        r.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Missing: {text}"))
        });
        let r = r.on_hover_text(format!("no file matches {text}"));
        *member_menu |= r.secondary_clicked();
        let open = r.context_menu(|ui| {
            let (label, act) = match m {
                Missing::NoFilesLeft => ("Delete set", Act::Delete(v.id.clone(), v.members.len())),
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
    // Members in no chip (an env mask `_m`, a clashing second kind) follow as plain thumbnails.
    for mk in &v.members {
        if chipped.contains(mk.as_str()) || badged.contains(mk.as_str()) {
            continue;
        }
        // F3a Task 8: the slot the member fills in this set, if any, has a slot menu.
        let owns = |f: &SlotFill| matches!(f, SlotFill::Own { item } if item == mk);
        let kind = v.slots.iter().find(|(_, f)| owns(f)).map(|(k, _)| *k);
        let Some(item) = model.item(mk) else {
            continue;
        };
        // Named by its file and why it is here (REPORT M1); an unused helper is dimmed.
        let role = match (kind, &item.helper, &item.job) {
            (Some(k), ..) => {
                format!("{} (its slot is taken)", v.slot_text(k))
            }
            // the tag paints "unused", so the name and hover say it too (Note HP12)
            (None, Some(h), _) => helper_target_text(h).replacen("not used", "unused", 1),
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
            member_menu,
        );
        // the tag is painted at the tile's opacity, so an unused helper's fades with it
        if let Some(r) = &r {
            paint_letter(ui, r.rect, &tile_tag(model, v, mk, kind), false, CHIP - 2.0);
        }
        ui.set_opacity(was);
        if let Some(r) = r {
            fixed.push(r.rect);
            let name = format!("{}, {role}", item.name);
            r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
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
    // Note HP: its own row under the title, right-aligned and as wide as its value (capped at the card's inner width less COMBO_FREE). The
    // row's free left part allocates nothing, so a click there reaches the card.
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
                    egui::WidgetType::ComboBox,
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

    fn fit_at(id: &str, query: &str, n: usize) -> String {
        fit_title(id, query, |t: &str| t.chars().count() <= n)
    }

    #[test]
    fn fit_title_keeps_a_middle_match() {
        assert_eq!(
            fit_at("architecture/stone/banner", "STONE", 15),
            "…/stone/banner"
        );
        assert_eq!(fit_at("a/bb/stone/leaf", "ston", 13), "…/stone/leaf");
    }

    #[test]
    fn fit_title_without_a_middle_match_keeps_hn6() {
        let id = "architecture/stone/banner";
        let hn6 = fit_at(id, "", 15);
        for q in ["", "banner", "arch", "stone/ban"] {
            assert_eq!(fit_at(id, q, 15), hn6, "query {q:?}");
        }
    }

    #[test]
    fn fit_title_with_a_match_that_does_not_fit_falls_through_to_hn6() {
        // "…/stone/banner" is 14 characters: at 12 the middle match is skipped
        let id = "architecture/stone/banner";
        assert_eq!(fit_at(id, "stone", 12), fit_at(id, "", 12));
    }

    #[test]
    fn a_tile_tag_is_the_slot_letter_or_the_first_fill_or_the_tail_or_a_dash() {
        use engine::helper::{AlphaTarget as A, FillTarget as F};
        let mask = F::Packed {
            suffix: "m",
            channel: 1,
        };
        // a helper: its first fill, "unused" with none (the tail is not read)
        assert_eq!(tag_text(Some(&[mask, F::Alpha(A::N)]), "_m"), "M G");
        assert_eq!(tag_text(Some(&[]), "_m"), "unused");
        // any other member: its tail, or an em dash
        assert_eq!(tag_text(None, "_m"), "_m");
        assert_eq!(tag_text(None, ""), "—");
    }

    #[test]
    fn fit_title_shows_the_whole_id_when_it_fits() {
        let id = "architecture/stone/banner";
        assert_eq!(fit_at(id, "stone", 40), id);
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
