//! The texture panel's group list and their open state (texture-panel-compact plan, Task 1).
mod support;

use eframe::egui::{self, accesskit::Role, Key};
use egui_kittest::kittest::{NodeT as _, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::panel_groups::{self, PanelGroup};
use sdc_gui::ui::group::{collapsing_group, header_name, GroupHead};

#[test]
fn every_panel_field_has_one_place() {
    for (key, _) in sdc_gui::model::bulk::panel_fields() {
        let first = panel_groups::FIRST_SCREEN.contains(&key);
        assert!(first != panel_groups::panel_group(key).is_some(), "{key}");
    }
    assert_eq!(
        panel_groups::panel_group("channels.msn_source"),
        Some(PanelGroup::NormalMap)
    );
    assert_eq!(
        panel_groups::panel_group("mips.normal"),
        Some(PanelGroup::NormalMap)
    );
    assert_eq!(
        panel_groups::panel_group("mips.normal_base"),
        Some(PanelGroup::NormalMap)
    );
    assert_eq!(
        panel_groups::panel_group("mips.filter"),
        Some(PanelGroup::Mipmaps)
    );
    assert_eq!(
        panel_groups::panel_group("mips.coverage"),
        Some(PanelGroup::Alpha)
    );
    assert_eq!(
        panel_groups::panel_group("header"),
        Some(PanelGroup::Compression)
    );
    assert_eq!(panel_groups::panel_group("format"), None);
    for (key, _) in sdc_gui::model::fields::CUBE_FIELDS {
        assert_eq!(
            panel_groups::panel_group(key),
            Some(PanelGroup::Cubemap),
            "{key}"
        );
    }
}

/// The plan's field-to-group table, one line per field (plan "Where each row goes"). A field that lands in another
/// group, or one that no line lists, fails here.
#[test]
fn the_whole_field_to_group_table_is_pinned() {
    use PanelGroup::*;
    let table: [(&str, Option<PanelGroup>); 31] = [
        ("subtype", None),
        ("format", None),
        ("channels.green_convention", None),
        ("alpha.policy", Some(Alpha)),
        ("alpha.threshold", Some(Alpha)),
        ("alpha.compare", Some(Alpha)),
        ("mips.coverage", Some(Alpha)),
        ("encoder.bc1_alpha", Some(Alpha)),
        ("alpha.exact_opaque", Some(Alpha)),
        ("alpha.parallax", Some(Alpha)),
        ("alpha.exr_straight", Some(Alpha)),
        ("mips.levels", Some(Mipmaps)),
        ("mips.filter", Some(Mipmaps)),
        ("mips.edge", Some(Mipmaps)),
        ("mips.normal", Some(NormalMap)),
        ("mips.normal_base", Some(NormalMap)),
        ("channels.msn_source", Some(NormalMap)),
        ("resize", Some(Size)),
        ("resize_relative_to", Some(Size)),
        ("resize_floor", Some(Size)),
        ("color.tag", Some(Compression)),
        ("color.content", Some(Compression)),
        ("encoder.effort", Some(Compression)),
        ("encoder.bc7_weights", Some(Compression)),
        ("encoder.perceptual", Some(Compression)),
        ("header", Some(Compression)),
        ("encoder.bc3_search", Some(Compression)),
        ("encoder.bc45_search_radius", Some(Compression)),
        ("encoder.bc1_search", Some(Compression)),
        // Channels holds read-only lines, never a field; an unknown key has no group.
        ("channels.red", None),
        ("no.such_field", None),
    ];
    for (key, want) in table {
        assert_eq!(panel_groups::panel_group(key), want, "{key}");
    }
    let listed: Vec<&str> = table.iter().map(|(k, _)| *k).collect();
    for (key, _) in sdc_gui::model::bulk::panel_fields()
        .into_iter()
        .chain(sdc_gui::model::fields::FIELDS)
        .chain(sdc_gui::model::fields::all_fields())
    {
        assert!(listed.contains(&key), "{key} has no line in the table");
    }
}

#[test]
fn a_stored_state_overrides_the_default_and_an_old_file_takes_defaults() {
    let mut s = sdc_gui::settings::GuiSettings::default();
    panel_groups::set_open(&mut s, PanelGroup::Sources, true);
    assert!(panel_groups::is_open(&s, PanelGroup::Sources));
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("gui.toml");
    std::fs::write(&p, "live_encode = true\n").unwrap();
    let (old, err) = sdc_gui::settings::GuiSettings::load(&p);
    assert_eq!(err, None);
    for g in PanelGroup::ALL {
        assert_eq!(
            panel_groups::is_open(&old, g),
            panel_groups::default_open(g)
        );
    }
}

#[test]
fn group_ids_are_unique_and_titles_follow_the_table() {
    let ids: std::collections::BTreeSet<_> = PanelGroup::ALL.iter().map(|g| g.id()).collect();
    assert_eq!(ids.len(), 8);
    assert_eq!(PanelGroup::NormalMap.title(), "Normal map");
    assert_eq!(
        PanelGroup::Sources.title(),
        "Where these settings come from"
    );
}

#[test]
fn where_settings_come_from_starts_closed_from_the_start() {
    let s = sdc_gui::settings::GuiSettings::default();
    assert!(!panel_groups::is_open(&s, PanelGroup::Sources));
}

// Task 2: the header widget `collapsing_group`.

struct Head {
    title: &'static str,
    summary: String,
    changed: bool,
}

fn install_fonts(ctx: &egui::Context) {
    sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
}

/// One header over a one-line body; the state is the open flag, flipped on `toggled`.
fn head_harness(w: f32, head: Head) -> Harness<'static, bool> {
    Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(w, 200.0))
        .build_ui_state(
            move |ui, open: &mut bool| {
                let g = collapsing_group(
                    ui,
                    "t",
                    &GroupHead {
                        title: head.title,
                        summary: &head.summary,
                        summary_hover: None,
                        changed: head.changed,
                        problem: None,
                    },
                    *open,
                    false,
                    |ui| ui.label("row inside"),
                );
                if g.toggled {
                    *open = !*open;
                }
            },
            false,
        )
}

