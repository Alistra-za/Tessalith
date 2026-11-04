//! Track A Task 7 (DECISIONS Note CN): `also::run_also` writes `<name> PBR`, `<name> Vanilla` and `<name> CM` side by
//! side, as three engine runs. Fixtures are generated: a PBR set `textures/pbr/arm/steel`, a plain set
//! `textures/clutter/cup`, a PBR landscape set `textures/landscape/dirt` and a complex-material set
//! `textures/clutter/plate` (a raw `_height`: cs packs a `_p`, vanilla a complex `_m`, Note AZ2). The byte-identity test
//! with a BC1 output is `also_export_bc1.rs`.
mod also_support;
use also_support::*;
use engine::also::*;
use engine::api::*;
use engine::config::Target;
use engine::events::*;
use engine::manifest::{Manifest, ManifestRecord};
use engine::overrides::parse_set;
use engine::presets::{AlsoTarget, ExportConfig};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

fn set(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn phase(s: &AlsoSummary, p: Phase) -> &BatchSummary {
    &s.phases.iter().find(|(q, _)| *q == p).expect("phase ran").1
}

fn keys_of(ev: &[EngineEvent], f: impl Fn(&EventKind) -> bool) -> Vec<String> {
    ev.iter()
        .filter(|e| f(&e.kind))
        .filter_map(|e| e.job.clone())
        .collect()
}

#[test]
fn a_vanilla_mode_project_is_refused() {
    let (_d, m, loc) = project(cup);
    let mut req = request(&m, &loc);
    req.target = Some(Target::Vanilla);
    match also(req, &spec(&loc, BOTH)).0 {
        Err(EngineError::Usage(msg)) => assert!(msg.contains("cs mode"), "{msg}"),
        other => panic!("{other:?}"),
    }
    for f in ["Mod PBR", "Mod Vanilla", "Mod CM"] {
        assert!(!loc.join(f).exists(), "{f}");
    }
}

#[test]
fn a_location_containing_the_opened_folder_is_allowed() {
    let (_d, m, loc) = project(cup);
    let s = also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    assert_eq!(
        s.folders,
        [
            loc.join("Mod PBR"),
            loc.join("Mod Vanilla"),
            loc.join("Mod CM")
        ]
    );
    assert!(loc.join("Mod PBR/textures/clutter/cup.dds").exists());
    assert!(loc.join("Mod Vanilla/textures/clutter/cup.dds").exists());

    let mut sp = spec(&loc, BOTH);
    sp.cfg.names.pbr = Some("Mod".into());
    match also(request(&m, &loc), &sp).0 {
        Err(EngineError::Usage(msg)) => assert!(msg.contains("opened folder"), "{msg}"),
        other => panic!("{other:?}"),
    }
}

/// Removes `SDC_TEST_PANIC_ON` when the PBR phase ends (its `batch_done`, the first one), so Vanilla encodes the file.
struct ClearHookOnPbrDone(VecSink);

impl EventSink for ClearHookOnPbrDone {
    fn emit(&self, ev: &EngineEvent) {
        if matches!(ev.kind, EventKind::BatchDone { .. }) {
            std::env::remove_var("SDC_TEST_PANIC_ON");
        }
        self.0.emit(ev);
    }
}

struct HookVar(&'static str);

impl Drop for HookVar {
    fn drop(&mut self) {
        std::env::remove_var(self.0);
    }
}

#[test]
fn a_failed_pbr_encode_is_encoded_again_in_vanilla_not_copied() {
    let (_d, m, loc) = project(|m| {
        cup(m);
        gradient(&m.join("textures/clutter/cup_panic_hook.png"));
    });
    let key = "textures/clutter/cup_panic_hook.dds";
    let (pbr, van) = (loc.join("Mod PBR"), loc.join("Mod Vanilla"));
    // A clean export copies it; then its source changes and the PBR encode fails, leaving a stale PBR file and record.
    also(request(&m, &loc), &spec(&loc, &[AlsoTarget::Vanilla]))
        .0
        .unwrap();
    assert!(record(&van, key).raw.get("copied_from").is_some());
    png(
        &m.join("textures/clutter/cup_panic_hook.png"),
        16,
        16,
        [250, 10, 10, 255],
    );
    std::env::set_var("SDC_TEST_PANIC_ON", "cup_panic_hook.png");
    let _hook = HookVar("SDC_TEST_PANIC_ON");
    let sink = Arc::new(ClearHookOnPbrDone(VecSink::default()));
    let s = also_with(
        request(&m, &loc),
        &spec(&loc, &[AlsoTarget::Vanilla]),
        sink,
        &CancelToken::default(),
    )
    .unwrap();
    assert_eq!(phase(&s, Phase::Pbr).failed, 1, "{s:#?}");
    assert!(pbr.join(key).exists(), "the stale PBR file stays");
    let r = record(&van, key);
    assert!(r.raw.get("copied_from").is_none(), "encoded, not copied");
    assert_ne!(
        std::fs::read(pbr.join(key)).unwrap(),
        std::fs::read(van.join(key)).unwrap()
    );
    assert_eq!(phase(&s, Phase::Vanilla).converted, 1, "{s:#?}");
    assert_eq!(s.copied, 0, "cup and cup_n are up to date");
}

#[test]
fn the_cm_summary_counts_only_derived_items() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
        std::fs::write(m.join("textures/clutter/broken.png"), b"not a png").unwrap();
    });
    let s = also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    assert_eq!(phase(&s, Phase::Pbr).failed, 1, "{s:#?}");
    assert_eq!(phase(&s, Phase::Vanilla).failed, 1, "{s:#?}");
    assert_eq!(phase(&s, Phase::Cm).failed, 0, "{s:#?}");
    assert_eq!(s.combined().failed, 2);
    assert_eq!(s.exit_code(), 1);
}

