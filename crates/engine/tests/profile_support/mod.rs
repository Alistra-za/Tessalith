#[path = "../../src/profile_fixture.rs"]
pub mod profile_fixture;

use engine::api::{BatchRequest, BatchSummary, CancelToken, Engine, OnExisting, Plan};
use engine::config::Target;
use engine::events::{Reporter, VecSink};
use engine::manifest::{output_key, Manifest};
use engine::policy::{AcceptSuggestions, JobStatus};
use engine::profile::{call_of_batch, Call, CallKind, Recorder, Report, Stage};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub expected: BTreeSet<String>,
    /// `BatchRequest::validate` for `plan`; off by default, as on main.
    pub validate: bool,
}
impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let expected = profile_fixture::write_project(&dir.path().join("in"))
            .unwrap()
            .into_iter()
            .map(str::to_owned)
            .collect();
        Self {
            dir,
            expected,
            validate: false,
        }
    }
    pub fn plan(&self, out: &str, recorder: Option<&Recorder>, cancel: &CancelToken) -> Plan {
        let mut req = BatchRequest::new(
            vec![self.dir.path().join("in")],
            self.dir.path().join(out),
            Target::Cs,
        );
        req.recursive = true;
        req.accept = AcceptSuggestions::High;
        req.on_existing = OnExisting::Overwrite;
        req.jobs = 4;
        req.validate = self.validate;
        req.profile = recorder.cloned();
        Engine::new()
            .unwrap()
            .plan(req, &reporter(), cancel)
            .unwrap()
    }
}
fn reporter() -> Reporter {
    Reporter::new(Arc::new(VecSink::default()), "profile")
}
pub fn run(plan: Plan, cancel: &CancelToken) -> BatchSummary {
    Engine::new()
        .unwrap()
        .run(plan, &reporter(), cancel)
        .unwrap()
}

pub type Facts = BTreeMap<String, (bool, bool, serde_json::Value)>;
pub fn facts(plan: &Plan) -> Facts {
    plan.jobs
        .iter()
        .filter(|j| matches!(j.status, JobStatus::Ready))
        .map(|j| {
            (
                output_key(&plan.request.output_root, &j.output),
                (
                    j.pack.is_some(),
                    j.bake.is_some(),
                    serde_json::json!(j.profile.alpha.policy),
                ),
            )
        })
        .collect()
}

pub type Span = (u8, u32, u16, u64, u64, u64, u32);
pub fn spans(r: &Report, stage: Stage) -> impl Iterator<Item = &Span> {
    r.spans.iter().filter(move |s| s.0 == stage as u8)
}
pub fn inside(child: &Span, parent: &Span) -> bool {
    parent.3 <= child.3 && child.4 <= parent.4
}

pub fn call_of_kind(report: &Report, kind: CallKind) -> &Call {
    report
        .calls
        .iter()
        .find(|c| c.kind == kind)
        .expect("missing profile call")
}

fn run_only(stage: Stage) -> bool {
    use Stage::*;
    matches!(
        stage,
        Run | RunAdmitEstimate
            | RunAdmitWait
            | RunClassify
            | Currency
            | CurrencyHash
            | File
            | StampCheck
            | HashSource
            | BakeLoad
            | Decode
            | PackAssemble
            | DeriveAssemble
            | Fill
            | CubePrepare
            | Preprocess
            | Dilate
            | SrgbDecode
            | Resize
            | ResizeRows
            | Mips
            | FilterRows
            | RowJoin
            | Coverage
            | DetailCheck
            | Quantize
            | Encode
            | EncodeGather
            | EncodePack
            | EncodeBand
            | DdsAssemble
            | Validate
            | ManifestRecord
            | Write
            | ManifestWait
            | ManifestAppend
    )
}

