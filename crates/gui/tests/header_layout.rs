//! AG126: the header row never overlaps itself at any window width, and every control stays in the row. Layout v2 A4:
//! one row, Export to ▾ · Save · Export at the right edge; the header is the same in the Meshes view (C7).
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{by, NodeT as _, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::meshes::MeshesModel;
use sdc_gui::model::AppModel;
use sdc_gui::settings::MeshesSettings;
use sdc_gui::ui::header::{elide_middle, readable_path};
use sdc_gui::ui::State;
use support::*;

type H = Harness<'static, (AppModel, State, MeshesModel)>;

const WIDTHS: [f32; 4] = [1440.0, 1024.0, 800.0, 640.0];

/// Fixed, exactly named controls. The export action, path and history buttons have dynamic names.
const CONTROLS: [(Role, &str); 5] = [
    (Role::Button, "Open"),
    (Role::Button, "Textures view"),
    (Role::Button, "Meshes view"),
    (Role::Button, "Settings…"),
    (Role::Button, "Save"),
];

const MORE: &str = "More header items";
const LONG_DESTINATION: &str = r"C:\Users\runneradmin\AppData\Local\Temp\a rather long folder name number 0\a rather long folder name number 1\mods\Landscape textures at full resolution";

fn reachable_controls(h: &mut H, w: f32) {
    let panel = outer(h, "header");
    for (role, label) in CONTROLS {
        let r = widget(h, role, label).rect();
        assert!(
            panel.contains_rect(r) && r.right() <= w,
            "{label} at {w}: {r:?}"
        );
    }
    let export = h.state().0.export_text(&h.state().0.header_answer());
    for r in [
        header_undo(h, "Undo").rect(),
        header_undo(h, "Redo").rect(),
        h.get_by_label_contains("Export to: ").rect(),
        widget(h, Role::Button, &export).rect(),
    ] {
        assert!(panel.contains_rect(r) && r.right() <= w, "at {w}: {r:?}");
    }
    if w == 640.0 {
        let save = h.get_by_label("Save").rect();
        let chip = h.get_by_label_contains("Export to: ").rect();
        assert!(save.top() > h.get_by_label("Open").rect().bottom());
        assert!(
            chip.y_range().contains(save.center().y),
            "{chip:?} vs {save:?}"
        );
    }
    let mut moved = Vec::new();
    for (role, label) in [
        (Role::Button, "Tools"),
        (Role::Button, "MO2…"),
        (Role::ComboBox, "Preset"),
    ] {
        if let Some(n) = h.query_by_role_and_label(role, label) {
            assert!(
                panel.contains_rect(n.rect()) && n.rect().right() <= w,
                "{label} at {w}"
            );
        } else {
            moved.push(label);
        }
    }
    if !moved.is_empty() {
        let more = h.get_by_label(MORE);
        assert_eq!(
            more.accesskit_node().description().as_deref(),
            Some("Show more header controls")
        );
        assert!(panel.contains_rect(more.rect()) && more.rect().right() <= w);
        more.click();
        h.run_steps(3);
        for label in moved {
            let r = h.get_by_role_and_label(Role::Button, label).rect();
            assert!(r.left() >= 0.0 && r.right() <= w, "{label} at {w}: {r:?}");
        }
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
    }
}

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
    let panel = support::panel(&h.ctx, id);
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
        let mut h = harness(pbr_opened(&d.path().join(format!("{w}"))), w, false);
        assert_no_overlap(&h, "header", w);
        // Note AM: an idle scan row is not drawn (the source moved to MO2…).
        assert!(!panel_never_built(&h.ctx, "header"));
        assert!(panel_never_built(&h.ctx, "textures-scan"));
        reachable_controls(&mut h, w);
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
    support::panel(&h.ctx, id)
}

#[test]
fn elide_middle_keeps_the_drive_and_the_tail() {
    let p = r"C:\Users\Modder\AppData\Local\Temp\mods\My Mod";
    assert_eq!(elide_middle(p, 100), p);
    assert_eq!(elide_middle(p, 20), r"C:\…\mods\My Mod");
    assert_eq!(elide_middle(p, 24), r"C:\…\Temp\mods\My Mod");
    assert_eq!(elide_middle(r"C:\a\VeryLongFolderName", 10), "…olderName");
}

