//! Task 14 (item 20, DECISIONS AG112-AG114): the toasts (fake clock), the Messages log and its action.
mod support;
use eframe::egui;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::Subtype;
use sdc_gui::model::messages::{
    EntryAction, Messages, Severity, FADE, FADE_WARN, LOG_CAP, MAX_SHOWN,
};
use sdc_gui::model::{Answer, AppModel};
use sdc_gui::ui::actions;
use std::collections::HashSet;
use std::time::{Duration, Instant};

fn none() -> HashSet<u64> {
    HashSet::new()
}

fn secs(t0: Instant, s: f32) -> Instant {
    t0 + Duration::from_secs_f32(s)
}

fn store() -> Messages {
    let mut m = Messages::default();
    m.clock = || "12:00:00".into();
    m
}

#[test]
fn a_toast_fades_after_four_seconds_and_a_warning_after_eight() {
    let (mut m, t0) = (store(), Instant::now());
    m.notify(Severity::Info, "Saved");
    m.notify(Severity::Warning, "careful");
    assert!(m.tick(t0, &none()));
    assert_eq!(m.toasts().len(), 2);
    assert_eq!(FADE, Duration::from_secs(4));
    assert_eq!(FADE_WARN, Duration::from_secs(8));
    m.tick(secs(t0, 3.9), &none());
    assert_eq!(m.toasts().len(), 2);
    m.tick(secs(t0, 4.1), &none());
    assert_eq!(m.toasts().len(), 1);
    assert_eq!(m.toasts()[0].text, "careful");
    m.tick(secs(t0, 7.9), &none());
    assert_eq!(m.toasts().len(), 1);
    assert!(!m.tick(secs(t0, 8.1), &none()), "nothing left");
    assert!(m.toasts().is_empty());
}

#[test]
fn a_toast_with_a_button_stays_long_enough_to_answer() {
    use sdc_gui::model::messages::ToastAction;
    let (mut m, t0) = (store(), Instant::now());
    m.notify_with(
        Severity::Info,
        "Restored",
        Some(ToastAction::UndoCrashRestore),
    );
    m.tick(t0, &none());
    m.tick(secs(t0, 4.1), &none());
    assert_eq!(m.toasts().len(), 1, "still shown after 4 s");
    m.tick(secs(t0, 19.0), &none());
    assert_eq!(m.toasts().len(), 1);
    m.tick(secs(t0, 20.1), &none());
    assert!(m.toasts().is_empty());
}

#[test]
fn a_toast_that_waited_for_the_first_tick_still_gets_its_full_time() {
    let (mut m, t0) = (store(), Instant::now());
    m.notify(Severity::Info, "a");
    // The first frame comes long after the notice; the time counts from it.
    m.tick(secs(t0, 100.0), &none());
    m.tick(secs(t0, 103.0), &none());
    assert_eq!(m.toasts().len(), 1);
    m.tick(secs(t0, 104.5), &none());
    assert!(m.toasts().is_empty());
}

#[test]
fn at_most_three_are_on_screen_and_the_rest_queue() {
    let (mut m, t0) = (store(), Instant::now());
    for i in 0..5 {
        m.notify(Severity::Info, format!("n{i}"));
    }
    m.tick(t0, &none());
    assert_eq!(MAX_SHOWN, 3);
    assert_eq!(m.toasts().len(), 3);
    assert_eq!(m.queued(), 2);
    // The first three expire; the queued two come up with their full time.
    m.tick(secs(t0, 4.1), &none());
    let texts: Vec<&str> = m.toasts().iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["n3", "n4"]);
    assert_eq!(m.queued(), 0);
    m.tick(secs(t0, 8.0), &none());
    assert_eq!(m.toasts().len(), 2, "3.9 s of 4 s used");
    m.tick(secs(t0, 8.2), &none());
    assert!(m.toasts().is_empty());
    assert_eq!(m.len(), 5, "the log kept all of them");
}

#[test]
fn hovering_a_toast_pauses_its_fade() {
    let (mut m, t0) = (store(), Instant::now());
    m.notify(Severity::Info, "one");
    m.notify(Severity::Info, "two");
    m.tick(t0, &none());
    let held: HashSet<u64> = [m.toasts()[0].id].into();
    m.tick(secs(t0, 3.0), &held);
    m.tick(secs(t0, 10.0), &held);
    let texts: Vec<&str> = m.toasts().iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["one"], "the other one went; the held one waits");
    // Leaving it (final review I2): the leave tick charges nothing, then the 4 s count on from there.
    m.tick(secs(t0, 13.0), &none());
    m.tick(secs(t0, 16.9), &none());
    assert_eq!(m.toasts().len(), 1);
    m.tick(secs(t0, 17.1), &none());
    assert!(m.toasts().is_empty());
}

