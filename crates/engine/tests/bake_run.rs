//! M4 Task 7: a roughness member with a `BakeJob` decodes its `_n`, widens its roughness, records the `_n` in the
//! manifest, re-encodes when the `_n` or its decode settings change, and fails when the `_n` changed after
//! planning (DECISIONS R4, R12).
use engine::api::*;
use engine::bake::BakeJob;
use engine::config::*;
use engine::dds::read_dds;
use engine::decode::{decode_level, Decoded};
use engine::events::*;
use engine::manifest::{sha256_file, Manifest};
use engine::policy::*;
use engine::presets::PresetRegistry;
use image::{ImageBuffer, Rgb, Rgba};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A unit-length normal map whose direction changes from texel to texel; another `phase` gives another map.
fn bumpy_n(dir: &Path, w: u32, phase: f32) -> PathBuf {
    let p = dir.join("in/iron_n.png");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(w, w, |x, y| {
        let nx = 0.6 * (x as f32 * 1.3 + y as f32 * 0.7 + phase).sin();
        let ny = 0.6 * (x as f32 * 0.9 - y as f32 * 1.1).cos();
        let nz = (1.0 - nx * nx - ny * ny).sqrt();
        let e = |v: f32| ((v * 0.5 + 0.5) * 255.0).round() as u8;
        Rgb([e(nx), e(ny), e(nz)])
    })
    .save(&p)
    .unwrap();
    p
}

/// A constant roughness of 100/255 (G 0, B 255, A 255).
fn rmaos(dir: &Path, w: u32) -> PathBuf {
    let p = dir.join("in/iron_rmaos.png");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(w, w, |_, _| Rgba([100u8, 0, 255, 255]))
        .save(&p)
        .unwrap();
    p
}

/// The `_rmaos` job as `Engine::plan` builds it for a baking set (stamps taken now). `bake: false` leaves the
/// `BakeJob` off; `flip_green` changes the `_n`'s decode settings.
fn plan(dir: &Path, rough: &Path, normal: &Path, bake: bool, flip_green: bool) -> Plan {
    let registry = PresetRegistry::builtin().unwrap();
    let mut normal_profile = registry
        .profile("cs-quality", Target::Cs, Subtype::PbrNormal)
        .unwrap();
    normal_profile.channels.flip_green = flip_green;
    let mut request = BatchRequest::new(vec![dir.join("in")], dir.join("out"), Target::Cs);
    request.on_existing = OnExisting::IfChanged;
    let job = ResolvedJob {
        input: rough.to_path_buf(),
        output: dir.join("out/iron_rmaos.dds"),
        game_path: "iron_rmaos.png".into(),
        set_key: "out/iron".into(),
        profile: registry
            .profile("cs-quality", Target::Cs, Subtype::Rmaos)
            .unwrap(),
        provenance: Default::default(),
        suggestions: vec![],
        warnings: vec![],
        status: JobStatus::Ready,
        suffix_alias: None,
        source: SourceStamp::of(rough),
        pack: None,
        bake: bake.then(|| BakeJob {
            normal: normal.to_path_buf(),
            normal_profile,
            shift: 0,
            stamp: SourceStamp::of(normal),
            fit: None,
        }),
        shader: None,
        derive: None,
        cube: None,
        fill: None,
        unknown_suffix: false,
    };
    Plan {
        request,
        registry,
        preset_id: "cs-quality".into(),
        target: Target::Cs,
        jobs: vec![job],
        skipped: vec![],
        failed: vec![],
        peak_admitted_bytes: 0,
        cancelled: vec![],
        shaders: Default::default(),
        derive_own: Default::default(),
        fill_ends: Default::default(),
        helpers: vec![],
        no_set: Default::default(),
        no_set_sets: Default::default(),
    }
}

fn run(plan: Plan) -> (BatchSummary, Vec<EngineEvent>) {
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "bake-run");
    let s = Engine::new()
        .unwrap()
        .run(plan, &rep, &CancelToken::default())
        .unwrap();
    (s, sink.take())
}

/// Mean decoded R of mip `level` of `out/iron_rmaos.dds`, in codes.
fn mean_r(dir: &Path, level: usize) -> f64 {
    let f = read_dds(&std::fs::read(dir.join("out/iron_rmaos.dds")).unwrap()).unwrap();
    let (w, h, r) = f.levels[level].clone();
    let Decoded::Ldr(px) = decode_level(f.format, w, h, &f.data[r]).unwrap() else {
        panic!("LDR expected")
    };
    px.iter().map(|p| f64::from(p[0])).sum::<f64>() / px.len() as f64
}

