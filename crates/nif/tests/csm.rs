// crates/nif/tests/csm.rs
//! The `.csm` material model (Note IE1): parse, per-file checks, the schema's allOf rules, writing, colours, slots;
//! root chains (Note IE2) and the PBR value bridge (Note IE3).
#[path = "common/v1_fixtures.rs"]
mod v1_fixtures;
use nif::csm::{self, Csm, CsmError, Kind, Value, KEYS, VERSION};
use nif::path::{canon_material, canon_root};
use nif::pbr2::{csm_from_values, feat, pbr_values, v1_values, PbrValues, PBR2_DEFAULTS};

/// The example from CS's proposal (csm-spec.md), as written there.
const SPEC_EXAMPLE: &str = r##"{
  "$schema": "<TBD>",
  "Version": 1,
  "RootMaterialPath": "",

  "DiffuseTexture": "pbr/example/material.dds",
  "NormalTexture": "pbr/example/material_n.dds",
  "GlowTexture": "pbr/example/material_g.dds",
  "DisplacementTexture": "pbr/example/material_p.dds",
  "RmaosTexture": "pbr/example/material_rmaos.dds",
  "Features0Texture": "pbr/example/material_s.dds",
  "Features1Texture": "pbr/example/material_f.dds",

  "TileU": true,
  "TileV": true,
  "UOffset": 0.0,
  "VOffset": 0.0,
  "UScale": 1.0,
  "VScale": 1.0,
  "Alpha": 1.0,

  "TwoSided": false,
  "Decal": false,
  "DynamicDecal": false,
  "ModelSpaceNormals": false,
  "VertexColors": false,
  "CastShadows": true,
  "ReceiveShadows": true,
  "ZBufferTest": true,
  "ZBufferWrite": true,

  "ExternalEmittance": false,
  "EmittanceColor": "#ffffff",
  "EmittanceMult": 1.0,

  "PBREnabled": true,
  "PBRDisplacementTextureScale": 1.0,
  "PBRSpecularLevel": 0.04,
  "PBRRoughnessScale": 1.0,

  "PBRSubsurfaceEnabled": true,
  "PBRSubsurfaceColor": "#e68066",
  "PBRSubsurfaceOpacity": 0.3,

  "PBRThinSurfaceEnabled": false,
  "PBRThinSurfaceSubsurfaceBalance": 0.75,
  "PBRThinSurfaceSpecularWeight": 0.5,
  "PBRThinSurfaceBackSpecularWeight": 0.5,
  "PBRThinSurfaceBackRoughnessScale": 1.0,

  "PBRTwoLayerEnabled": false,
  "PBRTwoLayerColoredCoat": false,
  "PBRTwoLayerInterlayerParallax": false,
  "PBRTwoLayerCoatNormal": false,
  "PBRTwoLayerCoatColor": "#ffffff",
  "PBRTwoLayerCoatStrength": 1.0,
  "PBRTwoLayerCoatRoughness": 1.0,
  "PBRTwoLayerCoatSpecularLevel": 0.04,

  "PBRFuzzEnabled": true,
  "PBRFuzzColor": "#e6d9cc",
  "PBRFuzzWeight": 0.6,
  "PBRFuzzRoughness": -1.0,
  "PBRFuzzRoughnessTexture": false,

  "PBRGlintEnabled": false,
  "PBRGlintScreenSpaceScale": 1.5,
  "PBRGlintLogMicrofacetDensity": 40.0,
  "PBRGlintMicrofacetRoughness": 0.015,
  "PBRGlintDensityRandomization": 2.0,

  "PBRHairMarschner": false
}"##;

/// A version-1 file holding `body` (comma-separated `"Key": value` pairs).
fn file(body: &str) -> Result<Csm, CsmError> {
    csm::parse(&format!("{{\"Version\": 1, {body}}}"))
}

/// The key names of a JSON text, in order, one key per line as `to_text` and the spec example write them.
fn key_order(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.trim_start().strip_prefix('"'))
        .filter_map(|l| l.split('"').next())
        .map(str::to_string)
        .collect()
}

#[test]
fn parse_spec_example() {
    let c = csm::parse(SPEC_EXAMPLE).unwrap();
    assert_eq!(c.version, 1);
    // Every schema key but `$schema` and `Version`; only PBRFoliageEnabled (ours) is missing.
    assert_eq!(c.values.len(), KEYS.len() - 1);
    assert!(!c.values.contains_key("PBRFoliageEnabled"));
    assert_eq!(
        c.values["PBRSubsurfaceColor"],
        Value::Color([0xe6, 0x80, 0x66])
    );
    assert_eq!(c.values["EmittanceColor"], Value::Color([0xff, 0xff, 0xff]));
    assert_eq!(c.values["RootMaterialPath"], Value::Text(String::new()));
    assert_eq!(
        c.values["RmaosTexture"],
        Value::Text("pbr/example/material_rmaos.dds".into())
    );
    assert_eq!(c.values["TileU"], Value::Bool(true));
    assert_eq!(c.values["PBRFuzzRoughness"], Value::Num(-1.0));
    assert_eq!(c.values["PBRGlintLogMicrofacetDensity"], Value::Num(40.0));
    // The example turns on subsurface and fuzz together, which the schema allows.
    assert!(csm::rule_errors(&c).is_empty());
}

