//! E28 (egui 0.37): every input widget has an accessible name. egui_kittest's own check runs at every `run()` and
//! sees only the input roles on screen, so these tests open the screens other tests may not reach, check the one role
//! it skips (`ProgressIndicator`), and pin the short names: a hover sentence would otherwise become the name.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::cs_pbr::Kind;
use sdc_gui::model::meshes::{edit_doc::EDIT_MOD, MeshesModel};
use sdc_gui::model::rename::RenameMode;
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use sdc_gui::settings::MeshesSettings;
use support::*;

fn harness(m: AppModel) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 1000.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m)
}

/// Every progress node (a spinner, a bar) with no name: its value and rect.
fn unnamed_progress<S>(h: &Harness<'_, S>) -> Vec<String> {
    h.query_all_by_role(Role::ProgressIndicator)
        .map(|n| n.accesskit_node())
        .filter(|n| n.label().is_none_or(|l| l.trim().is_empty()))
        .map(|n| format!("{:?} {:?}", n.value(), n.raw_bounds()))
        .collect()
}

/// Input nodes on screen as (role, name, hover-only): visible, editable, named. An input linked with `labelled_by`
/// lands here under its row label's name: egui 0.37's `labelled_by` clears the node's own label, and AccessKit
/// resolves the name through the link.
fn named_inputs<S>(h: &Harness<'_, S>) -> Vec<(Role, String, bool)> {
    h.root()
        .children_recursive()
        .map(|n| n.accesskit_node())
        .filter(|n| egui::accessibility::is_input(n.role()) && !n.is_hidden())
        .filter(|n| !n.data().is_read_only())
        .filter_map(|n| {
            let name = n.label().filter(|l| !l.trim().is_empty())?;
            // The name egui 0.37 copies from `on_hover_text` into a node with none: label equals description (the
            // predicate of `support::shown_containing`).
            let hover_only =
                n.labelled_by().next().is_none() && n.description() == Some(name.clone());
            Some((n.role(), name.to_string(), hover_only))
        })
        .collect()
}

/// True when the inputs with these roles sharing one name are a fault. Two exceptions: a Slider and its own value box
/// (egui gives the box the slider's name), and splitters alone (egui gives every panel splitter the same fixed name,
/// which the app cannot set).
fn shared_name_is_a_fault(roles: &[Role]) -> bool {
    let slider_and_box =
        roles.len() == 2 && roles.contains(&Role::Slider) && roles.contains(&Role::SpinButton);
    let splitters = roles.iter().all(|r| *r == Role::Splitter);
    roles.len() > 1 && !slider_and_box && !splitters
}

/// The two name faults on a screen: the names of inputs named only by their hover text, and the names shared by two or
/// more inputs (with their roles), less the two exceptions of `shared_name_is_a_fault`.
fn name_faults<S>(h: &Harness<'_, S>) -> (Vec<String>, Vec<(String, Vec<Role>)>) {
    let inputs = named_inputs(h);
    let mut hover: Vec<String> = Vec::new();
    for (_, name, hover_only) in &inputs {
        if *hover_only && !hover.contains(name) {
            hover.push(name.clone());
        }
    }
    let mut dup: Vec<(String, Vec<Role>)> = Vec::new();
    for (_, name, _) in &inputs {
        if dup.iter().any(|(n, _)| n == name) {
            continue;
        }
        let roles: Vec<Role> = inputs
            .iter()
            .filter(|(_, n, _)| n == name)
            .map(|(r, _, _)| *r)
            .collect();
        if shared_name_is_a_fault(&roles) {
            dup.push((name.clone(), roles));
        }
    }
    (hover, dup)
}

/// The screen names `audit_names` accepts. An audit call or an allowlist entry that names a screen outside this list
/// fails; a name that stays here after its audit call is deleted is not caught.
const SCREENS: &[&str] = &[
    "textures view with filters",
    "set editor",
    "json editor",
    "presets",
    "rename dialog",
    "meshes view",
    "meshes mod filter open",
    "meshes skin browse popup",
    "meshes edit browse popup",
];

