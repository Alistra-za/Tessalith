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
use sdc_gui::model::{Answer, AppModel};
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

/// `textures/g`: a ready set `g/<n>` per `ready` (a colour map and a flat `_n`), and a set per `warn` whose colour
/// map has an alpha that needs a decision. Opened as the app opens a folder, exports under `<dir>/export`.
fn plain(dir: &Path, ready: &[&str], warn: &[&str]) -> AppModel {
    let g = dir.join("textures/g");
    for n in ready {
        colour(&g.join(format!("{n}.png")), 90);
        flat(&g.join(format!("{n}_n.png")));
    }
    for n in warn {
        std::fs::create_dir_all(&g).unwrap();
        ImageBuffer::from_fn(64, 64, |x, y| {
            Rgba([
                (x * 4) as u8,
                (y * 4) as u8,
                90,
                if x < 32 { 255 } else { 0 },
            ])
        })
        .save(g.join(format!("{n}.png")))
        .unwrap();
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

/// CS PBR and XPMF: rock01 (PBR Landscape) has a texture-set block and an XPMF block; leaf is a plain set.
#[test]
fn per_set_files_only_for_the_run_cs_pbr_and_xpmf() {
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    const ROCK: &str = "landscape/rock01";
    m.add_cs_block(ROCK, Kind::Txst);
    m.set_cs_edids(ROCK, Kind::Txst, &["LandscapeRock01".into()]);
    m.add_cs_block(ROCK, Kind::Mato);
    m.set_cs_edids(ROCK, Kind::Mato, &["MatoA".into()]);
    m.sets_doc
        .as_mut()
        .unwrap()
        .set_xpmf(ROCK, json!({"shelterFade": 40}).as_object().cloned());
    m.decide(
        "landscape/leaf.png",
        Answer::Subtype(engine::config::Subtype::Diffuse),
    );
    m.decide(
        "landscape/leaf.png",
        Answer::Alpha(engine::config::AlphaPolicy::Cutout),
    );
    idle(&mut m);
    let txst = out(&m, "PBRTextureSets/LandscapeRock01.json");
    let xpmf = m
        .output_root()
        .join("SKSE/Plugins/XPMF/tessalith_rock01.json");
    // Review's lines for a scope without rock01.
    let k = keys(&m, &["landscape/leaf"]);
    m.export_sets(k, "Export set landscape/leaf".into());
    if m.review.is_some() {
        assert_eq!(lines_of(&m, JsonKind::CsPbr), 0);
        assert_eq!(lines_of(&m, JsonKind::Xpmf), 0);
    }
    finish(&mut m);
    assert!(!txst.exists(), "{}", notes(&m));
    assert!(!xpmf.exists(), "{}", notes(&m));
    let k = keys(&m, &[ROCK]);
    m.export_sets(k, "Export set landscape/rock01".into());
    if m.review.is_some() {
        assert!(lines_of(&m, JsonKind::CsPbr) > 0);
        assert!(lines_of(&m, JsonKind::Xpmf) > 0);
    }
    finish(&mut m);
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
    let k = keys(&m, &["m/iron"]);
    m.export_sets(k, "Export set m/iron".into());
    if m.review.is_some() {
        assert_eq!(lines_of(&m, JsonKind::ComplexMaterial), 1);
    }
    finish(&mut m);
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
    let mut m = opened_as_the_app(d.path().to_path_buf(), d.path().join("export"));
    for id in ["textures/x/a", "textures/x/b"] {
        let id = set_views(&m)
            .iter()
            .find(|v| v.key.ends_with(&id[9..]))
            .map(|v| v.id.clone())
            .expect("a skin set");
        m.set_skin_value(&id, "Roughness", Some(json!(0.6)));
    }
    idle(&mut m);
    let a = set_views(&m)
        .iter()
        .find(|v| v.key.ends_with("x/a"))
        .unwrap()
        .key
        .clone();
    m.export_sets([a].into(), "Export set x/a".into());
    if m.review.is_some() {
        assert_eq!(lines_of(&m, JsonKind::CsSkin), 1);
    }
    finish(&mut m);
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
