mod support;
use sdc_gui::model::sets::*;
use serde_json::json;
use std::time::Duration;
use support::*;

fn idle(m: &mut sdc_gui::model::AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)));
}

fn view<'a>(v: &'a [SetView], id: &str) -> &'a SetView {
    v.iter().find(|s| s.id == id).unwrap_or_else(|| {
        panic!(
            "no set {id} in {:?}",
            v.iter().map(|s| &s.id).collect::<Vec<_>>()
        )
    })
}

fn key_of(m: &sdc_gui::model::AppModel, name: &str) -> String {
    m.items
        .iter()
        .find(|i| i.name == name)
        .unwrap_or_else(|| panic!("no item {name}"))
        .key
        .clone()
}

#[test]
fn the_fixture_set_has_its_slots_a_derived_match_and_pbr_output_paths() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m); // the second (output-path) plan
    let v = set_views(&m);
    let rock = view(&v, "landscape/rock01");
    assert!(
        rock.pbr,
        "rock01 packs _rmaos, so its diffuse resolves pbr_albedo"
    );
    assert!(matches!(
        rock.slots.get(&SlotKind::Diffuse),
        Some(SlotFill::Own { .. })
    ));
    assert!(matches!(
        rock.slots.get(&SlotKind::Normal),
        Some(SlotFill::Own { .. })
    ));
    assert!(matches!(
        rock.slots.get(&SlotKind::Rmaos),
        Some(SlotFill::Own { .. })
    ));
    assert_eq!(rock.derived_match.as_deref(), Some(r"landscape\rock01"));
    assert_eq!(
        rock.prefix.as_deref(),
        Some(r"textures\pbr\landscape\rock01")
    );
    let (_, paths) = engine_inputs(&m);
    use engine::sets::OutputKey;
    assert_eq!(
        paths.get(&OutputKey::File(key_of(&m, "rock01.png"))),
        Some(&std::path::PathBuf::from(
            "textures/pbr/landscape/rock01.dds"
        ))
    );
    assert_eq!(
        paths.get(&OutputKey::Packed {
            set: "landscape/rock01".into(),
            suffix: "rmaos".into()
        }),
        Some(&std::path::PathBuf::from(
            "textures/pbr/landscape/rock01_rmaos.dds"
        ))
    );
    // The plan the model holds now writes there.
    let diffuse = m.items.iter().find(|i| i.name == "rock01.png").unwrap();
    assert!(diffuse
        .job
        .as_ref()
        .unwrap()
        .output
        .to_string_lossy()
        .replace('\\', "/")
        .ends_with("textures/pbr/landscape/rock01.dds"));
}

#[test]
fn vanilla_gets_no_output_paths() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    assert!(engine_inputs(&m).1.is_empty());
    // The plan went back to the mirrored path.
    let diffuse = m.items.iter().find(|i| i.name == "rock01.png").unwrap();
    let out = diffuse
        .job
        .as_ref()
        .unwrap()
        .output
        .to_string_lossy()
        .replace('\\', "/");
    assert!(
        out.ends_with("export/textures/landscape/rock01.dds"),
        "{out}"
    );
}

#[test]
fn dropping_a_diffuse_on_a_set_with_one_asks_to_replace() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let leaf = key_of(&m, "leaf.png");
    let out = m.drop_on_set(&leaf, "landscape/rock01");
    assert_eq!(
        out,
        DropOutcome::AskReplace {
            existing: key_of(&m, "rock01.png")
        }
    );
    m.drop_replace(&leaf, "landscape/rock01", DropChoice::Move)
        .unwrap();
    idle(&mut m);
    let v = set_views(&m);
    let rock = view(&v, "landscape/rock01");
    assert!(
        matches!(rock.slots.get(&SlotKind::Diffuse), Some(SlotFill::Own { item }) if *item == leaf)
    );
    // The replaced diffuse became a set of its own; one undo puts both back.
    m.undo();
    idle(&mut m);
    let v = set_views(&m);
    assert!(
        matches!(view(&v, "landscape/rock01").slots.get(&SlotKind::Diffuse), Some(SlotFill::Own { item }) if *item == key_of(&m, "rock01.png"))
    );
}

#[test]
fn new_set_share_and_reorder() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    assert!(set_views(&m).iter().any(|s| s.id == id && s.created));
    let rmaos = m
        .items
        .iter()
        .find(|i| i.name.ends_with("rmaos"))
        .unwrap()
        .key
        .clone();
    assert_eq!(
        m.drop_on_set(&rmaos, &id),
        DropOutcome::AskChoice {
            from: "landscape/rock01".into()
        }
    );
    assert_eq!(
        m.drop_choice(&rmaos, &id, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    let v = set_views(&m);
    assert!(
        matches!(view(&v, &id).slots.get(&SlotKind::Rmaos), Some(SlotFill::Borrowed { from, .. }) if from == "landscape/rock01")
    );
    // rock01 is the first PBR set using the shared output: it owns it.
    assert_eq!(view(&v, "landscape/rock01").owns, vec![SlotKind::Rmaos]);
    // Reorder puts the new set first.
    m.reorder_set(&id, 0);
    assert_eq!(set_views(&m)[0].id, id);
    assert_eq!(m.sets_doc.as_ref().unwrap().order()[0], id);
}

#[test]
fn a_packed_output_cannot_be_moved_only_shared() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    let rmaos = key_of(&m, "rock01_rmaos");
    assert!(matches!(
        m.drop_choice(&rmaos, &id, DropChoice::Move),
        DropOutcome::Refused(_)
    ));
    assert!(m.remove_member("landscape/rock01", &rmaos).is_err());
}

#[test]
fn duplicating_a_raw_source_copies_it_and_undo_deletes_it_only_when_unchanged() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap(); // e.g. "landscape/odd-2"
    idle(&mut m);
    let rough = key_of(&m, "rock01_roughness.png");
    assert_eq!(
        m.drop_on_set(&rough, &id),
        DropOutcome::AskChoice {
            from: "landscape/rock01".into()
        }
    );
    // Note HH5: a raw helper is shared as the file (a `share` line); undone here, the test is about Duplicate.
    assert_eq!(
        m.drop_choice(&rough, &id, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().text().contains("share"));
    m.undo();
    idle(&mut m);
    assert!(!m.sets_doc.as_ref().unwrap().text().contains("share"));
    assert_eq!(
        m.drop_choice(&rough, &id, DropChoice::Duplicate),
        DropOutcome::Joined
    );
    let base = id.rsplit('/').next().unwrap().to_string();
    let copy = m
        .root
        .clone()
        .unwrap()
        .join(format!("landscape/{base}_roughness.png"));
    assert!(copy.exists(), "{copy:?}");
    m.undo();
    assert!(!copy.exists(), "an unchanged copy is deleted by undo");
    m.redo();
    assert!(copy.exists(), "redo copies it again");
    std::fs::write(&copy, b"changed").unwrap();
    m.undo();
    assert!(copy.exists(), "a changed copy is kept");
    assert!(matches!(m.dialog, Some(sdc_gui::model::Dialog::Error(_))));
}

#[test]
fn coat_and_fuzz_cannot_share_a_set() {
    assert!(slot_conflict(&[SlotKind::Fuzz], SlotKind::Coat).is_some());
    assert!(slot_conflict(&[SlotKind::Coat], SlotKind::Fuzz).is_some());
    assert!(slot_conflict(&[SlotKind::Emissive], SlotKind::SkinSk).is_some());
    assert!(slot_conflict(&[SlotKind::SkinSk], SlotKind::Emissive).is_some());
    assert!(slot_conflict(&[SlotKind::Diffuse, SlotKind::Normal], SlotKind::Coat).is_none());
}

#[test]
fn emissive_and_skin_sk_cannot_share_a_set() {
    assert!(slot_conflict(&[SlotKind::Emissive], SlotKind::SkinSk).is_some());
    assert!(slot_conflict(&[SlotKind::SkinSk], SlotKind::Emissive).is_some());
    assert!(slot_conflict(&[SlotKind::Diffuse, SlotKind::Normal], SlotKind::SkinSk).is_none());
}

#[test]
fn slot_numbers_follow_p6() {
    let n: Vec<(SlotKind, Option<u8>, &str)> = SlotKind::ALL
        .iter()
        .map(|k| (*k, k.slot_number(), k.suffix()))
        .collect();
    assert_eq!(
        n,
        vec![
            (SlotKind::Diffuse, Some(1), ""),
            (SlotKind::Normal, Some(2), "n"),
            (SlotKind::Rmaos, Some(6), "rmaos"),
            (SlotKind::Height, Some(4), "p"),
            (SlotKind::Emissive, Some(3), "g"),
            (SlotKind::SkinSk, Some(3), "sk"),
            (SlotKind::Subsurface, Some(8), "s"),
            (SlotKind::Coat, Some(7), "cnr"),
            (SlotKind::Fuzz, Some(7), "f"),
            // Note GI: the XPMF maps have no PGPatcher slot.
            (SlotKind::ProjectedNoise, None, "noise"),
            (SlotKind::DetailNormal, None, "detail"),
            // Task 4b: numbers past the game's 8 (`game_slot` is None for both).
            (SlotKind::SkinWet, Some(9), "wet"),
            (SlotKind::SkinDetail, Some(10), "detail_n"),
        ]
    );
}

#[test]
fn a_member_that_vanished_shows_as_missing_and_is_reported() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    // On disk but no texture, so no plan lists it; a deleted file's line clears itself (Note HE2).
    std::fs::write(m.root.clone().unwrap().join("landscape/notes.txt"), "x").unwrap();
    m.edit_docs("t", None, true, |_, s| {
        s.add_member("landscape/rock01", "landscape/notes.txt")
    });
    idle(&mut m);
    let v = set_views(&m);
    assert_eq!(
        view(&v, "landscape/rock01").missing,
        vec![Missing::File("landscape/notes.txt".to_string())]
    );
    assert!(
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .any(|e| e.add.contains(&"landscape/notes.txt".to_string())),
        "never dropped"
    );
}

#[test]
fn edits_go_creates_and_renames_first_then_moves_then_removes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.edit_docs("t", None, false, |_, s| {
        s.add_member("mine/new", "landscape/leaf.png");
        s.create_set("mine/new");
        s.remove_member("landscape/rock01", "landscape/rock01_normal.png");
    });
    let (edits, _) = engine_inputs(&m);
    use engine::sets::SetEdit;
    assert!(matches!(&edits[0], SetEdit::Create { set } if set == "mine/new"));
    assert!(matches!(&edits[1], SetEdit::Move { to, .. } if to == "mine/new"));
    assert!(matches!(edits.last().unwrap(), SetEdit::NoSet { .. }));
}

#[test]
fn json_only_edits_do_not_replan_but_match_edits_do() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.set_json_value("landscape/rock01", "specular_level", Some(json!(0.05)));
    assert!(!m.busy(), "a material value does not plan");
    m.set_json_value(
        "landscape/rock01",
        "match1",
        Some(json!(r"landscape\boulder")),
    );
    assert!(m.busy(), "a match changes output paths, so it plans");
    idle(&mut m);
    assert_eq!(
        view(&set_views(&m), "landscape/rock01").prefix.as_deref(),
        Some(r"textures\pbr\landscape\boulder")
    );
}

#[test]
fn clearing_a_match_returns_to_the_derived_prefix() {
    // The derived match comes from the source, not from the (redirected) output, so it never follows a match.
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.set_json_value(
        "landscape/rock01",
        "match1",
        Some(json!(r"landscape\boulder")),
    );
    idle(&mut m);
    let diffuse = m.items.iter().find(|i| i.name == "rock01.png").unwrap();
    assert!(diffuse
        .job
        .as_ref()
        .unwrap()
        .output
        .to_string_lossy()
        .replace('\\', "/")
        .ends_with("textures/pbr/landscape/boulder.dds"));
    assert_eq!(
        view(&set_views(&m), "landscape/rock01")
            .derived_match
            .as_deref(),
        Some(r"landscape\rock01")
    );
    m.set_json_value("landscape/rock01", "match1", None);
    idle(&mut m);
    assert_eq!(
        view(&set_views(&m), "landscape/rock01").prefix.as_deref(),
        Some(r"textures\pbr\landscape\rock01")
    );
}

#[test]
fn removing_the_only_member_of_a_set_puts_it_in_no_set() {
    // Note HF1 supersedes the old refusal (DU5): it leaves for no set, with no `remove` line.
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let leaf = key_of(&m, "leaf.png");
    let steps = m.history.undo.len();
    assert!(m.remove_member("landscape/leaf", &leaf).is_ok());
    assert_eq!(m.history.undo.len(), steps + 1);
    assert!(m.is_no_set(&leaf));
    // The only member of a set it was moved into leaves for no set too (its `add` line goes).
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    assert!(m.remove_member(&id, &odd).is_ok());
    idle(&mut m);
    let v = set_views(&m);
    assert!(m.is_no_set(&odd));
    assert!(v.iter().all(|s| !s.members.contains(&odd)));
    assert!(
        view(&v, &id).members.is_empty(),
        "the created set stays, empty"
    );
    assert!(engine_inputs(&m)
        .0
        .iter()
        .all(|e| !matches!(e, engine::sets::SetEdit::Remove { .. })));
    // A member of a bigger set leaves it.
    let normal = key_of(&m, "rock01_normal.png");
    assert!(m.remove_member("landscape/rock01", &normal).is_ok());
    idle(&mut m);
    assert!(!view(&set_views(&m), "landscape/rock01")
        .members
        .contains(&normal));
}

#[test]
fn rename_refuses_names_in_use_and_bad_names() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    assert!(m.rename_set("landscape/rock01", "landscape/leaf").is_err());
    assert!(m.rename_set("landscape/rock01", "LANDSCAPE/Leaf").is_err());
    assert!(m.rename_set("landscape/rock01", "../up").is_err());
    assert!(m.rename_set("landscape/rock01", "a|b").is_err());
    assert!(m.rename_set("landscape/rock01", "").is_err());
    assert!(m
        .rename_set("landscape/rock01", "landscape/rock01")
        .is_err());
    assert!(m.rename_set("landscape/rock01", "stone/boulder").is_ok());
    idle(&mut m);
    let v = set_views(&m);
    assert!(view(&v, "stone/boulder").pbr);
    assert!(!v.iter().any(|s| s.id == "landscape/rock01"));
    // The old name is still the engine's until the rename applies, so another set may not take it.
    assert!(m.rename_set("landscape/leaf", "landscape/rock01").is_err());
    // Renaming back is a plain rename that leaves no stale edit.
    assert!(m.rename_set("stone/boulder", "landscape/rock01").is_ok());
    idle(&mut m);
    assert!(view(&set_views(&m), "landscape/rock01").pbr);
    assert!(engine_inputs(&m)
        .0
        .iter()
        .all(|e| !matches!(e, engine::sets::SetEdit::Rename { .. })));
}

/// Note HS11: the engine still makes a deleted automatic set (it renames before it moves files out), so its name
/// stays refused to a rename.
#[test]
fn rename_refuses_the_name_of_a_deleted_automatic_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    idle(&mut m);
    assert!(m.delete_set("landscape/leaf"));
    idle(&mut m);
    assert!(!set_views(&m).iter().any(|s| s.id == "landscape/leaf"));
    assert!(m.rename_set("landscape/rock01", "landscape/leaf").is_err());
    idle(&mut m);
    let entries = m.sets_doc.as_ref().unwrap().entries();
    let rock = entries
        .iter()
        .find(|e| e.id == "landscape/rock01")
        .expect("rock01's entry");
    assert_eq!(rock.shader, Some(engine::shader::Shader::Pbr));
    assert!(entries.iter().all(|e| e.renamed_from.is_none()));
    assert!(!set_views(&m).iter().any(|s| s.id == "landscape/leaf"));
    assert!(engine_inputs(&m)
        .0
        .iter()
        .all(|e| !matches!(e, engine::sets::SetEdit::Rename { .. })));
}

#[test]
fn rename_refuses_the_name_of_an_automatic_set_emptied_by_a_move() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let leaf = key_of(&m, "leaf.png");
    m.drop_replace(&leaf, "landscape/rock01", DropChoice::Move)
        .unwrap();
    idle(&mut m);
    let v = set_views(&m);
    assert!(!v.iter().any(|s| s.id == "landscape/leaf"), "no leaf card");
    let odd = key_of(&m, "odd.png");
    let odd_set = v
        .iter()
        .find(|s| s.members.contains(&odd))
        .expect("odd's set")
        .id
        .clone();
    let e = m.rename_set(&odd_set, "landscape/leaf").unwrap_err();
    assert!(e.contains("automatic set of a file"), "{e}");
}

/// Note HS11 (final review Issue 3): a file's automatic set follows its input name, so the name it was read under
/// before is free.
#[test]
fn rename_may_take_an_automatic_name_an_input_name_left() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.set_input_name("landscape/rock01", "textures/landscape/stone")
        .unwrap();
    idle(&mut m);
    assert!(set_views(&m).iter().any(|s| s.id == "landscape/stone"));
    assert_eq!(m.rename_set("landscape/leaf", "landscape/rock01"), Ok(()));
}

/// A set the user created makes no automatic set, so it may take a hidden automatic name.
#[test]
fn a_created_set_may_take_a_hidden_automatic_name() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    idle(&mut m);
    assert!(m.delete_set("landscape/leaf"));
    idle(&mut m);
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    assert_eq!(m.rename_set(&id, "landscape/leaf"), Ok(()));
}

#[test]
fn delete_set_deletes_a_created_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    assert!(m.delete_set(&id));
    idle(&mut m);
    assert!(!set_views(&m).iter().any(|s| s.id == id));
}

#[test]
fn slot_paths_are_checked() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let set = "landscape/rock01";
    assert!(m
        .set_slot_command(set, 6, SlotCommand::Path("../x.dds".into()))
        .is_err());
    assert!(m
        .set_slot_command(set, 6, SlotCommand::Path("textures/x.png".into()))
        .is_err());
    assert!(m
        .set_slot_command(set, 6, SlotCommand::Path("textures/x.dds".into()))
        .is_ok());
    let e = |m: &sdc_gui::model::AppModel| {
        m.sets_doc
            .as_ref()
            .unwrap()
            .entries()
            .iter()
            .find(|&e| e.id == set)
            .cloned()
            .unwrap()
            .settings
    };
    assert_eq!(e(&m).get("slot6"), Some(&json!(r"textures\x.dds")));
    m.set_slot_command(set, 6, SlotCommand::Clear).unwrap();
    assert_eq!(e(&m).get("slot6"), Some(&json!("")));
    m.set_slot_command(set, 6, SlotCommand::FromSet).unwrap();
    assert_eq!(e(&m).get("slot6"), None);
}

