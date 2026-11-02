//! XPMF profiles (Note GI, design X2-X9): `SKSE/Plugins/XPMF/tessalith_<set>.json`, one per set with a `[sets.xpmf]`
//! block, written after a clean export with the CS PBR export's guard. No egui types.
use super::history::{orphans, record_key, recorded, records_at, remove_own_case_twin, sha256_of};
use super::pbr_export::file_name_problem;
use super::sets::{is_pbr_landscape, views, SetView, SlotFill, SlotKind};
use super::AppModel;
use crate::sets_doc::SetEntry;
use engine::config::Target;
use engine::derive::vanilla_rel;
use engine::policy::ResolvedJob;
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
pub(super) fn data_rel(root: &std::path::Path, output: &std::path::Path) -> Option<String> {
    Some(
        output
            .strip_prefix(root)
            .ok()?
            .to_string_lossy()
            .replace('\\', "/"),
    )
}

/// The sets that can have a profile: Landscape and PBR Landscape, on either target.
pub fn is_landscape(v: &SetView) -> bool {
    v.shader
        .as_ref()
        .is_some_and(|p| matches!(p.shader, Shader::Landscape | Shader::PbrLandscape))
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
    profile_in(m, &views(m), set, false)
}

/// `profile` with the set views computed once by the caller (`prepare`). `vanilla`: the copy for the Also export's
/// Vanilla folder (Note GI27): `pbr` is false whatever the block says, so `specularMult` stays, and the default
/// textures are the Vanilla folder's files.
fn profile_in(
    m: &AppModel,
    vs: &[SetView],
    set: &SetEntry,
    vanilla: bool,
) -> Result<Map<String, Value>, String> {
    let mut doc = defaults_in(m, vs, set, vanilla)?;
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
    if vanilla {
        doc.insert("pbr".into(), json!(false));
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

/// The values Tessalith fills while the block leaves a key unset (X2-X5): `name`, `editorIds` (maybe empty), `pbr`
/// and the textures of filled slots. `profile` lays the block over these; the card shows them greyed.
fn defaults_in(
    m: &AppModel,
    vs: &[SetView],
    set: &SetEntry,
    vanilla: bool,
) -> Result<Map<String, Value>, String> {
    let Some(v) = vs
        .iter()
        .find(|v| same_id(&v.id, &set.id) && !v.members.is_empty())
    else {
        return Err(format!("{}: no files; XPMF profile not written", set.id));
    };
    if !is_landscape(v) {
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
        json!(is_pbr_landscape(v) && m.target == Target::Cs && !vanilla),
    );
    let root = m.output_root();
    for (key, slot) in TEXTURES {
        if let Some(rel) =
            slot_job(m, v.slots.get(&slot)).and_then(|j| texture_rel(&root, j, vanilla))
        {
            doc.insert(key.into(), json!(rel));
        }
    }
    Ok(doc)
}

/// What the card shows for one set: the defaults (greyed) and the profile (its `Err` is the card's amber line).
#[derive(Debug, Clone, PartialEq)]
pub struct CardValues {
    pub defaults: Result<Map<String, Value>, String>,
    pub profile: Result<Map<String, Value>, String>,
    /// M1: why a built profile would still not be written (fold clash, schema); the card's amber line after `profile`'s.
    pub refusal: Option<String>,
}

/// `CardValues` of set `set` (an id), cached with the set views (ui-rules §9.1): the card asks every frame.
pub fn card_values(m: &AppModel, set: &str) -> std::sync::Arc<CardValues> {
    let key = m.view_key();
    {
        let c = m.views.borrow();
        if c.key.as_ref() == Some(&key) {
            if let Some((id, v)) = &c.xpmf_card {
                if id == set {
                    return v.clone();
                }
            }
        }
    }
    // No borrow is held here: `views` fills the same cell.
    let vs = views(m);
    let none = || Err(format!("{set}: no such set"));
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    let v = std::sync::Arc::new(match entries.iter().find(|e| same_id(&e.id, set)) {
        Some(e) => {
            let profile = profile_in(m, &vs, e, false);
            CardValues {
                defaults: defaults_in(m, &vs, e, false),
                refusal: profile
                    .as_ref()
                    .ok()
                    .and_then(|doc| refusal(&vs, &entries, e, doc).err()),
                profile,
            }
        }
        None => CardValues {
            defaults: none(),
            profile: none(),
            refusal: None,
        },
    });
    let mut c = m.views.borrow_mut();
    if c.key.as_ref() != Some(&key) {
        *c = super::ViewCache {
            key: Some(key),
            ..Default::default()
        };
    }
    c.xpmf_card = Some((set.to_string(), v.clone()));
    v
}

/// Note GI10 (X10): the profile of the set with id `set` for the MATO preview, while the set has an `[sets.xpmf]`
/// block whose profile can be built (cached with the card's values).
pub fn preview_profile(m: &AppModel, set: &str) -> Option<Map<String, Value>> {
    let has_block = m
        .sets_doc
        .as_ref()?
        .entries()
        .iter()
        .any(|e| same_id(&e.id, set) && e.xpmf.is_some());
    if !has_block {
        return None;
    }
    card_values(m, set).profile.clone().ok()
}

/// The X4 Problems row's hover.
pub const SKIP_HOVER: &str = "Only this project's cs_mato blocks are checked; PBRMaterialObjects files from other mods are not scanned.";

/// X4: per set with a block whose profile has `pbr` on, each exact EDID (no `*` or `?`) of its `editorIds` that no
/// `cs_mato` block of the project names, case-insensitively as `refusals` compares them: (set key, set id, EDID).
/// Read by `bottom::build_problems`, which is cached.
pub fn skipped_edids(m: &AppModel) -> Vec<(String, String, String)> {
    let Some(doc) = m.sets_doc.as_ref() else {
        return vec![];
    };
    let entries = doc.entries();
    if entries.iter().all(|e| e.xpmf.is_none()) {
        return vec![];
    }
    let mato: std::collections::HashSet<String> = entries
        .iter()
        .filter_map(|e| e.cs_mato.as_ref())
        .flat_map(|b| b.edids.iter().map(|x| x.to_lowercase()))
        .collect();
    let vs = views(m);
    let mut out = vec![];
    for (i, e) in entries.iter().enumerate() {
        if e.xpmf.is_none() || entries[..i].iter().any(|o| same_id(&o.id, &e.id)) {
            continue;
        }
        let (Ok(p), Some(v)) = (
            profile_in(m, &vs, e, false),
            vs.iter().find(|v| same_id(&v.id, &e.id)),
        ) else {
            continue;
        };
        if p.get("pbr") != Some(&json!(true)) {
            continue;
        }
        let ids = p.get("editorIds").and_then(Value::as_array);
        for x in ids.into_iter().flatten().filter_map(Value::as_str) {
            // One case-fold rule (Unicode, as the cs_mato set above) for the match and the dedupe.
            let low = x.to_lowercase();
            let new = !out.iter().any(|(_, id, e): &(String, String, String)| {
                *id == v.id && e.to_lowercase() == low
            });
            if new && !x.contains(['*', '?']) && !mato.contains(&low) {
                out.push((v.key.clone(), v.id.clone(), x.to_string()));
            }
        }
    }
    out
}

fn slot_job<'a>(m: &'a AppModel, fill: Option<&SlotFill>) -> Option<&'a ResolvedJob> {
    let item = match fill? {
        SlotFill::Own { item } | SlotFill::Borrowed { item, .. } => item,
        SlotFill::Empty => return None,
    };
    m.item(item)?.job.as_ref()
}

