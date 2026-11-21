//! Layout v2 A5 (R15): the set cards in the Sets column. One row for the handle, the name, the status pill and the
//! ⚠ button; a full-width shader; slot chips labelled by their suffix letter.
mod support;
use eframe::egui;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::Subtype;
use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::sets::{set_views, view_row, views, SlotCommand, SlotFill, SlotKind};
use sdc_gui::model::{AppModel, CentreTab, Selection};
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01";

/// `pbr_opened_plain` plus `rock01_metallic.png` and `rock01_specular.png`: four raw helpers on rock01's RMAOS.
fn four_helpers(dir: &std::path::Path) -> AppModel {
    four_helper_files(dir);
    let m = pbr_opened_plain(dir);
    let v = set_views(&m).into_iter().find(|v| v.id == ROCK).unwrap();
    let on_rmaos = sdc_gui::model::helpers::docked(&m, &v)
        .iter()
        .filter(|d| d.slot == SlotKind::Rmaos)
        .count();
    assert_eq!(on_rmaos, 4, "premise: four helpers fill rock01's RMAOS");
    m
}

fn four_helper_files(dir: &std::path::Path) -> std::path::PathBuf {
    let r = support::mod_folder(dir).join("landscape");
    for n in ["rock01_metallic.png", "rock01_specular.png"] {
        image::ImageBuffer::from_fn(64, 64, |x, _| image::Luma([x as u8 * 3]))
            .save(r.join(n))
            .unwrap();
    }
    r
}

/// Note JW7 (A9, "Never large"): one badge and "+3" on the RMAOS chip; the folded three are not drawn again as
/// tiles; "+3" is a button that lists them, and a row selects its helper.
#[test]
fn folded_helpers_sit_behind_plus_k_and_never_as_tiles() {
    use egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(four_helpers(d.path()));
    h.run();
    h.run();
    let card = card_of(&h, ROCK);

    // A node on the card named for one of the four helpers: the badge, or (before this task) a trailing tile.
    const HELPERS: [&str; 4] = [
        "rock01_roughness.png, ",
        "rock01_ao.png, ",
        "rock01_metallic.png, ",
        "rock01_specular.png, ",
    ];
    let helper_nodes = |h: &Harness<'static, AppModel>| {
        h.query_all(egui_kittest::kittest::by())
            .filter(|n| n.accesskit_node().raw_bounds().is_some() && card.contains_rect(n.rect()))
            .filter(|n| {
                n.accesskit_node()
                    .label()
                    .is_some_and(|l| HELPERS.iter().any(|f| l.starts_with(f)))
            })
            .count()
    };
    assert_eq!(
        helper_nodes(&h),
        1,
        "only the one badge names a helper on the card (today: 1 + 3 tiles)"
    );
    let more = h.get_by_role_and_label(
        Role::Button,
        "3 more helpers fill RMAOS in landscape/rock01",
    );
    assert!(card.contains_rect(more.rect()));
    more.click();
    h.run();
    let rows: Vec<String> = h
        .query_all_by_role(Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.starts_with("rock01_") && l.contains(", fills RMAOS"))
        .collect();
    assert_eq!(
        rows.len(),
        4,
        "the badge and the three rows of the list: {rows:?}"
    );
    let removes = h
        .query_all_by_role(Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.starts_with("Remove rock01_") && l.ends_with(" from set"))
        .count();
    assert_eq!(removes, 3);
    let remove = h
        .query_all_by_role(Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .find(|l| l.starts_with("Remove rock01_") && l.ends_with(" from set"))
        .expect("open popup");
    let file = remove["Remove ".len()..]
        .trim_end_matches(" from set")
        .to_string();
    h.get_by_label_contains(&format!("{file}, fills RMAOS"))
        .click();
    h.run();
    let key = h
        .state()
        .items
        .iter()
        .find(|i| i.name == file)
        .unwrap()
        .key
        .clone();
    assert_eq!(h.state().selection, sdc_gui::model::Selection::Item(key));
}

/// Note JW7: the ✕ of a list row removes that helper from the set, as one step.
#[test]
fn a_folded_helper_is_removed_from_its_row() {
    use egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(four_helpers(d.path()));
    h.run();
    h.run();
    let steps = h.state().history.undo.len();
    h.get_by_role_and_label(
        Role::Button,
        "3 more helpers fill RMAOS in landscape/rock01",
    )
    .click();
    h.run();
    let x = h
        .query_all_by_role(Role::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Remove rock01_"))
        })
        .expect("a ✕");
    let file = x.accesskit_node().label().unwrap()["Remove ".len()..]
        .trim_end_matches(" from set")
        .to_string();
    x.click();
    h.run();
    let m = h.state_mut();
    idle(m);
    let key = m.items.iter().find(|i| i.name == file).unwrap().key.clone();
    let v = set_views(m).into_iter().find(|v| v.id == ROCK).unwrap();
    assert!(!v.members.contains(&key), "{file} left rock01");
    assert_eq!(m.history.undo.len(), steps + 1);
    m.undo();
    idle(m);
    assert!(set_views(m)
        .iter()
        .find(|v| v.id == ROCK)
        .unwrap()
        .members
        .contains(&key));
}

/// Note JW7: a row of the "+K" list drags its helper as a texture, and the list stays open after a click on a row.
#[test]
fn a_folded_helper_drags_from_its_row() {
    use egui::accesskit::Role;
    use sdc_gui::ui::sets_column::DragItem;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(four_helpers(d.path()));
    h.run();
    h.run();
    h.get_by_role_and_label(
        Role::Button,
        "3 more helpers fill RMAOS in landscape/rock01",
    )
    .click();
    h.run();
    let remove = h
        .query_all_by_role(Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .find(|l| l.starts_with("Remove rock01_") && l.ends_with(" from set"))
        .expect("open popup");
    let file = remove["Remove ".len()..]
        .trim_end_matches(" from set")
        .to_string();
    let label = format!("{file}, fills RMAOS");
    h.get_by_label_contains(&label).click();
    h.run();
    h.get_by_role_and_label(Role::Button, &remove);
    let at = h.get_by_label_contains(&label).rect().center();
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.hover_at(at);
    h.run();
    h.event(press(true));
    h.run();
    for step in 1..=4 {
        h.event(egui::Event::PointerMoved(
            at + egui::vec2(-6.0 * step as f32, 0.0),
        ));
        h.run_steps(2);
    }
    let key = h
        .state()
        .items
        .iter()
        .find(|i| i.name == file)
        .unwrap()
        .key
        .clone();
    let got = egui::DragAndDrop::payload::<DragItem>(&h.ctx).map(|p| p.0.clone());
    egui::DragAndDrop::clear_payload(&h.ctx);
    h.event(press(false));
    h.run();
    assert_eq!(got, Some(key), "the row drags its helper");
}

#[test]
fn a_closed_unhovered_overflow_builds_no_display_names() {
    use sdc_gui::ui::paint_counts::Work::FoldedNames;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(four_helpers(d.path()));
    h.run();
    let ctx = h.ctx.clone();
    assert!(!egui::Popup::is_any_open(&ctx));
    assert!(ctx.input(|i| i.pointer.hover_pos().is_none()));
    // kittest requires an AccessKit tree for its own steps. Draw these two ordinary frames directly.
    ctx.disable_accesskit();
    let before = FoldedNames.count();
    for _ in 0..2 {
        let mut out = ctx.run_ui(Default::default(), |ui| {
            sdc_gui::ui::draw(ui, h.state_mut())
        });
        out.textures_delta.clear();
        assert!(out
            .shapes
            .iter()
            .any(|s| matches!(&s.shape, egui::Shape::Text(t)
            if t.galley.job.text == "+3")));
    }
    assert_eq!(
        FoldedNames.count(),
        before,
        "two idle frames built folded names"
    );
    ctx.enable_accesskit();
    h.run_steps(2);
    let before_open = FoldedNames.count();
    h.get_by_role_and_label(
        egui::Role::Button,
        "3 more helpers fill RMAOS in landscape/rock01",
    )
    .click();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, "Remove rock01_metallic.png from set");
    assert!(
        FoldedNames.count() > before_open,
        "the open list builds its names"
    );
}

#[test]
fn folded_helpers_are_grouped_once_per_change() {
    use sdc_gui::ui::paint_counts::Work::{FoldedGroups, FoldedItemLookups};
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(four_helpers(d.path()));
    h.run();
    let more = "3 more helpers fill RMAOS in landscape/rock01";
    let description = |h: &Harness<'static, AppModel>| {
        h.get_by_role_and_label(egui::Role::Button, more)
            .accesskit_node()
            .description()
            .unwrap()
    };
    assert!(description(&h).contains("rock01_metallic.png, fills RMAOS G"));
    assert!(!egui::Popup::is_any_open(&h.ctx));
    assert!(h.ctx.input(|i| i.pointer.hover_pos().is_none()));
    let counts = || (FoldedGroups.count(), FoldedItemLookups.count());
    let before = counts();
    h.run_steps(2);
    assert_eq!(counts(), before, "two idle frames regrouped hidden helpers");

    let model = h.state_mut();
    let mut checks = model.settings.checks;
    checks.parallax = !checks.parallax;
    model.set_checks(checks);
    assert!(model.poll(), "the changed artist checks completed");
    h.run_steps(1);
    assert!(description(&h).contains("rock01_metallic.png, fills RMAOS G"));
    assert_eq!(counts(), before, "artist checks regrouped hidden helpers");

    h.state_mut()
        .edit_docs("Swap two folded channels", None, true, |p, _| {
            for (file, fill) in [
                ("landscape/rock01_metallic.png", "rmaos.r"),
                ("landscape/rock01_roughness.png", "rmaos.g"),
            ] {
                p.set_file_value(
                    file,
                    engine::helper::FILLS_FIELD,
                    &serde_json::json!([fill]),
                );
            }
        });
    idle(h.state_mut());
    h.run_steps(2);
    assert!(description(&h).contains("rock01_metallic.png, fills RMAOS R"));
    assert_eq!(
        counts(),
        (before.0 + 1, before.1 + 3),
        "one rebuild for the three hidden files"
    );
}

#[test]
fn overflow_name_identifies_each_visible_set_with_the_same_count_and_slot() {
    let d = tempfile::tempdir().unwrap();
    let r = four_helper_files(d.path());
    for tail in ["", "_normal", "_roughness", "_ao", "_metallic", "_specular"] {
        std::fs::copy(
            r.join(format!("rock01{tail}.png")),
            r.join(format!("rock02{tail}.png")),
        )
        .unwrap();
    }
    let mut m = pbr_opened_plain(d.path());
    let other = "landscape/rock02";
    m.decide(
        "landscape/rock02.png",
        sdc_gui::model::Answer::Subtype(Subtype::PbrAlbedo),
    );
    m.set_shader(other, Some(engine::shader::Shader::Pbr));
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    let labels = [ROCK, other].map(|set| {
        let card = card_of(&h, set);
        let hits: Vec<_> = h
            .query_all_by_role(egui::Role::Button)
            .filter(|n| card.contains_rect(n.rect()))
            .filter_map(|n| n.accesskit_node().label())
            .filter(|name| name.starts_with("3 more helpers fill RMAOS"))
            .collect();
        assert_eq!(hits.len(), 1, "one three-helper overflow on {set}");
        hits.into_iter().next().unwrap()
    });
    assert_eq!(
        labels,
        [ROCK, other].map(|set| format!("3 more helpers fill RMAOS in {set}"))
    );
}

