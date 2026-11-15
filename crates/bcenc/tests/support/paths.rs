//! The code paths a test runs `bcenc`'s hidden `_on` entries on. Shared by `crates/bcenc/tests` and the crate's unit
//! tests (`path::tested_paths`, through `#[path]`).
#![allow(dead_code)]

use bcenc::Path;
use std::sync::OnceLock;

/// The paths this machine runs, in `Path::ALL` order. The first call in a test binary prints one line to stderr:
/// "bcenc paths: ran base, avx2; skipped avx512 (CPU lacks avx512bw)". With SDC_BCENC_PATHS=<name,...> set, panics
/// when a named path is not available, or with "unknown path <name>" when no path has that name.
pub fn paths() -> Vec<Path> {
    // A panic leaves the cell empty, so every later call panics too: no test runs on fewer paths than named.
    static RAN: OnceLock<Vec<Path>> = OnceLock::new();
    RAN.get_or_init(|| {
        if let Ok(names) = std::env::var("SDC_BCENC_PATHS") {
            demand(&names);
        }
        let ran: Vec<Path> = Path::ALL.into_iter().filter(|p| p.available()).collect();
        let names: Vec<&str> = ran.iter().map(|p| p.name()).collect();
        let skipped: Vec<String> = skipped()
            .iter()
            .map(|(p, why)| format!("{} ({why})", p.name()))
            .collect();
        let skipped = if skipped.is_empty() {
            "none".to_string()
        } else {
            skipped.join(", ")
        };
        eprintln!("bcenc paths: ran {}; skipped {skipped}", names.join(", "));
        ran
    })
    .clone()
}

/// The paths this machine does not run, each with why (`Path::missing`), in `Path::ALL` order.
pub fn skipped() -> Vec<(Path, &'static str)> {
    Path::ALL
        .into_iter()
        .filter_map(|p| p.missing().map(|why| (p, why)))
        .collect()
}

/// Panics unless every path named in the comma-separated `names` exists and is available.
fn demand(names: &str) {
    for name in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        let Some(p) = Path::ALL.into_iter().find(|p| p.name() == name) else {
            panic!("SDC_BCENC_PATHS: unknown path {name}");
        };
        if let Some(why) = p.missing() {
            panic!("SDC_BCENC_PATHS: path {name} is not available: {why}");
        }
    }
}
