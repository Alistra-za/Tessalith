//! XPMF Task 2 (Note GI, design X12, X13, X15): in a landscape set `_detail` is its own detail-normal output;
//! `_noise` is the BC4 projected noise; a MATO-only PBR Landscape set needs no RMAOS.
use engine::api::*;
use engine::codes::{Code, Severity};
use engine::config::*;
use engine::events::*;
use engine::policy::*;
use engine::shader::Shader;
use image::{ImageBuffer, Luma, Rgb};
use std::path::Path;
use std::sync::Arc;

mod rnm_support;
use rnm_support::png;

fn grey(dir: &Path, name: &str) {
    let p = dir.join("in").join(name);
    ImageBuffer::from_fn(32, 32, |x, y| Luma([((x + y) * 4) as u8]))
        .save(p)
        .unwrap();
}

/// A flat tangent-space normal (unit length, so no CHAN-NORMAL-LENGTH).
fn flat(dir: &Path, name: &str) {
    let p = dir.join("in").join(name);
    ImageBuffer::from_pixel(32, 32, Rgb([128u8, 128, 255]))
        .save(p)
        .unwrap();
}

/// Plan `<dir>/in` with set `foo`'s shader chosen explicitly; also returns the events.
fn plan(dir: &Path, target: Target, shader: Shader, sets: &[&str]) -> (Plan, Vec<EngineEvent>) {
    let mut req = BatchRequest::new(vec![dir.join("in")], dir.join("out"), target);
    req.accept = AcceptSuggestions::High;
    req.set_shaders = [("foo".to_string(), shader)].into();
    for s in sets {
        req.overrides.push(engine::overrides::parse_set(s).unwrap());
    }
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "landscape-detail");
    let p = Engine::new()
        .unwrap()
        .plan(req, &rep, &CancelToken::default())
        .unwrap();
    (p, sink.take())
}

fn job<'a>(p: &'a Plan, name: &str) -> &'a ResolvedJob {
    p.jobs
        .iter()
        .find(|j| j.input.ends_with(name))
        .unwrap_or_else(|| panic!("no job {name}"))
}

fn three(dir: &Path) {
    for n in ["foo.png", "foo_n.png", "foo_detail.png"] {
        png(dir, n);
    }
}

fn assert_detail_is_its_own_output(target: Target, shader: Shader) {
    let d = tempfile::tempdir().unwrap();
    three(d.path());
    let (p, _) = plan(d.path(), target, shader, &[]);
    let j = job(&p, "foo_detail.png");
    assert_eq!(j.profile.subtype, Subtype::DetailNormal, "{shader:?}");
    assert!(j.output.ends_with("foo_detail.dds"), "{:?}", j.output);
}

#[test]
fn pbr_landscape_detail_is_its_own_output() {
    assert_detail_is_its_own_output(Target::Cs, Shader::PbrLandscape);
}

#[test]
fn landscape_detail_is_its_own_output() {
    assert_detail_is_its_own_output(Target::Vanilla, Shader::Landscape);
}

/// Note GW1: outside a landscape or skin set a `_detail` is an undecided member of its set.
#[test]
fn pbr_set_detail_is_undecided() {
    let d = tempfile::tempdir().unwrap();
    three(d.path());
    let (p, _) = plan(d.path(), Target::Cs, Shader::Pbr, &[]);
    let j = job(&p, "foo_detail.png");
    assert!(
        matches!(j.status, JobStatus::Unresolved { .. }),
        "{:?}",
        j.status
    );
}

/// Ruling R5: a landscape set's `_noise` is decided ProjectedNoise by its shader, with or without a `_detail`.
#[test]
fn landscape_noise_is_decided() {
    for (target, shader) in [
        (Target::Cs, Shader::PbrLandscape),
        (Target::Vanilla, Shader::Landscape),
    ] {
        let d = tempfile::tempdir().unwrap();
        png(d.path(), "foo.png");
        flat(d.path(), "foo_n.png");
        grey(d.path(), "foo_noise.png");
        let (p, _) = plan(d.path(), target, shader, &[]);
        let j = job(&p, "foo_noise.png");
        assert!(
            matches!(j.status, JobStatus::Ready),
            "{shader:?}: {:?}",
            j.status
        );
        assert_eq!(j.profile.subtype, Subtype::ProjectedNoise);
        assert_eq!(
            j.provenance.get("subtype").map(|p| p.to_string()),
            Some(format!("shader:{}", shader.id())),
            "{shader:?}"
        );
    }
}

