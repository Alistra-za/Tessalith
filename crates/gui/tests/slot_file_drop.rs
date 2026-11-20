//! Item 17 (Task 12): a file dropped from Explorer onto a set editor slot card fills that slot, through the move
//! command; nothing is ever copied.
mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use sdc_gui::model::sets::{set_views, DropOutcome, SlotKind, ONLY_FIRST_FILE};
use sdc_gui::model::AppModel;
use sdc_gui::ui::State;
use std::path::{Path, PathBuf};
use std::time::Duration;
use support::*;

const LEAF: &str = "landscape/leaf";
const ROCK: &str = "landscape/rock01";
const NORMAL: &str = "landscape/rock01_normal.png";

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)));
}

fn file(m: &AppModel, key: &str) -> PathBuf {
    m.root.as_ref().unwrap().join(key)
}

fn members(m: &AppModel, id: &str) -> Vec<String> {
    set_views(m)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap()
        .members
}

fn files_in(dir: &Path) -> usize {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter(|e| e.as_ref().is_ok_and(|e| e.file_type().is_file()))
        .count()
}

#[test]
fn a_file_inside_the_folder_fills_the_slot_as_one_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let before = (m.history.undo.len(), files_in(d.path()));
    let p = file(&m, NORMAL);
    let r = m.drop_file_on_slot(LEAF, SlotKind::Normal, &[p]);
    assert_eq!(r.outcome, DropOutcome::Joined);
    assert_eq!((r.item.as_deref(), r.note), (Some(NORMAL), None));
    idle(&mut m);
    assert!(members(&m, LEAF).contains(&NORMAL.to_string()));
    assert_eq!(
        (m.history.undo.len(), files_in(d.path())),
        (before.0 + 1, before.1)
    );
    m.undo();
    idle(&mut m);
    assert!(!members(&m, LEAF).contains(&NORMAL.to_string()));
}

#[test]
fn a_filled_slot_asks_first_and_a_locked_slot_refuses() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let leaf = file(&m, "landscape/leaf.png");
    let r = m.drop_file_on_slot(ROCK, SlotKind::Diffuse, &[leaf]);
    assert_eq!(
        r.outcome,
        DropOutcome::AskReplace {
            existing: "landscape/rock01.png".into()
        }
    );
    m.set_slot_lock(LEAF, 2, true).unwrap();
    idle(&mut m);
    let undo = m.history.undo.len();
    let r = m.drop_file_on_slot(LEAF, SlotKind::Normal, &[file(&m, NORMAL)]);
    assert!(
        matches!(&r.outcome, DropOutcome::Refused(why) if why.contains("locked")),
        "{r:?}"
    );
    assert_eq!(m.history.undo.len(), undo);
}

#[test]
fn a_file_drop_keeps_the_slot_7_clash_guard() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("textures/landscape");
    std::fs::create_dir_all(&dir).unwrap();
    for n in ["rock01_cnr.png", "zed_f.png"] {
        image::ImageBuffer::from_fn(16, 16, |_, _| image::Rgb([128u8, 128, 255]))
            .save(dir.join(n))
            .unwrap();
    }
    let mut m = pbr_opened(d.path());
    m.drop_choice(
        "landscape/rock01_cnr.png",
        ROCK,
        sdc_gui::model::sets::DropChoice::Move,
    );
    idle(&mut m);
    assert!(members(&m, ROCK).contains(&"landscape/rock01_cnr.png".to_string()));
    let undo = m.history.undo.len();
    let r = m.drop_file_on_slot(ROCK, SlotKind::Fuzz, &[file(&m, "landscape/zed_f.png")]);
    assert!(
        matches!(&r.outcome, DropOutcome::Refused(why) if why.contains("slot 7")),
        "{r:?}"
    );
    assert_eq!(m.history.undo.len(), undo);
}

#[test]
fn a_file_outside_the_folder_is_refused_and_never_copied() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let other = tempfile::tempdir().unwrap();
    let outside = other.path().join("brick_n.png");
    std::fs::copy(file(&m, NORMAL), &outside).unwrap();
    let n = files_in(d.path());
    let r = m.drop_file_on_slot(LEAF, SlotKind::Normal, &[outside]);
    assert_eq!(
        r.outcome,
        DropOutcome::Refused(
            "brick_n.png is outside the open folder; files are never copied".into()
        )
    );
    assert_eq!(files_in(d.path()), n, "nothing was copied in");
}

