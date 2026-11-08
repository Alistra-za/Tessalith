mod support;
use engine::config::{GreenConvention, Subtype};
use image::{ImageBuffer, Rgb};
use sdc_gui::model::cs_skin_json::fnv1a64;
use sdc_gui::model::{Answer, AppModel};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use support::*;

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)));
}

fn png(dir: &Path, name: &str, rgb: [u8; 3]) {
    ImageBuffer::from_fn(64, 64, |_, _| Rgb(rgb))
        .save(dir.join(name))
        .unwrap();
}

/// A Skyrim-shaped project: a PBR set with albedo + normal + rmaos, a vanilla set, and a PBR set with only `_n` +
/// `_rmaos` (no albedo), target CS, no scan, no target mod.
fn skyrim_shaped(dir: &Path) -> AppModel {
    let r = dir.join("textures/arch");
    std::fs::create_dir_all(&r).unwrap();
    for (n, c) in [
        ("wall.png", [90, 90, 90]),
        ("wall_rmaos.png", [200, 20, 255]),
        ("rock.png", [60, 120, 30]),
        ("leaf_rmaos.png", [100, 50, 255]),
    ] {
        png(&r, n, c);
    }
    for n in ["wall_n.png", "rock_n.png", "leaf_n.png"] {
        png(&r, n, [128, 128, 255]);
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    m.decide("arch/wall.png", Answer::Subtype(Subtype::PbrAlbedo));
    m.decide("arch/wall_n.png", Answer::Subtype(Subtype::PbrNormal));
    m.decide("arch/wall_n.png", Answer::Green(GreenConvention::Directx));
    m.decide("arch/rock.png", Answer::Subtype(Subtype::Diffuse));
    m.decide("arch/rock_n.png", Answer::Subtype(Subtype::Normal));
    m.decide("arch/rock_n.png", Answer::Green(GreenConvention::Directx));
    m.decide("arch/leaf_n.png", Answer::Subtype(Subtype::PbrNormal));
    m.decide("arch/leaf_n.png", Answer::Green(GreenConvention::Directx));
    idle(&mut m);
    let ids: Vec<String> = sdc_gui::model::sets::views(&m)
        .iter()
        .filter(|v| v.key.ends_with("wall") || v.key.ends_with("leaf"))
        .map(|v| v.id.clone())
        .collect();
    for id in ids {
        m.set_shader(&id, Some(engine::shader::Shader::Pbr));
        idle(&mut m);
    }
    m
}

fn files_under(root: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// `{ "json": <PGPatcher JSON, entries sorted by match1>, "dds": { rel path: fnv1a64 hex } }`.
fn collect(m: &AppModel) -> Value {
    let mut json: Value =
        serde_json::from_slice(&std::fs::read(pgp_json_path(m)).expect("the PGPatcher JSON"))
            .unwrap();
    if let Some(a) = json["entries"].as_array_mut() {
        a.sort_by_key(|e| e["match1"].as_str().unwrap_or_default().to_string());
    }
    let root = m.export_root();
    let mut all = vec![];
    files_under(&root, &mut all);
    let dds: BTreeMap<String, String> = all
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "dds"))
        .map(|p| {
            let rel = p
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (rel, format!("{:016x}", fnv1a64(&std::fs::read(p).unwrap())))
        })
        .collect();
    json!({ "json": json, "dds": dds })
}

/// Entries (by `match1`) the later tasks add on purpose; the pin ignores them. Task 4 adds the no-albedo set's.
const NEW_ENTRIES: &[&str] = &[];

#[test]
fn a_skyrim_shaped_project_keeps_its_output() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skyrim_shaped(d.path());
    export_through(&mut m);
    idle(&mut m);
    let mut got = collect(&m);
    assert!(
        !got["dds"].as_object().unwrap().is_empty(),
        "no DDS written"
    );
    let pin = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/skyrim_shaped_pin.json");
    if std::env::var_os("SDC_BLESS_PIN").is_some() {
        std::fs::create_dir_all(pin.parent().unwrap()).unwrap();
        std::fs::write(&pin, serde_json::to_string_pretty(&got).unwrap() + "\n").unwrap();
        return;
    }
    if let Some(a) = got["json"]["entries"].as_array_mut() {
        a.retain(|e| !NEW_ENTRIES.contains(&e["match1"].as_str().unwrap_or_default()));
    }
    let want: Value = serde_json::from_slice(&std::fs::read(&pin).expect("the pin file")).unwrap();
    assert_eq!(got, want);
}

