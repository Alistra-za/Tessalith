//! Packing fills Task 2: per-output fills, the RMAOS/RFAOS choice on the packed card, the Advanced skin follow, the
//! dropdown groups and the retired global fill panel.
mod support;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::codes::Code;
use engine::config::{PackRecipe, RawMap, Subtype};
use engine::shader::Shader;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::bottom::{problems, At, Fix};
use sdc_gui::model::pack_fills::{fill_rows, FillRow};
use sdc_gui::model::pbr_export::{export_blocks, json_entries};
use sdc_gui::model::sets::{set_views, views, SlotFill, SlotKind};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use support::session;

const DIFFUSE: &str = "m/rock01.png";
const SK: &str = "m/rock01_sk.png";
const RMAOS: &str = "m/rock01_rmaos";
const RFAOS: &str = "m/rock01_rfaos";

fn grey(dir: &Path, name: &str, v: u8) {
    ImageBuffer::from_fn(64, 64, |x, _| Luma([v.wrapping_add(x as u8)]))
        .save(dir.join(name))
        .unwrap();
}

/// A diffuse, a roughness map and a `_sk`: `_rmaos` with no AO or metallic source.
fn fixture_with(dir: &Path, sdc_toml: &str, second_set: bool) -> AppModel {
    let r = dir.join("m");
    std::fs::create_dir_all(&r).unwrap();
    if !sdc_toml.is_empty() {
        std::fs::write(dir.join("sdc.toml"), sdc_toml).unwrap();
    }
    for base in ["rock01", "rock02"] {
        if base == "rock02" && !second_set {
            continue;
        }
        ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
            .save(r.join(format!("{base}.png")))
            .unwrap();
        grey(&r, &format!("{base}_roughness.png"), 120);
    }
    grey(&r, "rock01_sk.png", 0);
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    m
}

fn fixture(dir: &Path) -> AppModel {
    fixture_with(dir, "", false)
}

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

fn subtype(m: &AppModel, key: &str) -> Option<serde_json::Value> {
    m.project.as_ref().unwrap().file_value(key, "subtype")
}

fn resolved(m: &AppModel, key: &str) -> Subtype {
    m.item(key).unwrap().job.as_ref().unwrap().profile.subtype
}

fn texts(m: &AppModel) -> (String, String) {
    (
        m.project.as_ref().unwrap().text(),
        m.sets_doc.as_ref().unwrap().text(),
    )
}

fn channel(m: &AppModel, key: &str, which: &str) -> String {
    m.item(key).unwrap().job.as_ref().unwrap().provenance[which].to_string()
}

fn roles(m: &AppModel, key: &str) -> Vec<RawMap> {
    fill_rows(m, key)
        .into_iter()
        .map(|r| match r {
            FillRow::FromFile { role, .. } | FillRow::Value { role, .. } => role,
        })
        .collect()
}

fn editor(mut m: AppModel) -> Harness<'static, AppModel> {
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with("m/rock01"))
        .expect("the rock01 set")
        .key
        .clone();
    m.select_set(&key);
    let h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    h
}

/// The whole app in a 1400x900 window.
fn app(m: AppModel) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m)
}

/// The id of the rock01 set.
fn set_id(m: &AppModel) -> String {
    views(m)
        .iter()
        .find(|v| v.key.ends_with("m/rock01"))
        .expect("the rock01 set")
        .id
        .clone()
}

fn settle(h: &mut Harness<'static, AppModel>) {
    h.run();
    h.state_mut().wait_idle(Duration::from_secs(60));
    h.run();
}

#[test]
fn an_rmaos_card_without_an_ao_source_has_an_ao_row_and_a_set_value_becomes_the_channel() {
    let d = tempfile::tempdir().unwrap();
    let mut h = editor(fixture(d.path()));
    h.run();
    h.get_by_label("AO");
    // Note DY18: a channel with a file names it in its source picker.
    assert_eq!(
        h.get_by_role_and_label(
            eframe::egui::accesskit::Role::ComboBox,
            "roughness channel source"
        )
        .value()
        .as_deref(),
        Some("Source: rock01_roughness.png")
    );
    assert_eq!(
        roles(h.state(), RMAOS),
        vec![
            RawMap::Roughness,
            RawMap::Metallic,
            RawMap::Ao,
            RawMap::Specular
        ]
    );
    h.state_mut().set_pack_fill(RMAOS, RawMap::Ao, Some(200));
    settle(&mut h);
    let m = h.state();
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(RMAOS, "packing.fill.ao"),
        Some(json!(200)),
        "the fill is the packed output's own key"
    );
    // The card never writes a project-wide fill: `[defaults]` stays out of the file text.
    let text = m.project.as_ref().unwrap().text();
    assert!(!text.contains("[defaults"), "{text}");
    assert_eq!(channel(m, RMAOS, "pack.b"), "fill:200");
    // The reset removes the key again (bring it into view: the set editor scrolls).
    h.get_by_label("Reset").scroll_to_me();
    h.run();
    h.get_by_label("Reset").click();
    settle(&mut h);
    let m = h.state();
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(RMAOS, "packing.fill.ao"),
        None
    );
    assert!(!channel(m, RMAOS, "pack.b").starts_with("fill:"));
}

/// Note FN2: typing 200 into a Fills box is one edit (one undo step), written when the box is left, not one per
/// keystroke.
#[test]
fn typing_into_a_fill_box_is_one_edit() {
    use eframe::egui::{Event, Key, Modifiers};
    let d = tempfile::tempdir().unwrap();
    let mut h = editor(fixture(d.path()));
    h.run();
    fn ao<'a>(h: &'a Harness<'static, AppModel>) -> egui_kittest::Node<'a> {
        h.get_by_role_and_label(eframe::egui::accesskit::Role::SpinButton, "AO fill")
    }
    ao(&h).scroll_to_me();
    h.run();
    ao(&h).focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    for c in ["2", "0", "0"] {
        h.event(Event::Text(c.into()));
        h.run();
    }
    let fill = |m: &AppModel| {
        m.project
            .as_ref()
            .unwrap()
            .file_value(RMAOS, "packing.fill.ao")
    };
    assert_eq!(fill(h.state()), None, "nothing is written while typing");
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(fill(h.state()), Some(json!(200)));
    assert_eq!(h.state().undo_label(), Some("Fill value"));
    h.state_mut().undo();
    settle(&mut h);
    assert_eq!(fill(h.state()), None, "one undo step");
    assert!(!h.state().can_undo(), "no earlier partial edit");
}

