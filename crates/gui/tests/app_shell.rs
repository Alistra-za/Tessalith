mod app_harness;
#[allow(
    dead_code,
    reason = "this test never reads the doc-shot manifest's shot and exempt lists, the render adapter and fonts, or `Manifest::png`; the doc_shots test does"
)]
#[path = "doc_shots/manifest.rs"]
mod manifest;
mod support;

use eframe::egui::{self, Key, Modifiers, ViewportCommand};
use egui_kittest::kittest::{NodeT, Queryable};
use sdc_gui::{model::AppModel, ui::header};
use std::sync::OnceLock;
use tempfile::TempDir;

const BANNER_OPEN: &str = "Open a texture folder from the Meshes banner";

fn viewport_commands(h: &app_harness::H) -> &[ViewportCommand] {
    &h.output()
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .expect("root viewport output")
        .commands
}

#[test]
fn the_app_asks_for_a_max_size_once_the_gpu_limit_is_known() {
    app_data();
    let m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    let mut h = app_harness::plain_harness(m);
    h.state_mut().model.gpu_max_dim = 4096;
    h.run_steps(1);
    let cap = sdc_gui::layout::surface_cap(4096, h.ctx.pixels_per_point());
    let root = h
        .output()
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .expect("root viewport output");
    assert!(
        root.commands
            .iter()
            .any(|c| matches!(c, egui::ViewportCommand::MaxInnerSize(v) if *v == cap)),
        "{:?}",
        root.commands.len()
    );
}

#[test]
fn the_surface_cap_tracks_a_scale_change() {
    let mut h = app_harness::plain_harness(model());
    h.state_mut().model.gpu_max_dim = 4096;
    h.run_steps(1);
    h.set_pixels_per_point(1.5).run_steps(1);
    assert!(viewport_commands(&h).iter().any(|c| {
        matches!(c, ViewportCommand::MaxInnerSize(size) if *size == egui::Vec2::splat(2730.0))
    }));
}

fn clamp_frame(size: egui::Vec2, limit: u32) -> app_harness::H {
    let mut h = app_harness::plain_harness(model());
    h.set_size(size);
    h.state_mut().model.gpu_max_dim = limit;
    h.run_steps(1);
    h
}

#[test]
fn an_oversized_inner_window_gets_the_surface_size() {
    let h = clamp_frame(egui::vec2(800.0, 1100.0), 1024);
    assert!(viewport_commands(&h).iter().any(|c| {
        matches!(c, ViewportCommand::InnerSize(size) if *size == egui::Vec2::splat(1024.0))
    }));
}

#[test]
fn a_surface_clamp_logs_once_and_sends_no_repeat_commands() {
    let mut h = clamp_frame(egui::vec2(1100.0, 800.0), 1024);
    h.run_steps(1);
    assert_eq!(
        h.state()
            .model
            .messages
            .entries()
            .filter(
                |e| e.text == "Window limited to 1024×1024: the graphics card's largest surface."
            )
            .count(),
        1
    );
    assert!(!viewport_commands(&h).iter().any(|c| matches!(
        c,
        ViewportCommand::MaxInnerSize(_) | ViewportCommand::InnerSize(_)
    )));
}

#[test]
fn a_fitting_window_keeps_its_inner_size_without_a_message() {
    let h = clamp_frame(egui::vec2(1024.0, 800.0), 1024);
    assert!(!viewport_commands(&h)
        .iter()
        .any(|c| matches!(c, ViewportCommand::InnerSize(_))));
    assert!(!h
        .state()
        .model
        .messages
        .entries()
        .any(|e| e.text.starts_with("Window limited to ")));
}

#[test]
fn an_unknown_surface_limit_sends_no_size_command() {
    let h = clamp_frame(egui::vec2(1100.0, 800.0), 0);
    assert!(!viewport_commands(&h).iter().any(|c| matches!(
        c,
        ViewportCommand::MaxInnerSize(_) | ViewportCommand::InnerSize(_)
    )));
}

#[test]
fn starting_without_a_render_state_leaves_the_surface_limit_unknown() {
    let app = sdc_gui::app::SdcApp::new(model()).with_render_state(None);
    assert_eq!(app.model.gpu_max_dim, 0);
}

fn app_data() {
    static DATA: OnceLock<TempDir> = OnceLock::new();
    DATA.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        app_harness::redirect_app_data(dir.path());
        dir
    });
}

