#![allow(dead_code)]
//! A folder like a small mod: a diffuse with varying alpha (needs an alpha decision), a normal with no stated green
//! convention, a 3 px diffuse (blocked by DIM-SMALL), a plain diffuse, and roughness + AO that pack `_rmaos`.
use eframe::egui;
use engine::codes::Code;
use engine::config::{AlphaPolicy, GreenConvention, Subtype};
use image::{ImageBuffer, Luma, Rgb, Rgba};
use sdc_gui::model::{Answer, AppModel};
use sdc_gui::settings::GuiSettings;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Both pairs are (sdc.toml, sdc-project.toml). Recovery includes pending automatic writes alongside user work.
/// Check the dialog's flags and every recovery path and text.
#[track_caller]
pub fn check_close(m: &mut AppModel, dirty: (bool, bool), recovery: (bool, bool)) {
    assert_eq!(m.request_close(), !(dirty.0 || dirty.1), "close");
    assert_eq!(
        matches!(m.dialog, Some(sdc_gui::model::Dialog::CloseUnsaved)),
        dirty.0 || dirty.1,
        "close dialog"
    );
    m.dialog = None;
    let files = m.project.as_ref().unwrap();
    let sets = m.sets_doc.as_ref().unwrap();
    assert_eq!((files.user_dirty(), sets.user_dirty()), dirty, "documents");
    let expected: Vec<_> = [
        (recovery.0, files.path(), files.text()),
        (recovery.1, sets.path(), sets.text()),
    ]
    .into_iter()
    .filter(|(include, _, _)| *include)
    .map(|(_, path, text)| (path.to_owned(), text))
    .collect();
    assert_eq!(m.recovery_snapshot(), expected, "recovery");
}

/// Complete a fixture's ordinary drop immediately, approving exactly the returned occupant.
pub fn accept_drop(
    m: &mut AppModel,
    item: &str,
    set: &str,
    choice: sdc_gui::model::sets::DropChoice,
) -> Result<(), String> {
    use sdc_gui::model::sets::DropOutcome;
    match m.drop_choice(item, set, choice) {
        DropOutcome::AskReplace { kind, existing } => {
            m.drop_replace(item, set, choice, kind, &existing)
        }
        DropOutcome::Joined => Ok(()),
        DropOutcome::Refused(why) => Err(why),
        DropOutcome::AskChoice { .. } => unreachable!("the fixture supplied its choice"),
    }
}

pub fn shapes<S>(h: &egui_kittest::Harness<'_, S>) -> Vec<egui::Shape> {
    fn flat(s: &egui::Shape, out: &mut Vec<egui::Shape>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|s| flat(s, out)),
            s => out.push(s.clone()),
        }
    }
    let mut out = vec![];
    for c in &h.output().shapes {
        flat(&c.shape, &mut out);
    }
    out
}

pub fn mod_folder(dir: &Path) -> PathBuf {
    let r = dir.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    let grad = |x: u32, y: u32| [(x * 4) as u8, (y * 4) as u8, 90u8];
    ImageBuffer::from_fn(64, 64, |x, y| Rgb(grad(x, y)))
        .save(r.join("rock01.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| {
        let g = grad(x, y);
        Rgba([g[0], g[1], g[2], if x < 32 { 255 } else { 0 }])
    })
    .save(r.join("leaf.png"))
    .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(r.join("rock01_normal.png"))
        .unwrap();
    ImageBuffer::from_fn(3, 3, |x, y| Rgb(grad(x, y)))
        .save(r.join("odd.png"))
        .unwrap();
    for (n, v) in [("rock01_roughness.png", 120u8), ("rock01_ao.png", 200)] {
        ImageBuffer::from_fn(64, 64, |x, _| Luma([v.wrapping_add(x as u8)]))
            .save(r.join(n))
            .unwrap();
    }
    dir.join("textures")
}

/// Rewrites the opened fixture's `rock01_normal.png` with a blue of 0 and plans again, so the folder has one check
/// result (CHK-Z-SIGN on that normal) once it is a tangent-space normal; the fixture's own flat normal has none.
pub fn give_a_check(m: &mut AppModel, dir: &Path) {
    give_a_check_at(m, &dir.join("textures/landscape/rock01_normal.png"));
}

/// `give_a_check` for the normal map at `normal` (a blue of 0 gives CHK-Z-SIGN once it is a tangent-space normal).
pub fn give_a_check_at(m: &mut AppModel, normal: &Path) {
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 0]))
        .save(normal)
        .unwrap();
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
}

/// A colour map under `path`'s folder (created): a gradient with the given blue.
pub fn colour(path: &Path, tint: u8) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, tint]))
        .save(path)
        .unwrap();
}

/// Add a High-confidence diffuse between history steps and check that it recorded without a command.
pub fn record_another_colour(m: &mut AppModel) {
    let pushes = m.history.pushes;
    let key = "extra_d.png";
    colour(&m.root.as_ref().unwrap().join(key), 90);
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(
        m.project.as_ref().unwrap().file_value(key, "subtype"),
        Some(serde_json::json!("diffuse"))
    );
    assert_eq!(m.history.pushes, pushes);
}

/// A flat normal map (128, 128, 255): nothing to decide, no check result.
pub fn flat(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(path)
        .unwrap();
}

/// A colour map whose right half is transparent: its alpha needs a decision.
pub fn half_alpha(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| {
        Rgba([
            (x * 4) as u8,
            (y * 4) as u8,
            90,
            if x < 32 { 255 } else { 0 },
        ])
    })
    .save(path)
    .unwrap();
}

