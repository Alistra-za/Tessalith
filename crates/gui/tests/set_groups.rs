//! Note HF4: set groups in the model and the project file. Membership lives in `[[groups]]`; what a group shows reads
//! the views (a set with no card is not shown in it and a group never keeps a card); the view order pulls members
//! behind the first one.
mod support;
use image::{ImageBuffer, Rgb};
use sdc_gui::model::set_groups::{GroupTarget, SetGroup};
use sdc_gui::model::sets::set_views;
use sdc_gui::model::AppModel;
use std::path::Path;
use std::time::Duration;
use support::*;

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "model did not settle");
}

/// `textures/g`: five sets `g/a` .. `g/e`, each a colour map and a `_n` normal.
fn five(dir: &Path) -> AppModel {
    let g = dir.join("textures/g");
    std::fs::create_dir_all(&g).unwrap();
    for n in ["a", "b", "c", "d", "e"] {
        ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
            .save(g.join(format!("{n}.png")))
            .unwrap();
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
            .save(g.join(format!("{n}_n.png")))
            .unwrap();
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    idle(&mut m);
    m
}

fn ids(m: &AppModel) -> Vec<String> {
    set_views(m).iter().map(|v| v.id.clone()).collect()
}

fn text(m: &AppModel) -> String {
    m.sets_doc.as_ref().unwrap().text()
}

fn stored(m: &AppModel) -> Vec<SetGroup> {
    m.sets_doc.as_ref().unwrap().groups()
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn new_group(m: &mut AppModel, name: &str, sets: &[&str]) {
    m.move_to_group(&s(sets), GroupTarget::New(name.into()))
        .expect("grouped");
}

#[test]
fn a_group_is_stored_as_the_spec_shows_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/a", "g/b"]);
    assert!(
        text(&m).contains("[[groups]]\nname = \"Armor\"\nsets = [\"g/a\", \"g/b\"]\n"),
        "{}",
        text(&m)
    );
    assert!(!text(&m).contains("excluded"), "left out when false");
    m.set_group_excluded("Armor", true).unwrap();
    assert!(
        text(&m).contains("sets = [\"g/a\", \"g/b\"]\nexcluded = true\n"),
        "{}",
        text(&m)
    );
    m.set_group_excluded("Armor", false).unwrap();
    assert!(!text(&m).contains("excluded"), "false is not written");
}

#[test]
fn a_new_group_holds_the_set_and_goes_with_its_last_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/a"]);
    let g = m.group_of("g/a").expect("in a group");
    assert_eq!(
        (g.name.as_str(), g.sets.as_slice()),
        ("Armor", &s(&["g/a"])[..])
    );
    assert!(m.group_of("g/b").is_none());
    // A second set joins; the first leaves; the group stays while one is in it.
    m.move_to_group(&s(&["g/b"]), GroupTarget::Existing("armor".into()))
        .unwrap();
    m.move_to_group(&s(&["g/a"]), GroupTarget::None).unwrap();
    assert_eq!(stored(&m)[0].sets, s(&["g/b"]));
    // Its last set leaves: the group goes, key and all.
    m.move_to_group(&s(&["g/b"]), GroupTarget::None).unwrap();
    assert!(stored(&m).is_empty());
    assert!(!text(&m).contains("groups"), "{}", text(&m));
    // Moving into a group takes the set out of the one it was in.
    new_group(&mut m, "One", &["g/a"]);
    new_group(&mut m, "Two", &["g/a"]);
    assert_eq!(m.group_of("g/a").unwrap().name, "Two");
    assert_eq!(stored(&m).len(), 1, "One lost its last set");
}

#[test]
fn names_are_unique_ignoring_case() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/a"]);
    new_group(&mut m, "Weapons", &["g/b"]);
    let err = m
        .move_to_group(&s(&["g/c"]), GroupTarget::New("ARMOR".into()))
        .unwrap_err();
    assert!(err.contains("Armor") || err.contains("ARMOR"), "{err}");
    let err = m.rename_group("Weapons", "armor").unwrap_err();
    assert!(err.contains("already"), "{err}");
    assert_eq!(stored(&m)[1].name, "Weapons", "refused: nothing changed");
    // Its own name in another case is its own.
    m.rename_group("Weapons", "WEAPONS").unwrap();
    assert_eq!(stored(&m)[1].name, "WEAPONS");
    m.rename_group("WEAPONS", "Blades").unwrap();
    assert!(m.group_of("g/b").is_some_and(|g| g.name == "Blades"));
    assert!(m
        .move_to_group(&s(&["g/c"]), GroupTarget::New("  ".into()))
        .is_err());
    assert!(m
        .move_to_group(&s(&["g/c"]), GroupTarget::Existing("Nope".into()))
        .is_err());
}

