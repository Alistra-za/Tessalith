//! Note JI1: the Settings centre tab. "This project" (Mode, Preset, export folder) above "App settings" (Preview,
//! Checks, Cache, Columns); it opens from the header's "Settings…" as a centre tab, as Presets does (Note FB7).
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{by, NodeT as _, Queryable};
use egui_kittest::Harness;
use sdc_gui::model::{AppModel, CentreTab};
use sdc_gui::settings::GuiSettings;
use std::time::Duration;
use support::*;

type H = Harness<'static, AppModel>;

const PROJECT_HEADING: &str = "This project: saved in this folder's sdc.toml and sdc-project.toml";
const APP_HEADING: &str = "App settings: these apply to every project";

fn harness_sized(m: AppModel, size: egui::Vec2) -> H {
    let mut h = Harness::builder()
        .with_size(size)
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h
}

fn harness(m: AppModel, width: f32) -> H {
    harness_sized(m, egui::vec2(width, 1000.0))
}

/// The header's "Settings…", as a user opens the tab.
fn open_from_header(h: &mut H) {
    h.get_by_role_and_label(Role::Button, "Settings…").click();
    h.run();
    h.run();
}

fn mode_name(target: &str) -> String {
    format!("Mode (this project): {target}")
}

/// The text of a combo: its value, or the label egui gives it (the selected text).
fn combo_text(n: &egui_kittest::Node<'_>) -> Option<String> {
    let a = n.accesskit_node();
    a.value().or(a.label()).map(|v| v.to_string())
}

/// The header's Preset combo shows `default (…)` or a preset id; the Settings tab's is named "Preset (this project): …".
fn header_preset_text(h: &H) -> String {
    h.query_all_by_role(Role::ComboBox)
        .filter_map(|n| combo_text(&n))
        .find(|v| v.starts_with("default ("))
        .expect("the header's Preset combo")
}

#[test]
fn settings_opens_as_a_centre_tab_from_the_header() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()), 1400.0);
    h.get_by_role_and_label(Role::Button, "Settings…").click();
    h.run();
    assert_eq!(h.state().centre_front(), CentreTab::Settings);
    assert_in_centre(&h, "Settings tab");
    assert_in_centre(&h, "Pop out Settings");
    assert_eq!(windows(&h), 0);
}

