//! M6 Track F3a: the selection gestures and the bulk panel, drawn.
mod support;
use eframe::egui::{Key, Modifiers};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::Subtype;
use sdc_gui::model::{AppModel, Dialog, Selection};
use support::*;

const ROCK: &str = "landscape/rock01.png";
const LEAF: &str = "landscape/leaf.png";

/// A harness over `model`. The crate bundles no fonts, so the harness needs the system fonts, as the app does.
fn harness(model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    h
}

#[test]
fn ctrl_click_on_tiles_selects_both_and_the_heading_counts_them() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    h.get_by_label("rock01.png").click();
    h.run();
    h.get_by_label("leaf.png")
        .click_modifiers(Modifiers::COMMAND);
    h.run();
    assert_eq!(
        h.state().selected_items(),
        vec![ROCK.to_string(), LEAF.to_string()]
    );
    assert_eq!(h.query_all_by_label("2 selected").count(), 1);
}

/// Final fix wave A7: Reset all for one texture asks in the singular.
#[test]
fn reset_all_of_one_texture_asks_in_the_singular() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.dialog = Some(Dialog::ConfirmResetAll(vec![ROCK.into()]));
    let mut h = harness(m);
    h.run();
    h.get_by_label("Reset all settings of 1 texture?");
}

/// Final fix wave A7: two sets' kind chip that holds one texture heads the bulk panel in the singular.
#[test]
fn a_one_texture_chip_heads_the_panel_in_the_singular() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    // The second set without its first member: that member's kind is in one set only.
    let (id, member) = sdc_gui::model::sets::views(&m)
        .iter()
        .filter(|v| !v.key.is_empty())
        .nth(1)
        .map(|v| (v.id.clone(), v.members[0].clone()))
        .expect("two sets");
    m.remove_member(&id, &member).unwrap();
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let keys: Vec<String> = sdc_gui::model::sets::views(&m)
        .iter()
        .filter(|v| !v.key.is_empty())
        .map(|v| v.key.clone())
        .collect();
    m.click_set(&keys[0], false, false);
    m.click_set(&keys[1], true, false);
    let view = m.bulk_view();
    let chip = view
        .chips
        .iter()
        .find(|c| c.members.len() == 1)
        .unwrap_or_else(|| {
            let n: Vec<usize> = view.chips.iter().map(|c| c.members.len()).collect();
            panic!("precondition: a one-texture chip in {n:?}")
        });
    m.bulk.kind = chip.kind;
    let mut h = harness(m);
    h.run();
    h.get_by_label("1 texture selected");
}

#[test]
fn right_click_on_a_selected_tile_keeps_the_selection_and_opens_the_menu() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let mut h = harness(m);
    h.run();
    // rock01.png is selected but not active, so only its tile carries that label.
    h.get_by_label("rock01.png").click_secondary();
    h.run();
    assert_eq!(h.state().selected_items().len(), 2);
    h.get_by_label("Reset all settings for 2 selected…").click();
    h.run();
    assert!(matches!(h.state().dialog, Some(Dialog::ConfirmResetAll(_))));
}

#[test]
fn ctrl_a_selects_every_visible_texture_and_esc_keeps_them() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    let shown = h.state().visible_items().len();
    assert_eq!(h.state().selected_items().len(), shown);
    h.key_press(Key::Escape);
    h.run();
    // Owner amendment G (Note FB11): Esc never clears the selection.
    assert_eq!(h.state().selected_items().len(), shown);
}

#[test]
fn keys_do_not_reach_the_selection_behind_a_dialog() {
    // Review Focus 3.
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.dialog = Some(Dialog::Error("x".into()));
    let mut h = harness(m);
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().selection, Selection::Item(ROCK.into()));
    assert_eq!(h.state().selected_items().len(), 1);
}

#[test]
fn two_selected_sets_are_counted_in_the_sets_heading() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let keys: Vec<String> = sdc_gui::model::sets::views(&m)
        .iter()
        .filter(|v| !v.key.is_empty())
        .map(|v| v.key.clone())
        .collect();
    m.click_set(&keys[0], false, false);
    m.click_set(&keys[1], true, false);
    let mut h = harness(m);
    h.run();
    assert_eq!(h.query_all_by_label("2 selected").count(), 1);
}

