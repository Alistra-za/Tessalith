//! Note HG: a set mapped to a texture set record (TXST) writes its outputs at the record's paths. Slots here are the
//! record's 1-based lighting slots (`Subtype::txst_slot`, `TxstMap::slots` keys), never `SlotKind` indexes.

use super::items::{ItemKind, ItemStatus, TextureItem};
use super::known_paths::TxstRecord;
use super::sets::{
    entry_of, map_slot, recipe_suffix, renamed_output, DefaultOutputs, SetView, SlotFill, SlotKind,
    LAST_GAME_SLOT, READ_ONLY,
};
use super::target_mod::{FileBase, MeshPath};
use super::AppModel;
use crate::pgp::prefix::check_rel_path;
use crate::sets_doc::{SetEntry, TxstMap};
use engine::codes::Code;
use engine::config::{Subtype, Target};
use engine::sets::{same_id, OutputKey};
use engine::shader::Shader;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

/// Where an output of the file `item_key` (relative to the input `root`) goes under the export folder so that its game
/// path is `record`, by `engine::discover::game_path`'s rule: the first `textures` folder below the root, else the
/// last one in the root's own path. `None` when `record` is not a `textures/...dds` path that stays inside the export
/// folder, or lies outside the root's own `textures` folder.
///
/// The record's spelling is normalised (agent ruling, Note HG): lower-case, `/`, empty and `.` components dropped,
/// so `textures/s/./a.dds` and `textures/s//a.dds` name the same output as `textures/s/a.dds`; `..` is refused.
// shortcut: containment is by path components only, so a junction the user made inside the export folder is followed
// as it is for every other output; upgrade when the engine's write boundary resolves real paths.
pub fn record_rel(root: &Path, item_key: &str, record: &str) -> Option<PathBuf> {
    let record = record.replace('\\', "/");
    if record.starts_with('/') {
        return None;
    }
    let parts: Vec<String> = record
        .split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .map(str::to_ascii_lowercase)
        .collect();
    let rec = parts.join("/");
    if parts.first().map(String::as_str) != Some("textures")
        || !rec.ends_with(".dds")
        || check_rel_path(&rec, "record path").is_err()
    {
        return None;
    }
    let key = item_key.replace('\\', "/");
    let dirs: Vec<&str> = key.split('/').collect();
    let dirs = &dirs[..dirs.len() - 1];
    let rel = if let Some(i) = dirs.iter().position(|c| c.eq_ignore_ascii_case("textures")) {
        let before = dirs[..i].join("/");
        if before.is_empty() {
            rec
        } else {
            format!("{before}/{rec}")
        }
    } else {
        let comps: Vec<String> = root
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
            .collect();
        match comps.iter().rposition(|c| c == "textures") {
            Some(i) => {
                let prefix = format!("{}/", comps[i..].join("/"));
                rec.strip_prefix(&prefix)?.to_string()
            }
            None => rec,
        }
    };
    // The result is checked too: stripping a prefix must never leave a rooted or empty path.
    (!rel.starts_with('/') && check_rel_path(&rel, "record path").is_ok())
        .then(|| PathBuf::from(rel))
}

/// The record slot (1-based) a file fills: none for a raw packing source or a helper, as `slot_of` is; unlike
/// `slot_kind` it covers the env mask, complex material and `msn_specular`.
pub fn record_slot(item: &TextureItem) -> Option<u8> {
    if matches!(item.status, ItemStatus::PackedInto(_)) || item.kind == ItemKind::Helper {
        return None;
    }
    item.job.as_ref()?.profile.subtype.txst_slot()
}