#[test]
fn readable_path_keeps_the_drive_and_the_last_folder() {
    for (path, expected) in [
        (
            r"C:\Users\runneradmin\AppData\Local\Temp\mods\Landscape textures at full resolution",
            r"C:\…\Landscape textures at full resolution",
        ),
        (
            r"C:\Users\Modder\AppData\Local\Temp\mods\My Mod",
            r"C:\…\My Mod",
        ),
        (r"C:\export", r"C:\export"),
        (r"C:\a\b", r"C:\…\b"),
        (r"C:\é\b", r"C:\…\b"),
        (r"C:\ab\b", r"C:\…\b"),
        (r"D:\Mods\StoneWork\sdc-out", r"D:\…\sdc-out"),
        ("C:/Users/Modder/mods/My Mod", "C:/…/My Mod"),
        (
            r"C:\Mods\Landscape textures at full resolution\",
            r"C:\…\Landscape textures at full resolution",
        ),
        (r"C:\", r"C:\"),
    ] {
        let readable = readable_path(path);
        assert_eq!(readable, expected, "{path}");
        assert_eq!(readable_path(&readable), readable, "{path}");
    }
}

/// A4 (K2): one row: every control's rect holds Open's centre line, and they run left to right in the spec's order.
#[test]
fn the_header_is_one_row_in_the_spec_order() {
    let d = tempfile::tempdir().unwrap();
    let h = harness_sized(pbr_opened(d.path()), egui::vec2(1600.0, 1000.0), false);
    assert_one_row_in_the_spec_order(&h);
}

/// Note CD2: the fixed controls stay on one row.
#[test]
fn the_header_is_one_row_from_1280() {
    let d = tempfile::tempdir().unwrap();
    for w in [1280.0, 1215.0, 1055.0] {
        let h = harness(pbr_opened(&d.path().join(format!("{w}"))), w, false);
        let open = h.get_by_label("Open").rect().center().y;
        let save = h.get_by_label("Save").rect();
        assert!(
            save.min.y <= open && open <= save.max.y,
            "at {w}: Save {save:?}, Open's centre at y {open}"
        );
    }
}

#[test]
fn the_header_is_one_row_at_1055_1100_and_1280_in_both_views() {
    for meshes in [false, true] {
        for w in [1055.0, 1100.0, 1280.0] {
            let d = tempfile::tempdir().unwrap();
            let h = harness(opened(d.path()), w, meshes);
            assert_header_row(&h, w, meshes, true);
        }
    }
}

#[test]
fn the_1055_header_fits_long_fields_pending_nif_edits_and_launch() {
    let d = tempfile::tempdir().unwrap();
    for meshes in [false, true] {
        let mut m = opened(d.path());
        let preset = "High resolution landscape and architecture ".repeat(4);
        m.set_preset(Some(preset.clone()));
        choose_export(
            &mut m,
            d.path()
                .join("Long destination for a texture overhaul")
                .join("Landscape textures at full resolution"),
        );
        m.set_nif_review_rows(vec![nif_row(r"meshes\landscape\rock.nif", None)]);
        assert_eq!(m.project_preset(), Some(preset));
        let mut h = harness(m, 1055.0, meshes);
        if meshes {
            let step = "Change the shader type on the mountain cliff shape";
            let target = sdc_gui::ui::meshes::UndoTarget {
                can_undo: true,
                can_redo: false,
                undo_label: Some(step.into()),
                redo_label: None,
            };
            h.ctx
                .data_mut(|d| d.insert_temp(egui::Id::unique("nif-undo-target"), Some(target)));
            h.run_steps(1);
            h.get_by_label(&format!("Undo (NIF edit history): {step}"));
        }
        assert_eq!(h.state().0.nif_review_rows().len(), 1);
        h.get_by_label(MORE);
        assert_header_row(&h, 1055.0, meshes, true);

        let empty = AppModel::new(Default::default(), None, session());
        let h = harness(empty, 1055.0, meshes);
        assert!(h.query_by_label(MORE).is_none());
        assert_header_row(&h, 1055.0, meshes, false);
    }
}

fn assert_header_row(h: &H, w: f32, meshes: bool, project: bool) {
    let panel = outer(h, "header");
    let y = h.get_by_label("Open").rect().center().y;
    assert_eq!(h.state().0.root.is_some(), project);
    let mut rects: Vec<_> = CONTROLS
        .iter()
        .filter(|(_, name)| project || *name != "Save")
        .map(|&(role, name)| (name, widget(h, role, name).rect()))
        .collect();
    if project || (meshes && !h.state().0.nif_review_rows().is_empty()) {
        let export = h.state().0.export_text(&h.state().0.header_answer());
        rects.extend([
            ("Undo", header_undo(h, "Undo").rect()),
            ("Redo", header_undo(h, "Redo").rect()),
            ("Export", widget(h, Role::Button, &export).rect()),
        ]);
    }
    let dest = if project {
        h.get_by_label_contains("Export to: ")
    } else {
        h.get_by_label("More export options")
    };
    rects.push(("Export to", dest.rect()));
    let mut moved = false;
    for (role, name) in [
        (Role::Button, "Tools"),
        (Role::Button, "MO2…"),
        (Role::ComboBox, "Preset"),
    ] {
        if let Some(node) = h.query_by_role_and_label(role, name) {
            rects.push((name, node.rect()));
        } else if project || name != "Tools" {
            moved = true;
        }
    }
    if moved {
        rects.push((MORE, h.get_by_label(MORE).rect()));
    } else {
        assert!(h.query_by_label(MORE).is_none());
    }
    for (_, r) in &rects {
        assert!(
            panel.contains_rect(*r) && r.right() <= w && (r.center().y - y).abs() < 1.0,
            "{w} px, meshes {meshes}, {panel:?}: {rects:?}"
        );
    }
    let rows: std::collections::BTreeSet<_> = widgets(h, "header")
        .iter()
        .map(|(_, _, r)| r.center().y.round() as i32)
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "{w} px, meshes {meshes}: {:?}",
        widgets(h, "header")
    );
    assert_no_overlap(h, "header", w);
}

