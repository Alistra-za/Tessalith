//! The Community Shaders runtime JSON (design §5): `PBRTextureSets\<EDID>.json` and `PBRMaterialObjects\<EDID>.json`,
//! one file per EDID per block, written after a clean export. No egui types.
use super::cs_pbr::{field, floats, Kind, GLINT_FIELDS};
use serde::Serialize;
use serde_json::{json, Map, Value as Json};

/// The material-object `specularLevel` written when the block has none: the material reset value. CS's own default
/// (1.0) is 25× that (research §2), so the key is always written (plan ruling R9).
pub const MATO_SPECULAR: f64 = 0.04;

/// `<folder>/<EDID>.json`, the path under the export root (and the hash record's key).
pub fn rel_path(kind: Kind, edid: &str) -> String {
    format!("{}/{edid}.json", kind.folder())
}

/// One file's text: the block's known fields with every number a float (CS drops int colour arrays), 4-space
/// indent, keys sorted as CS's own saver (nlohmann::json) writes them, `\n` line ends, no final newline (ruling R3).
/// serde_json's map is sorted (no `preserve_order` in Cargo.lock); the golden tests pin it.
pub fn to_text(kind: Kind, values: &Map<String, Json>) -> String {
    let mut obj = Map::new();
    for (k, v) in values {
        if field(kind, k).is_none() {
            continue;
        }
        let v = match v.as_object() {
            Some(g) => Json::Object(
                g.iter()
                    .filter(|(gk, _)| {
                        *gk == "enabled" || GLINT_FIELDS.iter().any(|f| f.name == gk.as_str())
                    })
                    .map(|(gk, gv)| (gk.clone(), gv.clone()))
                    .collect(),
            ),
            None => v.clone(),
        };
        obj.insert(k.clone(), floats(&v));
    }
    if kind == Kind::Mato {
        obj.entry("specularLevel").or_insert(json!(MATO_SPECULAR));
    }
    let mut out = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut out,
        serde_json::ser::PrettyFormatter::with_indent(b"    "),
    );
    Json::Object(obj)
        .serialize(&mut ser)
        .expect("JSON values serialise");
    String::from_utf8(out).expect("serde_json writes UTF-8")
}

use super::cs_pbr::{valid_value, CsBlock, StaticsStatus};
use super::history::{orphans, records_at};
use super::pbr_export::{file_name_problem, json_entries};
use super::AppModel;
use crate::sets_doc::SetEntry;
use engine::config::Target;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

/// Why an EDID cannot be a file name (design §5): empty, a character Windows refuses, a trailing dot or space,
/// a device name.
pub fn edid_problem(edid: &str) -> Option<String> {
    file_name_problem("EDID", edid)
}

fn blocks(e: &SetEntry) -> impl Iterator<Item = (Kind, &CsBlock)> {
    [
        (Kind::Txst, e.cs_txst.as_ref()),
        (Kind::Mato, e.cs_mato.as_ref()),
    ]
    .into_iter()
    .filter_map(|(k, b)| Some((k, b?)))
}

