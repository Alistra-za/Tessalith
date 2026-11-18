//! The bottom panel's state (layout v2 A1, A9; R7, R8): which tab shows, and whether the panel is open.
use super::code_text::{note_sentence, row_class, RowClass};
use super::cs_pbr::{set_key, statics_text, Kind, StaticsStatus, NO_TXST_USES};
use super::items::{self, Decision, ItemKind};
use super::meshes::{NifLevel, NifTab};
use super::nav::SlotTarget;
use super::{
    checks, code_text, counts_toward_badge, makes_a_row, pbr_export, sets, AppModel, ItemStatus,
};
use crate::pgp::settings::Feature;
use crate::sets_doc::{normalise_sentence, IntendedKey};
use engine::codes::Code;
use nif::fields::Field;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BottomTab {
    Problems,
    Review,
    Messages,
    #[default]
    Files,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BottomPanel {
    pub open: bool,
    pub tab: BottomTab,
    pub filter: Option<ProblemsFilter>,
}

/// Note HC4: a row its kind can hide. A block or a question never hides, and hiding never touches the Review gate,
/// which reads `export_blocks`, never `problems`.
pub fn can_hide(p: &Problem) -> bool {
    p.code.is_some() && matches!(p.level, Level::Warning | Level::Info)
}

/// Note HC4: each kind the project's rows can hide and its row count, by code name.
pub fn kinds(m: &AppModel) -> Vec<(engine::codes::Code, usize)> {
    let mut out = std::collections::BTreeMap::new();
    for p in problems(m).iter().filter(|p| can_hide(p)) {
        if let Some(c) = p.code {
            out.entry(c.as_str()).or_insert((c, 0)).1 += 1;
        }
    }
    out.into_values().collect()
}

impl AppModel {
    /// The `gui.toml` key of this project's hidden kinds (Note HC4).
    fn problems_key(&self) -> Option<String> {
        let root = self.root.as_ref()?;
        Some(root.to_string_lossy().replace('\\', "/").to_lowercase())
    }

    /// The kinds of row hidden in this project (Note HC4); empty without a project.
    pub fn hidden_kinds(&self) -> Vec<engine::codes::Code> {
        let names = self
            .problems_key()
            .and_then(|k| self.settings.problems_hidden.get(&k));
        let Some(names) = names else {
            return vec![];
        };
        engine::codes::Code::ALL
            .iter()
            .copied()
            .filter(|c| names.contains(c.as_str()))
            .collect()
    }

    /// Hide or show one kind of row in this project; `gui.toml` keeps it.
    pub fn set_kind_hidden(&mut self, code: engine::codes::Code, hidden: bool) {
        let Some(key) = self.problems_key() else {
            return;
        };
        let names = self
            .settings
            .problems_hidden
            .entry(key.clone())
            .or_default();
        if hidden {
            names.insert(code.as_str().to_string());
        } else {
            names.remove(code.as_str());
        }
        if names.is_empty() {
            self.settings.problems_hidden.remove(&key);
        }
        self.save_settings();
    }

    /// Open the panel on `tab`.
    pub fn show_tab(&mut self, tab: BottomTab) {
        self.bottom = BottomPanel {
            open: true,
            tab,
            filter: None,
        };
        self.sync_messages_open();
    }

    /// Open on `tab`, or collapse when the panel already shows it.
    pub fn toggle_tab(&mut self, tab: BottomTab) {
        if self.bottom.open && self.bottom.tab == tab {
            self.bottom.open = false;
        } else {
            self.bottom = BottomPanel {
                open: true,
                tab,
                filter: None,
            };
        }
        self.sync_messages_open();
    }

    pub fn collapse_bottom(&mut self) {
        self.bottom.open = false;
        self.sync_messages_open();
    }

    /// R8: `Messages::is_open` means "the Messages tab is showing".
    fn sync_messages_open(&mut self) {
        let showing = self.bottom.open && self.bottom.tab == BottomTab::Messages;
        self.messages.set_open(showing);
    }
}

/// Red blocks the export; Decide and Warning are amber (A12). `Check` rows (artist checks, DECISIONS Note CL) and
/// `Info` rows are drawn weak and never change "ready" (`counts_as_ready_problem`, Note FC3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Blocks,
    Decide,
    Warning,
    Check,
    /// Note FC3: a result with an action and no consequence for the export (a dropped fill, a cleared format).
    Info,
    /// Review E2: a note whose fix is already queued for the next export.
    Queued,
}

/// Whether a row takes "ready" away: every row but an artist check's (owner answer 1), a note (Note FC3, review E2)
/// and a row the user marked as intended.
pub fn counts_as_ready_problem(p: &Problem) -> bool {
    !matches!(p.level, Level::Check | Level::Info | Level::Queued) && !p.intended
}

/// Review E2: the mark of a row at `at` of code (or kind) `kind` that shows `shown`. The subject names where the row
/// is; the sentence is `normalise_sentence`'s, so a set row's own id in front is left out.
pub fn intended_key(at: &At, kind: &str, shown: &str) -> IntendedKey {
    let subject = match at {
        At::Texture(k) => format!("texture:{k}"),
        At::Set { id, .. } => format!("set:{id}"),
        At::File(k) => format!("file:{k}"),
        At::Project => "project".into(),
        At::Nif(p) => format!("nif:{p}"),
    };
    IntendedKey {
        code: kind.into(),
        sentence: normalise_sentence(&subject, shown),
        subject,
    }
}

/// Review E2 (controller ruling 7): a row that gets **Mark as intended**: a warning or check row whose code, if any,
/// is a warning or an Info result. A file that fails validation (Error, Blocking) is exported again, not marked.
pub fn markable(level: Level, code: Option<Code>) -> bool {
    use engine::codes::Severity;
    matches!(level, Level::Warning | Level::Check)
        && code.is_none_or(|c| matches!(c.severity(), Severity::Warning | Severity::Info))
}

/// Review E2: whether warning `w` at `at` takes "ready" away: it counts toward the badge, is no artist check (they
/// count in the tab only, Note CL), and has no mark on the sentence its row shows, so a tile and its row agree.
pub fn counts_for(m: &AppModel, at: &At, w: &engine::codes::Warning) -> bool {
    if !counts_toward_badge(w) || checks::check_of(w.code).is_some() {
        return false;
    }
    !marked(m, at, w)
}

/// The text a row of the item warning `w` is built from, `job` being the item's: the engine's message, but for
/// GRASS-DETECT-SOURCE under `mips.normal_base = keep` its own sentence (`code_text::GRASS_SOURCE_KEPT`; the written
/// file is not renormalised then). The row, its mark and `marked` read it, so a tile and its row agree.
pub fn warning_text<'a>(
    job: Option<&engine::policy::ResolvedJob>,
    w: &'a engine::codes::Warning,
) -> std::borrow::Cow<'a, str> {
    let kept = job.is_some_and(|j| j.profile.mips.normal_base == engine::config::NormalBase::Keep);
    if w.code == Code::GrassDetectSource && kept {
        code_text::GRASS_SOURCE_KEPT.into()
    } else {
        w.message.as_str().into()
    }
}

/// The job of the item a row at `at` is about: a texture's, or a last-export file's when exactly one item maps to it
/// (as the File rows' `live`, Note DP4).
fn job_at<'m>(m: &'m AppModel, at: &At) -> Option<&'m engine::policy::ResolvedJob> {
    match at {
        At::Texture(k) => m.item(k)?.job.as_ref(),
        At::File(jk) => {
            let mut of = m.items.iter().filter(|i| i.job_key == *jk);
            let one = of.next()?;
            of.next().is_none().then_some(one)?.job.as_ref()
        }
        _ => None,
    }
}

/// Review E25: the queued note of an exported short normal with Rebuild Z now on, and the Rebuild Z button's hover
/// once it is on (final fix wave C3: one string for both).
pub const QUEUED_REBUILD_Z: &str = "Rebuild Z is on for this file: export again to rewrite it";

/// Final fix wave B3: the kinds a row with no code of its own is marked under (`[project] intended`'s `code` in
/// `sdc-project.toml`). They are stored in users' project files: a changed string orphans their marks.
pub mod kind {
    pub const NIF_WARNING: &str = "NIF-WARNING";
    pub const PGP_PITFALL: &str = "PGP-PITFALL";
    pub const SET_MISSING_ENTRY: &str = "SET-MISSING-ENTRY";
    pub const RECORD_CHANGED: &str = "RECORD-CHANGED";
    pub const XPMF_SKIP: &str = "XPMF-SKIP";
    pub const SKIN_SKIP: &str = "SKIN-SKIP";
    pub const READ_ONLY_FLAG: &str = "READ-ONLY-FLAG";
    /// A skipped user preset's rows; the same string as the engine's `Code::CfgPresetSkipped`.
    pub const CFG_PRESET_SKIPPED: &str = "CFG-PRESET-SKIPPED";
    /// Every kind above.
    pub const ALL: [&str; 8] = [
        NIF_WARNING,
        PGP_PITFALL,
        SET_MISSING_ENTRY,
        RECORD_CHANGED,
        XPMF_SKIP,
        SKIN_SKIP,
        READ_ONLY_FLAG,
        CFG_PRESET_SKIPPED,
    ];
}

