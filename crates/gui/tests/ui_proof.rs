//! The UI driver's own tests, on tiny UIs. The journeys that drive the real app come later.
#![allow(dead_code)] // the shared manifest has fields only the doc shots read

mod app_harness;
#[path = "ui_drive/fixtures.rs"]
mod fixtures;
#[path = "ui_drive/frame.rs"]
mod frame;
#[path = "doc_shots/manifest.rs"]
mod manifest;
#[path = "ui_drive/steps.rs"]
mod steps;
mod support;

use eframe::egui::{self, vec2, Key, Modifiers};
use egui_kittest::Harness;
use frame::{snapshot, tree_text};
use steps::{find, parse_key, parse_script, ui_step, Step, Target};

/// `%APPDATA%` and `%LOCALAPPDATA%` in a temp dir for the whole process; every test that starts the app calls this.
fn app_data() {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        app_harness::redirect_app_data(d.path());
        d
    });
}

#[derive(Default)]
struct Tiny {
    clicks: Vec<u32>,
    text: String,
    dropped: bool,
    init: bool,
}

type T = Harness<'static, Tiny>;

fn tiny(ui: impl FnMut(&mut egui::Ui, &mut Tiny) + 'static) -> T {
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 300.0))
        .build_ui_state(
            ui,
            Tiny {
                clicks: vec![0; 64],
                ..Tiny::default()
            },
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run_steps(3);
    h
}

fn step(h: &mut T, line: &str) -> Result<(), String> {
    let s = parse_script(line)?;
    ui_step(h, &s[0].1)
}

fn label(l: &str) -> Target {
    Target {
        label: Some(l.into()),
        ..Target::default()
    }
}

fn two_d_buttons(ui: &mut egui::Ui, s: &mut Tiny) {
    ui.horizontal(|ui| {
        for i in 0..2 {
            if ui.button("D").clicked() {
                s.clicks[i] += 1;
            }
        }
    });
}

#[test]
fn every_op_parses() {
    let script = [
        r#"{"op":"click","at":{"label":"a"},"mods":"Ctrl"}"#,
        r#"{"op":"double_click","at":{"label":"a"}}"#,
        r#"{"op":"right_click","at":{"starts":"a"}}"#,
        r#"{"op":"hover","at":{"placeholder":"a"}}"#,
        r#"{"op":"type","text":"x"}"#,
        r#"{"op":"key","key":"Ctrl+K"}"#,
        r#"{"op":"drag","from":{"label":"a"},"to":{"label":"b"}}"#,
        r#"{"op":"scroll","at":{"label":"a"},"dir":"into_view"}"#,
        r#"{"op":"resize","w":800,"h":600}"#,
        r#"{"op":"settle"}"#,
        r#"{"op":"pick","path":"x"}"#,
        r#"{"op":"expect","fact":"f","equals":1}"#,
        r#"{"op":"check","name":"c"}"#,
    ];
    let steps = parse_script(&format!("{}\n\n", script.join("\n\n"))).unwrap();
    assert_eq!(steps.len(), 13);
    assert_eq!(steps[5].0, script[5]);
    assert_eq!(
        steps[5].1,
        Step::Key {
            key: "Ctrl+K".into()
        }
    );
}

