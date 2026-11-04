//! `resize_relative_to` (Note FM44): `resize` measured from a texture-set slot's source size instead of the map's own.
use engine::api::*;
use engine::codes::Code;
use engine::config::{ResizeMode, ResizeRelativeTo, Subtype, Target};
use engine::events::*;
use engine::overrides::parse_set;
use engine::policy::{AcceptSuggestions, ResolvedJob};
use engine::presets::PresetRegistry;
use image::{ImageBuffer, Rgb, RgbImage};
use std::path::Path;
use std::sync::Arc;

fn flat(w: u32, h: u32, c: [u8; 3]) -> RgbImage {
    ImageBuffer::from_pixel(w, h, Rgb(c))
}

/// Writes each `(file, sub-type, w, h)` under `in/` and returns the `--set`s that type it: a flat normal for the
/// normals, a flat colour for the rest.
fn files(dir: &Path, list: &[(&str, &str, u32, u32)]) -> Vec<String> {
    let i = dir.join("in");
    std::fs::create_dir_all(&i).unwrap();
    let mut sets = vec![];
    for (name, st, w, h) in list {
        let c = if st.contains("normal") {
            [128, 128, 255]
        } else {
            [120, 90, 60]
        };
        flat(*w, *h, c).save(i.join(name)).unwrap();
        sets.push(format!("file:**/{name}.subtype={st}"));
    }
    sets
}

/// `half` relative to `slot` with a 32 px floor, on `name`.
fn relative(name: &str, slot: &str) -> Vec<String> {
    vec![
        format!("file:**/{name}.resize=half"),
        format!("file:**/{name}.resize_floor=32"),
        format!("file:**/{name}.resize_relative_to={slot}"),
    ]
}

fn request(dir: &Path, sets: &[String]) -> BatchRequest {
    let mut r = BatchRequest::new(vec![dir.join("in")], dir.join("out"), Target::Cs);
    r.recursive = true;
    r.accept = AcceptSuggestions::High;
    r.overrides = sets.iter().map(|s| parse_set(s).unwrap()).collect();
    r.overrides
        .push(parse_set("encoder.effort=ultrafast").unwrap());
    r
}

fn plan(dir: &Path, sets: &[String]) -> Plan {
    let rep = Reporter::new(Arc::new(VecSink::default()), new_batch_id());
    Engine::new()
        .unwrap()
        .plan(request(dir, sets), &rep, &CancelToken::default())
        .unwrap()
}

fn job<'a>(p: &'a Plan, name: &str) -> &'a ResolvedJob {
    p.jobs
        .iter()
        .find(|j| j.input.file_name().unwrap() == name)
        .unwrap_or_else(|| panic!("no job for {name}: {:#?}", p.jobs))
}

/// The size `name` (a `src`-sized source) is written at.
fn out(p: &Plan, name: &str, src: (u32, u32)) -> (u32, u32) {
    engine::resize::target_dims(src, &job(p, name).profile)
}

fn codes(p: &Plan, name: &str) -> Vec<Code> {
    job(p, name).warnings.iter().map(|w| w.code).collect()
}

/// The brief's first example, at a quarter of its scale: a 256 diffuse, a normal already exported at 128 stays 128
/// (its own `half` would make it 64). No DIM-RESIZED: nothing is resized.
#[test]
fn relative_half_keeps_an_already_halved_normal() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = files(
        d.path(),
        &[
            ("rock.png", "diffuse", 256, 256),
            ("rock_n.png", "normal", 128, 128),
        ],
    );
    sets.extend(relative("rock_n.png", "slot1"));
    let p = plan(d.path(), &sets);
    assert_eq!(
        job(&p, "rock_n.png").profile.resize_reference,
        Some((256, 256))
    );
    assert_eq!(out(&p, "rock_n.png", (128, 128)), (128, 128));
    assert!(!codes(&p, "rock_n.png").contains(&Code::DimResized));
    assert!(!codes(&p, "rock_n.png").contains(&Code::ResizeNoReference));
}

