//! Note HQ: the export always has the `textures` folder. A request whose input root is at or inside a `textures`
//! folder plans under `<export>\<that textures folder down to the root>`; `rel` and set ids stay as they were.
use engine::api::*;
use engine::config::Target;
use engine::discover::effective_output_root;
use engine::events::*;
use image::{ImageBuffer, Rgb};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn png(p: &Path) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(16, 16, |x, y| Rgb([x as u8 * 8, y as u8 * 8, 90]))
        .save(p)
        .unwrap();
}

fn out_root(root: &Path, export: &Path) -> PathBuf {
    BatchRequest::new(vec![root.to_path_buf()], export.to_path_buf(), Target::Cs).output_root
}

/// HQ1: a root that is a `textures` folder, or lies inside one, plans under the export's own `textures\...`, and
/// discovery skips the whole export folder; a mod folder root keeps the export folder, as its `rel` has `textures`.
#[test]
fn a_textures_root_plans_under_the_exports_textures_folder() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");
    png(&d.path().join("Mod/textures/armor/x.png"));
    let tex = d.path().join("Mod/textures");
    assert_eq!(out_root(&tex, &out), out.join("textures"));
    let armor = tex.join("armor");
    assert_eq!(out_root(&armor, &out), out.join("textures").join("armor"));
    let r = BatchRequest::new(vec![armor], out.clone(), Target::Cs);
    assert_eq!(r.skip_root.as_deref(), Some(out.as_path()));
    assert_eq!(out_root(&d.path().join("Mod"), &out), out);
}

/// HQ1: the folder names keep their case from disk.
#[test]
fn the_folder_case_comes_from_disk() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");
    png(&d.path().join("Mod/Textures/Armor/x.png"));
    assert_eq!(
        out_root(&d.path().join("Mod/Textures/Armor"), &out),
        out.join("Textures").join("Armor")
    );
}

/// HQ2: no `textures` folder anywhere: nothing is added, and nothing extra is skipped.
#[test]
fn a_root_without_textures_is_unchanged() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");
    png(&d.path().join("src/x.png"));
    let r = BatchRequest::new(vec![d.path().join("src")], out.clone(), Target::Cs);
    assert_eq!((r.output_root, r.skip_root), (out, None));
}

/// HQ3 and HQ5: in place, or with the export folder an ancestor inside the root's own `textures` folders, the output
/// root is the input root, so a DDS lands beside its source.
#[test]
fn in_place_writes_beside_the_source() {
    let d = tempfile::tempdir().unwrap();
    png(&d.path().join("Mod/textures/armor/x.png"));
    let armor = d.path().join("Mod/textures/armor");
    let tex = d.path().join("Mod/textures");
    assert_eq!(effective_output_root(&armor, &armor), armor);
    assert_eq!(effective_output_root(&tex, &tex), tex);
    assert_eq!(effective_output_root(&armor, &tex), armor);
}

/// HQ1, HQ7: a planned texture lands at its game path under the export folder, and its set id is the same as before
/// (relative to the output root, without `textures`).
#[test]
fn outputs_move_and_set_ids_stay() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");
    let tex = d.path().join("Mod/textures");
    png(&tex.join("armor/iron.png"));
    let mut req = BatchRequest::new(vec![tex], out.clone(), Target::Cs);
    req.recursive = true;
    req.overrides = vec![engine::overrides::parse_set("file:**/iron.png.subtype=diffuse").unwrap()];
    let rep = Reporter::new(Arc::new(VecSink::default()), new_batch_id());
    let plan = Engine::new()
        .unwrap()
        .plan(req, &rep, &CancelToken::default())
        .unwrap();
    let outputs: Vec<&PathBuf> = plan.jobs.iter().map(|j| &j.output).collect();
    assert_eq!(outputs, vec![&out.join("textures/armor/iron.dds")]);
    let ids: Vec<String> = plan
        .shaders
        .keys()
        .map(|k| engine::sets::set_id(&plan.request.output_root, k))
        .collect();
    assert_eq!(ids, vec!["armor/iron".to_string()]);
}