fn mipmaps() -> Head {
    Head {
        title: "Mipmaps",
        summary: "Kaiser, full chain".into(),
        changed: true,
    }
}

fn mipmaps_head<'a>(summary: &'a str, hover: Option<&'a str>) -> GroupHead<'a> {
    GroupHead {
        title: "Mipmaps",
        summary,
        summary_hover: hover,
        changed: true,
        problem: None,
    }
}

#[test]
fn the_header_is_a_disclosure_triangle_with_a_full_name() {
    let h = head_harness(400.0, mipmaps());
    let n = h.get_by_label("Mipmaps, collapsed, changed: Kaiser, full chain");
    assert_eq!(n.accesskit_node().role(), Role::DisclosureTriangle);
    assert_eq!(n.accesskit_node().data().is_expanded(), Some(false));
}

#[test]
fn an_open_header_shows_no_summary() {
    // TPC-R1 (JR6): the rows show the values while the group is open, so the header does not repeat them.
    let mut h = head_harness(400.0, mipmaps());
    h.get_by_label_contains("Mipmaps, ").click();
    h.run();
    let n = h.get_by_label("Mipmaps, expanded, changed");
    assert_eq!(n.accesskit_node().data().is_expanded(), Some(true));
    assert!(
        h.query_by_label("Kaiser, full chain").is_none(),
        "no summary label is drawn while open"
    );
    assert_eq!(
        header_name(&mipmaps_head("Kaiser, full chain", None), true),
        "Mipmaps, expanded, changed"
    );
}

#[test]
fn the_header_hit_target_is_24_px_and_the_full_row() {
    let h = head_harness(400.0, mipmaps());
    let r = h.get_by_label_contains("Mipmaps, ").rect();
    assert!(r.height() >= 24.0, "{r:?}");
    // The harness keeps an 8 px margin on each side of the 400 px window: the row is the 384 px between them.
    assert!((r.width() - 384.0).abs() < 1.0, "{r:?}");
}

/// A click at `pos`: move, press, release, then a frame.
fn click_at(h: &mut Harness<'static, bool>, pos: egui::Pos2) {
    h.hover_at(pos);
    h.run();
    h.drag_at(pos);
    h.drop_at(pos);
    h.run();
}

