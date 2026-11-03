//! Per-sub-type encode versions (DECISIONS Note DM): the guard that fails when a sub-type's encode input changes
//! without its `encode_version` arm being bumped.
use engine::analyse::code8;
use engine::api::*;
use engine::config::{Subtype, Target};
use engine::dds::write_atomic;
use engine::events::*;
use engine::image::Rgba32F;
use engine::manifest::{encode_version, ENGINE_VERSION, RELEASED_ENGINE_VERSION};
use engine::policy::AcceptSuggestions;
use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

/// Sub-types no forced fixture can encode, with the reason. Each one is also listed in Note DM.
const UNGUARDED: &[(Subtype, &str)] = &[];

/// Checks `observed` (output key -> object of pinned fields) against the pins in `file` and rewrites the file when
/// the pinned map changes, field by field. A new key or field is pinned; a field whose value differs is accepted
/// only when `accept(key, field)`, else the old value stays and `changed(key)` is returned once for that key; a
/// field in the file that this run did not observe (another CPU's `payload_sha256@<isa>`) is kept; keys no longer
/// observed are dropped.
fn pin_fields(
    file: &Path,
    observed: BTreeMap<String, Value>,
    accept: &dyn Fn(&str, &str) -> bool,
    changed: &dyn Fn(&str) -> String,
) -> Vec<String> {
    let old: BTreeMap<String, Value> = match std::fs::read_to_string(file) {
        Ok(s) => serde_json::from_str(&s).unwrap(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => panic!("{}: {e}", file.display()),
    };
    let mut pins = BTreeMap::new();
    let mut failures = vec![];
    for (key, now) in observed {
        let mut entry = old
            .get(&key)
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        let mut failed = false;
        for (field, v) in now.as_object().unwrap() {
            match entry.get(field) {
                Some(was) if was != v && !accept(&key, field) => failed = true,
                _ => {
                    entry.insert(field.clone(), v.clone());
                }
            }
        }
        if failed {
            failures.push(changed(&key));
        }
        pins.insert(key, Value::Object(entry));
    }
    // Compare the parsed map, not the bytes: the checkout is CRLF.
    if pins != old {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let text = serde_json::to_string_pretty(&pins).unwrap() + "\n";
        write_atomic(file, text.as_bytes()).unwrap();
    }
    failures
}

/// `pin_fields` for `{ subtype, sha256 }` pins (output key -> (sub-type, hash)): a changed hash is accepted only when
/// `bumped(subtype)`.
fn check_pins(
    file: &Path,
    observed: BTreeMap<String, (Subtype, String)>,
    what: &str,
    bumped: &dyn Fn(Subtype) -> bool,
) -> Vec<String> {
    let subtypes: BTreeMap<String, Subtype> = observed
        .iter()
        .map(|(k, (st, _))| (k.clone(), *st))
        .collect();
    let observed = observed
        .into_iter()
        .map(|(k, (st, sha))| (k, json!({ "subtype": st.id(), "sha256": sha })))
        .collect();
    pin_fields(
        file,
        observed,
        &|key, field| field != "sha256" || bumped(subtypes[key]),
        &|key| {
            format!(
                "{} output changed ({what} of {key}): set its encode_version arm in \
                 crates/engine/src/manifest.rs to ENGINE_VERSION (and bump the engine version if it is \
                 still the released version)",
                subtypes[key].id()
            )
        },
    )
}

/// `pin_fields` with no change accepted: the byte-identity pins (Note FM1).
fn check_strict(file: &Path, observed: BTreeMap<String, Value>) -> Vec<String> {
    pin_fields(file, observed, &|_, _| false, &|key| {
        format!(
            "{key}: output changed. If intended, delete the key from identity.json, re-run, and name the key \
             and the reason in the Note FM item."
        )
    })
}

/// The byte-identity pin of every output `observed` names (keys carry `prefix`), read back from the DDS that
/// `run_target` wrote under `d`: sub-type, format, size and the SHA-256 of the pixel data. BC7 and BC6H bytes depend
/// on the CPU's ISA path, so their hash is keyed `payload_sha256@<isa>`.
fn identity_entries(
    d: &Path,
    prefix: &str,
    observed: &BTreeMap<String, (Subtype, String)>,
) -> BTreeMap<String, Value> {
    use engine::dds::DdsPixelFormat::*;
    observed
        .iter()
        .map(|(key, (st, _))| {
            let path = out_dir(d, prefix).join(key.strip_prefix(prefix).unwrap());
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let dds = engine::dds::read_dds(&bytes).unwrap();
            let by_isa = matches!(dds.format, Bc7 { .. } | Bc6hUf16 | Bc6hSf16);
            let field = if by_isa {
                format!("payload_sha256@{}", engine::manifest::dispatched_isa())
            } else {
                "payload_sha256".to_string()
            };
            let mut pin = json!({
                "subtype": st.id(),
                "format": format!("{:?}", dds.format),
                "width": dds.width,
                "height": dds.height,
            });
            pin[field] = hex::encode(Sha256::digest(&dds.data)).into();
            (key.clone(), pin)
        })
        .collect()
}

/// The `cutout_bc3_t200` pin only means something if the cutoff changes bytes: its mip 0 alpha blocks equal the BC3
/// encode with cutoff 201 (`gt` 200), and at least one of them differs from the default-cutoff (128) encode.
fn t200_pins_the_cutoff(d: &Path) {
    let bytes = std::fs::read(out_dir(d, "cov/").join("textures/c/cutout_bc3_t200.dds")).unwrap();
    let dds = engine::dds::read_dds(&bytes).unwrap();
    let px: Vec<[u8; 4]> = rgba_near_cut().to_rgba8().pixels().map(|p| p.0).collect();
    let mut differs = 0;
    for (b, blk) in encoders_sys::gather_blocks(&px, 64, 64)
        .chunks(16)
        .enumerate()
    {
        let blk: &[[u8; 4]; 16] = blk.try_into().unwrap();
        let alpha = |cut| encoders_sys::bc3_encode_block(blk, 5, cut)[..8].to_vec();
        assert_eq!(dds.data[b * 16..b * 16 + 8], alpha(201), "block {b}");
        differs += (alpha(201) != alpha(128)) as u32;
    }
    assert!(
        differs > 0,
        "cutout_bc3_t200 is the same under the default cutoff"
    );
}

fn out_dir(d: &Path, prefix: &str) -> std::path::PathBuf {
    d.join(format!("out-{}", prefix.trim_end_matches('/')))
}

fn bumped(st: Subtype) -> bool {
    encode_version(st) == ENGINE_VERSION && ENGINE_VERSION != RELEASED_ENGINE_VERSION
}

fn put(dir: &Path, rel: &str, img: DynamicImage) {
    let p = dir.join("in").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    img.save(p).unwrap();
}

/// 64×64 RGBA gradient with varying alpha.
/// Alpha hashed over 0..=255, so blocks hold texels on both sides of a `gt` 200 test and the BC3 alpha guard has
/// something to protect (a smooth ramp or a narrow band gives the same bytes under any cutoff).
fn rgba_near_cut() -> DynamicImage {
    DynamicImage::ImageRgba8(ImageBuffer::from_fn(64, 64, |x, y| {
        let h =
            (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)).wrapping_mul(2_654_435_761);
        Rgba([(x * 4) as u8, (y * 4) as u8, 90, (h >> 24) as u8])
    }))
}

