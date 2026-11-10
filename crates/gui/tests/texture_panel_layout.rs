//! Note HI1: the texture panel's rows share one label column and one control column, at every width it is drawn at
//! (the centre, the 3D settings window, the preview's 280 px strip).
mod support;
use eframe::egui::{self, accesskit::Role, Rect};
use egui_kittest::kittest::{by, NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::{ColorTag, Format, Subtype, Target};
use sdc_gui::model::fields::format_name;
use sdc_gui::model::{Answer, AppModel, Selection};
use sdc_gui::ui::settings_panel::{COMPACT_CONTROL_W, CONTROL_W};
use std::time::Duration;
use support::*;

const KEY: &str = "landscape/rock01.png";

const COMBOS: [&str; 12] = [
    "Sub-type",
    "Input content",
    "DDS tag",
    "Alpha",
    "Comparison",
    "Filter",
    "Edge",
    "Normal handling",
    "Normal base level",
    "Format",
    "Effort",
    "BC7 weighting",
];
const CHECKS: [&str; 2] = ["Exact opaque alpha", "Mipmaps"];

/// The fixture's rock01 decided as a CS diffuse (BC7) and selected.
fn model(dir: &std::path::Path) -> AppModel {
    let mut m = opened(dir);
    m.set_target(Target::Cs);
    m.decide(KEY, Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(KEY);
    m
}

fn harness(m: AppModel, w: f32) -> Harness<'static, AppModel> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(w, 800.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h
}

fn centre(h: &Harness<'static, AppModel>) -> Rect {
    sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn")
}

/// The one `role` node named `name` whose centre is inside `area`.
fn control<S>(h: &Harness<'_, S>, role: Role, name: &str, area: Rect) -> Rect {
    let found: Vec<Rect> = h
        .query_all(by().role(role).label(name))
        .map(|n| n.rect())
        .filter(|r| area.contains(r.center()))
        .collect();
    assert_eq!(found.len(), 1, "{role:?} {name:?} in {area:?}: {found:?}");
    found[0]
}

fn combos(h: &Harness<'static, AppModel>, area: Rect) -> Vec<(&'static str, Rect)> {
    COMBOS
        .iter()
        .map(|n| (*n, control(h, Role::ComboBox, n, area)))
        .collect()
}

fn controls(h: &Harness<'static, AppModel>, area: Rect) -> Vec<(&'static str, Rect)> {
    let mut v = combos(h, area);
    v.extend(
        CHECKS
            .iter()
            .map(|n| (*n, control(h, Role::CheckBox, n, area))),
    );
    v
}

#[test]
fn every_control_starts_at_the_same_x() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(model(d.path()), 1280.0);
    let all = controls(&h, centre(&h));
    let x = all[0].1.min.x;
    for (n, r) in &all {
        assert!(
            (r.min.x - x).abs() <= 1.0,
            "{n} starts at {}, Sub-type at {x}",
            r.min.x
        );
    }
}

/// Note HI4: the Helper tick is the row under Sub-type, in the control column.
#[test]
fn the_helper_tick_sits_under_sub_type() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(model(d.path()), 1280.0);
    let c = centre(&h);
    let sub = control(&h, Role::ComboBox, "Sub-type", c);
    let tick = control(&h, Role::CheckBox, "Helper", c);
    assert!(
        (tick.min.x - sub.min.x).abs() <= 1.0,
        "tick {tick:?}, Sub-type {sub:?}"
    );
    assert!(
        tick.min.y >= sub.max.y,
        "tick {tick:?} below Sub-type {sub:?}"
    );
    // The next row: nothing else of the Type group between them.
    assert!(
        tick.min.y - sub.max.y < 2.0 * sub.height(),
        "tick {tick:?} right under Sub-type {sub:?}"
    );
}

#[test]
fn every_dropdown_has_the_same_width() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(model(d.path()), 1280.0);
    for (n, r) in combos(&h, centre(&h)) {
        assert!(
            (r.width() - CONTROL_W).abs() <= 1.0,
            "{n} is {} wide",
            r.width()
        );
    }
}

