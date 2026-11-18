//! A fuzz map follows the set's shader (plan 2026-11-17-fuzz-follows-shader): what the GUI plans, names and exports
//! for a `_fuzz` or `_sheen` image.
mod support;
use engine::codes::Code;
use engine::config::{PackRecipe, Subtype};
use engine::group::{split_vocab, VocabRole};
use engine::pack::ChannelSource;
use engine::policy::AcceptSuggestions;
use engine::shader::{Shader, ShaderOrigin, ShaderPick};
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::bottom::{problems, Fix, Level};
use sdc_gui::model::code_text::RowClass;
use sdc_gui::model::{bottom, code_text};
use sdc_gui::model::{Answer, AppModel, FUZZ_SWITCH_HOVER};
use sdc_gui::settings::GuiSettings;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;
use support::*;

const FUZZ: &str = "m/rock01_fuzz.png";
const DIFFUSE: &str = "m/rock01.png";
const RMAOS: &str = "m/rock01_rmaos";
const RFAOS: &str = "m/rock01_rfaos";
const NORMAL: &str = "m/rock01_n.png";

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
    let mut m = open_unsettled(dir, files, toml, accept, hook);
    idle(&mut m);
    m
}

/// `open_with_hook` without waiting for the plan: the open's first plan is still in flight.
fn open_unsettled(
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

/// `job_out` when `key` has a job, else `None`.
fn job_out_or_none(m: &AppModel, key: &str) -> Option<String> {
    m.item(key)
        .and_then(|it| it.job.as_ref())
        .map(|_| job_out(m, key))
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

/// The saved `sdc.toml` text planned by the engine as `sdc convert` would (no set shaders), as `auto_accept.rs`'s.
fn cli_plan(root: &Path, toml: &str) -> engine::api::Plan {
    use engine::api::*;
    use engine::config::Target;
    use engine::events::{new_batch_id, NullSink, Reporter};
    use engine::presets::ConfigFile;
    let mut req = BatchRequest::new(vec![root.to_path_buf()], root.join("../cli"), Target::Cs);
    req.recursive = true;
    req.project_config_inline = Some(ConfigFile::parse(toml, "sdc.toml").unwrap());
    let rep = Reporter::new(std::sync::Arc::new(NullSink), new_batch_id());
    Engine::new()
        .unwrap()
        .plan(req, &rep, &CancelToken::default())
        .unwrap()
}

/// The keys `sdc-project.toml` lists as Tessalith's own PBR writes (`[project] auto_pbr`).
fn auto_pbr(m: &AppModel) -> BTreeSet<String> {
    m.sets_doc
        .as_ref()
        .unwrap()
        .auto_pbr()
        .into_iter()
        .collect()
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

/// Fuzz-18: High, Auto reads PBR (nothing recorded on the fuzz map), then an explicit Glow: the fuzz map is unused.
#[test]
fn high_auto_pbr_then_an_explicit_glow_leaves_the_fuzz_map_unused() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::High,
    );
    assert_eq!(job_out(&m, FUZZ), "rock01_f.dds");
    assert_eq!(subtype(&m, FUZZ), None);
    m.set_shader("m/rock01", Some(Shader::Glow));
    idle(&mut m);
    assert_ne!(job_out_or_none(&m, FUZZ).as_deref(), Some("rock01_f.dds"));
    let it = m.item(FUZZ).unwrap();
    let warned = it
        .warnings
        .iter()
        .chain(it.job.iter().flat_map(|j| j.warnings.iter()))
        .any(|w| w.message.contains("the glow shader reads no fuzz map"));
    assert!(warned, "{:?}", it.warnings);
}

/// Fuzz-18: High, Auto reads PBR, then skin evidence is added (a written `rfaos`): the fuzz map is the G mask.
#[test]
fn high_auto_pbr_then_skin_evidence_makes_the_fuzz_map_the_mask() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::High,
    );
    assert!(m.item(RMAOS).is_some());
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    assert!(m.item(RFAOS).is_some());
    assert_eq!(subtype(&m, FUZZ), None);
    assert_ne!(job_out_or_none(&m, FUZZ).as_deref(), Some("rock01_f.dds"));
    // The point of the test: the fuzz map is the `_rfaos` G mask now.
    let packed = m.item(RFAOS).unwrap().job.as_ref().unwrap();
    let g = &packed.pack.as_ref().unwrap().channels[1];
    assert!(
        matches!(g, ChannelSource::Source { path, .. } if path.ends_with("rock01_fuzz.png")),
        "{g:?}"
    );
    // High recorded `pbr_albedo` on the diffuse at open; a recorded type stays the user's (`set_packing_recipe`
    // re-types no member), so `set_view(&m).skin` is false here and is not asserted.
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("pbr_albedo")));
}

/// Fuzz-2's known limit with Note HH1 (`pack::plan::plan_packing`'s `shortcut:`): the borrower's G PACK-DEFAULTED is a
/// Note that says why the shared fuzz map is not its source (the engine's `data.why`).
#[test]
fn a_borrowers_fuzz_default_says_why_the_shared_map_is_not_read() {
    let why = "the shared rock_fuzz.png is converted by its own set textures/a/rock (pbr shader) and is not read \
               here";
    let w = engine::codes::Warning::new(
        Code::PackDefaulted,
        format!("stone_rfaos.dds G: no fuzz source; neutral default 0; {why}"),
    )
    .with_data(json!({
        "output": "textures/b/stone_rfaos.dds",
        "channel": "G",
        "slot": "fuzz",
        "value": 0.0,
        "why": why,
    }));
    assert_eq!(code_text::row_class(Code::PackDefaulted), RowClass::Note);
    assert_eq!(
        code_text::note_sentence(&w),
        format!("stone_rfaos.dds G: {why}, so the default 0 (no fuzz) is used.")
    );
}

/// Fuzz-25: the unused fuzz map is a Note: an Info row listed under Notes, with no fix and no mark, and it never
/// counts against Ready.
#[test]
fn an_unused_fuzz_map_is_a_note_under_notes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::None,
    );
    m.set_shader("m/rock01", Some(Shader::Glow));
    idle(&mut m);
    assert_eq!(code_text::row_class(Code::PackUnusedInput), RowClass::Note);
    let problems = bottom::problems(&m);
    let rows: Vec<_> = problems
        .iter()
        .filter(|p| p.code == Some(Code::PackUnusedInput))
        .collect();
    assert_eq!(rows.len(), 1, "{problems:?}");
    let p = rows[0];
    assert_eq!(
        p.text,
        "rock01_fuzz.png isn't used: the glow shader reads no fuzz map. Nothing else changes."
    );
    assert_eq!(p.level, Level::Info);
    assert!(p.is_note());
    assert!(p.fix.is_none());
    assert!(!p.in_worklist());
    assert!(!bottom::markable(p.level, p.code));
    let it = m.item(FUZZ).unwrap();
    let w = it
        .warnings
        .iter()
        .find(|w| w.code == Code::PackUnusedInput)
        .expect("the item's PACK-UNUSED-INPUT warning");
    assert!(!bottom::counts_for(&m, &p.at, w));
}

/// Fuzz-9 (as JE2 for Advanced Skin): after an explicit PBR pick the saved text plans the same in `sdc convert`.
#[test]
fn an_explicit_pbr_pick_writes_pbr_fuzz_and_the_cli_matches() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::High,
    );
    assert_eq!(
        set_view(&m).shader.as_ref().unwrap().shader,
        Shader::Glow,
        "the _g picks Glow (Fuzz-1a)"
    );
    assert_eq!(
        subtype(&m, DIFFUSE),
        Some(json!("diffuse")),
        "High recorded the Glow set's types"
    );
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m, FUZZ), Some(json!("pbr_fuzz")));
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("pbr_albedo")));
    assert_eq!(subtype(&m, "m/rock01_g.png"), Some(json!("pbr_emissive")));
    assert_eq!(job_out(&m, FUZZ), "rock01_f.dds");
    m.save().unwrap();
    let text = m.project.as_ref().unwrap().text();
    let p = cli_plan(d.path(), &text);
    let cli = |file: &str| {
        p.jobs
            .iter()
            .find(|j| j.pack.is_none() && j.input.ends_with(file))
            .unwrap()
    };
    assert_eq!(cli("rock01_fuzz.png").profile.subtype, Subtype::PbrFuzz);
    assert_eq!(
        cli("rock01_fuzz.png")
            .output
            .file_name()
            .unwrap()
            .to_string_lossy(),
        "rock01_f.dds"
    );
    assert_eq!(cli("rock01.png").profile.subtype, Subtype::PbrAlbedo);
    assert_eq!(cli("rock01_g.png").profile.subtype, Subtype::PbrEmissive);
    // Back to Auto: the step's own values go; the _g picks Glow again and the fuzz map is unused.
    m.set_shader("m/rock01", None);
    idle(&mut m);
    assert_eq!(subtype(&m, FUZZ), None);
    assert_ne!(subtype(&m, DIFFUSE), Some(json!("pbr_albedo")));
}

