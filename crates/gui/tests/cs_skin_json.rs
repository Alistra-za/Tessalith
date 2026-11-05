//! Task 4b: the Advanced skin `_wet` and `_detail_n` fill slots 9 and 10, which the cards, locks and borrows know but no
//! game-facing writer (PGPatcher `slotN`, TXST, NIF) ever sees.
mod support;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::pbr_export::json_entries;
use sdc_gui::model::sets::{set_views, DropChoice, SetView, SlotFill, SlotKind};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use std::path::Path;
use std::time::Duration;
use support::session;

fn grey(dir: &Path, name: &str) {
    ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
        .save(dir.join(name))
        .unwrap();
}

fn colour(dir: &Path, name: &str) {
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(dir.join(name))
        .unwrap();
}

/// Two Advanced skin sets (`rock01`, `rock02`: diffuse, `_rfaos`, `_wet`, `_detail_n`) and a plain `plain` diffuse.
fn fixture(dir: &Path) -> AppModel {
    let r = dir.join("m");
    std::fs::create_dir_all(&r).unwrap();
    for base in ["rock01", "rock02"] {
        colour(&r, &format!("{base}.png"));
        colour(&r, &format!("{base}_rfaos.png"));
        colour(&r, &format!("{base}_wet.png"));
        colour(&r, &format!("{base}_detail_n.png"));
    }
    colour(&r, "plain.png");
    grey(&r, "plain_roughness.png");
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(dir.to_path_buf());
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m
}

fn view(m: &AppModel, id: &str) -> SetView {
    set_views(m)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("no set {id}"))
}

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

#[test]
fn wet_and_detail_have_slots_9_and_10() {
    assert_eq!(SlotKind::SkinWet.slot_number(), Some(9));
    assert_eq!(SlotKind::SkinDetail.slot_number(), Some(10));
    assert_eq!(SlotKind::SkinWet.game_slot(), None);
    assert_eq!(SlotKind::SkinDetail.game_slot(), None);
    assert_eq!(SlotKind::Subsurface.game_slot(), Some(8));
    assert_eq!(sdc_gui::model::sets::LAST_GAME_SLOT, 8);
    assert_eq!(SlotKind::SkinWet.suffix(), "wet");
    assert_eq!(SlotKind::SkinDetail.suffix(), "detail_n");
    assert_eq!(SlotKind::SkinWet.label(), "Skin wet _wet");
    assert_eq!(SlotKind::SkinDetail.label(), "Skin detail _detail_n");
    assert_eq!(SlotKind::ALL.len(), 13);
}

#[test]
fn a_skin_set_fills_its_wet_and_detail_slots() {
    let d = tempfile::tempdir().unwrap();
    let m = fixture(d.path());
    let v = view(&m, "m/rock01");
    assert!(v.skin, "{:?}", v.slots);
    assert_eq!(
        v.slots.get(&SlotKind::SkinWet),
        Some(&SlotFill::Own {
            item: "m/rock01_wet.png".into()
        })
    );
    assert!(matches!(
        v.slots.get(&SlotKind::SkinDetail),
        Some(SlotFill::Own { .. })
    ));
}

#[test]
fn wet_can_be_shared_from_another_skin_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    // rock01 gives its own `_wet` up for rock02's: Share rock02's into rock01 (replacing the occupant).
    let out = m.drop_choice("m/rock02_wet.png", "m/rock01", DropChoice::Share);
    let r = match out {
        sdc_gui::model::sets::DropOutcome::AskReplace { .. } => {
            m.drop_replace("m/rock02_wet.png", "m/rock01", DropChoice::Share)
        }
        sdc_gui::model::sets::DropOutcome::Joined => Ok(()),
        other => panic!("{other:?}"),
    };
    r.unwrap();
    idle(&mut m);
    let v = view(&m, "m/rock01");
    assert_eq!(
        v.slots.get(&SlotKind::SkinWet),
        Some(&SlotFill::Borrowed {
            from: "m/rock02".into(),
            item: "m/rock02_wet.png".into()
        }),
        "{:?}",
        v.slots
    );
    // The borrow persists in sdc-project.toml under slot 9 and reads back.
    let e = m
        .sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|e| e.id == "m/rock01")
        .cloned()
        .unwrap();
    assert_eq!(e.borrow.get(&9).map(String::as_str), Some("m/rock02"));
}

#[test]
fn a_wet_slot_can_be_pinned() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_slot_lock("m/rock01", 9, true).unwrap();
    idle(&mut m);
    assert!(view(&m, "m/rock01").locks.contains(&9));
    assert!(m.slot_locked("m/rock01", 9));
}

/// rock01 borrows rock02's `_wet` (slot 9).
fn with_borrowed_wet(m: &mut AppModel) {
    let out = m.drop_choice("m/rock02_wet.png", "m/rock01", DropChoice::Share);
    if matches!(out, sdc_gui::model::sets::DropOutcome::AskReplace { .. }) {
        m.drop_replace("m/rock02_wet.png", "m/rock01", DropChoice::Share)
            .unwrap();
    }
    idle(m);
    assert!(matches!(
        view(m, "m/rock01").slots.get(&SlotKind::SkinWet),
        Some(SlotFill::Borrowed { .. })
    ));
}

fn clear_entry(m: &AppModel) -> sdc_gui::model::bulk::MenuEntry {
    m.slot_menu("m/rock01", SlotKind::SkinWet)
        .into_iter()
        .find(|e| {
            matches!(
                e.action,
                sdc_gui::model::bulk::MenuAction::ClearBorrow { n: 9, .. }
            )
        })
        .expect("a borrowed skin slot offers Clear")
}

