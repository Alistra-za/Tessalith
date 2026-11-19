//! The whole-app walk: every entry of `screens::entries()` at three window sizes, one frame run of `run_checks` per
//! entry and size, and every invariant of `FRAME_INVARIANTS` run on it. `walk` and `verdict` are the only code that
//! iterates entries, collects hits and judges them, for the shipped list and for the proof's test-only entry alike.
//!
//! To add an invariant that reads one frame: write a `fn(&Frame) -> Vec<Hit>` and add it to `FRAME_INVARIANTS` (add
//! to `Frame` what it reads). The per-screen key step (`press_keys`) runs in `walk`, once after an entry's sizes. A hit the tree has today
//! goes in `ALLOWED` with a reason; a hit no entry covers fails, and so does an entry no hit consumed.

use crate::checks::{run_checks, Finding};
use crate::frame::{snapshot, Node, Snapshot, Text};
use crate::screens::{entries, holds, no_stray_dialog, Needle, Screen, ScreenEntry};
use crate::support::bar_clearance;
use eframe::egui::{vec2, Key, Rect, Vec2};
use egui_kittest::Harness;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The window sizes every screen is visited at: a narrow laptop, the common case, a large monitor.
pub const SIZES: [Vec2; 3] = [
    vec2(1100.0, 800.0),
    vec2(1280.0, 800.0),
    vec2(1600.0, 900.0),
];

/// What the walk visits: anything that can show a screen at a size. A shipped entry builds the app on a fixture; a
/// test-only entry builds a small UI.
pub trait Entry {
    fn name(&self) -> &str;
    /// What only this screen shows; no other entry's screen may satisfy it.
    fn marker(&self) -> &'static [Needle];
    /// The frame of the screen at `size`, or why it cannot be had (the walk names the screen and size).
    fn at(&mut self, size: Vec2) -> Result<Snapshot, String>;

    /// True when the screen is a modal dialog; on any other entry an open modal dialog is an error of the visit and of
    /// the key step (`screens::no_stray_dialog`).
    fn dialog(&self) -> bool {
        false
    }
    /// True when the key step may press keys on this entry; a test-only entry that is only a layout fixture says no.
    fn takes_keys(&self) -> bool {
        false
    }
    /// The folders the key step hashes, each with the name a failure message gives it. Read again after `reopen`.
    fn watched(&self) -> Vec<(&'static str, PathBuf)> {
        vec![]
    }
    /// Clears the keyboard focus and says whether the entry's marker still holds on the frame after: the check right
    /// before a key. `Ok(false)` means the screen under test is gone (the walk reopens it and asks again).
    fn arm(&mut self) -> Result<bool, String> {
        Err("this entry takes no keys".into())
    }
    /// Presses `key` on the armed screen (see `strike`) and says what it found.
    fn press(&mut self, _key: Key) -> Result<Pressed, String> {
        Err("this entry takes no keys".into())
    }
    /// Shows the screen again from scratch at `size`, after a key closed it.
    fn reopen(&mut self, _size: Vec2) -> Result<(), String> {
        Err("this entry cannot be reopened".into())
    }
    /// Drops what the entry holds open (a window and its folders) once the walk is done with it.
    fn release(&mut self) {}
}

/// `snap` when it was taken at `size`, else an error: a frame of another size is not a visit at this one.
fn at_the_size(snap: Snapshot, size: Vec2) -> Result<Snapshot, String> {
    if (snap.size - size).abs().max_elem() > 0.5 {
        return Err(format!(
            "the frame is {}x{}, not the {}x{} asked for, so it is no visit at this size",
            snap.size.x, snap.size.y, size.x, size.y
        ));
    }
    Ok(snap)
}

/// What one key did to a screen, besides the files (those are hashed).
pub struct Pressed {
    /// The export gate's flag, read on the one frame that processed the key going down.
    pub running_on_key_down: bool,
    /// The same flag after the key came up and the app settled.
    pub running_after_settle: bool,
    /// The export's record (rows, events, summary, error, notes) differs from before the key: a second witness that
    /// does not depend on when the flag is read.
    pub witness_changed: bool,
    /// The entry's marker still holds: false when the key closed the screen.
    pub shown: bool,
}

/// Before a key: takes the keyboard focus from whatever holds it (`clear_focus`) and says whether `marker` holds on
/// the frame after, which is the frame the key lands in. A modal dialog open on that frame of a screen that is no
/// dialog (`dialog` false) is an `Err` naming it: the key would land in the dialog.
pub fn arm<S>(
    h: &mut Harness<'static, S>,
    marker: &[Needle],
    dialog: bool,
) -> Result<bool, String> {
    clear_focus(h)?;
    let snap = snapshot(h);
    no_stray_dialog(&snap, dialog)?;
    Ok(holds(&snap, marker))
}

/// One key on an armed screen. Needs an idle baseline (no export running; else an error, not a suppressed finding).
/// Sends the key going down alone, runs the one frame that processes it and reads the running flag at once: an export
/// that starts on this frame is seen even if the app clears it a frame later. Then the key comes up, `settle` runs (a
/// timeout is an error and nothing is taken from it) and the flag is read again; `witness` is read before the key and
/// after settling.
pub fn strike<S>(
    h: &mut Harness<'static, S>,
    key: Key,
    running: &dyn Fn(&S) -> bool,
    witness: &dyn Fn(&S) -> String,
    settle: &mut dyn FnMut(&mut Harness<'static, S>) -> bool,
    marker: &[Needle],
) -> Result<Pressed, String> {
    if running(h.state()) {
        return Err("an export is already running before the key, so the key's effect cannot be told from it".into());
    }
    let witness_before = witness(h.state());
    h.key_down(key);
    h.run_steps(1);
    let running_on_key_down = running(h.state());
    h.key_up(key);
    h.run_steps(1);
    if !settle(h) {
        return Err("the app did not go idle after the key (the settle timed out), so nothing is taken from this key".into());
    }
    Ok(Pressed {
        running_on_key_down,
        running_after_settle: running(h.state()),
        witness_changed: witness(h.state()) != witness_before,
        shown: holds(&snapshot(h), marker),
    })
}

/// Takes the keyboard focus from whatever holds it and proves that nothing holds it on the frame after, which is the
/// frame the next key lands in. `Err` when a widget takes the focus back at once (nothing can then be "unfocused").
pub fn clear_focus<S>(h: &mut Harness<'static, S>) -> Result<(), String> {
    let ctx = h.ctx.clone();
    for _ in 0..3 {
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        h.run_steps(1);
        if ctx.memory(|m| m.focused()).is_none() {
            return Ok(());
        }
    }
    Err("a widget takes the keyboard focus back on every frame, so no key can be pressed with nothing focused".into())
}

/// One frame of one screen at one size.
pub struct Frame<'a> {
    pub screen: &'a str,
    pub size: Vec2,
    /// The frame as data, for an invariant that reads nodes and painted texts itself.
    pub snapshot: &'a Snapshot,
    /// `checks::run_checks` of the frame, run once for every invariant to filter.
    pub findings: &'a [Finding],
}

/// One broken rule: where (screen, size), which invariant, what (the widget or text), how far (px), and the message.
#[derive(Debug, Clone)]
pub struct Hit {
    pub screen: String,
    pub size: Vec2,
    pub invariant: &'static str,
    pub what: String,
    /// The widget's top edge, in px: what orders same-named hits.
    pub y: f32,
    /// Among the hits of this screen, size, invariant and name, the widget's place from the top (0 is the upper).
    pub ordinal: usize,
    /// How far the widget is past the edge, or how much of the text is clipped, in px.
    pub px: f32,
    /// The widgets a hit on an area is made of, each with its own overshoot in px, sorted by name then px: what a
    /// per-bar entry in `ALLOWED` pins. Empty for a hit that is one widget or one text.
    pub members: Vec<(String, f32)>,
    pub message: String,
}

impl Hit {
    fn text(&self) -> String {
        format!(
            "{} at {}x{}: {} {:?} #{} ({:.1} px): {}",
            self.screen,
            self.size.x,
            self.size.y,
            self.invariant,
            self.what,
            self.ordinal,
            self.px,
            self.message
        )
    }
}

type FrameInvariant = fn(&Frame) -> Vec<Hit>;

/// Invariants that read the one frame of a screen at a size.
pub const FRAME_INVARIANTS: &[FrameInvariant] = &[
    nothing_past_the_window_edge,
    no_clipped_text,
    cut_text_keeps_its_value,
];

/// An invariant that can also fail to observe: it pushes to `errors` what the snapshot cannot tell (the visit is then
/// an error of its screen, never a pass).
type ObservingInvariant = fn(&Frame, &mut Vec<String>) -> Vec<Hit>;

pub const OBSERVING_INVARIANTS: &[ObservingInvariant] =
    &[scroll_bars_are_visible_and_clear, status_dots_have_names];

/// What a scroll bar's own paint says: its track and whether the bar is visible. A candidate is a filled rect of the bar
/// node's own layer, visible inside its clip, lying inside the node's strip (a handle can be as short as egui allows,
/// 12 px). The track is the one candidate that spans the strip's length; a handle is a candidate nested in the track, at
/// most one. The bar is visible when the track or the handle has alpha above 0. `Err` is any other count or arrangement:
/// what the frame cannot tell is never a pass.
// shortcut (ST-9): a decoy in the bar's own layer, inside the bar's own strip, nested in the track like a handle, is
// taken for the handle; upgrade if a bar ever gets painted by two widgets in one strip.
fn bar_paint(f: &Frame, bar: &Node, layer: &str) -> Result<(Rect, bool), String> {
    let strip = bar.rect.expand(0.5);
    let cands: Vec<(Rect, u8)> = f
        .snapshot
        .filled
        .iter()
        .filter(|x| x.layer.as_deref() == Some(layer))
        .filter_map(|x| {
            let seen = x.rect.intersect(x.clip);
            (seen.is_positive() && strip.contains_rect(seen)).then_some((seen, x.alpha))
        })
        .collect();
    let spans = |r: &Rect| {
        (r.min.y - bar.rect.min.y).abs() <= 0.5 && (r.max.y - bar.rect.max.y).abs() <= 0.5
    };
    // the track is the tallest candidate that spans the strip; a handle as long as the track (content that barely
    // overflows) spans it too and is taken for the handle. Equal heights (within 0.5 px) go to the candidate that holds
    // the other, then to the wider one, whatever order they were painted in.
    let rank = |a: &Rect, b: &Rect| {
        let height = if (a.height() - b.height()).abs() <= 0.5 {
            std::cmp::Ordering::Equal
        } else {
            a.height().total_cmp(&b.height())
        };
        height
            .then(
                a.expand(0.5)
                    .contains_rect(*b)
                    .cmp(&b.expand(0.5).contains_rect(*a)),
            )
            .then(a.width().total_cmp(&b.width()))
    };
    let Some(at) = (0..cands.len())
        .filter(|i| spans(&cands[*i].0))
        .max_by(|a, b| rank(&cands[*a].0, &cands[*b].0))
    else {
        return Err(format!(
            "the scroll bar node at {:?} has no filled rect of its own layer spanning its strip in view ({:?}), so which paint is the bar's cannot be told",
            bar.rect, cands
        ));
    };
    let (track, track_alpha) = cands[at];
    let others: Vec<&(Rect, u8)> = cands
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != at)
        .map(|(_, c)| c)
        .collect();
    let arrangement = |why: &str| {
        Err(format!(
            "the scroll bar node at {:?} has {why} in its strip ({:?}), so which paint is the bar's cannot be told",
            bar.rect, cands
        ))
    };
    let nested = |r: &Rect| {
        r.min.x >= track.min.x - 0.5
            && r.max.x <= track.max.x + 0.5
            && r.min.y >= track.min.y - 0.5
            && r.max.y <= track.max.y + 0.5
    };
    if others.len() > 1 || others.iter().any(|&&(r, _)| !nested(&r)) {
        return arrangement("paint that is not one track with at most one nested handle");
    }
    let visible = track_alpha > 0 || others.iter().any(|&&(_, a)| a > 0);
    Ok((track, visible))
}

/// Where each scroll bar node belongs, as a name that does not move with the layout: "scroll bar K of N in the P panel",
/// P the smallest named panel (`Snapshot::panels`, the centre column included) that holds the bar's node, K its place
/// among all N bars of that panel on the screen (top to bottom, then left to right). A bar no named panel holds is an
/// `Err`, never a weaker name: add the panel to `PANEL_NAMES` in frame.rs. (node index, name).
pub fn bar_places(snap: &Snapshot) -> Vec<(usize, Result<String, String>)> {
    let mut owned: Vec<(usize, Option<&str>)> = vec![];
    for (i, n) in snap.nodes.iter().enumerate() {
        if n.role != "ScrollBar" || !n.rect.is_positive() {
            continue;
        }
        let owner = snap
            .panels
            .iter()
            .filter(|(_, r)| r.expand(0.5).contains_rect(n.rect))
            .min_by(|a, b| a.1.area().total_cmp(&b.1.area()))
            .map(|(name, _)| name.as_str());
        owned.push((i, owner));
    }
    owned
        .iter()
        .map(|&(i, owner)| {
            let rect = snap.nodes[i].rect;
            let Some(owner) = owner else {
                return (
                    i,
                    Err(format!(
                        "no named panel holds the scroll bar at {rect:?}, so it has no stable name; add its panel to PANEL_NAMES in crates/gui/tests/ui_drive/frame.rs"
                    )),
                );
            };
            let mut same: Vec<Rect> = owned
                .iter()
                .filter(|(_, o)| *o == Some(owner))
                .map(|&(j, _)| snap.nodes[j].rect)
                .collect();
            same.sort_by(|a, b| a.min.y.total_cmp(&b.min.y).then(a.min.x.total_cmp(&b.min.x)));
            // two bars at one rect (on one layer or two) cannot be told apart by their place
            let near = |a: &Rect, b: &Rect| {
                (a.min - b.min).abs().max_elem() <= 0.5 && (a.max - b.max).abs().max_elem() <= 0.5
            };
            let together = same.iter().filter(|r| near(r, &rect)).count();
            if together > 1 {
                return (
                    i,
                    Err(format!(
                        "{together} scroll bars of the {owner} panel lie at one rect {rect:?}, so their places cannot be told apart and no allowance can name one of them"
                    )),
                );
            }
            let k = same.iter().position(|r| *r == rect).unwrap_or(0) + 1;
            (i, Ok(format!("scroll bar {k} of {} in the {owner} panel", same.len())))
        })
        .collect()
}

/// A set card as `sets_column::card_node` builds it (Note FM32), read from the node: a Button named "Set <id>, position
/// <i> of <n>", with a selected state and the ScrollIntoView action, inside the Sets column's panel width (a card
/// scrolled partly out of view may run past the panel's top or bottom). A button that only starts with "Set " is not one.
fn is_set_card(snap: &Snapshot, n: &Node) -> bool {
    let named = n
        .label
        .strip_prefix("Set ")
        .and_then(|rest| rest.rsplit_once(", position "))
        .and_then(|(_, place)| place.split_once(" of "))
        .is_some_and(|(i, total)| i.parse::<usize>().is_ok() && total.parse::<usize>().is_ok());
    n.role == "Button"
        && named
        && n.selected.is_some()
        && n.scroll_into_view
        && snap.panel("sets").is_some_and(|p| {
            n.rect.min.x >= p.min.x - 0.5
                && n.rect.max.x <= p.max.x + 0.5
                && n.rect.max.y > p.min.y
                && n.rect.min.y < p.max.y
        })
}

