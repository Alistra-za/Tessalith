//! Review E1: a `slotN` path a JSON set carries is part of its view (`SetView::linked`), and the albedo check counts it.
mod support;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::codes::Code;
use engine::config::{GreenConvention, Subtype};
use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::pbr_export::json_entries;
use sdc_gui::model::sets::*;
use sdc_gui::model::{Answer, AppModel, Selection};
use sdc_gui::pgp::writer::document;
use serde_json::{json, Value};
use support::*;

const LEAF: &str = "arch/leaf";
const WALL_ALBEDO: &str = "arch/wall.png";
/// How far a row's cells sit from its slot label: the label is centred on the 56 px thumbnail, the Assign… button and
/// the file name at the top. Rows are 60 px apart.
const ROW_REACH: f32 = 28.0;

fn harness(m: AppModel) -> Harness<'static, AppModel> {
    harness_at(m, 1400.0)
}

fn harness_at(m: AppModel, width: f32) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(width, 1200.0))
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
fn the_title_arrow_shows_only_when_files_go_there() {
    for (set, arrow) in [(LEAF, false), ("arch/wall", true)] {
        let d = tempfile::tempdir().unwrap();
        let mut m = no_albedo_pair(d.path());
        select_set(&mut m, set);
        let h = harness(m);
        assert_eq!(
            h.query_by_label_contains("→ textures").is_some(),
            arrow,
            "{set}: arrow only where files go"
        );
    }
}

#[test]
fn every_listed_output_exists_after_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    let leaf = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    let listed = set_outputs(&leaf).to_vec();
    assert!(!listed.is_empty());
    // The harness is built after this, so StatCache's TTL never holds a stale "missing".
    export_through(&mut m);
    // The export runs on a worker: wait for it, or the files are not there yet.
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    for (_, _, p) in &listed {
        assert!(p.is_file(), "{} exists after export", p.display());
    }
    select_set(&mut m, LEAF);
    let mut h = harness(m);
    let n = listed.len();
    assert!(
        h.query_by_label_contains(&format!("{n}, all written"))
            .is_some(),
        "closed header summary"
    );
    h.get_by_label_contains("Output files").click();
    h.run();
    for (_, _, p) in &listed {
        let name = p.file_name().unwrap().to_string_lossy();
        assert!(h
            .query_by_label(&format!("Show {name} in Explorer"))
            .is_some());
    }
}

/// The arrow exists because a PBR set's files go under the prefix: the listed path of `arch/wall` must be the file the
/// export wrote there.
#[test]
fn every_listed_output_of_the_pbr_set_exists_after_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    let wall = views(&m)
        .iter()
        .find(|v| v.id == "arch/wall")
        .unwrap()
        .clone();
    assert!(wall.pbr, "premise: arch/wall is the PBR set");
    let listed = set_outputs(&wall).to_vec();
    assert!(!listed.is_empty());
    export_through(&mut m);
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    for (_, _, p) in &listed {
        assert!(p.is_file(), "{} exists after export", p.display());
    }
    select_set(&mut m, "arch/wall");
    let h = harness(m);
    assert!(h
        .query_by_label_contains(&format!("{}, all written", listed.len()))
        .is_some());
}

#[test]
fn a_member_that_fills_no_slot_is_listed_after_the_slot_files() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[("leaf_m.png", [10, 20, 30], Subtype::ComplexMaterial)],
    );
    let key = "arch/leaf_m.png";
    let item = m.item(key).expect("the _m file is an item");
    assert!(slot_of(item).is_none(), "premise: it fills no slot");
    let out = item
        .job
        .as_ref()
        .expect("premise: it has a job")
        .output
        .clone();
    let leaf = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert!(
        leaf.members.iter().any(|k| k == key),
        "premise: it is leaf's"
    );
    let listed = set_outputs(&leaf).to_vec();
    let at: Vec<usize> = listed
        .iter()
        .enumerate()
        .filter(|(_, (_, _, p))| *p == out)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(at, vec![listed.len() - 1], "once, after the slot files");
    assert_eq!(listed.last().unwrap().0, None);
    assert!(listed[..listed.len() - 1]
        .iter()
        .all(|(k, _, _)| k.is_some()));
    select_set(&mut m, LEAF);
    let h = harness(m);
    assert!(
        h.query_by_label_contains(&format!("{}, not exported yet", listed.len()))
            .is_some(),
        "the count includes it"
    );
}

#[test]
fn the_output_files_group_is_not_drawn_in_the_3d_view() {
    use sdc_gui::ui::set_editor::{draw, View};
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    select_set(&mut m, LEAF);
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with(LEAF))
        .unwrap()
        .key
        .clone();
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            move |ui, s: &mut (AppModel, String)| {
                let view = View {
                    three_d: true,
                    ..Default::default()
                };
                draw(
                    ui,
                    &mut s.0,
                    &s.1,
                    &mut Default::default(),
                    &mut Default::default(),
                    &mut Default::default(),
                    view,
                );
            },
            (m, key),
        );
    // The viewport's share of the centre is pinned (`set_preview_layout`), so the header stays out of the 3D view.
    assert!(h.query_by_label_contains("Output files").is_none());
    assert!(
        h.query_by_label("Slot commands…").is_none(),
        "premise: the 3D path draws no slot rows"
    );
}

#[test]
fn a_set_with_some_files_written_says_k_of_n() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    let leaf = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    let listed = set_outputs(&leaf).to_vec();
    assert!(listed.len() >= 2, "premise: leaf writes two files");
    export_through(&mut m);
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    std::fs::remove_file(&listed[0].2).unwrap();
    select_set(&mut m, LEAF);
    let n = listed.len();
    let h = harness(m);
    assert!(h
        .query_by_label_contains(&format!("{} of {n} written", n - 1))
        .is_some());
}

