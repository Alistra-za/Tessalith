//! Note HF1: a texture can be in no set, and any set can be deleted. A no-set texture is on no card, written nowhere,
//! and its warnings and questions count for nothing; Remove from set and Delete set put textures there.
mod support;
use engine::sets::SetEdit;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::bottom::{problems, At};
use sdc_gui::model::items::{ItemKind, ItemStatus};
use sdc_gui::model::sets::{engine_inputs, set_views, DropChoice, DropOutcome, Missing};
use sdc_gui::model::{Answer, AppModel};
use std::path::Path;
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01";
const NORMAL: &str = "landscape/rock01_normal.png";
const ROUGH: &str = "landscape/rock01_roughness.png";
const AO: &str = "landscape/rock01_ao.png";
const LEAF: &str = "landscape/leaf.png";

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "model did not settle");
}

fn no_set(m: &AppModel) -> Vec<String> {
    m.sets_doc.as_ref().unwrap().no_set()
}

fn sets_text(m: &AppModel) -> String {
    m.sets_doc.as_ref().unwrap().text()
}

/// Whether some set view holds `key`.
fn in_a_view(m: &AppModel, key: &str) -> bool {
    set_views(m)
        .iter()
        .any(|v| v.members.iter().any(|k| k == key))
}

/// Problems rows at texture `key`.
fn rows_at(m: &AppModel, key: &str) -> usize {
    problems(m)
        .iter()
        .filter(|p| p.at == At::Texture(key.into()))
        .count()
}

fn grey(path: &Path, v: u8) {
    ImageBuffer::from_fn(64, 64, |x, _| Luma([v.wrapping_add(x as u8)]))
        .save(path)
        .unwrap();
}

fn colour(path: &Path) {
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(path)
        .unwrap();
}

/// Move `key` into set `id` unless it is in it: a file with an unknown suffix starts as a set of its own.
fn ensure_in(m: &mut AppModel, id: &str, key: &str) {
    if !view_holds(m, id, key) {
        m.drop_choice(key, id, DropChoice::Move);
        idle(m);
    }
    assert!(view_holds(m, id, key), "{key} is in {id}");
}

/// `textures/clutter`: a `barrel` set (colour and `_n`) and the file `extra` with an unknown suffix.
fn clutter(dir: &Path, extra: &str) -> AppModel {
    let c = dir.join("textures/clutter");
    std::fs::create_dir_all(&c).unwrap();
    colour(&c.join("barrel.png"));
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(c.join("barrel_n.png"))
        .unwrap();
    colour(&c.join(extra));
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    idle(&mut m);
    m
}

// ---- Remove from set ----

#[test]
fn removing_a_member_puts_it_in_no_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    // A member of a bigger automatic set.
    m.remove_member(ROCK, NORMAL).expect("removed");
    idle(&mut m);
    assert_eq!(no_set(&m), [NORMAL]);
    assert!(m.is_no_set(NORMAL));
    assert!(!in_a_view(&m, NORMAL), "no view holds it");
    assert!(!sets_text(&m).contains("remove"), "{}", sets_text(&m));
    // A moved-in member: its `add` line goes too.
    let odd = "landscape/odd.png";
    let id = m.drop_new_set(odd).unwrap();
    idle(&mut m);
    assert!(sets_text(&m).contains("odd.png"));
    m.remove_member(&id, odd).expect("removed");
    idle(&mut m);
    assert!(no_set(&m).iter().any(|k| k == odd));
    assert!(!sets_text(&m).contains("add"), "{}", sets_text(&m));
    // The only member of an automatic set (today refused).
    m.remove_member("landscape/leaf", LEAF).expect("removed");
    idle(&mut m);
    assert!(m.is_no_set(LEAF));
    assert!(!in_a_view(&m, LEAF));
    // A packed output still refuses.
    let rmaos = "landscape/rock01_rmaos";
    assert!(m.item(rmaos).is_some(), "the packed output exists");
    assert!(m.remove_member(ROCK, rmaos).is_err());
}