#[test]
fn esc_closes_the_slot_commands_modal_and_keeps_the_selection() {
    // Review I1: a modal not tied to `model.dialog` owns Esc.
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let key = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| !v.key.is_empty())
        .map(|v| v.key.clone())
        .unwrap();
    m.click_set(&key, false, false);
    let mut state = sdc_gui::ui::State::default();
    state.slot_cmds.open = Some(key.clone());
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, sdc_gui::ui::State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, state),
        );
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().1.slot_cmds.open.is_none(), "Esc closes the modal");
    assert_eq!(h.state().0.selection, Selection::Set(key));
}

#[test]
fn right_click_on_an_unselected_tile_selects_it_alone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let mut h = harness(m);
    h.run();
    // Neither selected tile carries this label.
    h.get_by_label("odd.png").click_secondary();
    h.run();
    assert_eq!(
        h.state().selected_items(),
        vec!["landscape/odd.png".to_string()]
    );
}

#[test]
fn ctrl_click_on_a_set_pill_adds_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let views = sdc_gui::model::sets::views(&m);
    let keys: Vec<&sdc_gui::model::sets::SetView> =
        views.iter().filter(|v| !v.key.is_empty()).collect();
    let (a, b) = (keys[0].key.clone(), keys[1].key.clone());
    let b_name = keys[1].id.clone();
    m.click_set(&a, false, false);
    let mut h = harness(m);
    h.run();
    h.get_by_label(&b_name).click_modifiers(Modifiers::COMMAND);
    h.run();
    assert_eq!(h.state().selected_sets(), vec![a, b]);
}

#[test]
fn two_selected_textures_show_the_bulk_panel_with_mixed_values() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    // Note JI4: both BC7 (plain diffuses, preflight 4.1), so the Effort row edits `encoder.effort` on both.
    for k in [ROCK, LEAF] {
        m.decide(
            k,
            sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
        );
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        m.set_format(
            k,
            engine::config::Format::Bc7,
            engine::config::ColorTag::Unorm,
        );
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    }
    m.set_field(ROCK, "encoder.effort", serde_json::json!("veryfast"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let effort = m
        .bulk_view()
        .rows
        .iter()
        .find(|r| r.label == "Effort")
        .cloned()
        .unwrap();
    assert_eq!(
        (effort.field, effort.value, effort.applies),
        ("encoder.effort", None, 2),
        "effort mixed"
    );
    let mut h = harness(m);
    h.run();
    h.get_by_label("2 textures selected");
    assert!(h.query_all_by_label("— mixed").count() >= 1);
    // The single panel's heading for the active texture is gone.
    assert!(h.query_by_label("Undecided").is_none());
}

#[test]
fn two_selected_sets_show_the_kind_chips() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let keys: Vec<String> = sdc_gui::model::sets::views(&m)
        .iter()
        .filter(|v| !v.key.is_empty())
        .map(|v| v.key.clone())
        .collect();
    m.click_set(&keys[0], false, false);
    m.click_set(&keys[1], true, false);
    let mut h = harness(m);
    h.run();
    h.get_by_label("2 sets selected");
    h.get_by_label_contains("All (").click();
    h.run();
    assert_eq!(h.state().bulk.kind, None);
}

