//! Export and its report (M3a spec §2.5, §2.6, §5.5).
use super::bottom::open_export_warnings;
use super::AppModel;
use engine::api::{BatchSummary, CancelToken, CHECKED_IN_PLACE, CHECKED_IN_PLACE_FAILED};
use engine::codes::{Code, Warning};
use engine::events::{EngineEvent, EventKind, SkipKind};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub enum RowState {
    Queued,
    Running,
    Done {
        passed: bool,
    },
    UpToDate,
    /// Checked in place and failed (Note LF10): the file was kept and not written.
    CheckFailed,
    Kept,
    Skipped(SkipKind, String),
    Failed(String),
    Cancelled,
}

impl RowState {
    /// The drawer's one-word state; `Skipped` for an undecided texture says why.
    pub fn label(&self) -> String {
        match self {
            RowState::Queued => "queued".into(),
            RowState::Running => "running".into(),
            RowState::Done { passed: true } => "converted".into(),
            RowState::Done { passed: false } => "validation failed".into(),
            RowState::UpToDate => "up to date".into(),
            RowState::CheckFailed => "up to date, failed validation".into(),
            RowState::Kept | RowState::Skipped(SkipKind::OnExistingSkip, _) => "kept".into(),
            RowState::Skipped(SkipKind::Unresolved | SkipKind::Blocked, _) => {
                "skipped: undecided".into()
            }
            RowState::Skipped(_, reason) => format!("skipped: {reason}"),
            RowState::Failed(e) => format!("failed: {e}"),
            RowState::Cancelled => "cancelled".into(),
        }
    }

    /// A source skip can share its key with a recipe. Keep the recipe's terminal state if its skip arrives later.
    fn accepts_skip(&self) -> bool {
        match self {
            Self::Queued => true,
            Self::Skipped(kind, _) => {
                !matches!(kind, SkipKind::UnknownExisting | SkipKind::OnExistingSkip)
            }
            _ => false,
        }
    }