fn rgba64() -> DynamicImage {
    DynamicImage::ImageRgba8(ImageBuffer::from_fn(64, 64, |x, y| {
        Rgba([(x * 4) as u8, (y * 4) as u8, 90, ((x + y) * 2) as u8])
    }))
}

/// Float source for the sub-types that need one.
fn hdr64() -> DynamicImage {
    DynamicImage::ImageRgb32F(ImageBuffer::from_fn(64, 64, |x, y| {
        Rgb([4.0 * x as f32 / 63.0, 2.0 * y as f32 / 63.0, 0.25])
    }))
}

/// A 64×128 complex-grass source as complex_grass.rs builds one: top = colour with a cutout alpha, bottom = unit
/// normals with specular in alpha, so VAL-GRASS-DETECT passes.
fn grass64() -> DynamicImage {
    let w = 64;
    let mut img = Rgba32F::new(w, 2 * w, [0.0; 4]);
    for y in 0..2 * w {
        for x in 0..w {
            *img.at_mut(x, y) = if y < w {
                let a = if (x * 7 + y * 13) % 10 < 3 { 1.0 } else { 0.0 };
                [0.25, 0.5, 0.15, a]
            } else {
                let t = (x as f32 / w as f32 - 0.5) * 0.8;
                [t.sin() * 0.5 + 0.5, 0.5, t.cos() * 0.5 + 0.5, 12.0 / 255.0]
            };
        }
    }
    DynamicImage::ImageRgba8(ImageBuffer::from_fn(img.width, img.height, |x, y| {
        Rgba(img.at(x, y).map(code8))
    }))
}

