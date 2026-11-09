//! Note GI (XPMF Task 6, design X2, X5): XPMF profiles in a full export, in Review's What will be written, as stale
//! rows (FC11).
mod support;
use engine::presets::AlsoTarget;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::cs_pbr::Kind;
use sdc_gui::model::history::{record_key, sha256_of};
use sdc_gui::model::review::{DiskState, JsonKind};
use sdc_gui::model::xpmf_json::prepare;
use sdc_gui::model::AppModel;
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};
use std::time::Duration;

const ID: &str = "landscape/rock01";
const FILE: &str = "SKSE/Plugins/XPMF/tessalith_rock01.json";

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(120)),
        "the model did not settle"
    );
}

/// As `xpmf_json.rs`: `support::pbr_opened` (rock01 auto PBR Landscape on CS) with a `_noise` and a `_detail` and a
/// `cs_mato` block `[MatoA, MatoB]`; no `[sets.xpmf]` block yet.
fn opened(dir: &Path) -> AppModel {
    let r = dir.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Luma([((x + y) * 2) as u8]))
        .save(r.join("rock01_noise.png"))
        .unwrap();
    ImageBuffer::from_pixel(64, 64, Rgb([128u8, 128, 255]))
        .save(r.join("rock01_detail.png"))
        .unwrap();
    let mut m = support::pbr_opened(dir);
    m.add_cs_block(ID, Kind::Mato);
    m.set_cs_edids(ID, Kind::Mato, &["MatoA".into(), "MatoB".into()]);
    idle(&mut m);
    m
}

fn set_block(m: &mut AppModel, b: Option<Json>) {
    let b = b.map(|v| v.as_object().unwrap().clone());
    m.sets_doc.as_mut().unwrap().set_xpmf(ID, b);
}

fn export(m: &mut AppModel) {
    idle(m);
    support::export_through(m);
    idle(m);
}

fn profile_path(m: &AppModel) -> PathBuf {
    m.output_root().join(FILE)
}

fn notes(m: &AppModel) -> String {
    m.export.notes.join("\n")
}

fn xpmf_lines(m: &AppModel) -> Vec<sdc_gui::model::review::JsonLine> {
    m.review_json()
        .into_iter()
        .filter(|l| l.kind == JsonKind::Xpmf)
        .collect()
}

#[test]
fn export_writes_profile() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    assert!(
        prepare(&m).is_none(),
        "no block and no record: nothing to do"
    );
    set_block(&mut m, Some(json!({"shelterFade": 40})));
    export(&mut m);
    let f = profile_path(&m);
    let text = std::fs::read_to_string(&f).unwrap_or_else(|e| panic!("{e}; {}", notes(&m)));
    let doc: Json = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["name"], json!(ID));
    assert_eq!(doc["shelterFade"], json!(40));
    assert_eq!(doc["editorIds"], json!(["MatoA", "MatoB"]));
    let shas = m.sets_doc.as_ref().unwrap().xpmf_shas();
    assert_eq!(
        shas.get(&record_key(&f)),
        sha256_of(&f).as_ref(),
        "{shas:?}"
    );
    let doc = m.sets_doc.as_ref().unwrap();
    let side: Json = serde_json::from_slice(&std::fs::read(doc.sidecar_path()).unwrap()).unwrap();
    assert_eq!(
        side["xpmf_sha256"][record_key(&f)].as_str(),
        sha256_of(&f).as_deref(),
        "{side}"
    );
    assert!(
        notes(&m).contains("XPMF profiles written: 1 file"),
        "{}",
        notes(&m)
    );
}

