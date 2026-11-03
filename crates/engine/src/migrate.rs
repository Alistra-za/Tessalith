//! Version checks and the removed-key table for presets and configs (Note FM). A file is refused when its schema is
//! newer than this sdc, and when it names a key or value that 2.0 removed: the error says where and what to write
//! instead. Nothing is upgraded in place; there are no step tables. Every removal adds a row to [`REMOVED`].
use crate::presets::SCHEMA_VERSION as PRESET_SCHEMA;
use toml::Value;

/// Version of `config.toml` / `sdc.toml`; an absent `schema_version` is 1.
pub const CONFIG_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MigrateError {
    #[error("missing or invalid {0} (a whole number from 1)")]
    NoVersion(&'static str),
    #[error("{key} {found} is newer than this sdc supports ({current})")]
    Newer {
        key: &'static str,
        found: u32,
        current: u32,
    },
    /// `<dotted path>: removed in 2.0; <hint>`.
    #[error("{0}")]
    Removed(String),
}

/// One key or value that 2.0 no longer reads. `key` is dot-separated segments compared with the last segments of
/// each key's path, so `encoder.backend` matches under `[defaults]`, `[subtypes.X]`, `[files."k"]` and `[[rules]]`
/// tables alike (array indices are not segments; a file key that contains dots is one segment). A table key matches
/// too (`subtypes.rfaos`). `value: None` matches any value; `Some(v)` matches a string equal to `v` or an array
/// holding the string `v`. The error names the key even when its value is harmless. A key that starts with `^` is
/// anchored: it matches the whole path from the root, so `^defaults.packing.fill` is the top-level `[defaults]` table
/// only, not `[files."defaults"]`.
pub struct Removed {
    pub key: &'static str,
    pub value: Option<&'static str>,
    pub hint: &'static str,
}

pub const REMOVED: &[Removed] = &[
    Removed {
        key: "subtypes.rfaos",
        value: None,
        hint: "the sub-type is skin_rfaos",
    },
    Removed {
        key: "subtype",
        value: Some("rfaos"),
        hint: "use skin_rfaos",
    },
    Removed {
        key: "subtypes",
        value: Some("rfaos"),
        hint: "use skin_rfaos",
    },
    Removed {
        key: "encoder.backend",
        value: Some("nvtt"),
        hint: "NVTT was removed in 2.0; delete the key",
    },
    // After the `nvtt` row: the first match names the value.
    Removed {
        key: "encoder.backend",
        value: None,
        hint: "the texture format picks the encoder; delete the key",
    },
    Removed {
        key: "encoder.rgbcx_level",
        value: None,
        hint: "the BC1/BC3 level is fixed at 18; delete the key",
    },
    Removed {
        key: "allow",
        value: Some("ENC-NVTT-MISSING"),
        hint: "ENC-NVTT-MISSING was removed in 2.0 with NVTT; delete it from the list",
    },
    Removed {
        key: "allow",
        value: Some("ENC-NVTT-VERSION"),
        hint: "ENC-NVTT-VERSION was removed in 2.0 with NVTT; delete it from the list",
    },
    Removed {
        key: "allow",
        value: Some("ENC-NVTT-FAILED"),
        hint: "ENC-NVTT-FAILED was removed in 2.0 with NVTT; delete it from the list",
    },
    Removed {
        key: "allow",
        value: Some("IN-FLOAT-LDR"),
        hint: "IN-FLOAT-LDR was removed in 2.0 (a float source is linear light; nothing reports it); delete it from the list",
    },
    Removed {
        key: "validation.min_mip_psnr_db",
        value: None,
        hint: "the value is fixed at 30 dB; delete the key",
    },
    Removed {
        key: "validation.coverage_tolerance_pts",
        value: None,
        hint: "the value is fixed at 2 points; delete the key",
    },
    Removed {
        key: "validation.complex_m_warn_alpha",
        value: None,
        hint: "the value is fixed at 247; delete the key",
    },
    Removed {
        key: "validation.bc6h_min_peak_ratio",
        value: None,
        hint: "the value is fixed at 0.8; delete the key",
    },
    Removed {
        key: "validation.pgpatcher_non_white_share",
        value: None,
        hint: "the value is fixed at 0.5; delete the key",
    },
    Removed {
        key: "^defaults.packing.fill",
        value: None,
        hint: "fills are set per set; delete the [defaults] packing.fill line by hand, then set each fill on its packed output's card",
    },
];

fn key_hit(r: &Removed, segs: &[&str]) -> bool {
    let (anchored, key) = match r.key.strip_prefix('^') {
        Some(k) => (true, k),
        None => (false, r.key),
    };
    let want: Vec<&str> = key.split('.').collect();
    if anchored {
        segs == want
    } else {
        segs.ends_with(&want)
    }
}

/// A key shown the way a TOML header writes it: quoted when it is not a bare key.
fn shown_key(k: &str) -> String {
    if !k.is_empty()
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        k.to_string()
    } else {
        format!("\"{k}\"")
    }
}