#[test]
fn three_folders_side_by_side_with_their_own_manifests() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let s = also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    assert_eq!(s.exit_code(), 0, "{s:#?}");
    for f in ["Mod PBR", "Mod Vanilla", "Mod CM"] {
        assert!(loc.join(f).join("_sdc/manifest.jsonl").exists(), "{f}");
    }
    assert_eq!(
        dds_files(&loc.join("Mod Vanilla")),
        set(&[
            "textures/arm/steel.dds",
            "textures/arm/steel_n.dds",
            "textures/clutter/cup.dds",
            "textures/clutter/cup_n.dds"
        ])
    );
    assert_eq!(
        dds_files(&loc.join("Mod CM")),
        set(&["textures/arm/steel_m.dds"])
    );
}

#[test]
fn plain_sets_are_copied_and_their_bytes_match_the_pbr_folder() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let s = also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    assert_eq!(s.copied, 2, "{s:#?}");
    let (pbr, van) = (loc.join("Mod PBR"), loc.join("Mod Vanilla"));
    for f in ["textures/clutter/cup.dds", "textures/clutter/cup_n.dds"] {
        assert_eq!(
            std::fs::read(pbr.join(f)).unwrap(),
            std::fs::read(van.join(f)).unwrap()
        );
        let (twin, copy) = (record(&pbr, f), record(&van, f));
        assert_eq!(
            copy.raw["copied_from"],
            serde_json::to_value(twin.output_stamp).unwrap()
        );
        assert_eq!(
            copy.output_stamp,
            engine::policy::SourceStamp::of(&van.join(f))
        );
        assert_eq!(copy.source_sha256, twin.source_sha256);
    }
}

/// Note FM43: the vanilla presets encode a diffuse perceptually and cs-quality does not, so the Vanilla phase encodes
/// the diffuse itself instead of copying the PBR output. The normal map has the same profile in both and is copied.
#[test]
fn a_perceptual_vanilla_diffuse_is_encoded_not_copied() {
    let (_d, m, loc) = project(cup);
    let mut r = request(&m, &loc);
    r.overrides.clear(); // the cs phase keeps its preset default (perceptual off for a diffuse)
    let s = also(r, &spec(&loc, &[AlsoTarget::Vanilla])).0.unwrap();
    assert_eq!(s.exit_code(), 0, "{s:#?}");
    let (pbr, van) = (loc.join("Mod PBR"), loc.join("Mod Vanilla"));
    let diffuse = "textures/clutter/cup.dds";
    assert!(record(&van, diffuse).raw.get("copied_from").is_none());
    assert_ne!(
        std::fs::read(pbr.join(diffuse)).unwrap(),
        std::fs::read(van.join(diffuse)).unwrap(),
        "the perceptual encode differs from the plain one on a gradient"
    );
    assert!(record(&van, "textures/clutter/cup_n.dds")
        .raw
        .get("copied_from")
        .is_some());
    assert_eq!(s.copied, 1, "{s:#?}");
}

#[test]
fn a_set_whose_vanilla_plan_differs_is_encoded_not_copied() {
    let (_d, m, loc) = project(plate);
    let s = also(request(&m, &loc), &spec(&loc, &[AlsoTarget::Vanilla]))
        .0
        .unwrap();
    assert_eq!(s.exit_code(), 0, "{s:#?}");
    let (pbr, van) = (loc.join("Mod PBR"), loc.join("Mod Vanilla"));
    assert!(pbr.join("textures/clutter/plate_p.dds").exists());
    let m_rec = record(&van, "textures/clutter/plate_m.dds");
    assert!(
        m_rec.raw.get("copied_from").is_none(),
        "the complex _m is encoded"
    );
    assert!(record(&van, "textures/clutter/plate.dds")
        .raw
        .get("copied_from")
        .is_some());
    assert!(!van.join("textures/clutter/plate_p.dds").exists());
    assert_eq!(s.copied, 1);
}

#[test]
fn a_second_export_is_all_up_to_date() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    let (s, ev) = also(request(&m, &loc), &spec(&loc, BOTH));
    let s = s.unwrap();
    for (p, b) in &s.phases {
        assert_eq!(b.converted, 0, "{p:?}: {b:#?}");
    }
    assert_eq!(s.copied, 0);
    assert_eq!(s.exit_code(), 0);
    let up = keys_of(&ev, |k| {
        matches!(
            k,
            EventKind::FileSkipped {
                kind: SkipKind::UpToDate,
                ..
            }
        )
    });
    assert!(
        up.contains(&"Mod Vanilla/textures/clutter/cup.dds".to_string()),
        "{up:?}"
    );
}

