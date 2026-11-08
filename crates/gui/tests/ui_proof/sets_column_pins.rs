//! kittest pins for the Sets column polish, Note HN, on the driver's fixtures.
use crate::support::in_sets_column;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use engine::config::RnmTarget as R;
use engine::helper::{AlphaTarget as A, FillTarget as F};
use sdc_gui::model::helpers::{docked, fill_channel, fill_slot};
use sdc_gui::model::items::{helper_target_text, ItemKind};
use sdc_gui::model::labels::subtype_name;
use sdc_gui::model::sets::{self, SlotFill, SlotKind};
use sdc_gui::model::Selection;

#[test]
fn fill_targets_map_to_the_chip_they_fill() {
    for (t, slot, ch) in [
        (F::Alpha(A::Diffuse), Some(SlotKind::Diffuse), "alpha"),
        (F::Alpha(A::N), Some(SlotKind::Normal), "alpha"),
        (F::Alpha(A::F), Some(SlotKind::Fuzz), "alpha"),
        (F::Alpha(A::S), Some(SlotKind::Subsurface), "alpha"),
        (F::Alpha(A::Cnr), Some(SlotKind::Coat), "alpha"),
        (
            F::Packed {
                suffix: "rmaos",
                channel: 0,
            },
            Some(SlotKind::Rmaos),
            "R",
        ),
        (
            F::Packed {
                suffix: "rfaos",
                channel: 3,
            },
            Some(SlotKind::Rmaos),
            "A",
        ),
        (
            F::Packed {
                suffix: "p",
                channel: 0,
            },
            Some(SlotKind::Height),
            "R",
        ),
        (
            F::Packed {
                suffix: "m",
                channel: 1,
            },
            None,
            "G",
        ),
        (F::Rnm(R::Cnr), Some(SlotKind::Coat), "layer"),
        (F::Rnm(R::N), Some(SlotKind::Normal), "layer"),
    ] {
        assert_eq!((fill_slot(t), fill_channel(t)), (slot, ch), "{t:?}");
    }
}

#[test]
fn the_helpers_fixture_has_a_two_chip_helper_a_filled_chip_helper_and_an_unused_one() {
    crate::app_data();
    let d = tempfile::tempdir().unwrap();
    let m = crate::fixtures::lookup("helpers").unwrap()(d.path());
    let rep = |k: &str| m.item(k).unwrap().helper.clone().unwrap();
    assert!(rep("props/crate_roughness.png").skipped_targets.is_empty());
    // sorted: the report order is the engine's
    let mut rough = rep("props/crate_roughness.png").fills.clone();
    rough.sort_by_key(|t| t.id());
    assert_eq!(
        rough,
        [
            F::Alpha(A::Cnr),
            F::Packed {
                suffix: "rmaos",
                channel: 0
            }
        ]
    );
    assert_eq!(rep("props/crate_opacity.png").fills, [F::Alpha(A::Diffuse)]);
    assert_eq!(
        helper_target_text(&rep("props/crate_metallic.png")),
        "not used: no targets chosen"
    );
    let views = sets::views(&m);
    let v = views
        .iter()
        .find(|v| v.id == "props/crate")
        .expect("the crate set");
    for k in [
        "props/crate_roughness.png",
        "props/crate_opacity.png",
        "props/crate_metallic.png",
    ] {
        assert!(v.members.iter().any(|x| x == k), "{k} is a member");
    }
    // Decision 2: the RMAOS chip holds the packed output, which has no source file.
    let Some(SlotFill::Own { item }) = v.slots.get(&SlotKind::Rmaos) else {
        panic!("RMAOS chip")
    };
    assert_eq!(m.item(item).unwrap().kind, ItemKind::Packed);
    let mut got: Vec<_> = docked(&m, v)
        .into_iter()
        .map(|d| (d.helper, d.slot, d.channel))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            ("props/crate_opacity.png".into(), SlotKind::Diffuse, "alpha"),
            ("props/crate_roughness.png".into(), SlotKind::Rmaos, "R"),
            ("props/crate_roughness.png".into(), SlotKind::Coat, "alpha"),
        ]
    );
}

#[test]
fn the_demos_iron_layers_dock_on_its_normal_chip() {
    let (_d, h) = crate::replay("demo", &[]);
    let m = &h.state().model;
    let views = sets::views(m);
    let v = views
        .iter()
        .find(|v| v.id == "armor/iron")
        .expect("iron's set");
    let mut got: Vec<_> = docked(m, v)
        .into_iter()
        .map(|d| (d.helper, d.slot, d.channel))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            ("armor/iron_bump.png".into(), SlotKind::Normal, "layer"),
            (
                "armor/iron_detailnorm.png".into(),
                SlotKind::Normal,
                "layer"
            ),
        ]
    );
}

const SEARCH: &str = r#"{"op":"type","text":"crate","at":{"placeholder":"Search sets"}}"#;

