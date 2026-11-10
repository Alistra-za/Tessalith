mod support;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::{AppModel, Selection};
use sdc_gui::settings::GuiSettings;
use support::*;

/// A harness over `model`. The crate bundles no fonts, so the harness needs the system fonts, as the app does.
fn harness(model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

#[test]
fn the_start_screen_offers_open_folder_recent_and_the_drop_hint() {
    let s = GuiSettings {
        recent: vec![std::path::PathBuf::from(r"C:\mods\One")],
        ..Default::default()
    };
    let mut h = harness(AppModel::placeholder(s));
    h.run();
    h.get_by_label("Open folder…");
    h.get_by_label(r"C:\mods\One");
    h.get_by_label("Or drag a folder onto the window.");
    h.get_by_label_contains("Open a folder");
    assert!(h.query_by_label("Advanced").is_none());
    assert!(
        h.query_all_by_role(eframe::egui::accesskit::Role::TextInput)
            .next()
            .is_none(),
        "no Sets search box"
    );
    assert!(
        h.query_all_by_label("Textures").next().is_none(),
        "no column heading or eye"
    );
    assert!(
        h.query_all_by_label("Sets").next().is_none(),
        "no Sets column"
    );
    h.get_by_label("Save");
    header_export_label(&h);
}

#[test]
fn a_recent_folder_on_the_start_screen_opens_it() {
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    let s = GuiSettings {
        recent: vec![root.clone()],
        ..Default::default()
    };
    let mut h = harness(AppModel::new(s, None, session()));
    h.run();
    h.get_by_label(&root.display().to_string()).click();
    h.run();
    assert_eq!(h.state().root.as_deref(), Some(root.as_path()));
}

#[test]
fn a_cancelled_first_open_shows_its_note_on_the_start_screen() {
    let d = tempfile::tempdir().unwrap();
    let mut m = AppModel::new(GuiSettings::default(), None, session());
    m.open_folder(mod_folder(d.path()));
    m.cancel_open();
    let mut h = harness(m);
    h.run();
    h.get_by_label("Opening cancelled");
    h.get_by_label("Open folder…");
}

#[test]
fn an_opened_folder_shows_items_filters_and_the_undecided_count() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    for label in ["leaf.png", "rock01_normal.png", "Undecided", "Not in a set"] {
        h.get_by_label_contains(label);
    }
    // The packed output's tile alone: the two raw maps' helper rows say "fills RMAOS R" without the file (Note HP).
    // (Note HN: the Sets column's RMAOS chip is named for its file too, so the count leaves that column out.)
    let in_grid = h
        .query_all_by_label_contains("rock01_rmaos")
        .filter(|n| !in_sets_column(&h, n.rect()))
        .count();
    assert_eq!(in_grid, 1);
    assert_eq!(h.query_all_by_label_contains("fills RMAOS R").count(), 2);
    h.get_by_label_contains(" undecided");
    // A set is named by its path under the export folder.
    h.get_by_label("landscape/rock01");
}

#[test]
fn collapsing_the_columns_hides_the_thumbnails() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.settings.textures_collapsed = true;
    model.settings.sets_collapsed = true;
    let mut h = harness(model);
    h.run();
    assert!(h.query_by_label_contains("leaf.png").is_none());
}

#[test]
fn clicking_a_tile_selects_it_and_the_needs_decision_filter_narrows_the_grid() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    h.get_by_label_contains("rock01_normal.png").click();
    h.run();
    assert!(
        matches!(&h.state().selection, Selection::Item(k) if k.ends_with("rock01_normal.png")),
        "selection was {:?}",
        h.state().selection
    );
    let pill = format!("Undecided {}", h.state().filter_counts().needs_decision);
    h.get_by_label(&pill).click();
    h.run();
    assert!(h.state().filter.needs_decision);
    h.get_by_label_contains("leaf.png");
    // The helper's badge still sits on the set card (Note HN); the grid no longer shows it.
    assert_eq!(
        h.query_all_by_label_contains("rock01_roughness.png")
            .filter(|n| !in_sets_column(&h, n.rect()))
            .count(),
        0
    );
}

#[test]
fn closing_with_unsaved_edits_asks_and_discard_drops_them() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.set_target(engine::config::Target::Vanilla);
    assert!(model.is_dirty());
    assert!(!model.request_close());
    let mut h = harness(model);
    h.run();
    h.get_by_label("Cancel");
    // egui hit-tests against the previous frame: let the pointer settle on the button before the click.
    h.get_by_label("Don't save").hover();
    h.run();
    h.get_by_label("Don't save").click();
    h.run();
    assert!(!h.state().is_dirty());
    assert!(h.state().dialog.is_none());
}

#[test]
fn the_columns_keep_their_width_through_a_collapse_and_expand() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    // The Sets column starts where the Textures column ends.
    let before = h.get_by_label("Sets").rect().min.x;
    assert!(before > 300.0, "Sets starts at {before}");
    h.state_mut().settings.textures_collapsed = true;
    h.state_mut().settings.sets_collapsed = true;
    h.run();
    assert!(h
        .query_by_label("Sets")
        .is_some_and(|n| n.rect().width() < 30.0));
    h.state_mut().settings.textures_collapsed = false;
    h.state_mut().settings.sets_collapsed = false;
    h.run();
    let after = h.get_by_label("Sets").rect().min.x;
    assert!(after > 300.0, "Sets starts at {after} after expanding");
}

#[test]
fn selecting_a_texture_shows_its_decisions_and_groups() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.select_item("landscape/rock01_normal.png");
    // Note FB8: the Info line shows its message; the code is on its hover.
    let unstated = model
        .item("landscape/rock01_normal.png")
        .unwrap()
        .warnings
        .iter()
        .find(|w| w.code == engine::codes::Code::InGreenUnstated)
        .expect("the unstated-green question")
        .message
        .clone();
    let mut h = harness(model);
    h.run();
    assert_eq!(
        h.query_all_by_label_contains("IN-GREEN-UNSTATED").count(),
        0
    );
    h.get_by_label("Undecided");
    for label in [unstated.as_str(), "Input content", "DDS tag", "Compression"] {
        h.get_by_label_contains(label);
    }
    // One frame with a Kaiser profile selected: the live Filter combo and the Edge row draw.
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.decide(
        "landscape/rock01.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    model.set_field(
        "landscape/rock01.png",
        "mips.filter",
        serde_json::json!("kaiser"),
    );
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/rock01.png");
    let mut h = harness(model);
    h.run();
    h.get_by_value("Kaiser");
    h.get_by_label("Edge");
}

#[test]
fn selecting_a_set_shows_its_recipe() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    let key = model
        .sets
        .iter()
        .find(|s| s.key.ends_with("rock01"))
        .unwrap()
        .key
        .clone();
    model.select_set(&key);
    let mut h = harness(model);
    h.run();
    // The recipe line names the packed output and its status; the Sets column's pill says only "rmaos: run".
    h.get_by_label_contains("rock01_rmaos.dds: run");
}

fn settle(h: &mut Harness<'static, AppModel>) {
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
}

/// Click `label` the way a pointer does: settle on it first, because egui hit-tests against the previous frame.
fn press(h: &mut Harness<'static, AppModel>, label: &str) {
    h.get_by_label(label).hover();
    h.run();
    h.get_by_label(label).click();
    h.run();
}