/// Note FN2: an arrow key on a focused Fills box changes the value, and leaving the box (Tab) writes it as one step.
/// ArrowDown, since the AO fill starts at its maximum (255).
#[test]
fn an_arrow_key_on_a_fill_box_is_written_when_the_box_is_left() {
    use eframe::egui::Key;
    let d = tempfile::tempdir().unwrap();
    let mut h = editor(fixture(d.path()));
    h.run();
    fn ao<'a>(h: &'a Harness<'static, AppModel>) -> egui_kittest::Node<'a> {
        h.get_by_role_and_label(eframe::egui::accesskit::Role::SpinButton, "AO fill")
    }
    ao(&h).scroll_to_me();
    h.run();
    ao(&h).focus();
    h.run();
    h.key_press(Key::ArrowDown);
    h.run();
    h.key_press(Key::Tab);
    settle(&mut h);
    let fill = h
        .state()
        .project
        .as_ref()
        .unwrap()
        .file_value(RMAOS, "packing.fill.ao");
    assert_eq!(fill, Some(json!(254)));
    assert_eq!(h.state().undo_label(), Some("Fill value"));
    h.state_mut().undo();
    settle(&mut h);
    assert!(!h.state().can_undo(), "one undo step");
}

#[test]
fn an_rmaos_card_never_shows_a_fuzz_row() {
    let d = tempfile::tempdir().unwrap();
    let mut h = editor(fixture(d.path()));
    h.run();
    assert!(h.query_by_label("fuzz").is_none());
    assert!(h.query_by_label("opacity").is_none());
    h.get_by_label("metallic");
    assert!(!roles(h.state(), RMAOS).contains(&RawMap::Fuzz));
    // Choosing _rmaos writes no fuzz fill.
    h.state_mut().set_packing_recipe(RMAOS, PackRecipe::Rmaos);
    settle(&mut h);
    let p = h.state().project.as_ref().unwrap();
    assert_eq!(p.file_value(RMAOS, "packing.fill.fuzz"), None);
    assert_eq!(p.file_value(RFAOS, "packing.fill.fuzz"), None);
}

#[test]
fn rfaos_shows_a_fuzz_row_hides_metallic_and_switches_the_inferred_members() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    assert_eq!(resolved(&m, DIFFUSE), Subtype::PbrAlbedo);
    assert_eq!(resolved(&m, SK), Subtype::BacklightTint);
    m.set_pack_fill(RMAOS, RawMap::Ao, Some(77));
    m.set_pack_fill(RMAOS, RawMap::Metallic, Some(9));
    idle(&mut m);
    let before = texts(&m);
    let mut h = editor(m);
    h.run();
    h.state_mut().set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    settle(&mut h);
    h.run();
    h.get_by_label("fuzz");
    assert!(h.query_by_label("metallic").is_none());
    let m = h.state();
    assert_eq!(
        roles(m, RFAOS),
        vec![
            RawMap::Roughness,
            RawMap::Fuzz,
            RawMap::Ao,
            RawMap::Specular
        ]
    );
    // The recipe is on the _rfaos key, the contradicting one is gone, and the fill followed the output.
    let p = m.project.as_ref().unwrap();
    assert_eq!(p.file_value(RFAOS, "packing.recipe"), Some(json!("rfaos")));
    assert_eq!(p.file_value(RMAOS, "packing.recipe"), None);
    assert_eq!(p.file_value(RFAOS, "packing.fill.ao"), Some(json!(77)));
    assert_eq!(p.file_value(RMAOS, "packing.fill.ao"), None);
    assert_eq!(p.file_value(RMAOS, "packing.fill.metallic"), None);
    assert!(m.unmatched_keys.is_empty(), "{:?}", m.unmatched_keys);
    // The inferred members follow the engine's suggestion: nothing was written for them.
    assert_eq!(subtype(m, DIFFUSE), None);
    assert_eq!(subtype(m, SK), None);
    assert_eq!(resolved(m, DIFFUSE), Subtype::SkinAlbedo);
    assert_eq!(resolved(m, SK), Subtype::SkinSk);
    assert!(!m.sets_doc.as_ref().unwrap().text().contains("auto_skin"));
    // Note FC3 (supersedes R-fills-2's card note): the dropped fill is a Problems Info row.
    assert!(sdc_gui::model::bottom::problems(m)
        .iter()
        .any(|p| p.text == "metallic fill removed (RFAOS has no metallic channel)"));
    let v = set_views(m);
    let v = v.iter().find(|v| v.id == "m/rock01").unwrap();
    assert!(matches!(
        v.slots.get(&SlotKind::SkinSk),
        Some(SlotFill::Own { .. })
    ));
    // One undo restores both documents.
    h.state_mut().undo();
    settle(&mut h);
    assert_eq!(texts(h.state()), before);
    assert_eq!(resolved(h.state(), DIFFUSE), Subtype::PbrAlbedo);
    assert_eq!(resolved(h.state(), SK), Subtype::BacklightTint);
}

#[test]
fn switching_back_to_rmaos_reverts_the_inferred_members_and_keeps_user_set_ones() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    assert_eq!(resolved(&m, SK), Subtype::SkinSk);
    m.set_field(SK, "subtype", json!("glow"));
    idle(&mut m);
    m.set_packing_recipe(RFAOS, PackRecipe::Rmaos);
    idle(&mut m);
    assert_eq!(resolved(&m, DIFFUSE), Subtype::PbrAlbedo, "inferred again");
    assert_eq!(subtype(&m, DIFFUSE), None);
    assert_eq!(
        subtype(&m, SK),
        Some(json!("glow")),
        "the hand-set one stays"
    );
    let p = m.project.as_ref().unwrap();
    assert_eq!(p.file_value(RMAOS, "packing.recipe"), Some(json!("rmaos")));
    assert_eq!(p.file_value(RFAOS, "packing.recipe"), None);
    assert!(m.item(RMAOS).is_some());
}

#[test]
fn members_the_user_set_are_left_alone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_field(DIFFUSE, "subtype", json!("diffuse"));
    m.set_field(SK, "subtype", json!("backlight_tint"));
    idle(&mut m);
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    assert_eq!(subtype(&m, DIFFUSE), Some(json!("diffuse")));
    assert_eq!(subtype(&m, SK), Some(json!("backlight_tint")));
    assert_eq!(resolved(&m, DIFFUSE), Subtype::Diffuse);
    assert_eq!(resolved(&m, SK), Subtype::BacklightTint);
}

#[test]
fn a_read_only_project_file_does_not_stop_the_recipe() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("sdc-project.toml"), "[[sets]\nbroken").unwrap();
    let mut m = fixture(d.path());
    assert!(m.sets_doc.as_ref().unwrap().read_only());
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(RFAOS, "packing.recipe"),
        Some(json!("rfaos"))
    );
    assert_eq!(subtype(&m, DIFFUSE), None, "no member was written");
    assert_eq!(
        resolved(&m, DIFFUSE),
        Subtype::SkinAlbedo,
        "the engine follows"
    );
    assert!(m.item(RFAOS).is_some(), "the recipe took effect");
}

