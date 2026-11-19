//! Review C1, C21 and C14, owner amendment C (Note FB2): the set editor is one scrolling column. The slot rows sit on
//! top at full width, the Material / Also export tabs and groups below them, so nothing is ever drawn over the rows and
//! no sideways scroll holds them; the set name and the slot label are each shown once.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::{AppModel, CentreTab};
use sdc_gui::ui::State;
use support::*;

type H = Harness<'static, (AppModel, State)>;

fn harness(m: AppModel, w: f32) -> H {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(w, 800.0))
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, State::default()),
        );
    h
}

fn rock_set(dir: &std::path::Path) -> (AppModel, String) {
    let mut m = pbr_opened_plain(dir);
    let v = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.ends_with("landscape/rock01"))
        .map(|v| (v.key.clone(), v.id.clone()))
        .expect("the rock01 set");
    m.select_set(&v.0);
    (m, v.1)
}

/// At every width, with either tab open: each slot-row widget is above the tabs and the first group, starts at the
/// tabs' left edge, and ends inside the centre column (no sideways scroll, nothing beside the rows).
#[test]
fn set_editor_is_one_column() {
    for open in ["Material", "Also export"] {
        for w in [1024.0, 1100.0, 1280.0, 1536.0] {
            let d = tempfile::tempdir().unwrap();
            let (m, _) = rock_set(d.path());
            let mut h = harness(m, w);
            h.run();
            // Note JI2: the tab picked lives in egui memory, so each case clicks the one it wants.
            h.get_by_role_and_label(Role::Button, open).scroll_to_me();
            h.run();
            h.get_by_role_and_label(Role::Button, open).click();
            h.run();
            let at = format!("{w} px, {open} tab");
            // The centre panel's rect grows with its content; what the user sees ends at the window.
            let full = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
            let centre = full.intersect(h.ctx.content_rect());
            let tab = |t: &str| h.get_by_role_and_label(Role::Button, t).rect();
            let (material, also) = (tab("Material"), tab("Also export"));
            // The first thing under the tabs: the Material group, or the Also export tab's "This set" heading.
            let first = if open == "Also export" {
                h.get_by_label("This set").rect()
            } else {
                h.query_all_by_label("Material")
                    .find(|n| n.accesskit_node().role() != Role::Button)
                    .expect("the Material group")
                    .rect()
            };
            let top = material.min.y.min(also.min.y).min(first.min.y);
            let (diffuse, cmds) = (
                slot_rect(&h, "Diffuse"),
                h.get_by_role_and_label(Role::Button, "Slot commands…")
                    .rect(),
            );
            // Between the first row and Slot commands… (the rows' y-range, in either layout).
            let in_rows = |r: &egui::Rect| {
                r.min.x >= full.min.x && r.min.y >= diffuse.min.y - 30.0 && r.max.y <= cmds.max.y
            };
            let mut rows = vec![diffuse, slot_rect(&h, "Normal _n"), cmds];
            for l in [
                "Diffuse thumbnail",
                "Match on this (Diffuse)",
                "Match on this (Normal _n)",
                "Assign…",
                "ready",
                "warning",
                "blocks export",
            ] {
                rows.extend(h.query_all_by_label(l).map(|n| n.rect()).filter(in_rows));
            }
            // The format, recipe and channel source combos and the fill values.
            for role in [Role::ComboBox, Role::SpinButton] {
                rows.extend(h.query_all_by_role(role).map(|n| n.rect()).filter(in_rows));
            }
            // Note JI5: the Flags cell's alpha dropdown (it replaced "Opaque") is in the rows, above the tabs.
            let alpha: Vec<_> = h
                .query_all_by_role(Role::ComboBox)
                .filter(|n| {
                    n.accesskit_node()
                        .label()
                        .is_some_and(|l| l.starts_with("Slot alpha type: "))
                })
                .map(|n| n.rect())
                .collect();
            assert_eq!(alpha.len(), 1, "{at}: the slot alpha combo");
            assert!(in_rows(&alpha[0]), "{at}: {:?} is in the rows", alpha[0]);
            assert!(rows.len() > 6, "{at}: found the row widgets {rows:?}");
            for r in &rows {
                assert!(
                    r.max.y <= top,
                    "{at}: row widget {r:?} is not above the tabs and groups (top {top})"
                );
                assert!(
                    r.max.x <= centre.max.x,
                    "{at}: row widget {r:?} reaches past the centre {centre:?}"
                );
            }
            let left = slot_rect(&h, "Diffuse").min.x;
            assert!(
                (left - material.min.x).abs() <= 2.0,
                "{at}: rows start at {left}, the tabs at {}",
                material.min.x
            );
        }
    }
}

