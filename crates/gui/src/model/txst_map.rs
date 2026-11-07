//! Note HG: a set mapped to a texture set record (TXST) writes its outputs at the record's paths. Slots here are the
//! record's 1-based lighting slots (`Subtype::txst_slot`, `TxstMap::slots` keys), never `SlotKind` indexes.

use super::items::{ItemKind, ItemStatus, TextureItem};
use super::sets::{recipe_suffix, SetView, READ_ONLY};
use super::AppModel;
use crate::pgp::prefix::check_rel_path;
use crate::sets_doc::TxstMap;
use engine::config::Target;
use engine::sets::{same_id, OutputKey};
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
    for e in doc.entries().iter() {
        let Some(txst) = &e.txst else { continue };
        let Some(v) = views.iter().find(|v| same_id(&v.id, &e.id)) else {
            continue;
        };
        if v.pbr && m.target == Target::Cs {
            continue;
        }
        for key in &v.members {
            let Some(item) = m.item(key) else { continue };
            let Some(path) = record_slot(item).and_then(|s| txst.slots.get(&s)) else {
                continue;
            };
            let Some(rel) = record_rel(root, &item.key, path) else {
                continue;
            };
            let k = if item.kind == ItemKind::Packed {
                let Some(suffix) = recipe_suffix(item) else {
                    continue;
                };
                OutputKey::Packed {
                    set: v.id.clone(),
                    suffix,
                }
            } else {
                OutputKey::File(item.key.clone())
            };
            out.push((v.id.clone(), k, rel));
        }
    }
    out
}

/// `record_entries` without the owners: the explicit paths `engine_inputs` sends.
pub fn record_paths(m: &AppModel, views: &[SetView]) -> Vec<(OutputKey, PathBuf)> {
    record_entries(m, views)
        .into_iter()
        .map(|(_, k, p)| (k, p))
        .collect()
}

impl AppModel {
    /// Map set `set` to a texture set record. A re-plan follows; inside `batch_edit` it joins the batch.
    pub fn map_record(&mut self, set: &str, map: TxstMap) -> Result<(), String> {
        if !self.sets_editable() {
            return Err(READ_ONLY.into());
        }
        let Some(v) = self.view_of(set) else {
            return Err(format!("no set {set}"));
        };
        let (id, label) = (v.id.clone(), format!("Map {} to {}", v.id, map.edid));
        self.edit_docs(&label, None, true, move |_, d| d.set_txst(&id, Some(&map)));
        Ok(())
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
        self.edit_docs(&label, None, true, move |_, d| d.set_txst(&id, None));
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