#[test]
fn a_baked_member_is_rougher_and_records_its_normal() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for (d, bake) in [(&a, true), (&b, false)] {
        let n = bumpy_n(d.path(), 32, 0.0);
        let r = rmaos(d.path(), 32);
        let (s, ev) = run(plan(d.path(), &r, &n, bake, false));
        assert_eq!(s.converted, 1, "{ev:#?}");
    }
    for level in 1..=3 {
        let (baked, plain) = (mean_r(a.path(), level), mean_r(b.path(), level));
        assert!(baked > plain + 2.0, "mip {level}: {baked} vs {plain}");
    }
    // 1:1, so mip 0 has no variance: both encode the same constant.
    assert!((mean_r(a.path(), 0) - mean_r(b.path(), 0)).abs() < 1.0);
    let m = Manifest::at(&a.path().join("out")).load().unwrap();
    let e = m.values().next().unwrap();
    assert_eq!(e.sources.len(), 1, "{:?}", e.sources);
    assert!(e.sources[0].path.ends_with("iron_n.png"));
    assert_eq!(
        e.sources[0].sha256,
        sha256_file(&a.path().join("in/iron_n.png")).unwrap()
    );
    assert!(e.sources[0].bake.is_some());
    let plain = Manifest::at(&b.path().join("out")).load().unwrap();
    assert!(plain.values().next().unwrap().sources.is_empty());
}

#[test]
fn kept_normal_dds_preserves_length_at_two_lower_levels() {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("iron_n.png");
    let source = ImageBuffer::from_fn(4, 4, |x, y| {
        Rgba(match (x % 2, y / 2) {
            (0, 0) => [173u8, 116, 137, 16],
            (1, 0) => [108, 144, 140, 124],
            (0, _) => [131, 117, 187, 210],
            _ => [144, 132, 169, 250],
        })
    });
    source.save(&input).unwrap();
    let mut req = BatchRequest::new(vec![input], d.path().join("out"), Target::Cs);
    req.overrides = [
        "file:*iron_n.png.subtype=normal",
        "file:*iron_n.png.format=rgba8",
        "file:*iron_n.png.color.tag=unorm",
        "file:*iron_n.png.mips.normal_base=keep",
        "file:*iron_n.png.alpha.policy=data",
    ]
    .map(|s| engine::overrides::parse_set(s).unwrap())
    .into();
    let rep = Reporter::new(Arc::new(VecSink::default()), "kept-normal");
    let p = Engine::new()
        .unwrap()
        .plan(req, &rep, &CancelToken::default())
        .unwrap();
    assert_eq!(p.jobs.len(), 1);
    let output = p.jobs[0].output.clone();
    let (summary, events) = run(p);
    assert_eq!(summary.converted, 1, "{events:#?}");
    let dds = read_dds(&std::fs::read(output).unwrap()).unwrap();
    assert_eq!(dds.format, Format::Rgba8.pixel_format(false));
    assert_eq!(dds.levels.len(), 3);
    for (level, (w, h, range)) in dds.levels.iter().enumerate() {
        let Decoded::Ldr(pixels) =
            decode_level(dds.format, *w, *h, &dds.data[range.clone()]).unwrap()
        else {
            panic!("expected RGBA8")
        };
        assert_eq!((*w, *h), (4 >> level, 4 >> level));
        let side = 1 << level;
        for y in 0..*h {
            for x in 0..*w {
                let got = pixels[(y * w + x) as usize];
                if level == 0 {
                    assert_eq!(got, source.get_pixel(x, y).0);
                    continue;
                }
                // Reference each footprint directly to the signed base samples.
                let mut sum = [0.0; 3];
                let mut radius = 0.0;
                let mut alpha = 0.0;
                for dy in 0..side {
                    for dx in 0..side {
                        let [r, g, b, a] = source.get_pixel(side * x + dx, side * y + dy).0;
                        let v = [r, g, b].map(|c| f64::from(c) / 127.5 - 1.0);
                        radius += v.iter().map(|c| c * c).sum::<f64>().sqrt();
                        for i in 0..3 {
                            sum[i] += v[i];
                        }
                        alpha += f64::from(a);
                    }
                }
                let area = f64::from(side * side);
                let scale = radius / area / sum.iter().map(|c| c * c).sum::<f64>().sqrt();
                let [r, g, b] = sum.map(|c| ((c * scale + 1.0) * 127.5).round() as u8);
                let want = [r, g, b, (alpha / area).round() as u8];
                assert!(
                    got.iter().zip(want).all(|(a, b)| a.abs_diff(b) <= 1),
                    "mip {level} ({x},{y}): {got:?}, expected {want:?}"
                );
            }
        }
    }
}

