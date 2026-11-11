//! Helper textures Task 8 (Note DY17): the helper panel edits the helper file's `helper.*` keys in `sdc.toml` (role,
//! fills, read from, invert), one undo step each. Temp dirs only.
mod support;
use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use egui_kittest::Harness;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::items::ItemKind;
use sdc_gui::model::AppModel;
use sdc_gui::settings::GuiSettings;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use support::session;

type H = Harness<'static, AppModel>;

/// A folder with `files` (a diffuse for `<base>.png`, flat normals for `_n` and `_cnr`, grey ramps for the rest) and
/// `toml` as its `sdc.toml`, opened with high-confidence suggestions accepted.
fn fixture(dir: &Path, files: &[&str], toml: &str) -> AppModel {
    for f in files {
        let p = dir.join(f);
        if f.ends_with("_n.png") || f.ends_with("_cnr.png") {
            ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
                .save(p)
                .unwrap();
        } else if !f.contains('_') {
            ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
                .save(p)
                .unwrap();
        } else {
            ImageBuffer::from_fn(64, 64, |x, _| Luma([(x * 3) as u8]))
                .save(p)
                .unwrap();
        }
    }
    if !toml.is_empty() {
        std::fs::write(dir.join("sdc.toml"), toml).unwrap();
    }
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

const XSET: [&str; 3] = ["x.png", "x_roughness.png", "x_opacity.png"];
const CNRSET: [&str; 3] = ["x.png", "x_roughness.png", "x_cnr.png"];

/// The app at `size` with `key` selected.
fn open(m: AppModel, key: &str, size: egui::Vec2) -> H {
    let mut m = m;
    m.select_item(key);
    let mut h = Harness::builder()
        .with_size(size)
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h.run();
    h
}

fn app(m: AppModel, key: &str) -> H {
    open(m, key, egui::vec2(1500.0, 900.0))
}

fn value(h: &H, key: &str, field: &str) -> Option<Value> {
    h.state().project.as_ref().unwrap().file_value(key, field)
}

/// Waits for the re-plan an edit started, then draws its result.
fn settle(h: &mut H) {
    h.run();
    idle(h.state_mut());
    h.run();
    h.run();
}

/// Opens the combo named `name` and picks `option`.
fn pick(h: &mut H, name: &str, option: &str) {
    let below = h.get_by_role_and_label(Role::ComboBox, name).rect().max.y;
    h.get_by_role_and_label(Role::ComboBox, name).click();
    h.run();
    // The option in the open popup: the nearest button of that label below the combo (a "G" or "CNR" label or chip may be
    // elsewhere, and a set card's member chip can sit below the combo: Note FI13 moved one there).
    h.query_all_by_role_and_label(Role::Button, option)
        .filter(|n| n.rect().min.y >= below - 1.0)
        .min_by(|a, b| a.rect().min.y.total_cmp(&b.rect().min.y))
        .unwrap_or_else(|| panic!("no {option} under {name}"))
        .click();
    settle(h);
}

fn click(h: &mut H, name: &str) {
    h.get_by_role_and_label(Role::Button, name).click();
    settle(h);
}

/// Clicks the Helper tick (Note HI4).
fn tick(h: &mut H) {
    h.get_by_role_and_label(Role::CheckBox, "Helper").click();
    settle(h);
}

fn ticked(h: &H) -> bool {
    h.get_by_role_and_label(Role::CheckBox, "Helper")
        .accesskit_node()
        .toggled()
        == Some(egui::accesskit::Toggled::True)
}

/// The rects of the `role` controls named `name` in the centre column.
fn in_centre(h: &H, role: Role, name: &str) -> Vec<egui::Rect> {
    let c = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre");
    h.query_all_by_role_and_label(role, name)
        .map(|n| n.rect())
        .filter(|r| c.contains(r.center()))
        .collect()
}

fn toml_text(h: &H) -> String {
    h.state().project.as_ref().unwrap().text()
}

/// The Helper row reads "set by you", with the Reset role button (a `helper.role = "none"` file, Note HI6).
fn set_by_you_on_the_helper_row(h: &H) {
    let row = h.get_by_role_and_label(Role::CheckBox, "Helper").rect();
    let on_row = |r: egui::Rect| r.center().y > row.min.y && r.center().y < row.max.y;
    assert!(
        h.query_all_by_label("set by you").any(|n| on_row(n.rect())),
        "the Helper row says set by you"
    );
    assert!(on_row(
        h.get_by_role_and_label(Role::Button, "Reset role").rect()
    ));
}

/// The Textures column's helper row for `file`, by its accessible name.
fn row_name(h: &H, file: &str) -> Option<String> {
    let prefix = format!("Helper {file}:");
    h.query_all_by_role(Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .find(|l| l.starts_with(&prefix))
}

#[test]
fn role_combo_writes_helper_role() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &XSET, ""), "x_roughness.png");
    assert_eq!(
        h.query_all_by_label("Reset role").count(),
        0,
        "guessed role"
    );
    pick(&mut h, "Helper role", "Glossiness");
    assert_eq!(
        value(&h, "x_roughness.png", "helper.role"),
        Some(json!("glossiness"))
    );
    assert!(row_name(&h, "x_roughness.png")
        .unwrap()
        .contains("Glossiness (set by you)"));
    click(&mut h, "Reset role");
    assert_eq!(value(&h, "x_roughness.png", "helper.role"), None);
}