#[test]
fn the_log_keeps_500_counts_unread_and_clears() {
    let mut m = store();
    for i in 0..LOG_CAP + 20 {
        m.log(Severity::Info, format!("m{i}"));
    }
    assert_eq!(m.len(), LOG_CAP);
    assert_eq!(m.entries().next().unwrap().text, "m20", "the oldest went");
    assert_eq!(m.entries().next().unwrap().time, "12:00:00");
    assert_eq!(m.unread(), LOG_CAP);
    m.set_open(true);
    assert_eq!(m.unread(), LOG_CAP, "opening alone shows no entries");
    m.mark_read();
    assert_eq!(m.unread(), 0);
    m.log(Severity::Warning, "while open");
    assert_eq!(m.unread(), 1, "the next frame has yet to draw this entry");
    m.mark_read();
    assert_eq!(m.unread(), 0);
    m.set_open(false);
    m.log(Severity::Error, "closed");
    assert_eq!(m.unread(), 1);
    m.clear();
    assert_eq!((m.len(), m.unread()), (0, 0));
}

#[test]
fn a_log_only_note_raises_no_toast() {
    let mut m = store();
    m.log(Severity::Info, "quiet");
    assert!(m.toasts().is_empty());
    assert_eq!(m.len(), 1);
}

#[test]
fn ctrl_m_is_an_action_in_global_and_toggles_the_panel() {
    let a = actions::action("messages").expect("listed");
    assert_eq!(actions::keys_text("messages"), "Ctrl+M");
    assert_eq!(a.area, actions::Area::Global);
    let dir = tempfile::tempdir().unwrap();
    let mut m = support::opened(dir.path());
    actions::run("messages", &mut m, &egui::Context::default());
    assert!(m.bottom.open && m.messages.is_open());
    actions::run("messages", &mut m, &egui::Context::default());
    assert!(!m.bottom.open && !m.messages.is_open());
}

#[test]
fn the_panel_lists_newest_first_and_clear_empties_it() {
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    m.messages.log(Severity::Info, "first note");
    m.messages.log(Severity::Warning, "second note");
    m.messages.set_open(true);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(700.0, 500.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::messages::tab(ui, m), m);
    h.run();
    let first = h.get_by_label("first note").rect().min.y;
    let second = h.get_by_label("second note").rect().min.y;
    assert!(second < first, "newest on top");
    h.get_by_label("2 messages");
    h.get_by_label("Clear").click();
    h.run();
    h.get_by_label("0 messages");
    assert!(h.query_by_label("first note").is_none());
    assert!(h.state().messages.is_empty());
}

/// Review C16 (Note FB4): one message is "1 message".
#[test]
fn the_panel_counts_one_message_in_the_singular() {
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    m.messages.log(Severity::Info, "only note");
    m.messages.set_open(true);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(700.0, 500.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::messages::tab(ui, m), m);
    h.run();
    h.get_by_label("1 message");
}

/// R14 (K9): the unread count is on the bottom panel's Messages tab (the header button is gone).
#[test]
fn the_messages_tab_shows_the_unread_count_and_opens_the_panel() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = support::opened(dir.path());
    m.messages.clear();
    m.messages.log(Severity::Info, "a");
    m.messages.log(Severity::Info, "b");
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1200.0, 700.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    h.get_by_label("Messages (2)").click();
    h.run();
    assert!(h.state().bottom.open && h.state().messages.is_open());
    assert_eq!(h.state().messages.unread(), 0);
    h.get_by_label("Messages");
}

#[test]
fn toasts_draw_at_the_bottom_right_and_are_gone_after_their_time() {
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    m.messages.notify(Severity::Info, "Saved");
    let t0 = Instant::now();
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(800.0, 600.0))
        .build_ui_state(
            move |ui, (m, at): &mut (AppModel, f32)| {
                sdc_gui::ui::messages::draw_at(ui.ctx(), m, t0 + Duration::from_secs_f32(*at))
            },
            (m, 0.0),
        );
    h.step();
    let r = h.get_by_label("Saved").rect();
    assert!(r.max.x > 600.0 && r.max.y > 450.0, "{r:?}");
    h.state_mut().1 = 4.2;
    h.step();
    h.step();
    assert!(h.query_by_label("Saved").is_none());
}

/// The Textures view: `ui::draw` only (the toasts are not drawn).
fn harness(model: AppModel) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), model)
}

/// Task 7: a log row can carry a Show button that selects its set; a set no longer listed disables it.
#[test]
fn a_show_button_selects_its_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    m.set_field(
        "landscape/rock01.png",
        "subtype",
        serde_json::json!("diffuse"),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    let rock = "landscape/rock01".to_owned();
    let target = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.id == rock)
        .map(sdc_gui::model::SetTarget::view)
        .expect("fixture has a rock01 card");
    let root = m.root.clone().unwrap();
    m.messages.log_with(
        Severity::Info,
        "x",
        EntryAction::ShowSet {
            root: root.clone(),
            id: rock.clone(),
        },
    );
    m.messages.log_with(
        Severity::Info,
        "y",
        EntryAction::ShowSet {
            root,
            id: "gone".into(),
        },
    );
    m.toggle_tab(sdc_gui::model::bottom::BottomTab::Messages);
    let mut h = harness(m);
    support::instant_tips(&h.ctx);
    h.run();
    h.run();
    assert!(h.query_by_label("x").is_some() && h.query_by_label("y").is_some());
    assert!(
        h.query_by_label(&format!("Show {rock}")).is_some(),
        "the live card is reachable from its log row"
    );
    let gone = h.get_by_label("Show gone");
    assert!(gone.accesskit_node().is_disabled());
    gone.hover();
    h.run();
    assert!(h.query_by_label("That set is no longer listed").is_some());
    h.get_by_label(&format!("Show {rock}")).click();
    h.run();
    h.run();
    assert_eq!(h.state().selection, target.clone().selection());
    h.get_by_label(&format!("Show {rock}")).click();
    h.run();
    assert_eq!(h.state().selection, target.selection());
    assert_eq!(h.state().messages.unread(), 0);
}

