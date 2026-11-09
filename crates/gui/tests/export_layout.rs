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
    // HQ6: a plan path as a path under the export folder.
    assert_eq!(
        m.export_rel(Path::new("armor/iron.dds")),
        PathBuf::from("textures/armor/iron.dds")
    );
    assert_eq!(m.export_key("Armor\\Iron.dds"), "textures/armor/iron.dds");
    // The Meshes view's game paths now include a `textures` root's outputs.
    assert_eq!(
        m.output_game_paths().1,
        &["textures/armor/iron.dds".to_string()]
    );
}

/// HQ6 (fix round 1): a PGPatcher PBR set in a root inside `textures` still writes under `<export>\textures\pbr\`,
/// since explicit paths are under the export folder, not the moved output root; its set id is unchanged.
#[test]
fn a_pbr_set_in_a_root_inside_textures_writes_under_textures_pbr() {
    use engine::config::{GreenConvention, Subtype};
    use sdc_gui::model::Answer;
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("Mod/textures/armor");
    let export = d.path().join("export");
    std::fs::create_dir_all(&root).unwrap();
    image::ImageBuffer::from_fn(16, 16, |x, y| image::Rgb([x as u8 * 8, y as u8 * 8, 90]))
        .save(root.join("iron.png"))
        .unwrap();
    image::ImageBuffer::from_fn(16, 16, |_, _| image::Rgb([128u8, 128, 255]))
        .save(root.join("iron_n.png"))
        .unwrap();
    let mut m = opened_as_the_app(root, export.clone());
    m.decide("iron.png", Answer::Subtype(Subtype::PbrAlbedo));
    m.decide("iron_n.png", Answer::Subtype(Subtype::PbrNormal));
    m.decide("iron_n.png", Answer::Green(GreenConvention::Directx));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert_eq!(m.sets_root(), export.join("textures").join("armor"));
    let views = sdc_gui::model::sets::views(&m);
    let v = views.iter().find(|v| v.pbr).expect("a PBR set");
    assert_eq!(v.id, "iron");
    let prefix = v.prefix.clone().expect("a prefix");
    assert!(prefix.starts_with(r"textures\pbr\"), "{prefix}");
    let want = export.join(format!("{}.dds", prefix.replace('\\', "/")));
    let got = m
        .item("iron.png")
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .output
        .clone();
    assert_eq!(got, want);
    export_through(&mut m);
    assert!(m.wait_idle(std::time::Duration::from_secs(120)));
    assert_eq!(m.export.summary.as_ref().unwrap().failed, 0);
    assert!(want.is_file(), "{want:?}");
}