#[test]
fn keep_upgrade_obeys_encode_version_scope() {
    use engine::codes::Code;
    use engine::manifest::{encode_version, output_key};
    use engine::overrides::parse_set;

    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("iron_n.png");
    ImageBuffer::from_pixel(8, 8, Rgb([160u8, 112, 192]))
        .save(&input)
        .unwrap();
    let engine = Engine::new().unwrap();
    let rep = Reporter::new(Arc::new(VecSink::default()), "keep-upgrade");
    let resolve = |out: &str, base: Option<&str>, ev| {
        let mut req = BatchRequest::new(vec![input.clone()], d.path().join(out), Target::Cs);
        req.on_existing = OnExisting::IfChanged;
        req.ev = ev;
        req.overrides = [
            "file:*iron_n.png.subtype=normal",
            "file:*iron_n.png.format=rgba8",
            "file:*iron_n.png.color.tag=unorm",
        ]
        .map(|s| parse_set(s).unwrap())
        .into();
        if let Some(base) = base {
            req.overrides
                .push(parse_set(&format!("file:*iron_n.png.mips.normal_base={base}")).unwrap());
        }
        engine.plan(req, &rep, &CancelToken::default()).unwrap()
    };
    let keep = |ev| resolve("out", Some("keep"), ev);
    let seed = keep(EvScope::Off);
    assert_eq!(seed.jobs.len(), 1);
    let job = &seed.jobs[0];
    assert_eq!(job.profile.subtype, Subtype::Normal);
    assert_eq!(job.profile.mips.normal_base, NormalBase::Keep);
    let output = job.output.clone();
    let key = output_key(&seed.request.output_root, &output);
    let manifest = Manifest::at(&seed.request.output_root);
    let (s, events) = run(seed);
    assert_eq!((s.converted, s.validation_failed), (1, 0), "{events:#?}");
    let kept_dds = std::fs::read(&output).unwrap();
    let mut record = manifest.read().unwrap()[&key].raw.clone();
    // Normal's encode-version arm before the Keep mip-chain change.
    record["encode_version"] = "2.0.0".into();
    let older = format!("{record}\n");
    std::fs::write(manifest.path(), &older).unwrap();

    for ev in [
        EvScope::Off,
        EvScope::matching(vec!["diffuse".parse().unwrap()]).unwrap(),
    ] {
        let (s, events) = run(keep(ev));
        assert_eq!((s.converted, s.up_to_date, s.kept_older_encode), (0, 1, 1));
        assert!(events
            .iter()
            .any(|e| matches!(&e.kind, EventKind::Warning(w)
            if w.code == Code::OutKeptOlderEncode && w.data["by_subtype"]["normal"] == 1)));
        assert_eq!(std::fs::read_to_string(manifest.path()).unwrap(), older);
        assert_eq!(std::fs::read(&output).unwrap(), kept_dds);
    }
    let matching = EvScope::matching(vec!["normal".parse().unwrap()]).unwrap();
    let (s, events) = run(keep(matching.clone()));
    assert_eq!((s.converted, s.validation_failed), (1, 0), "{events:#?}");
    assert_eq!(
        manifest.read().unwrap()[&key].encode_version.as_deref(),
        Some(encode_version(Subtype::Normal))
    );
    let (s, _) = run(keep(matching));
    assert_eq!((s.converted, s.up_to_date, s.kept_older_encode), (0, 1, 0));

    let encode = |out, base| {
        let p = resolve(out, base, EvScope::Off);
        let path = p.jobs[0].output.clone();
        let (s, events) = run(p);
        assert_eq!((s.converted, s.validation_failed), (1, 0), "{events:#?}");
        std::fs::read(path).unwrap()
    };
    let explicit = encode("explicit", Some("renormalize"));
    assert_ne!(explicit, kept_dds);
    assert_eq!(encode("omitted", None), explicit);
}

