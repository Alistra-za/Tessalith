//! Texture type words: the table's labels and descriptions, and where the card and the pills show them.
mod support;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use image::{ImageBuffer, Rgb};
use sdc_gui::model::AppModel;
use std::path::Path;
use std::time::Duration;
use support::*;

fn harness(model: AppModel) -> Harness<'static, AppModel> {
    Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(eframe::egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, model: &mut AppModel| sdc_gui::ui::draw(ui, model),
            model,
        )
}

fn key_of(m: &AppModel, name: &str) -> String {
    m.items
        .iter()
        .find(|i| i.key.ends_with(name))
        .expect("an item with that name")
        .key
        .clone()
}

/// `rock_bump.png` alone waits for a type (copied from `unknown_suffix.rs`).
fn waiting_folder(dir: &Path) -> AppModel {
    let c = dir.join("textures/clutter");
    std::fs::create_dir_all(&c).unwrap();
    let grad = |x: u32, y: u32| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]);
    ImageBuffer::from_fn(64, 64, grad)
        .save(c.join("rock_bump.png"))
        .unwrap();
    opened_as_the_app(dir.join("textures"), dir.join("export"))
}

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "model did not settle");
}

fn entry_below<'a, S>(
    h: &'a Harness<'_, S>,
    label: &'a str,
    combo: eframe::egui::Rect,
) -> egui_kittest::Node<'a> {
    h.query_all_by_label(label)
        .find(|n| n.rect().min.y > combo.max.y)
        .unwrap_or_else(|| panic!("{label} in the open combo"))
}
/// `clutter/a_bump.png` and `other/b_bump.png`, each alone in its set, both a flat tangent normal; `a` is decided
/// Height. Returns the model and the keys of a and b.
fn two_bumps(dir: &Path) -> (AppModel, String, String) {
    for (folder, name) in [("clutter", "a_bump.png"), ("other", "b_bump.png")] {
        let f = dir.join("textures").join(folder);
        std::fs::create_dir_all(&f).unwrap();
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
            .save(f.join(name))
            .unwrap();
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    idle(&mut m);
    let (a, b) = (key_of(&m, "a_bump.png"), key_of(&m, "b_bump.png"));
    m.decide_undecided_row(&a, engine::config::Subtype::Height);
    idle(&mut m);
    (m, a, b)
}

#[test]
fn the_card_shows_likely_buttons() {
    let d = tempfile::tempdir().unwrap();
    let (mut m, _a, b) = two_bumps(d.path());
    m.select_item(&b);
    let mut h = harness(m);
    h.run();
    h.run();
    h.get_by_label("Use Height (parallax)").click();
    h.run();
    // The combo's value is the type the button picked; nothing is written before Confirm.
    assert_eq!(
        h.get_by_label("Type for this texture").value().as_deref(),
        Some("Height (parallax)")
    );
    assert!(h.state().undecided_keys().contains(&b));
}

#[test]
fn likely_types_rank_a_written_type_then_pixels() {
    use engine::config::Subtype;
    let d = tempfile::tempdir().unwrap();
    let (m, _a, b) = two_bumps(d.path());
    // An unknown suffix gives no engine suggestion (GY3: `unknown_suffix`).
    assert_eq!(
        sdc_gui::model::likely::likely_types(&m, std::slice::from_ref(&b)),
        vec![
            (Subtype::Height, "a_bump.png has this type.".to_string()),
            (
                Subtype::Normal,
                "The pixels look like a normal map.".to_string()
            ),
        ]
    );
}

#[test]
fn a_restricted_picker_skips_an_unavailable_likely_type() {
    use engine::config::{Subtype, Target};
    let d = tempfile::tempdir().unwrap();
    let (mut m, _a, b) = two_bumps(d.path());
    m.set_target(Target::Vanilla);
    idle(&mut m);
    // Three CS-only engine suggestions outrank the written Height and the pixels' Normal.
    let cs_only = [Subtype::Rmaos, Subtype::SkinRfaos, Subtype::Flow];
    let job = m
        .items
        .iter_mut()
        .find(|i| i.key == b)
        .and_then(|i| i.job.as_mut())
        .expect("a job");
    job.suggestions = cs_only
        .iter()
        .map(|st| engine::suggest::Suggestion {
            subtype: *st,
            confidence: engine::suggest::Confidence::High,
            reason: "cs only".to_string(),
        })
        .collect();
    for st in cs_only {
        assert!(!m.type_offered_to(&b, st), "{st:?} is CS only");
    }
    // Five candidates before the filter, two after: taking three first would leave only the CS-only ones.
    assert_eq!(
        sdc_gui::model::likely::likely_types(&m, &[b]),
        vec![
            (Subtype::Height, "a_bump.png has this type.".to_string()),
            (
                Subtype::Normal,
                "The pixels look like a normal map.".to_string()
            ),
        ]
    );
}

/// Four textures in four folders. `p_wait.png` and `q_wait.png` wait for a type, with one engine suggestion each
/// (Normal and Height, which no real file name gives a waiting texture, so they are set on the jobs); `r_wait.png`
/// is written Gray and `s_wait.png` Envmask. Returns the model and the keys of p and q.
fn bulk_fixture(dir: &Path) -> (AppModel, String, String) {
    use engine::config::Subtype;
    for (folder, name) in [
        ("one", "p_wait.png"),
        ("two", "q_wait.png"),
        ("three", "r_wait.png"),
        ("four", "s_wait.png"),
    ] {
        let f = dir.join("textures").join(folder);
        std::fs::create_dir_all(&f).unwrap();
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
            .save(f.join(name))
            .unwrap();
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    idle(&mut m);
    let key = |m: &AppModel, n: &str| key_of(m, n);
    let (p, q, r, s) = (
        key(&m, "p_wait.png"),
        key(&m, "q_wait.png"),
        key(&m, "r_wait.png"),
        key(&m, "s_wait.png"),
    );
    m.set_field(&r, "subtype", serde_json::json!(Subtype::Gray));
    m.set_field(&s, "subtype", serde_json::json!(Subtype::Envmask));
    idle(&mut m);
    for (k, st, why) in [
        (&p, Subtype::Normal, "the engine reads p as a normal"),
        (&q, Subtype::Height, "the engine reads q as a height"),
    ] {
        let job = m
            .items
            .iter_mut()
            .find(|i| &i.key == k)
            .and_then(|i| i.job.as_mut())
            .expect("a job");
        job.suggestions = vec![engine::suggest::Suggestion {
            subtype: st,
            confidence: engine::suggest::Confidence::High,
            reason: why.to_string(),
        }];
    }
    (m, p, q)
}

/// The bulk card's buttons in drawing order, with the selection taken in `order`.
fn bulk_buttons(m: AppModel, order: [&String; 2]) -> Harness<'static, AppModel> {
    let mut m = m;
    m.select_item(order[0]);
    m.click_item(order[1], true, false);
    let mut h = harness(m);
    h.run();
    h.run();
    h
}

/// One run of the tie-break check with the two waiting textures selected in the given order.
fn tie_break_run(reversed: bool) {
    use engine::config::Subtype;
    let d = tempfile::tempdir().unwrap();
    let (m, p, q) = bulk_fixture(d.path());
    // Before drawing: the fixture is what the test says it is.
    for (k, st) in [(&p, Subtype::Normal), (&q, Subtype::Height)] {
        let j = m.item(k).and_then(|i| i.job.as_ref()).unwrap();
        assert_eq!(j.suggestions.len(), 1);
        assert_eq!(j.suggestions[0].subtype, st);
        assert!(m.type_offered_to(k, Subtype::Gray) && m.type_offered_to(k, Subtype::Envmask));
    }
    let doc = m.project.as_ref().unwrap();
    let written: Vec<_> = doc
        .file_keys()
        .into_iter()
        .filter(|k| k.ends_with("_wait.png"))
        .map(|k| (k.clone(), doc.file_value(&k, "subtype")))
        .collect();
    assert_eq!(written.len(), 2, "only r and s are written: {written:?}");
    for (k, v) in &written {
        let want = if k.ends_with("r_wait.png") {
            serde_json::json!(Subtype::Gray)
        } else {
            serde_json::json!(Subtype::Envmask)
        };
        assert_eq!(v.as_ref(), Some(&want), "{k}");
    }
    let want = vec![
        (
            Subtype::Height,
            "the engine reads q as a height".to_string(),
        ),
        (
            Subtype::Normal,
            "the engine reads p as a normal".to_string(),
        ),
        (Subtype::Envmask, "s_wait.png has this type.".to_string()),
    ];
    let keys = vec![p.clone(), q.clone()];
    assert_eq!(sdc_gui::model::likely::likely_types(&m, &keys), want);
    let order = if reversed { [&q, &p] } else { [&p, &q] };
    let mut h = bulk_buttons(m, order);
    assert_eq!(h.state().selected_undecided().len(), 2);
    // Drawing order: row first, then left to right.
    let at = |h: &Harness<'static, AppModel>, label: &str| {
        let r = h.get_by_label(label).rect();
        (r.min.y.round() as i32, r.min.x.round() as i32)
    };
    let (a, b, c) = (
        at(&h, "Use Height (parallax)"),
        at(&h, "Use Normal map"),
        at(&h, "Use Environment mask"),
    );
    assert!(a < b && b < c, "{a:?} {b:?} {c:?}");
    assert!(h.query_by_label("Use Grey map").is_none());
    // Each button's hover is its reason.
    instant_tips(&h.ctx);
    for (label, why) in [
        ("Use Height (parallax)", "the engine reads q as a height"),
        ("Use Normal map", "the engine reads p as a normal"),
        ("Use Environment mask", "s_wait.png has this type."),
    ] {
        h.get_by_label(label).hover();
        h.run();
        assert!(h.query_by_label(why).is_some(), "{label}: {why}");
    }
    let before = h.state().project.as_ref().unwrap().text();
    if reversed {
        h.get_by_label("Use Environment mask").click();
        h.run();
        assert_eq!(
            h.get_by_label("Type for 2 textures").value().as_deref(),
            Some("Environment mask")
        );
        assert_eq!(h.state().project.as_ref().unwrap().text(), before);
        assert_eq!(h.state().selected_undecided().len(), 2);
    }
}

