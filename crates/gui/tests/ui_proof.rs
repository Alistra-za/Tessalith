//! The UI driver's own tests, on tiny UIs. The journeys that drive the real app live in `ui_proof/<feature>.rs`.

mod app_harness;
#[path = "ui_drive/checks.rs"]
mod checks;
#[path = "ui_drive/fixtures.rs"]
mod fixtures;
#[path = "ui_drive/frame.rs"]
mod frame;
#[allow(dead_code)] // fields only the doc shots read
#[path = "doc_shots/manifest.rs"]
mod manifest;
#[path = "ui_drive/oracles.rs"]
mod oracles;
#[path = "ui_drive/state.rs"]
mod state;
#[path = "ui_drive/steps.rs"]
mod steps;
mod support;

use checks::{run_checks, Finding};
use eframe::egui::{self, vec2, Key, Modifiers};
use egui_kittest::Harness;
use frame::{snapshot, tree_text};
use sdc_gui::model::sets;
use steps::{find, parse_key, parse_script, ui_step, Step, Target};

/// A test that replays a step script on a fixture; see `run_journey`.
macro_rules! journey {
    ($name:ident, $fixture:literal, [$($step:literal),* $(,)?]) => {
        #[test]
        fn $name() {
            crate::run_journey($fixture, &[$($step),*]);
        }
    };
}

#[path = "ui_proof/sets_column.rs"]
mod sets_column;
#[path = "ui_proof/sets_column_pins.rs"]
mod sets_column_pins;

/// Replays `lines` on `fixture` without wgpu; panics "step N `<line>`: <error>" at the first failing step.
pub fn run_journey(fixture: &str, lines: &[&str]) {
    replay(fixture, lines);
}

/// Replays `lines` like `run_journey` and returns the harness (and its temp dir) for further checks. Keep the
/// `TempDir` alive as long as the harness: dropping it deletes the project folder the app reads.
pub fn replay(fixture: &str, lines: &[&str]) -> (tempfile::TempDir, app_harness::H) {
    replay_with(fixture, |_| {}, lines)
}

/// `replay`, with `before` run once the fixture has opened and before the first line.
pub fn replay_with(
    fixture: &str,
    before: impl FnOnce(&mut app_harness::H),
    lines: &[&str],
) -> (tempfile::TempDir, app_harness::H) {
    replay_mid(fixture, before, lines, |_| {}, &[])
}

/// `replay_with`, then `mid` between `first` and `rest` (a change the first lines' clicks must not see).
pub fn replay_mid(
    fixture: &str,
    before: impl FnOnce(&mut app_harness::H),
    first: &[&str],
    mid: impl FnOnce(&mut app_harness::H),
    rest: &[&str],
) -> (tempfile::TempDir, app_harness::H) {
    app_data();
    let d = tempfile::tempdir().unwrap();
    let (mut h, run) = fixtures::start(fixture, d.path(), false).unwrap();
    before(&mut h);
    play(&mut h, &run, first, 0);
    mid(&mut h);
    play(&mut h, &run, rest, first.len());
    (d, h)
}

/// Runs `lines`, numbering steps from `done + 1` (the lines already played).
fn play(h: &mut app_harness::H, run: &fixtures::Run, lines: &[&str], done: usize) {
    let script = parse_script(&lines.join("\n")).unwrap();
    for (i, (line, step)) in script.iter().enumerate() {
        let n = done + i + 1;
        match fixtures::app_step(h, run, step) {
            Err(e) => panic!("step {n} `{line}`: {e}"),
            // A journey's check must decide: a note is a check that proved nothing.
            Ok(Some(note)) if matches!(step, Step::Check { .. }) => {
                panic!("step {n} `{line}`: check did not decide: {note}")
            }
            Ok(_) => {}
        }
    }
}

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
            for i in 0..500 {
                ui.label(format!("row {i}"));
            }
        });
    });
    let text = tree_text(&snapshot(&h));
    assert_eq!(
        text.lines().filter(|l| l.contains("\"row ")).count(),
        400,
        "{text}"
    );
    assert!(text.lines().any(|l| l.trim() == "… 100 more"), "{text}");
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
    let asked = run.picker.asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0], "folder: cancel");
    assert_eq!(h.state().model.root, root, "the fake answered Cancel");
}