/// A full-size normal halves, and the written file is that size; its DIM-RESIZED names the planned size.
#[test]
fn relative_half_halves_a_full_size_normal() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = files(
        d.path(),
        &[
            ("rock.png", "diffuse", 256, 256),
            ("rock_n.png", "normal", 256, 256),
        ],
    );
    sets.extend(relative("rock_n.png", "slot1"));
    let p = plan(d.path(), &sets);
    assert_eq!(out(&p, "rock_n.png", (256, 256)), (128, 128));
    let resized: Vec<_> = job(&p, "rock_n.png")
        .warnings
        .iter()
        .filter(|w| w.code == Code::DimResized)
        .collect();
    assert_eq!(resized.len(), 1);
    assert_eq!(resized[0].data["to"], serde_json::json!([128, 128]));
    let rep = Reporter::new(Arc::new(VecSink::default()), new_batch_id());
    let s = Engine::new()
        .unwrap()
        .run(p, &rep, &CancelToken::default())
        .unwrap();
    assert_eq!((s.failed, s.validation_failed), (0, 0));
    let b = std::fs::read(d.path().join("out/rock_n.dds")).unwrap();
    let f = engine::dds::read_dds(&b).unwrap();
    assert_eq!((f.width, f.height), (128, 128));
}

#[test]
fn relative_never_enlarges() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = files(
        d.path(),
        &[
            ("rock.png", "diffuse", 256, 256),
            ("rock_n.png", "normal", 64, 64),
        ],
    );
    sets.extend(relative("rock_n.png", "slot1"));
    let p = plan(d.path(), &sets);
    assert_eq!(out(&p, "rock_n.png", (64, 64)), (64, 64));
}

/// A 2:1 normal next to a square diffuse halves both sides: the long edge meets the reference, the shape stays.
#[test]
fn relative_keeps_the_maps_own_aspect() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = files(
        d.path(),
        &[
            ("rock.png", "diffuse", 256, 256),
            ("rock_n.png", "normal", 256, 128),
        ],
    );
    sets.extend(relative("rock_n.png", "slot1"));
    let p = plan(d.path(), &sets);
    assert_eq!(out(&p, "rock_n.png", (256, 128)), (128, 64));
}

/// Slot 4 is the set's height map: half of its 128 is 64, which neither the normal's own size nor the diffuse gives.
#[test]
fn relative_to_slot4_uses_the_height_map() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = files(
        d.path(),
        &[
            ("rock.png", "diffuse", 256, 256),
            ("rock_n.png", "normal", 256, 256),
            ("rock_p.png", "height", 128, 128),
        ],
    );
    sets.extend(relative("rock_n.png", "slot4"));
    let p = plan(d.path(), &sets);
    assert_eq!(
        job(&p, "rock_n.png").profile.resize_reference,
        Some((128, 128))
    );
    assert_eq!(out(&p, "rock_n.png", (256, 256)), (64, 64));
}

/// No map in the slot, or two: the map keeps its own size, with RESIZE-NO-REFERENCE naming the slot.
#[test]
fn no_map_in_the_slot_keeps_own_size_with_info() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = files(
        d.path(),
        &[
            ("rock.png", "diffuse", 256, 256),
            ("rock_g.png", "glow", 256, 256),
            ("rock_sk.png", "subsurface_tint", 256, 256),
            ("rock_n.png", "normal", 256, 256),
            ("rock_height.png", "height", 256, 256),
        ],
    );
    sets.extend(relative("rock_height.png", "slot5"));
    sets.extend(relative("rock_n.png", "slot3"));
    let p = plan(d.path(), &sets);
    for (name, slot, maps) in [("rock_height.png", 5, 0), ("rock_n.png", 3, 2)] {
        assert_eq!(out(&p, name, (256, 256)), (256, 256), "{name}");
        let w = job(&p, name)
            .warnings
            .iter()
            .find(|w| w.code == Code::ResizeNoReference)
            .unwrap_or_else(|| panic!("{name}: {:?}", codes(&p, name)));
        assert_eq!(w.severity, engine::codes::Severity::Info);
        assert!(w.message.contains(&format!("slot {slot}")), "{}", w.message);
        assert_eq!(w.data["slot"], slot);
        let sets: Vec<_> = p
            .jobs
            .iter()
            .map(|j| (j.input.file_name().unwrap(), &j.set_key, j.profile.subtype))
            .collect();
        assert_eq!(w.data["maps"], maps, "{sets:?}");
        assert!(!codes(&p, name).contains(&Code::DimResized), "{name}");
    }
}

