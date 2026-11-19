//! Review (layout v2 B2, C2, C5; rulings R1–R4): the pre-export gate as a bottom-panel tab, not a modal. It holds
//! only what the user answered; blockers and undecided textures are read live from the model.
use super::code_text::counted;
use super::history::{record_key, records_at, sha256_of};
use super::meshes::export::GuardChoice;
use super::{
    bottom::BottomTab, cm_json, cs_pbr_export, cs_skin_json, pbr_export, xpmf_json, AppModel,
    Continue, ExportPress, ItemStatus,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long `review_json` reuses its read of the JSON files. Tests expire it with `expire_review_json` instead of
/// sleeping past it.
pub const REVIEW_JSON_TTL: Duration = Duration::from_secs(1);

/// E3 (rulings 3c-24, 3c-28): the whole project's keyed terms, held while a scope is set: the export file count, the
/// export blocker count, the undecided count and the counted Problems rows (see `AppModel::whole_terms`).
#[derive(Debug, Clone, Copy)]
pub(super) struct WholeTerms {
    files: usize,
    blocks: usize,
    undecided: usize,
    problem_rows: bool,
}

/// E3 (ruling 3c-28): the whole project's filesystem-derived display terms while a scope is set, in one drawing cache:
/// re-read once per `REVIEW_JSON_TTL` and whenever the view key moves. The write decision never reads it.
pub(super) struct WholeDisk {
    key: super::ViewKey,
    at: Instant,
    /// The whole project's PGPatcher JSON line (one file read, a hash when edited).
    pgp: Option<JsonLine>,
    /// Why `sdc.toml` cannot be saved (`None`: writable), read whatever the scope: a tick runs on a whole-project
    /// export.
    toml_unwritable: Option<String>,
}

/// E3 (ruling 3c-28): what the header's Export button shows this frame, computed once (`AppModel::header_answer`):
/// whether a click writes at once, the file count its label names, and whether it draws the accent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderAnswer {
    pub now: bool,
    pub files: usize,
    pub accent: bool,
}

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

/// E3 (ruling 3c-28): how many times the write-now rule has been evaluated (a frame of the header evaluates it once).
#[cfg(feature = "test-hooks")]
pub fn rule_reads(m: &AppModel) -> u64 {
    m.rule_reads.get()
}

/// E3 (ruling 3c-36): how many times the header's whole-project drawing cache was read from disk (`WholeDisk`).
#[cfg(feature = "test-hooks")]
pub fn whole_disk_reads(m: &AppModel) -> u64 {
    m.whole_disk_reads.get()
}

/// E7: hovers of the files Review's footer lists besides the outputs.
pub const HOVER_SDC_TOML: &str = "Your texture settings. Export saves them first.";
pub const HOVER_PROJECT_TOML: &str =
    "Sets and export choices. Export saves them first, and records the hash of each JSON it writes.";
pub const HOVER_GENERATED: &str =
    "Tessalith's record of the JSON files it wrote, so the next export does not ask about them.";
pub const HOVER_MANIFEST: &str =
    "Tessalith's record of what it wrote. The next export reads it to skip files that did not change.";
pub const HOVER_COPY: &str = "A vanilla file a set borrows, copied so PGPatcher finds it.";
pub const HOVER_NIF: &str = "The NIF with your edits.";
pub const HOVER_NIF_RECORD: &str = "Tessalith's record of this NIF edit.";
pub const HOVER_ALSO_FOLDER: &str = "Every file in this folder is written or checked again.";

/// E7: one file (or, with an Also tick, one folder) the export changes besides its outputs, and what it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewWrite {
    pub path: PathBuf,
    pub hover: &'static str,
}

/// E7: every path the export touches. `outputs` are the DDS and JSON files; `also_updated` the rest, in the order
/// the footer names them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewWrites {
    pub outputs: Vec<PathBuf>,
    pub also_updated: Vec<ReviewWrite>,
}

impl ReviewWrites {
    /// The Also folder entries of `also_updated`: their files exist only after the engine plans them (JF1), so a
    /// changed path under one counts as listed.
    pub fn also_folders(&self) -> Vec<PathBuf> {
        self.also_updated
            .iter()
            .filter(|w| w.hover == HOVER_ALSO_FOLDER)
            .map(|w| w.path.clone())
            .collect()
    }
}

/// What `AppModel::review_writes` reads besides the disk (see `writes_key`).
#[derive(Clone, PartialEq)]
pub(super) struct WritesKey {
    view: super::ViewKey,
    project_rev: u64,
    project_unsaved: bool,
    doc_unsaved: bool,
    export_root: PathBuf,
    reencode_all: bool,
    nif_rows: (usize, u64),
    answer: Option<JsonAnswer>,
}

pub(super) struct WritesCache {
    key: WritesKey,
    at: Instant,
    value: Arc<ReviewWrites>,
}

