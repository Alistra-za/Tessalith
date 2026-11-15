// crates/nif/tests/csm.rs
//! The `.csm` material model (Note IE1): parse, per-file checks, the schema's allOf rules, writing, colours, slots.
use nif::csm::{self, Csm, CsmError, Kind, Value, KEYS, VERSION};

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
