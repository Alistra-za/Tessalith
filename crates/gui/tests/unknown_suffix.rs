//! Note GY: a lone file with an unknown suffix (`rock_bump.png`) waits for a type. It is undecided, with one Problems
//! row and one Review row, an amber tile, no set card, and it writes nothing until a type is assigned.
mod support;
use eframe::egui::accesskit::Role;
use eframe::egui::{containers::panel::PanelState, Id};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::Subtype;
use engine::shader::Shader;
use image::{ImageBuffer, Rgb};
use sdc_gui::model::bottom::{problems, At};
use sdc_gui::model::items::ItemStatus;
use sdc_gui::model::sets::{slot_of, views};
use sdc_gui::model::{Answer, AppModel, Selection};
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

fn card_key(m: &AppModel, stem: &str) -> Option<String> {
    views(m)
        .iter()
        .find(|v| v.key.ends_with(stem))
        .map(|v| v.key.clone())
}

fn has_card(m: &AppModel, stem: &str) -> bool {
    card_key(m, stem).is_some()
}

/// The Problems rows of the rock file itself.
fn texture_rows(m: &AppModel) -> usize {
    problems(m)
        .iter()
        .filter(|p| p.at == At::Texture(ROCK.into()))
        .count()
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
    // The status line and the Sets pills count the cards.
    assert!(
        m.status_line.contains("3 textures, 1 set"),
        "{}",
        m.status_line
    );
    let c = m.set_counts();
    assert_eq!(c.ready + c.attention, 1, "{c:?}");
    // Minor 13: the row asks for a sub-type; the CLI's `--set` advice stays on the command line.
    for p in problems(&m)
        .iter()
        .filter(|p| p.at == At::Texture(ROCK.into()))
    {
        assert!(!p.text.contains("--set"), "{}", p.text);
        assert!(
            !p.detail.as_deref().unwrap_or("").contains("--set"),
            "{:?}",
            p.detail
        );
    }
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

/// Every file under `dir`, recursively.
fn files_under(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(files_under(&p));
        } else {
            out.push(p);
        }
    }
    out
}

/// Final review Minor 3: no DDS, no JSON entry (PGPatcher, CS PBR, complex material), no `_sdc/generated.json` hash and
/// nothing from Also export names the waiting file.
#[test]
fn a_waiting_file_does_not_export() {
    let d = tempfile::tempdir().unwrap();
    let mut m = folder(d.path());
    for t in [
        engine::presets::AlsoTarget::Vanilla,
        engine::presets::AlsoTarget::Cm,
    ] {
        m.set_also_tick(t, true);
        idle(&mut m);
    }
    assert!(m.also_on(), "the ticks took: {}", m.status_line);
    m.export_clicked();
    assert!(
        m.review_button_text().contains("(skip 1 undecided)"),
        "{}",
        m.review_button_text()
    );
    m.export_from_review();
    idle(&mut m);
    assert!(m.export.summary.is_some(), "the export ran");
    // With a tick on, the export goes to the PBR folder (TA10) and the Vanilla folder gets its copy.
    for dir in m.also_folders().iter().take(2) {
        assert!(
            files_under(dir).iter().any(|p| p.ends_with("barrel.dds")),
            "{dir:?}: {:?}",
            files_under(d.path())
        );
    }
    let source = d.path().join("textures/clutter/rock_bump.png");
    for p in files_under(d.path()).into_iter().filter(|p| *p != source) {
        let name = p.file_name().unwrap().to_string_lossy().to_lowercase();
        assert!(!name.contains("rock_bump"), "{p:?}");
        if name.ends_with(".json") || name.ends_with(".toml") {
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            assert!(!text.contains("rock_bump"), "{p:?}: {text}");
        }
    }
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
    // The card is selected when Undo takes it away: the selection goes with it.
    let key = card_key(m, "rock_bump").expect("the card");
    h.state_mut().select_set(&key);
    h.state_mut().undo();
    idle(h.state_mut());
    h.run();
    let m = h.state();
    assert!(m.item(ROCK).unwrap().waiting);
    assert!(!has_card(m, "rock_bump"), "Undo takes the card away");
    assert!(sets_column_labels(&h, "rock_bump").is_empty());
    assert_eq!(m.selection, Selection::None);
    // Minor 13: Redo brings it back.
    h.state_mut().redo();
    idle(h.state_mut());
    h.run();
    let m = h.state();
    assert!(!m.item(ROCK).unwrap().waiting);
    assert!(has_card(m, "rock_bump"), "Redo brings the card back");
    assert!(!sets_column_labels(&h, "rock_bump").is_empty());
}