/// Rules 8.8 and 8.4 over everything on `screen` (a checkpoint's name, one of `SCREENS`), with today's hits allowed:
/// no input is named only by its hover text, and no two inputs share a name. Each allowlist below holds one entry per
/// (screen, name) with the reason; a shared-name entry also pins the roles of the inputs sharing the name, so an input
/// added, gone or changed fails here, and so does an entry whose hit is gone (delete it).
#[track_caller]
fn audit_names<S>(h: &Harness<'_, S>, screen: &str) {
    assert!(
        SCREENS.contains(&screen),
        "{screen:?} is not in SCREENS in crates/gui/tests/accessible_names.rs; add it there"
    );
    let (hover, dup) = name_faults(h);
    let mut bad: Vec<String> = Vec::new();
    for name in &hover {
        if !HOVER_ONLY_ALLOWED
            .iter()
            .any(|(s, n, _)| *s == screen && n == name)
        {
            bad.push(format!(
                "{screen}: an input is named only by its hover text {name:?}; give it a short name with `ui::name_input` (rule 8.8). HOVER_ONLY_ALLOWED in crates/gui/tests/accessible_names.rs lists the allowed ones"
            ));
        }
    }
    for (name, roles) in &dup {
        let mut measured: Vec<String> = roles.iter().map(|r| format!("{r:?}")).collect();
        measured.sort();
        match SHARED_NAME_ALLOWED
            .iter()
            .find(|(s, n, _, _)| *s == screen && n == name)
        {
            None => bad.push(format!(
                "{screen}: {} inputs share the name {name:?} ({measured:?}); give each its own name (rule 8.4)",
                roles.len()
            )),
            Some((_, _, stored, _)) => {
                let mut stored = stored.to_vec();
                stored.sort();
                if stored[..] != measured[..] {
                    bad.push(format!(
                        "{screen}: the inputs sharing the name {name:?} changed: SHARED_NAME_ALLOWED in crates/gui/tests/accessible_names.rs has {stored:?}, measured {measured:?}; name the new input apart (rule 8.4), or update the entry's roles with its reason"
                    ));
                }
            }
        }
    }
    for (s, n, _) in HOVER_ONLY_ALLOWED {
        if *s == screen && !hover.iter().any(|h| h == n) {
            bad.push(format!(
                "{screen}: the hover-only name {n:?} allowed in HOVER_ONLY_ALLOWED in crates/gui/tests/accessible_names.rs is gone; delete its entry"
            ));
        }
    }
    for (s, n, _, _) in SHARED_NAME_ALLOWED {
        if *s == screen && !dup.iter().any(|(d, _)| d == n) {
            bad.push(format!(
                "{screen}: the shared name {n:?} allowed in SHARED_NAME_ALLOWED in crates/gui/tests/accessible_names.rs is gone; delete its entry"
            ));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// Inputs named only by their hover text today: none.
const HOVER_ONLY_ALLOWED: &[(&str, &str, &str)] = &[];

const COLLAPSE: &str =
    "column-collapse small_button: the glyph is the name, the hover names the column";
const CHIP: &str =
    "a member chip with no docked helper is named by its slot letter alone (chip_name)";
const GROUP_ROW: &str =
    "a group header named after its main row, whose control carries the same name";
const SUBTYPE_ROWS: &str = "two open sub-type rows draw the same tree with fixed labels";

/// Names two or more inputs share today: one entry per (screen, name), with the Debug names of the roles sharing it
/// (sorted) and the reason. R5: allowed until the names are fixed (the sets branch is changing the set editor's names).
const SHARED_NAME_ALLOWED: &[(&str, &str, &[&str], &str)] = &[
    (
        "textures view with filters",
        "◀",
        &["Button", "Button"],
        COLLAPSE,
    ),
    (
        "textures view with filters",
        "D",
        &["Button", "Button", "Button"],
        CHIP,
    ),
    (
        "textures view with filters",
        "Alpha",
        &["ComboBox", "DisclosureTriangle"],
        GROUP_ROW,
    ),
    (
        "textures view with filters",
        "Mipmaps",
        &["CheckBox", "DisclosureTriangle"],
        GROUP_ROW,
    ),
    ("set editor", "◀", &["Button", "Button"], COLLAPSE),
    ("set editor", "D", &["Button", "Button", "Button"], CHIP),
    (
        "set editor",
        "Match on this",
        &["Button", "Button"],
        "one fixed-name flag per filled slot row",
    ),
    (
        "set editor",
        "Assign…",
        &["Button", "Button", "Button"],
        "one fixed-name menu button per empty slot row",
    ),
    (
        "set editor",
        "Roughness scale",
        &["CheckBox", "SpinButton"],
        "CS PBR's texture-set tick and PGPatcher's number box, one key label",
    ),
    (
        "set editor",
        "Specular level",
        &["CheckBox", "SpinButton"],
        "CS PBR's texture-set tick and PGPatcher's number box, one key label",
    ),
    ("json editor", "◀", &["Button", "Button"], COLLAPSE),
    ("json editor", "D", &["Button", "Button", "Button"], CHIP),
    (
        "json editor",
        "×",
        &["Button", "Button"],
        "remove_button: the glyph is the name, the hover says which key",
    ),
    (
        "presets",
        "Suffixes",
        &["DisclosureTriangle", "DisclosureTriangle"],
        SUBTYPE_ROWS,
    ),
    (
        "presets",
        "New alias _",
        &["TextInput", "TextInput"],
        SUBTYPE_ROWS,
    ),
    ("presets", "Add alias", &["Button", "Button"], SUBTYPE_ROWS),
    (
        "presets",
        "Compression",
        &["DisclosureTriangle", "DisclosureTriangle"],
        SUBTYPE_ROWS,
    ),
    ("presets", "Format", &["ComboBox", "ComboBox"], SUBTYPE_ROWS),
    ("presets", "Effort", &["ComboBox", "ComboBox"], SUBTYPE_ROWS),
    (
        "presets",
        "Perceptual",
        &["CheckBox", "CheckBox"],
        SUBTYPE_ROWS,
    ),
    (
        "presets",
        "Resize & mips",
        &["DisclosureTriangle", "DisclosureTriangle"],
        SUBTYPE_ROWS,
    ),
    ("presets", "Resize", &["ComboBox", "ComboBox"], SUBTYPE_ROWS),
    (
        "presets",
        "Relative to",
        &["ComboBox", "ComboBox"],
        SUBTYPE_ROWS,
    ),
    (
        "presets",
        "Size floor",
        &["ComboBox", "ComboBox"],
        SUBTYPE_ROWS,
    ),
    (
        "presets",
        "Mip levels",
        &["ComboBox", "ComboBox"],
        SUBTYPE_ROWS,
    ),
    (
        "presets",
        "Mip filter",
        &["ComboBox", "ComboBox"],
        SUBTYPE_ROWS,
    ),
    ("presets", "Policy", &["ComboBox", "ComboBox"], SUBTYPE_ROWS),
    (
        "presets",
        "Alpha",
        &["DisclosureTriangle", "DisclosureTriangle"],
        SUBTYPE_ROWS,
    ),
    ("rename dialog", "◀", &["Button", "Button"], COLLAPSE),
    (
        "rename dialog",
        "⚠ 2",
        &["Button", "Button"],
        "each set card's warning-count button is named by its text (`sets_column.rs` warn_button), and two cards have two warnings",
    ),
    ("rename dialog", "D", &["Button", "Button", "Button"], CHIP),
    (
        "rename dialog",
        "Alpha",
        &["ComboBox", "DisclosureTriangle"],
        GROUP_ROW,
    ),
    (
        "rename dialog",
        "Mipmaps",
        &["CheckBox", "DisclosureTriangle"],
        GROUP_ROW,
    ),
    (
        "meshes skin browse popup",
        "textures/white.dds",
        &["Link", "Link"],
        "each shape card names a texture slot by its path (`meshes.rs` shape), and the test's skin shape is a copy of the fixture's shape 0, so both show its textures/white.dds",
    ),
];

/// The two checks see what they stand for: an icon button named only by its hover, two boxes sharing a name, and a
/// Slider with its own value box (which is allowed).
#[test]
fn the_name_checks_see_a_hover_only_name_and_a_shared_name() {
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(600.0, 300.0))
        .build_ui_state(
            |ui, st: &mut (String, String, f32)| {
                ui.add(egui::Button::new(""))
                    .on_hover_text("Close the window");
                ui.text_edit_singleline(&mut st.0).accessible_name("Same");
                ui.text_edit_singleline(&mut st.1).accessible_name("Same");
                ui.add(egui::Slider::new(&mut st.2, 0.0..=1.0).text("Gain"));
            },
            (String::new(), String::new(), 0.5),
        );
    h.run();
    let (hover, dup) = name_faults(&h);
    assert_eq!(hover, vec!["Close the window".to_string()]);
    assert_eq!(dup.len(), 1, "{dup:?}");
    assert_eq!(dup[0].0, "Same");
}

/// The two exceptions are exact: splitters alone and a Slider with its value box pass; a splitter or a slider that
/// shares its name with another input still fails.
#[test]
fn a_shared_name_is_a_fault_unless_splitters_alone_or_a_slider_and_its_box() {
    use Role::*;
    assert!(!shared_name_is_a_fault(&[Splitter, Splitter]));
    assert!(!shared_name_is_a_fault(&[Splitter, Splitter, Splitter]));
    assert!(!shared_name_is_a_fault(&[Slider, SpinButton]));
    assert!(!shared_name_is_a_fault(&[Button]));
    assert!(shared_name_is_a_fault(&[Splitter, Button]));
    assert!(shared_name_is_a_fault(&[Slider, SpinButton, SpinButton]));
    assert!(shared_name_is_a_fault(&[Button, Button]));
}

/// Every allowlist entry names a screen in `SCREENS`, so an entry for a screen outside it (a typo) fails here. Whether
/// an audit call still names that screen is not checked.
#[test]
fn every_allowlist_entry_names_an_audited_screen() {
    let screens = HOVER_ONLY_ALLOWED
        .iter()
        .map(|(s, _, _)| *s)
        .chain(SHARED_NAME_ALLOWED.iter().map(|(s, _, _, _)| *s));
    let unknown: Vec<&str> = screens.filter(|s| !SCREENS.contains(s)).collect();
    assert!(
        unknown.is_empty(),
        "allowlist entries in crates/gui/tests/accessible_names.rs name a screen not in SCREENS (a typo, or a screen no longer audited; fix or delete the entry): {unknown:?}"
    );
}

/// Opens the Textures column's Filter ▾ popup.
fn open_filters(h: &mut Harness<'static, AppModel>) {
    h.query_all_by_label("Filter ▾")
        .find(|n| n.accesskit_node().role() == Role::Button)
        .expect("the Filter button")
        .click();
    h.run();
    h.run();
}

#[test]
fn every_input_has_a_name_in_the_textures_view() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.select_item("landscape/rock01.png");
    let mut h = harness(m);
    h.run();
    open_filters(&mut h);
    h.get_by_role_and_label(Role::TextInput, "Search textures");
    h.get_by_role_and_label(Role::ComboBox, "Sort order");
    audit_names(&h, "textures view with filters");
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
}

#[test]
fn every_input_has_a_name_in_the_set_editor_and_json_editor() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let s = m.sets.iter().find(|s| s.key.ends_with("rock01")).unwrap();
    let key = s.key.clone();
    let id = m.set_of_item_probe(&s.members[0]).unwrap().id;
    m.select_set(&key);
    // The CS texture set's value shows its number box beside the tick.
    m.add_cs_block(&id, Kind::Txst);
    m.set_cs_value(
        &id,
        Kind::Txst,
        "specularLevel",
        Some(serde_json::json!(0.02)),
    );
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(Role::ComboBox, "Packed texture recipe");
    h.get_by_role_and_label(Role::TextInput, "EDID to add");
    // Rule 8.4: a number box beside a named tick is "{label} value", never the tick's own name.
    h.get_by_role_and_label(Role::SpinButton, "Specular level value");
    audit_names(&h, "set editor");
    // The JSON editor is a centre tab: it takes the set editor's place.
    h.state_mut().open_json_editor();
    h.run();
    h.get_by_label(sdc_gui::ui::json_editor::NAME_LABEL);
    h.get_by_role_and_label(Role::TextInput, "Search sets and paths");
    audit_names(&h, "json editor");
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
}