#[test]
fn a_read_only_project_file_refuses_set_edits_but_sdc_toml_still_edits() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let root = m.root.clone().unwrap();
    std::fs::write(root.join("sdc-project.toml"), "[[sets]\nbroken").unwrap();
    m.reload_documents();
    idle(&mut m);
    assert!(m.sets_doc.as_ref().unwrap().read_only());
    let odd = key_of(&m, "odd.png");
    let rough = key_of(&m, "rock01_roughness.png");
    let leaf = key_of(&m, "leaf.png");
    assert!(matches!(
        m.drop_on_set(&leaf, "landscape/rock01"),
        DropOutcome::Refused(_)
    ));
    assert!(matches!(
        m.drop_choice(&rough, "landscape/odd", DropChoice::Duplicate),
        DropOutcome::Refused(_)
    ));
    assert!(!root.join("landscape/odd_roughness.png").exists());
    assert!(m.drop_new_set(&odd).is_err());
    assert!(m.rename_set("landscape/rock01", "x/y").is_err());
    assert!(m.remove_member("landscape/rock01", &rough).is_err());
    assert!(m
        .drop_replace(&leaf, "landscape/rock01", DropChoice::Move)
        .is_err());
    assert!(!m.delete_set("landscape/rock01"));
    assert!(m
        .set_slot_command("landscape/rock01", 6, SlotCommand::Clear)
        .is_err());
    let order = m.sets_doc.as_ref().unwrap().order();
    m.reorder_set("landscape/rock01", 3);
    assert_eq!(m.sets_doc.as_ref().unwrap().order(), order);
    m.set_json_value("landscape/rock01", "specular_level", Some(json!(0.1)));
    assert!(!m.can_undo(), "no set edit was recorded");
    m.set_target(engine::config::Target::Vanilla);
    assert!(m.can_undo(), "sdc.toml edits still work");
}

/// rock01 (PBR) plus a created set `landscape/leaf-2` holding leaf.png in its diffuse slot.
fn with_leaf_set(d: &std::path::Path) -> (sdc_gui::model::AppModel, String) {
    let mut m = pbr_opened(d);
    idle(&mut m);
    let leaf = key_of(&m, "leaf.png");
    let id = m.drop_new_set(&leaf).unwrap();
    idle(&mut m);
    assert!(matches!(
        view(&set_views(&m), &id).slots.get(&SlotKind::Diffuse),
        Some(SlotFill::Own { item }) if *item == leaf
    ));
    (m, id)
}

#[test]
fn sharing_into_a_filled_slot_asks_to_replace() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = with_leaf_set(d.path());
    let leaf = key_of(&m, "leaf.png");
    let rock = key_of(&m, "rock01.png");
    let before = m.sets_doc.as_ref().unwrap().text();
    assert_eq!(
        m.drop_choice(&rock, &id, DropChoice::Share),
        DropOutcome::AskReplace {
            existing: leaf.clone()
        }
    );
    // Cancel: nothing changed.
    assert_eq!(m.sets_doc.as_ref().unwrap().text(), before);
    // Replace: leaf leaves, the slot borrows rock01's diffuse, in one step.
    m.drop_replace(&rock, &id, DropChoice::Share).unwrap();
    idle(&mut m);
    let v = set_views(&m);
    assert!(matches!(
        view(&v, &id).slots.get(&SlotKind::Diffuse),
        Some(SlotFill::Borrowed { from, item }) if from == "landscape/rock01" && *item == rock
    ));
    assert!(!view(&v, &id).members.contains(&leaf));
    m.undo();
    idle(&mut m);
    assert_eq!(m.sets_doc.as_ref().unwrap().text(), before);
}

#[test]
fn sharing_away_the_only_texture_of_an_automatic_set_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let rock = key_of(&m, "rock01.png");
    assert!(matches!(
        m.drop_choice(&rock, "landscape/leaf", DropChoice::Share),
        DropOutcome::AskReplace { .. }
    ));
    assert!(m
        .drop_replace(&rock, "landscape/leaf", DropChoice::Share)
        .is_err());
}

#[test]
fn duplicating_a_finished_map_into_a_filled_slot_asks_to_replace() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = with_leaf_set(d.path());
    let leaf = key_of(&m, "leaf.png");
    let rock = key_of(&m, "rock01.png");
    let base = id.rsplit('/').next().unwrap().to_string();
    let copy = m
        .root
        .clone()
        .unwrap()
        .join(format!("landscape/{base}.png"));
    assert_eq!(
        m.drop_choice(&rock, &id, DropChoice::Duplicate),
        DropOutcome::AskReplace {
            existing: leaf.clone()
        }
    );
    assert!(!copy.exists(), "cancel copies nothing");
    m.drop_replace(&rock, &id, DropChoice::Duplicate).unwrap();
    assert!(copy.exists(), "{copy:?}");
    idle(&mut m);
    let members = view(&set_views(&m), &id).members.clone();
    assert!(
        members.contains(&format!("landscape/{base}.png")),
        "{members:?}"
    );
    assert!(!members.contains(&leaf));
    m.undo();
    assert!(
        !copy.exists(),
        "one undo removes the copy and puts leaf back"
    );
    idle(&mut m);
    assert!(view(&set_views(&m), &id).members.contains(&leaf));
}

#[test]
fn an_entry_whose_files_are_all_gone_shows_as_missing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.set_json_value("landscape/rock01", "specular_level", Some(json!(0.05)));
    let root = m.root.clone().unwrap();
    for f in [
        "rock01.png",
        "rock01_normal.png",
        "rock01_roughness.png",
        "rock01_ao.png",
    ] {
        std::fs::remove_file(root.join("landscape").join(f)).unwrap();
    }
    m.reload_changed();
    idle(&mut m);
    let v = set_views(&m);
    let rock = view(&v, "landscape/rock01");
    assert!(rock.key.is_empty() && rock.members.is_empty());
    assert!(!rock.missing.is_empty(), "shown as missing");
    let leaf = key_of(&m, "leaf.png");
    assert!(matches!(
        m.drop_on_set(&leaf, "landscape/rock01"),
        DropOutcome::Refused(_)
    ));
}

#[test]
fn a_borrow_from_a_set_without_that_slot_is_reported() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 6, Some("landscape/odd"))
    });
    idle(&mut m);
    let v = set_views(&m);
    let leaf = view(&v, "landscape/leaf");
    assert!(
        leaf.missing
            .iter()
            .any(|x| x.text().contains("slot6") && x.text().contains("landscape/odd")),
        "{:?}",
        leaf.missing
    );
    assert!(!leaf.slots.contains_key(&SlotKind::Rmaos));
}

#[test]
fn export_refuses_a_plan_that_does_not_match_the_sets() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    // A set change that was not planned (the state the bounded re-planning leaves when it gives up).
    m.edit_docs("t", None, false, |_, s| s.create_set("x/y"));
    m.export_clicked();
    assert!(matches!(m.dialog, Some(sdc_gui::model::Dialog::Error(_))));
    assert!(m.export.running.is_none(), "nothing was exported");
}

#[test]
fn an_entry_left_with_only_its_id_is_not_an_edit() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.rename_set("landscape/leaf", "landscape/leafy").unwrap();
    idle(&mut m);
    m.rename_set("landscape/leafy", "landscape/leaf").unwrap();
    idle(&mut m);
    let leaf = key_of(&m, "leaf.png");
    assert!(!matches!(
        m.drop_on_set(&leaf, "landscape/rock01"),
        DropOutcome::AskChoice { .. }
    ));
}

#[test]
fn the_entry_of_a_set_whose_files_are_gone_can_be_deleted() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.set_json_value("landscape/leaf", "specular_level", Some(json!(0.05)));
    std::fs::remove_file(m.root.clone().unwrap().join("landscape/leaf.png")).unwrap();
    m.reload_changed();
    idle(&mut m);
    assert!(view(&set_views(&m), "landscape/leaf").key.is_empty());
    assert!(m.delete_set("landscape/leaf"));
    idle(&mut m);
    assert!(!set_views(&m).iter().any(|s| s.id == "landscape/leaf"));
}

/// Final review I1: the packed output's `[files]` key follows its first source's name, as the engine scopes it, so a
/// format chosen on the RMAOS card still applies after a match edit moves the output under another prefix.
#[test]
fn a_packed_format_applies_after_a_match_edit() {
    use engine::config::{ColorTag, Format};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let packed = |m: &sdc_gui::model::AppModel| {
        m.items
            .iter()
            .find(|i| i.name.ends_with("rmaos"))
            .unwrap()
            .clone()
    };
    let before = packed(&m).key;
    m.set_json_value(
        "landscape/rock01",
        "match1",
        Some(json!(r"landscape\stone")),
    );
    idle(&mut m);
    let it = packed(&m);
    assert!(
        it.job
            .as_ref()
            .unwrap()
            .output
            .to_string_lossy()
            .contains("stone_rmaos"),
        "the match edit moved the output: {:?}",
        it.job.as_ref().map(|j| &j.output)
    );
    assert_eq!(
        it.key, before,
        "the item key does not follow the output path"
    );
    let want = if it.job.as_ref().unwrap().profile.format == Format::Bc1 {
        Format::Bc7
    } else {
        Format::Bc1
    };
    m.set_format(&it.key, want, ColorTag::Unorm);
    idle(&mut m);
    assert_eq!(packed(&m).job.as_ref().unwrap().profile.format, want);
    assert!(m.unmatched_keys.is_empty(), "{:?}", m.unmatched_keys);
}

/// Final review I1: a packed `[files]` entry made before a match edit still names the packed output afterwards (the
/// engine's unmatched-key check uses the same first-source stem), so it is not offered for removal.
#[test]
fn a_packed_entry_stays_matched_after_a_match_edit() {
    use engine::config::{ColorTag, Format};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let key = key_of(&m, "rock01_rmaos");
    m.set_format(&key, Format::Bc1, ColorTag::Unorm);
    m.set_json_value(
        "landscape/rock01",
        "match1",
        Some(json!(r"landscape\stone")),
    );
    idle(&mut m);
    let it = m.items.iter().find(|i| i.key == key).unwrap();
    assert_eq!(it.job.as_ref().unwrap().profile.format, Format::Bc1);
    assert!(m.unmatched_keys.is_empty(), "{:?}", m.unmatched_keys);
}

/// Final review I3: set ids fold as the engine folds them (Unicode lower case), so a rename to a name with a
/// non-ASCII capital stays one set and keeps its settings.
#[test]
fn a_rename_with_a_non_ascii_capital_keeps_one_set_and_its_settings() {
    use sdc_gui::model::pbr_export::json_entries;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.set_json_value("landscape/rock01", "specular_level", Some(json!(0.2)));
    m.rename_set("landscape/rock01", "landscape/Öl").unwrap();
    idle(&mut m);
    let views = set_views(&m);
    let same: Vec<_> = views
        .iter()
        .filter(|v| v.id.to_lowercase() == "landscape/öl")
        .collect();
    assert_eq!(same.len(), 1, "{views:#?}");
    assert!(views.iter().all(|v| v.missing.is_empty()), "{views:#?}");
    assert!(same[0].pbr && !same[0].key.is_empty());
    assert_eq!(same[0].settings.get("specular_level"), Some(&json!(0.2)));
    let entries = json_entries(&m);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].1.settings.get("specular_level"),
        Some(&json!(0.2))
    );
}

/// Final review I3: a set created in a folder whose name has a non-ASCII capital is one set (created, no phantom).
#[test]
fn a_created_set_in_a_non_ascii_folder_is_one_set() {
    use image::{ImageBuffer, Rgb};
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    let f = root.join("Äpfel");
    std::fs::create_dir_all(&f).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(f.join("stein.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(f.join("stein_n.png"))
        .unwrap();
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(d.path().join("export")),
        ..Default::default()
    };
    let mut m = sdc_gui::model::AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    idle(&mut m);
    let normal = key_of(&m, "stein_n.png");
    assert!(normal.starts_with("Äpfel/"), "{normal}");
    let id = m.drop_new_set(&normal).unwrap();
    idle(&mut m);
    let views = set_views(&m);
    let folded = id.to_lowercase();
    let same: Vec<_> = views
        .iter()
        .filter(|v| v.id.to_lowercase() == folded)
        .collect();
    assert_eq!(same.len(), 1, "{id}: {views:#?}");
    assert!(same[0].created && !same[0].key.is_empty(), "{:#?}", same[0]);
    assert!(same[0].members.contains(&normal));
    assert!(views.iter().all(|v| v.missing.is_empty()), "{views:#?}");
}

#[test]
fn a_shared_rmaos_is_sent_to_the_engine_for_owner_and_borrower() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    assert!(shared_roughness(&m).is_empty());
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    let rmaos = m
        .items
        .iter()
        .find(|i| i.name.ends_with("rmaos"))
        .unwrap()
        .key
        .clone();
    assert_eq!(
        m.drop_on_set(&rmaos, &id),
        DropOutcome::AskChoice {
            from: "landscape/rock01".into()
        }
    );
    assert_eq!(
        m.drop_choice(&rmaos, &id, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    let shared = shared_roughness(&m);
    assert!(shared.contains("landscape/rock01"), "{shared:?}");
    assert!(shared.contains(&id), "{shared:?}");
    // Plan (`replan`) and export (`start_export`) both build their `BatchRequest` through `request()`.
    assert_eq!(m.request().unwrap().shared_roughness, shared);
}

/// Final review I1 (DECISIONS Note R M4-final-1): `rock01_normal` shared into a set with its own `rock02_rmaos`. The
/// borrower bakes its roughness from the borrowed `_n` and reports no missing companion.
#[test]
fn a_shared_normal_bakes_the_borrowers_own_rmaos() {
    use engine::codes::Code;
    use engine::config::Subtype;
    use image::{ImageBuffer, Rgb, Rgba};
    use sdc_gui::model::Answer;
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 60u8]))
        .save(r.join("rock02.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, _| Rgba([(x * 4) as u8, 0, 200, 255]))
        .save(r.join("rock02_rmaos.png"))
        .unwrap();
    let mut m = pbr_opened(d.path());
    m.decide("landscape/rock02.png", Answer::Subtype(Subtype::PbrAlbedo));
    idle(&mut m);
    let rmaos02 = key_of(&m, "rock02_rmaos.png");
    let member = |m: &sdc_gui::model::AppModel| {
        m.item(&rmaos02)
            .and_then(|i| i.job.clone())
            .expect("rock02_rmaos has a job")
    };
    assert_eq!(member(&m).profile.subtype, Subtype::Rmaos);
    // Before the share, rock02 has no `_n`: the honest code.
    assert!(member(&m)
        .warnings
        .iter()
        .any(|w| w.code == Code::SetMissingCompanion));
    let normal = key_of(&m, "rock01_normal.png");
    assert_eq!(
        m.drop_on_set(&normal, "landscape/rock02"),
        DropOutcome::AskChoice {
            from: "landscape/rock01".into()
        }
    );
    assert_eq!(
        m.drop_choice(&normal, "landscape/rock02", DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    let borrowed = borrowed_normal(&m);
    assert_eq!(
        borrowed.get("landscape/rock02").map(String::as_str),
        Some("landscape/rock01"),
        "{borrowed:?}"
    );
    assert!(shared_roughness(&m).is_empty());
    assert_eq!(m.request().unwrap().borrowed_normal, borrowed);
    let j = member(&m);
    let b = j.bake.as_ref().expect("the borrower's own rmaos bakes");
    assert!(b.normal.ends_with("rock01_normal.png"), "{:?}", b.normal);
    assert!(
        !j.warnings
            .iter()
            .any(|w| w.code == Code::SetMissingCompanion),
        "{:?}",
        j.warnings
    );
}

/// `pbr_opened` with a second PBR set, `landscape/rock02`: its albedo, with its own `rock02_rmaos.png` when `rmaos`,
/// and its own `rock02_n.png` (a PBR normal, DirectX green) when `normal`.
fn rock02_opened(dir: &std::path::Path, rmaos: bool, normal: bool) -> sdc_gui::model::AppModel {
    use engine::config::{GreenConvention, Subtype};
    use image::{ImageBuffer, Rgb, Rgba};
    use sdc_gui::model::Answer;
    let r = dir.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 60u8]))
        .save(r.join("rock02.png"))
        .unwrap();
    if rmaos {
        ImageBuffer::from_fn(64, 64, |x, _| Rgba([(x * 4) as u8, 0, 200, 255]))
            .save(r.join("rock02_rmaos.png"))
            .unwrap();
    }
    if normal {
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
            .save(r.join("rock02_n.png"))
            .unwrap();
    }
    let mut m = pbr_opened(dir);
    m.decide("landscape/rock02.png", Answer::Subtype(Subtype::PbrAlbedo));
    if rmaos {
        // Asked for once the set has its own normal.
        m.decide(
            "landscape/rock02_rmaos.png",
            Answer::Subtype(Subtype::Rmaos),
        );
    }
    if normal {
        m.decide(
            "landscape/rock02_n.png",
            Answer::Subtype(Subtype::PbrNormal),
        );
        m.decide(
            "landscape/rock02_n.png",
            Answer::Green(GreenConvention::Directx),
        );
    }
    idle(&mut m);
    m
}

/// The Problems rows of set `id` that say the shader needs `map`.
fn needs_rows(m: &sdc_gui::model::AppModel, id: &str, map: &str) -> usize {
    use sdc_gui::model::bottom::At;
    sdc_gui::model::bottom::problems(m)
        .iter()
        .filter(|p| {
            p.code == Some(engine::codes::Code::ShaderMissingMap)
                && p.text.contains(&format!("needs {map};"))
                && matches!(&p.at, At::Set { id: i, .. } if i == id)
        })
        .count()
}

/// The set's own part of the card's "⚠ n" (`sets_column`'s `rows`: its `set_warnings` that count toward the badge).
fn set_badge(m: &sdc_gui::model::AppModel, id: &str) -> usize {
    view(&set_views(m), id)
        .set_warnings
        .iter()
        .filter(|w| sdc_gui::model::counts_toward_badge(w))
        .count()
}

/// The members' part of the card's "⚠ n" (`sets_column`'s `rows`: the member items' warnings that count). A shared
/// normal leaves it alone: the `SET-MISSING-COMPANION` it takes off the member `_rmaos` is Info and never counts.
fn member_badge(m: &sdc_gui::model::AppModel, id: &str) -> usize {
    view(&set_views(m), id)
        .members
        .iter()
        .filter_map(|k| m.item(k))
        .flat_map(|i| i.warnings.iter())
        .filter(|w| sdc_gui::model::counts_toward_badge(w))
        .count()
}

/// Whether the card of `id` is Ready, with the set's Problems rows for the failure message.
fn ready(m: &sdc_gui::model::AppModel, id: &str) -> (bool, Vec<String>) {
    use sdc_gui::model::bottom::{filtered, set_ready, ProblemsFilter};
    let views = set_views(m);
    let v = view(&views, id);
    let rows = filtered(m, Some(&ProblemsFilter::Set(v.key.clone())))
        .iter()
        .map(|p| format!("{:?}: {}", p.at, p.text))
        .collect();
    (set_ready(m, v), rows)
}