#[test]
fn show_in_explorer_waits_for_the_file() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    select_set(&mut m, LEAF);
    let mut h = harness(m);
    assert!(
        h.query_by_label_contains("not exported yet").is_some(),
        "closed and not silent"
    );
    assert!(
        h.query_by_label_contains(" in Explorer").is_none(),
        "closed by default"
    );
    h.get_by_label_contains("Output files").click();
    h.run();
    assert!(
        h.query_by_label_contains(" in Explorer").is_none(),
        "nothing exported yet"
    );
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
    assert!(h.query_by_label("Open set arch/wall (Diffuse)").is_some());
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
    h.get_by_label("Open set arch/wall (Diffuse)").click();
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
        h.query_by_label("Normal _n format").is_none(),
        "a linked row has no format control: the file is the other set's"
    );
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
    assert!(
        h.query_by_label("Normal _n format").is_some(),
        "premise: the own file's row has the format control the linked row lacked"
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

/// Fix round 1 (I1 of the review): a link shows on the row of the slot it sits on, whatever kind the typed path's
/// file is. Unlink then acts on that slot.
#[test]
fn a_path_to_another_kind_on_slot_1_is_the_diffuse_rows_link() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    let normal = member_game_path(&m, "arch/wall_n.png").unwrap();
    let file = normal.rsplit('\\').next().unwrap().to_string();
    m.set_slot_command(LEAF, 1, SlotCommand::Path(normal))
        .unwrap();
    select_set(&mut m, LEAF);
    let mut h = harness(m);
    let (diffuse, normal_row) = (slot_rect(&h, "Diffuse"), slot_rect(&h, "Normal _n"));
    let on = |h: &Harness<'static, AppModel>, row: egui::Rect, label: &str| {
        h.query_all_by_label(label)
            .any(|n| (n.rect().center().y - row.center().y).abs() < ROW_REACH)
    };
    assert!(on(&h, diffuse, &file), "the Diffuse row shows the file");
    assert!(
        h.query_by_label("Unlink Diffuse").is_some(),
        "the Diffuse row offers Unlink"
    );
    assert!(
        h.query_by_label("Unlink Normal _n").is_none(),
        "the Normal row is not Linked"
    );
    assert!(!on(&h, normal_row, &file));
    h.get_by_label("Unlink Diffuse").click();
    h.run();
    let v = views(h.state())
        .iter()
        .find(|v| v.id == LEAF)
        .unwrap()
        .clone();
    assert!(
        !v.linked.contains_key(&1),
        "Unlink cleared slot 1: {:?}",
        v.linked
    );
}

/// Fix round 1 (I2): the "from set" link stays inside the File cell beside a long file name, left of Unlink.
#[test]
fn a_long_linked_file_name_keeps_the_set_link_inside_its_cell() {
    use egui_kittest::kittest::NodeT;
    let long = "lacquer_with_a_very_long_name_that_cannot_fit_in_the_file_cell_beside_the_link_to_its_set_and_the_unlink_button_cnr.png";
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &[(long, [128, 128, 255], Subtype::CoatNormal)]);
    m.run_menu(MenuAction::SlotLink {
        set: LEAF.into(),
        n: 7,
        item: format!("arch/{long}"),
    });
    select_set(&mut m, LEAF);
    const WIDTH: f32 = 1280.0;
    let mut h = harness_at(m, WIDTH);
    let unlink = h.get_by_label("Unlink Coat normal / Fuzz (slot 7)").rect();
    let link_name = "Open set arch/lacquer";
    let link = h
        .query_all_by_label_contains(link_name)
        .next()
        .expect("the set link");
    let link_rect = link.rect();
    let on_row = |r: &egui::Rect| (r.center().y - unlink.center().y).abs() < ROW_REACH;
    let file = h
        .query_all_by_label_contains(long.trim_end_matches(".png"))
        .map(|n| n.rect())
        .find(on_row)
        .expect("the file label on the linked row");
    // The restored window assert: nothing on the row reaches past the centre's right edge, Unlink and the Status dot
    // included.
    let full = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    let margin = egui::Frame::central_panel(&h.ctx.global_style())
        .inner_margin
        .right as f32;
    let edge = full.intersect(h.ctx.content_rect()).right() - margin;
    assert!(
        unlink.max.x <= edge,
        "Unlink {unlink:?} passes the centre's edge {edge}"
    );
    let mut row_nodes = vec![];
    for role in [
        egui::accesskit::Role::Button,
        egui::accesskit::Role::Link,
        egui::accesskit::Role::Label,
        egui::accesskit::Role::Image,
        egui::accesskit::Role::GenericContainer,
    ] {
        row_nodes.extend(
            h.query_all_by_role(role)
                .filter(|n| n.accesskit_node().raw_bounds().is_some())
                .map(|n| n.rect())
                .filter(|r| {
                    on_row(r) && r.min.x >= full.min.x && r.min.x < edge && r.width() > 0.0
                }),
        );
    }
    assert!(
        row_nodes.len() > 4,
        "found the row's controls {row_nodes:?}"
    );
    for r in &row_nodes {
        assert!(
            r.max.x <= edge,
            "row control {r:?} passes the centre's edge {edge}: Unlink {unlink:?}"
        );
    }
    // Both sides stay in the File cell, which ends where the Format cell (Unlink) starts: the file label, then the
    // link, and the link takes at most half of the cell.
    let cell_w = unlink.min.x - file.min.x;
    assert!(
        file.max.x <= link_rect.min.x + 0.5 && link_rect.max.x <= unlink.min.x,
        "file {file:?} then link {link_rect:?} inside the File cell ending at Unlink {unlink:?}"
    );
    assert!(
        link_rect.width() <= cell_w / 2.0 + 0.5 && file.width() >= link_rect.width(),
        "the link {link_rect:?} takes at most half of the {cell_w} px cell, the file {file:?} the rest"
    );
    // The accessible name keeps the full set name; the full visible text is on hover.
    let name = h
        .get_by_label_contains(link_name)
        .accesskit_node()
        .label()
        .expect("the link's name");
    let short = name
        .strip_prefix("Open set ")
        .and_then(|s| s.strip_suffix(" (Coat normal / Fuzz (slot 7))"))
        .unwrap_or_else(|| panic!("the name keeps the full set name and the slot: {name}"));
    assert!(short.len() > 60, "premise: a long set name, {short}");
    let full_text = format!("from set {short}");
    assert!(
        h.query_by_label(&full_text).is_none(),
        "premise: no tooltip yet"
    );
    h.get_by_label_contains(link_name).hover();
    h.run();
    assert!(
        h.query_all_by_label(&full_text).next().is_some(),
        "hovering the truncated link shows its full text"
    );
}

/// Fix round 1 (I3): a pinned slot refuses a slot command, so Unlink and Use this file are disabled, as the menu is.
#[test]
fn unlink_and_use_this_file_are_disabled_on_a_pinned_slot() {
    use egui_kittest::kittest::NodeT;
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.run_menu(MenuAction::SlotLink {
        set: LEAF.into(),
        n: 1,
        item: WALL_ALBEDO.into(),
    });
    m.set_slot_command("arch/wall", 2, SlotCommand::Clear)
        .unwrap();
    for (set, n) in [(LEAF, 1), ("arch/wall", 2)] {
        assert_eq!(
            m.run_menu(MenuAction::SlotLock {
                set: set.into(),
                n,
                on: true
            }),
            None
        );
    }
    select_set(&mut m, LEAF);
    let h = harness(m);
    assert!(h
        .get_by_label("Unlink Diffuse")
        .accesskit_node()
        .is_disabled());
    let mut m = h.into_state();
    select_set(&mut m, "arch/wall");
    let h = harness(m);
    assert!(h
        .get_by_label("Use wall_n.png for Normal _n")
        .accesskit_node()
        .is_disabled());
}

