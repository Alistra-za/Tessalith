//! Note HH (Task 2): drag a helper onto a set to share it. The file stays where it is (its own set, or no set); the
//! target gets one `share` line, and nothing is copied. The set editor lists it with Stop sharing, the helper panel
//! says where else it goes.
mod support;
use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::pack::ChannelSource;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::items::{helper_target_text, ItemStatus};
use sdc_gui::model::pack_fills::channel_helpers;
use sdc_gui::model::sets::{set_views, DropChoice, DropOutcome, SetView, SlotKind};
use sdc_gui::model::{AppModel, Selection};
use std::path::{Path, PathBuf};
use std::time::Duration;
use support::*;

const AO: &str = "a/rock_ao.png";
const A: &str = "a/rock";
const B: &str = "b/stone";
const RMAOS: &str = "b/stone_rmaos";

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "model did not settle");
}

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

fn view(m: &AppModel, id: &str) -> SetView {
    set_views(m)
        .into_iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("no set {id}"))
}

fn has_view(m: &AppModel, id: &str) -> bool {
    set_views(m).iter().any(|v| v.id == id)
}

fn share_line(m: &AppModel, id: &str) -> Vec<String> {
    m.sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|e| e.id == id)
        .map(|e| e.share.clone())
        .unwrap_or_default()
}

fn sets_text(m: &AppModel) -> String {
    m.sets_doc.as_ref().unwrap().text()
}

/// The paths of `packed`'s recipe sources.
fn sources(m: &AppModel, packed: &str) -> Vec<PathBuf> {
    let i = m.item(packed).unwrap_or_else(|| panic!("no item {packed}"));
    let p = i
        .job
        .as_ref()
        .and_then(|j| j.pack.as_ref())
        .expect("a recipe");
    p.sources.iter().map(|s| s.path.clone()).collect()
}

fn holds(m: &AppModel, packed: &str, file: &str) -> bool {
    sources(m, packed).iter().any(|p| p.ends_with(file))
}

/// Every file under `dir` but the project documents.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else if p.extension().is_none_or(|x| x != "toml") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn share(m: &mut AppModel) {
    assert_eq!(m.drop_choice(AO, B, DropChoice::Share), DropOutcome::Joined);
    idle(m);
}

/// `ab` with rock_ao moved out to no set.
fn no_set_ao(dir: &Path) -> AppModel {
    let mut m = ab(dir);
    m.remove_member(A, AO).expect("removed");
    idle(&mut m);
    assert!(m.is_no_set(AO));
    m
}

// ---- the drop ----

#[test]
fn share_writes_one_line_and_no_file() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    let root = m.root.clone().unwrap();
    let before = files_under(&root);
    share(&mut m);
    assert_eq!(share_line(&m, B), [AO]);
    assert!(view(&m, A).members.iter().any(|k| k == AO), "still in a");
    assert!(
        !view(&m, B).members.iter().any(|k| k == AO),
        "no member of b"
    );
    assert_eq!(files_under(&root), before, "no file written");
}

#[test]
fn the_share_reaches_the_plan() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    assert!(!holds(&m, RMAOS, AO));
    share(&mut m);
    assert!(holds(&m, RMAOS, AO), "{:?}", sources(&m, RMAOS));
    assert_eq!(view(&m, B).shared_helpers, [AO]);
    assert!(view(&m, A).shared_helpers.is_empty());
}

#[test]
fn a_lone_helper_still_moves() {
    // A lone cs AO packs an `_rmaos`, so its set has two members; a lone `_opacity` packs nothing.
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(
        d.path(),
        &["a/rock_opacity.png", "b/stone.png", "b/stone_roughness.png"],
        "",
    );
    let op = "a/rock_opacity.png";
    let own = set_views(&m)
        .into_iter()
        .find(|v| v.members.iter().any(|k| k == op))
        .expect("its own set");
    assert_eq!(own.members, [op], "alone in its set");
    assert_eq!(
        m.drop_on_set(op, B),
        DropOutcome::Joined,
        "moved, no question"
    );
    idle(&mut m);
    assert!(view(&m, B).members.iter().any(|k| k == op));
    assert!(share_line(&m, B).is_empty());
}

#[test]
fn a_helper_from_a_real_set_asks() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    assert_eq!(
        m.drop_on_set(AO, B),
        DropOutcome::AskChoice { from: A.into() }
    );
    // Share is allowed for it (a raw map, so no finished output to borrow).
    share(&mut m);
}