#[test]
fn read_and_invert_write_their_keys() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &XSET, ""), "x_roughness.png");
    let read = h.get_by_role_and_label(Role::ComboBox, "Read from");
    assert_eq!(read.accesskit_node().value().as_deref(), Some("Auto (R)"));
    pick(&mut h, "Read from", "G");
    assert_eq!(
        value(&h, "x_roughness.png", "helper.read"),
        Some(json!("g"))
    );
    pick(&mut h, "Read from", "Luminance");
    assert_eq!(
        value(&h, "x_roughness.png", "helper.read"),
        Some(json!("luma"))
    );
    pick(&mut h, "Read from", "Auto (R)");
    assert_eq!(value(&h, "x_roughness.png", "helper.read"), None);
    h.get_by_role_and_label(Role::CheckBox, "Invert").click();
    settle(&mut h);
    assert_eq!(
        value(&h, "x_roughness.png", "helper.invert"),
        Some(json!(true))
    );
    h.get_by_role_and_label(Role::CheckBox, "Invert").click();
    settle(&mut h);
    assert_eq!(value(&h, "x_roughness.png", "helper.invert"), None);
}

#[test]
fn not_a_helper_makes_a_tile() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &XSET, ""), "x_opacity.png");
    assert!(row_name(&h, "x_opacity.png").is_some());
    tick(&mut h);
    assert_eq!(
        value(&h, "x_opacity.png", "helper.role"),
        Some(json!("none"))
    );
    let m = h.state();
    let item = m.items.iter().find(|i| i.name == "x_opacity.png").unwrap();
    assert_ne!(item.kind, ItemKind::Helper);
    assert!(m
        .visible_items()
        .iter()
        .any(|&i| m.items[i].name == "x_opacity.png"));
    assert!(row_name(&h, "x_opacity.png").is_none(), "no helper row");
    assert!(!ticked(&h));
    set_by_you_on_the_helper_row(&h);
    click(&mut h, "Reset role");
    assert_eq!(value(&h, "x_opacity.png", "helper.role"), None);
    assert!(row_name(&h, "x_opacity.png").is_some(), "the row is back");
}

/// The brief's "a `subtype` key already on that file is left untouched throughout". With a `subtype` key, clearing
/// `helper.role` leaves the file ordinary (Ruling 13: the sub-type escape hatch), so the row cannot come back here.
#[test]
fn not_a_helper_leaves_a_subtype_key_alone() {
    let d = tempfile::tempdir().unwrap();
    let toml = "[files.\"x_opacity.png\"]\nsubtype = \"diffuse\"\nhelper.role = \"opacity\"\n";
    let mut h = app(fixture(d.path(), &XSET, toml), "x_opacity.png");
    let subtype = Some(json!("diffuse"));
    assert!(
        row_name(&h, "x_opacity.png").is_some(),
        "hand-set role wins"
    );
    tick(&mut h);
    assert_eq!(
        value(&h, "x_opacity.png", "helper.role"),
        Some(json!("none"))
    );
    assert_eq!(value(&h, "x_opacity.png", "subtype"), subtype);
    assert!(row_name(&h, "x_opacity.png").is_none());
    click(&mut h, "Reset role");
    assert_eq!(value(&h, "x_opacity.png", "helper.role"), None);
    assert_eq!(value(&h, "x_opacity.png", "subtype"), subtype);
}

