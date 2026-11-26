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
    let table: [(&str, Option<PanelGroup>); 35] = [
        ("subtype", None),
        ("format", None),
        ("channels.green_convention", None),
        ("alpha.policy", Some(Alpha)),
        ("alpha.threshold", Some(Alpha)),
        ("alpha.compare", Some(Alpha)),
        ("mips.coverage", Some(Alpha)),
        ("encoder.bc1_alpha", Some(Alpha)),
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
        // Note LR, LR8: the Channel guard row and its four fields; exact opaque is the guard's A, in Compression.
        ("encoder.channel_guard", Some(Compression)),
        ("encoder.channel_guard.r", Some(Compression)),
        ("encoder.channel_guard.g", Some(Compression)),
        ("encoder.channel_guard.b", Some(Compression)),
        ("alpha.exact_opaque", Some(Compression)),
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
fn the_header_hit_target_is_24_px_and_the_row_short_of_the_scroll_bar() {
    let h = head_harness(400.0, mipmaps());
    let r = h.get_by_label_contains("Mipmaps, ").rect();
    assert!(r.height() >= 24.0, "{r:?}");
    // The harness keeps an 8 px margin on each side of the 400 px window: the row is the 384 px between them, less the
    // scroll bar's width at its right end (LQ3).
    let s = h.ctx.global_style().spacing.scroll;
    let row = 384.0 - (s.bar_width + s.bar_outer_margin);
    assert!((r.width() - row).abs() < 1.0, "{r:?}, want {row}");
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

use engine::config::{ColorTag, Format, GreenConvention, Subtype, Target};
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
    for (field, value) in writes {
        assert_eq!(
            m.project.as_ref().unwrap().file_value(KEY, field),
            Some(value.clone()),
            "premise: {field} written"
        );
    }
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

// Task 4: the first screen and the groups, drawn.

use engine::codes::Code;
use sdc_gui::model::bottom::{At, Level, Problem};
use sdc_gui::model::SetTarget;
use sdc_gui::settings::GuiSettings;
use sdc_gui::ui::settings_panel::{first_screen_problems, problem_lines, Action, Surface};

/// `texture_panel_layout.rs::sized_harness`, copied: the whole window, the preview and the 3D view off, so the centre
/// is the only surface and each "{Title}, " name is unique. Fonts go in before kittest's first frame.
fn harness(m: AppModel, w: f32, h: f32) -> Harness<'static, AppModel> {
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(w, h))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    h
}

/// `support::opened`'s fixture, with nothing decided.
fn opened(d: &std::path::Path) -> AppModel {
    support::opened(d)
}

/// A 64x64 RGB PNG with a 116-character CJK name, decided Diffuse and selected. Reused by the first-screen tests.
fn long_named(d: &std::path::Path) -> (AppModel, String) {
    let name = format!("{}.png", "皮".repeat(116));
    let dir = d.join("textures/landscape");
    std::fs::create_dir_all(&dir).unwrap();
    image::RgbImage::from_fn(64, 64, |x, y| image::Rgb([x as u8, y as u8, 9]))
        .save(dir.join(&name))
        .unwrap();
    let mut m = opened(d);
    let key = format!("landscape/{name}");
    m.decide(&key, Answer::Subtype(engine::config::Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(&key);
    (m, key)
}

/// A Warning row at `KEY`; no code, so `shown` is the text itself.
fn problem_row(text: &str) -> Problem {
    Problem {
        level: Level::Warning,
        at: At::Texture(KEY.into()),
        text: text.into(),
        code: None,
        also: vec![],
        fix: None,
        second_fix: None,
        suggested: None,
        detail: None,
        carried: false,
        intended: false,
        mark: None,
        target: None,
    }
}

#[test]
fn the_first_screen_comes_before_every_group_header() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(rock(d.path()), 1280.0, 800.0);
    let first = h.get_by_label_contains("Alpha, ").rect().top(); // only the header's name contains "Alpha, "
    for l in ["Texture type", "Format"] {
        assert!(
            h.get_by_role_and_label(Role::ComboBox, l).rect().bottom() <= first,
            "{l}"
        );
    }
    assert!(h.get_by_label_contains("Output: ").rect().bottom() <= first);
}

/// `used_by_meshes.rs`'s scan database, copied: texture `tex` used by two NIFs (one copy each).
fn scan_db_for(db: &std::path::Path, tex: &str) {
    let s = modscan::store::Store::open(db).unwrap();
    let c = s.conn();
    c.execute("INSERT INTO path(id, p) VALUES (1, ?1)", [tex])
        .unwrap();
    c.execute_batch(
        "INSERT INTO texset(id, n, t0) VALUES (1, 1, 1);
         INSERT INTO tex_use(tex_path_id, texset_id, slot) VALUES (1, 1, 0);",
    )
    .unwrap();
    for (i, nif) in ["meshes/a.nif", "meshes/b.nif"].iter().enumerate() {
        let id = i as i64 + 2;
        c.execute(
            "INSERT INTO path(id, p) VALUES (?1, ?2)",
            rusqlite::params![id, nif],
        )
        .unwrap();
        c.execute(
            "INSERT INTO origin(id, kind, name, root, priority, enabled) VALUES (?1, 1, ?2, ?2, ?3, 1)",
            rusqlite::params![id, format!("Mod {i}"), 7 + i as i64],
        )
        .unwrap();
        c.execute(
            "INSERT INTO nif_copy(id, path_id, origin_id, parse_status, is_derived) VALUES (?1, ?1, ?1, 0, 0)",
            rusqlite::params![id],
        )
        .unwrap();
        c.execute(
            "INSERT INTO shape(copy_id, block, kind, shader_kind, shader_type, slsf2, texset_id, alpha_flags, alpha_threshold) VALUES (?1, 0, 'BSTriShape', 1, 0, 0, 1, NULL, NULL)",
            rusqlite::params![id],
        )
        .unwrap();
    }
}

#[test]
fn used_by_is_a_group_header_in_3d_s_place() {
    let d = tempfile::tempdir().unwrap();
    let m = rock(d.path());
    let mut h = harness(m, 1280.0, 800.0);
    // The scan key of the item, as the plan names its output (`used_by_meshes.rs::tex_of`).
    let j = job(h.state(), KEY);
    let stem = j.output.file_stem().unwrap().to_string_lossy().into_owned();
    let tex = format!("{}.dds", engine::shader::mesh_key(&j.game_path, &stem));
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    scan_db_for(&db, &tex);
    h.state_mut().scan_db = Some(db);
    let t = std::time::Instant::now();
    loop {
        h.state_mut().poll();
        if matches!(h.state_mut().used_by_for(KEY), Some(Some(_))) {
            break;
        }
        assert!(
            t.elapsed() < Duration::from_secs(30),
            "the read never landed"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    h.run();
    h.run();
    let n = h.get_by_label("Used by 2 meshes, collapsed");
    assert_eq!(n.accesskit_node().role(), Role::DisclosureTriangle);
    assert!(n.rect().height() >= 24.0);
    assert!(
        n.rect().top()
            < h.get_by_role_and_label(Role::ComboBox, "Texture type")
                .rect()
                .top(),
        "LM13's place, unchanged"
    );
    // Closed at start; a click opens it, and the open state is not a setting.
    h.get_by_label("Used by 2 meshes, collapsed").click();
    h.run();
    h.get_by_label("Used by 2 meshes, expanded");
    assert!(h.state().settings.texture_groups.is_empty());
}

#[test]
fn a_closed_header_follows_an_edit_made_while_it_is_drawn() {
    // The panel_cache must rebuild when the plan lands, not only when the document moves (Review Focus 1). Every
    // other test here builds its harness after wait_idle, so none would see a stale cache.
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    let mut h = harness(m, 1280.0, 800.0);
    h.run();
    let before = h
        .get_by_label_contains("Mipmaps, ")
        .accesskit_node()
        .label()
        .unwrap()
        .to_string();
    assert!(!before.contains(", changed"), "{before}");
    h.state_mut().set_field(KEY, "mips.filter", json!("box"));
    h.run();
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run();
    let after = h
        .get_by_label_contains("Mipmaps, ")
        .accesskit_node()
        .label()
        .unwrap()
        .to_string();
    assert!(
        after.starts_with("Mipmaps, collapsed, changed: Box"),
        "{after}"
    );
}

#[test]
fn the_type_group_is_gone() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(rock(d.path()), 1280.0, 800.0);
    assert!(h.query_by_label_contains("Type, ").is_none());
    // The old header was named by its title alone.
    assert!(h.query_by_label("Type").is_none());
}

#[test]
fn baked_in_sits_in_the_normal_map_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let key = "landscape/rock01_normal.png";
    m.decide(key, Answer::Subtype(engine::config::Subtype::Msn));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(key);
    panel_groups::set_open(&mut m.settings, PanelGroup::NormalMap, false); // R3-2: the state this test needs, set here
    let mut h = harness(m, 1280.0, 800.0);
    h.get_by_label_contains("Normal map, collapsed").click();
    h.run();
    let head = h.get_by_label_contains("Normal map, expanded").rect();
    let baked = h.get_by_role_and_label(Role::ComboBox, "Baked in").rect();
    assert!(baked.top() > head.bottom());
    assert!(baked.top() < h.get_by_label_contains("Size, ").rect().top());
}

#[test]
fn disabled_rows_still_say_why_inside_a_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path()); // rock01 has no alpha: not a cutout
    panel_groups::set_open(&mut m.settings, PanelGroup::Alpha, false);
    let mut h = harness(m, 1280.0, 800.0);
    h.get_by_label_contains("Alpha, collapsed").click();
    h.run();
    assert!(h.query_by_label_contains("Alpha, expanded").is_some());
    assert!(
        h.query_all_by_label("used only by cutout and cutout + blend")
            .count()
            >= 1
    );
}

#[test]
fn an_unused_group_says_so_while_closed_and_opens_to_each_row_s_reason() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::NormalMap, false);
    let mut h = harness(m, 1280.0, 800.0);
    let head = h.get_by_label("Normal map, collapsed: not used for this texture");
    assert!(!head.accesskit_node().is_disabled());
    head.click();
    h.run();
    // TPC-R1 (JR6): open, the rows say why, so the header drops its summary.
    assert!(
        h.query_by_label("Normal map, expanded").is_some(),
        "it still opens, with no summary"
    );
    assert!(h
        .query_by_label_contains("Normal map, expanded: ")
        .is_none());
    assert!(
        h.query_all_by_label("normal maps only").count() >= 2,
        "Normal handling and Normal base level say why"
    );
}

#[test]
fn headers_toggle_while_the_plan_is_pending() {
    // The resize_field.rs:190-222 pattern: an edit with no wait leaves a plan in flight, and a harness that only
    // draws (ui::draw never polls the planner) keeps it in flight until wait_idle. edit_off = fix_off() = "Wait for
    // the plan to finish" (model/review.rs:1299-1304).
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Size, true);
    m.set_field(KEY, "mips.filter", json!("box"));
    assert_eq!(m.fix_off().as_deref(), Some("Wait for the plan to finish"));
    let mut h = harness(m, 1280.0, 800.0);
    h.run_steps(3);
    h.get_by_label_contains("Size, expanded").click();
    h.run_steps(3);
    assert!(
        h.query_by_label_contains("Size, collapsed").is_some(),
        "the header still works"
    );
    h.get_by_label_contains("Size, collapsed").click();
    h.run_steps(3);
    assert!(
        h.get_by_role_and_label(Role::ComboBox, "Resize")
            .accesskit_node()
            .is_disabled(),
        "the row waits"
    );
    assert!(
        h.state().fix_off().is_some(),
        "still pending: the test proved what it meant to"
    );
}

#[test]
fn the_cubemap_and_sources_sections_use_the_group_header() {
    let d = tempfile::tempdir().unwrap();
    let mut m = cube_model(d.path(), CUBE);
    panel_groups::set_open(&mut m.settings, PanelGroup::Sources, false);
    let h = harness(m, 1280.0, 800.0);
    assert_eq!(
        h.get_by_label_contains("Cubemap, ").accesskit_node().role(),
        Role::DisclosureTriangle
    );
    let src = h.get_by_label_contains("Where these settings come from, ");
    assert_eq!(src.accesskit_node().role(), Role::DisclosureTriangle);
    assert!(src
        .accesskit_node()
        .label()
        .unwrap()
        .contains(", collapsed"));
}

/// A `first_screen_problems` harness over `rows`, `notes` and `set` (its lines built once, as the panel's slot does),
/// its state the actions it pushed.
fn problems_harness(
    rows: Vec<Problem>,
    notes: usize,
    set: Option<SetTarget>,
) -> Harness<'static, Vec<Action>> {
    let lines = problem_lines(rows.len(), notes, set);
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .build_ui_state(
            move |ui, acts: &mut Vec<Action>| first_screen_problems(ui, &rows, &lines, acts),
            vec![],
        );
    h.run();
    h
}

#[test]
fn the_problem_list_on_the_first_screen_shows_three_then_a_button() {
    let rows: Vec<Problem> = (0..5)
        .map(|i| problem_row(&format!("Problem {i}")))
        .collect();
    let mut h = problems_harness(rows, 2, Some(SetTarget::Key("landscape/rock01".into())));
    assert!(h.query_by_label("Problem 2").is_some());
    assert!(h.query_by_label("Problem 3").is_none());
    assert!(h.query_by_label("2 notes in Problems").is_some());
    h.get_by_label("2 more problems: open Problems").click();
    h.run();
    assert_eq!(
        h.state(),
        &vec![Action::OpenProblems(Some(SetTarget::Key(
            "landscape/rock01".into()
        )))]
    );
}

#[test]
fn more_problems_names_a_key_less_set_by_its_id() {
    // JP5: a key-less set is filtered by its id, through open_set_problems (which also sets name_kind)
    let rows: Vec<Problem> = (0..4)
        .map(|i| problem_row(&format!("Problem {i}")))
        .collect();
    let mut h = problems_harness(rows, 0, Some(SetTarget::Id("set-7".into())));
    h.get_by_label("1 more problem: open Problems").click();
    h.run();
    assert_eq!(
        h.state(),
        &vec![Action::OpenProblems(Some(SetTarget::Id("set-7".into())))]
    );
}

