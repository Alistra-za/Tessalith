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
/// `right_of_sets` asks for the node to start at or right of the Sets column's panel (`Snapshot::sets_right`): the set
/// editor is the area right of that panel, and a menu opened from a Sets chip opens inside it.
pub struct Needle {
    pub role: &'static str,
    pub text: &'static str,
    pub right_of_sets: bool,
    pub present: bool,
}

pub const fn has(text: &'static str) -> Needle {
    Needle {
        role: "",
        text,
        right_of_sets: false,
        present: true,
    }
}

pub const fn has_role(role: &'static str, text: &'static str) -> Needle {
    Needle {
        role,
        text,
        right_of_sets: false,
        present: true,
    }
}

const fn has_right_of_sets(text: &'static str) -> Needle {
    Needle {
        role: "",
        text,
        right_of_sets: true,
        present: true,
    }
}

const fn lacks(text: &'static str) -> Needle {
    Needle {
        role: "",
        text,
        right_of_sets: false,
        present: false,
    }
}

/// True when every needle of `marker` holds on `s`.
pub fn holds(s: &Snapshot, marker: &[Needle]) -> bool {
    marker.iter().all(|n| {
        // no Sets column panel in the frame: nothing is right of it
        let left = match (n.right_of_sets, s.sets_right) {
            (false, _) => f32::MIN,
            (true, Some(edge)) => edge,
            (true, None) => f32::MAX,
        };
        let found = s.nodes.iter().any(|x| {
            (n.role.is_empty() || x.role == n.role)
                && x.rect.min.x >= left
                && (x.label == n.text
                    || x.value.as_deref() == Some(n.text)
                    || x.placeholder.as_deref() == Some(n.text))
        }) || (n.role.is_empty()
            && s.texts
                .iter()
                .any(|t| t.text == n.text && t.rect.min.x >= left));
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
    /// A last action `steps` cannot express (a click scoped to a region), run after `steps`.
    pub act: Option<fn(&mut H)>,
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

/// The nodes named `label` that lie wholly right of the Sets column's panel and inside the window (the set editor's),
/// or, with `in_sets`, wholly inside the Sets column's panel.
fn nodes_in<'h>(h: &'h H, label: &'h str, in_sets: bool) -> Vec<egui_kittest::Node<'h>> {
    use egui_kittest::kittest::Queryable as _;
    let sets = sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "sets")
        .expect("the Sets column's panel was never built");
    let window = h.ctx.content_rect();
    h.query_all_by_label(label)
        .filter(|n| {
            let r = n.rect();
            if in_sets {
                sets.contains_rect(r)
            } else {
                r.min.x >= sets.max.x && window.contains_rect(r)
            }
        })
        .collect()
}

/// Right-clicks the widest node named `label` right of the Sets column's panel: the set editor's slot row, which
/// carries the menu (the row's name beside it is not clickable).
fn right_click_in_editor(h: &mut H, label: &str) {
    settle(h, 4);
    let nodes = nodes_in(h, label, false);
    let target = nodes
        .iter()
        .max_by(|a, b| a.rect().width().total_cmp(&b.rect().width()))
        .unwrap_or_else(|| panic!("no {label:?} right of the Sets column's panel"));
    target.click_secondary();
    h.run_steps(2);
}

/// Right-clicks the widest node named `label` inside the Sets column's panel: a set card's chip.
fn right_click_in_sets(h: &mut H, label: &str) {
    settle(h, 4);
    let nodes = nodes_in(h, label, true);
    let target = nodes
        .iter()
        .max_by(|a, b| a.rect().width().total_cmp(&b.rect().width()))
        .unwrap_or_else(|| panic!("no {label:?} inside the Sets column's panel"));
    target.click_secondary();
    h.run_steps(2);
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
        act: None,
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
        act: None,
        marker: &[has("Kinds ▾"), has("← architecture/stone/stonewall")],
        transient: false,
    },
    Screen {
        name: "export to menu",
        fixture: "demo",
        open: nothing,
        steps: &[r#"{"op":"click","at":{"starts":"Export to","role":"Button"}}"#],
        act: None,
        marker: &[has("Re-encode every output")],
        transient: true,
    },
    Screen {
        name: "settings tab",
        fixture: "demo",
        open: |h| model(h).open_settings_tab(),
        steps: &[],
        act: None,
        marker: CLOSE_SETTINGS,
        transient: false,
    },
    Screen {
        name: "tile menu",
        fixture: "demo",
        open: |h| model(h).select_item("architecture/stone/stonewall.png"),
        steps: &[r#"{"op":"right_click","at":{"label":"stonewall.png","index":1}}"#],
        act: None,
        marker: &[has("Copy settings from stonewall.png to 0 selected…")],
        transient: true,
    },
    Screen {
        name: "texture panel",
        fixture: "demo",
        open: |h| model(h).select_item("plants/fern.png"),
        steps: &[],
        act: None,
        marker: &[has("← plants/fern"), has("Alpha threshold")],
        transient: false,
    },
    Screen {
        name: "set card menu",
        fixture: "demo",
        open: |h| select_set(h, "architecture/stone/stonewall"),
        steps: &[r#"{"op":"right_click","at":{"label":"architecture/stone/stonewall","index":0}}"#],
        act: None,
        marker: &[has("Fill from names…")],
        transient: true,
    },
    Screen {
        name: "review tab",
        fixture: "demo",
        open: |h| model(h).open_review(),
        steps: &[],
        act: None,
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
        act: None,
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
        act: None,
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
        act: None,
        marker: &[has("Rename 2 inputs")],
        transient: false,
    },
    Screen {
        name: "marker dialog",
        fixture: "demo",
        open: |h| model(h).open_marker_dialog(),
        steps: &[],
        act: None,
        marker: &[has("F0 = (code/255)^1.6")],
        transient: false,
    },
    Screen {
        name: "set editor",
        fixture: "demo",
        open: |h| select_set(h, "architecture/stone/stonewall"),
        steps: &[],
        act: None,
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
        steps: &[],
        // Right-clicks the RMAOS row right of the Sets column's panel (the editor's), and the marker wants the pin command
        // there too; a menu from a Sets chip opens inside that panel (`the_sets_columns_slot_menu_is_not_the_editors`).
        // Limit: the marker does not carry the slot's number (the editor's menu title is the bare `RMAOS`), so it
        // identifies "the editor's slot menu on cobble", not which slot.
        act: Some(|h| right_click_in_editor(h, "RMAOS")),
        marker: &[
            has("textures/architecture/stone/cobble"),
            has_right_of_sets("Pin this slot (always use this file)"),
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
        act: None,
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
        act: None,
        marker: &[
            has("textures/architecture/stone/cobble"),
            lacks("Skip vanilla/CM for this set"),
            lacks("Pin this slot (always use this file)"),
        ],
        transient: false,
    },
    Screen {
        name: "helper panel",
        fixture: "helpers",
        open: |h| model(h).select_item("props/crate_roughness.png"),
        steps: &[],
        act: None,
        marker: &[has("← props/crate"), has_role("ComboBox", "Helper role")],
        transient: false,
    },
    Screen {
        name: "meshes view",
        fixture: "demo",
        open: meshes_view,
        steps: &[],
        act: None,
        marker: &[has("5 of 5 NIFs")],
        transient: false,
    },
    Screen {
        name: "mo2 popup",
        fixture: "demo",
        open: nothing,
        steps: &[r#"{"op":"click","at":{"label":"MO2…"}}"#],
        act: None,
        marker: &[has("Scan a folder…")],
        transient: true,
    },
    Screen {
        name: "skin card",
        fixture: "demo",
        open: |h| material_tab(h, "actors/body", engine::shader::Shader::AdvancedSkin),
        steps: &[],
        act: None,
        marker: &[has("textures/actors/body"), has("RFAOS, empty")],
        transient: false,
    },
    Screen {
        name: "landscape card",
        fixture: "demo",
        open: |h| material_tab(h, "pbr/metal", engine::shader::Shader::PbrLandscape),
        steps: &[],
        act: None,
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
    // the whole visit is inside the handler: a panic or an idle timeout in the fixture or in `open` is a failure of
    // this screen at this size, not the end of the walk
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| enter_inner(s, size))).unwrap_or_else(
        |e| {
            let why = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            Err(format!("panicked: {why}"))
        },
    )
}

fn enter_inner(s: &Screen, size: Vec2) -> Result<(tempfile::TempDir, H), String> {
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
    let last_step = !s.steps.is_empty() || s.act.is_some();
    if last_step && holds(&snapshot(&h), s.marker) {
        return Err("the marker already holds before the step lines run, so the steps are not what shows it; choose a marker only the last step satisfies".into());
    }
    crate::play(&mut h, &run, s.steps, 0);
    if let Some(act) = s.act {
        act(&mut h);
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

impl ScreenEntry {
    pub fn new(screen: &'static Screen) -> Self {
        ScreenEntry { screen, open: None }
    }
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
        .map(|screen| Box::new(ScreenEntry::new(screen)) as Box<dyn Entry>)
        .collect()
}

/// The start screen has no window under it to compare with in `enter`; its text must not be on a window with a project
/// open.
#[test]
fn the_start_screen_text_is_not_on_an_open_project() {
    let start = SCREENS.iter().find(|s| s.fixture == NO_PROJECT).unwrap();
    let d = tempfile::tempdir().unwrap();
    crate::app_data();
    let (h, _) = fixtures::start("demo", d.path(), false).unwrap();
    assert!(!holds(&snapshot(&h), start.marker));
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
            act: None,
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
            act: None,
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
            act: None,
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

/// The competing menu: a right-click on cobble's RMAOS chip in the Sets column opens a slot menu that has the same pin
/// command, and the editor entry's marker does not hold on that frame.
#[test]
fn the_sets_columns_slot_menu_is_not_the_editors() {
    let editor = SCREENS
        .iter()
        .find(|s| s.name == "set editor slot menu")
        .unwrap();
    crate::app_data();
    let d = tempfile::tempdir().unwrap();
    let (mut h, _run) = fixtures::start("demo", d.path(), false).unwrap();
    select_set(&mut h, "architecture/stone/cobble");
    right_click_in_sets(&mut h, "RMAOS");
    assert!(settle(&mut h, 4));
    let snap = snapshot(&h);
    // the Sets column's menu is open: its title names the slot, and the pin command is in the frame
    assert!(
        holds(&snap, &[has("Slot 6: RMAOS")]),
        "the Sets column's slot menu did not open"
    );
    assert!(
        holds(&snap, &[has("Pin this slot (always use this file)")]),
        "the Sets column's slot menu has no pin command"
    );
    assert!(
        !holds(&snap, editor.marker),
        "the editor's marker holds on the Sets column's menu"
    );
}

/// A frame with the Sets column's panel ending at x = 300, cobble's path in the editor, and the pin command starting at
/// `pin_x`, as a node (`as_node`) or as painted text.
fn pin_frame(pin_x: f32, as_node: bool) -> Snapshot {
    use crate::frame::{Node, Text};
    use eframe::egui::{pos2, Rect};
    let rect = |x: f32| Rect::from_min_size(pos2(x, 100.0), eframe::egui::vec2(200.0, 20.0));
    let node = |label: &str, x: f32| Node {
        parent: None,
        depth: 0,
        role: "Label".into(),
        label: label.into(),
        value: None,
        placeholder: None,
        description: None,
        rect: rect(x),
        clip: None,
        layer: None,
        interactive: false,
        focused: false,
        disabled: false,
        hovered: false,
    };
    let pin = "Pin this slot (always use this file)";
    let mut nodes = vec![node("textures/architecture/stone/cobble", 400.0)];
    let mut texts = vec![];
    if as_node {
        nodes.push(node(pin, pin_x));
    } else {
        texts.push(Text {
            text: pin.into(),
            rect: rect(pin_x),
            clip: rect(pin_x),
            elided: false,
            ends_in_ellipsis: false,
        });
    }
    Snapshot {
        size: vec2_1280(),
        nodes,
        texts,
        sets_right: Some(300.0),
    }
}

/// The editor's marker wants the pin command at or right of the Sets column's edge itself: one starting half a point
/// inside the column, as a node or as painted text, is the Sets column's.
#[test]
fn a_pin_command_inside_the_sets_columns_edge_is_not_the_editors() {
    let editor = SCREENS
        .iter()
        .find(|s| s.name == "set editor slot menu")
        .unwrap();
    for as_node in [true, false] {
        assert!(
            holds(&pin_frame(300.0, as_node), editor.marker),
            "a pin command at the edge is the editor's (as node: {as_node})"
        );
        assert!(
            !holds(&pin_frame(299.5, as_node), editor.marker),
            "a pin command 0.5 pt inside the Sets column is the editor's (as node: {as_node})"
        );
    }
}