#[test]
fn a_no_set_helper_asks_and_shares() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_set_ao(d.path());
    assert_eq!(
        m.drop_on_set(AO, B),
        DropOutcome::AskChoice {
            from: "no set".into()
        }
    );
    share(&mut m);
    assert_eq!(share_line(&m, B), [AO]);
    assert!(m.is_no_set(AO), "still in no set");
    assert!(
        set_views(&m)
            .iter()
            .all(|v| !v.members.iter().any(|k| k == AO)),
        "no card holds it"
    );
    let ch = &m
        .item(RMAOS)
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .pack
        .as_ref()
        .unwrap()
        .channels[2];
    assert!(
        matches!(ch, ChannelSource::Source { path, .. } if path.ends_with(AO)),
        "{ch:?}"
    );
}

#[test]
fn a_no_set_helper_can_still_move() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_set_ao(d.path());
    // Shared first: the move takes the share line with it (Note HH6).
    share(&mut m);
    assert_eq!(m.drop_choice(AO, B, DropChoice::Move), DropOutcome::Joined);
    idle(&mut m);
    assert!(!m.is_no_set(AO));
    assert!(view(&m, B).members.iter().any(|k| k == AO));
    assert!(share_line(&m, B).is_empty(), "{}", sets_text(&m));
}

#[test]
fn a_plain_no_set_texture_still_moves() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path(), &["a/rock.png", "a/rock_n.png", "b/stone.png"], "");
    m.remove_member(A, "a/rock_n.png").unwrap();
    idle(&mut m);
    assert!(m.is_no_set("a/rock_n.png"));
    assert_eq!(m.drop_on_set("a/rock_n.png", B), DropOutcome::Joined);
    idle(&mut m);
    assert!(view(&m, B).members.iter().any(|k| k == "a/rock_n.png"));
}

#[test]
fn an_empty_set_refuses_a_share() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    m.edit_docs("t", None, true, |_, s| s.create_set("e/empty"));
    idle(&mut m);
    assert!(has_view(&m, "e/empty"));
    let before = sets_text(&m);
    assert_eq!(
        m.drop_choice(AO, "e/empty", DropChoice::Share),
        DropOutcome::Refused("e/empty has no texture of its own; move one in first".into())
    );
    assert_eq!(sets_text(&m), before);
}

#[test]
fn a_finished_texture_still_borrows() {
    let d = tempfile::tempdir().unwrap();
    // `c` has no RMAOS of its own, so the borrow needs no replace.
    let mut m = fixture(d.path(), &["a/rock.png", AO, "c/slab.png"], "");
    assert_eq!(
        m.drop_choice("a/rock_rmaos", "c/slab", DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    let e = m.sets_doc.as_ref().unwrap().entries();
    let slab = e.iter().find(|e| e.id == "c/slab").expect("c's entry");
    let n = SlotKind::Rmaos.slot_number().unwrap();
    assert_eq!(
        slab.borrow,
        [(n, A.to_string())].into(),
        "borrow.slot{n} = a/rock"
    );
    assert!(slab.share.is_empty());
}

#[test]
fn a_share_on_its_own_set_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    let before = sets_text(&m);
    assert_eq!(m.drop_choice(AO, A, DropChoice::Share), DropOutcome::Joined);
    assert_eq!(sets_text(&m), before);
    // A no-set helper on the set its name puts it in (where Back to its automatic set returns it) writes nothing.
    let mut m = no_set_ao(d.path().join("n").as_path());
    let before = sets_text(&m);
    assert_eq!(m.drop_choice(AO, A, DropChoice::Share), DropOutcome::Joined);
    assert_eq!(sets_text(&m), before);
}

#[test]
fn a_share_line_naming_a_member_draws_no_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    m.edit_docs("t", None, true, |_, s| s.add_share(A, AO));
    idle(&mut m);
    assert_eq!(share_line(&m, A), [AO], "the hand-written line");
    assert!(view(&m, A).shared_helpers.is_empty());
}

#[test]
fn stop_sharing_what_is_not_shared_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    let steps = m.history.undo.len();
    assert_eq!(
        m.stop_sharing(B, AO),
        Err("b/stone does not share rock_ao.png".into())
    );
    assert_eq!(m.history.undo.len(), steps, "no undo step");
}