fn model() -> AppModel {
    app_data();
    AppModel::new(
        support::settings_with_meshes_view(),
        None,
        support::session(),
    )
}

#[test]
fn settings_opens_in_the_meshes_view_and_back_returns() {
    app_data();
    let d = tempfile::tempdir().unwrap();
    let mut h = app_harness::plain_harness(support::opened(d.path()));
    header::set_meshes_view(&h.ctx, true);
    app_harness::settle(&mut h, 3);
    h.get_by_label("Settings…").click();
    app_harness::settle(&mut h, 3);
    assert!(header::meshes_view_active(&h.ctx), "the view stays Meshes");
    h.get_by_label("Settings tab");
    h.get_by_label("Close Settings");
    h.get_by_label_contains("Back to meshes").click();
    app_harness::settle(&mut h, 3);
    assert!(!h.state().model.centre_over_meshes);
    h.get_by_label("NIFs");
}

#[test]
fn settings_over_meshes_with_no_project_keeps_the_banner() {
    app_data();
    let m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    let mut h = app_harness::plain_harness(m);
    header::set_meshes_view(&h.ctx, true);
    app_harness::settle(&mut h, 3);
    h.get_by_label("Settings…").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label_contains("No project open. This list");
    h.get_by_label_contains("Back to meshes");
    h.get_by_label_contains("App settings: these apply to every project");
    assert!(h.query_by_label("Export without review").is_none());
    assert!(header::meshes_view_active(&h.ctx));
    h.get_by_label_contains("Back to meshes").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label_contains("No project open.");
    assert_eq!(h.query_all_by_label_contains("Back to meshes").count(), 0);
}

#[test]
fn settings_and_other_docked_tools_yield_to_meshes_or_view_navigation() {
    use sdc_gui::ui::json_editor::WINDOW_TITLE;

    app_data();
    let d = tempfile::tempdir().unwrap();
    let mut h = app_harness::plain_harness(support::opened(d.path()));
    h.get_by_label("Meshes view").click();
    app_harness::settle(&mut h, 3);
    h.state_mut().model.open_presets();
    app_harness::settle(&mut h, 3);
    assert!(
        h.state().model.centre_over_meshes,
        "Presets opens over Meshes"
    );
    h.get_by_label("Close Presets");
    h.state_mut().model.open_json_editor();
    app_harness::settle(&mut h, 3);
    h.get_by_label("Presets tab");
    h.get_by_label(&format!("Pop out {WINDOW_TITLE}")).click();
    app_harness::settle(&mut h, 3);
    assert!(h.state().model.json_editor_popped);
    assert!(!h.state().model.centre_over_meshes);
    h.get_by_label("NIFs");
    h.get_by_label(&format!("Dock {WINDOW_TITLE}")).click();
    app_harness::settle(&mut h, 3);
    assert!(h.state().model.centre_over_meshes);
    h.get_by_label(&format!("Close {WINDOW_TITLE}")).click();
    app_harness::settle(&mut h, 3);
    h.state_mut().model.open_presets();
    app_harness::settle(&mut h, 3);
    h.get_by_label("Close Presets").click();
    app_harness::settle(&mut h, 3);
    assert!(!h.state().model.centre_over_meshes);
    h.get_by_label("NIFs");
    h.get_by_label("Settings…").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("Textures view").click();
    app_harness::settle(&mut h, 3);
    assert!(!h.state().model.centre_over_meshes);
    assert!(!header::meshes_view_active(&h.ctx));
    h.get_by_label("Close Settings");
    h.get_by_label("Meshes view").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("NIFs");
    h.get_by_label("Settings…").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("Meshes view").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("NIFs");
    assert!(!h.state().model.centre_over_meshes);
    h.get_by_label("Settings…").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("Selection tab").click();
    app_harness::settle(&mut h, 3);
    assert!(!header::meshes_view_active(&h.ctx));
    assert!(!h.state().model.centre_over_meshes);
}

