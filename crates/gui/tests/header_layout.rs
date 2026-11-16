//! AG126: the header row never overlaps itself at any window width, and every control stays in the row. Layout v2 A4:
//! one row, Export to ▾ · Save · Export at the right edge; the header is the same in the Meshes view (C7).
mod support;
use eframe::egui::{self, accesskit::Role, containers::panel::PanelState, Id};
use egui_kittest::kittest::{by, NodeT as _, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::meshes::MeshesModel;
use sdc_gui::model::AppModel;
use sdc_gui::settings::MeshesSettings;
use sdc_gui::ui::header::elide_middle;
use sdc_gui::ui::State;
use support::*;

type H = Harness<'static, (AppModel, State, MeshesModel)>;

const WIDTHS: [f32; 4] = [1440.0, 1024.0, 800.0, 640.0];

/// The controls every Textures header must show (plus "Export to: …", matched by its start, and Undo / Redo, found by
/// `header_undo`).
const CONTROLS: [&str; 7] = [
    "Open",
    "MO2…",
    "Preset",
    "Textures view",
    "Meshes view",
    "Settings…",
    "Save",
];

fn harness(m: AppModel, w: f32, meshes_view: bool) -> H {
    harness_sized(m, egui::vec2(w, 800.0), meshes_view)
}

fn harness_sized(m: AppModel, size: egui::Vec2, meshes_view: bool) -> H {
    let mut meshes = MeshesModel::new(None, MeshesSettings::default());
    meshes.procs = sdc_gui::model::meshes::no_procs;
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(size)
        .build_ui_state(
            move |ui, (m, s, me): &mut (AppModel, State, MeshesModel)| {
                sdc_gui::ui::header::set_meshes_view(ui.ctx(), meshes_view);
                if meshes_view {
                    // K6: the app's Meshes branch (K3): the header, the bottom panel, then the view.
                    sdc_gui::ui::header::draw(ui, m, Some(&mut *me));
                    sdc_gui::ui::bottom::draw(ui, m, &mut s.drawer, false);
                    sdc_gui::ui::meshes::show(ui, me);
                } else {
                    sdc_gui::ui::draw_with_scan(ui, m, s, Some(me));
                }
            },
            (m, State::default(), meshes),
        );
    h.run_steps(4);
    h
}

/// The widgets inside the panel `id`, not the containers: (role, label, rect).
fn widgets(h: &H, id: &str) -> Vec<(Role, String, egui::Rect)> {
    let panel = PanelState::load(&h.ctx, Id::unique(id)).unwrap().outer_rect;
    h.query_all(by().predicate(|n| n.bounding_box().is_some()))
        .filter(|n| {
            matches!(
                n.accesskit_node().role(),
                Role::Button | Role::CheckBox | Role::ComboBox | Role::TextInput | Role::Label
            )
        })
        .map(|n| {
            let a = n.accesskit_node();
            (a.role(), a.label().unwrap_or_default(), n.rect())
        })
        .filter(|(_, _, r)| panel.contains_rect(*r))
        .collect()
}

fn assert_no_overlap(h: &H, id: &str, w: f32) {
    let ws = widgets(h, id);
    assert!(!ws.is_empty(), "{id} at {w}: no widgets");
    for (i, a) in ws.iter().enumerate() {
        assert!(
            a.2.right() <= w + 0.5,
            "{id} at {w}: {a:?} runs off the window"
        );
        for b in &ws[i + 1..] {
            let overlap = a.2.shrink(0.5).intersects(b.2.shrink(0.5));
            assert!(!overlap, "{id} at {w}: {a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn the_textures_header_never_overlaps() {
    let d = tempfile::tempdir().unwrap();
    for w in WIDTHS {
        let h = harness(pbr_opened(&d.path().join(format!("{w}"))), w, false);
        assert_no_overlap(&h, "header", w);
        // Note AM: an idle scan row is not drawn (the source moved to MO2…).
        assert!(PanelState::load(&h.ctx, Id::unique("textures-scan")).is_none());
        for c in CONTROLS {
            h.get_by_label(c);
        }
        header_undo(&h, "Undo");
        header_undo(&h, "Redo");
        // Note FB1: Export reads "Review n files" until the project is clean.
        header_export_label(&h);
        h.get_by_label_contains("Export to: ");
        // R14: Messages, Presets…, Recent and PG JSON Editor left the header.
        let header = outer(&h, "header");
        for gone in ["Messages", "Presets…", "Recent", "PG JSON Editor"] {
            assert!(
                !h.query_all_by_label(gone)
                    .any(|n| header.contains_rect(n.rect())),
                "at {w} the header still has {gone}"
            );
        }
        // Note BH7: the JSON name field moved into the editor window.
        assert!(
            !h.query_all_by_role(Role::TextInput)
                .any(|n| header.contains_rect(n.rect())),
            "at {w} the header still has a text field"
        );
    }
}

fn outer(h: &H, id: &str) -> egui::Rect {
    PanelState::load(&h.ctx, Id::unique(id)).unwrap().outer_rect
}

#[test]
fn elide_middle_keeps_the_drive_and_the_tail() {
    let p = r"C:\Users\Modder\AppData\Local\Temp\mods\My Mod";
    assert_eq!(elide_middle(p, 100), p);
    assert_eq!(elide_middle(p, 20), r"C:\…\mods\My Mod");
    assert_eq!(elide_middle(p, 24), r"C:\…\Temp\mods\My Mod");
    assert_eq!(elide_middle(r"C:\a\VeryLongFolderName", 10), "…olderName");
}

/// A4 (K2): one row: every control's rect holds Open's centre line, and they run left to right in the spec's order.
#[test]
fn the_header_is_one_row_in_the_spec_order() {
    let d = tempfile::tempdir().unwrap();
    let h = harness_sized(pbr_opened(d.path()), egui::vec2(1600.0, 1000.0), false);
    assert_one_row_in_the_spec_order(&h);
}

/// Note CD2: the header is one row at 1280 pt (measured after Note FI16 took Mode out and FI5 fixed the Undo slot
/// width). Note JI1's Settings… is narrower than the two column eyes it replaced: one row from 1210 pt (measured in
/// 1 pt steps; 1209 wraps), pinned with 5 pt of slack each side; narrower, Save wraps onto the second line.
#[test]
fn the_header_is_one_row_from_1280() {
    let d = tempfile::tempdir().unwrap();
    for (w, one_row) in [(1280.0, true), (1215.0, true), (1205.0, false)] {
        let h = harness(pbr_opened(&d.path().join(format!("{w}"))), w, false);
        let open = h.get_by_label("Open").rect().center().y;
        let save = h.get_by_label("Save").rect();
        assert_eq!(
            save.min.y <= open && open <= save.max.y,
            one_row,
            "at {w}: Save {save:?}, Open's centre at y {open}"
        );
    }
}

/// The assertions of `the_header_is_one_row_in_the_spec_order`, on a 1600 px harness (P20: shared, not restated).
fn assert_one_row_in_the_spec_order(h: &H) {
    let order = [
        ("Open", h.get_by_label("Open").rect()),
        ("Undo", header_undo(h, "Undo").rect()),
        ("Redo", header_undo(h, "Redo").rect()),
        ("Preset", h.get_by_label("Preset").rect()),
        ("Textures view", h.get_by_label("Textures view").rect()),
        ("Meshes view", h.get_by_label("Meshes view").rect()),
        ("Settings…", h.get_by_label("Settings…").rect()),
        ("Export to", h.get_by_label_contains("Export to: ").rect()),
        ("Save", h.get_by_label("Save").rect()),
        ("Export", h.get_by_label(&header_export_label(h)).rect()),
    ];
    let y = order[0].1.center().y;
    for (name, r) in order {
        assert!(
            r.min.y <= y && y <= r.max.y,
            "{name} {r:?} is off the row at y {y}"
        );
        // K2: the single-height controls share the centre line (the tabs are 6 pt taller, Save/Export two rows).
        if !matches!(name, "Textures view" | "Meshes view" | "Save" | "Export") {
            assert!((r.center().y - y).abs() < 2.0, "{name} {r:?} vs y {y}");
        }
    }
    for pair in order.windows(2) {
        assert!(
            pair[0].1.center().x < pair[1].1.center().x,
            "{} {:?} is not left of {} {:?}",
            pair[0].0,
            pair[0].1,
            pair[1].0,
            pair[1].1
        );
    }
    // Export ends the row at the right edge, only the panel margin past it.
    let export = order[order.len() - 1].1;
    assert!(1600.0 - export.right() <= 16.5, "{export:?}");
}

/// Task 12 (Note CN, owner answer 5): with an "Also export as" tick on, the header is the same one row, and the
/// ticks are not in it (they are Export to ▾ items and a Review section).
#[test]
fn the_header_row_is_unchanged_by_also_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_also_tick(engine::presets::AlsoTarget::Vanilla, true);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    assert!(m.also_on(), "the tick took: {}", m.status_line);
    let h = harness_sized(m, egui::vec2(1600.0, 1000.0), false);
    assert_one_row_in_the_spec_order(&h);
    let header = outer(&h, "header");
    for l in ["Vanilla", "Complex material", "Also export as Vanilla"] {
        assert!(
            !h.query_all_by_label(l)
                .any(|n| header.contains_rect(n.rect())),
            "{l} is in the header"
        );
    }
}

/// A4: Save sits right before Export, as tall as it (two rows).
#[test]
fn save_sits_next_to_export_at_its_height() {
    let d = tempfile::tempdir().unwrap();
    for w in [1600.0, 1024.0] {
        let h = harness(pbr_opened(&d.path().join(format!("{w}"))), w, false);
        let (s, e) = (
            h.get_by_label("Save").rect(),
            h.get_by_label(&header_export_label(&h)).rect(),
        );
        assert!((s.height() - e.height()).abs() < 1.0, "at {w}: {s:?} {e:?}");
        assert!(
            s.right() <= e.left() && e.left() - s.right() < 12.0,
            "at {w}: {s:?} {e:?}"
        );
        assert!(
            (s.center().y - e.center().y).abs() < 1.0,
            "at {w}: {s:?} {e:?}"
        );
        let open = h.get_by_label("Open").rect();
        assert!(
            e.height() > 1.5 * open.height(),
            "at {w}: {e:?} vs {open:?}"
        );
        assert!(e.width() >= 230.0, "at {w}: {e:?}");
    }
}

/// A4, K1, K8: a long export folder is cut in the middle so Export to, Save and Export stay on one row; the hover and
/// the accessible name keep the whole path.
#[test]
fn the_export_folder_shows_its_tail_and_the_whole_path_on_hover() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    // Far longer than the window is wide (300+ characters): uncut, the button would run off it.
    let mut deep = d.path().to_path_buf();
    for i in 0..8 {
        deep = deep.join(format!("a rather long folder name number {i}"));
    }
    let deep = deep.join("mods").join("My Mod");
    choose_export(&mut m, deep.clone());
    let full = deep.display().to_string();
    let mut h = harness(m, 1024.0, false);
    let b = h.get_by_label_contains(r"\mods\My Mod");
    assert_eq!(
        b.accesskit_node().label().unwrap_or_default(),
        format!("Export to: {full}")
    );
    // Cut: the button shares Export's row and ends before Save and Export, inside the window.
    assert!(full.chars().count() > 300);
    let r = b.rect();
    let (save, export) = (
        h.get_by_label("Save").rect(),
        h.get_by_label(&header_export_label(&h)).rect(),
    );
    assert!(
        r.min.y <= export.center().y && export.center().y <= r.max.y,
        "{r:?} {export:?}"
    );
    assert!(
        r.right() <= save.left() && export.right() <= 1024.0,
        "{r:?} {save:?} {export:?}"
    );
    assert!(h.query_by_label(&full).is_none());
    h.get_by_label_contains(r"\mods\My Mod").hover();
    h.run();
    // The tooltip: the whole path, alone.
    h.get_by_label(&full);
}

/// Note FI5: at 1100, 1280 and 1600 px the export chip (`not chosen`, or a long folder cut in the middle) stays on the
/// row, before Save and inside the window, and nothing overlaps (the capture check of the plan's Task 5, as a test).
#[test]
fn the_export_chip_holds_at_three_widths() {
    let d = tempfile::tempdir().unwrap();
    for w in [1100.0, 1280.0, 1600.0] {
        for chosen in [false, true] {
            let dir = d.path().join(format!("{w}-{chosen}"));
            let mut m = opened_unchosen(&dir);
            if chosen {
                let mut deep = dir.clone();
                for i in 0..8 {
                    deep = deep.join(format!("a rather long folder name number {i}"));
                }
                choose_export(&mut m, deep.join("mods").join("My Mod"));
            }
            let h = harness(m, w, false);
            assert_no_overlap(&h, "header", w);
            let chip = h.get_by_label_contains("Export to: ").rect();
            let save = h.get_by_label("Save").rect();
            let export = h.get_by_label(&header_export_label(&h)).rect();
            assert!(
                chip.right() <= save.left() && export.right() <= w,
                "at {w} (chosen {chosen}): {chip:?} {save:?} {export:?}"
            );
            assert!(
                chip.min.y <= export.center().y && export.center().y <= chip.max.y,
                "at {w}: the chip is off Export's row"
            );
        }
    }
}

/// Note FI16 (owner O7): the Mode control left the header for the Presets tab; the Preset menu stays. Plus the plan's
/// capture check as a test: at 1100, 1280 and 1600 px nothing overlaps and MO2… stands in the row.
#[test]
fn the_header_has_no_mode_control() {
    let d = tempfile::tempdir().unwrap();
    for (w, meshes_view) in [
        (1440.0, false),
        (800.0, false),
        (1440.0, true),
        (800.0, true),
        (1100.0, false),
        (1280.0, false),
        (1600.0, false),
    ] {
        let m = pbr_opened(&d.path().join(format!("{w}-{meshes_view}")));
        let h = harness(m, w, meshes_view);
        assert_no_overlap(&h, "header", w);
        let header = outer(&h, "header");
        let inside = |r: egui::Rect| header.contains_rect(r);
        assert!(
            !h.query_all_by_label("Mode").any(|n| inside(n.rect())),
            "at {w}: the header has a Mode label"
        );
        assert!(
            !h.query_all_by_role(Role::ComboBox).any(|n| {
                inside(n.rect())
                    && n.accesskit_node()
                        .value()
                        .or(n.accesskit_node().label())
                        .is_some_and(|v| v == "cs" || v == "vanilla")
            }),
            "at {w}: the header has a Mode combo"
        );
        h.get_by_label("Preset");
        h.get_by_label("MO2…");
    }
}

/// Note JI1: "Settings…" sits where the column eyes were, right after the view tabs, and opens the Settings tab; the
/// eyes are gone from the header (they are the Settings tab's Columns group).
#[test]
fn the_settings_button_sits_after_the_view_tabs() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()), 1440.0, false);
    let header = outer(&h, "header");
    let (meshes, settings) = (
        h.get_by_label("Meshes view").rect(),
        h.get_by_role_and_label(Role::Button, "Settings…").rect(),
    );
    assert!(header.contains_rect(settings), "{settings:?}");
    assert!(
        settings.left() >= meshes.right() && settings.left() - meshes.right() < 24.0,
        "{meshes:?} {settings:?}"
    );
    for gone in ["Textures column", "Sets column"] {
        assert!(
            !h.query_all_by_label(gone)
                .any(|n| header.contains_rect(n.rect())),
            "the header still has {gone}"
        );
    }
    h.get_by_role_and_label(Role::Button, "Settings…").click();
    h.run_steps(3);
    assert_eq!(
        h.state().0.centre_front(),
        sdc_gui::model::CentreTab::Settings
    );
}