/// Note HH8: a slot a resolved borrow fills answers the shader's "needs" warning for its map, so the view, the
/// Problems row, the card's badge and readiness follow; clearing the borrow brings it back.
#[test]
fn a_borrowed_normal_answers_the_missing_map_warning() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), true, false);
    let id = "landscape/rock02";
    assert_eq!(missing_maps(&m, id), ["_n"], "before the borrow");
    assert_eq!(
        needs_rows(&m, id, "_n"),
        1,
        "a Problems row before the borrow"
    );
    assert!(!ready(&m, id).0, "not Ready before the borrow");
    let badge_before = set_badge(&m, id);
    let members_before = member_badge(&m, id);
    let normal = key_of(&m, "rock01_normal.png");
    assert_eq!(
        m.drop_choice(&normal, id, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    assert!(matches!(
        view(&set_views(&m), id).slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { .. })
    ));
    assert_eq!(
        missing_maps(&m, id),
        Vec::<String>::new(),
        "after the borrow"
    );
    assert_eq!(
        needs_rows(&m, id, "_n"),
        0,
        "no Problems row after the borrow"
    );
    assert_eq!(
        set_badge(&m, id),
        badge_before - 1,
        "the badge drops by one"
    );
    assert_eq!(
        member_badge(&m, id),
        members_before,
        "the members' part is unchanged, so the card's total drops by one"
    );
    let d2 = tempfile::tempdir().unwrap();
    let twin = rock02_opened(d2.path(), true, true);
    assert_eq!(missing_maps(&twin, id), Vec::<String>::new(), "the twin");
    let (twin_ready, twin_rows) = ready(&twin, id);
    assert!(
        twin_ready,
        "the twin with its own normal is Ready: {twin_rows:?}"
    );
    let (now, rows) = ready(&m, id);
    assert!(now, "as ready as with its own normal: {rows:?}");
    m.set_slot_command(id, 2, SlotCommand::Clear).unwrap();
    idle(&mut m);
    assert_eq!(missing_maps(&m, id), ["_n"], "the clear brings it back");
    assert_eq!(needs_rows(&m, id, "_n"), 1);
}

/// Note HH8: a borrow whose lender has nothing in that slot fills nothing, so the warning stays.
#[test]
fn a_borrow_from_a_set_without_that_slot_keeps_the_missing_map_warning() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), true, false);
    let id = "landscape/rock02";
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock02", 2, Some("landscape/leaf"))
    });
    idle(&mut m);
    let views = set_views(&m);
    let v = view(&views, id);
    assert!(
        v.missing.iter().any(|x| matches!(
            x,
            Missing::Borrow {
                slot: 2,
                empty: true,
                ..
            }
        )),
        "{:?}",
        v.missing
    );
    assert_eq!(missing_maps(&m, id), ["_n"]);
    assert_eq!(needs_rows(&m, id, "_n"), 1);
    // The lender set is gone.
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock02", 2, Some("landscape/nosuch"))
    });
    idle(&mut m);
    let views = set_views(&m);
    let v = view(&views, id);
    assert!(
        v.missing.iter().any(|x| matches!(
            x,
            Missing::Borrow {
                slot: 2,
                empty: false,
                ..
            }
        )),
        "{:?}",
        v.missing
    );
    assert_eq!(missing_maps(&m, id), ["_n"], "no lender set");
    assert_eq!(needs_rows(&m, id, "_n"), 1);
}

/// The rock02 set on plain `pbr` (PBR Landscape, the auto pick under `textures/landscape/`, takes any normal), with
/// rock01's normal decided as `st`, then shared into rock02's Normal slot. Returns the model after the borrow.
fn borrow_rock01_normal_as(
    dir: &std::path::Path,
    st: engine::config::Subtype,
) -> sdc_gui::model::AppModel {
    let mut m = rock02_opened(dir, true, false);
    let id = "landscape/rock02";
    m.set_shader(id, Some(engine::shader::Shader::Pbr));
    m.decide(
        "landscape/rock01_normal.png",
        sdc_gui::model::Answer::Subtype(st),
    );
    idle(&mut m);
    assert!(missing_maps(&m, id).contains(&"_n".to_string()), "before");
    let normal = key_of(&m, "rock01_normal.png");
    assert_eq!(
        m.item(&normal)
            .and_then(|i| i.job.as_ref())
            .map(|j| j.profile.subtype),
        Some(st)
    );
    assert_eq!(
        m.drop_choice(&normal, id, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    assert!(matches!(
        view(&set_views(&m), id).slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { .. })
    ));
    m
}

/// Note HH8 (B1-6): any tangent-space normal answers a shader that needs one. A plain `normal` borrowed into a `pbr`
/// set (whose role asks for `pbr_normal`) answers "needs _n": the borrower names the lender's exported file.
#[test]
fn a_borrowed_vanilla_normal_answers_the_pbr_missing_normal() {
    let d = tempfile::tempdir().unwrap();
    let m = borrow_rock01_normal_as(d.path(), engine::config::Subtype::Normal);
    let id = "landscape/rock02";
    assert!(
        !missing_maps(&m, id).contains(&"_n".to_string()),
        "a tangent-space normal answers pbr's _n"
    );
    assert_eq!(needs_rows(&m, id, "_n"), 0);
}

/// B3-3 (Note JP4): a borrowed vanilla normal raises no SHADER-NORMAL-SPECULAR on the borrower (the engine sees own
/// members only), and HH8's borrow still answers "needs _n".
#[test]
fn a_borrowed_vanilla_normal_raises_no_specular_note() {
    use sdc_gui::model::bottom::{problems, At};
    let d = tempfile::tempdir().unwrap();
    let m = borrow_rock01_normal_as(d.path(), engine::config::Subtype::Normal);
    let id = "landscape/rock02";
    // Review r1 I1: bind the views first; `view(&set_views(&m), id)` borrows a temporary (E0716).
    let views = set_views(&m);
    let v = view(&views, id);
    assert!(
        !v.set_warnings
            .iter()
            .any(|w| w.code == engine::codes::Code::ShaderNormalSpecular),
        "{:?}",
        v.set_warnings
    );
    assert!(!problems(&m).iter().any(|p| p.code
        == Some(engine::codes::Code::ShaderNormalSpecular)
        && matches!(&p.at, At::Set { id: i, .. } if i == id)));
    assert_eq!(needs_rows(&m, id, "_n"), 0);
}

/// Note HH8 (B1-6): a model-space normal is not a tangent-space one, so borrowed into a `pbr` set it keeps "needs _n".
#[test]
fn a_borrowed_model_space_normal_keeps_the_missing_normal() {
    let d = tempfile::tempdir().unwrap();
    let m = borrow_rock01_normal_as(d.path(), engine::config::Subtype::Msn);
    let id = "landscape/rock02";
    assert!(missing_maps(&m, id).contains(&"_n".to_string()));
    assert_eq!(needs_rows(&m, id, "_n"), 1);
}

/// Note HH8 (B1-5): outside the normal slot the engine's check decides. A skin `_rfaos` borrowed into a `pbr` set's
/// RMAOS slot does not answer "needs _rmaos".
#[test]
fn a_borrowed_rfaos_keeps_the_missing_rmaos() {
    use engine::config::Subtype;
    use image::{ImageBuffer, Rgba};
    use sdc_gui::model::Answer;
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    ImageBuffer::from_fn(64, 64, |x, _| Rgba([(x * 4) as u8, 0, 200, 255]))
        .save(r.join("skin01_rfaos.png"))
        .unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let id = "landscape/rock02";
    m.set_shader(id, Some(engine::shader::Shader::Pbr));
    m.decide(
        "landscape/skin01_rfaos.png",
        Answer::Subtype(Subtype::SkinRfaos),
    );
    idle(&mut m);
    assert_eq!(missing_maps(&m, id), ["_rmaos"], "before the borrow");
    let rfaos = key_of(&m, "skin01_rfaos.png");
    assert_eq!(
        m.item(&rfaos)
            .and_then(|i| i.job.as_ref())
            .map(|j| j.profile.subtype),
        Some(Subtype::SkinRfaos)
    );
    assert_eq!(
        m.drop_choice(&rfaos, id, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    assert!(matches!(
        view(&set_views(&m), id).slots.get(&SlotKind::Rmaos),
        Some(SlotFill::Borrowed { .. })
    ));
    assert_eq!(
        missing_maps(&m, id),
        ["_rmaos"],
        "an _rfaos is not an _rmaos"
    );
    assert_eq!(needs_rows(&m, id, "_rmaos"), 1);
}

/// Note HH8: a borrowed packed map (the lender packs its own `_rmaos`) answers "needs _rmaos" as a normal does.
#[test]
fn a_borrowed_packed_rmaos_answers_the_missing_map_warning() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let id = "landscape/rock02";
    // Under `textures/landscape/` the auto pick is PBR Landscape, which needs no `_rmaos`; plain `pbr` does.
    m.set_shader(id, Some(engine::shader::Shader::Pbr));
    idle(&mut m);
    assert_eq!(missing_maps(&m, id), ["_rmaos"], "before the borrow");
    assert_eq!(needs_rows(&m, id, "_rmaos"), 1);
    let rmaos = key_of(&m, "rock01_rmaos");
    assert_eq!(
        m.drop_choice(&rmaos, id, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    assert!(matches!(
        view(&set_views(&m), id).slots.get(&SlotKind::Rmaos),
        Some(SlotFill::Borrowed { .. })
    ));
    assert_eq!(missing_maps(&m, id), Vec::<String>::new());
    assert_eq!(needs_rows(&m, id, "_rmaos"), 0);
}

/// Note JN1 (B2-2): a borrow whose lender borrows that slot too follows it to the set that owns the file. The fill
/// names that set, the engine bakes the borrower's own rmaos from its normal, and the slot answers "needs _n" as a
/// direct borrow does (HH8, B1-5: rock01's normal is a `pbr_normal`).
#[test]
fn a_borrow_from_a_lender_that_borrows_the_slot_fills_from_the_source() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), true, false);
    let id = "landscape/rock02";
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock02", 2, Some("landscape/leaf"));
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
    });
    idle(&mut m);
    let views = set_views(&m);
    let v = view(&views, id);
    assert!(
        matches!(
            v.slots.get(&SlotKind::Normal),
            Some(SlotFill::Borrowed { from, item }) if from == "landscape/rock01" && *item == normal
        ),
        "{:?} {:?}",
        v.slots,
        v.missing
    );
    assert!(v.missing.is_empty(), "{:?}", v.missing);
    let borrowed = borrowed_normal(&m);
    assert_eq!(
        borrowed.get(id).map(String::as_str),
        Some("landscape/rock01"),
        "{borrowed:?}"
    );
    assert_eq!(m.request().unwrap().borrowed_normal, borrowed);
    assert_eq!(missing_maps(&m, id), Vec::<String>::new());
    assert_eq!(needs_rows(&m, id, "_n"), 0);
    let rmaos02 = key_of(&m, "rock02_rmaos.png");
    let j = m
        .item(&rmaos02)
        .and_then(|i| i.job.clone())
        .expect("rock02_rmaos has a job");
    let b = j.bake.as_ref().expect("the borrower's own rmaos bakes");
    assert!(b.normal.ends_with("rock01_normal.png"), "{:?}", b.normal);
}

/// Note JN1 (B1-5, B1-6 carried over): a chain to a model-space normal fills the slot but keeps pbr's "needs _n"
/// (a plain `normal` would answer it, B1-6, as `a_borrowed_vanilla_normal_answers_the_pbr_missing_normal` pins).
#[test]
fn a_chain_to_a_model_space_normal_keeps_the_missing_normal() {
    use engine::config::Subtype;
    use sdc_gui::model::Answer;
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), true, false);
    let id = "landscape/rock02";
    m.set_shader(id, Some(engine::shader::Shader::Pbr));
    m.decide("landscape/rock01_normal.png", Answer::Subtype(Subtype::Msn));
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock02", 2, Some("landscape/leaf"));
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
    });
    idle(&mut m);
    assert!(matches!(
        view(&set_views(&m), id).slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { from, .. }) if from == "landscape/rock01"
    ));
    assert!(
        missing_maps(&m, id).contains(&"_n".to_string()),
        "a model-space normal does not answer pbr's _n"
    );
    assert_eq!(needs_rows(&m, id, "_n"), 1);
}

/// Notes HH8, JN1 (B1-6 guard, a pin): the "any tangent-space normal" rule is only for a role that takes a tangent-space
/// normal. `eye`'s `_n (model-space)` takes `msn` only, so a borrowed tangent-space normal keeps "needs" there. Fails
/// if `borrow_answers` loses its `takes_normal &&`.
#[test]
fn a_borrowed_tangent_space_normal_keeps_the_eye_shaders_model_space_warning() {
    use engine::config::Subtype;
    use sdc_gui::model::Answer;
    for st in [Subtype::Normal, Subtype::PbrNormal] {
        let d = tempfile::tempdir().unwrap();
        let mut m = rock02_opened(d.path(), false, false);
        let id = "landscape/rock02";
        m.set_shader(id, Some(engine::shader::Shader::Eye));
        m.decide("landscape/rock01_normal.png", Answer::Subtype(st));
        idle(&mut m);
        let map = "_n (model-space)".to_string();
        assert!(missing_maps(&m, id).contains(&map), "{st:?}: before");
        let normal = key_of(&m, "rock01_normal.png");
        assert_eq!(
            m.drop_choice(&normal, id, DropChoice::Share),
            DropOutcome::Joined
        );
        idle(&mut m);
        assert!(matches!(
            view(&set_views(&m), id).slots.get(&SlotKind::Normal),
            Some(SlotFill::Borrowed { .. })
        ));
        assert!(
            missing_maps(&m, id).contains(&map),
            "{st:?}: a tangent-space normal does not answer eye's model-space _n"
        );
        assert_eq!(needs_rows(&m, id, &map), 1, "{st:?}");
    }
}

/// Note JN1 (B2-2): a cycle fills nothing and reads "(it has none)"; a set that borrows from itself keeps its own
/// file, is reported too, and sends no `X → X` share to the engine. Nothing hangs.
#[test]
fn a_borrow_cycle_fills_nothing_and_a_self_borrow_keeps_its_own_file() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/odd"));
        s.set_borrow("landscape/odd", 2, Some("landscape/leaf"));
        s.set_borrow("landscape/rock01", 2, Some("landscape/rock01"));
    });
    idle(&mut m);
    let views = set_views(&m);
    for (id, from) in [
        ("landscape/leaf", "landscape/odd"),
        ("landscape/odd", "landscape/leaf"),
        ("landscape/rock01", "landscape/rock01"),
    ] {
        let v = view(&views, id);
        assert!(
            v.missing.contains(&Missing::Borrow {
                slot: 2,
                from: from.into(),
                empty: true,
                itself: id == from,
            }),
            "{id}: {:?}",
            v.missing
        );
    }
    let borrowed = borrowed_normal(&m);
    assert!(borrowed.is_empty(), "{borrowed:?}");
    assert_eq!(m.request().unwrap().borrowed_normal, borrowed);
    for id in ["landscape/leaf", "landscape/odd"] {
        assert!(
            !view(&views, id).slots.contains_key(&SlotKind::Normal),
            "{id}"
        );
    }
    assert_eq!(
        view(&views, "landscape/rock01")
            .slots
            .get(&SlotKind::Normal),
        Some(&SlotFill::Own { item: normal })
    );
}

/// Note JN1 (planner's call): a lender holding its own file and a borrow line for the same slot lends its own file,
/// as a single hop did before the follow. Its own card shows the borrow.
#[test]
fn a_lender_with_its_own_file_and_a_borrow_line_lends_its_own() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let normal = key_of(&m, "rock01_normal.png");
    let n02 = key_of(&m, "rock02_n.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_borrow("landscape/rock01", 2, Some("landscape/rock02"));
    });
    idle(&mut m);
    let views = set_views(&m);
    assert!(matches!(
        view(&views, "landscape/rock01").slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { item, .. }) if *item == n02
    ));
    assert!(matches!(
        view(&views, "landscape/leaf").slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { from, item }) if from == "landscape/rock01" && *item == normal
    ));
}

/// Final wave Group 2 (review Important 3, Q5 b): rock01 owns a normal and borrows rock02's into its Normal slot, and
/// its own normal still converts (leaf could borrow it). The Output files list is what the set's own members write, so
/// the displaced normal is listed after the ordered slot pass, under its slot kind.
#[test]
fn the_output_files_list_an_own_normal_a_borrow_displaced() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock01", 2, Some("landscape/rock02"));
    });
    idle(&mut m);
    let out = m
        .item(&normal)
        .and_then(|i| i.job.as_ref())
        .map(|j| j.output.clone())
        .expect("premise: the displaced normal still has a job");
    let views = set_views(&m);
    let v = view(&views, "landscape/rock01");
    assert!(
        matches!(
            v.slots.get(&SlotKind::Normal),
            Some(SlotFill::Borrowed { .. })
        ),
        "premise: the borrow holds the Normal slot"
    );
    let listed = set_outputs(v);
    let at = listed
        .iter()
        .position(|(k, i, p)| *i == normal && *p == out && *k == Some(SlotKind::Normal))
        .unwrap_or_else(|| panic!("the displaced normal is listed: {listed:?}"));
    let own_slots = v
        .slots
        .values()
        .filter(|f| matches!(f, SlotFill::Own { .. }))
        .count();
    assert!(
        at >= own_slots,
        "it comes after the {own_slots} own slots: {listed:?}"
    );
    // The packed RMAOS output is both an own slot and a member: listed once.
    let paths: std::collections::BTreeSet<_> = listed.iter().map(|(_, _, p)| p).collect();
    assert_eq!(paths.len(), listed.len(), "each path once: {listed:?}");
    assert!(
        listed
            .iter()
            .any(|(k, i, _)| *k == Some(SlotKind::Rmaos) && i == "landscape/rock01_rmaos"),
        "premise: the packed output is listed: {listed:?}"
    );
}

/// Note JN2: what `sdc-project.toml` says set `id` borrows for slot `n`.
fn borrow_of(m: &sdc_gui::model::AppModel, id: &str, n: u8) -> Option<String> {
    m.sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.borrow.get(&n).cloned())
}

/// Note JN2: runs `edit` and waits for the plan; checks that it made exactly one undo step and, for `locked =
/// Some((id, n))`, that set `id` still locks slot `n` in `sdc-project.toml`. Returns the file's text from before the
/// edit, for `undo_restores`.
fn one_step(
    m: &mut sdc_gui::model::AppModel,
    locked: Option<(&str, u8)>,
    edit: impl FnOnce(&mut sdc_gui::model::AppModel),
) -> String {
    let before = m.sets_doc.as_ref().unwrap().text();
    let steps = m.history.undo.len();
    edit(m);
    idle(m);
    assert_eq!(m.history.undo.len(), steps + 1, "one undo step");
    if let Some((id, n)) = locked {
        assert!(
            m.sets_doc
                .as_ref()
                .unwrap()
                .entries()
                .iter()
                .any(|e| e.id == id && e.lock.contains(&n)),
            "{id} keeps its slot{n} lock"
        );
    }
    before
}

/// Note JN2: one undo puts `sdc-project.toml` back to `before`, every set's line with it.
fn undo_restores(m: &mut sdc_gui::model::AppModel, before: &str) {
    m.undo();
    idle(m);
    assert_eq!(
        m.sets_doc.as_ref().unwrap().text(),
        before,
        "one undo restores every entry"
    );
}