#[test]
fn a_read_only_sdc_toml_disables_the_fills_and_says_why() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    let bad = d.path().join("bad");
    std::fs::create_dir_all(&bad).unwrap();
    std::fs::write(bad.join("sdc.toml"), "this is = = not toml\n").unwrap();
    m.project = Some(sdc_gui::project::ProjectDoc::open(&bad));
    assert!(m.project.as_ref().unwrap().read_only());
    // The model refuses too.
    m.set_pack_fill(RMAOS, RawMap::Ao, Some(1));
    // A note from an earlier switch: a refused call must leave it (the guard returns before `dropped_fills.clear()`;
    // the document's own refusal alone would not show in the text).
    m.dropped_fills
        .insert(RMAOS.into(), "metallic fill removed".into());
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    assert_eq!(m.dropped_fills.len(), 1, "a refused switch touches nothing");
    assert_eq!(
        m.project.as_ref().unwrap().file_value(RMAOS, "packing"),
        None
    );
    let mut h = editor(m);
    h.run();
    h.run();
    let picker = h.get_by_role_and_label(
        eframe::egui::accesskit::Role::ComboBox,
        "roughness channel source",
    );
    assert!(picker.accesskit_node().is_disabled());
    // The value box on each named fill row is disabled; the controls are found by their row's label.
    for name in ["metallic", "AO", "specular"] {
        let y = h.get_by_label(name).accesskit_node().raw_bounds().unwrap();
        let row: Vec<_> = h
            .query_all_by_role(eframe::egui::accesskit::Role::SpinButton)
            .filter(|n| {
                let b = n.accesskit_node().raw_bounds().unwrap();
                (b.y0 - y.y0).abs() < 6.0
            })
            .collect();
        assert_eq!(row.len(), 1, "one value box on the {name} row");
        assert!(row[0].accesskit_node().is_disabled(), "{name}");
    }
}

/// A1-6 (was `the_recipe_selector_shows_the_chosen_recipe_with_auto`): the slot title is the recipe's one display
/// (HE1); no combo shows it.
#[test]
fn the_packed_slot_title_follows_the_planned_recipe() {
    let d = tempfile::tempdir().unwrap();
    let mut h = editor(fixture(d.path()));
    settle(&mut h);
    let title = |h: &Harness<'static, AppModel>, t: &str| {
        h.query_all_by_label(t)
            .any(|n| !support::in_sets_column(h, n.rect()))
    };
    let combo_reads = |h: &Harness<'static, AppModel>| {
        h.query_all_by_role(eframe::egui::accesskit::Role::ComboBox)
            .any(|n| {
                n.value()
                    .is_some_and(|v| v.starts_with("RMAOS") || v.starts_with("RFAOS"))
            })
    };
    assert!(title(&h, "RMAOS"), "auto: the slot title reads RMAOS");
    assert!(!combo_reads(&h), "no recipe combo");
    h.state_mut().set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    settle(&mut h);
    assert!(title(&h, "RFAOS"), "a written rfaos: the title follows");
    assert!(!combo_reads(&h));
}

#[test]
fn two_sets_with_different_ao_fills_pack_different_channels() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture_with(d.path(), "", true);
    m.set_pack_fill(RMAOS, RawMap::Ao, Some(10));
    m.set_pack_fill("m/rock02_rmaos", RawMap::Ao, Some(200));
    idle(&mut m);
    assert_eq!(channel(&m, RMAOS, "pack.b"), "fill:10");
    assert_eq!(channel(&m, "m/rock02_rmaos", "pack.b"), "fill:200");
}

#[test]
fn the_dropdown_has_an_advanced_skin_group_after_pbr_and_other_last() {
    use sdc_gui::ui::settings_panel::{subtype_groups, OTHER_HEADING, PBR_HEADING, SKIN_HEADING};
    let groups = subtype_groups(|_| true);
    assert_eq!(groups[0].0, PBR_HEADING);
    assert_eq!(groups[1].0, SKIN_HEADING);
    assert_eq!(SKIN_HEADING, "Advanced skin (Community Shaders)");
    assert_eq!(
        groups[1].1,
        vec![
            Subtype::SkinAlbedo,
            Subtype::SkinRfaos,
            Subtype::SkinSk,
            Subtype::SkinWet,
            Subtype::SkinDetail
        ]
    );
    let last = groups.last().unwrap();
    assert_eq!(
        (last.0, last.1.clone()),
        (OTHER_HEADING, vec![Subtype::UiSky, Subtype::Cubemap])
    );
    assert!(groups
        .iter()
        .filter(|g| g.0 != OTHER_HEADING)
        .all(|g| !g.1.contains(&Subtype::UiSky)));
    assert!(!groups[0].1.contains(&Subtype::SkinRfaos));
    let mut all: Vec<Subtype> = groups.iter().flat_map(|(_, v)| v.clone()).collect();
    all.sort_by_key(|s| s.id());
    all.dedup();
    assert_eq!(all.len(), Subtype::ALL.len(), "each exactly once");
}

#[test]
fn the_open_dropdown_draws_the_skin_group_after_pbr_and_other_last() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.select_item(DIFFUSE);
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1400.0, 1200.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    h.run();
    h.query_all_by_role(eframe::egui::accesskit::Role::ComboBox)
        .find(|n| n.value().as_deref() == Some("pbr albedo"))
        .expect("the sub-type combo")
        .click();
    h.run();
    h.run();
    let y = |label: &str| {
        h.get_by_label(label)
            .accesskit_node()
            .raw_bounds()
            .unwrap()
            .y0
    };
    assert!(y("PBR (Community Shaders)") < y("Advanced skin (Community Shaders)"));
    assert!(y("Advanced skin (Community Shaders)") < y("Skin RFAOS"));
    assert!(y("Skin RFAOS") < y("Colour maps"));
    assert!(y("Other") > y("Grayscale maps"));
    assert!(y("ui sky") > y("Other"));
}

fn note(m: &AppModel, key: &str) -> Option<String> {
    m.dropped_fills.get(key).cloned()
}

#[test]
fn a_dropped_fill_note_does_not_survive_reopening_the_folder() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_pack_fill(RMAOS, RawMap::Metallic, Some(9));
    idle(&mut m);
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    assert!(note(&m, RFAOS).unwrap().contains("metallic fill removed"));
    m.open_folder(d.path().to_path_buf());
    idle(&mut m);
    assert!(m.dropped_fills.is_empty());
}

#[test]
fn only_the_latest_switchs_notes_show() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_pack_fill(RMAOS, RawMap::Metallic, Some(9));
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    assert!(note(&m, RFAOS).is_some());
    m.set_pack_fill(RFAOS, RawMap::Fuzz, Some(30));
    idle(&mut m);
    m.set_packing_recipe(RFAOS, PackRecipe::Rmaos);
    idle(&mut m);
    assert!(note(&m, RFAOS).is_none(), "the old note is gone");
    assert!(note(&m, RMAOS).unwrap().contains("fuzz fill removed"));
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    assert!(m.dropped_fills.is_empty(), "{:?}", m.dropped_fills);
}

