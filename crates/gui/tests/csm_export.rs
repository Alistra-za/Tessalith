//! Note IE10: each PBR texture set exports a `.csm` material, and its texture-set JSON points at it (design §5.2,
//! §5.3); a PBR Landscape set keeps its inline JSON.
mod support;
use nif::csm::{self, Csm, Value};
use sdc_gui::model::auto_flags::{hint, resolved_for};
use sdc_gui::model::bottom::{problems, Level};
use sdc_gui::model::cs_pbr::{Kind, TXST2_FIELDS, TXST_FIELDS, TXST_PARAMS};
use sdc_gui::model::csm_export::*;
use sdc_gui::model::history::{record_key, sha256_of};
use sdc_gui::model::sets::{views, SetView};
use sdc_gui::model::AppModel;
use sdc_gui::pgp::settings::Settings;
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};
use std::time::Duration;
use support::*;

const ROCK: &str = "landscape/rock01";
/// The fixture set's `.csm`: its output prefix is `textures\pbr\landscape\rock01`.
const ROCK_CSM: &str = "materials\\pbr\\landscape\\rock01.csm";

fn view(m: &AppModel) -> SetView {
    views(m)
        .iter()
        .find(|v| v.id == ROCK)
        .cloned()
        .expect("the rock01 set")
}

/// The set's PGPatcher settings as the export resolves them.
fn pgp(m: &AppModel, v: &SetView) -> Settings {
    resolved_for(v.json, &v.slots, &v.settings, &hint(m, v))
}

fn rock_csm(m: &AppModel) -> SetCsm {
    let v = view(m);
    set_csm(&v, &pgp(m, &v), m.cs_block(ROCK, Kind::Txst).as_ref()).expect("a PBR set gets a .csm")
}

fn with_txst(d: &Path) -> AppModel {
    let mut m = pbr_opened_plain(d);
    m.add_cs_block(ROCK, Kind::Txst);
    m.set_cs_edids(ROCK, Kind::Txst, &["Rock01".into()]);
    assert!(m.wait_idle(Duration::from_secs(60)));
    m
}

fn export_and_wait(m: &mut AppModel) {
    assert!(m.wait_idle(Duration::from_secs(60)), "planning");
    export_through(m);
    assert!(m.wait_idle(Duration::from_secs(120)), "export");
}

fn csm_file(m: &AppModel) -> PathBuf {
    m.export_root().join(ROCK_CSM.replace('\\', "/"))
}

fn txst_file(m: &AppModel) -> PathBuf {
    m.export_root().join("PBRTextureSets/Rock01.json")
}

fn problem_texts(m: &AppModel) -> Vec<String> {
    problems(m)
        .iter()
        .filter(|p| p.level == Level::Warning)
        .map(|p| p.text.clone())
        .collect()
}

const GENERIC: [&str; 19] = [
    "TileU",
    "TileV",
    "UOffset",
    "VOffset",
    "UScale",
    "VScale",
    "Alpha",
    "TwoSided",
    "Decal",
    "DynamicDecal",
    "ModelSpaceNormals",
    "VertexColors",
    "CastShadows",
    "ReceiveShadows",
    "ZBufferTest",
    "ZBufferWrite",
    "ExternalEmittance",
    "EmittanceColor",
    "EmittanceMult",
];

