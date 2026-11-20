//! Notes JW1 to JW7: set cards and dragging (owner additions A4 to A9).
mod support;
use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::sets::{
    set_views, slot_shown, BorrowOccupant, DropChoice, DropOutcome, SetView, SlotCommand, SlotFill,
    SlotKind, SlotShown,
};
use sdc_gui::model::AppModel;
use sdc_gui::ui::sets_column::{Pending, DEFAULT_HINT, DEFAULT_HOVER, DEFAULT_SUFFIX};
use sdc_gui::ui::State;
use std::path::Path;
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01";

/// `pbr_ready` with a new set from leaf (pbr_albedo, Vanilla target) that borrows rock01's normal; returns the model,
/// that set's id, and the normal's key.
fn leaf_borrows_normal(dir: &std::path::Path) -> (sdc_gui::model::AppModel, String, String) {
    let (mut m, id) = leaf_set(dir);
    let normal = key_of(&m, "rock01_normal.png");
    assert_eq!(
        m.drop_choice(&normal, &id, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    (m, id, normal)
}

fn leaf_set(dir: &Path) -> (AppModel, String) {
    let mut m = pbr_ready(dir);
    let leaf = key_of(&m, "leaf.png");
    m.set_field(&leaf, "subtype", serde_json::json!("pbr_albedo"));
    idle(&mut m);
    let id = m.drop_new_set(&leaf).unwrap();
    idle(&mut m);
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    (m, id)
}

fn key_of(m: &sdc_gui::model::AppModel, name: &str) -> String {
    m.items
        .iter()
        .find(|i| i.name == name)
        .unwrap_or_else(|| panic!("no item {name}"))
        .key
        .clone()
}

/// A drawn card has both its full-id card node and a position label inside it.
fn try_card_of(h: &H, id: &str) -> Option<egui::Rect> {
    let prefix = format!("Set {id}, position ");
    let node = h.query_by_label_contains(&prefix)?;
    let rect = node.rect();
    let label = node.accesskit_node().label()?;
    let position = format!("Position {}", label.strip_prefix(&prefix)?);
    let drawn = h
        .query_all_by_label(&position)
        .any(|n| rect.contains_rect(n.rect()));
    drawn.then_some(rect)
}

fn card_of(h: &H, id: &str) -> egui::Rect {
    try_card_of(h, id).unwrap_or_else(|| panic!("{id}'s card is not drawn"))
}

fn on_card_of<'a>(h: &'a H, id: &str, label: &'a str) -> Vec<egui_kittest::Node<'a>> {
    let card = card_of(h, id);
    h.query_all_by_label(label)
        .filter(|n| card.contains_rect(n.rect()))
        .collect()
}

/// Press at `at`, drag to `to`, and release with `release` held.
fn drag_to(h: &mut H, at: egui::Pos2, to: egui::Pos2, release: egui::Modifiers) {
    drag_with_modifiers(h, at, to, egui::Modifiers::NONE, release);
}

fn drag_with_modifiers(
    h: &mut H,
    at: egui::Pos2,
    to: egui::Pos2,
    start: egui::Modifiers,
    release: egui::Modifiers,
) {
    drag_over(h, at, to, start);
    h.event(egui::Event::ModifiersChanged(release));
    h.run_steps(1);
    h.event(pointer_button(to, false, release));
    h.run_steps(3);
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.run();
}

fn pointer_button(pos: egui::Pos2, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    }
}

fn drag_over(h: &mut H, at: egui::Pos2, to: egui::Pos2, start: egui::Modifiers) {
    h.event(egui::Event::ModifiersChanged(start));
    h.run_steps(1);
    h.hover_at(at);
    h.run();
    h.event(pointer_button(at, true, start));
    h.run();
    for step in 1..=6 {
        h.event(egui::Event::PointerMoved(
            at + (to - at) * (step as f32 / 6.0),
        ));
        h.run_steps(2);
    }
}

/// Note JW4: dropping a shared chip on another card shares it there through the card's real zone, with no dialog.
#[test]
fn a_shared_chip_dragged_onto_a_card_shares_it_with_no_question() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, leaf, _normal) = leaf_borrows_normal(d.path());
    let odd = m
        .items
        .iter()
        .find(|i| i.name == "odd.png")
        .unwrap()
        .key
        .clone();
    let other = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    h.run();
    let from = on_card_of(&h, &leaf, "N")[0].rect().center();
    let to = card_of(&h, &other).center();
    drag_to(&mut h, from, to, egui::Modifiers::NONE);
    assert_eq!(h.state().1.drops.pending, None, "no question");
    idle(&mut h.state_mut().0);
    let v = set_views(&h.state().0)
        .into_iter()
        .find(|v| v.id == other)
        .unwrap();
    assert!(
        matches!(v.slots.get(&SlotKind::Normal), Some(SlotFill::Borrowed { from, .. }) if from == ROCK),
        "{:?}",
        v.slots
    );
}

fn filled_normal(dir: &Path) -> (AppModel, String, String, String, String) {
    let (mut m, leaf, normal) = leaf_borrows_normal(dir);
    let odd = key_of(&m, "odd.png");
    let other = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    assert_eq!(
        m.drop_choice(&normal, &other, DropChoice::Duplicate),
        DropOutcome::Joined
    );
    idle(&mut m);
    let base = other.rsplit('/').next().unwrap();
    let copy_key = key_of(&m, &format!("{base}_normal.png"));
    m.decide(
        &copy_key,
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::PbrNormal),
    );
    m.decide(
        &copy_key,
        sdc_gui::model::Answer::Green(engine::config::GreenConvention::Directx),
    );
    idle(&mut m);
    let v = set_views(&m).into_iter().find(|v| v.id == other).unwrap();
    let Some(SlotFill::Own { item: copy }) = v.slots.get(&SlotKind::Normal).cloned() else {
        panic!(
            "premise: the duplicate fills the normal slot: {:?}",
            v.slots
        )
    };
    (m, leaf, normal, other, copy)
}

/// Every painted text whose rect lies inside `r` (grown by 1): its text, its rect and its colour (the galley's first
/// section, which `paint_letter` paints in one colour).
fn painted_text_in(h: &H, r: egui::Rect) -> Vec<(String, egui::Rect, egui::Color32)> {
    shapes(h)
        .into_iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) => {
                let rect = t.visual_bounding_rect();
                let colour = t
                    .galley
                    .job
                    .sections
                    .first()
                    .map_or(t.fallback_color, |s| s.format.color);
                let colour = if colour == egui::Color32::PLACEHOLDER {
                    t.fallback_color
                } else {
                    colour
                };
                r.expand(1.0)
                    .contains_rect(rect)
                    .then(|| (t.galley.text().to_string(), rect, colour))
            }
            _ => None,
        })
        .collect()
}

/// The AccessKit description of set `id`'s card node ("Set {id}, position …").
fn card_description(h: &H, id: &str) -> Option<String> {
    let prefix = format!("Set {id}, position ");
    h.query_all(egui_kittest::kittest::by())
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with(&prefix))
        })
        .unwrap_or_else(|| panic!("{id}'s card node"))
        .accesskit_node()
        .description() // accesskit_consumer 0.39: `Option<String>` already
}

/// The sets whose cards are drawn now, with their rects (stand-ins of off-screen cards are skipped).
fn drawn_cards(h: &H) -> Vec<(String, egui::Rect)> {
    set_views(&h.state().0)
        .into_iter()
        .filter_map(|v| try_card_of(h, &v.id).map(|r| (v.id, r)))
        .collect()
}

/// `pbr_opened_plain` with leaf (Diffuse, alpha Cutout) and odd (Diffuse, small image acknowledged) decided, as
/// `support::decide_all` decides them: three cards, rock01 first.
fn three_cards(dir: &Path) -> AppModel {
    use engine::config::{AlphaPolicy, Subtype};
    use sdc_gui::model::Answer;
    let mut m = pbr_opened_plain(dir);
    m.decide("landscape/odd.png", Answer::Subtype(Subtype::Diffuse));
    m.decide(
        "landscape/odd.png",
        Answer::Acknowledge(engine::codes::Code::DimSmall),
    );
    m.decide("landscape/leaf.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/leaf.png", Answer::Alpha(AlphaPolicy::Cutout));
    idle(&mut m);
    m
}

/// Puts `payload` in flight and hovers `at` over three frames (the payload pass, then the painted one each).
fn hover_with<P: Clone + Send + Sync + 'static>(h: &mut H, payload: P, at: egui::Pos2) {
    h.hover_at(at);
    h.run();
    for step in 1..=3 {
        egui::DragAndDrop::set_payload(&h.ctx, payload.clone());
        h.event(egui::Event::PointerMoved(at + egui::vec2(step as f32, 0.0)));
        h.run_steps(2);
    }
}

const LEAF: &str = "landscape/leaf.png";