/// Final fix wave A6: the line a hover puts before the warnings and check results marked as intended, which it lists
/// after the others (the tile, the Files row and the card's ⓘ).
pub const MARKED_LINE: &str = "Marked as intended:";

/// A hover's lines: `open`, then `MARKED_LINE` and `marked` when there are any.
pub fn with_marked(mut open: Vec<String>, marked: Vec<String>) -> Vec<String> {
    if !marked.is_empty() {
        open.push(MARKED_LINE.to_string());
        open.extend(marked);
    }
    open
}

/// Review E4: the project holds the mark of the row warning `w` at `at` makes (`intended_key` of its shown sentence).
/// Final fix wave A6: only a code that can be marked (Warning or Info, as `markable`) reads one; a hand-written mark on
/// an Error code is not one.
pub fn marked(m: &AppModel, at: &At, w: &engine::codes::Warning) -> bool {
    use engine::codes::Severity;
    if !matches!(w.code.severity(), Severity::Warning | Severity::Info) {
        return false;
    }
    let Some(doc) = m.sets_doc.as_ref() else {
        return false;
    };
    // Tiles and cards ask every frame: only a warning whose code a mark names builds its key. This holds because a
    // key's `code` is stored and compared verbatim: `sets_doc::folded` folds only the `subject`. A fold of the code
    // there must fold it here too.
    doc.intended_keys()
        .iter()
        .any(|k| k.code == w.code.as_str())
        && {
            let text = warning_text(job_at(m, at), w);
            let shown = code_text::shown_for(Some(w.code), &text);
            doc.is_intended(&intended_key(at, w.code.as_str(), &shown))
        }
}

/// Final fix wave A6: whether the last export's warning `w` on the file `job_key` is marked, read where its row is: on
/// the texture's row when the one live item of that file has a warning of the same code (the File loop of
/// `build_problems` leaves the warning to it), else on the File row.
pub fn export_warning_marked(m: &AppModel, job_key: &str, w: &engine::codes::Warning) -> bool {
    let mut of = m.items.iter().filter(|i| i.job_key == job_key);
    let first = of.next();
    let live = first.filter(|_| of.next().is_none());
    let on_texture = live
        .filter(|i| i.status != ItemStatus::NeedsDecision)
        .and_then(|i| i.warnings.iter().find(|x| x.code == w.code).map(|x| (i, x)));
    match on_texture {
        Some((i, x)) => marked(m, &At::Texture(i.key.clone()), x),
        None => marked(m, &At::File(job_key.to_string()), w),
    }
}

/// Review E4: a row the Checks pill counts and its filter lists: a check row not marked as intended (a marked one is
/// a note, listed under Notes).
pub fn counts_as_check(p: &Problem) -> bool {
    p.level == Level::Check && !p.intended
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum At {
    /// An item key.
    Texture(String),
    Set {
        key: String,
        id: String,
    },
    /// A job key.
    File(String),
    Project,
    /// Note FC3: a NIF, by its data-relative path.
    Nif(String),
}

/// Note FC3: a row's action, drawn as its button. Each arm is built by the row kind that needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// Note DU3: remove the set entry that matches no file.
    RemoveMissing(sets::Missing),
    /// CHAN-NORMAL-LENGTH (Note DP): set `channels.reconstruct_z` on this item.
    RebuildZ(String),
    /// IN-UNREADABLE (Y4): open the folder that holds the skipped entry.
    OpenFolder(PathBuf),
    /// CHK-GREEN-GUESS (CL5): answer this item's green convention as OpenGL.
    TreatAsOpenGl(String),
    /// P26: turn one of two exclusive features of a set (by id) off; what a `FeatureClash` button asks for.
    TurnFeatureOff(String, Feature),
    /// P26, owner 2026-11-04: two exclusive features of a set (by id) are both on. The row offers one
    /// `TurnFeatureOff` button per feature, neither a default; the modder picks which one goes.
    FeatureClash(String, Feature, Feature),
    /// P13: go to the set (by key) and focus the path field (a settings key) the blocker names.
    FixPath(String, String),
    /// R-fills-2, Z11: undo the step that caused the row; only built while that step is the top of the history.
    UndoLast,
    /// Note GD1: open the NIF on this tab and field.
    ShowNifTab(String, NifTab, Option<Field>),
    /// Note FC1 (AM14): open the header's Preset menu.
    ChoosePreset,
    /// Note FC10 (P8): open the unreadable `sdc-project.toml` with the OS default program; the row also has `Reload`.
    OpenInEditor(PathBuf),
    /// Note FC10 (P8): reload both documents (asks first when something is unsaved), as the banner's Reload.
    Reload,
    /// Note FC10 (item 27): clear the read-only flag of this project file.
    ClearReadOnly(PathBuf),
    /// Note FC10 (X10, X3): write `subtype = "complex_grass"` for this item.
    TagAsGrass(String),
    /// Note FC10 (BQ15): write `alpha.exr_straight = true` for this item.
    StraightAlpha(String),
    /// Note FC10 (CV8): remove the stored empty `match{n}` of the set (by id).
    RemoveMatch(String, u8),
    /// Note FC10 (T8): drop the `suffix_aliases` table of a skipped user preset file (the file, the preset's id).
    RemoveAliases(PathBuf, String),
    /// Note HG: copy the winning record's current paths into the set's (by id) mapping.
    UseRecordPaths(String),
    /// Note JI6: remove these unmatched `[files]` entries, one undo step. The row carries every entry; its
    /// `Entries ▾` menu asks for one at a time.
    RemoveUnmatched(Vec<String>),
    /// Review E10 (ALPHA-PARALLAX-NO-ALPHA): write `alpha.parallax = false` for this item.
    ParallaxOff(String),
    /// Review E10 (SHADER-EXTRA-MAP): take the member `item` (an item key) out of the set `set_id`.
    RemoveMember { set_id: String, item: String },
    /// Review E10 (SHADER-UNAVAILABLE, SHADER-MISSING-MAP, SHADER-EXTRA-MAP): pick another shader for the set (by id).
    ChangeShader(String),
    /// Review E10 (SUG-CONTRADICTION): the undecided rows' type picker, for this item.
    SetType(String),
    /// Note JP3 (SHADER-NORMAL-SPECULAR): write subtype = "pbr_normal" for this item.
    UsePbrNormal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub level: Level,
    pub at: At,
    pub text: String,
    pub code: Option<engine::codes::Code>,
    /// Codes the text names in words (a failed texture's, a question's); only the row's hover prints them (Note FB8).
    pub also: Vec<engine::codes::Code>,
    /// Note FC3: the row's action button (the Decide rows' buttons and `Go to set` are drawn from the level and `at`).
    pub fix: Option<Fix>,
    /// Review E10 (controller ruling 2): a second button, drawn after `fix`; `Some` only when `fix` is.
    pub second_fix: Option<Fix>,
    /// Note FB9: a Decide row's suggested sub-type, which its ✓ button, Enter and Space accept.
    pub suggested: Option<engine::config::Subtype>,
    /// Note FC1: more for the row's hover (the combined row lists what it covers), one line each.
    pub detail: Option<String>,
    /// Note FL: a block or ask row, checked on the last full plan (`AppModel::fix_off_for`).
    pub carried: bool,
    /// Review E2: the user marked this row as intended, so it is a note.
    pub intended: bool,
    /// Review E2: the key **Mark as intended** and **Undo mark** write; `Some` exactly on a `markable` row.
    pub mark: Option<IntendedKey>,
    /// Review E10: the slot or file a set row's Go to set outlines; `None` lands on the set alone.
    pub target: Option<SlotTarget>,
}

impl Problem {
    /// The codes behind the row, for its hover and nowhere else: `CODE, CODE`.
    pub fn codes_text(&self) -> Option<String> {
        let all: Vec<_> = self
            .code
            .iter()
            .chain(&self.also)
            .map(|c| c.as_str())
            .collect();
        (!all.is_empty()).then(|| all.join(", "))
    }

    /// What the row shows (Note FB8): the code's action sentence, else the engine's text with any code taken out. A
    /// queued row (review E25) says what is queued, not the code's action.
    pub fn shown(&self) -> std::borrow::Cow<'_, str> {
        if self.level == Level::Queued {
            return self.text.as_str().into();
        }
        code_text::shown_for(self.code, &self.text)
    }

    /// Review E2: a row that needs nothing from the user: an Info or queued row, or one marked as intended.
    pub fn is_note(&self) -> bool {
        matches!(self.level, Level::Info | Level::Queued) || self.intended
    }

    /// Review E2 (controller ruling 5): a row of the default list. A note with a button stays; a row the user marked
    /// leaves it, fix or not.
    pub fn in_worklist(&self) -> bool {
        !self.is_note() || (self.fix.is_some() && !self.intended)
    }
}