#[test]
fn a_1x_twin_without_encode_version_is_re_encoded_and_the_copy_records_the_current_version() {
    let (_d, m, loc) = project(cup);
    let sp = spec(&loc, &[AlsoTarget::Vanilla]);
    also(request(&m, &loc), &sp).0.unwrap();
    let (pbr, van) = (loc.join("Mod PBR"), loc.join("Mod Vanilla"));
    let f = "textures/clutter/cup.dds";
    // Make the PBR record look like a 1.x one: engine 1.0.0 and no encode_version.
    let mut raw = record(&pbr, f).raw;
    raw.as_object_mut().unwrap().remove("encode_version");
    raw["engine_version"] = "1.0.0".into();
    let rec: ManifestRecord = serde_json::from_value(raw).unwrap();
    assert_eq!(rec.encode_version, None);
    Manifest::at(&pbr).append(&rec).unwrap();
    std::fs::remove_file(van.join(f)).unwrap();
    let (s, ev) = also(request(&m, &loc), &sp);
    let s = s.unwrap();
    let failed = keys_of(&ev, |k| matches!(k, EventKind::FileFailed { .. }));
    assert!(failed.is_empty(), "{failed:?} {s:#?}");
    // The 1.x record is a stale baseline, so the PBR twin is re-encoded (and no OUT-UNKNOWN-EXISTING).
    assert_eq!(phase(&s, Phase::Pbr).converted, 1, "{s:#?}");
    assert!(van.join(f).exists(), "the deleted Vanilla file is restored");
    let copy = record(&van, f);
    assert!(copy.raw.get("copied_from").is_some());
    let r: ManifestRecord = serde_json::from_value(copy.raw.clone()).unwrap();
    assert_eq!(
        copy.raw["encode_version"],
        engine::manifest::encode_version(r.profile.subtype)
    );
}
#[test]
fn a_1x_bc1_target_re_copies_the_plain_sets() {
    let (_d, m, loc) = project(cup);
    let mut req = request(&m, &loc);
    req.overrides
        .push(parse_set("file:**/cup.png.format=bc1").unwrap());
    also(req.clone(), &spec(&loc, &[AlsoTarget::Vanilla]))
        .0
        .unwrap();
    // The earlier export is made to look like a 1.x one (encoded against a decoder model): its PBR record says so,
    // and this export must re-encode cup.dds and copy it again.
    let pbr = loc.join("Mod PBR");
    let mut rec: ManifestRecord =
        serde_json::from_value(record(&pbr, "textures/clutter/cup.dds").raw).unwrap();
    rec.bc1_model = Some("ideal".into());
    Manifest::at(&pbr).append(&rec).unwrap();
    let s = also(req, &spec(&loc, &[AlsoTarget::Vanilla])).0.unwrap();
    assert_eq!(phase(&s, Phase::Pbr).converted, 1, "{s:#?}");
    assert_eq!(s.copied, 1, "{s:#?}");
    let f = "textures/clutter/cup.dds";
    assert_eq!(
        std::fs::read(loc.join("Mod PBR").join(f)).unwrap(),
        std::fs::read(loc.join("Mod Vanilla").join(f)).unwrap()
    );
}

#[test]
fn a_landscape_p_is_copied_into_vanilla_when_identical() {
    let (_d, m, loc) = project(dirt);
    let s = also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    assert_eq!(s.exit_code(), 0, "{s:#?}");
    let f = "textures/landscape/dirt_p.dds";
    let van = loc.join("Mod Vanilla");
    assert_eq!(
        std::fs::read(loc.join("Mod PBR").join(f)).unwrap(),
        std::fs::read(van.join(f)).unwrap()
    );
    assert!(record(&van, f).raw.get("copied_from").is_some());
    assert!(!loc.join("Mod CM").join(f).exists());
}

#[test]
fn cm_alone_also_writes_vanilla_and_says_why() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let s = also(request(&m, &loc), &spec(&loc, &[AlsoTarget::Cm]))
        .0
        .unwrap();
    assert!(loc.join("Mod Vanilla/textures/arm/steel.dds").exists());
    assert!(loc.join("Mod CM/textures/arm/steel_m.dds").exists());
    assert!(
        s.notes
            .iter()
            .any(|n| n.contains("CM installs on top of Vanilla")),
        "{:?}",
        s.notes
    );
}

#[test]
fn a_project_without_pbr_sets_writes_no_cm_folder() {
    let (_d, m, loc) = project(cup);
    let s = also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    assert!(!loc.join("Mod CM").exists());
    assert!(s.phases.iter().all(|(p, _)| *p != Phase::Cm));
    assert!(
        s.notes
            .contains(&"no PBR sets: Vanilla holds copies only; no CM folder written".to_string()),
        "{:?}",
        s.notes
    );
}

#[test]
fn vanilla_alone_writes_two_folders() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let s = also(request(&m, &loc), &spec(&loc, &[AlsoTarget::Vanilla]))
        .0
        .unwrap();
    assert!(loc.join("Mod PBR").exists() && loc.join("Mod Vanilla").exists());
    assert!(!loc.join("Mod CM").exists());
    assert_eq!(s.phases.len(), 2);
}

/// Note FM23: an Also export keeps no `also.json`. A fresh export creates none, and a record a 1.x export left (readable
/// or not) is neither read, rewritten nor deleted, and raises no warning.
#[test]
fn a_1x_also_json_is_left_untouched() {
    let (_d, m, loc) = project(cup);
    let s = also(request(&m, &loc), &spec(&loc, &[AlsoTarget::Vanilla]))
        .0
        .unwrap();
    assert_eq!(s.exit_code(), 0, "{:?}", s.notes);
    assert!(!loc.join("Mod PBR/_sdc/also.json").exists());

    for old in [
        &b"{\"c:/x/mod sha256sums.txt\": {\"sha256\": \"ab\"}}"[..],
        &b"{ not json"[..],
    ] {
        let rec = loc.join("Mod PBR/_sdc/also.json");
        std::fs::write(&rec, old).unwrap();
        let (s, ev) = also(request(&m, &loc), &spec(&loc, &[AlsoTarget::Vanilla]));
        let s = s.unwrap();
        assert_eq!(s.exit_code(), 0, "{:?}", s.notes);
        assert_eq!(std::fs::read(&rec).unwrap(), old);
        assert!(
            !s.notes.iter().any(|n| n.contains("also.json")),
            "{:?}",
            s.notes
        );
        assert!(
            !ev.iter().any(|e| matches!(&e.kind,
                EventKind::Warning(w) if w.message.contains("also.json"))),
            "a warning names also.json"
        );
    }
}