/// Note JW1 (A4-1, A4-2, A4-3): rock01's empty slots (G, P, CNR, S under plain PBR) are not drawn at rest, are listed
/// in the card's description, and float in a strip under a dragged texture or a dragged share, never under a dragged
/// set. The strip's letters are dim and sit in their boxes where the D chip's letter sits (HP9).
#[test]
fn empty_slots_hide_at_rest_and_float_under_a_dragged_image() {
    use sdc_gui::ui::sets_column::{DragBorrow, DragItem, DragSet};
    const LINE: &str = "Empty: G, P, CNR, S";
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    m.invalidate_views();
    let v = view(&m, ROCK);
    // `shader.rs` l.86: plain PBR's optional slots are _g, _s, _cnr, _f, _p; slot 7 shows Coat (CNR).
    assert_eq!(
        v.empty,
        [
            SlotKind::Emissive,
            SlotKind::Height,
            SlotKind::Coat,
            SlotKind::Subsurface
        ],
        "premise: rock01's empty slots under plain PBR, in row order"
    );
    assert_eq!(v.empty_line, LINE);
    let mut h = harness(m);
    h.run();
    h.run();
    let card = card_of(&h, ROCK);
    for l in ["G", "P", "CNR", "S"] {
        assert!(
            on_card_of(&h, ROCK, &format!("{l}, empty")).is_empty(),
            "no empty {l} chip at rest"
        );
    }
    assert_eq!(card_description(&h, ROCK).as_deref(), Some(LINE));
    assert_eq!(h.state().1.drops.strip, None);
    // the D chip's letter offset, to compare the strip's letters with (HP9)
    let d_chip = on_card_of(&h, ROCK, "D")[0].rect();
    let d_text = painted_text_in(&h, d_chip)
        .iter()
        .find(|(t, ..)| t == "D")
        .expect("the D letter")
        .1;
    hover_with(&mut h, DragSet(ROCK.into()), card.center());
    assert_eq!(
        h.state().1.drops.strip,
        None,
        "a dragged set is not an image"
    );
    egui::DragAndDrop::clear_payload(&h.ctx);
    h.run();
    hover_with(&mut h, DragItem(LEAF.into()), card.center());
    let (set, r, _) = h
        .state()
        .1
        .drops
        .strip
        .clone()
        .expect("a strip under a dragged texture");
    assert_eq!(set, ROCK);
    // four boxes at five per row (JJ2): 4·32 + 3·16 + 2·4 wide, 32 + 2·4 high
    assert_eq!(r.size(), egui::vec2(184.0, 40.0));
    assert!(
        (r.min.x - card.min.x).abs() <= 1.0 && (r.min.y - (card.max.y + 2.0)).abs() <= 1.0,
        "{r:?} under {card:?}"
    );
    let dim = sdc_gui::ui::sets_column::dim_letter_colour(&h.ctx.global_style().visuals);
    let texts = painted_text_in(&h, r);
    for (i, l) in ["G", "P", "CNR", "S"].into_iter().enumerate() {
        let (_, at, c) = texts
            .iter()
            .find(|(t, ..)| t == l)
            .unwrap_or_else(|| panic!("{l} in the strip: {texts:?}"));
        assert_eq!(*c, dim, "{l} is dim");
        let cell = egui::Rect::from_min_size(
            r.min + egui::vec2(4.0 + i as f32 * 48.0, 4.0),
            egui::vec2(32.0, 32.0),
        );
        let (off, expected) = if l.len() == 1 {
            (at.min - cell.min, d_text.min - d_chip.min)
        } else {
            (
                at.left_bottom() - cell.left_bottom(),
                d_text.left_bottom() - d_chip.left_bottom(),
            )
        };
        assert!(
            (off - expected).length() <= 1.0,
            "{l}: offset {off:?} against the D chip's {expected:?}"
        );
    }
    assert_eq!(card_of(&h, ROCK), card, "the card did not change size");
    egui::DragAndDrop::clear_payload(&h.ctx);
    h.run();
    assert_eq!(
        h.state().1.drops.strip,
        None,
        "the strip goes with the drag"
    );
    hover_with(
        &mut h,
        DragBorrow {
            item: NORMAL.into(),
            from: ROCK.into(),
        },
        card.center(),
    );
    assert!(
        h.state().1.drops.strip.is_some(),
        "a dragged share is an image too"
    );
    egui::DragAndDrop::clear_payload(&h.ctx);
}

/// Review Focus 5 (owner: "Floating strip"; FM32): no card's measured height or rect moves during a drag.
#[test]
fn no_card_changes_height_while_an_image_is_dragged() {
    use sdc_gui::ui::sets_column::DragItem;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(three_cards(d.path()));
    h.run();
    h.run();
    let before = drawn_cards(&h);
    assert!(
        before.len() >= 2,
        "premise: several cards are drawn: {before:?}"
    );
    let heights = h.state().1.drops.card_h.clone();
    let at = card_of(&h, ROCK).center();
    hover_with(&mut h, DragItem(LEAF.into()), at);
    assert!(
        h.state().1.drops.strip.is_some(),
        "premise: the strip is up"
    );
    assert_eq!(h.state().1.drops.card_h, heights);
    assert_eq!(drawn_cards(&h), before, "every drawn card keeps its rect");
    egui::DragAndDrop::clear_payload(&h.ctx);
}

/// Review Focus 1, ruling A4-4: "a release on the strip goes to the STRIP'S OWN card (the hovered card whose empty
/// slots it shows); a card the strip happens to cover takes nothing". The covered card shows no outline either.
#[test]
fn a_drop_on_the_strip_goes_to_its_card_not_the_one_under_it() {
    use sdc_gui::ui::sets_column::DragItem;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(three_cards(d.path()));
    h.run();
    h.run();
    let cards = drawn_cards(&h);
    // the first drawn card with an empty slot whose strip covers another drawn card
    let mut found = None;
    for (id, card) in &cards {
        hover_with(&mut h, DragItem(LEAF.into()), card.center());
        let strip = h.state().1.drops.strip.clone().map(|s| s.1);
        egui::DragAndDrop::clear_payload(&h.ctx);
        h.run();
        let Some(strip) = strip else { continue };
        if let Some((under, _)) = cards
            .iter()
            .find(|(o, r)| o != id && r.contains(strip.center()))
        {
            found = Some((id.clone(), *card, strip, under.clone()));
            break;
        }
    }
    let (owner, card, strip, under) = found.expect("premise: a strip covers another card");
    // The payload is a member of the covered card. Dropped on the covered card it would change nothing (it is
    // already there), so only a drop on the strip's card can make `landed` the owner.
    // The drag starts on the member's own Textures-column widget, so the payload is the real one: a press on a chip of
    // the owner's card would start a drag of that chip's own member instead.
    let member = view(&h.state().0, &under).members[0].clone();
    let source = h.state().0.item(&member).unwrap();
    let from = if source.helper.is_some() {
        helper_source(&h, &source.name)
    } else {
        outside_sets(&h, &source.name).rect().center()
    };
    let active = h.ctx.global_style().visuals.widgets.active.bg_stroke;
    drag_over(&mut h, from, card.center(), egui::Modifiers::NONE);
    assert!(
        egui::DragAndDrop::payload::<DragItem>(&h.ctx).is_some_and(|p| p.0 == member),
        "premise: {member} is in flight"
    );
    let edge = if strip.center().y > card.center().y {
        (card.bottom() + strip.top()) / 2.0
    } else {
        (card.top() + strip.bottom()) / 2.0
    };
    let gap = egui::pos2(strip.center().x, edge);
    assert!(
        !strip.contains(gap) && !card.contains(gap),
        "one frame strictly in the gap"
    );
    for p in [gap, strip.center(), strip.center()] {
        h.event(egui::Event::PointerMoved(p));
        h.run_steps(2);
        assert_eq!(
            h.state().1.drops.strip.as_ref().map(|s| s.0.as_str()),
            Some(owner.as_str())
        );
    }
    let under_rect = card_of(&h, &under);
    assert!(
        !shapes(&h).iter().any(|s| matches!(s, egui::Shape::Rect(r)
            if (r.rect.min - under_rect.min).length() < 1.0 && r.stroke == active)),
        "the covered card shows no drop outline"
    );
    assert_eq!(
        h.state().1.drops.strip.as_ref().map(|s| s.0.clone()),
        Some(owner.clone()),
        "the strip stays its card's"
    );
    h.event(pointer_button(strip.center(), false, egui::Modifiers::NONE));
    h.run_steps(3);
    let landed = match &h.state().1.drops.pending {
        Some(Pending::Replace { set, .. }) | Some(Pending::Choice { set, .. }) => Some(set.clone()),
        Some(other) => panic!("{other:?}"),
        None => set_views(&h.state().0)
            .into_iter()
            .find(|v| v.members.contains(&member))
            .map(|v| v.id),
    };
    assert_eq!(
        landed.as_deref(),
        Some(owner.as_str()),
        "dropped on the strip's card, not {under}"
    );
}

/// Note JW1: when the strip flips up, the covered card draws first and must leave the release for its owner.
#[test]
fn a_flipped_strip_takes_the_drop_before_the_card_under_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_cards(d.path());
    assert!(m.reorder_set(ROCK, 1));
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    let (under, first) = drawn_cards(&h).into_iter().next().unwrap();
    assert_ne!(under, ROCK);
    let bottom = first.top() + h.state().1.drops.view.unwrap().1;
    let height = 1000.0 - (bottom - card_of(&h, ROCK).bottom()) + 8.0;
    h.set_size(egui::vec2(1600.0, height));
    h.run();
    let card = card_of(&h, ROCK);
    let member = view(&h.state().0, &under).members[0].clone();
    let from = outside_sets(&h, &h.state().0.item(&member).unwrap().name)
        .rect()
        .center();
    drag_over(&mut h, from, card.center(), egui::Modifiers::NONE);
    let (_, strip, _) = h.state().1.drops.strip.clone().expect("strip on rock01");
    assert!(
        strip.bottom() <= card.top() && card_of(&h, &under).contains(strip.center()),
        "{strip:?} above {card:?}"
    );
    h.hover_at(egui::pos2(
        strip.center().x,
        (strip.bottom() + card.top()) / 2.0,
    ));
    h.run_steps(2);
    h.hover_at(strip.center());
    h.run_steps(2);
    assert_eq!(
        h.state().1.drops.strip.as_ref().map(|s| s.0.as_str()),
        Some(ROCK)
    );
    h.event(pointer_button(strip.center(), false, egui::Modifiers::NONE));
    h.run_steps(3);
    assert!(
        matches!(&h.state().1.drops.pending, Some(Pending::Choice { set, .. } | Pending::Replace { set, .. }) if set == ROCK),
        "the earlier card left the drop for rock01: {:?}",
        h.state().1.drops.pending
    );
}

