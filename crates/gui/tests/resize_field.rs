//! A texture's own `resize` and its Size floor (Note FM29), and check 3's Problems rows. Review never asks the
//! half-size question (Note FM).
mod support;
use eframe::egui;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use engine::codes::Code;
use image::{ImageBuffer, Rgb, RgbImage};
use sdc_gui::model::bottom::{problems, At, Level};
use sdc_gui::model::{AppModel, Dialog};
use sdc_gui::settings::GuiSettings;
use std::path::{Path, PathBuf};
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(120);

/// `n`² RGB white noise from a fixed LCG.
fn noise(n: u32, seed: u64) -> RgbImage {
    let mut s = seed;
    ImageBuffer::from_fn(n, n, |_, _| {
        let mut b = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (s >> 56) as u8
        };
        Rgb([b(), b(), b()])
    })
}

/// 512² noise upscaled 2x (nearest): mip 0 adds nothing over mip 1.
fn upscaled(seed: u64) -> RgbImage {
    let small = noise(512, seed);
    ImageBuffer::from_fn(1024, 1024, |x, y| *small.get_pixel(x / 2, y / 2))
}

/// `textures/` with `up.png`, `up2.png` (flagged) and `noise.png` (native 1024²), all diffuse, encoded ultrafast.
fn folder(dir: &Path) -> PathBuf {
    let r = dir.join("textures");
    std::fs::create_dir_all(&r).unwrap();
    noise(1024, 1).save(r.join("noise.png")).unwrap();
    upscaled(2).save(r.join("up.png")).unwrap();
    upscaled(3).save(r.join("up2.png")).unwrap();
    let mut toml = String::from("[defaults.encoder]\neffort = \"ultrafast\"\n");
    for n in ["noise.png", "up.png", "up2.png"] {
        toml += &format!("\n[files.\"{n}\"]\nsubtype = \"diffuse\"\n");
    }
    std::fs::write(r.join("sdc.toml"), toml).unwrap();
    r
}

fn opened(dir: &Path) -> AppModel {
    let root = folder(dir);
    let settings = GuiSettings {
        export_folder: Some(dir.join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, support::session());
    m.open_folder(root);
    assert!(m.wait_idle(WAIT), "planning did not finish");
    support::choose_export(&mut m, dir.join("export"));
    m
}

/// An export with no Review: nothing is undecided, and the check's warnings never open it.
fn export(m: &mut AppModel) {
    m.export_clicked();
    assert!(m.review.is_none(), "the fixture needs no Review");
    assert!(m.wait_idle(WAIT));
}

fn job(m: &AppModel, item: &str) -> String {
    m.item(item).unwrap().job_key.clone()
}

/// The written DDS of `item`: (width, height).
fn dims(m: &AppModel, item: &str) -> (u32, u32) {
    let p = m.export_root().join(job(m, item));
    let b = std::fs::read(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
    let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    (u(16), u(12))
}

fn own(m: &AppModel, item: &str, field: &str) -> Option<serde_json::Value> {
    m.project.as_ref().unwrap().file_value(item, field)
}

/// The whole app, polled every frame as the real app does.
fn harness(model: AppModel) -> Harness<'static, AppModel> {
    let h = Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, model: &mut AppModel| {
                model.poll();
                sdc_gui::ui::draw(ui, model)
            },
            model,
        );
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h
}

/// Copy settings lists `resize` and `resize_floor` as ticks like any other field.
#[test]
fn copy_settings_ticks_resize_and_its_floor() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_field("up.png", "resize", serde_json::json!("quarter"));
    m.set_field("up.png", "resize_floor", serde_json::json!(256));
    assert!(m.wait_idle(WAIT));
    m.click_item("noise.png", false, false);
    m.click_item("up2.png", true, false);
    m.click_item("up.png", true, false);
    m.open_copy_settings();
    let fields = match &m.dialog {
        Some(Dialog::CopySettings { fields, .. }) => fields.clone(),
        d => panic!("{d:?}"),
    };
    for f in ["resize", "resize_floor"] {
        assert!(fields.contains(&(f.to_string(), true)), "{fields:?}");
    }
    m.apply_copy_settings(
        "up.png",
        &["up2.png".to_string(), "noise.png".to_string()],
        &["resize".to_string(), "resize_floor".to_string()],
    );
    assert_eq!(m.dialog, None);
    for item in ["up2.png", "noise.png"] {
        assert_eq!(own(&m, item, "resize"), Some(serde_json::json!("quarter")));
        assert_eq!(own(&m, item, "resize_floor"), Some(serde_json::json!(256)));
    }
    assert_eq!(m.undo_label(), Some("Copy settings from up.png"));
}

/// `resize = "half"` on the texture's own settings writes the smaller file; Undo removes the key in one step.
#[test]
fn a_saved_half_resize_writes_the_smaller_file_and_undo_removes_it_in_one_step() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_field("up.png", "resize", serde_json::json!("half"));
    m.set_field("up2.png", "resize", serde_json::json!("half"));
    assert!(m.wait_idle(WAIT));
    export(&mut m);
    assert_eq!(dims(&m, "up.png"), (512, 512));
    assert_eq!(dims(&m, "up2.png"), (512, 512));
    assert_eq!(dims(&m, "noise.png"), (1024, 1024));
    m.undo();
    assert!(m.wait_idle(WAIT));
    assert_eq!(own(&m, "up2.png", "resize"), None);
}

