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
    assert!(h.query_all_by_label("— mixed · applies to 2 of 2").count() >= 1);
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

/// The lines `before` has and `after` lacks, and the lines `after` has and `before` lacks (trimmed, in order).
fn changed_lines(before: &str, after: &str) -> (Vec<String>, Vec<String>) {
    let lines = |t: &str| -> Vec<String> { t.lines().map(|l| l.trim().to_string()).collect() };
    let (b, a) = (lines(before), lines(after));
    let gone = b.iter().filter(|l| !a.contains(l)).cloned().collect();
    let new = a.iter().filter(|l| !b.contains(l)).cloned().collect();
    (gone, new)
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
    let (before, steps) = (docs(&m), m.history.undo.len());
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
    // Both documents, line by line: x1's `subtype` is the only [files] change, and the sets document gains
    // only what the new type's follow writes name.
    let (files_gone, files_new) = changed_lines(&before.0, &docs(m).0);
    assert!(files_gone.is_empty(), "{files_gone:?}");
    assert_eq!(
        files_new,
        vec![
            format!("[files.\"{x1}\"]"),
            "subtype = \"height\"".to_string()
        ]
    );
    let (sets_gone, sets_new) = changed_lines(
        before.1.as_deref().unwrap_or_default(),
        docs(m).1.as_deref().unwrap_or_default(),
    );
    assert!(
        sets_gone.is_empty() && sets_new.is_empty(),
        "the sets document is untouched: {sets_gone:?} {sets_new:?}"
    );
    assert_eq!(docs(m).1, before.1);
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
    assert!(m.wait_idle(WAIT));
    assert!(
        m.toml_read_only_reason().is_some(),
        "the file is read-only for the app"
    );
    assert!(m.fix_off().is_some_and(|r| r.contains("read-only")));
    assert!(
        !m.selected_undecided().is_empty(),
        "the choice is still on offer, so only the read-only guard stops it"
    );
    let (before, steps) = (docs(&m), m.history.undo.len());
    m.decide_many(&keys, Subtype::Height);
    assert_eq!(docs(&m), before, "read-only: nothing written");
    assert_eq!(m.history.undo.len(), steps);
}

/// Open the card's combo and pick Height (the entry sits at the window edge, so it is scrolled to first).
fn pick_height(h: &mut Harness<'static, AppModel>, combo: &str) {
    h.get_by_label(combo).click();
    h.run();
    h.get_by_label("Height (parallax)").scroll_to_me();
    h.run();
    h.get_by_label("Height (parallax)").click();
    h.run();
}

#[test]
fn the_pick_follows_the_selection() {
    let d = tempfile::tempdir().unwrap();
    let mut m = nine_waiting(d.path());
    m.select_all_visible();
    let (x1, x2) = (key_of(&m, "x1_bump.png"), key_of(&m, "x2_bump.png"));
    let mut h = harness(m);
    h.run();
    pick_height(&mut h, "Type for 9 textures");
    assert!(!h.get_by_label("Apply to 9").accesskit_node().is_disabled());
    // Another set of textures asks: the old pick is not carried over to it.
    h.state_mut().click_item(&x1, false, false);
    h.state_mut().click_item(&x2, true, false);
    h.run();
    let combo = h.get_by_label("Type for 2 textures");
    assert_eq!(
        combo.accesskit_node().value().as_deref(),
        Some("Choose a type"),
        "no pick shown"
    );
    assert!(h.get_by_label("Apply to 2").accesskit_node().is_disabled());
    // Back to the nine: the first pick is gone too.
    h.state_mut().select_all_visible();
    h.run();
    assert!(h.get_by_label("Apply to 9").accesskit_node().is_disabled());
}

/// `a_bump.png` is under `textures/` (no grass context); `b_bump.png` is not (a grass context, Note FC10).
fn mixed_grass(dir: &std::path::Path) -> AppModel {
    std::fs::create_dir_all(dir.join("textures/clutter")).unwrap();
    std::fs::create_dir_all(dir.join("loose")).unwrap();
    gradient(&dir.join("textures/clutter/a_bump.png"));
    gradient(&dir.join("loose/b_bump.png"));
    gradient(&dir.join("loose/c_bump.png"));
    let mut m = opened_as_the_app(dir.to_path_buf(), dir.join("export"));
    assert!(m.wait_idle(WAIT));
    m
}

