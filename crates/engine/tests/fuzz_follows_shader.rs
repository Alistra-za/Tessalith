//! A fuzz map follows the set's shader (plan 2026-11-17-fuzz-follows-shader): what a `_fuzz` or `_sheen` image is
//! and where it is written.
use engine::api::*;
use engine::codes::Code;
use engine::config::{Subtype, Target};
use engine::events::*;
use engine::group::{split_vocab, VocabRole};
use engine::overrides::parse_set;
use engine::pack::ChannelSource;
use engine::policy::{AcceptSuggestions, JobStatus, ResolvedJob};
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

/// The `PACK-UNUSED-INPUT` message of the raw map `file`, wherever the plan keeps it.
fn unused_reason(p: &Plan, file: &str) -> String {
    let has = |path: &Path| {
        path.file_name()
            .is_some_and(|n| n.to_string_lossy() == file)
    };
    for j in p.jobs.iter().filter(|j| has(&j.input)) {
        if let Some(w) = j.warnings.iter().find(|w| w.code == Code::PackUnusedInput) {
            return w.message.clone();
        }
    }
    for s in p.skipped.iter().filter(|s| has(&s.path)) {
        if s.code == Some(Code::PackUnusedInput) {
            return s.reason.clone();
        }
    }
    panic!("{file} is not reported unused");
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

/// Fuzz-4: a PBR set holding both an `_f` and a fuzz map keeps the `_f`; the fuzz map is listed unused.
#[test]
fn an_f_beside_a_fuzz_map_wins_under_pbr() {
    let d = fixture(
        &[
            "iron.png",
            "iron_roughness.png",
            "iron_ao.png",
            "iron_fuzz.png",
            "iron_f.png",
        ],
        "",
    );
    let p = plan(&d);
    assert!(
        packed(&p, "iron_rmaos.dds").is_some(),
        "{:?}",
        out_names(&p)
    );
    assert_eq!(job(&p, "iron_f.png").profile.subtype, Subtype::PbrFuzz);
    assert_eq!(
        unused_reason(&p, "iron_fuzz.png"),
        "iron_fuzz.png: iron_f.png is the set's PBR fuzz texture (_f); not converted"
    );
    assert_eq!(
        out_names(&p).iter().filter(|n| *n == "iron_f.dds").count(),
        1
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

/// Fuzz-1 (fix round 1): a real `_rfaos` image is skin evidence beside a fuzz map, as without one: the set is
/// Advanced Skin, its diffuse `skin_albedo`, and the fuzz map the skin mask (no `pbr_fuzz`, no `_f.dds`). Which data
/// wins is unchanged (`pack::plan::plan_packing`, `packing.prefer = prepacked`): the pre-packed `_rfaos` wins over the
/// raw maps it would be built from, so it carries PACK-CONFLICT, the roughness and fuzz maps PACK-UNUSED-INPUT, and
/// nothing is packed (no `_rmaos` beside it). Both terms.
#[test]
fn a_real_rfaos_beside_a_fuzz_map_keeps_the_set_skin() {
    for term in ["fuzz", "sheen"] {
        let fuzz = format!("rock01_{term}.png");
        let d = fixture(
            &[
                "rock01.png",
                "rock01_rfaos.png",
                "rock01_roughness.png",
                &fuzz,
            ],
            "",
        );
        let p = plan(&d);
        assert_eq!(shader(&p), Shader::AdvancedSkin, "{term}");
        assert_eq!(
            job(&p, "rock01.png").profile.subtype,
            Subtype::SkinAlbedo,
            "{term}"
        );
        let f = job(&p, &fuzz);
        assert_ne!(f.profile.subtype, Subtype::PbrFuzz, "{term}");
        assert!(
            codes_of(f).contains(&Code::PackUnusedInput),
            "{term}: {:?}",
            codes_of(f)
        );
        assert!(
            !out_names(&p).contains(&"rock01_f.dds".to_string()),
            "{term}: {:?}",
            out_names(&p)
        );
        assert!(
            p.jobs.iter().all(|j| j.pack.is_none()),
            "{term}: {:?}",
            out_names(&p)
        );
        assert!(
            codes_of(job(&p, "rock01_rfaos.png")).contains(&Code::PackConflict),
            "{term}"
        );
        assert!(
            codes_of(job(&p, "rock01_roughness.png")).contains(&Code::PackUnusedInput),
            "{term}"
        );
    }
    // The same evidence without a fuzz map (unchanged): skin, with a `skin_albedo` diffuse.
    let d = fixture(
        &["rock01.png", "rock01_rfaos.png", "rock01_roughness.png"],
        "",
    );
    let p = plan(&d);
    assert_eq!(shader(&p), Shader::AdvancedSkin);
    assert_eq!(job(&p, "rock01.png").profile.subtype, Subtype::SkinAlbedo);
}

/// The mirror of the above (fix round 1): a real `_rmaos` image is no skin evidence, so beside a fuzz map and a raw
/// roughness map the set reads PBR, the fuzz map is `pbr_fuzz` written `rock01_f.dds`, and no `_rfaos` is built; the
/// pre-packed `_rmaos` wins over the roughness map (PACK-CONFLICT, PACK-UNUSED-INPUT).
#[test]
fn a_real_rmaos_beside_a_fuzz_map_reads_pbr() {
    let d = fixture(
        &[
            "rock01.png",
            "rock01_rmaos.png",
            "rock01_roughness.png",
            "rock01_fuzz.png",
        ],
        "",
    );
    let p = plan(&d);
    assert_eq!(shader(&p), Shader::Pbr);
    let f = job(&p, "rock01_fuzz.png");
    assert_eq!(f.profile.subtype, Subtype::PbrFuzz);
    assert_eq!(out_name(f), "rock01_f.dds");
    assert!(
        packed(&p, "rock01_rfaos.dds").is_none(),
        "{:?}",
        out_names(&p)
    );
    assert!(codes_of(job(&p, "rock01_rmaos.png")).contains(&Code::PackConflict));
    assert!(codes_of(job(&p, "rock01_roughness.png")).contains(&Code::PackUnusedInput));
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

/// Fuzz-2, Review Focus 5: an explicit Advanced Skin (GUI pick or `--set set:`) with nothing written reads the
/// fuzz map as the `_rfaos` G mask.
#[test]
fn an_explicit_advanced_skin_reads_the_fuzz_map_as_the_mask() {
    let files = ["rock01.png", "rock01_roughness.png", "rock01_fuzz.png"];
    for (shaders, sets) in [
        (&[("rock01", Shader::AdvancedSkin)][..], &[][..]),
        (&[][..], &["set:rock01.shader=advanced_skin"][..]),
    ] {
        let d = fixture(&files, "");
        let p = plan_with(&d, AcceptSuggestions::None, shaders, sets);
        let j = packed(&p, "rock01_rfaos.dds")
            .unwrap_or_else(|| panic!("{sets:?}: {:?}", out_names(&p)));
        assert!(
            matches!(g(j), ChannelSource::Source { path, .. } if path.ends_with("rock01_fuzz.png"))
        );
        assert!(!out_names(&p).contains(&"rock01_f.dds".to_string()));
        assert_eq!(job(&p, "rock01.png").profile.subtype, Subtype::SkinAlbedo);
    }
}

/// Fuzz-2, Fuzz-9 (engine half): an explicit PBR-family pick reads the fuzz map as `pbr_fuzz` even when Tessalith
/// recorded `skin_albedo` before (that value stays; the set packs `_rmaos` from the roughness).
#[test]
fn an_explicit_pbr_reads_the_fuzz_map_as_pbr_fuzz() {
    for sh in [Shader::Pbr, Shader::PbrGrass, Shader::PbrLandscape] {
        let d = fixture(
            &["rock01.png", "rock01_roughness.png", "rock01_fuzz.png"],
            "[files.\"rock01.png\"]\nsubtype = \"skin_albedo\"\n",
        );
        let p = plan_with(&d, AcceptSuggestions::None, &[("rock01", sh)], &[]);
        let f = job(&p, "rock01_fuzz.png");
        assert_eq!(f.profile.subtype, Subtype::PbrFuzz, "{sh:?}");
        assert_eq!(
            f.output.file_name().unwrap().to_string_lossy(),
            "rock01_f.dds",
            "{sh:?}"
        );
        assert!(
            packed(&p, "rock01_rmaos.dds").is_some(),
            "{sh:?}: {:?}",
            out_names(&p)
        );
    }
}

/// Fuzz-20 and A1-7: a written recipe wins over an explicit shader. A written `rfaos` under explicit PBR keeps
/// `_rfaos` (G at its default when a fuzz map exists, which is `pbr_fuzz`); a written `rmaos` under explicit Advanced
/// Skin keeps `_rmaos`. With and without a fuzz map.
#[test]
fn a_written_recipe_wins_over_an_explicit_shader() {
    // rfaos + explicit PBR, with a fuzz map.
    let d = fixture(
        &["rock01.png", "rock01_roughness.png", "rock01_fuzz.png"],
        "[files.\"rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
    );
    let p = plan_with(&d, AcceptSuggestions::None, &[("rock01", Shader::Pbr)], &[]);
    let j = packed(&p, "rock01_rfaos.dds").unwrap_or_else(|| panic!("{:?}", out_names(&p)));
    assert_eq!(*g(j), ChannelSource::Default { value: 0.0 });
    assert_eq!(
        job(&p, "rock01_fuzz.png")
            .output
            .file_name()
            .unwrap()
            .to_string_lossy(),
        "rock01_f.dds"
    );
    // rfaos + explicit PBR, no fuzz map (Plan 3b P1c: roughness and sk).
    let d = fixture(
        &["rock01.png", "rock01_roughness.png", "rock01_sk.png"],
        "[files.\"rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
    );
    let p = plan_with(&d, AcceptSuggestions::None, &[("rock01", Shader::Pbr)], &[]);
    assert!(
        packed(&p, "rock01_rfaos.dds").is_some(),
        "{:?}",
        out_names(&p)
    );
    assert!(
        packed(&p, "rock01_rmaos.dds").is_none(),
        "{:?}",
        out_names(&p)
    );
    // rmaos + explicit Advanced Skin, with and without a fuzz map.
    for extra in [Some("rock01_fuzz.png"), None] {
        let mut files = vec!["rock01.png", "rock01_roughness.png", "rock01_sk.png"];
        files.extend(extra);
        let d = fixture(
            &files,
            "[files.\"rock01_rmaos\".packing]\nrecipe = \"rmaos\"\n",
        );
        let p = plan_with(
            &d,
            AcceptSuggestions::None,
            &[("rock01", Shader::AdvancedSkin)],
            &[],
        );
        assert!(
            packed(&p, "rock01_rmaos.dds").is_some(),
            "{extra:?}: {:?}",
            out_names(&p)
        );
        assert!(packed(&p, "rock01_rfaos.dds").is_none(), "{extra:?}");
        assert!(
            !out_names(&p).contains(&"rock01_f.dds".to_string()),
            "{extra:?}"
        );
    }
}

/// Fuzz-3: an explicit other shader leaves the fuzz map unused, with the shader named. (The automatic row, a `_g`
/// picking Glow, is `the_fuzz_map_does_not_outvote_other_maps`.)
#[test]
fn other_shaders_leave_the_fuzz_map_unused() {
    let d = fixture(&["rock01.png", "rock01_n.png", "rock01_sheen.png"], "");
    let p = plan_with(
        &d,
        AcceptSuggestions::None,
        &[("rock01", Shader::Default)],
        &[],
    );
    assert_eq!(
        unused_reason(&p, "rock01_sheen.png"),
        "rock01_sheen.png: the default shader reads no fuzz map; not converted"
    );
    // An unused raw map keeps its `PackingInput` job (not converted), as every PACK-UNUSED-INPUT map does.
    let converted: Vec<String> = p
        .jobs
        .iter()
        .filter(|j| !matches!(j.status, JobStatus::PackingInput { .. }))
        .map(out_name)
        .collect();
    assert!(
        !converted
            .iter()
            .any(|n| n.starts_with("rock01_f") || n.starts_with("rock01_sheen")),
        "{converted:?}"
    );
}

/// Spec §3b: under the Unused reading an RGBA fuzz map is not read at all: no CHAN-NOT-GRAY anywhere.
#[test]
fn an_rgba_fuzz_map_under_another_shader_is_not_read() {
    let d = fixture(&["rock01.png", "rock01_n.png"], "");
    rgba(d.path(), "rock01_fuzz.png");
    let p = plan_with(
        &d,
        AcceptSuggestions::None,
        &[("rock01", Shader::Default)],
        &[],
    );
    assert_eq!(
        unused_reason(&p, "rock01_fuzz.png"),
        "rock01_fuzz.png: the default shader reads no fuzz map; not converted"
    );
    let all: Vec<Code> = p
        .jobs
        .iter()
        .flat_map(codes_of)
        .chain(p.skipped.iter().filter_map(|s| s.code))
        .collect();
    assert!(!all.contains(&Code::ChanNotGray), "{all:?}");
}

/// Spec §3a, Fuzz-22: a hand-set `helper.role = "fuzz"` keeps today's meaning under every shader: the `_rfaos` G
/// source. It is skin evidence, so under Auto the set stays Advanced Skin with a `skin_albedo` diffuse (DY).
#[test]
fn a_hand_set_fuzz_role_stays_the_skin_mask_under_every_shader() {
    // `helper_roles.rs`'s `files()` form: the key without extension.
    let toml = "[files.\"rock01_fuzz\"]\nhelper.role = \"fuzz\"\n";
    for shaders in [
        &[][..],
        &[("rock01", Shader::Pbr)][..],
        &[("rock01", Shader::Glow)][..],
        &[("rock01", Shader::AdvancedSkin)][..],
    ] {
        let d = fixture(
            &["rock01.png", "rock01_roughness.png", "rock01_fuzz.png"],
            toml,
        );
        let p = plan_with(&d, AcceptSuggestions::None, shaders, &[]);
        let j = packed(&p, "rock01_rfaos.dds")
            .unwrap_or_else(|| panic!("{shaders:?}: {:?}", out_names(&p)));
        assert!(
            matches!(g(j), ChannelSource::Source { path, .. } if path.ends_with("rock01_fuzz.png")),
            "{shaders:?}"
        );
        assert!(
            !out_names(&p).contains(&"rock01_f.dds".to_string()),
            "{shaders:?}"
        );
        if shaders.is_empty() {
            // The Auto row (Fuzz-22): the shader and the diffuse type, not only G.
            assert_eq!(shader(&p), Shader::AdvancedSkin);
            assert_eq!(job(&p, "rock01.png").profile.subtype, Subtype::SkinAlbedo);
        }
    }
}

/// Fuzz-22 before `auto`'s suffix prediction: the set joins are decided before any `_rfaos` is predicted, so there
/// the hand-set fuzz role is the only skin evidence. With it the set is Advanced Skin when `x_wet` joins (Final review
/// I3): `rock01_wet` is the set's skin `_wet` member.
#[test]
fn a_hand_set_fuzz_role_joins_the_skin_wet_map() {
    let d = fixture(
        &[
            "rock01.png",
            "rock01_roughness.png",
            "rock01_fuzz.png",
            "rock01_wet.png",
        ],
        "[files.\"rock01_fuzz\"]\nhelper.role = \"fuzz\"\n",
    );
    let p = plan(&d);
    assert_eq!(
        job(&p, "rock01_wet.png").set_key,
        job(&p, "rock01.png").set_key
    );
    assert_eq!(job(&p, "rock01_wet.png").profile.subtype, Subtype::SkinWet);
}

/// Spec §3a: a hand-set `helper.role = "none"` makes the fuzz map an ordinary file under every shader: converted
/// alone under its own name, never renamed, never in a packed map, never listed unused. Under Advanced Skin the set
/// still packs `_rfaos` from its roughness (DV1), with G at its default: the file is not the mask.
#[test]
fn a_hand_set_none_role_is_an_ordinary_file_under_every_shader() {
    let toml = "[files.\"rock01_fuzz\"]\nhelper.role = \"none\"\n";
    for (shaders, rfaos) in [
        (&[][..], false),
        (&[("rock01", Shader::Pbr)][..], false),
        (&[("rock01", Shader::Glow)][..], false),
        (&[("rock01", Shader::AdvancedSkin)][..], true),
    ] {
        let d = fixture(
            &["rock01.png", "rock01_roughness.png", "rock01_fuzz.png"],
            toml,
        );
        let p = plan_with(&d, AcceptSuggestions::None, shaders, &[]);
        assert_eq!(
            job(&p, "rock01_fuzz.png")
                .output
                .file_name()
                .unwrap()
                .to_string_lossy(),
            "rock01_fuzz.dds",
            "{shaders:?}"
        );
        match packed(&p, "rock01_rfaos.dds") {
            Some(j) => {
                assert!(rfaos, "{shaders:?}: {:?}", out_names(&p));
                assert_eq!(*g(j), ChannelSource::Default { value: 0.0 }, "{shaders:?}");
            }
            None => assert!(!rfaos, "{shaders:?}: {:?}", out_names(&p)),
        }
    }
}

/// Note HH1: `a/rock`'s fuzz map shared into `b/stone`; each set has a roughness map, so it reads PBR under Auto.
/// Planned under `shaders`.
fn plan_shared(shaders: &[(&str, Shader)]) -> Plan {
    let d = fixture(
        &[
            "textures/a/rock.png",
            "textures/a/rock_roughness.png",
            "textures/a/rock_fuzz.png",
            "textures/b/stone.png",
            "textures/b/stone_roughness.png",
        ],
        "",
    );
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
    r.recursive = true;
    r.set_shaders = shaders.iter().map(|(k, s)| (k.to_string(), *s)).collect();
    r.set_edits = vec![engine::sets::SetEdit::Share {
        file: "textures/a/rock_fuzz.png".into(),
        to: "textures/b/stone".into(),
    }];
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap()
}

/// The shared fuzz map's own job: (output name, sub-type, codes).
fn shared_fuzz_job(p: &Plan) -> Option<(String, Subtype, Vec<Code>)> {
    p.jobs
        .iter()
        .find(|j| j.pack.is_none() && j.input.ends_with("rock_fuzz.png"))
        .map(|j| (out_name(j), j.profile.subtype, codes_of(j)))
}

/// Fuzz-2, Fuzz-3 with Note HH1: each set reads a shared fuzz map by its own shader. Lender Glow and borrower explicit
/// Advanced Skin: the borrower's `_rfaos` G reads it and it is consumed (used by one set, so not unused). Lender Glow
/// and borrower Auto (PBR from its roughness): unused, and the reason names both sets, since the borrower's shader
/// does read fuzz maps; the borrower packs `_rmaos`.
#[test]
fn a_shared_fuzz_map_is_consumed_by_a_skin_borrower_and_unused_otherwise() {
    let p = plan_shared(&[
        ("textures/a/rock", Shader::Glow),
        ("textures/b/stone", Shader::AdvancedSkin),
    ]);
    let j = packed(&p, "stone_rfaos.dds").unwrap_or_else(|| panic!("{:?}", out_names(&p)));
    assert!(
        matches!(g(j), ChannelSource::Source { path, .. } if path.ends_with("rock_fuzz.png")),
        "{:?}",
        g(j)
    );
    assert_eq!(shared_fuzz_job(&p), None);
    let p = plan_shared(&[("textures/a/rock", Shader::Glow)]);
    assert_eq!(
        unused_reason(&p, "rock_fuzz.png"),
        "rock_fuzz.png: the glow shader of set textures/a/rock reads no fuzz map, and set textures/b/stone (pbr \
         shader) would read it as its fuzz texture, which only the file's own set can convert; not converted"
    );
    assert!(
        packed(&p, "stone_rmaos.dds").is_some(),
        "{:?}",
        out_names(&p)
    );
}

/// Known limit (Fuzz-2 with Note HH1; `pack::plan::plan_packing`'s `shortcut:`): a file has one job, so a shared
/// fuzz map its own set converts (lender Auto, so PBR: `rock_f.dds`) is no packing source of a borrower's `_rfaos`
/// (explicit Advanced Skin). The borrower's G takes its default and its PACK-DEFAULTED says why, naming the lender
/// set and its shader. The mirror (lender explicit Advanced Skin packs it into its own `_rfaos`, borrower Auto reads
/// PBR): no set writes a fuzz texture, and the borrower packs `_rmaos` without it.
#[test]
fn a_shared_fuzz_map_its_own_set_converts_is_no_borrowers_mask_known_limit() {
    let p = plan_shared(&[("textures/b/stone", Shader::AdvancedSkin)]);
    assert_eq!(
        shared_fuzz_job(&p),
        Some(("rock_f.dds".to_string(), Subtype::PbrFuzz, vec![]))
    );
    let j = packed(&p, "stone_rfaos.dds").unwrap_or_else(|| panic!("{:?}", out_names(&p)));
    assert_eq!(*g(j), ChannelSource::Default { value: 0.0 });
    let w = j
        .warnings
        .iter()
        .find(|w| w.code == Code::PackDefaulted && w.message.contains(" G: "))
        .expect("the G PACK-DEFAULTED");
    let why = "the shared rock_fuzz.png is converted by its own set textures/a/rock (pbr shader) and is not read \
               here";
    assert_eq!(
        w.message,
        format!("stone_rfaos.dds G: no fuzz source; neutral default 0; {why}")
    );
    assert_eq!(w.data["why"], why);
    // The mirror.
    let p = plan_shared(&[("textures/a/rock", Shader::AdvancedSkin)]);
    let j = packed(&p, "rock_rfaos.dds").unwrap_or_else(|| panic!("{:?}", out_names(&p)));
    assert!(
        matches!(g(j), ChannelSource::Source { path, .. } if path.ends_with("rock_fuzz.png")),
        "{:?}",
        g(j)
    );
    assert_eq!(shared_fuzz_job(&p), None);
    assert!(
        packed(&p, "stone_rmaos.dds").is_some(),
        "{:?}",
        out_names(&p)
    );
    assert!(
        !out_names(&p).iter().any(|n| n.ends_with("_f.dds")),
        "{:?}",
        out_names(&p)
    );
}

/// Review t3 V1: a set with skin evidence (a recorded `skin_albedo` diffuse) predicts an `_rfaos` before any pick
/// (`api::predict_suffixes`). Under an explicit `shader` that does not read the fuzz map as the mask, no `_rfaos` is
/// built, and neither the set's `set_resolved` (recipes, SHADER-* warnings) nor any job's warning names one.
fn no_rfaos_is_named(shader: Shader) {
    let d = fixture(
        &["rock01.png", "rock01_roughness.png", "rock01_fuzz.png"],
        "[files.\"rock01.png\"]\nsubtype = \"skin_albedo\"\n",
    );
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
    r.recursive = true;
    r.set_shaders = [("rock01".to_string(), shader)].into_iter().collect();
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "t");
    let p = Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap();
    assert!(packed(&p, "rock01_rfaos.dds").is_none(), "{shader:?}");
    let key = job(&p, "rock01.png").set_key.clone();
    let resolved: Vec<_> = sink
        .take()
        .into_iter()
        .filter(|e| e.set.as_deref() == Some(key.as_str()))
        .filter_map(|e| match e.kind {
            EventKind::SetResolved {
                recipes, warnings, ..
            } => Some((recipes, warnings)),
            _ => None,
        })
        .collect();
    assert_eq!(resolved.len(), 1, "{shader:?}");
    let (recipes, warnings) = &resolved[0];
    assert!(
        recipes.iter().all(|r| !r.output.contains("rfaos")),
        "{shader:?}: {recipes:?}"
    );
    let named: Vec<&str> = warnings
        .iter()
        .chain(p.jobs.iter().flat_map(|j| &j.warnings))
        .map(|w| w.message.as_str())
        .filter(|m| m.contains("rfaos"))
        .collect();
    assert!(named.is_empty(), "{shader:?}: {named:?}");
}

#[test]
fn an_explicit_pbr_on_a_skin_set_names_no_rfaos() {
    no_rfaos_is_named(Shader::Pbr);
}

#[test]
fn another_explicit_shader_on_a_skin_set_names_no_rfaos() {
    no_rfaos_is_named(Shader::Glow);
}

/// The `plan_packing` gate also takes a fuzz map that failed to load: its name still gives it role Fuzz there, and it
/// must not make `auto` pack `_rfaos` in a set that does not read it as the skin mask.
#[test]
fn a_fuzz_map_that_fails_to_load_packs_no_rfaos() {
    let d = fixture(&["rock01.png", "rock01_roughness.png"], "");
    std::fs::write(d.path().join("in/rock01_fuzz.png"), b"not a png").unwrap();
    let p = plan(&d);
    assert!(
        packed(&p, "rock01_rfaos.dds").is_none(),
        "{:?}",
        out_names(&p)
    );
    assert!(
        packed(&p, "rock01_rmaos.dds").is_some(),
        "{:?}",
        out_names(&p)
    );
}

/// Known limit (Fuzz-11, spec §6.1), pinned so it shows: the CLI cannot see an explicit non-PBR shader that is not
/// written to `sdc.toml`, so with the same files and no shader it plans Fuzz-1a and converts the fuzz map.
#[test]
fn without_the_explicit_shader_the_cli_converts_the_fuzz_map() {
    let d = fixture(&["rock01.png", "rock01_n.png", "rock01_sheen.png"], "");
    let p = plan(&d);
    assert_eq!(
        job(&p, "rock01_sheen.png")
            .output
            .file_name()
            .unwrap()
            .to_string_lossy(),
        "rock01_f.dds"
    );
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

/// Fuzz-1a: diffuse + normal + fuzz map, nothing else: PBR; the fuzz map is the PBR fuzz texture; no packed output.
#[test]
fn a_fuzz_map_and_nothing_else_picks_pbr() {
    for term in ["fuzz", "sheen"] {
        for accept in [AcceptSuggestions::None, AcceptSuggestions::High] {
            let name = format!("rock01_{term}.png");
            let d = fixture(&["rock01.png", "rock01_n.png", &name], "");
            let p = plan_with(&d, accept, &[], &[]);
            assert_eq!(shader(&p), Shader::Pbr, "{term} {accept:?}");
            assert_eq!(
                job(&p, "rock01.png").profile.subtype,
                Subtype::PbrAlbedo,
                "{term} {accept:?}"
            );
            assert_eq!(
                job(&p, "rock01_n.png").profile.subtype,
                Subtype::PbrNormal,
                "{term} {accept:?}"
            );
            assert_eq!(out_name(job(&p, &name)), "rock01_f.dds");
            assert!(
                p.jobs.iter().all(|j| j.pack.is_none()),
                "{:?}",
                out_names(&p)
            );
        }
    }
}

/// Fuzz-1a, Fuzz-3: the fuzz map is the lowest evidence. A `_g` picks Glow with or without it; the `_g` stays
/// `glow`, and the fuzz map is listed unused with the automatic shader named.
#[test]
fn the_fuzz_map_does_not_outvote_other_maps() {
    let d = fixture(&["rock01.png", "rock01_g.png", "rock01_fuzz.png"], "");
    let p = plan(&d);
    assert_eq!(shader(&p), Shader::Glow);
    assert_eq!(job(&p, "rock01_g.png").profile.subtype, Subtype::Glow);
    assert_eq!(job(&p, "rock01.png").profile.subtype, Subtype::Diffuse);
    assert_eq!(
        unused_reason(&p, "rock01_fuzz.png"),
        "rock01_fuzz.png: the glow shader reads no fuzz map; not converted"
    );
    // The unused map keeps its `PackingInput` job (`rock01_fuzz.dds`, never written), so only converted jobs count.
    let converted: Vec<String> = p
        .jobs
        .iter()
        .filter(|j| !matches!(j.status, JobStatus::PackingInput { .. }))
        .map(out_name)
        .collect();
    assert!(
        !converted.iter().any(|n| n.starts_with("rock01_f")),
        "{converted:?}"
    );
}

/// `plan` with mesh votes (`BatchRequest::mesh_shaders`, keyed by `shader::mesh_key`), as `shaders.rs`'s
/// `plan_with` passes them.
fn plan_mesh(d: &TempDir, mesh: &[(&str, Vec<Shader>)]) -> Plan {
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
    r.recursive = true;
    r.mesh_shaders = mesh
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap()
}

/// Fuzz-23, both cells. A mesh majority of `Default` is no evidence on a fuzz-map set: Fuzz-1a applies and the set
/// is PBR, as `sdc convert` plans it with no votes. A majority of another shader (Hair) wins as today; the fuzz map
/// is listed unused with that shader named.
#[test]
fn a_default_mesh_vote_keeps_fuzz_1a_and_another_vote_wins() {
    // Under `textures/`, so the mesh key is the game path (`shaders.rs` uses `textures/a/wall`).
    let files = [
        "textures/a/rock01.png",
        "textures/a/rock01_n.png",
        "textures/a/rock01_fuzz.png",
    ];
    let key = "textures/a/rock01";
    let d = fixture(&files, "");
    let p = plan_mesh(&d, &[(key, vec![Shader::Default, Shader::Default])]);
    assert_eq!(shader(&p), Shader::Pbr);
    assert_eq!(job(&p, "rock01.png").profile.subtype, Subtype::PbrAlbedo);
    assert_eq!(out_name(job(&p, "rock01_fuzz.png")), "rock01_f.dds");
    let d = fixture(&files, "");
    let p = plan_mesh(&d, &[(key, vec![Shader::Hair, Shader::Hair])]);
    assert_eq!(shader(&p), Shader::Hair);
    assert_eq!(
        unused_reason(&p, "rock01_fuzz.png"),
        format!(
            "rock01_fuzz.png: the {} shader reads no fuzz map; not converted",
            Shader::Hair.id()
        )
    );
}

/// Fuzz-1a is cs only: under vanilla nothing changes (no `_f` output, the pick is not PBR).
#[test]
fn vanilla_fuzz_sets_are_unchanged() {
    let d = fixture(&["rock01.png", "rock01_n.png", "rock01_fuzz.png"], "");
    let mut r = BatchRequest::new(
        vec![d.path().join("in")],
        d.path().join("out"),
        Target::Vanilla,
    );
    r.recursive = true;
    let p = Engine::new()
        .unwrap()
        .plan(
            r,
            &Reporter::new(Arc::new(VecSink::default()), "t"),
            &CancelToken::default(),
        )
        .unwrap();
    assert_ne!(shader(&p), Shader::Pbr);
    // Not even wanted: a PBR pick would fall back to Default with `wanted: pbr` (SHADER-UNAVAILABLE).
    assert_eq!(p.shaders[&job(&p, "rock01.png").set_key].wanted, None);
    assert!(
        !out_names(&p).iter().any(|n| n == "rock01_f.dds"),
        "{:?}",
        out_names(&p)
    );
}

/// Fuzz-1a with a Note DY alpha fill: a `fuzzweight` helper fills the fuzz map's alpha, and the filled map still
/// writes `<base>_f.dds` (the fill step resolves it again, before the set gains `f`). The set reports `pbr`.
#[test]
fn a_filled_fuzz_map_keeps_base_f() {
    let d = fixture(
        &["x.png", "x_n.png", "x_fuzz.png", "x_w.png"],
        "[files.\"x_w.png\"]\nhelper.role = \"fuzzweight\"\n",
    );
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
    r.recursive = true;
    let sink = Arc::new(VecSink::default());
    let p = Engine::new()
        .unwrap()
        .plan(
            r,
            &Reporter::new(sink.clone(), "t"),
            &CancelToken::default(),
        )
        .unwrap();
    let j = job(&p, "x_fuzz.png");
    assert!(
        j.fill
            .as_ref()
            .is_some_and(|f| f.source.path.ends_with("x_w.png")),
        "the helper fills the fuzz map: {:?}",
        j.fill
    );
    assert!(
        !matches!(j.status, JobStatus::PackingInput { .. }),
        "{:?} {}",
        j.status,
        out_name(j)
    );
    assert_eq!(j.profile.subtype, Subtype::PbrFuzz);
    assert_eq!(out_name(j), "x_f.dds");
    let key = j.set_key.clone();
    let pbr: Vec<bool> = sink
        .take()
        .into_iter()
        .filter(|e| e.set.as_deref() == Some(key.as_str()))
        .filter_map(|e| match e.kind {
            EventKind::SetResolved { pbr, .. } => Some(pbr),
            _ => None,
        })
        .collect();
    assert_eq!(pbr, vec![true]);
}