/// Review E2: what the Problems tab's label counts. A hidden note counts nowhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TabCounts {
    pub problems: usize,
    pub hidden: usize,
    pub notes: usize,
    /// Review E18: the hidden kinds that hide at least one of the `hidden` problem rows.
    pub hidden_kinds: usize,
}

/// Note HC4: a row that one of the `hidden` kinds takes out of the list; the tab counts and the list share it.
pub fn is_hidden(p: &Problem, hidden: &[Code]) -> bool {
    can_hide(p) && p.code.is_some_and(|k| hidden.contains(&k))
}

/// The tab counts of `all` with the kinds in `hidden` hidden (`is_hidden`, as the list).
pub fn tab_counts(all: &[Problem], hidden: &[Code]) -> TabCounts {
    let mut c = TabCounts::default();
    // The few hidden kinds seen on a problem row, in the same pass.
    let mut kinds: Vec<Code> = vec![];
    for p in all {
        match (p.is_note(), is_hidden(p, hidden)) {
            (true, true) => {}
            (true, false) => c.notes += 1,
            (false, true) => {
                c.hidden += 1;
                if let Some(k) = p.code.filter(|k| !kinds.contains(k)) {
                    kinds.push(k);
                }
            }
            (false, false) => c.problems += 1,
        }
    }
    c.hidden_kinds = kinds.len();
    c
}

/// `tab_counts`, scanned once per new `problems()` build or hidden-kinds change (review E2; ui-rules §9.1). The cache
/// holds the counted `Arc`, so `Arc::ptr_eq` can't match a new list at a freed address.
pub fn cached_tab_counts(m: &AppModel) -> TabCounts {
    // Both before the borrow: `problems` writes the same cell.
    let all = problems(m);
    let hidden = m.hidden_kinds();
    if let Some((_, _, c)) = m
        .views
        .borrow()
        .tab_counts
        .as_ref()
        .filter(|(p, h, _)| Arc::ptr_eq(p, &all) && *h == hidden)
    {
        return *c;
    }
    #[cfg(feature = "test-hooks")]
    m.tab_count_scans.set(m.tab_count_scans.get() + 1);
    let c = tab_counts(&all, &hidden);
    m.views.borrow_mut().tab_counts = Some((all, hidden, c));
    c
}

