#[allow(
    dead_code,
    reason = "shared by the profile, profile_panic, profile_cancel and profile_session tests; each uses a subset"
)]
mod profile_support;
use engine::{
    api::CancelToken,
    profile::{Recorder, Stage},
};
use profile_support::*;

#[test]
fn a_plan_panic_closes_its_spans_and_the_report_writes() {
    let _hook = Hook::set("SDC_TEST_PANIC_ON_PLAN", "leaf.png@analyse");
    let mut f = Fixture::new();
    f.validate = true; // Validation is opt-in (Note LG1); on here, so `check_spans` sees a Validate span for every job.
    let rec = Recorder::new();
    let cancel = CancelToken::default();
    let plan = f.plan("out", Some(&rec), &cancel);
    assert_eq!(plan.failed.len(), 1);
    assert!(plan.failed[0].path.ends_with("leaf.png"));
    let planned = facts(&plan);
    let summary = run(plan, &cancel);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.converted as usize, f.expected.len() - 1);
    let r = written_report(&rec, f.dir.path());
    let leaf = r
        .inputs
        .iter()
        .find(|i| i.path.ends_with("leaf.png"))
        .unwrap();
    assert!(spans(&r, Stage::PlanFile).any(|s| s.1 == leaf.id && s.4 >= s.3));
    check_spans(&r, &f.dir.path().join("out"), &planned);
}
