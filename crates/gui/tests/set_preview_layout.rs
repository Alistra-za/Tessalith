//! The set's 3D view inside the whole Textures view (no GPU, no scan DB). AG78: the 2D | 3D switch swaps the
//! centre pane in place; there is no side panel, and the 3D image stays inside the centre.
mod support;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use sdc_gui::model::set_preview::{SetPreview, DEFAULT_CAP};
use sdc_gui::model::viewport3d::FRAME_LABEL;
use sdc_gui::model::{pbr_export::json_entries, AppModel};
use sdc_gui::ui::set_preview::CENTRE_MIN;
use sdc_gui::ui::State;
use support::*;

type H = Harness<'static, (AppModel, State)>;

fn harness(d: &std::path::Path, collapsed: bool, w: f32, three_d: bool) -> H {
    harness_with(d, collapsed, w, three_d, |_| {})
}

fn harness_with(
    d: &std::path::Path,
    collapsed: bool,
    w: f32,
    three_d: bool,
    prep: impl FnOnce(&mut AppModel),
) -> H {
    let mut m = pbr_opened_plain(d);
    let key = json_entries(&m)
        .into_iter()
        .find(|(v, _)| v.id == "landscape/rock01")
        .unwrap()
        .0
        .key;
    m.select_set(&key);
    m.settings.textures_collapsed = collapsed;
    m.settings.sets_collapsed = collapsed;
    m.settings.set_view_3d = three_d;
    prep(&mut m);
    let state = State {
        set3d: SetPreview::new(None, DEFAULT_CAP),
        ..State::default()
    };
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(w, 800.0))
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, state),
        );
    for _ in 0..6 {
        h.run_steps(3);
        let (m, s) = h.state_mut();
        s.set3d.poll(&*m);
    }
    h
}

#[test]
fn clicking_another_sets_texture_waits_for_its_preview_poll() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(d.path(), false, 1400.0, true);
    h.get_by_label(FRAME_LABEL);
    let (m, state) = h.state_mut();
    state.set3d.poll(m);
    assert_eq!(state.set3d.fp.as_ref().unwrap().label, "landscape/rock01");
    h.get_by_label("leaf.png").click();
    h.run();
    assert_eq!(
        h.state().0.resolved_selection().view().unwrap().id,
        "landscape/leaf"
    );
    assert!(
        h.query_by_label(sdc_gui::ui::set_preview::PREPARING)
            .is_some(),
        "the new texture must wait for its own fingerprint"
    );
    assert!(h.query_by_label(FRAME_LABEL).is_none());
    let (m, state) = h.state_mut();
    state.set3d.poll(m);
    h.run();
    assert_eq!(
        h.state().1.set3d.fp.as_ref().unwrap().label,
        "landscape/leaf"
    );
    h.get_by_label(FRAME_LABEL);
}

fn paint_set_frame(h: &mut H, set: &str) -> egui::TextureId {
    use sdc_gui::model::viewport3d::Frame;
    let start = std::time::Instant::now();
    let (m, state) = h.state_mut();
    let s = &mut state.set3d;
    while s.v3.content().is_none() {
        assert!(
            start.elapsed() < std::time::Duration::from_secs(60),
            "{set}'s scene did not load"
        );
        s.shown = true;
        s.poll(m);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(s.fp.as_ref().unwrap().label, set);
    s.v3.frame = Some(std::sync::Arc::new(Frame {
        id: 73,
        width: 1,
        height: 1,
        rgba: vec![220, 40, 80, 255],
    }));
    s.enc.hold_workers(true);
    h.step();
    let (frame_id, texture) = h
        .ctx
        .data(|d| {
            d.get_temp::<(u64, egui::TextureHandle)>(sdc_gui::ui::viewport3d::frame_cache_id(
                "set_preview",
            ))
        })
        .unwrap();
    assert_eq!(frame_id, 73);
    assert!(paints_frame(h, texture.id()));
    texture.id()
}

fn paints_frame(h: &H, texture: egui::TextureId) -> bool {
    h.output()
        .shapes
        .iter()
        .any(|s| matches!(&s.shape, egui::Shape::Mesh(mesh) if mesh.texture_id == texture))
}

#[test]
fn the_new_sets_controls_never_paint_the_previous_frame() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness_with(d.path(), false, 1400.0, true, |m| {
        m.select_item("landscape/leaf.png");
    });
    let leaf = paint_set_frame(&mut h, "landscape/leaf");
    h.get_by_label("rock01.png").click();
    h.run_steps(3);
    let (m, state) = h.state_mut();
    state.set3d.poll(m);
    assert_eq!(state.set3d.fp.as_ref().unwrap().label, "landscape/rock01");
    assert!(
        !state.set3d.enc.settled(),
        "the new scene is still encoding"
    );
    h.step();
    h.get_by_label(FRAME_LABEL);
    assert!(
        !paints_frame(&h, leaf),
        "the viewport painted leaf's pixels under rock01's controls"
    );
}