#[test]
fn stop_sharing_takes_a_line_in_another_case() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    m.edit_docs("t", None, true, |_, s| s.add_share(B, "A/Rock_AO.png"));
    idle(&mut m);
    assert_eq!(view(&m, B).shared_helpers, [AO]);
    m.stop_sharing(B, AO).expect("stopped");
    idle(&mut m);
    assert!(share_line(&m, B).is_empty(), "{}", sets_text(&m));
    assert!(view(&m, B).shared_helpers.is_empty());
}

#[test]
fn a_non_helper_still_cannot_be_shared() {
    // Pins of `choice_allowed`: a file with no slot that is no helper, and a no-set texture.
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(
        d.path(),
        &[
            "a/rock.png",
            "a/rock_n.png",
            AO,
            "b/stone.png",
            "b/iron_ore.png",
        ],
        "",
    );
    assert!(!m.item("b/iron_ore.png").unwrap().helper.is_some());
    assert!(matches!(
        m.drop_choice("b/iron_ore.png", A, DropChoice::Share),
        DropOutcome::Refused(r) if r.starts_with("only a finished texture can be shared")
    ));
    m.remove_member(A, "a/rock_n.png").unwrap();
    idle(&mut m);
    assert_eq!(
        m.drop_choice("a/rock_n.png", B, DropChoice::Share),
        DropOutcome::Refused("the texture is in no set".into())
    );
}

#[test]
fn a_helper_no_set_uses_reads_not_used() {
    // b's own `stone_ao.png` sorts before `z/rock_ao.tga` and takes the channel; `z`'s `rock_ao.png` takes its own.
    let d = tempfile::tempdir().unwrap();
    let tga = "z/rock_ao.tga";
    let mut m = fixture(
        d.path(),
        &[
            "z/rock.png",
            "z/rock_ao.png",
            tga,
            "b/stone.png",
            "b/stone_roughness.png",
            "b/stone_ao.png",
        ],
        "",
    );
    m.edit_docs("t", None, true, |_, s| s.add_share(B, tga));
    idle(&mut m);
    let h = m.item(tga).unwrap().helper.clone().expect("a helper");
    assert_eq!(h.shared_with.len(), 1, "shared with b");
    assert!(sdc_gui::model::items::used_by(&h).is_none());
    let text = helper_target_text(&h);
    assert!(text.starts_with("not used"), "{text}");
}

#[test]
fn a_share_survives_reopen() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    share(&mut m);
    m.sets_doc.as_mut().unwrap().save(true).unwrap();
    let again = opened_as_the_app(m.root.clone().unwrap(), d.path().join("export"));
    assert_eq!(share_line(&again, B), [AO]);
    assert_eq!(view(&again, B).shared_helpers, [AO]);
    assert!(holds(&again, RMAOS, AO));
}

#[test]
fn stop_sharing_removes_the_line() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    share(&mut m);
    m.stop_sharing(B, AO).expect("stopped");
    idle(&mut m);
    assert!(share_line(&m, B).is_empty());
    // The emptied key goes with the line.
    assert!(!sets_text(&m).contains("share"), "{}", sets_text(&m));
    assert!(!sets_text(&m).contains(AO), "{}", sets_text(&m));
    assert!(!holds(&m, RMAOS, AO));
    assert!(view(&m, B).shared_helpers.is_empty());
}

#[test]
fn a_read_only_doc_refuses() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    share(&mut m);
    m.sets_doc.as_mut().unwrap().save(true).unwrap();
    let root = m.root.clone().unwrap();
    std::fs::write(root.join("sdc-project.toml"), "[[sets]\nbroken").unwrap();
    m.reload_documents();
    idle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().read_only());
    assert!(
        matches!(m.drop_choice(AO, B, DropChoice::Share), DropOutcome::Refused(r) if r.starts_with("sdc-project.toml is read-only"))
    );
    assert!(m
        .stop_sharing(B, AO)
        .is_err_and(|e| e.starts_with("sdc-project.toml is read-only")));
}

