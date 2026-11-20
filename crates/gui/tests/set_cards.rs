//! Layout v2 A5 (R15): the set cards in the Sets column. One row for the handle, the name, the status pill and the
//! ⚠ button; a full-width shader; slot chips labelled by their suffix letter.
mod support;
use eframe::egui;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::Subtype;
use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::sets::{set_views, slot_row, view_row, views, SlotCommand, SlotFill, SlotKind};
use sdc_gui::model::AppModel;
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01";

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(0.02)
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h
}

fn shapes(h: &Harness<'static, AppModel>) -> Vec<egui::Shape> {
    fn flat(s: &egui::Shape, out: &mut Vec<egui::Shape>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| flat(s, out)),
            s => out.push(s.clone()),
        }
    }
    let mut out = vec![];
    for c in &h.output().shapes {
        flat(&c.shape, &mut out);
    }
    out
}

/// rock01's name on its card.
fn name_rect(h: &Harness<'static, AppModel>) -> egui::Rect {
    name_of(h, ROCK)
}

/// Set `id`'s name on its card: the leftmost label (the Sets column is left of the set editor).
fn name_of(h: &Harness<'static, AppModel>, id: &str) -> egui::Rect {
    h.query_all_by_label(id)
        .map(|n| n.rect())
        .min_by(|a, b| a.min.x.total_cmp(&b.min.x))
        .unwrap_or_else(|| panic!("{id}'s card name"))
}

/// rock01's card.
fn card_rect(h: &Harness<'static, AppModel>) -> egui::Rect {
    card_of(h, ROCK)
}

/// A card: the smallest painted rect holding the set's name and the chip row below it.
fn card_of(h: &Harness<'static, AppModel>, id: &str) -> egui::Rect {
    let name = name_of(h, id);
    shapes(h)
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Rect(r) => Some(r.rect),
            _ => None,
        })
        .filter(|r| r.contains_rect(name) && r.height() > name.height() + 40.0)
        .min_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("the card's frame")
}

/// The nodes labelled `label` inside rock01's card (K11: every card has a "D").
fn on_card<'a>(h: &'a Harness<'static, AppModel>, label: &'a str) -> Vec<egui_kittest::Node<'a>> {
    on_card_of(h, ROCK, label)
}

fn on_card_of<'a>(
    h: &'a Harness<'static, AppModel>,
    id: &str,
    label: &'a str,
) -> Vec<egui_kittest::Node<'a>> {
    let card = card_of(h, id);
    h.query_all_by_label(label)
        .filter(|n| card.contains_rect(n.rect()))
        .collect()
}

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

#[test]
fn the_idle_sets_column_checks_bulk_membership_once_for_each_visible_summary_and_banner() {
    let d = tempfile::tempdir().unwrap();
    let mut m = many_sets(d.path(), 64);
    let cards = views(&m);
    m.click_set(&cards[0].key, false, false);
    m.click_set(&cards[62].key, false, true);
    m.click_set(&cards[63].key, true, false);
    assert_eq!(m.selected_sets().last(), Some(&cards[63].key));
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(440.0, 900.0))
        .build_ui_state(
            |ui, (m, state): &mut (AppModel, sdc_gui::ui::State)| {
                sdc_gui::ui::sets_column::draw(
                    ui,
                    m,
                    &mut state.thumbs,
                    &mut state.drops,
                    &mut state.slot_cmds,
                );
            },
            (m, sdc_gui::ui::State::default()),
        );
    h.run();
    h.get_by_label("64 selected");
    let before = h.state().0.bulk_membership_scans.get();
    h.run_steps(5);
    let scans = h.state().0.bulk_membership_scans.get() - before;
    assert!(
        scans <= 10,
        "five idle frames scanned bulk membership {scans} times"
    );
}

#[test]
fn a_card_shows_name_pill_and_a_right_aligned_shader() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    h.run();
    // The default column is too narrow for half a card to hold "PBR Landscape (auto)": widen it, as a drag leaves it.
    h.ctx.data_mut(|d| {
        d.insert_persisted(sdc_gui::ui::panel_ids::chosen_width_id("sets"), 460.0_f32)
    });
    h.run();
    let card = card_rect(&h);
    let kind = on_card(&h, "PBR Landscape");
    assert_eq!(kind.len(), 1, "the kind text");
    let (name, kind) = (name_rect(&h), kind[0].rect());
    // Note FI13: the heading is one row only when the name keeps 80 px beside the pill and badges. rock01's pill and
    // two badges do not leave that at the default width, so the pill is on the row below; either way the name is whole.
    assert!(
        name.width() >= 80.0 && (kind.min.y > name.max.y || kind.min.x > name.max.x),
        "the name keeps its room at the default width: the pill {kind:?}, the name {name:?}"
    );
    let combo = h
        .query_all_by_role(egui::accesskit::Role::ComboBox)
        .find(|n| card.contains_rect(n.rect()))
        .expect("the card's shader");
    assert_eq!(
        combo.accesskit_node().value().as_deref(),
        Some("PBR Landscape (auto)")
    );
    let w = combo.rect().width();
    assert!(
        w < card.width() / 2.0,
        "the shader ({w}) is under half the card ({})",
        card.width()
    );
    assert!(
        combo.rect().min.y >= kind.max.y - 0.5,
        "the shader row is under the pill"
    );
    assert!(
        (card.max.x - combo.rect().max.x).abs() <= 8.0,
        "the shader's right edge {} is the card's inner right {}",
        combo.rect().max.x,
        card.max.x
    );
    let texts: Vec<_> = shapes(&h)
        .into_iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) if t.galley.job.text.starts_with("PBR Landscape (") => Some(t),
            _ => None,
        })
        .collect();
    assert!(!texts.is_empty(), "the shader's text is painted");
    assert!(
        texts.iter().all(|t| !t.galley.elided),
        "the shader's text is not cut to \"PBR Landsc…\""
    );
}

#[test]
fn slot_chips_are_labelled_and_empty_ones_are_dim() {
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened_plain(d.path());
    // A4: the plain PBR shader uses Height and the fixture has no `_p`.
    let v = set_views(&m).into_iter().find(|v| v.id == ROCK).unwrap();
    let row = slot_row(v.shader.as_ref(), &v.slots);
    assert!(
        row.iter()
            .any(|c| c.kind == SlotKind::Height && c.fill == SlotFill::Empty),
        "{row:?}"
    );
    let mut h = harness(m);
    h.run();
    // A chip with docked helpers is named "<letter>, <file>, filled by <helpers>" (Note HN).
    let card = card_of(&h, ROCK);
    for l in ["D", "N", "RMAOS", "P"] {
        let prefix = format!("{l}, ");
        let named = h
            .query_all_by_label_contains(&prefix)
            .filter(|n| card.contains_rect(n.rect()))
            .filter(|n| {
                n.accesskit_node()
                    .label()
                    .unwrap_or_default()
                    .starts_with(&prefix)
            })
            .count();
        // an empty chip is named "<letter>, empty" (Note HP)
        let empty = format!("{l}, empty");
        let n = on_card(&h, l).len() + on_card(&h, &empty).len() + named;
        assert_eq!(n, 1, "one {l:?} chip on rock01's card");
    }
    assert!(h.query_by_label_contains("Height _p (empty)").is_none());
    on_card(&h, "P, empty")[0].hover();
    h.run();
    h.run();
    h.get_by_label("Height _p (empty)");
}

