//! Note HH: a set can share another set's helper. The file stays a member of its own set; the sharing set reads it
//! as one of its own, under its own folder and name, and nothing new is written for it.
use engine::api::*;
use engine::codes::Code;
use engine::config::Target;
use engine::events::*;
use engine::overrides::parse_set;
use engine::pack::ChannelSource;
use engine::policy::{JobStatus, ResolvedJob};
use engine::sets::SetEdit;
use image::{ImageBuffer, Luma, Rgb};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

/// `in/<rel>` at `w`x`h`: a name with no `_` in colour, the rest grey.
fn put(d: &Path, rel: &str, w: u32, h: u32) {
    let p = d.join("in").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
    if stem.contains('_') {
        ImageBuffer::from_fn(w, h, |x, y| Luma([(x * 3 + y) as u8]))
            .save(p)
            .unwrap();
    } else {
        ImageBuffer::from_fn(w, h, |x, y| Rgb([(x * 3) as u8, (y * 3) as u8, 90]))
            .save(p)
            .unwrap();
    }
}

fn fixture(files: &[&str]) -> TempDir {
    let d = tempfile::tempdir().unwrap();
    for f in files {
        put(d.path(), f, 32, 32);
    }
    d
}

const AB: &[&str] = &[
    "textures/a/rock.png",
    "textures/a/rock_ao.png",
    "textures/b/stone.png",
    "textures/b/stone_roughness.png",
];

fn share(file: &str, to: &str) -> SetEdit {
    SetEdit::Share {
        file: file.into(),
        to: to.into(),
    }
}

fn s() -> SetEdit {
    share("textures/a/rock_ao.png", "textures/b/stone")
}

fn request(d: &TempDir, edits: Vec<SetEdit>, sets: &[&str]) -> BatchRequest {
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
    r.recursive = true;
    r.overrides = sets.iter().map(|s| parse_set(s).unwrap()).collect();
    r.set_edits = edits;
    r
}

fn plan_req(r: BatchRequest) -> (Plan, Vec<EngineEvent>) {
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "t");
    let p = Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap();
    (p, sink.take())
}

fn plan(d: &TempDir, edits: Vec<SetEdit>) -> Plan {
    plan_req(request(d, edits, &[])).0
}

fn key(p: &Plan, id: &str) -> String {
    engine::sets::set_key(&p.request.output_root, id)
}

/// The packed job writing `name`.
fn packed<'a>(p: &'a Plan, name: &str) -> &'a ResolvedJob {
    p.jobs
        .iter()
        .find(|j| j.pack.is_some() && j.output.file_name().is_some_and(|n| n == name))
        .unwrap_or_else(|| panic!("no packed {name}: {:#?}", outputs(p)))
}

fn outputs(p: &Plan) -> Vec<String> {
    let mut v: Vec<String> = p
        .jobs
        .iter()
        .filter(|j| !matches!(j.status, JobStatus::PackingInput { .. }))
        .map(|j| j.output.to_string_lossy().replace('\\', "/"))
        .collect();
    v.sort();
    v
}

/// The source of channel `c` of `job`'s recipe.
fn channel(job: &ResolvedJob, c: usize) -> &Path {
    match &job.pack.as_ref().unwrap().channels[c] {
        ChannelSource::Source { path, .. } => path,
        other => panic!("channel {c} is {other:?}"),
    }
}

fn job_of<'a>(p: &'a Plan, name: &str) -> Option<&'a ResolvedJob> {
    p.jobs
        .iter()
        .find(|j| j.pack.is_none() && j.input.file_name().is_some_and(|n| n == name))
}

fn unused(p: &Plan, name: &str) -> bool {
    job_of(p, name).is_some_and(|j| j.warnings.iter().any(|w| w.code == Code::PackUnusedInput))
}

fn report<'a>(p: &'a Plan, name: &str) -> &'a engine::helper::report::HelperReport {
    p.helpers
        .iter()
        .find(|h| h.path.file_name().is_some_and(|n| n == name))
        .unwrap_or_else(|| panic!("no helper report for {name}"))
}