#[test]
fn a_deleted_shared_file_clears_with_info() {
    // In no set (its no-set entry goes too, HF1) and in a real set.
    for no_set in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let mut m = if no_set {
            no_set_ao(d.path())
        } else {
            ab(d.path())
        };
        share(&mut m);
        std::fs::remove_file(m.root.clone().unwrap().join(AO)).unwrap();
        m.reload_changed();
        idle(&mut m);
        idle(&mut m);
        assert!(share_line(&m, B).is_empty(), "{no_set}: {}", sets_text(&m));
        assert!(
            m.messages
                .entries()
                .any(|e| e.severity == sdc_gui::model::messages::Severity::Info
                    && e.text.contains("rock_ao.png")
                    && e.text.contains(B)),
            "{no_set}"
        );
        assert!(view(&m, B).missing.is_empty(), "{no_set}: no red row");
        assert!(m.item(RMAOS).is_some(), "{no_set}: the plan succeeded");
    }
}

#[test]
fn the_picker_offers_a_shared_helper() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    let listed = |m: &AppModel| -> Vec<String> {
        channel_helpers(m, RMAOS)
            .into_iter()
            .map(|(k, ..)| k)
            .collect()
    };
    assert!(!listed(&m).iter().any(|k| k == AO));
    share(&mut m);
    assert!(listed(&m).iter().any(|k| k == AO), "{:?}", listed(&m));
    m.set_channel_helper(RMAOS, 2, Some(AO));
    idle(&mut m);
    let text = m.project.as_ref().unwrap().text();
    assert!(
        text.contains("[files.\"a/rock_ao.png\"]") && text.contains("rmaos.b"),
        "{text}"
    );
}

#[test]
fn an_edit_on_the_helper_refreshes_the_target() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    share(&mut m);
    m.set_helper_invert(AO, true);
    idle(&mut m);
    let ch = &m
        .item(RMAOS)
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .pack
        .as_ref()
        .unwrap()
        .channels[2];
    assert!(
        matches!(ch, ChannelSource::Source { path, invert: true, .. } if path.ends_with(AO)),
        "{ch:?}"
    );
}

#[test]
fn a_share_keeps_a_waiting_sets_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path(), &["a/rock.png", AO, "b/iron_ore.png"], "");
    let ore = "b/iron_ore.png";
    assert!(m.item(ore).unwrap().waiting, "precondition: it waits");
    let key = m.item(ore).unwrap().set_key.clone().unwrap();
    assert!(set_views(&m).iter().all(|v| v.key != key), "no card");
    m.edit_docs("t", None, true, |_, s| s.add_share("b/iron_ore", AO));
    idle(&mut m);
    assert!(set_views(&m).iter().any(|v| v.key == key), "the card shows");
    assert!(m.item(ore).unwrap().waiting, "the member still waits (GY1)");
}

#[test]
fn export_set_does_not_pull_in_the_helpers_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    share(&mut m);
    let b = view(&m, B).key;
    m.export_sets([b.clone()].into(), "Export set b/stone".into());
    let keys = m.export_scope().expect("a scope").keys.clone();
    assert_eq!(keys, [b].into());
    assert!(
        m.review.is_none(),
        "b asks nothing, so no Review: {:?}",
        m.dialog
    );
    idle(&mut m);
    // A shared AO next to a roughness makes `b` a PBR set, so it writes under `textures/pbr/`.
    let written = files_under(&m.export_root());
    let named = |n: &str| written.iter().any(|p| p.ends_with(n));
    assert!(named("stone_rmaos.dds"), "{written:?}");
    assert!(
        !named("rock.dds") && !named("rock_rmaos.dds"),
        "a is not in the run: {written:?}"
    );
}

#[test]
fn the_helper_row_says_where_it_goes() {
    // A lone opacity map fills nothing in its own set, so the share is all it does.
    let d = tempfile::tempdir().unwrap();
    let op = "a/rock_opacity.png";
    let mut m = fixture(
        d.path(),
        &[op, "b/stone.png", "b/stone_roughness.png"],
        "[files.\"a/rock_opacity.png\"]\nhelper.fills = [\"diffuse.a\"]\n",
    );
    m.edit_docs("t", None, true, |_, s| s.add_share(B, op));
    idle(&mut m);
    let i = m.item(op).unwrap();
    let h = i.helper.clone().expect("a helper");
    assert!(h.fills.is_empty(), "{:?}", h.fills);
    assert_eq!(helper_target_text(&h), "used by stone");
    assert!(
        matches!(i.status, ItemStatus::PackedInto(_)),
        "{:?}",
        i.status
    );
}