#[test]
fn the_settings_tab_holds_exactly_four_groups() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.open_settings_tab();
    let h = harness(m, 1400.0);
    let c = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    let project = h.get_by_label(PROJECT_HEADING).rect();
    let app = h.get_by_label(APP_HEADING).rect();
    assert!(project.bottom() <= app.top(), "{project:?} {app:?}");
    // The group titles: the labels at the first title's x under the App heading (a group's body is indented less
    // than its title, which follows the collapse arrow), in y order. A fifth group would add a fifth.
    let below_app = |r: egui::Rect| c.contains_rect(r) && r.top() >= app.bottom();
    let x0 = h
        .query_all_by_label("Preview")
        .map(|n| n.rect())
        .find(|r| below_app(*r))
        .expect("the Preview group")
        .left();
    let mut titles: Vec<(f32, String)> = h
        .query_all_by_role(Role::Label)
        .filter(|n| below_app(n.rect()) && (n.rect().left() - x0).abs() < 1.0)
        .filter_map(|n| {
            let a = n.accesskit_node();
            a.value()
                .or(a.label())
                .map(|v| (n.rect().top(), v.to_string()))
        })
        .collect();
    titles.sort_by(|a, b| a.0.total_cmp(&b.0));
    let names: Vec<&str> = titles.iter().map(|t| t.1.as_str()).collect();
    assert_eq!(names, ["Preview", "Checks", "Cache", "Columns"]);
    // Each group holds its controls: between its title and the next one.
    let ys: Vec<f32> = titles.iter().map(|t| t.0).chain([f32::INFINITY]).collect();
    let groups: [&[(Role, &str)]; 4] = [
        &[(Role::CheckBox, "Live encode setting")],
        &[
            (Role::CheckBox, "Low detail check"),
            (Role::CheckBox, "Green direction check"),
            (Role::CheckBox, "Parallax check"),
        ],
        &[(Role::SpinButton, "Analysis cache limit")],
        &[
            (Role::CheckBox, "Textures column"),
            (Role::CheckBox, "Sets column"),
        ],
    ];
    for (i, controls) in groups.iter().enumerate() {
        for (role, name) in *controls {
            let r = h.get_by_role_and_label(*role, name).rect();
            assert!(
                r.top() > ys[i] && r.top() < ys[i + 1] && c.contains_rect(r),
                "{name} {r:?} is not in the {} group",
                names[i]
            );
        }
    }
    // This project: Mode, Preset and the export folder, between the two headings.
    let in_project = |r: egui::Rect| r.top() >= project.bottom() && r.bottom() <= app.top();
    let mode = h
        .get_by_role_and_label(Role::ComboBox, &mode_name("cs"))
        .rect();
    assert!(in_project(mode), "{mode:?}");
    let preset: Vec<_> = h
        .query_all_by_role(Role::ComboBox)
        .filter(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Preset (this project): "))
        })
        .collect();
    assert_eq!(preset.len(), 1, "one Settings Preset combo");
    assert!(in_project(preset[0].rect()), "{:?}", preset[0].rect());
    for b in ["Choose export folder", "Settings: Open export folder"] {
        let r = h.get_by_role_and_label(Role::Button, b).rect();
        assert!(in_project(r), "{b} {r:?}");
    }
}

#[test]
fn no_project_open_shows_no_project_controls() {
    let mut m = AppModel::placeholder(GuiSettings::default());
    m.open_settings_tab();
    let h = harness(m, 1280.0);
    h.get_by_label(PROJECT_HEADING);
    h.get_by_label("No project open");
    assert!(h.query_by_label_contains("Mode (this project)").is_none());
    assert!(h.query_by_label_contains("Preset (this project)").is_none());
    assert!(h.query_by_label("Choose export folder").is_none());
    assert!(h.query_by_label("Settings: Open export folder").is_none());
    h.get_by_label("Live encode setting"); // app settings still there
}

#[test]
fn the_settings_preset_combo_sets_the_project_preset() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let default = engine::presets::default_preset_for(m.target);
    let want = m
        .preset_choices()
        .into_iter()
        .find(|id| id != default)
        .expect("a preset other than the default");
    m.open_settings_tab();
    let mut h = harness(m, 1400.0);
    h.query_all_by_role(Role::ComboBox)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Preset (this project): "))
        })
        .expect("the Settings Preset combo")
        .click();
    h.run();
    h.get_by_role_and_label(Role::Button, &want).click();
    h.run();
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run();
    assert_eq!(h.state().project_preset(), Some(want.clone()));
    assert_eq!(h.state().undo_label(), Some("Preset"));
    h.get_by_role_and_label(Role::ComboBox, &format!("Preset (this project): {want}"));
    // The header's Preset combo shows the same value.
    let header = egui::containers::panel::PanelState::load(&h.ctx, egui::Id::new("header"))
        .expect("the header")
        .outer_rect;
    let shown: Vec<String> = h
        .query_all_by_role(Role::ComboBox)
        .filter(|n| header.contains_rect(n.rect()))
        .filter_map(|n| combo_text(&n))
        .collect();
    assert_eq!(shown, [want]);
}

