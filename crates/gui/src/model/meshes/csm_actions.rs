//! Note IE14: the NIF editor's `.csm` actions (design §5.4): point a shape at its project set's `.csm`, and convert a
//! v1 PBR shape to one, copying its values into the set (or showing how they differ). The set export stays the only
//! `.csm` writer; these actions change the NIF's name field and flags and the set's `cs_txst` block. Also the Warn on a
//! `.csm` shape whose shader controller drives a value the `.csm` sets. No egui types.
use super::MeshesModel;
use crate::model::cs_pbr::{field_label, Kind};
use crate::model::csm_export::{
    csm_key, entry_csm, gets_csm, memory_source, BLOCK_SWITCHES, PGP_SWITCHES,
};
use crate::model::viewport3d::FileSource;
use crate::model::{AppModel, ViewKey};
use engine::config::Target as BuildTarget;
use engine::sets::same_id;
use nif::csm::{self, Csm, Value};
use nif::edit::validate::{var_label, Level, Target, Warning};
use nif::edit::{convert_v1, point_at_csm, EditError, Op};
use nif::material::SLSF2_TRUE_PBR;
use nif::model::{LightingModel, NifModel};
use nif::pbr2::{self, feat, PbrValues, PBR2_DEFAULTS, TXST_PARAMS};
use render3d::ShapeInfo;
use serde_json::{json, Map, Value as Json};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Why Convert to .csm (or Use the set's .csm) is refused: no project set owns the shape's diffuse.
pub const NO_SET: &str = "Make a texture set for these textures first.";
/// Off the Community Shaders target there is no `.csm` export.
pub const NOT_CS: &str =
    "A .csm is a Community Shaders file: set the project's target to Community Shaders first.";
/// The sets document cannot be edited (read-only or not loaded), so the values have nowhere to go.
pub const SETS_LOCKED: &str =
    "The project's texture sets cannot be edited now, so the shape's values have nowhere to go.";
/// Note IE14 (design §5.4, §7): CS 2.0 may draw a `.csm` value where a controller animates the NIF's.
pub const CSM_CTRL: &str = "CS 2.0 may not animate values a .csm sets";
/// The `fuzzRoughness` block key: a number, not in `TXST_PARAMS`.
const FUZZ_ROUGHNESS: &str = "fuzzRoughness";

/// One project set whose `.csm` a shape can use (Note IE14).
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectCsm {
    pub set_id: String,
    /// `materials\…\<name>.csm`, Data-relative (`csm_export::csm_path` of the set's prefix).
    pub path: String,
    /// The set's diffuse, `nif::path::canon_texture` form: its own output and the path its `.csm` names.
    pub diffuse: Vec<String>,
    /// The set's `cs_txst` block values as the document holds them (empty: no block).
    pub block: Map<String, Json>,
    /// The `.csm` switches the set's PGPatcher settings turn on (`PGP_SWITCHES`); empty without a PGPatcher entry.
    pub pgp_on: Vec<&'static str>,
    /// The set's in-memory `.csm` as a 3D view file (`csm_export::memory_source`), for the edit preview; None when
    /// the export writes none (no PGPatcher entry, or blocked).
    pub memory: Option<(String, FileSource)>,
}

/// The project sets as the app hands them over, and whether the actions can use them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectCsms {
    /// The project targets Community Shaders.
    pub cs: bool,
    /// The sets document can be edited (`AppModel::sets_editable`).
    pub editable: bool,
    pub sets: Vec<ProjectCsm>,
}

/// What `ProjectCsms` was built from: the sets' view key and the scan evidence revision.
pub(crate) type ProjectCsmKey = (ViewKey, u64);

