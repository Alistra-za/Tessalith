//! Note HF9: Export set, Export group, Export N sets and Exclude from export. A scoped export writes the chosen sets
//! and the sets they borrow from; an excluded group's sets are left out of a whole export. Files of sets outside the
//! run stay as they are on disk, and their per-set JSON and PGPatcher entries stay while their files exist.
mod support;
use engine::config::Target;
use engine::presets::AlsoTarget;
use image::{ImageBuffer, Rgb, Rgba};
use sdc_gui::model::bulk::MenuAction;
use sdc_gui::model::cs_pbr::Kind;
use sdc_gui::model::review::JsonKind;
use sdc_gui::model::set_groups::GroupTarget;
use sdc_gui::model::sets::{set_views, DropChoice, DropOutcome};
use sdc_gui::model::{Answer, AppModel, Dialog};
use sdc_gui::settings::GuiSettings;
use serde_json::json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use support::*;

const WAIT: Duration = Duration::from_secs(120);

fn idle(m: &mut AppModel) {
    assert!(m.wait_idle(WAIT), "the model did not settle");
}

fn colour(path: &Path, tint: u8) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([(x * 4) as u8, (y * 4) as u8, tint]))
        .save(path)
        .unwrap();
}

fn flat(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(64, 64, |_, _| Rgb([128u8, 128, 255]))
        .save(path)
        .unwrap();
}

/// A colour map whose right half is transparent: its alpha needs a decision.
fn half_alpha(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| {
        Rgba([
            (x * 4) as u8,
            (y * 4) as u8,
            90,
            if x < 32 { 255 } else { 0 },
        ])
    })
    .save(path)
    .unwrap();
}

/// `textures/g`: a ready set `g/<n>` per `ready` (a colour map and a flat `_n`), and a set per `warn` whose colour
/// map has an alpha that needs a decision. Opened as the app opens a folder, exports under `<dir>/export`.
fn plain(dir: &Path, ready: &[&str], warn: &[&str]) -> AppModel {
    let g = dir.join("textures/g");
    for n in ready {
        colour(&g.join(format!("{n}.png")), 90);
        flat(&g.join(format!("{n}_n.png")));
    }
    for n in warn {
        half_alpha(&g.join(format!("{n}.png")));
        flat(&g.join(format!("{n}_n.png")));
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    idle(&mut m);
    m
}

/// `textures/pbr/arm`: PBR sets `steel` and `iron` (colour, `_n`, `_rmaos`).
fn pbr(dir: &Path) -> AppModel {
    pbr_with(dir, true)
}

/// `pbr`; without `iron_rmaos` unless `iron_rmaos`.
fn pbr_with(dir: &Path, iron_rmaos: bool) -> AppModel {
    let t = dir.join("textures/pbr/arm");
    for (n, tint) in [("steel", 200u8), ("iron", 120)] {
        colour(&t.join(format!("{n}.png")), tint);
        flat(&t.join(format!("{n}_n.png")));
        if n == "steel" || iron_rmaos {
            colour(&t.join(format!("{n}_rmaos.png")), 30);
        }
    }
    let mut m = opened_as_the_app(dir.join("textures"), dir.join("export"));
    idle(&mut m);
    m
}

fn key(m: &AppModel, id: &str) -> String {
    set_views(m)
        .iter()
        .find(|v| v.id == id)
        .unwrap_or_else(|| panic!("no set {id}"))
        .key
        .clone()
}

fn id_ending(m: &AppModel, end: &str) -> String {
    set_views(m)
        .iter()
        .find(|v| v.id.ends_with(end))
        .unwrap_or_else(|| panic!("no set ending {end}"))
        .id
        .clone()
}

fn keys(m: &AppModel, ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|i| key(m, i)).collect()
}

fn group(m: &mut AppModel, name: &str, ids: &[&str]) {
    let ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
    m.move_to_group(&ids, GroupTarget::New(name.into()))
        .expect("grouped");
}

/// Share `item` into set `to`, replacing its own occupant.
fn share(m: &mut AppModel, item: &str, to: &str) {
    if matches!(
        m.drop_choice(item, to, DropChoice::Share),
        DropOutcome::AskReplace { .. }
    ) {
        m.drop_replace(item, to, DropChoice::Share).unwrap();
    }
    idle(m);
}

/// Through Review when it opens, as a user would, then wait for the export.
fn finish(m: &mut AppModel) {
    if m.review.is_some() {
        m.export_from_review();
    }
    idle(m);
}

fn export_sets(m: &mut AppModel, ids: &[&str], label: &str) {
    let k = keys(m, ids);
    m.export_sets(k, label.into());
    finish(m);
}

fn whole(m: &mut AppModel) {
    idle(m);
    export_through(m);
    idle(m);
}