/// Review M1: an error of a later phase keeps its class: recorded with its phase, its exit code wins, it is not a
/// cancel, and a warning event names the phase.
#[test]
fn a_later_phase_error_keeps_its_exit_code_and_is_reported() {
    let (_d, m, loc) = project(cup);
    // The Vanilla folder's name is taken by a file: its run cannot create the output root (Fatal, exit 3).
    std::fs::create_dir_all(&loc).unwrap();
    std::fs::write(loc.join("Mod Vanilla"), b"a file").unwrap();
    let (s, ev) = also(request(&m, &loc), &spec(&loc, BOTH));
    let s = s.unwrap();
    assert_eq!(s.cancelled_after, None);
    let (phase, _, code) = s.error.clone().expect("the error is kept");
    assert_eq!((phase, code), (Phase::Vanilla, 3));
    assert_eq!(s.exit_code(), 3);
    assert!(ev.iter().any(|e| matches!(&e.kind,
        EventKind::Warning(w) if w.code == engine::codes::Code::AlsoPhaseFailed && w.message.contains("Vanilla"))));
}

/// Review M2: the copies' events come before the Vanilla phase's `batch_done`.
#[test]
fn copy_events_come_before_the_vanilla_batch_done() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let (s, ev) = also(request(&m, &loc), &spec(&loc, &[AlsoTarget::Vanilla]));
    assert_eq!(s.unwrap().copied, 2);
    let copy = ev
        .iter()
        .position(|e| {
            e.job.as_deref() == Some("Mod Vanilla/textures/clutter/cup.dds")
                && matches!(e.kind, EventKind::FileDone { .. })
        })
        .unwrap();
    let done = ev
        .iter()
        .position(|e| e.batch == ev[copy].batch && matches!(e.kind, EventKind::BatchDone { .. }))
        .unwrap();
    assert!(copy < done, "copy {copy}, batch_done {done}");
}

/// Review M7: a batch-level planning warning (here an unmatched `[files]` key) appears once, not once per phase.
#[test]
fn batch_level_warnings_appear_once() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    std::fs::write(
        m.join("sdc.toml"),
        "[files.\"textures/nothing.png\"]\nalpha.threshold = 90\n",
    )
    .unwrap();
    let (s, ev) = also(request(&m, &loc), &spec(&loc, BOTH));
    assert_eq!(s.unwrap().phases.len(), 3);
    let n = ev
        .iter()
        .filter(|e| {
            matches!(&e.kind,
            EventKind::Warning(w) if w.code == engine::codes::Code::CfgFileUnmatched)
        })
        .count();
    assert_eq!(n, 1);
}

/// Review M3: `same_identity` holds for a cs job and its vanilla clone, and fails when any compared field differs.
#[test]
fn same_identity_fails_when_any_compared_field_differs() {
    use engine::config::{Format, Target};
    use engine::policy::{JobStatus, ResolvedJob};
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
        plate(m);
    });
    let (cs_root, van_root) = (loc.join("Mod PBR"), loc.join("Mod Vanilla"));
    let e = Engine::new().unwrap();
    let rep = Reporter::new(Arc::new(NullSink), "t");
    let cs = e
        .plan(request(&m, &cs_root), &rep, &CancelToken::default())
        .unwrap();
    let job = |p: &engine::api::Plan, tail: &str| -> ResolvedJob {
        p.jobs
            .iter()
            .find(|j| {
                j.output
                    .to_string_lossy()
                    .replace('\\', "/")
                    .ends_with(tail)
            })
            .unwrap_or_else(|| panic!("{tail}"))
            .clone()
    };
    let cup = job(&cs, "textures/clutter/cup.dds");
    let packed = job(&cs, "textures/clutter/plate_p.dds");
    let mut vreq = request(&m, &van_root);
    vreq.target = Some(Target::Vanilla);
    vreq.derive = Some(engine::derive::DeriveRequest {
        mode: engine::derive::DeriveMode::Vanilla,
        params: Default::default(),
        sets: derive_sets(&cs, &BTreeSet::new()).0,
        drop: vec![],
        scanned: false,
    });
    let vp = e.plan(vreq, &rep, &CancelToken::default()).unwrap();
    let derived = job(&vp, "textures/arm/steel.dds");
    assert!(derived.derive.is_some());

    let mut van = cup.clone();
    van.profile.target = Target::Vanilla;
    van.output = van_root.join("textures/clutter/cup.dds");
    assert!(same_identity(&cup, &cs_root, &van, &van_root));
    type Flip = Box<dyn Fn(&mut ResolvedJob)>;
    let flips: Vec<(&str, Flip)> = vec![
        ("format", Box::new(|j| j.profile.format = Format::Bc1)),
        (
            "input",
            Box::new(|j| j.input = j.input.with_file_name("other.png")),
        ),
        (
            "output key",
            Box::new(|j| j.output = j.output.with_file_name("other.dds")),
        ),
        (
            "status",
            Box::new(|j| j.status = JobStatus::Blocked { codes: vec![] }),
        ),
        (
            "rnm",
            Box::new(|j| {
                j.rnm = Some(engine::rnm::RnmJob {
                    layers: vec![],
                    ao: None,
                })
            }),
        ),
        (
            "bake",
            Box::new(|j| {
                j.bake = Some(engine::bake::BakeJob {
                    normal: j.input.clone(),
                    normal_profile: j.profile.clone(),
                    shift: 0,
                    stamp: None,
                    rnm: None,
                })
            }),
        ),
    ];
    for (what, flip) in &flips {
        let mut v = van.clone();
        flip(&mut v);
        assert!(!same_identity(&cup, &cs_root, &v, &van_root), "{what}");
    }
    let mut v = van.clone();
    v.pack = packed.pack.clone();
    assert!(!same_identity(&cup, &cs_root, &v, &van_root), "pack");
    let mut v = van.clone();
    v.derive = derived.derive.clone();
    assert!(!same_identity(&cup, &cs_root, &v, &van_root), "derive");
    let mut c = cup.clone();
    c.status = JobStatus::Blocked { codes: vec![] };
    assert!(!same_identity(&c, &cs_root, &van, &van_root), "cs status");
}

