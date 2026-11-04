use engine::codes::{Code, Severity};
use engine::config::*;
use engine::overrides::to_patch;
use engine::presets::*;
use serde_json::json;
use std::path::{Path, PathBuf};

#[test]
fn merge_is_deep_and_leaf_paths_are_dotted() {
    let mut a = json!({"alpha": {"policy": "remove", "threshold": 127}, "format": "bc7"});
    merge_json(&mut a, &json!({"alpha": {"policy": "data"}}));
    assert_eq!(
        a,
        json!({"alpha": {"policy": "data", "threshold": 127}, "format": "bc7"})
    );
    let mut paths = vec![];
    leaf_paths(
        &json!({"alpha": {"policy": "data"}, "format": "bc1"}),
        "",
        &mut paths,
    );
    paths.sort();
    assert_eq!(paths, vec!["alpha.policy", "format"]);
}

#[test]
fn null_patches_never_erase() {
    let mut a = json!({"format": "bc7", "alpha": {"policy": "remove"}});
    merge_json(&mut a, &json!({"format": null, "alpha": null}));
    assert_eq!(a, json!({"format": "bc7", "alpha": {"policy": "remove"}}));
}

#[test]
fn builtin_defaults_match_spec_table_4_2() {
    let r = PresetRegistry::builtin().unwrap();
    let p = r
        .profile("cs-quality", Target::Cs, Subtype::PbrAlbedo)
        .unwrap();
    assert_eq!(
        (p.format, p.color.tag, p.color.content),
        (Format::Bc7, ColorTag::Srgb, ColorContent::Srgb)
    );
    let p = r
        .profile("vanilla-quality", Target::Vanilla, Subtype::Diffuse)
        .unwrap();
    assert_eq!((p.format, p.color.tag), (Format::Bc7, ColorTag::Unorm));
    let p = r.profile("cs-quality", Target::Cs, Subtype::Flow).unwrap();
    assert_eq!(p.format, Format::Bc5);
    let p = r
        .profile("cs-quality", Target::Cs, Subtype::Envmask)
        .unwrap();
    assert_eq!(
        (p.format, p.alpha.policy, p.alpha.exact_opaque),
        (Format::Bc4, AlphaPolicy::Remove, true)
    );
    let p = r
        .profile("cs-quality", Target::Cs, Subtype::ComplexMaterial)
        .unwrap();
    assert_eq!(
        (p.alpha.policy, p.alpha.exact_opaque),
        (AlphaPolicy::Data, true)
    );
    let p = r
        .profile("cs-quality", Target::Cs, Subtype::PbrEmissive)
        .unwrap();
    assert_eq!(
        (p.format, p.color.tag),
        (Format::Bc6hUf16, ColorTag::NotApplicable)
    );
    let p = r
        .profile("cs-quality", Target::Cs, Subtype::Normal)
        .unwrap();
    assert_eq!(
        (
            p.alpha.policy,
            p.alpha.threshold,
            p.encoder.effort,
            p.encoder.perceptual
        ),
        (AlphaPolicy::Data, 127, Effort::Veryslow, false)
    );
}

#[test]
fn every_builtin_subtype_profile_deserialises() {
    let r = PresetRegistry::builtin().unwrap();
    for (id, target) in [
        ("cs-quality", Target::Cs),
        ("vanilla-quality", Target::Vanilla),
    ] {
        for st in Subtype::ALL {
            let p = r
                .profile(id, target, st)
                .unwrap_or_else(|e| panic!("{id}/{st:?}: {e}"));
            assert_eq!(
                (p.target, p.subtype, p.category),
                (target, st, st.category())
            );
        }
    }
}

#[test]
fn preset_targets_and_default_preset_agree() {
    let r = PresetRegistry::builtin().unwrap();
    for t in [Target::Cs, Target::Vanilla] {
        assert_eq!(r.get(default_preset_for(t)).unwrap().target, Some(t));
    }
    assert_eq!(default_preset_for(Target::Cs), DEFAULT_PRESET);
    assert!(r.get(BASE_ID).unwrap().hidden);
    let ids: Vec<_> = r.selectable().iter().map(|p| p.id.clone()).collect();
    assert_eq!(
        ids,
        [
            "cs-compact",
            "cs-quality",
            "fast-preview",
            "vanilla-compact",
            "vanilla-quality"
        ]
    );
    assert_eq!(r.get(BASE_ID).unwrap().rules.len(), 2);
}

/// M6 Track D spec §2 (Notes T1–T3): the three new presets as profiles from presets alone (rules are not applied by
/// `profile`; the BC1 rules are checked by the goldens).
#[test]
fn compact_and_fast_preview_resolve_as_designed() {
    let r = PresetRegistry::builtin().unwrap();
    for (id, target) in [
        ("cs-compact", Some(Target::Cs)),
        ("vanilla-compact", Some(Target::Vanilla)),
        ("fast-preview", None),
    ] {
        let p = r.get(id).unwrap();
        assert_eq!(
            (p.schema_version, p.version.as_str(), p.hidden, p.target),
            (1, "2.0.0", false, target)
        );
        r.check_preset(id).unwrap();
    }
    // Owner rules (2026-10-12): data maps and colour that stays BC7 halve; pbr_emissive and hdr_sky keep full size.
    let halved = [
        Subtype::Normal,
        Subtype::LandscapeNormal,
        Subtype::PbrNormal,
        Subtype::Msn,
        Subtype::CoatNormal,
        Subtype::SkinWet,
        Subtype::Envmask,
        Subtype::Height,
        Subtype::ProjectedNoise,
        Subtype::DetailNormal,
        Subtype::MsnSpecular,
        Subtype::SkinSk,
        Subtype::ComplexMaterial,
        Subtype::Rmaos,
        Subtype::SkinRfaos,
        Subtype::Flow,
        Subtype::GenericTwoChannel,
        Subtype::GenericData,
        Subtype::Gray,
        Subtype::Diffuse,
        Subtype::LandscapeDiffuse,
        Subtype::PbrAlbedo,
        Subtype::Glow,
        Subtype::PbrSubsurface,
        Subtype::PbrFuzz,
        Subtype::SubsurfaceTint,
        Subtype::BacklightTint,
        Subtype::UiSky,
        Subtype::SkinAlbedo,
    ];
    for st in Subtype::ALL {
        let p = r.profile("cs-compact", Target::Cs, st).unwrap();
        let want = if halved.contains(&st) {
            ResizeMode::Half
        } else {
            ResizeMode::None
        };
        assert_eq!(p.resize, want, "{st:?}");
        // Note FM44: the slot-1 colour maps halve their own size, every other halved map relative to slot 1.
        let relative = if want == ResizeMode::Half && st.txst_slot() != Some(1) {
            engine::config::ResizeRelativeTo::Slot1
        } else {
            engine::config::ResizeRelativeTo::Own
        };
        assert_eq!(p.resize_relative_to, relative, "{st:?}");
        // Formats are the quality preset's: BC1 comes from the rules only.
        assert_eq!(
            p.format,
            r.profile("cs-quality", Target::Cs, st).unwrap().format,
            "{st:?}"
        );
    }
    let compact = r.get("cs-compact").unwrap();
    assert_eq!(compact.rules.len(), 2);
    assert_eq!(compact.rules[0].matcher.alpha, Some(AlphaContent::None));
    assert_eq!(compact.rules[0].matcher.subtypes.len(), 9);
    assert_eq!(
        compact.rules[1].matcher.alpha,
        Some(AlphaContent::Constant255)
    );
    assert_eq!(compact.rules[1].matcher.subtypes.len(), 7);
    for rule in &compact.rules {
        assert!(!rule.matcher.subtypes.contains(&Subtype::LandscapeDiffuse));
        assert!(!rule.matcher.subtypes.contains(&Subtype::PbrEmissive));
        assert_eq!(rule.overrides, json!({"format": "bc1", "resize": "none"}));
    }
    // vanilla-compact has the same rules, with perceptual BC1 (FM42).
    let vanilla = &r.get("vanilla-compact").unwrap().rules;
    assert_eq!(vanilla.len(), compact.rules.len());
    for (v, c) in vanilla.iter().zip(&compact.rules) {
        assert_eq!(v.matcher, c.matcher);
        assert_eq!(
            v.overrides,
            json!({"format": "bc1", "resize": "none", "encoder": {"perceptual": true}})
        );
    }
    for t in [Target::Cs, Target::Vanilla] {
        let p = r.profile("fast-preview", t, Subtype::Diffuse).unwrap();
        assert_eq!(p.encoder.effort, Effort::Fast);
        assert_eq!(p.validation.mips, ValidationMips::Smallest);
        assert_eq!(p.resize, ResizeMode::None);
    }
    for id in ["cs-compact", "vanilla-compact", "fast-preview"] {
        let t = r.get(id).unwrap().target.unwrap_or(Target::Cs);
        assert_eq!(
            r.profile(id, t, Subtype::Envmask)
                .unwrap()
                .encoder
                .bc45_search_radius,
            SearchRadius::Fast
        );
    }
    assert_eq!(
        r.profile("cs-quality", Target::Cs, Subtype::Envmask)
            .unwrap()
            .encoder
            .bc45_search_radius,
        SearchRadius::Default
    );
}