// ---- the target mod setting and reader (Note HL) ----

use sdc_gui::model::bottom::{problems, Level};
use sdc_gui::model::known_paths::{KnownPaths, TxstRecord};
use sdc_gui::model::target_mod::{FileBase, TARGET_IS_EXPORT};

fn trec(edid: &str, plugin: &str, formid: &str, slots: &[(u8, &str)]) -> TxstRecord {
    TxstRecord {
        plugin: plugin.into(),
        winner: plugin.into(),
        formid: formid.into(),
        edid: edid.into(),
        slots: slots.iter().map(|(n, p)| (*n, p.to_string())).collect(),
    }
}

fn scan(m: &mut AppModel, records: Vec<TxstRecord>) {
    m.set_known(KnownPaths {
        records,
        ..KnownPaths::default()
    });
}

/// Poll until the worker's read of the target mod has landed.
fn landed(m: &mut AppModel) {
    let end = std::time::Instant::now() + Duration::from_secs(30);
    while m.target().is_none() {
        m.poll();
        assert!(
            std::time::Instant::now() < end,
            "the target mod never landed"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn project_file(m: &AppModel) -> PathBuf {
    m.root.clone().unwrap().join("sdc-project.toml")
}

fn nif_fixture() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nif/tests/fixtures/TestNifFile_Skinned_SE.nif"
    ))
    .unwrap()
}

/// A target mod folder with a plugin, a mesh, three texture files of one base and a note.
fn target_folder(dir: &Path) -> PathBuf {
    let t = dir.join("target");
    for (rel, bytes) in [
        ("Plug.esp", Vec::new()),
        ("meshes/a/b.nif", nif_fixture()),
        ("Textures/FS6/FS6_Boots_IR.dds", b"d".to_vec()),
        ("Textures/FS6/FS6_Boots_IR_n.dds", b"n".to_vec()),
        ("Textures/FS6/FS6_Boots_IR_m.dds", b"m".to_vec()),
        ("notes.txt", b"x".to_vec()),
    ] {
        let p = t.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }
    t
}

fn plain(dir: &Path) -> AppModel {
    opened_as_the_app(dir.join("textures"), dir.join("export"))
}

#[test]
fn the_target_mod_is_stored_in_the_project_file() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path());
    let t = target_folder(d.path());
    assert_eq!(m.set_target_mod(Some(t.clone())), Ok(()));
    let text = std::fs::read_to_string(project_file(&m)).unwrap();
    assert!(text.contains("target_mod = "), "{text}");
    let root = m.root.clone().unwrap();
    m.open_folder(root);
    idle(&mut m);
    landed(&mut m);
    assert_eq!(m.target().unwrap().root, t);
}

#[test]
fn the_export_folder_cannot_be_the_target_mod() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path());
    let before = std::fs::read_to_string(project_file(&m)).unwrap();
    let export = d.path().join("export");
    for bad in [export.clone(), export.join("sub"), d.path().to_path_buf()] {
        assert_eq!(m.set_target_mod(Some(bad)), Err(TARGET_IS_EXPORT));
        assert_eq!(std::fs::read_to_string(project_file(&m)).unwrap(), before);
    }
}