#[test]
fn the_opengl_button_on_the_unstated_green_line_writes_an_explicit_value() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.select_item("landscape/rock01_normal.png");
    let mut h = harness(model);
    h.run();
    press(&mut h, "Treat as OpenGL (flip green)");
    settle(&mut h);
    let item = h.state().item("landscape/rock01_normal.png").unwrap();
    let job = item.job.as_ref().unwrap();
    assert_eq!(
        job.profile.channels.green_convention,
        Some(engine::config::GreenConvention::Opengl)
    );
    assert!(h.state().is_dirty());
    // The assumption line and its button are gone; the sub-type question is still open, so the box stays.
    assert!(h.query_by_label("Treat as OpenGL (flip green)").is_none());
    h.get_by_label("Undecided");
}

fn green_of(h: &Harness<'static, AppModel>) -> Option<engine::config::GreenConvention> {
    let item = h.state().item("landscape/rock01_normal.png").unwrap();
    item.job.as_ref().unwrap().profile.channels.green_convention
}

#[test]
fn undo_takes_the_opengl_button_back() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.select_item("landscape/rock01_normal.png");
    let mut h = harness(model);
    h.run();
    press(&mut h, "Treat as OpenGL (flip green)");
    settle(&mut h);
    assert!(green_of(&h).is_some());
    h.state_mut().undo();
    settle(&mut h);
    assert_eq!(green_of(&h), None);
    h.get_by_label("Treat as OpenGL (flip green)");
}

#[test]
fn the_treat_as_directx_button_clears_an_opengl_setting() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.select_item("landscape/rock01_normal.png");
    let mut h = harness(model);
    h.run();
    press(&mut h, "Treat as OpenGL (flip green)");
    settle(&mut h);
    h.get_by_label_contains("Green: OpenGL (flipped)");
    press(&mut h, "Treat as DirectX");
    settle(&mut h);
    assert_eq!(green_of(&h), None);
    assert!(h.query_by_label_contains("Green: OpenGL").is_none());
    h.get_by_label("Treat as OpenGL (flip green)");
}

#[test]
fn the_alpha_dropdown_writes_the_policy_and_the_reset_button_takes_it_back() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    // A plain diffuse takes its alpha meaning from the mesh, so once the sub-type is decided the alpha is asked.
    model.decide(
        "landscape/leaf.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/leaf.png");
    let mut h = harness(model);
    h.run();
    h.get_by_label("Undecided");
    // Note FM35: one line and one dropdown, open on the proposed type; the five buttons are gone.
    h.get_by_label("Alpha: Cutout (proposed) — confirm");
    h.get_by_label("Alpha type: Cutout");
    assert_eq!(h.query_all_by_label("Blend").count(), 0);
    pick_alpha(&mut h, "", "Cutout", "Cutout");
    settle(&mut h);
    let job = h
        .state()
        .item("landscape/leaf.png")
        .unwrap()
        .job
        .clone()
        .unwrap();
    assert_eq!(
        job.profile.alpha.policy,
        engine::config::AlphaPolicy::Cutout
    );
    assert_eq!(job.provenance["alpha.policy"].to_string(), "project_file");
    // The write is the old buttons' write: the policy in the texture's own entry, and no acknowledgement.
    let doc = h.state().project.as_ref().unwrap();
    assert_eq!(
        doc.file_value("landscape/leaf.png", "alpha.policy"),
        Some(serde_json::json!("cutout"))
    );
    assert!(doc.allow("landscape/leaf.png").is_empty());
    assert!(h.query_by_label("Undecided").is_none());
    // The edited field offers a reset.
    // Two fields are edited now (sub-type, then alpha): the second reset is the alpha one.
    assert_eq!(h.query_all_by_label("↺").count(), 2);
    h.query_all_by_label("↺").nth(1).unwrap().hover();
    h.run();
    h.query_all_by_label("↺").nth(1).unwrap().click();
    h.run();
    settle(&mut h);
    let item = h.state().item("landscape/leaf.png").unwrap();
    assert_ne!(
        item.job.as_ref().unwrap().provenance["alpha.policy"].to_string(),
        "project_file"
    );
    assert!(!item.decisions.is_empty(), "the decision is open again");
}

/// Note FK, as Note FL refines it: the panel shows the last plan until the answer's partial plan lands. Its controls
/// are off meanwhile, with the reason, so a click is not lost to a stale view and nothing looks like it reverted.
/// (`ui::draw` does not poll, so the partial stays out until `settle`.)
#[test]
fn the_texture_panel_waits_for_the_plan_its_answer_started() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.decide(
        "landscape/leaf.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/leaf.png");
    use eframe::egui::accesskit::Role;
    let mut h = harness(model);
    h.run();
    // Note FM35: the alpha question is one dropdown, open on the proposed type.
    let dropdown = |h: &Harness<'static, AppModel>| {
        h.get_by_label("Alpha type: Cutout")
            .accesskit_node()
            .is_disabled()
    };
    assert!(!dropdown(&h));
    // The Alpha combo: the one whose value is an alpha meaning.
    let meanings = ["Remove", "Cutout", "Blend", "Cutout + Blend", "Data"];
    let alpha_combo = |h: &Harness<'static, AppModel>| {
        let n = h
            .query_all_by_role(Role::ComboBox)
            .find(|n| {
                n.accesskit_node()
                    .value()
                    .as_deref()
                    .is_some_and(|v| meanings.contains(&v))
            })
            .expect("the Alpha combo");
        (n.rect(), n.accesskit_node().is_disabled())
    };
    let (before, _) = alpha_combo(&h);
    pick_alpha(&mut h, "", "Cutout", "Cutout");
    assert!(h.state().busy(), "the answer's plan is in flight");
    assert!(h.state().partial_pending, "the answer's partial is out");
    assert!(dropdown(&h), "the dropdown is enabled");
    // The field combos are off too, and nothing moved: the reason is hover text, not a line.
    let (during, off) = alpha_combo(&h);
    assert!(off, "the Alpha combo is enabled");
    assert_eq!(before, during, "the panel moved when the plan started");
    h.get_by_label("Alpha type: Cutout").hover();
    h.run();
    h.get_by_label("Wait for the plan to finish");
    settle(&mut h);
    assert!(h.query_by_label("Undecided").is_none());
    let alpha = h
        .query_all_by_role(eframe::egui::accesskit::Role::ComboBox)
        .find(|n| n.accesskit_node().value().as_deref() == Some("Cutout"))
        .expect("the Alpha combo shows the answer");
    assert!(!alpha.accesskit_node().is_disabled());
}

/// Poll until the partial of the newest edit is parked, then handle it alone. The full plans stay parked in `full`,
/// so none can land in between (`hold_plan_results`).
fn land_partial(h: &mut Harness<'static, AppModel>, full: &mut Vec<sdc_gui::worker::WorkerMsg>) {
    use sdc_gui::worker::WorkerMsg;
    let m = h.state_mut();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut partial = None;
    while partial.is_none() {
        assert!(
            std::time::Instant::now() < end,
            "the partial did not arrive"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
        m.poll();
        for x in m.take_held() {
            match x {
                WorkerMsg::Partial { seq, .. } if seq == m.edit_seq => partial = Some(x),
                WorkerMsg::Partial { .. } => {}
                x => full.push(x),
            }
        }
    }
    m.handle_held(partial.unwrap());
    assert!(m.busy(), "the full plan is still out");
}

/// The Alpha combo showing `value`.
fn alpha_combo<'a>(
    h: &'a Harness<'static, AppModel>,
    value: &str,
) -> Option<egui_kittest::Node<'a>> {
    h.query_all_by_role(eframe::egui::accesskit::Role::ComboBox)
        .find(|n| n.accesskit_node().value().as_deref() == Some(value))
}