#[test]
fn a_removed_fuzz_is_not_a_set_and_reads_no_set() {
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    grey(&root.join("landscape/rock01_f.png"), 90);
    let mut m = AppModelExt::open(&root, d.path());
    m.decide(
        "landscape/rock01.png",
        Answer::Subtype(engine::config::Subtype::SkinAlbedo),
    );
    idle(&mut m);
    let f = "landscape/rock01_f.png";
    assert!(in_a_view(&m, f), "the fuzz starts in the skin set");
    m.remove_member(ROCK, f).expect("removed");
    idle(&mut m);
    assert!(m.is_no_set(f));
    assert!(!in_a_view(&m, f), "no view holds it");
    assert!(
        set_views(&m).iter().all(|v| !v.id.contains("rock01_f")),
        "no set of its own"
    );
    let item = m.item(f).expect("still listed");
    let set_key = item.set_key.clone().expect("the engine kept it in a set");
    assert!(!m.in_run(&set_key), "no job in the run");
    assert_eq!(rows_at(&m, f), 0);
}

/// Helpers of a skin-less set: `mouth_ao.png` and `mouth_ao.tga` (twins) and `mouth_rough.png`, `mouth_mask.png`.
fn mouth(dir: &Path) -> AppModel {
    let r = dir.join("textures/face");
    std::fs::create_dir_all(&r).unwrap();
    colour(&r.join("mouth.png"));
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(r.join("mouth_n.png"))
        .unwrap();
    grey(&r.join("mouth_ao.png"), 200);
    grey(&r.join("mouth_ao.tga"), 100);
    grey(&r.join("mouth_rough.png"), 50);
    grey(&r.join("mouth_mask.png"), 60);
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    m.decide(
        "face/mouth.png",
        Answer::Subtype(engine::config::Subtype::SkinAlbedo),
    );
    idle(&mut m);
    m
}

#[test]
fn a_removed_helper_makes_no_set_of_any_kind() {
    for helper in [
        ["mouth_ao.png", "mouth_ao.tga"].as_slice(),
        &["mouth_rough.png"],
        &["mouth_mask.png"],
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut m = mouth(d.path());
        let before = set_views(&m).len();
        for h in helper {
            let key = format!("face/{h}");
            ensure_in(&mut m, "face/mouth", &key);
            m.remove_member("face/mouth", &key).expect("removed");
            idle(&mut m);
            assert!(m.is_no_set(&key));
        }
        let views = set_views(&m);
        assert_eq!(
            views.len(),
            before,
            "{helper:?}: {:?}",
            views.iter().map(|v| &v.id).collect::<Vec<_>>()
        );
        for h in helper {
            assert!(!in_a_view(&m, &format!("face/{h}")));
        }
        assert!(
            views.iter().all(|v| !v.id.contains('~')),
            "no ~2 shown: {:?}",
            views.iter().map(|v| &v.id).collect::<Vec<_>>()
        );
        // The owner's report: no `mouth_ao_rfaos` packed output, listed or in the plan.
        let listed: Vec<String> = m
            .visible_items()
            .into_iter()
            .map(|n| m.items[n].key.clone())
            .collect();
        assert!(
            listed.iter().all(|k| !k.contains("mouth_ao_rfaos")),
            "{helper:?}: {listed:?}"
        );
        assert!(
            m.items.iter().all(|i| !i.key.contains("mouth_ao_rfaos")),
            "{helper:?}"
        );
        // Whatever packed output a no-set set still has in the plan is not listed.
        for p in m.items.iter().filter(|i| i.kind == ItemKind::Packed) {
            if p.set_key.as_deref().is_some_and(|k| !m.in_run(k)) {
                assert!(!listed.contains(&p.key), "{} is listed", p.key);
            }
        }
    }
}

// ---- the GY1 guard ----