fn show_set_action(m: &AppModel, id: &str) -> EntryAction {
    EntryAction::ShowSet {
        root: m.root.clone().unwrap(),
        id: id.into(),
    }
}

/// A card made in the document alone has no key; once a file joins it the same Show finds it by its ID.
#[test]
fn a_show_button_follows_a_document_only_card_that_later_gets_a_key() {
    use sdc_gui::model::sets::{views, DropChoice, DropOutcome};
    use sdc_gui::model::{Selection, SetTarget};
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    let normal = "landscape/rock01_normal.png";
    m.edit_docs("t", None, true, |_, s| s.create_set("mine/empty"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let card = |m: &AppModel| {
        views(m)
            .iter()
            .find(|v| v.id == "mine/empty")
            .map(|v| (v.key.clone(), SetTarget::view(v)))
            .expect("the card is listed")
    };
    let (key, target) = card(&m);
    assert!(key.is_empty(), "no file yet: the card has no key");
    let action = show_set_action(&m, "mine/empty");
    m.messages.log_with(Severity::Info, "made", action.clone());
    assert_eq!(m.logged_set_target(&action), Some(target));
    m.remove_member("landscape/rock01", normal).unwrap();
    assert!(m.wait_idle(Duration::from_secs(60)));
    let out = m.drop_on_set(normal, "mine/empty");
    if matches!(out, DropOutcome::AskChoice { .. }) {
        m.drop_choice(normal, "mine/empty", DropChoice::Move);
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
    let (key, target) = card(&m);
    assert!(!key.is_empty(), "a file joined: the card has a key now");
    assert_eq!(m.logged_set_target(&action), Some(target));
    assert!(m.show_logged_set(&action));
    assert_eq!(m.selection, Selection::Set(key));
}

/// A Sets filter that hides the card is cleared by Show, so the card can be revealed; a filter that shows it stays.
#[test]
fn a_show_button_clears_a_sets_filter_that_hides_its_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    let action = show_set_action(&m, "landscape/rock01");
    m.sets_filter.search = "rock".into();
    assert!(m.show_logged_set(&action));
    assert_eq!(
        m.sets_filter.search, "rock",
        "the card passes: the filter stays"
    );
    m.sets_filter.search = "no such set".into();
    assert!(m.show_logged_set(&action));
    assert!(!m.sets_filter_active(), "the hiding filter is cleared");
    let key = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.id == "landscape/rock01")
        .map(|v| v.key.clone())
        .unwrap();
    assert_eq!(
        m.nav.reveal_set,
        Some(sdc_gui::model::SetTarget::Key(key.clone()))
    );
    assert_eq!(m.selection, sdc_gui::model::Selection::Set(key));
}

/// A set deleted after its row was logged: Show finds nothing and changes nothing.
#[test]
fn a_show_button_for_a_target_removed_since_logging_does_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    m.edit_docs("t", None, true, |_, s| s.create_set("mine/empty"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let action = show_set_action(&m, "mine/empty");
    assert!(m.logged_set_target(&action).is_some());
    assert!(m.delete_set("mine/empty"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let before = m.selection.clone();
    assert_eq!(m.logged_set_target(&action), None);
    assert!(!m.show_logged_set(&action));
    assert_eq!(m.selection, before);
    assert!(m.nav.reveal_set.is_none());
}

/// Note FC8 (supersedes CA10's anchor): while the bottom panel is open, the toasts sit above it, so they never cover
/// its buttons (Review's Export); with it collapsed they keep the window's bottom-right corner.
#[test]
fn toasts_sit_above_an_open_bottom_panel() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = support::opened(dir.path());
    m.messages.notify(Severity::Info, "Saved");
    m.toggle_tab(sdc_gui::model::bottom::BottomTab::Messages);
    assert!(m.bottom.open);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(1200.0, 800.0))
        .build_ui_state(
            |ui, m: &mut AppModel| {
                sdc_gui::ui::draw(ui, m);
                sdc_gui::ui::messages::draw(ui.ctx(), m);
            },
            m,
        );
    // `step`, not `run`: a toast keeps asking for frames while it shows.
    for _ in 0..6 {
        h.step();
    }
    let panel = support::panel(&h.ctx, "bottom");
    // The log row in the Messages tab says "Saved" too, at the left; the toast is the rightmost.
    let toast = h
        .query_all_by_label("Saved")
        .map(|n| n.rect())
        .max_by(|a, b| a.max.x.total_cmp(&b.max.x))
        .unwrap();
    assert!(
        toast.max.y <= panel.min.y,
        "toast {toast:?} over the panel {panel:?}"
    );
    assert!(toast.max.x > 1000.0, "still on the right: {toast:?}");
}

#[test]
fn saved_undo_and_export_results_raise_toasts() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    m.set_field(
        "landscape/rock01.png",
        "alpha.policy",
        serde_json::json!("remove"),
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    m.save().unwrap();
    let toasts = |m: &AppModel| -> Vec<String> {
        m.messages.toasts().iter().map(|t| t.text.clone()).collect()
    };
    assert!(
        toasts(&m).contains(&"Saved".to_string()),
        "{:?}",
        toasts(&m)
    );
    m.undo();
    assert!(
        toasts(&m).iter().any(|t| t.starts_with("Undid: ")),
        "{:?}",
        toasts(&m)
    );
    let s = sdc_gui::model::export::ExportResult {
        primary: sdc_gui::model::export::Outcomes {
            written: 7,
            skipped_sources: 2,
            failed: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        sdc_gui::model::export_finished_text(&s),
        "Export finished: 7 written, 0 up to date, 1 failed, 2 skipped sources"
    );
}

#[test]
fn twenty_quick_undos_leave_one_queued_undo_toast_and_the_queue_is_capped() {
    let mut m = store();
    for i in 0..20 {
        m.notify(Severity::Info, format!("Undid: step {i}."));
    }
    assert_eq!(m.toasts().len(), MAX_SHOWN);
    assert_eq!(m.queued(), 1, "the newest replaced the waiting one");
    assert_eq!(m.len(), 20, "the log keeps every one");
    let mut m = store();
    for i in 0..30 {
        m.notify(Severity::Info, format!("kind{i}: x"));
    }
    assert_eq!(m.queued(), sdc_gui::model::messages::MAX_QUEUED);
}

// ---- Note FM25: toasts appear whole, stay whole and go when their time is up

type ToastHarness = Harness<'static, (AppModel, f32)>;

/// A `draw_at` harness at 0.05 s steps; `at` is the model clock in seconds after `t0`.
fn toast_harness(first: &str) -> ToastHarness {
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    m.messages.notify(Severity::Info, first);
    let t0 = Instant::now();
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(800.0, 600.0))
        .with_step_dt(0.05)
        .build_ui_state(
            move |ui, (m, at): &mut (AppModel, f32)| {
                sdc_gui::ui::messages::draw_at(ui.ctx(), m, t0 + Duration::from_secs_f32(*at))
            },
            (m, 0.0),
        );
    h
}

fn steps(h: &mut ToastHarness, n: usize) {
    for _ in 0..n {
        h.step();
    }
}

/// What the last frame painted for the toast `text`.
#[derive(Debug, Clone, Copy)]
struct Paint {
    /// The popup frame's fill alpha (0-1).
    frame: f32,
    /// The text's alpha (0-1).
    text: f32,
    /// The text galley's top.
    y: f32,
    /// The text's painted rect.
    rect: egui::Rect,
}

fn paint(h: &ToastHarness, text: &str) -> Option<Paint> {
    fn walk<'a>(s: &'a egui::Shape, out: &mut Vec<&'a egui::Shape>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            s => out.push(s),
        }
    }
    let mut all = vec![];
    for c in &h.output().shapes {
        walk(&c.shape, &mut all);
    }
    let t = all.iter().find_map(|s| match s {
        egui::Shape::Text(t) if t.galley.text() == text => Some(t),
        _ => None,
    })?;
    let rect = t.galley.rect.translate(t.pos.to_vec2());
    // A label's glyphs carry PLACEHOLDER; the painted colour is the fallback.
    let v = t.galley.rows[0].row.visuals.mesh.vertices[0].color;
    let text_a = if v == egui::Color32::PLACEHOLDER {
        t.fallback_color
    } else {
        v
    }
    .a() as f32
        / 255.0;
    // The popup frame: the smallest unblurred rect around the text (the shadow is blurred).
    let frame = all
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Rect(r) if r.blur_width == 0.0 && r.rect.contains_rect(rect) => Some(r),
            _ => None,
        })
        .min_by(|a, b| a.rect.area().total_cmp(&b.rect.area()))?;
    Some(Paint {
        frame: frame.fill.a() as f32 / 255.0,
        text: text_a,
        y: t.pos.y,
        rect,
    })
}

