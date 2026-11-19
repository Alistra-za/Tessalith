//! The whole-app walk: every entry of `screens::entries()` at three window sizes, one frame run of `run_checks` per
//! entry and size, and every invariant of `FRAME_INVARIANTS` run on it. `walk` and `verdict` are the only code that
//! iterates entries, collects hits and judges them, for the shipped list and for the proof's test-only entry alike.
//!
//! To add an invariant that reads one frame: write a `fn(&Frame) -> Vec<Hit>` and add it to `FRAME_INVARIANTS` (add
//! to `Frame` what it reads). Task 3's per-screen step goes in `walk`, after an entry's sizes. A hit the tree has today
//! goes in `ALLOWED` with a reason; a hit no entry covers fails, and so does an entry no hit consumed.

use crate::checks::{run_checks, Finding};
use crate::frame::{snapshot, Node, Snapshot, Text};
use crate::screens::{entries, holds, Needle, Screen, ScreenEntry};
use eframe::egui::{vec2, Vec2};
use egui_kittest::Harness;

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

/// True when `value` is a longer text that `shown` is cut from. `shown` holds the ellipsis where the cut is: at the
/// start (a front cut shows the end of the value), at the end (a back cut shows the start), or between two pieces (a
/// middle cut shows both ends, in order; more than one ellipsis gives more pieces, found in order).
///
/// A text without an ellipsis is a galley egui cut itself (`Text::elided`): its text is the full text, so the value
/// must contain it.
///
/// A text that merely ends in one ellipsis ("Settings…", "Open folder…": the author's sign that a dialog opens) is its
/// own value: a node holding exactly that text passes, nothing is cut. Any other node text equal to `shown` (a middle
/// cut named by itself, two ellipses) keeps nothing and fails.
// shortcut: a hand-cut "head…" named by that same text cannot be told from an author's ellipsis; upgrade when such a
// text is found (its hit would be missing from the list).
fn holds_the_longer_text(shown: &str, value: &str) -> bool {
    if !shown.contains('…') {
        return value.contains(shown);
    }
    if value == shown {
        return shown.ends_with('…') && shown.matches('…').count() == 1;
    }
    let pieces: Vec<&str> = shown.split('…').collect();
    let (first, last) = (pieces[0], pieces[pieces.len() - 1]);
    let mids = &pieces[1..pieces.len() - 1];
    // a glyph painted beside the text ("⚠ ", " ▾") is not in the value: try the end pieces without it too
    let plain = |c: char| !c.is_alphanumeric();
    pieces_cut_from(value, first, mids, last)
        || pieces_cut_from(
            value,
            first.trim_start_matches(plain),
            mids,
            last.trim_end_matches(plain),
        )
}