#[test]
fn fills_add_and_remove_write_the_list() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &CNRSET, ""), "x_roughness.png");
    h.get_by_label("suggested");
    pick(&mut h, "Add fill target", "CNR alpha  x_cnr");
    assert_eq!(
        value(&h, "x_roughness.png", "helper.fills"),
        Some(json!(["rmaos.r", "cnr.a"]))
    );
    assert_eq!(h.query_all_by_label("suggested").count(), 0, "hand-set");
    click(&mut h, "Stop filling RMAOS R");
    assert_eq!(
        value(&h, "x_roughness.png", "helper.fills"),
        Some(json!(["cnr.a"]))
    );
    click(&mut h, "Stop filling CNR alpha");
    assert_eq!(
        value(&h, "x_roughness.png", "helper.fills"),
        Some(json!([]))
    );
    let row = row_name(&h, "x_roughness.png").unwrap();
    assert!(row.contains("not used"), "{row}");
    h.get_by_label("not used: no targets chosen");
}

/// Final review M1: a hand-set list shows every target from `sdc.toml` (DY18b), so ✕ on one never drops a hand target
/// that did not land (`n.a` under vanilla with no `_n`).
#[test]
fn hand_targets_that_did_not_land_stay_listed() {
    let d = tempfile::tempdir().unwrap();
    let toml = "[files.\"x_opacity.png\"]\nhelper.fills = [\"diffuse.a\", \"n.a\"]\n";
    let mut m = fixture(d.path(), &["x.png", "x_opacity.png"], toml);
    m.set_target(engine::config::Target::Vanilla);
    idle(&mut m);
    let mut h = app(m, "x_opacity.png");
    h.get_by_role_and_label(Role::Button, "Stop filling D alpha");
    h.get_by_role_and_label(Role::Button, "Stop filling N alpha");
    click(&mut h, "Stop filling D alpha");
    assert_eq!(
        value(&h, "x_opacity.png", "helper.fills"),
        Some(json!(["n.a"]))
    );
}

#[test]
fn fills_reset_returns_to_suggestion() {
    let d = tempfile::tempdir().unwrap();
    let toml = "[files.\"x_roughness.png\"]\nhelper.fills = [\"cnr.a\"]\n";
    let mut h = app(fixture(d.path(), &CNRSET, toml), "x_roughness.png");
    assert_eq!(h.query_all_by_label("suggested").count(), 0);
    click(&mut h, "Reset fills");
    assert_eq!(value(&h, "x_roughness.png", "helper.fills"), None);
    h.get_by_label("suggested");
    assert_eq!(h.query_all_by_label("Reset fills").count(), 0);
}

#[test]
fn read_only_project_disables_controls() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &XSET, "target = [\n"), "x_roughness.png");
    for (role, name) in [
        (Role::ComboBox, "Helper role"),
        (Role::ComboBox, "Add fill target"),
        (Role::ComboBox, "Read from"),
        (Role::CheckBox, "Invert"),
        (Role::Button, "Stop filling RMAOS R"),
    ] {
        let n = h.get_by_role_and_label(role, name);
        assert!(n.accesskit_node().is_disabled(), "{name} is enabled");
    }
    h.get_by_role_and_label(Role::ComboBox, "Helper role")
        .hover();
    h.run();
    assert!(
        h.query_by_label_contains("sdc.toml is read-only").is_some(),
        "the reason on hover"
    );
}

