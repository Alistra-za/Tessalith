//! Task 6g (Note LR, LR8): the Channel guard row in the Compression group. Four ticks, R, G, B and A, over
//! `encoder.channel_guard.r`, `.g`, `.b` and `alpha.exact_opaque`; the row stands where "Exact opaque alpha" stood. The
//! ids are spelt out so the file compiles before the row exists.
mod support;
use eframe::egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::Format;
use sdc_gui::model::fields::{all_fields, field_help, hidden_reason, FIELDS, WEIGHTING_OPTIONS};
use sdc_gui::model::AppModel;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01.png";
const ROW: &str = "encoder.channel_guard";
const A: &str = "alpha.exact_opaque";
const TICKS: [&str; 4] = [
    "Channel guard R",
    "Channel guard G",
    "Channel guard B",
    "Channel guard A",
];

/// BC7-O29 (c), word for word.
const A_HOVER: &str = "On: fully opaque 4x4 blocks decode as exactly 255 in BC7, and blocks whose alpha is 0 throughout as exactly 0. Where only part of a block is 0 or 255, the encoder keeps it when it can, ahead of R, G and B.";
const RGB_HOVER: &str = "On: where this channel is exactly 0 or exactly 255 across part of a block, BC7 keeps that value wherever the encoder can. With several channels on, A wins, then G, then B, then R.";
const A_LOCK: &str = "this map keeps alpha 0 and 255 exact wherever the encoder can, and opaque alpha exactly 255: PGPatcher and ENB read it (ALPHA-EXACT-OFF-M)";
const ALPHA8_HOVER: &str = "Alpha ×8: favours alpha accuracy over colour; an alternative to BC3 for delicate alpha. On colour it costs about 1 dB of RGB per 2 dB of alpha gained.";

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)));
}

fn ready(dir: &Path) -> AppModel {
    let mut m = opened(dir);
    decide_all(&mut m);
    idle(&mut m);
    m
}

fn harness(mut m: AppModel, key: &str) -> Harness<'static, AppModel> {
    m.select_item(key);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1400.0, 1000.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    instant_tips(&h.ctx);
    h.run();
    h
}

fn ticks(h: &Harness<'static, AppModel>, name: &str) -> usize {
    h.query_all_by_role_and_label(Role::CheckBox, name).count()
}

fn own(m: &AppModel, field: &str) -> Option<Value> {
    m.project.as_ref().unwrap().file_value(ROCK, field)
}

/// Clicks the tick, then lets the plan settle as the app does.
fn click(h: &mut Harness<'static, AppModel>, name: &str) {
    h.get_by_role_and_label(Role::CheckBox, name).click();
    h.step();
    idle(h.state_mut());
    h.run();
}

#[test]
fn a_bc7_texture_shows_four_named_ticks() {
    let d = tempfile::tempdir().unwrap();
    let m = ready(d.path());
    let p = &m.item(ROCK).unwrap().job.as_ref().unwrap().profile;
    assert_eq!(p.format, Format::Bc7, "fixture: a BC7 colour map");
    let h = harness(m, ROCK);
    for name in TICKS {
        assert_eq!(ticks(&h, name), 1, "{name}");
    }
    // The visible text is the letter alone.
    for (name, letter) in TICKS.iter().zip(["R", "G", "B", "A"]) {
        let r = h.get_by_role_and_label(Role::CheckBox, name).rect();
        assert_eq!(painted_in(&h, r).trim(), letter, "{name}");
    }
}

#[test]
fn ticking_g_writes_the_texture_own_field() {
    let d = tempfile::tempdir().unwrap();
    let m = ready(d.path());
    let mut h = harness(m, ROCK);
    assert_eq!(own(h.state(), "encoder.channel_guard.g"), None);
    click(&mut h, "Channel guard G");
    let m = h.state();
    assert_eq!(own(m, "encoder.channel_guard.g"), Some(json!(true)));
    for other in ["encoder.channel_guard.r", "encoder.channel_guard.b", A] {
        assert_eq!(own(m, other), None, "{other}");
    }
    let job = m.item(ROCK).unwrap().job.as_ref().unwrap();
    assert!(job.profile.encoder.channel_guard.g);
    assert!(h
        .get_by_role_and_label(Role::CheckBox, "Channel guard G")
        .accesskit_node()
        .toggled()
        .is_some_and(|t| t == eframe::egui::accesskit::Toggled::True));
}

#[test]
fn the_a_tick_writes_exact_opaque_and_removes_a_hand_written_a() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ready(d.path());
    // A hand-written `encoder.channel_guard.a` in the same entry: the load moves it to `alpha.exact_opaque`, and a
    // second key that disagrees would be refused, so the tick takes it away in the same step.
    m.project
        .as_mut()
        .unwrap()
        .set_file_value(ROCK, "encoder.channel_guard.a", &json!(false));
    idle(&mut m);
    let mut h = harness(m, ROCK);
    click(&mut h, "Channel guard A");
    let m = h.state();
    assert_eq!(own(m, A), Some(json!(true)));
    assert_eq!(own(m, "encoder.channel_guard.a"), None);
    let job = m.item(ROCK).unwrap().job.as_ref().unwrap();
    assert!(job.profile.alpha.exact_opaque, "the plan took the A");
    assert!(
        !job.warnings
            .iter()
            .any(|w| w.code == engine::codes::Code::CfgInvalid),
        "no clash of the two keys"
    );
}

