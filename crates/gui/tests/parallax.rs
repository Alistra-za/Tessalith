//! DECISIONS AH2 (owner, 2026-10-14): "Enable parallax" on the complex `_m`. Off writes alpha 0 (complex material on,
//! parallax off); a source without alpha cannot turn it on.
mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::config::{Subtype, Target};
use image::{ImageBuffer, Rgb, Rgba};
use sdc_gui::model::fields::{field_hidden, field_view};
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use serde_json::json;
use std::path::Path;
use std::time::Duration;
use support::session;

const IRON_M: &str = "m/iron_m.png";
const BOLT_M: &str = "m/bolt_m.png";

/// `iron`: a diffuse and an `_m` with a varying (height) alpha. `bolt`: a diffuse and an RGB-only `_m`.
fn fixture(dir: &Path) -> AppModel {
    let r = dir.join("m");
    std::fs::create_dir_all(&r).unwrap();
    let rgb = |x: u32, y: u32| [(x * 4) as u8, (y * 4) as u8, 90u8];
    for base in ["iron", "bolt"] {
        ImageBuffer::from_fn(64, 64, |x, y| Rgb(rgb(x, y)))
            .save(r.join(format!("{base}.png")))
            .unwrap();
    }
    ImageBuffer::from_fn(64, 64, |x, y| {
        let [a, b, c] = rgb(x, y);
        Rgba([a, b, c, (40 + x * 2) as u8])
    })
    .save(r.join("iron_m.png"))
    .unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb(rgb(x, y)))
        .save(r.join("bolt_m.png"))
        .unwrap();
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.accept = engine::policy::AcceptSuggestions::High;
    m.open_folder(dir.to_path_buf());
    idle(&mut m);
    m
}

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

fn parallax(m: &AppModel, key: &str) -> Option<bool> {
    m.item(key)
        .unwrap()
        .job
        .as_ref()
        .unwrap()
        .profile
        .alpha
        .parallax
}

fn editor(mut m: AppModel, set: &str) -> Harness<'static, AppModel> {
    let key = m
        .sets
        .iter()
        .find(|s| s.key.ends_with(set))
        .expect("the set")
        .key
        .clone();
    m.select_set(&key);
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

fn settle(h: &mut Harness<'static, AppModel>) {
    h.run();
    h.state_mut().wait_idle(Duration::from_secs(60));
    h.run();
}

#[test]
fn the_complex_m_card_checkbox_turns_parallax_off_in_one_undo_step() {
    let d = tempfile::tempdir().unwrap();
    let m = fixture(d.path());
    assert_eq!(
        m.item(IRON_M)
            .unwrap()
            .job
            .as_ref()
            .unwrap()
            .profile
            .subtype,
        Subtype::ComplexMaterial
    );
    assert_eq!(parallax(&m, IRON_M), Some(true), "varying alpha: on");
    let mut h = editor(m, "m/iron");
    h.run();
    h.get_by_label("Enable parallax").click();
    settle(&mut h);
    let m = h.state_mut();
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(IRON_M, "alpha.parallax"),
        Some(json!(false))
    );
    assert_eq!(parallax(m, IRON_M), Some(false));
    m.undo();
    idle(m);
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(IRON_M, "alpha.parallax"),
        None,
        "one undo step takes the toggle back"
    );
    assert_eq!(parallax(m, IRON_M), Some(true));
}

#[test]
fn the_settings_row_shows_only_for_complex_material_and_locks_without_source_alpha() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    let job = m.item(IRON_M).unwrap().job.clone().unwrap();
    let v = field_view(m.target, &job, "alpha.parallax").expect("a parallax row");
    assert_eq!(
        (v.label, v.value.clone(), v.lock),
        ("Enable parallax", json!(true), None)
    );
    assert!(!field_hidden(&job.profile, "alpha.parallax"));
    let diffuse = m.item("m/iron.png").unwrap().job.clone().unwrap();
    assert!(field_hidden(&diffuse.profile, "alpha.parallax"));

    m.set_field(BOLT_M, "subtype", json!(Subtype::ComplexMaterial));
    idle(&mut m);
    let job = m.item(BOLT_M).unwrap().job.clone().unwrap();
    assert_eq!(job.profile.alpha.parallax, Some(false), "no alpha: off");
    let v = field_view(Target::Cs, &job, "alpha.parallax").unwrap();
    assert!(
        v.lock.is_some_and(|w| w.contains("no alpha")),
        "{:?}",
        v.lock
    );
}

