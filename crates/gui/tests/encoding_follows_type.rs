//! Task 9 (owner, 2026-10-13; DECISIONS Z11): when a texture's sub-type changes, its explicit format and tag that
//! the new sub-type does not allow are cleared, so the new type's default applies; a note says so.
mod support;
use engine::config::{ColorTag, Format, Subtype, Target};
use engine::policy::allowed_encodings;
use engine::shader::Shader;
use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::sets::set_views;
use sdc_gui::model::AppModel;
use serde_json::json;
use std::time::Duration;

const KEY: &str = "plants/g.png";

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

fn file(m: &AppModel, f: &str) -> Option<serde_json::Value> {
    m.project.as_ref().unwrap().file_value(KEY, f)
}

fn job_of(m: &AppModel) -> engine::policy::ResolvedJob {
    m.item(KEY).and_then(|i| i.job.clone()).expect("job")
}

/// The owner's case: `g.png` with `g_rmaos.png` is a TruePBR set (its albedo `pbr_albedo`); under the complex_grass
/// shader the albedo becomes complex grass, and `e` is written on it explicitly. Returns the model and the set id.
fn grass_with(dir: &std::path::Path, e: Option<(Format, ColorTag)>) -> (AppModel, String) {
    let t = dir.join("textures/plants");
    std::fs::create_dir_all(&t).unwrap();
    image::ImageBuffer::from_fn(32, 64, |_, y| {
        image::Rgba([64u8, 128, 38, if y % 3 == 0 { 255 } else { 0 }])
    })
    .save(t.join("g.png"))
    .unwrap();
    image::ImageBuffer::from_fn(32, 32, |_, _| image::Rgb([100u8, 0, 255]))
        .save(t.join("g_rmaos.png"))
        .unwrap();
    let mut m = support::opened(dir);
    m.accept = engine::policy::AcceptSuggestions::High;
    m.replan();
    idle(&mut m);
    assert_eq!(job_of(&m).profile.subtype, Subtype::PbrAlbedo);
    // Note FC1: the accepted type is recorded; untouched, the shader below still re-types it (R5).
    assert_eq!(file(&m, "subtype"), Some(json!("pbr_albedo")));
    let set = set_views(&m)
        .into_iter()
        .find(|v| v.members.iter().any(|k| k == KEY))
        .expect("the set")
        .id;
    m.set_shader(&set, Some(Shader::ComplexGrass));
    idle(&mut m);
    assert_eq!(job_of(&m).profile.subtype, Subtype::ComplexGrass);
    if let Some((f, t)) = e {
        m.set_format(KEY, f, t);
        idle(&mut m);
    }
    m.encoding_note = None;
    (m, set)
}

const LINEAR: (Format, ColorTag) = (Format::Bc7, ColorTag::Unorm);

#[test]
fn switching_grass_to_pbr_resets_a_linear_albedo_and_says_so() {
    assert!(allowed_encodings(Subtype::ComplexGrass, Target::Cs).contains(&LINEAR));
    let d = tempfile::tempdir().unwrap();
    let (mut m, set) = grass_with(d.path(), Some(LINEAR));
    assert_eq!(file(&m, "format"), Some(json!("bc7")));
    m.set_shader(&set, Some(Shader::Pbr));
    idle(&mut m);
    let p = job_of(&m).profile;
    assert_ne!(p.subtype, Subtype::ComplexGrass);
    assert!(
        !allowed_encodings(p.subtype, m.target).contains(&LINEAR),
        "{:?} {:?}",
        p.subtype,
        m.target
    );
    assert_eq!(file(&m, "format"), None, "the explicit format is cleared");
    assert_eq!(file(&m, "color.tag"), None, "the explicit tag is cleared");
    assert_eq!((p.format, p.color.tag), (Format::Bc7, ColorTag::Srgb));
    let (line, _) = m.encoding_note.clone().expect("a note");
    assert!(
        line.starts_with("g.png: complex grass → pbr albedo"),
        "{line}"
    );
    assert!(line.contains("BC7 · sRGB"), "{line}");
    assert!(
        m.messages.entries().any(|e| e.text == line),
        "the status-line note is in the Messages log"
    );
}