#[test]
fn long_destination_is_cut_to_its_last_folder_before_an_optional_control_moves() {
    let d = tempfile::tempdir().unwrap();
    let chip_width = |h: &H| h.get_by_label_contains("Export to: ").rect().width();
    let floor = chip_width(&harness_with_destination(
        d.path(),
        r"C:\…\Landscape textures at full resolution",
        2400.0,
        false,
    ));
    let mut h = harness_with_destination(d.path(), LONG_DESTINATION, 2400.0, false);
    let full = chip_width(&h);
    let resize = |h: &mut H, w| {
        h.set_size(egui::vec2(w as f32, 800.0));
        h.run_steps(3);
    };
    let widths = (900..=2400).rev().step_by(20);
    let mut first_more = None;
    let mut smallest_with_optional = full;
    let mut cut_before_move = false;
    let mut cut_after_all_moved = false;
    for w in widths.clone() {
        resize(&mut h, w);
        let chip = chip_width(&h);
        let more = h.query_by_label(MORE).is_some();
        if more {
            first_more.get_or_insert(w);
        } else {
            cut_before_move |= chip < full - 1.0;
        }
        if h.query_by_role_and_label(Role::Button, "MO2…").is_some()
            || h.query_by_role_and_label(Role::ComboBox, "Preset")
                .is_some()
        {
            smallest_with_optional = smallest_with_optional.min(chip);
        } else if more && h.query_by_role_and_label(Role::Button, "Tools").is_none() {
            cut_after_all_moved |= chip < floor - 1.0;
        }
    }

    let first_more = first_more.expect("the sweep reaches overflow");
    let w0 = (first_more + 1..=2400)
        .find(|&w| {
            resize(&mut h, w);
            h.query_by_label(MORE).is_none()
        })
        .expect("the wide header fits all controls");
    let chip = chip_width(&h);
    assert!(
        (floor - 0.5..=floor + 12.0).contains(&chip),
        "at w0 {w0}: chip {chip}, floor {floor}, full {full}"
    );
    resize(&mut h, w0 - 1);
    h.get_by_label(MORE);
    assert!(h.query_by_role_and_label(Role::Button, "Tools").is_none());
    h.get_by_role_and_label(Role::Button, "MO2…");
    h.get_by_role_and_label(Role::ComboBox, "Preset");
    assert!(
        smallest_with_optional >= floor - 0.5,
        "chip {smallest_with_optional}, floor {floor}"
    );
    assert!(cut_before_move, "the path shortened before overflow");
    assert!(
        cut_after_all_moved,
        "the path shrank below its readable form"
    );

    for w in widths.chain([1055, 640]) {
        resize(&mut h, w);
        let open = h.get_by_label("Open").rect();
        let chip = h.get_by_label_contains("Export to: ").rect();
        if w >= 1055 || chip.y_range().contains(open.center().y) {
            assert_header_row(&h, w as f32, false, true);
        } else {
            let save = h.get_by_label("Save").rect();
            let export = h.get_by_label(&header_export_label(&h)).rect();
            let panel = outer(&h, "header");
            assert!(
                [chip, save, export]
                    .into_iter()
                    .all(|r| panel.contains_rect(r) && r.right() <= w as f32),
                "at {w}: wrapped chip {chip:?}, Save {save:?}, Export {export:?}, header {panel:?}"
            );
            for r in [chip, save, export] {
                assert!(
                    r.top() > open.bottom() && (r.center().y - chip.center().y).abs() < 1.0,
                    "at {w}: Open {open:?}, chip {chip:?}, Save {save:?}, Export {export:?}"
                );
            }
            assert_no_overlap(&h, "header", w as f32);
        }
    }
}