fn rgb(w: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(w, w, |x, y| {
        Rgb([(x * 4) as u8, (y * 4) as u8, 90])
    }))
}

fn flat_normal(w: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(w, w, |_, _| Rgb([128, 128, 255])))
}

/// golden.rs `pbr_fixture`: a cs PBR set with its `_rmaos` and the roughness variance bake.
fn pbr_fixture() -> Vec<(&'static str, DynamicImage)> {
    vec![
        ("textures/pbr/rock.png", rgb(16)),
        ("textures/pbr/rock_n.png", flat_normal(16)),
        ("textures/pbr/rock_rmaos.png", rgb(16)),
    ]
}

/// rnm_bake.rs: an RNM base `_n` with one detail layer, plus the `_rmaos` its bake reads.
fn rnm_fixture(dir: &Path) {
    let p = |n: &str| format!("textures/rnm/{n}");
    put(
        dir,
        &p("iron.png"),
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(32, 32, |x, y| {
            Rgb([(x * 8) as u8, (y * 8) as u8, 255])
        })),
    );
    put(
        dir,
        &p("iron_n.png"),
        DynamicImage::ImageRgba8(ImageBuffer::from_fn(32, 32, |_, _| {
            Rgba([128, 128, 255, 255])
        })),
    );
    put(
        dir,
        &p("iron_rmaos.png"),
        DynamicImage::ImageRgba8(ImageBuffer::from_fn(32, 32, |_, _| {
            Rgba([128, 0, 255, 255])
        })),
    );
    put(
        dir,
        &p("iron_detailnorm.png"),
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(32, 32, |x, y| {
            Rgb([
                (128.0 + 100.0 * (x as f32 * 1.7).sin()) as u8,
                (128.0 + 100.0 * (y as f32 * 1.3).cos()) as u8,
                200,
            ])
        })),
    );
}