#[test]
fn the_open_set_is_highlighted() {
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let key = set_views(&m)
        .into_iter()
        .find(|v| v.id == ROCK)
        .unwrap()
        .key;
    let mut h = harness(m);
    let fill = h
        .ctx
        .global_style()
        .visuals
        .selection
        .bg_fill
        .gamma_multiply(0.35);
    let lit = |h: &Harness<'static, AppModel>| {
        let name = name_rect(h);
        shapes(h).iter().any(
            |s| matches!(s, egui::Shape::Rect(r) if r.fill == fill && r.rect.contains_rect(name)),
        )
    };
    h.run();
    assert!(!lit(&h), "a set that is not open is not highlighted");
    h.state_mut().select_set(&key);
    h.run();
    assert!(
        lit(&h),
        "the open set's card is filled with the selection colour"
    );
}

#[test]
fn a_chip_keeps_the_member_behaviours() {
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let diffuse = m
        .items
        .iter()
        .find(|i| i.name == "rock01.png")
        .unwrap()
        .key
        .clone();
    let mut h = harness(m);
    h.run();
    let at = on_card(&h, "D")[0].rect().center();
    h.hover_at(at);
    h.run();
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
        }
    }
    h.run();
    assert_eq!(h.state().expanded.as_deref(), Some(diffuse.as_str()));
    h.state_mut().expanded = None;
    h.run();
    on_card(&h, "D")[0].click_secondary();
    h.run();
    h.get_by_label("Slot 1: Diffuse");
}

#[test]
fn the_long_badge_text_is_on_the_pill_hover() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    // On disk but no texture, so no plan lists it and the chip stays (a deleted file's line clears: Note HE2).
    std::fs::write(m.root.clone().unwrap().join("landscape/notes.txt"), "x").unwrap();
    m.edit_docs("t", None, true, |_, s| {
        s.add_member(ROCK, "landscape/notes.txt")
    });
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    use sdc_gui::model::sets::BADGE_PBR_LANDSCAPE_PGP_ONLY;
    assert!(h
        .query_by_label_contains(BADGE_PBR_LANDSCAPE_PGP_ONLY)
        .is_none());
    on_card(&h, "PBR Landscape")[0].hover();
    h.run();
    h.run();
    h.get_by_label_contains(BADGE_PBR_LANDSCAPE_PGP_ONLY);
    // The recipe and missing-entry lines follow on the same hover.
    h.get_by_label_contains("rmaos: run");
    h.get_by_label_contains("no file matches landscape/notes.txt");
}

/// K12: an `add` entry that matches no file is a red "?" chip with the reason on its hover.
#[test]
fn a_missing_entry_is_a_question_mark_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    std::fs::write(m.root.clone().unwrap().join("landscape/notes.txt"), "x").unwrap();
    m.edit_docs("t", None, true, |_, s| {
        s.add_member(ROCK, "landscape/notes.txt")
    });
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    // Note DU1: the chip is a button named by its entry.
    let q = on_card(&h, "Missing: landscape/notes.txt");
    assert_eq!(q.len(), 1, "one chip per missing entry");
    q[0].hover();
    h.run();
    h.run();
    h.get_by_label("no file matches landscape/notes.txt");
}

/// Press on `at` and move past the drag threshold: whether a texture is now in flight. The payload is cleared before
/// the release, so nothing is dropped.
fn drags_a_texture(h: &mut Harness<'static, AppModel>, at: egui::Pos2) -> bool {
    use sdc_gui::ui::sets_column::DragItem;
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.hover_at(at);
    h.run();
    h.event(press(true));
    h.run();
    for step in 1..=4 {
        h.event(egui::Event::PointerMoved(
            at + egui::vec2(6.0 * step as f32, 0.0),
        ));
        h.run_steps(2); // the payload pass, then the painted one
    }
    let dragging = egui::DragAndDrop::payload::<DragItem>(&h.ctx).is_some();
    egui::DragAndDrop::clear_payload(&h.ctx);
    h.event(press(false));
    h.run();
    dragging
}

/// Note JW3, K13: a borrowed chip has a neutral outline and link mark, says where it is from in its one hover, and is never offered for
/// dragging (dragging it would move another set's member). An own chip still drags.
#[test]
fn a_borrowed_chip_is_outlined_neutral_and_offers_no_drag() {
    use sdc_gui::model::sets::DropChoice;
    use sdc_gui::model::sets::DropOutcome;
    let d = tempfile::tempdir().unwrap();
    // pbr_export.rs::vanilla_borrow: a new set from leaf borrows rock01's normal.
    let mut m = pbr_ready(d.path());
    let leaf = m
        .items
        .iter()
        .find(|i| i.name == "leaf.png")
        .unwrap()
        .key
        .clone();
    m.set_field(&leaf, "subtype", serde_json::json!("pbr_albedo"));
    idle(&mut m);
    let id = m.drop_new_set(&leaf).unwrap();
    idle(&mut m);
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    let normal = m
        .items
        .iter()
        .find(|i| i.name == "rock01_normal.png")
        .unwrap()
        .key
        .clone();
    assert!(matches!(
        m.drop_choice(&normal, &id, DropChoice::Share),
        DropOutcome::Joined
    ));
    idle(&mut m);
    let v = set_views(&m).into_iter().find(|v| v.id == id).unwrap();
    assert!(
        matches!(
            v.slots.get(&SlotKind::Normal),
            Some(SlotFill::Borrowed { .. })
        ),
        "{:?}",
        v.slots
    );
    let mut h = harness(m);
    h.run();
    let chip = on_card_of(&h, &id, "N");
    assert_eq!(chip.len(), 1, "the borrower's N chip");
    let at = chip[0].rect();
    let strong = h.ctx.global_style().visuals.strong_text_color();
    let stroked = |h: &Harness<'static, AppModel>, r: egui::Rect, c: egui::Color32| {
        shapes(h).iter().any(|s| {
            matches!(s, egui::Shape::Rect(x) if x.rect == r && x.stroke.width == 2.0 && x.stroke.color == c)
        })
    };
    let backing = |chip: egui::Rect| {
        egui::Rect::from_min_size(chip.min + egui::vec2(11.0, 2.0), egui::Vec2::splat(10.0))
    };
    let rings = |chip| {
        shapes(&h)
            .iter()
            .filter(|s| {
                matches!(s, egui::Shape::Circle(c)
        if c.stroke.width == 1.5 && c.stroke.color == strong && backing(chip).contains(c.center))
            })
            .count()
    };
    assert!(
        stroked(&h, at, strong),
        "the borrowed chip has the neutral 2 px outline"
    );
    assert!(!stroked(&h, at, sdc_gui::ui::status::AMBER), "and no amber");
    assert!(shapes(&h).iter().any(|s| matches!(s, egui::Shape::Rect(r)
        if r.rect == backing(at) && r.fill == h.ctx.global_style().visuals.panel_fill)));
    assert_eq!(rings(at), 2);
    let own = on_card(&h, "N")[0].rect();
    assert!(
        !stroked(&h, own, strong) && rings(own) == 0,
        "rock01's own N chip has neither"
    );
    assert!(!stroked(&h, own, sdc_gui::ui::status::AMBER));
    h.hover_at(at.center());
    h.run();
    h.run();
    assert_eq!(
        h.query_all_by_label_contains("borrowed from landscape/rock01")
            .count(),
        1,
        "one hover"
    );
    // The member's name and the borrowed line are the same hover.
    h.get_by_label_contains("rock01_normal.png\nborrowed from landscape/rock01");
    assert!(
        !drags_a_texture(&mut h, at.center()),
        "a borrowed chip offers no drag"
    );
    assert!(
        drags_a_texture(&mut h, own.center()),
        "an own chip still drags"
    );
}

