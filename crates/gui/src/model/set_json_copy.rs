//! Copy JSON settings from one set to the selected sets: the material values a set's writers emit (rows), what each
//! target would take (skips, with their reasons) and the one-step apply. Identity (matches, slot commands, rename,
//! PGPatcher locks, EDIDs, XPMF names and paths, the cubemap path) and every control never copy, and a copy never
//! creates a block. No egui types.
use super::bulk::{Change, MenuAction, MenuEntry};
use super::cs_pbr::{
    mato_synced, pgp_to_cs, Kind, GLINT_FIELDS, MATO_FIELDS, PGP_MAP, TXST_FIELDS,
};
use super::cs_pbr_export::writable;
use super::messages::Severity;
use super::sets::{is_json_set, is_pbr_grass, views, SetView, SlotFill, SlotKind};
use super::skin::{skin_field, skin_value_problem};
use super::xpmf_json::FieldKind as XpmfKind;
use super::{cm_json, labels, skin, xpmf_json, AppModel, Dialog, Selection};
use crate::pgp::schema::{grass_ignores, Group, KeyKind, KEYS};
use crate::pgp::writer::{clamped, colour};
use crate::sets_doc::SetEntry;
use crate::ui::pgp_fields::edit_key;
use engine::sets::same_id;
use serde_json::Value;

/// The six places a set keeps JSON settings, in the dialog's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JsonGroup {
    Pgp,
    CsTxst,
    CsMato,
    Cm,
    Xpmf,
    Skin,
}

impl JsonGroup {
    /// W5: the group's heading in the dialog, and the first part of a row's accessible name.
    pub fn heading(self) -> &'static str {
        match self {
            JsonGroup::Pgp => "PGPatcher JSON",
            JsonGroup::CsTxst => "CS PBR texture set",
            JsonGroup::CsMato => "CS PBR material object",
            JsonGroup::Cm => "Complex material JSON",
            JsonGroup::Xpmf => "XPMF profile",
            JsonGroup::Skin => "Advanced skin",
        }
    }
}

/// One value a copy can carry: the group it belongs to, its storage key (the key a write to a target uses), the label
/// the dialog shows and the value the source's writer exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonRow {
    pub group: JsonGroup,
    pub key: String,
    pub label: String,
    pub value: Value,
}

impl JsonRow {
    /// W6: the value as the dialog shows it, a string without its quotes.
    pub fn value_text(&self) -> String {
        match &self.value {
            Value::String(s) => s.clone(),
            v => v.to_string(),
        }
    }
}

/// The rows a dialog has ticked, in order: the one conversion at the Apply boundary.
pub fn ticked(rows: &[(JsonRow, bool)]) -> Vec<JsonRow> {
    rows.iter().filter(|r| r.1).map(|r| r.0.clone()).collect()
}

/// W15: one report line for a row: how many targets took it and, per reason, how many skipped it.
fn copy_report(label: &str, plan: &[(String, Change)]) -> String {
    let n = plan.len();
    let written = plan.iter().filter(|(_, c)| *c == Change::Write).count();
    let mut skipped: Vec<(&str, usize)> = vec![];
    for (_, c) in plan {
        if let Change::Skip(r) = c {
            match skipped.iter_mut().find(|e| e.0 == r.as_str()) {
                Some(e) => e.1 += 1,
                None => skipped.push((r.as_str(), 1)),
            }
        }
    }
    let mut s = format!("{label}: written to {written} of {n}");
    for (r, k) in skipped {
        s.push_str(&format!("; skipped on {k} of {n}: {r}"));
    }
    s
}

fn row(group: JsonGroup, key: &str, label: String, value: &Value) -> JsonRow {
    JsonRow {
        group,
        key: key.to_string(),
        label,
        value: value.clone(),
    }
}

fn readable(key: &str) -> String {
    labels::readable(&key.replace('.', "_"))
}

/// Whether `PGP_MAP` maps PGPatcher key `key` to a CS texture-set field.
fn pgp_mapped(key: &str) -> bool {
    PGP_MAP.iter().any(|(_, p)| *p == key)
}

/// Whether `PGP_MAP` maps CS texture-set field `name` (a top-level name, so every `glintParameters` sub-key counts).
fn cs_mapped(name: &str) -> bool {
    PGP_MAP
        .iter()
        .any(|(cs, _)| cs.split('.').next() == Some(name))
}