fn out(m: &AppModel, rel: &str) -> PathBuf {
    m.export_root().join(rel)
}

fn notes(m: &AppModel) -> String {
    m.export.notes.join("\n")
}

fn stamp(p: &Path) -> (Vec<u8>, SystemTime) {
    (
        std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
        std::fs::metadata(p).unwrap().modified().unwrap(),
    )
}

fn pgp_text(m: &AppModel) -> String {
    std::fs::read_to_string(pgp_json_path(m)).unwrap_or_default()
}

// ---- What a scope writes ----

#[test]
fn export_set_writes_only_that_set_and_its_borrow_host() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "h", "u"], &[]);
    share(&mut m, "g/h_n.png", "g/u");
    export_sets(&mut m, &["g/u"], "Export set g/u");
    assert!(out(&m, "textures/g/u.dds").is_file(), "{}", notes(&m));
    // The host comes in as a whole set.
    assert!(out(&m, "textures/g/h.dds").is_file());
    assert!(out(&m, "textures/g/h_n.dds").is_file());
    assert!(!out(&m, "textures/g/a.dds").exists(), "a is not in the run");
    assert!(m.export_scope().is_none(), "the export ended the scope");
}

#[test]
fn export_group() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "b", "c"], &[]);
    group(&mut m, "Armor", &["g/a", "g/b"]);
    m.export_group("armor");
    finish(&mut m);
    assert!(out(&m, "textures/g/a.dds").is_file(), "{}", notes(&m));
    assert!(out(&m, "textures/g/b.dds").is_file());
    assert!(!out(&m, "textures/g/c.dds").exists());
}

#[test]
fn export_n_sets() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "b", "c"], &[]);
    let (a, c) = (key(&m, "g/a"), key(&m, "g/c"));
    m.click_set(&a, false, false);
    m.click_set(&c, true, false);
    let entry = m
        .set_menu()
        .into_iter()
        .find(|e| e.action == MenuAction::ExportSets)
        .expect("the Export N sets entry");
    assert_eq!(entry.label, "Export 2 sets");
    assert!(m.run_menu(entry.action).is_none());
    finish(&mut m);
    assert!(out(&m, "textures/g/a.dds").is_file(), "{}", notes(&m));
    assert!(out(&m, "textures/g/c.dds").is_file());
    assert!(!out(&m, "textures/g/b.dds").exists());
}

// ---- When it starts ----

#[test]
fn a_clean_scoped_export_starts_at_once() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "b"], &[]);
    let k = keys(&m, &["g/a"]);
    m.export_sets(k, "Export set g/a".into());
    assert!(m.review.is_none(), "no Review for a clean scope");
    assert!(m.export.running.is_some() || m.export_queued());
    idle(&mut m);
    assert!(out(&m, "textures/g/a.dds").is_file(), "{}", notes(&m));
}

#[test]
fn others_problems_do_not_block() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a"], &["w"]);
    m.export_clicked();
    assert!(m.review.is_some(), "the whole export stops at w's question");
    m.close_review();
    let k = keys(&m, &["g/a"]);
    m.export_sets(k, "Export set g/a".into());
    assert!(m.review.is_none(), "w is outside the run");
    idle(&mut m);
    assert!(out(&m, "textures/g/a.dds").is_file(), "{}", notes(&m));
    assert!(!out(&m, "textures/g/w.dds").exists());
}

#[test]
fn its_own_warning_still_stops_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a"], &["w"]);
    let k = keys(&m, &["g/w"]);
    m.export_sets(k, "Export set g/w".into());
    assert!(m.review.is_some(), "w's own question opens Review");
    assert!(m.export.running.is_none());
    assert_eq!(
        m.export_scope().map(|s| s.label.as_str()),
        Some("Export set g/w")
    );
}

#[test]
fn whole_export_with_nothing_excluded_sends_none() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "b"], &[]);
    assert_eq!(m.write_only(), None);
    group(&mut m, "Armor", &["g/a"]);
    assert_eq!(m.write_only(), None, "a group alone leaves nothing out");
    m.set_group_excluded("Armor", true).unwrap();
    assert_eq!(m.write_only(), Some(keys(&m, &["g/b"])));
    m.set_group_excluded("Armor", false).unwrap();
    assert_eq!(m.write_only(), None);
}

// ---- Exclude ----

#[test]
fn an_excluded_group_is_left_alone() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "b"], &[]);
    whole(&mut m);
    let a = out(&m, "textures/g/a.dds");
    let before = stamp(&a);
    group(&mut m, "Armor", &["g/a"]);
    m.set_group_excluded("Armor", true).unwrap();
    colour(&d.path().join("textures/g/a.png"), 10);
    colour(&d.path().join("textures/g/b.png"), 10);
    m.replan();
    whole(&mut m);
    assert_eq!(stamp(&a), before, "a.dds untouched");
    assert!(out(&m, "textures/g/b.dds").is_file(), "{}", notes(&m));
}

