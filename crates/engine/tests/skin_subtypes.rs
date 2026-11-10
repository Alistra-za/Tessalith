//! Packing-fills Task 1b (DECISIONS Note R-skin-1): the Advanced skin sub-types `skin_albedo`, `skin_rfaos`,
//! `skin_sk` and `skin_wet`.
use engine::api::*;
use engine::codes::Code;
use engine::config::{Format, Subtype, Target};
use engine::dds::{read_dds, DdsFile, DdsPixelFormat};
use engine::decode::{decode_level, Decoded};
use engine::events::*;
use engine::policy::{availability, AcceptSuggestions, Availability, ResolvedJob};
use engine::suggest::Confidence;
use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

fn put(dir: &Path, rel: &str, img: DynamicImage) {
    let p = dir.join("in").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    img.save(p).unwrap();
}

fn colour(w: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(w, w, |x, y| {
        Rgb([(x * 3) as u8, (y * 2) as u8, 90])
    }))
}

fn grey(w: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(w, w, |x, _| Rgb([(x * 3) as u8; 3])))
}

/// A tangent-space-looking normal with a binary mask in alpha.
fn wet_normal(w: u32) -> DynamicImage {
    DynamicImage::ImageRgba8(ImageBuffer::from_fn(w, w, |x, y| {
        Rgba([
            128 + (x % 8) as u8,
            128 + (y % 8) as u8,
            250,
            if x < w / 2 { 255 } else { 0 },
        ])
    }))
}

fn req(d: &TempDir, target: Target, sets: &[&str]) -> BatchRequest {
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), target);
    r.recursive = true;
    r.accept = AcceptSuggestions::High;
    r.on_existing = OnExisting::Overwrite;
    r.overrides = sets
        .iter()
        .map(|s| engine::overrides::parse_set(s).unwrap())
        .collect();
    r
}

fn plan(d: &TempDir, target: Target, sets: &[&str]) -> Plan {
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    Engine::new()
        .unwrap()
        .plan(req(d, target, sets), &rep, &CancelToken::default())
        .unwrap()
}

fn run(d: &TempDir) -> (Vec<EngineEvent>, BatchSummary) {
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "t");
    let e = Engine::new().unwrap();
    let p = e
        .plan(req(d, Target::Cs, &[]), &rep, &CancelToken::default())
        .unwrap();
    let s = e.run(p, &rep, &CancelToken::default()).unwrap();
    (sink.take(), s)
}

fn job<'a>(p: &'a Plan, input: &str) -> &'a ResolvedJob {
    p.jobs
        .iter()
        .find(|j| j.input.file_name().is_some_and(|n| n == input))
        .unwrap_or_else(|| panic!("no job for {input}"))
}

fn top(p: &Plan, input: &str) -> (Subtype, Confidence) {
    let s = &job(p, input).suggestions[0];
    (s.subtype, s.confidence)
}

fn out(d: &TempDir, name: &str) -> DdsFile {
    read_dds(&std::fs::read(d.path().join("out").join(name)).unwrap()).unwrap()
}

fn texels(f: &DdsFile) -> Vec<[u8; 4]> {
    let (w, h, r) = f.levels[0].clone();
    match decode_level(f.format, w, h, &f.data[r]).unwrap() {
        Decoded::Ldr(px) => px,
        Decoded::Hdr(_) => panic!("expected an LDR format"),
    }
}

fn set_files(d: &TempDir, with_rfaos: bool) {
    put(d.path(), "x.png", colour(64));
    put(d.path(), "x_sk.png", grey(64));
    put(d.path(), "x_wet.png", wet_normal(64));
    if with_rfaos {
        put(d.path(), "x_rfaos.png", colour(64));
    }
}

/// (a) An `_rfaos` member makes the set Advanced skin.
#[test]
fn an_rfaos_set_suggests_the_skin_sub_types() {
    let d = tempfile::tempdir().unwrap();
    set_files(&d, true);
    let p = plan(&d, Target::Cs, &[]);
    assert_eq!(top(&p, "x.png"), (Subtype::SkinAlbedo, Confidence::High));
    assert_eq!(
        top(&p, "x_rfaos.png"),
        (Subtype::SkinRfaos, Confidence::High)
    );
    assert_eq!(top(&p, "x_sk.png"), (Subtype::SkinSk, Confidence::High));
    assert_eq!(top(&p, "x_wet.png"), (Subtype::SkinWet, Confidence::High));
}

/// (a) A raw-map set whose recipe resolves to `_rfaos` (a fuzz source, or `packing.recipe = rfaos`) is Advanced skin.
#[test]
fn a_set_that_builds_rfaos_is_advanced_skin() {
    for toml in ["", "[files.\"x_rmaos\".packing]\nrecipe = \"rfaos\"\n"] {
        let d = tempfile::tempdir().unwrap();
        set_files(&d, false);
        put(d.path(), "x_roughness.png", grey(64));
        put(d.path(), "x_fuzz.png", grey(64));
        put(d.path(), "x_ao.png", grey(64));
        std::fs::write(d.path().join("in/sdc.toml"), toml).unwrap();
        let p = plan(&d, Target::Cs, &[]);
        assert_eq!(
            top(&p, "x_sk.png"),
            (Subtype::SkinSk, Confidence::High),
            "{toml}"
        );
        assert_eq!(top(&p, "x.png").0, Subtype::SkinAlbedo, "{toml}");
    }
    // Without a fuzz source and no override the recipe is `_rmaos`: not skin.
    let d = tempfile::tempdir().unwrap();
    set_files(&d, false);
    put(d.path(), "x_roughness.png", grey(64));
    put(d.path(), "x_ao.png", grey(64));
    let p = plan(&d, Target::Cs, &[]);
    assert_eq!(top(&p, "x_sk.png").0, Subtype::BacklightTint);
}

