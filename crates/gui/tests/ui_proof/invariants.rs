//! The whole-app walk: every screen of `screens::SCREENS` at three window sizes, one frame findings run per screen and
//! size, and every invariant of `FRAME_INVARIANTS` run on it. Nothing else walks the list. A screen is entered afresh
//! at each size: a menu or popup that a resize would close is then still open when it is checked.
//!
//! To add an invariant that reads one frame: write a `fn(&Frame) -> Vec<Hit>` and add it to `FRAME_INVARIANTS` (add
//! to `Frame` what it reads). To add one that needs more than a frame (keys pressed on the screen): write a
//! `fn(&Screen) -> Vec<Hit>` that enters the screen itself and add it to `SCREEN_HOOKS`; it runs once per screen,
//! after the screen's three sizes. A hit the tree has today goes in `ALLOWED` with a reason; a hit not in it fails the
//! walk, and so does an entry with no hit.

use crate::checks::{run_checks, Finding};
use crate::frame::snapshot;
use crate::screens::{enter, Screen, SCREENS};
use eframe::egui::{vec2, Vec2};
use egui_kittest::Harness;

/// The window sizes every screen is visited at: a narrow laptop, the common case, a large monitor.
pub const SIZES: [Vec2; 3] = [
    vec2(1100.0, 800.0),
    vec2(1280.0, 800.0),
    vec2(1600.0, 1000.0),
];

/// One frame of one screen at one size.
pub struct Frame<'a> {
    pub screen: &'a str,
    pub size: Vec2,
    /// `checks::run_checks` of the frame, run once for every invariant to filter.
    pub findings: &'a [Finding],
}

/// One broken rule: where (screen, size), which invariant, what (the widget or text), and the rule's message.
#[derive(Debug, Clone)]
pub struct Hit {
    pub screen: String,
    pub size: Vec2,
    pub invariant: &'static str,
    pub what: String,
    pub message: String,
}

impl Hit {
    fn text(&self) -> String {
        format!(
            "{} at {}x{}: {} {:?}: {}",
            self.screen, self.size.x, self.size.y, self.invariant, self.what, self.message
        )
    }
}

type FrameInvariant = fn(&Frame) -> Vec<Hit>;
type ScreenHook = fn(&Screen) -> Vec<Hit>;

/// Invariants that read the one frame of a screen at a size.
pub const FRAME_INVARIANTS: &[FrameInvariant] = &[nothing_past_the_window_edge, no_clipped_text];

/// Invariants that run once per screen after its sizes; each enters the screen itself.
pub const SCREEN_HOOKS: &[ScreenHook] = &[];

/// The findings of `check`, as hits with the rule's message and what to do instead.
fn from_findings(
    f: &Frame,
    check: &'static str,
    invariant: &'static str,
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
            message: format!(
                "{} {:?}: {} ({}); {instead}",
                x.subject.role, x.subject.rect, x.detail, x.rule
            ),
        })
        .collect()
}

/// Every interactive widget is inside the window's width.
fn nothing_past_the_window_edge(f: &Frame) -> Vec<Hit> {
    from_findings(
        f,
        "off_window",
        "window_edge",
        "let the row wrap or scroll, or give the widget a smaller minimum width, so it fits at this size",
    )
}

/// No text is cut by a clip rect.
fn no_clipped_text(f: &Frame) -> Vec<Hit> {
    from_findings(
        f,
        "clipped_text",
        "clipped_text",
        "truncate the label with its full text on hover, or let it wrap, instead of clipping it",
    )
}

/// The three sizes of one screen: `enter` it at each size, snapshot once, run every frame invariant on the snapshot.
/// Generic over the harness so a test-only entry can walk through the same code.
pub fn visit_sizes<'h, S>(screen: &str, enter: &mut dyn FnMut(Vec2) -> Harness<'h, S>) -> Vec<Hit> {
    let mut hits = vec![];
    for size in SIZES {
        let h = enter(size);
        let findings = run_checks(&snapshot(&h));
        let frame = Frame {
            screen,
            size,
            findings: &findings,
        };
        for invariant in FRAME_INVARIANTS {
            hits.extend(invariant(&frame));
        }
    }
    hits
}

/// Every screen of the list at every size, then its hooks. Returns every hit, allowed or not.
pub fn walk(screens: &[Screen]) -> Vec<Hit> {
    let mut hits = vec![];
    for s in screens {
        // the temp folder lives as long as the harness the closure returns; the next call replaces both
        let mut dir = None;
        hits.extend(visit_sizes(s.name, &mut |size| {
            let (d, h) = enter(s, size);
            dir = Some(d);
            h
        }));
        for hook in SCREEN_HOOKS {
            hits.extend(hook(s));
        }
    }
    hits
}

/// One allowed hit: the screen, the invariant, a part of the widget's name or text, and why it is allowed today. It
/// covers the hit at every size.
pub struct Allowed {
    pub screen: &'static str,
    pub invariant: &'static str,
    pub what: &'static str,
    pub reason: &'static str,
}