/// Ruling R5: outside a landscape set `_noise` stays a Medium suggestion, undecided.
#[test]
fn pbr_set_noise_stays_undecided() {
    let d = tempfile::tempdir().unwrap();
    png(d.path(), "foo.png");
    flat(d.path(), "foo_n.png");
    grey(d.path(), "foo_noise.png");
    let (p, _) = plan(d.path(), Target::Cs, Shader::Pbr, &[]);
    let j = job(&p, "foo_noise.png");
    assert!(
        matches!(j.status, JobStatus::Unresolved { .. }),
        "{:?}",
        j.status
    );
    assert!(j.warnings.iter().any(|w| w.code == Code::SugLowConfidence));
}

/// Review Focus 4, Note GW1: `_detail_mask` and `_detail2` beside a `_detail` map plan without a blocking warning on
/// either of them, and the detail map stays DetailNormal.
#[test]
fn detail_mask_and_detail2_leave_the_detail_map_alone() {
    for (target, shader) in [
        (Target::Cs, Shader::PbrLandscape),
        (Target::Vanilla, Shader::Landscape),
    ] {
        let d = tempfile::tempdir().unwrap();
        three(d.path());
        for n in [
            "foo_detail_mask.png",
            "foo_detail2.png",
            "foo_detailmask.png",
        ] {
            png(d.path(), n);
        }
        let (p, _) = plan(d.path(), target, shader, &[]);
        assert_eq!(
            job(&p, "foo_detail.png").profile.subtype,
            Subtype::DetailNormal
        );
        for name in ["foo_detail_mask.png", "foo_detail2.png"] {
            let j = job(&p, name);
            assert!(
                j.warnings.iter().all(|w| w.severity < Severity::Blocking),
                "{shader:?} {name}: {:?}",
                j.warnings
            );
        }
    }
}

/// X15: albedo + `_n` + noise + detail normal, no RMAOS, exports clean.
#[test]
fn mato_only_set_exports_clean() {
    let d = tempfile::tempdir().unwrap();
    png(d.path(), "foo.png");
    flat(d.path(), "foo_n.png");
    flat(d.path(), "foo_detail.png");
    grey(d.path(), "foo_noise.png");
    let (p, ev) = plan(d.path(), Target::Cs, Shader::PbrLandscape, &[]);
    let mut codes: Vec<(Code, Severity)> = p
        .jobs
        .iter()
        .flat_map(|j| &j.warnings)
        .map(|w| (w.code, w.severity))
        .collect();
    let mut set_warnings = vec![];
    for e in &ev {
        if let EventKind::SetResolved { warnings, .. } = &e.kind {
            set_warnings.extend(warnings.iter().map(|w| w.code));
            codes.extend(warnings.iter().map(|w| (w.code, w.severity)));
        }
    }
    assert!(set_warnings.is_empty(), "check_maps: {set_warnings:?}");
    assert!(
        codes.iter().all(|(_, s)| *s < Severity::Warning),
        "{codes:?}"
    );
    for j in &p.jobs {
        assert!(
            matches!(j.status, JobStatus::Ready),
            "{:?}: {:?}",
            j.input,
            j.status
        );
    }
    let noise = job(&p, "foo_noise.png");
    assert_eq!(
        (noise.profile.subtype, noise.profile.format),
        (Subtype::ProjectedNoise, Format::Bc4)
    );
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink, "landscape-detail-run");
    Engine::new()
        .unwrap()
        .run(p, &rep, &CancelToken::default())
        .unwrap();
    let dds = std::fs::read(d.path().join("out").join("foo_noise.dds")).unwrap();
    let u32_at = |o: usize| u32::from_le_bytes(dds[o..o + 4].try_into().unwrap());
    // DDS_HEADER.dwMipMapCount (offset 28), DX10 dxgiFormat (offset 128); 32x32 has six levels.
    assert_eq!((u32_at(128), u32_at(28)), (80, 6));
    assert!(d.path().join("out").join("foo_detail.dds").exists());
}
