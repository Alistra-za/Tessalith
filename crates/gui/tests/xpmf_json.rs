//! Note GI (XPMF Task 5, design X2-X9): the XPMF profile writer: file name, defaults from the set, text, the write
//! guard, refusals and schema validation.
mod support;
use image::{ImageBuffer, Luma, Rgb};
use sdc_gui::model::cs_pbr::Kind;
use sdc_gui::model::history::{record_key, sha256_of};
use sdc_gui::model::sets::set_views;
use sdc_gui::model::xpmf_json::*;
use sdc_gui::model::AppModel;
use sdc_gui::pgp::validate::{validate_xpmf, XPMF_SCHEMA_JSON};
use serde_json::{json, Map, Value as Json};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

const ID: &str = "landscape/rock01";
const FILE: &str = "SKSE/Plugins/XPMF/tessalith_rock01.json";

fn obj(v: Json) -> Map<String, Json> {
    v.as_object().unwrap().clone()
}

/// `support::pbr_opened` (rock01 auto PBR Landscape on CS) with a `_noise` and a `_detail` beside it (all 4 slots
/// filled) and a `cs_mato` block `[MatoA, MatoB]` when `mato`.
fn opened(dir: &Path, mato: bool) -> AppModel {
    let r = dir.join("textures/landscape");
    std::fs::create_dir_all(&r).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Luma([((x + y) * 2) as u8]))
        .save(r.join("rock01_noise.png"))
        .unwrap();
    ImageBuffer::from_pixel(64, 64, Rgb([128u8, 128, 255]))
        .save(r.join("rock01_detail.png"))
        .unwrap();
    let mut m = support::pbr_opened(dir);
    if mato {
        m.add_cs_block(ID, Kind::Mato);
        m.set_cs_edids(ID, Kind::Mato, &["MatoA".into(), "MatoB".into()]);
    }
    assert!(
        m.wait_idle(Duration::from_secs(60)),
        "planning did not finish"
    );
    m
}

fn set_block(m: &mut AppModel, id: &str, b: Json) {
    m.sets_doc.as_mut().unwrap().set_xpmf(id, Some(obj(b)));
}

fn entry(m: &AppModel, id: &str) -> sdc_gui::sets_doc::SetEntry {
    m.sets_doc
        .as_ref()
        .unwrap()
        .entries()
        .iter()
        .find(|e| e.id == id)
        .unwrap()
        .clone()
}

#[test]
fn file_name_folds() {
    assert_eq!(
        file_name("rocks/Snow Rocks").as_deref(),
        Ok("tessalith_snow_rocks.json")
    );
    assert_eq!(
        file_name("a/b/Ünï-ç_9").as_deref(),
        Ok("tessalith__n_-__9.json")
    );
    assert!(file_name("a/").is_err());
}

#[test]
fn defaults_from_the_set() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path(), true);
    set_block(&mut m, ID, json!({}));
    let doc = profile(&m, &entry(&m, ID)).unwrap();
    assert_eq!(doc["name"], json!(ID));
    assert_eq!(doc["editorIds"], json!(["MatoA", "MatoB"]));
    assert_eq!(doc["pbr"], json!(true));
    let tex = [
        "diffuseTexture",
        "normalTexture",
        "noiseTexture",
        "detailNormalTexture",
    ];
    for k in tex {
        let p = doc[k].as_str().unwrap_or_else(|| panic!("{k}: {doc:?}"));
        assert!(p.starts_with("textures/"), "{k}: {p}");
        assert!(p.ends_with(".dds"), "{k}: {p}");
    }
    let mut keys: Vec<&str> = doc.keys().map(String::as_str).collect();
    keys.sort();
    let mut want = vec!["name", "editorIds", "pbr"];
    want.extend(tex);
    want.sort();
    assert_eq!(keys, want);
    // prepare: one file, every default texture is this project's own output.
    let p = prepare(&m).unwrap();
    assert_eq!(p.files.len(), 1, "{:?}", p.notes);
    assert_eq!(p.files[0].0, FILE);
    let own = &p.own_textures[FILE];
    assert_eq!(own.len(), 4, "{own:?}");
    assert!(tex
        .iter()
        .all(|k| own.contains(&doc[*k].as_str().unwrap().to_string())));
}