#[test]
fn a_moved_fill_never_overwrites_the_destination_silently() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_pack_fill(RMAOS, RawMap::Ao, Some(128));
    m.set_pack_fill(RFAOS, RawMap::Ao, Some(64));
    idle(&mut m);
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    let p = m.project.as_ref().unwrap();
    assert_eq!(p.file_value(RFAOS, "packing.fill.ao"), Some(json!(64)));
    assert_eq!(p.file_value(RMAOS, "packing.fill.ao"), None);
    assert_eq!(
        note(&m, RFAOS).unwrap(),
        "ao fill 128 not moved: the _rfaos entry already had 64"
    );
}

#[test]
fn complex_m_has_glossiness_metallic_and_height_rows_and_vanilla_m_none() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    grey(&d.path().join("m"), "rock03_metallic.png", 5);
    m.open_folder(d.path().to_path_buf());
    idle(&mut m);
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    support::complex_material(&mut m, "m/rock01");
    assert_eq!(
        roles(&m, "m/rock01_m"),
        vec![RawMap::Glossiness, RawMap::Metallic, RawMap::Height]
    );
    assert!(m.item("m/rock03_m").is_some(), "vanilla builds the _m");
    assert!(roles(&m, "m/rock03_m").is_empty());
}

/// Fuzz-1: with a fuzz map and no skin evidence `auto` builds `_rmaos` and the fuzz map is `pbr_fuzz`; picking RFAOS
/// is skin evidence (Fuzz-5), so the inferred members turn skin, and Undo brings the `_rmaos` back.
#[test]
fn a_fuzz_set_packs_rmaos_and_a_written_rfaos_makes_it_skin() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("m")).unwrap();
    grey(&d.path().join("m"), "rock01_fuzz.png", 60);
    let mut m = fixture(d.path());
    assert!(m.item(RMAOS).is_some(), "auto builds _rmaos");
    assert_eq!(resolved(&m, DIFFUSE), Subtype::PbrAlbedo);
    assert_eq!(resolved(&m, SK), Subtype::BacklightTint);
    assert_eq!(resolved(&m, "m/rock01_fuzz.png"), Subtype::PbrFuzz);
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    assert!(m.item(RFAOS).is_some(), "the written recipe builds _rfaos");
    assert_eq!(resolved(&m, DIFFUSE), Subtype::SkinAlbedo);
    assert_eq!(resolved(&m, SK), Subtype::SkinSk);
    m.undo();
    idle(&mut m);
    assert!(m.item(RMAOS).is_some(), "Undo builds _rmaos again");
}

/// A pre-packed Advanced skin set: `rock01.png`, `rock01_rfaos.png`, `rock01_sk.png`.
fn skin_fixture(dir: &Path) -> AppModel {
    let r = dir.join("m");
    std::fs::create_dir_all(&r).unwrap();
    for name in ["rock01.png", "rock01_rfaos.png"] {
        ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
            .save(r.join(name))
            .unwrap();
    }
    grey(&r, "rock01_sk.png", 0);
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    m
}

fn rock01_entry(m: &AppModel) -> Option<sdc_gui::pgp::writer::EntryInput> {
    json_entries(m)
        .into_iter()
        .find(|(v, _)| v.id == "m/rock01")
        .map(|(_, e)| e)
}

/// AG90 (owner, 2026-10-14, replaces final review C1): an Advanced skin set is not a PGPatcher JSON set.
#[test]
fn an_rfaos_set_is_a_skin_set_with_no_json_entry() {
    let d = tempfile::tempdir().unwrap();
    let m = skin_fixture(d.path());
    assert_eq!(resolved(&m, DIFFUSE), Subtype::SkinAlbedo);
    let v = set_views(&m);
    let v = v.iter().find(|v| v.id == "m/rock01").unwrap();
    assert!(!v.pbr && v.skin, "pbr {} skin {}", v.pbr, v.skin);
    assert!(rock01_entry(&m).is_none());
    // The packed file keeps its `_rfaos` name (never `_rmaos`).
    let out = &m
        .item("m/rock01_rfaos.png")
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .output;
    assert!(
        out.to_string_lossy().ends_with("rock01_rfaos.dds"),
        "{out:?}"
    );
    assert_eq!(*export_blocks(&m), vec![]);
}

/// AG90 (owner, 2026-10-14, replaces final review C1): a skin set has no PGPatcher pitfalls or blocks, even with a
/// leftover JSON value.
#[test]
fn a_skin_set_with_emissive_on_and_no_g_is_not_blocked() {
    let d = tempfile::tempdir().unwrap();
    let mut m = skin_fixture(d.path());
    m.set_json_value("m/rock01", "emissive", Some(json!(true)));
    idle(&mut m);
    let b = export_blocks(&m);
    assert!(b.iter().all(|i| i.set_id != "m/rock01"), "{b:?}");
}

/// AG90: picking RFAOS on the card makes the set an Advanced skin set, which leaves the JSON; the badge says so.
#[test]
fn picking_rfaos_takes_the_set_out_of_the_json() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    assert!(rock01_entry(&m).is_some());
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    assert_eq!(resolved(&m, DIFFUSE), Subtype::SkinAlbedo);
    assert!(rock01_entry(&m).is_none());
    let mut h = editor(m);
    h.run();
    h.run();
    // Layout v2 A5 (K14): the card's pill says "skin" (its hover the long "Advanced skin (no JSON)").
    h.get_by_label("skin");
}

/// Owner 2026-10-27: making the diffuse Advanced skin switches the set's packed `_rmaos` to `_rfaos`, as one undo step.
#[test]
fn a_diffuse_made_skin_albedo_switches_the_packed_rmaos_to_rfaos() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    assert!(m.item(RMAOS).is_some());
    let before = texts(&m);
    m.set_field(DIFFUSE, "subtype", json!(Subtype::SkinAlbedo));
    idle(&mut m);
    assert_eq!(resolved(&m, DIFFUSE), Subtype::SkinAlbedo);
    assert!(m.item(RFAOS).is_some(), "the packed output is _rfaos");
    assert!(m.item(RMAOS).is_none());
    let p = m.project.as_ref().unwrap();
    assert_eq!(p.file_value(RFAOS, "packing.recipe"), Some(json!("rfaos")));
    m.undo();
    idle(&mut m);
    assert_eq!(texts(&m), before, "one undo step");
    assert!(m.item(RMAOS).is_some());
}

/// Owner 2026-10-27: taking the diffuse off Advanced skin switches the packed `_rfaos` back to `_rmaos`.
#[test]
fn a_diffuse_taken_off_skin_albedo_switches_the_packed_rfaos_back() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    assert_eq!(resolved(&m, DIFFUSE), Subtype::SkinAlbedo);
    assert!(m.item(RFAOS).is_some());
    m.set_field(DIFFUSE, "subtype", json!(Subtype::PbrAlbedo));
    idle(&mut m);
    assert_eq!(resolved(&m, DIFFUSE), Subtype::PbrAlbedo);
    assert!(m.item(RMAOS).is_some(), "the packed output is _rmaos again");
    assert!(m.item(RFAOS).is_none());
    let p = m.project.as_ref().unwrap();
    assert_eq!(p.file_value(RMAOS, "packing.recipe"), Some(json!("rmaos")));
    assert_eq!(p.file_value(RFAOS, "packing.recipe"), None);
}