/// Each set that gets a `.csm` (`csm_export::gets_csm`, PGPatcher entry or not), as the actions read it.
pub fn project_csms(app: &AppModel) -> ProjectCsms {
    let cs = app.target == BuildTarget::Cs;
    let mut out = ProjectCsms {
        cs,
        editable: app.sets_editable(),
        sets: vec![],
    };
    if !cs || app.sets_doc.is_none() {
        return out;
    }
    let entries = crate::model::pbr_export::json_entries(app);
    for v in crate::model::sets::views(app)
        .iter()
        .filter(|v| gets_csm(v))
    {
        let block = app.cs_block(&v.id, Kind::Txst);
        let entry = entries.iter().find(|(w, _)| same_id(&w.id, &v.id));
        let built = entry.and_then(|(w, e)| entry_csm(w, e, block.as_ref()));
        let mut diffuse = vec![];
        if let Some(pre) = v.prefix.as_deref() {
            let own = crate::pgp::prefix::output_rel(pre, "");
            diffuse.push(nif::path::canon_texture(own.to_string_lossy().as_bytes()).norm);
        }
        if let Some(Value::Text(t)) = built
            .as_ref()
            .and_then(|c| c.csm.values.get("DiffuseTexture"))
        {
            let n = nif::path::canon_texture(t.as_bytes()).norm;
            if !t.is_empty() && !diffuse.contains(&n) {
                diffuse.push(n);
            }
        }
        let pgp_on = entry.map_or_else(Vec::new, |(_, e)| {
            let on = |k: &str| e.settings.get(k) == Some(&Json::Bool(true));
            let mut names: Vec<&'static str> = PGP_SWITCHES
                .iter()
                .filter(|(keys, _)| keys.iter().all(|k| on(k)))
                .map(|(_, name)| *name)
                .collect();
            names.dedup();
            names
        });
        let path = built.as_ref().map_or_else(
            || crate::model::csm_export::csm_path(v.prefix.as_deref().unwrap_or(&v.id)),
            |c| c.path.clone(),
        );
        let memory = built
            .as_ref()
            .filter(|c| c.blocked.is_none())
            .and_then(memory_source);
        out.sets.push(ProjectCsm {
            set_id: v.id.clone(),
            path,
            diffuse,
            block: block.map(|b| b.values).unwrap_or_default(),
            pgp_on,
            memory,
        });
    }
    out
}

/// One row of the differences window: a value the set and the shape hold differently.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffRow {
    /// The readable name ("Specular level"); the block key or `.csm` switch is `key`.
    pub label: String,
    pub key: String,
    pub set: String,
    pub shape: String,
    /// A PGPatcher setting: shown, never replaced (the set's PGPatcher settings own it).
    pub read_only: bool,
}

/// Which action a name question continues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsmAction {
    Convert,
    Point,
}

/// The question a `.csm` action waits on.
#[derive(Debug, Clone, PartialEq)]
pub enum CsmAsk {
    /// The shader property's name field holds `old`, which Tessalith did not write (another tool may use it).
    Name {
        shape: u32,
        old: String,
        then: CsmAction,
    },
    /// The set holds values that differ from the shape's v1 values.
    Differences {
        set_id: String,
        rows: Vec<DiffRow>,
        /// What "Replace the set's values" writes (None: the key is removed).
        writes: Vec<(String, Option<Json>)>,
        /// Why "Replace the set's values" is off: the shape's switches with the set's PGPatcher settings make no
        /// valid `.csm`.
        replace_blocked: Option<String>,
        op: Box<Op>,
    },
}

/// The user's answer to a [`CsmAsk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsmAnswer {
    /// Name question: replace the name the dialog showed.
    ReplaceName,
    /// Differences: keep the set as it is.
    UseSet,
    /// Differences: the set takes the shape's values.
    ReplaceSet,
    Cancel,
}

/// Values for a project set and the NIF op that follows them (the app writes the set, then pushes the op).
#[derive(Debug, Clone, PartialEq)]
pub struct CsmWrite {
    pub set_id: String,
    /// The set history's undo step.
    pub label: String,
    pub values: Vec<(String, Option<Json>)>,
    pub op: Op,
    /// The line shown once the op is pushed.
    pub notice: String,
}

/// The lighting property of shape `shape`.
fn lighting_of(m: &NifModel, shape: u32) -> Option<&LightingModel> {
    m.lighting.get(&m.shape(shape)?.shader?)
}

/// The shape's diffuse (slot 1) in `canon_texture` form; None when it has no texture set or an empty slot.
fn diffuse_of(m: &NifModel, shape: u32) -> Option<String> {
    let ts = m.texsets.get(&lighting_of(m, shape)?.texture_set?)?;
    let raw = ts.raw.slots.first()?;
    (!raw.is_empty()).then(|| nif::path::canon_texture(raw).norm)
}

/// The block value at dotted `key`.
fn get<'a>(b: &'a Map<String, Json>, key: &str) -> Option<&'a Json> {
    match key.split_once('.') {
        Some((o, k)) => b.get(o).and_then(|g| g.get(k)),
        None => b.get(key),
    }
}