#[test]
fn block_overrides_and_drops() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path(), true);
    set_block(
        &mut m,
        ID,
        json!({"specularMult": 2, "falloffScale": 0.5, "noiseTexture": "", "excludeEditorIds": [],
               "editorIds": ["*snow*"], "name": "Snow rocks"}),
    );
    let doc = profile(&m, &entry(&m, ID)).unwrap();
    assert!(!doc.contains_key("specularMult"), "{doc:?}");
    assert!(!doc.contains_key("noiseTexture"), "{doc:?}");
    assert!(!doc.contains_key("excludeEditorIds"), "{doc:?}");
    assert_eq!(doc["falloffScale"], json!(0.5));
    assert_eq!(doc["editorIds"], json!(["*snow*"]));
    assert_eq!(doc["name"], json!("Snow rocks"));
    // A typed path that is not this project's output is not checked on export.
    set_block(
        &mut m,
        ID,
        json!({"diffuseTexture": "landscape\\snow01", "pbr": false, "specularMult": 2}),
    );
    let doc = profile(&m, &entry(&m, ID)).unwrap();
    assert_eq!(doc["specularMult"], json!(2), "kept while pbr is off");
    let p = prepare(&m).unwrap();
    assert_eq!(p.own_textures[FILE].len(), 3, "{:?}", p.own_textures);
}

#[test]
fn golden_text() {
    let doc = obj(json!({
        "name": "Snow rocks",
        "editorIds": ["MatoA"],
        "pbr": true,
        "diffuseTexture": "textures/landscape/rock01.dds",
        "shelterFade": 32,
        "falloffBias": 0.4
    }));
    assert_eq!(
        to_text(&doc),
        [
            "{",
            "  \"$schema\": \"https://raw.githubusercontent.com/hakasapl/xpmf/main/schema/profile.schema.json\",",
            "  \"diffuseTexture\": \"textures/landscape/rock01.dds\",",
            "  \"editorIds\": [",
            "    \"MatoA\"",
            "  ],",
            "  \"falloffBias\": 0.4,",
            "  \"name\": \"Snow rocks\",",
            "  \"pbr\": true,",
            "  \"shelterFade\": 32",
            "}",
            "",
        ]
        .join("\n")
    );
    let schema: Json = serde_json::from_str(XPMF_SCHEMA_JSON).unwrap();
    assert_eq!(schema["$id"], json!(SCHEMA_URL));
}

/// A hand-built pending profile under `root`.
fn pending(root: &Path, text: &str) -> PendingXpmf {
    PendingXpmf {
        root: root.to_path_buf(),
        files: vec![(FILE.into(), text.into())],
        ..Default::default()
    }
}

#[test]
fn foreign_file_kept() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join(FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{\"name\": \"theirs\"}").unwrap();
    let mut notes = vec![];
    let shas = finish(&pending(d.path(), "{}\n"), &BTreeMap::new(), &mut notes);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{\"name\": \"theirs\"}"
    );
    assert!(shas.is_empty(), "{shas:?}");
    assert!(notes.iter().any(|n| n.contains("kept")), "{notes:?}");
}

#[test]
fn own_file_replaced() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join(FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "old\n").unwrap();
    let rec: BTreeMap<String, String> = [(record_key(&path), sha256_of(&path).unwrap())].into();
    let mut notes = vec![];
    let shas = finish(&pending(d.path(), "new\n"), &rec, &mut notes);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
    assert_eq!(shas[&record_key(&path)], sha256_of(&path).unwrap());
    assert!(
        notes.iter().any(|n| n.contains("XPMF profiles written: 1")),
        "{notes:?}"
    );
}

