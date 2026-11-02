//! Fast re-plan equivalence (R2 §6): a partial plan of the sets an edit reaches equals the next full plan of the
//! edited project, restricted to the partial's scope; and that scope holds every set the edit changes.
#[path = "common/fast_fixture.rs"]
mod fast_fixture;

use engine::api::{BatchRequest, CancelToken, Plan};
use engine::events::{EngineEvent, EventKind, Reporter, VecSink};
use engine::session::{Partial, Session};
use engine::skeleton::{
    BAD_ENTRY, HELPER_ROLE_CHANGE, HITS_NOTHING, MEMBERSHIP_CHANGE, PBR_CHANGE, PBR_SUBTYPE,
    REENCODE_CHANGE,
};
use fast_fixture::{fast_fixture, with_entry};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, PartialEq)]
enum Expect {
    Scoped,
    NeedsFull(&'static str),
}
use Expect::*;

struct Row {
    name: &'static str,
    /// `(key, entry as TOML)`; `None` removes the key. An entry replaces the whole `[files."<key>"]` table.
    edits: Vec<(&'static str, Option<&'static str>)>,
    expect: Expect,
    /// Set ids the scope must hold.
    scope_has: &'static [&'static str],
}

fn row(
    name: &'static str,
    edits: &[(&'static str, Option<&'static str>)],
    expect: Expect,
    scope_has: &'static [&'static str],
) -> Row {
    Row {
        name,
        edits: edits.to_vec(),
        expect,
        scope_has,
    }
}

const FACES: [&str; 6] = ["px", "nx", "py", "ny", "pz", "nz"];

fn rows() -> Vec<Row> {
    let cube_six: Vec<(&'static str, Option<&'static str>)> = FACES
        .iter()
        .map(|f| {
            let k: &'static str = Box::leak(format!("dir/c_{f}").into_boxed_str());
            (k, Some("subtype = \"cubemap\"\nmips.filter = \"kaiser\""))
        })
        .collect();
    vec![
        // Expected to scope (R2 §6).
        row(
            "alpha.policy on a (borrowed by i)",
            &[("dir/a.png", Some("alpha.policy = \"cutout\""))],
            Scoped,
            &["dir/a", "dir/i"],
        ),
        row(
            "alpha.policy on the borrower i",
            &[("dir/i.png", Some("alpha.policy = \"cutout\""))],
            Scoped,
            &["dir/a", "dir/i"],
        ),
        row(
            "format on d",
            &[("dir/d.png", Some("format = \"bc1\""))],
            Scoped,
            &["dir/d"],
        ),
        row(
            "mips on b",
            &[("dir/b.png", Some("mips.filter = \"kaiser\""))],
            Scoped,
            &["dir/b"],
        ),
        row(
            "subtype on d_n",
            &[("dir/d_n.png", Some("subtype = \"msn\""))],
            Scoped,
            &["dir/d"],
        ),
        row(
            "subtype on a raw map of non-PBR q",
            &[("dir/q_height.png", Some("subtype = \"diffuse\""))],
            Scoped,
            &["dir/q"],
        ),
        // Demoted from R2's Scoped list: `e` packs `_rmaos`, so it is a PBR set and P1 applies.
        row(
            "subtype on e_ao",
            &[("dir/e_ao.png", Some("subtype = \"diffuse\""))],
            NeedsFull(PBR_SUBTYPE),
            &[],
        ),
        row(
            "one cube face",
            &[(
                "dir/c_px",
                Some("subtype = \"cubemap\"\nmips.filter = \"kaiser\""),
            )],
            Scoped,
            &[
                "dir/c_px", "dir/c_nx", "dir/c_py", "dir/c_ny", "dir/c_pz", "dir/c_nz",
            ],
        ),
        row(
            "all six cube faces (DT31)",
            &cube_six,
            Scoped,
            &["dir/c_px", "dir/c_nz"],
        ),
        row(
            "cube face key removed",
            &[("dir/c_px", None)],
            Scoped,
            &["dir/c_px", "dir/c_nz"],
        ),
        row(
            "rnm.target on the layer",
            &[("dir/d_detailnorm.png", Some("rnm.target = \"n\""))],
            Scoped,
            &["dir/d"],
        ),
        row(
            "packing field on the packed key e_p",
            &[("dir/e_p", Some("packing.fill.specular = 64"))],
            Scoped,
            &["dir/e"],
        ),
        row(
            "helper fill field (not the role)",
            &[(
                "dir/b_mask",
                Some("helper.role = \"opacity\"\nhelper.invert = true"),
            )],
            Scoped,
            &["dir/b"],
        ),
        row(
            "allow codes",
            &[("dir/g.png", Some("allow = [\"IN-DUPLICATE\"]"))],
            Scoped,
            &["dir/g"],
        ),
        row(
            "key removed (q_rough's sibling)",
            &[("dir/q_height.png", None)],
            Scoped,
            &["dir/q"],
        ),
        row(
            "stem f over .png and .dds",
            &[("dir/f", Some("alpha.policy = \"cutout\""))],
            Scoped,
            &["dir/f"],
        ),
        row(
            "set-edit key k (moved into j)",
            &[("dir/k.png", Some("alpha.policy = \"cutout\""))],
            Scoped,
            &["dir/j"],
        ),
        row(
            "alpha on the skin set h",
            &[("dir/h.png", Some("alpha.policy = \"cutout\""))],
            Scoped,
            &["dir/h"],
        ),
        row(
            "mixed-case key",
            &[("DIR/MIXEDCASE", Some("alpha.policy = \"cutout\""))],
            Scoped,
            &["dir/mixedcase"],
        ),
        // Expected to fall back (R2 §6).
        row(
            "subtype on PBR a_n (P1)",
            &[("dir/a_n.png", Some("subtype = \"normal\""))],
            NeedsFull(PBR_SUBTYPE),
            &[],
        ),
        row(
            "reencode on f.dds",
            &[("dir/f.dds", Some("reencode = true"))],
            NeedsFull(REENCODE_CHANGE),
            &[],
        ),
        row(
            "helper.role removed",
            &[("dir/b_mask", None)],
            NeedsFull(HELPER_ROLE_CHANGE),
            &[],
        ),
        row(
            "skin recipe that moves h_wet",
            &[("dir/h_rmaos", None)],
            NeedsFull(MEMBERSHIP_CHANGE),
            &[],
        ),
        row(
            "unmatched key",
            &[("dir/zzz.png", Some("alpha.policy = \"cutout\""))],
            NeedsFull(HITS_NOTHING),
            &[],
        ),
        // The full plan refuses this value up front (a config error), so the partial must not plan it.
        row(
            "value of the wrong type",
            &[("dir/e_p", Some("packing.fill.specular = 0.25"))],
            NeedsFull(BAD_ENTRY),
            &[],
        ),
        row(
            "q turns PBR",
            &[("dir/q_rough", None)],
            NeedsFull(PBR_CHANGE),
            &[],
        ),
    ]
}