/// Fuzz-9 under accept off: only the fuzz map is written; the CLI still plans the members as PBR.
#[test]
fn an_explicit_pbr_pick_with_accept_off_matches_the_cli() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::None,
    );
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m, FUZZ), Some(json!("pbr_fuzz")));
    assert_eq!(subtype(&m, DIFFUSE), None);
    m.save().unwrap();
    let p = cli_plan(d.path(), &m.project.as_ref().unwrap().text());
    let diffuse = p
        .jobs
        .iter()
        .find(|j| j.pack.is_none() && j.input.ends_with("rock01.png"))
        .unwrap();
    assert_eq!(diffuse.profile.subtype, Subtype::PbrAlbedo);
}

/// Review Focus 2: beside an `_f`, the explicit PBR pick writes nothing on the fuzz map (Fuzz-4 lists it unused).
#[test]
fn an_explicit_pbr_pick_writes_no_pbr_fuzz_beside_an_f() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png", "rock01_f.png"],
        "",
        AcceptSuggestions::None,
    );
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m, FUZZ), None);
    assert_eq!(job_out(&m, "m/rock01_f.png"), "rock01_f.dds");
}

/// Fuzz-9: a fuzz map with a hand-set helper role (Fuzz-22's skin evidence) takes no `pbr_fuzz` from the pick.
#[test]
fn an_explicit_pbr_pick_writes_no_pbr_fuzz_on_a_hand_role() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "[files.\"m/rock01_fuzz.png\"]\nhelper.role = \"fuzz\"\n",
        AcceptSuggestions::None,
    );
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m, FUZZ), None);
    assert!(!auto_pbr(&m).contains(FUZZ));
}

/// Fuzz-9: leaving PBR clears only a value still equal to the step's write. One changed in `sdc.toml` by hand (the
/// key is still in `auto_pbr`, as no GUI edit dropped it) stays. Under High, so the diffuse's `pbr_albedo` keeps the
/// leave step open (fix round 1: it runs only in a set the step wrote).
#[test]
fn leaving_pbr_keeps_a_fuzz_type_changed_outside_the_gui() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::High,
    );
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("pbr_albedo")));
    m.save().unwrap();
    let path = d.path().join("sdc.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let edited = text.replacen("\"pbr_fuzz\"", "\"generic_data\"", 1);
    assert_ne!(edited, text);
    std::fs::write(&path, edited).unwrap();
    m.reload_documents();
    idle(&mut m);
    assert_eq!(subtype(&m, FUZZ), Some(json!("generic_data")));
    assert!(auto_pbr(&m).contains(FUZZ));
    m.set_shader("m/rock01", None);
    idle(&mut m);
    assert_eq!(subtype(&m, FUZZ), Some(json!("generic_data")));
    assert_ne!(
        subtype(&m, DIFFUSE),
        Some(json!("pbr_albedo")),
        "the step's own value still goes"
    );
}

/// The engine's plan of `key` in the GUI (`None` without a job).
fn gui_subtype(m: &AppModel, key: &str) -> Option<Subtype> {
    m.item(key)
        .and_then(|i| i.job.as_ref())
        .map(|j| j.profile.subtype)
}

/// The saved-text plan's sub-type for the unpacked job of `file`.
fn cli_subtype(p: &engine::api::Plan, file: &str) -> Option<Subtype> {
    p.jobs
        .iter()
        .find(|j| j.pack.is_none() && j.input.ends_with(file))
        .map(|j| j.profile.subtype)
}

/// Fuzz-9 with DU7 (review t6 I1): Advanced Skin then PBR under High. Leaving Advanced Skin writes back the recorded
/// `diffuse`; the explicit PBR pick maps it to `pbr_albedo` as the step's own, so the GUI and `sdc convert` plan
/// an albedo, and back on Auto it goes.
#[test]
fn advanced_skin_then_pbr_gives_the_diffuse_pbr_albedo() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::High,
    );
    m.set_shader("m/rock01", Some(Shader::AdvancedSkin));
    idle(&mut m);
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("skin_albedo")));
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("pbr_albedo")));
    assert!(auto_pbr(&m).contains(DIFFUSE));
    assert_eq!(gui_subtype(&m, DIFFUSE), Some(Subtype::PbrAlbedo));
    m.save().unwrap();
    let p = cli_plan(d.path(), &m.project.as_ref().unwrap().text());
    assert_eq!(cli_subtype(&p, "rock01.png"), Some(Subtype::PbrAlbedo));
    m.set_shader("m/rock01", None);
    idle(&mut m);
    assert_ne!(subtype(&m, DIFFUSE), Some(json!("pbr_albedo")));
    assert!(!auto_pbr(&m).contains(DIFFUSE));
}

/// Review t6 I2: the step's values are reverted on leaving PBR after the fuzz map left the set (Auto), and an
/// explicit Glow then plans no PBR from the saved text.
#[test]
fn leaving_pbr_after_the_fuzz_map_left_reverts_the_step() {
    for leave in [None, Some(Shader::Glow)] {
        let d = tempfile::tempdir().unwrap();
        let mut m = open_with(
            d.path(),
            &["rock01_g.png", "rock01_fuzz.png"],
            "",
            AcceptSuggestions::High,
        );
        m.set_shader("m/rock01", Some(Shader::Pbr));
        idle(&mut m);
        assert_eq!(subtype(&m, DIFFUSE), Some(json!("pbr_albedo")));
        m.remove_member("m/rock01", FUZZ).unwrap();
        idle(&mut m);
        assert!(!set_view(&m).members.iter().any(|k| k == FUZZ));
        m.set_shader("m/rock01", leave);
        idle(&mut m);
        assert_ne!(subtype(&m, DIFFUSE), Some(json!("pbr_albedo")), "{leave:?}");
        assert_ne!(
            subtype(&m, "m/rock01_g.png"),
            Some(json!("pbr_emissive")),
            "{leave:?}"
        );
        assert!(!auto_pbr(&m).contains(DIFFUSE), "{leave:?}");
        if leave.is_some() {
            m.save().unwrap();
            let p = cli_plan(d.path(), &m.project.as_ref().unwrap().text());
            assert_eq!(cli_subtype(&p, "rock01.png"), Some(Subtype::Diffuse));
            assert_eq!(cli_subtype(&p, "rock01_g.png"), Some(Subtype::Glow));
        }
    }
}

/// Ruling R1 (review t6): a member in a locked slot keeps its type under the explicit PBR pick (as DW1 and
/// `pbr_follow` leave it); the unpinned members still take theirs.
#[test]
fn an_explicit_pbr_pick_leaves_a_pinned_diffuse_alone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::High,
    );
    m.set_slot_lock("m/rock01", 1, true).unwrap();
    idle(&mut m);
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("diffuse")));
    assert!(!auto_pbr(&m).contains(DIFFUSE));
    assert_eq!(subtype(&m, "m/rock01_g.png"), Some(json!("pbr_emissive")));
    assert_eq!(subtype(&m, FUZZ), Some(json!("pbr_fuzz")));
    m.save().unwrap();
    let p = cli_plan(d.path(), &m.project.as_ref().unwrap().text());
    // The pinned `diffuse` is the user's choice (as DW1 has it for skin): both sides plan it `diffuse`.
    assert_eq!(gui_subtype(&m, DIFFUSE), Some(Subtype::Diffuse));
    assert_eq!(cli_subtype(&p, "rock01.png"), Some(Subtype::Diffuse));
    assert_eq!(cli_subtype(&p, "rock01_fuzz.png"), Some(Subtype::PbrFuzz));
}

