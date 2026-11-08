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

/// Final review I1: a Landscape picked from a mesh makes the set a detail host after the pick, so two detail maps in
/// it are IN-DUPLICATE (not both written), on every target.
#[test]
fn a_mesh_picked_landscape_with_two_details_is_in_duplicate() {
    for target in [Target::Vanilla, Target::Cs] {
        let d = tempfile::tempdir().unwrap();
        files(
            d.path(),
            &[
                "textures/a/x.png",
                "textures/a/x_detail.png",
                "textures/a/x_detailnorm.png",
            ],
        );
        let mesh = BTreeMap::from([("textures/a/x".to_string(), vec![Shader::Landscape])]);
        let p = plan_with(d.path(), target, &[], mesh, AcceptSuggestions::High);
        assert_eq!(
            dup_names(&p),
            ["x_detail.png", "x_detailnorm.png"],
            "{target:?}"
        );
        assert!(
            p.jobs
                .iter()
                .all(|j| !j.input.to_string_lossy().contains("_detail")),
            "{target:?}"
        );
    }
}

/// Final review I3: a set edit keeps the detail members of a landscape or skin set as detail maps.
#[test]
fn set_edits_keep_the_detail_members() {
    let cases: [(Target, &[&str], &str, Subtype); 2] = [
        (
            Target::Cs,
            &[
                "textures/landscape/x.png",
                "textures/landscape/x_detail.png",
            ],
            "x_detail.png",
            Subtype::DetailNormal,
        ),
        (
            Target::Cs,
            &[
                "textures/a/x_rfaos.png",
                "textures/a/x.png",
                "textures/a/x_detail_n.png",
            ],
            "x_detail_n.png",
            Subtype::SkinDetail,
        ),
    ];
    for (target, names, detail, want) in cases {
        let dir = names[0].rsplit_once('/').unwrap().0;
        let id = format!("{dir}/x");
        let spelled = detail.trim_end_matches(".png").trim_start_matches("x");
        for edit in [
            engine::sets::SetEdit::Rename {
                from: id.clone(),
                to: format!("{dir}/x2"),
            },
            engine::sets::SetEdit::Move {
                file: format!("{dir}/other.png"),
                to: id.clone(),
            },
        ] {
            let d = tempfile::tempdir().unwrap();
            files(d.path(), names);
            put(d.path(), &format!("{dir}/other.png"));
            let mut req =
                BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), target);
            req.recursive = true;
            req.set_edits = vec![edit.clone()];
            let rep = Reporter::new(Arc::new(VecSink::default()), "detail-names");
            let p = Engine::new()
                .unwrap()
                .plan(req, &rep, &CancelToken::default())
                .unwrap();
            let j = job(&p, detail);
            // A Rename moves the set id only; the file keeps its own spelling and output name.
            assert_eq!(j.profile.subtype, want, "{edit:?}");
            assert_eq!(out_name(j), format!("x{spelled}.dds"), "{edit:?}");
        }
    }
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

    // The other skin pair, and the landscape `detail` + `detail_n` pair.
    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/a/x_rfaos.png",
            "textures/a/x.png",
            "textures/a/x_detailnorm.png",
            "textures/a/x_detail_n.png",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(dup_names(&p), ["x_detail_n.png", "x_detailnorm.png"]);

    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/landscape/x.png",
            "textures/landscape/x_detail.png",
            "textures/landscape/x_detail_n.png",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    assert_eq!(dup_names(&p), ["x_detail.png", "x_detail_n.png"]);
}

/// Final review I2: two same-stem twins in one detail host are one IN-DUPLICATE group each, reported once per file.
#[test]
fn same_stem_detail_twins_are_reported_once() {
    let d = tempfile::tempdir().unwrap();
    files(
        d.path(),
        &[
            "textures/landscape/x.png",
            "textures/landscape/x_detail.png",
            "textures/landscape/x_detail.tga",
        ],
    );
    let p = plan(d.path(), Target::Cs, &[]);
    let n = p
        .skipped
        .iter()
        .filter(|s| s.code == Some(Code::InDuplicate))
        .count();
    assert_eq!(n, 2);
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

/// Note GW3 (controller's call 1): the old layer names are plain files now. `rock_n` exports with no blend input, and
/// nothing reports an `RNM-*` code for `rock_bump`.
#[test]
fn old_layer_names_are_just_files() {
    let d = tempfile::tempdir().unwrap();
    files(d.path(), &["rock.png", "rock_n.png", "rock_bump.png"]);
    let p = plan(d.path(), Target::Cs, &[]);
    for j in &p.jobs {
        for w in &j.warnings {
            assert!(!w.code.as_str().starts_with("RNM-"), "{:?}", w);
        }
    }
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "old-layer-names");
    Engine::new()
        .unwrap()
        .run(p, &rep, &CancelToken::default())
        .unwrap();
    let events = serde_json::to_string(&sink.take()).unwrap();
    assert!(!events.contains("RNM-"), "{events}");
    let m = engine::manifest::Manifest::at(&d.path().join("out"))
        .load()
        .unwrap();
    let n = m
        .values()
        .find(|e| e.output.ends_with("rock_n.dds"))
        .unwrap();
    assert!(n.sources.is_empty(), "{:?}", n.sources);
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

    // The same through a `[files]` entry in `sdc.toml` (project provenance).
    for (saved, want) in [
        ("detail_normal", Subtype::DetailNormal),
        ("generic_data", Subtype::GenericData),
    ] {
        std::fs::write(
            d.path().join("in/sdc.toml"),
            format!("[files.\"textures/landscape/x_detail.png\"]\nsubtype = \"{saved}\"\n"),
        )
        .unwrap();
        let p = plan(d.path(), Target::Cs, &[]);
        let j = job(&p, "x_detail.png");
        assert_eq!(j.profile.subtype, want, "{saved}");
        assert!(
            !j.provenance
                .get("subtype")
                .is_some_and(|p| p.to_string().starts_with("shader:")),
            "{saved}: {:?}",
            j.provenance.get("subtype")
        );
    }
}
