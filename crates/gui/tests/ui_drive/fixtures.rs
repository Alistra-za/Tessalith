//! The projects the UI driver runs on, the fake file dialog and the app-level steps (`pick`).

use crate::app_harness::{self, harness, plain_harness, settle, H};
use crate::oracles::ORACLES;
use crate::state::{eval_expect, state_json};
use crate::steps::{ui_step, Step};
use crate::support;
use engine::policy::AcceptSuggestions;
use engine::shader::Shader;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::picker::{PickerBackend, PickerKind, PickerSpec};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type Fixture = fn(&Path) -> AppModel;

pub const FIXTURES: &[(&str, Fixture)] = &[("demo", demo_opened), ("helpers", helpers_opened)];

/// Answers file dialogs from a queue; an empty queue is Cancel. `asked` records every dialog and its answer.
#[derive(Default)]
pub struct DriverPicker {
    answers: Mutex<VecDeque<PathBuf>>,
    pub asked: Mutex<Vec<String>>,
}

impl DriverPicker {
    pub fn queue(&self, p: PathBuf) {
        self.answers.lock().unwrap().push_back(p);
    }
}

impl PickerBackend for DriverPicker {
    fn pick(&self, spec: &PickerSpec) -> Option<PathBuf> {
        let answer = self.answers.lock().unwrap().pop_front();
        // the backend sees the spec, not the purpose: the title when it has one, else the kind of dialog
        let what = spec.title.unwrap_or(match spec.kind {
            PickerKind::Folder => "folder",
            PickerKind::File { .. } => "file",
            PickerKind::Save { .. } => "save",
        });
        let shown = answer
            .as_ref()
            .map_or("cancel".to_string(), |p| p.display().to_string());
        self.asked.lock().unwrap().push(format!("{what}: {shown}"));
        answer
    }
}

pub struct Run {
    pub dir: PathBuf,
    pub picker: Arc<DriverPicker>,
}

/// The demo project opened as the doc shots open it (cobble on PBR), with the export folder inside `dir`.
fn demo_opened(dir: &Path) -> AppModel {
    let root = app_harness::demo(dir);
    open_project(
        dir,
        root,
        None,
        &[("architecture/stone/cobble", Shader::Pbr)],
    )
}

/// Opens `root` with the export folder inside `dir`, `accept` set before the open when given, then each shader set.
fn open_project(
    dir: &Path,
    root: PathBuf,
    accept: Option<AcceptSuggestions>,
    shaders: &[(&str, Shader)],
) -> AppModel {
    let export = dir.join("export");
    let settings = GuiSettings {
        export_folder: Some(export.clone()),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, support::session());
    if let Some(a) = accept {
        m.accept = a;
    }
    m.open_folder(root);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    for (set, shader) in shaders {
        m.set_shader(set, Some(*shader));
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_export_folder(export);
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

/// The demo plus a `props/crate` set whose helpers dock on the cards (Note HN, Decision 1). A second fixture because
/// changing `demo` would move every count the goals and journeys pin. Suggestions are accepted (High) so the raw maps
/// become helpers, as in `helper_panel.rs`.
fn helpers_opened(dir: &Path) -> AppModel {
    let root = app_harness::demo(dir);
    let props = root.join("props");
    std::fs::create_dir_all(&props).unwrap();
    let save = |name: &str, f: &dyn Fn(u32, u32) -> [u8; 3]| {
        ImageBuffer::from_fn(128, 128, |x, y| Rgb(f(x, y)))
            .save(props.join(name))
            .unwrap();
    };
    save("crate.png", &|x, y| [x as u8, y as u8, 90]);
    save("crate_cnr.png", &|_, _| [128, 128, 255]);
    for n in ["roughness", "opacity", "metallic"] {
        ImageBuffer::from_fn(128, 128, |x, _| Luma([x as u8]))
            .save(props.join(format!("crate_{n}.png")))
            .unwrap();
    }
    std::fs::write(
        root.join("sdc.toml"),
        "[files.\"props/crate_roughness.png\"]\nhelper.fills = [\"rmaos.r\", \"cnr.a\"]\n\n\
         [files.\"props/crate_opacity.png\"]\nhelper.role = \"opacity\"\n\n\
         [files.\"props/crate_metallic.png\"]\nhelper.fills = []\n",
    )
    .unwrap();
    open_project(
        dir,
        root,
        Some(AcceptSuggestions::High),
        &[
            ("architecture/stone/cobble", Shader::Pbr),
            ("props/crate", Shader::Pbr),
        ],
    )
}

/// The fixture called `name`, or an error listing the known ones.
pub fn lookup(name: &str) -> Result<Fixture, String> {
    FIXTURES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, f)| *f)
        .ok_or_else(|| {
            let known: Vec<&str> = FIXTURES.iter().map(|(n, _)| *n).collect();
            format!("unknown fixture \"{name}\"; known: {}", known.join(", "))
        })
}

/// Build the fixture in `dir` and start the app on it; `wgpu` false skips the GPU (no `render()`).
pub fn start(fixture: &str, dir: &Path, wgpu: bool) -> Result<(H, Run), String> {
    let m = lookup(fixture)?(dir);
    let picker = Arc::new(DriverPicker::default());
    m.picker.set_backend(picker.clone());
    let mut h = if wgpu { harness(m) } else { plain_harness(m) };
    h.state_mut().shell_log = Some(vec![]);
    settle(&mut h, 10);
    Ok((
        h,
        Run {
            dir: dir.to_path_buf(),
            picker,
        },
    ))
}

/// An `Expect` or `Check` step against the model as it is now; `Some(note)` is a check that could not decide.
pub fn evaluate(h: &H, step: &Step) -> Result<Option<String>, String> {
    let m = &h.state().model;
    match step {
        Step::Expect {
            fact,
            equals,
            contains,
            absent,
        } => eval_expect(
            &state_json(m),
            fact,
            equals.as_ref(),
            contains.as_ref(),
            *absent,
        )
        .map(|()| None),
        Step::Check { name } => match ORACLES.iter().find(|(n, _)| n == name) {
            Some((_, f)) => f(m),
            None => {
                let known: Vec<&str> = ORACLES.iter().map(|(n, _)| *n).collect();
                Err(format!(
                    "unknown check \"{name}\"; known: {}",
                    known.join(", ")
                ))
            }
        },
        _ => Err("not an expect or check step".into()),
    }
}

/// Run one step. `Some(note)` is text the caller reports beside the step.
pub fn app_step(h: &mut H, run: &Run, step: &Step) -> Result<Option<String>, String> {
    match step {
        Step::Pick { path } => {
            let p = Path::new(path);
            if p.is_absolute()
                || p.has_root()
                || p.components().any(|c| matches!(c, Component::ParentDir))
            {
                return Err(format!(
                    "pick path \"{path}\" must be relative and inside the fixture folder"
                ));
            }
            run.picker.queue(run.dir.join(p));
        }
        Step::Settle { frames } => settle(h, *frames),
        Step::Expect { .. } | Step::Check { .. } => return evaluate(h, step),
        other => {
            ui_step(h, other)?;
            settle(h, 2);
        }
    }
    Ok(None)
}
