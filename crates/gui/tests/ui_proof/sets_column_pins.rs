//! kittest pins for the Sets column polish, Note HN, on the driver's fixtures.
use crate::support::in_sets_column;
use eframe::egui::{self, accesskit::Role};
use egui::containers::panel::PanelState;
use egui_kittest::kittest::{NodeT, Queryable};
use engine::config::RnmTarget as R;
use engine::helper::{AlphaTarget as A, FillTarget as F};
use sdc_gui::model::helpers::{docked, fill_channel, fill_slot};
use sdc_gui::model::items::{helper_target_text, ItemKind};
use sdc_gui::model::labels::subtype_name;
use sdc_gui::model::sets::{self, SlotFill, SlotKind};
use sdc_gui::model::Selection;
use sdc_gui::ui::columns::{SETS_MIN, TEXTURES_MIN};
use sdc_gui::ui::set_editor::ROWS_MIN;

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

/// The names of the Sets-column Buttons that start with `prefix`.
fn named_from(h: &crate::app_harness::H, prefix: &str) -> Vec<String> {
    h.query_all_by_role(Role::Button)
        .filter(|n| in_sets_column(h, n.rect()))
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.starts_with(prefix))
        .collect()
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

/// The painted `Shape::Rect`s of the frame, flattened.
fn rect_shapes(h: &crate::app_harness::H) -> Vec<egui::epaint::RectShape> {
    fn flat(s: &egui::Shape, out: &mut Vec<egui::epaint::RectShape>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| flat(s, out)),
            egui::Shape::Rect(r) => out.push(r.clone()),
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

fn shared_area(a: egui::Rect, b: egui::Rect) -> f32 {
    let i = a.intersect(b);
    i.width().max(0.0) * i.height().max(0.0)
}

fn near(a: egui::Rect, w: f32, h: f32) -> bool {
    (a.width() - w).abs() <= 0.5 && (a.height() - h).abs() <= 0.5
}

/// The crate card, its badges (Buttons named ", fills "), its chips (other 32x32 Buttons and Labels) and its
/// other interactive parts (combos).
struct CrateCard {
    card: egui::Rect,
    badges: Vec<egui::Rect>,
    chips: Vec<egui::Rect>,
    others: Vec<egui::Rect>,
}

fn crate_card(h: &crate::app_harness::H) -> CrateCard {
    let cards: Vec<_> = h
        .query_all(egui_kittest::kittest::by())
        .filter(|n| n.accesskit_node().raw_bounds().is_some())
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Set props/crate, "))
        })
        .map(|n| n.rect())
        .collect();
    assert_eq!(cards.len(), 1, "one crate card, got {cards:?}");
    let card = cards[0];
    let (mut badges, mut chips, mut others) = (vec![], vec![], vec![]);
    for n in h.query_all(egui_kittest::kittest::by()) {
        if n.accesskit_node().raw_bounds().is_none() {
            continue;
        }
        let r = n.rect();
        if !card.contains_rect(r) || r == card {
            continue;
        }
        let label = n.accesskit_node().label().unwrap_or_default();
        let role = n.accesskit_node().role();
        if role == Role::Button && label.contains(", fills ") {
            badges.push(r);
        } else if matches!(role, Role::Button | Role::Label | Role::Unknown) && near(r, 32.0, 32.0)
        {
            chips.push(r);
        } else if role == Role::ComboBox {
            others.push(r);
        }
    }
    CrateCard {
        card,
        badges,
        chips,
        others,
    }
}