#[test]
fn every_input_has_a_name_in_presets_and_rename() {
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_presets();
    m.presets_model().select("cs-quality");
    m.presets_model().duplicate();
    let mut h = harness(m);
    h.run();
    h.get_by_label("diffuse — BC7 · linear").click();
    h.run();
    h.get_by_label_contains("rmaos —").click();
    h.run();
    h.get_by_label("TOML preview").click();
    h.run();
    // Rule 8.4: the header's "Preset" and the tab's own combos have distinct names.
    for n in [
        "Preset",
        "Preset to edit",
        "Preset target",
        "Base preset",
        "Red source",
    ] {
        h.get_by_role_and_label(Role::ComboBox, n);
    }
    h.get_by_role_and_label(Role::TextInput, "Preset name");
    h.get_by_role_and_label(Role::MultilineTextInput, "TOML preview");
    h.get_by_role_and_label(Role::SpinButton, "AO fill value");
    audit_names(&h, "presets");

    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.click_item("landscape/leaf.png", false, false);
    m.open_rename(RenameMode::Inputs);
    assert!(m.dialog.is_some(), "the rename dialog is open");
    let mut h = harness(m);
    h.run();
    h.get_by_role_and_label(Role::CheckBox, "Rename landscape/leaf.png");
    // Tied to the visible labels (`labelled_by`), so no hover sentence takes their place.
    for n in ["Find", "Replace", "Prefix", "Suffix"] {
        h.get_by_role_and_label(Role::TextInput, n);
    }
    h.get_by_role_and_label(Role::ComboBox, "Letter case");
    audit_names(&h, "rename dialog");
}