/// Fix round 1 (I1): a link is found by the game path `game_path_in` writes, whatever folder is opened. Opened at
/// `textures/arch`, the output key is `pbr/wall.dds` while the link is `textures\pbr\arch\wall.dds`.
#[test]
fn a_link_resolves_when_the_opened_folder_is_below_textures() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_at(
        d.path(),
        "arch",
        &[
            ("leaf_f.png", [200, 200, 200], Subtype::PbrFuzz),
            ("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal),
        ],
    );
    assert_eq!(
        m.run_menu(MenuAction::SlotLink {
            set: "leaf".into(),
            n: 7,
            item: "lacq_cnr.png".into()
        }),
        None
    );
    let v = views(&m).iter().find(|v| v.id == "leaf").unwrap().clone();
    let l = &v.linked[&7];
    assert!(
        l.from.is_some() && l.item.as_deref() == Some("lacq_cnr.png"),
        "{l:?}"
    );
    assert_eq!(l.kind, Some(SlotKind::Coat), "{l:?}");
    assert!(matches!(
        slot_shown(&v, SlotKind::Coat),
        SlotShown::Linked(_)
    ));
    assert!(
        matches!(slot_shown(&v, SlotKind::Fuzz), SlotShown::Replaced { .. }),
        "the coat link is not shown on the Fuzz row"
    );
}

fn entry(m: &AppModel, id: &str) -> Value {
    let e = json_entries(m)
        .into_iter()
        .find(|(v, _)| v.id == id)
        .unwrap_or_else(|| panic!("no entry for {id}"))
        .1;
    document(std::slice::from_ref(&e)).to_value()["entries"][0].clone()
}

fn extra() -> Vec<(&'static str, [u8; 3], Subtype)> {
    vec![
        ("glow_g.png", [255, 200, 0], Subtype::PbrEmissive),
        ("bump_p.png", [128, 128, 128], Subtype::Height),
        ("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal),
        ("silk_f.png", [200, 200, 200], Subtype::PbrFuzz),
        ("skin_s.png", [220, 120, 100], Subtype::PbrSubsurface),
    ]
}

fn link(m: &mut AppModel, n: u8, item: &str) {
    assert_eq!(
        m.run_menu(MenuAction::SlotLink {
            set: LEAF.into(),
            n,
            item: item.into()
        }),
        None
    );
}

/// The selected set's `control` is greyed, and hovering it says `reason` (the reason shows on hover only).
fn blocked_with_reason(m: AppModel, control: &str, reason: &str) {
    use egui_kittest::kittest::NodeT;
    let mut h = harness(m);
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.run();
    h.get_by_label(control).scroll_to_me();
    h.run();
    assert!(
        h.get_by_label(control).accesskit_node().is_disabled(),
        "{control} is greyed"
    );
    assert!(
        h.query_by_label_contains(reason).is_none(),
        "premise: on hover only"
    );
    h.get_by_label(control).hover();
    h.run();
    h.run();
    assert!(h.query_by_label_contains(reason).is_some(), "{reason}");
}

/// Whether the selected set's `control` is greyed.
fn greyed(m: AppModel, control: &str) -> (AppModel, bool) {
    use egui_kittest::kittest::NodeT;
    let mut h = harness(m);
    h.run();
    h.get_by_label(control).scroll_to_me();
    h.run();
    let off = h.get_by_label(control).accesskit_node().is_disabled();
    (h.into_state(), off)
}

#[test]
fn a_slot_linked_from_another_set_turns_its_feature_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    assert_eq!(
        entry(&m, LEAF).get("emissive"),
        None,
        "premise: no emissive before"
    );
    link(&mut m, 3, "arch/glow_g.png");
    assert_eq!(entry(&m, LEAF)["emissive"], json!(true));
    m.undo();
    assert_eq!(
        entry(&m, LEAF).get("emissive"),
        None,
        "one Undo takes the key away"
    );
}

#[test]
fn a_linked_height_turns_parallax_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    assert_eq!(
        entry(&m, LEAF).get("parallax"),
        None,
        "premise: no parallax before"
    );
    link(&mut m, 4, "arch/bump_p.png");
    assert_eq!(entry(&m, LEAF)["parallax"], json!(true));
}

#[test]
fn a_linked_coat_turns_multilayer_on_and_blocks_fuzz() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    link(&mut m, 7, "arch/lacq_cnr.png");
    let e = entry(&m, LEAF);
    assert_eq!(
        (e["multilayer"].clone(), e["coat_normal"].clone()),
        (json!(true), json!(true))
    );
    // The writer turns `fuzz_on` into the `fuzz` object, so that is the key that shows fuzz on.
    assert_eq!(e.get("fuzz"), None);
    select_set(&mut m, LEAF);
    blocked_with_reason(m, "Fuzz", "off: the set has a coat normal (_cnr)");
}

#[test]
fn a_linked_fuzz_turns_fuzz_on_and_blocks_the_coat_normal() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    select_set(&mut m, LEAF);
    // The coat normal's own row sits in the Multilayer group, which is not drawn while Multilayer is off, so the
    // block shows as the greyed Multilayer switch.
    let (mut m, off) = greyed(m, "Multilayer (coat)");
    assert!(!off, "premise: Multilayer is free before the link");
    link(&mut m, 7, "arch/silk_f.png");
    // The writer turns `fuzz_on` into the `fuzz` object, so that is the key that shows fuzz on.
    assert!(entry(&m, LEAF).get("fuzz").is_some());
    let (mut m, off) = greyed(m, "Multilayer (coat)");
    assert!(off, "a linked fuzz greys Multilayer");
    // Multilayer set by hand shows the Coat normal row, which the linked fuzz blocks with its own reason.
    m.set_json_value(LEAF, "multilayer", Some(json!(true)));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    blocked_with_reason(
        m,
        "Coat normal (_cnr)",
        "off: the set has a fuzz texture (_f)",
    );
}

#[test]
fn a_linked_subsurface_turns_subsurface_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    link(&mut m, 8, "arch/skin_s.png");
    assert_eq!(entry(&m, LEAF)["subsurface"], json!(true));
}

/// A link turns on the feature of the slot it fills, so two links to the other's kind keep both features.
#[test]
fn links_across_two_slots_keep_each_others_features() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    link(&mut m, 3, "arch/bump_p.png");
    link(&mut m, 4, "arch/glow_g.png");
    let e = entry(&m, LEAF);
    assert_eq!(e["emissive"], json!(true), "slot 3 is the emissive slot");
    assert_eq!(e["parallax"], json!(true), "slot 4 is the height slot");
}

/// A clear on one slot does not take away what a link on another slot gives.
#[test]
fn a_clear_on_one_slot_keeps_a_cross_slot_links_feature() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    link(&mut m, 3, "arch/bump_p.png");
    m.set_slot_command(LEAF, 4, SlotCommand::Clear).unwrap();
    let e = entry(&m, LEAF);
    assert_eq!(e["slot4"], json!(""), "premise: slot 4 is cleared");
    assert_eq!(e["emissive"], json!(true), "slot 3 still names its file");
    assert_eq!(e.get("parallax"), None, "slot 4 names none");
}

/// A link on game slot 5 fills no row, so it turns no feature on.
#[test]
fn a_link_on_a_slot_with_no_row_turns_no_feature_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    link(&mut m, 5, "arch/glow_g.png");
    let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert!(
        v.linked[&5].item.is_some() && v.linked[&5].row.is_none(),
        "premise: a link with an item and no row"
    );
    assert_eq!(entry(&m, LEAF).get("emissive"), None);
}