/// The owner's bug (Note FK1, fixed by Note FL): two alpha answers in a row. The second waits only for the first one's
/// partial plan, not for the full plan, which stays parked until the end.
#[test]
fn two_alpha_decisions_in_a_row_do_not_wait_for_the_full_plan() {
    use engine::config::AlphaPolicy;
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.decide(
        "landscape/leaf.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/leaf.png");
    model.hold_plan_results = true;
    let mut h = harness(model);
    h.run();
    let policy = |h: &Harness<'static, AppModel>| {
        h.state()
            .item("landscape/leaf.png")
            .and_then(|i| i.job.as_ref())
            .map(|j| j.profile.alpha.policy)
    };
    let mut full = vec![];
    pick_alpha(&mut h, "", "Cutout", "Cutout");
    assert!(
        h.state().fix_off().is_some(),
        "the first answer's partial is out"
    );
    land_partial(&mut h, &mut full);
    h.run();
    assert_eq!(policy(&h), Some(AlphaPolicy::Cutout));
    assert!(h.query_by_label("Undecided").is_none());
    let off = alpha_combo(&h, "Cutout")
        .expect("the Alpha combo shows the answer")
        .accesskit_node()
        .is_disabled();
    assert!(!off, "the second answer waits for the full plan");
    assert!(h.state().busy(), "the first full plan is still out");
    // The second answer, through the Alpha combo.
    alpha_combo(&h, "Cutout").unwrap().hover();
    h.run();
    alpha_combo(&h, "Cutout").unwrap().click();
    h.run();
    h.get_by_label("Blend").hover();
    h.run();
    h.get_by_label("Blend").click();
    h.run();
    assert!(h.state().fix_off().is_some(), "the second partial is out");
    land_partial(&mut h, &mut full);
    h.run();
    assert_eq!(policy(&h), Some(AlphaPolicy::Blend));
    let off = alpha_combo(&h, "Blend")
        .expect("the Alpha combo shows the second answer")
        .accesskit_node()
        .is_disabled();
    assert!(!off);
    // Review says the full plan is still out.
    h.state_mut().open_review();
    h.run();
    h.get_by_label("Planning…");
    // Both full plans land behind; the newer one wins.
    let m = h.state_mut();
    m.hold_plan_results = false;
    for x in full {
        m.sender().send(x).unwrap();
    }
    settle(&mut h);
    assert_eq!(policy(&h), Some(AlphaPolicy::Blend));
    assert!(h.query_by_label("Planning…").is_none());
}

/// Note FK3: only the edit rows wait; with a read-only `sdc.toml` a group header still opens and closes.
#[test]
fn a_read_only_project_keeps_the_texture_panel_groups_openable() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.decide(
        "landscape/leaf.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/leaf.png");
    model
        .project
        .as_mut()
        .unwrap()
        .set_error("broken on purpose");
    assert!(model.fix_off().is_some_and(|w| w.contains("read-only")));
    let mut h = harness(model);
    h.run();
    // The answers and rows are off …
    assert!(h
        .get_by_label("Alpha type: Cutout")
        .accesskit_node()
        .is_disabled());
    let format_rows = |h: &Harness<'static, AppModel>| h.query_all_by_label("Format").count();
    assert!(format_rows(&h) >= 1, "the Compression group starts open");
    // … the group header is not: it collapses the group.
    let header = h
        .query_all_by_label("Compression")
        .find(|n| !n.accesskit_node().is_disabled())
        .expect("an enabled Compression header");
    header.hover();
    h.run();
    h.query_all_by_label("Compression")
        .find(|n| !n.accesskit_node().is_disabled())
        .unwrap()
        .click();
    h.run();
    assert_eq!(format_rows(&h), 0, "the Compression group did not collapse");
}

#[test]
fn the_threshold_commits_when_the_drag_stops_not_while_dragging() {
    use eframe::egui::{Event, Modifiers, PointerButton};
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.decide(
        "landscape/leaf.png",
        sdc_gui::model::Answer::Alpha(engine::config::AlphaPolicy::Cutout),
    );
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/leaf.png");
    let mut h = harness(model);
    h.run();
    let before = h
        .state()
        .item("landscape/leaf.png")
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .profile
        .alpha
        .threshold;
    let rect = h
        .get_by_role_and_label(eframe::egui::accesskit::Role::Slider, "Alpha threshold")
        .rect();
    let at = |f: f32| {
        eframe::egui::pos2(
            rect.left() + 10.0 + (rect.width() - 20.0) * f,
            rect.center().y,
        )
    };
    let ev = |h: &mut Harness<'static, AppModel>, e: Event| {
        h.input_mut().events.push(e);
        h.step();
    };
    ev(&mut h, Event::PointerMoved(at(0.2)));
    ev(
        &mut h,
        Event::PointerButton {
            pos: at(0.2),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
    );
    ev(&mut h, Event::PointerMoved(at(0.5)));
    ev(&mut h, Event::PointerMoved(at(0.8)));
    // Still dragging: nothing was sent to the model, so no re-plan is running.
    assert!(!h.state().busy(), "a re-plan started mid-drag");
    ev(
        &mut h,
        Event::PointerButton {
            pos: at(0.8),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    );
    h.run();
    settle(&mut h);
    let job = h
        .state()
        .item("landscape/leaf.png")
        .unwrap()
        .job
        .clone()
        .unwrap();
    assert_ne!(job.profile.alpha.threshold, before);
    assert_eq!(
        job.provenance["alpha.threshold"].to_string(),
        "project_file"
    );
}

#[test]
fn a_packed_item_lists_its_channels_and_a_raw_map_points_to_the_output() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    let packed = model
        .items
        .iter()
        .find(|i| i.kind == sdc_gui::model::items::ItemKind::Packed)
        .unwrap()
        .key
        .clone();
    model.select_item(&packed);
    let mut h = harness(model);
    h.run();
    h.get_by_label("Channels");
    h.get_by_label_contains("R: from ");
    h.get_by_label_contains("B: from rock01_ao.png");
    h.get_by_label("A: default value 1");
    // A raw map (a helper since Note DY16) says where it goes and offers the way to its output.
    h.state_mut().select_item("landscape/rock01_roughness.png");
    h.run();
    h.get_by_label("fills RMAOS R  rock01_rmaos");
    press(&mut h, "Show rock01_rmaos");
    assert_eq!(h.state().selection, Selection::Item(packed));
}

#[test]
fn a_locked_field_is_disabled_and_says_why() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.set_target(engine::config::Target::Vanilla);
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/rock01.png");
    let mut h = harness(model);
    h.run();
    h.get_by_label("_SRGB is only written under cs");
}