#[test]
fn changing_the_target_keeps_the_same_sets_frame_while_encoding() {
    use engine::config::Target;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(d.path(), false, 1400.0, true);
    let rock = paint_set_frame(&mut h, "landscape/rock01");
    let (m, state) = h.state_mut();
    let preview = &mut state.set3d;
    let before = preview.fp.clone().unwrap();
    assert!(before.entry.is_some());
    m.set_target(Target::Vanilla);
    assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    preview.input = true;
    preview.poll(m);
    let after = preview.fp.as_ref().unwrap();
    assert_eq!(after.set_key, before.set_key);
    assert!(after.entry.is_none());
    assert!(!preview.enc.settled());
    h.step();
    h.get_by_label(FRAME_LABEL);
    assert!(
        paints_frame(&h, rock),
        "rock01's frame disappeared while its Vanilla preview was encoding"
    );
}

#[test]
fn selection_lifetime_reopening_a_scrolled_card_starts_at_the_top() {
    scroll_after_selection_change(false);
}

#[test]
fn selection_lifetime_a_change_in_3d_starts_the_next_2d_editor_at_the_top() {
    scroll_after_selection_change(true);
}

fn scroll_after_selection_change(in_3d: bool) {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(d.path(), false, 1400.0, false);
    h.set_size(egui::vec2(1400.0, 480.0));
    h.run_steps(4);
    let offset = |h: &H| {
        h.ctx
            .data(|d| d.get_temp::<f32>(egui::Id::unique("set-editor-observed-offset")))
            .unwrap()
    };
    h.get_by_label("Advanced TruePBR settings").scroll_to_me();
    h.run_steps(4);
    assert!(offset(&h) > 0.0);
    let id = if in_3d {
        h.get_by_label("3D").click_accesskit();
        h.run_steps(4);
        assert!(h.state().0.settings.set_view_3d);
        "landscape/leaf"
    } else {
        let column = support::panel(&h.ctx, "sets");
        h.query_all_by_label("landscape/rock01")
            .find(|n| column.contains(n.rect().center()))
            .unwrap()
            .click_modifiers(egui::Modifiers::COMMAND);
        h.run_steps(4);
        assert_eq!(h.state().0.selection, sdc_gui::model::Selection::None);
        "landscape/rock01"
    };
    let column = support::panel(&h.ctx, "sets");
    h.query_all_by_label(id)
        .find(|n| column.contains(n.rect().center()))
        .unwrap()
        .click();
    h.run_steps(4);
    assert_eq!(h.state().0.resolved_selection().view().unwrap().id, id);
    if in_3d {
        h.get_by_label("2D").click();
        h.run_steps(4);
    }
    assert_eq!(
        offset(&h),
        0.0,
        "the first 2D editor kept the previous offset"
    );
}