#[test]
fn the_copy_dialog_ticks_every_field_and_check_none_clears_them() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_field(LEAF, "format", serde_json::json!("bc5"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    m.open_copy_settings();
    let mut h = harness(m);
    h.run();
    h.get_by_label("Check none").click();
    h.run();
    assert!(matches!(
        &h.state().dialog,
        Some(Dialog::CopySettings { fields, .. }) if fields.iter().all(|f| !f.1)
    ));
}

#[test]
fn the_copy_dialog_applies_nothing_with_nothing_ticked() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_field(LEAF, "format", serde_json::json!("bc5"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    m.open_copy_settings();
    let mut h = harness(m);
    h.run();
    h.get_by_label("Check none").click();
    h.run();
    let apply = h.get_by_label("Apply");
    assert!(apply.accesskit_node().is_disabled());
    apply.click();
    h.run();
    assert!(matches!(
        &h.state().dialog,
        Some(Dialog::CopySettings { .. })
    ));
}

#[test]
fn a_second_frame_with_nothing_changed_does_not_recompute_the_bulk_view() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let mut h = harness(m);
    h.run();
    let n = h.state().bulk_computes();
    assert!(n >= 1);
    h.run();
    h.run();
    assert_eq!(h.state().bulk_computes(), n);
}

/// Backlog L14: the re-encode split (it builds a writers map) is part of the cached view, not redone every frame.
#[test]
fn the_reencode_split_is_computed_with_the_bulk_view_not_per_frame() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let mut h = harness(m);
    h.run();
    let n = h.state().reencode_splits();
    assert!(n >= 1, "the panel asked for the split");
    h.step();
    h.step();
    h.step();
    assert_eq!(h.state().reencode_splits(), n);
}

#[test]
fn a_mixed_slider_shows_mixed_and_an_idle_frame_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    for k in [ROCK, LEAF] {
        m.set_field(k, "alpha.policy", serde_json::json!("cutout"));
    }
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.set_field(ROCK, "alpha.threshold", serde_json::json!(100));
    m.set_field(LEAF, "alpha.threshold", serde_json::json!(200));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let before = m.bulk_view().rows.clone();
    let row = before
        .iter()
        .find(|r| r.field == "alpha.threshold")
        .unwrap();
    assert!(row.applies > 0 && row.value.is_none(), "{row:?}");
    let mut h = harness(m);
    h.run();
    h.run();
    assert!(h.query_all_by_value("— mixed").count() >= 1);
    // The slider keeps its own name, which says more than the row's "Threshold" label (E28).
    widget(&h, eframe::egui::accesskit::Role::Slider, "Alpha threshold");
    assert_eq!(h.state_mut().bulk_view().rows, before);
}

/// Review C4 (Note FB4): a row that applies to none of the selection shows "— not set", never the word `null`.
#[test]
fn a_bulk_row_that_applies_to_nothing_reads_not_set_never_null() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let rows = m.bulk_view().rows.clone();
    assert!(
        rows.iter().any(|r| r.applies == 0),
        "the fixture has a row that applies to 0"
    );
    let mut h = harness(m);
    h.run();
    h.run();
    h.get_by_label("2 textures selected");
    let shown = |h: &Harness<'static, AppModel>, s: &str| {
        h.query_all_by_label(s).count() + h.query_all_by_value(s).count()
    };
    assert_eq!(shown(&h, "null"), 0, "the word null is never drawn");
    assert!(shown(&h, "— not set") >= 1, "an unset combo says so");
}

/// Note HI9: the bulk panel's rows have a `?` after the label, with the row's help on hover.
#[test]
fn a_bulk_row_has_a_question_mark() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    let mut h = harness(m);
    h.run();
    h.get_by_label("2 textures selected");
    let text = sdc_gui::model::fields::field_help("encoder.effort").unwrap();
    assert!(h.query_all_by_label_contains(text).next().is_none());
    h.get_by_label("Help: Effort").hover();
    h.run();
    assert!(h.query_all_by_label_contains(text).next().is_some());
}