/// The block keys a Convert owns: every number (`TXST_PARAMS`, `fuzzRoughness`), then the block's switches.
fn owned_keys() -> impl Iterator<Item = &'static str> {
    TXST_PARAMS
        .iter()
        .map(|(k, _)| *k)
        .chain([FUZZ_ROUGHNESS])
        .chain(BLOCK_SWITCHES.iter().map(|(k, _)| *k))
}

fn is_switch(key: &str) -> bool {
    BLOCK_SWITCHES.iter().any(|(k, _)| *k == key)
}

/// The block values that stand for `v`: the numbers `csm_from_values` writes (clamped to the spec; a colour to 0-1,
/// which is all `#rrggbb` holds), the fuzz roughness when it is not -1, and the glint, thin-surface, foliage and
/// fuzz-roughness-texture switches that are on.
pub fn shape_block(v: &PbrValues) -> BTreeMap<String, Json> {
    let c = pbr2::csm_from_values(v);
    let mut out = BTreeMap::new();
    for &(key, i) in TXST_PARAMS {
        let Some(name) = csm_key(key) else { continue };
        match c.values.get(name) {
            Some(Value::Color(_)) => {
                let rgb: Vec<f64> = (i..i + 3)
                    .map(|j| csm::num(pbr2::clamp_param(j, v.params[j]).clamp(0.0, 1.0)))
                    .collect();
                out.insert(key.to_string(), json!(rgb));
            }
            Some(Value::Num(n)) => {
                out.insert(key.to_string(), json!(n));
            }
            _ => {}
        }
    }
    if let Some(Value::Num(n)) = c.values.get("PBRFuzzRoughness") {
        if *n != -1.0 {
            out.insert(FUZZ_ROUGHNESS.into(), json!(n));
        }
    }
    for (key, name) in BLOCK_SWITCHES {
        if c.values.get(name) == Some(&Value::Bool(true)) {
            out.insert(key.to_string(), Json::Bool(true));
        }
    }
    out
}

/// The owned values the set's block holds (a switch only when true).
fn set_block(b: &Map<String, Json>) -> BTreeMap<String, Json> {
    owned_keys()
        .filter_map(|k| {
            let v = get(b, k)?;
            (!is_switch(k) || v == &Json::Bool(true)).then(|| (k.to_string(), v.clone()))
        })
        .collect()
}

/// `key`'s value as the `.csm` writes it (`csm::num`, `#rrggbb`), the default for an absent one: what two values are
/// compared by.
fn shown(key: &str, v: Option<&Json>) -> String {
    if is_switch(key) {
        return if v == Some(&Json::Bool(true)) {
            "on"
        } else {
            "off"
        }
        .into();
    }
    let default = || match key {
        FUZZ_ROUGHNESS => Some(-1.0),
        _ => TXST_PARAMS
            .iter()
            .find(|(k, _)| *k == key)
            .and_then(|(_, i)| PBR2_DEFAULTS.get(*i))
            .map(|d| *d as f64),
    };
    let color = |a: &[Json]| -> Option<String> {
        let c: Vec<f32> = a
            .iter()
            .map(|x| x.as_f64().map(|f| f as f32))
            .collect::<Option<_>>()?;
        let [r, g, b] = csm::hex(<[f32; 3]>::try_from(c).ok()?);
        Some(format!("#{r:02x}{g:02x}{b:02x}"))
    };
    match v {
        Some(Json::Array(a)) => color(a).unwrap_or_else(|| Json::Array(a.clone()).to_string()),
        Some(Json::Number(n)) => n
            .as_f64()
            .map_or_else(|| n.to_string(), |f| csm::num(f as f32).to_string()),
        Some(other) => other.to_string(),
        None => match TXST_PARAMS.iter().find(|(k, _)| *k == key) {
            Some((_, i)) if pbr2::COLOR_PARAMS.contains(i) => {
                let d = &PBR2_DEFAULTS[*i..*i + 3];
                let [r, g, b] = csm::hex([d[0], d[1], d[2]]);
                format!("#{r:02x}{g:02x}{b:02x}")
            }
            _ => default().map_or_else(String::new, |d| csm::num(d as f32).to_string()),
        },
    }
}

/// A block key's readable name: the set editor's (`field_label`, by the last dotted part).
fn label_of(key: &str) -> String {
    let last = key.rsplit('.').next().unwrap_or(key);
    match key {
        "glintParameters.enabled" => "Glint".into(),
        _ => field_label(last).to_string(),
    }
}