/// Fix round 1: a box ticked by the texture's own entry on a source without alpha is locked; the reset next to it
/// clears the entry.
#[test]
fn a_locked_ticked_box_has_a_reset() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_field(BOLT_M, "subtype", json!(Subtype::ComplexMaterial));
    idle(&mut m);
    m.set_field(BOLT_M, "alpha.parallax", json!(true));
    idle(&mut m);
    let job = m.item(BOLT_M).unwrap().job.clone().unwrap();
    let v = field_view(Target::Cs, &job, "alpha.parallax").unwrap();
    assert!(v.lock.is_some() && v.edited, "{v:?}");
    let mut h = editor(m, "m/bolt");
    h.run();
    h.get_by_label("↺").click();
    settle(&mut h);
    let m = h.state_mut();
    assert_eq!(
        m.project
            .as_ref()
            .unwrap()
            .file_value(BOLT_M, "alpha.parallax"),
        None
    );
    assert_eq!(parallax(m, BOLT_M), Some(false));
}

use sdc_gui::model::cs_pbr::FUZZ_TEXTURE;

const ROCK: &str = "landscape/rock01";
const ROCK_P: &str = "landscape/rock01_p.png";

/// The `_p` source's height (R): a ramp across x.
fn p_height(x: u32) -> u8 {
    (20 + x * 3) as u8
}

/// The `_p` source's fuzz roughness (G): a ramp down y.
fn p_fuzz(y: u32) -> u8 {
    (230 - y * 3) as u8
}

fn rock(m: &AppModel) -> sdc_gui::model::sets::SetView {
    sdc_gui::model::sets::views(m)
        .iter()
        .find(|v| v.id == ROCK)
        .cloned()
        .expect("the rock01 set")
}

fn p_job(m: &AppModel) -> engine::policy::ResolvedJob {
    m.item(ROCK_P)
        .unwrap()
        .job
        .clone()
        .expect("the _p is planned")
}

/// The fuzz roughness blocker of rock01, with its fix.
fn fuzz_row(m: &AppModel) -> Option<sdc_gui::model::pbr_export::ExportIssue> {
    let text = format!("{ROCK}: {}", sdc_gui::model::pbr_export::FUZZ_TWO_CHANNELS);
    sdc_gui::model::pbr_export::export_blocks(m)
        .iter()
        .find(|i| i.text == text)
        .cloned()
}

/// Note IE16: the support PBR fixture (rock01 picked `pbr`) with a `_p` holding a height in R and a fuzz roughness in
/// G, a texture-set block, fuzz on and the block's fuzz roughness texture switch `switch`. The set gets a `.csm` (a
/// block alone would pick PBR Landscape, which gets none) and, with the switch, the file holds both switches.
fn fuzz_fixture(dir: &Path, switch: bool) -> AppModel {
    use sdc_gui::model::cs_pbr::Kind;
    let mut m = support::pbr_opened_plain(dir);
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([p_height(x), p_fuzz(y), 0]))
        .save(dir.join("textures/landscape/rock01_p.png"))
        .unwrap();
    m.replan();
    idle(&mut m);
    m.decide(ROCK_P, sdc_gui::model::Answer::Subtype(Subtype::Height));
    m.add_cs_block(ROCK, Kind::Txst);
    m.set_cs_edids(ROCK, Kind::Txst, &["Rock01".into()]);
    m.set_json_value(ROCK, "fuzz_on", Some(json!(true)));
    if switch {
        m.set_cs_value(ROCK, Kind::Txst, FUZZ_TEXTURE, Some(json!(true)));
    }
    idle(&mut m);
    let v = rock(&m);
    assert!(sdc_gui::model::csm_export::gets_csm(&v), "{:?}", v.shader);
    assert!(v
        .slots
        .contains_key(&sdc_gui::model::sets::SlotKind::Height));
    let pgp = sdc_gui::model::auto_flags::resolved_for(
        v.json,
        &v.slots,
        &v.settings,
        &sdc_gui::model::auto_flags::hint(&m, &v),
    );
    let c = sdc_gui::model::csm_export::set_csm(&v, &pgp, m.cs_block(ROCK, Kind::Txst).as_ref())
        .expect("a .csm");
    let on = |k: &str| c.csm.values.get(k) == Some(&nif::csm::Value::Bool(true));
    assert!(on("PBRFuzzEnabled"), "{:?}", c.csm);
    assert_eq!(on("PBRFuzzRoughnessTexture"), switch, "{:?}", c.csm);
    assert_eq!(c.blocked, None);
    assert_eq!(
        p_job(&m).profile.format,
        engine::config::Format::Bc4,
        "the _p default"
    );
    m
}