/// C14: the name box names the set; no label below it repeats the name.
#[test]
fn the_set_name_is_shown_once() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = rock_set(d.path());
    let mut h = harness(m, 1400.0);
    h.run();
    let centre = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    let labels: Vec<_> = h
        .query_all_by_label(&id)
        .filter(|n| n.accesskit_node().role() == Role::Label && centre.contains(n.rect().center()))
        .map(|n| n.rect())
        .collect();
    assert!(
        labels.is_empty(),
        "{id:?} repeated as a label at {labels:?}"
    );
}

/// C14: the Features group, whose ticks are the state, has no count beside its title.
#[test]
fn the_features_group_has_no_header_value() {
    let d = tempfile::tempdir().unwrap();
    let (m, _) = rock_set(d.path());
    let mut h = harness(m, 1400.0);
    h.run();
    let t = h.get_by_label("Features").rect();
    let beside: Vec<_> = h
        .query_all_by_role(Role::Label)
        .filter(|n| n.rect().min.x > t.max.x && (n.rect().center().y - t.center().y).abs() < 2.0)
        .filter_map(|n| n.value())
        .collect();
    assert!(beside.is_empty(), "Features header value {beside:?}");
}

/// The header values beside a group title: the "beside labels" form of `the_features_group_has_no_header_value`.
fn beside(h: &H, title: &str) -> Vec<String> {
    let t = h
        .query_all_by_label(title)
        .find(|n| n.accesskit_node().role() == Role::Label)
        .unwrap_or_else(|| panic!("the {title} group"))
        .rect();
    h.query_all_by_role(Role::Label)
        .filter(|n| n.rect().min.x > t.max.x && (n.rect().center().y - t.center().y).abs() < 2.0)
        .filter_map(|n| n.value().map(|v| v.to_string()))
        .collect()
}

/// C14: a feature's state shows once, in its Features tick and its control, never again in the group's header.
#[test]
fn a_feature_group_header_has_no_state_word() {
    for (title, key) in [
        ("Emissive settings", "emissive"),
        ("Coat", "multilayer"),
        ("Glint settings", "glint_on"),
        ("Fuzz settings", "fuzz_on"),
    ] {
        let d = tempfile::tempdir().unwrap();
        let (mut m, id) = rock_set(d.path());
        // One feature on: nothing blocks it.
        m.set_json_value(&id, key, Some(serde_json::Value::Bool(true)));
        let mut h = harness(m, 1400.0);
        h.run();
        let words = beside(&h, title);
        assert!(
            !words.iter().any(|w| w == "on" || w == "off"),
            "{title} header repeats its state: {words:?}"
        );
    }
}

/// Click a group's title (scrolled into view first, as a user would).
fn click_title(h: &mut H, title: &str) {
    let n = h
        .query_all_by_label(title)
        .find(|n| n.accesskit_node().role() == Role::Label)
        .unwrap_or_else(|| panic!("the {title} group"));
    n.scroll_to_me();
    h.run();
    h.query_all_by_label(title)
        .find(|n| n.accesskit_node().role() == Role::Label)
        .unwrap()
        .click();
    h.run();
}

/// Is the Subsurface selector (a combo reading `mode`) on screen?
fn selector_shows(h: &H, mode: &str) -> bool {
    h.query_all_by_role(Role::ComboBox)
        .any(|n| n.value().as_deref() == Some(mode))
}

/// C14 and Task 5: the Subsurface selector is the one control showing the mode, so the header says the mode only while
/// the group is really closed (the user's click counts, not the default), and never while the selector is on screen.
#[test]
fn the_subsurface_header_says_the_mode_only_while_closed() {
    // Off: closed by default.
    let d = tempfile::tempdir().unwrap();
    let (m, _) = rock_set(d.path());
    let mut h = harness(m, 1400.0);
    h.run();
    assert_eq!(beside(&h, "Subsurface"), ["Off"], "closed at Off");
    click_title(&mut h, "Subsurface");
    assert!(beside(&h, "Subsurface").is_empty(), "opened by the user");
    assert!(selector_shows(&h, "Off"), "the selector is on screen");

    // On: open by default; the user closes it.
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = rock_set(d.path());
    m.set_json_value(&id, "subsurface", Some(serde_json::Value::Bool(true)));
    let mut h = harness(m, 1400.0);
    h.run();
    assert!(beside(&h, "Subsurface").is_empty(), "open by default");
    assert!(selector_shows(&h, "Standard"));
    click_title(&mut h, "Subsurface");
    assert_eq!(beside(&h, "Subsurface"), ["Standard"], "closed by the user");
}