/// "Saved"'s y below "Ready" (same Area). The anchored Area places itself from last frame's size, so on a new
/// toast's first frame the whole stack sits one toast low; measuring against the other toast removes that.
fn below_ready(h: &ToastHarness) -> f32 {
    paint(h, "Saved").expect("Saved drawn").y - paint(h, "Ready").expect("Ready drawn").y
}

/// K4: the first toast warms the Area, so the first frame of "Saved" is a visible one. A new toast is whole on its
/// first frame: no fade-in, no rise.
#[test]
fn a_new_toast_appears_whole_without_rising() {
    let mut h = toast_harness("Ready");
    h.step();
    h.state_mut().1 = 1.0;
    steps(&mut h, 4);
    let rest = paint(&h, "Ready").expect("drawn");
    h.state_mut().0.messages.notify(Severity::Info, "Saved");
    h.step();
    let first = paint(&h, "Saved").expect("drawn on its first frame");
    assert_eq!((first.frame, first.text), (rest.frame, rest.text));
    let first_dy = below_ready(&h);
    steps(&mut h, 14);
    let settled = below_ready(&h);
    assert!(
        (first_dy - settled).abs() <= 0.5,
        "no rise: {first_dy} then {settled}"
    );
    assert_eq!(paint(&h, "Saved").unwrap().frame, rest.frame);
}