fn all_codes(p: &Plan) -> Vec<Code> {
    p.jobs
        .iter()
        .flat_map(|j| j.warnings.iter().map(|w| w.code))
        .chain(p.skipped.iter().filter_map(|s| s.code))
        .collect()
}

/// Note R-fills-1 (closed by HR4), a pin of existing behaviour: `rfaos` is in `group::SUFFIXES`, so a pre-packed
/// `_rfaos` joins a forced-`rfaos` set without a prediction, and the pre-packed conflict is raised (M9).
#[test]
fn a_prepacked_rfaos_joins_a_forced_rfaos_set_without_fuzz() {
    let d = tempfile::tempdir().unwrap();
    set_files(&d, false);
    put(d.path(), "x_roughness.png", grey(64));
    put(d.path(), "x_ao.png", grey(64));
    put(d.path(), "x_rfaos.png", colour(64));
    std::fs::write(
        d.path().join("in/sdc.toml"),
        "[files.\"x_rmaos\".packing]\nrecipe = \"rfaos\"\n",
    )
    .unwrap();
    let p = plan(&d, Target::Cs, &[]);
    assert_eq!(
        set_of(&p, "x_rfaos.png"),
        set_of(&p, "x_roughness.png"),
        "one set"
    );
    assert_eq!(set_of(&p, "x_ao.png"), set_of(&p, "x_rfaos.png"), "ao too");
    let codes = all_codes(&p);
    assert!(codes.contains(&Code::PackConflict), "{codes:?}");
}

/// (b) Without `_rfaos`: the vanilla `_sk`, and `x_wet` is its own colour map (final review I3: `_wet` is a suffix
/// only in an Advanced skin set).
#[test]
fn outside_advanced_skin_sk_is_the_vanilla_guess_and_wet_is_a_base_name() {
    let d = tempfile::tempdir().unwrap();
    set_files(&d, false);
    // Note GY1: a normal on `x_wet`, so the unjoined `x_wet` is not a lone unknown suffix.
    put(d.path(), "x_wet_n.png", wet_normal(64));
    let p = plan(&d, Target::Cs, &[]);
    assert_eq!(
        top(&p, "x_sk.png"),
        (Subtype::BacklightTint, Confidence::Low)
    );
    assert_eq!(top(&p, "x_wet.png"), (Subtype::Diffuse, Confidence::High));
    assert_eq!(top(&p, "x.png").0, Subtype::Diffuse);
}

/// (c) CS no longer reads a grey `_wet` (CS 176a0083): it is not written, the rest of the batch is, and the skip is
/// reported as SKIN-WET-GREY.
#[test]
fn grey_wet_is_not_written_and_reports_skin_wet_grey() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "h.png", colour(64));
    put(d.path(), "h_rfaos.png", colour(64));
    put(d.path(), "h_wet.png", grey(64));
    let (events, s) = run(&d);
    assert_eq!(s.exit_code(), 0, "{s:?}");
    assert!(!d.path().join("out/h_wet.dds").exists());
    assert_eq!(out(&d, "h.dds").format, DdsPixelFormat::Bc7 { srgb: false });
    assert!(d.path().join("out/h_rfaos.dds").exists());
    let skips: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                &e.kind,
                EventKind::FileSkipped {
                    code: Some(Code::SkinWetGrey),
                    ..
                }
            )
        })
        .collect();
    assert_eq!(skips.len(), 1, "{events:#?}");
    assert!(skips[0]
        .path
        .as_deref()
        .is_some_and(|p| p.ends_with("h_wet.png")));
    // An explicit BC7 does not bring the grey map back.
    let p = plan(&d, Target::Cs, &["subtype:skin_wet.format=bc7"]);
    assert!(!p.jobs.iter().any(|j| j.input.ends_with("h_wet.png")));
}

/// (c) A coloured `_wet` is always BC7 with its alpha.
#[test]
fn coloured_wet_is_bc7() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "y.png", colour(64));
    put(d.path(), "y_rfaos.png", colour(64));
    put(d.path(), "y_wet.png", wet_normal(64));
    let (_, s) = run(&d);
    assert_eq!(s.exit_code(), 0, "{s:?}");
    let cw = out(&d, "y_wet.dds");
    assert_eq!(cw.format, DdsPixelFormat::Bc7 { srgb: false });
    // Alpha kept: the mask's two halves decode apart.
    let px = texels(&cw);
    assert!(px[0][3] > 200 && px[63][3] < 50, "{:?} {:?}", px[0], px[63]);
}

