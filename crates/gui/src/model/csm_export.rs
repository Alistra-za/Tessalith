//! Note IE10: each PBR texture set's `.csm` material (design §5.2): `materials\<set prefix>.csm`, holding `Version`,
//! `PBREnabled`, the set's texture paths, the switches its PGPatcher settings and `cs_txst` block turn on and only the
//! numbers the block sets; never a generic key. Written after a clean export with the other JSON kinds' guard. A PBR
//! Landscape set keeps its inline texture-set JSON and gets none (design §5.3). No egui types.
use super::cs_pbr::{CsBlock, Kind};
use super::history::{orphans, records_at};
use super::sets::SetView;
use super::viewport3d::FileSource;
use super::AppModel;
use crate::pgp::settings::{Settings, FOLIAGE_KEY};
use crate::pgp::writer::EntryInput;
use crate::sets_doc::SetEntry;
use engine::config::Target;
use engine::sets::same_id;
use nif::csm::{self, Csm, Value};
use nif::pbr2::{COLOR_PARAMS, CSM_PARAMS, TXST_PARAMS};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Each PGPatcher switch pair (design §5.2): the `.csm` switch written when every listed setting is `true`. Checked
/// against PGPatcher's v1 output (`pgpsim::patch`) and `nif::pbr2::from_v1` in Note IE10: interlayer parallax comes
/// from `coat_parallax` (SLSF2 25 in v1), and the coat options need `multilayer` (the schema's coat sub-flags need
/// two-layer).
pub const PGP_SWITCHES: [(&[&str], &str); 9] = [
    (&["subsurface"], "PBRSubsurfaceEnabled"),
    (&[FOLIAGE_KEY], "PBRSubsurfaceEnabled"),
    (&[FOLIAGE_KEY], "PBRFoliageEnabled"),
    (&["hair"], "PBRHairMarschner"),
    (&["multilayer"], "PBRTwoLayerEnabled"),
    (&["multilayer", "coat_normal"], "PBRTwoLayerCoatNormal"),
    (&["multilayer", "coat_diffuse"], "PBRTwoLayerColoredCoat"),
    (
        &["multilayer", "coat_parallax"],
        "PBRTwoLayerInterlayerParallax",
    ),
    (&["fuzz_on"], "PBRFuzzEnabled"),
];

/// The `.csm` switches the `cs_txst` block holds (design §5.2), by block key (dotted under `glintParameters`).
/// PGPatcher's `glint_on` is not read: the block's `glintParameters.enabled` is the glint switch.
pub const BLOCK_SWITCHES: [(&str, &str); 4] = [
    ("glintParameters.enabled", "PBRGlintEnabled"),
    ("thinSurface", "PBRThinSurfaceEnabled"),
    ("foliage", "PBRFoliageEnabled"),
    ("fuzzRoughnessTexture", "PBRFuzzRoughnessTexture"),
];

/// The block number outside `nif::pbr2::TXST_PARAMS`.
const FUZZ_ROUGHNESS: (&str, &str) = ("fuzzRoughness", "PBRFuzzRoughness");

/// A block number's `.csm` key and whether it is a colour (`#rrggbb`).
#[derive(Debug, Clone, Copy, PartialEq)]
struct NumberKey {
    csm: &'static str,
    color: bool,
}

/// The block keys that are `.csm` numbers or colours: the `TXST_PARAMS` keys, then `fuzzRoughness`.
fn number_keys() -> impl Iterator<Item = &'static str> {
    TXST_PARAMS
        .iter()
        .map(|(k, _)| *k)
        .chain([FUZZ_ROUGHNESS.0])
}

/// The one TXST→`.csm` lookup for numbers (`csm_key` and `block_values` share it).
fn number_key(key: &str) -> Option<NumberKey> {
    if key == FUZZ_ROUGHNESS.0 {
        return Some(NumberKey {
            csm: FUZZ_ROUGHNESS.1,
            color: false,
        });
    }
    let (_, i) = TXST_PARAMS.iter().find(|(k, _)| *k == key)?;
    let (csm, _) = CSM_PARAMS.iter().find(|(_, j)| j == i)?;
    Some(NumberKey {
        csm,
        color: COLOR_PARAMS.contains(i),
    })
}

