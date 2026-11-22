//! Note LG: `BatchRequest::validate` off skips the post-encode check (spec §5.8). Validation only reads the bytes,
//! so the written DDS is the same either way; the record keeps a `validation` object that says `skipped`.
use engine::api::*;
use engine::config::{Format, Target};
use engine::events::{EngineEvent, EventKind, Reporter, SkipKind, VecSink};
use engine::manifest::Manifest;
use engine::policy::{AcceptSuggestions, SourceStamp};
use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One BC1, one BC4, one BC6H and BC7 outputs (one of them a cutout), none of an always-validated type.
const SETS: [&str; 10] = [
    "file:**/stone.png.subtype=diffuse",
    "file:**/stone.png.format=bc7",
    "file:**/noise.png.subtype=diffuse",
    "file:**/wall.png.subtype=diffuse",
    "file:**/wall.png.format=bc1",
    "file:**/mask.png.subtype=gray",
    "file:**/sky.hdr.subtype=hdr_sky",
    "file:**/leaf.png.subtype=diffuse",
    "file:**/leaf.png.alpha.policy=cutout",
    "file:**/leaf.png.format=bc7",
];

fn put(d: &Path, name: &str, img: DynamicImage) {
    let p = d.join("in/textures/a").join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    img.save(p).unwrap();
}

fn rgb64() -> DynamicImage {
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(64, 64, |x, y| {
        let n = x.wrapping_mul(73) ^ y.wrapping_mul(151);
        Rgb([(n % 200) as u8 + 20, (x * 3) as u8 + 10, (y * 2) as u8 + 40])
    }))
}

fn fixture(d: &Path) {
    put(d, "stone.png", rgb64());
    let mut state = 12345u32;
    let mut next = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        (state >> 24) as u8
    };
    put(
        d,
        "noise.png",
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(64, 64, |_, _| {
            Rgb([next(), next(), next()])
        })),
    );
    put(d, "wall.png", rgb64());
    put(d, "mask.png", rgb64());
    put(
        d,
        "sky.hdr",
        DynamicImage::ImageRgb32F(ImageBuffer::from_fn(64, 64, |x, y| {
            Rgb([4.0 * x as f32 / 63.0, 2.0 * y as f32 / 63.0, 0.25])
        })),
    );
    // A ~30 % cutout: every alpha 0 or 255.
    put(
        d,
        "leaf.png",
        DynamicImage::ImageRgba8(ImageBuffer::from_fn(64, 64, |x, y| {
            let a = if (x * 7 + y * 13) % 10 < 3 { 255 } else { 0 };
            Rgba([60, (x * 3) as u8 + 20, (y * 2) as u8 + 30, a])
        })),
    );
    put(d, "rock_n.png", {
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(64, 64, |x, _| {
            let t = (x as f32 / 64.0 - 0.5) * 0.8;
            Rgb([
                ((t.sin() * 0.5 + 0.5) * 255.0) as u8,
                128,
                ((t.cos() * 0.5 + 0.5) * 255.0) as u8,
            ])
        }))
    });
}

/// Plans `d/in` into `d/<out>` and runs it; returns the summary and the output root.
fn convert(
    d: &Path,
    out: &str,
    validate: bool,
    on_existing: OnExisting,
) -> (BatchSummary, PathBuf) {
    let req = request(d, out, validate, on_existing);
    let root = req.output_root.clone();
    (run(req).0, root)
}

fn request(d: &Path, out: &str, validate: bool, on_existing: OnExisting) -> BatchRequest {
    let mut req = BatchRequest::new(vec![d.join("in")], d.join(out), Target::Cs);
    req.recursive = true;
    req.accept = AcceptSuggestions::High;
    req.on_existing = on_existing;
    req.validate = validate;
    req.overrides = SETS
        .iter()
        .map(|s| engine::overrides::parse_set(s).unwrap())
        .collect();
    req
}

fn run(req: BatchRequest) -> (BatchSummary, Vec<EngineEvent>) {
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "optional-validate");
    let e = Engine::new().unwrap();
    let plan = e.plan(req, &rep, &CancelToken::default()).unwrap();
    let s = e.run(plan, &rep, &CancelToken::default()).unwrap();
    (s, sink.take())
}

/// Every `.dds` below `root`, keyed by its path relative to `root`.
fn dds_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(|e| e.unwrap())
        .filter(|e| e.path().extension().is_some_and(|x| x == "dds"))
        .map(|e| {
            let rel = e.path().strip_prefix(root).unwrap().to_path_buf();
            (rel, std::fs::read(e.path()).unwrap())
        })
        .collect()
}

fn records(root: &Path) -> BTreeMap<String, Value> {
    Manifest::at(root)
        .read()
        .unwrap()
        .into_iter()
        .map(|(k, e)| (k, e.raw))
        .collect()
}