#[test]
fn is_panel_problem_drops_notes_and_marked_rows() {
    // a mapped code (CUBE-NO-MIPS, in FIX_FIELDS), so only the predicate can keep it off the panel
    let mut p = problem_row("x");
    p.code = Some(Code::CubeNoMips);
    assert!(panel_groups::is_panel_problem(&p), "a counted Warning");
    let mut marked = p.clone();
    marked.intended = true;
    assert!(
        !panel_groups::is_panel_problem(&marked),
        "marked as intended (3a)"
    );
    let mut info = p.clone();
    info.level = Level::Info;
    assert!(
        !panel_groups::is_panel_problem(&info),
        "an Info row is a Note (3a is_note)"
    );
    // A Warning whose code Plan 3a classes as a Note (row_class), which is_note does not see.
    let mut note_code = p.clone();
    note_code.code = Some(Code::DimNpot);
    assert!(
        !panel_groups::is_panel_problem(&note_code),
        "a Note-class code"
    );
}

#[test]
fn one_more_problem_and_one_note_read_in_the_singular() {
    let rows: Vec<Problem> = (0..4)
        .map(|i| problem_row(&format!("Problem {i}")))
        .collect();
    let mut h = problems_harness(rows, 1, None);
    assert!(h.query_by_label("1 note in Problems").is_some());
    h.get_by_label("1 more problem: open Problems").click();
    h.run();
    assert_eq!(
        h.state(),
        &vec![Action::OpenProblems(None)],
        "no set: Problems unfiltered"
    );
}

#[test]
fn three_problems_need_no_more_button() {
    let rows: Vec<Problem> = (0..3)
        .map(|i| problem_row(&format!("Problem {i}")))
        .collect();
    let h = problems_harness(rows, 0, None);
    assert!(h.query_by_label("Problem 2").is_some());
    assert!(h.query_by_label_contains("more problem").is_none());
    assert!(h.query_by_label_contains("in Problems").is_none());
}

#[test]
fn a_header_click_saves_the_group() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("gui.toml");
    let mut m = rock(d.path());
    m.settings_path = Some(p.clone()); // model/mod.rs:703, the path save_settings (1609) writes
    panel_groups::set_open(&mut m.settings, PanelGroup::Size, true);
    let mut h = harness(m, 1280.0, 800.0);
    h.get_by_label_contains("Size, expanded").click();
    h.run();
    assert_eq!(
        GuiSettings::load(&p).0.texture_groups.get("size"),
        Some(&false)
    );
}

// Two surfaces, one harness: the state carries which surface draws, so a click finds one header and the check after
// it finds the other surface's header alone (R2-4: no lookup with two matches).
#[derive(Clone, Copy, PartialEq)]
enum Draw {
    Centre,
    Strip,
    Window,
}

fn surfaces(m: AppModel) -> Harness<'static, (AppModel, Draw)> {
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, (m, d): &mut (AppModel, Draw)| match d {
                Draw::Centre => sdc_gui::ui::settings_panel::draw(ui, m, KEY, Surface::Centre),
                Draw::Strip => sdc_gui::ui::settings_panel::draw(ui, m, KEY, Surface::Strip),
                Draw::Window => {
                    m.settings.set_settings_open = true;
                    sdc_gui::ui::set_preview::settings_window(ui.ctx(), m, true)
                }
            },
            (m, Draw::Centre),
        );
    h.run();
    h
}

#[test]
fn the_preview_strip_shares_the_open_state() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, true);
    let mut h = surfaces(m);
    h.state_mut().1 = Draw::Strip;
    h.run();
    h.get_by_label_contains("Mipmaps, expanded").click();
    h.run();
    h.state_mut().1 = Draw::Centre;
    h.run();
    assert!(
        h.query_by_label_contains("Mipmaps, collapsed").is_some(),
        "the centre reads the strip's click"
    );
}

#[test]
fn the_3d_settings_window_shares_the_open_state() {
    // settings_window (set_preview.rs:502) draws the selected item's panel in a Window; rock() selects rock01
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Size, true);
    let mut h = surfaces(m);
    h.state_mut().1 = Draw::Window;
    h.run_steps(3);
    h.get_by_label_contains("Size, expanded").click();
    h.run_steps(3);
    h.state_mut().1 = Draw::Centre;
    h.run();
    assert!(
        h.query_by_label_contains("Size, collapsed").is_some(),
        "the centre reads the window's click"
    );
}

#[test]
fn headers_fit_at_1100_1280_and_1600() {
    for w in [1100.0, 1280.0, 1600.0] {
        let d = tempfile::tempdir().unwrap();
        let (mut m, key) = long_named(d.path()); // a 116-character CJK file name, decided Diffuse and selected
        for (f, v) in [
            ("resize", json!("half")),
            ("resize_relative_to", json!("slot8")),
        ] {
            m.set_field(&key, f, v);
        }
        assert!(m.wait_idle(Duration::from_secs(60)));
        // TPC-R1: only a closed header draws its summary, so close every group (R3-2: never rely on the default).
        for g in PanelGroup::ALL {
            panel_groups::set_open(&mut m.settings, g, false);
        }
        let h = harness(m, w, 800.0);
        let centre = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
        let heads: Vec<egui::Rect> = h
            .query_all_by_role(Role::DisclosureTriangle)
            .map(|n| n.rect())
            .collect();
        for n in h.query_all_by_role(Role::DisclosureTriangle) {
            assert!(
                n.rect().max.x <= centre.max.x + 0.5,
                "{w}: {:?}",
                n.accesskit_node().label()
            );
        }
        // The header rect is the whole row, so also check what is painted in it: "● changed" (Size has one) ends at
        // or inside its own header, as Task 2's changed_is_never_cut_at_1100_px checks.
        let changed: Vec<_> = h
            .query_all_by_label("● changed")
            .map(|n| n.rect())
            .collect();
        assert!(
            !changed.is_empty(),
            "{w}: premise: the Size group is changed"
        );
        for c in changed {
            let head = heads
                .iter()
                .find(|r| r.top() <= c.center().y && c.center().y <= r.bottom())
                .expect("its header");
            assert!(
                c.max.x <= head.max.x + 0.5,
                "{w}: changed {c:?} past its header {head:?}"
            );
        }
    }
}

/// FM35: while the alpha question is open, its LOSS-ALPHA-DATA line is the question's, not a first-screen problem row.
#[test]
fn the_open_alpha_question_is_not_listed_as_a_problem() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let leaf = "landscape/leaf.png";
    m.decide(leaf, Answer::Subtype(engine::config::Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(leaf);
    let proposal = sdc_gui::model::bottom::problems(&m)
        .iter()
        .find(|p| p.at == At::Texture(leaf.into()) && p.level == Level::Decide)
        .map(|p| p.text.clone())
        .expect("the Undecided row that says the alpha question");
    let h = harness(m, 1280.0, 800.0);
    h.get_by_label("Confirm alpha"); // the question is on screen
    let lines = h
        .query_all_by_label_contains(&proposal)
        .filter(|n| n.accesskit_node().role() == Role::Label)
        .count();
    assert_eq!(lines, 0, "no first-screen row repeats the question");
}

const FLAT: &str = "a/flat.png";

/// One slot per surface holds one texture at a time: the next texture drawn on the same surface rebuilds it, so the
/// header summary and the problem rows are the new texture's, though no model revision moved.
#[test]
fn the_caches_follow_the_drawn_key() {
    let d = tempfile::tempdir().unwrap();
    let mut m = cube_model(d.path(), CUBE);
    m.set_field(CUBE, "mips.levels", json!("none")); // CUBE-NO-MIPS: a Problem on the cube only
    m.set_field(FLAT, "mips.filter", json!("box")); // a changed Mipmaps row on the flat texture only
    assert!(m.wait_idle(Duration::from_secs(60)));
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    let problem = sdc_gui::model::bottom::problems(&m)
        .iter()
        .find(|p| p.at == At::Texture(CUBE.into()) && panel_groups::is_panel_problem(p))
        .map(|p| p.shown().to_string())
        .expect("premise: the cube has a Problem row");
    let mut h = harness(m, 1280.0, 800.0);
    let label = |h: &Harness<'static, AppModel>| {
        h.get_by_label_contains("Mipmaps, ")
            .accesskit_node()
            .label()
            .unwrap()
            .to_string()
    };
    let listed = |h: &Harness<'static, AppModel>| {
        h.query_all_by_label(&problem)
            .any(|n| n.accesskit_node().role() == Role::Label)
    };
    // The cube's problem forces Mipmaps open (Task 5); a closed header is where the summary shows, so close it.
    h.get_by_label_contains("Mipmaps, expanded").click();
    h.run_steps(10);
    assert!(
        label(&h).ends_with("None (base level only)"),
        "the cube's summary: {}",
        label(&h)
    );
    assert!(listed(&h), "the cube's problem is on its first screen");
    h.state_mut().select_item(FLAT);
    h.run();
    assert!(
        label(&h).ends_with("Box, full chain"),
        "the flat texture's summary: {}",
        label(&h)
    );
    assert!(!listed(&h), "the flat texture has no such problem");
}

/// A problem on this texture shows on its first screen (drawn from the real problem list, not a hand-built row).
#[test]
fn a_texture_s_problems_are_on_its_first_screen() {
    let d = tempfile::tempdir().unwrap();
    let mut m = cube_model(d.path(), CUBE);
    m.set_field(CUBE, "mips.levels", json!("none"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let rows: Vec<Problem> = sdc_gui::model::bottom::problems(&m)
        .iter()
        .filter(|p| p.at == At::Texture(CUBE.into()) && panel_groups::is_panel_problem(p))
        .cloned()
        .collect();
    assert!(!rows.is_empty(), "premise: CUBE-NO-MIPS is a Problem row");
    let shown = rows[0].shown().to_string();
    let h = harness(m, 1280.0, 800.0);
    assert!(
        h.query_all_by_label(&shown)
            .any(|n| n.accesskit_node().role() == Role::Label),
        "the first screen lists {shown:?}"
    );
}

/// `ui_smoke.rs::land_partial`, copied: poll until the partial of the newest edit is parked, then handle it alone. The
/// full plans stay parked in `full`, so none can land in between (`hold_plan_results`).
fn land_partial(h: &mut Harness<'static, AppModel>, full: &mut Vec<sdc_gui::worker::WorkerMsg>) {
    use sdc_gui::worker::WorkerMsg;
    let m = h.state_mut();
    let end = std::time::Instant::now() + Duration::from_secs(60);
    let mut partial = None;
    while partial.is_none() {
        assert!(
            std::time::Instant::now() < end,
            "the partial did not arrive"
        );
        std::thread::sleep(Duration::from_millis(10));
        m.poll();
        for x in m.take_held() {
            match x {
                WorkerMsg::Partial { seq, .. } if seq == m.edit_seq => partial = Some(x),
                WorkerMsg::Partial { .. } => {}
                x => full.push(x),
            }
        }
    }
    m.handle_held(partial.unwrap());
    assert!(m.busy(), "the full plan is still out");
}

/// CODING_STANDARDS, "The fast re-plan keeps what the full plan keeps": the panel's cache follows the fast re-plan
/// (`apply_partial`) with the full plan still out, and the full plan (`apply_plan`) on its own moves the inputs too.
#[test]
fn a_closed_header_follows_the_fast_plan_and_the_full_plan_each_alone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    m.hold_plan_results = true;
    let mut h = harness(m, 1280.0, 800.0);
    let name = |h: &Harness<'static, AppModel>| {
        h.get_by_label_contains("Mipmaps, ")
            .accesskit_node()
            .label()
            .unwrap()
            .to_string()
    };
    // Box is rock01's own default, so the edit changes the mark (the row's provenance), not the summary text.
    assert!(!name(&h).contains(", changed"), "{}", name(&h));
    h.state_mut().set_field(KEY, "mips.filter", json!("box"));
    h.run();
    assert!(
        !name(&h).contains(", changed"),
        "the panel shows the last plan until one lands: {}",
        name(&h)
    );
    let mut full = vec![];
    land_partial(&mut h, &mut full);
    h.run();
    assert_eq!(
        name(&h),
        "Mipmaps, collapsed, changed: Box, full chain",
        "the fast plan alone"
    );
    let rev = h.state().items_rev();
    let m = h.state_mut();
    m.hold_plan_results = false;
    for x in full {
        m.sender().send(x).unwrap();
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
    h.run();
    assert!(
        h.state().items_rev() > rev,
        "the full plan alone moves the cache's inputs too"
    );
    assert!(
        name(&h).starts_with("Mipmaps, collapsed, changed: Box"),
        "after the full plan: {}",
        name(&h)
    );
}

/// TPC-T4-1: while the Undecided card asks the type it has its own picker, so the first screen drops the Texture type
/// row; the Helper row (the card has none) stays.
#[test]
fn the_undecided_card_replaces_the_texture_type_row_and_keeps_the_helper_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path()); // nothing decided: rock01 is undecided
    m.select_item(KEY);
    let h = harness(m, 1280.0, 1600.0);
    h.get_by_role_and_label(Role::ComboBox, "Type for this texture");
    assert!(
        h.query_by_role_and_label(Role::ComboBox, "Texture type")
            .is_none(),
        "the type is asked once"
    );
    h.get_by_role_and_label(Role::CheckBox, "Helper");
    // Decided, the card is gone and the row is back.
    let d = tempfile::tempdir().unwrap();
    let h = harness(rock(d.path()), 1280.0, 800.0);
    h.get_by_role_and_label(Role::ComboBox, "Texture type");
    assert!(h
        .query_by_role_and_label(Role::ComboBox, "Type for this texture")
        .is_none());
}

/// The first screen reads Texture type, then Format (`FIRST_SCREEN` order), then the Output line.
#[test]
fn texture_type_comes_before_format() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(rock(d.path()), 1280.0, 800.0);
    let top = |l: &str| h.get_by_role_and_label(Role::ComboBox, l).rect().top();
    assert!(top("Texture type") < top("Format"));
    assert!(top("Format") < h.get_by_label_contains("Output: ").rect().top());
}

/// TPC-T4-2: an undecided texture shows the card, not also its "Undecided: ..." row from the problem list.
#[test]
fn an_undecided_texture_shows_the_card_and_no_undecided_problem_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.select_item(KEY);
    let row = sdc_gui::model::bottom::problems(&m)
        .iter()
        .find(|p| p.at == At::Texture(KEY.into()) && p.level == Level::Decide)
        .map(|p| p.text.clone())
        .expect("premise: the list has the Undecided row");
    assert!(row.starts_with("Undecided: "), "{row}");
    let h = harness(m, 1280.0, 1600.0);
    h.get_by_label("Undecided"); // the card's title
    assert_eq!(
        h.query_all_by_label_contains("Undecided: ")
            .filter(|n| n.accesskit_node().role() == Role::Label)
            .count(),
        0,
        "no first-screen row repeats the card"
    );
}