#[test]
fn the_state_names_selection_dirt_and_outputs() {
    app_data();
    let d = tempfile::tempdir().unwrap();
    let (mut h, run) = fixtures::start("demo", d.path(), false).unwrap();
    // an unknown check names the known ones
    let unknown = parse_script(r#"{"op":"check","name":"nope"}"#).unwrap();
    let e = fixtures::app_step(&mut h, &run, &unknown[0].1).unwrap_err();
    assert!(e.contains("sets_pills_match_their_counts"), "{e}");
    let key = sets::views(&h.state().model)
        .iter()
        .find(|v| v.id == "architecture/stone/stonewall")
        .unwrap()
        .key
        .clone();
    h.state_mut().model.select_set(&key);
    app_harness::settle(&mut h, 2);
    let m = &h.state().model;
    let s = state::state_json(m);
    let at = |p: &str| s.pointer(p).unwrap_or_else(|| panic!("no {p} in {s}"));
    assert_eq!(at("/selection"), "set:architecture/stone/stonewall");
    assert!(at("/dirty").is_boolean());
    assert_eq!(at("/review_open"), false);
    let out = at("/items/architecture~1stone~1stonewall.png/output");
    assert!(out.as_str().unwrap().ends_with("stonewall.dds"), "{out}");
    assert!(at("/sets/architecture~1stone~1cobble/slots")
        .as_object()
        .unwrap()
        .values()
        .any(|v| v.as_str().unwrap().starts_with("own:")));
    assert_eq!(at("/counts/sets"), sets::views(m).len());
}

#[test]
fn expect_reads_equals_contains_and_absent() {
    use serde_json::json;
    use state::eval_expect as ev;
    let s = json!({"a": [1, 2], "s": "hello", "o": {"k": 1}});
    assert!(ev(&s, "/a", None, Some(&json!(2)), false).is_ok());
    let e = ev(&s, "/a", None, Some(&json!(3)), false).unwrap_err();
    assert!(e.contains("found [1,2]"), "{e}");
    assert!(ev(&s, "/s", None, Some(&json!("ell")), false).is_ok());
    assert!(ev(&s, "/o", None, Some(&json!("k")), false).is_ok());
    assert!(ev(&s, "/nope", None, None, true).is_ok());
    assert!(ev(&s, "/a", None, None, true).is_err());
    assert!(ev(&s, "/s", Some(&json!("hello")), None, false).is_ok());
    let e = ev(&s, "/s", Some(&json!("hello")), None, true).unwrap_err();
    assert!(e.contains("exactly one"), "{e}");
}

#[test]
fn filter_matches_names_wrong_rows_and_flags_hide_all_and_no_change() {
    use oracles::filter_matches as fm;
    let all: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
    let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(fm(&v(&["a"]), &all, |n| n == "a"), Ok(None));
    let e = fm(&v(&["a", "b"]), &all, |n| n == "a").unwrap_err();
    assert!(e.contains('b') && !e.contains("\"a\""), "{e}");
    assert_eq!(
        fm(&v(&[]), &all, |_| false),
        Ok(Some("hides everything".into()))
    );
    assert_eq!(fm(&all, &all, |_| true), Ok(Some("changes nothing".into())));
}

#[test]
fn one_set_open_decides_on_the_selection() {
    let run = |m: &sdc_gui::model::AppModel| {
        let (_, f) = oracles::ORACLES
            .iter()
            .find(|(n, _)| *n == "one_set_open")
            .expect("registered");
        f(m)
    };
    let (_d, mut h) = replay("demo", &[]);
    let e = run(&h.state().model).unwrap_err();
    assert!(e.contains("not a set"), "{e}");
    let key = sets::views(&h.state().model)[0].key.clone();
    h.state_mut().model.select_set(&key);
    assert_eq!(run(&h.state().model), Ok(None));
    h.state_mut().model.selection = sdc_gui::model::Selection::Item("x.png".into());
    assert!(run(&h.state().model).unwrap_err().contains("not a set"));
}

journey!(
    clicking_a_set_card_opens_the_set,
    "demo",
    [
        r#"{"op":"click","at":{"label":"architecture/stone/stonewall"}}"#,
        r#"{"op":"expect","fact":"/selection","equals":"set:architecture/stone/stonewall"}"#
    ]
);

journey!(
    the_not_ready_pill_shows_what_it_counts,
    "demo",
    [
        r#"{"op":"click","at":{"starts":"Not ready","role":"Button"}}"#,
        r#"{"op":"check","name":"sets_pills_match_their_counts"}"#,
        r#"{"op":"expect","fact":"/sets_filter/attention","equals":true}"#
    ]
);

#[test]
#[should_panic(expected = "expect /review_open")]
fn a_false_expect_fails_the_journey() {
    run_journey(
        "demo",
        &[r#"{"op":"expect","fact":"/review_open","equals":true}"#],
    );
}

#[test]
fn an_unknown_fixture_lists_the_known_ones() {
    let d = tempfile::tempdir().unwrap();
    let e = fixtures::start("nope", d.path(), false).err().unwrap();
    assert!(e.contains("nope") && e.contains("demo"), "{e}");
}

// ---- mechanical frame checks (checks.rs) ----

type Shared = std::rc::Rc<std::cell::Cell<egui::Rect>>;

/// Findings of every check on one frame of a tiny UI in a `w`x`h` window.
fn found(w: f32, h: f32, ui: impl FnMut(&mut egui::Ui) + 'static) -> Vec<Finding> {
    let mut h = Harness::builder().with_size(vec2(w, h)).build_ui(ui);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    run_checks(&snapshot(&h))
}

fn of<'a>(f: &'a [Finding], check: &str) -> Vec<&'a Finding> {
    f.iter().filter(|x| x.check == check).collect()
}