#[test]
fn mipmaps_tick_is_in_the_control_column() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(model(d.path()), 1280.0);
    let c = centre(&h);
    let tick = control(&h, Role::CheckBox, "Mipmaps", c);
    let filter = control(&h, Role::ComboBox, "Filter", c);
    assert!(
        (tick.min.x - filter.min.x).abs() <= 1.0,
        "tick {tick:?}, Filter {filter:?}"
    );
    let label = control(&h, Role::Label, "Mipmaps", c);
    assert!(
        label.max.x <= tick.min.x,
        "label {label:?} left of the tick {tick:?}"
    );
    assert!(
        label.center().y > tick.min.y && label.center().y < tick.max.y,
        "one row"
    );
}

#[test]
fn a_long_value_does_not_widen_the_column() {
    let d = tempfile::tempdir().unwrap();
    let mut m = model(d.path());
    // A CS diffuse is linear only: BC7 _SRGB is kept, and marked.
    m.set_format(KEY, Format::Bc7, ColorTag::Srgb);
    assert!(m.wait_idle(Duration::from_secs(60)));
    let h = harness(m, 1280.0);
    let c = centre(&h);
    let node = h
        .query_all(by().role(Role::ComboBox).label("Format"))
        .find(|n| c.contains(n.rect().center()))
        .expect("the Format combo");
    assert!(
        node.value().is_some_and(|v| v.contains("not valid")),
        "{:?}",
        node.value()
    );
    let w = node.rect().width();
    assert!((w - CONTROL_W).abs() <= 1.0, "Format is {w} wide");
}

/// With the Advanced drawer open too: the centre is then narrower than two full columns, and a row as wide as it
/// widened the centre under the drawer.
#[test]
fn rows_stay_inside_the_column_at_1100() {
    for drawer in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let mut m = model(d.path());
        m.settings.advanced_open = drawer;
        let h = harness(m, 1100.0);
        let c = centre(&h);
        let edge = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("advanced"))
            .filter(|_| drawer)
            .map_or(c.max.x, |p| p.outer_rect.min.x);
        assert!(
            c.max.x <= edge + 0.5,
            "the centre {c:?} runs under the drawer at {edge}"
        );
        for (n, r) in controls(&h, c) {
            assert!(
                r.max.x <= edge,
                "drawer {drawer}: {n} ends at {}, the centre at {edge}",
                r.max.x
            );
        }
        // Scaled down or not, the column keeps one width.
        let all = combos(&h, c);
        for (n, r) in &all {
            assert!(
                (r.width() - all[0].1.width()).abs() <= 1.0,
                "drawer {drawer}: {n} is {} wide, Sub-type {}",
                r.width(),
                all[0].1.width()
            );
        }
    }
}

/// Screens for a layout review (Note HI): the panel at 1100, 1280 and 1600 px, the strip and the 3D settings window,
/// cropped to each, written to `SDC_PANEL_SHOTS`. GPU (`render_`): skipped unless that variable names a folder.
#[test]
fn render_texture_panel_shots() {
    let Some(out) = std::env::var_os("SDC_PANEL_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    let save = |img: image::RgbaImage, ppp: f32, r: Rect, name: &str| {
        let (x, y) = ((r.min.x * ppp) as u32, (r.min.y * ppp) as u32);
        let w = ((r.width() * ppp) as u32).min(img.width() - x);
        let h = ((r.height() * ppp) as u32).min(img.height() - y);
        image::imageops::crop_imm(&img, x, y, w, h)
            .to_image()
            .save(out.join(name))
            .unwrap();
    };
    for w in [1100.0, 1280.0, 1600.0] {
        let d = tempfile::tempdir().unwrap();
        let mut h = harness(model(d.path()), w);
        let c = centre(&h);
        let ppp = h.ctx.pixels_per_point();
        save(
            h.render().expect("render"),
            ppp,
            c,
            &format!("panel-{w}.png"),
        );
    }
    let d = tempfile::tempdir().unwrap();
    let (mut h, strip) = strip_harness(d.path());
    let ppp = h.ctx.pixels_per_point();
    save(h.render().expect("render"), ppp, strip, "strip-280.png");
    let d = tempfile::tempdir().unwrap();
    let mut h = window_harness(d.path());
    let win = h
        .ctx
        .memory(|m| m.area_rect(sdc_gui::ui::set_preview::settings_window_id()))
        .unwrap();
    let ppp = h.ctx.pixels_per_point();
    save(h.render().expect("render"), ppp, win, "window-460.png");
}

