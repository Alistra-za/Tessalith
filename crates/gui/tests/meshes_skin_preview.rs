//! Note GN: the Meshes view's Advanced Skin card (preview-only RFAOS / wetness picks and a wet amount slider).
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use modscan::store::{ModRow, NifRow, ShapeView};
use sdc_gui::model::meshes::skin_preview::{SkinMap, SkinPick};
use sdc_gui::model::meshes::MeshesModel;
use sdc_gui::model::viewport3d::{FileSource, MeshSpec};
use std::collections::HashMap;
use std::sync::Arc;

fn harness(m: MeshesModel) -> Harness<'static, MeshesModel> {
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
            m,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

fn shape(block: u32, name: &str, shader_type: u32) -> ShapeView {
    ShapeView {
        block,
        name: Some(name.into()),
        kind: "BSTriShape".into(),
        shader_kind: 1,
        shader_type: Some(shader_type),
        slsf1: Some(0x8000_0101),
        slsf2: Some(0x21),
        textures: vec![
            Some("textures/a/body.dds".into()),
            Some("textures/a/body_msn.dds".into()),
        ],
        raw: vec![None; 2],
        alpha_flags: None,
        alpha_threshold: None,
        params: None,
        skinned: true,
        vertex_desc: None,
        num_vertices: Some(120),
        num_triangles: Some(80),
        index3d: Some(block),
    }
}

/// One LE mesh (nothing loads into the 3D view) with a default, a FaceGen and a FaceGen RGB tint shape.
fn model() -> MeshesModel {
    let mut m = MeshesModel::empty();
    m.mods = vec![ModRow {
        id: 1,
        name: "A".into(),
        kind: 1,
        priority: 0,
        derived: false,
        nifs: 1,
    }];
    m.selected_mod = Some(1);
    m.nifs = Arc::new(vec![NifRow {
        copy_id: 7,
        path: "meshes/x.nif".into(),
        wins: true,
        overridden_by: None,
        n_copies: 1,
        derived: false,
        le_mesh: true,
        problem: None,
        patched_in_game: false,
    }]);
    m.selected_nif = Some(7);
    m.shapes = vec![
        shape(3, "Hair", 0),
        shape(4, "Head", 4),
        shape(5, "Body", 5),
    ];
    m.shapes_col = true;
    m
}

#[test]
fn the_card_is_under_face_tint_and_skin_tint_shapes_only() {
    let mut h = harness(model());
    h.run();
    assert_eq!(
        h.query_all_by_label("Advanced Skin preview").count(),
        2,
        "types 4 and 5, not type 0"
    );
    h.get_by_label("Wet amount of block 4");
    h.get_by_label("Wet amount of block 5");
    assert!(h.query_by_label("Wet amount of block 3").is_none());
    h.get_by_label("Browse for the RFAOS of block 5");
    h.get_by_label("Browse for the Wetness map of block 4");
    // Auto until something is picked: no "back to Auto" button.
    assert!(h.query_by_label("RFAOS of block 5: back to Auto").is_none());
}

#[test]
fn a_project_pick_shows_and_x_goes_back_to_auto() {
    let dir = tempfile::tempdir().unwrap();
    let tex = dir.path().join("textures").join("a");
    std::fs::create_dir_all(&tex).unwrap();
    std::fs::write(tex.join("body_wet.DDS"), b"dds").unwrap();
    std::fs::write(tex.join("body.png"), b"png").unwrap();
    let mut m = model();
    m.nif_export_root = Some(dir.path().to_path_buf());
    assert_eq!(m.project_textures(), ["textures/a/body_wet.DDS"]);
    m.set_skin_map(
        5,
        SkinMap::Wetness,
        Some(SkinPick::Project("textures/a/body_wet.DDS".into())),
    )
    .unwrap();
    let e = m.skin_entry(5).unwrap();
    assert_eq!(
        e.preview.wet_map.as_deref(),
        Some("project:textures/a/body_wet.DDS")
    );
    assert_eq!(
        e.sources,
        [(
            "project:textures/a/body_wet.DDS".to_string(),
            FileSource::Loose(tex.join("body_wet.DDS"))
        )]
    );
    let mut h = harness(m);
    h.run();
    h.get_by_label("Wetness map of block 5: textures/a/body_wet.DDS");
    h.get_by_label("Wetness map of block 5: back to Auto")
        .click();
    h.run();
    assert!(
        h.state().skin_entry(5).is_none(),
        "Auto and dry: the shape as loaded"
    );
    h.get_by_label("Wetness map of block 5: Auto");
}