/// Note JI1 (survey correction 2): "Settings…" is drawn always: with no folder open and in the Meshes view, where
/// the eyes it replaces were not.
#[test]
fn the_settings_button_shows_with_no_folder_and_in_meshes_view() {
    let none = AppModel::placeholder(sdc_gui::settings::GuiSettings::default());
    let d = tempfile::tempdir().unwrap();
    for (m, meshes_view) in [(none, false), (pbr_opened(d.path()), true)] {
        let mut h = harness(m, 1440.0, meshes_view);
        let header = outer(&h, "header");
        let r = h.get_by_role_and_label(Role::Button, "Settings…").rect();
        assert!(header.contains_rect(r), "meshes {meshes_view}: {r:?}");
        assert!(h.query_by_label("Textures column").is_none());
        // A click puts the tab in front; from the Meshes view it also asks the app to show the centre (Note FB7:
        // `app.rs` takes `reveal_centre` and switches to Textures; the Textures view's draw clears it itself).
        h.get_by_role_and_label(Role::Button, "Settings…").click();
        h.run_steps(3);
        let m = &h.state().0;
        assert_eq!(
            m.centre_front(),
            sdc_gui::model::CentreTab::Settings,
            "meshes {meshes_view}"
        );
        assert_eq!(m.reveal_centre, meshes_view);
    }
}

