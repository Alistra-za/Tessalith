//! Layout v2 A3 / C6: every set editor has a Material (JSON set) or Set tab and an Also export tab (Note JI2), under
//! the slot rows since Note FB2 (one column); nothing is drawn at the window edge in any state (Note JI7).
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::set_preview::{SetPreview, DEFAULT_CAP};
use sdc_gui::model::AppModel;
use sdc_gui::ui::State;
use support::*;

type H = Harness<'static, (AppModel, State)>;

/// Reduced from set_preview_layout.rs `harness_with` (K19): `three_d` builds the enabled (IO-free) set preview.
fn harness_with(m: AppModel, w: f32, h: f32, three_d: bool) -> H {
    let state = State {
        set3d: if three_d {
            SetPreview::new(None, DEFAULT_CAP)
        } else {
            SetPreview::default()
        },
        ..State::default()
    };
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(w, h))
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, state),
        );
    h
}

fn harness(m: AppModel) -> H {
    harness_with(m, 1400.0, 900.0, false)
}

fn rock_selected(m: &mut AppModel) {
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with("landscape/rock01"))
        .unwrap()
        .key
        .clone();
    m.select_set(&key);
}

/// Note FB2: the set editor is one column in the centre; its tabs and groups end at the centre's visible edge (the
/// panel's rect grows with its content, so it is cut at the window).
fn centre(h: &H) -> egui::Rect {
    sdc_gui::ui::centre_rect(&h.ctx)
        .expect("the centre is drawn")
        .intersect(h.ctx.content_rect())
}

/// The set editor's tabs are drawn: its "Also export" tab is in the centre (Note JI2).
fn set_tabs(h: &H) -> bool {
    sdc_gui::ui::centre_rect(&h.ctx).is_some_and(|c| {
        h.query_all_by_role(Role::Button).any(|n| {
            n.accesskit_node().label().as_deref() == Some("Also export")
                && c.contains(n.rect().center())
        })
    })
}

/// Note JI2 (Review Focus 1): on a set that is not PBR the Also export tab says it reads PBR sets only, with no Skip
/// tick, and still shows the project's ticks and recipe.
#[test]
fn a_vanilla_set_reads_pbr_only_and_keeps_the_project_block() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    // rock01 is auto PBR Landscape here (its roughness and AO pack); leaf is a plain set.
    let key = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.id == "landscape/leaf")
        .filter(|v| !sdc_gui::model::also::is_pbr_set(v))
        .expect("the plain leaf set")
        .key
        .clone();
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(Role::Button, "Set");
    h.get_by_role_and_label(Role::Button, "Also export").click();
    h.run();
    h.get_by_label("Also export reads PBR sets only; this set is not one");
    assert!(h.query_by_label("Skip vanilla/CM for this set").is_none());
    h.get_by_role_and_label(Role::CheckBox, "All sets: also export as Vanilla");
    h.get_by_role_and_label(Role::SpinButton, "Diffuse brightness");
}

/// 2.0 debloat (Note GJ5): the set editor has no "Used by plugins" section.
#[test]
fn the_set_editor_has_no_used_by_plugins_section() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    rock_selected(&mut m);
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(Role::Button, "Set");
    assert!(h
        .query_all_by_label_contains("Used by plugins")
        .next()
        .is_none());
}

#[test]
fn a_pbr_set_has_a_material_tab() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    rock_selected(&mut m);
    let mut h = harness(m);
    h.run();
    // K11: the pgp group "Material" shares the label; the tab is the Button.
    h.get_by_role_and_label(Role::Button, "Material");
    h.get_by_label("Features");
}

