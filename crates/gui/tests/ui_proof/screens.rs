//! The list of GUI screens the whole-app walk visits (`invariants::walk`). Each entry opens its screen the way the
//! matching doc-shot scenario (`doc_shots/scenarios.rs`) does, on a temp-dir fixture and without the GPU.
//!
//! To add a screen: add a `Screen` to `SCREENS` with the model calls (`open`) or step lines (`steps`) that open it, and
//! the text only that screen shows (`shows`). `every_entry_opens_its_screen` fails an entry that opens nothing.
//!
//! Left out of the list, each named for the doc-shot scenario it mirrors:
//! - `cube_panel`: needs six extra face files that neither fixture holds; the walk's screens use the two fixtures.
//! - `preview_encoded` and `preview_normal`: the 2D preview draws only on the GPU, and `render_*` tests run alone.
//! - Every doc-shot scenario writes its export folder to a fixed `D:\` path; the walk's fixtures keep it in the
//!   screen's own temp folder instead.
//! - A real scan (the Meshes view's winners and mods come from a scan database): the `meshes view` entry sets made-up
//!   rows on the model, as the doc shot does.

use crate::app_harness::{settle, H};
use crate::fixtures;
use crate::frame::{snapshot, Snapshot};
use crate::invariants::SIZES;
use eframe::egui::Vec2;
use sdc_gui::model::bottom::BottomTab;
use sdc_gui::model::rename::RenameMode;
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use std::sync::Arc;
use std::time::Duration;

/// The fixture name of a screen that has no project open (the start screen).
pub const NO_PROJECT: &str = "none";

