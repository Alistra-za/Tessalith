//! Review (layout v2 B2, C2, C5; rulings R1–R4): the pre-export gate as a bottom-panel tab, not a modal. It holds
//! only what the user answered; blockers and undecided textures are read live from the model.
use super::history::{record_key, records_at, sha256_of};
use super::{
    bottom::BottomTab, cm_json, cs_pbr_export, pbr_export, xpmf_json, AppModel, Continue,
    ItemStatus,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Note FB1: the gate's reason, and the Blockers row's text, while no export folder is chosen.
pub const NO_EXPORT_FOLDER: &str = "No export folder chosen";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Review {
    /// C4, R6: the answer, given in Review, for the edited PGPatcher JSON.
    pub json_answer: Option<JsonAnswer>,
    /// Note CN (Review Focus 5, P9): with a tick on, why `sdc.toml` cannot be saved; taken when Review opens and
    /// again on its Export, never per frame. `None`: writable (or no tick).
    pub toml_unwritable: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonKind {
    PgPatcher,
    CsPbr,
    ComplexMaterial,
    /// Note GI: an XPMF profile (`SKSE/Plugins/XPMF/tessalith_<set>.json`).
    Xpmf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskState {
    New,
    Ours,
    Edited,
    Unreadable,
}

/// One JSON file the export would write, and what is on disk there now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonLine {
    pub kind: JsonKind,
    pub path: PathBuf,
    pub state: DiskState,
    /// Note FC5: the file's hash now, for an edited PGPatcher JSON only (what an answer is tied to).
    pub sha: Option<String>,
}

/// Keep or overwrite the edited PGPatcher JSON at `path`, for the file whose hash was `sha` when answered: a file
/// changed since gets the question again (C4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonAnswer {
    pub path: PathBuf,
    pub sha: Option<String>,
    pub overwrite: bool,
}

impl JsonAnswer {
    /// Note FC5: this answer is for `l` as it is on disk now; a file changed since the answer is asked again.
    pub fn answers(&self, l: &JsonLine) -> bool {
        self.path == l.path && self.sha == l.sha
    }
}

/// What is at `path` against the hash recorded for Tessalith's last write there.
pub fn disk_state(path: &Path, recorded: Option<&str>) -> DiskState {
    use sha2::{Digest, Sha256};
    match std::fs::read(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => DiskState::New,
        Err(_) => DiskState::Unreadable,
        Ok(b) if recorded == Some(hex::encode(Sha256::digest(&b)).as_str()) => DiskState::Ours,
        Ok(_) => DiskState::Edited,
    }
}

pub(super) fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn nif_edits(n: usize) -> String {
    format!("{n} NIF edit{}", plural(n))
}

impl AppModel {
    /// Open Review (its answers are kept when it is already open) and show its tab.
    pub fn open_review(&mut self) {
        let unwritable = self.also_toml_unwritable();
        self.review
            .get_or_insert_with(Review::default)
            .toml_unwritable = unwritable;
        self.show_tab(BottomTab::Review);
    }

    /// C4: the JSON files the export would write and their state on disk. Cached (K3) per view key for 1 s, since
    /// Review draws it every frame and it reads the files (AG128).
    pub fn review_json(&self) -> Vec<JsonLine> {
        let key = self.view_key();
        if let Some((k, at, lines)) = &*self.review_json_cache.borrow() {
            if *k == key && at.elapsed() < Duration::from_secs(1) {
                return lines.clone();
            }
        }
        let mut lines = vec![];
        // K13: vanilla (and a project without a PBR set) writes no PGPatcher JSON.
        if let Some(p) = pbr_export::prepare(self).filter(|p| !p.entries.is_empty()) {
            let state = disk_state(&p.path, self.pgp_record(&p.root, &p.path).as_deref());
            let sha = (state == DiskState::Edited)
                .then(|| sha256_of(&p.path))
                .flatten();
            lines.push(JsonLine {
                kind: JsonKind::PgPatcher,
                path: p.path,
                state,
                sha,
            });
        }
        let doc = self.sets_doc.as_ref();
        let mut files = |kind, root: PathBuf, rels: Vec<String>, shas| {
            // As the finishers read them: a relative record is under the export folder.
            let shas = records_at(&shas, &root);
            for rel in rels {
                let path = root.join(rel);
                let state = disk_state(&path, shas.get(&record_key(&path)).map(String::as_str));
                lines.push(JsonLine {
                    kind,
                    path,
                    state,
                    sha: None,
                });
            }
        };
        if let (Some(p), Some(d)) = (cs_pbr_export::prepare(self), doc) {
            let rels = p.files.into_iter().map(|(r, _)| r).collect();
            files(JsonKind::CsPbr, p.root, rels, d.cs_pbr_shas());
        }
        if let (Some(p), Some(d)) = (cm_json::prepare(self), doc) {
            let rels = p.files.into_iter().map(|(r, _)| r).collect();
            files(JsonKind::ComplexMaterial, p.root, rels, d.cm_json_shas());
        }
        if let (Some(p), Some(d)) = (xpmf_json::prepare(self), doc) {
            for f in p.folders() {
                let rels = f.files.iter().map(|(r, _)| r.clone()).collect();
                files(JsonKind::Xpmf, f.root.clone(), rels, d.xpmf_shas());
            }
        }
        *self.review_json_cache.borrow_mut() = Some((key, Instant::now(), lines.clone()));
        lines
    }

    /// C4: Keep (false) or Overwrite (true) the edited PGPatcher JSON, for the file as it is now. No-op while Review
    /// is closed.
    pub fn answer_review_json(&mut self, overwrite: bool) {
        if self.review.is_none() {
            return;
        }
        let Some(p) = pbr_export::prepare(self).filter(|p| !p.entries.is_empty()) else {
            return;
        };
        let sha = sha256_of(&p.path);
        // The next frame's lines hash the file again, so the answer matches the line it was given for.
        self.review_json_cache.take();
        if let Some(r) = self.review.as_mut() {
            r.json_answer = Some(JsonAnswer {
                path: p.path,
                sha,
                overwrite,
            });
        }
    }

    /// R6: the PGPatcher JSON this export would write exists and is not Tessalith's (uncached: one read per click).
    pub fn pgp_json_edited(&self) -> bool {
        pbr_export::prepare(self)
            .filter(|p| !p.entries.is_empty())
            .is_some_and(|p| {
                disk_state(&p.path, self.pgp_record(&p.root, &p.path).as_deref())
                    == DiskState::Edited
            })
    }

    /// Close Review and its answers; the panel collapses when it showed Review (R7: the tab is listed only while open).
    pub fn close_review(&mut self) {
        self.review = None;
        // Note FC5 (GD29): the NIF answers go with Review; each foreign row asks again. Not while a run Review started
        // still owes the NIF export (a Reload mid-export): those answers are its own (rule 7.4).
        self.nif_clear_answers_request |= !self.nif_export_queued;
        self.nif_answer_request = None;
        self.review_json_cache.take();
        // Its queued export goes with it (Task 1.4's Close).
        self.continue_after_plan = None;
        if self.bottom.tab == BottomTab::Review {
            self.collapse_bottom();
        }
    }

    /// The files an export writes: items with a job that is neither undecided, packed into another nor skipped.
    /// Cached like `bottom::problems` (Review and the header read it every frame): `items` is only replaced
    /// together with `invalidate_views`, which changes the view key.
    pub fn export_files(&self) -> usize {
        let key = self.view_key();
        let inner = super::bottom::problems_inner(self);
        {
            let c = self.views.borrow();
            if c.key.as_ref() == Some(&key) {
                if let Some((k, n)) = c.export_files {
                    if k == inner {
                        return n;
                    }
                }
            }
        }
        let n = self.count_export_files();
        #[cfg(feature = "test-hooks")]
        self.export_files_builds
            .set(self.export_files_builds.get() + 1);
        let mut c = self.views.borrow_mut();
        if c.key.as_ref() != Some(&key) {
            *c = super::ViewCache {
                key: Some(key),
                ..Default::default()
            };
        }
        c.export_files = Some((inner, n));
        n
    }

    fn count_export_files(&self) -> usize {
        self.items
            .iter()
            .filter(|i| {
                i.job.is_some()
                    && !matches!(
                        i.status,
                        ItemStatus::NeedsDecision
                            | ItemStatus::PackedInto(_)
                            | ItemStatus::Skipped(_)
                    )
            })
            .count()
    }

    /// Export can start: a folder is open, no export runs, and `sdc.toml` is not ignored (M3a spec §7.4: an export
    /// would not be the project's settings).
    pub fn can_export(&self) -> bool {
        let read_only = self.project.as_ref().is_some_and(|p| p.read_only());
        self.root.is_some() && self.export.running.is_none() && !read_only
    }

    /// K11: why Problems' and Review's fix buttons (and the texture panel's edit controls, Note FK) are off, or `None`
    /// when they work.
    pub fn fix_off(&self) -> Option<String> {
        self.toml_read_only_reason().or_else(|| {
            self.plan_wait()
                .then(|| "Wait for the plan to finish".to_string())
        })
    }

    /// Note FL: `fix_off` for one Problems row. A block or ask row comes from the last full plan, so a partial plan
    /// may have made it stale: its fix stays off until the full plan lands.
    pub fn fix_off_for(&self, p: &super::bottom::Problem) -> Option<String> {
        self.fix_off().or_else(|| {
            (p.carried && self.full_plan_pending())
                .then(|| "Being rechecked: wait for the plan to finish".to_string())
        })
    }

    /// Ok((files, skipped_undecided)) or Err(the reason the button shows), in A9's order.
    pub fn review_gate(&self) -> Result<(usize, usize), String> {
        if self.root.is_none() {
            // Note GH8: with no project Review exports NIF edits only, to the gui.toml folder (never a `sdc-out` guess).
            // No NIF row at all is the fresh-start state, which keeps its old reason; a blocked row (no folder) says why.
            if self.nif_rows.is_empty() {
                return Err("Open a folder first".into());
            }
            if self.export_folder_chosen().is_none() {
                return Err(NO_EXPORT_FOLDER.into());
            }
            if self.export.running.is_some() {
                return Err("An export is running".into());
            }
            if !self.nif_rows_written() {
                return Err("Open a folder first".into());
            }
            if let Some(name) = self.unanswered_overwrites().first() {
                return Err(format!("Answer Keep or Overwrite for {name}"));
            }
            return Ok((0, 0));
        }
        // `toml_read_only_reason` is `Some` exactly when `project.read_only()` (both read the load error).
        if let Some(why) = self.toml_read_only_reason() {
            return Err(why);
        }
        // Note FB1: never write to a folder nobody chose (the `sdc-out` fallback is only a name).
        if self.export_folder_chosen().is_none() {
            return Err(NO_EXPORT_FOLDER.into());
        }
        if self.export.running.is_some() {
            return Err("An export is running".into());
        }
        // Note FC1 (AM14): the plan ran on the fallback; exporting it would not be the project's preset (the CLI
        // exits 2 on the same file).
        if let Some(id) = self.missing_preset_id() {
            return Err(format!("Preset {id} is not installed"));
        }
        let n = pbr_export::export_blocks(self).len();
        if n > 0 {
            return Err(format!("Fix {n} blocker{} first", plural(n)));
        }
        // Note CN: the Also rows, after the blocks and before the overwrite questions (A9); nothing writes (7.1, 7.2).
        if self.also_on() {
            self.also_gate()?;
        }
        // Note FC5: then every overwrite question (supersedes CA7's "no choice made: asked after the export").
        if let Some(name) = self.unanswered_overwrites().first() {
            return Err(format!("Answer Keep or Overwrite for {name}"));
        }
        Ok((self.export_files(), self.undecided()))
    }

    /// Note FC5: the file names of the overwrite questions Review has no answer for: an edited PGPatcher JSON, then
    /// each NIF whose export path holds a file Tessalith did not write (a blocked NIF row is skipped, never asked).
    pub fn unanswered_overwrites(&self) -> Vec<String> {
        let answered = self.review.as_ref().and_then(|r| r.json_answer.as_ref());
        let name = |p: &Path| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        };
        // shortcut: the header asks this every frame (export_is_clean), so `review_json` rereads the JSON files once a
        // second through its 1 s cache, even with no PGPatcher JSON edited (knowing that needs the same read); add a
        // cheap "any PBR set" check or an mtime key if a large CS PBR or CM set list makes that read show in a profile.
        let json = self.review_json().into_iter().filter(|l| {
            l.kind == JsonKind::PgPatcher
                && l.state == DiskState::Edited
                && !answered.is_some_and(|a| a.answers(l))
        });
        let nifs = self
            .nif_rows
            .iter()
            .filter(|r| r.blocked.is_none() && r.foreign.is_some() && r.answer.is_none());
        json.map(|l| name(&l.path))
            .chain(nifs.map(|r| name(Path::new(&r.path))))
            .collect()
    }

    /// Note FC5: the NIF edits Review lists, handed over by `app.rs` each frame.
    pub fn set_nif_review_rows(&mut self, rows: Vec<super::meshes::NifReviewRow>) {
        self.nif_rows = rows;
    }

    pub fn nif_review_rows(&self) -> &[super::meshes::NifReviewRow] {
        &self.nif_rows
    }

    /// Note FC14: a NIF edit Review's Export would write (a blocked one is skipped, so it keeps nothing waiting).
    pub(crate) fn nif_rows_written(&self) -> bool {
        self.nif_rows.iter().any(|r| r.blocked.is_none())
    }

    /// Note FC5: Review's Keep (false) or Overwrite (true) for the NIF at `path`, for the file with hash `sha256`;
    /// `app.rs` passes it to `MeshesModel::set_nif_answer`. No-op while Review is closed.
    pub fn answer_review_nif(&mut self, path: &str, overwrite: bool, sha256: [u8; 32]) {
        use super::meshes::export::GuardChoice;
        if self.review.is_none() {
            return;
        }
        let choice = if overwrite {
            GuardChoice::Overwrite
        } else {
            GuardChoice::KeepTheirs
        };
        self.nif_answer_request = Some((path.to_string(), choice, sha256));
    }

    /// Note FC5: true once, in the frame after the run Review's Export started has fully ended (with Also on, after
    /// its zips); `app.rs` then calls `MeshesModel::export_nif_edits` and passes the result to `nif_export_result`.
    pub fn take_nif_export_due(&mut self) -> bool {
        std::mem::take(&mut self.nif_export_due)
    }

    /// Note FC5: the NIF export's result, in Messages and in the export's notes (Review is closed by then; the notes
    /// are the Files tab's lines).
    pub fn nif_export_result(&mut self, result: Result<usize, String>) {
        let (sev, text) = match result {
            Ok(0) => return,
            Ok(n) => (
                super::messages::Severity::Info,
                format!("NIF edits written: {n} file{}", plural(n)),
            ),
            Err(why) => (
                super::messages::Severity::Warning,
                format!("NIF edits skipped: {why}"),
            ),
        };
        self.export.notes.push(text.clone());
        self.messages.notify(sev, text);
    }

    /// Note FB1: Review would write without skipping anything: a finished plan with files to write, the gate open and
    /// nothing undecided (A12 withheld "ready" from an empty or still-planning project too).
    pub fn export_is_clean(&self) -> bool {
        self.planning.is_none()
            && self.votes_pending.is_none()
            && self.export_files() > 0
            && self.review_gate().is_ok()
            && self.undecided() == 0
            // Note FC5: NIF edits are written by Review's Export only. Note FC14: a blocked one is never written.
            && !self.nif_rows_written()
    }

    /// Note FB1: the header's Export button: `Export {n} files` once the project is clean, else `Review {n} files`.
    pub fn header_export_text(&self) -> String {
        self.export_text(self.export_is_clean())
    }

    /// [`Self::header_export_text`] for a `clean` the caller already has (the header asks once per frame).
    pub fn export_text(&self, clean: bool) -> String {
        // Final review M5: with no folder open there is no count to give ("Review 0 files" greeted a new user).
        if self.root.is_none() {
            // Note FI18 (GH handoff item 3): pending NIF edits are counted as in FI7; there are no files to name.
            return match self.nif_rows.len() {
                0 => "Export".into(),
                rows => format!("Review {}", nif_edits(rows)),
            };
        }
        let n = self.export_files();
        let rows = self.nif_rows.len();
        let blocked = self.nif_rows.iter().filter(|r| r.blocked.is_some()).count();
        let mut t = format!(
            "{} {n} file{}",
            if clean { "Export" } else { "Review" },
            plural(n)
        );
        // Note FI7: `Review` writes nothing, so it counts every pending NIF edit; `Export` names only what it writes.
        if !clean && rows > 0 {
            t.push_str(&format!(" + {}", nif_edits(rows)));
        } else if clean && blocked > 0 {
            t.push_str(&format!(" (skip {})", nif_edits(blocked)));
        }
        t
    }

    /// "Export 20 files (skip 2 undecided)".
    pub fn review_button_text(&self) -> String {
        // Note GH8: no project, no texture run: the button writes the NIF edits alone.
        if self.root.is_none() {
            let n = self.nif_rows.iter().filter(|r| r.blocked.is_none()).count();
            return format!("Export {n} NIF edit{}", plural(n));
        }
        let files = self.export_files();
        let skipped = self.undecided();
        let mut t = format!("Export {files} file{}", plural(files));
        // Re-check A7 (TA30): this project's planned files, written to the PBR folder and copied or derived on.
        if self.also_on() {
            t.push_str(&format!(" to {} folders", self.also_folder_count()));
        }
        // Note FI7 (rule 7.1): only the NIF edits this Export writes; a blocked one is skipped.
        let blocked = self.nif_rows.iter().filter(|r| r.blocked.is_some()).count();
        let written = self.nif_rows.len() - blocked;
        if written > 0 {
            t.push_str(&format!(" + {}", nif_edits(written)));
        }
        let mut skip = vec![];
        if skipped > 0 {
            skip.push(format!("{skipped} undecided"));
        }
        if blocked > 0 {
            skip.push(nif_edits(blocked));
        }
        if !skip.is_empty() {
            t.push_str(&format!(" (skip {})", skip.join(", ")));
        }
        t
    }

    /// Review's Export (R3): `export_clicked`'s front half (a plan in flight → wait; changed sources → plan again and
    /// wait), then `Continue::Review`: the guards and the gate again before the export. Never on a stale plan.
    pub fn export_from_review(&mut self) {
        // Note GH8: Review's Export also works with no project (it writes the NIF edits alone).
        if self.review.is_none() {
            return;
        }
        self.status_note = None;
        self.continue_review();
    }

    /// The front half of Review's Export, also run when a scan-finished read it waited for lands.
    pub(super) fn continue_review(&mut self) {
        if self.planning.is_some() || self.votes_pending.is_some() {
            self.continue_after_plan = Some(Continue::Review);
            return;
        }
        if self.check_sources() > 0 && self.session.is_some() {
            self.continue_after_plan = Some(Continue::Review);
            self.replan();
            return;
        }
        self.run_continue(Continue::Review);
    }
}