/// Hits the tree has today. Each is a real fault in `crates/gui/src` that the task adding the invariant did not fix;
/// delete the entry when it is fixed.
pub const ALLOWED: &[Allowed] = &[
    Allowed {
        screen: "presets tab",
        invariant: "window_edge",
        what: "Preset target",
        reason: "at 1100 wide the target combo ends 78 px past the right edge (x 1078..1178)",
    },
    Allowed {
        screen: "json editor",
        invariant: "window_edge",
        what: "derived from the set",
        reason: "at 1100 wide the two rows' \"derived from the set\" tags end 92 and 107 px past the right edge",
    },
    Allowed {
        screen: "json editor",
        invariant: "clipped_text",
        what: "derived from the set",
        reason: "at 1100 wide the same two tags stick out of their clip by 99.7 and 115.3 px",
    },
];

/// The hits that are not allowed, and the allowed entries no hit matched; empty when the walk passes.
pub fn failures(hits: &[Hit], allowed: &[Allowed]) -> Vec<String> {
    let is = |a: &Allowed, h: &Hit| {
        a.screen == h.screen && a.invariant == h.invariant && h.what.contains(a.what)
    };
    let mut out: Vec<String> = hits
        .iter()
        .filter(|h| !allowed.iter().any(|a| is(a, h)))
        .map(|h| {
            format!(
                "{}; if it cannot be fixed now, add it to ALLOWED in crates/gui/tests/ui_proof/invariants.rs with a reason",
                h.text()
            )
        })
        .collect();
    out.extend(
        allowed
            .iter()
            .filter(|a| !hits.iter().any(|h| is(a, h)))
            .map(|a| {
                format!(
                    "ALLOWED in crates/gui/tests/ui_proof/invariants.rs holds {} / {} / {:?} ({}), which no longer hits; delete the entry",
                    a.screen, a.invariant, a.what, a.reason
                )
            }),
    );
    out
}

#[test]
fn the_walk_finds_nothing_new_on_any_screen() {
    let start = std::time::Instant::now();
    let hits = walk(SCREENS);
    println!(
        "walk: {} screens x {} sizes in {:.1?}, {} hits",
        SCREENS.len(),
        SIZES.len(),
        start.elapsed(),
        hits.len()
    );
    let bad = failures(&hits, ALLOWED);
    assert!(bad.is_empty(), "{bad:#?}");
}

/// A tiny UI with a button that ends past the narrowest window's edge and a label cut by its clip: not in `SCREENS`.
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

/// Both findings fire through `visit_sizes`, the code the walk uses, and the failure names screen, size and widget.
/// The entry is a tiny UI, not a `Screen`: a `Screen` opens the real app, which has no widget to plant.
#[test]
fn a_widget_past_the_edge_and_a_clipped_text_fail_the_walk() {
    let hits = visit_sizes("test-only faulty screen", &mut |size| {
        let mut h = Harness::builder()
            .with_context_setup(|ctx| {
                sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
            })
            .with_size(size)
            .build_ui(faulty_ui);
        h.run_steps(3);
        h
    });
    let bad = failures(&hits, &[]);
    println!("{bad:#?}");
    let of = |inv: &str, what: &str| {
        hits.iter()
            .filter(|x| x.invariant == inv && x.what == what)
            .map(|x| x.size.x)
            .collect::<Vec<_>>()
    };
    // 1000 + 200 = 1200: past the 1100 window only
    assert_eq!(of("window_edge", "Far away"), vec![1100.0]);
    // cut by its clip at every size
    assert_eq!(
        of("clipped_text", "cut label sticking out"),
        vec![1100.0, 1280.0, 1600.0]
    );
    assert!(
        bad.iter().any(|m| m
            .starts_with("test-only faulty screen at 1100x800: window_edge \"Far away\": Button"))
            && bad.iter().all(|m| m.contains("ALLOWED")),
        "{bad:#?}"
    );
}

/// An allowed entry covers its hit at every size, and one that no longer hits is a failure to delete.
#[test]
fn an_allowed_hit_passes_and_a_stale_entry_fails() {
    let hit = |invariant, what: &str| Hit {
        screen: "s".into(),
        size: SIZES[0],
        invariant,
        what: what.into(),
        message: String::new(),
    };
    let hits = [hit("window_edge", "a long name")];
    let entry = |invariant, what| Allowed {
        screen: "s",
        invariant,
        what,
        reason: "r",
    };
    assert!(failures(&hits, &[entry("window_edge", "long")]).is_empty());
    assert_eq!(failures(&hits, &[]).len(), 1);
    let stale = failures(
        &hits,
        &[entry("window_edge", "long"), entry("clipped_text", "b")],
    );
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert!(stale[0].contains("delete the entry"), "{stale:?}");
}
