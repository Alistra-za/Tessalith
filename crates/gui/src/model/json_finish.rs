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

/// E7 (ruling 3c-50): what `guarded_write` does at `path`, asked of the disk now and written nowhere. `text`: the text it
/// writes, known before the run; `out` then gets the hash it records. `None`: this run decides whether the file is
/// written and with what, so nothing is recorded here. Returns whether the file is listed (`Some(marked)`) and whether
/// the run decides a hash record. Ruling 3c-48: a case-only rename (`remove_own_case_twin` drops sdc's own file under
/// the old spelling) changes the file's name on disk, so it is a change; the removed spelling counts under this entry,
/// whatever `may_replace` then says of the file (3c-51 (a)).
pub(super) fn guarded_plan(
    path: &Path,
    text: Option<&str>,
    out: &mut BTreeMap<String, String>,
) -> (Option<bool>, bool) {
    use super::history::{own_case_twin, record_key, recorded};
    use sha2::{Digest, Sha256};
    let key = record_key(path);
    let last = out.get(&key).cloned();
    // On a case-insensitive disk `path` reads the twin, which is ours, so `may_replace` answers as after the removal.
    let renamed = own_case_twin(path, last.as_deref()).is_some();
    let Some(text) = text else {
        let changes = renamed || super::review::may_change(path, last.as_deref());
        return (changes.then_some(true), true);
    };
    // Ruling 3c-51 (a): the twin goes before `may_replace` is asked, so a kept file still loses its old spelling.
    if engine::dds::may_replace(path, text.as_bytes(), recorded(last.as_deref())).is_err() {
        return (renamed.then_some(false), false);
    }
    out.insert(key, hex::encode(Sha256::digest(text.as_bytes())));
    let changes = renamed || std::fs::read(path).map_or(true, |b| b != text.as_bytes());
    (changes.then_some(false), false)
}

/// E7 (ruling 3c-50): `Guarded::write` asked instead of done: the record it returns for the files decided before the
/// run, whether the run decides some record, and in `list` each file it may change, marked when the run decides it.
pub(super) fn plan_guarded<P: Guarded>(
    p: &P,
    recorded: &BTreeMap<String, String>,
    written: &dyn Fn(&Path) -> bool,
    list: &mut Vec<(PathBuf, bool)>,
) -> (BTreeMap<String, String>, bool) {
    let mut out = recorded.clone();
    let mut run = false;
    for (root, files) in p.planned(written) {
        out = super::history::records_at(&out, &root);
        for (path, text) in files {
            let (listed, by_run) = guarded_plan(&path, text.as_deref(), &mut out);
            run |= by_run;
            if let Some(marked) = listed {
                list.push((path, marked));
            }
        }
    }
    (out, run)
}

/// One folder of a guarded export's planned files: its root, and each file with its text (`None`: this run decides
/// whether it is written, or its text).
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
    /// E7 (ruling 3c-50): the files `write` may go through, per folder in its order, each with its text when that and
    /// whether it is written are decided before the run, else `None`. `written` says whether this run may write a file
    /// (a DDS output).
    fn planned(&self, written: &dyn Fn(&Path) -> bool) -> Vec<PlannedFolder>;
    /// `record`, applied to Review's copy of the records.
    fn preview(r: &mut crate::sets_doc::RecordPreview, shas: &BTreeMap<String, String>);
}

impl AppModel {
    /// Test evidence for a prepared guarded JSON: did its phase pass this export's gate?
    #[cfg(feature = "test-hooks")]
    pub fn guarded_json_phase_passed(&self, kind: super::review::JsonKind) -> bool {
        use super::review::JsonKind;
        let gate = match kind {
            JsonKind::CsPbr => super::cs_pbr_export::PendingCs::GATE,
            JsonKind::ComplexMaterial => super::cm_json::PendingCm::GATE,
            JsonKind::Xpmf => super::xpmf_json::PendingXpmf::GATE,
            JsonKind::CsSkin => super::cs_skin_json::PendingCsSkin::GATE,
            JsonKind::PgPatcher => return false,
        };
        self.json_gate(gate).is_ok()
    }

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