fn reporter() -> (Reporter, Arc<VecSink>) {
    let sink = Arc::new(VecSink::default());
    (Reporter::new(sink.clone(), "t"), sink)
}

/// A sorted, comparable form of the `SetResolved` and `FilePlanned` events of the sets `keep` accepts.
fn set_events(events: &[EngineEvent], keep: impl Fn(&str) -> bool) -> Vec<String> {
    let mut v: Vec<String> = events
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                EventKind::SetResolved { .. } | EventKind::FilePlanned { .. }
            )
        })
        .filter(|e| e.set.as_deref().is_some_and(&keep))
        .map(|e| {
            json!({ "set": e.set, "path": e.path, "job": e.job, "kind": serde_json::to_value(&e.kind).unwrap() })
                .to_string()
        })
        .collect();
    v.sort();
    v
}

/// Each set's jobs (sorted by input) and `SetResolved`/`FilePlanned` events, by set key.
fn by_set(plan: &Plan, events: &[EngineEvent]) -> BTreeMap<String, (Vec<Value>, Vec<String>)> {
    let mut m: BTreeMap<String, (Vec<Value>, Vec<String>)> = BTreeMap::new();
    let mut jobs: Vec<_> = plan.jobs.iter().collect();
    jobs.sort_by(|a, b| a.input.cmp(&b.input));
    for j in jobs {
        m.entry(j.set_key.clone())
            .or_default()
            .0
            .push(serde_json::to_value(j).unwrap());
    }
    let keys: BTreeSet<String> = events.iter().filter_map(|e| e.set.clone()).collect();
    for k in keys {
        m.entry(k.clone()).or_default().1 = set_events(events, |s| s == k);
    }
    m
}