fn numbers(v: &Value, lens: &[usize]) -> bool {
    v.as_array()
        .is_some_and(|a| lens.contains(&a.len()) && a.iter().all(Value::is_number))
}

/// PGPatcher rows: every `KEYS` entry the source stores with the type its writer reads, material keys only, each
/// valued as the source's own writer exports it.
fn pgp_rows(e: &SetEntry) -> Vec<JsonRow> {
    let block = e.cs_txst.is_some();
    let mut rows = vec![];
    for spec in KEYS {
        if matches!(spec.group, Group::Match | Group::Slots | Group::Locks)
            || matches!(spec.kind, KeyKind::Text | KeyKind::Slot)
        {
            continue;
        }
        // `glint` and `fuzz` are read and written as their switches, never as the raw key.
        let key = edit_key(spec.key);
        let Some(v) = e.settings.get(key) else {
            continue;
        };
        let mapped = pgp_mapped(key);
        let value = match spec.kind {
            KeyKind::OnlyTrue | KeyKind::Tri | KeyKind::Object => v.is_boolean().then(|| v.clone()),
            KeyKind::Number(range) => v.is_number().then(|| {
                // A mapped key on a set with a texture-set block reaches the settings through the block and the CS
                // writer exports it as it is; any other number is what the PGPatcher writer writes.
                if mapped && block {
                    v.clone()
                } else {
                    clamped(v, range)
                }
            }),
            KeyKind::Rgb => numbers(v, &[3]).then(|| {
                if mapped && !block {
                    colour(v, false).map_or_else(|| v.clone(), |n| n.to_value())
                } else {
                    v.clone()
                }
            }),
            KeyKind::Rgba => numbers(v, &[3, 4]).then(|| v.clone()),
            KeyKind::Text | KeyKind::Slot => None,
        };
        if let Some(value) = value {
            rows.push(row(JsonGroup::Pgp, key, spec.label.to_string(), &value));
        }
    }
    rows
}

/// CS texture-set rows: the block's values the CS writer exports, minus every name `PGP_MAP` maps (those are the
/// PGPatcher rows).
fn txst_rows(e: &SetEntry) -> Vec<JsonRow> {
    let Some(b) = &e.cs_txst else {
        return vec![];
    };
    TXST_FIELDS
        .iter()
        .filter(|f| !cs_mapped(f.name))
        .filter_map(|f| {
            let v = b
                .values
                .get(f.name)
                .filter(|v| writable(Kind::Txst, f.name, v))?;
            Some(row(JsonGroup::CsTxst, f.name, readable(f.name), v))
        })
        .collect()
}

/// CS material-object rows: the block's exported values, `glintParameters` flattened to its sub-keys.
fn mato_rows(e: &SetEntry) -> Vec<JsonRow> {
    let Some(b) = &e.cs_mato else {
        return vec![];
    };
    let mut rows = vec![];
    for f in MATO_FIELDS {
        let Some(v) = b
            .values
            .get(f.name)
            .filter(|v| writable(Kind::Mato, f.name, v))
        else {
            continue;
        };
        match v.as_object() {
            None => rows.push(row(JsonGroup::CsMato, f.name, readable(f.name), v)),
            Some(o) => {
                let subs = std::iter::once("enabled").chain(GLINT_FIELDS.iter().map(|g| g.name));
                for sub in subs {
                    if let Some(x) = o.get(sub) {
                        let key = format!("{}.{sub}", f.name);
                        rows.push(row(JsonGroup::CsMato, &key, readable(&key), x));
                    }
                }
            }
        }
    }
    rows
}

/// Complex material rows: the schema keys (and PGPatcher's `disabled`) with a valid value. `export` is a control.
fn cm_rows(e: &SetEntry) -> Vec<JsonRow> {
    let Some(b) = &e.cm else {
        return vec![];
    };
    cm_json::KEYS
        .iter()
        .map(|k| k.name)
        .chain(["disabled"])
        .filter_map(|name| {
            let k = cm_json::key(name)?;
            let v = b.values.get(name).filter(|v| cm_json::valid(k.kind, v))?;
            Some(row(JsonGroup::Cm, name, readable(name), v))
        })
        .collect()
}