#[test]
fn selection_lifetime_a_second_folder_starts_its_borrowers_mesh_at_auto() {
    use render3d::builtin::Builtin;
    use sdc_gui::model::set_preview::MeshChoice;
    use sdc_gui::model::SetTarget;
    let d = tempfile::tempdir().unwrap();
    let id = "landscape/other";
    let project = |name| {
        let mut m = key_less_json_sets(&d.path().join(name));
        m.set_shader(id, Some(engine::shader::Shader::Pbr));
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        m.save().unwrap();
        m
    };
    let second = project("second").root.unwrap();
    let mut h = harness_on(project("first"), egui::vec2(1400.0, 900.0), |m| {
        m.select_target(SetTarget::Id(id.into()));
    });
    h.get_by_label("Preview mesh").click();
    h.run_steps(3);
    h.get_by_label("Cube").click();
    h.run_steps(3);
    assert_eq!(
        h.state().1.set3d.choice(),
        MeshChoice::Builtin(Builtin::Cube)
    );
    h.state_mut().0.open_folder(second);
    assert!(h
        .state_mut()
        .0
        .wait_idle(std::time::Duration::from_secs(60)));
    h.run_steps(3);
    let column = support::panel(&h.ctx, "sets");
    h.query_all_by_label(id)
        .find(|n| column.contains(n.rect().center()))
        .unwrap()
        .click();
    settle(&mut h);
    assert_eq!(
        h.state().0.selection,
        sdc_gui::model::Selection::SetId(id.into())
    );
    assert_eq!(h.state().1.set3d.choice(), MeshChoice::Auto);
}

#[test]
fn texture_follow_reuses_one_resolution_across_frames() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness_on(pbr_opened(d.path()), egui::vec2(1400.0, 900.0), |m| {
        m.select_item("landscape/rock01.png");
        m.settings.set_settings_open = true;
    });
    h.get_by_label(FRAME_LABEL);
    let active = h.state().0.resolved_selection();
    assert_eq!(active.view().unwrap().id, "landscape/rock01");
    let n = h.state().0.selection_builds.get();
    let work = sdc_gui::model::identity_work();
    for _ in 0..5 {
        h.step();
        let (m, state) = h.state_mut();
        state.set3d.poll(m);
    }
    assert_eq!(
        h.state().0.selection_builds.get(),
        n,
        "idle texture follow rebuilt identity"
    );
    assert_eq!(
        sdc_gui::model::identity_work(),
        work,
        "member owner was searched or copied"
    );
    assert!(std::sync::Arc::ptr_eq(
        &active,
        &h.state().0.resolved_selection()
    ));
    assert_eq!(
        h.state().1.set3d.fp.as_ref().unwrap().label,
        "landscape/rock01"
    );
}

#[test]
fn vanilla_member_settings_reuse_the_complex_material_card_decision() {
    use engine::config::Target;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness_on(pbr_opened(d.path()), egui::vec2(1400.0, 900.0), |m| {
        m.set_target(Target::Vanilla);
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
        m.select_item("landscape/leaf.png");
        m.settings.set_settings_open = true;
    });
    h.get_by_label("leaf.png settings");
    let (m, _) = h.state();
    let active = m.resolved_selection();
    let v = active.view().unwrap();
    assert_eq!(m.target, Target::Vanilla);
    assert_eq!(v.id, "landscape/leaf");
    assert!(!v.skin && !sdc_gui::model::sets::shows_json_panel(v, m.target));
    let reads = m.doc_block_reads.get();
    for _ in 0..5 {
        h.step();
        let (m, state) = h.state_mut();
        state.set3d.poll(m);
    }
    assert_eq!(
        h.state().0.doc_block_reads.get(),
        reads,
        "settings searched the document on idle frames"
    );
    h.state_mut().0.set_cm_export(&v.id, true);
    h.run();
    h.get_by_label(sdc_gui::ui::cm_card::SECTION);
}