/// `textures/g`: a ready set `g/<n>` per `ready` (a colour map and a flat `_n`), and a set per `warn` whose colour
/// map has an alpha that needs a decision. Every file is written before the folder is opened as the app opens it,
/// exports under `<dir>/export`, and the model is idle. It asserts nothing (a warn set leaves a problem row).
pub fn project_with(dir: &Path, ready: &[&str], warn: &[&str]) -> AppModel {
    let g = dir.join("textures/g");
    for n in ready {
        colour(&g.join(format!("{n}.png")), 90);
        flat(&g.join(format!("{n}_n.png")));
    }
    for n in warn {
        half_alpha(&g.join(format!("{n}.png")));
        flat(&g.join(format!("{n}_n.png")));
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    assert!(
        m.wait_idle(Duration::from_secs(120)),
        "the model did not settle"
    );
    m
}

/// `project_with(dir, names, &[])`, asserted clean: no problem row, nothing undecided, `export_is_clean()`. A fixture
/// drift fails here, not in a later assert. Each name gives `textures/g/{n}.png` and `{n}_n.png`.
pub fn ready_project(dir: &Path, names: &[&str]) -> AppModel {
    let m = project_with(dir, names, &[]);
    assert!(
        !m.has_problem_rows() && m.undecided() == 0 && m.export_is_clean(),
        "the fixture is not clean"
    );
    m
}

/// `ready_project` with sets `a` and `u`: four outputs, no problem row.
pub fn opened_chosen_decided(dir: &Path) -> AppModel {
    let m = ready_project(dir, &["a", "u"]);
    assert_eq!(m.export_files(), 4);
    m
}

/// Mid-test event: two more clean outputs (`textures/g/{name}.png` and `{name}_n.png`) for the next plan.
pub fn add_ready_set(dir: &Path, name: &str) {
    let g = dir.join("textures/g");
    colour(&g.join(format!("{name}.png")), 90);
    flat(&g.join(format!("{name}_n.png")));
}

/// Mid-test event: a set whose colour map has an alpha that needs a decision, so the next plan has an undecided
/// texture. It starts nothing itself; the test plans (`check_sources`). Never a fixture.
pub fn add_warn_set(dir: &Path, name: &str) {
    let g = dir.join("textures/g");
    half_alpha(&g.join(format!("{name}.png")));
    flat(&g.join(format!("{name}_n.png")));
}

pub fn nif_problem_warning(text: &str) -> sdc_gui::model::meshes::NifProblem {
    use sdc_gui::model::meshes::{NifLevel, NifProblem, NifTab};
    NifProblem {
        level: NifLevel::Warning,
        path: "meshes/rock.nif".into(),
        text: text.into(),
        tab: NifTab::Shader,
        field: None,
    }
}

/// A NIF review row for `path` that Review's Export would write: not blocked, the given foreign hash, no answer.
pub fn nif_row(path: &str, foreign: Option<[u8; 32]>) -> sdc_gui::model::meshes::NifReviewRow {
    sdc_gui::model::meshes::NifReviewRow {
        path: path.into(),
        mod_name: "My Mod".into(),
        blocked: None,
        foreign,
        answer: None,
        compaction: None,
    }
}

/// A8: `pbr_ready` with leaf and odd decided (not `decide_all`, which would make rock01 a plain diffuse), exported once
/// and settled; then the PGPatcher JSON is edited by hand. Returns the model and the JSON's path. (Was
/// `review.rs::clean_pbr_with_edited_json`.)
pub fn pbr_with_edited_json(dir: &Path) -> (AppModel, PathBuf) {
    let mut m = pbr_ready(dir);
    m.decide("landscape/leaf.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/leaf.png", Answer::Alpha(AlphaPolicy::Cutout));
    m.decide("landscape/odd.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/odd.png", Answer::Acknowledge(Code::DimSmall));
    assert!(
        m.wait_idle(Duration::from_secs(120)),
        "model did not settle"
    );
    // C2: clean, so the first export starts without Review.
    m.export_clicked();
    assert!(m.review.is_none(), "{:?}", m.review_gate());
    assert!(
        m.wait_idle(Duration::from_secs(120)),
        "model did not settle"
    );
    let p = pgp_json_path(&m);
    assert!(p.is_file(), "{:?}", m.export.notes);
    std::fs::write(&p, "{ \"edited\": true }\n").unwrap();
    (m, p)
}

/// A session of its own per call: a shared one would drop another test's cache entries on every plan (audit 8).
pub fn session() -> Arc<engine::session::Session> {
    Arc::new(engine::session::Session::new().unwrap())
}

pub fn opened(dir: &Path) -> AppModel {
    let mut m = opened_unchosen(dir);
    choose_export(&mut m, dir.join("export"));
    m
}

pub fn png_4x4() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(4, 4, Rgb([128, 128, 128]))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

pub fn write_auto_png(dir: &Path, name: &str) {
    let textures = dir.join("textures");
    std::fs::create_dir_all(&textures).unwrap();
    std::fs::write(textures.join(name), png_4x4()).unwrap();
}

pub fn write_auto_decidable(dir: &Path) {
    write_auto_png(dir, "teeth_n.png");
    colour(&dir.join("textures/other_unknown.png"), 90);
}

pub fn write_undecided_pngs(dir: &Path, n: usize) {
    for i in 0..n {
        colour(&dir.join(format!("textures/mystery_{i}.png")), i as u8);
    }
}

pub fn auto_project(dir: &Path) -> AppModel {
    write_auto_decidable(dir);
    let mut m = AppModel::new(GuiSettings::default(), None, session());
    m.open_folder(dir.to_path_buf());
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

pub fn set_type(m: &mut AppModel, file: &str, subtype: &str) {
    m.set_field(
        &format!("textures/{file}"),
        "subtype",
        serde_json::json!(subtype),
    );
}

pub fn touch_sdc_toml_by_hand(dir: &Path, text: &str) {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("sdc.toml"))
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.set_modified(std::time::SystemTime::now() + Duration::from_secs(2))
        .unwrap();
}

pub fn cs_opened_chosen(dir: &Path) -> AppModel {
    let m = opened(dir);
    assert_eq!(m.target, engine::config::Target::Cs);
    m
}

/// `opened` before the export folder is chosen: `gui.toml` names one, the project has none (Note FB1), and nothing
/// is written to the project folder.
pub fn opened_unchosen(dir: &Path) -> AppModel {
    let root = mod_folder(dir);
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    // M3-era fixture: High members stay undecided, as before M6 Track E (DECISIONS Note U3).
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(root);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m
}

/// Note FB1: a project's export folder is the one chosen for it (the `gui.toml` value is no longer inherited), so a
/// fixture that exports picks its folder as the user does and waits for the re-plan.
pub fn choose_export(m: &mut AppModel, folder: PathBuf) {
    m.set_export_folder(folder);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

/// Task 5: `opened` with `m.sets` replaced by `n` clones of the first set, each under its own key, members unchanged.
/// The synthetic sets last only until the next plan: any re-plan (a reorder does one) replaces them with the fixture's
/// own sets. After an edit, check `sets_doc.order()`, not `views`.
pub fn many_sets(dir: &Path, n: usize) -> AppModel {
    let mut m = opened(dir);
    let first = m.sets[0].clone();
    m.sets = (0..n)
        .map(|i| {
            let mut s = first.clone();
            s.key = format!("{}{i:04}", first.key);
            s
        })
        .collect();
    m.invalidate_views();
    // A10: a doc-created set or a dedupe must not change N.
    assert_eq!(sdc_gui::model::sets::views(&m).len(), n);
    m
}

/// Note HJ2: `set_output_name` refuses a name on a non-PBR set, but an old project can still hold one. This seeds
/// `output_name` in `sdc-project.toml` as such a project would, and plans again.
pub fn seed_output_name(m: &mut AppModel, id: &str, name: &str) {
    let id = id.to_string();
    let name = name.to_string();
    m.edit_docs("Seed output name", None, true, move |_, d| {
        d.set_output_name(&id, Some(&name))
    });
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

/// The M3a fixture with rock01 decided as a TruePBR set: `pbr_albedo` diffuse, `pbr_normal` normal (DirectX green).
/// leaf.png and odd.png stay undecided, as in M3a.
pub fn pbr_opened(dir: &Path) -> AppModel {
    use engine::config::{GreenConvention, Subtype};
    use sdc_gui::model::Answer;
    let mut m = opened(dir);
    m.decide("landscape/rock01.png", Answer::Subtype(Subtype::PbrAlbedo));
    m.decide(
        "landscape/rock01_normal.png",
        Answer::Subtype(Subtype::PbrNormal),
    );
    m.decide(
        "landscape/rock01_normal.png",
        Answer::Green(GreenConvention::Directx),
    );
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m
}

pub fn key_less_json_sets(dir: &Path) -> AppModel {
    use sdc_gui::model::sets::{views, SlotFill, SlotKind};
    let mut m = pbr_opened(dir);
    for id in ["landscape/empty", "landscape/other"] {
        m.edit_docs("t", None, true, |_, s| {
            s.create_set(id);
            s.set_borrow(id, 1, Some("landscape/rock01"));
        });
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
    let all = views(&m);
    for id in ["landscape/empty", "landscape/other"] {
        let v = all.iter().find(|v| v.id == id).unwrap();
        assert!(v.key.is_empty() && v.members.is_empty(), "{v:?}");
        assert!(matches!(v.slots.get(&SlotKind::Diffuse),
            Some(SlotFill::Borrowed { from, .. }) if from == "landscape/rock01"));
    }
    m
}

/// `pbr_opened` with rock01 picked as plain `pbr`: the fixture lives under `textures/landscape/`, so it is auto PBR
/// Landscape (Note BM); tests of plain-PBR cards, slots and the sphere/mesh preview use this.
pub fn pbr_opened_plain(dir: &Path) -> AppModel {
    let mut m = pbr_opened(dir);
    m.set_shader("landscape/rock01", Some(engine::shader::Shader::Pbr));
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m
}

/// A model built as the app builds it (no test overrides), opened on `root` with exports under `export`.
pub fn opened_as_the_app(root: PathBuf, export: PathBuf) -> AppModel {
    let settings = GuiSettings {
        export_folder: Some(export.clone()),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.open_folder(root);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    choose_export(&mut m, export);
    m
}

fn flat_png(dir: &Path, name: &str, rgb: [u8; 3]) {
    ImageBuffer::from_fn(64, 64, |_, _| Rgb(rgb))
        .save(dir.join(name))
        .unwrap();
}

/// A PBR set `wall` (albedo + normal), and a PBR-picked set `leaf` with only `_n` and `_rmaos` (Note HL5).
pub fn no_albedo_pair(dir: &Path) -> AppModel {
    no_albedo_pair_with(dir, &[])
}

/// `no_albedo_pair` with more PNGs in `textures/arch`: each `(name, colour, sub-type)` is written before the folder
/// is opened and decided to its sub-type after the first plan.
pub fn no_albedo_pair_with(dir: &Path, extra: &[(&str, [u8; 3], Subtype)]) -> AppModel {
    no_albedo_pair_at(dir, "", extra)
}

/// `no_albedo_pair_with` opened at `dir/textures/<below>` (`""` is `dir/textures`): the files are the same, so with
/// `below = "arch"` the set ids are `wall` and `leaf` and the item keys have no `arch/` part.
pub fn no_albedo_pair_at(dir: &Path, below: &str, extra: &[(&str, [u8; 3], Subtype)]) -> AppModel {
    let key = |n: &str| {
        if below.is_empty() {
            format!("arch/{n}")
        } else {
            n.to_string()
        }
    };
    let r = dir.join("textures/arch");
    std::fs::create_dir_all(&r).unwrap();
    for (n, c) in [
        ("wall.png", [90, 90, 90]),
        ("wall_rmaos.png", [200, 20, 255]),
        ("leaf_rmaos.png", [100, 50, 255]),
        ("wall_n.png", [128, 128, 255]),
        ("leaf_n.png", [128, 128, 255]),
    ] {
        flat_png(&r, n, c);
    }
    for (n, c, _) in extra {
        flat_png(&r, n, *c);
    }
    let mut m = opened_as_the_app(dir.join("textures").join(below), dir.join("export"));
    for (n, _, st) in extra {
        m.decide(&key(n), Answer::Subtype(*st));
    }
    m.decide(&key("wall.png"), Answer::Subtype(Subtype::PbrAlbedo));
    m.decide(&key("wall_n.png"), Answer::Subtype(Subtype::PbrNormal));
    m.decide(&key("wall_n.png"), Answer::Green(GreenConvention::Directx));
    m.decide(&key("leaf_n.png"), Answer::Subtype(Subtype::PbrNormal));
    m.decide(&key("leaf_n.png"), Answer::Green(GreenConvention::Directx));
    assert!(m.wait_idle(Duration::from_secs(60)));
    for id in sdc_gui::model::sets::views(&m)
        .iter()
        .map(|v| v.id.clone())
        .collect::<Vec<_>>()
    {
        m.set_shader(&id, Some(engine::shader::Shader::Pbr));
        assert!(m.wait_idle(Duration::from_secs(60)));
    }
    m
}

/// Note AZ: give the set whose key ends with `key_end` the complex-material shader (under vanilla it then packs the
/// complex `_m`, the old cpm target) and wait for the re-plan.
pub fn complex_material(m: &mut AppModel, key_end: &str) {
    let views = sdc_gui::model::sets::views(m);
    let id = views
        .iter()
        .find(|v| v.key.ends_with(key_end))
        .unwrap_or_else(|| panic!("no set {key_end}"))
        .id
        .clone();
    m.set_shader(&id, Some(engine::shader::Shader::ComplexMaterial));
    assert!(m.wait_idle(Duration::from_secs(60)));
}

pub fn decide_all(m: &mut AppModel) {
    m.decide("landscape/rock01.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/odd.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/odd.png", Answer::Acknowledge(Code::DimSmall));
    m.decide("landscape/leaf.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/leaf.png", Answer::Alpha(AlphaPolicy::Cutout));
    m.decide(
        "landscape/rock01_normal.png",
        Answer::Subtype(Subtype::Normal),
    );
    m.decide(
        "landscape/rock01_normal.png",
        Answer::Green(GreenConvention::Directx),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
}

/// E7: the sha256 of every file under `roots`, by path. No exclusion: a cache or a recovery file the export writes is
/// a change like any other.
pub fn tree_hashes(roots: &[&Path]) -> std::collections::BTreeMap<PathBuf, String> {
    use sha2::{Digest, Sha256};
    let mut out = std::collections::BTreeMap::new();
    for root in roots {
        for e in walkdir::WalkDir::new(root)
            .into_iter()
            .map(|e| e.unwrap())
            .filter(|e| e.file_type().is_file())
        {
            let bytes = std::fs::read(e.path()).unwrap();
            out.insert(e.path().to_path_buf(), hex::encode(Sha256::digest(&bytes)));
        }
    }
    out
}

/// E7: one entry of Review's list as the listing rules read it: its path, what it is (`""` for an output) and whether
/// it is marked run-dependent (ruling 3c-50).
pub struct Listed {
    pub path: PathBuf,
    pub hover: &'static str,
    pub marked: bool,
}

/// Only the write groups participate in the comparison with the writer. Current, kept and protected paths stay visible in Review.
pub fn listed(w: &sdc_gui::model::review::ReviewWrites) -> Vec<Listed> {
    w.outputs
        .iter()
        .map(|o| Listed {
            path: o.path.clone(),
            hover: "",
            marked: o.marked,
        })
        .chain(w.also_updated.iter().map(|e| Listed {
            path: e.path.clone(),
            hover: e.hover,
            marked: e.marked,
        }))
        .collect()
}

/// A junction may hide the writer's spelling from the walk. A re-encode can also replace a DDS with identical bytes.
pub fn write_snapshot(
    dir: &Path,
    w: &sdc_gui::model::review::ReviewWrites,
) -> std::collections::BTreeMap<PathBuf, String> {
    let mut files = tree_hashes(&[dir]);
    let paths = w
        .outputs
        .iter()
        .map(|o| &o.path)
        .chain(&w.current)
        .chain(&w.kept);
    for path in paths.filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dds"))) {
        if let Some(hash) = sdc_gui::model::history::sha256_of(path) {
            let stamp = std::fs::metadata(path).unwrap().modified().unwrap();
            files.insert(path.clone(), format!("{hash} {stamp:?}"));
        }
    }
    files
}

/// Removals that the export's saves and guarded JSON finishers can account for.
#[derive(Default)]
pub struct RemovalCredits {
    pub initial_saves: std::collections::BTreeSet<PathBuf>,
    /// Listed guarded output -> (the old spelling the writer finds in its folder, its recorded hash).
    pub guarded_twins: std::collections::BTreeMap<PathBuf, (PathBuf, String)>,
}

/// Capture ownership before Export, then match each candidate to its finisher's completed phase.
#[derive(Default)]
pub struct PendingRemovals {
    drawer_epoch: u64,
    folder_epoch: u64,
    initial_saves: std::collections::BTreeSet<PathBuf>,
    twins: Vec<(sdc_gui::model::review::JsonKind, PathBuf, PathBuf, String)>,
}

impl PendingRemovals {
    fn unsaved_documents(m: &AppModel) -> BTreeSet<PathBuf> {
        // Includes tentative stashed edits, dirty writable sets and automatic-only settings edits.
        m.recovery_snapshot()
            .into_iter()
            .map(|(path, _)| path)
            .chain(
                m.project
                    .as_ref()
                    .filter(|p| p.is_dirty())
                    .map(|p| p.path().to_owned()),
            )
            .collect()
    }

    pub fn before_export(m: &AppModel) -> Self {
        use sdc_gui::model::history::{case_twin, record_key, records_at};
        use sdc_gui::model::review::JsonKind;
        let mut credits = Self {
            drawer_epoch: m.drawer_epoch,
            folder_epoch: m.folder_epoch,
            initial_saves: Self::unsaved_documents(m),
            ..Self::default()
        };
        if let Some(doc) = &m.sets_doc {
            let lines = m.review_json();
            let root = m.output_root();
            for (kind, records) in [
                (JsonKind::CsPbr, doc.cs_pbr_shas()),
                (JsonKind::ComplexMaterial, doc.cm_json_shas()),
                (JsonKind::Xpmf, doc.xpmf_shas()),
                (JsonKind::CsSkin, doc.cs_skin_shas()),
            ] {
                // Each finisher resolves legacy relative records at its first (primary) output root.
                let records = records_at(&records, &root);
                for line in lines.iter().filter(|l| l.kind == kind) {
                    if let Some(sha) = records.get(&record_key(&line.path)) {
                        if let Some(old) = case_twin(&line.path) {
                            credits
                                .twins
                                .push((kind, line.path.clone(), old, sha.clone()));
                        }
                    }
                }
            }
        }
        credits
    }

    // A new export advances the drawer epoch. Opening a folder advances both epochs.
    pub fn started(&self, m: &AppModel) -> bool {
        self.folder_epoch == m.folder_epoch && self.drawer_epoch != m.drawer_epoch
    }

    pub fn after_export(mut self, m: &AppModel) -> RemovalCredits {
        if self.folder_epoch != m.folder_epoch {
            return RemovalCredits::default();
        }
        let started = self.started(m);
        if !started {
            let unsaved = Self::unsaved_documents(m);
            self.initial_saves.retain(|path| !unsaved.contains(path));
        }
        RemovalCredits {
            initial_saves: self.initial_saves,
            guarded_twins: self
                .twins
                .into_iter()
                .filter(|(kind, _, _, _)| started && m.guarded_json_phase_passed(*kind))
                .map(|(_, path, old, sha)| (path, (old, sha)))
                .collect(),
        }
    }
}

/// Compare synthetic snapshots with no evidence of a settings save or a guarded JSON rename.
pub fn listing_violations(
    w: &sdc_gui::model::review::ReviewWrites,
    before: &std::collections::BTreeMap<PathBuf, String>,
    after: &std::collections::BTreeMap<PathBuf, String>,
    store: Option<&Path>,
) -> Vec<String> {
    compare_listing(w, before, after, store, &RemovalCredits::default()).violations
}

pub struct ListingComparison {
    pub changed: BTreeSet<PathBuf>,
    pub violations: Vec<String>,
}

/// The removed spelling must still open the single surviving entry under its listed name.
fn replaced_spelling(
    old: &Path,
    entries: &[Listed],
    before: &BTreeMap<PathBuf, String>,
    after: &BTreeMap<PathBuf, String>,
) -> bool {
    if !old.symlink_metadata().is_ok_and(|m| m.is_file()) {
        return false;
    }
    let Some(name) = old.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let folded = name.to_lowercase();
    let same_name = |p: &Path| {
        p.parent() == old.parent()
            && p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.to_lowercase() == folded)
    };
    let mut surviving = after.keys().filter(|p| same_name(p));
    let Some(new) = surviving.next() else {
        return false;
    };
    surviving.next().is_none()
        && !before.keys().any(|p| p != old && same_name(p))
        && entries.iter().any(|e| e.path == *new)
        && old
            .canonicalize()
            .is_ok_and(|resolved| new.canonicalize().ok() == Some(resolved))
}

/// E7, the listing rules (rulings 3c-48, 3c-51, 3c-52, 3c-57, 3c-62), between two `tree_hashes` snapshots taken around one
/// export. Every file whose bytes or name changed is listed, lies under a listed Also folder, or is credited to the one
/// listed entry that causes it, by one of four kinds, each bound to its entry, its folder and its direction:
/// (a) `<document>.recovery` REMOVED, beside a listed document the export saves (`sdc.toml`, `sdc-project.toml`: the
///     two `AppModel::save` writes and clears a recovery for);
/// (b) the selected case twin of a listed guarded JSON REMOVED, with its pre-run hash and a passing finisher phase;
/// (c) ONE `<stem>.<digits>.toml` CREATED beside a listed NIF record, holding that record's bytes from before (the record
///     moved aside, Note GD25); the credit is used up, so a second such file fails;
/// (d) the autosave `nif-edits\<hash>.toml` of a listed NIF record's data path REMOVED, under the session's own scan
///     store `store` only (Note GD5).
///
/// An entry changed when its own path, a file credited to it or (a folder) a file under it changed; every entry that
/// did not change must be marked run-dependent. Returns the changed paths and any violations.
pub fn compare_listing(
    w: &sdc_gui::model::review::ReviewWrites,
    before: &std::collections::BTreeMap<PathBuf, String>,
    after: &std::collections::BTreeMap<PathBuf, String>,
    store: Option<&Path>,
    removals: &RemovalCredits,
) -> ListingComparison {
    use sdc_gui::model::review::HOVER_ALSO_FOLDER;
    let entries = listed(w);
    // (c): the records whose one move aside has been seen.
    let mut moved = vec![false; entries.len()];
    let mut touched = vec![false; entries.len()];
    let mut out = vec![];
    let changed = before
        .keys()
        .chain(after.keys())
        .filter(|p| before.get(*p) != after.get(*p))
        .filter(|p| after.contains_key(*p) || !replaced_spelling(p, &entries, before, after))
        .cloned()
        .collect::<BTreeSet<_>>();
    for p in &changed {
        let at = entries
            .iter()
            .position(|e| e.hover == HOVER_ALSO_FOLDER && p.starts_with(&e.path))
            .or_else(|| entries.iter().position(|e| e.path == *p))
            .or_else(|| credit(p, before, after, &entries, store, &mut moved, removals));
        match at {
            Some(i) => touched[i] = true,
            None => out.push(format!(
                "changed, not listed: {p:?} (before {}, after {})",
                before.contains_key(p),
                after.contains_key(p)
            )),
        }
    }
    for (e, t) in entries.iter().zip(touched) {
        if !t && !e.marked {
            out.push(format!(
                "listed, not changed, not marked: {:?} ({:?})",
                e.path, e.hover
            ));
        }
    }
    ListingComparison {
        changed,
        violations: out,
    }
}

/// The listed entry a changed file not on the list counts under, by the four kinds of `listing_violations`, each bound
/// to the entry kind that causes it. `moved`: the records whose move aside (c) is used up.
fn credit(
    p: &Path,
    before: &std::collections::BTreeMap<PathBuf, String>,
    after: &std::collections::BTreeMap<PathBuf, String>,
    entries: &[Listed],
    store: Option<&Path>,
    moved: &mut [bool],
    removals: &RemovalCredits,
) -> Option<usize> {
    use sdc_gui::model::review::{HOVER_NIF_RECORD, HOVER_PROJECT_TOML, HOVER_SDC_TOML};
    let name = p.file_name()?.to_string_lossy().into_owned();
    let removed = before.contains_key(p) && !after.contains_key(p);
    let created = !before.contains_key(p) && after.contains_key(p);
    let find = |f: &dyn Fn(&Listed) -> bool| entries.iter().position(f);
    if removed {
        // (a) beside a document `AppModel::save` writes.
        let saved = |e: &Listed| {
            matches!(e.hover, HOVER_SDC_TOML | HOVER_PROJECT_TOML)
                && removals.initial_saves.contains(&e.path)
        };
        if let Some(i) = find(&|e| saved(e) && sdc_gui::crash_log::recovery_path(&e.path) == p) {
            return Some(i);
        }
        // (b) the guarded writer's own pre-run spelling and hash.
        if let Some(i) = find(&|e| {
            e.hover.is_empty()
                && removals
                    .guarded_twins
                    .get(&e.path)
                    .is_some_and(|(old, sha)| old == p && before.get(p) == Some(sha))
        }) {
            return Some(i);
        }
        // (d) the autosave of the record's data path, under the session's scan store.
        if let Some(store) = store {
            let autosave_of = |e: &Listed| {
                let rec = e.path.to_string_lossy();
                let data = rec
                    .split_once(r"\.sdc\nif-edits\")?
                    .1
                    .strip_suffix(".sdc-nifedit.toml")?;
                sdc_gui::model::meshes::edit_doc::autosave_path(store, data).ok()
            };
            if let Some(i) =
                find(&|e| e.hover == HOVER_NIF_RECORD && autosave_of(e).as_deref() == Some(p))
            {
                return Some(i);
            }
        }
    }
    // (c) one move aside per record.
    if created {
        let i = (0..entries.len()).find(|&i| {
            let e = &entries[i];
            let stem = e.path.file_stem().unwrap_or_default().to_string_lossy();
            !moved[i]
                && e.hover == HOVER_NIF_RECORD
                && e.path.parent() == p.parent()
                && name
                    .strip_prefix(&format!("{stem}."))
                    .and_then(|r| r.strip_suffix(".toml"))
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
                && before.get(&e.path) == after.get(p)
        })?;
        moved[i] = true;
        return Some(i);
    }
    None
}

/// E7: `pbr_opened` (export folder `dir/export`), then leaf (Diffuse, Cutout) and odd (Diffuse, Acknowledge DimSmall)
/// decided, not rock01's `decide_all` lines. Review's gate is open, so a first export really writes files, among them
/// a PGPatcher JSON.
pub fn pbr_opened_chosen_decided(dir: &Path) -> AppModel {
    let mut m = pbr_opened(dir);
    m.decide("landscape/leaf.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/leaf.png", Answer::Alpha(AlphaPolicy::Cutout));
    m.decide("landscape/odd.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("landscape/odd.png", Answer::Acknowledge(Code::DimSmall));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.review_gate().is_ok(), "{:?}", m.review_gate());
    m
}

/// `pbr_opened`, planned and idle (was pbr_export.rs::ready).
pub fn pbr_ready(dir: &Path) -> AppModel {
    let mut m = pbr_opened(dir);
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

/// E3: the header's Export button reads `Review {n} files…` or, with the project's opt-out, `Export {n} files
/// now`; its label as drawn now (the Review panel's own `Export {n} files` button is below the header, so the
/// header's rect picks the right one).
pub fn header_export_label<S>(h: &egui_kittest::Harness<'_, S>) -> String {
    header_export_action(h).expect("the header Export button")
}

/// Reveal a header button at either its inline or overflow location.
pub fn reveal_header_control<S>(h: &mut egui_kittest::Harness<'_, S>, name: &str) {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::Queryable as _;

    if h.query_by_role_and_label(Role::Button, name).is_none() {
        h.get_by_label("More header items").click();
        h.run_steps(3);
    }
    h.get_by_role_and_label(Role::Button, name);
}

pub fn settings_with_meshes_view() -> GuiSettings {
    let mut settings = GuiSettings::default();
    settings.layout.meshes_view = true;
    settings
}

pub fn header_export_action<S>(h: &egui_kittest::Harness<'_, S>) -> Option<String> {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    let header =
        sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "header").expect("the header was drawn");
    h.query_all_by_role(Role::Button)
        .filter(|n| header.contains_rect(n.rect()))
        .filter_map(|n| n.accesskit_node().label())
        .find(|l| {
            *l == "Export"
                || l.strip_prefix("Review ")
                    .or_else(|| l.strip_prefix("Export "))
                    .is_some_and(|tail| tail.starts_with(|c: char| c.is_ascii_digit()))
        })
}

/// Panel `name`'s outer rect from the last frame that built the panel (`ui::panel_ids`); panics when the panel was
/// never built.
pub fn panel(ctx: &eframe::egui::Context, name: &str) -> eframe::egui::Rect {
    sdc_gui::ui::panel_ids::panel_rect(ctx, name)
        .unwrap_or_else(|| panic!("panel {name} was never built"))
}

/// Whether panel `name` was never built in `ctx`. A test that checks a panel is absent pairs it with a panel the same
/// frame does build, so an unrecorded registry (no panel recorded at all) cannot make the check pass. A renamed salt
/// still passes it vacuously: the control does not catch that.
pub fn panel_never_built(ctx: &eframe::egui::Context, name: &str) -> bool {
    sdc_gui::ui::panel_ids::panel_id(ctx, name).is_none()
}

/// E28: the header's Undo or Redo (`verb`), found by role and name: the bare forms exactly, the `{step}` forms by
/// their prefix. Its name is `{verb}` or `{verb}: {step}` for
/// the Textures history, `{verb} (NIF edit history)` or `{verb} (NIF edit history): {step}` for the NIF one. On the
/// main screen no other button carries one of these names: the toast's and the Problems row's Undo say more.
/// It uses no rect on purpose: while the command palette is open, its Undo / Redo rows match as well, and the finder
/// then panics with every matching label, so a test can never click a header button hidden under the palette.
pub fn header_undo<'h, S>(
    h: &'h egui_kittest::Harness<'_, S>,
    verb: &str,
) -> egui_kittest::Node<'h> {
    use eframe::egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    let nif = format!("{verb} (NIF edit history)");
    let is = |l: &str| {
        l == verb
            || l.starts_with(&format!("{verb}: "))
            || l == nif
            || l.starts_with(&format!("{nif}: "))
    };
    let mut hits: Vec<_> = h
        .query_all_by_role(Role::Button)
        .filter(|n| n.accesskit_node().label().is_some_and(|l| is(&l)))
        .collect();
    match hits.len() {
        1 => hits.pop().unwrap(),
        0 => {
            // Every Undo / Redo name on screen, whatever its role, with the role.
            let all: Vec<String> = h
                .query_all_by_label_contains("Undo")
                .chain(h.query_all_by_label_contains("Redo"))
                .filter_map(|n| {
                    let node = n.accesskit_node();
                    node.label().map(|l| format!("{:?} {l:?}", node.role()))
                })
                .collect();
            panic!("no header {verb} button; Undo / Redo names on screen: {all:?}")
        }
        _ => {
            let labels: Vec<_> = hits
                .iter()
                .filter_map(|n| n.accesskit_node().label())
                .collect();
            panic!("several header {verb} buttons: {labels:?}")
        }
    }
}

/// egui 0.37: the one node of `role` named `name`. A bare label query now also finds the Label beside a named control,
/// or a panel, popup or tooltip of that name; this panics with every node of that name when there is not exactly one.
pub fn widget<'t, 'n, N: egui_kittest::kittest::NodeT<'t> + 't>(
    h: &'n impl egui_kittest::kittest::Queryable<'t, 'n, N>,
    role: eframe::egui::accesskit::Role,
    name: &'t str,
) -> N {
    let mut hits: Vec<N> = h.query_all_by_role_and_label(role, name).collect();
    if hits.len() == 1 {
        return hits.pop().unwrap();
    }
    let all: Vec<String> = h
        .query_all_by_label(name)
        .map(|n| format!("{:?}", n.accesskit_node().role()))
        .collect();
    panic!(
        "{} {role:?} nodes named {name:?}; the roles of every node of that name: {all:?}",
        hits.len()
    )
}

/// The nodes whose name contains `text` (kittest's `label_contains`), leaving out one kind: an interaction area with no
/// widget role of its own (`Role::Unknown`, no `labelled_by`) whose name equals its description. That is the name
/// egui 0.37 copies from `on_hover_text` into a node that has none (`Response::describe_for_accessibility`, egui
/// `response.rs:797-816`: label and description both set to the hover text), so its words are on screen only while
/// the tooltip shows. Every other node is kept: a `Label` (egui never copies onto one; a cut-short Label named by its
/// full text is on screen), any widget with a role of its own, and a shown tooltip's text, which is a Label.
pub fn shown_containing<'t, 'n, N: egui_kittest::kittest::NodeT<'t> + 't>(
    h: &'n impl egui_kittest::kittest::Queryable<'t, 'n, N>,
    text: &'t str,
) -> Vec<N> {
    use eframe::egui::accesskit::Role;
    h.query_all(
        egui_kittest::kittest::by()
            .label_contains(text)
            .predicate(|n| {
                let copied = n.role() == Role::Unknown
                    && n.labelled_by().next().is_none()
                    && n.description().is_some()
                    && n.label() == n.description();
                !copied
            }),
    )
    .collect()
}

/// Note FM35: open the alpha dropdown that shows `shown` and pick `entry` in it. `place` names the dropdown as
/// `alpha_dropdown` does: `""` (texture panel), `"Problems"` or `"Review"`.
pub fn pick_alpha<S>(
    h: &mut egui_kittest::Harness<'static, S>,
    place: &str,
    shown: &str,
    entry: &str,
) {
    use egui_kittest::kittest::Queryable;
    let name = match place {
        "" => format!("Alpha type: {shown}"),
        p => format!("{p} alpha type: {shown}"),
    };
    h.get_by_label(&name).click();
    h.run();
    h.get_by_label(entry).click();
    h.run();
}

/// Export as a user would: Export, then, if Review opened, press its Export.
pub fn export_through(m: &mut AppModel) {
    m.export_clicked();
    if m.review.is_some() {
        m.export_from_review();
    }
}

/// The PGPatcher JSON an export of `m` writes (was pbr_export.rs::json_path).
pub fn pgp_json_path(m: &AppModel) -> PathBuf {
    m.export_root()
        .join("PBRNIFPatcher")
        .join(format!("{}.json", sdc_gui::model::pbr_export::json_name(m)))
}

/// Layout v2 K5: a slot row's label, the leftmost match (the Matching header repeats "Diffuse" and "Normal _n").
pub fn slot_rect<S>(h: &egui_kittest::Harness<'_, S>, label: &str) -> eframe::egui::Rect {
    use egui_kittest::kittest::Queryable;
    h.query_all_by_label(label)
        .map(|n| n.rect())
        .min_by(|a, b| a.min.x.total_cmp(&b.min.x))
        .unwrap_or_else(|| panic!("no slot row {label:?}"))
}

/// Layout v2 K11: whether `r` is inside the Sets column. Its cards' chips are named by their letter, and "RMAOS" is
/// also the set editor's slot row.
pub fn in_sets_column<S>(h: &egui_kittest::Harness<'_, S>, r: eframe::egui::Rect) -> bool {
    sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "sets").is_some_and(|p| p.contains_rect(r))
}

/// Floating windows on screen: an `egui::Window` is a Middle-order area, and so is a viewport embedded as one (Note
/// FB7: a docked centre view opens none).
pub fn windows<S>(h: &egui_kittest::Harness<'_, S>) -> usize {
    h.ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .iter()
            .filter(|l| l.order == eframe::egui::Order::Middle)
            .count()
    })
}

/// The one node labelled `label` lies inside the centre column (Note FB7: a docked view's controls and body).
pub fn assert_in_centre<S>(h: &egui_kittest::Harness<'_, S>, label: &str) {
    use egui_kittest::kittest::Queryable;
    let r = h.get_by_label(label).rect();
    let c = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    assert!(
        c.expand(0.5).contains_rect(r),
        "{label} {r:?} is outside the centre {c:?}"
    );
}

/// The one node labelled `label` outside the Sets column (K11).
pub fn outside_sets<'a, S>(
    h: &'a egui_kittest::Harness<'_, S>,
    label: &'a str,
) -> egui_kittest::Node<'a> {
    use egui_kittest::kittest::Queryable;
    let mut n: Vec<_> = h
        .query_all_by_label(label)
        .filter(|n| !in_sets_column(h, n.rect()))
        .collect();
    assert_eq!(n.len(), 1, "one {label:?} outside the Sets column");
    n.pop().unwrap()
}