fn walk<'a>(v: &'a Value, segs: &mut Vec<&'a str>, shown: &mut String) -> Option<String> {
    match v {
        Value::Table(t) => {
            for (k, child) in t {
                let keep = shown.len();
                if !shown.is_empty() {
                    shown.push('.');
                }
                shown.push_str(&shown_key(k));
                segs.push(k);
                let value_hit = |w: &str| match child {
                    Value::String(s) => s == w,
                    Value::Array(a) => a.iter().any(|x| x.as_str() == Some(w)),
                    _ => false,
                };
                let hit = REMOVED
                    .iter()
                    .find(|r| key_hit(r, segs) && r.value.is_none_or(value_hit));
                if let Some(r) = hit {
                    return Some(format!("{shown}: removed in 2.0; {}", r.hint));
                }
                if let Some(m) = walk(child, segs, shown) {
                    return Some(m);
                }
                segs.pop();
                shown.truncate(keep);
            }
        }
        Value::Array(a) => {
            for (i, child) in a.iter().enumerate() {
                let keep = shown.len();
                shown.push_str(&format!("[{i}]"));
                if let Some(m) = walk(child, segs, shown) {
                    return Some(m);
                }
                shown.truncate(keep);
            }
        }
        _ => {}
    }
    None
}

/// First removed key or value in a config or preset table: `<dotted path>: removed in 2.0; <hint>`.
pub fn removed_in(v: &Value) -> Option<String> {
    walk(v, &mut vec![], &mut String::new())
}

/// The same for one `--set field=value`. `field` may carry a `file:<glob>.` prefix: only its last segments count.
pub fn removed_set(field: &str, value: &str) -> Option<String> {
    let segs: Vec<&str> = field.split('.').collect();
    // `["a", "b"]` or `a`: the elements, unquoted.
    let items: Vec<&str> = value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|e| e.trim().trim_matches(['"', '\'']))
        .collect();
    let r = REMOVED
        .iter()
        .find(|r| key_hit(r, &segs) && r.value.is_none_or(|w| items.contains(&w)))?;
    Some(format!("{field}: removed in 2.0; {}", r.hint))
}

fn toml_version(v: &Value, key: &'static str, absent: Option<u32>) -> Result<u32, MigrateError> {
    match v.get(key) {
        None => absent.ok_or(MigrateError::NoVersion(key)),
        Some(x) => x
            .as_integer()
            .and_then(|i| u32::try_from(i).ok())
            .ok_or(MigrateError::NoVersion(key)),
    }
}

/// The version first (a newer file may use keys this sdc does not know), then the removed keys.
fn check(v: Value, from: u32, current: u32) -> Result<Value, MigrateError> {
    let key = "schema_version";
    if from == 0 {
        return Err(MigrateError::NoVersion(key));
    }
    if from > current {
        return Err(MigrateError::Newer {
            key,
            found: from,
            current,
        });
    }
    match removed_in(&v) {
        Some(m) => Err(MigrateError::Removed(m)),
        None => Ok(v),
    }
}

/// A preset file's table; `schema_version` is required.
pub fn migrate_preset(v: Value) -> Result<Value, MigrateError> {
    let from = toml_version(&v, "schema_version", None)?;
    check(v, from, PRESET_SCHEMA)
}