#[test]
fn panel_controls_inside_panel_at_1100() {
    let d = tempfile::tempdir().unwrap();
    // A long set name makes every fill label long.
    let base = format!("x{}", "a".repeat(64));
    let long = format!("{base}_roughness.png");
    let (diffuse, cnr) = (format!("{base}.png"), format!("{base}_cnr.png"));
    let files = [diffuse.as_str(), cnr.as_str(), long.as_str()];
    let toml = format!("[files.\"{long}\"]\nhelper.fills = [\"rmaos.r\", \"cnr.a\"]\n");
    let h = open(
        fixture(d.path(), &files, &toml),
        &long,
        egui::vec2(1100.0, 800.0),
    );
    let centre = sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn");
    let mut seen = vec![];
    for n in h.query_all(egui_kittest::kittest::by()) {
        let ak = n.accesskit_node();
        if !matches!(
            ak.role(),
            Role::Button | Role::ComboBox | Role::CheckBox | Role::Link
        ) {
            continue;
        }
        let r = n.rect();
        if r.min.x < centre.min.x || r.min.x >= centre.max.x || r.min.y < centre.min.y {
            continue;
        }
        let name = ak.label().unwrap_or_default();
        // The Advanced drawer strip is no part of the helper panel; at 1100 px it overlaps the centre by 18 px (the
        // old, over-wide centre hid that).
        if name == "Advanced" {
            continue;
        }
        assert!(
            r.max.x <= centre.max.x,
            "{name} ends at {} past {}",
            r.max.x,
            centre.max.x
        );
        seen.push(name);
    }
    let stop_cnr = "Stop filling CNR alpha".to_string();
    let stop_rmaos = "Stop filling RMAOS R".to_string();
    for want in [
        "Helper role",
        "Add fill target",
        "Read from",
        "Invert",
        &stop_cnr,
        &stop_rmaos,
        "Reset fills",
    ] {
        assert!(seen.iter().any(|s| s == want), "{want} not in {seen:?}");
    }
}

#[test]
fn one_undo_step_per_edit() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &XSET, ""), "x_roughness.png");
    // Steps before the edit: the opening's "Set n types automatically" (Note FC1), if any.
    let steps = h.state().history.undo.len();
    pick(&mut h, "Helper role", "Glossiness");
    assert!(value(&h, "x_roughness.png", "helper.role").is_some());
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    settle(&mut h);
    assert_eq!(value(&h, "x_roughness.png", "helper.role"), None);
    assert_eq!(h.state().history.undo.len(), steps, "one step");
}

/// The rect of the widget with `role` and `name` (nothing here is clipped, so it equals the interact rect).
fn irect(h: &H, role: Role, name: &str) -> egui::Rect {
    h.get_by_role_and_label(role, name).rect()
}

const HAND_FILLS: &str = "[files.\"x_roughness.png\"]\nhelper.role = \"roughness\"\nhelper.fills = [\"rmaos.r\", \"cnr.a\"]\n";

/// Owner, 2026-11-06 ("this looks sloppy"): the Fills rows share the combos' left edge and the Reset column, and the
/// Reset sits on the Add line.
#[test]
fn fills_rows_line_up() {
    let d = tempfile::tempdir().unwrap();
    let mut h = open(
        fixture(d.path(), &CNRSET, HAND_FILLS),
        "x_roughness.png",
        egui::vec2(1280.0, 800.0),
    );
    h.run();
    let reset_role = irect(&h, Role::Button, "Reset role");
    let reset_fills = irect(&h, Role::Button, "Reset fills");
    let role = irect(&h, Role::ComboBox, "Helper role");
    let add = irect(&h, Role::ComboBox, "Add fill target");
    let near = |a: f32, b: f32, what: &str| assert!((a - b).abs() <= 1.0, "{what}: {a} vs {b}");
    near(reset_fills.min.x, reset_role.min.x, "Reset fills x");
    near(add.min.x, role.min.x, "Add combo x");
    near(add.width(), role.width(), "Add combo width");
    assert!(
        reset_fills.center().y >= add.min.y && reset_fills.center().y <= add.max.y,
        "Reset fills {reset_fills:?} is not on the Add line {add:?}"
    );
    let text_x = role.min.x + h.ctx.global_style().spacing.button_padding.x;
    for (stop, label) in [
        ("Stop filling RMAOS R", "RMAOS R  x_rmaos"),
        ("Stop filling CNR alpha", "CNR alpha  x_cnr"),
    ] {
        near(irect(&h, Role::Button, stop).min.x, reset_role.min.x, stop);
        let t = h
            .query_all_by_label(label)
            .filter(|n| n.rect().min.y > role.max.y)
            .map(|n| n.rect())
            .next()
            .unwrap_or_else(|| panic!("no {label} text"));
        near(t.min.x, text_x, label);
    }
}