/// Note JW1: a borrowed payload released on the strip still uses the sharing path.
#[test]
fn a_borrow_released_on_the_strip_keeps_its_lender() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_cards(d.path());
    let leaf = "landscape/leaf";
    assert_eq!(
        m.drop_choice(NORMAL, leaf, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    assert!(!view(&m, ROCK).empty.is_empty());
    let mut h = harness(m);
    h.run();
    let from = on_card_of(&h, leaf, "N")[0].rect().center();
    let card = card_of(&h, ROCK);
    drag_over(&mut h, from, card.center(), egui::Modifiers::NONE);
    let (owner, strip, _) = h
        .state()
        .1
        .drops
        .strip
        .clone()
        .expect("the borrowed chip opens the strip");
    assert_eq!(owner, ROCK);
    h.hover_at(strip.center());
    h.run_steps(2);
    h.event(pointer_button(strip.center(), false, egui::Modifiers::NONE));
    h.run_steps(3);
    assert!(h.state().1.drops.pending.is_none());
    assert_eq!(
        h.state().0.status_line,
        "Nothing to share: rock01_normal.png is already in landscape/rock01."
    );
}

fn borrow_across_strip_edge(released_inside: bool) {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_cards(d.path());
    let leaf = "landscape/leaf";
    let odd = "landscape/odd";
    assert_eq!(
        m.drop_choice(NORMAL, leaf, DropChoice::Share),
        DropOutcome::Joined
    );
    m.set_shader(odd, Some(engine::shader::Shader::Pbr));
    idle(&mut m);
    let before = documents(&m);
    let mut h = harness(m);
    h.run();
    let from = on_card_of(&h, leaf, "N")[0].rect().center();
    let card = card_of(&h, odd);
    drag_over(&mut h, from, card.center(), egui::Modifiers::NONE);
    let (owner, strip, bridge) = h.state().1.drops.strip.clone().expect("strip on odd");
    assert_eq!(owner, odd);
    let inside = strip.left_center() + egui::vec2(1.0, 0.0);
    let outside = strip.left_center() - egui::vec2(2.0, 0.0);
    assert!(!bridge.contains(outside) && !card.contains(outside));
    let (release, later) = if released_inside {
        (inside, outside)
    } else {
        (outside, inside)
    };
    h.input_mut().events.extend([
        pointer_button(release, false, egui::Modifiers::NONE),
        egui::Event::PointerMoved(later),
    ]);
    h.step();
    assert_eq!(h.state().1.drops.pending, None);
    if released_inside {
        idle(&mut h.state_mut().0);
        assert!(
            matches!(view(&h.state().0, odd).slots.get(&SlotKind::Normal),
            Some(SlotFill::Borrowed { from, item }) if from == ROCK && item == NORMAL),
            "a release inside the strip shares into its owner"
        );
    } else {
        assert_eq!(
            documents(&h.state().0),
            before,
            "moving onto the strip after release cannot share"
        );
    }
}

#[test]
fn a_borrow_crossing_the_strip_after_release_outside_leaves_membership_alone() {
    borrow_across_strip_edge(false);
}

#[test]
fn a_borrow_crossing_the_strip_after_release_inside_shares_into_its_owner() {
    borrow_across_strip_edge(true);
}

#[test]
fn a_release_on_an_uncovered_card_stays_there_after_a_move_onto_the_strip() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(three_cards(d.path()));
    h.run();
    let from = outside_sets(&h, "rock01_normal.png").rect().center();
    let rock = card_of(&h, ROCK);
    drag_over(&mut h, from, rock.center(), egui::Modifiers::NONE);
    let (owner, strip, bridge) = h.state().1.drops.strip.clone().expect("strip on rock01");
    assert_eq!(owner, ROCK);
    let odd = "landscape/odd";
    let release = card_of(&h, odd).center();
    assert!(!strip.contains(release) && !bridge.contains(release));
    h.input_mut().events.extend([
        pointer_button(release, false, egui::Modifiers::NONE),
        egui::Event::PointerMoved(strip.center()),
    ]);
    h.step();
    assert_eq!(h.state().1.drops.pending, Some(choice(NORMAL, odd, ROCK)));
}

/// Note JW1: a strip covering the new-set area keeps both the hover and the release.
#[test]
fn the_new_set_area_stays_inactive_under_the_strip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = three_cards(d.path());
    m.sets_filter.search = ROCK.into();
    let mut h = harness(m);
    h.run();
    let card = card_of(&h, ROCK);
    let from = outside_sets(&h, "leaf.png").rect().center();
    drag_over(&mut h, from, card.center(), egui::Modifiers::NONE);
    let (_, strip, _) = h
        .state()
        .1
        .drops
        .strip
        .clone()
        .expect("strip below the filtered card");
    let label = h.get_by_label("Drop here to start a new set").rect();
    let overlap = strip.intersect(label);
    assert!(
        overlap.is_positive(),
        "strip {strip:?} over new-set hint {label:?}"
    );
    h.hover_at(overlap.center());
    h.run_steps(3);
    let active = h.ctx.global_style().visuals.widgets.active.bg_stroke;
    assert!(!shapes(&h).iter().any(|s| matches!(s, egui::Shape::Rect(r) if r.rect.contains_rect(label) && r.stroke == active)), "covered new-set drop zone is inactive");
    let hints = painted_text_in(&h, label);
    assert!(
        hints
            .iter()
            .any(|(t, _, c)| t == "Drop here to start a new set"
                && *c == h.ctx.global_style().visuals.weak_text_color()),
        "covered new-set hint stays dim: {hints:?}"
    );
    h.event(pointer_button(
        overlap.center(),
        false,
        egui::Modifiers::NONE,
    ));
    h.run_steps(3);
    assert!(
        matches!(&h.state().1.drops.pending, Some(Pending::Choice { set, .. } | Pending::Replace { set, .. }) if set == ROCK),
        "strip release targets {ROCK}: {:?}",
        h.state().1.drops.pending
    );
}

/// Note JW1: a read-only card still identifies its empty slots during a drag.
#[test]
fn a_read_only_card_shows_the_strip_without_taking_a_drop() {
    use sdc_gui::ui::sets_column::DragItem;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    m.save().unwrap();
    std::fs::write(
        m.root.as_ref().unwrap().join("sdc-project.toml"),
        "[[sets]\nbroken",
    )
    .unwrap();
    m.reload_documents();
    idle(&mut m);
    assert!(m.sets_read_only_reason().is_some());
    let empties = view(&m, ROCK).empty_line;
    let letters = empties
        .strip_prefix("Empty: ")
        .expect("read-only set has empty slots");
    let before = documents(&m);
    let released = std::rc::Rc::new(std::cell::Cell::new(None));
    let probe = released.clone();
    let mut h = harness_observing(m, move |ctx| {
        if ctx.input(|i| i.pointer.any_released()) {
            probe.set(Some(egui::DragAndDrop::has_payload_of_type::<DragItem>(
                ctx,
            )));
        }
    });
    h.run();
    let card = card_of(&h, ROCK);
    let source = outside_sets(&h, "rock01_normal.png").rect().center();
    for on_strip in [true, false] {
        drag_over(&mut h, source, card.center(), egui::Modifiers::NONE);
        let (id, strip, _) = h
            .state()
            .1
            .drops
            .strip
            .clone()
            .expect("strip on read-only card");
        assert_eq!(id, ROCK);
        let painted = painted_text_in(&h, strip);
        assert!(
            painted.iter().any(|(text, ..)| text == "P"),
            "Height is empty on the reloaded read-only card and must paint P"
        );
        for letter in letters.split(", ") {
            assert!(
                painted.iter().any(|(t, ..)| t == letter),
                "{letter} on the read-only strip"
            );
        }
        let at = if on_strip {
            strip.center()
        } else {
            card.center()
        };
        h.hover_at(at);
        h.run_steps(2);
        assert_eq!(
            h.state().1.drops.strip.as_ref().map(|s| s.0.as_str()),
            Some(ROCK)
        );
        let active = h.ctx.global_style().visuals.widgets.active.bg_stroke;
        assert!(!shapes(&h)
            .iter()
            .any(|s| matches!(s, egui::Shape::Rect(r) if r.rect == card && r.stroke == active)));
        released.set(None);
        h.event(pointer_button(at, false, egui::Modifiers::NONE));
        h.run_steps(3);
        assert_eq!(
            released.get(),
            Some(true),
            "the read-only target leaves the payload in flight on release"
        );
        assert!(h.state().1.drops.pending.is_none());
        assert_eq!(documents(&h.state().0), before);
    }
}

fn documents(m: &AppModel) -> (String, String, usize) {
    (
        m.project.as_ref().unwrap().text(),
        m.sets_doc.as_ref().unwrap().text(),
        m.history.undo.len(),
    )
}

fn normal_fill(m: &AppModel, id: &str) -> Option<SlotFill> {
    set_views(m)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap()
        .slots
        .get(&SlotKind::Normal)
        .cloned()
}