#[test]
fn export_asks_about_undecided_textures_then_the_drawer_reports() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d.path()));
    h.run();
    {
        let l = header_export_label(&h);
        press(&mut h, &l);
    }
    // Export opens the Review tab, not a modal, and starts nothing.
    assert!(h.state().review.is_some() && h.state().dialog.is_none());
    h.get_by_label_contains("Undecided (");
    h.get_by_label("Accept all suggestions");
    assert!(h.query_all_by_label_contains("Set type: ").count() >= 1);
    // Review's Close starts nothing and collapses the panel.
    press(&mut h, "Close");
    assert!(h.state().review.is_none() && !h.state().bottom.open);
    assert!(h.state().export.running.is_none() && h.state().export.summary.is_none());
    {
        let l = header_export_label(&h);
        press(&mut h, &l);
    }
    let skip = h.state().review_button_text();
    assert!(skip.ends_with("undecided)"), "{skip}");
    press(&mut h, &skip);
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(120)));
    h.run();
    for label in ["Files", "Save report…", "Copy CLI command", "Close"] {
        h.get_by_label(label);
    }
    h.get_by_label_contains("converted, ");
    assert!(h.query_all_by_label_contains("skipped: undecided").count() >= 1);
    press(&mut h, "Close");
    assert!(!h.state().bottom.open);
    assert!(h.query_by_label("Copy CLI command").is_none());
}

#[test]
fn the_expanded_preview_offers_encode_now_and_collapses_on_escape() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.expand("landscape/rock01_rmaos");
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    let mut h = harness(model);
    h.run();
    h.get_by_label_contains("Not encoded");
    // The toolbar's button and the one on the empty encoded side.
    assert_eq!(h.query_all_by_label("Encode now").count(), 2);
    h.get_by_label("Live encode");
    h.get_by_label_contains("mip 0");
    h.key_press(eframe::egui::Key::Escape);
    h.run();
    assert!(h.state().expanded.is_none());
}

#[test]
fn encode_now_in_the_preview_encodes_and_shows_the_readouts() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.expand("landscape/rock01_rmaos");
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    let mut h = harness(model);
    h.run();
    h.query_all_by_label("Encode now").next().unwrap().hover();
    h.run();
    h.query_all_by_label("Encode now").next().unwrap().click();
    h.run();
    settle(&mut h);
    assert!(h.query_by_label_contains("Not encoded").is_none());
    h.get_by_label("Minimum mip PSNR");
    // Step to another mip with the slider's keyboard control: the label follows.
    h.state_mut().view.mip = 2;
    h.run();
    h.get_by_label_contains("mip 2 (16×16)");
}

#[test]
fn the_preview_toolbar_changes_the_view() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.expand("landscape/rock01_rmaos");
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    let mut h = harness(model);
    h.run();
    press(&mut h, "G");
    assert_eq!(h.state().view.channel, sdc_gui::model::view::Channel::G);
    press(&mut h, "UNORM");
    assert!(h.state().view.view_linear);
    press(&mut h, "Live encode");
    assert!(h.state().preview.live);
}

#[test]
fn a_double_click_in_a_viewport_collapses_the_preview() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.expand("landscape/rock01_rmaos");
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    // Two clicks must land within egui's double-click delay: shorten the simulated frame.
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .with_step_dt(0.02)
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    // A point inside the encoded viewport, below its message.
    let label = h.get_by_label_contains("Not encoded").rect();
    let at = label.center() + eframe::egui::vec2(0.0, 100.0);
    h.hover_at(at);
    h.run();
    for _ in 0..2 {
        h.event(eframe::egui::Event::PointerButton {
            pos: at,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        });
        h.event(eframe::egui::Event::PointerButton {
            pos: at,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        });
    }
    h.run();
    assert!(h.state().expanded.is_none());
}

#[test]
fn the_settings_strip_is_compact() {
    let d = tempfile::tempdir().unwrap();
    // The full panel names where each value comes from.
    let mut model = opened(d.path());
    model.select_item("landscape/rock01_rmaos");
    let mut h = harness(model);
    h.run();
    assert!(h.query_all_by_label_contains("built-in default").count() > 0);
    // The strip keeps Alpha, Mipmaps and Compression and drops that text.
    let mut model = opened(d.path());
    model.expand("landscape/rock01_rmaos");
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    let mut h = harness(model);
    h.run();
    // At 1280×800 the settings start open (Note FC8, supersedes CE16's strip there).
    assert!(h.query_by_label("Settings").is_none());
    for group in ["Alpha", "Mipmaps", "Compression"] {
        assert!(h.query_all_by_label_contains(group).count() > 0, "{group}");
    }
    // Note HI2: the colour rows moved into Compression, and the strip still leaves them out.
    for row in ["Colour", "Input content", "DDS tag"] {
        assert!(h.query_by_label_contains(row).is_none(), "{row}");
    }
    assert_eq!(h.query_all_by_label_contains("built-in default").count(), 0);
}

/// The preview's toolbar panel rect (A7: every toolbar lookup is scoped to it).
fn toolbar_rect(h: &Harness<'static, AppModel>) -> eframe::egui::Rect {
    use eframe::egui::containers::panel::PanelState;
    PanelState::load(&h.ctx, eframe::egui::Id::new("preview-toolbar"))
        .expect("the toolbar is drawn")
        .outer_rect
}

/// The topmost node labelled `label` (or containing it, with `contains`) that starts within the toolbar's x range, at or
/// below its top: the toolbar's control, or one that leaked out of it.
fn toolbar_node(h: &Harness<'static, AppModel>, label: &str, contains: bool) -> eframe::egui::Rect {
    let bar = toolbar_rect(h);
    let nodes: Vec<_> = if contains {
        h.query_all_by_label_contains(label).collect()
    } else {
        h.query_all_by_label(label).collect()
    };
    nodes
        .iter()
        .map(|n| n.rect())
        .filter(|r| r.min.y >= bar.min.y - 1.0 && (bar.min.x - 1.0..bar.max.x).contains(&r.min.x))
        .min_by(|a, b| a.min.y.total_cmp(&b.min.y))
        .unwrap_or_else(|| panic!("{label} is not in the toolbar"))
}

/// K9/K10: the toolbar's controls in reading order (row, then x).
const TOOLBAR_ORDER: [(&str, bool); 19] = [
    ("Channels", false),
    ("RGBA", false),
    ("R", false),
    ("G", false),
    ("B", false),
    ("A", false),
    ("α-mask", false),
    ("View", false),
    ("Heatmap", false),
    ("Lens", false),
    ("Tile 3×3", false),
    ("Fit", false),
    ("1:1", false),
    ("Encode", false),
    ("Live encode", false),
    ("Encode now", false),
    ("sRGB", false),
    ("UNORM", false),
    ("mip 0", true),
];

/// `a` comes before `b` reading the toolbar: on an earlier row, or on the same row and to the left.
fn reads_before(a: eframe::egui::Rect, b: eframe::egui::Rect) -> bool {
    a.center().y < b.min.y || (b.y_range().contains(a.center().y) && a.max.x <= b.min.x)
}

fn expanded_at_1280(advanced: bool) -> (Harness<'static, AppModel>, tempfile::TempDir) {
    expanded_at(1280.0, advanced)
}