/// The row filter alone, on hand-built rows: notes are counted, never listed; the Decide row and each code another
/// widget says are left out (by `code` or `also`); LOSS-ALPHA-DATA only while the alpha question is open.
#[test]
fn the_first_screen_leaves_out_what_another_widget_says() {
    use sdc_gui::ui::settings_panel::panel_problem_rows;
    let kept = problem_row("kept");
    let mut decide = problem_row("Undecided: x");
    decide.level = Level::Decide;
    let mut note = problem_row("note");
    note.level = Level::Info;
    let mut other = problem_row("other texture");
    other.at = At::Texture("other.png".into());
    let coded = |c: Code, also: bool| {
        let mut p = problem_row("coded");
        if also {
            p.also = vec![c];
        } else {
            p.code = Some(c);
        }
        p
    };
    let mut all = vec![kept.clone(), decide, note, other];
    for c in [
        Code::SugLowConfidence,
        Code::InGreenUnstated,
        Code::ChkGreenGuess,
    ] {
        all.push(coded(c, false));
        all.push(coded(c, true));
    }
    let (rows, notes) = panel_problem_rows(&all, KEY, false);
    assert_eq!(rows, vec![kept.clone()]);
    // A note is any row of this texture that is not a panel problem (the Info row, and the coded rows whose code
    // is a note by `row_class`): counted, never listed.
    let this = |p: &&Problem| p.at == At::Texture(KEY.into());
    let want = all
        .iter()
        .filter(this)
        .filter(|p| !panel_groups::is_panel_problem(p))
        .count();
    assert!(want >= 1);
    assert_eq!(notes, want);
    // LOSS-ALPHA-DATA: listed while no question is open, left out while it is, by `code` or `also`.
    let loss = [
        coded(Code::LossAlphaData, false),
        coded(Code::LossAlphaData, true),
    ];
    assert_eq!(panel_problem_rows(&loss, KEY, false).0.len(), 2);
    assert_eq!(panel_problem_rows(&loss, KEY, true).0.len(), 0);
}

/// `texture_problems` keeps its slot by `Arc::ptr_eq` on the problem list, which only fires if `problems` hands out the
/// same `Arc` while nothing moved (`model::cached`).
#[test]
fn the_problem_list_is_one_arc_until_something_moves() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    let a = sdc_gui::model::bottom::problems(&m);
    assert!(std::sync::Arc::ptr_eq(
        &a,
        &sdc_gui::model::bottom::problems(&m)
    ));
    m.set_field(KEY, "mips.filter", json!("box"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(!std::sync::Arc::ptr_eq(
        &a,
        &sdc_gui::model::bottom::problems(&m)
    ));
}

/// TPC-T4-4: the groups draw in `PanelGroup::ALL` order.
#[test]
fn the_groups_draw_in_panel_order() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(rock(d.path()), 1280.0, 2000.0);
    let tops: Vec<f32> = [
        PanelGroup::Alpha,
        PanelGroup::Mipmaps,
        PanelGroup::NormalMap,
        PanelGroup::Size,
        PanelGroup::Compression,
        PanelGroup::Sources,
    ]
    .iter()
    .map(|g| {
        h.get_by_label_contains(&format!("{}, ", g.title()))
            .rect()
            .top()
    })
    .collect();
    assert!(tops.windows(2).all(|w| w[0] < w[1]), "{tops:?}");
    // The table the loop follows: no group outside it is missing from `ALL`.
    assert_eq!(PanelGroup::ALL.len(), 8);
}

/// The panel's cache builds from the job `draw` passes, which carries the open alpha question's proposal. The proposal
/// is a function of the item, but it is keyed anyway: a change that moves no revision (here, a hand edit of the item's
/// analysis) must not leave the Alpha header on the old proposal.
#[test]
fn the_cache_follows_the_alpha_proposal() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let leaf = "landscape/leaf.png";
    m.decide(leaf, Answer::Subtype(engine::config::Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(leaf);
    panel_groups::set_open(&mut m.settings, PanelGroup::Alpha, false);
    let mut h = harness(m, 1280.0, 800.0);
    let name = |h: &Harness<'static, AppModel>| {
        h.get_by_label_contains("Alpha, ")
            .accesskit_node()
            .label()
            .unwrap()
            .to_string()
    };
    assert!(name(&h).contains("Cutout"), "the proposal: {}", name(&h));
    let i = h.state().items.iter().position(|i| i.key == leaf).unwrap();
    h.state_mut().items[i].analysis = None; // no analysis: no proposal, the profile's own policy
    h.run();
    assert!(!name(&h).contains("Cutout"), "{}", name(&h));
}

/// A profile may hide one of the Mipmaps rows: the others are still drawn (and no group's rows are skipped).
#[test]
fn the_mipmaps_group_draws_the_rows_that_are_present() {
    use sdc_gui::ui::settings_panel::{group_section, Ctx};
    let d = tempfile::tempdir().unwrap();
    let m = rock(d.path());
    let job = job(&m, KEY).clone();
    let all = panel_views(m.target, &job);
    let mut groups = std::collections::BTreeMap::new();
    groups.insert("mipmaps".to_string(), true);
    let drawn = |dropped: &'static str| {
        let views: Vec<FieldView> = all.iter().filter(|v| v.field != dropped).cloned().collect();
        let mut h = Harness::builder()
            .with_context_setup(install_fonts)
            .with_size(egui::vec2(1280.0, 800.0))
            .build_ui_state(
                |ui,
                 (job, groups, views): &mut (
                    engine::policy::ResolvedJob,
                    std::collections::BTreeMap<String, bool>,
                    Vec<FieldView>,
                )| {
                    let forced = std::collections::BTreeSet::new();
                    let cx = Ctx {
                        key: "k",
                        job,
                        target: Target::Cs,
                        busy: false,
                        compact: false,
                        alpha_asked: false,
                        edit_off: None,
                        warnings: &[],
                        surface: Surface::Centre,
                        groups,
                        forced: &forced,
                        problem_groups: &std::collections::BTreeMap::new(),
                        summaries: &[],
                        reveal: None,
                    };
                    let mut acts = vec![];
                    group_section(ui, &cx, PanelGroup::Mipmaps, views, &mut acts);
                },
                (job.clone(), groups.clone(), views),
            );
        h.run();
        ["Levels", "Filter", "Edge"].map(|l| h.query_by_role_and_label(Role::ComboBox, l).is_some())
    };
    assert_eq!(drawn("none"), [true, true, true], "premise: all three");
    assert_eq!(drawn("mips.edge"), [true, true, false]);
    assert_eq!(drawn("mips.filter"), [true, false, true]);
    assert_eq!(drawn("mips.levels"), [false, true, true]);
}

/// "+n more" opens Problems on the selected item's set view (a key-less set by its id), or on the set key of another
/// drawn item (the strip or the 3D window), and on no set when the texture is in none.
#[test]
fn the_more_button_names_the_selected_items_set_or_the_drawn_items_set_key() {
    use sdc_gui::ui::settings_panel::problems_target;
    let d = tempfile::tempdir().unwrap();
    let m = rock(d.path()); // rock01 is selected
    let active = m.resolved_selection();
    let view = active.view().expect("rock01 is in a set");
    assert_eq!(
        problems_target(&m, KEY),
        Some(SetTarget::of(&view.key, &view.id))
    );
    let leaf = "landscape/leaf.png";
    let leaf_set = m
        .item(leaf)
        .and_then(|i| i.set_key.clone())
        .expect("leaf is in a set");
    assert_ne!(leaf_set, view.key, "premise: another set");
    assert_eq!(problems_target(&m, leaf), Some(SetTarget::Key(leaf_set)));
    assert_eq!(problems_target(&m, "no/such.png"), None);
}

// Task 5: a problem opens its group once per selection.

use sdc_gui::model::Selection;

/// `cube_model`'s cube with `mips.levels = none`: raises CUBE-NO-MIPS on Mipmaps. Selected.
fn cube_without_mips(dir: &std::path::Path) -> (AppModel, String) {
    let mut m = cube_model(dir, CUBE);
    m.set_field(CUBE, "mips.levels", json!("none"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    (m, CUBE.to_string())
}

fn closed_mipmaps(dir: &std::path::Path) -> (Harness<'static, AppModel>, String) {
    let (mut m, key) = cube_without_mips(dir);
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    m.select_item(&key);
    let mut h = harness(m, 1280.0, 800.0);
    h.run();
    (h, key)
}

#[test]
fn selecting_a_texture_with_a_problem_opens_its_group() {
    let d = tempfile::tempdir().unwrap();
    let (h, _) = closed_mipmaps(d.path());
    let n = h.get_by_label_contains("Mipmaps, expanded");
    assert!(n
        .accesskit_node()
        .label()
        .unwrap()
        .contains(", has a problem"));
    assert_eq!(
        h.state().settings.texture_groups.get("mipmaps"),
        Some(&false),
        "nothing saved"
    );
}

#[test]
fn the_forced_open_ends_with_the_selection() {
    let d = tempfile::tempdir().unwrap();
    let (mut h, _) = closed_mipmaps(d.path());
    h.state_mut()
        .decide(FLAT, Answer::Subtype(engine::config::Subtype::Diffuse));
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.state_mut().select_item(FLAT);
    h.run();
    assert!(h.query_by_label_contains("Mipmaps, collapsed").is_some());
}

#[test]
fn closing_a_forced_group_keeps_it_closed() {
    let d = tempfile::tempdir().unwrap();
    let (mut h, _) = closed_mipmaps(d.path());
    h.get_by_label_contains("Mipmaps, expanded").click();
    h.run_steps(10);
    assert!(h.query_by_label_contains("Mipmaps, collapsed").is_some());
}

#[test]
fn a_replan_does_not_reopen_a_group_the_user_closed() {
    let d = tempfile::tempdir().unwrap();
    let (mut h, _) = closed_mipmaps(d.path());
    h.get_by_label_contains("Mipmaps, expanded").click();
    h.run();
    h.state_mut().replan();
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run_steps(5);
    assert!(h.query_by_label_contains("Mipmaps, collapsed").is_some());
}

#[test]
fn an_undo_does_not_open_a_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Size, false);
    set(&mut m, &[("resize", json!("half"))]);
    let mut h = harness(m, 1280.0, 800.0);
    h.state_mut().undo();
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run_steps(5);
    h.state_mut().redo();
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run_steps(5);
    assert!(
        h.query_by_label_contains("Size, collapsed").is_some(),
        "only a click, a problem or a Go to opens a group"
    );
}

#[test]
fn a_late_problem_shows_its_dot_without_opening() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, key) = (cube_model(d.path(), CUBE), CUBE.to_string()); // no problem yet
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    m.select_item(&key);
    let mut h = harness(m, 1280.0, 800.0);
    h.run(); // the latch fills here, empty
    h.state_mut().set_field(&key, "mips.levels", json!("none"));
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run_steps(5);
    let n = h.get_by_label_contains("Mipmaps, collapsed");
    assert!(n
        .accesskit_node()
        .label()
        .unwrap()
        .contains(", has a problem"));
}

#[test]
fn a_note_does_not_open_a_group_or_draw_a_dot() {
    // A pin of existing behaviour: resize = half raises DIM-RESIZED, an Info code that no `FIX_FIELDS` entry maps, so
    // this passes whatever the note filter does; `a_note_with_a_mapped_code_...` tests the filter.
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Size, false);
    // The floor keeps a small source from being halved, so lower it for the fixture's 64 px rock.
    set(
        &mut m,
        &[("resize", json!("half")), ("resize_floor", json!(8))],
    );
    assert!(
        m.item_messages(KEY)
            .iter()
            .any(|w| w.code == Code::DimResized),
        "the fixture still raises its Note"
    );
    let mut h = harness(m, 1280.0, 800.0);
    h.run();
    let label = h
        .get_by_label_contains("Size, ")
        .accesskit_node()
        .label()
        .unwrap()
        .to_string();
    assert!(
        label.contains("Size, collapsed") && !label.contains("has a problem"),
        "{label}"
    );
}

#[test]
fn two_surfaces_on_two_items_do_not_reopen_a_closed_group() {
    // R2-8: the centre draws the selected cube, the strip draws FLAT in the same frame. A latch keyed on the last
    // drawn key would flip every frame and reopen Mipmaps; one keyed on the selection does not.
    let d = tempfile::tempdir().unwrap();
    let (mut m, key) = cube_without_mips(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    m.decide(FLAT, Answer::Subtype(engine::config::Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(&key);
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(1280.0, 2400.0)) // tall: no scroll area here, so every header is on screen
        .build_ui_state(
            |ui, m: &mut AppModel| {
                ui.columns(2, |c| {
                    // Two panels of the real app sit in separate panels; here each column gets its own id scope.
                    c[0].push_id("centre", |ui| {
                        sdc_gui::ui::settings_panel::draw(ui, m, CUBE, Surface::Centre)
                    });
                    c[1].push_id("strip", |ui| {
                        sdc_gui::ui::settings_panel::draw(ui, m, FLAT, Surface::Strip)
                    });
                });
            },
            m,
        );
    h.run();
    let left = |h: &Harness<'static, AppModel>| {
        h.query_all_by_label_contains("Mipmaps, ")
            .filter(|n| n.rect().center().x < 640.0)
            .map(|n| n.accesskit_node().label().unwrap().to_string())
            .next()
            .unwrap()
    };
    assert!(
        left(&h).starts_with("Mipmaps, expanded"),
        "the problem opened the cube's group"
    );
    h.query_all_by_label_contains("Mipmaps, expanded")
        .find(|n| n.rect().center().x < 640.0)
        .unwrap()
        .click();
    h.run_steps(10);
    assert!(
        left(&h).starts_with("Mipmaps, collapsed"),
        "closed and stays closed while two items are drawn"
    );
}

#[test]
fn every_mapped_code_points_at_a_panel_field_and_is_a_problem() {
    for (code, f) in panel_groups::FIX_FIELDS {
        assert!(
            panel_groups::panel_group(f).is_some() || panel_groups::FIRST_SCREEN.contains(f),
            "{code}"
        );
        let c = Code::parse(code).unwrap();
        assert_eq!(
            sdc_gui::model::code_text::row_class(c),
            sdc_gui::model::code_text::RowClass::Problem,
            "{code}"
        );
        assert_eq!(panel_groups::fix_field(code), Some(*f), "{code}");
    }
    assert_eq!(
        panel_groups::fix_field("DIM-RESIZED"),
        None,
        "a Note maps to nothing"
    );
}

#[test]
fn the_sources_header_honours_a_forced_open() {
    // Sources is drawn by hand after the rows, and reads the same forced set as every other header.
    let d = tempfile::tempdir().unwrap();
    let m = rock(d.path());
    let mut h = harness(m, 1280.0, 800.0); // the first frame starts the latch for KEY
    h.state_mut()
        .panel_forced
        .groups
        .insert(PanelGroup::Sources);
    h.run();
    assert!(h
        .query_by_label_contains("Where these settings come from, expanded")
        .is_some());
}

/// Close the forced Mipmaps (rule 4 takes it out of the latch), so only a fresh fill can open it again.
fn closed_by_the_user(dir: &std::path::Path) -> Harness<'static, AppModel> {
    let (mut h, _) = closed_mipmaps(dir);
    h.get_by_label_contains("Mipmaps, expanded").click();
    h.run_steps(10);
    assert!(h.query_by_label_contains("Mipmaps, collapsed").is_some());
    h
}