fn drag_normal(h: &mut H, leaf: &str, dest: &str, modifiers: egui::Modifiers) {
    let from = on_card_of(h, leaf, "N")[0].rect().center();
    let to = card_of(h, dest).center();
    drag_to(h, from, to, modifiers);
}

fn click_replace(h: &mut H, copy: &str) {
    let name = h.state().0.item(copy).unwrap().name.clone();
    h.get_by_role_and_label(Role::Button, &format!("Replace {name}"))
        .click();
    h.run();
    idle(&mut h.state_mut().0);
    assert_eq!(h.state().1.drops.pending, None);
}

/// Note JW4 (spec A6, "Replace still asks"): a shared chip dropped on a card whose slot is filled opens Replace in the
/// UI, and its answer is a Share.
#[test]
fn a_shared_chip_dropped_on_a_filled_slot_opens_replace_as_a_share() {
    let d = tempfile::tempdir().unwrap();
    let (m, leaf, normal, other, copy) = filled_normal(d.path());
    let before = documents(&m);
    let selection = (m.selection.clone(), m.multi.clone());
    let mut h = harness(m);
    h.run();
    drag_normal(&mut h, &leaf, &other, egui::Modifiers::NONE);
    assert_eq!(
        h.state().1.drops.pending,
        Some(Pending::BorrowReplace {
            item: normal.clone(),
            from: ROCK.into(),
            set: other.clone(),
            kind: SlotKind::Normal,
            existing: BorrowOccupant::Own { item: copy.clone() }
        })
    );
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(h.state().1.drops.pending, None);
    assert_eq!(documents(&h.state().0), before);
    assert_eq!(
        (h.state().0.selection.clone(), h.state().0.multi.clone()),
        selection
    );
    drag_normal(&mut h, &leaf, &other, egui::Modifiers::NONE);
    click_replace(&mut h, &copy);
    assert_eq!(
        normal_fill(&h.state().0, &other),
        Some(SlotFill::Borrowed {
            item: normal,
            from: ROCK.into()
        })
    );
    assert_eq!(h.state().0.history.undo.len(), before.2 + 1);
    h.state_mut().0.undo();
    idle(&mut h.state_mut().0);
    assert_eq!(
        normal_fill(&h.state().0, &other),
        Some(SlotFill::Own { item: copy })
    );
}

#[test]
fn a_borrow_dropped_on_a_locked_card_opens_the_refusal() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, leaf, _) = leaf_borrows_normal(d.path());
    let odd = key_of(&m, "odd.png");
    let dest = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    m.set_slot_lock(&dest, 2, true).unwrap();
    idle(&mut m);
    let before = documents(&m);
    let mut h = harness(m);
    h.run();
    drag_normal(&mut h, &leaf, &dest, egui::Modifiers::NONE);
    assert_eq!(
        h.state().1.drops.pending,
        Some(Pending::Refused(format!(
            "slot 2 of {dest} is locked; unlock it first"
        )))
    );
    assert_eq!(documents(&h.state().0), before);
}

#[test]
fn replacing_before_the_source_move_lands_keeps_both_documents_and_undo() {
    let d = tempfile::tempdir().unwrap();
    let (m, leaf, normal, other, copy) = filled_normal(d.path());
    let mut h = harness(m);
    h.run();
    drag_normal(&mut h, &leaf, &other, egui::Modifiers::NONE);
    assert!(matches!(
        h.state().1.drops.pending,
        Some(Pending::BorrowReplace { .. })
    ));
    h.state_mut()
        .0
        .edit_docs("Move the source", None, true, |_, s| {
            s.create_set("landscape/new_lender");
            s.add_member("landscape/new_lender", &normal);
        });
    let before = documents(&h.state().0);
    let name = h.state().0.item(&copy).unwrap().name.clone();
    h.get_by_role_and_label(Role::Button, &format!("Replace {name}"))
        .click();
    h.run();
    assert_eq!(h.state().1.drops.pending, None);
    let m = &mut h.state_mut().0;
    assert!(m.full_plan_pending(), "the harness has not polled the move");
    assert_eq!(
        (m.status_line.as_str(), documents(m)),
        (
            "The shared source changed. Drag the chip again.",
            before.clone()
        )
    );
    assert_eq!(
        m.drop_borrow(&normal, ROCK, &other),
        Err("The shared source changed. Drag the chip again.".into())
    );
    assert_eq!(documents(m), before);
}

#[test]
fn retyping_the_shared_normal_to_height_while_replace_is_open_keeps_both_slots() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, leaf, normal) = leaf_borrows_normal(d.path());
    let dest = "landscape/record";
    let path = "textures/black.dds";
    let map = sdc_gui::sets_doc::TxstMap {
        plugin: "a.esp".into(),
        formid: "1".into(),
        edid: "E".into(),
        slots: [(2, path.into()), (4, path.into())].into(),
        base: None,
    };
    m.edit_docs("Set up record slots", None, true, |_, s| {
        s.create_set(dest);
        s.set_txst(dest, Some(&map));
    });
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    drag_normal(&mut h, &leaf, dest, egui::Modifiers::NONE);
    assert!(
        matches!(&h.state().1.drops.pending, Some(Pending::BorrowReplace {
        existing: BorrowOccupant::Record { path: old }, ..
    }) if old == path)
    );
    h.state_mut()
        .0
        .set_field(&normal, "subtype", serde_json::json!("height"));
    idle(&mut h.state_mut().0);
    let cards = set_views(&h.state().0);
    let owner = cards.iter().find(|v| v.id == ROCK).unwrap();
    assert_eq!(
        slot_shown(owner, SlotKind::Height),
        SlotShown::Own { item: &normal }
    );
    let card = cards.iter().find(|v| v.id == dest).unwrap();
    for kind in [SlotKind::Normal, SlotKind::Height] {
        assert_eq!(slot_shown(card, kind), SlotShown::Record { path });
    }
    let before = documents(&h.state().0);
    h.get_by_role_and_label(Role::Button, &format!("Replace {path}"))
        .click();
    h.run();
    assert_eq!(h.state().1.drops.pending, None);
    assert_eq!(
        (h.state().0.status_line.as_str(), documents(&h.state().0)),
        ("The destination slot changed. Drag the chip again.", before)
    );
}

#[test]
fn a_borrow_replace_click_refuses_a_now_owned_file() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, leaf, normal, other, copy) = filled_normal(d.path());
    let recipient = "landscape/recipient";
    m.edit_docs("Create recipient", None, true, |_, s| {
        s.create_set(recipient);
        s.set_borrow(recipient, 2, Some(&other));
    });
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    drag_normal(&mut h, &leaf, recipient, egui::Modifiers::NONE);
    assert_eq!(
        h.state().1.drops.pending,
        Some(Pending::BorrowReplace {
            item: normal,
            from: ROCK.into(),
            set: recipient.into(),
            kind: SlotKind::Normal,
            existing: BorrowOccupant::Borrowed {
                item: copy.clone(),
                from: other
            }
        })
    );
    h.state_mut()
        .0
        .edit_docs("Make normal local", None, true, |_, s| {
            s.set_borrow(recipient, 2, None);
            s.add_member(recipient, &copy);
        });
    idle(&mut h.state_mut().0);
    let own = Some(SlotFill::Own { item: copy.clone() });
    assert_eq!(normal_fill(&h.state().0, recipient), own);
    let before = documents(&h.state().0);
    click_replace(&mut h, &copy);
    assert_eq!(
        h.state().0.status_line,
        "The destination slot changed. Drag the chip again."
    );
    assert_eq!(documents(&h.state().0), before);
    assert_eq!(normal_fill(&h.state().0, recipient), own);
}

#[test]
fn a_borrow_drop_uses_the_keyless_cards_document_id() {
    let d = tempfile::tempdir().unwrap();
    let mut m = key_less_json_sets(d.path());
    let (leaf, other) = ("landscape/empty", "landscape/other");
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("Share normal", None, true, |_, s| {
        s.set_borrow(leaf, 2, Some(ROCK))
    });
    idle(&mut m);
    let shared = SlotShown::Borrowed {
        item: &normal,
        from: ROCK,
    };
    let view = |m: &AppModel, id: &str| set_views(m).into_iter().find(|v| v.id == id).unwrap();
    assert_eq!(slot_shown(&view(&m, leaf), SlotKind::Normal), shared);
    let dest = view(&m, other);
    assert!(dest.created && dest.key.is_empty() && !dest.slots.contains_key(&SlotKind::Normal));
    m.select_set(&view(&m, ROCK).key);
    let mut selection = (m.selection.clone(), m.multi.clone());
    // The one Share replan invalidates the selection cache in prune_selection.
    selection.1.selection_rev += 1;
    let steps = m.history.undo.len();
    let mut h = harness(m);
    h.run();
    drag_normal(&mut h, leaf, other, egui::Modifiers::SHIFT);
    assert_eq!(h.state().1.drops.pending, None);
    idle(&mut h.state_mut().0);
    let m = &h.state().0;
    assert_eq!((m.selection.clone(), m.multi.clone()), selection);
    assert_eq!(m.history.undo.len(), steps + 1);
    let entries = m.sets_doc.as_ref().unwrap().entries();
    for id in [leaf, other] {
        assert_eq!(slot_shown(&view(m, id), SlotKind::Normal), shared);
        let entry = entries.iter().find(|e| e.id == id).unwrap();
        assert_eq!(entry.borrow.get(&2).map(String::as_str), Some(ROCK));
    }
    let path = r"textures\other\guard.dds";
    h.state_mut()
        .0
        .set_slot_command(other, 2, SlotCommand::Path(path.into()))
        .unwrap();
    idle(&mut h.state_mut().0);
    h.run();
    let before = documents(&h.state().0);
    drag_normal(&mut h, leaf, other, egui::Modifiers::NONE);
    assert_eq!(h.state().1.drops.pending, None);
    assert_eq!(
        h.state().0.status_line,
        format!("{other}'s Normal _n is linked to {path}: Unlink it first.")
    );
    assert_eq!(documents(&h.state().0), before);
}