fn harness_with_destination(dir: &std::path::Path, path: &str, w: f32, meshes: bool) -> H {
    let mut m = opened(dir);
    m.sets_doc
        .as_mut()
        .unwrap()
        .set_export_folder(path.as_ref());
    harness(m, w, meshes)
}

/// Find the first whole-pixel viewport width at which the reference header fits.
fn first_fit(h: &mut H, fits: impl Fn(&H) -> bool) -> f32 {
    let (mut low, mut high) = (640, 1600);
    h.set_size(egui::vec2(low as f32, 800.0)).run_steps(3);
    assert!(!fits(h), "header already fits at {low}");
    h.set_size(egui::vec2(high as f32, 800.0)).run_steps(3);
    assert!(fits(h), "header still does not fit at {high}");
    while high - low > 1 {
        let mid = (low + high) / 2;
        h.set_size(egui::vec2(mid as f32, 800.0)).run_steps(3);
        if fits(h) {
            high = mid;
        } else {
            low = mid;
        }
    }
    high as f32
}

fn tools_on_row(h: &H) -> bool {
    h.query_by_role_and_label(Role::Button, "Tools").is_some()
}

fn painted_chip(h: &H) -> String {
    let rect = h.get_by_label_contains("Export to: ").rect();
    assert!(!painted_elided(h, rect));
    painted_in(h, rect)
}

#[test]
fn one_letter_folder_keeps_tools_when_the_cut_path_fits() {
    let d = tempfile::tempdir().unwrap();
    let mut reference = harness_with_destination(d.path(), r"C:\…\b", 1600.0, false);
    let w = first_fit(&mut reference, tools_on_row);
    let h = harness_with_destination(d.path(), r"C:\W\b", w, false);
    assert!(tools_on_row(&h), "Tools moved while C:\\…\\b fits at {w}");
    assert_eq!(painted_chip(&h), r"Export to: C:\…\b ▾");
}

#[test]
fn short_leaf_keeps_tools_at_the_drive_and_leaf_boundary() {
    let d = tempfile::tempdir().unwrap();
    let mut reference = harness_with_destination(d.path(), r"C:\…\x", 1600.0, false);
    let w = first_fit(&mut reference, tools_on_row);
    let h = harness_with_destination(d.path(), r"C:\Users\Modder\mods\x", w, false);
    assert!(tools_on_row(&h), "Tools moved while C:\\…\\x fits at {w}");
    assert_eq!(painted_chip(&h), r"Export to: C:\…\x ▾");
    assert_header_row(&h, w, false, true);
}

#[test]
fn short_leaf_wraps_only_when_the_drive_and_leaf_no_longer_fit() {
    let d = tempfile::tempdir().unwrap();
    let mut reference = harness_with_destination(d.path(), r"C:\…\x", 1600.0, false);
    let one_row = |h: &H| {
        h.get_by_label("Save")
            .rect()
            .y_range()
            .contains(h.get_by_label("Open").rect().center().y)
    };
    let w = first_fit(&mut reference, one_row);
    let mut h = harness_with_destination(d.path(), r"C:\Users\Modder\mods\x", w, false);
    assert!(one_row(&h), "export end wrapped while C:\\…\\x fits at {w}");
    assert_eq!(painted_chip(&h), r"Export to: C:\…\x ▾");
    assert_header_row(&h, w, false, true);
    h.set_size(egui::vec2(w - 1.0, 800.0)).run_steps(3);
    assert!(!one_row(&h));
}

#[test]
fn tools_stays_when_the_full_path_is_narrower_than_the_readable_form() {
    let d = tempfile::tempdir().unwrap();
    let full = r"C:\ii\Landscape textures at full resolution";
    let mut reference = harness_with_destination(
        d.path(),
        r"C:\…\Landscape textures at full resolution",
        1600.0,
        false,
    );
    let mut h = harness_with_destination(d.path(), full, 1600.0, false);
    let width = |h: &H| h.get_by_label_contains("Export to: ").rect().width();
    let saved = (width(&reference) - width(&h)).floor();
    assert!(saved >= 1.0, "the full path must save at least one pixel");
    let w = first_fit(&mut reference, tools_on_row) - saved;
    h.set_size(egui::vec2(w, 800.0)).run_steps(3);
    assert!(
        tools_on_row(&h),
        "Tools moved while the full path fits at {w}"
    );
    assert_eq!(painted_chip(&h), format!("Export to: {full} ▾"));
    assert_header_row(&h, w, false, true);
}

