//! A fuzz map follows the set's shader (plan 2026-11-17-fuzz-follows-shader): what a `_fuzz` or `_sheen` image is
//! and where it is written.
use engine::api::*;
use engine::codes::Code;
use engine::config::{Subtype, Target};
use engine::events::*;
use engine::group::{split_vocab, VocabRole};
use engine::overrides::parse_set;
use engine::pack::ChannelSource;
use engine::policy::{AcceptSuggestions, ResolvedJob};
use engine::presets::Provenance;
use engine::shader::Shader;
use image::{ImageBuffer, Luma, Rgb, Rgba};
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

/// Every job's output file name, sorted.
fn out_names(p: &Plan) -> Vec<String> {
    let mut v: Vec<String> = p.jobs.iter().map(out_name).collect();
    v.sort();
    v
}

fn packed<'a>(p: &'a Plan, name: &str) -> Option<&'a ResolvedJob> {
    p.jobs.iter().find(|j| {
        j.pack.is_some()
            && j.output
                .file_name()
                .is_some_and(|n| n.to_string_lossy() == name)
    })
}

fn g(j: &ResolvedJob) -> &ChannelSource {
    &j.pack.as_ref().unwrap().channels[1]
}

/// The shader picked for the set of `rock01.png`.
fn shader(p: &Plan) -> Shader {
    p.shaders[&job(p, "rock01.png").set_key].shader
}

fn codes_of(j: &ResolvedJob) -> Vec<Code> {
    j.warnings.iter().map(|w| w.code).collect()
}

/// `in/<rel>` as an RGBA colour image (the probe's `open_fuzz`).
fn rgba(dir: &Path, rel: &str) {
    let p = dir.join("in").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| {
        Rgba([(x * 4) as u8, 200, (y * 4) as u8, (x * 2 + y) as u8])
    })
    .save(p)
    .unwrap();
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

/// Fuzz-1, Fuzz-2, Fuzz-18: with other PBR evidence (a raw roughness map) the fuzz map is `pbr_fuzz` decided by the
/// set (never a suggestion), `<base>_f.dds`, and the set packs `_rmaos`; the diffuse is `pbr_albedo`. Both terms,
/// both accept modes.
#[test]
fn a_fuzz_map_with_pbr_evidence_is_pbr_fuzz_and_the_set_packs_rmaos() {
    for term in ["fuzz", "sheen"] {
        for accept in [AcceptSuggestions::None, AcceptSuggestions::High] {
            let name = format!("rock01_{term}.png");
            let d = fixture(&["rock01.png", "rock01_roughness.png", &name], "");
            let p = plan_with(&d, accept, &[], &[]);
            assert!(
                packed(&p, "rock01_rmaos.dds").is_some(),
                "{term} {accept:?}: {:?}",
                out_names(&p)
            );
            assert!(
                packed(&p, "rock01_rfaos.dds").is_none(),
                "{term} {accept:?}"
            );
            let f = job(&p, &name);
            assert_eq!(f.profile.subtype, Subtype::PbrFuzz);
            // `ResolvedJob::provenance` is a map by field name; `Provenance` is `engine::presets::Provenance`.
            assert_eq!(
                f.provenance.get("subtype"),
                Some(&Provenance::Set),
                "{term} {accept:?}"
            );
            assert_eq!(
                f.output.file_name().unwrap().to_string_lossy(),
                "rock01_f.dds"
            );
            assert_eq!(f.suggestions[0].reason, engine::suggest::FUZZ_PBR_REASON);
            assert_eq!(job(&p, "rock01.png").profile.subtype, Subtype::PbrAlbedo);
            assert_eq!(shader(&p), Shader::Pbr);
        }
    }
}

/// Fuzz-2: an RGBA fuzz map under PBR keeps its colour: a colour `pbr_fuzz`, sRGB-tagged, no CHAN-NOT-GRAY.
#[test]
fn an_rgba_fuzz_map_under_pbr_keeps_its_colour() {
    let d = fixture(&["rock01.png", "rock01_roughness.png"], "");
    rgba(d.path(), "rock01_fuzz.png");
    let p = plan(&d);
    let f = job(&p, "rock01_fuzz.png");
    assert_eq!(f.profile.subtype, Subtype::PbrFuzz);
    assert!(f.profile.srgb_tag());
    assert!(
        !codes_of(f).contains(&Code::ChanNotGray),
        "{:?}",
        f.warnings
    );
}

