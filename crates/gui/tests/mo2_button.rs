//! Note AM: the scan source moved into one "MO2…" header button and its popup.
use egui_kittest::kittest::{NodeT as _, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::meshes::{MeshesModel, ScanTarget};
use sdc_gui::settings::MeshesSettings;
use std::path::Path;

/// An MO2 instance with two profiles (and a folder that is not one), as `meshes_view.rs` builds it.
fn instance() -> tempfile::TempDir {
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
    for p in ["Default", "Survival"] {
        std::fs::create_dir_all(r.join("profiles").join(p)).unwrap();
        std::fs::write(r.join("profiles").join(p).join("modlist.txt"), "").unwrap();
    }
    std::fs::create_dir_all(r.join("profiles/NotAProfile")).unwrap();
    d
}

fn model(root: &Path) -> MeshesModel {
    let saved = MeshesSettings {
        instance: Some(root.to_path_buf()),
        profile: Some("Default".into()),
        folder: None,
    };
    let mut m = MeshesModel::new(None, saved);
    m.procs = sdc_gui::model::meshes::no_procs;
    m.activate_with(None);
    m
}

#[test]
fn the_instance_lists_its_profiles_and_the_hover_names_both() {
    let d = instance();
    let m = model(d.path());
    assert_eq!(m.profiles(), ["Default", "Survival"]);
    let hover = m.source_text();
    assert!(hover.contains(&d.path().display().to_string()), "{hover}");
    assert!(hover.contains("Default"), "{hover}");
    assert_eq!(MeshesModel::empty().source_text(), "No source chosen");
}

#[test]
fn the_popup_picks_a_profile_and_stays_open_until_a_click_outside() {
    let d = instance();
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(900.0, 600.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| {
                ui.horizontal(|ui| sdc_gui::ui::meshes::mo2_button(ui, m));
                ui.add_space(300.0);
                ui.label("elsewhere");
            },
            model(d.path()),
        );
    h.run();
    h.get_by_label("MO2…").click();
    h.run();
    h.get_by_label("Choose…");
    h.get_by_label("Scan a folder…");
    h.get_by_label("Scan");
    h.get_by_label_contains(&d.path().display().to_string());
    h.get_by_label("Survival").click();
    h.run();
    assert!(
        matches!(&h.state().target, Some(ScanTarget::Mo2 { profile, .. }) if profile == "Survival")
    );
    // A click inside kept it open.
    h.get_by_label("Scan a folder…");
    h.get_by_label("elsewhere").click();
    h.run();
    assert!(h.query_by_label("Scan a folder…").is_none());
    let s = h.state_mut().take_settings().expect("the pick is saved");
    assert_eq!(s.profile.as_deref(), Some("Survival"));
}

/// A harness drawing the "MO2…" button with the popup open.
fn open_popup(m: MeshesModel) -> Harness<'static, MeshesModel> {
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(900.0, 600.0))
        .build_ui_state(
            |ui, m: &mut MeshesModel| {
                ui.horizontal(|ui| sdc_gui::ui::meshes::mo2_button(ui, m));
            },
            m,
        );
    h.run();
    h.get_by_label("MO2…").click();
    h.run();
    h
}

#[test]
fn with_no_instance_the_scan_row_says_choose_an_instance_first() {
    let h = open_popup(MeshesModel::empty());
    let row = h.get_by_label(sdc_gui::ui::meshes::SCAN_NEEDS_SOURCE);
    assert!(row.accesskit_node().is_disabled());
    assert!(h.query_by_label("Scan").is_none());
    drop(h);

    let d = instance();
    let h = open_popup(model(d.path()));
    assert!(!h.get_by_label("Scan").accesskit_node().is_disabled());
    assert!(h
        .query_by_label(sdc_gui::ui::meshes::SCAN_NEEDS_SOURCE)
        .is_none());
    assert!(h
        .query_by_label(sdc_gui::ui::meshes::SCAN_NEEDS_PROFILE)
        .is_none());
}

#[test]
fn a_profile_that_is_gone_says_choose_a_profile_first() {
    let d = instance();
    let mut m = model(d.path());
    m.use_instance(d.path().to_path_buf(), "Gone".into());
    let h = open_popup(m);
    let row = h.get_by_label(sdc_gui::ui::meshes::SCAN_NEEDS_PROFILE);
    assert!(row.accesskit_node().is_disabled());
    assert!(h.query_by_label("Scan").is_none());
}

/// The painted text of the label whose whole text is `full`.
fn painted(h: &Harness<'static, MeshesModel>, full: &str) -> bool {
    h.output().shapes.iter().any(|c| match &c.shape {
        eframe::egui::Shape::Text(t) => t.galley.job.text == full,
        _ => false,
    })
}

#[test]
fn while_a_scan_runs_choose_and_scan_a_folder_say_why() {
    let d = instance();
    let mut m = model(d.path());
    m.set_scanning_for_test(true);
    let mut h = open_popup(m);
    for row in ["Choose…", "Scan a folder…"] {
        assert!(h.get_by_label(row).accesskit_node().is_disabled(), "{row}");
        h.get_by_label(row).hover();
        h.run();
        assert!(
            painted(&h, sdc_gui::ui::meshes::SCAN_RUNNING_HOVER),
            "{row}: the hover says why"
        );
    }
}
