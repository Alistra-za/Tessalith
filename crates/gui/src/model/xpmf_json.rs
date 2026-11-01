//! XPMF profiles (Note GI, design X2-X9): `SKSE/Plugins/XPMF/tessalith_<set>.json`, one per set with a `[sets.xpmf]`
//! block, written after a clean export with the CS PBR export's guard. No egui types.
use super::history::{orphans, record_key, recorded, records_at, remove_own_case_twin, sha256_of};
use super::pbr_export::file_name_problem;
use super::sets::{is_pbr_landscape, views, SetView, SlotFill, SlotKind};
use super::AppModel;
use crate::sets_doc::SetEntry;
use engine::config::Target;
use engine::sets::same_id;
use engine::shader::Shader;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The vendored schema's `$id`, written as `$schema` (XPMF ignores it; editors use it).
pub const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/hakasapl/xpmf/main/schema/profile.schema.json";

/// The profiles' folder under the export root.
const FOLDER: &str = "SKSE/Plugins/XPMF";

/// The type of a profile key, for the card (Task 7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldKind {
    Bool,
    Number {
        min: f64,
        max: Option<f64>,
        exclusive_min: bool,
    },
    Text,
    /// A list of EditorID wildcard patterns.
    Patterns,
    /// A Data-relative texture path.
    Texture,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Field {
    pub key: &'static str,
    pub kind: FieldKind,
    /// The MATO preview shows it (X10); the card's other rows say "Not shown in the preview".
    pub preview: bool,
    /// What XPMF does while the key is unset, as display text (research §1); `None` where it has no default.
    pub default: Option<&'static str>,
}

const fn f(
    key: &'static str,
    kind: FieldKind,
    preview: bool,
    default: Option<&'static str>,
) -> Field {
    Field {
        key,
        kind,
        preview,
        default,
    }
}

const fn num(min: f64, max: Option<f64>) -> FieldKind {
    FieldKind::Number {
        min,
        max,
        exclusive_min: false,
    }
}

use FieldKind::{Bool, Patterns, Text, Texture};

/// Every schema key but `$schema`, in schema order. `falloffScale` and `falloffBias` have no schema bounds: `min` is
/// `-inf`. `name` and `editorIds` are required (no XPMF default; Tessalith fills the set id and the cs_mato EDIDs).
pub const FIELDS: &[Field] = &[
    f("comment", Text, false, None),
    f("name", Text, false, None),
    f("editorIds", Patterns, false, None),
    f("excludeEditorIds", Patterns, false, None),
    f("pbr", Bool, false, Some("false")),
    f("diffuseTexture", Texture, true, Some("the game's")),
    f("normalTexture", Texture, true, Some("the game's")),
    f("noiseTexture", Texture, true, Some("the game's")),
    f("detailNormalTexture", Texture, true, Some("the game's")),
    f(
        "falloffScale",
        num(f64::NEG_INFINITY, None),
        true,
        Some("the record's"),
    ),
    f(
        "falloffBias",
        num(f64::NEG_INFINITY, None),
        true,
        Some("the record's"),
    ),
    f(
        "noiseUVScale",
        FieldKind::Number {
            min: 0.0,
            max: None,
            exclusive_min: true,
        },
        true,
        Some("the record's"),
    ),
    f(
        "maxAngle",
        num(0.0, Some(180.0)),
        false,
        Some("each static's"),
    ),
    f("lodIdentifier", Text, false, Some("obj<name>")),
    f("lodMaterial", Text, false, None),
    f("lodMaterialHD", Text, false, Some("= lodMaterial")),
    f("neutralizeVertexColors", Bool, false, Some("true")),
    f("neutralizeVertexColorsSkip", Patterns, false, None),
    f("neutralizeVertexAlpha", Bool, false, Some("true")),
    f("neutralizeVertexAlphaSkip", Patterns, false, None),
    f("roofShelter", Bool, false, Some("true")),
    f("roofShelterSkip", Patterns, false, None),
    f("roofShelterFixVertices", Bool, false, Some("true")),
    f("shelterFade", num(0.0, Some(128.0)), false, Some("64")),
    f(
        "shelterRandomness",
        num(0.0, Some(128.0)),
        false,
        Some("32"),
    ),
    f("waterFade", Bool, false, Some("false")),
    f(
        "waterFadeRandomness",
        num(0.0, Some(64.0)),
        false,
        Some("16"),
    ),
    // Hidden while pbr is on (X9).
    f(
        "specularMult",
        num(0.0, Some(10.0)),
        false,
        Some("as the mesh has it"),
    ),
];

/// The texture keys and the slots that fill them by default (X5, X12).
const TEXTURES: [(&str, SlotKind); 4] = [
    ("diffuseTexture", SlotKind::Diffuse),
    ("normalTexture", SlotKind::Normal),
    ("noiseTexture", SlotKind::ProjectedNoise),
    ("detailNormalTexture", SlotKind::DetailNormal),
];

