//! Hostile UI review 2026-11-15, finding 6: "Assign…" on an Advanced skin set's empty Skin wet or Skin detail slot
//! (slots 9 and 10) must be able to put a file there, as the other slots' Assign menus can.
mod support;
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::Subtype;
use engine::shader::Shader;
use image::{ImageBuffer, Rgb};
use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::sets::{set_views, SetView, SlotFill, SlotKind};
use sdc_gui::model::{Answer, AppModel};
use sdc_gui::settings::GuiSettings;
use std::path::Path;
use std::time::Duration;

fn colour(dir: &Path, name: &str) {
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(dir.join(name))
        .unwrap();
}

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

fn view(m: &AppModel, id: &str) -> SetView {
    set_views(m)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("no set {id}"))
}

/// `m/body` (diffuse and `_n`, shader Advanced skin by hand), its `body_wet.png` and `body_detail_n.png`, and
/// `m/rock02`, an Advanced skin set by its maps with its own `_wet` and `_detail_n`.
fn project(dir: &Path) -> AppModel {
    let r = dir.join("m");
    std::fs::create_dir_all(&r).unwrap();
    for f in [
        "body.png",
        "body_n.png",
        "body_wet.png",
        "body_detail_n.png",
        "rock02.png",
        "rock02_rfaos.png",
        "rock02_wet.png",
        "rock02_detail_n.png",
    ] {
        colour(&r, f);
    }
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, support::session());
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    support::choose_export(&mut m, dir.join("export"));
    m.set_shader("m/body", Some(Shader::AdvancedSkin));
    idle(&mut m);
    m
}

fn harness(m: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(egui::vec2(1600.0, 1400.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

/// Open the "Assign…" menu of the set editor row labelled `slot` ("Skin wet _wet").
fn open_assign(h: &mut Harness<'static, AppModel>, slot: &str) {
    let row = h.get_by_label(slot).rect();
    // The row's Assign button is the last one whose top is at or above the slot label's.
    let assign = h
        .query_all_by_label("Assign…")
        .filter(|n| n.rect().min.y <= row.min.y + 1.0)
        .max_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y))
        .unwrap_or_else(|| panic!("no Assign… on {slot}"));
    assign.click();
    h.run();
}

/// Click the menu button whose label holds `text`. A Textures-column tile is a label, not a button. The popup opens
/// left of a button near the window's right edge, so it is not scoped by position.
fn click_entry(h: &mut Harness<'static, AppModel>, text: &str) {
    let mut found: Vec<_> = h
        .query_all_by_label_contains(text)
        .filter(|n| n.accesskit_node().role() == Role::Button)
        .collect();
    let seen = || -> Vec<String> {
        h.query_all_by_label_contains(text)
            .map(|n| {
                format!(
                    "{:?} {:?}",
                    n.accesskit_node().role(),
                    n.accesskit_node().label()
                )
            })
            .collect()
    };
    assert_eq!(found.len(), 1, "menu buttons holding {text}: {:?}", seen());
    let n = found.remove(0);
    assert!(!n.accesskit_node().is_disabled(), "{text} is off");
    n.click();
    h.run();
}

/// The labels of `set`'s `kind` slot menu that are on.
fn enabled_labels(m: &AppModel, set: &str, kind: SlotKind) -> Vec<String> {
    m.slot_menu(set, kind)
        .into_iter()
        .filter(|e| e.disabled.is_none())
        .map(|e| e.label)
        .collect()
}

/// The review's case: `body_wet.png` typed "skin wet" sits in a set of its own; the body set's empty wet slot takes it
/// from Assign….
#[test]
fn assign_on_an_empty_wet_slot_puts_a_wet_file_from_the_folder_there() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    m.decide("m/body_wet.png", Answer::Subtype(Subtype::SkinWet));
    idle(&mut m);
    let body = view(&m, "m/body");
    assert!(body.skin, "m/body is an Advanced skin set");
    assert!(
        !body
            .slots
            .get(&SlotKind::SkinWet)
            .is_some_and(SlotFill::is_filled),
        "{:?}",
        body.slots
    );
    // Listed to move in, and (its set is one other skin sets may share) to borrow.
    let menu = m.slot_menu("m/body", SlotKind::SkinWet);
    for borrow in [false, true] {
        assert!(
            menu.iter().any(|e| match &e.action {
                MenuAction::SlotMove { item, .. } => !borrow && item == "m/body_wet.png",
                MenuAction::SlotBorrow { item, .. } => borrow && item == "m/body_wet.png",
                _ => false,
            } && e.disabled.is_none()),
            "borrow {borrow}: {:?}",
            menu.iter().map(|e| &e.label).collect::<Vec<_>>()
        );
    }
    m.select_set(&body.key);
    let mut h = harness(m);
    h.run();
    open_assign(&mut h, "Skin wet _wet");
    click_entry(&mut h, "body_wet.png");
    let m = h.state_mut();
    idle(m);
    assert_eq!(
        view(m, "m/body").slots.get(&SlotKind::SkinWet),
        Some(&SlotFill::Own {
            item: "m/body_wet.png".into()
        })
    );
}