/// CS's texture loader reads RGBA8, BGRA8, BC1, BC2, BC3 and BC7 (BC5 for a detail map only): any other format on a
/// skin map is blocked by FMT-CS-SKIN-LOADER and nothing is written for it.
#[test]
fn wet_format_outside_the_cs_loader_list_is_blocked() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "h.png", colour(64));
    put(d.path(), "h_rfaos.png", colour(64));
    put(d.path(), "h_wet.png", wet_normal(64));
    let p = plan(&d, Target::Cs, &["subtype:skin_wet.format=bc4"]);
    let j = job(&p, "h_wet.png");
    assert!(
        matches!(&j.status, engine::policy::JobStatus::Blocked { codes } if codes.contains(&Code::FmtCsSkinLoader)),
        "{:?} {:?}",
        j.status,
        j.warnings
    );
    // Final review M2: the message lists the formats Tessalith can write that CS reads.
    let w = j
        .warnings
        .iter()
        .find(|w| w.code == Code::FmtCsSkinLoader)
        .expect("the loader warning");
    assert!(
        w.message
            .contains("skin_wet only as BC1, BC3, BC7 or RGBA8;"),
        "{}",
        w.message
    );
    let mut r = req(&d, Target::Cs, &["subtype:skin_wet.format=bc4"]);
    r.on_existing = OnExisting::Overwrite;
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    let e = Engine::new().unwrap();
    let plan = e.plan(r, &rep, &CancelToken::default()).unwrap();
    e.run(plan, &rep, &CancelToken::default()).unwrap();
    assert!(!d.path().join("out/h_wet.dds").exists());
    assert!(d.path().join("out/h.dds").exists());
}

/// `--allow FMT-CS-SKIN-LOADER` cannot make an unsupported format writable: the file would load as nothing in CS.
#[test]
fn allowing_the_loader_code_does_not_unblock_the_format() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "h.png", colour(64));
    put(d.path(), "h_rfaos.png", colour(64));
    put(d.path(), "h_wet.png", wet_normal(64));
    let mut r = req(&d, Target::Cs, &["subtype:skin_wet.format=bc4"]);
    r.allow.insert(Code::FmtCsSkinLoader);
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    let e = Engine::new().unwrap();
    let plan = e.plan(r, &rep, &CancelToken::default()).unwrap();
    let j = plan
        .jobs
        .iter()
        .find(|j| j.input.ends_with("h_wet.png"))
        .unwrap();
    assert!(
        matches!(&j.status, engine::policy::JobStatus::Blocked { codes } if codes.contains(&Code::FmtCsSkinLoader)),
        "{:?} {:?}",
        j.status,
        j.warnings
    );
    e.run(plan, &rep, &CancelToken::default()).unwrap();
    assert!(!d.path().join("out/h_wet.dds").exists());
}

/// The CS loader list is a property of the sub-type and format alone.
#[test]
fn cs_skin_loader_list() {
    use engine::policy::cs_skin_loader_ok;
    for f in [Format::Bc1, Format::Bc3, Format::Bc7, Format::Rgba8] {
        assert!(cs_skin_loader_ok(Subtype::SkinWet, f), "{f:?}");
        assert!(cs_skin_loader_ok(Subtype::SkinRfaos, f), "{f:?}");
    }
    for f in [
        Format::Bc4,
        Format::Bc5,
        Format::Rgb8,
        Format::R8,
        Format::Rg8,
    ] {
        assert!(!cs_skin_loader_ok(Subtype::SkinWet, f), "{f:?}");
    }
    // A detail map also loads as BC5 (CS rebuilds Z), but not as BC4.
    assert!(cs_skin_loader_ok(Subtype::SkinDetail, Format::Bc5));
    assert!(!cs_skin_loader_ok(Subtype::SkinDetail, Format::Bc4));
    assert!(cs_skin_loader_ok(Subtype::Diffuse, Format::Bc4));
}

fn detail_rgba(alpha: impl Fn(u32, u32) -> u8) -> DynamicImage {
    DynamicImage::ImageRgba8(ImageBuffer::from_fn(64, 64, |x, y| {
        Rgba([128 + (x % 8) as u8, 128 + (y % 8) as u8, 250, alpha(x, y)])
    }))
}

const DETAIL: &str = "file:**/d.png.subtype=skin_detail";

fn detail_job_format(d: &TempDir, sets: &[&str]) -> (Format, Vec<Code>) {
    let p = plan(d, Target::Cs, sets);
    let j = job(&p, "d.png");
    (
        j.profile.format,
        j.warnings.iter().map(|w| w.code).collect(),
    )
}

/// An opaque `skin_detail` is BC5, and FMT-BC5-NORMAL does not fire (CS rebuilds Z from BC5).
#[test]
fn opaque_skin_detail_is_bc5() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "d.png", detail_rgba(|_, _| 255));
    let (f, codes) = detail_job_format(&d, &[DETAIL]);
    assert_eq!(f, Format::Bc5);
    assert!(!codes.contains(&Code::FmtBc5Normal), "{codes:?}");
    assert!(!codes.contains(&Code::FmtCsSkinLoader), "{codes:?}");
    // End to end: not blocked, and BC5 is what gets written.
    let p = plan(&d, Target::Cs, &[DETAIL]);
    let j = job(&p, "d.png");
    assert!(
        !matches!(j.status, engine::policy::JobStatus::Blocked { .. }),
        "{:?} {:?}",
        j.status,
        j.warnings
    );
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    let e = Engine::new().unwrap();
    let s = e.run(p, &rep, &CancelToken::default()).unwrap();
    assert_eq!(s.exit_code(), 0, "{s:?}");
    assert_eq!(out(&d, "d.dds").format, DdsPixelFormat::Bc5);
}

#[test]
fn skin_detail_without_alpha_is_bc5() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "d.png", colour(64));
    let (f, codes) = detail_job_format(&d, &[DETAIL]);
    assert_eq!(f, Format::Bc5);
    assert!(!codes.contains(&Code::FmtBc5Normal), "{codes:?}");
    let p = plan(&d, Target::Cs, &[DETAIL]);
    let j = job(&p, "d.png");
    assert!(
        !matches!(j.status, engine::policy::JobStatus::Blocked { .. }),
        "{:?} {:?}",
        j.status,
        j.warnings
    );
}