#[test]
fn a_no_set_file_is_not_waiting() {
    for stem in ["barrel_mask", "barrel_bump"] {
        let d = tempfile::tempdir().unwrap();
        let mut m = clutter(d.path(), &format!("{stem}.png"));
        let key = format!("clutter/{stem}.png");
        // Alone it waits for a type; moved into the barrel set it is one file among others.
        assert!(m.item(&key).unwrap().waiting, "{key} waits for a type");
        // Accepting High suggestions would type it as a colour map while it sits in the set.
        m.accept = engine::policy::AcceptSuggestions::None;
        ensure_in(&mut m, "clutter/barrel", &key);
        let before_project = m.project.as_ref().unwrap().text();
        m.remove_member("clutter/barrel", &key).expect("removed");
        idle(&mut m);
        let item = m.item(&key).expect("listed");
        assert!(!item.waiting, "the guard: a no-set file never waits");
        // The engine still asks (nothing writes), but nothing here counts it.
        assert_eq!(item.status, ItemStatus::NeedsDecision);
        assert!(!m.undecided_keys().contains(&key));
        assert!(
            m.undecided_keys().iter().all(|k| !k.contains(stem)),
            "{:?}",
            m.undecided_keys()
        );
        assert_eq!(rows_at(&m, &key), 0, "{:#?}", problems(&m));
        assert!(problems(&m)
            .iter()
            .all(|p| p.code != Some(engine::codes::Code::SugLowConfidence)
                || p.at != At::Texture(key.clone())),);
        assert_eq!(m.filter_counts().needs_decision, m.undecided());
        // Export doesn't open Review for it.
        assert_eq!(m.undecided(), 0, "{:?}", m.undecided_keys());
        m.export_clicked();
        assert!(m.review.is_none(), "Review stays shut: {:?}", m.dialog);
        // No `[files]` sub-type recorded for it.
        assert_eq!(m.project.as_ref().unwrap().text(), before_project);
        assert!(!m.project.as_ref().unwrap().text().contains(stem));
        let tail = m.visible_items().iter().any(|&n| m.items[n].key == key);
        assert!(tail, "still listed in Textures");
        assert!(m.is_no_set(&key));
        // Its tile reads `No set` in place of a question; the tile's hover text is built from the same flag.
        let mut h = egui_kittest::Harness::builder()
            .with_size(eframe::egui::vec2(1500.0, 900.0))
            .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run();
        use egui_kittest::kittest::Queryable;
        assert!(
            h.query_all_by_label_contains("No set").count() >= 1,
            "{stem}: the tile says No set"
        );
    }
}

#[test]
fn a_no_set_textures_warnings_count_for_nothing() {
    let d = tempfile::tempdir().unwrap();
    // `leaf.png` is the one file that asks: the other fixture files are decided or ask nothing.
    let mut m = pbr_opened(d.path());
    let leaf_row = |m: &AppModel| rows_at(m, LEAF);
    m.decide(
        "landscape/odd.png",
        Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    m.decide(
        "landscape/odd.png",
        Answer::Acknowledge(engine::codes::Code::DimSmall),
    );
    m.decide(LEAF, Answer::Subtype(engine::config::Subtype::Diffuse));
    idle(&mut m);
    assert_eq!(m.undecided_keys(), [LEAF], "the leaf is the one question");
    assert!(m.item(LEAF).unwrap().status == ItemStatus::NeedsDecision);
    let undecided = m.undecided();
    assert!(leaf_row(&m) > 0);
    assert!(m.undecided_keys().iter().any(|k| k == LEAF));
    m.remove_member("landscape/leaf", LEAF).expect("removed");
    idle(&mut m);
    assert_eq!(m.undecided(), undecided - 1);
    assert_eq!(m.undecided(), 0, "{:?}", m.undecided_keys());
    assert_eq!(leaf_row(&m), 0, "no Problems row");
    // Export opens no Review for it: nothing else asks, so Review stays shut, and the export counts the same files.
    m.export_clicked();
    assert!(m.review.is_none(), "Review stays shut: {:?}", m.dialog);
    // The pill counts the other undecided files, not the leaf.
    assert_eq!(m.filter_counts().needs_decision, m.undecided());
}

// ---- Delete set ----

#[test]
fn an_old_remove_line_set_can_be_deleted() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let text = sets_text(&m);
    assert!(!text.contains("remove"));
    m.edit_docs("t", None, true, |_, s| {
        s.restore("[project]\n\n[[sets]]\nid = \"landscape/rock01\"\nremove = [\"landscape/rock01_normal.png\"]\n");
    });
    idle(&mut m);
    // An old `remove` line still means a one-file set that exports.
    let v = set_views(&m);
    let one = v
        .iter()
        .find(|v| v.members.len() == 1 && v.members[0] == NORMAL)
        .expect("its one-file set");
    let id = one.id.clone();
    assert!(!m.is_no_set(NORMAL));
    assert!(m.delete_set(&id));
    idle(&mut m);
    assert!(m.is_no_set(NORMAL));
    assert!(!in_a_view(&m, NORMAL));
    // And it stays gone after a new plan.
    m.replan();
    idle(&mut m);
    assert!(!in_a_view(&m, NORMAL));
}