#[test]
fn clicking_the_title_the_summary_or_the_marks_toggles() {
    let mut h = head_harness(400.0, mipmaps());
    let r = h.get_by_label_contains("Mipmaps, ").rect();
    click_at(&mut h, egui::pos2(r.left() + 40.0, r.center().y));
    assert!(*h.state(), "the title");
    let title = h.get_by_label("Mipmaps").rect();
    click_at(&mut h, title.center());
    assert!(!*h.state(), "the title label itself");
    let summary = h.get_by_label("Kaiser, full chain").rect();
    click_at(&mut h, summary.center());
    assert!(*h.state(), "the summary");
    let changed = h.get_by_label("● changed").rect().center();
    click_at(&mut h, changed);
    assert!(!*h.state(), "the changed mark");
}

#[test]
fn tab_reaches_the_header_and_space_and_enter_toggle_it() {
    let mut h = head_harness(400.0, mipmaps());
    h.key_press(Key::Tab);
    h.run();
    assert!(h.get_by_label_contains("Mipmaps, ").is_focused());
    h.key_press(Key::Space);
    h.run();
    assert!(*h.state());
    h.key_press(Key::Enter);
    h.run();
    assert!(!*h.state());
}

/// Every stroked rectangle of the last frame's shapes.
fn rect_strokes(h: &Harness<'static, bool>) -> Vec<(egui::Rect, egui::Stroke)> {
    fn walk(s: &egui::Shape, out: &mut Vec<(egui::Rect, egui::Stroke)>) {
        match s {
            egui::Shape::Rect(r) => out.push((r.rect, r.stroke)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut out);
    }
    out
}

#[test]
fn the_focused_header_draws_a_ring() {
    // The ring is paint only (no AccessKit property), so this reads the frame's shapes.
    let mut h = head_harness(400.0, mipmaps());
    let stroke = h.ctx.global_style().visuals.selection.stroke;
    let r = h.get_by_label_contains("Mipmaps, ").rect();
    let ringed = |h: &Harness<'static, bool>| {
        rect_strokes(h)
            .iter()
            .any(|(rect, s)| *s == stroke && rect.expand(2.0).contains_rect(r))
    };
    assert!(!ringed(&h), "premise: no ring before the header has focus");
    h.key_press(Key::Tab);
    h.run();
    assert!(ringed(&h));
}

#[test]
fn a_long_summary_truncates_and_keeps_its_full_text() {
    let long = "皮膚の粗さ".repeat(40);
    let mut h = head_harness(
        300.0,
        Head {
            title: "Size",
            summary: long.clone(),
            changed: true,
        },
    );
    let name = format!("Size, collapsed, changed: {long}");
    let header = h
        .query_all_by_label(&name)
        .find(|n| n.accesskit_node().role() == Role::DisclosureTriangle)
        .unwrap()
        .rect();
    let changed = h.get_by_label("● changed").rect();
    assert!(
        changed.max.x <= header.max.x + 0.5,
        "changed stays inside the row"
    );
    // The header's name and the summary label both carry the text: hover the summary by its rect, not by a lookup.
    let summary = h
        .query_all_by_label_contains("皮膚の粗さ皮膚")
        .filter(|n| n.accesskit_node().role() != Role::DisclosureTriangle)
        .map(|n| n.rect())
        .next()
        .expect("the summary label");
    assert!(
        summary.max.x <= changed.min.x + 0.5,
        "the summary is cut before changed"
    );
    // The truncated label already names its node with the full text, so count: the tooltip adds one more node.
    let full = |h: &Harness<'static, bool>| {
        h.query_all_by_label(&long)
            .filter(|n| n.accesskit_node().role() != Role::DisclosureTriangle)
            .count()
    };
    let before = full(&h);
    h.hover_at(summary.center());
    h.run();
    assert_eq!(
        full(&h),
        before + 1,
        "the hover shows the full summary in a tooltip"
    );
}

/// The centre column's width in an app window `w` wide with a project open. Not `app_harness::harness`: that module
/// is for the test binaries kept apart (it redirects `%APPDATA%` through the environment, which an `it` module may not
/// do, and needs a GPU adapter). This is `gui_tweaks`'s harness (`draw_with`) on `support::opened`, whose model is built
/// with `AppModel::new(.., None, ..)`: no settings path, so no settings are written anywhere.
fn centre_width_at(w: f32) -> f32 {
    let d = tempfile::tempdir().unwrap();
    let m = support::opened(d.path());
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(w, 800.0))
        .build_ui_state(
            |ui, (m, s): &mut (sdc_gui::model::AppModel, sdc_gui::ui::State)| {
                sdc_gui::ui::draw_with(ui, m, s)
            },
            (m, sdc_gui::ui::State::default()),
        );
    h.run();
    sdc_gui::ui::centre_rect(&h.ctx)
        .expect("the centre is drawn")
        .width()
}