/// Spec 4.7 (Z4/Z8 amended), Note JI3: the bulk panel loops the texture panel's groups, so it has the Size group's
/// rows and DDS header; a Resize pick writes every selected texture in one undo step.
#[test]
fn the_bulk_panel_has_size_and_dds_header_rows() {
    use eframe::egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    // Tall enough that the Size group is on screen.
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1280.0, 1600.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    h.run();
    h.get_by_label("2 textures selected");
    h.get_by_label("Size");
    // The bulk combos have no name of their own yet: a row's combo is the one on its label's line.
    let combo_on = |h: &Harness<'static, AppModel>, label: &str| {
        let y = h
            .query_all(egui_kittest::kittest::by().role(Role::Label).label(label))
            .next()
            .unwrap_or_else(|| panic!("a {label} row"))
            .rect()
            .center()
            .y;
        let on: Vec<_> = h
            .query_all_by_role(Role::ComboBox)
            .map(|n| n.rect())
            .filter(|r| r.min.y <= y && y <= r.max.y)
            .collect();
        assert_eq!(on.len(), 1, "one combo on the {label} row: {on:?}");
        on[0]
    };
    combo_on(&h, "DDS header");
    let resize = combo_on(&h, "Resize");
    h.query_all_by_role(Role::ComboBox)
        .find(|n| n.rect() == resize)
        .unwrap()
        .click();
    h.run();
    h.get_by_label("Half").click();
    h.run();
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    for k in [ROCK, LEAF] {
        assert_eq!(
            h.state().project.as_ref().unwrap().file_value(k, "resize"),
            Some(serde_json::json!("half")),
            "{k}"
        );
    }
    assert_eq!(h.state().undo_label(), Some("Change Resize on 2 textures"));
}

fn key_of(m: &AppModel, name: &str) -> String {
    m.items
        .iter()
        .find(|i| i.key.ends_with(name))
        .expect("an item with that name")
        .key
        .clone()
}

fn gradient(path: &std::path::Path) {
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
    })
    .save(path)
    .unwrap();
}

/// Nine lone files that ask for a type (GY1: unknown suffix), and `keep.png`, which the app has decided.
fn nine_waiting(dir: &std::path::Path) -> AppModel {
    nine_waiting_with(dir, |_| {})
}

/// `nine_waiting` with `extra` run on the folder before it is opened.
fn nine_waiting_with(dir: &std::path::Path, extra: impl FnOnce(&std::path::Path)) -> AppModel {
    let c = dir.join("textures/clutter");
    std::fs::create_dir_all(&c).unwrap();
    for i in 1..=9 {
        gradient(&c.join(format!("x{i}_bump.png")));
    }
    gradient(&c.join("keep.png"));
    extra(&c);
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    assert!(m.wait_idle(WAIT));
    let keep = key_of(&m, "keep.png");
    m.set_field(&keep, "subtype", serde_json::json!("diffuse"));
    assert!(m.wait_idle(WAIT));
    m
}

const WAIT: std::time::Duration = std::time::Duration::from_secs(60);

#[test]
fn bulk_card_asks_for_the_undecided_ones() {
    let d = tempfile::tempdir().unwrap();
    let mut m = nine_waiting(d.path());
    m.select_all_visible();
    assert_eq!(
        m.selected_items()
            .iter()
            .filter(|k| m.subtype_question(k).is_some())
            .count(),
        9
    );
    let mut h = harness(m);
    h.run();
    assert!(h
        .query_by_label("What kind are these 9 textures?")
        .is_some());
    assert!(h
        .query_by_label("1 other selected texture already has a type.")
        .is_some());
    h.get_by_label("Type for 9 textures").click();
    h.run();
    h.get_by_label("Height (parallax)").scroll_to_me();
    h.run();
    h.get_by_label("Height (parallax)").click();
    h.run();
    h.get_by_label("Apply to 9").click();
    h.run();
    assert!(h.state_mut().wait_idle(WAIT));
    assert!(h.state().undecided_keys().is_empty());
}

type Docs = (String, Option<String>);

fn docs(m: &AppModel) -> Docs {
    (
        m.project.as_ref().unwrap().text(),
        m.sets_doc.as_ref().map(|s| s.text()),
    )
}

#[test]
fn apply_to_undecided_is_one_undo_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = nine_waiting(d.path());
    let before = docs(&m);
    let (steps, generation) = (m.history.undo.len(), m.generation());
    m.select_all_visible();
    let keys = m.selected_undecided();
    m.decide_many(&keys, Subtype::Height);
    assert_eq!(m.history.undo.len(), steps + 1, "one undo step");
    assert_eq!(m.generation(), generation + 1, "one plan");
    assert!(m.wait_idle(WAIT));
    let new_sets: Vec<_> = m
        .messages
        .entries()
        .filter(|e| e.text.contains("new sets"))
        .collect();
    assert_eq!(
        new_sets.len(),
        1,
        "one line for the nine new sets (ruling 6)"
    );
    assert!(
        new_sets[0].text.starts_with("9 new sets: ") && new_sets[0].text.ends_with(" and 6 more."),
        "{}",
        new_sets[0].text
    );
    m.undo();
    assert!(m.wait_idle(WAIT));
    assert_eq!(docs(&m), before, "both documents byte for byte");
    assert_eq!(m.undecided_keys().len(), 9);
}