#[test]
fn ungroup_keeps_the_sets() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/a", "g/b"]);
    let before = ids(&m);
    m.ungroup("armor").unwrap();
    assert!(stored(&m).is_empty());
    assert_eq!(ids(&m), before, "the sets are all still there");
    assert!(m.group_of("g/a").is_none());
}

#[test]
fn every_change_is_one_undo_step_and_no_replan() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    let step = |m: &mut AppModel, what: &str, f: &dyn Fn(&mut AppModel)| {
        let (before, steps) = (text(m), m.history.undo.len());
        f(m);
        assert!(!m.busy(), "{what}: no re-plan");
        assert_eq!(m.history.undo.len(), steps + 1, "{what}: one undo step");
        assert_ne!(text(m), before, "{what}: changed");
        let after = text(m);
        m.undo();
        idle(m);
        assert_eq!(text(m), before, "{what}: undone");
        m.redo();
        idle(m);
        assert_eq!(text(m), after, "{what}: redone");
    };
    step(&mut m, "new group", &|m| {
        new_group(m, "Armor", &["g/a", "g/b"])
    });
    step(&mut m, "join", &|m| {
        m.move_to_group(&s(&["g/c"]), GroupTarget::Existing("Armor".into()))
            .unwrap()
    });
    step(&mut m, "exclude", &|m| {
        m.set_group_excluded("Armor", true).unwrap()
    });
    step(&mut m, "rename", &|m| {
        m.rename_group("Armor", "Plate").unwrap()
    });
    step(&mut m, "leave", &|m| {
        m.move_to_group(&s(&["g/a"]), GroupTarget::None).unwrap()
    });
    step(&mut m, "ungroup", &|m| m.ungroup("Plate").unwrap());
}

#[test]
fn a_renamed_set_stays_in_its_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/a", "g/b"]);
    m.rename_set("g/a", "g/zed").unwrap();
    idle(&mut m);
    assert_eq!(stored(&m)[0].sets, s(&["g/zed", "g/b"]));
    assert_eq!(m.group_of("g/zed").unwrap().name, "Armor");
}

/// Review Focus 2: an input rename moves an automatic set's id (`retarget_auto`); its membership follows, whether the
/// set has an entry in the file or only a place in a group. Driven through the real input-rename path.
#[test]
fn an_input_rename_carries_membership() {
    for with_entry in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let mut m = pbr_opened(d.path());
        if with_entry {
            m.set_json_value(
                "landscape/rock01",
                "specular_level",
                Some(serde_json::json!(0.5)),
            );
        }
        let other = ids(&m)
            .into_iter()
            .find(|i| i != "landscape/rock01")
            .unwrap();
        new_group(&mut m, "Rocks", &["landscape/rock01", &other]);
        m.set_input_name("landscape/rock01", "textures/landscape/rock02")
            .unwrap();
        idle(&mut m);
        assert!(
            ids(&m).contains(&"landscape/rock02".to_string()),
            "{with_entry}"
        );
        assert_eq!(
            m.group_of("landscape/rock02").map(|g| g.name),
            Some("Rocks".into()),
            "entry: {with_entry}"
        );
        assert!(!stored(&m)[0].sets.iter().any(|x| x == "landscape/rock01"));
    }
}

/// `retarget_auto` on the document: no entry, an entry under the automatic id, and a renamed set.
#[test]
fn retarget_auto_moves_the_ids_a_group_holds() {
    let d = tempfile::tempdir().unwrap();
    let mut doc = sdc_gui::sets_doc::SetsDoc::open(d.path());
    doc.restore(
        "[[sets]]\nid = \"g/b\"\n[[sets]]\nid = \"g/c\"\nrenamed_from = \"g/cc\"\n\
         [[groups]]\nname = \"Armor\"\nsets = [\"g/a\", \"g/b\", \"g/c\"]\n",
    );
    doc.retarget_auto("g/a", "g/aa");
    assert_eq!(doc.groups()[0].sets, s(&["g/aa", "g/b", "g/c"]));
    doc.retarget_auto("g/b", "g/bb");
    assert_eq!(doc.groups()[0].sets, s(&["g/aa", "g/bb", "g/c"]));
    // A renamed set keeps the user's name, which is what the group holds.
    doc.retarget_auto("g/cc", "g/ccc");
    assert_eq!(doc.groups()[0].sets, s(&["g/aa", "g/bb", "g/c"]));
}