/// Why a set that gets a `.csm` has none this run, so its texture-set file is not written either.
pub const NO_ENTRY: &str = "the set writes no .csm this run (it has no PGPatcher entry)";

/// The texture-set key a `.csm` has no key for: dropped, with a warning when the block holds a non-default value.
pub const NO_CSM_KEY: &str = "innerLayerDisplacementOffset";

/// The slots CS PBR needs (the schema's `PBREnabled` rule): (game slot, name in the messages).
const NEEDED: [(u8, &str); 3] = [(1, "Diffuse"), (2, "Normal"), (6, "RMAOS")];

fn strip_ci<'a>(s: &'a str, prefix: &str) -> &'a str {
    match s.get(..prefix.len()) {
        Some(head) if head.eq_ignore_ascii_case(prefix) => &s[prefix.len()..],
        _ => s,
    }
}

/// `materials\` + the set's texture path without `textures\` and without `.dds`, + `.csm` (`textures\pbr\clutter\rug01`
/// → `materials\pbr\clutter\rug01.csm`). The path is the set's output prefix, which has no slot suffix. The form is
/// Data-relative with `\`, as the texture-set JSON's `csm` value (design §5.3; one constant to confirm against CS).
pub fn csm_path(set_id: &str) -> String {
    let p = set_id.trim().replace('/', "\\");
    let p = strip_ci(&p, "textures\\");
    let p = match p.len().checked_sub(4) {
        Some(n) if p[n..].eq_ignore_ascii_case(".dds") => &p[..n],
        _ => p,
    };
    format!("materials\\{p}.csm")
}

/// A Data-relative texture path as a `.csm` texture value: relative to `Data\textures`, with `/` (the spec's form).
fn texture_value(p: &str) -> String {
    strip_ci(&p.replace('\\', "/"), "textures/").to_string()
}

/// The `.csm` key a `cs_txst` block key (dotted under `glintParameters`) is written as; `None` for a key it has none
/// for (`innerLayerDisplacementOffset`, the `glintParameters` object itself).
pub fn csm_key(key: &str) -> Option<&'static str> {
    number_key(key).map(|n| n.csm).or_else(|| {
        BLOCK_SWITCHES
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, c)| *c)
    })
}

fn key_def(name: &str) -> Option<&'static csm::KeyDef> {
    csm::KEYS.iter().find(|k| k.name == name)
}

/// The schema range of a block number or colour channel that a `.csm` holds (a colour's is 0-1, what `#rrggbb` can
/// hold); `None` for a key without one. The set editor drags a `.csm` set's fields in it.
pub fn csm_range(key: &str) -> Option<(f64, f64)> {
    let def = key_def(csm_key(key)?)?;
    match def.kind {
        csm::Kind::Color => Some((0.0, 1.0)),
        _ => def.range,
    }
}

/// What a set's `.csm` is.
#[derive(Debug, Clone, PartialEq)]
pub struct SetCsm {
    /// `materials\...\<name>.csm`, Data-relative.
    pub path: String,
    pub csm: Csm,
    /// Clamped and left-out values; each names the set.
    pub warnings: Vec<String>,
    /// Why no `.csm` is written (a needed texture is missing, or the switches break a schema rule), naming the set.
    /// The set's texture-set JSON is not written either: it would name a missing file.
    pub blocked: Option<String>,
}

/// Whether set `v` gets a `.csm`: a PBR-family set but PBR Landscape (design §5.2; PBR grass included).
pub fn gets_csm(v: &SetView) -> bool {
    super::also::is_pbr_set(v) && !super::sets::is_pbr_landscape(v)
}

/// The `.csm` of set `v` from its PGPatcher settings `pgp` (`auto_flags::resolved_for`'s) and its `cs_txst` block;
/// `None` for a set that gets none. Texture paths are the set's own outputs (its prefix and each slot's suffix) and
/// explicit `slotN` settings; [`csm_sets`] adds borrowed slots as the PGPatcher entry names them.
pub fn set_csm(v: &SetView, pgp: &Settings, cs_txst: Option<&CsBlock>) -> Option<SetCsm> {
    build(v, pgp, cs_txst, &BTreeMap::new())
}