/// Screens of the Helper row's states (Note HI4) at 1100 and 1600 px, cropped to the centre, written to
/// `SDC_PANEL_SHOTS`. GPU (`render_`): skipped unless that variable names a folder.
#[test]
fn render_helper_tick_shots() {
    let Some(out) = std::env::var_os("SDC_PANEL_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    type Setup = fn(&mut AppModel) -> bool;
    let states: [(&str, Setup); 5] = [
        ("unticked", |_| false),
        ("ticked", |_| true),
        ("helper", |m| {
            m.select_item("landscape/rock01_roughness.png");
            false
        }),
        ("none", |m| {
            m.set_helper_role(KEY, Some(engine::helper::HelperRole::NotHelper));
            assert!(m.wait_idle(Duration::from_secs(60)));
            false
        }),
        ("jobless", |m| {
            let i = m.items.iter().position(|i| i.key == KEY).unwrap();
            m.items[i].status = sdc_gui::model::items::ItemStatus::Failed("no good".into());
            m.items[i].job = None;
            false
        }),
    ];
    for w in [1100.0, 1600.0] {
        for (name, setup) in states {
            let d = tempfile::tempdir().unwrap();
            let mut m = model(d.path());
            let tick = setup(&mut m);
            let mut h = harness(m, w);
            if tick {
                h.get_by_role_and_label(Role::CheckBox, "Helper").click();
                h.run();
            }
            let c = centre(&h);
            let ppp = h.ctx.pixels_per_point();
            let img = h.render().expect("render");
            let (x, y) = ((c.min.x * ppp) as u32, (c.min.y * ppp) as u32);
            let cw = ((c.width() * ppp) as u32).min(img.width() - x);
            let ch = ((c.height() * ppp) as u32).min(img.height() - y);
            image::imageops::crop_imm(&img, x, y, cw, ch)
                .to_image()
                .save(out.join(format!("helper-{name}-{w}.png")))
                .unwrap();
        }
    }
}

/// The helper panel shares the columns: with the drawer open at 1100 px it stays inside the centre too.
#[test]
fn helper_rows_stay_inside_at_1100() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.settings.advanced_open = true;
    m.select_item("landscape/rock01_roughness.png");
    let h = harness(m, 1100.0);
    h.get_by_role_and_label(Role::ComboBox, "Helper role");
    let c = centre(&h);
    let drawer = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("advanced"))
        .expect("the drawer is drawn")
        .outer_rect;
    assert!(
        c.max.x <= drawer.min.x + 0.5,
        "the centre {c:?} runs under the drawer {drawer:?}"
    );
}

const LEAF: &str = "landscape/leaf.png";

/// The fixture's leaf.png decided as a diffuse, so its alpha is asked (proposed Cutout), and selected.
fn leaf(dir: &std::path::Path) -> AppModel {
    let mut m = opened(dir);
    m.decide(LEAF, Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m
        .item(LEAF)
        .unwrap()
        .decisions
        .contains(&sdc_gui::model::items::Decision::AlphaMeaning));
    m.select_item(LEAF);
    m
}

fn idle(h: &mut Harness<'static, AppModel>) {
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run();
}

fn leaf_policy(h: &Harness<'static, AppModel>) -> Option<serde_json::Value> {
    h.state()
        .project
        .as_ref()
        .unwrap()
        .file_value(LEAF, "alpha.policy")
}

/// Note HI13: the Undecided box's alpha question stands in the Alpha row's columns.
#[test]
fn alpha_row_and_undecided_line_up() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(leaf(d.path()), 1280.0);
    let c = centre(&h);
    let asked = control(&h, Role::ComboBox, "Alpha type: Cutout", c);
    let row = control(&h, Role::ComboBox, "Alpha", c);
    assert!(
        (asked.min.x - row.min.x).abs() <= 1.0,
        "the question's dropdown {asked:?}, the Alpha row's {row:?}"
    );
    assert!(
        (asked.width() - row.width()).abs() <= 1.0,
        "the question's dropdown {asked:?}, the Alpha row's {row:?}"
    );
    let labels: Vec<Rect> = h
        .query_all(by().role(Role::Label).label("Alpha"))
        .map(|n| n.rect())
        .filter(|r| c.contains(r.center()))
        .collect();
    assert!(
        labels.len() >= 2,
        "the box's and the row's labels: {labels:?}"
    );
    for l in labels {
        assert!(l.max.x <= row.min.x, "label {l:?} runs into {row:?}");
    }
}