/// Every output of a mapped set that its record names: (set id, output key, path under the export folder). A PBR set
/// on CS is left out (its prefix names its outputs), and so is a record path `record_rel` refuses.
pub fn record_entries(m: &AppModel, views: &[SetView]) -> Vec<(String, OutputKey, PathBuf)> {
    let (Some(doc), Some(root)) = (m.sets_doc.as_ref(), m.root.as_deref()) else {
        return vec![];
    };
    let mut out = vec![];
    let entries = doc.entries();
    // Once per pass, and only when a base mapping needs the files' default names.
    let defaults = entries
        .iter()
        .any(|e| e.txst.as_ref().is_some_and(TxstMap::is_base))
        .then(|| DefaultOutputs::new(m))
        .flatten();
    for e in entries.iter() {
        let Some(txst) = &e.txst else { continue };
        let Some(v) = views.iter().find(|v| same_id(&v.id, &e.id)) else {
            continue;
        };
        if m.record_ignored(v) {
            continue;
        }
        let base = txst.base.as_deref().zip(defaults.as_ref());
        // `DefaultOutputs::new` is `None` only without a root or export root; then no output is planned at all.
        if txst.is_base() && base.is_none() {
            continue;
        }
        for key in &v.members {
            let Some(item) = m.item(key) else { continue };
            if let Some((k, _, rel)) = member_write(root, v, item, &txst.slots, base) {
                // Note HQ6: explicit paths are under the export folder.
                out.push((v.id.clone(), k, m.export_rel(&rel)));
            }
        }
    }
    out
}

/// R1, the one rule for "this member of set `v`, under a record with `slots`, writes this path": the output key, the
/// record's path as stored, and where it goes under the export folder. `None` when the file keeps its own path: the
/// record names no slot for it, it has no job (a kept DDS), a packing source or a helper, or `record_rel` refuses the
/// path. `record_entries` (the plan) and `record_preview` (the picker) both call it.
///
/// Note HL3: under a base mapping (`base`: the base and the files' default names) a slot the source lacks goes to
/// the base's folder, named by the file's default name with the base's stem (`renamed_output`).
fn member_write(
    root: &Path,
    v: &SetView,
    item: &TextureItem,
    slots: &BTreeMap<u8, String>,
    base: Option<(&str, &DefaultOutputs)>,
) -> Option<(OutputKey, String, PathBuf)> {
    let slot = record_slot(item)?;
    let path = match (slots.get(&slot), base) {
        (Some(p), _) => p.clone(),
        (None, Some((base, defaults))) => {
            let (dir, stem) = base.rsplit_once('/')?;
            let named = renamed_output(&defaults.key(item)?, stem, v.tails())?;
            format!("{dir}/{}", named.file_name()?.to_string_lossy())
        }
        (None, None) => return None,
    };
    let rel = record_rel(root, &item.key, &path)?;
    let k = if item.kind == ItemKind::Packed {
        OutputKey::Packed {
            set: v.id.clone(),
            suffix: recipe_suffix(item)?,
        }
    } else {
        OutputKey::File(item.key.clone())
    };
    Some((k, path, rel))
}

/// `record_entries` without the owners: the explicit paths `engine_inputs` sends.
pub fn record_paths(m: &AppModel, views: &[SetView]) -> Vec<(OutputKey, PathBuf)> {
    record_entries(m, views)
        .into_iter()
        .map(|(_, k, p)| (k, p))
        .collect()
}

/// Whether a set holding `slots` is a PGPatcher PBR set on CS, whose mapping is ignored (`record_entries`). `finish_view`
/// has not run yet, so this reads the diffuse's subtype.
fn pbr_on_cs(m: &AppModel, slots: &BTreeMap<SlotKind, SlotFill>) -> bool {
    m.target == Target::Cs
        && slots
            .get(&SlotKind::Diffuse)
            .and_then(SlotFill::item)
            .and_then(|k| m.item(k))
            .and_then(|i| i.job.as_ref())
            .is_some_and(|j| j.profile.subtype == Subtype::PbrAlbedo)
}

/// Where each set's own files are written: game path (`game_key`) → (set id, item key). A
/// file in a slot its set's record names is at the record's path, else at its last plan's `job_key`. The first writer
/// in display order keeps a path two sets write (that is the duplicate row's business).
fn written_index<'a>(
    m: &AppModel,
    entries: &[SetEntry],
    sets: impl Iterator<Item = (&'a str, &'a BTreeMap<SlotKind, SlotFill>)>,
) -> HashMap<String, (String, String)> {
    let mut out = HashMap::new();
    for (id, slots) in sets {
        let txst = entry_of(entries, id)
            .and_then(|e| e.txst.as_ref())
            .filter(|_| !pbr_on_cs(m, slots));
        for item in slots.values().filter_map(|f| match f {
            SlotFill::Own { item } => Some(item),
            _ => None,
        }) {
            let Some(it) = m.item(item) else { continue };
            let rel = txst
                .zip(m.root.as_deref())
                .and_then(|(t, root)| {
                    let path = t.slots.get(&record_slot(it)?)?;
                    record_rel(root, &it.key, path)
                })
                .map(|p| game_key(&p.to_string_lossy()))
                .or_else(|| (!it.job_key.is_empty()).then(|| game_key(&it.job_key)));
            if let Some(rel) = rel {
                out.entry(rel).or_insert((id.to_string(), item.clone()));
            }
        }
    }
    out
}