fn rect4(r: egui::Rect) -> [f32; 4] {
    [r.min.x, r.min.y, r.width(), r.height()]
}

/// A 32x32 clickable tile named `name`, with `paint` drawing its text.
fn tile(
    ui: &mut egui::Ui,
    name: &str,
    paint: impl FnOnce(&egui::Painter, egui::Rect),
) -> egui::Rect {
    let (r, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    paint(ui.painter(), r);
    r
}

fn centred(letter: &'static str) -> impl FnOnce(&egui::Painter, egui::Rect) {
    move |p, r| {
        p.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            letter,
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
    }
}

#[test]
fn names_flags_an_unnamed_button() {
    let empty = Shared::new(std::cell::Cell::new(egui::Rect::NOTHING));
    let e2 = empty.clone();
    let f = found(400.0, 200.0, move |ui| {
        egui::Panel::left("side").resizable(true).show(ui, |ui| {
            ui.label("side");
        });
        ui.horizontal(|ui| {
            e2.set(ui.add(egui::Button::new("")).rect);
            let _ = ui.button("Save");
        });
    });
    let n = of(&f, "names");
    assert_eq!(
        n.len(),
        1,
        "{:?}",
        n.iter().map(|x| &x.detail).collect::<Vec<_>>()
    );
    assert_eq!(n[0].subject.rect, rect4(empty.get()));
    assert!(n[0].rule.starts_with("egui-ui-rules 8.2"));
}

#[test]
fn clipped_text_flags_a_cut_label_but_not_a_truncated_one() {
    let f = found(400.0, 200.0, |ui| {
        ui.scope(|ui| {
            ui.set_clip_rect(egui::Rect::from_min_size(
                egui::pos2(10.0, 10.0),
                vec2(40.0, 40.0),
            ));
            ui.put(
                egui::Rect::from_min_size(egui::pos2(10.0, 10.0), vec2(200.0, 20.0)),
                egui::Label::new("cut label sticking out"),
            );
        });
        ui.allocate_ui(vec2(60.0, 20.0), |ui| {
            ui.add(egui::Label::new("truncated label that is long").truncate());
        });
        ui.scope(|ui| {
            ui.set_clip_rect(egui::Rect::from_min_size(
                egui::pos2(10.0, 100.0),
                vec2(300.0, 6.0),
            ));
            ui.put(
                egui::Rect::from_min_size(egui::pos2(10.0, 100.0), vec2(200.0, 20.0)),
                egui::Label::new("cut at the bottom"),
            );
        });
    });
    let c = of(&f, "clipped_text");
    assert_eq!(
        c.len(),
        1,
        "{:?}",
        c.iter().map(|x| &x.subject.label).collect::<Vec<_>>()
    );
    assert_eq!(c[0].subject.label, "cut label sticking out");
    assert!(c[0].detail.contains("hover"), "{}", c[0].detail);
    let s = of(&f, "summary");
    assert!(
        s.len() == 1 && s[0].detail.contains("clipped_text"),
        "the bottom cut is counted in the summary: {:?}",
        s.iter().map(|x| &x.detail).collect::<Vec<_>>()
    );
}

#[test]
fn off_window_flags_a_button_cut_by_the_edge() {
    let f = found(300.0, 200.0, |ui| {
        let at = |x| egui::Rect::from_min_size(egui::pos2(x, 20.0), vec2(60.0, 24.0));
        ui.put(at(280.0), egui::Button::new("Far"));
        ui.put(at(10.0), egui::Button::new("Near"));
    });
    let o = of(&f, "off_window");
    assert_eq!(
        o.len(),
        1,
        "{:?}",
        o.iter().map(|x| &x.subject.label).collect::<Vec<_>>()
    );
    assert_eq!(o[0].subject.label, "Far");
}

/// Note HP: the dock's badge hit rects sit on a chip's corner by design; anything else that overlaps still counts.
#[test]
fn overlap_spares_a_badge_centred_on_a_chip_corner() {
    let hit = |ui: &mut egui::Ui, at: egui::Rect, name: &str| {
        let r = ui.interact(at, ui.id().with(name), egui::Sense::click());
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    };
    let chip = egui::Rect::from_min_size(egui::pos2(100.0, 60.0), vec2(32.0, 32.0));
    let overlaps = |badge: egui::Rect| {
        let f = found(300.0, 200.0, move |ui| {
            hit(ui, chip, "chip");
            hit(ui, badge, "badge");
        });
        of(&f, "overlap").len()
    };
    // 18x20, centre 1 px left of the top-right corner (a clamped badge)
    let corner = egui::Rect::from_center_size(chip.right_top() - vec2(1.0, 0.0), vec2(18.0, 20.0));
    assert_eq!(overlaps(corner), 0, "a badge on the corner");
    // 20x20 centred on the top edge's midpoint is a real overlap
    let mid = egui::Rect::from_center_size(chip.center_top(), vec2(20.0, 20.0));
    assert_eq!(overlaps(mid), 1, "a badge on the edge's middle");
    // two buttons sharing an edge
    let side = egui::Rect::from_min_size(chip.right_top(), vec2(32.0, 32.0));
    assert_eq!(overlaps(side), 0, "touching edges");
}

#[test]
fn overlap_flags_two_buttons_on_top_of_each_other() {
    let f = found(300.0, 200.0, |ui| {
        let at = |x, y| egui::Rect::from_min_size(egui::pos2(x, y), vec2(50.0, 30.0));
        ui.put(at(10.0, 10.0), egui::Button::new("A"));
        ui.put(at(40.0, 20.0), egui::Button::new("B"));
        ui.put(at(10.0, 100.0), egui::Button::new("C"));
        ui.put(at(70.0, 100.0), egui::Button::new("D"));
    });
    let o = of(&f, "overlap");
    assert_eq!(
        o.len(),
        1,
        "{:?}",
        o.iter().map(|x| &x.detail).collect::<Vec<_>>()
    );
    let pair = [&o[0].subject.label, &o[0].other.as_ref().unwrap().label];
    assert!(
        pair.contains(&&"A".to_string()) && pair.contains(&&"B".to_string()),
        "{pair:?}"
    );
}

#[test]
fn alignment_flags_a_tile_whose_letter_moved_and_a_tile_with_none() {
    let f = found(400.0, 200.0, |ui| {
        ui.horizontal(|ui| {
            tile(ui, "tile 1", centred("A"));
            tile(ui, "tile 2", centred("B"));
            tile(ui, "tile 3", |p, r| {
                p.text(
                    r.left_bottom(),
                    egui::Align2::LEFT_BOTTOM,
                    "C",
                    egui::FontId::proportional(14.0),
                    egui::Color32::WHITE,
                );
            });
            tile(ui, "tile 4", |_, _| {});
            tile(ui, "tile 5", centred("E"));
        });
    });
    let a = of(&f, "alignment");
    let who: Vec<_> = a
        .iter()
        .map(|x| (x.subject.label.as_str(), x.detail.as_str()))
        .collect();
    assert_eq!(a.len(), 2, "{who:?}");
    let moved = a
        .iter()
        .find(|x| x.subject.label == "tile 3")
        .expect("moved tile");
    assert!(moved.detail.contains("offset"), "{}", moved.detail);
    let none = a
        .iter()
        .find(|x| x.subject.label == "tile 4")
        .expect("empty tile");
    assert!(none.detail.contains("no painted text"), "{}", none.detail);

    // left-aligned names of different widths are one placement
    let f = found(400.0, 200.0, |ui| {
        ui.horizontal(|ui| {
            for name in ["a", "abc", "abcdef"] {
                tile(ui, name, move |p, r| {
                    p.text(
                        r.left_center() + vec2(4.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        name,
                        egui::FontId::proportional(14.0),
                        egui::Color32::WHITE,
                    );
                });
            }
        });
    });
    assert!(of(&f, "alignment").is_empty(), "{:?}", of(&f, "alignment"));
}

#[test]
fn lonely_wrap_flags_one_tile_alone_on_its_row() {
    fn wrapped(n: usize) -> Vec<Finding> {
        found(500.0, 300.0, move |ui| {
            let w = 4.0 * 32.0 + 3.0 * ui.spacing().item_spacing.x + 1.0;
            ui.allocate_ui(vec2(w, 200.0), |ui| {
                ui.horizontal_wrapped(|ui| {
                    for i in 0..n {
                        tile(ui, &format!("tile {i}"), centred("T"));
                    }
                });
            });
        })
    }
    let l = wrapped(5);
    assert_eq!(
        of(&l, "lonely_wrap").len(),
        1,
        "{:?}",
        of(&l, "lonely_wrap")
    );
    assert_eq!(of(&l, "lonely_wrap")[0].subject.label, "tile 4");
    assert!(of(&wrapped(4), "lonely_wrap").is_empty());
    assert!(of(&wrapped(8), "lonely_wrap").is_empty());
}

#[test]
fn painted_only_matches_whole_words_and_skips_glyphs() {
    let f = found(400.0, 200.0, |ui| {
        ui.horizontal(|ui| {
            for (name, painted) in [
                ("Normal thumbnail", "N"),
                ("Textures view", "Textures"),
                ("Smaller tiles", "\u{2212}"),
            ] {
                let (r, resp) = ui.allocate_exact_size(vec2(90.0, 30.0), egui::Sense::click());
                resp.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name)
                });
                ui.painter().text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    painted,
                    egui::FontId::proportional(14.0),
                    egui::Color32::WHITE,
                );
            }
        });
    });
    let p = of(&f, "painted_only");
    assert_eq!(
        p.len(),
        1,
        "{:?}",
        p.iter().map(|x| &x.subject.label).collect::<Vec<_>>()
    );
    assert_eq!(p[0].subject.label, "N");
}

