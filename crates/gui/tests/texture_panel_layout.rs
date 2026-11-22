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

const COMBOS: [&str; 17] = [
    "Texture type",
    "Input content",
    "DDS tag",
    "Alpha",
    "Comparison",
    "Levels",
    "Filter",
    "Edge",
    "Normal handling",
    "Normal base level",
    "Resize",
    "Relative to",
    "Size floor",
    "Format",
    "Effort",
    "BC7 weighting",
    "DDS header",
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
    sized_harness(m, w, 800.0)
}

fn sized_harness(m: AppModel, w: f32, height: f32) -> Harness<'static, AppModel> {
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(w, height))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
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
            "{n} starts at {}, Type at {x}",
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
    let sub = control(&h, Role::ComboBox, "Texture type", c);
    let tick = control(&h, Role::CheckBox, "Helper", c);
    assert!(
        (tick.min.x - sub.min.x).abs() <= 1.0,
        "tick {tick:?}, Type {sub:?}"
    );
    assert!(tick.min.y >= sub.max.y, "tick {tick:?} below Type {sub:?}");
    // The next row: nothing else of the Type group between them.
    assert!(
        tick.min.y - sub.max.y < 2.0 * sub.height(),
        "tick {tick:?} right under Type {sub:?}"
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
    let m = model(d.path());
    // Every format the texture allows keeps the Format combo at its width. (A pair the type does not allow, whose
    // "(not valid for this map)" label was the longest value, is no longer written or kept, Note JG.)
    let pairs = sdc_gui::model::format_rule::allowed(Subtype::Diffuse, Target::Cs);
    assert!(pairs.len() > 5, "{pairs:?}");
    let mut h = harness(m, 1280.0);
    for (f, t) in pairs {
        h.state_mut().set_format(KEY, f, t);
        assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
        h.run();
        let c = centre(&h);
        let node = h
            .query_all(by().role(Role::ComboBox).label("Format"))
            .find(|n| c.contains(n.rect().center()))
            .expect("the Format combo");
        let w = node.rect().width();
        assert!(
            (w - CONTROL_W).abs() <= 1.0,
            "Format is {w} wide at {:?}",
            node.value()
        );
    }
}

/// At 1100 px the centre is narrower than two full columns: every row stays inside it, a row as wide as it does not
/// widen it past the window's edge (less the harness' 8 px margin), and the combos keep one width.
#[test]
fn rows_stay_inside_the_column_at_1100() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(model(d.path()), 1100.0);
    h.get_by_label("New project: nothing saved yet.");
    let c = centre(&h);
    let edge = h.ctx.content_rect().right() - 8.0;
    assert!(c.max.x <= edge + 0.5, "the centre {c:?} runs past {edge}");
    let mut combo_width = None;
    for (role, n) in COMBOS
        .map(|n| (Role::ComboBox, n))
        .into_iter()
        .chain(CHECKS.map(|n| (Role::CheckBox, n)))
    {
        h.query_all(by().role(role).label(n))
            .find(|n| n.rect().left() >= c.left())
            .unwrap()
            .scroll_to_me();
        h.run();
        let r = control(&h, role, n, c);
        assert!(
            r.max.x <= c.max.x,
            "{n} ends at {}, the centre at {}",
            r.max.x,
            c.max.x
        );
        if role == Role::ComboBox {
            let width = *combo_width.get_or_insert(r.width());
            assert!(
                (r.width() - width).abs() <= 1.0,
                "{n} is {} wide, Type {width}",
                r.width()
            );
        }
    }
}

/// Screens for a layout review (Note HI): the panel at 1100, 1280 and 1600 px, the strip and the 3D settings window,
/// cropped to each, written to `SDC_PANEL_SHOTS`. GPU (`render_`): skipped unless that variable names a folder.
#[test]
// Dev tooling, not a test: it asserts nothing and runs only with `SDC_PANEL_SHOTS` set.
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
// Dev tooling, not a test: it asserts nothing and runs only with `SDC_PANEL_SHOTS` set.
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