#[test]
fn delete_set_works_on_an_automatic_set_and_it_stays_gone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let before = sets_text(&m);
    let steps = m.history.undo.len();
    m.delete_set_with_toast(ROCK);
    idle(&mut m);
    let toast = m
        .messages
        .entries()
        .map(|e| e.text.clone())
        .find(|t| t.starts_with("Deleted set"))
        .expect("the toast");
    // The packed output follows its raw maps: rock01, normal, roughness and ao are the four files.
    assert_eq!(
        toast,
        format!("Deleted set {ROCK} (4 textures now in no set)")
    );
    assert_eq!(m.history.undo.len(), steps + 1, "one undo step");
    assert_eq!(no_set(&m).len(), 4);
    assert!(set_views(&m).iter().all(|v| v.id != ROCK));
    // A new plan, then a reopen of the project: still gone.
    m.replan();
    idle(&mut m);
    assert!(set_views(&m).iter().all(|v| v.id != ROCK));
    m.sets_doc.as_mut().unwrap().save(true).unwrap();
    let root = m.root.clone().unwrap();
    let mut again = AppModelExt::reopen(&root, d.path());
    idle(&mut again);
    assert!(set_views(&again).iter().all(|v| v.id != ROCK));
    assert_eq!(no_set(&again).len(), 4);
    // One undo step restores `no_set` and the entry.
    m.undo();
    idle(&mut m);
    assert!(no_set(&m).is_empty());
    assert_eq!(sets_text(&m), before);
    assert!(set_views(&m).iter().any(|v| v.id == ROCK));
}

#[test]
fn delete_set_on_a_created_set_leaves_its_files_in_no_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let odd = "landscape/odd.png";
    let id = m.drop_new_set(odd).unwrap();
    idle(&mut m);
    assert!(m.delete_set(&id));
    idle(&mut m);
    assert!(m.is_no_set(odd));
    assert!(!sets_text(&m).contains(&id), "{}", sets_text(&m));
    assert!(!in_a_view(&m, odd));
}

#[test]
fn delete_set_with_no_files_just_goes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.edit_docs("t", None, true, |_, s| s.create_set("mine/empty"));
    idle(&mut m);
    assert!(set_views(&m).iter().any(|v| v.id == "mine/empty"));
    m.delete_set_with_toast("mine/empty");
    idle(&mut m);
    assert!(set_views(&m).iter().all(|v| v.id != "mine/empty"));
    assert!(no_set(&m).is_empty());
    assert!(m
        .messages
        .entries()
        .any(|e| e.text == "Deleted set mine/empty"));
}

// ---- back into a set ----

#[test]
fn dropping_a_no_set_texture_on_a_set_takes_it_out_of_no_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.remove_member(ROCK, NORMAL).unwrap();
    idle(&mut m);
    assert!(m.is_no_set(NORMAL));
    let out = m.drop_on_set(NORMAL, ROCK);
    assert!(
        matches!(out, DropOutcome::Joined | DropOutcome::AskChoice { .. }),
        "{out:?}"
    );
    if matches!(out, DropOutcome::AskChoice { .. }) {
        m.drop_choice(NORMAL, ROCK, DropChoice::Move);
    }
    idle(&mut m);
    assert!(!m.is_no_set(NORMAL));
    assert!(view_holds(&m, ROCK, NORMAL), "{:?}", sets_text(&m));
}