#[test]
fn helper_follow_reuses_its_owner_in_the_settings_window() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness_on(pbr_opened(d.path()), egui::vec2(1400.0, 900.0), |m| {
        let key = "landscape/rock01_roughness.png";
        assert!(m.item(key).unwrap().helper.is_some());
        m.select_item(key);
        m.settings.set_settings_open = true;
    });
    h.get_by_label(FRAME_LABEL);
    h.get_by_label("rock01_roughness.png settings");
    let active = h.state().0.resolved_selection();
    assert_eq!(active.view().unwrap().id, "landscape/rock01");
    let n = h.state().0.selection_builds.get();
    let work = sdc_gui::model::identity_work();
    for _ in 0..5 {
        h.step();
        let (m, state) = h.state_mut();
        state.set3d.poll(m);
    }
    assert_eq!(
        h.state().0.selection_builds.get(),
        n,
        "idle texture follow rebuilt identity"
    );
    assert_eq!(
        sdc_gui::model::identity_work(),
        work,
        "member owner was searched or copied"
    );
    assert!(std::sync::Arc::ptr_eq(
        &active,
        &h.state().0.resolved_selection()
    ));
    assert_eq!(
        h.state().1.set3d.fp.as_ref().unwrap().label,
        "landscape/rock01"
    );
}

fn outer(h: &H, id: &str) -> Option<egui::Rect> {
    sdc_gui::ui::panel_ids::panel_rect(&h.ctx, id)
}

fn check(w: f32, collapsed: bool) {
    let d = tempfile::tempdir().unwrap();
    let h = harness(d.path(), collapsed, w, true);
    assert!(!panel_never_built(&h.ctx, "header"));
    assert!(panel_never_built(&h.ctx, "set-3d"), "no side panel at {w}");
    let left = ["textures", "sets", "textures-strip", "sets-strip"]
        .iter()
        .filter_map(|i| outer(&h, i))
        .map(|r| r.max.x)
        .fold(0.0f32, f32::max);
    // Nothing is right of the centre (Note JI7) but the harness' 8 px margin.
    let right = w - 8.0;
    assert!(
        right - left >= CENTRE_MIN - 0.5,
        "centre {} at {w}",
        right - left
    );
    // The frame takes the centre's width (less its margins), between the columns and the window edge.
    let img = h.get_by_label(FRAME_LABEL).rect();
    assert!(
        img.min.x >= left - 0.5,
        "image {img:?} over the columns at {left}"
    );
    assert!(
        img.max.x <= right + 0.5,
        "image {img:?} past the centre's {right}"
    );
    assert!(
        img.width() >= (right - left) - 24.0,
        "image {} wide in a centre of {}",
        img.width(),
        right - left
    );
    // The set editor's header row stays above it; its slot cards are gone.
    h.get_by_label("2D");
    assert!(h.query_by_label_contains("Match on this").is_none());
}

#[test]
fn narrow_window_expanded_columns() {
    check(900.0, false);
}

#[test]
fn wide_window_expanded_columns() {
    check(1600.0, false);
}

#[test]
fn narrow_window_collapsed_columns() {
    check(700.0, true);
}

#[test]
fn in_2d_the_centre_is_the_set_editor_and_no_3d_view_exists() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(d.path(), false, 1280.0, false);
    assert!(h.query_all_by_label_contains("Match on this").count() >= 1);
    assert!(h.query_by_label(FRAME_LABEL).is_none());
    assert!(!panel_never_built(&h.ctx, "header"));
    assert!(panel_never_built(&h.ctx, "set-3d"));
}

/// AG87 (owner: "Keep 3D up", fix round 2): with 3D on, any set member shows its set's 3D view with its settings
/// folded above it, another set's member too; 2D brings the normal texture view back.
#[test]
fn a_member_keeps_the_3d_view_and_2d_shows_its_normal_view() {
    use sdc_gui::model::Selection;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(d.path(), false, 1400.0, true);
    h.get_by_label(FRAME_LABEL);
    let m = &h.state().0;
    let active = m.resolved_selection();
    let set = &active.view().expect("keyed fixture").key;
    let member = m
        .items
        .iter()
        .find(|i| i.set_key.as_deref() == Some(set.as_str()))
        .unwrap();
    let (key, name) = (member.key.clone(), member.name.clone());
    let other = m
        .items
        .iter()
        .find(|i| i.set_key.is_some() && i.set_key.as_deref() != Some(set.as_str()))
        .map(|i| (i.key.clone(), i.name.clone()));
    // AG132: the member's own settings are in the settings window, titled with its name.
    h.state_mut().0.settings.set_settings_open = true;
    h.state_mut().0.selection = Selection::Item(key);
    h.run_steps(3);
    h.get_by_label(FRAME_LABEL);
    h.get_by_label(&format!("{name} settings"));
    h.get_by_label("3D");
    if let Some((k, n)) = other {
        h.state_mut().0.selection = Selection::Item(k);
        let (m, state) = h.state_mut();
        state.set3d.poll(m);
        h.run_steps(3);
        h.get_by_label(FRAME_LABEL);
        h.get_by_label(&format!("{n} settings"));
    }
    h.state_mut().0.settings.set_view_3d = false;
    h.run_steps(3);
    assert!(h.query_by_label(FRAME_LABEL).is_none(), "2D: no 3D view");
}