/// The Buttons named `label` inside the Sets column, as rects.
fn buttons(h: &crate::app_harness::H, label: &str) -> Vec<egui::Rect> {
    h.query_all_by_role_and_label(Role::Button, label)
        .map(|n| n.rect())
        .filter(|r| in_sets_column(h, *r))
        .collect()
}

fn one(h: &crate::app_harness::H, label: &str) -> egui::Rect {
    let b = buttons(h, label);
    assert_eq!(b.len(), 1, "one Sets-column Button {label:?}, got {b:?}");
    b[0]
}

/// The painted texts of the frame, flattened.
fn texts(h: &crate::app_harness::H) -> Vec<(String, egui::Rect)> {
    fn flat(s: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| flat(s, out)),
            egui::Shape::Text(t) => out.push((t.galley.job.text.clone(), t.visual_bounding_rect())),
            _ => {}
        }
    }
    let mut out = vec![];
    h.output()
        .shapes
        .iter()
        .for_each(|c| flat(&c.shape, &mut out));
    out
}

/// The packed RMAOS item's key and name on `props/crate`.
fn packed(h: &crate::app_harness::H) -> (String, String) {
    let m = &h.state().model;
    let views = sets::views(m);
    let v = views.iter().find(|v| v.id == "props/crate").unwrap();
    let Some(SlotFill::Own { item }) = v.slots.get(&SlotKind::Rmaos) else {
        panic!("RMAOS chip")
    };
    (item.clone(), m.item(item).unwrap().name.clone())
}

