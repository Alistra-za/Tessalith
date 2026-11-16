//! Note JI7 (egui-ui-rules 3.2): the centre's three scrolling panels, the texture panel, the bulk panel and the set
//! editor, draw their vertical scroll bar with no pointer over them. egui gives a bar no AccessKit node, so these read
//! the paint: the bar's background and handle are filled rects at the centre's right edge, transparent while the
//! default floating style is dormant.
mod support;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use sdc_gui::model::AppModel;
use std::time::Duration;
use support::*;

type H = Harness<'static, AppModel>;

fn harness(model: AppModel, size: egui::Vec2) -> H {
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(size)
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), model);
    h.run();
    h
}

/// The centre column, cut at the window.
fn centre(h: &H) -> egui::Rect {
    sdc_gui::ui::centre_rect(&h.ctx)
        .expect("the centre is drawn")
        .intersect(h.ctx.content_rect())
}

/// Every filled rect a vertical bar could be: at most 12 px wide, at least 20 px tall, ending within 20 px of the
/// centre's right edge. (rect, fill alpha).
fn bar_rects(h: &H) -> Vec<(egui::Rect, u8)> {
    bar_rects_in(h, centre(h))
}

/// `bar_rects` at the right edge of `c`, any panel.
fn bar_rects_in(h: &H, c: egui::Rect) -> Vec<(egui::Rect, u8)> {
    fn walk(s: &egui::Shape, out: &mut Vec<(egui::Rect, u8)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            egui::Shape::Rect(r) => out.push((r.rect, r.fill.a())),
            _ => {}
        }
    }
    let mut all = vec![];
    for s in &h.output().shapes {
        walk(&s.shape, &mut all);
    }
    all.into_iter()
        .filter(|(r, _)| {
            r.width() <= 12.0
                && r.height() >= 20.0
                && r.max.x <= c.max.x + 0.5
                && r.max.x >= c.max.x - 20.0
        })
        .collect()
}

/// The panel's content runs past the centre's bottom (so a bar is due), and a bar is painted there, visible, with no
/// pointer in the window.
fn assert_bar_at_rest(h: &H, what: &str) {
    let c = centre(h);
    let lowest = h
        .query_all_by_role(egui::accesskit::Role::Button)
        .chain(h.query_all_by_role(egui::accesskit::Role::ComboBox))
        .chain(h.query_all_by_role(egui::accesskit::Role::Label))
        .map(|n| n.rect())
        .filter(|r| r.min.x >= c.min.x && r.max.x <= c.max.x)
        .map(|r| r.max.y)
        .fold(f32::MIN, f32::max);
    assert!(
        lowest > c.max.y,
        "fixture: the {what} must overflow ({lowest} <= {})",
        c.max.y
    );
    let bars = bar_rects(h);
    assert!(
        !bars.is_empty(),
        "the {what}'s bar is painted at the centre's right edge"
    );
    assert!(
        bars.iter().any(|(_, a)| *a > 0),
        "the {what}'s bar is visible at rest: {bars:?}"
    );
}

/// B1: the texture panel at 1100x800 (DDS header and the inspector fold run below the edge).
#[test]
fn the_texture_panel_shows_its_bar_at_rest() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.select_item("landscape/leaf.png");
    assert!(m.wait_idle(Duration::from_secs(60)));
    let h = harness(m, egui::vec2(1100.0, 800.0));
    assert_bar_at_rest(&h, "texture panel");
}

/// B2: the bulk panel, two textures selected, 1280 wide. The fixture's panel is shorter than the demo folder's
/// (739 px of the 768 a 1280x800 window gives it), so the window is 680 high.
#[test]
fn the_bulk_panel_shows_its_bar_at_rest() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item("landscape/leaf.png", false, false);
    m.click_item("landscape/rock01.png", true, false);
    assert!(m.in_bulk());
    let h = harness(m, egui::vec2(1280.0, 680.0));
    h.get_by_label("2 textures selected");
    assert_bar_at_rest(&h, "bulk panel");
}

