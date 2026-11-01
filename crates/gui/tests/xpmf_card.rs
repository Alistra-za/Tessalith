//! Note GI (XPMF Task 7, design X1, X3, X4, X7, X9): the set editor's XPMF card and the PBRMaterialObjects Problems
//! row.
mod support;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use engine::shader::Shader;
use sdc_gui::model::bottom::{problems, At};
use sdc_gui::model::cs_pbr::Kind;
use sdc_gui::model::sets::set_views;
use sdc_gui::model::AppModel;
use sdc_gui::ui::problems::status_of;
use sdc_gui::ui::status::Status;
use serde_json::{json, Map, Value as Json};
use std::time::Duration;

const ID: &str = "landscape/rock01";
const TOGGLE: &str = "Write XPMF profile";
const NOT_PREVIEWED: &str = "Not shown in the preview";
const X4_HOVER: &str = "Only this project's cs_mato blocks are checked; PBRMaterialObjects files from other mods are not scanned.";

type H = Harness<'static, AppModel>;

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "the model did not settle"
    );
}

/// `support::pbr_opened` (rock01 auto PBR Landscape on CS) with a `cs_mato` block `[MatoA]`, rock01 selected.
fn opened(dir: &std::path::Path) -> AppModel {
    let mut m = support::pbr_opened(dir);
    m.add_cs_block(ID, Kind::Mato);
    m.set_cs_edids(ID, Kind::Mato, &["MatoA".into()]);
    idle(&mut m);
    let key = set_views(&m).into_iter().find(|v| v.id == ID).unwrap().key;
    m.select_set(&key);
    m
}

fn harness(m: AppModel, w: f32, h: f32) -> H {
    let h = Harness::builder()
        .with_size(egui::vec2(w, h))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

fn tall(m: AppModel) -> H {
    harness(m, 1400.0, 2400.0)
}

fn block(m: &AppModel) -> Option<Map<String, Json>> {
    m.sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|e| e.id == ID)
        .and_then(|e| e.xpmf.clone())
}

fn set_block(m: &mut AppModel, b: Json) {
    let b = b.as_object().unwrap().clone();
    m.sets_doc.as_mut().unwrap().set_xpmf(ID, Some(b));
}

fn has(h: &H, s: &str) -> bool {
    h.query_all_by_label(s).next().is_some()
}

/// The rect of the row label `label` (the card's row labels are unique on the page).
fn row_rect(h: &H, label: &str) -> egui::Rect {
    h.query_all_by_label(label)
        .map(|n| n.rect())
        .next()
        .unwrap_or_else(|| panic!("no row {label:?}"))
}

#[test]
fn card_only_on_landscape_sets() {
    let d = tempfile::tempdir().unwrap();
    let mut h = tall(opened(d.path()));
    h.run_steps(4);
    assert!(has(&h, TOGGLE), "PBR Landscape shows the card");
    h.state_mut().set_shader(ID, Some(Shader::Pbr));
    idle(h.state_mut());
    h.run_steps(4);
    assert!(!has(&h, TOGGLE), "a plain PBR set has no card");
    h.state_mut().set_shader(ID, Some(Shader::Landscape));
    idle(h.state_mut());
    h.run_steps(4);
    assert!(has(&h, TOGGLE), "Landscape shows the card");
}

#[test]
fn toggle_creates_and_removes_block() {
    let d = tempfile::tempdir().unwrap();
    let mut h = tall(opened(d.path()));
    h.run_steps(4);
    assert_eq!(block(h.state()), None);
    h.get_by_label(TOGGLE).click();
    h.run_steps(4);
    assert_eq!(block(h.state()), Some(Map::new()), "an empty block");
    h.get_by_label(TOGGLE).click();
    h.run_steps(4);
    assert_eq!(block(h.state()), None, "the block is removed");
}

/// X9: pbr is on by default for a PBR Landscape set on CS, so `specularMult` is hidden; pbr off shows it.
#[test]
fn specular_hidden_when_pbr() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, json!({}));
    let mut h = tall(m);
    h.run_steps(4);
    assert!(has(&h, "Shelter fade"), "the rows are drawn");
    assert!(!has(&h, "Specular mult"), "hidden while pbr is on");
    set_block(h.state_mut(), json!({"pbr": false}));
    h.run_steps(4);
    assert!(has(&h, "Specular mult"), "shown while pbr is off");
}

/// X7: typed values clamp to the schema range: 500 in `shelterFade` (0..=128) stores 128.
#[test]
fn out_of_range_number_clamps() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, json!({"shelterFade": 40}));
    let mut h = tall(m);
    h.run_steps(4);
    h.get_by_label("Shelter fade").scroll_to_me();
    h.run_steps(4);
    let row = row_rect(&h, "Shelter fade");
    let spin = h
        .query_all_by_role(Role::SpinButton)
        .find(|n| {
            n.rect().center().y >= row.top() - 4.0 && n.rect().center().y <= row.bottom() + 4.0
        })
        .expect("the shelterFade value");
    // As `also_export.rs` edits a DragValue: focus puts it in text mode (its role stays SpinButton), select all, type.
    spin.focus();
    h.run_steps(4);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(4);
    h.event(egui::Event::Text("500".into()));
    h.run_steps(4);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    assert_eq!(
        block(h.state())
            .unwrap()
            .get("shelterFade")
            .and_then(Json::as_f64),
        Some(128.0)
    );
}

fn x4_rows(m: &AppModel) -> Vec<sdc_gui::model::bottom::Problem> {
    problems(m)
        .iter()
        .filter(|p| p.text.starts_with("XPMF skips"))
        .cloned()
        .collect()
}