/// Every shown scroll bar is visible with no pointer in the window, and no interactive widget's visible rect comes
/// within the bar's width of its track (the room the bar takes when hovered), except a set card, which only has to stop
/// short of the track itself (`support::bar_clearance`).
// shortcut: vertical bars only (every bar of the shipped screens is one: a horizontal one is an error until this reads
// it), and the widgets beside the bar are the nodes the snapshot lists as interactive; upgrade with the first
// horizontal bar the app gets.
fn scroll_bars_are_visible_and_clear(f: &Frame, errors: &mut Vec<String>) -> Vec<Hit> {
    let snap = f.snapshot;
    let bars: Vec<&Node> = snap
        .nodes
        .iter()
        .filter(|n| n.role == "ScrollBar")
        .collect();
    if bars.is_empty() {
        return vec![];
    }
    if let Some(p) = snap.pointer {
        errors.push(format!(
            "the pointer is at {p:?} in the window, so a scroll bar's opacity is not the one at rest; the walk must send the pointer away before it reads the frame"
        ));
        return vec![];
    }
    if !snap.paint_layers_known {
        errors.push("the frame's paint has no layers (frame::track_paint was not installed, or a layer's shapes were missed), so a scroll bar's own paint cannot be told from another layer's".into());
        return vec![];
    }
    let bar_width = sdc_gui::look::thin_scroll().bar_width;
    let mut hits = vec![];
    let places = bar_places(snap);
    for bar in bars {
        let r = bar.rect;
        if !r.is_positive() {
            errors.push("a scroll bar node has no rect, so its track cannot be found".into());
            continue;
        }
        if r.width() > r.height() {
            errors.push(format!(
                "a horizontal scroll bar at {r:?}: the scroll bar check reads vertical bars only"
            ));
            continue;
        }
        let Some(layer) = bar.layer.as_deref() else {
            errors.push(format!(
                "the scroll bar node at {r:?} has no egui layer, so its own paint and the widgets beside it cannot be told from another layer's"
            ));
            continue;
        };
        if snap.transformed_layers.iter().any(|l| l == layer) {
            errors.push(format!(
                "the scroll bar node at {r:?} is in the layer {layer}, which egui moves by a transform when it drains the pass, so its paint and its neighbours are not in window coordinates"
            ));
            continue;
        }
        let idx = snap
            .nodes
            .iter()
            .position(|n| std::ptr::eq(n, bar))
            .unwrap_or(0);
        let what = match places.iter().find(|(i, _)| *i == idx) {
            Some((_, Ok(what))) => what.clone(),
            Some((_, Err(why))) => {
                errors.push(why.clone());
                continue;
            }
            None => {
                errors.push(format!("the scroll bar at {r:?} has no place"));
                continue;
            }
        };
        let owner = what.clone();
        let (track, visible) = match bar_paint(f, bar, layer) {
            Ok(found) => found,
            Err(why) => {
                errors.push(format!("{owner}: {why}"));
                continue;
            }
        };
        if !visible {
            hits.push(Hit {
                screen: f.screen.to_string(),
                size: f.size,
                invariant: "scroll_bar_at_rest",
                what: what.clone(),
                y: r.min.y,
                ordinal: 0,
                px: 0.0,
                members: vec![],
                message: format!(
                    "the scroll bar at {r:?} ({owner}) paints only fully transparent fills with no pointer in the window; scope look::thin_scroll() (or solid) on its ScrollArea so the bar shows at rest"
                ),
            });
        }
        // One hit per bar, not per widget: one fault in a panel's right-edge reserve puts every widget of that panel
        // under the bar, and a hit each would fill ALLOWED's 30 entries. The hit carries each widget with its own
        // overshoot (`members`), which the entry in ALLOWED pins.
        let mut under: Vec<(String, f32)> = vec![];
        for n in &snap.nodes {
            // what is judged is what is shown: the control's rect cut by its clip where the snapshot knows it (the
            // visible interaction rect), so a control a clip ends before the reserve is clear of it
            let seen = n.clip.map_or(n.rect, |c| n.rect.intersect(c));
            if !n.interactive
                || n.role == "ScrollBar"
                || !seen.is_positive()
                || seen.max.y <= track.min.y
                || seen.min.y >= track.max.y
                || seen.min.x >= track.max.x
            {
                continue;
            }
            let label = if n.label.is_empty() {
                n.value.clone().unwrap_or_default()
            } else {
                n.label.clone()
            };
            let limit = bar_clearance(is_set_card(snap, n), track, bar_width);
            if seen.max.x <= limit + 0.5 {
                continue;
            }
            // A Splitter is a panel's resize handle, egui's own widget on the panel edge: it lies across the foot of the
            // area and is no control of the app (ruling ST-7).
            if n.role == "Splitter" {
                continue;
            }
            match n.layer.as_deref() {
                // a menu, a popup or a dialog floats over the area on purpose
                Some(l) if l != layer => continue,
                Some(_) => {}
                None => {
                    errors.push(format!(
                        "{owner}: the interactive {} {label:?} at {seen:?} overlaps the scroll bar {track:?} and has no egui layer, so whether it floats over the area or sits in it cannot be told",
                        n.role
                    ));
                    continue;
                }
            }
            under.push((format!("{}:{label}", n.role), seen.max.x - limit));
        }
        if !under.is_empty() {
            under.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
            let px = under.iter().map(|(_, px)| *px).fold(0.0, f32::max);
            hits.push(Hit {
                screen: f.screen.to_string(),
                size: f.size,
                invariant: "scroll_bar_clearance",
                what,
                y: r.min.y,
                ordinal: 0,
                px,
                message: format!(
                    "the scroll bar {track:?} ({owner}) has {} interactive widgets within its width: {}; end each {bar_width} px before the track, with the right-edge reserve of look::thin_scroll(), so the bar never lies over a widget",
                    under.len(),
                    under
                        .iter()
                        .map(|(name, px)| format!("{name} ({px:.1} px)"))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                members: under,
            });
        }
    }
    hits
}

/// Whether the disc at `centre` with `radius` and the rect `q` share a positive area: `q` has a positive area and a
/// point nearer the centre than the radius. The one predicate for a dot's visibility in its clip and for a node shown
/// over it: a rect that only touches the disc's edge, or holds only a corner of its bounding square, shares none.
fn disc_meets(centre: eframe::egui::Pos2, radius: f32, q: Rect) -> bool {
    let dx = (q.min.x - centre.x).max(0.0).max(centre.x - q.max.x);
    let dy = (q.min.y - centre.y).max(0.0).max(centre.y - q.max.y);
    q.is_positive() && dx.hypot(dy) < radius
}

/// Every filled circle in a status colour (`ui::status::RED`, `AMBER`, `GREEN`) lies under a named node: the innermost
/// node of the circle's own layer whose rect holds the whole circle has a non-empty
/// accessible name. A text input's value is no name; a Label's text is (the snapshot folds it into `label`). The
/// innermost node is the deepest one in the tree, the smallest on a tie; its ancestors do not count, so a named window
/// or row cannot name an unnamed dot. A circle clipped away entirely is not judged. Nodes with no layer that hold the
/// circle are an error (they may be the innermost one).
// shortcut: a circle is judged by the nodes of the frame's AccessKit tree; a node the tree lacks (an unlabelled painted
// widget) counts as no node; upgrade with the first painted status circle that sits in a widget the tree does not list.
fn status_dots_have_names(f: &Frame, errors: &mut Vec<String>) -> Vec<Hit> {
    use sdc_gui::ui::status::{AMBER, GREEN, RED};
    let snap = f.snapshot;
    let mut hits = vec![];
    for d in &snap.discs {
        let [r, g, b, _] = d.fill.to_srgba_unmultiplied();
        let Some(colour) = [(RED, "red"), (AMBER, "amber"), (GREEN, "green")]
            .into_iter()
            .find(|(c, _)| (c.r(), c.g(), c.b()) == (r, g, b))
            .map(|(_, name)| name)
        else {
            continue;
        };
        // A node holds the WHOLE circle: a node's rect is its widget's full rect, not cut by a clip (frame.rs takes it from
        // the AccessKit bounds, which egui sets from `Response::rect`, response.rs `fill_accesskit_node_common`), so a
        // circle that a clip cuts part of is judged like any other. A circle whose disc its clip hides entirely shows
        // nothing and is not judged (the disc, not its bounding square: a clip on the square's corner shows nothing).
        let bounds = Rect::from_center_size(d.center, Vec2::splat(2.0 * d.radius));
        if !disc_meets(d.center, d.radius, d.clip) {
            continue;
        }
        let Some(layer) = d.layer.as_deref() else {
            errors.push(format!(
                "the {colour} status circle at {:?} has no egui layer (frame::track_paint was not installed, or a layer's shapes were missed), so the node over it cannot be told from another layer's",
                d.center
            ));
            continue;
        };
        if snap.transformed_layers.iter().any(|l| l == layer) {
            errors.push(format!(
                "the {colour} status circle at {:?} is in the layer {layer}, which egui moves by a transform when it drains the pass, so its position and the nodes' rects are not in the same coordinates",
                d.center
            ));
            continue;
        }
        // ... and it has to be shown there: the node's visible rect (its rect cut by its clip, and by the circle's clip) has
        // to overlap the visible disc with positive area. A row clipped to a sliver beside the circle, or to a corner of
        // its bounding box outside the disc, is not what the user sees under it.
        let over_disc = |q: Rect| disc_meets(d.center, d.radius, q.intersect(d.clip));
        let holds = |n: &&Node| n.rect.is_positive() && n.rect.expand(0.5).contains_rect(bounds);
        if let Some(n) = snap
            .nodes
            .iter()
            .filter(|n| n.layer.is_none())
            .filter(holds)
            .find(|n| n.clip.is_none_or(|c| over_disc(c.intersect(n.rect))))
        {
            errors.push(format!(
                "the {colour} status circle at {:?} lies under the {} {:?} at {:?}, which has no egui layer, so whether it is the circle's own node cannot be told",
                d.center, n.role, n.label, n.rect
            ));
            continue;
        }
        let deepest = |nodes: &[&Node]| -> Option<usize> {
            (0..nodes.len()).max_by(|&a, &b| {
                nodes[a]
                    .depth
                    .cmp(&nodes[b].depth)
                    .then(nodes[b].rect.area().total_cmp(&nodes[a].rect.area()))
            })
        };
        let own: Vec<&Node> = snap
            .nodes
            .iter()
            .filter(|n| n.layer.as_deref() == Some(layer))
            .filter(holds)
            .collect();
        let shown_nodes: Vec<&Node> = own
            .iter()
            .copied()
            .filter(|n| n.clip.is_some_and(|c| over_disc(c.intersect(n.rect))))
            .collect();
        // a node whose clip the snapshot does not know may or may not be shown over the disc: when counting it in would
        // change the node that names the dot, that is not told. The candidates keep their snapshot order, so a tie of
        // depth and area goes the same way with and without them.
        let unsure: Vec<&Node> = own.iter().copied().filter(|n| n.clip.is_none()).collect();
        let with_unsure: Vec<&Node> = own
            .iter()
            .copied()
            .filter(|n| n.clip.is_none_or(|c| over_disc(c.intersect(n.rect))))
            .collect();
        let innermost = deepest(&shown_nodes).map(|i| shown_nodes[i]);
        let would_be = deepest(&with_unsure).map(|i| with_unsure[i]);
        if !unsure.is_empty()
            && !match (innermost, would_be) {
                (Some(a), Some(b)) => std::ptr::eq(a, b),
                (None, None) => true,
                _ => false,
            }
        {
            errors.push(format!(
                "the {colour} status circle at {:?} may lie under a node whose clip the snapshot does not know ({}), and counting it in would change which node names the dot, so it cannot be told",
                d.center,
                unsure
                    .iter()
                    .map(|n| format!("{} {:?} at {:?}", n.role, n.label, n.rect))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
            continue;
        }
        let (named, under) = match innermost {
            Some(n) => (!n.label.is_empty(), format!("the {} {:?}", n.role, n.label)),
            None => (false, "no node of its layer".to_string()),
        };
        if !named {
            hits.push(Hit {
                screen: f.screen.to_string(),
                size: f.size,
                invariant: "status_dot_named",
                what: format!("{colour} dot"),
                y: d.center.y,
                ordinal: 0,
                px: 0.0,
                members: vec![],
                message: format!(
                    "a {colour} status circle at {:?} lies under {under}, which has no accessible name; draw it with ui::status::dot (it names the status), or name the widget that holds it with the status word, instead of colour alone",
                    d.center
                ),
            });
        }
    }
    hits
}

/// The findings of `check`, as hits with the rule's message and what to do instead; `px` measures each.
fn from_findings(
    f: &Frame,
    check: &'static str,
    invariant: &'static str,
    px: fn(&Finding, Vec2) -> f32,
    instead: &str,
) -> Vec<Hit> {
    f.findings
        .iter()
        .filter(|x| x.check == check)
        .map(|x| Hit {
            screen: f.screen.to_string(),
            size: f.size,
            invariant,
            what: x.subject.label.clone(),
            y: x.subject.rect[1],
            ordinal: 0,
            px: px(x, f.size),
            members: vec![],
            message: format!(
                "{} {:?}: {} ({}); {instead}",
                x.subject.role, x.subject.rect, x.detail, x.rule
            ),
        })
        .collect()
}

/// How far a finding's rect is past the window's left or right edge.
fn past_edge(x: &Finding, size: Vec2) -> f32 {
    let [left, _, w, _] = x.subject.rect;
    (left + w - size.x).max(-left).max(0.0)
}

/// The larger of the two clip overruns in a `clipped_text` detail ("... by L px on the left, R px on the right ...").
fn clipped_px(x: &Finding, _: Vec2) -> f32 {
    let number = |after: &str, before: &str| -> f32 {
        x.detail
            .split(after)
            .nth(1)
            .and_then(|s| s.split(before).next())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(f32::NAN)
    };
    let left = number("clip by ", " px on the left");
    let right = number("px on the left, ", " px on the right");
    left.max(right)
}

/// Every interactive widget is inside the window's width.
// shortcut: the list sees only what a headless run without the GPU draws: not the 2D and 3D previews, tooltips, a
// worker still running, error states, the light theme or windows under 1100 wide, and the Meshes view only with made-up
// rows; upgrade when a fault a user sees hides in one of those.
fn nothing_past_the_window_edge(f: &Frame) -> Vec<Hit> {
    from_findings(
        f,
        "off_window",
        "window_edge",
        past_edge,
        "let the row wrap or scroll, or give the widget a smaller minimum width, so it fits at this size",
    )
}

/// No text is cut by a clip rect.
// shortcut: same limits as `nothing_past_the_window_edge`, and only a label's left and right cuts are listed (a cut at
// the top or bottom is counted in the summary finding, not listed); upgrade with the first vertical cut a user reports.
fn no_clipped_text(f: &Frame) -> Vec<Hit> {
    from_findings(
        f,
        "clipped_text",
        "clipped_text",
        clipped_px,
        "truncate the label with its full text on hover, or let it wrap, instead of clipping it",
    )
}

/// What the app paints beside a value that is not part of it, each stripped with its one separator space from the end
/// it is painted at: the combo arrow after "Export to: … ▾". Nothing else is trimmed: a leading slash, a trailing dot or
/// an extension belongs to the value. A new decoration fails the walk until it is named here.
const TRAILING_DECORATIONS: &[&str] = &[" ▾"];
const LEADING_DECORATIONS: &[&str] = &[];

/// `source` without one decoration at each end.
fn undecorated(source: &str) -> &str {
    let mut s = source;
    if let Some(t) = TRAILING_DECORATIONS.iter().find_map(|d| s.strip_suffix(d)) {
        s = t;
    }
    if let Some(t) = LEADING_DECORATIONS.iter().find_map(|d| s.strip_prefix(d)) {
        s = t;
    }
    s
}

/// The app's own command labels that end in an ellipsis, by the author's design (it opens a dialog), each whole text
/// exactly as painted. A painted text that ends in an ellipsis, is not cut by egui and whose node holds nothing longer
/// passes only when its whole text is here: the snapshot cannot tell "Settings…" from a hand-cut "report…" by spelling.
/// This is a classification of authored labels, not a tolerance of faults: ALLOWED's count and cap do not apply, but an
/// entry no screen of the walk paints fails (`stale_authored`).
pub const AUTHORED: &[&str] = &[
    "Assign…",
    "Choose export folder…",
    "Choose where the Also folders go…",
    "Choose…",
    "Copy settings from stonewall.png to 0 selected…",
    "Edit match…",
    "Fill from names…",
    "Make marker cube…",
    "MO2…",
    "Map to target…",
    "Move to…",
    "Open folder…",
    "PGPatcher JSON editor…",
    "Path…",
    "Recipe settings…",
    "Reset all settings for 1 selected…",
    "Save…",
    "Scan a folder…",
    "Settings…",
    "Slot commands…",
    "Template…",
];

/// True when `value` is a longer text that `cut` is cut from. `cut` holds the ellipsis where the cut is: at the start
/// (a front cut shows the end of the value), at the end (a back cut shows the start), or between two pieces (a middle
/// cut shows both ends, in order; more than one ellipsis gives more pieces, found in order). The pieces are compared as
/// they are: no trimming. A value equal to `cut` keeps nothing.
fn holds_the_longer_text(cut: &str, value: &str) -> bool {
    if value == cut {
        return false;
    }
    let pieces: Vec<&str> = cut.split('…').collect();
    let (first, last) = (pieces[0], pieces[pieces.len() - 1]);
    let mids = &pieces[1..pieces.len() - 1];
    pieces_cut_from(value, first, mids, last)
}

/// Whether a painted text keeps its full value, exactly as painted (`source`, untrimmed; `elided` when egui cut it):
/// - not cut (no ellipsis in it, not elided): nothing to keep;
/// - cut by egui: some field of the node (`held`: name, value, description) holds the whole source, trailing ellipses
///   included, so a node holding only the shortened start does not pass; and a source that has an ellipsis of its own
///   (a text the app cut by hand before egui cut it again) passes the next rule too;
/// - an ellipsis in it: a field holds a longer text it is cut from (`holds_the_longer_text`, after the named
///   decorations are stripped), or the whole source is in `authored`.
///
/// egui's own truncating `Label` passes by design: `Label::ui` names its node with `galley.text()`, the full text
/// (egui-0.37.0/src/widgets/label.rs line 281), and shows that text on hover when the galley is elided (lines 284 to
/// 292), so its value holds the whole source.
fn cut_text_kept(source: &str, elided: bool, held: &[&str], authored: &[&str]) -> bool {
    if elided && !held.iter().any(|v| v.contains(source)) {
        return false;
    }
    if !source.contains('…') {
        return true;
    }
    // A field equal to the painted source as it is (decoration included) is the text naming itself: never the longer
    // value. A field's own decoration is stripped like the painted text's, so it cannot stand for omitted characters.
    let cut = undecorated(source);
    held.iter()
        .any(|v| *v != source && holds_the_longer_text(cut, undecorated(v)))
        || authored.contains(&source)
}

/// `value` starts with `first`, ends with `last` and holds `mids` in order between them, with at least one character
/// in every gap.
fn pieces_cut_from(value: &str, first: &str, mids: &[&str], last: &str) -> bool {
    let Some(rest) = value.strip_prefix(first) else {
        return false;
    };
    let Some(mut rest) = rest.strip_suffix(last) else {
        return false;
    };
    // each ellipsis stands for at least one character of the value, gap by gap: a piece is looked for after one
    // character (the earliest place is enough: it leaves the most room for the gaps after it)
    for piece in mids {
        let Some(skip) = rest.chars().next().map(char::len_utf8) else {
            return false;
        };
        match rest[skip..].find(piece) {
            Some(at) => rest = &rest[skip + at + piece.len()..],
            None => return false,
        }
    }
    // the last gap, before `last`
    !rest.is_empty()
}

/// The node a painted text lies under: the smallest node whose rect holds the text's rect and that has a name, value or
/// description (a nameless container says nothing about the text). Geometry decides, since the painted text carries no
/// node id; when two nodes have the same area the later (deeper) one wins.
// Compared exactly: egui lays a Label's node out on its galley's rect, and a Button's or ComboBox's node holds the
// text with padding, so no tolerance is needed.
fn node_under<'a>(snap: &'a Snapshot, t: &Text) -> Option<&'a Node> {
    snap.nodes
        .iter()
        .filter(|n| n.rect.contains_rect(t.rect))
        .filter(|n| {
            !n.label.is_empty()
                || n.value.as_deref().is_some_and(|v| !v.is_empty())
                || n.description.as_deref().is_some_and(|v| !v.is_empty())
        })
        .min_by(|a, b| {
            a.rect
                .area()
                .total_cmp(&b.rect.area())
                .then(b.depth.cmp(&a.depth))
        })
}

/// Every painted text that is cut (elided by egui, or showing an ellipsis) lies under a node whose name, value or
/// description holds the longer text.
// Reads `Text::source` (the text exactly as painted) and `Text::elided`; the rules and egui's own `Label::truncate`
// are in `cut_text_kept`.
// shortcut: the node is found by geometry (the smallest named node holding the text's rect); a text drawn in a box
// that another named node sits in is judged by that smaller node. Upgrade when a hit names the wrong node.
fn cut_text_keeps_its_value(f: &Frame) -> Vec<Hit> {
    let mut hits = vec![];
    for t in f.snapshot.texts.iter().chain(&f.snapshot.bare_ellipses) {
        if !t.elided && !t.source.contains('…') {
            continue; // not cut: nothing to keep (`cut_text_kept` says the same)
        }
        let shown = t.source.clone();
        let node = node_under(f.snapshot, t);
        let held = node.map_or_else(Vec::new, |n| {
            [Some(&n.label), n.value.as_ref(), n.description.as_ref()]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .map(String::as_str)
                .collect::<Vec<_>>()
        });
        if cut_text_kept(&t.source, t.elided, &held, AUTHORED) {
            continue;
        }
        let under = match node {
            Some(n) => format!(
                "under {} {:?} (value {:?}, description {:?})",
                n.role, n.label, n.value, n.description
            ),
            None => "under no named node".to_string(),
        };
        hits.push(Hit {
            screen: f.screen.to_string(),
            size: f.size,
            invariant: "cut_text",
            what: shown.clone(),
            y: t.rect.min.y,
            ordinal: 0,
            px: 0.0,
            members: vec![],
            message: format!(
                "the cut text at {:?} {under} keeps no longer text; give the node the full text as its accessible name or value and show it on hover, or use Label::truncate(), instead of a hand-cut text; if it is a command label the author wrote with an ellipsis, add its whole text to AUTHORED in crates/gui/tests/ui_proof/invariants.rs",
                [t.rect.min.x, t.rect.min.y, t.rect.width(), t.rect.height()]
            ),
        });
    }
    hits
}

/// What `walk` collected.
pub struct Walked {
    pub hits: Vec<Hit>,
    /// Visits that could not be made, and markers other entries' screens satisfy; never allowed.
    pub errors: Vec<String>,
    /// Every visit that was completed: what the frame invariants were run on.
    pub visits: Vec<Visit>,
    /// Every distinct painted text (as painted, untrimmed) that has an ellipsis in it, on any screen at any size: what
    /// `stale_authored` reads.
    pub painted_ellipsis_texts: std::collections::BTreeSet<String>,
    /// Every key the key step pressed: what the proof counts, so a step that presses nothing cannot pass.
    pub presses: Vec<Press>,
    /// The name of every scroll bar the walk saw, with its screen and size (`bar_places`): what the proof counts, so a bar no panel names cannot hide.
    pub bars: Vec<String>,
}

/// One key the key step pressed on a screen.
#[derive(Debug, PartialEq)]
pub struct Press {
    pub screen: String,
    pub key: &'static str,
    /// How many paths (files and folders) were hashed after the key.
    pub paths: usize,
    /// False when the key closed the screen and the walk reopened it.
    pub shown: bool,
}

/// The `AUTHORED` entries that no screen of the walk painted: each is a failure naming the entry.
pub fn stale_authored(walked: &Walked, authored: &[&str]) -> Vec<String> {
    authored
        .iter()
        .filter(|a| !walked.painted_ellipsis_texts.contains(**a))
        .map(|a| {
            format!(
                "AUTHORED in crates/gui/tests/ui_proof/invariants.rs holds {a:?}, but no screen of the walk paints it; delete the entry"
            )
        })
        .collect()
}

/// One completed visit: the screen at a size, and how many findings `run_checks` returned for its frame.
#[derive(Debug, PartialEq)]
pub struct Visit {
    pub screen: String,
    pub size: (f32, f32),
    pub findings: usize,
}

/// Every entry at every size: one frame, one `run_checks`, every frame invariant. A visit that fails is collected and
/// the walk goes on. Afterwards each entry's marker is checked against every other entry's screen at the middle size.
/// The checker the shipped walk runs on each frame; the proof passes a wrapper that records its calls.
pub fn real_checker(_screen: &str, _size: Vec2, snap: &Snapshot) -> Vec<Finding> {
    run_checks(snap)
}

pub fn walk(
    entries: &mut [Box<dyn Entry>],
    checker: &mut dyn FnMut(&str, Vec2, &Snapshot) -> Vec<Finding>,
) -> Walked {
    let mut walked = Walked {
        hits: vec![],
        errors: vec![],
        visits: vec![],
        painted_ellipsis_texts: Default::default(),
        presses: vec![],
        bars: vec![],
    };
    let mut middle: Vec<Option<Snapshot>> = vec![];
    for e in entries.iter_mut() {
        let mut kept = None;
        for (i, size) in SIZES.into_iter().enumerate() {
            let snap = match e
                .at(size)
                .and_then(|snap| no_stray_dialog(&snap, e.dialog()).map(|()| snap))
                .and_then(|snap| at_the_size(snap, size))
            {
                Ok(snap) => snap,
                Err(why) => {
                    walked
                        .errors
                        .push(format!("{} at {}x{}: {why}", e.name(), size.x, size.y));
                    continue;
                }
            };
            let findings = checker(e.name(), size, &snap);
            for t in snap.texts.iter().chain(&snap.bare_ellipses) {
                if t.source.contains('…') {
                    walked.painted_ellipsis_texts.insert(t.source.clone());
                }
            }
            let frame = Frame {
                screen: e.name(),
                size,
                snapshot: &snap,
                findings: &findings,
            };
            let first = walked.hits.len();
            for (_, place) in bar_places(&snap) {
                if let Ok(what) = place {
                    walked
                        .bars
                        .push(format!("{} at {}x{}: {what}", e.name(), size.x, size.y));
                }
            }
            for invariant in FRAME_INVARIANTS {
                walked.hits.extend(invariant(&frame));
            }
            for invariant in OBSERVING_INVARIANTS {
                let mut unseen = vec![];
                walked.hits.extend(invariant(&frame, &mut unseen));
                walked.errors.extend(
                    unseen
                        .into_iter()
                        .map(|why| format!("{} at {}x{}: {why}", e.name(), size.x, size.y)),
                );
            }
            number_same_named(&mut walked.hits[first..]);
            walked.visits.push(Visit {
                screen: e.name().to_string(),
                size: (size.x, size.y),
                findings: findings.len(),
            });
            if i == 1 {
                kept = Some(snap);
            }
        }
        middle.push(kept);
        // The key step runs wherever the walk does, so the proof tests that put the real start screen in their list run it
        // on that screen too (a second or so each); their assertions do not read it.
        if e.takes_keys() {
            press_keys(&mut **e, SIZES[1], &mut walked);
        }
        // the pairwise marker check below reads only the kept middle frame and the marker
        e.release();
    }
    for (a, ea) in entries.iter().enumerate() {
        for (b, eb) in entries.iter().enumerate() {
            if a != b && middle[b].as_ref().is_some_and(|s| holds(s, ea.marker())) {
                walked.errors.push(format!(
                    "the marker of {:?} also holds on the screen {:?}; give {:?} a marker only its own screen satisfies",
                    ea.name(),
                    eb.name(),
                    ea.name()
                ));
            }
        }
    }
    walked
}

/// The invariant name of the key step's hits.
pub const KEYS_NEVER_WRITE: &str = "keys_never_write";

/// The keys the key step presses on every screen, in this order, each with the name a failure gives it.
pub const KEYS: [(Key, &str); 3] = [
    (Key::Enter, "Enter"),
    (Key::Escape, "Escape"),
    (Key::Space, "Space"),
];

/// Every path under some folders, keyed by the folder's name and the native path relative to it (two names that print
/// alike stay two entries; `shown` formats a key for a message): a folder is `None`, a file is its length and a hash of
/// its bytes. A file counts as changed when its bytes are, not when its modification time is: a save that writes the
/// same bytes is no change.
pub type Tree = BTreeMap<(&'static str, PathBuf), Option<(u64, u64)>>;

/// A key of a `Tree` as a message names it: "<folder name>/<relative path>", with forward slashes.
fn shown(key: &(&'static str, PathBuf)) -> String {
    format!("{}/{}", key.0, key.1.to_string_lossy().replace('\\', "/"))
}

/// How `scan` reads a file; the real one is `std::fs::read`, a test injects a failing one.
type ReadFile<'a> = &'a dyn Fn(&Path) -> std::io::Result<Vec<u8>>;

fn add_to_tree(
    name: &'static str,
    root: &Path,
    dir: &Path,
    read: ReadFile,
    tree: &mut Tree,
) -> Result<(), String> {
    use std::hash::Hasher;
    let list = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot list {} in the {name} folder: {e}", dir.display()))?;
    for entry in list {
        let entry = entry.map_err(|e| {
            format!(
                "cannot read an entry of {} in the {name} folder: {e}",
                dir.display()
            )
        })?;
        let path = entry.path();
        let is_dir = entry
            .file_type()
            .map_err(|e| {
                format!(
                    "cannot tell what {} in the {name} folder is: {e}",
                    path.display()
                )
            })?
            .is_dir();
        let key = (name, path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        if is_dir {
            tree.insert(key, None);
            add_to_tree(name, root, &path, read, tree)?;
        } else {
            // an unreadable file is an error, never a stand-in hash: two failed reads must not compare equal
            let bytes = read(&path)
                .map_err(|e| format!("cannot read {} in the {name} folder: {e}", path.display()))?;
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            hasher.write(&bytes);
            tree.insert(key, Some((bytes.len() as u64, hasher.finish())));
        }
    }
    Ok(())
}

/// The tree of every watched folder, each under its name; `Err` (naming the folder and the path) when anything cannot
/// be listed or read, because a tree with a hole in it proves nothing.
pub fn scan(roots: &[(&'static str, PathBuf)], read: ReadFile) -> Result<Tree, String> {
    let mut tree = Tree::new();
    for (name, root) in roots {
        add_to_tree(name, root, root, read, &mut tree)?;
    }
    Ok(tree)
}

/// `scan` with the real file read.
pub fn tree_of(roots: &[(&'static str, PathBuf)]) -> Result<Tree, String> {
    scan(roots, &|p| std::fs::read(p))
}

/// What differs between two trees: a new path, a deleted path, a file whose bytes changed (or a path that was a folder
/// and is a file now), each with the kind of change.
pub fn changes(before: &Tree, after: &Tree) -> Vec<(String, &'static str)> {
    let mut out = vec![];
    for (path, now) in after {
        match (before.get(path), now) {
            (None, None) => out.push((shown(path), "new folder")),
            (None, Some(_)) => out.push((shown(path), "new file")),
            (Some(was), now) if was != now => out.push((shown(path), "changed")),
            _ => {}
        }
    }
    for (path, was) in before {
        if !after.contains_key(path) {
            out.push((
                shown(path),
                if was.is_none() {
                    "deleted folder"
                } else {
                    "deleted"
                },
            ));
        }
    }
    out
}

/// The hit of one key: what it changed or started.
fn key_hit(screen: &str, size: Vec2, key: &str, what: String, effect: &str) -> Hit {
    Hit {
        screen: screen.to_string(),
        size,
        invariant: KEYS_NEVER_WRITE,
        what,
        y: 0.0,
        ordinal: 0,
        px: 0.0,
        members: vec![],
        message: format!(
            "pressing {key} with nothing focused {effect}; a key with nothing focused never writes a file or starts an export (egui-ui-rules, dialogs: Enter triggers the safe default and never overwrites, deletes or exports)"
        ),
    }
}

/// The key step of one screen, at `size`: presses Enter, Escape and Space one after the other, each with nothing focused
/// and on the screen under test, and hits every file a key changed and every key that started an export. It passes only
/// on what it observed: a folder it could not scan, a settle that timed out, an export already running, or a screen that
/// clearing the focus dismisses is an error of this screen and key.
///
/// Per key: `arm` clears the focus and checks the marker on the frame after; a marker that is gone means a replay of the
/// entry (part of this screen's visit), asked again, and gone again is an error. The folders are hashed right before the
/// key and right after it, so only a change the key caused counts. A key that closed the screen is followed by a replay
/// only when another key comes (the next `arm` finds the marker gone). Each key's findings are appended at once, so a
/// later error cannot drop them.
fn press_keys(e: &mut dyn Entry, size: Vec2, walked: &mut Walked) {
    let screen = e.name().to_string();
    let mut error = |why: String| {
        walked
            .errors
            .push(format!("{screen} keys at {}x{}: {why}", size.x, size.y))
    };
    if let Err(why) = e.at(size) {
        return error(why);
    }
    for (key, name) in KEYS {
        let mut replayed = false;
        loop {
            match e.arm() {
                Err(why) => return error(format!("before {name}: {why}")),
                Ok(true) => break,
                Ok(false) if replayed => {
                    return error(format!(
                        "before {name}: clearing the keyboard focus dismisses this screen, so no key can be pressed on it with nothing focused; the screen's open or marker needs a state that survives an empty focus"
                    ))
                }
                Ok(false) => {
                    replayed = true;
                    if let Err(why) = e.reopen(size) {
                        return error(format!("before {name}: the screen could not be reopened: {why}"));
                    }
                }
            }
        }
        let before = match tree_of(&e.watched()) {
            Ok(tree) => tree,
            Err(why) => return error(format!("before {name}: {why}")),
        };
        let pressed = match e.press(key) {
            Ok(p) => p,
            Err(why) => return error(format!("{name}: {why}")),
        };
        let after = match tree_of(&e.watched()) {
            Ok(tree) => tree,
            Err(why) => return error(format!("after {name}: {why}")),
        };
        walked.presses.push(Press {
            screen: screen.clone(),
            key: name,
            paths: after.len(),
            shown: pressed.shown,
        });
        for (path, kind) in changes(&before, &after) {
            walked.hits.push(key_hit(
                &screen,
                size,
                name,
                format!("{name}: {path} ({kind})"),
                &format!("changed {path} ({kind})"),
            ));
        }
        if pressed.running_on_key_down || pressed.running_after_settle {
            walked.hits.push(key_hit(
                &screen,
                size,
                name,
                format!("{name}: export running"),
                "started an export (the model's export.running was set)",
            ));
        } else if pressed.witness_changed {
            walked.hits.push(key_hit(
                &screen,
                size,
                name,
                format!("{name}: export record changed"),
                "changed the export's record (rows, events, summary, error or notes) without export.running being seen: an export started and ended inside the key's frames",
            ));
        }
    }
}
/// Numbers the hits of one frame that share an invariant and a name, top to bottom.
fn number_same_named(hits: &mut [Hit]) {
    let mut order: Vec<usize> = (0..hits.len()).collect();
    order.sort_by(|&a, &b| hits[a].y.total_cmp(&hits[b].y));
    for &i in &order {
        let before = order
            .iter()
            .take_while(|&&j| j != i)
            .filter(|&&j| hits[j].invariant == hits[i].invariant && hits[j].what == hits[i].what)
            .count();
        hits[i].ordinal = before;
    }
}

/// One allowed hit: the screen, the invariant, the size, the widget's exact name and its place among the same-named
/// hits from the top (`ordinal`), how many hits it covers and how far past the edge (or clipped) each is, in px, within
/// `PX_TOLERANCE`; and why it is allowed today.
pub struct Allowed {
    pub screen: &'static str,
    pub invariant: &'static str,
    pub size: (f32, f32),
    pub what: &'static str,
    pub ordinal: usize,
    pub count: usize,
    pub px: f32,
    /// For a hit on an area (a scroll bar's clearance): its widgets, "Role:name" with each one's own px, sorted by name
    /// then px, exactly the hit's `members` (each px within `PX_TOLERANCE`). Empty for every other entry.
    pub members: &'static [(&'static str, f32)],
    pub reason: &'static str,
}

/// How far a measured overflow may differ from an entry's.
pub const PX_TOLERANCE: f32 = 2.0;

/// Hits the tree has today. Each is a real fault in `crates/gui/src` that the task adding the invariant did not fix;
/// delete the entry when it is fixed.
pub const ALLOWED: &[Allowed] = &[
    Allowed {
        screen: "presets tab",
        invariant: "window_edge",
        size: (1100.0, 800.0),
        what: "Preset target",
        ordinal: 0,
        count: 1,
        px: 78.1,
        members: &[],
        reason: "the target combo ends past the right edge (x 1078..1178)",
    },
    Allowed {
        screen: "json editor",
        invariant: "window_edge",
        size: (1100.0, 800.0),
        what: "derived from the set",
        ordinal: 0,
        count: 1,
        px: 91.7,
        members: &[],
        reason:
            "the first row's derived-path tag ends past the right edge (x 1075.5..1191.7, y 400.7)",
    },
    Allowed {
        screen: "json editor",
        invariant: "window_edge",
        size: (1100.0, 800.0),
        what: "derived from the set",
        ordinal: 1,
        count: 1,
        px: 107.3,
        members: &[],
        reason:
            "the second row's derived-path tag ends past the right edge (x 1091.2..1207.3, y 507.2)",
    },
    Allowed {
        screen: "json editor",
        invariant: "clipped_text",
        size: (1100.0, 800.0),
        what: "derived from the set",
        ordinal: 0,
        count: 1,
        px: 99.7,
        members: &[],
        reason: "the first row's tag sticks out of its clip",
    },
    Allowed {
        screen: "json editor",
        invariant: "clipped_text",
        size: (1100.0, 800.0),
        what: "derived from the set",
        ordinal: 1,
        count: 1,
        px: 115.3,
        members: &[],
        reason: "the second row's tag sticks out of its clip",
    },
    // The nine below are not faults: an ellipsis the author wrote in the middle of a text (a shortcut after it, a word in
    // parentheses, a list that goes on), which the walk cannot tell from a hand-cut middle ("C:\…\export" named by
    // itself). Each is one text, one size. The texts that end in an ellipsis are in AUTHORED.
    Allowed {
        screen: "tile menu",
        invariant: "cut_text",
        size: (1100.0, 800.0),
        what: "Rename input…  F2",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the menu entry's own ellipsis, then its shortcut; the button's name is that text",
    },
    Allowed {
        screen: "tile menu",
        invariant: "cut_text",
        size: (1280.0, 800.0),
        what: "Rename input…  F2",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the menu entry's own ellipsis, then its shortcut; the button's name is that text",
    },
    Allowed {
        screen: "tile menu",
        invariant: "cut_text",
        size: (1600.0, 900.0),
        what: "Rename input…  F2",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the menu entry's own ellipsis, then its shortcut; the button's name is that text",
    },
    Allowed {
        screen: "rename inputs dialog",
        invariant: "cut_text",
        size: (1100.0, 800.0),
        what: "Only the base name changes (the part before _n, _normal…). Tick Whole path to change the folders too.",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the sentence's own \"…\" (and so on) inside the parentheses; the label is that sentence",
    },
    Allowed {
        screen: "rename inputs dialog",
        invariant: "cut_text",
        size: (1280.0, 800.0),
        what: "Only the base name changes (the part before _n, _normal…). Tick Whole path to change the folders too.",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the sentence's own \"…\" (and so on) inside the parentheses; the label is that sentence",
    },
    Allowed {
        screen: "rename inputs dialog",
        invariant: "cut_text",
        size: (1600.0, 900.0),
        what: "Only the base name changes (the part before _n, _normal…). Tick Whole path to change the folders too.",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the sentence's own \"…\" (and so on) inside the parentheses; the label is that sentence",
    },
    Allowed {
        screen: "skin card",
        invariant: "cut_text",
        size: (1100.0, 800.0),
        what: "no RFAOS (Assign…)",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the chip's own \"Assign…\" inside parentheses; its button's name goes on with \" — go to the fix\"",
    },
    Allowed {
        screen: "skin card",
        invariant: "cut_text",
        size: (1280.0, 800.0),
        what: "no RFAOS (Assign…)",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the chip's own \"Assign…\" inside parentheses; its button's name goes on with \" — go to the fix\"",
    },
    Allowed {
        screen: "skin card",
        invariant: "cut_text",
        size: (1600.0, 900.0),
        what: "no RFAOS (Assign…)",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the chip's own \"Assign…\" inside parentheses; its button's name goes on with \" — go to the fix\"",
    },
    Allowed {
        screen: "presets tab",
        invariant: "scroll_bar_at_rest",
        size: (1100.0, 800.0),
        what: "scroll bar 1 of 1 in the centre panel",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the presets tab's ScrollArea::both (ui/presets.rs, id presets-scroll) shows a floating bar that paints only transparent fills at rest; it has no look::thin_scroll()",
    },
    Allowed {
        screen: "presets tab",
        invariant: "scroll_bar_at_rest",
        size: (1280.0, 800.0),
        what: "scroll bar 1 of 1 in the centre panel",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the presets tab's ScrollArea::both (ui/presets.rs, id presets-scroll) shows a floating bar that paints only transparent fills at rest; it has no look::thin_scroll()",
    },
    Allowed {
        screen: "presets tab",
        invariant: "scroll_bar_at_rest",
        size: (1600.0, 900.0),
        what: "scroll bar 1 of 1 in the centre panel",
        ordinal: 0,
        count: 1,
        px: 0.0,
        members: &[],
        reason: "the presets tab's ScrollArea::both (ui/presets.rs, id presets-scroll) shows a floating bar that paints only transparent fills at rest; it has no look::thin_scroll()",
    },
    Allowed {
        screen: "review tab",
        invariant: "scroll_bar_clearance",
        size: (1100.0, 800.0),
        what: "scroll bar 1 of 1 in the bottom-review panel",
        ordinal: 0,
        count: 1,
        px: 4.0,
        members: &[("Button:Choose where the Also folders go…", 4.0)],
        reason: "the Also folders button ends 4 px inside the bar's hovered width",
    },
    Allowed {
        screen: "review tab",
        invariant: "scroll_bar_clearance",
        size: (1280.0, 800.0),
        what: "scroll bar 1 of 1 in the bottom-review panel",
        ordinal: 0,
        count: 1,
        px: 3.7,
        members: &[
            ("Button:Choose where the Also folders go…", 3.7),
            ("CheckBox:architecture/stone/cobble", 3.7),
            ("CheckBox:pbr/metal", 3.7),
            ("Label:Also folders go in", 3.7),
            ("Label:Skip these sets", 3.7),
            ("Link:Recipe settings…", 3.7),
            ("TextInput:CM folder name", 3.7),
            ("TextInput:PBR folder name", 3.7),
            ("TextInput:Vanilla folder name", 3.7),
        ],
        reason: "the Also folders controls end 3.7 px inside the bar's hovered width",
    },
    Allowed {
        screen: "review tab",
        invariant: "scroll_bar_clearance",
        size: (1600.0, 900.0),
        what: "scroll bar 1 of 1 in the bottom-review panel",
        ordinal: 0,
        count: 1,
        px: 4.0,
        members: &[
            ("Button:Choose where the Also folders go…", 4.0),
            ("CheckBox:architecture/stone/cobble", 4.0),
            ("CheckBox:pbr/metal", 4.0),
            ("Label:Also folders go in", 4.0),
            ("Label:Skip these sets", 4.0),
            ("Link:Recipe settings…", 4.0),
            ("TextInput:CM folder name", 4.0),
            ("TextInput:PBR folder name", 4.0),
            ("TextInput:Vanilla folder name", 4.0),
        ],
        reason: "the Also folders controls end 4 px inside the bar's hovered width",
    },
];
fn covers(a: &Allowed, h: &Hit) -> bool {
    a.screen == h.screen
        && a.invariant == h.invariant
        && (a.size.0 - h.size.x).abs() < 0.5
        && (a.size.1 - h.size.y).abs() < 0.5
        && a.what == h.what
        && a.ordinal == h.ordinal
        && (a.px - h.px).abs() <= PX_TOLERANCE
        && a.members.len() == h.members.len()
        && a.members
            .iter()
            .zip(&h.members)
            .all(|((name, px), (hname, hpx))| name == hname && (px - hpx).abs() <= PX_TOLERANCE)
}

/// The final failure list: every collected error, every hit no entry consumed, every entry not consumed in full, and
/// every `authored` text no screen painted (`stale_authored`). Entries are consumed one hit at a time; empty when the
/// walk passes.
pub fn verdict(walked: &Walked, allowed: &[Allowed], authored: &[&str]) -> Vec<String> {
    let mut left: Vec<usize> = allowed.iter().map(|a| a.count).collect();
    let mut out = walked.errors.clone();
    for h in &walked.hits {
        let slot = allowed
            .iter()
            .enumerate()
            .find(|(i, a)| left[*i] > 0 && covers(a, h))
            .map(|(i, _)| i);
        match slot {
            Some(i) => left[i] -= 1,
            None => out.push(format!(
                "{}; if it cannot be fixed now, add it to ALLOWED in crates/gui/tests/ui_proof/invariants.rs with its screen, size, name, ordinal, count and px, and a reason",
                h.text()
            )),
        }
    }
    for (a, n) in allowed.iter().zip(&left) {
        if *n > 0 {
            out.push(format!(
                "ALLOWED in crates/gui/tests/ui_proof/invariants.rs holds {} / {} / {}x{} / {:?} #{} / count {} / {:.1} px ({}), but {} of its hits did not come; delete or correct the entry",
                a.screen, a.invariant, a.size.0, a.size.1, a.what, a.ordinal, a.count, a.px, a.reason, n
            ));
        }
    }
    out.extend(stale_authored(walked, authored));
    out
}

#[test]
fn the_walk_finds_nothing_new_on_any_screen() {
    let start = std::time::Instant::now();
    let mut list = entries();
    let walked = walk(&mut list, &mut real_checker);
    println!(
        "walk: {} screens x {} sizes in {:.1?}, {} hits, {} errors",
        list.len(),
        SIZES.len(),
        start.elapsed(),
        walked.hits.len(),
        walked.errors.len()
    );
    if std::env::var_os("SDC_WALK_HITS").is_some() {
        for h in &walked.hits {
            println!("HIT {}", h.text());
        }
        for p in &walked.presses {
            println!("PRESS {p:?}");
        }
        for b in &walked.bars {
            println!("BAR {b}");
        }
    }
    let bad = verdict(&walked, ALLOWED, AUTHORED);
    assert!(bad.is_empty(), "{bad:#?}");
    assert_eq!(
        walked.visits.len(),
        list.len() * SIZES.len(),
        "a visit was not completed or not recorded"
    );
    // the key step pressed Enter, Escape and Space on every screen, in order
    let pressed: Vec<(&str, &str)> = walked
        .presses
        .iter()
        .map(|p| (p.screen.as_str(), p.key))
        .collect();
    let mut want = vec![];
    for e in &list {
        for (_, key) in KEYS {
            want.push((e.name(), key));
        }
    }
    assert_eq!(pressed, want, "a key was not pressed on a screen");
}

/// A test-only entry: a tiny UI with a button that ends past the window's right edge at every size and a label cut by
/// its clip. It is not in the shipped list: a shipped entry opens the real app, which has no widget to plant.
struct Faulty;

const FAULTY_MARKER: &[Needle] = &[crate::screens::has_role("Button", "Far away")];

fn faulty_ui(ui: &mut eframe::egui::Ui) {
    use eframe::egui::{pos2, Rect};
    // 1000 + 700 = 1700: past the edge of the 1100, 1280 and 1600 wide windows by 600, 420 and 100 px
    ui.put(
        Rect::from_min_size(pos2(1000.0, 20.0), vec2(700.0, 24.0)),
        eframe::egui::Button::new("Far away"),
    );
    ui.scope(|ui| {
        ui.set_clip_rect(Rect::from_min_size(pos2(10.0, 60.0), vec2(40.0, 40.0)));
        ui.put(
            Rect::from_min_size(pos2(10.0, 60.0), vec2(200.0, 20.0)),
            eframe::egui::Label::new("cut label sticking out"),
        );
    });
}

impl Entry for Faulty {
    fn name(&self) -> &str {
        "test-only faulty screen"
    }

    fn marker(&self) -> &'static [Needle] {
        FAULTY_MARKER
    }

    fn at(&mut self, size: Vec2) -> Result<Snapshot, String> {
        let mut h = Harness::builder()
            .with_context_setup(|ctx| {
                sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
            })
            .with_size(size)
            .build_ui(faulty_ui);
        h.run_steps(3);
        Ok(snapshot(&h))
    }
}

/// An entry that cannot show its screen at the narrowest size.
struct Broken;

const BROKEN_MARKER: &[Needle] = &[crate::screens::has("never drawn")];

impl Entry for Broken {
    fn name(&self) -> &str {
        "test-only broken screen"
    }

    fn marker(&self) -> &'static [Needle] {
        BROKEN_MARKER
    }

    fn at(&mut self, size: Vec2) -> Result<Snapshot, String> {
        if size.x < 1200.0 {
            Err("did not go idle".into())
        } else {
            Faulty.at(size)
        }
    }
}

/// The start of a failure message up to its px: screen, size, invariant, name and ordinal, without the rect text.
fn head(message: &str) -> &str {
    &message[..message.find(" px): ").expect("a hit line") + " px)".len()]
}

fn the_start_screen() -> Box<dyn Entry> {
    entries()
        .into_iter()
        .find(|e| e.name() == "start screen")
        .expect("the start screen entry")
}

/// The faulty entry goes through `walk` and `verdict`, the code the shipped list goes through, beside a clean shipped
/// entry. The six visits are exactly those of the two entries at the three sizes, and the final failure list is
/// exactly the six diagnostics of the faulty entry (the button past the edge and the clipped label, at each size):
/// nothing for the clean one, and none dropped.
#[test]
fn a_widget_past_the_edge_and_a_clipped_text_fail_the_walk() {
    let mut list: Vec<Box<dyn Entry>> = vec![the_start_screen(), Box::new(Faulty)];
    // the checker is a wrapper that records each call and runs the real one: a visit that skips it leaves no record
    let mut calls: Vec<(String, (f32, f32))> = vec![];
    let walked = walk(&mut list, &mut |screen, size, snap| {
        calls.push((screen.to_string(), (size.x, size.y)));
        real_checker(screen, size, snap)
    });
    let bad = verdict(&walked, &[], &[]);
    println!("{bad:#?}");
    let mut want_calls = vec![];
    for name in ["start screen", "test-only faulty screen"] {
        for size in SIZES {
            want_calls.push((name.to_string(), (size.x, size.y)));
        }
    }
    assert_eq!(calls, want_calls, "the checker ran on every frame");
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    // (a) every visit happened, and each ran `run_checks` on the frame the entry shows
    let count = |mut e: Box<dyn Entry>, size: Vec2| run_checks(&e.at(size).unwrap()).len();
    let mut expected = vec![];
    for (name, make) in [
        ("start screen", the_start_screen as fn() -> Box<dyn Entry>),
        ("test-only faulty screen", || -> Box<dyn Entry> {
            Box::new(Faulty)
        }),
    ] {
        for size in SIZES {
            expected.push(Visit {
                screen: name.into(),
                size: (size.x, size.y),
                findings: count(make(), size),
            });
        }
    }
    assert_eq!(walked.visits, expected);
    // (b) exactly these failures, sorted
    let mut got: Vec<&str> = bad.iter().map(|m| head(m)).collect();
    got.sort();
    let mut want = vec![
        r#"test-only faulty screen at 1100x800: window_edge "Far away" #0 (600.0 px)"#,
        r#"test-only faulty screen at 1280x800: window_edge "Far away" #0 (420.0 px)"#,
        r#"test-only faulty screen at 1600x900: window_edge "Far away" #0 (100.0 px)"#,
        r#"test-only faulty screen at 1100x800: clipped_text "cut label sticking out" #0 (160.0 px)"#,
        r#"test-only faulty screen at 1280x800: clipped_text "cut label sticking out" #0 (160.0 px)"#,
        r#"test-only faulty screen at 1600x900: clipped_text "cut label sticking out" #0 (160.0 px)"#,
    ];
    want.sort();
    assert_eq!(got, want, "{bad:#?}");
    assert!(
        bad.iter().all(|m| m.contains("ALLOWED")),
        "every failure says what to do about it: {bad:#?}"
    );
    // the narrow-window line names the widget's role
    assert!(
        bad.iter().any(|m| m.starts_with(
            r#"test-only faulty screen at 1100x800: window_edge "Far away" #0 (600.0 px): Button ["#
        )),
        "{bad:#?}"
    );
}

/// A test-only entry with cut texts: hand-cut texts whose node keeps no longer value (the faults: a middle cut named
/// by itself, a self-named `report…`, a leading slash the name lost, a text egui cut whose name lacks its trailing
/// ellipsis, a hand cut egui cut again), and fine ones (a `Label::truncate`, a hand cut named by the full path, with and
/// without a decoration, an authored label egui cut).
struct CutTexts;

const CUT_MARKER: &[Needle] = &[crate::screens::has_role("Label", "some/long/…/name")];

fn cut_texts_ui(ui: &mut eframe::egui::Ui) {
    use eframe::egui::{pos2, Rect};
    let slot = |y: f32, w: f32| Rect::from_min_size(pos2(10.0, y), vec2(w, 20.0));
    let label = |text: &str| eframe::egui::Label::new(text.to_string());
    // faults
    ui.put(slot(20.0, 200.0), label("some/long/…/name"));
    ui.put(slot(110.0, 200.0), label("report…"));
    ui.put(slot(230.0, 200.0), label("report… ▾"));
    ui.put(slot(140.0, 200.0), label("/root…file"))
        .accessible_name("rootXYZfile");
    // egui cuts this galley (its source is the whole text, ellipsis included); the name has the text without it
    ui.put(
        slot(170.0, 60.0),
        eframe::egui::Button::new("abcdefghijkl…").truncate(),
    )
    .accessible_name("abcdefghijkl");
    // fine
    ui.put(
        slot(50.0, 90.0),
        label("a long label that egui cuts").truncate(),
    );
    ui.put(slot(80.0, 200.0), label("other/long/…/name"))
        .on_hover_text("other/long/path/to/the/name")
        .accessible_name("other/long/path/to/the/name");
    ui.put(slot(200.0, 200.0), label("pick/…/z ▾"))
        .accessible_name("pick/a/b/z");
    // egui cuts both again: a text the app cut by hand, named only by itself (a fault), and an authored label (fine)
    ui.put(slot(260.0, 40.0), label("report/final…").truncate());
    ui.put(slot(290.0, 40.0), label("Settings…").truncate());
}

impl Entry for CutTexts {
    fn name(&self) -> &str {
        "test-only cut-text screen"
    }

    fn marker(&self) -> &'static [Needle] {
        CUT_MARKER
    }

    fn at(&mut self, size: Vec2) -> Result<Snapshot, String> {
        let mut h = Harness::builder()
            .with_context_setup(|ctx| {
                sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
            })
            .with_size(size)
            .build_ui(cut_texts_ui);
        h.run_steps(3);
        Ok(snapshot(&h))
    }
}

/// The cut-text invariant goes through `walk` and `verdict` beside a clean shipped entry: the exact failure list is the
/// six faulty texts at each size, and nothing for the five fine ones.
#[test]
fn a_cut_text_without_its_full_value_fails_the_walk() {
    let mut list: Vec<Box<dyn Entry>> = vec![the_start_screen(), Box::new(CutTexts)];
    let mut calls: Vec<(String, (f32, f32))> = vec![];
    let walked = walk(&mut list, &mut |screen, size, snap| {
        calls.push((screen.to_string(), (size.x, size.y)));
        real_checker(screen, size, snap)
    });
    let bad = verdict(&walked, &[], &[]);
    let mut want_calls = vec![];
    for name in ["start screen", "test-only cut-text screen"] {
        for size in SIZES {
            want_calls.push((name.to_string(), (size.x, size.y)));
        }
    }
    assert_eq!(calls, want_calls, "the checker ran on every frame");
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    // the fixture is the case in question: egui cut four galleys, and six more painted texts show an ellipsis
    let snap = CutTexts.at(SIZES[0]).unwrap();
    let mut elided: Vec<&str> = snap
        .texts
        .iter()
        .filter(|t| t.elided)
        .map(|t| t.source.as_str())
        .collect();
    elided.sort();
    assert_eq!(
        elided,
        [
            "Settings…",
            "a long label that egui cuts",
            "abcdefghijkl…",
            "report/final…"
        ]
    );
    assert_eq!(
        snap.texts
            .iter()
            .filter(|t| !t.elided && t.source.contains('…'))
            .count(),
        6
    );
    // each faulty text and the role of the node it lies under
    let faults = [
        ("some/long/…/name", "Label"),
        ("report…", "Label"),
        ("report… ▾", "Label"),
        ("/root…file", "Label"),
        ("abcdefghijkl…", "Button"),
        ("report/final…", "Label"),
    ];
    let mut got: Vec<&str> = bad.iter().map(|m| head(m)).collect();
    got.sort();
    let mut want = vec![];
    for size in ["1100x800", "1280x800", "1600x900"] {
        for (text, _) in faults {
            want.push(format!(
                "test-only cut-text screen at {size}: cut_text {text:?} #0 (0.0 px)"
            ));
        }
    }
    want.sort();
    assert_eq!(got, want, "{bad:#?}");
    for (text, role) in faults {
        let lines: Vec<&String> = bad
            .iter()
            .filter(|m| m.contains(&format!(": cut_text {text:?} #0 ")))
            .collect();
        assert_eq!(lines.len(), 3, "{text:?}: {bad:#?}");
        assert!(
            lines
                .iter()
                .all(|m| m.contains(&format!("under {role} \""))),
            "{text:?} lies under a {role}: {lines:#?}"
        );
    }
    assert!(
        bad.iter().all(|m| m.contains("ALLOWED")
            && m.contains("AUTHORED")
            && !m.contains("under no named node")
            && m.contains("Label::truncate()")),
        "every failure names the node and says what to do about it: {bad:#?}"
    );
}

/// Every ellipsis stands for at least one character of the value, gap by gap: one gap cannot pay for another.
#[test]
fn every_ellipsis_stands_for_a_character_gap_by_gap() {
    let none: &[&str] = &[];
    assert!(!cut_text_kept("a…b…c", false, &["abXXc"], none));
    assert!(!cut_text_kept("a…b…c", false, &["aXXbc"], none));
    assert!(cut_text_kept("a…b…c", false, &["aXbXc"], none));
}

/// A decorated text named only by itself keeps nothing, whatever AUTHORED holds, and a field's own decoration is no
/// omitted character either.
#[test]
fn a_decorated_text_named_by_itself_keeps_nothing() {
    let kept = cut_text_kept;
    let none: &[&str] = &[];
    assert!(!kept("report… ▾", false, &["report… ▾"], none));
    assert!(!kept("report… ▾", false, &["report… ▾"], AUTHORED));
    assert!(!kept("report… ▾", false, &["report ▾"], none));
    assert!(kept("report… ▾", false, &["report-final.dds"], none));
    assert!(kept("report… ▾", false, &["report-final.dds ▾"], none));
}

/// What a painted text must keep, through `cut_text_kept`, the function the walk calls for every painted text.
#[test]
fn a_cut_text_must_keep_its_whole_value_exactly() {
    let kept = cut_text_kept;
    let none: &[&str] = &[];
    // a front cut shows the end of the value
    assert!(kept(
        "…path/to/name",
        false,
        &["some/long/path/to/name"],
        none
    ));
    assert!(!kept(
        "…path/to/name",
        false,
        &["some/long/path/to/other"],
        none
    ));
    assert!(!kept("…path/to/name", false, &["…path/to/name"], none));
    // a back cut shows the start
    assert!(kept("some/long/p…", false, &["some/long/path"], none));
    assert!(!kept("some/long/p…", false, &["other/long/path"], none));
    // a middle cut shows both ends, in order
    assert!(kept("some/…/name", false, &["some/long/path/name"], none));
    assert!(!kept("some/…/name", false, &["some/long/path/other"], none));
    assert!(!kept("some/…/name", false, &["other/long/path/name"], none));
    assert!(
        !kept("some/…/name", false, &["some/…/name"], none),
        "named by itself"
    );
    assert!(
        !kept("a…c…e", false, &["aXXe"], none),
        "a middle piece is missing"
    );
    assert!(kept("a…c…e", false, &["aXcXe"], none));
    // any one field of the node may hold it
    assert!(kept("some/…/name", false, &["x", "some/long/name"], none));
    // a named decoration is stripped, with its separator, and nothing else
    assert!(kept(
        "Export to: C:\\…\\export ▾",
        false,
        &["Export to: C:\\Users\\me\\export"],
        none
    ));
    assert!(
        !kept("/root…file", false, &["rootXYZfile"], none),
        "a leading slash belongs to the value"
    );
    assert!(
        !kept("a…b.", false, &["aXXb"], none),
        "a trailing dot belongs to the value"
    );
    assert!(kept("a…b.", false, &["aXXb."], none));
    // a trailing ellipsis named by itself is a fault unless its whole text is AUTHORED
    assert!(!kept("report…", false, &["report…"], none));
    assert!(!kept("report…", false, &["report"], &["Settings…"]));
    assert!(kept("Settings…", false, &["Settings…"], &["Settings…"]));
    assert!(!kept("Settings…", false, &["Settings…"], &["Settings"]));
    // two trailing ellipses and an ellipsis-only text are read as painted
    assert!(!kept("head……", false, &["head…"], none));
    assert!(kept("head……", false, &["head and the rest"], none));
    assert!(!kept("…", false, &["…"], none));
    assert!(kept("…", false, &["Loading the list"], none));
    // a galley egui cut: the whole source, ellipsis included, must be in the node; a source with its own ellipsis was
    // cut by hand before egui cut it again, so it needs a longer field or its whole text in AUTHORED too
    assert!(!kept("abcdef…", true, &["abcdef…"], none), "a double cut");
    assert!(kept("abcdef…", true, &["abcdef…", "abcdefghij"], none));
    assert!(kept("Settings…", true, &["Settings…"], &["Settings…"]));
    assert!(!kept("abcdef…", true, &["abcdef"], none));
    assert!(!kept("abcdef…", true, &["abcdefghij"], none));
    assert!(kept("the full text", true, &["x", "the full text"], none));
    assert!(!kept("the full text", true, &["other"], none));
    // a text with no ellipsis that egui did not cut has nothing to keep
    assert!(kept("plain", false, &[], none));
}
/// A visit that fails is a collected failure naming screen and size; the walk goes on to the other sizes and entries.
#[test]
fn a_failed_visit_is_collected_and_the_walk_goes_on() {
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(Broken), Box::new(Faulty)];
    let walked = walk(&mut list, &mut real_checker);
    let bad = verdict(&walked, &[], &[]);
    assert!(
        bad.iter()
            .any(|m| m == "test-only broken screen at 1100x800: did not go idle"),
        "{bad:#?}"
    );
    // the later sizes of the broken entry and the whole faulty entry were still visited
    assert!(
        walked
            .visits
            .iter()
            .any(|v| v.screen == "test-only broken screen" && v.size.0 == 1280.0),
        "{:?}",
        walked.visits
    );
    assert_eq!(
        walked
            .visits
            .iter()
            .filter(|v| v.screen == "test-only faulty screen")
            .count(),
        SIZES.len()
    );
    // findings survive a failed visit: a real hit from the broken entry's later size and one from the entry after it
    assert!(
        walked
            .hits
            .iter()
            .any(|h| h.screen == "test-only broken screen" && h.size.x == 1280.0),
        "{:?}",
        walked.hits
    );
    assert!(
        walked
            .hits
            .iter()
            .any(|h| h.screen == "test-only faulty screen" && h.invariant == "window_edge"),
        "{:?}",
        walked.hits
    );
}

/// A frame of another size than the one asked for is an error of that size, never a visit credited to it.
#[test]
fn a_frame_of_another_size_is_an_error() {
    struct OneSize;
    impl Entry for OneSize {
        fn name(&self) -> &str {
            "test-only one-size screen"
        }
        fn marker(&self) -> &'static [Needle] {
            FAULTY_MARKER
        }
        fn at(&mut self, _size: Vec2) -> Result<Snapshot, String> {
            Faulty.at(SIZES[1])
        }
    }
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(OneSize)];
    let walked = walk(&mut list, &mut real_checker);
    assert_eq!(
        walked.errors,
        [
            "test-only one-size screen at 1100x800: the frame is 1280x800, not the 1100x800 asked for, so it is no visit at this size",
            "test-only one-size screen at 1600x900: the frame is 1280x800, not the 1600x900 asked for, so it is no visit at this size",
        ]
    );
    let sizes: Vec<(f32, f32)> = walked.visits.iter().map(|v| v.size).collect();
    assert_eq!(sizes, [(1280.0, 800.0)]);
}