/// Final review I1: a texture dragged over a card outlines that card, on top of the card's own frame (the drop
/// zone's own hover stroke sits under the card frame).
#[test]
fn a_texture_dragged_over_a_card_outlines_it() {
    use sdc_gui::ui::sets_column::DragItem;
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let item = m.items[0].key.clone();
    let mut h = harness(m);
    h.run();
    let card = card_rect(&h);
    let active = h.ctx.global_style().visuals.widgets.active.bg_stroke;
    let near = |a: egui::Rect, b: egui::Rect| {
        (a.min - b.min).length() < 1.0 && (a.max - b.max).length() < 1.0
    };
    // The last rect shape on the card, and whether it is the drop outline.
    let top_is_outline = |h: &Harness<'static, AppModel>| {
        shapes(h)
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Rect(r) if near(r.rect, card) && r.stroke.width > 0.0 => {
                    Some(r.stroke)
                }
                _ => None,
            })
            .next_back()
            .is_some_and(|s| s == active)
    };
    assert!(!top_is_outline(&h), "no outline without a drag");
    let at = card.center();
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.hover_at(at);
    h.run();
    h.event(press(true));
    for step in 1..=3 {
        egui::DragAndDrop::set_payload(&h.ctx, DragItem(item.clone()));
        h.event(egui::Event::PointerMoved(
            at + egui::vec2(2.0 * step as f32, 0.0),
        ));
        h.run_steps(2); // the payload pass, then the painted one
    }
    assert!(
        egui::DragAndDrop::has_payload_of_type::<DragItem>(&h.ctx),
        "a texture is in flight"
    );
    assert!(
        top_is_outline(&h),
        "the card under a dragged texture is outlined on top of its frame"
    );
    egui::DragAndDrop::clear_payload(&h.ctx);
    h.event(press(false));
    h.run();
    assert!(!top_is_outline(&h), "the outline goes with the drag");
}

/// Press `at` and move it 30 pt in 3 steps (the button stays down); returns where the pointer ends.
fn start_drag(h: &mut Harness<'static, AppModel>, at: egui::Pos2) -> egui::Pos2 {
    h.hover_at(at);
    h.run();
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
    h.run();
    for step in 1..=3 {
        h.event(egui::Event::PointerMoved(
            at + egui::vec2(10.0 * step as f32, 0.0),
        ));
        h.run_steps(2); // the payload pass, then the painted one
    }
    at + egui::vec2(30.0, 0.0)
}

/// T7 K10: a 40 x 40 ghost 12 pt below and right of the pointer.
fn assert_ghost_rect(h: &Harness<'static, AppModel>, pointer: egui::Pos2) {
    assert!(egui::DragAndDrop::has_payload_of_type::<
        sdc_gui::ui::sets_column::DragItem,
    >(&h.ctx));
    let want = pointer + egui::vec2(12.0, 12.0);
    let bounds = |s: &egui::Shape| match s {
        egui::Shape::Rect(r) => Some(r.rect),
        egui::Shape::Mesh(m) => Some(m.calc_bounds()),
        _ => None,
    };
    assert!(
        shapes(h)
            .iter()
            .filter_map(bounds)
            .any(|r| (r.min - want).length() < 1.0 && (r.width() - 40.0).abs() < 0.5),
        "a 40 wide ghost at {want:?}"
    );
}

/// The ghost plus the Grabbing cursor (R5, green from the start; the drag plugin sets it for a dragged widget).
fn assert_ghost(h: &Harness<'static, AppModel>, pointer: egui::Pos2) {
    assert_ghost_rect(h, pointer);
    assert_eq!(
        h.output().platform_output.cursor_icon,
        egui::CursorIcon::Grabbing
    );
}

#[test]
fn a_dragged_tile_shows_a_ghost_under_the_pointer_and_dims_its_source() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    h.run();
    let label = outside_sets(&h, "rock01_normal.png").rect();
    let at = label.center();
    let p = start_drag(&mut h, at);
    assert_ghost(&h, p);
    // The source tile is dimmed: its 2 pt frame stroke is half transparent.
    let faded = shapes(&h).iter().any(|s| matches!(s,
        egui::Shape::Rect(r) if r.rect.contains_rect(label) && r.stroke.width == 2.0 && r.stroke.color.a() < 200));
    assert!(faded, "the dragged source is dimmed");
    // Review I1: no full-opacity hover or press stroke over the dimmed source.
    let lit = shapes(&h).iter().any(|s| matches!(s,
        egui::Shape::Rect(r) if r.rect.contains_rect(label) && r.stroke.width == 2.0 && r.stroke.color.a() == 255));
    assert!(!lit, "the dimmed source shows no full-opacity stroke");
    egui::DragAndDrop::clear_payload(&h.ctx);
}

/// Review M3: the ghost needs no source widget, and goes with the payload.
#[test]
fn the_ghost_is_painted_from_the_payload_alone_and_goes_with_it() {
    use sdc_gui::ui::sets_column::DragItem;
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let key = m.items[0].key.clone();
    let mut h = harness(m);
    h.run();
    let at = egui::pos2(900.0, 500.0);
    egui::DragAndDrop::set_payload(&h.ctx, DragItem(key));
    h.hover_at(at);
    h.run_steps(2); // the payload pass, then the painted one
    assert_ghost_rect(&h, at);
    egui::DragAndDrop::clear_payload(&h.ctx);
    h.run();
    let want = at + egui::vec2(12.0, 12.0);
    assert!(
        !shapes(&h).iter().any(|s| match s {
            egui::Shape::Rect(r) => (r.rect.min - want).length() < 1.0,
            egui::Shape::Mesh(m) => (m.calc_bounds().min - want).length() < 1.0,
            _ => false,
        }),
        "the ghost is gone with the payload"
    );
    assert!(
        !shapes(&h).iter().any(|s| matches!(s,
            egui::Shape::Rect(r) if r.stroke.width == 2.0 && r.stroke.color.a() < 200)),
        "no tile is dimmed"
    );
}