/// Re-review t6-r1 N1 (a): a `diffuse` the user wrote is theirs. Advanced Skin then PBR then Auto keeps it under
/// accept off. Under High a hand `diffuse` equal to the recorder's value is the recorder's (DU8): the PBR pick maps
/// it to `pbr_albedo` as the step's own, leaving PBR clears it, and the next settled plan records `diffuse` again,
/// so the written value ends as `diffuse` either way.
#[test]
fn a_hand_diffuse_survives_advanced_skin_then_pbr_then_auto() {
    for (row, accept, under_pbr) in [
        (
            "accept off: the user's diffuse",
            AcceptSuggestions::None,
            "diffuse",
        ),
        (
            "High: a hand diffuse equal to the recorder's value (DU8)",
            AcceptSuggestions::High,
            "pbr_albedo",
        ),
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut m = open_with(
            d.path(),
            &["rock01_g.png", "rock01_fuzz.png"],
            "[files.\"m/rock01.png\"]\nsubtype = \"diffuse\"\n",
            accept,
        );
        m.set_shader("m/rock01", Some(Shader::AdvancedSkin));
        idle(&mut m);
        assert_eq!(subtype(&m, DIFFUSE), Some(json!("skin_albedo")), "{row}");
        m.set_shader("m/rock01", Some(Shader::Pbr));
        idle(&mut m);
        assert_eq!(subtype(&m, DIFFUSE), Some(json!(under_pbr)), "{row}");
        assert_eq!(
            auto_pbr(&m).contains(DIFFUSE),
            under_pbr == "pbr_albedo",
            "{row}"
        );
        m.set_shader("m/rock01", None);
        idle(&mut m);
        assert_eq!(subtype(&m, DIFFUSE), Some(json!("diffuse")), "{row}");
        assert!(!auto_pbr(&m).contains(DIFFUSE), "{row}");
    }
}

/// Re-review t6-r1 N1 (b): Advanced Skin, PBR, Advanced Skin again under accept off: the saved text holds the skin
/// evidence the GUI plans (`skin_albedo`), so `sdc convert` plans the set skin too.
#[test]
fn advanced_skin_then_pbr_then_advanced_skin_keeps_skin_evidence_with_accept_off() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::None,
    );
    for shader in [Shader::AdvancedSkin, Shader::Pbr, Shader::AdvancedSkin] {
        m.set_shader("m/rock01", Some(shader));
        idle(&mut m);
    }
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("skin_albedo")));
    assert_eq!(gui_subtype(&m, DIFFUSE), Some(Subtype::SkinAlbedo));
    m.save().unwrap();
    let p = cli_plan(d.path(), &m.project.as_ref().unwrap().text());
    assert_eq!(cli_subtype(&p, "rock01.png"), Some(Subtype::SkinAlbedo));
}

/// Re-review t6-r1 N2: a pick that wrote only `pbr_emissive` (the diffuse pinned, R1; the fuzz map a kept DDS,
/// Fuzz-28) is still reverted on leaving PBR, for Auto and for an explicit Glow.
#[test]
fn leaving_pbr_reverts_a_pick_that_wrote_only_emissive() {
    for leave in [None, Some(Shader::Glow)] {
        let d = tempfile::tempdir().unwrap();
        const KEPT: &str = "m/rock01_fuzz.dds";
        let mut m = open_with_hook(
            d.path(),
            &["rock01_g.png"],
            "",
            AcceptSuggestions::High,
            |_| bc1_dds(&d.path().join(KEPT)),
        );
        m.set_slot_lock("m/rock01", 1, true).unwrap();
        idle(&mut m);
        m.set_shader("m/rock01", Some(Shader::Pbr));
        idle(&mut m);
        assert_eq!(subtype(&m, "m/rock01_g.png"), Some(json!("pbr_emissive")));
        assert_eq!(subtype(&m, DIFFUSE), Some(json!("diffuse")));
        m.set_shader("m/rock01", leave);
        idle(&mut m);
        assert_ne!(
            subtype(&m, "m/rock01_g.png"),
            Some(json!("pbr_emissive")),
            "{leave:?}"
        );
        assert!(!auto_pbr(&m).contains("m/rock01_g.png"), "{leave:?}");
    }
}

/// Review t6 V1 (measured): an explicit Advanced Skin on a set with a `_g`, a fuzz map and no raw maps, directly or
/// after PBR, and with no fuzz map. The GUI and the saved text agree: shader Advanced Skin (the GUI by the pick, the
/// CLI by the written `skin_albedo`), diffuse `skin_albedo`, no packed output, and the fuzz map a waiting packing
/// input that is not converted (the GUI shows it skipped: nothing to pack).
#[test]
fn an_explicit_advanced_skin_without_raw_maps_plans_the_same_in_gui_and_cli() {
    use engine::policy::JobStatus;
    use sdc_gui::model::items::ItemStatus;
    for (files, accept, via_pbr) in [
        (
            &["rock01_g.png", "rock01_fuzz.png"][..],
            AcceptSuggestions::None,
            false,
        ),
        (
            &["rock01_g.png", "rock01_fuzz.png"][..],
            AcceptSuggestions::High,
            true,
        ),
        (&["rock01_g.png"][..], AcceptSuggestions::None, false),
    ] {
        let row = format!("{files:?} {accept:?} via PBR {via_pbr}");
        let d = tempfile::tempdir().unwrap();
        let mut m = open_with(d.path(), files, "", accept);
        if via_pbr {
            m.set_shader("m/rock01", Some(Shader::Pbr));
            idle(&mut m);
        }
        m.set_shader("m/rock01", Some(Shader::AdvancedSkin));
        idle(&mut m);
        m.save().unwrap();
        let p = cli_plan(d.path(), &m.project.as_ref().unwrap().text());
        // Shader.
        assert_eq!(
            set_view(&m).shader.map(|s| s.shader),
            Some(Shader::AdvancedSkin),
            "{row}"
        );
        assert!(
            p.shaders.iter().any(|(k, _)| k.ends_with("m/rock01")),
            "{row}: {:?}",
            p.shaders
        );
        assert!(
            p.shaders.values().all(|s| s.shader == Shader::AdvancedSkin),
            "{row}: {:?}",
            p.shaders
        );
        // Diffuse.
        assert_eq!(gui_subtype(&m, DIFFUSE), Some(Subtype::SkinAlbedo), "{row}");
        assert_eq!(
            cli_subtype(&p, "rock01.png"),
            Some(Subtype::SkinAlbedo),
            "{row}"
        );
        // Packed outputs: none on either side.
        assert!(m.item(RFAOS).is_none() && m.item(RMAOS).is_none(), "{row}");
        assert!(p.jobs.iter().all(|j| j.pack.is_none()), "{row}");
        // The fuzz map: never converted.
        if files.contains(&"rock01_fuzz.png") {
            let it = m.item(FUZZ).unwrap();
            assert!(it.job.is_none(), "{row}");
            assert!(
                matches!(&it.status, ItemStatus::Skipped(why) if why.contains("nothing to pack")),
                "{row}: {:?}",
                it.status
            );
            let j = p
                .jobs
                .iter()
                .find(|j| j.input.ends_with("rock01_fuzz.png"))
                .unwrap();
            assert!(
                matches!(j.status, JobStatus::PackingInput { .. }),
                "{row}: {:?}",
                j.status
            );
        }
    }
}

/// Fuzz-9 with DU6-DU7: from an explicit PBR pick to Advanced Skin, the diffuse the step typed `pbr_albedo` is
/// cleared by the step, so Advanced Skin remembers no `pbr_albedo` to write back; back on Auto the Glow set's
/// diffuse is `diffuse` again.
#[test]
fn pbr_then_advanced_skin_then_auto_leaves_no_pbr_albedo() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::High,
    );
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("pbr_albedo")));
    m.set_shader("m/rock01", Some(Shader::AdvancedSkin));
    idle(&mut m);
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("skin_albedo")));
    assert_eq!(subtype(&m, FUZZ), None);
    m.set_shader("m/rock01", None);
    idle(&mut m);
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("diffuse")));
}

