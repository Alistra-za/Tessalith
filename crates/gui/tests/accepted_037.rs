//! egui 0.37 behaviour the owner accepted, tested where Tessalith has the widget for it (owner rulings 3 to 5; the
//! others are in `set_order`, `problems_accept` and `popups`).
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT as _, Queryable};
use egui_kittest::Harness;
use engine::presets::AlsoTarget;
use image::{ImageBuffer, Rgba};
use sdc_gui::model::AppModel;
use std::path::Path;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(120);

/// PBR sets in the fixture: more than the 8 rows Review's "Skip these sets" draws inline, so the list scrolls in an
/// area of its own (`ui/review.rs::skip_list`).
const SETS: usize = 14;

fn png(path: &Path, px: [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    ImageBuffer::from_pixel(16, 16, Rgba(px))
        .save(path)
        .unwrap();
}

/// `SETS` PBR sets with Vanilla ticked and Review open, drawn 1100 wide: under `STACK_BELOW`, so Review's sections
/// stack in one scroll area and the skip list's area is nested in it.
fn reviewing(d: &Path) -> Harness<'static, AppModel> {
    let root = d.join("mods/Mod");
    for i in 0..SETS {
        let base = root.join(format!("textures/pbr/arm/s{i:02}"));
        let base = base.to_string_lossy();
        png(Path::new(&format!("{base}.png")), [200, 200, 200, 255]);
        png(Path::new(&format!("{base}_n.png")), [128, 128, 255, 255]);
        png(
            Path::new(&format!("{base}_rmaos.png")),
            [128, 255, 200, 255],
        );
    }
    let mut m = support::opened_as_the_app(root, d.join("out"));
    m.set_also_tick(AlsoTarget::Vanilla, true);
    assert!(m.wait_idle(WAIT));
    assert!(m.also_on(), "the tick took: {}", m.status_line);
    m.export_clicked();
    assert!(m.wait_idle(WAIT));
    assert!(m.review.is_some(), "Review is open");
    m.review.as_mut().unwrap().options_open = true;
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1100.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    assert!(h.state_mut().wait_idle(WAIT));
    h.run();
    h
}

/// The skip list's row for set `i` (its checkbox, named by the set id).
fn row(h: &Harness<'static, AppModel>, i: usize) -> Option<egui::Rect> {
    let end = format!("s{i:02}");
    h.query_all_by_role(Role::CheckBox)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.ends_with(&end))
        })
        .map(|n| n.rect())
}

/// Where the inner list stands: its first laid-out row's index and that row's top, relative to the label above it.
fn inner(h: &Harness<'static, AppModel>) -> Option<(usize, i32)> {
    let y = outer_y(h);
    (0..SETS).find_map(|i| row(h, i).map(|r| (i, (r.top() - y).round() as i32)))
}

/// The "Skip these sets" label above the list: it moves only when the outer area scrolls.
fn outer_y(h: &Harness<'static, AppModel>) -> f32 {
    h.get_by_label("Skip these sets").rect().top()
}

fn wheel(h: &mut Harness<'static, AppModel>, dy: f32) {
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, dy),
        source: egui::MouseWheelSource::Wheel,
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
    });
    // egui spreads a wheel turn over a few frames.
    h.run_steps(10);
}

/// Owner ruling (egui 0.37): a scroll area at its end passes the wheel on to the area it sits in.
#[test]
fn the_wheel_passes_to_the_outer_list_at_the_inner_lists_end() {
    let d = tempfile::tempdir().unwrap();
    let mut h = reviewing(d.path());
    // The list is the last thing in Review: bring its label into view, then wheel the outer area back over the label,
    // so the list runs on below the window and the outer area is not at its end. Then point at the list's first row.
    h.get_by_label("Skip these sets").scroll_to_me();
    h.run();
    let y = outer_y(&h);
    h.hover_at(h.get_by_label("Skip these sets").rect().center());
    wheel(&mut h, 60.0);
    assert!(outer_y(&h) > y + 30.0, "the outer area scrolled back");
    let first = row(&h, 0).expect("row 0 is laid out");
    h.hover_at(first.center());
    h.run();
    // Small turns of the wheel: while the inner list can scroll, it takes them and the outer area holds (the control);
    // once it is at its end, the next turn scrolls the outer area.
    let mut inner_turns = 0;
    let mut handed_off = false;
    for turn in 0..40 {
        let (y, before) = (outer_y(&h), inner(&h));
        // The last row has a node only once it is laid out: the list is a `show_rows`, which builds only the rows
        // in view. A list that laid out every row would make this true from the start.
        let at_end = row(&h, SETS - 1).is_some();
        wheel(&mut h, -10.0);
        let after = inner(&h);
        if outer_y(&h) < y - 1.0 {
            assert!(
                at_end,
                "turn {turn}: the outer area moved before the inner list reached its end"
            );
            handed_off = true;
            break;
        }
        assert_ne!(
            after, before,
            "turn {turn}: the inner list took the turn (label at {y})"
        );
        inner_turns += 1;
    }
    assert!(inner_turns > 0, "the inner list scrolled first");
    assert!(
        handed_off,
        "the outer area scrolled at the inner list's end"
    );
}