fn decided_flat(h: &mut Harness<'static, AppModel>) {
    h.state_mut()
        .decide(FLAT, Answer::Subtype(engine::config::Subtype::Diffuse));
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
}

#[test]
fn selecting_a_texture_again_opens_its_group_again() {
    // Rule 1: another item is a new selection, so coming back to the cube fills the latch again.
    let d = tempfile::tempdir().unwrap();
    let mut h = closed_by_the_user(d.path());
    decided_flat(&mut h);
    h.state_mut().select_item(FLAT);
    h.run();
    h.state_mut().select_item(CUBE);
    h.run();
    assert!(h.query_by_label_contains("Mipmaps, expanded").is_some());
}

#[test]
fn selecting_through_nothing_starts_a_new_latch() {
    // Rule 1: the latch still names the cube after the selection went to nothing and back, and a draw may not have
    // seen the in-between. Both orders: a frame drawn in between, and none.
    for frame_between in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let mut h = closed_by_the_user(d.path());
        h.state_mut().selection = Selection::None;
        if frame_between {
            h.run();
        }
        h.state_mut().select_item(CUBE);
        h.run();
        assert!(
            h.query_by_label_contains("Mipmaps, expanded").is_some(),
            "frame between: {frame_between}"
        );
    }
}

#[test]
fn selecting_the_selected_texture_again_keeps_a_closed_group_closed() {
    // A click on the item already selected is not a new selection.
    let d = tempfile::tempdir().unwrap();
    let mut h = closed_by_the_user(d.path());
    h.state_mut().select_item(CUBE);
    h.run();
    assert!(h.query_by_label_contains("Mipmaps, collapsed").is_some());
}

fn problem_at(code: Code, level: Level) -> Problem {
    Problem {
        code: Some(code),
        level,
        ..problem_row("p")
    }
}

#[test]
fn a_group_dot_shows_the_worst_level_of_its_rows() {
    let rows = [
        problem_at(Code::ValMipError, Level::Warning),
        problem_at(Code::CubeNoMips, Level::Blocks),
        problem_at(Code::FmtBc1Alpha, Level::Warning), // Format is on the first screen: in no group
    ];
    let levels = panel_groups::problem_levels(&rows);
    assert_eq!(levels.get(&PanelGroup::Mipmaps), Some(&Level::Blocks));
    assert_eq!(levels.len(), 1, "{levels:?}");
    assert_eq!(
        panel_groups::groups_with_problems(&rows),
        [PanelGroup::Mipmaps].into()
    );
}

#[test]
fn a_row_s_also_codes_map_to_their_groups() {
    // TPC-T5-3: a merged row names more codes than its own.
    let row = Problem {
        also: vec![Code::CubeNoMips],
        ..problem_row("merged")
    };
    assert_eq!(
        panel_groups::groups_with_problems(&[row]),
        [PanelGroup::Mipmaps].into()
    );
}

#[test]
fn a_note_with_a_mapped_code_maps_to_no_group() {
    // The note filter, not the table, is under test: CUBE-NO-MIPS is mapped, the user marked the row as intended.
    let mut row = problem_at(Code::CubeNoMips, Level::Warning);
    let all = |rows: &[Problem]| -> Vec<Problem> {
        rows.iter()
            .filter(|p| panel_groups::is_panel_problem(p))
            .cloned()
            .collect()
    };
    assert_eq!(
        panel_groups::groups_with_problems(&all(&[row.clone()])),
        [PanelGroup::Mipmaps].into(),
        "premise: unmarked, it maps"
    );
    row.intended = true;
    assert!(panel_groups::groups_with_problems(&all(&[row])).is_empty());
}

// Task 6: Go to opens the group and focuses the control.

use sdc_gui::model::nav::{RevealField, REVEAL_FIELD_PASSES};

/// The Link node of the row whose texture is `key` (the key also appears as text elsewhere).
fn click_row_link(h: &mut Harness<'static, AppModel>, key: &str) {
    h.query_all_by_label(key)
        .find(|n| n.accesskit_node().role() == Role::Link)
        .unwrap()
        .click();
}

#[test]
fn go_to_opens_the_group_and_focuses_the_control() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, key) = cube_without_mips(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    m.decide(FLAT, Answer::Subtype(engine::config::Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(FLAT); // the latch fills for FLAT, not for the cube
    m.open_problems(None);
    let mut h = harness(m, 1280.0, 800.0);
    h.run();
    click_row_link(&mut h, &key);
    h.run_steps(3);
    assert!(h.query_by_label_contains("Mipmaps, expanded").is_some());
    let control = h.get_by_role_and_label(Role::ComboBox, "Levels");
    assert!(!control.accesskit_node().is_disabled());
    assert_eq!(
        control.accesskit_node().value().as_deref(),
        Some("None (base level only)")
    );
    assert!(control.is_focused());
    assert!(!h
        .get_by_role_and_label(Role::CheckBox, "Mipmaps")
        .is_focused());
    assert!(sdc_gui::ui::centre_rect(&h.ctx)
        .unwrap()
        .contains_rect(control.rect()));
    assert!(h.state().nav.reveal_field.is_none(), "taken once");
}

#[test]
fn go_to_reopens_a_group_the_user_closed_on_the_selected_texture() {
    // R3-1: the cube is selected, the latch has filled, the user closes Mipmaps, then clicks the row's link.
    let d = tempfile::tempdir().unwrap();
    let (mut m, key) = cube_without_mips(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    m.select_item(&key);
    m.open_problems(None);
    let mut h = harness(m, 1280.0, 800.0);
    h.run();
    assert!(h.state().panel_forced.filled, "the latch filled");
    h.get_by_label_contains("Mipmaps, expanded").click();
    h.run();
    assert!(
        h.query_by_label_contains("Mipmaps, collapsed").is_some(),
        "closed by the user"
    );
    click_row_link(&mut h, &key);
    h.run_steps(3);
    assert!(
        h.query_by_label_contains("Mipmaps, expanded").is_some(),
        "Go to opened it again"
    );
    assert!(h
        .get_by_role_and_label(Role::ComboBox, "Levels")
        .is_focused());
    assert_eq!(
        h.state().settings.texture_groups.get("mipmaps"),
        Some(&false),
        "a forced open is not saved"
    );
}

#[test]
fn a_code_without_a_field_maps_to_nothing() {
    assert_eq!(panel_groups::fix_field("DIM-RESIZED"), None);
    assert_eq!(
        panel_groups::fix_field("SET-BAKE-RATIO"),
        None,
        "never raised; left out of the table"
    );
    assert_eq!(panel_groups::fix_field("CUBE-NO-MIPS"), Some("mips.levels"));
}

#[test]
fn a_stale_focus_request_is_dropped() {
    // counted in passes (one kittest step can run more than one pass)
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(rock(d.path()), 1280.0, 800.0);
    h.run();
    let pass = h.ctx.cumulative_pass_nr();
    h.state_mut().nav.reveal_field = Some(RevealField {
        item: "landscape/odd.png".into(),
        field: "mips.levels",
        pass,
    });
    while h.ctx.cumulative_pass_nr() < pass + REVEAL_FIELD_PASSES {
        h.step();
    }
    assert!(
        h.state().nav.reveal_field.is_some(),
        "kept for {REVEAL_FIELD_PASSES} passes"
    );
    while h.ctx.cumulative_pass_nr() <= pass + REVEAL_FIELD_PASSES + 1 {
        h.step();
    }
    assert!(
        h.state().nav.reveal_field.is_none(),
        "dropped by draw_with_scan, though odd.png was never drawn"
    );
}

#[test]
fn a_new_selection_drops_a_waiting_reveal() {
    // JO8's rule for a waiting reveal: select_item and select_set drop it
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    m.nav.reveal_field = Some(RevealField {
        item: KEY.into(),
        field: "mips.levels",
        pass: 0,
    });
    m.select_item("landscape/leaf.png");
    assert!(m.nav.reveal_field.is_none(), "select_item drops it");
    m.nav.reveal_field = Some(RevealField {
        item: KEY.into(),
        field: "mips.levels",
        pass: 0,
    });
    let set = m
        .item(KEY)
        .and_then(|i| i.set_key.clone())
        .expect("rock01 is in a set");
    m.select_set(&set);
    assert!(m.nav.reveal_field.is_none(), "select_set drops it");
}

#[test]
fn go_to_another_texture_wins_over_the_scroll_reset() {
    // JO8: the reset to the top and the reveal's scroll land on one frame, and the reveal wins.
    const HEIGHT: f32 = 500.0;
    // The premise of the conflict: with the cube's panel at the top, Levels is below the centre's bottom edge, so only
    // the reveal's scroll can bring it in.
    {
        let d = tempfile::tempdir().unwrap();
        let (mut m, key) = cube_without_mips(d.path());
        for g in PanelGroup::ALL {
            panel_groups::set_open(&mut m.settings, g, true);
        }
        m.select_item(&key);
        m.open_problems(None);
        let h = harness(m, 1280.0, HEIGHT);
        let centre = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
        let levels = h.get_by_role_and_label(Role::ComboBox, "Levels");
        assert!(
            levels.rect().max.y > centre.max.y,
            "premise: Levels is below the centre at the top of the panel"
        );
    }
    let d = tempfile::tempdir().unwrap();
    let (mut m, key) = cube_without_mips(d.path());
    for g in PanelGroup::ALL {
        panel_groups::set_open(&mut m.settings, g, true);
    }
    m.decide(FLAT, Answer::Subtype(engine::config::Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(FLAT);
    m.open_problems(None);
    let mut h = harness(m, 1280.0, HEIGHT);
    h.run();
    let centre = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
    h.hover_at(centre.center());
    h.run();
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -2000.0),
        source: egui::MouseWheelSource::Wheel,
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
    });
    h.run_steps(10);
    // The premise: the centre scrolled down, so its first rows are above its top edge.
    let format = h.get_by_role_and_label(Role::ComboBox, "Format");
    assert!(format.rect().min.y < centre.min.y, "the centre is scrolled");
    click_row_link(&mut h, &key);
    h.run_steps(3);
    let control = h.get_by_role_and_label(Role::ComboBox, "Levels");
    assert!(control.is_focused());
    assert!(
        sdc_gui::ui::centre_rect(&h.ctx)
            .unwrap()
            .contains_rect(control.rect()),
        "the reveal's scroll won"
    );
}

#[test]
fn go_to_keeps_its_latch_through_the_next_selection_check() {
    // The write sets `seen`; without it `observe_panel_selection` takes the write for a new selection and empties it.
    let d = tempfile::tempdir().unwrap();
    let (mut m, key) = cube_without_mips(d.path());
    m.decide(FLAT, Answer::Subtype(engine::config::Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(FLAT);
    m.observe_panel_selection();
    m.select_item(&key); // another item: select_item clears the latch
    m.go_to_field(&key, "mips.levels", 0);
    m.observe_panel_selection(); // the next frame's check
    assert_eq!(m.panel_forced.item, key);
    assert!(m.panel_forced.groups.contains(&PanelGroup::Mipmaps));
    // The same item: the group is added to the latch the selection already holds.
    m.panel_forced.groups.clear();
    m.go_to_field(&key, "mips.levels", 0);
    m.observe_panel_selection();
    assert!(m.panel_forced.groups.contains(&PanelGroup::Mipmaps));
}

/// Ask the panel for `field` of the selected cube the way the Problems handler does, and let it draw.
fn go_to_in_the_panel(h: &mut Harness<'static, AppModel>, key: &str, field: &'static str) {
    let pass = h.ctx.cumulative_pass_nr();
    h.state_mut().go_to_field(key, field, pass);
    h.run_steps(3);
}

#[test]
fn go_to_the_filter_row_focuses_it_and_takes_the_request() {
    let d = tempfile::tempdir().unwrap();
    let (m, key) = cube_without_mips(d.path());
    let mut h = harness(m, 1280.0, 800.0);
    go_to_in_the_panel(&mut h, &key, "mips.filter");
    assert!(h
        .get_by_role_and_label(Role::ComboBox, "Filter")
        .is_focused());
    assert!(h.state().nav.reveal_field.is_none(), "taken once");
}

#[test]
fn go_to_a_cube_row_focuses_it_and_takes_the_request() {
    let d = tempfile::tempdir().unwrap();
    let (m, key) = cube_without_mips(d.path());
    let mut h = harness(m, 1280.0, 800.0);
    go_to_in_the_panel(&mut h, &key, "cubemap.layout");
    assert!(h
        .get_by_role_and_label(Role::ComboBox, "Cubemap layout")
        .is_focused());
    assert!(
        h.state().nav.reveal_field.is_none(),
        "taken, or it would pull the focus back for {REVEAL_FIELD_PASSES} passes"
    );
}

#[test]
fn go_to_a_field_the_panel_leaves_out_opens_the_group_and_expires() {
    // The table's stated limit (`FIX_FIELDS`): a row `hidden_reason` leaves out has no control to focus. The group
    // still opens, nothing is focused, and the request is dropped by the ageing.
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Alpha, false);
    let mut h = harness(m, 1280.0, 800.0);
    go_to_in_the_panel(&mut h, KEY, "alpha.parallax"); // Enable parallax: complex material only
    assert!(
        h.query_by_label_contains("Alpha, expanded").is_some(),
        "the group opened"
    );
    assert!(
        h.state().nav.reveal_field.is_some(),
        "premise: no row drew the field, so none took it"
    );
    let pass = h.ctx.cumulative_pass_nr();
    while h.ctx.cumulative_pass_nr() <= pass + REVEAL_FIELD_PASSES + 1 {
        h.step();
    }
    assert!(h.state().nav.reveal_field.is_none(), "expired");
}

#[test]
fn the_latch_fill_after_go_to_adds_to_its_group() {
    // The fill (`filled: false` after a Go to on another item) unions the problems' groups with the group Go to wrote.
    // The rock has no problem on Size, so only Go to can keep it open.
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Size, false);
    m.select_item("landscape/leaf.png");
    m.select_item(KEY);
    m.go_to_field(KEY, "resize", 0);
    assert!(!m.panel_forced.filled, "premise: the fill is still to come");
    let mut h = harness(m, 1280.0, 800.0);
    h.run_steps(3);
    assert!(h.state().panel_forced.filled, "the fill ran");
    assert!(
        h.query_by_label_contains("Size, expanded").is_some(),
        "Go to's group survived the fill"
    );
}

#[test]
fn go_to_a_first_screen_field_opens_nothing() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, key) = cube_without_mips(d.path());
    m.observe_panel_selection();
    m.go_to_field(&key, "format", 0);
    assert!(m.panel_forced.groups.is_empty());
    assert!(m.nav.reveal_field.is_some(), "the focus is still asked for");
}