/// Note HI14: while the alpha is asked, the rows read the proposal.
#[test]
fn alpha_row_shows_the_proposal() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(leaf(d.path()), 1280.0);
    let c = centre(&h);
    let row = h
        .query_all(by().role(Role::ComboBox).label("Alpha"))
        .find(|n| c.contains(n.rect().center()))
        .expect("the Alpha row");
    assert_eq!(row.value().as_deref(), Some("Cutout"));
    h.get_by_label("proposed: confirm above");
    let slider = h.get_by_role_and_label(Role::Slider, "Alpha threshold");
    assert!(
        !slider.accesskit_node().is_disabled(),
        "the threshold is off"
    );
}

/// Note HI14: the proposal is display only.
#[test]
fn the_proposal_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let m = leaf(d.path());
    let dirty = m.is_dirty();
    let mut h = harness(m, 1280.0);
    h.run();
    assert_eq!(leaf_policy(&h), None);
    assert_eq!(h.state().is_dirty(), dirty, "the document changed");
    assert!(!h.state().busy(), "a plan started");
    assert!(h.state().undecided_keys().iter().any(|k| k == LEAF));
}

/// Note HI14: a pick on the Alpha row is an answer.
#[test]
fn picking_in_the_row_answers_the_question() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(leaf(d.path()), 1280.0);
    let c = centre(&h);
    h.query_all(by().role(Role::ComboBox).label("Alpha"))
        .find(|n| c.contains(n.rect().center()))
        .expect("the Alpha row")
        .click();
    h.run();
    h.get_by_label("Blend").click();
    h.run();
    idle(&mut h);
    assert_eq!(leaf_policy(&h), Some(serde_json::json!("blend")));
    assert!(h.query_by_label("Undecided").is_none());
}

/// Note HI13: the sub-type question's combo stands in the control column too.
#[test]
fn the_sub_type_question_lines_up_too() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.select_item(KEY);
    let h = harness(m, 1280.0);
    let c = centre(&h);
    let q = h.get_by_label("What kind of texture is this?").rect();
    let combo = h
        .query_all_by_role(Role::ComboBox)
        .map(|n| n.rect())
        .filter(|r| r.min.y >= q.max.y && c.contains(r.center()))
        .min_by(|a, b| a.min.y.total_cmp(&b.min.y))
        .expect("the sub-type question's combo");
    let row = control(&h, Role::ComboBox, "Alpha", c);
    assert!(
        (combo.min.x - row.min.x).abs() <= 1.0,
        "the question's combo {combo:?}, the Alpha row's {row:?}"
    );
}

/// Note HI7: Confirm answers with the shown type.
#[test]
fn confirm_answers_the_alpha_question() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(leaf(d.path()), 1280.0);
    h.get_by_label("Confirm alpha").click();
    h.run();
    idle(&mut h);
    assert_eq!(leaf_policy(&h), Some(serde_json::json!("cutout")));
    assert!(h.query_by_label("Undecided").is_none());
}

/// Note FK: Confirm waits for the plan, as the dropdown does.
#[test]
fn confirm_is_off_while_a_plan_is_pending() {
    let d = tempfile::tempdir().unwrap();
    let mut m = leaf(d.path());
    // Another texture's answer starts a plan; `ui::draw` alone never lands it.
    m.decide(KEY, Answer::Subtype(Subtype::Diffuse));
    assert!(m.fix_off().is_some());
    let h = harness(m, 1280.0);
    assert!(h
        .get_by_label("Confirm alpha")
        .accesskit_node()
        .is_disabled());
}