/// Fuzz-5: other skin evidence keeps the set Advanced Skin and the fuzz map the `_rfaos` G mask (DR2). Both terms.
#[test]
fn other_skin_evidence_keeps_the_fuzz_map_a_skin_mask() {
    for term in ["fuzz", "sheen"] {
        let name = format!("rock01_{term}.png");
        for (extra, toml) in [
            (
                None,
                "[files.\"rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
            ),
            (
                None,
                "[files.\"rock01_rmaos\".packing]\nrecipe = \"rfaos\"\n",
            ),
            (None, "[files.\"rock01.png\"]\nsubtype = \"skin_albedo\"\n"),
            (
                Some("rock01_sk.png"),
                "[files.\"rock01_sk.png\"]\nsubtype = \"skin_sk\"\n",
            ),
        ] {
            let mut files = vec!["rock01.png", "rock01_roughness.png", name.as_str()];
            files.extend(extra);
            let d = fixture(&files, toml);
            let p = plan(&d);
            let j = packed(&p, "rock01_rfaos.dds")
                .unwrap_or_else(|| panic!("{term} {toml}: {:?}", out_names(&p)));
            match g(j) {
                ChannelSource::Source { path, .. } => {
                    assert!(path.ends_with(&name), "{term} {toml}: {path:?}")
                }
                other => panic!("{term} {toml}: G is {other:?}"),
            }
            assert!(
                !out_names(&p).contains(&"rock01_f.dds".to_string()),
                "{term} {toml}"
            );
            assert_eq!(shader(&p), Shader::AdvancedSkin, "{term} {toml}");
        }
    }
}

/// Fuzz-2 with DR2: in a skin set an RGBA fuzz map is still a greyscale mask: R packs and CHAN-NOT-GRAY stays.
#[test]
fn an_rgba_fuzz_map_in_a_skin_set_still_raises_chan_not_gray() {
    for term in ["fuzz", "sheen"] {
        let d = fixture(
            &["rock01.png", "rock01_roughness.png"],
            "[files.\"rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
        );
        rgba(d.path(), &format!("rock01_{term}.png"));
        let p = plan(&d);
        let j = packed(&p, "rock01_rfaos.dds").expect("_rfaos");
        let all: Vec<Code> = p
            .jobs
            .iter()
            .flat_map(codes_of)
            .chain(p.skipped.iter().filter_map(|s| s.code))
            .collect();
        assert!(all.contains(&Code::ChanNotGray), "{term}: {all:?}");
        assert!(matches!(g(j), ChannelSource::Source { .. }), "{term}");
    }
}

/// Planner's call (spec §3d): two fuzz maps in one PBR-read set would both be `<base>_f.dds`; one writes it and the
/// writer clash check (BV2) reports the other as IN-DUPLICATE in `skipped`.
#[test]
fn two_fuzz_maps_in_a_pbr_set_clash_once() {
    let d = fixture(
        &[
            "rock01.png",
            "rock01_roughness.png",
            "rock01_fuzz.png",
            "rock01_sheen.png",
        ],
        "",
    );
    let p = plan(&d);
    assert_eq!(
        out_names(&p)
            .iter()
            .filter(|n| *n == "rock01_f.dds")
            .count(),
        1,
        "{:?}",
        out_names(&p)
    );
    let dup = p.skipped.iter().any(|s| s.code == Some(Code::InDuplicate));
    assert!(dup, "{:?}", p.skipped);
}

/// The new evidence is scoped to fuzz-map sets: a metallic-only set with a written `rfaos` still builds `_rmaos`
/// and is not skin (DQ2, probe P1e).
#[test]
fn a_written_rfaos_on_a_metallic_only_set_is_not_skin() {
    let d = fixture(
        &["rock01.png", "rock01_metallic.png"],
        "[files.\"rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
    );
    let p = plan(&d);
    assert!(
        packed(&p, "rock01_rmaos.dds").is_some(),
        "{:?}",
        out_names(&p)
    );
    assert_ne!(job(&p, "rock01.png").profile.subtype, Subtype::SkinAlbedo);
}

/// Fuzz-1: a written `rmaos` is not skin evidence either way; the fuzz map is the PBR fuzz texture beside `_rmaos`.
#[test]
fn a_written_rmaos_keeps_the_fuzz_map_pbr() {
    let d = fixture(
        &["rock01.png", "rock01_roughness.png", "rock01_fuzz.png"],
        "[files.\"rock01_rmaos\".packing]\nrecipe = \"rmaos\"\n",
    );
    let p = plan(&d);
    assert!(packed(&p, "rock01_rmaos.dds").is_some());
    assert_eq!(
        job(&p, "rock01_fuzz.png")
            .output
            .file_name()
            .unwrap()
            .to_string_lossy(),
        "rock01_f.dds"
    );
}