fn view_holds(m: &AppModel, id: &str, key: &str) -> bool {
    set_views(m)
        .iter()
        .any(|v| v.id == id && v.members.iter().any(|k| k == key))
}

#[test]
fn drop_here_to_start_a_new_set_takes_it_too() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.remove_member(ROCK, NORMAL).unwrap();
    idle(&mut m);
    let id = m.drop_new_set(NORMAL).unwrap();
    idle(&mut m);
    assert!(!m.is_no_set(NORMAL));
    assert!(view_holds(&m, &id, NORMAL));
}

#[test]
fn back_to_its_automatic_set_returns_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.remove_member(ROCK, NORMAL).unwrap();
    idle(&mut m);
    m.select_item(NORMAL);
    let menu = m.item_menu();
    let entry = menu
        .iter()
        .find(|e| e.label == "Back to its automatic set")
        .expect("the entry");
    assert!(entry.disabled.is_none());
    assert!(m.run_menu(entry.action.clone()).is_none());
    idle(&mut m);
    assert!(!m.is_no_set(NORMAL));
    assert!(view_holds(&m, ROCK, NORMAL));
    // A texture in a set has no such entry.
    assert!(m
        .item_menu()
        .iter()
        .all(|e| e.label != "Back to its automatic set"));
}

// ---- borrowing, packed outputs, links ----

#[test]
fn borrowing_from_a_no_set_set_reads_missing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    // leaf borrows slot 1 from rock01; then rock01 goes to no set.
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 1, Some(ROCK))
    });
    idle(&mut m);
    for f in ["landscape/rock01.png", NORMAL, ROUGH, AO] {
        m.remove_member(ROCK, f).ok();
        idle(&mut m);
    }
    let leaf = set_views(&m).into_iter().find(|v| v.id == "landscape/leaf");
    let leaf = leaf.expect("leaf card");
    assert!(
        leaf.missing
            .iter()
            .any(|x| matches!(x, Missing::Borrow { .. })),
        "{:?}",
        leaf.missing
    );
}

#[test]
fn a_packed_output_of_a_no_set_raw_map_is_not_listed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    for f in [ROUGH, AO] {
        m.remove_member(ROCK, f).unwrap();
        idle(&mut m);
    }
    let listed: Vec<String> = m
        .visible_items()
        .into_iter()
        .chain(m.visible_helpers_shared().iter().copied())
        .map(|n| m.items[n].key.clone())
        .collect();
    // The packed output stays in the rock01 set (it still has other files), so it is listed there; every output
    // whose set holds only no-set files is not.
    for p in m.items.iter().filter(|i| i.kind == ItemKind::Packed) {
        if p.set_key.as_deref().is_some_and(|k| !m.in_run(k)) {
            assert!(!listed.contains(&p.key), "{}", p.key);
        }
    }
    for f in [ROUGH, AO] {
        assert!(m.is_no_set(f));
        assert!(listed.contains(&f.to_string()), "{f} is listed in Textures");
    }
}

#[test]
fn nothing_opens_the_set_of_a_no_set_texture() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.remove_member(ROCK, NORMAL).unwrap();
    idle(&mut m);
    assert!(m.set_of_item_probe(NORMAL).is_none());
    let sk = m.item(NORMAL).unwrap().set_key.clone().unwrap();
    assert!(set_views(&m).iter().all(|v| v.key != sk), "no card");
    // Problems and Review rows: none names a set for it.
    assert!(problems(&m).iter().all(|p| match &p.at {
        At::Set { key, .. } => Some(key) != m.item(NORMAL).unwrap().set_key.as_ref(),
        _ => true,
    }));
    // The panel follows no set when the texture is selected.
    m.select_item(NORMAL);
    assert_eq!(sdc_gui::model::set_preview::followed_set(&m), None);
    // The Review "Go to set" and Problems rows take their set key from a view: every key is a card's.
    let keys: Vec<String> = set_views(&m).iter().map(|v| v.key.clone()).collect();
    for b in sdc_gui::model::pbr_export::export_blocks(&m).iter() {
        assert!(b.set_key.is_empty() || keys.contains(&b.set_key), "{b:?}");
    }
    for p in problems(&m).iter() {
        if let At::Set { key, .. } = &p.at {
            assert!(keys.contains(key), "{p:?}");
        }
    }
}