/// Note JN2 (B2-1): moving a lent normal to another set rewrites every set sharing it, a locked slot too, in one
/// undo step.
#[test]
fn moving_a_lent_normal_to_another_set_moves_its_borrowers_in_one_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), true, false);
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_lock("landscape/leaf", 2, true);
        s.set_borrow("landscape/odd", 2, Some("landscape/rock01"));
    });
    idle(&mut m);
    let before = one_step(&mut m, Some(("landscape/leaf", 2)), |m| {
        assert_eq!(
            m.drop_choice(&normal, "landscape/rock02", DropChoice::Move),
            DropOutcome::Joined
        );
    });
    let views = set_views(&m);
    for id in ["landscape/leaf", "landscape/odd"] {
        assert_eq!(
            borrow_of(&m, id, 2).as_deref(),
            Some("landscape/rock02"),
            "{id}"
        );
        assert!(
            matches!(
                view(&views, id).slots.get(&SlotKind::Normal),
                Some(SlotFill::Borrowed { from, item }) if from == "landscape/rock02" && *item == normal
            ),
            "{id}"
        );
    }
    assert!(
        view(&views, "landscape/leaf").locks.contains(&2),
        "the lock stays"
    );
    undo_restores(&mut m, &before);
}

/// Note JN2 (B2-1): the set a lent file moves into owns it now, so it shares that slot from nobody, and the other set
/// sharing the file follows it there. The no-self-borrow assert is a pin (the Move-replace's `clear_slot` already
/// dropped rock02's line before JN2); leaf's line naming rock02 is what JN2 added.
#[test]
fn moving_a_lent_normal_into_its_borrower_leaves_no_self_borrow() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), true, false);
    let id = "landscape/rock02";
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"))
    });
    assert_eq!(
        m.drop_choice(&normal, id, DropChoice::Share),
        DropOutcome::Joined
    );
    idle(&mut m);
    assert_eq!(
        m.drop_choice(&normal, id, DropChoice::Move),
        DropOutcome::AskReplace {
            existing: normal.clone()
        }
    );
    m.drop_replace(&normal, id, DropChoice::Move).unwrap();
    idle(&mut m);
    assert_eq!(borrow_of(&m, id, 2), None);
    assert_eq!(
        view(&set_views(&m), id).slots.get(&SlotKind::Normal),
        Some(&SlotFill::Own {
            item: normal.clone()
        })
    );
    assert_eq!(borrow_of(&m, "landscape/leaf", 2).as_deref(), Some(id));
}

/// Note JN2 (B2-1): a new set made from a lent file takes its borrowers, and Fill from names brings them back; each
/// in one undo step, a locked borrower slot rewritten too.
#[test]
fn a_new_set_from_a_lent_file_and_fill_from_names_take_its_borrowers() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let n02 = key_of(&m, "rock02_n.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock02"));
        s.set_lock("landscape/leaf", 2, true);
    });
    idle(&mut m);
    let mut id = String::new();
    let before = one_step(&mut m, Some(("landscape/leaf", 2)), |m| {
        id = m.drop_new_set(&n02).unwrap();
    });
    // `drop_new_set` names the set `auto_id` + "-2" when the automatic id is taken.
    assert_eq!(id, "landscape/rock02-2");
    assert_eq!(borrow_of(&m, "landscape/leaf", 2), Some(id.clone()));
    undo_restores(&mut m, &before);
    one_step(&mut m, None, |m| {
        m.drop_new_set(&n02).unwrap();
    });
    let before = one_step(&mut m, Some(("landscape/leaf", 2)), |m| {
        assert_eq!(
            m.fill_from_names("landscape/rock02", std::slice::from_ref(&n02)),
            Ok(1)
        );
    });
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 2).as_deref(),
        Some("landscape/rock02")
    );
    assert!(matches!(
        view(&set_views(&m), "landscape/leaf").slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { from, item }) if from == "landscape/rock02" && *item == n02
    ));
    undo_restores(&mut m, &before);
}

/// Notes HF4, HS13: renaming a lender rewrites the sets sharing from it (survey §3's test gap).
#[test]
fn renaming_a_lender_rewrites_its_borrowers() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_lock("landscape/leaf", 2, true);
    });
    idle(&mut m);
    m.rename_set("landscape/rock01", "landscape/boulder")
        .unwrap();
    idle(&mut m);
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 2).as_deref(),
        Some("landscape/boulder")
    );
    assert!(matches!(
        view(&set_views(&m), "landscape/leaf").slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { from, item }) if from == "landscape/boulder" && *item == normal
    ));
}

/// Note JN2: the slot-3 fixture (Tasks 2 and 5). rock01 gets `rock01_sk.png` (a skin `_sk`) and, with `lender_g`,
/// `rock01_g.png` (a PBR emissive); leaf gets `leaf_g.png` (a PBR emissive) with `leaf_g`. All decided; grouped by
/// name.
fn slot3_opened(d: &std::path::Path, lender_g: bool, leaf_g: bool) -> sdc_gui::model::AppModel {
    use engine::config::Subtype;
    use image::{ImageBuffer, Rgb};
    use sdc_gui::model::Answer;
    let r = d.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    let mut files = vec![("rock01_sk.png", Subtype::SkinSk)];
    if lender_g {
        files.push(("rock01_g.png", Subtype::PbrEmissive));
    }
    if leaf_g {
        files.push(("leaf_g.png", Subtype::PbrEmissive));
    }
    for (n, _) in &files {
        ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 30u8]))
            .save(r.join(n))
            .unwrap();
    }
    let mut m = pbr_opened(d);
    for (n, st) in &files {
        m.decide(&format!("landscape/{n}"), Answer::Subtype(*st));
    }
    idle(&mut m);
    for (n, _) in &files {
        let set = if n.starts_with("leaf") {
            "landscape/leaf"
        } else {
            "landscape/rock01"
        };
        assert!(
            view(&set_views(&m), set)
                .members
                .iter()
                .any(|k| k.ends_with(n)),
            "{n} is grouped into {set}"
        );
    }
    m
}

/// The item set `id` resolves its slot-3 share to.
fn slot3_share(m: &sdc_gui::model::AppModel, id: &str) -> String {
    view(&set_views(m), id)
        .slots
        .iter()
        .find_map(|(k, f)| match f {
            SlotFill::Borrowed { item, .. } if k.slot_number() == Some(3) => Some(item.clone()),
            _ => None,
        })
        .expect("shares slot 3")
}

/// Note JN2 (planner's call, lent by slot number): rock01 holds both kinds of slot 3 and lends one of them. Moving
/// the other one out leaves leaf on rock01; moving the lent one out takes leaf with it.
#[test]
fn moving_the_lent_kind_of_a_two_kind_slot_takes_its_borrowers_and_the_other_does_not() {
    let d = tempfile::tempdir().unwrap();
    let mut m = slot3_opened(d.path(), true, false);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 3, Some("landscape/rock01"))
    });
    idle(&mut m);
    let lent = slot3_share(&m, "landscape/leaf");
    let (g, sk) = (key_of(&m, "rock01_g.png"), key_of(&m, "rock01_sk.png"));
    let other = if lent == g { sk } else { g };
    let before = one_step(&mut m, None, |m| {
        assert_eq!(
            m.drop_choice(&other, "landscape/odd", DropChoice::Move),
            DropOutcome::Joined
        );
    });
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 3).as_deref(),
        Some("landscape/rock01"),
        "the file leaf does not read moved: leaf stays"
    );
    undo_restores(&mut m, &before);
    let before = one_step(&mut m, None, |m| {
        assert_eq!(
            m.drop_choice(&lent, "landscape/odd", DropChoice::Move),
            DropOutcome::Joined
        );
    });
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 3).as_deref(),
        Some("landscape/odd"),
        "the file leaf reads moved: leaf follows it"
    );
    undo_restores(&mut m, &before);
}

/// Note JN3 (B2-1b): a Share that replaces a lent file points the sets sharing that slot at the new source's set,
/// in the same step, so no chain is written.
#[test]
fn replacing_a_lent_file_by_a_share_points_its_borrowers_at_the_new_source() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let id = "landscape/rock02";
    let normal = key_of(&m, "rock01_normal.png");
    let n02 = key_of(&m, "rock02_n.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock02"));
        s.set_lock("landscape/leaf", 2, true);
    });
    idle(&mut m);
    assert_eq!(
        m.drop_choice(&normal, id, DropChoice::Share),
        DropOutcome::AskReplace { existing: n02 }
    );
    let before = one_step(&mut m, Some(("landscape/leaf", 2)), |m| {
        m.drop_replace(&normal, id, DropChoice::Share).unwrap();
    });
    assert_eq!(borrow_of(&m, id, 2).as_deref(), Some("landscape/rock01"));
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 2).as_deref(),
        Some("landscape/rock01"),
        "no chain is written"
    );
    assert!(matches!(
        view(&set_views(&m), "landscape/leaf").slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { from, item }) if from == "landscape/rock01" && *item == normal
    ));
    let borrowed = borrowed_normal(&m);
    assert_eq!(
        borrowed.get("landscape/leaf").map(String::as_str),
        Some("landscape/rock01")
    );
    assert_eq!(m.request().unwrap().borrowed_normal, borrowed);
    undo_restores(&mut m, &before);
}

/// Note JN3 (B2-1b): a Share into an empty slot that another set shares passes that set to the new source. odd shares
/// slot 2 from leaf, which has no normal; sharing rock01's normal into leaf points odd at rock01, in one undo step.
#[test]
fn a_share_into_an_empty_shared_slot_points_its_borrowers_at_the_new_source() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/odd", 2, Some("landscape/leaf"))
    });
    idle(&mut m);
    let before = one_step(&mut m, None, |m| {
        assert_eq!(
            m.drop_choice(&normal, "landscape/leaf", DropChoice::Share),
            DropOutcome::Joined
        );
    });
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 2).as_deref(),
        Some("landscape/rock01")
    );
    assert_eq!(
        borrow_of(&m, "landscape/odd", 2).as_deref(),
        Some("landscape/rock01"),
        "no chain is written"
    );
    let borrowed = borrowed_normal(&m);
    for id in ["landscape/leaf", "landscape/odd"] {
        assert_eq!(
            borrowed.get(id).map(String::as_str),
            Some("landscape/rock01"),
            "{id}"
        );
    }
    assert_eq!(m.request().unwrap().borrowed_normal, borrowed);
    undo_restores(&mut m, &before);
}

/// Note JN3 (B2-1b): a Share that replaces a share passes the slot's sharing sets to the new source. leaf shares
/// rock02's normal and odd shares leaf's slot; sharing rock01's normal into leaf points both at rock01.
#[test]
fn a_share_that_replaces_a_share_points_its_borrowers_at_the_new_source() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock02"));
        s.set_borrow("landscape/odd", 2, Some("landscape/leaf"));
    });
    idle(&mut m);
    assert!(matches!(
        m.drop_choice(&normal, "landscape/leaf", DropChoice::Share),
        DropOutcome::AskReplace { .. }
    ));
    let before = one_step(&mut m, None, |m| {
        m.drop_replace(&normal, "landscape/leaf", DropChoice::Share)
            .unwrap()
    });
    for id in ["landscape/leaf", "landscape/odd"] {
        assert_eq!(
            borrow_of(&m, id, 2).as_deref(),
            Some("landscape/rock01"),
            "{id}"
        );
    }
    assert!(matches!(
        view(&set_views(&m), "landscape/odd").slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { from, item }) if from == "landscape/rock01" && *item == normal
    ));
    let borrowed = borrowed_normal(&m);
    assert_eq!(
        borrowed.get("landscape/odd").map(String::as_str),
        Some("landscape/rock01")
    );
    assert_eq!(m.request().unwrap().borrowed_normal, borrowed);
    undo_restores(&mut m, &before);
}

/// Note JN3 (t3): a Share refused (the target slot is locked) changes no line: the set sharing that slot stays on it.
/// A pin: it fails if `occupant` loses its `lock_refusal` (the Share then runs and rewrites leaf's line).
#[test]
fn a_refused_share_leaves_the_sharing_sets_lines() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock02"));
        s.set_lock("landscape/rock02", 2, true);
    });
    idle(&mut m);
    let before = m.sets_doc.as_ref().unwrap().text();
    let steps = m.history.undo.len();
    assert!(matches!(
        m.drop_choice(&normal, "landscape/rock02", DropChoice::Share),
        DropOutcome::Refused(_)
    ));
    idle(&mut m);
    assert_eq!(m.history.undo.len(), steps);
    assert_eq!(m.sets_doc.as_ref().unwrap().text(), before);
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 2).as_deref(),
        Some("landscape/rock02")
    );
}

/// Note JN3 (B2-1b): a set that shared the slot and is the new source owns the file: it drops its line (no cycle).
#[test]
fn a_borrower_that_is_the_new_source_drops_its_borrow() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let id = "landscape/rock02";
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock01", 2, Some("landscape/rock02"))
    });
    idle(&mut m);
    assert!(matches!(
        m.drop_choice(&normal, id, DropChoice::Share),
        DropOutcome::AskReplace { .. }
    ));
    let before = one_step(&mut m, None, |m| {
        m.drop_replace(&normal, id, DropChoice::Share).unwrap()
    });
    assert_eq!(borrow_of(&m, "landscape/rock01", 2), None);
    let views = set_views(&m);
    assert_eq!(
        view(&views, "landscape/rock01")
            .slots
            .get(&SlotKind::Normal),
        Some(&SlotFill::Own {
            item: normal.clone()
        })
    );
    assert!(matches!(
        view(&views, id).slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { from, item }) if from == "landscape/rock01" && *item == normal
    ));
    let borrowed = borrowed_normal(&m);
    assert_eq!(
        borrowed.get(id).map(String::as_str),
        Some("landscape/rock01")
    );
    assert!(!borrowed.contains_key("landscape/rock01"), "{borrowed:?}");
    assert_eq!(m.request().unwrap().borrowed_normal, borrowed);
    undo_restores(&mut m, &before);
}

/// Note JN3 (B2-1b): a Move, a Duplicate or a Fill from names that replaces a lent file leaves the sharing sets on
/// the lender; they get the new file ("share rock02's normal" keeps meaning rock02's normal). The Fill leg fills a
/// file from another set (odd), so a Share-style rewrite to that set would fail it.
#[test]
fn a_move_duplicate_or_fill_that_replaces_a_lent_file_leaves_the_borrowers_on_the_lender() {
    use engine::config::{GreenConvention, Subtype};
    use sdc_gui::model::Answer;
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let id = "landscape/rock02";
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock02"))
    });
    idle(&mut m);
    let leaf_fill = |m: &sdc_gui::model::AppModel| {
        view(&set_views(m), "landscape/leaf")
            .slots
            .get(&SlotKind::Normal)
            .cloned()
    };
    m.drop_replace(&normal, id, DropChoice::Move).unwrap();
    idle(&mut m);
    assert_eq!(borrow_of(&m, "landscape/leaf", 2).as_deref(), Some(id));
    assert_eq!(
        leaf_fill(&m),
        Some(SlotFill::Borrowed {
            from: id.into(),
            item: normal.clone()
        })
    );
    m.undo();
    idle(&mut m);
    m.drop_replace(&normal, id, DropChoice::Duplicate).unwrap();
    idle(&mut m);
    let copy = "landscape/rock02_normal.png";
    m.decide(copy, Answer::Subtype(Subtype::PbrNormal));
    m.decide(copy, Answer::Green(GreenConvention::Directx));
    idle(&mut m);
    assert_eq!(borrow_of(&m, "landscape/leaf", 2).as_deref(), Some(id));
    assert_eq!(
        leaf_fill(&m),
        Some(SlotFill::Borrowed {
            from: id.into(),
            item: copy.into()
        })
    );
    // Fill from names: rock02_n.png went to no set when the copy replaced it, still decided a `pbr_normal`. It is moved
    // into odd first, so the Fill takes it from another set; its name matches rock02's base, so Fill offers it for the
    // Normal slot, replacing the copy.
    let n02 = key_of(&m, "rock02_n.png");
    assert_eq!(
        m.drop_choice(&n02, "landscape/odd", DropChoice::Move),
        DropOutcome::Joined
    );
    idle(&mut m);
    assert!(view(&set_views(&m), "landscape/odd").members.contains(&n02));
    assert_eq!(m.fill_from_names(id, std::slice::from_ref(&n02)), Ok(1));
    idle(&mut m);
    assert_eq!(borrow_of(&m, "landscape/leaf", 2).as_deref(), Some(id));
    assert_eq!(
        leaf_fill(&m),
        Some(SlotFill::Borrowed {
            from: id.into(),
            item: n02
        })
    );
}

/// Note JN4: leaf shares rock01's normal with its slot locked, odd shares rock01's packed rmaos. Neither can take
/// the file when it leaves rock01 (Note JN5: a locked slot, a packed map), so both slots go empty.
fn lent_to_leaf_and_odd(d: &std::path::Path) -> sdc_gui::model::AppModel {
    let mut m = pbr_opened(d);
    idle(&mut m);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_lock("landscape/leaf", 2, true);
        s.set_borrow("landscape/odd", 6, Some("landscape/rock01"));
    });
    idle(&mut m);
    m
}

/// The newest Messages line that starts with `start`.
fn message_starting(m: &sdc_gui::model::AppModel, start: &str) -> String {
    m.messages
        .entries()
        .rev()
        .find(|e| e.text.starts_with(start))
        .map(|e| e.text.clone())
        .unwrap_or_else(|| panic!("no message starting {start:?}"))
}

/// Note JN4 (B2-3): deleting a lender names the sets left with an empty shared slot; their lines stay (HE3).
#[test]
fn deleting_a_lender_keeps_the_borrows_and_names_the_sets_that_lost_a_texture() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lent_to_leaf_and_odd(d.path());
    m.delete_set_with_toast("landscape/rock01");
    idle(&mut m);
    assert_eq!(
        message_starting(&m, "Deleted set"),
        "Deleted set landscape/rock01 (4 textures now in no set). 2 sets shared rock01_normal.png and \
         rock01_rmaos; those slots are empty now: landscape/leaf and landscape/odd."
    );
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 2).as_deref(),
        Some("landscape/rock01")
    );
    assert_eq!(
        borrow_of(&m, "landscape/odd", 6).as_deref(),
        Some("landscape/rock01")
    );
}

/// Note JN4 (B2-3): removing a lent file says which set lost it; a file no set shares adds nothing.
#[test]
fn removing_a_lent_file_names_the_set_that_lost_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lent_to_leaf_and_odd(d.path());
    let normal = key_of(&m, "rock01_normal.png");
    m.remove_member("landscape/rock01", &normal).unwrap();
    idle(&mut m);
    assert_eq!(
        message_starting(&m, "rock01_normal.png"),
        "rock01_normal.png is in no set now. landscape/leaf shared it; that slot is empty now."
    );
    let diffuse = key_of(&m, "rock01.png");
    m.remove_member("landscape/rock01", &diffuse).unwrap();
    idle(&mut m);
    assert!(
        m.messages
            .entries()
            .all(|e| !e.text.starts_with("rock01.png")),
        "a file no set shares says nothing"
    );
}

