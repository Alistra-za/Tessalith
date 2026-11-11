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
        // A repeat in other case is one row.
        json!({"editorIds": ["MatoA", "NoSuchMato", "*snow*", "Ash?Mat", "nosuchmato"]}),
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

// ---- Task 7 fix round 1 ----

/// The text box whose placeholder (greyed default) is `hint`.
fn input_with_hint<'a>(h: &'a H, hint: &str) -> egui_kittest::Node<'a> {
    h.query_all_by_role(Role::TextInput)
        .find(|n| n.accesskit_node().placeholder() == Some(hint))
        .unwrap_or_else(|| panic!("no text box hinted {hint:?}"))
}

/// The text box in the row labelled `label` (the pattern boxes all share one hint).
fn input_in_row<'a>(h: &'a H, label: &str) -> egui_kittest::Node<'a> {
    let row = row_rect(h, label);
    h.query_all_by_role(Role::TextInput)
        .find(|n| (row.top() - 4.0..=row.bottom() + 4.0).contains(&n.rect().center().y))
        .unwrap_or_else(|| panic!("no text box in row {label:?}"))
}

/// The diffuse texture Tessalith fills, as the card shows it greyed.
fn diffuse_default(m: &AppModel) -> String {
    let c = sdc_gui::model::xpmf_json::card_values(m, ID);
    c.defaults.as_ref().unwrap()["diffuseTexture"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Review I1: tabbing through (focus, then leave) an unset texture row Tessalith fills changes nothing.
#[test]
fn texture_focus_and_blur_changes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, json!({}));
    let hint = diffuse_default(&m);
    let mut h = tall(m);
    h.run_steps(4);
    input_with_hint(&h, &hint).focus();
    h.run_steps(4);
    input_with_hint(&h, ID).focus(); // the Name box
    h.run_steps(4);
    assert_eq!(block(h.state()), Some(Map::new()), "the block is unchanged");
}

/// The Texture path flow: a typed path is stored; clearing it stores "" (the game's own); Reset (named by its key)
/// goes back to Tessalith's texture.
#[test]
fn texture_path_flow_and_reset() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, json!({}));
    let hint = diffuse_default(&m);
    let mut h = tall(m);
    h.run_steps(4);
    input_with_hint(&h, &hint).focus();
    h.run_steps(4);
    h.event(egui::Event::Text("landscape/snow01".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    assert_eq!(
        block(h.state()).unwrap().get("diffuseTexture"),
        Some(&json!("landscape/snow01"))
    );
    input_in_row(&h, "Diffuse texture").focus();
    h.run_steps(4);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(2);
    h.key_press(egui::Key::Backspace);
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    assert_eq!(
        block(h.state()).unwrap().get("diffuseTexture"),
        Some(&json!("")),
        "cleared: the game's own"
    );
    h.get_by_role_and_label(Role::Button, "Reset diffuseTexture")
        .click();
    h.run_steps(4);
    assert_eq!(block(h.state()), Some(Map::new()), "back to the default");
    let es = h.state().sets_doc.as_ref().unwrap().entries();
    let e = es.iter().find(|e| e.id == ID).unwrap();
    let p = sdc_gui::model::xpmf_json::profile(h.state(), e).unwrap();
    assert_eq!(p.get("diffuseTexture"), Some(&json!(hint)));
}

/// ui-rules 8.4 (review I2): every repeated control is named by its key; Reset restores the default.
#[test]
fn controls_are_named_by_their_key() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(
        &mut m,
        json!({"shelterFade": 40, "roofShelter": false, "editorIds": ["MatoA", "NoSuchMato"]}),
    );
    let mut h = tall(m);
    h.run_steps(4);
    // Each lookup panics on a duplicate name.
    h.get_by_role_and_label(Role::Button, "Override maxAngle");
    h.get_by_role_and_label(Role::Button, "Add pattern to editorIds");
    h.get_by_role_and_label(Role::Button, "Add pattern to roofShelterSkip");
    h.get_by_role_and_label(Role::Button, "Remove pattern NoSuchMato from editorIds");
    h.get_by_role_and_label(Role::CheckBox, "Roof shelter enabled");
    h.get_by_role_and_label(Role::CheckBox, "Water fade enabled");
    h.get_by_role_and_label(Role::Button, "Reset shelterFade")
        .click();
    h.run_steps(4);
    let b = block(h.state()).unwrap();
    assert!(!b.contains_key("shelterFade"), "{b:?}");
    assert!(
        has(&h, "default: 64"),
        "shelterFade shows its default again"
    );
}

