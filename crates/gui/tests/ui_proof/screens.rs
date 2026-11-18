//! The list of GUI screens the whole-app walk visits (`invariants::walk`). Each entry opens its screen the way the
//! matching doc-shot scenario (`doc_shots/scenarios.rs`) does, on a temp-dir fixture and without the GPU.
//!
//! To add a screen: add a `Screen` to `SCREENS` with the model calls (`open`) or step lines (`steps`) that open it, and
//! a `marker` only that screen can satisfy. The walk fails an entry whose marker is on the window before the entry
//! opens anything, is on the window before its step lines run, is missing afterwards or after a resize, or is
//! satisfied by another entry's screen.
//!
//! Left out of the list, each named for the doc-shot scenario it mirrors:
//! - `problems_tab`: `main window with problems` shows the same tab with an item selected, and a marker cannot tell
//!   the two apart, so they are one entry.
//! - `cube_panel`: needs six extra face files that neither fixture holds; the walk's screens use the two fixtures.
//! - `preview_encoded` and `preview_normal`: the 2D preview draws only on the GPU, and `render_*` tests run alone.
//! - Every doc-shot scenario writes its export folder to a fixed `D:\` path; the walk's fixtures keep it in the
//!   screen's own temp folder instead.
//! - A real scan (the Meshes view's winners and mods come from a scan database): the `meshes view` entry sets made-up
//!   rows on the model, as the doc shot does.

use crate::app_harness::{plain_harness, settle, H};
use crate::fixtures;
use crate::frame::{snapshot, Snapshot};
use crate::invariants::Entry;
use eframe::egui::Vec2;
use sdc_gui::model::bottom::BottomTab;
use sdc_gui::model::rename::RenameMode;
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use std::sync::Arc;
use std::time::Duration;

/// The fixture name of a screen that has no project open (the start screen).
pub const NO_PROJECT: &str = "none";

/// One condition on a frame: a node of `role` (any role when empty) named `text`, or a painted text equal to it, is
/// there (`present`) or is not.
pub struct Needle {
    pub role: &'static str,
    pub text: &'static str,
    pub present: bool,
}

pub const fn has(text: &'static str) -> Needle {
    Needle {
        role: "",
        text,
        present: true,
    }
}

pub const fn has_role(role: &'static str, text: &'static str) -> Needle {
    Needle {
        role,
        text,
        present: true,
    }
}

const fn lacks(text: &'static str) -> Needle {
    Needle {
        role: "",
        text,
        present: false,
    }
}

/// True when every needle of `marker` holds on `s`.
pub fn holds(s: &Snapshot, marker: &[Needle]) -> bool {
    marker.iter().all(|n| {
        let found = s.nodes.iter().any(|x| {
            (n.role.is_empty() || x.role == n.role)
                && (x.label == n.text
                    || x.value.as_deref() == Some(n.text)
                    || x.placeholder.as_deref() == Some(n.text))
        }) || (n.role.is_empty() && s.texts.iter().any(|t| t.text == n.text));
        found == n.present
    })
}

pub struct Screen {
    pub name: &'static str,
    /// A fixture of `ui_drive/fixtures.rs` (`demo`, `helpers`), or `NO_PROJECT`.
    pub fixture: &'static str,
    /// Model calls that open the screen, run right after the fixture has opened.
    pub open: fn(&mut H),
    /// Step lines (`ui_drive/steps.rs`) run after `open`, for a menu or popup a click opens.
    pub steps: &'static [&'static str],
    /// What only this screen shows. It must not hold before `open` or, with `steps`, before the steps, and no other
    /// entry's screen may satisfy it.
    pub marker: &'static [Needle],
    /// A menu or popup: reopened at each size, because a resize can close it. A stable screen is opened once and
    /// resized.
    pub transient: bool,
}

fn nothing(_: &mut H) {}

fn model(h: &mut H) -> &mut AppModel {
    &mut h.state_mut().model
}

fn idle(h: &mut H) {
    assert!(model(h).wait_idle(Duration::from_secs(60)), "no idle");
}

/// The key of the set whose id ends with `id`.
fn set_key(h: &mut H, id: &str) -> String {
    model(h)
        .sets
        .iter()
        .find(|s| s.key.ends_with(id))
        .unwrap_or_else(|| panic!("no set {id}"))
        .key
        .clone()
}

fn select_set(h: &mut H, id: &str) {
    let k = set_key(h, id);
    model(h).select_set(&k);
}

/// A set's Material tab after giving it `shader`.
fn material_tab(h: &mut H, set: &str, shader: engine::shader::Shader) {
    model(h).set_shader(set, Some(shader));
    idle(h);
    select_set(h, set);
}