/// A key to resolve `record_rel` against for set `v`: its first file that is not a packed output (the rule reads the
/// folder it sits in).
fn key_of(m: &AppModel, v: &SetView) -> String {
    v.members
        .iter()
        .find(|k| m.item(k).is_some_and(|i| i.kind != ItemKind::Packed))
        .cloned()
        .unwrap_or_default()
}

/// The path `record` names for set `v`, as `written_index` spells it.
fn record_key(m: &AppModel, v: &SetView, record: &str) -> Option<String> {
    let rel = record_rel(m.root.as_deref()?, &key_of(m, v), record)?;
    Some(game_key(&rel.to_string_lossy()))
}

/// Agent ruling (Note HG): the index compares game paths, as R2 says. `rel` is a path under the export folder; this
/// folds it (`/`, ASCII lower case) and cuts everything before its first `textures` folder, so a host in another mod
/// folder (`mod_a/textures/s/x_n.dds`) matches the consumer's record path resolved under its own
/// (`mod_b/textures/s/x_n.dds`). With no `textures` folder (the root is at or inside it) every rel has the same shape
/// and stays whole.
fn game_key(rel: &str) -> String {
    let rel = rel.replace('\\', "/").to_ascii_lowercase();
    match rel.split('/').position(|c| c == "textures") {
        Some(i) => rel.split('/').skip(i).collect::<Vec<_>>().join("/"),
        None => rel,
    }
}

/// Note HG: empty record slots of each mapped set borrow from the set that writes that path, or count as supplied.
/// Runs after `entry_view`, so an explicit borrow, a `slotN` and a pin are already there to win. `own_slots` are the
/// sets' own files by id, as the plan has them (before any borrow).
pub(super) fn fill_from_records(
    m: &AppModel,
    views: &mut [SetView],
    entries: &[SetEntry],
    own_slots: &[(String, BTreeMap<SlotKind, SlotFill>)],
) {
    if !entries.iter().any(|e| e.txst.is_some()) {
        return;
    }
    let written = written_index(m, entries, own_slots.iter().map(|(i, s)| (i.as_str(), s)));
    let host_slots = |id: &str| {
        own_slots
            .iter()
            .find(|(i, _)| same_id(i, id))
            .map(|(_, s)| s)
    };
    for v in views.iter_mut() {
        let Some(e) = entry_of(entries, &v.id) else {
            continue;
        };
        let Some(txst) = &e.txst else { continue };
        if pbr_on_cs(m, &v.slots) {
            continue;
        }
        // A slot another source already decided: its own file, an explicit borrow, a pin, or a `slotN` setting.
        let decided = |v: &SetView, n: u8| {
            v.slots.keys().any(|k| k.slot_number() == Some(n))
                || e.borrow.contains_key(&n)
                || v.locks.contains(&n)
                || (n <= LAST_GAME_SLOT && v.settings.contains_key(&format!("slot{n}")))
        };
        for (&n, path) in txst.slots.iter().filter(|(n, _)| **n >= 2) {
            if decided(v, n) {
                continue;
            }
            let writer = record_key(m, v, path)
                .and_then(|k| written.get(&k))
                .filter(|(w, _)| !same_id(w, &v.id));
            if e.no_fill.contains(&n) {
                let host = writer.map_or_else(String::new, |(w, _)| w.clone());
                v.fill_off.insert(n, host);
                continue;
            }
            let kind = writer.and_then(|(w, item)| {
                host_slots(w)?
                    .iter()
                    .find(|(k, f)| {
                        k.slot_number() == Some(n)
                            && matches!(f, SlotFill::Own { item: i } if i == item)
                    })
                    .map(|(k, _)| (*k, w.clone(), item.clone()))
            });
            match kind {
                Some((kind, from, item)) => {
                    v.filled.insert(n, from.clone());
                    v.slots.insert(kind, SlotFill::Borrowed { from, item });
                }
                None => {
                    v.record_supplied.insert(n, path.clone());
                }
            }
        }
        wet_with_the_normal(v, e, &host_slots);
    }
    for v in views.iter_mut() {
        let (filled, supplied) = (&v.filled, &v.record_supplied);
        v.set_warnings.retain(|w| {
            let n = (w.code == Code::ShaderMissingMap)
                .then(|| w.data["map"].as_str().and_then(map_slot))
                .flatten();
            !n.is_some_and(|n| filled.contains_key(&n) || supplied.contains_key(&n))
        });
    }
}

