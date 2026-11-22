//! Engine API (spec §2, §5.9, §8): `Engine::plan(BatchRequest) -> Plan`, then `Engine::run(Plan, events, cancel)`.
//!
//! `plan` runs Discover → Group → Load → Analyse → Resolve and reports every decision; nothing is written. Its
//! Load/Analyse work is admitted against `--mem-budget` just like conversion.
//! `run` applies `--on-existing`, then converts the ready files on a rayon pool of `jobs` threads, admitting them
//! by estimated memory against `--mem-budget`. Files that are unresolved, blocked or in error are reported and
//! never converted: the engine does not guess (spec §8, DECISIONS "never auto-decide").
use crate::analyse::analyse_with;
use crate::codes::{Code, Warning};
use crate::config::*;
use crate::discover::{canonicalise_outputs, discover_reporting, InputFile, InputKind};
use crate::events::{EventKind, Reporter, SkipKind, Stage};
use crate::group::{
    group_sets_with, join_detail_n, join_skin_wet, member_split, SuffixSplit, TextureSet,
};
use crate::image::SourceInfo;
use crate::load::{load_image_meta, LoadError};
use crate::manifest::{
    encode_version, output_key, profile_sha256, recorded_sources, sha256_file, EncoderVersions,
    Manifest, ManifestEntry, PresetRef, BC1_ENCODE_ID,
};
use crate::overrides::{project_rel, to_patch, FileScopes, KeyForms, Scope, SetOverride};
use crate::pack;
use crate::pipeline::{check_in_place, process_file, FileError, FileTracker};
use crate::policy::*;
use crate::presets::*;
use crate::session::{fill_cache_key, pack_cache_key, AnalysisCache, PackEntry};
use crate::shader::{MeshAlpha, Shader, ShaderPick};
use crate::suggest::{is_pbr_set, SuggestContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

pub use crate::ev_scope::{EvScope, Property, PropertyGroup, PropertyGroups};

/// What `run` does when a ready file's output already exists (spec §3.2 `output.on_existing`, §8). It is a batch
/// setting, not a `Profile` field.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum OnExisting {
    /// Re-encode when the source, a settings field other than a quality target, or the validation verdict changed, or
    /// a quality target rose; keep an output only its encode version or a lower target separates (`--ev` re-encodes
    /// the first, Note JA) and an output that has no manifest record (`OUT-UNKNOWN-EXISTING`).
    #[default]
    IfChanged,
    Skip,
    Overwrite,
    // Note JA: encode every output that has a manifest record again; keep one without (`OUT-UNKNOWN-EXISTING`), as
    // `if-changed` does. The GUI's re-encode box; the CLI has no value for it.
    Reencode,
}

/// The disposition of one scoped, ready primary output under the request's existing-output policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewVerdict {
    WillWrite,
    Current,
    Kept {
        older_encode: bool,
        lower_target: bool,
    },
    UnknownExisting,
    OnExistingSkip,
}

/// A shared roughness job and the normal input in each user's resolved slot (Note LE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedRoughness {
    pub producer: String,
    /// The pre-packed input, or None for the producer's pack of this subtype.
    pub source: Option<PathBuf>,
    pub subtype: Subtype,
    /// Set id to (normal owner id, input path). An empty or external slot has no local input.
    pub users: BTreeMap<String, Option<(String, PathBuf)>>,
}

/// One batch, as the CLI or GUI hands it to the engine. `output_root` and `on_existing` are the spec §3.2
/// `output {path, on_existing}` batch settings.
#[derive(Debug, Clone)]
pub struct BatchRequest {
    pub inputs: Vec<PathBuf>,
    pub output_root: PathBuf,
    /// `--target`. `None` lets the project config, then the user config, then the selected preset decide, and
    /// finally `cs`; the origin is recorded as `provenance["target"]` of every job.
    pub target: Option<Target>,
    /// Preset id; `None` takes the configs' `preset`, then `default_preset_for(target)`.
    pub preset: Option<String>,
    /// Preset files to load (not select), e.g. ones declaring suffix aliases.
    pub preset_files: Vec<PathBuf>,
    /// User config; skipped when the file does not exist.
    pub user_config: Option<PathBuf>,
    /// The user presets folder (`%APPDATA%\sdc\presets`, M6 Track D spec §3): every `*.toml` in it is loaded before
    /// `preset_files`; a bad one is skipped with CFG-PRESET-SKIPPED. `None` (tests) loads nothing.
    pub user_presets: Option<PathBuf>,
    /// Project config; defaults to `sdc.toml` in the first input folder when that exists.
    pub project_config: Option<PathBuf>,
    /// The project config as edited in memory (DECISIONS N4). When `Some`, it is used instead of
    /// `project_config` or the first input folder's `sdc.toml`, and `[files]` keys are relative to the first input.
    pub project_config_inline: Option<ConfigFile>,
    pub overrides: Vec<SetOverride>,
    pub accept: AcceptSuggestions,
    pub allow: BTreeSet<Code>,
    /// Per-file acknowledgements (DECISIONS N2): `[files]` key → codes. Merged with the project config's
    /// `[files.*].allow` (Task 3).
    pub file_allow: BTreeMap<String, BTreeSet<Code>>,
    /// GUI set edits (M3b spec §2.1): applied in order after the automatic grouping. Empty for the CLI.
    pub set_edits: Vec<crate::sets::SetEdit>,
    /// GUI output paths (M3b, DECISIONS P1), relative to `output_root`. Empty for the CLI.
    pub output_paths: BTreeMap<crate::sets::OutputKey, PathBuf>,
    /// Explicit R7 exclusions for callers that flag shared sets off without resolved slots.
    pub shared_roughness: BTreeSet<String>,
    /// GUI slot-6 outputs with multiple users, including their effective Normal slots.
    pub shared_roughness_groups: Vec<SharedRoughness>,
    /// GUI (DECISIONS Note R M4-final-1): borrower set id → the id of the set whose `_n` its Normal slot borrows. The
    /// engine bakes the borrower's own roughness member from that `_n`, under every usual bake rule. Empty for the CLI.
    pub borrowed_normal: BTreeMap<String, String>,
    /// GUI (M6, DECISIONS Note S-shader-2): set id → explicit shader. Empty for the CLI; `--set set:<id>.shader`
    /// beats it.
    pub set_shaders: BTreeMap<String, Shader>,
    /// GUI (Note DW): `[files]` keys of the files in a pinned slot of their own set. A set whose diffuse member is
    /// listed gets no Advanced Skin reading from its chosen shader (Note DV1). Empty for the CLI.
    pub pinned_files: BTreeSet<String>,
    /// GUI (M6, DECISIONS Note S-shader-10): `shader::mesh_key` → the shader each scanned lighting shape that uses
    /// `<key>.dds` as its diffuse reports. Empty for the CLI and when no scan exists: the suffix rules decide.
    pub mesh_shaders: BTreeMap<String, Vec<Shader>>,
    /// GUI (M6 Track E, DECISIONS Note U5): `shader::colour_mesh_key` → the NiAlphaProperty of each scanned lighting
    /// shape that uses `<key>.dds` in slot 0. The GUI fills it from the scan database; the CLI sends none (empty: no
    /// evidence, and the alpha question is asked as before).
    pub mesh_alpha: BTreeMap<String, Vec<MeshAlpha>>,
    /// GUI (complex grass design §5): the slot-0 textures of the NIFs that GRAS records of the active load order use,
    /// as `grass::path_key` keys. A listed 1:2 file that passes the detection tests gets a high-confidence
    /// `complex_grass` suggestion. Empty for the CLI, which therefore never auto-tags.
    pub gras_diffuses: BTreeSet<String>,
    /// GUI (PBR grass design §4.2): as `gras_diffuses` with TruePBR shapes kept (`Store::gras_all_diffuse_textures`).
    /// A PBR set with a member listed here auto-picks `pbr_grass`. Empty for the CLI, which never auto-picks it.
    pub gras_all_diffuses: BTreeSet<String>,
    /// GUI (Note BM): ids of the sets with a `[sets.cs_txst]` block. A PBR set listed here auto-picks `pbr_landscape`
    /// under CS. Empty for the CLI.
    pub cs_txst_sets: BTreeSet<String>,
    pub recursive: bool,
    pub on_existing: OnExisting,
    /// Which outputs written by an older encode version are re-encoded (`--ev`, `--property`). Read only under
    /// `OnExisting::IfChanged` (Note JA).
    pub ev: EvScope,
    /// Worker threads; 0 = one per logical core.
    pub jobs: usize,
    /// Memory budget in bytes for admitting files; 0 = half of physical RAM.
    pub mem_budget: u64,
    pub dry_run: bool,
    /// Decode and check every written output before it is written (spec §5.8). Off by default (Note LG1); with
    /// `if-changed` it also checks skipped current outputs in place (LG10). `sdc convert --validate` and the GUI's
    /// Validate exported DDS turn it on. Not a planning input: left out of `sig_without_files`.
    pub validate: bool,
    /// GUI (DECISIONS AG91): counters the planner updates while it loads and resolves; nothing is emitted or written.
    pub plan_progress: Option<PlanProgress>,
    /// GUI (v2 artist checks, check 3, DECISIONS Note CL20): measure each eligible texture's detail ratio during the
    /// run. `Off` for the CLI.
    pub low_detail: LowDetailMode,
    /// Track A (DECISIONS Note CN): replace each PBR set's jobs with derived vanilla or complex-material jobs. `None`
    /// (the default) plans as before (C6).
    pub derive: Option<crate::derive::DeriveRequest>,
    /// Track A (Note CN): the folder discovery skips in place of `output_root` (the Also export location, which
    /// holds the PBR, Vanilla and CM folders). `None` skips `output_root` as before.
    pub skip_root: Option<PathBuf>,
    /// Note HQ1: the export folder when `output_root` lies below it (`set_export`); `output_paths` are relative to it,
    /// so an explicit path such as a PGPatcher `textures\pbr\...` prefix can leave the output root. `None`: relative
    /// to `output_root`.
    pub export_root: Option<PathBuf>,
    /// GUI (Note HF9): the set keys a partial or excluding export writes. `run` drops every job, skip, failure and
    /// fill end of a set not listed before its first event, so the summary and events cover only them; an entry with
    /// no set stays. `None` (the CLI, and a whole-project export with nothing left out) writes everything.
    pub write_only: Option<BTreeSet<String>>,
}

/// What a run does with a texture whose mip 0 adds little detail over mip 1 (`CHK-LOW-DETAIL`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LowDetailMode {
    /// Nothing is measured.
    #[default]
    Off,
    /// The warning is emitted and the file is written as usual.
    Warn,
}

/// How far a plan's per-file pass has got: `total` is the number of files it loads and resolves (duplicates and
/// skipped DDS inputs left out), `done` the files finished so far, cache hits included. One per request, so a
/// cancelled plan still draining never counts into the next one.
#[derive(Debug, Clone, Default)]
pub struct PlanProgress {
    pub done: Arc<AtomicUsize>,
    pub total: Arc<AtomicUsize>,
}

impl BatchRequest {
    /// `export` is the export folder; the request plans under `set_export`'s output root inside it (Note HQ1).
    pub fn new(inputs: Vec<PathBuf>, export: PathBuf, target: Target) -> Self {
        let mut r = BatchRequest {
            inputs,
            output_root: PathBuf::new(),
            target: Some(target),
            preset: None,
            preset_files: vec![],
            user_config: None,
            user_presets: None,
            project_config: None,
            project_config_inline: None,
            overrides: vec![],
            accept: AcceptSuggestions::None,
            allow: BTreeSet::new(),
            file_allow: BTreeMap::new(),
            set_edits: vec![],
            output_paths: BTreeMap::new(),
            shared_roughness: BTreeSet::new(),
            shared_roughness_groups: Vec::new(),
            borrowed_normal: BTreeMap::new(),
            set_shaders: BTreeMap::new(),
            pinned_files: BTreeSet::new(),
            mesh_shaders: BTreeMap::new(),
            mesh_alpha: BTreeMap::new(),
            gras_diffuses: BTreeSet::new(),
            gras_all_diffuses: BTreeSet::new(),
            cs_txst_sets: BTreeSet::new(),
            recursive: false,
            on_existing: OnExisting::IfChanged,
            ev: EvScope::Off,
            jobs: 0,
            mem_budget: 0,
            dry_run: false,
            validate: false, // Note LG: off, as the owner ruled ("Off, like sdc").
            plan_progress: None,
            low_detail: LowDetailMode::Off,
            derive: None,
            skip_root: None,
            export_root: None,
            write_only: None,
        };
        r.set_export(export);
        r
    }

    /// Note HQ1: plan into export folder `export`. The output root is `export` below the first input folder's own
    /// `textures` folders (`discover::effective_output_root`), so outputs land at their game paths while `rel`, set ids
    /// and the manifest stay relative to it, while `output_paths` stay relative to `export` (`export_root`).
    /// Discovery then skips the whole export folder, unless that folder holds the input root (HQ5: it writes beside
    /// the source). A first input that is a file, or no input, adds nothing.
    // shortcut: later input folders share the first one's prefix; upgrade if multi-root CLI runs need their own.
    pub fn set_export(&mut self, export: PathBuf) {
        let root = self.inputs.first().filter(|p| p.is_dir());
        self.output_root = match root {
            Some(root) => crate::discover::effective_output_root(root, &export),
            None => export.clone(),
        };
        let moved = self.output_root != export;
        let holds_root = root.is_some_and(|r| crate::discover::holds(&export, r));
        self.skip_root = (moved && !holds_root).then(|| export.clone());
        self.export_root = moved.then_some(export);
    }

    /// A hash of every field except the `[files]` table of `project_config_inline` and `plan_progress` (live
    /// counters, not an input). Fields are hashed through their `Debug` form (most are not `Hash`), streamed into
    /// the hasher with no copy of the request. Every map and set reachable from a request is a `BTreeMap`/`BTreeSet`
    /// (`serde_json` without `preserve_order` included), so the form is the same whatever the insertion order; a
    /// `HashMap` field would need hashing in sorted form. Stable within a build.
    pub fn sig_without_files(&self) -> u64 {
        use std::fmt::Write;
        use std::hash::Hasher;
        struct Sink(std::collections::hash_map::DefaultHasher);
        impl Write for Sink {
            fn write_str(&mut self, s: &str) -> std::fmt::Result {
                self.0.write(s.as_bytes());
                Ok(())
            }
        }
        let mut h = Sink(Default::default());
        // Destructured, so a new field fails to compile until it is hashed or left out on purpose.
        let BatchRequest {
            inputs,
            output_root,
            target,
            preset,
            preset_files,
            user_config,
            user_presets,
            project_config,
            project_config_inline,
            overrides,
            accept,
            allow,
            file_allow,
            set_edits,
            output_paths,
            shared_roughness,
            shared_roughness_groups,
            borrowed_normal,
            set_shaders,
            pinned_files,
            mesh_shaders,
            mesh_alpha,
            gras_diffuses,
            gras_all_diffuses,
            cs_txst_sets,
            recursive,
            on_existing,
            ev,
            jobs,
            mem_budget,
            dry_run,
            // A run setting: it changes no plan, so toggling it re-plans nothing.
            validate: _,
            plan_progress: _,
            low_detail,
            derive,
            skip_root,
            export_root,
            // Note HF9: the run's filter, not a planning input (the export plan does not use the skeleton).
            write_only: _,
        } = self;
        let fields: [&dyn std::fmt::Debug; 34] = [
            inputs,
            output_root,
            target,
            preset,
            preset_files,
            user_config,
            user_presets,
            project_config,
            overrides,
            accept,
            allow,
            file_allow,
            set_edits,
            output_paths,
            shared_roughness,
            shared_roughness_groups,
            borrowed_normal,
            set_shaders,
            pinned_files,
            mesh_shaders,
            mesh_alpha,
            gras_diffuses,
            gras_all_diffuses,
            cs_txst_sets,
            recursive,
            on_existing,
            ev,
            jobs,
            mem_budget,
            dry_run,
            low_detail,
            derive,
            skip_root,
            export_root,
        ];
        for f in fields {
            let _ = write!(h, "{f:?};");
        }
        match project_config_inline {
            None => {
                let _ = write!(h, "None;");
            }
            Some(ConfigFile {
                schema_version,
                preset,
                target,
                defaults,
                subtypes,
                rules,
                files: _,
                inputs,
                export,
            }) => {
                let inline: [&dyn std::fmt::Debug; 8] = [
                    schema_version,
                    preset,
                    target,
                    defaults,
                    subtypes,
                    rules,
                    inputs,
                    export,
                ];
                for f in inline {
                    let _ = write!(h, "{f:?};");
                }
            }
        }
        h.0.finish()
    }
}

#[derive(Debug, Clone)]
pub struct SkippedFile {
    pub path: PathBuf,
    /// The output the file would have written (its job key is this relative to the output root).
    pub output: PathBuf,
    /// The key of the texture set the file belongs to.
    pub set: Option<String>,
    pub code: Option<Code>,
    pub reason: String,
}

impl SkippedFile {
    /// What the skip means for a consumer of events: `IN-DDS-SKIPPED` (and `DERIVE-DDS-SOURCE`, a set whose DDS sources are
    /// not re-read, Note CN16), `PACK-CONFLICT`, `GEN-REPLACED` (Note CO), else `IN-DUPLICATE`.
    pub fn kind(&self) -> SkipKind {
        match self.code {
            Some(Code::InDdsSkipped | Code::DeriveDdsSource) => SkipKind::DdsInput,
            Some(Code::PackConflict) => SkipKind::PackConflict,
            Some(Code::DeriveNotConverted | Code::SkinWetGrey) => SkipKind::PackingInput,
            Some(Code::GenReplaced) => SkipKind::GeneratedReplaced,
            _ => SkipKind::Duplicate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FailedFile {
    pub path: PathBuf,
    /// The output the file would have written.
    pub output: PathBuf,
    pub stage: Stage,
    pub error: String,
    /// The key of the set the file belongs to, filled once the sets are final (`None` while planning and for a
    /// file in no set).
    pub set: Option<String>,
}

/// Why planning produced no job for a file: its Load failed, or it panicked at the stage it had entered.
enum PlanFailure {
    Load(LoadError),
    Panic(Stage, String),
    /// Assembling a packed image failed other than by a load error (spec §6.4): a source changed size after
    /// pass 1. A load error of a source is a `PlanFailure::Load`, handled like a pass-1 load failure.
    Pack(String),
}

impl PlanFailure {
    /// An assembly failure: a source's load error is a `Load`, anything else a `Pack`.
    fn of_pack(e: pack::assemble::PackError) -> Self {
        match e {
            pack::assemble::PackError::Load(e) => PlanFailure::Load(e),
            e => PlanFailure::Pack(e.to_string()),
        }
    }
}

/// Packing pass 2 and the derive step (spec §5.9, §6.4; Note CN): each item is admitted on the calling thread at
/// `est(item)` bytes, then `work` runs on the pool under `catch_unwind`, so a panic fails that item alone at the stage
/// it had entered (preflight P18). One result per item, in order; `None` = cancelled before its Load started.
fn admit_all<T: Sync>(
    pool: &rayon::ThreadPool,
    budget: &Budget,
    cancel: &CancelToken,
    items: &[T],
    est: impl Fn(&T) -> u64,
    #[cfg_attr(not(feature = "test-hooks"), allow(unused_variables))] output: impl Fn(&T) -> &Path
        + Sync,
    work: impl Fn(&T, Admitted<'_>, &Cell<Stage>) -> Result<ResolvedJob, PlanFailure> + Sync,
) -> Vec<Option<Result<ResolvedJob, PlanFailure>>> {
    let slots: Vec<Mutex<Option<Result<ResolvedJob, PlanFailure>>>> =
        items.iter().map(|_| Mutex::new(None)).collect();
    pool.in_place_scope(|scope| {
        for (item, slot) in items.iter().zip(&slots) {
            if cancel.is_cancelled() {
                continue;
            }
            let est = est(item);
            budget.acquire(est);
            let admitted = Admitted(budget, est);
            let work = &work;
            #[cfg(feature = "test-hooks")]
            let output = &output;
            scope.spawn(move |_| {
                if cancel.is_cancelled() {
                    return;
                }
                let stage = Cell::new(Stage::Load);
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    // Named by the output, so the pass-1 hook (named by the input) never fires here.
                    #[cfg(feature = "test-hooks")]
                    panic_while_planning_for_test(output(item), Stage::Load);
                    work(item, admitted, &stage)
                }));
                *lock(slot) = Some(r.unwrap_or_else(|panic| {
                    Err(PlanFailure::Panic(
                        stage.get(),
                        format!("internal error: {}", panic_message(&*panic)),
                    ))
                }));
            });
        }
    });
    slots
        .into_iter()
        .map(|s| s.into_inner().unwrap_or_else(|e| e.into_inner()))
        .collect()
}

/// The §10 code of a load failure.
fn load_code(e: &LoadError) -> Code {
    match e {
        LoadError::TooLarge { .. } => Code::DimTooLarge,
        LoadError::Decode { .. } => Code::InDecode,
    }
}

pub struct Plan {
    pub request: BatchRequest,
    pub registry: PresetRegistry,
    pub preset_id: String,
    /// The batch target as resolved from `request.target`, the configs or the preset.
    pub target: Target,
    /// Every resolved file, plus one job per packed output (spec §6.4); raw maps consumed by a recipe are not
    /// listed.
    pub jobs: Vec<ResolvedJob>,
    /// Files reported before Resolve: `IN-DUPLICATE` and `IN-DDS-SKIPPED`, and `PACK-CONFLICT` (a pre-packed file
    /// replaced under `packing.prefer = raw`).
    pub skipped: Vec<SkippedFile>,
    /// Files that could not be loaded (`DIM-TOO-LARGE`, `IN-DECODE`), or that panicked while planning (at the
    /// stage they had entered: Load, Analyse or Resolve).
    pub failed: Vec<FailedFile>,
    /// The most estimated bytes admitted at once while loading and analysing (spec §5.9 memory admission).
    pub peak_admitted_bytes: u64,
    /// Files whose Load/Analyse never ran because the batch was cancelled while planning. `run` lists them as
    /// cancelled in `batch_done` (exit 1).
    pub cancelled: Vec<PathBuf>,
    /// M6 (DECISIONS Note S): set key → the set's shader pick.
    pub shaders: BTreeMap<String, ShaderPick>,
    /// Note CN23: members of derived or dropped sets whose own output a job of this plan writes (no TA12 fold: a
    /// derived diffuse on its albedo's path, a kept landscape `_p`). That job's event ends them, so `run` emits no
    /// `packed_source` for them.
    pub derive_own: BTreeSet<PathBuf>,
    /// Note DY (review 4 I1): helpers an alpha fill consumed whose target has no job left (failed or cancelled while
    /// planning, or replaced): (helper, the output it filled, set key). `run` ends each with a `packed_source` skip.
    pub fill_ends: Vec<(PathBuf, PathBuf, String)>,
    /// Note DY: every helper of the batch (`helper::report::reports`), by set key then file name.
    pub helpers: Vec<crate::helper::report::HelperReport>,
    /// Note HF1: the files `[project] no_set` names (`sets::no_set_paths`). Their jobs stay in `jobs` (the GUI lists
    /// them), but `run` never writes one, whatever `write_only` says.
    pub no_set: BTreeSet<PathBuf>,
    /// Note HH4: the sets whose jobs `run` never writes (every member is in `no_set`); a set sharing a no-set helper
    /// is not one.
    pub no_set_sets: BTreeSet<String>,
}

impl Plan {
    fn scoped_jobs(&self) -> impl Iterator<Item = &ResolvedJob> {
        self.jobs.iter().filter(|job| {
            !self.no_set_sets.contains(&job.set_key)
                && self
                    .request
                    .write_only
                    .as_ref()
                    .is_none_or(|only| only.contains(&job.set_key))
        })
    }

    fn ready_jobs(&self) -> impl Iterator<Item = &ResolvedJob> {
        self.scoped_jobs()
            .filter(|job| matches!(job.status, JobStatus::Ready))
    }
}

/// The suffix after `up to date; ` in the `file_skipped` reason of a current output checked in place that passed.
pub const CHECKED_IN_PLACE: &str = "checked in place";
/// The same suffix when the check's validation reported an error.
pub const CHECKED_IN_PLACE_FAILED: &str = "checked in place: failed validation";

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct BatchSummary {
    /// Written outputs, including those whose validation failed.
    pub converted: u32,
    /// Outputs kept under `if-changed`: current ones and the kept ones counted below.
    pub up_to_date: u32,
    /// Of `up_to_date`, outputs checked in place during this run.
    pub checked: u32,
    /// Of `checked`, outputs whose validation reported an error.
    pub checked_failed: u32,
    /// Of `up_to_date`, outputs written by an older encode version and kept (Note JA; `--ev` re-encodes them).
    pub kept_older_encode: u32,
    /// Of `up_to_date`, outputs encoded at a higher quality target than asked and kept (Note JA).
    pub kept_lower_target: u32,
    /// Every `file_skipped` except up-to-date files and raw packing inputs.
    pub skipped: u32,
    /// Of `skipped`, those skipped on purpose: `IN-DDS-SKIPPED` inputs, `PACK-CONFLICT` pre-packed files replaced
    /// under `packing.prefer = raw` and outputs kept by an explicit `--on-existing skip` (low-detail textures are no
    /// longer held). They do not make the batch partial (DECISIONS Notes I, M9).
    pub skipped_intentional: u32,
    pub failed: u32,
    /// Written outputs whose validation reported an error (`passed = false`); also counted in `converted`.
    pub validation_failed: u32,
    /// Sorted.
    pub cancelled: Vec<PathBuf>,
}

impl BatchSummary {
    /// Spec §8 precedence for a batch that ran: 1 partial (unintentionally skipped, failed or cancelled
    /// files) > 4 validation failed on a written or checked output > 0. Up-to-date files, raw packing inputs and
    /// intentional skips (DECISIONS Note I) do not make a batch partial. (2 and 3 come from
    /// `EngineError`.)
    pub fn exit_code(&self) -> i32 {
        let unintended = self.skipped.saturating_sub(self.skipped_intentional);
        if unintended + self.failed > 0 || !self.cancelled.is_empty() {
            1
        } else if self.validation_failed + self.checked_failed > 0 {
            4
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst)
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.0.clone()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Bad flag, `--set` value or input path (exit 2).
    #[error("{0}")]
    Usage(String),
    /// Invalid preset or config file, including `PresetError::Alias` (`CFG-INVALID`: a bad or conflicting suffix
    /// alias, or the reserved `sdc-base` id) (exit 2).
    #[error("{0}")]
    Config(String),
    /// Output-root I/O, encoder initialisation, internal error (exit 3).
    #[error("{0}")]
    Fatal(String),
}

impl EngineError {
    pub fn exit_code(&self) -> i32 {
        match self {
            EngineError::Usage(_) | EngineError::Config(_) => 2,
            EngineError::Fatal(_) => 3,
        }
    }
}

/// A preset or config file error.
fn config_err(e: PresetError) -> EngineError {
    EngineError::Config(e.to_string())
}

/// A preset error reached through a flag (`--preset <id>`): `Alias` stays a config error.
fn preset_err(e: PresetError) -> EngineError {
    match e {
        PresetError::Alias(m) => EngineError::Config(m),
        e => EngineError::Usage(e.to_string()),
    }
}

/// Lock `m`, taking the data of a poisoned lock as it is (engine and session share this).
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Memory admission (spec §5.9). Only the scheduling thread acquires; workers release.
///
/// Besides bytes, admission caps the files admitted and not yet finished at `max_files` (the pool size). A pool
/// worker waiting inside one file's band-parallel encode may pick up another admitted file from rayon's queue and
/// run it on the same stack; the cap bounds that nesting by the thread count (review M1).
struct Budget {
    limit: u64,
    max_files: usize,
    /// (bytes, files) admitted and not yet released.
    used: Mutex<(u64, usize)>,
    /// The most bytes ever admitted at once.
    peak: AtomicU64,
    cv: Condvar,
}

impl Budget {
    fn new(limit: u64, max_files: usize) -> Self {
        Budget {
            limit,
            max_files: max_files.max(1),
            used: Mutex::new((0, 0)),
            peak: AtomicU64::new(0),
            cv: Condvar::new(),
        }
    }
    /// Waits until `n` more bytes fit and fewer than `max_files` files are in flight. A file larger than the
    /// whole budget is admitted alone.
    fn acquire(&self, n: u64) {
        let mut used = lock(&self.used);
        while used.1 >= self.max_files || (used.0 > 0 && used.0.saturating_add(n) > self.limit) {
            used = self.cv.wait(used).unwrap_or_else(|e| e.into_inner());
        }
        used.0 += n;
        used.1 += 1;
        self.peak.fetch_max(used.0, Ordering::Relaxed);
    }
    fn release(&self, n: u64) {
        {
            let mut used = lock(&self.used);
            used.0 -= n;
            used.1 -= 1;
        }
        self.cv.notify_all();
    }
}

/// Releases a file's admitted bytes when dropped, so a panic cannot leak budget.
struct Admitted<'a>(&'a Budget, u64);

impl Drop for Admitted<'_> {
    fn drop(&mut self) {
        self.0.release(self.1);
    }
}