/// Enter in a pattern box adds the pattern.
#[test]
fn enter_in_pattern_box_adds() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, json!({}));
    let mut h = tall(m);
    h.run_steps(4);
    input_in_row(&h, "Exclude editor IDs").focus();
    h.run_steps(4);
    h.event(egui::Event::Text("*ash*".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    assert_eq!(
        block(h.state()).unwrap().get("excludeEditorIds"),
        Some(&json!(["*ash*"]))
    );
}
/// Task 7 Step 4: the card at 1100, 1280 and 1600 px wide, for checking against egui-ui-rules groups 2 and 3. Needs
/// a GPU; writes `xpmf-card-<w>-{top,bottom}.png` into `XPMF_SHOTS`: `cargo test -p sdc-gui --test it xpmf_card:: -- --ignored`.
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

/// The card's amber line after a refusal only `prepare` used to know (final review M1): a hand-edited value the
/// schema refuses.
#[test]
fn schema_refusal_shows_on_the_card() {
    use sdc_gui::model::xpmf_json::card_values;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, json!({"shelterFade": 40}));
    assert_eq!(card_values(&m, ID).refusal, None);
    set_block(&mut m, json!({"shelterFade": 500}));
    let why = card_values(&m, ID).refusal.clone().expect("refused");
    assert!(
        why.starts_with(&format!("{ID}: XPMF profile not written: ")),
        "{why}"
    );
    let mut h = tall(m);
    h.run_steps(4);
    assert!(has(&h, &why), "the amber line shows it");
}

/// Final review M1: two sets that fold to one file both say so on their cards, not only in the export note.
#[test]
fn fold_clash_shows_on_both_cards() {
    use sdc_gui::model::xpmf_json::card_values;
    let d = tempfile::tempdir().unwrap();
    let sub = d.path().join("textures/landscape/sub");
    std::fs::create_dir_all(&sub).unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| image::Rgb([x as u8, y as u8, 7u8]))
        .save(sub.join("Rock01.png"))
        .unwrap();
    let mut m = opened(d.path());
    let views = set_views(&m);
    let other = views
        .iter()
        .find(|v| v.id != ID && v.id.to_lowercase().ends_with("/rock01"))
        .expect("a second rock01 set");
    let (other_id, other_key) = (other.id.clone(), other.key.clone());
    let own_key = views.iter().find(|v| v.id == ID).unwrap().key.clone();
    set_block(&mut m, json!({}));
    let b = json!({"editorIds": ["MatoC"]}).as_object().unwrap().clone();
    m.sets_doc.as_mut().unwrap().set_xpmf(&other_id, Some(b));
    let say = |a: &str, b: &str| {
        format!("{a}: XPMF profile not written: tessalith_rock01.json is also the file of {b}")
    };
    assert_eq!(
        card_values(&m, ID).refusal,
        Some(say(ID, &other_id)),
        "{:?}",
        card_values(&m, ID)
    );
    assert_eq!(card_values(&m, &other_id).refusal, Some(say(&other_id, ID)));
    let mut h = tall(m);
    for (key, line) in [
        (own_key, say(ID, &other_id)),
        (other_key, say(&other_id, ID)),
    ] {
        h.state_mut().select_set(&key);
        h.run_steps(4);
        assert!(has(&h, &line), "{line}");
    }
}

/// Final review M2: a block on a set that is no longer landscape keeps its toggle and amber line (and no rows), so
/// the block can be removed.
#[test]
fn a_block_on_a_non_landscape_set_can_be_removed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    set_block(&mut m, json!({"shelterFade": 40}));
    m.set_shader(ID, Some(Shader::Pbr));
    idle(&mut m);
    let mut h = tall(m);
    h.run_steps(4);
    assert!(has(&h, TOGGLE), "the toggle stays");
    assert!(has(
        &h,
        &format!("{ID}: not a landscape set; XPMF profile not written")
    ));
    assert!(!has(&h, "Shelter fade"), "no rows");
    h.get_by_label(TOGGLE).click();
    h.run_steps(4);
    assert_eq!(block(h.state()), None, "the block is gone");
    h.run_steps(2);
    assert!(!has(&h, TOGGLE), "and with it the card");
}