#[test]
fn the_pill_still_counts_a_lone_texture_in_a_one_file_set() {
    let d = tempfile::tempdir().unwrap();
    let m = opened(d.path());
    // `leaf.png` and `odd.png` are alone in their sets and export; the pill counts them (spec: the pill keeps its meaning).
    assert!(m.filter_counts().not_in_set >= 2);
    assert!(m.not_in_set_count() >= 2);
    let mut m = m;
    let before = m.not_in_set_count();
    m.remove_member(ROCK, NORMAL).unwrap();
    idle(&mut m);
    assert_eq!(
        m.not_in_set_count(),
        before + 1,
        "a no-set texture is counted too"
    );
}

// ---- Note HE2 ----

#[test]
fn a_deleted_no_set_file_clears_its_entry_with_an_info_line() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.remove_member(ROCK, NORMAL).unwrap();
    idle(&mut m);
    assert_eq!(no_set(&m), [NORMAL]);
    std::fs::remove_file(m.root.clone().unwrap().join(NORMAL)).unwrap();
    m.reload_changed();
    idle(&mut m);
    idle(&mut m);
    assert!(no_set(&m).is_empty(), "{}", sets_text(&m));
    assert!(m
        .messages
        .entries()
        .any(|e| e.severity == sdc_gui::model::messages::Severity::Info
            && e.text.contains("rock01_normal.png")));
}

// ---- the engine inputs ----

#[test]
fn the_engine_gets_one_no_set_edit_per_entry_after_the_others() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.remove_member(ROCK, NORMAL).unwrap();
    m.edit_docs("t", None, false, |_, s| s.create_set("mine/new"));
    let (edits, _) = engine_inputs(&m);
    assert!(matches!(&edits[0], SetEdit::Create { .. }));
    assert_eq!(
        edits.last(),
        Some(&SetEdit::NoSet {
            file: NORMAL.into()
        })
    );
}

// ---- a no-set twin, on the GUI side ----

#[test]
fn a_no_set_twin_stops_blocking_its_twin() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/clutter");
    std::fs::create_dir_all(&r).unwrap();
    colour(&r.join("rock.png"));
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 3) as u8, (y * 4) as u8, 60u8]))
        .save(r.join("rock.tga"))
        .unwrap();
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"clutter/rock.png\"]\nsubtype = \"diffuse\"\n[files.\"clutter/rock.tga\"]\nsubtype = \"diffuse\"\n",
    )
    .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    idle(&mut m);
    // The twins clash: both are Skipped, for one output.
    let skipped = |m: &AppModel| {
        ["clutter/rock.png", "clutter/rock.tga"]
            .into_iter()
            .filter(|k| matches!(m.item(k).unwrap().status, ItemStatus::Skipped(_)))
            .count()
    };
    assert_eq!(
        skipped(&m),
        2,
        "the twins clash: {:?}",
        m.items
            .iter()
            .map(|i| (&i.key, &i.status))
            .collect::<Vec<_>>()
    );
    m.remove_member("clutter/rock", "clutter/rock.png").unwrap();
    idle(&mut m);
    let tga = m.item("clutter/rock.tga").expect("listed");
    assert_eq!(tga.status, ItemStatus::Ready, "{:?}", tga.status);
    // Toggling `no_set` on an open project is a full plan (a changed request is never a fast re-plan).
    m.back_to_automatic_set(&["clutter/rock.png".to_string()])
        .unwrap();
    idle(&mut m);
    assert_eq!(skipped(&m), 2, "the clash is back");
}