#[test]
fn a_mod_pick_not_in_the_scan_changes_nothing() {
    let mut m = model();
    let e = m.set_skin_map(
        4,
        SkinMap::Rfaos,
        Some(SkinPick::Mod(r"a\head_rfaos.dds".into())),
    );
    assert!(e.is_err());
    assert!(m.skin_entry(4).is_none());
}

/// The render thread applies the picks to the loaded shape, applies them again after a reload of the same model, and
/// drops them for another model (WARP, stand-in shaders).
#[test]
fn the_render_thread_applies_the_picks_and_keeps_them_across_a_reload() {
    use nif::material::{MaterialState, SkinPreview};
    use render3d::ShapeId;
    use sdc_gui::model::viewport3d::{Content, SkinEntry, Viewport3d};
    use std::time::{Duration, Instant};
    let standin = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../render3d/tests/fixtures/standin"
    ));
    let f = render3d::locate::fingerprint(&standin, None).unwrap();
    let cache = tempfile::tempdir().unwrap();
    let mut v = Viewport3d::new(Some(f.clone()));
    v.on_located(Ok(Some(f)));
    v.cache_dir = Some(cache.path().to_path_buf());
    let mut mat = MaterialState::builtin_default();
    mat.shader_type = 5;
    // DO_ALPHA_TEST: the stand-in's pixel permutation compiles.
    mat.alpha = Some(nif::props::Alpha {
        flags: 0x200,
        threshold: 0,
    });
    let content = |key: u64| {
        Some(Content::Builtin {
            key,
            shape: render3d::builtin::Builtin::Sphere,
            material: mat.clone(),
            textures: Arc::new(HashMap::new()),
        })
    };
    let wait = |v: &mut Viewport3d, wet: f32| {
        let t = Instant::now();
        while v.shapes.first().map(|s| s.material.skin().wet) != Some(wet) {
            v.poll();
            v.request_frame(64, 64);
            assert!(t.elapsed().as_secs() < 60, "stuck, wanted wet {wet}");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    v.set_content(content(1), false);
    wait(&mut v, 0.0);
    let entry = SkinEntry {
        preview: SkinPreview {
            wet: 0.5,
            ..Default::default()
        },
        sources: vec![],
    };
    v.set_skin_preview(ShapeId(0), Some(entry));
    wait(&mut v, 0.5);
    // The same model reloaded (an edit's new bytes): the picks are applied again.
    v.set_content(content(2), true);
    v.request_frame(64, 64);
    v.shapes.clear();
    wait(&mut v, 0.5);
    // Another model: none.
    v.set_content(content(3), false);
    assert!(v.skin_preview(ShapeId(0)).is_none());
    wait(&mut v, 0.0);
}

#[test]
fn the_wet_amount_is_kept_per_shape_and_another_mesh_resets_every_pick() {
    let mut m = model();
    m.set_skin_wet(4, 0.5);
    m.set_skin_wet(5, 2.0);
    assert_eq!(m.skin_entry(4).unwrap().preview.wet, 0.5);
    assert_eq!(m.skin_entry(5).unwrap().preview.wet, 1.0, "clamped");
    m.set_skin_wet(4, 0.0);
    assert!(m.skin_entry(4).is_none(), "back to the shape as loaded");
    m.v3.set_mesh(Some(MeshSpec {
        key: 8,
        nif: FileSource::Loose("other.nif".into()),
        textures: HashMap::new(),
        le_mesh: false,
        head_part: false,
        txst: Default::default(),
    }));
    assert!(m.skin_entry(5).is_none(), "picks reset with another mesh");
}