#[test]
fn review_lists_profile() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, Some(json!({"shelterFade": 40})));
    idle(&mut m);
    let f = profile_path(&m);
    let l = xpmf_lines(&m);
    assert_eq!(l.len(), 1, "{:?}", m.review_json());
    assert_eq!((l[0].path.clone(), l[0].state), (f.clone(), DiskState::New));
    export(&mut m);
    set_block(&mut m, Some(json!({"shelterFade": 50})));
    idle(&mut m);
    assert_eq!(xpmf_lines(&m)[0].state, DiskState::Ours, "will be replaced");
    export(&mut m);
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(
        text.contains("\"shelterFade\": 50"),
        "{text}; {}",
        notes(&m)
    );
    assert_eq!(xpmf_lines(&m)[0].state, DiskState::Ours);

    // Review Focus 2: a profile Tessalith did not write is listed as edited and kept by the export.
    let d2 = tempfile::tempdir().unwrap();
    let mut m = opened(d2.path());
    set_block(&mut m, Some(json!({})));
    idle(&mut m);
    let f = profile_path(&m);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, "{\"name\": \"theirs\"}").unwrap();
    // The cached lines key on the document: an edit reads the file again.
    set_block(&mut m, Some(json!({"shelterFade": 30})));
    idle(&mut m);
    assert_eq!(xpmf_lines(&m)[0].state, DiskState::Edited);
    export(&mut m);
    assert_eq!(
        std::fs::read_to_string(&f).unwrap(),
        "{\"name\": \"theirs\"}"
    );
    assert!(
        m.export
            .notes
            .iter()
            .any(|n| n.contains("tessalith_rock01.json")
                && n.contains("kept as edited outside Tessalith")),
        "{}",
        notes(&m)
    );
    assert!(m.sets_doc.as_ref().unwrap().xpmf_shas().is_empty());
}

/// The Task 5 ruling: removing the last block leaves the old profile in place, and the export says so.
#[test]
fn an_orphaned_profile_is_left_in_place_with_a_note() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, Some(json!({})));
    export(&mut m);
    let f = profile_path(&m);
    assert!(f.is_file(), "{}", notes(&m));
    set_block(&mut m, None);
    m.replan();
    idle(&mut m);
    assert!(
        prepare(&m).is_some(),
        "a record with no block still reports"
    );
    std::fs::write(&f, "{}\n").unwrap();
    m.replan();
    idle(&mut m);
    // The export leaves it in place and says so.
    export(&mut m);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "{}\n");
    assert!(
        notes(&m).contains("no set writes this XPMF profile any more"),
        "{}",
        notes(&m)
    );
}

/// As `opened`, but the set lives under `textures/pbr/landscape/`, so the Vanilla folder's paths differ from the PBR
/// folder's (TA12): set `pbr/landscape/rock01`, shader PBR Landscape, with a `cs_mato` block.
const PBR_ID: &str = "pbr/landscape/rock01";

fn opened_pbr_dir(dir: &Path) -> AppModel {
    opened_pbr_dir_with(dir, |_| {})
}

