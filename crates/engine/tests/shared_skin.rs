//! Note HG2: an explicit Advanced skin on a colourless set (`_n`, `_sk`, its joined `_wet`) marks it shared: the maps
//! other skin sets use. Its members are typed as skin members, no diffuse or `_rfaos` is looked for, and a hand edit
//! that gives it a colour map or another kind of member unmarks it.
use engine::api::*;
use engine::codes::Code;
use engine::config::{Subtype, Target};
use engine::events::*;
use engine::policy::AcceptSuggestions;
use engine::sets::SetEdit;
use engine::shader::Shader;
use image::{DynamicImage, ImageBuffer, Rgb};
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;

const X: &str = "textures/s/x";

fn put(dir: &Path, rel: &str) {
    let p = dir.join("in").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(16, 16, |x, y| {
        Rgb([128 + (x % 8) as u8, 128 + (y % 8) as u8, 250])
    }))
    .save(p)
    .unwrap();
}

/// `x_normal`, `x_sk` and `x_wet` under `textures/s`.
fn shared_files() -> TempDir {
    let d = tempfile::tempdir().unwrap();
    for f in ["x_normal", "x_sk", "x_wet"] {
        put(d.path(), &format!("textures/s/{f}.png"));
    }
    d
}

fn plan_with(
    d: &TempDir,
    target: Target,
    shaders: &[(&str, Shader)],
    edits: Vec<SetEdit>,
) -> (Plan, Vec<EngineEvent>) {
    let mut r = BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), target);
    r.recursive = true;
    r.accept = AcceptSuggestions::High;
    r.set_shaders = shaders.iter().map(|(k, s)| (k.to_string(), *s)).collect();
    r.set_edits = edits;
    let sink = Arc::new(VecSink::default());
    let rep = Reporter::new(sink.clone(), "t");
    let p = Engine::new()
        .unwrap()
        .plan(r, &rep, &CancelToken::default())
        .unwrap();
    (p, sink.take())
}

fn skin(d: &TempDir) -> (Plan, Vec<EngineEvent>) {
    plan_with(d, Target::Cs, &[(X, Shader::AdvancedSkin)], vec![])
}

fn job<'a>(p: &'a Plan, input: &str) -> &'a engine::policy::ResolvedJob {
    p.jobs
        .iter()
        .find(|j| j.input.file_name().is_some_and(|n| n == input))
        .unwrap_or_else(|| panic!("no job for {input}"))
}

fn sub(p: &Plan, input: &str) -> Subtype {
    job(p, input).profile.subtype
}

/// The `SetResolved` event of the set `textures/s/x`: its pick and warning codes.
fn resolved(events: &[EngineEvent]) -> (Option<Shader>, Vec<Code>) {
    events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::SetResolved {
                shader, warnings, ..
            } if e.set.as_deref().is_some_and(|s| s.ends_with(X)) => Some((
                shader.as_ref().map(|p| p.shader),
                warnings.iter().map(|w| w.code).collect(),
            )),
            _ => None,
        })
        .expect("a SetResolved for textures/s/x")
}

fn same_set(p: &Plan, a: &str, b: &str) -> bool {
    job(p, a).set_key == job(p, b).set_key
}

fn not_shared(codes: &[Code]) -> bool {
    codes.contains(&Code::ShaderMissingMap)
}