#[test]
fn round_trip() {
    let c = csm::parse(SPEC_EXAMPLE).unwrap();
    let text = csm::to_text(&c);
    assert_eq!(csm::parse(&text), Ok(c));
    // Version first, then the keys in the spec example's order; no `$schema` while SCHEMA_URL is empty.
    assert_eq!(csm::SCHEMA_URL, "");
    assert!(!text.contains("$schema"));
    let mut expected = key_order(SPEC_EXAMPLE);
    expected.retain(|k| k != "$schema");
    assert_eq!(key_order(&text), expected);
    assert!(text.starts_with(
        "{\n  \"Version\": 1,\n  \"RootMaterialPath\": \"\",\n  \"DiffuseTexture\": "
    ));
    assert!(text.contains("\n  \"PBRSubsurfaceColor\": \"#e68066\",\n"));
    assert!(text.contains("\n  \"UOffset\": 0.0,\n"));
    assert!(text.ends_with("\n  \"PBRHairMarschner\": false\n}"));

    let one = csm::parse(r#"{"Version":1,"PBRFuzzWeight":0.6}"#).unwrap();
    assert_eq!(
        csm::to_text(&one),
        "{\n  \"Version\": 1,\n  \"PBRFuzzWeight\": 0.6\n}"
    );
    assert_eq!(csm::parse(&csm::to_text(&one)), Ok(one));

    // Colours are written lowercase; a path is escaped as JSON.
    let mixed = file(r##""PBRFuzzColor": "#E6D9CC", "GlowTexture": "a\\b.dds""##).unwrap();
    assert_eq!(
        csm::to_text(&mixed),
        "{\n  \"Version\": 1,\n  \"GlowTexture\": \"a\\\\b.dds\",\n  \"PBRFuzzColor\": \"#e6d9cc\"\n}"
    );
    assert_eq!(csm::parse(&csm::to_text(&mixed)), Ok(mixed));

    // Version 1 is written whatever the struct holds: the model has no other version.
    let empty = Csm::default();
    assert_eq!(csm::to_text(&empty), "{\n  \"Version\": 1\n}");
}

#[test]
fn parse_errors() {
    use CsmError::*;
    let key = |k: &str| k.to_string();
    assert_eq!(csm::parse(r#"{"Version": 2}"#), Err(Version(2)));
    assert_eq!(
        csm::parse(r#"{"TileU": true}"#),
        Err(Missing(key("Version")))
    );
    assert_eq!(
        csm::parse(r#"{"Version": "1"}"#),
        Err(Type {
            key: key("Version")
        })
    );
    assert_eq!(
        csm::parse(r#"{"Version": -1}"#),
        Err(Type {
            key: key("Version")
        })
    );
    assert_eq!(file(r#""Foo": 1"#), Err(UnknownKey(key("Foo"))));
    assert_eq!(file(r#""tileu": true"#), Err(UnknownKey(key("tileu"))));
    assert_eq!(
        file(r#""PBRSpecularLevel": 1.5"#),
        Err(Range {
            key: key("PBRSpecularLevel"),
            value: 1.5
        })
    );
    assert_eq!(
        file(r#""PBRFuzzColor": "red""#),
        Err(Pattern {
            key: key("PBRFuzzColor")
        })
    );
    assert_eq!(
        file(r#""DiffuseTexture": "x.png""#),
        Err(Pattern {
            key: key("DiffuseTexture")
        })
    );
    assert_eq!(
        file(r#""TwoSided": 1"#),
        Err(Type {
            key: key("TwoSided")
        })
    );

    // Types per kind.
    assert_eq!(file(r#""UScale": "1""#), Err(Type { key: key("UScale") }));
    assert_eq!(
        file(r#""EmittanceColor": 16777215"#),
        Err(Type {
            key: key("EmittanceColor")
        })
    );
    assert_eq!(
        file(r#""NormalTexture": null"#),
        Err(Type {
            key: key("NormalTexture")
        })
    );
    assert_eq!(
        file(r#""RootMaterialPath": false"#),
        Err(Type {
            key: key("RootMaterialPath")
        })
    );
    assert_eq!(
        file(r#""$schema": 3"#),
        Err(Type {
            key: key("$schema")
        })
    );
    assert!(
        file(r#""$schema": "https://example.invalid/csm.schema.json""#)
            .unwrap()
            .values
            .is_empty()
    );

    // Patterns: `^#[0-9a-fA-F]{6}$`, `^$|\.[dD][dD][sS]$`, `^$|\.[cC][sS][mM]$`.
    for bad in ["#12345", "#1234567", "#12345g", "123456", " #123456"] {
        let body = format!(r#""PBRTwoLayerCoatColor": "{bad}""#);
        assert_eq!(
            file(&body),
            Err(Pattern {
                key: key("PBRTwoLayerCoatColor")
            }),
            "{bad}"
        );
    }
    assert!(file(r##""PBRTwoLayerCoatColor": "#AbCdEf""##).is_ok());
    for good in ["", "x.dds", "a/b/C.DDS", "x.DdS"] {
        assert!(
            file(&format!(r#""GlowTexture": "{good}""#)).is_ok(),
            "{good}"
        );
    }
    assert_eq!(
        file(r#""GlowTexture": "x.dds ""#),
        Err(Pattern {
            key: key("GlowTexture")
        })
    );
    assert_eq!(
        file(r#""GlowTexture": "xdds""#),
        Err(Pattern {
            key: key("GlowTexture")
        })
    );
    for good in ["", "base.csm", "sub/Base.CSM"] {
        assert!(
            file(&format!(r#""RootMaterialPath": "{good}""#)).is_ok(),
            "{good}"
        );
    }
    assert_eq!(
        file(r#""RootMaterialPath": "base.json""#),
        Err(Pattern {
            key: key("RootMaterialPath")
        })
    );

    // Ranges are inclusive; one-sided ones have no other bound.
    assert!(file(r#""Alpha": 128"#).is_ok());
    assert_eq!(
        file(r#""Alpha": 128.5"#),
        Err(Range {
            key: key("Alpha"),
            value: 128.5
        })
    );
    assert!(file(r#""PBRFuzzRoughness": -50"#).is_ok());
    assert_eq!(
        file(r#""PBRFuzzRoughness": 1.01"#),
        Err(Range {
            key: key("PBRFuzzRoughness"),
            value: 1.01
        })
    );
    assert!(file(r#""PBRGlintScreenSpaceScale": 1e6"#).is_ok());
    assert_eq!(
        file(r#""PBRGlintScreenSpaceScale": 0.99"#),
        Err(Range {
            key: key("PBRGlintScreenSpaceScale"),
            value: 0.99
        })
    );
    assert!(file(r#""UOffset": -1e9"#).is_ok());
    assert_eq!(
        file(r#""EmittanceMult": -0.1"#),
        Err(Range {
            key: key("EmittanceMult"),
            value: -0.1
        })
    );

    assert!(UnknownKey(key("Foo")).to_string().contains("Foo"));
    assert!(Range {
        key: key("Alpha"),
        value: 200.0
    }
    .to_string()
    .contains("Alpha"));
}

#[test]
fn ranges_match_the_schema() {
    // The schema's minimum / maximum per key (inf = no bound); every other key has none.
    let inf = f64::INFINITY;
    let expected: &[(&str, f64, f64)] = &[
        ("Alpha", 0.0, 128.0),
        ("EmittanceMult", 0.0, inf),
        ("PBRDisplacementTextureScale", 0.0, inf),
        ("PBRSpecularLevel", 0.0, 1.0),
        ("PBRRoughnessScale", 0.0, inf),
        ("PBRSubsurfaceOpacity", 0.0, 1.0),
        ("PBRThinSurfaceSubsurfaceBalance", 0.0, 1.0),
        ("PBRThinSurfaceSpecularWeight", 0.0, 1.0),
        ("PBRThinSurfaceBackSpecularWeight", 0.0, 1.0),
        ("PBRThinSurfaceBackRoughnessScale", 0.0, 3.0),
        ("PBRTwoLayerCoatStrength", 0.0, 1.0),
        ("PBRTwoLayerCoatRoughness", 0.0, 1.0),
        ("PBRTwoLayerCoatSpecularLevel", 0.0, 1.0),
        ("PBRFuzzWeight", 0.0, 1.0),
        ("PBRFuzzRoughness", -inf, 1.0),
        ("PBRGlintScreenSpaceScale", 1.0, inf),
        ("PBRGlintLogMicrofacetDensity", 1.0, 40.0),
        ("PBRGlintMicrofacetRoughness", 0.005, 0.3),
        ("PBRGlintDensityRandomization", 0.0, 5.0),
    ];
    let got: Vec<(&str, f64, f64)> = KEYS
        .iter()
        .filter_map(|k| k.range.map(|(lo, hi)| (k.name, lo, hi)))
        .collect();
    assert_eq!(got, expected);
    assert!(KEYS
        .iter()
        .all(|k| k.range.is_none() || k.kind == Kind::Num));
}

#[test]
fn keys_table() {
    assert_eq!(KEYS.len(), 59);
    let mut names: Vec<&str> = KEYS.iter().map(|k| k.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), KEYS.len());
    let generic: Vec<&str> = KEYS.iter().filter(|k| k.generic).map(|k| k.name).collect();
    assert_eq!(
        generic,
        [
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
        ]
    );
    let at = |n: &str| KEYS.iter().position(|k| k.name == n).unwrap();
    assert_eq!(at("PBRFoliageEnabled"), at("PBRSubsurfaceOpacity") + 1);
    let foliage = &KEYS[at("PBRFoliageEnabled")];
    assert_eq!(
        (foliage.kind, foliage.slot, foliage.generic),
        (Kind::Bool, None, false)
    );
    assert_eq!(KEYS[at("RootMaterialPath")].kind, Kind::Path);
    assert_eq!(KEYS[0].name, "RootMaterialPath");
    assert_eq!(KEYS[KEYS.len() - 1].name, "PBRHairMarschner");
    assert_eq!(VERSION, 1);
}

#[test]
fn parse_bom_and_broken_json() {
    let c = csm::parse("\u{feff}{\"Version\":1}").unwrap();
    assert_eq!(
        c,
        Csm {
            version: 1,
            values: Default::default()
        }
    );
    assert!(matches!(
        csm::parse(r#"{"Version":1,}"#),
        Err(CsmError::Json(_))
    ));
    assert!(matches!(
        csm::parse(r#"{"Version":1,"TileU":tr"#),
        Err(CsmError::Json(_))
    ));
    assert!(matches!(csm::parse(r#"[1]"#), Err(CsmError::Json(_))));
    assert!(matches!(csm::parse(""), Err(CsmError::Json(_))));
}

#[test]
fn rules() {
    use csm::rule::*;
    let errs = |body: &str| csm::rule_errors(&file(body).unwrap());
    let none: [&str; 0] = [];

    for other in [
        "PBRSubsurfaceEnabled",
        "PBRThinSurfaceEnabled",
        "PBRTwoLayerEnabled",
        "PBRTwoLayerColoredCoat",
        "PBRTwoLayerInterlayerParallax",
        "PBRTwoLayerCoatNormal",
        "PBRFuzzEnabled",
        "PBRGlintEnabled",
        "PBRFoliageEnabled",
    ] {
        assert_eq!(
            errs(&format!(r#""PBRHairMarschner": true, "{other}": true"#)),
            [HAIR_ALONE],
            "{other}"
        );
        assert_eq!(
            errs(&format!(r#""PBRHairMarschner": true, "{other}": false"#)),
            none,
            "{other}"
        );
        assert_eq!(
            errs(&format!(r#""PBRHairMarschner": false, "{other}": true"#)),
            none,
            "{other}"
        );
    }
    for other in [
        "PBRSubsurfaceEnabled",
        "PBRThinSurfaceEnabled",
        "PBRFuzzEnabled",
        "PBRGlintEnabled",
        "PBRFoliageEnabled",
    ] {
        assert_eq!(
            errs(&format!(r#""PBRTwoLayerEnabled": true, "{other}": true"#)),
            [TWO_LAYER_EXCLUDES],
            "{other}"
        );
    }
    assert_eq!(
        errs(r#""PBRFuzzEnabled": true, "PBRGlintEnabled": true"#),
        [FUZZ_GLINT]
    );
    assert_eq!(
        errs(r#""PBRFuzzEnabled": true, "PBRGlintEnabled": false"#),
        none
    );

    assert_eq!(
        errs(r#""PBRTwoLayerColoredCoat": true, "PBRTwoLayerEnabled": false"#),
        [COLORED_COAT_NEEDS_TWO_LAYER]
    );
    assert_eq!(
        errs(r#""PBRTwoLayerInterlayerParallax": true, "PBRTwoLayerEnabled": false"#),
        [PARALLAX_NEEDS_TWO_LAYER]
    );
    assert_eq!(
        errs(r#""PBRTwoLayerCoatNormal": true, "PBRTwoLayerEnabled": false"#),
        [COAT_NORMAL_NEEDS_TWO_LAYER]
    );
    assert_eq!(
        errs(r#""PBRTwoLayerColoredCoat": true, "PBRTwoLayerEnabled": true"#),
        none
    );

    for tex in ["DiffuseTexture", "NormalTexture", "RmaosTexture"] {
        assert_eq!(
            errs(&format!(r#""PBREnabled": true, "{tex}": """#)),
            [PBR_NEEDS_TEXTURES],
            "{tex}"
        );
        assert_eq!(
            errs(&format!(r#""PBREnabled": false, "{tex}": """#)),
            none,
            "{tex}"
        );
    }
    assert_eq!(
        errs(
            r#""PBRTwoLayerEnabled": true, "PBRTwoLayerInterlayerParallax": true, "DisplacementTexture": """#
        ),
        [PARALLAX_NEEDS_DISPLACEMENT]
    );
    assert_eq!(
        errs(
            r#""PBRTwoLayerEnabled": true, "PBRTwoLayerCoatNormal": true, "Features1Texture": """#
        ),
        [COAT_NORMAL_NEEDS_FEATURES1]
    );

    // As in the schema, a rule only checks keys the file holds: an absent key comes from the root or the NIF.
    assert_eq!(errs(r#""PBRHairMarschner": true"#), none);
    assert_eq!(errs(r#""PBRTwoLayerColoredCoat": true"#), none);
    assert_eq!(errs(r#""PBRTwoLayerInterlayerParallax": true"#), none);
    assert_eq!(errs(r#""PBRTwoLayerCoatNormal": true"#), none);
    assert_eq!(errs(r#""PBREnabled": true"#), none);

    // Several broken rules are each listed once, in the schema's order.
    assert_eq!(
        errs(
            r#""PBRHairMarschner": true, "PBRFuzzEnabled": true, "PBRGlintEnabled": true, "PBRSubsurfaceEnabled": true"#
        ),
        [HAIR_ALONE, FUZZ_GLINT]
    );
}

#[test]
fn colour_round_trip() {
    assert_eq!(csm::hex([1.0, 0.5, 0.0]), [255, 128, 0]);
    assert_eq!(csm::hex([0.9, 0.85, 0.8]), [0xe6, 0xd9, 0xcc]);
    for x in [0.0f32, 0.3, 1.0] {
        let back = csm::rgb(csm::hex([x; 3]));
        assert!(
            back.iter().all(|&c| (c - x).abs() <= 1.0 / 510.0),
            "{x} -> {back:?}"
        );
    }
    assert_eq!(csm::hex([-0.5, 1.5, 2.0]), [0, 255, 255]);
    assert_eq!(csm::rgb([255, 0, 51]), [1.0, 0.0, 0.2]);
}

#[test]
fn numbers_write_short() {
    assert_eq!(csm::num(0.6), 0.6f64);
    let mut c = Csm {
        version: VERSION,
        ..Default::default()
    };
    c.values.insert("PBRFuzzWeight", Value::Num(csm::num(0.6)));
    let text = csm::to_text(&c);
    assert!(text.contains("\"PBRFuzzWeight\": 0.6\n"), "{text}");
    assert!(!text.contains("0.6000000238418579"));
}

#[test]
fn slot_map() {
    let slots: Vec<(&str, usize)> = KEYS
        .iter()
        .filter_map(|k| k.slot.map(|s| (k.name, s)))
        .collect();
    assert_eq!(
        slots,
        [
            ("DiffuseTexture", 0),
            ("NormalTexture", 1),
            ("GlowTexture", 2),
            ("DisplacementTexture", 3),
            ("RmaosTexture", 5),
            ("Features0Texture", 7),
            ("Features1Texture", 6),
        ]
    );
    assert!(KEYS
        .iter()
        .all(|k| k.slot.is_some() == (k.kind == Kind::Texture)));
}

#[test]
fn parse_version_as_float() {
    // Draft-07 counts 1.0 as an integer, so the schema accepts it.
    assert_eq!(csm::parse(r#"{"Version": 1.0}"#).map(|c| c.version), Ok(1));
    assert_eq!(csm::parse(r#"{"Version": 2.0}"#), Err(CsmError::Version(2)));
    for bad in ["1.5", "-1", "-1.0"] {
        assert_eq!(
            csm::parse(&format!(r#"{{"Version": {bad}}}"#)),
            Err(CsmError::Type {
                key: "Version".into()
            }),
            "{bad}"
        );
    }
}

#[test]
fn duplicate_keys_last_wins() {
    // serde_json keeps the last of a repeated key; the schema says nothing about repeats.
    let c = file(r#""PBRFuzzWeight": 0.1, "PBRFuzzWeight": 0.2"#).unwrap();
    assert_eq!(c.values["PBRFuzzWeight"], Value::Num(0.2));
}

// --- root chains (Note IE2) ---

/// A reader over in-memory files stored under their canonical paths.
fn reader(files: Vec<(String, String)>) -> impl Fn(&str) -> Option<Vec<u8>> {
    move |p: &str| {
        files
            .iter()
            .find(|(k, _)| k == p)
            .map(|(_, v)| v.clone().into_bytes())
    }
}

fn files(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn resolve_chain_child_wins() {
    let read = reader(files(&[
        (
            "materials/root.csm",
            r#"{"Version": 1, "PBRRoughnessScale": 0.5, "DiffuseTexture": "pbr/a.dds", "PBRSpecularLevel": 0.2}"#,
        ),
        (
            "materials/mid.csm",
            r#"{"Version": 1, "RootMaterialPath": "materials\\root.csm", "PBRSpecularLevel": 0.3}"#,
        ),
        (
            "materials/child.csm",
            r#"{"Version": 1, "RootMaterialPath": "materials\\mid.csm", "PBRRoughnessScale": 0.8}"#,
        ),
    ]));
    let c = csm::resolve("materials/child.csm", &read).unwrap();
    assert_eq!(c.version, VERSION);
    assert_eq!(c.values["PBRRoughnessScale"], Value::Num(0.8));
    assert_eq!(c.values["DiffuseTexture"], Value::Text("pbr/a.dds".into()));
    assert_eq!(c.values["PBRSpecularLevel"], Value::Num(0.3));
    // The merged material is resolved: it holds no root of its own.
    assert!(!c.values.contains_key("RootMaterialPath"));
    assert_eq!(c.values.len(), 3);

    // A file without a root resolves to itself.
    let c = csm::resolve("materials/root.csm", &read).unwrap();
    assert_eq!(c.values.len(), 3);
    assert_eq!(c.values["PBRRoughnessScale"], Value::Num(0.5));
    // An empty RootMaterialPath means no root, and is dropped too.
    let read = reader(files(&[(
        "materials/a.csm",
        r#"{"Version": 1, "RootMaterialPath": "", "PBRFuzzWeight": 0.5}"#,
    )]));
    let c = csm::resolve("materials/a.csm", &read).unwrap();
    assert_eq!(
        c.values.into_iter().collect::<Vec<_>>(),
        [("PBRFuzzWeight", Value::Num(0.5))]
    );
}

#[test]
fn resolve_checks_rules_on_the_merge() {
    let root = r#"{"Version": 1, "PBRTwoLayerEnabled": false}"#;
    let child = r#"{"Version": 1, "RootMaterialPath": "materials/root.csm", "PBRTwoLayerColoredCoat": true}"#;
    // Each file alone breaks no rule.
    for text in [root, child] {
        assert!(csm::rule_errors(&csm::parse(text).unwrap()).is_empty());
    }
    let read = reader(files(&[
        ("materials/root.csm", root),
        ("materials/child.csm", child),
    ]));
    assert_eq!(
        csm::resolve("materials/child.csm", &read),
        Err(CsmError::Rule(csm::rule::COLORED_COAT_NEEDS_TWO_LAYER))
    );
    // A child's explicit empty texture under PBR from the root breaks the PBR rule.
    let read = reader(files(&[
        (
            "materials/root.csm",
            r#"{"Version": 1, "PBREnabled": true, "DiffuseTexture": "a.dds"}"#,
        ),
        (
            "materials/child.csm",
            r#"{"Version": 1, "RootMaterialPath": "materials/root.csm", "DiffuseTexture": ""}"#,
        ),
    ]));
    assert_eq!(
        csm::resolve("materials/child.csm", &read),
        Err(CsmError::Rule(csm::rule::PBR_NEEDS_TEXTURES))
    );
}

/// `n` files `materials/c0.csm` .. `c{n-1}`, each with the next as its root.
fn chain(n: usize) -> Vec<(String, String)> {
    (0..n)
        .map(|i| {
            let root = if i + 1 < n {
                format!(r#", "RootMaterialPath": "materials\\c{}.csm""#, i + 1)
            } else {
                String::new()
            };
            (
                format!("materials/c{i}.csm"),
                format!(r#"{{"Version": 1, "PBRFuzzWeight": 0.{i:02}{root}}}"#),
            )
        })
        .collect()
}

#[test]
fn resolve_rejects_loops_and_depth() {
    let read = reader(files(&[(
        "materials/a.csm",
        r#"{"Version": 1, "RootMaterialPath": "materials/a.csm"}"#,
    )]));
    assert_eq!(
        csm::resolve("materials/a.csm", &read),
        Err(CsmError::Loop("materials/a.csm".into()))
    );
    // A -> B -> A, the second step written with other case and separators.
    let read = reader(files(&[
        (
            "materials/a.csm",
            r#"{"Version": 1, "RootMaterialPath": "Materials\\B.csm"}"#,
        ),
        (
            "materials/b.csm",
            r#"{"Version": 1, "RootMaterialPath": "DATA\\materials\\A.CSM"}"#,
        ),
    ]));
    assert_eq!(
        csm::resolve("materials/a.csm", &read),
        Err(CsmError::Loop("materials/a.csm".into()))
    );
    // 16 files resolve; 17 are too deep.
    let read = reader(chain(17));
    assert_eq!(
        csm::resolve("materials/c0.csm", &read),
        Err(CsmError::Depth)
    );
    let c = csm::resolve("materials/c1.csm", &read).unwrap();
    assert_eq!(c.values["PBRFuzzWeight"], Value::Num(0.01));
}

#[test]
fn resolve_root_errors_name_the_file() {
    let root_err = |reason: CsmError| CsmError::Root {
        path: "materials/root.csm".into(),
        reason: Box::new(reason),
    };
    let child = r#"{"Version": 1, "RootMaterialPath": "Materials\\Root.csm"}"#;
    let read = reader(files(&[
        ("materials/root.csm", r#"{"Version": 2}"#),
        ("materials/child.csm", child),
    ]));
    assert_eq!(
        csm::resolve("materials/child.csm", &read),
        Err(root_err(CsmError::Version(2)))
    );
    // The file asked for reports its own errors bare.
    assert_eq!(
        csm::resolve("materials/root.csm", &read),
        Err(CsmError::Version(2))
    );
    // A missing root.
    let read = reader(files(&[("materials/child.csm", child)]));
    assert_eq!(
        csm::resolve("materials/child.csm", &read),
        Err(root_err(CsmError::NotFound("materials/root.csm".into())))
    );
    assert_eq!(
        csm::resolve("materials/none.csm", &read),
        Err(CsmError::NotFound("materials/none.csm".into()))
    );
    assert_eq!(
        csm::resolve("textures/a.csm", &read),
        Err(CsmError::NotMaterial("textures/a.csm".into()))
    );
    // A root deeper in the chain is named itself, not wrapped once per level.
    let read = reader(files(&[
        ("materials/child.csm", child),
        (
            "materials/root.csm",
            r#"{"Version": 1, "RootMaterialPath": "materials/base.csm"}"#,
        ),
        ("materials/base.csm", r#"{"Version": 1, "Foo": 1}"#),
    ]));
    assert_eq!(
        csm::resolve("materials/child.csm", &read),
        Err(CsmError::Root {
            path: "materials/base.csm".into(),
            reason: Box::new(CsmError::UnknownKey("Foo".into()))
        })
    );
    // A missing root named relative to materials is named by its canonical path.
    let read = reader(files(&[(
        "materials/child.csm",
        r#"{"Version": 1, "RootMaterialPath": "base.csm"}"#,
    )]));
    assert_eq!(
        csm::resolve("materials/child.csm", &read),
        Err(CsmError::Root {
            path: "materials/base.csm".into(),
            reason: Box::new(CsmError::NotFound("materials/base.csm".into()))
        })
    );
    // Bytes that are not UTF-8 are not JSON.
    let read = |_: &str| Some(vec![b'{', 0xff, b'}']);
    assert!(matches!(
        csm::resolve("materials/a.csm", &read),
        Err(CsmError::Json(_))
    ));
    assert!(root_err(CsmError::Version(2))
        .to_string()
        .contains("materials/root.csm"));
}

#[test]
fn resolve_root_path_forms() {
    // Data-relative (`materials\…`) and materials-relative (`pbr\…`) roots reach the same file.
    for root in [
        "pbr/base.csm",
        "PBR\\Base.CSM",
        "materials\\pbr\\base.csm",
        "Data\\Materials\\PBR\\base.csm",
        "\\pbr\\base.csm",
    ] {
        let child =
            format!(r#"{{"Version": 1, "RootMaterialPath": {root:?}, "PBRFuzzWeight": 0.5}}"#);
        let read = reader(files(&[
            ("materials/child.csm", child.as_str()),
            (
                "materials/pbr/base.csm",
                r#"{"Version": 1, "PBRSpecularLevel": 0.2}"#,
            ),
        ]));
        let c = csm::resolve("materials/child.csm", &read).unwrap();
        assert_eq!(c.values["PBRSpecularLevel"], Value::Num(0.2), "{root}");
        assert_eq!(c.values["PBRFuzzWeight"], Value::Num(0.5), "{root}");
    }
    let some = |s: &str| Some(s.to_string());
    assert_eq!(canon_root("pbr/base.csm"), some("materials/pbr/base.csm"));
    assert_eq!(
        canon_root("materials\\pbr\\base.csm"),
        some("materials/pbr/base.csm")
    );
    // Not a `.csm`, or empty (no root): no path.
    assert_eq!(canon_root("pbr/base.json"), None);
    assert_eq!(canon_root(""), None);
    assert!(CsmError::RootPath("x.json".into())
        .to_string()
        .contains("RootMaterialPath x.json"));
}

#[test]
fn resolve_folds_case_and_separators() {
    let read = reader(files(&[(
        "materials/pbr/x.csm",
        r#"{"Version": 1, "PBRFuzzWeight": 0.5}"#,
    )]));
    for p in [
        "Materials\\PBR\\X.CSM",
        "data\\materials\\pbr\\x.csm",
        "materials/pbr/x.csm",
    ] {
        let c = csm::resolve(p, &read).unwrap();
        assert_eq!(c.values["PBRFuzzWeight"], Value::Num(0.5), "{p}");
    }
}

#[test]
fn canon_material_cases() {
    let some = |s: &str| Some(s.to_string());
    assert_eq!(
        canon_material("data\\materials\\a.csm"),
        some("materials/a.csm")
    );
    assert_eq!(
        canon_material("Data/Materials/A.CSM"),
        some("materials/a.csm")
    );
    assert_eq!(
        canon_material("Materials\\PBR\\Clutter\\Rug01.csm"),
        some("materials/pbr/clutter/rug01.csm")
    );
    assert_eq!(canon_material("materials/a.csm"), some("materials/a.csm"));
    assert_eq!(canon_material("textures/a.csm"), None);
    assert_eq!(canon_material("materials/a.json"), None);
    assert_eq!(canon_material("textures/materials/a.csm"), None);
    assert_eq!(canon_material("a.csm"), None);
    assert_eq!(canon_material("data/data/materials/a.csm"), None);
    assert_eq!(canon_material(""), None);
}

// --- the PBR value bridge (Note IE3) ---

/// The params each feature owns (p0-p2 belong to every PBR material), written out here as the brief gives them.
const OWNERS: [(u32, std::ops::RangeInclusive<usize>); 6] = [
    (0, 0..=2),
    (feat::SUBSURFACE, 3..=6),
    (feat::TWO_LAYER, 7..=12),
    (feat::FUZZ, 13..=16),
    (feat::GLINT, 17..=20),
    (feat::THIN_SURFACE, 21..=24),
];
const COLOUR_PARAMS: [usize; 9] = [3, 4, 5, 7, 8, 9, 13, 14, 15];

fn enabled(features: u32, i: usize) -> bool {
    OWNERS
        .iter()
        .any(|(bit, rows)| rows.contains(&i) && (*bit == 0 || features & bit != 0))
}

fn pbr_file(body: &str) -> Csm {
    file(&format!(r#""PBREnabled": true, {body}"#)).unwrap()
}

#[test]
fn pbr_disabled_is_none() {
    assert_eq!(pbr_values(&file(r#""PBRFuzzWeight": 0.5"#).unwrap()), None);
    assert_eq!(pbr_values(&file(r#""PBREnabled": false"#).unwrap()), None);
    assert_eq!(
        pbr_values(&file(r#""PBREnabled": true"#).unwrap()),
        Some(PbrValues {
            features: 0,
            params: PBR2_DEFAULTS,
            fuzz_roughness: -1.0
        })
    );
}

#[test]
fn pbr_values_maps_keys() {
    let c = pbr_file(
        r##""PBRRoughnessScale": 0.11, "PBRSpecularLevel": 0.12, "PBRDisplacementTextureScale": 0.13,
        "PBRSubsurfaceColor": "#ff0033", "PBRSubsurfaceOpacity": 0.16,
        "PBRTwoLayerCoatColor": "#336600", "PBRTwoLayerCoatStrength": 0.2, "PBRTwoLayerCoatRoughness": 0.21,
        "PBRTwoLayerCoatSpecularLevel": 0.22,
        "PBRFuzzColor": "#00ff99", "PBRFuzzWeight": 0.26,
        "PBRGlintScreenSpaceScale": 2.7, "PBRGlintLogMicrofacetDensity": 2.8, "PBRGlintMicrofacetRoughness": 0.29,
        "PBRGlintDensityRandomization": 3.0,
        "PBRThinSurfaceSubsurfaceBalance": 0.31, "PBRThinSurfaceSpecularWeight": 0.32,
        "PBRThinSurfaceBackSpecularWeight": 0.33, "PBRThinSurfaceBackRoughnessScale": 0.34"##,
    );
    let v = pbr_values(&c).unwrap();
    let want: [f32; 25] = [
        0.11, 0.12, 0.13, // p0-p2
        1.0, 0.0, 0.2, 0.16, // subsurface
        0.2, 0.4, 0.0, 0.2, 0.21, 0.22, // coat
        0.0, 1.0, 0.6, 0.26, // fuzz
        2.7, 2.8, 0.29, 3.0, // glint
        0.31, 0.32, 0.33, 0.34, // thin surface
    ];
    assert_eq!(v.params, want);
    assert_eq!(v.features, 0);

    // Each switch, written out here: one key, one bit.
    let switches = [
        ("PBRSubsurfaceEnabled", feat::SUBSURFACE),
        ("PBRTwoLayerEnabled", feat::TWO_LAYER),
        ("PBRTwoLayerColoredCoat", feat::COLORED_COAT),
        ("PBRTwoLayerInterlayerParallax", feat::INTERLAYER_PARALLAX),
        ("PBRTwoLayerCoatNormal", feat::COAT_NORMAL),
        ("PBRFuzzEnabled", feat::FUZZ),
        ("PBRHairMarschner", feat::HAIR),
        ("PBRGlintEnabled", feat::GLINT),
        ("PBRFoliageEnabled", feat::FOLIAGE),
        ("PBRThinSurfaceEnabled", feat::THIN_SURFACE),
        ("PBRFuzzRoughnessTexture", feat::FUZZ_ROUGHNESS_TEXTURE),
    ];
    for (key, bit) in switches {
        let on = pbr_values(&pbr_file(&format!(r#""{key}": true"#))).unwrap();
        assert_eq!(on.features, bit, "{key}");
        let off = pbr_values(&pbr_file(&format!(r#""{key}": false"#))).unwrap();
        assert_eq!(off.features, 0, "{key}");
        // The fuzz roughness texture switch is written only beside fuzz.
        let features = if bit == feat::FUZZ_ROUGHNESS_TEXTURE {
            bit | feat::FUZZ
        } else {
            bit
        };
        let back = csm_from_values(&PbrValues { features, ..on });
        assert_eq!(back.values.get(key), Some(&Value::Bool(true)), "{key}");
        let n_switches = back
            .values
            .iter()
            .filter(|(k, v)| **k != "PBREnabled" && matches!(v, Value::Bool(_)))
            .count();
        assert_eq!(n_switches, features.count_ones() as usize, "{key}");
    }
    assert_eq!(feat::FUZZ_ROUGHNESS_TEXTURE, 1024);
    let lone = csm_from_values(&PbrValues {
        features: feat::FUZZ_ROUGHNESS_TEXTURE,
        params: PBR2_DEFAULTS,
        fuzz_roughness: -1.0,
    });
    assert!(!lone.values.contains_key("PBRFuzzRoughnessTexture"));
    // Every `feat` bit but the texture switch is a payload bit.
    assert_eq!(feat::PAYLOAD_FEATURES, (1 << 10) - 1);
    assert_eq!(feat::FUZZ_ROUGHNESS_TEXTURE & feat::PAYLOAD_FEATURES, 0);
    assert_eq!(
        nif::pbr2::feature_errors(feat::FUZZ | feat::FUZZ_ROUGHNESS_TEXTURE),
        [nif::pbr2::BITS_ABOVE_9]
    );
    assert!(nif::pbr2::feature_errors(
        (feat::FUZZ | feat::FUZZ_ROUGHNESS_TEXTURE) & feat::PAYLOAD_FEATURES
    )
    .is_empty());
}

#[test]
fn values_round_trip() {
    let mut cases: Vec<(String, PbrValues)> = Vec::new();
    for (name, t, f2, p) in v1_fixtures::fixtures() {
        let (v, clamped) = v1_values(t, f2, &p);
        // The fixtures are in range: nothing clamped, colours inside 0-1 so `#rrggbb` holds them.
        assert!(clamped.is_empty(), "{name}: {clamped:?}");
        assert!(
            COLOUR_PARAMS
                .iter()
                .all(|&i| (0.0..=1.0).contains(&v.params[i])),
            "{name}"
        );
        assert_eq!(v.fuzz_roughness, -1.0, "{name}");
        let w = nif::pbr2::from_v1(t, f2, &p);
        assert_eq!((v.features, v.params), (w.features, w.params), "{name}");
        cases.push((name.to_string(), v));
    }
    // Shapes v1 can't make: thin surface, foliage, the fuzz roughness pair, all coat options.
    let mut p = PBR2_DEFAULTS;
    for (i, x) in p.iter_mut().enumerate() {
        *x = 0.01 * (i + 1) as f32;
    }
    (p[17], p[18], p[19]) = (1.7, 18.5, 0.19);
    for (features, fuzz_roughness) in [
        (
            feat::SUBSURFACE | feat::THIN_SURFACE | feat::FOLIAGE | feat::GLINT,
            -1.0,
        ),
        (
            feat::TWO_LAYER | feat::COLORED_COAT | feat::INTERLAYER_PARALLAX | feat::COAT_NORMAL,
            -1.0,
        ),
        (feat::FUZZ | feat::FUZZ_ROUGHNESS_TEXTURE, 0.25),
        (feat::HAIR, -1.0),
    ] {
        cases.push((
            format!("synthetic {features:#x}"),
            PbrValues {
                features,
                params: p,
                fuzz_roughness,
            },
        ));
    }
    for (name, v) in cases {
        let c = csm_from_values(&v);
        assert_eq!(c.values.get("PBREnabled"), Some(&Value::Bool(true)));
        assert!(csm::rule_errors(&c).is_empty(), "{name}");
        assert!(csm::parse(&csm::to_text(&c)).is_ok(), "{name}");
        let w = pbr_values(&c).unwrap();
        assert_eq!(w.features, v.features, "{name}");
        for (i, &default) in PBR2_DEFAULTS.iter().enumerate() {
            if !enabled(v.features, i) {
                // Not written, so read back as the default.
                assert_eq!(w.params[i], default, "{name} p{i}");
            } else if COLOUR_PARAMS.contains(&i) {
                // Half a step of 1/255; a value on the half step (0.9 * 255 = 229.5) adds float error.
                let off = (f64::from(w.params[i]) - f64::from(v.params[i])).abs();
                assert!(
                    off <= 1.0 / 510.0 + 1e-6,
                    "{name} p{i}: {} -> {}",
                    v.params[i],
                    w.params[i]
                );
            } else {
                assert_eq!(w.params[i], v.params[i], "{name} p{i}");
            }
        }
        let fuzz = v.features & feat::FUZZ != 0;
        assert_eq!(
            w.fuzz_roughness,
            if fuzz { v.fuzz_roughness } else { -1.0 },
            "{name}"
        );
        // Only PBR keys and switches: no texture, generic key or root.
        assert!(c.values.keys().all(|k| KEYS
            .iter()
            .any(|d| d.name == *k && !d.generic && d.slot.is_none())
            && *k != "RootMaterialPath"));
    }
}

#[test]
fn csm_from_values_clamps() {
    let mut params = PBR2_DEFAULTS;
    (params[0], params[1], params[19], params[24]) = (f32::NAN, 2.0, 0.0, 9.0);
    params[3] = 1.5; // a colour above 1 writes #ff
    let c = csm_from_values(&PbrValues {
        features: feat::GLINT | feat::THIN_SURFACE | feat::SUBSURFACE,
        params,
        fuzz_roughness: -1.0,
    });
    assert_eq!(c.values["PBRRoughnessScale"], Value::Num(1.0));
    assert_eq!(c.values["PBRSpecularLevel"], Value::Num(1.0));
    assert_eq!(c.values["PBRGlintMicrofacetRoughness"], Value::Num(0.005));
    assert_eq!(
        c.values["PBRThinSurfaceBackRoughnessScale"],
        Value::Num(3.0)
    );
    assert_eq!(
        c.values["PBRSubsurfaceColor"],
        Value::Color([0xff, 0xff, 0xff])
    );
    assert!(csm::parse(&csm::to_text(&c)).is_ok());
}

#[test]
fn fuzz_roughness_maps() {
    let v = pbr_values(&pbr_file(r#""PBRFuzzRoughness": 0.4"#)).unwrap();
    assert_eq!(v.fuzz_roughness, 0.4);
    let v = pbr_values(&pbr_file(r#""PBRFuzzWeight": 0.4"#)).unwrap();
    assert_eq!(v.fuzz_roughness, -1.0);
    let with = |features: u32, fuzz_roughness: f32| {
        csm_from_values(&PbrValues {
            features,
            params: PBR2_DEFAULTS,
            fuzz_roughness,
        })
        .values
        .get("PBRFuzzRoughness")
        .cloned()
    };
    assert_eq!(with(feat::FUZZ, 0.4), Some(Value::Num(0.4)));
    assert_eq!(with(feat::FUZZ, -1.0), Some(Value::Num(-1.0)));
    assert_eq!(with(0, 0.4), None);
    // Clamped to -1 (base roughness) to 1, the schema's maximum; not finite, it is -1.
    assert_eq!(with(feat::FUZZ, 5.0), Some(Value::Num(1.0)));
    assert_eq!(with(feat::FUZZ, -3.0), Some(Value::Num(-1.0)));
    assert_eq!(with(feat::FUZZ, f32::NAN), Some(Value::Num(-1.0)));
    for (from, to) in [(0.4, 0.4), (5.0, 1.0), (-3.0, -1.0), (f32::INFINITY, -1.0)] {
        assert_eq!(nif::pbr2::clamp_fuzz_roughness(from), to, "{from}");
    }
}

#[test]
fn foliage_maps() {
    let v = pbr_values(&pbr_file(r#""PBRFoliageEnabled": true"#)).unwrap();
    assert_eq!(v.features, feat::FOLIAGE);
    let c = csm_from_values(&v);
    assert_eq!(c.values.get("PBRFoliageEnabled"), Some(&Value::Bool(true)));
    let c = csm_from_values(&PbrValues { features: 0, ..v });
    assert!(!c.values.contains_key("PBRFoliageEnabled"));
}

#[test]
fn v1_values_reports_clamps() {
    let p = v1_fixtures::v1(1.0, 2.0, [1.0; 3], 0.0, 1.0, None);
    let (v, clamped) = v1_values(0, v1_fixtures::PBR, &p);
    assert_eq!(clamped, [(1, 2.0, 1.0)]);
    assert_eq!(v.params[1], 1.0);
    assert_eq!(v.fuzz_roughness, -1.0);
}
