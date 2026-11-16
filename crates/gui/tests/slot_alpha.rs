//! Note JI5: the diffuse slot row's alpha type is a dropdown (the texture panel's alpha meanings) with a Confirm
//! button while the type is only the engine's proposal; the Opaque checkbox is gone.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::code_text::alpha_label;
use sdc_gui::model::AppModel;
use serde_json::json;
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01.png";
const LEAF: &str = "landscape/leaf.png";

fn harness(m: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 1200.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h
}

fn select(m: &mut AppModel, id_end: &str) {
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with(id_end))
        .unwrap_or_else(|| panic!("a set ending {id_end}"))
        .key
        .clone();
    m.select_set(&key);
}

/// PBR set selected, rock01's diffuse decided (no alpha question).
fn rock(dir: &std::path::Path) -> AppModel {
    let mut m = pbr_opened_plain(dir);
    select(&mut m, "landscape/rock01");
    m
}

/// leaf.png decided Diffuse with no alpha answer: it proposes one, and its set is selected.
fn leaf(dir: &std::path::Path) -> AppModel {
    let mut m = opened(dir);
    m.decide(
        LEAF,
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(sdc_gui::model::items::alpha_asked(m.item(LEAF).unwrap()).is_some());
    select(&mut m, "landscape/leaf");
    m
}

fn slot_combo<'a>(h: &'a Harness<'static, AppModel>) -> egui_kittest::Node<'a> {
    let mut all: Vec<_> = h
        .query_all_by_role(Role::ComboBox)
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Slot alpha type: "))
        })
        .collect();
    assert_eq!(all.len(), 1, "one slot alpha combo");
    all.remove(0)
}

fn own(m: &AppModel, key: &str) -> Option<serde_json::Value> {
    m.project.as_ref().unwrap().file_value(key, "alpha.policy")
}

#[test]
fn the_slot_row_shows_the_resolved_alpha() {
    let d = tempfile::tempdir().unwrap();
    let m = rock(d.path());
    let want = alpha_label(
        m.item(ROCK)
            .unwrap()
            .job
            .as_ref()
            .unwrap()
            .profile
            .alpha
            .policy,
    );
    let mut h = harness(m);
    h.run();
    let name = slot_combo(&h).accesskit_node().label().unwrap();
    assert_eq!(name, format!("Slot alpha type: {want}"));
    assert_eq!(own(h.state(), ROCK), None);
    assert!(h.query_by_label("Opaque").is_none(), "the checkbox is gone");
}

#[test]
fn a_slot_pick_writes_the_policy_in_one_step() {
    let d = tempfile::tempdir().unwrap();
    let m = rock(d.path());
    let shown = alpha_label(
        m.item(ROCK)
            .unwrap()
            .job
            .as_ref()
            .unwrap()
            .profile
            .alpha
            .policy,
    );
    // Pick an entry other than the shown one.
    let entry = if shown == "Remove" {
        "Cutout"
    } else {
        "Remove"
    };
    let want = if entry == "Remove" {
        "remove"
    } else {
        "cutout"
    };
    let mut h = harness(m);
    h.run();
    h.get_by_label(&format!("Slot alpha type: {shown}")).click();
    h.run();
    h.get_by_label(entry).click();
    h.run();
    assert_eq!(own(h.state(), ROCK), Some(json!(want)));
    assert_eq!(h.state().undo_label(), Some("Alpha"));
    h.state_mut().undo();
    assert_eq!(own(h.state(), ROCK), None);
}

#[test]
fn picking_the_shown_alpha_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let m = rock(d.path());
    let shown = alpha_label(
        m.item(ROCK)
            .unwrap()
            .job
            .as_ref()
            .unwrap()
            .profile
            .alpha
            .policy,
    );
    let mut h = harness(m);
    h.run();
    let pushes = h.state().history.pushes;
    h.get_by_label(&format!("Slot alpha type: {shown}")).click();
    h.run();
    h.get_by_label(shown).click();
    h.run();
    assert_eq!(own(h.state(), ROCK), None);
    assert_eq!(h.state().history.pushes, pushes);
}

#[test]
fn confirm_shows_only_while_undecided() {
    let d = tempfile::tempdir().unwrap();
    let m = leaf(d.path());
    let proposal = sdc_gui::model::items::alpha_asked(m.item(LEAF).unwrap()).unwrap();
    let mut h = harness(m);
    h.run();
    assert_eq!(
        slot_combo(&h).accesskit_node().label().unwrap(),
        format!("Slot alpha type: {}", alpha_label(proposal))
    );
    h.get_by_label("Slot confirm alpha").click();
    h.run();
    h.state_mut().wait_idle(Duration::from_secs(60));
    h.run_steps(3);
    assert_eq!(own(h.state(), LEAF), Some(json!(proposal)));
    assert_eq!(h.state().undo_label(), Some("Alpha"));
    assert!(h.query_by_label("Slot confirm alpha").is_none());
}