#[test]
fn identical_file_adopted() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join(FILE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "same\n").unwrap();
    let mut notes = vec![];
    let shas = finish(&pending(d.path(), "same\n"), &BTreeMap::new(), &mut notes);
    assert_eq!(
        shas[&record_key(&path)],
        sha256_of(&path).unwrap(),
        "{notes:?}"
    );
}

/// Review Focus 1: two sets that fold to one file name are both refused.
#[test]
fn two_sets_one_name_refused() {
    let d = tempfile::tempdir().unwrap();
    let sub = d.path().join("textures/landscape/sub");
    std::fs::create_dir_all(&sub).unwrap();
    ImageBuffer::from_fn(64, 64, |x, y| Rgb([x as u8, y as u8, 7u8]))
        .save(sub.join("Rock01.png"))
        .unwrap();
    let mut m = opened(d.path(), true);
    let other = set_views(&m)
        .into_iter()
        .find(|v| v.id != ID && v.id.to_lowercase().ends_with("/rock01"))
        .unwrap_or_else(|| {
            panic!(
                "no second rock01 set: {:?}",
                set_views(&m).iter().map(|v| &v.id).collect::<Vec<_>>()
            )
        })
        .id;
    set_block(&mut m, ID, json!({}));
    set_block(&mut m, &other, json!({"editorIds": ["MatoC"]}));
    let p = prepare(&m).unwrap();
    assert!(p.files.is_empty(), "{:?}", p.files);
    assert_eq!(p.refused, [FILE.to_string()]);
    for id in [ID, other.as_str()] {
        assert!(
            p.notes
                .iter()
                .any(|n| n.starts_with(&format!("{id}: ")) && n.contains("tessalith_rock01.json")),
            "{id}: {:?}",
            p.notes
        );
    }
}

/// Review Focus 3: a hand-edited out-of-range or wrongly typed value writes nothing and the note quotes the error.
#[test]
fn invalid_value_not_written() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path(), true);
    for (b, key) in [
        (json!({"shelterFade": 500}), "shelterFade"),
        (json!({"noiseUVScale": 0}), "noiseUVScale"),
        (json!({"pbr": "yes"}), "pbr"),
    ] {
        set_block(&mut m, ID, b.clone());
        let p = prepare(&m).unwrap();
        assert!(p.files.is_empty(), "{b}: {:?}", p.files);
        assert_eq!(p.refused, [FILE.to_string()], "{b}");
        assert!(
            p.notes.iter().any(
                |n| n.starts_with(&format!("{ID}: XPMF profile not written: ")) && n.contains(key)
            ),
            "{b}: {:?}",
            p.notes
        );
    }
}

/// Review Focus 5.
#[test]
fn no_editor_ids_not_written() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path(), false);
    set_block(&mut m, ID, json!({}));
    let note = format!("{ID}: no editorIds; XPMF profile not written");
    assert_eq!(profile(&m, &entry(&m, ID)), Err(note.clone()));
    let p = prepare(&m).unwrap();
    assert!(p.files.is_empty());
    assert!(p.notes.contains(&note), "{:?}", p.notes);
}

#[test]
fn unexported_texture_blocks_file() {
    let d = tempfile::tempdir().unwrap();
    let mut p = pending(d.path(), "{}\n");
    let tex = "textures/landscape/rock01_noise.dds";
    p.own_textures.insert(FILE.into(), vec![tex.into()]);
    let mut notes = vec![];
    finish(&p, &BTreeMap::new(), &mut notes);
    let path = d.path().join(FILE);
    assert!(!path.exists());
    assert!(
        notes.contains(&format!(
            "{}: {tex} was not exported; JSON not written",
            path.display()
        )),
        "{notes:?}"
    );
    // Once the texture is on disk the file is written.
    let dds = d.path().join(tex);
    std::fs::create_dir_all(dds.parent().unwrap()).unwrap();
    std::fs::write(&dds, b"DDS ").unwrap();
    let mut notes = vec![];
    finish(&p, &BTreeMap::new(), &mut notes);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}\n", "{notes:?}");
}