/// A sub-type the user picked for the fuzz map is theirs: leaving PBR keeps it. The user path is the type picker
/// (`set_field(key, "subtype", v)` → `edit_subtype` → `edit_subtype_alone`, which drops the key from `auto_pbr`).
/// Two real writes, so the second is not a same-value no-op.
#[test]
fn a_hand_picked_pbr_fuzz_survives_leaving_pbr() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::None,
    );
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert!(
        auto_pbr(&m).contains(FUZZ),
        "the pick's own write is recorded"
    );
    m.set_field(FUZZ, "subtype", json!("generic_data"));
    idle(&mut m);
    m.set_field(FUZZ, "subtype", json!("pbr_fuzz"));
    idle(&mut m);
    assert!(
        !auto_pbr(&m).contains(FUZZ),
        "the user's edit made it theirs"
    );
    m.set_shader("m/rock01", None);
    idle(&mut m);
    assert_eq!(subtype(&m, FUZZ), Some(json!("pbr_fuzz")));
}

/// Known limit (Fuzz-24), pinned so it shows: an explicit Advanced Skin on a set whose diffuse holds a user-written
/// type DU6 leaves alone writes nothing. The GUI plans the set skin (`chosen_skin`); `sdc convert` on the saved text
/// sees no skin evidence and plans it PBR with `rock01_f.dds`. Upgrade with Fuzz-11 (an explicit pick in `sdc.toml`).
#[test]
fn an_explicit_advanced_skin_that_writes_nothing_splits_gui_and_cli() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "[files.\"m/rock01.png\"]\nsubtype = \"pbr_albedo\"\n",
        AcceptSuggestions::None,
    );
    m.set_shader("m/rock01", Some(Shader::AdvancedSkin));
    idle(&mut m);
    assert_eq!(
        subtype(&m, DIFFUSE),
        Some(json!("pbr_albedo")),
        "DU6 leaves a user-written type alone"
    );
    assert!(m.item(RFAOS).is_some(), "the GUI plans the set skin");
    assert_ne!(job_out_or_none(&m, FUZZ).as_deref(), Some("rock01_f.dds"));
    m.save().unwrap();
    let p = cli_plan(d.path(), &m.project.as_ref().unwrap().text());
    let fuzz = p
        .jobs
        .iter()
        .find(|j| j.pack.is_none() && j.input.ends_with("rock01_fuzz.png"))
        .unwrap();
    assert_eq!(
        fuzz.output.file_name().unwrap().to_string_lossy(),
        "rock01_f.dds"
    );
    assert!(p
        .jobs
        .iter()
        .any(|j| j.pack.is_some() && j.output.ends_with("rock01_rmaos.dds")));
}

/// Fuzz-27: in a set not read as PBR (Glow), an explicit PBR pick retypes the untouched recorded `normal` to
/// `pbr_normal`, so B3's SHADER-NORMAL-SPECULAR note does not show; leaving PBR reverts it.
#[test]
fn an_explicit_pbr_pick_retypes_an_untouched_normal_and_the_b3_note_goes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_n.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::High,
    );
    let normal_note = |m: &AppModel| {
        bottom::problems(m)
            .iter()
            .any(|r| r.code == Some(Code::ShaderNormalSpecular))
    };
    assert_eq!(set_view(&m).shader.map(|p| p.shader), Some(Shader::Glow));
    assert_eq!(subtype(&m, NORMAL), Some(json!("normal")));
    assert!(!normal_note(&m));
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m, NORMAL), Some(json!("pbr_normal")));
    assert!(auto_pbr(&m).contains(NORMAL));
    assert!(!normal_note(&m), "{:?}", bottom::problems(&m));
    m.set_shader("m/rock01", Some(Shader::Default));
    idle(&mut m);
    assert_ne!(subtype(&m, NORMAL), Some(json!("pbr_normal")));
    assert!(!auto_pbr(&m).contains(NORMAL));
}