#[test]
fn skin_detail_with_any_alpha_below_255_is_bc7() {
    let d = tempfile::tempdir().unwrap();
    put(
        d.path(),
        "d.png",
        detail_rgba(|x, y| if x == 3 && y == 5 { 254 } else { 255 }),
    );
    assert_eq!(detail_job_format(&d, &[DETAIL]).0, Format::Bc7);
}

#[test]
fn explicit_format_wins_over_the_opaque_fact() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "d.png", detail_rgba(|_, _| 255));
    assert_eq!(
        detail_job_format(&d, &[DETAIL, "file:**/d.png.format=bc7"]).0,
        Format::Bc7
    );
}

/// Final review M2: a detail map's loader message offers BC5 too.
#[test]
fn bc4_skin_detail_is_blocked_and_names_bc5() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "d.png", detail_rgba(|_, _| 255));
    let p = plan(&d, Target::Cs, &[DETAIL, "file:**/d.png.format=bc4"]);
    let j = job(&p, "d.png");
    let w = j
        .warnings
        .iter()
        .find(|w| w.code == Code::FmtCsSkinLoader)
        .unwrap_or_else(|| panic!("{:?}", j.warnings));
    assert!(
        w.message
            .contains("skin_detail only as BC1, BC3, BC5, BC7 or RGBA8;"),
        "{}",
        w.message
    );
}

#[test]
fn skin_detail_is_unavailable_on_vanilla() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "d.png", detail_rgba(|_, _| 255));
    let p = plan(&d, Target::Vanilla, &[DETAIL]);
    let j = job(&p, "d.png");
    assert!(
        matches!(&j.status, engine::policy::JobStatus::Blocked { codes } if codes.contains(&Code::CfgUnavailable)),
        "{:?} {:?}",
        j.status,
        j.warnings
    );
}

/// (d) `skin_albedo` is UNORM, never sRGB.
#[test]
fn skin_albedo_writes_bc7_unorm() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x.png", colour(64));
    put(d.path(), "x_rfaos.png", colour(64));
    let (_, s) = run(&d);
    assert_eq!(s.exit_code(), 0, "{s:?}");
    assert_eq!(out(&d, "x.dds").format, DdsPixelFormat::Bc7 { srgb: false });
    let p = plan(&d, Target::Cs, &[]);
    assert_eq!(job(&p, "x.png").profile.subtype, Subtype::SkinAlbedo);
    assert!(!Subtype::SkinAlbedo.is_pbr_color());
}

/// `skin_sk` keeps the red channel only, as BC4.
#[test]
fn skin_sk_is_bc4_from_red() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x.png", colour(64));
    put(d.path(), "x_rfaos.png", colour(64));
    put(d.path(), "x_sk.png", colour(64));
    let (_, s) = run(&d);
    assert_eq!(s.exit_code(), 0, "{s:?}");
    let f = out(&d, "x_sk.dds");
    assert_eq!(f.format, DdsPixelFormat::Bc4);
    let px = texels(&f);
    // R ramps with x (x * 3); G (y * 2) must not leak in.
    assert!(
        px[0][0] < 12 && (i32::from(px[63][0]) - 189).abs() < 12,
        "{:?} {:?}",
        px[0],
        px[63]
    );
}

/// (e) The skin sub-types exist on `cs` only.
#[test]
fn skin_sub_types_are_unavailable_off_cs() {
    for st in [
        Subtype::SkinAlbedo,
        Subtype::SkinRfaos,
        Subtype::SkinSk,
        Subtype::SkinWet,
    ] {
        assert_eq!(availability(Target::Cs, st), Availability::Available);
        let t = Target::Vanilla;
        assert_eq!(
            availability(t, st),
            Availability::Unavailable { fallback: None }
        );
    }
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x_sk.png", grey(64));
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    let err = Engine::new()
        .unwrap()
        .plan(
            req(&d, Target::Vanilla, &["subtype=skin_sk"]),
            &rep,
            &CancelToken::default(),
        )
        .err()
        .expect("a batch-wide unavailable sub-type is refused")
        .to_string();
    assert!(
        err.contains("skin_sk") && err.contains("unavailable") && err.contains("CFG-UNAVAILABLE"),
        "{err}"
    );
}

/// Note FM11: the pre-rename id `rfaos` is not read (JSON, a config table or `--set`); `skin_rfaos` is the id.
#[test]
fn the_old_rfaos_id_is_not_read() {
    assert!(serde_json::from_str::<Subtype>("\"rfaos\"").is_err());
    assert_eq!(Subtype::SkinRfaos.id(), "skin_rfaos");
    assert_eq!(
        serde_json::to_string(&Subtype::SkinRfaos).unwrap(),
        "\"skin_rfaos\""
    );
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x_rfaos.png", colour(64));
    std::fs::write(
        d.path().join("in/sdc.toml"),
        "[subtypes.rfaos]\nmips = { levels = \"none\" }\n",
    )
    .unwrap();
    let rep = Reporter::new(Arc::new(NullSink), "skin-rfaos");
    let err = Engine::new()
        .unwrap()
        .plan(req(&d, Target::Cs, &[]), &rep, &CancelToken::default())
        .err()
        .expect("a removed sub-type id is refused")
        .to_string();
    assert!(
        err.contains("subtypes.rfaos: removed in 2.0") && err.contains("skin_rfaos"),
        "{err}"
    );
    let e = engine::overrides::parse_set("subtype=rfaos").unwrap_err();
    assert!(
        e.contains("removed in 2.0") && e.contains("skin_rfaos"),
        "{e}"
    );
}