/// Peak bytes one file holds while converting: the float base plus its mip chain (about 1.34x), counted three
/// times for the loaded image, the chain and the encoder inputs, decoded validation levels and output bytes.
/// A DDS input (`image` cannot read its size) also holds its file data while mip 0 decodes (`load::load_dds`).
fn estimated_bytes(path: &Path) -> u64 {
    input_dims(path).map_or(0, |d| bytes_at(path, d))
}

/// [`estimated_bytes`] of `path` whose size `dims` is already read.
fn bytes_at(path: &Path, (w, h): (u32, u32)) -> u64 {
    let file = if is_dds(path) {
        std::fs::metadata(path).map_or(0, |m| m.len())
    } else {
        0
    };
    input_bytes(w, h) + file
}

/// [`estimated_bytes`] of each of `paths` when all are brought to their common size (Note FM30): a smaller source is
/// held enlarged, so it counts at the common size. Equal sizes give [`estimated_bytes`] unchanged.
fn grown_bytes<'a>(paths: impl Iterator<Item = &'a Path>) -> Vec<u64> {
    let each: Vec<(&Path, Option<(u32, u32)>)> = paths.map(|p| (p, input_dims(p))).collect();
    let dims: Vec<_> = each.iter().filter_map(|e| e.1).collect();
    let grown = match crate::pack::common_size(&dims) {
        Some((w, h)) if dims.len() == each.len() => input_bytes(w, h),
        _ => 0,
    };
    each.iter()
        .map(|&(p, d)| d.map_or(0, |d| bytes_at(p, d)).max(grown))
        .collect()
}

fn is_dds(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("dds"))
}

/// The input's size: a DDS from its header (`load::dds_header_size`), anything else by content
/// (`load::image_size`, MAX_DIM-checked; a source refused here is refused at load too, so it holds nothing).
/// `None` when the size cannot be read.
fn input_dims(path: &Path) -> Option<(u32, u32)> {
    if is_dds(path) {
        let ((w, h), cube) = crate::load::dds_header_facts(path)?;
        // Clamped: a larger DDS is refused at load, and a hostile header must not overflow `input_bytes`.
        let dim = |v: u32| v.min(crate::load::MAX_DIM);
        // A cube DDS loads as a 6w x h strip (DECISIONS DS7), so that is what it holds.
        return Some((dim(w) * if cube { 6 } else { 1 }, dim(h)));
    }
    crate::load::image_size(path).ok()
}

/// [`estimated_bytes`] of a `w`×`h` input.
fn input_bytes(w: u32, h: u32) -> u64 {
    u64::from(w) * u64::from(h) * 16 * 134 / 100 * 3
}

/// The size `job` holds its input at: its own, or for a pack, a derive or an alpha fill the common size its sources
/// are enlarged to (Note FM30). `own` is `job.input`'s size, read once by the caller.
pub(crate) fn job_dims(job: &ResolvedJob, own: Option<(u32, u32)>) -> Option<(u32, u32)> {
    job_dims_by(job, own, &input_dims)
}

/// [`job_dims`] with each other source's size from `read`.
fn job_dims_by(
    job: &ResolvedJob,
    own: Option<(u32, u32)>,
    read: &dyn Fn(&Path) -> Option<(u32, u32)>,
) -> Option<(u32, u32)> {
    let mut paths: Vec<&Path> = vec![];
    paths.extend(
        job.pack
            .iter()
            .flat_map(|p| &p.sources)
            .map(|s| s.path.as_path()),
    );
    paths.extend(
        job.derive
            .iter()
            .flat_map(|d| &d.sources)
            .map(|s| s.path.as_path()),
    );
    if let Some(f) = &job.fill {
        paths.extend([job.input.as_path(), f.source.path.as_path()]);
    }
    let dims: Vec<_> = paths
        .iter()
        .filter_map(|&p| if p == job.input { own } else { read(p) })
        .collect();
    crate::pack::common_size(&dims)
        .filter(|_| dims.len() == paths.len())
        .or(own)
}

/// [`estimated_bytes`] of the job's input, plus what its variance bake holds (DECISIONS R4; the M4 carry "the
/// summed-area table counts against `--mem-budget`", a length pyramid since R5), plus (S-mips-11) 8 B/texel for a
/// non-box filter's intermediate and 32 B/texel for a resize (64 when an axis grows), both on the input's size, plus, for complex grass,
/// one more float chain (both half chains and the stacked chain are live together in `grass::build`).
fn job_bytes(job: &ResolvedJob) -> u64 {
    let p = &job.profile;
    let own = input_dims(&job.input);
    let dims = job_dims(job, own);
    let extra = dims.map_or(0, |(w, h)| {
        let texels = u64::from(w) * u64::from(h);
        let filter = if p.mips.filter != MipFilter::Box {
            8
        } else {
            0
        };
        let to = crate::resize::target_dims((w, h), p);
        let domain = crate::ops::domain_for(p);
        let lengths = if crate::mips::keeps_normal_lengths(
            p,
            p.subtype,
            domain,
            crate::mips::level_count(to.0, to.1, &p.mips.levels),
        ) {
            let reduced = (u64::from(to.0) / 2).max(1) * (u64::from(to.1) / 2).max(1);
            // Kept coat alpha also holds the unit base while reducing it into the first direction level.
            let directions = if crate::mips::coat_self_bake(p, domain, true).is_some() {
                (u64::from(to.0) * u64::from(to.1) + reduced) * 16
            } else {
                0
            };
            reduced * 8 + directions // two rolling f32 length levels
        } else {
            0
        };
        // An enlarge holds its premultiplied copy and two grown images (up to ~1.3x the input) on top.
        let resize = if to == (w, h) {
            0
        } else if to.0 > w || to.1 > h {
            64
        } else {
            32
        };
        let grass = if p.subtype == Subtype::ComplexGrass {
            texels * 16 * 134 / 100
        } else {
            0
        };
        // Note CN13: a derived job also holds the colour maps it reads, its scalars (and `_curves`/cavity) image and
        // the recipe's output, twice the input for the grass stack.
        let derived = job.derive.as_ref().map_or(0, |dj| {
            use crate::derive::DeriveKind::*;
            let colour = match dj.kind {
                VanillaDiffuse | CmDiffuse | LegacyN => 1,
                CmM | EnvM => 0,
                GrassStack => 2,
            };
            let detail = u64::from(dj.curves.is_some() || dj.cavity.is_some());
            let output = if dj.kind == GrassStack { 2 } else { 1 };
            texels * 16 * (colour + 1 + detail + output)
        });
        texels * (filter + resize) + grass + derived + lengths
    });
    // DECISIONS DS8: a cube holds every source plus three float cubes of its face size (base, blur copy, mip chain).
    let cube = job.cube.as_ref().map_or(0, |c| {
        let others: u64 = c
            .sources
            .iter()
            .skip(1)
            .map(|s| estimated_bytes(&s.path))
            .sum();
        let face = own.map_or(0, |src| {
            u64::from(crate::cube::layout::face_size(p, c.layout, src))
        });
        others + 6 * face * face * 16 * 3
    });
    own.map_or(0, |d| bytes_at(&job.input, d))
        .max(dims.map_or(0, |(w, h)| input_bytes(w, h)))
        + extra
        + cube
        // Note DY: `apply_fill` holds the helper's decoded image beside the target's.
        + job.fill.as_ref().map_or(0, |f| estimated_bytes(&f.source.path))
        + job
            .bake
            .as_ref()
            .map_or(0, crate::bake::BakeJob::estimated_bytes)
}

/// The admission limit: `--mem-budget`, else half of physical memory (spec §5.9).
fn budget_limit(req: &BatchRequest) -> u64 {
    if req.mem_budget > 0 {
        req.mem_budget
    } else {
        crate::sysmem::total_ram() / 2
    }
}

fn thread_pool(jobs: usize) -> Result<rayon::ThreadPool, EngineError> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .thread_name(|i| format!("sdc-worker-{i}"))
        .build()
        .map_err(|e| EngineError::Fatal(format!("thread pool: {e}")))
}

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// The batch target and where it came from: `--target`, the project config, the user config, the selected
/// preset's chain (nearest preset first), else the built-in `cs`.
fn resolve_target(
    req: &BatchRequest,
    project: Option<&ConfigFile>,
    user: Option<&ConfigFile>,
    preset_chain: &[&PresetFile],
) -> (Target, Provenance) {
    if let Some(t) = req.target {
        return (t, Provenance::User);
    }
    if let Some(t) = project.and_then(|c| c.target) {
        return (t, Provenance::ProjectConfig);
    }
    if let Some(t) = user.and_then(|c| c.target) {
        return (t, Provenance::UserConfig);
    }
    for p in preset_chain.iter().rev() {
        if let Some(t) = p.target {
            let src = if p.id == BASE_ID {
                Provenance::Builtin
            } else {
                Provenance::Preset(p.id.clone())
            };
            return (t, src);
        }
    }
    (Target::Cs, Provenance::Builtin)
}

/// The skip code of an unresolved job: its `SUG-LOW-CONFIDENCE` warning.
fn unresolved_code(job: &ResolvedJob) -> Code {
    job.warnings
        .iter()
        .map(|w| w.code)
        .find(|c| *c == Code::SugLowConfidence)
        .unwrap_or(Code::SugLowConfidence)
}

/// (SHA-256, bake identity, derive identity, fill identity) of one recorded source.
type SourceKey = (String, Option<String>, Option<String>, Option<String>);

/// Every recorded source of `job` now, as `SourceKey`s in record order: the pack sources, a derived job's sources,
/// a six-file cube's faces, the baked `_n` and the helper that fills the alpha (`manifest::recorded_sources`, the
/// list `process_file` records). Empty for an ordinary file; `None` when a source cannot be hashed (then the output
/// is not up to date).
fn current_sources(job: &ResolvedJob) -> Option<Vec<SourceKey>> {
    let now = recorded_sources(job).ok()?;
    Some(
        now.into_iter()
            .map(|r| (r.sha256, r.bake, r.derive, r.fill))
            .collect(),
    )
}

/// A packed job's sources fill the same roles, inverted the same way, as the record's (DECISIONS N7). A schema-1
/// record has no roles, so it never matches and the output re-encodes once. Ordinary jobs always match.
fn pack_roles_match(e: &ManifestEntry, job: &ResolvedJob) -> bool {
    match &job.pack {
        None => true,
        Some(pk) => e
            .sources
            .iter()
            .filter(|r| r.bake.is_none())
            .map(|r| (r.role, r.invert))
            .eq(pk
                .sources
                .iter()
                .map(|s| (Some(s.role), pack::invert_of(pk, &s.path)))),
    }
}

/// Whether the record's encode version equals the type's version now. A record without one (written by a 1.x
/// engine) is a known, stale baseline: it is never current.
pub(crate) fn encode_current(e: &ManifestEntry, now: &str) -> bool {
    e.encode_version.as_deref() == Some(now)
}

/// Whether `req`'s `--ev` scope covers the output of `job` recorded by `e`: the `ev` argument of [`currency`]. Only
/// `OnExisting::IfChanged` reads the scope (Note JA). `mips` reads the record's `validation.levels`, the written
/// file's header mip count (`ValidationSummary::levels`); a record without it is in scope.
pub(crate) fn ev_covers(req: &BatchRequest, e: &ManifestEntry, job: &ResolvedJob) -> bool {
    let levels = e
        .raw
        .get("validation")
        .and_then(|v| v.get("levels"))
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok());
    req.on_existing == OnExisting::IfChanged && req.ev.covers(&job.profile, levels)
}

/// An `if-changed` verdict on an existing output with a manifest record (Note JA).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Currency {
    /// Nothing that decides the bytes changed: the output is up to date.
    Current,
    /// Kept although an older encode version wrote it (`older_encode`, and the run's `--ev` scope leaves it out) or
    /// it was encoded at a higher quality target than the job asks (`lower_target`).
    Kept {
        older_encode: bool,
        lower_target: bool,
    },
    /// Encode it again.
    Stale,
}

#[cfg(test)]
thread_local! {
    static READY_EXISTENCE_CHECKS: Cell<usize> = const { Cell::new(0) };
}

/// Hash recorded outputs in parallel, preserving plan order for the final existence check and reporting.
fn classify_ready_jobs<'a>(
    plan: &'a Plan,
    existing: &BTreeMap<String, ManifestEntry>,
    pool: &rayon::ThreadPool,
    cancel: &CancelToken,
) -> Vec<(&'a ResolvedJob, String, Option<Currency>)> {
    use rayon::prelude::*;
    let req = &plan.request;
    let encoders = EncoderVersions::current();
    let ready: Vec<_> = plan.ready_jobs().collect();
    pool.install(|| {
        ready
            .into_par_iter()
            .map(|job| {
                let key = output_key(&req.output_root, &job.output);
                let recorded = existing.get(&key).map(|e| {
                    if req.on_existing == OnExisting::IfChanged
                        && !cancel.is_cancelled()
                        && job.output.exists()
                    {
                        currency(e, job, &encoders, ev_covers(req, e, job))
                    } else {
                        Currency::Stale
                    }
                });
                (job, key, recorded)
            })
            .collect()
    })
}

// Recheck at the verdict: the output can appear or disappear after parallel currency classification.
fn ready_output_exists(output: &Path) -> bool {
    #[cfg(test)]
    READY_EXISTENCE_CHECKS.set(READY_EXISTENCE_CHECKS.get() + 1);
    output.exists()
}

fn ready_verdict(policy: OnExisting, exists: bool, recorded: Option<Currency>) -> PreviewVerdict {
    match (policy, recorded) {
        _ if !exists => PreviewVerdict::WillWrite,
        (OnExisting::Overwrite, _) => PreviewVerdict::WillWrite,
        (OnExisting::Skip, _) => PreviewVerdict::OnExistingSkip,
        (_, None) => PreviewVerdict::UnknownExisting,
        (_, Some(Currency::Stale)) => PreviewVerdict::WillWrite,
        (_, Some(Currency::Current)) => PreviewVerdict::Current,
        (
            _,
            Some(Currency::Kept {
                older_encode,
                lower_target,
            }),
        ) => PreviewVerdict::Kept {
            older_encode,
            lower_target,
        },
    }
}

/// The if-changed rule (spec §8, carry 13; Note JA, the owner: re-encode when the image changed, not when only our
/// encoder did; a different encoder type, weighting or a higher quality target re-encodes, a lower one does not).
///
/// - Not comparable, so `Stale` whatever `ev` says: no encoders, no preset, a newer schema, or no or an empty
///   encode version (a 1.x record, Note DM).
/// - `Stale`: another output stamp (Note Y1), a failed or missing validation (ruling O2), other sources or pack
///   roles, or another source SHA-256.
/// - The encode version: another `encode_version` for the sub-type, another encoder tag (the rgbcx patch only for
///   BC1/BC3/BC4/BC5), or on BC1/BC3 a `bc1_model` other than [`BC1_ENCODE_ID`] (missing counts). With `ev` (the
///   run's `--ev` scope covers the output) `Stale`, else kept (`older_encode`).
/// - The profile: the same hash changes nothing. Else the record's own profile (`raw["profile"]`, which must hash to
///   its `profile_sha256`) with the job's three quality targets (`encoder.effort`, `encoder.bc45_search_radius`,
///   `encoder.bc1_search`) must hash as the job's profile, or `Stale` (format, weighting or any other field changed);
///   then a target that rose for the job's format is `Stale` and one that fell keeps it (`lower_target`).
///
/// Neither flag: `Current`. The engine version is not compared, nor is the record's `isa`.
pub(crate) fn currency(
    e: &ManifestEntry,
    job: &ResolvedJob,
    encoders: &EncoderVersions,
    ev: bool,
) -> Currency {
    let Some(rec) = e.encoders.as_ref() else {
        return Currency::Stale;
    };
    if e.is_newer_schema()
        || e.preset.is_none()
        || e.encode_version.as_deref().is_none_or(str::is_empty)
    {
        return Currency::Stale;
    }
    // Note Y1: metadata only, and before any hashing; a legacy record (no stamp) is stale once.
    if e.output_stamp.is_none() || SourceStamp::of(&job.output) != e.output_stamp {
        return Currency::Stale;
    }
    let passed = e
        .raw
        .get("validation")
        .and_then(|v| v.get("passed"))
        .and_then(Value::as_bool)
        == Some(true);
    let same_sources = || {
        current_sources(job).is_some_and(|now| {
            now.into_iter().eq(e.sources.iter().map(|r| {
                (
                    r.sha256.clone(),
                    r.bake.clone(),
                    r.derive.clone(),
                    r.fill.clone(),
                )
            }))
        })
    };
    if !(passed
        && same_sources()
        && pack_roles_match(e, job)
        && sha256_file(&job.input).is_ok_and(|h| h == e.source_sha256))
    {
        return Currency::Stale;
    }
    // The rgbcx patches change the BC1 colour block (BC1, BC3) and the BC4 blocks (BC3 alpha, BC4, BC5), so other
    // formats ignore the tag.
    let format = job.profile.format;
    let rgbcx = matches!(
        format,
        Format::Bc1 | Format::Bc3 | Format::Bc4 | Format::Bc5
    );
    let older_encode = !encode_current(e, encode_version(job.profile.subtype))
        || (&rec.bc7enc_rdo, &rec.ispctc, &rec.ispc, &rec.bc7e_patch)
            != (
                &encoders.bc7enc_rdo,
                &encoders.ispctc,
                &encoders.ispc,
                &encoders.bc7e_patch,
            )
        || (rgbcx && rec.rgbcx_patch != encoders.rgbcx_patch)
        || (matches!(format, Format::Bc1 | Format::Bc3)
            && e.bc1_model.as_deref() != Some(BC1_ENCODE_ID));
    if older_encode && ev {
        return Currency::Stale;
    }
    let Some(lower_target) = target_fell(e, &job.profile) else {
        return Currency::Stale;
    };
    if older_encode || lower_target {
        Currency::Kept {
            older_encode,
            lower_target,
        }
    } else {
        Currency::Current
    }
}

/// [`currency`]'s profile step: `Some(false)` when the record's profile is the job's or differs only in a quality
/// target that no format reads or that stayed level, `Some(true)` when a target fell (and none rose), `None` (stale)
/// otherwise, an unreadable or self-inconsistent record included. The exception is a `max` record not known to be
/// newer than 3.0.0 (Note LH): it reads as `slowest`, so against a job at `max` it is stale.
fn target_fell(e: &ManifestEntry, p: &Profile) -> Option<bool> {
    let now = profile_sha256(p).ok()?;
    // Equal hashes mean equal radii, so an old or undated `max` record against a job at `max` must not return here.
    let max_was_slowest = radius_rank(p.format, p.encoder.bc45_search_radius).is_some()
        && p.encoder.bc45_search_radius == SearchRadius::Max
        && old_max(e);
    if now == e.profile_sha256 && !max_was_slowest {
        return Some(false);
    }
    let mut was: Profile = serde_json::from_value(e.raw.get("profile")?.clone()).ok()?;
    if profile_sha256(&was).ok()? != e.profile_sha256 {
        return None;
    }
    let old = was.encoder.clone();
    let new = &p.encoder;
    was.encoder.effort = new.effort;
    was.encoder.bc45_search_radius = new.bc45_search_radius;
    was.encoder.bc1_search = new.bc1_search;
    if profile_sha256(&was).ok()? != now {
        return None;
    }
    let f = p.format;
    let moves = [
        effort_rank(f, new.effort).cmp(&effort_rank(f, old.effort)),
        radius_rank(f, new.bc45_search_radius)
            .cmp(&radius_rank(f, recorded_radius(e, old.bc45_search_radius))),
        search_rank(f, new.bc1_search).cmp(&search_rank(f, old.bc1_search)),
    ];
    if moves.contains(&std::cmp::Ordering::Greater) {
        return None;
    }
    Some(moves.contains(&std::cmp::Ordering::Less))
}

/// Note LH: through engine 3.0.0, `bc45_search_radius = "max"` was the radius-10 search, today's `slowest`.
/// shortcut: a 3.0.1 build from before Note LH also wrote `max` for radius 10; such dev records read as the new `max`.
const MAX_WAS_SLOWEST_THROUGH: [u32; 3] = [3, 0, 0];

/// Where a record's `engine_version` places it against a last version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecordAge {
    // At most the last version.
    Old,
    // Above it.
    New,
    // Missing, not a string, or not `X.Y.Z`: the manifest reader stores the first two as "".
    Unknown,
}

/// The age of engine version `v` (`X.Y.Z`, a `-pre` or `+build` suffix ignored) against `last`.
fn record_age(v: &str, last: [u32; 3]) -> RecordAge {
    let core = v.split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<Option<u32>> = core.split('.').map(|s| s.parse().ok()).collect();
    match parts[..] {
        [Some(a), Some(b), Some(c)] => {
            if [a, b, c] <= last {
                RecordAge::Old
            } else {
                RecordAge::New
            }
        }
        _ => RecordAge::Unknown,
    }
}

/// The radius a record's bytes were searched at: a `max` not known to be newer than 3.0.0 was `slowest`. An unknown
/// age counts as old (BC45-R9), so a `max` job re-encodes it rather than trusting bytes it cannot date.
fn recorded_radius(e: &ManifestEntry, r: SearchRadius) -> SearchRadius {
    if r == SearchRadius::Max && old_max(e) {
        SearchRadius::Slowest
    } else {
        r
    }
}

/// Whether the record was written by engine 3.0.0 or older, or by one that cannot be dated. It checks the age only;
/// the callers check that the radius is `max`.
fn old_max(e: &ManifestEntry) -> bool {
    record_age(&e.engine_version, MAX_WAS_SLOWEST_THROUGH) != RecordAge::New
}

/// How hard format `f` searches at `effort`: BC7 by its bc7e level, BC6H by its ISPC level (`slowest` and `max`
/// share one); `None` for the formats that do not read it.
fn effort_rank(f: Format, effort: Effort) -> Option<u8> {
    use encoders_sys::Bc6hLevel as L;
    match f {
        Format::Bc7 => Some(crate::encode::bc7_level(effort) as u8),
        // Exhaustive, so a new level fails to compile instead of reading as "not applicable".
        Format::Bc6hUf16 => Some(match crate::encode::bc6h_level(effort) {
            L::Veryfast => 0,
            L::Fast => 1,
            L::Basic => 2,
            L::Slow => 3,
            L::Veryslow => 4,
            L::VeryslowPeak => 5,
        }),
        _ => None,
    }
}

/// How hard format `f` searches its BC4 endpoints (BC3's alpha, BC4, BC5): `fast` < `default` < `slowest` < `max`
/// (Note LH), not the declared order and not the radius (Slowest and Max both start at 10); `None` elsewhere.
fn radius_rank(f: Format, r: SearchRadius) -> Option<u8> {
    matches!(f, Format::Bc3 | Format::Bc4 | Format::Bc5).then_some(match r {
        SearchRadius::Fast => 0,
        SearchRadius::Default => 1,
        SearchRadius::Slowest => 2,
        SearchRadius::Max => 3,
    })
}

/// The BC1 search depth of format `f` (BC1, BC3's colour): `fast` below `default`; `None` elsewhere.
fn search_rank(f: Format, s: Bc1Search) -> Option<u8> {
    matches!(f, Format::Bc1 | Format::Bc3).then_some(match s {
        Bc1Search::Fast => 0,
        Bc1Search::Default => 1,
    })
}

pub struct Engine {
    registry: PresetRegistry,
}

/// The folder `[files]` keys are relative to (DECISIONS N2, N4).
pub fn project_root(req: &BatchRequest) -> Option<PathBuf> {
    let first_dir = || req.inputs.first().filter(|p| p.is_dir()).cloned();
    if req.project_config_inline.is_some() {
        return first_dir();
    }
    match &req.project_config {
        // `--config sdc.toml` has an empty parent: that is the current directory.
        Some(p) => p.parent().map(crate::overrides::absolute_or_same),
        None => first_dir().filter(|d| d.join("sdc.toml").exists()),
    }
}

/// The project config: the inline one, `--config`, or the first input folder's `sdc.toml`.
fn load_project_config(req: &BatchRequest) -> Result<Option<ConfigFile>, EngineError> {
    if let Some(c) = &req.project_config_inline {
        return Ok(Some(c.clone()));
    }
    let project_path = req.project_config.clone().or_else(|| {
        req.inputs
            .first()
            .filter(|p| p.is_dir())
            .map(|p| p.join("sdc.toml"))
            .filter(|p| p.exists())
    });
    project_path
        .as_deref()
        .map(ConfigFile::load)
        .transpose()
        .map_err(config_err)
}

/// What [`Engine::preflight`]'s checks resolve, kept for planning.
struct Setup {
    registry: PresetRegistry,
    user_config: Option<ConfigFile>,
    project_config: Option<ConfigFile>,
    target: Target,
    target_src: Provenance,
    preset_id: String,
    preset_skips: Vec<Warning>,
}

/// The global half of a plan ([`Engine::prepare`]): everything before the per-set work, owned, so
/// [`Engine::plan_sets`] can build its `ResolveCtx` from a borrow of it.
#[derive(Clone)]
pub(crate) struct Prepared {
    pub(crate) req: BatchRequest,
    pub(crate) registry: PresetRegistry,
    pub(crate) user_config: Option<ConfigFile>,
    pub(crate) project_config: Option<ConfigFile>,
    pub(crate) target: Target,
    pub(crate) target_src: Provenance,
    pub(crate) preset_id: String,
    pub(crate) files: Vec<InputFile>,
    /// `group_sets_with`'s sets, before `drop_typed_roles` and the joins.
    pub(crate) grouped: Vec<TextureSet>,
    /// The sets after [`join_members`], before set edits.
    pub(crate) joined: Vec<TextureSet>,
    /// Final membership, before the predicted packing suffixes.
    pub(crate) sets: Vec<TextureSet>,
    pub(crate) file_scopes: FileScopes,
    pub(crate) hand: Vec<Option<crate::helper::HelperRole>>,
    pub(crate) typed: BTreeSet<usize>,
    pub(crate) dup_idx: BTreeSet<usize>,
    pub(crate) gen_replaced: Vec<SkippedFile>,
    pub(crate) packed_outputs: BTreeMap<(String, String), PathBuf>,
}

impl Prepared {
    /// The pass-1 `ResolveCtx` over this skeleton ([`base_ctx`]).
    pub(crate) fn ctx(&self) -> ResolveCtx<'_> {
        base_ctx(
            &self.req,
            self.target,
            &self.registry,
            &self.preset_id,
            self.user_config.as_ref(),
            self.project_config.as_ref(),
            &self.file_scopes,
        )
    }
}

/// The pass-1 `ResolveCtx` (no shader picks). `prepare` and `plan_sets` both build theirs here, so the two cannot
/// drift.
#[allow(clippy::too_many_arguments)]
fn base_ctx<'a>(
    req: &'a BatchRequest,
    target: Target,
    registry: &'a PresetRegistry,
    preset_id: &'a str,
    user_config: Option<&'a ConfigFile>,
    project_config: Option<&'a ConfigFile>,
    file_scopes: &'a FileScopes,
) -> ResolveCtx<'a> {
    ResolveCtx {
        target,
        registry,
        preset: preset_id,
        user_config,
        project_config,
        overrides: &req.overrides,
        accept: req.accept,
        allow: &req.allow,
        files: file_scopes,
        shaders: &NO_SHADERS,
        mesh_alpha: &req.mesh_alpha,
    }
}

/// The head of the BV2 writer dedupe's IN-DUPLICATE reason, which the fast re-plan skeleton reads back.
pub(crate) const BV2_REASON: &str = "two outputs map to one path ";

