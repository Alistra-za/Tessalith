//! The pinned CS tree embedded in the crate (Note GT2): reads, closure, and equality with the folder it came from.
use render3d::includes::expand;
use render3d::source::SourceSet;
use render3d::ShaderSource;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const ENTRIES: [&str; 3] = [
    "Lighting.hlsl",
    "DeferredCompositeCS.hlsl",
    "DynamicCubemaps/SpecularIrradianceCS.hlsl",
];

fn unexpanded(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with('#') && t[1..].trim_start().starts_with("include")
        })
        .collect()
}

#[test]
fn every_entry_expands_from_the_bundle() {
    for e in ENTRIES {
        let x = expand(&ShaderSource::default(), e).unwrap();
        assert!(
            unexpanded(&x.text).is_empty(),
            "{e}: {:?}",
            unexpanded(&x.text)
        );
        assert!(x.files.len() > 1, "{e}");
    }
}

#[test]
fn the_bundle_holds_only_the_closure() {
    let mut reached = BTreeSet::new();
    for e in ENTRIES {
        for f in expand(&ShaderSource::Bundled, e).unwrap().files {
            reached.insert(f.to_string_lossy().to_ascii_lowercase());
        }
    }
    reached.insert("settingsdefault.json".to_string());
    let held: BTreeSet<String> = render3d::bundled_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(held, reached);
    assert_eq!(held.len(), 58);
}

#[test]
fn a_missing_file_is_an_error_naming_it() {
    let e = SourceSet::new(ShaderSource::Bundled)
        .get(Path::new("nope.hlsli"))
        .err()
        .unwrap();
    assert!(e.contains("nope.hlsli"), "{e}");
    // A missing include stays verbatim, so the compiler names it.
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("a.hlsl"), "#include \"nope.hlsli\"\n").unwrap();
    let x = expand(&ShaderSource::Folder(d.path().to_path_buf()), "a.hlsl").unwrap();
    assert!(x.text.contains("nope.hlsli"));
}

#[test]
fn bundled_expansion_equals_the_folder() {
    let Some(root) = std::env::var_os("SDC_CS_SHADERS").map(PathBuf::from) else {
        eprintln!("skipped: set SDC_CS_SHADERS");
        return;
    };
    let bundled = SourceSet::new(ShaderSource::Bundled);
    let folder = SourceSet::new(ShaderSource::Folder(root));
    for e in ENTRIES {
        let (b, f) = (
            bundled.get(Path::new(e)).unwrap(),
            folder.get(Path::new(e)).unwrap(),
        );
        assert!(b.text == f.text, "{e}: text differs");
        assert_eq!(b.sha256, f.sha256, "{e}");
        eprintln!("{e}: {} bytes, sha256 equal", b.text.len());
    }
}
