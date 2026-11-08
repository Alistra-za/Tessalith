//! Note GY1 (v3 chunk F, Task 1): a suffix-less name with an underscore, alone in its set, waits for a type. It gets
//! no suggestion, stays unresolved with `SUG-LOW-CONFIDENCE` and writes nothing, unless a `[files]` sub-type, a rule,
//! another member of its set or its path says what it is.
use engine::api::*;
use engine::codes::Code;
use engine::config::*;
use engine::events::*;
use engine::policy::*;
use engine::presets::ConfigFile;
use engine::session::Session;
use engine::sets::SetEdit;
use image::{ImageBuffer, Rgb};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

const REASON: &str =
    "unknown suffix: assign a type with --set <file>:...subtype=<type> or a [files] entry";

/// A 32 px colour gradient at `<dir>/in/<rel>`.
fn put(dir: &Path, rel: &str) {
    let p = dir.join("in").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(32, 32, |x, y| Rgb([(x * 8) as u8, (y * 8) as u8, 90]))
        .save(p)
        .unwrap();
}

/// A flat tangent-space normal at `<dir>/in/<rel>`.
fn flat(dir: &Path, rel: &str) {
    let p = dir.join("in").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_pixel(32, 32, Rgb([128u8, 128, 255]))
        .save(p)
        .unwrap();
}

/// The GUI's request shape: recursive, High suggestions accepted.
fn request(dir: &Path, target: Target) -> BatchRequest {
    let mut req = BatchRequest::new(vec![dir.join("in")], dir.join("out"), target);
    req.recursive = true;
    req.accept = AcceptSuggestions::High;
    req
}

fn plan_req(req: BatchRequest) -> Plan {
    let rep = Reporter::new(Arc::new(VecSink::default()), "unknown-suffix");
    Engine::new()
        .unwrap()
        .plan(req, &rep, &CancelToken::default())
        .unwrap()
}

fn plan(dir: &Path) -> Plan {
    plan_req(request(dir, Target::Cs))
}

fn run(p: Plan) {
    let rep = Reporter::new(Arc::new(VecSink::default()), "unknown-suffix-run");
    Engine::new()
        .unwrap()
        .run(p, &rep, &CancelToken::default())
        .unwrap();
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

/// What a waiting file looks like: unresolved for its sub-type, with no suggestion and the unknown-suffix reason.
fn assert_waits(j: &ResolvedJob) {
    match &j.status {
        JobStatus::Unresolved { reasons, needs } => {
            assert!(needs.contains(&Need::Subtype), "{:?}: {needs:?}", j.input);
            assert!(
                reasons.iter().any(|r| r == REASON),
                "{:?}: {reasons:?}",
                j.input
            );
        }
        s => panic!("{:?}: {s:?}", j.input),
    }
    assert!(
        j.suggestions.is_empty(),
        "{:?}: {:?}",
        j.input,
        j.suggestions
    );
    assert!(
        j.warnings.iter().any(|w| w.code == Code::SugLowConfidence),
        "{:?}: {:?}",
        j.input,
        j.warnings
    );
    assert!(j.waits_for_type(), "{:?}", j.input);
}

fn assert_does_not_wait(j: &ResolvedJob) {
    assert!(!j.suggestions.is_empty(), "{:?}: no suggestion", j.input);
    assert!(!j.waits_for_type(), "{:?}", j.input);
}

fn assert_ready(j: &ResolvedJob, subtype: Subtype) {
    assert!(
        matches!(j.status, JobStatus::Ready),
        "{:?}: {:?}",
        j.input,
        j.status
    );
    assert_eq!(j.profile.subtype, subtype, "{:?}", j.input);
}

#[test]
fn an_unknown_suffix_waits() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/x/rock_bump.png");
    let p = plan(d.path());
    assert_waits(job(&p, "rock_bump.png"));
    run(p);
    assert!(!d.path().join("out/textures/x/rock_bump.dds").exists());
}

#[test]
fn a_bare_name_is_still_colour() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/x/iron.png");
    let p = plan(d.path());
    let j = job(&p, "iron.png");
    assert_ready(j, Subtype::Diffuse);
    assert_eq!(
        j.suggestions[0].confidence,
        engine::suggest::Confidence::High
    );
    assert!(!j.waits_for_type());
}