// Task 7: the bulk panel uses the same groups.

const ROCK: &str = "landscape/rock01.png";
const LEAF: &str = "landscape/leaf.png";

/// `bulk_ui.rs::harness`, copied under a name of its own: Task 4's `harness(m, w, h)` lives in this module.
fn bulk_harness(m: AppModel) -> Harness<'static, AppModel> {
    harness(m, 1280.0, 800.0)
}

/// `bulk_apply.rs::many`, copied: `n` small colour textures under `textures/many`, each its own set.
fn many(dir: &std::path::Path, n: usize) -> std::path::PathBuf {
    let r = dir.join("textures/many");
    std::fs::create_dir_all(&r).unwrap();
    for i in 0..n {
        image::ImageBuffer::from_fn(8, 8, |x, y| {
            image::Rgb([(x * 30) as u8, (y * 30) as u8, i as u8])
        })
        .save(r.join(format!("t{i:03}.png")))
        .unwrap();
    }
    dir.join("textures")
}

fn two_alike(dir: &std::path::Path) -> AppModel {
    let r = dir.join("textures/a");
    std::fs::create_dir_all(&r).unwrap();
    for n in ["one.png", "two.png"] {
        image::RgbImage::from_fn(64, 64, |x, y| image::Rgb([x as u8, y as u8, 90]))
            .save(r.join(n))
            .unwrap();
    }
    let mut m = support::opened_as_the_app(dir.join("textures"), dir.join("export"));
    for k in ["a/one.png", "a/two.png"] {
        m.decide(k, Answer::Subtype(engine::config::Subtype::Diffuse));
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

#[test]
fn two_selected_alike_read_like_the_single_panel() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_alike(d.path());
    // TPC-R1: only a closed header carries a summary, so close the three groups this test compares.
    for g in [
        PanelGroup::Alpha,
        PanelGroup::Mipmaps,
        PanelGroup::Compression,
    ] {
        panel_groups::set_open(&mut m.settings, g, false);
    }
    m.select_item("a/one.png");
    let mut h = bulk_harness(m);
    h.run();
    let single: Vec<String> = ["Alpha, ", "Mipmaps, ", "Compression, "]
        .iter()
        .map(|t| {
            h.get_by_label_contains(t)
                .accesskit_node()
                .label()
                .unwrap()
                .to_string()
        })
        .collect();
    h.state_mut().click_item("a/two.png", true, false);
    h.run();
    assert!(
        single
            .iter()
            .all(|s| s.contains(", collapsed") && s.contains(": ")),
        "premise: closed, with summaries: {single:?}"
    );
    for (t, s) in ["Alpha, ", "Mipmaps, ", "Compression, "]
        .iter()
        .zip(&single)
    {
        assert_eq!(
            &h.get_by_label_contains(t)
                .accesskit_node()
                .label()
                .unwrap()
                .to_string(),
            s
        );
    }
}

#[test]
fn four_selected_with_two_filters_say_mixed_while_closed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_as_the_app(many(d.path(), 4), d.path().join("export"));
    // Two different filters, neither the default, so the four textures are mixed whatever the default is.
    m.set_field("many/t000.png", "mips.filter", json!("kaiser"));
    m.set_field("many/t001.png", "mips.filter", json!("tent"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    m.select_all_visible();
    let mut h = bulk_harness(m);
    h.run();
    let label = h
        .get_by_label_contains("Mipmaps, ")
        .accesskit_node()
        .label()
        .unwrap()
        .to_string();
    assert!(
        label.starts_with("Mipmaps, collapsed, changed: "),
        "{label}"
    );
    assert!(label.contains("Filter: mixed"), "{label}");
    // TPC-R1: open, the rows show the values; the header keeps "changed" and drops the summary.
    h.get_by_label_contains("Mipmaps, ").click();
    h.run();
    assert!(h.query_by_label("Mipmaps, expanded, changed").is_some());
}

#[test]
fn many_selected_build_the_summaries_once() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened_as_the_app(many(d.path(), 300), d.path().join("export"));
    m.select_all_visible();
    let mut h = bulk_harness(m);
    h.run();
    let n = panel_groups::bulk_summary_builds();
    assert!(n >= 1, "the bulk view built its summaries");
    h.get_by_label_contains("Size, ").hover();
    h.run();
    h.get_by_label_contains("Alpha, ").click();
    h.run();
    h.run_steps(3);
    assert_eq!(
        panel_groups::bulk_summary_builds(),
        n,
        "no bulk summary is built per frame, per hover or per toggle"
    );
}

#[test]
fn cube_and_channels_never_appear_in_bulk() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let mut h = bulk_harness(m);
    h.run();
    for t in [
        "Cubemap, ",
        "Channels, ",
        "Where these settings come from, ",
        "Used by ",
    ] {
        assert!(h.query_by_label_contains(t).is_none(), "{t}");
    }
}

#[test]
fn a_group_with_no_applying_field_reads_not_used_for_these_textures_while_closed() {
    // two colour textures: every Normal map row has applies == 0
    let d = tempfile::tempdir().unwrap();
    let mut m = two_alike(d.path());
    m.select_item("a/one.png");
    m.click_item("a/two.png", true, false);
    panel_groups::set_open(&mut m.settings, PanelGroup::NormalMap, false);
    let mut h = bulk_harness(m);
    h.run();
    let n = h.get_by_label_contains("Normal map, collapsed");
    assert!(n
        .accesskit_node()
        .label()
        .unwrap()
        .contains("not used for these textures"));
    assert!(
        !n.accesskit_node().is_disabled(),
        "a header is never disabled"
    );
    n.click();
    h.run();
    assert!(
        h.query_by_label("Normal map, expanded").is_some(),
        "open, with no summary (TPC-R1)"
    );
}

#[test]
fn a_read_only_sdc_toml_keeps_the_bulk_headers_working() {
    // R1-1 and LM11: a read-only sdc.toml greys every bulk row, never a header
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    panel_groups::set_open(&mut m.settings, PanelGroup::Size, false);
    m.project
        .as_mut()
        .unwrap()
        .set_error("rejected by the engine");
    assert!(m.toml_read_only_reason().is_some(), "premise: read-only");
    let mut h = bulk_harness(m);
    h.run();
    let head = h.get_by_label_contains("Size, collapsed");
    assert!(
        !head.accesskit_node().is_disabled(),
        "the header is not greyed"
    );
    head.click();
    h.run();
    assert!(
        h.query_by_label_contains("Size, expanded").is_some(),
        "it still opens"
    );
    assert!(
        h.get_by_role_and_label(Role::ComboBox, "Resize")
            .accesskit_node()
            .is_disabled(),
        "the rows are greyed"
    );
    // LM11 "greys every bulk row": the first-screen rows outside the groups stay greyed too
    assert!(
        h.get_by_role_and_label(Role::ComboBox, "Format")
            .accesskit_node()
            .is_disabled(),
        "the first-screen Format row is greyed"
    );
}

#[test]
fn the_bulk_and_single_panels_share_the_open_state() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    panel_groups::set_open(&mut m.settings, PanelGroup::Size, true);
    let mut h = bulk_harness(m);
    h.run();
    h.get_by_label_contains("Size, expanded").click();
    h.run();
    h.state_mut().select_item(ROCK);
    h.run();
    assert!(h.query_by_label_contains("Size, collapsed").is_some());
}

// Task 7 fix round 1.

fn bulk_group(m: &mut AppModel, g: PanelGroup) -> panel_groups::GroupSummary {
    let view = m.bulk_view();
    view.groups
        .iter()
        .find(|(x, _)| *x == g)
        .map(|(_, s)| s.clone())
        .unwrap_or_else(|| panic!("no {g:?} group in the bulk view"))
}