#[test]
fn acknowledged_bc5_keep_fails_before_if_changed() {
    use engine::codes::{Code, Severity};
    use engine::manifest::output_key;
    use engine::overrides::parse_set;

    let d = tempfile::tempdir().unwrap();
    let normal = bumpy_n(d.path(), 32, 0.0);
    let rough = rmaos(d.path(), 32);
    let resolve = |root: &str, base: &str, bake_off: bool| {
        let mut req = BatchRequest::new(vec![d.path().join("in")], d.path().join(root), Target::Cs);
        req.on_existing = OnExisting::IfChanged;
        req.accept = AcceptSuggestions::High;
        req.allow.insert(Code::FmtBc5Normal);
        req.overrides = [
            "file:*iron_n.png.subtype=pbr_normal",
            "file:*iron_n.png.format=bc5",
            "file:*iron_n.png.color.tag=unorm",
            &format!("file:*iron_n.png.mips.normal_base={base}"),
        ]
        .into_iter()
        .map(|s| parse_set(s).unwrap())
        .collect();
        if bake_off {
            req.overrides
                .push(parse_set("file:*iron_n.png.mips.normal=renormalize").unwrap());
        }
        let rep = Reporter::new(Arc::new(VecSink::default()), "bc5-keep");
        Engine::new()
            .unwrap()
            .plan(req, &rep, &CancelToken::default())
            .unwrap()
    };
    let seed = || {
        let mut p = resolve("out", "renormalize", false);
        assert_eq!(p.jobs.len(), 2);
        for j in &mut p.jobs {
            assert_eq!(j.status, JobStatus::Ready, "{:?}", j.warnings);
            if j.input == normal {
                assert_eq!(j.profile.format, Format::Bc5);
                j.profile.mips.normal_base = NormalBase::Keep;
            } else {
                assert_eq!(j.input, rough);
                let bake = j.bake.as_mut().unwrap();
                assert_eq!(bake.normal, normal);
                bake.normal_profile.mips.normal_base = NormalBase::Keep;
            }
        }
        p
    };
    let p = seed();
    let out = &d.path().join("out");
    let normal_out = p
        .jobs
        .iter()
        .find(|j| j.input == normal)
        .unwrap()
        .output
        .clone();
    let rough_out = p
        .jobs
        .iter()
        .find(|j| j.input == rough)
        .unwrap()
        .output
        .clone();
    let normal_key = output_key(out, &normal_out);
    let rough_key = output_key(out, &rough_out);
    let (s, events) = run(p);
    assert_eq!((s.converted, s.validation_failed), (2, 0), "{events:#?}");
    assert_eq!(run(seed()).0.up_to_date, 2);
    let manifest = Manifest::at(out);
    let before = manifest.load().unwrap();
    let stamp = SourceStamp::of(&normal_out).unwrap();
    let bytes = std::fs::read(&normal_out).unwrap();
    let baked = std::fs::read(&rough_out).unwrap();
    assert_eq!(before[&normal_key].output_stamp, Some(stamp));
    assert_eq!(before[&rough_key].sources.len(), 1);
    assert!(before[&rough_key].sources[0].path.ends_with("iron_n.png"));

    let p = resolve("out", "keep", false);
    let n = p.jobs.iter().find(|j| j.input == normal).unwrap();
    assert_eq!(
        n.status,
        JobStatus::Error {
            codes: vec![Code::CfgInvalid]
        }
    );
    let r = p.jobs.iter().find(|j| j.input == rough).unwrap();
    assert!(r.bake.is_none());
    assert!(r
        .warnings
        .iter()
        .any(|w| w.code == Code::SetMissingCompanion && w.severity == Severity::Info));
    let (s, events) = run(p);
    assert_eq!(
        (s.failed, s.converted, s.up_to_date, s.validation_failed),
        (1, 1, 0, 0),
        "{events:#?}"
    );
    assert!(events.iter().any(|e| e.job.as_ref() == Some(&normal_key) &&
        matches!(&e.kind, EventKind::FileFailed { stage: Stage::Resolve, error } if error.contains("CFG-INVALID"))));
    let after = manifest.load().unwrap();
    assert_eq!(std::fs::read(&normal_out).unwrap(), bytes);
    assert_eq!(SourceStamp::of(&normal_out), Some(stamp));
    assert_eq!(after[&normal_key], before[&normal_key]);
    assert!(after[&rough_key].sources.is_empty());
    let plain = resolve("plain", "renormalize", true);
    let r = plain.jobs.iter().find(|j| j.input == rough).unwrap();
    assert!(r.bake.is_none());
    let reference = r.output.clone();
    assert_eq!(run(plain).0.converted, 2);
    let rewritten = std::fs::read(rough_out).unwrap();
    assert_ne!(rewritten, baked);
    assert_eq!(rewritten, std::fs::read(reference).unwrap());
}

#[test]
fn a_two_to_one_normal_leaves_mip_0_as_authored() {
    // Note HD1: the bake adds nothing to mip 0, even at shift 1.
    let d = tempfile::tempdir().unwrap();
    let n = bumpy_n(d.path(), 64, 0.0);
    let r = rmaos(d.path(), 32);
    let mut p = plan(d.path(), &r, &n, true, false);
    p.jobs[0].bake.as_mut().unwrap().shift = 1;
    assert_eq!(run(p).0.converted, 1);
    assert!(mean_r(d.path(), 0) < 102.0, "{}", mean_r(d.path(), 0));
}

/// A 64x32 `_rmaos` over a 64x64 `_n`: the planned job fits the `_n` to the member (Note HD2).
fn fitted_plan(d: &Path, bake: bool) -> Plan {
    let n = bumpy_n(d, 64, 0.0);
    let r = d.join("in/iron_rmaos.png");
    ImageBuffer::from_fn(64, 32, |_, _| Rgba([100u8, 0, 255, 255]))
        .save(&r)
        .unwrap();
    let mut p = plan(d, &r, &n, bake, false);
    if let Some(b) = p.jobs[0].bake.as_mut() {
        b.fit = Some((64, 32));
    }
    p
}

#[test]
fn a_fitted_bake_widens_the_written_roughness() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for (d, bake) in [(&a, true), (&b, false)] {
        let (s, ev) = run(fitted_plan(d.path(), bake));
        assert_eq!(s.converted, 1, "{ev:#?}");
    }
    let (baked, plain) = (mean_r(a.path(), 1), mean_r(b.path(), 1));
    assert!(baked > plain + 2.0, "mip 1: {baked} vs {plain}");
    assert!((mean_r(a.path(), 0) - mean_r(b.path(), 0)).abs() < 1.0);
}