/// Note FC8: below 1280 px the preview's settings start as the strip.
fn expanded_at_1200(advanced: bool) -> (Harness<'static, AppModel>, tempfile::TempDir) {
    expanded_at(1200.0, advanced)
}

fn expanded_at(width: f32, advanced: bool) -> (Harness<'static, AppModel>, tempfile::TempDir) {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.settings.advanced_open = advanced;
    model.expand("landscape/rock01_rmaos");
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(width, 800.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    (h, d)
}

/// `extra`: conditional controls drawn after "Tile 3×3" and before the overlay toggle, by AccessKit name (none now).
fn assert_toolbar_layout(h: &Harness<'static, AppModel>, extra: &[&str]) {
    let bar = toolbar_rect(h);
    let tile = TOOLBAR_ORDER
        .iter()
        .position(|(l, _)| *l == "Tile 3×3")
        .unwrap();
    let order = TOOLBAR_ORDER[..=tile]
        .iter()
        .copied()
        .chain(extra.iter().map(|l| (*l, false)))
        .chain(TOOLBAR_ORDER[tile + 1..].iter().copied());
    let rects: Vec<_> = order.map(|(l, c)| (l, toolbar_node(h, l, c))).collect();
    for w in rects.windows(2) {
        assert!(
            reads_before(w[0].1, w[1].1),
            "{} {:?} should read before {} {:?}",
            w[0].0,
            w[0].1,
            w[1].0,
            w[1].1
        );
    }
    // ✕ Close stays on the right of the first row.
    let close = toolbar_node(h, "✕ Close", false);
    let first = rects[0].1;
    assert!(
        close.y_range().contains(first.center().y),
        "Close {close:?}"
    );
    for (l, r) in &rects {
        assert!(
            r.max.x <= close.min.x || r.min.y > close.max.y,
            "{l} {r:?} under Close {close:?}"
        );
    }
    // Every control lies inside the toolbar panel and is not clipped by it (skill §10.2-10.3). The finished pass is
    // `prev_pass`: egui swaps the two at the end of a pass.
    let widgets: Vec<_> = h.ctx.viewport(|v| {
        v.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, ws)| ws)
            .map(|w| (w.rect, w.interact_rect))
            .collect()
    });
    for (l, r) in rects.iter().chain([("✕ Close", close)].iter()) {
        assert!(
            bar.contains_rect(*r),
            "{l} {r:?} outside the toolbar {bar:?}"
        );
        let hit = widgets.iter().find(|(w, _)| w == r).map(|(_, i)| *i);
        let hit = hit.unwrap_or_else(|| panic!("{l} {r:?} has no widget rect in the last pass"));
        assert!(
            hit.expand(0.5).contains_rect(*r),
            "{l} {r:?} clipped to {hit:?}"
        );
    }
}

#[test]
fn the_toolbar_groups_channels_view_and_encode() {
    let (h, _d) = expanded_at_1280(false);
    assert_toolbar_layout(&h, &[]);
}

/// The toolbar sits inside the preview and never under ✕ Close, at 1100, 1280 and 1600 px.
#[test]
fn the_toolbar_pin_holds_at_three_widths() {
    for width in [1100.0, 1280.0, 1600.0] {
        let d = tempfile::tempdir().unwrap();
        let mut model = pbr_wall(d.path(), 20, [128, 10, 200]);
        model.expand("test/wall.png");
        assert!(model.wait_idle(std::time::Duration::from_secs(60)));
        let mut h = Harness::builder()
            .with_size(eframe::egui::vec2(width, 800.0))
            .build_ui_state(
                |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
                model,
            );
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run();
        assert_toolbar_layout(&h, &[]);
    }
}

/// K8, K10 (shots 15/16/17): with Advanced open at 1200×800 the viewport keeps PREVIEW_MIN and the settings collapse
/// (Note FC8: from 1280 they start open).
#[test]
fn at_1200x800_with_advanced_open_the_preview_keeps_its_width_and_the_toolbar_fits() {
    use eframe::egui::containers::panel::PanelState;
    let (mut h, _d) = expanded_at_1200(true);
    assert!(PanelState::load(&h.ctx, eframe::egui::Id::new("advanced")).is_some());
    // The wrapped rows settle: centring them in last frame's height grew the toolbar every frame.
    let settled = toolbar_rect(&h);
    h.run_steps(5);
    assert_eq!(toolbar_rect(&h), settled);
    // The toolbar spans the viewport's width (the settings or their strip are drawn first, on the right).
    let bar = toolbar_rect(&h);
    assert!(
        bar.width() >= sdc_gui::ui::preview::PREVIEW_MIN,
        "viewport {} < {}",
        bar.width(),
        sdc_gui::ui::preview::PREVIEW_MIN
    );
    h.get_by_label("Settings");
    assert_toolbar_layout(&h, &[]);
}

/// A6, A7, A10: the strip opens the settings, and they stay open for this preview.
#[test]
fn the_collapsed_settings_strip_opens_the_settings() {
    use eframe::egui::containers::panel::PanelState;
    let (mut h, _d) = expanded_at_1200(true);
    assert!(PanelState::load(&h.ctx, eframe::egui::Id::new("preview-settings-strip")).is_some());
    assert!(h.query_by_label_contains("Compression").is_none());
    h.get_by_label("Settings").hover();
    h.run();
    h.get_by_label("Settings").click();
    h.run();
    h.run();
    assert!(h.query_all_by_label_contains("Compression").count() > 0);
    assert!(h.query_by_label("Settings").is_none());
    // Collapsing and expanding again starts collapsed (a fresh PreviewUi per expansion).
    h.state_mut().collapse();
    h.run();
    h.state_mut().expand("landscape/rock01_rmaos");
    h.run();
    h.get_by_label("Settings");
}

/// Note FC8 (supersedes CE16's collapsed default): from 1280 px wide the preview's settings start open, and opening
/// Advanced does not collapse them; below 1280 they start as the strip.
#[test]
fn preview_settings_open_at_1280() {
    // The strip's button is drawn only while the settings are collapsed (their PanelState outlives one frame).
    use sdc_gui::ui::State;
    type H = Harness<'static, (AppModel, State)>;
    let open = |h: &H| {
        h.query_by_label("Settings").is_none()
            && h.query_all_by_label_contains("Compression").count() > 0
    };
    for (width, want) in [(1280.0, true), (1600.0, true), (1200.0, false)] {
        let d = tempfile::tempdir().unwrap();
        let mut model = opened(d.path());
        // Below 1280 Advanced is open from the start: the settings would fit beside the picture without it.
        model.settings.advanced_open = !want;
        model.expand("landscape/rock01_rmaos");
        assert!(model.wait_idle(std::time::Duration::from_secs(60)));
        // Its own drawing state: `ui::draw`'s thread-local one would carry the last preview's settings over.
        let mut h: H = Harness::builder()
            .with_size(eframe::egui::vec2(width, 800.0))
            .build_ui_state(
                |ui, (m, s): &mut (AppModel, State)| sdc_gui::ui::draw_with(ui, m, s),
                (model, State::default()),
            );
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run();
        assert_eq!(open(&h), want, "{width}");
        if want {
            h.state_mut().0.settings.advanced_open = true;
            h.run();
            h.run();
            assert!(open(&h), "{width}: Advanced opened");
        }
    }
}