/// Note JN4 (B2-3): a packed output leaves its set with the last of its raw maps, while the set keeps other files.
/// Removing that raw map names the sets that shared the pack; removing the set's last file afterwards adds nothing.
#[test]
fn removing_the_last_raw_map_of_a_lent_pack_names_the_sets_that_shared_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lent_to_leaf_and_odd(d.path());
    let rmaos = key_of(&m, "rock01_rmaos");
    let odd_rmaos = |m: &sdc_gui::model::AppModel| {
        view(&set_views(m), "landscape/odd")
            .slots
            .get(&SlotKind::Rmaos)
            .cloned()
    };
    for f in ["rock01_normal.png", "rock01_roughness.png"] {
        let k = key_of(&m, f);
        m.remove_member("landscape/rock01", &k).unwrap();
        idle(&mut m);
    }
    assert_eq!(
        odd_rmaos(&m),
        Some(SlotFill::Borrowed {
            from: "landscape/rock01".into(),
            item: rmaos
        }),
        "the pack still fills the slot while one of its raw maps stays"
    );
    let odd_named = |m: &sdc_gui::model::AppModel| {
        m.messages
            .entries()
            .filter(|e| e.text.contains("landscape/odd"))
            .count()
    };
    assert_eq!(odd_named(&m), 0, "no earlier removal names odd");
    let ao = key_of(&m, "rock01_ao.png");
    m.remove_member("landscape/rock01", &ao).unwrap();
    idle(&mut m);
    assert_eq!(odd_rmaos(&m), None, "the pack left with its last raw map");
    assert_eq!(
        message_starting(&m, "rock01_ao.png"),
        "rock01_ao.png is in no set now. landscape/odd shared rock01_rmaos; that slot is empty now."
    );
    let diffuse = key_of(&m, "rock01.png");
    m.remove_member("landscape/rock01", &diffuse).unwrap();
    idle(&mut m);
    assert_eq!(
        odd_named(&m),
        1,
        "the set's last file does not name odd again"
    );
}

/// Note JN4 (B2-11): a share that passes through a deleted set is passed on to the set it shared from, in the same
/// undo step; nothing is lost, so the toast names nobody.
#[test]
fn deleting_the_middle_of_a_chain_passes_the_share_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_borrow("landscape/odd", 2, Some("landscape/leaf"));
    });
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    let before = one_step(&mut m, None, |m| m.delete_set_with_toast("landscape/leaf"));
    assert_eq!(
        borrow_of(&m, "landscape/odd", 2).as_deref(),
        Some("landscape/rock01")
    );
    assert_eq!(
        view(&set_views(&m), "landscape/odd")
            .slots
            .get(&SlotKind::Normal),
        Some(&SlotFill::Borrowed {
            from: "landscape/rock01".into(),
            item: normal
        })
    );
    assert_eq!(
        message_starting(&m, "Deleted set"),
        "Deleted set landscape/leaf (1 texture now in no set)"
    );
    undo_restores(&mut m, &before);
}

/// Note JN5: leaf and odd both share rock01's normal (slot 2); leaf sorts first.
fn normal_lent_twice(d: &std::path::Path) -> sdc_gui::model::AppModel {
    let mut m = pbr_opened(d);
    idle(&mut m);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_borrow("landscape/odd", 2, Some("landscape/rock01"));
    });
    idle(&mut m);
    m
}

/// Note JN5 (B2-6, owner: option 1): deleting a lender hands its shared normal to the first set sharing it, as that
/// set's own file; the other one (its slot locked) shares it from there; one undo step; the toast says where it went.
#[test]
fn deleting_a_lender_hands_its_shared_file_to_the_first_borrower() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normal_lent_twice(d.path());
    m.edit_docs("t", None, true, |_, s| s.set_lock("landscape/odd", 2, true));
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    let before = one_step(&mut m, Some(("landscape/odd", 2)), |m| {
        m.delete_set_with_toast("landscape/rock01")
    });
    assert_eq!(
        message_starting(&m, "Deleted set"),
        "Deleted set landscape/rock01 (3 textures now in no set). rock01_normal.png went to landscape/leaf, which \
         shared it; 1 other set now shares it from there."
    );
    assert!(!m.is_no_set(&normal));
    let views = set_views(&m);
    assert_eq!(
        view(&views, "landscape/leaf").slots.get(&SlotKind::Normal),
        Some(&SlotFill::Own {
            item: normal.clone()
        })
    );
    assert_eq!(borrow_of(&m, "landscape/leaf", 2), None, "no self-borrow");
    assert_eq!(
        borrow_of(&m, "landscape/odd", 2).as_deref(),
        Some("landscape/leaf")
    );
    assert!(matches!(
        view(&views, "landscape/odd").slots.get(&SlotKind::Normal),
        Some(SlotFill::Borrowed { from, item }) if from == "landscape/leaf" && *item == normal
    ));
    assert!(engine_inputs(&m).0.iter().any(|e| matches!(
        e,
        engine::sets::SetEdit::Move { file, to } if *file == normal && to == "landscape/leaf"
    )));
    undo_restores(&mut m, &before);
}

/// Note JN5 (B2-7): a set whose slot is locked is skipped for the next one, and its share is rewritten to that one,
/// lock kept, in one undo step (Remove from set).
#[test]
fn a_borrower_that_cannot_take_the_file_is_skipped_for_the_next() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normal_lent_twice(d.path());
    m.edit_docs("t", None, true, |_, s| {
        s.set_lock("landscape/leaf", 2, true)
    });
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    let before = one_step(&mut m, Some(("landscape/leaf", 2)), |m| {
        m.remove_member("landscape/rock01", &normal).unwrap()
    });
    assert_eq!(
        message_starting(&m, "rock01_normal.png"),
        "rock01_normal.png went to landscape/odd, which shared it; 1 other set now shares it from there."
    );
    let views = set_views(&m);
    assert_eq!(
        view(&views, "landscape/odd").slots.get(&SlotKind::Normal),
        Some(&SlotFill::Own {
            item: normal.clone()
        })
    );
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 2).as_deref(),
        Some("landscape/odd")
    );
    assert!(view(&views, "landscape/leaf").locks.contains(&2));
    undo_restores(&mut m, &before);
}

/// Note JN5 (B2-7): a set that has its own file of that kind is skipped; "first" is the Sets column's order, not the
/// id's.
#[test]
fn a_borrower_with_its_own_file_of_that_kind_is_skipped() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock02", 2, Some("landscape/rock01"));
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_order(&[
            "landscape/rock02".to_string(),
            "landscape/rock01".into(),
            "landscape/leaf".into(),
        ]);
    });
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    m.remove_member("landscape/rock01", &normal).unwrap();
    idle(&mut m);
    assert_eq!(
        message_starting(&m, "rock01_normal.png"),
        "rock01_normal.png went to landscape/leaf, which shared it; 1 other set now shares it from there."
    );
    assert_eq!(
        borrow_of(&m, "landscape/rock02", 2).as_deref(),
        Some("landscape/leaf")
    );
}

/// Note JN5 (B2-7): a set that cannot take a drop (`drop_target`) cannot take the file either. `landscape/ghost` is
/// an entry with no files that the user did not create (a view with an empty key, `NoFilesLeft`); leaf's only file
/// went to no set, which leaves leaf the same (its entry keeps a view with an empty key). Both are skipped, odd takes
/// the file, and both lines now name odd. A set with no view is never an heir: heirs come from `views()`.
#[test]
fn borrowers_with_no_files_are_skipped() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normal_lent_twice(d.path());
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/ghost", 2, Some("landscape/rock01"))
    });
    idle(&mut m);
    let leaf = key_of(&m, "leaf.png");
    m.remove_member("landscape/leaf", &leaf).unwrap();
    idle(&mut m);
    let views = set_views(&m);
    let ghost = view(&views, "landscape/ghost");
    assert!(
        ghost.key.is_empty() && !ghost.created,
        "ghost: no files, not created"
    );
    let normal = key_of(&m, "rock01_normal.png");
    m.remove_member("landscape/rock01", &normal).unwrap();
    idle(&mut m);
    assert_eq!(
        message_starting(&m, "rock01_normal.png"),
        "rock01_normal.png went to landscape/odd, which shared it; 2 other sets now share it from there."
    );
    for id in ["landscape/ghost", "landscape/leaf"] {
        assert_eq!(
            borrow_of(&m, id, 2).as_deref(),
            Some("landscape/odd"),
            "{id}"
        );
    }
}

/// Note JN5 (B2-7): slot 3 holds two kinds. Leaf owns an emissive `_g` there, so it cannot take rock01's `_sk`
/// (it would hold both kinds of slot 3); odd takes it. A kind-only check would hand it to leaf.
#[test]
fn a_borrower_with_its_own_file_in_that_slot_number_is_skipped() {
    let d = tempfile::tempdir().unwrap();
    let mut m = slot3_opened(d.path(), false, true);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 3, Some("landscape/rock01"));
        s.set_borrow("landscape/odd", 3, Some("landscape/rock01"));
    });
    idle(&mut m);
    let sk = key_of(&m, "rock01_sk.png");
    m.remove_member("landscape/rock01", &sk).unwrap();
    idle(&mut m);
    assert_eq!(
        message_starting(&m, "rock01_sk.png"),
        "rock01_sk.png went to landscape/odd, which shared it; 1 other set now shares it from there."
    );
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 3).as_deref(),
        Some("landscape/odd")
    );
}

/// Note JN5: a Delete set that hands two files to one set in one undo step. `landscape/heir` is a set the user
/// created with no files (so it may take files, `drop_target`), and it shares rock01's diffuse and normal; both
/// become its own, and odd, which shared the normal too, shares it from there.
#[test]
fn deleting_a_lender_hands_each_shared_file_to_its_own_heir() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.edit_docs("t", None, true, |_, s| {
        s.create_set("landscape/heir");
        s.set_borrow("landscape/heir", 1, Some("landscape/rock01"));
        s.set_borrow("landscape/heir", 2, Some("landscape/rock01"));
        s.set_borrow("landscape/odd", 2, Some("landscape/rock01"));
    });
    idle(&mut m);
    let (diffuse, normal) = (key_of(&m, "rock01.png"), key_of(&m, "rock01_normal.png"));
    let before = one_step(&mut m, None, |m| {
        m.delete_set_with_toast("landscape/rock01")
    });
    assert_eq!(
        message_starting(&m, "Deleted set"),
        "Deleted set landscape/rock01 (2 textures now in no set). rock01.png went to landscape/heir, which shared \
         it. rock01_normal.png went to landscape/heir, which shared it; 1 other set now shares it from there."
    );
    let views = set_views(&m);
    let heir = view(&views, "landscape/heir");
    assert_eq!(
        heir.slots.get(&SlotKind::Diffuse),
        Some(&SlotFill::Own { item: diffuse })
    );
    assert_eq!(
        heir.slots.get(&SlotKind::Normal),
        Some(&SlotFill::Own { item: normal })
    );
    assert_eq!(borrow_of(&m, "landscape/heir", 1), None);
    assert_eq!(borrow_of(&m, "landscape/heir", 2), None);
    assert_eq!(
        borrow_of(&m, "landscape/odd", 2).as_deref(),
        Some("landscape/heir")
    );
    undo_restores(&mut m, &before);
}

/// Notes JN4, JN5: one Delete set that hands rock01's normal to leaf and leaves odd's share of the packed rmaos
/// empty (a packed map is never handed over): the toast gives the hand-over sentence, then the lost one.
#[test]
fn deleting_a_lender_hands_one_file_over_and_names_the_set_left_empty() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_borrow("landscape/odd", 6, Some("landscape/rock01"));
    });
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    let before = one_step(&mut m, None, |m| {
        m.delete_set_with_toast("landscape/rock01")
    });
    assert_eq!(
        message_starting(&m, "Deleted set"),
        "Deleted set landscape/rock01 (3 textures now in no set). rock01_normal.png went to landscape/leaf, which \
         shared it. landscape/odd shared rock01_rmaos; that slot is empty now."
    );
    assert_eq!(
        view(&set_views(&m), "landscape/leaf")
            .slots
            .get(&SlotKind::Normal),
        Some(&SlotFill::Own { item: normal })
    );
    assert_eq!(
        borrow_of(&m, "landscape/odd", 6).as_deref(),
        Some("landscape/rock01")
    );
    undo_restores(&mut m, &before);
}

/// Note JN5 (B2-8): rock01 holds both kinds of slot 3. Removing the one leaf's share resolves to hands nothing over:
/// rock01 still fills slot 3, leaf keeps its line and now reads the other file, and no toast is shown.
#[test]
fn removing_one_of_two_files_in_a_slot_hands_nothing_over() {
    let d = tempfile::tempdir().unwrap();
    let mut m = slot3_opened(d.path(), true, false);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 3, Some("landscape/rock01"))
    });
    idle(&mut m);
    let shared = |m: &sdc_gui::model::AppModel| {
        view(&set_views(m), "landscape/leaf")
            .slots
            .iter()
            .find_map(|(k, f)| match f {
                SlotFill::Borrowed { item, .. } if k.slot_number() == Some(3) => Some(item.clone()),
                _ => None,
            })
            .expect("leaf shares slot 3")
    };
    let first = shared(&m);
    let name = first.rsplit('/').next().unwrap().to_string();
    m.remove_member("landscape/rock01", &first).unwrap();
    idle(&mut m);
    assert!(m.is_no_set(&first), "nothing handed over");
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 3).as_deref(),
        Some("landscape/rock01")
    );
    assert_ne!(shared(&m), first, "leaf reads rock01's other slot-3 file");
    assert!(
        m.messages.entries().all(|e| !e.text.starts_with(&name)),
        "the slot is not empty, so nothing is said"
    );
}

/// Note JN5 (B2-11 narrowed): rock01 owns its normal AND has a hand-edited share line for slot 2 to leaf; odd shares
/// slot 2 from rock01, so it used rock01's own normal. Deleting rock01 hands that normal to odd as its own file; odd
/// is not pointed at leaf (the B2-11 pass-on is only for a slot the deleted set has no file of its own in).
#[test]
fn deleting_a_lender_with_its_own_file_and_a_share_line_hands_its_own_file() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock01", 2, Some("landscape/leaf"));
        s.set_borrow("landscape/odd", 2, Some("landscape/rock01"));
    });
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    let before = one_step(&mut m, None, |m| {
        m.delete_set_with_toast("landscape/rock01")
    });
    assert!(!m.is_no_set(&normal));
    assert_eq!(
        view(&set_views(&m), "landscape/odd")
            .slots
            .get(&SlotKind::Normal),
        Some(&SlotFill::Own {
            item: normal.clone()
        })
    );
    assert_eq!(
        borrow_of(&m, "landscape/odd", 2),
        None,
        "odd owns the file, no line to leaf"
    );
    undo_restores(&mut m, &before);
}

/// Note JN4 (final review Important 2): rock01 owns its normal and has a hand-edited share line for slot 2 to rock02,
/// so its card shows rock02's normal. When rock02's normal leaves, rock01's own file fills the slot again, so the
/// toast does not call rock01's slot empty.
#[test]
fn removing_a_file_a_set_shares_over_its_own_does_not_call_that_slot_empty() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let normal = key_of(&m, "rock01_normal.png");
    let n02 = key_of(&m, "rock02_n.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_borrow("landscape/rock01", 2, Some("landscape/rock02"));
    });
    idle(&mut m);
    m.remove_member("landscape/rock02", &n02).unwrap();
    idle(&mut m);
    assert!(m.is_no_set(&n02));
    assert_eq!(
        view(&set_views(&m), "landscape/rock01")
            .slots
            .get(&SlotKind::Normal),
        Some(&SlotFill::Own { item: normal })
    );
    let named: Vec<String> = m
        .messages
        .entries()
        .filter(|e| e.text.starts_with("rock02_n.png"))
        .map(|e| e.text.clone())
        .collect();
    assert!(named.is_empty(), "{named:?}");
}

/// Note JN4 (B2-12): odd shares slot 2 from leaf, which shares it from rock01 and has no normal of its own. Clearing
/// leaf's share passes odd on to rock01, in the same undo step and with no toast; leaf's slot is empty.
#[test]
fn clearing_the_middle_of_a_chain_passes_the_share_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_borrow("landscape/odd", 2, Some("landscape/leaf"));
    });
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    let toasts = m.messages.entries().count();
    let before = one_step(&mut m, None, |m| {
        m.set_slot_command("landscape/leaf", 2, SlotCommand::Clear)
            .unwrap()
    });
    assert_eq!(borrow_of(&m, "landscape/leaf", 2), None);
    assert_eq!(
        borrow_of(&m, "landscape/odd", 2).as_deref(),
        Some("landscape/rock01")
    );
    let views = set_views(&m);
    assert_eq!(
        view(&views, "landscape/odd").slots.get(&SlotKind::Normal),
        Some(&SlotFill::Borrowed {
            from: "landscape/rock01".into(),
            item: normal
        })
    );
    assert!(!view(&views, "landscape/leaf")
        .slots
        .contains_key(&SlotKind::Normal));
    assert_eq!(m.messages.entries().count(), toasts, "no toast");
    undo_restores(&mut m, &before);
}

/// Note JN4 (B2-12), a pin: rock01 has its own normal and a share line for slot 2 to rock02, and leaf shares slot 2
/// from rock01, so leaf reads rock01's own file (JN1). Clearing rock01's share passes nothing on: leaf's line still
/// names rock01 and leaf still reads rock01's own normal.
#[test]
fn clearing_the_share_of_a_set_with_its_own_file_passes_nothing_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock02_opened(d.path(), false, true);
    let normal = key_of(&m, "rock01_normal.png");
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/leaf", 2, Some("landscape/rock01"));
        s.set_borrow("landscape/rock01", 2, Some("landscape/rock02"));
    });
    idle(&mut m);
    let before = one_step(&mut m, None, |m| {
        m.set_slot_command("landscape/rock01", 2, SlotCommand::Clear)
            .unwrap()
    });
    assert_eq!(borrow_of(&m, "landscape/rock01", 2), None);
    assert_eq!(
        borrow_of(&m, "landscape/leaf", 2).as_deref(),
        Some("landscape/rock01")
    );
    assert_eq!(
        view(&set_views(&m), "landscape/leaf")
            .slots
            .get(&SlotKind::Normal),
        Some(&SlotFill::Borrowed {
            from: "landscape/rock01".into(),
            item: normal
        })
    );
    let borrowed = borrowed_normal(&m);
    assert_eq!(
        borrowed.get("landscape/leaf").map(String::as_str),
        Some("landscape/rock01")
    );
    assert_eq!(m.request().unwrap().borrowed_normal, borrowed);
    undo_restores(&mut m, &before);
}

/// Note JN5 (t5 Minor 5): rock01 has a hand-edited share line for slot 2 naming itself. When its normal is handed to
/// leaf, that line goes (rock01 would share back the file it gave away); it is not counted among the sets that share
/// the file from leaf.
#[test]
fn a_hand_over_drops_the_lenders_share_line_to_itself() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normal_lent_twice(d.path());
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock01", 2, Some("landscape/rock01"))
    });
    idle(&mut m);
    let normal = key_of(&m, "rock01_normal.png");
    let before = one_step(&mut m, None, |m| {
        m.remove_member("landscape/rock01", &normal).unwrap()
    });
    assert_eq!(
        message_starting(&m, "rock01_normal.png"),
        "rock01_normal.png went to landscape/leaf, which shared it; 1 other set now shares it from there."
    );
    assert_eq!(borrow_of(&m, "landscape/rock01", 2), None);
    assert_eq!(
        view(&set_views(&m), "landscape/leaf")
            .slots
            .get(&SlotKind::Normal),
        Some(&SlotFill::Own { item: normal })
    );
    assert_eq!(
        borrow_of(&m, "landscape/odd", 2).as_deref(),
        Some("landscape/leaf")
    );
    undo_restores(&mut m, &before);
}