/// A toast stays at full opacity to the end of its time, then is gone: no fade-out.
#[test]
fn a_toast_stays_whole_until_its_time_is_up() {
    let mut h = toast_harness("Saved");
    h.step();
    h.state_mut().1 = 1.0;
    steps(&mut h, 4);
    let rest = paint(&h, "Saved").expect("drawn");
    h.state_mut().1 = 3.95;
    steps(&mut h, 2);
    let late = paint(&h, "Saved").expect("still drawn at 3.95 s of 4");
    assert_eq!((late.frame, late.text), (rest.frame, rest.text));
    h.state_mut().1 = 4.2;
    steps(&mut h, 2);
    assert!(h.query_by_label("Saved").is_none());
}

#[test]
fn a_resting_toast_asks_for_no_frames_until_its_time_is_up() {
    let mut h = toast_harness("Saved");
    h.step();
    h.state_mut().1 = 0.25;
    steps(&mut h, 10);
    let d = h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    assert!(d != Duration::ZERO, "{d:?}");
    assert!(
        (Duration::from_secs_f32(3.6)..=Duration::from_secs_f32(3.8)).contains(&d),
        "{d:?}"
    );
}

/// Final review I2a: a pointer resting on a toast sends no frames. The frame it leaves on must not charge the rest
/// period, or the hover pause is lost.
#[test]
fn a_toast_rested_on_with_no_frames_keeps_its_time_after_the_pointer_leaves() {
    let mut h = toast_harness("Saved");
    h.step();
    h.state_mut().1 = 1.0;
    steps(&mut h, 4);
    let rest = paint(&h, "Saved").unwrap();
    h.hover_at(rest.rect.center());
    h.step();
    // 2.9 s later (most of the 3 s left), with no frame in between, the pointer leaves.
    h.state_mut().1 = 3.9;
    h.hover_at(egui::pos2(5.0, 5.0));
    h.step();
    steps(&mut h, 5);
    assert_eq!(paint(&h, "Saved").expect("still shown").frame, rest.frame);
}

/// Final review I2b: a hovered toast's clock is stopped, so it asks for no timer.
#[test]
fn a_hovered_toast_asks_for_no_short_timer() {
    let mut h = toast_harness("Saved");
    h.step();
    h.state_mut().1 = 1.0;
    steps(&mut h, 4);
    let rest = paint(&h, "Saved").unwrap();
    h.state_mut().1 = 3.95;
    h.step();
    h.hover_at(rest.rect.center());
    steps(&mut h, 10);
    assert_eq!(paint(&h, "Saved").expect("held").frame, rest.frame);
    let d = h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    assert!(d >= Duration::from_secs(1), "{d:?}");
}

// Task 8 (E8): a type choice that makes a set says so, once, after the final plan.

/// The `m.items` key whose relative path ends with `name`.
fn item_key_ending(m: &AppModel, name: &str) -> String {
    m.items
        .iter()
        .find(|i| i.key.ends_with(name))
        .unwrap_or_else(|| panic!("no item ending {name}"))
        .key
        .clone()
}

/// Writes each `textures/<name>` as a 64x64 gradient.
fn write_gradients(dir: &std::path::Path, names: &[&str]) {
    for n in names {
        let p = dir.join("textures").join(n);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        image::ImageBuffer::from_fn(64, 64, |x, y| {
            image::Rgb([(x * 4) as u8, (y * 4) as u8, 90u8])
        })
        .save(&p)
        .unwrap();
    }
}

/// `write_gradients`, then opens the folder as the app does and waits idle.
fn lone(dir: &std::path::Path, names: &[&str]) -> AppModel {
    write_gradients(dir, names);
    let mut m = support::opened_as_the_app(dir.join("textures"), dir.join("export"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

/// Hands the parked plan results back (stale ones are dropped by generation) and waits for the model to settle.
fn release(m: &mut AppModel) {
    m.hold_plan_results = false;
    for x in m.take_held() {
        m.sender().send(x).unwrap();
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
}

fn new_set_lines(m: &AppModel, from: usize) -> Vec<String> {
    m.messages
        .entries()
        .skip(from)
        .filter(|e| e.text.contains("New set") || e.text.contains("new sets"))
        .map(|e| e.text.clone())
        .collect()
}

#[test]
fn a_type_that_makes_a_set_says_so_with_show() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let last = m.messages.entries().last().unwrap();
    assert_eq!(
        last.text,
        "teeth_albedo2.png is now Diffuse. New set actors/teeth_albedo2 (1 texture)."
    );
    assert!(
        matches!(&last.action, Some(EntryAction::ShowSet { root, id })
        if Some(root) == m.root.as_ref() && id == "actors/teeth_albedo2")
    );
    assert_eq!(
        m.status_note.as_deref(),
        Some("New set actors/teeth_albedo2: see Messages.")
    );
}

#[test]
fn a_type_on_a_file_in_a_carded_set_logs_no_new_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    let diffuse = item_key_ending(&m, "rock01.png");
    m.set_field(&diffuse, "subtype", serde_json::json!("diffuse"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let start = m.messages.entries().count();
    let key = item_key_ending(&m, "rock01_normal.png");
    let set_of = |m: &AppModel| {
        m.sets
            .iter()
            .find(|s| s.members.contains(&key))
            .map(|s| s.key.clone())
    };
    let rock = set_of(&m).expect("premise: rock01_normal.png is in a set before the choice");
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.key == rock),
        "premise: that set has a card"
    );
    m.set_field(&key, "subtype", serde_json::json!("height"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(
        set_of(&m).as_deref(),
        Some(rock.as_str()),
        "premise: the file is still in rock01's set"
    );
    assert!(new_set_lines(&m, start).is_empty());
}

#[test]
fn an_undone_choice_logs_no_new_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    let before = m.messages.entries().count();
    // A: a choice undone before its plan lands logs nothing.
    m.hold_plan_results = true;
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    m.undo();
    release(&mut m);
    assert!(new_set_lines(&m, before).is_empty());
    // B: undo, then another choice, before the plan lands: the line names the type that stands.
    m.hold_plan_results = true;
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    m.undo();
    m.decide(&key, Answer::Subtype(Subtype::Height));
    release(&mut m);
    let lines = new_set_lines(&m, before);
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0],
        "teeth_albedo2.png is now Height (parallax). New set actors/teeth_albedo2 (1 texture)."
    );
}

#[test]
fn several_type_edits_announce_one_card_after_the_final_plan() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    let before = m.messages.entries().count();
    m.hold_plan_results = true;
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    m.decide(&key, Answer::Subtype(Subtype::Height));
    assert!(
        new_set_lines(&m, before).is_empty(),
        "nothing before a plan lands"
    );
    release(&mut m);
    let lines = new_set_lines(&m, before);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("teeth_albedo2.png is now Height (parallax)."));
}