#[test]
fn a_typed_path_turns_no_feature_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    let before = entry(&m, LEAF);
    m.set_slot_command(LEAF, 3, SlotCommand::Path(r"textures\other\x_g.dds".into()))
        .unwrap();
    let after = entry(&m, LEAF);
    assert_eq!(after.get("emissive"), None);
    assert_eq!(
        after["slot3"],
        json!(r"textures\other\x_g.dds"),
        "the path itself is written"
    );
    assert_eq!(
        before
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| *k != "slot3")
            .count(),
        after
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| *k != "slot3")
            .count()
    );
}

#[test]
fn a_link_whose_source_set_goes_loses_its_feature_and_undo_brings_it_back() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    link(&mut m, 3, "arch/glow_g.png");
    assert_eq!(entry(&m, LEAF)["emissive"], json!(true));
    assert!(m.delete_set("arch/glow"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert_eq!(
        entry(&m, LEAF).get("emissive"),
        None,
        "no source set: the link no longer counts"
    );
    m.undo();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert_eq!(
        entry(&m, LEAF)["emissive"],
        json!(true),
        "Undo restores the set and the key"
    );
}

#[test]
fn a_cleared_slot_turns_no_feature_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[("leaf_g.png", [255, 200, 0], Subtype::PbrEmissive)],
    );
    let v = views(&m).iter().find(|v| v.id == LEAF).unwrap().clone();
    assert!(
        matches!(v.slots.get(&SlotKind::Emissive), Some(SlotFill::Own { .. })),
        "premise: an own _g"
    );
    assert_eq!(
        entry(&m, LEAF)["emissive"],
        json!(true),
        "premise: the own _g turns emissive on"
    );
    m.set_slot_command(LEAF, 3, SlotCommand::Clear).unwrap();
    let e = entry(&m, LEAF);
    assert_eq!(e["slot3"], json!(""), "the JSON carries the clear command");
    assert_eq!(
        e.get("emissive"),
        None,
        "a cleared slot turns no feature on"
    );
    m.undo();
    assert_eq!(
        entry(&m, LEAF)["emissive"],
        json!(true),
        "one Undo brings the key back"
    );
}

// ---- Task 15a: Export reads the slots the editor shows ----

fn leaf(m: &AppModel) -> SetView {
    views(m).iter().find(|v| v.id == LEAF).unwrap().clone()
}

fn block_texts(m: &AppModel) -> Vec<String> {
    sdc_gui::model::pbr_export::export_blocks(m)
        .iter()
        .map(|i| i.text.clone())
        .collect()
}

/// Export the whole project as a user would; the entry whose `match1` is `match1` in the written JSON (`None`: no such
/// entry, or no JSON was written), and the export's notes.
fn exported_entry(m: &mut AppModel, match1: &str) -> (Option<Value>, String) {
    export_through(m);
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    let notes = m.export.notes.join("\n");
    let entry = std::fs::read_to_string(pgp_json_path(m))
        .ok()
        .and_then(|t| {
            let j: Value = serde_json::from_str(&t).unwrap();
            j["entries"]
                .as_array()?
                .iter()
                .find(|e| e["match1"] == match1)
                .cloned()
        });
    (entry, notes)
}

const LEAF_MATCH: &str = r"arch\leaf.dds";
const NO_HEIGHT: &str = "arch/leaf: parallax is on but the set has no height texture (_p)";

/// R1 (a): Parallax set by hand on a set with no height of its own, and a height linked from another set: Export does
/// not refuse the set, and the written JSON carries the linked path and the key.
#[test]
fn export_takes_a_linked_height_for_parallax_set_by_hand() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    m.set_json_value(LEAF, "parallax", Some(json!(true)));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(
        block_texts(&m).iter().any(|b| b == NO_HEIGHT),
        "premise: {:?}",
        block_texts(&m)
    );
    link(&mut m, 4, "arch/bump_p.png");
    assert!(
        !block_texts(&m).iter().any(|b| b == NO_HEIGHT),
        "{:?}",
        block_texts(&m)
    );
    let path = member_game_path(&m, "arch/bump_p.png").unwrap();
    let (e, notes) = exported_entry(&mut m, LEAF_MATCH);
    let e = e.unwrap_or_else(|| panic!("no entry for arch/leaf: {notes}"));
    assert_eq!(
        (e["slot4"].clone(), e["parallax"].clone()),
        (json!(path), json!(true))
    );
}

/// R1 (a), a typed path no set writes: it fills the slot for the JSON and is never missing.
#[test]
fn export_takes_a_typed_height_path_for_parallax_set_by_hand() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_json_value(LEAF, "parallax", Some(json!(true)));
    m.set_slot_command(LEAF, 4, SlotCommand::Path(r"textures\other\x_p.dds".into()))
        .unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(
        !block_texts(&m).iter().any(|b| b == NO_HEIGHT),
        "{:?}",
        block_texts(&m)
    );
    let (e, notes) = exported_entry(&mut m, LEAF_MATCH);
    let e = e.unwrap_or_else(|| panic!("no entry for arch/leaf: {notes}"));
    assert_eq!(e["slot4"], json!(r"textures\other\x_p.dds"));
}

/// R1 (b): Parallax set by hand over an own height the user cleared: the JSON carries no height, so Export refuses the
/// set as it refuses any set with Parallax on and no height, and writes nothing.
#[test]
fn export_refuses_parallax_set_by_hand_over_a_cleared_height() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[("leaf_p.png", [128, 128, 128], Subtype::Height)],
    );
    m.set_json_value(LEAF, "parallax", Some(json!(true)));
    m.set_slot_command(LEAF, 4, SlotCommand::Clear).unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(
        matches!(
            leaf(&m).slots.get(&SlotKind::Height),
            Some(SlotFill::Own { .. })
        ),
        "premise: an own height"
    );
    assert!(
        block_texts(&m).iter().any(|b| b == NO_HEIGHT),
        "{:?}",
        block_texts(&m)
    );
    let (e, _) = exported_entry(&mut m, LEAF_MATCH);
    assert!(e.is_none(), "Export stopped: {e:?}");
    assert!(!pgp_json_path(&m).exists());
}

/// R1 (c): a set with both `_cnr` and `_f` whose slot 7 is linked: the JSON carries one file there, so there is no
/// clash, as the editor shows none.
#[test]
fn export_takes_a_linked_slot_7_over_an_own_cnr_and_f() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[
            ("leaf_cnr.png", [128, 128, 255], Subtype::CoatNormal),
            ("leaf_f.png", [200, 200, 200], Subtype::PbrFuzz),
            ("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal),
        ],
    );
    let clash = "arch/leaf: _cnr and _f both use slot 7";
    assert!(
        block_texts(&m).iter().any(|b| b == clash),
        "premise: {:?}",
        block_texts(&m)
    );
    link(&mut m, 7, "arch/lacq_cnr.png");
    assert!(
        !block_texts(&m).iter().any(|b| b == clash),
        "{:?}",
        block_texts(&m)
    );
    let path = member_game_path(&m, "arch/lacq_cnr.png").unwrap();
    let (e, notes) = exported_entry(&mut m, LEAF_MATCH);
    let e = e.unwrap_or_else(|| panic!("no entry for arch/leaf: {notes}"));
    assert_eq!(e["slot7"], json!(path));
}