/// Note JN5 (t5 Minor 5): the same on Delete set. The lender's entry goes with its self line, and no entry for it
/// comes back from the hand-over.
#[test]
fn deleting_a_lender_with_a_share_line_to_itself_hands_over_and_leaves_no_entry() {
    let d = tempfile::tempdir().unwrap();
    let mut m = normal_lent_twice(d.path());
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock01", 2, Some("landscape/rock01"))
    });
    idle(&mut m);
    let before = one_step(&mut m, None, |m| {
        m.delete_set_with_toast("landscape/rock01")
    });
    assert_eq!(
        message_starting(&m, "Deleted set"),
        "Deleted set landscape/rock01 (3 textures now in no set). rock01_normal.png went to landscape/leaf, which \
         shared it; 1 other set now shares it from there."
    );
    let ids: Vec<String> = m
        .sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .map(|e| e.id.clone())
        .collect();
    assert!(!ids.iter().any(|i| i == "landscape/rock01"), "{ids:?}");
    assert_eq!(
        borrow_of(&m, "landscape/odd", 2).as_deref(),
        Some("landscape/leaf")
    );
    undo_restores(&mut m, &before);
}

/// Note JN1 (t1 Minor 1): a set whose share line names itself keeps its own file, and its card's line says so instead
/// of "(it has none)"; a two-set loop keeps "(it has none)".
#[test]
fn a_share_from_the_set_itself_says_so() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.edit_docs("t", None, true, |_, s| {
        s.set_borrow("landscape/rock01", 2, Some("landscape/rock01"));
        s.set_borrow("landscape/leaf", 2, Some("landscape/odd"));
        s.set_borrow("landscape/odd", 2, Some("landscape/leaf"));
    });
    idle(&mut m);
    let views = set_views(&m);
    let texts =
        |id: &str| -> Vec<String> { view(&views, id).missing.iter().map(Missing::text).collect() };
    assert_eq!(
        texts("landscape/rock01"),
        ["slot2 from landscape/rock01 (this set itself)"]
    );
    assert_eq!(
        texts("landscape/leaf"),
        ["slot2 from landscape/odd (it has none)"]
    );
}

/// Final review Minor 1: after a vanilla → cs switch the first cs plan is requested with the old target's (empty)
/// shared-roughness list. The settle check compares that list too, so the plan on screen is re-planned even when the
/// output paths did not change (here rock01 has a bad `rename`, so it has no output paths at all).
#[test]
fn a_target_switch_replans_when_only_the_shared_roughness_changed() {
    use engine::config::Target;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    let rmaos = m
        .items
        .iter()
        .find(|i| i.name.ends_with("rmaos"))
        .unwrap()
        .key
        .clone();
    m.drop_on_set(&rmaos, &id);
    assert_eq!(
        m.drop_choice(&rmaos, &id, DropChoice::Share),
        DropOutcome::Joined
    );
    m.set_json_value("landscape/rock01", "rename", Some(json!("../outside")));
    idle(&mut m);
    let bake = |m: &sdc_gui::model::AppModel| {
        m.item(&rmaos)
            .and_then(|i| i.job.as_ref())
            .map(|j| j.bake.is_some())
    };
    assert_eq!(bake(&m), Some(false), "a shared rmaos never bakes");
    m.set_target(Target::Vanilla);
    idle(&mut m);
    m.set_target(Target::Cs);
    idle(&mut m);
    assert!(!shared_roughness(&m).is_empty());
    assert_eq!(
        bake(&m),
        Some(false),
        "the plan on screen still bakes the shared rmaos"
    );
}

#[test]
fn shader_dropdown_lists_match_the_mode() {
    use engine::{config::Target, shader::Shader};
    assert_eq!(shader_options(Target::Cs).len(), 15);
    assert!(!shader_options(Target::Vanilla).contains(&Shader::PbrLandscape));
    assert!(!shader_options(Target::Vanilla).contains(&Shader::PbrGrass));
    assert!(!shader_options(Target::Vanilla).contains(&Shader::Pbr));
    // Note AZ (reverses ruling R1): vanilla lists complex grass and complex material.
    assert!(shader_options(Target::Vanilla).contains(&Shader::ComplexGrass));
    assert!(shader_options(Target::Vanilla).contains(&Shader::ComplexMaterial));
    assert_eq!(shader_options(Target::Vanilla).len(), 11);
}

#[test]
fn choosing_and_clearing_a_shader_round_trips_in_sdc_project_toml() {
    use engine::shader::{Shader, ShaderOrigin};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let v = set_views(&m);
    assert_eq!(
        view(&v, "landscape/leaf").shader.as_ref().unwrap().shader,
        Shader::Landscape
    );
    assert!(matches!(
        view(&v, "landscape/leaf")
            .shader
            .as_ref()
            .unwrap()
            .provenance,
        ShaderOrigin::Auto(_)
    ));
    m.set_shader("landscape/leaf", Some(Shader::Hair));
    idle(&mut m);
    let v = set_views(&m);
    let leaf = view(&v, "landscape/leaf");
    assert_eq!(
        (
            leaf.shader_choice,
            leaf.shader.as_ref().unwrap().provenance.clone()
        ),
        (Some(Shader::Hair), ShaderOrigin::Set)
    );
    assert!(m
        .sets_doc
        .as_ref()
        .unwrap()
        .text()
        .contains("shader = \"hair\""));
    assert_eq!(set_shaders(&m).get("landscape/leaf"), Some(&Shader::Hair));
    m.set_shader("landscape/leaf", None);
    idle(&mut m);
    assert!(!m.sets_doc.as_ref().unwrap().text().contains("shader"));
    assert_eq!(view(&set_views(&m), "landscape/leaf").shader_choice, None);
}

#[test]
fn switching_mode_falls_back_and_back() {
    use engine::{
        codes::Code,
        config::Target,
        shader::{Shader, ShaderOrigin},
    };
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.set_shader("landscape/rock01", Some(Shader::Pbr));
    idle(&mut m);
    m.set_target(Target::Vanilla);
    idle(&mut m);
    let v = set_views(&m);
    let rock = view(&v, "landscape/rock01");
    let pick = rock.shader.as_ref().unwrap();
    assert_eq!(
        (pick.shader, &pick.provenance),
        (Shader::Default, &ShaderOrigin::Fallback)
    );
    assert!(rock
        .set_warnings
        .iter()
        .any(|w| w.code == Code::ShaderUnavailable));
    assert_eq!(
        rock.shader_choice,
        Some(Shader::Pbr),
        "the saved choice stays"
    );
    m.set_target(Target::Cs);
    idle(&mut m);
    assert_eq!(
        view(&set_views(&m), "landscape/rock01")
            .shader
            .as_ref()
            .unwrap()
            .shader,
        Shader::Pbr
    );
}

/// A scan database where `tex` (a `textures/…` path with `.dds`) is slot 0 of these shapes:
/// (lighting shader type, SLSF2, shader kind, derived copy, enabled origin).
fn scan_db(db: &std::path::Path, tex: &str, shapes: &[(u32, u32, i32, bool, bool)]) {
    let s = modscan::store::Store::open(db).unwrap();
    let c = s.conn();
    c.execute("INSERT INTO path(id, p) VALUES (1, ?1)", [tex])
        .unwrap();
    c.execute_batch(
        "INSERT INTO texset(id, n, t0) VALUES (1, 1, 1);
         INSERT INTO tex_use(tex_path_id, texset_id, slot) VALUES (1, 1, 0);",
    )
    .unwrap();
    for (i, (ty, slsf2, kind, derived, enabled)) in shapes.iter().enumerate() {
        let id = i as i64 + 2;
        c.execute(
            "INSERT INTO path(id, p) VALUES (?1, ?2)",
            rusqlite::params![id, format!("meshes/m{id}.nif")],
        )
        .unwrap();
        c.execute(
            "INSERT INTO origin(id, kind, name, root, priority, enabled) VALUES (?1, 1, ?2, ?2, ?1, ?3)",
            rusqlite::params![id, format!("mod{id}"), enabled],
        )
        .unwrap();
        c.execute(
            "INSERT INTO nif_copy(id, path_id, origin_id, parse_status, is_derived) VALUES (?1, ?1, ?1, 0, ?2)",
            rusqlite::params![id, derived],
        )
        .unwrap();
        c.execute(
            "INSERT INTO shape(copy_id, block, kind, shader_kind, shader_type, slsf2, texset_id) VALUES (?1, 0, 'BSTriShape', ?2, ?3, ?4, 1)",
            rusqlite::params![id, kind, ty, slsf2],
        )
        .unwrap();
    }
}

/// L7: one read gives each mesh once per fact. One NIF in two enabled mods whose copies differ only in the alpha
/// property: one shader vote, two alpha entries, and Tree_Anim evidence (not set).
#[test]
fn mesh_evidence_counts_each_mesh_once_per_fact() {
    use engine::shader::{MeshAlpha, Shader};
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("scan.db");
    let s = modscan::store::Store::open(&db).unwrap();
    s.conn()
        .execute_batch(
            "INSERT INTO path(id, p) VALUES (1, 'textures/a/wall.dds'), (2, 'meshes/m.nif');
             INSERT INTO texset(id, n, t0) VALUES (1, 1, 1);
             INSERT INTO tex_use(tex_path_id, texset_id, slot) VALUES (1, 1, 0);
             INSERT INTO origin(id, kind, name, root, priority, enabled) VALUES (1, 1, 'A', 'A', 1, 1), (2, 1, 'B', 'B', 2, 1);
             INSERT INTO nif_copy(id, path_id, origin_id, parse_status, is_derived) VALUES (1, 2, 1, 0, 0), (2, 2, 2, 0, 0);
             INSERT INTO shape(copy_id, block, kind, shader_kind, shader_type, slsf2, texset_id, alpha_flags, alpha_threshold) VALUES
               (1, 0, 'BSTriShape', 1, 6, 0, 1, 1, 0), (2, 0, 'BSTriShape', 1, 6, 0, 1, NULL, NULL);",
        )
        .unwrap();
    drop(s);
    let keys = ["textures/a/wall".to_string(), "textures/a/none".to_string()]
        .into_iter()
        .collect();
    let (votes, alpha, tree) = sdc_gui::model::meshes::mesh_evidence(&db, &keys).unwrap();
    assert_eq!(votes["textures/a/wall"], vec![Shader::Hair]);
    // The copy without the property (no alpha), then the blend one: entries sort by flags, NULL first.
    let none = MeshAlpha {
        blend: false,
        test: false,
        threshold: 0,
    };
    let blend = MeshAlpha {
        blend: true,
        ..none
    };
    assert_eq!(alpha["textures/a/wall"], vec![none, blend]);
    assert_eq!(tree.get("textures/a/wall"), Some(&false));
    assert!(!votes.contains_key("textures/a/none"));
    assert!(!alpha.contains_key("textures/a/none"));
    assert!(!tree.contains_key("textures/a/none"));
}

#[test]
fn mesh_votes_count_enabled_source_lighting_shapes_only() {
    use engine::shader::Shader;
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("scan.db");
    scan_db(
        &db,
        "textures/a/wall.dds",
        &[
            (6, 0, 1, false, true),       // votes hair
            (6, 1 << 23, 1, false, true), // TruePBR: no vote
            (0, 0, 2, false, true),       // an effect shader: no vote
            (0, 0, 1, true, true),        // PGPatcher's output copy: no vote
            (0, 0, 1, false, false),      // a disabled mod: no vote
            (4, 0, 1, false, true),       // Face Tint: not one of the 12, no vote
        ],
    );
    let keys = ["textures/a/wall".to_string(), "textures/a/none".to_string()]
        .into_iter()
        .collect();
    let v = sdc_gui::model::meshes::mesh_evidence(&db, &keys).unwrap().0;
    assert_eq!(v.get("textures/a/wall"), Some(&vec![Shader::Hair]));
    assert!(!v.contains_key("textures/a/none"));
}

#[test]
fn a_scan_database_picks_the_set_shader() {
    use engine::shader::{AutoRule, Shader, ShaderOrigin};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    // The key the GUI asks for, taken from the plan: the leaf diffuse's game folder and output stem.
    let j = m
        .items
        .iter()
        .filter_map(|i| i.job.as_ref())
        .find(|j| {
            j.set_key.ends_with("landscape/leaf")
                && j.output.file_stem().is_some_and(|s| s == "leaf")
        })
        .expect("the leaf diffuse job");
    let key = engine::shader::mesh_key(&j.game_path, "leaf");
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    scan_db(&db, &format!("{key}.dds"), &[(6, 0, 1, false, true)]);
    m.scan_db = Some(db);
    m.refresh_mesh_votes(false);
    idle(&mut m);
    let leaf = view(&set_views(&m), "landscape/leaf")
        .shader
        .clone()
        .unwrap();
    assert_eq!(
        (leaf.shader, leaf.provenance),
        (Shader::Hair, ShaderOrigin::Auto(AutoRule::Mesh))
    );
    // No database: the suffix rules again (the leaf is under textures/landscape).
    m.scan_db = None;
    m.refresh_mesh_votes(false);
    idle(&mut m);
    assert_eq!(
        view(&set_views(&m), "landscape/leaf")
            .shader
            .as_ref()
            .unwrap()
            .shader,
        Shader::Landscape
    );
}

/// A scan database where `tex` is slot 0 of these lighting shapes:
/// (shader type, `alpha_flags`, `alpha_threshold`, SLSF2, derived copy). Every origin is enabled.
type AlphaShape = (u32, Option<i64>, Option<i64>, u32, bool);

fn scan_db_alpha(db: &std::path::Path, tex: &str, shapes: &[AlphaShape]) {
    let s = modscan::store::Store::open(db).unwrap();
    let c = s.conn();
    c.execute("INSERT INTO path(id, p) VALUES (1, ?1)", [tex])
        .unwrap();
    c.execute_batch(
        "INSERT INTO texset(id, n, t0) VALUES (1, 1, 1);
         INSERT INTO tex_use(tex_path_id, texset_id, slot) VALUES (1, 1, 0);",
    )
    .unwrap();
    for (i, (ty, flags, threshold, slsf2, derived)) in shapes.iter().enumerate() {
        let id = i as i64 + 2;
        c.execute(
            "INSERT INTO path(id, p) VALUES (?1, ?2)",
            rusqlite::params![id, format!("meshes/m{id}.nif")],
        )
        .unwrap();
        c.execute(
            "INSERT INTO origin(id, kind, name, root, priority, enabled) VALUES (?1, 1, ?2, ?2, ?1, 1)",
            rusqlite::params![id, format!("mod{id}")],
        )
        .unwrap();
        c.execute(
            "INSERT INTO nif_copy(id, path_id, origin_id, parse_status, is_derived) VALUES (?1, ?1, ?1, 0, ?2)",
            rusqlite::params![id, derived],
        )
        .unwrap();
        c.execute(
            "INSERT INTO shape(copy_id, block, kind, shader_kind, shader_type, slsf2, texset_id, alpha_flags, alpha_threshold) VALUES (?1, 0, 'BSTriShape', 1, ?2, ?3, 1, ?4, ?5)",
            rusqlite::params![id, ty, slsf2, flags, threshold],
        )
        .unwrap();
    }
}

/// M6 Track E spec §4.1, §6 (Note U5): one entry per source lighting shape, from NiAlphaProperty bit 0 (blend) and bit
/// 9 (test). No property is no alpha. TruePBR shapes and PGPatcher's output copies are left out; a shader type that
/// is no vote (Face Tint, 4) still counts.
#[test]
fn mesh_alpha_reads_the_alpha_property_of_each_source_lighting_shape() {
    use engine::shader::MeshAlpha;
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("scan.db");
    scan_db_alpha(
        &db,
        "textures/a/wall.dds",
        &[
            (0, Some(1), Some(0), 0, false),
            (0, Some(1 << 9), Some(128), 0, false),
            (4, Some(1 | 1 << 9), Some(90), 0, false),
            (0, None, None, 0, false),
            (0, Some(1 << 9), Some(200), 1 << 23, false),
            (0, Some(1), Some(0), 0, true),
        ],
    );
    let keys = ["textures/a/wall".to_string(), "textures/a/none".to_string()]
        .into_iter()
        .collect();
    let v = sdc_gui::model::meshes::mesh_evidence(&db, &keys).unwrap().1;
    let mut got = v["textures/a/wall"].clone();
    got.sort_by_key(|a| (a.blend, a.test, a.threshold));
    let ma = |blend, test, threshold| MeshAlpha {
        blend,
        test,
        threshold,
    };
    assert_eq!(
        got,
        vec![
            ma(false, false, 0),
            ma(false, true, 128),
            ma(true, false, 0),
            ma(true, true, 90),
        ]
    );
    assert!(!v.contains_key("textures/a/none"));
}

/// M6 Track E spec §1 success, plan Review Focus 3 and 4: the scan's alpha evidence reaches the plan. A refresh that
/// changes only the alpha (a Face Tint shape is no shader vote) still reports a change, and a varying-alpha diffuse
/// is then no longer asked its alpha meaning.
#[test]
fn a_scan_database_decides_a_diffuse_alpha() {
    use engine::config::AlphaPolicy;
    use sdc_gui::model::items::Decision;
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/a");
    std::fs::create_dir_all(&t).unwrap();
    image::ImageBuffer::from_fn(16, 16, |x, _| image::Rgba([90u8, 60, 30, (x * 16) as u8]))
        .save(t.join("glass.png"))
        .unwrap();
    let mut m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let asked = |m: &sdc_gui::model::AppModel| {
        m.item("a/glass.png")
            .unwrap()
            .decisions
            .contains(&Decision::AlphaMeaning)
    };
    assert!(asked(&m));
    let j = m.item("a/glass.png").unwrap().job.clone().unwrap();
    let key = engine::shader::mesh_key(&j.game_path, "glass");
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    scan_db_alpha(
        &db,
        &format!("{key}.dds"),
        &[(4, Some(1), Some(0), 0, false)],
    );
    m.scan_db = Some(db);
    m.refresh_mesh_votes(false);
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.mesh_votes.is_empty(), "Face Tint is no shader vote");
    let j = m.item("a/glass.png").unwrap().job.clone().unwrap();
    // Note FC1 (R1): the meshes' verdict is then recorded in `sdc.toml`, so it plans as the project's.
    assert_eq!(
        (
            j.profile.alpha.policy,
            j.provenance["alpha.policy"].to_string()
        ),
        (AlphaPolicy::Blend, "project_file".to_string())
    );
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value("a/glass.png", "alpha.policy"),
        Some(serde_json::json!("blend"))
    );
    assert!(!asked(&m));
}

