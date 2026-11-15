//! Note JF3: Acknowledge is offered only for a blocking code the engine lets `allow` clear. `FMT-CS-SKIN-LOADER` and a
//! `CFG-UNAVAILABLE` without a fallback sub-type stay blocked whatever `allow` holds (`engine::policy::resolve`), so
//! their question never offers it. A type the mode does not have offers the type picker; a skin map in a format
//! Community Shaders cannot load never asks, because Note JG3 resets that format when the plan settles.
mod support;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::codes::Code;
use engine::config::{Format, Subtype, Target};
use engine::policy::cs_skin_loader_ok;
use image::{ImageBuffer, Rgb};
use sdc_gui::model::items::{Decision, ItemStatus};
use sdc_gui::model::{Answer, AppModel};
use std::path::Path;
use std::time::Duration;

const SKIN: &str = "clutter/body_rfaos.png";
const SKIN_TOML: &str =
    "[files.\"clutter/body_rfaos.png\"]\nsubtype = \"skin_rfaos\"\nformat = \"rgb8\"\n";
const RMAOS: &str = "clutter/cup_rmaos.png";
const RMAOS_TOML: &str = "[files.\"clutter/cup_rmaos.png\"]\nsubtype = \"rmaos\"\n";

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "model did not settle");
}

/// `textures/<key>` (64 px) with `toml` as the project's `sdc.toml`, planned under `target`.
fn one_file(dir: &Path, key: &str, toml: &str, target: Target) -> AppModel {
    let file = dir.join("textures").join(key);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(file)
        .unwrap();
    std::fs::write(dir.join("textures/sdc.toml"), toml).unwrap();
    let mut m = support::opened_as_the_app(dir.join("textures"), dir.join("export"));
    if m.target != target {
        m.set_target(target);
        idle(&mut m);
    }
    assert_eq!(m.target, target);
    m
}

fn decisions(m: &AppModel, key: &str) -> Vec<Decision> {
    m.item(key)
        .unwrap_or_else(|| panic!("no item {key}"))
        .decisions
        .clone()
}

fn blocked_by(m: &AppModel, key: &str, code: Code) {
    let item = m.item(key).unwrap();
    assert_eq!(item.status, ItemStatus::NeedsDecision, "{key}");
    assert!(
        item.warnings
            .iter()
            .any(|w| w.code == code && w.severity == engine::codes::Severity::Blocking),
        "{key}: {:?}",
        item.warnings
    );
}

/// The Problems tab on `m`, as the app draws it.
fn problems_tab(mut m: AppModel) -> Harness<'static, AppModel> {
    m.open_problems(None);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h
}

/// The engine never lets `allow` clear FMT-CS-SKIN-LOADER (the file would load as nothing in CS). The GUI never
/// leaves it as a question: a format the type does not allow is reset when the plan settles (Note JG3), and the
/// format it lands on is one CS loads. So there is nothing to acknowledge and no row.
#[test]
fn a_skin_map_in_a_format_cs_cannot_load_is_reset_and_never_asks() {
    let d = tempfile::tempdir().unwrap();
    let m = one_file(d.path(), SKIN, SKIN_TOML, Target::Cs);
    let item = m.item(SKIN).unwrap();
    assert_eq!(item.status, ItemStatus::Ready, "{:?}", item.warnings);
    assert!(decisions(&m, SKIN).is_empty());
    assert!(
        !item
            .warnings
            .iter()
            .any(|w| w.code == Code::FmtCsSkinLoader),
        "{:?}",
        item.warnings
    );
    let job = item.job.as_ref().unwrap();
    assert_eq!(job.profile.subtype, Subtype::SkinRfaos);
    assert_ne!(job.profile.format, Format::Rgb8);
    assert!(
        cs_skin_loader_ok(job.profile.subtype, job.profile.format),
        "{:?}",
        job.profile.format
    );
}

/// A sub-type with no fallback under the mode stays blocked with `allow` too: the question offers the type picker,
/// not Acknowledge, and another type ends it.
#[test]
fn an_unavailable_type_without_a_fallback_offers_the_type_picker_not_acknowledge() {
    let d = tempfile::tempdir().unwrap();
    let mut m = one_file(d.path(), RMAOS, RMAOS_TOML, Target::Vanilla);
    blocked_by(&m, RMAOS, Code::CfgUnavailable);
    assert_eq!(
        decisions(&m, RMAOS),
        [Decision::Change(Code::CfgUnavailable)]
    );
    m.decide(RMAOS, Answer::Acknowledge(Code::CfgUnavailable));
    idle(&mut m);
    blocked_by(&m, RMAOS, Code::CfgUnavailable);
    m.decide(RMAOS, Answer::Subtype(Subtype::Diffuse));
    idle(&mut m);
    let item = m.item(RMAOS).unwrap();
    assert!(
        !item.warnings.iter().any(|w| w.code == Code::CfgUnavailable),
        "{:?}",
        item.warnings
    );
    assert!(!decisions(&m, RMAOS).contains(&Decision::Change(Code::CfgUnavailable)));
}

/// The same on screen: the Problems row names the two ways out and has **Set type…**, no Acknowledge.
#[test]
fn the_problems_row_of_an_unavailable_type_has_set_type() {
    let d = tempfile::tempdir().unwrap();
    let h = problems_tab(one_file(d.path(), RMAOS, RMAOS_TOML, Target::Vanilla));
    assert_eq!(h.query_all_by_label("Acknowledge").count(), 0);
    h.get_by_label("Set type…");
    assert!(
        h.query_all_by_label_contains("This type isn't available in this mode")
            .next()
            .is_some(),
        "the row names the two ways out"
    );
}

/// Why the reset covers the code on every skin map it is raised for: each pair the GUI's format list holds for one
/// is a pair CS loads, so the reset can never land on a format that raises the code again.
#[test]
fn every_format_the_gui_allows_on_a_skin_map_is_one_cs_loads() {
    for st in [Subtype::SkinRfaos, Subtype::SkinWet, Subtype::SkinDetail] {
        assert!(
            !cs_skin_loader_ok(st, Format::Rgb8),
            "{st:?}: the code is raised"
        );
        let list = sdc_gui::model::format_rule::allowed(st, Target::Cs);
        assert!(!list.is_empty(), "{st:?}");
        assert!(
            list.iter().all(|(f, _)| cs_skin_loader_ok(st, *f)),
            "{st:?}: {list:?}"
        );
    }
}

/// A sub-type with a fallback is resolved as the fallback once acknowledged, so its Acknowledge stays and works.
#[test]
fn an_unavailable_type_with_a_fallback_keeps_its_acknowledge() {
    let d = tempfile::tempdir().unwrap();
    let key = "clutter/cup.png";
    let toml = "[files.\"clutter/cup.png\"]\nsubtype = \"pbr_albedo\"\n";
    let mut m = one_file(d.path(), key, toml, Target::Vanilla);
    blocked_by(&m, key, Code::CfgUnavailable);
    assert_eq!(
        decisions(&m, key),
        [Decision::Acknowledge(Code::CfgUnavailable)]
    );
    m.decide(key, Answer::Acknowledge(Code::CfgUnavailable));
    idle(&mut m);
    let item = m.item(key).unwrap();
    assert_eq!(item.status, ItemStatus::Ready, "{:?}", item.warnings);
    assert_eq!(
        item.job.as_ref().unwrap().profile.subtype,
        Subtype::Diffuse,
        "resolved as the fallback"
    );
}
