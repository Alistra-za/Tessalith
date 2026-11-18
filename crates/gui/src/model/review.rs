//! Review (layout v2 B2, C2, C5; rulings R1–R4): the pre-export gate as a bottom-panel tab, not a modal. It holds
//! only what the user answered; blockers and undecided textures are read live from the model.
use super::code_text::counted;
use super::history::{record_key, records_at, sha256_of};
use super::{
    bottom::BottomTab, cm_json, cs_pbr_export, cs_skin_json, pbr_export, xpmf_json, AppModel,
    Continue, ItemStatus,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long `review_json` reuses its read of the JSON files. Tests expire it with `expire_review_json` instead of
/// sleeping past it.
pub const REVIEW_JSON_TTL: Duration = Duration::from_secs(1);

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
    /// A CS Advanced skin material (`<diffuse>.csskin.json`).
    CsSkin,
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

fn nif_edits(n: usize) -> String {
    counted(n, "NIF edit", "NIF edits")
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

    /// Tests: end `review_json`'s cache now, as `REVIEW_JSON_TTL` passing would.
    #[cfg(feature = "test-hooks")]
    pub fn expire_review_json(&self) {
        self.review_json_cache.take();
    }

    /// C4: the JSON files the export would write and their state on disk. Cached (K3) per view key for
    /// `REVIEW_JSON_TTL`, since Review draws it every frame and it reads the files (AG128).
    pub fn review_json(&self) -> Vec<JsonLine> {
        let key = self.view_key();
        if let Some((k, at, lines)) = &*self.review_json_cache.borrow() {
            if *k == key && at.elapsed() < REVIEW_JSON_TTL {
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
        if let (Some(p), Some(d)) = (cs_skin_json::prepare(self), doc) {
            let rels = p.files.into_iter().map(|f| f.rel).collect();
            files(JsonKind::CsSkin, p.root, rels, d.cs_skin_shas());
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
        // Note HF9: closing Review ends an Export set / group; the next Export is the whole project.
        self.set_scope(None);
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

    /// The files an export writes: items with a job that is neither undecided, packed into another, skipped nor in
    /// error (an `Error` job runs and fails, so it writes nothing).
    /// Cached like `bottom::problems` (Review and the header read it every frame): `items` is only replaced
    /// together with `invalidate_views`, which changes the view key.
    pub fn export_files(&self) -> usize {
        let inner = super::bottom::problems_inner(self);
        super::cached(
            self,
            |c| c.export_files.filter(|(k, _)| *k == inner).map(|(_, n)| n),
            |c, n| c.export_files = Some((inner, n)),
            || {
                #[cfg(feature = "test-hooks")]
                self.export_files_builds
                    .set(self.export_files_builds.get() + 1);
                self.count_export_files()
            },
        )
    }

    fn count_export_files(&self) -> usize {
        self.items
            .iter()
            .filter(|i| {
                i.job.is_some()
                    && self.item_in_run(i)
                    && !matches!(
                        i.status,
                        ItemStatus::NeedsDecision
                            | ItemStatus::PackedInto(_)
                            | ItemStatus::Skipped(_)
                            | ItemStatus::Error(_)
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
        if let Some((text, _)) = self.missing_preset_text() {
            return Err(text);
        }
        let n = pbr_export::export_blocks(self).len();
        if n > 0 {
            return Err(format!("Fix {} first", counted(n, "blocker", "blockers")));
        }
        // Note CN: the Also rows, after the blocks and before the overwrite questions (A9); nothing writes (7.1, 7.2).
        if self.also_runs() {
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
                format!("NIF edits written: {}", counted(n, "file", "files")),
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
            // Note FP1 (T29): a tick sends the click to Review (`export_checks`), so the button says Review.
            && !self.also_on()
    }

    /// E3: some Problems row counts (Plan 3a's `counts_as_ready_problem`: not a Check, Note, queued or intended
    /// row). Hidden rows count (hiding is a list filter, E18); only the scope's rows exist (HF10).
    pub fn has_problem_rows(&self) -> bool {
        super::bottom::problems(self)
            .iter()
            .any(super::bottom::counts_as_ready_problem)
    }

    /// E3, the one rule (under the current scope): the project opted out of Review and nothing needs the user's
    /// eyes, so Export writes at once. `unanswered_overwrites` is left out: its NIF half needs a written NIF row
    /// (`nif_rows_written`) and its JSON half an edited PGPatcher JSON, both refused here.
    pub fn writes_without_review(&self) -> bool {
        self.export_without_review()
            && !self.full_plan_pending()
            && self.export_files() > 0
            && self.review_gate().is_ok()
            && self.undecided() == 0
            && !self.nif_rows_written()
            // Not `pgp_json_edited()`: it hashes the file on every call; `review_json` is cached for 1 s.
            && !self
                .review_json()
                .iter()
                .any(|l| l.kind == JsonKind::PgPatcher && l.state == DiskState::Edited)
            && !self.also_runs()
            && !self.has_problem_rows()
    }

    /// E3 (ruling 3c-18): `writes_without_review` for a write about to start: the JSON state is read fresh (the 1 s
    /// cache is for the per-frame label). Once per dispatch, never per frame.
    pub(super) fn writes_without_review_fresh(&self) -> bool {
        self.review_json_cache.take();
        self.writes_without_review()
    }

    /// E3 (ruling 3c-14): `(writes_without_review, export_files)` of the whole project, whatever scope an open Review
    /// was made for (a header click clears the scope first). With no scope it is the live pair; with one it is read
    /// through `with_whole_view`, once per change of what it reads (the view key and the problems key), so the
    /// per-frame cost is one key compare. The JSON recheck (1 s) does not move the key: the click itself re-decides.
    fn whole_pair(&self) -> (bool, usize) {
        if self.export_scope().is_none() {
            return (self.writes_without_review(), self.export_files());
        }
        let key = (self.view_key(), super::bottom::problems_inner(self));
        if let Some((v, p, pair)) = &*self.whole_cache.borrow() {
            if (v, p) == (&key.0, &key.1) {
                return *pair;
            }
        }
        let pair = self.with_whole_view(|| (self.writes_without_review(), self.export_files()));
        *self.whole_cache.borrow_mut() = Some((key.0, key.1, pair));
        pair
    }

    /// `whole_pair().0`, without counting files when there is no scope.
    pub(super) fn whole_writes(&self) -> bool {
        if self.export_scope().is_none() {
            self.writes_without_review()
        } else {
            self.whole_pair().0
        }
    }

    /// `whole_pair().1`.
    pub(super) fn whole_files(&self) -> usize {
        if self.export_scope().is_none() {
            self.export_files()
        } else {
            self.whole_pair().1
        }
    }

    /// E3: the header label's write-now state. While a plan is pending it is the last settled answer, so the
    /// label never flips mid-plan (a partial plan lands without settling); else the live rule, whole project.
    pub fn header_writes_now(&self) -> bool {
        if self.full_plan_pending() {
            self.settled_writes_now
        } else {
            self.whole_writes()
        }
    }

    /// The file count the header label shows: the settled one while a write-now label is held, else the live one;
    /// the whole project's in both.
    fn header_files(&self, now: bool) -> usize {
        if now && self.full_plan_pending() {
            self.settled_export_files
        } else {
            self.whole_files()
        }
    }

    /// E3 (ruling 3c-17): the header draws the accent only when a click has something to write or review: the
    /// write-now answer or an open gate, and a count above zero (the label's files plus NIF edits).
    pub fn header_accent(&self) -> bool {
        self.accent_for(self.header_writes_now())
    }

    /// [`Self::header_accent`] for a `now` the caller already has (the header asks once per frame).
    pub fn accent_for(&self, now: bool) -> bool {
        (now || self.review_gate().is_ok()) && self.header_files(now) + self.nif_rows.len() > 0
    }

    /// E3: the header's Export button: `Review {n} files…`, or `Export {n} files now` when a click writes.
    pub fn header_export_text(&self) -> String {
        self.export_text(self.header_writes_now())
    }

    /// [`Self::header_export_text`] for a `now` the caller already has (the header asks once per frame).
    pub fn export_text(&self, now: bool) -> String {
        // Final review M5: with no folder open there is no count to give ("Review 0 files" greeted a new user).
        if self.root.is_none() {
            // Note FI18 (GH handoff item 3): pending NIF edits are counted as in FI7; there are no files to name.
            return match self.nif_rows.len() {
                0 => "Export".into(),
                rows => format!("Review {}…", nif_edits(rows)),
            };
        }
        // Only the held write-now label keeps the settled count; the Review label reads the decisions live.
        if now {
            return format!(
                "Export {} now",
                counted(self.header_files(true), "file", "files")
            );
        }
        let rows = self.nif_rows.len();
        let mut t = format!(
            "Review {}",
            counted(self.header_files(false), "file", "files")
        );
        // Note FI7: `Review` writes nothing, so it counts every pending NIF edit.
        if rows > 0 {
            t.push_str(&format!(" + {}", nif_edits(rows)));
        }
        t.push('…');
        t
    }

    /// Review's count under What will be written: "20 DDS files". Note JF1: with an Also tick the count is the PBR
    /// folder's and the other folders are named without one, "20 DDS files in the PBR folder, plus the Vanilla folder".
    pub fn written_text(&self) -> String {
        let n = self.export_files();
        let dds = counted(n, "DDS file", "DDS files");
        if self.also_runs() {
            format!(
                "{dds} in the PBR folder, plus the {}",
                self.also_extra_folders()
            )
        } else {
            dds
        }
    }

    /// "Export 20 files (skip 2 undecided)"; with an Also tick, "Export 20 files + Vanilla folder".
    pub fn review_button_text(&self) -> String {
        // Note GH8: no project, no texture run: the button writes the NIF edits alone.
        if self.root.is_none() {
            let n = self.nif_rows.iter().filter(|r| r.blocked.is_none()).count();
            return format!("Export {}", nif_edits(n));
        }
        let files = self.export_files();
        let skipped = self.undecided();
        // Note HF9: a scoped export's button says what Review is for (`Export group Armor`).
        let mut t = match self.export_scope() {
            Some(s) => s.label.clone(),
            None => format!("Export {}", counted(files, "file", "files")),
        };
        // Note JF1: the count is the PBR folder's; the folders copied or derived from it are named, not counted.
        if self.also_runs() {
            t.push_str(&format!(" + {}", self.also_extra_folders()));
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