/// `tessalith_<fold>.json` (X2): the id's last `/` component, lower-cased, every char outside `[a-z0-9_-]` as `_`.
pub fn file_name(set_id: &str) -> Result<String, String> {
    let leaf = set_id.rsplit('/').next().unwrap_or_default().to_lowercase();
    if leaf.is_empty() {
        return Err(format!("set id {set_id:?} gives an empty file name"));
    }
    let fold: String = leaf
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '_' | '-' => c,
            _ => '_',
        })
        .collect();
    // As the brief orders: the whole name, then `file_name_problem` (its device check reads the part before the
    // first dot, so the `.json` changes nothing).
    let name = format!("tessalith_{fold}.json");
    match file_name_problem("XPMF file name", &name) {
        Some(why) => Err(why),
        None => Ok(name),
    }
}

/// The Data-relative path of an output (`textures/...`, `/`-separated), if it lies under the export root.
fn data_rel(root: &std::path::Path, output: &std::path::Path) -> Option<String> {
    Some(
        output
            .strip_prefix(root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/"),
    )
}

/// An empty value means XPMF's default (X7).
fn empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        _ => false,
    }
}

/// The set's profile (X2-X9): the block's values over the defaults from the set, null and empty values dropped,
/// `specularMult` dropped while `pbr` is on. `Err` is the export note.
pub fn profile(m: &AppModel, set: &SetEntry) -> Result<Map<String, Value>, String> {
    profile_in(m, &views(m), set)
}

/// `profile` with the set views computed once by the caller (`prepare`).
fn profile_in(m: &AppModel, vs: &[SetView], set: &SetEntry) -> Result<Map<String, Value>, String> {
    let Some(v) = vs
        .iter()
        .find(|v| same_id(&v.id, &set.id) && !v.members.is_empty())
    else {
        return Err(format!("{}: no files; XPMF profile not written", set.id));
    };
    let landscape = v
        .shader
        .as_ref()
        .is_some_and(|p| matches!(p.shader, Shader::Landscape | Shader::PbrLandscape));
    if !landscape {
        return Err(format!(
            "{}: not a landscape set; XPMF profile not written",
            set.id
        ));
    }
    let mut doc = Map::new();
    doc.insert("name".into(), json!(set.id));
    let edids = set
        .cs_mato
        .as_ref()
        .map(|b| b.edids.clone())
        .unwrap_or_default();
    doc.insert("editorIds".into(), json!(edids));
    doc.insert(
        "pbr".into(),
        json!(is_pbr_landscape(v) && m.target == Target::Cs),
    );
    let root = m.output_root();
    for (key, slot) in TEXTURES {
        if let Some(rel) = slot_output(m, v.slots.get(&slot)).and_then(|o| data_rel(&root, &o)) {
            doc.insert(key.into(), json!(rel));
        }
    }
    if let Some(b) = &set.xpmf {
        // R13: an empty `name` or `editorIds` is unset, so the default above stays. Other empty keys drop the
        // default below (`noiseTexture = ""` is the game's own).
        let unset = |k: &str, v: &Value| matches!(k, "name" | "editorIds") && empty(v);
        doc.extend(
            b.iter()
                .filter(|(k, v)| !unset(k, v))
                .map(|(k, v)| (k.clone(), v.clone())),
        );
    }
    doc.retain(|_, v| !empty(v));
    if doc.get("pbr") == Some(&json!(true)) {
        doc.remove("specularMult");
    }
    if !doc.contains_key("editorIds") {
        return Err(format!(
            "{}: no editorIds; XPMF profile not written",
            set.id
        ));
    }
    Ok(doc)
}

fn slot_output(m: &AppModel, fill: Option<&SlotFill>) -> Option<PathBuf> {
    let item = match fill? {
        SlotFill::Own { item } | SlotFill::Borrowed { item, .. } => item,
        SlotFill::Empty => return None,
    };
    Some(m.item(item)?.job.as_ref()?.output.clone())
}

/// The file's text: `$schema`, then the keys sorted (serde_json's map is sorted and `$` sorts first), 2-space pretty
/// print, a final newline (cm_json's style).
pub fn to_text(doc: &Map<String, Value>) -> String {
    let mut obj = doc.clone();
    obj.insert("$schema".into(), json!(SCHEMA_URL));
    let mut s = serde_json::to_string_pretty(&Value::Object(obj)).expect("JSON values serialise");
    s.push('\n');
    s
}

/// What an export about to start writes once it has ended (computed when it starts, like the CS PBR files).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingXpmf {
    pub root: PathBuf,
    /// (path under the root, `/`-separated; text).
    pub files: Vec<(String, String)>,
    /// Refused profiles, for the export report.
    pub notes: Vec<String>,
    /// Paths of refused files (already noted): not orphans, and their record stays.
    pub refused: Vec<String>,
    /// Controller ruling R2: file (as in `files`) → the Data-relative textures taken from this project's own outputs.
    /// `finish` writes the file only when each is on disk.
    pub own_textures: BTreeMap<String, Vec<String>>,
}