#[test]
fn several_files_take_the_first_and_a_file_for_another_slot_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let paths = [file(&m, NORMAL), file(&m, "landscape/leaf.png")];
    let r = m.drop_file_on_slot(LEAF, SlotKind::Normal, &paths);
    assert_eq!(r.note, Some(ONLY_FIRST_FILE));
    assert_eq!(r.item.as_deref(), Some(NORMAL));
    assert_eq!(r.outcome, DropOutcome::Joined);
    assert_eq!(ONLY_FIRST_FILE, "only the first file was used");
    let r = m.drop_file_on_slot(ROCK, SlotKind::Normal, &[file(&m, "landscape/leaf.png")]);
    assert!(
        matches!(&r.outcome, DropOutcome::Refused(why) if why.contains("Diffuse")),
        "{r:?}"
    );
}

#[derive(Debug)]
struct Dropped(PathBuf);

impl eframe::egui::DroppedFile for Dropped {
    fn path(&self) -> &Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

fn harness(m: AppModel) -> Harness<'static, (AppModel, State)> {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(
            |ui, (model, state): &mut (AppModel, State)| sdc_gui::ui::draw_with(ui, model, state),
            (m, State::default()),
        );
    h
}

fn drop_on(h: &mut Harness<'static, (AppModel, State)>, card: &str, path: PathBuf) {
    // K5: the slot row, not the set panel's Matching row of the same name.
    h.hover_at(slot_rect(h, card).center());
    h.run();
    h.input_mut()
        .dropped_files
        .push(std::sync::Arc::new(Dropped(path)));
    h.run();
}

#[test]
fn a_drop_on_a_slot_card_fills_it_and_an_outside_file_shows_the_error_line() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let key = set_views(&m)
        .into_iter()
        .find(|v| v.id == LEAF)
        .unwrap()
        .key;
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let p = file(&h.state().0, NORMAL);
    drop_on(&mut h, "Normal _n", p);
    idle(&mut h.state_mut().0);
    assert!(members(&h.state().0, LEAF).contains(&NORMAL.to_string()));
    let other = tempfile::tempdir().unwrap();
    let outside = other.path().join("brick.png");
    std::fs::write(&outside, b"x").unwrap();
    h.run();
    drop_on(&mut h, "Normal _n", outside);
    h.run();
    h.get_by_label("brick.png is outside the open folder; files are never copied");
    // A drop that landed on a slot does not also open the file's folder.
    assert_ne!(h.state().0.root.as_deref(), Some(other.path()));
}

#[test]
fn a_replace_asked_by_a_drop_shows_its_prompt_with_the_sets_column_hidden() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.settings.sets_hidden = true;
    let key = set_views(&m)
        .into_iter()
        .find(|v| v.id == ROCK)
        .unwrap()
        .key;
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let leaf = file(&h.state().0, "landscape/leaf.png");
    drop_on(&mut h, "Diffuse", leaf);
    h.run();
    h.get_by_label("Replace?");
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().1.drops.pending.is_none());
}

/// Trace-debug #51: a drop above the set editor's scroll area (the window header, Note FB2), over a card scrolled out of
/// view, opens the folder.
#[test]
fn drop_on_card_scrolled_out_of_view_opens_folder_not_slot() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let key = set_views(&m)
        .into_iter()
        .find(|v| v.id == LEAF)
        .unwrap()
        .key;
    m.select_set(&key);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        // A low window, so the column scrolls.
        .with_size(eframe::egui::vec2(1400.0, 240.0))
        .build_ui_state(
            |ui, (model, state): &mut (AppModel, State)| sdc_gui::ui::draw_with(ui, model, state),
            (m, State::default()),
        );
    h.run();
    let card = slot_rect(&h, "Diffuse");
    // Note FB2: the set editor is one scroll area filling the centre (its heading scrolls too); the window header is
    // above it.
    let header = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    let y = header.top() - 6.0;
    // Inside the set editor's scroll area (the centre less its margin): the editor's status line (review E4) leaves
    // less than 40 px under the card here.
    h.hover_at(eframe::egui::pos2(
        card.center().x,
        (card.bottom() + 40.0).min(header.bottom() - 16.0),
    ));
    h.event(eframe::egui::Event::MouseWheel {
        unit: eframe::egui::MouseWheelUnit::Point,
        delta: eframe::egui::vec2(0.0, -(card.top() - y + 30.0)),
        source: eframe::egui::MouseWheelSource::Wheel,
        phase: eframe::egui::TouchPhase::Move,
        modifiers: eframe::egui::Modifiers::NONE,
    });
    for _ in 0..30 {
        h.step();
    }
    let moved = slot_rect(&h, "Diffuse");
    assert!(
        moved.top() < y,
        "the Diffuse card scrolled up out of the centre: {card:?} -> {moved:?}, centre {header:?}"
    );
    let other = tempfile::tempdir().unwrap();
    let outside = other.path().join("brick.png");
    std::fs::write(&outside, b"x").unwrap();
    h.hover_at(eframe::egui::pos2(moved.center().x, y));
    h.run();
    h.input_mut()
        .dropped_files
        .push(std::sync::Arc::new(Dropped(outside)));
    h.run();
    assert_eq!(h.state().1.drops.slot_note, None, "no slot card took it");
    // The fixture has unsaved edits, so opening the file's folder asks first.
    assert!(
        matches!(&h.state().0.dialog, Some(sdc_gui::model::Dialog::OpenUnsaved(p)) if p == other.path()),
        "{:?}",
        h.state().0.dialog
    );
}