#[test]
fn the_bulk_picker_breaks_likely_type_ties_by_id() {
    tie_break_run(false);
    tie_break_run(true);
}

#[test]
fn every_type_has_a_label_a_description_and_a_suffix() {
    for st in engine::config::Subtype::ALL {
        // config.rs:360, [Subtype; 34]
        let i = sdc_gui::model::labels::type_info(st);
        assert!(
            !i.label.is_empty() && !i.description.is_empty() && !i.suffix.is_empty(),
            "{st:?}"
        );
        assert!(
            !i.label.contains('_'),
            "{st:?} label has an underscore: {}",
            i.label
        );
        assert!(i.description.ends_with('.'), "{st:?}");
        assert!(
            !i.description.contains('`') && !i.suffix.contains('`'),
            "{st:?} ships Markdown backticks"
        ); // round 3
    }
}

#[test]
fn the_table_reads_like_the_wiki() {
    use engine::config::Subtype::*;
    use sdc_gui::model::labels::{subtype_name, type_info};
    assert_eq!(subtype_name(SkinSk), "Skin subsurface mask");
    assert_eq!(subtype_name(Msn), "Model-space normal");
    assert_eq!(subtype_name(Envmask), "Environment mask");
    assert_eq!(subtype_name(GenericTwoChannel), "Two-channel data");
    assert_eq!(subtype_name(Rmaos), "PBR RMAOS");
    assert_eq!(type_info(Rmaos).suffix, "_rmaos");
    assert_eq!(
        type_info(Normal).description,
        "The surface bumps of an ordinary material (tangent space)."
    );
    assert_eq!(
        sdc_gui::model::labels::type_hover(Normal),
        "The surface bumps of an ordinary material (tangent space).\nFile name: _n\nA file alone under its base name becomes a set of its own once it has a type."
    );
    let gray = sdc_gui::model::labels::type_hover(Gray);
    assert!(
        gray.contains("No file ending gives this type (picked by hand)")
            && !gray.contains("File name: none ("),
        "{gray}"
    );
}

