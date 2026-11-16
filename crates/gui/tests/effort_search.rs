//! Note JI4: the texture panel's Effort row offers what the texture's format reads and writes that field: the seven
//! levels (`encoder.effort`) on BC7 and BC6H, the radius on BC4 and BC5, the BC1 search on BC1, and a pair of both on
//! BC3. The bulk panel's Effort row follows the field every selected texture reads.
mod support;
use eframe::egui::{self, accesskit::Role, Rect};
use egui_kittest::kittest::{by, NodeT, Queryable};
use egui_kittest::Harness;
use engine::config::{ColorTag, Format, Subtype, Target};
use engine::policy::{Provenance, ResolvedJob};
use sdc_gui::model::fields::field_view;
use sdc_gui::model::{Answer, AppModel};
use serde_json::{json, Value};
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01.png";
const LEAF: &str = "landscape/leaf.png";
const RADIUS: &str = "encoder.bc45_search_radius";
const BC1: &str = "encoder.bc1_search";
/// The BC3 row's id (`fields::BC3_SEARCH`), spelt out so these tests compile before it exists.
const BC3: &str = "encoder.bc3_search";
const LEVELS: [&str; 7] = [
    "Ultrafast",
    "Veryfast",
    "Fast",
    "Basic",
    "Veryslow",
    "Slowest",
    "Max",
];

fn idle(m: &mut AppModel) {
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
}

fn file(m: &AppModel, k: &str, f: &str) -> Option<Value> {
    m.project.as_ref().unwrap().file_value(k, f)
}

fn format_of(m: &AppModel, k: &str) -> Format {
    m.item(k).unwrap().job.as_ref().unwrap().profile.format
}

/// `key` decided as a plain diffuse and pinned to `f` UNORM (preflight 4.1: Note JG's rule refuses other pairs).
fn pin(m: &mut AppModel, key: &str, f: Format) {
    m.decide(key, Answer::Subtype(Subtype::Diffuse));
    idle(m);
    m.set_format(key, f, ColorTag::Unorm);
    idle(m);
    assert_eq!(format_of(m, key), f, "fixture: {key} pinned to {f:?}");
}

fn cs(dir: &std::path::Path) -> AppModel {
    let mut m = opened(dir);
    m.set_target(Target::Cs);
    idle(&mut m);
    m
}

/// rock01 alone, pinned to `f` and selected.
fn single(dir: &std::path::Path, f: Format) -> AppModel {
    let mut m = cs(dir);
    pin(&mut m, ROCK, f);
    m.select_item(ROCK);
    m
}

/// rock01 and leaf pinned to `a` and `b`, both selected (the bulk panel).
fn pair(dir: &std::path::Path, a: Format, b: Format) -> AppModel {
    let mut m = cs(dir);
    pin(&mut m, ROCK, a);
    pin(&mut m, LEAF, b);
    m.click_item(ROCK, false, false);
    m.click_item(LEAF, true, false);
    m
}

/// Tall enough that the Compression group is on screen without scrolling.
fn harness(m: AppModel) -> Harness<'static, AppModel> {
    wide_harness(m, 1400.0)
}