#[test]
fn overflow_name_uses_the_singular_for_one_helper_filling_two_channels() {
    use engine::helper::FillTarget;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    m.set_helper_fills("landscape/rock01_ao.png", Some(vec![]));
    m.set_helper_fills(
        "landscape/rock01_roughness.png",
        Some(vec![
            FillTarget::Packed {
                suffix: "rmaos",
                channel: 0,
            },
            FillTarget::Packed {
                suffix: "rmaos",
                channel: 1,
            },
        ]),
    );
    m.edit_docs("Lock one RMAOS corner", None, false, |_, s| {
        s.set_lock(ROCK, 6, true)
    });
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    let card = card_of(&h, ROCK);
    let names: Vec<_> = h
        .query_all_by_role(egui::Role::Button)
        .filter(|n| card.contains_rect(n.rect()))
        .filter_map(|n| n.accesskit_node().label())
        .filter(|name| name.starts_with("1 more helper"))
        .collect();
    assert_eq!(names, ["1 more helper fills RMAOS in landscape/rock01"]);
}

/// Note JW7 (A9-2): the "+K" hover lists the folded helpers and ends with "Click to list them.".
#[test]
fn plus_k_says_it_lists_them() {
    use egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(four_helpers(d.path()));
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.run();
    h.run();
    let more = h
        .get_by_role_and_label(
            Role::Button,
            "3 more helpers fill RMAOS in landscape/rock01",
        )
        .rect();
    h.hover_at(more.center());
    h.run();
    h.get_by_label_contains("Click to list them.");
}

fn folded_ao_files(dir: &std::path::Path, count: usize) -> AppModel {
    let root = mod_folder(dir);
    let keys: Vec<_> = (0..count - 2)
        .map(|n| format!("layer{n}/rock01_ao.png"))
        .collect();
    for key in &keys {
        let path = root.join(key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(root.join("landscape/rock01_ao.png"), path).unwrap();
    }
    let mut m = pbr_opened_plain(dir);
    m.edit_docs("Dock the helpers on one locked chip", None, true, |_, s| {
        s.set_lock(ROCK, 6, true);
        for key in &keys {
            s.add_member(ROCK, key);
        }
    });
    idle(&mut m);
    if count == 4 {
        use engine::{config::RawMap, helper::HelperRole};
        for (key, role) in keys.iter().zip([RawMap::Metallic, RawMap::Specular]) {
            m.set_helper_role(key, Some(HelperRole::Raw(role)));
        }
        idle(&mut m);
    } else {
        // A four-channel map cannot supply ten distinct fills. Exercise the renderer with that many reported rows.
        for item in &mut m.items {
            if keys.contains(&item.key) {
                let report = std::sync::Arc::make_mut(item.helper.as_mut().unwrap());
                report.fills = vec![engine::helper::FillTarget::Packed {
                    suffix: "rmaos",
                    channel: 2,
                }];
            }
        }
        m.invalidate_views();
    }
    m
}

#[test]
fn folded_helpers_with_matching_names_use_project_paths() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(folded_ao_files(d.path(), 4));
    h.run();
    h.get_by_label("4 more helpers fill RMAOS in landscape/rock01")
        .click();
    h.run();
    for dir in ["layer0", "layer1", "landscape"] {
        let path = format!("{dir}/rock01_ao.png");
        h.get_by_role_and_label(egui::Role::Button, &format!("Remove {path} from set"));
        assert_eq!(
            h.query_all_by_label_contains(&format!("{path}, fills RMAOS"))
                .filter(|n| n.accesskit_node().role() == egui::Role::Button)
                .count(),
            1
        );
    }
}

#[test]
fn folded_overflow_paints_nine_then_only_plus_at_ten() {
    for (count, expected) in [(9, "+9"), (10, "+")] {
        let d = tempfile::tempdir().unwrap();
        let mut h = harness(folded_ao_files(d.path(), count));
        h.run();
        let cell = h
            .get_by_label(&format!("{count} more helpers fill RMAOS in {ROCK}"))
            .rect();
        let labels: Vec<_> = shapes(&h)
            .into_iter()
            .filter_map(|s| {
                let egui::Shape::Text(t) = s else { return None };
                cell.contains(t.galley.rect.translate(t.pos.to_vec2()).center())
                    .then(|| t.galley.job.text.clone())
            })
            .collect();
        assert_eq!(labels, [expected], "{count} folded helpers");
    }
}

/// Note JW7: on a read-only project file the ✕ of a list row is off, with the read-only reason on hover.
#[test]
fn a_folded_helpers_remove_is_off_when_the_project_is_read_only() {
    use egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let mut m = four_helpers(d.path());
    m.save().unwrap();
    let root = m.root.clone().unwrap();
    std::fs::write(root.join("sdc-project.toml"), "[[sets]\nbroken").unwrap();
    m.reload_documents();
    idle(&mut m);
    let v = set_views(&m).into_iter().find(|v| v.id == ROCK).unwrap();
    assert_eq!(
        sdc_gui::model::helpers::docked(&m, &v)
            .iter()
            .filter(|d| d.slot == SlotKind::Rmaos)
            .count(),
        4
    );
    let why = m
        .sets_read_only_reason()
        .expect("premise: the project file is read-only");
    let mut h = harness(m);
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.run();
    h.run();
    h.get_by_role_and_label(
        Role::Button,
        "3 more helpers fill RMAOS in landscape/rock01",
    )
    .click();
    h.run();
    let x = h
        .query_all_by_role(Role::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Remove rock01_"))
        })
        .expect("a ✕");
    assert_eq!(
        x.accesskit_node().description().as_deref(),
        Some(why.as_str())
    );
    assert!(x.accesskit_node().is_disabled(), "the ✕ is off");
    let at = x.rect().center();
    // The reason may already be drawn elsewhere (a banner, a Problems row), so the hover is one more node.
    let shown = h.query_all_by_label(&why).count();
    // egui suppresses a tooltip if the pointer moved within 0.1 s of the click that opened the list.
    h.run_steps(6);
    h.hover_at(at);
    h.run();
    assert!(
        h.query_all_by_label(&why).count() > shown,
        "the ✕'s hover gives the reason"
    );
}

#[test]
fn a_folded_helper_with_two_channels_has_one_remove() {
    use egui::accesskit::Role;
    use engine::helper::FillTarget;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    let key = "landscape/rock01_roughness.png";
    let packed = |channel| FillTarget::Packed {
        suffix: "rmaos",
        channel,
    };
    m.set_helper_fills(key, Some(vec![packed(0), packed(1)]));
    idle(&mut m);
    m.edit_docs("Lock one RMAOS corner", None, false, |_, s| {
        s.set_lock(ROCK, 6, true)
    });
    idle(&mut m);
    let v = set_views(&m).into_iter().find(|v| v.id == ROCK).unwrap();
    let dock = sdc_gui::model::helpers::docked(&m, &v);
    let on_chip: Vec<_> = dock.iter().filter(|d| d.slot == SlotKind::Rmaos).collect();
    assert_eq!(on_chip.len(), 3, "roughness R and G, plus AO");
    assert_eq!(
        on_chip
            .iter()
            .filter(|d| d.helper == key)
            .map(|d| d.target)
            .collect::<Vec<_>>(),
        [packed(0), packed(1)]
    );
    let mut h = harness(m);
    h.run();
    h.run();
    let more = h.get_by_role_and_label(
        Role::Button,
        "2 more helpers fill RMAOS in landscape/rock01",
    );
    assert!(card_of(&h, ROCK).contains_rect(more.rect()));
    assert!(!egui::Popup::is_any_open(&h.ctx));
    assert_eq!(
        more.accesskit_node().description().as_deref(),
        Some("rock01_ao.png, fills RMAOS B\nrock01_roughness.png, fills RMAOS R, RMAOS G\nClick to list them."),
        "the closed overflow describes its files and their grouped fills"
    );
    more.click();
    h.run();
    let rows: Vec<_> = h
        .query_all_by_role(Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|name| name.starts_with("rock01_roughness.png, fills "))
        .collect();
    assert_eq!(rows, ["rock01_roughness.png, fills RMAOS R, RMAOS G"]);
    assert_eq!(
        h.query_all_by_role_and_label(Role::Button, "Remove rock01_roughness.png from set")
            .count(),
        1
    );
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(h
        .query_by_label("Remove rock01_roughness.png from set")
        .is_none());
    let more = h.get_by_role_and_label(
        Role::Button,
        "2 more helpers fill RMAOS in landscape/rock01",
    );
    let at = more.rect();
    assert!(shapes(&h).iter().any(|s| matches!(s, egui::Shape::Text(t)
        if t.galley.job.text == "+2" && at.contains(t.visual_bounding_rect().center()))));
    more.hover();
    h.run();
    h.get_by_label("rock01_roughness.png, fills RMAOS R, RMAOS G");
    h.get_by_label("Click to list them.");
}

fn helper_tiles(h: &Harness<'static, AppModel>, card: egui::Rect, file: &str) -> Vec<egui::Rect> {
    let prefix = format!("rock01_{file}.png, ");
    h.query_all_by_role(egui::Role::Button)
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|s| s.starts_with(&prefix))
        })
        .map(|n| n.rect())
        .filter(|r| card.contains_rect(*r))
        .collect()
}

/// Note JW7: one free corner lists all four files without trailing copies.
#[test]
fn folded_helpers_follow_the_free_corners() {
    use egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let mut m = four_helpers(d.path());
    m.edit_docs("Lock one RMAOS corner", None, false, |_, s| {
        s.set_lock(ROCK, 6, true)
    });
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    let card = card_of(&h, ROCK);
    for file in ["roughness", "metallic", "ao", "specular"] {
        assert!(
            helper_tiles(&h, card, file).is_empty(),
            "trailing copy of {file}"
        );
    }
    h.get_by_role_and_label(
        Role::Button,
        "4 more helpers fill RMAOS in landscape/rock01",
    )
    .click();
    h.run();
    for file in ["roughness", "metallic", "ao", "specular"] {
        h.get_by_role_and_label(Role::Button, &format!("Remove rock01_{file}.png from set"));
        h.get_by_label_contains(&format!("rock01_{file}.png, fills RMAOS"));
    }
    on_card(&h, ROCK)[0].click();
    h.run();
    assert!(h
        .query_by_label("Remove rock01_roughness.png from set")
        .is_none());
}