#[cfg(windows)]
#[test]
fn a_failed_native_lookup_skips_the_editor_slot_under_the_stale_pointer() {
    use eframe::egui;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let key = set_views(&m)
        .into_iter()
        .find(|v| v.id == LEAF)
        .unwrap()
        .key;
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    let slot = slot_rect(&h, "Normal _n").center();
    h.hover_at(slot);
    h.run();
    assert_eq!(h.ctx.pointer_latest_pos(), Some(slot));
    let inner = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
    h.input_mut()
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .inner_rect = Some(inner);
    sdc_gui::ui::fail_cursor_lookup(&h.ctx);
    let path = file(&h.state().0, NORMAL);
    let folder = path.parent().unwrap().to_owned();
    h.input_mut()
        .dropped_files
        .push(std::sync::Arc::new(Dropped(path)));
    h.step();
    idle(&mut h.state_mut().0);
    assert!(
        !members(&h.state().0, LEAF).iter().any(|key| key == NORMAL),
        "the stale pointer must not deliver the file to the editor slot"
    );
    assert!(
        matches!(&h.state().0.dialog, Some(sdc_gui::model::Dialog::OpenUnsaved(p)) if p == &folder)
    );
}

/// Note JW2: previous paint owns a file under the OS cursor even if the set card is no longer drawn.
#[test]
fn an_explorer_drop_uses_the_cursor_on_the_previous_strip_or_bridge() {
    use eframe::egui;
    let d = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let outside = other.path().join("brick.png");
    std::fs::write(&outside, b"x").unwrap();
    let strip = egui::Vec2::ZERO;
    let bridge = egui::vec2(0.0, 17.0);
    let beside = egui::vec2(48.0, 0.0);
    for (release, pointer, opens_folder) in [
        (Some(beside), strip, true),
        (Some(beside), bridge, true),
        (Some(strip), beside, false),
        (None, strip, true),
        (None, beside, false),
    ] {
        let mut m = pbr_opened(d.path());
        let key = set_views(&m)
            .into_iter()
            .find(|v| v.id == LEAF)
            .unwrap()
            .key;
        m.select_set(&key);
        let mut h = harness(m);
        h.run();
        let slot = slot_rect(&h, "Normal _n").center();
        // Seed the previous paint: the owner's column is gone and the editor now occupies its space.
        let painted = egui::Rect::from_center_size(slot, egui::vec2(32.0, 32.0));
        let hit = egui::Rect::from_center_size(slot + bridge, egui::vec2(32.0, 2.0));
        h.state_mut().1.drops.strip = Some((ROCK.into(), painted, hit));
        h.state_mut().0.settings.sets_hidden = true;
        if let Some(release) = release {
            for (pos, button) in [
                (slot + pointer, egui::PointerButton::Primary),
                (slot + release, egui::PointerButton::Secondary),
            ] {
                h.input_mut().events.push(egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: false,
                    modifiers: egui::Modifiers::SHIFT,
                });
            }
        }
        h.input_mut()
            .events
            .push(egui::Event::PointerMoved(slot + pointer));
        h.input_mut()
            .dropped_files
            .push(std::sync::Arc::new(Dropped(outside.clone())));
        h.step();
        if opens_folder {
            assert!(
                h.state().1.drops.slot_note.is_none(),
                "the new slot must not take last frame's strip"
            );
            assert!(
                matches!(&h.state().0.dialog, Some(sdc_gui::model::Dialog::OpenUnsaved(p)) if p == other.path())
            );
        } else {
            assert!(
                h.state()
                    .1
                    .drops
                    .slot_note
                    .as_ref()
                    .is_some_and(|n| n.error),
                "the OS cursor was on an editor slot"
            );
            assert!(h.state().0.dialog.is_none());
        }
        assert!(
            h.state().1.drops.strip.is_none(),
            "the hidden column has no current strip"
        );
    }
}