/// `opened_pbr_dir` with `extra(<the set's folder>)` run before the folder is opened.
fn opened_pbr_dir_with(dir: &Path, extra: impl FnOnce(&Path)) -> AppModel {
    use engine::config::{GreenConvention, Subtype};
    use sdc_gui::model::Answer;
    let r = dir.join("textures/pbr/landscape");
    std::fs::create_dir_all(&r).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(r.join("rock01.png"))
        .unwrap();
    ImageBuffer::from_pixel(64, 64, Rgb([128u8, 128, 255]))
        .save(r.join("rock01_normal.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Luma([((x + y) * 2) as u8]))
        .save(r.join("rock01_noise.png"))
        .unwrap();
    ImageBuffer::from_pixel(64, 64, Rgb([128u8, 128, 255]))
        .save(r.join("rock01_detail.png"))
        .unwrap();
    extra(&r);
    let settings = sdc_gui::settings::GuiSettings {
        export_folder: Some(dir.join("export")),
        ..Default::default()
    };
    let mut m = AppModel::new(settings, None, support::session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(dir.join("textures"));
    idle(&mut m);
    support::choose_export(&mut m, dir.join("export"));
    m.decide(
        "pbr/landscape/rock01.png",
        Answer::Subtype(Subtype::PbrAlbedo),
    );
    m.decide(
        "pbr/landscape/rock01_normal.png",
        Answer::Subtype(Subtype::PbrNormal),
    );
    m.decide(
        "pbr/landscape/rock01_normal.png",
        Answer::Green(GreenConvention::Directx),
    );
    m.set_shader(PBR_ID, Some(engine::shader::Shader::PbrLandscape));
    idle(&mut m);
    m.add_cs_block(PBR_ID, Kind::Mato);
    m.set_cs_edids(PBR_ID, Kind::Mato, &["MatoA".into(), "MatoB".into()]);
    idle(&mut m);
    m
}

fn read_doc(p: &Path) -> Json {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p:?}: {e}")))
        .unwrap()
}

/// Final review I1b (owner ruling, Note GI27): with Vanilla ticked, the Vanilla folder gets the Projected noise, the
/// Detail normal and a `pbr = false` copy of a PBR Landscape set's profile. The copy's default textures are the
/// Vanilla folder's files, typed values stay as typed, and `specularMult` is kept. No tick, no copy; a set left out of
/// Also gets none.
#[test]
fn also_vanilla_gets_a_pbr_false_copy_and_the_two_maps() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_pbr_dir(d.path());
    m.sets_doc.as_mut().unwrap().set_xpmf(
        PBR_ID,
        json!({"pbr": true, "shelterFade": 40, "specularMult": 0.7, "detailNormalTexture": "textures/custom/detail.dds"})
            .as_object()
            .cloned(),
    );
    idle(&mut m);
    assert!(prepare(&m).unwrap().vanilla.is_none(), "no tick, no copy");
    m.set_also_tick(AlsoTarget::Vanilla, true);
    idle(&mut m);
    assert!(m.also_on(), "{}", m.status_line);
    let p = prepare(&m).unwrap();
    assert_eq!(
        p.vanilla.as_ref().map(|v| v.files.len()),
        Some(1),
        "{:?}",
        p.notes
    );
    assert!(
        !p.notes.iter().any(|n| n.contains("Vanilla")),
        "the I1 note is gone: {:?}",
        p.notes
    );
    m.set_skip_also(PBR_ID, true);
    idle(&mut m);
    assert_eq!(
        prepare(&m).unwrap().vanilla.unwrap().files.len(),
        0,
        "a set left out of Also"
    );
    m.set_skip_also(PBR_ID, false);
    idle(&mut m);
    export(&mut m);
    let [pbr, van, _] = m.also_folders();
    let main = read_doc(&pbr.join(FILE));
    assert_eq!(main["pbr"], json!(true), "{}", notes(&m));
    assert!(main.get("specularMult").is_none());
    assert_eq!(
        main["diffuseTexture"],
        json!("textures/pbr/landscape/rock01.dds")
    );
    let copy = read_doc(&van.join(FILE));
    // A typed `pbr = true` is the PBR file's; the Vanilla copy is false whatever the block says.
    assert_eq!(copy["pbr"], json!(false));
    assert_eq!(copy["specularMult"], json!(0.7));
    assert_eq!(copy["shelterFade"], json!(40));
    assert_eq!(copy["editorIds"], json!(["MatoA", "MatoB"]), "{copy}");
    assert_eq!(copy["name"], main["name"]);
    // The defaults are the Vanilla folder's files; a typed texture stays as typed.
    assert_eq!(
        copy["diffuseTexture"],
        json!("textures/landscape/rock01.dds")
    );
    assert_eq!(
        copy["normalTexture"],
        json!("textures/landscape/rock01_n.dds")
    );
    assert_eq!(
        copy["noiseTexture"],
        json!("textures/landscape/rock01_noise.dds")
    );
    assert_eq!(
        copy["detailNormalTexture"],
        json!("textures/custom/detail.dds")
    );
    for t in ["rock01", "rock01_n", "rock01_noise", "rock01_detail"] {
        assert!(
            van.join(format!("textures/landscape/{t}.dds")).is_file(),
            "{t}: {}",
            notes(&m)
        );
    }
    let shas = m.sets_doc.as_ref().unwrap().xpmf_shas();
    assert_eq!(
        shas.get(&record_key(&van.join(FILE))),
        sha256_of(&van.join(FILE)).as_ref(),
        "{shas:?}"
    );
}

/// Without the tick an export writes no Vanilla folder and no copy.
#[test]
fn no_vanilla_copy_without_the_tick() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_pbr_dir(d.path());
    m.sets_doc
        .as_mut()
        .unwrap()
        .set_xpmf(PBR_ID, json!({}).as_object().cloned());
    export(&mut m);
    assert!(m.output_root().join(FILE).is_file(), "{}", notes(&m));
    assert!(!m.also_folders()[1].exists());
    assert!(!notes(&m).contains("Vanilla"), "{}", notes(&m));
}