/// Ruling 3b-F10 (a), pinning 3b-F5: a typed path no set writes on slot 7 (`_cnr` + `_f`) or slot 3 (`_g` + `_sk`)
/// is the one path the JSON carries there, so the clash block goes and the written entry names the typed path.
#[test]
fn export_takes_a_typed_path_over_a_clashing_slot_7_or_3() {
    let cases = [
        (
            7u8,
            [
                ("leaf_cnr.png", Subtype::CoatNormal),
                ("leaf_f.png", Subtype::PbrFuzz),
            ],
            "arch/leaf: _cnr and _f both use slot 7",
        ),
        (
            3,
            [
                ("leaf_g.png", Subtype::PbrEmissive),
                ("leaf_sk.png", Subtype::SkinSk),
            ],
            "arch/leaf: _g and _sk both use slot 3",
        ),
    ];
    let typed = r"textures\other\typed.dds";
    for (n, files, clash) in cases {
        let d = tempfile::tempdir().unwrap();
        let extra: Vec<_> = files
            .iter()
            .map(|(f, s)| (*f, [128, 128, 255], *s))
            .collect();
        let mut m = no_albedo_pair_with(d.path(), &extra);
        assert!(
            block_texts(&m).iter().any(|b| b == clash),
            "premise (slot {n}): {:?}",
            block_texts(&m)
        );
        m.set_slot_command(LEAF, n, SlotCommand::Path(typed.into()))
            .unwrap();
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        assert!(
            !block_texts(&m).iter().any(|b| b == clash),
            "slot {n}: {:?}",
            block_texts(&m)
        );
        let (e, notes) = exported_entry(&mut m, LEAF_MATCH);
        let e = e.unwrap_or_else(|| panic!("slot {n}: no entry for arch/leaf: {notes}"));
        assert_eq!(e[format!("slot{n}")], json!(typed), "slot {n}");
    }
}

/// R1 (d): a clear over a fill that has no output path (a borrow from `arch/bump`, a set that is not PBR, so its
/// output has no explicit path): the JSON clears the slot, so nothing blocks on that path.
#[test]
fn export_takes_a_clear_over_a_fill_with_no_output_path() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    m.edit_docs("share", None, true, |_, s| {
        s.set_borrow(LEAF, 4, Some("arch/bump"))
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let no_path = "arch/leaf: slot4 borrows from arch/bump, which has no output path";
    assert!(
        block_texts(&m).iter().any(|b| b == no_path),
        "premise: {:?}",
        block_texts(&m)
    );
    // The menu's Clear drops a borrow (Note HC7); a `slot4 = ""` written over it (by hand, or kept from before the
    // borrow) clears the JSON's slot and keeps the borrow.
    m.set_json_value(LEAF, "slot4", Some(json!("")));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(
        matches!(
            leaf(&m).slots.get(&SlotKind::Height),
            Some(SlotFill::Borrowed { .. })
        ),
        "premise: the borrow stays"
    );
    assert!(
        !block_texts(&m).iter().any(|b| b.contains("slot4")),
        "{:?}",
        block_texts(&m)
    );
    let (e, notes) = exported_entry(&mut m, LEAF_MATCH);
    let e = e.unwrap_or_else(|| panic!("no entry for arch/leaf: {notes}"));
    assert_eq!(e["slot4"], json!(""));
}

/// R3: a link is looked up with PGPatcher's explicit-slot rule (`pgpsim` `slots.rs`: lower case, `textures\` put in
/// front unless it is there): `pbr\arch\wall.dds` names the file `textures\pbr\arch\wall.dds` names, and
/// `other\textures\pbr\arch\wall.dds` names another file.
#[test]
fn a_link_is_looked_up_with_the_games_slot_path_rule() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    let game = member_game_path(&m, WALL_ALBEDO).unwrap();
    let bare = game
        .strip_prefix(r"textures\")
        .expect("premise: a textures path")
        .to_string();
    for (typed, from) in [
        (bare, Some("arch/wall")),
        (game.to_ascii_uppercase(), Some("arch/wall")),
        (format!(r"other\{game}"), None),
    ] {
        m.set_slot_command(LEAF, 1, SlotCommand::Path(typed.clone()))
            .unwrap();
        assert_eq!(leaf(&m).linked[&1].from.as_deref(), from, "{typed}");
    }
}

/// R3: every pick the From another set menu offers is shown as a link to its set and logs its Info line, a second
/// member of one kind included; an unslotted output named by hand is a link to its set too.
#[test]
fn every_pick_the_menu_offers_is_a_link_to_its_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[
            ("rock_n.png", [128, 128, 255], Subtype::PbrNormal),
            ("moss_n.png", [128, 128, 255], Subtype::PbrNormal),
            ("glass_m.png", [10, 20, 30], Subtype::ComplexMaterial),
        ],
    );
    for n in ["arch/rock_n.png", "arch/moss_n.png"] {
        m.decide(n, Answer::Green(GreenConvention::Directx));
    }
    m.edit_docs("two normals", None, true, |_, s| {
        s.add_member("arch/rock", "arch/moss_n.png")
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let rock = views(&m)
        .iter()
        .find(|v| v.id == "arch/rock")
        .unwrap()
        .clone();
    for k in ["arch/rock_n.png", "arch/moss_n.png"] {
        assert!(
            rock.members.iter().any(|x| x == k)
                && m.item(k).and_then(slot_of) == Some(SlotKind::Normal),
            "premise: arch/rock has two normals: {:?}",
            rock.members
        );
    }
    let picks: Vec<MenuAction> = m
        .slot_menu(LEAF, SlotKind::Normal)
        .into_iter()
        .filter(|e| matches!(e.action, MenuAction::SlotLink { .. }) && e.disabled.is_none())
        .map(|e| e.action)
        .collect();
    let picked = |item: &str| {
        picks
            .iter()
            .any(|a| matches!(a, MenuAction::SlotLink { item: i, .. } if i == item))
    };
    assert!(
        picked("arch/rock_n.png") && picked("arch/moss_n.png") && picked("arch/wall_n.png"),
        "premise: {picks:?}"
    );
    for a in picks {
        let MenuAction::SlotLink { item, .. } = &a else {
            unreachable!()
        };
        let set = views(&m)
            .iter()
            .find(|v| v.members.contains(item))
            .unwrap()
            .clone();
        let logged = m.messages.entries().count();
        assert_eq!(m.run_menu(a.clone()), None);
        match slot_shown(&leaf(&m), SlotKind::Normal) {
            SlotShown::Linked(l) => assert_eq!(l.from.as_deref(), Some(set.id.as_str()), "{item}"),
            other => panic!("{item}: {other:?}"),
        }
        let lines: Vec<String> = m.messages.entries().map(|e| e.text.clone()).collect();
        assert_eq!(lines.len(), logged + 1, "{item}: {lines:?}");
        assert!(
            lines[logged].ends_with(&format!("(from set {})", set.short)),
            "{item}: {lines:?}"
        );
        m.undo();
    }
    let glass = member_game_path(&m, "arch/glass_m.png").unwrap();
    m.set_slot_command(LEAF, 3, SlotCommand::Path(glass))
        .unwrap();
    let l = leaf(&m).linked[&3].clone();
    assert_eq!(
        (l.from.as_deref(), l.item.as_deref()),
        (Some("arch/glass"), Some("arch/glass_m.png")),
        "{l:?}"
    );
}

/// R4: a CS set whose statics are Never writes no PGPatcher entry, so a stored link or clear on it is no link and no
/// clear: no feature default, no missing-map row dropped, and an own file under `slotN = ""` shows as itself.
#[test]
fn a_set_the_statics_rule_drops_takes_no_link_and_no_clear() {
    use sdc_gui::model::cs_pbr::{Kind, Statics};
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(d.path(), &extra());
    m.add_cs_block(LEAF, Kind::Txst);
    m.set_cs_statics(LEAF, Statics::Never);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(!m.pgp_entry_wanted(&leaf(&m)), "premise: no entry");
    assert!(needs_albedo(&m), "premise");
    link(&mut m, 1, WALL_ALBEDO);
    link(&mut m, 4, "arch/bump_p.png");
    m.set_slot_command(LEAF, 2, SlotCommand::Clear).unwrap();
    let v = leaf(&m);
    assert!(v.linked.is_empty(), "{:?}", v.linked);
    assert!(needs_albedo(&m), "the albedo row stays");
    assert_eq!(entry(&m, LEAF).get("parallax"), None, "no feature default");
    assert!(
        matches!(slot_shown(&v, SlotKind::Normal), SlotShown::Own { .. }),
        "{:?}",
        slot_shown(&v, SlotKind::Normal)
    );
}

const REC_LEAF: &str = "textures/n/leaf";

/// Ruling 3b-F9: a PBR-picked no-albedo set `textures/n/leaf` (`_n` only) whose TXST record names the height
/// `textures/h/hgt_p.dds`, which the set `textures/h/hgt` writes, so the record borrows it into slot 4.
fn record_height_project(dir: &std::path::Path) -> AppModel {
    for f in ["textures/n/leaf_n.png", "textures/h/hgt_p.png"] {
        let p = dir.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb([128u8, 128, 255]))
            .save(&p)
            .unwrap();
    }
    std::fs::write(
        dir.join("sdc-project.toml"),
        format!(
            "[[sets]]\nid = \"{REC_LEAF}\"\nshader = \"pbr\"\n[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\n\
             edid = \"L\"\nslot4 = \"textures/h/hgt_p.dds\"\n"
        ),
    )
    .unwrap();
    let mut m = opened_as_the_app(dir.to_path_buf(), dir.join("export"));
    m.decide("textures/h/hgt_p.png", Answer::Subtype(Subtype::Height));
    m.decide("textures/n/leaf_n.png", Answer::Subtype(Subtype::PbrNormal));
    m.decide(
        "textures/n/leaf_n.png",
        Answer::Green(GreenConvention::Directx),
    );
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m
}