#[test]
fn an_unknown_file_in_the_vanilla_folder_is_kept() {
    let (_d, m, loc) = project(cup);
    let mine = loc.join("Mod Vanilla/textures/clutter/cup.dds");
    std::fs::create_dir_all(mine.parent().unwrap()).unwrap();
    std::fs::write(&mine, b"mine").unwrap();
    let (s, ev) = also(request(&m, &loc), &spec(&loc, &[AlsoTarget::Vanilla]));
    let s = s.unwrap();
    assert_eq!(std::fs::read(&mine).unwrap(), b"mine");
    assert_eq!(s.copied, 1, "cup_n only");
    let key = "Mod Vanilla/textures/clutter/cup.dds";
    assert!(ev.iter().any(|e| e.job.as_deref() == Some(key)
        && matches!(&e.kind, EventKind::Warning(w) if w.code == engine::codes::Code::OutUnknownExisting)));
    assert!(keys_of(&ev, |k| matches!(
        k,
        EventKind::FileSkipped {
            kind: SkipKind::UnknownExisting,
            ..
        }
    ))
    .contains(&key.to_string()));
    assert_eq!(s.exit_code(), 1);
}

#[test]
fn the_opened_folder_is_refused_as_a_location_and_a_location_inside_it_is_not_discovered() {
    let (_d, m, _loc) = project(cup);
    match also(request(&m, &m), &spec(&m, BOTH)).0 {
        Err(EngineError::Usage(msg)) => assert!(msg.contains("opened folder"), "{msg}"),
        other => panic!("{other:?}"),
    }
    assert!(!m.join("Mod PBR").exists());

    let inside = m.join("out");
    also(request(&m, &inside), &spec(&inside, BOTH)).0.unwrap();
    assert!(inside.join("Mod Vanilla/textures/clutter/cup.dds").exists());
    let (s, ev) = also(request(&m, &inside), &spec(&inside, BOTH));
    assert_eq!(s.unwrap().exit_code(), 0);
    let dds_skip = ev.iter().any(|e| match &e.kind {
        EventKind::Warning(w) => w.code == engine::codes::Code::InDdsSkipped,
        EventKind::FileSkipped { code, .. } => *code == Some(engine::codes::Code::InDdsSkipped),
        _ => false,
    });
    assert!(!dds_skip);
    assert!(ev
        .iter()
        .all(|e| !e.path.as_deref().is_some_and(|p| p.contains("/out/"))));
}

#[test]
fn a_skipped_set_is_copied_nowhere_and_noted() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let mut sp = spec(&loc, BOTH);
    sp.cfg.skip = vec!["textures/pbr/arm/steel".into()];
    let s = also(request(&m, &loc), &sp).0.unwrap();
    assert!(loc.join("Mod PBR/textures/pbr/arm/steel.dds").exists());
    for f in ["Mod Vanilla", "Mod CM"] {
        let files = dds_files(&loc.join(f));
        assert!(files.iter().all(|f| !f.contains("steel")), "{f}: {files:?}");
    }
    assert_eq!(
        s.sets,
        vec![SetNote {
            id: "textures/pbr/arm/steel".into(),
            why: "skipped (Skip vanilla/CM for this set)".into()
        }]
    );
}

#[test]
fn vanilla_and_cm_job_keys_carry_their_folder_prefix() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let (s, ev) = also(request(&m, &loc), &spec(&loc, BOTH));
    s.unwrap();
    let done = keys_of(&ev, |k| matches!(k, EventKind::FileDone { .. }));
    for k in [
        "textures/clutter/cup.dds",
        "Mod Vanilla/textures/clutter/cup.dds",
        "Mod Vanilla/textures/arm/steel.dds",
        "Mod CM/textures/arm/steel_m.dds",
    ] {
        assert!(done.contains(&k.to_string()), "{k}: {done:?}");
    }
    // The CM phase keeps only the derived items: no event of the plain set.
    assert!(ev
        .iter()
        .filter_map(|e| e.job.as_deref())
        .all(|k| !k.starts_with("Mod CM/textures/clutter")));
}

#[test]
fn a_cs_only_set_override_fails_before_anything_is_written() {
    let (_d, m, loc) = project(cup);
    let mut req = request(&m, &loc);
    req.overrides.push(parse_set("format=bc6h_uf16").unwrap());
    let e = also(req, &spec(&loc, BOTH)).0.unwrap_err();
    assert_eq!(e.exit_code(), 2, "{e}");
    for f in ["Mod PBR", "Mod Vanilla", "Mod CM"] {
        assert!(!loc.join(f).exists(), "{f}");
    }
}

/// Cancels on the PBR phase's `batch_done`.
struct CancelOnPbrDone(CancelToken);

impl EventSink for CancelOnPbrDone {
    fn emit(&self, ev: &EngineEvent) {
        if matches!(ev.kind, EventKind::BatchDone { .. }) {
            self.0.cancel();
        }
    }
}

#[test]
fn cancel_after_the_pbr_phase_stops_the_rest() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let cancel = CancelToken::default();
    let s = also_with(
        request(&m, &loc),
        &spec(&loc, BOTH),
        Arc::new(CancelOnPbrDone(cancel.clone())),
        &cancel,
    )
    .unwrap();
    assert_eq!(s.cancelled_after, Some(Phase::Pbr));
    assert_eq!(s.phases.len(), 1);
    assert!(!loc.join("Mod Vanilla").exists());
    assert_eq!(s.exit_code(), 1);
}