/// A shipped-kind entry whose `open` panics is a collected failure naming screen and size; a clean entry beside it is
/// still visited at every size.
#[test]
fn an_entry_whose_open_panics_is_collected_and_the_walk_goes_on() {
    static BOOM: Screen = Screen {
        name: "test-only exploding screen",
        fixture: "demo",
        open: |_| panic!("boom in open"),
        steps: &[],
        act: None,
        marker: &[crate::screens::has("never drawn")],
        transient: false,
        dialog: false,
    };
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(ScreenEntry::new(&BOOM)), the_start_screen()];
    let walked = walk(&mut list, &mut real_checker);
    let bad = verdict(&walked, &[], &[]);
    assert!(
        SIZES.iter().all(|s| bad.iter().any(|m| m
            .starts_with(&format!("test-only exploding screen at {}x{}: ", s.x, s.y))
            && m.contains("boom in open"))),
        "{bad:#?}"
    );
    assert_eq!(
        walked
            .visits
            .iter()
            .filter(|v| v.screen == "start screen")
            .count(),
        SIZES.len(),
        "{:?}",
        walked.visits
    );
}

/// A marker that another entry's screen satisfies is a failure naming both.
#[test]
fn a_marker_another_screen_satisfies_fails() {
    // Twin draws the faulty UI, so the faulty marker holds on it.
    struct Twin;
    impl Entry for Twin {
        fn name(&self) -> &str {
            "test-only twin"
        }
        fn marker(&self) -> &'static [Needle] {
            FAULTY_MARKER
        }
        fn at(&mut self, size: Vec2) -> Result<Snapshot, String> {
            Faulty.at(size)
        }
    }
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(Faulty), Box::new(Twin)];
    let walked = walk(&mut list, &mut real_checker);
    assert!(
        walked.errors.iter().any(|e| e.contains(
            "marker of \"test-only twin\" also holds on the screen \"test-only faulty screen\""
        )),
        "{:?}",
        walked.errors
    );
}