/// Note JW7: with both corners locked, each file has its own small hit in the trailing row.
#[test]
fn folded_helpers_without_a_free_corner_stay_on_the_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = four_helpers(d.path());
    m.edit_docs("Lock one RMAOS corner", None, false, |_, s| {
        s.set_lock(ROCK, 6, true)
    });
    idle(&mut m);
    m.set_pgp_lock(ROCK, 6, true).unwrap();
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    assert!(h
        .query_by_label_contains("more helpers fill RMAOS")
        .is_none());
    let card = card_of(&h, ROCK);
    for file in ["roughness", "metallic", "ao", "specular"] {
        let tiles = helper_tiles(&h, card, file);
        assert_eq!(tiles.len(), 1, "one tile for {file}");
        assert_eq!(tiles[0].size(), egui::Vec2::splat(20.0), "{file}");
    }
}

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 1000.0))
        .with_step_dt(0.02)
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h
}

/// rock01's name on its card.
fn name_rect(h: &Harness<'static, AppModel>) -> egui::Rect {
    name_of(h, ROCK)
}

/// Set `id`'s name on its card: the leftmost label (the Sets column is left of the set editor).
fn name_of(h: &Harness<'static, AppModel>, id: &str) -> egui::Rect {
    h.query_all_by_label(id)
        .map(|n| n.rect())
        .min_by(|a, b| a.min.x.total_cmp(&b.min.x))
        .unwrap_or_else(|| panic!("{id}'s card name"))
}

/// rock01's card.
fn card_rect(h: &Harness<'static, AppModel>) -> egui::Rect {
    card_of(h, ROCK)
}

/// A drawn card has its position label inside the full-id card node.
fn card_of(h: &Harness<'static, AppModel>, id: &str) -> egui::Rect {
    let prefix = format!("Set {id}, position ");
    let card = h.get_by_label_contains(&prefix);
    let label = card.accesskit_node().label().unwrap();
    let position = format!("Position {}", label.strip_prefix(&prefix).unwrap());
    assert!(
        h.query_all_by_label(&position)
            .any(|n| card.rect().contains_rect(n.rect())),
        "{id} is drawn"
    );
    card.rect()
}

/// The nodes labelled `label` inside rock01's card (K11: every card has a "D").
fn on_card<'a>(h: &'a Harness<'static, AppModel>, label: &'a str) -> Vec<egui_kittest::Node<'a>> {
    on_card_of(h, ROCK, label)
}

fn on_card_of<'a>(
    h: &'a Harness<'static, AppModel>,
    id: &str,
    label: &'a str,
) -> Vec<egui_kittest::Node<'a>> {
    let card = card_of(h, id);
    h.query_all_by_label(label)
        .filter(|n| card.contains_rect(n.rect()))
        .collect()
}

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

const DIFFUSE: &str = "landscape/rock01.png";

fn rock_selection(m: &AppModel) -> Selection {
    Selection::Set(views(m).iter().find(|v| v.id == ROCK).unwrap().key.clone())
}

#[test]
fn a_plain_click_on_a_thumbnail_opens_its_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.select_item(DIFFUSE);
    let set = rock_selection(&m);
    let mut h = harness(m);
    h.run();
    on_card(&h, "D")[0].click();
    h.run();
    assert_eq!(h.state().selection, set);
    assert_in_centre(&h, "Slot commands…");
}

#[test]
fn shift_click_on_a_thumbnail_opens_its_texture() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    for already_selected in [false, true] {
        h.state_mut().open_presets();
        h.run();
        let item = Selection::Item(DIFFUSE.into());
        assert_eq!(h.state().selection == item, already_selected);
        assert_eq!(h.state().centre_front(), CentreTab::Presets);
        on_card(&h, "D")[0].click_modifiers(egui::Modifiers::SHIFT);
        h.run();
        assert_eq!(h.state().selection, item);
        assert_eq!(h.state().centre_front(), CentreTab::Selection);
        let tick = h
            .get_by_role_and_label(egui::Role::CheckBox, "Mipmaps")
            .rect();
        assert!(sdc_gui::ui::centre_rect(&h.ctx)
            .unwrap()
            .contains_rect(tick));
        h.run_steps(30); // a second single click on the selected texture
    }
}

#[test]
fn ctrl_click_on_a_thumbnail_toggles_its_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let cards = views(&m);
    let rock = &cards.iter().find(|v| v.id == ROCK).unwrap().key;
    let other = &cards
        .iter()
        .find(|v| v.id != ROCK && !v.key.is_empty())
        .unwrap()
        .key;
    m.select_set(other);
    let mut h = harness(m);
    h.run();
    for include in [true, false] {
        on_card(&h, "D")[0].click_modifiers(egui::Modifiers::CTRL | egui::Modifiers::COMMAND);
        h.run();
        let selected = h.state().selected_sets();
        assert!(selected.contains(other), "keep the other set");
        assert_eq!(selected.contains(rock), include);
        assert_eq!(selected.len(), if include { 2 } else { 1 });
        h.run_steps(30); // separate single clicks
    }
}

#[test]
fn keyboard_on_a_thumbnail_follows_shift() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    let set = rock_selection(h.state());
    let texture = Selection::Item(DIFFUSE.into());
    h.state_mut().select_item(DIFFUSE);
    h.run();
    for key in [egui::Key::Enter, egui::Key::Space] {
        for (mods, expected) in [
            (egui::Modifiers::NONE, &set),
            (egui::Modifiers::SHIFT, &texture),
        ] {
            h.get_by_label_contains("Set landscape/rock01, position ")
                .focus();
            h.run();
            for _ in 0..20 {
                h.key_press(egui::Key::Tab);
                h.run();
                if on_card(&h, "D")[0].is_focused() {
                    break;
                }
            }
            assert!(
                on_card(&h, "D")[0].is_focused(),
                "Tab reached the thumbnail"
            );
            h.key_press_modifiers(mods, key);
            h.run();
            assert_eq!(&h.state().selection, expected, "{mods:?} + {key:?}");
        }
    }
}

#[test]
fn helper_thumbnails_follow_shift() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    m.set_helper_fills("landscape/rock01_ao.png", Some(vec![]));
    idle(&mut m);
    let set = rock_selection(&m);
    let mut h = harness(m);
    h.run();
    for file in ["roughness", "ao"] {
        let key = format!("landscape/rock01_{file}.png");
        h.state_mut().select_item(&key);
        h.run();
        for (mods, expected) in [
            (egui::Modifiers::NONE, set.clone()),
            (egui::Modifiers::SHIFT, Selection::Item(key.clone())),
        ] {
            let card = card_rect(&h);
            let prefix = format!("rock01_{file}.png, ");
            let n = h
                .query_all_by_label_contains(&prefix)
                .find(|n| card.contains_rect(n.rect()))
                .unwrap();
            n.click_modifiers(mods);
            h.run();
            assert_eq!(h.state().selection, expected, "{file}, {mods:?}");
            h.run_steps(30);
        }
    }
}

#[test]
fn thumbnail_hover_and_description_explain_shift() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    h.run();
    let chip = &on_card(&h, "D")[0];
    let hint = "Shift+click to open its settings.";
    assert!(
        chip.accesskit_node()
            .description()
            .unwrap_or_default()
            .contains(hint),
        "thumbnail description"
    );
    chip.hover();
    h.run();
    assert!(h.query_by_label_contains(hint).is_some(), "thumbnail hover");
}

#[test]
fn the_overflow_list_still_opens_a_texture() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(four_helpers(d.path()));
    h.run();
    h.get_by_label("3 more helpers fill RMAOS in landscape/rock01")
        .click();
    h.run();
    h.get_by_label_contains("rock01_metallic.png, fills RMAOS")
        .click();
    h.run();
    assert_eq!(
        h.state().selection,
        Selection::Item("landscape/rock01_metallic.png".into())
    );
}

#[test]
fn the_idle_sets_column_checks_bulk_membership_once_for_each_visible_summary_and_banner() {
    let d = tempfile::tempdir().unwrap();
    let mut m = many_sets(d.path(), 64);
    let cards = views(&m);
    m.click_set(&cards[0].key, false, false);
    m.click_set(&cards[62].key, false, true);
    m.click_set(&cards[63].key, true, false);
    assert_eq!(m.selected_sets().last(), Some(&cards[63].key));
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(440.0, 900.0))
        .build_ui_state(
            |ui, (m, state): &mut (AppModel, sdc_gui::ui::State)| {
                sdc_gui::ui::sets_column::draw(
                    ui,
                    m,
                    &mut state.thumbs,
                    &mut state.drops,
                    &mut state.slot_cmds,
                );
            },
            (m, sdc_gui::ui::State::default()),
        );
    h.run();
    h.get_by_label("64 selected");
    let before = h.state().0.bulk_membership_scans.get();
    h.run_steps(5);
    let scans = h.state().0.bulk_membership_scans.get() - before;
    assert!(
        scans <= 10,
        "five idle frames scanned bulk membership {scans} times"
    );
}

#[test]
fn a_card_shows_name_pill_and_a_right_aligned_shader() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    h.run();
    // The default column is too narrow for half a card to hold "PBR Landscape (auto)": widen it, as a drag leaves it.
    h.ctx.data_mut(|d| {
        d.insert_persisted(sdc_gui::ui::panel_ids::chosen_width_id("sets"), 460.0_f32)
    });
    h.run();
    let card = card_rect(&h);
    let kind = on_card(&h, "PBR Landscape");
    assert_eq!(kind.len(), 1, "the kind text");
    let (name, kind) = (name_rect(&h), kind[0].rect());
    // Note FI13: the heading is one row only when the name keeps 80 px beside the pill and badges. rock01's pill and
    // two badges do not leave that at the default width, so the pill is on the row below; either way the name is whole.
    assert!(
        name.width() >= 80.0 && (kind.min.y > name.max.y || kind.min.x > name.max.x),
        "the name keeps its room at the default width: the pill {kind:?}, the name {name:?}"
    );
    let combo = h
        .query_all_by_role(egui::accesskit::Role::ComboBox)
        .find(|n| card.contains_rect(n.rect()))
        .expect("the card's shader");
    assert_eq!(
        combo.accesskit_node().value().as_deref(),
        Some("PBR Landscape (auto)")
    );
    let w = combo.rect().width();
    assert!(
        w < card.width() / 2.0,
        "the shader ({w}) is under half the card ({})",
        card.width()
    );
    assert!(
        combo.rect().min.y >= kind.max.y - 0.5,
        "the shader row is under the pill"
    );
    assert!(
        (card.max.x - combo.rect().max.x).abs() <= 8.0,
        "the shader's right edge {} is the card's inner right {}",
        combo.rect().max.x,
        card.max.x
    );
    let texts: Vec<_> = shapes(&h)
        .into_iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) if t.galley.job.text.starts_with("PBR Landscape (") => Some(t),
            _ => None,
        })
        .collect();
    assert!(!texts.is_empty(), "the shader's text is painted");
    assert!(
        texts.iter().all(|t| !t.galley.elided),
        "the shader's text is not cut to \"PBR Landsc…\""
    );
}