#[test]
fn the_cli_command_reproduces_the_resized_output() {
    use engine::api::*;
    use engine::events::{new_batch_id, Reporter, VecSink};
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    m.set_field("up.png", "resize", serde_json::json!("half"));
    assert!(m.wait_idle(WAIT));
    export(&mut m);
    let root = m.root.clone().unwrap();
    let mut req = BatchRequest::new(
        vec![root.clone()],
        d.path().join("cli"),
        engine::config::Target::Cs,
    );
    req.recursive = true;
    req.accept = engine::policy::AcceptSuggestions::High;
    req.project_config = Some(root.join("sdc.toml"));
    let sink = std::sync::Arc::new(VecSink::default());
    let rep = Reporter::new(sink, new_batch_id());
    let e = Engine::new().unwrap();
    let plan = e.plan(req, &rep, &CancelToken::default()).unwrap();
    let up = plan
        .jobs
        .iter()
        .find(|j| j.input.ends_with("up.png"))
        .unwrap();
    assert_eq!(up.profile.resize, engine::config::ResizeMode::Half);
    e.run(plan, &rep, &CancelToken::default()).unwrap();
    for item in ["up.png", "up2.png"] {
        let k = job(&m, item);
        assert_eq!(
            std::fs::read(d.path().join("cli").join(&k)).unwrap(),
            std::fs::read(m.export_root().join(&k)).unwrap(),
            "{item}"
        );
    }
}

/// A flagged texture is written at full size and listed in Problems; Review is never asked (Note FM).
#[test]
fn flagged_textures_export_full_and_are_listed() {
    use sdc_gui::model::bottom::{filtered, ProblemsFilter};
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    export(&mut m);
    assert_eq!(m.dialog, None);
    assert_eq!(dims(&m, "up.png"), (1024, 1024));
    let k = job(&m, "up.png");
    let row = m.export.rows.iter().find(|r| r.job_key == k).unwrap();
    assert!(row.warnings.iter().any(|w| w.code == Code::ChkLowDetail));
    // K2 / M9 (T2): the engine's CHK row is a check row, counted by the Checks filter and once per texture.
    let low = |m: &AppModel| {
        problems(m)
            .iter()
            .filter(|p| p.code == Some(Code::ChkLowDetail))
            .map(|p| (p.level, p.at.clone()))
            .collect::<Vec<_>>()
    };
    let rows = low(&m);
    assert!(
        rows.contains(&(Level::Check, At::File(k.clone()))),
        "{rows:?}"
    );
    assert_eq!(rows.len(), 2, "once per texture (up, up2): {rows:?}");
    assert!(rows.iter().all(|r| r.0 == Level::Check));
    assert!(filtered(&m, Some(&ProblemsFilter::Checks))
        .iter()
        .any(|p| p.code == Some(Code::ChkLowDetail)));
    // M3: the header Export dot reads `ready_rows`; turning the check off removes the 2 CHK rows and must leave
    // `ready_rows` unchanged (the fixture has other, real warnings, so the dot itself is not green here).
    let on = sdc_gui::model::bottom::flagged(&m);
    let summary_count = |m: &AppModel| -> usize {
        sdc_gui::model::checks::checks_summary(m)
            .and_then(|s| s.split_whitespace().nth(1)?.parse().ok())
            .unwrap_or(0)
    };
    let n_on = summary_count(&m);
    let mut off = m.settings.checks;
    off.low_detail = false;
    m.set_checks(off);
    m.poll();
    assert!(low(&m).is_empty());
    let f_off = sdc_gui::model::bottom::flagged(&m);
    assert_eq!(
        f_off.ready_rows, on.ready_rows,
        "a CHK row never counts as ready"
    );
    assert_eq!(f_off.check_rows + 2, on.check_rows);
    assert_eq!(summary_count(&m) + 2, n_on, "the summary counts each once");
}

/// The size estimate follows the engine: the power of two first, then `resize` down to the floor.
#[test]
fn the_estimate_follows_resize_and_its_floor() {
    use engine::config::{Profile, ResizeMode};
    use sdc_gui::model::out_size::estimate;
    let d = tempfile::tempdir().unwrap();
    let m = opened(d.path());
    let mut p: Profile = m.item("up.png").unwrap().job.clone().unwrap().profile;
    let full = estimate((1000, 1000), &p);
    assert_eq!((full.width, full.height), (1024, 1024));
    p.resize = ResizeMode::Eighth;
    let small = estimate((1024, 1024), &p);
    assert_eq!((small.width, small.height), (512, 512), "the 512 floor");
    assert_eq!(small.chain_bytes, estimate((512, 512), &p).chain_bytes);
    p.resize_floor = 128;
    assert_eq!(estimate((1024, 1024), &p).width, 128);
}

