#[allow(
    dead_code,
    reason = "shared by the profile, profile_panic, profile_cancel and profile_session tests; each uses a subset"
)]
mod profile_support;
use engine::{api::CancelToken, profile::Recorder};
use profile_support::*;

#[test]
fn a_cancelled_batch_closes_its_spans_and_the_report_writes() {
    let _hook = Hook::set("SDC_TEST_CANCEL_ON", "wide.png");
    let f = Fixture::new();
    let rec = Recorder::new();
    let cancel = CancelToken::default();
    let summary = run(f.plan("out", Some(&rec), &cancel), &cancel);
    assert!(
        summary.cancelled.iter().any(|p| p.ends_with("wide.png")),
        "{summary:?}"
    );
    let r = written_report(&rec, f.dir.path());
    assert_eq!(r.calls.len(), 2);
    assert!(r.calls.iter().all(|c| c.end_ns >= c.start_ns));
    assert!(!r.spans.is_empty() && r.spans.iter().all(|s| s.4 >= s.3));
}
