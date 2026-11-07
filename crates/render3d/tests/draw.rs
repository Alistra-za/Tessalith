use render3d::scene::Scene;
use render3d::textures::{default_for_register, register_for_slot};

#[test]
fn slot_register_tables() {
    assert_eq!(register_for_slot(0, false, false, 0), Some(0));
    assert_eq!(register_for_slot(2, false, false, 2), Some(6));
    assert_eq!(register_for_slot(5, false, true, 2), Some(12));
    assert_eq!(register_for_slot(5, false, true, 7), Some(2));
    assert_eq!(register_for_slot(1, false, false, 4), Some(4));
    assert_eq!(register_for_slot(11, false, false, 6), Some(8));
    assert_eq!(register_for_slot(0, true, false, 5), Some(5));
    assert_eq!(register_for_slot(0, true, false, 6), Some(9));
    assert_eq!(register_for_slot(0, true, false, 7), Some(12));
    assert_eq!(default_for_register(1), [128, 128, 255, 255]);
    // Capture-confirmed rows (frame11178, see register_for_slot docs).
    assert_eq!(register_for_slot(4, false, false, 2), Some(12)); // Arch_Head _sk
    assert_eq!(register_for_slot(6, false, false, 7), Some(9)); // Lashes lash_flow
    assert_eq!(register_for_slot(0, true, false, 3), Some(4)); // Moisture m_p
    assert_eq!(register_for_slot(0, true, false, 4), None);
}

fn shaders() -> Option<std::path::PathBuf> {
    std::env::var("SDC_CS_SHADERS").ok().map(Into::into)
}

fn lit(px: &[u8]) -> usize {
    px.chunks(4)
        .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30)
        .count()
}

/// Needs the installed CS: `SDC_CS_SHADERS=... cargo test -p render3d -- --ignored`
#[test]
#[ignore]
fn renders_fixture_nonblack_with_defaults() {
    let root = shaders().unwrap();
    let fp = render3d::ShaderSource::Folder(root.to_path_buf());
    let cache = tempfile::tempdir().unwrap();
    let mut r = render3d::Renderer::with_source(fp, cache.path().to_path_buf()).unwrap();
    let nif = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nif/tests/fixtures/TestNifFile_Furniture_Col_SE.nif"
    ))
    .unwrap();
    let rep = r.load(&nif, &|_| None);
    assert!(rep.compile_errors.is_empty(), "{:?}", rep.compile_errors);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert!(rep.shapes > 0);
    let px = r.render(&Scene::neutral(), 256, 256).unwrap();
    let lit = lit(&px);
    assert!(
        lit > 256 * 256 / 50,
        "object should cover >2% of the frame and be lit, got {lit} px"
    );
}

#[test]
#[ignore]
fn missing_textures_use_defaults() {
    let root = shaders().unwrap();
    let fp = render3d::ShaderSource::Folder(root.to_path_buf());
    let cache = tempfile::tempdir().unwrap();
    let mut r = render3d::Renderer::with_source(fp, cache.path().to_path_buf()).unwrap();
    let nif = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nif/tests/fixtures/TestNifFile_Skinned_SE.nif"
    ))
    .unwrap();
    let rep = r.load(&nif, &|_| None);
    assert!(!rep.missing_textures.is_empty());
    assert!(rep.compile_errors.is_empty(), "{:?}", rep.compile_errors);
    assert!(rep.shapes > 0, "{rep:?}");
    let px = r.render(&Scene::neutral(), 128, 128).unwrap();
    assert!(lit(&px) > 0, "skinned shape with default textures renders");
}

#[test]
#[ignore]
fn le_mesh_is_skipped() {
    let root = shaders().unwrap();
    let fp = render3d::ShaderSource::Folder(root);
    let mut r = render3d::Renderer::with_source(fp, tempfile::tempdir().unwrap().keep()).unwrap();
    let nif = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nif/tests/fixtures/TestNifFile_Animated_LE.nif"
    ))
    .unwrap();
    let rep = r.load(&nif, &|_| None);
    assert!(rep.le_mesh);
    assert_eq!(rep.shapes, 0);
}

/// B8: the display encode brightens the same frame, and the timing is filled in.
#[test]
#[ignore]
fn srgb_readback_and_timing() {
    let root = shaders().unwrap();
    let fp = render3d::ShaderSource::Folder(root.to_path_buf());
    let cache = tempfile::tempdir().unwrap();
    let mut r = render3d::Renderer::with_source(fp.clone(), cache.path().to_path_buf()).unwrap();
    let nif = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nif/tests/fixtures/TestNifFile_Furniture_Col_SE.nif"
    ))
    .unwrap();
    assert!(r.load(&nif, &|_| None).shapes > 0);
    let linear = r.render(&Scene::neutral(), 128, 128).unwrap();
    r.set_encode_srgb(true);
    let srgb = r.render(&Scene::neutral(), 128, 128).unwrap();
    let sum = |px: &[u8]| px.iter().map(|&v| v as u64).sum::<u64>();
    assert!(sum(&srgb) > sum(&linear));
    let t = r.last_timing();
    assert!(
        t.draw_ms >= 0.0 && t.gpu_ms >= 0.0 && t.readback_ms > 0.0,
        "{t:?}"
    );
}

/// Note GT: `Renderer::new` compiles the CS tree embedded in the exe. No `SDC_CS_SHADERS` and no MO2 variable is
/// read (run it with them unset: the constructor takes no folder), and the furniture's centre is lit.
#[test]
fn render_bundled_tree_draws_with_no_cs() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = render3d::Renderer::new(cache.path().to_path_buf()).unwrap();
    let nif = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../nif/tests/fixtures/TestNifFile_Furniture_Col_SE.nif"
    ))
    .unwrap();
    let rep = r.load(&nif, &|_| None);
    assert!(rep.compile_errors.is_empty(), "{:?}", rep.compile_errors);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert!(rep.shapes > 0);
    let px = r.render(&Scene::neutral(), 256, 256).unwrap();
    // The furniture has a gap at the exact centre pixel: the central 64x64 block must be lit, a third of it here.
    let block = (96..160usize)
        .flat_map(|y| (96..160usize).map(move |x| (y * 256 + x) * 4))
        .filter(|&i| px[i..i + 3].iter().map(|&v| v as u32).sum::<u32>() > 30)
        .count();
    assert!(
        block > 64 * 64 / 10,
        "the centre is lit, got {block} of 4096"
    );
    assert!(lit(&px) > 256 * 256 / 50);
}