/// I2 (shots 15, 28): settings wider than the panel moved its left edge in, so the toolbar and viewports drew over its
/// left strip. The centre ends at the panel's left edge, and the group headings start inside the panel.
#[test]
fn the_preview_settings_panel_is_never_drawn_under_the_toolbar() {
    use eframe::egui::containers::panel::PanelState;
    for key in ["landscape/rock01_rmaos", "landscape/rock01_normal.png"] {
        let d = tempfile::tempdir().unwrap();
        let mut model = opened(d.path());
        model.expand(key);
        assert!(model.wait_idle(std::time::Duration::from_secs(60)));
        let mut h = Harness::builder()
            .with_size(eframe::egui::vec2(1600.0, 1000.0))
            .build_ui_state(
                |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
                model,
            );
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run();
        let panel = PanelState::load(&h.ctx, eframe::egui::Id::new("preview-settings"))
            .expect("the settings are open at 1600 wide")
            .outer_rect;
        let bar = toolbar_rect(&h);
        assert!(
            bar.right() <= panel.left() + 0.5,
            "{key}: the toolbar {bar:?} runs under the settings {panel:?}"
        );
        assert!(
            panel.right() <= h.ctx.content_rect().right(),
            "{key}: the settings {panel:?} run off the window"
        );
        // The toolbar beside the open settings is narrower, and nothing in it runs under ✕ Close.
        assert_toolbar_layout(&h, &[]);
        // The panel is the rightmost thing with these labels; its stored rect is where egui moved it.
        for g in ["Alpha", "Mipmaps", "Compression"] {
            let r = h
                .query_all_by_label(g)
                .map(|n| n.rect())
                .max_by(|a, b| a.min.x.total_cmp(&b.min.x))
                .unwrap_or_else(|| panic!("{key}: no {g} heading"));
            assert!(
                panel.contains_rect(r),
                "{key}: {g} {r:?} outside the settings {panel:?}"
            );
        }
    }
}

/// K8, S7: at 1100 px the Advanced drawer gives way (down to its 260 px minimum) so the preview keeps PREVIEW_MIN, and
/// it gets its width back once the preview closes.
#[test]
fn at_1100_wide_the_advanced_drawer_leaves_the_preview_its_width() {
    use eframe::egui::containers::panel::PanelState;
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.settings.advanced_open = true;
    model.select_item("landscape/rock01_rmaos");
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(1100.0, 800.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    let width = |h: &Harness<'static, AppModel>| {
        PanelState::load(&h.ctx, eframe::egui::Id::new("advanced"))
            .unwrap()
            .outer_rect
            .width()
    };
    let before = width(&h);
    assert!(before > 370.0, "Advanced {before}");
    h.state_mut().expand("landscape/rock01_rmaos");
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.run();
    assert!(
        (260.0..=330.0).contains(&width(&h)),
        "Advanced {}",
        width(&h)
    );
    let bar = toolbar_rect(&h);
    assert!(
        bar.width() >= sdc_gui::ui::preview::PREVIEW_MIN,
        "viewport {}",
        bar.width()
    );
    assert_toolbar_layout(&h, &[]);
    assert_toolbar_layout(&h, &[]);
    h.state_mut().collapse();
    h.run();
    assert_eq!(width(&h), before);
}

#[test]
fn an_edit_after_encoding_shows_the_stale_badge() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.expand("landscape/rock01_rmaos");
    model.encode_now();
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.set_field(
        "landscape/rock01_rmaos",
        "encoder.effort",
        serde_json::json!("fast"),
    );
    let mut h = harness(model);
    settle(&mut h);
    h.get_by_label_contains("stale");
    assert!(h.query_by_label_contains("Not encoded").is_none());
}

#[test]
fn a_changed_source_offers_reload_which_plans_again() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    let p = model.root.clone().unwrap().join("landscape/rock01.png");
    image::ImageBuffer::from_fn(32, 32, |_, _| image::Rgb([9u8, 9, 9]))
        .save(&p)
        .unwrap();
    assert_eq!(model.check_sources(), 1);
    let mut h = harness(model);
    h.run();
    h.get_by_label_contains("1 changed on disk");
    press(&mut h, "Reload");
    settle(&mut h);
    assert!(h.query_by_label_contains("changed on disk").is_none());
}

#[test]
fn without_a_gpu_the_preview_says_so_and_cannot_encode() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.gpu_available = false;
    model.expand("landscape/rock01_rmaos");
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    let mut h = harness(model);
    h.run();
    h.get_all_by_label_contains("No usable GPU: the preview is off; everything else works")
        .next()
        .unwrap();
    // The toolbar's button stays, disabled; the pane offers none.
    assert_eq!(h.query_all_by_label("Encode now").count(), 1);
    assert!(h.get_by_label("Encode now").accesskit_node().is_disabled());
}

#[test]
fn closing_during_an_export_asks_and_keep_running_leaves_it_alone() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    export_through(&mut model);
    assert!(!model.request_close());
    let mut h = harness(model);
    h.run();
    h.get_by_label_contains("An export is running");
    h.get_by_label("Stop the export and close");
    press(&mut h, "Keep exporting");
    assert!(h.state().dialog.is_none());
    assert!(!h.state().export.running.as_ref().unwrap().is_cancelled());
    // Asking again and cancelling sets the token.
    assert!(!h.state_mut().request_close());
    h.run();
    press(&mut h, "Stop the export and close");
    assert!(h.state().export.running.as_ref().unwrap().is_cancelled());
    settle(&mut h);
    assert!(h.state_mut().take_close_ready());
}

#[test]
fn the_header_shows_the_chosen_export_folder() {
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    let mut model = AppModel::new(GuiSettings::default(), None, session());
    model.accept = engine::policy::AcceptSuggestions::None;
    model.open_folder(root);
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    // Note FI5 (amends FB1): "not chosen" (and no "not set") until a folder is chosen.
    let mut h = harness(model);
    h.run();
    h.get_by_label("Export to: not chosen");
    assert!(h.query_by_label_contains("not set").is_none());
    let out = d.path().join("out");
    h.state_mut().set_export_folder(out.clone());
    settle(&mut h);
    // K8: the accessible name keeps the whole path; the button text may cut it.
    h.get_by_label(&format!("Export to: {}", out.display()));
}

#[test]
fn export_is_disabled_while_sdc_toml_is_ignored() {
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    std::fs::write(root.join("sdc.toml"), "target = [\n").unwrap();
    let settings = GuiSettings {
        export_folder: Some(d.path().join("export")),
        ..GuiSettings::default()
    };
    let mut model = AppModel::new(settings, None, session());
    model.accept = engine::policy::AcceptSuggestions::None;
    model.open_folder(root);
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    let mut h = harness(model);
    h.run();
    assert!(h
        .get_by_label(&header_export_label(&h))
        .accesskit_node()
        .is_disabled());
    // A healthy folder has it enabled.
    let d2 = tempfile::tempdir().unwrap();
    let mut h = harness(opened(d2.path()));
    h.run();
    assert!(!h
        .get_by_label(&header_export_label(&h))
        .accesskit_node()
        .is_disabled());
}