/// Note JW4 (A6-2): a share held over the new-set zone lights nothing there, and a release there writes nothing.
#[test]
fn a_share_over_the_new_set_zone_draws_no_highlight() {
    let d = tempfile::tempdir().unwrap();
    let (m, leaf, _normal) = leaf_borrows_normal(d.path());
    let steps = m.history.undo.len();
    let mut h = harness(m);
    h.run();
    h.run();
    let from = on_card_of(&h, &leaf, "N")[0].rect().center();
    let zone = h.get_by_label("Drop here to start a new set").rect();
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.hover_at(from);
    h.run();
    h.event(press(from, true));
    h.run();
    for step in 1..=6 {
        h.event(egui::Event::PointerMoved(
            from + (zone.center() - from) * (step as f32 / 6.0),
        ));
        h.run_steps(2);
    }
    assert!(
        egui::DragAndDrop::has_payload_of_type::<sdc_gui::ui::sets_column::DragBorrow>(&h.ctx),
        "premise: a share is in flight"
    );
    let active = h.ctx.global_style().visuals.widgets.active.bg_stroke;
    let lit = shapes(&h).iter().any(
        |s| matches!(s, egui::Shape::Rect(r) if r.rect.contains_rect(zone) && r.stroke == active),
    );
    assert!(
        !lit,
        "the new-set zone draws no drop highlight under a share"
    );
    h.event(press(zone.center(), false));
    h.run_steps(3);
    assert_eq!(h.state().1.drops.pending, None);
    assert_eq!(h.state().0.history.undo.len(), steps, "nothing written");
}

/// Note JW4: dropped on its source's card, a shared chip changes nothing and says why.
#[test]
fn a_shared_chip_dropped_on_its_source_says_so() {
    let d = tempfile::tempdir().unwrap();
    let (m, leaf, _normal) = leaf_borrows_normal(d.path());
    let steps = m.history.undo.len();
    let mut h = harness(m);
    h.run();
    h.run();
    let from = on_card_of(&h, &leaf, "N")[0].rect().center();
    let to = card_of(&h, ROCK).center();
    drag_to(&mut h, from, to, egui::Modifiers::NONE);
    assert_eq!(h.state().1.drops.pending, None);
    assert_eq!(
        h.state().0.status_line,
        "Nothing to share: rock01_normal.png is already in landscape/rock01."
    );
    assert_eq!(h.state().0.history.undo.len(), steps);
}

const AO: &str = "a/rock_ao.png";
const A: &str = "a/rock";
const B: &str = "b/stone";

/// `textures/<f>` for each of `files` (a colour ramp for a name with no `_`, a grey ramp otherwise), `toml` as the
/// `sdc.toml`, opened as the app opens a folder.
fn fixture(dir: &Path, files: &[&str], toml: &str) -> AppModel {
    let root = dir.join("textures");
    for f in files {
        let p = root.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        if p.file_stem().unwrap().to_string_lossy().contains('_') {
            ImageBuffer::from_fn(64, 64, |x, y| Luma([(x * 3 + y) as u8]))
                .save(p)
                .unwrap();
        } else {
            ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
                .save(p)
                .unwrap();
        }
    }
    if !toml.is_empty() {
        std::fs::write(root.join("sdc.toml"), toml).unwrap();
    }
    let mut m = opened_as_the_app(root, dir.join("export"));
    idle(&mut m);
    m
}

/// Task 1's fixture: set `a` (rock, rock_ao) and set `b` (stone, stone_roughness).
fn ab(dir: &Path) -> AppModel {
    fixture(
        dir,
        &["a/rock.png", AO, "b/stone.png", "b/stone_roughness.png"],
        "",
    )
}

/// `ab` with rock_ao moved out to no set.
fn no_set_ao(dir: &Path) -> AppModel {
    let mut m = ab(dir);
    m.remove_member(A, AO).expect("removed");
    idle(&mut m);
    assert!(m.is_no_set(AO));
    m
}

type H = Harness<'static, (AppModel, State)>;

fn harness(m: AppModel) -> H {
    harness_observing(m, |_| {})
}

fn harness_observing(m: AppModel, after_draw: impl Fn(&egui::Context) + 'static) -> H {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(0.02)
        .build_ui_state(
            move |ui, (m, s): &mut (AppModel, State)| {
                sdc_gui::ui::draw_with(ui, m, s);
                after_draw(ui.ctx());
            },
            (m, State::default()),
        );
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h
}

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "model did not settle");
}

fn choice(item: &str, set: &str, from: &str) -> Pending {
    Pending::Choice {
        item: item.into(),
        set: set.into(),
        from: from.into(),
    }
}

/// Pinned before Step 3 replaces the hand-built button list: a no-set helper's dialog offers Move and Share today,
/// and still does after (A7-1 adds no button). Passes on the stub and after.
#[test]
fn a_no_set_helper_dialog_keeps_move_and_share() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(no_set_ao(d.path()));
    h.state_mut().1.drops.pending = Some(choice(AO, B, "no set"));
    h.run();
    h.run();
    let all = [
        "Move it here",
        "Share (use this file here too, no copy)",
        "Duplicate the source for this set",
    ];
    let offered: Vec<&str> = all
        .into_iter()
        .filter(|l| h.query_by_role_and_label(Role::Button, l).is_some())
        .collect();
    assert_eq!(
        offered,
        ["Move it here", "Share (use this file here too, no copy)"]
    );
}

/// Note JW5 (A7-1): the dialog for a helper from a real set offers Move and Share only.
#[test]
fn the_drop_dialog_offers_a_helper_no_duplicate() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(ab(d.path()));
    h.state_mut().1.drops.pending = Some(choice(AO, B, A));
    h.run();
    h.run();
    h.get_by_role_and_label(Role::Button, "Move it here");
    h.get_by_role_and_label(Role::Button, "Share (use this file here too, no copy)");
    assert!(h
        .query_by_label("Duplicate the source for this set")
        .is_none());
}

fn assert_refused_button(h: &mut H, dir: &Path, label: &str, reason: &str) {
    let files = || {
        walkdir::WalkDir::new(dir)
            .sort_by_file_name()
            .into_iter()
            .map(Result::unwrap)
            .filter(|e| e.file_type().is_file())
            .map(|e| (e.path().to_owned(), std::fs::read(e.path()).unwrap()))
            .collect::<Vec<_>>()
    };
    h.run();
    let before = files();
    let undo = h.state().0.history.undo.len();
    let button = h.get_by_role_and_label(Role::Button, label);
    assert!(button.accesskit_node().is_disabled(), "{label} is enabled");
    assert_eq!(
        button.accesskit_node().description().as_deref(),
        Some(reason)
    );
    button.hover();
    h.run_steps(4);
    h.get_by_role_and_label(Role::Label, reason);
    h.get_by_role_and_label(Role::Button, label).click();
    h.run();
    assert!(matches!(
        h.state().1.drops.pending,
        Some(Pending::Choice { .. })
    ));
    assert_eq!(h.state().0.history.undo.len(), undo);
    assert_eq!(files(), before);
}

#[test]
fn refused_button_for_a_non_raw_helper() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    m.items.iter_mut().find(|i| i.key == AO).unwrap().helper = None;
    let DropOutcome::AskChoice { from } = m.drop_on_set(AO, B) else {
        panic!("expected a choice for the helper");
    };
    let mut h = harness(m);
    h.state_mut().1.drops.pending = Some(choice(AO, B, &from));
    assert_refused_button(
        &mut h,
        d.path(),
        "Share (borrow its finished output)",
        "Only a raw helper can be shared as a file.",
    );
}

#[test]
fn refused_buttons_for_a_packed_map() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let packed = m
        .items
        .iter()
        .find(|i| i.name == "rock01_rmaos")
        .unwrap()
        .key
        .clone();
    let set = "landscape/leaf";
    let DropOutcome::AskChoice { from } = m.drop_on_set(&packed, set) else {
        panic!("expected a choice for the packed map");
    };
    let mut h = harness(m);
    h.state_mut().1.drops.pending = Some(choice(&packed, set, &from));
    assert_refused_button(
        &mut h,
        d.path(),
        "Duplicate the source for this set",
        "A packed map cannot be duplicated. Share its finished output or move its raw maps.",
    );
    assert_refused_button(
        &mut h,
        d.path(),
        "Move it here",
        "a packed map follows its raw maps; move those, or share the finished map",
    );
}

const NORMAL: &str = "landscape/rock01_normal.png";

/// `key_press` only queues the key; the run delivers it.
fn press_enter(h: &mut H) {
    h.key_press(egui::Key::Enter);
    h.run();
}

fn open_choice(h: &mut H, last: Option<DropChoice>, item: &str, set: &str) {
    h.state_mut().1.drops.last_choice = last;
    h.state_mut().1.drops.pending = Some(choice(item, set, ROCK));
    h.run();
    h.run();
}

fn remembered_share() -> (tempfile::TempDir, H, String) {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    open_choice(&mut h, Some(DropChoice::Share), NORMAL, &id);
    (d, h, id)
}

