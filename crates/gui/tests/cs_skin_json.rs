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

// ---- Task 5: the `.csskin.json` writer ----

use sdc_gui::model::cs_skin_json::{
    fnv1a64, material_text, skin_json_plan, SkinMaterial, SkinSkip,
};
use sdc_gui::model::review::{DiskState, JsonKind};
use serde_json::{json, Value as Json};
use std::path::PathBuf;

/// Advanced skin sets `bases` (diffuse, `_rfaos`, `_wet`, `_detail_n`, all colour) under `<dir>/<folder>`, exported to
/// `<dir>/export`.
fn skin_project(dir: &Path, folder: &str, bases: &[&str]) -> AppModel {
    let r = dir.join(folder);
    std::fs::create_dir_all(&r).unwrap();
    for base in bases {
        for s in ["", "_rfaos", "_wet", "_detail_n"] {
            colour(&r, &format!("{base}{s}.png"));
        }
    }
    opened_skin(dir)
}

fn opened_skin(dir: &Path) -> AppModel {
    opened_skin_at(dir, dir)
}

/// The project at `root` (under `dir`), exported to `<dir>/export`.
fn opened_skin_at(dir: &Path, root: &Path) -> AppModel {
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.open_folder(root.to_path_buf());
    idle(&mut m);
    support::choose_export(&mut m, dir.join("export"));
    assert_eq!(m.target, engine::config::Target::Cs);
    m
}

fn id_of(m: &AppModel, key_end: &str) -> String {
    let v = set_views(m)
        .into_iter()
        .find(|v| v.key.ends_with(key_end))
        .unwrap_or_else(|| panic!("no set {key_end}"));
    v.id
}

fn export_and_wait(m: &mut AppModel) {
    idle(m);
    support::export_through(m);
    assert!(
        m.wait_idle(Duration::from_secs(120)),
        "export did not finish"
    );
}

fn notes(m: &AppModel) -> String {
    m.export.notes.join("\n")
}

fn out(m: &AppModel, rel: &str) -> PathBuf {
    m.export_root().join(rel)
}

fn read_json(p: &Path) -> Json {
    let t = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    serde_json::from_str(&t).unwrap()
}

/// Share `item` into set `to` (replacing its own occupant).
fn share(m: &mut AppModel, item: &str, to: &str) {
    let o = m.drop_choice(item, to, DropChoice::Share);
    if matches!(o, sdc_gui::model::sets::DropOutcome::AskReplace { .. }) {
        m.drop_replace(item, to, DropChoice::Share).unwrap();
    }
    idle(m);
}

#[test]
fn fnv1a64_matches_cs() {
    assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
    assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
}

#[test]
fn material_text_orders_keys_and_omits_empty_members() {
    let mut parameters = serde_json::Map::new();
    parameters.insert("Roughness".into(), json!(0.5));
    let m = SkinMaterial {
        parameters,
        controls: Some("textures/x/h_rfaos.dds".into()),
        wet: None,
        detail: None,
        authored_for: Some((12, 0xab)),
    };
    assert_eq!(
        material_text(&m),
        "{\n  \"format\": \"CommunityShaders.SkinMaterial\",\n  \"version\": 1,\n  \"parameters\": {\n    \
         \"Roughness\": 0.5\n  },\n  \"textures\": {\n    \"controls\": \"textures/x/h_rfaos.dds\"\n  },\n  \
         \"authoredFor\": {\n    \"size\": 12,\n    \"hash\": \"00000000000000ab\"\n  }\n}\n"
    );
    let bare = SkinMaterial {
        parameters: serde_json::Map::new(),
        controls: None,
        wet: None,
        detail: None,
        authored_for: None,
    };
    assert_eq!(
        material_text(&bare),
        "{\n  \"format\": \"CommunityShaders.SkinMaterial\",\n  \"version\": 1\n}\n"
    );
}

