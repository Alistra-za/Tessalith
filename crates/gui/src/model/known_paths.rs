//! GUI tweaks G1/G5: the texture and mesh paths the mod scan knows, for the path pickers (the set editor's input
//! name and the PGPatcher JSON editor's match values). Read from the scan database on a worker after a scan and at
//! start; empty without a scan.
use super::sets::{set_views, SlotFill};
use super::AppModel;
use crate::worker::{spawn_guarded, WorkerMsg};
use modscan::store::Store;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What the scan knows. Paths are canonical: lower case, `/`, starting `textures/` or `meshes/`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KnownPaths {
    /// Texture path (with `.dds`) → the lighting slots (0-based bits) any scanned mesh uses it in.
    pub textures: BTreeMap<String, u16>,
    /// Every folder that holds a winning mesh, and its parent folders, without `meshes/` (`architecture/whiterun`).
    pub mesh_dirs: Vec<String>,
    /// Diffuse path (with `.dds`) → the slots (0-based bits, 1..=7) a live mesh's texture set or a winning TXST fills
    /// beside it. A set's missing-map warning is dropped for such a map (Note HE4).
    pub supplies: BTreeMap<String, u16>,
}

/// Read the known paths from scan database `db`.
pub fn read(db: &Path) -> Result<KnownPaths, String> {
    let store = Store::open_read_only(db).map_err(|e| e.to_string())?;
    let conn = store.conn();
    let mut textures: BTreeMap<String, u16> = BTreeMap::new();
    for sql in [
        // Lighting slots of enabled, non-derived copies only (trace-debug #42): not an effect shape's greyscale.
        format!(
            "SELECT DISTINCT p.p, tu.slot FROM tex_use tu JOIN path p ON p.id = tu.tex_path_id \
             JOIN texset ts ON ts.id = tu.texset_id WHERE tu.slot < 16 AND {}",
            super::used_by::LIVE_LIGHTING_TEXSET
        ),
        // Note HC6: the slots of each winning TXST whose master is loaded, as `txsts_matching` reads them.
        "SELECT DISTINCT p.p, ts.slot FROM txst_slot ts JOIN path p ON p.id = ts.path_id \
         JOIN rec_winner w ON w.formid = ts.formid AND w.plugin_id = ts.plugin_id \
         JOIN rec r ON r.plugin_id = ts.plugin_id AND r.local = ts.local AND r.owner_ok = 1 \
         WHERE ts.slot < 16"
            .to_string(),
    ] {
        let mut st = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (p, slot) = row.map_err(|e| e.to_string())?;
            if p.starts_with("textures/") {
                *textures.entry(p).or_default() |= 1 << slot;
            }
        }
    }
    // Only the missing-map warnings depend on these; a failing query must not blank the pickers (final review K1).
    let supplies = (|| -> Result<BTreeMap<String, u16>, String> {
    let mut supplies: BTreeMap<String, u16> = BTreeMap::new();
    for sql in [
        // A mesh's texture set: bit N when its slot N is filled. Not a self-join of `tex_use` (no index on texset_id).
        format!(
            "SELECT d.p, (ts.t1 IS NOT NULL) * 2 + (ts.t2 IS NOT NULL) * 4 + (ts.t3 IS NOT NULL) * 8 \
             + (ts.t4 IS NOT NULL) * 16 + (ts.t5 IS NOT NULL) * 32 + (ts.t6 IS NOT NULL) * 64 \
             + (ts.t7 IS NOT NULL) * 128 FROM texset ts JOIN path d ON d.id = ts.t0 WHERE {}",
            super::used_by::LIVE_LIGHTING_TEXSET
        ),
        // A winning TXST (HC6's conditions on its diffuse row): the other slots of the same record.
        "SELECT d.p, 1 << o.slot FROM txst_slot d0 JOIN path d ON d.id = d0.path_id \
         JOIN rec_winner w ON w.formid = d0.formid AND w.plugin_id = d0.plugin_id \
         JOIN rec r ON r.plugin_id = d0.plugin_id AND r.local = d0.local AND r.owner_ok = 1 \
         JOIN txst_slot o ON o.plugin_id = d0.plugin_id AND o.local = d0.local \
         WHERE d0.slot = 0 AND o.slot BETWEEN 1 AND 7"
            .to_string(),
    ] {
        let mut st = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .map_err(|e| e.to_string())?;
        for row in rows {
            let (p, bits) = row.map_err(|e| e.to_string())?;
            if p.starts_with("textures/") && bits != 0 {
                *supplies.entry(p).or_default() |= bits as u16;
            }
        }
    }
    Ok(supplies)
    })()
    .unwrap_or_default();
    let mut dirs: BTreeSet<String> = BTreeSet::new();
    let mut st = conn
        .prepare("SELECT p.p FROM nif_winner w JOIN path p ON p.id = w.path_id")
        .map_err(|e| e.to_string())?;
    let rows = st
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    for row in rows {
        let p = row.map_err(|e| e.to_string())?;
        let p = p.strip_prefix("meshes/").unwrap_or(&p);
        let mut cur = p;
        while let Some(i) = cur.rfind('/') {
            cur = &cur[..i];
            if !dirs.insert(cur.to_string()) {
                break;
            }
        }
    }
    Ok(KnownPaths {
        textures,
        mesh_dirs: dirs.into_iter().collect(),
        supplies,
    })
}

