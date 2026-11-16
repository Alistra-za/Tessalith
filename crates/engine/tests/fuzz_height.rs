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
    let mut req = BatchRequest::new(
        vec![dir.join("in")],
        dir.join("out"),
        engine::config::Target::Cs,
    );
    req.recursive = true;
    req.accept = AcceptSuggestions::High;
    req.on_existing = OnExisting::Overwrite;
    req.set_shaders = [(SET.to_string(), shader)].into();
    if listed {
        req.fuzz_texture_sets = [SET.to_string()].into();
    }
    req.overrides = vec![parse_set(&format!("file:**/rock_p.png.format={format}")).unwrap()];
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

/// A layer cannot turn it on: only the plan's listing does.
#[test]
fn a_layer_value_is_cleared() {
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
    let p = Engine::new()
        .unwrap()
        .plan(req, &rep, &CancelToken::default())
        .unwrap();
    let j = p
        .jobs
        .iter()
        .find(|j| j.input.ends_with("rock_p.png"))
        .expect("the _p job");
    assert!(!j.profile.channels.keep_green);
}