/// F1 spec Review Focus 1: match1 comes from the source, so a set rename does not move it off the mesh's path.
#[test]
fn a_renamed_set_still_matches_its_source_path() {
    use sdc_gui::model::pbr_export::json_entries;
    use sdc_gui::pgp::writer::entry_node;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    m.rename_set("landscape/rock01", "landscape/boulder")
        .unwrap();
    idle(&mut m);
    let (_, e) = json_entries(&m)
        .into_iter()
        .find(|(v, _)| v.id == "landscape/boulder")
        .expect("the renamed set is a PBR set");
    assert_eq!(
        entry_node(&e).to_value()["match1"],
        json!(r"landscape\rock01.dds")
    );
}

/// F1 spec §3.2, owner 2026-10-12: the Matching rows are the active matches and the empty + rows; the + menu offers
/// slots 3–8 without a row, a filled slot starting from the set's path and any other empty.
#[test]
fn the_matching_rows_and_the_plus_menu_follow_the_filled_slots() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let (active, add) = slot_matches(view(&set_views(&m), "landscape/rock01"));
    assert_eq!(
        active,
        vec![SlotMatch {
            n: 1,
            slot: "Diffuse".into(),
            path: r"landscape\rock01".into()
        }]
    );
    let slot = |n: u8, slot: &str, path: &str| SlotMatch {
        n,
        slot: slot.into(),
        path: path.into(),
    };
    assert_eq!(
        add,
        vec![
            slot(3, "Slot 3", ""),
            slot(4, "Slot 4", ""),
            slot(5, "Slot 5", ""),
            slot(6, "RMAOS", r"landscape\rock01_rmaos"),
            slot(7, "Slot 7", ""),
            slot(8, "Slot 8", ""),
        ]
    );
    m.set_json_value(
        "landscape/rock01",
        "match6",
        Some(json!(r"landscape\rock01_rmaos")),
    );
    m.set_json_value("landscape/rock01", "match3", Some(json!("")));
    m.set_json_value("landscape/rock01", "match1", Some(json!("")));
    idle(&mut m);
    let (active, add) = slot_matches(view(&set_views(&m), "landscape/rock01"));
    assert_eq!(
        active,
        vec![
            slot(3, "Slot 3", ""),
            slot(6, "RMAOS", r"landscape\rock01_rmaos")
        ],
        "an empty + row stays a row; match1 = \"\" (diffuse off) is not one"
    );
    assert_eq!(
        add.iter().map(|a| a.n).collect::<Vec<_>>(),
        vec![4, 5, 7, 8]
    );
}

/// F1 spec §5, DECISIONS V8: a PBR set's cards are the PBR slots in PGPatcher order 1–8, empty or not.
#[test]
fn a_pbr_set_shows_its_pbr_slots_in_slot_order() {
    use engine::shader::Shader;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_without_rmaos(d.path());
    idle(&mut m);
    let v = view(&set_views(&m), "landscape/rock01").clone();
    assert_eq!(v.shader.as_ref().map(|p| p.shader), Some(Shader::Pbr));
    let row = slot_row(v.shader.as_ref(), &v.slots);
    assert_eq!(
        row.iter().map(|c| c.kind).collect::<Vec<_>>(),
        vec![
            SlotKind::Diffuse,
            SlotKind::Normal,
            SlotKind::Emissive,
            SlotKind::Height,
            SlotKind::Rmaos,
            SlotKind::Coat,
            SlotKind::Subsurface,
        ]
    );
    assert!(row.iter().all(|c| c.usable));
    let set = |k: &[SlotKind]| k.iter().copied().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        usable_slots(Shader::AdvancedSkin),
        set(&[
            SlotKind::Diffuse,
            SlotKind::Normal,
            SlotKind::SkinSk,
            SlotKind::Rmaos,
            SlotKind::SkinWet,
            SlotKind::SkinDetail
        ])
    );
    assert_eq!(
        usable_slots(Shader::Glow),
        set(&[SlotKind::Diffuse, SlotKind::Normal, SlotKind::Emissive])
    );
    assert_eq!(
        usable_slots(Shader::Eye),
        set(&[SlotKind::Diffuse, SlotKind::Normal])
    );
}

/// F1 spec §5, Review Focus 4: a SHADER-UNAVAILABLE fallback (default in place of pbr) keeps the filled RMAOS
/// visible, in slot order and marked; with no pick, only the filled slots show.
#[test]
fn a_fallback_shader_keeps_filled_slots_it_cannot_use() {
    use engine::shader::{Shader, ShaderOrigin, ShaderPick};
    use std::collections::BTreeMap;
    let own = |i: &str| SlotFill::Own { item: i.into() };
    let slots: BTreeMap<SlotKind, SlotFill> =
        [(SlotKind::Rmaos, own("r")), (SlotKind::Diffuse, own("d"))]
            .into_iter()
            .collect();
    let fallback = ShaderPick {
        shader: Shader::Default,
        provenance: ShaderOrigin::Fallback,
        wanted: Some(Shader::Pbr),
    };
    let row = slot_row(Some(&fallback), &slots);
    assert_eq!(
        row,
        vec![
            SlotCell {
                kind: SlotKind::Diffuse,
                fill: own("d"),
                usable: true,
                clash: None
            },
            SlotCell {
                kind: SlotKind::Normal,
                fill: SlotFill::Empty,
                usable: true,
                clash: None
            },
            SlotCell {
                kind: SlotKind::Rmaos,
                fill: own("r"),
                usable: false,
                clash: None
            },
        ]
    );
    let none = slot_row(None, &slots);
    assert_eq!(
        none.iter().map(|c| (c.kind, c.usable)).collect::<Vec<_>>(),
        vec![(SlotKind::Diffuse, true), (SlotKind::Rmaos, true)]
    );
}

/// Controller ruling (final fix wave): + prefills slot N only when PGPatcher's slot-N list strips the member's
/// suffix, so a prefilled row never splits the prefix. Subsurface `_s` in slot 8 is not in that list (`_b`/`_bl`/
/// `_flow`): it gets an empty row instead. Height `_p` in slot 4 is stripped, so it is prefilled.
#[test]
fn the_plus_menu_prefills_only_suffixes_pgpatcher_strips() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let mut v = view(&set_views(&m), "landscape/rock01").clone();
    for k in [SlotKind::Height, SlotKind::Subsurface] {
        v.slots.insert(
            k,
            SlotFill::Own {
                item: "landscape/x.png".into(),
            },
        );
    }
    let (_, add) = slot_matches(&v);
    let path = |n: u8| add.iter().find(|a| a.n == n).map(|a| a.path.clone());
    assert_eq!(path(4), Some(r"landscape\rock01_p".into()));
    assert_eq!(path(6), Some(r"landscape\rock01_rmaos".into()));
    assert_eq!(path(8), Some(String::new()), "slot 8 does not strip _s");
}

#[test]
fn the_scan_database_is_read_off_the_ui_thread_and_a_read_error_keeps_the_values() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let j = m
        .items
        .iter()
        .filter_map(|i| i.job.as_ref())
        .find(|j| {
            j.set_key.ends_with("landscape/leaf")
                && j.output.file_stem().is_some_and(|s| s == "leaf")
        })
        .expect("the leaf diffuse job");
    let key = engine::shader::mesh_key(&j.game_path, "leaf");
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    scan_db(&db, &format!("{key}.dds"), &[(6, 0, 1, false, true)]);
    m.scan_db = Some(db.clone());
    m.refresh_mesh_votes(false);
    // The read is on a worker: nothing is applied until the model polls, and the model reports busy meanwhile.
    assert!(m.mesh_votes.is_empty() && m.busy());
    idle(&mut m);
    assert!(!m.mesh_votes.is_empty(), "the result landed in poll");
    // A database that cannot be read keeps the previous evidence (and logs to stderr), it does not clear it.
    let before = m.mesh_votes.clone();
    std::fs::write(&db, b"not a database").unwrap();
    m.refresh_mesh_votes(false);
    idle(&mut m);
    assert_eq!(m.mesh_votes, before);
}

/// A model with a leaf mesh vote in a scan database, planned again until its post-plan read is pending.
fn pending_read(d: &std::path::Path) -> (sdc_gui::model::AppModel, tempfile::TempDir) {
    let mut m = pbr_opened(d);
    idle(&mut m);
    let j = m
        .items
        .iter()
        .filter_map(|i| i.job.as_ref())
        .find(|j| {
            j.set_key.ends_with("landscape/leaf")
                && j.output.file_stem().is_some_and(|s| s == "leaf")
        })
        .expect("the leaf diffuse job");
    let key = engine::shader::mesh_key(&j.game_path, "leaf");
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    scan_db(&db, &format!("{key}.dds"), &[(6, 0, 1, false, true)]);
    m.scan_db = Some(db);
    m.replan();
    let end = std::time::Instant::now() + Duration::from_secs(60);
    while !m.votes_pending() {
        assert!(std::time::Instant::now() < end, "the plan never landed");
        m.poll();
    }
    assert!(m.mesh_votes.is_empty());
    (m, dbdir)
}

fn export_started_or_asked(m: &sdc_gui::model::AppModel) -> bool {
    m.dialog.is_some()
        || m.review.is_some()
        || m.export.running.is_some()
        || m.export.summary.is_some()
}

#[test]
fn a_queued_export_fires_once_the_post_plan_read_lands() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _db) = pending_read(d.path());
    m.export_clicked();
    assert!(!export_started_or_asked(&m), "waits for the read");
    idle(&mut m);
    assert!(!m.mesh_votes.is_empty());
    assert!(export_started_or_asked(&m), "the tail of the plan ran");
}

/// Trace-debug 2026-10-22 #4 (Note CW1): a scan that starts while a plan's post-plan read is pending keeps the evidence
/// held and still finishes the plan (a queued export starts).
#[test]
fn a_scan_start_during_the_post_plan_read_keeps_the_evidence_and_finishes_the_plan() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _db) = pending_read(d.path());
    m.gras_diffuses = ["keep".to_string()].into();
    m.export_clicked();
    assert!(!export_started_or_asked(&m), "waits for the read");
    m.scan_started();
    assert!(!m.votes_pending(), "the read was re-issued and landed");
    idle(&mut m);
    assert!(m.gras_diffuses.contains("keep"), "the evidence was emptied");
    assert!(
        m.mesh_votes.is_empty(),
        "the real read must not land after the scan start"
    );
    assert!(export_started_or_asked(&m), "the tail of the plan ran");
}
#[test]
fn a_scan_finished_refresh_keeps_the_pending_plan_continuation() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _db) = pending_read(d.path());
    m.export_clicked();
    m.refresh_mesh_votes(false);
    idle(&mut m);
    assert!(export_started_or_asked(&m), "the plan still finished");
}

#[test]
fn an_edit_during_a_pending_read_drops_the_stale_read() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _db) = pending_read(d.path());
    m.export_clicked();
    m.replan();
    assert!(!m.votes_pending(), "the stale read was cleared");
    m.poll();
    assert!(m.mesh_votes.is_empty(), "its evidence is never applied");
    assert!(!export_started_or_asked(&m), "no finish on the old plan");
    idle(&mut m);
    assert!(!m.mesh_votes.is_empty(), "the new plan read again");
    assert!(export_started_or_asked(&m));
}

#[test]
fn the_json_panel_and_its_errors_belong_to_json_sets_only() {
    use engine::config::Target;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    assert_eq!(m.target, Target::Cs);
    let plain = view(&set_views(&m), "landscape/rock01").clone();
    assert!(!plain.pbr && !plain.skin);
    assert!(!shows_json_panel(&plain, Target::Cs));
    // Existing JSON settings are never hidden.
    let mut with_json = plain.clone();
    with_json.settings.insert("slot1".into(), json!("x"));
    assert!(shows_json_panel(&with_json, Target::Cs));
    // A PBR set shows it on cs only.
    let d = tempfile::tempdir().unwrap();
    m = pbr_opened(d.path());
    let pbr = view(&set_views(&m), "landscape/rock01").clone();
    assert!(pbr.pbr && shows_json_panel(&pbr, Target::Cs));
    assert!(!shows_json_panel(&pbr, Target::Vanilla));
}

#[test]
fn a_grass_diffuse_card_is_titled_complex_grass() {
    use engine::shader::Shader;
    assert_eq!(
        card_title(SlotKind::Diffuse, "a.png", Some(Shader::ComplexGrass)),
        "Complex grass"
    );
    assert_eq!(
        card_title(SlotKind::Diffuse, "a.png", Some(Shader::Landscape)),
        "Diffuse"
    );
    assert_eq!(
        card_title(SlotKind::Normal, "a.png", Some(Shader::ComplexGrass)),
        "Normal _n"
    );
}

#[test]
fn a_bad_prefix_on_a_non_json_set_is_no_export_block() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_json_value("landscape/rock01", "rename", Some(json!("../b")));
    idle(&mut m);
    let v = view(&set_views(&m), "landscape/rock01").clone();
    assert!(!v.pbr && v.prefix_error.is_some(), "{v:?}");
    assert!(sdc_gui::model::pbr_export::export_blocks(&m)
        .iter()
        .all(|b| !b.text.contains("rename")));
}

/// Round 3 review: the settings Format row writes the tag with the format, so its reset clears both.
#[test]
fn resetting_format_also_clears_the_tag_it_wrote() {
    use engine::config::{ColorTag, Format};
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let key = key_of(&m, "rock01_rmaos");
    m.set_format(&key, Format::Bc1, ColorTag::Unorm);
    idle(&mut m);
    let p = m.project.as_ref().unwrap();
    assert!(p.file_value(&key, "format").is_some() && p.file_value(&key, "color.tag").is_some());
    m.clear_field(&key, "format");
    idle(&mut m);
    let p = m.project.as_ref().unwrap();
    assert!(p.file_value(&key, "format").is_none());
    assert!(
        p.file_value(&key, "color.tag").is_none(),
        "tag outlived the format reset"
    );
}

/// Audit 2026-10-13b E-I1 (DECISIONS AF4): Export clicked during a scan-finished read (not a plan's own) starts
/// when that read lands, with unchanged evidence and with a read error; it never waits for an unrelated plan.
#[test]
fn an_export_clicked_during_a_scan_finished_read_starts_when_it_lands() {
    for broken in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let (mut m, db) = pending_read(d.path());
        idle(&mut m);
        assert!(!m.mesh_votes.is_empty());
        if broken {
            let bad = db.path().join("broken.db");
            std::fs::write(&bad, b"not a database").unwrap();
            m.scan_db = Some(bad);
        }
        m.refresh_mesh_votes(false);
        assert!(m.votes_pending(), "broken={broken}");
        m.export_clicked();
        assert!(!export_started_or_asked(&m), "waits for the read");
        idle(&mut m);
        assert!(
            export_started_or_asked(&m),
            "broken={broken}: started when the read landed"
        );
    }
}

/// Review round 1 M5 (DECISIONS AF4): an edit that supersedes the scan-finished read the export was waiting for
/// drops the queued export; the edit's plan never starts it unasked.
#[test]
fn an_edit_during_a_scan_finished_read_drops_the_queued_export() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _db) = pending_read(d.path());
    idle(&mut m);
    m.refresh_mesh_votes(false);
    m.export_clicked();
    m.set_field("landscape/rock01.png", "header", json!("dx10"));
    idle(&mut m);
    assert!(!export_started_or_asked(&m), "{:?}", m.dialog);
    assert!(
        m.status_line
            .contains("Export cancelled: the project changed while scan results were loading"),
        "{}",
        m.status_line
    );
    assert!(m
        .messages
        .entries()
        .any(|e| e.text.starts_with("Export cancelled: the project changed")));
}

/// Fix round 2 (DECISIONS AF4): a re-plan that is no edit keeps an Export
/// queued behind a scan-finished read; that plan carries it to its end.
#[test]
fn a_non_edit_replan_during_a_scan_finished_read_keeps_the_queued_export() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _db) = pending_read(d.path());
    idle(&mut m);
    m.refresh_mesh_votes(false);
    m.export_clicked();
    assert!(m.votes_pending() && !export_started_or_asked(&m));
    // A re-plan that is no edit runs while the read is pending.
    m.replan();
    idle(&mut m);
    assert!(export_started_or_asked(&m), "the re-plan kept the export");
}

/// Fix round 3 (DECISIONS AF11): an undo changes the project like an edit, so it cancels an Export queued behind a
/// scan-finished read, with the note.
#[test]
fn an_undo_during_a_scan_finished_read_cancels_the_queued_export() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _db) = pending_read(d.path());
    idle(&mut m);
    m.set_field("landscape/rock01.png", "header", json!("dx10"));
    idle(&mut m);
    m.refresh_mesh_votes(false);
    m.export_clicked();
    m.undo();
    idle(&mut m);
    assert!(!export_started_or_asked(&m), "{:?}", m.dialog);
    assert!(
        m.status_line
            .contains("Export cancelled: the project changed while scan results were loading"),
        "{}",
        m.status_line
    );
}

/// Note AO (owner, 2026-10-15): one card per PGPatcher slot, at most 8. `_cnr` and `_f` share the slot 7 card, which
/// holds whichever is there; both at once is the slot's conflict, shown on the card.
#[test]
fn one_card_per_pgpatcher_slot_and_the_slot_7_card_holds_cnr_or_f() {
    use engine::shader::{Shader, ShaderOrigin, ShaderPick};
    use std::collections::BTreeMap;
    let own = |i: &str| SlotFill::Own { item: i.into() };
    let pbr = ShaderPick {
        shader: Shader::Pbr,
        provenance: ShaderOrigin::Fallback,
        wanted: None,
    };
    let fuzz: BTreeMap<SlotKind, SlotFill> = [(SlotKind::Fuzz, own("f"))].into_iter().collect();
    let row = slot_row(Some(&pbr), &fuzz);
    let seven: Vec<_> = row
        .iter()
        .filter(|c| c.kind.slot_number() == Some(7))
        .collect();
    assert_eq!(seven.len(), 1);
    assert_eq!(
        (seven[0].kind, &seven[0].fill, seven[0].clash),
        (SlotKind::Fuzz, &own("f"), None)
    );
    let both: BTreeMap<SlotKind, SlotFill> = SlotKind::ALL
        .into_iter()
        .map(|k| (k, own(&format!("{k:?}"))))
        .collect();
    for pick in [None, Some(&pbr)] {
        let row = slot_row(pick, &both);
        // One card per PGPatcher slot, plus the two XPMF maps (Note GI) and the two skin members (slots 9, 10, Task
        // 4b), each a card of its own.
        let numbered = row.iter().filter(|c| c.kind.game_slot().is_some());
        assert!(numbered.count() <= 8, "{} cards", row.len());
        let xpmf: Vec<_> = row
            .iter()
            .filter(|c| c.kind.slot_number().is_none())
            .map(|c| (c.kind, c.clash))
            .collect();
        assert_eq!(
            xpmf,
            [
                (SlotKind::ProjectedNoise, None),
                (SlotKind::DetailNormal, None)
            ]
        );
        let seven = row
            .iter()
            .find(|c| c.kind.slot_number() == Some(7))
            .unwrap();
        assert_eq!(
            (seven.kind, seven.clash),
            (SlotKind::Coat, Some(SlotKind::Fuzz))
        );
        let three = row
            .iter()
            .find(|c| c.kind.slot_number() == Some(3))
            .unwrap();
        assert!(three.clash.is_some());
        assert!(slot_clash_text(seven).unwrap().contains("slot 7"));
    }
    assert_eq!(slot_label(SlotKind::Fuzz, "x/rock01_f.png"), SLOT7_CARD);
    assert_eq!(slot_label(SlotKind::Coat, ""), SLOT7_CARD);
}