/// R8: a skin set whose normal came from a shared-map set `H` through its record also takes `H`'s `_wet` and
/// `_detail_n`, when `H` has them. Its own, an explicit borrow and a pin win, as for any fill.
fn wet_with_the_normal<'a>(
    v: &mut SetView,
    e: &SetEntry,
    host_slots: &impl Fn(&str) -> Option<&'a BTreeMap<SlotKind, SlotFill>>,
) {
    let Some(host) = v.filled.get(&2).cloned() else {
        return;
    };
    let skin = v
        .shader
        .as_ref()
        .is_some_and(|p| p.shader == Shader::AdvancedSkin)
        && v.slots.contains_key(&SlotKind::Diffuse);
    let Some(hs) = host_slots(&host).filter(|hs| skin && !hs.contains_key(&SlotKind::Diffuse))
    else {
        return;
    };
    for kind in [SlotKind::SkinWet, SlotKind::SkinDetail] {
        let Some(n) = kind.slot_number() else {
            continue;
        };
        let Some(SlotFill::Own { item }) = hs.get(&kind) else {
            continue;
        };
        if v.slots.contains_key(&kind) || e.borrow.contains_key(&n) || v.locks.contains(&n) {
            continue;
        }
        if e.no_fill.contains(&n) {
            v.fill_off.insert(n, host.clone());
            continue;
        }
        v.filled.insert(n, host.clone());
        v.slots.insert(
            kind,
            SlotFill::Borrowed {
                from: host.clone(),
                item: item.clone(),
            },
        );
    }
}

/// R7: the ids of the sets mapped to a record, with no `shader` of their own and members that pass the engine's
/// shared member rule ([`engine::group::shared_member_tails`]), whose record's colour path another set, a skin one,
/// writes. `set_shaders` marks them Advanced skin. The rule reads member names only, so it is the same in every plan;
/// `w.skin` is the last plan's, so this settles through the `EngineInputs` compare.
pub(super) fn record_shared_sets(m: &AppModel, views: &[SetView]) -> Vec<String> {
    let Some(doc) = m.sets_doc.as_ref() else {
        return vec![];
    };
    let entries = doc.entries();
    if !entries.iter().any(|e| e.txst.is_some()) {
        return vec![];
    }
    entries
        .iter()
        .filter(|e| e.shader.is_none())
        .filter_map(|e| {
            let path = e.txst.as_ref()?.slots.get(&1)?;
            let v = views.iter().find(|v| same_id(&v.id, &e.id))?;
            // The engine's members are its input files, read by their `[inputs]` alias.
            let stems: Vec<String> = v
                .members
                .iter()
                .filter(|k| m.item(k).is_some_and(|i| i.kind != ItemKind::Packed))
                .map(|k| {
                    let rel = m.read_as_of(k).unwrap_or_else(|| k.clone());
                    let stem = Path::new(&rel).file_stem().unwrap_or_default();
                    stem.to_string_lossy().into_owned()
                })
                .collect();
            engine::group::shared_member_tails(&stems, &m.suffix_aliases)?;
            // Until the set is skin the plan may write a hand-added `x_wet` at the record's slot 1, so the writer to
            // find is another set's.
            let others = views
                .iter()
                .filter(|w| !same_id(&w.id, &v.id))
                .map(|w| (w.id.as_str(), &w.slots));
            let written = written_index(m, &entries, others);
            let (writer, _) = written.get(&record_key(m, v, path)?)?;
            let w = views.iter().find(|w| same_id(&w.id, writer))?;
            w.skin.then(|| e.id.clone())
        })
        .collect()
}

