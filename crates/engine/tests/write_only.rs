//! Note HF9: `BatchRequest::write_only` writes and reports only the listed sets; `None` is today's run.
use engine::api::*;
use engine::config::Target;
use engine::events::*;
use engine::manifest::Manifest;
use engine::overrides::parse_set;
use engine::policy::JobStatus;
use image::{ImageBuffer, Rgb};
use std::sync::Arc;
use tempfile::TempDir;

const NAMES: [&str; 3] = ["rock", "wood", "iron"];

fn fixture() -> TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("in")).unwrap();
    for (n, name) in NAMES.iter().enumerate() {
        ImageBuffer::from_fn(16, 16, |x, y| {
            Rgb([(x * 5) as u8 + n as u8 * 40, (y * 3) as u8, 90])
        })
        .save(d.path().join("in").join(format!("{name}.png")))
        .unwrap();
    }
    d
}

fn request(d: &TempDir, only: Option<&[&str]>) -> BatchRequest {
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
    r.recursive = true;
    r.overrides = NAMES
        .iter()
        .map(|n| parse_set(&format!("file:**/{n}.png.subtype=diffuse")).unwrap())
        .collect();
    r.write_only = only.map(|names| {
        names
            .iter()
            .map(|n| engine::sets::set_key(&r.output_root, n))
            .collect()
    });
    r
}

fn plan(r: BatchRequest) -> Plan {
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap()
}

/// Run `p`, returning the summary and every event.
fn run(p: Plan) -> (BatchSummary, Vec<EngineEvent>) {
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "t");
    let s = Engine::new()
        .unwrap()
        .run(p, &rep, &CancelToken::default())
        .unwrap();
    (s, sink.take())
}

fn out(d: &TempDir, name: &str) -> std::path::PathBuf {
    d.path().join("out").join(format!("{name}.dds"))
}

#[test]
fn write_only_writes_only_the_listed_sets() {
    let d = fixture();
    let (s, _) = run(plan(request(&d, Some(&["rock"]))));
    assert!(out(&d, "rock").is_file());
    assert!(!out(&d, "wood").exists());
    assert!(!out(&d, "iron").exists());
    assert_eq!(s.converted, 1, "{s:?}");
}

#[test]
fn the_summary_and_events_cover_only_them() {
    let d = fixture();
    let mut p = plan(request(&d, Some(&["rock"])));
    // A blocked and an unresolved job outside the list: neither is counted nor reported.
    for j in &mut p.jobs {
        if j.input.ends_with("wood.png") {
            j.status = JobStatus::Blocked { codes: vec![] };
        } else if j.input.ends_with("iron.png") {
            j.status = JobStatus::Unresolved {
                reasons: vec!["undecided".into()],
                needs: vec![],
            };
        }
    }
    let (s, events) = run(p);
    assert_eq!((s.converted, s.skipped, s.failed), (1, 0, 0), "{s:?}");
    let named: Vec<&str> = events.iter().filter_map(|e| e.path.as_deref()).collect();
    assert!(
        named
            .iter()
            .all(|p| !p.contains("wood") && !p.contains("iron")),
        "{named:?}"
    );
    assert!(named.iter().any(|p| p.contains("rock")), "{named:?}");
}

#[test]
fn none_is_todays_run() {
    let d = fixture();
    let (s, _) = run(plan(request(&d, None)));
    assert_eq!(s.converted, 3, "{s:?}");
    assert!(NAMES.iter().all(|n| out(&d, n).is_file()));
}

#[test]
fn an_entry_with_no_set_stays() {
    let d = fixture();
    let mut p = plan(request(&d, Some(&["rock"])));
    // A project-wide fact (an input duplicate) has no set: it is reported whatever the list.
    p.skipped.push(SkippedFile {
        path: d.path().join("in").join("rock.tga"),
        output: out(&d, "rock"),
        set: None,
        code: Some(engine::codes::Code::InDuplicate),
        reason: "two inputs write one output".into(),
    });
    let (s, events) = run(p);
    assert_eq!(s.skipped, 1, "{s:?}");
    assert!(events
        .iter()
        .any(|e| e.path.as_deref().is_some_and(|p| p.ends_with("rock.tga"))));
}

/// Each output's manifest record (its raw line), by output key.
fn records(d: &TempDir) -> std::collections::BTreeMap<String, serde_json::Value> {
    Manifest::at(&d.path().join("out"))
        .read()
        .unwrap()
        .into_iter()
        .map(|(k, e)| (k, e.raw))
        .collect()
}

#[test]
fn manifest_records_outside_the_run_stay() {
    let d = fixture();
    run(plan(request(&d, None)));
    let before = records(&d);
    assert_eq!(before.len(), 3, "{before:?}");
    // A changed source in the run is written again; the other sets' records are untouched.
    ImageBuffer::from_fn(16, 16, |x, _| Rgb([x as u8 * 9, 7, 7]))
        .save(d.path().join("in").join("rock.png"))
        .unwrap();
    let (s, _) = run(plan(request(&d, Some(&["rock"]))));
    assert_eq!(s.converted, 1, "{s:?}");
    let after = records(&d);
    assert_eq!(after.len(), 3, "{after:?}");
    let key = |n: &str| {
        before
            .keys()
            .find(|k| k.ends_with(&format!("{n}.dds")))
            .unwrap()
            .clone()
    };
    for n in ["wood", "iron"] {
        assert_eq!(before[&key(n)], after[&key(n)], "{n}");
    }
    assert_ne!(before[&key("rock")], after[&key("rock")]);
}

#[test]
fn the_skeleton_signature_ignores_it() {
    let d = fixture();
    let all = request(&d, None);
    let some = request(&d, Some(&["rock"]));
    assert!(some.write_only.is_some());
    assert_eq!(all.sig_without_files(), some.sig_without_files());
}

/// Note HF9: a file cancelled while planning is listed only when its set is in the run. The hook cancels just before
/// `hfcancelhook.png`'s Load, a name no other test here uses.
#[test]
fn files_cancelled_while_planning_outside_the_run_are_not_listed() {
    struct Unset;
    impl Drop for Unset {
        fn drop(&mut self) {
            std::env::remove_var("SDC_TEST_CANCEL_ON");
        }
    }
    let d = fixture();
    ImageBuffer::from_fn(16, 16, |x, y| Rgb([(x * 9) as u8, (y * 9) as u8, 9]))
        .save(d.path().join("in").join("hfcancelhook.png"))
        .unwrap();
    std::env::set_var("SDC_TEST_CANCEL_ON", "hfcancelhook.png");
    let _unset = Unset;
    let hook = |s: &BatchSummary| s.cancelled.iter().any(|p| p.ends_with("hfcancelhook.png"));
    let (all, _) = run(plan(request(&d, None)));
    assert!(hook(&all), "the hook cancels it: {all:?}");
    let (some, _) = run(plan(request(&d, Some(&["rock"]))));
    assert!(!hook(&some), "outside the run: {some:?}");
}