#[test]
fn the_first_resize_frame_matches_the_settled_header_in_both_directions() {
    let d = tempfile::tempdir().unwrap();
    for path in [LONG_DESTINATION, r"C:\Users\Modder\mods\x"] {
        let mut h = harness_with_destination(d.path(), path, 1600.0, false);
        let w = first_fit(&mut h, tools_on_row);
        h.set_size(egui::vec2(w, 800.0)).run_steps(3);
        for (width, tools_visible) in [(w - 1.0, false), (w, true)] {
            h.set_size(egui::vec2(width, 800.0)).step();
            let first = (widgets(&h, "header"), painted_chip(&h));
            h.run_steps(3);
            assert_eq!(tools_on_row(&h), tools_visible, "{path} at {width}");
            assert_eq!(
                first,
                (widgets(&h, "header"), painted_chip(&h)),
                "{path} at {width}"
            );
        }
    }
}

#[test]
fn a_cut_destination_trims_the_painted_separator_but_keeps_it_on_hover() {
    let d = tempfile::tempdir().unwrap();
    let path = r"C:\Users\Modder\long parent folder for a texture overhaul\mods\My Mod\";
    let mut h = harness_with_destination(d.path(), path, 1280.0, false);
    let text = painted_chip(&h);
    assert!(text.contains('…') && text.ends_with(r"\My Mod ▾"), "{text}");
    h.get_by_role_and_label(Role::Button, &format!("Export to: {path}"))
        .hover();
    h.run();
    h.get_by_label(path);
}

#[test]
fn a_long_export_path_keeps_tools_mo2_and_preset_at_1440_and_1600() {
    let d = tempfile::tempdir().unwrap();
    let mut path = String::from(r"C:\Users\runneradmin\AppData\Local\Temp\");
    for i in 0..4 {
        path.push_str(&format!(r"a rather long folder name number {i}\"));
    }
    path.push_str("export");
    for (w, meshes) in [
        (1440.0, false),
        (1600.0, false),
        (1600.0, true),
        (1440.0, true),
        (1055.0, false),
        (1055.0, true),
    ] {
        let h = harness_with_destination(d.path(), &path, w, meshes);
        if w >= 1600.0 || (w == 1440.0 && !meshes) {
            for (role, name) in [
                (Role::Button, "Tools"),
                (Role::Button, "MO2…"),
                (Role::ComboBox, "Preset"),
            ] {
                assert!(
                    h.query_by_role_and_label(role, name).is_some(),
                    "{name} missing at {w} px, meshes {meshes}"
                );
            }
            assert!(h.query_by_label(MORE).is_none());
        }
        assert_header_row(&h, w, meshes, true);
        let chip = h.get_by_label(&format!("Export to: {path}")).rect();
        assert!(chip.right() <= h.get_by_label("Save").rect().left());
    }
}

#[test]
fn no_more_button_when_nothing_moved() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(opened(d.path()), 1600.0, false);
    assert_eq!(h.query_all_by_label(MORE).count(), 0);
}

#[test]
fn overflow_moves_tools_first_then_mo2_then_preset() {
    use sdc_gui::ui::header::{overflow, HeaderWidths, Overflow};
    let mut w = HeaderWidths {
        fixed: 900.0,
        tools: 70.0,
        mo2: 60.0,
        preset: 180.0,
        more: 30.0,
    };
    for (avail, tools, mo2, preset) in [
        (1300.0, false, false, false),
        (1210.0, false, false, false),
        (1170.0, true, false, false),
        (1150.0, true, true, false),
        (1000.0, true, true, true),
    ] {
        assert_eq!(
            overflow(avail, &w),
            Overflow { tools, mo2, preset },
            "at {avail}"
        );
    }
    w.tools = 0.0;
    w.mo2 = 0.0;
    assert_eq!(
        overflow(1000.0, &w),
        Overflow {
            tools: false,
            mo2: false,
            preset: true
        }
    );
}

#[test]
fn undo_is_sized_to_its_label_in_the_textures_view() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(opened(d.path()), 1600.0, false);
    let undo = header_undo(&h, "Undo").rect();
    assert!(undo.width() < 80.0, "{}", undo.width());
    assert_eq!(header_undo(&h, "Redo").rect().width(), undo.width());
}

fn open_moved_item(h: &mut H, label: &str) {
    assert!(
        h.query_by_label(MORE).is_some(),
        "{label} needs the overflow menu at 800 px"
    );
    h.get_by_label(MORE).click();
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, label).click();
    h.run_steps(3);
}

