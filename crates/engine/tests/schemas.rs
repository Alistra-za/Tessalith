use engine::schemas::*;
use std::path::Path;

fn schema_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema")
}

/// A type change without a schema update fails here: the committed files must be exactly what the types
/// generate, byte for byte (`.gitattributes` pins `schema/*.json` to LF so a Windows checkout matches too).
#[test]
fn committed_schemas_match_the_types() {
    for kind in SchemaKind::FILES {
        let path = schema_dir().join(kind.file_name());
        let committed = std::fs::read(&path).unwrap_or_default();
        assert!(
            committed == schema_json(kind).as_bytes(),
            "schema/{} is stale: run `sdc presets schema all --out schema`",
            kind.file_name()
        );
    }
}

/// Unit enums already written as `oneOf` in a committed schema: (file, path of the definition). Ruling R4
/// (skill-checks plan): the committed schema stays as it is, and switching these variants' docs to `//` is
/// the owner's call.
const UNIT_ENUMS_AS_ONE_OF: &[(&str, &str)] = &[
    // `Need`: its variants carry `///` docs, so the schema is `oneOf` instead of an `enum` list.
    ("event.schema.json", "$defs/Need"),
    // `RecipeKind`: same.
    ("event.schema.json", "$defs/RecipeKind"),
    // `RecipeStatus`: same.
    ("event.schema.json", "$defs/RecipeStatus"),
];

/// Collects the path of every `oneOf` whose members are all objects with a string `const`.
fn unit_enum_one_ofs(node: &serde_json::Value, path: &str, out: &mut Vec<String>) {
    match node {
        serde_json::Value::Object(map) => {
            if let Some(members) = map.get("oneOf").and_then(|m| m.as_array()) {
                let all_const = !members.is_empty()
                    && members
                        .iter()
                        .all(|m| m.get("const").is_some_and(|c| c.is_string()));
                if all_const {
                    out.push(path.to_string());
                }
            }
            for (k, v) in map {
                unit_enum_one_ofs(v, &format!("{path}/{k}"), out);
            }
        }
        serde_json::Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                unit_enum_one_ofs(v, &format!("{path}/{i}"), out);
            }
        }
        _ => {}
    }
}

/// A `///` doc comment on an enum variant turns the enum's schema into `oneOf` (one object per variant with a
/// string `const`) instead of a plain `enum` list. Data-carrying enums are `oneOf` by nature and do not match.
#[test]
fn a_unit_enum_is_never_written_as_one_of_in_a_committed_schema() {
    for kind in SchemaKind::FILES {
        let file = kind.file_name();
        let text = std::fs::read_to_string(schema_dir().join(&file)).expect("committed schema");
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut hits = Vec::new();
        unit_enum_one_ofs(&v, "", &mut hits);
        for hit in hits {
            let path = hit.trim_start_matches('/');
            assert!(
                UNIT_ENUMS_AS_ONE_OF.contains(&(file.as_str(), path)),
                "schema/{file}: `{path}` is a unit enum written as `oneOf`, because its variants have `///` docs. \
                 Use `//` on the variants, then regenerate with `sdc presets schema all --out schema`."
            );
        }
    }
}

#[test]
fn schema_dir_holds_exactly_the_published_files() {
    let mut found: Vec<String> = std::fs::read_dir(schema_dir())
        .expect("schema/ exists")
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    let mut expected: Vec<String> = SchemaKind::FILES.iter().map(|k| k.file_name()).collect();
    expected.sort();
    assert_eq!(found, expected);
}

#[test]
fn schema_json_is_deterministic_lf_and_newline_terminated() {
    for kind in SchemaKind::FILES {
        let a = schema_json(kind);
        assert_eq!(a, schema_json(kind));
        assert!(a.ends_with("}\n") && !a.contains('\r'), "{kind}");
        let v: serde_json::Value = serde_json::from_str(&a).unwrap();
        assert!(
            v.get("$schema").is_some() || v.get("title").is_some(),
            "{kind}"
        );
    }
}

#[test]
fn write_all_writes_every_file_and_names_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    write_all(dir.path()).unwrap();
    for kind in SchemaKind::FILES {
        let text = std::fs::read_to_string(dir.path().join(kind.file_name())).unwrap();
        assert_eq!(text, schema_json(kind));
        assert_eq!(kind.name().parse::<SchemaKind>(), Ok(kind));
        assert_eq!(kind.to_string(), kind.name());
    }
    assert_eq!("all".parse::<SchemaKind>(), Ok(SchemaKind::All));
    assert!("nope".parse::<SchemaKind>().is_err());
}

/// The shapes added in review fixes are in the schemas (the verbs' reports are checked against their types in
/// the CLI tests).
#[test]
fn schemas_cover_the_reviewed_fields() {
    let event = schema_json(SchemaKind::Event);
    for needle in ["skipped_intentional", "file_skipped", "batch_done"] {
        assert!(event.contains(needle), "event schema lacks {needle}");
    }
    let manifest = schema_json(SchemaKind::Manifest);
    for needle in ["sources", "bc1_model", "alias_chain"] {
        assert!(manifest.contains(needle), "manifest schema lacks {needle}");
    }
    assert!(schema_json(SchemaKind::Preset).contains("suffix_aliases"));
    let info = schema_json(SchemaKind::DdsInfo);
    assert!(info.contains("dds_info") && info.contains("header_issues"));
    assert!(schema_json(SchemaKind::DdsValidation).contains("dds_validation"));
}

/// Final review Minor 2: `schema_version = 0` is refused by the version check, so the config schema says so.
#[test]
fn config_schema_version_minimum_is_one() {
    let v: serde_json::Value = serde_json::from_str(&schema_json(SchemaKind::Config)).unwrap();
    let sv = &v["properties"]["schema_version"];
    assert_eq!(sv["minimum"], 1, "{sv}");
}

/// Audit REPORT #17 (Note BC): every settable field (`FIELD_PATHS`, which `[files]` entries accept) is in the
/// config schema's `[files]` entry, including `reencode`; `subtype` is not a `[files]` field.
#[test]
fn the_files_entry_schema_accepts_every_field_path() {
    let v: serde_json::Value = serde_json::from_str(&schema_json(SchemaKind::Config)).unwrap();
    let defs = &v["$defs"];
    let resolve = |s: &serde_json::Value| -> serde_json::Value {
        match s["$ref"].as_str() {
            Some(r) => defs[r.trim_start_matches("#/$defs/")].clone(),
            None => s.clone(),
        }
    };
    for path in engine::overrides::FIELD_PATHS {
        if *path == "subtype" {
            continue;
        }
        let mut node = defs["FileEntry"].clone();
        for part in path.split('.') {
            let next = resolve(&node)["properties"][part].clone();
            assert!(!next.is_null(), "[files] schema lacks {path}");
            node = next;
        }
    }
}