#[test]
fn chain_inheritance_and_errors() {
    let mut r = PresetRegistry::builtin().unwrap();
    assert_eq!(
        r.chain("cs-quality")
            .unwrap()
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<_>>(),
        ["sdc-base", "cs-quality"]
    );
    assert!(r.selectable().iter().all(|p| p.id != "sdc-base"));
    let id = r
        .add_toml("schema_version = 1\nid = \"mine\"\nversion = \"0.1.0\"\nbase = \"cs-quality\"\n[subtypes.diffuse]\nformat = \"bc1\"\n")
        .unwrap();
    assert_eq!(
        r.profile(&id, Target::Cs, Subtype::Diffuse).unwrap().format,
        Format::Bc1
    );
    assert!(matches!(
        r.add_toml("schema_version = 2\nid = \"x\"\nversion = \"1.0.0\"\n"),
        Err(PresetError::NewerSchema(2))
    ));
    assert!(matches!(r.chain("nope"), Err(PresetError::Unknown(_))));
    let bad = r.add_toml("schema_version = 1\nid = \"bad\"\nversion = \"1.0.0\"\nbase = \"sdc-base\"\n[defaults]\nformat = \"bc9\"\n").unwrap();
    assert!(matches!(
        r.profile(&bad, Target::Cs, Subtype::Diffuse),
        Err(PresetError::Invalid(_))
    ));
    assert_eq!(r.sha256("cs-quality").unwrap().len(), 64);
    assert_eq!(r.sha256("nope"), None);
}

#[test]
fn inheritance_cycle_is_detected() {
    let mut r = PresetRegistry::builtin().unwrap();
    r.add_toml("schema_version = 1\nid = \"a\"\nversion = \"1\"\nbase = \"b\"\n")
        .unwrap();
    r.add_toml("schema_version = 1\nid = \"b\"\nversion = \"1\"\nbase = \"a\"\n")
        .unwrap();
    assert!(matches!(r.chain("a"), Err(PresetError::Cycle(_))));
}

#[test]
fn layers_put_all_defaults_before_all_subtype_tables() {
    let mut r = PresetRegistry::builtin().unwrap();
    let id = r
        .add_toml("schema_version = 1\nid = \"child\"\nversion = \"1\"\nbase = \"cs-quality\"\n[defaults]\nformat = \"bc1\"\n")
        .unwrap();
    let layers = r.layers(&id, Subtype::Envmask).unwrap();
    let provs: Vec<_> = layers.iter().map(|(p, _)| p.to_string()).collect();
    // base defaults, child defaults, then base's envmask table (cs-quality has none).
    assert_eq!(provs, ["builtin", "preset:child", "builtin"]);
    // A child generic default does NOT override the base envmask sub-type format.
    assert_eq!(
        r.profile(&id, Target::Cs, Subtype::Envmask).unwrap().format,
        Format::Bc4
    );
    // ... but does apply to sub-types with no format of their own.
    assert_eq!(
        r.profile(&id, Target::Cs, Subtype::Diffuse).unwrap().format,
        Format::Bc1
    );
}

#[test]
fn provenance_serialises_as_spec_strings() {
    let cases = [
        (Provenance::Builtin, "builtin"),
        (Provenance::Preset("cs-quality".into()), "preset:cs-quality"),
        (Provenance::UserConfig, "user_config"),
        (Provenance::ProjectConfig, "project_config"),
        (Provenance::Rule("sdc.toml#3".into()), "rule:sdc.toml#3"),
        (Provenance::Analysis, "analysis"),
        (
            Provenance::AnalysisFact("float-source".into()),
            "analysis:float-source",
        ),
        (Provenance::Suggestion, "suggestion"),
        (Provenance::ProjectFile, "project_file"),
        (Provenance::User, "user"),
        (Provenance::Fallback, "fallback"),
    ];
    for (p, s) in cases {
        assert_eq!(serde_json::to_value(&p).unwrap(), json!(s));
        assert_eq!(serde_json::from_value::<Provenance>(json!(s)).unwrap(), p);
    }
    assert!(serde_json::from_value::<Provenance>(json!("bogus")).is_err());
    assert!(Provenance::Builtin < Provenance::Preset("a".into()));
    assert!(
        Provenance::Suggestion < Provenance::ProjectFile
            && Provenance::ProjectFile < Provenance::User
    );
    // Spec §5.2's `set` (set-level decisions) is a value since M4.
    assert_eq!(
        serde_json::from_value::<Provenance>(json!("set")).unwrap(),
        Provenance::Set
    );
}

#[test]
fn config_file_loads_and_rejects_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("sdc.toml");
    std::fs::write(
        &p,
        "preset = \"cs-quality\"\ntarget = \"cs\"\n[defaults]\nformat = \"bc7\"\n[subtypes.diffuse]\nalpha = { policy = \"cutout\" }\n[[rules]]\nmatch = { glob = \"**/armor/**\", alpha = \"varying\" }\nsubtype = \"diffuse\"\noverrides = { alpha = { policy = \"blend\" } }\n",
    )
    .unwrap();
    let c = ConfigFile::load(&p).unwrap();
    assert_eq!(c.preset.as_deref(), Some("cs-quality"));
    assert_eq!(c.rules.len(), 1);
    assert_eq!(c.rules[0].matcher.alpha, Some(AlphaContent::Varying));
    assert_eq!(c.rules[0].subtype, Some(Subtype::Diffuse));
    assert!(c.subtypes.contains_key(&Subtype::Diffuse));
    std::fs::write(&p, "bogus = 1\n").unwrap();
    assert!(matches!(ConfigFile::load(&p), Err(PresetError::Parse(_))));
    assert!(matches!(
        ConfigFile::load(&dir.path().join("missing.toml")),
        Err(PresetError::Parse(_))
    ));
}

