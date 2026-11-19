//! The four guarded JSON finishers (CS PBR, complex material, CS skin, XPMF) as one (Note FW). Each kind says how it
//! reads and writes its files and which Also phase gates it; the gate, the guarded write, the hash record and the Also
//! walk's file list are here once.
use super::{also, AppModel};
use crate::sets_doc::SetsDoc;
use engine::also::Phase;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One guarded JSON write, for every kind: drop sdc's own case twin (O6: a name whose case changed renames sdc's own
/// unchanged file; a replace may keep the old name's case), keep a file Tessalith did not write or cannot read, else
/// write `text` atomically and record its hash in `out`. A kept or failed file adds its note. Returns whether it was
/// written.
pub(super) fn guarded_write(
    path: &Path,
    text: &str,
    out: &mut BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> bool {
    use super::history::{record_key, recorded, remove_own_case_twin, sha256_of};
    let key = record_key(path);
    let last = out.get(&key).cloned();
    let last = last.as_deref();
    remove_own_case_twin(path, last);
    if let Err(k) = engine::dds::may_replace(path, text.as_bytes(), recorded(last)) {
        notes.push(k.note(path));
        return false;
    }
    match engine::dds::write_atomic(path, text.as_bytes()) {
        Ok(()) => {
            if let Some(s) = sha256_of(path) {
                out.insert(key, s);
            }
            true
        }
        Err(e) => {
            notes.push(format!("{} not written: {e}", path.display()));
            false
        }
    }
}

/// The value Review records for a file whose text is not known before the run: it never equals a stored hash, so the
/// records count as changed (ruling 3c-47).
pub(super) const UNKNOWN_SHA: &str = "unknown until the export writes it";

/// E7 (ruling 3c-46): what `guarded_write` would do with `text` at `path`, asked of the disk now and written nowhere.
/// `None`: the file is kept. `Some(changes)`: it is written (and `out` gets the hash it would record), and `changes`
/// says whether that changes the file. A `text` of `None` is not known before the run (it depends on a DDS this run
/// writes): only an absent file or one whose hash is recorded is replaced, and the change is assumed. Ruling 3c-48: a
/// case-only rename (`remove_own_case_twin` drops sdc's own file under the old spelling) changes the file's name on
/// disk, so it is a change; the removed spelling counts under this entry.
pub(super) fn guarded_plan(
    path: &Path,
    text: Option<&str>,
    out: &mut BTreeMap<String, String>,
) -> Option<bool> {
    use super::history::{own_case_twin, record_key, recorded};
    use sha2::{Digest, Sha256};
    let key = record_key(path);
    let last = out.get(&key).cloned();
    // On a case-insensitive disk `path` reads the twin, which is ours, so `may_replace` answers as after the removal.
    let renamed = own_case_twin(path, last.as_deref()).is_some();
    let new = text.map_or(&[][..], str::as_bytes);
    engine::dds::may_replace(path, new, recorded(last.as_deref())).ok()?;
    let Some(text) = text else {
        out.insert(key, UNKNOWN_SHA.into());
        return Some(true);
    };
    out.insert(key, hex::encode(Sha256::digest(text.as_bytes())));
    Some(renamed || std::fs::read(path).map_or(true, |b| b != text.as_bytes()))
}

/// E7 (ruling 3c-46): `Guarded::write` asked instead of done: the record it would return, and in `list` each file it
/// would change, with whether that change is certain (`false`: its text is not known before the run).
pub(super) fn plan_guarded<P: Guarded>(
    p: &P,
    recorded: &BTreeMap<String, String>,
    written: &dyn Fn(&Path) -> bool,
    list: &mut Vec<(PathBuf, bool)>,
) -> BTreeMap<String, String> {
    let mut out = recorded.clone();
    for (root, files) in p.planned(written) {
        out = super::history::records_at(&out, &root);
        for (path, text) in files {
            if guarded_plan(&path, text.as_deref(), &mut out) == Some(true) {
                list.push((path, text.is_some()));
            }
        }
    }
    out
}

/// One folder of a guarded export's planned files: its root, and each file with its text (`None`: not known before the
/// run).
pub(super) type PlannedFolder = (PathBuf, Vec<(PathBuf, Option<String>)>);

/// What a kind of guarded JSON export hands the shared finisher.
pub(super) trait Guarded {
    /// How the kind's notes start: `<NAME> not written: <why>`.
    const NAME: &'static str;
    /// The Also phase whose own summary must have `failed == 0` (Note FW1). `None`: only a cancel or an error holds
    /// the write back.
    const GATE: Option<Phase>;
    fn has_files(&self) -> bool;
    fn recorded(d: &SetsDoc) -> BTreeMap<String, String>;
    /// Write what may be written; returns the new hash record.
    fn write(
        &self,
        recorded: &BTreeMap<String, String>,
        notes: &mut Vec<String>,
    ) -> BTreeMap<String, String>;
    /// Files of this export that now hold their text (for the Also walk).
    fn current(&self) -> Vec<PathBuf>;
    fn record(d: &mut SetsDoc, shas: &BTreeMap<String, String>) -> std::io::Result<()>;
    /// E7 (ruling 3c-46): the files `write` would go through, per folder in its order, as the disk will be after a
    /// clean run: `written` says whether this run writes a file (a DDS output), so a file the kind needs on disk is
    /// there when it is now or when this run writes it.
    fn planned(&self, written: &dyn Fn(&Path) -> bool) -> Vec<PlannedFolder>;
    /// `record`, applied to Review's copy of the records.
    fn preview(r: &mut crate::sets_doc::RecordPreview, shas: &BTreeMap<String, String>);
}

impl AppModel {
    /// Why the JSON of a kind gated on `gate` is not written, or `Ok`. A cancel or an error always holds it back. After
    /// that a plain export needs `failed == 0`; an Also export needs it for `gate`'s phase alone (a CM gate falls back
    /// to the PBR phase when no CM phase ran: the file sits in the PBR folder).
    fn json_gate(&self, gate: Option<Phase>) -> Result<(), String> {
        let summary = self.export.summary.as_ref();
        let unfinished = || "the export did not finish".to_string();
        if self.export.error.is_some() || !summary.is_some_and(|s| s.cancelled.is_empty()) {
            return Err(unfinished());
        }
        let Some(gate) = gate else {
            return Ok(());
        };
        if self.also_phases.is_empty() {
            return if summary.is_some_and(|s| s.failed == 0) {
                Ok(())
            } else {
                Err(unfinished())
            };
        }
        let own = if self.also_phases.iter().any(|(p, _)| *p == gate) {
            gate
        } else {
            Phase::Pbr
        };
        match self.also_phases.iter().find(|(p, _)| *p == own) {
            Some((_, true)) => Err(format!("the {} phase failed", also::phase_name(own))),
            _ => Ok(()),
        }
    }

    /// The export ended: after a clean one (see `json_gate`) write the files of `p` and record their hashes (saved,
    /// like the PGPatcher JSON's); otherwise only a note.
    pub(super) fn finish_guarded<P: Guarded>(&mut self, p: Option<P>) {
        let Some(p) = p else {
            return;
        };
        if let Err(why) = self.json_gate(P::GATE) {
            if p.has_files() {
                self.export
                    .notes
                    .push(format!("{} not written: {why}", P::NAME));
            }
            return;
        }
        let recorded = match self.sets_doc.as_ref() {
            Some(d) => P::recorded(d),
            None => return,
        };
        let shas = p.write(&recorded, &mut self.export.notes);
        self.also_json.extend(p.current());
        if shas == recorded {
            return;
        }
        let Some(d) = self.sets_doc.as_mut() else {
            return;
        };
        let was_dirty = d.is_dirty();
        let side = P::record(d, &shas);
        if d.read_only() {
            self.export.notes.push(super::sidecar_only_note(&side));
            return;
        }
        self.save_hash_record(was_dirty);
    }
}
