//! Note HF9: a file cancelled while planning is listed only when its set is in the run. The `SDC_TEST_CANCEL_ON`
//! hook reads the process environment, so this test has a binary of its own: no sibling test plans while it sets it.
use engine::api::*;
use engine::config::Target;
use engine::events::*;
use engine::overrides::parse_set;
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

fn run(p: Plan) -> BatchSummary {
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    Engine::new()
        .unwrap()
        .run(p, &rep, &CancelToken::default())
        .unwrap()
}

/// The hook cancels just before `hfcancelhook.png`'s Load.
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
    let all = run(plan(request(&d, None)));
    assert!(hook(&all), "the hook cancels it: {all:?}");
    let some = run(plan(request(&d, Some(&["rock"]))));
    assert!(!hook(&some), "outside the run: {some:?}");
}
