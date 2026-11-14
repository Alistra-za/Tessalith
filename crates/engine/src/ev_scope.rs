//! Which outputs written by an older encode version `--on-existing if-changed` re-encodes (`--ev`, `--property`;
//! Note JA). `--ev` alone covers every output; `--property` words narrow it to the outputs a fix touched: within one
//! value every word must hold (AND), across repeated values any value may hold (OR).
use crate::config::{AlphaPolicy, Format, Profile, Subtype};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Every format, in declared order (the `--property` words). `formats_are_every_format` keeps it complete.
const FORMATS: [Format; 11] = [
    Format::Bc1,
    Format::Bc3,
    Format::Bc4,
    Format::Bc5,
    Format::Bc6hUf16,
    Format::Bc7,
    Format::Rgba8,
    Format::Rgb8,
    Format::R8,
    Format::Rg8,
    Format::Rgba16f,
];

/// The error of a comma list that can never hold.
const NEVER_HOLDS: &str = "a comma list must all hold; repeat --property for alternatives";

/// One `--property` word, decided per output from its profile and its record.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(into = "String", try_from = "String")]
#[schemars(with = "String")]
pub enum Property {
    /// The output's format, by its config id (`bc1`, `bc7`, ...).
    Format(Format),
    /// The format stores the alpha and the policy keeps it (BC1 only with one-bit alpha).
    Alpha,
    /// `Alpha`, and the alpha is alpha-tested: `cutout`, `cutout_blend`, or `blend` on one-bit-alpha BC1.
    Cutout,
    /// The written file has more than one mip level.
    Mips,
    /// The output's texture type, by its `Subtype::id`.
    Subtype(Subtype),
}

impl Property {
    /// The property `word` names: trimmed, ASCII case-insensitive. `None` for an unknown word.
    pub fn parse(word: &str) -> Option<Property> {
        let w = word.trim().to_ascii_lowercase();
        match w.as_str() {
            "alpha" => return Some(Property::Alpha),
            "cutout" => return Some(Property::Cutout),
            "mips" => return Some(Property::Mips),
            _ => {}
        }
        if let Ok(f) = serde_json::from_value(serde_json::Value::String(w.clone())) {
            return Some(Property::Format(f));
        }
        Subtype::from_id(&w).map(Property::Subtype)
    }

    /// Every property, formats first, then `alpha`, `cutout`, `mips`, then the texture types.
    fn all() -> Vec<Property> {
        FORMATS
            .into_iter()
            .map(Property::Format)
            .chain([Property::Alpha, Property::Cutout, Property::Mips])
            .chain(Subtype::ALL.into_iter().map(Property::Subtype))
            .collect()
    }

    /// Every valid word, for an error message.
    pub fn words() -> Vec<String> {
        Self::all().into_iter().map(String::from).collect()
    }

    /// Whether the output holds the property. `p` is the job's profile; `levels` is the record's
    /// `validation.levels` (the written file's mip count), `None` when the record lacks it, which `mips` counts in.
    pub fn holds(&self, p: &Profile, levels: Option<u32>) -> bool {
        let alpha = || {
            p.alpha.policy != AlphaPolicy::Remove
                && if p.format == Format::Bc1 {
                    p.bc1_one_bit_alpha()
                } else {
                    p.format.has_alpha()
                }
        };
        match self {
            Property::Format(f) => p.format == *f,
            Property::Alpha => alpha(),
            Property::Cutout => {
                alpha()
                    && match p.alpha.policy {
                        AlphaPolicy::Cutout | AlphaPolicy::CutoutBlend => true,
                        AlphaPolicy::Blend => p.bc1_one_bit_alpha(),
                        AlphaPolicy::Data | AlphaPolicy::Remove => false,
                    }
            }
            Property::Mips => levels.is_none_or(|n| n > 1),
            Property::Subtype(s) => p.subtype == *s,
        }
    }
}