/// The PGPatcher-owned `.csm` switches the shape's v1 flags turn on (the switches the block does not own).
fn pgp_switches_of(features: u32) -> BTreeSet<&'static str> {
    pbr2::CSM_SWITCHES
        .iter()
        .filter(|(name, bit)| features & bit != 0 && !BLOCK_SWITCHES.iter().any(|(_, n)| n == name))
        .map(|(name, _)| *name)
        .collect()
}

/// The PGPatcher switches the set and the shape hold differently: read-only rows.
fn pgp_rows(set: &ProjectCsm, features: u32) -> Vec<DiffRow> {
    let shape = pgp_switches_of(features);
    let set_on: BTreeSet<&str> = set
        .pgp_on
        .iter()
        .copied()
        .filter(|n| !BLOCK_SWITCHES.iter().any(|(_, b)| b == n))
        .collect();
    let on = |b: bool| if b { "on" } else { "off" }.to_string();
    shape
        .union(&set_on)
        .filter(|n| shape.contains(*n) != set_on.contains(*n))
        .map(|n| DiffRow {
            label: n.to_string(),
            key: n.to_string(),
            set: on(set_on.contains(n)),
            shape: on(shape.contains(n)),
            read_only: true,
        })
        .collect()
}

/// Why the set's PGPatcher switches with the block switches `block` make no valid `.csm` (a schema rule or a payload
/// feature rule); None when they are fine.
fn switch_error(set: &ProjectCsm, block: &BTreeMap<String, Json>) -> Option<String> {
    let mut values = BTreeMap::new();
    values.insert("PBREnabled", Value::Bool(true));
    for n in &set.pgp_on {
        values.insert(*n, Value::Bool(true));
    }
    for (key, name) in BLOCK_SWITCHES {
        if block.get(key) == Some(&Json::Bool(true)) {
            values.insert(name, Value::Bool(true));
        }
    }
    let c = Csm {
        version: csm::VERSION,
        values,
    };
    if let Some(r) = csm::rule_errors(&c).first() {
        return Some((*r).to_string());
    }
    let f = pbr2::pbr_values(&c).map_or(0, |p| p.features);
    pbr2::feature_errors(f & feat::PAYLOAD_FEATURES)
        .first()
        .map(|r| (*r).to_string())
}

/// Why set `id` would get no `.csm` with the shape's switches.
fn no_valid_csm(id: &str, why: &str) -> String {
    format!(
        "{id}: the shape's switches with the set's PGPatcher settings make no valid .csm: {why}"
    )
}

/// A line naming the PGPatcher switches that differ, or None.
fn pgp_notice(set_id: &str, rows: &[DiffRow]) -> Option<String> {
    let names: Vec<String> = rows
        .iter()
        .filter(|r| r.read_only)
        .map(|r| format!("{} ({} on the shape)", r.label, r.shape))
        .collect();
    (!names.is_empty()).then(|| {
        format!(
            "{set_id}'s PGPatcher settings differ from the shape: {}. Change them in the set's PGPatcher settings.",
            names.join(", ")
        )
    })
}

/// What a step decided.
enum Step {
    Ask(CsmAsk),
    Write(CsmWrite),
    Push(Op, Option<String>),
}

impl MeshesModel {
    /// Note IE14: the app's per-frame hand-over while a NIF edit is open: [`project_csms`], built only when the sets'
    /// view key or the scan evidence revision moved (an idle frame builds nothing, Note FI18).
    pub fn sync_project_csms(&mut self, app: &AppModel) {
        if self.edit.is_none() {
            return;
        }
        let key = (app.view_key(), app.evidence_rev);
        if self.project_csm_key.as_ref() == Some(&key) {
            return;
        }
        self.project_csm_key = Some(key);
        self.set_project_csms(project_csms(app));
    }

    /// The project sets the `.csm` actions read; a changed in-memory `.csm` a shape names reaches the edit preview.
    pub fn set_project_csms(&mut self, p: ProjectCsms) {
        if self.project_csms == p {
            return;
        }
        self.project_csms = p;
        self.refresh_edit_materials();
    }

    /// The in-memory `.csm` of each project set by its canonical `materials/…` key (the edit preview reads these
    /// ahead of the scan).
    pub(super) fn project_materials(&self) -> HashMap<String, FileSource> {
        self.project_csms
            .sets
            .iter()
            .filter_map(|s| s.memory.clone())
            .collect()
    }

    /// The project set whose diffuse is shape `shape`'s.
    pub fn set_for_shape(&self, shape: u32) -> Option<&ProjectCsm> {
        let m = &self.edit.as_ref()?.model;
        let d = diffuse_of(m, shape)?;
        self.project_csms
            .sets
            .iter()
            .find(|s| s.diffuse.contains(&d))
    }