#[test]
fn deleting_a_set_takes_it_out_and_undo_restores_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/a", "g/b"]);
    let (before, steps) = (text(&m), m.history.undo.len());
    assert!(m.delete_set("g/a"));
    idle(&mut m);
    assert_eq!(m.history.undo.len(), steps + 1, "one step");
    assert_eq!(stored(&m)[0].sets, s(&["g/b"]));
    assert_eq!(m.sets_doc.as_ref().unwrap().no_set().len(), 2);
    // The last member: the group goes with it.
    assert!(m.delete_set("g/b"));
    idle(&mut m);
    assert!(stored(&m).is_empty());
    m.undo();
    idle(&mut m);
    m.undo();
    idle(&mut m);
    assert_eq!(
        text(&m),
        before,
        "no_set, the entry and the membership are back"
    );
    assert_eq!(stored(&m)[0].sets, s(&["g/a", "g/b"]));
    assert!(m.sets_doc.as_ref().unwrap().no_set().is_empty());
    assert!(ids(&m).contains(&"g/a".to_string()));
}

#[test]
fn a_read_only_document_refuses_with_the_reason() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    let root = m.root.clone().unwrap();
    std::fs::write(root.join("sdc-project.toml"), "[[sets]\nbroken").unwrap();
    m.reload_documents();
    idle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().read_only());
    let steps = m.history.undo.len();
    let new = GroupTarget::New("Armor".into());
    for r in [
        m.move_to_group(&s(&["g/a"]), new),
        m.rename_group("Armor", "Plate"),
        m.ungroup("Armor"),
        m.set_group_excluded("Armor", true),
    ] {
        let why = r.unwrap_err();
        assert!(why.contains("read-only"), "{why}");
    }
    assert_eq!(m.history.undo.len(), steps);
    assert!(stored(&m).is_empty());
}

#[test]
fn collapse_is_kept_by_project_and_name_and_is_no_undo_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    let gui_toml = d.path().join("settings/gui.toml");
    m.settings_path = Some(gui_toml.clone());
    new_group(&mut m, "Armor", &["g/a"]);
    let (steps, doc) = (m.history.undo.len(), text(&m));
    assert!(!m.group_collapsed("Armor"));
    m.set_group_collapsed("Armor", true);
    assert!(m.group_collapsed("armor"), "by name, ignoring case");
    assert!(!m.group_collapsed("Weapons"));
    assert_eq!((m.history.undo.len(), text(&m)), (steps, doc));
    let written = std::fs::read_to_string(&gui_toml).unwrap();
    assert!(written.contains("group_collapsed"), "{written}");
    // Another project with the same settings and group name starts open.
    let d2 = tempfile::tempdir().unwrap();
    let mut m2 = five(d2.path());
    m2.settings = m.settings.clone();
    assert!(!m2.group_collapsed("Armor"));
    m.set_group_collapsed("Armor", false);
    assert!(!m.group_collapsed("Armor"));
    assert!(m.settings.group_collapsed.is_empty(), "open is not stored");
}

/// A collapsed group's key follows a Rename, and goes with the group (Ungroup, its last set leaving or deleted), so a
/// later group with the old name starts open.
#[test]
fn the_collapsed_key_follows_the_group_and_goes_with_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/a", "g/b"]);
    m.set_group_collapsed("Armor", true);
    m.rename_group("Armor", "Plate").unwrap();
    assert!(m.group_collapsed("Plate") && !m.group_collapsed("Armor"));
    assert_eq!(m.settings.group_collapsed.len(), 1);
    m.ungroup("Plate").unwrap();
    assert!(m.settings.group_collapsed.is_empty(), "Ungroup");
    new_group(&mut m, "Plate", &["g/a"]);
    assert!(
        !m.group_collapsed("Plate"),
        "a new group of the old name starts open"
    );
    m.set_group_collapsed("Plate", true);
    m.move_to_group(&s(&["g/a"]), GroupTarget::None).unwrap();
    assert!(m.settings.group_collapsed.is_empty(), "its last set left");
    new_group(&mut m, "Plate", &["g/a"]);
    m.set_group_collapsed("Plate", true);
    assert!(m.delete_set("g/a"));
    idle(&mut m);
    assert!(
        m.settings.group_collapsed.is_empty(),
        "its last set was deleted"
    );
}