/// Fuzz-28: a kept DDS fuzz map takes no type from the explicit PBR pick (a type cannot change its bytes, as B3-11's
/// `use_pbr_normal_refusal` says), and its key never joins `auto_pbr`.
#[test]
fn an_explicit_pbr_pick_writes_no_type_on_a_kept_dds() {
    let d = tempfile::tempdir().unwrap();
    const KEPT: &str = "m/rock01_fuzz.dds";
    let mut m = open_with_hook(
        d.path(),
        &["rock01_g.png"],
        "",
        AcceptSuggestions::None,
        |_| bc1_dds(&d.path().join(KEPT)),
    );
    assert!(m.item(KEPT).unwrap().kept_dds());
    assert!(set_view(&m).members.iter().any(|k| k == KEPT));
    m.set_shader("m/rock01", Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(subtype(&m, KEPT), None);
    assert!(!auto_pbr(&m).contains(KEPT));
    assert!(matches!(
        set_view(&m).shader,
        Some(ShaderPick {
            shader: Shader::Pbr,
            provenance: ShaderOrigin::Set,
            ..
        })
    ));
}

/// The old High project of Fuzz-8: `files` (greys) beside rock01.png, whose `skin_albedo` an older Tessalith recorded
/// because the set had a fuzz map, then `extra`. The switch on open (Fuzz-8) is off, so these tests read the
/// predicate alone.
fn old_high(dir: &Path, files: &[&str], extra: &str, accept: AcceptSuggestions) -> AppModel {
    let toml = format!("[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n{extra}");
    open_with_hook(dir, files, &toml, accept, |m| m.switch_fuzz_on_open = false)
}

/// Fuzz-5: the recorded `skin_albedo` of an old fuzz-only skin set is untouched, in both accept modes; an explicit
/// PBR pick replaces it in the shader's step.
#[test]
fn a_fuzz_recorded_skin_albedo_is_untouched() {
    for fuzz in ["rock01_fuzz.png", "rock01_sheen.png"] {
        for accept in [AcceptSuggestions::None, AcceptSuggestions::High] {
            let d = tempfile::tempdir().unwrap();
            let mut m = old_high(d.path(), &["rock01_roughness.png", fuzz], "", accept);
            assert!(m.fuzz_recorded_skin(DIFFUSE), "{fuzz} {accept:?}");
            m.set_shader("m/rock01", Some(Shader::Pbr));
            idle(&mut m);
            assert_ne!(
                subtype(&m, DIFFUSE),
                Some(json!("skin_albedo")),
                "{fuzz} {accept:?}"
            );
            assert!(!set_view(&m).skin, "{fuzz} {accept:?}");
            // The GUI and `sdc convert` on the saved text plan the diffuse alike.
            m.save().unwrap();
            let p = cli_plan(d.path(), &m.project.as_ref().unwrap().text());
            assert_eq!(
                gui_subtype(&m, DIFFUSE),
                cli_subtype(&p, "rock01.png"),
                "{fuzz} {accept:?}"
            );
        }
    }
}

/// Fuzz-5: other skin evidence keeps it the user's.
#[test]
fn other_skin_evidence_keeps_the_recorded_skin_albedo() {
    for extra in [
        "[files.\"m/rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
        "[files.\"m/rock01_roughness.png\"]\nsubtype = \"skin_rfaos\"\n",
    ] {
        for fuzz in ["rock01_fuzz.png", "rock01_sheen.png"] {
            let d = tempfile::tempdir().unwrap();
            let mut m = old_high(
                d.path(),
                &["rock01_roughness.png", fuzz],
                extra,
                AcceptSuggestions::None,
            );
            assert!(!m.fuzz_recorded_skin(DIFFUSE), "{fuzz} {extra}");
            // The user's value: an explicit PBR pick leaves it.
            m.set_shader("m/rock01", Some(Shader::Pbr));
            idle(&mut m);
            assert_eq!(
                subtype(&m, DIFFUSE),
                Some(json!("skin_albedo")),
                "{fuzz} {extra}"
            );
        }
    }
    // An explicit Advanced Skin pick keeps the value, in both accept modes. DU6 records nothing here: the diffuse
    // already resolves `SkinAlbedo`, so `skin_follow` returns `None`. The predicate is then false through "shader
    // origin not `Set`".
    for accept in [AcceptSuggestions::None, AcceptSuggestions::High] {
        let d = tempfile::tempdir().unwrap();
        let mut m = old_high(
            d.path(),
            &["rock01_roughness.png", "rock01_fuzz.png"],
            "",
            accept,
        );
        m.set_shader("m/rock01", Some(Shader::AdvancedSkin));
        idle(&mut m);
        assert_eq!(
            subtype(&m, DIFFUSE),
            Some(json!("skin_albedo")),
            "{accept:?}"
        );
        assert!(!m.fuzz_recorded_skin(DIFFUSE), "{accept:?}");
        // Review t7 probe: a presets save re-types no untouched value here.
        m.presets_saved();
        idle(&mut m);
        assert_eq!(
            subtype(&m, DIFFUSE),
            Some(json!("skin_albedo")),
            "{accept:?}"
        );
    }
}

/// Fuzz-5 with Fuzz-22: a hand-set `helper.role = "fuzz"` is skin evidence, on a member or on a file shared in.
#[test]
fn a_hand_set_fuzz_role_keeps_the_recorded_skin_albedo() {
    let d = tempfile::tempdir().unwrap();
    let m = old_high(
        d.path(),
        &[
            "rock01_roughness.png",
            "rock01_fuzz.png",
            "rock01_sheen.png",
        ],
        "[files.\"m/rock01_sheen.png\".helper]\nrole = \"fuzz\"\n",
        AcceptSuggestions::None,
    );
    assert!(!m.fuzz_recorded_skin(DIFFUSE));
    // Shared in (Note HH) from another set.
    let d = tempfile::tempdir().unwrap();
    let mut m = old_high(
        d.path(),
        &[
            "rock01_roughness.png",
            "rock01_fuzz.png",
            "aaa01.png",
            "aaa01_mask.png",
        ],
        "[files.\"m/aaa01_mask.png\".helper]\nrole = \"fuzz\"\n",
        AcceptSuggestions::None,
    );
    assert!(m.fuzz_recorded_skin(DIFFUSE), "before the share");
    use sdc_gui::model::sets::{DropChoice, DropOutcome};
    let out = m.drop_choice("m/aaa01_mask.png", "m/rock01", DropChoice::Share);
    if matches!(out, DropOutcome::AskReplace { .. }) {
        m.drop_replace("m/aaa01_mask.png", "m/rock01", DropChoice::Share)
            .unwrap();
    }
    idle(&mut m);
    assert!(
        set_view(&m)
            .shared_helpers
            .iter()
            .any(|k| k == "m/aaa01_mask.png"),
        "{:?}",
        set_view(&m)
    );
    assert!(!m.fuzz_recorded_skin(DIFFUSE));
}

/// Fuzz-5 (review t7 M1): a written `rfaos` recipe is read from the set's base, not through the Rmaos slot: a real
/// `rock01_rmaos.png` fills that slot.
#[test]
fn a_written_rfaos_recipe_beside_a_real_rmaos_keeps_the_recorded_skin_albedo() {
    let d = tempfile::tempdir().unwrap();
    let m = old_high(
        d.path(),
        &["rock01_rmaos.png", "rock01_fuzz.png"],
        "[files.\"m/rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
        AcceptSuggestions::None,
    );
    assert!(!m.fuzz_recorded_skin(DIFFUSE));
}

/// Fuzz-5 (review t7 M1): in a sub-folder the recipe key is `<folder>/<base>_rfaos`; without one the value is
/// untouched there too.
#[test]
fn the_recipe_key_is_read_in_the_diffuses_folder() {
    for (extra, recorded) in [
        ("", true),
        (
            "[files.\"m/sub/stone_rfaos\".packing]\nrecipe = \"rfaos\"\n",
            false,
        ),
    ] {
        let d = tempfile::tempdir().unwrap();
        let sub = d.path().join("m/sub");
        let toml = format!("[files.\"m/sub/stone.png\"]\nsubtype = \"skin_albedo\"\n{extra}");
        // The switch on open (Fuzz-8) is off, as in `old_high`.
        let m = open_with_hook(d.path(), &[], &toml, AcceptSuggestions::None, |m| {
            m.switch_fuzz_on_open = false;
            std::fs::create_dir_all(&sub).unwrap();
            rgb(&sub, "stone.png");
            grey(&sub, "stone_roughness.png", 60);
            grey(&sub, "stone_fuzz.png", 60);
        });
        assert_eq!(m.fuzz_recorded_skin("m/sub/stone.png"), recorded, "{extra}");
    }
}

/// Fuzz-5 (review t7 M2): a locked Diffuse slot, a real `_rfaos` file and a vanilla target each keep the value the
/// user's.
#[test]
fn a_lock_a_real_rfaos_or_vanilla_keeps_the_recorded_skin_albedo() {
    let d = tempfile::tempdir().unwrap();
    let mut m = old_high(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::None,
    );
    assert!(m.fuzz_recorded_skin(DIFFUSE));
    m.set_slot_lock("m/rock01", 1, true).unwrap();
    idle(&mut m);
    assert!(!m.fuzz_recorded_skin(DIFFUSE), "locked");
    let d = tempfile::tempdir().unwrap();
    let m = old_high(
        d.path(),
        &["rock01_rfaos.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::None,
    );
    assert!(!m.fuzz_recorded_skin(DIFFUSE), "a real _rfaos");
    let d = tempfile::tempdir().unwrap();
    let mut m = old_high(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "",
        AcceptSuggestions::None,
    );
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    assert_eq!(m.target, engine::config::Target::Vanilla);
    assert!(!m.fuzz_recorded_skin(DIFFUSE), "vanilla");
}

/// Fuzz-5 (review t7 M6): the fuzz map is an image, as the engine's `untyped_fuzz_map` requires; a kept
/// `rock01_fuzz.dds` alone is no fuzz map here.
#[test]
fn a_kept_dds_fuzz_map_alone_is_not_a_fuzz_recorded_skin() {
    let d = tempfile::tempdir().unwrap();
    const KEPT: &str = "m/rock01_fuzz.dds";
    let m = open_with_hook(
        d.path(),
        &["rock01_roughness.png"],
        "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n",
        AcceptSuggestions::None,
        |_| bc1_dds(&d.path().join(KEPT)),
    );
    assert!(set_view(&m).members.iter().any(|k| k == KEPT));
    assert!(!m.fuzz_recorded_skin(DIFFUSE));
}

/// Fuzz-30: a set borrowing the diffuse (JN1 share) never decides the lender's file. The borrower `m/aaa01` holds
/// only a fuzz map and sorts before the lender, so a find-first match over the views would meet it first.
#[test]
fn a_borrowers_view_does_not_decide_the_lenders_file() {
    use sdc_gui::model::sets::{DropChoice, DropOutcome, SlotFill, SlotKind};
    let d = tempfile::tempdir().unwrap();
    let mut m = old_high(
        d.path(),
        &["rock01_roughness.png", "aaa01_fuzz.png"],
        "[files.\"m/rock01_roughness.png\"]\nsubtype = \"skin_rfaos\"\n",
        AcceptSuggestions::None,
    );
    let out = m.drop_choice(DIFFUSE, "m/aaa01", DropChoice::Share);
    if matches!(out, DropOutcome::AskReplace { .. }) {
        m.drop_replace(DIFFUSE, "m/aaa01", DropChoice::Share)
            .unwrap();
    }
    idle(&mut m);
    let views = sdc_gui::model::sets::views(&m);
    let at = |id: &str| views.iter().position(|v| v.id == id).unwrap();
    assert!(at("m/aaa01") < at("m/rock01"));
    assert!(matches!(
        views[at("m/aaa01")].slots.get(&SlotKind::Diffuse),
        Some(SlotFill::Borrowed { .. })
    ));
    assert!(!m.fuzz_recorded_skin(DIFFUSE));
}

/// Clears a test file's read-only flag on drop, so a failed assert does not leave it set (as `by_hand_rows.rs`).
struct ReadOnly(std::path::PathBuf);

impl Drop for ReadOnly {
    fn drop(&mut self) {
        if let Ok(m) = std::fs::metadata(&self.0) {
            let mut p = m.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            p.set_readonly(false);
            let _ = std::fs::set_permissions(&self.0, p);
        }
    }
}

/// `open_with_hook` under accept off, whose hook marks `sdc.toml` read-only; the guard clears the flag again.
fn open_with_read_only(dir: &Path, files: &[&str], toml: &str) -> (AppModel, ReadOnly) {
    let file = dir.join("sdc.toml");
    let guard = ReadOnly(file.clone());
    let m = open_with_hook(dir, files, toml, AcceptSuggestions::None, |_| {
        let mut p = std::fs::metadata(&file).unwrap().permissions();
        p.set_readonly(true);
        std::fs::set_permissions(&file, p).unwrap();
    });
    (m, guard)
}

/// The file names of every file under `root`, recursively.
fn exported_names(root: &Path) -> Vec<String> {
    let mut out = vec![];
    let mut dirs = vec![root.to_path_buf()];
    while let Some(d) = dirs.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else {
                out.push(e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    out
}

/// Fuzz-8: an old High fuzz-only project opens with the set switched off Advanced Skin, one undoable step, with a
/// row that names every changed file; Undo restores the file; the switch does not fire again until reopened.
/// Fuzz-18: nothing is recorded on the fuzz map.
#[test]
fn an_old_fuzz_only_skin_set_switches_on_open_and_undo_restores_it() {
    let d = tempfile::tempdir().unwrap();
    let before = "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n";
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        before,
        AcceptSuggestions::High,
    );
    idle(&mut m);
    let v = set_view(&m);
    assert!(v.pbr && !v.skin, "switched to PBR");
    assert_eq!(
        subtype(&m, DIFFUSE),
        Some(json!("pbr_albedo")),
        "High records the new pick in the same step"
    );
    assert_eq!(
        subtype(&m, FUZZ),
        None,
        "Fuzz-18: the fuzz map is never recorded"
    );
    let (text, lines) = m.fuzz_note_for_test().expect("the switch row");
    assert_eq!(
        text,
        "1 set left Advanced Skin (a _fuzz or _sheen map alone no longer picks it)"
    );
    assert!(
        lines.contains(&"rock01.png: skin albedo → pbr albedo".to_string()),
        "{lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.starts_with("rock01_fuzz.png: ")),
        "{lines:?}"
    );
    // The High recording that joined the step lists its line too.
    assert!(
        lines.contains(&"rock01.png: pbr albedo".to_string()),
        "{lines:?}"
    );
    assert_eq!(lines.last().map(String::as_str), Some(FUZZ_SWITCH_HOVER));
    let rows = sdc_gui::model::bottom::problems(&m);
    // The row of its own: its text, and the Undo fix while the step is on top.
    assert!(
        rows.iter()
            .any(|r| r.text == text && r.fix == Some(Fix::UndoLast)),
        "{rows:?}"
    );
    m.undo();
    idle(&mut m);
    assert_eq!(
        m.project.as_ref().unwrap().text(),
        before,
        "one step: Undo restores the file"
    );
    assert!(set_view(&m).skin, "skin again after Undo");
    assert!(
        m.fuzz_note_for_test().is_none(),
        "the undone step's row goes"
    );
    idle(&mut m);
    assert!(set_view(&m).skin, "the switch does not fire again");
}

/// Fuzz-8, the next open: a kept and saved switch leaves nothing to switch; an undone switch saved as skin switches
/// again on the next open (the switch runs once per open).
#[test]
fn the_next_open_switches_an_undone_save_again_and_not_a_kept_one() {
    let before = "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n";
    for undo in [false, true] {
        let d = tempfile::tempdir().unwrap();
        let mut m = open_with(
            d.path(),
            &["rock01_roughness.png", "rock01_fuzz.png"],
            before,
            AcceptSuggestions::None,
        );
        assert!(m.fuzz_note_for_test().is_some(), "undo {undo}");
        if undo {
            m.undo();
            idle(&mut m);
            assert!(set_view(&m).skin, "undo {undo}");
        }
        m.save().unwrap();
        m.open_folder(d.path().to_path_buf());
        idle(&mut m);
        assert_eq!(
            m.fuzz_note_for_test().is_some(),
            undo,
            "undo {undo}: the next open switches only the undone save"
        );
        assert!(!set_view(&m).skin, "undo {undo}");
        assert_eq!(subtype(&m, DIFFUSE), None, "undo {undo}");
    }
}

/// Fuzz-8 with two sets: one step clears both, and one Undo restores both (the plural strings too).
#[test]
fn two_switched_sets_are_one_step_and_one_undo() {
    let d = tempfile::tempdir().unwrap();
    let before = "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n\n[files.\"m/stone01.png\"]\nsubtype = \"skin_albedo\"\n";
    let mut m = open_with(
        d.path(),
        &[
            "rock01_roughness.png",
            "rock01_fuzz.png",
            "stone01.png",
            "stone01_roughness.png",
            "stone01_sheen.png",
        ],
        before,
        AcceptSuggestions::High,
    );
    let skin = |m: &AppModel, id: &str| {
        sdc_gui::model::sets::set_views(m)
            .into_iter()
            .find(|v| v.id == id)
            .unwrap()
            .skin
    };
    assert!(!skin(&m, "m/rock01") && !skin(&m, "m/stone01"));
    let (text, lines) = m.fuzz_note_for_test().expect("the switch row");
    assert_eq!(
        text,
        "2 sets left Advanced Skin (a _fuzz or _sheen map alone no longer picks it)"
    );
    for l in [
        "rock01.png: skin albedo → pbr albedo",
        "stone01.png: skin albedo → pbr albedo",
    ] {
        assert!(lines.contains(&l.to_string()), "{lines:?}");
    }
    m.undo();
    idle(&mut m);
    assert_eq!(m.project.as_ref().unwrap().text(), before);
    assert!(skin(&m, "m/rock01") && skin(&m, "m/stone01"));
}

/// Fuzz-8 under accept off, both terms: the same switch; nothing is recorded, the row still lists the diffuse.
#[test]
fn the_switch_runs_with_accept_off() {
    for fuzz in ["rock01_fuzz.png", "rock01_sheen.png"] {
        let d = tempfile::tempdir().unwrap();
        let mut m = open_with(
            d.path(),
            &["rock01_roughness.png", fuzz],
            "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n",
            AcceptSuggestions::None,
        );
        idle(&mut m);
        assert_eq!(subtype(&m, DIFFUSE), None, "{fuzz}");
        assert_eq!(subtype(&m, &format!("m/{fuzz}")), None, "{fuzz}");
        assert!(!set_view(&m).skin, "{fuzz}");
        let (_, lines) = m.fuzz_note_for_test().unwrap();
        assert!(
            lines.contains(&"rock01.png: skin albedo → pbr albedo".to_string()),
            "{fuzz}: {lines:?}"
        );
    }
}

/// Fuzz-8: sets with other skin evidence stay skin; nothing is written. Both terms.
#[test]
fn sets_with_other_skin_evidence_are_not_switched() {
    for fuzz in ["rock01_fuzz.png", "rock01_sheen.png"] {
        for extra in [
            "[files.\"m/rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
            "[files.\"m/rock01_roughness.png\"]\nsubtype = \"skin_rfaos\"\n",
        ] {
            let d = tempfile::tempdir().unwrap();
            let text = format!("[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n{extra}");
            let mut m = open_with(
                d.path(),
                &["rock01_roughness.png", fuzz],
                &text,
                AcceptSuggestions::None,
            );
            idle(&mut m);
            assert!(set_view(&m).skin, "{fuzz} {extra}");
            assert_eq!(m.project.as_ref().unwrap().text(), text, "{fuzz} {extra}");
        }
    }
    // A set the user made Advanced Skin: its shader is in sdc-project.toml, at the folder root beside sdc.toml.
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("m")).unwrap();
    std::fs::write(
        d.path().join("sdc-project.toml"),
        "[[sets]]\nid = \"m/rock01\"\nshader = \"advanced_skin\"\n",
    )
    .unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n",
        AcceptSuggestions::None,
    );
    idle(&mut m);
    assert!(set_view(&m).skin);
}

/// Review Focus 3: a read-only `sdc.toml` switches nothing and shows no switch row.
#[test]
fn a_read_only_project_switches_nothing() {
    let d = tempfile::tempdir().unwrap();
    let text = "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n";
    let (mut m, _flag) =
        open_with_read_only(d.path(), &["rock01_roughness.png", "rock01_fuzz.png"], text);
    idle(&mut m);
    assert!(set_view(&m).skin);
    assert!(m.fuzz_note_for_test().is_none());
    assert_eq!(
        std::fs::read_to_string(d.path().join("sdc.toml")).unwrap(),
        text
    );
}

/// Review Focus 4: a switched set lands on whatever the other maps pick (Glow here); the row says "left Advanced
/// Skin", and the fuzz map is unused (Fuzz-3).
#[test]
fn a_switched_set_can_land_on_another_shader() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_g.png", "rock01_fuzz.png"],
        "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n",
        AcceptSuggestions::None,
    );
    idle(&mut m);
    assert_eq!(set_view(&m).shader.as_ref().unwrap().shader, Shader::Glow);
    let (_, lines) = m.fuzz_note_for_test().unwrap();
    assert!(
        lines.contains(&"rock01.png: skin albedo → diffuse".to_string()),
        "{lines:?}"
    );
    let it = m.item(FUZZ).unwrap();
    let warned = it
        .warnings
        .iter()
        .chain(it.job.iter().flat_map(|j| j.warnings.iter()))
        .any(|w| w.message.contains("the glow shader reads no fuzz map"));
    assert!(warned, "{:?}", it.warnings);
}