/// Byte-identity coverage the per-sub-type fixtures lack (identity.json only; encode_inputs.json does not pin it):
/// opaque BC1, a BC1 1-bit alpha cutout, BC3 and BC7 colour with a cutout alpha (box mips) and with a blend alpha, a
/// BC3 cutout at alpha.threshold 200, a BC3 cutout with the alpha-weighted tent, a two-size `_rmaos` pack, plus BC4 and BC5. Returns the
/// `[files]` overrides.
fn coverage_fixture(dir: &Path) -> Vec<String> {
    let mut sets = vec![];
    let mut add = |name: &str, img: DynamicImage, fields: &[&str]| {
        put(dir, &format!("textures/c/{name}.png"), img);
        for f in fields {
            sets.push(format!("file:**/c/{name}.png.{f}"));
        }
    };
    // 0/255 checker alpha for the cutout policy; `rgba64` is the smooth ramp for blend.
    let checker = || {
        DynamicImage::ImageRgba8(ImageBuffer::from_fn(64, 64, |x, y| {
            let a = if (x / 4 + y / 4) % 2 == 0 { 255 } else { 0 };
            Rgba([(x * 4) as u8, (y * 4) as u8, 90, a])
        }))
    };
    // BC1 stores no alpha (FMT-BC1-ALPHA), so it gets an opaque source; with `encoder.bc1_alpha` it keeps 1-bit alpha.
    add("opaque_bc1", rgb(64), &["subtype=diffuse", "format=bc1"]);
    add(
        "cutout_bc1",
        checker(),
        &[
            "subtype=diffuse",
            "format=bc1",
            "alpha.policy=cutout",
            "encoder.bc1_alpha=true",
            "mips.filter=box",
        ],
    );
    for f in ["bc3", "bc7"] {
        let fmt = format!("format={f}");
        add(
            &format!("cutout_{f}"),
            checker(),
            &[
                "subtype=diffuse",
                &fmt,
                "alpha.policy=cutout",
                "mips.filter=box",
            ],
        );
        add(
            &format!("blend_{f}"),
            rgba64(),
            &["subtype=diffuse", &fmt, "alpha.policy=blend"],
        );
    }
    // An alpha cutout scattered around threshold 200: the BC3 alpha guard keeps texels across 201, not 128 (Note FM).
    add(
        "cutout_bc3_t200",
        rgba_near_cut(),
        &[
            "subtype=diffuse",
            "format=bc3",
            "alpha.policy=cutout",
            "alpha.threshold=200",
            "mips.filter=box",
        ],
    );
    // The alpha-weighted tent (Note FM28): the checker cutout with `mips.filter = tent`.
    add(
        "cutout_bc3_tent",
        checker(),
        &[
            "subtype=diffuse",
            "format=bc3",
            "alpha.policy=cutout",
            "mips.filter=tent",
        ],
    );
    // `_SRGB`-tagged BC1 and BC3 (pbr_albedo is the cs sub-type that carries the tag), an opaque and a blend source.
    add("srgb_bc1", rgb(64), &["subtype=pbr_albedo", "format=bc1"]);
    add(
        "srgb_bc3",
        rgba64(),
        &["subtype=pbr_albedo", "format=bc3", "alpha.policy=blend"],
    );
    // A non-square gradient: BC1 and BC3 with a 2:1 mip chain.
    let wide = DynamicImage::ImageRgb8(ImageBuffer::from_fn(32, 16, |x, y| {
        Rgb([(x * 8) as u8, (y * 14) as u8, ((x * y) % 256) as u8])
    }));
    add(
        "wide_bc1",
        wide.clone(),
        &["subtype=diffuse", "format=bc1", "mips.filter=box"],
    );
    add(
        "wide_bc3",
        wide,
        &["subtype=diffuse", "format=bc3", "mips.filter=box"],
    );
    add("gray_bc4", rgb(64), &["subtype=height", "format=bc4"]);
    // A normal cannot be BC5 (FMT-BC5-NORMAL), so BC5 is a two-channel map.
    add(
        "rg_bc5",
        rgb(64),
        &["subtype=generic_two_channel", "format=bc5"],
    );
    // Note FM29: an NPOT source written at its power-of-two size (1000 enlarged to 1024), and a 512 height map that
    // `resize = "half"` takes to its 256 floor.
    add(
        "npot_1000_bc1",
        rgb(1000),
        &["subtype=diffuse", "format=bc1", "mips.filter=box"],
    );
    add("height_half", rgb(512), &["subtype=height", "resize=half"]);
    // Note FM30: a pack of two sizes. The 32x32 AO is enlarged to the roughness map's 64x64 (the `_rmaos` is BC7).
    let ramp = |w: u32| {
        DynamicImage::ImageRgb8(ImageBuffer::from_fn(w, w, |x, y| {
            let v = ((x + y) * 255 / (2 * (w - 1))) as u8;
            Rgb([v, v, v])
        }))
    };
    add("twosize_roughness", ramp(64), &[]);
    add("twosize_ao", ramp(32), &[]);
    sets
}

/// Sub-types vanilla refuses (CFG-UNAVAILABLE: cs-only, Note DM2), so the vanilla run cannot pin them; the cs run
/// does.
const CS_ONLY: &[Subtype] = &[
    Subtype::PbrAlbedo,
    Subtype::PbrEmissive,
    Subtype::PbrSubsurface,
    Subtype::PbrFuzz,
    Subtype::PbrNormal,
    Subtype::CoatNormal,
    Subtype::Rmaos,
    Subtype::SkinRfaos,
    Subtype::Flow,
    Subtype::HdrSky,
    Subtype::SkinAlbedo,
    Subtype::SkinSk,
    Subtype::SkinWet,
];