#[test]
fn an_excluded_groups_json_entry_is_kept() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr(d.path());
    whole(&mut m);
    assert!(pgp_text(&m).contains("iron"), "{}", notes(&m));
    group(&mut m, "Iron", &["pbr/arm/iron"]);
    m.set_group_excluded("Iron", true).unwrap();
    whole(&mut m);
    let j = pgp_text(&m);
    assert!(
        j.contains("iron") && j.contains("steel"),
        "{j}\n{}",
        notes(&m)
    );
}

#[test]
fn export_group_on_an_excluded_group_works_and_keeps_the_flag() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "b"], &[]);
    group(&mut m, "Armor", &["g/a"]);
    m.set_group_excluded("Armor", true).unwrap();
    m.export_group("Armor");
    finish(&mut m);
    assert!(out(&m, "textures/g/a.dds").is_file(), "{}", notes(&m));
    assert!(!out(&m, "textures/g/b.dds").exists());
    assert!(m.group_of("g/a").unwrap().excluded, "the flag stays");
}

#[test]
fn a_borrower_of_an_excluded_host_stays_in_the_json_and_the_host_is_untouched() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_with(d.path(), false);
    share(&mut m, "pbr/arm/steel_rmaos.png", "pbr/arm/iron");
    whole(&mut m);
    let host = out(&m, "textures/pbr/arm/steel.dds");
    let before = stamp(&host);
    group(&mut m, "Steel", &["pbr/arm/steel"]);
    m.set_group_excluded("Steel", true).unwrap();
    colour(&d.path().join("textures/pbr/arm/steel.png"), 7);
    m.replan();
    whole(&mut m);
    assert_eq!(stamp(&host), before, "the host is left alone");
    assert!(pgp_text(&m).contains("iron"), "{}", notes(&m));
    // Export set on the borrower does not pull in an excluded host either.
    export_sets(&mut m, &["pbr/arm/iron"], "Export set pbr/arm/iron");
    assert_eq!(stamp(&host), before);
}

// ---- The PGPatcher JSON on a partial run ----

#[test]
fn the_json_keeps_a_set_outside_the_run_while_its_files_exist() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr(d.path());
    whole(&mut m);
    export_sets(&mut m, &["pbr/arm/steel"], "Export set pbr/arm/steel");
    let j = pgp_text(&m);
    assert!(
        j.contains("iron") && j.contains("steel"),
        "{j}\n{}",
        notes(&m)
    );
}

#[test]
fn one_never_exported_stays_out_with_a_note() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr(d.path());
    export_sets(&mut m, &["pbr/arm/steel"], "Export set pbr/arm/steel");
    let j = pgp_text(&m);
    assert!(j.contains("steel") && !j.contains("iron"), "{j}");
    assert!(
        m.export.rows.iter().all(|r| !r.job_key.contains("iron")),
        "iron is outside the run: no Files row"
    );
    assert!(
        notes(&m).contains("iron") && notes(&m).contains("not exported yet"),
        "{}",
        notes(&m)
    );
}

#[test]
fn an_emptied_export_folder_drops_others_with_notes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr(d.path());
    whole(&mut m);
    std::fs::remove_dir_all(out(&m, "textures")).unwrap();
    export_sets(&mut m, &["pbr/arm/steel"], "Export set pbr/arm/steel");
    let j = pgp_text(&m);
    assert!(j.contains("steel") && !j.contains("iron"), "{j}");
    assert!(notes(&m).contains("not exported yet"), "{}", notes(&m));
}

#[test]
fn a_hand_edited_json_still_asks() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr(d.path());
    whole(&mut m);
    let p = pgp_json_path(&m);
    std::fs::write(&p, "{ \"edited\": true }\n").unwrap();
    let k = keys(&m, &["pbr/arm/steel"]);
    m.export_sets(k, "Export set pbr/arm/steel".into());
    assert!(m.review.is_some());
    m.export_from_review();
    assert!(
        m.status_line
            .starts_with("Export not started: Answer Keep or Overwrite for "),
        "{}",
        m.status_line
    );
    assert!(std::fs::read_to_string(&p).unwrap().contains("edited"));
}

// ---- Per-set files ----

fn lines_of(m: &AppModel, kind: JsonKind) -> usize {
    m.review_json().iter().filter(|l| l.kind == kind).count()
}