fn hit(invariant: &'static str, what: &str, ordinal: usize, px: f32) -> Hit {
    Hit {
        screen: "s".into(),
        size: SIZES[0],
        invariant,
        what: what.into(),
        y: 0.0,
        ordinal,
        px,
        members: vec![],
        message: String::new(),
    }
}

fn entry(what: &'static str, ordinal: usize, count: usize, px: f32) -> Allowed {
    Allowed {
        screen: "s",
        invariant: "window_edge",
        size: (SIZES[0].x, SIZES[0].y),
        what,
        ordinal,
        count,
        px,
        members: &[],
        reason: "r",
    }
}

fn walked(hits: Vec<Hit>) -> Walked {
    Walked {
        hits,
        errors: vec![],
        visits: vec![],
        painted_ellipsis_texts: Default::default(),
        presses: vec![],
        bars: vec![],
    }
}

/// The allowlist is exact: size, name, ordinal, count and px must all agree, and what it covers is consumed one for
/// one.
#[test]
fn an_allowed_hit_must_match_exactly_and_is_consumed_one_for_one() {
    let two = walked(vec![
        hit("window_edge", "tag", 0, 92.0),
        hit("window_edge", "tag", 1, 107.0),
    ]);
    let exact = [entry("tag", 0, 1, 91.0), entry("tag", 1, 1, 108.0)];
    assert!(verdict(&two, &exact, &[]).is_empty());
    // the two rows exchange their overflows: both entries are wrong
    let swapped = [entry("tag", 0, 1, 108.0), entry("tag", 1, 1, 91.0)];
    assert_eq!(verdict(&two, &swapped, &[]).len(), 4);
    // one fixed: the other entry is stale
    let one = walked(vec![hit("window_edge", "tag", 0, 92.0)]);
    let stale = verdict(&one, &exact, &[]);
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert!(stale[0].contains("did not come"), "{stale:?}");
    // a third hit: unmatched
    let three = walked(vec![
        hit("window_edge", "tag", 0, 92.0),
        hit("window_edge", "tag", 0, 92.0),
        hit("window_edge", "tag", 1, 107.0),
    ]);
    let extra = verdict(&three, &exact, &[]);
    assert_eq!(extra.len(), 1, "{extra:?}");
    // one entry with a count of two is consumed twice
    let both = walked(vec![
        hit("window_edge", "tag", 0, 92.0),
        hit("window_edge", "tag", 0, 92.5),
    ]);
    assert!(verdict(&both, &[entry("tag", 0, 2, 92.0)], &[]).is_empty());
    assert_eq!(verdict(&both, &[entry("tag", 0, 3, 92.0)], &[]).len(), 1);
    assert_eq!(verdict(&both, &[entry("tag", 0, 1, 92.0)], &[]).len(), 1);
    // other size, px off by 10, a name that is only part of the widget's
    let mut moved = entry("tag", 0, 1, 92.0);
    moved.size = (1280.0, 800.0);
    let h = walked(vec![hit("window_edge", "tag", 0, 92.0)]);
    assert_eq!(verdict(&h, &[moved], &[]).len(), 2);
    assert_eq!(verdict(&h, &[entry("tag", 0, 1, 102.0)], &[]).len(), 2);
    assert_eq!(verdict(&h, &[entry("ta", 0, 1, 92.0)], &[]).len(), 2);
    assert_eq!(verdict(&h, &[entry("tag", 0, 1, 94.0)], &[]).len(), 0);
}

