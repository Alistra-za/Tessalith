//! Note IE16 (owner, 2026-11-16): a `pbr` set whose texture-set block turns CS 2.0's fuzz roughness texture on
//! (`BatchRequest::fuzz_texture_sets`) keeps its `_p` source's G: R stays the height, G the fuzz roughness. Any other
//! `_p` is grey as before (R = G = B = the picked channel).
use engine::api::*;
use engine::codes::Code;
use engine::dds::read_dds;
use engine::decode::{decode_level, Decoded};
use engine::events::*;
use engine::overrides::parse_set;
use engine::policy::AcceptSuggestions;
use engine::shader::Shader;
use image::{ImageBuffer, Rgb};
use std::path::Path;
use std::sync::Arc;

const W: u32 = 64;
const SET: &str = "textures/a/rock";

fn put(dir: &Path, name: &str, f: impl Fn(u32, u32) -> [u8; 3]) {
    let p = dir.join("in/textures/a").join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(W, W, |x, y| Rgb(f(x, y)))
        .save(p)
        .unwrap();
}

/// The height (R): a ramp across x.
fn height(x: u32) -> u8 {
    (20 + x * 3) as u8
}

/// The fuzz roughness (G): a ramp down y, unrelated to the height.
fn fuzz(y: u32) -> u8 {
    (230 - y * 3) as u8
}

/// A CS PBR set with a two-channel `_p` (height in R, fuzz roughness in G).
fn fixture(dir: &Path) {
    put(dir, "rock.png", |x, y| {
        [(x * 2) as u8 + 60, (y * 2) as u8 + 40, 90]
    });
    put(dir, "rock_n.png", |_, _| [128, 128, 255]);
    put(dir, "rock_rmaos.png", |_, _| [128, 0, 255]);
    put(dir, "rock_p.png", |x, y| [height(x), fuzz(y), 0]);
}

struct Out {
    summary: BatchSummary,
    events: Vec<EngineEvent>,
}

/// Plan and run the fixture as a set picked `shader`, listed in `fuzz_texture_sets` when `listed`, its `_p` written
/// as `format`.
fn convert(dir: &Path, shader: Shader, listed: bool, format: &str) -> Out {
    convert_with(
        dir,
        shader,
        listed,
        &[&format!("file:**/rock_p.png.format={format}")],
        OnExisting::Overwrite,
    )
}

/// `convert` with the `--set` overrides `sets` and `on_existing`.
fn convert_with(dir: &Path, shader: Shader, listed: bool, sets: &[&str], on: OnExisting) -> Out {
    let mut req = BatchRequest::new(
        vec![dir.join("in")],
        dir.join("out"),
        engine::config::Target::Cs,
    );
    req.recursive = true;
    req.accept = AcceptSuggestions::High;
    req.on_existing = on;
    req.set_shaders = [(SET.to_string(), shader)].into();
    if listed {
        req.fuzz_texture_sets = [SET.to_string()].into();
    }
    req.overrides = sets.iter().map(|s| parse_set(s).unwrap()).collect();
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "fuzz-height");
    let e = Engine::new().unwrap();
    let plan = e.plan(req, &rep, &CancelToken::default()).unwrap();
    let summary = e.run(plan, &rep, &CancelToken::default()).unwrap();
    Out {
        summary,
        events: sink.take(),
    }
}

fn codes_of(o: &Out, suffix: &str) -> Vec<Code> {
    o.events
        .iter()
        .filter(|e| e.path.as_deref().is_some_and(|p| p.ends_with(suffix)))
        .filter_map(|e| match &e.kind {
            EventKind::Warning(w) => Some(w.code),
            _ => None,
        })
        .collect()
}

fn planned_p(o: &Out) -> serde_json::Value {
    o.events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::FilePlanned { job }
                if e.path.as_deref().is_some_and(|p| p.ends_with("rock_p.png")) =>
            {
                Some(serde_json::to_value(&**job).unwrap())
            }
            _ => None,
        })
        .expect("a file_planned for rock_p.png")
}

/// Mip 0 of `out/textures/a/rock_p.dds`, RGBA8.
fn mip0(dir: &Path) -> Vec<[u8; 4]> {
    let f = read_dds(&std::fs::read(dir.join("out/textures/a/rock_p.dds")).unwrap()).unwrap();
    let (w, h, r) = f.levels[0].clone();
    match decode_level(f.format, w, h, &f.data[r]).unwrap() {
        Decoded::Ldr(px) => px,
        Decoded::Hdr(_) => panic!("HDR output for a _p"),
    }
}

/// Every texel's (R, G) against the source's height and `g(x, y)`, within `tol` codes (BC1 has 5:6:5 endpoints and
/// one line through RGB for both ramps, so it gets more room than BC5's two independent channels).
fn assert_rg(px: &[[u8; 4]], g: impl Fn(u32, u32) -> u8, tol: i32, what: &str) {
    let near = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs() <= tol;
    for (i, p) in px.iter().enumerate() {
        let (x, y) = (i as u32 % W, i as u32 / W);
        assert!(
            near(p[0], height(x)) && near(p[1], g(x, y)),
            "{what}: {x},{y}: {p:?}, want R {} G {}",
            height(x),
            g(x, y)
        );
    }
}