/// Note HI15: a texture in no set (no Problems or Review row) answers its alpha in the panel.
#[test]
fn a_no_set_image_confirms_its_alpha() {
    let d = tempfile::tempdir().unwrap();
    let mut m = leaf(d.path());
    m.remove_member("landscape/leaf", LEAF).expect("removed");
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.is_no_set(LEAF));
    m.select_item(LEAF);
    let mut h = harness(m, 1280.0);
    h.get_by_label("Confirm alpha").click();
    h.run();
    idle(&mut h);
    assert_eq!(leaf_policy(&h), Some(serde_json::json!("cutout")));
    assert!(h.query_by_label("Undecided").is_none());
}

/// Screens of the alpha question (Note HI7, HI13) at 1100 and 1600 px: the panel, and the Review and Problems rows,
/// written to `SDC_PANEL_SHOTS`. GPU (`render_`): skipped unless that variable names a folder.
#[test]
fn render_alpha_question_shots() {
    let Some(out) = std::env::var_os("SDC_PANEL_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    for w in [1100.0, 1600.0] {
        for view in ["panel", "review", "problems"] {
            let d = tempfile::tempdir().unwrap();
            let mut m = leaf(d.path());
            match view {
                "review" => m.open_review(),
                "problems" => m.open_problems(None),
                _ => {}
            }
            let mut h = harness(m, w);
            h.run();
            let area = match view {
                "panel" => centre(&h),
                _ => {
                    let n = if view == "review" {
                        "Review"
                    } else {
                        "Problems"
                    };
                    let r = h.get_by_label(&format!("{n} confirm alpha")).rect();
                    Rect::from_min_max(
                        egui::pos2(0.0, r.min.y - 60.0),
                        egui::pos2(w, r.max.y + 60.0),
                    )
                }
            };
            let ppp = h.ctx.pixels_per_point();
            let img = h.render().expect("render");
            let (x, y) = (
                (area.min.x * ppp) as u32,
                (area.min.y.max(0.0) * ppp) as u32,
            );
            let cw = ((area.width() * ppp) as u32).min(img.width() - x);
            let ch = ((area.height() * ppp) as u32).min(img.height() - y);
            image::imageops::crop_imm(&img, x, y, cw, ch)
                .to_image()
                .save(out.join(format!("alpha-{view}-{w}.png")))
                .unwrap();
        }
    }
}

type H3 = Harness<'static, (AppModel, sdc_gui::ui::State)>;

/// AG132: a set member selected in the 3D view shows its settings in the floating window, at its default size.
fn window_harness(dir: &std::path::Path) -> H3 {
    use sdc_gui::model::pbr_export::json_entries;
    use sdc_gui::model::set_preview::{followed_set, SetPreview, DEFAULT_CAP};
    let mut m = pbr_opened_plain(dir);
    let key = json_entries(&m)
        .into_iter()
        .find(|(v, _)| v.id == "landscape/rock01")
        .unwrap()
        .0
        .key;
    m.select_set(&key);
    m.settings.set_view_3d = true;
    let set = followed_set(&m).unwrap();
    let item = m
        .items
        .iter()
        .find(|i| i.set_key.as_deref() == Some(set.as_str()))
        .unwrap()
        .key
        .clone();
    m.selection = Selection::Item(item);
    m.settings.set_settings_open = true;
    let state = sdc_gui::ui::State {
        set3d: SetPreview::new(None, DEFAULT_CAP),
        ..Default::default()
    };
    let mut h = Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, sdc_gui::ui::State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, state),
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    // The 3D view never idles: step and poll, as set_preview_layout does.
    for _ in 0..6 {
        h.run_steps(3);
        let (m, s) = h.state_mut();
        s.set3d.poll(&*m);
    }
    h
}

