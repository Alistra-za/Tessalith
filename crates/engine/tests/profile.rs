mod also_support;
#[allow(dead_code)]
mod profile_support;

use engine::api::CancelToken;
use engine::config::{AlphaPolicy, Format, MipFilter, NormalBase, Subtype};
use engine::manifest::Manifest;
use engine::presets::AlsoTarget;
use engine::profile::{CallKind, Recorder, Stage};
use profile_support::*;
use std::{fs, io::Write, path::PathBuf};

#[test]
fn the_fixture_project_plans_every_case() {
    let fixture = Fixture::new();
    let plan = fixture.plan("out", None, &CancelToken::default());
    let ready = facts(&plan);
    assert_eq!(
        ready
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        fixture.expected,
        "{:?}",
        plan.jobs
            .iter()
            .map(|j| (&j.output, &j.status))
            .collect::<Vec<_>>()
    );
    let job = |name| plan.jobs.iter().find(|j| j.output.ends_with(name)).unwrap();
    let packed = job("crate_rmaos.dds");
    assert!(packed.pack.is_some() && packed.bake.is_some());
    assert_eq!(job("leaf.dds").profile.alpha.policy, AlphaPolicy::Cutout);
    assert_eq!(job("glass.dds").profile.alpha.policy, AlphaPolicy::Blend);
    let one = &job("onebit.dds").profile;
    assert_eq!(one.format, Format::Bc1);
    assert!(one.bc1_one_bit_alpha() && one.alpha.policy == AlphaPolicy::Cutout);
    assert_eq!(job("keep_n.dds").profile.subtype, Subtype::Normal);
    assert_eq!(job("keep_n.dds").profile.mips.normal_base, NormalBase::Keep);
    let flow = &job("water_flow.dds").profile;
    assert_eq!((flow.subtype, flow.format), (Subtype::Flow, Format::Bc5));
    assert_eq!(job("cliff_h.dds").profile.format, Format::Bc4);
    assert_eq!(
        engine::resize::target_dims((31, 20), &job("wide.dds").profile),
        (32, 16)
    );
    assert_eq!(job("plain.dds").profile.mips.filter, MipFilter::Kaiser);
}

#[test]
fn write_project_refuses_an_existing_folder() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("empty");
    fs::create_dir(&root).unwrap();
    assert!(profile_fixture::write_project(&root).is_err());
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[test]
fn fixture_writer_requires_its_destination() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "write_fixture_project",
            "--nocapture",
        ])
        .env_remove("SDC_PROFILE_FIXTURE_DIR")
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "writer succeeded without a destination"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("set SDC_PROFILE_FIXTURE_DIR"));
}

#[test]
#[ignore]
fn write_fixture_project() {
    let root = std::env::var_os("SDC_PROFILE_FIXTURE_DIR")
        .expect("set SDC_PROFILE_FIXTURE_DIR to a new fixture directory");
    let keys = profile_fixture::write_project(&PathBuf::from(&root)).unwrap();
    let mut list = root;
    list.push("-expected.txt");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(list)
        .unwrap();
    for key in keys {
        writeln!(file, "{key}").unwrap();
    }
}