#[test]
fn the_dock_straddles_each_chips_top_corner() {
    for (w, h_px, open, narrow) in [
        (1400.0, 790.0, false, false),
        (1055.0, 770.0, true, false),
        (1055.0, 770.0, true, true),
    ] {
        let resize = resize_line(w, h_px);
        let mut lines = vec![resize.as_str(), SEARCH];
        if open {
            lines.push(r#"{"op":"click","at":{"label":"props/crate"}}"#);
        }
        let (_d, mut h) = crate::replay("helpers", &lines);
        if narrow {
            sets_width(&mut h, SETS_MIN);
        }
        let at = format!("at {w}x{h_px} narrow {narrow}");
        let c = crate_card(&h);
        assert_eq!(c.badges.len(), 3, "{at}: three badges, got {:?}", c.badges);
        if narrow {
            let mut rows: Vec<i32> = c.chips.iter().map(|r| r.top().round() as i32).collect();
            rows.sort();
            rows.dedup();
            assert!(rows.len() >= 2, "{at}: chip rows {rows:?}");
        }
        let shapes = rect_shapes(&h);
        for (i, b) in c.badges.iter().enumerate() {
            let s = format!("{at}: badge {i} {b:?}");
            assert!(
                (18.0 - 0.5..=20.0 + 0.5).contains(&b.width())
                    && (18.0 - 0.5..=20.0 + 0.5).contains(&b.height()),
                "{s}: 18 to 20 px a side"
            );
            assert!(
                c.card.contains_rect(*b),
                "{s}: inside the card {:?}",
                c.card
            );
            let on_corner = |p: &egui::Rect, ch: &egui::Rect| {
                [ch.left_top(), ch.right_top()]
                    .iter()
                    .any(|q| (p.center() - *q).abs().max_elem() <= 0.5)
            };
            let painted = shapes
                .iter()
                .find(|r| {
                    near(r.rect, 16.0, 16.0)
                        && b.expand(0.5).contains_rect(r.rect)
                        && c.chips.iter().any(|ch| on_corner(&r.rect, ch))
                })
                .map(|r| r.rect);
            let Some(painted) = painted else {
                panic!("{s}: holds no painted 16x16 on a chip's top corner");
            };
            // every chip but the one this badge sits on, and the other parts
            let others = c.chips.iter().filter(|ch| !on_corner(&painted, ch));
            for o in others.chain(&c.others) {
                assert!(
                    shared_area(*b, *o) <= 0.5,
                    "{s}: meets {o:?} by {} px2",
                    shared_area(*b, *o)
                );
            }
            for (j, o) in c.badges.iter().enumerate() {
                if j != i {
                    assert!(shared_area(*b, *o) <= 0.5, "{s}: meets badge {j} {o:?}");
                }
            }
        }
    }
}

#[test]
fn a_selected_badge_has_a_two_pixel_selection_stroke() {
    let (_d, mut h) = crate::replay("helpers", &[SEARCH]);
    let badge = one(&h, "crate_roughness.png, fills RMAOS R");
    h.event(egui::Event::PointerMoved(badge.center()));
    h.run_steps(1);
    for pressed in [true, false] {
        press(&mut h, badge.center(), pressed);
    }
    crate::app_harness::settle(&mut h, 6);
    let accent = h.ctx.global_style().visuals.selection.stroke.color;
    let badge = one(&h, "crate_roughness.png, fills RMAOS R");
    let hit = rect_shapes(&h).into_iter().any(|r| {
        near(r.rect, 16.0, 16.0)
            && badge.expand(0.5).contains_rect(r.rect)
            && r.stroke.width == 2.0
            && r.stroke.color == accent
    });
    assert!(hit, "a 2 px selection stroke on the selected badge");
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
        "crate_roughness.png, fills CNR alpha".to_string(),
        "crate_opacity.png, fills D alpha".to_string(),
    ] {
        one(&h, &l);
    }
    // a badge's helper is not also in the trailing row: its only Button is the badge itself
    assert_eq!(
        named_from(&h, "crate_opacity.png, "),
        ["crate_opacity.png, fills D alpha"]
    );
    let (_d, h) = crate::replay("demo", &[]);
    for l in [
        "N, iron_n.png, filled by iron_bump.png, iron_detailnorm.png",
        "iron_bump.png, fills N layer",
        "iron_detailnorm.png, fills N layer",
    ] {
        one(&h, l);
    }
    assert_eq!(
        named_from(&h, "iron_bump.png, "),
        ["iron_bump.png, fills N layer"]
    );
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
            r#"{"op":"right_click","at":{"label":"crate_opacity.png, fills D alpha"}}"#,
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

const OPEN_GOLD: [&str; 3] = [
    r#"{"op":"resize","w":1055,"h":770}"#,
    r#"{"op":"click","at":{"label":"armor/gold"}}"#,
    r#"{"op":"settle","frames":6}"#,
];
const GOLD_CARD: &str = "Set armor/gold, position 5 of 9";

/// Sets the Sets column to width `w`, as a drag leaves it (`<id>/chosen`), and lets the layout follow.
fn sets_width(h: &mut crate::app_harness::H, w: f32) {
    h.ctx
        .data_mut(|d| d.insert_persisted(egui::Id::new("sets").with("chosen"), w));
    crate::app_harness::settle(h, 6);
}

fn narrow_sets(h: &mut crate::app_harness::H) {
    sets_width(h, SETS_MIN);
}

/// (Sets width, Textures width, "Search sets" box width, centre width) of the last frame.
fn widths(h: &crate::app_harness::H) -> (f32, f32, f32, f32) {
    use egui::containers::panel::PanelState;
    let panel = |id: &str| {
        PanelState::load(&h.ctx, egui::Id::new(id))
            .unwrap()
            .outer_rect
    };
    let search = h
        .query_all_by_role(Role::TextInput)
        .find(|n| n.accesskit_node().placeholder() == Some("Search sets"))
        .expect("the Search sets box")
        .rect()
        .width();
    let centre = h
        .ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("centre-rect")))
        .expect("the centre rect")
        .width();
    (
        panel("sets").width(),
        panel("textures").width(),
        search,
        centre,
    )
}