#[test]
fn a_double_click_on_a_set_member_expands_it() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    let key = model
        .sets
        .iter()
        .find(|s| s.key.ends_with("rock01"))
        .unwrap()
        .key
        .clone();
    model.select_set(&key);
    // Two clicks must land within egui's double-click delay: shorten the simulated frame.
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .with_step_dt(0.02)
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    // The member name is in its slot row; the same name is also a tile in the Textures column to the left.
    let label = h
        .query_all_by_label_contains("rock01_normal.png")
        .map(|n| n.rect())
        .max_by(|a, b| a.min.x.total_cmp(&b.min.x))
        .expect("the member in the set view");
    // Layout v2 A2: the thumbnail sits left of the name, on the same slot row.
    let at = h
        .query_all_by_label_contains(" thumbnail")
        .map(|n| n.rect())
        .find(|r| r.y_range().contains(label.center().y))
        .expect("the member's thumbnail")
        .center();
    h.hover_at(at);
    h.run();
    assert!(h.state().expanded.is_none());
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(eframe::egui::Event::PointerButton {
                pos: at,
                button: eframe::egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
        }
    }
    h.run();
    assert_eq!(
        h.state().expanded.as_deref(),
        Some("landscape/rock01_normal.png")
    );
}

#[test]
fn a_single_click_on_a_set_member_selects_it_after_the_double_click_delay() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    let key = model
        .sets
        .iter()
        .find(|s| s.key.ends_with("rock01"))
        .unwrap()
        .key
        .clone();
    model.select_set(&key);
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .with_step_dt(0.02)
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    let label = h
        .query_all_by_label_contains("rock01_normal.png")
        .map(|n| n.rect())
        .max_by(|a, b| a.min.x.total_cmp(&b.min.x))
        .expect("the member in the set view");
    // Layout v2 A2: the thumbnail sits left of the name, on the same slot row.
    let at = h
        .query_all_by_label_contains(" thumbnail")
        .map(|n| n.rect())
        .find(|r| r.y_range().contains(label.center().y))
        .expect("the member's thumbnail")
        .center();
    h.hover_at(at);
    h.run();
    for pressed in [true, false] {
        h.event(eframe::egui::Event::PointerButton {
            pos: at,
            button: eframe::egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
    }
    h.step();
    assert_eq!(h.state().selection, Selection::Set(key), "not yet");
    for _ in 0..40 {
        h.step();
    }
    assert_eq!(
        h.state().selection,
        Selection::Item("landscape/rock01_normal.png".into())
    );
    assert!(h.state().expanded.is_none());
}

#[test]
fn a_stale_pending_member_click_does_not_replace_a_set_opened_later() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    let key = model
        .sets
        .iter()
        .find(|s| s.key.ends_with("rock01"))
        .unwrap()
        .key
        .clone();
    model.select_set(&key);
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .with_step_dt(0.02)
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    let label = h
        .query_all_by_label_contains("rock01_normal.png")
        .map(|n| n.rect())
        .max_by(|a, b| a.min.x.total_cmp(&b.min.x))
        .expect("the member in the set view");
    // Layout v2 A2: the thumbnail sits left of the name, on the same slot row.
    let at = h
        .query_all_by_label_contains(" thumbnail")
        .map(|n| n.rect())
        .find(|r| r.y_range().contains(label.center().y))
        .expect("the member's thumbnail")
        .center();
    h.hover_at(at);
    h.run();
    for pressed in [true, false] {
        h.event(eframe::egui::Event::PointerButton {
            pos: at,
            button: eframe::egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
    }
    h.step();
    // Something else is selected before the wait is over: the set view is no longer drawn.
    h.state_mut().select_item("landscape/leaf.png");
    for _ in 0..120 {
        h.step();
    }
    h.state_mut().select_set(&key);
    for _ in 0..10 {
        h.step();
    }
    assert_eq!(h.state().selection, Selection::Set(key));
}

#[test]
fn the_preview_has_a_close_button_that_collapses_it() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.expand("landscape/rock01_rmaos");
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    let mut h = harness(model);
    h.run();
    h.get_by_label("✕ Close");
    press(&mut h, "✕ Close");
    assert!(h.state().expanded.is_none());
}

#[test]
fn the_export_button_is_big_enough_to_find() {
    let mut h = harness(AppModel::placeholder(GuiSettings::default()));
    h.run();
    let r = h.get_by_label(&header_export_label(&h)).rect();
    assert!(r.width() >= 110.0 && r.height() >= 28.0, "{r:?}");
}

fn leaf_with_policy(policy: engine::config::AlphaPolicy) -> Harness<'static, AppModel> {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.decide("landscape/leaf.png", sdc_gui::model::Answer::Alpha(policy));
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/leaf.png");
    // The temp folder is only read while the model opens; the harness keeps the model.
    std::mem::forget(d);
    let mut h = harness(model);
    h.run();
    h
}

fn mips_levels(h: &Harness<'static, AppModel>) -> serde_json::Value {
    let job = h.state().item("landscape/leaf.png").unwrap().job.clone();
    serde_json::to_value(job.unwrap().profile.mips.levels).unwrap()
}

#[test]
fn the_mipmaps_tick_box_writes_none_and_full() {
    use eframe::egui::accesskit::Role;
    let mut h = leaf_with_policy(engine::config::AlphaPolicy::Cutout);
    assert_eq!(mips_levels(&h), serde_json::json!("full"));
    let tick = |h: &mut Harness<'static, AppModel>| {
        h.get_by_role_and_label(Role::CheckBox, "Mipmaps").hover();
        h.run();
        h.get_by_role_and_label(Role::CheckBox, "Mipmaps").click();
        h.run();
        settle(h);
    };
    tick(&mut h);
    assert_eq!(mips_levels(&h), serde_json::json!("none"));
    let item = h.state().item("landscape/leaf.png").unwrap();
    assert_eq!(
        item.job.as_ref().unwrap().provenance["mips.levels"].to_string(),
        "project_file"
    );
    tick(&mut h);
    assert_eq!(mips_levels(&h), serde_json::json!("full"));
}

#[test]
fn a_mip_count_shows_ticked_with_a_note() {
    use eframe::egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.set_field("landscape/leaf.png", "mips.levels", serde_json::json!(3));
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/leaf.png");
    let mut h = harness(model);
    h.run();
    let tick = h.get_by_role_and_label(Role::CheckBox, "Mipmaps");
    assert_eq!(
        tick.accesskit_node().toggled(),
        Some(eframe::egui::accesskit::Toggled::True)
    );
    h.get_by_label("(3 levels — see Advanced)");
}

#[test]
fn levels_live_in_the_advanced_drawer_not_the_main_panel() {
    let mut h = leaf_with_policy(engine::config::AlphaPolicy::Cutout);
    assert!(h.query_by_label("Levels").is_none());
    h.state_mut().settings.advanced_open = true;
    h.run();
    h.get_by_label("Levels");
    // The drawer adds its own "Mipmaps" section heading next to the group heading and the tick box.
    assert!(h.query_all_by_label("Mipmaps").count() >= 3);
}