#[test]
fn a_repeated_choice_that_changes_nothing_logs_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let after_first = m.messages.entries().count();
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(new_set_lines(&m, after_first).is_empty());
}

#[test]
fn a_choice_held_across_opening_another_folder_announces_nothing_there() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut m = lone(a.path(), &["actors/teeth_albedo2.png"]);
    write_gradients(b.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    let before = m.messages.entries().count();
    m.hold_plan_results = true;
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    m.hold_plan_results = false;
    m.open_folder(b.path().join("textures"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    release(&mut m);
    assert!(new_set_lines(&m, before).is_empty());
}

#[test]
fn two_choices_that_make_two_sets_log_one_line_with_show_for_the_first() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(
        d.path(),
        &["actors/teeth_albedo2.png", "actors/hair_albedo3.png"],
    );
    let (a, b) = (
        item_key_ending(&m, "teeth_albedo2.png"),
        item_key_ending(&m, "hair_albedo3.png"),
    );
    let before = m.messages.entries().count();
    m.hold_plan_results = true;
    m.decide(&a, Answer::Subtype(Subtype::Diffuse));
    m.decide(&b, Answer::Subtype(Subtype::Diffuse));
    release(&mut m);
    let lines = new_set_lines(&m, before);
    assert_eq!(
        lines,
        ["2 new sets: actors/hair_albedo3 and actors/teeth_albedo2."]
    );
    assert!(matches!(&m.messages.entries().last().unwrap().action,
        Some(EntryAction::ShowSet { id, .. }) if id == "actors/hair_albedo3"));
    assert_eq!(m.status_note.as_deref(), Some("2 new sets: see Messages."));
}

#[test]
fn a_choice_the_read_only_project_refuses_logs_nothing() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("textures/actors")).unwrap();
    std::fs::write(d.path().join("textures/sdc.toml"), "target = [\n").unwrap();
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    assert!(m.project.as_ref().unwrap().read_only(), "premise");
    let key = item_key_ending(&m, "teeth_albedo2.png");
    let before = m.messages.entries().count();
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(new_set_lines(&m, before).is_empty());
}

/// The plan results parked by `hold_plan_results`, once the current generation's full plan is among them.
fn parked(m: &mut AppModel) -> Vec<sdc_gui::worker::WorkerMsg> {
    use sdc_gui::worker::WorkerMsg;
    let mut got = vec![];
    let end = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        m.poll();
        got.extend(m.take_held());
        let generation = m.generation();
        if got
            .iter()
            .any(|x| matches!(x, WorkerMsg::Planned { generation: g, .. } if *g == generation))
        {
            return got;
        }
        assert!(std::time::Instant::now() < end, "no plan result parked");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_reset_after_a_choice_logs_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    let before = m.messages.entries().count();
    m.hold_plan_results = true;
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    m.clear_field(&key, "subtype");
    release(&mut m);
    assert!(new_set_lines(&m, before).is_empty());
}

/// An undo drops the whole group: the other choice, which the undo left standing on a new card, is not announced
/// either (only the forget in `reset_type_follow` keeps it silent).
#[test]
fn an_undo_drops_the_group_even_when_a_kept_choice_sits_on_a_new_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(
        d.path(),
        &["actors/teeth_albedo2.png", "actors/teeth_albedo2_bump.png"],
    );
    let (a, b) = (
        item_key_ending(&m, "teeth_albedo2.png"),
        item_key_ending(&m, "_bump.png"),
    );
    let before = m.messages.entries().count();
    m.hold_plan_results = true;
    m.decide(&a, Answer::Subtype(Subtype::Diffuse));
    m.decide(&b, Answer::Subtype(Subtype::Height));
    assert_eq!(m.new_set_notes_pending(), 2);
    m.undo();
    assert_eq!(m.new_set_notes_pending(), 0);
    release(&mut m);
    let views = sdc_gui::model::sets::views(&m);
    assert!(
        views
            .iter()
            .any(|v| v.members.contains(&a) && v.id == "actors/teeth_albedo2"),
        "premise: the kept choice sits on a card that was not listed before"
    );
    assert!(new_set_lines(&m, before).is_empty());
}