#[test]
fn changed_is_never_cut_at_1100_px() {
    let w = centre_width_at(1100.0);
    let h = head_harness(
        w,
        Head {
            title: "Size",
            summary: "Half of Slot 8 · Back-light / specular, floor 512 px".into(),
            changed: true,
        },
    );
    assert!(
        h.get_by_label("● changed").rect().max.x
            <= h.get_by_label_contains("Size, ").rect().max.x + 0.5
    );
}

#[test]
fn titles_and_changed_fit_the_preview_strip_and_the_3d_window() {
    use sdc_gui::ui::{preview::SETTINGS_WIDTH, set_preview::SETTINGS_WINDOW_W};
    for w in [SETTINGS_WIDTH, SETTINGS_WINDOW_W] {
        let h = head_harness(
            w,
            Head {
                title: "Where these settings come from",
                summary: "How each value was chosen".into(),
                changed: true,
            },
        );
        let row = h
            .get_by_label_contains("Where these settings come from, ")
            .rect();
        let changed = h.get_by_label("● changed").rect();
        assert!(changed.max.x <= row.max.x + 0.5, "{w}");
        let title = h.get_by_label("Where these settings come from").rect();
        assert!(
            title.max.x <= changed.min.x + 0.5,
            "{w}: the title is clear of changed"
        );
    }
}

// Motion tests (R2-2): egui lays the body out at full size and clips it, so the body's own rect does not move. A
// "below" label drawn after the group does: it sits under the header while closed and under the body once open.
// kittest's default step (0.25 s) is longer than egui's animation time, so each harness steps by 1/120 s. kittest also
// sets `animation_time` to 0, which makes `motion::reduced` true in every harness (Note JO), so `motion_harness` sets it
// back; the snap tests then snap because of the code under test. A real collapse takes about 10 steps, past kittest's
// default `max_steps` of 4, so the harness allows 40.
fn motion_harness(instant: bool) -> Harness<'static, bool> {
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(400.0, 300.0))
        .with_step_dt(1.0 / 120.0)
        .with_max_steps(40)
        .build_ui_state(
            move |ui, open: &mut bool| {
                let g = collapsing_group(
                    ui,
                    "t",
                    &mipmaps_head("Kaiser, full chain", None),
                    *open,
                    instant,
                    |ui| ui.label("row inside"),
                );
                if g.toggled {
                    *open = !*open;
                }
                ui.label("below");
            },
            false,
        );
    h.ctx.all_styles_mut(|s| s.animation_time = 1.0 / 12.0);
    h.run();
    h
}

fn below_top(h: &Harness<'static, bool>) -> f32 {
    h.get_by_label("below").rect().top()
}

#[test]
fn a_keyboard_toggle_does_not_animate() {
    let mut h = motion_harness(false);
    sdc_gui::ui::motion::force_reduced(&h.ctx, false);
    assert!(
        !sdc_gui::ui::motion::reduced(&h.ctx),
        "premise: motion is on in this harness"
    );
    h.key_press(Key::Tab);
    h.run();
    let closed = below_top(&h);
    h.key_press(Key::Space);
    h.step();
    let one = below_top(&h);
    h.run();
    assert!(one > closed + 10.0, "the body is open at the first step");
    assert_eq!(one, below_top(&h), "and nothing slides after it");
}

#[test]
fn an_instant_open_does_not_animate() {
    let mut h = motion_harness(true); // instant = true, as a forced open draws
    let closed = below_top(&h);
    *h.state_mut() = true;
    h.step();
    let one = below_top(&h);
    h.run();
    assert!(one > closed + 10.0);
    assert_eq!(one, below_top(&h));
}

#[test]
fn under_reduced_motion_a_click_opens_in_one_frame() {
    let mut h = motion_harness(false);
    sdc_gui::ui::motion::force_reduced(&h.ctx, true); // Plan 3a Task 8's test-hooks seam, kept in ctx
    h.run();
    let closed = below_top(&h);
    h.get_by_role(Role::DisclosureTriangle).click();
    h.step();
    let one = below_top(&h);
    h.run();
    assert!(one > closed + 10.0);
    assert_eq!(one, below_top(&h));
}