/// Note FM44: the Advanced drawer's "Relative to" row sits after Resize, offers own size and slots 1 to 8 by name, is
/// never locked (so Copy settings carries it onto a texture whose resize is still None), and the size estimate
/// follows the planned reference.
#[test]
fn the_size_row_offers_relative_to_slots() {
    use engine::config::{ResizeMode, Target};
    use sdc_gui::model::fields::{advanced_field_views, ADVANCED_FIELDS};
    use sdc_gui::model::out_size::estimate;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let at = |f: &str| ADVANCED_FIELDS.iter().position(|(g, _)| *g == f).unwrap();
    assert_eq!(at("resize_relative_to"), at("resize") + 1);
    let view = |m: &AppModel, item: &str| {
        let j = m.item(item).unwrap().job.clone().unwrap();
        advanced_field_views(m.target, &j)
            .into_iter()
            .find(|v| v.field == "resize_relative_to")
            .unwrap()
    };
    let v = view(&m, "up.png");
    assert_eq!((v.value, v.lock), (serde_json::json!("self"), None));
    let p = m.item("up.png").unwrap().job.clone().unwrap().profile;
    let opts = sdc_gui::ui::presets::field_options("resize_relative_to", Target::Cs, &p);
    assert_eq!(opts.len(), 9, "{opts:?}");
    assert_eq!(opts[0], (serde_json::json!("self"), "Own size".to_string()));
    assert_eq!(opts[1].0, serde_json::json!("slot1"));
    assert!(opts[1].1.starts_with("Slot 1 · Diffuse"), "{opts:?}");
    assert!(opts[8].1.starts_with("Slot 8 · "), "{opts:?}");
    m.set_field("up.png", "resize", serde_json::json!("half"));
    m.set_field("up.png", "resize_relative_to", serde_json::json!("slot1"));
    assert!(m.wait_idle(WAIT));
    let v = view(&m, "up.png");
    assert_eq!(
        (v.value.clone(), v.lock),
        (serde_json::json!("slot1"), None)
    );
    assert!(v.edited);
    m.settings.advanced_open = true;
    m.select_item("up.png");
    let mut h = harness(m);
    h.run();
    h.get_by_label("Relative to");
    let mut m = h.into_state();
    // Copy settings carries it.
    m.click_item("up2.png", false, false);
    m.click_item("up.png", true, false);
    m.open_copy_settings();
    match &m.dialog {
        Some(Dialog::CopySettings { fields, .. }) => {
            assert!(
                fields.contains(&("resize_relative_to".to_string(), true)),
                "{fields:?}"
            )
        }
        d => panic!("{d:?}"),
    }
    m.apply_copy_settings(
        "up.png",
        &["up2.png".to_string()],
        &["resize_relative_to".to_string()],
    );
    assert_eq!(
        own(&m, "up2.png", "resize_relative_to"),
        Some(serde_json::json!("slot1"))
    );
    // The estimate reads the planned reference: half of a 512 reference over a 128 floor is 256.
    let mut p = p;
    p.resize = ResizeMode::Half;
    p.resize_floor = 128;
    assert_eq!(estimate((1024, 1024), &p).width, 512);
    p.resize_reference = Some((512, 512));
    assert_eq!(estimate((1024, 1024), &p).width, 256);
}

/// Note FM44: the engine's slot table agrees with the set editor's slot cards wherever a sub-type has a card.
#[test]
fn the_engine_slot_table_matches_the_slot_cards() {
    for st in engine::config::Subtype::ALL {
        if let Some(k) = sdc_gui::model::sets::slot_kind(st) {
            assert_eq!(k.slot_number(), st.txst_slot(), "{st:?}");
        }
    }
}

/// The Advanced drawer has a Size floor row showing the default, then the texture's own value; Clear removes the key.
#[test]
fn the_size_floor_row_follows_resize_and_clear_removes_it() {
    use sdc_gui::model::fields::advanced_field_views;
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let floor = |m: &AppModel| {
        let j = m.item("up.png").unwrap().job.clone().unwrap();
        advanced_field_views(m.target, &j)
            .into_iter()
            .find(|v| v.field == "resize_floor")
            .unwrap()
    };
    let v = floor(&m);
    assert_eq!(v.value, serde_json::json!(512));
    assert_eq!(v.lock, None);
    m.set_field("up.png", "resize", serde_json::json!("half"));
    m.set_field("up.png", "resize_floor", serde_json::json!(256));
    assert!(m.wait_idle(WAIT));
    let v = floor(&m);
    assert_eq!((v.value.clone(), v.lock), (serde_json::json!(256), None));
    assert!(v.edited, "Clear (↺) shows for the texture's own value");
    m.settings.advanced_open = true;
    m.select_item("up.png");
    let mut h = harness(m);
    h.run();
    h.get_by_label("Size floor");
    let mut m = h.into_state();
    m.clear_field("up.png", "resize_floor");
    assert!(m.wait_idle(WAIT));
    assert_eq!(m.undo_label(), Some("Reset resize_floor"));
    assert_eq!(own(&m, "up.png", "resize_floor"), None);
}