/// The Files tab's bar while an export runs (`drawer.rs`): a `ProgressIndicator`, which kittest's check does not
/// cover. A held `running` stands in for the export, as in `also_export.rs`.
#[test]
fn an_export_in_progress_names_its_bar() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.export.running = Some(Default::default());
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1600.0, 600.0))
        .build_ui_state(
            |ui, m: &mut AppModel| {
                let mut st = sdc_gui::ui::drawer::DrawerState::default();
                sdc_gui::ui::drawer::files_tab(ui, m, &mut st);
            },
            m,
        );
    // The bar asks for repaints while the export runs: steps, not `run()`.
    h.run_steps(3);
    h.get_by_role_and_label(Role::ProgressIndicator, "Export progress");
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
    h.state_mut().export.running = None;
}

/// The 3D view's busy spinner (survey row 48): a `ProgressIndicator`, which kittest's check does not cover.
#[test]
fn a_busy_3d_view_names_its_spinner() {
    use sdc_gui::model::viewport3d::{self as v3, Viewport3d};
    let mut v = Viewport3d::new();
    let _cmds = v.attach_test_thread();
    v.set_mesh(Some(v3::MeshSpec {
        key: 1,
        nif: v3::FileSource::Loose(std::path::PathBuf::from("C:/nope/1.nif")),
        textures: Default::default(),
        le_mesh: false,
        head_part: false,
        txst: Default::default(),
    }));
    v.request_frame(64, 64);
    assert!(v.busy());
    // `show_spinner`'s first call records the busy start (and says no), so the toolbar's own call below is well past
    // the spinner's delay.
    let long_ago = std::time::Instant::now() - 2 * v3::SPINNER_DELAY;
    assert!(!v.show_spinner(long_ago));
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(800.0, 100.0))
        .build_ui_state(
            |ui, v: &mut Viewport3d| {
                ui.horizontal(|ui| sdc_gui::ui::viewport3d::toolbar(ui, v));
            },
            v,
        );
    // The spinner never goes idle: steps, not `run()`.
    h.run_steps(2);
    h.get_by_role_and_label(Role::ProgressIndicator, "Working");
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
}