#[test]
fn coverage_sits_under_alpha_for_cutout_policies_only() {
    use eframe::egui::accesskit::Role;
    let h = leaf_with_policy(engine::config::AlphaPolicy::Cutout);
    let y = |label: &str| h.get_by_label(label).rect().top();
    let tick_y = h
        .get_by_role_and_label(Role::CheckBox, "Mipmaps")
        .rect()
        .top();
    assert!(y("Coverage") > y("Comparison"), "directly under Comparison");
    assert!(y("Coverage") < tick_y, "inside the Alpha group");
    let h = leaf_with_policy(engine::config::AlphaPolicy::Blend);
    assert!(h.query_by_label("Coverage").is_none());
    h.get_by_label("Threshold"); // locked, but still listed
}

#[test]
fn the_filter_row_is_a_live_combo_in_the_mipmaps_group() {
    let h = leaf_with_policy(engine::config::AlphaPolicy::Remove);
    h.get_by_label("Filter");
    assert!(!h.get_by_value("Box").accesskit_node().is_disabled());
}

#[test]
fn an_8_bit_emissive_says_why_it_is_bc7() {
    let d = tempfile::tempdir().unwrap();
    let t = d.path().join("textures/pbr");
    std::fs::create_dir_all(&t).unwrap();
    image::ImageBuffer::from_fn(16, 16, |x, _| image::Rgb([(x * 16) as u8, 40u8, 20]))
        .save(t.join("lamp_g.png"))
        .unwrap();
    let mut m = AppModel::new(GuiSettings::default(), None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().join("textures"));
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    m.select_item("pbr/lamp_g.png");
    let mut h = harness(m);
    h.run();
    // Exact and unique: shown once, under Compression, without the code prefix of the Warnings list.
    h.get_by_label("8-bit source, no HDR → BC7 sRGB");
}

/// Controller ruling F2 (M6 Task 11): an RGBA16F preview is tone-mapped, so its sRGB / UNORM toggle is disabled.
#[test]
fn an_rgba16f_preview_disables_the_srgb_unorm_toggle() {
    use engine::config::Format;
    use engine::dds::*;
    let d = tempfile::tempdir().unwrap();
    let root = mod_folder(d.path());
    let mut levels = vec![];
    let mut s = 16u32;
    loop {
        levels.push(vec![0u8; level_size(Format::Rgba16f, s, s)]);
        if s == 1 {
            break;
        }
        s /= 2;
    }
    let desc = DdsDesc {
        width: 16,
        height: 16,
        format: Format::Rgba16f,
        srgb: false,
        header: ResolvedHeader::Dx10,
    };
    std::fs::write(
        root.join("landscape/sky.dds"),
        dds_bytes(&desc, &levels).unwrap(),
    )
    .unwrap();
    let mut model = opened(d.path());
    model.expand("landscape/sky.dds");
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    let mut h = harness(model);
    h.run();
    assert!(matches!(
        &h.state().preview.encoded,
        sdc_gui::model::preview::EncodedState::Done(o) if o.result.format == Format::Rgba16f
    ));
    // The header panel's tag radios are also labelled sRGB / UNORM: take the toolbar's toggles.
    let radio = eframe::egui::accesskit::Role::RadioButton;
    for label in ["sRGB", "UNORM"] {
        let toggles: Vec<_> = h
            .query_all_by_label(label)
            .filter(|n| n.accesskit_node().role() != radio)
            .collect();
        assert_eq!(toggles.len(), 1, "{label}");
        assert!(toggles[0].accesskit_node().is_disabled(), "{label}");
    }
}

#[test]
fn the_dim_box_offers_no_resize_button() {
    // Note FM29: the power of two is automatic, so the box only acknowledges (odd.png is 3 px: DIM-SMALL).
    let d = tempfile::tempdir().unwrap();
    let mut model = opened(d.path());
    model.decide(
        "landscape/odd.png",
        sdc_gui::model::Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/odd.png");
    let mut h = harness(model);
    h.run();
    h.get_by_label("Acknowledge for this texture");
    assert!(h.query_by_label("Resize to a power of two").is_none());
}

/// M6 Track E spec §2, §6: a sub-type decided by an accepted High suggestion reads "(auto)" next to it, with the
/// suggestion's reason as hover text.
#[test]
fn an_accepted_suggestion_reads_auto_with_its_reason_on_hover() {
    let d = tempfile::tempdir().unwrap();
    let mut model = opened_as_the_app(mod_folder(d.path()), d.path().join("export"));
    // Note FC1 (R1): the opening recorded the accepted types as project values; with that step undone (and not
    // recorded again) the type is the accepted suggestion once more.
    assert!(model
        .undo_label()
        .is_some_and(|l| l.ends_with("automatically")));
    model.undo();
    assert!(model.wait_idle(std::time::Duration::from_secs(60)));
    model.select_item("landscape/rock01.png");
    let mut h = harness(model);
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.run();
    assert!(
        h.query_by_label_contains("colour under textures/landscape")
            .is_none(),
        "the reason shows only on hover"
    );
    h.get_by_label("(auto)").hover();
    h.run();
    h.run();
    h.get_by_label_contains("colour under textures/landscape");
}

/// The opacity of the text shapes painted last frame whose text is exactly `text`.
fn text_opacity(h: &Harness<'static, AppModel>, text: &str) -> Vec<f32> {
    use eframe::egui::Shape;
    fn walk(s: &Shape, text: &str, out: &mut Vec<f32>) {
        match s {
            // `Ui::multiply_opacity` scales the shape's colours, not `opacity_factor`; a label's glyphs are the
            // placeholder colour, painted with the fallback.
            Shape::Text(t) if t.galley.text() == text => {
                let c = t.override_text_color.unwrap_or(t.fallback_color);
                out.push(c.a() as f32 / 255.0 * t.opacity_factor)
            }
            Shape::Vec(v) => v.iter().for_each(|x| walk(x, text, out)),
            _ => {}
        }
    }
    let mut out = vec![];
    for s in &h.output().shapes {
        walk(&s.shape, text, &mut out);
    }
    out
}

/// Note FM25: the columns show at full opacity the frame a folder opens, as a restored folder and the start screen do.
#[test]
fn opening_a_folder_shows_the_columns_at_full_opacity_at_once() {
    let small = |m: AppModel| {
        let h = Harness::builder()
            .with_size(eframe::egui::vec2(1280.0, 800.0))
            .with_step_dt(0.05)
            .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h
    };
    let d = tempfile::tempdir().unwrap();
    let mut h = small(opened(d.path()));
    h.step();
    // The header's "Sets" eye and the column heading, both at full alpha on frame 1 and after.
    let rest = text_opacity(&h, "Sets");
    assert!(
        rest.len() == 2 && rest.iter().all(|&x| x == 1.0),
        "restored: {rest:?}"
    );
    for _ in 0..6 {
        h.step();
    }
    assert_eq!(text_opacity(&h, "Sets"), rest, "restored: no fade");

    let root = mod_folder(&d.path().join("fresh"));
    let mut h = small(AppModel::new(GuiSettings::default(), None, session()));
    h.step();
    let a = text_opacity(&h, "Open folder…");
    assert!(
        !a.is_empty() && a.iter().all(|&x| x == 1.0),
        "start screen: {a:?}"
    );
    h.state_mut().open_folder(root);
    assert!(h.state_mut().wait_idle(std::time::Duration::from_secs(60)));
    h.step();
    let a = text_opacity(&h, "Sets");
    assert_eq!(a, rest, "no fade on opening");
}
