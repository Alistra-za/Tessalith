//! XPMF Task 2 (Note GI, design X12, X13, X15): in a landscape set `_detail` is its own detail-normal output, not an
//! RNM layer; `_noise` is the BC4 projected noise; a MATO-only PBR Landscape set needs no RMAOS.
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

fn is_layer(j: &ResolvedJob) -> bool {
    matches!(j.status, JobStatus::NormalLayer { .. })
}

fn layer_files(j: &ResolvedJob) -> Vec<String> {
    j.rnm.as_ref().map_or(vec![], |r| {
        r.layers
            .iter()
            .map(|l| l.file.path.file_name().unwrap().to_string_lossy().into())
            .collect()
    })
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
    assert!(!is_layer(j), "{shader:?}: {:?}", j.status);
    assert_eq!(j.profile.subtype, Subtype::DetailNormal, "{shader:?}");
    assert!(j.output.ends_with("foo_detail.dds"), "{:?}", j.output);
    assert!(job(&p, "foo_n.png").active_rnm().is_none(), "{shader:?}");
}

#[test]
fn pbr_landscape_detail_is_its_own_output() {
    assert_detail_is_its_own_output(Target::Cs, Shader::PbrLandscape);
}

#[test]
fn landscape_detail_is_its_own_output() {
    assert_detail_is_its_own_output(Target::Vanilla, Shader::Landscape);
}

#[test]
fn pbr_set_detail_stays_rnm() {
    let d = tempfile::tempdir().unwrap();
    three(d.path());
    let (p, _) = plan(d.path(), Target::Cs, Shader::Pbr, &[]);
    assert!(is_layer(job(&p, "foo_detail.png")));
    let n = job(&p, "foo_n.png");
    assert_eq!(layer_files(n), ["foo_detail.png"]);
    assert!(n.active_rnm().is_some());
}

/// X13: a hand-set `rnm.target` keeps the file an RNM layer.
#[test]
fn rnm_target_keeps_layer() {
    for (target, shader) in [
        (Target::Cs, Shader::PbrLandscape),
        (Target::Vanilla, Shader::Landscape),
    ] {
        let d = tempfile::tempdir().unwrap();
        three(d.path());
        let (p, _) = plan(
            d.path(),
            target,
            shader,
            &["file:foo_detail.png.rnm.target=n"],
        );
        assert!(is_layer(job(&p, "foo_detail.png")), "{shader:?}");
        assert_eq!(layer_files(job(&p, "foo_n.png")), ["foo_detail.png"]);
    }
}

/// Review Focus 4: the mask and Detail2 stay layers whose base is gone; no panic, no Error, no new blocker.
#[test]
fn detail_mask_and_detail2_stay_layers() {
    for (target, shader) in [
        (Target::Cs, Shader::PbrLandscape),
        (Target::Vanilla, Shader::Landscape),
    ] {
        let d = tempfile::tempdir().unwrap();
        three(d.path());
        // The vocabulary spells the mask `_detail_mask` (or `_detailnormmask`); `_detailmask` is no layer term.
        for n in [
            "foo_detail_mask.png",
            "foo_detail2.png",
            "foo_detailmask.png",
        ] {
            png(d.path(), n);
        }
        let (p, _) = plan(d.path(), target, shader, &[]);
        // The brief's `foo_detailmask` spelling is no layer term: it is its own set's diffuse, as today.
        assert!(!is_layer(job(&p, "foo_detailmask.png")), "{shader:?}");
        assert_eq!(
            job(&p, "foo_detail.png").profile.subtype,
            Subtype::DetailNormal
        );
        let mut observed = vec![];
        for name in ["foo_detail_mask.png", "foo_detail2.png"] {
            let j = job(&p, name);
            assert!(is_layer(j), "{shader:?} {name}: {:?}", j.status);
            assert!(
                j.warnings.iter().all(|w| w.severity < Severity::Blocking),
                "{shader:?} {name}: {:?}",
                j.warnings
            );
            observed.push(j.warnings.iter().map(|w| w.code).collect::<Vec<_>>());
        }
        // Pinned as observed: the mask's layer was retyped, so it is RNM-MASK-ORPHAN (Warning); Detail2 needs no
        // Detail layer, so it still blends into `_n` with no code.
        assert_eq!(observed, [vec![Code::RnmMaskOrphan], vec![]], "{shader:?}");
        assert_eq!(layer_files(job(&p, "foo_n.png")), ["foo_detail2.png"]);
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
    // `_noise` is a Medium suggestion (Task 1), so the sub-type is given as the GUI's noise slot writes it.
    let (p, ev) = plan(
        d.path(),
        Target::Cs,
        Shader::PbrLandscape,
        &["file:foo_noise.png.subtype=projected_noise"],
    );
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