/// Review Focus 3: a long export path at 1100 px is cut in the middle, stays inside the centre, and keeps its whole
/// text in the accessible name (rules 3.4, 8.1).
#[test]
fn a_long_export_path_stays_inside_at_1100() {
    let d = tempfile::tempdir().unwrap();
    let segs: Vec<String> = (0..8).map(|i| format!("{i}{}", "x".repeat(24))).collect();
    let long = segs.iter().fold(d.path().to_path_buf(), |p, s| p.join(s));
    // The project is opened once per width: 1100 is the focus, 1280 and 1600 the other capture widths.
    for width in [1100.0f32, 1280.0, 1600.0] {
        let mut m = pbr_opened(&d.path().join(format!("w{width}")));
        m.set_export_folder(long.clone());
        assert!(m.wait_idle(Duration::from_secs(60)));
        m.open_settings_tab();
        let full = m
            .export_folder_chosen()
            .expect("the folder is chosen")
            .display()
            .to_string();
        let mut h = harness_sized(m, egui::vec2(width, 800.0));
        h.get_by_label(&format!("Export folder: {full}"));
        let c = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
        assert_in_centre(&h, "Choose export folder");
        assert_in_centre(&h, "Settings: Open export folder");
        // Every node that starts right of the centre's left edge, at the centre's height, ends inside it: one pushed
        // wholly past the right edge counts too.
        let mut seen = 0;
        for n in h.query_all(by().predicate(|n| n.bounding_box().is_some())) {
            let (role, r) = (n.accesskit_node().role(), n.rect());
            if !matches!(
                role,
                Role::Label | Role::Button | Role::ComboBox | Role::CheckBox | Role::SpinButton
            ) || r.left() < c.left() - 0.5
                || r.center().y < c.top()
                || r.center().y > c.bottom()
            {
                continue;
            }
            seen += 1;
            assert!(
                r.right() <= c.right() + 0.5,
                "at {width}: {role:?} {:?} {r:?} runs past the centre {c:?}",
                n.accesskit_node().label().or(n.accesskit_node().value())
            );
        }
        assert!(seen > 10, "at {width}: the tab's nodes were read: {seen}");
        let last = segs.last().unwrap();
        let shown: Vec<(String, egui::Rect)> = h
            .query_all_by_role(Role::Label)
            .filter_map(|n| {
                let v = n.accesskit_node().value()?.to_string();
                Some((v, n.rect()))
            })
            // The row's label carries the accessible name; the path's own label holds the text drawn.
            .filter(|(v, _)| v.ends_with(last.as_str()) && !v.starts_with("Export folder: "))
            .collect();
        assert_eq!(shown.len(), 1, "at {width}: one visible path: {shown:?}");
        let text = &shown[0].0;
        let drive = full.split(['\\', '/']).next().unwrap();
        assert!(text.starts_with('…') || text.starts_with(drive), "{text}");
        assert!(text.contains('…'), "at {width}: cut in the middle: {text}");
        assert!(
            text.chars().count() < full.chars().count(),
            "{text} vs {full}"
        );
        // The whole path on hover, once (the cut label adds no tooltip of its own).
        let whole = |h: &H| {
            h.query_all_by_role(Role::Label)
                .filter(|n| n.accesskit_node().value().as_deref() == Some(full.as_str()))
                .count()
        };
        assert_eq!(whole(&h), 0, "no tooltip before the hover");
        h.hover_at(shown[0].1.center());
        h.run();
        assert_eq!(whole(&h), 1, "at {width}: the whole path on hover, once");
    }
}