#[test]
fn a_used_base_is_still_colour() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/x/iron_ore.png");
    flat(d.path(), "textures/x/iron_ore_n.png");
    let p = plan(d.path());
    assert_ready(job(&p, "iron_ore.png"), Subtype::Diffuse);
    assert_ready(job(&p, "iron_ore_n.png"), Subtype::Normal);
    assert!(!job(&p, "iron_ore.png").waits_for_type());
}

#[test]
fn a_saved_subtype_assigns() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/x/rock_bump.png");
    std::fs::write(
        d.path().join("in/sdc.toml"),
        "[files.\"textures/x/rock_bump.png\"]\nsubtype = \"diffuse\"\n",
    )
    .unwrap();
    let p = plan(d.path());
    assert_ready(job(&p, "rock_bump.png"), Subtype::Diffuse);
    assert!(!job(&p, "rock_bump.png").waits_for_type());
    run(p);
    assert!(d.path().join("out/textures/x/rock_bump.dds").exists());
}

#[test]
fn a_rule_assigns() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/x/rock_bump.png");
    std::fs::write(
        d.path().join("in/sdc.toml"),
        "[[rules]]\nmatch = { glob = \"**/*_bump.png\" }\nsubtype = \"diffuse\"\n",
    )
    .unwrap();
    let p = plan(d.path());
    assert_ready(job(&p, "rock_bump.png"), Subtype::Diffuse);
    run(p);
    assert!(d.path().join("out/textures/x/rock_bump.dds").exists());
}

/// Pins the arm's place: the path arms come first, so a lone `a_c` under these paths keeps its suggestion (`a_c`,
/// not `a_b`: `_b` is the backlight suffix).
#[test]
fn path_rules_stay() {
    for (dir, want) in [
        ("textures/landscape", Subtype::LandscapeDiffuse),
        ("textures/sky", Subtype::UiSky),
        ("textures/interface", Subtype::UiSky),
        ("textures/cubemaps", Subtype::Cubemap),
    ] {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), &format!("{dir}/a_c.png"));
        let p = plan(d.path());
        let j = job(&p, "a_c.png");
        assert_does_not_wait(j);
        assert_eq!(j.suggestions[0].subtype, want, "{dir}");
    }
}

/// Names that look unknown but another rule reads: a cube face under `textures/cubemaps`, a `_wet` joined to an
/// Advanced skin set, and a generated height (`rock_p_generated`).
#[test]
fn recognised_shapes_do_not_wait() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/cubemaps/sky_posx.png");
    for n in ["x.png", "x_rfaos.png", "x_wet.png"] {
        put(d.path(), &format!("textures/s/{n}"));
    }
    put(d.path(), "textures/g/rock_p_generated.png");
    let p = plan(d.path());
    let face = job(&p, "sky_posx.png");
    assert_does_not_wait(face);
    assert_eq!(face.suggestions[0].subtype, Subtype::Cubemap);
    let wet = job(&p, "x_wet.png");
    assert!(wet.set_key.ends_with("textures/s/x"), "{}", wet.set_key);
    assert_does_not_wait(wet);
    assert_eq!(wet.profile.subtype, Subtype::SkinWet);
    let h = job(&p, "rock_p_generated.png");
    assert_does_not_wait(h);
    assert_eq!(h.profile.subtype, Subtype::Height);
}

#[test]
fn a_lone_generated_stem_waits() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/x/rock_generated.png");
    assert_waits(job(&plan(d.path()), "rock_generated.png"));
}

/// Spec §3: a lone `rock_detail` with no set `rock`, outside landscape and skin, falls to the unknown-suffix rule.
#[test]
fn a_lone_detail_waits() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/x/rock_detail.png");
    let p = plan(d.path());
    let j = job(&p, "rock_detail.png");
    assert!(j.set_key.ends_with("/rock_detail"), "{}", j.set_key);
    assert_waits(j);
}

