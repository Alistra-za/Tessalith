//! Note IE10: each PBR texture set's `.csm` material (design §5.2): `materials\<set prefix>.csm`, holding `Version`,
//! `PBREnabled`, the set's texture paths, the switches its PGPatcher settings and `cs_txst` block turn on and only the
//! numbers the block sets; never a generic key. Written after a clean export with the other JSON kinds' guard. A PBR
//! Landscape set keeps its inline texture-set JSON and gets none (design §5.3). No egui types.
use super::cs_pbr::CsBlock;
use super::history::{orphans, records_at};
use super::sets::SetView;
use super::AppModel;
use crate::pgp::settings::{Settings, FOLIAGE_KEY};
use engine::config::Target;
use engine::sets::same_id;
use nif::csm::{self, Csm, Value};
use nif::pbr2::{COLOR_PARAMS, CSM_PARAMS, TXST_PARAMS};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Each PGPatcher switch pair (design §5.2): the `.csm` switch written when every listed setting is `true`. Checked
/// against PGPatcher's v1 output (`pgpsim::patch`) and `nif::pbr2::from_v1` in Note IE10: `multilayer` + `parallax`
/// is the plan's pair; PGPatcher's own v1 interlayer-parallax flag comes from `coat_parallax`.
pub const PGP_SWITCHES: [(&[&str], &str); 9] = [
    (&["subsurface"], "PBRSubsurfaceEnabled"),
    (&[FOLIAGE_KEY], "PBRSubsurfaceEnabled"),
    (&[FOLIAGE_KEY], "PBRFoliageEnabled"),
    (&["hair"], "PBRHairMarschner"),
    (&["multilayer"], "PBRTwoLayerEnabled"),
    (&["multilayer", "coat_normal"], "PBRTwoLayerCoatNormal"),
    (&["multilayer", "coat_diffuse"], "PBRTwoLayerColoredCoat"),
    (&["multilayer", "parallax"], "PBRTwoLayerInterlayerParallax"),
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
    if let Some((_, i)) = TXST_PARAMS.iter().find(|(k, _)| *k == key) {
        return CSM_PARAMS.iter().find(|(_, j)| j == i).map(|(k, _)| *k);
    }
    BLOCK_SWITCHES
        .iter()
        .chain([&FUZZ_ROUGHNESS])
        .find(|(k, _)| *k == key)
        .map(|(_, c)| *c)
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
    for def in csm::KEYS.iter().filter(|k| k.kind == csm::Kind::Texture) {
        let n = def.slot.map_or(0, |s| s as u8 + 1);
        // As the PGPatcher entry: an explicit `slotN` wins; a lock keeps the mesh's own texture (the key is left out).
        let path = explicit(n).or_else(|| {
            if crate::pgp::prefix::pgp_locked(pgp, n) {
                return None;
            }
            slots.get(&n).cloned().or_else(|| {
                let kind = filled(n)?;
                let pre = v.prefix.as_deref()?;
                let rel = crate::pgp::prefix::output_rel(pre, kind.suffix());
                Some(rel.to_string_lossy().into_owned())
            })
        });
        if let Some(p) = path {
            values.insert(def.name, Value::Text(texture_value(&p)));
        }
    }
    let missing: Vec<&str> = NEEDED
        .iter()
        .filter(|(n, _)| filled(*n).is_none() && explicit(*n).is_none_or(|s| s.is_empty()))
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
    for (key, name) in BLOCK_SWITCHES {
        if get(&b.values, key) == Some(&Json::Bool(true)) {
            values.insert(name, Value::Bool(true));
        }
    }
    let numbers = TXST_PARAMS
        .iter()
        .filter_map(|(k, i)| Some((*k, CSM_PARAMS.iter().find(|(_, j)| j == i)?.0, *i)))
        .chain([(FUZZ_ROUGHNESS.0, FUZZ_ROUGHNESS.1, usize::MAX)]);
    for (key, name, i) in numbers {
        let Some(x) = get(&b.values, key) else {
            continue;
        };
        if COLOR_PARAMS.contains(&i) {
            let Some(rgb) = x
                .as_array()
                .and_then(|a| a.iter().map(Json::as_f64).collect::<Option<Vec<f64>>>())
                .filter(|a| a.len() == 3 && a.iter().all(|c| c.is_finite()))
            else {
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
        } else if let Some(n) = x.as_f64() {
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
        }
    }
    // `bad`: a value of the wrong type, `nan` or `inf` included (JSON holds no such number).
    for k in b
        .bad
        .iter()
        .filter(|k| csm_key(k).is_some() || k.as_str() == "glintParameters")
    {
        warnings.push(format!(
            "{id}: cs_txst.{k} is not a usable value (the wrong type, or not a finite number); left out of the .csm"
        ));
    }
    if let Some(x) = b
        .values
        .get(NO_CSM_KEY)
        .and_then(Json::as_f64)
        .filter(|x| *x != 0.0)
    {
        warnings.push(format!(
            "{id}: {NO_CSM_KEY} is {x}, but a .csm has no key for it; it is not exported"
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
            let c = build(&v, &e.settings, block, &e.slots)?;
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