#[test]
fn a_dragged_member_chip_shows_the_same_ghost() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    h.run();
    let at = on_card(&h, "N")[0].rect().center();
    let p = start_drag(&mut h, at);
    assert_ghost(&h, p);
    egui::DragAndDrop::clear_payload(&h.ctx);
}

#[test]
fn a_hovered_tile_takes_the_hover_stroke_and_an_amber_tile_keeps_amber() {
    use sdc_gui::model::items::ItemStatus;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let amber_name = m.items[0].name.clone();
    m.items[0].status = ItemStatus::NeedsDecision;
    let ready_name = m.items[1].name.clone();
    m.items[1].status = ItemStatus::Ready;
    let mut h = harness(m);
    h.run();
    let hov = h.ctx.global_style().visuals.widgets.hovered.bg_stroke.color;
    // The last 2 pt stroke around the tile that holds `name`.
    let top = |h: &Harness<'static, AppModel>, name: &str| {
        let label = outside_sets(h, name).rect();
        shapes(h)
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Rect(r) if r.rect.contains_rect(label) && r.stroke.width == 2.0 => {
                    Some(r.stroke.color)
                }
                _ => None,
            })
            .next_back()
    };
    let at = outside_sets(&h, &ready_name).rect().center();
    h.hover_at(at);
    h.run();
    assert_eq!(
        top(&h, &ready_name),
        Some(hov),
        "hover stroke on a Ready tile"
    );
    let at = outside_sets(&h, &amber_name).rect().center();
    h.hover_at(at);
    h.run();
    assert_eq!(
        top(&h, &amber_name),
        Some(sdc_gui::ui::status::AMBER),
        "status wins over hover"
    );
}

#[test]
fn a_pressed_strip_fills() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.settings.textures_collapsed = true;
    let mut h = harness(m);
    h.run();
    let at = h.get_by_label("Textures").rect().center();
    let fill = h.ctx.global_style().visuals.widgets.active.weak_bg_fill;
    let covers = |h: &Harness<'static, AppModel>| {
        shapes(h).iter().any(|s| {
            matches!(s,
            egui::Shape::Rect(r) if r.fill == fill && r.rect.contains(at) && r.rect.width() < 60.0)
        })
    };
    h.hover_at(at);
    h.run();
    assert!(!covers(&h), "no fill before the press");
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
    h.run();
    assert!(covers(&h), "the pressed strip is filled");
}

/// Final review D1: a drag held still asks for no frames (the ghost moves only with pointer events), and egui's own
/// repaint after the release frame takes the ghost and the dim away one frame after release.
#[test]
fn a_still_drag_asks_for_no_frames_and_the_ghost_goes_one_frame_after_release() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    h.run();
    let label = outside_sets(&h, "rock01_normal.png").rect();
    let p = start_drag(&mut h, label.center());
    let delay = |h: &Harness<'static, AppModel>| {
        h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay
    };
    h.run();
    assert_ghost_rect(&h, p);
    assert!(delay(&h) > std::time::Duration::ZERO, "a still drag idles");
    h.event(egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });
    h.step();
    assert_eq!(
        delay(&h),
        std::time::Duration::ZERO,
        "the release frame asks for one more"
    );
    h.step();
    let want = p + egui::vec2(12.0, 12.0);
    assert!(
        !shapes(&h).iter().any(|s| match s {
            egui::Shape::Rect(r) => (r.rect.min - want).length() < 1.0,
            egui::Shape::Mesh(m) => (m.calc_bounds().min - want).length() < 1.0,
            _ => false,
        }),
        "the ghost is gone"
    );
    assert!(
        !shapes(&h).iter().any(|s| matches!(s,
            egui::Shape::Rect(r) if r.stroke.width == 2.0 && r.stroke.color.a() < 200)),
        "no tile is dimmed"
    );
}

// Task 21 (Note FM32): the virtualised Sets list keeps keyboard reach, card heights and the scroll anchor.

type SetsHarness = Harness<'static, (AppModel, sdc_gui::ui::State)>;

fn many(m: AppModel) -> SetsHarness {
    many_at(m, 1500.0)
}

fn many_at(m: AppModel, width: f32) -> SetsHarness {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(width, 900.0))
        .with_max_steps(16)
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, sdc_gui::ui::State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, sdc_gui::ui::State::default()),
        );
    h
}

fn pos_rect(h: &SetsHarness, n: usize, total: usize) -> Option<egui::Rect> {
    h.query_all_by_label(&format!("Position {n} of {total}"))
        .map(|n| n.rect())
        .find(|r| in_sets_column(h, *r))
}

fn set_ids(m: &AppModel) -> Vec<String> {
    sdc_gui::model::sets::views(m)
        .iter()
        .map(|v| v.id.clone())
        .collect()
}

#[test]
fn off_screen_set_cards_have_accesskit_nodes_and_tab_stops() {
    use egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let m = many_sets(d.path(), 60);
    let ids = set_ids(&m);
    let mut h = many(m);
    h.run();
    let off: Vec<usize> = (1..=60)
        .filter(|&n| pos_rect(&h, n, 60).is_none())
        .collect();
    assert!(off.len() > 30, "most cards are off screen: {}", off.len());
    let node_name = |n: usize| format!("Set {}, position {n} of 60", ids[n - 1]);
    for &n in &off {
        let name = node_name(n);
        assert_eq!(h.query_all_by_label(&name).count(), 1, "{name}'s node");
    }
    // Tab from the Sets filter walks the cards: it reaches an off-screen card's stand-in, which scrolls its card in.
    h.query_all_by_role_and_label(Role::Button, "Filter, Sets shader filter")
        .find(|n| in_sets_column(&h, n.rect()))
        .expect("the Sets filter button")
        .focus();
    h.step();
    // The first card wholly past the panel is the next stop that is only a stand-in. (A card whose position label
    // is cut off by the bottom edge is drawn, so its Tab stops are its own parts, not a stand-in.)
    let panel = support::panel(&h.ctx, "sets");
    let next = *off
        .iter()
        .find(|&&n| {
            h.query_all_by_label(&node_name(n))
                .all(|c| !c.rect().intersects(panel))
        })
        .expect("a card wholly past the panel");
    let mut presses = 0;
    while pos_rect(&h, next, 60).is_none() && presses < 600 {
        h.key_press(egui::Key::Tab);
        h.step();
        presses += 1;
    }
    assert!(
        pos_rect(&h, next, 60).is_some(),
        "Tab never scrolled card {next} in"
    );
    // The focus survived the swap from the stand-in to the drawn card: the focused node is that card, by its name.
    let focused: Vec<String> = h
        .query_all_by_role(Role::Button)
        .filter(|n| n.is_focused())
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert_eq!(focused, [node_name(next)], "the focus follows the card in");
}