/// Note JW1 (A4-1, A4-3): the filled slots have one chip each; the empty ones have none at rest, and the card's
/// description and the pill's hover list them.
#[test]
fn slot_chips_are_labelled_and_empty_ones_hide_at_rest() {
    const LINE: &str = "Empty: G, P, CNR, S";
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened_plain(d.path());
    let v = set_views(&m).into_iter().find(|v| v.id == ROCK).unwrap();
    let row = sdc_gui::model::sets::view_row(&v);
    assert!(
        row.iter().any(|c| c.kind == SlotKind::Height
            && matches!(
                sdc_gui::model::sets::slot_shown(&v, c.kind),
                sdc_gui::model::sets::SlotShown::Empty
            )),
        "{row:?}"
    );
    assert_eq!(v.empty_line, LINE);
    let pill = sdc_gui::model::sets::kind_text(&m, &v);
    let mut h = harness(m);
    h.run();
    let card = card_of(&h, ROCK);
    for l in ["D", "N", "RMAOS"] {
        let prefix = format!("{l}, ");
        let named = h
            .query_all_by_label_contains(&prefix)
            .filter(|n| card.contains_rect(n.rect()))
            .filter(|n| {
                n.accesskit_node()
                    .label()
                    .unwrap_or_default()
                    .starts_with(&prefix)
            })
            .count();
        assert_eq!(
            on_card(&h, l).len() + named,
            1,
            "one {l:?} chip on rock01's card"
        );
    }
    for l in ["G", "P", "CNR", "S"] {
        assert!(
            on_card(&h, &format!("{l}, empty")).is_empty(),
            "no {l} chip at rest"
        );
        assert!(on_card(&h, l).is_empty(), "no {l} chip at rest");
    }
    assert_eq!(empty_letters(&h, ROCK), ["G", "P", "CNR", "S"]);
    let status = h
        .query_all_by_role_and_label(egui::accesskit::Role::Label, pill)
        .find(|n| card.contains_rect(n.rect()))
        .expect("the status pill");
    status.hover();
    h.run();
    h.run();
    let tooltip = h
        .get_by_label_contains(LINE)
        .accesskit_node()
        .value()
        .expect("the pill's hover text");
    assert_eq!(tooltip.lines().last(), Some(LINE));
}

/// Note JW1 (A4-3, spec detail 4): an off-screen card's stand-in node carries the same "Empty: …" description.
#[test]
fn a_stand_in_carries_the_empty_line() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    let seed = set_views(&m).into_iter().find(|v| v.id == ROCK).unwrap();
    assert_eq!(seed.empty_line, "Empty: G, P, CNR, S");
    let rock = m.sets.iter().find(|s| s.key == seed.key).unwrap().clone();
    m.sets = (0..60)
        .map(|i| {
            let mut s = rock.clone();
            if i > 0 {
                s.key = format!("{}{i:04}", rock.key);
            }
            s
        })
        .collect();
    m.invalidate_views();
    let all = set_views(&m);
    let total = all.len();
    let (index, last) = all
        .iter()
        .enumerate()
        .find(|(_, v)| v.key == format!("{}0059", rock.key))
        .unwrap();
    assert!(index >= 50 && last.empty_line == seed.empty_line);
    let mut h = many(m);
    h.run();
    assert!(
        pos_rect(&h, index + 1, total).is_none(),
        "premise: the last card is a stand-in, not drawn"
    );
    let prefix = format!("Set {}, position ", last.id);
    let node = h
        .query_all(egui_kittest::kittest::by())
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with(&prefix))
        })
        .expect("the stand-in's node");
    assert_eq!(
        node.accesskit_node().description().as_deref(),
        Some(last.empty_line.as_str())
    );
}

/// The letters of a card's "Empty: …" description (Note JW1).
fn empty_letters(h: &Harness<'static, AppModel>, id: &str) -> Vec<String> {
    let prefix = format!("Set {id}, position ");
    let card = h.get_by_label_contains(&prefix);
    card.accesskit_node()
        .description()
        .and_then(|d| {
            d.strip_prefix("Empty: ")
                .map(|s| s.split(", ").map(str::to_string).collect())
        })
        .unwrap_or_default()
}

#[test]
fn the_open_set_is_highlighted() {
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let key = set_views(&m)
        .into_iter()
        .find(|v| v.id == ROCK)
        .unwrap()
        .key;
    let mut h = harness(m);
    let fill = h
        .ctx
        .global_style()
        .visuals
        .selection
        .bg_fill
        .gamma_multiply(0.35);
    let lit = |h: &Harness<'static, AppModel>| {
        let name = name_rect(h);
        shapes(h).iter().any(
            |s| matches!(s, egui::Shape::Rect(r) if r.fill == fill && r.rect.contains_rect(name)),
        )
    };
    h.run();
    assert!(!lit(&h), "a set that is not open is not highlighted");
    h.state_mut().select_set(&key);
    h.run();
    assert!(
        lit(&h),
        "the open set's card is filled with the selection colour"
    );
}

#[test]
fn drag_double_click_and_menu_are_unchanged() {
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let diffuse = m
        .items
        .iter()
        .find(|i| i.name == "rock01.png")
        .unwrap()
        .key
        .clone();
    let mut h = harness(m);
    h.run();
    for mods in [egui::Modifiers::NONE, egui::Modifiers::SHIFT] {
        let chip = &on_card(&h, "D")[0];
        chip.click_modifiers(mods);
        chip.click_modifiers(mods);
        h.run();
        assert_eq!(h.state().expanded.as_deref(), Some(diffuse.as_str()));
        h.state_mut().expanded = None;
        h.run_steps(30);
    }
    on_card(&h, "D")[0].click_secondary();
    h.run();
    h.get_by_label("Slot 1: Diffuse");
    h.key_press(egui::Key::Escape);
    h.run();
    let source = on_card(&h, "D")[0].rect().center();
    let other = views(h.state())
        .iter()
        .find(|v| v.id != ROCK && !v.key.is_empty())
        .unwrap()
        .id
        .clone();
    let dest = name_of(&h, &other).center();
    start_drag(&mut h, source);
    h.hover_at(dest);
    h.run_steps(2);
    h.event(egui::Event::PointerButton {
        pos: dest,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert!(
        h.query_by_label("Move it here").is_some(),
        "drop on {other}"
    );
}

#[test]
fn the_long_badge_text_is_on_the_pill_hover() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    // On disk but no texture, so no plan lists it and the chip stays (a deleted file's line clears: Note HE2).
    std::fs::write(m.root.clone().unwrap().join("landscape/notes.txt"), "x").unwrap();
    m.edit_docs("t", None, true, |_, s| {
        s.add_member(ROCK, "landscape/notes.txt")
    });
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    use sdc_gui::model::sets::BADGE_PBR_LANDSCAPE_PGP_ONLY;
    assert!(h
        .query_by_label_contains(BADGE_PBR_LANDSCAPE_PGP_ONLY)
        .is_none());
    on_card(&h, "PBR Landscape")[0].hover();
    h.run();
    h.run();
    h.get_by_label_contains(BADGE_PBR_LANDSCAPE_PGP_ONLY);
    // The recipe and missing-entry lines follow on the same hover.
    h.get_by_label_contains("rmaos: run");
    h.get_by_label_contains("no file matches landscape/notes.txt");
}

/// K12: an `add` entry that matches no file is a red "?" chip with the reason on its hover.
#[test]
fn a_missing_entry_is_a_question_mark_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    std::fs::write(m.root.clone().unwrap().join("landscape/notes.txt"), "x").unwrap();
    m.edit_docs("t", None, true, |_, s| {
        s.add_member(ROCK, "landscape/notes.txt")
    });
    idle(&mut m);
    let mut h = harness(m);
    h.run();
    // Note DU1: the chip is a button named by its entry.
    let q = on_card(&h, "Missing: landscape/notes.txt");
    assert_eq!(q.len(), 1, "one chip per missing entry");
    q[0].hover();
    h.run();
    h.run();
    h.get_by_label("no file matches landscape/notes.txt");
}

/// Press on `at` and move past the drag threshold: whether a texture is now in flight. The payload is cleared before
/// the release, so nothing is dropped.
fn drags_a_texture(h: &mut Harness<'static, AppModel>, at: egui::Pos2) -> bool {
    use sdc_gui::ui::sets_column::DragItem;
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.hover_at(at);
    h.run();
    h.event(press(true));
    h.run();
    for step in 1..=4 {
        h.event(egui::Event::PointerMoved(
            at + egui::vec2(6.0 * step as f32, 0.0),
        ));
        h.run_steps(2); // the payload pass, then the painted one
    }
    let dragging = egui::DragAndDrop::payload::<DragItem>(&h.ctx).is_some();
    egui::DragAndDrop::clear_payload(&h.ctx);
    h.event(press(false));
    h.run();
    dragging
}

fn leaf_borrowing(dir: &std::path::Path) -> (AppModel, String, String) {
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    let mut m = pbr_ready(dir);
    let key =
        |m: &AppModel, name: &str| m.items.iter().find(|i| i.name == name).unwrap().key.clone();
    let leaf = key(&m, "leaf.png");
    m.set_field(&leaf, "subtype", serde_json::json!("pbr_albedo"));
    idle(&mut m);
    let id = m.drop_new_set(&leaf).unwrap();
    idle(&mut m);
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    let normal = key(&m, "rock01_normal.png");
    assert!(matches!(
        m.drop_choice(&normal, &id, DropChoice::Share),
        DropOutcome::Joined
    ));
    idle(&mut m);
    let v = set_views(&m).into_iter().find(|v| v.id == id).unwrap();
    assert!(
        matches!(
            v.slots.get(&SlotKind::Normal),
            Some(SlotFill::Borrowed { .. })
        ),
        "{:?}",
        v.slots
    );
    (m, id, normal)
}

/// Press on `at` and move past the drag threshold: the borrow now in flight, as (item, from). The payload is cleared
/// before the release, so nothing is dropped.
fn drags_a_borrow(h: &mut Harness<'static, AppModel>, at: egui::Pos2) -> Option<(String, String)> {
    use sdc_gui::ui::sets_column::DragBorrow;
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.hover_at(at);
    h.run();
    h.event(press(true));
    h.run();
    for step in 1..=4 {
        h.event(egui::Event::PointerMoved(
            at + egui::vec2(6.0 * step as f32, 0.0),
        ));
        h.run_steps(2); // the payload pass, then the painted one
    }
    let got =
        egui::DragAndDrop::payload::<DragBorrow>(&h.ctx).map(|p| (p.item.clone(), p.from.clone()));
    egui::DragAndDrop::clear_payload(&h.ctx);
    h.event(press(false));
    h.run();
    got
}

/// Note JW4: a dragged shared chip has the same ghost as a dragged texture.
#[test]
fn a_dragged_shared_chip_shows_the_ghost() {
    let d = tempfile::tempdir().unwrap();
    let (m, id, _normal) = leaf_borrowing(d.path());
    let mut h = harness(m);
    h.run();
    let at = on_card_of(&h, &id, "N")[0].rect().center();
    let p = start_drag(&mut h, at);
    assert!(egui::DragAndDrop::has_payload_of_type::<
        sdc_gui::ui::sets_column::DragBorrow,
    >(&h.ctx));
    let want = p + egui::vec2(12.0, 12.0);
    let bounds = |s: &egui::Shape| match s {
        egui::Shape::Rect(r) => Some(r.rect),
        egui::Shape::Mesh(m) => Some(m.calc_bounds()),
        _ => None,
    };
    assert!(shapes(&h)
        .iter()
        .filter_map(bounds)
        .any(|r| (r.min - want).length() < 1.0 && (r.width() - 40.0).abs() < 0.5));
    egui::DragAndDrop::clear_payload(&h.ctx);
}