fn wide_harness(m: AppModel, w: f32) -> Harness<'static, AppModel> {
    let mut h = Harness::builder()
        .with_context_setup(|ctx| {
            sdc_gui::fonts::install_system_fonts(ctx).expect("system fonts");
        })
        .with_size(egui::vec2(w, 1600.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    h.run();
    h
}

fn settle(h: &mut Harness<'static, AppModel>) {
    idle(h.state_mut());
    h.run();
}

fn centre(h: &Harness<'static, AppModel>) -> Rect {
    sdc_gui::ui::centre_rect(&h.ctx).expect("the centre is drawn")
}

/// The one combo named "Effort" in the centre.
fn effort(h: &Harness<'static, AppModel>) -> Rect {
    let c = centre(h);
    let found: Vec<Rect> = h
        .query_all(by().role(Role::ComboBox).label("Effort"))
        .map(|n| n.rect())
        .filter(|r| c.contains(r.center()))
        .collect();
    assert_eq!(found.len(), 1, "one Effort combo in the centre: {found:?}");
    found[0]
}

/// The Effort combo's shown value.
fn effort_value(h: &Harness<'static, AppModel>) -> Option<String> {
    let r = effort(h);
    h.query_all(by().role(Role::ComboBox).label("Effort"))
        .find(|n| n.rect() == r)
        .and_then(|n| n.accesskit_node().value())
}

/// Every named node, as (name, rect, disabled); a label's name is its value.
fn named(h: &Harness<'static, AppModel>) -> Vec<(String, Rect, bool)> {
    h.query_all(by())
        .filter_map(|n| {
            let a = n.accesskit_node();
            let name = if a.role() == Role::Label {
                a.value()
            } else {
                a.label()
            }?;
            Some((name, n.rect(), a.is_disabled()))
        })
        .collect()
}

/// Opens the Effort combo; its popup's entries, top to bottom (the nodes the click added).
fn open_effort(h: &mut Harness<'static, AppModel>) -> Vec<(String, Rect, bool)> {
    let before: Vec<(String, Rect)> = named(h).into_iter().map(|(n, r, _)| (n, r)).collect();
    let r = effort(h);
    h.query_all(by().role(Role::ComboBox).label("Effort"))
        .find(|n| n.rect() == r)
        .unwrap()
        .click();
    h.run();
    let mut new: Vec<_> = named(h)
        .into_iter()
        .filter(|(n, r, _)| !before.contains(&(n.clone(), *r)))
        .collect();
    new.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
    new.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    new
}

fn labels(entries: &[(String, Rect, bool)]) -> Vec<&str> {
    entries.iter().map(|e| e.0.as_str()).collect()
}

/// Clicks the open popup's entry `label`, then waits for the plan.
fn pick(h: &mut Harness<'static, AppModel>, entries: &[(String, Rect, bool)], label: &str) {
    let r = entries
        .iter()
        .find(|e| e.0 == label)
        .unwrap_or_else(|| panic!("{label:?} in {:?}", labels(entries)))
        .1;
    h.query_all(by().label(label))
        .find(|n| n.rect() == r)
        .unwrap()
        .click();
    h.run();
    settle(h);
}

#[test]
fn effort_offers_what_each_format_reads() {
    let three = ["Fast", "Default", "Max"];
    for (format, offered, writes) in [
        (Format::Bc7, &LEVELS[..], &["encoder.effort"][..]),
        (Format::Bc4, &three[..], &[RADIUS][..]),
        (Format::Bc5, &three[..], &[RADIUS][..]),
        (Format::Bc1, &["Fast", "Default"][..], &[BC1][..]),
        (Format::Bc3, &three[..], &[BC1, RADIUS][..]),
    ] {
        let d = tempfile::tempdir().unwrap();
        let mut h = harness(single(d.path(), format));
        let entries = open_effort(&mut h);
        assert_eq!(labels(&entries), offered, "{format:?}");
        pick(&mut h, &entries, "Fast");
        for f in writes {
            assert_eq!(
                file(h.state(), ROCK, f),
                Some(json!("fast")),
                "{format:?} {f}"
            );
        }
        if format != Format::Bc7 {
            // Spec 9.1: the same keys the old drawer rows wrote; `encoder.effort` is not touched.
            assert_eq!(file(h.state(), ROCK, "encoder.effort"), None, "{format:?}");
        }
    }
}

#[test]
fn bc3_choices_write_the_spec_pairs() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(single(d.path(), Format::Bc3));
    for (choice, label, bc1, radius) in [
        ("fast", "Fast", "fast", "fast"),
        ("default", "Default", "default", "default"),
        ("max", "Max", "default", "max"),
    ] {
        // A pair no choice names, so every pick changes both fields.
        h.state_mut().set_field(ROCK, BC1, json!("fast"));
        h.state_mut().set_field(ROCK, RADIUS, json!("max"));
        settle(&mut h);
        let entries = open_effort(&mut h);
        pick(&mut h, &entries, label);
        let m = h.state();
        assert_eq!(file(m, ROCK, BC1), Some(json!(bc1)), "{choice}");
        assert_eq!(file(m, ROCK, RADIUS), Some(json!(radius)), "{choice}");
        assert_eq!(m.undo_label(), Some("Change Effort"), "{choice}");
        assert_eq!(effort_value(&h).as_deref(), Some(label), "{choice}");
        // One step: one undo puts both fields back.
        h.state_mut().undo();
        settle(&mut h);
        assert_eq!(file(h.state(), ROCK, BC1), Some(json!("fast")), "{choice}");
        assert_eq!(
            file(h.state(), ROCK, RADIUS),
            Some(json!("max")),
            "{choice}"
        );
    }
}