#[test]
fn tree_text_prints_sixty_siblings_without_a_more_line() {
    let h = tiny(|ui, _| {
        for i in 0..60 {
            ui.label(format!("row{i:02}"));
        }
    });
    let t = tree_text(&snapshot(&h));
    for i in 0..60 {
        assert!(t.contains(&format!("row{i:02}")), "row{i:02} missing");
    }
    assert!(!t.contains(" more"), "{t}");
}

/// run.json `size` is where the run started and `final_size` where it ended. Needs the ui_drive exe (built by
/// `cargo test --test ui_drive --no-run`) and a GPU, so it is opt-in.
#[test]
#[ignore = "needs the built ui_drive exe and a GPU"]
fn run_json_records_the_start_size_and_the_final_size() {
    let deps = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let exe = std::fs::read_dir(&deps)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap().to_string_lossy().into_owned();
            n.starts_with("ui_drive-") && n.ends_with(".exe")
        })
        .max_by_key(|p| p.metadata().unwrap().modified().unwrap())
        .expect("no ui_drive exe; run `cargo test --test ui_drive --no-run`");
    let d = tempfile::tempdir().unwrap();
    let script = d.path().join("steps.jsonl");
    std::fs::write(&script, "{\"op\":\"resize\",\"w\":1100,\"h\":700}\n").unwrap();
    let out = d.path().join("out");
    let status = std::process::Command::new(exe)
        .args(["--fixture", "demo", "--size", "1300x800", "--script"])
        .arg(&script)
        .arg("--out")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success(), "ui_drive exited {status}");
    let run: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("run.json")).unwrap()).unwrap();
    assert_eq!(run["size"], serde_json::json!([1300.0, 800.0]));
    assert_eq!(run["final_size"], serde_json::json!([1100.0, 700.0]));
}