#[test]
fn an_explorer_drop_on_a_visible_card_clears_the_strip_and_stops_polling() {
    use eframe::egui;
    use sdc_gui::model::Dialog;
    use sdc_gui::ui::paint_counts::Work::DropPosition;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened_plain(d.path()));
    h.run();
    let card = h
        .get_by_label_contains("Set landscape/rock01, position ")
        .rect();
    h.hover_at(card.center());
    h.run();
    let polls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&polls);
    h.ctx.set_repaint_observer(move |info| {
        if info.delay == Duration::from_millis(50) {
            seen.fetch_add(1, Ordering::Relaxed);
        }
    });
    let reads = DropPosition.count();
    let counts = || (DropPosition.count() - reads, polls.load(Ordering::Relaxed));
    let path = file(&h.state().0, NORMAL);
    let folder = path.parent().unwrap().to_owned();
    h.input_mut().hovered_files.push(egui::HoveredFile {
        path: Some(path.clone()),
        ..Default::default()
    });
    h.run_steps(2);
    assert_eq!(
        h.state().1.drops.strip.as_ref().map(|s| s.0.as_str()),
        Some(ROCK)
    );
    assert_eq!(counts(), (2, 2));

    // egui-winit clears hovered_files when it delivers DroppedFile.
    h.input_mut().hovered_files.clear();
    h.input_mut().dropped_files.push(Arc::new(Dropped(path)));
    h.step();
    assert!(matches!(&h.state().0.dialog, Some(Dialog::OpenUnsaved(p)) if p == &folder));
    assert!(
        h.state().1.drops.strip.is_none(),
        "delivery clears the painted strip"
    );
    assert_eq!(
        counts(),
        (3, 2),
        "delivery reads the cursor once and ends polling"
    );
    h.run_steps(3);
    assert!(h.state().1.drops.strip.is_none());
    assert_eq!(
        counts(),
        (3, 2),
        "idle frames after delivery do no more polling"
    );
}

/// Trace-debug #52: a folder change closes the Sets column's modal and the Slot commands window.
#[test]
fn a_folder_change_closes_the_sets_modals_and_slot_commands() {
    use sdc_gui::ui::sets_column::Pending;
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let mut h = harness(m);
    h.run();
    h.state_mut().1.drops.pending = Some(Pending::Refused("why".into()));
    h.state_mut().1.slot_cmds.open = Some(LEAF.into());
    h.run();
    let root = h.state().0.root.clone().unwrap();
    h.state_mut().0.open_folder(root);
    idle(&mut h.state_mut().0);
    h.run();
    assert!(h.state().1.drops.pending.is_none());
    assert!(h.state().1.slot_cmds.open.is_none());
}

/// Backlog T21: an Explorer drop under an open modal is ignored; the dialog and its state stay.
#[test]
fn a_drop_under_a_modal_is_ignored() {
    use sdc_gui::model::Dialog;
    use sdc_gui::ui::sets_column::Pending;
    let other = tempfile::tempdir().unwrap();
    let outside = other.path().join("brick.png");
    std::fs::write(&outside, b"x").unwrap();
    // A Rename prompt with typed text, an error dialog, and the Slot commands dialog.
    for modal in 0..3 {
        let d = tempfile::tempdir().unwrap();
        let mut m = pbr_opened(d.path());
        let key = set_views(&m)
            .into_iter()
            .find(|v| v.id == LEAF)
            .unwrap()
            .key;
        m.select_set(&key);
        let root = m.root.clone();
        let mut h = harness(m);
        h.run();
        let rename = Pending::Rename {
            id: LEAF.into(),
            text: "typed".into(),
            error: None,
        };
        match modal {
            0 => h.state_mut().1.drops.pending = Some(rename.clone()),
            1 => h.state_mut().0.dialog = Some(Dialog::Error("busy".into())),
            _ => h.state_mut().1.slot_cmds.open = Some(LEAF.into()),
        }
        h.run();
        // Over a slot (the outside file would raise a refusal) and over empty space (it would open the folder).
        drop_on(&mut h, "Normal _n", outside.clone());
        h.hover_at(eframe::egui::pos2(700.0, 3.0));
        h.run();
        h.input_mut()
            .dropped_files
            .push(std::sync::Arc::new(Dropped(other.path().to_path_buf())));
        h.run();
        let (m, st) = (&h.state().0, &h.state().1);
        assert!(st.drops.slot_note.is_none(), "modal {modal}: no slot note");
        assert_eq!(m.root, root, "modal {modal}: the folder stays open");
        match modal {
            0 => assert_eq!(
                st.drops.pending,
                Some(rename),
                "the Rename prompt and its text stay"
            ),
            1 => {
                assert_eq!(m.dialog, Some(Dialog::Error("busy".into())));
                assert!(
                    st.drops.pending.is_none(),
                    "no prompt raised under the dialog"
                );
            }
            _ => {
                assert_eq!(st.slot_cmds.open.as_deref(), Some(LEAF));
                assert!(m.dialog.is_none() && st.drops.pending.is_none());
            }
        }
    }
}