/// TPC-T7-1: one selection reads the same whichever texture was clicked last.
#[test]
fn the_bulk_size_summary_does_not_depend_on_the_active_texture() {
    let d = tempfile::tempdir().unwrap();
    let mut m = cube_model(d.path(), CUBE);
    m.decide(
        "a/flat.png",
        Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_field("a/flat.png", "resize", json!("half"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(CUBE);
    m.click_item("a/flat.png", true, false); // the flat texture is active
    let flat_active = bulk_group(&mut m, PanelGroup::Size).text;
    m.select_item("a/flat.png");
    m.click_item(CUBE, true, false); // the cube map is active
    let cube_active = bulk_group(&mut m, PanelGroup::Size).text;
    assert_eq!(flat_active, cube_active);
    assert!(!cube_active.contains("no effect"), "{cube_active}");
}

fn bulk_row(
    field: &'static str,
    label: &'static str,
    why: &[(&str, usize)],
) -> sdc_gui::model::bulk::BulkField {
    sdc_gui::model::bulk::BulkField {
        field,
        label,
        value: None,
        applies: 0,
        edited: 0,
        skipped: why.iter().map(|(r, n)| (r.to_string(), *n)).collect(),
        total: 2,
        formats_differ: false,
        option_reach: vec![],
    }
}

#[test]
fn a_bulk_no_effect_reason_shows_when_the_caller_says_every_texture_shares_it() {
    let reg = engine::presets::PresetRegistry::builtin().unwrap();
    let p = reg.base_profile(Target::Cs, engine::config::Subtype::Diffuse);
    let mut resize = bulk_row("resize", "Resize", &[]);
    resize.applies = 2;
    resize.value = Some(json!("half"));
    let rows = [resize];
    let shared = panel_groups::bulk_summary(
        PanelGroup::Size,
        &rows,
        Target::Cs,
        &p,
        Some("no effect on a cubemap: Face size sizes it"),
    );
    assert_eq!(shared.text, "no effect on a cubemap: Face size sizes it");
    let none = panel_groups::bulk_summary(PanelGroup::Size, &rows, Target::Cs, &p, None);
    assert_eq!(none.text, "Half");
}

#[test]
fn the_bulk_all_locked_hover_gives_the_shared_reason_or_points_at_the_rows() {
    let reg = engine::presets::PresetRegistry::builtin().unwrap();
    let p = reg.base_profile(Target::Cs, engine::config::Subtype::Diffuse);
    let same = [
        bulk_row("mips.normal", "Normal handling", &[("normal maps only", 2)]),
        bulk_row(
            "mips.normal_base",
            "Normal base level",
            &[("normal maps only", 2)],
        ),
    ];
    let s = panel_groups::bulk_summary(PanelGroup::NormalMap, &same, Target::Cs, &p, None);
    assert_eq!(s.text, "not used for these textures");
    assert!(s.all_locked);
    assert_eq!(
        s.hover.as_deref(),
        Some("Every row here: normal maps only.")
    );
    let differ = [
        bulk_row("mips.normal", "Normal handling", &[("normal maps only", 2)]),
        bulk_row(
            "mips.normal_base",
            "Normal base level",
            &[("another reason", 1), ("normal maps only", 1)],
        ),
    ];
    let s = panel_groups::bulk_summary(PanelGroup::NormalMap, &differ, Target::Cs, &p, None);
    assert_eq!(
        s.hover.as_deref(),
        Some("Each row below says why it is not used.")
    );
}

/// Review item 4: no bulk row is dropped. Every row lands on the first screen or in a group the panel draws, and the
/// groups come in panel order.
#[test]
fn every_bulk_row_is_drawn_on_the_first_screen_or_in_a_bulk_group() {
    use sdc_gui::model::bulk::BULK_GROUPS;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let view = m.bulk_view();
    let mut fields: Vec<&str> = view.rows.iter().map(|r| r.field).collect();
    fields.extend(sdc_gui::model::bulk::panel_fields().iter().map(|f| f.0));
    fields.extend(sdc_gui::model::fields::SEARCH_FIELDS.iter().map(|f| f.0));
    fields.push(sdc_gui::model::fields::BC3_SEARCH);
    for f in fields {
        let ok = panel_groups::FIRST_SCREEN.contains(&f)
            || panel_groups::panel_group(f).is_some_and(|g| BULK_GROUPS.contains(&g));
        assert!(ok, "{f} would not be drawn in the bulk panel");
    }
    let drawn: Vec<PanelGroup> = view.groups.iter().map(|(g, _)| *g).collect();
    let in_panel_order: Vec<PanelGroup> = PanelGroup::ALL
        .into_iter()
        .filter(|g| drawn.contains(g))
        .collect();
    assert_eq!(drawn, in_panel_order);
    assert_eq!(drawn, BULK_GROUPS);
}

/// Review item 6: the bulk cache is keyed on the active texture, the target and the items' jobs.
#[test]
fn the_bulk_view_is_rebuilt_when_the_active_texture_the_target_or_a_job_changes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    m.bulk_view();
    let n = m.bulk_computes();
    m.bulk_view();
    assert_eq!(m.bulk_computes(), n, "premise: equal inputs reuse the view");
    m.selection = sdc_gui::model::Selection::Item(ROCK.into());
    m.bulk_view();
    assert_eq!(m.bulk_computes(), n + 1, "a new active texture");
    m.target = if m.target == Target::Cs {
        Target::Vanilla
    } else {
        Target::Cs
    };
    m.bulk_view();
    assert_eq!(m.bulk_computes(), n + 2, "a new target");
    m.set_field(LEAF, "resize", json!("half"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.bulk_view();
    assert!(m.bulk_computes() > n + 2, "a changed job");
}

// Task 7a (A3): the bulk Green row.

/// Two tangent normals, one model-space normal and a colour texture in `textures/g`, all decided. `sdc_toml` is
/// written to the project root before the folder opens; `bump` adds a lone `g/d_bump.png` that waits for its type.
fn normals_in(dir: &std::path::Path, sdc_toml: Option<&str>, bump: bool) -> AppModel {
    let r = dir.join("textures/g");
    std::fs::create_dir_all(&r).unwrap();
    let flat = |_, _| image::Rgb([128u8, 128, 255]);
    for n in ["a_n.png", "b_n.png", "c_msn.png"] {
        image::RgbImage::from_fn(64, 64, flat)
            .save(r.join(n))
            .unwrap();
    }
    if bump {
        image::RgbImage::from_fn(64, 64, flat)
            .save(r.join("d_bump.png"))
            .unwrap();
    }
    image::RgbImage::from_fn(64, 64, |x, y| image::Rgb([x as u8, y as u8, 90]))
        .save(r.join("plain.png"))
        .unwrap();
    if let Some(t) = sdc_toml {
        std::fs::write(dir.join("textures/sdc.toml"), t).unwrap();
    }
    let mut m = support::opened_as_the_app(dir.join("textures"), dir.join("export"));
    m.decide("g/a_n.png", Answer::Subtype(Subtype::Normal));
    m.decide("g/b_n.png", Answer::Subtype(Subtype::Normal));
    m.decide("g/c_msn.png", Answer::Subtype(Subtype::Msn));
    m.decide("g/plain.png", Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}
fn normals_and_others(dir: &std::path::Path) -> AppModel {
    normals_in(dir, None, false)
}
const GREEN_KEYS: [&str; 4] = ["g/a_n.png", "g/b_n.png", "g/c_msn.png", "g/plain.png"];
fn green_keys() -> Vec<String> {
    GREEN_KEYS.iter().map(|k| k.to_string()).collect()
}
fn green_of(m: &AppModel, k: &str) -> Option<serde_json::Value> {
    m.project
        .as_ref()
        .unwrap()
        .file_value(k, "channels.green_convention")
}
/// The planned convention of `k`, as the engine reads it (unset is DirectX).
fn green_read(m: &AppModel, k: &str) -> GreenConvention {
    let job = m.item(k).and_then(|i| i.job.as_ref()).expect("planned");
    job.profile
        .channels
        .green_convention
        .unwrap_or(GreenConvention::Directx)
}
/// Select `keys` (two or more, so the bulk panel is in effect) and return them as the click passes them: the model
/// re-reads the selection at the click (TPC-R2), so a model-level call selects first, as the panel's click does.
fn select(m: &mut AppModel, keys: &[&str]) -> Vec<String> {
    m.select_item(keys[0]);
    for k in &keys[1..] {
        m.click_item(k, true, false);
    }
    let ks: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    let (mut got, mut want) = (m.bulk_targets(), ks.clone());
    got.sort();
    want.sort();
    assert_eq!(got, want, "premise: the bulk targets are these keys");
    ks
}
/// `keys` selected, every group closed (so the panel looks the same whatever the group state). `tall`: the 1280 x 1600
/// harness; otherwise `bulk_harness`. Both only draw: `ui::draw` never polls the planner, so a plan lands only at
/// `wait_idle`, and a plan left pending by `set_field` stays pending.
fn green_panel(mut m: AppModel, keys: &[&str], tall: bool) -> Harness<'static, AppModel> {
    select(&mut m, keys);
    for g in PanelGroup::ALL {
        panel_groups::set_open(&mut m.settings, g, false);
    }
    if tall {
        return harness(m, 1280.0, 1600.0);
    }
    bulk_harness(m)
}
/// The Green row's own "applies to ..." note: the one on or just below the "Green: ..." label, not another row's.
fn green_reach_note(h: &Harness<'_, AppModel>, green: &str, note: &str) -> egui::Rect {
    let g = h.get_by_label(green).rect();
    h.query_all_by_label(note)
        .map(|n| n.rect())
        .filter(|r| r.top() >= g.top() - 2.0 && r.top() <= g.bottom() + 24.0)
        .min_by(|a, b| {
            (a.top() - g.top())
                .abs()
                .total_cmp(&(b.top() - g.top()).abs())
        })
        .unwrap_or_else(|| panic!("{note} on the {green} row"))
}

#[test]
fn green_reaches_the_normals_that_take_it_in_one_step() {
    use sdc_gui::model::bulk::{green_reach, SKIP_MSN, SKIP_NOT_NORMAL};
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_and_others(d.path());
    let r = green_reach(&m, &green_keys());
    assert_eq!(r.takes, ["g/a_n.png", "g/b_n.png"]);
    assert_eq!(
        r.skipped,
        [(SKIP_MSN.to_string(), 1), (SKIP_NOT_NORMAL.to_string(), 1)]
    );
    assert_eq!(
        r.value,
        Some(GreenConvention::Directx),
        "unset reads DirectX"
    );
    // TPC-R2: the click's keys are still selected.
    let keys = select(&mut m, &GREEN_KEYS);
    // A bulk Format write next, after `select` (`select_item` resets `self.bulk`, and `apply_bulk_edit` keeps the
    // selection): the Green write must replace its result line (the panel's one line).
    m.apply_bulk_edit(
        &keys,
        "format",
        sdc_gui::model::bulk::BulkOp::Set(json!("bc7")),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        m.bulk
            .report
            .as_deref()
            .is_some_and(|r| r.starts_with("Format: ")),
        "premise: {:?}",
        m.bulk.report
    );
    let steps = m.history.undo.len();
    assert_eq!(m.decide_many_green(&keys, GreenConvention::Opengl), Ok(2));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.history.undo.len(), steps + 1, "one undo step");
    assert_eq!(m.history.undo.last().unwrap().label, "Green for 2 textures");
    for k in ["g/a_n.png", "g/b_n.png"] {
        assert_eq!(green_of(&m, k), Some(json!("opengl")), "{k}");
    }
    for k in ["g/c_msn.png", "g/plain.png"] {
        assert_eq!(green_of(&m, k), None, "{k} is skipped");
    }
    assert_eq!(
        m.bulk.report.as_deref(),
        Some("Green: OpenGL written to 2 of 4"),
        "the Format line is replaced"
    );
    m.undo();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(green_of(&m, "g/a_n.png"), None, "Undo takes both back");
    assert_eq!(green_of(&m, "g/b_n.png"), None);
    // One texture reads "1 texture". Bulk needs two selected, so one normal and the colour texture.
    let one = select(&mut m, &["g/a_n.png", "g/plain.png"]);
    assert_eq!(m.decide_many_green(&one, GreenConvention::Opengl), Ok(1));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.history.undo.last().unwrap().label, "Green for 1 texture");
    assert_eq!(
        m.bulk.report.as_deref(),
        Some("Green: OpenGL written to 1 of 2")
    );
    // A second click on the same button changes nothing: no step and no report, as `apply_bulk_edit` does.
    let two = select(&mut m, &["g/a_n.png", "g/b_n.png"]);
    assert_eq!(m.decide_many_green(&two, GreenConvention::Opengl), Ok(2)); // writes b_n; a_n is already OpenGL
    assert!(m.wait_idle(Duration::from_secs(60)));
    let steps = m.history.undo.len();
    m.bulk.report = None;
    assert_eq!(m.decide_many_green(&two, GreenConvention::Opengl), Ok(2));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.history.undo.len(), steps, "same value: no step");
    assert_eq!(m.bulk.report, None, "same value: nothing was written");
}

#[test]
fn a_selection_with_no_normal_has_no_green_row_and_writes_nothing() {
    use sdc_gui::model::bulk::GREEN_NONE;
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_and_others(d.path());
    let keys = select(&mut m, &["g/c_msn.png", "g/plain.png"]);
    let steps = m.history.undo.len();
    assert_eq!(m.decide_many_green(&keys, GreenConvention::Opengl), Ok(0));
    assert_eq!(m.bulk.report.as_deref(), Some(GREEN_NONE));
    assert_eq!(m.history.undo.len(), steps);
    let mut h = green_panel(m, &["g/c_msn.png", "g/plain.png"], false);
    h.run();
    assert!(h.query_by_label("Treat as OpenGL (flip green)").is_none());
    assert!(h.query_by_label_contains("Green: ").is_none());
}

#[test]
fn the_green_row_reads_mixed_and_its_buttons_write_both_ways() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_and_others(d.path());
    m.decide("g/a_n.png", Answer::Green(GreenConvention::Opengl));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let mut h = green_panel(m, &GREEN_KEYS, false);
    h.run();
    let note = green_reach_note(&h, "Green: mixed", "applies to 2 of 4");
    h.hover_at(note.center());
    h.run();
    h.get_by_label_contains("locked on 1 of 4: model-space normal (_msn): no green to flip");
    h.get_by_label("Treat as DirectX").click();
    h.run();
    let m = h.state_mut();
    assert!(m.wait_idle(Duration::from_secs(60)));
    for k in ["g/a_n.png", "g/b_n.png"] {
        assert_eq!(green_of(m, k), Some(json!("directx")), "{k}");
    }
    h.run();
    h.get_by_label("Green: DirectX");
}

/// A3-2: a `[subtypes.normal]` line that says OpenGL is overridden by the bulk DirectX, which a Clear would not do.
#[test]
fn a_layer_opengl_reads_directx_after_the_bulk_click() {
    let d = tempfile::tempdir().unwrap();
    let m = normals_in(
        d.path(),
        Some("[subtypes.normal]\nchannels = { green_convention = \"opengl\" }\n"),
        false,
    );
    for k in ["g/a_n.png", "g/b_n.png"] {
        assert_eq!(
            green_read(&m, k),
            GreenConvention::Opengl,
            "{k}: the layer reads OpenGL"
        );
        assert_eq!(green_of(&m, k), None, "{k}");
    }
    let mut h = green_panel(m, &GREEN_KEYS, false);
    h.run();
    h.get_by_label("Green: OpenGL");
    h.get_by_label("Treat as DirectX").click();
    h.run();
    let m = h.state_mut();
    assert!(m.wait_idle(Duration::from_secs(60)));
    for k in ["g/a_n.png", "g/b_n.png"] {
        assert_eq!(green_of(m, k), Some(json!("directx")), "{k}");
        assert_eq!(
            green_read(m, k),
            GreenConvention::Directx,
            "{k} reads DirectX now"
        );
    }
    h.run();
    h.get_by_label("Green: DirectX");
}

/// A3-3: a texture still waiting for its type gets no green, with its own reason.
#[test]
fn a_texture_waiting_for_its_type_is_skipped() {
    use sdc_gui::model::bulk::{green_reach, SKIP_UNDECIDED};
    const BUMP: &str = "g/d_bump.png";
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_in(d.path(), None, true);
    assert!(
        m.item(BUMP).expect("listed").waiting,
        "premise: {BUMP} waits for a type"
    );
    let keys: Vec<String> = ["g/a_n.png", BUMP].iter().map(|k| k.to_string()).collect();
    let r = green_reach(&m, &keys);
    assert_eq!(r.takes, ["g/a_n.png"]);
    assert_eq!(r.skipped, [(SKIP_UNDECIDED.to_string(), 1)]);
    let keys = select(&mut m, &["g/a_n.png", BUMP]);
    assert_eq!(m.decide_many_green(&keys, GreenConvention::Opengl), Ok(1));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(
        green_of(&m, BUMP),
        None,
        "the waiting texture is not written"
    );
    assert_eq!(
        m.bulk.report.as_deref(),
        Some("Green: OpenGL written to 1 of 2")
    );
    let mut h = green_panel(m, &["g/a_n.png", BUMP], false);
    h.run();
    let note = green_reach_note(&h, "Green: OpenGL", "applies to 1 of 2");
    h.hover_at(note.center());
    h.run();
    h.get_by_label_contains("locked on 1 of 2: type not decided yet");
}

/// FK3 and TPC-R2: while a plan is pending both buttons are off with the wait reason, and the model refuses a write
/// that lands anyway (as `decide_many` returns on `fix_off()`).
#[test]
fn the_green_buttons_wait_for_the_plan_and_the_model_refuses() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_and_others(d.path());
    m.set_field("g/plain.png", "resize", serde_json::json!("half"));
    assert_eq!(
        m.fix_off().as_deref(),
        Some("Wait for the plan to finish"),
        "premise: the edit left a plan in flight"
    );
    let keys = select(&mut m, &GREEN_KEYS);
    let steps = m.history.undo.len();
    m.bulk.report = None;
    assert_eq!(
        m.decide_many_green(&keys, GreenConvention::Opengl),
        Err("Wait for the plan to finish".to_string())
    );
    assert_eq!(
        m.history.undo.len(),
        steps,
        "nothing written while the plan is pending"
    );
    assert_eq!(m.bulk.report, None, "no report");
    assert!(
        m.fix_off().is_some(),
        "still pending: the refusal is what the test saw"
    );
    let mut h = green_panel(m, &GREEN_KEYS, true);
    h.run_steps(3);
    for b in ["Treat as OpenGL (flip green)", "Treat as DirectX"] {
        assert!(
            h.get_by_label(b).accesskit_node().is_disabled(),
            "{b} is off"
        );
    }
    let r = h.get_by_label("Treat as DirectX").rect();
    h.hover_at(r.center());
    h.run();
    h.get_by_label("Wait for the plan to finish");
    for k in ["g/a_n.png", "g/b_n.png"] {
        assert_eq!(green_of(h.state(), k), None, "{k}");
    }
}