#[test]
fn export_writes_one_csskin_json_beside_the_diffuse() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), "textures/x", &["h"]);
    let id = id_of(&m, "x/h");
    m.set_skin_value(&id, "Roughness", Some(json!(0.62)));
    m.set_skin_value(&id, "DetailEnabled", Some(json!(false)));
    export_and_wait(&mut m);
    let f = out(&m, "textures/x/h.csskin.json");
    let j = read_json(&f);
    assert_eq!(j["format"], json!("CommunityShaders.SkinMaterial"), "{j}");
    assert_eq!(j["version"], json!(1));
    assert_eq!(
        j["parameters"],
        json!({"Roughness": 0.62, "DetailEnabled": false})
    );
    assert_eq!(j["textures"]["controls"], json!("textures/x/h_rfaos.dds"));
    assert_eq!(j["textures"]["wet"], json!("textures/x/h_wet.dds"));
    assert_eq!(j["textures"]["detail"], json!("textures/x/h_detail_n.dds"));
    for t in ["h_rfaos.dds", "h_wet.dds", "h_detail_n.dds"] {
        assert!(out(&m, &format!("textures/x/{t}")).is_file(), "{t}");
    }
    let bytes = std::fs::read(out(&m, "textures/x/h.dds")).unwrap();
    assert_eq!(j["authoredFor"]["size"], json!(bytes.len()));
    assert_eq!(
        j["authoredFor"]["hash"],
        json!(format!("{:016x}", fnv1a64(&bytes)))
    );
    assert!(notes(&m).contains("CS skin JSON written"), "{}", notes(&m));
}

#[test]
fn untouched_card_writes_no_parameters_member() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), "textures/x", &["h"]);
    export_and_wait(&mut m);
    let j = read_json(&out(&m, "textures/x/h.csskin.json"));
    assert!(j.get("parameters").is_none(), "{j}");
    assert!(j.get("textures").is_some(), "{j}");
}

#[test]
fn paths_inside_are_lowercase_forward_slash() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), "textures/Tex/Body", &["Hand"]);
    export_and_wait(&mut m);
    let j = read_json(&out(&m, "textures/Tex/Body/Hand.csskin.json"));
    assert_eq!(
        j["textures"]["controls"],
        json!("textures/tex/body/hand_rfaos.dds")
    );
    assert_eq!(
        j["textures"]["wet"],
        json!("textures/tex/body/hand_wet.dds")
    );
    assert_eq!(
        j["textures"]["detail"],
        json!("textures/tex/body/hand_detail_n.dds")
    );
}

#[test]
fn edited_csskin_json_is_kept_with_a_note() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), "textures/x", &["h"]);
    export_and_wait(&mut m);
    let f = out(&m, "textures/x/h.csskin.json");
    std::fs::write(&f, "{}\n").unwrap();
    let id = id_of(&m, "x/h");
    m.set_skin_value(&id, "Roughness", Some(json!(0.5)));
    export_and_wait(&mut m);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "{}\n");
    assert!(
        notes(&m).contains("kept as edited outside Tessalith"),
        "{}",
        notes(&m)
    );
}

#[test]
fn foreign_csskin_json_is_never_overwritten() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), "textures/x", &["h"]);
    let f = out(&m, "textures/x/h.csskin.json");
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, "{\"theirs\":1}\n").unwrap();
    export_and_wait(&mut m);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "{\"theirs\":1}\n");
    assert!(
        notes(&m).contains("kept as edited outside Tessalith"),
        "{}",
        notes(&m)
    );
}

#[test]
fn authored_for_hashes_an_unchanged_diffuse() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), "textures/x", &["h"]);
    export_and_wait(&mut m);
    let dds = out(&m, "textures/x/h.dds");
    let before = std::fs::metadata(&dds).unwrap().modified().unwrap();
    export_and_wait(&mut m);
    // The diffuse was current, so the second export left it alone.
    assert_eq!(std::fs::metadata(&dds).unwrap().modified().unwrap(), before);
    let bytes = std::fs::read(&dds).unwrap();
    let j = read_json(&out(&m, "textures/x/h.csskin.json"));
    assert_eq!(j["authoredFor"]["size"], json!(bytes.len()));
    assert_eq!(
        j["authoredFor"]["hash"],
        json!(format!("{:016x}", fnv1a64(&bytes)))
    );
    assert!(!notes(&m).contains("kept"), "{}", notes(&m));
}

#[test]
fn borrowed_rfaos_points_at_the_host_output() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), "textures/x", &["a", "b"]);
    let (a, b) = (id_of(&m, "x/a"), id_of(&m, "x/b"));
    share(&mut m, "textures/x/b_rfaos.png", &a);
    let v = view(&m, &a);
    assert!(v.skin, "{:?}", v.slots);
    assert_eq!(
        v.slots.get(&SlotKind::Rmaos),
        Some(&SlotFill::Borrowed {
            from: b.clone(),
            item: "textures/x/b_rfaos.png".into()
        })
    );
    export_and_wait(&mut m);
    let j = read_json(&out(&m, "textures/x/a.csskin.json"));
    assert_eq!(j["textures"]["controls"], json!("textures/x/b_rfaos.dds"));
    assert_eq!(j["textures"]["wet"], json!("textures/x/a_wet.dds"));
}