// ---- the UI ----

fn harness(m: AppModel, w: f32) -> Harness<'static, AppModel> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(w, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h.run();
    h
}

fn editing_b(dir: &Path, helper: &str) -> AppModel {
    let mut m = fixture(
        dir,
        &["a/rock.png", helper, "b/stone.png", "b/stone_roughness.png"],
        "",
    );
    assert_eq!(
        m.drop_choice(helper, B, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    let key = view(&m, B).key;
    m.select_set(&key);
    m
}

#[test]
fn the_target_lists_the_shared_helper_with_stop() {
    let d = tempfile::tempdir().unwrap();
    let m = editing_b(d.path(), AO);
    let mut h = harness(m, 1100.0);
    let centre = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre");
    assert!(
        h.query_all_by_label_contains("rock_ao.png")
            .any(|n| centre.contains(n.rect().center())),
        "a row names it in the editor"
    );
    // Below the slot rows: scrolled into view first, as a user would.
    h.get_by_role_and_label(Role::Button, "Stop sharing rock_ao.png")
        .scroll_to_me();
    h.run();
    h.run();
    // A pointer click: the ✕ must not sit under the editor's scroll bar.
    h.get_by_role_and_label(Role::Button, "Stop sharing rock_ao.png")
        .click();
    h.run();
    h.run();
    let m = h.state_mut();
    idle(m);
    assert!(share_line(m, B).is_empty(), "{}", sets_text(m));
}

#[test]
fn a_click_on_the_row_selects_the_helper() {
    let d = tempfile::tempdir().unwrap();
    let m = editing_b(d.path(), AO);
    let mut h = harness(m, 1100.0);
    let centre = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre");
    let stop = h.get_by_role_and_label(Role::Button, "Stop sharing rock_ao.png");
    stop.scroll_to_me();
    h.run();
    h.run();
    let stop = h
        .get_by_role_and_label(Role::Button, "Stop sharing rock_ao.png")
        .rect();
    let name = h
        .query_all_by_label("rock_ao.png (shared)")
        .map(|n| n.rect())
        .find(|r| centre.contains(r.center()))
        .expect("the row's name");
    // The blank between the name and the ✕: neither widget, only the row.
    let at = egui::pos2((name.max.x + stop.min.x) / 2.0, stop.center().y);
    assert!(name.max.x + 4.0 < stop.min.x, "a gap to click");
    h.hover_at(at);
    h.run();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
    }
    // The set editor waits out a double click before it selects.
    for _ in 0..60 {
        h.run_steps(1);
        if h.state().selection == Selection::Item(AO.into()) {
            break;
        }
    }
    assert_eq!(h.state().selection, Selection::Item(AO.into()));
    assert_eq!(share_line(h.state(), B), [AO], "a row click edits nothing");
}

#[test]
fn the_helper_panel_lists_where_it_goes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    share(&mut m);
    let b = view(&m, B).key;
    m.select_item(AO);
    let mut h = harness(m, 1100.0);
    h.get_by_label("Shared with");
    // egui names a link by its text (role Label); the set card's own label is "Set b/stone, …".
    let centre = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre");
    h.query_all_by_label(B)
        .find(|n| centre.contains(n.rect().center()))
        .expect("the link in the panel")
        .click();
    h.run();
    assert_eq!(h.state().selection, Selection::Set(b));
}

/// Note HI16: the tick's hover says which other sets use the file; unticked, none of them does.
#[test]
fn a_shared_helpers_tick_names_the_sets() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ab(d.path());
    share(&mut m);
    let packed_with_ao = |m: &AppModel| -> Vec<String> {
        m.items
            .iter()
            .filter(|i| i.job.as_ref().is_some_and(|j| j.pack.is_some()))
            .filter(|i| holds(m, &i.key, "rock_ao.png"))
            .map(|i| i.key.clone())
            .collect()
    };
    assert!(holds(&m, RMAOS, AO));
    assert!(
        packed_with_ao(&m).len() >= 2,
        "a and b both use it: {:?}",
        packed_with_ao(&m)
    );
    m.select_item(AO);
    let mut h = harness(m, 1100.0);
    h.get_by_role_and_label(Role::CheckBox, "Helper").hover();
    h.run();
    assert!(h
        .query_by_label_contains("Also used by 1 other set")
        .is_some());
    h.get_by_role_and_label(Role::CheckBox, "Helper").click();
    h.run();
    let m = h.state_mut();
    idle(m);
    assert_eq!(
        m.project.as_ref().unwrap().file_value(AO, "helper.role"),
        Some(serde_json::json!("none"))
    );
    assert!(packed_with_ao(m).is_empty(), "{:?}", packed_with_ao(m));
    assert_eq!(share_line(m, B), [AO], "the share line stays");
}

