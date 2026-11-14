//! Note ID29: `Renderer::set_tree` switches the embedded shader tree at runtime. WARP, bundled shaders; programs are
//! only queued (async), never compiled: the test reads the job keys.
use render3d::cs_settings::{defaults_in, FEATURE_NAMES_B};
use render3d::device::Gpu;
use render3d::{ParsedNif, Renderer, ShaderSource, Tree};
use std::collections::BTreeSet;

#[path = "common/mod.rs"]
mod common;

fn keys(r: &mut Renderer) -> BTreeSet<String> {
    r.take_jobs()
        .iter()
        .map(|j| format!("{:?}", j.key))
        .collect()
}

#[test]
fn set_tree_recompiles_from_b() {
    let cache = tempfile::tempdir().unwrap();
    let mut r = Renderer::with_gpu_source(
        Gpu::new_warp().unwrap(),
        ShaderSource::Bundled,
        cache.path().to_path_buf(),
    )
    .unwrap();
    r.set_async_compile(true);
    let parsed = ParsedNif {
        shapes: vec![common::plane_shape(1, common::IDENT, [1.0; 3], 0)],
        ..Default::default()
    };
    r.load_parsed(&parsed, &|_| None);
    assert_eq!(r.tree(), Tree::A);
    let a = keys(&mut r);
    assert!(!a.is_empty());

    r.set_tree(Tree::B);
    assert_eq!(r.tree(), Tree::B);
    assert_eq!(r.settings().loaded, defaults_in(Tree::B).loaded);
    assert_eq!(
        r.settings().loaded,
        FEATURE_NAMES_B.iter().map(|s| s.to_string()).collect()
    );
    let b = keys(&mut r);
    assert!(!b.is_empty(), "the shapes were re-permuted");
    assert!(a.is_disjoint(&b), "tree B's programs key apart from A's");
    assert!(!r.features().iter().any(|f| f == "VANILLA_FRESNEL"));

    r.set_tree(Tree::A);
    assert_eq!(r.tree(), Tree::A);
    assert_eq!(keys(&mut r), a, "back to tree A: the old keys");
    assert!(r.features().iter().any(|f| f == "VANILLA_FRESNEL"));
}