/// Note JW4: a borrowed chip keeps its neutral outline and offers its source for sharing.
#[test]
fn a_borrowed_chip_is_outlined_neutral_and_drags_as_a_share() {
    let d = tempfile::tempdir().unwrap();
    let (m, id, normal) = leaf_borrowing(d.path());
    let mut h = harness(m);
    h.run();
    let chip = on_card_of(&h, &id, "N");
    assert_eq!(chip.len(), 1, "the borrower's N chip");
    let at = chip[0].rect();
    let strong = h.ctx.global_style().visuals.strong_text_color();
    let stroked = |h: &Harness<'static, AppModel>, r: egui::Rect, c: egui::Color32| {
        shapes(h).iter().any(|s| {
            matches!(s, egui::Shape::Rect(x) if x.rect == r && x.stroke.width == 2.0 && x.stroke.color == c)
        })
    };
    let backing = |chip: egui::Rect| {
        egui::Rect::from_min_size(chip.min + egui::vec2(11.0, 2.0), egui::Vec2::splat(10.0))
    };
    let rings = |chip| {
        shapes(&h)
            .iter()
            .filter(|s| {
                matches!(s, egui::Shape::Circle(c)
        if c.stroke.width == 1.5 && c.stroke.color == strong && backing(chip).contains(c.center))
            })
            .count()
    };
    assert!(
        stroked(&h, at, strong),
        "the borrowed chip has the neutral 2 px outline"
    );
    assert!(!stroked(&h, at, sdc_gui::ui::status::AMBER), "and no amber");
    assert!(shapes(&h).iter().any(|s| matches!(s, egui::Shape::Rect(r)
        if r.rect == backing(at) && r.fill == h.ctx.global_style().visuals.panel_fill)));
    assert_eq!(rings(at), 2);
    let own = on_card(&h, "N")[0].rect();
    assert!(
        !stroked(&h, own, strong) && rings(own) == 0,
        "rock01's own N chip has neither"
    );
    assert!(!stroked(&h, own, sdc_gui::ui::status::AMBER));
    h.hover_at(at.center());
    h.run();
    h.run();
    assert_eq!(
        h.query_all_by_label_contains("borrowed from landscape/rock01")
            .count(),
        1,
        "one hover"
    );
    // The member's name and the borrowed line are the same hover.
    h.get_by_label_contains("rock01_normal.png\nborrowed from landscape/rock01");
    assert!(
        !drags_a_texture(&mut h, at.center()),
        "a borrowed chip carries no texture payload"
    );
    assert_eq!(
        drags_a_borrow(&mut h, at.center()),
        Some((normal.clone(), "landscape/rock01".to_string())),
        "it drags as a share of rock01's normal"
    );
    assert!(
        drags_a_texture(&mut h, own.center()),
        "an own chip still drags"
    );
    on_card_of(&h, &id, "N")[0].click_secondary();
    h.run();
    assert!(
        h.query_by_label("Remove from set").is_none(),
        "a borrowed chip cannot be removed from here"
    );
}

/// Final review I1: a texture dragged over a card outlines that card, on top of the card's own frame (the drop
/// zone's own hover stroke sits under the card frame).
#[test]
fn a_texture_dragged_over_a_card_outlines_it() {
    use sdc_gui::ui::sets_column::DragItem;
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let item = m.items[0].key.clone();
    let mut h = harness(m);
    h.run();
    let card = card_rect(&h);
    let active = h.ctx.global_style().visuals.widgets.active.bg_stroke;
    let near = |a: egui::Rect, b: egui::Rect| {
        (a.min - b.min).length() < 1.0 && (a.max - b.max).length() < 1.0
    };
    // The last rect shape on the card, and whether it is the drop outline.
    let top_is_outline = |h: &Harness<'static, AppModel>| {
        shapes(h)
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Rect(r) if near(r.rect, card) && r.stroke.width > 0.0 => {
                    Some(r.stroke)
                }
                _ => None,
            })
            .next_back()
            .is_some_and(|s| s == active)
    };
    assert!(!top_is_outline(&h), "no outline without a drag");
    let at = card.center();
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.hover_at(at);
    h.run();
    h.event(press(true));
    for step in 1..=3 {
        egui::DragAndDrop::set_payload(&h.ctx, DragItem(item.clone()));
        h.event(egui::Event::PointerMoved(
            at + egui::vec2(2.0 * step as f32, 0.0),
        ));
        h.run_steps(2); // the payload pass, then the painted one
    }
    assert!(
        egui::DragAndDrop::has_payload_of_type::<DragItem>(&h.ctx),
        "a texture is in flight"
    );
    assert!(
        top_is_outline(&h),
        "the card under a dragged texture is outlined on top of its frame"
    );
    egui::DragAndDrop::clear_payload(&h.ctx);
    h.event(press(false));
    h.run();
    assert!(!top_is_outline(&h), "the outline goes with the drag");
}

/// Note JW4: the accepting card draws its outline above the card frame for a share too.
#[test]
fn a_borrow_dragged_over_a_card_outlines_it() {
    let d = tempfile::tempdir().unwrap();
    let (m, leaf, _) = leaf_borrowing(d.path());
    let mut h = harness(m);
    h.run();
    let at = on_card_of(&h, &leaf, "N")[0].rect().center();
    start_drag(&mut h, at);
    let card = card_rect(&h);
    h.event(egui::Event::PointerMoved(card.center()));
    h.run_steps(3);
    let stroke = shapes(&h)
        .into_iter()
        .filter_map(|s| match s {
            egui::Shape::Rect(r)
                if (r.rect.min - card.min).length() < 1.0
                    && (r.rect.max - card.max).length() < 1.0
                    && r.stroke.width > 0.0 =>
            {
                Some(r.stroke)
            }
            _ => None,
        })
        .next_back();
    assert_eq!(
        stroke,
        Some(h.ctx.global_style().visuals.widgets.active.bg_stroke)
    );
}

/// Press `at` and move it 30 pt in 3 steps (the button stays down); returns where the pointer ends.
fn start_drag(h: &mut Harness<'static, AppModel>, at: egui::Pos2) -> egui::Pos2 {
    h.hover_at(at);
    h.run();
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
    h.run();
    for step in 1..=3 {
        h.event(egui::Event::PointerMoved(
            at + egui::vec2(10.0 * step as f32, 0.0),
        ));
        h.run_steps(2); // the payload pass, then the painted one
    }
    at + egui::vec2(30.0, 0.0)
}

/// T7 K10: a 40 x 40 ghost 12 pt below and right of the pointer.
fn assert_ghost_rect(h: &Harness<'static, AppModel>, pointer: egui::Pos2) {
    assert!(egui::DragAndDrop::has_payload_of_type::<
        sdc_gui::ui::sets_column::DragItem,
    >(&h.ctx));
    let want = pointer + egui::vec2(12.0, 12.0);
    let bounds = |s: &egui::Shape| match s {
        egui::Shape::Rect(r) => Some(r.rect),
        egui::Shape::Mesh(m) => Some(m.calc_bounds()),
        _ => None,
    };
    assert!(
        shapes(h)
            .iter()
            .filter_map(bounds)
            .any(|r| (r.min - want).length() < 1.0 && (r.width() - 40.0).abs() < 0.5),
        "a 40 wide ghost at {want:?}"
    );
}

/// The ghost plus the Grabbing cursor (R5, green from the start; the drag plugin sets it for a dragged widget).
fn assert_ghost(h: &Harness<'static, AppModel>, pointer: egui::Pos2) {
    assert_ghost_rect(h, pointer);
    assert_eq!(
        h.output().platform_output.cursor_icon,
        egui::CursorIcon::Grabbing
    );
}

#[test]
fn a_dragged_tile_shows_a_ghost_under_the_pointer_and_dims_its_source() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    h.run();
    let label = outside_sets(&h, "rock01_normal.png").rect();
    let at = label.center();
    let p = start_drag(&mut h, at);
    assert_ghost(&h, p);
    // The source tile is dimmed: its 2 pt frame stroke is half transparent.
    let faded = shapes(&h).iter().any(|s| matches!(s,
        egui::Shape::Rect(r) if r.rect.contains_rect(label) && r.stroke.width == 2.0 && r.stroke.color.a() < 200));
    assert!(faded, "the dragged source is dimmed");
    // Review I1: no full-opacity hover or press stroke over the dimmed source.
    let lit = shapes(&h).iter().any(|s| matches!(s,
        egui::Shape::Rect(r) if r.rect.contains_rect(label) && r.stroke.width == 2.0 && r.stroke.color.a() == 255));
    assert!(!lit, "the dimmed source shows no full-opacity stroke");
    egui::DragAndDrop::clear_payload(&h.ctx);
}

/// Review M3: the ghost needs no source widget, and goes with the payload.
#[test]
fn the_ghost_is_painted_from_the_payload_alone_and_goes_with_it() {
    use sdc_gui::ui::sets_column::DragItem;
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let key = m.items[0].key.clone();
    let mut h = harness(m);
    h.run();
    let at = egui::pos2(900.0, 500.0);
    egui::DragAndDrop::set_payload(&h.ctx, DragItem(key));
    h.hover_at(at);
    h.run_steps(2); // the payload pass, then the painted one
    assert_ghost_rect(&h, at);
    egui::DragAndDrop::clear_payload(&h.ctx);
    h.run();
    let want = at + egui::vec2(12.0, 12.0);
    assert!(
        !shapes(&h).iter().any(|s| match s {
            egui::Shape::Rect(r) => (r.rect.min - want).length() < 1.0,
            egui::Shape::Mesh(m) => (m.calc_bounds().min - want).length() < 1.0,
            _ => false,
        }),
        "the ghost is gone with the payload"
    );
    assert!(
        !shapes(&h).iter().any(|s| matches!(s,
            egui::Shape::Rect(r) if r.stroke.width == 2.0 && r.stroke.color.a() < 200)),
        "no tile is dimmed"
    );
}

#[test]
fn a_dragged_member_chip_shows_the_same_ghost() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    h.run();
    let at = on_card(&h, "N")[0].rect().center();
    let p = start_drag(&mut h, at);
    assert_ghost(&h, p);
    egui::DragAndDrop::clear_payload(&h.ctx);
}

