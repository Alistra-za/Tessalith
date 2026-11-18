//! A fuzz map follows the set's shader (plan 2026-11-17-fuzz-follows-shader): what a `_fuzz` or `_sheen` image is
//! and where it is written.
use engine::api::*;
use engine::codes::Code;
use engine::config::{Subtype, Target};
use engine::events::*;
use engine::group::{split_vocab, VocabRole};
use engine::overrides::parse_set;
use engine::policy::{AcceptSuggestions, ResolvedJob};
use engine::shader::Shader;
use image::{ImageBuffer, Luma, Rgb};
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

/// `in/<rel>`: grey (`Luma`) when its name ends in a raw-map term, RGB colour otherwise.
fn put(dir: &Path, rel: &str) {
    let p = dir.join("in").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    let stem = Path::new(rel).file_stem().unwrap().to_string_lossy();
    if matches!(split_vocab(&stem), Some((_, v)) if matches!(v.role, VocabRole::PackingInput(_))) {
        ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
            .save(p)
            .unwrap();
    } else {
        ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
            .save(p)
            .unwrap();
    }
}

/// `in/` holding `files`: names with a raw-map term are grey (`Luma`), others RGB colour; then `in/sdc.toml`.
fn fixture(files: &[&str], toml: &str) -> TempDir {
    let d = tempfile::tempdir().unwrap();
    for f in files {
        put(d.path(), f);
    }
    std::fs::write(d.path().join("in/sdc.toml"), toml).unwrap();
    d
}

fn plan_with(
    d: &TempDir,
    accept: AcceptSuggestions,
    shaders: &[(&str, Shader)],
    sets: &[&str],
) -> Plan {
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
    r.recursive = true;
    r.accept = accept;
    r.set_shaders = shaders.iter().map(|(k, s)| (k.to_string(), *s)).collect();
    r.overrides = sets.iter().map(|s| parse_set(s).unwrap()).collect();
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap()
}

fn plan(d: &TempDir) -> Plan {
    plan_with(d, AcceptSuggestions::None, &[], &[])
}

/// The own (non-packed) job of `file`.
fn job<'a>(p: &'a Plan, file: &str) -> &'a ResolvedJob {
    p.jobs
        .iter()
        .find(|j| {
            j.pack.is_none()
                && j.input
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy() == file)
        })
        .unwrap_or_else(|| panic!("no job for {file}"))
}

fn out_name(j: &ResolvedJob) -> String {
    j.output.file_name().unwrap().to_string_lossy().into_owned()
}

/// Fuzz-10: a fuzz map typed `pbr_fuzz` is the PBR fuzz texture and is written as `<base>_f.dds`.
#[test]
fn a_fuzz_map_typed_pbr_fuzz_writes_base_f() {
    for term in ["fuzz", "sheen"] {
        let name = format!("rock01_{term}.png");
        let d = fixture(
            &["rock01.png", "rock01_roughness.png", &name],
            &format!("[files.\"{name}\"]\nsubtype = \"pbr_fuzz\"\n"),
        );
        let p = plan(&d);
        let j = job(&p, &name);
        assert_eq!(j.profile.subtype, Subtype::PbrFuzz, "{term}");
        assert_eq!(out_name(j), "rock01_f.dds", "{term}");
    }
}

/// Planner's call (Review Focus 1): beside an `_f` the typed fuzz map keeps its own name; nothing clashes.
#[test]
fn a_typed_fuzz_map_beside_an_f_keeps_its_own_name() {
    let d = fixture(
        &["rock01.png", "rock01_f.png", "rock01_fuzz.png"],
        "[files.\"rock01_fuzz.png\"]\nsubtype = \"pbr_fuzz\"\n",
    );
    let p = plan(&d);
    assert_eq!(out_name(job(&p, "rock01_fuzz.png")), "rock01_fuzz.dds");
    assert_eq!(out_name(job(&p, "rock01_f.png")), "rock01_f.dds");
    assert!(
        p.skipped.iter().all(|s| s.code != Some(Code::InDuplicate)),
        "{:?}",
        p.skipped
    );
}
