//! Version checks and the removed-key table for presets and configs (Note FM). A file is refused when its schema is
//! newer than this sdc, and when it names a key or value that a past version removed: the error says where and what to write
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
    /// `<dotted path>: removed in <since>; <hint>`.
    #[error("{0}")]
    Removed(String),
}

/// One key or value that a later version no longer reads. `key` is dot-separated segments compared with the last segments of
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
    /// The version that removed it, as shown in the message (`"2.0"`).
    pub since: &'static str,
}

/// `<dotted path>: removed in <since>; <hint>`.
pub(crate) fn removed_line(path: &str, r: &Removed) -> String {
    format!("{path}: removed in {}; {}", r.since, r.hint)
}

pub const REMOVED: &[Removed] = &[
    Removed {
        key: "subtypes.rfaos",
        value: None,
        hint: "the sub-type is skin_rfaos",
        since: "2.0",
    },
    Removed {
        key: "subtype",
        value: Some("rfaos"),
        hint: "use skin_rfaos",
        since: "2.0",
    },
    Removed {
        key: "subtypes",
        value: Some("rfaos"),
        hint: "use skin_rfaos",
        since: "2.0",
    },
    Removed {
        key: "encoder.backend",
        value: Some("nvtt"),
        hint: "NVTT was removed in 2.0; delete the key",
        since: "2.0",
    },
    // After the `nvtt` row: the first match names the value.
    Removed {
        key: "encoder.backend",
        value: None,
        hint: "the texture format picks the encoder; delete the key",
        since: "2.0",
    },
    Removed {
        key: "encoder.rgbcx_level",
        value: None,
        hint: "the BC1/BC3 level is fixed at 18; delete the key",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("ENC-NVTT-MISSING"),
        hint: "ENC-NVTT-MISSING was removed in 2.0 with NVTT; delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("ENC-NVTT-VERSION"),
        hint: "ENC-NVTT-VERSION was removed in 2.0 with NVTT; delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("ENC-NVTT-FAILED"),
        hint: "ENC-NVTT-FAILED was removed in 2.0 with NVTT; delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("IN-FLOAT-LDR"),
        hint: "IN-FLOAT-LDR was removed in 2.0 (a float source is linear light; nothing reports it); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("LOSS-BITDEPTH"),
        hint: "LOSS-BITDEPTH was removed in 2.0 (a 16-bit or float source is written with 8-bit endpoints and nothing reports it); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("CHAN-CONSTANT-ALPHA-DATA"),
        hint: "CHAN-CONSTANT-ALPHA-DATA was removed in 2.0 (nothing reports a constant `data` alpha); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("CHAN-NORMAL-SHORT"),
        hint: "CHAN-NORMAL-SHORT was removed in 2.0 (nothing reports a short normal; mip 0 is renormalised as before); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("ALPHA-254"),
        hint: "ALPHA-254 was removed in 2.0 (nothing reports opaque alpha that decodes as 254); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("ALPHA-NEAR-OPAQUE"),
        hint: "ALPHA-NEAR-OPAQUE was removed in 2.0 (alpha 251 to 254 is still treated as opaque, silently); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("ALPHA-PBR-NORMAL-DROPPED"),
        hint: "ALPHA-PBR-NORMAL-DROPPED was removed in 2.0 (a `pbr_normal` still drops its vanilla specular alpha, silently); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("IN-ICC-IGNORED"),
        hint: "IN-ICC-IGNORED was removed in 2.0 (an embedded ICC profile is still ignored, silently); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("SKIN-WET-FORM"),
        hint: "SKIN-WET-FORM was removed in 2.0; since 2.1 a skin_wet is always a BC7 normal + mask and a grey one is not written (SKIN-WET-GREY); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("PACK-ORM-OVERRIDDEN"),
        hint: "PACK-ORM-OVERRIDDEN was removed in 2.0 (a loose map still wins over an `_orm` channel, silently); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("RNM-AO-MISSING"),
        hint: "RNM-AO-MISSING was removed in 2.0 (a `bent` layer with no AO source still uses a uniform weight, silently); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("SET-COMPANION-OUTSIDE"),
        hint: "SET-COMPANION-OUTSIDE was removed in 2.0 (a companion on disk outside the batch is no longer reported); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("DERIVE-NO-SCAN"),
        hint: "DERIVE-NO-SCAN was removed in 2.0 (nothing reports a missing scan); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("DERIVE-NO-ENVMAP"),
        hint: "DERIVE-NO-ENVMAP was removed in 2.0 (nothing reports a missing Environment Map mesh); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("CHAN-AO-INVERTED"),
        hint: "CHAN-AO-INVERTED was removed in 2.0 (nothing guesses at an inverted AO map); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("CHAN-GLOSS-AS-ROUGH"),
        hint: "CHAN-GLOSS-AS-ROUGH was removed in 2.0 (nothing guesses at gloss stored as roughness); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("OUT-SIDECAR-KEPT"),
        hint: "OUT-SIDECAR-KEPT was removed in 2.0 (Tessalith no longer writes or removes .dds.json files); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("DDS-HEADER-PATCHED"),
        hint: "DDS-HEADER-PATCHED was removed in 2.0 (patch-header is gone); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("DDS-HEADER-UNPATCHABLE"),
        hint: "DDS-HEADER-UNPATCHABLE was removed in 2.0 (patch-header is gone); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "validation.min_mip_psnr_db",
        value: None,
        hint: "the value is fixed at 30 dB; delete the key",
        since: "2.0",
    },
    Removed {
        key: "validation.coverage_tolerance_pts",
        value: None,
        hint: "the value is fixed at 2 points; delete the key",
        since: "2.0",
    },
    Removed {
        key: "validation.complex_m_warn_alpha",
        value: None,
        hint: "the value is fixed at 247; delete the key",
        since: "2.0",
    },
    Removed {
        key: "validation.bc6h_min_peak_ratio",
        value: None,
        hint: "the value is fixed at 0.8; delete the key",
        since: "2.0",
    },
    Removed {
        key: "validation.pgpatcher_non_white_share",
        value: None,
        hint: "the value is fixed at 0.5; delete the key",
        since: "2.0",
    },
    Removed {
        key: "^defaults.packing.fill",
        value: None,
        hint: "fills are set per set; delete the [defaults] packing.fill line by hand, then set each fill on its packed output's card",
        since: "2.0",
    },
    Removed {
        key: "bc1_model",
        value: None,
        hint: "BC1 always encodes for NVIDIA + AMD in 2.0; delete the key",
        since: "2.0",
    },
    Removed {
        key: "encoder.bc1_transparent_black",
        value: None,
        hint: "delete the line; use encoder.bc1_alpha for 1-bit alpha",
        since: "2.0",
    },
    Removed {
        key: "^export.zip",
        value: None,
        hint: "an Also export no longer writes zips or a SHA256SUMS file; delete the key",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("ALSO-ZIP-FAILED"),
        hint: "ALSO-ZIP-FAILED was removed in 2.0 (an Also export writes no zip); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("ALSO-REPORT-FAILED"),
        hint: "ALSO-REPORT-FAILED was removed in 2.0 (an Also export writes no conversion report); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("ALSO-OWNED-UNREADABLE"),
        hint: "ALSO-OWNED-UNREADABLE was removed in 2.0 (an Also export keeps no also.json record); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("CHK-RANGE"),
        hint: "CHK-RANGE was removed in 2.0 (the Value range check is gone); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("CHK-PLASTIC"),
        hint: "CHK-PLASTIC was removed in 2.0 (the Plastic look check is gone); delete it from the list",
        since: "2.0",
    },
    Removed {
        key: "allow",
        value: Some("GEN-STALE"),
        hint: "GEN-STALE was removed in 3.0 (Tessalith no longer generates maps); delete it from the list",
        since: "3.0",
    },
    Removed {
        key: "resize",
        value: Some("pot-down"),
        hint: "the power of two is automatic; delete the key",
        since: "2.0",
    },
    Removed {
        key: "drop_top_mip",
        value: None,
        hint: "use resize = \"half\"",
        since: "2.0",
    },
    Removed {
        key: "cubemap.layout",
        value: Some("equirect"),
        hint: "convert it to six faces, a cross or a strip first",
        since: "3.0",
    },
    Removed {
        key: "cubemap.layout",
        value: Some("mirror_ball"),
        hint: "convert it to six faces, a cross or a strip first",
        since: "3.0",
    },
    Removed {
        key: "cubemap.layout",
        value: Some("angular"),
        hint: "convert it to six faces, a cross or a strip first",
        since: "3.0",
    },
    Removed {
        key: "cubemap.layout",
        value: Some("photo"),
        hint: "convert it to six faces, a cross or a strip first",
        since: "3.0",
    },
    Removed {
        key: "cubemap.ground_fill",
        value: None,
        hint: "a cube's lower half is the source's own pixels; delete the key",
        since: "3.0",
    },
    Removed {
        key: "cubemap.horizon",
        value: None,
        hint: "a cube's lower half is the source's own pixels; delete the key",
        since: "3.0",
    },
    // RNM left in 3.0 (Note GW3). One leaf row per field, never a bare `rnm`: that would also refuse a file called
    // `rnm`. The rows are unanchored because `rnm` was a profile field under `[defaults]`, `[subtypes.X]`, `[files]` and
    // rules alike; the leaf ends a `[files."rnm"]` entry's own keys can never reach.
    Removed {
        key: "rnm.enabled",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.order",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bump.strength",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bump.flip_green",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bump.mask",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bump2.strength",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bump2.flip_green",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bump2.mask",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.detail.strength",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.detail.flip_green",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.detail.mask",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.detail2.strength",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.detail2.flip_green",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.detail2.mask",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bent.strength",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bent.flip_green",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bent.mask",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bent.mode",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bent.ao_power",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.bent.ao",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "rnm.target",
        value: None,
        hint: "normal layers are no longer blended into a normal; delete the key",
        since: "3.0",
    },
    Removed {
        key: "allow",
        value: Some("RNM-NO-BASE"),
        hint: "RNM-NO-BASE was removed in 3.0 with RNM normal layers; delete it from the list",
        since: "3.0",
    },
    Removed {
        key: "allow",
        value: Some("RNM-IGNORED-EYE"),
        hint: "RNM-IGNORED-EYE was removed in 3.0 with RNM normal layers; delete it from the list",
        since: "3.0",
    },
    Removed {
        key: "allow",
        value: Some("RNM-MASK-ORPHAN"),
        hint: "RNM-MASK-ORPHAN was removed in 3.0 with RNM normal layers; delete it from the list",
        since: "3.0",
    },
    Removed {
        key: "allow",
        value: Some("RNM-LAYER-SHADOWED"),
        hint: "RNM-LAYER-SHADOWED was removed in 3.0 with RNM normal layers; delete it from the list",
        since: "3.0",
    },
    Removed {
        key: "helper.role",
        value: Some("bump"),
        hint: "normal layers are gone; use none, or delete the key",
        since: "3.0",
    },
    Removed {
        key: "helper.role",
        value: Some("bumpmask"),
        hint: "normal layers are gone; use none, or delete the key",
        since: "3.0",
    },
    Removed {
        key: "helper.role",
        value: Some("bump2"),
        hint: "normal layers are gone; use none, or delete the key",
        since: "3.0",
    },
    Removed {
        key: "helper.role",
        value: Some("bumpmask2"),
        hint: "normal layers are gone; use none, or delete the key",
        since: "3.0",
    },
    Removed {
        key: "helper.role",
        value: Some("detailnorm"),
        hint: "normal layers are gone; use none, or delete the key",
        since: "3.0",
    },
    Removed {
        key: "helper.role",
        value: Some("detailnormmask"),
        hint: "normal layers are gone; use none, or delete the key",
        since: "3.0",
    },
    Removed {
        key: "helper.role",
        value: Some("detailnorm2"),
        hint: "normal layers are gone; use none, or delete the key",
        since: "3.0",
    },
    Removed {
        key: "helper.role",
        value: Some("detailnormmask2"),
        hint: "normal layers are gone; use none, or delete the key",
        since: "3.0",
    },
    Removed {
        key: "helper.role",
        value: Some("bent"),
        hint: "normal layers are gone; use none, or delete the key",
        since: "3.0",
    },
    Removed {
        key: "helper.role",
        value: Some("bentmask"),
        hint: "normal layers are gone; use none, or delete the key",
        since: "3.0",
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

/// The rows that `segs` with a value matching `has` hits: the first row only, except that a list (`allow`) gets one
/// row per removed value it holds (rows of one key with `value: Some` name distinct values).
fn rows_hit<'r>(segs: &[&str], list: bool, has: impl Fn(&str) -> bool) -> Vec<&'r Removed> {
    let mut hits = REMOVED
        .iter()
        .filter(|r| key_hit(r, segs) && r.value.is_none_or(&has));
    match (list, hits.next()) {
        (true, Some(first)) if first.value.is_some() => std::iter::once(first)
            .chain(hits.filter(|r| r.value.is_some()))
            .collect(),
        (_, first) => first.into_iter().collect(),
    }
}

fn walk<'a>(v: &'a Value, segs: &mut Vec<&'a str>, shown: &mut String, out: &mut Vec<String>) {
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
                let hits = rows_hit(segs, child.is_array(), value_hit);
                if hits.is_empty() {
                    walk(child, segs, shown, out);
                }
                out.extend(hits.iter().map(|r| removed_line(shown, r)));
                segs.pop();
                shown.truncate(keep);
            }
        }
        Value::Array(a) => {
            for (i, child) in a.iter().enumerate() {
                let keep = shown.len();
                shown.push_str(&format!("[{i}]"));
                walk(child, segs, shown, out);
                shown.truncate(keep);
            }
        }
        _ => {}
    }
}