/// A DDS input that `ctx` re-encodes (otherwise it is IN-DDS-SKIPPED).
pub(crate) fn reencodes(ctx: &ResolveCtx, f: &InputFile) -> bool {
    f.kind == InputKind::Dds && crate::policy::reencode_requested(ctx, f)
}

/// Note HE7: the images that write nothing of their own (a raw map, a skin `_f`): pack planning
/// consumes them or reports them unused, so two of one name are not duplicates and `prepare` leaves them out of the
/// `IN-DUPLICATE` comparison. The first by path feeds the channel (M13). The fast re-plan runs this on the old and the
/// new state, so an edit that moves any file in or out is seen.
/// shortcut: a file a `[[rules]]` entry types is invisible here (rules need the analysis); the BV2 writer dedupe
/// catches two of those, so none writes twice. Revisit if a rule needs the pre-pass to see it.
/// shortcut: a fuzz map the set reads as the PBR fuzz texture (Fuzz-1a) counts here by its raw name, although it
/// writes `<base>_f.dds`; two that would both write it are left to the BV2 writer dedupe (JQ40). Revisit if the
/// pre-pass must see the reading.
pub(crate) fn waiting_images(
    ctx: &ResolveCtx,
    target: Target,
    sets: &[TextureSet],
    files: &[InputFile],
    typed: &BTreeSet<usize>,
    hand: &[Option<crate::helper::HelperRole>],
) -> BTreeSet<PathBuf> {
    let mut waits = BTreeSet::new();
    for set in sets {
        let mut predicted = set.clone();
        let skin_fuzz = skin_fuzz(ctx, set, files);
        predict_suffixes(&mut predicted, skin_fuzz, target, false, files, typed, hand);
        for &i in set.members.iter().filter(|i| !typed.contains(i)) {
            let f = &files[i];
            let writes_nothing = match hand[i] {
                Some(r) => matches!(r, crate::helper::HelperRole::Raw(_)),
                None => {
                    pack::plan::raw_role(f).is_some() || pack::plan::is_skin_f(ctx, &predicted, f)
                }
            };
            if f.kind == InputKind::Image && writes_nothing {
                waits.insert(f.path.clone());
            }
        }
    }
    waits
}

/// The `skin_fuzz` argument of [`predict_suffixes`]: whether `set` (not yet predicted) is Advanced skin, read from its
/// first member; false for a set with no members.
pub(crate) fn skin_fuzz(ctx: &ResolveCtx, set: &TextureSet, files: &[InputFile]) -> bool {
    set.members
        .first()
        .is_some_and(|&i| pack::plan::set_is_advanced_skin(ctx, set, &files[i]))
}

/// Note DY: an image's hand-set `helper.role`.
pub(crate) fn hand_role(ctx: &ResolveCtx, f: &InputFile) -> Option<crate::helper::HelperRole> {
    (f.kind == InputKind::Image)
        .then(|| crate::policy::helper_requested(ctx, f).role)
        .flatten()
}

/// Fuzz-1a: `f` is a fuzz map image (`group::is_fuzz_map_image`) with no requested sub-type and no hand role, so its
/// reading is the set's to decide.
// shortcut: a sub-type a `[[rules]]` entry names is not seen (rules need the analysis, as `is_typed` says), so a fuzz
// map a preset rule types `pbr_fuzz` counts as untyped: with a `_g` the set picks Glow, yet the map converts to
// `<base>_f.dds` and nothing lists it unused. Revisit if a rule needs the pre-pass to see it.
pub(crate) fn untyped_fuzz_map(ctx: &ResolveCtx, f: &InputFile) -> bool {
    crate::group::is_fuzz_map_image(f)
        && hand_role(ctx, f).is_none()
        && !crate::policy::subtype_requested(ctx, f)
}

/// Fuzz-1a: on cs, `set` (with no `_f` of its own) reads its fuzz map as the PBR fuzz texture by the picks in
/// `ctx.shaders` (`pack::plan::fuzz_reading`); the map is untyped and not hand-set, or typed `pbr_fuzz`. Such a set
/// writes `<base>_f.dds`, so its other members (6c) and its `set_resolved` read it with `f`.
// shortcut: member fuzz maps only (here and in `shader::pick_for_set`'s `PickInput::fuzz`), while
// `pack::plan::has_fuzz_map` counts a share: a set whose only fuzz map is shared in (Note HH1) gets neither Fuzz-1a nor
// `f`; read `set.shares` too when the shared fuzz map follow-up (F10) lands.
pub(crate) fn pbr_read_fuzz(ctx: &ResolveCtx, set: &TextureSet, files: &[InputFile]) -> bool {
    ctx.target == Target::Cs
        && pack::plan::pbr_f_members(ctx, set, files).next().is_none()
        && set
            .members
            .iter()
            .map(|&i| &files[i])
            .find(|f| {
                untyped_fuzz_map(ctx, f)
                    || (crate::group::is_fuzz_map_image(f)
                        && crate::policy::subtype_override(ctx, f) == Some(Subtype::PbrFuzz))
            })
            .is_some_and(|fm| {
                pack::plan::fuzz_reading(ctx, set, fm) == pack::plan::FuzzReading::Pbr
            })
}

/// Fuzz-1a: a copy of `set` whose suffixes hold the `f` its PBR-read fuzz map writes, as M7 adds a predicted `_rmaos`.
fn with_f(set: &TextureSet) -> TextureSet {
    let mut c = set.clone();
    c.suffixes.insert("f".into());
    c
}

/// Fuzz-1a (Note JQ52): the set member `f` of `set` is typed and checked against. In a set of `f_sets` (its fuzz map
/// is read as the PBR fuzz texture, `pbr_read_fuzz`) the other members read [`with_f`]; the fuzz map itself reads the
/// real set, whose rename guard (JQ37) and `_f` arm read the set's own `f`. 6c, the fill step and the shader map check
/// (`map_members`) all ask here, so a member is typed and checked against one set. The set itself never gains `f`.
/// Note JQ53: a set of `f_sets` whose suffixes hold `f` has no `_f` of its own (`pack::plan::pbr_f_members`): its `f` is a
/// kept DDS, the fuzz map's earlier output. The fuzz map reads a copy without it, so policy's Fuzz-4 test and
/// `fuzz_f_output`'s guard see no `_f` and the map still writes `<base>_f.dds`.
fn member_set<'a>(
    set: &'a TextureSet,
    f: &InputFile,
    f_sets: &BTreeSet<String>,
) -> std::borrow::Cow<'a, TextureSet> {
    if !f_sets.contains(&set.key) {
        std::borrow::Cow::Borrowed(set)
    } else if !crate::group::is_fuzz_map_image(f) {
        std::borrow::Cow::Owned(with_f(set))
    } else if set.suffixes.contains("f") {
        let mut c = set.clone();
        c.suffixes.remove("f");
        std::borrow::Cow::Owned(c)
    } else {
        std::borrow::Cow::Borrowed(set)
    }
}

/// Trace-debug 2026-10-16 #4: a raw map whose sub-type was requested is an ordinary file (spec §4.4), as
/// `plan_packing` treats it, so its role leaves the set's suffixes (unless another raw member has it) and Advanced
/// skin detection and the `auto` prediction follow the recipe actually built. Note DY: a hand role `none` makes a raw
/// map an ordinary file, like a requested sub-type; any other hand role wins over a requested sub-type (ruling 13).
/// shortcut: rule-named sub-types are not seen here (rules need the analysis); revisit if a rule names one.
pub(crate) fn is_typed(
    ctx: &ResolveCtx,
    f: &InputFile,
    hand: Option<crate::helper::HelperRole>,
) -> bool {
    pack::plan::raw_role(f).is_some()
        && match hand {
            Some(r) => r == crate::helper::HelperRole::NotHelper,
            None => crate::policy::subtype_requested(ctx, f),
        }
}

/// Each typed raw member's role leaves its set's suffixes unless an untyped raw member has it too. A shared file
/// counts as a member (Note HH1).
pub(crate) fn drop_typed_roles(
    sets: &mut [TextureSet],
    files: &[InputFile],
    typed: &BTreeSet<usize>,
) {
    for set in sets.iter_mut() {
        let role = |i: usize| pack::plan::raw_role(&files[i]).map(|r| r.id());
        let (typed_m, raw_m): (Vec<usize>, Vec<usize>) = set
            .members
            .iter()
            .chain(&set.shares)
            .partition(|i| typed.contains(i));
        for r in typed_m.into_iter().filter_map(role) {
            if !raw_m.iter().any(|&j| role(j).as_ref() == Some(&r)) {
                set.suffixes.remove(&r);
            }
        }
    }
}

/// The membership joins of grouping, in `prepare`'s order.
pub(crate) fn join_members(
    sets: &mut Vec<TextureSet>,
    files: &[InputFile],
    aliases: &BTreeMap<String, String>,
    ctx: &ResolveCtx,
    hand: &[Option<crate::helper::HelperRole>],
    explicit: &BTreeMap<String, Shader>,
) {
    // Note DY: a hand-set helper with no recognised suffix (`x_mask`) joins set `x` when it exists.
    crate::group::join_helpers(sets, files, aliases, |i| {
        hand[i].is_some_and(|r| r != crate::helper::HelperRole::NotHelper)
    });
    // Final review I3 (DECISIONS Note R): `x_wet` joins set `x` as its skin `_wet` member only on cs and only
    // when `x` is Advanced skin. Before set edits, so they see the joined set like any automatic one.
    // shortcut: a set skin only by its chosen shader (Note DV7) is not seen here, unless it is shared (Note HG2, which
    // `set_is_advanced_skin` reads); use `explicit` here too if such a project needs its `_wet` joined.
    if ctx.target == Target::Cs {
        crate::group::mark_shared_skin(sets, files, aliases, explicit);
        let is_skin = |s: &TextureSet| {
            s.members
                .first()
                .is_some_and(|&i| pack::plan::set_is_advanced_skin(ctx, s, &files[i]))
        };
        join_skin_wet(sets, files, aliases, is_skin);
    }
    // Note GW1: `x_detail_n` joins any detail host, on every target (Landscape is on both).
    join_detail_n(sets, files, aliases, detail_host(ctx, files, explicit));
}

/// Note GW1: a set takes the detail names `_detail`, `_detailnorm` and `_detail_n` as one map when it is an Advanced skin
/// set (cs), or is landscape by its path (`suggest::landscape_path`) or by an explicit Landscape or PBR Landscape.
/// shortcut: a Landscape picked later (from a mesh) is not seen here, so such a set needs an explicit shader for
/// `_detail_n`; `plan_sets` repeats the IN-DUPLICATE rule after the pick.
pub(crate) fn detail_host<'a>(
    ctx: &'a ResolveCtx<'a>,
    files: &'a [InputFile],
    explicit: &'a BTreeMap<String, Shader>,
) -> impl Fn(&TextureSet) -> bool + 'a {
    move |s| {
        let first = s.members.first().map(|&i| &files[i]);
        first.is_some_and(|f| {
            (ctx.target == Target::Cs && pack::plan::set_is_advanced_skin(ctx, s, f))
                || crate::suggest::landscape_path(&f.game_path)
        }) || matches!(
            explicit.get(&s.key),
            Some(Shader::Landscape | Shader::PbrLandscape)
        )
    }
}

/// Spec §6.4 "Set membership", DECISIONS Note M7: a recipe's output joins its set before anything resolves.
/// Note AZ: a set explicitly given complex material predicts the complex `_m` (the auto pick comes later and needs
/// a member already typed complex_material, so it never adds a suffix of its own).
/// Fuzz-1: a fuzz map read from its name counts only when `skin_fuzz` (the set reads it as the skin mask,
/// `pack::plan::set_is_advanced_skin` on the set before prediction); a hand-set fuzz role always counts.
pub(crate) fn predict_suffixes(
    set: &mut TextureSet,
    skin_fuzz: bool,
    target: Target,
    complex: bool,
    files: &[InputFile],
    typed: &BTreeSet<usize>,
    hand: &[Option<crate::helper::HelperRole>],
) {
    // Note HH1: a shared helper's role is read as a member's.
    let roles: BTreeSet<RawMap> = set
        .members
        .iter()
        .chain(&set.shares)
        .filter(|i| !typed.contains(i))
        .filter_map(|&i| match hand[i] {
            Some(crate::helper::HelperRole::Raw(r)) => Some(r),
            Some(_) => None,
            None => pack::plan::raw_role(&files[i]).filter(|&r| skin_fuzz || r != RawMap::Fuzz),
        })
        .collect();
    for s in pack::predicted_suffixes(target, complex, &roles) {
        set.suffixes.insert(s.to_string());
    }
}

/// Whether `set` (its predicted suffixes included) is a PBR set, as its `set_resolved` reports it; `None` for a set
/// with no members.
pub(crate) fn set_is_pbr(target: Target, set: &TextureSet, files: &[InputFile]) -> Option<bool> {
    let &first = set
        .members
        .iter()
        .find(|&&i| files[i].kind == InputKind::Image)
        // Note FI2: an all-DDS set resolves too; `first` only feeds `is_pbr_set`'s game path.
        .or_else(|| set.members.first())?;
    Some(is_pbr_set(&SuggestContext {
        target,
        suffix: None,
        game_path: &files[first].game_path,
        set_suffixes: &set.suffixes,
        alpha: AlphaContent::None,
        is_float: false,
        advanced_skin: false,
        // Only `is_pbr_set` reads this context, and it never asks whether a file waits.
        may_wait: false,
    }))
}

impl Engine {
    pub fn new() -> Result<Self, EngineError> {
        Ok(Engine {
            registry: PresetRegistry::builtin().map_err(|e| EngineError::Fatal(e.to_string()))?,
        })
    }

    pub fn registry(&self) -> &PresetRegistry {
        &self.registry
    }

    /// The project config's `[export]`, else the default. Parses only: no `check()`, so a bad `[export.legacy]`
    /// value cannot break a plain export (O2). `run_also` checks it before anything else.
    pub fn export_config(&self, req: &BatchRequest) -> Result<ExportConfig, EngineError> {
        Ok(load_project_config(req)?
            .and_then(|c| c.export)
            .unwrap_or_default())
    }

    /// Spec §8 up-front checks without reading any input: load the preset files and the configs (the user
    /// config, `--config` or the first input folder's `sdc.toml`), resolve the target and preset, and build every
    /// profile with each rule and `--set` value. An invalid one is an error (exit 2). `plan` runs the same checks
    /// first; `inspect` calls this on its own so a DDS-only or empty input is held to them too (Codex-CLI 2).
    /// Returns the CFG-PRESET-SKIPPED warnings for the caller to report when it does not go on to `plan`.
    pub fn preflight(&self, req: &BatchRequest) -> Result<Vec<Warning>, EngineError> {
        self.setup(req).map(|s| s.preset_skips)
    }

    fn setup(&self, req: &BatchRequest) -> Result<Setup, EngineError> {
        let mut registry = self.registry.clone();
        // Spec §3: the user folder first (bad files skipped), then `--preset-file` (a bad one is an error).
        let preset_skips = req
            .user_presets
            .as_deref()
            .map(|d| registry.add_dir(d))
            .unwrap_or_default();
        for f in &req.preset_files {
            registry.add_file(f).map_err(config_err)?;
        }
        let user_path = req.user_config.as_deref().filter(|p| p.exists());
        let user_config = user_path
            .map(ConfigFile::load)
            .transpose()
            .map_err(config_err)?;
        if let (Some(p), Some(c)) = (user_path, &user_config) {
            if !c.files.is_empty() {
                return Err(EngineError::Config(format!(
                    "[files] is only valid in a project sdc.toml, not in {}",
                    p.display()
                )));
            }
            if c.export.is_some() {
                return Err(EngineError::Config(format!(
                    "[export] is only valid in a project sdc.toml, not in {}",
                    p.display()
                )));
            }
            if !c.inputs.is_empty() {
                return Err(EngineError::Config(format!(
                    "[inputs] is only valid in a project sdc.toml, not in {}",
                    p.display()
                )));
            }
        }
        let project_config = load_project_config(req)?;

        // Target and preset. An explicitly chosen preset may state the target; otherwise the target picks the
        // default preset.
        let chosen = req
            .preset
            .clone()
            .or_else(|| project_config.as_ref().and_then(|c| c.preset.clone()))
            .or_else(|| user_config.as_ref().and_then(|c| c.preset.clone()));
        let (target, target_src, preset_id) = {
            let chosen_chain = match &chosen {
                Some(id) => registry.chain(id).map_err(preset_err)?,
                None => vec![],
            };
            let (target, target_src) = resolve_target(
                req,
                project_config.as_ref(),
                user_config.as_ref(),
                &chosen_chain,
            );
            let preset_id = chosen.unwrap_or_else(|| default_preset_for(target).to_string());
            let chain = registry.chain(&preset_id).map_err(preset_err)?;
            let configs: Vec<&ConfigFile> =
                user_config.iter().chain(project_config.iter()).collect();
            check_up_front(req, &registry, target, &chain, &configs)?;
            (target, target_src, preset_id)
        };
        Ok(Setup {
            registry,
            user_config,
            project_config,
            target,
            target_src,
            preset_id,
            preset_skips,
        })
    }

    /// Discover, group, load, analyse and resolve every input, emitting `batch_started`, `file_discovered`,
    /// `set_resolved`, `file_planned` and every warning. Presets, configs, rules and `--set` values are
    /// validated first ([`Engine::preflight`]): any invalid one is an error (exit 2) before any file is read.
    /// Once `cancel` is set, no further file is loaded; those files are returned in `Plan::cancelled`. Each file's
    /// Load, Analyse and Resolve run under `catch_unwind`: a panic fails that file alone (`Plan::failed`).
    pub fn plan(
        &self,
        req: BatchRequest,
        rep: &Reporter,
        cancel: &CancelToken,
    ) -> Result<Plan, EngineError> {
        self.plan_cached(req, rep, cancel, &AnalysisCache::default())
    }

    /// [`Engine::plan`] reusing `cache`: a file whose size and modification time are unchanged is not reloaded, and
    /// neither is a packed image whose recipe and sources are unchanged (M3a spec §4.4).
    pub fn plan_cached(
        &self,
        req: BatchRequest,
        rep: &Reporter,
        cancel: &CancelToken,
        cache: &AnalysisCache,
    ) -> Result<Plan, EngineError> {
        self.plan_full(&req, rep, cancel, cache)
            .map(|(plan, ..)| plan)
    }

    /// The one composition of a full plan: [`Engine::prepare`] then every set through [`Engine::plan_sets`].
    /// Also returns the `Prepared`, the PBR set keys and the keys of the sets read with `f` (Fuzz-1a), which a
    /// session keeps as its skeleton.
    pub(crate) fn plan_full(
        &self,
        req: &BatchRequest,
        rep: &Reporter,
        cancel: &CancelToken,
        cache: &AnalysisCache,
    ) -> Result<(Plan, Prepared, BTreeSet<String>, BTreeSet<String>), EngineError> {
        let p = self.prepare(req, rep, cancel)?;
        let (plan, pbr, f) = self.plan_sets(&p, None, rep, cancel, cache)?;
        Ok((plan, p, pbr, f))
    }

    /// The global half of [`Engine::plan_cached`]: setup, discover, grouping, `[files]` scopes, set edits, output
    /// paths and duplicates. Emits, in order: `batch_started`; the CFG-PRESET-SKIPPED warnings, then the `[inputs]`
    /// read-as warnings; one IN-UNREADABLE per unreadable file; one SET-EDIT-STALE per skipped set edit; one
    /// `file_discovered` per input.
    pub(crate) fn prepare(
        &self,
        req: &BatchRequest,
        rep: &Reporter,
        cancel: &CancelToken,
    ) -> Result<Prepared, EngineError> {
        let _ = cancel; // nothing here checks it yet
        let Setup {
            registry,
            user_config,
            project_config,
            target,
            target_src,
            preset_id,
            preset_skips,
        } = self.setup(req)?;

        for i in &req.inputs {
            if !i.exists() {
                return Err(EngineError::Usage(format!(
                    "input not found: {}",
                    i.display()
                )));
            }
        }
        let (mut files, unreadable) = discover_reporting(
            &req.inputs,
            &req.output_root,
            req.recursive,
            req.skip_root.as_deref(),
        )
        .map_err(|e| EngineError::Usage(format!("discover: {e}")))?;
        // F3b: `[inputs]` aliases rename files before anything reads their names.
        let read_as_warnings = match (&project_config, project_root(req)) {
            (Some(c), Some(root)) if !c.inputs.is_empty() => {
                crate::discover::apply_read_as(&mut files, &root, &req.output_root, &c.inputs)
            }
            _ => vec![],
        };
        // Task 26a: canonical output names before grouping, so alias/canonical collisions are IN-DUPLICATE.
        canonicalise_outputs(&mut files, registry.alias_table());
        rep.emit(
            None,
            None,
            EventKind::BatchStarted {
                target,
                preset: preset_id.clone(),
                inputs: files.len(),
            },
        );
        for w in preset_skips.into_iter().chain(read_as_warnings) {
            rep.emit(None, None, EventKind::Warning(w));
        }
        for u in unreadable {
            rep.emit(
                None,
                None,
                EventKind::Warning(Warning::new(Code::InUnreadable, format!("skipped: {u}"))),
            );
        }
        let key = |out: &Path| output_key(&req.output_root, out);
        // M3b planning sequence (spec §2.1, DECISIONS P1). It is load-bearing:
        // 1. `discover`, then `[inputs]` aliases (`apply_read_as`, F3b), then `canonicalise_outputs` (above).
        // 2. `group_sets_with` gives the automatic sets; its duplicate-output groups are ignored from now on. On cs,
        //    `join_skin_wet` then moves a skin `_wet` member into its Advanced skin set.
        // 3. `apply_set_edits`. Set keys are fixed from here on: output overrides never change a key.
        // 4. `apply_output_paths` (Task 3).
        // 5. `duplicate_outputs_with` on the final paths, so two sets writing one file are caught after paths change.
        // 6. Predicted packing suffixes (they read the explicit shaders of 6a, Note AZ), the skip/work loop, then pass
        //    1 (Load, Analyse, Resolve with no shaders).
        // 6a. Explicit shader choices (`explicit_shaders`): `BatchRequest::set_shaders`, then `--set set:<id>.shader`.
        // 6b. One `ShaderPick` per set (`shader::pick_for_set`) from the pass-1 sub-types; `ctx.shaders` holds them.
        // 6c. Members of a set whose shader has member defaults are resolved again with the picks; then packing.
        //     A set that reads its fuzz map as the PBR fuzz texture (`pbr_read_fuzz`, Fuzz-1a) resolves its other
        //     members against a clone holding `f`, and its `set_resolved` reads a copy with `f`.
        let scope_root =
            project_root(req).or_else(|| req.inputs.first().filter(|p| p.is_dir()).cloned());
        let (mut sets, _) = group_sets_with(&files, registry.alias_table());
        let (file_overrides, mut file_allow) = match &project_config {
            Some(c) => c.file_entries().map_err(config_err)?,
            None => (vec![], BTreeMap::new()),
        };
        for (k, codes) in &req.file_allow {
            file_allow
                .entry(k.clone())
                .or_default()
                .extend(codes.iter().copied());
        }
        // Note DW: the pinned files that are a set's diffuse member (no suffix, or `_d`).
        let skin_pinned = match &scope_root {
            Some(root) if !req.pinned_files.is_empty() => {
                let index = crate::overrides::KeyIndex::new(root, &files);
                req.pinned_files
                    .iter()
                    .flat_map(|k| index.hits(k).iter().copied())
                    .filter(|&i| {
                        let stem = files[i].rel.file_stem().unwrap_or_default();
                        matches!(
                            crate::group::split_suffix_with(
                                &stem.to_string_lossy(),
                                registry.alias_table()
                            )
                            .suffix
                            .as_deref(),
                            None | Some("d")
                        )
                    })
                    .collect()
            }
            _ => BTreeSet::new(),
        };
        let mut file_scopes = FileScopes {
            root: scope_root.clone(),
            overrides: file_overrides,
            allow: file_allow,
            skin_pinned,
            ..FileScopes::default()
        };
        file_scopes.index(&files);
        // Fuzz-5, Fuzz-22: the skin evidence a set with a fuzz map reads (`set_is_advanced_skin`), from a ctx of its
        // own that is dropped before the sets are stored. Each image's hand-set role is read here once (it reads
        // neither field).
        let hand: Vec<Option<crate::helper::HelperRole>>;
        {
            let ctx = base_ctx(
                req,
                target,
                &registry,
                &preset_id,
                user_config.as_ref(),
                project_config.as_ref(),
                &file_scopes,
            );
            hand = files.iter().map(|f| hand_role(&ctx, f)).collect();
            // shortcut: a sub-type a `[[rules]]` entry names is not seen (rules need the analysis), as in `is_typed`;
            // revisit if a rule names a skin sub-type.
            file_scopes.skin_typed = (0..files.len())
                .filter(|&i| {
                    crate::policy::subtype_override(&ctx, &files[i]).is_some_and(Subtype::is_skin)
                })
                .collect();
            file_scopes.hand_fuzz = (0..files.len())
                .filter(|&i| hand[i] == Some(crate::helper::HelperRole::Raw(RawMap::Fuzz)))
                .collect();
        }
        let ctx = base_ctx(
            req,
            target,
            &registry,
            &preset_id,
            user_config.as_ref(),
            project_config.as_ref(),
            &file_scopes,
        );
        let typed: BTreeSet<usize> = (0..files.len())
            .filter(|&i| is_typed(&ctx, &files[i], hand[i]))
            .collect();
        // The skeleton keeps the grouping before the joins, so a patch can rerun them (fast re-plan).
        let grouped = sets.clone();
        drop_typed_roles(&mut sets, &files, &typed);
        let explicit = pre_edit_shaders(req, &sets);
        join_members(
            &mut sets,
            &files,
            registry.alias_table(),
            &ctx,
            &hand,
            &explicit,
        );
        let joined = sets.clone();
        let stale = crate::sets::apply_set_edits(
            &mut sets,
            &files,
            scope_root.as_deref(),
            &req.output_root,
            registry.alias_table(),
            &req.set_edits,
        );
        // A touched set's suffixes were rebuilt from its members' names.
        drop_typed_roles(&mut sets, &files, &typed);
        // Note HG2: decided on the edited sets, so a created or renamed set is shared by the same rule.
        let (explicit, _) = explicit_shaders(req, &sets);
        if target == Target::Cs {
            crate::group::mark_shared_skin(&mut sets, &files, registry.alias_table(), &explicit);
        }
        for s in &stale {
            rep.emit(
                None,
                None,
                EventKind::Warning(
                    Warning::new(
                        Code::SetEditStale,
                        format!("set edit skipped: {}", s.reason),
                    )
                    .with_data(serde_json::json!({ "edit": s.edit, "reason": s.reason })),
                ),
            );
        }
        // Note HQ1: explicit paths are under the export folder, which may hold the output root.
        let paths_root = req.export_root.as_deref().unwrap_or(&req.output_root);
        crate::sets::apply_output_paths(
            &mut files,
            scope_root.as_deref(),
            paths_root,
            &req.output_paths,
        );
        // Read by 3b too (CO102): a DDS at a packed output's explicit path is that output's earlier export.
        let packed_outputs =
            crate::sets::packed_outputs(&req.output_root, paths_root, &req.output_paths);
        // 3b, Note CO (the M9 fold): a generated member loses to a real map for its slot before duplicates and
        // pack planning see it, since a generated `_p` and a real `_p` share one output.
        let mut gen_replaced: Vec<SkippedFile> = vec![];
        let mut losers: BTreeSet<PathBuf> = BTreeSet::new();
        for (g, r) in crate::group::generated_losers(
            &sets,
            &files,
            registry.alias_table(),
            target,
            &packed_outputs,
        ) {
            let set = sets
                .iter_mut()
                .find(|s| s.members.contains(&g))
                .expect("a loser is a set member");
            set.members.retain(|&i| i != g);
            let suffix_of = |i: usize| {
                let stem = files[i]
                    .rel
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy();
                crate::group::split_suffix_with(&stem, registry.alias_table()).suffix
            };
            if let Some(sfx) = suffix_of(g) {
                if !set
                    .members
                    .iter()
                    .any(|&i| suffix_of(i).as_ref() == Some(&sfx))
                {
                    set.suffixes.remove(&sfx);
                }
            }
            let name = |i: usize| {
                files[i]
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            };
            let dds = if files[r].kind == InputKind::Dds {
                " (a DDS: not written to the output unless DDS inputs are re-encoded)"
            } else {
                ""
            };
            losers.insert(files[g].path.clone());
            gen_replaced.push(SkippedFile {
                path: files[g].path.clone(),
                output: files[g].output.clone(),
                set: Some(set.key.clone()),
                code: Some(Code::GenReplaced),
                reason: format!("generated {} replaced by {}{dds}", name(g), name(r)),
            });
        }
        for f in &files {
            rep.emit_job(
                Some(&key(&f.output)),
                None,
                Some(&f.path),
                EventKind::FileDiscovered {
                    output: slash(&f.output),
                },
            );
        }
        let waits = waiting_images(&ctx, target, &sets, &files, &typed, &hand);
        // Note HF1: a no-set file writes nothing, so it blocks no same-named file.
        let no_set = crate::sets::no_set_paths(&files, scope_root.as_deref(), &req.set_edits);
        let counts = |f: &InputFile| {
            !losers.contains(&f.path)
                && !waits.contains(&f.path)
                && !no_set.contains(&f.path)
                && (f.kind != InputKind::Dds || reencodes(&ctx, f))
        };
        let mut dups = crate::group::duplicate_outputs_with(&files, counts);
        // Note GW1: two detail maps of one host write different files for one slot.
        dups.extend(crate::group::duplicate_details(
            &sets,
            &files,
            registry.alias_table(),
            detail_host(&ctx, &files, &explicit),
            counts,
        ));
        let dup_idx: BTreeSet<usize> = dups.iter().flatten().copied().collect();
        Ok(Prepared {
            req: req.clone(),
            registry,
            user_config,
            project_config,
            target,
            target_src,
            preset_id,
            files,
            grouped,
            joined,
            sets,
            file_scopes,
            hand,
            typed,
            dup_idx,
            gen_replaced,
            packed_outputs,
        })
    }