#[test]
fn a_go_to_set_in_the_same_frame_wins_over_a_tab_reveal() {
    use egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let m = many_sets(d.path(), 60);
    let (ids, views) = (set_ids(&m), sdc_gui::model::sets::views(&m));
    let (far, near) = (views[59].key.clone(), 45);
    let mut h = many(m);
    h.run();
    // The focus lands on a far card's stand-in in the very frame a Go to set to the last card is pending.
    h.query_all_by_role_and_label(
        Role::Button,
        &format!("Set {}, position {near} of 60", ids[near - 1]),
    )
    .next()
    .expect("the stand-in")
    .focus();
    h.state_mut().0.nav.reveal_set = Some(sdc_gui::model::SetTarget::Key(far));
    h.step();
    h.run();
    assert!(
        h.state().0.nav.reveal_set.is_none(),
        "the Tab asks for no scroll of its own"
    );
    assert!(
        pos_rect(&h, 60, 60).is_some(),
        "the Go to set stays where it landed"
    );
}

/// Card `n` (1-based) of `total`, measured by scrolling to it, then scrolled away from again.
fn measure_far_card(h: &mut SetsHarness, n: usize) -> String {
    let views = sdc_gui::model::sets::views(&h.state().0);
    let (far, first) = (views[n - 1].clone(), views[0].key.clone());
    h.state_mut()
        .0
        .palette_pick(&sdc_gui::model::nav::Pick::Set(far.key.clone()));
    h.run();
    assert!(pos_rect(h, n, views.len()).is_some(), "card {n} was drawn");
    h.state_mut()
        .0
        .palette_pick(&sdc_gui::model::nav::Pick::Set(first));
    h.run();
    assert!(
        pos_rect(h, n, views.len()).is_none(),
        "card {n} is far again"
    );
    far.id
}

#[test]
fn card_heights_refresh_after_a_resize() {
    let d = tempfile::tempdir().unwrap();
    let mut m = many_sets(d.path(), 60);
    let all: Vec<String> = m.items.iter().map(|i| i.key.clone()).collect();
    // Repeats stand in for more files: enough chips to wrap in a narrow column.
    m.sets
        .iter_mut()
        .for_each(|s| s.members = all.iter().cycle().take(all.len() * 4).cloned().collect());
    m.invalidate_views();
    let mut h = many_at(m, 2600.0);
    let chosen = sdc_gui::ui::panel_ids::chosen_width_id("sets");
    h.ctx.data_mut(|d| d.insert_persisted(chosen, 520.0_f32));
    h.run();
    let far = measure_far_card(&mut h, 50);
    let wide = h.state().1.drops.card_h[&far];
    // Narrow the column: the far card is not drawn, yet its remembered height follows the width.
    h.ctx
        .data_mut(|d| d.insert_persisted(chosen, sdc_gui::ui::columns::SETS_MIN));
    h.run();
    assert!(pos_rect(&h, 50, 60).is_none(), "the far card stays undrawn");
    let narrow = h.state().1.drops.card_h[&far];
    assert!(
        narrow > wide + 1.0,
        "{wide} at 520 wide, {narrow} at the minimum"
    );
}

#[test]
fn a_far_jump_does_not_shift_the_scroll() {
    // Short cards first, tall ones after: the mean of the measured heights climbs as the jump lands.
    let d = tempfile::tempdir().unwrap();
    let mut m = many_sets(d.path(), 500);
    let all: Vec<String> = m.items.iter().map(|i| i.key.clone()).collect();
    for (i, s) in m.sets.iter_mut().enumerate() {
        s.members = if i < 20 { vec![] } else { all.clone() };
    }
    m.invalidate_views();
    let key = sdc_gui::model::sets::views(&m)[199].key.clone();
    let mut h = many(m);
    h.run();
    h.state_mut()
        .0
        .palette_pick(&sdc_gui::model::nav::Pick::Set(key));
    // Once card 200 is laid out, its place on screen does not move over the next frames.
    let mut ys = vec![];
    for _ in 0..12 {
        h.step();
        ys.extend(pos_rect(&h, 200, 500).map(|r| r.top()));
    }
    assert!(ys.len() >= 6, "card 200 is on screen: {ys:?}");
    // egui snaps each allocation, so the sum of 200 of them is a pixel off the sum of the heights: the first frames
    // may differ by that, never by a card's worth.
    assert!(
        ys.windows(2).all(|w| (w[0] - w[1]).abs() < 1.5),
        "card 200 moved between frames: {ys:?}"
    );
}

/// The AccessKit name `member_chip` gives the own `leaf_f.png` chip on base (read off the premise run).
const FUZZ_CHIP: &str = "F";

#[test]
fn a_linked_chip_is_filled_and_named_for_its_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.run_menu(MenuAction::SlotLink {
        set: "arch/leaf".into(),
        n: 1,
        item: "arch/wall.png".into(),
    });
    let mut h = harness(m);
    h.run();
    assert_eq!(
        on_card_of(&h, "arch/leaf", "D, from set arch/wall").len(),
        1
    );
    assert!(on_card_of(&h, "arch/leaf", "D, empty").is_empty());
    h.state_mut().undo();
    h.run();
    assert_eq!(on_card_of(&h, "arch/leaf", "D, empty").len(), 1);
}

#[test]
fn a_coat_link_fills_one_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal)],
    );
    m.run_menu(MenuAction::SlotLink {
        set: "arch/leaf".into(),
        n: 7,
        item: "arch/lacq_cnr.png".into(),
    });
    let mut h = harness(m);
    h.run();
    let linked = h
        .query_all_by_label_contains(", from set arch/lacq")
        .count();
    assert_eq!(
        linked, 1,
        "one slot-7 link is one chip, not a Coat and a Fuzz chip"
    );
}