/// The exports `review_json` prepared, kept beside its lines so `review_writes` reads the texts they would write
/// without preparing them again (ruling 3c-46).
pub(super) struct Prepared {
    pgp: Option<pbr_export::PendingJson>,
    cs: Option<cs_pbr_export::PendingCs>,
    cm: Option<cm_json::PendingCm>,
    xpmf: Option<xpmf_json::PendingXpmf>,
    skin: Option<cs_skin_json::PendingCsSkin>,
}

/// `review_json`'s cache: its view key, when it was read, its lines and what it prepared.
pub(super) type JsonCache = (super::ViewKey, Instant, Vec<JsonLine>, Arc<Prepared>);

/// E7: what one vanilla copy's plan was read under (the source's and the destination's stamps, the recorded hash and
/// whether this run writes the source), and what `pbr_export::copy_plan` answered. Only an answer of "copied" is
/// kept: a kept destination or a source that could not be read is asked again at the next build.
pub(super) type CopyMemo = std::collections::HashMap<
    PathBuf,
    (
        (
            Option<engine::policy::SourceStamp>,
            Option<engine::policy::SourceStamp>,
            Option<String>,
            bool,
        ),
        Option<(String, bool)>,
    ),
>;

/// Ruling 3c-46: the NIF export writes this row (and saves its record): not blocked, and, when the file at its export
/// path is not Tessalith's, answered Overwrite.
// shortcut: this is `MeshesModel::export_nif_edits`' own test, its three skip arms over the same row fields; it is a match
// inside that loop in a NIF-team file, with nothing callable. Upgrade to calling it when nif_edit exposes one.
fn nif_row_written(r: &super::meshes::NifReviewRow) -> bool {
    !matches!(
        (&r.blocked, r.foreign, r.answer),
        (Some(_), _, _) | (None, Some(_), None) | (None, Some(_), Some(GuardChoice::KeepTheirs))
    )
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
        // E7: the list Review opens with reads the disk as it is now, not a second-old answer.
        self.review_writes_cache.take();
        #[cfg(feature = "test-hooks")]
        {
            let w = self.review_writes();
            *self.review_writes_open.borrow_mut() = Some(w);
        }
    }

    /// Tests: the list of files Review showed when it opened (E7), kept after Review's Export takes `review`.
    #[cfg(feature = "test-hooks")]
    pub fn review_writes_at_open(&self) -> ReviewWrites {
        self.review_writes_open
            .borrow()
            .as_deref()
            .cloned()
            .unwrap_or_default()
    }

    /// Tests: how many times `review_writes` built its list (a cache hit builds nothing).
    #[cfg(feature = "test-hooks")]
    pub fn review_writes_builds(&self) -> u64 {
        self.review_writes_builds.get()
    }

    /// Tests: end `review_json`'s cache and the header's drawing cache now, as `REVIEW_JSON_TTL` passing would.
    #[cfg(feature = "test-hooks")]
    pub fn expire_review_json(&self) {
        self.review_json_cache.take();
        self.whole_disk.take();
        self.review_writes_cache.take();
    }

    /// E7: every file the export touches (the DDS and JSON outputs, and the files saved and recorded around them),
    /// for Review's footer. Cached like `review_json`: the `Arc` is rebuilt when the key moves or `REVIEW_JSON_TTL`
    /// passes (disk state), so a frame whose key is unchanged reads no file.
    pub fn review_writes(&self) -> Arc<ReviewWrites> {
        let key = self.writes_key();
        if let Some(c) = &*self.review_writes_cache.borrow() {
            if c.key == key && c.at.elapsed() < REVIEW_JSON_TTL {
                return c.value.clone();
            }
        }
        #[cfg(feature = "test-hooks")]
        self.review_writes_builds
            .set(self.review_writes_builds.get() + 1);
        let value = Arc::new(self.build_review_writes(&self.review_json_parts().1));
        *self.review_writes_cache.borrow_mut() = Some(WritesCache {
            key,
            at: Instant::now(),
            value: value.clone(),
        });
        value
    }

    /// Everything `build_review_writes` reads besides the disk. The Also ticks are `sdc.toml` text, so
    /// `project.rev()` carries them; the view key carries the plan and scope revisions and the output folder.
    fn writes_key(&self) -> WritesKey {
        use std::hash::{Hash, Hasher};
        // The NIF rows the export writes: their paths, whether each is blocked, its foreign file and its answer (ruling
        // 3c-46: a foreign row answered Keep is not written).
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for r in &self.nif_rows {
            r.path.hash(&mut h);
            r.blocked.is_none().hash(&mut h);
            r.foreign.hash(&mut h);
            r.answer.map(|a| a == GuardChoice::Overwrite).hash(&mut h);
        }
        WritesKey {
            view: self.view_key(),
            project_rev: self
                .project
                .as_ref()
                .map_or(0, crate::project::ProjectDoc::rev),
            project_unsaved: self.project_unsaved(),
            doc_unsaved: self.sets_unsaved(),
            export_root: self.export_root(),
            reencode_all: self.reencode_all,
            nif_rows: (self.nif_rows.len(), h.finish()),
            answer: self.review.as_ref().and_then(|r| r.json_answer.clone()),
        }
    }

    /// `save()` writes `sdc.toml`: it has unsaved edits, or a blame still holds some (Note FP7).
    fn project_unsaved(&self) -> bool {
        self.project
            .as_ref()
            .is_some_and(crate::project::ProjectDoc::is_dirty)
            || self.tentative_edits().is_some()
    }

    /// `save()` writes `sdc-project.toml`: unsaved edits in a document that takes them.
    fn sets_unsaved(&self) -> bool {
        self.sets_doc
            .as_ref()
            .is_some_and(|s| s.is_dirty() && !s.read_only())
    }

    /// Ruling 3c-46: every entry is decided by the predicate the export itself uses for that write, on the same inputs,
    /// as the disk will be after a clean run of this plan. The finishers run in `finish_export`'s order (copies, the
    /// PGPatcher JSON, CS PBR, complex material, XPMF, CS skin), each against Review's copy of the hash records, so the
    /// two record files are listed when a value the writer would store differs from the stored one.
    fn build_review_writes(&self, prep: &Prepared) -> ReviewWrites {
        use super::export::{Row, RowState};
        use super::json_finish::{plan_guarded, Guarded, UNKNOWN_SHA};
        // What this run writes: the plan's DDS outputs (the brief's broad rule until Task 8), each counted as done.
        let written_items = || {
            self.items
                .iter()
                .filter(|i| self.writes_a_file(i))
                .filter_map(|i| i.job.as_ref().map(|j| (i, j)))
        };
        // The Vanilla and CM folders of a whole-project Also run: the engine plans and writes their files.
        let also_folders: Vec<PathBuf> = if self.also_runs() {
            let folders = self.also_folders();
            let cm = self.also_ticks().1;
            [Some(folders[1].clone()), cm.then(|| folders[2].clone())]
                .into_iter()
                .flatten()
                .collect()
        } else {
            vec![]
        };
        let run: std::collections::HashSet<String> = written_items()
            .map(|(_, j)| record_key(&j.output))
            .collect();
        let written = |p: &Path| {
            run.contains(&record_key(p)) || also_folders.iter().any(|f| p.starts_with(f))
        };
        let rows: Vec<Row> = written_items()
            .filter(|(i, _)| !i.job_key.is_empty())
            .map(|(i, _)| Row {
                job_key: i.job_key.clone(),
                path: String::new(),
                state: RowState::UpToDate,
                warnings: vec![],
            })
            .collect();
        let doc = self.sets_doc.as_ref();
        let mut preview = doc.map(crate::sets_doc::SetsDoc::record_preview);
        // A record save (`save_record`) runs: some finisher recorded a hash.
        let mut saves = false;
        // (path, certain): the JSON files and copies whose bytes change.
        let mut json: Vec<(PathBuf, bool)> = vec![];
        let mut copies: Vec<PathBuf> = vec![];
        if let Some(p) = &prep.pgp {
            // `finish_json`: `copy_files` first, against the copy record.
            let old = doc.map(|d| d.copy_shas()).unwrap_or_default();
            let mut shas = old.clone();
            for (src, dst, owner_key) in &p.copies {
                if !owner_key.is_empty() && pbr_export::dep_ok(p, &rows, owner_key).is_err() {
                    continue;
                }
                let key = record_key(dst);
                let last = shas.get(&key).cloned();
                let Some(plan) = self.copy_plan(src, dst, last, written(src)) else {
                    continue;
                };
                match plan {
                    None => {
                        shas.insert(key, UNKNOWN_SHA.into());
                        copies.push(dst.clone());
                    }
                    Some((sha, changes)) => {
                        shas.insert(key, sha);
                        if changes {
                            copies.push(dst.clone());
                        }
                    }
                }
            }
            if let (Some(r), true) = (preview.as_mut(), shas != old) {
                r.set_copy_shas(&shas);
                saves = true;
            }
            // Then `pbr_export::finish` and `record_json_sha`.
            if let Some((text, _)) = pbr_export::json_text(p, &rows, &mut vec![]) {
                if self.pgp_writes(p, &text) {
                    if std::fs::read(&p.path).map_or(true, |b| b != text.as_bytes()) {
                        json.push((p.path.clone(), true));
                    }
                    if let (Some(r), Some(d)) = (preview.as_mut(), doc) {
                        use sha2::{Digest, Sha256};
                        let sha = hex::encode(Sha256::digest(text.as_bytes()));
                        let mut shas = records_at(&d.pgp_json_shas(), &p.root);
                        shas.insert(record_key(&p.path), sha.clone());
                        r.set_last_json_sha256(&sha);
                        r.set_pgp_json_shas(&shas);
                        saves = true;
                    }
                }
            }
        }
        // `finish_guarded` for each kind: written only with a project document, and recorded when the record moves.
        fn guarded<P: Guarded>(
            p: Option<&P>,
            doc: Option<&crate::sets_doc::SetsDoc>,
            preview: Option<&mut crate::sets_doc::RecordPreview>,
            written: &dyn Fn(&Path) -> bool,
            json: &mut Vec<(PathBuf, bool)>,
            saves: &mut bool,
        ) {
            let (Some(p), Some(d), Some(r)) = (p, doc, preview) else {
                return;
            };
            let recorded = P::recorded(d);
            let out = plan_guarded(p, &recorded, written, json);
            if out != recorded {
                P::preview(r, &out);
                *saves = true;
            }
        }
        guarded(
            prep.cs.as_ref(),
            doc,
            preview.as_mut(),
            &written,
            &mut json,
            &mut saves,
        );
        guarded(
            prep.cm.as_ref(),
            doc,
            preview.as_mut(),
            &written,
            &mut json,
            &mut saves,
        );
        guarded(
            prep.xpmf.as_ref(),
            doc,
            preview.as_mut(),
            &written,
            &mut json,
            &mut saves,
        );
        guarded(
            prep.skin.as_ref(),
            doc,
            preview.as_mut(),
            &written,
            &mut json,
            &mut saves,
        );

        let mut outputs: Vec<PathBuf> = written_items()
            .map(|(_, j)| j.output.clone())
            .chain(json.into_iter().map(|(p, _)| p))
            // A plan's output path can mix `/` and `\`; one path, one spelling.
            .map(|p| p.components().collect::<PathBuf>())
            // A file in a listed Also folder is counted by the folder's entry.
            .filter(|p| !also_folders.iter().any(|f| p.starts_with(f)))
            .collect();
        outputs.sort();
        outputs.dedup();
        let mut also: Vec<ReviewWrite> = vec![];
        let mut put = |path: PathBuf, hover: &'static str| also.push(ReviewWrite { path, hover });
        // `save()` first: `sdc.toml` when it holds unsaved edits.
        if let (Some(p), true) = (&self.project, self.project_unsaved()) {
            put(p.path().to_path_buf(), HOVER_SDC_TOML);
        }
        if let (Some(d), Some(r)) = (doc, &preview) {
            // `save()` writes unsaved edits; `save_record` writes the document's text after a record, which changes the
            // file when that text differs from the file's.
            let recorded = saves
                && !d.read_only()
                && std::fs::read(d.path()).ok().as_deref() != Some(r.text().as_bytes());
            if self.sets_unsaved() || recorded {
                put(d.path().to_path_buf(), HOVER_PROJECT_TOML);
            }
            // `put_side` writes the sidecar only for a value it does not hold.
            if r.side_changed() {
                put(d.sidecar_path(), HOVER_GENERATED);
            }
        }
        // The engine plans into `export/<root prefix>` (Note HQ1), and its manifest lives there.
        if let Some(root) = &self.root {
            let planned = engine::discover::effective_output_root(root, &self.output_root());
            put(planned.join("_sdc").join("manifest.jsonl"), HOVER_MANIFEST);
        }
        copies.sort();
        copies.dedup();
        for c in copies {
            put(c.components().collect(), HOVER_COPY);
        }
        // One loop per NIF row, so Plan 3e changes the record path in one place (its `row.record`).
        let out = self.output_root();
        for r in self.nif_rows.iter().filter(|r| nif_row_written(r)) {
            if let Ok(p) = super::meshes::edit_doc::export_path(&out, &r.path) {
                put(p, HOVER_NIF);
            }
            if let Ok(p) = super::meshes::edit_doc::doc_path(&out, &r.path) {
                put(p, HOVER_NIF_RECORD);
            }
        }
        // `n` is unknown before the engine plans a folder (JF1), so the entry says "all its files".
        for f in also_folders {
            put(f, HOVER_ALSO_FOLDER);
        }
        ReviewWrites {
            outputs,
            also_updated: also,
        }
    }

    /// `finish_json`'s decision for the PGPatcher JSON `text` (ruling 3c-46): `pbr_export::finish` writes a file that is
    /// absent, holds these bytes or is Tessalith's (`may_replace`); a file edited outside Tessalith only with Review's
    /// Overwrite answer for the bytes on disk now.
    fn pgp_writes(&self, p: &pbr_export::PendingJson, text: &str) -> bool {
        let last = self.pgp_record(&p.root, &p.path);
        match engine::dds::may_replace(
            &p.path,
            text.as_bytes(),
            super::history::recorded(last.as_deref()),
        ) {
            Ok(()) => true,
            Err(engine::dds::Kept::Foreign) => self
                .review
                .as_ref()
                .and_then(|r| r.json_answer.as_ref())
                .is_some_and(|a| {
                    a.overwrite
                        && a.path == p.path
                        && a.sha.is_some()
                        && sha256_of(&p.path) == a.sha
                }),
            Err(_) => false,
        }
    }

    /// `pbr_export::copy_plan`, kept per destination while both files' stamps, the recorded hash and whether this run
    /// writes the source are unchanged.
    fn copy_plan(
        &self,
        src: &Path,
        dst: &Path,
        last: Option<String>,
        src_written: bool,
    ) -> Option<Option<(String, bool)>> {
        use engine::policy::SourceStamp;
        let key = (
            SourceStamp::of(src),
            SourceStamp::of(dst),
            last,
            src_written,
        );
        if let Some((k, v)) = self.copy_sha_memo.borrow().get(dst) {
            if *k == key {
                return Some(v.clone());
            }
        }
        let plan = pbr_export::copy_plan(src, dst, key.2.as_deref(), src_written)?;
        self.copy_sha_memo
            .borrow_mut()
            .insert(dst.to_path_buf(), (key, plan.clone()));
        Some(plan)
    }

    /// C4: the JSON files the export would write and their state on disk. Cached (K3) per view key for
    /// `REVIEW_JSON_TTL`, since Review draws it every frame and it reads the files (AG128).
    pub fn review_json(&self) -> Vec<JsonLine> {
        self.review_json_parts().0
    }

    /// `review_json`'s lines and the exports it prepared for them (`review_writes` reads their texts, ruling 3c-46).
    fn review_json_parts(&self) -> (Vec<JsonLine>, Arc<Prepared>) {
        let key = self.view_key();
        if let Some((k, at, lines, prep)) = &*self.review_json_cache.borrow() {
            if *k == key && at.elapsed() < REVIEW_JSON_TTL {
                return (lines.clone(), prep.clone());
            }
        }
        let prep = Arc::new(Prepared {
            pgp: pbr_export::prepare(self),
            cs: cs_pbr_export::prepare(self),
            cm: cm_json::prepare(self),
            xpmf: xpmf_json::prepare(self),
            skin: cs_skin_json::prepare(self),
        });
        let mut lines = vec![];
        // K13: vanilla (and a project without a PBR set) writes no PGPatcher JSON.
        lines.extend(
            prep.pgp
                .as_ref()
                .filter(|p| !p.entries.is_empty())
                .map(|p| self.pgp_line(p)),
        );
        let doc = self.sets_doc.as_ref();
        let mut files = |kind, root: &Path, rels: Vec<&String>, shas| {
            // As the finishers read them: a relative record is under the export folder.
            let shas = records_at(&shas, root);
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
        if let (Some(p), Some(d)) = (&prep.cs, doc) {
            let rels = p.files.iter().map(|(r, _)| r).collect();
            files(JsonKind::CsPbr, &p.root, rels, d.cs_pbr_shas());
        }
        if let (Some(p), Some(d)) = (&prep.cm, doc) {
            let rels = p.files.iter().map(|(r, _)| r).collect();
            files(JsonKind::ComplexMaterial, &p.root, rels, d.cm_json_shas());
        }
        if let (Some(p), Some(d)) = (&prep.xpmf, doc) {
            for f in p.folders() {
                let rels = f.files.iter().map(|(r, _)| r).collect();
                files(JsonKind::Xpmf, &f.root, rels, d.xpmf_shas());
            }
        }
        if let (Some(p), Some(d)) = (&prep.skin, doc) {
            let rels = p.files.iter().map(|f| &f.rel).collect();
            files(JsonKind::CsSkin, &p.root, rels, d.cs_skin_shas());
        }
        *self.review_json_cache.borrow_mut() =
            Some((key, Instant::now(), lines.clone(), prep.clone()));
        (lines, prep)
    }

    /// The PGPatcher JSON this export would write and its state on disk (uncached: one file read).
    fn pgp_json_line(&self) -> Option<JsonLine> {
        let p = pbr_export::prepare(self).filter(|p| !p.entries.is_empty())?;
        Some(self.pgp_line(&p))
    }

    /// The line of the PGPatcher JSON `p` would write: its state on disk, and its hash when edited.
    fn pgp_line(&self, p: &pbr_export::PendingJson) -> JsonLine {
        let state = disk_state(&p.path, self.pgp_record(&p.root, &p.path).as_deref());
        let sha = (state == DiskState::Edited)
            .then(|| sha256_of(&p.path))
            .flatten();
        JsonLine {
            kind: JsonKind::PgPatcher,
            path: p.path.clone(),
            state,
            sha,
        }
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
        self.items.iter().filter(|i| self.writes_a_file(i)).count()
    }

    /// `i` is one of the files `export_files` counts.
    fn writes_a_file(&self, i: &super::TextureItem) -> bool {
        i.job.is_some()
            && self.item_in_run(i)
            && !matches!(
                i.status,
                ItemStatus::NeedsDecision
                    | ItemStatus::PackedInto(_)
                    | ItemStatus::Skipped(_)
                    | ItemStatus::Error(_)
            )
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
        self.gate_check(
            || pbr_export::export_blocks(self).len(),
            || {
                if self.also_runs() {
                    self.also_gate()
                } else {
                    Ok(())
                }
            },
            || self.unanswered_overwrites(),
        )?;
        Ok(if self.root.is_none() {
            (0, 0)
        } else {
            (self.export_files(), self.undecided())
        })
    }

    /// The gate's checks in A9's order. The caller says where its keyed term (the export blockers) comes from, what the
    /// Also check answers (`Ok` when no Also export runs), and where the unanswered overwrites come from: the
    /// whole-project header reads the same checks with its own sources (`whole_gate`), everything else live.
    fn gate_check(
        &self,
        blocks: impl FnOnce() -> usize,
        also: impl FnOnce() -> Result<(), String>,
        unanswered: impl Fn() -> Vec<String>,
    ) -> Result<(), String> {
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
            if let Some(name) = unanswered().first() {
                return Err(format!("Answer Keep or Overwrite for {name}"));
            }
            return Ok(());
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
        let n = blocks();
        if n > 0 {
            return Err(format!("Fix {} first", counted(n, "blocker", "blockers")));
        }
        // Note CN: the Also rows, after the blocks and before the overwrite questions (A9); nothing writes (7.1, 7.2).
        also()?;
        // Note FC5: then every overwrite question (supersedes CA7's "no choice made: asked after the export").
        if let Some(name) = unanswered().first() {
            return Err(format!("Answer Keep or Overwrite for {name}"));
        }
        Ok(())
    }

    /// Note FC5: the file names of the overwrite questions Review has no answer for: an edited PGPatcher JSON, then
    /// each NIF whose export path holds a file Tessalith did not write (a blocked NIF row is skipped, never asked).
    pub fn unanswered_overwrites(&self) -> Vec<String> {
        // shortcut: the header asks this every frame (export_is_clean), so `review_json` rereads the JSON files once a
        // second through its 1 s cache, even with no PGPatcher JSON edited (knowing that needs the same read); add a
        // cheap "any PBR set" check or an mtime key if a large CS PBR or CM set list makes that read show in a profile.
        let pgp = self
            .review_json()
            .into_iter()
            .find(|l| l.kind == JsonKind::PgPatcher);
        self.unanswered_with(pgp.as_ref())
    }

    /// `unanswered_overwrites` for a PGPatcher JSON line the caller read (the whole-project header reads its own).
    fn unanswered_with(&self, pgp: Option<&JsonLine>) -> Vec<String> {
        let answered = self.review.as_ref().and_then(|r| r.json_answer.as_ref());
        let name = |p: &Path| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        };
        let json = pgp.into_iter().filter(|l| {
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
        self.writes_rule(
            || self.export_files(),
            || self.review_gate().is_ok(),
            || self.undecided(),
            // Not `pgp_json_edited()`: it hashes the file on every call; `review_json` is cached for 1 s.
            || {
                self.review_json()
                    .iter()
                    .any(|l| l.kind == JsonKind::PgPatcher && l.state == DiskState::Edited)
            },
            self.also_runs(),
            || self.has_problem_rows(),
        )
    }

    /// The one rule's terms, in its order, each read only when the ones before it allow the write. The caller says
    /// where the expensive terms come from (live under the scope, or held for the whole project under one); the
    /// cheap, volatile ones (opt-out, pending plan, NIF rows, Also tick) are always read here, live.
    fn writes_rule(
        &self,
        files: impl FnOnce() -> usize,
        gate_ok: impl FnOnce() -> bool,
        undecided: impl FnOnce() -> usize,
        json_edited: impl FnOnce() -> bool,
        also: bool,
        problem_rows: impl FnOnce() -> bool,
    ) -> bool {
        #[cfg(feature = "test-hooks")]
        self.rule_reads.set(self.rule_reads.get() + 1);
        self.export_without_review()
            && !self.full_plan_pending()
            && files() > 0
            && gate_ok()
            && undecided() == 0
            && !self.nif_rows_written()
            && !json_edited()
            && !also
            && !problem_rows()
    }

    /// E3 (rulings 3c-18, 3c-27): `writes_without_review` for a write about to start, reading no cache whose value
    /// depends on the filesystem: the JSON state is read fresh (the 1 s cache is for the per-frame label), and so is
    /// the export blocker list (its target-mod overlap check resolves both folders on disk; a retargeted junction moves
    /// no key). Once per dispatch, never per frame.
    pub(super) fn writes_without_review_fresh(&self) -> bool {
        self.review_json_cache.take();
        // Not read by the decision; dropped so the next frame draws what the click saw.
        self.whole_disk.take();
        pbr_export::drop_blocks(self);
        self.writes_without_review()
    }

    /// E3 (rulings 3c-24, 3c-28): the whole project's keyed terms while a scope is set, read through `with_whole_view`
    /// once per change of the view key or the Problems key, so a stable frame pays a key compare: the export file
    /// count, the undecided count, the counted Problems rows and the export blocker count.
    /// The blocker count is as fresh as `export_blocks`' own cache, which the header has always read with no scope set:
    /// its target-mod overlap part resolves the folders on disk, so after a junction is retargeted with no project
    /// change it can be stale, with a scope or without. That is display only: the write decision drops that cache
    /// (ruling 3c-27).
    fn whole_terms(&self, view: &super::ViewKey) -> WholeTerms {
        let problems = super::bottom::problems_inner(self);
        if let Some((v, p, t)) = &*self.whole_cache.borrow() {
            if (v, p) == (view, &problems) {
                return *t;
            }
        }
        let t = self.with_whole_view(|| WholeTerms {
            files: self.export_files(),
            blocks: pbr_export::export_blocks(self).len(),
            undecided: self.undecided(),
            problem_rows: self.has_problem_rows(),
        });
        *self.whole_cache.borrow_mut() = Some((view.clone(), problems, t));
        t
    }

    /// E3 (ruling 3c-28): `f` over the whole project's filesystem-derived display terms (`WholeDisk`), re-read when
    /// `REVIEW_JSON_TTL` has passed or the view key moved: one PGPatcher JSON read and one `sdc.toml` stat per second
    /// at most while a scope is set. For drawing only; the write decision reads neither through it.
    fn with_whole_disk<T>(&self, view: &super::ViewKey, f: impl FnOnce(&WholeDisk) -> T) -> T {
        let fresh = self
            .whole_disk
            .borrow()
            .as_ref()
            .is_some_and(|w| w.key == *view && w.at.elapsed() < REVIEW_JSON_TTL);
        if !fresh {
            #[cfg(feature = "test-hooks")]
            self.whole_disk_reads.set(self.whole_disk_reads.get() + 1);
            let pgp = self.with_whole_view(|| self.pgp_json_line());
            let toml_unwritable = self.toml_unwritable_now();
            *self.whole_disk.borrow_mut() = Some(WholeDisk {
                key: view.clone(),
                at: Instant::now(),
                pgp,
                toml_unwritable,
            });
        }
        f(self.whole_disk.borrow().as_ref().expect("filled above"))
    }

    /// The whole project's gate as the header draws it (rulings 3c-21, 3c-28). With no scope it is `review_gate`.
    /// Under one: the same checks, with the keyed blocker count, and the whole project's Also check (a tick runs on a
    /// whole-project export) and PGPatcher JSON line from the drawing cache; every other part live. The drawing cache
    /// is read only once the cheap checks before it pass (ruling 3c-36): a missing folder or a known blocker pays no
    /// whole-project preparation.
    fn whole_gate(&self, whole: Option<&(super::ViewKey, WholeTerms)>) -> Result<(), String> {
        let Some((view, t)) = whole else {
            return self.review_gate().map(|_| ());
        };
        self.gate_check(
            || t.blocks,
            || {
                if self.also_on() {
                    let toml = self.with_whole_disk(view, |w| w.toml_unwritable.clone());
                    self.also_gate_with(toml.as_deref())
                } else {
                    Ok(())
                }
            },
            || self.with_whole_disk(view, |w| self.unanswered_with(w.pgp.as_ref())),
        )
    }

    /// E3 (ruling 3c-14): `writes_without_review` of the whole project, whatever scope an open Review was made for (a
    /// header click clears the scope first). With no scope it is the live rule. With one, the expensive terms are the
    /// keyed `whole_terms` and the JSON term comes from the drawing cache; everything volatile (opt-out, pending plan
    /// or read, NIF rows, Also tick) is read live by `writes_rule`. `gate_ok` is the caller's, so a frame reads the
    /// gate once for the rule and the accent.
    fn whole_rule(
        &self,
        whole: Option<&(super::ViewKey, WholeTerms)>,
        gate_ok: impl FnOnce() -> bool,
    ) -> bool {
        let Some((view, t)) = whole else {
            return self.writes_rule(
                || self.export_files(),
                gate_ok,
                || self.undecided(),
                || {
                    self.review_json()
                        .iter()
                        .any(|l| l.kind == JsonKind::PgPatcher && l.state == DiskState::Edited)
                },
                self.also_runs(),
                || self.has_problem_rows(),
            );
        };
        self.writes_rule(
            || t.files,
            gate_ok,
            || t.undecided,
            || {
                self.with_whole_disk(view, |w| {
                    w.pgp.as_ref().is_some_and(|l| l.state == DiskState::Edited)
                })
            },
            self.also_on(),
            || t.problem_rows,
        )
    }

    /// The whole project's view key and keyed terms while a scope is set; `None` without one (everything is live).
    fn whole(&self) -> Option<(super::ViewKey, WholeTerms)> {
        self.export_scope().is_some().then(|| {
            let view = self.view_key();
            let t = self.whole_terms(&view);
            (view, t)
        })
    }

    /// E3: what `hold_settled` keeps: the whole project's write-now answer and, only when it is true, its file count
    /// (the default project, which never writes at once, counts nothing per plan).
    pub(super) fn whole_writes_now(&self) -> (bool, usize) {
        let whole = self.whole();
        let whole = whole.as_ref();
        let writes = self.whole_rule(whole, || self.whole_gate(whole).is_ok());
        let files = match (writes, whole) {
            (false, _) => 0,
            (true, Some((_, t))) => t.files,
            (true, None) => self.export_files(),
        };
        (writes, files)
    }

    /// E3 (rulings 3c-14, 3c-17, 3c-21, 3c-28): the header's Export button for this frame, computed once and passed
    /// down. While a plan is pending the label is the last settled answer and count, so it never flips mid-plan (a
    /// partial plan lands without settling); else the live rule of the whole project. The accent shows when a click
    /// has something to write or review: the write-now answer or an open whole-project gate, and a count above zero
    /// (the label's files plus NIF edits). The rule and the gate are each read at most once.
    pub fn header_answer(&self) -> HeaderAnswer {
        let nifs = self.nif_rows.len();
        let pending = self.full_plan_pending();
        if pending && self.settled_writes_now {
            let files = self.settled_export_files;
            return HeaderAnswer {
                now: true,
                files,
                accent: files + nifs > 0,
            };
        }
        let whole = self.whole();
        let whole = whole.as_ref();
        let gate = std::cell::OnceCell::new();
        let gate_ok = || *gate.get_or_init(|| self.whole_gate(whole).is_ok());
        // Pending with a held Review answer: the rule would read false (pending) too, so it is not read.
        let now = !pending && self.whole_rule(whole, gate_ok);
        let files = whole.map_or_else(|| self.export_files(), |(_, t)| t.files);
        HeaderAnswer {
            now,
            files,
            accent: (now || gate_ok()) && files + nifs > 0,
        }
    }

    /// `header_answer().now`.
    pub fn header_writes_now(&self) -> bool {
        self.header_answer().now
    }

    /// `header_answer().accent`.
    pub fn header_accent(&self) -> bool {
        self.header_answer().accent
    }

    /// E3: the header's Export button: `Review {n} files…`, or `Export {n} files now` when a click writes.
    pub fn header_export_text(&self) -> String {
        self.export_text(&self.header_answer())
    }

    /// [`Self::header_export_text`] for the answer the header already has (it asks once per frame).
    pub fn export_text(&self, a: &HeaderAnswer) -> String {
        // Final review M5: with no folder open there is no count to give ("Review 0 files" greeted a new user).
        if self.root.is_none() {
            // Note FI18 (GH handoff item 3): pending NIF edits are counted as in FI7; there are no files to name.
            return match self.nif_rows.len() {
                0 => "Export".into(),
                rows => format!("Review {}…", nif_edits(rows)),
            };
        }
        // Only the held write-now label keeps the settled count; the Review label reads the decisions live.
        if a.now {
            return format!("Export {} now", counted(a.files, "file", "files"));
        }
        let rows = self.nif_rows.len();
        let mut t = format!("Review {}", counted(a.files, "file", "files"));
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
    /// A click run at once.
    pub fn export_from_review(&mut self) {
        self.review_export_pressed(self.export_press());
    }

    /// `export_from_review` for the press `press` (ruling 3c-38: Review's button can wait a frame).
    pub fn review_export_pressed(&mut self, press: ExportPress) {
        // Rulings 3c-37, 3c-38: before anything is queued or started.
        if self.press_refused(press) {
            return;
        }
        // Note GH8: Review's Export also works with no project (it writes the NIF edits alone).
        if self.review.is_none() {
            return;
        }
        self.status_note = None;
        // Ruling 3c-35: confirmed for the scope Review showed at the press (`press_refused`: the same as now).
        self.continue_review(Continue::Review {
            scope_rev: press.scope_rev,
        });
    }

    /// The front half of Review's Export (`how` keeps the scope it was confirmed for), also run when a scan-finished
    /// read it waited for lands.
    pub(super) fn continue_review(&mut self, how: Continue) {
        if self.planning.is_some() || self.votes_pending.is_some() {
            self.continue_after_plan = Some(how);
            return;
        }
        if self.check_sources() > 0 && self.session.is_some() {
            self.continue_after_plan = Some(how);
            self.replan();
            return;
        }
        self.run_continue(how);
    }
}