    /// The per-set half of [`Engine::plan_cached`] on `p`: Load, Analyse and Resolve, shaders, packing, fills,
    /// derive, bakes and the plan's events, the keys of the sets whose `set_resolved` says `pbr` (the skeleton's
    /// `pbr_sets`), and the keys of the sets read with `f` from a PBR-read fuzz map (Fuzz-1a; its `f_sets`).
    /// `Some(scope)` (set keys, closed under [`crate::skeleton::Skeleton::scope`]) plans only those sets
    /// and skips the batch-wide steps: the SET-EDIT-STALE shader warnings, CFG-FILE-UNMATCHED, the BV2 writer dedupe
    /// and the progress counters (the request is the skeleton's). Its skipped entries are the scope files' only.
    pub(crate) fn plan_sets(
        &self,
        p: &Prepared,
        scope: Option<&BTreeSet<String>>,
        rep: &Reporter,
        cancel: &CancelToken,
        cache: &AnalysisCache,
    ) -> Result<(Plan, BTreeSet<String>, BTreeSet<String>), EngineError> {
        let req = &p.req;
        let registry = &p.registry;
        let (target, target_src, preset_id) = (p.target, &p.target_src, &p.preset_id);
        let (files, file_scopes) = (&p.files, &p.file_scopes);
        let (hand, typed, dup_idx) = (&p.hand, &p.typed, &p.dup_idx);
        // Mutated below (predicted suffixes, replaced generated members); the prepared sets stay as built.
        let mut sets: Vec<TextureSet> = p
            .sets
            .iter()
            .filter(|s| scope.is_none_or(|sc| sc.contains(&s.key)))
            .cloned()
            .collect();
        let key = |out: &Path| output_key(&req.output_root, out);
        let ctx = base_ctx(
            req,
            target,
            registry,
            preset_id,
            p.user_config.as_ref(),
            p.project_config.as_ref(),
            file_scopes,
        );

        // On every set, so a choice for a set outside the scope is not taken for stale.
        let (explicit, stale_shaders) = explicit_shaders(req, &p.sets);
        for set in &mut sets {
            let complex = explicit.get(&set.key) == Some(&Shader::ComplexMaterial);
            let skin_fuzz = skin_fuzz(&ctx, set, files);
            predict_suffixes(set, skin_fuzz, target, complex, files, typed, hand);
        }
        let set_of: BTreeMap<usize, &str> = sets
            .iter()
            .flat_map(|s| s.members.iter().map(|&i| (i, s.key.as_str())))
            .collect();
        let in_scope =
            |set: Option<&String>| scope.is_none_or(|sc| set.is_some_and(|k| sc.contains(k)));
        let mut skipped: Vec<SkippedFile> = dup_idx
            .iter()
            .filter(|&i| scope.is_none() || set_of.contains_key(i))
            .map(|&i| SkippedFile {
                path: files[i].path.clone(),
                output: files[i].output.clone(),
                set: set_of.get(&i).map(|k| k.to_string()),
                code: Some(Code::InDuplicate),
                reason: "two inputs map to one output; exclude one".into(),
            })
            .collect();
        skipped.extend(
            p.gen_replaced
                .iter()
                .filter(|s| in_scope(s.set.as_ref()))
                .cloned(),
        );
        let mut self_overwrite: Vec<FailedFile> = vec![];
        let mut work: Vec<(&TextureSet, &InputFile)> = vec![];
        for set in &sets {
            for &i in &set.members {
                if dup_idx.contains(&i) {
                    continue;
                }
                if files[i].kind == InputKind::Dds {
                    if !reencodes(&ctx, &files[i]) {
                        skipped.push(SkippedFile {
                            path: files[i].path.clone(),
                            output: files[i].output.clone(),
                            set: Some(set.key.clone()),
                            code: Some(Code::InDdsSkipped),
                            reason: "DDS inputs are not re-encoded by default".into(),
                        });
                        continue;
                    }
                    // DECISIONS S6: never write over the source DDS (output root == input folder).
                    if crate::dds::same_file(&files[i].path, &files[i].output) {
                        self_overwrite.push(FailedFile {
                            path: files[i].path.clone(),
                            output: files[i].output.clone(),
                            stage: Stage::Load,
                            error: "re-encoding would overwrite the source DDS: choose another output folder".into(),
                            set: None,
                        });
                        continue;
                    }
                }
                work.push((set, &files[i]));
            }
        }

        // Load and Analyse in parallel under the same memory admission as `run` (spec §5.9, Codex-B I1): the calling
        // thread admits each file's estimated bytes before spawning it, and the bytes are released as soon as its
        // analysis is done (or it unwinds). Only the `Analysis` is kept.
        // Shared by pass 1 and the 6c re-resolve; each call site builds `FileFacts` from its own split (it borrows it).
        let split_for = |(set, f): (&TextureSet, &InputFile)| -> SuffixSplit {
            let stem = f
                .rel
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            member_split(&stem, set, registry.alias_table())
        };
        let gras = &req.gras_diffuses;
        // A sub-type a `[files]` entry or `--set` gave the file picks its hints; else the name's suffix does.
        let hints_of = |ctx: &ResolveCtx, f: &InputFile, split: &SuffixSplit| {
            match crate::policy::subtype_override(ctx, f) {
                Some(st) => crate::checks::hints_for_subtype(st, cache.checks),
                None => crate::checks::hints_for(split.suffix.as_deref(), cache.checks),
            }
        };
        let resolve_one =
            |&(set, f): &(&TextureSet, &InputFile),
             admitted: Admitted<'_>,
             stage: &Cell<Stage>,
             source: Option<SourceStamp>,
             hints: crate::analyse::AnalyseHints|
             -> Result<(ResolvedJob, crate::analyse::Analysis, SourceInfo), LoadError> {
                let enter = |st: Stage| {
                    stage.set(st);
                    #[cfg(feature = "test-hooks")]
                    panic_while_planning_for_test(&f.path, st);
                };
                enter(Stage::Load);
                let split = split_for((set, f));
                // `source` and `hints` come from the spawner: one stat and one hint lookup per file.
                let (a, info) = match source.and_then(|s| cache.file(&f.path, s, hints)) {
                    Some(hit) => {
                        drop(admitted);
                        hit
                    }
                    None => {
                        let (img, info, meta) = load_image_meta(&f.path)?;
                        cache.count_loads(1);
                        enter(Stage::Analyse);
                        let mut a = analyse_with(&img, &info, hints);
                        a.dds_dxgi = meta.dds_dxgi;
                        a.dds_cube = meta.dds_cube;
                        if let Some(s) = source {
                            cache.put_file(
                                &f.path,
                                s,
                                hints,
                                a.clone(),
                                info,
                                cache.make_thumb(&img, &info),
                            );
                        }
                        drop(img);
                        drop(admitted);
                        (a, info)
                    }
                };
                enter(Stage::Resolve);
                let facts = FileFacts {
                    input: f,
                    suffix: split.suffix.as_deref(),
                    alias: split.alias.as_deref(),
                    set,
                    analysis: &a,
                    info: &info,
                };
                let gras_diffuse = gras.contains(&crate::shader::gras_key(&f.game_path, &f.output));
                let mut job = resolve_file_with(&ctx, &facts, gras_diffuse);
                job.provenance.insert("target".into(), target_src.clone());
                job.source = source;
                Ok((job, a, info))
            };
        let pool = thread_pool(req.jobs)?;
        let budget = Budget::new(budget_limit(req), pool.current_num_threads());
        // One slot per work item, so results keep the input order. A slot left empty is a file that was cancelled
        // before its Load started.
        let slots: Vec<Mutex<Option<_>>> = work.iter().map(|_| Mutex::new(None)).collect();
        // A partial plans on the skeleton's request, whose counters are the last full plan's.
        let progress = req.plan_progress.clone().filter(|_| scope.is_none());
        if let Some(p) = &progress {
            p.total.store(work.len(), Ordering::Relaxed);
        }
        pool.in_place_scope(|scope| {
            for (item, slot) in work.iter().zip(&slots) {
                #[cfg(feature = "test-hooks")]
                cancel_on_load_for_test(&item.1.path, cancel);
                if cancel.is_cancelled() {
                    continue;
                }
                // DECISIONS AV4: a cache hit loads nothing, so it is admitted at 0 without reading its header.
                // shortcut: a file changed between this check and its Load loads unadmitted; the next plan admits it.
                let path = &item.1.path;
                let hints = hints_of(&ctx, item.1, &split_for(*item));
                // Taken before the read, so a change made while it is loaded is seen as a change too.
                let source = SourceStamp::of(path);
                let cached = source.is_some_and(|s| cache.file(path, s, hints).is_some());
                let est = if cached { 0 } else { estimated_bytes(path) };
                budget.acquire(est);
                let admitted = Admitted(&budget, est);
                let resolve_one = &resolve_one;
                let progress = &progress;
                scope.spawn(move |_| {
                    if !cancel.is_cancelled() {
                        // Spec §5.9, §8 (final review I1): a panic fails this file alone, at the stage it had
                        // entered, as in `run_one`. The admitted bytes are released while it unwinds.
                        let stage = Cell::new(Stage::Load);
                        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            resolve_one(item, admitted, &stage, source, hints)
                        }));
                        *lock(slot) = Some(match r {
                            Ok(r) => r.map_err(PlanFailure::Load),
                            Err(panic) => Err(PlanFailure::Panic(
                                stage.get(),
                                format!("internal error: {}", panic_message(&*panic)),
                            )),
                        });
                        if let Some(p) = progress {
                            p.done.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                });
            }
        });