/// X4: with pbr on, an exact EDID no cs_mato block of the project names is one amber row; wildcards and the set's own
/// cs_mato EDIDs are not. A cs_mato block for it clears the row; pbr off has none.
#[test]
fn x4_problem_row() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(
        &mut m,
        json!({"editorIds": ["MatoA", "NoSuchMato", "*snow*", "Ash?Mat"]}),
    );
    let rows = x4_rows(&m);
    assert_eq!(rows.len(), 1, "{rows:?}");
    let r = &rows[0];
    assert_eq!(
        r.text,
        "XPMF skips NoSuchMato: no PBRMaterialObjects JSON from this project"
    );
    assert_eq!(status_of(r.level), Status::Warning, "amber");
    assert_eq!(r.detail.as_deref(), Some(X4_HOVER));
    assert!(
        matches!(&r.at, At::Set { id, .. } if id == ID),
        "{:?}",
        r.at
    );
    // Drawn in the Problems tab.
    m.open_problems(None);
    let mut h = harness(m, 1400.0, 900.0);
    h.run_steps(4);
    let drawn: Vec<_> = h
        .query_all_by_label_contains("XPMF skips NoSuchMato")
        .filter(|n| n.accesskit_node().role() == Role::Label)
        .map(|n| format!("{:?}", n.rect()))
        .collect();
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    // Case does not matter (Windows file names), as in `refusals`.
    h.state_mut()
        .set_cs_edids(ID, Kind::Mato, &["MatoA".into(), "nosuchmato".into()]);
    h.run_steps(4);
    assert!(x4_rows(h.state()).is_empty(), "{:?}", x4_rows(h.state()));
    assert_eq!(h.query_all_by_label_contains("XPMF skips").count(), 0);
    h.state_mut()
        .set_cs_edids(ID, Kind::Mato, &["MatoA".into()]);
    set_block(
        h.state_mut(),
        json!({"pbr": false, "editorIds": ["NoSuchMato"]}),
    );
    assert!(x4_rows(h.state()).is_empty(), "pbr off: no row");
}

/// X1: a row the preview cannot show says so on hover; a previewed row does not.
#[test]
fn non_preview_rows_have_hover() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, json!({}));
    let mut h = tall(m);
    h.ctx.global_style_mut(|st| {
        st.interaction.tooltip_delay = 0.0;
        st.interaction.show_tooltips_only_when_still = false;
    });
    h.run_steps(4);
    assert!(!has(&h, NOT_PREVIEWED));
    h.hover_at(row_rect(&h, "Roof shelter").center());
    h.run_steps(4);
    assert!(has(&h, NOT_PREVIEWED), "the roofShelter hover");
    h.hover_at(row_rect(&h, "Falloff scale").center());
    h.run_steps(4);
    assert!(has(&h, "JSON key: falloffScale"), "the falloffScale hover");
    assert!(!has(&h, NOT_PREVIEWED), "falloffScale is previewed");
}

/// The greyed defaults of the keys Tessalith fills are the profile's: the set's cs_mato EDIDs and its id.
#[test]
fn defaults_come_from_the_profile() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, json!({}));
    let mut h = tall(m);
    h.run_steps(4);
    assert!(has(&h, "default: MatoA"), "editorIds' default");
    assert!(
        h.query_all_by_role(Role::TextInput)
            .any(|n| n.accesskit_node().placeholder() == Some(ID)),
        "name's default is the set id"
    );
}

/// ui-rules §9.1: the card's values are built once per set-view key, and again after a block edit.
#[test]
fn card_values_are_cached_by_revision() {
    use sdc_gui::model::xpmf_json::card_values;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, json!({}));
    let a = card_values(&m, ID);
    assert!(std::sync::Arc::ptr_eq(&a, &card_values(&m, ID)), "cached");
    set_block(&mut m, json!({"name": "snow"}));
    let b = card_values(&m, ID);
    assert!(!std::sync::Arc::ptr_eq(&a, &b), "a block edit rebuilds it");
    assert_eq!(b.profile.as_ref().unwrap()["name"], "snow");
    assert_eq!(b.defaults.as_ref().unwrap()["name"], ID);
}

/// Task 7 Step 4: the card at 1100, 1280 and 1600 px wide, for checking against egui-ui-rules groups 2 and 3. Needs
/// a GPU; writes `xpmf-card-<w>-{top,bottom}.png` into `XPMF_SHOTS`: `cargo test -p sdc-gui --test xpmf_card -- --ignored`.
#[test]
#[ignore]
fn xpmf_card_screenshots() {
    let Ok(out) = std::env::var("XPMF_SHOTS") else {
        panic!("set XPMF_SHOTS to the folder the PNGs go to");
    };
    for (w, ht) in [(1100.0, 800.0), (1280.0, 800.0), (1600.0, 900.0)] {
        let d = tempfile::tempdir().unwrap();
        let mut m = opened(d.path());
        set_block(
            &mut m,
            json!({"editorIds": ["MatoA", "NoSuchMato"], "shelterFade": 40, "comment": "a long comment to see how the row wraps"}),
        );
        let mut h = Harness::builder()
            .with_size(egui::vec2(w, ht))
            .with_pixels_per_point(1.0)
            .wgpu()
            .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run_steps(6);
        // The card's top (toggle, name, patterns) and its bottom rows.
        for (part, label) in [("top", "Editor IDs"), ("bottom", "Shelter fade")] {
            h.get_by_label(label).scroll_to_me();
            h.run_steps(6);
            let img = h.render().expect("render");
            img.save(format!("{out}/xpmf-card-{}-{part}.png", w as u32))
                .expect("save png");
        }
    }
}