#[test]
fn the_settings_window_keeps_its_rows_inside() {
    let d = tempfile::tempdir().unwrap();
    let h = window_harness(d.path());
    let win = h
        .ctx
        .memory(|m| m.area_rect(sdc_gui::ui::set_preview::settings_window_id()))
        .expect("the settings window is drawn");
    assert!(win.width() < 480.0, "the default 460 px: {win:?}");
    let inside = win.max.x - egui::Style::default().spacing.window_margin.rightf();
    let mut n = 0;
    let named = COMBOS
        .iter()
        .map(|c| (Role::ComboBox, *c))
        .chain(CHECKS.iter().map(|c| (Role::CheckBox, *c)));
    for (role, name) in named {
        for r in h
            .query_all(by().role(role).label(name))
            .map(|n| n.rect())
            .filter(|r| win.contains(r.min))
        {
            n += 1;
            assert!(
                r.max.x <= inside,
                "{name} ends at {}, the window's content at {inside}",
                r.max.x
            );
        }
    }
    assert!(n >= 8, "the window's rows are named: {n} found");
}

/// The fixture's rock01 expanded into the preview at 1600 px, where the strip starts open; its rect.
fn strip_harness(dir: &std::path::Path) -> (Harness<'static, AppModel>, Rect) {
    let mut m = model(dir);
    m.expand(KEY);
    assert!(m.wait_idle(Duration::from_secs(60)));
    let h = harness(m, 1600.0);
    let strip =
        egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("preview-settings"))
            .expect("the settings strip is open at 1600 wide")
            .outer_rect;
    (h, strip)
}

#[test]
fn compact_controls_have_one_width() {
    let d = tempfile::tempdir().unwrap();
    let (h, strip) = strip_harness(d.path());
    let all: Vec<Rect> = h
        .query_all_by_role(Role::ComboBox)
        .map(|n| n.rect())
        .filter(|r| strip.contains(r.center()))
        .collect();
    assert!(all.len() >= 4, "{all:?}");
    for r in all {
        assert!(
            (r.width() - COMPACT_CONTROL_W).abs() <= 1.0,
            "a combo is {} wide: {r:?}",
            r.width()
        );
        assert!(
            r.max.x <= strip.max.x,
            "{r:?} ends past the strip {strip:?}"
        );
    }
}

/// A pin: the strip never showed the colour rows, and now they are in Compression, which it shows.
#[test]
fn compact_strip_has_no_colour_rows() {
    let d = tempfile::tempdir().unwrap();
    let (h, strip) = strip_harness(d.path());
    assert!(h
        .query_all_by_label("Compression")
        .any(|n| strip.contains(n.rect().center())));
    for name in ["Input content", "DDS tag"] {
        assert!(
            h.query_by_label_contains(name).is_none(),
            "{name} in the strip"
        );
    }
}

#[test]
fn the_format_dropdown_shows_no_tag() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(model(d.path()), 1280.0);
    let c = centre(&h);
    let node = h
        .query_all(by().role(Role::ComboBox).label("Format"))
        .find(|n| c.contains(n.rect().center()))
        .expect("the Format combo");
    assert_eq!(node.value().as_deref(), Some("BC7"));
}

#[test]
fn picking_a_format_keeps_the_tag() {
    // A CS sub-type with two sRGB formats (most lists have one tag throughout).
    let (st, srgb) = Subtype::ALL
        .into_iter()
        .map(|st| {
            let a = engine::policy::allowed_encodings_for(st, Target::Cs, false);
            let srgb: Vec<Format> = a
                .iter()
                .filter(|(_, t)| *t == ColorTag::Srgb)
                .map(|(f, _)| *f)
                .collect();
            (st, srgb)
        })
        .find(|(_, s)| s.len() >= 2)
        .expect("a sub-type with two sRGB formats");
    let d = tempfile::tempdir().unwrap();
    let mut m = model(d.path());
    m.decide(KEY, Answer::Subtype(st));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_format(KEY, srgb[0], ColorTag::Srgb);
    assert!(m.wait_idle(Duration::from_secs(60)));
    let mut h = harness(m, 1280.0);
    let c = centre(&h);
    let combo = h
        .query_all(by().role(Role::ComboBox).label("Format"))
        .find(|n| c.contains(n.rect().center()))
        .expect("the Format combo");
    combo.click();
    h.run();
    h.get_by_role_and_label(Role::Button, format_name(srgb[1]))
        .click();
    h.run();
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    let p = &h.state().item(KEY).unwrap().job.as_ref().unwrap().profile;
    assert_eq!((p.format, p.color.tag), (srgb[1], ColorTag::Srgb), "{st:?}");
}
