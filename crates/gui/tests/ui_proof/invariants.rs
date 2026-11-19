//! The whole-app walk: every entry of `screens::entries()` at three window sizes, one frame run of `run_checks` per
//! entry and size, and every invariant of `FRAME_INVARIANTS` run on it. `walk` and `verdict` are the only code that
//! iterates entries, collects hits and judges them, for the shipped list and for the proof's test-only entry alike.
//!
//! To add an invariant that reads one frame: write a `fn(&Frame) -> Vec<Hit>` and add it to `FRAME_INVARIANTS` (add
//! to `Frame` what it reads). The per-screen key step (`press_keys`) runs in `walk`, once after an entry's sizes. A hit the tree has today
//! goes in `ALLOWED` with a reason; a hit no entry covers fails, and so does an entry no hit consumed.

use crate::checks::{run_checks, Finding};
use crate::frame::{snapshot, Node, Snapshot, Text};
use crate::screens::{entries, holds, Needle, Screen, ScreenEntry};
use eframe::egui::{vec2, Key, Vec2};
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

    /// True when the key step may press keys on this entry; a test-only entry that is only a layout fixture says no.
    fn takes_keys(&self) -> bool {
        false
    }
    /// The folders the key step hashes, each with the name a failure message gives it. Read again after `reopen`.
    fn watched(&self) -> Vec<(&'static str, PathBuf)> {
        vec![]
    }
    /// With nothing focused: presses `key`, lets the app settle and says what it found.
    fn press(&mut self, _key: Key) -> Result<Pressed, String> {
        Err("this entry takes no keys".into())
    }
    /// Shows the screen again from scratch at `size`, after a key closed it.
    fn reopen(&mut self, _size: Vec2) -> Result<(), String> {
        Err("this entry cannot be reopened".into())
    }
}