/// Fix round 1, item 4: the coat group (under Multilayer, Note AJ) is in the 3D layout's TruePBR settings too.
#[test]
fn the_3d_layout_truepbr_settings_show_the_coat_group() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness_with(d.path(), false, 1400.0, true, |m| {
        m.set_json_value(
            "landscape/rock01",
            "multilayer",
            Some(serde_json::json!(true)),
        );
    });
    h.get_by_label(sdc_gui::ui::set_preview::SETTINGS_BUTTON)
        .click();
    h.run_steps(3);
    h.get_by_label("Coat");
    h.get_by_label("Coat colour");
    h.get_by_label(FRAME_LABEL);
}

/// Fix round 2 (owner): with 3D on, a member clicked straight from the Textures column opens its set's 3D view,
/// even though the set itself was never selected.
#[test]
fn a_member_selected_first_opens_its_sets_3d_view() {
    use sdc_gui::model::Selection;
    let d = tempfile::tempdir().unwrap();
    let mut name = String::new();
    let h = harness_with(d.path(), false, 1400.0, true, |m| {
        let active = m.resolved_selection();
        let set = &active.view().expect("keyed fixture").key;
        let i = m
            .items
            .iter()
            .find(|i| i.set_key.as_deref() == Some(set.as_str()))
            .unwrap();
        name = i.name.clone();
        m.selection = Selection::Item(i.key.clone());
        m.settings.set_settings_open = true;
    });
    h.get_by_label(FRAME_LABEL);
    h.get_by_label(&format!("{name} settings"));
}

// ---- AG132: the compact 3D layout ----

fn harness_at(d: &std::path::Path, size: egui::Vec2, prep: impl FnOnce(&mut AppModel)) -> H {
    harness_on(pbr_opened_plain(d), size, prep)
}

/// `harness_at` on a model of the caller's (Note BM: `pbr_opened` is a PBR Landscape set, previewed on the plane).
fn harness_on(mut m: AppModel, size: egui::Vec2, prep: impl FnOnce(&mut AppModel)) -> H {
    let key = json_entries(&m)
        .into_iter()
        .find(|(v, _)| v.id == "landscape/rock01")
        .unwrap()
        .0
        .key;
    m.select_set(&key);
    m.settings.set_view_3d = true;
    prep(&mut m);
    let state = State {
        set3d: SetPreview::new(None, DEFAULT_CAP),
        ..State::default()
    };
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(size)
        .build_ui_state(
            |ui, (m, s): &mut (AppModel, State)| sdc_gui::ui::draw_with(ui, m, s),
            (m, state),
        );
    settle(&mut h);
    h
}

fn settle(h: &mut H) {
    for _ in 0..6 {
        h.run_steps(3);
        let (m, s) = h.state_mut();
        s.set3d.poll(&*m);
    }
}