#[test]
fn a_target_mod_lists_its_plugins_meshes_and_files() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path());
    let t = target_folder(d.path());
    m.set_target_mod(Some(t)).unwrap();
    landed(&mut m);
    let got = m.target().unwrap().clone();
    assert!(got.problems.is_empty(), "{:?}", got.problems);
    assert_eq!(
        got.plugins.iter().collect::<Vec<_>>(),
        vec!["plug.esp"],
        "only the plugin; notes.txt is not one"
    );
    let b = "textures/fs6/fs6_boots_ir";
    assert_eq!(
        got.files,
        vec![FileBase {
            base: b.into(),
            slots: [
                (1, format!("{b}.dds")),
                (2, format!("{b}_n.dds")),
                (6, format!("{b}_m.dds")),
            ]
            .into()
        }]
    );
    let scan = nif::scan::scan(&nif_fixture()).unwrap();
    let want: std::collections::BTreeSet<String> = scan
        .shapes
        .iter()
        .filter_map(|s| match &s.shader {
            nif::scan::Shader::Lighting(_, slots) => slots.first().map(|p| p.norm.clone()),
            _ => None,
        })
        .filter(|p| !p.is_empty())
        .map(|p| p.trim_end_matches(".dds").to_string())
        .collect();
    assert!(!want.is_empty(), "the fixture names a slot-1 path");
    let have: std::collections::BTreeSet<String> =
        got.meshes.iter().map(|x| x.base.clone()).collect();
    assert_eq!(have, want);
    assert!(got.meshes.iter().all(|x| x.meshes == 1));
}

fn edids(m: &AppModel) -> Vec<String> {
    m.target_records().iter().map(|r| r.edid.clone()).collect()
}

#[test]
fn target_records_are_the_scans_records_of_its_plugins() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path());
    let recs = vec![
        trec("A", "Plug.esp", "0x1", &[]),
        trec("B", "Other.esp", "0x2", &[]),
    ];
    scan(&mut m, recs);
    assert_eq!(edids(&m), ["A", "B"]);
    m.set_target_mod(Some(target_folder(d.path()))).unwrap();
    assert!(edids(&m).is_empty(), "set but not read yet: no records");
    landed(&mut m);
    assert_eq!(edids(&m), ["A"]);
}

#[test]
fn a_target_mod_outside_the_scan_has_no_records() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path());
    scan(&mut m, vec![trec("A", "Plug.esp", "0x1", &[])]);
    let t = d.path().join("mine");
    std::fs::create_dir_all(t.join("textures/x")).unwrap();
    std::fs::write(t.join("Mine.esp"), b"").unwrap();
    std::fs::write(t.join("textures/x/w.dds"), b"d").unwrap();
    m.set_target_mod(Some(t)).unwrap();
    landed(&mut m);
    assert!(m.target_records().is_empty());
    assert!(!m.target().unwrap().files.is_empty());
    let rows = problems(&m);
    assert!(rows.iter().all(|p| p.level > Level::Warning), "{rows:?}");
}

#[test]
fn used_by_counts_meshes_and_records() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path());
    m.set_target_mod(Some(target_folder(d.path()))).unwrap();
    landed(&mut m);
    let slot1 = format!("{}.dds", m.target().unwrap().meshes[0].base);
    scan(
        &mut m,
        vec![
            trec("A", "Plug.esp", "0x1", &[(1, &slot1)]),
            trec("B", "Other.esp", "0x2", &[(1, &slot1)]),
        ],
    );
    // One target mesh and one target record (the other record's plugin is not the target's).
    assert_eq!(m.used_by(&slot1), 2);
    assert_eq!(m.used_by(&slot1.to_uppercase()), 2, "ASCII case ignored");
    assert_eq!(m.used_by("textures/none.dds"), 0);
}

fn blocked_row(m: &AppModel) -> bool {
    problems(m)
        .iter()
        .any(|p| p.level == Level::Blocks && p.text == TARGET_IS_EXPORT)
}

fn no_files(dir: &Path) -> bool {
    let mut all = vec![];
    if dir.is_dir() {
        files_under(dir, &mut all);
    }
    all.is_empty()
}