        // Pass-1 jobs, with the work index, analysis and source info each one needs to be re-resolved (6c).
        let mut pass1_jobs: Vec<ResolvedJob> = vec![];
        let mut pass1 = vec![];
        let mut failed = vec![];
        failed.extend(self_overwrite);
        let mut cancelled = vec![];
        for (wi, (item, slot)) in work.iter().zip(slots).enumerate() {
            let Some(r) = slot.into_inner().unwrap_or_else(|e| e.into_inner()) else {
                cancelled.push(item.1.path.clone());
                continue;
            };
            let path = item.1.path.clone();
            let output = item.1.output.clone();
            match r {
                Ok((job, a, info)) => {
                    pass1_jobs.push(job);
                    pass1.push((wi, a, info));
                }
                Err(PlanFailure::Load(e)) => {
                    rep.emit_job(
                        Some(&key(&output)),
                        None,
                        Some(&path),
                        EventKind::Warning(Warning::new(load_code(&e), e.to_string())),
                    );
                    failed.push(FailedFile {
                        path,
                        output,
                        stage: Stage::Load,
                        error: e.to_string(),
                        set: None,
                    });
                }
                Err(PlanFailure::Panic(stage, error)) => failed.push(FailedFile {
                    path,
                    output,
                    stage,
                    error,
                    set: None,
                }),
                Err(PlanFailure::Pack(_)) => unreachable!("pass 1 never assembles"),
            }
        }
        // 6a. Explicit shader choices: the GUI map, then `--set set:<id>.shader` (the command line wins).
        for id in stale_shaders.iter().filter(|_| scope.is_none()) {
            rep.emit(
                None,
                None,
                EventKind::Warning(
                    Warning::new(
                        Code::SetEditStale,
                        format!("shader choice skipped: no set '{id}'"),
                    )
                    .with_data(serde_json::json!({ "set": id })),
                ),
            );
        }
        // 6b. One pick per set, from the pass-1 sub-types; later resolves see it through `ctx.shaders`.
        let index = crate::shader::JobIndex::new(&pass1_jobs);
        let txst: BTreeSet<String> = req
            .cs_txst_sets
            .iter()
            .map(|t| crate::sets::fold_id(t))
            .collect();
        let mut picks: BTreeMap<String, ShaderPick> = sets
            .iter()
            .map(|s| {
                let pick = crate::shader::pick_for_set(
                    &ctx,
                    s,
                    files,
                    &index,
                    explicit.get(&s.key).copied(),
                    &req.mesh_shaders,
                    &req.gras_all_diffuses,
                    txst.contains(&crate::sets::fold_id(&crate::sets::set_id(
                        &req.output_root,
                        &s.key,
                    ))),
                );
                (s.key.clone(), pick)
            })
            .collect();
        // Note GI27: a set whose `_noise` or `_detail` the Vanilla derive keeps is typed as its PBR phase typed it,
        // so the 6c pass below re-types them (a detail map into a detail normal, a medium `_noise`).
        let kept_maps: BTreeSet<&Path> = (req.derive.iter())
            .flat_map(|d| d.sets.iter().filter(|s| d.keeps_landscape(s)))
            .flat_map(|s| s.kept_maps.iter().map(PathBuf::as_path))
            .collect();
        for s in &sets {
            if s.members
                .iter()
                .any(|&i| kept_maps.contains(files[i].path.as_path()))
            {
                picks.insert(
                    s.key.clone(),
                    ShaderPick {
                        shader: crate::shader::Shader::PbrLandscape,
                        provenance: crate::shader::ShaderOrigin::Derived,
                        wanted: None,
                    },
                );
            }
        }
        // The pass-1 context (no picks) is kept, so the derive step can rebuild it after adding its picks.
        let base_ctx = ctx;
        let ctx = ResolveCtx {
            shaders: &picks,
            ..base_ctx
        };
        // Note GW1: a Landscape picked now (from a mesh) makes its set a detail host too, so two detail maps in it
        // are IN-DUPLICATE like any other host's. `prepare` could not see the pick; its groups are not repeated.
        let late: Vec<usize> = crate::group::duplicate_details(
            &sets,
            files,
            registry.alias_table(),
            |s| {
                picks
                    .get(&s.key)
                    .is_some_and(|p| matches!(p.shader, Shader::Landscape | Shader::PbrLandscape))
            },
            |f| f.kind != InputKind::Dds || reencodes(&ctx, f),
        )
        .into_iter()
        .flatten()
        .filter(|i| !dup_idx.contains(i))
        .collect();
        skipped.extend(late.iter().map(|&i| SkippedFile {
            path: files[i].path.clone(),
            output: files[i].output.clone(),
            set: set_of.get(&i).map(|k| k.to_string()),
            code: Some(Code::InDuplicate),
            reason: "two inputs map to one output; exclude one".into(),
        }));
        let late_paths: BTreeSet<&Path> = late.iter().map(|&i| files[i].path.as_path()).collect();
        // Note CO103: images whose B is empty, for a generated AO beside a pre-packed `_rmaos` (pack planning).
        let flat_b: BTreeSet<PathBuf> = pass1
            .iter()
            .filter(|(_, a, _)| !pack::b_has_content(&a.channels[2]))
            .map(|(wi, _, _)| work[*wi].1.path.clone())
            .collect();
        // Read by `plan_cubes` (DECISIONS DS8): each input's loaded size and cube-DDS flag.
        let cube_facts: crate::cube::plan::InputFacts = pass1
            .iter()
            .map(|(wi, a, _)| (work[*wi].1.path.clone(), (a.width, a.height, a.dds_cube)))
            .collect();
        // Read by `plan_fills` (Note DY, Ruling 6): each input's alpha verdict on the original source, and its size.
        let alpha_facts: BTreeMap<PathBuf, crate::helper::plan::AlphaFacts> = pass1
            .iter()
            .map(|(wi, a, _)| {
                let empty = matches!(a.alpha, AlphaContent::None | AlphaContent::Constant255)
                    || a.channels[3].min == a.channels[3].max;
                let facts = crate::helper::plan::AlphaFacts {
                    empty,
                    size: (a.width, a.height),
                };
                (work[*wi].1.path.clone(), facts)
            })
            .collect();
        // Note HC1: images whose alpha is flat, for a specular beside a pre-packed `_rmaos` (pack planning).
        let flat_a: BTreeSet<PathBuf> = alpha_facts
            .iter()
            .filter(|(_, f)| f.empty)
            .map(|(p, _)| p.clone())
            .collect();
        // The load fact a filled target's re-analysis cannot see (it loads pixels only): the DDS format.
        let load_meta: BTreeMap<PathBuf, Option<u32>> = pass1
            .iter()
            .map(|(wi, a, _)| (work[*wi].1.path.clone(), a.dds_dxgi))
            .collect();
        // Fuzz-1a: a set that reads its fuzz map as the PBR fuzz texture writes an `_f`, so its other members are
        // resolved below (and in the fill step, and checked in `map_members`) against a copy whose suffixes hold `f`
        // (`member_set`); `set_resolved` reads a copy with `f`, so it says `pbr`. The set itself never gains it.
        let f_sets: BTreeSet<String> = sets
            .iter()
            .filter(|s| pbr_read_fuzz(&ctx, s, files))
            .map(|s| s.key.clone())
            .collect();
        // 6c. Re-resolve the members of a set whose shader changes member defaults, of a chosen-skin set (DV1) and of a
        // set read with `f` (Fuzz-1a), and every cs fuzz map (its reading follows the pick).
        let mut jobs = vec![];
        for (mut old, (wi, a, info)) in pass1_jobs.into_iter().zip(pass1) {
            // PBR grass design §4.4: the albedo's corner texel against CS's grass shadow-pass detector, judged on the
            // job's final sub-type (a re-resolve can turn a GRAS complex_grass guess into an albedo).
            let grass_note = |j: &ResolvedJob| {
                picks.get(&j.set_key).and_then(|p| {
                    crate::shader::pbr_grass_albedo_note(p, j.profile.subtype, a.corner_err)
                })
            };
            // An explicit set shader also re-resolves a GRAS diffuse: it suppresses the auto-detect (`suggest_grass`).
            let is_gras = gras.contains(&crate::shader::gras_key(
                &work[wi].1.game_path,
                &work[wi].1.output,
            ));
            let (set, f) = work[wi];
            if late_paths.contains(f.path.as_path()) {
                continue;
            }
            // Note GI (R5), GW1: a PBR Landscape detail map or `_noise` is typed again (detail normal, noise).
            let landscape_retype = split_for((set, f))
                .suffix
                .as_deref()
                .is_some_and(crate::group::is_detail_suffix)
                || old
                    .suggestions
                    .first()
                    .is_some_and(|s| s.subtype == Subtype::ProjectedNoise);
            // Fuzz-2, Fuzz-3: a fuzz map is read again with the pick (`fuzz_reading`). A typed or hand-set one needs no
            // test of its own here: `resolve_file_with` asks `fuzz_reading` for neither.
            let fuzz_map = ctx.target == Target::Cs && crate::group::is_fuzz_map_image(f);
            // Fuzz-1a: the other members of a set read with `f` are resolved against the copy.
            let read = member_set(set, f, &f_sets);
            let with_f = matches!(read, std::borrow::Cow::Owned(_));
            // Note DV1: a chosen Advanced Skin makes the set skin (`chosen_recipe`), so its members are typed again
            // (not while its diffuse is pinned, Note DW1).
            if !picks.get(&old.set_key).is_some_and(|p| {
                p.shader.has_member_defaults()
                    || (p.provenance == crate::shader::ShaderOrigin::Set && is_gras)
                    || (p.shader == crate::shader::Shader::PbrLandscape && landscape_retype)
            }) && !pack::plan::chosen_skin(&ctx, set)
                && !fuzz_map
                && !with_f
            {
                old.warnings.extend(grass_note(&old));
                jobs.push(old);
                continue;
            }
            let set: &TextureSet = &read;
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let split = split_for((set, f));
                let facts = FileFacts {
                    input: f,
                    suffix: split.suffix.as_deref(),
                    alias: split.alias.as_deref(),
                    set,
                    analysis: &a,
                    info: &info,
                };
                resolve_file_with(
                    &ctx,
                    &facts,
                    gras.contains(&crate::shader::gras_key(&f.game_path, &f.output)),
                )
            }));
            match r {
                Ok(mut job) => {
                    job.source = old.source;
                    job.warnings.extend(grass_note(&job));
                    job.provenance.insert("target".into(), target_src.clone());
                    jobs.push(job);
                }
                Err(p) => failed.push(FailedFile {
                    path: f.path.clone(),
                    output: f.output.clone(),
                    stage: Stage::Resolve,
                    error: format!("internal error: {}", panic_message(&*p)),
                    set: None,
                }),
            }
        }
        // Spec §6.4 (M2.5): channel packing. Pass 1 decides the recipes; pass 2 assembles, analyses and resolves
        // each under the same memory admission and panic isolation as the files above.
        let failed_paths: Vec<PathBuf> = failed.iter().map(|f| f.path.clone()).collect();
        // Note CN26 (O6): the raw heights whose landscape `_p` a Vanilla derive keeps.
        let derive_heights: BTreeSet<PathBuf> = req
            .derive
            .iter()
            .filter(|d| d.mode == crate::derive::DeriveMode::Vanilla)
            .flat_map(|d| &d.sets)
            .filter(|s| s.shader == crate::shader::Shader::PbrLandscape)
            .filter_map(|s| match &s.height {
                Some(pack::ChannelSource::Source {
                    path,
                    role: crate::config::RawMap::Height,
                    ..
                }) => Some(path.clone()),
                _ => None,
            })
            .collect();
        // DECISIONS DS8: six face files become one cube job, and one-file cubes get their layout.
        crate::cube::plan::plan_cubes(&cube_facts, &mut jobs, &mut skipped);
        // Note DY: helper alpha fills, decided before packing so a helper an alpha fill uses is not reported unused.
        let fills = crate::helper::plan::plan_fills(&ctx, &sets, files, &mut jobs, &alpha_facts);
        let mut packed = pack::plan::plan_packing(
            &ctx,
            &sets,
            files,
            &mut jobs,
            &failed_paths,
            &cancelled,
            &mut skipped,
            &p.packed_outputs,
            &derive_heights,
            &flat_b,
            &flat_a,
            &fills,
            &req.output_root,
        );
        // As step 3b drops its losers, so later steps never read a replaced generated AO.
        for s in &mut sets {
            s.members
                .retain(|&i| !packed.gen_replaced.contains(&files[i].path));
            s.shares
                .retain(|&i| !packed.gen_replaced.contains(&files[i].path));
        }
        // Pass 1 may add a recipe job it fails at once (sources of different sizes); like every job it records the
        // target's origin (`BatchRequest::target`, ruling W1).
        for j in &mut jobs {
            j.provenance
                .entry("target".into())
                .or_insert_with(|| target_src.clone());
        }
        // Pass 2 and the derive step share `admit_all`; the pass-1 block above keeps its own loop (it returns an
        // analysis and source info with each job, and counts progress).
        let pack_results = admit_all(
            &pool,
            &budget,
            cancel,
            &packed.candidates,
            |c| {
                // One decoded source beside the output (the first, counted at the common size when it is enlarged).
                grown_bytes(c.job.sources.iter().map(|s| s.path.as_path()))
                    .first()
                    .copied()
                    .unwrap_or(0)
            },
            |c| &c.virtual_input.output,
            |c, admitted, stage| {
                // The key includes every source's stamp, so an edited source misses the cache. A recipe
                // without a key (a source with no stamp) is assembled every time and never stored.
                let key = pack_cache_key(&c.job);
                let hints = crate::checks::hints_for(Some(c.job.kind.suffix()), cache.checks);
                let entry = match key.as_deref().and_then(|k| cache.pack(k, hints)) {
                    Some(e) => {
                        drop(admitted);
                        e
                    }
                    None => {
                        // `check_sizes` (in `assemble`) enforces MAX_DIM per source before the canvas
                        // is allocated; its `TooLarge` maps to DIM-TOO-LARGE like `load_image`'s.
                        let asm = pack::assemble::assemble(&c.job, c.job.gray)
                            .map_err(PlanFailure::of_pack)?;
                        cache.count_loads(c.job.sources.len() as u64);
                        stage.set(Stage::Analyse);
                        let e = PackEntry {
                            analysis: analyse_with(&asm.image, &asm.info, hints),
                            info: asm.info,
                            non_gray: asm.non_gray,
                            thumb: cache.make_thumb(&asm.image, &asm.info),
                            hints,
                        };
                        drop(asm.image);
                        drop(admitted);
                        if let Some(key) = key {
                            cache.put_pack(key, e.clone());
                        }
                        e
                    }
                };
                stage.set(Stage::Resolve);
                let set = sets
                    .iter()
                    .find(|s| s.key == c.set_key)
                    .expect("a candidate's set is planned");
                Ok(pack::plan::finish_recipe(
                    &ctx,
                    c,
                    set,
                    &entry.analysis,
                    &entry.info,
                    &entry.non_gray,
                ))
            },
        );
        // A pass-2 or derive failure as (stage, error).
        let failure = |output: &Path, f: PlanFailure| match f {
            PlanFailure::Load(e) => {
                // The same §10 warning as a pass-1 load failure, so a JSON consumer sees the code; it names the
                // source that failed, the `file_failed` the caller records names the recipe's input.
                let (LoadError::TooLarge { path: bad, .. } | LoadError::Decode { path: bad, .. }) =
                    &e;
                rep.emit_job(
                    Some(&key(output)),
                    None,
                    Some(bad),
                    EventKind::Warning(Warning::new(load_code(&e), e.to_string())),
                );
                (Stage::Load, e.to_string())
            }
            PlanFailure::Pack(e) => (Stage::Load, e),
            PlanFailure::Panic(stage, e) => (stage, e),
        };
        for (c, r) in packed.candidates.iter().zip(pack_results) {
            let path = c.job.sources[0].path.clone();
            let output = c.virtual_input.output.clone();
            let Some(r) = r else {
                // Cancelled before its sources were loaded: the recipe has no job (Note M11, ruling W7).
                pack::plan::fail_report(&mut packed.reports, c, "cancelled".into());
                packed.lost.extend(
                    c.job
                        .sources
                        .iter()
                        .map(|s| (c.set_key.clone(), c.kind, s.path.clone(), PathBuf::new())),
                );
                cancelled.push(path);
                continue;
            };
            match r {
                Ok(mut job) => {
                    // As `resolve_one` does for every other job; `finish_recipe` cannot (it has no `target_src`).
                    job.provenance.insert("target".into(), target_src.clone());
                    jobs.push(job);
                }
                Err(f) => {
                    let (stage, error) = failure(&output, f);
                    pack::plan::fail_report(&mut packed.reports, c, error.clone());
                    packed.lost.extend(
                        c.job
                            .sources
                            .iter()
                            .map(|s| (c.set_key.clone(), c.kind, s.path.clone(), PathBuf::new())),
                    );
                    failed.push(FailedFile {
                        path,
                        output,
                        stage,
                        error,
                        set: None,
                    });
                }
            }
        }
        // Backlog T17: the sources of failed or cancelled recipes end as not converted, once every job is known.
        let lost: Vec<_> = packed
            .lost
            .iter()
            .filter_map(|(s, k, p, _)| {
                let f = files.iter().find(|f| f.path == *p)?;
                Some((s.clone(), *k, p.clone(), f.output.clone()))
            })
            .collect();
        pack::plan::end_lost_sources(&lost, &jobs, &failed, &cancelled, &mut skipped);
        // Note DY: each filled target is loaded, filled and analysed again, then re-resolved as 6c does, so alpha
        // policy, format, coverage and cutout all see the filled image. It runs before the derive step, so a target
        // a derive replaces loses its fill with it (Ruling 15). The filled analysis is cached in memory by both files'
        // stamps and the fill (Note FM31), so a re-plan of unchanged files loads nothing here.
        let fill_items: Vec<_> = fills
            .candidates
            .iter()
            .filter_map(|c| {
                let ji = jobs.iter().position(|j| {
                    j.input == c.target
                        && j.pack.is_none()
                        && j.derive.is_none()
                        && j.cube.is_none()
                })?;
                let set = sets.iter().find(|s| s.key == c.set_key)?;
                let f = files.iter().find(|f| f.path == c.target)?;
                Some((c, ji, set, f))
            })
            .collect();
        // Review 4 I1: every consumed helper with the output it fills (a size-mismatch error job carries its `fill`),
        // so one whose target job is later gone (failed, cancelled, derived away) still ends with that output.
        let fill_owner: Vec<(PathBuf, PathBuf, String)> = fill_items
            .iter()
            .map(|&(c, ji, ..)| (&c.fill, &jobs[ji]))
            .chain(
                jobs.iter()
                    .filter_map(|j| j.fill.as_ref().map(|fl| (fl, j))),
            )
            .map(|(fl, j)| (fl.source.path.clone(), j.output.clone(), j.set_key.clone()))
            .collect();
        // Test hook: `SDC_TEST_CANCEL_ON=<target output name>` cancels the batch as the fill step starts (pass 1 is
        // named by inputs, so only this step matches an output name).
        #[cfg(feature = "test-hooks")]
        for &(_, ji, ..) in &fill_items {
            cancel_on_load_for_test(&jobs[ji].output, cancel);
        }
        let fill_results = admit_all(
            &pool,
            &budget,
            cancel,
            &fill_items,
            |(c, ..)| estimated_bytes(&c.target) + estimated_bytes(&c.fill.source.path),
            |(_, ji, ..)| jobs[*ji].output.as_path(),
            |&(c, ji, set, f), admitted, stage| {
                let old = &jobs[ji];
                // Fuzz-1a: against the set 6c resolved this member with.
                let read = member_set(set, f, &f_sets);
                let set: &TextureSet = &read;
                let split = split_for((set, f));
                let gras_diffuse = gras.contains(&crate::shader::gras_key(&f.game_path, &f.output));
                // Review 4 I2: the resolved profile may change `alpha.exr_straight` (a rule on the filled alpha), so the
                // image is loaded again under the final setting: the analysed image is the one the run writes.
                // shortcut: a second change is not chased (the filled alpha, which rules match, is the same under both
                // settings); loop until stable if a rule ever reads RGB.
                let mut straight = old.profile.alpha.exr_straight;
                let mut tries = 0;
                let (mut job, a) = loop {
                    let hints = crate::checks::hints_for(split.suffix.as_deref(), cache.checks);
                    let key = fill_cache_key(&c.target, old.source, &c.fill, straight);
                    let (a, info) = match key.as_deref().and_then(|k| cache.fill(k, hints)) {
                        Some(hit) => hit,
                        None => {
                            stage.set(Stage::Load);
                            let (mut img, mut info) =
                                crate::load::load_image_as(&c.target, straight)
                                    .map_err(PlanFailure::Load)?;
                            crate::helper::apply_fill(&mut img, &mut info, &c.fill)
                                .map_err(PlanFailure::Pack)?;
                            cache.count_loads(2);
                            stage.set(Stage::Analyse);
                            let mut a = analyse_with(&img, &info, hints);
                            if let Some(&dxgi) = load_meta.get(&c.target) {
                                a.dds_dxgi = dxgi;
                            }
                            drop(img);
                            if let Some(k) = key {
                                cache.put_fill(k, hints, a.clone(), info);
                            }
                            (a, info)
                        }
                    };
                    stage.set(Stage::Resolve);
                    let facts = FileFacts {
                        input: f,
                        suffix: split.suffix.as_deref(),
                        alias: split.alias.as_deref(),
                        set,
                        analysis: &a,
                        info: &info,
                    };
                    let job = resolve_file_with(&ctx, &facts, gras_diffuse);
                    if job.profile.alpha.exr_straight == straight || tries == 1 {
                        break (job, a);
                    }
                    straight = job.profile.alpha.exr_straight;
                    tries += 1;
                };
                drop(admitted);
                job.source = old.source;
                job.warnings.extend(c.resized.clone());
                job.warnings.extend(picks.get(&job.set_key).and_then(|p| {
                    crate::shader::pbr_grass_albedo_note(p, job.profile.subtype, a.corner_err)
                }));
                job.provenance.insert("target".into(), target_src.clone());
                let read = pack::ChannelSource::Source {
                    path: c.fill.source.path.clone(),
                    role: c.fill.source.role,
                    invert: c.fill.invert,
                    channel: c.fill.read.map(|r| r.channel()),
                };
                if let Some(p) = read.provenance(GraySource::R) {
                    job.provenance.insert("fill.a".into(), p);
                }
                job.fill = Some(c.fill.clone());
                Ok(job)
            },
        );
        // A target that failed or was cancelled here has no job, as a pass-2 recipe.
        let mut gone = BTreeSet::new();
        for (&(c, ji, ..), r) in fill_items.iter().zip(fill_results) {
            match r {
                Some(Ok(job)) => jobs[ji] = job,
                Some(Err(e)) => {
                    let output = jobs[ji].output.clone();
                    let (stage, error) = failure(&output, e);
                    failed.push(FailedFile {
                        path: c.target.clone(),
                        output,
                        stage,
                        error,
                        set: None,
                    });
                    gone.insert(ji);
                }
                None => {
                    cancelled.push(c.target.clone());
                    gone.insert(ji);
                }
            }
        }
        let mut i = 0;
        jobs.retain(|_| {
            i += 1;
            !gone.contains(&(i - 1))
        });
        // Track A (DECISIONS Note CN): the derive step replaces each PBR set's jobs with derived jobs, after packing
        // (a packed member's job goes too) and before the shader assignment, so a derived set carries its vanilla
        // shader (TA29). Without `derive` nothing below runs (C6).
        let mut derive_own = BTreeSet::new();
        let derived = req.derive.as_ref().map(|d| {
            crate::derive::plan::plan_derived(
                &ctx,
                d,
                &req.mesh_shaders,
                files,
                &mut jobs,
                &mut skipped,
                &failed,
            )
        });
        if let Some(dp) = derived {
            // The derived jobs resolve under their derived pick.
            picks.extend(dp.picks);
            let ctx = ResolveCtx {
                shaders: &picks,
                ..base_ctx
            };
            // Error jobs (sources of different sizes, TA12 conflicts) record the target, as pass 1's do.
            for j in &mut jobs {
                j.provenance
                    .entry("target".into())
                    .or_insert_with(|| target_src.clone());
            }
            // shortcut: P25: a derived image's analysis is not cached (never under its albedo's path, which would
            // replace the albedo's own), so each plan assembles it again; key it on `identity_sha256` if
            // re-planning an Also export gets slow.
            let results = admit_all(
                &pool,
                &budget,
                cancel,
                &dp.candidates,
                |c| {
                    grown_bytes(c.job.sources.iter().map(|s| s.path.as_path()))
                        .iter()
                        .sum()
                },
                |c| &c.input.output,
                |c, admitted, stage| {
                    let (asm, mean_rough) = crate::derive::assemble::assemble_derived_rough(&c.job)
                        .map_err(PlanFailure::of_pack)?;
                    cache.count_loads(c.job.sources.len() as u64);
                    stage.set(Stage::Analyse);
                    let analysis = analyse_with(
                        &asm.image,
                        &asm.info,
                        crate::checks::hints_for(c.job.kind.suffix(), cache.checks),
                    );
                    drop(asm.image);
                    drop(admitted);
                    stage.set(Stage::Resolve);
                    #[cfg(feature = "test-hooks")]
                    panic_while_planning_for_test(&c.input.output, Stage::Resolve);
                    let set = sets
                        .iter()
                        .find(|s| s.key == c.set_key)
                        .expect("a candidate's set is planned");
                    Ok(crate::derive::plan::finish_derived(
                        &ctx, c, set, &analysis, &asm.info, mean_rough,
                    ))
                },
            );
            let failed_before = failed.len();
            for (c, r) in dp.candidates.iter().zip(results) {
                let path = c.input.path.clone();
                match r {
                    None => cancelled.push(path),
                    Some(Ok(mut job)) => {
                        job.provenance.insert("target".into(), target_src.clone());
                        jobs.push(job);
                    }
                    Some(Err(f)) => {
                        let (stage, error) = failure(&c.input.output, f);
                        failed.push(FailedFile {
                            path,
                            output: c.input.output.clone(),
                            stage,
                            error,
                            set: None,
                        });
                    }
                }
            }
            if let Some(d) = &req.derive {
                // A derived candidate that failed here ends its own-key source with its `file_failed`.
                let written: BTreeSet<String> = jobs
                    .iter()
                    .map(|j| key(&j.output))
                    .chain(failed[failed_before..].iter().map(|f| key(&f.output)))
                    .collect();
                let members: BTreeSet<&Path> = d
                    .sets
                    .iter()
                    .flat_map(|s| &s.members)
                    .chain(&d.drop)
                    .map(PathBuf::as_path)
                    .collect();
                derive_own = files
                    .iter()
                    .filter(|f| {
                        members.contains(f.path.as_path()) && written.contains(&key(&f.output))
                    })
                    .map(|f| f.path.clone())
                    .collect();
                crate::derive::plan::report_unread(
                    d,
                    &dp.member_sets,
                    files,
                    &jobs,
                    &mut skipped,
                    &failed[..failed_before],
                    &cancelled,
                    &derive_own,
                );
            }
        }
        // Note FM44: a `resize_relative_to` reads its set's final members (after packing and derive), before the bakes
        // and `file_planned` read the sizes. A cube's size comes from pass 1's facts; any other map's
        // from `input_dims`, which reads the file's header again (a few header reads per relative job).
        let known = |p: &Path| {
            cube_facts
                .get(p)
                .map(|&(w, h, _)| (w, h))
                .or_else(|| input_dims(p))
        };
        crate::resize::plan_references(&mut jobs, |j| {
            let own = known(&j.input);
            match &j.cube {
                Some(c) => own.map(|d| {
                    let s = crate::cube::layout::source_cell(c.layout, d);
                    (s, s)
                }),
                None => job_dims_by(j, own, &known),
            }
        });
        let ctx = ResolveCtx {
            shaders: &picks,
            ..base_ctx
        };
        // Spec §6.2–6.3 (M4): the set-level variance bake, after packing so a packed roughness member counts, and
        // before `set_resolved` and `file_planned` so both carry the decision.
        let shared: BTreeSet<String> = req
            .shared_roughness
            .iter()
            .map(|id| crate::sets::set_key(&req.output_root, id))
            .collect();
        let borrowed: BTreeMap<String, String> = req
            .borrowed_normal
            .iter()
            .map(|(b, o)| {
                (
                    crate::sets::set_key(&req.output_root, b),
                    crate::sets::set_key(&req.output_root, o),
                )
            })
            .collect();
        for j in &mut jobs {
            j.shader = picks.get(&j.set_key).cloned();
        }
        let helpers = crate::helper::report::reports(&ctx, &sets, files, &jobs, &fills);
        let bake_cx = crate::bake::plan::BakeCtx {
            target,
            sets: &sets,
            files,
            aliases: registry.alias_table(),
            shared: &shared,
            groups: &req.shared_roughness_groups,
            output_root: &req.output_root,
            borrowed: &borrowed,
        };
        let mut baking = crate::bake::plan::plan_bakes(&bake_cx, &mut jobs);
        // Note CL40: the opt-in complex `_m` gloss bake, any target.
        baking.extend(crate::bake::plan::plan_gloss_bakes(&bake_cx, &mut jobs));
        let peak_admitted_bytes = budget.peak.load(Ordering::Relaxed);
        // DECISIONS N2: a [files] key that names nothing planned is reported once (it stays in the file).
        // Batch-wide: a partial keeps the full plan's verdict (a value edit never changes which keys match).
        let mut rels: Vec<String> = vec![];
        if let (Some(root), None) = (&file_scopes.root, scope) {
            let rel_of_path = |p: &Path| -> Option<String> {
                let f = InputFile {
                    path: p.to_path_buf(),
                    rel: PathBuf::from(p.file_name()?),
                    game_path: String::new(),
                    output: PathBuf::new(),
                    kind: InputKind::Image,
                    real_rel: None,
                };
                project_rel(root, &f)
            };
            for j in &jobs {
                // A packed output, or a derived `_m` (TA15, Note CN19): the folder of its first source plus its
                // stem (e.g. `rock01_rmaos`; a packed output's own name, Note HS2, whatever output path it has),
                // compared with the extension-less key. A packed output's raw sources count too.
                let stem = match (&j.pack, &j.derive) {
                    (Some(pk), _) => Some(crate::pack::plan::files_stem(j, pk.kind)),
                    (None, Some(dj)) => dj
                        .kind
                        .suffix()
                        .filter(|s| *s == "m")
                        .map(|s| crate::pack::plan::virtual_stem_as(&j.input, s)),
                    (None, None) => None,
                };
                match stem {
                    Some(stem) => {
                        if let Some(r) = rel_of_path(&j.input) {
                            let dir = r
                                .rsplit_once('/')
                                .map_or(String::new(), |(d, _)| format!("{d}/"));
                            rels.push(format!("{dir}{stem}"));
                        }
                    }
                    None => rels.extend(rel_of_path(&j.input)),
                }
                if let Some(pk) = &j.pack {
                    rels.extend(pk.sources.iter().filter_map(|s| rel_of_path(&s.path)));
                }
                // Backlog T18: a cube's faces, and its output stem (folder of the first face + the output's name).
                if let Some(cj) = &j.cube {
                    rels.extend(cj.sources.iter().filter_map(|s| rel_of_path(&s.path)));
                    if let (Some(r), Some(stem)) = (rel_of_path(&j.input), j.output.file_stem()) {
                        let dir = r.rsplit_once('/').map_or("", |(d, _)| d);
                        rels.push(
                            format!("{dir}/{}", stem.to_string_lossy())
                                .trim_start_matches('/')
                                .to_string(),
                        );
                    }
                }
                rels.extend(j.fill.iter().filter_map(|f| rel_of_path(&f.source.path)));
            }
            rels.extend(skipped.iter().filter_map(|s| rel_of_path(&s.path)));
            // Note DY: a helper consumed by a fill or a recipe, or one that fills nothing, is a planned file.
            rels.extend(helpers.iter().filter_map(|h| rel_of_path(&h.path)));
            // The derive step removed the jobs of PBR set members (Note CN); their keys still name planned files.
            if let Some(d) = &req.derive {
                let members = d.sets.iter().flat_map(|s| &s.members).chain(&d.drop);
                rels.extend(members.filter_map(|p| rel_of_path(p)));
            }
            rels.extend(failed.iter().filter_map(|f| rel_of_path(&f.path)));
            rels.extend(cancelled.iter().filter_map(|p| rel_of_path(p)));
        }
        // Every form a key can match, so each key is one lookup (DECISIONS AV3).
        let matched: std::collections::HashSet<String> = rels
            .iter()
            .flat_map(|r| {
                let KeyForms { full, stem } = KeyForms::of(r);
                std::iter::once(full).chain(stem)
            })
            .collect();
        let mut keys: BTreeSet<&String> = file_scopes.allow.keys().collect();
        keys.extend(file_scopes.overrides.iter().filter_map(|o| match &o.scope {
            Scope::Path(k) => Some(k),
            _ => None,
        }));
        for key in keys.into_iter().filter(|_| scope.is_none()) {
            if !matched.contains(&key.to_lowercase()) {
                rep.emit(
                    None,
                    None,
                    EventKind::Warning(
                        Warning::new(
                            Code::CfgFileUnmatched,
                            format!("[files] entry '{key}' matches no file"),
                        )
                        .with_data(serde_json::json!({ "key": key })),
                    ),
                );
            }
        }
        // Audits 08d-1 / 08e-3 (Note BV2): the last word on output paths, whatever made two jobs meet (default
        // packed names, moved raw maps, re-encoded DDS). Both would write the path (the last rename wins) and append
        // two manifest records; the first in plan order keeps it and every later one is IN-DUPLICATE.
        // Not run on a partial: it sees only its scope's jobs. `Session::replan_sets` falls back instead when the scope
        // holds a set of the last full plan's collisions or the partial writes an output another job writes
        // (`Skeleton::writer_clash`).
        // CS no longer reads a grey `skin_wet`: it gets no job, the rest of the batch is written (also on a partial).
        jobs.retain(|j: &ResolvedJob| {
            let Some(w) = j.warnings.iter().find(|w| w.code == Code::SkinWetGrey) else {
                return true;
            };
            skipped.push(SkippedFile {
                path: j.input.clone(),
                output: j.output.clone(),
                set: Some(j.set_key.clone()),
                code: Some(Code::SkinWetGrey),
                reason: w.message.clone(),
            });
            false
        });
        let mut writers: BTreeMap<String, PathBuf> = BTreeMap::new();
        // Note HF9: a no-set file writes nothing, so it neither claims nor loses a path.
        let no_set = crate::sets::no_set_paths(files, file_scopes.root.as_deref(), &req.set_edits);
        // Note HH4: by set, so another set's recipe whose first source is a shared no-set helper still writes.
        let no_set_sets: BTreeSet<String> = sets
            .iter()
            .filter(|s| {
                !s.members.is_empty() && s.members.iter().all(|&i| no_set.contains(&files[i].path))
            })
            .map(|s| s.key.clone())
            .collect();
        jobs.retain(|j: &ResolvedJob| {
            if scope.is_some()
                || !matches!(j.status, JobStatus::Ready)
                || no_set_sets.contains(&j.set_key)
            {
                return true;
            }
            let Some(first) = writers.get(&output_key(&req.output_root, &j.output)) else {
                writers.insert(output_key(&req.output_root, &j.output), j.input.clone());
                return true;
            };
            skipped.push(SkippedFile {
                path: j.input.clone(),
                output: j.output.clone(),
                set: Some(j.set_key.clone()),
                code: Some(Code::InDuplicate),
                reason: format!(
                    "{BV2_REASON}({} is also written from {}); change an output path or move a file",
                    slash(&j.output),
                    slash(first)
                ),
            });
            false
        });
        let index = crate::shader::JobIndex::new(&jobs);
        let mut pbr_sets = BTreeSet::new();
        for set in &sets {
            // Fuzz-1a: the `_f` the fuzz map writes counts here, on a copy (`with_f`), as it does for the other
            // members (`member_set`). `sets` never gains it: the fill step and pass 2 resolve the fuzz map with the
            // set again, and its `f` would take the fuzz map's `<base>_f.dds` (the rename guard) or make it a packing
            // input (the `_f` arm).
            let copy;
            let read = if f_sets.contains(&set.key) {
                copy = with_f(set);
                &copy
            } else {
                set
            };
            let Some(pbr) = set_is_pbr(target, read, files) else {
                continue;
            };
            if pbr {
                pbr_sets.insert(set.key.clone());
            }
            let members = set.members.iter().map(|&m| slash(&files[m].path)).collect();
            let pick = picks.get(&set.key).cloned();
            let mut warnings = vec![];
            if let Some(p) = &pick {
                warnings = crate::shader::pick_notes(p, target);
                // Note HG2: a shared set holds only the maps other skin sets use, so no map is missing or extra.
                if !set.shared_skin {
                    let maps = map_members(
                        &ctx,
                        set,
                        &f_sets,
                        files,
                        &index,
                        &skipped,
                        registry.alias_table(),
                        target,
                    );
                    warnings.extend(crate::shader::check_maps(p, target, &maps));
                }
            }
            rep.emit(
                Some(&set.key),
                None,
                EventKind::SetResolved {
                    members,
                    pbr,
                    bake: baking.contains(&set.key),
                    recipes: packed.reports.remove(&set.key).unwrap_or_default(),
                    shader: pick,
                    warnings,
                },
            );
        }
        for job in &jobs {
            rep.emit_job(
                Some(&key(&job.output)),
                Some(&job.set_key),
                Some(&job.input),
                EventKind::FilePlanned {
                    job: Box::new(job.clone()),
                },
            );
            for w in &job.warnings {
                rep.emit_job(
                    Some(&key(&job.output)),
                    Some(&job.set_key),
                    Some(&job.input),
                    EventKind::Warning(w.clone()),
                );
            }
        }
        // Review 4 I1: a consumed helper that no surviving job, skip, failure or cancellation ends (a helper-free plan
        // skips the walk).
        let fill_ends: Vec<(PathBuf, PathBuf, String)> = if fill_owner.is_empty() {
            vec![]
        } else {
            let ended: BTreeSet<&Path> = jobs
                .iter()
                .flat_map(|j| {
                    let pack = j
                        .pack
                        .iter()
                        .flat_map(|p| p.sources.iter().map(|s| &s.path));
                    let derive = j
                        .derive
                        .iter()
                        .flat_map(|d| d.sources.iter().map(|s| &s.path));
                    let fill = j.fill.iter().map(|f| &f.source.path);
                    std::iter::once(&j.input)
                        .chain(pack)
                        .chain(derive)
                        .chain(fill)
                })
                .chain(skipped.iter().map(|s| &s.path))
                .chain(failed.iter().map(|f| &f.path))
                .chain(&cancelled)
                .chain(&derive_own)
                .map(PathBuf::as_path)
                .collect();
            let mut seen = BTreeSet::new();
            fill_owner
                .into_iter()
                .filter(|(h, ..)| !ended.contains(h.as_path()) && seen.insert(h.clone()))
                .collect()
        };
        // Note FV1: a failed file's set, from the final sets (joins and `[sets]` edits applied).
        let set_of: BTreeMap<&Path, &str> = sets
            .iter()
            .flat_map(|s| {
                s.members
                    .iter()
                    .map(move |&i| (files[i].path.as_path(), s.key.as_str()))
            })
            .collect();
        for f in &mut failed {
            f.set = set_of.get(f.path.as_path()).map(|k| k.to_string());
        }
        // Note HF9: a file cancelled while planning is listed only when its set is in the run (`run` filters the
        // rest; a cancelled path keeps no set, so it is done here, where the sets are known).
        if let Some(only) = &req.write_only {
            cancelled.retain(|p| set_of.get(p.as_path()).is_none_or(|k| only.contains(*k)));
        }
        let plan = Plan {
            request: req.clone(),
            registry: registry.clone(),
            preset_id: preset_id.clone(),
            target,
            jobs,
            skipped,
            failed,
            peak_admitted_bytes,
            cancelled,
            shaders: picks,
            derive_own,
            fill_ends,
            helpers,
            no_set,
            no_set_sets,
        };
        Ok((plan, pbr_sets, f_sets))
    }

    /// Predict each scoped, ready output under the plan's existing-output policy, without writing or compacting
    /// the manifest. Cancellation discards the whole preview.
    pub fn currency_preview(
        &self,
        plan: &Plan,
        cancel: &CancelToken,
    ) -> Result<BTreeMap<String, PreviewVerdict>, EngineError> {
        let cancelled = || EngineError::Fatal("currency preview cancelled".into());
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        let manifest = Manifest::at(&plan.request.output_root);
        let existing = manifest.read().map_err(|e| {
            EngineError::Fatal(format!("manifest {}: {e}", manifest.path().display()))
        })?;
        let pool = thread_pool(plan.request.jobs)?;
        let verdicts = classify_ready_jobs(plan, &existing, &pool, cancel)
            .into_iter()
            .take_while(|_| !cancel.is_cancelled())
            .map(|(job, key, recorded)| {
                let exists = ready_output_exists(&job.output);
                (
                    key,
                    ready_verdict(plan.request.on_existing, exists, recorded),
                )
            })
            .collect();
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        Ok(verdicts)
    }

    /// Report the plan's skipped and failed files, then (unless `dry_run`) convert every ready file and emit
    /// `batch_done`. Cancelled files are listed in `batch_done` and leave no output: the files cancelled while
    /// planning, and once `cancel` is set every ready file not yet started, including one a dry run would only
    /// have listed and one that would have been up to date or kept by `--on-existing`.
    pub fn run(
        &self,
        mut plan: Plan,
        rep: &Reporter,
        cancel: &CancelToken,
    ) -> Result<BatchSummary, EngineError> {
        // Note HF9: a partial or excluding run covers the listed sets only, from its first event and count; an entry
        // with no set (an input duplicate) is a project-wide fact and stays.
        if let Some(only) = &plan.request.write_only {
            let listed = |s: Option<&String>| s.is_none_or(|s| only.contains(s));
            plan.skipped.retain(|s| listed(s.set.as_ref()));
            plan.failed.retain(|f| listed(f.set.as_ref()));
            plan.fill_ends.retain(|(_, _, set)| only.contains(set));
        }
        let req = &plan.request;
        let mut summary = BatchSummary {
            failed: plan.failed.len() as u32,
            cancelled: plan.cancelled.clone(),
            ..Default::default()
        };
        // Every skip carries the file's set key (review M9).
        let key = |out: &Path| output_key(&req.output_root, out);
        let skip = |output: &Path,
                    set: Option<&str>,
                    path: &Path,
                    kind: SkipKind,
                    code: Option<Code>,
                    reason: String,
                    s: &mut BatchSummary| {
            rep.emit_job(
                Some(&key(output)),
                set,
                Some(path),
                EventKind::FileSkipped { kind, code, reason },
            );
            s.skipped += 1;
        };
        // Note BC2 (review M3): an in-place re-run finds its own earlier outputs as DDS inputs. One whose manifest
        // record still matches the file on disk is counted into one line instead of a skip per file. The manifest is
        // read leniently here; `run` reads it again below and fails there if it is unreadable.
        let recorded = if plan
            .skipped
            .iter()
            .any(|s| s.code == Some(Code::InDdsSkipped))
        {
            Manifest::at(&req.output_root).read().unwrap_or_default()
        } else {
            Default::default()
        };
        let mut earlier = 0usize;
        for s in &plan.skipped {
            if s.code == Some(Code::InDdsSkipped)
                && recorded.get(&key(&s.path)).is_some_and(|e| {
                    e.output_stamp.is_some() && e.output_stamp == SourceStamp::of(&s.path)
                })
            {
                earlier += 1;
                continue;
            }
            skip(
                &s.output,
                s.set.as_deref(),
                &s.path,
                s.kind(),
                s.code,
                s.reason.clone(),
                &mut summary,
            );
            // IN-DUPLICATE is a conflict the user must resolve; IN-DDS-SKIPPED (Note I) and a pre-packed file
            // replaced under `packing.prefer = raw` (Note M9) are deliberate, as is a PBR set left in DDS (Note CN16).
            if matches!(
                s.code,
                Some(
                    Code::InDdsSkipped
                        | Code::PackConflict
                        | Code::DeriveDdsSource
                        | Code::DeriveNotConverted
                        | Code::SkinWetGrey
                        | Code::GenReplaced
                )
            ) {
                summary.skipped_intentional += 1;
            }
        }
        if earlier > 0 {
            rep.emit(
                None,
                None,
                EventKind::Warning(
                    Warning::new(
                        Code::InDdsSkipped,
                        format!(
                            "{earlier} earlier outputs in the input folder were not re-read (DDS inputs are not re-encoded by default)"
                        ),
                    )
                    .with_data(serde_json::json!({ "count": earlier })),
                ),
            );
        }
        for f in &plan.failed {
            rep.emit_job(
                Some(&key(&f.output)),
                None,
                Some(&f.path),
                EventKind::FileFailed {
                    stage: f.stage,
                    error: f.error.clone(),
                },
            );
        }
        // Note DY (review 4 I1): a helper whose target has no job ends with the output it filled.
        for (helper, output, set) in &plan.fill_ends {
            rep.emit_job(
                Some(&key(output)),
                Some(set),
                Some(helper),
                EventKind::FileSkipped {
                    kind: SkipKind::PackedSource,
                    code: None,
                    reason: format!("filled into {}", slash(output)),
                },
            );
        }
        // A file a recipe packs (the kept landscape `_p`'s raw height, Note CN26) already ends with that event.
        let mut derive_read: BTreeSet<&Path> = plan
            .derive_own
            .iter()
            .map(PathBuf::as_path)
            .chain(
                plan.scoped_jobs()
                    .filter_map(|j| j.pack.as_ref())
                    .flat_map(|pk| pk.sources.iter().map(|s| s.path.as_path())),
            )
            .collect();
        for job in plan.scoped_jobs() {
            let (set, path) = (Some(job.set_key.as_str()), Some(job.input.as_path()));
            // A raw map consumed by a recipe has no job of its own: its terminal event names the packed output. So
            // does each file a derived job reads (Note CN23), once, naming the first derived output it feeds.
            let consumed = job
                .pack
                .iter()
                .flat_map(|pk| pk.sources.iter().map(|s| (&s.path, "packed")))
                .chain(
                    job.derive
                        .iter()
                        .flat_map(|d| d.sources.iter().map(|s| (&s.path, "derived")))
                        // Note DY: so does a helper, with the first output it fills, unless a recipe packs it too.
                        .chain(job.fill.iter().map(|f| (&f.source.path, "filled")))
                        .filter(|(p, _)| derive_read.insert(p.as_path())),
                )
                // DECISIONS DS8: each face of a face-set cube (a broken set lists its lead file only), keyed by the
                // cube's output like a packed source.
                .chain(
                    job.cube
                        .iter()
                        .filter(|c| c.layout == crate::config::CubeLayout::Faces)
                        .flat_map(|c| c.sources.iter().map(|s| (&s.path, "cubed"))),
                );
            for (path, how) in consumed {
                rep.emit_job(
                    Some(&key(&job.output)),
                    set,
                    Some(path),
                    EventKind::FileSkipped {
                        kind: SkipKind::PackedSource,
                        code: None,
                        reason: format!("{how} into {}", slash(&job.output)),
                    },
                );
            }
            match &job.status {
                JobStatus::Ready => {}
                JobStatus::Unresolved { reasons, .. } => skip(
                    &job.output,
                    set,
                    &job.input,
                    SkipKind::Unresolved,
                    Some(unresolved_code(job)),
                    reasons.join("; "),
                    &mut summary,
                ),
                // Spec §4.4/§6.4 (DECISIONS Note M4): a raw map that is not packed is reported, with its
                // IN-PACKING-INPUT or PACK-UNUSED-INPUT warning, but does not make the batch partial.
                JobStatus::PackingInput { role } => {
                    let w = job
                        .warnings
                        .iter()
                        .find(|w| matches!(w.code, Code::InPackingInput | Code::PackUnusedInput));
                    rep.emit_job(
                        Some(&key(&job.output)),
                        set,
                        path,
                        EventKind::FileSkipped {
                            kind: SkipKind::PackingInput,
                            code: Some(w.map_or(Code::InPackingInput, |w| w.code)),
                            reason: w.map_or_else(
                                || format!("raw {} map, not converted on its own", role.id()),
                                |w| w.message.clone(),
                            ),
                        },
                    )
                }
                JobStatus::Blocked { codes } => skip(
                    &job.output,
                    set,
                    &job.input,
                    SkipKind::Blocked,
                    codes.first().copied(),
                    format!(
                        "blocked by {}; acknowledge with --allow <code> or change the setting",
                        codes
                            .iter()
                            .map(|c| c.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    &mut summary,
                ),
                // Spec §10: an error-severity code fails the file.
                JobStatus::Error { codes } => {
                    summary.failed += 1;
                    rep.emit_job(
                        Some(&key(&job.output)),
                        set,
                        path,
                        EventKind::FileFailed {
                            stage: Stage::Resolve,
                            error: format!(
                                "resolve error: {}",
                                codes
                                    .iter()
                                    .map(|c| c.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        },
                    );
                }
            }
        }
        if req.dry_run {
            if cancel.is_cancelled() {
                summary
                    .cancelled
                    .extend(plan.ready_jobs().map(|j| j.input.clone()));
            }
            summary.cancelled.sort();
            rep.emit(None, None, done_event(&summary));
            return Ok(summary);
        }

        // Carry 8: rgbcx's mode is global state; set it once before any encode, decode or validate.
        encoders_sys::init();
        std::fs::create_dir_all(&req.output_root).map_err(|e| {
            EngineError::Fatal(format!("output root {}: {e}", req.output_root.display()))
        })?;
        let manifest = Manifest::at(&req.output_root);
        let existing = manifest.load().map_err(|e| {
            EngineError::Fatal(format!("manifest {}: {e}", manifest.path().display()))
        })?;
        let pool = thread_pool(req.jobs)?;
        // Note JA: the outputs kept for an older encode version, per format and per sub-type, for the run's one line.
        let mut older_by_format: BTreeMap<Format, u32> = BTreeMap::new();
        let mut older_by_subtype: BTreeMap<Subtype, u32> = BTreeMap::new();
        enum Work<'a> {
            Encode(&'a ResolvedJob, PresetRef, bool),
            Check(&'a ResolvedJob, String, &'a ManifestEntry),
        }
        let mut todo = vec![];
        for (job, job_key, recorded) in classify_ready_jobs(&plan, &existing, &pool, cancel) {
            if cancel.is_cancelled() {
                summary.cancelled.push(job.input.clone());
                continue;
            }
            let exists = ready_output_exists(&job.output);
            let verdict = ready_verdict(req.on_existing, exists, recorded);
            let reason = match verdict {
                PreviewVerdict::OnExistingSkip => {
                    skip(
                        &job.output,
                        Some(&job.set_key),
                        &job.input,
                        SkipKind::OnExistingSkip,
                        None,
                        "output exists (--on-existing skip)".into(),
                        &mut summary,
                    );
                    summary.skipped_intentional += 1;
                    continue;
                }
                PreviewVerdict::UnknownExisting => {
                    report_unknown_existing(rep, job, &job_key);
                    summary.skipped += 1;
                    continue;
                }
                PreviewVerdict::WillWrite => None,
                PreviewVerdict::Current => {
                    if req.validate && req.on_existing == OnExisting::IfChanged {
                        if let Some(entry) = existing.get(&job_key).filter(|entry| {
                            entry.raw["validation"]["skipped"] == true
                                && entry.raw["copied_from"].is_null()
                                && pack_identity_matches(entry, job)
                        }) {
                            todo.push(Work::Check(job, job_key.clone(), entry));
                            continue;
                        }
                    }
                    Some("up to date".into())
                }
                PreviewVerdict::Kept {
                    older_encode,
                    lower_target,
                } => {
                    let mut why = vec![];
                    if older_encode {
                        summary.kept_older_encode += 1;
                        *older_by_format.entry(job.profile.format).or_default() += 1;
                        *older_by_subtype.entry(job.profile.subtype).or_default() += 1;
                        why.push(match req.ev {
                            EvScope::Matching(_) => {
                                "written by an older encode version (outside the --property scope)"
                            }
                            _ => "written by an older encode version (--ev re-encodes it)",
                        });
                    }
                    if lower_target {
                        summary.kept_lower_target += 1;
                        why.push("encoded at a higher quality target");
                    }
                    Some(format!("kept: {}", why.join("; ")))
                }
            };
            if let Some(reason) = reason {
                summary.up_to_date += 1;
                rep.emit_job(
                    Some(&job_key),
                    Some(&job.set_key),
                    Some(&job.input),
                    EventKind::FileSkipped {
                        kind: SkipKind::UpToDate,
                        code: None,
                        reason,
                    },
                );
                continue;
            }
            // Spec §3.7 (Task 26a): an aliased file records its alias preset and that preset's chain.
            let preset = match &job.suffix_alias {
                Some(a) => plan
                    .registry
                    .alias_chain(&a.preset, plan.target)
                    .ok()
                    .and_then(|c| PresetRef::for_preset(&plan.registry, &a.preset, Some(&c))),
                None => PresetRef::for_preset(&plan.registry, &plan.preset_id, None),
            };
            match preset {
                Some(p) => {
                    let replace_existing =
                        recorded.is_some() || (exists && req.on_existing == OnExisting::Overwrite);
                    todo.push(Work::Encode(job, p, replace_existing));
                }
                None => {
                    summary.failed += 1;
                    rep.emit_job(
                        Some(&key(&job.output)),
                        Some(&job.set_key),
                        Some(&job.input),
                        EventKind::FileFailed {
                            stage: Stage::Resolve,
                            error: "internal error: the resolving preset is not in the registry"
                                .into(),
                        },
                    );
                }
            }
        }

        let budget = Budget::new(budget_limit(req), pool.current_num_threads());
        let state = Mutex::new(summary);
        // Admission runs on the calling thread (outside the pool) and only spawns admitted files, so no pool thread
        // ever blocks on the budget. A pool thread blocked in `acquire` while its nested band-parallel encode stole
        // another file's task could otherwise hold budget forever (deadlock).
        pool.in_place_scope(|scope| {
            for work in &todo {
                let job = match *work {
                    Work::Check(job, ..) | Work::Encode(job, ..) => job,
                };
                if cancel.is_cancelled() {
                    lock(&state).cancelled.push(job.input.clone());
                    continue;
                }
                let est = job_bytes(job);
                budget.acquire(est);
                let admitted = Admitted(&budget, est);
                if cancel.is_cancelled() {
                    drop(admitted);
                    lock(&state).cancelled.push(job.input.clone());
                    continue;
                }
                let (state, manifest) = (&state, &manifest);
                scope.spawn(move |_| {
                    let _admitted = admitted;
                    if cancel.is_cancelled() {
                        lock(state).cancelled.push(job.input.clone());
                        return;
                    }
                    match *work {
                        Work::Check(_, ref job_key, entry) => {
                            check_one(job, job_key, entry, cancel, rep, manifest, state)
                        }
                        Work::Encode(_, ref preset, replace_existing) => run_one(
                            job,
                            req,
                            preset,
                            cancel,
                            rep,
                            manifest,
                            state,
                            replace_existing,
                        ),
                    }
                });
            }
        });
        let mut summary = state.into_inner().unwrap_or_else(|e| e.into_inner());
        summary.cancelled.sort();
        for w in kept_warnings(&summary, &req.ev, &older_by_format, &older_by_subtype) {
            rep.emit(None, None, EventKind::Warning(w));
        }
        rep.emit(None, None, done_event(&summary));
        Ok(summary)
    }
}

/// Note JA: one run-level line per reason `if-changed` kept outputs, so a run that keeps thousands reports them once.
/// `by_format` and `by_subtype` count the outputs kept for an older encode version.
fn kept_warnings(
    s: &BatchSummary,
    ev: &EvScope,
    by_format: &BTreeMap<Format, u32>,
    by_subtype: &BTreeMap<Subtype, u32>,
) -> Vec<Warning> {
    let outputs = |n: u32| {
        if n == 1 {
            "1 output was".to_string()
        } else {
            format!("{n} outputs were")
        }
    };
    let mut out = vec![];
    if s.kept_older_encode > 0 {
        let n = s.kept_older_encode;
        // The message lists the formats in `Format` order; the data maps come out sorted by id (serde_json has no
        // `preserve_order` here).
        let per = by_format
            .iter()
            .map(|(f, k)| format!("{} {k}", f.id()))
            .collect::<Vec<_>>()
            .join(", ");
        let them = if n == 1 { "it" } else { "them" };
        let how = match ev {
            EvScope::Off => {
                format!("; --ev re-encodes {them}, --ev --property <words> only the matching ones")
            }
            EvScope::Matching(_) => {
                let are = if n == 1 { "it is" } else { "they are" };
                format!("; {are} outside the --property scope")
            }
            // Unreachable: under `All` an output of an older encode version is re-encoded, never kept.
            EvScope::All => String::new(),
        };
        out.push(
            Warning::new(
                Code::OutKeptOlderEncode,
                format!(
                    "{} written by an older encode version and kept ({per}){how}",
                    outputs(n)
                ),
            )
            .with_data(serde_json::json!({
                "count": n,
                "by_format": by_format.iter().map(|(f, k)| (f.id(), *k)).collect::<BTreeMap<_, _>>(),
                "by_subtype": by_subtype.iter().map(|(t, k)| (t.id(), *k)).collect::<BTreeMap<_, _>>(),
            })),
        );
    }
    if s.kept_lower_target > 0 {
        let n = s.kept_lower_target;
        let them = if n == 1 { "it" } else { "them" };
        out.push(
            Warning::new(
                Code::OutKeptLowerTarget,
                format!(
                    "{} encoded at a higher quality target than asked and kept; --on-existing overwrite re-encodes {them}",
                    outputs(n)
                ),
            )
            .with_data(serde_json::json!({ "count": n })),
        );
    }
    out
}

/// Whether a packed job's recorded `pack.*` provenance entries equal the ones it resolves now: the same keys, and
/// each label serialised to JSON equals the recorded value. Pass two can select the same profile after pass one's
/// source-channel choice changed, so the profile hash alone does not identify the pack. A job without `pack` has
/// nothing to compare and returns true.
pub(crate) fn pack_identity_matches(entry: &ManifestEntry, job: &ResolvedJob) -> bool {
    if job.pack.is_none() {
        return true;
    }
    let Some(recorded) = entry.raw["provenance"].as_object() else {
        return false;
    };
    let mut expected = job
        .provenance
        .iter()
        .filter(|(k, _)| k.starts_with("pack."));
    let count = recorded.keys().filter(|k| k.starts_with("pack.")).count();
    expected.clone().count() == count
        && expected.all(|(key, label)| {
            serde_json::to_value(label)
                .ok()
                .is_some_and(|value| recorded.get(key) == Some(&value))
        })
}

fn report_unknown_existing(rep: &Reporter, job: &ResolvedJob, key: &str) {
    let emit = |kind| rep.emit_job(Some(key), Some(&job.set_key), Some(&job.input), kind);
    emit(EventKind::Warning(Warning::new(
        Code::OutUnknownExisting,
        format!(
            "{} exists without a manifest record; kept (use --on-existing overwrite)",
            slash(&job.output)
        ),
    )));
    emit(EventKind::FileSkipped {
        kind: SkipKind::UnknownExisting,
        code: Some(Code::OutUnknownExisting),
        reason: "existing output without a manifest record".into(),
    });
}

/// Check one admitted current output. Only the manifest is updated on success. Events carry the run loop's `job_key`.
fn check_one(
    job: &ResolvedJob,
    job_key: &str,
    entry: &ManifestEntry,
    cancel: &CancelToken,
    rep: &Reporter,
    manifest: &Manifest,
    state: &Mutex<BatchSummary>,
) {
    let emit = |kind| rep.emit_job(Some(job_key), Some(&job.set_key), Some(&job.input), kind);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        check_in_place(job, entry, cancel, manifest)
    }));
    let (stage, error) = match result {
        Ok(Ok(report)) => {
            for w in report.warnings {
                emit(EventKind::Warning(w));
            }
            let suffix = if report.summary.passed {
                CHECKED_IN_PLACE
            } else {
                CHECKED_IN_PLACE_FAILED
            };
            emit(EventKind::FileSkipped {
                kind: SkipKind::UpToDate,
                code: None,
                reason: format!("up to date; {suffix}"),
            });
            let mut s = lock(state);
            s.up_to_date += 1;
            s.checked += 1;
            s.checked_failed += u32::from(!report.summary.passed);
            return;
        }
        Ok(Err(FileError::Cancelled)) => {
            lock(state).cancelled.push(job.input.clone());
            return;
        }
        Ok(Err(FileError::Failed(stage, error))) => (stage, error),
        Err(panic) => (
            Stage::Validate,
            format!("internal error: {}", panic_message(&*panic)),
        ),
    };
    lock(state).failed += 1;
    emit(EventKind::FileFailed { stage, error });
}