fn problems_harness(mut m: AppModel) -> Harness<'static, AppModel> {
    m.open_problems(None);
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 900.0))
        .build_ui_state(|ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

/// Note IE18: with the switch on, a BC4 `_p` blocks Export with the brief's text; its Use BC5 button writes
/// `format = "bc5"` for the `_p` through the edit history, which clears the row, and one undo brings both back.
#[test]
fn fuzz_texture_needs_two_channels() {
    use sdc_gui::model::bottom::Fix;
    let d = tempfile::tempdir().unwrap();
    let m = fuzz_fixture(d.path(), true);
    let row = fuzz_row(&m).expect("a fuzz roughness row");
    assert_eq!(row.fix, Some(Fix::UseBc5(ROCK_P.into())));
    assert!(
        p_job(&m).profile.channels.keep_green,
        "the switch keeps the _p G"
    );
    let mut h = problems_harness(m);
    h.run();
    h.get_by_label(sdc_gui::ui::problems::USE_BC5).click();
    settle(&mut h);
    let m = h.state_mut();
    assert_eq!(
        m.project.as_ref().unwrap().file_value(ROCK_P, "format"),
        Some(json!("bc5"))
    );
    assert_eq!(p_job(m).profile.format, engine::config::Format::Bc5);
    assert!(fuzz_row(m).is_none());
    m.undo();
    idle(m);
    assert_eq!(p_job(m).profile.format, engine::config::Format::Bc4);
    assert!(fuzz_row(m).is_some(), "undo brings the row back");
}

/// Note IE36 (final review item 11): a per-file `channels.grayscale_source` other than R on a fuzz `_p` whose G the
/// engine keeps is overruled (Note IE16), and a row names the override; R itself, or no pick, gets none.
#[test]
fn an_overruled_gray_pick_has_a_row() {
    use sdc_gui::model::pbr_export::{export_asks, GRAY_PICK_OVERRULED};
    let d = tempfile::tempdir().unwrap();
    let mut m = fuzz_fixture(d.path(), true);
    let rows = |m: &AppModel| -> Vec<String> {
        export_asks(m)
            .into_iter()
            .map(|i| i.text)
            .filter(|t| t.contains(GRAY_PICK_OVERRULED))
            .collect()
    };
    assert!(rows(&m).is_empty(), "no pick: {:?}", rows(&m));
    m.set_field(ROCK_P, "channels.grayscale_source", json!("g"));
    idle(&mut m);
    assert!(p_job(&m).profile.channels.keep_green);
    assert_eq!(
        rows(&m),
        [format!(
            "{ROCK}: {ROCK_P} {GRAY_PICK_OVERRULED} \"g\" is not used"
        )]
    );
    m.set_field(ROCK_P, "channels.grayscale_source", json!("r"));
    idle(&mut m);
    assert!(rows(&m).is_empty(), "R: {:?}", rows(&m));
}

/// Two-channel formats pass; R8, one channel like BC4, is refused too.
#[test]
fn fuzz_texture_bc5_ok() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fuzz_fixture(d.path(), true);
    for (format, refused) in [("bc5", false), ("bc1", false), ("r8", true)] {
        m.set_field(ROCK_P, "format", json!(format));
        idle(&mut m);
        assert_eq!(
            serde_json::to_value(p_job(&m).profile.format).unwrap(),
            json!(format)
        );
        assert_eq!(fuzz_row(&m).is_some(), refused, "{format}");
    }
}

/// The switch off: a BC4 `_p` is fine and grey. Turning it on keeps the G and adds the row; fuzz off drops the row
/// (the `.csm` then has no fuzz) but not the G (Note IE18: the engine follows the switch alone); undo of the switch
/// plans the grey `_p` again.
#[test]
fn switch_off_bc4_ok() {
    use sdc_gui::model::cs_pbr::Kind;
    let d = tempfile::tempdir().unwrap();
    let mut m = fuzz_fixture(d.path(), false);
    assert!(fuzz_row(&m).is_none());
    assert!(!p_job(&m).profile.channels.keep_green);
    m.set_cs_value(ROCK, Kind::Txst, FUZZ_TEXTURE, Some(json!(true)));
    idle(&mut m);
    assert!(p_job(&m).profile.channels.keep_green);
    assert!(fuzz_row(&m).is_some());
    m.set_json_value(ROCK, "fuzz_on", Some(json!(false)));
    idle(&mut m);
    assert!(fuzz_row(&m).is_none(), "fuzz off: the .csm reads no fuzz");
    assert!(p_job(&m).profile.channels.keep_green);
    m.undo();
    m.undo();
    idle(&mut m);
    assert!(!p_job(&m).profile.channels.keep_green, "undo plans again");
    assert!(fuzz_row(&m).is_none());
}