pub const SCREENS: &[Screen] = &[
    Screen {
        name: "start screen",
        fixture: NO_PROJECT,
        open: nothing,
        steps: &[],
        marker: &[has("Open a folder of source textures to begin.")],
        transient: false,
    },
    Screen {
        name: "main window with problems",
        fixture: "demo",
        open: |h| {
            model(h).select_item("architecture/stone/stonewall.png");
            model(h).toggle_tab(BottomTab::Problems);
        },
        steps: &[],
        marker: &[has("Kinds ▾"), has("← architecture/stone/stonewall")],
        transient: false,
    },
    Screen {
        name: "export to menu",
        fixture: "demo",
        open: nothing,
        steps: &[r#"{"op":"click","at":{"starts":"Export to","role":"Button"}}"#],
        marker: &[has("Re-encode every output")],
        transient: true,
    },
    Screen {
        name: "settings tab",
        fixture: "demo",
        open: |h| model(h).open_settings_tab(),
        steps: &[],
        marker: CLOSE_SETTINGS,
        transient: false,
    },
    Screen {
        name: "tile menu",
        fixture: "demo",
        open: |h| model(h).select_item("architecture/stone/stonewall.png"),
        steps: &[r#"{"op":"right_click","at":{"label":"stonewall.png","index":1}}"#],
        marker: &[has("Copy settings from stonewall.png to 0 selected…")],
        transient: true,
    },
    Screen {
        name: "texture panel",
        fixture: "demo",
        open: |h| model(h).select_item("plants/fern.png"),
        steps: &[],
        marker: &[has("← plants/fern"), has("Alpha threshold")],
        transient: false,
    },
    Screen {
        name: "set card menu",
        fixture: "demo",
        open: |h| select_set(h, "architecture/stone/stonewall"),
        steps: &[r#"{"op":"right_click","at":{"label":"architecture/stone/stonewall","index":0}}"#],
        marker: &[has("Fill from names…")],
        transient: true,
    },
    Screen {
        name: "review tab",
        fixture: "demo",
        open: |h| model(h).open_review(),
        steps: &[],
        marker: &[has("What will be written")],
        transient: false,
    },
    Screen {
        name: "presets tab",
        fixture: "demo",
        open: |h| {
            let m = model(h);
            m.open_presets();
            let id = engine::presets::default_preset_for(m.target).to_string();
            m.presets_model().select(&id);
        },
        steps: &[],
        marker: &[has(
            "Built-in presets are read-only: Duplicate makes an editable copy.",
        )],
        transient: false,
    },
    Screen {
        name: "json editor",
        fixture: "demo",
        open: |h| model(h).open_json_editor(),
        steps: &[],
        marker: &[has("+ Add a delete section")],
        transient: false,
    },
    Screen {
        name: "rename inputs dialog",
        fixture: "demo",
        open: |h| {
            let m = model(h);
            m.select_item("architecture/stone/stonewall.png");
            m.click_item("architecture/stone/cobble.png", true, false);
            m.open_rename(RenameMode::Inputs);
        },
        steps: &[],
        marker: &[has("Rename 2 inputs")],
        transient: false,
    },
    Screen {
        name: "marker dialog",
        fixture: "demo",
        open: |h| model(h).open_marker_dialog(),
        steps: &[],
        marker: &[has("F0 = (code/255)^1.6")],
        transient: false,
    },
    Screen {
        name: "set editor",
        fixture: "demo",
        open: |h| select_set(h, "architecture/stone/stonewall"),
        steps: &[],
        marker: &[
            has("textures/architecture/stone/stonewall"),
            lacks("Fill from names…"),
        ],
        transient: false,
    },
    Screen {
        name: "set editor slot menu",
        fixture: "demo",
        open: |h| select_set(h, "architecture/stone/cobble"),
        steps: &[r#"{"op":"right_click","at":{"label":"RMAOS","index":1}}"#],
        marker: &[
            has("textures/architecture/stone/cobble"),
            has("Slot 6: RMAOS"),
        ],
        transient: true,
    },
    Screen {
        name: "also export tab",
        fixture: "demo",
        open: |h| {
            select_set(h, "architecture/stone/cobble");
            settle(h, 4);
            sdc_gui::ui::set_editor::show_also_tab(&h.ctx);
        },
        steps: &[],
        marker: &[
            has("textures/architecture/stone/cobble"),
            has("Skip vanilla/CM for this set"),
        ],
        transient: false,
    },
    Screen {
        name: "pgp material tab",
        fixture: "demo",
        open: |h| select_set(h, "architecture/stone/cobble"),
        steps: &[],
        marker: &[
            has("textures/architecture/stone/cobble"),
            lacks("Skip vanilla/CM for this set"),
            lacks("Slot 6: RMAOS"),
        ],
        transient: false,
    },
    Screen {
        name: "helper panel",
        fixture: "helpers",
        open: |h| model(h).select_item("props/crate_roughness.png"),
        steps: &[],
        marker: &[has("← props/crate"), has_role("ComboBox", "Helper role")],
        transient: false,
    },
    Screen {
        name: "meshes view",
        fixture: "demo",
        open: meshes_view,
        steps: &[],
        marker: &[has("5 of 5 NIFs")],
        transient: false,
    },
    Screen {
        name: "mo2 popup",
        fixture: "demo",
        open: nothing,
        steps: &[r#"{"op":"click","at":{"label":"MO2…"}}"#],
        marker: &[has("Scan a folder…")],
        transient: true,
    },
    Screen {
        name: "skin card",
        fixture: "demo",
        open: |h| material_tab(h, "actors/body", engine::shader::Shader::AdvancedSkin),
        steps: &[],
        marker: &[has("textures/actors/body"), has("RFAOS, empty")],
        transient: false,
    },
    Screen {
        name: "landscape card",
        fixture: "demo",
        open: |h| material_tab(h, "pbr/metal", engine::shader::Shader::PbrLandscape),
        steps: &[],
        marker: &[has("textures/pbr/metal"), has("PBR Landscape")],
        transient: false,
    },
];

fn nif_row(copy_id: i64, path: &str) -> modscan::store::NifRow {
    modscan::store::NifRow {
        copy_id,
        path: path.into(),
        wins: true,
        overridden_by: None,
        n_copies: 1,
        derived: false,
        le_mesh: false,
        problem: None,
        patched_in_game: false,
    }
}

/// The Meshes view with a made-up scan (five meshes from three mods), as the doc shot has it.
fn meshes_view(h: &mut H) {
    use modscan::store::{ModRow, NifRow};
    {
        let m = &mut h.state_mut().meshes;
        let r#mod = |id, name: &str, derived| ModRow {
            id,
            name: name.into(),
            kind: 1,
            priority: id as i32,
            derived,
            nifs: 2,
        };
        m.mods = vec![
            r#mod(1, "StoneWork", false),
            r#mod(2, "Banners of the Hold", false),
            r#mod(3, "PGPatcher output", true),
        ];
        m.winners = Arc::new(vec![
            (
                nif_row(1, "meshes/architecture/stone/wall01.nif"),
                "StoneWork".into(),
            ),
            (
                NifRow {
                    n_copies: 2,
                    ..nif_row(2, "meshes/architecture/stone/wall02.nif")
                },
                "StoneWork".into(),
            ),
            (
                NifRow {
                    derived: true,
                    patched_in_game: true,
                    ..nif_row(3, "meshes/architecture/stone/cobblefloor.nif")
                },
                "PGPatcher output".into(),
            ),
            (
                nif_row(4, "meshes/clutter/banner01.nif"),
                "Banners of the Hold".into(),
            ),
            (
                NifRow {
                    le_mesh: true,
                    ..nif_row(5, "meshes/clutter/banner02.nif")
                },
                "Banners of the Hold".into(),
            ),
        ]);
    }
    sdc_gui::ui::header::set_meshes_view(&h.ctx, true);
}

fn at_text(size: Vec2) -> String {
    format!("{}x{}", size.x, size.y)
}

/// The screen opened on a fresh temp folder and settled at `size`. `Err` (screen and size included by the caller)
/// when the window or a step does not go idle, when the marker already holds before the entry opens anything or
/// before its step lines run, or when it does not hold afterwards. Keep the `TempDir` alive as long as the harness:
/// dropping it deletes the project folder the app reads.
pub fn enter(s: &Screen, size: Vec2) -> Result<(tempfile::TempDir, H), String> {
    crate::app_data();
    let d = tempfile::tempdir().unwrap();
    let (mut h, run) = if s.fixture == NO_PROJECT {
        let m = AppModel::new(GuiSettings::default(), None, crate::support::session());
        let picker = Arc::new(fixtures::DriverPicker::default());
        m.picker.set_backend(picker.clone());
        let mut h = plain_harness(m);
        settle(&mut h, 6);
        let run = fixtures::Run {
            dir: d.path().to_path_buf(),
            picker,
        };
        (h, run)
    } else {
        fixtures::start(s.fixture, d.path(), false)?
    };
    h.set_size(size);
    if !settle(&mut h, 2) {
        return Err("the window did not go idle after the fixture opened".into());
    }
    // the start screen is its own window: another entry's pairwise check covers it
    if s.fixture != NO_PROJECT && holds(&snapshot(&h), s.marker) {
        return Err("the marker already holds before the entry opens anything, so it cannot tell the screen from the window under it; choose a marker only the screen satisfies".into());
    }
    (s.open)(&mut h);
    if !settle(&mut h, 4) {
        return Err("the window did not go idle after `open`".into());
    }
    if !s.steps.is_empty() && holds(&snapshot(&h), s.marker) {
        return Err("the marker already holds before the step lines run, so the steps are not what shows it; choose a marker only the last step satisfies".into());
    }
    let entered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::play(&mut h, &run, s.steps, 0)
    }));
    if let Err(e) = entered {
        let why = e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        return Err(format!("a step line failed: {why}"));
    }
    if !settle(&mut h, 4) {
        return Err("the window did not go idle after the step lines".into());
    }
    if !holds(&snapshot(&h), s.marker) {
        return Err("the entry opened nothing that satisfies its marker; fix the entry's `open` or `steps`, or its `marker`".into());
    }
    Ok((d, h))
}

/// A screen of `SCREENS` as an entry of the walk.
pub struct ScreenEntry {
    screen: &'static Screen,
    open: Option<(tempfile::TempDir, H)>,
}

impl Entry for ScreenEntry {
    fn name(&self) -> &str {
        self.screen.name
    }

    fn marker(&self) -> &'static [Needle] {
        self.screen.marker
    }

    fn at(&mut self, size: Vec2) -> Result<Snapshot, String> {
        if !self.screen.transient {
            if let Some((_, h)) = &mut self.open {
                h.set_size(size);
                if !settle(h, 2) {
                    return Err("the window did not go idle after the resize".into());
                }
                let snap = snapshot(h);
                if !holds(&snap, self.screen.marker) {
                    return Err(format!(
                        "the screen no longer shows its marker after the resize to {}",
                        at_text(size)
                    ));
                }
                return Ok(snap);
            }
        }
        // the old window and its folder go before the new ones are built
        self.open = None;
        let (d, h) = enter(self.screen, size)?;
        let snap = snapshot(&h);
        self.open = Some((d, h));
        Ok(snap)
    }
}

