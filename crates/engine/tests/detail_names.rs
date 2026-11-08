//! Note GW1 (v3 chunk E, Task 1): `_detail`, `_detailnorm` and `_detail_n` are one detail map on landscape and skin
//! sets (`DetailNormal`, `SkinDetail`), written under their own spelling; elsewhere they stay undecided or ordinary.
use engine::api::*;
use engine::codes::{Code, Severity};
use engine::config::*;
use engine::events::*;
use engine::policy::*;
use engine::shader::Shader;
use image::{ImageBuffer, Rgb};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

fn put(dir: &Path, rel: &str) {
    let p = dir.join("in").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(32, 32, |x, y| {
        Rgb([128u8, 128 + (x % 2) as u8, 255 - (y % 2) as u8])
    })
    .save(p)
    .unwrap();
}

fn plan_with(
    dir: &Path,
    target: Target,
    sets: &[&str],
    mesh: BTreeMap<String, Vec<Shader>>,
    accept: AcceptSuggestions,
) -> Plan {
    let mut req = BatchRequest::new(vec![dir.join("in")], dir.join("out"), target);
    req.recursive = true;
    req.accept = accept;
    req.overrides = sets
        .iter()
        .map(|s| engine::overrides::parse_set(s).unwrap())
        .collect();
    req.mesh_shaders = mesh;
    let rep = Reporter::new(Arc::new(VecSink::default()), "detail-names");
    Engine::new()
        .unwrap()
        .plan(req, &rep, &CancelToken::default())
        .unwrap()
}

fn plan(dir: &Path, target: Target, sets: &[&str]) -> Plan {
    plan_with(dir, target, sets, BTreeMap::new(), AcceptSuggestions::High)
}

fn job<'a>(p: &'a Plan, name: &str) -> &'a ResolvedJob {
    p.jobs
        .iter()
        .find(|j| j.input.ends_with(name))
        .unwrap_or_else(|| {
            panic!(
                "no job {name}: {:#?}",
                p.jobs.iter().map(|j| &j.input).collect::<Vec<_>>()
            )
        })
}

fn out_name(j: &ResolvedJob) -> String {
    j.output.file_name().unwrap().to_string_lossy().into_owned()
}

/// (set tail, sub-type, output file name) of the job for `name`.
fn facts(p: &Plan, name: &str) -> (String, Subtype, String) {
    let j = job(p, name);
    let set = j.set_key.rsplit('/').next().unwrap().to_string();
    (set, j.profile.subtype, out_name(j))
}