// ---- the folder-name rule (Task 6 review: pairwise with the real project name, Windows names) ----

#[test]
fn a_custom_name_equal_to_another_folders_default_is_refused() {
    let (_d, m, loc) = project(cup);
    let mut sp = spec(&loc, BOTH);
    sp.cfg.names.cm = Some("mod vanilla".into());
    match also(request(&m, &loc), &sp).0 {
        Err(EngineError::Config(msg)) => assert!(msg.contains("export.names.cm"), "{msg}"),
        other => panic!("{other:?}"),
    }
    assert!(!loc.join("Mod PBR").exists());
}

#[test]
fn windows_illegal_and_reserved_folder_names_are_refused_by_key() {
    for (key, bad) in [
        ("pbr", "a:b"),
        ("vanilla", "what?"),
        ("cm", "x|y"),
        ("pbr", "CON"),
        ("vanilla", "nul.txt"),
        ("cm", "Com1"),
        ("cm", "LPT9"),
    ] {
        let mut c = ExportConfig::default();
        match key {
            "pbr" => c.names.pbr = Some(bad.into()),
            "vanilla" => c.names.vanilla = Some(bad.into()),
            _ => c.names.cm = Some(bad.into()),
        }
        let e = c.check().unwrap_err();
        assert!(e.contains(&format!("export.names.{key}")), "{bad}: {e}");
    }
    let mut c = ExportConfig::default();
    c.names.cm = Some("CONSOLE".into());
    c.check().unwrap();
}

#[test]
fn check_location_rules() {
    let d = tempfile::tempdir().unwrap();
    let m = d.path().join("mods/Mod");
    let loc = d.path().join("mods");
    let folders = |l: &Path| [l.join("Mod PBR"), l.join("Mod Vanilla"), l.join("Mod CM")];
    check_location(std::slice::from_ref(&m), &loc, &folders(&loc)).unwrap();
    assert!(check_location(std::slice::from_ref(&m), &m, &folders(&m)).is_err());
    let mut f = folders(&loc);
    f[1] = d.path().join("mods");
    assert!(check_location(std::slice::from_ref(&m), &loc, &f).is_err());
    let inside = m.join("out");
    check_location(std::slice::from_ref(&m), &inside, &folders(&inside)).unwrap();
    assert!(export_folder_problem(&m, &loc).is_some());
    assert!(export_folder_problem(&m, &m.join("x")).is_none());
}

#[test]
fn prefix_sink_prefixes_job_keys_only() {
    let inner = Arc::new(VecSink::default());
    let rep = Reporter::new(
        Arc::new(PrefixSink::new("Mod CM/".into(), inner.clone())),
        "b",
    );
    rep.emit_job(Some("textures/a.dds"), None, None, EventKind::FileStarted);
    rep.emit(None, None, EventKind::FileStarted);
    let ev = inner.take();
    assert_eq!(ev[0].job.as_deref(), Some("Mod CM/textures/a.dds"));
    assert_eq!(ev[1].job, None);
}

#[test]
fn derive_sets_skip_by_id() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let r = request(&m, &loc.join("Mod PBR"));
    let p = Engine::new()
        .unwrap()
        .plan(
            r,
            &Reporter::new(Arc::new(NullSink), "t"),
            &CancelToken::default(),
        )
        .unwrap();
    let (sets, drop, notes) = derive_sets(&p, &BTreeSet::new());
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].id, "textures/pbr/arm/steel");
    assert!(drop.is_empty() && notes.is_empty());
    let (sets, drop, notes) = derive_sets(&p, &set(&["Textures/PBR/Arm/Steel"]));
    assert!(sets.is_empty());
    assert_eq!(drop.len(), 3);
    assert_eq!(notes.len(), 1);
}

/// Backlog T26: a PBR member that fails to load leaves its set out of the derive, with a note.
#[test]
fn derive_sets_leaves_out_a_set_with_a_failed_member() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
        std::fs::write(m.join("textures/pbr/arm/steel_rmaos.png"), b"not a png").unwrap();
    });
    let p = Engine::new()
        .unwrap()
        .plan(
            request(&m, &loc.join("Mod PBR")),
            &Reporter::new(Arc::new(NullSink), "t"),
            &CancelToken::default(),
        )
        .unwrap();
    assert_eq!(p.failed.len(), 1, "{:?}", p.failed);
    let (sets, drop, notes) = derive_sets(&p, &BTreeSet::new());
    assert!(sets.is_empty(), "{:?}", sets.iter().map(|s| &s.id).collect::<Vec<_>>());
    assert!(!drop.is_empty());
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "textures/pbr/arm/steel");
    assert!(notes[0].why.contains("failed in the PBR export"), "{}", notes[0].why);
}

// ---- Note FM: no zip, no sums file, no report; files the run did not write are noted ----

/// Every name in `loc` (not below it) but the opened folder `Mod`.
fn names_in(loc: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(loc)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != "Mod")
        .collect();
    v.sort();
    v
}

#[test]
fn also_export_writes_no_zip_or_report() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let s = also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    assert_eq!(names_in(&loc), ["Mod CM", "Mod PBR", "Mod Vanilla"]);
    assert_eq!(s.phases.len(), 3);
    assert_eq!(s.exit_code(), 0, "{:?}", s.notes);
    assert!(loc.join("Mod CM/textures/arm/steel_m.dds").exists());
    // A second, all up-to-date export writes nothing else either.
    also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    assert_eq!(names_in(&loc), ["Mod CM", "Mod PBR", "Mod Vanilla"]);
}