/// Whether two stored FormIDs are one: compared as hex numbers, `0x` optional, in any case. One that doesn't parse
/// matches only its own text.
fn formid_eq(a: &str, b: &str) -> bool {
    let number = |s: &str| {
        let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
        u32::from_str_radix(hex.unwrap_or(s), 16).ok()
    };
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a.eq_ignore_ascii_case(b),
    }
}

/// The FormID a mapping stores and the picker shows: the plugin's own `local` without its master digits, `0x`, upper
/// case, no leading zeros (`05000814` is `0x814`). Owner, 2026-11-06. Never from the load-order FormID: a light
/// plugin's `FE4CD814` would keep `4CD`, which moves with the load order.
pub fn formid_text(local: u32) -> String {
    format!("0x{:X}", local & 0x00FF_FFFF)
}

/// The plugin that defines the record whose own copy has `local` in plugin `own`: its master at the index the first
/// two digits hold, or `own` when the index is at or past `masters.len()` (`clamp_local`).
pub fn defining_plugin(local: u32, own: &str, masters: &[String]) -> String {
    masters
        .get((local >> 24) as usize)
        .map_or_else(|| own.to_string(), Clone::clone)
}

/// The records of `list` that contain every word of `query` (case-insensitive, `\` read as `/`) in their EDID, plugin,
/// winning plugin, FormID or colour path.
pub fn filter_records<'a>(list: &'a [TxstRecord], query: &str) -> Vec<&'a TxstRecord> {
    record_hits(list, query)
        .into_iter()
        .map(|i| &list[i])
        .collect()
}

/// [`filter_records`] as indexes into `list`, for a picker that keeps its hits between frames.
pub fn record_hits(list: &[TxstRecord], query: &str) -> Vec<usize> {
    hits(list, query, record_hay)
}

/// [`record_hits`] over the Map to target… rows: a record by the same fields, a mesh path or file base by its base.
pub fn pick_hits(rows: &[TargetPick], query: &str) -> Vec<usize> {
    hits(rows, query, |p| match p {
        TargetPick::Record(r) => record_hay(r),
        TargetPick::Mesh(m) => m.base.clone(),
        TargetPick::Files(f) => f.base.clone(),
    })
}

fn record_hay(r: &TxstRecord) -> String {
    format!(
        "{}\n{}\n{}\n{}\n{}",
        r.edid,
        r.plugin,
        r.winner,
        r.formid,
        r.slots.get(&1).map_or("", String::as_str)
    )
}

fn hits<T>(list: &[T], query: &str, hay: impl Fn(&T) -> String) -> Vec<usize> {
    let q = query.trim().replace('\\', "/").to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    (0..list.len())
        .filter(|&i| {
            let hay = hay(&list[i]).to_lowercase();
            words.iter().all(|w| hay.contains(w))
        })
        .collect()
}

impl TxstRecord {
    /// What a mapping stores for this record.
    pub fn to_map(&self) -> TxstMap {
        TxstMap {
            plugin: self.plugin.clone(),
            formid: self.formid.clone(),
            edid: self.edid.clone(),
            slots: self.slots.clone(),
            base: None,
        }
    }
}

/// What a set can be mapped to (Note HL): a target mod's record, mesh path or file base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetPick {
    Record(TxstRecord),
    Mesh(MeshPath),
    Files(FileBase),
}

impl AppModel {
    /// The winning record `map` names, by plugin (ASCII-folded) + FormID (`formid_eq`), not by EDID or by the winning
    /// plugin.
    pub fn record_of(&self, map: &TxstMap) -> Option<&TxstRecord> {
        self.known.records.iter().find(|r| {
            r.plugin.eq_ignore_ascii_case(&map.plugin) && formid_eq(&r.formid, &map.formid)
        })
    }

    /// Whether the plan ignores `v`'s mapping: a PGPatcher PBR set on Community Shaders names its outputs in the JSON
    /// (`record_entries`, the menu, the picker and the record rows all ask this).
    pub fn record_ignored(&self, v: &SetView) -> bool {
        v.pbr && self.target == Target::Cs
    }