#[test]
fn the_slot_labels_read_like_table_a() {
    use sdc_gui::model::labels::slot_type_label as l;
    use sdc_gui::model::sets::SlotKind::*;
    assert_eq!(l(Normal, false, false, false), "Normal map");
    assert_eq!(l(Normal, true, false, false), "PBR normal");
    assert_eq!(l(Diffuse, false, true, false), "Skin albedo");
    assert_eq!(l(Normal, false, true, false), "PBR normal");
    assert_eq!(l(Emissive, false, false, false), "Glow");
    assert_eq!(
        l(Subsurface, true, false, false),
        "PBR subsurface or coat colour"
    );
    assert_eq!(
        l(Rmaos, true, false, true),
        "Skin RFAOS (roughness, fuzz, AO, specular)"
    );
    assert_eq!(l(Height, true, false, false), "Height (parallax)");
}

#[test]
fn the_undecided_card_hovers_the_description_not_the_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut m = waiting_folder(dir.path());
    idle(&mut m);
    m.select_item(&key_of(&m, "rock_bump.png"));
    let mut h = harness(m);
    instant_tips(&h.ctx);
    h.run();
    h.run();
    assert!(h.query_by_label("What kind of texture is this?").is_some());
    // The combo announces its current value: the label of the type it starts on.
    let shown = h.get_by_label("Type for this texture").value();
    assert_eq!(
        shown.as_deref(),
        Some(sdc_gui::model::labels::subtype_name(engine::config::Subtype::GenericData).as_str())
    );
    let combo = h.get_by_label("Type for this texture").rect();
    h.get_by_label("Type for this texture").click();
    h.run();
    // The popup starts clipped: scroll the entry into view, then re-query it.
    entry_below(&h, "Environment mask", combo).scroll_to_me();
    h.run();
    entry_below(&h, "Environment mask", combo).hover();
    h.run();
    assert!(h
        .query_by_label_contains("A grey mask that sets where the cubemap shines.")
        .is_some());
    assert!(h.query_by_label("envmask").is_none());
}
