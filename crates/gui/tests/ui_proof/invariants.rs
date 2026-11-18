//! The whole-app walk: every entry of `screens::entries()` at three window sizes, one frame run of `run_checks` per
//! entry and size, and every invariant of `FRAME_INVARIANTS` run on it. `walk` and `verdict` are the only code that
//! iterates entries, collects hits and judges them, for the shipped list and for the proof's test-only entry alike.
//!
//! To add an invariant that reads one frame: write a `fn(&Frame) -> Vec<Hit>` and add it to `FRAME_INVARIANTS` (add
//! to `Frame` what it reads). Task 3's per-screen step goes in `walk`, after an entry's sizes. A hit the tree has today
//! goes in `ALLOWED` with a reason; a hit no entry covers fails, and so does an entry no hit consumed.

use crate::checks::{run_checks, Finding};
use crate::frame::{snapshot, Snapshot};
use crate::screens::{entries, holds, Needle};
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
    /// How far the widget is past the edge, or how much of the text is clipped, in px.
    pub px: f32,
    pub message: String,
}

impl Hit {
    fn text(&self) -> String {
        format!(
            "{} at {}x{}: {} {:?} ({:.1} px): {}",
            self.screen, self.size.x, self.size.y, self.invariant, self.what, self.px, self.message
        )
    }
}

type FrameInvariant = fn(&Frame) -> Vec<Hit>;

/// Invariants that read the one frame of a screen at a size.
pub const FRAME_INVARIANTS: &[FrameInvariant] = &[nothing_past_the_window_edge, no_clipped_text];

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

/// What `walk` collected.
pub struct Walked {
    pub hits: Vec<Hit>,
    /// Visits that could not be made, and markers other entries' screens satisfy; never allowed.
    pub errors: Vec<String>,
}

/// Every entry at every size: one frame, one `run_checks`, every frame invariant. A visit that fails is collected and
/// the walk goes on. Afterwards each entry's marker is checked against every other entry's screen at the middle size.
pub fn walk(entries: &mut [Box<dyn Entry>]) -> Walked {
    let mut walked = Walked {
        hits: vec![],
        errors: vec![],
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
            let findings = run_checks(&snap);
            let frame = Frame {
                screen: e.name(),
                size,
                findings: &findings,
            };
            for invariant in FRAME_INVARIANTS {
                walked.hits.extend(invariant(&frame));
            }
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

/// One allowed hit: the screen, the invariant, the size, the widget's exact name, how many hits it covers and how far
/// past the edge (or clipped) each is, in px, within `PX_TOLERANCE`; and why it is allowed today.
pub struct Allowed {
    pub screen: &'static str,
    pub invariant: &'static str,
    pub size: (f32, f32),
    pub what: &'static str,
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
        count: 1,
        px: 78.1,
        reason: "the target combo ends past the right edge (x 1078..1178)",
    },
    Allowed {
        screen: "json editor",
        invariant: "window_edge",
        size: (1100.0, 800.0),
        what: "derived from the set",
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
        count: 1,
        px: 99.7,
        reason: "the first row's tag sticks out of its clip",
    },
    Allowed {
        screen: "json editor",
        invariant: "clipped_text",
        size: (1100.0, 800.0),
        what: "derived from the set",
        count: 1,
        px: 115.3,
        reason: "the second row's tag sticks out of its clip",
    },
];

fn covers(a: &Allowed, h: &Hit) -> bool {
    a.screen == h.screen
        && a.invariant == h.invariant
        && (a.size.0 - h.size.x).abs() < 0.5
        && (a.size.1 - h.size.y).abs() < 0.5
        && a.what == h.what
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
                "{}; if it cannot be fixed now, add it to ALLOWED in crates/gui/tests/ui_proof/invariants.rs with its screen, size, name, count and px, and a reason",
                h.text()
            )),
        }
    }
    for (a, n) in allowed.iter().zip(&left) {
        if *n > 0 {
            out.push(format!(
                "ALLOWED in crates/gui/tests/ui_proof/invariants.rs holds {} / {} / {}x{} / {:?} / count {} / {:.1} px ({}), but {} of its hits did not come; delete or correct the entry",
                a.screen, a.invariant, a.size.0, a.size.1, a.what, a.count, a.px, a.reason, n
            ));
        }
    }
    out
}

#[test]
fn the_walk_finds_nothing_new_on_any_screen() {
    let start = std::time::Instant::now();
    let mut list = entries();
    let walked = walk(&mut list);
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
}

/// A test-only entry: a tiny UI with a button that ends past the narrowest window's edge and a label cut by its clip.
/// It is not in the shipped list: a shipped entry opens the real app, which has no widget to plant.
struct Faulty;

const FAULTY_MARKER: &[Needle] = &[crate::screens::has_role("Button", "Far away")];