#[test]
fn a_coat_link_over_an_own_fuzz_shows_only_the_link() {
    let d = tempfile::tempdir().unwrap();
    let m = no_albedo_pair_with(
        d.path(),
        &[
            ("leaf_f.png", [200, 200, 200], Subtype::PbrFuzz),
            ("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal),
        ],
    );
    let mut h = harness(m);
    h.run();
    assert_eq!(
        on_card_of(&h, "arch/leaf", FUZZ_CHIP).len(),
        1,
        "premise: the own _f's chip is on the card"
    );
    h.state_mut().run_menu(MenuAction::SlotLink {
        set: "arch/leaf".into(),
        n: 7,
        item: "arch/lacq_cnr.png".into(),
    });
    h.run();
    let drawn = h
        .query_all_by_label_contains(", from set arch/lacq")
        .count()
        + on_card_of(&h, "arch/leaf", FUZZ_CHIP).len();
    let v = views(h.state())
        .iter()
        .find(|v| v.id == "arch/leaf")
        .unwrap()
        .clone();
    let counted = view_row(&v)
        .iter()
        .filter(|c| c.kind.game_slot() == Some(7))
        .count();
    assert_eq!(
        (drawn, counted),
        (1, 1),
        "slot 7: one chip drawn (the link's), one counted"
    );
    assert!(
        on_card_of(&h, "arch/leaf", FUZZ_CHIP).is_empty(),
        "the own _f is not written, so its chip goes"
    );
    // chip_row's trailing loop names an un-chipped member "{file}, {slot text} (its slot is taken)": not for a
    // Replaced file
    assert!(
        h.query_all_by_label_contains("leaf_f.png, ").all(|n| !n
            .accesskit_node()
            .label()
            .is_some_and(|l| l.contains("(its slot is taken)"))),
        "no thumbnail for the own _f"
    );
}

#[test]
fn a_cleared_slot_has_a_cleared_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command("arch/wall", 2, SlotCommand::Clear)
        .unwrap();
    let mut h = harness(m);
    h.run();
    assert_eq!(on_card_of(&h, "arch/wall", "N, cleared").len(), 1);
}

/// Final wave Group 5 (Important 6, ledger 88, 89): a chip's AccessKit node says what its hover says, while the hover
/// stays lazy: an own chip its file and lock, a cleared chip its file (and no "empty" value left behind by the box's
/// Label), a typed-path box its path; the typed-path box paints its slot's lock as a thumbnail does.
#[test]
fn chips_describe_what_their_hover_says_and_a_typed_path_box_paints_its_lock() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command("arch/wall", 2, SlotCommand::Clear)
        .unwrap();
    m.set_slot_command(
        "arch/leaf",
        1,
        SlotCommand::Path(r"textures\other\x_d.dds".into()),
    )
    .unwrap();
    for set in ["arch/wall", "arch/leaf"] {
        assert_eq!(
            m.run_menu(MenuAction::SlotLock {
                set: set.into(),
                n: 1,
                on: true
            }),
            None
        );
    }
    let mut h = harness(m);
    h.run();
    let desc = |h: &Harness<'static, AppModel>, set: &str, label: &str| {
        let n = on_card_of(h, set, label);
        assert_eq!(n.len(), 1, "{set}: one {label:?} chip");
        let a = n[0].accesskit_node();
        (a.description(), a.value(), n[0].rect())
    };
    let (own, _, _) = desc(&h, "arch/wall", "D");
    let own = own.unwrap_or_default();
    assert!(
        own.contains("wall.png") && own.contains("(slot pinned)"),
        "own chip: {own:?}"
    );
    let (cleared, value, _) = desc(&h, "arch/wall", "N, cleared");
    assert!(
        cleared
            .as_deref()
            .is_some_and(|c| c.contains("wall_n.png") && c.contains("(cleared)")),
        "cleared chip: {cleared:?}"
    );
    assert!(
        value.as_deref().is_none_or(|v| !v.contains("empty")),
        "no empty value: {value:?}"
    );
    let (typed, _, rect) = desc(&h, "arch/leaf", "D, path x_d.dds");
    assert_eq!(
        typed.as_deref(),
        Some(r"Diffuse linked to textures\other\x_d.dds")
    );
    assert!(
        texts_in(&h, rect).iter().any(|(t, _)| t == "🔒"),
        "the typed-path box paints its slot's lock: {:?}",
        texts_in(&h, rect)
    );
}

/// The painted text shapes of the frame inside `chip`: (text, galley rect in screen space).
fn texts_in(h: &Harness<'static, AppModel>, chip: egui::Rect) -> Vec<(String, egui::Rect)> {
    shapes(h)
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) => Some((
                t.galley.text().to_string(),
                t.galley.rect.translate(t.pos.to_vec2()),
            )),
            _ => None,
        })
        .filter(|(_, r)| chip.contains_rect(*r))
        .collect()
}

/// Whether two rects share area (rects that only touch along an edge do not overlap).
fn overlaps(a: egui::Rect, b: egui::Rect) -> bool {
    let i = a.intersect(b);
    i.width() > 0.0 && i.height() > 0.0
}

/// Linked Coat (slot 7) and RMAOS (slot 6) chips on `leaf`, each with both locks when `locks`: the chip named
/// `label` has its link mark (backing square and two rings) at its top centre, inside the chip and touching neither
/// the slot letter nor a lock.
fn assert_mark_at_top_centre(label: &str, letter: &str, locks: bool, theme: egui::Theme) {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal)],
    );
    for (n, item) in [(6, "arch/wall_rmaos.png"), (7, "arch/lacq_cnr.png")] {
        // Ruling 3b-F21: a pick waits for the plan the lock before it started.
        idle(&mut m);
        assert_eq!(
            m.run_menu(MenuAction::SlotLink {
                set: "arch/leaf".into(),
                n,
                item: item.into(),
            }),
            None,
            "{item}"
        );
        if locks {
            m.set_slot_lock("arch/leaf", n, true).unwrap();
            m.run_menu(MenuAction::PgpLock {
                set: "arch/leaf".into(),
                n,
                on: true,
            });
        }
    }
    idle(&mut m);
    let mut h = harness(m);
    h.ctx.set_theme(theme);
    h.run();
    let visuals = &h.ctx.global_style().visuals;
    assert_eq!(visuals.dark_mode, theme == egui::Theme::Dark);
    assert_ne!(visuals.strong_text_color(), visuals.panel_fill);
    let chip = {
        let card = card_of(&h, "arch/leaf");
        let found: Vec<_> = h
            .query_all_by_label_contains(label)
            .filter(|n| card.contains_rect(n.rect()))
            .collect();
        assert_eq!(found.len(), 1, "the {label} chip");
        found[0].rect()
    };
    let texts = texts_in(&h, chip);
    let lock_rects: Vec<egui::Rect> = texts
        .iter()
        .filter(|(t, _)| t == "🔒")
        .map(|(_, r)| *r)
        .collect();
    assert_eq!(lock_rects.len(), if locks { 2 } else { 0 }, "{texts:?}");
    let letter_rect = texts
        .iter()
        .find(|(t, _)| t == letter)
        .unwrap_or_else(|| panic!("the letter {letter}: {texts:?}"))
        .1;
    let circles: Vec<egui::Pos2> = shapes(&h)
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Circle(c) if c.stroke.width == 1.5 && chip.contains(c.center) => {
                assert_eq!(
                    c.stroke.color,
                    visuals.strong_text_color(),
                    "link rings in {theme:?}"
                );
                Some(c.center)
            }
            _ => None,
        })
        .collect();
    assert_eq!(circles.len(), 2, "the mark's two rings are inside the chip");
    let mark = shapes(&h)
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Rect(r)
                if r.fill == visuals.panel_fill
                    && chip.contains_rect(r.rect)
                    && r.rect.width() < chip.width()
                    && circles.iter().all(|c| r.rect.contains(*c)) =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .min_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("the mark's backing square");
    assert_eq!(
        mark,
        egui::Rect::from_min_size(chip.min + egui::vec2(11.0, 2.0), egui::Vec2::splat(10.0)),
        "the mark is at the chip's top centre in every lock state"
    );
    assert!(
        !overlaps(mark, letter_rect),
        "the mark {mark:?} covers the letter {letter_rect:?}"
    );
    for l in &lock_rects {
        assert!(!overlaps(mark, *l), "the mark {mark:?} covers a lock {l:?}");
    }
}