/// Same-named hits of one frame are numbered from the top, whatever order they were found in.
#[test]
fn same_named_hits_are_numbered_from_the_top() {
    let mut hits = vec![
        hit("window_edge", "tag", 0, 1.0),
        hit("window_edge", "other", 0, 1.0),
        hit("window_edge", "tag", 0, 1.0),
    ];
    hits[0].y = 500.0;
    hits[2].y = 400.0;
    number_same_named(&mut hits);
    assert_eq!(
        hits.iter().map(|h| h.ordinal).collect::<Vec<_>>(),
        vec![1, 0, 0]
    );
}

/// An AUTHORED entry no screen paints is a failure of the verdict naming it; one that is painted is not.
#[test]
fn an_authored_text_no_screen_paints_is_stale() {
    let mut w = walked(vec![]);
    w.painted_ellipsis_texts.insert("Settings…".to_string());
    assert!(verdict(&w, &[], &["Settings…"]).is_empty());
    let stale = verdict(&w, &[], &["Settings…", "Gone…"]);
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert!(stale[0].contains("\"Gone…\""), "{stale:?}");
}

/// What `KeyFault` keeps: whether its screen is open, the export "flag", a counter for the second witness, and what
/// its faults are.
struct FaultState {
    dir: PathBuf,
    cfg: Faults,
    open: bool,
    /// The export gate's flag: set on the frame that processes Space, cleared on the next frame, as an app that
    /// polls its worker would clear a short export.
    flag: bool,
    /// The second witness: counts exports that started and ended inside one frame.
    counter: u32,
    text: String,
    asked_focus: bool,
}

/// What Space does on the fault screen.
#[derive(Clone, Copy, PartialEq)]
enum SpaceDoes {
    Nothing,
    /// Deletes a file and sets the flag for one frame.
    DeletesAndFlashesFlag,
    /// Counts an export that was never visible as running.
    CountsOnly,
}

/// The faults one `KeyFault` has.
#[derive(Clone, Copy)]
struct Faults {
    enter_writes: bool,
    escape_changes_and_closes: bool,
    space: SpaceDoes,
    space_closes: bool,
    /// The screen shows its marker only while a text field has focus, so clearing the focus dismisses it.
    focus_dismisses: bool,
    /// The settle of this press (0 is the first) times out.
    times_out_at: Option<usize>,
    /// An export is running before any key.
    running_at_start: bool,
    /// The watched folder does not exist, so it cannot be listed.
    unreadable_root: bool,
    /// A modal dialog named "Planted dialog" is open over the screen, whose marker still holds underneath.
    modal: bool,
    /// The entry says it is a dialog screen (`Entry::dialog`).
    is_dialog: bool,
}

const NO_FAULTS: Faults = Faults {
    enter_writes: false,
    escape_changes_and_closes: false,
    space: SpaceDoes::Nothing,
    space_closes: false,
    focus_dismisses: false,
    times_out_at: None,
    running_at_start: false,
    unreadable_root: false,
    modal: false,
    is_dialog: false,
};

/// A test-only entry whose keys break the rule as `Faults` says: Enter writes a new file (and rewrites another with the
/// same bytes, which is not a change), Escape rewrites a file with new bytes and closes the screen, Space deletes a
/// file and flashes the export flag for one frame, or only counts an export. It is not in the shipped list.
struct KeyFault {
    dir: tempfile::TempDir,
    h: Harness<'static, FaultState>,
    cfg: Faults,
    /// How many times the walk asked for the screen again.
    reopened: std::rc::Rc<std::cell::Cell<usize>>,
    pressed: usize,
}

const KEY_FAULT_MARKER: &[Needle] = &[crate::screens::has("Keys fault screen")];

fn key_fault_harness(size: Vec2, cfg: Faults) -> (tempfile::TempDir, Harness<'static, FaultState>) {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in [
        ("kept.txt", "kept"),
        ("gone.txt", "gone"),
        ("same.txt", "same"),
    ] {
        std::fs::write(dir.path().join(name), text).unwrap();
    }
    let state = FaultState {
        dir: dir.path().to_path_buf(),
        cfg,
        open: true,
        flag: false,
        counter: 0,
        text: String::new(),
        asked_focus: false,
    };
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(size)
        .build_ui_state(
            |ui, s: &mut FaultState| {
                // the flag lives for the one frame that set it
                s.flag = false;
                let mut focused = true;
                if s.cfg.focus_dismisses {
                    let edit = ui
                        .text_edit_singleline(&mut s.text)
                        .accessible_name("Fault field");
                    if !s.asked_focus {
                        edit.request_focus();
                        s.asked_focus = true;
                    }
                    focused = edit.has_focus();
                }
                if s.open && focused {
                    ui.label("Keys fault screen");
                }
                if s.cfg.modal {
                    eframe::egui::Modal::new(eframe::egui::Id::unique("planted-dialog"))
                        .show(ui.ctx(), |ui| ui.label("Planted dialog"));
                }
                let (enter, escape, space) = ui.input(|i| {
                    (
                        i.key_pressed(Key::Enter),
                        i.key_pressed(Key::Escape),
                        i.key_pressed(Key::Space),
                    )
                });
                if enter && s.cfg.enter_writes {
                    std::fs::write(s.dir.join("new.txt"), "new").unwrap();
                    std::fs::write(s.dir.join("same.txt"), "same").unwrap();
                }
                if escape && s.cfg.escape_changes_and_closes {
                    std::fs::write(s.dir.join("kept.txt"), "changed").unwrap();
                    s.open = false;
                }
                if space {
                    match s.cfg.space {
                        SpaceDoes::Nothing => {}
                        SpaceDoes::DeletesAndFlashesFlag => {
                            std::fs::remove_file(s.dir.join("gone.txt")).unwrap();
                            s.flag = true;
                        }
                        SpaceDoes::CountsOnly => s.counter += 1,
                    }
                    if s.cfg.space_closes {
                        s.open = false;
                    }
                }
            },
            state,
        );
    (dir, h)
}

impl KeyFault {
    fn new(cfg: Faults, reopened: std::rc::Rc<std::cell::Cell<usize>>) -> Self {
        let (dir, h) = key_fault_harness(SIZES[1], cfg);
        KeyFault {
            dir,
            h,
            cfg,
            reopened,
            pressed: 0,
        }
    }
}

impl Entry for KeyFault {
    fn name(&self) -> &str {
        "test-only key-fault screen"
    }

    fn marker(&self) -> &'static [Needle] {
        KEY_FAULT_MARKER
    }

    fn at(&mut self, size: Vec2) -> Result<Snapshot, String> {
        self.h.set_size(size);
        self.h.run_steps(3);
        Ok(snapshot(&self.h))
    }

    fn dialog(&self) -> bool {
        self.cfg.is_dialog
    }

    fn takes_keys(&self) -> bool {
        true
    }

    fn watched(&self) -> Vec<(&'static str, PathBuf)> {
        let root = self.dir.path().to_path_buf();
        vec![(
            "fault folder",
            if self.cfg.unreadable_root {
                root.join("missing")
            } else {
                root
            },
        )]
    }

    fn arm(&mut self) -> Result<bool, String> {
        arm(&mut self.h, KEY_FAULT_MARKER, self.cfg.is_dialog)
    }

    fn press(&mut self, key: Key) -> Result<Pressed, String> {
        let n = self.pressed;
        self.pressed += 1;
        let times_out = self.cfg.times_out_at == Some(n);
        let stuck = self.cfg.running_at_start;
        strike(
            &mut self.h,
            key,
            &|s: &FaultState| s.flag || stuck,
            &|s: &FaultState| s.counter.to_string(),
            &mut |h| {
                h.run_steps(3);
                !times_out
            },
            KEY_FAULT_MARKER,
        )
    }

    fn reopen(&mut self, size: Vec2) -> Result<(), String> {
        self.reopened.set(self.reopened.get() + 1);
        let (dir, h) = key_fault_harness(size, self.cfg);
        (self.dir, self.h) = (dir, h);
        self.h.run_steps(3);
        Ok(())
    }
}

/// `KeyFault` alone through `walk`: what it collected and how often the walk reopened the screen.
fn walk_fault(cfg: Faults) -> (Walked, usize) {
    let reopened = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(KeyFault::new(cfg, reopened.clone()))];
    let walked = walk(&mut list, &mut real_checker);
    (walked, reopened.get())
}