#[test]
fn a_hand_add_assigns_and_a_remove_waits() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/x/x.png");
    put(d.path(), "textures/x/a_c.png");
    let plan_with = |edits: Vec<SetEdit>| {
        let mut req = request(d.path(), Target::Cs);
        req.set_edits = edits;
        plan_req(req)
    };
    assert_waits(job(&plan_with(vec![]), "a_c.png"));
    let add = SetEdit::Move {
        file: "textures/x/a_c.png".into(),
        to: "textures/x/x".into(),
    };
    let p = plan_with(vec![add.clone()]);
    let j = job(&p, "a_c.png");
    assert!(j.set_key.ends_with("textures/x/x"), "{}", j.set_key);
    assert_does_not_wait(j);
    let remove = SetEdit::Remove {
        file: "textures/x/a_c.png".into(),
    };
    assert_waits(job(&plan_with(vec![add, remove]), "a_c.png"));
}

/// The jobs of the sets in `scope`, sorted by input, in a comparable form.
fn scoped_jobs(p: &Plan, scope: &BTreeSet<String>) -> Vec<serde_json::Value> {
    let mut jobs: Vec<_> = p
        .jobs
        .iter()
        .filter(|j| scope.contains(&j.set_key))
        .collect();
    jobs.sort_by(|a, b| a.input.cmp(&b.input));
    jobs.iter()
        .map(|j| serde_json::to_value(j).unwrap())
        .collect()
}

/// Review Focus 1: a new `rock_bump_n.png` turns the waiting `rock_bump.png` into a colour map. The GUI plans a disk
/// change in full; after it, a `[files]` edit's fast re-plan gives the jobs the full plan of that edit gives, in both
/// states.
#[test]
fn adding_the_normal_settles_the_base() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/x/rock_bump.png");
    let mut base = request(d.path(), Target::Cs);
    base.project_config_inline = Some(ConfigFile::parse("", "unknown-suffix").unwrap());
    let session = Session::new().unwrap();
    let cancel = CancelToken::default();
    let rep = || Reporter::new(Arc::new(VecSink::default()), "unknown-suffix-fast");
    let key = "textures/x/rock_bump.png";
    // An edit any sub-type takes, so the job's status stays what the type decision makes it.
    let edit: toml::Table = toml::from_str("mips.levels = 1").unwrap();
    let mut edited = base.clone();
    edited
        .project_config_inline
        .as_mut()
        .unwrap()
        .files
        .insert(key.into(), serde_json::to_value(&edit).unwrap());
    let changed = vec![(key.to_string(), Some(edit))];
    let same_both_ways = |want_waiting: bool| {
        let full = session
            .plan_and_keep(base.clone(), &rep(), &cancel)
            .unwrap();
        assert_eq!(
            job(&full, "rock_bump.png").waits_for_type(),
            want_waiting,
            "full plan"
        );
        let partial = session
            .replan_sets(&edited, &changed, &rep(), &cancel)
            .unwrap_or_else(|e| panic!("{e:?}"));
        let next = session
            .engine()
            .plan_cached(edited.clone(), &rep(), &cancel, session.cache())
            .unwrap();
        assert_eq!(
            job(&partial.plan, "rock_bump.png").waits_for_type(),
            want_waiting,
            "fast re-plan"
        );
        assert_eq!(
            scoped_jobs(&partial.plan, &partial.scope),
            scoped_jobs(&next, &partial.scope)
        );
        full
    };
    let before = same_both_ways(true);
    assert_waits(job(&before, "rock_bump.png"));
    flat(d.path(), "textures/x/rock_bump_n.png");
    let after = same_both_ways(false);
    assert_ready(job(&after, "rock_bump.png"), Subtype::Diffuse);
    assert_ready(job(&after, "rock_bump_n.png"), Subtype::Normal);
}

/// Review Focus 2: `--allow SUG-LOW-CONFIDENCE` has no suggestion to accept on a waiting file.
#[test]
fn allow_does_not_convert_a_waiting_file() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/x/rock_bump.png");
    let mut req = request(d.path(), Target::Cs);
    req.allow.insert(Code::SugLowConfidence);
    let p = plan_req(req);
    let j = job(&p, "rock_bump.png");
    assert_waits(j);
    let JobStatus::Unresolved { reasons, .. } = &j.status else {
        unreachable!()
    };
    assert!(reasons.iter().any(|r| r.contains("--set")), "{reasons:?}");
    run(p);
    assert!(!d.path().join("out/textures/x/rock_bump.dds").exists());
}