/// Note JI1: the Preset combo's hover sends a mode change to Settings.
#[test]
fn the_preset_hover_names_settings() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()), 1440.0, false);
    h.query_all_by_role(Role::ComboBox)
        .find(|n| {
            n.accesskit_node()
                .value()
                .or(n.accesskit_node().label())
                .is_some_and(|v| v.starts_with("default ("))
        })
        .expect("the Preset combo")
        .hover();
    h.run();
    h.get_by_label_contains("Change the mode in Settings…");
    assert!(h.query_by_label_contains("in Presets…").is_none());
}

/// Note BH9: a running export shows its progress under the Export button, in the header.
#[test]
fn a_running_export_shows_progress_in_the_header() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()), 1440.0, false);
    let header = outer(&h, "header");
    let bar = |h: &H| {
        h.query_all_by_role(Role::ProgressIndicator)
            .any(|n| header.contains_rect(n.rect()))
    };
    assert!(!bar(&h));
    h.state_mut().0.export.running = Some(Default::default());
    h.run_steps(2);
    assert!(bar(&h));
    h.get_by_label("0/0");
    assert!(h
        .get_by_label(&header_export_label(&h))
        .accesskit_node()
        .is_disabled());
    let export = h.get_by_label(&header_export_label(&h)).rect();
    let p = h.get_by_label("0/0").rect();
    assert!(p.top() >= export.bottom() - 0.5 && (p.left() - export.left()).abs() < 1.0);
}

