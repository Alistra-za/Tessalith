//! Note HL: the target mod, a mod folder the modder is working on. Tessalith only reads it: its plugins, the texture
//! paths its meshes name and the texture files it holds. The folder is a `[project] target_mod` setting of
//! `sdc-project.toml` (GUI-only). Nothing is ever written into it.
use super::known_paths::{strip_dds, TxstRecord};
use super::AppModel;
use crate::worker::{spawn_guarded, WorkerMsg};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Shown where records are wanted but the target mod's plugins are not in the MO2 scan.
pub const RECORDS_NOT_IN_SCAN: &str = "Records come from the MO2 scan; this mod isn't in it";
/// Refused: the target mod and the export folder are one folder, or one holds the other.
pub const TARGET_IS_EXPORT: &str =
    "The target mod can't be the export folder, sit inside it or hold it";

/// Everything read from the target mod. Paths are lower case, use `/` and start `textures/`; a slot path ends
/// `.dds`, a `base` doesn't (the form of `TxstMap.slots`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetMod {
    pub root: PathBuf,
    /// The root-level `.esp`, `.esm` and `.esl` names, lower case.
    pub plugins: BTreeSet<String>,
    pub meshes: Vec<MeshPath>,
    pub files: Vec<FileBase>,
    /// One line per file that could not be read; the rest is still listed.
    pub problems: Vec<String>,
}

/// One distinct slot-1 path of the target's meshes, with the other slots of the first shape that names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshPath {
    /// Slot 1 without `.dds`.
    pub base: String,
    /// 1-based slot to path.
    pub slots: BTreeMap<u8, String>,
    /// How many meshes name this slot-1 path.
    pub meshes: u32,
}

/// The texture files of the target that share a base name, by the slot their suffix names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBase {
    pub base: String,
    pub slots: BTreeMap<u8, String>,
}

/// The 1-based lighting slot a file suffix (as `engine::group::split_suffix` returns it) belongs to.
pub fn suffix_slot(suffix: Option<&str>) -> Option<u8> {
    Some(match suffix {
        None | Some("d") => 1,
        Some("n" | "msn") => 2,
        Some("g" | "sk") => 3,
        Some("p") => 4,
        Some("e") => 5,
        Some("m" | "em" | "envmask" | "rmaos" | "rfaos") => 6,
        Some("cnr" | "f" | "i") => 7,
        Some("s" | "b" | "bl") => 8,
        Some(_) => return None,
    })
}

/// Read `root` once. Pure: no egui, no writes.
// shortcut: loose files only; a BSA-packed target lists records only. Upgrade by reading the scan's origin rows when owners hit it.
pub fn read(root: &Path) -> TargetMod {
    let mut t = TargetMod {
        root: root.to_path_buf(),
        ..Default::default()
    };
    let mut meshes: BTreeMap<String, MeshPath> = BTreeMap::new();
    let mut files: BTreeMap<String, FileBase> = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                t.problems.push(format!("{}: {e}", dir.display()));
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(path);
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/").to_lowercase();
            if !rel.contains('/') {
                if [".esp", ".esm", ".esl"].iter().any(|e| rel.ends_with(e)) {
                    t.plugins.insert(rel);
                }
            } else if rel.starts_with("meshes/") && rel.ends_with(".nif") {
                read_nif(&path, &rel, &mut meshes, &mut t.problems);
            } else if rel.starts_with("textures/") && rel.ends_with(".dds") {
                let (dir, name) = rel.rsplit_once('/').unwrap_or(("", &rel));
                let stem = name.strip_suffix(".dds").unwrap_or(name);
                let (stem, suffix) = engine::group::split_suffix(stem);
                if let Some(slot) = suffix_slot(suffix.as_deref()) {
                    let base = format!("{dir}/{stem}");
                    files
                        .entry(base.clone())
                        .or_insert_with(|| FileBase {
                            base,
                            slots: BTreeMap::new(),
                        })
                        .slots
                        .insert(slot, rel);
                }
            }
        }
    }
    t.meshes = meshes.into_values().collect();
    t.files = files.into_values().collect();
    t
}

fn read_nif(
    path: &Path,
    rel: &str,
    out: &mut BTreeMap<String, MeshPath>,
    problems: &mut Vec<String>,
) {
    let scan = match std::fs::read(path)
        .map_err(|e| e.to_string())
        .and_then(|b| nif::scan::scan(&b).map_err(|e| e.to_string()))
    {
        Ok(s) => s,
        Err(e) => {
            problems.push(format!("{rel}: {e}"));
            return;
        }
    };
    let mut seen = BTreeSet::new();
    for shape in &scan.shapes {
        let nif::scan::Shader::Lighting(_, slots) = &shape.shader else {
            continue;
        };
        let Some(first) = slots.first().filter(|s| !s.norm.is_empty()) else {
            continue;
        };
        let base = strip_dds(&first.norm).to_string();
        let m = out.entry(base.clone()).or_insert_with(|| MeshPath {
            base: base.clone(),
            slots: slots
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.norm.is_empty())
                .map(|(i, s)| (i as u8 + 1, s.norm.clone()))
                .collect(),
            meshes: 0,
        });
        if seen.insert(base) {
            m.meshes += 1;
        }
    }
}

