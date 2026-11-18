//! Review E1: a `slotN` path a JSON set carries is part of its view (`SetView::linked`), and the albedo check counts it.
mod support;
use engine::codes::Code;
use engine::config::{GreenConvention, Subtype};
use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::sets::*;
use sdc_gui::model::{Answer, AppModel};
use support::*;

const LEAF: &str = "arch/leaf";
const WALL_ALBEDO: &str = "arch/wall.png";

fn needs_albedo(m: &AppModel) -> bool {
    views(m)
        .iter()
        .find(|v| v.id == LEAF)
        .unwrap()
        .set_warnings
        .iter()
        .any(|w| w.code == Code::ShaderMissingMap && w.data["map"] == "albedo")
}

#[test]
fn a_linked_slot_is_filled_in_the_view_and_clears_the_albedo_check() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    assert!(needs_albedo(&m), "fixture premise: arch/leaf needs albedo");
    let game = member_game_path(&m, WALL_ALBEDO).unwrap();
    assert_eq!(
        m.run_menu(MenuAction::SlotLink {
            set: LEAF.into(),
            n: 1,
            item: WALL_ALBEDO.into()
        }),
        None
    );
    let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert_eq!(
        v.linked[&1],
        SlotLink {
            path: game.clone(),
            from: Some("arch/wall".into()),
            from_short: Some("arch/wall".into()),
            item: Some(WALL_ALBEDO.into()),
            kind: Some(SlotKind::Diffuse),
            row: Some(SlotKind::Diffuse)
        }
    );
    assert!(slot_shown(&v, SlotKind::Diffuse).is_filled());
    assert!(!needs_albedo(&m), "the albedo check counts the link");
    m.undo();
    let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert!(v.linked.is_empty() && matches!(slot_shown(&v, SlotKind::Diffuse), SlotShown::Empty));
    assert!(needs_albedo(&m), "one Undo brings the check back");
}

#[test]
fn a_typed_path_is_a_link_without_a_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command(LEAF, 1, SlotCommand::Path(r"textures\other\x.dds".into()))
        .unwrap();
    let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert_eq!(
        (v.linked[&1].from.as_deref(), v.linked[&1].item.as_deref()),
        (None, None)
    );
    assert!(!needs_albedo(&m));
}

#[test]
fn a_coat_link_fills_coat_only() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal)],
    );
    let n = SlotKind::Coat.game_slot().unwrap(); // 7, shared with Fuzz (sets.rs l.79)
    assert_eq!(
        m.run_menu(MenuAction::SlotLink {
            set: LEAF.into(),
            n,
            item: "arch/lacq_cnr.png".into()
        }),
        None
    );
    let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert_eq!(v.linked[&n].kind, Some(SlotKind::Coat));
    assert!(slot_shown(&v, SlotKind::Coat).is_filled());
    assert!(
        matches!(slot_shown(&v, SlotKind::Fuzz), SlotShown::Empty),
        "one link fills one kind"
    );
}

#[test]
fn an_own_fuzz_under_a_coat_link_is_not_written() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[
            ("leaf_f.png", [200, 200, 200], Subtype::PbrFuzz),
            ("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal),
        ],
    );
    let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert!(
        matches!(slot_shown(&v, SlotKind::Fuzz), SlotShown::Own { .. }),
        "premise: arch/leaf owns leaf_f.png"
    );
    m.run_menu(MenuAction::SlotLink {
        set: LEAF.into(),
        n: 7,
        item: "arch/lacq_cnr.png".into(),
    });
    let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert!(slot_shown(&v, SlotKind::Coat).is_filled());
    assert!(
        matches!(slot_shown(&v, SlotKind::Fuzz), SlotShown::Replaced { .. }),
        "slot 7 carries the coat link, so the own _f is not written"
    );
    assert!(!slot_shown(&v, SlotKind::Fuzz).is_filled());
}

#[test]
fn a_set_without_a_json_entry_shows_no_link() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path()); // landscape/leaf: a plain set, no PGPatcher entry
    let leaf = views(&m)
        .iter()
        .find(|v| v.id.ends_with("leaf"))
        .unwrap()
        .clone();
    assert!(
        !sdc_gui::model::pbr_export::json_entries(&m)
            .iter()
            .any(|(v, _)| v.id == leaf.id),
        "premise"
    );
    m.set_slot_command(
        &leaf.id,
        1,
        SlotCommand::Path(r"textures\other\x.dds".into()),
    )
    .unwrap();
    let v = views(&m).iter().find(|v| v.id == leaf.id).unwrap().clone();
    assert!(
        v.linked.is_empty(),
        "no file carries this slotN, so nothing reads filled"
    );
}

/// Guard (passes before the fix too): an explicit Clear is not a link.
#[test]
fn an_empty_slot_value_is_not_a_link() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command(LEAF, 1, SlotCommand::Clear).unwrap();
    let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert!(v.linked.is_empty() && needs_albedo(&m));
}

/// Guard: a re-plan between the link and its Undo leaves no stale view.
#[test]
fn undo_after_a_fast_replan_unlinks_the_slot() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.run_menu(MenuAction::SlotLink {
        set: LEAF.into(),
        n: 1,
        item: WALL_ALBEDO.into(),
    });
    m.decide("arch/wall_n.png", Answer::Green(GreenConvention::Opengl));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(!needs_albedo(&m));
    m.undo(); // the decision
    m.undo(); // the link
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(views(&m)
        .iter()
        .find(|v| v.id == LEAF)
        .unwrap()
        .linked
        .is_empty());
    assert!(needs_albedo(&m));
}

/// `job_key` in `view_reads`: when the source's output path moves, the link no longer names that set.
#[test]
fn a_moved_source_output_unresolves_the_link() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.run_menu(MenuAction::SlotLink {
        set: LEAF.into(),
        n: 1,
        item: WALL_ALBEDO.into(),
    });
    m.set_json_value("arch/wall", "rename", Some(serde_json::json!("stone"))); // re-plans (replans(), sets.rs l.2106)
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert_eq!(
        v.linked[&1].from, None,
        "the old path is no longer written by arch/wall"
    );
}