/// Ruling 3b-F13 (withdrawing 3b-F9's record-fill clause): a stored `slot4` keeps the TXST record from filling slot 4
/// on every set, as on main a0e2610c, so on a set that writes no entry (statics Never) the record's borrowed height
/// goes and with it the Parallax default (that decision feeds the engine's inputs: JR14's one exception to R4). Once
/// the same set writes an entry, the command is what the JSON carries.
#[test]
fn a_stored_slot_command_keeps_a_record_fill_out_as_on_main() {
    use sdc_gui::model::cs_pbr::{Kind, Statics};
    let typed = r"textures\other\x_p.dds";
    // Stored as the JSON editor stores them: the menu's Clear on a record fill turns the fill off instead (Note HG).
    for cmd in ["", typed] {
        let d = tempfile::tempdir().unwrap();
        let mut m = record_height_project(d.path());
        m.add_cs_block(REC_LEAF, Kind::Txst);
        m.set_cs_statics(REC_LEAF, Statics::Never);
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        let at = |m: &AppModel| set_views(m).into_iter().find(|v| v.id == REC_LEAF).unwrap();
        let v = at(&m);
        assert!(!m.pgp_entry_wanted(&v), "premise: no entry");
        assert!(
            matches!(v.slots.get(&SlotKind::Height), Some(SlotFill::Borrowed { from, .. }) if from == "textures/h/hgt"),
            "premise: the record borrows the height: {:?}",
            v.slots
        );
        let parallax = entry(&m, REC_LEAF).get("parallax").cloned();
        assert_eq!(parallax, Some(json!(true)), "premise: the default");
        m.set_json_value(REC_LEAF, "slot4", Some(json!(cmd)));
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        let after = at(&m);
        assert_eq!(
            after.slots.get(&SlotKind::Height),
            None,
            "{cmd:?}: the record fill goes, as on main"
        );
        assert!(
            !after.filled.contains_key(&4),
            "{cmd:?}: {:?}",
            after.filled
        );
        assert_eq!(
            entry(&m, REC_LEAF).get("parallax"),
            None,
            "{cmd:?}: no height, no feature default"
        );
        // The same set writing an entry: the command is the JSON's slot 4.
        m.set_cs_statics(REC_LEAF, Statics::Always);
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        assert!(m.pgp_entry_wanted(&at(&m)), "premise: an entry");
        assert_eq!(entry(&m, REC_LEAF)["slot4"], json!(cmd), "{cmd:?}");
    }
}

/// Ruling 3b-F9: the 3D preview simulates a set whose statics drop its entry from its candidate entry, which a stored
/// link or clear does not change (the editor shows the set's own files); once the set writes an entry, it does.
#[test]
fn the_preview_of_a_set_that_writes_no_entry_simulates_no_stored_slot_command() {
    use sdc_gui::model::cs_pbr::{Kind, Statics};
    use sdc_gui::model::set_preview::fingerprint;
    for cmd in ["", r"textures\other\x_n.dds"] {
        let d = tempfile::tempdir().unwrap();
        let mut m = no_albedo_pair(d.path());
        m.add_cs_block(LEAF, Kind::Txst);
        m.set_cs_statics(LEAF, Statics::Never);
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        let key = leaf(&m).key;
        let simulated = |m: &AppModel| {
            fingerprint(m, &key)
                .and_then(|f| f.entry)
                .expect("the preview simulates the candidate entry")
        };
        let before = simulated(&m);
        m.set_json_value(LEAF, "slot2", Some(json!(cmd)));
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        assert_eq!(simulated(&m), before, "{cmd:?}");
        m.set_cs_statics(LEAF, Statics::Always);
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        assert_eq!(
            simulated(&m).slots.get(&2).map(String::as_str),
            Some(cmd),
            "{cmd:?}: an entry the set writes carries the command"
        );
    }
}