/// A read-only `sdc.toml`: the buttons are off with its reason; the model refuses and writes nothing.
#[test]
fn a_read_only_sdc_toml_takes_no_green() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_and_others(d.path());
    m.project
        .as_mut()
        .unwrap()
        .set_error("rejected by the engine");
    let why = m.toml_read_only_reason().expect("read-only");
    assert_eq!(m.fix_off().as_deref(), Some(why.as_str()));
    let keys = select(&mut m, &GREEN_KEYS);
    let steps = m.history.undo.len();
    assert_eq!(
        m.decide_many_green(&keys, GreenConvention::Opengl),
        Err(why.clone())
    );
    assert_eq!(m.history.undo.len(), steps, "nothing written");
    assert_eq!(
        m.bulk.report, None,
        "no report, as apply_bulk_edit on a read-only sdc.toml"
    );
    let mut h = green_panel(m, &GREEN_KEYS, false);
    h.run();
    for b in ["Treat as OpenGL (flip green)", "Treat as DirectX"] {
        assert!(
            h.get_by_label(b).accesskit_node().is_disabled(),
            "{b} is off"
        );
    }
    // The panel already shows the reason as a line, so the hover is one more node.
    let shown = h.query_all_by_label(&why).count();
    let r = h.get_by_label("Treat as OpenGL (flip green)").rect();
    h.hover_at(r.center());
    h.run();
    assert!(
        h.query_all_by_label(&why).count() > shown,
        "the button's hover gives the reason"
    );
}

/// TPC-R2: the click re-reads the selection, as the type card's Apply does.
#[test]
fn a_stale_green_click_writes_only_to_what_is_still_selected() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_and_others(d.path());
    let keys = select(&mut m, &GREEN_KEYS);
    // The selection moved on to one texture before the click landed: no bulk selection is left.
    m.click_item("g/a_n.png", false, false);
    let steps = m.history.undo.len();
    m.bulk.report = None;
    assert_eq!(m.decide_many_green(&keys, GreenConvention::Opengl), Ok(0));
    assert_eq!(m.history.undo.len(), steps, "nothing written");
    assert_eq!(
        m.bulk.report, None,
        "no report, as decide_many with nothing eligible"
    );
    for k in ["g/a_n.png", "g/b_n.png"] {
        assert_eq!(green_of(&m, k), None, "{k}");
    }
    // The selection shrank to a_n and plain: only a_n is written, and the counts are of what is still selected.
    select(&mut m, &["g/a_n.png", "g/plain.png"]);
    assert_eq!(m.decide_many_green(&keys, GreenConvention::Opengl), Ok(1));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(green_of(&m, "g/a_n.png"), Some(json!("opengl")));
    assert_eq!(
        green_of(&m, "g/b_n.png"),
        None,
        "no longer selected: not written"
    );
    assert_eq!(
        m.bulk.report.as_deref(),
        Some("Green: OpenGL written to 1 of 2")
    );
}

/// Pin of existing behaviour (Z5): Green is no field, so Copy settings never carries it.
#[test]
fn green_stays_out_of_the_copied_fields() {
    assert!(!sdc_gui::model::bulk::panel_fields()
        .iter()
        .any(|(f, _)| *f == "channels.green_convention"));
    assert!(!sdc_gui::model::fields::FIELDS
        .iter()
        .any(|(f, _)| *f == "channels.green_convention"));
}

/// CODING_STANDARDS, "The fast re-plan keeps what the full plan keeps": the row reads the new convention after the
/// fast re-plan (`apply_partial`) alone, with the full plan still out, and after the full plan.
#[test]
fn the_green_row_follows_the_fast_plan_and_the_full_plan_each_alone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_and_others(d.path());
    let keys = select(&mut m, &["g/a_n.png", "g/b_n.png"]);
    for g in PanelGroup::ALL {
        panel_groups::set_open(&mut m.settings, g, false);
    }
    m.hold_plan_results = true;
    let mut h = harness(m, 1280.0, 1600.0);
    h.get_by_label("Green: DirectX");
    assert_eq!(
        h.state_mut()
            .decide_many_green(&keys, GreenConvention::Opengl),
        Ok(2)
    );
    h.run();
    h.get_by_label("Green: DirectX"); // the panel shows the last plan until one lands
    let mut full = vec![];
    land_partial(&mut h, &mut full);
    h.run();
    h.get_by_label("Green: OpenGL"); // the fast plan alone
    let m = h.state_mut();
    m.hold_plan_results = false;
    for x in full {
        m.sender().send(x).unwrap();
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
    h.run();
    h.get_by_label("Green: OpenGL"); // and after the full plan
}

/// A read-only `sdc.toml` greys the rows, but the reach notes keep their hover: the Green row's and another row's.
#[test]
fn a_read_only_sdc_toml_keeps_the_notes_hover() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_and_others(d.path());
    m.project
        .as_mut()
        .unwrap()
        .set_error("rejected by the engine");
    let mut h = green_panel(m, &GREEN_KEYS, true);
    h.run();
    let note = green_reach_note(&h, "Green: DirectX", "applies to 2 of 4");
    h.hover_at(note.center());
    h.run();
    h.get_by_label_contains("locked on 1 of 4: model-space normal (_msn): no green to flip");
    // A grouped row: open every group, then hover the note of a row that has a skip reason.
    let m = h.state_mut();
    for g in PanelGroup::ALL {
        panel_groups::set_open(&mut m.settings, g, true);
    }
    let view = m.bulk_view();
    // A note no other row shares, so the label names one row.
    let notes: Vec<String> = view.rows.iter().map(|r| r.note()).collect();
    let row = view
        .rows
        .iter()
        .find(|r| {
            !r.skipped.is_empty()
                && !r.note().is_empty()
                && notes.iter().filter(|n| **n == r.note()).count() == 1
        })
        .expect("a row with a skip reason and a note of its own");
    let (note, first) = (
        row.note(),
        row.skipped_text().lines().next().unwrap().to_string(),
    );
    h.run();
    let r = h.get_by_label(&note).rect();
    h.hover_at(r.center());
    h.run();
    h.get_by_label_contains(&first);
}

// The groups start closed, and the first screen fits.

/// `support::opened` leaves rock01 undecided until `decide`; select it as is (64x64 RGB).
fn undecided_rock(dir: &std::path::Path) -> AppModel {
    let mut m = support::opened(dir);
    m.select_item(KEY);
    m
}

/// R3-2: the layout tests below set every group closed themselves; only `every_group_starts_closed` reads the default.
fn all_closed(mut m: AppModel) -> AppModel {
    for g in PanelGroup::ALL {
        panel_groups::set_open(&mut m.settings, g, false);
    }
    m
}

#[test]
fn a_closed_panel_has_at_most_twelve_control_kinds() {
    for (name, make) in [
        ("decided colour", rock as fn(&std::path::Path) -> AppModel),
        ("undecided type", undecided_rock),
    ] {
        let d = tempfile::tempdir().unwrap();
        let h = harness(all_closed(make(d.path())), 1280.0, 800.0);
        let kinds = support::control_kinds(&h, sdc_gui::ui::centre_rect(&h.ctx).unwrap());
        eprintln!("{name}: {} control kinds: {kinds:?}", kinds.len());
        assert!(
            kinds.contains(&("ComboBox".to_string(), "Format".to_string())),
            "the helper sees the first screen: {name}: {kinds:#?}"
        );
        assert!(kinds.len() <= 12, "{name}: {kinds:#?}");
    }
}

/// The E22 report: `support::control_kinds` of seven cases, groups closed and then open. It asserts nothing.
#[test]
#[ignore = "a report: run by hand with --ignored --nocapture"]
fn e22_control_kinds_report() {
    type Make = fn(&std::path::Path) -> AppModel;
    let cases: [(&str, Make); 7] = [
        ("decided colour", rock),
        ("tangent normal map", |d| {
            let mut m = support::opened(d);
            let key = "landscape/rock01_normal.png";
            m.decide(key, Answer::Subtype(Subtype::Normal));
            assert!(m.wait_idle(Duration::from_secs(60)));
            m.select_item(key);
            m
        }),
        ("packed RMAOS", |d| {
            let mut m = support::no_albedo_pair(d);
            m.decide("arch/wall_rmaos.png", Answer::Subtype(Subtype::Rmaos));
            assert!(m.wait_idle(Duration::from_secs(60)));
            m.select_item("arch/wall_rmaos.png");
            m
        }),
        ("cube", |d| cube_model(d, CUBE)),
        ("undecided type", undecided_rock),
        // The engine asks the alpha meaning only once a type is chosen (bottom_panel.rs: "The alpha question only
        // follows a sub-type"), so type and alpha are never both open: this is the alpha question on a decided type.
        ("alpha question open (leaf decided Diffuse)", |d| {
            let mut m = support::opened(d);
            m.decide(LEAF, Answer::Subtype(Subtype::Diffuse));
            assert!(m.wait_idle(Duration::from_secs(60)));
            assert!(
                sdc_gui::model::items::alpha_asked(m.item(LEAF).unwrap()).is_some(),
                "premise: leaf's alpha question is open"
            );
            m.select_item(LEAF);
            m
        }),
        ("bulk of 4", |d| {
            let mut m = support::opened_as_the_app(many(d, 4), d.join("export"));
            m.select_all_visible();
            m
        }),
    ];
    for (name, make) in cases {
        // Open: tall windows, so no row scrolls out of the centre; the same count at two heights shows it is exact.
        for (open, height) in [(false, 800.0), (true, 2400.0), (true, 4800.0)] {
            let d = tempfile::tempdir().unwrap();
            let mut m = make(d.path());
            if open {
                support::open_texture_groups(&mut m);
            } else {
                m = all_closed(m);
            }
            let mut h = harness(m, 1280.0, height);
            h.run();
            let kinds = support::control_kinds(&h, sdc_gui::ui::centre_rect(&h.ctx).unwrap());
            let state = if open { "open" } else { "closed" };
            eprintln!(
                "{name} ({state}, {height} high): {} control kinds: {kinds:?}",
                kinds.len()
            );
        }
    }
}

#[test]
fn every_group_starts_closed() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(rock(d.path()), 1280.0, 800.0);
    let mut seen = 0;
    for g in PanelGroup::ALL {
        assert!(!panel_groups::default_open(g), "{}", g.title());
        if let Some(n) = h.query_by_label_contains(&format!("{}, ", g.title())) {
            seen += 1;
            assert!(
                n.accesskit_node().label().unwrap().contains(", collapsed"),
                "{}",
                g.title()
            );
        }
    }
    assert!(seen >= 4, "the groups were found at all");
}

#[test]
fn the_first_screen_needs_no_scrolling_at_1280_by_800() {
    let d = tempfile::tempdir().unwrap();
    let mut cases: Vec<(&str, AppModel, &str)> = vec![(
        "decided colour",
        rock(d.path().join("a").as_path()),
        "Texture type",
    )];
    cases.push((
        "undecided",
        undecided_rock(d.path().join("b").as_path()),
        "Type for this texture",
    ));
    let (cube, _) = cube_without_mips(d.path().join("c").as_path());
    cases.push(("cube", cube, "Texture type"));
    for (name, m, type_label) in cases {
        let h = harness(all_closed(m), 1280.0, 800.0);
        let centre = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
        for l in [type_label, "Format"] {
            assert!(
                centre.contains_rect(h.get_by_role_and_label(Role::ComboBox, l).rect()),
                "{name}: {l}"
            ); // label and combo share the name
        }
        assert!(
            centre.contains_rect(h.get_by_label_contains("Output: ").rect()),
            "{name}"
        );
        assert!(
            support::bar_rects_in(&h, centre).is_empty(),
            "{name}: the centre scrolls"
        );
    }
}

#[test]
fn five_problem_rows_draw_three_and_a_more_button() {
    // first_screen_problems drawn alone with five long Problem rows, at the centre's width
    let rows: Vec<Problem> = (0..5).map(|_| problem_row(&"x".repeat(200))).collect();
    let lines = problem_lines(rows.len(), 0, Some(SetTarget::Key("set-a".into())));
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(560.0, 400.0))
        .build_ui_state(
            |ui, acts: &mut Vec<Action>| first_screen_problems(ui, &rows, &lines, acts),
            vec![],
        );
    h.run();
    assert_eq!(
        h.query_all_by_label_contains("x".repeat(20).as_str())
            .count(),
        3
    );
    let more = h.get_by_label("2 more problems: open Problems");
    assert!(more.rect().right() <= 560.0);
    let bottom = h
        .query_all_by_role(Role::Button)
        .map(|n| n.rect().bottom())
        .fold(0.0, f32::max);
    assert!(
        bottom <= 3.0 * 24.0 + 24.0 + 8.0,
        "three rows and the button, each one line: {bottom}"
    );
}

#[test]
fn a_120_character_name_keeps_the_first_screen_on_screen() {
    let d = tempfile::tempdir().unwrap();
    let (m, _) = long_named(d.path());
    let h = harness(all_closed(m), 1280.0, 800.0);
    let centre = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
    assert!(centre.contains_rect(h.get_by_label_contains("Output: ").rect()));
    assert!(support::bar_rects_in(&h, centre).is_empty());
}

/// ui_proof's `scroll_bar_clearance` on the centre (rule 3.2): with every group open the panel scrolls, and each header
/// on screen ends short of the floating bar's width, as ↺ does, so a click there is the header's. The header senses and
/// paints in that shortened rect, so "● changed" sits left of the bar too.
#[test]
fn a_header_ends_before_the_centre_scroll_bar() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    set(&mut m, &[("mips.filter", json!("box"))]);
    support::open_texture_groups(&mut m);
    let h = harness(m, 1280.0, 800.0);
    let centre = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
    let track = h
        .query_all_by_role(Role::ScrollBar)
        .map(|n| n.rect())
        .find(|r| centre.contains_rect(*r))
        .expect("premise: the open groups scroll the centre");
    let limit = support::bar_clearance(false, track, sdc_gui::look::thin_scroll().bar_width);
    let headers: Vec<(String, egui::Rect)> = h
        .query_all_by_role(Role::DisclosureTriangle)
        .map(|n| (n.accesskit_node().label().unwrap_or_default(), n.rect()))
        .filter(|(_, r)| centre.contains_rect(*r) && r.max.y > track.min.y && r.min.y < track.max.y)
        .collect();
    assert!(
        headers.len() >= 5,
        "premise: the headers on screen: {headers:?}"
    );
    for (name, r) in &headers {
        assert!(
            r.max.x <= limit + 0.5,
            "{name}: ends at {}, the bar's limit is {limit}",
            r.max.x
        );
    }
}

