//! Note LF: `BatchRequest::validate` off skips the post-encode check (spec §5.8). Validation only reads the bytes,
//! so the written DDS is the same either way; the record keeps a `validation` object that says `skipped`.
use engine::api::*;
use engine::config::{Format, Target};
use engine::events::{Reporter, VecSink};
use engine::manifest::Manifest;
use engine::policy::AcceptSuggestions;
use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One BC1, one BC4, one BC6H and BC7 outputs (one of them a cutout), none of an always-validated type.
const SETS: [&str; 7] = [
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
    let mut req = BatchRequest::new(vec![d.join("in")], d.join(out), Target::Cs);
    req.recursive = true;
    req.accept = AcceptSuggestions::High;
    req.on_existing = on_existing;
    req.validate = validate;
    req.overrides = SETS
        .iter()
        .map(|s| engine::overrides::parse_set(s).unwrap())
        .collect();
    let rep = Reporter::new(Arc::new(VecSink::default()), "optional-validate");
    let e = Engine::new().unwrap();
    let plan = e.plan(req, &rep, &CancelToken::default()).unwrap();
    let root = plan.request.output_root.clone();
    let s = e.run(plan, &rep, &CancelToken::default()).unwrap();
    (s, root)
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

/// D4: a record that says `skipped` is `passed`, so `if-changed` keeps its output.
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