/// Convert one admitted file under `catch_unwind` and report its outcome. A file that fails or panics after its
/// output was written (manifest append) has that output removed, best effort, so no output is
/// left on disk without a record (review I1). A panic is reported at the last stage the file entered (review M10).
#[allow(clippy::too_many_arguments)]
fn run_one(
    job: &ResolvedJob,
    req: &BatchRequest,
    preset: &PresetRef,
    cancel: &CancelToken,
    rep: &Reporter,
    manifest: &Manifest,
    state: &Mutex<BatchSummary>,
    replace_existing: bool,
) {
    let (set, path) = (Some(job.set_key.as_str()), Some(job.input.as_path()));
    let key = output_key(&req.output_root, &job.output);
    let emit = |kind: EventKind| rep.emit_job(Some(&key), set, path, kind);
    emit(EventKind::FileStarted);
    let tracker = FileTracker::default();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        process_file(
            job,
            req,
            preset,
            cancel,
            rep,
            manifest,
            &tracker,
            replace_existing,
        )
    }));
    let (stage, error) = match r {
        Ok(Ok(None)) => {
            report_unknown_existing(rep, job, &key);
            lock(state).skipped += 1;
            return;
        }
        Ok(Ok(Some(out))) => {
            for w in &out.report.warnings {
                emit(EventKind::Warning(w.clone()));
            }
            {
                let mut s = lock(state);
                s.converted += 1;
                if !out.report.summary.passed {
                    s.validation_failed += 1;
                }
            }
            emit(EventKind::FileDone {
                output: slash(&out.output),
                validation: out.report.summary,
            });
            return;
        }
        Ok(Err(FileError::Cancelled)) => {
            lock(state).cancelled.push(job.input.clone());
            return;
        }
        Ok(Err(FileError::Failed(stage, error))) => (stage, error),
        Err(panic) => (
            tracker.stage.get(),
            format!("internal error: {}", panic_message(&*panic)),
        ),
    };
    if tracker.written.get() {
        let _ = std::fs::remove_file(&job.output);
    }
    lock(state).failed += 1;
    emit(EventKind::FileFailed { stage, error });
}

/// Planning step 6a (DECISIONS Note S-shader-1/-2): set key → explicit shader, from `req.set_shaders` then the
/// `set:` overrides in command-line order (later wins). An id names a set by `sets::set_id`; a command-line id
/// without `/` also names every set whose key ends in `/<id>`. GUI ids are canonical and match exactly (final
/// review I2). Also returns each id that names no set, once, for one SET-EDIT-STALE each at step 6a. Computed
/// before the packing prediction (step 6), which reads it (Note AZ).
pub(crate) fn explicit_shaders(
    req: &BatchRequest,
    sets: &[TextureSet],
) -> (BTreeMap<String, Shader>, Vec<String>) {
    let gui = req.set_shaders.iter().map(|(k, s)| (k, *s, false));
    let cli = req.overrides.iter().filter_map(|o| match &o.scope {
        Scope::Set(id) => Some((
            id,
            serde_json::from_value::<Shader>(o.value.clone()).ok()?,
            true,
        )),
        _ => None,
    });
    let mut out = BTreeMap::new();
    let mut stale = vec![];
    for (id, shader, tail_match) in gui.chain(cli) {
        let tail = format!("/{}", crate::sets::fold_id(id));
        let mut hit = false;
        for s in sets {
            if crate::sets::same_id(&crate::sets::set_id(&req.output_root, &s.key), id)
                || (tail_match && !id.contains('/') && s.key.ends_with(&tail))
            {
                out.insert(s.key.clone(), shader);
                hit = true;
            }
        }
        if !hit && !stale.contains(id) {
            stale.push(id.clone());
        }
    }
    (out, stale)
}

/// [`explicit_shaders`] for the sets before set edits (Note HG2): a shader stored under a Rename's new id also names
/// the set it renames, unless that id is taken, which makes the Rename stale. `prepare` and a fast re-plan's rerun of
/// the joins both ask this, so they agree.
// shortcut: a Rename's target is checked against the pre-edit sets, not the sets as the earlier edits leave them, so
// renames in an order that frees and then takes a name (`a` -> `c`, then `b` -> `a`) lend the shader to the wrong set
// and leave its `_wet` typing undecided. Only a hand-edited project reaches this; walk the edits in order if the GUI
// ever writes such a sequence.
pub(crate) fn pre_edit_shaders(
    req: &BatchRequest,
    sets: &[TextureSet],
) -> BTreeMap<String, Shader> {
    let (mut out, _) = explicit_shaders(req, sets);
    let root = &req.output_root;
    for s in sets {
        let mut after = s.key.clone();
        for e in &req.set_edits {
            if let crate::sets::SetEdit::Rename { from, to } = e {
                let to = crate::sets::set_key(root, to);
                if after == crate::sets::set_key(root, from) && !sets.iter().any(|t| t.key == to) {
                    after = to;
                }
            }
        }
        if after != s.key {
            let renamed = TextureSet {
                key: after,
                members: vec![],
                suffixes: BTreeSet::new(),
                shared_skin: false,
                shares: vec![],
            };
            let (found, _) = explicit_shaders(req, std::slice::from_ref(&renamed));
            if let Some(&shader) = found.get(&renamed.key) {
                out.insert(s.key.clone(), shader);
            }
        }
    }
    out
}

/// The members the shader map check sees (spec §5): each non-raw member with its ordinary job (or as a kept DDS
/// input with the sub-type its suffix suggests, not the set's own earlier output), then each packed output of the
/// set. Raw maps are left out by role, because
/// `plan_packing` re-tags an unused one; duplicates and failed files are left out. A raw-named image set to
/// `helper.role = "none"` (Note DY) is an ordinary member. A member's suffix and a kept DDS's sub-type are read
/// against the set it was typed with (`member_set`, Fuzz-1a's `f` from `f_sets`).
#[allow(clippy::too_many_arguments)]
fn map_members(
    ctx: &ResolveCtx,
    set: &TextureSet,
    f_sets: &BTreeSet<String>,
    files: &[InputFile],
    jobs: &crate::shader::JobIndex,
    skipped: &[SkippedFile],
    aliases: &BTreeMap<String, String>,
    target: Target,
) -> Vec<crate::shader::MemberMap> {
    let name = |p: &Path| {
        p.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    let suffix = |f: &InputFile| {
        let stem = f.rel.file_stem().unwrap_or_default().to_string_lossy();
        member_split(&stem, &member_set(set, f, f_sets), aliases).suffix
    };
    let mut out = vec![];
    for &i in &set.members {
        let f = &files[i];
        if pack::plan::raw_role(f).is_some()
            && crate::policy::helper_requested(ctx, f).role
                != Some(crate::helper::HelperRole::NotHelper)
        {
            continue;
        }
        if let Some(j) = jobs.by_input(&f.path).find(|j| j.pack.is_none()) {
            // Note DR: a skin `_f` packing input has no file-name role, so its job status leaves it out.
            if matches!(j.status, JobStatus::PackingInput { .. }) {
                continue;
            }
            out.push(crate::shader::MemberMap {
                name: name(&j.output),
                suffix: suffix(f),
                subtype: Some(j.profile.subtype),
            });
        } else if let Some(s) = skipped
            .iter()
            .find(|s| s.path == f.path && s.code == Some(Code::InDdsSkipped))
        {
            // Audit 08b-2: an in-place run's own earlier output is not a member map; any other kept DDS fills the
            // slot its suffix suggests (`_n.dds` fills `_n`).
            if jobs
                .in_set(&set.key)
                .any(|j| crate::dds::same_file(&f.path, &j.output))
            {
                continue;
            }
            let sfx = suffix(f);
            let subtype = crate::suggest::suggest(&SuggestContext {
                target,
                suffix: sfx.as_deref(),
                game_path: &f.game_path,
                set_suffixes: &member_set(set, f, f_sets).suffixes,
                alpha: AlphaContent::None,
                is_float: false,
                advanced_skin: false,
                // Note GY4: a kept DDS is never planned, so it never waits; its name still fills a slot.
                may_wait: false,
            })
            .first()
            .map(|s| s.subtype);
            out.push(crate::shader::MemberMap {
                name: name(&s.output),
                suffix: sfx,
                subtype,
            });
        }
    }
    out.extend(jobs.in_set(&set.key).filter(|j| j.pack.is_some()).map(|j| {
        crate::shader::MemberMap {
            name: name(&j.output),
            suffix: None,
            subtype: Some(j.profile.subtype),
        }
    }));
    out
}

/// The text of a caught panic's payload (`panic!` with a literal or a formatted message), or `panic`.
fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".into())
}

/// Test hook (`test-hooks` feature only): when `SDC_TEST_CANCEL_ON` names an input's file name, planning cancels
/// the batch just before that file's Load, as a Ctrl-C would, so the CLI's cancelled-plan path can be tested
/// without a console.
#[cfg(feature = "test-hooks")]
fn cancel_on_load_for_test(path: &Path, cancel: &CancelToken) {
    if std::env::var_os("SDC_TEST_CANCEL_ON")
        .is_some_and(|v| path.file_name() == Some(v.as_os_str()))
    {
        cancel.cancel();
    }
}

/// Test hook (`test-hooks` feature only, final review I1): when `SDC_TEST_PANIC_ON_PLAN` is `<file name>` or
/// `<file name>@<stage>` (`load`, the default, `analyse` or `resolve`), planning panics as that input enters that
/// stage, to prove per-file panic isolation while planning.
#[cfg(feature = "test-hooks")]
fn panic_while_planning_for_test(path: &Path, stage: Stage) {
    let Some(v) = std::env::var_os("SDC_TEST_PANIC_ON_PLAN") else {
        return;
    };
    let v = v.to_string_lossy();
    let (name, at) = v.split_once('@').unwrap_or((&v, "load"));
    let at = match at {
        "analyse" => Stage::Analyse,
        "resolve" => Stage::Resolve,
        _ => Stage::Load,
    };
    if stage == at
        && path
            .file_name()
            .is_some_and(|n| n.to_string_lossy() == name)
    {
        panic!("test hook: SDC_TEST_PANIC_ON_PLAN");
    }
}

/// DECISIONS N2: a `[files]` sub-type must be a known one (`check_up_front`).
fn check_file_subtype(o: &SetOverride) -> Result<(), EngineError> {
    match &o.scope {
        Scope::Path(key) if o.field == "subtype" => {
            serde_json::from_value::<Subtype>(o.value.clone())
                .map(|_| ())
                .map_err(|_| {
                    EngineError::Config(format!(
                        "[files.\"{key}\"] subtype: unknown sub-type {}",
                        o.value
                    ))
                })
        }
        _ => Ok(()),
    }
}

/// DECISIONS N2: any other `[files]` value must build `st`'s profile over `doc` ([`chain_doc`]) (`check_up_front`).
fn check_file_value(
    doc: &serde_json::Value,
    target: Target,
    st: Subtype,
    o: &SetOverride,
) -> Result<(), EngineError> {
    let Scope::Path(key) = &o.scope else {
        return Ok(());
    };
    if o.field == "subtype" || crate::overrides::is_selector(&o.field) {
        return Ok(());
    }
    let mut d = doc.clone();
    merge_json(&mut d, &to_patch(&o.field, &o.value));
    finish_profile(d, target, st)
        .map(|_| ())
        .map_err(|e| EngineError::Config(format!("[files.\"{key}\"] {}={}: {e}", o.field, o.value)))
}

/// `check_up_front`'s `[files]` checks on `ovs` only, against `p`'s preset chain and configs: a skeleton patch holds
/// the entries it changes to them, so an entry the full plan would refuse never plans as a partial.
pub(crate) fn check_file_overrides(p: &Prepared, ovs: &[&SetOverride]) -> Result<(), EngineError> {
    for o in ovs {
        check_file_subtype(o)?;
    }
    if ovs.iter().all(|o| o.field == "subtype") {
        return Ok(());
    }
    let chain = p.registry.chain(&p.preset_id).map_err(preset_err)?;
    let configs: Vec<&ConfigFile> = p
        .user_config
        .iter()
        .chain(p.project_config.iter())
        .collect();
    for st in Subtype::ALL {
        let doc = chain_doc(&chain, &configs, st);
        for o in ovs {
            check_file_value(&doc, p.target, st, o)?;
        }
    }
    Ok(())
}