/// CS PBR and XPMF: rock01 (PBR Landscape) has a texture-set block and an XPMF block; leaf is a plain set whose
/// undecided alpha keeps Review open, so its JSON lines are read for a scope without and then with rock01.
#[test]
fn per_set_files_only_for_the_run_cs_pbr_and_xpmf() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    const ROCK: &str = "landscape/rock01";
    const LEAF: &str = "landscape/leaf";
    m.add_cs_block(ROCK, Kind::Txst);
    m.set_cs_edids(ROCK, Kind::Txst, &["LandscapeRock01".into()]);
    m.add_cs_block(ROCK, Kind::Mato);
    m.set_cs_edids(ROCK, Kind::Mato, &["MatoA".into()]);
    m.sets_doc
        .as_mut()
        .unwrap()
        .set_xpmf(ROCK, json!({"shelterFade": 40}).as_object().cloned());
    idle(&mut m);
    let txst = out(&m, "PBRTextureSets/LandscapeRock01.json");
    let xpmf = m
        .output_root()
        .join("SKSE/Plugins/XPMF/tessalith_rock01.json");
    let k = keys(&m, &[LEAF]);
    m.export_sets(k, "Export set landscape/leaf".into());
    assert!(m.review.is_some(), "leaf's question opens Review");
    assert_eq!(lines_of(&m, JsonKind::CsPbr), 0);
    assert_eq!(lines_of(&m, JsonKind::Xpmf), 0);
    // The scope changes while Review is open: its lines follow at once.
    let k = keys(&m, &[LEAF, ROCK]);
    m.export_sets(k, "Export 2 sets".into());
    assert!(m.review.is_some());
    assert_eq!(
        m.export_scope().map(|s| s.label.as_str()),
        Some("Export 2 sets")
    );
    assert!(lines_of(&m, JsonKind::CsPbr) > 0);
    assert!(lines_of(&m, JsonKind::Xpmf) > 0);
    m.close_review();
    m.decide(
        "landscape/leaf.png",
        Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    m.decide(
        "landscape/leaf.png",
        Answer::Alpha(engine::config::AlphaPolicy::Cutout),
    );
    idle(&mut m);
    export_sets(&mut m, &[LEAF], "Export set landscape/leaf");
    assert!(!txst.exists(), "{}", notes(&m));
    assert!(!xpmf.exists(), "{}", notes(&m));
    export_sets(&mut m, &[ROCK], "Export set landscape/rock01");
    assert!(txst.is_file(), "{}", notes(&m));
    assert!(xpmf.is_file(), "{}", notes(&m));
}

/// Complex material: iron and axe both export a JSON; a scope of iron writes iron's only.
#[test]
fn per_set_files_only_for_the_run_complex_material() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("src/m");
    for base in ["iron", "axe"] {
        colour(&r.join(format!("{base}.png")), 90);
    }
    // w's alpha needs a decision: in the scope, it keeps Review open.
    half_alpha(&r.join("w.png"));
    for name in ["iron_m.png", "axe_em.png"] {
        ImageBuffer::from_fn(64, 64, |x, y| {
            Rgba([(x * 4) as u8, (y * 4) as u8, 90, (40 + x * 2) as u8])
        })
        .save(r.join(name))
        .unwrap();
    }
    let mut m = AppModel::new(
        sdc_gui::settings::GuiSettings {
            export_folder: Some(d.path().join("export")),
            ..Default::default()
        },
        None,
        session(),
    );
    m.accept = engine::policy::AcceptSuggestions::High;
    m.open_folder(d.path().join("src"));
    idle(&mut m);
    choose_export(&mut m, d.path().join("export"));
    assert_eq!(m.target, Target::Cs);
    for id in ["m/iron", "m/axe"] {
        m.set_cm_export(id, true);
        m.set_cm_value(id, "glossiness", Some(json!(80)));
    }
    idle(&mut m);
    let k = keys(&m, &["m/iron", "m/w"]);
    m.export_sets(k, "Export 2 sets".into());
    assert!(m.review.is_some(), "w's question opens Review");
    assert_eq!(lines_of(&m, JsonKind::ComplexMaterial), 1);
    m.close_review();
    export_sets(&mut m, &["m/iron"], "Export set m/iron");
    assert!(out(&m, "m/iron_m.json").is_file(), "{}", notes(&m));
    assert!(!out(&m, "m/axe_em.json").exists(), "{}", notes(&m));
}

