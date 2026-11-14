//! Note ID30: `Renderer::set_tree` switches the embedded shader tree at runtime. WARP, bundled shaders; programs are
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
fn render_set_tree_recompiles_from_b() {
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

    // With the features off no define is left to tell the trees apart: the key still covers the tree (the expanded
    // source).
    r.set_features_on(false);
    let off_a = keys(&mut r);
    r.set_tree(Tree::B);
    let off_b = keys(&mut r);
    assert!(!off_a.is_empty() && !off_b.is_empty());
    assert!(off_a.is_disjoint(&off_b), "the tree is in the key itself");
}

/// A renderer made on tree B starts with tree B's settings, as one switched to B does (Note ID32).
#[test]
fn a_renderer_made_on_tree_b_seeds_tree_b_settings() {
    let cache = tempfile::tempdir().unwrap();
    let r = Renderer::with_gpu_source(
        Gpu::new_warp().unwrap(),
        ShaderSource::BundledB,
        cache.path().to_path_buf(),
    )
    .unwrap();
    assert_eq!(r.tree(), Tree::B);
    assert_eq!(r.settings().loaded, defaults_in(Tree::B).loaded);
    assert!(!r.features().iter().any(|f| f == "VANILLA_FRESNEL"));
    assert!(r.features().iter().any(|f| f == "ENABLE_LL"));
}