/// The `.csm` of set `v` as its PGPatcher entry `e` names it (`pbr_export::json_entries`): its resolved settings and
/// the slot paths it names (borrowed slots and renames included).
pub fn entry_csm(v: &SetView, e: &EntryInput, cs_txst: Option<&CsBlock>) -> Option<SetCsm> {
    build(v, &e.settings, cs_txst, &e.slots)
}

fn get<'a>(values: &'a serde_json::Map<String, Json>, key: &str) -> Option<&'a Json> {
    match key.split_once('.') {
        Some((o, k)) => values.get(o).and_then(|g| g.get(k)),
        None => values.get(key),
    }
}

/// `is outside lo..hi`, or one side when the other is unbounded.
fn outside(lo: f64, hi: f64) -> String {
    match (lo.is_finite(), hi.is_finite()) {
        (false, true) => format!("is above {hi}"),
        (true, false) => format!("is below {lo}"),
        _ => format!("is outside {lo}..{hi}"),
    }
}

/// `slots`: game slot → Data-relative path the PGPatcher entry names (borrowed slots, renamed files, `slotN`).
fn build(
    v: &SetView,
    pgp: &Settings,
    cs_txst: Option<&CsBlock>,
    slots: &BTreeMap<u8, String>,
) -> Option<SetCsm> {
    if !gets_csm(v) {
        return None;
    }
    let id = &v.id;
    let mut values = BTreeMap::new();
    let mut warnings = vec![];
    values.insert("PBREnabled", Value::Bool(true));
    let explicit = |n: u8| match pgp.get(&format!("slot{n}")) {
        Some(Json::String(s)) => Some(s.clone()),
        _ => None,
    };
    let filled = |n: u8| {
        v.slots
            .iter()
            .find(|(k, f)| k.game_slot() == Some(n) && f.is_filled())
            .map(|(k, _)| *k)
    };
    // Game slots that got a texture key, and those a lock leaves to the mesh.
    let mut present = BTreeSet::new();
    let mut locked = BTreeSet::new();
    let textures = csm::KEYS
        .iter()
        .filter(|k| k.kind == csm::Kind::Texture)
        .filter_map(|k| Some((k.name, k.slot? as u8 + 1)));
    for (name, n) in textures {
        // As the PGPatcher entry: an explicit `slotN` wins; a lock keeps the mesh's own texture (the key is left out).
        let path = explicit(n).or_else(|| {
            if crate::pgp::prefix::pgp_locked(pgp, n) {
                locked.insert(n);
                return None;
            }
            slots.get(&n).cloned().or_else(|| {
                let kind = filled(n)?;
                let pre = v.prefix.as_deref()?;
                let rel = crate::pgp::prefix::output_rel(pre, kind.suffix());
                Some(rel.to_string_lossy().into_owned())
            })
        });
        // `""` clears the slot (the spec's form; an absent key would inherit the NIF's texture): written as `""`, and no
        // texture for the needed-slot check.
        match path {
            Some(p) if p.trim().is_empty() => {
                values.insert(name, Value::Text(String::new()));
            }
            Some(p) => {
                values.insert(name, Value::Text(texture_value(&p)));
                present.insert(n);
            }
            None => {}
        }
    }
    // Missing: no key written and no lock (a locked slot keeps the mesh's texture).
    let missing: Vec<&str> = NEEDED
        .iter()
        .filter(|(n, _)| !present.contains(n) && !locked.contains(n))
        .map(|(_, name)| *name)
        .collect();
    let on = |k: &str| pgp.get(k) == Some(&Json::Bool(true));
    for (keys, name) in PGP_SWITCHES {
        if keys.iter().all(|k| on(k)) {
            values.insert(name, Value::Bool(true));
        }
    }
    if let Some(b) = cs_txst {
        block_values(id, b, &mut values, &mut warnings);
    }
    let mut c = Csm {
        version: csm::VERSION,
        values,
    };
    warnings.extend(
        finite_only(&mut c)
            .into_iter()
            .map(|w| format!("{id}: {w}")),
    );
    let blocked = if missing.is_empty() {
        invalid(&c).map(|why| format!("{id}: no .csm written: {why}"))
    } else {
        Some(format!(
            "{id}: no .csm written: the set has no {} texture; Community Shaders PBR needs Diffuse, Normal and RMAOS",
            missing.join(" or ")
        ))
    };
    Some(SetCsm {
        path: csm_path(v.prefix.as_deref().unwrap_or(id)),
        csm: c,
        warnings,
        blocked,
    })
}