/// Every screen of `SCREENS`, each as one entry.
pub fn entries() -> Vec<Box<dyn Entry>> {
    SCREENS
        .iter()
        .map(|screen| Box::new(ScreenEntry { screen, open: None }) as Box<dyn Entry>)
        .collect()
}

#[test]
fn every_entry_has_its_own_name_and_a_marker() {
    let mut names: Vec<&str> = SCREENS.iter().map(|s| s.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), SCREENS.len(), "two entries share a name");
    assert!(SCREENS.iter().all(|s| !s.marker.is_empty()));
    assert!(SCREENS.iter().all(|s| s.marker.iter().any(|n| n.present)));
}

/// An entry that opens nothing fails.
#[test]
fn an_entry_that_opens_nothing_fails() {
    let e = enter(
        &Screen {
            name: "test-only empty entry",
            fixture: "demo",
            open: nothing,
            steps: &[],
            marker: CLOSE_SETTINGS,
            transient: false,
        },
        vec2_1280(),
    )
    .map(|_| ())
    .expect_err("an entry that opens nothing is refused");
    assert!(e.contains("opened nothing"), "{e}");
}

/// An entry whose marker is on the window already fails.
#[test]
fn an_entry_whose_marker_is_already_on_the_window_fails() {
    let e = enter(
        &Screen {
            name: "test-only entry on the plain window",
            fixture: "demo",
            open: nothing,
            steps: &[],
            marker: SAVE_BUTTON,
            transient: false,
        },
        vec2_1280(),
    )
    .map(|_| ())
    .expect_err("a marker on the plain window is refused");
    assert!(e.contains("already holds before the entry"), "{e}");
}

/// A multi-step entry whose marker is there before the last step fails.
#[test]
fn an_entry_whose_marker_is_there_before_its_last_step_fails() {
    let e = enter(
        &Screen {
            name: "test-only entry whose open already shows it",
            fixture: "demo",
            open: |h| model(h).open_settings_tab(),
            steps: &[r#"{"op":"settle"}"#],
            marker: CLOSE_SETTINGS,
            transient: false,
        },
        vec2_1280(),
    )
    .map(|_| ())
    .expect_err("a marker present before the last step is refused");
    assert!(e.contains("before the step lines run"), "{e}");
}

const CLOSE_SETTINGS: &[Needle] = &[has("Close Settings")];
const SAVE_BUTTON: &[Needle] = &[has_role("Button", "Save")];

fn vec2_1280() -> Vec2 {
    eframe::egui::vec2(1280.0, 800.0)
}