/// Controller ruling: a set the user created keeps its card, and its only member still waits.
#[test]
fn a_created_set_keeps_its_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = folder(d.path());
    let id = m.drop_new_set(ROCK).expect("a new set");
    idle(&mut m);
    let v = views(&m);
    let card = v
        .iter()
        .find(|v| v.id == id)
        .expect("the created set's card");
    assert_eq!(card.members, vec![ROCK.to_string()]);
    assert!(m.item(ROCK).unwrap().waiting);
    assert_eq!(texture_rows(&m), 1, "{:#?}", problems(&m));
}

/// Controller ruling: a set the user set something on (GY2's explicit shader) keeps its card and the choice.
#[test]
fn a_set_with_a_chosen_shader_keeps_its_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = folder(d.path());
    m.set_shader("clutter/rock_bump", Some(Shader::AdvancedSkin));
    idle(&mut m);
    let v = views(&m);
    let card = v
        .iter()
        .find(|v| v.key.ends_with("rock_bump"))
        .expect("the card stays");
    assert_eq!(card.shader_choice, Some(Shader::AdvancedSkin));
    assert!(
        m.item(ROCK).unwrap().waiting,
        "a set shader assigns no type (GY2)"
    );
    assert_eq!(texture_rows(&m), 1);
    // Minor 14: back to Auto leaves an entry that holds only its id, which is no choice: the card goes, and no
    // "missing" card takes its place.
    m.set_shader("clutter/rock_bump", None);
    idle(&mut m);
    assert!(
        !views(&m).iter().any(|v| v.id.ends_with("rock_bump")),
        "{:?}",
        views(&m).iter().map(|v| &v.id).collect::<Vec<_>>()
    );
    assert_eq!(rock_rows(&m), 1, "{:#?}", problems(&m));
}

/// Minor 14: the saved order list is no choice either. A `[project] order` naming the rock's set keeps no card.
#[test]
fn an_order_entry_keeps_no_card() {
    let d = tempfile::tempdir().unwrap();
    let mut m = folder(d.path());
    let ids: Vec<String> = ["clutter/rock_bump", "clutter/barrel"]
        .map(String::from)
        .to_vec();
    m.sets_doc
        .as_mut()
        .expect("the sets document")
        .set_order(&ids);
    m.replan();
    idle(&mut m);
    assert!(!has_card(&m, "rock_bump"));
    assert!(has_card(&m, "barrel"));
}

/// Minor 2 (ruled correct): an Error wins over waiting. The file is in error, not waiting, so its set keeps its card
/// and it writes nothing.
#[test]
fn an_error_keeps_the_card_and_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("textures")).unwrap();
    std::fs::write(
        d.path().join("textures/sdc.toml"),
        "[files.\"clutter/rock_bump.png\"]\nformat = \"bc6h_uf16\"\n",
    )
    .unwrap();
    let mut m = folder(d.path());
    let rock = m.item(ROCK).expect("the rock");
    assert!(
        matches!(rock.status, ItemStatus::Error(_)),
        "{:?}",
        rock.status
    );
    assert!(!rock.waiting);
    assert!(has_card(&m, "rock_bump"));
    m.export_clicked();
    m.export_from_review();
    idle(&mut m);
    assert!(!d.path().join("export/clutter/rock_bump.dds").exists());
}