#[test]
fn a_mixed_bc3_pair_shows_custom_and_a_pick_writes_both() {
    let d = tempfile::tempdir().unwrap();
    let mut m = single(d.path(), Format::Bc3);
    m.set_field(ROCK, BC1, json!("fast"));
    m.set_field(ROCK, RADIUS, json!("max"));
    idle(&mut m);
    let mut h = harness(m);
    const CUSTOM: &str = "Custom: search fast, radius max";
    assert_eq!(effort_value(&h).as_deref(), Some(CUSTOM));
    let entries = open_effort(&mut h);
    assert_eq!(labels(&entries), ["Fast", "Default", "Max", CUSTOM]);
    assert!(entries[3].2, "Custom is shown, never picked");
    assert!(entries[..3].iter().all(|e| !e.2), "{entries:?}");
    pick(&mut h, &entries, "Default");
    assert_eq!(file(h.state(), ROCK, BC1), Some(json!("default")));
    assert_eq!(file(h.state(), ROCK, RADIUS), Some(json!("default")));
    // ↺ on the Effort row (Format and DDS tag carry one too): it clears both fields in one step.
    let row = effort(&h);
    let reset = h
        .query_all(by().label("↺"))
        .map(|n| n.rect())
        .find(|r| r.min.y <= row.center().y && row.center().y <= r.max.y)
        .expect("a reset on the Effort row");
    h.query_all(by().label("↺"))
        .find(|n| n.rect() == reset)
        .unwrap()
        .click();
    h.run();
    settle(&mut h);
    assert_eq!(file(h.state(), ROCK, BC1), None);
    assert_eq!(file(h.state(), ROCK, RADIUS), None);
    assert_eq!(h.state().undo_label(), Some("Reset Effort"));
}

/// A BC3 texture's job, for the model-level views.
fn bc3_job(dir: &std::path::Path) -> ResolvedJob {
    let m = single(dir, Format::Bc3);
    m.item(ROCK).unwrap().job.clone().unwrap()
}

fn with_pair(job: &ResolvedJob, bc1: &str, radius: &str) -> ResolvedJob {
    let mut j = job.clone();
    j.profile.encoder.bc1_search = serde_json::from_value(json!(bc1)).unwrap();
    j.profile.encoder.bc45_search_radius = serde_json::from_value(json!(radius)).unwrap();
    j
}

#[test]
fn every_bc3_pair_maps_to_one_entry() {
    let d = tempfile::tempdir().unwrap();
    let job = bc3_job(d.path());
    for (bc1, radius, entry) in [
        ("fast", "fast", "fast"),
        ("default", "default", "default"),
        ("default", "max", "max"),
        ("fast", "default", "custom"),
        ("fast", "max", "custom"),
        ("default", "fast", "custom"),
    ] {
        let v = field_view(Target::Cs, &with_pair(&job, bc1, radius), BC3)
            .expect("the BC3 Effort view");
        assert_eq!(v.value, json!(entry), "({bc1}, {radius})");
        assert_eq!((v.field, v.label, v.lock), (BC3, "Effort", None));
    }
    // The row's `?` names the format it serves.
    let help = sdc_gui::model::fields::field_help(BC3).expect("BC3 Effort help");
    assert!(help.starts_with("How long the encoder searches."), "{help}");
    assert!(
        help.contains("On BC3 it sets both the BC1 colour search and the BC4 alpha radius."),
        "{help}"
    );
    for f in [RADIUS, BC1] {
        let h = sdc_gui::model::fields::field_help(f).unwrap();
        assert!(h.starts_with("How long the encoder searches."), "{f}: {h}");
    }
    // Off BC3 the row is locked, so a bulk write skips the texture with that reason.
    let mut bc7 = job.clone();
    bc7.profile.format = Format::Bc7;
    let v = field_view(Target::Cs, &bc7, BC3).expect("a locked view off BC3");
    assert_eq!(v.lock, Some("BC3 only"));
}