#[test]
fn without_reduced_motion_a_click_animates() {
    let mut h = motion_harness(false); // animation_time is already back at 1/12 s (see above)
    sdc_gui::ui::motion::force_reduced(&h.ctx, false);
    h.run();
    assert!(
        !sdc_gui::ui::motion::reduced(&h.ctx),
        "premise: motion is on in this harness"
    );
    let closed = below_top(&h);
    h.get_by_role(Role::DisclosureTriangle).click();
    h.step();
    let one = below_top(&h);
    h.run();
    let end = below_top(&h);
    assert!(
        one > closed + 1.0,
        "the body started opening in the toggling frame (not a frame late, not frozen)"
    );
    assert!(
        one + 1.0 < end,
        "egui's built-in collapse runs: the label below is still moving after one step"
    );
}

#[test]
fn an_empty_summary_drops_the_colon() {
    let head = GroupHead {
        title: "Used by 2 meshes",
        summary: "",
        summary_hover: None,
        changed: false,
        problem: None,
    };
    assert_eq!(header_name(&head, false), "Used by 2 meshes, collapsed");
}

#[test]
fn a_problem_is_in_the_name_and_draws_its_dot_open_or_closed() {
    let head = GroupHead {
        title: "Mipmaps",
        summary: "none",
        summary_hover: None,
        changed: true,
        problem: Some(sdc_gui::ui::status::Status::Warning),
    };
    assert_eq!(
        header_name(&head, false),
        "Mipmaps, collapsed, changed, has a problem: none"
    );
    assert_eq!(
        header_name(&head, true),
        "Mipmaps, expanded, changed, has a problem"
    );
}

#[test]
fn the_header_does_not_move_when_its_group_opens() {
    let mut h = head_harness(400.0, mipmaps());
    let before = h.get_by_label_contains("Mipmaps, ").rect();
    h.get_by_label_contains("Mipmaps, ").click();
    h.run();
    assert_eq!(before, h.get_by_label_contains("Mipmaps, ").rect());
}

#[test]
fn a_summary_hover_replaces_the_summary_text_on_hover() {
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(400.0, 200.0))
        .build_ui_state(
            |ui, open: &mut bool| {
                let g = collapsing_group(
                    ui,
                    "t",
                    &mipmaps_head(
                        "not used for this texture",
                        Some("Every row here: normal maps only."),
                    ),
                    *open,
                    false,
                    |ui| ui.label("row inside"),
                );
                if g.toggled {
                    *open = !*open;
                }
            },
            false,
        );
    h.run();
    let summary = h.get_by_label("not used for this texture").rect();
    assert!(h
        .query_by_label("Every row here: normal maps only.")
        .is_none());
    h.hover_at(summary.center());
    h.run();
    assert!(
        h.query_by_label("Every row here: normal maps only.")
            .is_some(),
        "the closed summary's hover is the summary_hover text"
    );
}

/// TPC-T2-1: with `summary_hover` set, an elided summary keeps its full text on hover, then the hover text; a summary
/// that fits shows the hover text alone.
#[test]
fn an_elided_summary_with_a_hover_shows_both_texts() {
    const HOVER: &str = "Each row below says why it is not used.";
    let hover_of = |w: f32, summary: String| {
        let mut h = Harness::builder()
            .with_context_setup(install_fonts)
            .with_size(egui::vec2(w, 200.0))
            .build_ui_state(
                move |ui, open: &mut bool| {
                    let g = collapsing_group(
                        ui,
                        "t",
                        &mipmaps_head(&summary, Some(HOVER)),
                        *open,
                        false,
                        |ui| ui.label("row inside"),
                    );
                    if g.toggled {
                        *open = !*open;
                    }
                },
                false,
            );
        h.run();
        let at = h
            .query_all_by_label_contains("Kaiser")
            .filter(|n| n.accesskit_node().role() != Role::DisclosureTriangle)
            .map(|n| n.rect().center())
            .next()
            .expect("the summary label");
        h.hover_at(at);
        h.run();
        // The tooltip's text node: every non-header node carrying the hover text, with what else it says.
        h.query_all_by_label_contains(HOVER)
            .filter(|n| n.accesskit_node().role() != Role::DisclosureTriangle)
            .filter_map(|n| {
                let d = n.accesskit_node();
                let d = d.data();
                d.value().or(d.label()).map(str::to_owned)
            })
            .collect::<Vec<_>>()
    };
    let long = format!("Kaiser {}", "full chain, ".repeat(30));
    let shown = hover_of(300.0, long.clone());
    assert!(
        shown.iter().any(|t| t == &format!("{long}\n{HOVER}")),
        "an elided summary: the full summary, a line break, then the hover text: {shown:?}"
    );
    let shown = hover_of(300.0, "Kaiser".to_owned());
    assert!(
        shown.iter().any(|t| t == HOVER),
        "a summary that fits: the hover text alone: {shown:?}"
    );
}