/// The same edit on a set that cannot build `_rfaos` (metallic only) still sets the sub-type and keeps `_rmaos`.
#[test]
fn a_skin_albedo_diffuse_keeps_rmaos_when_rfaos_cannot_be_built() {
    let d = tempfile::tempdir().unwrap();
    let mut m = metallic_only(d.path());
    m.set_field(DIFFUSE, "subtype", json!(Subtype::SkinAlbedo));
    idle(&mut m);
    assert_eq!(subtype(&m, DIFFUSE), Some(json!(Subtype::SkinAlbedo)));
    assert!(m.item(RMAOS).is_some());
}

/// A metallic-only cs set: `_rmaos` from a metallic map, which `_rfaos` (no metallic channel) cannot be built from.
fn metallic_only(dir: &Path) -> AppModel {
    metallic_only_with(dir, "")
}

/// [`metallic_only`] with more `sdc.toml` text.
fn metallic_only_with(dir: &Path, extra: &str) -> AppModel {
    let r = dir.join("m");
    std::fs::create_dir_all(&r).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(r.join("rock01.png"))
        .unwrap();
    grey(&r, "rock01_metallic.png", 30);
    std::fs::write(
        dir.join("sdc.toml"),
        format!("[files.\"m/rock01_rmaos\".packing.fill]\nao = 99\n{extra}"),
    )
    .unwrap();
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    m
}

const RFAOS_WHY: &str = "RFAOS needs a roughness, glossiness or AO source";

/// Final review I2: RFAOS cannot be built from a metallic-only set, so the model refuses it with the reason and
/// moves nothing (the fills stay on the planned `_rmaos` key).
#[test]
fn rfaos_on_a_metallic_only_set_is_refused_with_the_reason() {
    let d = tempfile::tempdir().unwrap();
    let mut m = metallic_only(d.path());
    assert!(m.item(RMAOS).is_some());
    assert_eq!(m.rfaos_unbuildable(RMAOS), Some(RFAOS_WHY));
    let before = texts(&m);
    assert_eq!(
        m.set_packing_recipe(RMAOS, PackRecipe::Rfaos),
        Some(RFAOS_WHY.to_string())
    );
    idle(&mut m);
    assert_eq!(texts(&m), before, "nothing written");
    assert!(m.item(RMAOS).is_some());
    assert!(m.unmatched_keys.is_empty(), "{:?}", m.unmatched_keys);
    assert!(m.dropped_fills.is_empty());
    // A set with a roughness source can build it.
    let d = tempfile::tempdir().unwrap();
    let m = fixture(d.path());
    assert_eq!(m.rfaos_unbuildable(RMAOS), None);
}

/// A1-5, A1-6 (was `the_rfaos_option_is_disabled_on_a_metallic_only_set`): Note DQ2's case says why on the title.
#[test]
fn a_metallic_only_skin_set_says_why_it_keeps_rmaos() {
    use sdc_gui::ui::set_editor::KEEPS_RMAOS;
    let d = tempfile::tempdir().unwrap();
    let mut m = metallic_only(d.path());
    let id = set_id(&m);
    m.set_shader(&id, Some(Shader::AdvancedSkin));
    idle(&mut m);
    assert!(m.item(RMAOS).is_some(), "DQ2: still _rmaos");
    let mut h = editor(m);
    settle(&mut h);
    let title = h
        .query_all_by_label("RMAOS")
        .find(|n| !support::in_sets_column(&h, n.rect()))
        .expect("the packed row's slot title");
    assert_eq!(
        title.accesskit_node().description().as_deref(),
        Some(KEEPS_RMAOS)
    );
    title.hover();
    h.run();
    h.run();
    h.get_by_label(KEEPS_RMAOS);
    // A set that can build RFAOS: Advanced Skin packs it; its slot title reads RFAOS and does not say the sentence.
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    let id = set_id(&m);
    m.set_shader(&id, Some(Shader::AdvancedSkin));
    idle(&mut m);
    let mut h = editor(m);
    settle(&mut h);
    let title = h
        .query_all_by_label("RFAOS")
        .find(|n| !support::in_sets_column(&h, n.rect()))
        .expect("the packed row's slot title reads RFAOS");
    assert_ne!(
        title.accesskit_node().description().as_deref(),
        Some(KEEPS_RMAOS)
    );
    assert!(h.query_by_label(KEEPS_RMAOS).is_none());
}

/// A1-4, A1-6 (was `picking_rmaos_clears_a_stale_rfaos`): a hand-written `rfaos` the set cannot build is cleared by
/// the row's fix.
#[test]
fn an_unbuildable_recipe_row_clears_the_recipe() {
    let d = tempfile::tempdir().unwrap();
    let m = metallic_only_with(
        d.path(),
        "\n[files.\"m/rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
    );
    assert!(m.item(RMAOS).is_some(), "unbuildable: still _rmaos");
    let row = |m: &AppModel| {
        problems(m)
            .iter()
            .find(|p| p.code == Some(Code::PackRecipeUnbuildable))
            .cloned()
    };
    let r = row(&m).expect("the UNBUILDABLE row");
    assert_eq!(r.at, At::Texture(RMAOS.into()));
    assert_eq!(r.fix, Some(Fix::ClearRecipe(RMAOS.into())));
    let mut m = m;
    m.open_problems(None);
    let mut h = app(m);
    h.run();
    h.get_by_label("Clear the packing recipe of rock01_rmaos.dds")
        .click();
    h.run();
    idle(h.state_mut());
    h.run();
    let m = h.state();
    let p = m.project.as_ref().unwrap();
    assert_eq!(p.file_value(RFAOS, "packing.recipe"), None);
    assert_eq!(p.file_value(RMAOS, "packing.recipe"), None);
    assert_eq!(
        p.file_value(RMAOS, "packing.fill.ao"),
        Some(json!(99)),
        "the fill stays"
    );
    assert!(row(m).is_none());
    assert!(m.item(RMAOS).is_some());
}