/// The key step goes through `walk` and `verdict`, the code the shipped list goes through, beside a clean shipped entry.
/// The faulty entry's Enter writes a new file, its Escape changes a file (and closes the screen, so the walk reopens it
/// before Space), its Space deletes a file and flashes the export flag for the one frame that processes the key; Enter's
/// rewrite of a file with the same bytes is no change. The final failure list is exactly those four, at the middle size
/// only, and the checker ran on the six frames of the two entries and nothing else.
#[test]
fn a_key_that_writes_a_file_or_starts_an_export_fails_the_walk() {
    let reopened = std::rc::Rc::new(std::cell::Cell::new(0));
    let cfg = Faults {
        enter_writes: true,
        escape_changes_and_closes: true,
        space: SpaceDoes::DeletesAndFlashesFlag,
        ..NO_FAULTS
    };
    let mut list: Vec<Box<dyn Entry>> = vec![
        the_start_screen(),
        Box::new(KeyFault::new(cfg, reopened.clone())),
    ];
    let mut calls: Vec<(String, (f32, f32))> = vec![];
    let walked = walk(&mut list, &mut |screen, size, snap| {
        calls.push((screen.to_string(), (size.x, size.y)));
        real_checker(screen, size, snap)
    });
    let bad = verdict(&walked, &[], &[]);
    let mut want_calls = vec![];
    for name in ["start screen", "test-only key-fault screen"] {
        for size in SIZES {
            want_calls.push((name.to_string(), (size.x, size.y)));
        }
    }
    assert_eq!(calls, want_calls, "the checker ran on every frame");
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    let mut got: Vec<&str> = bad.iter().map(|m| head(m)).collect();
    got.sort();
    let mut want = vec![
        r#"test-only key-fault screen at 1280x800: keys_never_write "Enter: fault folder/new.txt (new file)" #0 (0.0 px)"#,
        r#"test-only key-fault screen at 1280x800: keys_never_write "Escape: fault folder/kept.txt (changed)" #0 (0.0 px)"#,
        r#"test-only key-fault screen at 1280x800: keys_never_write "Space: export running" #0 (0.0 px)"#,
        r#"test-only key-fault screen at 1280x800: keys_never_write "Space: fault folder/gone.txt (deleted)" #0 (0.0 px)"#,
    ];
    want.sort();
    assert_eq!(got, want, "{bad:#?}");
    assert_eq!(
        reopened.get(),
        1,
        "Escape closed the screen once and the walk reopened it"
    );
    let shown: Vec<(&str, &str, bool)> = walked
        .presses
        .iter()
        .filter(|p| p.screen == "test-only key-fault screen")
        .map(|p| (p.screen.as_str(), p.key, p.shown))
        .collect();
    assert_eq!(
        shown,
        [
            ("test-only key-fault screen", "Enter", true),
            ("test-only key-fault screen", "Escape", false),
            ("test-only key-fault screen", "Space", true),
        ]
    );
    assert!(
        bad.iter()
            .all(|m| m.contains("ALLOWED") && m.contains("nothing focused")),
        "every failure names the key and says what to do about it: {bad:#?}"
    );
}

/// An export that starts and ends inside one frame is never seen as running; the second witness (the export's record)
/// sees it. The flag is not what fails here, and the hit says what was seen.
#[test]
fn an_export_the_flag_never_shows_is_caught_by_its_record() {
    let (walked, _) = walk_fault(Faults {
        space: SpaceDoes::CountsOnly,
        ..NO_FAULTS
    });
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    let got: Vec<&str> = walked.hits.iter().map(|h| h.what.as_str()).collect();
    assert_eq!(got, ["Space: export record changed"]);
    assert!(walked.hits[0].message.contains("nothing focused"));
}

/// A key that closes the screen and is the last of the visit is not followed by a replay.
#[test]
fn no_replay_follows_the_last_key() {
    let (walked, reopened) = walk_fault(Faults {
        space_closes: true,
        ..NO_FAULTS
    });
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    assert!(walked.hits.is_empty(), "{:?}", walked.hits);
    assert_eq!(reopened, 0, "Space was the last key");
    let shown: Vec<bool> = walked.presses.iter().map(|p| p.shown).collect();
    assert_eq!(shown, [true, true, false]);
}

/// A screen that clearing the keyboard focus always dismisses is an error that says so; no key is pressed on the
/// window underneath.
#[test]
fn a_screen_that_clearing_focus_dismisses_is_an_error() {
    let (walked, reopened) = walk_fault(Faults {
        enter_writes: true,
        focus_dismisses: true,
        ..NO_FAULTS
    });
    assert_eq!(reopened, 1, "replayed once, dismissed again");
    assert_eq!(walked.errors.len(), 1, "{:?}", walked.errors);
    let e = &walked.errors[0];
    assert!(
        e.starts_with("test-only key-fault screen keys at 1280x800: before Enter: clearing the keyboard focus dismisses this screen"),
        "{e}"
    );
    assert!(walked.presses.is_empty() && walked.hits.is_empty());
}

/// A settle that times out is an error naming the screen and the key, and nothing is taken from it: not the write that
/// key made, and not the findings of the keys before it are lost.
#[test]
fn a_settle_timeout_is_an_error_and_earlier_findings_stay() {
    let (walked, _) = walk_fault(Faults {
        enter_writes: true,
        times_out_at: Some(0),
        ..NO_FAULTS
    });
    assert_eq!(walked.errors.len(), 1, "{:?}", walked.errors);
    assert!(
        walked.errors[0].starts_with("test-only key-fault screen keys at 1280x800: Enter: the app did not go idle after the key"),
        "{:?}",
        walked.errors
    );
    assert!(
        walked.hits.is_empty() && walked.presses.is_empty(),
        "nothing from a timed-out key"
    );
    // the second key times out: the first key's write is still reported
    let (walked, _) = walk_fault(Faults {
        enter_writes: true,
        times_out_at: Some(1),
        ..NO_FAULTS
    });
    assert_eq!(walked.errors.len(), 1, "{:?}", walked.errors);
    assert!(walked.errors[0].contains(": Escape: the app did not go idle"));
    let got: Vec<&str> = walked.hits.iter().map(|h| h.what.as_str()).collect();
    assert_eq!(got, ["Enter: fault folder/new.txt (new file)"]);
}

/// A modal dialog open over a screen that is no dialog is an error naming the dialog, at every size and before the
/// first key, although the screen's marker still holds underneath: no visit and no key is taken from such a frame. The
/// same dialog on an entry that says it is a dialog screen is its screen.
#[test]
fn a_modal_dialog_over_a_screen_that_is_no_dialog_is_an_error() {
    let (walked, _) = walk_fault(Faults {
        modal: true,
        ..NO_FAULTS
    });
    let mut want: Vec<String> = WIDTHS
        .iter()
        .map(|size| format!("test-only key-fault screen at {size}: a modal dialog"))
        .collect();
    want.push("test-only key-fault screen keys at 1280x800: before Enter: a modal dialog".into());
    assert_eq!(walked.errors.len(), want.len(), "{:?}", walked.errors);
    for (e, start) in walked.errors.iter().zip(&want) {
        assert!(
            e.starts_with(start.as_str()) && e.contains("\"Planted dialog\""),
            "{e}"
        );
    }
    assert!(walked.visits.is_empty() && walked.presses.is_empty() && walked.hits.is_empty());
    let (walked, _) = walk_fault(Faults {
        modal: true,
        is_dialog: true,
        ..NO_FAULTS
    });
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    assert_eq!((walked.visits.len(), walked.presses.len()), (3, 3));
}

/// No idle baseline is an error, not a suppressed finding.
#[test]
fn an_export_running_before_the_key_is_an_error() {
    let (walked, _) = walk_fault(Faults {
        running_at_start: true,
        ..NO_FAULTS
    });
    assert_eq!(walked.errors.len(), 1, "{:?}", walked.errors);
    assert!(
        walked.errors[0].contains("an export is already running before the key"),
        "{:?}",
        walked.errors
    );
    assert!(walked.hits.is_empty());
}

/// A folder that cannot be listed fails the visit with its name and path, and compares nothing.
#[test]
fn a_folder_that_cannot_be_listed_is_an_error() {
    let (walked, _) = walk_fault(Faults {
        unreadable_root: true,
        ..NO_FAULTS
    });
    assert_eq!(walked.errors.len(), 1, "{:?}", walked.errors);
    assert!(
        walked.errors[0].contains("cannot list")
            && walked.errors[0].contains("missing")
            && walked.errors[0].contains("fault folder"),
        "{:?}",
        walked.errors
    );
    assert!(walked.hits.is_empty() && walked.presses.is_empty());
}

/// A file that cannot be read is an error naming the path (a read failure is injected: the OS gives a temp folder no
/// unreadable file without admin rights), never a stand-in hash that two failed reads would share.
#[test]
fn a_file_that_cannot_be_read_is_an_error() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a.txt"), "one").unwrap();
    std::fs::write(d.path().join("b.txt"), "two").unwrap();
    let roots = [("root", d.path().to_path_buf())];
    assert!(scan(&roots, &|p| std::fs::read(p)).is_ok());
    let e = scan(&roots, &|p| {
        if p.ends_with("b.txt") {
            Err(std::io::Error::other("sharing violation"))
        } else {
            std::fs::read(p)
        }
    })
    .unwrap_err();
    assert!(
        e.contains("cannot read")
            && e.contains("b.txt")
            && e.contains("root folder")
            && e.contains("sharing violation"),
        "{e}"
    );
}

/// The hash counts content and paths: a file rewritten with the same bytes (its modification time moves) is no change; a
/// file with other bytes, a new file, a new folder and a deleted file or folder each are, and each is named by its path.
#[test]
fn a_tree_differs_by_content_and_paths_not_by_modification_time() {
    let d = tempfile::tempdir().unwrap();
    let at = |name: &str| d.path().join(name);
    std::fs::write(at("a.txt"), "one").unwrap();
    std::fs::write(at("b.txt"), "two").unwrap();
    std::fs::create_dir(at("old")).unwrap();
    let tree = || tree_of(&[("root", d.path().to_path_buf())]).unwrap();
    let first = tree();
    assert_eq!(first.len(), 3, "{first:?}");
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(at("a.txt"), "one").unwrap();
    assert!(changes(&first, &tree()).is_empty(), "same bytes");
    std::fs::write(at("a.txt"), "other").unwrap();
    std::fs::remove_file(at("b.txt")).unwrap();
    std::fs::remove_dir(at("old")).unwrap();
    std::fs::write(at("c.txt"), "three").unwrap();
    std::fs::create_dir(at("new")).unwrap();
    let mut got = changes(&first, &tree());
    got.sort();
    let want: Vec<(String, &str)> = [
        ("root/a.txt", "changed"),
        ("root/b.txt", "deleted"),
        ("root/c.txt", "new file"),
        ("root/new", "new folder"),
        ("root/old", "deleted folder"),
    ]
    .map(|(p, k)| (p.to_string(), k))
    .to_vec();
    assert_eq!(got, want);
}

/// Two files whose names print alike (each with a code unit the lossy text form replaces by U+FFFD) are two entries of
/// the tree, and a change to one is that one change: the native path is the key, the text only the message.
#[test]
fn two_names_that_print_alike_are_two_paths() {
    #[cfg(windows)]
    let names: [std::ffi::OsString; 2] = {
        use std::os::windows::ffi::OsStringExt as _;
        [
            std::ffi::OsString::from_wide(&[0x61, 0xD800]),
            std::ffi::OsString::from_wide(&[0x61, 0xD801]),
        ]
    };
    #[cfg(unix)]
    let names: [std::ffi::OsString; 2] = {
        use std::os::unix::ffi::OsStringExt as _;
        [
            std::ffi::OsString::from_vec(b"a\xfe".to_vec()),
            std::ffi::OsString::from_vec(b"a\xff".to_vec()),
        ]
    };
    assert_eq!(
        names[0].to_string_lossy(),
        names[1].to_string_lossy(),
        "the fixture: the two names print alike"
    );
    let d = tempfile::tempdir().unwrap();
    for n in &names {
        std::fs::write(d.path().join(n), "one").unwrap();
    }
    let roots = [("root", d.path().to_path_buf())];
    let first = tree_of(&roots).unwrap();
    assert_eq!(first.len(), 2, "{first:?}");
    std::fs::write(d.path().join(&names[1]), "two").unwrap();
    let got = changes(&first, &tree_of(&roots).unwrap());
    assert_eq!(got, [("root/a\u{FFFD}".to_string(), "changed")]);
}

/// The test-only screens of the scroll-bar and status-dot invariants: tiny UIs with one planted violation each (and a
/// clean control), none in the shipped list.
#[derive(Clone, Copy)]
enum Planted {
    /// A floating bar (egui's default style) that paints only transparent fills at rest.
    HiddenBar,
    /// The hidden bar with an opaque decoy in its strip that a clip cuts away.
    ClippedDecoy,
    /// The hidden bar with an opaque decoy in its strip painted by another layer.
    LayerDecoy,
    /// The hidden bar with an opaque decoy in its strip, same layer and in view, beside the bar's own paint: which paint
    /// is the bar's cannot be told.
    SameLayerDecoy,
    /// A thin bar with a button as wide as the area, so the bar lies over the button's right end.
    BarOverButton,
    /// A thin bar and a narrow button: nothing to find.
    CleanBar,
    /// The clean screen with the pointer in the window.
    PointerOverBar,
    /// Status circles: red with no node, green with no node, a named green dot, amber over a whole named row (passes),
    /// and amber over an unnamed button with a small named label of another layer over it.
    Dots,
    /// A thin bar over very tall content, so its handle is the shortest egui draws (12 px): a clean bar.
    ShortHandle,
    /// A floating (transparent) bar over very tall content, with an opaque same-layer shape outside the bar's strip
    /// where the handle would be: the bar is invisible.
    ShortHandleDecoy,
    /// A thin bar in the Sets panel with a plain button named Set up now as wide as the area: no set card, so it is held
    /// to the hovered width like any control.
    SetUpButton,
    /// A status circle painted in a layer egui moves by a transform: an error, never a judgement.
    TransformedLayer,
    /// An amber circle on the bottom edge of a named label, clipped to the label: the label holds the part that shows
    /// and not the whole circle, so it is no name for the dot.
    ClippedDot,
    /// Two amber/red circles over a visible unnamed button and a visible named label, each with a deeper node of the
    /// other kind clipped to the corner of the circle's bounding box outside the disc: only what shows names the dot.
    CornerClip,
    /// A thin bar with a button as wide as the area whose clip ends well before the bar's reserve: nothing to find.
    ClippedControl,
    /// A red circle with no node, painted with a clip on the corner of its bounding square outside the disc: nothing of
    /// it shows, so nothing to find.
    CornerDisc,
}

struct PlantedScreen(Planted);