#[test]
fn add_file_registers_a_preset() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("mine.toml");
    std::fs::write(
        &p,
        "schema_version = 1\nid = \"filed\"\nversion = \"1\"\nbase = \"sdc-base\"\n",
    )
    .unwrap();
    let mut r = PresetRegistry::builtin().unwrap();
    assert_eq!(r.add_file(&p).unwrap(), "filed");
    assert!(r.get("filed").is_some());
}

#[test]
fn finish_profile_reports_invalid_documents() {
    let e = finish_profile(json!({"format": "bc9"}), Target::Cs, Subtype::Diffuse).unwrap_err();
    assert!(matches!(e, PresetError::Invalid(_)));
}

/// Codex #2: `sdc-base` is reserved; a user preset cannot replace it.
#[test]
fn the_base_preset_id_is_reserved() {
    let mut r = PresetRegistry::builtin().unwrap();
    let before = r.get(BASE_ID).unwrap().clone();
    let e = r
        .add_toml(
            "schema_version = 1
id = \"sdc-base\"
version = \"1\"
",
        )
        .unwrap_err();
    assert!(e.to_string().starts_with("CFG-INVALID"), "{e}");
    assert_eq!(r.get(BASE_ID), Some(&before));
    assert!(r.is_builtin(BASE_ID));
    let dir = std::env::temp_dir().join(format!("sdc-reserved-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("base.toml");
    std::fs::write(
        &f,
        "schema_version = 1
id = \"sdc-base\"
version = \"1\"
",
    )
    .unwrap();
    assert!(r
        .add_file(&f)
        .unwrap_err()
        .to_string()
        .starts_with("CFG-INVALID"));
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(r.get(BASE_ID), Some(&before));
}

/// M2.5 (spec §6.4): packing settings are profile fields with the neutral defaults of the base preset.
#[test]
fn packing_fields_default_from_the_base_preset() {
    let reg = PresetRegistry::builtin().unwrap();
    let p = reg
        .profile("cs-quality", Target::Cs, Subtype::Rmaos)
        .unwrap();
    assert_eq!(
        p.packing,
        PackingSpec {
            enabled: true,
            roughness_default: 0.5,
            prefer: PackPrefer::Prepacked,
            fill: FillSpec::default(),
            recipe: Default::default(),
        }
    );
    // Unset fills are left out of the JSON, so `profile_sha256` moves only when a fill is set.
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["packing"]["fill"], serde_json::json!({}));
}

/// DECISIONS Note M5: a fill is an 8-bit code per raw-map role.
#[test]
fn a_fill_is_an_8_bit_code_per_raw_map_role() {
    let reg = PresetRegistry::builtin().unwrap();
    let mut doc = serde_json::to_value(
        reg.profile("cs-quality", Target::Cs, Subtype::Rmaos)
            .unwrap(),
    )
    .unwrap();
    merge_json(
        &mut doc,
        &to_patch("packing.fill.metallic", &serde_json::json!(255)),
    );
    let p = finish_profile(doc.clone(), Target::Cs, Subtype::Rmaos).unwrap();
    assert_eq!(p.packing.fill.get(RawMap::Metallic), Some(255));
    assert_eq!(p.packing.fill.get(RawMap::Ao), None);
    for bad in [
        serde_json::json!(256),
        serde_json::json!(-1),
        serde_json::json!(0.5),
    ] {
        let mut d = doc.clone();
        merge_json(&mut d, &to_patch("packing.fill.ao", &bad));
        assert!(
            finish_profile(d, Target::Cs, Subtype::Rmaos).is_err(),
            "{bad}"
        );
    }
}

/// DECISIONS Note M12: packing provenance round-trips and ranks above every existing source, so
/// `float_source_is_linear`'s `>= Suffix` comparison never sees it below a suffix fact.
#[test]
fn packing_provenance_round_trips_and_ranks_last() {
    for (p, s) in [
        (
            Provenance::Source("iron_roughness.png:R".into()),
            "source:iron_roughness.png:R",
        ),
        (
            Provenance::Source("1-iron_gloss.png:R".into()),
            "source:1-iron_gloss.png:R",
        ),
        (Provenance::Default("0.5".into()), "default:0.5"),
        (Provenance::Fill("255".into()), "fill:255"),
        (Provenance::Recipe("rmaos".into()), "recipe:rmaos"),
    ] {
        assert_eq!(p.to_string(), s);
        assert_eq!(s.parse::<Provenance>().unwrap(), p);
        assert_eq!(serde_json::to_value(&p).unwrap(), serde_json::json!(s));
        assert!(p > Provenance::Fallback && p > Provenance::Suffix(String::new()));
    }
}

#[test]
fn pack_codes_are_in_the_catalogue() {
    for (s, sev) in [
        ("PACK-DEFAULTED", Severity::Warning),
        ("PACK-SIZE-MISMATCH", Severity::Error),
        ("PACK-SOURCE-RESIZED", Severity::Info),
        ("PACK-CONFLICT", Severity::Warning),
        ("PACK-UNUSED-INPUT", Severity::Warning),
        ("PACK-RECIPE-CONFLICT", Severity::Warning),
        ("PACK-RECIPE-UNBUILDABLE", Severity::Warning),
    ] {
        let c = Code::parse(s).unwrap();
        assert_eq!((c.as_str(), c.severity()), (s, sev));
    }
}

const MINE: &str =
    "schema_version = 1\nid = \"mine\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n";

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

/// Carried fix 1 (M6 Track D, Note T7): a second preset under a loaded id is refused, naming both sources.
#[test]
fn duplicate_and_builtin_ids_are_refused_naming_both_sources() {
    let mut r = PresetRegistry::builtin().unwrap();
    r.add_toml(MINE).unwrap();
    let e = r.add_toml(MINE).unwrap_err().to_string();
    assert!(
        e.starts_with("CFG-INVALID") && e.contains("'mine'") && e.contains("inline"),
        "{e}"
    );
    let e = r
        .add_toml("schema_version = 1\nid = \"cs-quality\"\nversion = \"9\"\nbase = \"sdc-base\"\n")
        .unwrap_err()
        .to_string();
    assert!(e.contains("already loaded from builtin"), "{e}");
    assert!(r.is_builtin("cs-quality"));
    assert!(!r.is_builtin("mine"));
    assert_eq!(r.source("mine"), Some("inline"));
    assert_eq!(r.source("cs-quality"), Some("builtin"));
    assert_eq!(r.user_presets(), vec![("mine", "inline")]);
    assert_eq!(
        r.table("mine").unwrap()["base"].as_str(),
        Some("cs-quality")
    );
}

/// Review Focus 1: a Windows BOM and CRLF line endings load.
#[test]
fn a_preset_with_a_bom_and_crlf_loads() {
    let mut r = PresetRegistry::builtin().unwrap();
    let text = "\u{feff}schema_version = 1\r\nid = \"crlf\"\r\nversion = \"1.0.0\"\r\nbase = \"cs-quality\"\r\n[subtypes.diffuse]\r\nformat = \"bc1\"\r\n";
    assert_eq!(r.add_toml(text).unwrap(), "crlf");
    assert_eq!(
        r.profile("crlf", Target::Cs, Subtype::Diffuse)
            .unwrap()
            .format,
        Format::Bc1
    );
    let d = tempfile::tempdir().unwrap();
    write(
        d.path(),
        "bom.toml",
        &text.replace("\"crlf\"", "\"bomfile\""),
    );
    assert!(r.add_dir(d.path()).is_empty());
    assert!(r.get("bomfile").is_some());
}

/// Spec §3 and Review Focus 2: everything loads first, then every user preset is checked; a bad one is skipped
/// with CFG-PRESET-SKIPPED, and so is a preset based on a skipped one.
#[test]
fn the_user_folder_loads_everything_then_checks_and_skips_bad_files() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path();
    write(
        dir,
        "a_child.toml",
        "schema_version = 1\nid = \"child\"\nversion = \"1\"\nbase = \"zbase\"\n",
    );
    write(dir, "b_bad.toml", "this is not toml");
    write(
        dir,
        "c_dup.toml",
        "schema_version = 1\nid = \"cs-quality\"\nversion = \"1\"\nbase = \"sdc-base\"\n",
    );
    write(
        dir,
        "d_orphan.toml",
        "schema_version = 1\nid = \"orphan\"\nversion = \"1\"\nbase = \"missing\"\n",
    );
    write(dir, "e_badfield.toml", "schema_version = 1\nid = \"badfield\"\nversion = \"1\"\nbase = \"cs-quality\"\n[defaults]\nformat = \"bc9\"\n");
    write(
        dir,
        "f_on_orphan.toml",
        "schema_version = 1\nid = \"grandchild\"\nversion = \"1\"\nbase = \"orphan\"\n",
    );
    write(dir, "notes.txt", "not a preset");
    write(
        &dir.join("sub"),
        "deep.toml",
        "schema_version = 1\nid = \"deep\"\nversion = \"1\"\nbase = \"cs-quality\"\n",
    );
    write(dir, "z_base.toml", "schema_version = 1\nid = \"zbase\"\nversion = \"1\"\nbase = \"cs-quality\"\n[subtypes.diffuse]\nformat = \"bc1\"\n");
    let mut r = PresetRegistry::builtin().unwrap();
    let w = r.add_dir(dir);
    assert_eq!(
        r.profile("child", Target::Cs, Subtype::Diffuse)
            .unwrap()
            .format,
        Format::Bc1
    );
    let mut skipped: Vec<String> = w
        .iter()
        .map(|w| {
            assert_eq!(
                (w.code, w.severity),
                (Code::CfgPresetSkipped, Severity::Warning)
            );
            assert!(
                w.data["reason"].as_str().is_some_and(|s| !s.is_empty()),
                "{w:?}"
            );
            let file = w.data["file"].as_str().unwrap();
            file.rsplit(['/', '\\']).next().unwrap().to_string()
        })
        .collect();
    skipped.sort();
    assert_eq!(
        skipped,
        [
            "b_bad.toml",
            "c_dup.toml",
            "d_orphan.toml",
            "e_badfield.toml",
            "f_on_orphan.toml"
        ]
    );
    for id in ["orphan", "grandchild", "badfield", "deep"] {
        assert!(r.get(id).is_none(), "{id}");
    }
    assert!(r.is_builtin("cs-quality"));
    assert!(r.source("zbase").unwrap().ends_with("z_base.toml"));
    assert!(PresetRegistry::builtin()
        .unwrap()
        .add_dir(&dir.join("nope"))
        .is_empty());
}