/// Note JI1: with no folder chosen the row says so and Open is off with the reason; Choose… picks the folder and
/// Open then opens it.
#[test]
fn the_export_folder_buttons_choose_and_open() {
    use sdc_gui::model::picker::{PickerBackend, PickerSpec};
    use sdc_gui::model::shell::ShellRequest;
    struct Pick(std::path::PathBuf);
    impl PickerBackend for Pick {
        fn pick(&self, _: &PickerSpec) -> Option<std::path::PathBuf> {
            Some(self.0.clone())
        }
    }
    let d = tempfile::tempdir().unwrap();
    let mut m = opened_unchosen(d.path());
    let out = d.path().join("picked");
    m.picker.set_backend(std::sync::Arc::new(Pick(out.clone())));
    m.open_settings_tab();
    let mut h = harness(m, 1400.0);
    assert_in_centre(&h, "Not chosen");
    h.get_by_label("Export folder: not chosen");
    let open = h.get_by_role_and_label(Role::Button, "Settings: Open export folder");
    assert!(open.accesskit_node().is_disabled());
    open.hover();
    h.run();
    h.get_by_label("No export folder chosen yet");

    h.get_by_role_and_label(Role::Button, "Choose export folder")
        .click();
    h.run();
    let end = std::time::Instant::now() + Duration::from_secs(10);
    while h.state().picker.busy() {
        assert!(std::time::Instant::now() < end, "the dialog never closed");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(h.state_mut().poll());
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    assert_eq!(h.state().export_folder_chosen(), Some(out.clone()));

    std::fs::create_dir_all(&out).unwrap();
    h.run();
    h.get_by_label(&format!("Export folder: {}", out.display()));
    let open = h.get_by_role_and_label(Role::Button, "Settings: Open export folder");
    assert!(!open.accesskit_node().is_disabled());
    open.click();
    h.run();
    assert_eq!(h.state_mut().take_shell(), vec![ShellRequest::Open(out)]);
}

#[test]
fn app_settings_save_to_gui_toml_at_once() {
    use eframe::egui::{Event, Key, Modifiers};
    fn spin<'a>(h: &'a H) -> egui_kittest::Node<'a> {
        h.get_by_role_and_label(Role::SpinButton, "Analysis cache limit")
    }
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    let toml = d.path().join("settings").join("gui.toml");
    m.settings_path = Some(toml.clone());
    m.open_settings_tab();
    let (live, low) = (m.settings.live_encode, m.settings.checks.low_detail);
    assert!(!m.settings.textures_hidden);
    let mut h = harness(m, 1400.0);
    assert_eq!(spin(&h).accesskit_node().numeric_value(), Some(500.0));

    // Note HV2: a limit from gui.toml outside the box's range is left as it is by drawing, and by leaving the box.
    h.state_mut().settings.analysis_cache_mb = Some(7);
    h.run();
    assert_eq!(spin(&h).accesskit_node().numeric_value(), Some(7.0));
    spin(&h).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.run();
    assert_eq!(h.state().settings.analysis_cache_mb, Some(7));
    assert!(!toml.exists(), "drawing and leaving the box save nothing");

    // Each edit is on disk before the next one: no later save carries an earlier value.
    let saved = || GuiSettings::load(&toml).0;
    h.get_by_role_and_label(Role::CheckBox, "Live encode setting")
        .click();
    h.run();
    assert_eq!(saved().live_encode, !live, "Live encode saved at once");
    h.get_by_role_and_label(Role::CheckBox, "Low detail check")
        .click();
    h.run();
    assert_eq!(saved().checks.low_detail, !low, "Low detail saved at once");
    // The Columns tick is a check box ticked while its column shows (Note BH15's eye, now a tick).
    let toggled = |h: &H| {
        h.get_by_role_and_label(Role::CheckBox, "Textures column")
            .accesskit_node()
            .toggled()
    };
    assert_eq!(toggled(&h), Some(egui::accesskit::Toggled::True));
    h.get_by_role_and_label(Role::CheckBox, "Textures column")
        .click();
    h.run();
    assert_eq!(toggled(&h), Some(egui::accesskit::Toggled::False));
    assert!(saved().textures_hidden, "the Textures column saved at once");
    assert!(!saved().sets_hidden);
    spin(&h).focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    h.event(Event::Text("800".into()));
    h.run();
    assert_eq!(
        h.state().settings.analysis_cache_mb,
        Some(7),
        "not while typing"
    );
    h.key_press(Key::Enter);
    h.run();
    h.run();
    let saved = GuiSettings::load(&toml).0;
    assert_eq!(saved.live_encode, !live);
    assert_eq!(saved.checks.low_detail, !low);
    assert!(saved.textures_hidden);
    assert_eq!(saved.analysis_cache_mb, Some(800));
}