/// `raw` without the fields that differ between two runs of the same plan, or with validation on and off.
fn comparable(raw: &Value) -> Value {
    let mut v = raw.clone();
    let o = v.as_object_mut().unwrap();
    for k in ["ts", "output_stamp", "validation"] {
        o.remove(k);
    }
    v
}

#[test]
fn skipping_validation_writes_the_same_bytes() {
    let d = tempfile::tempdir().unwrap();
    fixture(d.path());
    let (on, root_on) = convert(d.path(), "out-on", true, OnExisting::Overwrite);
    let (off, root_off) = convert(d.path(), "out-off", false, OnExisting::Overwrite);
    for s in [&on, &off] {
        assert_eq!((s.failed, s.validation_failed), (0, 0), "{s:?}");
    }
    assert_eq!(on.converted, off.converted);

    let (files_on, files_off) = (dds_files(&root_on), dds_files(&root_off));
    assert!(!files_on.is_empty());
    assert_eq!(
        files_on.keys().collect::<Vec<_>>(),
        files_off.keys().collect::<Vec<_>>()
    );
    for (rel, bytes) in &files_on {
        assert!(bytes == &files_off[rel], "{} differs", rel.display());
    }
    let formats: BTreeSet<String> = files_on
        .values()
        .map(|b| {
            let (f, _) = engine::dds::read_dds(b)
                .unwrap()
                .format
                .as_format()
                .unwrap();
            format!("{f:?}")
        })
        .collect();
    for f in [Format::Bc1, Format::Bc4, Format::Bc6hUf16, Format::Bc7] {
        assert!(
            formats.contains(&format!("{f:?}")),
            "{f:?} not in {formats:?}"
        );
    }

    let (rec_on, rec_off) = (records(&root_on), records(&root_off));
    assert_eq!(
        rec_on.keys().collect::<Vec<_>>(),
        rec_off.keys().collect::<Vec<_>>()
    );
    assert_eq!(rec_on.len(), files_on.len());
    assert!(
        rec_on
            .values()
            .any(|r| r["profile"]["alpha"]["policy"] == "cutout"),
        "no cutout output"
    );
    for (key, on) in &rec_on {
        let off = &rec_off[key];
        let st = on["profile"]["subtype"].as_str().unwrap();
        assert!(
            st != "complex_material" && st != "complex_grass",
            "{key}: {st} is always validated"
        );
        assert_eq!(comparable(on), comparable(off), "{key}");
        assert_eq!(
            on["validation"]["levels"], off["validation"]["levels"],
            "{key}"
        );
        assert_eq!(off["validation"]["skipped"], true, "{key}: {off}");
        assert_eq!(off["validation"]["passed"], true, "{key}: {off}");
        assert!(on["validation"].get("skipped").is_none(), "{key}: {on}");
    }
}

/// The engine default: a request straight from `BatchRequest::new`, with `validate` left alone, does not validate.
#[test]
fn a_request_from_new_does_not_validate() {
    let d = tempfile::tempdir().unwrap();
    fixture(d.path());
    let mut req = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
    req.recursive = true;
    req.accept = AcceptSuggestions::High;
    req.overrides = SETS
        .iter()
        .map(|s| engine::overrides::parse_set(s).unwrap())
        .collect();
    let rep = Reporter::new(Arc::new(VecSink::default()), "optional-validate");
    let e = Engine::new().unwrap();
    let plan = e.plan(req, &rep, &CancelToken::default()).unwrap();
    let root = plan.request.output_root.clone();
    let s = e.run(plan, &rep, &CancelToken::default()).unwrap();
    assert!(s.converted > 0, "{s:?}");
    let recs = records(&root);
    assert!(!recs.is_empty());
    for (key, r) in &recs {
        assert_eq!(r["validation"]["skipped"], true, "{key}: {r}");
    }
}

fn skips(events: &[EngineEvent]) -> BTreeMap<&str, &str> {
    events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::FileSkipped {
                kind: SkipKind::UpToDate,
                reason,
                ..
            } => Some((e.job.as_deref().unwrap(), reason.as_str())),
            _ => None,
        })
        .collect()
}

fn warnings<'a>(events: &'a [EngineEvent], key: &str) -> Vec<&'a engine::codes::Warning> {
    events
        .iter()
        .filter(|e| e.job.as_deref() == Some(key))
        .filter_map(|e| match &e.kind {
            EventKind::Warning(w) => Some(w),
            _ => None,
        })
        .collect()
}