/// `_wet` is not a §6.1 suffix (only `join_skin_wet` splits it, in a cs Advanced skin set) and is no vocabulary term.
#[test]
fn wet_is_not_a_general_suffix() {
    assert_eq!(
        engine::group::split_suffix("skin_wet"),
        ("skin_wet".to_string(), None)
    );
    assert!(!engine::group::is_vocabulary("wet"));
}

/// Final review I1: a forced `_rmaos` beats a fuzz source. The set builds `_rmaos`, so it is not Advanced skin;
/// only a real `_rfaos` member short-circuits, never the `rfaos` suffix that `auto` predicts from the fuzz source.
#[test]
fn a_forced_rmaos_with_a_fuzz_source_is_not_advanced_skin() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x.png", colour(64));
    put(d.path(), "x_sk.png", grey(64));
    put(d.path(), "x_roughness.png", grey(64));
    put(d.path(), "x_fuzz.png", grey(64));
    std::fs::write(
        d.path().join("in/sdc.toml"),
        "[files.\"x_rmaos\".packing]\nrecipe = \"rmaos\"\n",
    )
    .unwrap();
    let p = plan(&d, Target::Cs, &[]);
    assert!(p
        .jobs
        .iter()
        .any(|j| j.output.file_name().is_some_and(|n| n == "x_rmaos.dds")));
    assert_eq!(top(&p, "x.png").0, Subtype::PbrAlbedo);
    assert_eq!(top(&p, "x_sk.png").0, Subtype::BacklightTint);
}

fn outputs(p: &Plan) -> Vec<String> {
    let mut o: Vec<String> = p
        .jobs
        .iter()
        .filter_map(|j| Some(j.output.file_name()?.to_string_lossy().into_owned()))
        .collect();
    o.sort();
    o
}

/// A PBR set (`x`, `_roughness`, `_ao`) that packs `_rmaos` on its own, with `toml` as its project `sdc.toml`.
fn pbr_set(toml: &str) -> TempDir {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x.png", colour(64));
    put(d.path(), "x_sk.png", grey(64));
    put(d.path(), "x_roughness.png", grey(64));
    put(d.path(), "x_ao.png", grey(64));
    std::fs::write(d.path().join("in/sdc.toml"), toml).unwrap();
    d
}

const SKIN: &str = "set:x.shader=advanced_skin";

/// Note DV1: a set whose chosen shader is Advanced Skin is Advanced skin: its inferred diffuse resolves to
/// `skin_albedo` and its packed output is `_rfaos`, with nothing written to the project files.
#[test]
fn an_advanced_skin_shader_choice_makes_the_set_skin() {
    let d = pbr_set("");
    let before = plan(&d, Target::Cs, &[]);
    assert!(outputs(&before).contains(&"x_rmaos.dds".to_string()));
    let p = plan(&d, Target::Cs, &[SKIN]);
    assert_eq!(job(&p, "x.png").profile.subtype, Subtype::SkinAlbedo);
    assert_eq!(job(&p, "x_sk.png").profile.subtype, Subtype::SkinSk);
    let o = outputs(&p);
    assert!(o.contains(&"x_rfaos.dds".to_string()), "{o:?}");
    assert!(!o.contains(&"x_rmaos.dds".to_string()), "{o:?}");
}

/// Note DV2: the GUI's saved choice (`BatchRequest::set_shaders`) and the CLI's `--set set:x.shader=` plan alike.
#[test]
fn the_gui_choice_and_the_cli_set_plan_alike() {
    let d = pbr_set("");
    let cli = plan(&d, Target::Cs, &[SKIN]);
    let mut r = req(&d, Target::Cs, &[]);
    r.set_shaders = [("x".to_string(), engine::shader::Shader::AdvancedSkin)].into();
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    let gui = Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap();
    let view = |p: &Plan| {
        let mut v: Vec<(String, Subtype)> = p
            .jobs
            .iter()
            .map(|j| (j.output.to_string_lossy().into_owned(), j.profile.subtype))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };
    assert_eq!(view(&gui), view(&cli));
    assert_eq!(job(&gui, "x.png").profile.subtype, Subtype::SkinAlbedo);
}

/// Note DV3: a sub-type the user named other than plain `diffuse` is kept; a written `diffuse` reads as
/// `skin_albedo`, as DU6 rewrites it on a fresh pick. A written `packing.recipe` decides the packed output alone.
#[test]
fn an_advanced_skin_shader_keeps_explicit_choices() {
    let d = pbr_set("[files.x]\nsubtype = \"pbr_albedo\"\n");
    let p = plan(&d, Target::Cs, &[SKIN]);
    assert_eq!(job(&p, "x.png").profile.subtype, Subtype::PbrAlbedo);
    assert!(outputs(&p).contains(&"x_rfaos.dds".to_string()));

    let d = pbr_set("[files.x]\nsubtype = \"diffuse\"\n");
    let p = plan(&d, Target::Cs, &[SKIN]);
    assert_eq!(job(&p, "x.png").profile.subtype, Subtype::SkinAlbedo);
    assert!(outputs(&p).contains(&"x_rfaos.dds".to_string()));
    // The same written `diffuse` without the shader stays `diffuse`.
    let p = plan(&d, Target::Cs, &[]);
    assert_eq!(job(&p, "x.png").profile.subtype, Subtype::Diffuse);

    let d = pbr_set("[files.\"x_rmaos\".packing]\nrecipe = \"rmaos\"\n");
    let p = plan(&d, Target::Cs, &[SKIN]);
    assert!(outputs(&p).contains(&"x_rmaos.dds".to_string()));
    assert!(!outputs(&p).contains(&"x_rfaos.dds".to_string()));
}