/// The exported `_p` after the row's fix holds the source's height in R and its fuzz roughness in G.
#[test]
fn fuzz_texture_bc5_export_keeps_green() {
    use engine::decode::{decode_level, Decoded};
    let d = tempfile::tempdir().unwrap();
    let mut h = problems_harness(fuzz_fixture(d.path(), true));
    h.run();
    h.get_by_label(sdc_gui::ui::problems::USE_BC5).click();
    settle(&mut h);
    let m = h.state_mut();
    support::export_through(m);
    assert!(m.wait_idle(Duration::from_secs(120)), "export");
    let out = p_job(m).output;
    let f = engine::dds::read_dds(&std::fs::read(&out).unwrap()).unwrap();
    let (w, hgt, r) = f.levels[0].clone();
    let Decoded::Ldr(px) = decode_level(f.format, w, hgt, &f.data[r]).unwrap() else {
        panic!("HDR output for a _p");
    };
    let near = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs() <= 6;
    for (i, p) in px.iter().enumerate() {
        let (x, y) = (i as u32 % 64, i as u32 / 64);
        assert!(
            near(p[0], p_height(x)) && near(p[1], p_fuzz(y)),
            "{x},{y}: {p:?}"
        );
    }
}

/// Fix round 1 (item 5): with the switch on, the BC5 `_p` the fix writes gets no row that contradicts it: no
/// CHAN-NOT-GRAY (its G is meant to differ) and nothing that asks for BC4. The format list offers BC5 and BC1 for a
/// `_p`. Switch off, the same source still gets its CHAN-NOT-GRAY row, so the query sees such rows.
#[test]
fn fuzz_texture_bc5_has_no_contradicting_row() {
    use engine::codes::Code;
    use engine::config::Format;
    let grey_row = |m: &AppModel| {
        sdc_gui::model::bottom::problems(m)
            .iter()
            .any(|p| p.code == Some(Code::ChanNotGray) || p.also.contains(&Code::ChanNotGray))
    };
    let d = tempfile::tempdir().unwrap();
    let mut m = fuzz_fixture(d.path(), true);
    let formats: Vec<Format> = sdc_gui::model::fields::format_choices(m.target, &p_job(&m).profile)
        .into_iter()
        .map(|((f, _), _)| f)
        .collect();
    assert!(
        formats.contains(&Format::Bc5) && formats.contains(&Format::Bc1),
        "{formats:?}"
    );
    m.set_field(ROCK_P, "format", json!("bc5"));
    idle(&mut m);
    let rows = sdc_gui::model::bottom::problems(&m);
    assert!(!grey_row(&m), "switch on: no CHAN-NOT-GRAY");
    assert!(
        !rows.iter().any(|p| p.shown().contains("BC4")),
        "nothing asks for BC4: {:?}",
        rows.iter()
            .map(|p| p.shown().into_owned())
            .collect::<Vec<_>>()
    );
    let off = tempfile::tempdir().unwrap();
    let mut m = fuzz_fixture(off.path(), false);
    m.set_field(ROCK_P, "format", json!("bc5"));
    idle(&mut m);
    assert!(grey_row(&m), "switch off: the grey check still runs");
}

/// Fix round 1: an explicit `remove` hides the row with its own reason; each hidden field has its own reason.
#[test]
fn hidden_rows_say_why() {
    use sdc_gui::model::fields::hidden_reason;
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path());
    m.set_field(IRON_M, "alpha.policy", json!("remove"));
    idle(&mut m);
    let p = m.item(IRON_M).unwrap().job.clone().unwrap().profile;
    assert_eq!(
        hidden_reason(&p, "alpha.parallax"),
        Some("alpha removed: 255")
    );
    let diffuse = m.item("m/iron.png").unwrap().job.clone().unwrap().profile;
    assert_eq!(
        hidden_reason(&diffuse, "alpha.parallax"),
        Some("complex material _m only")
    );
    assert_eq!(
        hidden_reason(&diffuse, "mips.coverage"),
        Some("used only by cutout and cutout + blend")
    );
}
