//! Note HG2: an explicit Advanced skin on a colourless set (`_n`, `_sk`, its `_wet` and `_detail_n`) marks it shared:
//! the maps other skin sets use. Its members are typed as skin members, no diffuse or `_rfaos` is looked for, a created
//! or renamed set is shared by the same rule, and a hand edit that gives it a colour map or another kind of member
//! unmarks it.
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
    resolved_of(events, X)
}

/// The `SetResolved` event of set `id`: its pick and warning codes.
fn resolved_of(events: &[EngineEvent], id: &str) -> (Option<Shader>, Vec<Code>) {
    events
        .iter()
        .find_map(|e| match &e.kind {
            EventKind::SetResolved {
                shader, warnings, ..
            } if e.set.as_deref().is_some_and(|s| s.ends_with(id)) => Some((
                shader.as_ref().map(|p| p.shader),
                warnings.iter().map(|w| w.code).collect(),
            )),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a SetResolved for {id}"))
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

/// Note HG3: outside a shared set an explicit shader has no say in the join, so both cases plan as in 2.0: an
/// explicit Advanced set with a colour map and no `_rfaos` leaves `x_wet` a set of its own, and an explicit
/// non-Advanced set with a real `_rfaos` joins its `_wet`.
#[test]
fn an_explicit_shader_does_not_change_the_wet_join() {
    let colour = tempfile::tempdir().unwrap();
    put(colour.path(), "textures/s/e/x.png");
    put(colour.path(), "textures/s/e/x_wet.png");
    let (c, _) = plan_with(
        &colour,
        Target::Cs,
        &[("textures/s/e/x", Shader::AdvancedSkin)],
        vec![],
    );
    let rfaos = tempfile::tempdir().unwrap();
    put(rfaos.path(), "textures/s/e/x.png");
    put(rfaos.path(), "textures/s/e/x_rfaos.png");
    put(rfaos.path(), "textures/s/e/x_wet.png");
    let (r, _) = plan_with(
        &rfaos,
        Target::Cs,
        &[("textures/s/e/x", Shader::Skin)],
        vec![],
    );
    assert_eq!(
        (
            same_set(&c, "x.png", "x_wet.png"),
            same_set(&r, "x.png", "x_wet.png")
        ),
        (false, true),
        "(Advanced with a colour map joins, Skin with an _rfaos joins)"
    );
    assert_eq!(sub(&c, "x_wet.png"), Subtype::Diffuse);
    assert_eq!(sub(&r, "x_wet.png"), Subtype::SkinWet);
}

const Y: &str = "textures/s/y";

/// Move `files` under `textures/s` into `set`, created first.
fn create_with(set: &str, files: &[&str]) -> Vec<SetEdit> {
    let mut edits = vec![SetEdit::Create { set: set.into() }];
    edits.extend(files.iter().map(|f| SetEdit::Move {
        file: format!("textures/s/{f}"),
        to: set.into(),
    }));
    edits
}

fn in_set(p: &Plan, set: &str) -> Vec<Subtype> {
    p.jobs
        .iter()
        .filter(|j| j.set_key.ends_with(set))
        .map(|j| j.profile.subtype)
        .collect()
}

#[test]
fn a_created_set_of_n_and_sk_is_shared() {
    let d = shared_files();
    let edits = create_with(Y, &["x_normal.png", "x_sk.png"]);
    let (p, ev) = plan_with(&d, Target::Cs, &[(Y, Shader::AdvancedSkin)], edits);
    assert_eq!(sub(&p, "x_normal.png"), Subtype::PbrNormal);
    assert_eq!(sub(&p, "x_sk.png"), Subtype::SkinSk);
    let (pick, codes) = resolved_of(&ev, Y);
    assert_eq!(pick, Some(Shader::AdvancedSkin));
    assert!(!codes.contains(&Code::ShaderMissingMap), "{codes:?}");
    assert!(!codes.contains(&Code::ShaderExtraMap), "{codes:?}");
    // No diffuse is looked for or written: the set is its two maps.
    let mut got = in_set(&p, Y);
    got.sort_by_key(|s| format!("{s:?}"));
    assert_eq!(got, [Subtype::PbrNormal, Subtype::SkinSk]);
}

/// A hand-added `_wet` or `_detail_n` has no suffix of its own in a created set: `apply_set_edits` rebuilds the set's
/// suffixes through `member_split`, which reads those two tails only in a set already listing them.
#[test]
fn a_hand_added_wet_and_detail_type_as_skin_maps() {
    let d = shared_files();
    put(d.path(), "textures/s/x_detail_n.png");
    let edits = create_with(
        Y,
        &["x_normal.png", "x_sk.png", "x_wet.png", "x_detail_n.png"],
    );
    let (p, ev) = plan_with(&d, Target::Cs, &[(Y, Shader::AdvancedSkin)], edits);
    assert_eq!(sub(&p, "x_normal.png"), Subtype::PbrNormal);
    assert_eq!(sub(&p, "x_sk.png"), Subtype::SkinSk);
    assert_eq!(sub(&p, "x_wet.png"), Subtype::SkinWet);
    assert_eq!(sub(&p, "x_detail_n.png"), Subtype::SkinDetail);
    let codes = resolved_of(&ev, Y).1;
    assert!(!codes.contains(&Code::ShaderMissingMap), "{codes:?}");
}

/// The shader is stored under the new id, so the join before set edits finds it through the Rename.
#[test]
fn a_renamed_host_stays_shared() {
    let d = shared_files();
    let edits = vec![SetEdit::Rename {
        from: X.into(),
        to: Y.into(),
    }];
    let (p, ev) = plan_with(&d, Target::Cs, &[(Y, Shader::AdvancedSkin)], edits);
    assert_eq!(sub(&p, "x_normal.png"), Subtype::PbrNormal);
    assert_eq!(sub(&p, "x_sk.png"), Subtype::SkinSk);
    assert_eq!(sub(&p, "x_wet.png"), Subtype::SkinWet);
    assert!(job(&p, "x_wet.png").set_key.ends_with(Y));
    assert!(!resolved_of(&ev, Y).1.contains(&Code::ShaderMissingMap));
}

/// Pin: a Rename that goes stale leaves the set under its old id, which the shader on the taken id does not name.
#[test]
fn a_stale_rename_does_not_lend_its_shader() {
    let d = shared_files();
    put(d.path(), "textures/s/y.png");
    let edits = vec![SetEdit::Rename {
        from: X.into(),
        to: Y.into(),
    }];
    let (p, _) = plan_with(&d, Target::Cs, &[(Y, Shader::AdvancedSkin)], edits);
    assert_eq!(sub(&p, "x_normal.png"), Subtype::Normal);
    assert!(!same_set(&p, "x_normal.png", "x_wet.png"));
}

/// The member rule reads names, not the set's current name, so an ordinary set's members keep their meaning through a
/// rename: `road_wet` is the colour, alone or as the base of `road_wet_n`, and `x_detail_n` is no detail map.
#[test]
fn renaming_a_colour_set_keeps_its_colour() {
    let plan = |files: &[&str], from: &str, to: &str| {
        let d = tempfile::tempdir().unwrap();
        for f in files {
            put(d.path(), &format!("textures/s/{f}.png"));
        }
        let (from, to) = (format!("textures/s/{from}"), format!("textures/s/{to}"));
        let edits = vec![SetEdit::Rename {
            from,
            to: to.clone(),
        }];
        let (p, _) = plan_with(&d, Target::Cs, &[(&to, Shader::AdvancedSkin)], edits);
        p
    };
    let lone = plan(&["road_wet"], "road_wet", "road");
    let with_n = plan(&["road_wet", "road_wet_n"], "road_wet", "road");
    let detail = plan(&["x_detail_n"], "x_detail", "y");
    // With a sibling of its base `x_detail_n` is that base's normal, in a set of any name.
    let sibling = plan(&["x_detail_n", "x_detail_sk"], "x_detail", "y");
    assert_eq!(
        [
            sub(&lone, "road_wet.png"),
            sub(&with_n, "road_wet.png"),
            sub(&detail, "x_detail_n.png"),
            sub(&sibling, "x_detail_n.png"),
        ],
        [
            Subtype::SkinAlbedo,
            Subtype::SkinAlbedo,
            Subtype::Normal,
            Subtype::PbrNormal,
        ]
    );
}

/// Pin: a lone `_wet` file is its set's colour member, whatever shader the set has: the set has no `_n` or `_sk`.
#[test]
fn a_set_named_after_its_wet_file_keeps_its_colour() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "textures/s/road_wet.png");
    let id = "textures/s/road_wet";
    let (p, ev) = plan_with(&d, Target::Cs, &[(id, Shader::AdvancedSkin)], vec![]);
    assert_eq!(sub(&p, "road_wet.png"), Subtype::SkinAlbedo);
    assert!(not_shared(&resolved_of(&ev, id).1));
}

#[test]
fn nothing_is_shared_off_cs() {
    let d = shared_files();
    let (p, _) = plan_with(&d, Target::Vanilla, &[(X, Shader::AdvancedSkin)], vec![]);
    assert_eq!(sub(&p, "x_normal.png"), Subtype::Normal);
    assert!(!same_set(&p, "x_normal.png", "x_wet.png"));
    // Nor a created set, which is marked after set edits.
    let edits = create_with(Y, &["x_normal.png", "x_sk.png"]);
    let (p, ev) = plan_with(&d, Target::Vanilla, &[(Y, Shader::AdvancedSkin)], edits);
    assert_eq!(sub(&p, "x_normal.png"), Subtype::Normal);
    assert!(not_shared(&resolved_of(&ev, Y).1));
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
