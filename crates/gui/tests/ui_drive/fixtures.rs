//! The projects the UI driver runs on, the fake file dialog and the app-level steps (`pick`).

use crate::app_harness::{self, harness, plain_harness, settle, H};
use crate::steps::{ui_step, Step};
use crate::support;
use sdc_gui::model::picker::{PickerBackend, PickerKind, PickerSpec};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type Fixture = fn(&Path) -> AppModel;

pub const FIXTURES: &[(&str, Fixture)] = &[("demo", demo_opened)];

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
    let export = dir.join("export");
    let settings = GuiSettings {
        export_folder: Some(export.clone()),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, support::session());
    m.open_folder(root);
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m.set_shader(
        "architecture/stone/cobble",
        Some(engine::shader::Shader::Pbr),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.set_export_folder(export);
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
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
        other => {
            ui_step(h, other)?;
            settle(h, 2);
        }
    }
    Ok(None)
}