/// `p` without a `.dds` ending.
pub fn strip_dds(p: &str) -> &str {
    match p.len().checked_sub(4) {
        Some(c) if p.is_char_boundary(c) && p[c..].eq_ignore_ascii_case(".dds") => &p[..c],
        _ => p,
    }
}

/// The first `limit` of `list` that contain every word of `query` (case-insensitive, `\` read as `/`).
pub fn filter<'a>(list: &'a [String], query: &str, limit: usize) -> Vec<&'a str> {
    let q = query.trim().replace('\\', "/").to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    list.iter()
        .map(String::as_str)
        .filter(|p| {
            let l = p.to_lowercase();
            words.iter().all(|w| l.contains(w))
        })
        .take(limit)
        .collect()
}

impl AppModel {
    /// Read the known paths again from the scan database, on a worker (at start and after a scan).
    pub fn refresh_known_paths(&mut self) {
        let Some(db) = self.scan_db.clone().filter(|d| d.is_file()) else {
            return;
        };
        spawn_guarded(self.tx.clone(), "known-paths", move |tx| {
            let result = std::panic::catch_unwind(|| read(&db))
                .unwrap_or_else(|p| Err(format!("panic: {}", crate::worker::panic_text(&*p))));
            let _ = tx.send(WorkerMsg::KnownPaths(result));
        });
    }

    pub(super) fn known_paths_landed(&mut self, result: Result<KnownPaths, String>) {
        match result {
            Ok(k) => {
                self.scan_db_error = None;
                self.set_known(k);
            }
            Err(e) => self.scan_db_warning(format!(
                "Scan database: reading its texture paths failed: {e}"
            )),
        }
    }

    /// A scan-database read failed: a warning in Messages (the GUI has no console), once per distinct error.
    pub(super) fn scan_db_warning(&mut self, text: String) {
        if self.scan_db_error.as_ref() != Some(&text) {
            self.messages
                .log(super::messages::Severity::Warning, text.clone());
            self.scan_db_error = Some(text);
        }
    }

    /// Texture paths for a picker, without `.dds`: the scan's paths used in lighting slot `slot` (0-based; `None`
    /// for any slot), then this project's own outputs' game paths without their source extension. Sorted, no
    /// duplicates.
    pub fn known_textures(&self, slot: Option<u8>) -> Vec<String> {
        let mut out: BTreeSet<String> = self
            .known
            .textures
            .iter()
            .filter(|(_, bits)| slot.is_none_or(|s| *bits & (1 << s) != 0))
            .map(|(p, _)| strip_dds(p).to_string())
            .collect();
        for v in set_views(self) {
            for (kind, fill) in &v.slots {
                if slot.is_some_and(|s| Some(s + 1) != kind.game_slot()) {
                    continue;
                }
                if let SlotFill::Own { item } = fill {
                    if let Some(j) = self.item(item).and_then(|i| i.job.as_ref()) {
                        if j.game_path.starts_with("textures/") {
                            // Wave 2 #5: an output row has no source extension (`leaf.png` → `leaf`).
                            out.insert(super::input_name::no_ext(&j.game_path).to_string());
                        }
                    }
                }
            }
        }
        out.into_iter().collect()
    }

    /// Mesh folders for a `nif_filter` picker (`architecture/whiterun`).
    pub fn known_mesh_dirs(&self) -> &[String] {
        &self.known.mesh_dirs
    }
}