/// I1b fix round 1, Important 1: a main profile the schema refuses keeps its record as `refused`; the Vanilla copy of a
/// set that qualifies does the same, so a recorded copy is not reported as an orphan (nor its record dropped).
#[test]
fn a_refused_profile_does_not_orphan_the_recorded_vanilla_copy() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_pbr_dir(d.path());
    m.set_also_tick(AlsoTarget::Vanilla, true);
    m.sets_doc
        .as_mut()
        .unwrap()
        .set_xpmf(PBR_ID, json!({"shelterFade": 500}).as_object().cloned());
    idle(&mut m);
    assert!(m.also_on(), "{}", m.status_line);
    let p = prepare(&m).unwrap();
    assert!(
        p.files.is_empty() && p.refused == [FILE.to_string()],
        "{p:?}"
    );
    let van = p.vanilla.as_ref().expect("the tick gives a Vanilla child");
    assert_eq!(van.refused, [FILE.to_string()]);
    let path = van.root.join(FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{}\n").unwrap();
    let rec: std::collections::BTreeMap<String, String> =
        [(record_key(&path), sha256_of(&path).unwrap())].into();
    let mut notes = vec![];
    let out = sdc_gui::model::xpmf_json::finish(&p, &rec, &mut notes);
    assert!(
        !notes.iter().any(|n| n.contains("no set writes")),
        "{notes:?}"
    );
    assert!(out.contains_key(&record_key(&path)), "{out:?}");
}

/// I1b fix round 1, Minor 4: the copy is built only for a set the Vanilla derive keeps (`keeps_landscape`): a DDS member
/// is never re-encoded, so the derive skips the set (M9); a set left out of Also is skipped too. Neither gets a copy
/// (nor a "not exported" note about a Vanilla file that will never exist), and a recorded one is an orphan.
#[test]
fn a_set_the_vanilla_derive_skips_gets_no_copy() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_pbr_dir_with(d.path(), |r| {
        // A 4x4 A8R8G8B8 DDS beside the set (the `_p`): the derive step skips the set.
        let mut b: Vec<u8> = b"DDS ".to_vec();
        let mut u = |v: u32| b.extend_from_slice(&v.to_le_bytes());
        for v in [124, 0x100F, 4, 4, 16, 0, 0] {
            u(v);
        }
        for _ in 0..11 {
            u(0);
        }
        for v in [
            32,
            0x41,
            0,
            32,
            0x00ff_0000,
            0x0000_ff00,
            0x0000_00ff,
            0xff00_0000,
            0x1000,
            0,
            0,
            0,
            0,
        ] {
            u(v);
        }
        b.extend(std::iter::repeat_n(0x80u8, 64));
        std::fs::write(r.join("rock01_p.dds"), b).unwrap();
    });
    m.set_also_tick(AlsoTarget::Vanilla, true);
    m.sets_doc
        .as_mut()
        .unwrap()
        .set_xpmf(PBR_ID, json!({}).as_object().cloned());
    idle(&mut m);
    assert!(m.also_on(), "{}", m.status_line);
    let p = prepare(&m).unwrap();
    assert_eq!(
        p.files.len(),
        1,
        "the PBR file is still written: {:?}",
        p.notes
    );
    let van = p.vanilla.as_ref().unwrap();
    assert!(van.files.is_empty() && van.refused.is_empty(), "{van:?}");
}

/// Final review I2 for profiles (Note FM): with Also on, the profile written into the PBR folder is this export's, so the
/// walk of the folders does not note it as a file it did not write.
#[test]
fn the_profile_is_not_noted_as_unwritten() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_also_tick(AlsoTarget::Vanilla, true);
    idle(&mut m);
    assert!(m.also_on(), "{}", m.status_line);
    set_block(&mut m, Some(json!({})));
    export(&mut m);
    let pbr = m.also_folders()[0].clone();
    assert!(pbr.join(FILE).is_file(), "{}", notes(&m));
    let name = pbr.file_name().unwrap().to_string_lossy().into_owned();
    let entry = format!("{name}/{FILE}");
    assert!(
        !m.export.notes.iter().any(|n| n.starts_with(&entry)),
        "{entry}: {:?}",
        m.export.notes
    );
}
/// End to end: a cancelled export, and one with a failed job, write no profile and record no hash.
#[test]
fn a_cancelled_or_failed_export_writes_no_profile() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, Some(json!({})));
    idle(&mut m);
    let f = profile_path(&m);
    support::export_through(&mut m);
    assert!(m.export.running.is_some(), "the export started");
    m.cancel_export();
    idle(&mut m);
    let s = m.export.summary.as_ref().expect("a cancelled export ends");
    assert!(!s.cancelled.is_empty(), "the cancel came too late: {s:?}");
    assert!(!f.exists(), "{}", notes(&m));
    assert!(
        notes(&m).contains("XPMF profiles not written: the export did not finish"),
        "{}",
        notes(&m)
    );
    assert!(m.sets_doc.as_ref().unwrap().xpmf_shas().is_empty());
    // A failed job: a planned source that no longer decodes, with its size and mtime kept so the export does not
    // re-plan it first.
    let src = d.path().join("textures/landscape/rock01_roughness.png");
    let mtime = std::fs::metadata(&src).unwrap().modified().unwrap();
    let mut bytes = std::fs::read(&src).unwrap();
    bytes[..8].copy_from_slice(b"notapng!");
    std::fs::write(&src, &bytes).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&src)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    export(&mut m);
    let s = m.export.summary.as_ref().unwrap();
    assert!(
        s.cancelled.is_empty() && s.failed > 0,
        "{s:?}; {}",
        notes(&m)
    );
    assert!(!f.exists(), "{}", notes(&m));
    assert!(
        notes(&m).contains("XPMF profiles not written: the export did not finish"),
        "{}",
        notes(&m)
    );
    assert!(m.sets_doc.as_ref().unwrap().xpmf_shas().is_empty());
}