    /// Convert to .csm on shape `shape` (a v1 PBR shape): see [`Self::csm_step`].
    pub fn convert_to_csm(&mut self, shape: u32) {
        self.csm_step(shape, CsmAction::Convert, None);
    }

    /// Use the set's .csm on shape `shape`: its name field becomes the project set's `.csm` path.
    pub fn point_at_set_csm(&mut self, shape: u32) {
        self.csm_step(shape, CsmAction::Point, None);
    }

    /// One step of a `.csm` action. Refused (a line in the edit's notice) off CS or when no project set owns the
    /// shape's diffuse. A name field Tessalith did not write asks first ([`CsmAsk::Name`]); `agreed` is the name the
    /// user agreed to replace, used only while the field still holds it (a source reload asks again). Convert then
    /// compares the shape's values with the set's: the set has none, they are copied ([`CsmWrite`]); equal, the op is
    /// pushed; they differ, [`CsmAsk::Differences`].
    fn csm_step(&mut self, shape: u32, then: CsmAction, agreed: Option<String>) {
        self.csm_ask = None;
        let step = self.plan_step(shape, then, agreed);
        let Some(e) = &mut self.edit else { return };
        match step {
            Err(why) => e.notice = Some(why),
            Ok(Step::Ask(a)) => self.csm_ask = Some(a),
            Ok(Step::Write(w)) => self.csm_write = Some(w),
            Ok(Step::Push(op, notice)) => {
                self.edit_push(vec![op], None);
                if let Some(e) = &mut self.edit {
                    e.notice = notice;
                }
            }
        }
    }

    fn plan_step(
        &self,
        shape: u32,
        then: CsmAction,
        agreed: Option<String>,
    ) -> Result<Step, String> {
        let Some(e) = &self.edit else {
            return Err(String::new());
        };
        if !self.project_csms.cs {
            return Err(NOT_CS.into());
        }
        let set = self.set_for_shape(shape).ok_or(NO_SET)?;
        let current = lighting_of(&e.model, shape)
            .and_then(|l| l.lighting.name.as_deref())
            .unwrap_or("")
            .to_string();
        let force = agreed.is_some_and(|a| a == current);
        let name_asks = |r: EditError| match r {
            EditError::NameInUse(old) => Ok(Step::Ask(CsmAsk::Name { shape, old, then })),
            r => Err(r.to_string()),
        };
        let (op, values) = match then {
            CsmAction::Point => {
                return match point_at_csm(&e.model, shape, &set.path, force) {
                    Ok(op) => Ok(Step::Push(op, None)),
                    Err(r) => name_asks(r),
                }
            }
            CsmAction::Convert => match convert_v1(&e.model, shape, &set.path, force) {
                Ok(x) => x,
                Err(r) => return name_asks(r),
            },
        };
        let mine = shape_block(&values);
        let theirs = set_block(&set.block);
        let pgp = pgp_rows(set, values.features);
        let notice = pgp_notice(&set.set_id, &pgp);
        if theirs.is_empty() {
            if !self.project_csms.editable {
                return Err(SETS_LOCKED.into());
            }
            if let Some(why) = switch_error(set, &mine) {
                return Err(no_valid_csm(&set.set_id, &why));
            }
            let copied = format!("Copied the shape's PBR values into {}.", set.set_id);
            return Ok(Step::Write(CsmWrite {
                set_id: set.set_id.clone(),
                label: "Copy the shape's PBR values".into(),
                values: mine.into_iter().map(|(k, v)| (k, Some(v))).collect(),
                op,
                notice: notice.map_or(copied.clone(), |n| format!("{copied} {n}")),
            }));
        }
        let keys: BTreeSet<&String> = mine.keys().chain(theirs.keys()).collect();
        let mut rows: Vec<DiffRow> = keys
            .iter()
            .filter_map(|k| {
                let (s, t) = (shown(k, theirs.get(*k)), shown(k, mine.get(*k)));
                (s != t).then(|| DiffRow {
                    label: label_of(k),
                    key: k.to_string(),
                    set: s,
                    shape: t,
                    read_only: false,
                })
            })
            .collect();
        if rows.is_empty() {
            return Ok(Step::Push(op, notice));
        }
        let writes = keys
            .iter()
            .map(|k| (k.to_string(), mine.get(*k).cloned()))
            .collect();
        rows.extend(pgp);
        let replace_blocked = switch_error(set, &mine).map(|why| no_valid_csm(&set.set_id, &why));
        Ok(Step::Ask(CsmAsk::Differences {
            set_id: set.set_id.clone(),
            rows,
            writes,
            replace_blocked,
            op: Box::new(op),
        }))
    }