/// Note BH7, R14: Export to ▾ → PGPatcher JSON editor… opens the editor, whose top field renames the JSON.
#[test]
fn pg_json_editor_opens_the_editor_with_the_name_field() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()), 1440.0, false);
    assert!(h
        .query_by_label(sdc_gui::ui::json_editor::NAME_LABEL)
        .is_none());
    h.get_by_label_contains("Export to: ").click();
    h.run_steps(2);
    h.get_by_label(sdc_gui::ui::header::JSON_EDITOR).click();
    h.run_steps(2);
    assert!(h.state().0.json_editor_open);
    let label = h.get_by_label(sdc_gui::ui::json_editor::NAME_LABEL).rect();
    // The field right of the label on its row.
    h.query_all_by_role(Role::TextInput)
        .filter(|n| (n.rect().center().y - label.center().y).abs() < 6.0)
        .min_by(|a, b| a.rect().left().total_cmp(&b.rect().left()))
        .expect("the JSON name field")
        .click();
    h.run();
    h.event(egui::Event::Text("My Pack".into()));
    h.run();
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    let name = h.state().0.sets_doc.as_ref().unwrap().json_name();
    assert_eq!(name.as_deref(), Some("My Pack"));
    assert!(h.state().0.can_undo());
    h.state_mut().0.undo();
    assert_ne!(
        h.state()
            .0
            .sets_doc
            .as_ref()
            .unwrap()
            .json_name()
            .as_deref(),
        Some("My Pack")
    );
}