/// The block's switches and numbers (only the keys it sets), clamped to the schema with a warning per clamp.
fn block_values(
    id: &str,
    b: &CsBlock,
    values: &mut BTreeMap<&'static str, Value>,
    warnings: &mut Vec<String>,
) {
    // Block keys holding a value of the wrong type (or no finite number): each left out with one warning.
    let mut unusable: BTreeSet<String> = BTreeSet::new();
    for (key, name) in BLOCK_SWITCHES {
        match get(&b.values, key) {
            Some(Json::Bool(true)) => {
                values.insert(name, Value::Bool(true));
            }
            None | Some(Json::Bool(false)) => {}
            Some(_) => {
                unusable.insert(key.to_string());
            }
        }
    }
    if b.values
        .get("glintParameters")
        .is_some_and(|g| !g.is_object())
    {
        unusable.insert("glintParameters".into());
    }
    for key in number_keys() {
        let (Some(x), Some(NumberKey { csm: name, color })) =
            (get(&b.values, key), number_key(key))
        else {
            continue;
        };
        if color {
            let Some(rgb) = x
                .as_array()
                .and_then(|a| a.iter().map(Json::as_f64).collect::<Option<Vec<f64>>>())
                .filter(|a| a.len() == 3 && a.iter().all(|c| c.is_finite()))
            else {
                unusable.insert(key.to_string());
                continue;
            };
            let hex = csm::hex([rgb[0] as f32, rgb[1] as f32, rgb[2] as f32]);
            if rgb.iter().any(|c| !(0.0..=1.0).contains(c)) {
                let [r, g, bl] = hex;
                warnings.push(format!(
                    "{id}: {name} {}, {}, {} {}; exported as #{r:02x}{g:02x}{bl:02x}",
                    rgb[0],
                    rgb[1],
                    rgb[2],
                    outside(0.0, 1.0)
                ));
            }
            values.insert(name, Value::Color(hex));
        } else if let Some(n) = x.as_f64().filter(|n| n.is_finite()) {
            let (lo, hi) = key_def(name)
                .and_then(|d| d.range)
                .unwrap_or((f64::NEG_INFINITY, f64::INFINITY));
            let to = n.clamp(lo, hi);
            if to != n {
                warnings.push(format!(
                    "{id}: {name} {n} {}; exported as {to}",
                    outside(lo, hi)
                ));
            }
            values.insert(name, Value::Num(to));
        } else {
            unusable.insert(key.to_string());
        }
    }
    // The offset has no `.csm` key: a value other than the field's default is not exported, which the user is told.
    let offset_default =
        super::cs_pbr::field(super::cs_pbr::Kind::Txst, NO_CSM_KEY).map_or(0.0, |f| f.default);
    match b.values.get(NO_CSM_KEY).map(Json::as_f64) {
        Some(Some(x)) if x != offset_default => warnings.push(format!(
            "{id}: {NO_CSM_KEY} is {x}, but a .csm has no key for it; it is not exported"
        )),
        Some(None) => {
            unusable.insert(NO_CSM_KEY.into());
        }
        _ => {}
    }
    // `bad` (read from the project file): a value of the wrong type, `nan` or `inf` included.
    unusable.extend(
        b.bad
            .iter()
            .filter(|k| csm_key(k).is_some() || *k == "glintParameters" || *k == NO_CSM_KEY)
            .cloned(),
    );
    for k in unusable {
        warnings.push(format!(
            "{id}: cs_txst.{k} is not a usable value (the wrong type, or not a finite number); left out of the .csm"
        ));
    }
}

/// Why `c` is no valid `.csm`: an allOf rule, a feature rule the shader's payload checks, or the schema's per-file
/// checks on the written text. `None` when it is fine.
fn invalid(c: &Csm) -> Option<String> {
    if let Some(r) = csm::rule_errors(c).first() {
        return Some((*r).to_string());
    }
    let features = nif::pbr2::pbr_values(c).map_or(0, |p| p.features);
    if let Some(r) = nif::pbr2::feature_errors(features & nif::pbr2::feat::PAYLOAD_FEATURES).first()
    {
        return Some((*r).to_string());
    }
    csm::parse(&csm::to_text(c)).err().map(|e| e.to_string())
}