#[test]
fn a_validating_run_checks_a_skipped_output_in_place() {
    let d = tempfile::tempdir().unwrap();
    fixture(d.path());
    let req = |out, validate| request(d.path(), out, validate, OnExisting::IfChanged);
    let out = d.path().join("out");
    let first = run(req("out", false)).0;
    let before = records(&out);
    let bytes = dds_files(&out);
    let stamps: BTreeMap<_, _> = before
        .keys()
        .map(|key| (key, SourceStamp::of(&out.join(key)).unwrap()))
        .collect();
    put(d.path(), "stone.png", rgb64().brighten(30));
    let (checked, events) = run(req("out", true));
    assert_eq!(checked.converted, 1, "{checked:?}");
    assert_eq!(checked.up_to_date, first.converted - 1);
    assert_eq!(checked.checked, first.converted - 1);
    assert_eq!(checked.checked_failed, 0);
    let reasons = skips(&events);
    let after = records(&out);
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );
    for (key, rec) in &after {
        assert!(rec["validation"].get("skipped").is_none(), "{key}");
        if key == "textures/a/stone.dds" {
            continue;
        }
        assert!(
            std::fs::read(out.join(key)).unwrap() == bytes[Path::new(key)],
            "{key}"
        );
        assert_eq!(SourceStamp::of(&out.join(key)), Some(stamps[key]), "{key}");
        for field in ["output_stamp", "profile_sha256", "encode_version"] {
            assert_eq!(rec[field], before[key][field], "{key}: {field}");
        }
        assert!(reasons[key.as_str()].ends_with(CHECKED_IN_PLACE), "{key}");
    }
    let (reference, reference_events) = run(req("ref", true));
    assert_eq!(reference.converted, first.converted);
    assert!(
        warnings(&events, "textures/a/noise.dds")
            .iter()
            .any(|w| w.code == engine::codes::Code::ValMipError),
        "noise.dds missing VAL-MIP-ERROR"
    );
    for (key, rec) in records(&d.path().join("ref")) {
        assert_eq!(
            warnings(&events, &key),
            warnings(&reference_events, &key),
            "{key}"
        );
        assert_eq!(after[&key]["validation"], rec["validation"], "{key}");
    }
    let third = run(req("out", true)).0;
    assert_eq!((third.checked, third.converted), (0, 0));
}

/// Note LG10: a dry run decides before the manifest is opened, so a validating one checks nothing in place and
/// writes neither a record nor a DDS, even with unchecked records and a new source to convert.
#[test]
fn a_validating_dry_run_over_skipped_records_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    fixture(d.path());
    let out = d.path().join("out");
    let first = run(request(d.path(), "out", false, OnExisting::IfChanged)).0;
    assert!(first.converted > 0, "{first:?}");
    assert!(
        records(&out)
            .values()
            .all(|rec| rec["validation"]["skipped"] == true),
        "the first run must leave unchecked records"
    );
    put(d.path(), "extra.png", rgb64());
    let manifest = std::fs::read(Manifest::at(&out).path()).unwrap();
    let bytes = dds_files(&out);
    let mut req = request(d.path(), "out", true, OnExisting::IfChanged);
    req.overrides
        .push(engine::overrides::parse_set("file:**/extra.png.subtype=diffuse").unwrap());
    req.dry_run = true;
    let summary = run(req).0;
    assert_eq!((summary.checked, summary.converted), (0, 0), "{summary:?}");
    assert!(
        std::fs::read(Manifest::at(&out).path()).unwrap() == manifest,
        "the manifest changed"
    );
    assert!(dds_files(&out) == bytes, "a DDS changed or appeared");
}

#[test]
fn a_current_copy_record_is_not_checked_in_place() {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "stone.png", rgb64());
    let (first, out) = convert(dir.path(), "out", false, OnExisting::IfChanged);
    assert_eq!(first.converted, 1);
    let key = "textures/a/stone.dds";
    let manifest = Manifest::at(&out);
    let mut record: engine::manifest::ManifestRecord =
        serde_json::from_value(records(&out).remove(key).unwrap()).unwrap();
    // Keep the current profile, sources and output stamp, and mark the record as a copy.
    record.copied_from = record.output_stamp;
    assert!(record.copied_from.is_some());
    manifest.append(&record).unwrap();
    let (s, ev) = run(request(dir.path(), "out", true, OnExisting::IfChanged));
    assert_eq!(s.up_to_date, 1);
    assert_eq!(s.checked, 0);
    assert_eq!(skips(&ev)[key], "up to date");
    let after = &records(&out)[key];
    assert_eq!(after["validation"]["skipped"], true);
    assert_eq!(
        after["copied_from"],
        serde_json::to_value(record.copied_from).unwrap()
    );
}