/// Final review I2: a file written into the PBR folder between the phases and the walk (the GUI's JSON) and passed as
/// `extra` is this run's, with no note; one not passed is noted. A path outside the PBR folder is ignored.
#[test]
fn extra_files_written_after_the_phases_are_not_noted() {
    let (_d, m, loc) = project(steel);
    let sp = spec(&loc, &[AlsoTarget::Vanilla]);
    let start = || {
        start_also(
            &Engine::new().unwrap(),
            &engine::session::AnalysisCache::default(),
            request(&m, &loc),
            &sp,
            Arc::new(VecSink::default()),
            &CancelToken::default(),
        )
        .unwrap()
    };
    let json = loc.join("Mod PBR/PBRNIFPatcher/Mod.json");
    let note = format!("Mod PBR/PBRNIFPatcher/Mod.json: {NOT_WRITTEN}");
    let run = start();
    std::fs::create_dir_all(json.parent().unwrap()).unwrap();
    std::fs::write(&json, b"{}").unwrap();
    let outside = loc.join("elsewhere.json");
    let s = finish_also(run, &[json.clone(), outside], &CancelToken::default());
    assert!(!s.notes.contains(&note), "{:?}", s.notes);
    assert_eq!(s.exit_code(), 0, "{:?}", s.notes);
    let s = finish_also(start(), &[], &CancelToken::default());
    assert!(s.notes.contains(&note), "{:?}", s.notes);
}

/// Final review I1b (Note GI27): a file the GUI writes into the Vanilla folder after the phases (the XPMF profile copy)
/// and passes as `extra` is this run's, as the PBR folder's is.
#[test]
fn extra_files_in_the_vanilla_folder_are_not_noted() {
    let (_d, m, loc) = project(steel);
    let sp = spec(&loc, &[AlsoTarget::Vanilla]);
    let run = start_also(
        &Engine::new().unwrap(),
        &engine::session::AnalysisCache::default(),
        request(&m, &loc),
        &sp,
        Arc::new(VecSink::default()),
        &CancelToken::default(),
    )
    .unwrap();
    let json = loc.join("Mod Vanilla/SKSE/Plugins/XPMF/tessalith_steel.json");
    std::fs::create_dir_all(json.parent().unwrap()).unwrap();
    std::fs::write(&json, b"{}").unwrap();
    let s = finish_also(run, &[json], &CancelToken::default());
    assert!(
        !s.notes.iter().any(|n| n.contains("tessalith_steel")),
        "{:?}",
        s.notes
    );
    assert_eq!(s.exit_code(), 0, "{:?}", s.notes);
}

/// Final review I3 (Note FM: only the `NOT_WRITTEN` note is left): a file the run neither wrote nor found up to date
/// is kept and noted, in the folder that ran.
#[test]
fn a_file_this_run_did_not_write_is_kept_and_noted() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    let mut sp = spec(&loc, BOTH);
    sp.cfg.legacy.keep_metal_brightness = true;
    also(request(&m, &loc), &sp).0.unwrap();
    let stale = "Mod CM/textures/arm/steel.dds";
    assert!(loc.join(stale).exists(), "the keep-metal CM diffuse");
    let unknown = "Mod Vanilla/textures/x.dds";
    std::fs::write(loc.join(unknown), b"mine").unwrap();

    sp.cfg.legacy.keep_metal_brightness = false;
    let s = also(request(&m, &loc), &sp).0.unwrap();
    assert_eq!(std::fs::read(loc.join(unknown)).unwrap(), b"mine");
    assert!(loc.join(stale).exists(), "never deleted");
    for f in [unknown, stale] {
        let want = format!("{f}: {NOT_WRITTEN}");
        assert!(s.notes.contains(&want), "{want}: {:?}", s.notes);
    }
    assert!(
        !s.notes.iter().any(|n| n.contains("steel_m.dds")),
        "{:?}",
        s.notes
    );
}

/// A cancel or a phase error leaves most files unreached, so none is noted.
#[test]
fn an_unfinished_export_notes_no_unwritten_file() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        cup(m);
    });
    also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    std::fs::write(loc.join("Mod Vanilla/textures/x.dds"), b"mine").unwrap();
    let cancel = CancelToken::default();
    let s = also_with(
        request(&m, &loc),
        &spec(&loc, BOTH),
        Arc::new(CancelOnPbrDone(cancel.clone())),
        &cancel,
    )
    .unwrap();
    assert!(
        !s.notes.iter().any(|n| n.ends_with(NOT_WRITTEN)),
        "{:?}",
        s.notes
    );
    assert_eq!(s.exit_code(), 1);

    // A cancel after the phases, before the walk.
    let cancel = CancelToken::default();
    let run = start_also(
        &Engine::new().unwrap(),
        &engine::session::AnalysisCache::default(),
        request(&m, &loc),
        &spec(&loc, BOTH),
        Arc::new(VecSink::default()),
        &cancel,
    )
    .unwrap();
    cancel.cancel();
    let s = finish_also(run, &[], &cancel);
    assert!(
        !s.notes.iter().any(|n| n.ends_with(NOT_WRITTEN)),
        "{:?}",
        s.notes
    );
}

/// A dry run writes nothing and notes nothing.
#[test]
fn a_dry_run_writes_no_folder_and_notes_nothing() {
    let (_d, m, loc) = project(cup);
    let mut req = request(&m, &loc);
    req.dry_run = true;
    let s = also(req, &spec(&loc, &[AlsoTarget::Vanilla])).0.unwrap();
    assert!(!loc.exists() || names_in(&loc).is_empty());
    assert!(
        s.notes.iter().all(|n| !n.ends_with(NOT_WRITTEN)),
        "{:?}",
        s.notes
    );
}