#[test]
fn a_fitted_bake_is_up_to_date_on_a_second_run() {
    let d = tempfile::tempdir().unwrap();
    assert_eq!(run(fitted_plan(d.path(), true)).0.converted, 1);
    let (s, _) = run(fitted_plan(d.path(), true));
    assert_eq!((s.converted, s.up_to_date), (0, 1));
}

#[test]
fn a_stale_fit_fails_instead_of_baking() {
    let d = tempfile::tempdir().unwrap();
    let mut p = fitted_plan(d.path(), true);
    p.jobs[0].bake.as_mut().unwrap().fit = Some((32, 16));
    let (s, ev) = run(p);
    assert_eq!((s.converted, s.failed), (0, 1), "{ev:#?}");
    assert!(
        ev.iter().any(|e| matches!(&e.kind,
            EventKind::FileFailed { error, .. } if error.contains("size changed"))),
        "{ev:#?}"
    );
}

#[test]
fn flat_short_normal_adds_no_roughness_in_either_base_mode() {
    let levels = |base: Option<NormalBase>| {
        let d = tempfile::tempdir().unwrap();
        let n = d.path().join("in/iron_n.png");
        std::fs::create_dir_all(n.parent().unwrap()).unwrap();
        // (128, 128, 191) decodes to about (0.004, 0.004, 0.498): length 0.5, pointing up.
        ImageBuffer::from_fn(32, 32, |_, _| Rgb([128u8, 128, 191]))
            .save(&n)
            .unwrap();
        let r = rmaos(d.path(), 32);
        let mut p = plan(d.path(), &r, &n, base.is_some(), false);
        p.jobs[0].profile.format = Format::Rgba8;
        if let Some(b) = base {
            p.jobs[0]
                .bake
                .as_mut()
                .unwrap()
                .normal_profile
                .mips
                .normal_base = b;
        }
        assert_eq!(run(p).0.converted, 1);
        [0, 1, 2].map(|level| mean_r(d.path(), level))
    };
    let plain = levels(None);
    assert_eq!(plain, [100.0; 3]);
    for base in [NormalBase::Renormalize, NormalBase::Keep] {
        assert_eq!(levels(Some(base)), plain, "{base:?}");
    }
}

