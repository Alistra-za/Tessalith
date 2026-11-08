//! Note GY: a lone file with an unknown suffix (`rock_bump.png`) waits for a type. It is undecided, with one Problems
//! row and one Review row, an amber tile, no set card, and it writes nothing until a type is assigned.
mod support;
use eframe::egui::accesskit::Role;
use eframe::egui::{containers::panel::PanelState, Id};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::Subtype;
use image::{ImageBuffer, Rgb};
use sdc_gui::model::bottom::{problems, At};
use sdc_gui::model::items::ItemStatus;
use sdc_gui::model::sets::views;
use sdc_gui::model::AppModel;
use std::path::Path;
use std::time::Duration;
use support::*;

const ROCK: &str = "clutter/rock_bump.png";

/// `textures/clutter`: the lone `rock_bump.png`, and a `barrel` set (colour and `_n`) beside it.
fn folder(dir: &Path) -> AppModel {
    let c = dir.join("textures/clutter");
    std::fs::create_dir_all(&c).unwrap();
    let grad = |x: u32, y: u32| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]);
    ImageBuffer::from_fn(64, 64, grad)
        .save(c.join("rock_bump.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, grad)
        .save(c.join("barrel.png"))
        .unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(c.join("barrel_n.png"))
        .unwrap();
    opened_as_the_app(dir.join("textures"), dir.join("export"))
}

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "model did not settle");
}

fn has_card(m: &AppModel, stem: &str) -> bool {
    views(m).iter().any(|v| v.key.ends_with(stem))
}

/// The Problems rows about the rock: its own, and any of a set named after it.
fn rock_rows(m: &AppModel) -> usize {
    problems(m)
        .iter()
        .filter(|p| match &p.at {
            At::Texture(k) => k == ROCK,
            At::Set { key, .. } => key.ends_with("rock_bump"),
            _ => false,
        })
        .count()
}

fn ui_harness(m: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

/// Labels drawn inside the Sets column that name `stem`.
fn sets_column_labels<S>(h: &Harness<'_, S>, stem: &str) -> Vec<String> {
    h.query_all_by(|n| n.label().is_some_and(|l| l.contains(stem)))
        .filter(|n| in_sets_column(h, n.rect()))
        .filter_map(|n| n.accesskit_node().label())
        .collect()
}

/// The texture panel's "← set" link, if drawn.
fn back_link<S>(h: &Harness<'_, S>) -> Option<String> {
    h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with("← ")))
        .find_map(|n| n.accesskit_node().label())
}

/// The first node labelled `label` below `combo` (an entry of its open popup).
fn entry_below<'a, S>(
    h: &'a Harness<'_, S>,
    label: &'a str,
    combo: eframe::egui::Rect,
) -> egui_kittest::Node<'a> {
    h.query_all_by_label(label)
        .find(|n| n.rect().min.y > combo.max.y)
        .unwrap_or_else(|| panic!("{label} in the open combo"))
}

#[test]
fn rock_bump_has_one_problems_row_and_no_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = folder(d.path());
    let rock = m.item(ROCK).expect("the rock is listed");
    assert!(rock.waiting, "rock_bump waits for a type");
    // The amber tile (`columns::border_of`).
    assert_eq!(rock.status, ItemStatus::NeedsDecision);
    assert!(!m.item("clutter/barrel.png").unwrap().waiting);
    assert_eq!(rock_rows(&m), 1, "{:#?}", problems(&m));
    assert!(!has_card(&m, "rock_bump"));
    assert!(has_card(&m, "barrel"));
    // The status line counts the cards.
    assert!(
        m.status_line.contains("3 textures, 1 set"),
        "{}",
        m.status_line
    );
    m.export_clicked();
    assert!(m.review.is_some(), "Review opens on the undecided rock");
    let listed: Vec<_> = m.undecided_keys();
    assert_eq!(listed.iter().filter(|k| *k == ROCK).count(), 1);
    // Drawn: the Sets column has the barrel card and nothing named after the rock; the tile is in Textures.
    let mut h = ui_harness(m);
    h.run();
    assert!(sets_column_labels(&h, "rock_bump").is_empty());
    assert!(!sets_column_labels(&h, "barrel").is_empty());
    let textures = PanelState::load(&h.ctx, Id::new("textures")).expect("the Textures column");
    assert!(
        h.query_all_by_label("rock_bump.png")
            .any(|n| textures.outer_rect.contains_rect(n.rect())),
        "the tile is drawn"
    );
}