/// The Problems tab's label: `Problems (n)`, `Problems (n · h hidden)`, then ` · Notes (m)` when there are notes.
pub fn tab_label(c: TabCounts) -> String {
    let problems = match (c.problems, c.hidden, c.notes) {
        (0, 0, 0) => "Problems".to_string(),
        (n, 0, _) => format!("Problems ({n})"),
        (n, h, _) => format!("Problems ({n} · {h} hidden)"),
    };
    match c.notes {
        0 => problems,
        m => format!("{problems} · Notes ({m})"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemsFilter {
    /// A set's scope key (`scope_key`: its key, or a key-less set's id).
    Set(String),
    Undecided,
    /// Only the artist checks' rows.
    Checks,
    /// Only the artist checks' rows of one set (a set's scope key (`scope_key`: its key, or a key-less set's id)):
    /// "Checks {n}" pressed while `Set` is on.
    SetChecks(String),
    /// Review E2: every note row.
    Notes,
    /// Review E2: every note row of one set (a set's scope key (`scope_key`: its key, or a key-less set's id)): the
    /// Notes pill pressed while `Set` is on.
    SetNotes(String),
}

impl AppModel {
    /// Open the Problems tab, narrowed to `f`.
    pub fn open_problems(&mut self, f: Option<ProblemsFilter>) {
        self.show_tab(BottomTab::Problems);
        self.bottom.filter = f;
    }

    /// Note FC3: the NIF edit's problems, handed over by `app.rs` each frame. Only a new snapshot rebuilds Problems.
    pub fn set_nif_problems(&mut self, p: Arc<Vec<super::meshes::NifProblem>>) {
        if !Arc::ptr_eq(&self.nif_problems, &p) {
            self.nif_problems = p;
            self.invalidate_views();
        }
    }
}

/// Every problem of the project: blocks, then decisions, then warnings, then checks, then the last export's File rows
/// (warnings, or checks for an engine `CHK-*` code). Cached (K9) under the views key plus
/// what the asks and the File rows read; the build holds no `RefCell` borrow.
pub fn problems(m: &AppModel) -> Arc<Vec<Problem>> {
    #[cfg(feature = "test-hooks")]
    m.problem_lookups.set(m.problem_lookups.get() + 1);
    let inner = problems_inner(m);
    super::cached(
        m,
        |c| {
            c.problems
                .as_ref()
                .filter(|(k, _)| *k == inner)
                .map(|(_, p)| p.clone())
        },
        |c, p| {
            c.problems = Some((inner, p));
            c.flagged = None;
        },
        || {
            #[cfg(feature = "test-hooks")]
            m.problem_builds.set(m.problem_builds.get() + 1);
            Arc::new(build_problems(m))
        },
    )
}

/// What `problems` is keyed on inside the view-key cell; `export_files` shares it.
pub(super) fn problems_inner(m: &AppModel) -> super::ProblemsKey {
    (
        m.evidence_rev,
        m.export.revision,
        m.export.running.is_some(),
        // Note FC3: an `Undo` fix is built only while its step is the top one.
        m.history.pushes,
        // Note HL6: the statics and texture-set rows read the scan database.
        m.scan_stamp,
    )
}

/// The set keys and texture keys that carry a problem that takes "ready" away, and the row counts by that test.
#[derive(Debug, Default)]
pub struct Flagged {
    pub sets: HashSet<String>,
    /// Note JP5: the ids of key-less sets with a counted set row (`sets` holds keys only, so an id never meets a key).
    pub keyless: HashSet<String>,
    pub textures: HashSet<String>,
    /// Rows passing `counts_as_ready_problem` (the Export dot and its hover, K6).
    pub ready_rows: usize,
    /// `Level::Check` rows (whether the Problems tab shows its `Checks` pill).
    pub check_rows: usize,
}

/// `Flagged`, built once per `problems` build (R-a: a set card asks every frame). Fetch it once per frame and ask
/// `set_ready_in` per card (Task 6).
pub fn flagged(m: &AppModel) -> Arc<Flagged> {
    let all = problems(m);
    if let Some(f) = &m.views.borrow().flagged {
        return f.clone();
    }
    let mut f = Flagged::default();
    for p in all.iter() {
        if !counts_as_ready_problem(p) {
            f.check_rows += usize::from(counts_as_check(p));
            continue;
        }
        f.ready_rows += 1;
        match &p.at {
            At::Set { key, id } if key.is_empty() => f.keyless.insert(id.clone()),
            At::Set { key, .. } => f.sets.insert(key.clone()),
            At::Texture(t) => f.textures.insert(t.clone()),
            _ => false,
        };
    }
    let f = Arc::new(f);
    m.views.borrow_mut().flagged = Some(f.clone());
    f
}

/// A set card shows a green dot: a real set whose own rows and members' rows are all absent from Problems.
pub fn set_ready_in(f: &Flagged, v: &sets::SetView) -> bool {
    !v.key.is_empty()
        && !f.sets.contains(&v.key)
        && !v.members.iter().any(|t| f.textures.contains(t))
}

/// Note JP5 (B3-16, B3-19): the name a set's Problems filter, its reasons, the card's hover and its badges' clicks use:
/// its engine key, or for a set with no key (a created set with no file yet, or one whose every entry is missing) its
/// id.
pub fn scope_key(v: &sets::SetView) -> &str {
    if v.key.is_empty() {
        &v.id
    } else {
        &v.key
    }
}

/// Note JP5 (B3-17): a set with no key and no counted row of its own, whose status line and card hover say it has no
/// files yet.
pub fn no_files_yet(f: &Flagged, v: &sets::SetView) -> bool {
    v.key.is_empty() && !f.keyless.contains(&v.id)
}

/// `set_ready_in` with its own `flagged` fetch; a loop over sets fetches once and calls `set_ready_in`.
pub fn set_ready(m: &AppModel, v: &sets::SetView) -> bool {
    set_ready_in(&flagged(m), v)
}

/// A tile shows a green dot: ready, with no warning that `counts_for` it (review E4: a note or a marked row never
/// keeps it from Ready).
pub fn tile_ready(m: &AppModel, item: &super::items::TextureItem) -> bool {
    item.status == ItemStatus::Ready && !item_warned(m, item)
}

/// Review E4: `item` has a warning that takes "ready" away (`counts_for`): the tile's ⚠, the "Has warnings" pill and
/// filter, and the Status sort. Its `At` is built only when a warning could count.
pub fn item_warned(m: &AppModel, item: &super::items::TextureItem) -> bool {
    item.warnings.iter().any(counts_toward_badge) && {
        let at = At::Texture(item.key.clone());
        item.warnings.iter().any(|w| counts_for(m, &at, w))
    }
}

/// Review E4: artist-check result `w` at `at` is on its set card's ⓘ badge: not marked as intended, as its row
/// (`counts_as_check`). The mark test is `counts_for`'s, with its code prefilter (`marked`: a key's code is compared
/// verbatim, `sets_doc::folded` folds only the subject), so a project with no mark on a check code builds no key.
pub fn check_listed(m: &AppModel, at: &At, w: &engine::codes::Warning) -> bool {
    !marked(m, at, w)
}

#[cfg(feature = "test-hooks")]
pub fn filtered_builds(m: &AppModel) -> u64 {
    m.filter_builds.get()
}

#[cfg(feature = "test-hooks")]
pub fn problems_builds(m: &AppModel) -> u64 {
    m.problem_builds.get()
}

/// Every call to `problems()`, cached or not (A6).
#[cfg(feature = "test-hooks")]
pub fn problem_lookups(m: &AppModel) -> u64 {
    m.problem_lookups.get()
}

/// Every `tab_counts` scan `cached_tab_counts` ran (review E2), not its cached answers.
#[cfg(feature = "test-hooks")]
pub fn tab_count_scans(m: &AppModel) -> u64 {
    m.tab_count_scans.get()
}

/// Every `set_reasons` scan `cached_set_reasons` ran (final fix wave R2), not its cached answers.
#[cfg(feature = "test-hooks")]
pub fn reason_scans(m: &AppModel) -> u64 {
    m.reason_scans.get()
}

/// `AppModel::export_files` counts that were not served from the cache.
#[cfg(feature = "test-hooks")]
pub fn export_files_builds(m: &AppModel) -> u64 {
    m.export_files_builds.get()
}

fn build_problems(m: &AppModel) -> Vec<Problem> {
    let views = sets::views(m);
    let set_at = |key: &str, id: &str| At::Set {
        key: key.into(),
        id: id.into(),
    };
    let row = |level, at, text: String, code| Problem {
        level,
        at,
        text,
        code,
        also: vec![],
        fix: None,
        second_fix: None,
        suggested: None,
        detail: None,
        carried: false,
        intended: false,
        mark: None,
        target: None,
    };
    // Review E2: a row with no code is marked by its kind, named where it is built.
    let kinded = |p: Problem, kind: &str| Problem {
        mark: Some(intended_key(&p.at, kind, &p.shown())),
        ..p
    };
    // Review E2: the Info row of a warning that is a note, for the item, set and File loops alike; `None` for a
    // problem. FC7 (CN3), controller ruling 4: a `_curves`/cavity map no recipe reads is Also export's input, not a
    // mistake, so it keeps its own row with no code and no button, ahead of the general note.
    // shortcut: matched on the shared `ALSO_ONLY` text; give the warning a reason flag if a second row needs one.
    let note_row = |at: &At, w: &engine::codes::Warning| {
        if w.code == Code::PackUnusedInput && w.message.contains(engine::pack::ALSO_ONLY) {
            let text = "Read only by Also export; not written on its own".to_string();
            return Some(row(Level::Info, at.clone(), text, None));
        }
        // The code stays, so Kinds can still hide the row.
        (row_class(w.code) == RowClass::Note)
            .then(|| row(Level::Info, at.clone(), note_sentence(w), Some(w.code)))
    };
    let mut out = vec![];
    // Note FC1 (AM14): the plan runs on the fallback so the window is not empty, but the export waits for a preset.
    if let Some((text, detail)) = m.missing_preset_text() {
        out.push(Problem {
            fix: Some(Fix::ChoosePreset),
            detail,
            ..row(Level::Blocks, At::Project, text, None)
        });
    }
    // Note FC10 (P8): an unreadable `sdc-project.toml` is a red row with Open in editor and Reload; sets stay
    // read-only until it is fixed. The row names the parser's first line; the hover has all of it.
    if let Some((path, e)) = m
        .sets_doc
        .as_ref()
        .filter(|s| s.read_only())
        .and_then(|s| Some((s.path(), s.error()?)))
    {
        let e = e
            .strip_prefix(&format!("{}: ", path.display()))
            .unwrap_or(e);
        let first = e.lines().next().unwrap_or(e);
        out.push(Problem {
            fix: Some(Fix::OpenInEditor(path.to_path_buf())),
            detail: Some(e.to_string()),
            ..row(
                Level::Blocks,
                At::Project,
                format!("sdc-project.toml cannot be read: {first}"),
                None,
            )
        });
    }
    for b in pbr_export::export_blocks(m).iter().cloned() {
        let at = if b.set_key.is_empty() {
            At::Project
        } else {
            set_at(&b.set_key, &b.set_id)
        };
        out.push(Problem {
            fix: b.fix,
            carried: true,
            ..row(Level::Blocks, at, b.text, None)
        });
    }
    // Note FC3 (GD1): the NIF edit's rows, each with its level's group. A NIF Info is a row too: it has an action.
    let nif_rows = |want: NifLevel, level: Level| {
        m.nif_problems
            .iter()
            .filter(move |n| n.level == want)
            .map(move |n| {
                let p = Problem {
                    fix: Some(Fix::ShowNifTab(n.path.clone(), n.tab, n.field)),
                    ..row(level, At::Nif(n.path.clone()), n.text.clone(), None)
                };
                kinded(p, kind::NIF_WARNING)
            })
    };
    out.extend(nif_rows(NifLevel::Blocks, Level::Blocks));
    for i in &m.items {
        let (text, also) = match &i.status {
            ItemStatus::Failed(r) => (format!("Failed: {r}"), vec![]),
            ItemStatus::Error(cs0) => {
                let cs: Vec<_> = cs0.iter().map(|c| code_text::code_line(*c)).collect();
                (format!("Failed: {}", cs.join("; ")), cs0.clone())
            }
            _ => continue,
        };
        out.push(Problem {
            also,
            ..row(Level::Blocks, At::Texture(i.key.clone()), text, None)
        });
    }
    for i in m
        .items
        .iter()
        .filter(|i| i.status == ItemStatus::NeedsDecision && m.item_in_run(i))
    {
        // Note FM35: the alpha question is one line on the type it opens on.
        let alpha = items::alpha_asked(i).map(code_text::alpha_confirm_text);
        let parts: Vec<String> = i
            .decisions
            .iter()
            .map(|d| match d {
                Decision::Subtype { .. } => "sub-type".into(),
                Decision::AlphaMeaning => alpha.clone().unwrap_or_else(|| "alpha".into()),
                Decision::NotACubeSource => {
                    format!("not a cube source ({})", engine::cube::plan::CUBE_SOURCES)
                }
                // Note FB8: the sentence names the action; the code is on the hover.
                Decision::Acknowledge(c) => code_text::code_line(*c).into(),
                Decision::Change(c) => code_text::change_line(*c).into(),
            })
            .collect();
        let mut also: Vec<Code> = i
            .decisions
            .iter()
            .filter_map(|d| match d {
                Decision::Acknowledge(c) | Decision::Change(c) => Some(*c),
                _ => None,
            })
            .collect();
        // The alpha line answers LOSS-ALPHA-DATA too; the hover still names it.
        if alpha.is_some() && i.warnings.iter().any(|w| w.code == Code::LossAlphaData) {
            also.push(Code::LossAlphaData);
        }
        let text = match (&alpha, parts.len()) {
            (Some(line), 1) => line.clone(),
            _ => format!("Undecided: {}", parts.join(". ")),
        };
        let suggested = i.decisions.iter().find_map(|d| match d {
            Decision::Subtype { suggested } => *suggested,
            _ => None,
        });
        out.push(Problem {
            also,
            suggested,
            ..row(Level::Decide, At::Texture(i.key.clone()), text, None)
        });
    }
    for a in pbr_export::export_asks(m) {
        // A project-wide ask belongs to no set.
        let at = if a.set_id.is_empty() {
            At::Project
        } else {
            set_at(&a.set_key, &a.set_id)
        };
        let p = Problem {
            carried: true,
            ..row(Level::Warning, at, a.text, None)
        };
        out.push(kinded(p, kind::PGP_PITFALL));
    }
    for i in m
        .items
        .iter()
        .filter(|i| i.status != ItemStatus::NeedsDecision && m.item_in_run(i))
    {
        for w in i.warnings.iter().filter(|w| makes_a_row(w)) {
            let at = At::Texture(i.key.clone());
            if let Some(p) = note_row(&at, w) {
                out.push(p);
                continue;
            }
            // Note DP: a short normal's row rebuilds Z in place. Review E10: the table's other item fixes.
            let k = || i.key.clone();
            let fix = match w.code {
                Code::ChanNormalLength => Some(Fix::RebuildZ(k())),
                Code::AlphaParallaxNoAlpha => Some(Fix::ParallaxOff(k())),
                Code::SugContradiction => Some(Fix::SetType(k())),
                _ => None,
            };
            // A job-dependent sentence (A3) keeps the engine's message on the hover.
            let text = warning_text(i.job.as_ref(), w);
            let detail = (text != w.message).then(|| w.message.clone());
            out.push(Problem {
                fix,
                detail,
                ..row(Level::Warning, at, text.into_owned(), Some(w.code))
            });
        }
    }
    // Note JP3: SHADER-NORMAL-SPECULAR, an Info result, is a note row too: the user acts on it (Use PBR normal).
    let set_row =
        |w: &engine::codes::Warning| makes_a_row(w) || w.code == Code::ShaderNormalSpecular;
    for v in views.iter() {
        for w in v.set_warnings.iter().filter(|w| set_row(w)) {
            let at = set_at(&v.key, &v.id);
            let shader = || Some(Fix::ChangeShader(v.id.clone()));
            // Review E2: SHADER-EXTRA-MAP arrives here. Review E10: Remove from set for the member the warning
            // names, then Change shader…; with no such member, Change shader… alone.
            if let Some(p) = note_row(&at, w) {
                // Go to set outlines the member's row (review E10).
                let (fix, second_fix, target, text) = match w.code {
                    Code::ShaderExtraMap => match extra_member(m, v, w) {
                        Some(item) => (
                            Some(Fix::RemoveMember {
                                set_id: v.id.clone(),
                                item: item.clone(),
                            }),
                            shader(),
                            Some(SlotTarget::Item(item)),
                            None,
                        ),
                        None => (shader(), None, None, None),
                    },
                    // Note JP3 (B3-11): a kept DDS's type cannot drop its alpha, so it gets no button and its own
                    // sentence.
                    Code::ShaderNormalSpecular => match extra_member(m, v, w) {
                        Some(item)
                            if m.item(&item)
                                .is_some_and(|i| matches!(i.kind, ItemKind::Dds)) =>
                        {
                            (
                                None,
                                None,
                                Some(SlotTarget::Item(item)),
                                Some(code_text::kept_normal_sentence(w)),
                            )
                        }
                        Some(item) => (
                            Some(Fix::UsePbrNormal(item.clone())),
                            None,
                            Some(SlotTarget::Item(item)),
                            None,
                        ),
                        None => (None, None, None, None),
                    },
                    _ => (None, None, None, None),
                };
                out.push(Problem {
                    fix,
                    second_fix,
                    target,
                    text: text.unwrap_or(p.text),
                    ..p
                });
                continue;
            }
            let fix = match w.code {
                Code::ShaderUnavailable | Code::ShaderMissingMap => shader(),
                _ => None,
            };
            // Review E10: Go to set outlines the slot the missing map goes in.
            let target = match w.code {
                Code::ShaderMissingMap => w.data["map"]
                    .as_str()
                    .and_then(sets::slot_for_shader_map)
                    .map(SlotTarget::Slot),
                // No producer reaches this today: the engine raises the code only from the Also export planner, and set
                // rows come only from the main plan's set warnings. Kept so such a row lands on the Diffuse slot.
                Code::DeriveNoAlbedo => Some(SlotTarget::Slot(sets::SlotKind::Diffuse)),
                _ => None,
            };
            out.push(Problem {
                fix,
                target,
                ..row(Level::Warning, at, w.message.clone(), Some(w.code))
            });
        }
    }
    // Note DU3: a set entry that matches no file, with its removal as the fix.
    for v in views.iter() {
        for miss in &v.missing {
            let text = match miss {
                sets::Missing::NoFilesLeft => "No texture of this set is left in the folder".into(),
                _ => format!("No file matches {}", miss.text()),
            };
            let p = Problem {
                fix: Some(Fix::RemoveMissing(miss.clone())),
                ..row(Level::Warning, set_at(&v.key, &v.id), text, None)
            };
            out.push(kinded(p, kind::SET_MISSING_ENTRY));
        }
    }
    // Note HG: a mapped set whose record's winning copy now names other paths than the stored ones.
    for v in views.iter().filter(|v| !m.renames_outputs(v)) {
        let Some(map) = v.txst.as_ref().filter(|t| !t.is_base()) else {
            continue;
        };
        if m.record_of(map).is_some_and(|r| r.slots != map.slots) {
            let p = Problem {
                fix: Some(Fix::UseRecordPaths(v.id.clone())),
                ..row(
                    Level::Warning,
                    set_at(&v.key, &v.id),
                    format!("Record {} changed", map.edid),
                    None,
                )
            };
            out.push(kinded(p, kind::RECORD_CHANGED));
        }
    }
    // Note GI (X4): an XPMF pbr profile's exact EDID with no PBRMaterialObjects JSON from this project.
    for (key, id, edid) in super::xpmf_json::skipped_edids(m) {
        let p = Problem {
            detail: Some(super::xpmf_json::SKIP_HOVER.into()),
            ..row(
                Level::Warning,
                set_at(&key, &id),
                format!("XPMF skips {edid}: no PBRMaterialObjects JSON from this project"),
                None,
            )
        };
        out.push(kinded(p, kind::XPMF_SKIP));
    }
    // Advanced skin: a set that writes no `.csskin.json`, or leaves a texture out of it.
    for (id, key, why) in super::cs_skin_json::skin_json_plan(m).1 {
        use super::cs_skin_json::SkinSkip;
        let text = match why {
            SkinSkip::NoDiffuse => {
                "Advanced skin needs a diffuse in its set; no .csskin.json is written.".into()
            }
            SkinSkip::BorrowedDiffuse { host } => format!(
                "Uses {host}'s diffuse, so its material comes from {host}'s skin card; no .csskin.json is written for this set."
            ),
            SkinSkip::HostNotSkin { host } => format!(
                "{host} owns this diffuse but is not an Advanced skin set; no .csskin.json is written."
            ),
            SkinSkip::HostMissing { slot, host } => format!(
                "The {slot} texture borrowed from {host} is not exported by {host}; it is left out of the .csskin.json."
            ),
        };
        out.push(kinded(
            row(Level::Warning, set_at(&key, &id), text, None),
            kind::SKIN_SKIP,
        ));
    }
    // Note FC3 (Y4): an entry of the folder the plan could not read, with its folder to open.
    for w in &m.unreadable {
        out.push(Problem {
            fix: unreadable_folder(&w.message).map(Fix::OpenFolder),
            ..row(Level::Warning, At::Project, w.message.clone(), Some(w.code))
        });
    }
    // Note FC10 (item 27): a project file with the read-only flag, which a save cannot replace.
    for p in &m.read_only_flags {
        let name = p.file_name().unwrap_or_default().to_string_lossy();
        let flag = Problem {
            fix: Some(Fix::ClearReadOnly(p.clone())),
            ..row(
                Level::Warning,
                At::Project,
                format!("{name} is read-only: changes cannot be saved"),
                None,
            )
        };
        out.push(kinded(flag, kind::READ_ONLY_FLAG));
    }
    // Note FC10 (T8): a user preset skipped because another preset already declares its suffix aliases. Note FC14:
    // the skipped one is whichever loads second, the original or its copy, so the row names both. Ruling FR-b: any
    // other skipped file gets a row with its reason and Open folder.
    for (file, reason) in &m.skipped_user_presets {
        // Review E2 (planner's table): marked under the engine code these rows stand for.
        let skipped = kind::CFG_PRESET_SKIPPED;
        if let Some((id, other)) = alias_clash(reason) {
            let p = Problem {
                fix: Some(Fix::RemoveAliases(PathBuf::from(file), id.to_string())),
                detail: Some(format!("{file}: {reason}")),
                ..row(
                    Level::Warning,
                    At::Project,
                    format!("Preset {id} was skipped: it clashes with {other} on aliases"),
                    None,
                )
            };
            out.push(kinded(p, skipped));
            continue;
        }
        let path = Path::new(file);
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let why = skipped_reason(file, reason);
        let p = Problem {
            fix: path.parent().map(|p| Fix::OpenFolder(p.to_path_buf())),
            detail: Some(format!("{file}: {why}")),
            ..row(
                Level::Warning,
                At::Project,
                format!("Preset file {name} was skipped: {}", first_line(why)),
                None,
            )
        };
        out.push(kinded(p, skipped));
    }
    out.extend(nif_rows(NifLevel::Warning, Level::Warning));
    // Artist checks (DECISIONS Note CL): listed, never "ready"-changing. CHK-GREEN-GUESS, the one Info result, is a
    // row too, since it has an action (Note FC3, CL5).
    let check_row = |w: &engine::codes::Warning| makes_a_row(w) || w.code == Code::ChkGreenGuess;
    for i in m
        .items
        .iter()
        .filter(|i| i.status != ItemStatus::NeedsDecision && m.item_in_run(i))
    {
        for w in m.check_warnings(&i.key).iter().filter(|w| check_row(w)) {
            let at = At::Texture(i.key.clone());
            // Review E10: CHK-Z-SIGN's sentence names Rebuild Z.
            let fix = match w.code {
                Code::ChkGreenGuess => Some(Fix::TreatAsOpenGl(i.key.clone())),
                Code::ChkZSign => Some(Fix::RebuildZ(i.key.clone())),
                _ => None,
            };
            out.push(Problem {
                fix,
                ..row(Level::Check, at, w.message.clone(), Some(w.code))
            });
        }
    }
    for v in views.iter() {
        for w in m
            .set_check_warnings(&v.key)
            .iter()
            .filter(|w| makes_a_row(w))
        {
            let at = set_at(&v.key, &v.id);
            out.push(row(Level::Check, at, w.message.clone(), Some(w.code)));
        }
    }
    // Rows only count once the export has ended: `revision` bumps on every engine event (R9).
    if m.export.running.is_none() {
        // K2: an engine CHK-* row is a check row and follows its switch. The switches are not in `ProblemsKey`:
        // a switch change reaches this build through `refresh_checks`' `invalidate_views()`.
        let on = &m.settings.checks;
        // Note FC3 (CN67): the DERIVE result the user acts on is a Warning row that opens the report; the other
        // Info results stay on the row's hover (P11).
        let file_row =
            |w: &engine::codes::Warning| makes_a_row(w) || w.code == Code::DeriveNifSuggest;
        // Note DP4: the live item row is the current truth and carries the fix, so a same-code copy of it from
        // the last export is dropped. Only when the job maps to exactly one item (None = several share a job_key).
        // Built once, and only when some export row has a listed warning (no per-row scan of all items).
        let mut owners: Option<HashMap<&str, Option<&super::items::TextureItem>>> = None;
        for r in &m.export.rows {
            if !r.warnings.iter().any(file_row) {
                continue;
            }
            let owners = owners.get_or_insert_with(|| {
                let mut map = HashMap::new();
                for i in &m.items {
                    map.entry(i.job_key.as_str())
                        .and_modify(|e| *e = None)
                        .or_insert(Some(i));
                }
                map
            });
            let live = owners.get(r.job_key.as_str()).copied().flatten();
            for w in r.warnings.iter().filter(|w| file_row(w)) {
                if live.is_some_and(|i| {
                    i.status != ItemStatus::NeedsDecision
                        && i.warnings.iter().any(|x| x.code == w.code)
                }) {
                    continue;
                }
                let level = match checks::check_of(w.code) {
                    Some(c) if !checks::switch_on(on, c) => continue,
                    Some(_) => Level::Check,
                    None => Level::Warning,
                };
                let at = At::File(r.job_key.clone());
                // Review E25: with Rebuild Z now on, the file on disk is still short until the next export rewrites
                // it, so the row is a queued note with no button (the engine never raises it with Rebuild Z on).
                let rebuilt = live
                    .and_then(|i| i.job.as_ref())
                    .is_some_and(|j| j.profile.channels.reconstruct_z);
                if w.code == Code::ChanNormalLength && rebuilt {
                    let text = QUEUED_REBUILD_Z.to_string();
                    out.push(row(Level::Queued, at, text, Some(w.code)));
                    continue;
                }
                let fix = match w.code {
                    Code::ChanNormalLength => live.map(|i| Fix::RebuildZ(i.key.clone())),
                    _ => None,
                };
                // Review E2: a note from the last export gets the same row as a live one.
                if let Some(p) = note_row(&at, w) {
                    out.push(p);
                    continue;
                }
                // The suggested NIF values are the engine message; the row shows the sentence, the hover the values.
                // A job-dependent sentence (A3) keeps the engine's message on the hover too.
                let text = warning_text(live.and_then(|i| i.job.as_ref()), w);
                let detail = (w.code == Code::DeriveNifSuggest || text != w.message)
                    .then(|| w.message.clone());
                out.push(Problem {
                    fix,
                    detail,
                    ..row(level, at, text.into_owned(), Some(w.code))
                });
            }
        }
    }
    // Note FC3 (R-fills-2, Z11): what an edit removed is an Info row, with Undo while that edit is the last step.
    let undo = |step: u64| (step == m.history.pushes && m.can_undo()).then_some(Fix::UndoLast);
    for (k, note) in &m.dropped_fills {
        out.push(Problem {
            fix: undo(m.dropped_fills_step),
            ..row(Level::Info, At::Texture(k.clone()), note.clone(), None)
        });
    }
    // One row per texture the cleanup touched (the note's hover list before FC3).
    for line in m.encoding_note.iter().flat_map(|(_, all)| all.lines()) {
        out.push(Problem {
            fix: undo(m.encoding_note_step),
            ..row(Level::Info, At::Project, line.to_string(), None)
        });
    }
    // Note FC1: what the plan decided on its own and the project now records, one row with every texture on hover.
    if let Some(a) = &m.auto_note {
        out.push(Problem {
            fix: undo(a.step),
            detail: Some(a.lines.join("\n")),
            ..row(Level::Info, At::Project, a.text.clone(), None)
        });
    }
    // Note JI6: the `[files]` entries that name no file, checked by the last full plan in its mode (`m.target`); one
    // row with every entry on hover.
    if !m.unmatched_keys.is_empty() {
        let keys = m.unmatched_keys.clone();
        let mode = code_text::target_name(m.target);
        let text = match keys.len() {
            1 => format!("1 saved setting names no file in this mode ({mode})"),
            n => format!("{n} saved settings name no file in this mode ({mode})"),
        };
        out.push(Problem {
            detail: Some(keys.join("\n")),
            fix: Some(Fix::RemoveUnmatched(keys)),
            ..row(Level::Info, At::Project, text, Some(Code::CfgFileUnmatched))
        });
    }
    // Note FC10: hand remedies with a button. X10/X3: a diffuse shaped like complex grass that nothing tags as grass
    // (no GRAS record names it). BQ15: an OpenEXR whose varying alpha divided its colour on load.
    for i in &m.items {
        let (Some(j), Some(a)) = (&i.job, &i.analysis) else {
            continue;
        };
        let at = || At::Texture(i.key.clone());
        if j.profile.subtype == engine::config::Subtype::Diffuse
            && engine::grass::looks_like_grass(a)
        {
            out.push(Problem {
                fix: Some(Fix::TagAsGrass(i.key.clone())),
                detail: Some(TAG_AS_GRASS_HOVER.into()),
                ..row(
                    Level::Info,
                    at(),
                    "Looks like complex grass, but is not tagged as grass".into(),
                    None,
                )
            });
        }
        let exr = i
            .input
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exr"));
        if exr && a.alpha == engine::presets::AlphaContent::Varying && !j.profile.alpha.exr_straight
        {
            out.push(Problem {
                fix: Some(Fix::StraightAlpha(i.key.clone())),
                detail: Some(STRAIGHT_ALPHA_HOVER.into()),
                ..row(
                    Level::Info,
                    at(),
                    "Colour divided by alpha on load (premultiplied EXR)".into(),
                    None,
                )
            });
        }
    }
    // Note FC10 (CV8): a stored empty `match{n}` (n >= 2; `match1 = ""` turns the diffuse match off) matches nothing.
    for v in views.iter() {
        for n in 2..=8u8 {
            if v.settings.get(&format!("match{n}"))
                == Some(&serde_json::Value::String(String::new()))
            {
                out.push(Problem {
                    fix: Some(Fix::RemoveMatch(v.id.clone(), n)),
                    ..row(
                        Level::Info,
                        set_at(&v.key, &v.id),
                        format!("match{n} is empty and matches nothing"),
                        None,
                    )
                });
            }
        }
    }
    // Note HG: a mapped set whose record is in no winning copy of a non-empty scan; the mapping and paths stay. An
    // empty scan says nothing (a scan that has not run is not a record that went away).
    if !m.known.records.is_empty() {
        for v in views.iter().filter(|v| !m.renames_outputs(v)) {
            let Some(map) = v
                .txst
                .as_ref()
                .filter(|t| !t.is_base() && m.record_of(t).is_none())
            else {
                continue;
            };
            out.push(row(
                Level::Info,
                set_at(&v.key, &v.id),
                format!(
                    "Record {} isn't in the latest scan; its stored paths stay",
                    map.edid
                ),
                None,
            ));
        }
    }
    // Note HL6: missing links are Info rows and still export; conflicts stay Warning.
    for v in views.iter() {
        let at = || set_at(&v.key, &v.id);
        // `v.json` ignores the target (a PBR pick with a filled slot); `pgp_written` is the export's own test.
        if m.pgp_written(v) {
            // A typed match that matches nothing drops the set's entry (`json_entries`), so the mesh keeps nothing.
            let has_entry = !crate::pgp::prefix::matches_nothing(
                &v.settings,
                v.derived_match.as_deref().unwrap_or(""),
            );
            if let Some(path) = pbr_export::no_albedo_slot1(v).filter(|_| has_entry) {
                out.push(row(
                    Level::Info,
                    at(),
                    NO_ALBEDO_KEEPS.replace("{path}", &path),
                    None,
                ));
            }
            if let Some(s @ StaticsStatus::Unused) = m.statics_status(v) {
                out.push(row(Level::Info, at(), statics_text(&s), None));
            }
        }
        if m.cs_block(&v.id, Kind::Txst).is_some()
            && set_key(v)
                .and_then(|k| m.scan_facts(&k).ok())
                .is_some_and(|f| f.txst_edids.is_empty())
        {
            out.push(row(Level::Info, at(), NO_TXST_USES.into(), None));
        }
        for f in m.target_replaces(v) {
            out.push(row(
                Level::Info,
                at(),
                REPLACES_TARGET.replace("{file}", &f),
                None,
            ));
        }
    }
    out.extend(nif_rows(NifLevel::Info, Level::Info));
    // Review E2 (controller ruling 7): each markable row's mark, by code or by the kind named above, and whether the
    // project holds it. A row that is not markable keeps none, so a stale mark never takes it out of Ready.
    let doc = m.sets_doc.as_ref();
    for p in &mut out {
        if !markable(p.level, p.code) {
            p.mark = None;
            continue;
        }
        if let (None, Some(c)) = (&p.mark, p.code) {
            p.mark = Some(intended_key(&p.at, c.as_str(), &p.shown()));
        }
        // A codeless row gets its kind where it is built (`kinded`); one without is a row with no Mark button.
        debug_assert!(
            p.mark.is_some(),
            "a markable row with no kind: {:?}",
            p.text
        );
        p.intended = doc
            .zip(p.mark.as_ref())
            .is_some_and(|(d, k)| d.is_intended(k));
    }
    out
}

/// Review E10: the key of `v`'s member that a SHADER-EXTRA-MAP's or SHADER-NORMAL-SPECULAR's `data.member` names.
/// The engine writes the member's output file name (`engine::api` `shader_members`); a source file name is matched too.
fn extra_member(m: &AppModel, v: &sets::SetView, w: &engine::codes::Warning) -> Option<String> {
    let name = w.data.get("member")?.as_str()?;
    v.members
        .iter()
        .find(|k| {
            m.item(k).is_some_and(|i| {
                i.name == name
                    || i.job
                        .as_ref()
                        .is_some_and(|j| j.output.file_name().is_some_and(|f| f == name))
            })
        })
        .cloned()
}

/// Note HL6: a PBR set with no albedo keeps the mesh's own diffuse at `{path}`.
pub const NO_ALBEDO_KEEPS: &str = "No albedo in this set; the mesh keeps {path}. A vanilla diffuse isn't sRGB, so it can look washed out";
/// Note JP3: the button of a SHADER-NORMAL-SPECULAR row, and its undo step's label.
pub const USE_PBR_NORMAL: &str = "Use PBR normal";
/// Note HL6: an output the target mod already ships (it wins or loses by load order).
pub const REPLACES_TARGET: &str = "Replaces {file} of the target mod in the load order";

/// The hover of a `Tag as grass` row (Note FC10).
pub const TAG_AS_GRASS_HOVER: &str = "A 1:2 image whose bottom half is a normal map, as complex grass is. No grass record \
     (GRAS) of a scan names it, so it is converted as a diffuse until it is tagged. Tag as grass writes subtype = \
     \"complex_grass\" for it in sdc.toml.";
/// The hover of a `Straight alpha` row (Note FC10, BQ15).
pub const STRAIGHT_ALPHA_HOVER: &str = "OpenEXR stores colour multiplied by alpha, so Tessalith divides it back on \
     load. If this file's colour is not premultiplied (straight alpha), Straight alpha reads it as stored \
     (alpha.exr_straight = true).";

/// Note FC10 (T8): `(id, other)` of a CFG-PRESET-SKIPPED reason that is an alias clash.
/// shortcut: parses the engine's message (`presets::check_aliases`); read a `data` field once the engine adds one.
fn alias_clash(reason: &str) -> Option<(&str, &str)> {
    let (id, rest) = reason
        .split_once("preset '")?
        .1
        .split_once("': suffix alias '")?;
    let other = rest
        .split_once("is already declared by preset '")?
        .1
        .strip_suffix('\'')?;
    Some((id, other))
}

/// Ruling FR-b: a CFG-PRESET-SKIPPED reason without the engine's `preset parse error: <file>: ` lead-in (the file is
/// named once by `add_file` and again by `insert_toml`).
/// shortcut: parses the engine's message, as `alias_clash` does; read a `data` field once the engine adds one.
pub(crate) fn skipped_reason<'a>(file: &str, reason: &'a str) -> &'a str {
    let mut r = reason
        .strip_prefix("preset parse error: ")
        .unwrap_or(reason);
    let lead = format!("{file}: ");
    while let Some(rest) = r.strip_prefix(lead.as_str()) {
        r = rest;
    }
    r
}

/// The first line of a message; a removed-key list has one key per line (ruling FR-a).
pub(crate) fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s)
}