/// Note JI3: Levels in Mipmaps, a Size group (Resize, Relative to, Size floor) after it, and DDS header in
/// Compression, each drawn once and in the centre; BC7 weighting is one row.
#[test]
fn the_panel_shows_levels_a_size_group_and_the_dds_header() {
    let d = tempfile::tempdir().unwrap();
    let m = leaf(d.path());
    // Tall enough that the last group is inside the centre (AccessKit rects are not clipped).
    let h = sized_harness(m, 1280.0, 1600.0);
    let c = centre(&h);
    let header = |t: &str| {
        h.query_all_by_label(t)
            .find(|n| c.contains(n.rect().center()))
            .unwrap_or_else(|| panic!("{t} in the centre"))
            .rect()
    };
    // Group order: Mipmaps, Size, Compression.
    assert!(header("Mipmaps").top() < header("Size").top());
    assert!(header("Size").top() < header("Compression").top());
    for name in [
        "Levels",
        "Resize",
        "Relative to",
        "Size floor",
        "DDS header",
    ] {
        let n = h
            .query_all(by().role(Role::ComboBox).label(name))
            .filter(|n| c.contains(n.rect().center()))
            .count();
        assert_eq!(n, 1, "{name} once in the centre");
    }
    // The leaf is BC7. Count the row labels too: a row whose combo is unnamed would hide from the combo count.
    assert_eq!(
        h.query_all(by().role(Role::ComboBox).label("BC7 weighting"))
            .count(),
        1
    );
    for name in [
        "BC7 weighting",
        "Levels",
        "Resize",
        "Relative to",
        "Size floor",
        "DDS header",
    ] {
        let labels: Vec<Rect> = h
            .query_all(by().role(Role::Label).label(name))
            .map(|n| n.rect())
            .collect();
        assert_eq!(labels.len(), 1, "one {name} row: {labels:?}");
        assert!(c.contains(labels[0].center()), "{name} in the centre");
    }
}

/// Note JI3 (moved from the drawer's test): Relative to showing "Slot 8 …" stays inside the centre at 1100 and 1280.
#[test]
fn relative_to_slot_8_stays_inside_the_centre() {
    for w in [1100.0f32, 1280.0] {
        let d = tempfile::tempdir().unwrap();
        let mut m = model(d.path());
        m.set_field(KEY, "resize", serde_json::json!("half"));
        m.set_field(KEY, "resize_relative_to", serde_json::json!("slot8"));
        assert!(m.wait_idle(Duration::from_secs(60)));
        let h = sized_harness(m, w, 1600.0);
        let c = centre(&h);
        let combos: Vec<_> = h
            .query_all_by_role(Role::ComboBox)
            .filter(|n| c.contains(n.rect().center()))
            .collect();
        assert!(
            combos.iter().any(|n| n
                .accesskit_node()
                .value()
                .is_some_and(|v| v.starts_with("Slot 8"))),
            "{w}: the Relative to combo shows Slot 8"
        );
        let past: Vec<_> = combos
            .iter()
            .filter(|n| n.rect().max.x > c.max.x + 0.5)
            .map(|n| (n.accesskit_node().value(), n.rect().max.x))
            .collect();
        assert!(past.is_empty(), "{w}: past the centre {c:?}: {past:?}");
    }
}

/// The helper panel shares the columns: at 1100 px it stays inside the centre, and the centre inside the window (less
/// the harness' 8 px margin).
#[test]
fn helper_rows_stay_inside_at_1100() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.select_item("landscape/rock01_roughness.png");
    let h = harness(m, 1100.0);
    let c = centre(&h);
    let edge = h.ctx.content_rect().right() - 8.0;
    assert!(c.max.x <= edge + 0.5, "the centre {c:?} runs past {edge}");
    let role = control(&h, Role::ComboBox, "Helper role", c);
    assert!(role.max.x <= c.max.x, "Helper role {role:?} past {c:?}");
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
    // The slider keeps its own name, which says more than the row's "Threshold" label (E28).
    let slider = widget(&h, Role::Slider, "Alpha threshold");
    assert!(
        !slider.accesskit_node().is_disabled(),
        "the threshold is off"
    );
}