#[test]
fn a_hovered_tile_takes_the_hover_stroke_and_an_amber_tile_keeps_amber() {
    use sdc_gui::model::items::ItemStatus;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let amber_name = m.items[0].name.clone();
    m.items[0].status = ItemStatus::NeedsDecision;
    let ready_name = m.items[1].name.clone();
    m.items[1].status = ItemStatus::Ready;
    let mut h = harness(m);
    h.run();
    let hov = h.ctx.global_style().visuals.widgets.hovered.bg_stroke.color;
    // The last 2 pt stroke around the tile that holds `name`.
    let top = |h: &Harness<'static, AppModel>, name: &str| {
        let label = outside_sets(h, name).rect();
        shapes(h)
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Rect(r) if r.rect.contains_rect(label) && r.stroke.width == 2.0 => {
                    Some(r.stroke.color)
                }
                _ => None,
            })
            .next_back()
    };
    let at = outside_sets(&h, &ready_name).rect().center();
    h.hover_at(at);
    h.run();
    assert_eq!(
        top(&h, &ready_name),
        Some(hov),
        "hover stroke on a Ready tile"
    );
    let at = outside_sets(&h, &amber_name).rect().center();
    h.hover_at(at);
    h.run();
    assert_eq!(
        top(&h, &amber_name),
        Some(sdc_gui::ui::status::AMBER),
        "status wins over hover"
    );
}

#[test]
fn a_pressed_strip_fills() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.settings.textures_collapsed = true;
    let mut h = harness(m);
    h.run();
    let at = h.get_by_label("Textures").rect().center();
    let fill = h.ctx.global_style().visuals.widgets.active.weak_bg_fill;
    let covers = |h: &Harness<'static, AppModel>| {
        shapes(h).iter().any(|s| {
            matches!(s,
            egui::Shape::Rect(r) if r.fill == fill && r.rect.contains(at) && r.rect.width() < 60.0)
        })
    };
    h.hover_at(at);
    h.run();
    assert!(!covers(&h), "no fill before the press");
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    });
    h.run();
    assert!(covers(&h), "the pressed strip is filled");
}

/// Final review D1: a drag held still asks for no frames (the ghost moves only with pointer events), and egui's own
/// repaint after the release frame takes the ghost and the dim away one frame after release.
#[test]
fn a_still_drag_asks_for_no_frames_and_the_ghost_goes_one_frame_after_release() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()));
    h.run();
    let label = outside_sets(&h, "rock01_normal.png").rect();
    let p = start_drag(&mut h, label.center());
    let delay = |h: &Harness<'static, AppModel>| {
        h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay
    };
    h.run();
    assert_ghost_rect(&h, p);
    assert!(delay(&h) > std::time::Duration::ZERO, "a still drag idles");
    h.event(egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });
    h.step();
    assert_eq!(
        delay(&h),
        std::time::Duration::ZERO,
        "the release frame asks for one more"
    );
    h.step();
    let want = p + egui::vec2(12.0, 12.0);
    assert!(
        !shapes(&h).iter().any(|s| match s {
            egui::Shape::Rect(r) => (r.rect.min - want).length() < 1.0,
            egui::Shape::Mesh(m) => (m.calc_bounds().min - want).length() < 1.0,
            _ => false,
        }),
        "the ghost is gone"
    );
    assert!(
        !shapes(&h).iter().any(|s| matches!(s,
            egui::Shape::Rect(r) if r.stroke.width == 2.0 && r.stroke.color.a() < 200)),
        "no tile is dimmed"
    );
}

// Task 21 (Note FM32): the virtualised Sets list keeps keyboard reach, card heights and the scroll anchor.

type SetsHarness = Harness<'static, (AppModel, sdc_gui::ui::State)>;

fn many(m: AppModel) -> SetsHarness {
    many_at(m, 1500.0)
}

fn many_at(m: AppModel, width: f32) -> SetsHarness {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(width, 900.0))
        .with_max_steps(16)
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, sdc_gui::ui::State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, sdc_gui::ui::State::default()),
        );
    h
}

fn pos_rect(h: &SetsHarness, n: usize, total: usize) -> Option<egui::Rect> {
    h.query_all_by_label(&format!("Position {n} of {total}"))
        .map(|n| n.rect())
        .find(|r| in_sets_column(h, *r))
}

fn set_ids(m: &AppModel) -> Vec<String> {
    sdc_gui::model::sets::views(m)
        .iter()
        .map(|v| v.id.clone())
        .collect()
}

#[test]
fn off_screen_set_cards_have_accesskit_nodes_and_tab_stops() {
    use egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let m = many_sets(d.path(), 60);
    let ids = set_ids(&m);
    let mut h = many(m);
    h.run();
    let off: Vec<usize> = (1..=60)
        .filter(|&n| pos_rect(&h, n, 60).is_none())
        .collect();
    assert!(off.len() > 30, "most cards are off screen: {}", off.len());
    let node_name = |n: usize| format!("Set {}, position {n} of 60", ids[n - 1]);
    for &n in &off {
        let name = node_name(n);
        assert_eq!(h.query_all_by_label(&name).count(), 1, "{name}'s node");
    }
    // Tab from the Sets filter walks the cards: it reaches an off-screen card's stand-in, which scrolls its card in.
    h.query_all_by_role_and_label(Role::Button, "Filter, Sets shader filter")
        .find(|n| in_sets_column(&h, n.rect()))
        .expect("the Sets filter button")
        .focus();
    h.step();
    // The first card wholly past the panel is the next stop that is only a stand-in. (A card whose position label
    // is cut off by the bottom edge is drawn, so its Tab stops are its own parts, not a stand-in.)
    let panel = support::panel(&h.ctx, "sets");
    let next = *off
        .iter()
        .find(|&&n| {
            h.query_all_by_label(&node_name(n))
                .all(|c| !c.rect().intersects(panel))
        })
        .expect("a card wholly past the panel");
    let mut presses = 0;
    while pos_rect(&h, next, 60).is_none() && presses < 600 {
        h.key_press(egui::Key::Tab);
        h.step();
        presses += 1;
    }
    assert!(
        pos_rect(&h, next, 60).is_some(),
        "Tab never scrolled card {next} in"
    );
    // The focus survived the swap from the stand-in to the drawn card: the focused node is that card, by its name.
    let focused: Vec<String> = h
        .query_all_by_role(Role::Button)
        .filter(|n| n.is_focused())
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert_eq!(focused, [node_name(next)], "the focus follows the card in");
}

#[test]
fn a_go_to_set_in_the_same_frame_wins_over_a_tab_reveal() {
    use egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let m = many_sets(d.path(), 60);
    let (ids, views) = (set_ids(&m), sdc_gui::model::sets::views(&m));
    let (far, near) = (views[59].key.clone(), 45);
    let mut h = many(m);
    h.run();
    // The focus lands on a far card's stand-in in the very frame a Go to set to the last card is pending.
    h.query_all_by_role_and_label(
        Role::Button,
        &format!("Set {}, position {near} of 60", ids[near - 1]),
    )
    .next()
    .expect("the stand-in")
    .focus();
    h.state_mut().0.nav.reveal_set = Some(sdc_gui::model::SetTarget::Key(far));
    h.step();
    h.run();
    assert!(
        h.state().0.nav.reveal_set.is_none(),
        "the Tab asks for no scroll of its own"
    );
    assert!(
        pos_rect(&h, 60, 60).is_some(),
        "the Go to set stays where it landed"
    );
}

/// Card `n` (1-based) of `total`, measured by scrolling to it, then scrolled away from again.
fn measure_far_card(h: &mut SetsHarness, n: usize) -> String {
    let views = sdc_gui::model::sets::views(&h.state().0);
    let (far, first) = (views[n - 1].clone(), views[0].key.clone());
    h.state_mut()
        .0
        .palette_pick(&sdc_gui::model::nav::Pick::Set(far.key.clone()));
    h.run();
    assert!(pos_rect(h, n, views.len()).is_some(), "card {n} was drawn");
    h.state_mut()
        .0
        .palette_pick(&sdc_gui::model::nav::Pick::Set(first));
    h.run();
    assert!(
        pos_rect(h, n, views.len()).is_none(),
        "card {n} is far again"
    );
    far.id
}

#[test]
fn card_heights_refresh_after_a_resize() {
    let d = tempfile::tempdir().unwrap();
    let mut m = many_sets(d.path(), 60);
    let all: Vec<String> = m.items.iter().map(|i| i.key.clone()).collect();
    // Repeats stand in for more files: enough chips to wrap in a narrow column.
    m.sets
        .iter_mut()
        .for_each(|s| s.members = all.iter().cycle().take(all.len() * 4).cloned().collect());
    m.invalidate_views();
    let mut h = many_at(m, 2600.0);
    let chosen = sdc_gui::ui::panel_ids::chosen_width_id("sets");
    h.ctx.data_mut(|d| d.insert_persisted(chosen, 520.0_f32));
    h.run();
    let far = measure_far_card(&mut h, 50);
    let wide = h.state().1.drops.card_h[&far];
    // Narrow the column: the far card is not drawn, yet its remembered height follows the width.
    h.ctx
        .data_mut(|d| d.insert_persisted(chosen, sdc_gui::ui::columns::SETS_MIN));
    h.run();
    assert!(pos_rect(&h, 50, 60).is_none(), "the far card stays undrawn");
    let narrow = h.state().1.drops.card_h[&far];
    assert!(
        narrow > wide + 1.0,
        "{wide} at 520 wide, {narrow} at the minimum"
    );
}

#[test]
fn a_far_jump_does_not_shift_the_scroll() {
    // Short cards first, tall ones after: the mean of the measured heights climbs as the jump lands.
    let d = tempfile::tempdir().unwrap();
    let mut m = many_sets(d.path(), 500);
    let all: Vec<String> = m.items.iter().map(|i| i.key.clone()).collect();
    for (i, s) in m.sets.iter_mut().enumerate() {
        s.members = if i < 20 { vec![] } else { all.clone() };
    }
    m.invalidate_views();
    let key = sdc_gui::model::sets::views(&m)[199].key.clone();
    let mut h = many(m);
    h.run();
    h.state_mut()
        .0
        .palette_pick(&sdc_gui::model::nav::Pick::Set(key));
    // Once card 200 is laid out, its place on screen does not move over the next frames.
    let mut ys = vec![];
    for _ in 0..12 {
        h.step();
        ys.extend(pos_rect(&h, 200, 500).map(|r| r.top()));
    }
    assert!(ys.len() >= 6, "card 200 is on screen: {ys:?}");
    // egui snaps each allocation, so the sum of 200 of them is a pixel off the sum of the heights: the first frames
    // may differ by that, never by a card's worth.
    assert!(
        ys.windows(2).all(|w| (w[0] - w[1]).abs() < 1.5),
        "card 200 moved between frames: {ys:?}"
    );
}

/// The AccessKit name `member_chip` gives the own `leaf_f.png` chip on base (read off the premise run).
const FUZZ_CHIP: &str = "F";

#[test]
fn a_linked_chip_is_filled_and_named_for_its_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.run_menu(MenuAction::SlotLink {
        set: "arch/leaf".into(),
        n: 1,
        item: "arch/wall.png".into(),
    });
    let mut h = harness(m);
    h.run();
    assert_eq!(
        on_card_of(&h, "arch/leaf", "D, from set arch/wall").len(),
        1
    );
    assert!(on_card_of(&h, "arch/leaf", "D, empty").is_empty());
    h.state_mut().undo();
    h.run();
    assert!(empty_letters(&h, "arch/leaf").contains(&"D".to_string()));
    assert!(on_card_of(&h, "arch/leaf", "D, empty").is_empty());
}

