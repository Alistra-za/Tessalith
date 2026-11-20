//! Notes JW1 to JW7: set cards and dragging (owner additions A4 to A9).
mod support;
use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::sets::{
    set_views, slot_shown, BorrowOccupant, DropChoice, DropOutcome, SlotCommand, SlotFill,
    SlotKind, SlotShown,
};
use sdc_gui::model::AppModel;
use sdc_gui::ui::sets_column::Pending;
use sdc_gui::ui::State;
use std::path::Path;
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01";

/// `pbr_ready` with a new set from leaf (pbr_albedo, Vanilla target) that borrows rock01's normal; returns the model,
/// that set's id, and the normal's key.
fn leaf_borrows_normal(dir: &std::path::Path) -> (sdc_gui::model::AppModel, String, String) {
    let mut m = pbr_ready(dir);
    let leaf = key_of(&m, "leaf.png");
    m.set_field(&leaf, "subtype", serde_json::json!("pbr_albedo"));
    idle(&mut m);
    let id = m.drop_new_set(&leaf).unwrap();
    idle(&mut m);
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    assert_eq!(
        m.drop_choice(&normal, &id, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    (m, id, normal)
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

/// Press at `at`, move to `to` in six steps (the payload pass and the painted one each), release with `release` held.
/// egui 0.37's `RawInput` has no `modifiers` field: `i.modifiers` changes only on `Event::ModifiersChanged`, and
/// keeps its value across frames (`input_state/mod.rs` l.395, l.434). Kittest runs one frame per queued event, so
/// the change lands in its own frame before the release, as `tests/ui_drive/steps.rs` (l.346-353) does.
fn drag_to(h: &mut H, at: egui::Pos2, to: egui::Pos2, release: egui::Modifiers) {
    let press = |pos, pressed, modifiers| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    };
    h.hover_at(at);
    h.run();
    h.event(press(at, true, egui::Modifiers::NONE));
    h.run();
    for step in 1..=6 {
        h.event(egui::Event::PointerMoved(
            at + (to - at) * (step as f32 / 6.0),
        ));
        h.run_steps(2);
    }
    h.event(egui::Event::ModifiersChanged(release));
    h.run_steps(1);
    h.event(press(to, false, release));
    h.run_steps(3);
    h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    h.run();
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
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(0.02)
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, State)| sdc_gui::ui::draw_with(ui, m, s),
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