#[test]
fn the_default_button_uses_the_selection_fill() {
    let (_d, h, _) = remembered_share();
    let b = h
        .get_by_label_contains("Share (borrow its finished output)")
        .rect();
    let fill = h.ctx.global_style().visuals.selection.bg_fill;
    assert!(shapes(&h)
        .iter()
        .any(|s| matches!(s, egui::Shape::Rect(r) if r.rect == b && r.fill == fill)));
}

#[test]
fn the_default_button_explains_enter_and_shift_on_hover() {
    let (_d, mut h, _) = remembered_share();
    h.get_by_label_contains("Share (borrow its finished output)")
        .hover();
    h.run_steps(4);
    h.get_by_label(DEFAULT_HOVER);
}

#[test]
fn the_choice_dialog_shows_the_keyboard_hint() {
    let (_d, h, _) = remembered_share();
    h.get_by_label(DEFAULT_HINT);
}

/// Note JW6 (A8-2): the default is named and Enter writes the borrow.
#[test]
fn enter_picks_the_remembered_share() {
    let (_d, mut h, id) = remembered_share();
    let label = format!("Share (borrow its finished output){DEFAULT_SUFFIX}");
    h.get_by_role_and_label(Role::Button, &label);
    assert!(h
        .query_by_label(&format!("Move it here{DEFAULT_SUFFIX}"))
        .is_none());
    press_enter(&mut h);
    assert_eq!(h.state().1.drops.pending, None);
    idle(&mut h.state_mut().0);
    assert!(matches!(
        view(&h.state().0, &id).slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { .. })
    ));
}

/// Note JW6: Enter on Duplicate writes one new image. Undo removes the copy.
#[test]
fn enter_picks_a_remembered_duplicate() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    let before = files_under(d.path());
    open_choice(&mut h, Some(DropChoice::Duplicate), NORMAL, &id);
    press_enter(&mut h);
    idle(&mut h.state_mut().0);
    let after = files_under(d.path());
    let new: Vec<_> = after.iter().filter(|p| !before.contains(p)).collect();
    assert_eq!(new.len(), 1, "one copy: {new:?}");
    assert!(new[0].extension().is_some_and(|x| x == "png"), "{new:?}");
    h.state_mut().0.undo();
    idle(&mut h.state_mut().0);
    assert_eq!(files_under(d.path()), before, "Undo removes the copy");
}

#[test]
fn enter_cancels_unsaved_changes_above_choice() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    open_choice(&mut h, Some(DropChoice::Duplicate), NORMAL, &id);
    let before = documents(&h.state().0);
    let files = files_under(d.path());
    assert!(!h.state_mut().0.request_close());
    h.run();
    h.get_by_label("Unsaved changes");
    assert!(h.ctx.memory(|m| m.focused().is_none()));
    press_enter(&mut h);
    assert!(h.state().0.dialog.is_none(), "Enter cancelled the close");
    assert_eq!(h.state().1.drops.pending, Some(choice(NORMAL, &id, ROCK)));
    assert_eq!(files_under(d.path()), files);
    assert_eq!(documents(&h.state().0), before);
}

#[test]
fn enter_on_the_close_request_frame_leaves_choice_pending() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    open_choice(&mut h, Some(DropChoice::Duplicate), NORMAL, &id);
    let before = documents(&h.state().0);
    let files = files_under(d.path());
    assert!(!h.state_mut().0.request_close());
    h.input_mut().events.push(egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.step();
    assert_eq!(files_under(d.path()), files, "Choice must not duplicate");
    assert_eq!(documents(&h.state().0), before);
    assert_eq!(h.state().1.drops.pending, Some(choice(NORMAL, &id, ROCK)));
    h.run();
    h.get_by_label("Unsaved changes");
}

#[test]
fn escape_before_enter_in_one_frame_cancels_duplicate() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    open_choice(&mut h, Some(DropChoice::Duplicate), NORMAL, &id);
    h.get_by_label(&format!(
        "Duplicate the source for this set{DEFAULT_SUFFIX}"
    ));
    let files = files_under(d.path());
    let before = documents(&h.state().0);
    for key in [egui::Key::Escape, egui::Key::Enter] {
        h.input_mut().events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.step();
    assert_eq!(
        files_under(d.path()),
        files,
        "Escape must not create a copy"
    );
    assert_eq!(documents(&h.state().0), before);
    assert_eq!(h.state().1.drops.pending, None);
    assert_eq!(h.state().1.drops.last_choice, Some(DropChoice::Duplicate));
}

#[test]
fn choice_rechecks_only_when_its_inputs_change_or_it_reopens() {
    use sdc_gui::model::sets::DROP_PRECHECKS;
    let checks = || DROP_PRECHECKS.with(std::cell::Cell::get);
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = leaf_set(d.path());
    let odd = key_of(&m, "odd.png");
    let other = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    let start = checks();
    h.state_mut().1.drops.pending = Some(choice(NORMAL, &id, ROCK));
    h.step();
    let opened = checks();
    h.step();
    h.step();
    assert_eq!(checks(), opened, "two unchanged frames repeated prechecks");
    assert_eq!(opened - start, 3, "one check per offered answer");

    h.state_mut().0.replan();
    h.step();
    assert_eq!(checks() - opened, 3, "a new plan generation");
    idle(&mut h.state_mut().0);
    h.run();
    let settled = checks();
    h.state_mut().1.drops.pending = Some(choice("landscape/rock01.png", &id, ROCK));
    h.step();
    assert_eq!(checks() - settled, 3, "a different dragged item");
    h.state_mut().1.drops.pending = Some(choice("landscape/rock01.png", &other, ROCK));
    h.step();
    assert_eq!(checks() - settled, 6, "a different destination");
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(h.state().1.drops.pending, None);
    h.state_mut().1.drops.pending = Some(choice("landscape/rock01.png", &other, ROCK));
    h.step();
    assert_eq!(checks() - settled, 9, "a fresh opening of the same choice");
}

#[test]
fn enter_picks_a_remembered_move() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    open_choice(&mut h, Some(DropChoice::Move), NORMAL, &id);
    press_enter(&mut h);
    idle(&mut h.state_mut().0);
    assert!(view(&h.state().0, &id).members.iter().any(|k| k == NORMAL));
    assert!(!view(&h.state().0, ROCK).members.iter().any(|k| k == NORMAL));
}

/// Note JW6: Enter on the frame the dialog opens does nothing (the Enter that ended something else).
#[test]
fn enter_on_the_opening_frame_does_nothing() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    h.run();
    h.state_mut().1.drops.last_choice = Some(DropChoice::Share);
    h.state_mut().1.drops.pending = Some(choice(NORMAL, &id, ROCK));
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().1.drops.pending, Some(choice(NORMAL, &id, ROCK)));
}

/// Note JW6 (A8-1): a remembered answer refused for this item highlights nothing, and Enter does nothing.
#[test]
fn a_refused_remembered_answer_highlights_nothing_and_enter_does_nothing() {
    // a helper with Duplicate remembered (A7)
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(ab(d.path()));
    h.state_mut().1.drops.last_choice = Some(DropChoice::Duplicate);
    h.state_mut().1.drops.pending = Some(choice(AO, B, A));
    h.run();
    h.run();
    assert_eq!(h.query_all_by_label_contains(DEFAULT_SUFFIX).count(), 0);
    assert!(h.query_by_label(DEFAULT_HINT).is_none());
    press_enter(&mut h);
    assert_eq!(h.state().1.drops.pending, Some(choice(AO, B, A)));
    // Under the default target, rock01 packs rock01_rmaos. It can be shared into odd.
    let d2 = tempfile::tempdir().unwrap();
    let mut m = pbr_ready(d2.path());
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    let rmaos = key_of(&m, "rock01_rmaos");
    assert!(
        m.choice_refused(&rmaos, &id, DropChoice::Move).is_some(),
        "premise: Move of a packed map is refused"
    );
    let mut h = harness(m);
    open_choice(&mut h, Some(DropChoice::Move), &rmaos, &id);
    assert_eq!(h.query_all_by_label_contains(DEFAULT_SUFFIX).count(), 0);
    press_enter(&mut h);
    assert_eq!(h.state().1.drops.pending, Some(choice(&rmaos, &id, ROCK)));
}

/// Note JW6 (A8-2): after a real drop opens the question, Enter picks the remembered answer.
#[test]
fn a_real_drop_then_enter_picks_the_remembered_answer() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    h.run();
    h.run();
    h.state_mut().1.drops.last_choice = Some(DropChoice::Share);
    let from = outside_sets(&h, "rock01_normal.png").rect().center();
    let to = card_of(&h, &id).center();
    drag_to(&mut h, from, to, egui::Modifiers::NONE);
    assert_eq!(
        h.state().1.drops.pending,
        Some(choice(NORMAL, &id, ROCK)),
        "no Shift: it asks"
    );
    h.run(); // Deliver Enter after the opening frame.
    press_enter(&mut h);
    assert_eq!(h.state().1.drops.pending, None);
    idle(&mut h.state_mut().0);
    assert!(matches!(
        view(&h.state().0, &id).slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { .. })
    ));
}

/// Note JW6 (A8-2): a remembered Duplicate into a filled slot leads to Replace, and Enter never presses Replace.
#[test]
fn enter_never_presses_replace() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    let steps = h.state().0.history.undo.len();
    open_choice(
        &mut h,
        Some(DropChoice::Duplicate),
        "landscape/rock01.png",
        &id,
    );
    press_enter(&mut h);
    let Some(Pending::Replace { choice: c, set, .. }) = h.state().1.drops.pending.clone() else {
        panic!("Replace asks: {:?}", h.state().1.drops.pending)
    };
    assert_eq!((c, set.as_str()), (DropChoice::Duplicate, id.as_str()));
    press_enter(&mut h);
    assert!(
        matches!(h.state().1.drops.pending, Some(Pending::Replace { .. })),
        "Enter did nothing on Replace"
    );
    assert_eq!(h.state().0.history.undo.len(), steps, "nothing written");
}