/// The folder that holds an IN-UNREADABLE entry: the engine writes `skipped: <path>: <error>` (engine `api.rs`,
/// `discover.rs`), with no `data` field.
/// shortcut: parses the engine's human message; switch to `w.data` once the engine puts the path there.
fn unreadable_folder(message: &str) -> Option<PathBuf> {
    let rest = message.strip_prefix("skipped: ")?;
    // A drive's `C:\` has no space after its colon; the error's separator does.
    let path = rest.split_once(": ").map_or(rest, |(p, _)| p);
    PathBuf::from(path).parent().map(PathBuf::from)
}

/// The list narrowed by `f`: a set keeps its own rows and its members' rows; `Undecided` keeps the Decide rows.
/// `None` and `Set` list the worklist (`Problem::in_worklist`); `Notes` and `SetNotes` list every note.
pub fn filtered(m: &AppModel, f: Option<&ProblemsFilter>) -> Arc<Vec<Problem>> {
    #[cfg(feature = "test-hooks")]
    m.filter_builds.set(m.filter_builds.get() + 1);
    let all = problems(m);
    Arc::new(match f {
        None => worklist(&all, &|_| true),
        Some(ProblemsFilter::Notes) => all.iter().filter(|p| p.is_note()).cloned().collect(),
        Some(ProblemsFilter::SetNotes(k)) => {
            let s = scope(m, k);
            all.iter()
                .filter(|p| p.is_note() && in_set(p, &s))
                .cloned()
                .collect()
        }
        Some(ProblemsFilter::Undecided) => all
            .iter()
            .filter(|p| p.level == Level::Decide)
            .cloned()
            .collect(),
        Some(ProblemsFilter::Checks) => {
            all.iter().filter(|p| counts_as_check(p)).cloned().collect()
        }
        Some(ProblemsFilter::Set(k)) => {
            let s = scope(m, k);
            worklist(&all, &|p| in_set(p, &s))
        }
        Some(ProblemsFilter::SetChecks(k)) => {
            let s = scope(m, k);
            all.iter()
                .filter(|p| counts_as_check(p) && in_set(p, &s))
                .cloned()
                .collect()
        }
    })
}