#[test]
fn a_lower_target_keep_is_not_checked_in_place() {
    let d = tempfile::tempdir().unwrap();
    fixture(d.path());
    let mut req = request(d.path(), "out", false, OnExisting::IfChanged);
    req.overrides
        .push(engine::overrides::parse_set("encoder.effort=slowest").unwrap());
    assert_eq!(run(req).0.exit_code(), 0);
    let (s, events) = run(request(d.path(), "out", true, OnExisting::IfChanged));
    let reasons = skips(&events);
    let recs = records(&d.path().join("out"));
    assert!(s.kept_lower_target >= 1);
    assert!(reasons["textures/a/stone.dds"].starts_with("kept:"));
    for (key, reason) in reasons.iter().filter(|(_, r)| r.starts_with("kept:")) {
        assert_eq!(recs[*key]["validation"]["skipped"], true);
        assert!(!reason.ends_with(CHECKED_IN_PLACE));
    }
    assert_eq!(s.checked, recs.len() as u32 - s.kept_lower_target);
}

#[test]
fn an_older_encode_output_is_not_checked_in_place() {
    let d = tempfile::tempdir().unwrap();
    fixture(d.path());
    let (first, out) = convert(d.path(), "out", false, OnExisting::IfChanged);
    let manifest = Manifest::at(&out);
    let lines = std::fs::read_to_string(manifest.path()).unwrap();
    let (line, rest) = lines.split_once('\n').unwrap();
    let mut old: Value = serde_json::from_str(line).unwrap();
    old["encode_version"] = "0.9.0".into();
    std::fs::write(manifest.path(), format!("{old}\n{rest}")).unwrap();
    let mut req = request(d.path(), "out", true, OnExisting::IfChanged);
    req.ev = EvScope::Off;
    let (s, _) = run(req);
    assert_eq!(s.kept_older_encode, 1);
    assert_eq!(s.checked, first.converted - 1);
    assert_eq!(
        records(&out)[old["output"].as_str().unwrap()]["validation"]["skipped"],
        true
    );
}

#[test]
fn a_packed_output_whose_pass_one_pick_changed_is_not_checked_in_place() {
    use engine::config::GraySource;
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "iron_roughness.png", rgb64());
    let cfg = d.path().join("sdc.toml");
    let global = "[defaults.channels]\ngrayscale_source = \"luma\"\n";
    let rule = "[[rules]]\nmatch = { alpha = \"constant255\" }\noverrides = { channels = { grayscale_source = \"luma\" } }\n";
    let key = "textures/a/iron_rmaos.dds";
    let packed = |out, config, validate, gray| {
        std::fs::write(&cfg, config).unwrap();
        let mut req = request(d.path(), out, validate, OnExisting::IfChanged);
        req.project_config = Some(cfg.clone());
        let sink = Arc::new(VecSink::default());
        let rep = Reporter::new(sink.clone(), "pack-identity");
        let e = Engine::new().unwrap();
        let plan = e.plan(req, &rep, &CancelToken::default()).unwrap();
        let job = plan.jobs.iter().find(|j| j.output.ends_with(key)).unwrap();
        assert_eq!(job.pack.as_ref().unwrap().gray, gray);
        let profile = engine::manifest::profile_sha256(&job.profile).unwrap();
        let s = e.run(plan, &rep, &CancelToken::default()).unwrap();
        let rec = &records(&d.path().join(out))[key];
        assert_eq!(rec["profile_sha256"], profile);
        (s, sink.take(), rec.clone())
    };
    let (first, _, before) = packed("out", global, false, GraySource::Luma);
    assert_eq!(first.converted, 1);
    let path = d.path().join("out").join(key);
    let bytes = std::fs::read(&path).unwrap();
    let (s, ev, after) = packed("out", rule, true, GraySource::R);
    assert_eq!(after["profile_sha256"], before["profile_sha256"]);
    assert_eq!(s.up_to_date, 1);
    assert_eq!(s.checked, 0);
    assert!(!skips(&ev)[key].ends_with(CHECKED_IN_PLACE));
    assert_eq!(after["validation"]["skipped"], true);
    assert!(std::fs::read(&path).unwrap() == bytes);
    packed("control", rule, false, GraySource::R);
    let (s, ev, rec) = packed("control", rule, true, GraySource::R);
    assert_eq!(s.checked, 1);
    assert!(skips(&ev)[key].ends_with(CHECKED_IN_PLACE));
    assert!(rec["validation"].get("skipped").is_none());
}

/// Note LG4: a record that says `skipped` is `passed`, so `if-changed` keeps its output.
#[test]
fn a_record_not_validated_is_current_on_the_next_run() {
    let d = tempfile::tempdir().unwrap();
    fixture(d.path());
    let (first, _) = convert(d.path(), "out", false, OnExisting::IfChanged);
    assert!(first.converted > 0, "{first:?}");
    let (second, _) = convert(d.path(), "out", false, OnExisting::IfChanged);
    assert_eq!(second.converted, 0, "{second:?}");
    assert_eq!(second.up_to_date, first.converted, "{second:?}");
}