/// Note DV1 (re-review): only the set's diffuse member (no suffix, or `_d`) is read as the albedo. An `_i` inner
/// layer keeps its diffuse guess, and a raw-map-named file the user typed `diffuse` keeps that choice, stays a
/// written choice (not a packing input) and converts on its own.
#[test]
fn only_the_diffuse_member_becomes_skin_albedo() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x.png", colour(64));
    put(d.path(), "x_i.png", colour(64));
    let p = plan(&d, Target::Cs, &[SKIN]);
    assert_eq!(job(&p, "x.png").profile.subtype, Subtype::SkinAlbedo);
    assert_eq!(job(&p, "x_i.png").profile.subtype, Subtype::Diffuse);

    let d =
        pbr_set("[files.x_roughness]\nsubtype = \"diffuse\"\n[files.x]\nsubtype = \"diffuse\"\n");
    let p = plan(&d, Target::Cs, &[SKIN]);
    let r = job(&p, "x_roughness.png");
    assert_eq!(r.profile.subtype, Subtype::Diffuse);
    assert_eq!(r.output.file_name().unwrap(), "x_roughness.dds");
    let x = job(&p, "x.png");
    assert_eq!(x.profile.subtype, Subtype::SkinAlbedo);
    assert!(
        !matches!(
            x.provenance.get("subtype"),
            Some(engine::presets::Provenance::Shader(_) | engine::presets::Provenance::Suggestion)
        ),
        "a written diffuse keeps its written provenance: {:?}",
        x.provenance.get("subtype")
    );
}

/// Note DV1 (review issue 1): the shader alone types the diffuse, whatever the set can pack. A set with no packing
/// input, one whose packed map is a pre-packed `_rmaos` (its layout kept), and a metallic-only set that keeps
/// `_rmaos` (DQ2), with no PACK-RECIPE-UNBUILDABLE: nobody wrote a recipe.
#[test]
fn the_shader_alone_makes_the_diffuse_skin_albedo() {
    let diffuse_only = tempfile::tempdir().unwrap();
    put(diffuse_only.path(), "x.png", colour(64));
    let prepacked = tempfile::tempdir().unwrap();
    put(prepacked.path(), "x.png", colour(64));
    std::fs::write(prepacked.path().join("in/x_rmaos.dds"), b"not read").unwrap();
    let metallic = tempfile::tempdir().unwrap();
    put(metallic.path(), "x.png", colour(64));
    put(metallic.path(), "x_metallic.png", grey(64));
    for d in [&diffuse_only, &prepacked, &metallic] {
        let before = plan(d, Target::Cs, &[]);
        assert_ne!(job(&before, "x.png").profile.subtype, Subtype::SkinAlbedo);
        let p = plan(d, Target::Cs, &[SKIN]);
        assert_eq!(job(&p, "x.png").profile.subtype, Subtype::SkinAlbedo);
        let o = outputs(&p);
        assert!(!o.contains(&"x_rfaos.dds".to_string()), "{o:?}");
        assert_eq!(
            o.iter().filter(|n| n.contains("rmaos")).count(),
            outputs(&before)
                .iter()
                .filter(|n| n.contains("rmaos"))
                .count(),
            "packing unchanged: {o:?}"
        );
        assert!(!p
            .jobs
            .iter()
            .flat_map(|j| &j.warnings)
            .any(|w| w.code == Code::PackRecipeUnbuildable));
    }
}

/// Final review M2: on a set that is Advanced Skin only by its shader, a hand-set `rfaos.*` fill builds the `_rfaos`
/// when nothing else would.
#[test]
fn a_hand_rfaos_fill_builds_rfaos_on_a_shader_skin_set() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x.png", colour(64));
    put(d.path(), "x_specular.png", grey(64));
    std::fs::write(
        d.path().join("in/sdc.toml"),
        "[files.x_specular]\nhelper.fills = [\"rfaos.a\"]\n",
    )
    .unwrap();
    let p = plan(&d, Target::Cs, &[SKIN]);
    let o = outputs(&p);
    assert!(o.contains(&"x_rfaos.dds".to_string()), "{o:?}");
    let pk = p
        .jobs
        .iter()
        .find_map(|j| {
            j.pack
                .as_ref()
                .filter(|k| k.kind == engine::pack::RecipeKind::Rfaos)
        })
        .unwrap();
    assert!(
        matches!(&pk.channels[3], engine::pack::ChannelSource::Source { path, .. } if path.ends_with("x_specular.png")),
        "{:?}",
        pk.channels
    );
}

/// A GUI plan of `d` with `x` chosen Advanced Skin and `pinned` sent as the pinned files (Note DW3).
fn plan_pinned(d: &TempDir, pinned: &[&str]) -> Plan {
    let mut r = req(d, Target::Cs, &[]);
    r.set_shaders = [("x".to_string(), engine::shader::Shader::AdvancedSkin)].into();
    r.pinned_files = pinned.iter().map(|s| s.to_string()).collect();
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap()
}