#[test]
fn overflow_tools_can_change_reencode() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()), 800.0, false);
    open_moved_item(&mut h, "Tools");
    h.get_by_label("Re-encode every output").click();
    h.run_steps(3);
    assert!(h.state().0.reencode_all);
}

#[test]
fn overflow_mo2_opens_before_an_instance_is_chosen() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()), 800.0, false);
    open_moved_item(&mut h, "MO2…");
    h.get_by_label("Instance: none");
    h.get_by_label("Scan a folder…");
}

#[test]
fn overflow_preset_can_select_an_installed_preset() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()), 800.0, false);
    open_moved_item(&mut h, "Preset");
    pick_installed_preset(&mut h);
}

#[test]
fn picking_the_current_preset_closes_both_pickers() {
    for requested in [false, true] {
        for use_default in [true, false] {
            let d = tempfile::tempdir().unwrap();
            let mut m = opened(d.path());
            let current = if use_default {
                None
            } else {
                Some(m.preset_choices()[0].clone())
            };
            m.set_preset(current.clone());
            let label = current.clone().unwrap_or_else(|| {
                format!(
                    "{} · default",
                    engine::presets::default_preset_for(m.target)
                )
            });
            let mut h = harness(m, 800.0, false);
            if requested {
                h.state_mut().0.choose_preset = Some(h.ctx.cumulative_pass_nr());
                h.run_steps(4);
            } else {
                open_moved_item(&mut h, "Preset");
            }
            let current_row = h.get_by_role_and_label(Role::Button, &label);
            assert_eq!(
                current_row.accesskit_node().toggled(),
                Some(egui::accesskit::Toggled::True),
                "the current preset {label} is selected"
            );
            current_row.click();
            h.run_steps(4);
            assert!(
                h.query_by_role_and_label(Role::Button, &label).is_none(),
                "clicking {label} left the preset picker open, requested={requested}"
            );
            assert_eq!(h.state().0.project_preset(), current);
        }
    }
}

#[test]
fn short_preset_popups_reach_the_last_choice_and_editor() {
    for requested in [false, true] {
        for editor in [false, true] {
            let d = tempfile::tempdir().unwrap();
            let mut m = opened(d.path());
            let dir = d.path().join("presets");
            std::fs::create_dir(&dir).unwrap();
            for n in 0..32 {
                let id = format!("user-{n:02}");
                let base = engine::presets::default_preset_for(m.target);
                std::fs::write(
                    dir.join(format!("{id}.toml")),
                    format!(
                    "schema_version = 1\nid = \"{id}\"\nversion = \"1.0.0\"\nbase = \"{base}\"\n"
                ),
                )
                .unwrap();
            }
            m.user_presets = Some(dir);
            let choices = m.preset_choices();
            assert!(choices.len() >= 32);
            let last = choices.last().unwrap();
            let mut h = harness_sized(m, egui::vec2(800.0, 320.0), false);
            if requested {
                h.state_mut().0.choose_preset = Some(h.ctx.cumulative_pass_nr());
                h.run_steps(4);
            } else {
                open_moved_item(&mut h, "Preset");
            }
            let label = if editor { "Presets…" } else { last };
            h.get_by_role_and_label(Role::Button, &choices[0]).hover();
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -1600.0),
                source: egui::MouseWheelSource::Wheel,
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            });
            h.run_steps(45);
            let row = h.query_by_role_and_label(Role::Button, label);
            assert!(
                row.as_ref()
                    .is_some_and(|r| h.ctx.content_rect().contains_rect(r.rect())),
                "{label} outside the short popup, requested={requested}: {:?}",
                row.map(|r| r.rect())
            );
            h.get_by_role_and_label(Role::Button, label).click();
            h.run_steps(4);
            if editor {
                assert!(h.state().0.presets_open);
            } else {
                assert_eq!(h.state().0.project_preset().as_ref(), Some(last));
            }
        }
    }
}