#[test]
fn bc3_provenance_follows_the_differing_field() {
    let d = tempfile::tempdir().unwrap();
    let mut job = bc3_job(d.path());
    job.provenance
        .insert(RADIUS.into(), Provenance::Preset("radius-preset".into()));
    job.provenance.insert(BC1.into(), Provenance::ProjectConfig);
    let prov = |bc1: &str, radius: &str| {
        field_view(Target::Cs, &with_pair(&job, bc1, radius), BC3)
            .expect("the BC3 Effort view")
            .provenance
    };
    assert_eq!(
        prov("default", "max"),
        "preset:radius-preset",
        "radius differs"
    );
    assert_eq!(
        prov("fast", "max"),
        "preset:radius-preset",
        "both differ: the radius"
    );
    assert_eq!(prov("fast", "default"), "project_config", "bc1 differs");
    assert_eq!(
        prov("default", "default"),
        "preset:radius-preset",
        "neither: the radius"
    );
    // Either field in the texture's own entry makes the row resettable.
    let v = field_view(Target::Cs, &with_pair(&job, "fast", "fast"), BC3).unwrap();
    assert!(!v.edited);
    job.provenance.insert(BC1.into(), Provenance::ProjectFile);
    let v = field_view(Target::Cs, &with_pair(&job, "fast", "fast"), BC3).unwrap();
    assert!(v.edited);
}

#[test]
fn an_uncompressed_texture_greys_effort() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(single(d.path(), Format::Rgba8));
    let r = effort(&h);
    let node = h
        .query_all(by().role(Role::ComboBox).label("Effort"))
        .find(|n| n.rect() == r)
        .unwrap();
    assert!(node.accesskit_node().is_disabled());
    let c = centre(&h);
    let hint: Vec<_> = named(&h)
        .into_iter()
        .filter(|(n, r, _)| n == "this format has no search setting" && c.contains(r.center()))
        .collect();
    assert_eq!(hint.len(), 1, "the row's hint says why");
}