pub fn check_spans(r: &Report, out: &Path, planned: &Facts) {
    let mut ordered = r.spans.clone();
    ordered.sort_by_key(|s| (s.2, s.3, std::cmp::Reverse(s.4)));
    let mut ends = vec![];
    let mut thread = None;
    for s in &ordered {
        assert!(s.4 >= s.3 && s.5 <= s.4 - s.3 && s.6 >= 1, "{s:?}");
        if thread != Some(s.2) {
            ends.clear();
            thread = Some(s.2);
        }
        while ends.last().is_some_and(|&end| end <= s.3) {
            ends.pop();
        }
        assert!(
            ends.last().is_none_or(|&end| s.4 <= end),
            "crossing span {s:?} in {ends:?}"
        );
        ends.push(s.4);
    }
    assert_eq!(r.calls.len(), 2);
    let plan = call_of_kind(r, CallKind::Plan);
    let run = call_of_kind(r, CallKind::Run);
    assert!(plan.end_ns <= run.start_ns);
    for call in [plan, run] {
        assert_eq!(call.pool_threads, 4);
        assert!(call.budget_limit > 0 && call.budget_peak > 0);
        let stage = if call.kind == CallKind::Plan {
            Stage::Plan
        } else {
            Stage::Run
        };
        let root = spans(r, stage)
            .find(|s| call_of_batch(s.1) == call.call)
            .unwrap();
        assert!(call.start_ns <= root.3 && root.4 <= call.end_ns);
        let coordinator = |s: &&Span| s.1 == root.1 && s.2 == root.2 && inside(s, root);
        assert!(spans(r, Stage::ScopeJoin)
            .filter(coordinator)
            .any(|join| spans(r, Stage::Dispatch)
                .filter(coordinator)
                .any(|s| inside(s, join))));
        if call.kind == CallKind::Run {
            assert!(spans(r, Stage::RunClassify).any(|s| coordinator(&s)));
            for s in &r.spans {
                let stage = Stage::ALL[s.0 as usize];
                // Shared stages (e.g. event delivery and pool construction) can belong to either call.
                if run_only(stage) || s.4 > plan.end_ns {
                    assert!(inside(s, root), "outside Run span {s:?}");
                }
            }
        }
    }

    assert_eq!(
        r.jobs.iter().map(|j| &j.output).collect::<BTreeSet<_>>(),
        planned.keys().collect()
    );
    assert_eq!(r.jobs.len(), planned.len());
    let records = Manifest::at(out).read().unwrap();
    for job in &r.jobs {
        let files: Vec<_> = spans(r, Stage::File).filter(|s| s.1 == job.id).collect();
        assert_eq!(files.len(), 1, "File for {}", job.output);
        let has = |stage| spans(r, stage).any(|s| s.1 == job.id && inside(s, files[0]));
        let (packed, baked, alpha) = &planned[&job.output];
        assert!(has(if *packed {
            Stage::PackAssemble
        } else {
            Stage::Decode
        }));
        for stage in [
            Stage::Mips,
            Stage::Quantize,
            Stage::Encode,
            Stage::DdsAssemble,
            Stage::Write,
            Stage::ManifestAppend,
        ] {
            assert!(has(stage), "{} missing {stage:?}", job.output);
        }
        // Validation is opt-in (Note LG1): a Validate span exactly when the record's summary says it ran.
        let validation = &records[&job.output].raw["validation"];
        assert!(validation.is_object(), "{} has no validation", job.output);
        let validated = validation["skipped"] != true;
        assert_eq!(
            has(Stage::Validate),
            validated,
            "{} Validate span, validated {validated}",
            job.output
        );
        for (name, stage) in [
            ("crate_rmaos.dds", Stage::BakeLoad),
            ("leaf.dds", Stage::Dilate),
            ("plain.dds", Stage::SrgbDecode),
            ("wide.dds", Stage::Resize),
        ] {
            if job.output.ends_with(name) {
                assert!(has(stage), "{} missing {stage:?}", job.output);
            }
        }
        let dds = engine::dds::read_dds(&std::fs::read(out.join(&job.output)).unwrap()).unwrap();
        if job.output.ends_with("wide.dds") {
            let source = image::image_dimensions(&job.note.input).unwrap();
            assert_eq!((source, (dds.width, dds.height)), ((31, 20), (32, 16)));
        }
        let n = &job.note;
        assert_eq!(
            (n.width, n.height, n.levels),
            (dds.width, dds.height, dds.mip_count)
        );
        assert_eq!((n.packed, n.baked), (*packed, *baked));
        assert_eq!(n.alpha_policy, alpha.as_str().unwrap());
        assert_eq!(job.call, run.call);
    }
}

pub struct Hook(&'static str, Option<OsString>);
impl Hook {
    pub fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var_os(key);
        std::env::set_var(key, value);
        Self(key, old)
    }
}
impl Drop for Hook {
    fn drop(&mut self) {
        if let Some(old) = &self.1 {
            std::env::set_var(self.0, old);
        } else {
            std::env::remove_var(self.0);
        }
    }
}
pub fn written_report(rec: &Recorder, dir: &Path) -> Report {
    let path: PathBuf = rec.write_report(dir, "fixture", vec![]).unwrap();
    serde_json::from_reader(std::fs::File::open(path).unwrap()).unwrap()
}