/// Note JI2: a PBR set's Also export tab holds this set's Skip tick and the project's block, not the drawer's body;
/// the tab picked lives in egui memory, so nothing is written to gui.toml (review finding 9).
#[test]
fn a_pbr_set_has_material_and_also_export_tabs_and_the_choice_is_not_saved() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let toml = d.path().join("settings").join("gui.toml");
    m.settings_path = Some(toml.clone());
    rock_selected(&mut m);
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.key.ends_with("landscape/rock01") && sdc_gui::model::also::is_pbr_set(v)),
        "rock01 is a PBR set"
    );
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(Role::Button, "Material");
    h.get_by_role_and_label(Role::Button, "Also export").click();
    h.run();
    h.get_by_label("This set");
    h.get_by_label("All sets in this project");
    h.get_by_label("Skip vanilla/CM for this set");
    assert!(h.query_by_label("No texture selected").is_none());
    assert!(h.query_by_label_contains("Preset inspector").is_none());
    assert!(h
        .query_by_label_contains("Select a texture to edit")
        .is_none());
    assert!(h.query_by_label("Features").is_none(), "one tab at a time");
    assert!(!toml.exists(), "the tab choice is not written to gui.toml");
}

/// Note FB2 (was "the slot rows stay put while the material panel scrolls"): one column scrolls as one, so the rows
/// move with the groups under them.
#[test]
fn the_slot_rows_scroll_with_the_material_groups() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    rock_selected(&mut m);
    let mut h = harness_with(m, 1400.0, 700.0, false);
    h.run();
    let diffuse = slot_rect(&h, "Diffuse");
    let target = h.get_by_label("Advanced TruePBR settings").rect();
    h.get_by_label("Advanced TruePBR settings").scroll_to_me();
    h.run();
    let moved = h.get_by_label("Advanced TruePBR settings").rect();
    let dy = target.min.y - moved.min.y;
    assert!(dy > 0.0, "the column scrolled ({target:?} → {moved:?})");
    let now = slot_rect(&h, "Diffuse");
    assert!(
        (diffuse.min.y - now.min.y - dy).abs() < 1.0,
        "the rows moved with it ({diffuse:?} → {now:?}, {dy})"
    );
}

/// A group's title (K11: not the tab, which is a Button).
fn title<'a>(h: &'a H, t: &'a str) -> egui_kittest::Node<'a> {
    h.query_all_by_label(t)
        .find(|n| n.accesskit_node().role() != Role::Button)
        .unwrap_or_else(|| panic!("no group {t}"))
}

/// The value right-aligned in a group's header row (the same words can repeat, so it is found beside its title).
fn header_value(h: &H, t: &str) -> String {
    let r = title(h, t).rect();
    h.query_all_by_role(Role::Label)
        .filter(|n| n.rect().min.x > r.max.x && (n.rect().center().y - r.center().y).abs() < 2.0)
        .find_map(|n| n.value())
        .unwrap_or_else(|| panic!("no header value beside {t}"))
}

/// C14: a feature's group repeats neither its state nor its mode in the header.
fn header_has_no_value(h: &H, t: &str) -> bool {
    let r = title(h, t).rect();
    h.query_all_by_role(Role::Label)
        .filter(|n| n.rect().min.x > r.max.x && (n.rect().center().y - r.center().y).abs() < 2.0)
        .all(|n| n.value().is_none())
}

#[test]
fn the_material_groups_are_framed_in_order_with_values() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    rock_selected(&mut m);
    let mut h = harness(m);
    h.run();
    // Note FM33: a feature's card shows only while the feature is on, so Coat is not listed here.
    let ys: Vec<f32> = [
        "Material",
        "Features",
        "Subsurface",
        "NiAlphaProperty",
        "Matching",
        "CS PBR records",
        "Advanced TruePBR settings",
    ]
    .iter()
    .map(|t| title(&h, t).rect().min.y)
    .collect();
    assert!(ys.windows(2).all(|w| w[0] < w[1]), "{ys:?}");
    assert!(h.query_all_by_label("Coat").next().is_none());
    // Subsurface has no Features tick: closed at Off, its header says the mode.
    assert_eq!(header_value(&h, "Subsurface"), "Off");
    assert_eq!(header_value(&h, "Matching"), "1 match");
    assert_eq!(header_value(&h, "NiAlphaProperty"), "off");
    assert_eq!(header_value(&h, "CS PBR records"), "0 records");
}

#[test]
fn turning_multilayer_on_opens_the_coat_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    rock_selected(&mut m);
    let mut h = harness(m);
    h.run();
    assert!(h.query_by_label("Coat colour").is_none());
    // K21: the checkbox's label. Note FB2: below the slot rows; scroll to it as a user would.
    h.get_by_label("Multilayer (coat)").scroll_to_me();
    h.run();
    h.get_by_label("Multilayer (coat)").click();
    h.run();
    assert!(header_has_no_value(&h, "Coat"));
    h.get_by_label("Coat colour");
}