#[test]
fn a_borrowed_wet_can_be_cleared_from_its_menu() {
    use sdc_gui::model::bulk::MenuAction;
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    with_borrowed_wet(&mut m);
    let e = clear_entry(&m);
    assert_eq!(e.disabled, None);
    assert_eq!(m.run_menu(e.action), None);
    idle(&mut m);
    // The borrow is gone from the view and the document, no PGPatcher setting was written, and an own slot offers no Clear.
    let v = view(&m, "m/rock01");
    assert!(
        !matches!(
            v.slots.get(&SlotKind::SkinWet),
            Some(SlotFill::Borrowed { .. })
        ),
        "{:?}",
        v.slots
    );
    assert!(!v.settings.keys().any(|k| k.starts_with("slot")));
    let e = m
        .sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|e| e.id == "m/rock01")
        .cloned()
        .unwrap();
    assert!(e.borrow.is_empty(), "{:?}", e.borrow);
    assert!(!m
        .slot_menu("m/rock01", SlotKind::SkinWet)
        .iter()
        .any(|e| matches!(e.action, MenuAction::ClearBorrow { .. })));
}

#[test]
fn a_pinned_borrowed_wet_refuses_clear() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    with_borrowed_wet(&mut m);
    m.set_slot_lock("m/rock01", 9, true).unwrap();
    idle(&mut m);
    let e = clear_entry(&m);
    assert!(e.disabled.is_some());
    assert!(m.run_menu(e.action).is_some());
    idle(&mut m);
    assert!(matches!(
        view(&m, "m/rock01").slots.get(&SlotKind::SkinWet),
        Some(SlotFill::Borrowed { .. })
    ));
}

#[test]
fn the_wet_slot_menu_offers_no_slot_command() {
    let d = tempfile::tempdir().unwrap();
    let m = fixture(d.path());
    let menu = m.slot_menu("m/rock01", SlotKind::SkinWet);
    assert!(!menu.is_empty());
    for e in &menu {
        assert!(
            !matches!(
                e.action,
                sdc_gui::model::bulk::MenuAction::Slot { .. }
                    | sdc_gui::model::bulk::MenuAction::SlotPath { .. }
                    | sdc_gui::model::bulk::MenuAction::SlotLink { .. }
                    | sdc_gui::model::bulk::MenuAction::PgpLock { .. }
            ),
            "{}",
            e.label
        );
    }
    assert!(menu.iter().any(|e| matches!(
        e.action,
        sdc_gui::model::bulk::MenuAction::SlotLock { n: 9, .. }
    )));
}

#[test]
fn pgpatcher_json_never_names_slot_9_or_10() {
    use sdc_gui::model::Answer;
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    // `plain` is a TruePBR set that borrows both skin maps from a skin set.
    m.decide(
        "m/plain.png",
        Answer::Subtype(engine::config::Subtype::PbrAlbedo),
    );
    idle(&mut m);
    for item in ["m/rock02_wet.png", "m/rock02_detail_n.png"] {
        let _ = m.drop_choice(item, "m/plain", DropChoice::Share);
        idle(&mut m);
    }
    let v = view(&m, "m/plain");
    assert!(v.pbr, "plain is a JSON set");
    assert!(
        matches!(
            v.slots.get(&SlotKind::SkinWet),
            Some(SlotFill::Borrowed { .. })
        ),
        "{:?}",
        v.slots
    );
    assert!(matches!(
        v.slots.get(&SlotKind::SkinDetail),
        Some(SlotFill::Borrowed { .. })
    ));
    let entries: Vec<_> = json_entries(&m).into_iter().map(|(_, e)| e).collect();
    assert!(!entries.is_empty());
    for e in &entries {
        assert!(e.slots.keys().all(|n| *n <= 8), "{:?}", e.slots);
    }
    let text = sdc_gui::pgp::writer::to_text(&sdc_gui::pgp::writer::document(&entries));
    for bad in ["slot9", "slot10"] {
        assert!(!text.contains(bad), "{bad} in {text}");
    }
}

fn harness(m: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(egui::vec2(1400.0, 2400.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

#[test]
fn wet_and_detail_cards_only_in_advanced_skin_sets() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    // The plain set borrows the skin set's `_wet` and `_detail_n`; its view then holds both slots but is not an
    // Advanced skin set.
    for item in ["m/rock02_wet.png", "m/rock02_detail_n.png"] {
        let _ = m.drop_choice(item, "m/plain", DropChoice::Share);
        idle(&mut m);
    }
    let plain = view(&m, "m/plain");
    assert!(!plain.skin);
    for k in [SlotKind::SkinWet, SlotKind::SkinDetail] {
        assert!(plain.slots.contains_key(&k), "{:?}", plain.slots);
    }
    let (plain_key, skin_key) = (plain.key.clone(), view(&m, "m/rock01").key);
    m.select_set(&plain_key);
    let mut h = harness(m);
    h.run();
    assert_eq!(h.query_all_by_label("Skin wet _wet").count(), 0);
    assert_eq!(h.query_all_by_label("Skin detail _detail_n").count(), 0);
    h.state_mut().select_set(&skin_key);
    h.run();
    h.get_by_label("Skin wet _wet");
    h.get_by_label("Skin detail _detail_n");
}