/// A config file's table; an absent `schema_version` is 1.
pub fn migrate_config(v: Value) -> Result<Value, MigrateError> {
    let from = toml_version(&v, "schema_version", Some(1))?;
    check(v, from, CONFIG_SCHEMA)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Value {
        Value::Table(toml::from_str(s).unwrap())
    }

    const RULES_CASE: &str = "[[rules]]\nsubtype = \"rfaos\"\n[rules.match]\nsuffix = \"rfaos\"\n";

    #[test]
    fn presets_need_a_version_and_refuse_a_newer_one() {
        let v1 = t("schema_version = 1\nid = \"x\"");
        assert_eq!(migrate_preset(v1.clone()), Ok(v1));
        assert_eq!(
            migrate_preset(t("id = \"x\"")),
            Err(MigrateError::NoVersion("schema_version"))
        );
        assert_eq!(
            migrate_preset(t("schema_version = 0")),
            Err(MigrateError::NoVersion("schema_version"))
        );
        assert_eq!(
            migrate_preset(t("schema_version = \"1\"")),
            Err(MigrateError::NoVersion("schema_version"))
        );
        assert_eq!(
            migrate_preset(t("schema_version = 2")),
            Err(MigrateError::Newer {
                key: "schema_version",
                found: 2,
                current: 1
            })
        );
    }

    #[test]
    fn configs_default_to_version_1() {
        let none = t("preset = \"cs-quality\"");
        assert_eq!(migrate_config(none.clone()), Ok(none));
        let v1 = t("schema_version = 1");
        assert_eq!(migrate_config(v1.clone()), Ok(v1));
        assert_eq!(
            migrate_config(t("schema_version = 7")),
            Err(MigrateError::Newer {
                key: "schema_version",
                found: 7,
                current: CONFIG_SCHEMA
            })
        );
    }

    #[test]
    fn a_removed_key_is_named_at_any_depth() {
        for text in [
            "[subtypes.rfaos]\nformat = \"bc7\"",
            "[files.\"a/b.png\"]\nsubtype = \"rfaos\"",
            RULES_CASE,
        ] {
            let e = migrate_config(t(text)).unwrap_err();
            let MigrateError::Removed(m) = e else {
                panic!("{text}")
            };
            assert!(m.contains("rfaos") && m.contains("skin_rfaos"), "{m}");
        }
        let m = removed_in(&t("[files.\"a.b/c.png\"]\nsubtype = \"rfaos\"")).unwrap();
        assert!(
            m.starts_with("files.\"a.b/c.png\".subtype: removed in 2.0;"),
            "{m}"
        );
        let m = removed_in(&t(RULES_CASE)).unwrap();
        assert!(m.starts_with("rules[0].subtype:"), "{m}");
        let m = removed_in(&t("[[rules]]\n[rules.match]\nsubtypes = [\"rfaos\"]")).unwrap();
        assert!(m.starts_with("rules[0].match.subtypes:"), "{m}");
        assert!(removed_in(&t("[subtypes.skin_rfaos]\nformat = \"bc7\"")).is_none());
        assert!(removed_in(&t("[files.\"rfaos\"]\nformat = \"bc7\"")).is_none());
    }

    #[test]
    fn a_preset_with_a_removed_key_is_refused_after_the_version_check() {
        let e = migrate_preset(t("schema_version = 1\n[subtypes.rfaos]")).unwrap_err();
        assert!(matches!(e, MigrateError::Removed(_)), "{e}");
        let e = migrate_preset(t("schema_version = 2\n[subtypes.rfaos]")).unwrap_err();
        assert!(matches!(e, MigrateError::Newer { .. }), "{e}");
    }

    #[test]
    fn removed_set_names_the_fix() {
        assert!(removed_set("subtype", "rfaos")
            .unwrap()
            .contains("skin_rfaos"));
        assert!(removed_set("file:a.png.subtype", "\"rfaos\"").is_some());
        assert_eq!(removed_set("format", "bc7"), None);
        assert_eq!(removed_set("subtype", "diffuse"), None);
    }

    #[test]
    fn the_validation_thresholds_and_the_defaults_fill_are_named() {
        let m = removed_in(&t(
            "[defaults]\nvalidation = { coverage_tolerance_pts = 3.0 }",
        ))
        .unwrap();
        assert!(
            m.starts_with("defaults.validation.coverage_tolerance_pts: removed in 2.0;"),
            "{m}"
        );
        let m = removed_in(&t("[defaults]\npacking = { fill = { ao = 1.0 } }")).unwrap();
        assert!(
            m.starts_with("defaults.packing.fill: removed in 2.0;") && m.contains("per set"),
            "{m}"
        );
        assert!(removed_in(&t("[defaults]\nvalidation = { mips = \"smallest\" }")).is_none());
        assert!(removed_in(&t("[files.\"a_rmaos.png\".packing.fill]\nao = 10")).is_none());
        // Anchored at the root: a file or sub-type that is merely named `defaults` is not the `[defaults]` table.
        assert!(removed_in(&t("[files.\"defaults\".packing.fill]\nao = 10")).is_none());
        assert!(removed_in(&t("[subtypes.defaults.packing.fill]\nao = 10")).is_none());
        assert!(removed_in(&t("[defaults.packing.fill]\nao = 10")).is_some());
        let m = removed_set("validation.bc6h_min_peak_ratio", "0.5").unwrap();
        assert!(m.contains("fixed at 0.8"), "{m}");
        assert_eq!(removed_set("packing.fill.ao", "10"), None);
        assert_eq!(removed_set("validation.mips", "smallest"), None);
    }
}