#[test]
fn borrowed_wet_and_detail_point_at_the_host_output() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), "textures/x", &["a", "b"]);
    let a = id_of(&m, "x/a");
    share(&mut m, "textures/x/b_wet.png", &a);
    share(&mut m, "textures/x/b_detail_n.png", &a);
    let v = view(&m, &a);
    for k in [SlotKind::SkinWet, SlotKind::SkinDetail] {
        assert!(
            matches!(v.slots.get(&k), Some(SlotFill::Borrowed { .. })),
            "{:?}",
            v.slots
        );
    }
    export_and_wait(&mut m);
    let j = read_json(&out(&m, "textures/x/a.csskin.json"));
    assert_eq!(j["textures"]["wet"], json!("textures/x/b_wet.dds"));
    assert_eq!(j["textures"]["detail"], json!("textures/x/b_detail_n.dds"));
    assert_eq!(j["textures"]["controls"], json!("textures/x/a_rfaos.dds"));
}

#[test]
fn borrowed_diffuse_writes_no_json_for_the_borrower() {
    let d = tempfile::tempdir().unwrap();
    // `b` has no diffuse of its own (one would leave the set as its own skin set and write its own file).
    let r = d.path().join("textures/x");
    std::fs::create_dir_all(&r).unwrap();
    for s in ["", "_rfaos", "_wet", "_detail_n"] {
        colour(&r, &format!("a{s}.png"));
    }
    colour(&r, "b_rfaos.png");
    let mut m = opened_skin(d.path());
    let (a, b) = (id_of(&m, "x/a"), id_of(&m, "x/b"));
    share(&mut m, "textures/x/a.png", &b);
    let (plan, skips) = skin_json_plan(&m);
    assert!(
        skips
            .iter()
            .any(|(id, _, s)| *id == b && *s == SkinSkip::BorrowedDiffuse { host: a.clone() }),
        "{skips:?}"
    );
    assert!(!plan.iter().any(|(id, ..)| *id == b), "{plan:?}");
    export_and_wait(&mut m);
    assert!(out(&m, "textures/x/a.csskin.json").is_file());
    assert!(
        !out(&m, "textures/x/b.csskin.json").exists(),
        "{:?}\n{:?}",
        skin_json_plan(&m),
        set_views(&m)
            .iter()
            .map(|v| (&v.id, &v.members, v.skin, &v.slots))
            .collect::<Vec<_>>()
    );
}

#[test]
fn grey_wet_is_left_out_of_textures() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/x");
    std::fs::create_dir_all(&r).unwrap();
    for s in ["", "_rfaos", "_detail_n"] {
        colour(&r, &format!("h{s}.png"));
    }
    grey(&r, "h_wet.png");
    let mut m = opened_skin(d.path());
    export_and_wait(&mut m);
    let j = read_json(&out(&m, "textures/x/h.csskin.json"));
    assert!(j["textures"].get("wet").is_none(), "{j}");
    assert_eq!(j["textures"]["controls"], json!("textures/x/h_rfaos.dds"));
    assert!(!out(&m, "textures/x/h_wet.dds").exists());
}

#[test]
fn a_skin_set_without_a_diffuse_is_skipped() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/x");
    std::fs::create_dir_all(&r).unwrap();
    colour(&r, "q_rfaos.png");
    let m = opened_skin(d.path());
    let (plan, skips) = skin_json_plan(&m);
    assert!(plan.is_empty(), "{plan:?}");
    assert!(
        skips.iter().any(|(_, _, s)| *s == SkinSkip::NoDiffuse),
        "{skips:?} {:?}",
        set_views(&m)
    );
}

#[test]
fn review_lists_the_csskin_json() {
    let d = tempfile::tempdir().unwrap();
    let m = skin_project(d.path(), "textures/x", &["h"]);
    let l: Vec<_> = m
        .review_json()
        .into_iter()
        .filter(|l| l.kind == JsonKind::CsSkin)
        .collect();
    assert_eq!(l.len(), 1, "{:?}", m.review_json());
    assert_eq!(l[0].path, out(&m, "textures/x/h.csskin.json"));
    assert_eq!(l[0].state, DiskState::New);
}

#[test]
fn second_export_records_ours() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), "textures/x", &["h"]);
    export_and_wait(&mut m);
    m.expire_review_json();
    let l = m
        .review_json()
        .into_iter()
        .find(|l| l.kind == JsonKind::CsSkin)
        .expect("the CS skin line");
    assert_eq!(l.state, DiskState::Ours);
}