/// Task 5 and CC10: a Subsurface that is on but blocked reads its reason in the header, closed or open.
#[test]
fn a_blocked_subsurface_header_reads_its_reason_open_or_closed() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = rock_set(d.path());
    for k in ["subsurface", "multilayer"] {
        m.set_json_value(&id, k, Some(serde_json::Value::Bool(true)));
    }
    let mut h = harness(m, 1400.0);
    h.run();
    let reason = ["Standard, blocked by multilayer"];
    assert_eq!(beside(&h, "Subsurface"), reason, "closed");
    click_title(&mut h, "Subsurface");
    assert_eq!(beside(&h, "Subsurface"), reason, "opened by the user");
}

/// C14: the Assign… menu's header is the slot label, once ("Emissive / Skin (slot 3)", not "Slot 3: Emissive / Skin
/// (slot 3)").
#[test]
fn the_assign_menu_header_is_the_slot_label() {
    let d = tempfile::tempdir().unwrap();
    let (m, _) = rock_set(d.path());
    let mut h = harness(m, 1400.0);
    h.run();
    let row = slot_rect(&h, "Emissive / Skin (slot 3)");
    h.query_all_by_label("Assign…")
        .find(|n| (n.rect().center().y - row.center().y).abs() < 20.0)
        .expect("Assign… on the slot 3 row")
        .click();
    h.run();
    assert_eq!(
        h.query_all_by_label_contains("Slot 3:").count(),
        0,
        "the header repeats the slot number"
    );
    // The row's own label and the menu's header.
    assert_eq!(h.query_all_by_label("Emissive / Skin (slot 3)").count(), 2);
}

/// Review E24: a centre tab in front (Presets, Settings, JSON) does not take the set editor's floor, so the side
/// columns keep the widths they have with nothing selected.
#[test]
fn a_centre_tab_in_front_does_not_take_the_editors_floor() {
    for open in [
        AppModel::open_presets,
        AppModel::open_settings_tab,
        AppModel::open_json_editor,
    ] {
        let left = |select: bool| {
            let d = tempfile::tempdir().unwrap();
            let (mut m, _) = rock_set(d.path());
            if !select {
                m.clear_selection();
            }
            open(&mut m);
            assert_ne!(
                m.centre_front(),
                CentreTab::Selection,
                "premise: a tab is in front"
            );
            let mut h = harness(m, 1057.0);
            h.run();
            sdc_gui::ui::centre_rect(&h.ctx).unwrap().min.x
        };
        let (with, without) = (left(true), left(false));
        assert!(
            (with - without).abs() < 0.5,
            "the side columns keep their widths behind a centre tab: {with} against {without}"
        );
    }
}

#[test]
fn selection_in_front_of_a_docked_view_keeps_the_editors_floor() {
    let left = |dock: bool| {
        let d = tempfile::tempdir().unwrap();
        let (mut m, _) = rock_set(d.path());
        if dock {
            let sdc_gui::model::Selection::Set(key) = m.selection.clone() else {
                panic!("selected set")
            };
            m.open_json_editor();
            m.select_set(&key);
            assert!(m.json_editor_open && m.centre_front() == CentreTab::Selection);
        }
        let mut h = harness(m, 1057.0);
        h.run_steps(4);
        sdc_gui::ui::centre_rect(&h.ctx).unwrap().left()
    };
    assert!(
        (left(true) - left(false)).abs() < 0.5,
        "a docked background view keeps the same editor floor"
    );
}

