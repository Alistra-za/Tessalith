//! render3d must build without the engine and without the encoders (Note HX6): other programs use it.
use std::collections::{BTreeMap, BTreeSet};

/// Names of every package in render3d's normal and build dependency closure (dev dependencies not followed).
fn normal_closure(root: &str) -> BTreeSet<String> {
    let out = std::process::Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--offline",
            "--manifest-path",
        ])
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"))
        .output()
        .expect("run cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).expect("metadata json");

    let mut names: BTreeMap<String, String> = BTreeMap::new();
    for p in meta["packages"].as_array().expect("packages") {
        names.insert(
            p["id"].as_str().unwrap().to_owned(),
            p["name"].as_str().unwrap().to_owned(),
        );
    }
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in meta["resolve"]["nodes"].as_array().expect("resolve.nodes") {
        let follow: Vec<String> = node["deps"]
            .as_array()
            .expect("deps")
            .iter()
            .filter(|d| {
                d["dep_kinds"]
                    .as_array()
                    .expect("dep_kinds")
                    .iter()
                    .any(|k| k["kind"].is_null() || k["kind"] == "build")
            })
            .map(|d| d["pkg"].as_str().unwrap().to_owned())
            .collect();
        edges.insert(node["id"].as_str().unwrap().to_owned(), follow);
    }
    let start = names
        .iter()
        .find(|(_, n)| n.as_str() == root)
        .map(|(id, _)| id.clone())
        .expect("root package");
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut todo = vec![start];
    while let Some(id) = todo.pop() {
        if seen.insert(id.clone()) {
            todo.extend(edges.get(&id).cloned().unwrap_or_default());
        }
    }
    seen.iter().map(|id| names[id].clone()).collect()
}

#[test]
fn render3d_normal_dependencies_exclude_engine_and_encoders() {
    let closure = normal_closure("render3d");
    for banned in ["engine", "encoders-sys"] {
        assert!(
            !closure.contains(banned),
            "render3d depends on `{banned}` again: use texcore, or move what you need into it (Note HX6)"
        );
    }
    assert!(closure.contains("texcore"));
}

/// `cargo metadata` unifies features across the workspace, where the engine turns texcore's `load` on. `cargo tree
/// -p render3d` resolves them for render3d alone, which is how another program builds it.
#[test]
fn render3d_alone_builds_without_the_image_crate() {
    let out = std::process::Command::new(env!("CARGO"))
        .args([
            "tree",
            "--offline",
            "-p",
            "render3d",
            "-e",
            "normal",
            "--prefix",
            "none",
            "--manifest-path",
        ])
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"))
        .output()
        .expect("run cargo tree");
    assert!(
        out.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8_lossy(&out.stdout);
    for banned in ["image", "bytemuck", "engine", "encoders-sys"] {
        assert!(
            !tree.lines().any(|l| l.split(' ').next() == Some(banned)),
            "render3d alone pulls in `{banned}`: texcore's `load` feature must stay off for it (Note HX6)"
        );
    }
}
