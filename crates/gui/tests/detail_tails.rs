//! Chunk E final review (Note GW1): the GUI reads a set's detail tails by the engine's rule, so a rename preview and a
//! helper's target name agree with the plan's outputs.
mod support;
use engine::shader::Shader;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use std::path::Path;
use std::time::Duration;
use support::session;

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

/// `files` under `dir` (a normal for `_n` and `_detail*`, a grey ramp for exporter terms, a diffuse otherwise), opened.
fn fixture(dir: &Path, files: &[&str]) -> AppModel {
    for f in files {
        let p = dir.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let stem = f.trim_end_matches(".png");
        if stem.ends_with("_n") {
            ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
                .save(p)
                .unwrap();
        } else if stem.ends_with("_opacity") {
            ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
                .save(p)
                .unwrap();
        } else {
            ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
                .save(p)
                .unwrap();
        }
    }
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::High;
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    m
}

fn out_stem(m: &AppModel, file: &str) -> String {
    let it = m
        .items
        .iter()
        .find(|i| i.key.ends_with(file))
        .unwrap_or_else(|| panic!("no item {file}"));
    let job = it.job.as_ref().unwrap_or_else(|| panic!("no job {file}"));
    job.output
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// Renames the set holding `member` to `yy`, and checks every file in `files` keeps the tail the plan gave it: the
/// plan's output stem was `<base><tail>`, and the preview is `yy<tail>`.
fn assert_rename_keeps_the_plans_tails(m: &mut AppModel, member: &str, base: &str, files: &[&str]) {
    let before: Vec<String> = files.iter().map(|f| out_stem(m, f)).collect();
    let id = sdc_gui::model::sets::views(m)
        .iter()
        .find(|v| v.members.iter().any(|k| k.ends_with(member)))
        .unwrap_or_else(|| panic!("no set with {member}"))
        .id
        .clone();
    m.set_output_name(&id, Some("yy")).unwrap();
    idle(m);
    for (f, b) in files.iter().zip(&before) {
        let tail = b
            .strip_prefix(base)
            .unwrap_or_else(|| panic!("{f}: plan stem {b} does not start with {base}"));
        assert_eq!(out_stem(m, f), format!("yy{tail}"), "{f}");
    }
}

/// A set with no colour member: `x_n` and `x_detail` (no `x.png`).
#[test]
fn a_detail_member_in_a_set_with_no_colour_member_keeps_its_tail() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path(), &["a/x_n.png", "a/x_detail.png"]);
    let (n, det) = (out_stem(&m, "x_n.png"), out_stem(&m, "x_detail.png"));
    assert_eq!((n.as_str(), det.as_str()), ("x_n", "x_detail"));
    assert_rename_keeps_the_plans_tails(&mut m, "x_n.png", "x", &["x_n.png", "x_detail.png"]);
}

/// A landscape-path set whose shader is chosen as something else: the engine still joins `x_detail_n` (path rule).
#[test]
fn a_landscape_path_set_with_another_shader_keeps_detail_n() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(
        d.path(),
        &[
            "textures/landscape/x.png",
            "textures/landscape/x_detail_n.png",
        ],
    );
    let set = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.members.iter().any(|k| k.ends_with("x_detail_n.png")))
        .expect("a set holds x_detail_n")
        .id
        .clone();
    m.set_shader(&set, Some(Shader::Default));
    idle(&mut m);
    assert_eq!(
        out_stem(&m, "x_detail_n.png"),
        "x_detail_n",
        "the engine joins it to set x under its own spelling"
    );
    assert_rename_keeps_the_plans_tails(&mut m, "x.png", "x", &["x.png", "x_detail_n.png"]);
}

/// A lone `rock_detail` colour set is its own set: its helper's target is named `rock_detail`, not `rock`.
#[test]
fn a_lone_detail_named_colour_sets_helper_target_keeps_its_name() {
    let d = tempfile::tempdir().unwrap();
    let m = fixture(d.path(), &["rock_detail.png", "rock_detail_opacity.png"]);
    let rep = m
        .item("rock_detail_opacity.png")
        .unwrap()
        .helper
        .clone()
        .expect("a helper");
    let colour = engine::helper::FillTarget::Alpha(engine::helper::AlphaTarget::Diffuse);
    assert_eq!(
        sdc_gui::model::helpers::target_label(&rep, colour, m.helper_tails(&rep)),
        colour.label("rock_detail")
    );
}