#[test]
fn an_unknown_op_or_field_names_its_line() {
    let e = parse_script("\n{\"op\":\"clik\"}").unwrap_err();
    assert!(e.starts_with("line 2:"), "{e}");
    assert!(parse_script(r#"{"op":"click","at":{"label":"a"},"bogus":1}"#).is_err());
    let e = parse_script(r#"{"op":"click","at":{"label":"a","placeholder":"b"}}"#).unwrap_err();
    assert!(
        e.contains("exactly one of label, starts, placeholder"),
        "{e}"
    );
}

#[test]
fn key_chords_parse() {
    assert_eq!(
        parse_key("Ctrl+Shift+Z").unwrap(),
        (Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
    );
    assert_eq!(parse_key("Enter").unwrap(), (Modifiers::NONE, Key::Enter));
    assert!(parse_key("Hyper+Q").is_err());
}

#[test]
fn two_matches_without_index_list_both() {
    let mut h = tiny(|ui, _| {
        let _ = ui.button("D");
        let _ = ui.button("D");
    });
    let s = snapshot(&h);
    let e = find(&s, &label("D")).unwrap_err();
    assert!(e.contains("2 nodes labelled \"D\""), "{e}");
    assert_eq!(e.matches("Button@(").count(), 2, "{e}");
    assert!(
        e.contains("add \"index\" (0 = first in reading order) or \"role\""),
        "{e}"
    );
    h.run_steps(1);
}

#[test]
fn index_picks_in_reading_order() {
    let mut h = tiny(two_d_buttons);
    step(&mut h, r#"{"op":"click","at":{"label":"D","index":1}}"#).unwrap();
    assert_eq!(&h.state().clicks[..2], &[0, 1]);
}

#[test]
fn a_missing_label_lists_labels_that_share_a_word() {
    let mut h = tiny(|ui, _| {
        let _ = ui.button("Open folder…");
        let _ = ui.button("Save");
    });
    let e = step(&mut h, r#"{"op":"click","at":{"label":"Open"}}"#).unwrap_err();
    assert!(e.contains("Open folder…") && !e.contains("Save"), "{e}");
    let e = step(&mut h, r#"{"op":"click","at":{"label":"Zebra"}}"#).unwrap_err();
    assert!(e.contains("Open folder…") && e.contains("Save"), "{e}");
}

#[test]
fn an_off_screen_target_is_scrolled_into_view_first() {
    let mut h = tiny(|ui, s| {
        egui::ScrollArea::vertical()
            .max_height(100.0)
            .show(ui, |ui| {
                for i in 0..50 {
                    if ui.button(format!("Item {i}")).clicked() {
                        s.clicks[i] += 1;
                    }
                }
            });
    });
    step(&mut h, r#"{"op":"click","at":{"label":"Item 45"}}"#).unwrap();
    assert_eq!(h.state().clicks[45], 1);
    assert_eq!(h.state().clicks.iter().sum::<u32>(), 1);
}

#[test]
fn typing_without_a_focused_field_is_refused() {
    let mut h = tiny(|ui, s| {
        ui.add(egui::TextEdit::singleline(&mut s.text).hint_text("name"));
    });
    let e = step(&mut h, r#"{"op":"type","text":"x"}"#).unwrap_err();
    assert!(e.contains("no text field has focus"), "{e}");
    step(
        &mut h,
        r#"{"op":"type","text":"x","at":{"placeholder":"name"}}"#,
    )
    .unwrap();
    assert_eq!(h.state().text, "x");
}

#[test]
fn drag_carries_a_payload_between_widgets() {
    let mut h = tiny(|ui, s| {
        ui.dnd_drag_source(egui::Id::new("src"), 7u32, |ui| {
            ui.label("a");
        });
        ui.add_space(40.0);
        let (_, dropped) = ui.dnd_drop_zone::<u32, _>(egui::Frame::new().inner_margin(8.0), |ui| {
            let _ = ui.button("zone");
        });
        if dropped.is_some() {
            s.dropped = true;
        }
    });
    step(
        &mut h,
        r#"{"op":"drag","from":{"label":"a"},"to":{"label":"zone"}}"#,
    )
    .unwrap();
    assert!(h.state().dropped);
}

#[test]
fn the_tree_marks_focus_disabled_and_hover() {
    let mut h = tiny(|ui, s| {
        let r = ui.text_edit_singleline(&mut s.text);
        if !s.init {
            r.request_focus();
            s.init = true;
        }
        ui.add_enabled(false, egui::Button::new("Off"));
        let _ = ui.button("On");
    });
    step(&mut h, r#"{"op":"hover","at":{"label":"On"}}"#).unwrap();
    let text = tree_text(&snapshot(&h));
    let line = |needle: &str, flag: &str| {
        assert!(
            text.lines().any(|l| l.contains(needle) && l.contains(flag)),
            "no `{needle}` line with `{flag}` in:\n{text}"
        );
    };
    line("TextInput", "focused");
    line("Button \"Off\"", "disabled");
    line("Button \"On\"", "hovered");
}

#[test]
fn the_tree_folds_unnamed_containers_and_long_lists() {
    let h = tiny(|ui, _| {
        ui.vertical(|ui| {
            for i in 0..80 {
                ui.label(format!("row {i}"));
            }
        });
    });
    let text = tree_text(&snapshot(&h));
    assert_eq!(
        text.lines().filter(|l| l.contains("\"row ")).count(),
        50,
        "{text}"
    );
    assert!(text.lines().any(|l| l.trim() == "… 30 more"), "{text}");
    assert!(
        !text
            .lines()
            .any(|l| l.trim_start().starts_with("GenericContainer \"\"")),
        "{text}"
    );
}

#[test]
fn a_label_is_scrolled_into_view() {
    let mut h = tiny(|ui, _| {
        egui::ScrollArea::vertical()
            .max_height(100.0)
            .show(ui, |ui| {
                for i in 0..50 {
                    ui.label(format!("Row {i}"));
                }
            });
    });
    step(
        &mut h,
        r#"{"op":"scroll","at":{"label":"Row 45"},"dir":"into_view"}"#,
    )
    .unwrap();
    let s = snapshot(&h);
    let n = find(&s, &label("Row 45")).unwrap();
    // inside the 100 px scroll area (8..108) and, when the join is trusted, inside its clip
    assert!(n.rect.min.y >= 8.0 && n.rect.max.y <= 108.5, "{n:?}");
    assert!(
        n.clip.is_none_or(|c| c.contains(frame::visible_centre(n))),
        "{n:?}"
    );
}

#[test]
fn a_click_carries_its_modifiers() {
    let mut h = tiny(|ui, s| {
        if ui.button("M").clicked() {
            s.dropped = ui.input(|i| i.modifiers.command);
        }
    });
    step(&mut h, r#"{"op":"click","at":{"label":"M"},"mods":"Ctrl"}"#).unwrap();
    assert!(h.state().dropped);
}

#[test]
fn an_index_with_no_match_still_lists_labels() {
    let h = tiny(|ui, _| {
        let _ = ui.button("Open folder…");
    });
    let t = Target {
        index: Some(0),
        ..label("Open")
    };
    let e = find(&snapshot(&h), &t).unwrap_err();
    assert!(
        e.contains("no node labelled \"Open\"") && e.contains("Open folder…"),
        "{e}"
    );
}

#[test]
fn the_driver_picker_answers_in_order_then_cancels() {
    use sdc_gui::model::picker::{PickerBackend, PickerSpec};
    let p = fixtures::DriverPicker::default();
    p.queue("a".into());
    p.queue("b".into());
    let spec = PickerSpec::folder();
    assert_eq!(p.pick(&spec), Some("a".into()));
    assert_eq!(p.pick(&spec), Some("b".into()));
    assert_eq!(p.pick(&spec), None);
    let asked = p.asked.lock().unwrap();
    assert_eq!(asked.len(), 3);
    assert!(asked[2].ends_with("cancel"), "{asked:?}");
}

#[test]
fn pick_refuses_paths_outside_the_fixture() {
    app_data();
    let d = tempfile::tempdir().unwrap();
    let (mut h, run) = fixtures::start("demo", d.path(), false).unwrap();
    for bad in [r"..\x", r"C:\x", r"\x", r"a\..\..\x"] {
        let step = Step::Pick { path: bad.into() };
        let e = fixtures::app_step(&mut h, &run, &step).unwrap_err();
        assert!(e.contains("inside the fixture folder"), "{bad}: {e}");
    }
    let step = Step::Pick {
        path: "export2".into(),
    };
    fixtures::app_step(&mut h, &run, &step).unwrap();
}

#[test]
fn explorer_requests_are_recorded_not_spawned() {
    app_data();
    let d = tempfile::tempdir().unwrap();
    let (mut h, _run) = fixtures::start("demo", d.path(), false).unwrap();
    let file = d.path().join("textures/armor/steel.png");
    h.state_mut().model.reveal(&file).unwrap();
    app_harness::settle(&mut h, 2);
    let log = h.state().shell_log.clone().unwrap();
    assert_eq!(log, vec![sdc_gui::model::shell::ShellRequest::Reveal(file)]);
}

#[test]
fn the_open_folder_entry_asks_the_fake_dialog() {
    app_data();
    let d = tempfile::tempdir().unwrap();
    let (mut h, run) = fixtures::start("demo", d.path(), false).unwrap();
    let root = h.state().model.root.clone();
    assert!(root.is_some());
    for line in [
        r#"{"op":"click","at":{"label":"Open"}}"#,
        r#"{"op":"click","at":{"label":"Open folder…"}}"#,
    ] {
        let s = parse_script(line).unwrap();
        fixtures::app_step(&mut h, &run, &s[0].1).unwrap();
    }
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h.state().model.picker.busy() {
        assert!(std::time::Instant::now() < end, "the picker never finished");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    app_harness::settle(&mut h, 2);
    assert_eq!(run.picker.asked.lock().unwrap().len(), 1);
    assert_eq!(h.state().model.root, root, "the fake answered Cancel");
}

#[test]
fn an_unknown_fixture_lists_the_known_ones() {
    let d = tempfile::tempdir().unwrap();
    let e = fixtures::start("nope", d.path(), false).err().unwrap();
    assert!(e.contains("nope") && e.contains("demo"), "{e}");
}