impl PlantedScreen {
    fn ui(kind: Planted, ui: &mut eframe::egui::Ui) {
        use eframe::egui::{pos2, Area, Button, Id, Sense};
        use sdc_gui::ui::status::{dot, Status, AMBER, GREEN, RED};
        let title = match kind {
            Planted::HiddenBar => "Hidden bar screen",
            Planted::ClippedDecoy => "Clipped decoy screen",
            Planted::LayerDecoy => "Layer decoy screen",
            Planted::SameLayerDecoy => "Same layer decoy screen",
            Planted::BarOverButton => "Bar over button screen",
            Planted::CleanBar => "Clean bar screen",
            Planted::PointerOverBar => "Pointer screen",
            Planted::Dots => "Status dots screen",
            Planted::ShortHandle => "Short handle screen",
            Planted::ShortHandleDecoy => "Short handle decoy screen",
            Planted::SetUpButton => "Set up screen",
            Planted::TransformedLayer => "Transformed layer screen",
            Planted::ClippedDot => "Clipped dot screen",
            Planted::CornerClip => "Corner clip screen",
            Planted::ClippedControl => "Clipped control screen",
            Planted::CornerDisc => "Corner disc screen",
        };
        ui.label(title);
        if let Planted::CornerDisc = kind {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            let c = r.center();
            let corner = eframe::egui::Rect::from_min_max(c + vec2(3.0, 3.0), c + vec2(4.0, 4.0));
            ui.painter()
                .with_clip_rect(corner)
                .circle_filled(c, 4.0, RED);
            return;
        }
        if let Planted::Dots = kind {
            let (red, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(red.center(), 4.0, RED);
            let (green, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(green.center(), 4.0, GREEN);
            dot(ui, Status::Ready);
            let row = ui.label("a whole row that names itself and its status");
            ui.painter()
                .circle_filled(row.rect.left_center() + vec2(20.0, 0.0), 4.0, AMBER);
            let button = ui
                .add(Button::new("").min_size(vec2(24.0, 24.0)))
                .accessible_name(NAMELESS);
            let centre = button.rect.center();
            ui.painter().circle_filled(centre, 4.0, AMBER);
            Area::new(Id::unique("small named label"))
                .fixed_pos(centre - vec2(6.0, 6.0))
                .show(ui.ctx(), |ui| {
                    // nested, so that its node is deeper than the button's: only the layer rule keeps it out
                    ui.scope(|ui| {
                        ui.scope(|ui| {
                            ui.scope(|ui| {
                                ui.add_sized(vec2(12.0, 12.0), eframe::egui::Label::new("ok"))
                            })
                        })
                    })
                });
            return;
        }
        if let Planted::CornerClip = kind {
            use eframe::egui::{Label, Rect, UiBuilder};
            // a deeper node of `named` kind over `rect`, clipped to the corner of the circle's bounding box (outside the disc)
            let corner_node = |ui: &mut eframe::egui::Ui, rect: Rect, named: bool| {
                let c = rect.center();
                let corner = Rect::from_min_max(c + vec2(3.0, 3.0), c + vec2(4.0, 4.0));
                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                    ui.scope(|ui| {
                        ui.scope(|ui| {
                            ui.scope(|ui| {
                                ui.set_clip_rect(corner);
                                if named {
                                    ui.add_sized(rect.size(), Label::new("corner"));
                                } else {
                                    ui.add_sized(rect.size(), Button::new(""))
                                        .accessible_name(NAMELESS);
                                }
                            })
                        })
                    })
                });
            };
            // an unnamed button shows over the amber circle; the named node is clipped to the corner: a hit
            let button = ui
                .add(Button::new("").min_size(vec2(24.0, 24.0)))
                .accessible_name(NAMELESS);
            ui.painter().circle_filled(button.rect.center(), 4.0, AMBER);
            corner_node(ui, button.rect, true);
            // a named row shows over the red circle; the unnamed node is clipped to the corner: no hit
            let row = ui.add_sized(vec2(60.0, 24.0), Label::new("a named row"));
            ui.painter().circle_filled(row.rect.center(), 4.0, RED);
            corner_node(ui, row.rect, false);
            return;
        }
        if let Planted::ClippedDot = kind {
            let row = ui.label("a label that holds only the part of the dot that shows");
            ui.painter().with_clip_rect(row.rect).circle_filled(
                row.rect.left_bottom() + vec2(20.0, 0.0),
                4.0,
                AMBER,
            );
            return;
        }
        if let Planted::TransformedLayer = kind {
            Area::new(Id::unique("moved"))
                .fixed_pos(pos2(100.0, 100.0))
                .show(ui.ctx(), |ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, RED);
                    ui.ctx().set_transform_layer(
                        ui.layer_id(),
                        eframe::egui::emath::TSTransform::from_translation(vec2(40.0, 30.0)),
                    );
                });
            return;
        }
        // the bar lives in a named panel (a bar has no name of its own); the Set up screen in the Sets panel
        let panel = if let Planted::SetUpButton = kind {
            "sets"
        } else {
            "textures"
        };
        sdc_gui::ui::panel_ids::left(ui, panel)
            .exact_size(300.0)
            .show(ui, |ui| Self::bar(kind, ui));
    }

    fn bar(kind: Planted, ui: &mut eframe::egui::Ui) {
        use eframe::egui::{pos2, Area, Button, Color32, Id, Rect, ScrollArea, Sense};
        if !matches!(
            kind,
            Planted::HiddenBar
                | Planted::ClippedDecoy
                | Planted::LayerDecoy
                | Planted::SameLayerDecoy
                | Planted::ShortHandleDecoy
        ) {
            ui.style_mut().spacing.scroll = sdc_gui::look::thin_scroll();
        }
        if let Planted::ShortHandle = kind {
            // a transparent track with an opaque handle: only the handle (12 px at the shortest) shows the bar
            let scroll = &mut ui.style_mut().spacing.scroll;
            scroll.dormant_background_opacity = 0.0;
            scroll.dormant_handle_opacity = 1.0;
        }
        let tall = if matches!(kind, Planted::ShortHandle | Planted::ShortHandleDecoy) {
            6000.0
        } else {
            600.0
        };
        ScrollArea::vertical()
            .id_salt("planted")
            .max_height(150.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let width = if matches!(kind, Planted::BarOverButton | Planted::SetUpButton) {
                    ui.available_width()
                } else {
                    100.0
                };
                let text = if let Planted::SetUpButton = kind {
                    "Set up now"
                } else {
                    "A control"
                };
                if let Planted::ClippedControl = kind {
                    // as wide as the area, shown only on its left 100 px
                    let full = ui.available_width();
                    ui.scope(|ui| {
                        let mut clip = ui.clip_rect();
                        clip.max.x = ui.max_rect().left() + 100.0;
                        ui.set_clip_rect(clip);
                        ui.add_sized(vec2(full, 20.0), Button::new(text));
                    });
                } else {
                    ui.add_sized(vec2(width, 20.0), Button::new(text));
                }
                ui.allocate_space(vec2(10.0, tall));
            });
        // an opaque rect in the bar's strip, 2 px wide, that is not the bar's
        let right = ui.max_rect().right();
        let decoy = |at: f32| Rect::from_min_size(pos2(right - at, 40.0), vec2(2.0, 100.0));
        match kind {
            // where a short handle would be, 30 px left of the strip
            Planted::ShortHandleDecoy => {
                let short = Rect::from_min_size(pos2(right - 30.0, 30.0), vec2(2.0, 12.0));
                ui.painter().rect_filled(short, 0.0, Color32::WHITE);
            }
            Planted::ClippedDecoy => {
                let cut = Rect::from_min_size(pos2(0.0, 0.0), vec2(5.0, 5.0));
                ui.painter()
                    .with_clip_rect(cut)
                    .rect_filled(decoy(3.0), 0.0, Color32::WHITE);
            }
            Planted::LayerDecoy => {
                Area::new(Id::unique("decoy"))
                    .fixed_pos(decoy(3.0).min)
                    .show(ui.ctx(), |ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(2.0, 100.0), Sense::hover());
                        ui.painter().rect_filled(r, 0.0, Color32::WHITE);
                    });
            }
            Planted::SameLayerDecoy => {
                ui.painter().rect_filled(decoy(8.0), 0.0, Color32::WHITE);
            }
            _ => {}
        }
    }
}

impl Entry for PlantedScreen {
    fn name(&self) -> &str {
        match self.0 {
            Planted::HiddenBar => "test-only hidden bar",
            Planted::ClippedDecoy => "test-only hidden bar with a clipped decoy",
            Planted::LayerDecoy => "test-only hidden bar with a decoy in another layer",
            Planted::SameLayerDecoy => "test-only hidden bar with a decoy in its layer",
            Planted::BarOverButton => "test-only bar over a button",
            Planted::CleanBar => "test-only clean bar",
            Planted::PointerOverBar => "test-only bar with a pointer",
            Planted::Dots => "test-only status dots",
            Planted::ShortHandle => "test-only short handle",
            Planted::ShortHandleDecoy => "test-only transparent short handle with a decoy",
            Planted::SetUpButton => "test-only set up button",
            Planted::TransformedLayer => "test-only transformed layer",
            Planted::ClippedDot => "test-only clipped dot",
            Planted::CornerClip => "test-only corner clip",
            Planted::ClippedControl => "test-only clipped control",
            Planted::CornerDisc => "test-only disc clipped to its corner",
        }
    }

    fn marker(&self) -> &'static [Needle] {
        const HIDDEN: &[Needle] = &[crate::screens::has("Hidden bar screen")];
        const CLIPPED: &[Needle] = &[crate::screens::has("Clipped decoy screen")];
        const LAYER: &[Needle] = &[crate::screens::has("Layer decoy screen")];
        const SAME: &[Needle] = &[crate::screens::has("Same layer decoy screen")];
        const OVER: &[Needle] = &[crate::screens::has("Bar over button screen")];
        const CLEAN: &[Needle] = &[crate::screens::has("Clean bar screen")];
        const POINTER: &[Needle] = &[crate::screens::has("Pointer screen")];
        const DOTS: &[Needle] = &[crate::screens::has("Status dots screen")];
        const SHORT: &[Needle] = &[crate::screens::has("Short handle screen")];
        const SHORT_DECOY: &[Needle] = &[crate::screens::has("Short handle decoy screen")];
        const SET_UP: &[Needle] = &[crate::screens::has("Set up screen")];
        const MOVED: &[Needle] = &[crate::screens::has("Transformed layer screen")];
        const CLIPPED_DOT: &[Needle] = &[crate::screens::has("Clipped dot screen")];
        const CORNER: &[Needle] = &[crate::screens::has("Corner clip screen")];
        const CLIPPED_CONTROL: &[Needle] = &[crate::screens::has("Clipped control screen")];
        const CORNER_DISC: &[Needle] = &[crate::screens::has("Corner disc screen")];
        match self.0 {
            Planted::HiddenBar => HIDDEN,
            Planted::ClippedDecoy => CLIPPED,
            Planted::LayerDecoy => LAYER,
            Planted::SameLayerDecoy => SAME,
            Planted::BarOverButton => OVER,
            Planted::CleanBar => CLEAN,
            Planted::PointerOverBar => POINTER,
            Planted::Dots => DOTS,
            Planted::ShortHandle => SHORT,
            Planted::ShortHandleDecoy => SHORT_DECOY,
            Planted::SetUpButton => SET_UP,
            Planted::TransformedLayer => MOVED,
            Planted::ClippedDot => CLIPPED_DOT,
            Planted::CornerClip => CORNER,
            Planted::ClippedControl => CLIPPED_CONTROL,
            Planted::CornerDisc => CORNER_DISC,
        }
    }

    fn at(&mut self, size: Vec2) -> Result<Snapshot, String> {
        let kind = self.0;
        let mut h = Harness::builder()
            .with_context_setup(|ctx| {
                sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
                crate::frame::track_paint(ctx);
            })
            .with_size(size)
            .build_ui(move |ui| PlantedScreen::ui(kind, ui));
        h.run_steps(3);
        if let Planted::PointerOverBar = kind {
            h.event(eframe::egui::Event::PointerMoved(eframe::egui::pos2(
                50.0, 50.0,
            )));
            h.run_steps(3);
        }
        let mut snap = snapshot(&h);
        // The unnamed buttons are named in the UI, so kittest's own name check stays on, and lose that name here, in the
        // frame the checker reads.
        for n in snap.nodes.iter_mut().filter(|n| n.label == NAMELESS) {
            n.label.clear();
        }
        Ok(snap)
    }
}

/// The name a planted button carries in the UI and loses in its captured frame (`PlantedScreen::at`): an unnamed
/// button as the checker sees it, while kittest's own name check runs on every frame.
const NAMELESS: &str = "planted button whose name the frame drops";

const WIDTHS: [&str; 3] = ["1100x800", "1280x800", "1600x900"];

/// A scroll bar that is invisible at rest (also with an opaque decoy beside it that a clip cuts away, or that another
/// layer paints), an interactive widget inside a bar's width of the track, and status circles with no named node of their
/// own each fail the walk, through `walk` and `verdict` beside a clean shipped entry and a clean planted bar. The
/// checker's own calls are the 21 frames of the seven entries, and the failure list is exactly the planted ones at every
/// size: a decoy or a larger named node cannot stand in for the bar's paint or the dot's name.
#[test]
fn a_hidden_bar_a_widget_under_a_bar_and_a_nameless_dot_fail_the_walk() {
    let kinds = [
        Planted::HiddenBar,
        Planted::ClippedDecoy,
        Planted::LayerDecoy,
        Planted::BarOverButton,
        Planted::CleanBar,
        Planted::Dots,
        Planted::ShortHandle,
        Planted::ShortHandleDecoy,
        Planted::SetUpButton,
    ];
    let mut list: Vec<Box<dyn Entry>> = vec![the_start_screen()];
    list.extend(kinds.map(|k| Box::new(PlantedScreen(k)) as Box<dyn Entry>));
    let mut calls: Vec<(String, (f32, f32))> = vec![];
    let walked = walk(&mut list, &mut |screen, size, snap| {
        calls.push((screen.to_string(), (size.x, size.y)));
        real_checker(screen, size, snap)
    });
    let bad = verdict(&walked, &[], &[]);
    let mut want_calls = vec![];
    for e in &list {
        for size in SIZES {
            want_calls.push((e.name().to_string(), (size.x, size.y)));
        }
    }
    assert_eq!(want_calls.len(), 30);
    assert_eq!(calls, want_calls, "the checker ran on every frame");
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    let what = format!("{:?}", "scroll bar 1 of 1 in the textures panel");
    let sets = format!("{:?}", "scroll bar 1 of 1 in the sets panel");
    let mut got: Vec<&str> = bad.iter().map(|m| head(m)).collect();
    got.sort();
    let mut want = vec![];
    for size in WIDTHS {
        for screen in [
            "test-only hidden bar",
            "test-only hidden bar with a clipped decoy",
            "test-only hidden bar with a decoy in another layer",
            "test-only transparent short handle with a decoy",
        ] {
            want.push(format!(
                r#"{screen} at {size}: scroll_bar_at_rest {what} #0 (0.0 px)"#
            ));
        }
        want.push(format!(
            r#"test-only bar over a button at {size}: scroll_bar_clearance {what} #0 (4.0 px)"#
        ));
        want.push(format!(
            r#"test-only set up button at {size}: scroll_bar_clearance {sets} #0 (4.0 px)"#
        ));
        for dot in ["amber", "green", "red"] {
            want.push(format!(
                r#"test-only status dots at {size}: status_dot_named "{dot} dot" #0 (0.0 px)"#
            ));
        }
    }
    want.sort();
    assert_eq!(got, want, "{bad:#?}");
    // the clearance hit names its one widget with its own px
    let over: Vec<_> = walked
        .hits
        .iter()
        .filter(|h| h.invariant == "scroll_bar_clearance")
        .collect();
    assert_eq!(over.len(), 6);
    for h in over {
        assert_eq!(h.members.len(), 1);
        let name = if h.screen == "test-only set up button" {
            "Button:Set up now"
        } else {
            "Button:A control"
        };
        assert_eq!(h.members[0].0, name);
        assert!((h.members[0].1 - 4.0).abs() < 0.2, "{:?}", h.members);
    }
    // every failure names where to go: ALLOWED, and what to do instead
    for m in &bad {
        assert!(m.contains("ALLOWED"), "{m}");
        let what = if m.contains("scroll_bar_at_rest") {
            "look::thin_scroll()"
        } else if m.contains("scroll_bar_clearance") {
            "right-edge reserve"
        } else {
            "ui::status::dot"
        };
        assert!(m.contains(what), "{m}");
    }
}

/// The dots the walk judges, one by one: the whole named row passes, the named dot passes, and the unnamed circles
/// fail whatever else is near; the failures say what is under the circle.
#[test]
fn a_dot_is_named_by_the_innermost_node_of_its_own_layer() {
    let snap = PlantedScreen(Planted::Dots).at(SIZES[1]).unwrap();
    let mut errors = vec![];
    let hits = status_dots_have_names(
        &Frame {
            screen: "s",
            size: SIZES[1],
            snapshot: &snap,
            findings: &[],
        },
        &mut errors,
    );
    assert!(errors.is_empty(), "{errors:?}");
    let mut got: Vec<(&str, bool, bool)> = hits
        .iter()
        .map(|h| {
            (
                h.what.as_str(),
                h.message.contains("no node of its layer"),
                h.message.contains("the Button \"\""),
            )
        })
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            ("amber dot", false, true),
            ("green dot", true, false),
            ("red dot", true, false)
        ],
        "{hits:#?}"
    );
    // five status circles are painted: two pass (the named dot and the amber one over the named row)
    let status = snap
        .discs
        .iter()
        .filter(|d| d.fill.a() == 255 && d.radius == 4.0)
        .count();
    assert_eq!(status, 5, "{:?}", snap.discs);
}

/// A named node that holds only the part of a circle a clip shows is no name for the dot: the node must hold the whole
/// circle (its rect is the widget's full rect, so a circle that a clip cuts part of is judged like any other).
#[test]
fn a_node_holding_only_the_visible_part_of_a_dot_does_not_name_it() {
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(PlantedScreen(Planted::ClippedDot))];
    let walked = walk(&mut list, &mut real_checker);
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    let got: Vec<(&str, &str)> = walked
        .hits
        .iter()
        .map(|h| (h.what.as_str(), h.invariant))
        .collect();
    assert_eq!(
        got,
        [("amber dot", "status_dot_named"); 3],
        "{:?}",
        walked.hits
    );
}

/// A node names the dot only where it is shown over the disc (positive area): a deeper named node clipped to the corner
/// of the circle's bounding box does not (the unnamed node that shows is a hit), and a deeper unnamed one clipped there
/// does not take the name from a named node that shows. A node whose clip the snapshot lost, and whose count would change
/// the winner, is an error.
#[test]
fn a_node_clipped_to_the_corner_of_the_bounds_does_not_decide_a_dot() {
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(PlantedScreen(Planted::CornerClip))];
    let walked = walk(&mut list, &mut real_checker);
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    // (the label clipped to a corner is also a clipped text, by construction: that is another invariant's hit)
    let got: Vec<(&str, &str)> = walked
        .hits
        .iter()
        .filter(|h| h.invariant == "status_dot_named")
        .map(|h| (h.what.as_str(), h.invariant))
        .collect();
    assert_eq!(
        got,
        [("amber dot", "status_dot_named"); 3],
        "{:?}",
        walked.hits
    );
    let snap = PlantedScreen(Planted::CornerClip).at(SIZES[1]).unwrap();
    let observe = |snap: &Snapshot| {
        let mut errors = vec![];
        let hits = status_dots_have_names(
            &Frame {
                screen: "s",
                size: SIZES[1],
                snapshot: snap,
                findings: &[],
            },
            &mut errors,
        );
        (hits.len(), errors)
    };
    assert_eq!(observe(&snap), (1, vec![]));
    let mut lost = snap.clone();
    for n in lost.nodes.iter_mut().filter(|n| n.label == "corner") {
        n.clip = None;
    }
    let (hits, errors) = observe(&lost);
    assert_eq!(hits, 0);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains("whose clip the snapshot does not know"),
        "{errors:?}"
    );
}

/// What is judged is what is shown: a control whose clip ends before a bar's reserve is clear of the bar, and a status
/// circle whose clip holds only a corner of its bounding square (outside the disc) shows nothing; neither is a hit or an
/// error. The same frames with the full rect (the clip unknown) or the whole square shown are hits.
#[test]
fn a_control_or_a_dot_its_clip_hides_is_not_judged() {
    let clipped = PlantedScreen(Planted::ClippedControl).at(SIZES[1]).unwrap();
    let bar = clipped
        .nodes
        .iter()
        .find(|n| n.role == "ScrollBar")
        .unwrap()
        .rect;
    let control = clipped
        .nodes
        .iter()
        .find(|n| n.label == "A control")
        .unwrap();
    let reserve = bar.min.x - sdc_gui::look::thin_scroll().bar_width;
    assert!(
        control.rect.max.x > reserve && control.clip.is_some_and(|c| c.max.x < reserve - 10.0),
        "the fixture: the control's rect reaches the reserve and its clip ends before it: {control:?} {bar:?}"
    );
    let (hits, errors) = observe_bars(&clipped);
    assert!(hits.is_empty() && errors.is_empty(), "{hits:?} {errors:?}");
    let mut full = clipped.clone();
    for n in full.nodes.iter_mut().filter(|n| n.label == "A control") {
        n.clip = None;
    }
    let (hits, errors) = observe_bars(&full);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        hits.iter().map(|h| h.invariant).collect::<Vec<_>>(),
        ["scroll_bar_clearance"]
    );
    let observe_dots = |snap: &Snapshot| {
        let mut errors = vec![];
        let hits = status_dots_have_names(
            &Frame {
                screen: "s",
                size: SIZES[1],
                snapshot: snap,
                findings: &[],
            },
            &mut errors,
        );
        (hits.len(), errors)
    };
    let corner = PlantedScreen(Planted::CornerDisc).at(SIZES[1]).unwrap();
    let red: Vec<usize> = (0..corner.discs.len())
        .filter(|&i| corner.discs[i].radius == 4.0 && corner.discs[i].fill.a() == 255)
        .collect();
    assert_eq!(red.len(), 1, "{:?}", corner.discs);
    let d = &corner.discs[red[0]];
    let square = Rect::from_center_size(d.center, Vec2::splat(2.0 * d.radius));
    assert!(
        square.intersect(d.clip).is_positive(),
        "the fixture: the clip holds a corner of the square: {d:?}"
    );
    assert_eq!(observe_dots(&corner), (0, vec![]));
    let mut whole = corner.clone();
    whole.discs[red[0]].clip = square;
    assert_eq!(observe_dots(&whole), (1, vec![]));
}