/// R4: on a target with no PGPatcher JSON a stored `slotN = ""` leaves the own file as it is: no dim chip, no "Left
/// out of the JSON" sentence.
#[test]
fn on_a_target_with_no_json_a_stored_clear_leaves_the_own_file_as_it_is() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command(LEAF, 2, SlotCommand::Clear).unwrap();
    assert!(
        matches!(
            slot_shown(&leaf(&m), SlotKind::Normal),
            SlotShown::Cleared { .. }
        ),
        "premise: cleared on cs"
    );
    m.set_target(engine::config::Target::Vanilla);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(
        matches!(
            slot_shown(&leaf(&m), SlotKind::Normal),
            SlotShown::Own { .. }
        ),
        "{:?}",
        slot_shown(&leaf(&m), SlotKind::Normal)
    );
    select_set(&mut m, LEAF);
    let h = harness(m);
    assert!(h.query_by_label_contains("Left out of the JSON").is_none());
}

/// R5: with slot 1 linked, the JSON names the linked albedo, so the mesh keeps nothing and the no-albedo Info row is
/// not raised.
#[test]
fn with_a_linked_slot_1_the_no_albedo_info_is_not_raised() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    let keeps = |m: &AppModel| {
        sdc_gui::model::bottom::problems(m)
            .iter()
            .any(|p| p.text.starts_with("No albedo in this set"))
    };
    assert!(keeps(&m), "premise");
    link(&mut m, 1, WALL_ALBEDO);
    assert_eq!(
        entry(&m, LEAF)["slot1"],
        json!(member_game_path(&m, WALL_ALBEDO).unwrap()),
        "the JSON names the linked albedo"
    );
    assert!(!keeps(&m));
}

// ---- Task 15a fix round 1 (ruling 3b-F6): the planned-output index holds where each file is written ----

/// One layout of the index table: the project's PNGs (paths under the opened folder) and `sdc-project.toml`, the
/// producing output (its file name), the consumer's slot, the game location of the written DDS (`None`: under no
/// `textures` folder) and spellings that name no written file.
struct Layout {
    name: &'static str,
    files: &'static [&'static str],
    toml: &'static str,
    /// Files decided `pbr_albedo` after the folder is opened.
    albedos: &'static [&'static str],
    out_name: &'static str,
    slot: u8,
    real: Option<&'static str>,
    invented: &'static [&'static str],
    /// JSON settings set after the folder is opened: (set id, key, value).
    json: &'static [(&'static str, &'static str, &'static str)],
}

const LAYOUTS: &[Layout] = &[
    Layout {
        name: "plain own output",
        files: &["textures/p/plain.png"],
        toml: "",
        albedos: &[],
        out_name: "plain.dds",
        slot: 1,
        real: Some(r"textures\p\plain.dds"),
        invented: &[],
        json: &[],
    },
    Layout {
        name: "packed output",
        files: &["textures/s/q_roughness.png", "textures/s/q_ao.png"],
        toml: "",
        albedos: &[],
        out_name: "q_rmaos.dds",
        slot: 6,
        real: Some(r"textures\s\q_rmaos.dds"),
        invented: &[],
        json: &[],
    },
    Layout {
        name: "packed output mapped away, its set borrowing slot 6",
        // The lender is a PBR set, so the borrowed `_rmaos` has a path of its own (a borrow from a set that is not
        // PBR has none, and Export refuses it).
        files: &[
            "textures/s/q.png",
            "textures/s/q_roughness.png",
            "textures/s/q_ao.png",
            "textures/t/w_roughness.png",
            "textures/t/w_ao.png",
        ],
        toml: "[[sets]]\nid = \"textures/s/q\"\nshader = \"pbr\"\n[[sets]]\nid = \"textures/t/w\"\n[sets.borrow]\n\
               slot6 = \"textures/s/q\"\n[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"W\"\n\
               slot6 = \"textures/z/w_rmaos.dds\"\n",
        albedos: &["textures/s/q.png"],
        out_name: "w_rmaos.dds",
        slot: 6,
        real: Some(r"textures\z\w_rmaos.dds"),
        invented: &[r"textures\t\w_rmaos.dds"],
        json: &[],
    },
    Layout {
        name: "output mapped to another folder",
        files: &["textures/p/map.png"],
        toml: "[[sets]]\nid = \"textures/p/map\"\n[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"B\"\n\
               slot1 = \"textures/b/body.dds\"\n",
        albedos: &[],
        out_name: "body.dds",
        slot: 1,
        real: Some(r"textures\b\body.dds"),
        invented: &[r"textures\p\body.dds"],
        json: &[],
    },
    Layout {
        name: "output mapped under mod_a\\textures",
        files: &["mod_a/textures/m/x.png"],
        toml: "[[sets]]\nid = \"mod_a/textures/m/x\"\n[sets.txst]\nplugin = \"a.esp\"\nformid = \"1\"\nedid = \"X\"\n\
               slot1 = \"textures/m2/y.dds\"\n",
        albedos: &[],
        out_name: "y.dds",
        slot: 1,
        real: Some(r"textures\m2\y.dds"),
        invented: &[r"textures\mod_a\textures\m2\y.dds", r"textures\m\y.dds"],
        json: &[],
    },
    Layout {
        name: "legacy output_name outside textures",
        files: &["arch/leaf.png"],
        toml: "[[sets]]\nid = \"arch/leaf\"\noutput_name = \"maple\"\n",
        albedos: &[],
        out_name: "maple.dds",
        slot: 1,
        real: None,
        invented: &[r"textures\arch\maple.dds"],
        json: &[],
    },
    Layout {
        name: "a folder whose Unicode lower case differs (I with a dot)",
        files: &["textures/\u{130}/plain.png"],
        toml: "",
        albedos: &[],
        out_name: "plain.dds",
        slot: 1,
        real: Some("textures\\\u{130}\\plain.dds"),
        invented: &["textures\\i\u{307}\\plain.dds"],
        json: &[],
    },
    Layout {
        name: "a PBR rename with an interior '.'",
        files: &["textures/k/rock.png"],
        toml: "[[sets]]\nid = \"textures/k/rock\"\nshader = \"pbr\"\n",
        albedos: &["textures/k/rock.png"],
        out_name: "wall.dds",
        slot: 1,
        real: Some(r"textures\stone\wall.dds"),
        invented: &[],
        json: &[("textures/k/rock", "rename", r"textures\stone\.\wall")],
    },
    Layout {
        name: "two textures folders: the first wins",
        files: &["mod_a/textures/a/textures/b.png"],
        toml: "",
        albedos: &[],
        out_name: "b.dds",
        slot: 1,
        real: Some(r"textures\a\textures\b.dds"),
        invented: &[r"textures\b.dds"],
        json: &[],
    },
];

const CONSUMER: &str = "textures/c/con";