    /// The "Failed only" filter: a failed file or a validation failure.
    pub fn is_failed(&self) -> bool {
        matches!(
            self,
            RowState::Failed(_) | RowState::Done { passed: false } | RowState::CheckFailed
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub job_key: String,
    pub path: String,
    pub state: RowState,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ExportSummary {
    pub files: usize,
    pub textures: usize,
    pub generated: usize,
    pub sets: usize,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Terminal {
    Written,
    Current,
    Kept,
    Protected,
    Failed,
}

impl Terminal {
    fn from_event(kind: &EventKind) -> Option<Self> {
        match kind {
            EventKind::FileDone { validation, .. } if validation.passed => Some(Self::Written),
            EventKind::FileDone { .. } | EventKind::FileFailed { .. } => Some(Self::Failed),
            EventKind::FileSkipped { kind, reason, .. } => match kind {
                SkipKind::UpToDate if reason.ends_with(CHECKED_IN_PLACE_FAILED) => {
                    Some(Self::Failed)
                }
                SkipKind::UpToDate if reason.starts_with("kept:") => Some(Self::Kept),
                SkipKind::UpToDate => Some(Self::Current),
                SkipKind::OnExistingSkip => Some(Self::Kept),
                SkipKind::UnknownExisting => Some(Self::Protected),
                _ => None,
            },
            _ => None,
        }
    }
}

fn skipped_source(event: &EngineEvent) -> Option<&str> {
    match event.kind {
        EventKind::FileSkipped { kind, .. }
            if kind != SkipKind::PackedSource && Terminal::from_event(&event.kind).is_none() =>
        {
            event.path.as_deref()
        }
        _ => None,
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Outcomes {
    pub written: usize,
    pub current: usize,
    pub failed: usize,
    pub kept: usize,
    pub protected: usize,
    pub cancelled: usize,
    pub skipped_sources: usize,
}

impl Outcomes {
    fn line(&self) -> String {
        let mut line = format!(
            "{} written, {} up to date, {} failed",
            self.written, self.current, self.failed
        );
        for (n, label) in [
            (self.kept, "kept"),
            (self.protected, "protected"),
            (self.cancelled, "cancelled"),
        ] {
            if n > 0 {
                line.push_str(&format!(", {n} {label}"));
            }
        }
        if self.skipped_sources > 0 {
            line.push_str(&format!(
                ", {}",
                super::code_text::counted(
                    self.skipped_sources,
                    "skipped source",
                    "skipped sources"
                )
            ));
        }
        line
    }
}

#[derive(Debug, Default)]
pub struct ExportResult {
    pub primary: Outcomes,
    pub phases: Vec<(engine::also::Phase, Outcomes)>,
    pub copied: u32,
    pub phase_error: Option<String>,
}

pub fn result_line(result: &ExportResult) -> String {
    let mut line = result.primary.line();
    for (phase, outcomes) in &result.phases {
        line.push_str(&format!(
            " · {}: {}",
            super::also::phase_name(*phase),
            outcomes.line()
        ));
    }
    if result.copied > 0 {
        line.push_str(&format!(
            " · {}",
            super::code_text::counted(result.copied as usize, "Vanilla copy", "Vanilla copies")
        ));
    }
    if let Some(error) = &result.phase_error {
        line.push_str(&format!(" · {error}"));
    }
    line
}

#[derive(Default)]
pub struct ExportModel {
    pub running: Option<CancelToken>,
    /// In first-seen order, one per job key.
    pub rows: Vec<Row>,
    /// The stream without its per-mip `Progress` events, for JSON Lines export.
    pub events: Vec<EngineEvent>,
    pub summary: Option<BatchSummary>,
    pub result: Option<ExportResult>,
    /// Reviewed keys plus terminal keys reported by the primary writer.
    pub(super) primary_keys: HashMap<String, Option<Terminal>>,
    pub(super) primary_finished: usize,
    pub error: Option<String>,
    /// What the export did besides converting: the JSON, the vanilla copies, and what was left out.
    pub notes: Vec<String>,
    /// The export ran with Validate exported DDS on (its `BatchRequest::validate`); the report words follow it.
    pub validated: bool,
    /// Job key to the file the engine plans to write (or wrote): an absolute path with forward slashes.
    pub outputs: HashMap<String, String>,
    /// Progress timing of the running export (set when it starts).
    pub pace: Option<Pace>,
    /// Changes whenever a row, its state or its warnings change; the Files list caches by it.
    pub revision: u64,
    /// `job_key` to its index in `rows` (the drawer's per-event lookup; rebuilt when rows are removed).
    pub(crate) index: HashMap<String, usize>,
    /// Rows were removed since `index` was built: rebuild it at the next lookup (audit item 6).
    pub(crate) index_stale: bool,
    /// Note HF9: job keys (lower case) of textures outside this run. The plan still discovers them, but they get no
    /// row: the engine's `write_only` never runs them, so a row would end as "not run".
    pub(crate) outside_run: std::collections::HashSet<String>,
}

/// The source of `ExportModel::revision`: unique across models, so a cache never mistakes one for another.
static REV: AtomicU64 = AtomicU64::new(1);

fn slash(p: &str) -> String {
    p.replace('\\', "/")
}

/// `a == b` with `\` and `/` equal, without allocating.
fn same_path(a: &str, b: &str) -> bool {
    let sep = |c: u8| c == b'/' || c == b'\\';
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| x == y || (sep(x) && sep(y)))
}

impl ExportModel {
    /// Primary outputs only. A later Also phase or a discovery cannot change this denominator.
    pub fn progress(&self) -> (usize, usize) {
        let total = self.primary_keys.len();
        if self.summary.is_some() || self.error.is_some() {
            (total, total)
        } else {
            (self.primary_finished, total)
        }
    }

    pub(super) fn primary_outcomes(&self, cancelled: bool) -> Outcomes {
        let primary_batch = self.events.first().map(|e| &e.batch);
        let mut outcomes = Outcomes {
            skipped_sources: self
                .events
                .iter()
                .filter(|e| Some(&e.batch) == primary_batch)
                .filter_map(skipped_source)
                .collect::<HashSet<_>>()
                .len(),
            ..Default::default()
        };
        for outcome in self.primary_keys.values() {
            match outcome {
                Some(Terminal::Written) => outcomes.written += 1,
                Some(Terminal::Current) => outcomes.current += 1,
                Some(Terminal::Kept) => outcomes.kept += 1,
                Some(Terminal::Protected) => outcomes.protected += 1,
                None if cancelled => outcomes.cancelled += 1,
                Some(Terminal::Failed) | None => outcomes.failed += 1,
            }
        }
        outcomes
    }

    pub(super) fn secondary_outcomes(&self, summary: &BatchSummary, folder: &Path) -> Outcomes {
        let prefix = format!(
            "{}/",
            folder.file_name().unwrap_or_default().to_string_lossy()
        );
        let mut current = 0;
        let mut kept = 0;
        let mut protected = 0;
        let mut skipped_sources = HashSet::new();
        for ev in self.events.iter().filter(|e| {
            self.events
                .first()
                .is_some_and(|first| first.batch != e.batch)
                && e.job.as_ref().is_some_and(|k| k.starts_with(&prefix))
        }) {
            match Terminal::from_event(&ev.kind) {
                Some(Terminal::Current) => current += 1,
                Some(Terminal::Kept) => kept += 1,
                Some(Terminal::Protected) => protected += 1,
                _ => {}
            }
            skipped_sources.extend(skipped_source(ev));
        }
        Outcomes {
            written: (summary.converted - summary.validation_failed) as usize,
            // A failed check in place counts here and not under `current` (`Terminal::from_event`); it wrote nothing.
            failed: (summary.failed + summary.validation_failed + summary.checked_failed) as usize,
            current,
            kept,
            protected,
            cancelled: summary.cancelled.len(),
            skipped_sources: skipped_sources.len(),
        }
    }

    /// The file of the last running row, for the Export button's progress hover.
    pub fn current_file(&self) -> Option<&str> {
        self.rows
            .iter()
            .rev()
            .find(|r| r.state == RowState::Running)
            .map(|r| r.path.as_str())
    }

    /// Drop the queued rows of `path` (a raw map packed into a recipe).
    fn drop_queued(&mut self, path: &str) {
        let before = self.rows.len();
        self.rows
            .retain(|r| !(r.state == RowState::Queued && same_path(&r.path, path)));
        self.index_stale |= self.rows.len() != before;
    }

    fn row(&mut self, ev: &EngineEvent) -> Option<&mut Row> {
        if self.index_stale {
            self.reindex();
            self.index_stale = false;
        }
        let key = ev.job.clone()?;
        let i = match self.index.get(&key) {
            Some(&i) => i,
            None => {
                self.index.insert(key.clone(), self.rows.len());
                self.rows.push(Row {
                    job_key: key,
                    path: ev.path.clone().unwrap_or_default(),
                    state: RowState::Queued,
                    warnings: vec![],
                });
                self.rows.len() - 1
            }
        };
        Some(&mut self.rows[i])
    }

    fn reindex(&mut self) {
        self.index = self
            .rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.job_key.clone(), i))
            .collect();
    }

    /// A row for a job that runs: its path is the input the engine reports for the run, which for a packed output
    /// replaces the pre-packed file's path of an earlier `PACK-CONFLICT` skip.
    fn run_row(&mut self, ev: &EngineEvent, state: RowState) {
        let path = ev.path.clone();
        if let Some(r) = self.row(ev) {
            r.state = state;
            if let Some(p) = path {
                r.path = p;
            }
        }
    }

    pub fn on_event(&mut self, ev: EngineEvent) {
        self.on_event_at(ev, Instant::now());
    }

    /// [`Self::on_event`] with the clock given, so the pace can be tested.
    pub fn on_event_at(&mut self, ev: EngineEvent, now: Instant) {
        // Progress and the like change no row; every other event may, and the Files list re-filters on a new revision.
        if !matches!(
            ev.kind,
            EventKind::Progress { .. } | EventKind::BatchStarted { .. }
        ) {
            self.revision = REV.fetch_add(1, Ordering::Relaxed);
        }
        let primary_batch = self
            .events
            .first()
            .is_none_or(|first| first.batch == ev.batch);
        let terminal = Terminal::from_event(&ev.kind);
        match &ev.kind {
            EventKind::FileSkipped {
                kind: SkipKind::PackedSource,
                ..
            } => {
                // A raw map consumed by a recipe has no job of its own: drop the row its discovery made.
                if let Some(p) = ev.path.as_deref() {
                    self.drop_queued(p);
                }
            }
            EventKind::FileDiscovered { .. }
                if ev.job.as_ref().is_some_and(|j| self.outside(j)) => {}
            EventKind::FileDiscovered { output } => {
                self.row(&ev);
                self.note_output(&ev, output);
            }
            EventKind::FileStarted => self.run_row(&ev, RowState::Running),
            EventKind::FileDone { output, .. } => {
                let passed = terminal == Some(Terminal::Written);
                self.run_row(&ev, RowState::Done { passed });
                self.note_output(&ev, output);
                if primary_batch {
                    self.finished_at(now);
                }
            }
            EventKind::FileSkipped { kind, reason, .. } => {
                let state = match kind {
                    SkipKind::UpToDate if terminal == Some(Terminal::Failed) => {
                        RowState::CheckFailed
                    }
                    SkipKind::UpToDate if terminal == Some(Terminal::Kept) => RowState::Kept,
                    SkipKind::UpToDate => RowState::UpToDate,
                    k => RowState::Skipped(*k, reason.clone()),
                };
                // A skip is not work: it counts in done/total but not in the files/s rate or the ETA. A check in
                // place (Note LF10) is: it reads and decodes the DDS.
                if let Some(r) = self.row(&ev) {
                    if terminal.is_some() || r.state.accepts_skip() {
                        r.state = state;
                    }
                }
                if primary_batch && reason.contains(CHECKED_IN_PLACE) {
                    self.finished_at(now);
                }
            }
            EventKind::FileFailed { error, .. } => {
                let e = error.clone();
                self.run_row(&ev, RowState::Failed(e));
                if primary_batch {
                    self.finished_at(now);
                }
            }
            EventKind::Warning(w) => {
                let w = w.clone();
                if let Some(r) = self.row(&ev) {
                    if !r.warnings.contains(&w) {
                        r.warnings.push(w);
                    }
                }
            }
            EventKind::BatchDone { cancelled, .. } => {
                if let Some(p) = self.pace.as_mut() {
                    p.stop(now);
                }
                let cancelled: HashSet<String> = cancelled.iter().map(|c| slash(c)).collect();
                for r in self.rows.iter_mut().filter(|r| {
                    matches!(r.state, RowState::Queued | RowState::Running)
                        && cancelled.contains(&slash(&r.path))
                }) {
                    r.state = RowState::Cancelled;
                }
            }
            // Audit lead 10: one Progress per mip per file is not kept (a 10k batch held ~120k); rows and pace never
            // read `events`, so only the saved JSON Lines report loses its progress lines.
            EventKind::Progress { .. } => return,
            _ => {}
        }
        if let (true, Some(key), Some(outcome)) = (primary_batch, ev.job.as_ref(), terminal) {
            let entry = self.primary_keys.entry(key.clone()).or_default();
            if entry.replace(outcome).is_none() {
                self.primary_finished += 1;
            }
        }
        self.events.push(ev);
    }

    fn note_output(&mut self, ev: &EngineEvent, output: &str) {
        if let Some(k) = &ev.job {
            self.outputs.insert(k.clone(), output.to_string());
        }
    }

    fn finished_at(&mut self, now: Instant) {
        if let Some(p) = self.pace.as_mut() {
            p.finished(now);
        }
    }

    /// Note HF9: job key `k` is a texture outside the run: its own key, or an Also phase's `<folder>/<key>`.
    fn outside(&self, k: &str) -> bool {
        let k = k.to_lowercase();
        self.outside_run.contains(&k)
            || k.split_once('/')
                .is_some_and(|(_, rest)| self.outside_run.contains(rest))
    }

    /// Note HF9: drop the rows that never started whose texture is outside the run. A queued row of a texture in the
    /// run stays, to end as "not run".
    pub(crate) fn drop_unstarted(&mut self) {
        let before = self.rows.len();
        let outside: Vec<bool> = self.rows.iter().map(|r| self.outside(&r.job_key)).collect();
        let mut i = 0;
        self.rows.retain(|r| {
            i += 1;
            r.state != RowState::Queued || !outside[i - 1]
        });
        if self.rows.len() != before {
            self.index_stale = true;
            self.revision = REV.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Note CN: end every row still queued or running when an Also export ends: failed with `why` ("not run: …"),
    /// else cancelled after a cancel, else failed "not run".
    pub fn end_unfinished(&mut self, cancelled: bool, why: Option<&str>) {
        let state = match why {
            Some(w) => RowState::Failed(format!("not run: {w}")),
            None if cancelled => RowState::Cancelled,
            None => RowState::Failed("not run".into()),
        };
        let mut any = false;
        for r in self
            .rows
            .iter_mut()
            .filter(|r| matches!(r.state, RowState::Queued | RowState::Running))
        {
            r.state = state.clone();
            any = true;
        }
        if any {
            self.revision = REV.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Warnings grouped by code: code → job keys (M3a spec §2.5).
    pub fn by_code(&self) -> BTreeMap<Code, Vec<String>> {
        let mut m: BTreeMap<Code, Vec<String>> = BTreeMap::new();
        for r in &self.rows {
            for w in &r.warnings {
                let keys = m.entry(w.code).or_default();
                if !keys.contains(&r.job_key) {
                    keys.push(r.job_key.clone());
                }
            }
        }
        m
    }

    pub fn to_jsonl(&self) -> String {
        self.events
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .map(|l| l + "\n")
            .collect()
    }

    pub fn to_markdown(&self) -> String {
        let mut s = String::from("# Tessalith export report\n\n");
        if let Some(b) = &self.summary {
            let validation = match (self.validated, b.validation_failed) {
                (true, n) => format!("validation failed {n}"),
                (false, 0) => "not validated".to_string(),
                (false, n) => format!("validation failed {n} (complex types only)"),
            };
            let checked = if self.validated && b.checked > 0 {
                format!(
                    ", checked in place {} ({} failed)",
                    b.checked, b.checked_failed
                )
            } else {
                String::new()
            };
            s += &format!(
                "Converted {}, up to date {}, skipped {}, failed {}, {validation}{checked}.\n\n",
                b.converted, b.up_to_date, b.skipped, b.failed
            );
        }
        // Audit 08c-4: what the export did besides converting (a JSON not written or kept, the copies).
        if !self.notes.is_empty() {
            s += "## Export details\n\n";
            for n in &self.notes {
                s += &format!("- {n}\n");
            }
            s += "\n";
        }
        s += "## Warnings by code\n\n";
        for (code, keys) in self.by_code() {
            s += &format!(
                "- **{}** ({}): {}\n",
                code.as_str(),
                keys.len(),
                keys.join(", ")
            );
        }
        s += "\n## Files\n\n| Output | State |\n|---|---|\n";
        for r in &self.rows {
            s += &format!("| {} | {} |\n", r.job_key, r.state.label());
        }
        s
    }
}

/// Note JA6: the export's kept outputs in GUI words (no `--ev`), naming only the counts above 0, or `None` when it kept
/// none. One of the export notes, so the Files tab and the Markdown report show it.
pub fn kept_note(s: &BatchSummary) -> Option<String> {
    let parts: Vec<String> = [
        (s.kept_older_encode, "from an older encode version"),
        (s.kept_lower_target, "at a higher quality target"),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, why)| format!("{n} kept {why}"))
    .collect();
    // An output kept for both reasons counts in both (JA6), so the summary cannot tell one output from two when both
    // counts are 1. Name the object only when it is known: "them" if a count is above 1, "it" if the one count is 1.
    let (a, b) = (s.kept_older_encode, s.kept_lower_target);
    let again = match (a.max(b), a + b) {
        (m, _) if m > 1 => " them",
        (_, 1) => " it",
        _ => "",
    };
    (!parts.is_empty()).then(|| {
        format!(
            "{}: tick Re-encode every output (Tools) to encode{again} again",
            parts.join(", ")
        )
    })
}

/// Review Focus 2: discovery skips everything under the output root, so the export folder must not be the opened
/// folder or contain it. A subfolder of the opened folder is fine. P19: one rule, the engine's (TA19 uses it too).
pub fn export_folder_problem(root: &Path, export: &Path) -> Option<String> {
    engine::also::export_folder_problem(root, export)
}

/// The `sdc convert` command that reproduces the export from the saved `sdc.toml` (M3a spec §2.5). The GUI plans
/// with High suggestions accepted (Note U3), so the command does too (audit 08c-2). Set edits in
/// `sdc-project.toml` and mesh evidence are not carried: the CLI reads neither. `--validate` follows the Validate
/// exported DDS setting as it is now (Note LF); like the rest of the command it is rebuilt from the current saved
/// state, not read from the export the Files tab shows.
pub fn cli_command(root: &Path, export: &Path, validate: bool) -> String {
    let q = |p: &Path| format!("\"{}\"", p.to_string_lossy().replace('\\', "/"));
    format!(
        "sdc convert {} --recursive -o {} --accept-suggestions high{}",
        q(root),
        q(export),
        if validate { " --validate" } else { "" }
    )
}

// ---- Task 5: progress line, Files filters and caches (DECISIONS AG35-AG37, AG39) ----

/// The files/s average looks back this far.
pub const RATE_WINDOW: Duration = Duration::from_secs(10);
/// No ETA before this many files have finished ...
pub const ETA_MIN_FILES: usize = 3;
/// ... or this much time has passed.
pub const ETA_MIN_TIME: Duration = Duration::from_secs(2);

/// The timing of one export: when it started and when each file finished. Every method takes the time as an argument
/// (the model passes `Instant::now()`), so the rules are tested with a fake clock.
#[derive(Debug, Clone)]
pub struct Pace {
    start: Instant,
    end: Option<Instant>,
    /// Finish times of the last `RATE_WINDOW`, oldest first.
    recent: VecDeque<Instant>,
}

impl Pace {
    pub fn new(start: Instant) -> Self {
        Pace {
            start,
            end: None,
            recent: VecDeque::new(),
        }
    }

    pub fn finished(&mut self, now: Instant) {
        self.recent.push_back(now);
        while self
            .recent
            .front()
            .is_some_and(|t| now.saturating_duration_since(*t) > RATE_WINDOW)
        {
            self.recent.pop_front();
        }
    }

    /// The export ended: the elapsed time stops.
    pub fn stop(&mut self, now: Instant) {
        self.end.get_or_insert(now);
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        self.end
            .unwrap_or(now)
            .saturating_duration_since(self.start)
    }

    /// Files finished per second over the last `RATE_WINDOW` (or since the start, while that is shorter).
    pub fn rate(&self, now: Instant) -> f64 {
        let span = self.elapsed(now).min(RATE_WINDOW).as_secs_f64();
        let n = self
            .recent
            .iter()
            .filter(|t| now.saturating_duration_since(**t) <= RATE_WINDOW)
            .count();
        if span <= 0.0 {
            0.0
        } else {
            n as f64 / span
        }
    }

    /// `None` until `ETA_MIN_FILES` have finished or `ETA_MIN_TIME` has passed, and while nothing finished lately.
    pub fn eta(&self, done: usize, total: usize, now: Instant) -> Option<Duration> {
        if done < ETA_MIN_FILES && self.elapsed(now) < ETA_MIN_TIME {
            return None;
        }
        let rate = self.rate(now);
        (rate > 0.0).then(|| Duration::from_secs_f64(total.saturating_sub(done) as f64 / rate))
    }

    /// `done/total · elapsed · files/s · ETA`; the ETA is left out while it is `None`.
    pub fn line(&self, done: usize, total: usize, now: Instant) -> String {
        let mut s = format!(
            "{done}/{total} · {} · {:.1} files/s",
            fmt_duration(self.elapsed(now)),
            self.rate(now)
        );
        if let Some(eta) = self.eta(done, total, now) {
            s += &format!(" · ETA {}", fmt_duration(eta));
        }
        s
    }
}

/// `m:ss`, or `h:mm:ss` from an hour on; a fraction of a second rounds up (an ETA never reads 0:00 early).
pub fn fmt_duration(d: Duration) -> String {
    let s = d.as_secs_f64().ceil() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// The drawer's two Files filters; both on means both must hold.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FileFilter {
    pub failed_only: bool,
    pub has_warnings: bool,
}

impl FileFilter {
    pub fn any(self) -> bool {
        self.failed_only || self.has_warnings
    }

    pub fn matches(self, m: &AppModel, r: &Row) -> bool {
        (!self.failed_only || r.state.is_failed())
            && (!self.has_warnings || open_export_warnings(m, r) > 0)
    }
}

/// The visible row indices depend on the filter, the export, the live items and their intended marks.
#[derive(Default)]
pub struct FilesIndex {
    key: Option<(FileFilter, u64, u64, u64)>,
    indices: Vec<usize>,
    /// How many times the filter was computed (tests).
    pub computed: u32,
}

impl FilesIndex {
    /// How many rows pass `f`.
    pub fn len(&mut self, m: &AppModel, f: FileFilter) -> usize {
        if !f.any() {
            return m.export.rows.len();
        }
        let key = (f, m.export.revision, m.items_rev(), m.doc_rev());
        if self.key != Some(key) {
            self.computed += 1;
            self.indices.clear();
            self.indices
                .extend((0..m.export.rows.len()).filter(|&i| f.matches(m, &m.export.rows[i])));
            self.key = Some(key);
        }
        self.indices.len()
    }

    /// The `i`th row that passes `f`; call [`Self::len`] first in the frame.
    pub fn row<'a>(&self, m: &'a AppModel, f: FileFilter, i: usize) -> Option<&'a Row> {
        let i = if f.any() { *self.indices.get(i)? } else { i };
        m.export.rows.get(i)
    }
}

/// A warning as a Files row's hover shows it: `CODE (severity): message`.
pub fn warning_line(w: &Warning) -> String {
    format!(
        "{} ({}): {}",
        w.code.as_str(),
        format!("{:?}", w.severity).to_lowercase(),
        w.message
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Note HF9 (review round 1, Minor 2): only queued rows of textures outside the run go; a queued row of one in
    /// the run stays to end as "not run".
    #[test]
    fn drop_unstarted_drops_only_rows_outside_the_run() {
        let row = |k: &str| Row {
            job_key: k.into(),
            path: k.into(),
            state: RowState::Queued,
            warnings: vec![],
        };
        let mut m = ExportModel {
            rows: vec![row("g/A"), row("g/b"), row("Mod Vanilla/g/a")],
            outside_run: ["g/a".to_string()].into(),
            ..Default::default()
        };
        // The discovered rows use the same rule (final review M6): an Also phase's row gets none either.
        assert!(m.outside("Mod Vanilla/g/A") && m.outside("g/a") && !m.outside("g/b"));
        m.drop_unstarted();
        let left: Vec<&str> = m.rows.iter().map(|r| r.job_key.as_str()).collect();
        assert_eq!(left, ["g/b"]);
    }

    fn event(seq: u64, key: &str, kind: EventKind) -> EngineEvent {
        EngineEvent {
            schema: engine::events::EVENT_SCHEMA,
            seq,
            ts: "2026-11-21T00:00:00.000Z".into(),
            batch: "b".into(),
            set: None,
            path: Some("a.png".into()),
            job: Some(key.into()),
            kind,
        }
    }

    /// An export model that discovered `a/x.dds` and then got `reason` as its up-to-date skip.
    fn up_to_date_with(reason: String) -> ExportModel {
        let mut m = ExportModel::default();
        let found = EventKind::FileDiscovered {
            output: "a/x.dds".into(),
        };
        m.on_event(event(0, "a/x.dds", found));
        let skip = EventKind::FileSkipped {
            kind: SkipKind::UpToDate,
            code: None,
            reason,
        };
        m.on_event(event(1, "a/x.dds", skip));
        m
    }

    /// Note LF10: an output checked in place that failed is a failed row, in the Failed only filter, the
    /// Files counts and the report. One that passed, and a keep, are not.
    #[test]
    fn a_failed_check_in_place_is_a_failed_row() {
        use engine::api::{CHECKED_IN_PLACE, CHECKED_IN_PLACE_FAILED};
        let m = up_to_date_with(format!("up to date; {CHECKED_IN_PLACE_FAILED}"));
        assert_eq!(m.rows[0].state, RowState::CheckFailed);
        assert!(m.rows[0].state.is_failed());
        assert_eq!(m.rows[0].state.label(), "up to date, failed validation");
        let o = m.primary_outcomes(false);
        assert_eq!((o.failed, o.current), (1, 0));
        assert!(m
            .to_markdown()
            .contains("| a/x.dds | up to date, failed validation |"));

        let m = up_to_date_with(format!("up to date; {CHECKED_IN_PLACE}"));
        assert_eq!(m.rows[0].state, RowState::UpToDate);
        assert!(!m.rows[0].state.is_failed());
        let o = m.primary_outcomes(false);
        assert_eq!((o.failed, o.current), (0, 1));

        // A keep is never checked (Note LF11).
        let m = up_to_date_with("kept: x".into());
        assert_eq!(m.rows[0].state, RowState::Kept);
    }

    /// Note LF: in an Also phase a failed check in place counts once, under `failed` (from the summary), and not under
    /// `current` (the up-to-date event's terminal is `Failed`).
    #[test]
    fn an_also_phase_counts_a_failed_check_once() {
        use engine::api::CHECKED_IN_PLACE_FAILED;
        let mut m = ExportModel::default();
        // The first batch is the primary one; the phase below is another batch under the `v` folder.
        m.on_event(event(
            0,
            "a/p.dds",
            EventKind::FileDiscovered {
                output: "a/p.dds".into(),
            },
        ));
        let mut skip = event(
            1,
            "v/a/x.dds",
            EventKind::FileSkipped {
                kind: SkipKind::UpToDate,
                code: None,
                reason: format!("up to date; {CHECKED_IN_PLACE_FAILED}"),
            },
        );
        skip.batch = "phase".into();
        m.on_event(skip);
        let summary = BatchSummary {
            up_to_date: 1,
            checked: 1,
            checked_failed: 1,
            ..Default::default()
        };
        let o = m.secondary_outcomes(&summary, Path::new("x/v"));
        assert_eq!((o.failed, o.current), (1, 0));
    }

    /// Note LF: with the switch off, a complex-type failure is still reported as one, and `not validated` says nothing
    /// was checked; with it on, the in-place checks are counted.
    #[test]
    fn the_report_shows_mandatory_failures_when_not_validating() {
        let mut m = ExportModel {
            validated: false,
            summary: Some(BatchSummary {
                validation_failed: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        let md = m.to_markdown();
        assert!(
            md.contains("validation failed 1 (complex types only)"),
            "{md}"
        );
        assert!(!md.contains("not validated"), "{md}");
        m.summary = Some(BatchSummary::default());
        assert!(m.to_markdown().contains("not validated"));
        m.validated = true;
        m.summary = Some(BatchSummary {
            checked: 2,
            checked_failed: 1,
            ..Default::default()
        });
        let md = m.to_markdown();
        assert!(md.contains("checked in place 2 (1 failed)"), "{md}");
        assert!(md.contains("validation failed 0"), "{md}");
    }
}