#[test]
fn in_3d_the_viewport_takes_most_of_the_centre() {
    // Note BM (review M4): the PBR Landscape fixture (plane + Tiling) keeps the rule too.
    for (w, hh, landscape) in [
        (1400.0, 900.0, false),
        (1024.0, 768.0, false),
        (1024.0, 768.0, true),
    ] {
        let d = tempfile::tempdir().unwrap();
        let m = if landscape {
            pbr_opened(d.path())
        } else {
            pbr_opened_plain(d.path())
        };
        let h = harness_on(m, egui::vec2(w, hh), |_| {});
        let top = ["header", "status"]
            .iter()
            .filter_map(|i| outer(&h, i))
            .map(|r| r.max.y)
            .fold(0.0f32, f32::max);
        // The bottom panel (open) or its strip (collapsed) takes the bottom of the window.
        let bottom = ["bottom", "bottom-strip"]
            .iter()
            .filter_map(|i| outer(&h, i))
            .map(|r| r.min.y)
            .fold(hh, f32::min);
        let img = h.get_by_label(FRAME_LABEL).rect();
        let centre = bottom - top;
        assert!(
            img.height() >= 0.72 * centre,
            "{w}x{hh} (landscape {landscape}): frame {} high in a centre of {centre}",
            img.height()
        );
        // The compact layout: no heading, no second path line.
        assert!(h.query_by_label("Set preview (3D)").is_none());
    }
}

#[test]
fn the_3d_settings_edit_the_selected_document_card() {
    let d = tempfile::tempdir().unwrap();
    let id = "landscape/other";
    let mut h = harness_on(
        key_less_json_sets(d.path()),
        egui::vec2(1400.0, 900.0),
        |m| {
            m.select_target(sdc_gui::model::SetTarget::Id(id.into()));
            m.settings.set_settings_open = true;
        },
    );
    h.get_by_label(FRAME_LABEL);
    h.get_by_label("TruePBR settings");
    h.get(
        egui_kittest::kittest::by()
            .role(egui::accesskit::Role::CheckBox)
            .label("Emissive"),
    )
    .click();
    settle(&mut h);
    let entries = h.state().0.sets_doc.as_ref().unwrap().entries();
    let value = |id: &str| {
        entries
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .settings
            .get("emissive")
    };
    assert_eq!(value(id), Some(&serde_json::json!(true)));
    assert_eq!(value("landscape/empty"), None);
}

#[test]
fn the_settings_window_toggles_and_its_edits_reach_the_preview() {
    use sdc_gui::ui::set_preview::SETTINGS_BUTTON;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness_at(d.path(), egui::vec2(1400.0, 900.0), |_| {});
    assert!(h.query_by_label("Specular level").is_none(), "closed");
    h.get_by_label(SETTINGS_BUTTON).click();
    settle(&mut h);
    assert!(h.state().0.settings.set_settings_open, "remembered");
    h.get_by_label("TruePBR settings");
    widget(&h, egui::accesskit::Role::SpinButton, "Specular level");
    let before = h.state().1.set3d.fp.clone().expect("a fingerprint");
    h.get(
        egui_kittest::kittest::by()
            .role(egui::accesskit::Role::CheckBox)
            .label("Emissive"),
    )
    .click();
    settle(&mut h);
    let after = h.state().1.set3d.fp.clone().expect("a fingerprint");
    assert_ne!(before.entry, after.entry, "the preview follows the edit");
    // The toggle closes it.
    h.get_by_label(SETTINGS_BUTTON).click();
    settle(&mut h);
    assert!(!h.state().0.settings.set_settings_open);
    assert!(h.query_by_label("Specular level").is_none());
}

/// Note FM31 (supersedes CE21's creep shortcut): a saved W×H comes back as W×H and is saved again unchanged (it grew
/// 3 points per start). The layout takes off whatever egui draws past `default_rect`; here that is nothing.
#[test]
fn a_saved_window_size_reopens_at_the_same_size() {
    for saved in [[200.0, 100.0, 700.0, 500.0], [120.0, 120.0, 460.0, 620.0]] {
        let d = tempfile::tempdir().unwrap();
        let h = harness_at(d.path(), egui::vec2(1400.0, 900.0), |m| {
            m.settings.set_settings_open = true;
            m.settings.layout.settings_window = Some(saved);
        });
        let drawn = h
            .ctx
            .memory(|m| m.area_rect(sdc_gui::ui::set_preview::settings_window_id()))
            .expect("the settings window is drawn");
        // Was "drawn wider than saved" (706×506 and 466×626 on egui 0.36.2). That excess came from the harness: the
        // system fonts went in after kittest's first frames, and while no text had glyphs the window grew 1 point a
        // frame, which `Resize` keeps (`egui-0.36.2\src\containers\resize.rs:269`). The fonts now go in before the
        // first frame (egui_kittest 0.37 lays out text there), so the window holds its `default_rect`.
        assert_eq!(
            drawn.size(),
            egui::vec2(saved[2], saved[3]),
            "drawn at the size asked: {drawn:?}"
        );
        let r = sdc_gui::layout::settings_window_now(&h.ctx).expect("the size has settled");
        assert_eq!(
            [r.min.x, r.min.y, r.width(), r.height()],
            saved,
            "saved W×H, saved again W×H"
        );
    }
}

