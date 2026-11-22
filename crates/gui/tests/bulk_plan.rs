//! M6 Track F3a D2/D5: the bulk plan and the panel's rows share one rule.
mod support;
use sdc_gui::model::bulk::{bulk_fields, plan_bulk_edit, BulkField, BulkOp, Change, NO_SETTINGS};
use sdc_gui::model::AppModel;
use serde_json::json;
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01.png";
const ODD: &str = "landscape/odd.png";
const NORMAL: &str = "landscape/rock01_normal.png";
const RAW: &str = "landscape/rock01_roughness.png";

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

fn strings(k: &[&str]) -> Vec<String> {
    k.iter().map(|s| s.to_string()).collect()
}

fn row(m: &AppModel, keys: &[String], field: &str) -> BulkField {
    bulk_fields(m, keys)
        .into_iter()
        .find(|f| f.field == field)
        .expect("a panel row")
}

#[test]
fn equal_values_show_and_differing_ones_are_mixed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path());
    let ks = strings(&[ROCK, ODD]);
    m.set_field(ROCK, "encoder.effort", json!("veryfast"));
    idle(&mut m);
    let r = row(&m, &ks, "encoder.effort");
    assert_eq!(
        (r.value.clone(), r.applies, r.edited, r.total),
        (None, 2, 1, 2)
    );
    assert_eq!(r.note(), "\u{2014} mixed · applies to 2 of 2");
    m.set_field(ODD, "encoder.effort", json!("veryfast"));
    idle(&mut m);
    let r = row(&m, &ks, "encoder.effort");
    assert_eq!((r.value.clone(), r.edited), (Some(json!("veryfast")), 2));
    assert_eq!(r.note(), "applies to 2 of 2");
}

#[test]
fn a_locked_field_applies_to_the_rest_and_says_why() {
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    let ks = strings(&[ROCK, NORMAL]);
    assert_eq!(
        plan_bulk_edit(
            &m,
            &ks,
            "mips.normal",
            &BulkOp::Set(json!("renormalize_bake"))
        ),
        vec![
            (ROCK.to_string(), Change::Skip("normal maps only".into())),
            (NORMAL.to_string(), Change::Write),
        ]
    );
    let r = row(&m, &ks, "mips.normal");
    assert_eq!((r.applies, r.total), (1, 2));
    assert_eq!(r.note(), "applies to 1 of 2");
    assert_eq!(r.skipped_text(), "locked on 1 of 2: normal maps only");
}

#[test]
fn a_texture_without_a_job_or_gone_from_the_plan_is_skipped() {
    let d = tempfile::tempdir().unwrap();
    let m = opened(d.path());
    assert!(
        m.item(RAW).is_some_and(|i| i.job.is_none()),
        "fixture: {RAW} is a raw source packed into rock01_rmaos"
    );
    // Review Focus 5: a key that left the plan between drawing and applying.
    let gone = "landscape/gone.png";
    let plan = plan_bulk_edit(
        &m,
        &strings(&[RAW, gone, ROCK]),
        "format",
        &BulkOp::Set(json!("bc1")),
    );
    assert_eq!(
        plan,
        vec![
            (RAW.to_string(), Change::Skip(NO_SETTINGS.into())),
            (gone.to_string(), Change::Skip(NO_SETTINGS.into())),
            (ROCK.to_string(), Change::Write),
        ]
    );
    let r = row(&m, &strings(&[RAW]), "format");
    assert_eq!((r.applies, r.value.clone()), (0, None));
    assert_eq!(r.note(), "applies to 0 of 1");
}

#[test]
fn a_barred_filter_skips_that_texture_on_a_write_only() {
    let d = tempfile::tempdir().unwrap();
    let m = pbr_opened(d.path());
    // Kaiser is for sRGB colour only (engine `filter::filter_allowed`): the normal map takes no write of it; a reset
    // reaches both. If pbr_albedo were ever barred too, use `opened()` with ROCK decided `Diffuse` and NORMAL `Normal`.
    let ks = strings(&[ROCK, NORMAL]);
    let set = plan_bulk_edit(&m, &ks, "mips.filter", &BulkOp::Set(json!("kaiser")));
    assert_eq!(set[0], (ROCK.to_string(), Change::Write));
    assert!(
        matches!(&set[1].1, Change::Skip(r) if !r.is_empty()),
        "{set:?}"
    );
    assert!(plan_bulk_edit(&m, &ks, "mips.filter", &BulkOp::Clear)
        .iter()
        .all(|(_, c)| *c == Change::Write));
}