#[test]
fn choosing_duplicate_is_remembered_even_if_replace_is_cancelled() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let before = documents(&m);
    let mut h = harness(m);
    open_choice(&mut h, Some(DropChoice::Move), "landscape/rock01.png", &id);
    h.get_by_role_and_label(Role::Button, "Duplicate the source for this set")
        .click();
    h.run();
    assert!(matches!(
        h.state().1.drops.pending,
        Some(Pending::Replace {
            choice: DropChoice::Duplicate,
            ..
        })
    ));
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(h.state().1.drops.pending, None);
    assert_eq!(h.state().1.drops.last_choice, Some(DropChoice::Duplicate));
    assert_eq!(documents(&h.state().0), before);
}

#[test]
fn clicking_share_with_empty_memory_remembers_share() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    open_choice(&mut h, None, NORMAL, &id);
    h.get_by_role_and_label(Role::Button, "Share (borrow its finished output)")
        .click();
    h.run();
    idle(&mut h.state_mut().0);
    assert_eq!(
        normal_fill(&h.state().0, &id),
        Some(SlotFill::Borrowed {
            item: NORMAL.into(),
            from: ROCK.into(),
        })
    );
    assert_eq!(h.state().1.drops.last_choice, Some(DropChoice::Share));
}

/// Note JW6: an answer is remembered; Cancel and a refused answer leave it as it was.
#[test]
fn an_answer_is_remembered_and_cancel_is_not() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = leaf_set(d.path());
    let odd = key_of(&m, "odd.png");
    let fresh = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    let mut h = harness(m);
    open_choice(&mut h, None, NORMAL, &id);
    h.get_by_role_and_label(Role::Button, "Duplicate the source for this set")
        .click();
    h.run();
    assert_eq!(h.state().1.drops.last_choice, Some(DropChoice::Duplicate));
    idle(&mut h.state_mut().0);
    h.state_mut().1.drops.pending = Some(choice(NORMAL, &fresh, ROCK));
    h.run();
    h.get_by_label(&format!(
        "Duplicate the source for this set{DEFAULT_SUFFIX}"
    ));
    let files = files_under(d.path());
    let before = documents(&h.state().0);
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(h.state().1.drops.pending, None);
    assert_eq!(h.state().1.drops.last_choice, Some(DropChoice::Duplicate));
    assert_eq!(
        files_under(d.path()),
        files,
        "Escape must not create a copy"
    );
    assert_eq!(documents(&h.state().0), before);
}

/// Note JW6 (A8-1): "for the session": opening another folder keeps the remembered answer.
#[test]
fn the_remembered_answer_survives_opening_another_folder() {
    let d = tempfile::tempdir().unwrap();
    let (m, _id) = leaf_set(d.path());
    let mut h = harness(m);
    h.state_mut().1.drops.last_choice = Some(DropChoice::Move);
    h.run();
    let d2 = tempfile::tempdir().unwrap();
    h.state_mut().0.open_folder(support::mod_folder(d2.path()));
    idle(&mut h.state_mut().0);
    h.run();
    h.run();
    assert_eq!(h.state().1.drops.last_choice, Some(DropChoice::Move));
}

fn release_then_change_shift(release: egui::Modifiers, after: egui::Modifiers) {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    h.run();
    h.state_mut().1.drops.last_choice = Some(DropChoice::Share);
    let from = outside_sets(&h, "rock01_normal.png").rect().center();
    let to = card_of(&h, &id).center();
    drag_over(&mut h, from, to, release);
    let before = documents(&h.state().0);
    // Harness::event draws each event separately. RawInput delivers both to this single step.
    h.input_mut().events.extend([
        pointer_button(to, false, release),
        egui::Event::ModifiersChanged(after),
    ]);
    h.step();
    if release.shift {
        assert_eq!(h.state().1.drops.pending, None, "Shift was held at release");
        idle(&mut h.state_mut().0);
        assert!(matches!(
            view(&h.state().0, &id).slots.get(&SlotKind::Normal),
            Some(SlotFill::Borrowed { .. })
        ));
    } else {
        assert_eq!(h.state().1.drops.pending, Some(choice(NORMAL, &id, ROCK)));
        assert_eq!(documents(&h.state().0), before);
    }
}

fn release(
    pos: egui::Pos2,
    button: egui::PointerButton,
    modifiers: egui::Modifiers,
) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button,
        pressed: false,
        modifiers,
    }
}

fn assert_release_frame_asks<const N: usize>(
    releases: impl FnOnce(egui::Pos2, egui::Pos2) -> [egui::Event; N],
) {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    h.run();
    h.state_mut().1.drops.last_choice = Some(DropChoice::Share);
    let from = outside_sets(&h, "rock01_normal.png").rect().center();
    let to = card_of(&h, &id).center();
    drag_over(&mut h, from, to, egui::Modifiers::NONE);
    let before = documents(&h.state().0);
    h.input_mut().events.extend(releases(from, to));
    h.step();
    assert_eq!(h.state().1.drops.pending, Some(choice(NORMAL, &id, ROCK)));
    assert_eq!(documents(&h.state().0), before);
}

#[test]
fn a_drop_asks_after_a_shift_secondary_release_then_a_plain_primary_release() {
    use egui::{Modifiers, PointerButton};
    assert_release_frame_asks(|outside, card| {
        [
            release(outside, PointerButton::Secondary, Modifiers::SHIFT),
            release(card, PointerButton::Primary, Modifiers::NONE),
        ]
    });
}

#[test]
fn a_drop_asks_without_one_primary_release_over_the_card() {
    use egui::{Modifiers, PointerButton};
    assert_release_frame_asks(|_, card| {
        [release(card, PointerButton::Secondary, Modifiers::SHIFT)]
    });
    assert_release_frame_asks(|_, card| {
        [
            release(card, PointerButton::Primary, Modifiers::SHIFT),
            release(card, PointerButton::Primary, Modifiers::SHIFT),
        ]
    });
    assert_release_frame_asks(|outside, card| {
        [
            release(outside, PointerButton::Primary, Modifiers::SHIFT),
            release(card, PointerButton::Secondary, Modifiers::NONE),
        ]
    });
}

#[test]
fn shift_changed_after_release_down_still_asks() {
    release_then_change_shift(egui::Modifiers::NONE, egui::Modifiers::SHIFT);
}

#[test]
fn shift_changed_after_release_up_still_shares() {
    release_then_change_shift(egui::Modifiers::SHIFT, egui::Modifiers::NONE);
}

fn drag_normal_to_odd(dir: &Path) -> H {
    let mut m = three_cards(dir);
    let odd = "landscape/odd";
    m.set_shader(odd, Some(engine::shader::Shader::Pbr));
    idle(&mut m);
    assert_eq!(m.choice_refused(NORMAL, odd, DropChoice::Share), None);
    let mut h = harness(m);
    h.run();
    h.state_mut().1.drops.last_choice = Some(DropChoice::Share);
    let from = outside_sets(&h, "rock01_normal.png").rect().center();
    let card = card_of(&h, odd);
    drag_over(&mut h, from, card.center(), egui::Modifiers::NONE);
    h
}

fn assert_strip_release_asks(events: impl FnOnce(egui::Pos2, egui::Pos2) -> Vec<egui::Event>) {
    let d = tempfile::tempdir().unwrap();
    let mut h = drag_normal_to_odd(d.path());
    let odd = "landscape/odd";
    let (owner, strip, bridge) = h.state().1.drops.strip.clone().expect("strip on odd");
    let at = strip.center();
    assert_eq!(owner, odd);
    assert!(!card_of(&h, odd).contains(at));
    let outside = card_of(&h, ROCK).right_center() - egui::vec2(2.0, 0.0);
    assert!(!strip.contains(outside) && !bridge.contains(outside));
    let before = documents(&h.state().0);
    h.input_mut().events.extend(events(at, outside));
    h.step();
    assert_eq!(h.state().1.drops.pending, Some(choice(NORMAL, odd, ROCK)));
    assert_eq!(documents(&h.state().0), before);
}

#[test]
fn two_primary_releases_on_the_strip_ask() {
    assert_strip_release_asks(|at, _| {
        vec![
            pointer_button(at, false, egui::Modifiers::SHIFT),
            pointer_button(at, false, egui::Modifiers::SHIFT),
        ]
    });
}

#[test]
fn a_secondary_only_release_on_the_strip_asks() {
    assert_strip_release_asks(|at, _| {
        vec![release(
            at,
            egui::PointerButton::Secondary,
            egui::Modifiers::SHIFT,
        )]
    });
}

#[test]
fn a_last_release_on_the_strip_asks_when_the_shift_primary_was_elsewhere() {
    assert_strip_release_asks(|at, outside| {
        vec![
            pointer_button(outside, false, egui::Modifiers::SHIFT),
            release(at, egui::PointerButton::Secondary, egui::Modifiers::NONE),
            egui::Event::PointerMoved(outside),
        ]
    });
}