/// Advanced skin: a scope of skin set a writes a's `.csskin.json` only.
#[test]
fn per_set_files_only_for_the_run_cs_skin() {
    let d = tempfile::tempdir().unwrap();
    let r = d.path().join("textures/x");
    for base in ["a", "b"] {
        for s in ["", "_rfaos", "_wet", "_detail_n"] {
            colour(&r.join(format!("{base}{s}.png")), 90);
        }
    }
    // w's alpha needs a decision: in the scope, it keeps Review open.
    half_alpha(&r.join("w.png"));
    let mut m = opened_as_the_app(d.path().to_path_buf(), d.path().join("export"));
    let (a, b, w) = (
        id_ending(&m, "x/a"),
        id_ending(&m, "x/b"),
        id_ending(&m, "x/w"),
    );
    for id in [&a, &b] {
        m.set_skin_value(id, "Roughness", Some(json!(0.6)));
    }
    idle(&mut m);
    let k = keys(&m, &[&a, &w]);
    m.export_sets(k, "Export 2 sets".into());
    assert!(m.review.is_some(), "w's question opens Review");
    assert_eq!(lines_of(&m, JsonKind::CsSkin), 1);
    m.close_review();
    export_sets(&mut m, &[&a], "Export set x/a");
    assert!(
        out(&m, "textures/x/a.csskin.json").is_file(),
        "{}",
        notes(&m)
    );
    assert!(!out(&m, "textures/x/b.csskin.json").exists());
}

#[test]
fn no_orphan_notes_on_a_partial_or_excluding_run() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened_plain(d.path());
    const ROCK: &str = "landscape/rock01";
    m.add_cs_block(ROCK, Kind::Txst);
    m.set_cs_edids(ROCK, Kind::Txst, &["One".into(), "Two".into()]);
    whole(&mut m);
    assert!(
        out(&m, "PBRTextureSets/Two.json").is_file(),
        "{}",
        notes(&m)
    );
    // Two is no longer bound: a whole export would report it.
    m.set_cs_edids(ROCK, Kind::Txst, &["One".into()]);
    idle(&mut m);
    let orphan = |m: &AppModel| notes(m).contains("no set binds this EDID");
    export_sets(&mut m, &[ROCK], "Export set landscape/rock01");
    assert!(!orphan(&m), "scoped: {}", notes(&m));
    let other = set_views(&m)
        .iter()
        .find(|v| v.id != ROCK)
        .unwrap()
        .id
        .clone();
    group(&mut m, "Other", &[&other]);
    m.set_group_excluded("Other", true).unwrap();
    whole(&mut m);
    assert!(!orphan(&m), "excluding: {}", notes(&m));
    m.set_group_excluded("Other", false).unwrap();
    whole(&mut m);
    assert!(orphan(&m), "a whole export still reports it: {}", notes(&m));
}

// ---- Also ----

/// `<tmp>/mods/Mod`: a PBR set `pbr/arm/steel` and a plain set `clutter/cup`, as `also_export.rs`.
fn also_project(d: &Path) -> AppModel {
    let t = d.join("mods/Mod/textures");
    let px = |p: &Path, c: [u8; 4]| {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        ImageBuffer::from_pixel(16, 16, Rgba(c)).save(p).unwrap();
    };
    px(&t.join("pbr/arm/steel.png"), [200, 200, 200, 255]);
    px(&t.join("pbr/arm/steel_n.png"), [128, 128, 255, 255]);
    px(&t.join("pbr/arm/steel_rmaos.png"), [128, 255, 200, 255]);
    std::fs::create_dir_all(t.join("clutter")).unwrap();
    ImageBuffer::from_fn(16, 16, |x, y| {
        Rgb([(x * 16) as u8, (y * 16) as u8, ((x + y) * 8) as u8])
    })
    .save(t.join("clutter/cup.png"))
    .unwrap();
    px(&t.join("clutter/cup_n.png"), [128, 128, 255, 255]);
    let mut m = opened_as_the_app(d.join("mods/Mod"), d.join("out"));
    m.set_also_tick(AlsoTarget::Vanilla, true);
    idle(&mut m);
    assert!(m.also_on());
    m
}

#[test]
fn also_does_not_run_on_a_scoped_export_and_ticks_do_not_open_review_for_it() {
    let d = tempfile::tempdir().unwrap();
    let mut m = also_project(d.path());
    m.export_clicked();
    assert!(m.review.is_some(), "a tick opens Review for a whole export");
    m.close_review();
    let cup = id_ending(&m, "clutter/cup");
    let k = keys(&m, &[&cup]);
    m.export_sets(k, "Export set clutter/cup".into());
    assert!(
        m.review.is_none(),
        "the tick does not open Review for a scope"
    );
    idle(&mut m);
    let [p, v, _] = m.also_folders();
    assert!(
        p.join("textures/clutter/cup.dds").is_file(),
        "{}",
        notes(&m)
    );
    assert!(
        !v.join("textures/clutter/cup.dds").exists(),
        "no Vanilla phase"
    );
}