/// A normal relative to slot 2 is relative to itself: the self rule, with no notice.
#[test]
fn relative_to_its_own_slot_is_self() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = files(
        d.path(),
        &[
            ("rock.png", "diffuse", 512, 512),
            ("rock_n.png", "normal", 256, 256),
        ],
    );
    sets.extend(relative("rock_n.png", "slot2"));
    let p = plan(d.path(), &sets);
    let n = job(&p, "rock_n.png");
    assert_eq!(n.profile.resize_reference, None);
    assert_eq!(out(&p, "rock_n.png", (256, 256)), (128, 128));
    assert!(!codes(&p, "rock_n.png").contains(&Code::ResizeNoReference));
}

/// `resize = "none"` consults no reference: a map larger than the diffuse keeps its size, with no notice.
#[test]
fn resize_none_ignores_the_reference() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = files(
        d.path(),
        &[
            ("rock.png", "diffuse", 128, 128),
            ("rock_n.png", "normal", 256, 256),
        ],
    );
    sets.push("file:**/rock_n.png.resize_relative_to=slot1".into());
    let p = plan(d.path(), &sets);
    assert_eq!(job(&p, "rock_n.png").profile.resize_reference, None);
    assert_eq!(out(&p, "rock_n.png", (256, 256)), (256, 256));
    assert!(!codes(&p, "rock_n.png").contains(&Code::ResizeNoReference));
}

/// `"self"` is the default: it is never written into a profile, so profile hashes and identities are unchanged
/// (identity.json is the byte check, in encode_versions), and the planner leaves no reference.
#[test]
fn self_is_byte_identical_to_today() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = files(
        d.path(),
        &[
            ("rock.png", "diffuse", 256, 256),
            ("rock_n.png", "normal", 256, 256),
        ],
    );
    sets.push("file:**/rock_n.png.resize=half".into());
    sets.push("file:**/rock_n.png.resize_floor=32".into());
    let plain = plan(d.path(), &sets);
    sets.push("file:**/rock_n.png.resize_relative_to=self".into());
    let explicit = plan(d.path(), &sets);
    let (a, b) = (job(&plain, "rock_n.png"), job(&explicit, "rock_n.png"));
    assert_eq!(a.profile, b.profile);
    assert_eq!(a.profile.resize_relative_to, ResizeRelativeTo::Own);
    let v = serde_json::to_value(&a.profile).unwrap();
    assert!(v.get("resize_relative_to").is_none() && v.get("resize_reference").is_none());
    assert_eq!(out(&plain, "rock_n.png", (256, 256)), (128, 128));
}

/// Only the planner writes `resize_reference`: a config or `--set` naming it is refused.
#[test]
fn resize_reference_is_not_a_setting() {
    let d = tempfile::tempdir().unwrap();
    let i = d.path().join("in");
    let sets = files(d.path(), &[("rock.png", "diffuse", 64, 64)]);
    std::fs::write(
        i.join("sdc.toml"),
        "[files.\"rock.png\"]\nresize_reference = [8, 8]\n",
    )
    .unwrap();
    let mut r = request(d.path(), &sets);
    r.project_config = Some(i.join("sdc.toml"));
    let rep = Reporter::new(Arc::new(VecSink::default()), new_batch_id());
    let Err(e) = Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
    else {
        panic!("planned with resize_reference in [files]");
    };
    assert!(
        e.to_string().contains("unknown field 'resize_reference'"),
        "{e}"
    );
    assert!(parse_set("resize_reference=[8,8]").is_err());
}