#[test]
fn the_card_offers_only_types_every_waiting_texture_is_offered() {
    use sdc_gui::model::labels::subtype_name;
    let d = tempfile::tempdir().unwrap();
    let mut m = mixed_grass(d.path());
    let (a, b) = (key_of(&m, "a_bump.png"), key_of(&m, "b_bump.png"));
    let grass = |m: &AppModel, k: &str| {
        engine::policy::grass_context(m.item(k).unwrap().job.as_ref().unwrap())
    };
    assert!(
        !grass(&m, &a) && grass(&m, &b),
        "the two files differ in grass context"
    );
    let grass_only = subtype_name(Subtype::ComplexGrass);
    // Control: two grass-context files are offered complex grass.
    let c = key_of(&m, "c_bump.png");
    m.click_item(&b, false, false);
    m.click_item(&c, true, false);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Type for 2 textures").click();
    h.run();
    assert!(
        h.query_by_label(&grass_only).is_some(),
        "offered to a grass file"
    );
    // Both selected: the type is offered to one of them only, so the combo leaves it out.
    let d2 = tempfile::tempdir().unwrap();
    let mut m = mixed_grass(d2.path());
    let (a, b) = (key_of(&m, "a_bump.png"), key_of(&m, "b_bump.png"));
    m.click_item(&a, false, false);
    m.click_item(&b, true, false);
    let mut h = harness(m);
    h.run();
    h.get_by_label("Type for 2 textures").click();
    h.run();
    assert!(h.query_by_label(&grass_only).is_none(), "not offered to a");
    assert!(h.query_by_label("Height (parallax)").is_some());
    // A dispatch of it anyway (the offering is read again at click time) reaches only the texture it is offered to.
    let m = h.state_mut();
    let before = docs(m);
    m.decide_many(&[a.clone(), b.clone()], Subtype::ComplexGrass);
    let (gone, new) = changed_lines(&before.0, &docs(m).0);
    assert!(gone.is_empty(), "{gone:?}");
    assert_eq!(
        new,
        vec![
            format!("[files.\"{b}\"]"),
            "subtype = \"complex_grass\"".to_string()
        ],
        "{a} is not written"
    );
}

#[test]
fn a_locked_texture_that_has_a_type_counts_as_a_peer() {
    let d = tempfile::tempdir().unwrap();
    let mut m = nine_waiting_with(d.path(), |c| gradient(&c.join("lock_i.png")));
    let (x1, locked) = (key_of(&m, "x1_bump.png"), key_of(&m, "lock_i.png"));
    m.set_field(&locked, "subtype", serde_json::json!("diffuse"));
    assert!(m.wait_idle(WAIT));
    let id = m.set_of_item_probe(&locked).expect("a set").id;
    m.set_slot_lock(&id, 1, true).unwrap();
    assert!(m.wait_idle(WAIT));
    assert!(sdc_gui::model::sets::locked_items(&m).contains_key(&locked));
    m.click_item(&x1, false, false);
    m.click_item(&locked, true, false);
    let mut h = harness(m);
    h.run();
    h.get_by_label("What kind is this texture?");
    h.get_by_label("1 other selected texture already has a type.");
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

/// The BC7 weighting combo of the open bulk panel, found by its row's name.
fn weighting_combo<'a>(h: &'a Harness<'static, AppModel>) -> egui_kittest::Node<'a> {
    use eframe::egui::accesskit::Role;
    let combos: Vec<_> = h
        .query_all_by_role(Role::ComboBox)
        .filter(|n| n.accesskit_node().label().as_deref() == Some("BC7 weighting"))
        .collect();
    assert_eq!(combos.len(), 1, "one BC7 weighting combo");
    combos.into_iter().next().unwrap()
}

/// E14: the bulk BC7 weighting combo offers Perceptual to the diffuse even when the normal is the active texture,
/// whose own lock used to disable the entry for everyone.
#[test]
fn perceptual_reaches_the_diffuse_with_a_normal_active() {
    use sdc_gui::model::bulk::{plan_bulk_edit, BulkOp, Change};
    use sdc_gui::model::fields::WEIGHTING;
    const NORMAL: &str = "landscape/rock01_normal.png";
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    for k in [ROCK, NORMAL] {
        m.set_field(k, WEIGHTING, serde_json::json!("linear"));
        assert!(m.wait_idle(WAIT));
    }
    m.click_item(ROCK, false, false);
    m.click_item(NORMAL, true, false);
    assert_eq!(
        m.selection,
        Selection::Item(NORMAL.to_string()),
        "the normal is active"
    );
    let keys = vec![ROCK.to_string(), NORMAL.to_string()];
    assert!(keys.iter().all(|k| m.is_selected(k, false)));
    let plan = plan_bulk_edit(
        &m,
        &keys,
        WEIGHTING,
        &BulkOp::Set(serde_json::json!("perceptual")),
    );
    assert_eq!(plan[0], (ROCK.to_string(), Change::Write));
    assert!(matches!(plan[1].1, Change::Skip(_)), "{plan:?}");
    let weighting = |m: &AppModel, k: &str| {
        let p = m.project.as_ref().unwrap();
        (
            p.file_value(k, "encoder.perceptual"),
            p.file_value(k, WEIGHTING),
        )
    };
    let normal_before = weighting(&m, NORMAL);
    let mut h = harness(m);
    h.run();
    weighting_combo(&h).scroll_to_me();
    h.run();
    weighting_combo(&h).click();
    h.run();
    let entry = h
        .query_all_by_label_contains("Perceptual")
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Perceptual"))
        })
        .expect("the Perceptual entry");
    assert!(
        !entry.accesskit_node().is_disabled(),
        "enabled for the diffuse"
    );
    assert_eq!(
        entry.accesskit_node().label().as_deref(),
        Some("Perceptual (1 of 2)")
    );
    entry.click();
    h.run();
    assert!(h.state_mut().wait_idle(WAIT));
    h.run();
    let m = h.state();
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(ROCK, "encoder.perceptual"),
        Some(serde_json::json!(true))
    );
    assert_eq!(
        weighting(m, NORMAL),
        normal_before,
        "the normal is untouched"
    );
}