/// Design §5.2: the switches come from the PGPatcher settings, the numbers only from what the block sets, the
/// textures from the set; never a generic key.
#[test]
fn set_csm_from_settings() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_txst(d.path());
    m.set_json_value(ROCK, "fuzz_on", Some(json!(true)));
    m.set_cs_value(ROCK, Kind::Txst, "fuzzColor", Some(json!([0.9, 0.85, 0.8])));
    m.set_cs_value(ROCK, Kind::Txst, "fuzzWeight", Some(json!(0.6)));
    let s = rock_csm(&m);
    let text = csm::to_text(&s.csm);
    for want in [
        "\"PBREnabled\": true",
        "\"PBRFuzzEnabled\": true",
        "\"PBRFuzzColor\": \"#e6d9cc\"",
        "\"PBRFuzzWeight\": 0.6",
        "\"DiffuseTexture\": \"pbr/landscape/rock01.dds\"",
        "\"NormalTexture\": \"pbr/landscape/rock01_n.dds\"",
        "\"RmaosTexture\": \"pbr/landscape/rock01_rmaos.dds\"",
    ] {
        assert!(text.contains(want), "{want} in\n{text}");
    }
    for g in GENERIC {
        assert!(!text.contains(&format!("\"{g}\"")), "{g} in\n{text}");
    }
    // Only what the block sets: no default fills in the other numbers.
    assert!(!text.contains("PBRSpecularLevel"), "{text}");
    assert!(!text.contains("PBRFuzzRoughness"), "{text}");
    assert_eq!(s.path, ROCK_CSM);
    assert_eq!(s.blocked, None, "{:?}", s.warnings);
    assert_eq!(
        csm::parse(&text).as_ref(),
        Ok(&s.csm),
        "the file reads back"
    );
}

/// Design §5.2: each PGPatcher switch pair (Note IE10 records the pairs checked against the v1 mapping).
#[test]
fn pgp_switch_pairs() {
    let d = tempfile::tempdir().unwrap();
    let m = with_txst(d.path());
    let v = view(&m);
    let base = pgp(&m, &v);
    let on = |keys: &[&str]| {
        let mut s = base.clone();
        for k in keys {
            s.insert(k.to_string(), json!(true));
        }
        set_csm(&v, &s, None).unwrap().csm
    };
    let has = |c: &Csm, k: &str| c.values.get(k) == Some(&Value::Bool(true));
    for (keys, want) in [
        (&["subsurface"][..], &["PBRSubsurfaceEnabled"][..]),
        (
            &["subsurface_foliage"],
            &["PBRSubsurfaceEnabled", "PBRFoliageEnabled"],
        ),
        (&["hair"], &["PBRHairMarschner"]),
        (&["multilayer"], &["PBRTwoLayerEnabled"]),
        (
            &["multilayer", "coat_normal"],
            &["PBRTwoLayerEnabled", "PBRTwoLayerCoatNormal"],
        ),
        (
            &["multilayer", "coat_diffuse"],
            &["PBRTwoLayerEnabled", "PBRTwoLayerColoredCoat"],
        ),
        (
            &["multilayer", "parallax"],
            &["PBRTwoLayerEnabled", "PBRTwoLayerInterlayerParallax"],
        ),
        (&["fuzz_on"], &["PBRFuzzEnabled"]),
    ] {
        let c = on(keys);
        for w in want {
            assert!(has(&c, w), "{keys:?} -> {w}: {c:?}");
        }
    }
    // The coat options need two-layer, and PGPatcher's glint switch is not used (the block's is).
    for keys in [
        &["coat_normal"][..],
        &["coat_diffuse"],
        &["parallax"],
        &["glint_on"],
    ] {
        let c = on(keys);
        let switches: Vec<_> = c
            .values
            .iter()
            .filter(|(k, v)| **k != "PBREnabled" && matches!(v, Value::Bool(true)))
            .collect();
        assert!(switches.is_empty(), "{keys:?}: {switches:?}");
    }
}

#[test]
fn csm_path_cases() {
    for (set, want) in [
        (
            "textures\\pbr\\clutter\\rug01",
            "materials\\pbr\\clutter\\rug01.csm",
        ),
        (
            "textures/pbr/clutter/rug01",
            "materials\\pbr\\clutter\\rug01.csm",
        ),
        (
            "Textures\\PBR\\clutter\\rug01",
            "materials\\PBR\\clutter\\rug01.csm",
        ),
        ("pbr\\clutter\\rug01", "materials\\pbr\\clutter\\rug01.csm"),
        (
            "textures\\pbr\\clutter\\rug01.dds",
            "materials\\pbr\\clutter\\rug01.csm",
        ),
        ("textures\\rug01", "materials\\rug01.csm"),
    ] {
        assert_eq!(csm_path(set), want, "{set}");
    }
}