/// XPMF accepts any slashes and case (schema `texture`), so a typed path is compared to the outputs folded.
/// shortcut: a typed path without its `textures/` or `.dds` is not matched to an output (so not checked); fold those
/// too if users type them.
fn fold_path(p: &str) -> String {
    p.replace('\\', "/").to_lowercase()
}

/// Design X2-X9: one profile per set with a block, validated against the vendored schema. `None` when no set has a
/// block. Two sets whose ids fold to one file name are both refused.
pub fn prepare(m: &AppModel) -> Option<PendingXpmf> {
    let entries = m.sets_doc.as_ref()?.entries();
    // The first entry with an id decides, as in cm_json.
    let blocks: Vec<&SetEntry> = entries
        .iter()
        .enumerate()
        .filter(|(i, e)| e.xpmf.is_some() && !entries[..*i].iter().any(|o| same_id(&o.id, &e.id)))
        .map(|(_, e)| e)
        .collect();
    if blocks.is_empty() {
        return None;
    }
    let mut p = PendingXpmf {
        root: m.output_root(),
        ..Default::default()
    };
    let vs = views(m);
    let names: Vec<Result<String, String>> = blocks.iter().map(|e| file_name(&e.id)).collect();
    let outputs: BTreeMap<String, String> = m
        .items
        .iter()
        .filter_map(|i| data_rel(&p.root, &i.job.as_ref()?.output))
        .map(|r| (fold_path(&r), r))
        .collect();
    for (e, name) in blocks.iter().zip(&names) {
        let name = match name {
            Ok(n) => n,
            Err(why) => {
                p.notes
                    .push(format!("{}: XPMF profile not written: {why}", e.id));
                continue;
            }
        };
        let rel = format!("{FOLDER}/{name}");
        let others: Vec<&str> = blocks
            .iter()
            .zip(&names)
            .filter(|(o, n)| o.id != e.id && n.as_ref() == Ok(name))
            .map(|(o, _)| o.id.as_str())
            .collect();
        let refused = if !others.is_empty() {
            Err(format!(
                "{}: XPMF profile not written: {name} is also the file of {}",
                e.id,
                others.join(" and ")
            ))
        } else {
            profile_in(m, &vs, e).and_then(|doc| {
                let text = to_text(&doc);
                let parsed: Value = serde_json::from_str(&text).expect("to_text writes JSON");
                crate::pgp::validate::validate_xpmf(&parsed)
                    .map(|()| (doc, text))
                    .map_err(|errs| {
                        format!("{}: XPMF profile not written: {}", e.id, errs.join("; "))
                    })
            })
        };
        match refused {
            Ok((doc, text)) => {
                let own: Vec<String> = TEXTURES
                    .iter()
                    .filter_map(|(k, _)| doc.get(*k)?.as_str())
                    .filter_map(|t| outputs.get(&fold_path(t)).cloned())
                    .collect();
                if !own.is_empty() {
                    p.own_textures.insert(rel.clone(), own);
                }
                p.files.push((rel, text));
            }
            Err(note) => {
                if !p.refused.contains(&rel) {
                    p.refused.push(rel);
                }
                p.notes.push(note);
            }
        }
    }
    Some(p)
}

/// After a clean export: write each profile whose own textures are on disk, unless it was edited outside Tessalith,
/// then report the recorded profiles this run did not write (never deleted). `shas` is the record (path → hash of
/// what was last written there); returns the new record.
pub fn finish(
    p: &PendingXpmf,
    shas: &BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> BTreeMap<String, String> {
    notes.extend(p.notes.iter().cloned());
    let mut out = records_at(shas, &p.root);
    let mut written = 0;
    for (rel, text) in &p.files {
        let path = p.root.join(rel);
        // cm_json's rule (ruling R2): a profile naming an output of this project that is not on disk is not written.
        let missing = p
            .own_textures
            .get(rel)
            .into_iter()
            .flatten()
            .find(|t| !p.root.join(t).is_file());
        if let Some(t) = missing {
            notes.push(format!(
                "{}: {t} was not exported; JSON not written",
                path.display()
            ));
            continue;
        }
        let key = record_key(&path);
        let last = out.get(&key).cloned();
        let last = last.as_deref();
        remove_own_case_twin(&path, last);
        if let Err(k) = engine::dds::may_replace(&path, text.as_bytes(), recorded(last)) {
            notes.push(k.note(&path));
            continue;
        }
        match engine::dds::write_atomic(&path, text.as_bytes()) {
            Ok(()) => {
                written += 1;
                if let Some(s) = sha256_of(&path) {
                    out.insert(key, s);
                }
            }
            Err(e) => notes.push(format!("{} not written: {e}", path.display())),
        }
    }
    if written > 0 {
        notes.push(format!(
            "XPMF profiles written: {written} file{} under {}",
            if written == 1 { "" } else { "s" },
            p.root.display()
        ));
    }
    let this_run = p.files.iter().map(|(r, _)| r).chain(&p.refused);
    for path in orphans(&out, &p.root, this_run) {
        notes.push(format!(
            "{}: no set writes this XPMF profile any more; the file was left in place",
            path.display()
        ));
    }
    out
}
