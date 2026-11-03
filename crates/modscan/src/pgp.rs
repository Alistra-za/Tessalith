//! PGPatcher (ParallaxGen) output as a derived layer (plan 05 §5). Detection by marker files and the Diff's keys.
use nif::path::canon_mesh;
use std::collections::HashSet;
use std::path::Path;

pub fn running(names: &[String]) -> bool {
    names
        .iter()
        .any(|n| n == "pgpatcher.exe" || n == "pgtools.exe")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signature {
    None,
    Incomplete,
    Medium,
    Strong,
}

fn is_pg_esp(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n == "pgpatcher.esp"
        || n.strip_prefix("pg_")
            .and_then(|r| r.strip_suffix(".esp"))
            .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

pub fn signature(root: &Path) -> Signature {
    let names: Vec<String> = std::fs::read_dir(root)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    let diff = names
        .iter()
        .any(|n| n.eq_ignore_ascii_case("ParallaxGen_Diff.json"));
    // PGPatcher-named plugins only: an arbitrary .esp next to meshes\ is an ordinary mod (owner: 265 false
    // "incomplete" mods with any-ESP; controller ruling on Task 10).
    let esp = names.iter().any(|n| is_pg_esp(n));
    let cache = names.iter().any(|n| {
        n.to_ascii_lowercase()
            .starts_with("pgpatcher_updatecache.bin")
    });
    let dups = root.join("meshes").join("_pgpatcher_dups").is_dir();
    let meshes = root.join("meshes").is_dir();
    if diff && esp {
        Signature::Strong
    } else if meshes && !diff && (esp || cache) && !dups {
        Signature::Incomplete
    } else if diff || esp || dups {
        Signature::Medium
    } else {
        Signature::None
    }
}

/// The paths ParallaxGen_Diff.json names (canonical mesh keys): the files PGPatcher wrote.
#[derive(Debug, Default)]
pub struct Diff {
    pub keys: HashSet<String>,
}

pub fn read_diff(root: &Path) -> Option<Diff> {
    let text = std::fs::read_to_string(root.join("ParallaxGen_Diff.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(Diff {
        keys: v.as_object()?.keys().map(|k| canon_mesh(k)).collect(),
    })
}

pub fn dup_source(norm: &str) -> Option<String> {
    let rest = norm.strip_prefix("meshes/_pgpatcher_dups/")?;
    let (_, tail) = rest.split_once('/')?;
    Some(format!("meshes/{tail}"))
}

pub fn is_derived_path(norm: &str, diff: Option<&Diff>) -> bool {
    dup_source(norm).is_some() || diff.is_some_and(|d| d.keys.contains(norm))
}