impl From<Property> for String {
    fn from(p: Property) -> String {
        match p {
            Property::Format(f) => f.id(),
            Property::Alpha => "alpha".into(),
            Property::Cutout => "cutout".into(),
            Property::Mips => "mips".into(),
            Property::Subtype(s) => s.id(),
        }
    }
}

impl TryFrom<String> for Property {
    type Error = String;
    fn try_from(w: String) -> Result<Self, String> {
        Property::parse(&w).ok_or_else(|| unknown(&w))
    }
}

fn unknown(w: &str) -> String {
    format!(
        "unknown property '{}'; the words are {}",
        w.trim(),
        Property::words().join(", ")
    )
}

/// One `--property` value: words that must all hold. Sorted and deduplicated, never empty, at most one format and
/// one texture type (two can never both hold).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "Vec<Property>")]
pub struct PropertyGroup(Vec<Property>);

impl PropertyGroup {
    /// The one constructor: rejects an empty group and two format or two texture-type words, then sorts and
    /// deduplicates.
    pub fn new(mut words: Vec<Property>) -> Result<Self, String> {
        if words.is_empty() {
            return Err("an empty --property list".into());
        }
        words.sort();
        words.dedup();
        let formats: Vec<_> = words
            .iter()
            .filter(|w| matches!(w, Property::Format(_)))
            .collect();
        let types: Vec<_> = words
            .iter()
            .filter(|w| matches!(w, Property::Subtype(_)))
            .collect();
        for two in [formats, types] {
            if let [a, b, ..] = two[..] {
                return Err(format!(
                    "{} and {} cannot both hold: {NEVER_HOLDS}",
                    String::from(*a),
                    String::from(*b)
                ));
            }
        }
        Ok(PropertyGroup(words))
    }

    /// Every word holds for the output.
    fn holds(&self, p: &Profile, levels: Option<u32>) -> bool {
        self.0.iter().all(|w| w.holds(p, levels))
    }
}

impl TryFrom<Vec<Property>> for PropertyGroup {
    type Error = String;
    fn try_from(words: Vec<Property>) -> Result<Self, String> {
        PropertyGroup::new(words)
    }
}