/// Note FB2 (C14): the button sits on the tab row, after the Also export tab (Note JI2).
#[test]
fn the_json_editor_button_is_small_and_on_the_tab_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    rock_selected(&mut m);
    let mut h = harness(m);
    h.run();
    let tab = h.get_by_role_and_label(Role::Button, "Also export").rect();
    let b = h.get_by_label("Material: PGPatcher JSON editor…");
    assert!(b.rect().width() < 220.0);
    assert!(
        b.rect().min.x > tab.max.x && tab.y_range().contains(b.rect().center().y),
        "button {:?} beside the tab {tab:?}",
        b.rect()
    );
    b.click();
    h.run();
    assert!(h.state().0.json_editor_open);
}

/// Note DO, then FB2: at 1000 px with a long set name the button stays inside the centre.
#[test]
fn the_json_editor_button_stays_inside_a_narrow_centre() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    rock_selected(&mut m);
    // The screenshot's case: a long name.
    m.set_input_name(
        "landscape/rock01",
        "textures/landscape/kingolafeffigy/kingolafeffigyeffigy01",
    )
    .unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with("kingolafeffigyeffigy01"))
        .expect("the renamed set")
        .key
        .clone();
    m.select_set(&key);
    let mut h = harness_with(m, 1000.0, 800.0, false);
    h.run();
    let p = centre(&h);
    let b = h.get_by_label("Material: PGPatcher JSON editor…").rect();
    assert!(b.max.x <= p.max.x, "button {b:?} inside the centre {p:?}");
}

/// Note JI7: the Advanced drawer is gone, so nothing is drawn at the window edge in any centre state: no "Advanced"
/// label, and the centre reaches the window's right edge (less the harness' 8 px margin). The set editor's tabs are
/// drawn for a set in 2D only, and `set_editor_shown` agrees with that routing in every state with the selection in
/// front (its pin). Each case is a fresh harness, so the persisted panel states are this case's own. The last field is
/// the set editor's tabs.
#[test]
fn no_window_edge_strip_in_any_state() {
    type Setup = fn(&mut AppModel);
    let cases: [(&str, Setup, bool, bool); 11] = [
        // A11: the start screen.
        (
            "no folder",
            |m| *m = AppModel::placeholder(Default::default()),
            false,
            false,
        ),
        (
            "nothing",
            |m| m.selection = sdc_gui::model::Selection::None,
            false,
            false,
        ),
        (
            "a texture",
            |m| m.select_item("landscape/leaf.png"),
            false,
            false,
        ),
        ("a set in 2D", rock_selected, false, true),
        (
            "a set in 3D",
            |m| {
                rock_selected(m);
                m.settings.set_view_3d = true;
            },
            true,
            false,
        ),
        (
            "two sets",
            |m| {
                let keys: Vec<String> = m.sets.iter().map(|s| s.key.clone()).collect();
                m.click_set(&keys[0], false, false);
                m.click_set(&keys[1], true, false);
                assert!(m.in_bulk());
            },
            false,
            false,
        ),
        (
            "expanded texture over a set",
            |m| {
                rock_selected(m);
                m.expanded = Some("landscape/rock01.png".into());
            },
            false,
            false,
        ),
        (
            "a stale set",
            |m| m.selection = sdc_gui::model::Selection::Set("landscape/gone".into()),
            false,
            false,
        ),
        (
            "the Presets tab",
            |m| {
                rock_selected(m);
                m.open_presets();
            },
            false,
            false,
        ),
        (
            "the JSON editor tab",
            |m| {
                rock_selected(m);
                m.open_json_editor();
            },
            false,
            false,
        ),
        (
            "the Settings tab",
            |m| {
                rock_selected(m);
                m.open_settings_tab();
            },
            false,
            false,
        ),
    ];
    let (w, margin) = (1400.0, 8.0);
    for (name, setup, three_d, want_tabs) in cases {
        let d = tempfile::tempdir().unwrap();
        let mut m = pbr_opened_plain(d.path());
        setup(&mut m);
        let mut h = harness_with(m, w, 900.0, three_d);
        if three_d {
            // The 3D view never goes idle.
            for _ in 0..6 {
                h.run_steps(3);
                let (m, s) = h.state_mut();
                s.set3d.poll(&*m);
            }
        } else {
            h.run();
        }
        assert_eq!(set_tabs(&h), want_tabs, "{name}: the set editor's tabs");
        // The pin on `set_editor_shown`: with the selection in front it is true in 2D exactly when the tabs are drawn
        // (it sets the editor's ROWS_MIN floor), and in 3D for the set's view (the 3D settings window).
        let (m, s) = h.state();
        let now_3d = s.set3d.enabled && sdc_gui::ui::set_preview::wanted(m);
        assert_eq!(now_3d, three_d, "{name}: the 3D view");
        if m.centre_front() == sdc_gui::model::CentreTab::Selection {
            assert_eq!(
                sdc_gui::ui::set_editor_shown(m, now_3d),
                set_tabs(&h) || now_3d,
                "{name}: set_editor_shown"
            );
        }
        assert!(
            h.query_by_label("Advanced").is_none(),
            "{name}: an Advanced strip or drawer"
        );
        let c = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
        assert!(
            (c.max.x - (w - margin)).abs() < 0.5,
            "{name}: the centre {c:?} ends at the window edge {}",
            w - margin
        );
        if name == "a stale set" {
            h.get_by_label("This set is no longer in the plan.");
        }
    }
}