#[test]
fn exporting_into_the_target_mod_is_blocked() {
    // The export folder picked after the target mod.
    let d = tempfile::tempdir().unwrap();
    let mut m = skyrim_shaped(d.path());
    let t = target_folder(d.path());
    m.set_target_mod(Some(t.clone())).unwrap();
    assert!(!blocked_row(&m));
    m.set_export_folder(t.join("out"));
    idle(&mut m);
    assert!(blocked_row(&m));
    export_through(&mut m);
    idle(&mut m);
    assert!(no_files(&t.join("out")), "nothing was exported");

    // A hand-edited `target_mod` that holds the export folder.
    let d = tempfile::tempdir().unwrap();
    let mut m = skyrim_shaped(d.path());
    let file = project_file(&m);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("[project]"), "{text}");
    let edited = text.replacen(
        "[project]",
        &format!("[project]\ntarget_mod = '{}'", d.path().display()),
        1,
    );
    std::fs::write(&file, edited).unwrap();
    let root = m.root.clone().unwrap();
    m.open_folder(root);
    idle(&mut m);
    assert!(blocked_row(&m));
    export_through(&mut m);
    idle(&mut m);
    assert!(no_files(&d.path().join("export")), "nothing was exported");
}

fn snapshot(root: &Path) -> Vec<(PathBuf, u64, Option<std::time::SystemTime>)> {
    let mut all = vec![];
    files_under(root, &mut all);
    all.sort();
    all.into_iter()
        .map(|p| {
            let md = std::fs::metadata(&p).unwrap();
            (p, md.len(), md.modified().ok())
        })
        .collect()
}

#[test]
fn the_reader_never_writes_into_the_target_mod() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skyrim_shaped(d.path());
    let t = target_folder(d.path());
    let before = snapshot(&t);
    m.set_target_mod(Some(t.clone())).unwrap();
    landed(&mut m);
    // Planning again (a scan finishing does the same) reads the target once more.
    m.scan_finished(true, false);
    idle(&mut m);
    export_through(&mut m);
    idle(&mut m);
    assert_eq!(m.target().unwrap().root, t);
    assert_eq!(snapshot(&t), before);
}

// ---- base mappings (Note HL3) ----

use sdc_gui::model::target_mod::MeshPath;
use sdc_gui::model::txst_map::TargetPick;

const BOOTS_BASE: &str = "textures/fs6/fs6_boots_ir";

/// A DAZ-shaped vanilla set `Runner Boots_01` (`_pbr_d` read as the diffuse, `_n`, `_s`) and its set id.
fn boots(dir: &Path) -> (AppModel, String) {
    let r = dir.join("textures/textures/boots");
    std::fs::create_dir_all(&r).unwrap();
    png(&r, "Runner Boots_01_pbr_d.png", [90, 60, 40]);
    png(&r, "Runner Boots_01_n.png", [128, 128, 255]);
    png(&r, "Runner Boots_01_s.png", [40, 40, 40]);
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    m.set_target(engine::config::Target::Vanilla);
    // DAZ shape: the exporter's `_pbr_d` is read as the diffuse.
    m.edit_docs("alias", None, true, |p, _| {
        p.set_read_as(
            "textures/boots/Runner Boots_01_pbr_d.png",
            Some("textures/boots/Runner Boots_01.png"),
        )
    });
    idle(&mut m);
    m.decide(
        "textures/boots/Runner Boots_01_pbr_d.png",
        Answer::Subtype(Subtype::Diffuse),
    );
    m.decide(
        "textures/boots/Runner Boots_01_n.png",
        Answer::Subtype(Subtype::Normal),
    );
    m.decide(
        "textures/boots/Runner Boots_01_n.png",
        Answer::Green(GreenConvention::Directx),
    );
    m.decide(
        "textures/boots/Runner Boots_01_s.png",
        Answer::Subtype(Subtype::BacklightTint),
    );
    idle(&mut m);
    let id = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.to_lowercase().ends_with("runner boots_01"))
        .expect("the boots set")
        .id
        .clone();
    (m, id)
}

fn files(base: &str, slots: &[u8]) -> FileBase {
    let suffix = |n: u8| match n {
        1 => "",
        2 => "_n",
        6 => "_m",
        _ => unreachable!(),
    };
    FileBase {
        base: base.into(),
        slots: slots
            .iter()
            .map(|&n| (n, format!("{base}{}.dds", suffix(n))))
            .collect(),
    }
}

