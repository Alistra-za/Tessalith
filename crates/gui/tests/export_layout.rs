//! Note HQ: a project opened at a `textures` folder plans under `<export>\textures`; set ids, `[files]` keys and
//! job keys stay as they were, and paths that leave the plan (JSON, game paths) are put back under the export folder.
mod support;
use std::path::{Path, PathBuf};
use support::*;

#[test]
fn a_textures_root_plans_under_the_exports_textures_folder() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("textures");
    let export = d.path().join("export");
    std::fs::create_dir_all(root.join("armor")).unwrap();
    image::ImageBuffer::from_fn(16, 16, |x, y| image::Rgb([x as u8 * 8, y as u8 * 8, 90]))
        .save(root.join("armor/iron.png"))
        .unwrap();
    let m = opened_as_the_app(root, export.clone());
    let req = m.request().unwrap();
    assert_eq!(req.output_root, export.join("textures"));
    assert_eq!(req.skip_root.as_deref(), Some(export.as_path()));
    assert_eq!(m.sets_root(), export.join("textures"));
    // HQ7: the set id is the one the project always had.
    let ids: Vec<String> = sdc_gui::model::sets::views(&m)
        .iter()
        .map(|v| v.id.clone())
        .collect();
    assert_eq!(ids, vec!["armor/iron".to_string()]);
    let it = m.item("armor/iron.png").unwrap();
    assert_eq!(it.job_key, "armor/iron.dds");
    assert_eq!(
        it.job.as_ref().unwrap().output,
        export.join("textures").join("armor").join("iron.dds")
    );
    // HQ6: the two directions between the plan and the export folder.
    assert_eq!(
        m.export_rel(Path::new("armor/iron.dds")),
        PathBuf::from("textures/armor/iron.dds")
    );
    assert_eq!(
        m.plan_rel(Path::new("Textures/pbr/armor/iron.dds")),
        Some(PathBuf::from("pbr/armor/iron.dds"))
    );
    assert_eq!(m.plan_rel(Path::new("meshes/iron.dds")), None);
    // The Meshes view's game paths now include a `textures` root's outputs.
    assert_eq!(
        m.output_game_paths().1,
        &["textures/armor/iron.dds".to_string()]
    );
}