#[test]
fn the_row_stays_inside_the_column() {
    let d = tempfile::tempdir().unwrap();
    let long =
        "a/rock_with_a_name_long_enough_to_need_truncation_at_eleven_hundred_points_wide_ao.png";
    let m = editing_b(d.path(), long);
    let h = harness(m, 1100.0);
    let centre = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre");
    let name = long.rsplit('/').next().unwrap();
    let stop_name = format!("Stop sharing {name}");
    let stop = h.get_by_role_and_label(Role::Button, &stop_name).rect();
    let mut rects: Vec<egui::Rect> = h
        .query_all_by_label_contains("rock_with_a_name")
        .map(|n| n.rect())
        .filter(|r| r.min.x < centre.max.x && r.min.x >= centre.min.x)
        .collect();
    rects.push(stop);
    assert!(rects.len() >= 2, "the row's name and its button");
    for r in rects {
        assert!(
            r.max.x <= centre.max.x,
            "ends at {} past {}",
            r.max.x,
            centre.max.x
        );
    }
}

#[test]
fn a_helper_row_drags_onto_a_card() {
    let d = tempfile::tempdir().unwrap();
    let m = no_set_ao(d.path());
    let mut h = harness(m, 1500.0);
    let row = h
        .query_all_by_role(Role::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Helper rock_ao.png:"))
        })
        .expect("the helper row")
        .rect();
    let sets = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("sets"))
        .expect("the Sets column")
        .outer_rect;
    let card = h
        .query_all_by_label(B)
        .map(|n| n.rect())
        .find(|r| sets.contains(r.center()))
        .expect("stone's card");
    let at = row.center();
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.hover_at(at);
    h.run();
    h.event(press(at, true));
    h.run();
    let to = card.center();
    for step in 1..=6 {
        let p = at + (to - at) * (step as f32 / 6.0);
        h.event(egui::Event::PointerMoved(p));
        h.run_steps(2);
    }
    h.event(press(to, false));
    h.run_steps(3);
    h.get_by_label("rock_ao.png is in no set");
    assert!(h
        .query_by_label("Duplicate the source for this set")
        .is_none());
    // The dialog's Share goes through `drop_choice`: the line is written.
    h.get_by_role_and_label(Role::Button, "Share (use this file here too, no copy)")
        .click();
    h.run();
    let m = h.state_mut();
    idle(m);
    assert_eq!(share_line(m, B), [AO]);
    assert!(m.is_no_set(AO));
}

/// A click on a Textures-column helper row still selects it, now that the row also drags.
#[test]
fn a_click_on_a_helper_row_still_selects() {
    let d = tempfile::tempdir().unwrap();
    let m = ab(d.path());
    let mut h = harness(m, 1500.0);
    h.query_all_by_role(Role::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Helper rock_ao.png:"))
        })
        .expect("the helper row")
        .click();
    h.run();
    h.run();
    assert_eq!(h.state().selection, Selection::Item(AO.into()));
}

/// Review pictures of the set editor with a shared helper at 1100, 1280 and 1600 px (egui-ui-rules step 5). Needs a
/// GPU and `SDC_SHOTS_DIR`: `cargo test -p sdc-gui --test helper_share -- --ignored shots`.
#[test]
#[ignore]
fn shots() {
    let Ok(out) = std::env::var("SDC_SHOTS_DIR") else {
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let long = "a/rock_with_a_long_name_ao.png";
    for w in [1100.0, 1280.0, 1600.0] {
        let d = d.path().join(format!("{w}"));
        let m = editing_b(&d, long);
        let mut h = Harness::builder()
            .with_size(egui::vec2(w, 900.0))
            .with_pixels_per_point(1.0)
            .wgpu()
            .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run_steps(10);
        let img = h.render().expect("render");
        img.save(Path::new(&out).join(format!("helper-share-editor-{w}.png")))
            .expect("save png");
    }
}