#[test]
fn bulk_effort_follows_the_shared_field() {
    // Two BC4 textures: the radius's three choices, written to both in one step.
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pair(d.path(), Format::Bc4, Format::Bc4));
    h.get_by_label("2 textures selected");
    let entries = open_effort(&mut h);
    assert_eq!(labels(&entries), ["Fast", "Default", "Max"]);
    pick(&mut h, &entries, "Max");
    for k in [ROCK, LEAF] {
        assert_eq!(file(h.state(), k, RADIUS), Some(json!("max")), "{k}");
        assert_eq!(file(h.state(), k, "encoder.effort"), None, "{k}");
    }
    assert_eq!(h.state().undo_label(), Some("Change Effort on 2 textures"));

    // BC1 and BC3 read different fields (BC3 also the radius): no combo, the row says why.
    let d = tempfile::tempdir().unwrap();
    let h = harness(pair(d.path(), Format::Bc1, Format::Bc3));
    h.get_by_label("2 textures selected");
    let c = centre(&h);
    assert_eq!(
        h.query_all(by().role(Role::ComboBox).label("Effort"))
            .filter(|n| c.contains(n.rect().center()))
            .count(),
        0
    );
    let effort_y = h
        .query_all(by().role(Role::Label).label("Effort"))
        .map(|n| n.rect())
        .find(|r| c.contains(r.center()))
        .expect("the bulk Effort label")
        .center()
        .y;
    let differ = h
        .query_all(by().label("Formats differ: edit one format at a time"))
        .map(|n| n.rect())
        .find(|r| c.contains(r.center()))
        .expect("the formats-differ text");
    assert!(
        differ.min.y <= effort_y && effort_y <= differ.max.y,
        "on the Effort row"
    );

    // BC7 and RGBA8: the seven levels, for the one texture that reads them.
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pair(d.path(), Format::Bc7, Format::Rgba8));
    let row = effort(&h);
    assert!(
        named(&h).iter().any(|(n, r, _)| n == "applies to 1 of 2"
            && r.min.y <= row.center().y
            && row.center().y <= r.max.y),
        "the Effort row applies to 1 of 2"
    );
    let entries = open_effort(&mut h);
    assert_eq!(labels(&entries), LEVELS);

    // BC4 and RGBA8: the radius for the BC4 one; the panel and the write skip RGBA8 for the same reason.
    use sdc_gui::model::bulk::{bulk_fields, plan_bulk_edit, BulkOp, Change};
    let d = tempfile::tempdir().unwrap();
    let m = pair(d.path(), Format::Bc4, Format::Rgba8);
    let ks = vec![ROCK.to_string(), LEAF.to_string()];
    let row = bulk_fields(&m, &ks)
        .into_iter()
        .find(|r| r.label == "Effort")
        .unwrap();
    const NO_SEARCH: &str = "this format has no search setting";
    assert_eq!((row.field, row.applies), (RADIUS, 1));
    assert_eq!(row.skipped, [(NO_SEARCH.to_string(), 1)]);
    assert_eq!(
        plan_bulk_edit(&m, &ks, RADIUS, &BulkOp::Set(json!("max"))),
        [
            (ROCK.to_string(), Change::Write),
            (LEAF.to_string(), Change::Skip(NO_SEARCH.into()))
        ]
    );
}

/// Two BC3 textures: a bulk pick writes both fields on both, as one step named after the row.
#[test]
fn a_bulk_bc3_pick_writes_both_fields_on_every_texture() {
    let d = tempfile::tempdir().unwrap();
    let mut h = harness(pair(d.path(), Format::Bc3, Format::Bc3));
    h.get_by_label("2 textures selected");
    let entries = open_effort(&mut h);
    assert_eq!(labels(&entries), ["Fast", "Default", "Max"]);
    pick(&mut h, &entries, "Max");
    for k in [ROCK, LEAF] {
        assert_eq!(file(h.state(), k, BC1), Some(json!("default")), "{k}");
        assert_eq!(file(h.state(), k, RADIUS), Some(json!("max")), "{k}");
    }
    assert_eq!(h.state().undo_label(), Some("Change Effort on 2 textures"));
}

/// Review fix 1: two BC3 textures holding different Custom pairs read "— mixed", not one shared Custom value; the
/// same Custom pair on both is shared.
#[test]
fn different_custom_bc3_pairs_read_mixed_in_bulk() {
    use sdc_gui::model::bulk::bulk_fields;
    let d = tempfile::tempdir().unwrap();
    let mut m = pair(d.path(), Format::Bc3, Format::Bc3);
    m.set_field(ROCK, BC1, json!("fast"));
    m.set_field(ROCK, RADIUS, json!("max"));
    m.set_field(LEAF, BC1, json!("default"));
    m.set_field(LEAF, RADIUS, json!("fast"));
    idle(&mut m);
    let ks = vec![ROCK.to_string(), LEAF.to_string()];
    let effort = |m: &AppModel| {
        bulk_fields(m, &ks)
            .into_iter()
            .find(|r| r.label == "Effort")
            .unwrap()
    };
    let row = effort(&m);
    assert_eq!((row.field, row.value.clone(), row.applies), (BC3, None, 2));
    assert_eq!(row.note(), "— mixed");
    m.set_field(LEAF, BC1, json!("fast"));
    m.set_field(LEAF, RADIUS, json!("max"));
    idle(&mut m);
    assert_eq!(effort(&m).value, Some(json!("custom")), "one shared pair");
    let h = harness(m);
    assert_eq!(
        effort_value(&h).as_deref(),
        Some("Custom: search fast, radius max")
    );
}