/// The hint column sits on the first Fills line, not centred on the block.
#[test]
fn the_hint_column_says_set_by_you_or_suggested_on_the_first_fills_line() {
    let d = tempfile::tempdir().unwrap();
    for (toml, hint) in [("", "suggested"), (HAND_FILLS, "set by you")] {
        let h = open(
            fixture(d.path(), &CNRSET, toml),
            "x_roughness.png",
            egui::vec2(1280.0, 800.0),
        );
        let stop = irect(&h, Role::Button, "Stop filling RMAOS R");
        let add = irect(&h, Role::ComboBox, "Add fill target");
        let hint_rect = h
            .query_all_by_label(hint)
            .map(|n| n.rect())
            .find(|r| r.min.y > irect(&h, Role::ComboBox, "Helper role").max.y)
            .unwrap_or_else(|| panic!("no {hint}"));
        assert!(
            hint_rect.center().y >= stop.min.y && hint_rect.center().y <= stop.max.y,
            "{hint} {hint_rect:?} is not on the first Fills line {stop:?}"
        );
        assert!(hint_rect.max.y < add.min.y, "{hint} sits on the Add line");
    }
}

/// Owner, 2026-11-06: the target said `bake_1001_f_rmaos B` while the button said `Show bake_1001_rmaos`. A packed
/// output's own name is the target's name; a target that did not land falls back to the set's name.
#[test]
fn a_packed_target_is_named_by_its_output() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path(), &CNRSET, "");
    let set = sdc_gui::model::sets::views(&m)
        .iter()
        .find(|v| v.key.ends_with("/x") || v.key == "x")
        .expect("the x set")
        .id
        .clone();
    m.set_output_name(&set, Some("steel")).unwrap();
    idle(&mut m);
    let rep = m.item("x_roughness.png").unwrap().helper.clone().unwrap();
    let tails = m.helper_tails(&rep);
    let rmaos = engine::helper::FillTarget::parse("rmaos.r").unwrap();
    let lost = engine::helper::FillTarget::parse("m.r").unwrap();
    assert_eq!(
        sdc_gui::model::helpers::target_file(&rep, rmaos, tails),
        "steel_rmaos"
    );
    // Final review M4: a target that did not land is named by the outputs' base name, like the rows beside it.
    assert_eq!(
        sdc_gui::model::helpers::target_file(&rep, lost, tails),
        "steel_m",
        "no output of its own: another output's base name"
    );
    // Final review K2: the colour output has no suffix, and its stem is the base too.
    let colour = engine::helper::FillTarget::Alpha(engine::helper::AlphaTarget::Diffuse);
    assert_eq!(
        sdc_gui::model::helpers::target_file(&rep, colour, tails),
        "steel"
    );
    let h = app(m, "x_roughness.png");
    h.get_by_role_and_label(Role::Button, "Stop filling RMAOS R");
    h.get_by_role_and_label(Role::Button, "Show steel_rmaos");
    assert!(
        h.query_all_by_label_contains("fills RMAOS R  steel_rmaos")
            .count()
            >= 1
    );
}

/// Review pictures of the helper panel at 1100, 1280 and 1600 px (egui-ui-rules step 5). Needs a GPU and
/// `SDC_SHOTS_DIR`: `cargo test -p sdc-gui --test helper_panel -- --ignored shots`.
#[test]
#[ignore]
fn shots() {
    let Ok(out) = std::env::var("SDC_SHOTS_DIR") else {
        return;
    };
    let d = tempfile::tempdir().unwrap();
    let files = ["x.png", "x_roughness.png", "x_cnr.png", "x_n.png"];
    let toml = "[files.\"x_roughness.png\"]\nhelper.fills = [\"rmaos.r\", \"cnr.a\"]\nhelper.read = \"g\"\n";
    for (w, hgt) in [(1100.0, 800.0), (1280.0, 800.0), (1600.0, 900.0)] {
        let key = "x_roughness.png";
        let mut m = fixture(d.path(), &files, toml);
        m.select_item(key);
        let mut h = Harness::builder()
            .with_size(egui::vec2(w, hgt))
            .with_pixels_per_point(1.0)
            .wgpu()
            .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run_steps(10);
        let img = h.render().expect("render");
        let stem = key.trim_end_matches(".png");
        img.save(Path::new(&out).join(format!("helper-panel-{stem}-{w}.png")))
            .expect("save png");
    }
}