/// `layout` opened as the app opens a folder, with a PBR consumer `textures/c/con` (an albedo) in an excluded group,
/// so it writes an entry but no file and blocks nothing.
fn layout_project(dir: &std::path::Path, layout: &Layout) -> AppModel {
    for f in layout.files.iter().chain(&["textures/c/con.png"]) {
        let p = dir.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        image::ImageBuffer::from_fn(64, 64, |x, y| {
            image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
        })
        .save(&p)
        .unwrap();
    }
    let toml = format!(
        "{}[[sets]]\nid = \"{CONSUMER}\"\nshader = \"pbr\"\n",
        layout.toml
    );
    std::fs::write(dir.join("sdc-project.toml"), toml).unwrap();
    let mut m = opened_as_the_app(dir.to_path_buf(), dir.join("export"));
    for f in layout.albedos.iter().chain(&["textures/c/con.png"]) {
        m.decide(f, Answer::Subtype(Subtype::PbrAlbedo));
    }
    for (set, key, value) in layout.json {
        m.set_json_value(set, key, Some(json!(value)));
    }
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let ids = vec![CONSUMER.to_string()];
    m.move_to_group(
        &ids,
        sdc_gui::model::set_groups::GroupTarget::New("Consumer".into()),
    )
    .expect("grouped");
    m.set_group_excluded("Consumer", true).unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m
}

/// Every `.dds` under `root`, `/`-separated and relative to it.
fn dds_under(root: &std::path::Path) -> Vec<String> {
    let mut out = vec![];
    let mut todo = vec![root.to_path_buf()];
    while let Some(d) = todo.pop() {
        let entries = std::fs::read_dir(&d).unwrap_or_else(|e| panic!("{}: {e}", d.display()));
        for e in entries {
            let p = e.unwrap_or_else(|e| panic!("{}: {e}", d.display())).path();
            if p.is_dir() {
                todo.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dds")) {
                out.push(
                    p.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    out
}

/// The game location of `rel` (a path under the export folder): from its first `textures` folder on, lower case,
/// backslashes; `None` under no `textures` folder.
fn game_location(rel: &str) -> Option<String> {
    let parts: Vec<String> = rel
        .split(['/', '\\'])
        .map(str::to_ascii_lowercase)
        .collect();
    let i = parts.iter().position(|c| c == "textures")?;
    Some(parts[i..].join("\\"))
}

/// Ruling 3b-F6: for each layout, after a real export into a fresh folder, every index key names a DDS the export
/// wrote at that game location, every written DDS that has a game location is a key, and a consumer whose slot names
/// the real path gets the producing job as a dependency while one naming an invented spelling gets an external path.
#[test]
fn the_output_index_holds_where_each_file_is_written() {
    use sdc_gui::model::txst_map::written_output_index;
    for layout in LAYOUTS {
        let d = tempfile::tempdir().unwrap();
        let mut m = layout_project(d.path(), layout);
        let case = layout.name;
        let producer = m
            .items
            .iter()
            .find(|i| {
                i.job
                    .as_ref()
                    .is_some_and(|j| j.output.file_name().is_some_and(|n| n == layout.out_name))
            })
            .unwrap_or_else(|| panic!("{case}: premise: an item writes {}", layout.out_name))
            .clone();
        let set = views(&m)
            .iter()
            .find(|v| v.members.contains(&producer.key))
            .unwrap_or_else(|| panic!("{case}: premise: the producer has a set"))
            .id
            .clone();
        export_through(&mut m);
        assert!(m.wait_idle(std::time::Duration::from_secs(120)));
        let notes = m.export.notes.join("\n");
        assert!(
            m.export_root().is_dir(),
            "{case}: premise: the export ran: {notes} {:?} {}",
            block_texts(&m),
            m.status_line
        );
        let written = dds_under(&m.export_root());
        assert!(
            written.iter().any(|f| f.ends_with(layout.out_name)),
            "{case}: premise: the export wrote {}: {written:?} {notes}",
            layout.out_name
        );
        let index = written_output_index(&m);
        let keys: Vec<&str> = index.iter().map(|(k, _, _)| k.as_str()).collect();
        // The consumer is left out of this run, so its own planned output is a key with no file yet; the export at
        // the end includes it and checks every key.
        for (k, _, _) in index.iter().filter(|(_, s, _)| s != CONSUMER) {
            assert!(
                written
                    .iter()
                    .any(|f| game_location(f).as_deref() == Some(k.as_str())),
                "{case}: key {k} names no written DDS: {written:?}"
            );
        }
        for f in &written {
            if let Some(g) = game_location(f) {
                assert!(
                    keys.contains(&g.as_str()),
                    "{case}: {f} is no key: {keys:?}"
                );
            }
        }
        let con = |m: &AppModel| views(m).iter().find(|v| v.id == CONSUMER).unwrap().clone();
        let deps = |m: &AppModel| {
            sdc_gui::model::pbr_export::prepare(m)
                .and_then(|p| p.entries.into_iter().find(|(id, _, _)| id == CONSUMER))
                .map(|(_, deps, _)| deps)
                .unwrap_or_default()
        };
        let key = format!("slot{}", layout.slot);
        if let Some(real) = layout.real {
            m.set_json_value(CONSUMER, &key, Some(json!(real)));
            let l = con(&m).linked[&layout.slot].clone();
            assert_eq!(l.from.as_deref(), Some(set.as_str()), "{case}: {real}");
            assert!(
                deps(&m).contains(&producer.job_key),
                "{case}: {:?}",
                deps(&m)
            );
        } else {
            let entry = m
                .slot_menu(CONSUMER, SlotKind::Diffuse)
                .into_iter()
                .find(|e| matches!(&e.action, MenuAction::SlotLink { item, .. } if *item == producer.key))
                .unwrap_or_else(|| panic!("{case}: the picker lists {}", producer.key));
            assert!(entry.disabled.is_some(), "{case}: {entry:?}");
        }
        for wrong in layout.invented {
            m.set_json_value(CONSUMER, &key, Some(json!(wrong)));
            let l = con(&m).linked[&layout.slot].clone();
            assert_eq!(l.from, None, "{case}: {wrong} is external");
            assert!(!deps(&m).contains(&producer.job_key), "{case}: {wrong}");
        }
        // Ruling 3b-F11: with the consumer in the run as well, every index key names a written DDS.
        m.set_json_value(CONSUMER, &key, None);
        m.set_group_excluded("Consumer", false).unwrap();
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        export_through(&mut m);
        assert!(m.wait_idle(std::time::Duration::from_secs(120)));
        let written = dds_under(&m.export_root());
        let index = written_output_index(&m);
        assert!(
            index.iter().any(|(_, s, _)| s == CONSUMER),
            "{case}: premise: the consumer's output is a key: {index:?}"
        );
        for (k, _, _) in &index {
            assert!(
                written
                    .iter()
                    .any(|f| game_location(f).as_deref() == Some(k.as_str())),
                "{case}: with the consumer exported, key {k} names no written DDS: {written:?} {}",
                m.export.notes.join("\n")
            );
        }
    }
}