#[test]
fn keep_bake_changes_only_angular_roughness() {
    let d = tempfile::tempdir().unwrap();
    let input = d.path().join("in");
    let dir = input.join("textures/pbr");
    std::fs::create_dir_all(&dir).unwrap();
    ImageBuffer::from_fn(8, 8, |x, _| {
        Rgb([
            if x < 4 {
                128u8
            } else if x % 2 == 0 {
                146
            } else {
                109
            },
            128,
            189,
        ])
    })
    .save(dir.join("iron_n.png"))
    .unwrap();
    for name in ["iron_rmaos.png", "skin_rfaos.png"] {
        ImageBuffer::from_fn(8, 8, |x, y| Rgba([40u8, x as u8 * 17, 225, y as u8 * 13]))
            .save(dir.join(name))
            .unwrap();
    }
    let outputs = |base, mode| {
        let out = d.path().join(format!("{base}-{mode}"));
        let mut req = BatchRequest::new(vec![input.clone()], out, Target::Cs);
        req.recursive = true;
        req.accept = AcceptSuggestions::High;
        req.on_existing = OnExisting::IfChanged;
        req.borrowed_normal
            .insert("textures/pbr/skin".into(), "textures/pbr/iron".into());
        req.overrides = [
            "format=rgba8",
            "color.tag=unorm",
            "file:**/iron_n.png.subtype=pbr_normal",
            "file:**/skin_rfaos.png.subtype=skin_rfaos",
            &format!("file:**/iron_n.png.mips.normal_base={base}"),
            &format!("file:**/iron_n.png.mips.normal={mode}"),
        ]
        .map(|s| engine::overrides::parse_set(s).unwrap())
        .into();
        let resolve = || {
            Engine::new()
                .unwrap()
                .plan(
                    req.clone(),
                    &Reporter::new(Arc::new(NullSink), "rough"),
                    &CancelToken::default(),
                )
                .unwrap()
        };
        let p = resolve();
        let members = [Subtype::Rmaos, Subtype::SkinRfaos].map(|st| {
            let j = p.jobs.iter().find(|j| j.profile.subtype == st).unwrap();
            assert_eq!(
                j.bake.as_ref().map(|b| &b.normal),
                (mode == "renormalize_bake").then_some(&dir.join("iron_n.png"))
            );
            j.output.clone()
        });
        let (s, ev) = run(p);
        assert_eq!(s.converted, 3, "{ev:#?}");
        assert_eq!(run(resolve()).0.up_to_date, 3);
        members.map(|path| {
            let f = read_dds(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(f.format, Format::Rgba8.pixel_format(false));
            [0, 1, 2].map(|level: usize| {
                let (w, h, r) = &f.levels[level];
                let Decoded::Ldr(px) = decode_level(f.format, *w, *h, &f.data[r.clone()]).unwrap()
                else {
                    panic!("expected RGBA8");
                };
                px
            })
        })
    };
    let plain = outputs("keep", "renormalize");
    let unit = outputs("renormalize", "renormalize_bake");
    let kept = outputs("keep", "renormalize_bake");
    for member in 0..2 {
        for level in 0..=2 {
            let width = 8 >> level;
            for (i, px) in kept[member][level].iter().enumerate() {
                let off = &plain[member][level][i];
                let reference = &unit[member][level][i];
                assert_eq!(&px[1..], &off[1..]);
                if level == 0 || i % width < width / 2 {
                    assert_eq!(px, off, "member {member} mip {level} texel {i}");
                } else {
                    assert!(reference[0] > off[0]);
                    assert_eq!(px, reference, "member {member} mip {level} texel {i}");
                }
            }
        }
    }
}

#[test]
fn up_to_date_until_the_normal_or_its_decode_settings_change() {
    let d = tempfile::tempdir().unwrap();
    let n = bumpy_n(d.path(), 32, 0.0);
    let r = rmaos(d.path(), 32);
    assert_eq!(run(plan(d.path(), &r, &n, true, false)).0.converted, 1);
    let (s, _) = run(plan(d.path(), &r, &n, true, false));
    assert_eq!((s.converted, s.up_to_date), (0, 1));
    // The `_n`'s green flip is part of the bake's identity.
    let (s, _) = run(plan(d.path(), &r, &n, true, true));
    assert_eq!((s.converted, s.up_to_date), (1, 0));
    let (s, _) = run(plan(d.path(), &r, &n, true, true));
    assert_eq!((s.converted, s.up_to_date), (0, 1));
    // New `_n` bytes.
    bumpy_n(d.path(), 32, 1.0);
    let (s, _) = run(plan(d.path(), &r, &n, true, true));
    assert_eq!((s.converted, s.up_to_date), (1, 0));
    // How the `_n`'s mip 0 is written (`mips.normal_base`) is part of the bake's identity too (DECISIONS R1, R12).
    let keep = || {
        let mut p = plan(d.path(), &r, &n, true, true);
        p.jobs[0]
            .bake
            .as_mut()
            .unwrap()
            .normal_profile
            .mips
            .normal_base = NormalBase::Keep;
        p
    };
    let (s, _) = run(keep());
    assert_eq!((s.converted, s.up_to_date), (1, 0));
    let (s, _) = run(keep());
    assert_eq!((s.converted, s.up_to_date), (0, 1));
    // The member's own `mips.bake_clamp` (in its profile hash) and its own bytes re-encode as for any job.
    let clamp = || {
        let mut p = keep();
        p.jobs[0].profile.mips.bake_clamp = 0.5;
        p
    };
    let (s, _) = run(clamp());
    assert_eq!((s.converted, s.up_to_date), (1, 0));
    let (s, _) = run(clamp());
    assert_eq!((s.converted, s.up_to_date), (0, 1));
    ImageBuffer::from_fn(32, 32, |_, _| Rgba([140u8, 0, 255, 255]))
        .save(&r)
        .unwrap();
    let (s, _) = run(clamp());
    assert_eq!((s.converted, s.up_to_date), (1, 0));
    // Turning the bake off re-encodes too.
    let (s, _) = run(plan(d.path(), &r, &n, false, true));
    assert_eq!((s.converted, s.up_to_date), (1, 0));
}

/// Task 6 carry: `coat_normal` defaults to `mips.normal = renormalize_bake` for its own self-bake, which must not
/// make it a companion-bake target. Only roughness members widen from a `BakeJob`; one handed to a `_cnr` changes
/// none of its bytes.
#[test]
fn a_bake_job_on_a_coat_normal_changes_nothing() {
    let registry = PresetRegistry::builtin().unwrap();
    let coat = registry
        .profile("cs-quality", Target::Cs, Subtype::CoatNormal)
        .unwrap();
    assert_eq!(coat.mips.normal, NormalMips::RenormalizeBake);
    let bytes = |bake: bool| {
        let d = tempfile::tempdir().unwrap();
        let n = bumpy_n(d.path(), 32, 0.0);
        let cnr = d.path().join("in/iron_cnr.png");
        ImageBuffer::from_fn(32, 32, |x, y| {
            let v = (128 + (x as i32 * 7 - y as i32 * 5).rem_euclid(60) - 30) as u8;
            Rgba([v, 255 - v, 220, 140])
        })
        .save(&cnr)
        .unwrap();
        let mut p = plan(d.path(), &cnr, &n, bake, false);
        p.jobs[0].profile = coat.clone();
        p.jobs[0].output = d.path().join("out/iron_cnr.dds");
        p.jobs[0].game_path = "iron_cnr.png".into();
        let (s, ev) = run(p);
        assert_eq!(s.converted, 1, "{ev:#?}");
        std::fs::read(d.path().join("out/iron_cnr.dds")).unwrap()
    };
    assert_eq!(bytes(true), bytes(false));
}

/// Run a planned baking member after `edit` rewrote its `_n`; the member must fail at Load with this error.
fn fails_after(edit: impl FnOnce(&Path)) -> String {
    let d = tempfile::tempdir().unwrap();
    let n = bumpy_n(d.path(), 32, 0.0);
    let r = rmaos(d.path(), 32);
    let p = plan(d.path(), &r, &n, true, false);
    edit(d.path());
    let (s, ev) = run(p);
    assert_eq!((s.converted, s.failed), (0, 1), "{ev:#?}");
    assert!(!d.path().join("out/iron_rmaos.dds").exists());
    ev.iter()
        .find_map(|e| match &e.kind {
            EventKind::FileFailed {
                stage: Stage::Load,
                error,
            } => Some(error.clone()),
            _ => None,
        })
        .unwrap()
}

/// Review Focus 3: the `_n` is a source of the member (final review I7).
#[test]
fn a_normal_changed_after_planning_fails_the_member() {
    // Another size: the stamp catches it before the shift guard could.
    let err = fails_after(|d| {
        bumpy_n(d, 64, 0.0);
    });
    assert!(
        err.ends_with("iron_n.png: source changed since planning; re-run"),
        "{err}"
    );
}

/// The commonest edit: the `_n` repainted at the same size. Only the stamp (its modification time) can catch it.
#[test]
fn a_normal_repainted_at_the_same_size_after_planning_fails_the_member() {
    let err = fails_after(|d| {
        let n = bumpy_n(d, 32, 1.0);
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(10);
        std::fs::File::options()
            .write(true)
            .open(&n)
            .unwrap()
            .set_modified(later)
            .unwrap();
    });
    assert!(
        err.ends_with("iron_n.png: source changed since planning; re-run"),
        "{err}"
    );
}

/// Final review Minor 2 (DECISIONS R12): the `_n`'s bytes do not depend on whether it bakes, so attaching or
/// detaching the bake (its roughness member leaves the batch and comes back) leaves the `_n` up to date.
#[test]
fn the_n_stays_up_to_date_when_its_bake_turns_on_or_off() {
    let d = tempfile::tempdir().unwrap();
    // Under `textures/pbr/`, so the lone `_n` is still a `pbr_normal` (path-based PBR set).
    let pbr = d.path().join("in/textures/pbr");
    std::fs::create_dir_all(&pbr).unwrap();
    std::fs::rename(bumpy_n(d.path(), 32, 0.0), pbr.join("iron_n.png")).unwrap();
    let r = pbr.join("iron_rmaos.png");
    std::fs::rename(rmaos(d.path(), 32), &r).unwrap();
    let aside = d.path().join("iron_rmaos.png");
    let convert = || {
        let mut req =
            BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
        req.recursive = true;
        req.accept = AcceptSuggestions::High;
        req.on_existing = OnExisting::IfChanged;
        let sink = Arc::new(VecSink::default());
        let rep = Reporter::new(sink.clone(), "bake-toggle");
        let e = Engine::new().unwrap();
        let p = e.plan(req, &rep, &CancelToken::default()).unwrap();
        let n = p
            .jobs
            .iter()
            .find(|j| j.input.ends_with("iron_n.png"))
            .expect("the _n has a job");
        assert_eq!(n.profile.subtype, Subtype::PbrNormal);
        e.run(p, &rep, &CancelToken::default()).unwrap()
    };
    let s = convert();
    assert_eq!((s.converted, s.up_to_date), (2, 0));
    std::fs::rename(&r, &aside).unwrap();
    let s = convert();
    assert_eq!(
        (s.converted, s.up_to_date),
        (0, 1),
        "bake off: the _n is unchanged"
    );
    std::fs::rename(&aside, &r).unwrap();
    let s = convert();
    assert_eq!(
        (s.converted, s.up_to_date),
        (0, 2),
        "bake on again: nothing changed"
    );
}

/// S-mips-7 (ruling F7): an NPOT pair, written at its power-of-two size (Note FM29), bakes from the resized,
/// renormalised `_n`. The expectation is built through the public API: Preprocess (which resizes), the `_n`'s written
/// mip 0, its length pyramid and the member's chain with that bake.
#[test]
fn a_resized_pair_bakes_from_the_resized_normal() {
    use engine::bake::{LengthPyramid, NormalVariance};
    use engine::mips::build_chain_with;
    use engine::ops::preprocess;
    let d = tempfile::tempdir().unwrap();
    let n = bumpy_n(d.path(), 48, 0.0);
    let r = rmaos(d.path(), 48);
    let p = plan(d.path(), &r, &n, true, false);
    let job = &p.jobs[0];
    let (member_p, normal_p) = (
        job.profile.clone(),
        job.bake.as_ref().unwrap().normal_profile.clone(),
    );
    let (s, ev) = run(p);
    assert_eq!(s.converted, 1, "{ev:#?}");

    let load = |p: &Path| engine::load::load_image(p).unwrap().0;
    let n_work = preprocess(load(&n), &normal_p);
    assert_eq!((n_work.img.width, n_work.img.height), (32, 32));
    let base = build_chain_with(&n_work, &normal_p, None).swap_remove(0);
    let nv = NormalVariance::new(
        LengthPyramid::build(&base),
        (32, 32),
        member_p.mips.bake_clamp,
    )
    .unwrap();
    let m_work = preprocess(load(&r), &member_p);
    let chain = build_chain_with(&m_work, &member_p, Some(&nv));
    let want =
        chain[1].px.iter().map(|p| f64::from(p[0])).sum::<f64>() / chain[1].px.len() as f64 * 255.0;
    let got = mean_r(d.path(), 1);
    assert!(
        (got - want).abs() < 1.0,
        "level 1 mean R: {got} vs reference {want}"
    );
    assert!(got > 102.0, "the bake adds roughness: {got}");
}

/// Pin (Note GW3): a bake with no RNM layer hashes as it did before `rnm` left the tuple, so no such bake re-exports.
/// `bake/mod.rs::a_bake_identity_follows_fit` pins the same hash.
#[test]
fn a_no_layer_bake_keeps_its_identity() {
    let d = tempfile::tempdir().unwrap();
    let n = bumpy_n(d.path(), 32, 0.0);
    let r = rmaos(d.path(), 32);
    let p = plan(d.path(), &r, &n, true, false);
    let id = p.jobs[0].bake.as_ref().unwrap().identity_sha256();
    assert_eq!(
        id,
        "4601ca0e9449fc8e6ae19ec9f0eb557b7c2532c4d9488821f460039552018b7b"
    );
}

/// Note GW3: a manifest written by 2.x may carry an `rnm` role on a source. It loads, the field is ignored, and the
/// job is up to date instead of re-exporting.
#[test]
fn an_old_manifest_with_rnm_loads() {
    let d = tempfile::tempdir().unwrap();
    let n = bumpy_n(d.path(), 32, 0.0);
    let r = rmaos(d.path(), 32);
    assert_eq!(run(plan(d.path(), &r, &n, true, false)).0.converted, 1);
    let path = Manifest::at(&d.path().join("out")).path().to_path_buf();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"bake\":"), "{text}");
    std::fs::write(
        &path,
        text.replace("\"bake\":", "\"rnm\":\"detail\",\"bake\":"),
    )
    .unwrap();
    let (s, ev) = run(plan(d.path(), &r, &n, true, false));
    assert_eq!((s.converted, s.up_to_date), (0, 1), "{ev:#?}");
}

