//! Fast re-plan equivalence (R2 §6): a partial plan of the sets an edit reaches equals the next full plan of the
//! edited project, restricted to the partial's scope; and that scope holds every set the edit changes.
#[path = "common/fast_fixture.rs"]
mod fast_fixture;

use engine::api::{BatchRequest, CancelToken, Plan};
use engine::events::{EngineEvent, EventKind, Reporter, VecSink};
use engine::session::{Partial, Session};
use engine::skeleton::{
    BAD_ENTRY, CANCELLED, DERIVE, DUPLICATE_CHANGE, HELPER_ROLE_CHANGE, HITS_NOTHING,
    MEMBERSHIP_CHANGE, PBR_CHANGE, PBR_SUBTYPE, RECIPE_CHANGE, REENCODE_CHANGE, WRITER_CLASH,
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
    /// `(key, entry)`; `None` removes the key. An entry replaces the whole `[files."<key>"]` table.
    edits: Vec<(String, Option<toml::Table>)>,
    expect: Expect,
    /// Set ids the scope must hold.
    scope_has: &'static [&'static str],
}

/// A row; each entry is TOML text, parsed once here.
fn row(
    name: &'static str,
    edits: &[(&'static str, Option<&'static str>)],
    expect: Expect,
    scope_has: &'static [&'static str],
) -> Row {
    Row {
        name,
        edits: edits
            .iter()
            .map(|(k, e)| (k.to_string(), e.map(|s| toml::from_str(s).unwrap())))
            .collect(),
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
        row(
            "one face of cube s, whose merged output clashes with s.png",
            &[(
                "dir/s_px",
                Some("subtype = \"cubemap\"\nmips.filter = \"kaiser\""),
            )],
            Scoped,
            &["dir/s", "dir/s_px", "dir/s_nz"],
        ),
        row(
            "the set the merged cube s clashes with",
            &[("dir/s.png", Some("alpha.policy = \"cutout\""))],
            Scoped,
            &["dir/s", "dir/s_px", "dir/s_nz"],
        ),
        row(
            "shared roughness e pulls q",
            &[("dir/e_metal.png", Some("mips.filter = \"kaiser\""))],
            Scoped,
            &["dir/e", "dir/q"],
        ),
        row(
            "a corrupt source (a load failure in scope)",
            &[("dir/z.png", Some("alpha.policy = \"cutout\""))],
            Scoped,
            &["dir/z"],
        ),
        // Expected to fall back (R2 §6).
        row(
            "a set in the base's BV2 collision",
            &[("dir/o.png", Some("alpha.policy = \"cutout\""))],
            NeedsFull(WRITER_CLASH),
            &[],
        ),
        row(
            "the other set of that collision",
            &[("dir/n.png", Some("alpha.policy = \"cutout\""))],
            NeedsFull(WRITER_CLASH),
            &[],
        ),
        row(
            "packing recipe renamed",
            &[("dir/e_rfaos", Some("packing.recipe = \"rfaos\""))],
            NeedsFull(RECIPE_CHANGE),
            &[],
        ),
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

fn edited(base: &BatchRequest, edits: &[(String, Option<toml::Table>)]) -> BatchRequest {
    let mut r = base.clone();
    for (k, e) in edits {
        r = with_entry(&r, k, e.as_ref());
    }
    r
}

/// A session that planned the base fixture in full, and the base request it planned.
fn planned_session() -> (fast_fixture::FastFixture, Session, BatchRequest) {
    let fx = fast_fixture();
    let session = Session::new().unwrap();
    let base_req = fx.req.clone();
    (fx, session, base_req)
}

#[test]
fn partial_equals_next_full_plan() {
    let (_fx, session, base_req) = planned_session();
    let cancel = CancelToken::default();
    let (rep, sink) = reporter();
    let base = session
        .plan_and_keep(base_req.clone(), &rep, &cancel)
        .unwrap();
    assert!(base.cancelled.is_empty());
    let base_sets = by_set(&base, &sink.take());
    let root = &base_req.output_root;
    let id = |k: &String| engine::sets::set_id(root, k);
    let mut failures_compared = 0;

    for r in rows() {
        let req = edited(&base_req, &r.edits);
        let (prep, psink) = reporter();
        let partial = session.replan_sets(&req, &r.edits, &prep, &cancel);
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
        let scope = &partial.scope;
        let ids: BTreeSet<String> = scope.iter().map(id).collect();
        // The partial holds its scope only: nothing outside it, and every scope set resolved.
        let plan = &partial.plan;
        assert!(
            plan.jobs.iter().all(|j| scope.contains(&j.set_key))
                && plan
                    .skipped
                    .iter()
                    .all(|s| s.set.as_ref().is_some_and(|k| scope.contains(k)))
                && plan.helpers.iter().all(|h| scope.contains(&h.set_key)),
            "{}: the partial plans outside its scope",
            r.name
        );
        let pevents = psink.take();
        let resolved: BTreeSet<String> = pevents
            .iter()
            .filter(|e| matches!(e.kind, EventKind::SetResolved { .. }))
            .filter_map(|e| e.set.clone())
            .collect();
        assert_eq!(&resolved, scope, "{}: resolved sets", r.name);
        assert!(
            pevents
                .iter()
                .filter(|e| matches!(e.kind, EventKind::FilePlanned { .. }))
                .all(|e| e.set.as_ref().is_some_and(|k| scope.contains(k))),
            "{}: a planned file outside the scope",
            r.name
        );
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
        let got = restricted(&partial.plan, &pevents, scope);
        let want = restricted(&full, &fevents, scope);
        // Every failure of the partial is a scope file's, so the restriction keeps them all.
        assert_eq!(
            got["failed"].as_array().map_or(0, Vec::len),
            plan.failed.len(),
            "{}: a failure outside the scope",
            r.name
        );
        if !plan.failed.is_empty() {
            failures_compared += 1;
        }
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
    assert!(
        failures_compared > 0,
        "no row compared a non-empty failed list"
    );
}

/// Minor 2: a cancelled partial is never returned as a plan.
#[test]
fn a_cancelled_partial_needs_a_full_plan() {
    let (_fx, session, base_req) = planned_session();
    let (rep, _) = reporter();
    session
        .plan_and_keep(base_req.clone(), &rep, &CancelToken::default())
        .unwrap();
    let edit = vec![(
        "dir/a.png".to_string(),
        Some(toml::from_str("alpha.policy = \"cutout\"").unwrap()),
    )];
    let cancel = CancelToken::default();
    cancel.cancel();
    let got = session.replan_sets(&edited(&base_req, &edit), &edit, &rep, &cancel);
    assert!(
        matches!(got, Err(Partial::NeedsFull(why)) if why == CANCELLED),
        "{got:?}"
    );
}

/// Note HE7: a sub-type on one of two raw twins makes it write, so which of them is a duplicate changes.
#[test]
fn a_sub_type_edit_on_a_twin_plans_in_full() {
    let (fx, session, base_req) = planned_session();
    let d = fx.dir.path().join("in/dir");
    for (name, v) in [
        ("t.png", 5u8),
        ("t_rough.png", 6),
        ("t_ao.png", 7),
        ("t_ao.tga", 8),
    ] {
        image::ImageBuffer::from_fn(16, 16, |x, _| {
            image::Rgb([v.wrapping_add((x * 5) as u8); 3])
        })
        .save(d.join(name))
        .unwrap();
    }
    let cancel = CancelToken::default();
    let (rep, _) = reporter();
    session
        .plan_and_keep(base_req.clone(), &rep, &cancel)
        .unwrap();
    let edit = vec![(
        "dir/t_ao.png".to_string(),
        Some(toml::from_str("subtype = \"diffuse\"").unwrap()),
    )];
    let req = edited(&base_req, &edit);
    let got = session.replan_sets(&req, &edit, &rep, &cancel);
    assert!(
        matches!(got, Err(Partial::NeedsFull(why)) if why == DUPLICATE_CHANGE),
        "{got:?}"
    );
    // The full plan the GUI then runs equals a fresh one.
    let kept = session.plan_and_keep(req.clone(), &rep, &cancel).unwrap();
    let fresh = Session::new().unwrap().plan(req, &rep, &cancel).unwrap();
    let json = |p: &Plan| {
        (
            serde_json::to_value(&p.jobs).unwrap(),
            format!("{:?}", p.skipped),
        )
    };
    assert_eq!(json(&kept), json(&fresh));
}

/// An Advanced skin set `y` (`y_sk`, raw `y_roughness` and `y_ao` with `packing.recipe = rfaos`) with two skin `_f`
/// twins, planned in full; the skin `_f` has no file-name raw role, so `typed` never holds it.
fn skin_twins() -> (fast_fixture::FastFixture, Session, BatchRequest) {
    let (fx, session, base) = planned_session();
    let d = fx.dir.path().join("in/dir");
    for (name, v) in [
        ("y.png", 5u8),
        ("y_sk.png", 6),
        ("y_roughness.png", 7),
        ("y_ao.png", 8),
        ("y_f.png", 9),
        ("y_f.tga", 10),
    ] {
        image::ImageBuffer::from_fn(16, 16, |x, _| {
            image::Rgb([v.wrapping_add((x * 5) as u8); 3])
        })
        .save(d.join(name))
        .unwrap();
    }
    let rfaos: toml::Table = toml::from_str("packing.recipe = \"rfaos\"").unwrap();
    let req = with_entry(&base, "dir/y_rmaos", Some(&rfaos));
    let (rep, _) = reporter();
    let plan = session
        .plan_and_keep(req.clone(), &rep, &CancelToken::default())
        .unwrap();
    assert!(
        plan.skipped
            .iter()
            .all(|s| s.code != Some(engine::codes::Code::InDuplicate)
                || !s.path.to_string_lossy().contains("y_f")),
        "the skin twins start as no duplicates"
    );
    (fx, session, req)
}

/// The patch must give the verdict of the next full plan: `NeedsFull(DUPLICATE_CHANGE)`, and the full plan then
/// lists exactly `dups` of the `y_f` twins as `IN-DUPLICATE`.
fn assert_dup_change(
    session: &Session,
    kept: &BatchRequest,
    edit: &[(String, Option<toml::Table>)],
    dups: usize,
) {
    let (rep, _) = reporter();
    let cancel = CancelToken::default();
    let req = edited(kept, edit);
    let got = session.replan_sets(&req, edit, &rep, &cancel);
    assert!(
        matches!(got, Err(Partial::NeedsFull(why)) if why == DUPLICATE_CHANGE),
        "{got:?}"
    );
    let full = session.plan_and_keep(req, &rep, &cancel).unwrap();
    let n = full
        .skipped
        .iter()
        .filter(|s| {
            s.code == Some(engine::codes::Code::InDuplicate)
                && s.path.to_string_lossy().contains("y_f")
        })
        .count();
    assert_eq!(n, dups);
}

fn sub_type(key: &str) -> (String, Option<toml::Table>) {
    (
        key.to_string(),
        Some(toml::from_str("subtype = \"diffuse\"").unwrap()),
    )
}

/// Note HE7: a sub-type on a skin `_f` twin moves it in or out of the comparison with no `typed` flip.
#[test]
fn a_sub_type_on_a_skin_f_twin_plans_in_full() {
    let (_fx, session, base) = skin_twins();
    let (rep, _) = reporter();
    let cancel = CancelToken::default();
    // One twin typed: it writes beside the other, which still feeds the fuzz channel. Not a duplicate.
    let a = vec![sub_type("dir/y_f.png")];
    let req_a = edited(&base, &a);
    session.plan_and_keep(req_a.clone(), &rep, &cancel).unwrap();
    // The second typed: both write one output.
    assert_dup_change(&session, &req_a, &[sub_type("dir/y_f.tga")], 2);
}

/// Note HE7: typing the raw maps unmakes the skin set, so its `_f` twins become PBR writers.
#[test]
fn typing_a_skin_sets_raw_maps_plans_in_full() {
    let (_fx, session, base) = skin_twins();
    assert_dup_change(
        &session,
        &base,
        &[sub_type("dir/y_roughness.png"), sub_type("dir/y_ao.png")],
        2,
    );
}

/// Minor 3: a request carrying an Also export's derive step falls back before any patch work.
#[test]
fn a_derive_request_needs_a_full_plan() {
    let (_fx, session, base_req) = planned_session();
    let mut req = base_req.clone();
    req.derive = Some(engine::derive::DeriveRequest {
        mode: engine::derive::DeriveMode::Vanilla,
        params: Default::default(),
        sets: vec![],
        drop: vec![],
        scanned: false,
    });
    let (rep, _) = reporter();
    // No full plan yet: the derive check comes before the skeleton is even looked at.
    let got = session.replan_sets(&req, &[], &rep, &CancelToken::default());
    assert!(
        matches!(got, Err(Partial::NeedsFull(why)) if why == DERIVE),
        "{got:?}"
    );
}

/// The sanity timing (not a gate): `cargo test -p engine --release --test fast_replan -- --ignored --nocapture`.
#[test]
#[ignore]
fn replan_timing() {
    let (_fx, session, base_req) = planned_session();
    let cancel = CancelToken::default();
    let (rep, _) = reporter();
    let t = Instant::now();
    session
        .plan_and_keep(base_req.clone(), &rep, &cancel)
        .unwrap();
    let cold = t.elapsed();
    let edit = vec![(
        "dir/a.png".to_string(),
        Some(toml::from_str("alpha.policy = \"cutout\"").unwrap()),
    )];
    let req = edited(&base_req, &edit);
    let t = Instant::now();
    session.replan_sets(&req, &edit, &rep, &cancel).unwrap();
    let partial = t.elapsed();
    let t = Instant::now();
    session
        .engine()
        .plan_cached(req, &rep, &cancel, session.cache())
        .unwrap();
    let warm = t.elapsed();
    eprintln!("cold full plan {cold:?}; replan_sets {partial:?}; warm full plan {warm:?}");
}