/// Ruling FR-a: at most this many hits are listed, then "and N more".
const LISTED: usize = 10;

fn listed(hits: Vec<String>) -> Option<String> {
    let more = hits.len().saturating_sub(LISTED);
    let mut m = hits.into_iter().take(LISTED).collect::<Vec<_>>().join("\n");
    if more > 0 {
        m.push_str(&format!("\nand {more} more"));
    }
    (!m.is_empty()).then_some(m)
}

/// Every removed key or value in a config or preset table, one per line (`<dotted path>: removed in <since>; <hint>`),
/// in key order; past [`LISTED`] the last line is `and N more`.
pub fn removed_in(v: &Value) -> Option<String> {
    let mut out = vec![];
    walk(v, &mut vec![], &mut String::new(), &mut out);
    listed(out)
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
    let hits = rows_hit(&segs, items.len() > 1, |w| items.contains(&w));
    listed(hits.iter().map(|r| removed_line(field, r)).collect())
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

    /// Ruling FR-a: every hit in one error (10 lines, then "and N more"), so one editing pass fixes a 1.x file.
    #[test]
    fn every_removed_key_is_listed_in_one_error() {
        let mut text =
            String::from("[defaults]\nencoder = { backend = \"bc7e\" }\nresize = \"pot-down\"\n");
        for i in 0..20 {
            text.push_str(&format!("[files.\"t{i:02}.png\"]\ndrop_top_mip = true\n"));
        }
        let MigrateError::Removed(m) = migrate_config(t(&text)).unwrap_err() else {
            panic!()
        };
        let lines: Vec<&str> = m.lines().collect();
        assert_eq!(lines.len(), 11, "{m}");
        assert!(
            lines[0].starts_with("defaults.encoder.backend: removed in 2.0;"),
            "{m}"
        );
        assert!(
            lines[1].starts_with("defaults.resize: removed in 2.0;"),
            "{m}"
        );
        assert!(
            lines[2].starts_with("files.\"t00.png\".drop_top_mip: removed in 2.0;"),
            "{m}"
        );
        assert_eq!(lines[10], "and 12 more");
        // A single hit reads as before.
        let one = removed_in(&t("[files.\"a.png\"]\ndrop_top_mip = true")).unwrap();
        assert_eq!(
            one,
            "files.\"a.png\".drop_top_mip: removed in 2.0; use resize = \"half\""
        );
        // Two removed codes in one allow list, from a config or `--set`.
        let m = removed_in(&t("allow = [\"CHK-RANGE\", \"CHK-PLASTIC\"]")).unwrap();
        assert!(
            m.contains("CHK-RANGE was") && m.contains("CHK-PLASTIC was"),
            "{m}"
        );
        let m = removed_set("allow", "[\"CHK-RANGE\", \"CHK-PLASTIC\"]").unwrap();
        assert!(
            m.contains("CHK-RANGE was") && m.contains("CHK-PLASTIC was"),
            "{m}"
        );
        // The nvtt row still wins over the generic backend row: one line, naming the value.
        let m = removed_in(&t("[defaults]\nencoder = { backend = \"nvtt\" }")).unwrap();
        assert!(!m.contains('\n') && m.contains("NVTT was removed"), "{m}");
    }

    #[test]
    fn bc1_transparent_black_names_bc1_alpha() {
        let m = removed_in(&t(
            "[defaults]\nencoder = { effort = \"veryslow\", bc1_transparent_black = false }",
        ))
        .unwrap();
        assert!(
            m.starts_with("defaults.encoder.bc1_transparent_black: removed in 2.0;")
                && m.ends_with("delete the line; use encoder.bc1_alpha for 1-bit alpha"),
            "{m}"
        );
        assert!(removed_in(&t("[defaults]\nencoder = { bc1_alpha = true }")).is_none());
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

    #[test]
    fn a_2_0_row_still_says_2_0() {
        let m = removed_set("subtypes.rfaos", "x").unwrap();
        assert_eq!(
            m,
            "subtypes.rfaos: removed in 2.0; the sub-type is skin_rfaos"
        );
    }

    #[test]
    fn a_3_0_row_says_3_0() {
        let r = Removed {
            key: "a.b",
            value: None,
            hint: "delete the key",
            since: "3.0",
        };
        assert_eq!(
            removed_line("a.b", &r),
            "a.b: removed in 3.0; delete the key"
        );
    }
}