fn exported(m: &mut AppModel) -> Vec<String> {
    export_through(m);
    idle(m);
    let root = m.export_root();
    let mut all = vec![];
    if root.is_dir() {
        files_under(&root, &mut all);
    }
    let mut got: Vec<String> = all
        .iter()
        .map(|p| {
            p.strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    got.sort();
    got
}

fn project_text(m: &AppModel) -> String {
    m.sets_doc.as_ref().unwrap().text()
}

#[test]
fn a_vanilla_set_mapped_to_a_file_base_writes_the_targets_names() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    m.map_target(&id, TargetPick::Files(files(BOOTS_BASE, &[1, 2, 6])))
        .unwrap();
    idle(&mut m);
    let got = exported(&mut m);
    for want in [
        "textures/fs6/fs6_boots_ir.dds",
        "textures/fs6/fs6_boots_ir_n.dds",
    ] {
        assert!(got.iter().any(|g| g == want), "{want} in {got:?}");
    }
    assert!(got.iter().all(|g| !g.ends_with(".json")), "{got:?}");
    let text = project_text(&m);
    assert!(text.contains(&format!("base = \"{BOOTS_BASE}\"")), "{text}");
    assert!(!text.contains("plugin"), "{text}");
}

#[test]
fn a_slot_the_base_lacks_goes_to_base_plus_suffix() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    m.map_target(&id, TargetPick::Files(files(BOOTS_BASE, &[1, 2, 6])))
        .unwrap();
    idle(&mut m);
    assert_no_blocks(&m);
    let got = exported(&mut m);
    assert!(
        got.iter().any(|g| g == "textures/fs6/fs6_boots_ir_s.dds"),
        "{got:?}"
    );
}

fn assert_no_blocks(m: &AppModel) {
    let all = problems(m);
    let rows: Vec<_> = all.iter().filter(|p| p.level == Level::Blocks).collect();
    assert!(rows.is_empty(), "{rows:?}");
}

/// The `_s` (slot 8, record-only) member under a RECORD mapping: it is not a collision with itself either.
#[test]
fn a_slot_8_member_under_a_record_mapping_does_not_collide_with_itself() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    let rec = trec(
        "Boots",
        "Plug.esp",
        "0x1",
        &[
            (1, "textures/fs6/x.dds"),
            (2, "textures/fs6/x_n.dds"),
            (8, "textures/fs6/x_s.dds"),
        ],
    );
    m.map_target(&id, TargetPick::Record(rec)).unwrap();
    idle(&mut m);
    assert_no_blocks(&m);
    let got = exported(&mut m);
    assert!(got.iter().any(|g| g == "textures/fs6/x_s.dds"), "{got:?}");
}

#[test]
fn a_mesh_path_mapping_uses_the_meshes_slot_paths() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    let mesh = MeshPath {
        base: "textures/a/body".into(),
        slots: [
            (1, "textures/a/body.dds".to_string()),
            (2, "textures/a/body_msn.dds".to_string()),
        ]
        .into(),
        meshes: 1,
    };
    m.map_target(&id, TargetPick::Mesh(mesh)).unwrap();
    idle(&mut m);
    let got = exported(&mut m);
    assert!(
        got.iter().any(|g| g == "textures/a/body_msn.dds"),
        "{got:?}"
    );
    assert!(got.iter().any(|g| g == "textures/a/body.dds"), "{got:?}");
}

#[test]
fn a_base_mapping_has_no_record_rows() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    scan(&mut m, vec![trec("A", "Plug.esp", "0x1", &[])]);
    m.map_target(&id, TargetPick::Files(files(BOOTS_BASE, &[1, 2, 6])))
        .unwrap();
    idle(&mut m);
    assert!(project_text(&m).contains("base = "), "mapped");
    let rows = problems(&m);
    assert!(
        rows.iter().all(|p| !p.text.starts_with("Record ")),
        "{rows:?}"
    );
}

#[test]
fn picking_another_source_replaces_the_mapping() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    let rec = trec("Boots", "Plug.esp", "0x1", &[(1, "textures/x/y.dds")]);
    m.map_target(&id, TargetPick::Record(rec)).unwrap();
    idle(&mut m);
    assert!(project_text(&m).contains("plugin"));
    m.map_target(&id, TargetPick::Files(files(BOOTS_BASE, &[1, 2, 6])))
        .unwrap();
    idle(&mut m);
    let text = project_text(&m);
    assert_eq!(text.matches("txst").count(), 1, "{text}");
    assert!(text.contains("base = "), "{text}");
    assert!(!text.contains("plugin"), "{text}");
}