#[test]
fn the_link_mark_clears_the_letter_and_both_locks_on_a_coat_chip() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        assert_mark_at_top_centre(", from set arch/lacq", "CNR", true, theme);
    }
}

#[test]
fn the_link_mark_clears_the_letter_and_both_locks_on_an_rmaos_chip() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        assert_mark_at_top_centre(", from set arch/wall", "RMAOS", true, theme);
    }
}

#[test]
fn the_link_mark_is_in_the_same_place_without_locks() {
    assert_mark_at_top_centre(", from set arch/lacq", "CNR", false, egui::Theme::Dark);
    assert_mark_at_top_centre(", from set arch/wall", "RMAOS", false, egui::Theme::Dark);
}

#[test]
fn a_typed_path_link_is_a_filled_box_named_for_its_file_with_the_slot_menu() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command(
        "arch/leaf",
        1,
        SlotCommand::Path("textures\\other\\thing_d.dds".into()),
    )
    .unwrap();
    let mut h = harness(m);
    h.run();
    let chip = on_card_of(&h, "arch/leaf", "D, path thing_d.dds");
    assert_eq!(chip.len(), 1, "the typed path's box");
    chip[0].hover();
    h.run();
    h.run();
    h.get_by_label_contains("Diffuse linked to textures\\other\\thing_d.dds");
    on_card_of(&h, "arch/leaf", "D, path thing_d.dds")[0].click_secondary();
    h.run();
    h.get_by_label("Slot 1: Diffuse");
}

#[test]
fn a_cleared_chip_offers_the_slot_menu_that_restores_the_slot() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command("arch/wall", 2, SlotCommand::Clear)
        .unwrap();
    let mut h = harness(m);
    h.run();
    on_card_of(&h, "arch/wall", "N, cleared")[0].click_secondary();
    h.run();
    h.get_by_label_contains("Slot 2:");
    let from_set = h.get_by_label_contains("From this set");
    assert!(!from_set.accesskit_node().is_disabled());
    from_set.click();
    h.run();
    idle(h.state_mut());
    h.run();
    assert!(on_card_of(&h, "arch/wall", "N, cleared").is_empty());
}

/// Nodes named "<file>, ... (its slot is taken)": the trailing loop's thumbnails for a file with no slot of its own.
fn taken_thumbs(h: &Harness<'static, AppModel>, file: &str) -> usize {
    h.query_all_by_label_contains(&format!("{file}, "))
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.contains("(its slot is taken)"))
        })
        .count()
}

/// The cells of `id`'s slot row on game slot `n`: what `chip_rows` counts for it.
fn counted_on_slot(m: &AppModel, id: &str, n: u8) -> usize {
    let v = views(m).iter().find(|v| v.id == id).unwrap().clone();
    view_row(&v)
        .iter()
        .filter(|c| c.kind.game_slot() == Some(n))
        .count()
}

#[test]
fn a_cleared_slot_over_an_own_file_draws_one_chip_that_names_the_file() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command("arch/wall", 2, SlotCommand::Clear)
        .unwrap();
    let mut h = harness(m);
    h.run();
    let cleared = on_card_of(&h, "arch/wall", "N, cleared").len();
    let drawn = cleared + taken_thumbs(&h, "wall_n.png");
    assert_eq!(cleared, 1);
    assert_eq!(
        (drawn, counted_on_slot(h.state(), "arch/wall", 2)),
        (1, 1),
        "slot 2: one chip drawn, one counted"
    );
    assert_eq!(
        taken_thumbs(&h, "wall_n.png"),
        0,
        "no thumbnail for the file"
    );
    on_card_of(&h, "arch/wall", "N, cleared")[0].hover();
    h.run();
    h.run();
    h.get_by_label_contains("(cleared): wall_n.png");
}

#[test]
fn a_link_over_an_own_file_of_its_kind_draws_one_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[("other.png", [10, 200, 10], Subtype::PbrAlbedo)],
    );
    m.run_menu(MenuAction::SlotLink {
        set: "arch/wall".into(),
        n: 1,
        item: "arch/other.png".into(),
    });
    let mut h = harness(m);
    h.run();
    let linked = h
        .query_all_by_label_contains(", from set arch/other")
        .count();
    assert_eq!(linked, 1, "the linked chip");
    let drawn = linked + taken_thumbs(&h, "wall.png");
    assert_eq!(
        (drawn, counted_on_slot(h.state(), "arch/wall", 1)),
        (1, 1),
        "slot 1: one chip drawn (the link's), one counted"
    );
    assert_eq!(
        taken_thumbs(&h, "wall.png"),
        0,
        "no thumbnail for the own file"
    );
}

/// What a card's title painted: its text, how many lines the galley has, and the title node's rect.
fn title_probe(h: &Harness<'static, AppModel>, id: &str) -> (String, usize, egui::Rect) {
    let r = h
        .query_all_by_label(id)
        .next()
        .expect("the card title")
        .rect();
    let mut found = None;
    fn flat(s: &egui::Shape, r: egui::Rect, out: &mut Option<(String, usize)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| flat(s, r, out)),
            egui::Shape::Text(t) if r.expand(1.0).contains_rect(t.visual_bounding_rect()) => {
                *out = Some((t.galley.job.text.clone(), t.galley.rows.len()));
            }
            _ => {}
        }
    }
    for c in &h.output().shapes {
        flat(&c.shape, r, &mut found);
    }
    let (text, rows) = found.expect("the title's text is painted");
    (text, rows, r)
}