/// Review r2 I4: an export queued behind the first plan after open waits for the switch and runs on the post-switch
/// plan: the export writes `rock01_f.dds` and `_rmaos`, never the skin plan's `_rfaos`. High, so the open records
/// the diffuse's new type in the switch's step and no texture is left undecided.
#[test]
fn an_export_queued_at_open_runs_on_the_switched_plan() {
    let d = tempfile::tempdir().unwrap();
    // FB1: the export folder is the project's own `[project] export_folder`, not `GuiSettings`'. A TOML literal
    // string, because of Windows backslashes.
    let out = d.path().join("export");
    std::fs::write(
        d.path().join("sdc-project.toml"),
        format!("[project]\nexport_folder = '{}'\n", out.display()),
    )
    .unwrap();
    let mut m = open_unsettled(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n",
        AcceptSuggestions::High,
        |_| {},
    );
    m.export_clicked(); // a plan is in flight: `export_go` sets `export_after_plan`
    idle(&mut m);
    if m.review.is_some() {
        // support's `export_through` pattern; Review exports the decided files and skips undecided ones.
        m.export_from_review();
        idle(&mut m);
    }
    assert!(m.fuzz_note_for_test().is_some(), "the switch ran");
    assert_eq!(m.export_root(), out);
    let names = exported_names(&out);
    assert!(
        names.contains(&"rock01_f.dds".to_string()),
        "exported after the switch: {names:?}"
    );
    assert!(names.contains(&"rock01_rmaos.dds".to_string()), "{names:?}");
    assert!(
        !names.contains(&"rock01_rfaos.dds".to_string()),
        "never the pre-switch skin plan: {names:?}"
    );
}