#[test]
fn the_same_base_in_another_case_is_not_a_new_mapping() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    m.map_target(&id, TargetPick::Files(files(BOOTS_BASE, &[1, 2, 6])))
        .unwrap();
    idle(&mut m);
    let before = project_text(&m);
    assert!(before.contains("base = "), "mapped");
    m.map_target(
        &id,
        TargetPick::Files(files("TEXTURES/FS6/FS6_Boots_IR", &[1, 2, 6])),
    )
    .unwrap();
    idle(&mut m);
    assert_eq!(project_text(&m), before);
}

#[test]
fn target_replaces_lists_the_files_the_target_already_ships() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    m.set_target_mod(Some(target_folder(d.path()))).unwrap();
    landed(&mut m);
    let view = |m: &AppModel| {
        let all = sdc_gui::model::sets::views(m);
        all.iter().find(|v| v.id == id).cloned().unwrap()
    };
    assert!(m.target_replaces(&view(&m)).is_empty(), "default names");
    m.map_target(&id, TargetPick::Files(files(BOOTS_BASE, &[1, 2, 6])))
        .unwrap();
    idle(&mut m);
    assert_eq!(
        m.target_replaces(&view(&m)),
        [
            "textures/fs6/fs6_boots_ir.dds",
            "textures/fs6/fs6_boots_ir_n.dds"
        ]
    );
}

/// The views follow the document revision, not TxstMap fields: two bases with equal slots are two mappings.
#[test]
fn two_bases_with_equal_slots_are_not_one_cached_mapping() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    let base_of = |m: &AppModel| {
        let all = sdc_gui::model::sets::views(m);
        let v = all.iter().find(|v| v.id == id).unwrap();
        v.txst.as_ref().and_then(|t| t.base.clone())
    };
    let slots = |b: &str| FileBase {
        base: b.into(),
        slots: [(1, format!("{b}.dds"))].into(),
    };
    m.map_target(&id, TargetPick::Files(slots("textures/a/one")))
        .unwrap();
    idle(&mut m);
    assert_eq!(base_of(&m).as_deref(), Some("textures/a/one"));
    let mut other = slots("textures/a/two");
    other.slots = slots("textures/a/one").slots;
    m.map_target(&id, TargetPick::Files(other)).unwrap();
    idle(&mut m);
    assert_eq!(base_of(&m).as_deref(), Some("textures/a/two"));
}

// ---- the Map to target… dialog (Note HL4) ----

use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::target_mod::RECORDS_NOT_IN_SCAN;

fn harness(m: AppModel, w: f32) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(egui::vec2(w, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

const TARGET_ROW: &str = "Target mod (read only; Export writes to the export folder)";

/// The boots project with the target folder read and a scan holding `records`.
fn with_target(dir: &Path, records: Vec<TxstRecord>) -> (AppModel, String) {
    let (mut m, id) = boots(dir);
    assert_eq!(m.set_target_mod(Some(target_folder(dir))), Ok(()));
    landed(&mut m);
    scan(&mut m, records);
    idle(&mut m);
    (m, id)
}

fn open_dialog(m: AppModel, id: &str, w: f32) -> Harness<'static, AppModel> {
    let mut h = harness(m, w);
    h.run();
    let tail = id.rsplit('/').next().unwrap().to_string();
    let pill = h
        .query_all_by_label(id)
        .next()
        .or_else(|| h.query_all_by_label_contains(&tail).next());
    pill.expect("the set pill").click_secondary();
    h.run();
    h.get_by_label("Map to target…").click();
    h.run();
    h
}

const KINDS: [&str; 3] = ["Record ", "Mesh ", "File "];

fn is_row(l: &str) -> bool {
    KINDS.iter().any(|k| l.starts_with(k))
}

/// The AccessKit names of the list rows, in order.
fn row_names(h: &Harness<'static, AppModel>) -> Vec<String> {
    h.query_all_by_role(Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| is_row(l))
        .collect()
}

fn one_of_each() -> Vec<TxstRecord> {
    vec![trec(
        "FS6Rec",
        "Plug.esp",
        "0x800",
        &[(1, "textures/fs6/rec_d.dds")],
    )]
}

#[test]
fn the_set_menu_offers_map_to_target() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    scan(&mut m, one_of_each());
    let mut h = harness(m, 1280.0);
    h.run();
    let tail = id.rsplit('/').next().unwrap().to_string();
    h.query_all_by_label_contains(&tail)
        .next()
        .expect("the set pill")
        .click_secondary();
    h.run();
    h.get_by_label("Map to target…");
    assert!(h.query_by_label("Map to texture set record…").is_none());
}