/// B3 (CC21 regressed by FB2): the set editor at 1280x800.
#[test]
fn the_set_editor_shows_its_bar_at_rest() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with("landscape/rock01"))
        .expect("the rock set")
        .key
        .clone();
    m.select_set(&key);
    let mut h = harness(m, egui::vec2(1280.0, 800.0));
    h.run_steps(4);
    assert_bar_at_rest(&h, "set editor");
}

/// A side column's outer rect, cut at the window.
fn column(h: &H, id: &str) -> egui::Rect {
    support::panel(&h.ctx, id).intersect(h.ctx.content_rect())
}

/// `opened` with `n` more diffuse textures, each a set of its own, so both columns run past a short window.
fn many(d: &std::path::Path, n: u8) -> AppModel {
    let r = d.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    for i in 0..n {
        image::RgbImage::from_fn(16, 16, |x, y| {
            image::Rgb([(x * 16) as u8, (y * 16) as u8, i])
        })
        .save(r.join(format!("stone{i:02}.png")))
        .unwrap();
    }
    let mut m = opened(d);
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

/// B5 (Note JJ2): the column's content runs past its bottom, and a bar with a visible fill is painted at its right
/// edge with no pointer in the window.
fn assert_column_bar_at_rest(h: &H, id: &str, what: &str) {
    let c = column(h, id);
    let lowest = h
        .query_all_by_role(egui::accesskit::Role::Button)
        .chain(h.query_all_by_role(egui::accesskit::Role::Label))
        .chain(h.query_all_by_role(egui::accesskit::Role::Image))
        .map(|n| n.rect())
        .filter(|r| r.min.x >= c.min.x && r.max.x <= c.max.x)
        .map(|r| r.max.y)
        .fold(f32::MIN, f32::max);
    assert!(
        lowest > c.max.y,
        "fixture: the {what} must overflow ({lowest} <= {})",
        c.max.y
    );
    let bars = bar_rects_in(h, c);
    assert!(
        !bars.is_empty(),
        "the {what}'s bar is painted at the column's right edge"
    );
    assert!(
        bars.iter().any(|(_, a)| *a > 0),
        "the {what}'s bar is visible at rest: {bars:?}"
    );
    // The right-edge reserves cover the bar: beside it, no control reaches the 10 px it takes when hovered, and a set
    // card (a frame whose own margin is wider than the rest) stops short of the bar at rest.
    use egui_kittest::kittest::NodeT;
    let track = bars
        .iter()
        .map(|(r, _)| *r)
        .max_by(|a, b| a.height().total_cmp(&b.height()))
        .unwrap();
    let hovered = track.max.x - sdc_gui::look::thin_scroll().bar_width;
    for n in h
        .query_all_by_role(egui::accesskit::Role::Button)
        .chain(h.query_all_by_role(egui::accesskit::Role::Image))
        .chain(h.query_all_by_role(egui::accesskit::Role::CheckBox))
    {
        let r = n.rect();
        if r.min.x < c.min.x
            || r.min.x >= c.max.x
            || r.min.y >= track.max.y
            || r.max.y <= track.min.y
        {
            continue;
        }
        let label = n.accesskit_node().label().unwrap_or_default();
        let limit = if label.starts_with("Set ") {
            track.min.x
        } else {
            hovered
        };
        assert!(
            r.max.x <= limit + 0.5,
            "{label:?} {r:?} reaches the {what}'s bar {track:?}"
        );
    }
}

/// B5: the Textures column at 1100x600, more tiles than fit.
#[test]
fn the_textures_column_shows_its_bar_at_rest() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(many(d.path(), 24), egui::vec2(1100.0, 600.0));
    h.run_steps(4);
    assert_column_bar_at_rest(&h, "textures", "Textures column");
}

/// B5: the Sets column at 1100x600, more set cards than fit.
#[test]
fn the_sets_column_shows_its_bar_at_rest() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(many(d.path(), 24), egui::vec2(1100.0, 600.0));
    h.run_steps(4);
    assert_column_bar_at_rest(&h, "sets", "Sets column");
}