/// Review fix (DY17): picking the guessed role writes it, so it becomes assigned and replaces the diffuse's own
/// varying alpha, which a suggestion never does.
#[test]
fn picking_the_guessed_role_hand_sets_it() {
    let d = tempfile::tempdir().unwrap();
    image::ImageBuffer::from_fn(64, 64, |x, y| {
        image::Rgba([(x * 4) as u8, (y * 4) as u8, 90u8, (x * 4) as u8])
    })
    .save(d.path().join("x.png"))
    .unwrap();
    let mut h = app(fixture(d.path(), &["x_opacity.png"], ""), "x_opacity.png");
    let rep = |h: &H| {
        h.state()
            .item("x_opacity.png")
            .unwrap()
            .helper
            .clone()
            .unwrap()
    };
    assert!(!rep(&h).role_hand_set);
    assert!(
        rep(&h).fills.is_empty(),
        "a suggestion leaves real alpha: {:?}",
        rep(&h)
    );
    pick(&mut h, "Helper role", "Opacity");
    assert_eq!(
        value(&h, "x_opacity.png", "helper.role"),
        Some(json!("opacity"))
    );
    assert!(rep(&h).role_hand_set);
    assert_eq!(
        rep(&h).fills,
        [engine::helper::FillTarget::Alpha(
            engine::helper::AlphaTarget::Diffuse
        )]
    );
    h.get_by_label("✎ Opacity");
}

/// A 4x4 uncompressed A8R8G8B8 DDS at `p`.
fn tiny_dds(p: &Path) {
    let mut b: Vec<u8> = b"DDS ".to_vec();
    let mut u = |v: u32| b.extend_from_slice(&v.to_le_bytes());
    for v in [124, 0x100F, 4, 4, 16, 0, 0] {
        u(v);
    }
    for _ in 0..11 {
        u(0);
    }
    for v in [
        32,
        0x41,
        0,
        32,
        0x00ff_0000,
        0x0000_ff00,
        0x0000_00ff,
        0xff00_0000,
        0x1000,
        0,
        0,
        0,
        0,
    ] {
        u(v);
    }
    b.extend(std::iter::repeat_n(0x80u8, 64));
    std::fs::write(p, b).unwrap();
}

/// Docs review (Note DY17): an image whose name has no helper suffix can be made a helper from its own panel.
#[test]
fn ordinary_image_can_become_a_helper() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(
        fixture(d.path(), &["x.png", "x_mask.png"], ""),
        "x_mask.png",
    );
    let item = |h: &H| h.state().item("x_mask.png").unwrap().clone();
    assert_ne!(item(&h).kind, ItemKind::Helper, "a tile to start with");
    let steps = h.state().history.undo.len();
    let opened_type = value(&h, "x_mask.png", "subtype");
    assert!(!ticked(&h), "an unticked Helper box");
    tick(&mut h);
    pick(&mut h, "Helper role", "Opacity");
    assert_eq!(
        value(&h, "x_mask.png", "helper.role"),
        Some(json!("opacity"))
    );
    let it = item(&h);
    assert_eq!(it.kind, ItemKind::Helper);
    assert_eq!(
        it.helper.unwrap().fills,
        [engine::helper::FillTarget::Alpha(
            engine::helper::AlphaTarget::Diffuse
        )]
    );
    assert!(row_name(&h, "x_mask.png")
        .unwrap()
        .contains("fills D alpha"));
    // The selection stays on the file, now the helper panel.
    h.get_by_role_and_label(Role::ComboBox, "Read from");
    // One undo step, and the subtype key is not written by it (the type the opening recorded, Note FC1, stays).
    assert_eq!(value(&h, "x_mask.png", "subtype"), opened_type);
    h.state_mut().undo();
    settle(&mut h);
    assert_eq!(value(&h, "x_mask.png", "helper.role"), None);
    assert_eq!(value(&h, "x_mask.png", "subtype"), opened_type);
    assert_eq!(h.state().history.undo.len(), steps);
}