#[test]
fn a_coat_link_fills_one_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal)],
    );
    m.run_menu(MenuAction::SlotLink {
        set: "arch/leaf".into(),
        n: 7,
        item: "arch/lacq_cnr.png".into(),
    });
    let mut h = harness(m);
    h.run();
    let linked = h
        .query_all_by_label_contains(", from set arch/lacq")
        .count();
    assert_eq!(
        linked, 1,
        "one slot-7 link is one chip, not a Coat and a Fuzz chip"
    );
}

#[test]
fn a_coat_link_over_an_own_fuzz_shows_only_the_link() {
    let d = tempfile::tempdir().unwrap();
    let m = no_albedo_pair_with(
        d.path(),
        &[
            ("leaf_f.png", [200, 200, 200], Subtype::PbrFuzz),
            ("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal),
        ],
    );
    let mut h = harness(m);
    h.run();
    assert_eq!(
        on_card_of(&h, "arch/leaf", FUZZ_CHIP).len(),
        1,
        "premise: the own _f's chip is on the card"
    );
    h.state_mut().run_menu(MenuAction::SlotLink {
        set: "arch/leaf".into(),
        n: 7,
        item: "arch/lacq_cnr.png".into(),
    });
    h.run();
    let drawn = h
        .query_all_by_label_contains(", from set arch/lacq")
        .count()
        + on_card_of(&h, "arch/leaf", FUZZ_CHIP).len();
    let v = views(h.state())
        .iter()
        .find(|v| v.id == "arch/leaf")
        .unwrap()
        .clone();
    let counted = view_row(&v)
        .iter()
        .filter(|c| c.kind.game_slot() == Some(7))
        .count();
    assert_eq!(
        (drawn, counted),
        (1, 1),
        "slot 7: one chip drawn (the link's), one counted"
    );
    assert!(
        on_card_of(&h, "arch/leaf", FUZZ_CHIP).is_empty(),
        "the own _f is not written, so its chip goes"
    );
    // chip_row's trailing loop names an un-chipped member "{file}, {slot text} (its slot is taken)": not for a
    // Replaced file
    assert!(
        h.query_all_by_label_contains("leaf_f.png, ").all(|n| !n
            .accesskit_node()
            .label()
            .is_some_and(|l| l.contains("(its slot is taken)"))),
        "no thumbnail for the own _f"
    );
}

#[test]
fn a_cleared_slot_has_a_cleared_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command("arch/wall", 2, SlotCommand::Clear)
        .unwrap();
    let mut h = harness(m);
    h.run();
    assert_eq!(on_card_of(&h, "arch/wall", "N, cleared").len(), 1);
}

/// Final wave Group 5 (Important 6, ledger 88, 89): a chip's AccessKit node says what its hover says, while the hover
/// stays lazy: an own chip its file and lock, a cleared chip its file (and no "empty" value left behind by the box's
/// Label), a typed-path box its path; the typed-path box paints its slot's lock as a thumbnail does.
#[test]
fn chips_describe_what_their_hover_says_and_a_typed_path_box_paints_its_lock() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command("arch/wall", 2, SlotCommand::Clear)
        .unwrap();
    m.set_slot_command(
        "arch/leaf",
        1,
        SlotCommand::Path(r"textures\other\x_d.dds".into()),
    )
    .unwrap();
    for set in ["arch/wall", "arch/leaf"] {
        assert_eq!(
            m.run_menu(MenuAction::SlotLock {
                set: set.into(),
                n: 1,
                on: true
            }),
            None
        );
    }
    let mut h = harness(m);
    h.run();
    let desc = |h: &Harness<'static, AppModel>, set: &str, label: &str| {
        let n = on_card_of(h, set, label);
        assert_eq!(n.len(), 1, "{set}: one {label:?} chip");
        let a = n[0].accesskit_node();
        (a.description(), a.value(), n[0].rect())
    };
    let (own, _, _) = desc(&h, "arch/wall", "D");
    let own = own.unwrap_or_default();
    assert!(
        own.contains("wall.png") && own.contains("(slot pinned)"),
        "own chip: {own:?}"
    );
    let (cleared, value, _) = desc(&h, "arch/wall", "N, cleared");
    assert!(
        cleared
            .as_deref()
            .is_some_and(|c| c.contains("wall_n.png") && c.contains("(cleared)")),
        "cleared chip: {cleared:?}"
    );
    assert!(
        value.as_deref().is_none_or(|v| !v.contains("empty")),
        "no empty value: {value:?}"
    );
    let (typed, _, rect) = desc(&h, "arch/leaf", "D, path x_d.dds");
    assert_eq!(
        typed.as_deref(),
        Some(r"Diffuse linked to textures\other\x_d.dds")
    );
    assert!(
        texts_in(&h, rect).iter().any(|(t, _)| t == "🔒"),
        "the typed-path box paints its slot's lock: {:?}",
        texts_in(&h, rect)
    );
}

/// The painted text shapes of the frame inside `chip`: (text, galley rect in screen space).
fn texts_in(h: &Harness<'static, AppModel>, chip: egui::Rect) -> Vec<(String, egui::Rect)> {
    shapes(h)
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Text(t) => Some((
                t.galley.text().to_string(),
                t.galley.rect.translate(t.pos.to_vec2()),
            )),
            _ => None,
        })
        .filter(|(_, r)| chip.contains_rect(*r))
        .collect()
}

/// Whether two rects share area (rects that only touch along an edge do not overlap).
fn overlaps(a: egui::Rect, b: egui::Rect) -> bool {
    let i = a.intersect(b);
    i.width() > 0.0 && i.height() > 0.0
}

/// Linked Coat (slot 7) and RMAOS (slot 6) chips on `leaf`, each with both locks when `locks`: the chip named
/// `label` has its link mark (backing square and two rings) at its top centre, inside the chip and touching neither
/// the slot letter nor a lock.
fn assert_mark_at_top_centre(label: &str, letter: &str, locks: bool, theme: egui::Theme) {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[("lacq_cnr.png", [128, 128, 255], Subtype::CoatNormal)],
    );
    for (n, item) in [(6, "arch/wall_rmaos.png"), (7, "arch/lacq_cnr.png")] {
        // Ruling 3b-F21: a pick waits for the plan the lock before it started.
        idle(&mut m);
        assert_eq!(
            m.run_menu(MenuAction::SlotLink {
                set: "arch/leaf".into(),
                n,
                item: item.into(),
            }),
            None,
            "{item}"
        );
        if locks {
            m.set_slot_lock("arch/leaf", n, true).unwrap();
            m.run_menu(MenuAction::PgpLock {
                set: "arch/leaf".into(),
                n,
                on: true,
            });
        }
    }
    idle(&mut m);
    let mut h = harness(m);
    h.ctx.set_theme(theme);
    h.run();
    let visuals = &h.ctx.global_style().visuals;
    assert_eq!(visuals.dark_mode, theme == egui::Theme::Dark);
    assert_ne!(visuals.strong_text_color(), visuals.panel_fill);
    let chip = {
        let card = card_of(&h, "arch/leaf");
        let found: Vec<_> = h
            .query_all_by_label_contains(label)
            .filter(|n| card.contains_rect(n.rect()))
            .collect();
        assert_eq!(found.len(), 1, "the {label} chip");
        found[0].rect()
    };
    let texts = texts_in(&h, chip);
    let lock_rects: Vec<egui::Rect> = texts
        .iter()
        .filter(|(t, _)| t == "🔒")
        .map(|(_, r)| *r)
        .collect();
    assert_eq!(lock_rects.len(), if locks { 2 } else { 0 }, "{texts:?}");
    let letter_rect = texts
        .iter()
        .find(|(t, _)| t == letter)
        .unwrap_or_else(|| panic!("the letter {letter}: {texts:?}"))
        .1;
    let circles: Vec<egui::Pos2> = shapes(&h)
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Circle(c) if c.stroke.width == 1.5 && chip.contains(c.center) => {
                assert_eq!(
                    c.stroke.color,
                    visuals.strong_text_color(),
                    "link rings in {theme:?}"
                );
                Some(c.center)
            }
            _ => None,
        })
        .collect();
    assert_eq!(circles.len(), 2, "the mark's two rings are inside the chip");
    let mark = shapes(&h)
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Rect(r)
                if r.fill == visuals.panel_fill
                    && chip.contains_rect(r.rect)
                    && r.rect.width() < chip.width()
                    && circles.iter().all(|c| r.rect.contains(*c)) =>
            {
                Some(r.rect)
            }
            _ => None,
        })
        .min_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("the mark's backing square");
    assert_eq!(
        mark,
        egui::Rect::from_min_size(chip.min + egui::vec2(11.0, 2.0), egui::Vec2::splat(10.0)),
        "the mark is at the chip's top centre in every lock state"
    );
    assert!(
        !overlaps(mark, letter_rect),
        "the mark {mark:?} covers the letter {letter_rect:?}"
    );
    for l in &lock_rects {
        assert!(!overlaps(mark, *l), "the mark {mark:?} covers a lock {l:?}");
    }
}

#[test]
fn the_link_mark_clears_the_letter_and_both_locks_on_a_coat_chip() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        assert_mark_at_top_centre(", from set arch/lacq", "CNR", true, theme);
    }
}

#[test]
fn the_link_mark_clears_the_letter_and_both_locks_on_an_rmaos_chip() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        assert_mark_at_top_centre(", from set arch/wall", "RMAOS", true, theme);
    }
}

#[test]
fn the_link_mark_is_in_the_same_place_without_locks() {
    assert_mark_at_top_centre(", from set arch/lacq", "CNR", false, egui::Theme::Dark);
    assert_mark_at_top_centre(", from set arch/wall", "RMAOS", false, egui::Theme::Dark);
}

#[test]
fn a_typed_path_link_is_a_filled_box_named_for_its_file_with_the_slot_menu() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command(
        "arch/leaf",
        1,
        SlotCommand::Path("textures\\other\\thing_d.dds".into()),
    )
    .unwrap();
    let mut h = harness(m);
    h.run();
    let chip = on_card_of(&h, "arch/leaf", "D, path thing_d.dds");
    assert_eq!(chip.len(), 1, "the typed path's box");
    chip[0].hover();
    h.run();
    h.run();
    h.get_by_label_contains("Diffuse linked to textures\\other\\thing_d.dds");
    on_card_of(&h, "arch/leaf", "D, path thing_d.dds")[0].click_secondary();
    h.run();
    h.get_by_label("Slot 1: Diffuse");
}

#[test]
fn a_cleared_chip_offers_the_slot_menu_that_restores_the_slot() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command("arch/wall", 2, SlotCommand::Clear)
        .unwrap();
    let mut h = harness(m);
    h.run();
    on_card_of(&h, "arch/wall", "N, cleared")[0].click_secondary();
    h.run();
    h.get_by_label_contains("Slot 2:");
    let from_set = h.get_by_label_contains("From this set");
    assert!(!from_set.accesskit_node().is_disabled());
    from_set.click();
    h.run();
    idle(h.state_mut());
    h.run();
    assert!(on_card_of(&h, "arch/wall", "N, cleared").is_empty());
}