/// Review E2 (rule 1.2, controller ruling 5): one worklist of the `keep` rows, the problem rows, then the notes with a
/// button, each group in build order.
fn worklist(all: &[Problem], keep: &dyn Fn(&Problem) -> bool) -> Vec<Problem> {
    let (notes, rows): (Vec<&Problem>, Vec<&Problem>) = all
        .iter()
        .filter(|p| p.in_worklist() && keep(p))
        .partition(|p| p.is_note());
    rows.into_iter().chain(notes).cloned().collect()
}

/// How many of the set's own rows and members' rows pass `keep`, without cloning them. `key` is a set's scope key
/// (`scope_key`: its key, or a key-less set's id).
pub fn count_in_set(m: &AppModel, key: &str, keep: impl Fn(&Problem) -> bool) -> usize {
    let s = scope(m, key);
    problems(m)
        .iter()
        .filter(|p| in_set(p, &s) && keep(p))
        .count()
}

/// Note JP5 (B3-16): the rows a set filter or reason list names: by key, or for a key-less set by its id; and the
/// set's member items.
struct Scope {
    key: String,
    id: String,
    members: HashSet<String>,
}

impl Scope {
    fn of(v: &sets::SetView) -> Self {
        Scope {
            key: v.key.clone(),
            id: v.id.clone(),
            members: v.members.iter().cloned().collect(),
        }
    }
}