/// Another Advanced skin set's `_detail_n` is offered under "From another set" on the empty detail slot.
#[test]
fn assign_on_an_empty_detail_slot_offers_another_sets_detail_map() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let menu = enabled_labels(&m, "m/body", SlotKind::SkinDetail);
    assert!(menu.iter().any(|l| l == "From another set"), "{menu:?}");
    assert!(
        menu.iter().any(|l| l.contains("rock02_detail_n.png")),
        "{menu:?}"
    );
    let key = view(&m, "m/body").key;
    m.select_set(&key);
    let mut h = harness(m);
    h.run();
    open_assign(&mut h, "Skin detail _detail_n");
    click_entry(&mut h, "From another set");
    click_entry(&mut h, "rock02_detail_n.png");
    let m = h.state_mut();
    idle(m);
    assert!(
        matches!(
            view(m, "m/body").slots.get(&SlotKind::SkinDetail),
            Some(SlotFill::Borrowed { item, .. }) if item == "m/rock02_detail_n.png"
        ),
        "{:?}",
        view(m, "m/body").slots
    );
    // What is written: a `borrow.slot10` naming the host set in `sdc-project.toml`, and no slot 9 or 10 in the
    // PGPatcher JSON (HB2).
    let text = m.sets_doc.as_ref().unwrap().text();
    assert!(text.contains(r#"slot10 = "m/rock02""#), "{text}");
    assert_eq!(text.matches("slot10").count(), 1, "{text}");
    let entries: Vec<_> = sdc_gui::model::pbr_export::json_entries(m)
        .into_iter()
        .map(|(_, e)| e)
        .collect();
    let json = sdc_gui::pgp::writer::to_text(&sdc_gui::pgp::writer::document(&entries));
    for bad in ["slot9", "slot10"] {
        assert!(!json.contains(bad), "{bad} in {json}");
    }
}

/// An undecided `body_wet.png` (GY2: no suggestion, it waits) is listed too; the pick types it Skin wet and moves it
/// in as one undo step.
#[test]
fn an_undecided_wet_file_is_typed_and_moved_in_one_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    assert!(m.item("m/body_wet.png").unwrap().waiting, "GY2: it waits");
    let pick = m
        .slot_menu("m/body", SlotKind::SkinWet)
        .into_iter()
        .find(|e| e.label.starts_with("body_wet.png"))
        .expect("body_wet.png listed");
    assert!(pick.disabled.is_none(), "{:?}", pick.disabled);
    assert!(m.run_menu(pick.action).is_none());
    idle(&mut m);
    assert_eq!(
        view(&m, "m/body").slots.get(&SlotKind::SkinWet),
        Some(&SlotFill::Own {
            item: "m/body_wet.png".into()
        })
    );
    let it = m.item("m/body_wet.png").unwrap();
    assert!(!it.waiting);
    assert_eq!(it.job.as_ref().unwrap().profile.subtype, Subtype::SkinWet);
    m.undo();
    idle(&mut m);
    assert!(m.item("m/body_wet.png").unwrap().waiting, "one undo step");
    assert!(!view(&m, "m/body")
        .members
        .iter()
        .any(|k| k == "m/body_wet.png"));
}

/// A file of that type in a set the user made stays out of the move list; it is borrowed from "From another set".
#[test]
fn a_file_in_a_real_set_is_only_borrowed() {
    let d = tempfile::tempdir().unwrap();
    let m = project(d.path());
    let menu = m.slot_menu("m/body", SlotKind::SkinWet);
    assert!(
        !menu.iter().any(|e| matches!(
            &e.action,
            sdc_gui::model::bulk::MenuAction::SlotMove { item, .. } if item == "m/rock02_wet.png"
        )),
        "{:?}",
        menu.iter().map(|e| &e.label).collect::<Vec<_>>()
    );
    assert!(menu.iter().any(|e| matches!(
        &e.action,
        sdc_gui::model::bulk::MenuAction::SlotBorrow { item, .. } if item == "m/rock02_wet.png"
    )));
}

/// A set renamed into another folder still lists the files of the folder its own files are in.
#[test]
fn a_renamed_set_lists_its_files_folder() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    m.decide("m/body_wet.png", Answer::Subtype(Subtype::SkinWet));
    idle(&mut m);
    m.rename_set("m/body", "other/anna").unwrap();
    idle(&mut m);
    let menu = m.slot_menu("other/anna", SlotKind::SkinWet);
    assert!(
        menu.iter().any(|e| matches!(
            &e.action,
            sdc_gui::model::bulk::MenuAction::SlotMove { item, .. } if item == "m/body_wet.png"
        )),
        "{:?}",
        menu.iter().map(|e| &e.label).collect::<Vec<_>>()
    );
}

/// Note JD3's limit: beside a set that is skin only by its chosen shader, `body_detail_n.png` types `normal` and is
/// not listed; typed Skin detail by hand, it is.
#[test]
fn a_detail_map_typed_normal_is_listed_once_typed_skin_detail() {
    let d = tempfile::tempdir().unwrap();
    let mut m = project(d.path());
    let listed = |m: &AppModel| {
        m.slot_menu("m/body", SlotKind::SkinDetail).iter().any(|e| {
            matches!(
                &e.action,
                sdc_gui::model::bulk::MenuAction::SlotMove { item, .. } if item == "m/body_detail_n.png"
            )
        })
    };
    assert!(!listed(&m));
    m.decide("m/body_detail_n.png", Answer::Subtype(Subtype::SkinDetail));
    idle(&mut m);
    assert!(listed(&m));
}