/// Note GY6: the app asks the scan about the waiting file (its colour mesh key), and a mesh that uses it in slot 0
/// makes it a diffuse with a card, recorded as the GUI records any High pick.
#[test]
fn a_mesh_that_uses_the_file_settles_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = folder(d.path());
    let key = m
        .mesh_vote_keys()
        .into_iter()
        .find(|k| k.ends_with("clutter/rock_bump"))
        .expect("the rock's mesh key is read");
    let shape = engine::shader::MeshAlpha {
        blend: false,
        test: false,
        threshold: 0,
    };
    m.mesh_alpha = [(key, vec![shape])].into();
    m.replan();
    idle(&mut m);
    let rock = m.item(ROCK).unwrap();
    assert!(!rock.waiting);
    assert_eq!(
        rock.job.as_ref().map(|j| j.profile.subtype),
        Some(Subtype::Diffuse)
    );
    assert!(has_card(&m, "rock_bump"));
    let doc = m.project.as_ref().expect("the project");
    assert!(doc.file_value(ROCK, "subtype").is_some());
}

/// Important 4 (a): Problems builds set rows from the views, so a waiting file under a `pbr` folder, whose set the
/// engine checks as a PBR set, has its own row only.
#[test]
fn a_waiting_pbr_file_has_no_set_rows() {
    let d = tempfile::tempdir().unwrap();
    let c = d.path().join("textures/pbr");
    std::fs::create_dir_all(&c).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(c.join("rock_bump.png"))
        .unwrap();
    let m = opened_as_the_app(d.path().join("textures"), d.path().join("export"));
    let key = "pbr/rock_bump.png";
    assert!(m.item(key).expect("the rock").waiting);
    let set = m
        .sets
        .iter()
        .find(|s| s.key.ends_with("pbr/rock_bump"))
        .expect("the plan's set");
    eprintln!("set-level warnings: {:?}", set.warnings);
    let all = problems(&m);
    let rows: Vec<_> = all
        .iter()
        .filter(|p| match &p.at {
            At::Texture(k) => k == key,
            At::Set { key, .. } => key.ends_with("rock_bump"),
            _ => false,
        })
        .collect();
    assert_eq!(rows.len(), 1, "{rows:#?}");
    assert!(!has_card(&m, "rock_bump"));
}

/// `sets::view_reads` carries `waiting`: an explicit `generic_data` changes neither the slot nor the sub-type, only
/// whether the file waits, and the card must still come. The fast re-plan's partial would bring it on its own, so it
/// is dropped: only the full plan lands, on a warm views cache that it splices (`full_plan_changes`).
#[test]
fn a_type_that_changes_only_waiting_brings_the_card() {
    use sdc_gui::worker::WorkerMsg;
    let d = tempfile::tempdir().unwrap();
    let mut m = folder(d.path());
    let reads = |m: &AppModel| {
        let i = m.item(ROCK).unwrap();
        (slot_of(i), i.job.as_ref().map(|j| j.profile.subtype))
    };
    let before = reads(&m);
    assert!(!has_card(&m, "rock_bump"), "the views cache is warm");
    m.hold_plan_results = true;
    m.decide(ROCK, Answer::Subtype(Subtype::GenericData));
    let end = std::time::Instant::now() + Duration::from_secs(60);
    let full = loop {
        m.poll();
        let held = m.take_held();
        if let Some(x) = held
            .into_iter()
            .find(|x| !matches!(x, WorkerMsg::Partial { .. }))
        {
            break x;
        }
        assert!(
            std::time::Instant::now() < end,
            "the full plan did not arrive"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    m.hold_plan_results = false;
    m.handle_held(full);
    idle(&mut m);
    assert_eq!(before, reads(&m), "only waiting moved");
    assert!(!m.item(ROCK).unwrap().waiting);
    assert!(has_card(&m, "rock_bump"));
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

/// Task 1's trap: `ResolvedJob::unknown_suffix` is not serialised. No GUI path deserialises a job (worker messages,
/// the analysis store and the fast re-plan all keep them in memory), so this pins a re-plan: a second model on the same
/// session plans the rock again and still finds it waiting.
#[test]
fn a_replan_on_the_same_session_still_waits() {
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