/// Note FM31 with an excess: a saved width a few points under what the window's content needs (262.1 here) is drawn
/// at that width. The excess comes off what is saved, so the same narrow size is saved again and the next start draws
/// the same rect; an excess past `FIT_MAX_EXTRA` (16 points), here a saved width far under the content's, takes
/// nothing off.
#[test]
fn a_saved_size_under_the_content_width_takes_the_excess_off() {
    let open = |saved: [f32; 4]| {
        let d = tempfile::tempdir().unwrap();
        let h = harness_at(d.path(), egui::vec2(1400.0, 900.0), |m| {
            m.settings.set_settings_open = true;
            m.settings.layout.settings_window = Some(saved);
        });
        let drawn = h
            .ctx
            .memory(|m| m.area_rect(sdc_gui::ui::set_preview::settings_window_id()))
            .expect("the settings window is drawn");
        let r = sdc_gui::layout::settings_window_now(&h.ctx).expect("the size has settled");
        (drawn, [r.min.x, r.min.y, r.width(), r.height()])
    };
    let asked = [200.0, 100.0, 255.0, 500.0];
    let (drawn, saved) = open(asked);
    let extra = drawn.width() - asked[2];
    assert!(
        extra > 0.0 && extra <= 16.0,
        "drawn a few points wider than asked: {drawn:?}"
    );
    assert_eq!(saved, asked, "the excess comes off what is saved");
    let (again, saved_again) = open(saved);
    assert_eq!(again, drawn, "the next start draws the same rect");
    assert_eq!(saved_again, asked, "and saves the same size");

    // 62 points past a 200-point width (the content's minimum): over `FIT_MAX_EXTRA`, saved as drawn.
    let (wide, saved) = open([200.0, 100.0, 200.0, 500.0]);
    assert!(wide.width() - 200.0 > 16.0, "{wide:?}");
    assert_eq!(
        saved[2],
        wide.width(),
        "nothing comes off past FIT_MAX_EXTRA"
    );
}

#[test]
fn details_is_collapsed_by_default_and_holds_the_tech_lines() {
    use sdc_gui::model::set_preview::{cap_text, JSON_WINS};
    let d = tempfile::tempdir().unwrap();
    let mut h = harness_at(d.path(), egui::vec2(1400.0, 900.0), |_| {});
    h.get_by_label("Details");
    for t in [JSON_WINS, sdc_gui::model::viewport3d::FEATURES_LABEL] {
        assert!(h.query_by_label(t).is_none(), "{t} is in Details");
    }
    assert!(h.query_by_label(&cap_text(2048)).is_none());
    let tall = h.get_by_label(FRAME_LABEL).rect().height();
    h.get_by_label("Details").click();
    h.run_steps(3);
    assert!(sdc_gui::ui::viewport3d::details_open(
        &h.ctx,
        sdc_gui::ui::viewport3d::SET_DETAILS
    ));
    h.get_by_label(JSON_WINS);
    h.get_by_label(&cap_text(2048));
    assert!(
        h.query_by_label("Timings").is_none(),
        "the Timings panel is gone"
    );
    h.get_by_label(sdc_gui::model::viewport3d::FEATURES_LABEL);
    assert!(
        h.get_by_label(FRAME_LABEL).rect().height() < tall,
        "the frame makes room"
    );
}