/// egui 0.37 (`response.rs:799-815`): a hover added after a control is linked to its row label does not name it.
/// The Normal handling combo of a complex-material set's normal carries the CL40 hover; it stays named by its row.
#[test]
fn a_linked_control_keeps_its_label_under_a_hover() {
    const NORMAL: &str = "landscape/rock01_normal.png";
    const HOVER: &str =
        "Renormalise and bake also bakes this set's complex _m gloss (anti-shimmer)";
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.decide(KEY, Answer::Subtype(Subtype::Diffuse));
    m.decide(NORMAL, Answer::Subtype(Subtype::Normal));
    assert!(m.wait_idle(Duration::from_secs(60)));
    complex_material(&mut m, "landscape/rock01");
    m.select_item(NORMAL);
    let mut h = harness(m, 1600.0);
    let combo = widget(&h, Role::ComboBox, "Normal handling").rect();
    // The CL40 branch is the one drawn: its hover shows on the combo.
    h.hover_at(combo.center());
    h.run();
    assert!(
        h.query_all_by_label_contains("anti-shimmer")
            .next()
            .is_some()
            || h.query_all_by_value(HOVER).next().is_some(),
        "the CL40 hover shows"
    );
    assert_eq!(
        h.query_all_by_role_and_label(Role::ComboBox, "Normal handling")
            .count(),
        1
    );
    // Any combo whose name holds the hover's key word, so a reworded hover cannot slip past.
    let named_by_hover: Vec<String> = h
        .query_all_by_role(Role::ComboBox)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.contains("anti-shimmer"))
        .collect();
    assert!(
        named_by_hover.is_empty(),
        "no combo is named by the hover: {named_by_hover:?}"
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
        .expect("the type question's combo");
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
// Dev tooling, not a test: it asserts nothing and runs only with `SDC_PANEL_SHOTS` set.
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
    use sdc_gui::model::set_preview::{SetPreview, DEFAULT_CAP};
    let mut m = pbr_opened_plain(dir);
    let key = json_entries(&m)
        .into_iter()
        .find(|(v, _)| v.id == "landscape/rock01")
        .unwrap()
        .0
        .key;
    m.select_set(&key);
    m.settings.set_view_3d = true;
    let active = m.resolved_selection();
    let set = &active.view().expect("keyed fixture").key;
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
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, sdc_gui::ui::State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, state),
        );
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
    let strip = support::panel(&h.ctx, "preview-settings");
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