/// A1-4: the two packed entries disagree; the fix on the CONFLICT row clears both and the maps decide (`_rmaos`:
/// roughness and metallic, no fuzz).
#[test]
fn a_recipe_conflict_row_clears_both_entries() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("m")).unwrap();
    grey(&d.path().join("m"), "rock01_metallic.png", 30);
    let mut m = fixture_with(
        d.path(),
        "[files.\"m/rock01_rmaos\".packing]\nrecipe = \"rmaos\"\n\n[files.\"m/rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
        false,
    );
    assert!(m.item(RFAOS).is_some(), "the _rfaos entry wins");
    let r = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::PackRecipeConflict))
        .cloned()
        .expect("the CONFLICT row");
    assert_eq!(r.fix, Some(Fix::ClearRecipe(RFAOS.into())));
    m.open_problems(None);
    let mut h = app(m);
    h.run();
    h.get_by_label("Clear the packing recipe of rock01_rfaos.dds")
        .click();
    h.run();
    idle(h.state_mut());
    h.run();
    let m = h.state();
    assert_eq!(m.explicit_recipe(RMAOS), PackRecipe::Auto);
    assert!(m.item(RMAOS).is_some() && m.item(RFAOS).is_none());
    assert!(!problems(m)
        .iter()
        .any(|p| p.code == Some(Code::PackRecipeConflict)));
    // A1-8: the emptied `packing` tables and `[files."…"]` entries are pruned, not left empty.
    let text = m.project.as_ref().unwrap().text();
    assert!(
        !text.contains("rock01_rfaos") && !text.contains("rock01_rmaos"),
        "{text}"
    );
    assert!(
        !problems(m).iter().any(|p| p
            .detail
            .as_deref()
            .is_some_and(|d| d.contains("rock01_rfaos"))),
        "no CFG-FILE-UNMATCHED row for an empty entry"
    );
}

/// A1-8: fills stay. A fill on the losing `_rfaos` entry outlives Clear recipe, and the entry, now without a file,
/// keeps its own CFG-FILE-UNMATCHED row.
#[test]
fn clear_recipe_keeps_the_fills_with_their_unmatched_row() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("m")).unwrap();
    grey(&d.path().join("m"), "rock01_metallic.png", 30);
    let mut m = fixture_with(
        d.path(),
        "[files.\"m/rock01_rmaos\".packing]\nrecipe = \"rmaos\"\n\n[files.\"m/rock01_rfaos\".packing]\nrecipe = \"rfaos\"\nfill.ao = 99\n",
        false,
    );
    m.clear_packing_recipe(RFAOS);
    idle(&mut m);
    let p = m.project.as_ref().unwrap();
    assert_eq!(p.file_value(RFAOS, "packing.recipe"), None);
    assert_eq!(
        p.file_value(RFAOS, "packing.fill.ao"),
        Some(json!(99)),
        "the fill stays"
    );
    assert!(m.item(RMAOS).is_some() && m.item(RFAOS).is_none());
    // `problems` returns `Arc<Vec<Problem>>`: find by reference, then clone.
    let row = problems(&m)
        .iter()
        .find(|r| r.code == Some(Code::CfgFileUnmatched))
        .cloned()
        .expect("the CFG-FILE-UNMATCHED row");
    assert!(
        row.detail
            .as_deref()
            .is_some_and(|d| d.split('\n').any(|k| k == RFAOS)),
        "{:?}",
        row.detail
    );
}

/// A1-8, A1-9: on a read-only `sdc.toml` Clear recipe is disabled and its hover gives the read-only reason, as the
/// fills'. Every assert here can fail: the button's state and its hover.
#[test]
fn a_read_only_sdc_toml_disables_clear_recipe_and_says_why() {
    let d = tempfile::tempdir().unwrap();
    let mut m = metallic_only_with(
        d.path(),
        "\n[files.\"m/rock01_rfaos\".packing]\nrecipe = \"rfaos\"\n",
    );
    // The row is built while the entry is there (a read-only doc holds none, review F1); the doc then turns
    // read-only under it, as `problems_rows::a_read_only_project_offers_no_removal` does, and the button's draw
    // reads the state.
    assert!(problems(&m)
        .iter()
        .any(|p| p.fix == Some(Fix::ClearRecipe(RMAOS.into()))));
    m.project
        .as_mut()
        .unwrap()
        .set_error("rejected by the engine");
    let reason = m.toml_read_only_reason().expect("read-only");
    m.open_problems(None);
    let mut h = app(m);
    // As `problems_rows::no_tooltip_delay`: a disabled button's tooltip waits on egui's delay otherwise.
    h.ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    h.run();
    let button = h.get_by_label("Clear the packing recipe of rock01_rmaos.dds");
    assert!(button.accesskit_node().is_disabled());
    h.hover_at(button.rect().center());
    h.run_steps(4);
    h.get_by_label(&reason);
}

/// A1-4: the sentences no longer send the user to a control that is gone.
#[test]
fn the_recipe_rows_name_the_clear_fix() {
    use sdc_gui::model::code_text::code_sentence;
    assert_eq!(
        code_sentence(Code::PackRecipeConflict),
        "The two packed entries name different recipes — clear the recipe"
    );
    assert_eq!(
        code_sentence(Code::PackRecipeUnbuildable),
        "The written recipe can't be built from this set's maps — clear the recipe"
    );
}

/// A1-6 (was `re_picking_the_auto_recipe_writes_nothing`): an `auto` set drawn for a while writes nothing.
#[test]
fn drawing_an_auto_set_writes_no_recipe() {
    let d = tempfile::tempdir().unwrap();
    let m = fixture(d.path());
    let before = texts(&m);
    let mut h = editor(m);
    settle(&mut h);
    h.run();
    assert_eq!(texts(h.state()), before);
    assert_eq!(h.state().explicit_recipe(RMAOS), PackRecipe::Auto);
}

/// A1-7: the SHADER-EXTRA-MAP row that names the packed output `out` (`At::Set`, its sentence starts with `out`)
/// offers Clear recipe first and Change shader… second; pressing Clear recipe removes the recipe from both packed
/// keys, the set packs `after`, and the row goes.
fn extra_row_clears(mut m: AppModel, id: &str, packed: &str, out: &str, after: &str) {
    let row = |m: &AppModel| {
        problems(m)
            .iter()
            .find(|p| p.code == Some(Code::ShaderExtraMap) && p.text.starts_with(out))
            .cloned()
    };
    let r = row(&m).unwrap_or_else(|| panic!("the SHADER-EXTRA-MAP row for {out}"));
    assert!(
        matches!(&r.at, At::Set { id: i, .. } if i == id),
        "{:?}",
        r.at
    ); // `At::Set` is `{ key, id }`
    assert_eq!(
        r.fix,
        Some(Fix::ClearRecipe(packed.into())),
        "Clear recipe is the first fix"
    );
    assert_eq!(
        r.second_fix,
        Some(Fix::ChangeShader(id.into())),
        "Change shader… moves to second"
    );
    m.open_problems(None);
    let mut h = app(m);
    h.run();
    h.get_by_label(&format!("Clear the packing recipe of {out}"))
        .click();
    h.run();
    idle(h.state_mut());
    h.run();
    let m = h.state();
    let p = m.project.as_ref().unwrap();
    assert_eq!(p.file_value(RMAOS, "packing.recipe"), None);
    assert_eq!(p.file_value(RFAOS, "packing.recipe"), None);
    assert!(m.item(after).is_some(), "the shader decides: {after}");
    assert!(m.item(packed).is_none());
    assert!(row(m).is_none(), "the row goes");
}