impl FromStr for PropertyGroup {
    type Err = String;
    /// A comma list of words (`"cutout, mips"`); an empty or unknown word is an error naming it.
    fn from_str(s: &str) -> Result<Self, String> {
        let words = s
            .split(',')
            .map(|w| {
                if w.trim().is_empty() {
                    Err(format!(
                        "an empty word in --property '{s}'; the words are {}",
                        Property::words().join(", ")
                    ))
                } else {
                    Property::parse(w).ok_or_else(|| unknown(w))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        PropertyGroup::new(words)
    }
}

/// Which outputs written by an older encode version are re-encoded under `--on-existing if-changed`: none (`Off`,
/// the default: they are kept and counted), every one (`--ev`), or those any group covers (`--ev --property`).
/// Canonical (groups sorted and deduplicated), because `BatchRequest::sig_without_files` hashes its `Debug` form.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", try_from = "EvScopeRepr")]
pub enum EvScope {
    #[default]
    Off,
    All,
    Matching(Vec<PropertyGroup>),
}

/// `EvScope` as read, before `EvScope::matching` checks it.
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum EvScopeRepr {
    Off,
    All,
    Matching(Vec<PropertyGroup>),
}

impl TryFrom<EvScopeRepr> for EvScope {
    type Error = String;
    fn try_from(r: EvScopeRepr) -> Result<Self, String> {
        match r {
            EvScopeRepr::Off => Ok(EvScope::Off),
            EvScopeRepr::All => Ok(EvScope::All),
            EvScopeRepr::Matching(g) => EvScope::matching(g),
        }
    }
}

impl EvScope {
    /// The only way to build `Matching`: rejects an empty list, then sorts and deduplicates the groups.
    pub fn matching(mut groups: Vec<PropertyGroup>) -> Result<Self, String> {
        if groups.is_empty() {
            return Err("--property needs at least one word".into());
        }
        groups.sort();
        groups.dedup();
        Ok(EvScope::Matching(groups))
    }

    /// Whether an older-encode-version output with profile `p` and recorded mip count `levels` is re-encoded.
    pub fn covers(&self, p: &Profile, levels: Option<u32>) -> bool {
        match self {
            EvScope::Off => false,
            EvScope::All => true,
            EvScope::Matching(groups) => groups.iter().any(|g| g.holds(p, levels)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::BatchRequest;
    use crate::config::{MipKeyword, MipLevels, Target};
    use crate::presets::PresetRegistry;
    use std::collections::BTreeSet;

    fn group(s: &str) -> PropertyGroup {
        s.parse().unwrap()
    }

    fn profile(format: Format, policy: AlphaPolicy) -> Profile {
        let mut p = PresetRegistry::builtin()
            .unwrap()
            .base_profile(Target::Cs, Subtype::Diffuse);
        p.format = format;
        p.alpha.policy = policy;
        p
    }

    /// `FORMATS` is every `Format`, in schema order (a new format fails here until it is listed).
    #[test]
    fn formats_are_every_format() {
        let schema = serde_json::to_value(schemars::schema_for!(Format)).unwrap();
        let ids: Vec<_> = FORMATS.iter().map(|f| serde_json::json!(f.id())).collect();
        assert_eq!(schema["enum"], serde_json::Value::Array(ids));
    }

    #[test]
    fn no_property_word_is_ambiguous() {
        let words = Property::words();
        let distinct: BTreeSet<_> = words.iter().collect();
        assert_eq!(distinct.len(), words.len(), "{words:?}");
        for p in Property::all() {
            let w = String::from(p);
            assert_eq!(Property::parse(&w), Some(p), "{w}");
            assert_eq!(group(&w), PropertyGroup(vec![p]), "{w}");
        }
        for w in [
            "alpha",
            "cutout",
            "mips",
            "bc1",
            "bc6h_uf16",
            "diffuse",
            "skin_rfaos",
        ] {
            assert!(words.iter().any(|x| x == w), "{w}");
        }
    }

    #[test]
    fn alpha_needs_a_format_that_stores_it() {
        let holds = |p: &Profile, w: &str| Property::parse(w).unwrap().holds(p, Some(5));
        assert!(!holds(&profile(Format::Bc5, AlphaPolicy::Blend), "alpha"));
        assert!(holds(&profile(Format::Bc7, AlphaPolicy::Data), "alpha"));
        assert!(!holds(&profile(Format::Bc7, AlphaPolicy::Data), "cutout"));
        assert!(!holds(&profile(Format::Bc7, AlphaPolicy::Remove), "alpha"));
        let mut bc1 = profile(Format::Bc1, AlphaPolicy::Blend);
        assert!(!holds(&bc1, "alpha"));
        assert!(!holds(&bc1, "cutout"));
        bc1.encoder.bc1_alpha = true;
        assert!(holds(&bc1, "alpha"));
        assert!(holds(&bc1, "cutout"));
        for policy in [AlphaPolicy::Cutout, AlphaPolicy::CutoutBlend] {
            assert!(holds(&profile(Format::Bc3, policy), "cutout"), "{policy:?}");
        }
        assert!(!holds(&profile(Format::Bc3, AlphaPolicy::Blend), "cutout"));
    }

    /// `mips` reads the record's level count, never the profile's `mips.levels`.
    #[test]
    fn mips_reads_the_recorded_levels() {
        let mut p = profile(Format::Bc7, AlphaPolicy::Remove);
        p.mips.levels = MipLevels::Keyword(MipKeyword::Full);
        let mips = Property::Mips;
        assert!(!mips.holds(&p, Some(1)), "a 1x1 output with a full chain");
        assert!(mips.holds(&p, Some(2)));
        assert!(mips.holds(&p, None), "a record without levels is in scope");
        p.mips.levels = MipLevels::Keyword(MipKeyword::None);
        assert!(mips.holds(&p, Some(7)));
    }

    #[test]
    fn covers() {
        let bc7_cut = profile(Format::Bc7, AlphaPolicy::Cutout);
        let bc1 = profile(Format::Bc1, AlphaPolicy::Remove);
        assert!(!EvScope::Off.covers(&bc7_cut, Some(7)));
        assert!(EvScope::All.covers(&bc1, Some(1)));
        let and = EvScope::matching(vec![group("cutout,mips")]).unwrap();
        assert!(and.covers(&bc7_cut, Some(7)));
        assert!(!and.covers(&bc7_cut, Some(1)), "every word must hold");
        assert!(!and.covers(&bc1, Some(7)));
        let or = EvScope::matching(vec![group("bc1"), group("cutout,mips")]).unwrap();
        assert!(or.covers(&bc1, Some(1)));
        assert!(or.covers(&bc7_cut, Some(7)));
        assert!(!or.covers(&bc7_cut, Some(1)));
    }

    #[test]
    fn property_group_from_str() {
        assert_eq!(group(" Cutout , MIPS "), group("cutout,mips"));
        assert_eq!(
            group("BC1"),
            PropertyGroup(vec![Property::Format(Format::Bc1)])
        );
        for bad in ["bc1,,mips", "", "nope", "bc1,bc3", "diffuse,normal"] {
            assert!(bad.parse::<PropertyGroup>().is_err(), "{bad}");
        }
        assert!("nope"
            .parse::<PropertyGroup>()
            .unwrap_err()
            .contains("'nope'"));
        for never in ["bc1,bc3", "diffuse,normal"] {
            let e = never.parse::<PropertyGroup>().unwrap_err();
            assert!(e.contains(NEVER_HOLDS), "{e}");
        }
        assert_eq!(
            NEVER_HOLDS,
            "a comma list must all hold; repeat --property for alternatives"
        );
    }

    #[test]
    fn scopes_are_canonical() {
        let req = |ev: EvScope| {
            let mut r = BatchRequest::new(vec![], "out".into(), Target::Cs);
            r.ev = ev;
            r.sig_without_files()
        };
        for (a, b) in [
            (
                vec![group("bc1"), group("bc3")],
                vec![group("bc3"), group("bc1")],
            ),
            (vec![group("cutout,mips")], vec![group("mips,cutout")]),
            (vec![group("bc1"), group("bc1")], vec![group("bc1")]),
        ] {
            let (a, b) = (EvScope::matching(a).unwrap(), EvScope::matching(b).unwrap());
            assert_eq!(a, b);
            assert_eq!(format!("{a:?}"), format!("{b:?}"));
            assert_eq!(req(a), req(b));
        }
        assert_ne!(req(EvScope::Off), req(EvScope::All));
    }

    #[test]
    fn empty_scopes_do_not_deserialise() {
        for bad in [
            r#"{"matching": []}"#,
            r#"{"matching": [[]]}"#,
            r#"{"matching": [["nope"]]}"#,
        ] {
            assert!(serde_json::from_str::<EvScope>(bad).is_err(), "{bad}");
        }
        let ok: EvScope =
            serde_json::from_str(r#"{"matching": [["mips", "cutout"], ["bc1"]]}"#).unwrap();
        assert_eq!(
            ok,
            EvScope::matching(vec![group("bc1"), group("cutout,mips")]).unwrap()
        );
        assert_eq!(
            serde_json::from_str::<EvScope>(r#""all""#).unwrap(),
            EvScope::All
        );
        assert_eq!(
            serde_json::to_value(&ok).unwrap(),
            serde_json::json!({"matching": [["bc1"], ["cutout", "mips"]]})
        );
    }
}