/// Ledger ruling (Task 1): every number is finite before it is written. Removes the ones that are not and returns a
/// warning for each.
pub fn finite_only(c: &mut Csm) -> Vec<String> {
    let bad: Vec<&'static str> = c
        .values
        .iter()
        .filter(|(_, v)| matches!(v, Value::Num(n) if !n.is_finite()))
        .map(|(k, _)| *k)
        .collect();
    bad.into_iter()
        .map(|k| {
            c.values.remove(k);
            format!("{k} is not a finite number; left out of the .csm")
        })
        .collect()
}

/// Note IE12: set `c`'s `.csm` as a 3D view file: its canonical `materials/…` key and its text in memory, the source
/// id a hash of the text (an unchanged `.csm` is the same source, so no reload; a changed one reloads). `None` when the
/// path is no `.csm` under materials. The Meshes view's texture set picks and the set preview read it ahead of the
/// scan's copy of the same path.
pub fn memory_source(c: &SetCsm) -> Option<(String, FileSource)> {
    use std::hash::{Hash, Hasher};
    let key = nif::path::canon_material(&c.path)?;
    let text = csm::to_text(&c.csm);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    let src = FileSource::Memory {
        id: h.finish(),
        bytes: std::sync::Arc::new(text.into_bytes()),
    };
    Some((key, src))
}

/// The set entry whose `cs_txst` block binds TXST `edid` (exact case: CS matches the EDID as typed).
fn txst_binder<'a>(entries: &'a [SetEntry], edid: &str) -> Option<&'a SetEntry> {
    entries.iter().find(|e| {
        e.cs_txst
            .as_ref()
            .is_some_and(|b| b.edids.iter().any(|x| x == edid))
    })
}

/// Note IE12: for each TXST EDID in `edids` that a project set's `cs_txst` block binds, that set's in-memory `.csm`,
/// built as the export builds it ([`csm_sets`]: `entry_csm` from the set's PGPatcher entry, borrowed slots included;
/// unsaved edits included, exported or not). A set with no PGPatcher entry gets [`NO_ENTRY`] as its block reason: the
/// export writes neither its `.csm` nor its texture-set file. An EDID the export refuses (two sets bind it, or it
/// cannot be a file name: `cs_pbr_export::refusals`) binds no set. CS only; a set that gets no `.csm` (PBR Landscape,
/// not PBR) is left out, so its shapes keep their name field.
pub fn txst_csms(m: &AppModel, edids: &[String]) -> BTreeMap<String, SetCsm> {
    let mut out = BTreeMap::new();
    if m.target != Target::Cs || edids.is_empty() {
        return out;
    }
    let Some(doc) = m.sets_doc.as_ref() else {
        return out;
    };
    let entries = doc.entries();
    let bound: Vec<(&String, &SetEntry)> = edids
        .iter()
        .filter_map(|edid| Some((edid, txst_binder(&entries, edid)?)))
        .collect();
    if bound.is_empty() {
        return out;
    }
    let refused = super::cs_pbr_export::refusals(&entries);
    let views = super::sets::views(m);
    let mut built: Option<Vec<(SetView, SetCsm)>> = None;
    for (edid, e) in bound {
        if refused.contains_key(&(e.id.clone(), Kind::Txst, edid.clone())) {
            continue;
        }
        let Some(v) = views.iter().find(|v| same_id(&v.id, &e.id)) else {
            continue;
        };
        if !gets_csm(v) {
            continue;
        }
        let built = built.get_or_insert_with(|| csm_sets(m));
        let c = match built.iter().find(|(w, _)| same_id(&w.id, &e.id)) {
            Some((_, c)) => c.clone(),
            None => SetCsm {
                path: csm_path(v.prefix.as_deref().unwrap_or(&v.id)),
                csm: Csm::default(),
                warnings: vec![],
                blocked: Some(format!("{}: {NO_ENTRY}", v.id)),
            },
        };
        out.insert(edid.clone(), c);
    }
    out
}