    /// What mapping set `set` to `r` would do, for the picker: each file the set writes (its name) and the record path
    /// it would write (`None`: it keeps its own). It asks `member_write`, the rule `record_entries` plans with, so a
    /// packed output is listed and a kept DDS with no job is not. Raw packing sources and helpers write nothing and
    /// are left out. A path another set also writes is a collision row after mapping, not predicted here.
    pub fn record_preview(&self, set: &str, r: &TxstRecord) -> Vec<(String, Option<String>)> {
        let (Some(v), Some(root)) = (self.view_of(set), self.root.as_deref()) else {
            return vec![];
        };
        let off = self.record_ignored(&v);
        v.members
            .iter()
            .filter_map(|k| self.item(k))
            .filter(|i| {
                !matches!(i.status, ItemStatus::PackedInto(_)) && i.kind != ItemKind::Helper
            })
            .map(|i| {
                let path = member_write(root, &v, i, &r.slots, None)
                    .filter(|_| !off)
                    .map(|(_, p, _)| p);
                (i.name.clone(), path)
            })
            .collect()
    }

    /// "Use new paths": copy the winning record's current paths and EDID into set `set`'s mapping.
    pub fn use_record_paths(&mut self, set: &str) -> Result<(), String> {
        let Some(id) = self.view_of(set).map(|v| v.id) else {
            return Err(format!("no set {set}"));
        };
        let stored = self
            .sets_doc
            .as_ref()
            .and_then(|d| {
                d.entries()
                    .iter()
                    .find(|e| same_id(&e.id, &id))
                    .and_then(|e| e.txst.clone())
            })
            .ok_or_else(|| format!("{id} is not mapped to a texture set record"))?;
        let map = self
            .record_of(&stored)
            .ok_or_else(|| format!("{} isn't in the latest scan", stored.edid))?
            .to_map();
        self.map_record(&id, map)
    }

    /// Map set `id` to a record, mesh path or file base of the target mod; it replaces any other mapping.
    pub fn map_target(&mut self, id: &str, pick: TargetPick) -> Result<(), String> {
        let (base, slots) = match pick {
            TargetPick::Record(r) => return self.map_record(id, r.to_map()),
            TargetPick::Mesh(p) => (p.base, p.slots),
            TargetPick::Files(f) => (f.base, f.slots),
        };
        let base = Some(base);
        self.map_record(
            id,
            TxstMap {
                base,
                slots,
                ..TxstMap::default()
            },
        )
    }

    /// The written output paths of set `v` (`game_key`: lower case, from `textures`) that the target mod already
    /// ships as texture files. Empty without a target mod.
    pub fn target_replaces(&self, v: &SetView) -> Vec<String> {
        let (Some(t), Some(root)) = (self.target(), self.root.as_deref()) else {
            return vec![];
        };
        let txst = self.stored_txst(&v.id).filter(|_| !self.record_ignored(v));
        let defaults = txst
            .as_ref()
            .is_some_and(TxstMap::is_base)
            .then(|| DefaultOutputs::new(self))
            .flatten();
        let mut out: Vec<String> = v
            .members
            .iter()
            .filter_map(|k| self.item(k))
            .filter_map(|item| {
                let mapped = txst.as_ref().and_then(|t| {
                    let base = t.base.as_deref().zip(defaults.as_ref());
                    member_write(root, v, item, &t.slots, base).map(|(_, _, rel)| rel)
                });
                match mapped {
                    Some(rel) => Some(game_key(&rel.to_string_lossy())),
                    None => (item.job.is_some() && !item.job_key.is_empty())
                        .then(|| game_key(&item.job_key)),
                }
            })
            .filter(|p| t.files.iter().any(|f| f.slots.values().any(|s| s == p)))
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Map set `set` to a texture set record. A re-plan follows; inside `batch_edit` it joins the batch.
    pub fn map_record(&mut self, set: &str, map: TxstMap) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let Some(v) = self.view_of(set) else {
            return Err(format!("no set {set}"));
        };
        let (id, label) = (
            v.id.clone(),
            format!("Map {} to {}", v.id, map.base.as_ref().unwrap_or(&map.edid)),
        );
        // A base the set already has, in other case, is no new mapping (Note HL3).
        let stored = self.stored_txst(&id);
        if let (Some(t), Some(b)) = (&stored, &map.base) {
            let fold = |a: &BTreeMap<u8, String>, b: &BTreeMap<u8, String>| {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|((n, p), (m, q))| n == m && p.eq_ignore_ascii_case(q))
            };
            if t.base.as_ref().is_some_and(|s| s.eq_ignore_ascii_case(b))
                && fold(&t.slots, &map.slots)
            {
                return Ok(());
            }
        }
        // Another record starts with every fill on: a slot turned off under one record says nothing about another's.
        // The same key (plugin ASCII-folded + FormID; "Use new paths", Change record to its own) keeps them.
        let off = stored.map_or_else(
            || self.stored_no_fill(&id),
            |t| {
                let same = match (&t.base, &map.base) {
                    (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                    (None, None) => {
                        t.plugin.eq_ignore_ascii_case(&map.plugin)
                            && formid_eq(&t.formid, &map.formid)
                    }
                    _ => false,
                };
                if same {
                    vec![]
                } else {
                    self.stored_no_fill(&id)
                }
            },
        );
        self.edit_docs(&label, None, true, move |_, d| {
            d.set_txst(&id, Some(&map));
            for n in off {
                d.set_no_fill(&id, n, false);
            }
        });
        Ok(())
    }