/// Spec §8: an invalid preset, config, rule or `--set` value, or a setting unavailable for the whole batch, is an
/// error (exit 2) before any work. Profiles are built for every sub-type from the batch preset chain and the
/// configs, then with each rule's and each `--set` override's patch; every suffix alias preset's chain (spec §3.7)
/// must build too.
fn check_up_front(
    req: &BatchRequest,
    registry: &PresetRegistry,
    target: Target,
    chain: &[&PresetFile],
    configs: &[&ConfigFile],
) -> Result<(), EngineError> {
    let usage = EngineError::Usage;
    for o in req.overrides.iter().filter(|o| o.field == "subtype") {
        serde_json::from_value::<Subtype>(o.value.clone())
            .map_err(|_| usage(format!("--set subtype: unknown sub-type {}", o.value)))?;
    }
    // DECISIONS N2: the project config's `[files]` entries are held to the same checks, as config errors.
    let mut file_overrides = vec![];
    for c in configs {
        file_overrides.extend(c.file_entries().map_err(config_err)?.0);
    }
    for o in &file_overrides {
        check_file_subtype(o)?;
    }
    let alias_presets: BTreeSet<&str> = registry
        .alias_table()
        .keys()
        .filter_map(|a| registry.alias_for(a).map(|(_, p)| p))
        .collect();
    let mut alias_chains = vec![];
    for pid in alias_presets {
        let c = registry.alias_chain(pid, target).map_err(config_err)?;
        alias_chains.push((pid, c));
    }
    // Audit #7 (Note BC): a glob that does not compile would match nothing, silently.
    for o in &req.overrides {
        if let Scope::File(g) = &o.scope {
            crate::policy::compile_glob(g)
                .map_err(|e| usage(format!("--set file:{g}.{}: {e}", o.field)))?;
        }
    }
    // Audit EC-6 (Note BJ): the rule and profile checks are `presets validate`'s, for the batch chain with the
    // configs and for every suffix alias preset's chain.
    check_chain(chain, configs, target).map_err(config_err)?;
    for (pid, c) in &alias_chains {
        check_chain(c, configs, target)
            .map_err(|e| EngineError::Config(format!("suffix alias preset '{pid}': {e}")))?;
    }
    for st in Subtype::ALL {
        let doc = chain_doc(chain, configs, st);
        for o in req.overrides.iter().filter(|o| {
            o.field != "subtype"
                && !crate::overrides::is_selector(&o.field)
                && !matches!(o.scope, Scope::Set(_))
        }) {
            let mut d = doc.clone();
            merge_json(&mut d, &to_patch(&o.field, &o.value));
            finish_profile(d, target, st)
                .map_err(|e| usage(format!("--set {}={}: {e}", o.field, o.value)))?;
        }
        for o in &file_overrides {
            check_file_value(&doc, target, st, o)?;
        }
    }
    // Spec §8: an unsupported setting for the whole batch exits 2 before work starts.
    for o in req.overrides.iter().filter(|o| o.scope == Scope::All) {
        let bad = match o.field.as_str() {
            "format" => o.value == "bc6h_uf16" && target != Target::Cs,
            "color.tag" => o.value == "srgb" && target != Target::Cs,
            "subtype" => serde_json::from_value::<Subtype>(o.value.clone())
                .is_ok_and(|st| !matches!(availability(target, st), Availability::Available)),
            _ => false,
        };
        if bad {
            return Err(usage(format!(
                "--set {}={} is unavailable for the whole batch under {target:?} (CFG-UNAVAILABLE / TAG-SRGB-VANILLA)",
                o.field, o.value
            )));
        }
    }
    Ok(())
}