/// The scroll-bar margin `hint_and_reset` keeps (Note JI3) also applies in the strip, whose last cell is narrow: an
/// edited row's ↺ stays clear of the row's combo.
// shortcut: at 1600 px the strip's ↺ ends ~4 px past the strip's edge (it ended ~14 px past before the margin); the
// strip's label formula leaves the last cell 18 px for a 19 px ↺. Pin `r.max.x <= strip.max.x` once that is fixed.
#[test]
fn a_strip_reset_stays_beside_its_combo() {
    let d = tempfile::tempdir().unwrap();
    let mut m = model(d.path());
    m.set_field(KEY, "encoder.effort", serde_json::json!("veryfast"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.expand(KEY);
    assert!(m.wait_idle(Duration::from_secs(60)));
    let h = harness(m, 1600.0);
    let strip = support::panel(&h.ctx, "preview-settings");
    let resets: Vec<Rect> = h
        .query_all_by_label("↺")
        .map(|n| n.rect())
        .filter(|r| strip.contains(r.center()))
        .collect();
    assert_eq!(resets.len(), 1, "the edited Effort row's ↺: {resets:?}");
    let r = resets[0];
    let combo = h
        .query_all_by_role(Role::ComboBox)
        .map(|n| n.rect())
        .find(|c| strip.contains(c.center()) && c.min.y <= r.center().y && r.center().y <= c.max.y)
        .expect("the Effort combo on the ↺'s row");
    assert!(
        combo.max.x <= r.min.x,
        "↺ {r:?} overlaps its combo {combo:?}"
    );
}

/// A pin: the strip never showed the colour rows, and now they are in Compression, which it shows.
#[test]
fn compact_strip_has_no_colour_rows() {
    let d = tempfile::tempdir().unwrap();
    let (h, strip) = strip_harness(d.path());
    assert!(h
        .query_all_by_label("Compression")
        .any(|n| strip.contains(n.rect().center())));
    // Note JI3: nor the rows the drawer handed to the centre (Levels, the Size group, DDS header).
    for name in [
        "Input content",
        "DDS tag",
        "DDS header",
        "Levels",
        "Resize",
        "Size floor",
    ] {
        // egui 0.37 names every panel's resize handle "Resize panel" (a `Splitter`); those are not the Resize row.
        let rows: Vec<_> = h
            .query_all(
                by().label_contains(name)
                    .predicate(|n| n.role() != Role::Splitter),
            )
            .collect();
        assert!(rows.is_empty(), "{name} in the strip");
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
        .expect("a type with two sRGB formats");
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

/// Note HI9: hover `name` (exactly one such node in the centre) and say whether a tooltip holding `text` shows.
fn hover_shows(h: &mut Harness<'static, AppModel>, role: Role, name: &str, text: &str) -> bool {
    let c = centre(h);
    let at = control(h, role, name, c);
    h.query_all(by().role(role).label(name))
        .find(|n| n.rect() == at)
        .unwrap()
        .hover();
    h.run();
    h.query_all_by_label_contains(text).next().is_some()
}

#[test]
fn a_row_has_a_question_mark() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(model(d.path()), 1280.0);
    let text = sdc_gui::model::fields::field_help("color.content").unwrap();
    assert!(h.query_all_by_label_contains(text).next().is_none());
    assert!(hover_shows(
        &mut h,
        Role::Label,
        "Help: Input content",
        text
    ));
}

/// A row off for a pending plan (`fix_off`, Note FK) still shows its help.
#[test]
fn help_shows_on_a_disabled_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = model(d.path());
    // Another texture's answer starts a plan; `ui::draw` alone never lands it.
    m.decide(LEAF, Answer::Subtype(Subtype::Diffuse));
    assert!(m.fix_off().is_some());
    let mut h = harness(m, 1280.0);
    let c = centre(&h);
    assert!(h
        .query_all(by().role(Role::ComboBox).label("DDS tag"))
        .find(|n| c.contains(n.rect().center()))
        .unwrap()
        .accesskit_node()
        .is_disabled());
    let text = sdc_gui::model::fields::field_help("color.tag").unwrap();
    assert!(hover_shows(&mut h, Role::Label, "Help: DDS tag", text));
}

#[test]
fn question_marks_line_up() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(model(d.path()), 1280.0);
    let c = centre(&h);
    let marks: Vec<(String, Rect)> = h
        .query_all(by().role(Role::Label))
        .filter_map(|n| {
            let v = n.accesskit_node().value().or(n.accesskit_node().label())?;
            v.starts_with("Help: ").then(|| (v.to_string(), n.rect()))
        })
        .filter(|(_, r)| c.contains(r.center()))
        .collect();
    assert!(marks.len() >= 15, "{marks:?}");
    let x = marks[0].1.max.x;
    for (n, r) in &marks {
        assert!(
            (r.max.x - x).abs() <= 1.0,
            "{n} ends at {}, {} at {x}",
            r.max.x,
            marks[0].0
        );
    }
    // At the end of the label column: left of every control.
    let sub = control(&h, Role::ComboBox, "Texture type", c);
    assert!(
        x <= sub.min.x,
        "the marks end at {x}, past Type at {}",
        sub.min.x
    );
}

#[test]
fn hovering_the_label_shows_the_same_help() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(model(d.path()), 1280.0);
    let text = sdc_gui::model::fields::field_help("color.tag").unwrap();
    assert!(hover_shows(&mut h, Role::Label, "DDS tag", text));
}