#[test]
fn an_explicit_skin_set_with_no_colour_is_shared() {
    let d = shared_files();
    let (p, ev) = skin(&d);
    assert_eq!(sub(&p, "x_normal.png"), Subtype::PbrNormal);
    assert_eq!(sub(&p, "x_sk.png"), Subtype::SkinSk);
    assert_eq!(sub(&p, "x_wet.png"), Subtype::SkinWet);
    assert!(same_set(&p, "x_normal.png", "x_wet.png"));
    assert!(job(&p, "x_wet.png").set_key.ends_with(X));
    let (pick, codes) = resolved(&ev);
    assert_eq!(pick, Some(Shader::AdvancedSkin));
    assert!(!codes.contains(&Code::ShaderMissingMap), "{codes:?}");
    assert!(!codes.contains(&Code::ShaderExtraMap), "{codes:?}");
    // No diffuse and no `_rfaos` anywhere in the outputs.
    let outs: Vec<String> = p
        .jobs
        .iter()
        .map(|j| j.output.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert!(!outs.iter().any(|n| n.contains("rfaos")), "{outs:?}");
    assert_eq!(outs.len(), 3, "{outs:?}");
}

#[test]
fn a_lone_sk_and_n_stay_as_they_were() {
    let d = shared_files();
    let (p, ev) = plan_with(&d, Target::Cs, &[], vec![]);
    assert_eq!(sub(&p, "x_normal.png"), Subtype::Normal);
    assert_eq!(sub(&p, "x_sk.png"), Subtype::BacklightTint);
    assert_eq!(
        resolved(&ev).0,
        Some(Shader::Skin),
        "the auto pick is still skin"
    );
    assert!(!same_set(&p, "x_normal.png", "x_wet.png"));
}

#[test]
fn an_explicit_other_shader_is_not_shared() {
    let d = shared_files();
    let (p, _) = plan_with(&d, Target::Cs, &[(X, Shader::Skin)], vec![]);
    assert!(!same_set(&p, "x_normal.png", "x_wet.png"));
    assert_ne!(sub(&p, "x_wet.png"), Subtype::SkinWet);
    assert_ne!(sub(&p, "x_normal.png"), Subtype::PbrNormal);
}

#[test]
fn a_set_holding_a_colour_map_is_not_shared() {
    let d = shared_files();
    put(d.path(), "textures/s/x.png");
    let (p, ev) = skin(&d);
    assert_eq!(sub(&p, "x.png"), Subtype::SkinAlbedo);
    assert!(not_shared(&resolved(&ev).1));
}

#[test]
fn a_set_holding_another_map_is_not_shared() {
    let d = shared_files();
    put(d.path(), "textures/s/x_rmaos.png");
    let (_, ev) = skin(&d);
    assert!(not_shared(&resolved(&ev).1));
}

#[test]
fn an_explicit_skin_set_joins_its_own_wet() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/s/e/x.png");
    put(d.path(), "textures/s/e/x_wet.png");
    let (p, _) = plan_with(
        &d,
        Target::Cs,
        &[("textures/s/e/x", Shader::AdvancedSkin)],
        vec![],
    );
    // Joined only: the set has no `_rfaos` source, so `set_is_advanced_skin` does not type the member (not this note).
    assert!(same_set(&p, "x.png", "x_wet.png"));
}

#[test]
fn nothing_is_shared_off_cs() {
    let d = shared_files();
    let (p, _) = plan_with(&d, Target::Vanilla, &[(X, Shader::AdvancedSkin)], vec![]);
    assert_eq!(sub(&p, "x_normal.png"), Subtype::Normal);
    assert!(!same_set(&p, "x_normal.png", "x_wet.png"));
}

#[test]
fn a_hand_add_that_empties_the_shared_set_loses_nothing() {
    let d = shared_files();
    let edits = vec![
        SetEdit::Create {
            set: "textures/s/y".into(),
        },
        SetEdit::Move {
            file: "textures/s/x_normal.png".into(),
            to: "textures/s/y".into(),
        },
        SetEdit::Move {
            file: "textures/s/x_sk.png".into(),
            to: "textures/s/y".into(),
        },
    ];
    let (p, ev) = plan_with(&d, Target::Cs, &[(X, Shader::AdvancedSkin)], edits);
    for f in ["x_normal.png", "x_sk.png", "x_wet.png"] {
        job(&p, f);
    }
    assert!(p.skipped.is_empty(), "{:?}", p.skipped);
    assert_eq!(sub(&p, "x_wet.png"), Subtype::SkinWet);
    assert!(job(&p, "x_wet.png").set_key.ends_with(X));
    assert!(!resolved(&ev).1.contains(&Code::ShaderMissingMap));
}

#[test]
fn a_hand_add_of_a_colour_map_unmarks_the_set() {
    let d = shared_files();
    put(d.path(), "textures/o/colour.png");
    let edits = vec![SetEdit::Move {
        file: "textures/o/colour.png".into(),
        to: X.into(),
    }];
    let (p, ev) = plan_with(&d, Target::Cs, &[(X, Shader::AdvancedSkin)], edits);
    assert!(job(&p, "colour.png").set_key.ends_with(X));
    // Pinned as it is now: unmarked, the set is not skin by its maps, so the joined `x_wet` is not typed `skin_wet`.
    assert_eq!(sub(&p, "x_wet.png"), Subtype::GenericData);
    assert!(not_shared(&resolved(&ev).1), "{:?}", resolved(&ev));
}