    /// The answer to the open [`CsmAsk`]; nothing happens without one.
    pub fn answer_csm(&mut self, a: CsmAnswer) {
        let Some(ask) = self.csm_ask.take() else {
            return;
        };
        match (ask, a) {
            (_, CsmAnswer::Cancel) => {}
            (CsmAsk::Name { shape, old, then }, CsmAnswer::ReplaceName) => {
                self.csm_step(shape, then, Some(old))
            }
            (
                CsmAsk::Differences {
                    set_id, rows, op, ..
                },
                CsmAnswer::UseSet,
            ) => {
                self.edit_push(vec![*op], None);
                if let Some(e) = &mut self.edit {
                    e.notice = pgp_notice(&set_id, &rows);
                }
            }
            (
                CsmAsk::Differences {
                    set_id,
                    rows,
                    writes,
                    replace_blocked,
                    op,
                },
                CsmAnswer::ReplaceSet,
            ) => {
                let why = match replace_blocked {
                    Some(why) => Some(why),
                    None => (!self.project_csms.editable).then(|| SETS_LOCKED.to_string()),
                };
                if let Some(why) = why {
                    if let Some(e) = &mut self.edit {
                        e.notice = Some(why);
                    }
                    return;
                }
                let done = format!("Replaced {set_id}'s PBR values with the shape's.");
                self.csm_write = Some(CsmWrite {
                    label: "Replace the set's PBR values".into(),
                    values: writes,
                    op: *op,
                    notice: pgp_notice(&set_id, &rows)
                        .map_or(done.clone(), |n| format!("{done} {n}")),
                    set_id,
                });
            }
            // An answer that does not fit the question (a stale click): the question stays.
            (ask, _) => self.csm_ask = Some(ask),
        }
    }

    /// Note IE14 (design §5.4, §7): a Warn under each shader controller that drives a value a `.csm` sets, on a shape
    /// whose loaded material is CS 2.0 PBR (`v3.shapes`, from its resolved `.csm`). Called on each poll (the load
    /// lands later than the edit); replaced only when it changes.
    pub fn refresh_csm_warnings(&mut self) {
        let Some(e) = &mut self.edit else { return };
        let w = csm_ctrl_warnings(&e.model, &self.v3.shapes);
        if e.csm_warnings != w {
            e.csm_warnings = w;
            e.refresh_blocked();
        }
    }
}

/// [`MeshesModel::refresh_csm_warnings`]'s rows: the PBR value controllers are those whose variable means another
/// value on a PBR shape (`var_label` with and without the PBR flag: LSCF 9 and 10, LSCC 0), on lighting properties
/// with a user whose loaded material has PBR values.
pub fn csm_ctrl_warnings(m: &NifModel, loaded: &[ShapeInfo]) -> Vec<Warning> {
    let pbr_shapes: BTreeSet<u32> = loaded
        .iter()
        .filter(|s| s.material.pbr.is_some())
        .map(|s| s.id.0)
        .collect();
    let mut out = vec![];
    for l in m.lighting.values() {
        if !l.prop.users.iter().any(|u| pbr_shapes.contains(u)) {
            continue;
        }
        for c in &l.prop.ctrls {
            let Some(t) = m.ctrls.get(c) else { continue };
            let Some(v) = t.var else { continue };
            if var_label(&t.class, v, SLSF2_TRUE_PBR) != var_label(&t.class, v, 0) {
                out.push(Warning {
                    level: Level::Warn,
                    target: Target::Block { block: *c },
                    text: CSM_CTRL.into(),
                    fix: None,
                });
            }
        }
    }
    out
}

/// Note IE14: the app's hand-back of a Convert: the set's values in one undo step of the sets history, then the NIF
/// op (its own history), so a shape is never converted while its set did not take the values.
pub fn apply_csm_write(app: &mut AppModel, m: &mut MeshesModel) {
    let Some(w) = m.csm_write.take() else { return };
    if !app.set_cs_values(&w.set_id, &w.label, w.values) {
        if let Some(e) = &mut m.edit {
            e.notice = Some(SETS_LOCKED.into());
        }
        return;
    }
    m.edit_push(vec![w.op], None);
    if let Some(e) = &mut m.edit {
        e.notice = Some(w.notice);
    }
}