#[test]
fn a_failed_plan_drops_the_queued_choice() {
    use sdc_gui::worker::{PlanFailure, WorkerMsg};
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    let before = m.messages.entries().count();
    m.hold_plan_results = true;
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    let generation = m.generation();
    parked(&mut m);
    m.hold_plan_results = false;
    m.sender()
        .send(WorkerMsg::Planned {
            generation,
            result: Err(PlanFailure {
                message: "boom".into(),
                config: false,
            }),
        })
        .unwrap();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.new_set_notes_pending(), 0);
    // A later plan settles with the choice still written: it must not be announced.
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(new_set_lines(&m, before).is_empty());
}

#[test]
fn a_cancelled_open_drops_the_queued_choice() {
    let d = tempfile::tempdir().unwrap();
    write_gradients(d.path(), &["actors/teeth_albedo2.png"]);
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings::default(),
        None,
        support::session(),
    );
    m.open_folder(d.path().join("textures"));
    m.decide(
        "actors/teeth_albedo2.png",
        Answer::Subtype(Subtype::Diffuse),
    );
    assert_eq!(m.new_set_notes_pending(), 1, "premise: the choice queued");
    assert!(m.can_cancel_open(), "premise: the open is still planning");
    m.cancel_open();
    assert!(m.root.is_none(), "premise: no folder stays open");
    assert_eq!(m.new_set_notes_pending(), 0);
}

#[test]
fn the_final_plan_announces_once_after_a_partial_and_an_intermediate_plan() {
    let d = tempfile::tempdir().unwrap();
    use sdc_gui::worker::WorkerMsg;
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    // An explicit pair the chosen type does not allow: the first full plan settles into a cleanup re-plan
    // (`settle_types`, one of `finish_plan`'s early returns).
    m.set_format(
        &key,
        engine::config::Format::Bc7,
        engine::config::ColorTag::Unorm,
    );
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(
        m.project.as_ref().unwrap().file_value(&key, "format"),
        Some(serde_json::json!("bc7")),
        "premise: the explicit format is written"
    );
    let before = m.messages.entries().count();
    m.hold_plan_results = true;
    m.decide(&key, Answer::Subtype(Subtype::PbrAlbedo));
    let first = parked(&mut m);
    let first_generation = m.generation();
    assert!(
        first.iter().any(|x| matches!(x, WorkerMsg::Partial { .. })),
        "premise: a partial plan was sent for this edit"
    );
    for x in first {
        let planned = matches!(x, WorkerMsg::Planned { .. });
        m.handle_held(x);
        let lines = new_set_lines(&m, before);
        assert!(lines.is_empty(), "{lines:?}");
        assert_eq!(m.new_set_notes_pending(), 1);
        if planned {
            assert!(
                m.generation() > first_generation,
                "premise: the full plan landed in finish_plan and asked for a follow-up plan"
            );
        }
    }
    // The follow-up plan is the final one.
    let last = parked(&mut m);
    m.hold_plan_results = false;
    let generation = m.generation();
    for x in last {
        let is_final = matches!(&x, WorkerMsg::Planned { generation: g, .. } if *g == generation);
        m.handle_held(x);
        if is_final {
            break;
        }
        assert!(new_set_lines(&m, before).is_empty());
        assert_eq!(m.new_set_notes_pending(), 1);
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
    let lines = new_set_lines(&m, before);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("teeth_albedo2.png is now ")
            && lines[0].ends_with(". New set actors/teeth_albedo2 (1 texture)."),
        "{lines:?}"
    );
    let action = m.messages.entries().last().unwrap().action.clone().unwrap();
    assert!(m.logged_set_target(&action).is_some(), "Show resolves");
}