/// Pins the GUI's texture-set fields against `nif::pbr2`'s key map both ways: every field that is a PBR parameter
/// (a number or colour, and the glint numbers) is in `TXST_PARAMS`, every `TXST_PARAMS` key is a field, each index
/// has a `.csm` key, and `csm_key` agrees. The three fields outside the map are named.
#[test]
fn txst_params_match_gui() {
    use sdc_gui::model::cs_pbr::{FieldKind, GLINT_FIELDS};
    let outside = ["innerLayerDisplacementOffset", "fuzzRoughness"];
    let mut gui: Vec<String> = vec![];
    for f in TXST_FIELDS.iter().chain(&TXST2_FIELDS) {
        match f.kind {
            FieldKind::Num | FieldKind::Rgb if !outside.contains(&f.name) => {
                gui.push(f.name.to_string())
            }
            FieldKind::Glint => gui.extend(
                GLINT_FIELDS
                    .iter()
                    .map(|g| format!("{}.{}", f.name, g.name)),
            ),
            _ => {}
        }
    }
    let mut map: Vec<String> = TXST_PARAMS.iter().map(|(k, _)| k.to_string()).collect();
    gui.sort();
    map.sort();
    assert_eq!(gui, map);
    for (k, i) in TXST_PARAMS {
        let key = nif::pbr2::CSM_PARAMS
            .iter()
            .find(|(_, j)| j == i)
            .map(|(c, _)| *c);
        assert!(key.is_some(), "{k}: no .csm key for parameter {i}");
        assert_eq!(csm_key(k), key, "{k}");
    }
    assert_eq!(csm_key("fuzzRoughness"), Some("PBRFuzzRoughness"));
    assert_eq!(csm_key("innerLayerDisplacementOffset"), None);
}

/// Never overwrite a file Tessalith did not write: a `.csm` already at the path with no record is kept and named.
#[test]
fn export_refuses_foreign_csm() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_txst(d.path());
    let f = csm_file(&m);
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, "{\"Version\": 1}").unwrap();
    export_and_wait(&mut m);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "{\"Version\": 1}");
    assert!(
        m.export
            .notes
            .iter()
            .any(|n| n.contains("rock01.csm") && n.contains("kept as edited outside Tessalith")),
        "{:?}",
        m.export.notes
    );
    assert!(m.sets_doc.as_ref().unwrap().csm_shas().is_empty());
}

/// Ledger ruling: CS PBR needs Diffuse, Normal and RMAOS (the schema's PBREnabled rule), so a set missing one writes
/// no `.csm` and no texture-set JSON; the message names the set and the slot.
#[test]
fn export_blocks_pbr_set_without_rmaos() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_txst(d.path());
    let tex = d.path().join("textures/landscape");
    for f in ["rock01_roughness.png", "rock01_ao.png"] {
        std::fs::remove_file(tex.join(f)).unwrap();
    }
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
    let s = rock_csm(&m);
    let why = s.blocked.clone().expect("blocked");
    assert!(
        why.contains(ROCK) && why.contains("has no RMAOS texture;"),
        "{why}"
    );
    assert!(problem_texts(&m).contains(&why), "{:?}", problem_texts(&m));
    export_and_wait(&mut m);
    assert!(!csm_file(&m).exists());
    assert!(
        !txst_file(&m).exists(),
        "the JSON would name a missing .csm"
    );
    assert!(m.export.notes.contains(&why), "{:?}", m.export.notes);
}