/// Note T6: one file reached twice (the folder and --preset-file) loads once; a copy under the same id does not.
#[test]
fn the_same_file_twice_loads_once() {
    let d = tempfile::tempdir().unwrap();
    let p = write(d.path(), "mine.toml", MINE);
    let mut r = PresetRegistry::builtin().unwrap();
    assert!(r.add_dir(d.path()).is_empty());
    assert_eq!(r.add_file(&p).unwrap(), "mine");
    let copy = write(&d.path().join("copy"), "mine.toml", MINE);
    assert!(r
        .add_file(&copy)
        .unwrap_err()
        .to_string()
        .contains("already loaded from"));
}

#[test]
fn every_builtin_preset_passes_check_preset() {
    let r = PresetRegistry::builtin().unwrap();
    for p in r.selectable() {
        r.check_preset(&p.id).unwrap();
    }
    r.check_preset(BASE_ID).unwrap();
    let mut r = r;
    r.add_toml("schema_version = 1\nid = \"badrule\"\nversion = \"1\"\nbase = \"cs-quality\"\n[[rules]]\nmatch = { glob = \"**\" }\noverrides = { bogus = 1 }\n")
        .unwrap();
    assert!(r
        .check_preset("badrule")
        .unwrap_err()
        .to_string()
        .contains("bogus"));
}

/// Fix round 1: the reserved-id refusal names the incoming source and the built-in one.
#[test]
fn the_reserved_id_refusal_names_both_sources() {
    let base = "schema_version = 1\nid = \"sdc-base\"\nversion = \"1\"\n";
    let mut r = PresetRegistry::builtin().unwrap();
    let e = r.add_toml(base).unwrap_err().to_string();
    assert!(
        e.starts_with("CFG-INVALID")
            && e.contains("from inline")
            && e.contains("already loaded from builtin"),
        "{e}"
    );
    let d = tempfile::tempdir().unwrap();
    let p = write(d.path(), "base.toml", base);
    let e = r.add_file(&p).unwrap_err().to_string();
    assert!(
        e.contains(&format!("from {}", p.display())) && e.contains("already loaded from builtin"),
        "{e}"
    );
}

/// Fix round 1: a folder that cannot be read is one CFG-PRESET-SKIPPED naming it (a missing one is silent).
#[test]
fn an_unreadable_preset_folder_warns_naming_it() {
    let d = tempfile::tempdir().unwrap();
    let not_a_dir = write(d.path(), "file.toml", MINE);
    let w = PresetRegistry::builtin().unwrap().add_dir(&not_a_dir);
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].code, Code::CfgPresetSkipped);
    assert_eq!(
        w[0].data["file"].as_str(),
        Some(not_a_dir.display().to_string().as_str())
    );
}

/// Fix round 1: a `*.toml` entry whose metadata cannot be read (a dangling link) is skipped with a warning naming it.
#[test]
fn a_dangling_preset_link_warns_naming_it() {
    let d = tempfile::tempdir().unwrap();
    let link = d.path().join("dangling.toml");
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(d.path().join("gone.toml"), &link);
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(d.path().join("gone.toml"), &link);
    if made.is_err() {
        eprintln!("skipped: cannot create a symlink here (Windows without Developer Mode)");
        return;
    }
    let w = PresetRegistry::builtin().unwrap().add_dir(d.path());
    assert_eq!(w.len(), 1, "{w:?}");
    assert_eq!(w[0].code, Code::CfgPresetSkipped);
    assert_eq!(
        w[0].data["file"].as_str(),
        Some(link.display().to_string().as_str())
    );
}

/// Spec §5: a config's schema_version is optional (1), a newer one is refused naming the file.
#[test]
fn config_schema_version_is_optional_and_a_newer_one_names_the_file() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("sdc.toml");
    std::fs::write(&p, "schema_version = 1\npreset = \"cs-quality\"\n").unwrap();
    assert_eq!(ConfigFile::load(&p).unwrap().schema_version, Some(1));
    std::fs::write(&p, "schema_version = 2\n").unwrap();
    match ConfigFile::load(&p) {
        Err(e @ PresetError::NewerConfigSchema(_, 2)) => {
            assert!(e.to_string().contains("sdc.toml"), "{e}")
        }
        other => panic!("{other:?}"),
    }
    // Absent: not serialised back.
    let c = ConfigFile::parse("preset = \"cs-quality\"\n", "x").unwrap();
    assert!(serde_json::to_value(&c)
        .unwrap()
        .get("schema_version")
        .is_none());
}