/// XPMF rows: the booleans and the numbers inside the schema's bounds (a number outside them makes the writer refuse
/// the whole profile). `pbr` is a per-set choice; names, patterns and paths are identity.
fn xpmf_rows(e: &SetEntry) -> Vec<JsonRow> {
    let Some(b) = &e.xpmf else {
        return vec![];
    };
    xpmf_json::FIELDS
        .iter()
        .filter(|f| f.key != "pbr")
        .filter_map(|f| {
            let v = b.get(f.key)?;
            let ok = match f.kind {
                XpmfKind::Bool => v.is_boolean(),
                XpmfKind::Number {
                    min,
                    max,
                    exclusive_min,
                } => v.as_f64().is_some_and(|x| {
                    (if exclusive_min { x > min } else { x >= min }) && max.is_none_or(|m| x <= m)
                }),
                _ => false,
            };
            ok.then(|| row(JsonGroup::Xpmf, f.key, readable(f.key), v))
        })
        .collect()
}

/// Advanced skin rows: the explicit values the skin writer accepts.
fn skin_rows(e: &SetEntry) -> Vec<JsonRow> {
    skin::FIELDS
        .iter()
        .filter_map(|f| {
            let v = e.skin.get(f.key)?;
            (skin_field(f.key).is_some() && skin_value_problem(f.key, v).is_none())
                .then(|| row(JsonGroup::Skin, f.key, readable(f.key), v))
        })
        .collect()
}

impl AppModel {
    /// The rows set `set_id` carries, in group order, then in the order of the group's key table. Empty without an
    /// entry: a set's values live in its `[[sets]]` entry.
    pub fn json_rows(&self, set_id: &str) -> Vec<JsonRow> {
        let Some(doc) = self.sets_doc.as_ref() else {
            return vec![];
        };
        let entries = doc.entries();
        let Some(e) = entries.iter().find(|e| same_id(&e.id, set_id)) else {
            return vec![];
        };
        let mut rows = pgp_rows(e);
        rows.extend(txst_rows(e));
        rows.extend(mato_rows(e));
        rows.extend(cm_rows(e));
        rows.extend(xpmf_rows(e));
        rows.extend(skin_rows(e));
        rows
    }

    /// Whether set `set_id`'s material object follows its texture set's glint value: both blocks exist and the
    /// material object is synced for `glintParameters` (Note BM16). Decided once per target, before an apply's first
    /// write, from the blocks as they stand then.
    pub fn glint_follows(&self, set_id: &str) -> bool {
        match (
            self.cs_block(set_id, Kind::Txst),
            self.cs_block(set_id, Kind::Mato),
        ) {
            (Some(t), Some(m)) => mato_synced(Some(&t), &m, "glintParameters"),
            _ => false,
        }
    }

    /// The dotted paths a write of `row` to `target` lands on, in the document, for the hand-edit check.
    fn json_paths(&self, target: &SetView, follows: bool, row: &JsonRow) -> Vec<String> {
        let key = row.key.as_str();
        match row.group {
            JsonGroup::Pgp if target.cs_block(Kind::Txst).is_some() && pgp_mapped(key) => {
                let Some((cs, _)) = pgp_to_cs(key, None) else {
                    return vec![key.to_string()];
                };
                let mut v = vec![format!("{}.{cs}", Kind::Txst.key())];
                if follows && cs.starts_with("glintParameters.") {
                    v.push(format!("{}.{cs}", Kind::Mato.key()));
                }
                v
            }
            JsonGroup::Pgp => vec![key.to_string()],
            JsonGroup::CsTxst => vec![format!("{}.{key}", Kind::Txst.key())],
            JsonGroup::CsMato => vec![format!("{}.{key}", Kind::Mato.key())],
            JsonGroup::Cm => vec![format!("cm.{key}")],
            JsonGroup::Xpmf => vec![format!("xpmf.{key}")],
            JsonGroup::Skin => vec![format!("skin.{key}")],
        }
    }