/// The status pill's kind label of the card whose title is `title`: of every painted copy of the set's kind text
/// inside the Sets column (final wave Group 6, ledger 202: the set editor paints the same text), the one nearest the
/// title vertically (the pill sits on the title's row or the row under it).
fn pill_of(h: &Harness<'static, AppModel>, id: &str, title: egui::Rect) -> egui::Rect {
    let panel = sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "sets").expect("the Sets column");
    let v = set_views(h.state())
        .into_iter()
        .find(|v| v.id == id)
        .expect("the set's view");
    let kind = sdc_gui::model::sets::kind_text(h.state(), &v);
    fn flat(s: &egui::Shape, kind: &str, out: &mut Vec<egui::Rect>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| flat(s, kind, out)),
            egui::Shape::Text(t) if t.galley.job.text == kind => out.push(t.visual_bounding_rect()),
            _ => {}
        }
    }
    let mut all = vec![];
    for c in &h.output().shapes {
        flat(&c.shape, kind, &mut all);
    }
    all.into_iter()
        .filter(|r| panel.contains_rect(*r))
        .min_by(|a, b| {
            (a.center().y - title.center().y)
                .abs()
                .total_cmp(&(b.center().y - title.center().y).abs())
        })
        .unwrap_or_else(|| panic!("the pill's {kind:?} label is painted in the Sets column"))
}

#[test]
fn warm_card_titles_reuse_the_fit_at_the_drawn_width() {
    use sdc_gui::ui::paint_counts::Work::{Title, TitleFit};
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.sets_filter.search = "arch/wall".into();
    let mut h = harness(m);
    h.run_steps(4);
    let warm = Title.count();
    let fits = TitleFit.count();
    assert!(warm > 0 && fits > 0);
    h.run_steps(4);
    assert_eq!(
        Title.count(),
        warm,
        "unchanged card rebuilt its fitted title"
    );
    assert_eq!(TitleFit.count(), fits, "unchanged width repeated the fit");
    h.state_mut().sets_filter.search = "wall".into();
    h.run_steps(4);
    assert_eq!(
        Title.count(),
        warm + 1,
        "one measure for the changed title search"
    );
    assert_eq!(
        TitleFit.count(),
        fits + 1,
        "one fit at the drawn width after search changes"
    );
    let width_id = sdc_gui::ui::panel_ids::chosen_width_id("sets");
    h.ctx.data_mut(|d| d.insert_persisted(width_id, 380.0_f32));
    h.run_steps(4);
    assert_eq!(
        Title.count(),
        warm + 1,
        "resizing can reuse the measured text"
    );
    assert_eq!(
        TitleFit.count(),
        fits + 2,
        "one fit for the new label width"
    );
}

/// Task 9 review: a title too wide for the room beside the pill takes its own line (one line high, the pill
/// wholly below it) instead of wrapping mid-word beside the pill; with a set open, at 1055x770.
#[test]
fn a_title_too_wide_beside_the_pill_takes_its_own_line() {
    // Sets at its default 266 px at 1055x770 (nothing open): the one-row layout is chosen, and a 25-character
    // tail does not fit beside the pill. This is the state that is RED without the fix.
    own_line_case(
        [
            "architecture/whiterun/wrwoodbeamlong01",
            "architecture/windhelm/wrwoodbeamlong01",
        ],
        false,
    );
}

/// A pin of the narrower state: with a set open the Sets column is at its floor at 1055x770 and the narrow layout
/// was already chosen before the fix, so this passes on the old code too.
#[test]
fn a_title_in_the_narrowest_column_with_a_set_open_stays_on_one_line_above_the_pill() {
    own_line_case(
        [
            "architecture/whiterun/wrbeam01",
            "architecture/windhelm/wrbeam01",
        ],
        true,
    );
}

fn own_line_case(ids: [&str; 2], open: bool) {
    let d = tempfile::tempdir().unwrap();
    for (id, rgb) in ids.iter().zip([[90u8; 3], [60u8; 3]]) {
        let p = d.path().join("textures").join(format!("{id}.png"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb(rgb))
            .save(&p)
            .unwrap();
    }
    let m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let key = set_views(&m)
        .into_iter()
        .find(|v| v.id == ids[0])
        .unwrap()
        .key;
    let mut h = harness(m);
    h.set_size(egui::vec2(1055.0, 770.0));
    if open {
        h.state_mut().select_set(&key);
    }
    h.run();
    h.run();
    let panel = sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "sets").expect("the Sets column");
    for id in ids {
        // the narrow layout: the title beside the position and handle at the card's full width, the pill below
        let (text, rows, r) = title_probe(&h, id);
        assert_eq!(rows, 1, "{id}: one line, shown {text:?} in {r:?}");
        let pill = pill_of(&h, id, r);
        assert!(
            pill.min.y >= r.max.y - 1.0,
            "{id}: the pill {pill:?} lies below the title {r:?}"
        );
        assert!(
            r.height() < 1.5 * h.ctx.global_style().text_styles[&egui::TextStyle::Body].size + 6.0
                && panel.contains_rect(r),
            "{id}: {r:?} one line high inside the column {panel:?}"
        );
    }
}
#[test]
fn two_cards_sharing_a_leaf_never_read_the_same() {
    const IDS: [&str; 2] = [
        "architecture/whiterun/whiterunstonework/wallblocklarge01",
        "architecture/windhelm/whiterunstonework/wallblocklarge01",
    ];
    let d = tempfile::tempdir().unwrap();
    for (id, rgb) in IDS.iter().zip([[90u8; 3], [60u8; 3]]) {
        let p = d.path().join("textures").join(format!("{id}.png"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb(rgb))
            .save(&p)
            .unwrap();
    }
    let m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let mut h = harness(m);
    h.run();
    let shown: Vec<String> = IDS
        .iter()
        .map(|id| {
            // the title's AccessKit name is the full id (HN6 `widget_info`)
            let r = h
                .query_all_by_label(id)
                .next()
                .expect("the card title")
                .rect();
            painted_in(&h, r)
        })
        .collect();
    assert!(
        IDS.iter().zip(&shown).all(|(id, s)| s != id),
        "premise: both titles are cut: {shown:?}"
    );
    assert_ne!(shown[0], shown[1], "two cards read the same");
    let measured = sdc_gui::ui::paint_counts::Work::ShortTitles;
    assert_eq!(
        measured.count(),
        2,
        "each cut card measured its alternatives once"
    );
    h.run_steps(4);
    assert_eq!(
        measured.count(),
        2,
        "steady frames rebuilt the short title forms"
    );
    for id in IDS {
        let (text, rows, r) = title_probe(&h, id);
        // these tails are wider than the whole card, so they may wrap, and only as the last resort
        let last_resort = text == format!("…/{}", id.split_once('/').unwrap().1);
        assert!(
            rows == 1 || last_resort,
            "{id}: {rows} lines for {text:?} in {r:?}"
        );
        // on its own row: the pill lies wholly below the title, never beside it
        let pill = pill_of(&h, id, r);
        assert!(
            pill.min.y >= r.max.y - 1.0,
            "{id}: the pill {pill:?} lies below the title {r:?}"
        );
    }
    assert!(
        shown[0].ends_with("whiterun/whiterunstonework/wallblocklarge01")
            && shown[1].ends_with("windhelm/whiterunstonework/wallblocklarge01"),
        "each title ends with its unique tail (Task 1's `short`): {shown:?}"
    );
}