/// Opening helpers for a model on an existing folder.
struct AppModelExt;
impl AppModelExt {
    fn open(root: &Path, dir: &Path) -> AppModel {
        let mut m = opened_unchosen_at(root, dir);
        m.accept = engine::policy::AcceptSuggestions::None;
        idle(&mut m);
        m
    }
    fn reopen(root: &Path, dir: &Path) -> AppModel {
        opened_unchosen_at(root, dir)
    }
}

fn opened_unchosen_at(root: &Path, dir: &Path) -> AppModel {
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(dir.join("export")),
        ..Default::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root.to_path_buf());
    idle(&mut m);
    choose_export(&mut m, dir.join("export"));
    m
}

/// The row of a removed helper reads `No set` in place of its role and where it goes.
#[test]
fn a_no_set_helper_row_reads_no_set() {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    let d = tempfile::tempdir().unwrap();
    let mut m = mouth(d.path());
    let key = "face/mouth_rough.png";
    let rows = |h: &egui_kittest::Harness<'static, AppModel>| -> Vec<String> {
        h.query_all_by_role(Role::Button)
            .filter_map(|n| n.accesskit_node().label())
            .filter(|l| l.starts_with("Helper mouth_rough.png:"))
            .map(|l| l.to_string())
            .collect()
    };
    let mut h = egui_kittest::Harness::builder()
        .with_size(eframe::egui::vec2(1500.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), {
            m.remove_member("face/mouth", key).unwrap();
            idle(&mut m);
            m
        });
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h.run();
    let found = rows(&h);
    assert_eq!(found, ["Helper mouth_rough.png: No set"], "{found:?}");
}

/// Note HF3: a no-set texture asks nothing, so its preview carries no "suggested settings" notice.
#[test]
fn a_no_set_texture_previews_without_a_suggested_settings_notice() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.expand(LEAF);
    assert_eq!(
        m.preview.notice.as_deref(),
        Some("Preview of the suggested settings"),
        "in a set it is still asked"
    );
    m.remove_member("landscape/leaf", LEAF).expect("removed");
    idle(&mut m);
    m.expand(LEAF);
    assert_ne!(
        m.preview.notice.as_deref(),
        Some("Preview of the suggested settings")
    );
}

/// The helper panel's "← set" link goes through the one set check: a no-set helper has none.
#[test]
fn a_no_set_helper_panel_has_no_set_link() {
    use egui_kittest::kittest::Queryable;
    let d = tempfile::tempdir().unwrap();
    let mut m = mouth(d.path());
    let key = "face/mouth_rough.png";
    let draw = |m: AppModel| {
        let mut h = egui_kittest::Harness::builder()
            .with_size(eframe::egui::vec2(1500.0, 900.0))
            .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run();
        h
    };
    m.expand(key);
    let h = draw(m);
    let with_set = h.query_all_by_label_contains("← ").count();
    let mut m = h.into_state();
    m.remove_member("face/mouth", key).unwrap();
    idle(&mut m);
    m.expand(key);
    let h = draw(m);
    let without = h.query_all_by_label_contains("← ").count();
    assert!(with_set >= 1, "the link shows while it is in the set");
    assert_eq!(without, 0, "a no-set helper links to no set");
}

/// Delete set is disabled, with the reason, while the project file takes no edit.
#[test]
fn delete_set_is_disabled_on_a_read_only_project_file() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let root = m.root.clone().unwrap();
    std::fs::write(root.join("sdc-project.toml"), "[[sets]\nbroken").unwrap();
    m.reload_documents();
    idle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().read_only());
    let id = set_views(&m)[0].id.clone();
    let mut h = egui_kittest::Harness::builder()
        .with_size(eframe::egui::vec2(1500.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h.query_all_by_label(&id)
        .next()
        .expect("the set pill")
        .click_secondary();
    h.run();
    let del = h.get_by_label("Delete set");
    assert!(del.accesskit_node().is_disabled(), "disabled");
    assert!(h.state().sets_doc.as_ref().unwrap().read_only());
    assert!(!h.state_mut().delete_set(&id), "the model refuses too");
}