#[test]
fn the_row_reset_clears_the_four_fields_and_the_hand_written_a() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ready(d.path());
    for f in [
        "encoder.channel_guard.r",
        "encoder.channel_guard.g",
        "encoder.channel_guard.b",
        A,
    ] {
        m.set_field(ROCK, f, json!(true));
        idle(&mut m);
    }
    m.project
        .as_mut()
        .unwrap()
        .set_file_value(ROCK, "encoder.channel_guard.a", &json!(true));
    m.clear_field(ROCK, ROW);
    idle(&mut m);
    for f in [
        "encoder.channel_guard.r",
        "encoder.channel_guard.g",
        "encoder.channel_guard.b",
        "encoder.channel_guard.a",
        A,
    ] {
        assert_eq!(own(&m, f), None, "{f}");
    }
}

#[test]
fn a_complex_material_keeps_its_a_on_and_says_why() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ready(d.path());
    m.set_field(ROCK, "subtype", json!("complex_material"));
    idle(&mut m);
    let p = m.item(ROCK).unwrap().job.as_ref().unwrap().profile.clone();
    assert_eq!(p.format, Format::Bc7, "fixture: a BC7 complex material");
    assert!(p.alpha.exact_opaque, "fixture: its preset turns A on");
    let mut h = harness(m, ROCK);
    let a = h.get_by_role_and_label(Role::CheckBox, "Channel guard A");
    assert!(a.accesskit_node().is_disabled(), "A is locked while on");
    a.hover();
    h.run();
    assert!(h.query_by_label(A_LOCK).is_some(), "the reason on hover");
    // R, G and B stay editable.
    for name in &TICKS[..3] {
        let n = h.get_by_role_and_label(Role::CheckBox, name);
        assert!(!n.accesskit_node().is_disabled(), "{name}");
    }
}

/// The lock holds an A that is on; off, the tick stays enabled.
#[test]
fn a_complex_material_with_a_off_can_turn_it_on() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ready(d.path());
    m.set_field(ROCK, "subtype", json!("complex_material"));
    idle(&mut m);
    m.set_field(ROCK, A, json!(false));
    idle(&mut m);
    let p = m.item(ROCK).unwrap().job.as_ref().unwrap().profile.clone();
    assert!(!p.alpha.exact_opaque, "fixture: A is off");
    let h = harness(m, ROCK);
    let a = h.get_by_role_and_label(Role::CheckBox, "Channel guard A");
    assert!(!a.accesskit_node().is_disabled(), "A off can be turned on");
}

#[test]
fn the_row_is_absent_off_bc7() {
    let d = tempfile::tempdir().unwrap();
    let mut m = ready(d.path());
    m.set_field(ROCK, "subtype", json!("height"));
    idle(&mut m);
    let p = m.item(ROCK).unwrap().job.as_ref().unwrap().profile.clone();
    assert_eq!(p.format, Format::Bc4, "fixture: a BC4 map");
    assert_eq!(hidden_reason(&p, ROW), Some("BC7 only"));
    let h = harness(m, ROCK);
    for name in TICKS {
        assert_eq!(ticks(&h, name), 0, "{name}");
    }
    assert!(h.query_by_label("Channel guard").is_none());
}

#[test]
fn exact_opaque_alpha_exists_nowhere() {
    let d = tempfile::tempdir().unwrap();
    let m = ready(d.path());
    assert!(all_fields().iter().all(|(_, l)| *l != "Exact opaque alpha"));
    assert!(FIELDS.iter().all(|(_, l)| *l != "Exact opaque alpha"));
    let h = harness(m, ROCK);
    assert!(h.query_by_label_contains("Exact opaque").is_none());
    assert!(h.query_by_label_contains("exact opaque").is_none());
}

#[test]
fn the_tick_hovers_are_the_approved_texts() {
    let d = tempfile::tempdir().unwrap();
    let m = ready(d.path());
    let mut h = harness(m, ROCK);
    for (name, text) in [
        ("Channel guard A", A_HOVER),
        ("Channel guard R", RGB_HOVER),
        ("Channel guard G", RGB_HOVER),
        ("Channel guard B", RGB_HOVER),
    ] {
        h.get_by_role_and_label(Role::CheckBox, name).hover();
        h.run();
        assert!(h.query_by_label(text).is_some(), "{name}: {text}");
    }
}

/// BC7-O30: the row's own hover (its `?` and its label) carries the one-channel assumption, the priority order and
/// the 255 against exact 0 warning.
#[test]
fn the_row_help_carries_the_o30_texts() {
    let help = field_help(ROW).expect("the Channel guard row has help");
    for part in [
        "one guarded channel at a time",
        "A, then G, then B, then R",
        "255",
        "exact 0",
    ] {
        assert!(help.contains(part), "missing {part:?} in {help}");
    }
    // The row's help is also on screen, on the label.
    let d = tempfile::tempdir().unwrap();
    let m = ready(d.path());
    let mut h = harness(m, ROCK);
    h.get_by_label("Channel guard").hover();
    h.run();
    assert!(h.query_by_label(help).is_some());
}

#[test]
fn alpha_x8_names_its_cost_in_two_db() {
    let o = WEIGHTING_OPTIONS.iter().find(|o| o.0 == "alpha8").unwrap();
    assert_eq!(o.2, ALPHA8_HOVER);
}