/// Review Focus 9: a partial that applies while a type choice waits announces nothing and keeps the choice queued;
/// the full plan announces it once. The alpha edit after the choice makes the choice's own partial stale, and its own
/// partial carries both `[files]` entries onto the waiting file's set, which the open already landed.
#[test]
fn a_partial_that_applies_keeps_the_choice_for_the_final_plan() {
    use sdc_gui::worker::WorkerMsg;
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    let job = |m: &AppModel| {
        let p = &m.item(&key).unwrap().job.as_ref().unwrap().profile;
        (p.subtype, p.alpha.policy)
    };
    assert_ne!(
        job(&m).1,
        engine::config::AlphaPolicy::Data,
        "premise: the alpha is not data yet"
    );
    let before = m.messages.entries().count();
    m.hold_plan_results = true;
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    m.edit_docs("Data alpha", None, true, |p, _| {
        p.set_file_value(&key, "alpha.policy", &serde_json::json!("data"));
    });
    // Park the alpha edit's partial and the full plan of the current generation.
    let seq = m.edit_seq;
    let end = std::time::Instant::now() + Duration::from_secs(60);
    let mut got = vec![];
    loop {
        m.poll();
        got.extend(m.take_held());
        let generation = m.generation();
        let partial = got
            .iter()
            .any(|x| matches!(x, WorkerMsg::Partial { seq: s, .. } if *s == seq));
        let planned = got
            .iter()
            .any(|x| matches!(x, WorkerMsg::Planned { generation: g, .. } if *g == generation));
        if partial && planned {
            break;
        }
        assert!(
            std::time::Instant::now() < end,
            "no partial and plan parked"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let (partials, full): (Vec<_>, Vec<_>) = got
        .into_iter()
        .partition(|x| matches!(x, WorkerMsg::Partial { .. }));
    for x in partials {
        m.handle_held(x);
    }
    assert_eq!(
        m.applied_seq, seq,
        "premise: the alpha edit's partial applied"
    );
    assert_eq!(
        job(&m),
        (Subtype::Diffuse, engine::config::AlphaPolicy::Data),
        "premise: the partial carries the choice and the alpha"
    );
    assert_eq!(m.new_set_notes_pending(), 1, "the partial kept the choice");
    assert!(new_set_lines(&m, before).is_empty());
    m.hold_plan_results = false;
    for x in full {
        m.handle_held(x);
    }
    assert!(m.wait_idle(Duration::from_secs(60)));
    let lines = new_set_lines(&m, before);
    assert_eq!(
        lines,
        ["teeth_albedo2.png is now Diffuse. New set actors/teeth_albedo2 (1 texture)."]
    );
    assert_eq!(m.new_set_notes_pending(), 0);
}

#[test]
fn a_document_card_gaining_a_key_is_not_new() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    m.edit_docs("t", None, true, |_, s| s.create_set("actors/ghost"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    // Planned as a keyed set of the plan (`m.sets`), or only listed by the document.
    let in_plan = |m: &AppModel| {
        let views = sdc_gui::model::sets::views(m);
        let v = views.iter().find(|v| v.id == "actors/ghost")?;
        Some(m.sets.iter().any(|s| s.key == v.key))
    };
    assert_eq!(
        in_plan(&m),
        Some(false),
        "premise: the document lists a card that no plan set backs"
    );
    let before = m.messages.entries().count();
    // The choice is queued while the card is document-only; then the document puts the file in the card.
    m.hold_plan_results = true;
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    m.edit_docs("t", None, true, |_, s| s.add_member("actors/ghost", &key));
    assert_eq!(m.new_set_notes_pending(), 1);
    release(&mut m);
    assert_eq!(
        in_plan(&m),
        Some(true),
        "premise: the card gained a plan set"
    );
    assert!(new_set_lines(&m, before).is_empty());
}

#[test]
fn an_output_root_rekey_does_not_announce_a_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = support::opened(d.path());
    let before = m.messages.entries().count();
    let key = item_key_ending(&m, "rock01_normal.png");
    let card = |m: &AppModel| {
        sdc_gui::model::sets::views(m)
            .iter()
            .find(|v| v.members.contains(&key))
            .map(|v| (v.id.clone(), v.key.clone()))
            .expect("rock01_normal.png has a card")
    };
    let was = card(&m);
    m.hold_plan_results = true;
    m.set_field(&key, "subtype", serde_json::json!("height"));
    assert_eq!(m.new_set_notes_pending(), 1, "premise: the choice queued");
    m.set_export_folder(d.path().join("other-export"));
    release(&mut m);
    let now = card(&m);
    assert_eq!(was.0, now.0, "premise: the same card ID");
    assert_ne!(
        was.1, now.1,
        "premise: the plan key moved with the output root"
    );
    assert!(new_set_lines(&m, before).is_empty());
}

#[test]
fn an_invalid_type_value_queues_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = lone(d.path(), &["actors/teeth_albedo2.png"]);
    let key = item_key_ending(&m, "teeth_albedo2.png");
    let before = m.messages.entries().count();
    m.set_field(&key, "subtype", serde_json::json!("no_such_type"));
    assert_eq!(m.new_set_notes_pending(), 0);
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(new_set_lines(&m, before).is_empty());
}

#[test]
fn a_settled_entry_stays_in_messages_and_its_show_is_off_in_another_folder() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let mut m = lone(a.path(), &["actors/teeth_albedo2.png"]);
    // B lists a card of the same ID from the start.
    write_gradients(
        b.path(),
        &["actors/teeth_albedo2.png", "actors/teeth_albedo2_n.png"],
    );
    let key = item_key_ending(&m, "teeth_albedo2.png");
    m.decide(&key, Answer::Subtype(Subtype::Diffuse));
    assert!(m.wait_idle(Duration::from_secs(60)));
    let entry = m.messages.entries().last().unwrap().clone();
    let action = entry.action.clone().expect("a Show action");
    assert!(m.logged_set_target(&action).is_some(), "premise: live in A");
    m.open_folder(b.path().join("textures"));
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert!(
        sdc_gui::model::sets::views(&m)
            .iter()
            .any(|v| v.id == "actors/teeth_albedo2"),
        "premise: B has a card of that ID"
    );
    assert!(m.messages.entries().any(|e| *e == entry), "still logged");
    assert!(m.logged_set_target(&action).is_none(), "off in B");
    assert!(!m.show_logged_set(&action));
}