/// The view `s` names: the set with that key, else the key-less set with that id. `None` for the empty string.
fn view_of<'a>(views: &'a [sets::SetView], s: &str) -> Option<&'a sets::SetView> {
    if s.is_empty() {
        return None;
    }
    views
        .iter()
        .find(|v| v.key == s)
        .or_else(|| views.iter().find(|v| v.key.is_empty() && v.id == s))
}

/// `s`'s scope. A key no view holds still names its key's rows, as before; the empty string names no row.
fn scope(m: &AppModel, s: &str) -> Scope {
    let views = sets::views(m);
    match view_of(&views, s) {
        Some(v) => Scope::of(v),
        None => Scope {
            key: s.to_string(),
            id: String::new(),
            members: HashSet::new(),
        },
    }
}

/// A row of the scope: a set row by key, or for a key-less set by id (another key-less set's rows never match); a
/// texture row of a member.
fn in_set(p: &Problem, s: &Scope) -> bool {
    match &p.at {
        At::Set { key, id } if s.key.is_empty() => {
            !s.id.is_empty() && key.is_empty() && *id == s.id
        }
        At::Set { key, .. } => *key == s.key,
        At::Texture(t) => s.members.contains(t),
        _ => false,
    }
}

/// Review E4: one reason a set is not ready, as the set editor's first line names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    pub text: String,
    /// The whole sentence when `text` is cut (rule 3.4: the button's name and hover hold it).
    pub full: Option<String>,
    pub go: ReasonGo,
}

