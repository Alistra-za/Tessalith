//! Generate's provenance and the JSON hashes are recorded at write time in `<project root>/_sdc/generated.json`
//! (Note FC11). The stale-outputs list these tests once shared a file with is gone (Note FM).
mod support;
use engine::shader::Shader;
use sdc_gui::model::from_normal::GenMap;
use sdc_gui::model::review::{DiskState, JsonKind};
use sdc_gui::model::AppModel;
use std::path::Path;
use std::time::Duration;
use support::*;

const WAIT: Duration = Duration::from_secs(120);
const ROCK_SET: &str = "landscape/rock01";

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(WAIT), "the model did not settle");
}
/// `pbr_opened_plain` with a relief normal, so Generate makes a height map; the shader pick is unsaved.
fn relief_rock(d: &Path) -> AppModel {
    let mut m = pbr_opened_plain(d);
    relief_normal(&d.join("textures/landscape"), "rock01_normal.png", 64);
    m.replan();
    idle(&mut m);
    m
}

/// CO39/CO51, superseded: Generate records its provenance at write time, so closing without Save keeps the
/// generated file "generated here".
#[test]
fn generate_then_close_without_save_is_still_ours() {
    let d = tempfile::tempdir().unwrap();
    let mut m = relief_rock(d.path());
    assert!(
        m.sets_doc.as_ref().unwrap().is_dirty(),
        "the unsaved shader pick"
    );
    m.open_gen(ROCK_SET, Some(GenMap::Height));
    m.gen_generate();
    idle(&mut m);
    assert!(d
        .path()
        .join("textures/landscape/rock01_p_generated.png")
        .is_file());
    let root = d.path().join("textures");
    // Close without Save: a new session opens the folder.
    drop(m);
    let mut m = opened_as_the_app(root, d.path().join("export"));
    m.set_shader(ROCK_SET, Some(Shader::Pbr));
    idle(&mut m);
    assert!(
        m.provenance("landscape/rock01_p_generated.png").is_some(),
        "the provenance came back from the sidecar"
    );
    assert_eq!(
        m.gen_offer(ROCK_SET, GenMap::Height),
        Ok(()),
        "generated here"
    );
}

/// BD2/P21, superseded: the JSON hash is recorded at write time, so discarding the unsaved edits after an export
/// does not make the next export ask about Tessalith's own JSON.
#[test]
fn json_hash_survives_a_discard() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    export_through(&mut m);
    idle(&mut m);
    let json = pgp_json_path(&m);
    assert!(json.is_file());
    // BD2: the hash record lived only in the unsaved document; strip it from sdc-project.toml, which is what a
    // discarded edit leaves on disk (deterministic: no race with the running export).
    let toml = d.path().join("textures/sdc-project.toml");
    let stripped: String = std::fs::read_to_string(&toml)
        .unwrap()
        .lines()
        .filter(|l| !l.contains("json_sha256"))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(&toml, stripped).unwrap();
    let state = |m: &mut AppModel| {
        m.reload_documents();
        idle(m);
        assert!(
            !m.sets_doc.as_ref().unwrap().is_dirty(),
            "reconciling the sidecar on open does not dirty the document"
        );
        m.review_json()
            .into_iter()
            .find(|l| l.kind == JsonKind::PgPatcher)
            .expect("the PGPatcher JSON line")
            .state
    };
    assert_eq!(
        state(&mut m),
        DiskState::Ours,
        "the next export does not ask"
    );
    // Control: the quiet reload wrote nothing back to the toml, so without the sidecar the record is gone and the
    // same JSON is no longer Tessalith's.
    assert!(!std::fs::read_to_string(&toml)
        .unwrap()
        .contains("json_sha256"));
    std::fs::remove_file(d.path().join("textures/_sdc/generated.json")).unwrap();
    assert_ne!(state(&mut m), DiskState::Ours);
}

/// Note FC11 (owner): the sidecar wins over a hand edit of the same key in sdc-project.toml. Reconciling on open is
/// quiet (no unsaved changes, Discard leaves a clean document); the next real Save writes the readable copy.
#[test]
fn the_sidecar_wins_and_reconcile_is_quiet() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    idle(&mut m);
    m.save().unwrap();
    idle(&mut m);
    let root = d.path().join("textures");
    let toml = root.join("sdc-project.toml");
    let text = std::fs::read_to_string(&toml).unwrap();
    std::fs::write(
        &toml,
        format!("{text}\n[project.cs_pbr_sha256]\n\"a.json\" = \"hand\"\n"),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("_sdc")).unwrap();
    std::fs::write(
        root.join("_sdc/generated.json"),
        r#"{"cs_pbr_sha256":{"a.json":"side"}}"#,
    )
    .unwrap();
    m.reload_documents();
    idle(&mut m);
    let doc = m.sets_doc.as_ref().unwrap();
    assert_eq!(
        doc.cs_pbr_shas().get("a.json").map(String::as_str),
        Some("side")
    );
    assert!(!doc.is_dirty(), "open reconciles quietly");
    m.reload_documents();
    idle(&mut m);
    assert!(
        !m.sets_doc.as_ref().unwrap().is_dirty(),
        "Discard leaves a clean document"
    );
    assert!(std::fs::read_to_string(&toml).unwrap().contains("hand"));
    // A real edit, then Save: the readable copy follows the sidecar.
    let order: Vec<String> = vec![ROCK_SET.into()];
    m.sets_doc.as_mut().unwrap().set_order(&order);
    assert!(m.sets_doc.as_ref().unwrap().is_dirty());
    m.save().unwrap();
    idle(&mut m);
    let saved = std::fs::read_to_string(&toml).unwrap();
    assert!(
        saved.contains("\"side\"") && !saved.contains("\"hand\""),
        "{saved}"
    );
}
