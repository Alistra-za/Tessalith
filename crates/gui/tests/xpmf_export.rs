//! Note GI (XPMF Task 6, design X2, X5): XPMF profiles in a full export, in Review's What will be written, as stale
//! rows (FC11) and in the zip estimate (FC12).
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

/// FC11 and the Task 5 ruling: removing the last block lists the old profile as stale, with Delete while the file
/// still has Tessalith's hash.
#[test]
fn stale_profile_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, Some(json!({})));
    export(&mut m);
    let f = profile_path(&m);
    assert!(f.is_file(), "{}", notes(&m));
    assert!(!m.stale.iter().any(|s| s.path == f));
    set_block(&mut m, None);
    m.replan();
    idle(&mut m);
    assert!(
        m.stale.iter().any(|s| s.path == f && s.ours),
        "{:?}",
        m.stale
    );
    assert!(
        prepare(&m).is_some(),
        "a record with no block still reports"
    );
    std::fs::write(&f, "{}\n").unwrap();
    m.replan();
    idle(&mut m);
    let s = m.stale.iter().find(|s| s.path == f).expect("still stale");
    assert!(!s.ours, "edited by hand: no Delete");
    assert!(m.delete_stale_output(&f).is_err());
    assert!(f.is_file());
    // The export leaves it in place and says so.
    export(&mut m);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "{}\n");
    assert!(
        notes(&m).contains("no set writes this XPMF profile any more"),
        "{}",
        notes(&m)
    );
}

/// FC12: the zip estimate counts the profile.
#[test]
fn zip_counts_profile() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_also_settings(|c| c.zip = true);
    m.set_also_tick(AlsoTarget::Vanilla, true);
    idle(&mut m);
    assert!(m.also_on(), "{}", m.status_line);
    let before = m.zip_estimate().expect("zip on").entries;
    set_block(&mut m, Some(json!({})));
    idle(&mut m);
    assert_eq!(m.zip_estimate().unwrap().entries, before + 1);
}

/// A failed or cancelled export writes no profile.
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