/// I1 (final review): a group wider than its column (the Coat colour row) widens the scroll content, and egui unions
/// that into every later header row; each header value must still sit inside the centre (Note FB2: one column).
#[test]
fn header_values_stay_inside_the_panel_with_coat_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    rock_selected(&mut m);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Multilayer (coat)").scroll_to_me();
    h.run();
    h.get_by_label("Multilayer (coat)").click();
    h.run();
    assert!(header_has_no_value(&h, "Coat"));
    h.get_by_label("Emissive").scroll_to_me();
    h.run();
    h.get_by_label("Emissive").click();
    h.run();
    let p = centre(&h);
    // C14: Coat and Emissive settings have no header value; Subsurface is closed at Off here, so it says its mode.
    for t in [
        "Subsurface",
        "NiAlphaProperty",
        "Matching",
        "CS PBR records",
    ] {
        let r = title(&h, t).rect();
        let v = h
            .query_all_by_role(Role::Label)
            .find(|n| {
                n.rect().min.x > r.max.x
                    && (n.rect().center().y - r.center().y).abs() < 2.0
                    && n.value().is_some()
            })
            .unwrap_or_else(|| panic!("no header value beside {t}"));
        assert!(
            v.rect().max.x <= p.max.x,
            "{t}: value at {:?}, panel {p:?}",
            v.rect()
        );
    }
    // Final wave Group 6 (ledger 212): a blocked feature's amber reason ("{state}, blocked by {x}", CC10) still shows
    // in the Coat and Emissive headers once Hair blocks both, and it stays inside the centre too.
    // Written as the key: the Hair tick is itself blocked (by Coat and Emissive) and refuses a click.
    use sdc_gui::pgp::settings::{feature_key, Feature};
    h.state_mut().0.set_json_value(
        "landscape/rock01",
        feature_key(Feature::Hair),
        Some(serde_json::Value::Bool(true)),
    );
    h.run();
    let p = centre(&h);
    let reasons: Vec<egui::Rect> = ["Coat", "Emissive"]
        .iter()
        .flat_map(|t| {
            let r = title(&h, t).rect();
            h.query_all_by_role(Role::Label)
                .filter(move |n| {
                    n.rect().min.x > r.max.x
                        && (n.rect().center().y - r.center().y).abs() < 2.0
                        && n.value().is_some_and(|v| v.contains("blocked by"))
                })
                .map(|n| n.rect())
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        !reasons.is_empty(),
        "premise: a blocked header reason shows"
    );
    for r in reasons {
        assert!(r.max.x <= p.max.x, "blocked reason at {r:?}, panel {p:?}");
    }
}