/// Runs the fixture in `d/in` under `target` into `d/out-<prefix>` and returns the summary, the skip and fail
/// events, and the encode-input hashes keyed `<prefix><output key>`.
fn run_target(
    d: &Path,
    sets: &[String],
    target: Target,
    prefix: &str,
) -> (
    BatchSummary,
    Vec<EngineEvent>,
    BTreeMap<String, (Subtype, String)>,
) {
    let hooks = tempfile::tempdir().unwrap();
    let mut req = BatchRequest::new(vec![d.join("in")], out_dir(d, prefix), target);
    req.recursive = true;
    req.accept = AcceptSuggestions::High;
    req.on_existing = OnExisting::Overwrite;
    req.overrides = sets
        .iter()
        .map(|s| engine::overrides::parse_set(s).unwrap())
        .collect();
    // The only test in this binary that encodes, so setting the process-wide variable is safe.
    std::env::set_var("SDC_TEST_ENCODE_INPUT_DIR", hooks.path());
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "encode-versions");
    let e = Engine::new().unwrap();
    let plan = e.plan(req, &rep, &CancelToken::default()).unwrap();
    let summary = e.run(plan, &rep, &CancelToken::default()).unwrap();
    std::env::remove_var("SDC_TEST_ENCODE_INPUT_DIR");
    let events: Vec<_> = sink
        .take()
        .into_iter()
        .filter(|e| {
            matches!(
                e.kind,
                EventKind::FileFailed { .. } | EventKind::FileSkipped { .. }
            )
        })
        .collect();
    let mut observed = BTreeMap::new();
    for f in std::fs::read_dir(hooks.path()).unwrap() {
        let f = f.unwrap().path();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let st: Subtype = serde_json::from_value(v["subtype"].clone()).unwrap();
        let name = f.file_name().unwrap().to_str().unwrap();
        let key = name.strip_suffix(".json").unwrap().replace('~', "/");
        observed.insert(
            format!("{prefix}{key}"),
            (st, v["sha256"].as_str().unwrap().to_string()),
        );
    }
    (summary, events, observed)
}