#[test]
fn export_blocks_pbr_set_without_normal() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_txst(d.path());
    std::fs::remove_file(d.path().join("textures/landscape/rock01_normal.png")).unwrap();
    m.replan();
    assert!(m.wait_idle(Duration::from_secs(60)));
    let why = rock_csm(&m).blocked.expect("blocked");
    assert!(
        why.contains(ROCK) && why.contains("has no Normal texture;"),
        "{why}"
    );
}

/// A switch pair the schema refuses (PGPatcher's fuzz with the block's glint) writes no `.csm`, naming the rule.
#[test]
fn a_switch_clash_blocks_the_csm() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_txst(d.path());
    m.set_json_value(ROCK, "fuzz_on", Some(json!(true)));
    m.set_cs_value(
        ROCK,
        Kind::Txst,
        "glintParameters.enabled",
        Some(json!(true)),
    );
    let why = rock_csm(&m).blocked.expect("blocked");
    assert!(why.contains(ROCK), "{why}");
    assert!(why.contains(csm::rule::FUZZ_GLINT), "{why}");
}

/// Design §5.2 (Q6): a number outside the schema range is clamped, with a warning in the set's problem list that goes
/// as soon as the value is back in range.
#[test]
fn clamp_warns() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_txst(d.path());
    m.set_cs_value(ROCK, Kind::Txst, "specularLevel", Some(json!(2.0)));
    let s = rock_csm(&m);
    assert_eq!(s.csm.values.get("PBRSpecularLevel"), Some(&Value::Num(1.0)));
    let w = "PBRSpecularLevel 2 is outside 0..1; exported as 1";
    assert!(s.warnings.iter().any(|x| x.contains(w)), "{:?}", s.warnings);
    assert!(
        problem_texts(&m).iter().any(|x| x.contains(w)),
        "{:?}",
        problem_texts(&m)
    );
    m.set_cs_value(ROCK, Kind::Txst, "specularLevel", Some(json!(0.5)));
    assert!(
        !problem_texts(&m)
            .iter()
            .any(|x| x.contains("PBRSpecularLevel")),
        "{:?}",
        problem_texts(&m)
    );
    // A colour channel past 1 is clamped by `#rrggbb`, with a warning too.
    m.set_cs_value(ROCK, Kind::Txst, "fuzzColor", Some(json!([1.25, 0.5, 0.0])));
    let s = rock_csm(&m);
    assert_eq!(
        s.csm.values.get("PBRFuzzColor"),
        Some(&Value::Color([255, 128, 0]))
    );
    assert!(
        s.warnings
            .iter()
            .any(|x| x.contains("PBRFuzzColor 1.25, 0.5, 0 is outside 0..1; exported as #ff8000")),
        "{:?}",
        s.warnings
    );
}

/// Ledger ruling: every number is finite before it is written; one that is not is left out with a warning.
#[test]
fn non_finite_numbers_are_left_out() {
    let mut c = Csm {
        version: 1,
        values: [
            ("PBREnabled", Value::Bool(true)),
            ("PBRSpecularLevel", Value::Num(f64::NAN)),
            ("PBRRoughnessScale", Value::Num(f64::INFINITY)),
            ("PBRFuzzWeight", Value::Num(0.5)),
        ]
        .into(),
    };
    let w = finite_only(&mut c);
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(w.iter().any(|x| x.contains("PBRSpecularLevel")), "{w:?}");
    assert_eq!(c.values.len(), 2);
    assert_eq!(c.values.get("PBRFuzzWeight"), Some(&Value::Num(0.5)));
    assert!(csm::parse(&csm::to_text(&c)).is_ok());
    // From the project file: a `nan` there is no usable number, so the key is left out with a warning.
    let d = tempfile::tempdir().unwrap();
    let mut m = with_txst(d.path());
    m.set_cs_value(ROCK, Kind::Txst, "specularLevel", Some(json!(0.25)));
    m.sets_doc.as_mut().unwrap().save(true).unwrap();
    let toml = m.sets_doc.as_ref().unwrap().path().to_path_buf();
    let text = std::fs::read_to_string(&toml).unwrap();
    assert!(text.contains("specularLevel = 0.25"), "{text}");
    std::fs::write(
        &toml,
        text.replace("specularLevel = 0.25", "specularLevel = nan"),
    )
    .unwrap();
    let m = pbr_opened_plain(d.path());
    let s = rock_csm(&m);
    assert!(!s.csm.values.contains_key("PBRSpecularLevel"), "{s:?}");
    assert!(
        s.warnings.iter().any(|x| x.contains("specularLevel")),
        "{:?}",
        s.warnings
    );
}