/// Note DW1: a pinned diffuse blocks DV's reading: the set plans as before DV (the inferred guess, here
/// `pbr_albedo` as without the shader, and `_rmaos`). Unpinned it still reads `skin_albedo` with `_rfaos`. A pin on
/// another member changes nothing.
#[test]
fn a_pinned_diffuse_blocks_the_advanced_skin_reading() {
    let d = pbr_set("");
    let none = plan(&d, Target::Cs, &[]);
    assert_eq!(job(&none, "x.png").profile.subtype, Subtype::PbrAlbedo);
    let p = plan_pinned(&d, &["x.png"]);
    assert_eq!(job(&p, "x.png").profile.subtype, Subtype::PbrAlbedo);
    assert_eq!(outputs(&p), outputs(&none));
    let o = outputs(&p);
    assert!(o.contains(&"x_rmaos.dds".to_string()), "{o:?}");
    assert!(!o.contains(&"x_rfaos.dds".to_string()), "{o:?}");

    for pinned in [&[][..], &["x_roughness.png"][..]] {
        let p = plan_pinned(&d, pinned);
        assert_eq!(job(&p, "x.png").profile.subtype, Subtype::SkinAlbedo);
        let o = outputs(&p);
        assert!(o.contains(&"x_rfaos.dds".to_string()), "{o:?}");
        assert!(!o.contains(&"x_rmaos.dds".to_string()), "{o:?}");
    }

    // DW1(a): an inferred plain `diffuse` stays `diffuse` while pinned.
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x.png", colour(64));
    let p = plan_pinned(&d, &["x.png"]);
    assert_eq!(job(&p, "x.png").profile.subtype, Subtype::Diffuse);

    // DW1(a): a written plain `diffuse` stays `diffuse` while pinned.
    let d = pbr_set("[files.x]\nsubtype = \"diffuse\"\n");
    let p = plan_pinned(&d, &["x.png"]);
    assert_eq!(job(&p, "x.png").profile.subtype, Subtype::Diffuse);
    assert!(outputs(&p).contains(&"x_rmaos.dds".to_string()));
}

/// Note DW2: a pinned diffuse DU wrote `skin_albedo` (with its `rfaos` recipe) keeps both.
#[test]
fn a_pinned_written_skin_albedo_keeps_it() {
    let d = pbr_set(
        "[files.x]\nsubtype = \"skin_albedo\"\n[files.\"x_rfaos\".packing]\nrecipe = \"rfaos\"\n",
    );
    let p = plan_pinned(&d, &["x.png"]);
    assert_eq!(job(&p, "x.png").profile.subtype, Subtype::SkinAlbedo);
    let o = outputs(&p);
    assert!(o.contains(&"x_rfaos.dds".to_string()), "{o:?}");
    assert!(!o.contains(&"x_rmaos.dds".to_string()), "{o:?}");
}

/// Note DV3: off cs the choice changes nothing (Advanced Skin is a cs shader).
#[test]
fn an_advanced_skin_shader_off_cs_changes_nothing() {
    let d = pbr_set("");
    let a = plan(&d, Target::Vanilla, &[]);
    let b = plan(&d, Target::Vanilla, &[SKIN]);
    assert_eq!(outputs(&a), outputs(&b));
    assert_eq!(
        job(&a, "x.png").profile.subtype,
        job(&b, "x.png").profile.subtype
    );
}

fn set_of<'a>(p: &'a Plan, input: &str) -> &'a str {
    &job(p, input).set_key
}

/// Final review I3: outside a cs Advanced skin set `_wet` is part of the base name, as it was before `_wet` became
/// a skin suffix. A landscape `road_wet` is still a landscape diffuse on every target.
#[test]
fn a_wet_stem_outside_skin_keeps_its_old_reading() {
    for target in [Target::Vanilla, Target::Cs] {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "textures/landscape/road_wet.png", colour(64));
        let p = plan(&d, target, &[]);
        assert_eq!(
            top(&p, "road_wet.png"),
            (Subtype::LandscapeDiffuse, Confidence::High),
            "{target:?}"
        );
        assert!(
            set_of(&p, "road_wet.png").ends_with("/road_wet"),
            "{target:?}"
        );
    }
}

/// Final review I3: `cobble_wet.png` and `cobble_wet_n.png` stay one set (`cobble_wet`), next to a `cobble` set.
#[test]
fn a_wet_colour_map_and_its_normal_are_one_set() {
    for target in [Target::Vanilla, Target::Cs] {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), "cobble.png", colour(64));
        put(d.path(), "cobble_wet.png", colour(64));
        put(d.path(), "cobble_wet_n.png", wet_normal(64));
        let p = plan(&d, target, &[]);
        assert_eq!(
            set_of(&p, "cobble_wet.png"),
            set_of(&p, "cobble_wet_n.png"),
            "{target:?}"
        );
        assert!(set_of(&p, "cobble_wet.png").ends_with("/cobble_wet"));
        assert_ne!(set_of(&p, "cobble.png"), set_of(&p, "cobble_wet.png"));
        assert_ne!(top(&p, "cobble_wet.png").0, Subtype::SkinWet, "{target:?}");
    }
}