/// Nodes named "<file>, ... (its slot is taken)": the trailing loop's thumbnails for a file with no slot of its own.
fn taken_thumbs(h: &Harness<'static, AppModel>, file: &str) -> usize {
    h.query_all_by_label_contains(&format!("{file}, "))
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.contains("(its slot is taken)"))
        })
        .count()
}

/// The cells of `id`'s slot row on game slot `n`: what `chip_rows` counts for it.
fn counted_on_slot(m: &AppModel, id: &str, n: u8) -> usize {
    let v = views(m).iter().find(|v| v.id == id).unwrap().clone();
    view_row(&v)
        .iter()
        .filter(|c| c.kind.game_slot() == Some(n))
        .count()
}

#[test]
fn a_cleared_slot_over_an_own_file_draws_one_chip_that_names_the_file() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.set_slot_command("arch/wall", 2, SlotCommand::Clear)
        .unwrap();
    let mut h = harness(m);
    h.run();
    let cleared = on_card_of(&h, "arch/wall", "N, cleared").len();
    let drawn = cleared + taken_thumbs(&h, "wall_n.png");
    assert_eq!(cleared, 1);
    assert_eq!(
        (drawn, counted_on_slot(h.state(), "arch/wall", 2)),
        (1, 1),
        "slot 2: one chip drawn, one counted"
    );
    assert_eq!(
        taken_thumbs(&h, "wall_n.png"),
        0,
        "no thumbnail for the file"
    );
    on_card_of(&h, "arch/wall", "N, cleared")[0].hover();
    h.run();
    h.run();
    h.get_by_label_contains("(cleared): wall_n.png");
}

#[test]
fn a_link_over_an_own_file_of_its_kind_draws_one_chip() {
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair_with(
        d.path(),
        &[("other.png", [10, 200, 10], Subtype::PbrAlbedo)],
    );
    m.run_menu(MenuAction::SlotLink {
        set: "arch/wall".into(),
        n: 1,
        item: "arch/other.png".into(),
    });
    let mut h = harness(m);
    h.run();
    let linked = h
        .query_all_by_label_contains(", from set arch/other")
        .count();
    assert_eq!(linked, 1, "the linked chip");
    let drawn = linked + taken_thumbs(&h, "wall.png");
    assert_eq!(
        (drawn, counted_on_slot(h.state(), "arch/wall", 1)),
        (1, 1),
        "slot 1: one chip drawn (the link's), one counted"
    );
    assert_eq!(
        taken_thumbs(&h, "wall.png"),
        0,
        "no thumbnail for the own file"
    );
}

/// What a card's title painted: its text, how many lines the galley has, and the title node's rect.
fn title_probe(h: &Harness<'static, AppModel>, id: &str) -> (String, usize, egui::Rect) {
    let r = h
        .query_all_by_label(id)
        .next()
        .expect("the card title")
        .rect();
    let mut found = None;
    fn flat(s: &egui::Shape, r: egui::Rect, out: &mut Option<(String, usize)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| flat(s, r, out)),
            egui::Shape::Text(t) if r.expand(1.0).contains_rect(t.visual_bounding_rect()) => {
                *out = Some((t.galley.job.text.clone(), t.galley.rows.len()));
            }
            _ => {}
        }
    }
    for c in &h.output().shapes {
        flat(&c.shape, r, &mut found);
    }
    let (text, rows) = found.expect("the title's text is painted");
    (text, rows, r)
}

/// The status pill's kind label of the card whose title is `title`: of every painted copy of the set's kind text
/// inside the Sets column (final wave Group 6, ledger 202: the set editor paints the same text), the one nearest the
/// title vertically (the pill sits on the title's row or the row under it).
fn pill_of(h: &Harness<'static, AppModel>, id: &str, title: egui::Rect) -> egui::Rect {
    let panel = sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "sets").expect("the Sets column");
    let v = set_views(h.state())
        .into_iter()
        .find(|v| v.id == id)
        .expect("the set's view");
    let kind = sdc_gui::model::sets::kind_text(h.state(), &v);
    fn flat(s: &egui::Shape, kind: &str, out: &mut Vec<egui::Rect>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| flat(s, kind, out)),
            egui::Shape::Text(t) if t.galley.job.text == kind => out.push(t.visual_bounding_rect()),
            _ => {}
        }
    }
    let mut all = vec![];
    for c in &h.output().shapes {
        flat(&c.shape, kind, &mut all);
    }
    all.into_iter()
        .filter(|r| panel.contains_rect(*r))
        .min_by(|a, b| {
            (a.center().y - title.center().y)
                .abs()
                .total_cmp(&(b.center().y - title.center().y).abs())
        })
        .unwrap_or_else(|| panic!("the pill's {kind:?} label is painted in the Sets column"))
}

#[test]
fn warm_card_titles_reuse_the_fit_at_the_drawn_width() {
    use sdc_gui::ui::paint_counts::Work::{Title, TitleFit};
    let d = tempfile::tempdir().unwrap();
    let mut m = no_albedo_pair(d.path());
    m.sets_filter.search = "arch/wall".into();
    let mut h = harness(m);
    h.run_steps(4);
    let warm = Title.count();
    let fits = TitleFit.count();
    assert!(warm > 0 && fits > 0);
    h.run_steps(4);
    assert_eq!(
        Title.count(),
        warm,
        "unchanged card rebuilt its fitted title"
    );
    assert_eq!(TitleFit.count(), fits, "unchanged width repeated the fit");
    h.state_mut().sets_filter.search = "wall".into();
    h.run_steps(4);
    assert_eq!(
        Title.count(),
        warm + 1,
        "one measure for the changed title search"
    );
    assert_eq!(
        TitleFit.count(),
        fits + 1,
        "one fit at the drawn width after search changes"
    );
    let width_id = sdc_gui::ui::panel_ids::chosen_width_id("sets");
    h.ctx.data_mut(|d| d.insert_persisted(width_id, 380.0_f32));
    h.run_steps(4);
    assert_eq!(
        Title.count(),
        warm + 1,
        "resizing can reuse the measured text"
    );
    assert_eq!(
        TitleFit.count(),
        fits + 2,
        "one fit for the new label width"
    );
}

/// Task 9 review: a title too wide for the room beside the pill takes its own line (one line high, the pill
/// wholly below it) instead of wrapping mid-word beside the pill; with a set open, at 1055x770.
#[test]
fn a_title_too_wide_beside_the_pill_takes_its_own_line() {
    // Sets at its default 266 px at 1055x770 (nothing open): the one-row layout is chosen, and a 25-character
    // tail does not fit beside the pill. This is the state that is RED without the fix.
    own_line_case(
        [
            "architecture/whiterun/wrwoodbeamlong01",
            "architecture/windhelm/wrwoodbeamlong01",
        ],
        false,
    );
}

/// A pin of the narrower state: with a set open the Sets column is at its floor at 1055x770 and the narrow layout
/// was already chosen before the fix, so this passes on the old code too.
#[test]
fn a_title_in_the_narrowest_column_with_a_set_open_stays_on_one_line_above_the_pill() {
    own_line_case(
        [
            "architecture/whiterun/wrbeam01",
            "architecture/windhelm/wrbeam01",
        ],
        true,
    );
}

fn own_line_case(ids: [&str; 2], open: bool) {
    let d = tempfile::tempdir().unwrap();
    for (id, rgb) in ids.iter().zip([[90u8; 3], [60u8; 3]]) {
        let p = d.path().join("textures").join(format!("{id}.png"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb(rgb))
            .save(&p)
            .unwrap();
    }
    let m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let key = set_views(&m)
        .into_iter()
        .find(|v| v.id == ids[0])
        .unwrap()
        .key;
    let mut h = harness(m);
    h.set_size(egui::vec2(1055.0, 770.0));
    if open {
        h.state_mut().select_set(&key);
    }
    h.run();
    h.run();
    let panel = sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "sets").expect("the Sets column");
    for id in ids {
        // the narrow layout: the title beside the position and handle at the card's full width, the pill below
        let (text, rows, r) = title_probe(&h, id);
        assert_eq!(rows, 1, "{id}: one line, shown {text:?} in {r:?}");
        let pill = pill_of(&h, id, r);
        assert!(
            pill.min.y >= r.max.y - 1.0,
            "{id}: the pill {pill:?} lies below the title {r:?}"
        );
        assert!(
            r.height() < 1.5 * h.ctx.global_style().text_styles[&egui::TextStyle::Body].size + 6.0
                && panel.contains_rect(r),
            "{id}: {r:?} one line high inside the column {panel:?}"
        );
    }
}
#[test]
fn two_cards_sharing_a_leaf_never_read_the_same() {
    const IDS: [&str; 2] = [
        "architecture/whiterun/whiterunstonework/wallblocklarge01",
        "architecture/windhelm/whiterunstonework/wallblocklarge01",
    ];
    let d = tempfile::tempdir().unwrap();
    for (id, rgb) in IDS.iter().zip([[90u8; 3], [60u8; 3]]) {
        let p = d.path().join("textures").join(format!("{id}.png"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        image::ImageBuffer::from_fn(64, 64, |_, _| image::Rgb(rgb))
            .save(&p)
            .unwrap();
    }
    let m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let mut h = harness(m);
    h.run();
    let shown: Vec<String> = IDS
        .iter()
        .map(|id| {
            // the title's AccessKit name is the full id (HN6 `widget_info`)
            let r = h
                .query_all_by_label(id)
                .next()
                .expect("the card title")
                .rect();
            painted_in(&h, r)
        })
        .collect();
    assert!(
        IDS.iter().zip(&shown).all(|(id, s)| s != id),
        "premise: both titles are cut: {shown:?}"
    );
    assert_ne!(shown[0], shown[1], "two cards read the same");
    let measured = sdc_gui::ui::paint_counts::Work::ShortTitles;
    assert_eq!(
        measured.count(),
        2,
        "each cut card measured its alternatives once"
    );
    h.run_steps(4);
    assert_eq!(
        measured.count(),
        2,
        "steady frames rebuilt the short title forms"
    );
    for id in IDS {
        let (text, rows, r) = title_probe(&h, id);
        // these tails are wider than the whole card, so they may wrap, and only as the last resort
        let last_resort = text == format!("…/{}", id.split_once('/').unwrap().1);
        assert!(
            rows == 1 || last_resort,
            "{id}: {rows} lines for {text:?} in {r:?}"
        );
        // on its own row: the pill lies wholly below the title, never beside it
        let pill = pill_of(&h, id, r);
        assert!(
            pill.min.y >= r.max.y - 1.0,
            "{id}: the pill {pill:?} lies below the title {r:?}"
        );
    }
    assert!(
        shown[0].ends_with("whiterun/whiterunstonework/wallblocklarge01")
            && shown[1].ends_with("windhelm/whiterunstonework/wallblocklarge01"),
        "each title ends with its unique tail (Task 1's `short`): {shown:?}"
    );
}