/// Design §5.3: a PBR Landscape set keeps its inline texture-set JSON, byte for byte, and gets no `.csm`.
#[test]
fn landscape_keeps_inline_json() {
    use sdc_gui::model::cs_pbr_export;
    let d = tempfile::tempdir().unwrap();
    let mut m = pbr_opened(d.path());
    m.set_cs_value(ROCK, Kind::Txst, "specularLevel", Some(json!(0.02)));
    m.set_cs_edids(ROCK, Kind::Txst, &["Rock01".into()]);
    assert!(m.wait_idle(Duration::from_secs(60)));
    let v = view(&m);
    assert!(set_csm(&v, &pgp(&m, &v), m.cs_block(ROCK, Kind::Txst).as_ref()).is_none());
    let b = m.cs_block(ROCK, Kind::Txst).unwrap();
    let p = cs_pbr_export::prepare(&m).unwrap();
    assert_eq!(
        p.files,
        [(
            "PBRTextureSets/Rock01.json".to_string(),
            cs_pbr_export::to_text(Kind::Txst, &b.values)
        )]
    );
    assert!(prepare(&m).unwrap().files.is_empty());
    export_and_wait(&mut m);
    assert_eq!(
        std::fs::read_to_string(txst_file(&m)).unwrap(),
        cs_pbr_export::to_text(Kind::Txst, &b.values)
    );
    assert!(!m.export_root().join("materials").exists());
}

/// Design §5.3: a PBR set's texture-set JSON names its `.csm`, which the export writes and records (the record
/// survives undo, so the next export replaces sdc's own file).
#[test]
fn txst_json_points_at_csm() {
    let d = tempfile::tempdir().unwrap();
    let mut m = with_txst(d.path());
    m.set_cs_value(ROCK, Kind::Txst, "specularLevel", Some(json!(0.02)));
    export_and_wait(&mut m);
    let j: Json = serde_json::from_str(&std::fs::read_to_string(txst_file(&m)).unwrap()).unwrap();
    assert_eq!(j, json!({"csm": ROCK_CSM}), "{:?}", m.export.notes);
    let f = csm_file(&m);
    let text = std::fs::read_to_string(&f).unwrap();
    assert_eq!(text, csm::to_text(&rock_csm(&m).csm));
    assert!(text.contains("\"PBRSpecularLevel\": 0.02"), "{text}");
    let shas = m.sets_doc.as_ref().unwrap().csm_shas();
    assert_eq!(shas.get(&record_key(&f)).cloned(), sha256_of(&f));
    m.undo();
    assert!(m.wait_idle(Duration::from_secs(60)));
    assert_eq!(m.sets_doc.as_ref().unwrap().csm_shas(), shas);
    m.set_cs_value(ROCK, Kind::Txst, "specularLevel", Some(json!(0.03)));
    export_and_wait(&mut m);
    let text = std::fs::read_to_string(&f).unwrap();
    assert!(
        text.contains("\"PBRSpecularLevel\": 0.03"),
        "{text}; {:?}",
        m.export.notes
    );
}