fn faulty_ui(ui: &mut eframe::egui::Ui) {
    use eframe::egui::{pos2, Rect};
    ui.put(
        Rect::from_min_size(pos2(1000.0, 20.0), vec2(200.0, 24.0)),
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

/// The faulty entry goes through `walk` and `verdict`, the code the shipped list goes through, beside a clean shipped
/// entry: the final failure list holds both diagnostics for it and nothing for the clean one.
#[test]
fn a_widget_past_the_edge_and_a_clipped_text_fail_the_walk() {
    let clean = entries()
        .into_iter()
        .find(|e| e.name() == "start screen")
        .expect("the start screen entry");
    let mut list: Vec<Box<dyn Entry>> = vec![clean, Box::new(Faulty)];
    let walked = walk(&mut list);
    let bad = verdict(&walked, &[]);
    println!("{bad:#?}");
    let of = |inv: &str, what: &str| {
        walked
            .hits
            .iter()
            .filter(|x| {
                x.screen == "test-only faulty screen" && x.invariant == inv && x.what == what
            })
            .map(|x| (x.size.x, x.size.y))
            .collect::<Vec<_>>()
    };
    // 1000 + 200 = 1200: past the 1100 window only, by 100 px
    assert_eq!(of("window_edge", "Far away"), vec![(1100.0, 800.0)]);
    assert!(
        walked
            .hits
            .iter()
            .any(|x| x.invariant == "window_edge" && (x.px - 100.0).abs() < 0.5),
        "{:?}",
        walked.hits
    );
    // cut by its clip at every size, the third size 1600x900
    assert_eq!(
        of("clipped_text", "cut label sticking out"),
        vec![(1100.0, 800.0), (1280.0, 800.0), (1600.0, 900.0)]
    );
    assert!(
        bad.iter().any(|m| m.starts_with(
            "test-only faulty screen at 1100x800: window_edge \"Far away\" (100.0 px): Button"
        )),
        "{bad:#?}"
    );
    assert!(
        bad.iter()
            .any(|m| m.contains("clipped_text \"cut label sticking out\"")),
        "{bad:#?}"
    );
    assert!(
        bad.iter().all(|m| m.starts_with("test-only faulty screen")),
        "nothing is reported for the clean entry: {bad:#?}"
    );
    assert!(walked.errors.is_empty(), "{:?}", walked.errors);
}

/// A visit that fails is a collected failure naming screen and size; the walk goes on to the other sizes and entries.
#[test]
fn a_failed_visit_is_collected_and_the_walk_goes_on() {
    let mut list: Vec<Box<dyn Entry>> = vec![Box::new(Broken), Box::new(Faulty)];
    let walked = walk(&mut list);
    let bad = verdict(&walked, &[]);
    assert!(
        bad.iter()
            .any(|m| m == "test-only broken screen at 1100x800: did not go idle"),
        "{bad:#?}"
    );
    // the later sizes of the broken entry and the whole faulty entry were still visited
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
            .any(|h| h.screen == "test-only faulty screen"),
        "{:?}",
        walked.hits
    );
}

/// A marker that another entry's screen satisfies is a failure naming both.
#[test]
fn a_marker_another_screen_satisfies_fails() {
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
    let walked = walk(&mut list);
    assert!(
        walked.errors.iter().any(|e| e.contains(
            "marker of \"test-only twin\" also holds on the screen \"test-only faulty screen\""
        )),
        "{:?}",
        walked.errors
    );
}

fn hit(invariant: &'static str, what: &str, px: f32) -> Hit {
    Hit {
        screen: "s".into(),
        size: SIZES[0],
        invariant,
        what: what.into(),
        px,
        message: String::new(),
    }
}

fn entry(what: &'static str, count: usize, px: f32) -> Allowed {
    Allowed {
        screen: "s",
        invariant: "window_edge",
        size: (SIZES[0].x, SIZES[0].y),
        what,
        count,
        px,
        reason: "r",
    }
}

fn walked(hits: Vec<Hit>) -> Walked {
    Walked {
        hits,
        errors: vec![],
    }
}

/// The allowlist is exact: size, name, count and px must all agree, and what it covers is consumed one for one.
#[test]
fn an_allowed_hit_must_match_exactly_and_is_consumed_one_for_one() {
    let two = walked(vec![
        hit("window_edge", "tag", 92.0),
        hit("window_edge", "tag", 107.0),
    ]);
    let exact = [entry("tag", 1, 91.0), entry("tag", 1, 108.0)];
    assert!(verdict(&two, &exact).is_empty());
    // one fixed: the other entry is stale
    let one = walked(vec![hit("window_edge", "tag", 92.0)]);
    let stale = verdict(&one, &exact);
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert!(stale[0].contains("did not come"), "{stale:?}");
    // a third hit: unmatched
    let three = walked(vec![
        hit("window_edge", "tag", 92.0),
        hit("window_edge", "tag", 92.0),
        hit("window_edge", "tag", 107.0),
    ]);
    let extra = verdict(&three, &exact);
    assert_eq!(extra.len(), 1, "{extra:?}");
    // one entry with a count of two is consumed twice
    let both = walked(vec![
        hit("window_edge", "tag", 92.0),
        hit("window_edge", "tag", 92.5),
    ]);
    assert!(verdict(&both, &[entry("tag", 2, 92.0)]).is_empty());
    assert_eq!(verdict(&both, &[entry("tag", 3, 92.0)]).len(), 1);
    assert_eq!(verdict(&both, &[entry("tag", 1, 92.0)]).len(), 1);
    // other size, px off by 10, a name that is only part of the widget's
    let mut moved = entry("tag", 1, 92.0);
    moved.size = (1280.0, 800.0);
    let h = walked(vec![hit("window_edge", "tag", 92.0)]);
    assert_eq!(verdict(&h, &[moved]).len(), 2);
    assert_eq!(verdict(&h, &[entry("tag", 1, 102.0)]).len(), 2);
    assert_eq!(verdict(&h, &[entry("ta", 1, 92.0)]).len(), 2);
    assert_eq!(verdict(&h, &[entry("tag", 1, 94.0)]).len(), 0);
}

/// Errors are never allowed.
#[test]
fn an_error_is_never_allowed() {
    let w = Walked {
        hits: vec![],
        errors: vec!["s at 1100x800: did not go idle".into()],
    };
    assert_eq!(verdict(&w, &[]).len(), 1);
}