/// A node whose clip the snapshot does not know keeps its place among the candidates: two nodes of one depth and area
/// over a dot, the unknown one first, tie the same way with and without it (the later one names the dot), so the dot is
/// judged, not an error.
#[test]
fn an_unknown_clip_keeps_its_place_among_the_candidates() {
    let mut snap = PlantedScreen(Planted::CornerDisc).at(SIZES[1]).unwrap();
    let i = (0..snap.discs.len())
        .find(|&i| snap.discs[i].radius == 4.0 && snap.discs[i].fill.a() == 255)
        .unwrap();
    let d = &mut snap.discs[i];
    d.clip = Rect::EVERYTHING;
    let (centre, layer) = (d.center, d.layer.clone());
    let mut node = snap.nodes.last().unwrap().clone();
    node.rect = Rect::from_center_size(centre, Vec2::splat(20.0));
    node.layer = layer;
    node.depth = 50;
    let mut unknown = node.clone();
    unknown.label = String::new();
    unknown.clip = None;
    node.label = "named".into();
    node.clip = Some(node.rect);
    snap.nodes.extend([unknown, node]);
    let mut errors = vec![];
    let hits = status_dots_have_names(
        &Frame {
            screen: "s",
            size: SIZES[1],
            snapshot: &snap,
            findings: &[],
        },
        &mut errors,
    );
    assert!(hits.is_empty() && errors.is_empty(), "{hits:?} {errors:?}");
}

/// A frame with the pointer in the window is an error of its screen, not a pass: the bar's opacity at rest cannot be
/// read from it.
#[test]
fn a_bar_read_with_the_pointer_in_the_window_is_an_error() {
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(PlantedScreen(Planted::PointerOverBar))];
    let walked = walk(&mut list, &mut real_checker);
    assert!(walked.hits.is_empty(), "{:?}", walked.hits);
    assert_eq!(walked.errors.len(), 3, "{:?}", walked.errors);
    for (e, size) in walked.errors.iter().zip(WIDTHS) {
        assert!(
            e.starts_with(&format!(
                "test-only bar with a pointer at {size}: the pointer is at"
            )),
            "{e}"
        );
    }
}

/// Paint that cannot be attributed to one bar is an error naming the screen and the width, never a pass: a visible decoy
/// in the bar's own layer beside its transparent paint.
#[test]
fn paint_that_cannot_be_told_from_the_bars_is_an_error() {
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(PlantedScreen(Planted::SameLayerDecoy))];
    let walked = walk(&mut list, &mut real_checker);
    assert!(walked.hits.is_empty(), "{:?}", walked.hits);
    assert_eq!(walked.errors.len(), 3, "{:?}", walked.errors);
    for (e, size) in walked.errors.iter().zip(WIDTHS) {
        assert!(
            e.starts_with(&format!(
                "test-only hidden bar with a decoy in its layer at {size}: scroll bar 1 of 1 in the textures panel"
            )) && e.contains("paint that is not one track with at most one nested handle"),
            "{e}"
        );
    }
}

/// What the bar check cannot see is an error, not a pass: no paint of the bar's own layer in view, a frame without
/// paint layers, an overlapping interactive node with no layer. A frame with no bar node reports nothing.
#[test]
fn what_the_bar_check_cannot_attribute_is_an_error() {
    let size = SIZES[1];
    let observe = |snap: &Snapshot| {
        let mut errors = vec![];
        let hits = scroll_bars_are_visible_and_clear(
            &Frame {
                screen: "s",
                size,
                snapshot: snap,
                findings: &[],
            },
            &mut errors,
        );
        (hits.len(), errors)
    };
    let clean = PlantedScreen(Planted::CleanBar).at(size).unwrap();
    assert_eq!(observe(&clean), (0, vec![]), "the clean bar passes");
    let mut no_paint = clean.clone();
    no_paint.filled.clear();
    let (hits, errors) = observe(&no_paint);
    assert_eq!(hits, 0);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains("no filled rect of its own layer"),
        "{errors:?}"
    );
    let mut no_layers = clean.clone();
    no_layers.paint_layers_known = false;
    let (hits, errors) = observe(&no_layers);
    assert_eq!(hits, 0);
    assert!(errors[0].contains("has no layers"), "{errors:?}");
    let mut no_bar = clean.clone();
    no_bar.nodes.retain(|n| n.role != "ScrollBar");
    assert_eq!(
        observe(&no_bar),
        (0, vec![]),
        "no bar node, nothing to observe"
    );
    // an interactive node that overlaps the reserve and has no layer: an error when the node overlaps, a hit when its
    // layer is the bar's
    let over = PlantedScreen(Planted::BarOverButton).at(size).unwrap();
    let (hits, errors) = observe(&over);
    assert_eq!((hits, errors.len()), (1, 0), "{errors:?}");
    let mut unknown = over.clone();
    for n in unknown.nodes.iter_mut().filter(|n| n.label == "A control") {
        n.layer = None;
    }
    let (hits, errors) = observe(&unknown);
    assert_eq!(hits, 0);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("has no egui layer"), "{errors:?}");
    // the same node in another layer is a floating one, not an error
    let mut floating = over.clone();
    for n in floating.nodes.iter_mut().filter(|n| n.label == "A control") {
        n.layer = Some("LayerId { Foreground Id::unique(\"menu\") }".into());
    }
    assert_eq!(observe(&floating), (0, vec![]));
}

/// The set-card exception is for a "Set ..." button inside the Sets column's panel only: the same button anywhere else,
/// or another role with the name, is held to the hovered width.
#[test]
fn only_a_set_card_may_stop_at_the_track() {
    let size = SIZES[1];
    let observe = |snap: &Snapshot| {
        let mut errors = vec![];
        let hits = scroll_bars_are_visible_and_clear(
            &Frame {
                screen: "s",
                size,
                snapshot: snap,
                findings: &[],
            },
            &mut errors,
        );
        assert!(errors.is_empty(), "{errors:?}");
        hits.len()
    };
    // a plain button named "Set up now" in the Sets panel is held to the hovered width (the planted screen)
    let set_up = PlantedScreen(Planted::SetUpButton).at(size).unwrap();
    assert_eq!(observe(&set_up), 1, "a Set up now button is no set card");
    // what a card has by `sets_column::card_node`: the Button role, "Set <id>, position <i> of <n>", a selected state and
    // ScrollIntoView, in the Sets panel. Each one alone missing makes the node an ordinary control again.
    let mut card = set_up.clone();
    for n in card.nodes.iter_mut().filter(|n| n.label == "Set up now") {
        n.label = "Set stone/cobble, position 1 of 3".into();
        n.selected = Some(false);
        n.scroll_into_view = true;
    }
    assert_eq!(observe(&card), 0, "a real card stops at the track");
    type Spoil = fn(&mut Node);
    let broken: [(&str, Spoil); 5] = [
        ("no selected state", |n| n.selected = None),
        ("no ScrollIntoView", |n| n.scroll_into_view = false),
        ("a name without the position", |n| {
            n.label = "Set stone/cobble".into()
        }),
        ("a name with a prefix only", |n| {
            n.label = "Set up, position now of later".into()
        }),
        ("another role", |n| n.role = "Link".into()),
    ];
    for (what, spoil) in broken {
        let mut c = card.clone();
        c.nodes
            .iter_mut()
            .filter(|n| n.label.starts_with("Set ") || n.role == "Button" && n.interactive)
            .for_each(spoil);
        assert_eq!(observe(&c), 1, "{what}");
    }
    // outside the Sets panel the card is held to the hovered width too
    let mut away = card.clone();
    away.panels.retain(|(name, _)| name != "sets");
    away.panels.push((
        "centre".into(),
        Rect::from_min_size(eframe::egui::Pos2::ZERO, vec2(2000.0, 2000.0)),
    ));
    away.panels.push((
        "sets".into(),
        Rect::from_min_size(eframe::egui::Pos2::ZERO, vec2(1.0, 1.0)),
    ));
    assert_eq!(observe(&away), 1, "a card outside the Sets panel");
}

fn observe_bars(snap: &Snapshot) -> (Vec<Hit>, Vec<String>) {
    let mut errors = vec![];
    let hits = scroll_bars_are_visible_and_clear(
        &Frame {
            screen: "s",
            size: SIZES[1],
            snapshot: snap,
            findings: &[],
        },
        &mut errors,
    );
    (hits, errors)
}

/// A bar is "scroll bar K of N in the P panel": two bars of one panel are told apart by their place, top to bottom,
/// and a bar no named panel holds has no name, which is an error rather than a weaker name.
#[test]
fn a_bar_is_known_by_its_place_in_its_panel() {
    let clean = PlantedScreen(Planted::CleanBar).at(SIZES[1]).unwrap();
    let bar = clean
        .nodes
        .iter()
        .find(|n| n.role == "ScrollBar")
        .unwrap()
        .clone();
    let names = |s: &Snapshot| -> Vec<Result<String, String>> {
        bar_places(s).into_iter().map(|(_, p)| p).collect()
    };
    assert_eq!(
        names(&clean),
        [Ok("scroll bar 1 of 1 in the textures panel".to_string())]
    );
    let mut two = clean.clone();
    let mut lower = bar.clone();
    lower.rect = lower.rect.translate(vec2(0.0, 200.0));
    two.nodes.push(lower);
    assert_eq!(
        names(&two),
        [
            Ok("scroll bar 1 of 2 in the textures panel".to_string()),
            Ok("scroll bar 2 of 2 in the textures panel".to_string())
        ]
    );
    // two bars at one rect: no place can be told, so no name (an error, not a shared identity)
    let mut coincident = clean.clone();
    coincident.nodes.push(bar.clone());
    let places = names(&coincident);
    assert!(
        places.iter().all(
            |p| matches!(p, Err(e) if e.contains("lie at one rect") && e.contains("textures panel"))
        ),
        "{places:?}"
    );
    let (hits, errors) = observe_bars(&coincident);
    assert_eq!(hits.len(), 0);
    assert_eq!(errors.len(), 2, "{errors:?}");
    let mut nameless = clean.clone();
    nameless.panels.clear();
    let places = names(&nameless);
    assert!(
        matches!(&places[..], [Err(e)] if e.contains("PANEL_NAMES")),
        "{places:?}"
    );
    let (hits, errors) = observe_bars(&nameless);
    assert_eq!(hits.len(), 0);
    assert!(errors[0].contains("no named panel holds"), "{errors:?}");
}

/// A handle shorter than 20 px is the bar's own paint (egui draws one as short as 12 px); a transparent bar beside an
/// opaque shape of its layer outside its strip is invisible, whatever the shape's size and place.
#[test]
fn a_short_handle_is_judged_on_the_bars_own_paint() {
    let strip_of = |s: &Snapshot| s.nodes.iter().find(|n| n.role == "ScrollBar").unwrap().rect;
    let short = PlantedScreen(Planted::ShortHandle).at(SIZES[1]).unwrap();
    let strip = strip_of(&short).expand(0.5);
    assert!(
        short
            .filled
            .iter()
            .any(|x| x.alpha > 0 && x.rect.height() < 20.0 && strip.contains_rect(x.rect)),
        "the fixture paints a visible handle under 20 px: {:?}",
        short.filled
    );
    assert!(
        short
            .filled
            .iter()
            .any(|x| x.alpha == 0 && x.rect.height() > 100.0 && strip.contains_rect(x.rect)),
        "the track of the fixture is transparent: {:?}",
        short.filled
    );
    let (hits, errors) = observe_bars(&short);
    assert!(hits.is_empty() && errors.is_empty(), "{hits:?} {errors:?}");
    let decoy = PlantedScreen(Planted::ShortHandleDecoy)
        .at(SIZES[1])
        .unwrap();
    let strip = strip_of(&decoy).expand(0.5);
    assert!(
        decoy
            .filled
            .iter()
            .any(|x| x.alpha == 255 && x.rect.height() == 12.0 && !strip.intersects(x.rect)),
        "the fixture paints an opaque 12 px shape outside the strip: {:?}",
        decoy.filled
    );
    let (hits, errors) = observe_bars(&decoy);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        hits.iter().map(|h| h.invariant).collect::<Vec<_>>(),
        ["scroll_bar_at_rest"]
    );
}

/// A handle nested in its track need not be as wide: a 2 px opaque handle centred in a 4 px transparent track is the
/// bar's own paint and shows it; one that sticks out of the track is not.
#[test]
fn a_handle_may_be_narrower_than_its_track() {
    let clean = PlantedScreen(Planted::CleanBar).at(SIZES[1]).unwrap();
    let strip = clean
        .nodes
        .iter()
        .find(|n| n.role == "ScrollBar")
        .unwrap()
        .rect
        .expand(0.5);
    let mut snap = clean.clone();
    let mut inside: Vec<usize> = (0..snap.filled.len())
        .filter(|i| strip.contains_rect(snap.filled[*i].rect))
        .collect();
    inside.sort_by(|a, b| {
        snap.filled[*b]
            .rect
            .height()
            .total_cmp(&snap.filled[*a].rect.height())
    });
    let (track, handle) = (inside[0], inside[1]);
    let right = snap.filled[track].rect.max.x;
    snap.filled[track].rect.min.x = right - 4.0;
    snap.filled[track].alpha = 0;
    snap.filled[handle].rect.min.x = right - 3.0;
    snap.filled[handle].rect.max.x = right - 1.0;
    snap.filled[handle].alpha = 255;
    let (hits, errors) = observe_bars(&snap);
    assert!(hits.is_empty() && errors.is_empty(), "{hits:?} {errors:?}");
    snap.filled[handle].rect.min.x = right - 6.0;
    let (hits, errors) = observe_bars(&snap);
    assert!(hits.is_empty(), "{hits:?}");
    assert_eq!(errors.len(), 1, "{errors:?}");
}

/// A handle as tall as its track (content that barely overflows) and narrower, painted after the track, is the handle:
/// at equal heights the candidate that holds the other is the track, whatever the paint order.
#[test]
fn a_full_height_handle_painted_after_its_track_is_the_handle() {
    let clean = PlantedScreen(Planted::CleanBar).at(SIZES[1]).unwrap();
    let strip = clean
        .nodes
        .iter()
        .find(|n| n.role == "ScrollBar")
        .unwrap()
        .rect
        .expand(0.5);
    let mut snap = clean.clone();
    let mut inside: Vec<usize> = (0..snap.filled.len())
        .filter(|i| strip.contains_rect(snap.filled[*i].rect))
        .collect();
    inside.sort_by(|a, b| {
        snap.filled[*b]
            .rect
            .height()
            .total_cmp(&snap.filled[*a].rect.height())
    });
    let (track, handle) = (inside[0], inside[1]);
    // a 4 px track and a 2 px handle in its middle, both the strip's whole height
    let right = snap.filled[track].rect.max.x;
    snap.filled[track].rect.min.x = right - 4.0;
    let t = snap.filled[track].rect;
    let mut h = snap.filled.remove(handle);
    h.rect = Rect::from_min_max(
        eframe::egui::pos2(right - 3.0, t.min.y),
        eframe::egui::pos2(right - 1.0, t.max.y),
    );
    h.alpha = 255;
    snap.filled.push(h);
    let (hits, errors) = observe_bars(&snap);
    assert!(hits.is_empty() && errors.is_empty(), "{hits:?} {errors:?}");
}

/// Paint in a layer egui moves by a transform is in other coordinates than the nodes: a status circle or a bar there is
/// an error naming the screen, the width and the layer, never a judgement.
#[test]
fn a_transformed_layer_is_an_error() {
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(PlantedScreen(Planted::TransformedLayer))];
    let walked = walk(&mut list, &mut real_checker);
    assert!(walked.hits.is_empty(), "{:?}", walked.hits);
    assert_eq!(walked.errors.len(), 3, "{:?}", walked.errors);
    for (e, size) in walked.errors.iter().zip(WIDTHS) {
        assert!(
            e.starts_with(&format!(
                "test-only transformed layer at {size}: the red status circle"
            )) && e.contains("is in the layer LayerId")
                && e.contains("transform"),
            "{e}"
        );
    }
    let mut moved = PlantedScreen(Planted::CleanBar).at(SIZES[1]).unwrap();
    let layer = moved
        .nodes
        .iter()
        .find(|n| n.role == "ScrollBar")
        .and_then(|n| n.layer.clone())
        .unwrap();
    moved.transformed_layers.push(layer);
    let (hits, errors) = observe_bars(&moved);
    assert!(hits.is_empty(), "{hits:?}");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("transform"), "{errors:?}");
}

/// A per-bar entry pins the bar and its widgets: the same widgets under another bar, another widget with the same
/// overshoot, one more, one fewer, or a widget past the tolerance fails; the entry's own list passes.
#[test]
fn an_entry_for_a_bar_names_its_bar_and_its_widgets() {
    let bar_hit = |bar: &str, members: &[(&str, f32)]| {
        let mut h = hit("scroll_bar_clearance", bar, 0, 4.0);
        h.members = members.iter().map(|(n, px)| (n.to_string(), *px)).collect();
        h
    };
    let allowed = |bar: &'static str, members: &'static [(&'static str, f32)]| Allowed {
        invariant: "scroll_bar_clearance",
        what: bar,
        px: 4.0,
        members,
        ..entry("", 0, 1, 4.0)
    };
    let list: &'static [(&'static str, f32)] = &[("Button:A", 4.0), ("Link:B", 3.5)];
    let allow = [allowed("scroll bar 1 of 2 in the x panel", list)];
    let ok = |hits: Vec<Hit>| verdict(&walked(hits), &allow, &[]).is_empty();
    assert!(ok(vec![bar_hit(
        "scroll bar 1 of 2 in the x panel",
        &[("Button:A", 4.0), ("Link:B", 3.5)]
    )]));
    assert!(ok(vec![bar_hit(
        "scroll bar 1 of 2 in the x panel",
        &[("Button:A", 5.9), ("Link:B", 2.0)]
    )]));
    for (bar, members) in [
        (
            "scroll bar 2 of 2 in the x panel",
            &[("Button:A", 4.0), ("Link:B", 3.5)][..],
        ),
        (
            "scroll bar 1 of 2 in the x panel",
            &[("Button:A", 4.0), ("Link:C", 3.5)][..],
        ),
        ("scroll bar 1 of 2 in the x panel", &[("Button:A", 4.0)][..]),
        (
            "scroll bar 1 of 2 in the x panel",
            &[("Button:A", 4.0), ("Link:B", 3.5), ("Link:C", 1.0)][..],
        ),
        (
            "scroll bar 1 of 2 in the x panel",
            &[("Button:A", 7.0), ("Link:B", 3.5)][..],
        ),
    ] {
        assert!(!ok(vec![bar_hit(bar, members)]), "{bar} {members:?}");
    }
}

/// Errors are never allowed.
#[test]
fn an_error_is_never_allowed() {
    let w = Walked {
        hits: vec![],
        errors: vec!["s at 1100x800: did not go idle".into()],
        visits: vec![],
        painted_ellipsis_texts: Default::default(),
        presses: vec![],
        bars: vec![],
    };
    assert_eq!(verdict(&w, &[], &[]).len(), 1);
}