/// Review E24, the preview's floor (K8, Note FC8): a texture expanded behind a centre tab does not take the preview's
/// floor either. The floor is 776 px from 1280 px up, 520 below.
#[test]
fn a_centre_tab_in_front_does_not_take_the_previews_floor() {
    for w in [1057.0, 1280.0] {
        for open in [
            AppModel::open_presets,
            AppModel::open_settings_tab,
            AppModel::open_json_editor,
        ] {
            let left = |expand: bool| {
                let d = tempfile::tempdir().unwrap();
                let (mut m, _) = rock_set(d.path());
                m.clear_selection();
                if expand {
                    m.expand("landscape/leaf.png");
                    assert!(m.expanded.is_some(), "premise: a texture is expanded");
                }
                open(&mut m);
                if expand {
                    assert!(m.expanded.is_some(), "premise: the tab keeps it expanded");
                }
                assert_ne!(
                    m.centre_front(),
                    CentreTab::Selection,
                    "premise: a tab is in front"
                );
                let mut h = harness(m, w);
                h.run_steps(4);
                sdc_gui::ui::centre_rect(&h.ctx).unwrap().min.x
            };
            let (with, without) = (left(true), left(false));
            assert!(
                (with - without).abs() < 0.5,
                "{w} px: the side columns keep their widths behind a centre tab: {with} against {without}"
            );
        }
    }
}

/// A set whose diffuse row shows Confirm (`asks`: an undecided alpha on a plain diffuse) or the reset (its own alpha),
/// selected, in a window sized so the centre is exactly `ROWS_MIN` wide.
fn at_the_floor(asks: bool) -> (H, tempfile::TempDir) {
    use sdc_gui::ui::set_editor::ROWS_MIN;
    let set = if asks {
        "landscape/leaf"
    } else {
        "landscape/rock01"
    };
    // The width the other columns and chrome take, then corrected by what the first frames really give the centre.
    let mut w = ROWS_MIN + 515.0;
    for _ in 0..4 {
        let d = tempfile::tempdir().unwrap();
        let mut m = pbr_opened_plain(d.path());
        m.decide(
            "landscape/leaf.png",
            sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
        );
        if !asks {
            m.set_alpha("landscape/rock01.png", engine::config::AlphaPolicy::Blend);
        }
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        let key = m
            .sets
            .iter()
            .find(|s| s.key.ends_with(set))
            .unwrap()
            .key
            .clone();
        m.select_set(&key);
        let mut h = harness(m, w);
        h.run_steps(4);
        let full = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
        let centre = full.intersect(h.ctx.content_rect());
        let off = centre.width() - ROWS_MIN;
        if off.abs() <= 0.5 {
            // The caller keeps the temp folder for the harness's life, and it is deleted after.
            return (h, d);
        }
        w -= off;
    }
    panic!("no window width gives the centre exactly {ROWS_MIN} px");
}

/// Note JI5, Note HP goal 6: with the centre exactly at `ROWS_MIN`, every control of every slot row (the alpha combo,
/// Confirm and the reset included) ends inside the set editor, whose right edge is the centre's less its margin.
#[test]
fn slot_rows_fit_at_the_new_floor() {
    for asks in [true, false] {
        let (h, _d) = at_the_floor(asks);
        let full = sdc_gui::ui::centre_rect(&h.ctx).unwrap();
        let margin = egui::Frame::central_panel(&h.ctx.global_style())
            .inner_margin
            .right as f32;
        let edge = full.intersect(h.ctx.content_rect()).right() - margin;
        let at = format!("{} at the floor", if asks { "Confirm" } else { "reset" });
        let (diffuse, cmds) = (
            slot_rect(&h, "Diffuse"),
            h.get_by_role_and_label(Role::Button, "Slot commands…")
                .rect(),
        );
        // Every control between the first row and Slot commands….
        let in_rows = |r: &egui::Rect| {
            r.min.x >= full.min.x && r.min.y >= diffuse.min.y - 30.0 && r.max.y <= cmds.max.y
        };
        let mut rows = vec![];
        for role in [
            Role::Button,
            Role::ComboBox,
            Role::SpinButton,
            Role::CheckBox,
            Role::Image,
            Role::Label,
        ] {
            rows.extend(h.query_all_by_role(role).map(|n| n.rect()).filter(in_rows));
        }
        let want = if asks {
            "Slot confirm alpha"
        } else {
            "Reset slot alpha"
        };
        assert!(
            h.query_all_by_label(want).count() == 1,
            "{at}: the row shows {want}"
        );
        assert!(rows.len() > 12, "{at}: found the row controls {rows:?}");
        for r in &rows {
            assert!(
                r.max.x <= edge,
                "{at}: row control {r:?} passes the editor's edge {edge}"
            );
        }
    }
}