fn dup_names(p: &Plan) -> Vec<String> {
    let mut v: Vec<String> = p
        .skipped
        .iter()
        .filter(|s| s.code == Some(Code::InDuplicate))
        .map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn files(dir: &Path, names: &[&str]) {
    for n in names {
        put(dir, n);
    }
}

#[test]
fn landscape_detail_is_detail_normal() {
    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/landscape/x.png",
            "textures/landscape/x_detail.png",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(
        facts(&p, "x_detail.png"),
        ("x".into(), Subtype::DetailNormal, "x_detail.dds".into())
    );
}

#[test]
fn landscape_detailnorm_is_detail_normal() {
    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/landscape/x.png",
            "textures/landscape/x_detailnorm.png",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(
        facts(&p, "x_detailnorm.png"),
        ("x".into(), Subtype::DetailNormal, "x_detailnorm.dds".into())
    );
}

#[test]
fn landscape_detail_n_is_detail_normal() {
    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/landscape/x.png",
            "textures/landscape/x_detail_n.png",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(
        facts(&p, "x_detail_n.png"),
        ("x".into(), Subtype::DetailNormal, "x_detail_n.dds".into())
    );
}

#[test]
fn explicit_landscape_detail_n_joins() {
    for (target, shader) in [
        (Target::Cs, "pbr_landscape"),
        (Target::Cs, "landscape"),
        (Target::Vanilla, "landscape"),
    ] {
        let d = tempfile::tempdir().unwrap();
        files(d.path(), &["textures/a/x.png", "textures/a/x_detail_n.png"]);
        let p = plan(d.path(), target, &[&format!("set:x.shader={shader}")]);
        assert_eq!(
            facts(&p, "x_detail_n.png"),
            ("x".into(), Subtype::DetailNormal, "x_detail_n.dds".into()),
            "{target:?} {shader}"
        );
    }
}

#[test]
fn a_mesh_picked_landscape_keeps_its_detail() {
    let d = tempfile::tempdir().unwrap();
    files(d.path(), &["textures/a/x.png", "textures/a/x_detail.png"]);
    let mesh = BTreeMap::from([("textures/a/x".to_string(), vec![Shader::Landscape])]);
    let p = plan_with(
        d.path(),
        Target::Vanilla,
        &[],
        mesh,
        AcceptSuggestions::High,
    );
    assert_eq!(
        facts(&p, "x_detail.png"),
        ("x".into(), Subtype::DetailNormal, "x_detail.dds".into())
    );
}

#[test]
fn skin_detail_is_skin_detail() {
    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/a/x_rfaos.png",
            "textures/a/x.png",
            "textures/a/x_detail.png",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(
        facts(&p, "x_detail.png"),
        ("x".into(), Subtype::SkinDetail, "x_detail.dds".into())
    );
}

#[test]
fn skin_detailnorm_is_skin_detail() {
    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/a/x_rfaos.png",
            "textures/a/x.png",
            "textures/a/x_detailnorm.png",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(
        facts(&p, "x_detailnorm.png"),
        ("x".into(), Subtype::SkinDetail, "x_detailnorm.dds".into())
    );
}

#[test]
fn skin_detail_n_is_skin_detail() {
    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/a/x_rfaos.png",
            "textures/a/x.png",
            "textures/a/x_detail_n.png",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(
        facts(&p, "x_detail_n.png"),
        ("x".into(), Subtype::SkinDetail, "x_detail_n.dds".into())
    );
}

#[test]
fn two_detail_files_are_in_duplicate() {
    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/landscape/x.png",
            "textures/landscape/x_detail.png",
            "textures/landscape/x_detailnorm.png",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(dup_names(&p), ["x_detail.png", "x_detailnorm.png"]);
    assert!(Code::InDuplicate.severity() == Severity::Blocking);
    assert!(p.jobs.iter().all(|j| !j.input.ends_with("x_detail.png")));

    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/a/x_rfaos.png",
            "textures/a/x.png",
            "textures/a/x_detail.png",
            "textures/a/x_detail_n.png",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(dup_names(&p), ["x_detail.png", "x_detail_n.png"]);
}

#[test]
fn a_lone_detail_outside_landscape_is_not_a_detail_map() {
    let d = tempfile::tempdir().unwrap();
    files(d.path(), &["textures/x/rock_detail.png"]);
    let p = plan(d.path(), Target::Cs, &[]);
    let (set, subtype, _) = facts(&p, "rock_detail.png");
    assert_eq!(set, "rock_detail");
    assert_ne!(subtype, Subtype::DetailNormal);
}

#[test]
fn a_lone_detail_under_landscape_is_detail_normal() {
    let d = tempfile::tempdir().unwrap();
    files(d.path(), &["textures/landscape/rock_detail.png"]);
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(
        facts(&p, "rock_detail.png"),
        (
            "rock".into(),
            Subtype::DetailNormal,
            "rock_detail.dds".into()
        )
    );
}

#[test]
fn a_plain_set_keeps_detail_n_as_its_own_set() {
    let d = tempfile::tempdir().unwrap();
    files(d.path(), &["textures/x/x.png", "textures/x/x_detail_n.png"]);
    let p = plan(d.path(), Target::Cs, &[]);
    let (set, subtype, out) = facts(&p, "x_detail_n.png");
    assert_eq!((set.as_str(), out.as_str()), ("x_detail", "x_detail_n.dds"));
    assert_eq!(subtype.category(), Category::Normal);
    assert_ne!(subtype, Subtype::DetailNormal);
}

#[test]
fn a_plain_set_detail_is_undecided() {
    let d = tempfile::tempdir().unwrap();
    files(d.path(), &["textures/x/x.png", "textures/x/x_detail.png"]);
    let p = plan(d.path(), Target::Cs, &[]);
    let j = job(&p, "x_detail.png");
    assert!(j.set_key.ends_with("/x"), "{}", j.set_key);
    assert!(
        matches!(j.status, JobStatus::Unresolved { .. }),
        "{:?}",
        j.status
    );
    assert!(j.suggestions.is_empty(), "{:?}", j.suggestions);
}

#[test]
fn a_set_with_a_saved_subtype_is_unchanged() {
    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/landscape/x.png",
            "textures/landscape/x_detail.png",
        ],
    );
    let p = plan(
        d.path(),
        Target::Cs,
        &["file:**/x_detail.png.subtype=detail_normal"],
    );
    let j = job(&p, "x_detail.png");
    assert_eq!(j.profile.subtype, Subtype::DetailNormal);
    assert!(
        !j.provenance
            .get("subtype")
            .is_some_and(|p| p.to_string().starts_with("shader:")),
        "{:?}",
        j.provenance.get("subtype")
    );

    // A saved sub-type other than the shader's pick wins too.
    let p = plan(
        d.path(),
        Target::Cs,
        &["file:**/x_detail.png.subtype=generic_data"],
    );
    assert_eq!(
        job(&p, "x_detail.png").profile.subtype,
        Subtype::GenericData
    );
}