#[test]
fn an_explicit_encoding_the_new_type_allows_is_kept() {
    // Complex grass allows BC7 linear only and a TruePBR albedo sRGB only, so a landscape diffuse becomes a diffuse:
    // both allow BC3 linear.
    const ROCK: &str = "landscape/rock01.png";
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    let bc3 = (Format::Bc3, ColorTag::Unorm);
    m.set_format(ROCK, bc3.0, bc3.1);
    idle(&mut m);
    m.encoding_note = None;
    m.set_field(ROCK, "subtype", json!("diffuse"));
    idle(&mut m);
    let p = m
        .item(ROCK)
        .and_then(|i| i.job.clone())
        .expect("job")
        .profile;
    assert_eq!(p.subtype, Subtype::Diffuse);
    assert!(allowed_encodings(p.subtype, m.target).contains(&bc3));
    let own = |f| m.project.as_ref().unwrap().file_value(ROCK, f);
    assert_eq!(own("format"), Some(json!("bc3")));
    assert_eq!(own("color.tag"), Some(json!("unorm")));
    assert_eq!(m.encoding_note, None);
}

#[test]
fn a_texture_in_a_locked_slot_is_reset_too() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, set) = grass_with(d.path(), Some(LINEAR));
    assert_eq!(
        m.run_menu(MenuAction::SlotLock {
            set: set.clone(),
            n: 1,
            on: true
        }),
        None
    );
    idle(&mut m);
    m.set_shader(&set, Some(Shader::Pbr));
    idle(&mut m);
    // Note JG3: a lock no longer keeps a pair the type does not allow.
    assert_eq!(file(&m, "format"), None);
    assert_eq!(file(&m, "color.tag"), None);
    let (line, _) = m.encoding_note.clone().expect("a note");
    assert!(line.contains("format reset to the default"), "{line}");
}

#[test]
fn one_undo_restores_the_type_and_the_cleared_encoding() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, set) = grass_with(d.path(), Some(LINEAR));
    let steps = m.history.undo.len();
    m.set_shader(&set, Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(file(&m, "format"), None);
    assert_eq!(
        m.history.undo.len(),
        steps + 1,
        "the cleanup is part of the shader step"
    );
    m.undo();
    idle(&mut m);
    assert_eq!(job_of(&m).profile.subtype, Subtype::ComplexGrass);
    assert_eq!(file(&m, "format"), Some(json!("bc7")));
    assert_eq!(file(&m, "color.tag"), Some(json!("unorm")));
    assert_eq!(m.encoding_note, None, "the undo dismissed the note");
    assert!(m.can_redo(), "the undo did not start a new step");
    m.redo();
    idle(&mut m);
    assert_eq!(file(&m, "format"), None);
}

#[test]
fn the_cleanup_costs_one_extra_plan() {
    let flip = |e| {
        let d = tempfile::tempdir().unwrap();
        let (mut m, set) = grass_with(d.path(), e);
        let g = m.generation();
        m.set_shader(&set, Some(Shader::Pbr));
        idle(&mut m);
        m.generation() - g
    };
    let (plain, cleaned) = (flip(None), flip(Some(LINEAR)));
    assert!(
        cleaned == plain + 1,
        "{plain} plans without, {cleaned} with"
    );
}

/// `name.png` with `name_rmaos.png` under `textures/plants`: a TruePBR set once High suggestions are accepted.
fn pbr_pair(dir: &std::path::Path, name: &str) {
    let t = dir.join("textures/plants");
    std::fs::create_dir_all(&t).unwrap();
    image::ImageBuffer::from_fn(32, 64, |_, y| {
        image::Rgba([64u8, 128, 38, if y % 3 == 0 { 255 } else { 0 }])
    })
    .save(t.join(format!("{name}.png")))
    .unwrap();
    image::ImageBuffer::from_fn(32, 32, |_, _| image::Rgb([100u8, 0, 255]))
        .save(t.join(format!("{name}_rmaos.png")))
        .unwrap();
}

/// Review M6: two textures re-typed by one bulk edit give one count line, each listed on hover, in one step.
#[test]
fn several_textures_give_one_count_line_with_each_on_hover() {
    use sdc_gui::model::bulk::BulkOp;
    let d = tempfile::tempdir().unwrap();
    pbr_pair(d.path(), "g");
    pbr_pair(d.path(), "h");
    let mut m = support::opened(d.path());
    m.accept = engine::policy::AcceptSuggestions::High;
    m.replan();
    idle(&mut m);
    let keys = vec!["plants/g.png".to_string(), "plants/h.png".to_string()];
    for k in &keys {
        m.set_field(k, "subtype", json!("complex_grass"));
        m.set_format(k, Format::Bc7, ColorTag::Unorm);
    }
    idle(&mut m);
    let steps = m.history.undo.len();
    m.apply_bulk_edit(&keys, "subtype", BulkOp::Set(json!("pbr_albedo")));
    idle(&mut m);
    assert_eq!(
        m.history.undo.len(),
        steps + 1,
        "one step for the edit and the cleanup"
    );
    for k in &keys {
        assert_eq!(
            m.project.as_ref().unwrap().file_value(k, "format"),
            None,
            "{k}"
        );
    }
    let (line, hover) = m.encoding_note.clone().expect("a note");
    assert!(
        line.starts_with("Encoding follows type: 2 textures"),
        "{line}"
    );
    assert!(
        hover.contains("g.png: complex grass → pbr albedo"),
        "{hover}"
    );
    assert!(
        hover.contains("h.png: complex grass → pbr albedo"),
        "{hover}"
    );
}