/// The Data-relative path of a job's output under `root`; with `vanilla`, the file the Vanilla folder gets for it
/// (TA12's `vanilla_rel`, as the Also export's Vanilla phase paths its derived and kept files).
fn texture_rel(root: &std::path::Path, job: &ResolvedJob, vanilla: bool) -> Option<String> {
    let rel = data_rel(root, &job.output)?;
    Some(match vanilla {
        true => vanilla_rel(std::path::Path::new(&rel), &job.game_path)
            .to_string_lossy()
            .replace('\\', "/"),
        false => rel,
    })
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
    /// Note GI27: with Vanilla ticked in an Also export, the `pbr = false` copies for the Vanilla folder (its `root`,
    /// the same relative paths), written, guarded and reported as the files above. `None` as `prepare` leaves it
    /// without the tick.
    pub vanilla: Option<Box<PendingXpmf>>,
}

impl PendingXpmf {
    /// This pending set and the Vanilla folder's.
    pub fn folders(&self) -> impl Iterator<Item = &PendingXpmf> {
        std::iter::once(self).chain(self.vanilla.as_deref())
    }
}

/// XPMF accepts any slashes and case (schema `texture`), so a typed path is compared to the outputs folded.
/// shortcut: a typed path without its `textures/` or `.dds` is not matched to an output (so not checked); fold those
/// too if users type them.
fn fold_path(p: &str) -> String {
    p.replace('\\', "/").to_lowercase()
}