fn resize_line(w: f32, h: f32) -> String {
    format!(r#"{{"op":"resize","w":{w},"h":{h}}}"#)
}

#[test]
fn opening_a_set_takes_the_editors_room_from_textures_first() {
    for (w, h_px) in [(1055.0, 770.0), (1400.0, 790.0)] {
        let resize = resize_line(w, h_px);
        let (_d, mut h) =
            crate::replay("demo", &[resize.as_str(), r#"{"op":"settle","frames":6}"#]);
        let (s0, t0, q0, _) = widths(&h);
        h.get_by_label("armor/gold").click();
        crate::app_harness::settle(&mut h, 6);
        let (s1, t1, q1, c1) = widths(&h);
        let at = format!(
            "at {w}x{h_px}: sets {s0}->{s1}, textures {t0}->{t1}, search {q0}->{q1}, centre {c1}"
        );
        assert!(
            s1 >= s0 - 1.0 && q1 >= q0 - 1.0,
            "{at}: Sets keeps its room"
        );
        assert!(c1 >= ROWS_MIN - 1.0, "{at}: the editor keeps its rows");
        if w < 1100.0 {
            assert!(
                t0 - t1 >= 50.0 && t1 >= TEXTURES_MIN - 0.5,
                "{at}: Textures yields"
            );
        } else {
            assert!((t1 - t0).abs() <= 1.0, "{at}: Textures unchanged");
        }
    }
}

#[test]
fn a_tight_window_takes_textures_to_its_minimum_before_sets() {
    // At 900 the card's title is cut away and the card cannot be driven into view, so open it at 1055 first.
    let (open, tight) = (resize_line(1055.0, 770.0), resize_line(900.0, 770.0));
    let (_d, h) = crate::replay(
        "demo",
        &[
            open.as_str(),
            r#"{"op":"click","at":{"label":"armor/gold"}}"#,
            tight.as_str(),
            r#"{"op":"settle","frames":6}"#,
        ],
    );
    let (s, t, _, c) = widths(&h);
    let at = format!("sets {s}, textures {t}, centre {c}");
    assert!(
        (t - TEXTURES_MIN).abs() <= 1.0,
        "Textures at its minimum: {at}"
    );
    assert!(s >= SETS_MIN - 0.5 && c >= ROWS_MIN - 1.0, "{at}");
}

/// Textures' width at 1055 with armor/gold open, Sets expanded or (`hide`: hidden / collapsed) not.
fn textures_with_gold_open(hidden: bool, collapsed: bool) -> f32 {
    let resize = resize_line(1055.0, 770.0);
    let before = |h: &mut crate::app_harness::H| {
        let s = &mut h.state_mut().model.settings;
        (s.sets_hidden, s.sets_collapsed) = (hidden, collapsed);
    };
    let (_d, mut h) = crate::replay_with(
        "demo",
        before,
        &[resize.as_str(), r#"{"op":"settle","frames":6}"#],
    );
    let key = sets::views(&h.state().model)
        .iter()
        .find(|v| v.id == "armor/gold")
        .unwrap()
        .key
        .clone();
    h.state_mut().model.select_set(&key);
    crate::app_harness::settle(&mut h, 6);
    PanelState::load(&h.ctx, egui::Id::new("textures"))
        .unwrap()
        .outer_rect
        .width()
}

#[test]
fn a_hidden_or_collapsed_sets_column_leaves_textures_only_sets_min_of_room() {
    let open = textures_with_gold_open(false, false);
    for (hidden, collapsed) in [(true, false), (false, true)] {
        let t = textures_with_gold_open(hidden, collapsed);
        assert!(
            t - open >= 100.0,
            "hidden {hidden} collapsed {collapsed}: Textures {t} vs {open} with Sets open"
        );
    }
}

#[test]
fn closing_the_set_gives_textures_its_width_back() {
    let resize = resize_line(1055.0, 770.0);
    let (_d, mut h) = crate::replay("demo", &[resize.as_str(), r#"{"op":"settle","frames":6}"#]);
    let t0 = widths(&h).1;
    h.get_by_label("armor/gold").click();
    crate::app_harness::settle(&mut h, 6);
    assert!(widths(&h).1 < t0 - 50.0, "Textures yielded while open");
    let key = {
        let views = sets::views(&h.state().model);
        let v = views.iter().find(|v| v.id == "armor/gold").unwrap();
        let Some(SlotFill::Own { item }) = v.slots.get(&SlotKind::Diffuse) else {
            panic!("armor/gold's colour chip")
        };
        item.clone()
    };
    h.state_mut().model.select_item(&key);
    crate::app_harness::settle(&mut h, 6);
    let t1 = widths(&h).1;
    assert!((t1 - t0).abs() <= 1.0, "Textures back to {t0}, got {t1}");
}

/// The open card's rect and the Sets panel's outer rect.
fn card_and_panel(h: &crate::app_harness::H) -> (egui::Rect, egui::Rect) {
    let cards: Vec<_> = h.query_all_by_label(GOLD_CARD).map(|n| n.rect()).collect();
    assert_eq!(cards.len(), 1, "one {GOLD_CARD:?} node, got {cards:?}");
    let panel = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("sets"))
        .expect("the Sets panel")
        .outer_rect;
    (cards[0], panel)
}

#[test]
fn opening_a_set_in_a_narrow_window_keeps_its_card_in_view() {
    let (_d, mut h) = crate::replay("demo", &OPEN_GOLD);
    sets_width(&mut h, SETS_MIN);
    let (card, panel) = card_and_panel(&h);
    assert!(
        card.top() >= panel.top() - 0.5 && card.bottom() <= panel.bottom() + 0.5,
        "card {card:?} inside the Sets panel {panel:?}"
    );
}

/// Pins that a user scroll after the reveal is not undone: the reveal fires on a width change only. It scrolls up
/// because the reveal leaves the card in view, so a scroll down never left the panel and pinned nothing.
#[test]
fn a_wheel_scroll_after_the_reveal_stays_put() {
    let scroll =
        r#"{"op":"scroll","at":{"label":"Set armor/gold, position 5 of 9"},"dir":"up","times":8}"#;
    let top_after = |frames: &str| {
        let (_d, h) = crate::replay_mid("demo", |_| {}, &OPEN_GOLD, narrow_sets, &[scroll, frames]);
        card_and_panel(&h).0.top()
    };
    let before = {
        let (_d, h) = crate::replay_mid("demo", |_| {}, &OPEN_GOLD, narrow_sets, &[]);
        card_and_panel(&h).0.top()
    };
    let scrolled = top_after(r#"{"op":"settle","frames":2}"#);
    let later = top_after(r#"{"op":"settle","frames":8}"#);
    assert!(
        (scrolled - before).abs() > 1.0,
        "the wheel moved the card: {before} -> {scrolled}"
    );
    assert!(
        (later - scrolled).abs() < 0.5,
        "the card stays put: {scrolled} -> {later}"
    );
}

/// The painted Text shape whose galley text is `text`: (galley rows, elided, visual rect).
fn painted(h: &crate::app_harness::H, text: &str) -> (usize, bool, egui::Rect) {
    fn find(s: &egui::Shape, text: &str) -> Option<(usize, bool, egui::Rect)> {
        match s {
            egui::Shape::Vec(v) => v.iter().find_map(|s| find(s, text)),
            egui::Shape::Text(t) if t.galley.job.text == text => Some((
                t.galley.rows.len(),
                t.galley.elided,
                t.visual_bounding_rect(),
            )),
            _ => None,
        }
    }
    h.output()
        .shapes
        .iter()
        .find_map(|c| find(&c.shape, text))
        .unwrap_or_else(|| panic!("no painted {text:?}"))
}

fn press(h: &mut crate::app_harness::H, pos: egui::Pos2, pressed: bool) {
    h.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    });
}

#[test]
fn the_hidden_set_line_wraps_with_clear_filter_below() {
    for (w, h_px) in [(1400.0, 790.0), (1055.0, 770.0)] {
        let resize = format!(r#"{{"op":"resize","w":{w},"h":{h_px}}}"#);
        // Opening a set no longer narrows Sets at 1055, so the narrow case sets the width itself.
        let narrow = |h: &mut crate::app_harness::H| {
            if w < 1100.0 {
                narrow_sets(h);
            }
        };
        let (_d, h) = crate::replay_mid(
            "demo",
            |_| {},
            &[
                resize.as_str(),
                r#"{"op":"click","at":{"label":"armor/gold"}}"#,
            ],
            narrow,
            &[r#"{"op":"click","at":{"starts":"Not ready"}}"#],
        );
        let at = format!("at {w}x{h_px}");
        let (rows, elided, rect) = painted(&h, "armor/gold is hidden by the Sets filter");
        assert!(!elided, "{at}: the line is not elided");
        assert!(in_sets_column(&h, rect), "{at}: {rect:?} inside the column");
        if w < 1100.0 {
            assert!(rows >= 2, "{at}: wraps to {rows} rows");
        }
        let clear = one(&h, "Clear filter");
        assert!(
            clear.min.y >= rect.max.y - 0.5,
            "{at}: Clear filter {clear:?} below the line {rect:?}"
        );
    }
}

#[test]
fn the_drop_hint_is_weak_text_and_the_accent_under_a_drag() {
    const HINT: &str = "Drop here to start a new set";
    let (_d, mut h) = crate::replay(
        "demo",
        &[r#"{"op":"type","text":"body","at":{"placeholder":"Search sets"}}"#],
    );
    let vis = h.ctx.global_style().visuals.clone();
    assert_eq!(text_colour(&h, HINT), Some(vis.weak_text_color()));
    let chip = h
        .query_all_by_role(Role::Button)
        .find(|n| {
            in_sets_column(&h, n.rect())
                && n.accesskit_node()
                    .label()
                    .is_some_and(|l| l == "D" || l.starts_with("D, "))
        })
        .expect("a D chip in the Sets column")
        .rect()
        .center();
    let hint = painted(&h, HINT).2.center();
    h.event(egui::Event::PointerMoved(chip));
    h.run_steps(1);
    press(&mut h, chip, true);
    h.run_steps(1);
    let steps = 4;
    for i in 1..=steps {
        h.event(egui::Event::PointerMoved(
            chip + (hint - chip) * (i as f32 / steps as f32),
        ));
        h.run_steps(1);
    }
    h.run_steps(2);
    let under = text_colour(&h, HINT);
    // release outside the column so nothing is dropped
    let out = egui::pos2(5.0, 5.0);
    h.event(egui::Event::PointerMoved(out));
    press(&mut h, out, false);
    h.run_steps(2);
    assert_eq!(
        under,
        Some(vis.selection.stroke.color),
        "accent under a drag"
    );
}

#[test]
fn card_titles_keep_their_leaf() {
    // 1400 wide the titles may fit; 1055 with gold open the column narrows and they must be cut
    keep_leaf_at(1400, 790, false, &[0]);
    keep_leaf_at(1055, 770, true, &[0, 6]);
}

/// Checks the stone cards' titles at one window size; `scrolls` are the wheel steps down from the top.
fn keep_leaf_at(w: u32, h_px: u32, open: bool, scrolls: &[u32]) {
    let ids = [
        ("architecture/stone/banner", "banner"),
        ("architecture/stone/cobble", "cobble"),
        ("architecture/stone/stonewall", "stonewall"),
    ];
    let mut checked = vec![];
    for &down in scrolls {
        let resize = format!(r#"{{"op":"resize","w":{w},"h":{h_px}}}"#);
        let wheel = format!(
            r#"{{"op":"scroll","at":{{"label":"Set architecture/stone/banner, position 2 of 9"}},"dir":"down","times":{down}}}"#
        );
        let (mut first, mut rest) = (vec![resize.as_str()], vec![]);
        if open {
            // no search here: it would hide the open card; open it, narrow Sets, scroll to the top, then down
            first.extend(OPEN_GOLD);
            rest.push(
                r#"{"op":"scroll","at":{"label":"Set armor/gold, position 5 of 9"},"dir":"up","times":8}"#,
            );
            if down > 0 {
                rest.push(&wheel);
            }
        } else {
            first.push(r#"{"op":"type","text":"stone","at":{"placeholder":"Search sets"}}"#);
        }
        rest.push(r#"{"op":"settle","frames":6}"#);
        // Opening a set no longer narrows Sets at 1055, so the narrow case sets the width itself.
        let narrow = |h: &mut crate::app_harness::H| {
            if open {
                narrow_sets(h);
            }
        };
        let (_d, mut h) = crate::replay_mid("demo", |_| {}, &first, narrow, &rest);
        // the list starts under the filter row
        let list_top = texts(&h)
            .into_iter()
            .filter(|(t, r)| t.starts_with("Filter") && in_sets_column(&h, *r))
            .map(|(_, r)| r.max.y)
            .fold(0.0, f32::max);
        let panel_bottom = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("sets"))
            .expect("the Sets panel")
            .outer_rect
            .bottom();
        for (id, leaf) in ids {
            let Some(title) = h
                .query_all_by_label(id)
                .map(|n| n.rect())
                .find(|r| in_sets_column(&h, *r) && r.min.y > list_top && r.max.y < panel_bottom)
            else {
                continue;
            };
            checked.push(id);
            // the painted title: a text inside the node's rect that ends with the leaf
            let shown: Vec<_> = texts(&h)
                .into_iter()
                .filter(|(t, r)| title.expand(1.0).contains_rect(*r) && t.ends_with(leaf))
                .collect();
            assert_eq!(
                shown.len(),
                1,
                "{id} at {w}x{h_px}: one painted title ending {leaf:?}, got {shown:?}"
            );
            assert!(!painted(&h, &shown[0].0).1, "{id}: not elided");
            h.event(egui::Event::PointerMoved(title.center()));
            h.run();
            let full = texts(&h).into_iter().filter(|(t, _)| t == id).count();
            // the title shows the full id only when it fits; otherwise the tooltip does
            assert!(full >= 1, "{id}: shown in full as the title or its tooltip");
            let cut = shown[0].0 != id;
            assert!(cut || !open, "{id} at {w}x{h_px}: cut in the narrow column");
            if cut {
                assert_eq!(full, 1, "{id}: the tooltip is exactly the full id");
                let again = texts(&h).iter().filter(|(t, _)| *t == shown[0].0).count();
                assert_eq!(again, 1, "{id}: no second hover line of the cut text");
            }
        }
    }
    checked.sort();
    checked.dedup();
    assert_eq!(
        checked.len(),
        3,
        "every stone title was on screen once: {checked:?}"
    );
}

/// Note HP: one wording for a fill everywhere: the badge, its hover, the helper panel's subtitle and its Stop button.
#[test]
fn one_fill_has_one_wording() {
    for (w, h_px) in [(1400.0, 790.0), (1055.0, 770.0)] {
        let resize = resize_line(w, h_px);
        let (_d, mut h) = crate::replay("helpers", &[resize.as_str(), SEARCH]);
        let at = format!("at {w}x{h_px}");
        for l in [
            "crate_opacity.png, fills D alpha",
            "crate_roughness.png, fills RMAOS R",
            "crate_roughness.png, fills CNR alpha",
        ] {
            one(&h, l);
        }
        let badge = one(&h, "crate_roughness.png, fills RMAOS R");
        h.event(egui::Event::PointerMoved(badge.center()));
        h.run();
        let hover = "crate_roughness.png\nfills CNR alpha  crate_cnr, RMAOS R  crate_rmaos";
        assert!(texts(&h).iter().any(|(t, _)| t == hover), "{at}: hover");
        for p in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: badge.center(),
                button: egui::PointerButton::Primary,
                pressed: p,
                modifiers: egui::Modifiers::NONE,
            });
        }
        crate::app_harness::settle(&mut h, 6);
        let sub = "fills CNR alpha  crate_cnr, RMAOS R  crate_rmaos";
        let weak = h.ctx.global_style().visuals.weak_text_color();
        let job = painted_job(&h, sub);
        let sec = job.sections.last().unwrap();
        assert_eq!(
            &job.text[sec.byte_range.start.0..sec.byte_range.end.0],
            "crate_rmaos",
            "{at}"
        );
        assert_eq!(sec.format.color, weak, "{at}: the file is weak");
        h.get_by_role_and_label(Role::Button, "Stop filling RMAOS R");
        for (t, _) in texts(&h) {
            assert!(!t.contains("fills to"), "{at}: {t:?}");
        }
        for n in h.query_all(egui_kittest::kittest::by()) {
            let l = n.accesskit_node().label().unwrap_or_default();
            assert!(!l.contains("fills to"), "{at}: node {l:?}");
        }
    }
}

/// The painted text shape's layout job whose text is `text`.
fn painted_job(h: &crate::app_harness::H, text: &str) -> std::sync::Arc<egui::text::LayoutJob> {
    fn find(s: &egui::Shape, text: &str) -> Option<std::sync::Arc<egui::text::LayoutJob>> {
        match s {
            egui::Shape::Vec(v) => v.iter().find_map(|s| find(s, text)),
            egui::Shape::Text(t) if t.galley.job.text == text => Some(t.galley.job.clone()),
            _ => None,
        }
    }
    h.output()
        .shapes
        .iter()
        .find_map(|c| find(&c.shape, text))
        .unwrap_or_else(|| panic!("no painted {text:?}"))
}

fn contrast(a: egui::Color32, b: egui::Color32) -> f32 {
    let lum = |c: egui::Color32| {
        let f = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[test]
fn the_status_pill_text_reads() {
    let (_d, mut h) = crate::replay("demo", &[]);
    let fill = h.ctx.global_style().visuals.panel_fill;
    // None is the frame's default text colour (gray 140 today).
    let c = text_colour(&h, "vanilla").unwrap_or(egui::Color32::from_gray(140));
    for bg in [fill, egui::Color32::from_gray(60)] {
        let r = contrast(c, bg);
        assert!(r >= 4.5, "dark: {c:?} on {bg:?} is {r}:1");
    }
    h.ctx.set_visuals(egui::Visuals::light());
    crate::app_harness::settle(&mut h, 3);
    let c = text_colour(&h, "vanilla").unwrap_or(h.ctx.global_style().visuals.text_color());
    let fill = h.ctx.global_style().visuals.panel_fill;
    let r = contrast(c, fill);
    assert!(r >= 4.5, "light: {c:?} on {fill:?} is {r}:1");
}

/// Every drawn card: (its rect, its shader combo's rect, name, value).
fn card_combos(h: &crate::app_harness::H) -> Vec<(egui::Rect, egui::Rect, String, String)> {
    let cards: Vec<egui::Rect> = h
        .query_all(egui_kittest::kittest::by())
        .filter(|n| n.accesskit_node().raw_bounds().is_some())
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Set ") && l.contains(", position "))
        })
        .map(|n| n.rect())
        .filter(|r| in_sets_column(h, *r))
        .collect();
    let mut out = vec![];
    for n in h.query_all_by_role(Role::ComboBox) {
        let r = n.rect();
        if let Some(c) = cards.iter().find(|c| c.contains_rect(r)) {
            out.push((
                *c,
                r,
                n.accesskit_node().label().unwrap_or_default(),
                n.accesskit_node().value().unwrap_or_default(),
            ));
        }
    }
    out
}

#[test]
fn the_card_combo_is_named_and_stays_in_the_right_half() {
    for (w, h_px, narrow) in [(1400.0, 790.0, false), (1055.0, 770.0, true)] {
        let resize = resize_line(w, h_px);
        let (_d, mut h) = crate::replay("demo", &[resize.as_str()]);
        if narrow {
            sets_width(&mut h, SETS_MIN);
        }
        let at = format!("at {w}x{h_px}");
        let combos = card_combos(&h);
        assert!(!combos.is_empty(), "{at}: some cards are drawn");
        for (card, c, name, _) in &combos {
            assert!(!name.is_empty(), "{at}: a combo in {card:?} has no name");
            assert!(
                c.min.x >= card.center().x - 0.5
                    && c.max.x <= card.max.x + 0.5
                    && c.max.x >= card.max.x - 8.0,
                "{at}: {name} {c:?} sits in the right half of {card:?}, flush right"
            );
            let title = texts(&h)
                .into_iter()
                .filter(|(_, r)| card.contains_rect(*r) && r.max.y < c.center().y)
                .map(|(_, r)| r.max.y)
                .fold(f32::MIN, f32::max);
            assert!(title <= c.min.y + 0.5, "{at}: {name} under the title row");
        }
        for (id, value) in [
            ("actors/body", "Default (auto)"),
            ("architecture/stone/stonewall", "Environment Map (auto)"),
            ("architecture/stone/cobble", "PBR"),
        ] {
            let name = format!("Shader for {id}");
            let hit: Vec<_> = combos.iter().filter(|c| c.2 == name).collect();
            // The narrow column shows fewer cards: only the drawn ones are checked.
            assert!(
                hit.len() <= 1 && (narrow || hit.len() == 1),
                "{at}: {name:?}"
            );
            if let Some(c) = hit.first() {
                assert_eq!(c.3, value, "{at}: {name} value");
            }
        }
        if !narrow {
            let (_, elided, _) = painted(&h, "Default (auto)");
            assert!(!elided, "{at}: Default (auto) is whole");
        }
    }
}

/// Releases and presses nothing but the pointer at `pos`, then settles.
fn tap(h: &mut crate::app_harness::H, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    press(h, pos, true);
    press(h, pos, false);
    crate::app_harness::settle(h, 3);
}

#[test]
fn a_click_on_a_cards_middle_opens_the_set() {
    let (_d, h) = crate::replay(
        "demo",
        &[
            r#"{"op":"click","at":{"label":"Set actors/body, position 1 of 9"}}"#,
            r#"{"op":"settle","frames":3}"#,
            r#"{"op":"expect","fact":"/selection","equals":"set:actors/body"}"#,
        ],
    );
    assert!(!egui::Popup::is_any_open(&h.ctx), "no popup opened");
}

/// The rect of the one node whose label starts with `prefix`.
fn one_node(h: &crate::app_harness::H, prefix: &str) -> egui::Rect {
    let r: Vec<_> = h
        .query_all(egui_kittest::kittest::by())
        .filter(|n| n.accesskit_node().raw_bounds().is_some())
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with(prefix))
        })
        .map(|n| n.rect())
        .collect();
    assert_eq!(r.len(), 1, "one node {prefix:?}, got {r:?}");
    r[0]
}

#[test]
fn a_click_left_of_the_combo_opens_the_set() {
    let (_d, mut h) = crate::replay("demo", &[]);
    let card = one_node(&h, "Set architecture/stone/stonewall, position ");
    let combo = h
        .query_all_by_role(Role::ComboBox)
        .map(|n| n.rect())
        .find(|r| card.contains_rect(*r))
        .expect("the stonewall card's combo");
    tap(&mut h, egui::pos2(card.left() + 12.0, combo.center().y));
    let Selection::Set(key) = &h.state().model.selection else {
        panic!("a set is selected, got {:?}", h.state().model.selection);
    };
    assert!(key.ends_with("architecture/stone/stonewall"), "{key}");
    assert!(!egui::Popup::is_any_open(&h.ctx), "no popup opened");
}

#[test]
fn a_top_badge_is_not_trimmed_by_the_combo() {
    let (_d, h) = crate::replay("helpers", &[resize_line(1400.0, 790.0).as_str(), SEARCH]);
    let c = crate_card(&h);
    let tall = c.badges.iter().filter(|b| b.height() >= 19.5).count();
    assert!(tall >= 1, "a badge keeps its 20 px hit: {:?}", c.badges);
}

/// Note HP SC5: each pill not yet on, clicked, shows as many sets as its name says; clicking again restores the list.
fn pills_show_their_counts(h: &mut crate::app_harness::H, at: &str) {
    for prefix in ["Not ready ", "Ready ", "No JSON "] {
        let names = named_from(h, prefix);
        assert_eq!(names.len(), 1, "{at}: one {prefix:?} pill, got {names:?}");
        let on = |h: &crate::app_harness::H| {
            let f = &h.state().model.sets_filter;
            match prefix {
                "Not ready " => f.attention,
                "Ready " => f.ready,
                _ => f.no_json,
            }
        };
        if on(h) {
            continue;
        }
        let n: usize = names[0][prefix.len()..].parse().unwrap();
        let before = h.state().model.visible_sets().len();
        let pos = one(h, &names[0]).center();
        tap(h, pos);
        assert!(on(h), "{at}: {prefix:?} clicked on");
        assert_eq!(
            h.state().model.visible_sets().len(),
            n,
            "{at}: {} shows what it counts",
            names[0]
        );
        let now = named_from(h, prefix);
        let pos = one(h, &now[0]).center();
        tap(h, pos);
        assert!(!on(h), "{at}: {prefix:?} clicked off");
        assert_eq!(
            h.state().model.visible_sets().len(),
            before,
            "{at}: restored"
        );
    }
}

#[test]
fn each_pill_shows_exactly_its_count() {
    for (w, h_px) in [(1400.0, 790.0), (1055.0, 770.0)] {
        let resize = resize_line(w, h_px);
        let at = format!("at {w}x{h_px}");
        let (_d, mut h) = crate::replay("helpers", &[resize.as_str(), SEARCH]);
        one(&h, "Not ready 1");
        one(&h, "Ready 0");
        let weak = h.ctx.global_style().visuals.weak_text_color();
        assert_eq!(text_colour(&h, "Ready 0"), Some(weak), "{at}: zero is weak");
        pills_show_their_counts(&mut h, &at);

        let (_d, mut h) = crate::replay(
            "demo",
            &[
                resize.as_str(),
                r#"{"op":"click","at":{"label":"Sets shader filter"}}"#,
                r#"{"op":"click","at":{"label":"PBR 2"}}"#,
            ],
        );
        pills_show_their_counts(&mut h, &format!("{at} PBR"));

        let (_d, mut h) = crate::replay("demo", &[resize.as_str()]);
        let n = named_from(&h, "Not ready ");
        let pos = one(&h, &n[0]).center();
        tap(&mut h, pos);
        assert!(h.state().model.sets_filter.attention);
        pills_show_their_counts(&mut h, &format!("{at} Not ready on"));
    }
}

#[test]
fn the_shader_popup_counts_follow_the_search() {
    for (w, h_px) in [(1400.0, 790.0), (1055.0, 770.0)] {
        let resize = resize_line(w, h_px);
        let (_d, mut h) = crate::replay(
            "demo",
            &[
                resize.as_str(),
                r#"{"op":"type","text":"stone","at":{"placeholder":"Search sets"}}"#,
                r#"{"op":"click","at":{"label":"Sets shader filter"}}"#,
            ],
        );
        for l in [
            "PBR 1",
            "Default 1",
            "Environment Map 1",
            "Parallax 0",
            "Glow 0",
        ] {
            one(&h, l);
        }
        let pos = one(&h, "PBR 1").center();
        tap(&mut h, pos);
        assert_eq!(h.state().model.visible_sets().len(), 1, "at {w}x{h_px}");
    }
}

/// A filter change starts the list at its top: the old offset showed a set under the "hidden by filter" line (SC3).
#[test]
fn a_filter_change_starts_the_list_at_its_top() {
    for (w, h_px) in [(1055.0, 770.0), (1400.0, 790.0), (1055.0, 400.0)] {
        let resize = resize_line(w, h_px);
        let open = resize_line(w, 790.0);
        let (_d, h) = crate::replay_mid(
            "demo",
            |_| {},
            &[
                open.as_str(),
                r#"{"op":"click","at":{"label":"armor/gold"}}"#,
            ],
            |_| {},
            &[
                resize.as_str(),
                r#"{"op":"scroll","at":{"label":"Set armor/gold, position 5 of 9"},"dir":"down","times":5}"#,
                r#"{"op":"settle","frames":2}"#,
                r#"{"op":"click","at":{"starts":"Not ready"}}"#,
                r#"{"op":"settle","frames":3}"#,
            ],
        );
        let clear = one(&h, "Clear filter");
        let first = one_node(&h, "Set architecture/stone/stonewall, position");
        assert!(
            first.min.y >= clear.max.y - 0.5 && in_sets_column(&h, first),
            "at {w}x{h_px}: the first set {first:?} sits below Clear filter {clear:?} inside the column"
        );
    }
}

/// The one open set that still passes the new filter is revealed instead of the top.
#[test]
fn a_filter_change_reveals_the_open_set_when_it_stays_visible() {
    for (w, h_px) in [(1055.0, 770.0), (1400.0, 790.0), (1055.0, 560.0)] {
        let resize = resize_line(w, h_px);
        let open = resize_line(w, 790.0);
        let (_d, h) = crate::replay_mid(
            "demo",
            |_| {},
            &[
                open.as_str(),
                r#"{"op":"click","at":{"label":"armor/gold"}}"#,
            ],
            |_| {},
            &[
                resize.as_str(),
                r#"{"op":"scroll","at":{"label":"Set armor/gold, position 5 of 9"},"dir":"down","times":5}"#,
                r#"{"op":"settle","frames":2}"#,
                r#"{"op":"click","at":{"starts":"Ready"}}"#,
                r#"{"op":"settle","frames":6}"#,
            ],
        );
        let card = one_node(&h, "Set armor/gold, position");
        let panel = PanelState::load(&h.ctx, egui::Id::new("sets"))
            .expect("the Sets panel")
            .outer_rect;
        assert!(
            card.top() >= panel.top() - 0.5 && card.bottom() <= panel.bottom() + 0.5,
            "at {w}x{h_px}: the open card {card:?} inside the Sets panel {panel:?}"
        );
    }
}

/// Review Focus 2: typing a search after a scroll starts at the top, and a later wheel scroll is not undone.
#[test]
fn typing_a_search_after_a_scroll_starts_at_the_top() {
    const BANNER: &str = "Set architecture/stone/banner, position";
    for (w, h_px) in [(1055.0, 770.0), (1055.0, 400.0)] {
        let resize = resize_line(w, h_px);
        let typed = [
            resize.as_str(),
            r#"{"op":"scroll","at":{"label":"Set actors/body, position 1 of 9"},"dir":"down","times":5}"#,
            r#"{"op":"type","text":"stone","at":{"placeholder":"Search sets"}}"#,
            r#"{"op":"settle","frames":3}"#,
        ];
        let (_d, h) = crate::replay("demo", &typed);
        let search = h
            .query_all_by_role(Role::TextInput)
            .map(|n| n.rect())
            .find(|r| in_sets_column(&h, *r))
            .expect("the search box");
        let banner = one_node(&h, BANNER);
        assert!(
            banner.min.y >= search.max.y,
            "the first set {banner:?} starts below the search box {search:?}"
        );
        let top_after = |settle: &str| {
            let mut lines = typed.to_vec();
            lines.push(
            r#"{"op":"scroll","at":{"label":"Set architecture/stone/banner, position 2 of 9"},"dir":"down","times":2}"#,
        );
            lines.push(r#"{"op":"settle","frames":2}"#);
            lines.push(settle);
            let (_d, h) = crate::replay("demo", &lines);
            one_node(&h, BANNER).top()
        };
        let scrolled = top_after(r#"{"op":"settle","frames":1}"#);
        let later = top_after(r#"{"op":"settle","frames":6}"#);
        assert!(
            (later - scrolled).abs() < 0.5,
            "the wheel scroll stays put: {scrolled} -> {later}"
        );
    }
}