/// Old files open: a block with the phase-1 keys `foliage` and `innerLayerDisplacementOffset` loads; foliage exports
/// as `PBRFoliageEnabled`, the offset (no `.csm` key) is a warning, and a save keeps both values.
#[test]
fn old_project_loads() {
    let d = tempfile::tempdir().unwrap();
    {
        let mut m = with_txst(d.path());
        m.set_cs_value(ROCK, Kind::Txst, "foliage", Some(json!(true)));
        m.set_cs_value(
            ROCK,
            Kind::Txst,
            "innerLayerDisplacementOffset",
            Some(json!(0.5)),
        );
        m.sets_doc.as_mut().unwrap().save(true).unwrap();
    }
    let mut m = pbr_opened_plain(d.path());
    let b = m.cs_block(ROCK, Kind::Txst).expect("the block loads");
    assert_eq!(b.values.get("foliage"), Some(&json!(true)));
    let s = rock_csm(&m);
    assert_eq!(
        s.csm.values.get("PBRFoliageEnabled"),
        Some(&Value::Bool(true))
    );
    let warn = problem_texts(&m)
        .into_iter()
        .find(|t| t.contains("innerLayerDisplacementOffset"));
    assert!(warn.is_some(), "{:?}", problem_texts(&m));
    m.set_cs_value(ROCK, Kind::Txst, "specularLevel", Some(json!(0.03)));
    m.sets_doc.as_mut().unwrap().save(true).unwrap();
    let text = std::fs::read_to_string(m.sets_doc.as_ref().unwrap().path()).unwrap();
    assert!(text.contains("foliage = true"), "{text}");
    assert!(
        text.contains("innerLayerDisplacementOffset = 0.5"),
        "{text}"
    );
}

/// Design §5.2: a `.csm` set's panel hides the row a `.csm` has no key for (the value stays in the project); the
/// others are drawn.
#[test]
fn the_panel_hides_the_offset_row() {
    use egui_kittest::kittest::Queryable;
    let d = tempfile::tempdir().unwrap();
    let mut m = with_txst(d.path());
    m.set_cs_value(
        ROCK,
        Kind::Txst,
        "innerLayerDisplacementOffset",
        Some(json!(0.5)),
    );
    m.set_cs_value(ROCK, Kind::Txst, "specularLevel", Some(json!(0.5)));
    let key = view(&m).key;
    m.select_set(&key);
    let mut h = egui_kittest::Harness::builder()
        .with_size(eframe::egui::vec2(1400.0, 2400.0))
        .build_ui_state(|ui, m: &mut AppModel| sdc_gui::ui::draw(ui, m), m);
    sdc_gui::fonts::install_system_fonts(&h.ctx).expect("system fonts");
    h.run();
    h.get_by_label(sdc_gui::ui::cs_pbr::SECTION).click();
    for _ in 0..3 {
        h.run();
    }
    let label = |name: &str| sdc_gui::model::cs_pbr::field_label(name).to_string();
    let has = |s: &str| h.query_all_by_label_contains(s).next().is_some();
    assert!(has(&label("specularLevel")), "the other rows are drawn");
    assert!(!has(&label("innerLayerDisplacementOffset")));
    let b = h.state().cs_block(ROCK, Kind::Txst).unwrap();
    assert_eq!(
        b.values.get("innerLayerDisplacementOffset"),
        Some(&json!(0.5))
    );
}

/// The `.csm` hash record lives in the sidecar like the other JSON kinds': read back on open, kept across undo.
#[test]
fn csm_shas_live_in_sidecar() {
    use sdc_gui::sets_doc::SetsDoc;
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("sdc-project.toml"),
        "[[sets]]\nid = \"a/b\"\n",
    )
    .unwrap();
    let mut s = SetsDoc::open(d.path());
    let before = s.text();
    let shas = [("materials/pbr/a/b.csm".to_string(), "00".to_string())].into();
    s.set_csm_shas(&shas).unwrap();
    assert_eq!(s.csm_shas(), shas);
    assert_eq!(
        SetsDoc::open(d.path()).csm_shas(),
        shas,
        "read back on open"
    );
    s.set_value("a/b", "emissive", Some(json!(true)));
    s.restore_keeping_json_sha(&before);
    assert_eq!(s.csm_shas(), shas, "kept across undo");
}