/// Review Focus 1, config twin.
#[test]
fn a_config_with_a_bom_and_crlf_loads() {
    let c = ConfigFile::parse(
        "\u{feff}preset = \"cs-quality\"\r\n[defaults]\r\nformat = \"bc7\"\r\n",
        "user config",
    )
    .unwrap();
    assert_eq!(c.preset.as_deref(), Some("cs-quality"));
}

#[test]
fn a_newer_preset_is_still_newer_schema() {
    let mut r = PresetRegistry::builtin().unwrap();
    assert_eq!(
        r.add_toml("schema_version = 3\nid = \"x\"\n"),
        Err(PresetError::NewerSchema(3))
    );
    assert!(matches!(
        r.add_toml("id = \"x\"\nversion = \"1\"\n"),
        Err(PresetError::Parse(_))
    ));
}

/// Carried fix 2 (Note T10): a null patch neither erases nor adds a key.
#[test]
fn null_patches_never_add_absent_keys() {
    let mut a = json!({"format": "bc7"});
    merge_json(
        &mut a,
        &json!({"header": null, "alpha": {"policy": null, "threshold": 5}, "format": null}),
    );
    assert_eq!(a, json!({"format": "bc7", "alpha": {"threshold": 5}}));
    // A patch object replacing a null or scalar base is filtered the same way.
    for base in [json!({"alpha": null}), json!({"alpha": 5})] {
        let mut a = base;
        merge_json(&mut a, &json!({"alpha": {"policy": null, "threshold": 5}}));
        assert_eq!(a, json!({"alpha": {"threshold": 5}}));
    }
}

/// Note T2: `match.subtypes` round-trips and is not serialised when empty (existing preset hashes hold).
#[test]
fn rule_match_subtypes_is_optional() {
    let r: Rule = toml::from_str("match = { alpha = \"none\", subtypes = [\"diffuse\", \"glow\"] }\noverrides = { format = \"bc1\" }\n").unwrap();
    assert_eq!(r.matcher.subtypes, vec![Subtype::Diffuse, Subtype::Glow]);
    let plain = serde_json::to_value(RuleMatch::default()).unwrap();
    assert!(plain.get("subtypes").is_none());
}

/// A preset file that fails to load or to pass its check names the first problem (the batch and the GUI creator
/// show it).
#[test]
fn a_bad_preset_file_names_its_first_problem() {
    let d = tempfile::tempdir().unwrap();
    let r = PresetRegistry::builtin().unwrap();
    for (name, text, says) in [
        ("dup.toml", "schema_version = 1\nid = \"cs-quality\"\nversion = \"1\"\n", "already loaded"),
        ("nobase.toml", "schema_version = 1\nid = \"x\"\nversion = \"1\"\nbase = \"nope\"\n", "nope"),
        ("unknown.toml", "schema_version = 1\nid = \"y\"\nversion = \"1\"\nbogus = 1\n", "bogus"),
        ("newer.toml", "schema_version = 9\nid = \"z\"\n", "newer"),
        ("rule.toml", "schema_version = 1\nid = \"w\"\nversion = \"1\"\nbase = \"cs-quality\"\n[[rules]]\nmatch = { subtypes = [\"nope\"] }\n", "nope"),
    ] {
        let f = write(d.path(), name, text);
        let mut copy = r.clone();
        let problem = match copy.add_file(&f) {
            Ok(id) => copy.check_preset(&id).err().map(|e| e.to_string()),
            Err(e) => Some(e.to_string()),
        };
        let p = problem.unwrap_or_else(|| panic!("{name}: no problem"));
        assert!(p.contains(says), "{name}: {p}");
    }
}

/// Note AT: vanilla-compact halves a complex `_m` (BC7 with alpha), as cs-compact does.
#[test]
fn vanilla_compact_halves_a_complex_m() {
    let r = PresetRegistry::builtin().unwrap();
    let p = r
        .profile("vanilla-compact", Target::Vanilla, Subtype::ComplexMaterial)
        .unwrap();
    assert_eq!(
        (p.format, p.resize, p.alpha.policy),
        (Format::Bc7, ResizeMode::Half, AlphaPolicy::Data)
    );
}

/// Note AZ: the cpm target and its preset are gone, with no alias: `target = "cpm"` and `base = "cpm-quality"` are
/// errors.
#[test]
fn cpm_target_and_preset_are_rejected() {
    let mut r = PresetRegistry::builtin().unwrap();
    assert!(r.get("cpm-quality").is_none());
    let e = r
        .add_toml("schema_version = 1\nid = \"a\"\nversion = \"1\"\nbase = \"sdc-base\"\ntarget = \"cpm\"\n")
        .unwrap_err()
        .to_string();
    assert!(e.contains("cpm"), "{e}");
    let added = r
        .add_toml("schema_version = 1\nid = \"b\"\nversion = \"1\"\nbase = \"cpm-quality\"\n")
        .and_then(|id| r.chain(&id).map(|_| ()));
    assert!(added.unwrap_err().to_string().contains("cpm-quality"));
    assert!(toml::from_str::<engine::presets::ConfigFile>("target = \"cpm\"\n").is_err());
}

/// Note AZ1 (review M3): a user preset with `target = "cpm"` in the presets folder is skipped with CFG-PRESET-SKIPPED
/// (Note T6, unchanged), and the warning names the unknown target.
#[test]
fn a_user_preset_with_the_cpm_target_is_skipped_naming_it() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(
        d.path().join("old.toml"),
        "schema_version = 1\nid = \"old\"\nversion = \"1\"\nbase = \"sdc-base\"\ntarget = \"cpm\"\n",
    )
    .unwrap();
    let mut r = PresetRegistry::builtin().unwrap();
    let w = r.add_dir(d.path());
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].code, engine::codes::Code::CfgPresetSkipped);
    assert!(w[0].message.contains("`cpm`"), "{}", w[0].message);
    assert!(r.get("old").is_none());
}

/// Audit REPORT #7 (Note BC): a preset rule whose glob does not compile fails `check_preset`.
#[test]
fn a_rule_glob_that_does_not_compile_fails_check_preset() {
    let mut r = PresetRegistry::builtin().unwrap();
    r.add_toml("schema_version = 1\nid = \"badglob\"\nversion = \"1\"\nbase = \"cs-quality\"\n[[rules]]\nmatch = { glob = \"**/{a,b\" }\noverrides = { mips = { levels = 2 } }\n")
        .unwrap();
    let e = r.check_preset("badglob").unwrap_err().to_string();
    assert!(e.contains("glob"), "{e}");
}

/// Audit EC-4 (Note BC): `overrides.subtype` on a rule would be overwritten by the decided sub-type, so it is refused.
#[test]
fn a_rule_override_of_the_subtype_fails_check_preset() {
    let mut r = PresetRegistry::builtin().unwrap();
    r.add_toml("schema_version = 1\nid = \"stover\"\nversion = \"1\"\nbase = \"cs-quality\"\n[[rules]]\nmatch = { suffix = \"n\" }\noverrides = { subtype = \"pbr_normal\" }\n")
        .unwrap();
    let e = r.check_preset("stover").unwrap_err().to_string();
    assert!(e.contains("not in overrides"), "{e}");
}