/// Fuzz-26: the switch clears only the Diffuse-slot `skin_albedo`; a `normal` the user wrote stays, and B3 offers
/// **Use PBR normal** on it.
#[test]
fn the_switch_leaves_a_recorded_normal_and_b3_offers_the_pbr_normal() {
    let d = tempfile::tempdir().unwrap();
    let before = "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n[files.\"m/rock01_n.png\"]\nsubtype = \"normal\"\n";
    let mut m = open_with(
        d.path(),
        &["rock01_n.png", "rock01_roughness.png", "rock01_fuzz.png"],
        before,
        AcceptSuggestions::None,
    );
    idle(&mut m);
    assert!(!set_view(&m).skin && set_view(&m).pbr, "switched");
    assert_eq!(subtype(&m, NORMAL), Some(json!("normal")), "not rewritten");
    let (_, lines) = m.fuzz_note_for_test().expect("the switch row");
    assert!(
        !lines.iter().any(|l| l.starts_with("rock01_n.png: ")),
        "{lines:?}"
    );
    let rows = problems(&m);
    assert!(
        rows.iter()
            .any(|r| r.code == Some(Code::ShaderNormalSpecular)
                && r.fix == Some(Fix::UsePbrNormal(NORMAL.into()))),
        "{rows:?}"
    );
}

/// Fuzz-29, a pin: the switch row is in the rows built on the first frame (`poll`, then `problems`) after the poll
/// that built it, with no other change between. It does not fail if `finish_fuzz_switch`'s `invalidate_views()` is
/// removed: even with a scan database (the open's first read goes to a worker), the reads after the switch's plans
/// have the inputs of the last landed read (only the diffuse has a colour mesh key, and it does not move), so Note
/// FL6 lands them at once and the row is built in the poll whose `apply_plan` already invalidated the views.
#[test]
fn the_switch_row_shows_on_the_frame_after_it_is_built() {
    let d = tempfile::tempdir().unwrap();
    let dbdir = tempfile::tempdir().unwrap();
    let db = dbdir.path().join("scan.db");
    drop(modscan::store::Store::open(&db).unwrap());
    assert!(db.is_file());
    let mut m = open_unsettled(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n",
        AcceptSuggestions::None,
        |m| m.scan_db = Some(db.clone()),
    );
    let start = std::time::Instant::now();
    loop {
        m.poll();
        let rows = problems(&m);
        if let Some((text, _)) = m.fuzz_note_for_test() {
            assert!(rows.iter().any(|r| r.text == text), "{rows:?}");
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "no switch row in 60 s"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

const OLD_SKIN: &str = "[files.\"m/rock01.png\"]\nsubtype = \"skin_albedo\"\n";

/// An old fuzz-only skin project, opened and not yet settled.
fn old_unsettled(dir: &Path, accept: AcceptSuggestions) -> AppModel {
    open_unsettled(
        dir,
        &["rock01_roughness.png", "rock01_fuzz.png"],
        OLD_SKIN,
        accept,
        |_| {},
    )
}

/// Poll until the switch's step is pushed and its row is not built yet (the window between the two).
fn until_switch_pending(m: &mut AppModel) {
    let start = std::time::Instant::now();
    loop {
        m.poll();
        assert!(
            m.fuzz_note_for_test().is_none(),
            "the row was built before the window was seen"
        );
        if m.can_undo() {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(60), "no switch");
    }
}

/// A user edit: the diffuse's alpha policy.
fn user_edit(m: &mut AppModel) {
    m.set_field(
        DIFFUSE,
        "alpha.policy",
        json!(engine::config::AlphaPolicy::Remove),
    );
}

/// The dialogs the close, open and reload requests raise now (each from the same state).
fn asks(m: &mut AppModel, root: &Path) -> [bool; 3] {
    let close = !m.request_close();
    m.dialog = None;
    let opened = m.request_open(root.to_path_buf());
    let open = !opened;
    m.dialog = None;
    if opened {
        idle(m);
    }
    // The open above may have re-read the documents; reload last.
    m.request_reload();
    let reload = m.dialog.is_some();
    m.dialog = None;
    idle(m);
    [close, open, reload]
}

/// Fix round 1, I1 (FI4): the switch alone, with its joined recording, is an automatic change: no title mark, no
/// close, open or reload question, no crash-recovery copy; Save says what it writes.
#[test]
fn the_switch_alone_asks_nothing() {
    for accept in [AcceptSuggestions::None, AcceptSuggestions::High] {
        let d = tempfile::tempdir().unwrap();
        let mut m = open_with(
            d.path(),
            &["rock01_roughness.png", "rock01_fuzz.png"],
            OLD_SKIN,
            accept,
        );
        assert!(m.fuzz_note_for_test().is_some(), "{accept:?}");
        assert!(m.is_dirty(), "{accept:?}");
        assert!(!m.prompts_unsaved(), "{accept:?}");
        assert!(m.only_automatic_unsaved(), "{accept:?}");
        assert!(
            !sdc_gui::ui::header::window_title(&m).ends_with('●'),
            "{accept:?}"
        );
        assert!(m.recovery_snapshot().is_empty(), "{accept:?}");
        assert_eq!(asks(&mut m, d.path()), [false; 3], "{accept:?}");
    }
}

/// Fix round 1, I1: a user edit after the switch asks, in all three places.
#[test]
fn a_user_edit_after_the_switch_asks() {
    for accept in [AcceptSuggestions::None, AcceptSuggestions::High] {
        let d = tempfile::tempdir().unwrap();
        let mut m = open_with(
            d.path(),
            &["rock01_roughness.png", "rock01_fuzz.png"],
            OLD_SKIN,
            accept,
        );
        user_edit(&mut m);
        idle(&mut m);
        assert!(m.prompts_unsaved(), "{accept:?}");
        assert!(!m.request_close(), "{accept:?}");
        m.dialog = None;
        assert!(!m.request_open(d.path().to_path_buf()), "{accept:?}");
        m.dialog = None;
        m.request_reload();
        assert!(m.dialog.is_some(), "{accept:?}");
    }
}

/// Fix round 1, I1: a later automatic recording in its own step, after a clean switch, still asks nothing.
#[test]
fn a_later_recording_after_the_switch_asks_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        OLD_SKIN,
        AcceptSuggestions::High,
    );
    let pushes = m.history.pushes;
    rgb(&d.path().join("m"), "stone01.png");
    m.replan();
    idle(&mut m);
    assert!(subtype(&m, "m/stone01.png").is_some(), "recorded");
    assert!(m.history.pushes > pushes, "a step of its own");
    assert!(!m.prompts_unsaved());
}

/// Fix round 1, I1: Undo of an unsaved switch leaves the saved text, so nothing is unsaved; Undo of a saved switch
/// leaves a change the user made, so it asks.
#[test]
fn the_switchs_undo_asks_only_when_it_changes_the_saved_text() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        OLD_SKIN,
        AcceptSuggestions::None,
    );
    m.undo();
    idle(&mut m);
    assert!(!m.is_dirty() && !m.prompts_unsaved());
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        OLD_SKIN,
        AcceptSuggestions::None,
    );
    m.save().unwrap();
    m.undo();
    idle(&mut m);
    assert!(m.prompts_unsaved());
}

