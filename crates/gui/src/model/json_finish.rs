//! The three guarded JSON finishers (CS PBR, complex material, XPMF) as one (Note FW). Each kind says how it reads and
//! writes its files and which Also phase gates it; the gate, the hash record and the Also walk's file list are here once.
use super::{also, AppModel};
use crate::sets_doc::SetsDoc;
use engine::also::Phase;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One guarded JSON write (complex material, CS skin): drop sdc's own case twin (O6), keep a file Tessalith did not
/// write or cannot read, else write `text` atomically and record its hash in `out`. A kept or failed file adds its
/// note. Returns whether it was written.
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