/// Trace-debug #15 (Note BQ): a file refused because a preset loaded before it claimed its alias or id is retried
/// once that preset is skipped as broken, so the valid one loads.
#[test]
fn user_dir_retries_file_refused_by_a_skipped_preset() {
    for (a, b, id) in [
        (
            "schema_version = 1\nid = \"mine-old\"\nversion = \"1\"\nbase = \"cs-qualty\"\n[suffix_aliases]\nfoo = \"s\"\n",
            "schema_version = 1\nid = \"mine\"\nversion = \"1\"\nbase = \"cs-quality\"\n[suffix_aliases]\nfoo = \"s\"\n",
            "mine",
        ),
        (
            "schema_version = 1\nid = \"same\"\nversion = \"1\"\nbase = \"cs-qualty\"\n",
            "schema_version = 1\nid = \"same\"\nversion = \"2\"\nbase = \"cs-quality\"\n",
            "same",
        ),
    ] {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.toml"), a).unwrap();
        std::fs::write(d.path().join("b.toml"), b).unwrap();
        let mut r = PresetRegistry::builtin().unwrap();
        let w = r.add_dir(d.path());
        assert!(
            r.source(id).is_some_and(|s| s.ends_with("b.toml")),
            "{id} not loaded from b.toml: {w:?}"
        );
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].message.contains("a.toml"), "{}", w[0].message);
    }
}

/// Note FM11: `[subtypes.rfaos]` (the removed id) is an error that names `skin_rfaos`, alone or beside
/// `[subtypes.skin_rfaos]`, in a config and in a preset; `skin_rfaos` alone loads.
#[test]
fn config_with_rfaos_is_refused_and_names_skin_rfaos() {
    let alone = "[subtypes.rfaos]\nformat = \"bc3\"\n";
    let both = format!("{alone}[subtypes.skin_rfaos]\nresize = \"half\"\n");
    for text in [alone, both.as_str()] {
        let e = engine::presets::ConfigFile::parse(text, "sdc.toml")
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("sdc.toml: subtypes.rfaos: removed in 2.0;") && e.contains("skin_rfaos"),
            "{e}"
        );
    }
    let c =
        engine::presets::ConfigFile::parse("[subtypes.skin_rfaos]\nformat = \"bc3\"\n", "sdc.toml")
            .unwrap();
    assert!(c.subtypes.contains_key(&Subtype::SkinRfaos));

    let head = "schema_version = 1\nid = \"twin\"\nversion = \"1\"\nbase = \"cs-quality\"\n";
    let mut r = PresetRegistry::builtin().unwrap();
    for text in [alone, both.as_str()] {
        let e = r
            .add_toml(&format!("{head}{text}"))
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("subtypes.rfaos: removed in 2.0") && e.contains("skin_rfaos"),
            "{e}"
        );
    }
    r.add_toml(&format!("{head}[subtypes.skin_rfaos]\nformat = \"bc3\"\n"))
        .unwrap();
}

/// Note FM14: `encoder.backend = "nvtt"` and an allow list naming one of the three NVTT codes are refused by name.
#[test]
fn an_nvtt_backend_names_the_removed_value() {
    let e = ConfigFile::parse("[defaults]\nencoder = { backend = \"nvtt\" }\n", "sdc.toml")
        .unwrap_err();
    assert!(
        e.to_string().contains("encoder.backend") && e.to_string().contains("NVTT"),
        "{e}"
    );
}

/// `--set encoder.backend=nvtt` (also file- and sub-type-scoped) goes through `removed_set`.
#[test]
fn a_set_override_naming_nvtt_is_refused_with_the_hint() {
    for field in ["encoder.backend", "file:a.png.encoder.backend"] {
        let e = engine::overrides::parse_set(&format!("{field}=nvtt")).unwrap_err();
        assert!(
            e.contains("encoder.backend: removed in 2.0") && e.contains("NVTT was removed"),
            "{e}"
        );
    }
}

/// Note FM29: `resize = "pot-down"` and `drop_top_mip` (any value, `false` included) are refused by name in a config,
/// a preset and `--set`; `resize = "quarter"` and `resize_floor` load.
#[test]
fn pot_down_and_drop_top_mip_are_named() {
    let pot = "power of two is automatic";
    let half = "use resize = \"half\"";
    for (text, hint) in [
        ("[defaults]\nresize = \"pot-down\"\n", pot),
        ("[files.\"a.png\"]\nresize = \"pot-down\"\n", pot),
        ("[files.\"a.png\"]\ndrop_top_mip = true\n", half),
        ("[subtypes.diffuse]\ndrop_top_mip = false\n", half),
    ] {
        let e = ConfigFile::parse(text, "sdc.toml").unwrap_err().to_string();
        assert!(
            e.contains("removed in 2.0") && e.contains(hint),
            "{text}: {e}"
        );
    }
    let head = "schema_version = 1\nid = \"p\"\nversion = \"1\"\nbase = \"cs-quality\"\n";
    let mut r = PresetRegistry::builtin().unwrap();
    for (text, hint) in [
        ("[subtypes.normal]\nresize = \"pot-down\"\n", pot),
        ("[subtypes.normal]\ndrop_top_mip = true\n", half),
    ] {
        let e = r
            .add_toml(&format!("{head}{text}"))
            .unwrap_err()
            .to_string();
        assert!(e.contains(hint), "{text}: {e}");
    }
    for (set, hint) in [
        ("resize=pot-down", pot),
        ("file:a.png.resize=pot-down", pot),
        ("drop_top_mip=true", half),
    ] {
        let e = engine::overrides::parse_set(set).unwrap_err();
        assert!(
            e.contains("removed in 2.0") && e.contains(hint),
            "{set}: {e}"
        );
    }
    let c = ConfigFile::parse(
        "[subtypes.diffuse]\nresize = \"quarter\"\nresize_floor = 256\n",
        "sdc.toml",
    )
    .unwrap();
    assert!(c.subtypes.contains_key(&Subtype::Diffuse));
    engine::overrides::parse_set("resize=eighth").unwrap();
    engine::overrides::parse_set("resize_floor=128").unwrap();
}

/// Note FM29: height maps take `resize_floor = 256` from sdc-base, so a 512 height map under the compact presets'
/// `resize = "half"` is written at 256; other maps keep the 512 floor.
#[test]
fn height_maps_shrink_to_a_256_floor() {
    let reg = PresetRegistry::builtin().unwrap();
    for preset in ["cs-compact", "vanilla-compact"] {
        let target = if preset == "cs-compact" {
            Target::Cs
        } else {
            Target::Vanilla
        };
        let h = reg.profile(preset, target, Subtype::Height).unwrap();
        assert_eq!(
            (h.resize, h.resize_floor),
            (ResizeMode::Half, 256),
            "{preset}"
        );
        assert_eq!(engine::resize::target_dims((512, 512), &h), (256, 256));
        let n = reg.profile(preset, target, Subtype::Normal).unwrap();
        assert_eq!(engine::resize::target_dims((512, 512), &n), (512, 512));
    }
}

/// Note FM17: `bc1_model` (the 1.x BC1 decoder-model key) is refused by name.
#[test]
fn bc1_model_in_a_config_is_named() {
    for text in [
        "bc1_model = \"nvidia\"\n",
        "[defaults]\nbc1_model = \"amd\"\n",
    ] {
        let e = ConfigFile::parse(text, "sdc.toml").unwrap_err().to_string();
        assert!(
            e.contains("bc1_model") && e.contains("NVIDIA + AMD"),
            "{text}: {e}"
        );
    }
    let e = engine::overrides::parse_set("bc1_model=nvidia").unwrap_err();
    assert!(
        e.contains("bc1_model") && e.contains("removed in 2.0"),
        "{e}"
    );
}