/// A1-7 (probe P1c): a written `rfaos` under an explicit PBR shader, and the mirror, a written `rmaos` under an
/// explicit Advanced Skin (DV3). Each keeps its written recipe and gets Clear recipe on the SHADER-EXTRA-MAP row.
#[test]
fn the_extra_packed_map_row_offers_clear_recipe_first() {
    // P1c: roughness only, `rfaos` written, then PBR picked.
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    let id = set_id(&m);
    m.set_shader(&id, Some(Shader::Pbr));
    idle(&mut m);
    assert!(
        m.item(RFAOS).is_some(),
        "P1c: the written rfaos keeps _rfaos under PBR"
    );
    extra_row_clears(m, &id, RFAOS, "rock01_rfaos.dds", RMAOS);
    // The mirror, written to disk before opening: picking Advanced Skin in the app would rewrite `rmaos` to `rfaos`
    // (DU6 → DQ1), so the shader and the recipe are both on disk first.
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("sdc-project.toml"),
        "[[sets]]\nid = \"m/rock01\"\nshader = \"advanced_skin\"\n",
    )
    .unwrap();
    let m = fixture_with(
        d.path(),
        "[files.\"m/rock01_rmaos\".packing]\nrecipe = \"rmaos\"\n",
        false,
    );
    let id = set_id(&m);
    let v = views(&m).iter().find(|v| v.id == id).cloned().unwrap(); // `views` returns `Arc<Vec<SetView>>`
    assert_eq!(
        v.shader.as_ref().map(|p| p.shader),
        Some(Shader::AdvancedSkin),
        "fixture: the shader is read"
    );
    assert!(
        m.item(RMAOS).is_some(),
        "DV3: the written rmaos keeps _rmaos under Advanced Skin"
    );
    extra_row_clears(m, &id, RMAOS, "rock01_rmaos.dds", RFAOS);
}

/// Task 5a review F1: Clear recipe clears only `[files]` entries, so a row offers it only where one holds a recipe.
/// A recipe that comes from the project level has no button: it would change nothing.
#[test]
fn a_recipe_from_the_project_level_gets_no_clear_recipe() {
    let d = tempfile::tempdir().unwrap();
    let m = metallic_only_with(d.path(), "");
    // The project-level `packing.recipe`, set in the document the way a hand edit would.
    let text = std::fs::read_to_string(d.path().join("sdc.toml")).unwrap();
    std::fs::write(
        d.path().join("sdc.toml"),
        format!("[defaults.packing]\nrecipe = \"rfaos\"\n\n{text}"),
    )
    .unwrap();
    drop(m);
    let mut m = AppModel::new(
        GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..GuiSettings::default()
        },
        None,
        session(),
    );
    m.accept = engine::policy::AcceptSuggestions::None;
    m.open_folder(d.path().to_path_buf());
    idle(&mut m);
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(RFAOS, "packing.recipe"),
        None,
        "fixture: nothing on the entries"
    );
    let r = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::PackRecipeUnbuildable))
        .cloned()
        .expect("premise: the project-level rfaos makes an UNBUILDABLE row");
    assert_eq!(r.fix, None, "nothing on the entries to clear");
}

/// Task 5a review F1 (Minor 3): a hand-written `recipe = "auto"` is not a written recipe, so the SHADER-EXTRA-MAP
/// row does not offer Clear recipe first.
#[test]
fn a_written_auto_recipe_gets_no_clear_recipe() {
    let d = tempfile::tempdir().unwrap();
    let mut m = metallic_only_with(
        d.path(),
        "\n[files.\"m/rock01_rmaos\".packing]\nrecipe = \"auto\"\n",
    );
    let id = set_id(&m);
    m.set_shader(&id, Some(Shader::AdvancedSkin));
    idle(&mut m);
    assert!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(RMAOS, "packing.recipe")
            .is_some(),
        "fixture: the auto is on the entry"
    );
    let r = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::ShaderExtraMap) && p.text.starts_with("rock01_rmaos.dds"))
        .cloned()
        .expect("premise: the extra-map row");
    assert_eq!(r.fix, Some(Fix::ChangeShader(id)), "Change shader… alone");
}

/// Controller ruling (2026-11-18): with no recipe written, the SHADER-EXTRA-MAP row that names the set's packed
/// output (DQ2's metallic-only Advanced Skin set: `_rmaos`, nothing written) has exactly one fix, Change shader….
/// Remove from set is not offered: a packed output cannot leave its set, so that button could never work. Go to set
/// still outlines the packed row. A non-packed extra member keeps 3a's two fixes
/// (`problems_notes::each_row_carries_the_tables_fix`).
#[test]
fn the_extra_packed_map_row_without_a_recipe_offers_change_shader_alone() {
    use sdc_gui::model::nav::SlotTarget;
    let d = tempfile::tempdir().unwrap();
    let mut m = metallic_only(d.path());
    let id = set_id(&m);
    m.set_shader(&id, Some(Shader::AdvancedSkin));
    idle(&mut m);
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(RMAOS, "packing.recipe"),
        None,
        "fixture: nothing written"
    );
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(RFAOS, "packing.recipe"),
        None,
        "fixture: nothing written"
    );
    let r = problems(&m)
        .iter()
        .find(|p| p.code == Some(Code::ShaderExtraMap) && p.text.starts_with("rock01_rmaos.dds"))
        .cloned()
        .expect("DQ2's SHADER-EXTRA-MAP row");
    assert_eq!(
        r.text,
        "rock01_rmaos.dds isn't read by the advanced_skin shader. It is still converted, so nothing breaks."
    );
    assert_eq!(
        r.fix,
        Some(Fix::ChangeShader(id.clone())),
        "Change shader… is the one fix"
    );
    assert_eq!(r.second_fix, None, "no Remove from set for a packed output");
    assert_eq!(
        r.target,
        Some(SlotTarget::Item(RMAOS.into())),
        "Go to set still outlines the packed row"
    );
}