/// Each set that gets a `.csm`, as the export writes it (CS only): the sets with a PGPatcher entry
/// (`pbr_export::json_entries`, whatever the statics rule says), their resolved settings, and the slot paths the entry
/// names (borrowed slots and renames included).
pub fn csm_sets(m: &AppModel) -> Vec<(SetView, SetCsm)> {
    if m.target != Target::Cs {
        return vec![];
    }
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    super::pbr_export::json_entries(m)
        .into_iter()
        .filter_map(|(v, e)| {
            let block = entries
                .iter()
                .find(|x| same_id(&x.id, &v.id))
                .and_then(|x| x.cs_txst.as_ref());
            let c = entry_csm(&v, &e, block)?;
            Some((v, c))
        })
        .collect()
}

/// What an export about to start writes once it has ended (computed when it starts, like the CS PBR JSON).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingCsm {
    pub root: PathBuf,
    /// (path under the root, `/`-separated; text).
    pub files: Vec<(String, String)>,
    /// Warnings and refused sets, for the export report.
    pub notes: Vec<String>,
    /// Paths (as in `files`) refused this run: not orphans, and their record stays.
    pub refused: Vec<String>,
    /// Note HF9: a run that leaves sets out (a scope or an excluded group) reports no orphans.
    pub scoped: bool,
}

/// One `.csm` per set in the run (CS only; `Some` without sets too, so the orphan report runs).
pub fn prepare(m: &AppModel) -> Option<PendingCsm> {
    if m.target != Target::Cs {
        return None;
    }
    m.sets_doc.as_ref()?;
    let mut p = PendingCsm {
        root: m.output_root(),
        scoped: !m.reports_orphans(),
        ..Default::default()
    };
    let mut seen = BTreeSet::new();
    for (v, c) in csm_sets(m).into_iter().filter(|(v, _)| m.in_run(&v.key)) {
        let rel = c.path.replace('\\', "/");
        p.notes.extend(c.warnings);
        if let Some(why) = c.blocked {
            p.notes.push(why);
            p.refused.push(rel);
            continue;
        }
        // Windows names ignore case; two sets on one prefix are already an export block (`path_collisions`).
        if !seen.insert(rel.to_lowercase()) {
            p.notes.push(format!(
                "{}: {} is also another set's .csm; written once",
                v.id, c.path
            ));
            continue;
        }
        p.files.push((rel, csm::to_text(&c.csm)));
    }
    Some(p)
}

/// After a clean export: write each file unless it was edited outside Tessalith, then report the recorded files this
/// run did not write (never deleted). Returns the new record.
pub fn finish(
    p: &PendingCsm,
    shas: &BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> BTreeMap<String, String> {
    notes.extend(p.notes.iter().cloned());
    let mut out = records_at(shas, &p.root);
    let mut written = 0;
    for (rel, text) in &p.files {
        if super::json_finish::guarded_write(&p.root.join(rel), text, &mut out, notes) {
            written += 1;
        }
    }
    if written > 0 {
        notes.push(format!(
            ".csm materials written: {written} file{} under {}",
            if written == 1 { "" } else { "s" },
            p.root.display()
        ));
    }
    let this_run = p.files.iter().map(|(r, _)| r).chain(&p.refused);
    for path in orphans(&out, &p.root, this_run)
        .into_iter()
        .filter(|_| !p.scoped)
    {
        notes.push(format!(
            "{}: no PBR set writes this .csm any more; the file was left in place",
            path.display()
        ));
    }
    out
}

impl super::json_finish::Guarded for PendingCsm {
    const NAME: &'static str = ".csm materials";
    // Note FW1: cancel-only, as the CS PBR JSON that points at these files.
    const GATE: Option<engine::also::Phase> = None;
    fn has_files(&self) -> bool {
        !self.files.is_empty()
    }
    fn recorded(d: &crate::sets_doc::SetsDoc) -> BTreeMap<String, String> {
        d.csm_shas()
    }
    fn write(
        &self,
        recorded: &BTreeMap<String, String>,
        notes: &mut Vec<String>,
    ) -> BTreeMap<String, String> {
        finish(self, recorded, notes)
    }
    fn current(&self) -> Vec<PathBuf> {
        super::also::current_files(&self.root, &self.files)
    }
    fn record(
        d: &mut crate::sets_doc::SetsDoc,
        shas: &BTreeMap<String, String>,
    ) -> std::io::Result<()> {
        d.set_csm_shas(shas)
    }
}