#[test]
fn dds_has_no_helper_role_row() {
    let d = tempfile::tempdir().unwrap();
    tiny_dds(&d.path().join("y_mask.dds"));
    let h = app(fixture(d.path(), &["x.png"], ""), "y_mask.dds");
    assert_eq!(h.state().item("y_mask.dds").unwrap().kind, ItemKind::Dds);
    assert_eq!(h.query_all_by_label("Helper role").count(), 0);
    assert_eq!(
        h.query_all_by_role_and_label(Role::CheckBox, "Helper")
            .count(),
        0
    );
    h.get_by_label_contains("Already a DDS file");
}

#[test]
fn read_only_disables_the_helper_role_row() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(
        fixture(d.path(), &["x.png", "x_mask.png"], "target = [\n"),
        "x_mask.png",
    );
    let n = h.get_by_role_and_label(Role::CheckBox, "Helper");
    assert!(n.accesskit_node().is_disabled());
    n.hover();
    h.run();
    assert!(h.query_by_label_contains("sdc.toml is read-only").is_some());
    // The role combo is off too, with the same reason. The tick can't be clicked here, so set the ticked state the
    // tick keeps in egui temp data (`helper_panel::tick_id`, Note HI5).
    h.ctx
        .data_mut(|d| d.insert_temp(egui::Id::new(("helper-tick", "x_mask.png")), true));
    h.run();
    let role = h.get_by_role_and_label(Role::ComboBox, "Helper role");
    assert!(role.accesskit_node().is_disabled());
    role.hover();
    h.run();
    assert!(h.query_by_label_contains("sdc.toml is read-only").is_some());
}

/// Note HI4: the tick swaps Sub-type for the role list; nothing is written until a role is picked (HI5).
#[test]
fn ticking_helper_swaps_the_dropdown_and_writes_nothing() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &XSET, ""), "x.png");
    let sub = in_centre(&h, Role::ComboBox, "Sub-type");
    assert_eq!(sub.len(), 1, "{sub:?}");
    let (text, dirty) = (toml_text(&h), h.state().is_dirty());
    tick(&mut h);
    assert!(ticked(&h));
    let role = in_centre(&h, Role::ComboBox, "Helper role");
    assert_eq!(role.len(), 1, "{role:?}");
    assert!(
        (role[0].min - sub[0].min).length() <= 1.0
            && (role[0].width() - sub[0].width()).abs() <= 1.0,
        "Helper role {:?} where Sub-type was {:?}",
        role[0],
        sub[0]
    );
    let shown = h.get_by_role_and_label(Role::ComboBox, "Helper role");
    assert_eq!(
        shown.accesskit_node().value().as_deref(),
        Some("Pick a role…")
    );
    h.get_by_label("pick a role to make this a helper");
    assert!(in_centre(&h, Role::ComboBox, "Sub-type").is_empty());
    assert_eq!(toml_text(&h), text);
    assert_eq!(h.state().is_dirty(), dirty);
}

#[test]
fn picking_a_role_makes_it_a_helper() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &XSET, ""), "x.png");
    tick(&mut h);
    pick(&mut h, "Helper role", "Glossiness");
    assert_eq!(
        value(&h, "x.png", engine::helper::ROLE_FIELD),
        Some(json!("glossiness"))
    );
    assert_eq!(h.state().item("x.png").unwrap().kind, ItemKind::Helper);
    // The helper panel, ticked.
    h.get_by_role_and_label(Role::ComboBox, "Read from");
    assert!(ticked(&h));
}

#[test]
fn unticking_before_picking_goes_back() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &XSET, ""), "x.png");
    let text = toml_text(&h);
    tick(&mut h);
    tick(&mut h);
    assert!(!ticked(&h));
    assert_eq!(in_centre(&h, Role::ComboBox, "Sub-type").len(), 1);
    assert!(in_centre(&h, Role::ComboBox, "Helper role").is_empty());
    assert_eq!(toml_text(&h), text);
}