#[test]
fn encode_inputs_are_pinned_or_their_type_was_bumped() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = vec![];
    for st in Subtype::ALL {
        use Subtype::*;
        let id = st.id();
        let (file, img) = match st {
            HdrSky => (format!("{id}.hdr"), hdr64()),
            ComplexGrass => (format!("{id}.png"), grass64()),
            // These store no alpha: an alpha source is blocked by LOSS-ALPHA-DATA.
            PbrEmissive | Glow | SubsurfaceTint | BacklightTint | Msn | Envmask | Flow | Height
            | MsnSpecular | GenericTwoChannel | SkinSk | ProjectedNoise => {
                (format!("{id}.png"), rgb(64))
            }
            // DECISIONS DS8: a one-file cube needs a layout, so a 6:1 h-strip of 64-px faces.
            Cubemap => (
                format!("{id}.png"),
                DynamicImage::ImageRgb8(ImageBuffer::from_fn(384, 64, |x, y| {
                    Rgb([(x * 4) as u8, (y * 4) as u8, 90])
                })),
            ),
            _ => (format!("{id}.png"), rgba64()),
        };
        put(d.path(), &format!("textures/h/{file}"), img);
        sets.push(format!("file:**/h/{file}.subtype={id}"));
        // The mesh decides these alphas, so a forced sub-type still needs the meaning given.
        if st.alpha_from_mesh() && st != ComplexGrass {
            sets.push(format!("file:**/h/{file}.alpha.policy=blend"));
        }
    }
    for (rel, img) in pbr_fixture() {
        put(d.path(), rel, img);
    }
    rnm_fixture(d.path());
    let guarded = || {
        Subtype::ALL
            .into_iter()
            .filter(|st| UNGUARDED.iter().all(|(u, _)| u != st))
    };
    let seen = |o: &BTreeMap<String, (Subtype, String)>| -> BTreeSet<Subtype> {
        o.values().map(|(st, _)| *st).collect()
    };

    let (summary, events, mut observed) = run_target(d.path(), &sets, Target::Cs, "");
    // Every fixture encodes and passes validation, so each pin covers the passing path (complex_grass included).
    assert_eq!(
        (summary.failed, summary.skipped, summary.validation_failed),
        (0, 0, 0),
        "{events:#?}"
    );
    assert_eq!(
        seen(&observed),
        guarded().collect(),
        "sub-types with a cs encode-input pin"
    );
    let mut identity = identity_entries(d.path(), "", &observed);

    // Vanilla has its own pixel paths (packing arms, no PBR), so it is pinned too, under `vanilla/`.
    let (summary, events, vanilla) = run_target(d.path(), &sets, Target::Vanilla, "vanilla/");
    assert_eq!(
        (summary.failed, summary.validation_failed),
        (0, 0),
        "{events:#?}"
    );
    // Only the cs-only sub-types are refused: their forced fixtures are skipped with CFG-UNAVAILABLE. Uncoded skips
    // are intentional (vanilla skips the RNM detail layer as a normal layer).
    let refused: BTreeSet<String> = events
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::FileSkipped { code: None, .. } => None,
            EventKind::FileSkipped { code: Some(c), .. } if c.as_str() == "CFG-UNAVAILABLE" => {
                // A forced fixture by its sub-type id, any other file by its path under `in/`.
                let p = e.path.clone().unwrap_or_default().replace('\\', "/");
                let rel = p.rsplit_once("/in/").map_or(p.as_str(), |(_, r)| r);
                Some(
                    rel.strip_prefix("textures/h/")
                        .map_or(rel.to_string(), |f| {
                            f.split('.').next().unwrap().to_string()
                        }),
                )
            }
            _ => panic!("unexpected vanilla event: {e:#?}"),
        })
        .collect();
    // The two `_rmaos` fixture members are cs-only rmaos as well.
    let mut want_refused: BTreeSet<String> = CS_ONLY.iter().map(|st| st.id().to_string()).collect();
    want_refused.extend([
        "textures/pbr/rock_rmaos.png".into(),
        "textures/rnm/iron_rmaos.png".into(),
    ]);
    assert_eq!(refused, want_refused, "vanilla refusals: {refused:#?}");
    assert_eq!(
        seen(&vanilla),
        guarded().filter(|st| !CS_ONLY.contains(st)).collect(),
        "sub-types with a vanilla encode-input pin"
    );
    identity.extend(identity_entries(d.path(), "vanilla/", &vanilla));
    observed.extend(vanilla);

    // Byte-identity coverage (Note FM1): formats and alpha policies the fixtures above may not reach.
    let c = tempfile::tempdir().unwrap();
    let cov_sets = coverage_fixture(c.path());
    let (summary, events, cov) = run_target(c.path(), &cov_sets, Target::Cs, "cov/");
    assert_eq!(
        (summary.failed, summary.skipped, summary.validation_failed),
        (0, 0, 0),
        "{events:#?}"
    );
    identity.extend(identity_entries(c.path(), "cov/", &cov));
    let dims = |name: &str| {
        let p = &identity[&format!("cov/textures/c/{name}.dds")];
        (p["width"].as_u64(), p["height"].as_u64())
    };
    assert_eq!(dims("npot_1000_bc1"), (Some(1024), Some(1024)));
    assert_eq!(dims("height_half"), (Some(256), Some(256)));
    assert_eq!(dims("twosize_rmaos"), (Some(64), Some(64)));
    t200_pins_the_cutoff(c.path());
    // The tent pin only means something if the weighted tent changes bytes against the box cutout.
    let sha =
        |name: &str| identity[&format!("cov/textures/c/{name}.dds")]["payload_sha256"].clone();
    assert_ne!(sha("cutout_bc3_tent"), sha("cutout_bc3"));
    let formats: BTreeSet<&str> = identity
        .values()
        .map(|p| p["format"].as_str().unwrap())
        .collect();
    for want in ["Bc1", "Bc3", "Bc4", "Bc5"] {
        assert!(
            formats.iter().any(|f| f.starts_with(want)),
            "identity pins cover no {want} output: {formats:?}"
        );
    }
    let covered = |name: &str| identity.keys().any(|k| k.contains(name));
    for name in [
        "opaque_bc1",
        "cutout_bc1",
        "cutout_bc3",
        "cutout_bc7",
        "blend_bc3",
        "blend_bc7",
        "srgb_bc1",
        "srgb_bc3",
        "wide_bc1",
        "wide_bc3",
        "npot_1000_bc1",
        "height_half",
        "twosize_rmaos",
    ] {
        assert!(covered(name), "identity pins miss the {name} fixture");
    }

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/encode_versions");
    let failures = check_pins(
        &dir.join("encode_inputs.json"),
        observed,
        "encode input",
        &bumped,
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let failures = check_strict(&dir.join("identity.json"), identity);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn check_strict_fails_any_change_and_adds_new_keys() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("identity.json");
    let read = || std::fs::read_to_string(&f).unwrap();
    let one = |h: &str| BTreeMap::from([("a.dds".to_string(), json!({ "payload_sha256": h }))]);
    assert!(check_strict(&f, one("x")).is_empty()); // new key pinned
    assert!(check_strict(&f, one("x")).is_empty()); // unchanged
    let fails = check_strict(&f, one("y"));
    assert_eq!(fails.len(), 1);
    assert!(fails[0].contains("a.dds") && fails[0].contains("Note FM"));
    assert!(
        read().contains("\"x\"") && !read().contains("\"y\""),
        "old pin kept"
    );

    // Another CPU's ISA field survives a run that only observes this machine's, and a new field is added.
    std::fs::write(
        &f,
        r#"{"b.dds": {"payload_sha256@avx512": "z", "payload_sha256@avx2": "p"}}"#,
    )
    .unwrap();
    let here = BTreeMap::from([(
        "b.dds".to_string(),
        json!({ "payload_sha256@avx2": "p", "w": 4 }),
    )]);
    assert!(check_strict(&f, here).is_empty());
    let pins: Value = serde_json::from_str(&read()).unwrap();
    assert_eq!(
        pins["b.dds"],
        json!({ "payload_sha256@avx512": "z", "payload_sha256@avx2": "p", "w": 4 })
    );
    // This machine's own ISA field is still strict.
    let changed = BTreeMap::from([("b.dds".to_string(), json!({ "payload_sha256@avx2": "q" }))]);
    assert_eq!(check_strict(&f, changed).len(), 1);
    // A key no run observed is dropped.
    assert!(check_strict(&f, one("x")).is_empty());
    assert!(!read().contains("b.dds"));
}