#[test]
fn a_view_click_commits_the_overlay_json_name_once() {
    use eframe::egui::{Event, PointerButton};

    app_data();
    for tab in ["Meshes view", "Textures view"] {
        let d = tempfile::tempdir().unwrap();
        let mut h = app_harness::plain_harness(support::pbr_opened(d.path()));
        header::set_meshes_view(&h.ctx, true);
        h.state_mut().model.open_json_editor();
        app_harness::settle(&mut h, 3);
        h.get_by_label("Back to meshes");
        let steps = h.state().model.history.undo.len();
        h.get_by_label("JSON file name").click();
        h.run_steps(2);
        h.event(Event::Text("Overlay pack".into()));
        h.run_steps(2);
        assert_eq!(h.state().model.history.undo.len(), steps);
        let pos = h.get_by_label(tab).rect().center();
        h.input_mut().events.push(Event::PointerMoved(pos));
        for pressed in [true, false] {
            h.input_mut().events.push(Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        app_harness::settle(&mut h, 3);
        let m = &h.state().model;
        assert_eq!(
            m.sets_doc.as_ref().unwrap().json_name().as_deref(),
            Some("Overlay pack"),
            "{tab} commits the focused JSON name"
        );
        assert_eq!(m.history.undo.len(), steps + 1);
        assert!(!m.centre_over_meshes);
        assert_eq!(header::meshes_view_active(&h.ctx), tab == "Meshes view");
    }
}

#[test]
fn json_show_commits_the_focused_name_before_selecting_the_set() {
    use sdc_gui::model::{pbr_export::written_json_entries, CentreTab, SetTarget};
    use sdc_gui::sets_doc::SetsDoc;

    app_data();
    for meshes in [true, false] {
        let d = tempfile::tempdir().unwrap();
        let mut m = support::pbr_opened(d.path());
        let target = SetTarget::view(&written_json_entries(&m)[0].0).selection();
        m.select_item("landscape/rock01.png");
        let mut h = app_harness::plain_harness_no_mo2(m);
        header::set_meshes_view(&h.ctx, meshes);
        h.state_mut().model.open_json_editor();
        app_harness::settle(&mut h, 3);
        assert_eq!(h.state().model.centre_over_meshes, meshes);
        assert_ne!(h.state().model.selection, target);
        let steps = h.state().model.history.undo.len();
        h.get_by_label("JSON file name").click();
        h.run_steps(2);
        h.event(eframe::egui::Event::Text("Shown pack".into()));
        h.run_steps(2);
        assert!(h.get_by_label("JSON file name").is_focused());
        assert_eq!(h.state().model.history.undo.len(), steps);
        h.get_by_label("Show").click_accesskit();
        app_harness::settle(&mut h, 3);
        let m = &mut h.state_mut().model;
        m.save().unwrap();
        assert_eq!(
            SetsDoc::open(m.root.as_deref().unwrap())
                .json_name()
                .as_deref(),
            Some("Shown pack"),
            "Show commits the focused JSON name"
        );
        assert_eq!(m.history.undo.len(), steps + 1);
        assert_eq!(m.selection, target);
        assert_eq!(m.centre_front(), CentreTab::Selection);
        assert!(!header::meshes_view_active(&h.ctx));
    }
}

fn show_action(m: &AppModel, id: &str) -> sdc_gui::model::messages::EntryAction {
    sdc_gui::model::messages::EntryAction::ShowSet {
        root: m.root.clone().unwrap(),
        id: id.into(),
    }
}

/// Task 7: a Messages row's Show, clicked over the Meshes view, goes back to Textures and shows the card.
#[test]
fn a_show_button_reveals_its_set_from_meshes() {
    use sdc_gui::model::messages::Severity;
    app_data();
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    m.set_field(
        "landscape/rock01.png",
        "subtype",
        serde_json::json!("diffuse"),
    );
    // Cards ahead of rock01: it starts below the Sets column's view.
    m.edit_docs("t", None, true, |_, s| {
        for i in 0..40 {
            s.create_set(&format!("aaa/pad{i:02}"));
        }
    });
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    let rock = "landscape/rock01";
    let target = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.id == rock)
        .map(sdc_gui::model::SetTarget::view)
        .expect("fixture has a rock01 card");
    let action = show_action(&m, rock);
    m.messages.log_with(Severity::Info, "from the log", action);
    let mut h = app_harness::plain_harness(m);
    h.get_by_label("Meshes view").click();
    app_harness::settle(&mut h, 3);
    assert!(header::meshes_view_active(&h.ctx));
    h.get_by_label("NIFs");
    let tab = sdc_gui::ui::messages::button_text(&h.state().model);
    h.get_by_label(&tab).click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("from the log");
    let in_view = |h: &app_harness::H| {
        h.query_all_by_label_contains("Set landscape/rock01, position ")
            .any(|n| support::in_sets_column(h, n.rect()))
    };
    // Back in Textures the card would be off-screen: the click must reveal it, not just select it.
    h.state_mut().model.centre_over_meshes = false;
    header::set_meshes_view(&h.ctx, false);
    app_harness::settle(&mut h, 3);
    assert!(!in_view(&h), "the card starts below the Sets column's view");
    h.get_by_label("Meshes view").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label(&format!("Show {rock}")).click();
    app_harness::settle(&mut h, 3);
    assert!(!header::meshes_view_active(&h.ctx));
    assert!(!h.state().model.centre_over_meshes);
    assert_eq!(h.state().model.selection, target.selection());
    let card: Vec<_> = h
        .query_all_by_label_contains("Set landscape/rock01, position ")
        .map(|n| n.rect())
        .collect();
    assert!(
        card.iter().any(|r| support::in_sets_column(&h, *r)),
        "the card is in the Sets column's view: {card:?}"
    );
}

/// Task 7: a row's Show belongs to the project it was logged in; another project's card of the same ID is not it.
#[test]
fn a_show_button_is_bound_to_its_project() {
    use sdc_gui::model::messages::Severity;
    app_data();
    let wait = std::time::Duration::from_secs(60);
    let (da, db) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // A has enough cards before `arch/wall` that it starts below the Sets column's view.
    let wall = |dir: &std::path::Path, pads: usize| {
        let mut m = support::opened(dir);
        m.edit_docs("t", None, true, |_, s| {
            for i in 0..pads {
                s.create_set(&format!("aaa/pad{i:02}"));
            }
            s.create_set("arch/wall");
        });
        assert!(m.wait_idle(wait));
        m.save().unwrap();
        m.root.clone().unwrap()
    };
    let (root_a, root_b) = (wall(da.path(), 40), wall(db.path(), 0));
    // The oracle is the fixture's own ID, not the resolver's output: a document-only card is selected by its ID.
    let wall_sel = || sdc_gui::model::Selection::SetId("arch/wall".into());
    let in_view = |h: &app_harness::H| {
        h.query_all_by_label_contains("Set arch/wall, position ")
            .any(|n| support::in_sets_column(h, n.rect()))
    };
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    m.open_folder(root_a.clone());
    assert!(m.wait_idle(wait));
    let action = show_action(&m, "arch/wall");
    m.messages
        .log_with(Severity::Info, "logged in A", action.clone());
    let mut h = app_harness::plain_harness(m);
    support::instant_tips(&h.ctx);
    // A's export folder changes under the row: the root is the same project, the card still resolves.
    h.state_mut()
        .model
        .set_export_folder(da.path().join("elsewhere"));
    app_harness::settle(&mut h, 3);
    let tab = sdc_gui::ui::messages::button_text(&h.state().model);
    h.get_by_label(&tab).click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("logged in A");
    assert!(!in_view(&h), "the card starts below the Sets column's view");
    h.get_by_label("Show arch/wall").click();
    app_harness::settle(&mut h, 3);
    assert_eq!(h.state().model.selection, wall_sel());
    // The column took the reveal (`nav.reveal_set` is consumed by its frame) and scrolled the card into view.
    assert!(h.state().model.nav.reveal_set.is_none());
    assert!(in_view(&h), "the revealed card is in the Sets column");
    // Project B has a card of the same ID.
    h.state_mut().model.open_folder(root_b);
    app_harness::settle(&mut h, 3);
    assert!(sdc_gui::model::sets::views(&h.state().model)
        .iter()
        .any(|v| v.id == "arch/wall"));
    assert_eq!(h.state().model.logged_set_target(&action), None);
    h.get_by_label("logged in A");
    let show = h.get_by_label("Show arch/wall");
    assert!(show.accesskit_node().is_disabled());
    show.hover();
    h.run();
    h.get_by_label("That set is no longer listed");
    let (selection, reveal) = (
        h.state().model.selection.clone(),
        h.state().model.nav.reveal_set.clone(),
    );
    h.get_by_label("Show arch/wall").click();
    app_harness::settle(&mut h, 3);
    assert_eq!(h.state().model.selection, selection);
    assert_eq!(h.state().model.nav.reveal_set, reveal);
    // A again, with the card: the link is back.
    h.state_mut().model.open_folder(root_a);
    app_harness::settle(&mut h, 3);
    assert!(!h
        .get_by_label("Show arch/wall")
        .accesskit_node()
        .is_disabled());
    h.state_mut().model.selection = sdc_gui::model::Selection::None;
    app_harness::settle(&mut h, 3);
    assert!(!in_view(&h), "reopened A starts at the top again");
    h.get_by_label("Show arch/wall").click();
    app_harness::settle(&mut h, 3);
    assert_eq!(h.state().model.selection, wall_sel());
    assert!(in_view(&h), "the link is back: the card is revealed");
}

#[test]
fn invalid_header_mo2_pick_shows_the_error_above_the_open_tool() {
    app_data();
    let d = tempfile::tempdir().unwrap();
    let mut h = app_harness::plain_harness_no_mo2(model());
    header::set_meshes_view(&h.ctx, true);
    h.state_mut().model.open_settings_tab();
    app_harness::settle(&mut h, 3);
    let invalid = d.path().join("missing-instance");
    let (backend, requests) = support::recording_picker(Some(invalid.clone()));
    h.state().meshes.picker.set_backend(backend);
    h.get_by_label("MO2…").click();
    h.run_steps(3);
    h.get_by_label("Choose…").click();
    h.run_steps(2);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state().meshes.error.is_none() {
        assert!(
            std::time::Instant::now() < deadline,
            "the MO2 pick did not return an error"
        );
        app_harness::settle(&mut h, 1);
    }
    assert_eq!(requests.lock().unwrap().len(), 1);
    h.key_press(Key::Escape);
    h.run_steps(3);
    let error = h.state().meshes.error.as_deref().unwrap();
    assert!(error.contains(invalid.to_str().unwrap()));
    let back = h.get_by_label("Back to meshes").rect();
    let message = h
        .query_by_label(error)
        .expect("the MO2 error must be visible over Settings");
    assert!(message.rect().bottom() <= back.top());
    h.get_by_label("Dismiss").click();
    h.run_steps(3);
    assert!(h.state().meshes.error.is_none());
    h.get_by_label("Back to meshes");
}

fn no_edit_controls(h: &app_harness::H) {
    for label in ["Undo", "Redo", "Save", "Files", "Problems", "Messages"] {
        assert!(
            h.query_all_by_label_contains(label).next().is_none(),
            "{label} is visible"
        );
    }
    assert_eq!(support::header_export_action(h), None);
}

#[test]
fn no_more_button_with_no_project() {
    let mut h = app_harness::plain_harness(model());
    h.set_size(eframe::egui::vec2(1100.0, 800.0));
    app_harness::settle(&mut h, 3);
    h.get_by_label("Settings…");
    assert_eq!(h.query_all_by_label("More header items").count(), 0);
}

#[test]
fn tools_is_hidden_with_no_project() {
    let mut h = app_harness::plain_harness(model());
    app_harness::settle(&mut h, 3);
    h.get_by_label("Settings…");
    assert_eq!(h.query_all_by_label("Tools").count(), 0);
    h.get_by_label("More export options").click();
    h.run_steps(3);
    h.get_by_label("Make marker cube…").click();
    h.run_steps(3);
    assert_eq!(
        h.state().model.dialog,
        Some(sdc_gui::model::Dialog::MakeMarker)
    );
    h.get_by_label("Marker colour");
    h.get_by_label("Save…");
}

#[test]
fn a_launch_with_no_project_shows_the_start_card_even_after_meshes() {
    let mut m = model();
    let dir = tempfile::tempdir().unwrap();
    m.settings.recent.push(dir.path().to_owned());
    let h = app_harness::restored_harness(m);
    assert!(
        !header::meshes_view_active(&h.ctx),
        "the first frame should show Textures"
    );
    for label in [
        "Open",
        "Open folder…",
        "Recent",
        "Settings…",
        "Textures view",
        "Meshes view",
    ] {
        h.get_by_label(label);
    }
    h.get_by_label(dir.path().display().to_string().as_str());
    no_edit_controls(&h);
}

#[test]
fn reopening_a_project_restores_the_saved_meshes_view() {
    let mut m = model();
    let dir = tempfile::tempdir().unwrap();
    m.open_folder(dir.path().to_owned());
    let h = app_harness::restored_harness(m);
    assert!(header::meshes_view_active(&h.ctx));
    h.get_by_label("NIFs");
    h.get_by_label("Save");
}

#[test]
fn the_meshes_view_with_no_project_keeps_its_list_and_a_banner() {
    use sdc_gui::model::picker::{PickerBackend, PickerSpec};
    struct Folder(std::path::PathBuf);
    impl PickerBackend for Folder {
        fn pick(&self, _: &PickerSpec) -> Option<std::path::PathBuf> {
            Some(self.0.clone())
        }
    }
    let mut h = app_harness::plain_harness(model());
    h.get_by_label("Meshes view").click();
    app_harness::settle(&mut h, 3);
    h.get_by_label("NIFs");
    h.get_by_label(
        "No project open. This list is your MO2 instance; open a folder to edit its textures.",
    );
    no_edit_controls(&h);
    h.get_by_label("Open").click();
    h.run_steps(3);
    assert_eq!(
        h.query_all_by_label("Open folder…").count(),
        1,
        "only the open menu item should have the plain name"
    );
    h.get_by_label(BANNER_OPEN);
    h.key_press(Key::Escape);
    h.run_steps(2);
    let dir = tempfile::tempdir().unwrap();
    h.state()
        .model
        .picker
        .set_backend(std::sync::Arc::new(Folder(dir.path().to_owned())));
    h.get_by_label(BANNER_OPEN).click();
    h.run_steps(2);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state().model.picker.busy() {
        assert!(
            std::time::Instant::now() < deadline,
            "folder picker did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(app_harness::settle(&mut h, 3));
    assert_eq!(h.state().model.root.as_deref(), Some(dir.path()));
    assert!(h.query_by_label("Open folder…").is_none());
    assert!(h.query_by_label(BANNER_OPEN).is_none());
}

#[test]
fn selecting_a_nif_without_editing_it_keeps_edit_controls_hidden() {
    let mut h = app_harness::plain_harness(model());
    header::set_meshes_view(&h.ctx, true);
    h.state_mut().meshes.selected_nif = Some(1);
    h.run_steps(2);
    assert!(h.state().meshes.selected_nif.is_some());
    assert!(h.state().model.nif_review_rows().is_empty());
    no_edit_controls(&h);
}

#[test]
fn a_no_project_files_tab_stays_hidden_even_when_saved_open() {
    let mut m = model();
    m.show_tab(sdc_gui::model::bottom::BottomTab::Files);
    let h = app_harness::plain_harness(m);
    assert!(
        h.query_by_label("Files").is_none(),
        "the Files tab is visible"
    );
    assert!(h.query_by_label_contains("Open export folder").is_none());
}

const MESSAGES_REASON: &str = "Open a texture folder or edit a NIF in Meshes to see Messages.";

#[test]
fn a_hidden_messages_tab_stops_reading_new_entries() {
    let mut m = model();
    m.show_tab(sdc_gui::model::bottom::BottomTab::Messages);
    assert!(m.messages.is_open());
    let mut h = app_harness::plain_harness(m);
    assert!(h.query_by_label("Messages").is_none());
    let m = &mut h.state_mut().model;
    let unread = m.messages.unread();
    m.messages
        .log(sdc_gui::model::messages::Severity::Warning, "New entry");
    assert_eq!(m.messages.unread(), unread + 1);
}

fn start_card_with_unread_message() -> app_harness::H {
    let mut h = app_harness::restored_harness(model());
    h.state_mut().model.messages.clear();
    h.state_mut().model.messages.log(
        sdc_gui::model::messages::Severity::Warning,
        "Folder could not be opened",
    );
    h
}

#[test]
fn hidden_messages_shortcut_keeps_the_entry_unread() {
    let mut h = start_card_with_unread_message();
    h.key_press_modifiers(Modifiers::COMMAND, Key::M);
    h.run_steps(2);
    let m = &h.state().model;
    assert_eq!(m.messages.unread(), 1, "Ctrl+M read an unseen entry");
    assert!(!m.messages.is_open() && !m.bottom.open);
    h.get_by_label(MESSAGES_REASON);
}

#[test]
fn hidden_messages_palette_click_keeps_the_entry_unread() {
    let mut h = start_card_with_unread_message();
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    h.run_steps(2);
    h.state_mut().model.nav.palette.as_mut().unwrap().query = "Messages".into();
    h.run_steps(2);
    let row = "Messages   Ctrl+M";
    h.get_by_label(row).click();
    h.run_steps(2);
    assert_eq!(
        h.state().model.messages.unread(),
        1,
        "the palette read an unseen entry"
    );
    assert!(h.get_by_label(row).accesskit_node().is_disabled());
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.get_by_label(row).hover();
    h.run_steps(4);
    h.get_by_label(MESSAGES_REASON);
    h.key_press(Key::Enter);
    h.run_steps(2);
    let m = &h.state().model;
    assert_eq!(m.messages.unread(), 1);
    assert!(!m.messages.is_open() && !m.bottom.open);
    assert!(m.nav.palette.is_some());
}