/// Note JG3 (supersedes Review M6's lock): an explicit pair the type does not allow is cleared; with nothing explicit
/// the best allowed pair is pinned (BC7 first, the tag kept where it can be); an allowed pair is kept.
#[test]
fn the_verdict_clears_or_pins() {
    use sdc_gui::model::encoding_follow::{verdict, Verdict};
    let t = Target::Cs;
    let srgb = (Format::Bc7, ColorTag::Srgb);
    assert_eq!(verdict(t, Subtype::PbrAlbedo, LINEAR, true), Verdict::Clear);
    assert_eq!(
        verdict(t, Subtype::PbrAlbedo, LINEAR, false),
        Verdict::Pin(srgb),
        "nothing explicit: the best allowed pair"
    );
    assert_eq!(
        verdict(t, Subtype::PbrAlbedo, srgb, true),
        Verdict::Keep,
        "still allowed"
    );
    assert_eq!(
        verdict(t, Subtype::SkinSk, LINEAR, false),
        Verdict::Pin((Format::Bc4, ColorTag::Unorm)),
        "a grey map's best is BC4"
    );
}

#[test]
fn undo_after_the_encoding_cleanup_names_the_step_that_caused_it() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, set) = grass_with(d.path(), Some(LINEAR));
    m.set_shader(&set, Some(Shader::Pbr));
    idle(&mut m);
    assert_eq!(file(&m, "format"), None, "the cleanup ran");
    // The cleanup folded into the Shader step: no step of its own.
    assert_eq!(m.undo_label(), Some("Shader"));
    m.undo();
    assert!(
        m.status_line.starts_with("Undid: Shader."),
        "{}",
        m.status_line
    );
}

/// An explicit R8 on a texture re-typed to a `skin_wet` is cleared, grey or not: CS's loader reads no one-channel
/// skin map (the grey form is gone).
#[test]
fn a_skin_wet_clears_an_explicit_r8_grey_or_not() {
    use sdc_gui::model::encoding_follow::{verdict, Verdict};
    let r8 = (Format::R8, ColorTag::Unorm);
    let st = Subtype::SkinWet;
    assert_eq!(verdict(Target::Cs, st, r8, true), Verdict::Clear);
}

/// Note FC3 (supersedes Z11's status line): a cleared format is an Info row per texture, with Undo while its step is
/// the last one; it leaves "ready" alone.
#[test]
fn a_cleared_format_is_an_info_row_with_undo() {
    use sdc_gui::model::bottom::{counts_as_ready_problem, problems, At, Fix, Level};
    let d = tempfile::tempdir().unwrap();
    let (mut m, set) = grass_with(d.path(), Some(LINEAR));
    m.set_shader(&set, Some(Shader::Pbr));
    idle(&mut m);
    // Note FC1's "types set automatically" row is the opening's, not this edit's. The fixture's PACK-DEFAULTED notes
    // are Info rows too (review E2); the cleared format's row has no code.
    let edit_rows =
        |p: &&sdc_gui::model::bottom::Problem| p.level == Level::Info && p.code.is_none();
    let rows: Vec<_> = problems(&m)
        .iter()
        .filter(edit_rows)
        .filter(|p| !p.text.contains("set automatically"))
        .cloned()
        .collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].at, At::Project);
    assert!(
        rows[0]
            .text
            .starts_with("g.png: complex grass → pbr albedo"),
        "{}",
        rows[0].text
    );
    assert_eq!(rows[0].fix, Some(Fix::UndoLast));
    assert!(!counts_as_ready_problem(&rows[0]));
    m.undo();
    idle(&mut m);
    assert!(!problems(&m).iter().any(|p| edit_rows(&p)));
    assert_eq!(
        file(&m, "format"),
        Some(json!("bc7")),
        "the undo restored it"
    );
}