/// A failed or cancelled export writes no profile (`finish_xpmf` on its own).
#[test]
fn failed_export_writes_no_profile() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    export(&mut m);
    set_block(&mut m, Some(json!({})));
    idle(&mut m);
    let f = profile_path(&m);
    for failed in [true, false] {
        let p = prepare(&m).unwrap();
        assert_eq!(p.files.len(), 1, "{:?}", p.notes);
        let s = m.export.summary.as_mut().unwrap();
        if failed {
            s.failed = 1;
        } else {
            s.failed = 0;
            s.cancelled.push("x.png".into());
        }
        m.export.notes.clear();
        m.finish_xpmf(Some(p));
        assert!(!f.exists());
        assert!(
            notes(&m).contains("XPMF profiles not written: the export did not finish"),
            "{}",
            notes(&m)
        );
    }
    assert!(m.sets_doc.as_ref().unwrap().xpmf_shas().is_empty());
}

/// Note FW1 (T24), the production flow: a real Also export where one Vanilla file fails (its folder is a file) still
/// writes the PBR folder's profile, because the XPMF profile answers to the PBR phase.
#[test]
fn a_failed_vanilla_file_still_writes_the_pbr_folders_profile() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_pbr_dir(d.path());
    set_block_pbr(&mut m);
    m.set_also_tick(AlsoTarget::Vanilla, true);
    idle(&mut m);
    let [pbr, van, _] = m.also_folders();
    // Note HQ1: the Vanilla phase's output root is `<folder>\textures`, so the file's folder below it is the file.
    std::fs::create_dir_all(van.join("textures")).unwrap();
    std::fs::write(van.join("textures/landscape"), b"not a folder").unwrap();
    export(&mut m);
    let s = m.export.summary.as_ref().expect("the export ended");
    assert!(
        s.failed > 0,
        "the Vanilla file did not fail: {s:?}; {}",
        notes(&m)
    );
    assert!(pbr.join(FILE).is_file(), "{}", notes(&m));
}

/// Note FW1: a failed PBR phase (here fed to the model as the worker's `AlsoFinished` does) skips the profile and the
/// note names the phase; so does an engine error in it.
#[test]
fn a_failed_pbr_phase_skips_the_profile_and_names_it() {
    use engine::also::Phase::*;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, Some(json!({"shelterFade": 40})));
    export(&mut m);
    let f = profile_path(&m);
    std::fs::remove_file(&f).unwrap();
    for error in [None, Some((Pbr, "boom".to_string(), 2))] {
        let phases = [Pbr, Vanilla]
            .into_iter()
            .map(|p| {
                let failed = u32::from(p == Pbr && error.is_none());
                (
                    p,
                    engine::api::BatchSummary {
                        failed,
                        ..Default::default()
                    },
                )
            })
            .collect();
        m.also_finished_with(engine::also::AlsoSummary {
            phases,
            copied: 0,
            sets: vec![],
            folders: Default::default(),
            notes: vec![],
            cancelled_after: None,
            error,
        });
        m.export.notes.clear();
        m.finish_xpmf(prepare(&m));
        assert!(!f.exists());
        assert!(
            notes(&m).contains("XPMF profiles not written: the PBR phase failed"),
            "{}",
            notes(&m)
        );
    }
}

fn set_block_pbr(m: &mut AppModel) {
    m.sets_doc
        .as_mut()
        .unwrap()
        .set_xpmf(PBR_ID, json!({"shelterFade": 40}).as_object().cloned());
    idle(m);
}