/// `value` starts with `first`, ends with `last`, holds `mids` in order between them, and is longer than the pieces.
fn pieces_cut_from(value: &str, first: &str, mids: &[&str], last: &str) -> bool {
    let Some(rest) = value.strip_prefix(first) else {
        return false;
    };
    let Some(mut rest) = rest.strip_suffix(last) else {
        return false;
    };
    for piece in mids {
        match rest.find(piece) {
            Some(at) => rest = &rest[at + piece.len()..],
            None => return false,
        }
    }
    // longer than what is shown: something was left out
    value.chars().count()
        > [first, last]
            .iter()
            .chain(mids)
            .map(|p| p.chars().count())
            .sum::<usize>()
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
// egui's own truncating `Label` passes by design: `Label::ui` names its node with `galley.text()`, the full text
// (egui-0.37.0/src/widgets/label.rs line 281), and shows that text on hover when the galley is elided (line 284 to
// 292), so the node's value is the full text.
// shortcut: the node is found by geometry (the smallest named node holding the text's rect); a text drawn in a box
// that another named node sits in is judged by that smaller node. Upgrade when a hit names the wrong node.
fn cut_text_keeps_its_value(f: &Frame) -> Vec<Hit> {
    let mut hits = vec![];
    for t in &f.snapshot.texts {
        if !(t.elided || t.ends_in_ellipsis || t.text.contains('…')) {
            continue;
        }
        let shown = if t.ends_in_ellipsis {
            format!("{}…", t.text)
        } else {
            t.text.clone()
        };
        let node = node_under(f.snapshot, t);
        let held = node.map_or_else(Vec::new, |n| {
            [Some(&n.label), n.value.as_ref(), n.description.as_ref()]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .map(String::as_str)
                .collect::<Vec<_>>()
        });
        let ok = held.iter().any(|v| {
            if t.elided {
                v.contains(t.text.as_str())
            } else {
                holds_the_longer_text(&shown, v)
            }
        });
        if ok {
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
                "the cut text at {:?} {under} keeps no longer text; give the node the full text as its accessible name or value and show it on hover, or use Label::truncate(), instead of a hand-cut text",
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
    // The six below are not faults: an ellipsis the author wrote in the middle of a text, named by that same text, which
    // the walk cannot tell from a hand-cut middle ("C:\…\export" named by itself). Each is one text, one size.
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
    }
    let bad = verdict(&walked, ALLOWED);
    assert!(bad.is_empty(), "{bad:#?}");
    assert_eq!(
        walked.visits.len(),
        list.len() * SIZES.len(),
        "a visit was not completed or not recorded"
    );
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

/// A test-only entry with three cut texts: a hand-cut middle named by the cut text itself (the fault), a cut that egui
/// makes and names by its full text (`Label::truncate`, fine by design), and a hand-cut middle whose name is the full
/// path (fine).
struct CutTexts;

const CUT_MARKER: &[Needle] = &[crate::screens::has_role("Label", "some/long/…/name")];

fn cut_texts_ui(ui: &mut eframe::egui::Ui) {
    use eframe::egui::{pos2, Rect};
    let slot = |y: f32, w: f32| Rect::from_min_size(pos2(10.0, y), vec2(w, 20.0));
    ui.put(
        slot(20.0, 200.0),
        eframe::egui::Label::new("some/long/…/name"),
    );
    ui.put(
        slot(50.0, 90.0),
        eframe::egui::Label::new("a long label that egui cuts").truncate(),
    );
    ui.put(
        slot(80.0, 200.0),
        eframe::egui::Label::new("other/long/…/name"),
    )
    .on_hover_text("other/long/path/to/the/name")
    .accessible_name("other/long/path/to/the/name");
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
/// hand-cut text named by itself at each size, and nothing for the two fine ones.
#[test]
fn a_cut_text_without_its_full_value_fails_the_walk() {
    let mut list: Vec<Box<dyn Entry>> = vec![the_start_screen(), Box::new(CutTexts)];
    let mut calls: Vec<(String, (f32, f32))> = vec![];
    let walked = walk(&mut list, &mut |screen, size, snap| {
        calls.push((screen.to_string(), (size.x, size.y)));
        real_checker(screen, size, snap)
    });
    let bad = verdict(&walked, &[]);
    println!("{bad:#?}");
    let mut want_calls = vec![];
    for name in ["start screen", "test-only cut-text screen"] {
        for size in SIZES {
            want_calls.push((name.to_string(), (size.x, size.y)));
        }
    }
    assert_eq!(calls, want_calls, "the checker ran on every frame");
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
    // the fixture is the case in question: egui cut the truncated label, and the three cut-looking texts are painted
    let snap = CutTexts.at(SIZES[0]).unwrap();
    let elided: Vec<&str> = snap
        .texts
        .iter()
        .filter(|t| t.elided)
        .map(|t| t.text.as_str())
        .collect();
    assert_eq!(elided, ["a long label that egui cuts"]);
    assert_eq!(
        snap.texts.iter().filter(|t| t.text.contains('…')).count(),
        2
    );
    let mut got: Vec<&str> = bad.iter().map(|m| head(m)).collect();
    got.sort();
    let want = vec![
        r#"test-only cut-text screen at 1100x800: cut_text "some/long/…/name" #0 (0.0 px)"#,
        r#"test-only cut-text screen at 1280x800: cut_text "some/long/…/name" #0 (0.0 px)"#,
        r#"test-only cut-text screen at 1600x900: cut_text "some/long/…/name" #0 (0.0 px)"#,
    ];
    assert_eq!(got, want, "{bad:#?}");
    assert!(
        bad.iter().all(|m| m.contains("ALLOWED")
            && m.contains("under Label")
            && m.contains("Label::truncate()")),
        "every failure names the node and says what to do about it: {bad:#?}"
    );
}

/// What `holds_the_longer_text` accepts: each cut form against the value it is cut from, and against others.
#[test]
fn the_matcher_reads_front_back_and_middle_cuts() {
    let h = holds_the_longer_text;
    // a front cut shows the end of the value
    assert!(h("…path/to/name", "some/long/path/to/name"));
    assert!(!h("…path/to/name", "some/long/path/to/other"));
    assert!(!h("…path/to/name", "…path/to/name"));
    // a back cut shows the start
    assert!(h("some/long/p…", "some/long/path"));
    assert!(!h("some/long/p…", "other/long/path"));
    // a middle cut shows both ends, in order
    assert!(h("some/…/name", "some/long/path/name"));
    assert!(!h("some/…/name", "some/long/path/other"));
    assert!(!h("some/…/name", "other/long/path/name"));
    assert!(!h("some/…/name", "some/…/name"), "named by itself");
    assert!(!h("a…c…e", "aXXe"), "a middle piece is missing");
    assert!(h("a…c…e", "aXcXe"));
    // a glyph painted beside the cut text is not in the value
    assert!(h(
        "Export to: C:\\…\\export ▾",
        "Export to: C:\\Users\\me\\export"
    ));
    // a text that merely ends in an ellipsis the author wrote is its own value, and only with one ellipsis
    assert!(h("Settings…", "Settings…"));
    assert!(!h("a…b…", "a…b…"));
    assert!(
        !h("Settings…", "Settings"),
        "a name that is the cut head holds nothing longer"
    );
    // a galley egui cut keeps its full text; the value must contain it
    assert!(h("the full text", "the full text"));
    assert!(!h("the full text", "other"));
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

/// Errors are never allowed.
#[test]
fn an_error_is_never_allowed() {
    let w = Walked {
        hits: vec![],
        errors: vec!["s at 1100x800: did not go idle".into()],
        visits: vec![],
    };
    assert_eq!(verdict(&w, &[]).len(), 1);
}