#[test]
fn the_textures_filters_have_short_names() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    open_filters(&mut h);
    for n in [
        "Sub-type filter",
        "Output format filter",
        "Set membership filter",
        "Sort order",
    ] {
        h.get_by_role_and_label(Role::ComboBox, n);
    }
}

#[test]
fn the_header_preset_and_the_search_boxes_are_named() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    h.get_by_role_and_label(Role::ComboBox, "Preset");
    h.get_by_role_and_label(Role::TextInput, "Search textures");
}

type MeshesHarness = Harness<'static, MeshesModel>;

/// A Meshes model over a one-mod MO2 instance holding the Skinned SE fixture, with the scan requested and started.
fn scan_started() -> (tempfile::TempDir, MeshesModel) {
    let d = tempfile::tempdir().unwrap();
    let r = d.path();
    let game = r.join("Game");
    std::fs::create_dir_all(game.join("Data")).unwrap();
    std::fs::write(
        r.join("ModOrganizer.ini"),
        format!(
            "[General]\ngamePath=@ByteArray({})\nselected_profile=@ByteArray(Default)\n",
            game.display().to_string().replace('\\', "\\\\")
        ),
    )
    .unwrap();
    std::fs::create_dir_all(r.join("overwrite")).unwrap();
    std::fs::create_dir_all(r.join("profiles/Default")).unwrap();
    std::fs::write(r.join("profiles/Default/modlist.txt"), "+A\n").unwrap();
    std::fs::write(
        r.join("profiles/Default/settings.ini"),
        "[General]\nLocalSettings=true\n",
    )
    .unwrap();
    let nif = std::fs::read(format!(
        "{}/../nif/tests/fixtures/TestNifFile_Skinned_SE.nif",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    std::fs::create_dir_all(r.join("mods/A/meshes")).unwrap();
    std::fs::write(r.join("mods/A/meshes/x.nif"), nif).unwrap();
    std::fs::create_dir_all(r.join("mods/A/textures")).unwrap();
    std::fs::write(r.join("mods/A/textures/new.dds"), b"dds").unwrap();
    let mut m = MeshesModel::new(Some(r.join("scan.db")), MeshesSettings::default());
    m.procs = sdc_gui::model::meshes::no_procs;
    m.activate_with(None);
    m.use_instance(r.to_path_buf(), "Default".into());
    m.request_scan();
    assert!(m.take_request());
    m.start();
    (d, m)
}

/// `scan_started`, run to its end, with A's x.nif selected.
fn scanned_meshes() -> (tempfile::TempDir, MeshesModel) {
    let (d, mut m) = scan_started();
    let t = std::time::Instant::now();
    m.poll();
    while m.scanning() {
        assert!(t.elapsed() < std::time::Duration::from_secs(60));
        std::thread::sleep(std::time::Duration::from_millis(10));
        m.poll();
    }
    assert_eq!(m.error, None);
    let a = m.mods.iter().find(|r| r.name == "A").unwrap().id;
    m.select_mod(a);
    let c = m.nifs[0].copy_id;
    m.select_nif(c);
    m.set_nif_export_root(Some(d.path().join("mods").join(EDIT_MOD)));
    (d, m)
}

fn meshes_harness(m: MeshesModel) -> MeshesHarness {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1400.0, 900.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::show(ui, m),
            m,
        )
}

/// Clicks the first button whose name contains `part` and lets the popup draw.
fn open_browse(h: &mut MeshesHarness, part: &str) {
    h.query_all_by_label_contains(part)
        .find(|n| n.accesskit_node().role() == Role::Button)
        .unwrap_or_else(|| panic!("a button named {part:?}"))
        .click();
    h.run();
    h.run();
}

#[test]
fn every_input_has_a_name_in_the_meshes_view() {
    let (_d, m) = scanned_meshes();
    let mut h = meshes_harness(m);
    h.run();
    audit_names(&h, "meshes view");
    // The mod filter, and the filter box inside its open list.
    h.get_by_role_and_label(Role::ComboBox, "Mod filter")
        .click();
    h.run();
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Filter mods by name");
    audit_names(&h, "meshes mod filter open");
    h.key_press(egui::Key::Escape);
    h.run();
    // A skin-tint shape for the Advanced Skin card, whose Browse popup has its own filter.
    let m = h.state_mut();
    m.shapes_col = true;
    let mut skin = m.shapes[0].clone();
    skin.block = 900;
    skin.index3d = Some(900);
    skin.shader_kind = 1;
    skin.shader_type = Some(5);
    skin.name = Some("Body".into());
    m.shapes.push(skin);
    h.run();
    open_browse(&mut h, "Browse for the ");
    h.get_by_role_and_label(Role::TextInput, "Filter textures");
    audit_names(&h, "meshes skin browse popup");
    h.key_press(egui::Key::Escape);
    h.run();
    // Edit swaps the list for the edit model's cards and the edit panel; a slot's Browse has its own filter.
    h.state_mut().open_edit();
    h.run();
    h.get_by_label_contains("Select shape #0").click();
    h.run();
    h.run();
    open_browse(&mut h, "Browse for ");
    h.get_by_role_and_label(Role::TextInput, "Filter textures");
    audit_names(&h, "meshes edit browse popup");
}

#[test]
fn a_running_scan_names_its_spinner() {
    // The scan is started and never polled, so it runs while the controls draw.
    let (_d, m) = scan_started();
    assert!(m.scanning());
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(800.0, 100.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| sdc_gui::ui::meshes::scan_controls(ui, m),
            m,
        );
    // The spinner never goes idle: steps, not `run()`.
    h.run_steps(3);
    h.get_by_role_and_label(Role::ProgressIndicator, "Scanning mods");
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
    // Once the scan reports progress, its bar draws beside the spinner, named by its text (as a poll would set it).
    h.state_mut().progress = Some(("Reading", 3, 10));
    h.run_steps(2);
    assert!(
        h.query_all_by_role(Role::ProgressIndicator).any(|n| n
            .accesskit_node()
            .label()
            .is_some_and(|l| l.starts_with("Reading 3/10"))),
        "the scan's bar is drawn"
    );
    assert!(
        unnamed_progress(&h).is_empty(),
        "{:?}",
        unnamed_progress(&h)
    );
}

#[test]
fn the_meshes_search_is_named() {
    let (_d, m) = scanned_meshes();
    let mut h = meshes_harness(m);
    h.run();
    h.get_by_role_and_label(Role::TextInput, "Search paths and mods");
}