/// Note FM16: the five `validation.*` thresholds are constants and the project-wide `[defaults]` fill is gone, so
/// each is refused by name in a config (inline or dotted, any table), a preset and `--set`; `validation.mips` and a
/// per-file fill still load.
#[test]
fn removed_validation_and_fill_keys_are_named() {
    for key in [
        "min_mip_psnr_db",
        "coverage_tolerance_pts",
        "complex_m_warn_alpha",
        "bc6h_min_peak_ratio",
        "pgpatcher_non_white_share",
    ] {
        let full = format!("validation.{key}");
        for head in ["[defaults]", "[subtypes.diffuse]", "[files.\"a.png\"]"] {
            let e = ConfigFile::parse(
                &format!("{head}\nvalidation = {{ {key} = 1 }}\n"),
                "sdc.toml",
            )
            .unwrap_err();
            assert!(e.to_string().contains(&full), "{head}: {e}");
        }
        let e = engine::overrides::parse_set(&format!("{full}=1")).unwrap_err();
        assert!(e.contains(&full) && e.contains("removed in 2.0"), "{e}");
        let e = engine::overrides::parse_set(&format!("file:a.png.{full}=1")).unwrap_err();
        assert!(e.contains(&full), "{e}");
    }
    for text in [
        "[defaults]\npacking = { fill = { ao = 1.0 } }\n",
        "[defaults.packing.fill]\nao = 10\n",
        "[defaults]\npacking.fill.ao = 10\n",
    ] {
        let e = ConfigFile::parse(text, "sdc.toml").unwrap_err().to_string();
        assert!(
            e.contains("defaults.packing.fill") && e.contains("per set"),
            "{text}: {e}"
        );
    }
    let head = "schema_version = 1\nid = \"mine\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n";
    let mut r = PresetRegistry::builtin().unwrap();
    let e = r
        .add_toml(&format!(
            "{head}[defaults]\nvalidation = {{ min_mip_psnr_db = 20 }}\n"
        ))
        .unwrap_err();
    assert!(e.to_string().contains("validation.min_mip_psnr_db"), "{e}");
    // Still legal: `validation.mips`, fills per file and per sub-type, and `--set packing.fill.*`.
    ConfigFile::parse(
        "[defaults]\nvalidation = { mips = \"smallest\" }\n\
         [files.\"a_rmaos.png\".packing.fill]\nao = 10\n\
         [subtypes.rmaos]\npacking = { fill = { ao = 10 } }\n",
        "sdc.toml",
    )
    .unwrap();
    engine::overrides::parse_set("packing.fill.ao=10").unwrap();
}

/// Note FM15: the format alone picks the encoder, so `encoder.backend` and `encoder.rgbcx_level` are refused by name
/// whatever the value and wherever they sit (config, preset, `--set`); `IN-FLOAT-LDR` is refused in an allow list.
#[test]
fn removed_encoder_keys_and_codes_are_named() {
    for (key, text) in [
        ("encoder.backend", "encoder = { backend = \"bc7e\" }"),
        ("encoder.rgbcx_level", "encoder = { rgbcx_level = 10 }"),
    ] {
        for head in ["[defaults]", "[subtypes.diffuse]", "[files.\"a.png\"]"] {
            let e = ConfigFile::parse(&format!("{head}\n{text}\n"), "sdc.toml").unwrap_err();
            assert!(e.to_string().contains(key), "{head}: {e}");
        }
        let e = engine::overrides::parse_set(&format!("{key}=1")).unwrap_err();
        assert!(e.contains(key) && e.contains("removed in 2.0"), "{e}");
    }
    // A user preset file (the `migrate_preset` path) and a `[[rules]]` table in a config or preset.
    let head = "schema_version = 1\nid = \"mine\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n";
    let mut r = PresetRegistry::builtin().unwrap();
    let e = r
        .add_toml(&format!(
            "{head}[defaults]\nencoder = {{ backend = \"rgbcx\" }}\n"
        ))
        .unwrap_err();
    assert!(e.to_string().contains("encoder.backend"), "{e}");
    let rule = "[[rules]]\nmatch = { glob = \"*.png\" }\noverrides = { encoder = { backend = \"bc7e\" } }\n";
    let e = ConfigFile::parse(rule, "sdc.toml").unwrap_err();
    assert!(e.to_string().contains("encoder.backend"), "{e}");
    let e = r.add_toml(&format!("{head}{rule}")).unwrap_err();
    assert!(e.to_string().contains("encoder.backend"), "{e}");
    // The `nvtt` row comes first, so that value still names NVTT.
    let e = ConfigFile::parse("[defaults]\nencoder = { backend = \"nvtt\" }\n", "sdc.toml")
        .unwrap_err();
    assert!(e.to_string().contains("NVTT"), "{e}");

    let e = ConfigFile::parse("allow = [\"IN-FLOAT-LDR\"]\n", "sdc.toml").unwrap_err();
    assert!(e.to_string().contains("IN-FLOAT-LDR"), "{e}");
    let e = engine::codes::Code::parse_allow("IN-FLOAT-LDR").unwrap_err();
    assert!(
        e.contains("IN-FLOAT-LDR") && e.contains("removed in 2.0"),
        "{e}"
    );
}

#[test]
fn an_allow_list_naming_an_nvtt_code_is_named() {
    for code in ["ENC-NVTT-MISSING", "ENC-NVTT-VERSION", "ENC-NVTT-FAILED"] {
        let e = ConfigFile::parse(&format!("allow = [\"{code}\"]\n"), "sdc.toml").unwrap_err();
        assert!(e.to_string().contains(code), "{e}");
    }
}

/// Note FM: the 15 info and low-value warning codes 2.0 removed are named in an `allow` list, in `--set allow=` and in `--allow`.
#[test]
fn removed_codes_in_allow_lists_are_named() {
    for code in [
        "LOSS-BITDEPTH",
        "CHAN-CONSTANT-ALPHA-DATA",
        "CHAN-NORMAL-SHORT",
        "ALPHA-254",
        "ALPHA-NEAR-OPAQUE",
        "ALPHA-PBR-NORMAL-DROPPED",
        "IN-ICC-IGNORED",
        "SKIN-WET-FORM",
        "PACK-ORM-OVERRIDDEN",
        "RNM-AO-MISSING",
        "SET-COMPANION-OUTSIDE",
        "DERIVE-NO-SCAN",
        "DERIVE-NO-ENVMAP",
        "CHAN-AO-INVERTED",
        "CHAN-GLOSS-AS-ROUGH",
    ] {
        assert!(Code::parse(code).is_none(), "{code} is still a code");
        let e = ConfigFile::parse(&format!("allow = [\"{code}\"]\n"), "sdc.toml").unwrap_err();
        assert!(e.to_string().contains(code), "{e}");
        let e = Code::parse_allow(code).unwrap_err();
        assert!(e.contains(code) && e.contains("removed in 2.0"), "{e}");
    }
}

/// Note FM26: the Value range and Plastic look checks are gone, and so are their codes. An allow list that names one
/// gets the removal message from config and `Code::parse_allow` (so `--set` and `--allow` too).
#[test]
fn removed_check_codes_in_allow_lists_are_named() {
    for code in ["CHK-RANGE", "CHK-PLASTIC"] {
        assert!(Code::parse(code).is_none(), "{code} is still a code");
        let e = ConfigFile::parse(&format!("allow = [\"{code}\"]\n"), "sdc.toml").unwrap_err();
        assert!(e.to_string().contains(code), "{e}");
        let e = Code::parse_allow(code).unwrap_err();
        assert!(e.contains(code) && e.contains("removed in 2.0"), "{e}");
        let e = engine::overrides::parse_set(&format!("allow=[\"{code}\"]")).unwrap_err();
        assert!(e.contains(code), "{e}");
    }
    // The checks that stay are still allowable.
    assert!(Code::parse_allow("CHK-PARALLAX").is_ok());
}