#[test]
fn warnings_stay_above_the_viewport_outside_details() {
    use sdc_gui::model::set_preview::{EncState, NEEDS_CS};
    let d = tempfile::tempdir().unwrap();
    let mut h = harness_at(d.path(), egui::vec2(1400.0, 900.0), |m| {
        m.set_target(engine::config::Target::Vanilla);
        assert!(m.wait_idle(std::time::Duration::from_secs(60)));
    });
    // Not a PGPatcher simulation: its notice is a warning, shown with Details closed.
    let frame = h.get_by_label(FRAME_LABEL).rect();
    assert!(h.get_by_label(NEEDS_CS).rect().max.y <= frame.min.y + 0.5);
    // A failed map, the same: that member's own line.
    let label = {
        let mem = h
            .state_mut()
            .1
            .set3d
            .enc
            .members
            .values_mut()
            .next()
            .expect("the set has a member");
        mem.state = EncState::Failed("boom".into());
        format!("{}: {}", mem.slot.kind.label(), mem.state.label())
    };
    h.run_steps(3);
    let failed = h.get_by_label(&label).rect();
    // The failure line sits above the frame (the off line is one truncated row): compare with where the frame is now.
    let frame = h.get_by_label(FRAME_LABEL).rect();
    assert!(failed.max.y <= frame.min.y + 0.5);
    assert!(!sdc_gui::ui::viewport3d::details_open(
        &h.ctx,
        sdc_gui::ui::viewport3d::SET_DETAILS
    ));
}

/// Note JJ1 (B4 of the Settings-plan UI proof): at 1100x800 the toolbar's "Mesh: Auto (…)" combo stays inside the
/// centre; it ran past the window's right edge.
#[test]
fn the_mesh_combo_stays_inside_the_window_at_1100() {
    use egui::accesskit::Role;
    let d = tempfile::tempdir().unwrap();
    let (w, hh) = (1100.0, 800.0);
    let h = harness_at(d.path(), egui::vec2(w, hh), |_| {});
    let combo = h
        .query_all_by_role(Role::ComboBox)
        .find(|n| n.value().is_some_and(|v| v.starts_with("Mesh:")))
        .expect("the mesh combo is on the toolbar");
    let r = combo.rect();
    // Nothing is right of the centre but the harness' 8 px margin (as `check`).
    let right = (w - 8.0).min(h.ctx.content_rect().max.x);
    assert!(
        r.max.x <= right + 0.5,
        "the mesh combo {r:?} runs past the window's {right}"
    );
}

/// Note JJ1: a mesh label wider than the toolbar row (a scanned NIF's path, origin and shape count) truncates inside
/// the window, and the combo's accessible value keeps the whole text.
#[test]
fn a_long_mesh_label_truncates_inside_the_window() {
    use egui::accesskit::Role;
    use sdc_gui::model::set_preview::{Candidate, Candidates};
    let d = tempfile::tempdir().unwrap();
    let (w, hh) = (1100.0, 800.0);
    let mut h = harness_at(d.path(), egui::vec2(w, hh), |_| {});
    let nif = r"meshes\architecture\whiterun\wrbuildings\wrhouse01longnamevariantwithmanyparts.nif";
    h.state_mut().1.set3d.candidates = Candidates::Ready(vec![Candidate {
        load_copy_id: 1,
        copy_id: 1,
        nif_path: nif.into(),
        origin: "Some Long Mod Name - Whiterun Overhaul".into(),
        le_mesh: false,
        derived: false,
        shapes: vec![(0, None)],
    }]);
    h.run_steps(3);
    let combo = h
        .query_all_by_role(Role::ComboBox)
        .find(|n| n.value().is_some_and(|v| v.starts_with("Mesh:")))
        .expect("the mesh combo is on the toolbar");
    let value = combo.value().unwrap();
    assert!(value.contains(nif), "the whole label is the value: {value}");
    let r = combo.rect();
    let right = (w - 8.0).min(h.ctx.content_rect().max.x);
    assert!(
        r.max.x <= right + 0.5,
        "the mesh combo {r:?} runs past the window's {right}"
    );
}