/// Fix round 1 #1: a project opened at its `textures` folder writes outputs without that folder; renamed outputs
/// (and a borrowed renamed one) still get complete `textures/...` game paths.
#[test]
fn renamed_outputs_opened_beneath_textures_keep_full_game_paths() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/x");
    std::fs::create_dir_all(&r).unwrap();
    for base in ["a", "b"] {
        for s in ["", "_rfaos", "_wet", "_detail_n"] {
            colour(&r, &format!("{base}{s}.png"));
        }
    }
    let mut m = opened_skin_at(d.path(), &d.path().join("textures"));
    let (a, b) = (id_of(&m, "x/a"), id_of(&m, "x/b"));
    m.set_output_name(&a, Some("ay")).unwrap();
    idle(&mut m);
    m.set_output_name(&b, Some("bee")).unwrap();
    idle(&mut m);
    share(&mut m, "x/b_rfaos.png", &a);
    export_and_wait(&mut m);
    let j = read_json(&out(&m, "x/ay.csskin.json"));
    assert_eq!(
        j["textures"]["controls"],
        json!("textures/x/bee_rfaos.dds"),
        "{j}"
    );
    // Every path is a written file, under `textures/` (the export folder holds `x/...` here).
    for k in ["controls", "wet", "detail"] {
        let g = j["textures"][k]
            .as_str()
            .unwrap_or_else(|| panic!("{k}: {j}"));
        let rel = g.strip_prefix("textures/").unwrap_or_else(|| panic!("{g}"));
        assert!(out(&m, rel).is_file(), "{g}");
    }
}

/// Fix round 2: an input alias to another folder depth (`set_input_name`) moves the game path and the output, not the
/// source; the JSON's paths follow the aliased name, deeper or shallower.
#[test]
fn input_aliases_to_another_folder_depth_keep_full_game_paths() {
    for (src, alias, folder) in [("x", "textures/y/z/a", "y/z"), ("x/y", "textures/x/a", "x")] {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("textures").join(src);
        std::fs::create_dir_all(&r).unwrap();
        // Diffuse and `_rfaos` only: an input rename does not yet know the `_wet`/`_detail_n` suffixes (it refuses,
        // mapping `a_wet` onto `a`).
        for s in ["", "_rfaos"] {
            colour(&r, &format!("a{s}.png"));
        }
        let mut m = opened_skin_at(d.path(), &d.path().join("textures"));
        let id = id_of(&m, &format!("{src}/a"));
        m.set_input_name(&id, alias).unwrap();
        idle(&mut m);
        export_and_wait(&mut m);
        let j = read_json(&out(&m, &format!("{folder}/a.csskin.json")));
        assert_eq!(
            j["textures"]["controls"],
            json!(format!("textures/{folder}/a_rfaos.dds")),
            "{alias}: {j}"
        );
        assert!(
            out(&m, &format!("{folder}/a_rfaos.dds")).is_file(),
            "{alias}"
        );
    }
}

/// Fix round 1 #1: a texture with no `textures/...dds` game path is never written; the JSON leaves it out, with a note.
#[test]
fn a_texture_outside_textures_is_left_out_with_a_note() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("m");
    std::fs::create_dir_all(&r).unwrap();
    for s in ["", "_rfaos"] {
        colour(&r, &format!("h{s}.png"));
    }
    let mut m = opened_skin(d.path());
    export_and_wait(&mut m);
    let j = read_json(&out(&m, "m/h.csskin.json"));
    assert!(j.get("textures").is_none(), "{j}");
    assert!(
        notes(&m).contains("controls left out of the CS skin JSON"),
        "{}",
        notes(&m)
    );
}

/// R4: a borrowed slot whose host has no file there (its grey `_wet` is not exported) is a plan-time `HostMissing`,
/// and the JSON leaves it out.
#[test]
fn a_borrow_from_a_host_without_that_texture_is_host_missing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_project(d.path(), "textures/x", &["a", "b"]);
    let (a, b) = (id_of(&m, "x/a"), id_of(&m, "x/b"));
    share(&mut m, "textures/x/b_wet.png", &a);
    grey(&d.path().join("textures/x"), "b_wet.png");
    m.replan();
    idle(&mut m);
    let (plan, skips) = skin_json_plan(&m);
    assert!(
        skips.iter().any(|(id, _, s)| *id == a
            && *s
                == SkinSkip::HostMissing {
                    slot: "wet",
                    host: b.clone()
                }),
        "{skips:?} {:?}",
        view(&m, &a).missing
    );
    let row = plan
        .iter()
        .find(|(id, ..)| *id == a)
        .expect("a still writes");
    assert_eq!(row.3.wet, None);
    assert!(row.3.controls.is_some());
}