#[test]
fn the_slot_reset_goes_back_to_the_inherited_alpha() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    m.set_alpha(ROCK, engine::config::AlphaPolicy::Blend);
    assert!(m.wait_idle(Duration::from_secs(60)));
    let mut h = harness(m);
    h.run_steps(3);
    assert!(own(h.state(), ROCK).is_some());
    h.get_by_label("Reset slot alpha").click();
    h.run_steps(3);
    assert_eq!(own(h.state(), ROCK), None);
    assert_eq!(h.state().undo_label(), Some("Alpha"));
    assert!(h.query_by_label("Reset slot alpha").is_none());
    // One undo gives the own alpha back.
    h.state_mut().undo();
    assert_eq!(own(h.state(), ROCK), Some(json!("blend")));
}

#[test]
fn the_slot_alpha_waits_for_the_plan() {
    let d = tempfile::tempdir().unwrap();
    let mut m = rock(d.path());
    // A re-plan is running: an edit starts one.
    m.set_alpha(ROCK, engine::config::AlphaPolicy::Blend);
    assert!(m.plan_wait());
    let mut h = harness(m);
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.run_steps(4);
    let c = slot_combo(&h);
    assert!(c.accesskit_node().is_disabled());
    let r = c.rect();
    h.hover_at(r.center());
    h.run_steps(4);
    h.get_by_label("Wait for the plan to finish");
}

/// Natural widths of the dropdown showing the longest label and of Confirm, measured in a bare ui with the app's fonts.
fn natural_widths() -> (f32, f32) {
    let out = std::cell::Cell::new((0.0, 0.0));
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(600.0, 200.0))
        .build_ui(|ui| {
            ui.spacing_mut().combo_width = 0.0;
            ui.horizontal(|ui| {
                let combo = egui::ComboBox::from_id_salt("measure")
                    .selected_text(alpha_label(engine::config::AlphaPolicy::CutoutBlend))
                    .show_ui(ui, |_| {})
                    .response;
                let confirm = ui.button("Confirm");
                out.set((combo.rect.width(), confirm.rect.width()));
            });
        });
    h.run_steps(3);
    out.get()
}

#[test]
fn alpha_w_fits_the_longest_label() {
    let (combo, _) = natural_widths();
    let w = sdc_gui::ui::set_editor::ALPHA_W;
    assert!(
        combo <= w && w <= combo + 2.0,
        "ALPHA_W {w} must show `Cutout + Blend` uncut: measured {combo}"
    );
}

#[test]
fn rows_min_makes_room_for_the_alpha_cells() {
    // Measured in the real row (a set that asks, so Confirm shows): Confirm's width and the gap the row draws with.
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(leaf(d.path()));
    h.run_steps(4);
    let combo = slot_combo(&h).rect();
    let confirm = h.get_by_label("Slot confirm alpha").rect();
    let gap = confirm.min.x - combo.max.x;
    let grown = sdc_gui::ui::set_editor::ALPHA_W + confirm.width() + 2.0 * gap;
    // Before this change the floor was 540 (Note FB2); it grows by the combo, Confirm and the two gaps, rounded up.
    assert_eq!(
        sdc_gui::ui::set_editor::ROWS_MIN,
        (540.0 + grown).ceil(),
        "ROWS_MIN, Confirm {}, gap {gap}, grown {grown}",
        confirm.width()
    );
}

/// The Flags cell's controls share the diffuse row's line with its Format combo (they sat about 18 px lower when the
/// cell centred them in the row's whole height).
#[test]
fn the_slot_alpha_sits_on_the_formats_line() {
    for asks in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let mut h = harness(if asks { leaf(d.path()) } else { rock(d.path()) });
        h.run_steps(4);
        let alpha = slot_combo(&h).rect();
        // The diffuse row's Format combo: the one on the alpha combo's row (it shows a BC format).
        let format = h
            .query_all_by_role(Role::ComboBox)
            .filter(|n| {
                n.accesskit_node()
                    .value()
                    .is_some_and(|v| v.starts_with("BC") || v.starts_with("R8") || v.contains("·"))
            })
            .map(|n| n.rect())
            .min_by(|a, b| {
                (a.center().y - alpha.center().y)
                    .abs()
                    .total_cmp(&(b.center().y - alpha.center().y).abs())
            })
            .expect("the diffuse's format combo");
        assert!(
            (alpha.center().y - format.center().y).abs() <= 2.0,
            "asks {asks}: alpha {alpha:?} vs format {format:?}"
        );
    }
}