#[test]
fn a_header_drawn_before_disabled_rows_toggles() {
    // What Task 4's group_section does under FK3: the header first, then the body calls ui.disable() before its rows.
    // (A header drawn inside a disabled Ui cannot be enabled: egui 0.37.0 ui.rs:966 gives a widget the Ui's state.)
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(400.0, 200.0))
        .build_ui_state(
            |ui, open: &mut bool| {
                let g = collapsing_group(
                    ui,
                    "t",
                    &mipmaps_head("Kaiser, full chain", None),
                    *open,
                    false,
                    |ui| {
                        ui.disable();
                        let mut on = false;
                        ui.checkbox(&mut on, "row inside")
                    },
                );
                if g.toggled {
                    *open = !*open;
                }
            },
            true,
        );
    h.run();
    let row = h.get_by_label("row inside");
    assert!(row.accesskit_node().is_disabled(), "the row waits");
    assert!(
        !h.get_by_label_contains("Mipmaps, ")
            .accesskit_node()
            .is_disabled(),
        "the header does not"
    );
    h.get_by_label_contains("Mipmaps, ").click();
    h.run();
    assert!(!*h.state(), "the header still toggles");
}

// Task 3: group summaries and the changed mark.

use engine::config::{ColorTag, Format, Target};
use sdc_gui::model::fields::FieldView;
use sdc_gui::model::panel_groups::{panel_views, summary, GroupSummary, SummaryExtra};
use sdc_gui::model::{Answer, AppModel};
use serde_json::json;
use std::time::Duration;

const KEY: &str = "landscape/rock01.png";
const CUBE: &str = "a/chrome.png";
const E: SummaryExtra = SummaryExtra { channels: None };