#[test]
fn profile_on_and_off_write_the_same_bytes() {
    let fixture = Fixture::new();
    let rec = Recorder::new();
    let cancel = CancelToken::default();
    for (out, recorder) in [("off", None), ("on", Some(&rec))] {
        let plan = fixture.plan(out, recorder, &cancel);
        run(plan, &cancel);
        assert_eq!(
            also_support::dds_files(&fixture.dir.path().join(out)),
            fixture.expected
        );
    }
    let (off, on) = (
        fixture.dir.path().join("off"),
        fixture.dir.path().join("on"),
    );
    let hdr = "textures/pbr/lamp_g.dds";
    assert!(fixture.expected.contains(hdr), "missing HDR parity fixture");
    for out in [&off, &on] {
        let dds = engine::dds::read_dds(&fs::read(out.join(hdr)).unwrap()).unwrap();
        assert_eq!(dds.format, engine::dds::DdsPixelFormat::Bc6hUf16);
    }
    let records = |p| Manifest::at(p).read().unwrap();
    let (a, b) = (records(&off), records(&on));
    assert_eq!(
        a.keys().cloned().collect::<std::collections::BTreeSet<_>>(),
        fixture.expected
    );
    assert!(a.keys().eq(b.keys()));
    for key in &fixture.expected {
        assert!(
            fs::read(off.join(key)).unwrap() == fs::read(on.join(key)).unwrap(),
            "DDS bytes differ: {key}"
        );
        let normalized = |v: &serde_json::Value| {
            let mut o = v.as_object().unwrap().clone();
            o.remove("ts");
            o.remove("output_stamp");
            o
        };
        assert!(a[key].raw["validation"].is_object());
        assert_eq!(a[key].raw["validation"], b[key].raw["validation"], "{key}");
        assert_eq!(normalized(&a[key].raw), normalized(&b[key].raw), "{key}");
    }
}

#[test]
fn spans_nest_per_thread_and_cover_each_job() {
    let f = Fixture::new();
    let rec = Recorder::new();
    let cancel = CancelToken::default();
    let plan = f.plan("out", Some(&rec), &cancel);
    let planned = facts(&plan);
    assert_eq!(run(plan, &cancel).converted as usize, f.expected.len());
    check_spans(
        &rec.report("fixture", vec![]),
        &f.dir.path().join("out"),
        &planned,
    );
}

#[test]
fn row_spans_lie_inside_their_job() {
    let f = Fixture::new();
    let rec = Recorder::new();
    let cancel = CancelToken::default();
    run(f.plan("out", Some(&rec), &cancel), &cancel);
    let r = rec.report("fixture", vec![]);
    let run = call_of_kind(&r, CallKind::Run);
    for (stage, suffix) in [
        (Stage::EncodeBand, ".dds"),
        (Stage::EncodeBand, "lamp_g.dds"),
        (Stage::ResizeRows, "wide.dds"),
        (Stage::FilterRows, "plain.dds"),
    ] {
        assert!(
            spans(&r, stage).any(|s| r
                .jobs
                .iter()
                .any(|j| j.id == s.1 && j.output.ends_with(suffix))),
            "missing {stage:?} for {suffix}"
        );
        for row in spans(&r, stage) {
            assert!(r.jobs.iter().any(|j| j.call == run.call && j.id == row.1));
            for parent in [Stage::File, Stage::RowJoin] {
                assert!(
                    spans(&r, parent).any(|s| s.1 == row.1 && inside(row, s)),
                    "{row:?} outside {parent:?}"
                );
            }
        }
    }
}

#[test]
fn an_input_encoded_by_two_phases_has_two_jobs() {
    use also_support::{also, cup, project, request, spec};
    let (_d, m, loc) = project(cup);
    let rec = Recorder::new();
    let mut req = request(&m, &loc);
    req.overrides.clear();
    req.profile = Some(rec.clone());
    assert_eq!(
        also(req, &spec(&loc, &[AlsoTarget::Vanilla]))
            .0
            .unwrap()
            .exit_code(),
        0
    );
    let r = rec.report("also", vec![]);
    for kind in [CallKind::Plan, CallKind::Run] {
        assert_eq!(r.calls.iter().filter(|c| c.kind == kind).count(), 2);
    }
    let jobs: Vec<_> = r
        .jobs
        .iter()
        .filter(|j| j.note.input.ends_with("cup.png"))
        .collect();
    assert_eq!(jobs.len(), 2);
    assert_ne!(jobs[0].id, jobs[1].id);
    assert_ne!(jobs[0].call, jobs[1].call);
    assert!(jobs.iter().all(|j| j.output == "textures/clutter/cup.dds"));
}