/// What one key did to a screen, besides the files (those are hashed).
pub struct Pressed {
    /// An export was already running before the key (never the case on a screen of the walk).
    pub running_before: bool,
    /// The export gate's flag, read on the frame after the key.
    pub running_after_key: bool,
    /// The same flag after the app settled.
    pub running_after_settle: bool,
    /// The entry's marker still holds: false when the key closed the screen.
    pub shown: bool,
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
///   included, so a node holding only the shortened start does not pass;
/// - an ellipsis in it: a field holds a longer text it is cut from (`holds_the_longer_text`, after the named
///   decorations are stripped), or the whole source is in `authored`.
///
/// egui's own truncating `Label` passes by design: `Label::ui` names its node with `galley.text()`, the full text
/// (egui-0.37.0/src/widgets/label.rs line 281), and shows that text on hover when the galley is elided (lines 284 to
/// 292), so its value holds the whole source.
fn cut_text_kept(source: &str, elided: bool, held: &[&str], authored: &[&str]) -> bool {
    if elided {
        return held.iter().any(|v| v.contains(source));
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
    };
    let mut middle: Vec<Option<Snapshot>> = vec![];
    for e in entries.iter_mut() {
        let mut kept = None;
        for (i, size) in SIZES.into_iter().enumerate() {
            let snap = match e.at(size) {
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
            for invariant in FRAME_INVARIANTS {
                walked.hits.extend(invariant(&frame));
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
        if e.takes_keys() {
            press_keys(&mut **e, SIZES[1], &mut walked);
        }
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

/// Every path under some folders: a folder is `None`, a file is its length and a hash of its bytes. A file counts as
/// changed when its bytes are, not when its modification time is: a save that writes the same bytes is no change.
pub type Tree = BTreeMap<String, Option<(u64, u64)>>;

fn add_to_tree(name: &str, root: &Path, dir: &Path, tree: &mut Tree) {
    use std::hash::Hasher;
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let key = format!("{name}/{}", rel.to_string_lossy().replace('\\', "/"));
        if path.is_dir() {
            tree.insert(key, None);
            add_to_tree(name, root, &path, tree);
        } else {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            // a file that cannot be read hashes its error, so a file that becomes unreadable is a change too
            let len = match std::fs::read(&path) {
                Ok(bytes) => {
                    hasher.write(&bytes);
                    bytes.len() as u64
                }
                Err(e) => {
                    hasher.write(e.to_string().as_bytes());
                    u64::MAX
                }
            };
            tree.insert(key, Some((len, hasher.finish())));
        }
    }
}

/// The tree of every watched folder, each under its name.
pub fn tree_of(roots: &[(&'static str, PathBuf)]) -> Tree {
    let mut tree = Tree::new();
    for (name, root) in roots {
        add_to_tree(name, root, root, &mut tree);
    }
    tree
}

/// What differs between two trees: a new path, a deleted path, a file whose bytes changed (or a path that was a folder
/// and is a file now), each with the kind of change.
pub fn changes(before: &Tree, after: &Tree) -> Vec<(String, &'static str)> {
    let mut out = vec![];
    for (path, now) in after {
        match (before.get(path), now) {
            (None, None) => out.push((path.clone(), "new folder")),
            (None, Some(_)) => out.push((path.clone(), "new file")),
            (Some(was), now) if was != now => out.push((path.clone(), "changed")),
            _ => {}
        }
    }
    for (path, was) in before {
        if !after.contains_key(path) {
            out.push((
                path.clone(),
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

/// The key step of one screen, at `size`: with nothing focused, presses Enter, Escape and Space one after the other, and
/// hits every file a key changed and every key that left an export running. The folders are hashed once before the three
/// keys and once after each; a key that closed the screen is followed by `reopen` (part of this screen's visit) and a
/// new baseline, so a later key is judged on the screen it was meant for, and a change is reported against the key that
/// made it, not again against the next.
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
    let mut before = tree_of(&e.watched());
    let mut found: Vec<(&str, String, String)> = vec![];
    for (key, name) in KEYS {
        let pressed = match e.press(key) {
            Ok(p) => p,
            Err(why) => return error(format!("{name}: {why}")),
        };
        let after = tree_of(&e.watched());
        walked.presses.push(Press {
            screen: screen.clone(),
            key: name,
            paths: after.len(),
            shown: pressed.shown,
        });
        for (path, kind) in changes(&before, &after) {
            found.push((
                name,
                format!("{name}: {path} ({kind})"),
                format!("changed {path} ({kind})"),
            ));
        }
        if !pressed.running_before && (pressed.running_after_key || pressed.running_after_settle) {
            found.push((
                name,
                format!("{name}: export running"),
                "left an export running (the model's export.running is set)".to_string(),
            ));
        }
        before = after;
        if !pressed.shown {
            if let Err(why) = e.reopen(size) {
                return error(format!(
                    "{name} closed the screen and it could not be reopened: {why}"
                ));
            }
            before = tree_of(&e.watched());
        }
    }
    for (key, what, effect) in found {
        walked.hits.push(Hit {
            screen: screen.clone(),
            size,
            invariant: KEYS_NEVER_WRITE,
            what,
            y: 0.0,
            ordinal: 0,
            px: 0.0,
            message: format!(
                "pressing {key} with nothing focused {effect}; a key with nothing focused never writes a file or starts an export (egui-ui-rules, dialogs: Enter triggers the safe default and never overwrites, deletes or exports)"
            ),
        });
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
        reason: "the chip's own \"Assign…\" inside parentheses; its button's name goes on with \" — go to the fix\"",
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
}

/// The final failure list: every collected error, every hit no entry consumed, every entry not consumed in full.
/// Entries are consumed one hit at a time; empty when the walk passes.
pub fn verdict(walked: &Walked, allowed: &[Allowed]) -> Vec<String> {
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
    }
    let mut bad = verdict(&walked, ALLOWED);
    bad.extend(stale_authored(&walked, AUTHORED));
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
    let bad = verdict(&walked, &[]);
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
/// ellipsis), and fine ones (a `Label::truncate`, a hand cut named by the full path, with and without a decoration).
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
/// four faulty texts at each size, and nothing for the three fine ones.
#[test]
fn a_cut_text_without_its_full_value_fails_the_walk() {
    let mut list: Vec<Box<dyn Entry>> = vec![the_start_screen(), Box::new(CutTexts)];
    let mut calls: Vec<(String, (f32, f32))> = vec![];
    let walked = walk(&mut list, &mut |screen, size, snap| {
        calls.push((screen.to_string(), (size.x, size.y)));
        real_checker(screen, size, snap)
    });
    let bad = verdict(&walked, &[]);
    let mut want_calls = vec![];
    for name in ["start screen", "test-only cut-text screen"] {
        for size in SIZES {
            want_calls.push((name.to_string(), (size.x, size.y)));
        }
    }
    assert_eq!(calls, want_calls, "the checker ran on every frame");
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    // the fixture is the case in question: egui cut two galleys, and six more painted texts show an ellipsis
    let snap = CutTexts.at(SIZES[0]).unwrap();
    let mut elided: Vec<&str> = snap
        .texts
        .iter()
        .filter(|t| t.elided)
        .map(|t| t.source.as_str())
        .collect();
    elided.sort();
    assert_eq!(elided, ["a long label that egui cuts", "abcdefghijkl…"]);
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
    // a galley egui cut: the whole source, ellipsis included, must be in the node
    assert!(kept("abcdef…", true, &["abcdef…"], none));
    assert!(!kept("abcdef…", true, &["abcdef"], none));
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
    let bad = verdict(&walked, &[]);
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
    };
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(ScreenEntry::new(&BOOM)), the_start_screen()];
    let walked = walk(&mut list, &mut real_checker);
    let bad = verdict(&walked, &[]);
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
    assert!(verdict(&two, &exact).is_empty());
    // the two rows exchange their overflows: both entries are wrong
    let swapped = [entry("tag", 0, 1, 108.0), entry("tag", 1, 1, 91.0)];
    assert_eq!(verdict(&two, &swapped).len(), 4);
    // one fixed: the other entry is stale
    let one = walked(vec![hit("window_edge", "tag", 0, 92.0)]);
    let stale = verdict(&one, &exact);
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert!(stale[0].contains("did not come"), "{stale:?}");
    // a third hit: unmatched
    let three = walked(vec![
        hit("window_edge", "tag", 0, 92.0),
        hit("window_edge", "tag", 0, 92.0),
        hit("window_edge", "tag", 1, 107.0),
    ]);
    let extra = verdict(&three, &exact);
    assert_eq!(extra.len(), 1, "{extra:?}");
    // one entry with a count of two is consumed twice
    let both = walked(vec![
        hit("window_edge", "tag", 0, 92.0),
        hit("window_edge", "tag", 0, 92.5),
    ]);
    assert!(verdict(&both, &[entry("tag", 0, 2, 92.0)]).is_empty());
    assert_eq!(verdict(&both, &[entry("tag", 0, 3, 92.0)]).len(), 1);
    assert_eq!(verdict(&both, &[entry("tag", 0, 1, 92.0)]).len(), 1);
    // other size, px off by 10, a name that is only part of the widget's
    let mut moved = entry("tag", 0, 1, 92.0);
    moved.size = (1280.0, 800.0);
    let h = walked(vec![hit("window_edge", "tag", 0, 92.0)]);
    assert_eq!(verdict(&h, &[moved]).len(), 2);
    assert_eq!(verdict(&h, &[entry("tag", 0, 1, 102.0)]).len(), 2);
    assert_eq!(verdict(&h, &[entry("ta", 0, 1, 92.0)]).len(), 2);
    assert_eq!(verdict(&h, &[entry("tag", 0, 1, 94.0)]).len(), 0);
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

/// An AUTHORED entry no screen paints is a failure naming it; one that is painted is not.
#[test]
fn an_authored_text_no_screen_paints_is_stale() {
    let mut w = walked(vec![]);
    w.painted_ellipsis_texts.insert("Settings…".to_string());
    assert!(stale_authored(&w, &["Settings…"]).is_empty());
    let stale = stale_authored(&w, &["Settings…", "Gone…"]);
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert!(stale[0].contains("\"Gone…\""), "{stale:?}");
}

/// What `KeyFault` keeps: whether its screen is open, and whether its "export" runs.
struct FaultState {
    dir: PathBuf,
    open: bool,
    running: bool,
}

/// A test-only entry whose keys break the rule: Enter writes a new file (and rewrites another with the same bytes, which
/// is not a change), Escape rewrites a file with new bytes and closes the screen, Space deletes a file and sets the
/// running flag. It is not in the shipped list.
struct KeyFault {
    dir: tempfile::TempDir,
    h: Harness<'static, FaultState>,
    /// How many times the walk asked for the screen again.
    reopened: std::rc::Rc<std::cell::Cell<usize>>,
}

const KEY_FAULT_MARKER: &[Needle] = &[crate::screens::has("Keys fault screen")];

fn key_fault_harness(size: Vec2) -> (tempfile::TempDir, Harness<'static, FaultState>) {
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
        open: true,
        running: false,
    };
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(size)
        .build_ui_state(
            |ui, s: &mut FaultState| {
                if s.open {
                    ui.label("Keys fault screen");
                }
                let (enter, escape, space) = ui.input(|i| {
                    (
                        i.key_pressed(Key::Enter),
                        i.key_pressed(Key::Escape),
                        i.key_pressed(Key::Space),
                    )
                });
                if enter {
                    std::fs::write(s.dir.join("new.txt"), "new").unwrap();
                    std::fs::write(s.dir.join("same.txt"), "same").unwrap();
                }
                if escape {
                    std::fs::write(s.dir.join("kept.txt"), "changed").unwrap();
                    s.open = false;
                }
                if space {
                    std::fs::remove_file(s.dir.join("gone.txt")).unwrap();
                    s.running = true;
                }
            },
            state,
        );
    (dir, h)
}

impl KeyFault {
    fn new(reopened: std::rc::Rc<std::cell::Cell<usize>>) -> Self {
        let (dir, h) = key_fault_harness(SIZES[1]);
        KeyFault { dir, h, reopened }
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

    fn takes_keys(&self) -> bool {
        true
    }

    fn watched(&self) -> Vec<(&'static str, PathBuf)> {
        vec![("fault folder", self.dir.path().to_path_buf())]
    }

    fn press(&mut self, key: Key) -> Result<Pressed, String> {
        clear_focus(&mut self.h)?;
        let running_before = self.h.state().running;
        self.h.key_press(key);
        self.h.run_steps(1);
        let running_after_key = self.h.state().running;
        self.h.run_steps(3);
        Ok(Pressed {
            running_before,
            running_after_key,
            running_after_settle: self.h.state().running,
            shown: holds(&snapshot(&self.h), KEY_FAULT_MARKER),
        })
    }

    fn reopen(&mut self, size: Vec2) -> Result<(), String> {
        self.reopened.set(self.reopened.get() + 1);
        let (dir, h) = key_fault_harness(size);
        (self.dir, self.h) = (dir, h);
        self.h.run_steps(3);
        Ok(())
    }
}

/// The key step goes through `walk` and `verdict`, the code the shipped list goes through, beside a clean shipped entry.
/// The faulty entry's Enter writes a new file, its Escape changes a file (and closes the screen, so the walk reopens it
/// before Space), its Space deletes a file and starts its "export"; Enter's rewrite of a file with the same bytes is no
/// change. The final failure list is exactly those four, at the middle size only, and the checker ran on the six frames
/// of the two entries and nothing else.
#[test]
fn a_key_that_writes_a_file_or_starts_an_export_fails_the_walk() {
    let reopened = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut list: Vec<Box<dyn Entry>> = vec![
        the_start_screen(),
        Box::new(KeyFault::new(reopened.clone())),
    ];
    let mut calls: Vec<(String, (f32, f32))> = vec![];
    let walked = walk(&mut list, &mut |screen, size, snap| {
        calls.push((screen.to_string(), (size.x, size.y)));
        real_checker(screen, size, snap)
    });
    let bad = verdict(&walked, &[]);
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

/// The hash counts content and paths: a file rewritten with the same bytes (its modification time moves) is no change; a
/// file with other bytes, a new file, a new folder and a deleted file or folder each are, and each is named by its path.
#[test]
fn a_tree_differs_by_content_and_paths_not_by_modification_time() {
    let d = tempfile::tempdir().unwrap();
    let at = |name: &str| d.path().join(name);
    std::fs::write(at("a.txt"), "one").unwrap();
    std::fs::write(at("b.txt"), "two").unwrap();
    std::fs::create_dir(at("old")).unwrap();
    let tree = || tree_of(&[("root", d.path().to_path_buf())]);
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

/// Errors are never allowed.
#[test]
fn an_error_is_never_allowed() {
    let w = Walked {
        hits: vec![],
        errors: vec!["s at 1100x800: did not go idle".into()],
        visits: vec![],
        painted_ellipsis_texts: Default::default(),
        presses: vec![],
    };
    assert_eq!(verdict(&w, &[]).len(), 1);
}