/// Task 6: a PBR set under `textures/test` (`wall.png` at grey `albedo`, `wall_rmaos.png` filled with `rmaos`),
/// opened as the app opens a folder, shader Pbr.
pub fn pbr_wall(dir: &Path, albedo: u8, rmaos: [u8; 3]) -> AppModel {
    let r = dir.join("textures/test");
    std::fs::create_dir_all(&r).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([albedo, albedo, albedo]))
        .save(r.join("wall.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb(rmaos))
        .save(r.join("wall_rmaos.png"))
        .unwrap();
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    let id = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.ends_with("wall"))
        .expect("the set")
        .id
        .clone();
    m.set_shader(&id, Some(engine::shader::Shader::Pbr));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

/// A 64 px BC1 DDS at `path` (parent folders made), written by the engine's writer, never copied.
/// How many `.dds` files are under `dir`, at any depth (0 for a folder that does not exist): what an export wrote.
pub fn dds_under(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| e.path())
        .map(|p| {
            if p.is_dir() {
                dds_under(&p)
            } else {
                usize::from(p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dds")))
            }
        })
        .sum()
}

pub fn bc1_dds(path: &Path) {
    use engine::dds::{dds_bytes, DdsDesc, ResolvedHeader};
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let desc = DdsDesc {
        width: 64,
        height: 64,
        format: engine::config::Format::Bc1,
        srgb: false,
        header: ResolvedHeader::Legacy,
    };
    let data = vec![[0x00, 0xF8, 0x00, 0xF8, 0, 0, 0, 0].repeat(256)];
    std::fs::write(path, dds_bytes(&desc, &data).unwrap()).unwrap();
}

/// Note FI2: a released mod's all-DDS PBR sets: `textures/pbr/<s>/<s>.dds`, `<s>_n.dds` and `<s>_rmaos.dds` for each
/// of `sets` under `dir`; returns the `textures` folder.
pub fn dds_pbr_sets(dir: &Path, sets: &[&str]) -> PathBuf {
    for s in sets {
        for suffix in ["", "_n", "_rmaos"] {
            bc1_dds(&dir.join(format!("textures/pbr/{s}/{s}{suffix}.dds")));
        }
    }
    dir.join("textures")
}

/// The painted text galleys of the last frame whose rect lies inside `r.expand(1.0)`, each with whether egui elided
/// it or it ends in `…`. Shapes are flattened as `ui_proof/sets_column_pins.rs` `texts` does.
fn painted_galleys<S>(
    h: &egui_kittest::Harness<'_, S>,
    r: eframe::egui::Rect,
) -> Vec<(String, bool)> {
    use eframe::egui::Shape;
    fn flat(s: &Shape, r: eframe::egui::Rect, out: &mut Vec<(String, bool)>) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| flat(s, r, out)),
            Shape::Text(t) if r.contains_rect(t.visual_bounding_rect()) => {
                let text = t.galley.job.text.clone();
                // `job.text` is the whole text even when egui elided its end: read the elided flag too.
                let cut = t.galley.elided || text.ends_with('…');
                out.push((text, cut));
            }
            _ => {}
        }
    }
    let r = r.expand(1.0);
    let mut out = vec![];
    for c in &h.output().shapes {
        flat(&c.shape, r, &mut out);
    }
    out
}