#[test]
fn the_tick_stays_on_its_own_file() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &["x.png", "y.png"], ""), "x.png");
    let text = toml_text(&h);
    tick(&mut h);
    h.state_mut().select_item("y.png");
    h.run();
    h.run();
    assert!(!ticked(&h), "y reads unticked");
    assert_eq!(in_centre(&h, Role::ComboBox, "Sub-type").len(), 1);
    h.state_mut().select_item("x.png");
    h.run();
    h.run();
    assert!(ticked(&h), "x is still ticked");
    assert_eq!(in_centre(&h, Role::ComboBox, "Helper role").len(), 1);
    assert_eq!(toml_text(&h), text);
}

#[test]
fn unticking_a_helper_writes_none() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &XSET, ""), "x_roughness.png");
    assert!(ticked(&h), "a helper reads ticked");
    tick(&mut h);
    assert_eq!(
        value(&h, "x_roughness.png", engine::helper::ROLE_FIELD),
        Some(json!("none"))
    );
    assert_eq!(
        in_centre(&h, Role::ComboBox, "Sub-type").len(),
        1,
        "the texture panel"
    );
    assert!(!ticked(&h));
    set_by_you_on_the_helper_row(&h);
    click(&mut h, "Reset role");
    assert_eq!(
        value(&h, "x_roughness.png", engine::helper::ROLE_FIELD),
        None
    );
    assert!(ticked(&h), "the guessed helper again");
}

#[test]
fn a_helper_role_list_has_no_not_a_helper_entry() {
    let d = tempfile::tempdir().unwrap();
    let mut h = app(fixture(d.path(), &XSET, ""), "x_roughness.png");
    h.get_by_role_and_label(Role::ComboBox, "Helper role")
        .click();
    h.run();
    h.get_by_role_and_label(Role::Button, "Glossiness");
    assert_eq!(h.query_all_by_label("Not a helper").count(), 0);
}

/// A failed image has no Type group; its Helper tick stays reachable, and ticked it offers the roles (Note HI4).
#[test]
fn a_jobless_image_keeps_the_tick() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(d.path(), &XSET, "");
    let i = m.items.iter().position(|i| i.key == "x.png").unwrap();
    m.items[i].status = sdc_gui::model::items::ItemStatus::Failed("no good".into());
    m.items[i].job = None;
    let mut h = app(m, "x.png");
    h.get_by_label("Failed: no good");
    assert!(!ticked(&h));
    tick(&mut h);
    assert!(ticked(&h));
    h.get_by_role_and_label(Role::ComboBox, "Helper role");
}

const MASK_ROUGHNESS: &str = "[files.\"x_mask.png\"]\nhelper.role = \"roughness\"\n";

fn packed_keys(m: &AppModel) -> Vec<String> {
    m.items
        .iter()
        .filter(|i| i.kind == ItemKind::Packed)
        .map(|i| i.key.clone())
        .collect()
}

/// Note HS2: a joined helper's packed output is keyed as the engine reads it, under the set's name.
#[test]
fn a_joined_helpers_packed_output_is_keyed_as_the_set() {
    let d = tempfile::tempdir().unwrap();
    let m = fixture(
        d.path(),
        &["x.png", "x_ao.png", "x_mask.png"],
        MASK_ROUGHNESS,
    );
    assert!(m.item("x_rmaos").is_some(), "{:?}", packed_keys(&m));
    assert!(m.item("x_mask_rmaos").is_none());
}

/// Note HS2 (Review Focus 3): a helper whose name differs from the set's in letter case keeps one packed item, and a
/// fill under its key reaches the job.
#[test]
fn a_joined_helper_differing_in_case_keeps_one_item() {
    let d = tempfile::tempdir().unwrap();
    let mut m = fixture(
        d.path(),
        &["X.png", "X_ao.png", "x_mask.png"],
        MASK_ROUGHNESS,
    );
    let keys = packed_keys(&m);
    assert_eq!(keys.len(), 1, "{keys:?}");
    m.set_pack_fill(&keys[0], engine::config::RawMap::Metallic, Some(77));
    idle(&mut m);
    let job = m
        .item(&keys[0])
        .and_then(|i| i.job.as_ref())
        .expect("the job");
    assert_eq!(
        job.pack.as_ref().unwrap().channels[1],
        engine::pack::ChannelSource::Fill {
            role: engine::config::RawMap::Metallic,
            code: 77
        },
        "{keys:?}"
    );
}