/// The parts of `plan` that belong to the sets in `scope`, in a comparable form: jobs (sorted by input), skipped
/// and failed files, helpers, and the `SetResolved` (pack reports included) and `FilePlanned` events.
fn restricted(plan: &Plan, events: &[EngineEvent], scope: &BTreeSet<String>) -> Value {
    let mut jobs: Vec<_> = plan
        .jobs
        .iter()
        .filter(|j| scope.contains(&j.set_key))
        .collect();
    jobs.sort_by(|a, b| a.input.cmp(&b.input));
    let mut skipped: Vec<String> = plan
        .skipped
        .iter()
        .filter(|s| s.set.as_ref().is_some_and(|k| scope.contains(k)))
        .map(|s| format!("{s:?}"))
        .collect();
    skipped.sort();
    // A failed file carries no set: matched by the paths of the scope's jobs, skips and set members.
    let scope_paths: BTreeSet<String> = events
        .iter()
        .filter(|e| e.set.as_deref().is_some_and(|k| scope.contains(k)))
        .filter_map(|e| match &e.kind {
            EventKind::SetResolved { members, .. } => Some(members.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    let mut failed: Vec<String> = plan
        .failed
        .iter()
        .filter(|f| scope_paths.contains(&f.path.to_string_lossy().replace('\\', "/")))
        .map(|f| format!("{f:?}"))
        .collect();
    failed.sort();
    let mut helpers: Vec<String> = plan
        .helpers
        .iter()
        .filter(|h| scope.contains(&h.set_key))
        .map(|h| format!("{h:?}"))
        .collect();
    helpers.sort();
    json!({
        "jobs": jobs.iter().map(|j| serde_json::to_value(j).unwrap()).collect::<Vec<_>>(),
        "skipped": skipped,
        "failed": failed,
        "helpers": helpers,
        "events": set_events(events, |k| scope.contains(k)),
    })
}

fn edited(base: &BatchRequest, edits: &[(&str, Option<&str>)]) -> BatchRequest {
    let mut r = base.clone();
    for (k, e) in edits {
        let t: Option<toml::Table> = e.map(|s| toml::from_str(s).unwrap());
        r = with_entry(&r, k, t.as_ref());
    }
    r
}

#[test]
fn partial_equals_next_full_plan() {
    let fx = fast_fixture();
    let session = Session::new(encoders_sys::Bc1Model::Ideal).unwrap();
    let cancel = CancelToken::default();
    let mut base_req = fx.req.clone();
    base_req.bc1_model = session.bc1_model();
    let (rep, sink) = reporter();
    let t = Instant::now();
    let base = session.plan(base_req.clone(), &rep, &cancel).unwrap();
    let full_ms = t.elapsed().as_secs_f64() * 1e3;
    assert!(base.cancelled.is_empty());
    let base_sets = by_set(&base, &sink.take());
    let root = &base_req.output_root;
    let id = |k: &String| engine::sets::set_id(root, k);
    let mut timed = None;

    for r in rows() {
        let req = edited(&base_req, &r.edits);
        let changed: Vec<(String, Option<toml::Table>)> = r
            .edits
            .iter()
            .map(|(k, e)| (k.to_string(), e.map(|s| toml::from_str(s).unwrap())))
            .collect();
        let (prep, psink) = reporter();
        let t = Instant::now();
        let partial = session.replan_sets(&req, &changed, &prep, &cancel);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let partial = match (partial, &r.expect) {
            (Ok(p), Scoped) => p,
            (Err(Partial::NeedsFull(why)), NeedsFull(want)) => {
                assert_eq!(why, *want, "{}", r.name);
                continue;
            }
            (Ok(p), e) => panic!(
                "{}: scoped {:?}, expected {e:?}",
                r.name,
                p.scope.iter().map(id).collect::<Vec<_>>()
            ),
            (Err(Partial::NeedsFull(why)), e) => {
                panic!("{}: NeedsFull({why}), expected {e:?}", r.name)
            }
            (Err(Partial::Err(e)), _) => panic!("{}: {e}", r.name),
        };
        timed.get_or_insert(ms);
        let scope = &partial.scope;
        let ids: BTreeSet<String> = scope.iter().map(id).collect();
        for want in r.scope_has {
            assert!(
                ids.contains(*want),
                "{}: scope {ids:?} lacks {want}",
                r.name
            );
        }
        let (frep, fsink) = reporter();
        let full = session
            .engine()
            .plan_cached(req.clone(), &frep, &cancel, session.cache())
            .unwrap();
        let fevents = fsink.take();
        let pevents = psink.take();
        let got = restricted(&partial.plan, &pevents, scope);
        let want = restricted(&full, &fevents, scope);
        if got != want {
            for part in ["jobs", "skipped", "failed", "helpers", "events"] {
                if got[part] != want[part] {
                    eprintln!(
                        "{}: {part} differ\n partial: {:#}\n full:    {:#}",
                        r.name, got[part], want[part]
                    );
                }
            }
            panic!("{}: partial != full restricted to {ids:?}", r.name);
        }
        // Every set the edit changed is in the scope: a missing partner shows here.
        let full_sets = by_set(&full, &fevents);
        let changed_sets: BTreeSet<String> = base_sets
            .keys()
            .chain(full_sets.keys())
            .filter(|k| base_sets.get(*k) != full_sets.get(*k))
            .cloned()
            .collect();
        let missing: Vec<String> = changed_sets.difference(scope).map(id).collect();
        assert!(
            missing.is_empty(),
            "{}: sets {missing:?} changed but are not in scope {ids:?}",
            r.name
        );
    }
    eprintln!(
        "timing (this build): full Session::plan {full_ms:.1} ms, first replan_sets {:.1} ms",
        timed.unwrap_or_default()
    );
}