/// `set_resolved.members` (file names, sorted) and `pbr` of the set `key`.
fn resolved(ev: &[EngineEvent], key: &str) -> (Vec<String>, bool) {
    ev.iter()
        .find_map(|e| match &e.kind {
            EventKind::SetResolved { members, pbr, .. } if e.set.as_deref() == Some(key) => {
                let mut m: Vec<String> = members
                    .iter()
                    .map(|m| m.rsplit(['/', '\\']).next().unwrap().to_string())
                    .collect();
                m.sort();
                Some((m, *pbr))
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no set_resolved for {key}"))
}

fn stale(ev: &[EngineEvent]) -> Vec<String> {
    ev.iter()
        .filter_map(|e| match &e.kind {
            EventKind::Warning(w) if w.code == Code::SetEditStale => Some(w.message.clone()),
            _ => None,
        })
        .collect()
}

/// Run `p` and list the files written under `out/`, '/'-separated and relative to it.
fn written(d: &TempDir, p: Plan) -> Vec<String> {
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    Engine::new()
        .unwrap()
        .run(p, &rep, &CancelToken::default())
        .unwrap();
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if p.extension().is_some_and(|x| x == "dds") {
                let rel = p.strip_prefix(root).unwrap();
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let root = d.path().join("out");
    let mut v = vec![];
    walk(&root, &root, &mut v);
    v.sort();
    v
}

#[test]
fn a_shared_ao_feeds_the_other_sets_rmaos() {
    let d = fixture(AB);
    let (p, ev) = plan_req(request(&d, vec![s()], &[]));
    let rmaos = packed(&p, "stone_rmaos.dds");
    assert!(channel(rmaos, 2).ends_with("textures/a/rock_ao.png"));
    assert!(channel(rmaos, 0).ends_with("textures/b/stone_roughness.png"));
    assert_eq!(rmaos.set_key, key(&p, "textures/b/stone"));
    // rock_ao stays a member of set a; set b's members are unchanged.
    let (_, before) = plan_req(request(&d, vec![], &[]));
    for id in ["textures/a/rock", "textures/b/stone"] {
        assert_eq!(
            resolved(&ev, &key(&p, id)).0,
            resolved(&before, &key(&p, id)).0,
            "{id}"
        );
    }
    assert!(resolved(&ev, &key(&p, "textures/a/rock"))
        .0
        .contains(&"rock_ao.png".to_string()));
    assert_eq!(
        report(&p, "rock_ao.png").set_key,
        key(&p, "textures/a/rock")
    );
}

#[test]
fn a_share_adds_no_job_and_no_file() {
    let d = fixture(AB);
    let (with, ev_with) = plan_req(request(&d, vec![s()], &[]));
    let (without, ev_without) = plan_req(request(&d, vec![], &[]));
    let found = |ev: &[EngineEvent]| {
        ev.iter()
            .filter(|e| matches!(e.kind, EventKind::FileDiscovered { .. }))
            .count()
    };
    assert_eq!(found(&ev_with), found(&ev_without));
    // The same outputs: b's recipe only gains a source.
    assert_eq!(outputs(&with), outputs(&without));
    assert_eq!(
        packed(&without, "stone_rmaos.dds")
            .pack
            .as_ref()
            .unwrap()
            .sources
            .len()
            + 1,
        packed(&with, "stone_rmaos.dds")
            .pack
            .as_ref()
            .unwrap()
            .sources
            .len()
    );
}

#[test]
fn a_shared_roughness_writes_in_the_target_folder() {
    let d = fixture(&[
        "textures/a/rock.png",
        "textures/a/rock_roughness.png",
        "textures/c/pbr/slab.png",
    ]);
    let p = plan(
        &d,
        vec![share(
            "textures/a/rock_roughness.png",
            "textures/c/pbr/slab",
        )],
    );
    let c = key(&p, "textures/c/pbr/slab");
    let job = p
        .jobs
        .iter()
        .find(|j| j.pack.is_some() && j.set_key == c)
        .expect("set c packs");
    assert_eq!(
        job.output,
        d.path().join("out").join("textures/c/pbr/slab_rmaos.dds")
    );
    assert!(
        job.game_path.starts_with("textures/c/pbr/") && !job.game_path.contains("textures/a/"),
        "{}",
        job.game_path
    );
    assert!(channel(job, 0).ends_with("textures/a/rock_roughness.png"));
    // Set a still packs its own: two recipes with one first source, and both write.
    packed(&p, "rock_rmaos.dds");
    let w = written(&d, p);
    for f in ["textures/a/rock_rmaos.dds", "textures/c/pbr/slab_rmaos.dds"] {
        assert!(w.contains(&f.to_string()), "{f}: {w:?}");
    }
}

/// A cs AO alone packs an `_rmaos`, so an AO its own set holds no recipe for is the twin there that loses (HE7):
/// `rock_ao.tga` behind `rock_ao.png`.
fn twins(owner: &str, b: &[&str]) -> TempDir {
    let mut files = vec![
        format!("textures/{owner}/rock.png"),
        format!("textures/{owner}/rock_ao.png"),
        format!("textures/{owner}/rock_ao.tga"),
        "textures/b/stone.png".into(),
        "textures/b/stone_roughness.png".into(),
    ];
    files.extend(b.iter().map(|f| f.to_string()));
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    fixture(&files)
}

#[test]
fn an_unused_owner_is_not_flagged() {
    // `z` plans after `b`, `a` before it.
    for owner in ["a", "z"] {
        let d = twins(owner, &[]);
        let p = plan(&d, vec![]);
        assert!(
            unused(&p, "rock_ao.tga"),
            "{owner}: unused without the share"
        );
        let p = plan(
            &d,
            vec![share(
                &format!("textures/{owner}/rock_ao.tga"),
                "textures/b/stone",
            )],
        );
        assert!(!unused(&p, "rock_ao.tga"), "{owner}");
        assert!(
            job_of(&p, "rock_ao.tga").is_none(),
            "{owner}: consumed by b's recipe"
        );
        assert!(channel(packed(&p, "stone_rmaos.dds"), 2).ends_with("rock_ao.tga"));
        assert_eq!(report(&p, "rock_ao.tga").not_used, None, "{owner}");
    }
}

#[test]
fn unused_everywhere_is_still_flagged() {
    // b's own `stone_ao.png` sorts before `textures/z/rock_ao.tga`, so b can't use it either.
    let d = twins("z", &["textures/b/stone_ao.png"]);
    let p = plan(
        &d,
        vec![share("textures/z/rock_ao.tga", "textures/b/stone")],
    );
    assert!(unused(&p, "rock_ao.tga"));
    assert!(channel(packed(&p, "stone_rmaos.dds"), 2).ends_with("stone_ao.png"));
    let h = report(&p, "rock_ao.tga");
    assert!(h.not_used.is_some());
    // Shared with b, used by no set (Note HH7).
    assert_eq!(h.shared_with, [key(&p, "textures/b/stone")]);
    assert!(h.used_by.is_empty(), "{:?}", h.used_by);
}

#[test]
fn a_share_with_an_own_twin_takes_the_first_by_path() {
    let mut files = AB.to_vec();
    files.push("textures/b/stone_ao.png");
    let d = fixture(&files);
    let p = plan(&d, vec![s()]);
    let rmaos = packed(&p, "stone_rmaos.dds");
    // `textures/a/rock_ao.png` sorts before `textures/b/stone_ao.png`.
    assert!(channel(rmaos, 2).ends_with("textures/a/rock_ao.png"));
    assert!(unused(&p, "stone_ao.png"));
}

#[test]
fn a_shared_helper_fills_alpha() {
    // The mask is alone in its own set, so it lands nowhere there; `z` plans after `b`.
    for owner in ["a", "z"] {
        let d = fixture(&[
            &format!("textures/{owner}/rock_opacity.png"),
            "textures/b/stone.png",
            "textures/b/stone_roughness.png",
        ]);
        let fills = ["file:**/rock_opacity.png.helper.fills=[\"diffuse.a\"]"];
        let edits = vec![share(
            &format!("textures/{owner}/rock_opacity.png"),
            "textures/b/stone",
        )];
        let (p, _) = plan_req(request(&d, edits, &fills));
        let stone = job_of(&p, "stone.png").expect("stone.png has a job");
        let fill = stone.fill.as_ref().expect("stone.png is filled");
        assert!(fill.source.path.ends_with("rock_opacity.png"), "{owner}");
        let h = report(&p, "rock_opacity.png");
        assert_eq!(h.not_used, None, "{owner}");
        assert!(
            h.outputs.contains(&stone.output),
            "{owner}: {:?}",
            h.outputs
        );
        assert!(h.fills.is_empty(), "{owner}: fills stay own-set");
        // Used, so consumed: its own job leaves the plan.
        assert!(job_of(&p, "rock_opacity.png").is_none(), "{owner}");
    }
}

#[test]
fn an_aspect_mismatch_is_size_mismatch() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/a/rock.png", 64, 32);
    put(d.path(), "textures/a/rock_ao.png", 64, 32);
    put(d.path(), "textures/b/stone.png", 64, 64);
    put(d.path(), "textures/b/stone_roughness.png", 64, 64);
    let p = plan(&d, vec![s()]);
    let j = p
        .jobs
        .iter()
        .find(|j| j.output.file_name().is_some_and(|n| n == "stone_rmaos.dds"))
        .expect("stone_rmaos has a job");
    assert!(
        j.warnings.iter().any(|w| w.code == Code::PackSizeMismatch),
        "{:?}",
        j.warnings
    );
}

#[test]
fn a_share_of_a_missing_file_is_stale() {
    let d = fixture(AB);
    let (p, ev) = plan_req(request(
        &d,
        vec![share("textures/a/nope.png", "textures/b/stone")],
        &[],
    ));
    assert_eq!(stale(&ev).len(), 1, "{:?}", stale(&ev));
    assert_eq!(outputs(&p), outputs(&plan(&d, vec![])));
}

#[test]
fn a_share_into_an_empty_set_is_stale() {
    let d = fixture(AB);
    let (_, ev) = plan_req(request(
        &d,
        vec![
            SetEdit::Create {
                set: "textures/e".into(),
            },
            share("textures/a/rock_ao.png", "textures/e"),
        ],
        &[],
    ));
    let st = stale(&ev);
    assert_eq!(st.len(), 1, "{st:?}");
    assert!(st[0].contains("has no texture of its own"), "{}", st[0]);
}

#[test]
fn the_report_lists_where_it_went() {
    let mut files = AB.to_vec();
    files.push("textures/a/rock_roughness.png");
    let d = fixture(&files);
    let p = plan(&d, vec![s()]);
    let h = report(&p, "rock_ao.png");
    assert_eq!(h.shared_with, [key(&p, "textures/b/stone")]);
    assert_eq!(h.used_by, h.shared_with);
    let own = packed(&p, "rock_rmaos.dds").output.clone();
    let shared = packed(&p, "stone_rmaos.dds").output.clone();
    // Own outputs first.
    assert_eq!(h.outputs, [own, shared]);
    // `fills` stays own-set: a's `_rmaos` B only.
    assert_eq!(
        h.fills,
        [engine::helper::FillTarget::Packed {
            suffix: "rmaos",
            channel: 2
        }]
    );
    assert!(report(&p, "stone_roughness.png").shared_with.is_empty());
}

#[test]
fn a_share_into_its_own_set_does_nothing() {
    let d = fixture(AB);
    let (with, ev) = plan_req(request(
        &d,
        vec![share("textures/a/rock_ao.png", "textures/a/rock")],
        &[],
    ));
    assert!(stale(&ev).is_empty());
    let without = plan(&d, vec![]);
    let view = |p: &Plan| format!("{:?}", (&p.jobs, &p.skipped, &p.helpers));
    assert_eq!(view(&with), view(&without));
}

#[test]
fn a_no_set_helper_shared_as_first_source_writes_the_target() {
    let d = fixture(&["textures/a/rock_roughness.png", "textures/c/pbr/slab.png"]);
    let p = plan(
        &d,
        vec![
            SetEdit::NoSet {
                file: "textures/a/rock_roughness.png".into(),
            },
            share("textures/a/rock_roughness.png", "textures/c/pbr/slab"),
        ],
    );
    assert!(
        !p.skipped.iter().any(|s| s.code == Some(Code::InDuplicate)),
        "{:?}",
        p.skipped
    );
    let w = written(&d, p);
    assert!(
        w.contains(&"textures/c/pbr/slab_rmaos.dds".to_string()),
        "{w:?}"
    );
    assert!(!w.iter().any(|f| f.starts_with("textures/a/")), "{w:?}");
}

#[test]
fn a_no_set_file_still_writes_nothing_of_its_own() {
    let d = fixture(&["textures/a/rock_roughness.png"]);
    let p = plan(
        &d,
        vec![SetEdit::NoSet {
            file: "textures/a/rock_roughness.png".into(),
        }],
    );
    // Its one-file set plans an `_rmaos` from it, which is not written either.
    assert!(p.jobs.iter().any(|j| j.pack.is_some()));
    assert!(written(&d, p).is_empty());
}

#[test]
fn exporting_the_target_alone_reads_the_shared_helper() {
    let d = fixture(AB);
    let mut r = request(&d, vec![s()], &[]);
    let b = engine::sets::set_key(&r.output_root, "textures/b/stone");
    r.write_only = Some(BTreeSet::from([b]));
    let (p, _) = plan_req(r);
    assert!(channel(packed(&p, "stone_rmaos.dds"), 2).ends_with("rock_ao.png"));
    let w = written(&d, p);
    assert!(
        w.contains(&"textures/b/stone_rmaos.dds".to_string()),
        "{w:?}"
    );
    assert!(!w.iter().any(|f| f.starts_with("textures/a/")), "{w:?}");
}

#[test]
fn a_shared_roughness_makes_a_cs_set_pbr() {
    // Not under a `pbr` folder, which alone makes a set PBR.
    let d = fixture(&[
        "textures/a/rock.png",
        "textures/a/rock_roughness.png",
        "textures/c/slab.png",
    ]);
    let (p, ev) = plan_req(request(&d, vec![], &[]));
    let c = key(&p, "textures/c/slab");
    assert!(!resolved(&ev, &c).1, "colour only: not PBR");
    let (_, ev) = plan_req(request(
        &d,
        vec![share("textures/a/rock_roughness.png", "textures/c/slab")],
        &[],
    ));
    assert!(resolved(&ev, &c).1, "a shared roughness makes it PBR");
    // The share is no member.
    assert_eq!(resolved(&ev, &c).0, ["slab.png"]);
}