#[test]
fn whole_export_with_also_skips_excluded_sets() {
    let d = tempfile::tempdir().unwrap();
    let mut m = also_project(d.path());
    whole(&mut m);
    let [p, v, _] = m.also_folders();
    let (pc, vc) = (
        p.join("textures/clutter/cup.dds"),
        v.join("textures/clutter/cup.dds"),
    );
    assert!(vc.is_file(), "{}", notes(&m));
    let before = (stamp(&pc), stamp(&vc));
    let cup = id_ending(&m, "clutter/cup");
    group(&mut m, "Cups", &[&cup]);
    m.set_group_excluded("Cups", true).unwrap();
    ImageBuffer::from_pixel(16, 16, Rgb([9u8, 9, 9]))
        .save(d.path().join("mods/Mod/textures/clutter/cup.png"))
        .unwrap();
    m.replan();
    whole(&mut m);
    assert_eq!((stamp(&pc), stamp(&vc)), before, "{}", notes(&m));
    // The Files tab lists only what the run wrote: no "not run" row for the excluded set in any phase.
    assert!(
        m.export.rows.iter().all(|r| !r.job_key.contains("cup")),
        "{:?}",
        m.export
            .rows
            .iter()
            .map(|r| (&r.job_key, r.state.label()))
            .collect::<Vec<_>>()
    );
    assert!(v.join("textures/arm/steel.dds").is_file());
}

/// Note HF11: an excluded PBR grass set is in `[export] skip` too, so Review's CM text does not promise its
/// complex-grass stack.
#[test]
fn an_excluded_grass_set_gives_also_no_grass() {
    let d = tempfile::tempdir().unwrap();
    let mut m = also_project(d.path());
    let steel = id_ending(&m, "arm/steel");
    m.set_shader(&steel, Some(engine::shader::Shader::PbrGrass));
    idle(&mut m);
    assert!(m.also_grass());
    group(&mut m, "Grass", &[&steel]);
    m.set_group_excluded("Grass", true).unwrap();
    assert!(!m.also_grass(), "the excluded grass set writes nothing");
    assert!(m
        .also_spec()
        .cfg
        .skip
        .iter()
        .any(|s| engine::sets::same_id(s, &steel)));
}

// ---- The scope's life ----

#[test]
fn closing_review_ends_the_scope() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a"], &["w"]);
    let k = keys(&m, &["g/w"]);
    m.export_sets(k, "Export set g/w".into());
    assert!(m.export_scope().is_some());
    m.close_review();
    assert!(m.export_scope().is_none());
}

/// Note HF10: a queued scoped export that is dropped (a dialog open when the plan lands) takes its scope with it.
#[test]
fn a_dropped_queued_export_ends_the_scope() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "b"], &[]);
    m.replan();
    let k = keys(&m, &["g/a"]);
    m.export_sets(k, "Export set g/a".into());
    assert!(m.export_queued(), "waits for the plan");
    assert!(m.export_scope().is_some());
    m.dialog = Some(sdc_gui::model::Dialog::Error("x".into()));
    idle(&mut m);
    assert!(
        m.status_line.starts_with("Export not started"),
        "{}",
        m.status_line
    );
    // An open dialog may park a scoped export, so the scope waits for its answer.
    assert!(m.export_scope().is_some(), "kept while the dialog is open");
    m.dialog = None;
    idle(&mut m);
    assert!(m.export_scope().is_none(), "the dropped export ends it");
    assert_eq!(m.write_only(), None, "the next export is whole");
}

/// Note HF10 (final review I4): a scoped export that waits for a plan writes only its own set when the plan lands.
#[test]
fn a_queued_scoped_export_writes_only_its_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "b"], &[]);
    m.replan();
    let k = keys(&m, &["g/a"]);
    m.export_sets(k, "Export set g/a".into());
    assert!(m.export_queued(), "waits for the plan");
    idle(&mut m);
    assert!(out(&m, "textures/g/a.dds").is_file(), "{}", notes(&m));
    assert!(
        !out(&m, "textures/g/b.dds").exists(),
        "b is outside the scope"
    );
}

/// Note HF10 (final review I6): Export set is off, with the reason, on a set with no files.
#[test]
fn export_set_is_off_for_a_set_with_no_files() {
    use eframe::egui;
    use egui_kittest::kittest::{NodeT, Queryable};
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a"], &[]);
    m.edit_docs("New set", None, true, |_, s| s.create_set("g/ghost"));
    idle(&mut m);
    let ghost = set_views(&m)
        .into_iter()
        .find(|v| v.id == "g/ghost")
        .expect("the created set has a card");
    assert!(ghost.key.is_empty(), "no files, so no key");
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1500.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h.query_all_by_label_contains("ghost")
        .next()
        .expect("the ghost card")
        .click_secondary();
    h.run();
    assert!(h.get_by_label("Export set").accesskit_node().is_disabled());
}