/// Trace-debug #15: a quoted dotted key under `[files."x"]` used to pass the field check and then read as null (the
/// value silently dropped). It is refused with the way to write it; the unquoted form still works.
#[test]
fn a_quoted_dotted_files_key_is_refused_not_ignored() {
    let quoted =
        ConfigFile::parse("[files.\"a.dds\"]\n\"alpha.policy\" = \"cutout\"\n", "t").unwrap();
    let e = quoted.file_entries().unwrap_err().to_string();
    assert!(e.contains("alpha.policy") && e.contains("nested"), "{e}");
    let ok = ConfigFile::parse("[files.\"a.dds\"]\nalpha.policy = \"cutout\"\n", "t").unwrap();
    let (o, _) = ok.file_entries().unwrap();
    assert_eq!(o[0].value, json!("cutout"));
}

/// Note FM22: the output codes of the removed `.dds.json` sidecar and `patch-header` are gone. An allow list that
/// names one gets the removal message from config and `Code::parse_allow` (so `--set` and `--allow` too).
#[test]
fn removed_output_codes_in_allow_lists_are_named() {
    for code in [
        "OUT-SIDECAR-KEPT",
        "DDS-HEADER-PATCHED",
        "DDS-HEADER-UNPATCHABLE",
    ] {
        assert!(Code::parse(code).is_none(), "{code} is still a code");
        let e = ConfigFile::parse(&format!("allow = [\"{code}\"]\n"), "sdc.toml").unwrap_err();
        assert!(e.to_string().contains(code), "{e}");
        let e = Code::parse_allow(code).unwrap_err();
        assert!(e.contains(code) && e.contains("removed in 2.0"), "{e}");
    }
}

/// Note FM: the `[export] zip` key and the codes of the zip, the conversion report and the `also.json` record are
/// gone. A 1.x config names them (never a bare "unknown field"), and `ALSO-ZIP-FAILED`, `ALSO-REPORT-FAILED` and
/// `ALSO-OWNED-UNREADABLE` in an `allow` list, in `--set allow=` and in `--allow` get the removal message.
#[test]
fn the_export_zip_key_is_named() {
    for text in [
        "[export]\nzip = true\n",
        "[export]\nalso = [\"cm\"]\nzip = false\n",
    ] {
        let e = ConfigFile::parse(text, "sdc.toml").unwrap_err().to_string();
        assert!(
            e.contains("sdc.toml: export.zip: removed in 2.0;") && e.contains("zips"),
            "{e}"
        );
    }
    let ok = ConfigFile::parse("[export]\nalso = [\"cm\"]\n", "sdc.toml").unwrap();
    assert_eq!(ok.export.unwrap().also, vec![AlsoTarget::Cm]);
    // A preset names it the same way; `--set` too (the key is not settable, so the removal is what it says).
    let head = "schema_version = 1\nid = \"twin\"\nversion = \"1\"\nbase = \"cs-quality\"\n";
    let mut r = PresetRegistry::builtin().unwrap();
    let e = r
        .add_toml(&format!("{head}[export]\nzip = true\n"))
        .unwrap_err()
        .to_string();
    assert!(e.contains("export.zip: removed in 2.0"), "{e}");
    let e = engine::overrides::parse_set("export.zip=true").unwrap_err();
    assert!(
        e.contains("export.zip") && e.contains("removed in 2.0"),
        "{e}"
    );

    for code in [
        "ALSO-ZIP-FAILED",
        "ALSO-REPORT-FAILED",
        "ALSO-OWNED-UNREADABLE",
    ] {
        assert!(Code::parse(code).is_none(), "{code} is still a code");
        let e = ConfigFile::parse(&format!("allow = [\"{code}\"]\n"), "sdc.toml").unwrap_err();
        assert!(e.to_string().contains(code), "{e}");
        let e = Code::parse_allow(code).unwrap_err();
        assert!(e.contains(code) && e.contains("removed in 2.0"), "{e}");
        let e = engine::overrides::parse_set(&format!("allow=[\"{code}\"]")).unwrap_err();
        assert!(e.contains(code), "{e}");
    }
}

/// FM43: the vanilla quality and compact presets mark diffuse, glow and skin albedo perceptual; the sub-type's partial
/// `encoder` table merges into the base one (effort and the BC4/BC5 radius stay). Other colour types, the data maps
/// and every other built-in preset stay off. (`profile` does not run the resolve step that drops it for data alpha.)
#[test]
fn the_vanilla_presets_mark_three_colour_types_perceptual() {
    let r = PresetRegistry::builtin().unwrap();
    for id in ["vanilla-quality", "vanilla-compact"] {
        for st in Subtype::ALL {
            let p = r.profile(id, Target::Cs, st).unwrap();
            let on = matches!(st, Subtype::Diffuse | Subtype::Glow | Subtype::SkinAlbedo);
            assert_eq!(p.encoder.perceptual, on, "{id} {st:?}");
            assert_eq!(p.encoder.effort, Effort::Veryslow, "{id} {st:?}");
        }
    }
    let d = r
        .profile("vanilla-compact", Target::Cs, Subtype::Diffuse)
        .unwrap();
    assert_eq!(d.encoder.bc45_search_radius, SearchRadius::Fast);
    for id in ["sdc-base", "cs-quality", "cs-compact", "fast-preview"] {
        for st in Subtype::ALL {
            let p = r.profile(id, Target::Cs, st).unwrap();
            assert!(!p.encoder.perceptual, "{id} {st:?}");
        }
    }
}

/// Backlog T10/T11: keys a defaults table cannot use are load errors that name the table and the key.
#[test]
fn defaults_tables_refuse_keys_that_would_do_nothing() {
    let r = PresetRegistry::builtin().unwrap();
    let chain = r.chain("cs-quality").unwrap();
    let check = |defaults: serde_json::Value, subtypes: serde_json::Value| {
        let mut cfg = ConfigFile {
            defaults,
            ..Default::default()
        };
        if let serde_json::Value::Object(m) = subtypes {
            for (k, v) in m {
                cfg.subtypes
                    .insert(serde_json::from_value(json!(k)).unwrap(), v);
            }
        }
        check_chain(&chain, &[&cfg], Target::Cs)
    };
    let msg = |e: Result<(), PresetError>| match e {
        Err(PresetError::Invalid(m)) => m,
        other => panic!("expected Invalid, got {other:?}"),
    };
    let m = msg(check(json!({ "subtype": "glow" }), json!({})));
    assert!(
        m.contains("[defaults]") && m.contains("'subtype'") && m.contains("no effect"),
        "{m}"
    );
    let m = msg(check(
        json!({}),
        json!({ "diffuse": { "target": "vanilla" } }),
    ));
    assert!(
        m.contains("[subtypes.diffuse]") && m.contains("'target'"),
        "{m}"
    );
    let m = msg(check(
        json!({ "resize": "half", "resize_floor": 1, "resize_reference": [2, 2] }),
        json!({}),
    ));
    assert!(m.contains("'resize_reference'"), "{m}");
    assert!(check(json!({ "resize": "half" }), json!({})).is_ok());
}