/// Whether two folders are the same or one holds the other. A folder that doesn't exist yet is placed under its
/// nearest existing parent, and the comparison ignores case (Windows).
pub fn overlaps(a: &Path, b: &Path) -> bool {
    let (a, b) = (canon(a), canon(b));
    a.starts_with(&b) || b.starts_with(&a)
}

fn canon(p: &Path) -> PathBuf {
    let mut tail = vec![];
    let mut cur = p;
    let base = loop {
        if let Ok(c) = cur.canonicalize() {
            break c;
        }
        match (cur.file_name(), cur.parent()) {
            (Some(n), Some(par)) => {
                tail.push(n);
                cur = par;
            }
            _ => break p.to_path_buf(),
        }
    };
    let full: PathBuf = base.iter().chain(tail.into_iter().rev()).collect();
    PathBuf::from(full.to_string_lossy().to_lowercase())
}

impl AppModel {
    pub fn target(&self) -> Option<&Arc<TargetMod>> {
        self.target_mod.as_ref()
    }

    /// Store (or clear) the target mod in `sdc-project.toml`, saved at once like the export folder. `Err` leaves the
    /// file as it was.
    pub fn set_target_mod(&mut self, p: Option<PathBuf>) -> Result<(), &'static str> {
        if let Some(p) = &p {
            if self.export_folder_chosen().is_some_and(|e| overlaps(p, &e)) {
                return Err(TARGET_IS_EXPORT);
            }
        }
        let Some(d) = self.sets_doc.as_mut().filter(|d| !d.read_only()) else {
            return Err("sdc-project.toml is read-only, so the target mod can't be saved");
        };
        let was_dirty = d.is_dirty();
        d.set_target_mod(p.as_deref());
        if was_dirty {
            self.status_line =
                "Target mod set; it is saved in sdc-project.toml with the next Save".into();
        } else if let Err(e) = d.save(false) {
            self.save_failed(super::DocKind::Project, &e);
        }
        self.refresh_target_mod();
        Ok(())
    }

    /// A folder chosen in the Map to target… dialog: stored, or the refusal kept for the dialog to show.
    pub fn target_picked(&mut self, path: PathBuf) {
        self.target_pick_error = self.set_target_mod(Some(path)).err();
    }

    /// Read the project's target mod again on a worker (a new target, a new folder, a finished scan). A read of an
    /// older request is dropped when it lands.
    pub(crate) fn refresh_target_mod(&mut self) {
        self.target_mod_gen += 1;
        let gen = self.target_mod_gen;
        let Some(root) = self.sets_doc.as_ref().and_then(|d| d.target_mod()) else {
            self.target_mod = None;
            self.target_mod_rev += 1;
            self.evidence_rev += 1;
            return;
        };
        if self.target_mod.as_ref().is_some_and(|t| t.root != root) {
            self.target_mod = None;
        }
        spawn_guarded(self.tx.clone(), "target-mod", move |tx| {
            let target = Arc::new(read(&root));
            let _ = tx.send(WorkerMsg::TargetMod { gen, target });
        });
        // The path is known now; the lists follow when the read lands.
        self.target_mod_rev += 1;
    }

    pub(super) fn target_mod_landed(&mut self, gen: u64, target: Arc<TargetMod>) {
        if gen != self.target_mod_gen {
            return;
        }
        self.target_mod = Some(target);
        self.target_mod_rev += 1;
        self.evidence_rev += 1;
    }

    /// The scan's texture set records: those the target mod's plugins define, or all of them with no target mod.
    pub fn target_records(&self) -> Vec<&TxstRecord> {
        let all = self.known.records.iter();
        match &self.target_mod {
            Some(t) => all
                .filter(|r| t.plugins.contains(&r.plugin.to_lowercase()))
                .collect(),
            // Set but not read yet: no records rather than every record.
            None if self
                .sets_doc
                .as_ref()
                .and_then(|d| d.target_mod())
                .is_some() =>
            {
                vec![]
            }
            None => all.collect(),
        }
    }

    /// How many target meshes and target records name `path` in a slot (`textures/...dds`, ASCII case ignored).
    pub fn used_by(&self, path: &str) -> u32 {
        let has = |s: &BTreeMap<u8, String>| s.values().any(|p| p.eq_ignore_ascii_case(path));
        let meshes = self
            .target_mod
            .iter()
            .flat_map(|t| &t.meshes)
            .filter(|m| has(&m.slots))
            .count();
        let records = self
            .target_records()
            .iter()
            .filter(|r| has(&r.slots))
            .count();
        (meshes + records) as u32
    }
}