    /// Why `target` would not take `row` (a W14 reason), or `None` when it would. `follows` is the target's glint
    /// ownership, decided once per apply (`glint_follows`).
    pub fn json_skip(
        &self,
        target: &SetView,
        follows: bool,
        row: &JsonRow,
    ) -> Option<&'static str> {
        let missing = match row.group {
            JsonGroup::Pgp => {
                // A target with a texture-set block takes a mapped key there, whatever its PGPatcher eligibility.
                if pgp_mapped(&row.key) && target.cs_block(Kind::Txst).is_some() {
                    None
                } else if !is_json_set(target, self.target) {
                    Some("writes no PGPatcher JSON")
                } else if is_pbr_grass(target) && grass_ignores(&row.key) {
                    Some("PBR grass does not use this key")
                } else {
                    None
                }
            }
            JsonGroup::CsTxst => target
                .cs_block(Kind::Txst)
                .is_none()
                .then_some("has no CS PBR texture set"),
            JsonGroup::CsMato => {
                if target.cs_block(Kind::Mato).is_none() {
                    Some("has no CS PBR material object")
                } else if follows && row.key.starts_with("glintParameters.") {
                    Some("follows its texture set")
                } else {
                    None
                }
            }
            JsonGroup::Cm => self
                .cm_block(&target.id)
                .is_none()
                .then_some("has no complex material JSON"),
            JsonGroup::Xpmf => {
                let has = self.sets_doc.as_ref().is_some_and(|d| {
                    d.entries()
                        .iter()
                        .any(|e| same_id(&e.id, &target.id) && e.xpmf.is_some())
                });
                (!has).then_some("writes no XPMF profile")
            }
            JsonGroup::Skin => {
                let diffuse = target.slots.get(&SlotKind::Diffuse);
                if target.shared {
                    Some("is a shared skin set")
                } else if matches!(diffuse, Some(SlotFill::Borrowed { .. })) && target.skin {
                    Some("borrows its diffuse")
                } else if !(matches!(diffuse, Some(SlotFill::Own { .. })) && target.skin) {
                    Some("is not an Advanced skin set")
                } else {
                    None
                }
            }
        };
        if missing.is_some() {
            return missing;
        }
        let doc = self.sets_doc.as_ref()?;
        self.json_paths(target, follows, row)
            .iter()
            .any(|p| !doc.path_open(&target.id, p))
            .then_some("has a hand-edited value in the way")
    }

    /// The active set's card name (its `short`) and its document id; with no view resolved, the selected key or id and
    /// no id.
    fn copy_source(&self) -> (String, Option<String>) {
        let rs = self.resolved_selection();
        match rs.view() {
            Some(v) => (v.short.clone(), Some(v.id.clone())),
            None => (
                match &self.selection {
                    Selection::Set(k) => k.clone(),
                    Selection::SetId(id) => id.clone(),
                    _ => String::new(),
                },
                None,
            ),
        }
    }

    /// The selected sets other than the source, in selection order.
    fn copy_targets(&self, source: Option<&str>) -> Vec<String> {
        self.selected_set_ids()
            .into_iter()
            .filter(|id| source.is_none_or(|s| !same_id(id, s)))
            .collect()
    }

    /// W1, W2, W3: the sets menus' "Copy JSON settings from {set} to {n} selected…".
    pub fn copy_json_entry(&self) -> MenuEntry {
        let (name, source) = self.copy_source();
        let n = self.copy_targets(source.as_deref()).len();
        let no_rows = source
            .as_deref()
            .is_none_or(|id| self.json_rows(id).is_empty());
        let disabled = self
            .sets_read_only_reason()
            .or_else(|| (n == 0).then(|| "select two or more sets".to_string()))
            .or_else(|| no_rows.then(|| format!("{name} has no JSON settings of its own")));
        MenuEntry {
            action: MenuAction::CopyJsonFromActive,
            label: format!("Copy JSON settings from {name} to {n} selected…"),
            disabled,
        }
    }

    /// Open the checklist with every row ticked; nothing while the entry is off.
    pub fn open_copy_set_json(&mut self) {
        if self.copy_json_entry().disabled.is_some() {
            return;
        }
        let (name, source) = self.copy_source();
        let Some(source) = source else {
            return;
        };
        let to = self.copy_targets(Some(&source));
        let rows = self
            .json_rows(&source)
            .into_iter()
            .map(|r| (r, true))
            .collect();
        self.dialog = Some(Dialog::CopySetJson {
            source: name,
            paste: false,
            to,
            rows,
        });
    }

    /// Write one row to one target (`Change::Write` in the plan). A mapped PGPatcher key goes to a texture-set block
    /// (and a glint key to the synced material object too), any other PGPatcher key to the entry converted to its
    /// range, the other groups to their block. Never `set_shared_value` (it decides glint ownership again on every
    /// write) and never `set_skin_value` (it prunes the target's unknown skin keys and rounds floats).
    fn write_json_row(&mut self, target: &SetView, follows: bool, row: &JsonRow, label: &str) {
        let id = target.id.as_str();
        let key = row.key.as_str();
        let v = row.value.clone();
        match row.group {
            JsonGroup::Pgp => {
                if target.cs_block(Kind::Txst).is_some() {
                    if let Some((cs, Some(cv))) = pgp_to_cs(key, Some(&v)) {
                        self.set_cs_value(id, Kind::Txst, cs, Some(cv.clone()));
                        if follows && cs.starts_with("glintParameters.") {
                            self.set_cs_value(id, Kind::Mato, cs, Some(cv));
                        }
                        return;
                    }
                }
                let spec = KEYS.iter().find(|s| edit_key(s.key) == key);
                let v = match spec.map(|s| s.kind) {
                    Some(KeyKind::Number(range)) if v.is_number() => clamped(&v, range),
                    _ => v,
                };
                self.set_json_value(id, key, Some(v));
            }
            JsonGroup::CsTxst => self.set_cs_value(id, Kind::Txst, key, Some(v)),
            JsonGroup::CsMato => self.set_cs_value(id, Kind::Mato, key, Some(v)),
            JsonGroup::Cm => self.set_cm_value(id, key, Some(v)),
            JsonGroup::Xpmf => self.set_xpmf_value(id, key, Some(v)),
            JsonGroup::Skin => {
                let (id, path) = (id.to_string(), format!("skin.{key}"));
                self.edit_docs(label, None, false, move |_, d| {
                    d.set_value(&id, &path, Some(v))
                });
            }
        }
    }

    /// Write `rows` to the sets `to` as one undo step, skipping what a target would not take, and report every row
    /// (W15), in Messages (W15) and in the status line (W16). `source` is the source set's card name; `paste` picks
    /// the Paste words.
    pub fn apply_copy_set_json(
        &mut self,
        source: &str,
        paste: bool,
        to: &[String],
        rows: &[JsonRow],
    ) {
        self.dialog = None;
        if let Some(why) = self.sets_read_only_reason() {
            self.status_line = why;
            return;
        }
        let label = format!(
            "{} JSON settings from {source}",
            if paste { "Paste" } else { "Copy" }
        );
        let all = views(self);
        // Each target's glint ownership is decided here, before the first write, and every glint skip and write of
        // the target uses it for the whole batch.
        let targets: Vec<(SetView, bool)> = to
            .iter()
            .filter_map(|id| all.iter().find(|v| same_id(&v.id, id)))
            .map(|v| (v.clone(), self.glint_follows(&v.id)))
            .collect();
        let plan: Vec<Vec<(String, Change)>> = rows
            .iter()
            .map(|r| {
                targets
                    .iter()
                    .map(|(t, follows)| {
                        let change = match self.json_skip(t, *follows, r) {
                            Some(why) => Change::Skip(why.to_string()),
                            None => Change::Write,
                        };
                        (t.id.clone(), change)
                    })
                    .collect()
            })
            .collect();
        self.batch_edit(&label, |m| {
            for (r, row_plan) in rows.iter().zip(&plan) {
                for ((t, follows), (_, change)) in targets.iter().zip(row_plan) {
                    if *change == Change::Write {
                        m.write_json_row(t, *follows, r, &label);
                    }
                }
            }
        });
        let written = plan
            .iter()
            .flatten()
            .filter(|(_, c)| *c == Change::Write)
            .count();
        let skipped = plan.iter().flatten().count() - written;
        let lines: Vec<(String, bool)> = rows
            .iter()
            .zip(&plan)
            .map(|(r, p)| {
                (
                    copy_report(&r.label, p),
                    p.iter().any(|(_, c)| matches!(c, Change::Skip(_))),
                )
            })
            .collect();
        for (line, with_skip) in &lines {
            if *with_skip {
                self.messages.notify(Severity::Warning, line.clone());
            } else {
                self.messages.log(Severity::Info, line.clone());
            }
        }
        self.bulk.report = Some(
            lines
                .into_iter()
                .map(|l| l.0)
                .collect::<Vec<_>>()
                .join("\n"),
        );
        self.status_line = format!("{label}: {written} written, {skipped} skipped (see Messages)");
    }
}