/// Per (set id, kind, EDID as typed): why that file is not written (design §5, ruling R5). Two sets binding one
/// EDID of one kind both refuse it; EDIDs compare case-insensitively because Windows file names do.
pub fn refusals(entries: &[SetEntry]) -> BTreeMap<(String, Kind, String), String> {
    let mut owners: BTreeMap<(Kind, String), Vec<&str>> = BTreeMap::new();
    for e in entries {
        for (kind, b) in blocks(e) {
            for edid in &b.edids {
                let o = owners.entry((kind, edid.to_lowercase())).or_default();
                if !o.contains(&e.id.as_str()) {
                    o.push(&e.id);
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    for e in entries {
        for (kind, b) in blocks(e) {
            for edid in &b.edids {
                let o = &owners[&(kind, edid.to_lowercase())];
                let why = edid_problem(edid).or_else(|| {
                    (o.len() > 1)
                        .then(|| format!("{} {edid} is bound by {}", kind.label(), o.join(" and ")))
                });
                if let Some(w) = why {
                    out.insert((e.id.clone(), kind, edid.clone()), w);
                }
            }
        }
    }
    out
}

/// Whether a block value can be written as is: the type CS reads, and a glint object with a key CS reads (an empty
/// one would be written as `{}`). `to_text` does no shape check, so nothing else may reach it.
fn writable(kind: Kind, k: &str, v: &Json) -> bool {
    field(kind, k).is_some_and(|f| valid_value(f.kind, v))
        && v.as_object().is_none_or(|g| {
            g.keys()
                .any(|gk| gk == "enabled" || GLINT_FIELDS.iter().any(|f| f.name == gk.as_str()))
        })
}

/// What an export about to start writes once it has ended (computed when it starts, like the PGPatcher JSON).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingCs {
    pub root: PathBuf,
    /// (path under the root, `/`-separated; text).
    pub files: Vec<(String, String)>,
    /// Refused files and left-out keys, for the export report.
    pub notes: Vec<String>,
    /// #49: paths of refused files (already noted): not orphans, and their record stays.
    pub refused: Vec<String>,
    /// Note HF9: a run that leaves sets out (a scope or an excluded group) reports no orphans.
    pub scoped: bool,
}

/// Design §5: one file per EDID per block (CS only). `Some` on CS even without blocks: the orphan report needs it.
pub fn prepare(m: &AppModel) -> Option<PendingCs> {
    if m.target != Target::Cs {
        return None;
    }
    let entries = m.sets_doc.as_ref()?.entries();
    let refused = refusals(&entries);
    let mut p = PendingCs {
        root: m.output_root(),
        scoped: !m.reports_orphans(),
        ..Default::default()
    };
    let mut done = BTreeSet::new();
    // Note HF9: only the sets in the run write their files; the others' stay as they are.
    for e in entries.iter().filter(|e| m.in_run_id(&e.id)) {
        for (kind, b) in blocks(e) {
            let mut values = b.values.clone();
            values.retain(|k, v| writable(kind, k, v));
            let left_out = b
                .bad
                .iter()
                .chain(b.values.keys().filter(|k| !values.contains_key(*k)));
            for k in left_out {
                p.notes.push(format!(
                    "{}: {}.{k} has the wrong type and was left out",
                    e.id,
                    kind.key()
                ));
            }
            for edid in &b.edids {
                match refused.get(&(e.id.clone(), kind, edid.clone())) {
                    Some(why) => {
                        p.refused.push(rel_path(kind, edid));
                        let n = format!("{} {edid} not written: {why}", kind.label());
                        if !p.notes.contains(&n) {
                            p.notes.push(n);
                        }
                    }
                    None => {
                        if done.insert((kind, edid.to_lowercase())) {
                            p.files.push((rel_path(kind, edid), to_text(kind, &values)));
                        }
                    }
                }
            }
        }
    }
    // Final review M4: say once why texture-set sets got no PGPatcher entry while the scan cannot be read.
    let no_scan: Vec<String> = json_entries(m)
        .into_iter()
        // Note BM6: a PBR Landscape set keeps its entry without a scan, so it is not left out.
        .filter(|(v, _)| !crate::model::sets::is_pbr_landscape(v))
        .filter_map(|(v, _)| match m.statics_status(&v) {
            Some(StaticsStatus::NoScan(e)) => Some(e),
            _ => None,
        })
        .collect();
    if let Some(e) = no_scan.first() {
        p.notes.push(format!(
            "PGPatcher JSON: {} texture-set set{} left out, the scan database cannot be read ({e}). Rescan, then export again.",
            no_scan.len(),
            if no_scan.len() == 1 { "" } else { "s" },
        ));
    }
    Some(p)
}

/// After a clean export (design §5, ruling R4): write each file unless it was edited outside sdc, then report the
/// files sdc wrote before that no block binds any more (never deleted). `shas` = the record (path → hash of what
/// sdc last wrote there); returns the new record.
pub fn finish(
    p: &PendingCs,
    shas: &BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> BTreeMap<String, String> {
    notes.extend(p.notes.iter().cloned());
    // Audit 08c-1: every record is kept, whatever folder it names and whether its file is there.
    let mut out = records_at(shas, &p.root);
    let mut written = 0;
    for (rel, text) in &p.files {
        let path = p.root.join(rel);
        if super::json_finish::guarded_write(&path, text, &mut out, notes) {
            written += 1;
        }
    }
    if written > 0 {
        notes.push(format!(
            "CS PBR JSON written: {written} files under {}",
            p.root.display()
        ));
    }
    let this_run = p.files.iter().map(|(r, _)| r).chain(&p.refused);
    for path in orphans(&out, &p.root, this_run)
        .into_iter()
        .filter(|_| !p.scoped)
    {
        notes.push(format!(
            "{}: no set binds this EDID any more; the file was left in place",
            path.display()
        ));
    }
    out
}

impl AppModel {
    /// The export ended: after a clean export write the CS files of `p` (taken from `pending_cs` by the caller) and
    /// record their hashes (saved, like the PGPatcher JSON's); otherwise only a note.
    pub fn finish_cs_pbr(&mut self, p: Option<PendingCs>) {
        self.finish_guarded(p);
    }
}

impl super::json_finish::Guarded for PendingCs {
    const NAME: &'static str = "CS PBR JSON";
    // Note FW1: cancel-only, as the PGPatcher JSON.
    const GATE: Option<engine::also::Phase> = None;
    fn has_files(&self) -> bool {
        !self.files.is_empty()
    }
    fn recorded(d: &crate::sets_doc::SetsDoc) -> BTreeMap<String, String> {
        d.cs_pbr_shas()
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
        d.set_cs_pbr_shas(shas)
    }
}
