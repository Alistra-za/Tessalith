//! The process-wide recorder counter makes recorder activity visible, so this is a test binary of its own.
#[allow(dead_code)]
mod profile_support;
use engine::api::{BatchRequest, CancelToken};
use engine::config::Target;
use engine::events::{Reporter, VecSink};
use engine::policy::AcceptSuggestions;
use engine::profile::{recorders_created, CallKind, Report, Stage};
use engine::session::Session;
use profile_support::*;
use std::sync::Arc;

#[test]
fn only_export_creates_a_recorder() {
    let f = Fixture::new();
    let prof = f.dir.path().join("prof");
    std::fs::create_dir(&prof).unwrap();
    let mut session = Session::new().unwrap();
    session.set_profile_out(Some(prof.clone()));
    let req = |out: &str| {
        let mut r = BatchRequest::new(
            vec![f.dir.path().join("in")],
            f.dir.path().join(out),
            Target::Cs,
        );
        r.recursive = true;
        r.accept = AcceptSuggestions::High;
        r.jobs = 2;
        r
    };
    let rep = Reporter::new(Arc::new(VecSink::default()), "session");
    let cancel = CancelToken::default();

    let before = recorders_created();
    let plan = session.plan(req("out_plain"), &rep, &cancel).unwrap();
    session.run(plan, &rep, &cancel).unwrap();
    assert_eq!(recorders_created(), before, "plan and run make no recorder");
    assert_eq!(std::fs::read_dir(&prof).unwrap().count(), 0);

    session.export(req("out_export"), &rep, &cancel).unwrap();
    assert_eq!(recorders_created(), before + 1);
    let files: Vec<_> = std::fs::read_dir(&prof).unwrap().collect();
    assert_eq!(files.len(), 1);
    let path = files[0].as_ref().unwrap().path();
    let r: Report = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    for kind in [CallKind::Plan, CallKind::Run] {
        assert_eq!(r.calls.iter().filter(|c| c.kind == kind).count(), 1);
    }
    assert!(spans(&r, Stage::File).count() > 0);

    // A folder that does not exist yet is created by the export, as the CLI creates it.
    let fresh = f.dir.path().join("new").join("prof");
    session.set_profile_out(Some(fresh.clone()));
    session.export(req("out_fresh"), &rep, &cancel).unwrap();
    assert_eq!(std::fs::read_dir(&fresh).unwrap().count(), 1);
}