/// What refuses a built profile `doc` of `e` (M1; `prepare` and the card both ask): a file name that folds like another
/// landscape set's, then the schema. `Ok` is the file's text. Sets that are not landscape write nothing, so they
/// cannot clash (M2).
fn refusal(
    vs: &[SetView],
    entries: &[SetEntry],
    e: &SetEntry,
    doc: &Map<String, Value>,
) -> Result<String, String> {
    let name =
        file_name(&e.id).map_err(|why| format!("{}: XPMF profile not written: {why}", e.id))?;
    let landscape = |id: &str| {
        vs.iter()
            .any(|v| same_id(&v.id, id) && !v.members.is_empty() && is_landscape(v))
    };
    let others: Vec<&str> = entries
        .iter()
        .enumerate()
        .filter(|(i, o)| {
            o.xpmf.is_some()
                && !same_id(&o.id, &e.id)
                && !entries[..*i].iter().any(|x| same_id(&x.id, &o.id))
                && file_name(&o.id).as_ref() == Ok(&name)
                && landscape(&o.id)
        })
        .map(|(_, o)| o.id.as_str())
        .collect();
    if !others.is_empty() {
        return Err(format!(
            "{}: XPMF profile not written: {name} is also the file of {}",
            e.id,
            others.join(" and ")
        ));
    }
    let text = to_text(doc);
    let parsed: Value = serde_json::from_str(&text).expect("to_text writes JSON");
    crate::pgp::validate::validate_xpmf(&parsed)
        .map(|()| text)
        .map_err(|errs| format!("{}: XPMF profile not written: {}", e.id, errs.join("; ")))
}
/// Design X2-X9: one profile per set with a block, validated against the vendored schema. `None` when no set has a
/// block and nothing is recorded; with a record and no block it is empty, so the old profiles are reported (Task 5
/// ruling). Two sets whose ids fold to one file name are both refused.
pub fn prepare(m: &AppModel) -> Option<PendingXpmf> {
    let doc = m.sets_doc.as_ref()?;
    let entries = doc.entries();
    // The first entry with an id decides, as in cm_json.
    let blocks: Vec<&SetEntry> = entries
        .iter()
        .enumerate()
        .filter(|(i, e)| e.xpmf.is_some() && !entries[..*i].iter().any(|o| same_id(&o.id, &e.id)))
        .map(|(_, e)| e)
        .collect();
    let mut p = PendingXpmf {
        root: m.output_root(),
        // Note GI27 (owner ruling): a ticked Vanilla folder of an Also export gets a `pbr = false` copy of each PBR
        // Landscape set's profile, guarded and recorded as the main file is.
        vanilla: (m.also_ticks().0 && m.also_root().is_some()).then(|| {
            Box::new(PendingXpmf {
                root: m.also_folders()[1].clone(),
                ..Default::default()
            })
        }),
        ..Default::default()
    };
    if blocks.is_empty() {
        return (!doc.xpmf_shas().is_empty()).then_some(p);
    }
    let vs = views(m);
    // The project's own outputs by folded Data-relative path; with `vanilla`, the Vanilla folder's names for them.
    let outputs = |vanilla: bool| -> BTreeMap<String, String> {
        m.items
            .iter()
            .filter_map(|i| texture_rel(&p.root, i.job.as_ref()?, vanilla))
            .map(|r| (fold_path(&r), r))
            .collect()
    };
    let (own_outputs, vanilla_outputs) = (outputs(false), outputs(true));
    for e in &blocks {
        let name = match file_name(&e.id) {
            Ok(n) => n,
            Err(why) => {
                p.notes
                    .push(format!("{}: XPMF profile not written: {why}", e.id));
                continue;
            }
        };
        let rel = format!("{FOLDER}/{name}");
        // The profile built for one folder, its text, and the texture paths among the outputs of that folder.
        let build = |vanilla: bool, outputs: &BTreeMap<String, String>| {
            let doc = profile_in(m, &vs, e, vanilla)?;
            let text = refusal(&vs, &entries, e, &doc)?;
            let own: Vec<String> = TEXTURES
                .iter()
                .filter_map(|(k, _)| doc.get(*k)?.as_str())
                .filter_map(|t| outputs.get(&fold_path(t)).cloned())
                .collect();
            Ok::<_, String>((text, own))
        };
        match build(false, &own_outputs) {
            Ok((text, own)) => {
                if !own.is_empty() {
                    p.own_textures.insert(rel.clone(), own);
                }
                p.files.push((rel.clone(), text));
            }
            Err(note) => {
                if !p.refused.contains(&rel) {
                    p.refused.push(rel);
                }
                p.notes.push(note);
                continue;
            }
        }
        if let Some(van) = p.vanilla.as_mut().filter(|_| {
            !m.also_skipped(&e.id)
                && vs
                    .iter()
                    .any(|v| same_id(&v.id, &e.id) && is_pbr_landscape(v))
        }) {
            match build(true, &vanilla_outputs) {
                Ok((text, own)) => {
                    if !own.is_empty() {
                        van.own_textures.insert(rel.clone(), own);
                    }
                    van.files.push((rel, text));
                }
                Err(note) => {
                    van.refused.push(rel);
                    p.notes.push(format!("Vanilla folder: {note}"));
                }
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
    // Note GI27: the Vanilla folder's copies, with the same guard and reports (their records are absolute already).
    match p.vanilla.as_deref() {
        Some(v) => finish(v, &out, notes),
        None => out,
    }
}

impl AppModel {
    /// The card's toggle (Task 7): an empty `[sets.xpmf]` block, or none. One undo step.
    pub fn set_xpmf_on(&mut self, set: &str, on: bool) {
        if !self.sets_editable() {
            return;
        }
        let label = if on {
            "Write XPMF profile"
        } else {
            "Stop writing XPMF profile"
        };
        let s = set.to_string();
        self.edit_docs(label, None, false, move |_, d| {
            d.set_xpmf(&s, on.then(Map::new))
        });
    }

    /// Set (or with `None` reset) key `key` of the set's block; nothing without a block. Drags of one key merge into
    /// one undo step.
    pub fn set_xpmf_value(&mut self, set: &str, key: &str, v: Option<Value>) {
        if !self.sets_editable() {
            return;
        }
        let Some(mut b) = self.sets_doc.as_ref().and_then(|d| {
            d.entries()
                .iter()
                .find(|e| same_id(&e.id, set))
                .and_then(|e| e.xpmf.clone())
        }) else {
            return;
        };
        match v {
            Some(v) => b.insert(key.to_string(), v),
            None => b.remove(key),
        };
        let s = set.to_string();
        self.edit_docs(
            &format!("Change {key}"),
            Some(format!("{set}:xpmf:{key}")),
            false,
            move |_, d| d.set_xpmf(&s, Some(b)),
        );
    }

    /// The export ended. After a clean export (none cancelled or failed), write the profiles of `p` (taken from
    /// `pending_xpmf` by the caller) and record their hashes (saved, like the complex-material record). Otherwise
    /// only add a note.
    #[doc(hidden)]
    pub fn finish_xpmf(&mut self, p: Option<PendingXpmf>) {
        let Some(p) = p else {
            return;
        };
        let ok = self.export.error.is_none()
            && self
                .export
                .summary
                .as_ref()
                .is_some_and(|s| s.cancelled.is_empty() && s.failed == 0);
        if !ok {
            if p.folders().any(|f| !f.files.is_empty()) {
                self.export
                    .notes
                    .push("XPMF profiles not written: the export did not finish".into());
            }
            return;
        }
        let recorded = match self.sets_doc.as_ref() {
            Some(d) => d.xpmf_shas(),
            None => return,
        };
        let shas = finish(&p, &recorded, &mut self.export.notes);
        for f in p.folders() {
            self.also_json
                .extend(super::also::current_files(&f.root, &f.files));
        }
        if shas == recorded {
            return;
        }
        let Some(d) = self.sets_doc.as_mut() else {
            return;
        };
        let was_dirty = d.is_dirty();
        let side = d.set_xpmf_shas(shas);
        if d.read_only() {
            self.export.notes.push(super::sidecar_only_note(&side));
            return;
        }
        self.save_hash_record(was_dirty);
    }
}