fn own_types(m: &AppModel) -> Vec<(String, Option<serde_json::Value>)> {
    let p = m.project.as_ref().unwrap();
    m.items
        .iter()
        .map(|i| (i.key.clone(), p.file_value(&i.key, "subtype")))
        .collect()
}

#[test]
fn the_bulk_type_card_uses_only_selected_actionable_type_questions() {
    let d = tempfile::tempdir().unwrap();
    let mut m = nine_waiting_with(d.path(), |c| {
        image::ImageBuffer::from_fn(64, 64, |x, y| {
            image::Rgba([(x * 4) as u8, (y * 4) as u8, 90u8, (x * 4) as u8])
        })
        .save(c.join("halo.png"))
        .unwrap();
        // Waits for a type (suggested diffuse), and fills the Diffuse slot of its own set.
        gradient(&c.join("lock_i.png"));
    });
    // An alpha-only question: the type is decided, the alpha meaning is not.
    let halo = key_of(&m, "halo.png");
    m.set_field(&halo, "subtype", serde_json::json!("diffuse"));
    assert!(m.wait_idle(WAIT));
    assert!(m.subtype_question(&halo).is_none(), "halo asks no type");
    assert!(
        m.item(&halo)
            .unwrap()
            .decisions
            .contains(&sdc_gui::model::items::Decision::AlphaMeaning),
        "halo asks its alpha"
    );
    // A waiting file in a locked slot.
    let (x1, x2, x5) = (
        key_of(&m, "x1_bump.png"),
        key_of(&m, "lock_i.png"),
        key_of(&m, "x5_bump.png"),
    );
    // A lone unknown file has no set card until it is dropped on the empty area, which gives it one.
    for k in [&x1, &x5] {
        m.drop_new_set(k).unwrap();
        assert!(m.wait_idle(WAIT));
    }
    let set_of = |m: &AppModel, k: &str| m.set_of_item_probe(k).expect("a set").id;
    m.set_slot_lock(&set_of(&m, &x2), 1, true).unwrap();
    assert!(m.wait_idle(WAIT));
    assert!(
        sdc_gui::model::sets::locked_items(&m).contains_key(&x2),
        "x2 fills a locked slot"
    );
    // Review runs x5's set only: x1 is selected but outside it, x5 is inside and not selected.
    let scope = m.set_of_item_probe(&x5).unwrap().key;
    m.export_sets([scope].into(), "Export set".into());
    assert!(m.wait_idle(WAIT));
    assert!(m.undecided_keys().contains(&x5) && !m.undecided_keys().contains(&x1));
    let keep = key_of(&m, "keep.png");
    m.click_item(&x1, false, false);
    for k in [&halo, &x2, &keep] {
        m.click_item(k, true, false);
    }
    assert_eq!(m.selected_undecided(), vec![x1.clone()]);
    let (before, types, steps) = (docs(&m), own_types(&m), m.history.undo.len());
    let mut h = harness(m);
    h.run();
    h.get_by_label("What kind is this texture?");
    // keep.png and halo.png (its alpha is the open question, its type is decided).
    h.get_by_label("2 other selected textures already have a type.");
    h.get_by_label("Type for 1 texture").click();
    h.run();
    h.get_by_label("Height (parallax)").scroll_to_me();
    h.run();
    h.get_by_label("Height (parallax)").click();
    h.run();
    h.get_by_label("Apply to 1").click();
    h.run();
    assert!(h.state_mut().wait_idle(WAIT));
    let m = h.state_mut();
    let after = own_types(m);
    let changed: Vec<&str> = after
        .iter()
        .zip(&types)
        .filter(|(a, b)| a != b)
        .map(|(a, _)| a.0.as_str())
        .collect();
    assert_eq!(changed, vec![x1.as_str()], "only x1's type is written");
    assert_eq!(
        m.project.as_ref().unwrap().file_value(&x1, "subtype"),
        Some(serde_json::json!("height"))
    );
    assert_eq!(
        m.item(&x1).unwrap().job.as_ref().unwrap().profile.subtype,
        Subtype::Height,
        "the plan holds the new type"
    );
    assert!(m.undecided_keys().contains(&x5), "x5 still waits");
    assert!(
        m.subtype_question(&x2).is_some(),
        "the locked file still waits"
    );
    assert_eq!(m.history.undo.len(), steps + 1);
    m.undo();
    assert!(m.wait_idle(WAIT));
    assert_eq!(docs(m), before, "one Undo restores both texts");
}