fn done_event(s: &BatchSummary) -> EventKind {
    EventKind::BatchDone {
        converted: s.converted,
        up_to_date: s.up_to_date,
        checked: s.checked,
        checked_failed: s.checked_failed,
        skipped: s.skipped,
        skipped_intentional: s.skipped_intentional,
        failed: s.failed,
        validation_failed: s.validation_failed,
        cancelled: s.cancelled.iter().map(|p| slash(p)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ENGINE_VERSION;

    /// DECISIONS DS7: admission counts a cube DDS as the 6w x h strip it loads as, in both header forms.
    #[test]
    fn cube_dds_input_dims_are_the_loaded_strip() {
        use crate::dds::{dds_bytes_cube, DdsDesc, ResolvedHeader};
        let d = tempfile::tempdir().unwrap();
        for header in [ResolvedHeader::Dx10, ResolvedHeader::Legacy] {
            let desc = DdsDesc {
                width: 8,
                height: 8,
                format: crate::config::Format::Rgba8,
                srgb: false,
                header,
            };
            let levels: Vec<Vec<u8>> = (0..6)
                .flat_map(|_| [8usize, 4, 2, 1].map(|s| vec![0u8; s * s * 4]))
                .collect();
            let p = d.path().join(format!("c{header:?}.dds"));
            std::fs::write(&p, dds_bytes_cube(&desc, &levels).unwrap()).unwrap();
            let (img, ..) = crate::load::load_image_meta(&p).unwrap();
            assert_eq!(input_dims(&p), Some((img.width, img.height)), "{header:?}");
            assert_eq!(input_dims(&p), Some((48, 8)));
        }
    }

    /// A pin (passes on the old code too): admission (DS8) counts three float cubes of the face size beside the
    /// source; rebuilt on a strip when the photo layout (and its panorama term) went in 3.0.
    #[test]
    fn a_strip_cube_is_admitted_for_three_float_cubes() {
        let d = tempfile::tempdir().unwrap();
        let mut j = png_job(d.path(), "sky_e.png", (384, 64), Subtype::Cubemap);
        let base = job_bytes(&j);
        j.cube = Some(crate::cube::CubeJob {
            layout: CubeLayout::Hstrip,
            sources: vec![crate::cube::CubeSource {
                path: j.input.clone(),
                stamp: None,
                face: None,
            }],
        });
        let face = u64::from(crate::cube::layout::face_size(
            &j.profile,
            CubeLayout::Hstrip,
            (384, 64),
        ));
        assert_eq!(face, 64);
        assert_eq!(job_bytes(&j), base + 6 * face * face * 16 * 3);
    }

    #[test]
    fn exit_code_precedence() {
        let mut s = BatchSummary::default();
        assert_eq!(s.exit_code(), 0);
        s.up_to_date = 3;
        assert_eq!(s.exit_code(), 0);
        (s.skipped, s.skipped_intentional) = (2, 2);
        assert_eq!(s.exit_code(), 0);
        s.skipped = 3;
        assert_eq!(s.exit_code(), 1);
        s.skipped = 2;
        s.validation_failed = 1;
        assert_eq!(s.exit_code(), 4);
        s.cancelled.push("a.png".into());
        assert_eq!(s.exit_code(), 1);
        assert_eq!(EngineError::Usage("x".into()).exit_code(), 2);
        assert_eq!(EngineError::Config("x".into()).exit_code(), 2);
        assert_eq!(EngineError::Fatal("x".into()).exit_code(), 3);
    }

    #[test]
    fn budget_admits_an_oversized_file_alone_and_releases() {
        let b = Budget::new(10, 8);
        b.acquire(100);
        assert_eq!(*lock(&b.used), (100, 1));
        drop(Admitted(&b, 100));
        assert_eq!(*lock(&b.used), (0, 0));
        b.acquire(4);
        b.acquire(6);
        assert_eq!(*lock(&b.used), (10, 2));
    }

    /// Review Focus 4 (Task 7 review I2): a 16K `_n` (16-bit RGBA, the widest integer source) baking into a 4K
    /// roughness member is admitted for at least its peak. The `_n` decode (decoded image 8 + its `into_rgba32f`
    /// conversion 16 per texel) and the member's own conversion (with the 1.33 B/texel pyramid) never overlap.
    #[test]
    fn a_16k_normal_baking_into_a_4k_member_is_admitted_for_its_peak() {
        let (n, m) = (16384u64 * 16384, 4096u64 * 4096);
        let admitted = input_bytes(4096, 4096) + crate::bake::BakeJob::peak_bytes(16384, 16384, 8);
        let decode_peak = n * (8 + 16);
        let member_peak = input_bytes(4096, 4096) + n * 4 / 3 + 1;
        assert!(admitted >= decode_peak, "{admitted} < {decode_peak}");
        assert!(admitted >= member_peak, "{admitted} < {member_peak}");
        assert!(m * 64 <= input_bytes(4096, 4096));
    }

    /// Fix wave (final review I2): a re-encoded DDS is admitted for its size, read from its header, plus its file
    /// bytes; a non-DDS `.dds` and a hostile size do not panic.
    #[test]
    fn a_dds_input_is_admitted_for_its_header_size_plus_its_file() {
        use crate::dds::*;
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("a.DDS");
        let desc = DdsDesc {
            width: 64,
            height: 32,
            format: crate::config::Format::Bc7,
            srgb: false,
            header: ResolvedHeader::Dx10,
        };
        let b = dds_bytes(&desc, &[vec![0; 64 * 32]]).unwrap();
        std::fs::write(&p, &b).unwrap();
        assert_eq!(estimated_bytes(&p), input_bytes(64, 32) + b.len() as u64);
        let mut huge = b.clone();
        huge[12..20].fill(0xff);
        std::fs::write(&p, &huge).unwrap();
        assert_eq!(
            estimated_bytes(&p),
            input_bytes(crate::load::MAX_DIM, crate::load::MAX_DIM) + b.len() as u64
        );
        std::fs::write(&p, b"not a dds").unwrap();
        assert_eq!(estimated_bytes(&p), 0);
    }

    /// Residual fix (re-review minor 1): a non-DDS input's size is read by the decoder its content picks, so a
    /// PNG named `.bmp` is admitted for its size, not 0.
    #[test]
    fn a_non_dds_input_is_admitted_for_its_size_by_content() {
        let d = tempfile::tempdir().unwrap();
        let png = d.path().join("a.png");
        image::GrayImage::new(64, 32).save(&png).unwrap();
        let lying = d.path().join("a.bmp");
        std::fs::rename(&png, &lying).unwrap();
        assert_eq!(estimated_bytes(&lying), input_bytes(64, 32));
    }

    /// A Cs job of `subtype` reading a `w`×`h` RGB PNG at `dir/name`.
    fn png_job(dir: &Path, name: &str, (w, h): (u32, u32), subtype: Subtype) -> ResolvedJob {
        let input = dir.join(name);
        image::RgbImage::new(w, h).save(&input).unwrap();
        ResolvedJob {
            input,
            output: dir.join("out.dds"),
            game_path: "a".into(),
            set_key: "x/a".into(),
            profile: PresetRegistry::builtin()
                .unwrap()
                .base_profile(Target::Cs, subtype),
            provenance: Default::default(),
            suggestions: vec![],
            warnings: vec![],
            status: JobStatus::Ready,
            suffix_alias: None,
            source: None,
            pack: None,
            bake: None,
            derive: None,
            shader: None,
            cube: None,
            fill: None,
            unknown_suffix: false,
        }
    }

    /// Note BB: complex grass holds both half chains and the stacked chain at once, one float chain (16 B/texel ×
    /// 1.34) more than the generic model; about 1.33x the generic admission.
    #[test]
    fn complex_grass_is_admitted_for_its_chain_peak() {
        let d = tempfile::tempdir().unwrap();
        let mut j = png_job(d.path(), "g.png", (32, 64), Subtype::ComplexGrass);
        let grass = job_bytes(&j);
        j.profile.subtype = Subtype::Diffuse;
        let generic = job_bytes(&j);
        assert_eq!(grass, generic + 32 * 64 * 16 * 134 / 100);
        let ratio = grass as f64 / input_bytes(32, 64) as f64;
        assert!((1.3..=1.5).contains(&ratio), "{ratio}");
    }

    /// Generic admission (kept from the Note BB layered tests): a job larger than the whole budget is admitted alone,
    /// and a second job waits while the first holds the budget.
    #[test]
    fn an_oversized_job_runs_alone_and_blocks_a_second() {
        let d = tempfile::tempdir().unwrap();
        let j = png_job(d.path(), "big.png", (64, 64), Subtype::Diffuse);
        let n = job_bytes(&j);
        let oversized = Budget::new(n / 2, 8);
        oversized.acquire(n);
        assert_eq!(*lock(&oversized.used), (n, 1));
        // Two jobs of `n` do not fit in `n + n / 2`: the second is admitted only after the first is released.
        let b = Budget::new(n + n / 2, 8);
        b.acquire(n);
        let admitted = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|s| {
            s.spawn(|| {
                b.acquire(n);
                admitted.store(true, Ordering::SeqCst);
            });
            std::thread::sleep(std::time::Duration::from_millis(100));
            assert!(
                !admitted.load(Ordering::SeqCst),
                "second job admitted beside the first"
            );
            b.release(n);
        });
        assert!(admitted.load(Ordering::SeqCst));
        assert_eq!(*lock(&b.used), (n, 1));
    }

    #[test]
    fn kept_coat_admits_its_lengths_and_self_bake_directions() {
        let d = tempfile::tempdir().unwrap();
        let mut j = png_job(d.path(), "coat.png", (8, 8), Subtype::CoatNormal);
        j.profile.format = Format::Rgba8;
        j.profile.alpha.policy = AlphaPolicy::Data;
        j.profile.mips.normal = NormalMips::Renormalize;
        let base = job_bytes(&j);
        j.profile.mips.normal_base = NormalBase::Keep;
        let lengths = 4 * 4 * 8;
        j.profile.mips.normal = NormalMips::RenormalizeBake;
        assert_eq!(job_bytes(&j), base + lengths + (8 * 8 + 4 * 4) * 16);
        j.profile.mips.normal = NormalMips::Renormalize;
        assert_eq!(job_bytes(&j), base + lengths);
    }

    /// Note DY: a filled job also holds its helper's decoded image.
    #[test]
    fn job_bytes_counts_helper() {
        let d = tempfile::tempdir().unwrap();
        let mut j = png_job(d.path(), "x.png", (32, 32), Subtype::Diffuse);
        let base = job_bytes(&j);
        let helper = d.path().join("x_opacity.png");
        image::GrayImage::new(16, 16).save(&helper).unwrap();
        j.fill = Some(crate::helper::FillJob {
            source: crate::pack::PackSource {
                read: None,
                path: helper.clone(),
                role: crate::config::RawMap::Opacity,
                stamp: None,
            },
            read: None,
            invert: false,
            assigned: false,
        });
        assert!(estimated_bytes(&helper) > 0);
        assert_eq!(job_bytes(&j), base + estimated_bytes(&helper));
    }

    /// Note FM30: a smaller target is enlarged to its helper's size, so the job is charged as that larger image; a
    /// pack's pass-2 estimate counts every source at the common size.
    #[test]
    fn an_enlarged_source_is_admitted_at_the_common_size() {
        let d = tempfile::tempdir().unwrap();
        let mut small = png_job(d.path(), "x.png", (32, 32), Subtype::Diffuse);
        let big = png_job(d.path(), "y.png", (64, 64), Subtype::Diffuse);
        let helper = d.path().join("x_opacity.png");
        image::GrayImage::new(64, 64).save(&helper).unwrap();
        small.fill = Some(crate::helper::FillJob {
            source: crate::pack::PackSource {
                read: None,
                path: helper.clone(),
                role: crate::config::RawMap::Opacity,
                stamp: None,
            },
            read: None,
            invert: false,
            assigned: true,
        });
        assert_eq!(
            job_bytes(&small),
            job_bytes(&big) + estimated_bytes(&helper)
        );
        let paths = [small.input.as_path(), big.input.as_path()];
        assert_eq!(
            grown_bytes(paths.into_iter()),
            vec![input_bytes(64, 64); 2],
            "both sources at the larger size"
        );
        assert_eq!(
            grown_bytes([big.input.as_path()].into_iter()),
            vec![estimated_bytes(&big.input)]
        );
    }

    /// Note CN13: a derived job adds 16 B/texel per colour map, the scalars and the output (twice for the grass
    /// stack, which reads the albedo and the normal).
    #[test]
    fn a_derived_job_is_admitted_for_its_colour_scalars_and_output() {
        use crate::derive::{DeriveJob, DeriveKind};
        use crate::pack::ChannelSource::Default;
        let d = tempfile::tempdir().unwrap();
        let mut j = png_job(d.path(), "a_n.png", (32, 32), Subtype::Normal);
        let base = job_bytes(&j);
        let dj = |kind| DeriveJob {
            kind,
            landscape: false,
            albedo: None,
            normal: None,
            rough: Default { value: 0.5 },
            metal: Default { value: 0.0 },
            ao: Default { value: 1.0 },
            height: None,
            curves: None,
            cavity: None,
            gray: crate::config::GraySource::R,
            params: crate::derive::recipe::LegacyParams::default(),
            sources: vec![],
        };
        j.derive = Some(dj(DeriveKind::LegacyN));
        assert_eq!(job_bytes(&j), base + 32 * 32 * 16 * 3);
        j.derive = Some(dj(DeriveKind::GrassStack));
        assert_eq!(job_bytes(&j), base + 32 * 32 * 16 * 5);
    }

    /// S-mips-11: a non-box filter adds its 8 B/texel intermediate, a resize its 32 B/texel; a DDS is sized from
    /// its header for both (ruling F4).
    #[test]
    fn admission_counts_the_filter_and_resize_intermediates() {
        let d = tempfile::tempdir().unwrap();
        let png = d.path().join("a.png");
        image::GrayImage::new(48, 40).save(&png).unwrap();
        let mut job = ResolvedJob {
            input: png,
            output: d.path().join("a.dds"),
            game_path: "a".into(),
            set_key: "x/a".into(),
            profile: PresetRegistry::builtin()
                .unwrap()
                .base_profile(Target::Cs, Subtype::Diffuse),
            provenance: Default::default(),
            suggestions: vec![],
            warnings: vec![],
            status: JobStatus::Ready,
            suffix_alias: None,
            source: None,
            pack: None,
            bake: None,
            derive: None,
            shader: None,
            cube: None,
            fill: None,
            unknown_suffix: false,
        };
        let texels = 48 * 40;
        // `mips.levels = "none"` keeps the 48x40 size (Note FM29).
        let full = std::mem::replace(
            &mut job.profile.mips.levels,
            crate::config::MipLevels::Keyword(crate::config::MipKeyword::None),
        );
        assert_eq!(job_bytes(&job), input_bytes(48, 40));
        job.profile.mips.filter = MipFilter::Kaiser;
        assert_eq!(job_bytes(&job), input_bytes(48, 40) + texels * 8);
        job.profile.mips.filter = MipFilter::Box;
        // With mips, 48x40 is written 32x16 (it only shrinks).
        job.profile.mips.levels = full;
        assert_eq!(job_bytes(&job), input_bytes(48, 40) + texels * 32);

        use crate::dds::*;
        let dds = d.path().join("a.dds");
        let desc = DdsDesc {
            width: 48,
            height: 40,
            format: crate::config::Format::Bc7,
            srgb: false,
            header: ResolvedHeader::Dx10,
        };
        let b = dds_bytes(&desc, &[vec![0; 48 * 40]]).unwrap();
        std::fs::write(&dds, &b).unwrap();
        job.input = dds;
        assert_eq!(
            job_bytes(&job),
            input_bytes(48, 40) + b.len() as u64 + texels * 32
        );
        // 31x20 is written 32x16: the width grows, so the enlarge's 64 B/texel is charged.
        let grow = d.path().join("g.png");
        image::GrayImage::new(31, 20).save(&grow).unwrap();
        job.input = grow;
        assert_eq!(job_bytes(&job), input_bytes(31, 20) + 31 * 20 * 64);
    }

    /// The kept lines end "it" after a count of 1 and "them" after more (the singular case is pinned in
    /// `tests/api.rs`).
    #[test]
    fn the_kept_lines_say_them_for_more_than_one() {
        let s = BatchSummary {
            kept_older_encode: 2,
            kept_lower_target: 3,
            ..Default::default()
        };
        let by_format = BTreeMap::from([(Format::Bc7, 2)]);
        let by_subtype = BTreeMap::from([(Subtype::Diffuse, 2)]);
        let w = kept_warnings(&s, &EvScope::Off, &by_format, &by_subtype);
        let msg = |c: Code| w.iter().find(|w| w.code == c).unwrap().message.clone();
        assert_eq!(
            msg(Code::OutKeptOlderEncode),
            "2 outputs were written by an older encode version and kept (bc7 2); --ev re-encodes them, --ev --property <words> only the matching ones"
        );
        assert_eq!(
            msg(Code::OutKeptLowerTarget),
            "3 outputs were encoded at a higher quality target than asked and kept; --on-existing overwrite re-encodes them"
        );
        // Under `--property` the tail is singular after a count of 1, plural after more.
        let scope = EvScope::matching(vec![crate::ev_scope::PropertyGroup::new(vec![
            crate::ev_scope::Property::Cutout,
        ])
        .unwrap()])
        .unwrap();
        let one = BatchSummary {
            kept_older_encode: 1,
            ..Default::default()
        };
        let w = kept_warnings(&one, &scope, &by_format, &by_subtype);
        assert!(
            w[0].message
                .ends_with("; it is outside the --property scope"),
            "{}",
            w[0].message
        );
        let w = kept_warnings(&s, &scope, &by_format, &by_subtype);
        assert!(
            w[0].message
                .ends_with("; they are outside the --property scope"),
            "{}",
            w[0].message
        );
    }

    fn preview_fixture() -> (tempfile::TempDir, Engine, BatchRequest) {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in");
        std::fs::create_dir(&input).unwrap();
        let mut req = BatchRequest::new(vec![input.clone()], dir.path().join("out"), Target::Cs);
        for name in ["a", "b", "c", "unassigned"] {
            image::RgbImage::from_fn(32, 32, |x, y| {
                image::Rgb([(x * 7) as u8, (y * 5) as u8, 90])
            })
            .save(input.join(format!("{name}.png")))
            .unwrap();
            req.overrides.push(
                crate::overrides::parse_set(&format!("file:**/{name}.png.subtype=diffuse"))
                    .unwrap(),
            );
        }
        req.set_edits.push(crate::sets::SetEdit::NoSet {
            file: "unassigned.png".into(),
        });
        (dir, Engine::new().unwrap(), req)
    }

    #[test]
    fn currency_preview_agrees_with_the_run() {
        use PreviewVerdict::{Current, WillWrite};
        let (_dir, engine, req) = preview_fixture();
        let cancel = CancelToken::default();
        let sink = Arc::new(crate::events::VecSink::default());
        let rep = Reporter::new(sink.clone(), "preview");
        let plan = engine.plan(req.clone(), &rep, &cancel).unwrap();
        let expected = |b| {
            BTreeMap::from([
                ("a.dds".into(), Current),
                ("b.dds".into(), b),
                ("c.dds".into(), Current),
            ])
        };
        let first = engine.currency_preview(&plan, &cancel).unwrap();
        assert_eq!(
            first,
            ["a.dds", "b.dds", "c.dds"]
                .map(|k| (k.to_owned(), WillWrite))
                .into()
        );
        assert!(!req.output_root.exists());
        assert_eq!(engine.run(plan, &rep, &cancel).unwrap().converted, 3);

        image::RgbImage::from_pixel(32, 32, image::Rgb([12, 55, 92]))
            .save(req.inputs[0].join("b.png"))
            .unwrap();
        let manifest = Manifest::at(&req.output_root);
        let mut plan = engine.plan(req, &rep, &cancel).unwrap();
        // Duplicate a stale record past both compaction thresholds, retaining the three live records.
        let mut line = br#"{"schema":1,"output":"stale.dds"}"#.to_vec();
        line.resize(65535, b' ');
        line.push(b'\n');
        let n = crate::manifest::COMPACT_MIN_BYTES as usize / line.len() + 1;
        let mut before = line.repeat(n);
        before.extend(std::fs::read(manifest.path()).unwrap());
        std::fs::write(manifest.path(), &before).unwrap();
        let verdicts = engine.currency_preview(&plan, &cancel).unwrap();
        assert_eq!(verdicts, expected(WillWrite));
        plan.request.on_existing = OnExisting::Reencode;
        assert_eq!(engine.currency_preview(&plan, &cancel).unwrap(), first);
        plan.request.on_existing = OnExisting::IfChanged;
        assert!(
            std::fs::read(manifest.path()).unwrap() == before,
            "currency preview rewrote the manifest"
        );
        sink.take();
        let summary = engine.run(plan, &rep, &cancel).unwrap();
        assert_eq!((summary.converted, summary.up_to_date), (1, 2));
        let mut terminal = BTreeMap::new();
        for event in sink.take() {
            let verdict = match event.kind {
                EventKind::FileDone { .. } => WillWrite,
                EventKind::FileSkipped {
                    kind: SkipKind::UpToDate,
                    ..
                } => Current,
                EventKind::FileSkipped { .. } | EventKind::FileFailed { .. } => panic!("{event:?}"),
                _ => continue,
            };
            assert!(terminal.insert(event.job.unwrap(), verdict).is_none());
        }
        assert_eq!(terminal, verdicts);
    }

    #[test]
    fn run_protects_an_unrecorded_dds_that_appears_on_file_started() {
        use crate::events::{EventSink, VecSink};
        for mode in [
            OnExisting::IfChanged,
            OnExisting::Reencode,
            OnExisting::Skip,
            OnExisting::Overwrite,
        ] {
            let (_dir, engine, mut req) = preview_fixture();
            req.on_existing = mode;
            req.write_only = Some([crate::sets::set_key(&req.output_root, "a")].into());
            let path = req.output_root.join("a.dds");
            let dest = path.clone();
            let events = Arc::new(VecSink::default());
            let log = events.clone();
            let rep = Reporter::new(
                Arc::new(OnEvent(move |e: &crate::events::EngineEvent| {
                    if matches!(e.kind, EventKind::FileStarted) {
                        assert_eq!(e.job.as_deref(), Some("a.dds"));
                        std::fs::write(&dest, b"foreign bytes").unwrap();
                    }
                    log.emit(e);
                })),
                "late file",
            );
            let cancel = CancelToken::default();
            let manifest = Manifest::at(&req.output_root);
            let plan = engine.plan(req, &rep, &cancel).unwrap();
            let summary = engine.run(plan, &rep, &cancel).unwrap();
            assert!(
                events
                    .take()
                    .iter()
                    .any(|e| e.job.as_deref() == Some("a.dds")
                        && matches!(
                            e.kind,
                            EventKind::FileSkipped {
                                kind: SkipKind::UnknownExisting,
                                ..
                            }
                        )),
                "{mode:?}"
            );
            assert_eq!(std::fs::read(&path).unwrap(), b"foreign bytes");
            assert_eq!(
                (summary.converted, summary.skipped, summary.failed),
                (0, 1, 0)
            );
            assert!(manifest.read().unwrap().is_empty());
        }
    }

    struct OnEvent<F>(F);

    impl<F: Fn(&crate::events::EngineEvent) + Send + Sync> crate::events::EventSink for OnEvent<F> {
        fn emit(&self, event: &crate::events::EngineEvent) {
            self.0(event);
        }
    }

    #[test]
    fn cancelled_ready_outputs_skip_the_existence_check() {
        let (_dir, engine, req) = preview_fixture();
        std::fs::create_dir(&req.output_root).unwrap();
        std::fs::write(req.output_root.join("a.dds"), b"foreign bytes").unwrap();
        let cancel = CancelToken::default();
        let token = cancel.clone();
        let rep = Reporter::new(
            Arc::new(OnEvent(move |e: &crate::events::EngineEvent| {
                if e.job.as_deref() == Some("a.dds")
                    && matches!(e.kind, EventKind::FileSkipped { .. })
                {
                    token.cancel();
                }
            })),
            "cancel",
        );
        let remaining = ["b.png", "c.png"].map(|name| req.inputs[0].join(name));
        let plan = engine.plan(req, &rep, &cancel).unwrap();
        READY_EXISTENCE_CHECKS.set(0);
        let summary = engine.run(plan, &rep, &cancel).unwrap();
        assert_eq!(summary.cancelled, remaining);
        assert_eq!(READY_EXISTENCE_CHECKS.get(), 1, "only a.dds was checked");
    }

    #[test]
    fn currency_preview_protects_an_unrecorded_dds() {
        let (_dir, engine, mut req) = preview_fixture();
        let cancel = CancelToken::default();
        let sink = Arc::new(crate::events::VecSink::default());
        let rep = Reporter::new(sink.clone(), "preview");
        req.write_only = Some([crate::sets::set_key(&req.output_root, "a")].into());
        std::fs::create_dir(&req.output_root).unwrap();
        let path = req.output_root.join("a.dds");
        std::fs::write(&path, b"foreign bytes").unwrap();
        for mode in [OnExisting::IfChanged, OnExisting::Reencode] {
            req.on_existing = mode;
            let mut plan = engine.plan(req.clone(), &rep, &cancel).unwrap();
            for (policy, want) in [
                (mode, PreviewVerdict::UnknownExisting),
                (OnExisting::Skip, PreviewVerdict::OnExistingSkip),
                (OnExisting::Overwrite, PreviewVerdict::WillWrite),
            ] {
                plan.request.on_existing = policy;
                assert_eq!(
                    engine.currency_preview(&plan, &cancel).unwrap(),
                    [("a.dds".into(), want)].into()
                );
            }
            plan.request.on_existing = mode;
            let summary = engine.run(plan, &rep, &cancel).unwrap();
            assert_eq!(
                (
                    summary.converted,
                    summary.skipped,
                    summary.skipped_intentional
                ),
                (0, 1, 0)
            );
            let protected: Vec<_> = sink
                .take()
                .into_iter()
                .filter(|e| {
                    matches!(
                        e.kind,
                        EventKind::FileSkipped {
                            kind: SkipKind::UnknownExisting,
                            ..
                        }
                    )
                })
                .map(|e| e.job.unwrap())
                .collect();
            assert_eq!(protected, ["a.dds"]);
            assert_eq!(std::fs::read(&path).unwrap(), b"foreign bytes");
        }
    }

    #[test]
    fn currency_preview_counts_a_doubly_kept_output_once() {
        let (_dir, engine, mut req) = preview_fixture();
        let cancel = CancelToken::default();
        let rep = Reporter::new(Arc::new(crate::events::VecSink::default()), "preview");
        req.write_only = Some([crate::sets::set_key(&req.output_root, "a")].into());
        let mut plan = engine.plan(req.clone(), &rep, &cancel).unwrap();
        for job in &mut plan.jobs {
            job.profile.encoder.effort = Effort::Veryslow;
        }
        assert_eq!(engine.run(plan, &rep, &cancel).unwrap().converted, 1);
        let manifest = Manifest::at(&req.output_root);
        let mut record: Value =
            serde_json::from_slice(&std::fs::read(manifest.path()).unwrap()).unwrap();
        record["encode_version"] = "0.9.0".into();
        std::fs::write(manifest.path(), serde_json::to_vec(&record).unwrap()).unwrap();
        let mut plan = engine.plan(req, &rep, &cancel).unwrap();
        for job in &mut plan.jobs {
            job.profile.encoder.effort = Effort::Fast;
        }
        let kept = PreviewVerdict::Kept {
            older_encode: true,
            lower_target: true,
        };
        assert_eq!(
            engine.currency_preview(&plan, &cancel).unwrap(),
            [("a.dds".into(), kept)].into()
        );
        plan.request.ev = EvScope::All;
        assert_eq!(
            engine.currency_preview(&plan, &cancel).unwrap(),
            [("a.dds".into(), PreviewVerdict::WillWrite)].into()
        );
    }

    #[test]
    fn currency_preview_returns_an_error_when_cancelled() {
        let (_dir, engine, req) = preview_fixture();
        let cancel = CancelToken::default();
        let rep = Reporter::new(Arc::new(crate::events::VecSink::default()), "preview");
        let plan = engine.plan(req, &rep, &cancel).unwrap();
        cancel.cancel();
        assert!(engine.currency_preview(&plan, &cancel).is_err());
    }

    #[test]
    fn currency_preview_returns_an_error_for_an_unreadable_manifest() {
        let (_dir, engine, req) = preview_fixture();
        let cancel = CancelToken::default();
        let rep = Reporter::new(Arc::new(crate::events::VecSink::default()), "preview");
        let manifest = Manifest::at(&req.output_root);
        let plan = engine.plan(req, &rep, &cancel).unwrap();
        std::fs::create_dir_all(manifest.path()).unwrap();
        assert!(engine.currency_preview(&plan, &cancel).is_err());
    }

    /// Note JA (carry 13, review M2/I2, ruling O2 kept): the owner's if-changed rule, one good BC7 diffuse record and
    /// its variants. Each row gives the verdict without and with `--ev` covering the output.
    #[test]
    fn currency_follows_the_owners_rule() {
        use crate::manifest::MANIFEST_SCHEMA;
        use Currency::{Current, Kept, Stale};
        const ENC: Currency = Kept {
            older_encode: true,
            lower_target: false,
        };
        const TGT: Currency = Kept {
            older_encode: false,
            lower_target: true,
        };
        const BOTH: Currency = Kept {
            older_encode: true,
            lower_target: true,
        };
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("rock.png");
        std::fs::write(&src, b"source bytes").unwrap();
        let reg = PresetRegistry::builtin().unwrap();
        let job0 = ResolvedJob {
            input: src.clone(),
            output: d.path().join("rock.dds"),
            game_path: "rock".into(),
            set_key: "x/rock".into(),
            profile: reg.base_profile(Target::Cs, Subtype::Diffuse),
            provenance: Default::default(),
            suggestions: vec![],
            warnings: vec![],
            status: JobStatus::Ready,
            suffix_alias: None,
            source: None,
            pack: None,
            bake: None,
            derive: None,
            shader: None,
            cube: None,
            fill: None,
            unknown_suffix: false,
        };
        assert_eq!(job0.profile.format, Format::Bc7);
        std::fs::write(&job0.output, b"dds").unwrap();
        let enc = EncoderVersions::current();
        // The record the run writes for `job` when the profile was `job`'s edited by `rec`.
        let entry = |job: &ResolvedJob, rec: &dyn Fn(&mut Profile)| {
            let mut p = job.profile.clone();
            rec(&mut p);
            ManifestEntry {
                schema: MANIFEST_SCHEMA,
                output: "rock.dds".into(),
                source_sha256: sha256_file(&src).unwrap(),
                sources: vec![],
                profile_sha256: profile_sha256(&p).unwrap(),
                engine_version: ENGINE_VERSION.into(),
                encode_version: Some(crate::manifest::encode_version(job.profile.subtype).into()),
                encoders: Some(enc.clone()),
                preset: PresetRef::for_preset(&reg, "cs-quality", None),
                bc1_model: Some(BC1_ENCODE_ID.into()),
                output_stamp: SourceStamp::of(&job.output),
                copied_from: None,
                raw: serde_json::json!({
                    "validation": { "passed": true, "levels": 7 },
                    "profile": p,
                }),
            }
        };
        type Edit<'a, T> = &'a dyn Fn(&mut T);
        let same: Edit<Profile> = &|_| {};
        let as_is: Edit<ManifestEntry> = &|_| {};
        let mut rows: Vec<(String, ResolvedJob, ManifestEntry, [Currency; 2])> = vec![];
        let mut row = |what: &str,
                       job: Edit<Profile>,
                       rec: Edit<Profile>,
                       edit: Edit<ManifestEntry>,
                       want: [Currency; 2]| {
            let mut j = job0.clone();
            job(&mut j.profile);
            let mut e = entry(&j, rec);
            edit(&mut e);
            rows.push((what.to_string(), j, e, want));
        };
        let fmt = |f: Format| move |p: &mut Profile| p.format = f;
        let effort = |x: Effort| move |p: &mut Profile| p.encoder.effort = x;
        let radius = |r: SearchRadius| move |p: &mut Profile| p.encoder.bc45_search_radius = r;
        let search = |s: Bc1Search| move |p: &mut Profile| p.encoder.bc1_search = s;
        let both = |a: &dyn Fn(&mut Profile), b: &dyn Fn(&mut Profile), p: &mut Profile| {
            a(p);
            b(p)
        };
        let ev = |v: &'static str| move |e: &mut ManifestEntry| e.encode_version = Some(v.into());

        row("matching record", same, same, as_is, [Current, Current]);
        row(
            "other engine_version",
            same,
            same,
            &|e| e.engine_version = "0.0.0".into(),
            [Current, Current],
        );
        for v in ["0.9.0", "1.0.0", "1.1.0"] {
            row(
                &format!("no encode_version, engine {v}"),
                same,
                same,
                &|e| {
                    e.encode_version = None;
                    e.engine_version = v.into()
                },
                [Stale, Stale],
            );
        }
        row(
            "no encoders",
            same,
            same,
            &|e| e.encoders = None,
            [Stale, Stale],
        );
        row(
            "no preset",
            same,
            same,
            &|e| e.preset = None,
            [Stale, Stale],
        );
        row(
            "newer schema",
            same,
            same,
            &|e| e.schema = MANIFEST_SCHEMA + 1,
            [Stale, Stale],
        );
        for v in ["2.5.0", "99.0.0", "0.9.0"] {
            row(
                &format!("encode_version {v}"),
                same,
                same,
                &ev(v),
                [ENC, Stale],
            );
        }
        row("empty encode_version", same, same, &ev(""), [Stale, Stale]);
        row(
            "other ispc",
            same,
            same,
            &|e| e.encoders.as_mut().unwrap().ispc = "1.0.0".into(),
            [ENC, Stale],
        );
        // The rgbcx patches only change BC1/BC3/BC4/BC5 output; a record from before them is current for the rest.
        for f in [
            Format::Bc1,
            Format::Bc3,
            Format::Bc4,
            Format::Bc5,
            Format::Bc6hUf16,
            Format::Bc7,
            Format::Rgba8,
        ] {
            let want = match f {
                Format::Bc1 | Format::Bc3 | Format::Bc4 | Format::Bc5 => [ENC, Stale],
                _ => [Current, Current],
            };
            row(
                &format!("{f:?} without rgbcx_patch"),
                &fmt(f),
                same,
                &|e| e.encoders.as_mut().unwrap().rgbcx_patch = None,
                want,
            );
            // The fake hash only proves that any other tag string counts, not the real old value format.
            row(
                &format!("{f:?} old rgbcx_patch"),
                &fmt(f),
                same,
                &|e| {
                    e.encoders.as_mut().unwrap().rgbcx_patch =
                        Some("bc4-round-to-nearest@sha256:00".into())
                },
                want,
            );
        }
        // The BC1 encode id only matters for the formats whose colour blocks are encoded as BC1.
        for f in [Format::Bc1, Format::Bc3] {
            row(
                &format!("{f:?} 1.x target"),
                &fmt(f),
                same,
                &|e| e.bc1_model = Some("ideal".into()),
                [ENC, Stale],
            );
            row(
                &format!("{f:?} unrecorded target"),
                &fmt(f),
                same,
                &|e| e.bc1_model = None,
                [ENC, Stale],
            );
        }
        row(
            "BC7 unrecorded target",
            same,
            same,
            &|e| e.bc1_model = None,
            [Current, Current],
        );
        let stale: [(&str, Edit<ManifestEntry>); 7] = [
            ("other source", &|e| e.source_sha256 = "0".repeat(64)),
            ("no output stamp", &|e| e.output_stamp = None),
            ("replaced output", &|e| {
                e.output_stamp.as_mut().unwrap().size += 1
            }),
            ("validation failed", &|e| {
                e.raw["validation"]["passed"] = false.into()
            }),
            ("no validation", &|e| {
                e.raw.as_object_mut().unwrap().remove("validation");
            }),
            ("a packed source changed", &|e| {
                e.sources = vec![crate::manifest::SourceRef {
                    path: "a.png".into(),
                    sha256: "0".repeat(64),
                    role: None,
                    invert: false,
                    bake: None,
                    derive: None,
                    derive_role: None,
                    cube_face: None,
                    fill: None,
                }]
            }),
            // The record's own profile, but a hash it does not have.
            ("other profile hash", &|e| e.profile_sha256 = "0".repeat(64)),
        ];
        for (what, edit) in stale {
            row(what, same, same, edit, [Stale, Stale]);
        }
        let changed: [(&str, Edit<Profile>); 4] = [
            ("format", &fmt(Format::Bc3)),
            ("perceptual", &|p| {
                p.encoder.perceptual = !p.encoder.perceptual
            }),
            ("bc7_weights", &|p| {
                p.encoder.bc7_weights = Some(Bc7Weights::Alpha8)
            }),
            ("alpha.threshold", &|p| p.alpha.threshold = 100),
        ];
        for (what, rec) in changed {
            row(&format!("{what} changed"), same, rec, as_is, [Stale, Stale]);
        }
        row(
            "effort raised on BC7",
            &effort(Effort::Veryslow),
            &effort(Effort::Fast),
            as_is,
            [Stale, Stale],
        );
        row(
            "effort lowered on BC7",
            &effort(Effort::Fast),
            &effort(Effort::Veryslow),
            as_is,
            [TGT, TGT],
        );
        row(
            "effort raised on BC1",
            &|p| both(&fmt(Format::Bc1), &effort(Effort::Max), p),
            &effort(Effort::Fast),
            as_is,
            [Current, Current],
        );
        row(
            "BC6H slowest to max",
            &|p| both(&fmt(Format::Bc6hUf16), &effort(Effort::Max), p),
            &effort(Effort::Slowest),
            as_is,
            [Current, Current],
        );
        row(
            "BC6H fast to veryslow",
            &|p| both(&fmt(Format::Bc6hUf16), &effort(Effort::Veryslow), p),
            &effort(Effort::Fast),
            as_is,
            [Stale, Stale],
        );
        row(
            "BC6H veryslow to basic",
            &|p| both(&fmt(Format::Bc6hUf16), &effort(Effort::Basic), p),
            &effort(Effort::Veryslow),
            as_is,
            [TGT, TGT],
        );
        row(
            "bc1_search to fast on BC7",
            &search(Bc1Search::Fast),
            &search(Bc1Search::Default),
            as_is,
            [Current, Current],
        );
        row(
            "radius fast to max on BC4",
            &|p| both(&fmt(Format::Bc4), &radius(SearchRadius::Max), p),
            &radius(SearchRadius::Fast),
            as_is,
            [Stale, Stale],
        );
        row(
            "radius max to fast on BC4",
            &|p| both(&fmt(Format::Bc4), &radius(SearchRadius::Fast), p),
            &radius(SearchRadius::Max),
            as_is,
            [TGT, TGT],
        );
        row(
            "radius slowest to max on BC4",
            &|p| both(&fmt(Format::Bc4), &radius(SearchRadius::Max), p),
            &radius(SearchRadius::Slowest),
            as_is,
            [Stale, Stale],
        );
        row(
            "radius max to slowest on BC4",
            &|p| both(&fmt(Format::Bc4), &radius(SearchRadius::Slowest), p),
            &radius(SearchRadius::Max),
            as_is,
            [TGT, TGT],
        );
        row(
            "radius default to slowest on BC3",
            &|p| both(&fmt(Format::Bc3), &radius(SearchRadius::Slowest), p),
            &radius(SearchRadius::Default),
            as_is,
            [Stale, Stale],
        );
        row(
            "radius slowest to default on BC5",
            &|p| both(&fmt(Format::Bc5), &radius(SearchRadius::Default), p),
            &radius(SearchRadius::Slowest),
            as_is,
            [TGT, TGT],
        );
        row(
            "bc1_search fast to default on BC1",
            &|p| both(&fmt(Format::Bc1), &search(Bc1Search::Default), p),
            &search(Bc1Search::Fast),
            as_is,
            [Stale, Stale],
        );
        row(
            "bc1_search default to fast on BC1",
            &|p| both(&fmt(Format::Bc1), &search(Bc1Search::Fast), p),
            &search(Bc1Search::Default),
            as_is,
            [TGT, TGT],
        );
        row(
            "bc1_search raised and radius lowered on BC3",
            &|p| {
                both(&fmt(Format::Bc3), &search(Bc1Search::Default), p);
                radius(SearchRadius::Fast)(p)
            },
            &|p| both(&search(Bc1Search::Fast), &radius(SearchRadius::Max), p),
            as_is,
            [Stale, Stale],
        );
        row(
            "radius lowered and effort raised on BC3",
            &|p| {
                both(&fmt(Format::Bc3), &radius(SearchRadius::Fast), p);
                effort(Effort::Max)(p)
            },
            &|p| both(&radius(SearchRadius::Max), &effort(Effort::Fast), p),
            as_is,
            [TGT, TGT],
        );
        row(
            "bc7_weights unset to linear",
            &|p| p.encoder.bc7_weights = Some(Bc7Weights::Linear),
            &|p| p.encoder.bc7_weights = None,
            as_is,
            [Stale, Stale],
        );
        // The rows below would be a lower target if the record's profile were read as it claims.
        row(
            "raw profile a string",
            &effort(Effort::Fast),
            &effort(Effort::Veryslow),
            &|e| e.raw["profile"] = "cs-quality".into(),
            [Stale, Stale],
        );
        row(
            "a profile that parses but hashes otherwise",
            &effort(Effort::Fast),
            &effort(Effort::Veryslow),
            &|e| e.profile_sha256 = "0".repeat(64),
            [Stale, Stale],
        );
        // A 3.0.0 record has no `bc1_search` (it reads as `default`), and its hash is that of the profile without it.
        let v300: Edit<ManifestEntry> = &|e| {
            let p = &mut e.raw["profile"];
            p["encoder"].as_object_mut().unwrap().remove("bc1_search");
            let was: Profile = serde_json::from_value(p.clone()).unwrap();
            e.profile_sha256 = profile_sha256(&was).unwrap();
        };
        row(
            "3.0.0 record against fast on BC1",
            &|p| both(&fmt(Format::Bc1), &search(Bc1Search::Fast), p),
            &search(Bc1Search::Fast),
            v300,
            [TGT, TGT],
        );
        row(
            "3.0.0 record against fast on BC7",
            &search(Bc1Search::Fast),
            &search(Bc1Search::Fast),
            v300,
            [Current, Current],
        );
        row(
            "encode version changed and target lowered",
            &effort(Effort::Fast),
            &effort(Effort::Veryslow),
            &ev("0.9.0"),
            [BOTH, Stale],
        );
        // Note LH: through engine 3.0.0 a `max` record was the radius-10 search, today's `slowest`.
        let dated = |v: &'static str| move |e: &mut ManifestEntry| e.engine_version = v.into();
        let at = |f: Format, r: SearchRadius| {
            move |p: &mut Profile| {
                p.format = f;
                p.encoder.bc45_search_radius = r;
            }
        };
        row(
            "3.0.0 record at max against max on BC4",
            &at(Format::Bc4, SearchRadius::Max),
            &radius(SearchRadius::Max),
            &dated("3.0.0"),
            [Stale, Stale],
        );
        row(
            "3.0.0 record at max against slowest on BC4",
            &at(Format::Bc4, SearchRadius::Slowest),
            &radius(SearchRadius::Max),
            &dated("3.0.0"),
            [Current, Current],
        );
        row(
            "2.0.0 record at max against max on BC3",
            &at(Format::Bc3, SearchRadius::Max),
            &radius(SearchRadius::Max),
            &dated("2.0.0"),
            [Stale, Stale],
        );
        row(
            "3.0.0 record at max against max on BC7",
            &at(Format::Bc7, SearchRadius::Max),
            &radius(SearchRadius::Max),
            &dated("3.0.0"),
            [Current, Current],
        );
        // shortcut: a 3.0.1 record is the new `max`, even from a dev build that wrote radius 10.
        row(
            "3.0.1 record at max against max on BC5",
            &at(Format::Bc5, SearchRadius::Max),
            &radius(SearchRadius::Max),
            &dated(ENGINE_VERSION),
            [Current, Current],
        );
        row(
            "record without engine_version at max against max on BC4",
            &at(Format::Bc4, SearchRadius::Max),
            &radius(SearchRadius::Max),
            &dated(""),
            [Stale, Stale],
        );
        row(
            "record without engine_version at max against slowest on BC4",
            &at(Format::Bc4, SearchRadius::Slowest),
            &radius(SearchRadius::Max),
            &dated(""),
            [Current, Current],
        );
        row(
            "record without engine_version at max against max on BC7",
            &at(Format::Bc7, SearchRadius::Max),
            &radius(SearchRadius::Max),
            &dated(""),
            [Current, Current],
        );
        // BC45-R14: only a recorded `max` of unknown age reads as `slowest`.
        row(
            "record without engine_version at fast against slowest on BC4",
            &at(Format::Bc4, SearchRadius::Slowest),
            &radius(SearchRadius::Fast),
            &dated(""),
            [Stale, Stale],
        );
        for (what, job, e, [without, with]) in &rows {
            assert_eq!(
                currency(e, job, &enc, false),
                *without,
                "{what}, without ev"
            );
            assert_eq!(currency(e, job, &enc, true), *with, "{what}, with ev");
        }
        // BC45-R9, BC45-R14: the same record as the reader hands it over, with no `engine_version` key on disk.
        let mut bc4 = job0.clone();
        at(Format::Bc4, SearchRadius::Max)(&mut bc4.profile);
        let e = entry(&bc4, &radius(SearchRadius::Max));
        let line = serde_json::json!({
            "schema": e.schema,
            "output": e.output,
            "source_sha256": e.source_sha256,
            "sources": e.sources,
            "profile_sha256": e.profile_sha256,
            "encode_version": e.encode_version,
            "encoders": e.encoders,
            "preset": e.preset,
            "bc1_model": e.bc1_model,
            "output_stamp": e.output_stamp,
            "validation": e.raw["validation"],
            "profile": e.raw["profile"],
        });
        let root = d.path();
        std::fs::create_dir_all(root.join("_sdc")).unwrap();
        std::fs::write(
            root.join("_sdc").join("manifest.jsonl"),
            format!("{line}\n"),
        )
        .unwrap();
        let read = Manifest::at(root).read().unwrap();
        let key = crate::manifest::output_key(root, &bc4.output);
        let loaded = read.get(&key).expect("the record is read back");
        assert_eq!(loaded.engine_version, "");
        let mut slowest = bc4.clone();
        at(Format::Bc4, SearchRadius::Slowest)(&mut slowest.profile);
        for ev in [false, true] {
            assert_eq!(
                currency(loaded, &bc4, &enc, ev),
                Stale,
                "undated max against max, ev {ev}"
            );
            assert_eq!(
                currency(loaded, &slowest, &enc, ev),
                Current,
                "undated max against slowest, ev {ev}"
            );
        }
    }

    /// The `""` rows of `currency_follows_the_owners_rule` stand for what the reader hands `currency`.
    #[test]
    fn a_record_without_engine_version_reads_as_empty() {
        use crate::manifest::MANIFEST_SCHEMA;
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("_sdc")).unwrap();
        let lines = [
            serde_json::json!({ "schema": MANIFEST_SCHEMA, "output": "a.dds" }),
            serde_json::json!({ "schema": MANIFEST_SCHEMA, "output": "b.dds", "engine_version": 3 }),
        ];
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(d.path().join("_sdc").join("manifest.jsonl"), text).unwrap();
        let read = Manifest::at(d.path()).read().unwrap();
        assert_eq!(read.len(), 2);
        for key in ["a.dds", "b.dds"] {
            assert_eq!(read[key].engine_version, "", "{key}");
        }
    }

    #[test]
    fn record_age_reads_core_versions() {
        use RecordAge::{New, Old, Unknown};
        let last = MAX_WAS_SLOWEST_THROUGH;
        for v in ["3.0.0", "3.0.0-rc.1", "2.0.0", "1.2.0+x"] {
            assert_eq!(record_age(v, last), Old, "{v}");
        }
        for v in ["3.0.1", "10.0.0", ENGINE_VERSION] {
            assert_eq!(record_age(v, last), New, "{v}");
        }
        for v in ["", "3.0", "a.b.c", "3.0.0.1"] {
            assert_eq!(record_age(v, last), Unknown, "{v}");
        }
    }

    #[test]
    fn encode_current_compares_the_record_with_the_given_type_version() {
        let mk = |enc: Option<&str>, engine: &str| ManifestEntry {
            schema: crate::manifest::MANIFEST_SCHEMA,
            output: "x.dds".into(),
            source_sha256: String::new(),
            sources: vec![],
            profile_sha256: String::new(),
            engine_version: engine.into(),
            encode_version: enc.map(String::from),
            encoders: None,
            preset: None,
            bc1_model: None,
            output_stamp: None,
            copied_from: None,
            raw: serde_json::json!({}),
        };
        let e = mk(Some("2.0.0"), "2.1.0");
        assert!(encode_current(&e, "2.0.0")); // type unchanged in 2.1.0
        assert!(!encode_current(&e, "2.1.0")); // type bumped in 2.1.0
        for engine in ["0.9.0", "1.0.0", "1.1.0"] {
            assert!(!encode_current(&mk(None, engine), "2.0.0"), "{engine}");
        }
    }

    /// M1: with bytes to spare, the file cap alone holds the next admission until a file finishes.
    #[test]
    fn budget_caps_unfinished_files() {
        let b = Budget::new(u64::MAX, 2);
        b.acquire(1);
        b.acquire(1);
        let admitted = std::sync::atomic::AtomicBool::new(false);
        std::thread::scope(|s| {
            s.spawn(|| {
                b.acquire(1);
                admitted.store(true, Ordering::SeqCst);
            });
            std::thread::sleep(std::time::Duration::from_millis(100));
            assert!(
                !admitted.load(Ordering::SeqCst),
                "third file admitted past the cap"
            );
            b.release(1);
        });
        assert!(admitted.load(Ordering::SeqCst));
        assert_eq!(*lock(&b.used), (2, 2));
    }

    #[test]
    fn target_origin_follows_precedence() {
        let reg = PresetRegistry::builtin().unwrap();
        let chain = reg.chain("vanilla-quality").unwrap();
        let mut req = BatchRequest::new(vec![], "out".into(), Target::Cs);
        let cfg = |t| ConfigFile {
            target: Some(t),
            ..Default::default()
        };
        let (proj, user) = (cfg(Target::Vanilla), cfg(Target::Cs));
        assert_eq!(
            resolve_target(&req, Some(&proj), Some(&user), &chain),
            (Target::Cs, Provenance::User)
        );
        req.target = None;
        assert_eq!(
            resolve_target(&req, Some(&proj), Some(&user), &chain),
            (Target::Vanilla, Provenance::ProjectConfig)
        );
        assert_eq!(
            resolve_target(&req, None, Some(&user), &chain),
            (Target::Cs, Provenance::UserConfig)
        );
        assert_eq!(
            resolve_target(&req, None, None, &chain),
            (
                Target::Vanilla,
                Provenance::Preset("vanilla-quality".into())
            )
        );
        assert_eq!(
            resolve_target(&req, None, None, &[]),
            (Target::Cs, Provenance::Builtin)
        );
    }

    /// Fast re-plan Task 2: one `Prepared` planned twice gives the same plan and events, so `plan_sets` neither
    /// consumes nor changes what it borrows (the cloned `sets`).
    #[test]
    fn prepared_plans_twice_equal() {
        use crate::events::VecSink;
        use image::{ImageBuffer, Luma, Rgb};
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("in/textures/pbr");
        std::fs::create_dir_all(&dir).unwrap();
        ImageBuffer::from_fn(32, 32, |x, y| Rgb([(x * 7) as u8, (y * 5) as u8, 90]))
            .save(dir.join("iron.png"))
            .unwrap();
        for (name, v) in [("iron_roughness.png", 120u8), ("iron_ao.png", 200)] {
            ImageBuffer::from_fn(32, 32, |x, _| Luma([v.wrapping_add(x as u8)]))
                .save(dir.join(name))
                .unwrap();
        }
        let desc = crate::dds::DdsDesc {
            width: 4,
            height: 4,
            format: crate::config::Format::Rgba8,
            srgb: false,
            header: crate::dds::ResolvedHeader::Dx10,
        };
        let dds = crate::dds::dds_bytes(&desc, &[vec![0u8; 64]]).unwrap();
        std::fs::write(dir.join("plain.dds"), dds).unwrap();
        let mut req =
            BatchRequest::new(vec![d.path().join("in")], d.path().join("out"), Target::Cs);
        req.recursive = true;
        let engine = Engine::new().unwrap();
        let cache = AnalysisCache::default();
        let cancel = CancelToken::default();
        let rep = Reporter::new(Arc::new(VecSink::default()), "t");
        let p = engine.prepare(&req, &rep, &cancel).unwrap();
        let run = || {
            let sink = Arc::new(VecSink::default());
            let rep = Reporter::new(sink.clone(), "t");
            let plan = engine.plan_sets(&p, None, &rep, &cancel, &cache).unwrap().0;
            let events: Vec<Value> = sink
                .take()
                .into_iter()
                .map(|e| {
                    let mut v = serde_json::to_value(e).unwrap();
                    v.as_object_mut().unwrap().remove("ts");
                    v
                })
                .collect();
            (
                serde_json::to_value(&plan.jobs).unwrap(),
                format!("{:?}", plan.skipped),
                format!("{:?}", plan.failed),
                format!("{:?}", plan.helpers),
                events,
            )
        };
        let (a, b) = (run(), run());
        let jobs = a.0.as_array().unwrap();
        assert!(
            jobs.iter()
                .any(|j| j.get("pack").is_some_and(|v| !v.is_null())),
            "a packed job"
        );
        assert!(a.1.contains("InDdsSkipped"), "a DDS skip: {}", a.1);
        assert!(
            a.4.iter()
                .any(|e| e.to_string().contains("\"set_resolved\"")),
            "set_resolved events"
        );
        assert_eq!(a, b);
    }
}