/// Note HF1 (final review I3): an excluded set's packed output stays in the Textures column, as its raw maps do.
#[test]
fn an_excluded_sets_packed_output_stays_listed() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    const RMAOS: &str = "landscape/rock01_rmaos";
    let listed = |m: &AppModel| {
        m.visible_items()
            .into_iter()
            .chain(m.visible_helpers_shared().iter().copied())
            .any(|n| m.items[n].key == RMAOS)
    };
    assert!(listed(&m), "the packed output is listed");
    group(&mut m, "Rock", &["landscape/rock01"]);
    m.set_group_excluded("Rock", true).unwrap();
    assert!(listed(&m), "still listed when excluded");
}

/// Note HF10: while an export runs, Export N sets is off with the reason and Export set changes nothing.
#[test]
fn export_sets_is_off_while_an_export_runs() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "b"], &[]);
    let (a, b) = (key(&m, "g/a"), key(&m, "g/b"));
    m.click_set(&a, false, false);
    m.click_set(&b, true, false);
    m.export_clicked();
    assert!(m.export.running.is_some(), "a clean export starts at once");
    let entry = m
        .set_menu()
        .into_iter()
        .find(|e| e.action == MenuAction::ExportSets)
        .expect("the Export N sets entry");
    assert_eq!(
        entry.disabled.as_deref(),
        Some("Wait for the export to finish")
    );
    m.export_sets(keys(&m, &["g/a"]), "Export set g/a".into());
    assert!(
        m.export_scope().is_none(),
        "no scope under a running export"
    );
    idle(&mut m);
    assert!(out(&m, "textures/g/b.dds").is_file(), "{}", notes(&m));
}

#[test]
fn the_next_plain_export_is_whole_project() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a", "b"], &[]);
    export_sets(&mut m, &["g/a"], "Export set g/a");
    assert!(!out(&m, "textures/g/b.dds").exists());
    whole(&mut m);
    assert!(out(&m, "textures/g/b.dds").is_file(), "{}", notes(&m));
}

#[test]
fn the_scope_keys_the_caches() {
    use sdc_gui::model::bottom::problems;
    let d = tempfile::tempdir().unwrap();
    // w and x each ask a question; a scope of w keeps Review open and leaves x's rows out.
    let mut m = plain(d.path(), &["a"], &["w", "x"]);
    let before = problems(&m).len();
    let w = keys(&m, &["g/w"]);
    m.export_sets(w, "Export set g/w".into());
    assert!(m.review.is_some());
    let during = problems(&m).len();
    assert!(
        0 < during && during < before,
        "during {during}, before {before}"
    );
    m.close_review();
    assert_eq!(problems(&m).len(), before, "the scope's end restores them");
}

/// Note HF10 (final review I2): the Textures column's pill counts and lists follow the scope too.
#[test]
fn the_scope_keys_the_texture_counts_and_lists() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a"], &["w", "x"]);
    m.filter.needs_decision = true;
    let before = (m.filter_counts().needs_decision, m.visible_items().len());
    // The first ask starts the size reads; once they land the folder total has every estimate.
    m.folder_size();
    idle(&mut m);
    let size = m.folder_size();
    assert_eq!(size.1, 0, "every estimate is in");
    assert_eq!(before.0, 2);
    let w = keys(&m, &["g/w"]);
    m.export_sets(w, "Export set g/w".into());
    assert!(m.review.is_some());
    assert_eq!(m.filter_counts().needs_decision, 1);
    assert!(
        m.visible_items().len() < before.1,
        "x's tile leaves the list"
    );
    assert!(
        m.folder_size().0 < size.0,
        "the folder size counts the scope"
    );
    m.close_review();
    assert_eq!(
        (m.filter_counts().needs_decision, m.visible_items().len()),
        before
    );
    assert_eq!(m.folder_size(), size);
}

/// Note HF10 (final review M5): a scoped Review has no Also column; Also runs on whole exports only.
#[test]
fn a_scoped_review_has_no_also_column() {
    use eframe::egui;
    use egui_kittest::kittest::Queryable;
    let also_column = |m: AppModel| {
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1500.0, 900.0))
            .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
        sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
        h.run();
        let shown = h.query_all_by_label("Also export as").next().is_some();
        shown
    };
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a"], &["w"]);
    m.export_clicked();
    assert!(m.review.is_some());
    assert!(also_column(m), "a whole export's Review has it (cs mode)");
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a"], &["w"]);
    m.export_sets(keys(&m, &["g/w"]), "Export set g/w".into());
    assert!(m.review.is_some());
    assert!(!also_column(m), "a scoped Review has none");
}

#[test]
fn review_says_export_group_armor() {
    use eframe::egui;
    use egui_kittest::kittest::{NodeT, Queryable};
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a"], &["w"]);
    group(&mut m, "Armor", &["g/w"]);
    m.export_group("Armor");
    assert!(m.review.is_some());
    let mut h = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1500.0, 900.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    assert!(
        h.query_all_by_label("Export group Armor").next().is_some(),
        "the heading"
    );
    assert!(
        h.query_all_by_label_contains("Export group Armor")
            .any(|n| n.accesskit_node().role() == egui::accesskit::Role::Button),
        "the button"
    );
}