#[test]
fn dragging_a_fill_box_and_releasing_stores_the_final_dragged_value() {
    let d = tempfile::tempdir().unwrap();
    let mut h = editor(fixture(d.path()));
    h.run();
    let start = fill_rows(h.state(), RMAOS)
        .into_iter()
        .find_map(|r| match r {
            FillRow::Value {
                role: RawMap::Ao,
                shown,
                ..
            } => Some(shown),
            _ => None,
        })
        .expect("an AO value row");
    // Note DY18: the source picker widens the row, so the box may sit past the column's edge: scroll to it.
    h.get_by_label("AO").scroll_to_me();
    h.run();
    let label = h.get_by_label("AO").accesskit_node().raw_bounds().unwrap();
    let spin = h
        .query_all_by_role(eframe::egui::accesskit::Role::SpinButton)
        .find(|n| (n.accesskit_node().raw_bounds().unwrap().y0 - label.y0).abs() < 6.0)
        .expect("the AO value box");
    spin.scroll_to_me();
    h.run();
    let label = h.get_by_label("AO").accesskit_node().raw_bounds().unwrap();
    let r#box = h
        .query_all_by_role(eframe::egui::accesskit::Role::SpinButton)
        .find(|n| (n.accesskit_node().raw_bounds().unwrap().y0 - label.y0).abs() < 6.0)
        .expect("the AO value box")
        .rect();
    // DragValue speed 1.0: one step per pixel. Drag 40 px left in 10 px moves, then release.
    let p = r#box.center();
    h.hover_at(p);
    h.drag_at(p);
    h.step();
    for i in 1..=4 {
        h.hover_at(p - eframe::egui::vec2(10.0 * i as f32, 0.0));
        h.step();
    }
    h.drop_at(p - eframe::egui::vec2(40.0, 0.0));
    settle(&mut h);
    let want = start.saturating_sub(40);
    assert_ne!(want, start, "the fixture's AO must be able to move down");
    assert_eq!(
        h.state()
            .project
            .as_ref()
            .unwrap()
            .file_value(RMAOS, "packing.fill.ao"),
        Some(json!(want)),
        "the released drag's final value is the fill"
    );
}

/// Note DR: with a `_f` in the set, making the diffuse skin_albedo (Note DQ) fills the RFAOS card's fuzz row from it.
#[test]
fn a_skin_f_is_the_rfaos_cards_fuzz_file() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("m")).unwrap();
    grey(&d.path().join("m"), "rock01_f.png", 200);
    let mut m = fixture(d.path());
    assert!(
        fill_rows(&m, RMAOS).iter().all(|r| !matches!(
            r,
            FillRow::FromFile {
                role: RawMap::Fuzz,
                ..
            }
        )),
        "_f is not a fuzz source before the set is skin"
    );
    m.set_field(DIFFUSE, "subtype", json!(Subtype::SkinAlbedo));
    idle(&mut m);
    assert!(m.item(RFAOS).is_some(), "the packed output is _rfaos");
    let rows = fill_rows(&m, RFAOS);
    assert!(
        rows.iter().any(|r| matches!(r, FillRow::FromFile { role: RawMap::Fuzz, file, .. } if file == "rock01_f.png")),
        "{rows:?}"
    );
}

/// Note FC3 (R-fills-2): a fill a recipe switch dropped is an Info row with `Undo` while the switch is the last step;
/// it leaves "ready" alone, and Undo takes the switch back.
#[test]
fn a_dropped_fill_is_an_info_row_with_undo() {
    use egui_kittest::kittest::Queryable;
    use sdc_gui::model::bottom::{counts_as_ready_problem, Level};
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_pack_fill(RMAOS, RawMap::Metallic, Some(9));
    idle(&mut m);
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(&mut m);
    // The fixture's PACK-DEFAULTED notes are Info rows too (review E2); the dropped fill's row has no code.
    let row = |m: &AppModel| {
        problems(m)
            .iter()
            .find(|p| p.level == Level::Info && p.code.is_none())
            .cloned()
    };
    let r = row(&m).expect("the Info row");
    assert_eq!(r.at, At::Texture(RFAOS.into()));
    assert_eq!(
        r.text,
        "metallic fill removed (RFAOS has no metallic channel)"
    );
    assert_eq!(r.fix, Some(Fix::UndoLast));
    assert!(!counts_as_ready_problem(&r));
    m.open_problems(None);
    let mut h = app(m);
    h.run();
    h.get_by_label(sdc_gui::ui::problems::UNDO_ROW).click();
    h.run();
    idle(h.state_mut());
    h.run();
    let m = h.state();
    assert!(row(m).is_none());
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(RFAOS, "packing.recipe"),
        None
    );
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(RMAOS, "packing.fill.metallic"),
        Some(json!(9))
    );
    // Another step on top: the row stays, without Undo.
    let m = h.state_mut();
    m.set_packing_recipe(RMAOS, PackRecipe::Rfaos);
    idle(m);
    assert_eq!(row(m).expect("listed again").fix, Some(Fix::UndoLast));
    m.set_json_name("other");
    assert_eq!(row(m).expect("still listed").fix, None);
}

/// Backlog R2: after Enter the Fills box keeps showing the typed value until the re-plan lands (it reverted to the
/// old value for those frames).
#[test]
fn a_fill_box_keeps_the_typed_value_until_the_plan_lands() {
    use eframe::egui::{Event, Key, Modifiers};
    let d = tempfile::tempdir().unwrap();
    let mut h = editor(fixture(d.path()));
    h.run();
    fn ao<'a>(h: &'a Harness<'static, AppModel>) -> egui_kittest::Node<'a> {
        h.get_by_role_and_label(eframe::egui::accesskit::Role::SpinButton, "AO fill")
    }
    ao(&h).scroll_to_me();
    h.run();
    ao(&h).focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    for c in ["2", "0", "0"] {
        h.event(Event::Text(c.into()));
        h.run();
    }
    h.key_press(Key::Enter);
    for _ in 0..3 {
        h.run_steps(1);
        assert!(h.state().edit_busy(), "the re-plan is still out");
        assert_eq!(ao(&h).accesskit_node().numeric_value(), Some(200.0));
    }
    settle(&mut h);
    assert_eq!(ao(&h).accesskit_node().numeric_value(), Some(200.0));
}

/// Backlog R2 review: the held value belongs to its box. Switching to another set before the plan lands must not
/// show (or later write) the first set's typed value in the second set's AO box.
#[test]
fn a_held_fill_value_does_not_follow_to_another_set() {
    use eframe::egui::{Event, Key, Modifiers};
    let d = tempfile::tempdir().unwrap();
    let mut h = editor(fixture_with(d.path(), "", true));
    h.run();
    fn ao<'a>(h: &'a Harness<'static, AppModel>) -> egui_kittest::Node<'a> {
        h.get_by_role_and_label(eframe::egui::accesskit::Role::SpinButton, "AO fill")
    }
    ao(&h).scroll_to_me();
    h.run();
    ao(&h).focus();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    for c in ["2", "0", "0"] {
        h.event(Event::Text(c.into()));
        h.run();
    }
    h.key_press(Key::Enter);
    h.run_steps(2);
    assert!(h.state().edit_busy(), "the re-plan is still out");
    let other = h
        .state()
        .sets
        .iter()
        .find(|s| s.key.ends_with("m/rock02"))
        .expect("the rock02 set")
        .key
        .clone();
    h.state_mut().select_set(&other);
    h.run_steps(3);
    assert!(h.state().edit_busy(), "still out after the switch");
    assert_eq!(ao(&h).accesskit_node().numeric_value(), Some(255.0));
}