/// C7: the Meshes view draws the same header.
#[test]
fn the_meshes_header_never_overlaps() {
    let d = tempfile::tempdir().unwrap();
    for w in WIDTHS {
        let h = harness(pbr_opened(&d.path().join(format!("{w}"))), w, true);
        assert_no_overlap(&h, "header", w);
        for c in ["Open", "MO2…", "Meshes view", "Save"] {
            h.get_by_label(c);
        }
        header_undo(&h, "Undo");
        header_export_label(&h);
        // Note JI1: Settings… shows in the Meshes view too (the eyes it replaced did not).
        assert!(outer(&h, "header").contains_rect(h.get_by_label("Settings…").rect()));
        assert!(PanelState::load(&h.ctx, Id::unique("meshes-header")).is_none());
        // Note AM: an idle bar with no status holds nothing.
        // The bar is not drawn while it has nothing to show (layout v2 ask).
        if PanelState::load(&h.ctx, Id::unique("meshes-bar")).is_some()
            && !widgets(&h, "meshes-bar").is_empty()
        {
            assert_no_overlap(&h, "meshes-bar", w);
        }
    }
}

/// The 3D view's height under this row (at least 72 % of the centre at 1024×768) is checked by
/// `set_preview_layout::in_3d_the_viewport_takes_most_of_the_centre`.
/// Note AO: the set header's input name box and its ▾ never overlap each other or run off the window, and the box keeps
/// at least 160 pt (Note FI17: the box holds the whole path; there is no locked folder label).
#[test]
fn the_set_header_input_name_box_never_overlaps() {
    let d = tempfile::tempdir().unwrap();
    for w in [1024.0, 800.0, 640.0] {
        let mut m = pbr_opened(&d.path().join(format!("{w}")));
        let key = sdc_gui::model::pbr_export::json_entries(&m)
            .into_iter()
            .find(|(v, _)| v.id == "landscape/rock01")
            .unwrap()
            .0
            .key;
        m.select_set(&key);
        let h = harness(m, w, false);
        let edit = h
            .get(
                by().role(Role::TextInput)
                    .label(sdc_gui::ui::input_name::HINT),
            )
            .rect();
        let drop = h.get_by_label(sdc_gui::ui::input_name::DROPDOWN).rect();
        let all = [edit, drop];
        for (i, a) in all.iter().enumerate() {
            assert!(a.right() <= w + 0.5, "at {w}: {a:?} runs off the window");
            for b in &all[i + 1..] {
                assert!(
                    !a.shrink(0.5).intersects(b.shrink(0.5)),
                    "at {w}: {a:?} overlaps {b:?}"
                );
            }
        }
        assert!(edit.width() >= 160.0 - 1.0, "at {w} the box is {edit:?}");
    }
}