/// Three normals and a plain diffuse, mipmaps on, the first `selected` of them selected: the reach of each filter,
/// weighting, format and tag differs between the normals and the diffuse.
fn reach_fixture(dir: &std::path::Path, selected: usize) -> (AppModel, Vec<String>) {
    use engine::config::{GreenConvention, Subtype};
    use image::{ImageBuffer, Rgb};
    use sdc_gui::model::Answer;
    let r = dir.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    for n in ["a_n.png", "b_n.png", "c_n.png"] {
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
            .save(r.join(n))
            .unwrap();
    }
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(r.join("plain.png"))
        .unwrap();
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(dir.join("export")),
        ..sdc_gui::settings::GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.open_folder(dir.join("textures"));
    assert!(m.wait_idle(WAIT));
    choose_export(&mut m, dir.join("export"));
    let keys: Vec<String> = ["a_n", "b_n", "c_n", "plain"]
        .iter()
        .map(|n| format!("landscape/{n}.png"))
        .collect();
    for k in &keys[..3] {
        m.decide(k, Answer::Subtype(Subtype::Normal));
        m.decide(k, Answer::Green(GreenConvention::Directx));
    }
    m.decide(&keys[3], Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(WAIT));
    for k in &keys {
        m.set_field(k, "mips.levels", serde_json::json!("full"));
    }
    assert!(m.wait_idle(WAIT));
    m.click_item(&keys[0], false, false);
    for k in &keys[1..selected] {
        m.click_item(k, true, false);
    }
    assert_eq!(m.bulk_view().keys.len(), selected);
    (m, keys[..selected].to_vec())
}

/// A harness 1100 px wide (the narrowest width the layout promises) and tall enough for every popup.
fn narrow_harness(m: AppModel) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1100.0, 1500.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m)
}

/// Open the popup of the bulk row `row` over `selected` textures and compare every entry (text, enabled) with the
/// planner's count for each independently listed option. Every entry stays inside the 1100 px window. Returns the
/// entries.
fn popup_entries(row: &str, selected: usize) -> Vec<(String, bool)> {
    let d = tempfile::tempdir().unwrap();
    let (m, keys) = reach_fixture(d.path(), selected);
    popup_of(row, m, keys)
}

