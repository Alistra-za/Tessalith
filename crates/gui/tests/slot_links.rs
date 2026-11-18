//! Review E1: a `slotN` path a JSON set carries is part of its view (`SetView::linked`), and the albedo check counts it.
mod support;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::codes::Code;
use engine::config::{GreenConvention, Subtype};
use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::sets::*;
use sdc_gui::model::{Answer, AppModel, Selection};
use support::*;

const LEAF: &str = "arch/leaf";
const WALL_ALBEDO: &str = "arch/wall.png";
/// How far a row's cells sit from its slot label: the label is centred on the 56 px thumbnail, the Assign… button and
/// the file name at the top. Rows are 60 px apart.
const ROW_REACH: f32 = 28.0;

fn harness(m: AppModel) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 1200.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m)
}

fn select_set(m: &mut AppModel, id_end: &str) {
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with(id_end))
        .unwrap_or_else(|| panic!("a set ending {id_end}"))
        .key
        .clone();
    m.select_set(&key);
}

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

#[test]
fn picking_from_another_set_fills_the_diffuse_row_and_one_undo_empties_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    let file = member_game_path(&m, WALL_ALBEDO)
        .unwrap()
        .rsplit('\\')
        .next()
        .unwrap()
        .to_string();
    select_set(&mut m, LEAF);
    let mut h = harness(m);
    // premise: Plan 3a's status line names the missing Diffuse before the pick
    assert!(
        h.query_by_label_contains("no Diffuse").is_some(),
        "premise: the Diffuse reason shows first"
    );
    // several empty rows have an Assign…: take the one on the Diffuse row
    let diffuse = slot_rect(&h, "Diffuse");
    h.query_all_by_label("Assign…")
        .find(|n| (n.rect().center().y - diffuse.center().y).abs() < ROW_REACH)
        .expect("Assign… on the Diffuse row")
        .click();
    h.run();
    h.get_by_label("From another set").hover();
    h.run();
    h.get_by_label("arch/wall — wall.png").click();
    h.run();
    let on_row = |h: &Harness<'static, AppModel>, label: &str| {
        h.query_all_by_label(label)
            .any(|n| (n.rect().center().y - diffuse.center().y).abs() < ROW_REACH)
    };
    assert!(on_row(&h, &file), "the Diffuse row's text contains {file}");
    assert!(h.query_by_label("Open set arch/wall").is_some());
    assert!(h.query_by_label("Unlink Diffuse").is_some());
    // Plan 3a's status line drops the Diffuse reason
    assert!(h.query_by_label_contains("no Diffuse").is_none());
    assert!(h
        .state()
        .messages
        .entries()
        .any(|e| e.text == format!("Diffuse of arch/leaf: {file} (from set arch/wall)")));
    h.state_mut().undo();
    h.run();
    assert!(!on_row(&h, &file));
    assert!(on_row(&h, "Assign…"), "one Undo returns the row to empty");
}

#[test]
fn the_source_set_opens_from_the_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.run_menu(MenuAction::SlotLink {
        set: LEAF.into(),
        n: 1,
        item: WALL_ALBEDO.into(),
    });
    select_set(&mut m, LEAF);
    let mut h = harness(m);
    h.get_by_label("Open set arch/wall").click();
    h.run();
    assert!(matches!(&h.state().selection, Selection::Set(k) if k.ends_with("arch/wall")));
}

#[test]
fn a_link_over_an_own_file_shows_the_link_and_unlink_restores_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command(
        "arch/wall",
        2,
        SlotCommand::Path(r"textures\other\x_n.dds".into()),
    )
    .unwrap();
    select_set(&mut m, "arch/wall");
    let mut h = harness(m);
    // scoped to the Normal row: the Textures column's tile is also named "wall_n.png"
    let normal = slot_rect(&h, "Normal _n");
    let on_row = |h: &Harness<'static, AppModel>, label: &str| {
        h.query_all_by_label(label).any(|n| {
            (n.rect().center().y - normal.center().y).abs() < ROW_REACH
                && n.rect().min.x > normal.max.x
        })
    };
    assert!(on_row(&h, "x_n.dds · set by path"));
    assert!(
        !on_row(&h, "wall_n.png"),
        "the own file is not what the JSON carries"
    );
    h.get_by_label("Unlink Normal _n").click();
    h.run();
    assert!(!on_row(&h, "x_n.dds · set by path"));
    assert!(
        on_row(&h, "wall_n.png"),
        "the own file is back on the Normal row"
    );
}

#[test]
fn an_own_file_under_a_link_for_the_other_kind_says_it_is_not_written() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[
            ("leaf_f.png", [200, 200, 200], Subtype::PbrFuzz),
            ("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal),
        ],
    );
    m.run_menu(MenuAction::SlotLink {
        set: LEAF.into(),
        n: 7,
        item: "arch/lacq_cnr.png".into(),
    });
    select_set(&mut m, LEAF);
    let h = harness(m);
    assert!(
        h.query_by_label("leaf_f.png is not written: slot 7 is linked.")
            .is_some(),
        "the own file's line"
    );
    assert!(
        h.query_by_label("Unlink Coat normal / Fuzz (slot 7)")
            .is_some(),
        "the row shows the link"
    );
}

#[test]
fn a_shared_slot_row_belongs_to_the_links_kind() {
    // no own slot-7 or slot-3 file on arch/leaf: `slot_row` alone keeps the card's first kind
    for (n, item, unlink) in [
        (7, "arch/silk_f.png", "Unlink Coat normal / Fuzz (slot 7)"),
        (3, "arch/glow_g.png", "Unlink Emissive / Skin (slot 3)"),
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut m = no_albedo_pair_with(
            d.path(),
            &[
                ("silk_f.png", [200, 200, 200], Subtype::PbrFuzz),
                ("glow_g.png", [255, 200, 0], Subtype::PbrEmissive),
            ],
        );
        m.run_menu(MenuAction::SlotLink {
            set: LEAF.into(),
            n,
            item: item.into(),
        });
        let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
        let cell = view_row(&v)
            .into_iter()
            .find(|c| c.kind.game_slot() == Some(n))
            .expect("the slot's row");
        assert!(
            slot_shown(&v, cell.kind).is_filled(),
            "slot {n}: the row's kind is the link's"
        );
        select_set(&mut m, LEAF);
        let h = harness(m);
        assert!(
            h.query_by_label(unlink).is_some(),
            "slot {n}: the row offers Unlink"
        );
        let row = slot_rect(&h, v.slot_label(cell.kind, ""));
        assert!(
            !h.query_all_by_label("Assign…")
                .any(|a| (a.rect().center().y - row.center().y).abs() < ROW_REACH),
            "slot {n}: no Assign… on a linked row"
        );
    }
}

#[test]
fn a_clear_over_an_own_file_says_so() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command("arch/wall", 2, SlotCommand::Clear)
        .unwrap();
    select_set(&mut m, "arch/wall");
    let mut h = harness(m);
    assert!(h
        .query_by_label("Left out of the JSON: you cleared this slot.")
        .is_some());
    h.get_by_label("Use wall_n.png for Normal _n").click();
    h.run();
    assert!(h
        .query_by_label("Left out of the JSON: you cleared this slot.")
        .is_none());
}