#[test]
fn the_per_option_reach_equals_what_the_write_writes() {
    use engine::config::{ColorTag, GreenConvention, Subtype};
    use image::{ImageBuffer, Rgb};
    use sdc_gui::model::bulk::bulk_encodings;
    use sdc_gui::model::fields::{encoding_options, WEIGHTING, WEIGHTING_OPTIONS};
    use sdc_gui::model::Answer;
    use sdc_gui::settings::GuiSettings;
    // Three normals and a plain diffuse (no alpha channel), all with mipmaps on.
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    for n in ["a_n.png", "b_n.png", "c_n.png"] {
        ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
            .save(r.join(n))
            .unwrap();
    }
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, 90u8]))
        .save(r.join("plain.png"))
        .unwrap();
    let settings = GuiSettings {
        export_folder: Some(d.path().join("export")),
        ..GuiSettings::default()
    };
    let mut m = AppModel::new(settings, None, session());
    m.open_folder(d.path().join("textures"));
    idle(&mut m);
    choose_export(&mut m, d.path().join("export"));
    let keys = strings(&[
        "landscape/a_n.png",
        "landscape/b_n.png",
        "landscape/c_n.png",
        "landscape/plain.png",
    ]);
    for k in &keys[..3] {
        m.decide(k, Answer::Subtype(Subtype::Normal));
        m.decide(k, Answer::Green(GreenConvention::Directx));
    }
    m.decide(&keys[3], Answer::Subtype(Subtype::Diffuse));
    idle(&mut m);
    for k in &keys {
        m.set_field(k, "mips.levels", json!("full"));
    }
    idle(&mut m);
    let rows = bulk_fields(&m, &keys);
    // The candidate set, independent of the code under test.
    let tags: Vec<_> = keys
        .iter()
        .flat_map(|k| {
            let p = m.item(k).unwrap().job.as_ref().unwrap().profile.clone();
            encoding_options(m.target, &p)
                .into_iter()
                .filter(move |((f, t), _)| *f == p.format && *t != ColorTag::NotApplicable)
                .map(|((_, t), _)| json!(t))
        })
        .fold(vec![], |mut v, t| {
            if !v.contains(&t) {
                v.push(t);
            }
            v
        });
    assert!(tags.contains(&json!("unorm")), "{tags:?}");
    let candidates = |field: &str| -> Vec<serde_json::Value> {
        match field {
            f if f == sdc_gui::model::fields::FILTER_FIELD.0 => {
                ["box", "tent", "mitchell", "kaiser"]
                    .map(|f| json!(f))
                    .to_vec()
            }
            "format" => bulk_encodings(&m, &keys)
                .iter()
                .map(|(e, _)| json!(e))
                .collect(),
            "color.tag" => tags.clone(),
            _ => WEIGHTING_OPTIONS.iter().map(|o| json!(o.0)).collect(),
        }
    };
    for field in [
        sdc_gui::model::fields::FILTER_FIELD.0,
        "format",
        "color.tag",
        WEIGHTING,
    ] {
        let row = rows
            .iter()
            .find(|r| r.field == field)
            .expect("field in this selection");
        assert!(!row.option_reach.is_empty(), "{field}");
        let offered: Vec<_> = row.option_reach.iter().map(|o| o.1.clone()).collect();
        assert_eq!(
            offered,
            candidates(field),
            "{field}: every option is listed"
        );
        for (label, value, reach) in &row.option_reach {
            if field == "format" {
                assert_eq!(value.as_array().unwrap().len(), 2);
            }
            let plan = plan_bulk_edit(&m, &keys, field, &BulkOp::Set(value.clone()));
            assert_eq!(
                *reach,
                plan.iter().filter(|(_, c)| *c == Change::Write).count(),
                "{field} {label}"
            );
        }
    }
    let f = rows
        .iter()
        .find(|r| r.field == sdc_gui::model::fields::FILTER_FIELD.0)
        .unwrap();
    assert_eq!(f.note(), "Box: 4 of 4 · Tent, Mitchell, Kaiser: 1 of 4");
    // The reach differs between options, so the check above is not trivially equal.
    let w = rows.iter().find(|r| r.field == WEIGHTING).unwrap();
    assert!(w.option_reach.iter().any(|o| o.2 == 1) && w.option_reach.iter().any(|o| o.2 == 4));
}