#[test]
fn keyboard_more_reaches_mo2_and_selects_a_preset() {
    use egui::Key;

    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()), 800.0, false);
    let choice = h.state_mut().0.preset_choices()[0].clone();
    assert_ne!(h.state().0.project_preset().as_ref(), Some(&choice));
    let press = |h: &mut H, key| {
        h.key_press(key);
        h.run_steps(3);
    };
    for _ in 0..12 {
        if h.get_by_label(MORE).is_focused() {
            break;
        }
        press(&mut h, Key::Tab);
    }
    assert!(h.get_by_label(MORE).is_focused());
    press(&mut h, Key::Enter);
    press(&mut h, Key::ArrowDown);
    assert!(h.get_by_label("Tools").is_focused());
    press(&mut h, Key::ArrowDown);
    assert!(h.get_by_label("MO2…").is_focused());
    press(&mut h, Key::ArrowRight);
    // Task 2 (E16) names the disabled Scan row for the missing instance.
    let scan = h.get_by_role_and_label(Role::Button, "Scan: choose an instance first");
    assert!(
        scan.accesskit_node().is_disabled(),
        "Scan is disabled before choosing an instance"
    );
    assert!(h.query_by_role_and_label(Role::Button, "Scan").is_none());
    press(&mut h, Key::ArrowLeft);
    assert!(h.get_by_label("MO2…").is_focused());
    press(&mut h, Key::ArrowDown);
    assert!(h.get_by_label("Preset").is_focused());
    press(&mut h, Key::ArrowRight);
    for _ in 0..3 {
        if h.get_by_role_and_label(Role::Button, &choice).is_focused() {
            break;
        }
        press(&mut h, Key::ArrowDown);
    }
    assert!(h.get_by_role_and_label(Role::Button, &choice).is_focused());
    press(&mut h, Key::Enter);
    assert_eq!(h.state().0.project_preset(), Some(choice));
}

fn pick_installed_preset(h: &mut H) {
    let id = h.state_mut().0.preset_choices()[0].clone();
    h.get_by_role_and_label(Role::Button, &id).click();
    h.run_steps(3);
    assert_eq!(h.state().0.project_preset(), Some(id));
}

#[test]
fn choose_preset_from_review_reaches_the_overflow_presets() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_preset(Some("missing".into()));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.open_review();
    let mut h = harness(m, 800.0, false);
    assert!(h
        .query_by_role_and_label(Role::ComboBox, "Preset")
        .is_none());
    h.get_by_role_and_label(Role::Button, "Choose preset…")
        .click();
    h.run_steps(4);
    pick_installed_preset(&mut h);
    assert!(h.state().0.choose_preset.is_none());
}

/// The assertions of `the_header_is_one_row_in_the_spec_order`, on a 1600 px harness (P20: shared, not restated).
fn assert_one_row_in_the_spec_order(h: &H) {
    let order = [
        ("Open", h.get_by_label("Open").rect()),
        ("Undo", header_undo(h, "Undo").rect()),
        ("Redo", header_undo(h, "Redo").rect()),
        ("Preset", widget(h, Role::ComboBox, "Preset").rect()),
        ("Tools", h.get_by_label("Tools").rect()),
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

/// Note FI16: Mode is in Settings. Preset and MO2… stay reachable at every width.
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
        let mut h = harness(m, w, meshes_view);
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
        reachable_controls(&mut h, w);
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
        // The Meshes header asks the app to show the docked tool. Textures already drew its centre.
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

#[test]
fn the_default_preset_shows_once_with_a_tag() {
    let d = tempfile::tempdir().unwrap();
    let m = support::opened(d.path());
    let def = engine::presets::default_preset_for(m.target);
    let mut h = harness(m, 1600.0, false);
    let tagged = format!("{def} · default");
    h.query_all_by_role(Role::ComboBox)
        .find(|n| n.accesskit_node().value().is_some_and(|v| v == tagged))
        .expect("the closed Preset combo reads the tagged default")
        .click();
    h.run();
    assert_eq!(h.query_all_by_label_contains("default (").count(), 0);
    let rows: Vec<String> = h
        .query_all_by_label_contains(def)
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert_eq!(rows, vec![tagged.clone()], "{rows:?}");
    assert_eq!(
        h.get_by_role_and_label(Role::Button, &tagged)
            .accesskit_node()
            .toggled(),
        Some(egui::accesskit::Toggled::True)
    );
}

#[test]
fn clicking_the_selected_explicit_default_returns_to_following_the_target() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    let def = engine::presets::default_preset_for(m.target);
    m.set_preset(Some(def.into()));
    let mut h = harness(m, 1600.0, false);
    h.get_by_role_and_label(Role::ComboBox, "Preset").click();
    h.run();
    h.query_all_by_label_contains(def)
        .find(|n| n.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True))
        .expect("the project's explicit default is selected")
        .click();
    h.run();
    assert_eq!(h.state().0.project_preset(), None);
}