/// [`popup_entries`] over the selection `keys` of `m`.
fn popup_of(row: &str, m: AppModel, keys: Vec<String>) -> Vec<(String, bool)> {
    use eframe::egui::accesskit::Role;
    use sdc_gui::model::bulk::{bulk_encodings, plan_bulk_edit, BulkOp, Change};
    use sdc_gui::model::fields::{WEIGHTING, WEIGHTING_OPTIONS};
    use sdc_gui::model::labels::UNORM_LABEL;
    let selected = keys.len();
    let field = match row {
        "Filter" => sdc_gui::model::fields::FILTER_FIELD.0,
        "BC7 weighting" => WEIGHTING,
        "Format" => "format",
        _ => "color.tag",
    };
    let options: Vec<(String, serde_json::Value)> = match field {
        f if f == sdc_gui::model::fields::FILTER_FIELD.0 => ["Box", "Tent", "Mitchell", "Kaiser"]
            .map(|l| (l.to_string(), serde_json::json!(l.to_lowercase())))
            .to_vec(),
        WEIGHTING => WEIGHTING_OPTIONS
            .iter()
            .map(|o| (o.1.to_string(), serde_json::json!(o.0)))
            .collect(),
        "format" => bulk_encodings(&m, &keys)
            .into_iter()
            .map(|(e, l)| (l, serde_json::json!(e)))
            .collect(),
        // The tags the engine offers with each texture's own format (srgb only where the target writes it).
        _ => {
            let mut v: Vec<(String, serde_json::Value)> = vec![];
            for k in &keys {
                let p = m.item(k).unwrap().job.as_ref().unwrap().profile.clone();
                for ((f, t), _) in sdc_gui::model::fields::encoding_options(m.target, &p) {
                    let label = match t {
                        engine::config::ColorTag::Srgb => "_SRGB",
                        engine::config::ColorTag::Unorm => UNORM_LABEL,
                        engine::config::ColorTag::NotApplicable => continue,
                    };
                    let e = (label.to_string(), serde_json::json!(t));
                    if f == p.format && !v.contains(&e) {
                        v.push(e);
                    }
                }
            }
            assert!(!v.is_empty());
            v
        }
    };
    let mut h = narrow_harness(m);
    h.run();
    let combos: Vec<_> = h
        .query_all_by_role(Role::ComboBox)
        .filter(|n| n.accesskit_node().label().as_deref() == Some(row))
        .collect();
    assert_eq!(combos.len(), 1, "{row}: one combo");
    combos[0].scroll_to_me();
    h.run();
    h.get_by_role_and_label(Role::ComboBox, row).click();
    h.run();
    let tail = format!(" of {selected})");
    let nodes: Vec<_> = h.query_all_by_label_contains(&tail).collect();
    assert!(
        nodes.iter().all(|n| n.rect().max.x <= 1100.0),
        "{row}: an entry runs past the window"
    );
    let mut shown: Vec<(String, bool)> = nodes
        .iter()
        .map(|n| {
            (
                n.accesskit_node().label().unwrap().to_string(),
                n.accesskit_node().is_disabled(),
            )
        })
        .collect();
    let mut want: Vec<(String, bool)> = options
        .iter()
        .map(|(label, v)| {
            let k = plan_bulk_edit(h.state(), &keys, field, &BulkOp::Set(v.clone()))
                .iter()
                .filter(|(_, c)| *c == Change::Write)
                .count();
            (format!("{label} ({k}{tail}"), k == 0)
        })
        .collect();
    shown.sort();
    want.sort();
    assert_eq!(shown, want, "{row}");
    shown
}

/// E14: each of the four value-dependent popups lists every option as "{option} ({k} of {n})", enabled exactly when
/// the independent planner says a write reaches some texture.
#[test]
fn the_value_dependent_popups_show_every_option_with_the_planners_reach() {
    for row in ["Filter", "BC7 weighting", "Format", "DDS tag"] {
        popup_entries(row, 4);
    }
}

/// E14: an option no selected texture can take is listed, disabled, with its zero (three normals: only Box, and no
/// Perceptual).
#[test]
fn an_option_nothing_can_take_is_listed_disabled_with_zero() {
    let filter = popup_entries("Filter", 3);
    assert!(filter.contains(&("Box (3 of 3)".to_string(), false)));
    for o in ["Tent", "Mitchell", "Kaiser"] {
        assert!(
            filter.contains(&(format!("{o} (0 of 3)"), true)),
            "{o}: {filter:?}"
        );
    }
    let weighting = popup_entries("BC7 weighting", 3);
    assert!(
        weighting.contains(&("Perceptual (0 of 3)".to_string(), true)),
        "{weighting:?}"
    );
    assert!(
        weighting.contains(&("Alpha ×8 (3 of 3)".to_string(), false)),
        "{weighting:?}"
    );
}