/// Note HI9: the Helper tick's help says what a helper is, with no normal-map detail layers (RNM is gone on 3.0).
#[test]
fn the_helper_tick_has_help() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(model(d.path()), 1280.0);
    let text = sdc_gui::model::fields::HELPER_HELP;
    assert!(!text.to_lowercase().contains("normal map"), "{text}");
    assert!(hover_shows(&mut h, Role::Label, "Help: Helper", text));
}

/// Screens of the help (Note HI9) at 1100, 1280 and 1600 px: a `?`'s tooltip, and a disabled row's (a pending plan),
/// written whole to `SDC_PANEL_SHOTS`. GPU (`render_`): skipped unless that variable names a folder.
#[test]
fn render_help_shots() {
    let Some(out) = std::env::var_os("SDC_PANEL_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    for w in [1100.0, 1280.0, 1600.0] {
        for (name, off) in [("help", false), ("help-disabled", true)] {
            let d = tempfile::tempdir().unwrap();
            let mut m = model(d.path());
            if off {
                m.decide(LEAF, Answer::Subtype(Subtype::Diffuse));
            }
            let mut h = harness(m, w);
            let (role, mark) = (
                Role::Label,
                if off {
                    "Help: DDS tag"
                } else {
                    "Help: Input content"
                },
            );
            let text = if off { "_SRGB flag" } else { "colour values" };
            assert!(hover_shows(&mut h, role, mark, text));
            h.render()
                .expect("render")
                .save(out.join(format!("{name}-{w}.png")))
                .unwrap();
        }
    }
}

/// Note HI18: the strip keeps its label room (no `?`); the label still shows the row's help.
#[test]
fn the_strip_has_help_on_its_labels() {
    let d = tempfile::tempdir().unwrap();
    let (mut h, strip) = strip_harness(d.path());
    let marks = h
        .query_all(by().role(Role::Label))
        .filter(|n| strip.contains(n.rect().center()))
        .filter(|n| {
            let a = n.accesskit_node();
            a.value()
                .or(a.label())
                .is_some_and(|v| v.starts_with("Help: "))
        })
        .count();
    assert_eq!(marks, 0, "a ? in the strip");
    let text = sdc_gui::model::fields::field_help("format").unwrap();
    let at = control(&h, Role::Label, "Format", strip);
    h.query_all(by().role(Role::Label).label("Format"))
        .find(|n| n.rect() == at)
        .unwrap()
        .hover();
    h.run();
    assert!(h.query_all_by_label_contains(text).next().is_some());
}

/// Note HI13: at 1100 px the Undecided box's dropdown still starts at the Alpha row's x, and Confirm is never cut short
/// (the dropdown gives up the room).
#[test]
fn alpha_question_at_1100_keeps_its_x_and_an_uncut_confirm() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(leaf(d.path()), 1100.0);
    let c = centre(&h);
    let asked = control(&h, Role::ComboBox, "Alpha type: Cutout", c);
    let row = control(&h, Role::ComboBox, "Alpha", c);
    assert!(
        (asked.min.x - row.min.x).abs() <= 1.0,
        "the question's dropdown {asked:?}, the Alpha row's {row:?}"
    );
    let confirm = h.get_by_label("Confirm alpha").rect();
    let font = egui::TextStyle::Button.resolve(&h.ctx.global_style());
    let text = h.ctx.fonts_mut(|f| {
        f.layout_no_wrap("Confirm".into(), font, egui::Color32::WHITE)
            .size()
            .x
    });
    assert!(
        confirm.width() >= text,
        "Confirm {confirm:?} is narrower than its text ({text})"
    );
    assert!(
        confirm.max.x <= c.max.x,
        "Confirm {confirm:?} past the centre {c:?}"
    );
}