/// Moved from the Presets tab (Note FI16 → JI1): the project's Mode is set in the Settings tab.
#[test]
fn the_settings_tab_sets_the_project_mode() {
    use engine::config::Target;
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pbr_opened(d.path()), 1400.0);
    // A preset draft open in the creator stays untouched by a Mode change (Review Focus 7 of Note FI16).
    h.state_mut().presets_model().select("cs-quality");
    open_from_header(&mut h);
    h.get_by_label(&mode_name("cs")).click();
    h.run();
    h.get_by_role_and_label(Role::Button, "vanilla").click();
    h.run();
    // The target the plan reports lands with the re-plan.
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run();
    assert_eq!(h.state().target, Target::Vanilla);
    let text = h.state().project.as_ref().unwrap().text();
    assert!(text.contains("target = \"vanilla\""), "{text}");
    assert_eq!(h.state().undo_label(), Some("Mode"));
    assert!(!h.state_mut().presets_model().dirty());
    h.get_by_label(&mode_name("vanilla"));
    let want = format!(
        "default ({})",
        engine::presets::default_preset_for(Target::Vanilla)
    );
    assert_eq!(header_preset_text(&h), want);
    h.state_mut().undo();
    assert!(h.state_mut().wait_idle(Duration::from_secs(60)));
    h.run();
    assert_eq!(h.state().target, Target::Cs);
    h.get_by_label(&mode_name("cs"));
}

#[test]
fn the_mode_row_is_off_when_read_only() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    std::fs::write(d.path().join("textures/sdc.toml"), "this is = [not toml").unwrap();
    m.reload_documents();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(m.project.as_ref().unwrap().read_only());
    let mut h = harness(m, 1400.0);
    open_from_header(&mut h);
    let name = mode_name("cs");
    let combo = h.get_by_label(&name);
    assert!(combo.accesskit_node().is_disabled());
    combo.hover();
    h.run();
    h.get_by_label_contains("sdc.toml is read-only");
    // The Preset combo beside it writes sdc.toml too: off for the same reason.
    let preset = h
        .query_all_by_role(Role::ComboBox)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Preset (this project): "))
        })
        .expect("the Settings Preset combo");
    assert!(preset.accesskit_node().is_disabled());
    preset.hover();
    h.run();
    h.get_by_label_contains("sdc.toml is read-only");
    assert_eq!(
        std::fs::read_to_string(d.path().join("textures/sdc.toml")).unwrap(),
        "this is = [not toml"
    );
}

#[test]
fn the_mode_row_shows_in_the_popped_out_window() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.settings_tab_popped = true;
    let mut h = harness(m, 1400.0);
    open_from_header(&mut h);
    assert_eq!(windows(&h), 1, "Settings is in a viewport of its own");
    h.get_by_label(&mode_name("cs"));
    h.get_by_label(PROJECT_HEADING);
}

/// The plan's 1280 px capture of the Settings tab, as a test: the Mode row is there and its pieces do not overlap.
#[test]
fn the_mode_row_fits_at_1280() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness_sized(opened(d.path()), egui::vec2(1280.0, 800.0));
    open_from_header(&mut h);
    let heading = h.get_by_label(PROJECT_HEADING).rect();
    let label = h.get_by_label("Mode").rect();
    let combo = h.get_by_label(&mode_name("cs")).rect();
    let note = h.get_by_label("Wins over a preset's own Target.").rect();
    let all = [heading, label, combo, note];
    for (i, a) in all.iter().enumerate() {
        assert!(a.right() <= 1280.5, "{a:?} runs off the window");
        for b in &all[i + 1..] {
            assert!(!a.shrink(0.5).intersects(b.shrink(0.5)), "{a:?} {b:?}");
        }
    }
}