/// Note GY5: a lone file waiting for a type has no card. Being in a group does not give it one, and it is not shown
/// or counted in the group until it gets one.
#[test]
fn a_waiting_lone_file_in_a_group_has_no_card_until_it_gets_one() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("textures/g")).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(d.path().join("textures/g/zz_bump.png"))
        .unwrap();
    let mut m = five(d.path());
    assert!(
        !ids(&m).contains(&"g/zz_bump".to_string()),
        "waits for a type"
    );
    new_group(&mut m, "Armor", &["g/a", "g/zz_bump"]);
    idle(&mut m);
    assert!(
        !ids(&m).contains(&"g/zz_bump".to_string()),
        "the group gives it no card"
    );
    assert_eq!(m.group_of("g/a").unwrap().sets, s(&["g/a"]));
    assert!(m.group_of("g/zz_bump").is_none());
    assert_eq!(
        stored(&m)[0].sets,
        s(&["g/a", "g/zz_bump"]),
        "the file keeps it"
    );
    m.decide(
        "g/zz_bump.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    idle(&mut m);
    assert_eq!(m.group_of("g/a").unwrap().sets, s(&["g/a", "g/zz_bump"]));
}

// ---- Order ----

#[test]
fn members_follow_the_first_member_in_view_order() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    assert_eq!(ids(&m), s(&["g/a", "g/b", "g/c", "g/d", "g/e"]));
    // d and b are a group; the block sits where b sits and d comes straight after it.
    new_group(&mut m, "Armor", &["g/d", "g/b"]);
    assert_eq!(ids(&m), s(&["g/a", "g/b", "g/d", "g/c", "g/e"]));
    // The saved order is not rewritten by the pull.
    assert!(m.sets_doc.as_ref().unwrap().order().is_empty());
}

#[test]
fn moving_the_first_member_moves_the_block() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/b", "g/c"]);
    assert_eq!(ids(&m), s(&["g/a", "g/b", "g/c", "g/d", "g/e"]));
    // b is first in the block: moving it to the end of the list takes c along.
    assert!(m.reorder_set("g/b", 3));
    idle(&mut m);
    assert_eq!(ids(&m), s(&["g/a", "g/d", "g/e", "g/b", "g/c"]));
    // And back to the front.
    assert!(m.reorder_set("g/b", 0));
    idle(&mut m);
    assert_eq!(ids(&m), s(&["g/b", "g/c", "g/a", "g/d", "g/e"]));
}

#[test]
fn moving_a_set_through_a_block_lands_after_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/b", "g/c"]);
    // a goes to position 1, between b and c as the list would read without it: it lands after the block.
    assert!(m.reorder_set("g/a", 1));
    idle(&mut m);
    assert_eq!(ids(&m), s(&["g/b", "g/c", "g/a", "g/d", "g/e"]));
}

/// A set stepping up from just below a block lands before the whole block.
#[test]
fn a_set_stepping_up_from_below_a_block_lands_before_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/b", "g/c"]);
    assert!(m.reorder_set("g/d", 2));
    idle(&mut m);
    assert_eq!(ids(&m), s(&["g/a", "g/d", "g/b", "g/c", "g/e"]));
    // Stepping down from just above lands after it.
    assert!(m.reorder_set("g/d", 2));
    idle(&mut m);
    assert_eq!(ids(&m), s(&["g/a", "g/b", "g/c", "g/d", "g/e"]));
}

/// A move that changes nothing on screen is no move: no undo step, no re-plan, the saved order untouched.
#[test]
fn a_move_that_changes_nothing_on_screen_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/b", "g/c"]);
    let (steps, doc) = (m.history.undo.len(), text(&m));
    // The block's last member down one: the pull puts it straight back.
    assert!(!m.reorder_set("g/c", 3));
    // The block's first member to where it is.
    assert!(!m.reorder_set("g/b", 1));
    // A plain set to where it is.
    assert!(!m.reorder_set("g/d", 3));
    assert!(!m.busy(), "no re-plan");
    assert_eq!((m.history.undo.len(), text(&m)), (steps, doc));
    assert_eq!(ids(&m), s(&["g/a", "g/b", "g/c", "g/d", "g/e"]));
}

#[test]
fn reordering_never_changes_membership() {
    let d = tempfile::tempdir().unwrap();
    let mut m = five(d.path());
    new_group(&mut m, "Armor", &["g/b", "g/c"]);
    new_group(&mut m, "Weapons", &["g/d", "g/e"]);
    let before = stored(&m);
    for (id, to) in [("g/a", 2), ("g/c", 0), ("g/b", 4), ("g/d", 1), ("g/e", 3)] {
        m.reorder_set(id, to);
        idle(&mut m);
        assert_eq!(stored(&m), before, "{id} to {to}");
    }
}