/// Note BH: renders the header (with rock01 selected, so the set row shows under it) to `SDC_TOOLBAR_PNG` for the
/// owner. Needs a GPU: `cargo test -p sdc-gui --test it header_layout::render_the_toolbar -- --ignored`.
#[test]
#[ignore]
fn render_the_toolbar() {
    let out = std::env::var("SDC_TOOLBAR_PNG").unwrap_or_else(|_| "toolbar-after.png".into());
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let key = sdc_gui::model::pbr_export::json_entries(&m)
        .into_iter()
        .find(|(v, _)| v.id == "landscape/rock01")
        .unwrap()
        .0
        .key;
    m.select_set(&key);
    let scale = 1.5;
    let mut meshes = MeshesModel::new(None, MeshesSettings::default());
    meshes.procs = sdc_gui::model::meshes::no_procs;
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1440.0, 800.0))
        .with_pixels_per_point(scale)
        .wgpu()
        .build_ui_state(
            |ui, (m, s, me): &mut (AppModel, State, MeshesModel)| {
                sdc_gui::ui::draw_with_scan(ui, m, s, Some(me))
            },
            (m, State::default(), meshes),
        );
    h.run_steps(6);
    let edit = h
        .get(
            by().role(Role::TextInput)
                .label(sdc_gui::ui::input_name::HINT),
        )
        .rect();
    let img = h.render().expect("render");
    let bottom = ((edit.bottom() + 12.0) * scale) as u32;
    let crop =
        image::imageops::crop_imm(&img, 0, 0, img.width(), bottom.min(img.height())).to_image();
    crop.save(&out).expect("save png");
}

/// Layout v2 ask (HANDOFF 2026-10-19): with no scan and nothing to say, the Meshes view draws no empty bar strip.
#[test]
fn meshes_bar_hidden_without_scan() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(pbr_opened(d.path()), 1024.0, true);
    assert!(PanelState::load(&h.ctx, Id::unique("meshes-bar")).is_none());
}