/// Final review I3: `_wet` joins its base as a skin member only on cs and only in an Advanced skin set: an `_rfaos`
/// member, or a raw-map set whose recipe resolves to `rfaos`.
#[test]
fn a_wet_member_joins_only_a_cs_advanced_skin_set() {
    let d = tempfile::tempdir().unwrap();
    set_files(&d, true);
    let p = plan(&d, Target::Cs, &[]);
    assert_eq!(set_of(&p, "x_wet.png"), set_of(&p, "x.png"));
    assert_eq!(top(&p, "x_wet.png"), (Subtype::SkinWet, Confidence::High));
    // A raw-map set with `packing.recipe = rfaos` and no fuzz source.
    let d = tempfile::tempdir().unwrap();
    set_files(&d, false);
    put(d.path(), "x_roughness.png", grey(64));
    std::fs::write(
        d.path().join("in/sdc.toml"),
        "[files.\"x_rfaos\".packing]\nrecipe = \"rfaos\"\n",
    )
    .unwrap();
    let p = plan(&d, Target::Cs, &[]);
    assert_eq!(set_of(&p, "x_wet.png"), set_of(&p, "x.png"));
    assert_eq!(top(&p, "x_wet.png"), (Subtype::SkinWet, Confidence::High));
    // Not on vanilla, even with an `_rfaos` beside it.
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x.png", colour(64));
    put(d.path(), "x_rfaos.png", colour(64));
    put(d.path(), "x_wet.png", colour(64));
    // Note GY1: a normal on `x_wet`, so the unjoined `x_wet` is not a lone unknown suffix.
    put(d.path(), "x_wet_n.png", wet_normal(64));
    let p = plan(&d, Target::Vanilla, &[]);
    assert_ne!(set_of(&p, "x_wet.png"), set_of(&p, "x.png"));
    assert_eq!(top(&p, "x_wet.png").0, Subtype::Diffuse);
}

/// Trace-debug 2026-10-16 #3: renaming an Advanced skin set keeps its joined `_wet` member as the skin wet map.
#[test]
fn rename_keeps_joined_skin_wet() {
    let d = tempfile::tempdir().unwrap();
    set_files(&d, true);
    let mut r = req(&d, Target::Cs, &[]);
    r.set_edits = vec![engine::sets::SetEdit::Rename {
        from: "x".into(),
        to: "x2".into(),
    }];
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    let p = Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap();
    assert!(
        set_of(&p, "x.png").ends_with("/x2"),
        "{}",
        set_of(&p, "x.png")
    );
    assert_eq!(set_of(&p, "x_wet.png"), set_of(&p, "x.png"));
    assert_eq!(top(&p, "x_wet.png"), (Subtype::SkinWet, Confidence::High));
    assert_eq!(top(&p, "x.png").0, Subtype::SkinAlbedo);
}

/// Trace-debug 2026-10-16 #4: a fuzz map given a sub-type is an ordinary file (spec §4.4), so it neither picks the
/// `_rfaos` recipe nor makes the set Advanced skin.
#[test]
fn a_fuzz_map_given_a_sub_type_does_not_make_the_set_advanced_skin() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "rock.png", colour(64));
    put(d.path(), "rock_n.png", wet_normal(64));
    put(d.path(), "rock_roughness.png", grey(64));
    put(d.path(), "rock_fuzz.png", grey(64));
    let p = plan(
        &d,
        Target::Cs,
        &["file:**/rock_fuzz.png.subtype=generic_data"],
    );
    let outs: Vec<_> = p
        .jobs
        .iter()
        .filter_map(|j| j.output.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .collect();
    assert!(outs.iter().any(|n| n == "rock_rmaos.dds"), "{outs:?}");
    assert!(!outs.iter().any(|n| n == "rock_rfaos.dds"), "{outs:?}");
    let albedo = job(&p, "rock.png");
    assert_eq!(albedo.profile.subtype, Subtype::PbrAlbedo);
    assert!(albedo.profile.srgb_tag());
}

/// Task 3: `_detail_n` joins its cs Advanced skin set as the `skin_detail` member.
#[test]
fn detail_n_joins_its_advanced_skin_set_on_cs() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "h.png", colour(64));
    put(d.path(), "h_rfaos.png", colour(64));
    put(d.path(), "h_detail_n.png", wet_normal(64));
    let p = plan(&d, Target::Cs, &[]);
    assert_eq!(set_of(&p, "h_detail_n.png"), set_of(&p, "h.png"));
    assert_eq!(
        top(&p, "h_detail_n.png"),
        (Subtype::SkinDetail, Confidence::High)
    );
}

#[test]
fn detail_n_outside_an_advanced_skin_set_keeps_todays_grouping() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "h.png", colour(64));
    put(d.path(), "h_detail_n.png", wet_normal(64));
    let p = plan(&d, Target::Cs, &[]);
    assert_ne!(set_of(&p, "h_detail_n.png"), set_of(&p, "h.png"));
    assert!(set_of(&p, "h_detail_n.png").ends_with("/h_detail"));
    assert_ne!(top(&p, "h_detail_n.png").0, Subtype::SkinDetail);
}

#[test]
fn detail_n_is_not_joined_on_vanilla() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "h.png", colour(64));
    put(d.path(), "h_rfaos.png", colour(64));
    put(d.path(), "h_detail_n.png", wet_normal(64));
    let p = plan(&d, Target::Vanilla, &[]);
    assert_ne!(set_of(&p, "h_detail_n.png"), set_of(&p, "h.png"));
    assert_ne!(top(&p, "h_detail_n.png").0, Subtype::SkinDetail);
}

#[test]
fn rename_keeps_the_joined_detail_n() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "x.png", colour(64));
    put(d.path(), "x_rfaos.png", colour(64));
    put(d.path(), "x_detail_n.png", wet_normal(64));
    let mut r = req(&d, Target::Cs, &[]);
    r.set_edits = vec![engine::sets::SetEdit::Rename {
        from: "x".into(),
        to: "x2".into(),
    }];
    let rep = Reporter::new(Arc::new(VecSink::default()), "t");
    let p = Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap();
    assert!(set_of(&p, "x.png").ends_with("/x2"));
    assert_eq!(set_of(&p, "x_detail_n.png"), set_of(&p, "x.png"));
    assert_eq!(
        top(&p, "x_detail_n.png"),
        (Subtype::SkinDetail, Confidence::High)
    );
}