/// Note HI13: the sub-type question's combo is as wide as the Alpha row's.
#[test]
fn the_sub_type_question_has_the_rows_width() {
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
        .expect("the type question's combo");
    let row = control(&h, Role::ComboBox, "Alpha", c);
    assert!(
        (combo.width() - row.width()).abs() <= 1.0,
        "the question's combo {combo:?}, the Alpha row's {row:?}"
    );
}

/// Note HI15: a texture in no set keeps its Helper tick under Sub-type.
#[test]
fn a_no_set_image_has_the_helper_tick() {
    let d = tempfile::tempdir().unwrap();
    let mut m = leaf(d.path());
    m.remove_member("landscape/leaf", LEAF).expect("removed");
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.is_no_set(LEAF));
    m.select_item(LEAF);
    let mut h = harness(m, 1280.0);
    let c = centre(&h);
    let sub = control(&h, Role::ComboBox, "Texture type", c);
    let tick = control(&h, Role::CheckBox, "Helper", c);
    assert!(
        (tick.min.x - sub.min.x).abs() <= 1.0,
        "tick {tick:?}, Type {sub:?}"
    );
    h.get_by_role_and_label(Role::CheckBox, "Helper").click();
    h.run();
    control(&h, Role::ComboBox, "Helper role", c);
}

/// Note HI1, HI4: at 1100 px the Helper tick stays in the control column, inside the centre.
#[test]
fn the_helper_tick_at_1100_stays_in_the_control_column() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(model(d.path()), 1100.0);
    let c = centre(&h);
    let edge = h.ctx.content_rect().right() - 8.0;
    assert!(c.max.x <= edge + 0.5, "the centre {c:?} runs past {edge}");
    let sub = control(&h, Role::ComboBox, "Texture type", c);
    let tick = control(&h, Role::CheckBox, "Helper", c);
    assert!(
        (tick.min.x - sub.min.x).abs() <= 1.0,
        "tick {tick:?}, Type {sub:?}"
    );
    assert!(tick.min.y >= sub.max.y, "tick {tick:?} below Type {sub:?}");
    assert!(tick.max.x <= c.max.x, "tick {tick:?} past the centre {c:?}");
}

/// Note JI3 (spec 3.6): the provenance note keeps NOTE_MIN; the value column shrinks first. The leaf (a BC7 diffuse)
/// shows every COMBOS entry, so each is expected: a combo that vanished fails here rather than being skipped.
#[test]
fn the_note_keeps_its_width_and_the_control_shrinks_first() {
    // The control column's offset from the centre's left edge at full room: where the label column ends.
    let mut full_x = None;
    for w in [1600.0, 1100.0, 1280.0] {
        let d = tempfile::tempdir().unwrap();
        let h = sized_harness(leaf(d.path()), w, 1600.0);
        let c = centre(&h);
        for name in COMBOS {
            let r = control(&h, Role::ComboBox, name, c);
            // The control gives way first: the label column keeps its full width, so the control starts at the
            // same offset at every width (a label shrunk along with the control would move it left).
            let off = r.min.x - c.min.x;
            let full = *full_x.get_or_insert(off);
            assert!(
                (off - full).abs() < 0.5,
                "{w}: {name} starts {off} from the centre's left, {full} with room"
            );
            assert!(
                c.max.x - r.max.x >= 120.0 - 0.5,
                "{w}: {name} ends at {} in a centre ending at {}",
                r.max.x,
                c.max.x
            );
            assert!(r.width() >= 96.0 - 0.5, "{w}: {name} is {} wide", r.width());
            // With room, the control keeps CONTROL_W: it is the first to shrink, not the only one drawn small.
            if w >= 1600.0 {
                assert!(
                    (r.width() - CONTROL_W).abs() < 0.5,
                    "{w}: {name} is {} wide",
                    r.width()
                );
            }
        }
    }
}
