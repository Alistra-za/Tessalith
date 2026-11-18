//! A fuzz map follows the set's shader (plan 2026-11-17-fuzz-follows-shader): what the GUI plans, names and exports
//! for a `_fuzz` or `_sheen` image.
mod support;
use engine::config::Subtype;
use engine::group::{split_vocab, VocabRole};
use engine::policy::AcceptSuggestions;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::{Answer, AppModel};
use sdc_gui::settings::GuiSettings;
use std::path::Path;
use std::time::Duration;
use support::*;

const FUZZ: &str = "m/rock01_fuzz.png";

fn grey(dir: &Path, name: &str, v: u8) {
    ImageBuffer::from_fn(64, 64, |x, _| Luma([v.wrapping_add(x as u8)]))
        .save(dir.join(name))
        .unwrap();
}

fn rgb(dir: &Path, name: &str) {
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(dir.join(name))
        .unwrap();
}

/// `m/` holding rock01.png and `files`: a name with a raw-map term (`engine::group::split_vocab` role
/// `PackingInput`) is grey (value 60), any other name RGB (`rgb`); then `sdc.toml` = `toml` when not empty; opened
/// under `accept`. `hook` runs on the new model before `open_folder`.
fn open_with_hook(
    dir: &Path,
    files: &[&str],
    toml: &str,
    accept: AcceptSuggestions,
    hook: impl FnOnce(&mut AppModel),
) -> AppModel {
    let r = dir.join("m");
    std::fs::create_dir_all(&r).unwrap();
    rgb(&r, "rock01.png");
    for f in files {
        let stem = Path::new(f).file_stem().unwrap().to_string_lossy();
        if matches!(split_vocab(&stem), Some((_, v)) if matches!(v.role, VocabRole::PackingInput(_)))
        {
            grey(&r, f, 60);
        } else {
            rgb(&r, f);
        }
    }
    if !toml.is_empty() {
        std::fs::write(dir.join("sdc.toml"), toml).unwrap();
    }
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = accept;
    hook(&mut m);
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    m
}

fn open_with(dir: &Path, files: &[&str], toml: &str, accept: AcceptSuggestions) -> AppModel {
    open_with_hook(dir, files, toml, accept, |_| {})
}

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

fn job_out(m: &AppModel, key: &str) -> String {
    m.item(key)
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .output
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// The `subtype` value written in `sdc.toml` for `key` (`ProjectDoc::file_value`, as `auto_accept.rs` reads it).
fn subtype(m: &AppModel, key: &str) -> Option<serde_json::Value> {
    m.project.as_ref().unwrap().file_value(key, "subtype")
}

fn set_view(m: &AppModel) -> sdc_gui::model::sets::SetView {
    sdc_gui::model::sets::set_views(m)
        .into_iter()
        .find(|v| v.id == "m/rock01")
        .expect("the rock01 set")
}

/// Fuzz-10, probe P1g: the typed fuzz map writes `rock01_f.dds` with accept off and under High alike.
#[test]
fn a_fuzz_map_typed_pbr_fuzz_writes_base_f_in_both_modes() {
    // Off: written before opening (P1g).
    let d = tempfile::tempdir().unwrap();
    let m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "[files.\"m/rock01_fuzz.png\"]\nsubtype = \"pbr_fuzz\"\n",
        AcceptSuggestions::None,
    );
    assert_eq!(job_out(&m, FUZZ), "rock01_f.dds");
    // High: open first, then decide (P1g's High run: at this task the open records skin_albedo first; from Task 2
    // on the set reads PBR and records pbr_albedo, never pbr_fuzz on the fuzz map (Fuzz-18); the explicit decide
    // writes it here, and the assertion holds either way).
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::High,
    );
    m.decide(FUZZ, Answer::Subtype(Subtype::PbrFuzz));
    idle(&mut m);
    assert_eq!(job_out(&m, FUZZ), "rock01_f.dds");
}

/// Fuzz-10: the PGPatcher JSON follows. A `pbr_fuzz` made from a `_fuzz` fills the Fuzz slot and turns `fuzz_on` on.
#[test]
fn a_pbr_fuzz_fuzz_map_fills_the_fuzz_slot_and_turns_fuzz_on() {
    use sdc_gui::model::auto_flags::auto_flags;
    use sdc_gui::model::sets::{SlotFill, SlotKind};
    let d = tempfile::tempdir().unwrap();
    let m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "[files.\"m/rock01_fuzz.png\"]\nsubtype = \"pbr_fuzz\"\n",
        AcceptSuggestions::None,
    );
    let v = set_view(&m);
    assert!(v.pbr && !v.skin);
    assert_eq!(
        v.slots.get(&SlotKind::Fuzz),
        Some(&SlotFill::Own { item: FUZZ.into() })
    );
    let keys: Vec<_> = auto_flags(v.pbr, &v.slots, &Default::default())
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert!(keys.contains(&"fuzz_on"), "{keys:?}");
}

/// Fuzz-18 and spec §3d in the GUI: an untyped fuzz map read as PBR fills the Fuzz slot, turns `fuzz_on` on, writes
/// `rock01_f.dds`, and High records nothing on it.
#[test]
fn an_untyped_fuzz_map_read_as_pbr_fills_the_fuzz_slot_and_is_not_recorded() {
    use sdc_gui::model::auto_flags::auto_flags;
    use sdc_gui::model::sets::{SlotFill, SlotKind};
    for accept in [AcceptSuggestions::None, AcceptSuggestions::High] {
        let d = tempfile::tempdir().unwrap();
        let m = open_with(
            d.path(),
            &["rock01_roughness.png", "rock01_fuzz.png"],
            "",
            accept,
        );
        let v = set_view(&m);
        assert!(v.pbr && !v.skin, "{accept:?}");
        assert_eq!(
            v.slots.get(&SlotKind::Fuzz),
            Some(&SlotFill::Own { item: FUZZ.into() }),
            "{accept:?}"
        );
        let keys: Vec<_> = auto_flags(v.pbr, &v.slots, &Default::default())
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert!(keys.contains(&"fuzz_on"), "{accept:?}: {keys:?}");
        assert_eq!(job_out(&m, FUZZ), "rock01_f.dds", "{accept:?}");
        assert_eq!(subtype(&m, FUZZ), None, "{accept:?}");
    }
}

/// Review Focus 1 where the user sees it: the GUI names a PBR set's outputs by slot (`output_candidates`).
#[test]
fn a_typed_fuzz_map_beside_an_f_keeps_its_own_name_in_the_gui() {
    use sdc_gui::model::sets::{SlotFill, SlotKind};
    let d = tempfile::tempdir().unwrap();
    let m = open_with(
        d.path(),
        &["rock01_f.png", "rock01_fuzz.png"],
        "[files.\"m/rock01_fuzz.png\"]\nsubtype = \"pbr_fuzz\"\n",
        AcceptSuggestions::None,
    );
    assert_eq!(
        set_view(&m).slots.get(&SlotKind::Fuzz),
        Some(&SlotFill::Own {
            item: "m/rock01_f.png".into()
        })
    );
    assert_eq!(job_out(&m, "m/rock01_f.png"), "rock01_f.dds");
    assert_eq!(job_out(&m, FUZZ), "rock01_fuzz.dds");
    let problems = sdc_gui::model::bottom::problems(&m);
    assert!(
        problems
            .iter()
            .all(|r| r.code != Some(engine::codes::Code::InDuplicate)),
        "{problems:?}"
    );
}