/// Review fix: an empty slot 3 card is titled as slot 7's is, and "From another set" names both kinds of the slot.
#[test]
fn slot_3_and_7_cards_name_both_kinds() {
    assert_eq!(slot_label(SlotKind::Emissive, ""), SLOT3_CARD);
    assert_eq!(SLOT3_CARD, "Emissive / Skin (slot 3)");
    assert_eq!(
        slot_label(SlotKind::Emissive, "x/rock01_g.png"),
        "Emissive _g"
    );
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    for (kind, both) in [
        (SlotKind::Coat, "Coat _cnr or Fuzz _f"),
        (SlotKind::Emissive, "Emissive _g or Skin _sk"),
    ] {
        let menu = m.slot_menu("landscape/rock01", kind);
        let e = menu.iter().find(|e| e.label == "From another set").unwrap();
        let why = e.disabled.clone().unwrap_or_default();
        assert!(why.contains(both), "{why}");
    }
}

/// Audit 2026-10-15 item 8: redo of a Duplicate never writes over a file at the copy's path, and records the new
/// copy's hash so undo deletes it again even when the source changed in between.
#[test]
fn redo_of_a_duplicate_refuses_an_occupied_path_and_records_the_new_hash() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let odd = key_of(&m, "odd.png");
    let id = m.drop_new_set(&odd).unwrap();
    idle(&mut m);
    let rough = key_of(&m, "rock01_roughness.png");
    m.drop_on_set(&rough, &id);
    assert_eq!(
        m.drop_choice(&rough, &id, DropChoice::Duplicate),
        DropOutcome::Joined
    );
    let root = m.root.clone().unwrap();
    let base = id.rsplit('/').next().unwrap().to_string();
    let copy = root.join(format!("landscape/{base}_roughness.png"));
    let src = root.join("landscape/rock01_roughness.png");
    m.undo();
    std::fs::write(&copy, b"someone else").unwrap();
    m.redo();
    assert!(matches!(m.dialog, Some(sdc_gui::model::Dialog::Error(_))));
    assert!(m.history.can_redo(), "the step stays to redo");
    assert_eq!(std::fs::read(&copy).unwrap(), b"someone else");
    m.dialog = None;
    std::fs::remove_file(&copy).unwrap();
    let mut bytes = std::fs::read(&src).unwrap();
    bytes.extend_from_slice(b"trailing");
    std::fs::write(&src, bytes).unwrap();
    m.redo();
    assert!(m.dialog.is_none(), "{:?}", m.dialog);
    assert!(copy.exists());
    m.undo();
    assert!(m.dialog.is_none(), "{:?}", m.dialog);
    assert!(!copy.exists(), "the copy made by redo is deleted by undo");
}

/// Trace-debug 2026-10-16 #39: Duplicate into set skips a name another file is read as (its `[inputs]` alias), as
/// the rename code does; the engine would refuse that alias next to a real file of the same name.
#[test]
fn duplicate_skips_a_name_another_file_reads_as() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let root = m.root.clone().unwrap();
    let leaf = key_of(&m, "leaf.png");
    let rough = key_of(&m, "rock01_roughness.png");
    m.edit_docs("alias", None, true, |p, _| {
        p.set_read_as(&leaf, Some("landscape/odd_roughness.png"))
    });
    idle(&mut m);
    let _ = m.drop_replace(&rough, "landscape/odd", DropChoice::Duplicate);
    assert!(!root.join("landscape/odd_roughness.png").exists());
    assert!(root.join("landscape/odd-2_roughness.png").exists());
}

/// Trace-debug #55: Move down on the last set (no `[project].order` yet) is no edit at all.
#[test]
fn move_down_on_last_set_changes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let views = set_views(&m);
    let last = views.last().unwrap().id.clone();
    let sets_dirty = |m: &sdc_gui::model::AppModel| m.sets_doc.as_ref().unwrap().is_dirty();
    let (undo, dirty) = (m.history.undo.len(), sets_dirty(&m));
    assert!(!m.reorder_set(&last, views.len()));
    assert_eq!((m.history.undo.len(), sets_dirty(&m)), (undo, dirty));
    assert!(!dirty, "sdc-project.toml is untouched");
    assert!(m.reorder_set(&last, 0), "a real move still moves");
}

/// Audit 08c-3: while the re-plan after an export folder change runs, set ids still strip the folder the current
/// sets were planned with, so an edit made then lands on the right set.
#[test]
fn set_ids_stay_right_while_a_new_export_folder_replans() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    idle(&mut m);
    let before: Vec<String> = set_views(&m).iter().map(|v| v.id.clone()).collect();
    assert!(
        before.iter().any(|id| id == "landscape/rock01"),
        "{before:?}"
    );
    m.set_export_folder(d.path().join("elsewhere"));
    let during: Vec<String> = set_views(&m).iter().map(|v| v.id.clone()).collect();
    assert_eq!(during, before);
    idle(&mut m);
    let after: Vec<String> = set_views(&m).iter().map(|v| v.id.clone()).collect();
    assert_eq!(after, before);
}

/// Layout v2 Task 1.3: the header's Export and Review's Export, both queued behind one scan-finished read, are taken
/// together when it lands; neither is left to fire on a later, unrelated plan.
#[test]
fn both_exports_queued_behind_a_scan_finished_read_are_taken_when_it_lands() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _db) = pending_read(d.path());
    idle(&mut m);
    // The fixture has undecided textures: Export opens Review.
    m.export_clicked();
    assert!(m.review.is_some());
    m.refresh_mesh_votes(false);
    assert!(m.votes_pending());
    m.export_from_review();
    m.export_clicked();
    idle(&mut m);
    assert!(!m.export_queued());
    assert!(m.export.summary.is_none() && m.export.running.is_none());
    m.set_field("landscape/rock01.png", "mipmaps.count", json!(3));
    idle(&mut m);
    assert!(
        m.export.summary.is_none() && m.export.running.is_none(),
        "a later plan started the stranded Review export"
    );
}

/// Fix round 1 (Task 1.3): only Review's Export queued behind a scan-finished read starts when the read lands, once.
#[test]
fn review_export_queued_behind_a_scan_finished_read_starts_when_it_lands() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _db) = pending_read(d.path());
    idle(&mut m);
    m.export_clicked();
    assert!(m.review.is_some());
    m.refresh_mesh_votes(false);
    assert!(m.votes_pending());
    m.export_from_review();
    assert!(m.export_queued() && m.export.running.is_none());
    idle(&mut m);
    assert!(!m.export_queued());
    assert!(m.export.summary.is_some(), "started when the read landed");
    m.export.summary = None;
    m.set_field("landscape/rock01.png", "mipmaps.count", json!(3));
    idle(&mut m);
    assert!(m.export.summary.is_none() && m.export.running.is_none());
}

/// v2 Generate from normal map, Task 2 (Note CO, reconciliation S8): a generated `_p` fills the Height slot until a
/// real `_p` replaces it; the replaced item keeps its GEN-REPLACED Info warning (preflight P4), never a Problems row.
#[test]
fn a_generated_p_fills_the_height_slot_until_a_real_p_replaces_it() {
    use engine::codes::{Code, Severity};
    use image::{ImageBuffer, Luma};
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    let grey = |name: &str, v: u8| {
        ImageBuffer::from_fn(64, 64, |x, _| Luma([v.wrapping_add(x as u8)]))
            .save(r.join(name))
            .unwrap()
    };
    grey("rock01_p_generated.png", 60);
    let mut m = opened(d.path());
    let generated = key_of(&m, "rock01_p_generated.png");
    let v = set_views(&m);
    assert_eq!(
        view(&v, "landscape/rock01").slots.get(&SlotKind::Height),
        Some(&SlotFill::Own {
            item: generated.clone()
        })
    );
    grey("rock01_p.png", 90);
    m.reload_changed();
    idle(&mut m);
    let real = key_of(&m, "rock01_p.png");
    let v = set_views(&m);
    let rock = view(&v, "landscape/rock01");
    assert_eq!(
        rock.slots.get(&SlotKind::Height),
        Some(&SlotFill::Own { item: real })
    );
    assert!(
        !rock
            .slots
            .values()
            .any(|f| matches!(f, SlotFill::Own { item } if *item == generated)),
        "{:?}",
        rock.slots
    );
    let it = m.item(&generated).unwrap();
    assert_eq!(it.set_key.as_deref(), Some(rock.key.as_str()));
    assert_eq!(it.warnings.len(), 1, "{:?}", it.warnings);
    assert_eq!(it.warnings[0].code, Code::GenReplaced);
    assert_eq!(it.warnings[0].severity, Severity::Info);
    assert_eq!(
        it.warnings[0].message,
        "generated rock01_p_generated.png replaced by rock01_p.png"
    );
    assert!(!sdc_gui::model::bottom::problems(&m)
        .iter()
        .any(|p| format!("{p:?}").contains("GEN-REPLACED")));
}

/// Note FI2: a set of DDS files is a set: listed, and "Not in a set" is about names only.
#[test]
fn an_all_dds_folder_shows_its_set() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    bc1_dds(&root.join("rock/rock.dds"));
    bc1_dds(&root.join("rock/rock_n.dds"));
    let m = opened_as_the_app(root, d.path().join("export"));
    assert_eq!(m.sets.len(), 1, "{:?}", m.sets);
    assert_eq!(m.sets[0].members.len(), 2);
    assert_eq!(m.not_in_set_count(), 0);
    // The settings panel's `← set` link shows when the item's set key is a listed set (settings_panel.rs).
    let it = m.item("rock/rock.dds").expect("the DDS item");
    let sk = it.set_key.clone().expect("a set key");
    assert!(m.sets.iter().any(|s| s.key == sk));
}

fn png64(path: &std::path::Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| image::Rgb([x as u8 * 4, y as u8 * 4, 90u8]))
        .save(path)
        .unwrap();
}

#[test]
fn output_game_paths_lists_the_folders_textures() {
    let d = tempfile::tempdir().unwrap();
    png64(&d.path().join("textures/pbr/xmead/Ale_BC.png"));
    bc1_dds(&d.path().join("textures/pbr/xmead/ale_bc_n.dds"));
    let mut m = opened_as_the_app(d.path().to_path_buf(), d.path().join("export"));
    let (rev, paths) = m.output_game_paths();
    assert_eq!(
        paths,
        [
            "textures/pbr/xmead/ale_bc.dds",
            "textures/pbr/xmead/ale_bc_n.dds"
        ]
    );
    // A re-plan with no change keeps the revision.
    m.replan();
    idle(&mut m);
    assert_eq!(m.output_game_paths().0, rev);
    // Adding a file bumps it.
    png64(&d.path().join("textures/pbr/xmead/Ale_Extra.png"));
    m.replan();
    idle(&mut m);
    let (rev2, paths) = m.output_game_paths();
    assert!(rev2 > rev);
    assert_eq!(paths.len(), 3, "{paths:?}");
}

#[test]
fn output_game_paths_leaves_out_a_path_with_no_textures_component() {
    let d = tempfile::tempdir().unwrap();
    // `loose.png` outputs at the export root; its sibling under `textures/` must still be listed.
    png64(&d.path().join("loose.png"));
    png64(&d.path().join("textures/c.png"));
    let m = opened_as_the_app(d.path().to_path_buf(), d.path().join("export"));
    assert!(m.item("loose.png").is_some_and(|i| i.job.is_some()));
    assert_eq!(m.output_game_paths().1, ["textures/c.dds"]);
}

#[test]
fn output_game_paths_starts_at_textures_after_a_prefix() {
    let d = tempfile::tempdir().unwrap();
    png64(&d.path().join("mymod/textures/x.png"));
    let m = opened_as_the_app(d.path().to_path_buf(), d.path().join("export"));
    assert_eq!(m.output_game_paths().1, ["textures/x.dds"]);
}

#[test]
fn output_game_paths_dedups_two_inputs_with_one_output() {
    let d = tempfile::tempdir().unwrap();
    png64(&d.path().join("textures/Ale_BC.png"));
    bc1_dds(&d.path().join("textures/ale_bc.dds"));
    let m = opened_as_the_app(d.path().to_path_buf(), d.path().join("export"));
    let both = m
        .items
        .iter()
        .filter(|i| i.job_key == "textures/ale_bc.dds")
        .count();
    assert_eq!(both, 2, "the fixture must give two inputs one output");
    assert_eq!(m.output_game_paths().1, ["textures/ale_bc.dds"]);
}

#[test]
fn output_game_paths_includes_a_skipped_dds() {
    let d = tempfile::tempdir().unwrap();
    bc1_dds(&d.path().join("textures/Rock/Rock.dds"));
    let m = opened_as_the_app(d.path().to_path_buf(), d.path().join("export"));
    assert!(m
        .item("textures/Rock/Rock.dds")
        .is_some_and(|i| i.job.is_none()));
    assert_eq!(m.output_game_paths().1, ["textures/rock/rock.dds"]);
}

/// The `pbr_opened_plain` set with no RMAOS: its roughness and AO sources are gone, so it lacks a map.
fn pbr_without_rmaos(dir: &std::path::Path) -> sdc_gui::model::AppModel {
    let mut m = pbr_opened_plain(dir);
    for n in ["rock01_roughness.png", "rock01_ao.png"] {
        std::fs::remove_file(dir.join("textures/landscape").join(n)).unwrap();
    }
    m.replan();
    idle(&mut m);
    m
}
/// The labels of the missing-map warnings the view of `id` carries.
fn missing_maps(m: &sdc_gui::model::AppModel, id: &str) -> Vec<String> {
    use engine::codes::Code;
    view(&set_views(m), id)
        .set_warnings
        .iter()
        .filter(|w| w.code == Code::ShaderMissingMap)
        .map(|w| w.data["map"].as_str().unwrap_or_default().to_string())
        .collect()
}

fn supplying(path: &str, bits: u16) -> sdc_gui::model::known_paths::KnownPaths {
    sdc_gui::model::known_paths::KnownPaths {
        supplies: [(path.to_string(), bits)].into(),
        ..Default::default()
    }
}

/// Note HE4: a map a mesh or TXST fills beside the set's diffuse is not "missing". `set_known` alone takes the
/// warning away (no new plan), from the set and from the Problems list; a map nobody supplies still warns.
#[test]
fn a_map_a_mesh_supplies_raises_no_missing_map() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_without_rmaos(d.path());
    idle(&mut m);
    let rock = "textures/landscape/rock01.dds";
    assert_eq!(missing_maps(&m, "landscape/rock01"), ["_rmaos"]);
    let rows = |m: &sdc_gui::model::AppModel| {
        sdc_gui::model::bottom::problems(m)
            .iter()
            .filter(|p| p.text.contains("needs _rmaos"))
            .count()
    };
    assert_eq!(rows(&m), 1, "a Problems row to begin with");
    let generation = m.generation();
    m.set_known(supplying(rock, 1 << 1)); // only a normal map
    assert_eq!(
        missing_maps(&m, "landscape/rock01"),
        ["_rmaos"],
        "other slot"
    );
    m.set_known(supplying(rock, 1 << 5));
    assert!(missing_maps(&m, "landscape/rock01").is_empty());
    assert_eq!(m.generation(), generation, "no new plan");
    assert_eq!(rows(&m), 0, "no Problems row");
    m.set_known(Default::default());
    assert_eq!(missing_maps(&m, "landscape/rock01"), ["_rmaos"], "back");
}

/// A set read under an alias is looked up by the name it writes (final review I2: the meshes name the output). A PBR
/// set's match key and outputs follow its input name (Note JH1), so the name on disk is not asked.
#[test]
fn an_aliased_set_looks_up_the_written_name() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_without_rmaos(d.path());
    m.set_input_name("landscape/rock01", "textures/landscape/stone")
        .unwrap();
    idle(&mut m);
    let id = "landscape/stone";
    assert_eq!(missing_maps(&m, id), ["_rmaos"]);
    m.set_known(supplying("textures/landscape/rock01.dds", 1 << 5));
    assert_eq!(
        missing_maps(&m, id),
        ["_rmaos"],
        "the name on disk is not asked"
    );
    m.set_known(supplying("textures/landscape/stone.dds", 1 << 5));
    assert!(missing_maps(&m, id).is_empty());
}

/// Note HE4 (final review I2): the meshes name what the set writes, so a set with an `output_name` is looked up by
/// that name, not by the name it is read as.
#[test]
fn a_renamed_output_looks_up_the_written_name() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_without_rmaos(d.path());
    m.set_output_name("landscape/rock01", Some("steel"))
        .unwrap();
    idle(&mut m);
    let id = "landscape/rock01";
    assert_eq!(missing_maps(&m, id), ["_rmaos"]);
    m.set_known(supplying("textures/landscape/rock01.dds", 1 << 5));
    assert_eq!(
        missing_maps(&m, id),
        ["_rmaos"],
        "the read-as name is not asked"
    );
    m.set_known(supplying("textures/landscape/steel.dds", 1 << 5));
    assert!(missing_maps(&m, id).is_empty());
}

#[test]
fn no_scan_warns_as_before() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_without_rmaos(d.path());
    idle(&mut m);
    assert!(m.known.supplies.is_empty());
    assert_eq!(missing_maps(&m, "landscape/rock01"), ["_rmaos"]);
}

/// A role the check can name has a slot to look up, or is the diffuse itself.
#[test]
fn every_required_role_has_a_slot() {
    use engine::{config::Target, shader::Shader};
    let diffuse_like = ["albedo", "diffuse", "skin_albedo", "complex_grass"];
    for s in Shader::ALL {
        for t in [Target::Cs, Target::Vanilla] {
            for r in s.spec_for(t).required {
                assert!(
                    map_slot(r.label).is_some() || diffuse_like.contains(&r.label),
                    "{s:?} {t:?}: {}",
                    r.label
                );
            }
        }
    }
}

#[test]
fn short_names_keep_two_parts_and_differ() {
    use sdc_gui::model::sets::short_names;
    assert_eq!(
        short_names(&["textures/arch/eye/eye", "textures/arch/eye/m", "solo"]),
        vec!["eye/eye", "eye/m", "solo"]
    );
    // a shared two-part tail grows until the names differ
    assert_eq!(
        short_names(&["a/stone/m", "b/stone/m", "c/wood/m"]),
        vec!["a/stone/m", "b/stone/m", "wood/m"]
    );
    // tails compare folded (Note BF): "Stone/m" and "stone/M" collide at two parts, so both grow to three
    assert_eq!(
        short_names(&["X/Stone/m", "y/stone/M"]),
        vec!["X/Stone/m", "y/stone/M"]
    );
    // ids equal after folding never get a unique tail: both keep the whole id
    assert_eq!(short_names(&["a/B", "A/b"]), vec!["a/B", "A/b"]);
}

#[test]
fn every_view_has_its_short_name() {
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let v = views(&m);
    assert!(v
        .iter()
        .all(|v| !v.short.is_empty() && v.id.ends_with(&v.short)));
}