#[test]
fn the_dialog_names_the_target_mod_read_only() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    scan(&mut m, one_of_each());
    let h = open_dialog(m, &id, 1280.0);
    h.get_by_label(TARGET_ROW);
    h.get_by_role_and_label(Role::Button, "Choose…");
    assert!(h.query_by_role_and_label(Role::Button, "Clear").is_none());
    let rows = row_names(&h);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(rows[0].starts_with("Record FS6Rec"), "{rows:?}");
}

#[test]
fn the_list_has_records_meshes_and_files_with_counts() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = with_target(d.path(), one_of_each());
    let meshes: Vec<String> = m
        .target()
        .unwrap()
        .meshes
        .iter()
        .map(|x| x.base.clone())
        .collect();
    let mut h = open_dialog(m, &id, 1280.0);
    h.get_by_role_and_label(Role::Button, "Clear");
    let rows = row_names(&h);
    // The record, one row per distinct mesh path of the fixture NIF, the file base.
    assert_eq!(rows.len(), meshes.len() + 2, "{rows:?}");
    assert!(rows[0].starts_with("Record "), "{rows:?}");
    for (row, base) in rows[1..].iter().zip(&meshes) {
        assert!(
            row.starts_with("Mesh ") && row.contains(base.as_str()),
            "{row}"
        );
    }
    assert!(rows.last().unwrap().starts_with("File "), "{rows:?}");
    for row in &rows {
        assert!(row.contains("used by ("), "{row}");
    }
    assert!(rows[0].ends_with("used by (1)"), "{rows:?}");
    assert!(rows.last().unwrap().ends_with("used by (0)"), "{rows:?}");
    h.get_by_label("Search targets").type_text("boots");
    h.run();
    let rows = row_names(&h);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(rows[0].starts_with("File ") && rows[0].contains("fs6_boots_ir"));
}

#[test]
fn the_picker_says_records_come_from_the_scan() {
    let d = tempfile::tempdir().unwrap();
    let other = trec("Other", "Elsewhere.esp", "0x1", &[(1, "textures/o.dds")]);
    let (m, id) = with_target(d.path(), vec![other]);
    let mut h = open_dialog(m, &id, 1280.0);
    h.get_by_label(RECORDS_NOT_IN_SCAN);
    let rows = row_names(&h);
    assert!(rows.iter().all(|r| !r.starts_with("Record ")), "{rows:?}");
    let file = h
        .query_all_by_role(Role::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("File "))
        })
        .expect("the file row");
    file.click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Map").click();
    h.run();
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    let all = sdc_gui::model::sets::views(h.state());
    let v = all.iter().find(|v| v.id == id).unwrap();
    assert_eq!(
        v.txst.as_ref().and_then(|t| t.base.as_deref()),
        Some(BOOTS_BASE)
    );
}