#[test]
fn presets_opens_on_the_projects_preset() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    m.set_preset(Some("cs-compact".into()));
    m.open_presets();
    assert_eq!(m.presets_model().id(), "cs-compact");
}

#[test]
fn presets_opens_on_the_target_default_without_a_project_pick() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    let def = engine::presets::default_preset_for(m.target);
    m.open_presets();
    assert_eq!(m.presets_model().id(), def);
}

#[test]
fn reopening_presets_keeps_the_editors_draft() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    let target = m.target;
    m.presets_model().new_preset(target);
    m.presets_model().set_id("work-in-progress");
    m.open_presets();
    assert_eq!(m.presets_model().id(), "work-in-progress");
    assert!(m.presets_model().dirty());
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
                .is_some_and(|v| v.ends_with(" · default"))
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

#[test]
fn tools_holds_the_json_editor_marker_and_reencode() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()), 1600.0, false);
    h.get_by_label("Tools").click();
    h.run();
    h.get_by_label("PGPatcher JSON editor…");
    h.get_by_label("Make marker cube…");
    h.get_by_role_and_label(Role::CheckBox, "Re-encode every output")
        .hover();
    h.run();
    h.get_by_label("Encode every output again, even when nothing changed. Files Tessalith has no record of are kept.");
}

#[test]
fn export_to_holds_only_export_things() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened_unchosen(d.path()), 1600.0, false);
    h.get_by_label_contains("Export to: ").click();
    h.run();
    for gone in [
        "Make marker cube…",
        "Re-encode every output",
        "PGPatcher JSON editor…",
    ] {
        assert_eq!(h.query_all_by_label(gone).count(), 0, "{gone}");
    }
    h.get_by_label("Choose export folder…");
    h.get_by_label("Open export folder (none chosen yet)");
}

#[test]
fn tools_explains_why_the_json_editor_is_disabled() {
    let d = tempfile::tempdir().unwrap();
    for (vanilla, reason) in [
        (true, "Switch to Community Shaders mode in Settings."),
        (false, "Load the project's sets document first."),
    ] {
        let mut m = pbr_opened(&d.path().join(vanilla.to_string()));
        if vanilla {
            m.target = engine::config::Target::Vanilla;
        } else {
            m.sets_doc = None;
        }
        let mut h = harness(m, 1600.0, false);
        h.get_by_label("Tools").click();
        h.run();
        let json = h.get_by_label("PGPatcher JSON editor…");
        assert!(json.accesskit_node().is_disabled());
        json.hover();
        h.run();
        h.get_by_label(reason);
    }
}

/// Note BH7, R14: Tools ▾ → PGPatcher JSON editor… opens the editor, whose top field renames the JSON.
#[test]
fn pg_json_editor_opens_the_editor_with_the_name_field() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()), 1440.0, false);
    assert!(h
        .query_by_label(sdc_gui::ui::json_editor::NAME_LABEL)
        .is_none());
    h.get_by_label("Tools").click();
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
        let mut h = harness(pbr_opened(&d.path().join(format!("{w}"))), w, true);
        assert_no_overlap(&h, "header", w);
        reachable_controls(&mut h, w);
        header_undo(&h, "Undo");
        header_export_label(&h);
        // Note JI1: Settings… shows in the Meshes view too (the eyes it replaced did not).
        assert!(outer(&h, "header").contains_rect(h.get_by_label("Settings…").rect()));
        assert!(!panel_never_built(&h.ctx, "header"));
        assert!(panel_never_built(&h.ctx, "meshes-header"));
        // Note AM: an idle bar with no status holds nothing.
        // The bar is not drawn while it has nothing to show (layout v2 ask).
        if sdc_gui::ui::panel_ids::panel_rect(&h.ctx, "meshes-bar").is_some()
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
    assert!(!panel_never_built(&h.ctx, "header"));
    assert!(panel_never_built(&h.ctx, "meshes-bar"));
}

#[test]
fn the_header_without_a_meshes_model_freezes_the_supplied_inventory() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(&d.path().join("mod"));
    m.open_review();
    let row = support::nif_row("meshes/x.nif", None);
    let record = row.record.clone();
    m.set_nif_review_rows(vec![row]);
    let mut h = egui_kittest::Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1280.0, 720.0))
        .build_ui_state(sdc_gui::ui::draw, m);
    h.run_steps(2);
    let captured = h.state().review_writes_at_open();
    assert!(captured.also_updated.iter().any(|w| w.path == record));
    h.state_mut().set_nif_review_rows(vec![]);
    h.run_steps(2);
    assert_eq!(
        h.state().review_writes_at_open(),
        captured,
        "frozen only once per opening"
    );
}