/// A packed `_rmaos` is sized from its sources (their common size), both as the map that is resized and as the
/// reference, as `assemble` builds it.
#[test]
fn a_packed_map_is_sized_from_its_sources() {
    let d = tempfile::tempdir().unwrap();
    let mut sets = files(
        d.path(),
        &[
            ("rock.png", "pbr_albedo", 256, 256),
            ("rock_n.png", "pbr_normal", 256, 256),
        ],
    );
    let i = d.path().join("in");
    flat(128, 128, [100, 100, 100])
        .save(i.join("rock_roughness.png"))
        .unwrap();
    flat(64, 64, [200, 200, 200])
        .save(i.join("rock_ao.png"))
        .unwrap();
    sets.extend(relative("rock_n.png", "slot6"));
    sets.extend([
        "subtype:rmaos.resize=half".to_string(),
        "subtype:rmaos.resize_floor=32".to_string(),
        "subtype:rmaos.resize_relative_to=slot1".to_string(),
    ]);
    let p = plan(d.path(), &sets);
    let rmaos = p
        .jobs
        .iter()
        .find(|j| j.pack.is_some())
        .unwrap_or_else(|| panic!("no packed job: {:#?}", p.jobs));
    assert_eq!(rmaos.profile.subtype, Subtype::Rmaos);
    // The `_rmaos` (128, its roughness' size) is no larger than half the 256 albedo: kept.
    assert_eq!(rmaos.profile.resize_reference, Some((256, 256)));
    assert_eq!(
        engine::resize::target_dims((128, 128), &rmaos.profile),
        (128, 128)
    );
    // The normal halves from the `_rmaos`'s 128: 64.
    assert_eq!(
        job(&p, "rock_n.png").profile.resize_reference,
        Some((128, 128))
    );
    assert_eq!(out(&p, "rock_n.png", (256, 256)), (64, 64));
}

/// Owner (2026-11-03): in the compact presets the slot-1 colour types halve on their own size; every other type
/// in the table halves relative to slot 1.
#[test]
fn compact_presets_halve_slot1_flat_and_the_rest_relative() {
    let r = PresetRegistry::builtin().unwrap();
    for (id, target) in [
        ("cs-compact", Target::Cs),
        ("vanilla-compact", Target::Vanilla),
    ] {
        let mut relative = 0;
        for st in Subtype::ALL {
            let Ok(p) = r.profile(id, target, st) else {
                continue;
            };
            if p.resize == ResizeMode::None {
                assert_eq!(p.resize_relative_to, ResizeRelativeTo::Own, "{id} {st:?}");
                continue;
            }
            let want = if st.txst_slot() == Some(1) {
                ResizeRelativeTo::Own
            } else {
                relative += 1;
                ResizeRelativeTo::Slot1
            };
            assert_eq!(p.resize_relative_to, want, "{id} {st:?}");
        }
        assert!(relative > 20, "{id}: {relative}");
        for st in [
            Subtype::Diffuse,
            Subtype::LandscapeDiffuse,
            Subtype::PbrAlbedo,
            Subtype::SkinAlbedo,
        ] {
            let p = r.profile(id, target, st).unwrap();
            assert_eq!(
                (p.resize, p.resize_relative_to),
                (ResizeMode::Half, ResizeRelativeTo::Own)
            );
        }
    }
}

/// The slot each sub-type fills (owner's slot words, Note FM44).
#[test]
fn txst_slots_follow_the_owners_slot_words() {
    use Subtype::*;
    for (st, n) in [
        (Diffuse, Some(1)),
        (PbrAlbedo, Some(1)),
        (Normal, Some(2)),
        (Msn, Some(2)),
        (Glow, Some(3)),
        (SkinSk, Some(3)),
        (Height, Some(4)),
        (Cubemap, Some(5)),
        (Envmask, Some(6)),
        (ComplexMaterial, Some(6)),
        (Rmaos, Some(6)),
        (CoatNormal, Some(7)),
        (BacklightTint, Some(8)),
        (MsnSpecular, Some(8)),
        (PbrSubsurface, Some(8)),
        (Flow, None),
        (GenericData, None),
    ] {
        assert_eq!(st.txst_slot(), n, "{st:?}");
    }
}