#[test]
fn a_stale_bulk_type_choice_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = nine_waiting(d.path());
    m.select_all_visible();
    let keys = m.selected_undecided();
    assert_eq!(keys.len(), 9);
    // The selection moved on to a typed file before the click landed.
    let keep = key_of(&m, "keep.png");
    m.click_item(&keep, false, false);
    let (before, steps) = (docs(&m), m.history.undo.len());
    m.decide_many(&keys, Subtype::Height);
    assert_eq!(
        docs(&m),
        before,
        "no write for a texture no longer selected"
    );
    assert_eq!(m.history.undo.len(), steps);
    // An sdc.toml the app cannot read is read-only for it: a choice that was drawn enabled writes nothing.
    m.select_all_visible();
    let keys = m.selected_undecided();
    m.save().unwrap();
    std::fs::write(m.root.clone().unwrap().join("sdc.toml"), "[[[ not toml").unwrap();
    m.reload_documents();
    assert!(
        m.toml_read_only_reason().is_some(),
        "the file is read-only for the app"
    );
    let (before, steps) = (docs(&m), m.history.undo.len());
    m.decide_many(&keys, Subtype::Height);
    assert_eq!(docs(&m), before, "read-only: nothing written");
    assert_eq!(m.history.undo.len(), steps);
}

#[test]
fn a_second_bulk_type_cannot_overwrite_a_held_choice() {
    let d = tempfile::tempdir().unwrap();
    let mut m = nine_waiting(d.path());
    m.select_all_visible();
    let keys = m.selected_undecided();
    let before = docs(&m);
    m.hold_plan_results = true;
    let mut h = harness(m);
    h.run();
    h.get_by_label("Type for 9 textures").click();
    h.run();
    h.get_by_label("Height (parallax)").scroll_to_me();
    h.run();
    h.get_by_label("Height (parallax)").click();
    h.run();
    h.get_by_label("Apply to 9").click();
    h.run();
    let after_one = docs(h.state());
    let (steps, generation) = (h.state().history.undo.len(), h.state().generation());
    assert_ne!(after_one, before, "click one wrote");
    assert_eq!(
        h.state().fix_off().as_deref(),
        Some("Wait for the plan to finish")
    );
    assert!(h.get_by_label("Apply to 9").accesskit_node().is_disabled());
    h.get_by_label("Apply to 9").click();
    h.run();
    h.state_mut().decide_many(&keys, Subtype::Diffuse);
    let m = h.state_mut();
    for k in &keys {
        assert_eq!(
            m.project.as_ref().unwrap().file_value(k, "subtype"),
            Some(serde_json::json!("height")),
            "{k}"
        );
    }
    assert_eq!(docs(m), after_one);
    assert_eq!((m.history.undo.len(), m.generation()), (steps, generation));
    m.hold_plan_results = false;
    for msg in m.take_held() {
        m.handle_held(msg);
    }
    assert!(m.wait_idle(WAIT));
    for k in &keys {
        let st = m.item(k).unwrap().job.as_ref().unwrap().profile.subtype;
        assert_eq!(st, Subtype::Height, "{k}");
    }
    m.undo();
    assert!(m.wait_idle(WAIT));
    assert_eq!(docs(m), before, "one Undo restores both texts");
}