#[test]
fn check_pins_fails_an_unbumped_change_and_accepts_a_bumped_one() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("pins.json");
    let read =
        || -> Value { serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap() };
    let normal =
        |h: &str| BTreeMap::from([("h/normal.dds".to_string(), (Subtype::Normal, h.to_string()))]);
    std::fs::write(
        &file,
        r#"{"h/normal.dds": {"subtype":"normal","sha256":"aa"}}"#,
    )
    .unwrap();

    let f = check_pins(&file, normal("bb"), "encode input", &|_| false);
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].contains("normal output changed"), "{f:?}");
    assert_eq!(read()["h/normal.dds"]["sha256"], "aa");

    let f = check_pins(&file, normal("bb"), "encode input", &|_| true);
    assert!(f.is_empty(), "{f:?}");
    assert_eq!(read()["h/normal.dds"]["sha256"], "bb");

    let mut both = normal("bb");
    both.insert("h/msn.dds".into(), (Subtype::Msn, "cc".into()));
    let f = check_pins(&file, both, "encode input", &|_| false);
    assert!(f.is_empty(), "{f:?}");
    assert_eq!(
        read()["h/msn.dds"],
        json!({"subtype": "msn", "sha256": "cc"})
    );

    let f = check_pins(&file, normal("bb"), "encode input", &|_| false);
    assert!(f.is_empty(), "{f:?}");
    assert!(read().get("h/msn.dds").is_none(), "unobserved key dropped");
}

/// Adding a `Profile` field must not change any built-in preset's `profile_sha256`, or `--on-existing if-changed`
/// re-encodes everything. Optional fields need `skip_serializing_if` so they vanish from the hash while unset.
#[test]
fn builtin_profile_hashes_are_pinned() {
    let reg = engine::presets::PresetRegistry::builtin().unwrap();
    let mut ids: Vec<String> = reg.selectable().iter().map(|p| p.id.clone()).collect();
    ids.push("sdc-base".into());
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 6, "a new built-in preset: check it is pinned");
    let mut observed = BTreeMap::new();
    for id in &ids {
        for (target, tid) in [(Target::Cs, "cs"), (Target::Vanilla, "vanilla")] {
            for st in Subtype::ALL {
                if let Ok(p) = reg.profile(id, target, st) {
                    let sha = engine::manifest::profile_sha256(&p).unwrap();
                    observed.insert(format!("{id}/{tid}/{}", st.id()), (st, sha));
                }
            }
        }
    }
    assert!(observed.keys().any(|k| k.starts_with("cs-quality/cs/")));
    // Every id x target x sub-type must resolve, so a combination that newly becomes Err shows here.
    assert_eq!(observed.len(), ids.len() * 2 * Subtype::ALL.len());
    let file =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/encode_versions/profile_hashes.json");
    let failures = check_pins(&file, observed, "profile_sha256", &bumped);
    assert!(
        failures.is_empty(),
        "{}\nif a new Profile field caused it, make the field `#[serde(default, skip_serializing_if = ...)]` instead",
        failures.join("\n")
    );
}