/// Review fix 2: ↺ on the bulk Effort row of two BC3 textures clears both fields on both, as one step.
#[test]
fn a_bulk_bc3_reset_clears_both_fields_on_every_texture() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pair(d.path(), Format::Bc3, Format::Bc3);
    for k in [ROCK, LEAF] {
        m.set_field(k, BC3, json!("max"));
    }
    idle(&mut m);
    let mut h = harness(m);
    let row = effort(&h);
    let reset = h
        .query_all(by().label("↺"))
        .map(|n| n.rect())
        .find(|r| r.min.y <= row.center().y && row.center().y <= r.max.y)
        .expect("a reset on the bulk Effort row");
    h.query_all(by().label("↺"))
        .find(|n| n.rect() == reset)
        .unwrap()
        .click();
    h.run();
    settle(&mut h);
    for k in [ROCK, LEAF] {
        assert_eq!(file(h.state(), k, BC1), None, "{k}");
        assert_eq!(file(h.state(), k, RADIUS), None, "{k}");
    }
    assert_eq!(h.state().undo_label(), Some("Reset Effort on 2 textures"));
}

/// The bulk Effort row's choices come from a texture that reads its field, not from the active one: a mixed BC3
/// pair beside an active RGBA8 texture still shows its Custom entry.
#[test]
fn bulk_effort_options_come_from_a_texture_that_reads_the_field() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pair(d.path(), Format::Bc3, Format::Rgba8);
    m.set_field(ROCK, BC1, json!("fast"));
    m.set_field(ROCK, RADIUS, json!("max"));
    idle(&mut m);
    assert!(
        matches!(&m.selection, sdc_gui::model::Selection::Item(k) if k == LEAF),
        "fixture: the RGBA8 texture is the active one"
    );
    let h = harness(m);
    assert_eq!(
        effort_value(&h).as_deref(),
        Some("Custom: search fast, radius max")
    );
}

/// Rule 3.1: the bulk row's "Formats differ" text stays inside the centre, with every note and ↺ of its group.
#[test]
fn formats_differ_stays_inside_the_centre() {
    for w in [1100.0, 1280.0] {
        let d = tempfile::tempdir().unwrap();
        let h = wide_harness(pair(d.path(), Format::Bc1, Format::Bc3), w);
        let c = centre(&h);
        let differ = h
            .query_all(by().label("Formats differ: edit one format at a time"))
            .map(|n| n.rect())
            .find(|r| c.contains(r.center()))
            .expect("the formats-differ text");
        assert!(differ.max.x <= c.max.x, "at {w}: {differ:?} past {c:?}");
        let below = named(&h)
            .into_iter()
            .filter(|(_, r, _)| r.min.y >= differ.min.y - 1.0 && c.contains(r.center()));
        for (n, r, _) in below {
            assert!(r.max.x <= c.max.x + 0.5, "at {w}: {n:?} {r:?} past {c:?}");
        }
    }
}

#[test]
fn every_bulk_combo_has_its_row_name() {
    let d = tempfile::tempdir().unwrap();
    let h = harness(pair(d.path(), Format::Bc7, Format::Bc7));
    h.get_by_label("2 textures selected");
    let c = centre(&h);
    let rows: Vec<&str> = sdc_gui::model::bulk::panel_fields()
        .into_iter()
        .map(|(_, l)| l)
        .collect();
    let names: Vec<Option<String>> = h
        .query_all_by_role(Role::ComboBox)
        .filter(|n| c.contains(n.rect().center()))
        .map(|n| n.accesskit_node().label())
        .collect();
    assert!(names.len() >= 5, "the bulk combos: {names:?}");
    for n in &names {
        assert!(
            n.as_deref().is_some_and(|n| rows.contains(&n)),
            "every bulk combo is named after its row: {names:?}"
        );
    }
}