#[test]
fn record_rows_are_disabled_on_an_albedo_pbr_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::pbr_opened(d.path());
    let t = tempfile::tempdir().unwrap();
    assert_eq!(m.set_target_mod(Some(target_folder(t.path()))), Ok(()));
    landed(&mut m);
    scan(&mut m, one_of_each());
    assert!(sdc_gui::model::sets::views(&m)
        .iter()
        .any(|v| v.id == "landscape/rock01" && v.pbr));
    let mut h = open_dialog(m, "landscape/rock01", 1280.0);
    let mut seen = [false; 3];
    for n in h.query_all_by_role(Role::Button) {
        let Some(l) = n.accesskit_node().label() else {
            continue;
        };
        let Some(i) = KINDS.iter().position(|k| l.starts_with(k)) else {
            continue;
        };
        assert_eq!(n.accesskit_node().is_disabled(), i == 0, "{l}");
        seen[i] = true;
    }
    assert_eq!(seen, [true; 3]);
    let rec = h
        .query_all_by_role(Role::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Record "))
        })
        .unwrap();
    rec.hover();
    h.run();
    h.get_by_label("PGPatcher PBR sets name their outputs in the JSON; use Rename outputs");
}

#[test]
fn a_refused_target_is_said_in_the_dialog() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, id) = boots(d.path());
    scan(&mut m, one_of_each());
    let mut h = open_dialog(m, &id, 1280.0);
    h.state_mut().target_picked(d.path().join("export"));
    h.run();
    assert!(h.state().target().is_none());
    h.get_by_label(TARGET_IS_EXPORT);
}

#[test]
fn the_dialog_fits_at_1100_px() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = with_target(d.path(), one_of_each());
    let h = open_dialog(m, &id, 1100.0);
    let edge = h.get_by_role_and_label(Role::Button, "Map").rect().max.x;
    assert!(row_names(&h).len() >= 3);
    for n in h.query_all_by_role(Role::Button) {
        let Some(l) = n.accesskit_node().label() else {
            continue;
        };
        if !is_row(&l) {
            continue;
        }
        let rect = n.rect();
        let c = h
            .ctx
            .viewport(|v| {
                v.this_pass
                    .widgets
                    .layers()
                    .flat_map(|(_, ws)| ws)
                    .find(|w| w.rect == rect)
                    .map(|w| w.interact_rect)
            })
            .expect("the row is a widget");
        assert!(
            c.max.x <= edge + 0.5,
            "{l}: ends at {}, dialog at {edge}",
            c.max.x
        );
    }
}

#[test]
fn clear_removes_the_target_and_the_list_goes_back_to_the_scan() {
    let d = tempfile::tempdir().unwrap();
    let (m, id) = with_target(d.path(), one_of_each());
    let n = m.target().unwrap().meshes.len() + 2;
    let mut h = open_dialog(m, &id, 1280.0);
    assert_eq!(row_names(&h).len(), n, "{:?}", row_names(&h));
    h.get_by_role_and_label(Role::Button, "Clear").click();
    h.run();
    assert!(h.state().target().is_none());
    assert!(project_text(h.state()).find("target_mod").is_none());
    let rows = row_names(&h);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(rows[0].starts_with("Record "), "{rows:?}");
    assert!(h.query_by_role_and_label(Role::Button, "Clear").is_none());
}

/// Screenshots of the dialog at 1100, 1280 and 1600 px into `SDC_SHOT_DIR` (skipped without it). Run alone, like
/// the other `render_` tests.
#[test]
fn render_map_to_target_dialog() {
    let Some(out) = std::env::var_os("SDC_SHOT_DIR") else {
        return;
    };
    let long = trec(
        "A_Very_Long_Editor_Id_That_Goes_On_And_On_For_A_While_Before_It_Ends",
        "Plug.esp",
        "0xABCDEF",
        &[(1, "textures/fs6/rec_d.dds")],
    );
    for w in [1100.0, 1280.0, 1600.0] {
        let d = tempfile::tempdir().unwrap();
        let mut records = one_of_each();
        records.push(long.clone());
        for n in 0..12 {
            records.push(trec(&format!("Filler{n:02}"), "Plug.esp", "0x900", &[]));
        }
        let (m, id) = with_target(d.path(), records);
        let mut h = open_dialog(m, &id, w);
        let row = h
            .query_all_by_role(Role::Button)
            .find(|n| {
                n.accesskit_node()
                    .label()
                    .is_some_and(|l| l.starts_with("Record Filler"))
            })
            .unwrap();
        row.click();
        h.run();
        let img = h.render().expect("render");
        img.save(Path::new(&out).join(format!("map-target-{w}.png")))
            .expect("save png");
    }
}