fn shift_share_at_strip(bridge: bool) {
    let d = tempfile::tempdir().unwrap();
    let mut h = drag_normal_to_odd(d.path());
    let odd = "landscape/odd";
    let card = card_of(&h, odd);
    let (owner, strip, hit) = h.state().1.drops.strip.clone().expect("strip on odd");
    assert_eq!(owner, odd);
    let at = if bridge {
        let y = if strip.top() >= card.bottom() {
            (strip.top() + card.bottom()) / 2.0
        } else {
            (strip.bottom() + card.top()) / 2.0
        };
        let gap = egui::pos2(hit.center().x, y);
        assert!(hit.contains(gap) && !strip.contains(gap));
        gap
    } else {
        strip.center()
    };
    assert!(!card.contains(at));
    h.input_mut().events.extend([
        egui::Event::PointerMoved(at),
        pointer_button(at, false, egui::Modifiers::SHIFT),
        egui::Event::ModifiersChanged(egui::Modifiers::NONE),
    ]);
    h.step();
    assert_eq!(
        h.state().1.drops.pending,
        None,
        "Shift on the release uses Share"
    );
    idle(&mut h.state_mut().0);
    assert!(
        matches!(view(&h.state().0, odd).slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { from, item }) if from == ROCK && item == NORMAL)
    );
}

#[test]
fn a_shift_drop_on_the_strip_writes_the_remembered_share() {
    shift_share_at_strip(false);
}

#[test]
fn a_shift_drop_on_the_strip_bridge_writes_the_remembered_share() {
    shift_share_at_strip(true);
}

/// Note JW6 (A8-3): Shift held at the drop answers with the remembered Share: no dialog, the borrow is written.
#[test]
fn shift_at_the_drop_skips_the_question_with_the_remembered_answer() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    h.run();
    h.run();
    h.state_mut().1.drops.last_choice = Some(DropChoice::Share);
    let from = outside_sets(&h, "rock01_normal.png").rect().center();
    let to = card_of(&h, &id).center();
    drag_to(&mut h, from, to, egui::Modifiers::SHIFT);
    assert_eq!(h.state().1.drops.pending, None, "no question");
    idle(&mut h.state_mut().0);
    assert!(matches!(
        view(&h.state().0, &id).slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { .. })
    ));
}

/// Note JW6 (A8-3): with nothing remembered, or a refused answer, Shift still asks.
#[test]
fn shift_with_nothing_remembered_or_a_refused_answer_still_asks() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    let mut h = harness(m);
    h.run();
    h.run();
    let from = outside_sets(&h, "rock01_normal.png").rect().center();
    let to = card_of(&h, &id).center();
    drag_to(&mut h, from, to, egui::Modifiers::SHIFT);
    assert_eq!(
        h.state().1.drops.pending,
        Some(choice(NORMAL, &id, ROCK)),
        "nothing remembered: it asks"
    );
    h.key_press(egui::Key::Escape);
    h.run();
    // a helper with Duplicate remembered
    let d2 = tempfile::tempdir().unwrap();
    let mut h = harness(ab(d2.path()));
    h.run();
    h.run();
    h.state_mut().1.drops.last_choice = Some(DropChoice::Duplicate);
    let from = helper_source(&h, "rock_ao.png");
    let to = card_of(&h, B).center();
    let before = (
        documents(&h.state().0),
        h.state().0.selection.clone(),
        h.state().0.multi.clone(),
    );
    drag_to(&mut h, from, to, egui::Modifiers::SHIFT);
    assert_eq!(
        (
            documents(&h.state().0),
            h.state().0.selection.clone(),
            h.state().0.multi.clone()
        ),
        before
    );
    assert_eq!(
        h.state().1.drops.pending,
        Some(choice(AO, B, A)),
        "a refused answer: it asks"
    );
}

#[test]
fn shift_release_with_a_packed_map_and_remembered_move_still_asks() {
    use sdc_gui::model::sets::DropChoice::Move;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    let item = key_of(&m, "rock01_rmaos");
    let mut h = harness(m);
    h.run();
    h.run();
    h.state_mut().1.drops.last_choice = Some(Move);
    let snapshot = |h: &H| {
        let m = &h.state().0;
        (documents(m), m.selection.clone(), m.multi.clone())
    };
    let before = snapshot(&h);
    let at = outside_sets(&h, "rock01_rmaos").rect().center();
    let to = card_of(&h, &id).center();
    drag_to(&mut h, at, to, egui::Modifiers::SHIFT);
    assert_eq!(h.state().1.drops.pending, Some(choice(&item, &id, ROCK)));
    assert_eq!(snapshot(&h), before);
    for previous in [Move, DropChoice::Share] {
        h.state_mut().1.drops.last_choice = Some(previous);
        h.run();
        let b = h.get_by_role_and_label(Role::Button, "Move it here");
        assert!(b.accesskit_node().is_disabled());
        b.click();
        h.run();
        assert_eq!(h.state().1.drops.pending, Some(choice(&item, &id, ROCK)));
        assert_eq!(h.state().1.drops.last_choice, Some(previous));
        assert_eq!(snapshot(&h), before);
    }
}

/// Note JW6 (A8-5): Shift held from before the press still skips the question at the drop, and the drag selects
/// nothing (a tile range-selects on a click only).
#[test]
fn shift_held_from_before_the_press_skips_the_question_and_selects_nothing() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = leaf_set(d.path());
    let odd = key_of(&m, "odd.png");
    m.select_item(&odd);
    let mut h = harness(m);
    h.run();
    h.run();
    h.state_mut().1.drops.last_choice = Some(DropChoice::Share);
    let before = h.state().0.selected_items();
    let from = outside_sets(&h, "rock01_normal.png").rect().center();
    let to = card_of(&h, &id).center();
    drag_with_modifiers(
        &mut h,
        from,
        to,
        egui::Modifiers::SHIFT,
        egui::Modifiers::SHIFT,
    );
    assert_eq!(h.state().1.drops.pending, None, "no question");
    assert_eq!(
        h.state().0.selected_items(),
        before,
        "the drag range-selected nothing"
    );
    idle(&mut h.state_mut().0);
    assert!(matches!(
        view(&h.state().0, &id).slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { .. })
    ));
}

/// Note JW6 (A8-5): a Shift-click on a Textures tile, with no move, range-selects as today.
#[test]
fn a_shift_click_on_a_tile_still_range_selects() {
    let (d, d2) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (mut m, _id) = leaf_set(d.path());
    let (mut twin, _) = leaf_set(d2.path());
    let odd = key_of(&m, "odd.png");
    m.select_item(&odd);
    twin.select_item(&odd);
    twin.click_item(NORMAL, false, true);
    let want = twin.selected_items();
    assert!(want.len() >= 2, "premise: a range: {want:?}");
    let mut h = harness(m);
    h.run();
    h.run();
    let at = outside_sets(&h, "rock01_normal.png").rect().center();
    let click = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::SHIFT,
    };
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::SHIFT));
    h.run_steps(1);
    h.hover_at(at);
    h.run();
    h.event(click(true));
    h.event(click(false));
    h.run();
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.run();
    assert_eq!(h.state().0.selected_items(), want);
}

fn helper_source(h: &H, file: &str) -> egui::Pos2 {
    let row = format!("Helper {file}:");
    h.query_all_by_role(Role::Button)
        .filter(|n| !in_sets_column(h, n.rect()))
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with(&row))
        })
        .unwrap_or_else(|| panic!("helper row for {file} outside the Sets column"))
        .rect()
        .center()
}

fn view(m: &AppModel, id: &str) -> SetView {
    set_views(m).into_iter().find(|v| v.id == id).unwrap()
}

fn files_under(dir: &Path) -> Vec<std::path::PathBuf> {
    walkdir::WalkDir::new(dir)
        .sort_by_file_name()
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file() && e.path().extension().is_none_or(|x| x != "toml"))
        .map(|e| e.into_path())
        .collect()
}

/// Note JW6 (SC-13): Share explains why a memberless set cannot use a raw helper.
#[test]
fn share_into_an_empty_set_is_disabled_with_the_models_reason() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    let set = "e/empty";
    m.edit_docs("Create empty set", None, true, |_, s| s.create_set(set));
    idle(&mut m);
    assert_eq!(
        m.drop_on_set(AO, set),
        DropOutcome::AskChoice { from: A.into() }
    );
    let mut h = harness(m);
    h.state_mut().1.drops.pending = Some(choice(AO, set, A));
    assert_refused_button(
        &mut h,
        d.path(),
        "Share (use this file here too, no copy)",
        "e/empty has no texture of its own; move one in first",
    );
}

/// A failed copy leaves the last successful answer in place, including when Shift skips Choice.
#[test]
fn a_copy_error_keeps_the_remembered_answer_and_shows_the_reason() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = leaf_set(d.path());
    std::fs::remove_file(&m.item(NORMAL).unwrap().input).unwrap();
    let before = documents(&m);
    let mut h = harness(m);
    open_choice(&mut h, Some(DropChoice::Share), NORMAL, &id);
    h.get_by_role_and_label(Role::Button, "Duplicate the source for this set")
        .click();
    h.run();
    assert!(
        matches!(&h.state().1.drops.pending, Some(Pending::Refused(why)) if why.starts_with("could not copy"))
    );
    assert_eq!(h.state().1.drops.last_choice, Some(DropChoice::Share));
    h.key_press(egui::Key::Escape);
    h.run();
    h.state_mut().1.drops.last_choice = Some(DropChoice::Duplicate);
    let from = outside_sets(&h, "rock01_normal.png").rect().center();
    let to = card_of(&h, &id).center();
    drag_to(&mut h, from, to, egui::Modifiers::SHIFT);
    assert!(
        matches!(&h.state().1.drops.pending, Some(Pending::Refused(why)) if why.starts_with("could not copy"))
    );
    assert_eq!(h.state().1.drops.last_choice, Some(DropChoice::Duplicate));
    assert_eq!(documents(&h.state().0), before);
}