/// The fixture's rock01 decided as a CS diffuse (BC7) and selected: `texture_panel_layout.rs::model`, copied.
fn rock(dir: &std::path::Path) -> AppModel {
    let mut m = support::opened(dir);
    m.set_target(Target::Cs);
    m.decide(KEY, Answer::Subtype(engine::config::Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(KEY);
    m
}

/// `cubemap_gui.rs::cube_model`, copied: a 6:1 strip decided as Cubemap and a flat 2D texture, with `selected` open.
fn cube_model(d: &std::path::Path, selected: &str) -> AppModel {
    let r = d.join("textures/a");
    std::fs::create_dir_all(&r).unwrap();
    image::RgbImage::from_fn(384, 64, |x, _| image::Rgb([(x % 256) as u8, 9, 9]))
        .save(r.join("chrome.png"))
        .unwrap();
    image::RgbImage::from_fn(64, 64, |x, y| image::Rgb([x as u8, y as u8, 90]))
        .save(r.join("flat.png"))
        .unwrap();
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(d.join("export")),
        ..sdc_gui::settings::GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, support::session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.join("textures"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    support::choose_export(&mut m, d.join("export"));
    m.decide(CUBE, Answer::Subtype(engine::config::Subtype::Cubemap));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(selected);
    m
}

fn job<'a>(m: &'a AppModel, k: &str) -> &'a engine::policy::ResolvedJob {
    m.item(k).and_then(|i| i.job.as_ref()).expect("a job")
}

fn views(m: &AppModel) -> Vec<FieldView> {
    panel_views(m.target, job(m, KEY))
}

fn set(m: &mut AppModel, writes: &[(&str, serde_json::Value)]) {
    for (field, value) in writes {
        m.set_field(KEY, field, value.clone());
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
}

fn sum(m: &AppModel, g: PanelGroup) -> GroupSummary {
    summary(g, &views(m), Target::Cs, &job(m, KEY).profile, &E)
}

#[test]
fn summaries_read_like_the_table() {
    use PanelGroup::*;
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    set(&mut m, &[("mips.filter", json!("kaiser"))]);
    assert_eq!(sum(&m, Mipmaps).text, "Kaiser, full chain");
    // rock01 is 64x64 (support/mod.rs:113-118): 7 levels, so 4 is offered.
    set(
        &mut m,
        &[("mips.filter", json!("box")), ("mips.levels", json!(4))],
    );
    assert_eq!(sum(&m, Mipmaps).text, "Box, 4 levels");
    set(&mut m, &[("mips.levels", json!("none"))]);
    assert_eq!(sum(&m, Mipmaps).text, "None (base level only)");
    set(
        &mut m,
        &[
            ("alpha.policy", json!("cutout")),
            ("alpha.threshold", json!(128)),
        ],
    );
    assert_eq!(sum(&m, Alpha).text, "Cutout at 128");
    set(
        &mut m,
        &[("resize", json!("half")), ("resize_floor", json!(512))],
    );
    assert_eq!(sum(&m, Size).text, "Half, floor 512 px");
    set(&mut m, &[("resize_relative_to", json!("slot8"))]);
    assert!(
        sum(&m, Size).text.starts_with("Half of Slot 8 · "),
        "Relative to reads its slot label: {}",
        sum(&m, Size).text
    );
    // The brief writes `color.tag = srgb` here, but under cs a plain Diffuse blocks _SRGB (TAG-SRGB-NONPBR), so
    // `format_write_ok` refuses the write: the diffuse keeps its UNORM tag, and the _SRGB case is the next test's.
    set(&mut m, &[("encoder.effort", json!("veryslow"))]);
    assert_eq!(
        sum(&m, Compression).text,
        "UNORM (as stored), Effort Very slow"
    );
    assert_eq!(sum(&m, Sources).text, "How each value was chosen");
}

#[test]
fn a_pbr_colour_reads_its_srgb_tag() {
    // PBR colour is _SRGB under cs (UNORM is blocked there), so the tag reads "_SRGB".
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    m.decide(KEY, Answer::Subtype(engine::config::Subtype::PbrAlbedo));
    assert!(m.wait_idle(Duration::from_secs(60)));
    set(&mut m, &[("encoder.effort", json!("veryslow"))]);
    assert_eq!(
        sum(&m, PanelGroup::Compression).text,
        "_SRGB, Effort Very slow"
    );
}

#[test]
fn a_locked_part_is_left_out() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    // The brief's `set_field("format", "bc6h_uf16")` pairs BC6H with the diffuse's UNORM tag, which `format_write_ok`
    // refuses (Note JG2), so the pair goes in through `set_format`: color.tag locked, "BC6H has no _SRGB form".
    m.set_format(KEY, Format::Bc6hUf16, ColorTag::NotApplicable);
    assert!(m.wait_idle(Duration::from_secs(60)));
    let text = sum(&m, PanelGroup::Compression).text;
    assert!(text.starts_with("Effort "), "{text}");
}

#[test]
fn a_colour_texture_has_its_normal_map_group_not_used() {
    let d = tempfile::tempdir().unwrap();
    let m = rock(d.path());
    let s = sum(&m, PanelGroup::NormalMap);
    assert!(s.all_locked);
    assert_eq!(s.text, "not used for this texture");
    // Baked in is left out by hidden_reason; the other two rows share "normal maps only".
    assert_eq!(
        s.hover.as_deref(),
        Some("Every row here: normal maps only.")
    );
}

#[test]
fn rows_with_different_reasons_point_at_the_rows() {
    // The rule alone, on two views built by hand: no fixture locks every row of one group with two reasons today.
    let v = |field, lock| FieldView {
        field,
        label: "x",
        value: json!(null),
        provenance: "builtin".into(),
        edited: false,
        lock: Some(lock),
    };
    let views = [
        v("mips.normal", "normal maps only"),
        v("mips.normal_base", "another reason"),
    ];
    let d = tempfile::tempdir().unwrap();
    let m = rock(d.path());
    let s = summary(
        PanelGroup::NormalMap,
        &views,
        Target::Cs,
        &job(&m, KEY).profile,
        &E,
    );
    assert!(s.all_locked);
    assert_eq!(
        s.hover.as_deref(),
        Some("Each row below says why it is not used.")
    );
}

#[test]
fn a_pending_plan_does_not_lock_a_group() {
    // A pin of the R1-1 structure, not a behavioural test: the panel's edit_off is `fix_off()`, "Wait for the plan to
    // finish" while a plan is in flight. It lives in Ctx, not in FieldView.lock, and `summary` takes no input that
    // reflects it, so this holds for any implementation that keeps reading `FieldView.lock` only. It fails if a later
    // change feeds the pending state into the views or the summary. NormalMap is the one all-locked group on this
    // texture, so it is compared too, with its hover.
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    let state = |m: &AppModel, g| {
        let s = sum(m, g);
        (s.text, s.all_locked, s.hover)
    };
    let before = (
        state(&m, PanelGroup::Mipmaps),
        state(&m, PanelGroup::NormalMap),
    );
    assert!(before.1 .1, "premise: Normal map is all locked");
    m.replan();
    assert_eq!(m.fix_off().as_deref(), Some("Wait for the plan to finish"));
    let after = (
        state(&m, PanelGroup::Mipmaps),
        state(&m, PanelGroup::NormalMap),
    );
    assert_eq!(before, after);
}

#[test]
fn a_cube_map_summary_reads_both_face_size_forms() {
    let d = tempfile::tempdir().unwrap();
    let mut m = cube_model(d.path(), CUBE);
    let cube = |m: &AppModel| {
        let j = job(m, CUBE);
        summary(
            PanelGroup::Cubemap,
            &panel_views(m.target, j),
            m.target,
            &j.profile,
            &E,
        )
        .text
    };
    m.set_field(CUBE, "cubemap.layout", json!("hstrip"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(cube(&m), "Horizontal strip, auto-sized faces");
    m.set_field(CUBE, "cubemap.face_size", json!(512));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(cube(&m), "Horizontal strip, 512 px faces");
}

#[test]
fn a_format_write_alone_does_not_mark_compression_changed() {
    // The `changed_covers_every_row_source` BC3 case writes format and bc1_search together; this shows the format
    // write is not what flips Compression, so that case proves `effort_view` feeds `changed`.
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    set(&mut m, &[("format", json!("bc3"))]);
    assert!(!sum(&m, PanelGroup::Compression).changed);
    set(&mut m, &[("encoder.bc1_search", json!("fast"))]);
    assert!(sum(&m, PanelGroup::Compression).changed);
}

#[test]
fn a_cube_map_size_says_it_has_no_effect() {
    let d = tempfile::tempdir().unwrap();
    let m = cube_model(d.path(), CUBE);
    let j = job(&m, CUBE);
    let s = summary(
        PanelGroup::Size,
        &panel_views(m.target, j),
        m.target,
        &j.profile,
        &E,
    );
    assert_eq!(s.text, "no effect on a cubemap: Face size sizes it");
}

#[test]
fn changed_covers_every_row_source() {
    use PanelGroup::*;
    for (writes, g) in [
        (vec![("resize", json!("half"))], Size),
        (vec![("mips.filter", json!("box"))], Mipmaps), // filter_view
        (vec![("mips.edge", json!("clamp"))], Mipmaps), // edge_view
        (vec![("mips.levels", json!(4))], Mipmaps),     // levels_view
        // By name: WEIGHTING_OPTIONS[1] is "perceptual", a value of encoder.perceptual, not of encoder.bc7_weights.
        (vec![("encoder.bc7_weights", json!("alpha4"))], Compression), // weighting_view
        // "bc3" (lowercase serde name); Bc1Search is only default or fast (engine config.rs:172).
        (
            vec![
                ("format", json!("bc3")),
                ("encoder.bc1_search", json!("fast")),
            ],
            Compression,
        ), // effort_view, BC3 search
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut m = rock(d.path());
        assert!(!sum(&m, g).changed, "{writes:?} before");
        set(&mut m, &writes);
        assert!(sum(&m, g).changed, "{writes:?}");
    }
    let d = tempfile::tempdir().unwrap();
    let mut m = cube_model(d.path(), CUBE);
    m.set_field(CUBE, "cubemap.yaw", json!(90.0));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let j = job(&m, CUBE);
    let s = summary(Cubemap, &panel_views(m.target, j), m.target, &j.profile, &E);
    assert!(s.changed);
}