/// E14: a PBR albedo takes _SRGB and a PBR normal UNORM: the tag popup offers both, each with its own reach.
#[test]
fn the_tag_popup_offers_both_tags_where_the_target_writes_srgb() {
    const NORMAL: &str = "landscape/rock01_normal.png";
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.click_item(ROCK, false, false);
    m.click_item(NORMAL, true, false);
    let keys = vec![ROCK.to_string(), NORMAL.to_string()];
    let tag = popup_of("DDS tag", m, keys);
    assert!(tag.iter().any(|(l, _)| l.starts_with("_SRGB (")), "{tag:?}");
    assert!(
        tag.iter()
            .any(|(l, _)| l.starts_with("UNORM (as stored) (")),
        "{tag:?}"
    );
}

/// E14: at 1100 px the longest notes (the BC7 weighting and Format rows name every option) stay inside the window and
/// keep their whole text in their name and on hover.
#[test]
fn a_long_reach_note_stays_inside_the_panel_with_its_text_on_hover() {
    use sdc_gui::model::bulk::bulk_fields;
    let d = tempfile::tempdir().unwrap();
    let (mut m, keys) = reach_fixture(d.path(), 4);
    // The diffuse sets its own weighting, filter and format, so each of the three rows ends in a reset button.
    m.set_field(
        &keys[3],
        sdc_gui::model::fields::WEIGHTING,
        serde_json::json!("alpha2"),
    );
    m.set_field(
        &keys[3],
        sdc_gui::model::fields::FILTER_FIELD.0,
        serde_json::json!("tent"),
    );
    m.set_format(
        &keys[3],
        engine::config::Format::Bc7,
        engine::config::ColorTag::Unorm,
    );
    assert!(m.wait_idle(WAIT));
    let notes: Vec<String> = bulk_fields(&m, &keys)
        .into_iter()
        .filter(|r| ["BC7 weighting", "Format", "Filter"].contains(&r.label))
        .map(|r| r.note())
        .collect();
    assert_eq!(notes.len(), 3);
    let mut h = narrow_harness(m);
    instant_tips(&h.ctx);
    h.run();
    for note in &notes {
        h.get_by_label(note).scroll_to_me();
        h.run();
        let n = h.get_by_label(note);
        let (left, right) = (n.rect().min.x, n.rect().max.x);
        assert!(right <= 1100.0 - 8.0, "{note}: ends at {right}");
        assert!(left < right, "{note}");
        h.get_by_label(note).hover();
        h.run();
        assert!(
            h.query_all_by_label(note).count() >= 2,
            "{note}: the hover repeats it"
        );
    }
    // The reset buttons after the notes stay inside the window too.
    let resets: Vec<f32> = h.query_all_by_label("↺").map(|n| n.rect().max.x).collect();
    assert!(resets.len() >= 3, "{resets:?}");
    assert!(resets.iter().all(|x| *x <= 1100.0 - 8.0), "{resets:?}");
    // The longest of them does not fit its column: it is cut, not drawn at its full width.
    let longest = notes.iter().max_by_key(|n| n.len()).unwrap();
    let r = h.get_by_label(longest).rect();
    let font = eframe::egui::TextStyle::Small.resolve(&h.ctx.global_style());
    let full = h.ctx.fonts_mut(|f| {
        f.layout_no_wrap(longest.clone(), font, eframe::egui::Color32::WHITE)
            .size()
            .x
    });
    assert!(r.width() < full - 1.0, "cut: {} of {full}", r.width());
}