#[test]
fn non_landscape_set_not_written() {
    let d = tempfile::tempdir().unwrap();
    let mut m = opened(d.path(), true);
    m.set_shader(ID, Some(engine::shader::Shader::Pbr));
    assert!(m.wait_idle(Duration::from_secs(60)));
    set_block(&mut m, ID, json!({}));
    let note = format!("{ID}: not a landscape set; XPMF profile not written");
    let p = prepare(&m).unwrap();
    assert!(p.files.is_empty());
    assert!(p.notes.contains(&note), "{:?}", p.notes);
}

#[test]
fn no_block_no_pending() {
    let d = tempfile::tempdir().unwrap();
    let m = opened(d.path(), true);
    assert_eq!(prepare(&m), None);
}

/// The validator's new keywords on the XPMF schema (the keyword coverage itself is pinned in pgp.rs):
/// `minLength`, `exclusiveMinimum`, and `$ref` into `#/definitions/...` (including through `oneOf`).
#[test]
fn validator_knows_xpmf_keywords() {
    let ok = |v: Json| validate_xpmf(&v);
    assert_eq!(ok(json!({"name": "n", "editorIds": ["x"]})), Ok(()));
    assert_eq!(
        ok(
            json!({"name": "n", "editorIds": ["x"], "excludeEditorIds": null, "noiseTexture": null,
                  "noiseUVScale": 0.001})
        ),
        Ok(())
    );
    for (bad, why) in [
        (json!({"name": "", "editorIds": ["x"]}), "$.name"),
        (
            json!({"name": "n", "editorIds": ["x"], "noiseUVScale": 0}),
            "$.noiseUVScale",
        ),
        (json!({"name": "n", "editorIds": "x"}), "$.editorIds"),
        (json!({"name": "n", "editorIds": [1]}), "$.editorIds[0]"),
        (
            json!({"name": "n", "editorIds": ["x"], "excludeEditorIds": "x"}),
            "$.excludeEditorIds",
        ),
        (
            json!({"name": "n", "editorIds": ["x"], "shelterFade": 500}),
            "$.shelterFade",
        ),
        (json!({"name": "n"}), "missing 'editorIds'"),
        (
            json!({"name": "n", "editorIds": ["x"], "isSnow": true}),
            "unknown key 'isSnow'",
        ),
    ] {
        let e = validate_xpmf(&bad).unwrap_err();
        assert!(e.iter().any(|x| x.contains(why)), "{bad}: {e:?}");
    }
}

#[test]
fn fields_cover_the_schema() {
    let schema: Json = serde_json::from_str(XPMF_SCHEMA_JSON).unwrap();
    let mut keys: Vec<&str> = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .filter(|k| *k != "$schema")
        .collect();
    keys.sort();
    let mut fields: Vec<&str> = FIELDS.iter().map(|f| f.key).collect();
    fields.sort();
    assert_eq!(fields, keys);
    let preview: Vec<&str> = FIELDS.iter().filter(|f| f.preview).map(|f| f.key).collect();
    assert_eq!(
        preview,
        [
            "diffuseTexture",
            "normalTexture",
            "noiseTexture",
            "detailNormalTexture",
            "falloffScale",
            "falloffBias",
            "noiseUVScale"
        ]
    );
    // The ranges follow the schema.
    for f in FIELDS {
        let s = &schema["properties"][f.key];
        if let FieldKind::Number {
            min,
            max,
            exclusive_min,
        } = f.kind
        {
            let lo = s
                .get("minimum")
                .or(s.get("exclusiveMinimum"))
                .and_then(Json::as_f64);
            assert_eq!(lo.unwrap_or(f64::NEG_INFINITY), min, "{}", f.key);
            assert_eq!(s.get("maximum").and_then(Json::as_f64), max, "{}", f.key);
            assert_eq!(
                s.get("exclusiveMinimum").is_some(),
                exclusive_min,
                "{}",
                f.key
            );
        }
    }
}