/// Where a reason's button goes: the empty slot's **Assign…**, or the row in Problems (boxed: a `Problem` is large).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReasonGo {
    Slot(sets::SlotKind),
    Row(Box<Problem>),
}

/// Review E4: the rows of the set (a set's scope key (`scope_key`: its key, or a key-less set's id)) and of its
/// members that take "ready" away (the rows `set_ready` reads), hidden kinds included, in list order. A missing map
/// whose slot the set editor draws a row for goes to that slot's **Assign…**; every other row goes to itself in
/// Problems.
pub fn set_reasons(m: &AppModel, key: &str) -> Vec<Reason> {
    with_counted_rows(m, key, |v, rows| {
        let drawn: Vec<sets::SlotKind> = sets::editor_cells(v).iter().map(|c| c.kind).collect();
        rows.into_iter().map(|p| reason(v, p, &drawn)).collect()
    })
    .unwrap_or_default()
}

/// `set_reasons` for the set editor's status line, which reads it every frame (final review Minor 1): scanned once
/// per `problems()` build, a set's scope key (`scope_key`: its key, or a key-less set's id) and hidden kinds, as
/// `cached_tab_counts`. The set views it reads live in the same cache cell, which is emptied when they change.
pub fn cached_set_reasons(m: &AppModel, key: &str) -> Arc<Vec<Reason>> {
    // Both before the borrow: `problems` writes the same cell.
    let all = problems(m);
    let hidden = m.hidden_kinds();
    if let Some((.., r)) = m
        .views
        .borrow()
        .set_reasons
        .as_ref()
        .filter(|(p, k, h, _)| Arc::ptr_eq(p, &all) && k == key && *h == hidden)
    {
        return r.clone();
    }
    #[cfg(feature = "test-hooks")]
    m.reason_scans.set(m.reason_scans.get() + 1);
    let r = Arc::new(set_reasons(m, key));
    m.views.borrow_mut().set_reasons = Some((all, key.to_string(), hidden, r.clone()));
    r
}

/// Review E4: the hidden kinds among the rows `set_reasons` counts for the set `key` (a set's scope key
/// (`scope_key`: its key, or a key-less set's id)), slot reasons' rows included (what "+k more in Problems" shows
/// again).
pub fn hidden_reason_kinds(m: &AppModel, key: &str) -> Vec<Code> {
    let hidden = m.hidden_kinds();
    with_counted_rows(m, key, |_, rows| {
        rows.into_iter()
            .filter(|p| is_hidden(p, &hidden))
            .filter_map(|p| p.code)
            .collect()
    })
    .unwrap_or_default()
}

/// `f` on the set `key`'s view and the rows that take its "ready" away, in list order; `None` for a set gone from
/// the plan or ready (the cached answer the cards read, so a ready set never scans the rows). `key` is a set's scope
/// key (`scope_key`). `None` for a name no set has (the empty string among them), or a ready set.
fn with_counted_rows<T>(
    m: &AppModel,
    key: &str,
    f: impl FnOnce(&sets::SetView, Vec<&Problem>) -> T,
) -> Option<T> {
    let views = sets::views(m);
    let v = view_of(&views, key)?;
    if set_ready_in(&flagged(m), v) {
        return None;
    }
    let s = Scope::of(v);
    let all = problems(m);
    let rows = all
        .iter()
        .filter(|p| counts_as_ready_problem(p) && in_set(p, &s))
        .collect();
    Some(f(v, rows))
}

/// One counted row as a reason: a missing map whose slot the editor draws goes to the slot.
fn reason(v: &sets::SetView, p: &Problem, drawn: &[sets::SlotKind]) -> Reason {
    match (p.code, &p.target) {
        // The DERIVE-NO-ALBEDO arm has no producer today: the engine raises it only from the Also export planner,
        // whose rows are not set rows.
        (Some(Code::ShaderMissingMap | Code::DeriveNoAlbedo), Some(SlotTarget::Slot(kind)))
            if drawn.contains(kind) =>
        {
            Reason {
                text: format!("no {} (Assign…)", unnested(v.slot_label(*kind, ""))),
                full: None,
                go: ReasonGo::Slot(*kind),
            }
        }
        _ => {
            let (text, full) = reason_text(p);
            Reason {
                text,
                full,
                go: ReasonGo::Row(Box::new(p.clone())),
            }
        }
    }
}

/// A slot label without its own closing parentheses, so the reason's "(Assign…)" is not nested: "Emissive / Skin
/// (slot 3)" reads "Emissive / Skin, slot 3" (final fix wave A11).
fn unnested(label: &str) -> std::borrow::Cow<'_, str> {
    match label.strip_suffix(')').and_then(|l| l.rsplit_once(" (")) {
        Some((name, inner)) => format!("{name}, {inner}").into(),
        None => label.into(),
    }
}

/// A row reason's words: its code's sentence up to " — ", or a code-less row's text cut at 60 characters with "…";
/// then the whole text when it was cut.
fn reason_text(p: &Problem) -> (String, Option<String>) {
    const MAX: usize = 60;
    match p.code {
        Some(c) => (
            code_text::code_line(c)
                .split(" — ")
                .next()
                .unwrap_or_default()
                .to_string(),
            None,
        ),
        None if p.text.chars().count() > MAX => {
            let cut: String = p.text.chars().take(MAX).collect();
            (format!("{cut}…"), Some(p.text.clone()))
        }
        None => (p.text.clone(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review E2 (controller ruling 5): a note with a button built before a problem row is listed after it. No row
    /// `build_problems` makes today comes in that order, so the rows are written out here.
    #[test]
    fn the_worklist_lists_the_problems_before_the_notes_with_a_button() {
        let row = |level, text: &str, fix| Problem {
            level,
            at: At::Project,
            text: text.into(),
            code: None,
            also: vec![],
            fix,
            second_fix: None,
            suggested: None,
            detail: None,
            carried: false,
            intended: false,
            mark: None,
            target: None,
        };
        let all = [
            row(Level::Info, "note", Some(Fix::UndoLast)),
            row(Level::Info, "bare note", None),
            row(Level::Warning, "problem", None),
        ];
        let texts: Vec<_> = worklist(&all, &|_| true)
            .into_iter()
            .map(|p| p.text)
            .collect();
        assert_eq!(texts, ["problem", "note"]);
    }

    /// Review E4: a reason names a code's row by its sentence up to " — ", and a code-less row by its text, cut at 60
    /// characters with "…"; a cut text keeps the whole one beside it (final review Important 1).
    #[test]
    fn a_reason_is_the_sentence_before_its_dash_or_the_text_cut_at_60() {
        let row = |text: &str, code| Problem {
            level: Level::Warning,
            at: At::Project,
            text: text.into(),
            code,
            also: vec![],
            fix: None,
            second_fix: None,
            suggested: None,
            detail: None,
            carried: false,
            intended: false,
            mark: None,
            target: None,
        };
        assert_eq!(
            reason_text(&row("mip 3", Some(Code::ValMipError))),
            ("A mip level strays from its source".to_string(), None)
        );
        let long = "é".repeat(61);
        assert_eq!(
            reason_text(&row(&long, None)),
            (format!("{}…", "é".repeat(60)), Some(long.clone()))
        );
        let sixty = "a".repeat(60);
        assert_eq!(reason_text(&row(&sixty, None)), (sixty, None));
    }

    /// Pins the engine's IN-UNREADABLE message shape (`skipped: <path>: <error>`) that `unreadable_folder` parses.
    #[test]
    fn unreadable_folder_reads_the_engine_message() {
        let m = r"skipped: C:\mods\a\locked: Access is denied. (os error 5)";
        assert_eq!(unreadable_folder(m), Some(PathBuf::from(r"C:\mods\a")));
        assert_eq!(unreadable_folder("not the shape"), None);
        // The shapes the engine writes for a dangling link and for a link loop (Note FM5).
        let dead =
            r"skipped: C:\mods\a\link: The system cannot find the file specified. (os error 2)";
        assert_eq!(unreadable_folder(dead), Some(PathBuf::from(r"C:\mods\a")));
        let looped = r"skipped: C:\mods\a\loop: file system loop to C:\mods\a";
        assert_eq!(unreadable_folder(looped), Some(PathBuf::from(r"C:\mods\a")));
    }

    /// Pins the engine's alias-clash refusal (`presets::check_aliases`) that `alias_clash` parses.
    #[test]
    fn alias_clash_reads_the_engine_reason() {
        let mut r = engine::presets::PresetRegistry::builtin().unwrap();
        let t = |id: &str| {
            format!("schema_version = 1\nid = \"{id}\"\nversion = \"1.0.0\"\nbase = \"cs-quality\"\n[suffix_aliases]\nzz9 = \"n\"\n")
        };
        r.add_toml(&t("a")).unwrap();
        let e = r.add_toml(&t("b-export")).unwrap_err().to_string();
        assert_eq!(alias_clash(&e), Some(("b-export", "a")), "{e}");
        assert_eq!(
            alias_clash("CFG-INVALID: preset 'x': unknown preset 'nope'"),
            None
        );
    }
}
