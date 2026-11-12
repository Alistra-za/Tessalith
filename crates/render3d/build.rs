//! Embeds `shaders/cs` (the pinned Community Shaders closure, Note GT) as a table sorted by lowercased path.
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, root, out);
        } else {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy();
            out.push((rel.replace('\\', "/").to_ascii_lowercase(), p));
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("shaders").join("cs");
    println!("cargo:rerun-if-changed=shaders/cs");
    println!("cargo:rerun-if-changed=build.rs");
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    files.sort();
    let mut src = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (key, path) in &files {
        let abs = path.to_string_lossy().replace('\\', "/");
        writeln!(src, "    ({key:?}, include_bytes!({abs:?})),").unwrap();
    }
    src.push_str("];\n");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("bundled.rs");
    std::fs::write(out, src).unwrap();
}