/// E14: the unit of the note's wording: groups by equal reach, two at most, then "others vary".
#[test]
fn a_third_group_of_reaches_reads_others_vary() {
    use sdc_gui::model::bulk::BulkField;
    let mut f = BulkField {
        field: sdc_gui::model::fields::FILTER_FIELD.0,
        label: "Filter",
        value: Some(serde_json::json!("box")),
        applies: 4,
        edited: 0,
        skipped: vec![],
        total: 4,
        formats_differ: false,
        option_reach: ["A", "B", "C", "D"]
            .iter()
            .zip([4, 3, 2, 3])
            .map(|(l, k)| (l.to_string(), serde_json::json!(l), k))
            .collect(),
    };
    assert_eq!(f.note(), "A: 4 of 4 · B, D: 3 of 4 · others vary");
    // The widest group is never the one folded away: groups sort by reach, ties keep option order.
    let mut g = f.clone();
    g.option_reach = ["X", "Y", "Z"]
        .iter()
        .zip([1, 2, 4])
        .map(|(l, k)| (l.to_string(), serde_json::json!(l), k))
        .collect();
    assert_eq!(g.note(), "Z: 4 of 4 · Y: 2 of 4 · others vary");
    // No option reaches any texture: one plain count.
    g.option_reach.iter_mut().for_each(|o| o.2 = 0);
    assert_eq!(g.note(), "applies to 0 of 4");
    f.option_reach.truncate(2);
    f.value = None;
    assert_eq!(f.note(), "— mixed · A: 4 of 4 · B: 3 of 4");
}

/// E14: a pick that reaches some texture dispatches even though the active texture (the normal) already has that
/// value: the diffuse takes it.
#[test]
fn a_pick_dispatches_when_the_active_texture_already_has_the_value() {
    use sdc_gui::model::fields::WEIGHTING;
    const NORMAL: &str = "landscape/rock01_normal.png";
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.set_field(NORMAL, WEIGHTING, serde_json::json!("linear"));
    assert!(m.wait_idle(WAIT));
    m.set_field(ROCK, WEIGHTING, serde_json::json!("alpha2"));
    assert!(m.wait_idle(WAIT));
    m.click_item(ROCK, false, false);
    m.click_item(NORMAL, true, false);
    let mut h = harness(m);
    h.run();
    weighting_combo(&h).scroll_to_me();
    h.run();
    weighting_combo(&h).click();
    h.run();
    let entry = h
        .query_all_by_label_contains("Linear")
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Linear ("))
        })
        .expect("the Linear entry");
    assert_eq!(
        entry.accesskit_node().label().as_deref(),
        Some("Linear (2 of 2)")
    );
    entry.click();
    h.run();
    assert!(h.state_mut().wait_idle(WAIT));
    let m = h.state();
    let job = m.item(ROCK).unwrap().job.clone().unwrap();
    let shown = sdc_gui::model::fields::field_view(m.target, &job, WEIGHTING).unwrap();
    assert_eq!(
        shown.value,
        serde_json::json!("linear"),
        "the diffuse took Linear"
    );
}

/// E14 (I1): while an edit is in flight the reach entries are off, as the old editor's busy gate had them.
#[test]
fn the_reach_entries_are_off_while_an_edit_is_in_flight() {
    const NORMAL: &str = "landscape/rock01_normal.png";
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    decide_all(&mut m);
    m.click_item(ROCK, false, false);
    m.click_item(NORMAL, true, false);
    m.hold_plan_results = true;
    m.set_field(ROCK, "mips.levels", serde_json::json!(3));
    assert!(m.edit_busy(), "the edit's plan is in flight");
    let mut h = harness(m);
    h.run_steps(6);
    weighting_combo(&h).scroll_to_me();
    h.run_steps(4);
    weighting_combo(&h).click();
    h.run_steps(4);
    let entries: Vec<_> = h.query_all_by_label_contains(" of 2)").collect();
    assert!(entries.len() >= 5, "the popup is open: {}", entries.len());
    assert!(
        entries.iter().all(|n| n.accesskit_node().is_disabled()),
        "every entry is off while busy"
    );
}

/// E14 (M5): a texture filling a locked slot is not reached, and the count says so as the planner does.
#[test]
fn a_slot_locked_member_is_left_out_of_the_reach() {
    const NORMAL: &str = "landscape/rock01_normal.png";
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_slot_lock("landscape/rock01", 1, true).unwrap();
    assert!(m.wait_idle(WAIT));
    m.click_item(ROCK, false, false);
    m.click_item(NORMAL, true, false);
    let keys = vec![ROCK.to_string(), NORMAL.to_string()];
    // popup_of compares every entry with `plan_bulk_edit`, which skips the locked slot's texture.
    let filter = popup_of("Filter", m, keys);
    assert!(
        filter.contains(&("Box (1 of 2)".to_string(), false)),
        "{filter:?}"
    );
}