// ---- Note HS1: the scope follows a new export folder ----

#[test]
fn export_set_survives_choosing_the_folder_from_review() {
    let d = tempfile::tempdir().unwrap();
    for n in ["a", "b"] {
        colour(&d.path().join(format!("textures/g/{n}.png")), 90);
        flat(&d.path().join(format!("textures/g/{n}_n.png")));
    }
    let mut m = AppModel::new(GuiSettings::default(), None, session());
    m.open_folder(d.path().join("textures"));
    idle(&mut m);
    assert!(m.export_folder_chosen().is_none());
    let k = keys(&m, &["g/a"]);
    m.export_sets(k, "Export set g/a".into());
    assert!(m.review.is_some() && m.export_scope().is_some());
    let export = d.path().join("export");
    m.set_export_folder(export.clone());
    idle(&mut m);
    assert_eq!(
        m.export_scope().map(|s| s.keys.clone()),
        Some(keys(&m, &["g/a"]))
    );
    assert_eq!(m.export_files(), 2);
    m.export_from_review();
    idle(&mut m);
    assert!(export.join("textures/g/a.dds").is_file(), "{}", notes(&m));
    assert!(export.join("textures/g/a_n.dds").is_file());
    assert!(!export.join("textures/g/b.dds").exists());
    assert!(m.export_scope().is_none());
}

#[test]
fn export_set_survives_a_folder_change_while_review_is_open() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["a"], &["w"]);
    let k = keys(&m, &["g/w"]);
    m.export_sets(k, "Export set g/w".into());
    assert!(m.review.is_some());
    let export2 = d.path().join("export2");
    m.set_export_folder(export2.clone());
    idle(&mut m);
    m.decide(
        "g/w.png",
        Answer::Alpha(engine::config::AlphaPolicy::Cutout),
    );
    idle(&mut m);
    m.export_from_review();
    idle(&mut m);
    assert!(export2.join("textures/g/w.dds").is_file(), "{}", notes(&m));
    assert!(!export2.join("textures/g/a.dds").exists());
}

#[test]
fn a_scoped_set_gone_after_the_folder_change_does_not_stop_the_others() {
    let d = tempfile::tempdir().unwrap();
    let mut m = plain(d.path(), &["x"], &["w", "v"]);
    let k = keys(&m, &["g/w", "g/v"]);
    m.export_sets(k, "Export 2 sets".into());
    assert!(m.review.is_some());
    for f in ["v.png", "v_n.png"] {
        std::fs::remove_file(d.path().join("textures/g").join(f)).unwrap();
    }
    let export2 = d.path().join("export2");
    m.set_export_folder(export2.clone());
    idle(&mut m);
    m.decide(
        "g/w.png",
        Answer::Alpha(engine::config::AlphaPolicy::Cutout),
    );
    idle(&mut m);
    m.export_from_review();
    idle(&mut m);
    assert!(export2.join("textures/g/w.dds").is_file(), "{}", notes(&m));
    assert!(!export2.join("textures/g/x.dds").exists());
    assert!(m.dialog.is_none(), "{:?}", m.dialog);
}

// ---- Note HS7: a scoped export with a tick checks the PBR folder ----

fn scoped_cup_export(m: &mut AppModel) {
    let cup = id_ending(m, "clutter/cup");
    let k = keys(m, &[&cup]);
    m.export_sets(k, "Export set clutter/cup".into());
    idle(m);
}

#[test]
fn a_scoped_export_with_a_tick_checks_the_pbr_folder_it_writes() {
    let d = tempfile::tempdir().unwrap();
    let mut m = also_project(d.path());
    m.set_export_folder(d.path().join("mods"));
    idle(&mut m);
    scoped_cup_export(&mut m);
    assert!(m.dialog.is_none(), "{:?}", m.dialog);
    let pbr = d.path().join("mods/Mod PBR");
    assert_eq!(m.also_folders()[0], pbr);
    assert!(
        pbr.join("textures/clutter/cup.dds").is_file(),
        "{}",
        notes(&m)
    );
    // The PBR folder is the source mod: refused, not written in place.
    let d2 = tempfile::tempdir().unwrap();
    let mut m = also_project(d2.path());
    m.set_also_location(&d2.path().join("mods"));
    idle(&mut m);
    m.set_also_settings(|c| c.names.pbr = Some("Mod".into()));
    idle(&mut m);
    scoped_cup_export(&mut m);
    assert!(matches!(m.dialog, Some(Dialog::Error(_))), "{:?}", m.dialog);
    assert!(!d2.path().join("mods/Mod/textures/clutter/cup.dds").exists());
}