/// The painted texts inside `r.expand(1.0)`, joined (Tasks 6, 7 and 9).
pub fn painted_in<S>(h: &egui_kittest::Harness<'_, S>, r: eframe::egui::Rect) -> String {
    painted_galleys(h, r)
        .into_iter()
        .map(|(t, _)| t)
        .collect::<String>()
}

/// Whether any text painted inside `r.expand(1.0)` is elided by egui or ends in `…`.
pub fn painted_cut<S>(h: &egui_kittest::Harness<'_, S>, r: eframe::egui::Rect) -> bool {
    painted_galleys(h, r).into_iter().any(|(_, cut)| cut)
}

/// Whether egui elided any text painted inside `r.expand(1.0)` (a hand cut such as `front_cut`'s leading `…` is not
/// egui's; final wave Group 6, ledger 140).
pub fn painted_elided<S>(h: &egui_kittest::Harness<'_, S>, r: eframe::egui::Rect) -> bool {
    use eframe::egui::Shape;
    fn flat(s: &Shape, r: eframe::egui::Rect) -> bool {
        match s {
            Shape::Vec(v) => v.iter().any(|s| flat(s, r)),
            Shape::Text(t) => r.contains_rect(t.visual_bounding_rect()) && t.galley.elided,
            _ => false,
        }
    }
    let r = r.expand(1.0);
    h.output().shapes.iter().any(|c| flat(&c.shape, r))
}