#[test]
fn the_dock_puts_each_badge_in_its_chips_top_cells() {
    for (w, h_px, open) in [(1600.0, 900.0, false), (1055.0, 770.0, true)] {
        let resize = format!(r#"{{"op":"resize","w":{w},"h":{h_px}}}"#);
        let mut lines = vec![resize.as_str(), SEARCH];
        if open {
            lines.push(r#"{"op":"click","at":{"label":"props/crate"}}"#);
        }
        let (_d, h) = crate::replay("helpers", &lines);
        let (_, packed_name) = packed(&h);
        for (badge, chip, letter) in [
            (
                "crate_opacity.png, fills Diffuse alpha".to_string(),
                "D, crate.png, filled by crate_opacity.png".to_string(),
                "D",
            ),
            (
                "crate_roughness.png, fills RMAOS R".to_string(),
                format!("RMAOS, {packed_name}, filled by crate_roughness.png"),
                "RMAOS",
            ),
            (
                "crate_roughness.png, fills Coat _cnr alpha".to_string(),
                "CNR, crate_cnr.png, filled by crate_roughness.png".to_string(),
                "CNR",
            ),
        ] {
            let b = one(&h, &badge);
            let c = one(&h, &chip);
            let at = format!("{badge} at {w}x{h_px}");
            assert!(
                (b.width() - 12.0).abs() <= 0.5 && (b.height() - 12.0).abs() <= 0.5,
                "{at}: 12x12, got {b:?}"
            );
            assert!(c.contains_rect(b.shrink(0.5)), "{at}: {b:?} inside {c:?}");
            assert!(b.top() - c.top() <= 1.5, "{at}: in a top cell");
            assert!(
                c.right() - b.right() <= 1.5,
                "{at}: the first badge is the right cell"
            );
            for (t, r) in texts(&h) {
                if t == letter && c.contains_rect(r.shrink(0.5)) {
                    assert!(
                        !r.intersects(b),
                        "{at}: the letter {t:?} {r:?} meets the badge"
                    );
                }
            }
        }
    }
}

#[test]
fn a_badged_chip_and_its_badges_are_named_for_the_helpers() {
    let (_d, h) = crate::replay("helpers", &[SEARCH]);
    let (_, packed_name) = packed(&h);
    for l in [
        format!("RMAOS, {packed_name}, filled by crate_roughness.png"),
        "CNR, crate_cnr.png, filled by crate_roughness.png".to_string(),
        "D, crate.png, filled by crate_opacity.png".to_string(),
        "crate_roughness.png, fills RMAOS R".to_string(),
        "crate_roughness.png, fills Coat _cnr alpha".to_string(),
        "crate_opacity.png, fills Diffuse alpha".to_string(),
    ] {
        one(&h, &l);
    }
    // a badge's helper is not also in the trailing row
    assert!(buttons(&h, "crate_opacity.png, fills to crate alpha").is_empty());
    let (_d, h) = crate::replay("demo", &[]);
    for l in [
        "N, iron_n.png, filled by iron_bump.png, iron_detailnorm.png",
        "iron_bump.png, fills Normal _n layer",
        "iron_detailnorm.png, fills Normal _n layer",
    ] {
        one(&h, l);
    }
    assert!(buttons(&h, "iron_bump.png").is_empty());
}

#[test]
fn the_trailing_row_names_each_member() {
    let (_d, h) = crate::replay("helpers", &[SEARCH]);
    one(&h, "crate_metallic.png, not used: no targets chosen");
    let (_d, h) = crate::replay("demo", &[]);
    let m = &h.state().model;
    for (key, file) in [
        ("armor/gold_m.png", "gold_m.png"),
        ("architecture/stone/stonewall_m.png", "stonewall_m.png"),
    ] {
        let st = m.item(key).unwrap().job.as_ref().unwrap().profile.subtype;
        one(&h, &format!("{file}, {}", subtype_name(st)));
    }
    let nameless = h
        .query_all_by_role(Role::Unknown)
        .filter(|n| in_sets_column(&h, n.rect()))
        .filter(|n| n.accesskit_node().label().unwrap_or_default().is_empty())
        .count();
    assert_eq!(nameless, 0, "no unnamed node in the Sets column");
}

#[test]
fn right_click_on_a_badge_opens_the_member_menu() {
    let (_d, h) = crate::replay(
        "helpers",
        &[
            SEARCH,
            r#"{"op":"right_click","at":{"label":"crate_opacity.png, fills Diffuse alpha"}}"#,
        ],
    );
    assert!(h.query_by_label("Remove from set").is_some());
    assert!(h.query_by_label("Move to…").is_none());
}

#[test]
fn a_click_on_the_chip_beside_its_badge_selects_the_chip_file() {
    let (_d, mut h) = crate::replay("helpers", &[SEARCH]);
    let (key, name) = packed(&h);
    let label = format!("RMAOS, {name}, filled by crate_roughness.png");
    let c = one(&h, &label);
    h.event(egui::Event::PointerMoved(c.center()));
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: c.center(),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(2);
    assert_eq!(h.state().model.selection, Selection::Item(key));
}

#[test]
fn the_sets_filter_controls_have_their_own_names() {
    let (_d, h) = crate::replay("demo", &[]);
    one(&h, "Sets shader filter");
    let boxes = h
        .query_all_by_role_and_label(Role::TextInput, "Search sets")
        .filter(|n| in_sets_column(&h, n.rect()))
        .count();
    assert_eq!(boxes, 1, "one Search sets box in the Sets column");
    // the Textures column keeps its own button
    let all = h.query_all_by_role_and_label(Role::Button, "Filter ▾");
    assert_eq!(all.filter(|n| !in_sets_column(&h, n.rect())).count(), 1);
    let (_d, h) = crate::replay(
        "demo",
        &[
            r#"{"op":"click","at":{"label":"Sets shader filter"}}"#,
            r#"{"op":"click","at":{"label":"PBR 2"}}"#,
        ],
    );
    let r = one(&h, "Sets shader filter: PBR");
    assert!(
        texts(&h)
            .iter()
            .any(|(t, tr)| t == "Filter (1) ▾" && r.contains_rect(tr.shrink(0.5))),
        "the visible text stays Filter (1) ▾"
    );
}

/// The colour a painted text was drawn with: its section colour, or `None` for the default text colour.
fn text_colour(h: &crate::app_harness::H, text: &str) -> Option<egui::Color32> {
    fn find(s: &egui::Shape, text: &str) -> Option<Option<egui::Color32>> {
        match s {
            egui::Shape::Vec(v) => v.iter().find_map(|s| find(s, text)),
            egui::Shape::Text(t) if t.galley.job.text == text => {
                let c = t.galley.job.sections[0].format.color;
                Some((c != egui::Color32::PLACEHOLDER).then_some(c))
            }
            _ => None,
        }
    }
    h.output()
        .shapes
        .iter()
        .find_map(|c| find(&c.shape, text))
        .unwrap_or_else(|| panic!("no painted {text:?}"))
}

#[test]
fn the_shader_popup_counts_sets_per_shader() {
    let (_d, mut h) = crate::replay(
        "demo",
        &[r#"{"op":"click","at":{"label":"Sets shader filter"}}"#],
    );
    for l in [
        "PBR 2",
        "Default 4",
        "Environment Map 2",
        "Parallax 1",
        "Glow 0",
        "Any",
    ] {
        one(&h, l);
    }
    let weak = h.ctx.global_style().visuals.weak_text_color();
    assert_eq!(text_colour(&h, "Glow 0"), Some(weak));
    assert_ne!(text_colour(&h, "PBR 2"), Some(weak));
    // zero entries stay clickable
    let pos = one(&h, "Glow 0").center();
    h.event(egui::Event::PointerMoved(pos));
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(2);
    assert_eq!(
        h.state().model.sets_filter.shader,
        Some(engine::shader::Shader::Glow)
    );
}

#[test]
fn sets_with_shader_counts_keyed_sets_per_pick() {
    use engine::shader::Shader as S;
    let (_d, mut h) = crate::replay("demo", &[]);
    for (s, n) in [
        (S::Pbr, 2),
        (S::Default, 4),
        (S::Envmap, 2),
        (S::Parallax, 1),
        (S::Glow, 0),
    ] {
        let m = &mut h.state_mut().model;
        assert_eq!(m.sets_with_shader(s), n, "{s:?}");
        m.sets_filter.shader = Some(s);
        assert_eq!(m.visible_sets().len(), n, "{s:?} shows what it counts");
    }
}