/// `harness` with instant tooltips, so a hover shows its text on the next frame.
fn hover_harness(model: AppModel) -> Harness<'static, AppModel> {
    let h = harness(model);
    instant_tips(&h.ctx);
    h
}

/// The tile of `key`: its exact file name outside the Sets column (a "packed into ..." badge never matches exactly).
fn tile<'a>(h: &'a Harness<'static, AppModel>, key: &str) -> egui_kittest::Node<'a> {
    let name: &'a str = &h.state().items.iter().find(|i| i.key == key).unwrap().name;
    let mut found: Vec<_> = h
        .query_all_by_label(name)
        .filter(|n| !in_sets_column(h, n.rect()))
        .collect();
    assert_eq!(found.len(), 1, "one tile named {name}");
    found.remove(0)
}

/// Pins today's behaviour (AG48/CD14, ruling 3d-R5): Ctrl+A keeps a packed tile. It passes before the hover change.
#[test]
fn ctrl_a_includes_packed_tiles() {
    use sdc_gui::model::items::ItemKind;
    let d = tempfile::tempdir().unwrap();
    let mut h = hover_harness(opened(d.path()));
    h.run();
    let packed = h
        .state()
        .items
        .iter()
        .find(|i| i.kind == ItemKind::Packed)
        .unwrap()
        .key
        .clone();
    tile(&h, &packed).click();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    let selected = h.state().selected_items();
    assert!(selected.contains(&packed) && selected.len() > 1);
    assert!(selected
        .iter()
        .all(|k| h.state().item(k).unwrap().helper.is_none()));
}

#[test]
fn a_packed_tile_says_what_it_is_made_from() {
    use sdc_gui::model::items::ItemKind;
    let d = tempfile::tempdir().unwrap();
    let mut h = hover_harness(opened(d.path()));
    h.run();
    let key = h
        .state()
        .items
        .iter()
        .find(|i| i.kind == ItemKind::Packed)
        .unwrap()
        .key
        .clone();
    tile(&h, &key).hover();
    h.run();
    h.run();
    assert!(h
        .query_by_label_contains(
            "Packed output, made from rock01_ao.png and rock01_roughness.png. Bulk edits change unlocked settings on this output."
        )
        .is_some());
}

/// The drawn helper row named by `name`: the Button outside the Sets column.
fn helper_row_of<'a>(h: &'a Harness<'static, AppModel>, name: &'a str) -> egui_kittest::Node<'a> {
    h.query_all_by_label_contains(name)
        .find(|n| {
            n.accesskit_node().role() == eframe::egui::accesskit::Role::Button
                && n.accesskit_node()
                    .label()
                    .is_some_and(|l| l.starts_with("Helper "))
                && !in_sets_column(h, n.rect())
        })
        .expect("the drawn helper row")
}

/// The target label ("→ ..." on the right) drawn in the helper row of `name`: the hover the pack lines live on.
fn target_label_of<'a>(h: &'a Harness<'static, AppModel>, name: &'a str) -> egui_kittest::Node<'a> {
    let row = helper_row_of(h, name).rect();
    ["→", "used by", "not used"]
        .into_iter()
        .flat_map(|w| h.query_all_by_label_contains(w))
        .find(|n| row.contains(n.rect().center()))
        .unwrap_or_else(|| {
            let all: Vec<_> = h
                .query_all_by_label_contains("")
                .map(|n| (n.accesskit_node().label().unwrap_or_default(), n.rect()))
                .collect();
            panic!("no target label in {row:?}: {all:?}")
        })
}

#[test]
fn a_packed_source_says_what_changing_it_changes() {
    let d = tempfile::tempdir().unwrap();
    let mut h = hover_harness(opened(d.path()));
    h.run();
    let src = "landscape/rock01_roughness.png";
    let helper = h.state().item(src).unwrap();
    assert!(helper.helper.is_some(), "exercise the helper row");
    let name = helper.name.clone();
    h.state_mut().filter.hide_helpers = false;
    h.run();
    helper_row_of(&h, &name).scroll_to_me();
    h.run();
    target_label_of(&h, &name).hover();
    h.run();
    assert!(h
        .query_by_label_contains("changing this file changes landscape/rock01_rmaos.dds.")
        .is_some());
    assert!(h
        .query_by_label_contains(
            "Bulk edits skip this source because it has no settings of its own."
        )
        .is_some());
    // Ruling 3d-T19-1: the row's AccessKit description holds the lines; its name does not.
    let row = helper_row_of(&h, &name);
    let node = row.accesskit_node();
    assert!(node
        .description()
        .is_some_and(|d| d.contains("changing this file changes landscape/rock01_rmaos.dds.")));
    assert!(!node
        .label()
        .unwrap_or_default()
        .contains("changing this file"));
}