/// Fix round 1, I1: between the switch and its row, the switch alone asks nothing.
#[test]
fn the_window_before_the_switch_row_asks_nothing() {
    for accept in [AcceptSuggestions::None, AcceptSuggestions::High] {
        let d = tempfile::tempdir().unwrap();
        let mut m = old_unsettled(d.path(), accept);
        until_switch_pending(&mut m);
        assert!(m.is_dirty(), "{accept:?}");
        assert!(!m.prompts_unsaved(), "{accept:?}");
        assert!(m.request_close(), "{accept:?}");
        idle(&mut m);
    }
}

/// Fix round 1, I2: a user edit made before the switch's row is built: the row never offers Undo (it would take back
/// the user's edit, or only the recording), and the user's edit asks.
#[test]
fn a_user_edit_before_the_switch_row_takes_no_undo_from_it() {
    for accept in [AcceptSuggestions::None, AcceptSuggestions::High] {
        let d = tempfile::tempdir().unwrap();
        let mut m = old_unsettled(d.path(), accept);
        until_switch_pending(&mut m);
        user_edit(&mut m);
        idle(&mut m);
        let (text, _) = m.fuzz_note_for_test().expect("the switch row");
        let rows = problems(&m);
        let row = rows
            .iter()
            .find(|r| r.text == text)
            .expect("the switch row is listed");
        assert_eq!(row.fix, None, "{accept:?}: {rows:?}");
        assert!(m.prompts_unsaved(), "{accept:?}");
        // The normal Undo takes back the user's edit only (under High the recording's own step is on top of it).
        let undos = if accept == AcceptSuggestions::High {
            2
        } else {
            1
        };
        for _ in 0..undos {
            m.undo();
            idle(&mut m);
        }
        assert_eq!(
            m.project
                .as_ref()
                .unwrap()
                .file_value(DIFFUSE, "alpha.policy"),
            None,
            "{accept:?}"
        );
        assert!(!set_view(&m).skin, "{accept:?}: still switched");
    }
}

/// Fix round 1, I3 (a): a reload is an open: the old row goes and the reloaded text is switched again, with a fresh
/// row whose Undo works.
#[test]
fn a_reload_switches_the_reloaded_text_again() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        OLD_SKIN,
        AcceptSuggestions::None,
    );
    let first = m.history.pushes;
    m.request_reload();
    assert_eq!(m.dialog, None);
    idle(&mut m);
    assert!(!set_view(&m).skin, "switched again");
    assert!(m.history.pushes > first, "a fresh step");
    let (text, lines) = m.fuzz_note_for_test().expect("a fresh row");
    assert!(
        lines.contains(&"rock01.png: skin albedo → pbr albedo".to_string()),
        "{lines:?}"
    );
    assert!(problems(&m)
        .iter()
        .any(|r| r.text == text && r.fix == Some(Fix::UndoLast)));
    m.undo();
    idle(&mut m);
    assert!(set_view(&m).skin);
    assert_eq!(m.project.as_ref().unwrap().text(), OLD_SKIN);
}

/// Fix round 1, I3 (b): a reload while the switch's step waits for its row: no stale line, one correct row.
#[test]
fn a_reload_while_the_switch_is_pending_builds_one_correct_row() {
    for accept in [AcceptSuggestions::None, AcceptSuggestions::High] {
        let d = tempfile::tempdir().unwrap();
        let mut m = old_unsettled(d.path(), accept);
        until_switch_pending(&mut m);
        m.reload_documents();
        idle(&mut m);
        let (text, lines) = m.fuzz_note_for_test().expect("the row");
        assert!(
            !lines
                .iter()
                .any(|l| l.contains("skin albedo → skin albedo")),
            "{accept:?}: {lines:?}"
        );
        assert!(
            lines.contains(&"rock01.png: skin albedo → pbr albedo".to_string()),
            "{accept:?}: {lines:?}"
        );
        assert_eq!(
            problems(&m).iter().filter(|r| r.text == text).count(),
            1,
            "{accept:?}"
        );
        assert!(!set_view(&m).skin, "{accept:?}");
    }
}

/// Fix round 1, I3 (c): a hand edit of `sdc.toml` while only the switch is unsaved is taken once, the set is switched
/// again with one row, and a second check reloads nothing.
#[test]
fn a_hand_edit_under_an_unsaved_switch_is_taken_once() {
    let d = tempfile::tempdir().unwrap();
    let mut m = open_with(
        d.path(),
        &["rock01_roughness.png", "rock01_fuzz.png"],
        OLD_SKIN,
        AcceptSuggestions::None,
    );
    let edited = format!("{OLD_SKIN}\n[files.\"m/rock01.png\".alpha]\npolicy = \"remove\"\n");
    std::fs::write(d.path().join("sdc.toml"), &edited).unwrap();
    m.check_sources();
    idle(&mut m);
    let doc = m.project.as_ref().unwrap();
    assert!(
        doc.file_value(DIFFUSE, "alpha.policy").is_some(),
        "the hand edit is taken: {}",
        doc.text()
    );
    assert_eq!(subtype(&m, DIFFUSE), None, "switched again");
    let (text, _) = m.fuzz_note_for_test().expect("the row");
    assert_eq!(problems(&m).iter().filter(|r| r.text == text).count(), 1);
    let generation = m.generation();
    m.check_sources();
    idle(&mut m);
    assert_eq!(m.generation(), generation, "no second reload");
    assert_eq!(subtype(&m, DIFFUSE), None);
}

/// Fix round 1, M2: a switched file that has no job when the row is built (deleted meanwhile) gets a line with no
/// arrow.
#[test]
fn a_switched_file_gone_before_the_row_gets_a_line_without_an_arrow() {
    let d = tempfile::tempdir().unwrap();
    let mut m = old_unsettled(d.path(), AcceptSuggestions::None);
    until_switch_pending(&mut m);
    // The switch's plan is in flight: park its result, then plan again without the diffuse.
    m.hold_plan_results = true;
    let start = std::time::Instant::now();
    while m.take_held().is_empty() {
        m.poll();
        assert!(start.elapsed() < Duration::from_secs(60), "no plan result");
        std::thread::sleep(Duration::from_millis(5));
    }
    m.hold_plan_results = false;
    std::fs::remove_file(d.path().join("m/rock01.png")).unwrap();
    m.replan();
    idle(&mut m);
    let (_, lines) = m.fuzz_note_for_test().expect("the row");
    assert_eq!(
        lines.first().map(String::as_str),
        Some("rock01.png: skin albedo"),
        "{lines:?}"
    );
}
