//! Task 10b: the Advanced skin card's fields (Community Shaders `SkinProfile`, CS README section 4). Per set,
//! non-inheritable: a field is either unset (nothing stored, the CS default shows as a hint) or an explicit value in
//! the set's `skin` table of `sdc-project.toml`, under the exact CS key. Export writes it as the `.csskin.json` beside the diffuse.
use super::AppModel;
use crate::pgp::settings::Settings;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// A float with the UI-only clamp `lo..=hi` (AG83).
    Num {
        default: f64,
        lo: f64,
        hi: f64,
    },
    Flag {
        default: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct SkinField {
    pub key: &'static str,
    pub group: &'static str,
    pub kind: Kind,
}

const fn num(key: &'static str, group: &'static str, default: f64, lo: f64, hi: f64) -> SkinField {
    SkinField {
        key,
        group,
        kind: Kind::Num { default, lo, hi },
    }
}
const fn flag(key: &'static str, group: &'static str, default: bool) -> SkinField {
    SkinField {
        key,
        group,
        kind: Kind::Flag { default },
    }
}

pub const GROUPS: [&str; 6] = [
    "Specular",
    "Physical",
    "Edge & detail",
    "Transmission",
    "Fuzz",
    "Wet",
];

/// The 22 per-material parameters of a Community Shaders `.csskin.json` (spec table order, CS ranges).
pub const FIELDS: [SkinField; 22] = [
    num("Roughness", "Specular", 0.7, 0.0, 1.0),
    num("SecondaryRoughness", "Specular", 0.35, 0.0, 1.0),
    num("SpecularTextureMultiplier", "Specular", 1.0, 0.0, 10.0),
    num("SecondarySpecularStrength", "Specular", 0.15, 0.0, 1.0),
    num("Reflectance", "Specular", 0.0278, 0.0, 0.1),
    num("BaseColorMultiplier", "Physical", 1.0, 0.0, 2.0),
    num("PhysicalMainRoughnessMultiplier", "Physical", 1.3, 0.0, 2.0),
    num(
        "PhysicalSecondRoughnessMultiplier",
        "Physical",
        0.75,
        0.0,
        2.0,
    ),
    num("PhysicalSpecularStrength", "Physical", 1.0, 0.0, 2.0),
    num("ExtraEdgeRoughness", "Edge & detail", 0.25, 0.0, 1.0),
    num("Fuzz", "Fuzz", 1.0, 0.0, 2.0),
    num("FuzzRoughness", "Fuzz", 0.35, 0.1, 1.0),
    num("FuzzF0", "Fuzz", 0.045, 0.0, 0.5),
    flag("TransmissionEnabled", "Transmission", true),
    num("Transmission", "Transmission", 0.1, 0.0, 1.0),
    num("TransmissionDepth", "Transmission", 0.2, 0.0, 1.0),
    num("SSSAmount", "Transmission", 1.0, 0.0, 1.0),
    flag("DetailEnabled", "Edge & detail", true),
    num("DetailStrength", "Edge & detail", 0.25, -2.0, 2.0),
    num("DetailTiling", "Edge & detail", 10.0, 0.1, 100.0),
    num("BodyTilingMultiplier", "Edge & detail", 2.0, 0.1, 5.0),
    num("WetResponse", "Wet", 1.0, 0.0, 1.0),
];

pub fn skin_field(key: &str) -> Option<&'static SkinField> {
    FIELDS.iter().find(|f| f.key == key)
}

/// Known keys of `values` that CS would reject (a hand-edited `sdc-project.toml`; the card cannot make them): (key,
/// why). Unknown keys are ignored here.
pub fn bad_skin_values(values: &Settings) -> Vec<(String, String)> {
    let mut out = vec![];
    for (k, v) in values.iter() {
        let why = match skin_field(k).map(|f| f.kind) {
            Some(Kind::Num { lo, hi, .. }) => match v.as_f64() {
                None => Some("not a number".to_string()),
                Some(x) if !(lo..=hi).contains(&x) => Some(format!("{x}, outside {lo} to {hi}")),
                Some(_) => None,
            },
            Some(Kind::Flag { .. }) => (!v.is_boolean()).then(|| "not true or false".to_string()),
            None => None,
        };
        if let Some(w) = why {
            out.push((k.clone(), w));
        }
    }
    out
}

impl AppModel {
    /// Set `set`'s explicit skin values: only keys `skin_field` knows (old key names are ignored).
    pub fn skin_values(&self, set: &str) -> Settings {
        let mut s = self.skin_table(set);
        s.retain(|k, _| skin_field(k).is_some());
        s
    }

    fn skin_table(&self, set: &str) -> Settings {
        self.sets_doc
            .as_ref()
            .and_then(|d| {
                d.entries()
                    .iter()
                    .find(|e| engine::sets::same_id(&e.id, set))
                    .map(|e| e.skin.clone())
            })
            .unwrap_or_default()
    }

    /// Set (or with `None` clear) skin field `key` of set `set`. One undo step; a drag on the same field coalesces.
    /// No plan: nothing in the skin table reaches the engine.
    pub fn set_skin_value(&mut self, set: &str, key: &str, v: Option<Value>) {
        if !self.sets_editable() {
            return;
        }
        // AG89: a drag's f64 is rounded to the card's 4 shown decimals, so 0.7100000000000001 is stored as 0.71.
        let v = v.map(|x| match x.as_f64() {
            Some(f) if x.is_f64() => Value::from((f * 1e4).round() / 1e4),
            _ => x,
        });
        let (s, path) = (set.to_string(), format!("skin.{key}"));
        let merge = Some(format!("{set}:{path}"));
        self.edit_docs(&format!("Change {path}"), merge, false, move |_, d| {
            // Old key names go in the same step: nothing reads them and the project file is ours.
            d.retain_skin(&s, |k| skin_field(k).is_some());
            d.set_value(&s, &path, v)
        });
    }
}