/// Task 13 M3 / final review I3: the CM text names the complex-grass stacks when a PBR grass set is exported.
#[test]
fn the_cm_text_names_the_grass_stacks() {
    let mut c = ExportConfig::default();
    assert_eq!(
        cm_lines(&c, false),
        [
            "Install the CM folder on top of the Vanilla folder.",
            "It holds only the `_m` files."
        ]
    );
    assert!(cm_lines(&c, true)[1].contains("complex-grass"));
    c.legacy.keep_metal_brightness = true;
    assert_eq!(cm_lines(&c, true).len(), 1);
    assert_eq!(
        cm_lines(&c, false),
        vec!["Install the CM folder on top of the Vanilla folder."]
    );
}

fn fake_dds(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"DDS not a real texture").unwrap();
}

/// A DDS-only PBR set `textures/pbr/arm/iron` (DERIVE-DDS-SOURCE).
fn iron(m: &Path) {
    for f in ["iron.dds", "iron_n.dds", "iron_rmaos.dds"] {
        fake_dds(&m.join("textures/pbr/arm").join(f));
    }
}

/// A set the Vanilla plan skips whole (here a DDS-only one) is a `SetNote` with the plan's reason; the others have none.
#[test]
fn a_set_the_vanilla_plan_skips_whole_is_noted() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        iron(m);
        cup(m);
    });
    let s = also(request(&m, &loc), &spec(&loc, BOTH)).0.unwrap();
    let iron = s
        .sets
        .iter()
        .find(|n| n.id.ends_with("textures/pbr/arm/iron"))
        .unwrap_or_else(|| panic!("{:?}", s.sets));
    assert!(iron.why.contains("is DDS"), "{iron:?}");
    assert!(
        s.sets.iter().all(|n| !n.id.ends_with("steel")),
        "{:?}",
        s.sets
    );
}

// ---- Task 9 fix round 1 (I1, M4, N1) ----

/// I1: a size mismatch only the CM `_m` meets (a `_p` of another aspect ratio) reaches the CM run: one `FileFailed`, a failed
/// count and a partial exit.
#[test]
fn a_cm_only_size_mismatch_fails_in_the_cm_phase() {
    let (_d, m, loc) = project(|m| {
        steel(m);
        png(
            &m.join("textures/pbr/arm/steel_p.png"),
            32,
            16,
            [100, 100, 100, 255],
        );
    });
    let (s, ev) = also(request(&m, &loc), &spec(&loc, BOTH));
    let s = s.unwrap();
    assert_eq!(phase(&s, Phase::Vanilla).failed, 0, "{s:#?}");
    assert_eq!(phase(&s, Phase::Cm).failed, 1, "{s:#?}");
    let failed: Vec<String> = keys_of(&ev, |k| matches!(k, EventKind::FileFailed { .. }))
        .into_iter()
        .filter(|k| k.starts_with("Mod CM/"))
        .collect();
    assert_eq!(failed, ["Mod CM/textures/arm/steel_m.dds"], "{ev:#?}");
    // Every CM job key ends once.
    let mut ends = std::collections::BTreeMap::<String, u32>::new();
    for k in keys_of(&ev, |k| {
        matches!(
            k,
            EventKind::FileDone { .. }
                | EventKind::FileSkipped { .. }
                | EventKind::FileFailed { .. }
        )
    }) {
        if k.starts_with("Mod CM/") {
            *ends.entry(k).or_default() += 1;
        }
    }
    assert!(ends.values().all(|&n| n == 1), "{ends:?}");
    assert_eq!(s.exit_code(), 1);
}

/// N1: a PBR run that fails is recorded with its phase and exit code (here the PBR folder's name is taken by a file).
#[test]
fn a_failed_pbr_run_is_recorded_not_returned() {
    let (_d, m, loc) = project(cup);
    std::fs::write(loc.join("Mod PBR"), b"a file").unwrap();
    let (s, ev) = also(request(&m, &loc), &spec(&loc, BOTH));
    let s = s.unwrap();
    let (p, _, code) = s.error.clone().expect("the error is kept");
    assert_eq!((p, code), (Phase::Pbr, 3));
    assert_eq!(s.exit_code(), 3);
    assert!(!loc.join("Mod Vanilla").exists());
    assert!(ev.iter().any(|e| matches!(&e.kind,
        EventKind::Warning(w) if w.code == engine::codes::Code::AlsoPhaseFailed && w.message.contains("Pbr"))));
}

/// Final review I1: a set's `_curves` member reaches the Vanilla diffuse (spec step 3, CN3), and is not reported as
/// not converted.
#[test]
fn a_curves_member_changes_the_vanilla_diffuse() {
    let diffuse = |curves: bool| {
        let (_d, m, loc) = project(|m| {
            steel(m);
            if curves {
                png(
                    &m.join("textures/pbr/arm/steel_curves.png"),
                    16,
                    16,
                    [240, 240, 240, 255],
                );
            }
        });
        let (s, ev) = also(request(&m, &loc), &spec(&loc, &[AlsoTarget::Vanilla]));
        assert_eq!(s.unwrap().exit_code(), 0);
        let not_converted = ev.iter().any(|e| match &e.kind {
            EventKind::Warning(w) => w.code == engine::codes::Code::DeriveNotConverted,
            EventKind::FileSkipped { code, .. } => {
                *code == Some(engine::codes::Code::DeriveNotConverted)
            }
            _ => false,
        });
        assert!(!not_converted, "{curves}: {ev:#?}");
        std::fs::read(loc.join("Mod Vanilla/textures/arm/steel.dds")).unwrap()
    };
    assert_ne!(diffuse(false), diffuse(true));
}