#[test]
fn a_listed_pbr_set_keeps_the_p_sources_green_in_bc5_and_bc1() {
    for (format, tol) in [("bc5", 6), ("bc1", 16)] {
        let d = tempfile::tempdir().unwrap();
        fixture(d.path());
        let o = convert(d.path(), Shader::Pbr, true, format);
        assert_eq!(o.summary.converted, 4, "{format}: {:?}", o.summary);
        let j = planned_p(&o);
        assert_eq!(j["profile"]["subtype"], "height", "{format}");
        assert_eq!(j["profile"]["channels"]["keep_green"], true, "{format}");
        assert_rg(&mip0(d.path()), |_, y| fuzz(y), tol, format);
        let w = codes_of(&o, "rock_p.png");
        assert!(!w.contains(&Code::ChanNotGray), "{format}: {w:?}");
    }
}

/// The owner's opt-in: an unlisted set, and a listed set picked as anything but `pbr` (PBR Landscape gets no `.csm`),
/// write the grey `_p` they always did, CHAN-NOT-GRAY included, and their profile holds no `keep_green`.
#[test]
fn an_unlisted_or_non_pbr_set_keeps_the_grey_p() {
    for (shader, listed) in [(Shader::Pbr, false), (Shader::PbrLandscape, true)] {
        let d = tempfile::tempdir().unwrap();
        fixture(d.path());
        let o = convert(d.path(), shader, listed, "bc5");
        assert_eq!(o.summary.converted, 4, "{shader:?}: {:?}", o.summary);
        let j = planned_p(&o);
        assert!(
            j["profile"]["channels"].get("keep_green").is_none(),
            "{shader:?}: {}",
            j["profile"]["channels"]
        );
        assert_rg(&mip0(d.path()), |x, _| height(x), 6, &format!("{shader:?}"));
        let w = codes_of(&o, "rock_p.png");
        assert!(w.contains(&Code::ChanNotGray), "{shader:?}: {w:?}");
    }
}

/// Fix round 1 (item 1): under `if-changed`, turning the switch on re-encodes an existing BC5 `_p` with its G kept, a
/// second run with the switch still on skips it, and turning it off re-encodes the grey `_p`. Only the `_p` changes.
#[test]
fn if_changed_re_encodes_when_the_switch_turns() {
    let d = tempfile::tempdir().unwrap();
    fixture(d.path());
    let bc5 = ["file:**/rock_p.png.format=bc5"];
    let run = |listed| convert_with(d.path(), Shader::Pbr, listed, &bc5, OnExisting::IfChanged);
    let o = run(false);
    assert_eq!(o.summary.converted, 4, "{:?}", o.summary);
    assert_rg(&mip0(d.path()), |x, _| height(x), 6, "first run");
    let o = run(true);
    assert_eq!(
        o.summary.converted, 1,
        "switch on: the _p only: {:?}",
        o.summary
    );
    assert_rg(&mip0(d.path()), |_, y| fuzz(y), 6, "switch on");
    let o = run(true);
    assert_eq!(o.summary.converted, 0, "unchanged: {:?}", o.summary);
    let o = run(false);
    assert_eq!(
        o.summary.converted, 1,
        "switch off: the _p only: {:?}",
        o.summary
    );
    assert_rg(&mip0(d.path()), |x, _| height(x), 6, "switch off");
}

/// Fix round 1 (item 2): the height stays the source's R even when the `_p` names another grey pick, here G on a
/// source whose G varies far more than its R.
#[test]
fn the_height_stays_in_r_whatever_the_grey_pick() {
    let d = tempfile::tempdir().unwrap();
    fixture(d.path());
    let small = |x: u32| (100 + x / 4) as u8;
    put(d.path(), "rock_p.png", |x, y| [small(x), fuzz(y), 0]);
    let sets = [
        "file:**/rock_p.png.format=bc5",
        "file:**/rock_p.png.channels.grayscale_source=g",
    ];
    let o = convert_with(d.path(), Shader::Pbr, true, &sets, OnExisting::Overwrite);
    assert_eq!(o.summary.converted, 4, "{:?}", o.summary);
    let near = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs() <= 6;
    for (i, p) in mip0(d.path()).iter().enumerate() {
        let (x, y) = (i as u32 % W, i as u32 / W);
        assert!(
            near(p[0], small(x)) && near(p[1], fuzz(y)),
            "{x},{y}: {p:?}, want R {} G {}",
            small(x),
            fuzz(y)
        );
    }
}

/// Fix round 1 (item 3): a layer that writes `channels.keep_green` is refused by name, as the planner's other fields
/// are: only the plan's listing turns it on.
#[test]
fn a_layer_value_is_refused() {
    let d = tempfile::tempdir().unwrap();
    fixture(d.path());
    let cfg: engine::presets::ConfigFile =
        toml::from_str("schema_version = 1\n[subtypes.height.channels]\nkeep_green = true\n")
            .unwrap();
    let mut req = BatchRequest::new(
        vec![d.path().join("in")],
        d.path().join("out"),
        engine::config::Target::Cs,
    );
    req.recursive = true;
    req.accept = AcceptSuggestions::High;
    req.set_shaders = [(SET.to_string(), Shader::Pbr)].into();
    req.project_config_inline = Some(cfg);
    let rep = Reporter::new(Arc::new(VecSink::default()), "fuzz-height");
    let e = match Engine::new()
        .unwrap()
        .plan(req, &rep, &CancelToken::default())
    {
        Ok(_) => panic!("a layer's keep_green planned"),
        Err(e) => e.to_string(),
    };
    assert!(
        e.contains("[subtypes.height]")
            && e.contains("'channels.keep_green'")
            && e.contains("remove it"),
        "{e}"
    );
}