/// The filled rects of one shape (nested shapes flattened), each with its fill's alpha.
pub fn fill_rects_of(s: &eframe::egui::Shape, out: &mut Vec<(eframe::egui::Rect, u8)>) {
    use eframe::egui::Shape;
    match s {
        Shape::Vec(v) => v.iter().for_each(|x| fill_rects_of(x, out)),
        Shape::Rect(r) => out.push((r.rect, r.fill.a())),
        _ => {}
    }
}

/// Every filled rect painted in `shapes`, with its fill's alpha.
pub fn filled_rects(
    shapes: &[eframe::egui::epaint::ClippedShape],
) -> Vec<(eframe::egui::Rect, u8)> {
    let mut all = vec![];
    for s in shapes {
        fill_rects_of(&s.shape, &mut all);
    }
    all
}

/// Of `all` (filled rects, fill alpha), the ones a vertical scroll bar could be at the right edge of `c`: at most 12 px
/// wide, at least 20 px tall, ending within 20 px of `c`'s right edge.
pub fn bar_like(
    all: &[(eframe::egui::Rect, u8)],
    c: eframe::egui::Rect,
) -> Vec<(eframe::egui::Rect, u8)> {
    all.iter()
        .copied()
        .filter(|(r, _)| {
            r.width() <= 12.0
                && r.height() >= 20.0
                && r.max.x <= c.max.x + 0.5
                && r.max.x >= c.max.x - 20.0
        })
        .collect()
}

/// `bar_like` over what `h` painted, at the right edge of `c`, any panel. (rect, fill alpha).
pub fn bar_rects_in<S>(
    h: &egui_kittest::Harness<'_, S>,
    c: eframe::egui::Rect,
) -> Vec<(eframe::egui::Rect, u8)> {
    bar_like(&filled_rects(&h.output().shapes), c)
}

/// The x a control may reach beside a scroll bar whose track is `track`: short of the 10 px the bar takes when hovered
/// (`bar_width`), or, for a set card (a frame in the Sets column whose own margin is wider than the rest), short of the
/// track itself at rest. The caller says whether the control is a set card.
pub fn bar_clearance(is_set_card: bool, track: eframe::egui::Rect, bar_width: f32) -> f32 {
    if is_set_card {
        track.min.x
    } else {
        track.max.x - bar_width
    }
}