/// A source feeding two packs lists both outputs once each; each output lists just its own sources.
#[test]
fn a_shared_source_lists_every_output_it_feeds() {
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    for f in ["a/rock.png", "a/rock_roughness.png", "c/slab.png"] {
        let p = root.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        if f.contains('_') {
            image::ImageBuffer::from_fn(64, 64, |x, y| image::Luma([(x * 3 + y) as u8]))
                .save(p)
                .unwrap();
        } else {
            image::ImageBuffer::from_fn(64, 64, |x, y| {
                image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
            })
            .save(p)
            .unwrap();
        }
    }
    let mut m = opened_as_the_app(root, d.path().join("export"));
    let rough = "a/rock_roughness.png";
    assert_eq!(
        m.drop_choice(rough, "c/slab", DropChoice::Share),
        DropOutcome::Joined
    );
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let tip = |k: &str| sdc_gui::ui::columns::tile_tooltip(&m, m.item(k).unwrap(), false);
    let outs: Vec<String> = ["a/rock_rmaos", "a/slab_rmaos"].map(tip).into();
    // The source is a helper: the hover drawn on its row's target label lists both outputs, once each.
    let mut h = hover_harness(m);
    h.run();
    h.state_mut().filter.hide_helpers = false;
    h.run();
    let name = h.state().item(rough).unwrap().name.clone();
    helper_row_of(&h, &name).scroll_to_me();
    h.run();
    target_label_of(&h, &name).hover();
    h.run();
    h.run();
    let node = h
        .query_by_label_contains("changing this file changes")
        .expect("the helper's hover");
    // A tooltip's text is the Label's value.
    let src = node.accesskit_node().value().unwrap_or_default();
    assert_eq!(
        src.matches("changing this file changes").count(),
        2,
        "{src}"
    );
    assert!(
        src.contains(
            "Packed into pbr/a/rock_rmaos.dds: changing this file changes pbr/a/rock_rmaos.dds."
        ),
        "{src}"
    );
    assert!(
        src.contains("Packed into c/slab_rmaos.dds: changing this file changes c/slab_rmaos.dds."),
        "{src}"
    );
    assert!(src.contains("Bulk edits skip this source"), "{src}");
    for t in outs {
        assert!(
            t.contains("Packed output, made from rock_roughness.png. Bulk edits"),
            "{t}"
        );
    }
}

/// Preservation (passes before and after the hover change, a pin and not a RED): a packed output takes its own
/// unlocked field on its own key; the consumed source with no job is skipped.
#[test]
fn a_packed_output_writes_its_own_mip_levels() {
    use sdc_gui::model::bulk::{plan_bulk_edit, BulkOp, Change, NO_SETTINGS};
    use sdc_gui::model::items::ItemKind;
    use sdc_gui::project::ProjectDoc;
    use serde_json::json;

    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let packed = "landscape/rock01_rmaos";
    let src = "landscape/rock01_roughness.png";
    let out = m.item(packed).unwrap();
    assert_eq!(out.kind, ItemKind::Packed);
    assert!(out.job.is_some() && m.item(src).unwrap().job.is_none());
    let field = "mips.levels";
    let doc = m.project.as_ref().unwrap();
    assert_eq!(doc.file_value(packed, field), None);
    assert_eq!(doc.file_value(src, field), None);
    let keys = [packed.to_owned(), src.to_owned()];
    let op = BulkOp::Set(json!("none"));
    assert_eq!(
        plan_bulk_edit(&m, &keys, field, &op),
        vec![
            (packed.to_owned(), Change::Write),
            (src.to_owned(), Change::Skip(NO_SETTINGS.into()))
        ]
    );
    m.apply_bulk_edit(&keys, field, op);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.save().unwrap();
    let root = m.root.as_ref().unwrap();
    assert!(root.join("sdc.toml").is_file());
    let saved = ProjectDoc::open(root);
    assert_eq!(saved.file_value(packed, field), Some(json!("none")));
    assert_eq!(saved.file_value(src, field), None);
}