#[test]
fn a_waiting_file_does_not_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = folder(d.path());
    m.export_clicked();
    assert!(
        m.review_button_text().contains("(skip 1 undecided)"),
        "{}",
        m.review_button_text()
    );
    m.export_from_review();
    idle(&mut m);
    assert!(m.export.summary.is_some(), "the export ran");
    assert!(d.path().join("export/clutter/barrel.dds").exists());
    assert!(!d.path().join("export/clutter/rock_bump.dds").exists());
}

#[test]
fn a_waiting_file_counts_as_undecided() {
    let d = tempfile::tempdir().unwrap();
    let m = folder(d.path());
    assert_eq!(m.undecided(), 1, "{:?}", m.undecided_keys());
    assert_eq!(m.undecided_keys(), vec![ROCK.to_string()]);
}

#[test]
fn assigning_a_type_brings_the_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = folder(d.path());
    m.select_item(ROCK);
    let mut h = ui_harness(m);
    h.run();
    assert!(back_link(&h).is_none(), "no link to a set with no card");
    // The Undecided box: its combo sits on the Confirm button's row.
    let confirm_y = h.get_by_label("Confirm").rect().min.y;
    let node = h
        .query_all_by_role(Role::ComboBox)
        .min_by_key(|n| (n.rect().min.y - confirm_y).abs() as i64)
        .expect("the sub-type combo");
    let combo = node.rect();
    node.click();
    h.run();
    // The popup's entry (`subtype_label`) below the combo, scrolled into the popup's view first: it starts clipped.
    entry_below(&h, "diffuse", combo).scroll_to_me();
    h.run();
    entry_below(&h, "diffuse", combo).click();
    h.run();
    h.get_by_label("Confirm").click();
    h.run();
    idle(h.state_mut());
    h.run();
    let m = h.state();
    assert_eq!(
        m.item(ROCK)
            .and_then(|i| i.job.as_ref())
            .map(|j| j.profile.subtype),
        Some(Subtype::Diffuse)
    );
    assert!(!m.item(ROCK).unwrap().waiting);
    assert!(has_card(m, "rock_bump"), "a card appears after the plan");
    assert!(!sets_column_labels(&h, "rock_bump").is_empty());
    assert_eq!(back_link(&h).as_deref(), Some("← clutter/rock_bump"));
    h.state_mut().undo();
    idle(h.state_mut());
    h.run();
    let m = h.state();
    assert!(m.item(ROCK).unwrap().waiting);
    assert!(!has_card(m, "rock_bump"), "Undo takes the card away");
    assert!(sets_column_labels(&h, "rock_bump").is_empty());
}

#[test]
fn the_first_open_records_no_decision() {
    let d = tempfile::tempdir().unwrap();
    let m = folder(d.path());
    let doc = m.project.as_ref().expect("the project");
    // The recorder ran: the barrel's High pick is recorded.
    assert!(doc.file_value("clutter/barrel.png", "subtype").is_some());
    assert_eq!(doc.file_value(ROCK, "subtype"), None);
}

/// Task 1's trap: `ResolvedJob::unknown_suffix` is not serialised. A reopen on a warm analysis cache plans the rock
/// again and still finds it waiting.
#[test]
fn a_reopen_with_a_warm_cache_still_waits() {
    let d = tempfile::tempdir().unwrap();
    let first = folder(d.path());
    let session = first.session.clone().expect("the session");
    drop(first);
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..Default::default()
        },
        None,
        session,
    );
    m.open_folder(d.path().join("textures"));
    idle(&mut m);
    assert!(m.item(ROCK).unwrap().waiting);
    assert_eq!(rock_rows(&m), 1);
    assert!(!has_card(&m, "rock_bump"));
}