// The final review's fix wave.

/// The id and slot number of the set slot `key` fills as its own file.
fn own_slot(m: &AppModel, key: &str) -> (String, u8) {
    use sdc_gui::model::sets::SlotFill;
    sdc_gui::model::sets::views(m)
        .iter()
        .find_map(|v| {
            v.slots.iter().find_map(|(k, f)| match f {
                SlotFill::Own { item } if item == key => k.slot_number().map(|n| (v.id.clone(), n)),
                _ => None,
            })
        })
        .unwrap_or_else(|| panic!("{key} fills no slot of its own"))
}

/// TPC-F-3: a normal in a locked slot takes no green, skipped with the lock's reason as every other bulk row skips it
/// (LM11), at the row and at the click.
#[test]
fn a_normal_in_a_locked_slot_takes_no_green() {
    use sdc_gui::model::bulk::{green_reach, SKIP_MSN, SKIP_NOT_NORMAL};
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_and_others(d.path());
    let (set, n) = own_slot(&m, "g/a_n.png");
    m.set_slot_lock(&set, n, true).unwrap();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(
        sdc_gui::model::sets::locked_items(&m)
            .get("g/a_n.png")
            .map(String::as_str),
        Some("slot locked"),
        "premise: a_n fills a locked slot of its own set"
    );
    let r = green_reach(&m, &green_keys());
    assert_eq!(r.takes, ["g/b_n.png"]);
    assert_eq!(
        r.skipped,
        [
            ("slot locked".to_string(), 1),
            (SKIP_MSN.to_string(), 1),
            (SKIP_NOT_NORMAL.to_string(), 1)
        ]
    );
    let keys = select(&mut m, &GREEN_KEYS);
    assert_eq!(m.decide_many_green(&keys, GreenConvention::Opengl), Ok(1));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(
        green_of(&m, "g/a_n.png"),
        None,
        "the locked normal is not written"
    );
    assert_eq!(green_of(&m, "g/b_n.png"), Some(json!("opengl")));
    assert_eq!(
        m.bulk.report.as_deref(),
        Some("Green: OpenGL written to 1 of 4")
    );
    let mut h = green_panel(m, &GREEN_KEYS, false);
    h.run();
    let note = green_reach_note(&h, "Green: OpenGL", "applies to 1 of 4");
    h.hover_at(note.center());
    h.run();
    h.get_by_label_contains("locked on 1 of 4: slot locked");
}

/// CODING_STANDARDS, "The fast re-plan keeps what the full plan keeps": the lock is a document fact, so the Green row
/// leaves the locked normal out while the lock's own plan (a full plan; a lock has no fast one) is still out, after it,
/// and after another edit's fast re-plan alone and its full plan.
#[test]
fn the_green_row_skips_a_locked_slot_after_the_fast_plan_and_the_full_plan_each_alone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normals_and_others(d.path());
    let (set, n) = own_slot(&m, "g/a_n.png");
    select(&mut m, &["g/a_n.png", "g/b_n.png"]);
    for g in PanelGroup::ALL {
        panel_groups::set_open(&mut m.settings, g, false);
    }
    let mut h = harness(m, 1280.0, 1600.0);
    green_reach_note(&h, "Green: DirectX", "applies to 2 of 2");
    h.state_mut().set_slot_lock(&set, n, true).unwrap();
    assert!(h.state().plan_wait(), "premise: the lock's plan is out");
    h.run();
    green_reach_note(&h, "Green: DirectX", "applies to 1 of 2"); // the lock's plan still out
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run();
    green_reach_note(&h, "Green: DirectX", "applies to 1 of 2"); // after it
    h.state_mut().hold_plan_results = true;
    h.state_mut()
        .set_field("g/b_n.png", "mips.filter", json!("box"));
    let mut full = vec![];
    land_partial(&mut h, &mut full);
    h.run();
    green_reach_note(&h, "Green: DirectX", "applies to 1 of 2"); // another edit's fast plan alone
    let m = h.state_mut();
    m.hold_plan_results = false;
    for x in full {
        m.sender().send(x).unwrap();
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
    h.run();
    green_reach_note(&h, "Green: DirectX", "applies to 1 of 2"); // and its full plan
}

/// LQ7 and TPC-F-1 (Minor 5): FMT-HDR-SOURCE is an Info code. As a row's own code it is a note (counted, never listed);
/// in a problem row's `also` it no longer hides that row.
#[test]
fn an_hdr_source_line_is_a_note_and_hides_no_problem_row() {
    use sdc_gui::ui::settings_panel::panel_problem_rows;
    let mut own = problem_row("hdr");
    own.code = Some(Code::FmtHdrSource);
    let mut also = problem_row("a problem that also names the HDR line");
    also.also = vec![Code::FmtHdrSource];
    let (rows, notes) = panel_problem_rows(&[own, also.clone()], KEY, false);
    assert_eq!(rows, vec![also], "listed as any problem row");
    assert_eq!(notes, 1, "the Info row is counted as a note");
}

/// LQ5 and TPC-F-1 (Minor 6): the latch fills once the job resolves, so a problem the alpha answer raises opens its
/// group. leaf's alpha question is open (proposal Cutout); BC3 with Perceptual is fine for Cutout, and Data raises
/// FMT-PERCEPTUAL-DATA, whose field is in Compression.
#[test]
fn a_problem_the_alpha_answer_raises_opens_its_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.decide(LEAF, Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    for (f, v) in [
        ("format", json!("bc3")),
        ("encoder.perceptual", json!(true)),
    ] {
        m.set_field(LEAF, f, v.clone());
        assert!(m.wait_idle(Duration::from_secs(60)));
        assert_eq!(
            m.project.as_ref().unwrap().file_value(LEAF, f),
            Some(v),
            "premise: {f} written"
        );
    }
    panel_groups::set_open(&mut m.settings, PanelGroup::Compression, false);
    m.select_item(LEAF);
    let mut h = harness(m, 1280.0, 800.0);
    h.get_by_label("Confirm alpha"); // the question is open
    assert!(h
        .query_by_label_contains("Compression, collapsed")
        .is_some());
    h.state_mut()
        .decide(LEAF, Answer::Alpha(engine::config::AlphaPolicy::Data));
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run_steps(5);
    let n = h.get_by_label_contains("Compression, expanded");
    assert!(n
        .accesskit_node()
        .label()
        .unwrap()
        .contains(", has a problem"));
}

/// A header whose open flag and hold are the state `(open, held)`, over a one-line body and a label below, with motion
/// on (see `motion_harness`).
fn held_harness() -> Harness<'static, (bool, bool)> {
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(400.0, 300.0))
        .with_step_dt(1.0 / 120.0)
        .with_max_steps(40)
        .build_ui_state(
            |ui, st: &mut (bool, bool)| {
                let g = collapsing_group(
                    ui,
                    "t",
                    &mipmaps_head("Kaiser, full chain", None),
                    st.0,
                    st.1,
                    |ui| ui.label("row inside"),
                );
                if g.toggled {
                    // As `set_group_open` does: the user's click ends the hold.
                    *st = (!st.0, false);
                }
                ui.label("below");
            },
            (false, false),
        );
    h.ctx.all_styles_mut(|s| s.animation_time = 1.0 / 12.0);
    sdc_gui::ui::motion::force_reduced(&h.ctx, false);
    h.run();
    assert!(
        !sdc_gui::ui::motion::reduced(&h.ctx),
        "premise: motion is on in this harness"
    );
    h
}

/// LQ8 and TPC-F-1 (Minor 7): a forced group never slides. A hold that ends without a click (the next selection resets
/// `panel_forced`, and the group is closed there) snaps shut in one step, as the forced open snapped.
#[test]
fn a_forced_hold_that_ends_closes_without_sliding() {
    let mut h = held_harness();
    let below = |h: &Harness<'static, (bool, bool)>| h.get_by_label("below").rect().top();
    let closed = below(&h);
    *h.state_mut() = (true, true);
    h.run();
    assert!(
        below(&h) > closed + 10.0,
        "premise: the forced open shows the body"
    );
    *h.state_mut() = (false, false);
    h.step();
    let one = below(&h);
    h.run();
    assert_eq!(one, closed, "shut at the first step");
    assert_eq!(one, below(&h), "and nothing slides after it");
}

/// LQ8: a mouse close of a forced group snaps too (the hold is still set in the click's frame). A pin of the old
/// behaviour, which the brief keeps.
#[test]
fn a_mouse_close_of_a_forced_group_does_not_slide() {
    let mut h = held_harness();
    let below = |h: &Harness<'static, (bool, bool)>| h.get_by_label("below").rect().top();
    let closed = below(&h);
    *h.state_mut() = (true, true);
    h.run();
    h.get_by_role(Role::DisclosureTriangle).click();
    h.step();
    let one = below(&h);
    h.run();
    assert_eq!(*h.state(), (false, false), "premise: the click closed it");
    assert_eq!(one, closed, "shut at the first step");
}

/// LQ8 on the panel: Mipmaps is forced open on the cube (CUBE-NO-MIPS); selecting a texture where it is closed shuts it
/// at once, so the headers under it do not slide up.
#[test]
fn a_new_selection_closes_a_forced_group_without_sliding() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, key) = cube_without_mips(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    m.decide(FLAT, Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.select_item(&key);
    // `harness`, with `motion_harness`'s short steps: kittest's default step finishes a collapse in one step.
    let mut h = Harness::builder()
        .with_context_setup(install_fonts)
        .with_size(egui::vec2(1280.0, 800.0))
        .with_step_dt(1.0 / 120.0)
        .with_max_steps(40)
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.ctx.all_styles_mut(|s| s.animation_time = 1.0 / 12.0);
    sdc_gui::ui::motion::force_reduced(&h.ctx, false);
    h.run();
    h.get_by_label_contains("Mipmaps, expanded");
    assert!(
        !sdc_gui::ui::motion::reduced(&h.ctx),
        "premise: motion is on"
    );
    h.state_mut().select_item(FLAT);
    h.step();
    let one = h.get_by_label_contains("Normal map, ").rect().top();
    h.run_steps(30);
    assert!(h.query_by_label_contains("Mipmaps, collapsed").is_some());
    assert_eq!(
        one,
        h.get_by_label_contains("Normal map, ").rect().top(),
        "the header under Mipmaps is in place at the first step"
    );
}

/// LQ4 (Minor 8): "● changed" mirrors the resets inside the group, on both panels. A diffuse's Normal map rows are all
/// locked ("normal maps only"); the texture's own `mips.normal` shows its ↺ on the single panel, so the header says
/// changed. In bulk a row counts only the textures it reaches, none here, so neither the ↺ nor "changed" shows. A pin.
#[test]
fn changed_follows_the_resets_on_both_panels() {
    let d = tempfile::tempdir().unwrap();
    let mut m = two_alike(d.path());
    m.set_field("a/one.png", "mips.normal", json!("renormalize_bake"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value("a/one.png", "mips.normal"),
        Some(json!("renormalize_bake")),
        "premise: the locked row holds the texture's own value"
    );
    panel_groups::set_open(&mut m.settings, PanelGroup::NormalMap, true);
    m.select_item("a/one.png");
    let mut h = harness(m, 1280.0, 1600.0);
    h.run();
    h.get_by_label("Normal map, expanded, changed");
    // The ↺ buttons between the Normal map header and the next one (Size).
    let resets = |h: &Harness<'static, AppModel>| {
        let top = h.get_by_label_contains("Normal map, ").rect().bottom();
        let end = h.get_by_label_contains("Size, ").rect().top();
        h.query_all_by_label("↺")
            .filter(|n| n.rect().top() >= top && n.rect().bottom() <= end)
            .count()
    };
    assert!(resets(&h) >= 1, "the single panel's ↺ on the locked row");
    h.state_mut().click_item("a/two.png", true, false);
    h.run();
    h.get_by_label("Normal map, expanded");
    assert_eq!(resets(&h), 0, "no bulk row reaches a texture, so no ↺");
}

/// LQ13 and TPC-F-1 (Minor 9): the bulk "Treat as DirectX" writes DirectX, unlike the one-texture button, and says so.
#[test]
fn the_bulk_directx_button_says_it_writes_directx() {
    let d = tempfile::tempdir().unwrap();
    let mut h = green_panel(normals_and_others(d.path()), &GREEN_KEYS, false);
    h.run();
    h.get_by_label("Treat as DirectX").hover();
    h.run();
    h.get_by_label_contains("Writes DirectX to each of these normal maps");
}

/// LQ5: a texture selected while a plan is pending draws its last job, which may not have the plan's problems yet; the
/// latch waits for the plan. Here the cube's `mips.levels = none` is still planning (CUBE-NO-MIPS is not on its job)
/// when it is selected and drawn; once the plan lands, Mipmaps opens.
#[test]
fn a_selection_during_a_pending_plan_fills_after_the_plan_lands() {
    let d = tempfile::tempdir().unwrap();
    let mut m = cube_model(d.path(), FLAT);
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    m.set_field(CUBE, "mips.levels", json!("none"));
    assert!(m.plan_wait(), "premise: the plan is pending");
    m.select_item(CUBE);
    let mut h = harness(m, 1280.0, 800.0); // `ui::draw` polls no plan: it stays pending while this draws
    h.run();
    assert!(
        h.state().plan_wait(),
        "premise: still pending after the draw"
    );
    assert!(h.query_by_label_contains("Mipmaps, collapsed").is_some());
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run_steps(5);
    assert!(h.query_by_label_contains("Mipmaps, expanded").is_some());
}

/// The latch's gate is a settled plan (`plan_wait`), not `fix_off()`: a read-only `sdc.toml` keeps edits off, but a
/// problem still opens its group (headers are navigation, LQ2). A pin of the forced open, guarding the gate's choice.
#[test]
fn a_read_only_sdc_toml_still_opens_a_problem_s_group() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, key) = cube_without_mips(d.path());
    panel_groups::set_open(&mut m.settings, PanelGroup::Mipmaps, false);
    m.project
        .as_mut()
        .unwrap()
        .set_error("rejected by the engine");
    assert!(m.fix_off().is_some(), "premise: edits are off");
    m.select_item(&key);
    let mut h = harness(m, 1280.0, 800.0);
    h.run();
    assert!(h.query_by_label_contains("Mipmaps, expanded").is_some());
}