/// N-11 (Note JS3): a set that starts baking from a plain `normal` or a `landscape_normal` re-encodes its `_rmaos`
/// once (its sources gain the bake) and leaves the `_n` up to date (N-13 for `landscape_normal`).
#[test]
fn a_plain_normal_that_starts_baking_re_encodes_only_its_roughness() {
    for (folder, subtype) in [("pbr", "normal"), ("landscape", "landscape_normal")] {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("in/textures").join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::rename(bumpy_n(d.path(), 32, 0.0), dir.join("iron_n.png")).unwrap();
        std::fs::rename(rmaos(d.path(), 32), dir.join("iron_rmaos.png")).unwrap();
        let toml = |opt_out: bool| {
            let mut t =
                format!("[files.\"textures/{folder}/iron_n.png\"]\nsubtype = \"{subtype}\"\n");
            if opt_out {
                // DECISIONS R6: an explicit `renormalize` opts out, as every plain normal behaved before.
                t.push_str("mips.normal = \"renormalize\"\n");
            }
            std::fs::write(d.path().join("in/sdc.toml"), t).unwrap();
        };
        let convert = || {
            let mut req =
                BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
            req.recursive = true;
            req.accept = AcceptSuggestions::High;
            req.on_existing = OnExisting::IfChanged;
            let rep = Reporter::new(Arc::new(VecSink::default()), "starts-baking");
            let e = Engine::new().unwrap();
            let p = e.plan(req, &rep, &CancelToken::default()).unwrap();
            e.run(p, &rep, &CancelToken::default()).unwrap()
        };
        toml(true);
        let s = convert();
        assert_eq!((s.converted, s.up_to_date), (2, 0), "{folder}");
        toml(false);
        let s = convert();
        assert_eq!(
            (s.converted, s.up_to_date),
            (1, 1),
            "{folder}: the _rmaos re-encodes once, the _n is unchanged"
        );
        let s = convert();
        assert_eq!((s.converted, s.up_to_date), (0, 2), "{folder}");
    }
}