    fn stored_txst(&self, id: &str) -> Option<TxstMap> {
        self.sets_doc
            .as_ref()?
            .entries()
            .iter()
            .find(|e| same_id(&e.id, id))?
            .txst
            .clone()
    }

    fn stored_no_fill(&self, id: &str) -> Vec<u8> {
        self.sets_doc
            .as_ref()
            .and_then(|d| {
                d.entries()
                    .iter()
                    .find(|e| same_id(&e.id, id))
                    .map(|e| e.no_fill.iter().copied().collect())
            })
            .unwrap_or_default()
    }

    /// Remove set `set`'s record mapping; its outputs go back to their default names.
    pub fn unmap_record(&mut self, set: &str) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let Some(v) = self.view_of(set) else {
            return Err(format!("no set {set}"));
        };
        let (id, label) = (v.id.clone(), format!("Remove record mapping from {}", v.id));
        let off = self.stored_no_fill(&id);
        // `no_fill` first: `set_txst(None)` removes a table left with only its id, and a later edit would make it again.
        self.edit_docs(&label, None, true, move |_, d| {
            for n in off {
                d.set_no_fill(&id, n, false);
            }
            d.set_txst(&id, None);
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(root: &str, key: &str, record: &str) -> Option<String> {
        record_rel(Path::new(root), key, record).map(|p| p.to_string_lossy().replace('\\', "/"))
    }

    #[test]
    fn the_record_rel_rule() {
        let r = "textures/b/y.dds";
        assert_eq!(rel("C:/p", "textures/a/x.png", r).as_deref(), Some(r));
        assert_eq!(
            rel("C:/p", "mod/textures/a/x.png", r).as_deref(),
            Some("mod/textures/b/y.dds")
        );
        assert_eq!(rel("C:/p", "a/x.png", r).as_deref(), Some(r));
        let at = "C:/d/textures/a";
        assert_eq!(
            rel(at, "x.png", "Textures\\A\\b\\y.dds").as_deref(),
            Some("b/y.dds")
        );
        assert_eq!(rel(at, "x.png", "textures/z/y.dds"), None);
        // Spelling is normalised, not refused.
        for same in [
            "textures/b/./y.dds",
            "textures//b//y.dds",
            "Textures\\B\\.\\y.dds",
        ] {
            assert_eq!(rel("C:/p", "x.png", same).as_deref(), Some(r), "{same}");
        }
        assert_eq!(
            rel("C:/d/textures", "x.png", "textures//y.dds").as_deref(),
            Some("y.dds")
        );
        for bad in [
            "/textures/y.dds",
            "meshes/y.dds",
            "textures/y.png",
            "textures/../y.dds",
            "textures/a:/y.dds",
        ] {
            assert_eq!(rel("C:/p", "x.png", bad), None, "{bad}");
        }
    }

    /// The rule agrees with `engine::discover::game_path`: a file placed at the returned rel under the root reads
    /// back as the record.
    #[test]
    fn the_rule_agrees_with_discover() {
        let record = "textures/a/b/y.dds";
        for (root, key) in [
            ("C:/p", "textures/a/x.png"),
            ("C:/p", "mod/textures/a/x.png"),
            ("C:/d/textures/a", "x.png"),
        ] {
            let rel = record_rel(Path::new(root), key, record).unwrap();
            let abs = Path::new(root).join(&rel);
            assert_eq!(engine::discover::game_path(&abs, &rel), record, "{root}");
        }
    }
}