pub struct Screen {
    pub name: &'static str,
    /// A fixture of `ui_drive/fixtures.rs` (`demo`, `helpers`), or `NO_PROJECT`.
    pub fixture: &'static str,
    /// Model calls that open the screen, run right after the fixture has opened.
    pub open: fn(&mut H),
    /// Step lines (`ui_drive/steps.rs`) run after `open`, for a menu or popup a click opens.
    pub steps: &'static [&'static str],
    /// A label or text the screen shows and its fixture's window did not show before `open` (for `NO_PROJECT`, one a
    /// window with a project open does not show).
    pub shows: &'static str,
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
        shows: "Open a folder of source textures to begin.",
    },
    Screen {
        name: "main window with problems",
        fixture: "demo",
        open: |h| {
            model(h).select_item("architecture/stone/stonewall.png");
            model(h).toggle_tab(BottomTab::Problems);
        },
        steps: &[],
        shows: "Kinds ▾",
    },
    Screen {
        name: "export to menu",
        fixture: "demo",
        open: nothing,
        steps: &[r#"{"op":"click","at":{"starts":"Export to","role":"Button"}}"#],
        shows: "Also export as Vanilla",
    },
    Screen {
        name: "settings tab",
        fixture: "demo",
        open: |h| model(h).open_settings_tab(),
        steps: &[],
        shows: "Close Settings",
    },
    Screen {
        name: "problems tab",
        fixture: "demo",
        open: |h| model(h).toggle_tab(BottomTab::Problems),
        steps: &[],
        shows: "Kinds ▾",
    },
    Screen {
        name: "tile menu",
        fixture: "demo",
        open: |h| model(h).select_item("architecture/stone/stonewall.png"),
        steps: &[r#"{"op":"right_click","at":{"label":"stonewall.png","index":1}}"#],
        shows: "Reset all settings for 1 selected…",
    },
    Screen {
        name: "texture panel",
        fixture: "demo",
        open: |h| model(h).select_item("plants/fern.png"),
        steps: &[],
        shows: "Alpha threshold",
    },
    Screen {
        name: "set card menu",
        fixture: "demo",
        open: |h| select_set(h, "architecture/stone/stonewall"),
        steps: &[r#"{"op":"right_click","at":{"label":"architecture/stone/stonewall","index":0}}"#],
        shows: "Fill from names…",
    },
    Screen {
        name: "review tab",
        fixture: "demo",
        open: |h| model(h).open_review(),
        steps: &[],
        shows: "What will be written",
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
        shows: "Built-in presets are read-only",
    },
    Screen {
        name: "json editor",
        fixture: "demo",
        open: |h| model(h).open_json_editor(),
        steps: &[],
        shows: "+ Add a delete section",
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
        shows: "Rename 2 inputs",
    },
    Screen {
        name: "marker dialog",
        fixture: "demo",
        open: |h| model(h).open_marker_dialog(),
        steps: &[],
        shows: "F0 = (code/255)^1.6",
    },
    Screen {
        name: "set editor",
        fixture: "demo",
        open: |h| select_set(h, "architecture/stone/stonewall"),
        steps: &[],
        shows: "CS PBR records",
    },
    Screen {
        name: "set editor slot menu",
        fixture: "demo",
        open: |h| select_set(h, "architecture/stone/cobble"),
        steps: &[r#"{"op":"right_click","at":{"label":"RMAOS","index":1}}"#],
        shows: "Pin this slot (always use this file)",
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
        shows: "Skip vanilla/CM for this set",
    },
    Screen {
        name: "pgp material tab",
        fixture: "demo",
        open: |h| select_set(h, "architecture/stone/cobble"),
        steps: &[],
        shows: "Match on this",
    },
    Screen {
        name: "helper panel",
        fixture: "helpers",
        open: |h| model(h).select_item("props/crate_roughness.png"),
        steps: &[],
        shows: "crate_roughness.png",
    },
    Screen {
        name: "meshes view",
        fixture: "demo",
        open: meshes_view,
        steps: &[],
        shows: "5 of 5 NIFs",
    },
    Screen {
        name: "mo2 popup",
        fixture: "demo",
        open: nothing,
        steps: &[r#"{"op":"click","at":{"label":"MO2…"}}"#],
        shows: "Scan a folder…",
    },
    Screen {
        name: "skin card",
        fixture: "demo",
        open: |h| material_tab(h, "actors/body", engine::shader::Shader::AdvancedSkin),
        steps: &[],
        shows: "Advanced Skin",
    },
    Screen {
        name: "landscape card",
        fixture: "demo",
        open: |h| material_tab(h, "pbr/metal", engine::shader::Shader::PbrLandscape),
        steps: &[],
        shows: "Height _p",
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

/// The screen opened on a fresh temp folder, settled at `size`. Panics when `shows` was already on the
/// window before `open` (it would not tell the screen from the window under it), or is not on it afterwards (the entry
/// opened nothing). Keep the `TempDir` alive as long as the harness: dropping it deletes the project folder the app
/// reads.
pub fn enter(s: &Screen, size: Vec2) -> (tempfile::TempDir, H) {
    crate::app_data();
    let d = tempfile::tempdir().unwrap();
    let (mut h, run) = if s.fixture == NO_PROJECT {
        let m = AppModel::new(GuiSettings::default(), None, crate::support::session());
        let picker = Arc::new(fixtures::DriverPicker::default());
        m.picker.set_backend(picker.clone());
        let mut h = crate::app_harness::plain_harness(m);
        settle(&mut h, 6);
        let run = fixtures::Run {
            dir: d.path().to_path_buf(),
            picker,
        };
        (h, run)
    } else {
        fixtures::start(s.fixture, d.path(), false).unwrap()
    };
    h.set_size(size);
    settle(&mut h, 2);
    assert!(
        s.fixture == NO_PROJECT || !shows(&snapshot(&h), s.shows),
        "screen {:?}: {:?} is already on the window before the entry opens anything, so it cannot tell the screen from the window under it; choose a text only the screen shows",
        s.name,
        s.shows
    );
    (s.open)(&mut h);
    settle(&mut h, 4);
    crate::play(&mut h, &run, s.steps, 0);
    settle(&mut h, 4);
    assert!(
        shows(&snapshot(&h), s.shows),
        "screen {:?}: its entry opened nothing that shows {:?}; fix the entry's `open` or `steps`, or its `shows`",
        s.name,
        s.shows
    );
    (d, h)
}

/// True when a node's name, value or placeholder, or a painted text, holds `needle`.
pub fn shows(s: &Snapshot, needle: &str) -> bool {
    s.nodes.iter().any(|n| {
        n.label.contains(needle)
            || n.value.as_deref().is_some_and(|v| v.contains(needle))
            || n.placeholder.as_deref().is_some_and(|p| p.contains(needle))
    }) || s.texts.iter().any(|t| t.text.contains(needle))
}

/// Every entry opens a screen that shows its text (`enter` asserts it), and the names are distinct.
#[test]
fn every_entry_opens_its_screen() {
    let mut names: Vec<&str> = SCREENS.iter().map(|s| s.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), SCREENS.len(), "two entries share a name");
    for s in SCREENS {
        let _ = enter(s, SIZES[1]);
    }
}

/// The start screen has no window under it to compare with; its text must not be on a window with a project open.
#[test]
fn the_start_screen_text_is_not_on_an_open_project() {
    let start = SCREENS.iter().find(|s| s.fixture == NO_PROJECT).unwrap();
    let d = tempfile::tempdir().unwrap();
    crate::app_data();
    let (h, _) = fixtures::start("demo", d.path(), false).unwrap();
    assert!(!shows(&snapshot(&h), start.shows));
}

/// An entry that opens nothing fails.
#[test]
#[should_panic(expected = "its entry opened nothing")]
fn an_entry_that_opens_nothing_fails() {
    let _ = enter(
        &Screen {
            name: "test-only empty entry",
            fixture: "demo",
            open: nothing,
            steps: &[],
            shows: "Close Settings",
        },
        SIZES[1],
    );
}

/// An entry whose text is on the window already fails.
#[test]
#[should_panic(expected = "is already on the window")]
fn an_entry_whose_text_is_already_on_the_window_fails() {
    let _ = enter(
        &Screen {
            name: "test-only entry on the plain window",
            fixture: "demo",
            open: nothing,
            steps: &[],
            shows: "Export to",
        },
        SIZES[1],
    );
}
