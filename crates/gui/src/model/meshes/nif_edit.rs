//! The NIF edit session (spec §3.4, §7, §8.1; plan A16): the source copy an edit applies to, its edit document and
//! undo stack, the patched bytes and their model, and the live `Parsed` preview in the Meshes 3D panel. No egui types.
use super::edit_doc::{self, DocOp, EditDoc, ExportRec, NifHistory, OpStatus, SourceId};
use super::export::{placement, read_back, DocBadge, ExportOutcome, GuardChoice, Placement};
use super::pg_entries::{
    candidates, crash_lines, pg_ignore, pg_stock, simulate_shape, PgFile, PGP_ENV_SCALE,
    PG_IGNORED, PG_STOCK_REVERT,
};
use super::worklist::{NifProblem, NifTab};
use super::{db_err, texture_sources, MeshesModel, ScanTarget};
use crate::model::viewport3d::{read_file, Clock, Content, FileSource};
use engine::dds::Kept;
use modscan::mo2::Instance;
use modscan::store::{PathCopy, ShapeView, KIND_BSA};
use nif::anim::{apply_anim, clip_tracks_in, free_tracks_in, Track};
use nif::edit::validate::{
    demote_source, new_errors, validate, validate_ops, Level, Target, Warning,
};
use nif::edit::{Anchor, Op, Outcome};
use nif::fields::{Field, Value};
use nif::keys::KeyData;
use nif::material::{MaterialState, SLSF2_TRUE_PBR};
use nif::model::{read_model, NifModel, ShapeModel};
use nif::props::LightingTrailer;
use render3d::{ParsedNif, ParsedShape, ShapeId};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// What the edit panel shows: the root's extra data or one shape (by block).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Root,
    Shape(u32),
}

/// The panel line when a controller's times are not finite (B-P33): no clock, no Time slider.
pub const ANIM_INVALID: &str = "This animation has invalid times: playback is off.";

/// What the preview animates (v2 B3): the free controller chains, or one manager clip by its sequence block (the
/// Controllers tab's Preview selector, B7). A clip pauses the free chains: they show the file value (E6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Playing {
    #[default]
    Free,
    Clip(u32),
}

/// Origin-name hints of tool-generated output (OD3): editable, with a warning.
pub const GENERATED_HINTS: [&str; 7] = [
    "dyndolod",
    "texgen",
    "xlodgen",
    "bodyslide",
    "grass",
    "synthesis",
    "nemesis",
];

pub struct NifEdit {
    pub doc: EditDoc,
    pub history: NifHistory,
    /// Where the working document (the draft) is saved: under the project (`edit_doc::draft_path`), else `autosave`.
    /// Empty with neither (a test's read-only session): nothing is saved.
    pub draft_path: PathBuf,
    /// The project the draft belongs to; None: the draft is the autosave.
    project: Option<PathBuf>,
    /// The record an export leaves in the export folder (`edit_doc::doc_path`), when a folder is chosen.
    pub record_path: Option<PathBuf>,
    /// The ops of the record at `record_path` when the session does not know it as its own (`record_fp` None) and it
    /// holds any; the export does not replace it unless they begin the session's ops (rulings E6-A, E6-F).
    foreign_record: Option<Vec<DocOp>>,
    /// Ruling E6-F: the fingerprint of the record this session knows as its own: read while it held nothing the
    /// session lacked (its ops empty or the first of the session's), or written by this session's export. An equal
    /// draft id is not ownership.
    record_fp: Option<String>,
    /// The record's (length, modified time) when last read, so a moved or changed record is read again while it blocks.
    record_meta: Option<(u64, Option<std::time::SystemTime>)>,
    /// Both empty without an export folder: export is blocked with `NO_EXPORT_FOLDER`.
    pub export_path: PathBuf,
    /// The export folder (empty without one).
    pub edit_mod: PathBuf,
    /// Note GD5 (c): the app-folder document of this NIF used while no project is open; removed by the first save
    /// into a project, but only while it holds the bytes this session loaded or wrote there (`plain_fp`, Note GD11).
    pub autosave: Option<PathBuf>,
    /// Ruling E6-F: the fingerprint of the draft at `draft_path` as this session last loaded or wrote it; None when it
    /// knows none of its own there (absent at open, or another document it never showed: a recovered autosave won
    /// over it, or a carry-over came in). The draft is replaced only while it holds these bytes (or is absent);
    /// otherwise it is moved aside first (`free_unseen`, Notes GD19, GD25).
    draft_fp: Option<String>,
    /// Ruling E6-F: with a project, the plain autosave's bytes this session recovered or carried in (it removes that
    /// file after its first project save only while it still holds them).
    plain_fp: Option<String>,
    /// The kept autosave (`edit_doc::kept_autosave_path`) this session wrote or loaded, with the fingerprint of those
    /// bytes (`kept_fp`); it is replaced or deleted (after the first project save) only while it holds them.
    kept_autosave: Option<PathBuf>,
    kept_fp: Option<String>,
    /// Ruling E6-D: the kept autosave of the draft's id that lost to the newer draft at open; the first draft save
    /// moves it to `<sha>.<id>.<secs>.toml` (`move_aside`, as `free_unseen` does), so its name is free and its bytes
    /// are kept.
    kept_older: Option<PathBuf>,
    /// `retarget_error` says no save worked when the session was to end (a project switch, or ruling E6-C's other
    /// ways out), not a record folder's; `switch_wording`: it was a project switch.
    switch_failed: bool,
    switch_wording: bool,
    /// The save before an end (`save_for_switch`) worked and no op changed since: the session may end (ruling E6-G:
    /// when not durable the saves are tried, and one that works allows the end).
    saved_to_leave: bool,
    /// A retry of the two saves succeeded: the model ends the session and opens the wanted project's.
    switch_saved: bool,
    /// Attempts of the project switch's two saves (the test of ruling r4-2).
    #[cfg(feature = "test-hooks")]
    switch_save_attempts: u32,
    /// Note GD11: lines for Messages from `open` (an autosave recovered, or left because the folder's is newer) and,
    /// Note GD19, from a save that moved an autosave aside; `MeshesModel` moves them to its `messages`.
    pub open_lines: Vec<String>,
    pub source: Arc<Vec<u8>>,
    pub source_copy: PathCopy,
    pub data_path: String,
    /// The source with the ops applied. Written only by `reapply` (and the test seam `set_patched`), which keep
    /// `read_err` and `cards` in step with it (Note GP8): never assign it elsewhere.
    pub patched: Arc<Vec<u8>>,
    /// Note GD2 (CQ1-CQ4): what the export writes, `patched` without its orphan blocks; rebuilt by `reapply`.
    pub out: Arc<Vec<u8>>,
    /// How many blocks `out` dropped; None when the cleanup could not run (`cleanup_skip` says why).
    pub compaction: Option<usize>,
    pub cleanup_skip: Option<String>,
    /// The source's block count: blocks past it are ones the edits appended.
    source_blocks: u32,
    /// A process-unique id of the current edit state, new on every apply, undo, redo, revert and reload (Note CU2):
    /// the refusal lines key on it, not on the `Arc` address, which an allocation can reuse.
    pub rev: u64,
    pub model: NifModel,
    pub outcomes: Vec<Outcome>,
    /// Note GD16: per op, the shader type in effect when it applies, for a trailer op (`Field::Trailer`); made by
    /// `reapply`, so its row keeps naming the field it changed after a later type change.
    trailer_types: Vec<Option<u32>>,
    pub warnings: Vec<Warning>,
    /// validate_ops over the document's ops (B4/B5 key errors), recomputed with the ops in `reapply`.
    pub op_warnings: Vec<Warning>,
    /// B-P41: `PGP_ENV_SCALE` Warnings from the cached In game simulation (never blocking, never a new simulation).
    pub pg_warnings: Vec<Warning>,
    /// Every controller's editable keys by (controller, clip), `nif::edit::keys_of`; rebuilt in `reapply`, never
    /// per frame (F23). B7 adds the clip entries.
    pub keys: BTreeMap<(u32, Option<u32>), Result<KeyData, String>>,
    pub source_warnings: Vec<Warning>,
    pub selection: Selection,
    pub solo: bool,
    pub check_exists: bool,
    /// (applied, conflicts) when the source hash differed from the document's.
    pub source_changed: Option<(usize, usize)>,
    /// OD3 warning text.
    pub generated: Option<String>,
    pub le_mesh: bool,
    /// The last failure (apply, save, undo text); `MeshesModel` moves it to its own `error`.
    pub error: Option<String>,
    /// Every enabled copy of the NIF (the store's order), for the placement check and the PGPatcher re-run note.
    pub copies: Vec<PathCopy>,
    /// The shape cards of the edited bytes (`shape_views` of `reapply`'s one scan of `patched`, Note GP8), rebuilt by
    /// every `reapply` (Note CZ1; was the scan's cards of `source_copy`, final review I1).
    pub cards: Vec<ShapeView>,
    /// Note GP8: `read_back`'s text for `patched` from the same scan; `export_block` reads it, never scans.
    read_err: Option<String>,
    /// Meshes UI T2: each shape's controllers line ("2: Emissive multiple, Alpha (clip)"), by shape block; rebuilt with
    /// `cards`.
    pub card_ctrls: HashMap<u32, String>,
    /// The placement check (spec §8.2); None without an MO2 instance.
    pub placement: Option<Placement>,
    /// Why "Export NIF" is disabled, from the in-memory state (`export_block`), kept current by every change.
    pub blocked: Option<String>,
    /// Note GP8: the `rev` `refresh_blocked` last ran for; `edit_checks` skips a check it already made.
    blocked_rev: u64,
    /// Note GD1: this edit's worklist rows, rebuilt with `blocked` (a new `Arc` only when they change, rule 9.1).
    pub problems: Arc<Vec<NifProblem>>,
    /// Note CS: what a shader type change also did to the flags, until the next change.
    pub notice: Option<String>,
    /// The last export's result line, and whether it is an error. While `partial` holds, an op change keeps it: it is
    /// the reason the session stays pending.
    pub export_note: Option<(String, bool)>,
    /// Ruling E6-I: an export wrote its NIF (the SHA-256 hex of those bytes) and not its record. Until an export
    /// writes the record, the session is pending whatever its ops are, and that NIF counts as this session's
    /// (`foreign_hash`). In memory only.
    partial: Option<String>,
    /// Ruling E6-J: the SHA-256 of the source the export record was built from (taken from the record, or set by an
    /// export); None without a record. A source change forces a re-export only when the source differs from it.
    record_source: Option<String>,
    /// Note CQ5: "Exported without cleanup: <reason>." after an export that wrote the patch uncompacted.
    pub cleanup_note: Option<String>,
    /// Export is blocked with this reason: the export folder's record did not load (Note GD12), or (`switch_failed`)
    /// a project switch could not save the edits anywhere and they are still open here.
    pub retarget_error: Option<String>,
    /// Note GD20: the failed export folder change behind `retarget_error`: (root, its record, that file's length and
    /// time). The same key is not tried again, so a bad folder is read once, not each frame.
    retarget_fail: Option<RetargetKey>,
    /// Note GD23: `doc.pending()` (it serialises and hashes the ops) keyed on (rev, op count, export record's hash).
    pending_cache: std::cell::RefCell<Option<PendingKey>>,
    /// Meshes UI T3: the last export wrote exactly these ops ("Export NIF again"); kept here so the panel never hashes
    /// the ops per frame.
    pub up_to_date: bool,
    /// Meshes UI T5b: the other weight file (`x_0.nif` ↔ `x_1.nif`) when the scan has it, set when the session opens.
    /// The game uses `_1` only at weight 100, so a heel height belongs in both; the editor edits one file at a time.
    pub weight_sibling: Option<String>,
    /// A process-unique id of this session (T5b review Important 1): UI state that belongs to one session keys on it.
    pub session: u64,
    /// `nif::edit::apply` failed: the patched bytes are stale.
    apply_err: Option<String>,
    /// `render3d::parse(&patched)`, once per change.
    parsed: Arc<ParsedNif>,
    /// What the preview animates.
    pub playing: Playing,
    /// The picked clip's name (Note CU1): a revert can move another clip onto its block.
    playing_name: Option<String>,
    /// Per animated shape: its parsed material and its tracks for `playing`. Rebuilt by `reapply` and
    /// `set_playing`, never per frame (F22, F23).
    anim: Vec<(ShapeId, MaterialState, Vec<Track>)>,
    /// The clock time whose materials the viewport shows; None after anything sent un-animated materials.
    anim_shown: Option<f32>,
    /// The last `edit_changed` store lookup failed: the next change looks every shape up again (Note GP9).
    lookup_failed: bool,
    /// The shapes the last `tick_anim` animated (D15: one that leaves gets its file material back).
    anim_last: Vec<ShapeId>,
    /// v2 B8 fix F1: per shape, the files that can patch it (keyed on its slots and the JSON generation) and its
    /// last simulation (keyed on its parsed material, the JSON generation and the scan). The NIF path is the
    /// session's own, so it needs no key.
    sim_cache: BTreeMap<ShapeId, SimCache>,
    /// `simulate_shape` calls so far (the cache's misses).
    #[cfg(feature = "test-hooks")]
    pub sim_runs: usize,
    /// Entries `shape_matches` tested to find candidates (fix round 2: the index keeps this far below a full scan).
    #[cfg(feature = "test-hooks")]
    pub match_tests: usize,
    /// v2 B8: In game (simulated) is showing: each shape's simulated material and its lines. None on Edited source.
    /// `preview_nif` draws these materials, so `materials()` and every `before` follow the shown side.
    pub in_game: Option<Vec<(ShapeId, MaterialState, Vec<String>)>>,
    /// Note GH1: on Edited source, the slot paths the simulation replaces on every shape naming them (empty In game
    /// and before the JSON lands). `MeshesModel::poll` lists the missing ones apart.
    pub pg_supplied: Vec<String>,
    /// A merged (drag) push not saved yet.
    unsaved: bool,
    /// The merge key of the drag whose ops are the last ones, and how many; a push with the same key and count
    /// replaces those ops.
    drag: Option<(String, usize)>,
}

/// One shape's cached In game work (fix F1): the candidate files for (slots, JSON generation) and the simulation
/// for (parsed material, JSON generation, scan revision).
#[derive(Default)]
struct SimCache {
    #[allow(clippy::type_complexity)]
    cands: Option<(Vec<nif::path::TexPath>, u64, Vec<(usize, Vec<usize>)>)>,
    #[allow(clippy::type_complexity)]
    out: Option<(MaterialState, u64, u64, (MaterialState, Vec<String>))>,
}

/// Under the toggle when `show_edit_preview` failed and a plain copy shows (fix F8).
pub const EDIT_HIDDEN: &str = "Your edit could not be shown. Switch to In game or Source to retry.";

/// The idle reason Export NIF is off: drawn neutral, not as a block (review Minor 5).
pub const NO_EDITS: &str = "No edits to export.";
pub const RERUN_PGPATCHER: &str = "Re-run PGPatcher so its output includes this edit.";
/// No export folder chosen: the edit works and autosaves, its export waits (Note GD5); FB1's reason, word for word.
pub const NO_EXPORT_FOLDER: &str = crate::model::review::NO_EXPORT_FOLDER;
/// Note GD2: why `export_nif_edits` skips a row with a file Tessalith did not write.
pub const NIF_UNANSWERED: &str =
    "the file on disk was not written by Tessalith: answer Keep or Overwrite on its row";
pub const NIF_KEPT: &str = "you kept the file on disk";
pub const NIF_CHANGED_SINCE: &str = "changed on disk since you answered";
/// Note GP10: the write found a file Tessalith did not write that the row had not (it changed in between).
pub const NIF_FOREIGN_AT_WRITE: &str =
    "the file on disk changed before the write: answer Keep or Overwrite on its row";
/// Note GD2 (was CQ5's under-button line): the export will write the patch without dropping orphan blocks.
pub const NO_CLEANUP: &str =
    "Exports without cleanup: the file has a block Tessalith does not understand";

/// Note GD2: one NIF with pending edits, as the Review lists it. `foreign`: SHA-256 of the file at the export path
/// when Tessalith did not write it (the row waits for an answer); `answer`: the user's, while that file still has
/// the hash it was given for; `compaction`: the orphan blocks the export drops (None when it cannot clean up).
#[derive(Debug, Clone, PartialEq)]
pub struct NifReviewRow {
    pub path: String,
    pub mod_name: String,
    pub blocked: Option<String>,
    pub foreign: Option<[u8; 32]>,
    pub answer: Option<GuardChoice>,
    pub compaction: Option<usize>,
}

/// `foreign_cache`'s key: the export path, the last export's hash, a partial export's hash (ruling E6-I) and the
/// file's (length, modified time).
pub(super) type ForeignKey = (
    PathBuf,
    Option<String>,
    Option<String>,
    Option<(u64, Option<std::time::SystemTime>)>,
);
pub(super) type ForeignCache = Option<(ForeignKey, Result<Option<[u8; 32]>, String>)>;

/// `NifEdit::pending_cache`: (rev, op count, the export record's ops hash) and the pending state for them.
type PendingKey = (u64, usize, Option<String>, bool);

/// `NifEdit::retarget_fail`'s key: the root, its edit document and the document's (length, modified time).
type RetargetKey = (
    PathBuf,
    PathBuf,
    Option<(u64, Option<std::time::SystemTime>)>,
);

/// The copy an edit applies to: the first that is neither PGPatcher output, our chosen edit mod, the 1.x default
/// edit mod (spec §8.1; Note GD28: `mods\sdc NIF Edits` holds Tessalith's own earlier output), nor a folder holding
/// our edit document for this NIF (Note GQ3).
pub fn pick_source<'a>(
    copies: &'a [PathCopy],
    edit_mod: &Path,
    data_path: &str,
) -> Option<&'a PathCopy> {
    let legacy = |root: &str| {
        Path::new(&root.replace('/', "\\"))
            .file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(edit_doc::EDIT_MOD))
    };
    // Note GQ3: a folder holding our edit document for this NIF is one of our own exports.
    // shortcut: a folder whose .sdc\nif-edits was deleted is not recognised as ours, upgrade if users report it
    let ours =
        |root: &str| edit_doc::doc_path(Path::new(root), data_path).is_ok_and(|p| p.exists());
    copies.iter().find(|c| {
        !c.derived && !same_dir(Path::new(&c.root), edit_mod) && !legacy(&c.root) && !ours(&c.root)
    })
}

pub fn generated_warning(origin: &str) -> Option<String> {
    let o = origin.to_ascii_lowercase();
    GENERATED_HINTS
        .iter()
        .any(|h| o.contains(h))
        .then(|| format!("{origin} is generated by a tool: its next run will overwrite this edit."))
}

const SLSF1_ENVIRONMENT_MAPPING: u32 = 1 << 7;
const SLSF1_MODEL_SPACE_NORMALS: u32 = 1 << 12;
const SLSF2_GLOW_MAP: u32 = 1 << 6;
const SLSF2_BACK_LIGHTING: u32 = 1 << 27;
/// The CS/PGPatcher TruePBR slot meanings, as the set editor's slot kinds name them.
const PBR_SLOTS: [&str; 9] = [
    "Diffuse",
    "Normal",
    "Emissive",
    "Height",
    "Unused",
    "RMAOS",
    "Coat / Fuzz",
    "Subsurface",
    "Unused",
];

/// The Shader tab's name of texture-set slot `slot` (spec §7, F§1.3).
pub fn edit_slot_label(shader_type: u32, slsf1: u32, slsf2: u32, slot: usize) -> &'static str {
    if slsf2 & SLSF2_TRUE_PBR != 0 {
        return PBR_SLOTS.get(slot).copied().unwrap_or("Unused");
    }
    match (slot, shader_type) {
        (0, _) => "Diffuse",
        (1, _) => "Normal",
        (2, 5) => "Skin",
        (2, 6) => "Hair",
        (2, 2) => "Glow",
        (2, _) if slsf2 & SLSF2_GLOW_MAP != 0 => "Glow",
        (2, _) => "Rim / back-light",
        (3, 4) => "Detail",
        (3, _) => "Height",
        (4, 16) => "Eye env cube",
        (5, 16) => "Eye env mask",
        (4, 1) => "Env cube",
        (5, 1) => "Env mask",
        (4, _) if slsf1 & SLSF1_ENVIRONMENT_MAPPING != 0 => "Env cube",
        (5, _) if slsf1 & SLSF1_ENVIRONMENT_MAPPING != 0 => "Env mask",
        (4, _) => "Env cube",
        (5, _) => "Env mask",
        (6, 11) => "Inner layer",
        (6, 4) => "Face tint",
        (6, _) => "Inner layer / tint",
        (7, _) if slsf2 & SLSF2_BACK_LIGHTING != 0 => "Back-light",
        (7, _) if slsf1 & SLSF1_MODEL_SPACE_NORMALS != 0 => "Model-space specular",
        (7, _) => "Back-light / specular",
        _ => "Unused",
    }
}

/// Note GH7: the name the form shows for texture-set slot `slot` (0-based, the NIF's index). A True PBR set uses the
/// set editor's card names for PGPatcher slots 3 and 7; every other slot keeps [`edit_slot_label`]'s own name (the set
/// editor names only the True PBR slots).
pub fn edit_slot_shown(shader_type: u32, slsf1: u32, slsf2: u32, slot: usize) -> &'static str {
    match slot {
        2 if slsf2 & SLSF2_TRUE_PBR != 0 => "Emissive / Skin",
        6 if slsf2 & SLSF2_TRUE_PBR != 0 => "Coat normal / Fuzz",
        _ => edit_slot_label(shader_type, slsf1, slsf2, slot),
    }
}

/// The four texture clamp modes in nif.xml's `TexClampMode` order (S is U, T is V).
pub const CLAMP_MODE_LABELS: [&str; 4] = [
    "Clamp both",
    "Clamp U, wrap V",
    "Wrap U, clamp V",
    "Wrap both",
];

/// A clamp mode in words; a value outside 0-3 reads "Other (n)".
pub fn clamp_mode_label(n: u32) -> String {
    match CLAMP_MODE_LABELS.get(n as usize) {
        Some(l) => (*l).to_string(),
        None => format!("Other ({n})"),
    }
}

/// A field in words, as the tabs name it (Meshes UI T3: no `Slsf1` or `UvOffset(0)` in the pending list).
pub fn field_label(f: Field) -> String {
    let axis = |n: u8| ["U", "V"].get(n as usize).copied().unwrap_or("?");
    let rgb = |n: u8| ["R", "G", "B"].get(n as usize).copied().unwrap_or("?");
    match f {
        Field::ShaderType => "Shader type".into(),
        Field::Slsf1 => "Shader flags 1".into(),
        Field::Slsf2 => "Shader flags 2".into(),
        Field::UvOffset(n) => format!("UV offset {}", axis(n)),
        Field::UvScale(n) => format!("UV scale {}", axis(n)),
        Field::EmissiveColor(n) => format!("Emissive colour {}", rgb(n)),
        Field::EmissiveMultiple => "Emissive multiple".into(),
        Field::ClampMode => "Texture wrap".into(),
        Field::Alpha => "Shader alpha".into(),
        Field::Refraction => "Refraction strength".into(),
        Field::Glossiness => "Glossiness".into(),
        Field::SpecularColor(n) => format!("Specular colour {}", rgb(n)),
        Field::SpecularStrength => "Specular strength".into(),
        Field::LightingEffect1 => "Lighting effect 1".into(),
        Field::LightingEffect2 => "Lighting effect 2".into(),
        // `field_name` names it from the shader type (Note GD1); this is the fallback for a type without names.
        Field::Trailer(n) => format!("Shader type value {}", n + 1),
        Field::AlphaFlags => "Alpha flags".into(),
        Field::AlphaThreshold => "Alpha threshold".into(),
        Field::CtrlFlags => "Controller flags".into(),
        Field::Frequency => "Frequency".into(),
        Field::Phase => "Phase".into(),
        Field::StartTime => "Start".into(),
        Field::StopTime => "Stop".into(),
        Field::Variable => "Controlled variable".into(),
        Field::InterpValue(n) => format!("Interpolator value {}", n + 1),
    }
}

/// Field `f` of block `block` in words: a trailer value by its name for the block's shader type (Note GD1).
pub fn field_name(m: &NifModel, block: u32, f: Field) -> String {
    let trailer = |n: u8| {
        let l = m.lighting.get(&block)?;
        l.lighting.params.trailer.names().get(n as usize).copied()
    };
    match f {
        Field::Trailer(n) => trailer(n).map_or_else(|| field_label(f), str::to_string),
        _ => field_label(f),
    }
}

/// A short name of what an op changes, for the pending list's and the worklist's rows. `m` is the model the op
/// applied to (its anchor's block is current after `reapply`).
pub fn op_label(op: &Op, m: &NifModel) -> String {
    let (what, at) = match op {
        Op::SetField { at, field, .. } | Op::SetBits { at, field, .. } => {
            (field_name(m, at.block, *field), at)
        }
        // Note GH7: slots count from 1 like the set editor.
        Op::SetTexturePath { at, slot, .. } => (format!("Texture slot {}", slot + 1), at),
        Op::SplitAlpha { at } => ("Split alpha".into(), at),
        Op::AddAlpha { at, .. } => ("Add alpha".into(), at),
        Op::RemoveAlpha { at } => ("Remove alpha".into(), at),
        Op::AddExtra { at, name, .. } => (format!("Add extra data {name}"), at),
        Op::EditExtra { at, .. } => ("Edit extra data".into(), at),
        Op::RemoveExtra { at } => ("Remove extra data".into(), at),
        Op::SetKeys { at, .. } => ("Edit keys".into(), at),
        Op::AddController { at, .. } => ("Add controller".into(), at),
        Op::RemoveController { at } => ("Remove controller".into(), at),
    };
    match &at.shape {
        Some(s) => format!("{s}: {what}"),
        None => what,
    }
}

/// Note GD16: for each trailer op, the shader type of its block when it applies: the last type change before it on
/// that block, else the type the first later change started from, else the block's type now. Note GD22: only changes
/// that applied count (one applies only from its `base`), so a change that conflicted names nothing.
fn trailer_types(ops: &[Op], outcomes: &[Outcome], m: &NifModel) -> Vec<Option<u32>> {
    let type_change = |block: u32, (o, out): (&Op, &Outcome)| match (o, out) {
        (_, Outcome::Conflict(_) | Outcome::Refused(_)) => None,
        (
            Op::SetField {
                at,
                field: Field::ShaderType,
                value: Value::U32(v),
                base: Value::U32(b),
            },
            _,
        ) if at.block == block => Some((*v, *b)),
        _ => None,
    };
    ops.iter()
        .enumerate()
        .map(|(i, op)| {
            let Op::SetField {
                at,
                field: Field::Trailer(_),
                ..
            } = op
            else {
                return None;
            };
            let b = at.block;
            ops[..i]
                .iter()
                .zip(outcomes)
                .rev()
                .find_map(|o| type_change(b, o).map(|(v, _)| v))
                .or_else(|| {
                    ops.iter()
                        .zip(outcomes)
                        .skip(i + 1)
                        .find_map(|o| type_change(b, o).map(|(_, base)| base))
                })
                .or_else(|| m.lighting.get(&b).map(|l| l.lighting.shader_type))
        })
        .collect()
}

impl NifEdit {
    /// [`op_label`] of op `i`, a trailer field named by the shader type in effect when it applied (Note GD16).
    pub fn op_label_at(&self, i: usize) -> String {
        let Some(d) = self.doc.ops.get(i) else {
            return String::new();
        };
        match (&d.op, self.trailer_types.get(i).copied().flatten()) {
            (
                Op::SetField {
                    at,
                    field: Field::Trailer(n),
                    ..
                },
                Some(t),
            ) => {
                let name = LightingTrailer::names_of_type(t)
                    .get(*n as usize)
                    .map_or_else(|| field_label(Field::Trailer(*n)), |s| s.to_string());
                match &at.shape {
                    Some(s) => format!("{s}: {name}"),
                    None => name,
                }
            }
            (op, _) => op_label(op, &self.model),
        }
    }
}

/// The undo step's name for an op (Note GD4): "Set shader type", "Remove controller".
pub fn op_name(op: &Op) -> String {
    match op {
        Op::SetField { field, .. } | Op::SetBits { field, .. } => {
            let f = field_label(*field);
            // "Shader type" reads "Set shader type"; an initialism ("UV offset U") keeps its case.
            let mut c = f.chars();
            match (c.next(), c.next()) {
                (Some(a), Some(b)) if b.is_lowercase() => {
                    format!("Set {}{}", a.to_lowercase(), &f[a.len_utf8()..])
                }
                _ => format!("Set {f}"),
            }
        }
        Op::SetTexturePath { .. } => "Set texture path".into(),
        Op::SplitAlpha { .. } => "Split alpha".into(),
        Op::AddAlpha { .. } => "Add alpha".into(),
        Op::RemoveAlpha { .. } => "Remove alpha".into(),
        Op::AddExtra { .. } => "Add extra data".into(),
        Op::EditExtra { .. } => "Edit extra data".into(),
        Op::RemoveExtra { .. } => "Remove extra data".into(),
        Op::SetKeys { .. } => "Edit keys".into(),
        Op::AddController { .. } => "Add controller".into(),
        Op::RemoveController { .. } => "Remove controller".into(),
    }
}

/// The shape's lighting shader's LSCF var-8 (Env map scale) controller, if its chain has one.
fn env_scale_ctrl(m: &NifModel, sm: &ShapeModel) -> Option<u32> {
    let l = m.lighting.get(&sm.shader?)?;
    l.prop.ctrls.iter().copied().find(|c| {
        m.ctrls.get(c).is_some_and(|c| {
            &*c.class == "BSLightingShaderPropertyFloatController" && c.var == Some(8)
        })
    })
}

fn anchor_mut(op: &mut Op) -> &mut Anchor {
    match op {
        Op::SetField { at, .. }
        | Op::SetBits { at, .. }
        | Op::SetTexturePath { at, .. }
        | Op::SplitAlpha { at }
        | Op::AddAlpha { at, .. }
        | Op::RemoveAlpha { at }
        | Op::AddExtra { at, .. }
        | Op::EditExtra { at, .. }
        | Op::RemoveExtra { at }
        | Op::AddController { at, .. }
        | Op::RemoveController { at }
        | Op::SetKeys { at, .. } => at,
    }
}

/// `op` (a later tick of the same drag as `old`) takes `old`'s base: the value the drag started from.
fn keep_base(op: &mut Op, old: &Op) {
    match (op, old) {
        (Op::SetField { base, .. }, Op::SetField { base: b, .. }) => *base = *b,
        (Op::SetBits { base, .. }, Op::SetBits { base: b, .. }) => *base = *b,
        (Op::SetTexturePath { base, .. }, Op::SetTexturePath { base: b, .. }) => base.clone_from(b),
        (Op::EditExtra { base, .. }, Op::EditExtra { base: b, .. }) => base.clone_from(b),
        (Op::SetKeys { base, .. }, Op::SetKeys { base: b, .. }) => base.clone_from(b),
        _ => {}
    }
}

/// What `pick_draft` decided.
struct Picked {
    /// The document that opens (None: the project has none and no autosave won).
    doc: Option<EditDoc>,
    /// GD11's lines for Messages.
    lines: Vec<String>,
    /// The project's draft as read (id checked) and the fingerprint of its bytes; None when there is none.
    draft: Option<(EditDoc, String)>,
    /// The draft is the document (it won, or nothing else did).
    draft_won: bool,
    /// The kept autosave of the draft's id won, with the fingerprint of the bytes read: the session holds it.
    kept: Option<(PathBuf, String)>,
    /// The kept autosave of the draft's id lost: the next save moves it aside (ruling E6-D).
    kept_older: Option<PathBuf>,
    /// The plain autosave won: the fingerprint of the bytes read.
    plain: Option<String>,
}

/// Note GD11, rulings E6-D and E6-H: the one recovery decision, for `open` in a project, for the live carry-over into
/// one (`NifEdit::retarget`) and for what is durable (`NifEdit::durable`, ruling E6-G). It reads the project's draft
/// at `draft_path` (Err when it does not load: it is never overwritten), then weighs the kept autosave of its id, then
/// the plain autosave `autosave`, each against the winner so far: the newest of the three wins, the earlier file at
/// equal times (the draft first, GD24). Reads only.
fn pick_draft(
    draft_path: &Path,
    autosave: Option<&Path>,
    data_path: &str,
) -> Result<Picked, String> {
    let draft = edit_doc::load_fp(draft_path)?.map(|(d, fp)| (checked_id(d), fp));
    let mut p = Picked {
        doc: draft.as_ref().map(|(d, _)| d.clone()),
        lines: vec![],
        draft_won: draft.is_some(),
        draft,
        kept: None,
        kept_older: None,
        plain: None,
    };
    let k = p
        .doc
        .as_ref()
        .and_then(|d| d.id.as_deref())
        .zip(autosave.and_then(autosave_dir))
        .map(|(i, dir)| edit_doc::kept_autosave_path(dir, data_path, i))
        .filter(|k| k.is_file());
    // ruling E6-H: the file whose document is the current winner; each next candidate is weighed against it
    let mut winner = draft_path.to_path_buf();
    if let Some(k) = k {
        let (s, l, r) = recover_from(p.doc.take(), &winner, &k, data_path);
        p.doc = s;
        p.lines.extend(l);
        match r {
            Recovery::Won(fp) => {
                p.kept = Some((k.clone(), fp));
                p.draft_won = false;
                winner = k;
            }
            Recovery::Older => p.kept_older = Some(k),
            Recovery::Stays => {}
        }
    }
    if let Some(a) = autosave.filter(|a| a.is_file()) {
        let (s, l, r) = recover_from(p.doc.take(), &winner, a, data_path);
        p.doc = s;
        p.lines.extend(l);
        if let Recovery::Won(fp) = r {
            p.plain = Some(fp);
            p.draft_won = false;
            // a kept autosave the plain one beat is moved aside at the next save, like one the draft beat
            if let Some((k, _)) = p.kept.take() {
                p.kept_older = Some(k);
            }
        }
    }
    Ok(p)
}

/// What `recover_from` did with an autosave.
#[derive(PartialEq)]
enum Recovery {
    /// It is the document: the fingerprint of the bytes read.
    Won(String),
    /// The draft is newer (or as new): the autosave stays as it is.
    Older,
    /// It is gone or does not load: it stays for the user.
    Stays,
}

/// Note GD11: with a project open, an autosave `a` of the NIF (the plain one, or the kept one of the draft's id) that
/// is newer than `winner`, the file the current document `saved` came from (the project's draft, or a kept autosave
/// that beat it; ruling E6-H), or there is no document, is offered: it is the document, and the session owns it (its
/// first save in the project removes it). An older one stays, named in Messages. The draft's id stays the
/// document's. Returns (document, Messages lines, what happened to `a`).
fn recover_from(
    saved: Option<EditDoc>,
    winner: &Path,
    a: &Path,
    data_path: &str,
) -> (Option<EditDoc>, Vec<String>, Recovery) {
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    if saved.is_some() && mtime(a) <= mtime(winner) {
        let line = format!(
            "Older edits of {data_path} stay autosaved in {}.",
            a.display()
        );
        return (saved, vec![line], Recovery::Older);
    }
    match edit_doc::load_fp(a) {
        Ok(Some((mine, fp))) => {
            let mut mine = checked_id(mine);
            let line = match &saved {
                None => format!(
                    "Recovered the edits of {data_path} autosaved in {}.",
                    a.display()
                ),
                Some(_) => format!(
                    "The newer edits of {data_path} autosaved in {} opened instead of {}'s saved edits; those are \
                     kept aside at the next save.",
                    a.display(),
                    winner.display()
                ),
            };
            if let Some(id) = saved.and_then(|s| s.id) {
                mine.id = Some(id);
            }
            (Some(mine), vec![line], Recovery::Won(fp))
        }
        Ok(None) => (saved, vec![], Recovery::Stays),
        // never deleted or overwritten: it stays for the user, and the project's draft opens
        Err(e) => {
            let line = format!("The autosave {} does not load and stays: {e}", a.display());
            (saved, vec![line], Recovery::Stays)
        }
    }
}

/// Ruling E6-E: a document read from disk keeps its id only when it has `new_id`'s form (`edit_doc::valid_id`);
/// otherwise it has none (a new one is made in memory), so a crafted id never reaches `kept_autosave_path`.
fn checked_id(mut d: EditDoc) -> EditDoc {
    d.id = d.id.filter(|i| edit_doc::valid_id(i));
    d
}

/// The same ops in the same order; their statuses are worked out again on every apply, so they are not compared.
fn same_ops(a: &[DocOp], b: &[DocOp]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.op == y.op)
}

/// `theirs` are the first ops of `ours` (empty included): replacing `theirs` with `ours` erases no op.
fn starts(theirs: &[DocOp], ours: &[DocOp]) -> bool {
    theirs.len() <= ours.len() && same_ops(theirs, &ours[..theirs.len()])
}

/// A file's (length, modified time); None when it cannot be read.
fn file_meta(p: &Path) -> Option<(u64, Option<std::time::SystemTime>)> {
    std::fs::metadata(p)
        .ok()
        .map(|m| (m.len(), m.modified().ok()))
}

/// The folder the plain autosave lives in (`<dir>\nif-edits\<sha>.toml`), where kept autosaves go too.
fn autosave_dir(autosave: &Path) -> Option<&Path> {
    autosave.parent()?.parent()
}

impl NifEdit {
    /// Opens `source`, the bytes of `source_copy`, at `data_path`. The draft is read from the project's
    /// (`edit_doc::draft_path`), or from `autosave` with no project; neither: read-only with an in-memory document.
    /// `export_root` None: export is blocked; with one, the export state (`doc.export`) is the record's there. A saved
    /// document is loaded (Err when it is malformed or newer: it is never overwritten); when its source hash differs,
    /// its ops are re-anchored on the new source and `source_changed` counts the outcomes.
    pub fn open(
        source: Vec<u8>,
        source_copy: PathCopy,
        data_path: &str,
        project: Option<&Path>,
        export_root: Option<&Path>,
        autosave: Option<&Path>,
    ) -> Result<NifEdit, String> {
        edit_doc::win(data_path)?; // read-only too: the session never opens on such a path
        let (scan, model) = read_source(&source)?;
        let id = source_id(&source_copy, data_path, &source);
        let (edit_mod, record_path, export_path) = match export_root {
            Some(m) => (
                m.to_path_buf(),
                Some(edit_doc::doc_path(m, data_path)?),
                edit_doc::export_path(m, data_path)?,
            ),
            None => (PathBuf::new(), None, PathBuf::new()),
        };
        let draft_path = match project {
            Some(p) => edit_doc::draft_path(p, data_path)?,
            None => autosave.map(Path::to_path_buf).unwrap_or_default(),
        };
        // Note GD11: with a project, a newer kept autosave of the draft's id (a switch that could not save into the
        // project) or else a newer plain autosave is offered (`pick_draft`); with none the draft is the autosave.
        // Ruling E6-F: the fingerprints of what the session took as its own.
        let mut open_lines = vec![];
        let (mut kept_autosave, mut kept_fp, mut kept_older, mut plain_fp) =
            (None, None, None, None);
        let (saved, draft_fp) = match (project, draft_path.as_os_str().is_empty()) {
            (_, true) => (None, None),
            (None, false) => match edit_doc::load_fp(&draft_path)? {
                Some((d, fp)) => (Some(checked_id(d)), Some(fp)),
                None => (None, None),
            },
            (Some(_), false) => {
                let p = pick_draft(&draft_path, autosave, data_path)?;
                open_lines = p.lines;
                if let Some((k, fp)) = p.kept {
                    (kept_autosave, kept_fp) = (Some(k), Some(fp));
                }
                kept_older = p.kept_older;
                plain_fp = p.plain;
                // a draft the session does not show is not its own: the next save moves it aside (GD25)
                let draft_fp = p.draft.filter(|_| p.draft_won).map(|(_, fp)| fp);
                (p.doc, draft_fp)
            }
        };
        let recovered = kept_autosave.is_some() || plain_fp.is_some();
        let mut doc = saved.unwrap_or_else(|| EditDoc::new(id.clone()));
        // the id every draft save writes (`write_draft` writes `doc`); a draft without one gets it with its next edit
        if doc.id.is_none() {
            doc.id = Some(edit_doc::new_id(data_path));
        }
        // the record wins: what was exported to the chosen folder, and from which source (`take_record`, below)
        let record = match &record_path {
            Some(p) => edit_doc::load_fp(p)?,
            None => None,
        };
        if doc.dropped_clip_ops > 0 {
            open_lines.push(format!(
                "{data_path}: {} clip edit(s) dropped: NIF Edit no longer edits clips.",
                doc.dropped_clip_ops
            ));
        }
        let changed = doc.source.sha256 != id.sha256;
        if changed {
            doc.source = id; // saved with the next change; the statuses carry the conflicts
        }
        let mut e = NifEdit {
            doc,
            history: NifHistory::default(),
            draft_path,
            project: project.map(Path::to_path_buf),
            record_path,
            foreign_record: None,
            record_fp: None,
            record_meta: None,
            export_path,
            edit_mod,
            autosave: autosave.map(Path::to_path_buf),
            draft_fp,
            plain_fp,
            kept_autosave,
            kept_fp,
            kept_older,
            switch_failed: false,
            switch_wording: false,
            saved_to_leave: false,
            switch_saved: false,
            #[cfg(feature = "test-hooks")]
            switch_save_attempts: 0,
            open_lines,
            source_blocks: block_count(&source),
            source: Arc::new(source),
            data_path: data_path.into(),
            patched: Arc::new(vec![]),
            out: Arc::new(vec![]),
            compaction: None,
            cleanup_skip: None,
            rev: 0,
            source_warnings: validate(&model, None),
            model,
            outcomes: vec![],
            trailer_types: vec![],
            warnings: vec![],
            op_warnings: vec![],
            pg_warnings: vec![],
            keys: BTreeMap::new(),
            selection: Selection::Root,
            solo: false,
            check_exists: false,
            source_changed: None,
            generated: generated_warning(&source_copy.origin),
            source_copy,
            le_mesh: scan.le_mesh,
            error: None,
            copies: vec![],
            cards: vec![],
            read_err: None,
            card_ctrls: HashMap::new(),
            placement: None,
            blocked: None,
            blocked_rev: 0,
            problems: Arc::default(),
            notice: None,
            export_note: None,
            partial: None,
            record_source: None,
            cleanup_note: None,
            retarget_error: None,
            retarget_fail: None,
            pending_cache: Default::default(),
            up_to_date: false,
            weight_sibling: None,
            session: next_key(),
            apply_err: None,
            parsed: Arc::default(),
            playing: Playing::Free,
            playing_name: None,
            anim: vec![],
            anim_shown: None,
            lookup_failed: false,
            anim_last: vec![],
            sim_cache: BTreeMap::new(),
            #[cfg(feature = "test-hooks")]
            sim_runs: 0,
            #[cfg(feature = "test-hooks")]
            match_tests: 0,
            in_game: None,
            pg_supplied: vec![],
            // a recovered autosave is saved into the folder with the next save or close; an op re-anchored at open
            // makes the session unsaved too (below, ruling E6-J), so Export saves the draft first
            unsaved: recovered,
            drag: None,
        };
        if e.record_path.is_some() {
            e.take_record(record);
        }
        // Applying re-anchors each op on the current source: `nif::edit::rebase` and the patched bytes in one pass.
        let read = e.doc.ops.clone();
        e.reapply();
        if !same_ops(&read, &e.doc.ops) {
            e.unsaved = true;
        }
        if changed {
            e.count_changed();
        }
        // Ruling E6-A: open writes nothing; a forced re-export (GQ1) is worked out again at each open
        Ok(e)
    }

    /// Takes the export folder's record (`edit_doc::load_fp` of `record_path`; None: never exported there): its export
    /// entry becomes the session's, kept in memory only (ruling E6-A). A record exported from another source than the
    /// current one makes the edit pending (Note GQ1's sentinel). Ruling E6-F: a record holding nothing the session
    /// lacks (its ops empty or the first of the session's) is known as the session's own by its fingerprint; any
    /// other is remembered (`foreign_record`) so the export never replaces it. An equal draft id is not ownership.
    fn take_record(&mut self, record: Option<(EditDoc, String)>) {
        self.record_meta = self.record_path.as_deref().and_then(file_meta);
        let Some((r, fp)) = record else {
            self.doc.export = ExportRec::default();
            self.foreign_record = None;
            self.record_fp = None;
            self.record_source = None;
            return;
        };
        self.record_source = Some(r.source.sha256.clone());
        let stale = r.export.ops_hash.is_some() && r.source.sha256 != self.doc.source.sha256;
        let known = starts(&r.ops, &self.doc.ops);
        self.record_fp = known.then_some(fp);
        self.foreign_record = (!known).then(|| r.ops.clone());
        self.doc.export = r.export;
        if stale {
            self.doc.export.ops_hash = Some(String::new());
        }
    }

    /// Ruling E6-F: a block from the record is read again when the record's file changed (moved away, replaced): the
    /// app's per-frame `set_nif_export_root` calls this, which stats the file only while a block holds.
    pub(super) fn recheck_record(&mut self) {
        let Some(p) = self
            .record_path
            .clone()
            .filter(|_| self.foreign_record.is_some())
        else {
            return;
        };
        if file_meta(&p) == self.record_meta {
            return;
        }
        match edit_doc::load_fp(&p) {
            Ok(r) => self.take_record(r),
            Err(e) => self.error = Some(e),
        }
        self.up_to_date = !self.pending() && self.doc.export.sha256.is_some();
        self.refresh_blocked();
    }

    /// Ruling E6-F at the write: the record at `record_path` may be replaced when it is absent, holds exactly the
    /// bytes this session knows as its own (`record_fp`), or holds nothing the session's ops lack. Else the reason.
    fn record_write_block(&self) -> Option<String> {
        let p = self.record_path.as_deref()?;
        if self.record_fp.is_some() && edit_doc::is_known(p, self.record_fp.as_deref()) {
            return None;
        }
        match edit_doc::load(p) {
            Ok(None) => None,
            Ok(Some(r)) => self.record_block(Some(&r.ops)),
            Err(e) => Some(e),
        }
    }

    /// Ruling E6-A: why the export may not replace the record in the export folder: it holds edits that are not this
    /// session's (`foreign_record`) and that the session's ops do not start with, so the export would lose them. A
    /// record whose ops begin the session's (an export of this edit before a carry-over, then reopened) loses nothing.
    /// Task 9 moves the other documents; until then the export waits.
    fn record_block(&self, foreign: Option<&[DocOp]>) -> Option<String> {
        let theirs = foreign?;
        (!starts(theirs, &self.doc.ops)).then(|| {
            format!(
                "{} holds other edits of this NIF (from another project or an earlier Tessalith); an export would \
                 replace them. Move that file out of the folder to keep them, or choose another export folder.",
                self.record_path.as_deref().unwrap_or(Path::new("")).display()
            )
        })
    }

    fn count_changed(&mut self) {
        let n = self
            .doc
            .ops
            .iter()
            .filter(|d| d.status == OpStatus::Conflict)
            .count();
        self.source_changed = Some((self.doc.ops.len() - n, n));
        // the exported file was built from another source than the current one: force a re-export (write_out sets a
        // real hash again). Ruling E6-J: the record's source decides, not the draft's.
        if self.doc.export.ops_hash.is_some()
            && self.record_source.as_deref() != Some(self.doc.source.sha256.as_str())
        {
            self.doc.export.ops_hash = Some(String::new());
        }
    }

    /// Writes `record` (the document as exported, with its new export entry) to the export folder; only `write_out`
    /// calls it (ruling E6-A). Ok with no folder.
    fn write_record(&self, record: &EditDoc) -> Result<Option<String>, String> {
        match &self.record_path {
            Some(p) => edit_doc::save(p, record).map(Some),
            None => Ok(None),
        }
    }

    /// The source copy's bytes changed on disk (export step 1): the ops are re-anchored on them and the statuses
    /// updated and saved in the draft. The export folder is not written (ruling E6-A): the forced re-export (GQ1) is
    /// in memory, and the next open works it out again from the record's source.
    pub fn reload_source(&mut self, source: Vec<u8>) -> Result<(), String> {
        let (scan, model) = read_source(&source)?;
        self.doc.source = source_id(&self.source_copy, &self.data_path, &source);
        self.source_warnings = validate(&model, None);
        self.le_mesh = scan.le_mesh;
        self.source_blocks = block_count(&source);
        self.source = Arc::new(source);
        self.reapply();
        self.count_changed();
        self.ops_changed(true);
        Ok(())
    }

    /// Why the current state cannot be exported, from memory alone (export steps 2-4): refused ops first (A16 keeps
    /// them as conflicts), then conflicts, new validation errors and bytes that do not read back.
    /// Note GD13: the session holds live work: ops, an undo or redo step, or an export to put back (every edit undone
    /// after an export) or, Note GD21, an unsaved state (a recovered autosave, even one with no ops). Only an
    /// untouched session may be replaced by a folder's saved document.
    pub fn touched(&self) -> bool {
        self.unsaved
            || !self.doc.ops.is_empty()
            || self.history.can_undo()
            || self.history.can_redo()
            || self.pending()
    }

    /// `doc.pending()`, computed once per change (Note GD23): every op change makes a new `rev`, and an export or a
    /// folder change a new export record. Ruling E6-I: a partial export is pending whatever the ops are.
    pub fn pending(&self) -> bool {
        if self.partial.is_some() {
            return true;
        }
        let key = (self.rev, self.doc.ops.len());
        let mut c = self.pending_cache.borrow_mut();
        if let Some((rev, n, h, p)) = &*c {
            if (*rev, *n) == key && *h == self.doc.export.ops_hash {
                return *p;
            }
        }
        let p = self.doc.pending();
        *c = Some((key.0, key.1, self.doc.export.ops_hash.clone(), p));
        p
    }

    pub fn export_block(&self) -> Option<String> {
        // the project switch's block (`switch_failed`) and an export folder's bad record share this field
        if let Some(why) = &self.retarget_error {
            return Some(why.clone());
        }
        // Note GH5: nothing edited is the idle state with or without a folder, and wins over `apply_err` too (a reloaded
        // source that stops parsing with no ops reads idle; `error` still says why).
        if self.doc.ops.is_empty() && self.doc.export.sha256.is_none() {
            return Some(NO_EDITS.into());
        }
        if self.edit_mod.as_os_str().is_empty() {
            return Some(NO_EXPORT_FOLDER.into());
        }
        if let Some(why) = self.record_block(self.foreign_record.as_deref()) {
            return Some(why);
        }
        if let Some(e) = &self.apply_err {
            return Some(e.clone());
        }
        let refused = self
            .outcomes
            .iter()
            .filter(|o| matches!(o, Outcome::Refused(_)))
            .count();
        if refused > 0 {
            let s = if refused == 1 { "edit" } else { "edits" };
            return Some(format!("{refused} {s} cannot be applied."));
        }
        if self.doc.has_conflicts()
            || self
                .outcomes
                .iter()
                .any(|o| matches!(o, Outcome::Conflict(_)))
        {
            return Some("Resolve the conflicts first.".into());
        }
        if let Some(w) = new_errors(&self.source_warnings, &self.warnings).first() {
            return Some(w.text.clone());
        }
        // Note GD8: a Warn (bad clip timing) holds back only what it names
        if let Some(w) = self.op_warnings.iter().find(|w| w.level == Level::Error) {
            return Some(w.text.clone());
        }
        self.read_err.clone()
    }

    /// Note GD33: a reason the export is blocked that no worklist row says: a failed folder change, an apply error, or
    /// bytes that do not read back. Refused ops, conflicts and errors have their own rows, so `export_block`'s
    /// summaries of them are never a row.
    pub fn uncovered_block(&self) -> Option<String> {
        self.retarget_error
            .clone()
            .or_else(|| self.record_block(self.foreign_record.as_deref()))
            .or_else(|| self.apply_err.clone())
            .or_else(|| self.read_err.clone())
    }

    /// Recomputes `blocked` after the ops or the warnings changed.
    pub fn refresh_blocked(&mut self) {
        #[cfg(feature = "test-hooks")]
        CHECKS.with(|n| n.set(n.get() + 1));
        self.blocked_rev = self.rev;
        self.blocked = self.export_block();
        self.refresh_problems();
    }

    /// The worklist rows (Note GD1), replaced only when they differ, so a cache keyed on the `Arc` holds.
    fn refresh_problems(&mut self) {
        let p = super::worklist::problems(self);
        if *self.problems != p {
            self.problems = Arc::new(p);
        }
    }

    /// Export steps 1-6 (spec §8.3) for Review's Export (Note GD2). Nothing is written when a check fails; a file
    /// Tessalith did not write is replaced only when `overwrite`, the SHA-256 its Overwrite answer was given for, is
    /// the hash of the bytes on disk now.
    pub(super) fn write_out(&mut self, overwrite: Option<[u8; 32]>) -> ExportOutcome {
        if self.edit_mod.as_os_str().is_empty() {
            return ExportOutcome::Blocked(NO_EXPORT_FOLDER.into());
        }
        let c = &self.source_copy;
        let now = match read_file(&FileSource::from_store(c.kind, &c.root, &self.data_path)) {
            Ok(b) => b,
            Err(e) => return ExportOutcome::Blocked(e),
        };
        if edit_doc::sha256_hex(&now) != self.doc.source.sha256 {
            if let Err(e) = self.reload_source(now) {
                return ExportOutcome::Blocked(e);
            }
        }
        self.refresh_blocked();
        if let Some(why) = self.blocked.clone() {
            return ExportOutcome::Blocked(why);
        }
        // Note CQ: the file drops the blocks our edits appended that nothing reaches; on any doubt it is the patch
        // (`reapply` made both, Note GD2)
        let (out, skipped) = (self.out.to_vec(), self.cleanup_skip.clone());
        let path = self.export_path.clone();
        match foreign_hash(
            &path,
            &out,
            self.doc.export.sha256.as_deref(),
            self.partial.as_deref(),
        ) {
            Err(e) => return ExportOutcome::Blocked(e),
            Ok(Some(h)) if overwrite != Some(h) => return ExportOutcome::Foreign,
            Ok(_) => {}
        }
        // Rulings E6-A, E6-F: the record there now (read again: it may have come or changed since it was taken) is
        // replaced only when it is absent, the session's known bytes, or holds nothing this session lacks
        if let Some(why) = self.record_write_block() {
            return ExportOutcome::Blocked(why);
        }
        // the draft first, when a drag was not flushed: the NIF is written from what the draft holds, and not at all
        // when the draft cannot be saved (ruling E6-E)
        if self.unsaved {
            self.save();
            if self.unsaved {
                let why = self.error.take().unwrap_or_default();
                return ExportOutcome::Blocked(format!(
                    "the edits could not be saved, so nothing was exported: {why}"
                ));
            }
        }
        if let Err(e) = engine::dds::write_atomic(&path, &out) {
            return ExportOutcome::Blocked(format!("Cannot write {}: {e}", path.display()));
        }
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let export = ExportRec {
            sha256: Some(edit_doc::sha256_hex(&out)),
            at: Some(at),
            ops_hash: Some(self.doc.ops_hash()),
        };
        // Ruling E6-E: the export counts once its record is saved; until then the state stays (the row pending)
        let record = EditDoc {
            export: export.clone(),
            ..self.doc.clone()
        };
        let fp = match self.write_record(&record) {
            Ok(fp) => fp,
            Err(e) => {
                // ruling E6-I: the NIF in place is this session's and not recorded: pending until repaired
                self.partial = export.sha256.clone();
                return ExportOutcome::Blocked(format!(
                    "{} is in place, but its record could not be saved ({e}); the edit stays pending: export it again.",
                    path.display()
                ));
            }
        };
        self.doc.export = export;
        self.record_source = Some(self.doc.source.sha256.clone());
        self.partial = None;
        self.up_to_date = true;
        self.foreign_record = None;
        self.record_fp = fp;
        self.record_meta = self.record_path.as_deref().and_then(file_meta);
        self.cleanup_note = skipped.map(|why| format!("Exported without cleanup: {why}."));
        ExportOutcome::Written {
            rerun_pgpatcher: self.copies.iter().any(|c| c.derived),
        }
    }

    /// Applies the ops to the source: patched bytes, outcomes (each `ReAnchored` block written back into its op),
    /// statuses (a conflict or a refusal blocks export), the model, the warnings and the parsed preview.
    fn reapply(&mut self) {
        self.rev = next_key();
        self.notice = None;
        let a = match nif::edit::apply(&self.source, &self.doc.ops()) {
            Ok(a) => a,
            Err(e) => {
                self.error = Some(e.to_string());
                self.apply_err = Some(e.to_string());
                self.read_err = None; // `apply_err` says why
                self.up_to_date = false; // review Minor 3: the stale bytes are not what the last export wrote
                self.refresh_blocked();
                return;
            }
        };
        self.apply_err = None;
        // the last export's line no longer describes these edits, except a partial export's (ruling E6-I)
        if self.partial.is_none() {
            self.export_note = None;
        }
        self.cleanup_note = None;
        for (d, o) in self.doc.ops.iter_mut().zip(&a.outcomes) {
            if let Outcome::ReAnchored { block } = o {
                anchor_mut(&mut d.op).block = *block;
            }
            d.status = match o {
                Outcome::Conflict(_) | Outcome::Refused(_) => OpStatus::Conflict,
                Outcome::Applied | Outcome::ReAnchored { .. } => OpStatus::Applied,
            };
        }
        self.patched = Arc::new(a.bytes);
        // Note GD2: the export's bytes once per change, so the Review row can count what the cleanup drops
        match nif::edit::compact(&self.patched, self.source_blocks) {
            Ok(out) => {
                let n = block_count(&self.patched).saturating_sub(block_count(&out));
                self.compaction = Some(n as usize);
                self.out = Arc::new(out);
                self.cleanup_skip = None;
            }
            Err(why) => {
                self.out = self.patched.clone();
                self.compaction = None;
                self.cleanup_skip = Some(why);
            }
        }
        self.outcomes = a.outcomes;
        self.up_to_date = !self.pending() && self.doc.export.sha256.is_some();
        match read_model(&self.patched) {
            Ok(m) => self.model = m,
            Err(e) => self.error = Some(e.to_string()),
        }
        // After the ReAnchored write-back: each op's `at.block` is the block its warning targets.
        let ops = self.doc.ops();
        self.trailer_types = trailer_types(&ops, &self.outcomes, &self.model);
        self.scan_patched(); // once per change, never per frame (rule 9.1)
        self.card_ctrls = card_ctrls(&self.model);
        self.warnings = validate(&self.model, None);
        demote_source(&self.source_warnings, &mut self.warnings);
        self.op_warnings = validate_ops(&ops);
        // B-P41: one header parse per change; one per controller cost 266 ms a drag tick on a 608-controller NIF.
        let header = nif::header::Header::parse(&self.patched).map_err(|e| e.to_string());
        self.keys =
            self.model
                .ctrls
                .keys()
                .map(|&c| {
                    let k = header.as_ref().map_err(Clone::clone).and_then(|h| {
                        nif::edit::keys_of_in(h, &self.patched, &self.model, c, None)
                    });
                    ((c, None), k)
                })
                .collect();
        // v2 B7: the keys of each clip a managed controller is driven by (once per change, one bound map per sequence).
        if let Ok(h) = &header {
            for (c, seqs) in nif::anim::clips_by_ctrl(&self.model, h) {
                if !self.model.ctrls.contains_key(&c) || !self.model.managed(c) {
                    continue;
                }
                for seq in seqs {
                    let k = nif::edit::keys_of_in(h, &self.patched, &self.model, c, Some(seq));
                    self.keys.insert((c, Some(seq)), k);
                }
            }
        }
        self.parsed = Arc::new(render3d::parse(&self.patched));
        // A clip the edits or a source reload removed: back to the free chains (with the last sequence gone, the
        // Preview selector is gone too). Another name at its block is another clip a revert moved there (Note CU1).
        if let Playing::Clip(seq) = self.playing {
            let name = &self.playing_name;
            if !self
                .model
                .sequences
                .iter()
                .any(|s| s.block == seq && s.name.as_deref() == name.as_deref())
            {
                self.playing = Playing::Free;
            }
        }
        self.anim = anim_tracks(&self.model, &self.patched, &self.parsed, self.playing);
        self.anim_shown = None;
        self.refresh_pg_warnings();
        self.refresh_blocked();
    }

    /// Note GP8: the one scan of `patched`: its shape cards, as the scan store would give them for these bytes
    /// (`modscan::store::shape_views`, Note CZ1), and whether it reads back.
    fn scan_patched(&mut self) {
        // Silent cards on purpose (review Minor 4): they fall back to titles, and `read_err` blocks Export NIF.
        (self.cards, self.read_err) = match read_back(&self.patched) {
            Ok(s) => (modscan::store::shape_views(&s), None),
            Err(e) => (vec![], Some(e)),
        };
    }

    /// Test seam: `patched` replaced as `reapply` would leave it, for bytes no edit can make (Note GP8).
    #[cfg(feature = "test-hooks")]
    pub fn set_patched(&mut self, bytes: Vec<u8>) {
        self.patched = Arc::new(bytes);
        self.scan_patched();
        self.refresh_blocked();
    }

    /// B-P41: a Warning under each LSCF var-8 controller of a non-PBR shape that the last cached In game simulation
    /// (for the shape's current slots) made PBR. PG_IGNORE shapes are skipped, as the simulation skips them.
    fn refresh_pg_warnings(&mut self) {
        let m = &self.model;
        self.pg_warnings = self
            .parsed
            .shapes
            .iter()
            .filter(|s| s.material.slsf2 & SLSF2_TRUE_PBR == 0)
            .filter_map(|s| {
                let sm = m.shape(s.id.0).filter(|sm| !pg_ignore(m, sm))?;
                let block = env_scale_ctrl(m, sm)?;
                let (input, _, _, (out, _)) = self.sim_cache.get(&s.id)?.out.as_ref()?;
                (input.slots == s.material.slots && out.slsf2 & SLSF2_TRUE_PBR != 0).then(|| {
                    Warning {
                        level: Level::Warn,
                        target: Target::Block { block },
                        text: PGP_ENV_SCALE.into(),
                        fix: None,
                    }
                })
            })
            .collect();
    }

    fn save(&mut self) {
        // The session could not be saved when it was to end (`MeshesModel::may_end_edit`): each save tries the same
        // two again (the draft, then the kept autosave); a success tells the model (`switch_saved`), which finishes a
        // waiting project switch.
        if self.switch_failed {
            self.unsaved = true;
            match self.save_for_switch() {
                Ok(()) => {
                    self.switch_saved = true;
                    self.switch_failed = false;
                    self.retarget_error = None;
                    self.retarget_fail = None;
                    self.refresh_blocked();
                }
                Err(why) => self.set_switch_failed(&why, self.switch_wording),
            }
            return;
        }
        if self.draft_path.as_os_str().is_empty() {
            self.unsaved = false;
            return;
        }
        // Note GD27: unsaved until the write succeeds, so a failed save keeps the session touched
        if let Err(e) = self.save_draft() {
            self.error = Some(e);
        }
    }

    /// Ruling E6-F: the draft's write, never over bytes this session does not know as its own there (`draft_fp`):
    /// those are moved aside first (`free_unseen`), and nothing is written when that fails.
    fn save_draft(&mut self) -> Result<(), String> {
        let path = self.draft_path.clone();
        let what = match self.project {
            Some(_) => "The edit document",
            None => "The autosave",
        };
        if !self.free_unseen(&path, self.draft_fp.clone(), what) {
            return Err(self.error.take().unwrap_or_default());
        }
        let fp = self.write_draft(&path)?;
        self.draft_fp = Some(fp);
        self.draft_saved();
        Ok(())
    }

    /// The draft's own write: into the project (never re-creating a project folder that is gone), else the autosave.
    /// Ok(the fingerprint written).
    fn write_draft(&self, path: &Path) -> Result<String, String> {
        match &self.project {
            Some(p) => edit_doc::save_in(p, path, &self.doc),
            None => edit_doc::save(path, &self.doc),
        }
    }

    /// After the draft was written: the session is saved, the draft carries its id, and what the project draft makes
    /// redundant is removed: the plain autosave this session recovered or carried in (Note GD5 (c), GD11) and the
    /// kept autosave it wrote or loaded, each only while it holds the bytes the session knows (ruling E6-F): a file
    /// replaced since is not the session's to delete.
    fn draft_saved(&mut self) {
        self.unsaved = false;
        if self.project.is_none() {
            return;
        }
        // ruling E6-D: the older kept autosave goes to `<sha>.<id>.<secs>.toml`; a failed move is tried by the next save
        if let Some(k) = self.kept_older.take().filter(|k| k.is_file()) {
            if !self.move_aside(&k, "The kept autosave") {
                self.kept_older = Some(k);
            }
        }
        let plain = self.autosave.clone().zip(self.plain_fp.take());
        let kept = self.kept_autosave.take().zip(self.kept_fp.take());
        for (f, fp) in plain.into_iter().chain(kept) {
            if !edit_doc::is_known(&f, Some(&fp)) {
                continue;
            }
            if let Err(e) = std::fs::remove_file(&f) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    self.error = Some(format!("{}: {e}", f.display()));
                }
            }
        }
    }

    /// The save before the session ends (a project switch, another NIF, Close edit): the draft in its own project,
    /// and when that fails (the folder is gone, renamed or not writable) the kept autosave keyed on the draft's id;
    /// with no project, the plain autosave (its draft) alone. Err(reason) when no save worked. Counted by
    /// `switch_save_attempts`.
    fn save_for_switch(&mut self) -> Result<(), String> {
        #[cfg(feature = "test-hooks")]
        {
            self.switch_save_attempts += 1;
        }
        let draft_err = match self.save_draft() {
            Ok(()) => {
                self.saved_to_leave = true;
                return Ok(());
            }
            Err(e) => e,
        };
        if self.project.is_none() {
            return Err(draft_err); // the kept autosave would be beside it, in the folder that just failed
        }
        match self.save_kept(&draft_err) {
            Ok(()) => {
                self.unsaved = false;
                // ruling E6-G: a kept autosave the next open would not find (the draft gone, or without its id)
                // still holds the ops; the session may leave until its next change
                self.saved_to_leave = true;
                Ok(())
            }
            Err(kept_err) => Err(format!("{draft_err}; the autosave failed too: {kept_err}")),
        }
    }

    /// Writes the kept autosave `<sha>.<id>.toml` beside the plain autosave, never over bytes this session did not
    /// write or load there (`kept_fp`; another file is moved aside first, Note GD19, ruling E6-F), and says so in
    /// Messages.
    fn save_kept(&mut self, draft_err: &str) -> Result<(), String> {
        let (Some(dir), Some(id)) = (
            self.autosave.as_deref().and_then(autosave_dir),
            self.doc.id.as_deref(),
        ) else {
            return Err("there is no autosave folder".into());
        };
        let k = edit_doc::kept_autosave_path(dir, &self.data_path, id);
        let known = self
            .kept_fp
            .clone()
            .filter(|_| self.kept_autosave.as_deref() == Some(k.as_path()));
        if !self.free_unseen(&k, known, "The kept autosave") {
            return Err(self.error.take().unwrap_or_default());
        }
        // what is left at the name is this session's own bytes (replaced), or nothing (published, never replacing)
        let fp = match k.is_file() {
            true => edit_doc::save(&k, &self.doc)?,
            false => edit_doc::save_new(&k, &self.doc)?,
        };
        self.kept_autosave = Some(k.clone());
        self.kept_fp = Some(fp);
        let project = self.project.as_deref().unwrap_or(Path::new(""));
        self.open_lines.push(format!(
            "Could not save the edits of {} to {}: {draft_err}. They are kept in {}.",
            edit_doc::win(&self.data_path).unwrap_or_else(|_| self.data_path.clone()),
            project.display(),
            k.display()
        ));
        Ok(())
    }

    /// The session was to end but no save worked: the edits stay open here, export is blocked with the reason and the
    /// footer shows it (ruling r3-3 for a project switch, E6-C for another NIF, Close edit or the Edit toggle).
    pub(super) fn set_switch_failed(&mut self, why: &str, switching: bool) {
        let path = edit_doc::win(&self.data_path).unwrap_or_else(|_| self.data_path.clone());
        let line = match switching {
            true => format!(
                "Could not save the edits of {path} while changing the project: {why}. They are still open here. \
                 Make a change to try again, or open the project again."
            ),
            false => format!(
                "Could not save the edits of {path}: {why}. They are still open here. Make a change to try again, or \
                 undo back to what was last saved to close this NIF."
            ),
        };
        self.switch_failed = true;
        self.switch_wording = switching;
        self.unsaved = true;
        if self.retarget_error.as_ref() != Some(&line) {
            self.retarget_error = Some(line);
            self.refresh_blocked();
        }
    }

    /// Back in the session's own project after a failed switch (ruling r4-5): the draft is saved there and the
    /// session goes on as it was. False (the failed state stays) when that save fails too.
    pub(super) fn save_back_into_own_project(&mut self) -> bool {
        if self.save_draft().is_err() {
            return false;
        }
        self.switch_failed = false;
        self.retarget_error = None;
        self.retarget_fail = None; // the export folder's record is read again
        self.refresh_blocked();
        true
    }

    /// Ruling E6-G (E6-C1 made exact): the session may end with nothing written, because its current ops are what the
    /// next open would load: with a project, `pick_draft`'s decision over the draft, the kept autosave of its id and
    /// the plain autosave as they are now; with none, the plain autosave. No winner and no ops count as equal; a
    /// winner that does not read or parse does not. Undo and redo history are not ops, so they never hold a session
    /// back, and a draft is never written only for its id (E6-B). An untouched session is equal without a read; a
    /// read-only one (no draft path) has nowhere to save. Reads only.
    pub(super) fn durable(&self) -> bool {
        if !self.touched() || self.draft_path.as_os_str().is_empty() {
            return true;
        }
        let next = match self.project {
            Some(_) => pick_draft(&self.draft_path, self.autosave.as_deref(), &self.data_path)
                .map(|p| p.doc),
            None => edit_doc::load(&self.draft_path),
        };
        match next {
            Ok(Some(d)) => same_ops(&d.ops, &self.doc.ops),
            Ok(None) => self.doc.ops.is_empty(),
            Err(_) => false,
        }
    }

    /// Ends a drag without saving it: what is left to save is `MeshesModel::may_end_edit`'s to decide.
    pub(super) fn end_drag(&mut self) {
        self.drag = None;
        self.history.break_merge();
    }

    /// After `switch_saved`: the draft (not a kept autosave) holds the edits, so the session can go on in its project.
    pub(super) fn saved_in_draft(&self) -> bool {
        !self.unsaved && self.kept_autosave.is_none()
    }

    /// Notes GD19, GD25, ruling E6-F: before this session writes `path`, a file there that is not as it knows it
    /// (`known`: the fingerprint of the bytes it last loaded or wrote there; other bytes, an unreadable file, or a
    /// file where it knew none) is moved aside to `<stem>.<unix seconds>.toml` (named in Messages), never overwritten.
    /// False (write nothing) when the move fails.
    fn free_unseen(&mut self, path: &Path, known: Option<String>, what: &str) -> bool {
        // a folder at the name is not moved: the write fails on it and destroys nothing
        path.is_dir() || edit_doc::is_known(path, known.as_deref()) || self.move_aside(path, what)
    }

    /// Moves the file `path` to `<stem>.<unix seconds>.toml` beside it, naming it in the session's lines. False
    /// (nothing written) when that fails; the reason is in `error`.
    fn move_aside(&mut self, path: &Path, what: &str) -> bool {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        // Note GD26: the name is reserved with create_new (another instance's create of it fails), then renamed over
        let mut s = secs;
        let to = loop {
            let to = path.with_file_name(format!("{stem}.{s}.toml"));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&to)
            {
                Ok(_) => break to,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => s += 1,
                Err(e) => {
                    self.error = Some(format!("{}: {e}", to.display()));
                    return false;
                }
            }
        };
        match std::fs::rename(path, &to) {
            Ok(()) => {
                self.open_lines.push(format!(
                    "{what} {} held edits of {} that this session did not open: kept as {}.",
                    path.display(),
                    self.data_path,
                    to.display()
                ));
                true
            }
            Err(e) => {
                let _ = std::fs::remove_file(&to); // the empty reservation only
                self.error = Some(format!("{}: {e}", path.display()));
                false
            }
        }
    }

    /// Note GD5 (c), GD11: a project is opened while this edit is touched and its draft is still the autosave, which
    /// the caller has made durable (`MeshesModel::may_end_edit`), so its time is the live edit's. Ruling E6-D: the
    /// decision `open` makes (`pick_draft`) runs against the project's draft and the kept autosave of its id. When the
    /// autosave wins, the document stays in memory and moves to the project's draft (`edit_doc::draft_path`), taking
    /// the draft's id (a new one when the project has none); the caller saves it there, and a draft the project
    /// already holds is kept aside by that save (Notes GD19, GD25). Ok(None) when the project's draft or kept autosave
    /// wins: nothing moved, and the caller opens the NIF in the project, where `open` makes the same decision (the
    /// live edit stays in the autosave). Err (nothing moved, the reason) when the project is not a folder or its
    /// draft does not load, so it is never overwritten.
    pub fn retarget(&mut self, project: &Path) -> Result<Option<Vec<String>>, String> {
        if !project.is_dir() {
            return Err(format!("{} no longer exists", project.display()));
        }
        let draft = edit_doc::draft_path(project, &self.data_path)?;
        let picked = pick_draft(&draft, self.autosave.as_deref(), &self.data_path)?;
        let Some(plain_fp) = picked.plain else {
            return Ok(None);
        };
        let theirs = picked.draft.map(|(d, _)| d);
        self.kept_older = picked.kept_older;
        self.doc.id = Some(
            theirs
                .as_ref()
                .and_then(|t| t.id.clone())
                .unwrap_or_else(|| edit_doc::new_id(&self.data_path)),
        );
        let mut lines = vec![];
        if theirs.is_some() {
            lines.push(format!(
                "Project opened: the open edit of {} goes to {}; the edits saved there are kept aside at the next \
                 save.",
                self.data_path,
                draft.display()
            ));
        }
        // Note GD25, ruling E6-F: the destination's edits were never loaded, so none of the draft's bytes are the
        // session's (the first save moves them aside); the autosave's are, so the first save into the project
        // removes it while it still holds them
        self.draft_fp = None;
        self.plain_fp = Some(plain_fp);
        (self.project, self.draft_path) = (Some(project.to_path_buf()), draft);
        self.drag = None;
        self.history.break_merge();
        self.unsaved = true;
        Ok(Some(lines))
    }

    /// The export folder changed: the session keeps its draft and takes the new folder's record (`export`); nothing
    /// is written. Err (nothing moved, the reason as the row shows it) when the record does not load, so it is never
    /// overwritten, or when `root` holds the source copy (Note HS12).
    pub fn set_record_root(&mut self, root: Option<&Path>) -> Result<(), String> {
        // Note HS12: an export there would replace the source copy (pick_source skips that folder as well)
        if let Some(r) = root.filter(|r| same_dir(Path::new(&self.source_copy.root), r)) {
            return Err(format!(
                "{} holds the mesh this edit applies to ({}); an export there would replace it. Choose another \
                 export folder.",
                r.display(),
                self.source_copy.origin
            ));
        }
        let cannot = |r: &Path, e: String| {
            format!("could not open the edit document in {}: {e}", r.display())
        };
        let (edit_mod, record_path, export_path, record) = match root {
            Some(r) => {
                let p = edit_doc::doc_path(r, &self.data_path).map_err(|e| cannot(r, e))?;
                let export_path =
                    edit_doc::export_path(r, &self.data_path).map_err(|e| cannot(r, e))?;
                let record = edit_doc::load_fp(&p).map_err(|e| cannot(r, e))?;
                (r.to_path_buf(), Some(p), export_path, record)
            }
            None => (PathBuf::new(), None, PathBuf::new(), None),
        };
        (self.edit_mod, self.record_path, self.export_path) = (edit_mod, record_path, export_path);
        // the folder's export state, a forced re-export when its record names another source (in memory, E6-A)
        // ruling E6-I: a partial export belongs to the folder it was made in; another folder has its own record
        self.partial = None;
        self.take_record(record);
        self.up_to_date = !self.pending() && self.doc.export.sha256.is_some();
        self.export_note = None;
        self.refresh_blocked();
        Ok(())
    }
    /// Appends `ops` as one undo step. A push with the same merge key as the previous one (the next tick of a
    /// drag) replaces the drag's ops instead, each keeping the `base` of the drag's op with its merge key: a drag
    /// is one op (or one op per colour channel), plus the controller's start / stop while they follow the keys
    /// (B-P44), which come and go during the drag, and the SplitAlpha that started it (Note FA10).
    pub fn push(&mut self, ops: Vec<Op>, merge: Option<String>) {
        let merging = merge.is_some();
        let n = ops.len();
        let tail = self.drag_ops(merge.as_deref().unwrap_or_default()).len();
        let same_drag = merging && tail > 0;
        // Note FA10: a drag that started with a SplitAlpha (a shared alpha) keeps it; later ticks no longer see the
        // alpha as shared and push only the field op.
        let lead = same_drag
            && matches!(
                self.doc.ops[self.doc.ops.len() - tail].op,
                Op::SplitAlpha { .. }
            )
            && !matches!(ops.first(), Some(Op::SplitAlpha { .. }));
        self.drag = merge.clone().map(|k| (k, n + usize::from(lead)));
        let label = ops.first().map(op_name).unwrap_or_default();
        self.history.record(self.doc.to_text(), &label, merge);
        let mut old: Vec<DocOp> = match same_drag {
            true => self.doc.ops.split_off(self.doc.ops.len() - tail),
            false => vec![],
        };
        if lead {
            self.doc.ops.push(old.remove(0));
        }
        let key = |op: &Op| nif::edit::merge_key(op);
        for (i, mut op) in ops.into_iter().enumerate() {
            let same = old.get(i).filter(|d| key(&d.op) == key(&op));
            if let Some(d) = same.or_else(|| old.iter().find(|d| key(&d.op) == key(&op))) {
                keep_base(&mut op, &d.op);
            }
            self.doc.ops.push(DocOp {
                op,
                status: OpStatus::Applied,
            });
        }
        self.reapply();
        self.ops_changed(!merging); // a drag is saved when it ends (`flush`)
    }

    /// Ruling E6-B, the one way a change of the ops reaches disk (push, undo, redo, revert, a resolved conflict, a
    /// source reload): the session is unsaved BEFORE the save is tried, and only a successful write of the draft (or
    /// of a kept autosave in a failed switch) clears it, so a failed save never looks saved. `now` false (a drag
    /// tick) leaves the save to `flush`, except after a failed switch, where every change is the next try.
    fn ops_changed(&mut self, now: bool) {
        self.unsaved = true;
        self.saved_to_leave = false;
        if now || self.switch_failed {
            self.save();
        }
    }

    /// The ops of the drag with merge key `merge` while it runs (empty otherwise): their bases are the values from
    /// before the drag.
    pub fn drag_ops(&self, merge: &str) -> &[DocOp] {
        match &self.drag {
            Some((k, n)) if k == merge && self.doc.ops.len() >= *n => {
                &self.doc.ops[self.doc.ops.len() - n..]
            }
            _ => &[],
        }
    }

    /// Ends a drag: the next merged push is a new undo step, and an unsaved drag is saved.
    pub fn flush(&mut self) {
        self.drag = None;
        self.history.break_merge();
        if self.unsaved {
            self.save();
        }
    }

    fn restore(&mut self, text: Option<String>) -> bool {
        self.drag = None;
        let Some(t) = text else { return false };
        match EditDoc::from_text(&t) {
            // Undo changes the ops, not what is on disk: the export record and the source identity stay current.
            Ok(mut d) => {
                d.export = std::mem::take(&mut self.doc.export);
                d.source = self.doc.source.clone();
                d.id = self.doc.id.clone();
                self.doc = d;
            }
            Err(e) => {
                self.error = Some(e);
                return false;
            }
        }
        self.reapply();
        self.ops_changed(true);
        true
    }

    pub fn undo(&mut self) -> bool {
        let t = self.history.undo(self.doc.to_text());
        self.restore(t)
    }

    pub fn redo(&mut self) -> bool {
        let t = self.history.redo(self.doc.to_text());
        self.restore(t)
    }

    /// Drops op `i` (one undo step).
    pub fn revert(&mut self, i: usize) {
        if i >= self.doc.ops.len() {
            return;
        }
        self.drag = None;
        let label = format!("Revert: {}", op_name(&self.doc.ops[i].op));
        self.history.record(self.doc.to_text(), &label, None);
        self.doc.ops.remove(i);
        self.reapply();
        self.ops_changed(true);
    }

    /// A conflict resolved for my value: its base becomes the value the source has now.
    pub fn keep_mine(&mut self, i: usize) {
        let ops = match nif::edit::keep_mine(&self.source, &self.doc.ops(), i) {
            Ok(o) => o,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        self.drag = None;
        self.history.record(self.doc.to_text(), "Keep mine", None);
        for (d, op) in self.doc.ops.iter_mut().zip(ops) {
            d.op = op;
        }
        self.reapply();
        self.ops_changed(true);
    }

    /// A conflict resolved for the source: the op is dropped (`nif::edit::take_source`).
    pub fn take_source(&mut self, i: usize) {
        self.revert(i);
    }

    /// The parsed patched NIF as the preview draws it: In game, each shape's simulated material (v2 B8); with Solo,
    /// every shape but the selected one is hidden.
    fn preview_nif(&self) -> ParsedNif {
        let mut p = (*self.parsed).clone();
        if let Some(rows) = &self.in_game {
            for s in &mut p.shapes {
                if let Some((_, m, _)) = rows.iter().find(|(id, _, _)| *id == s.id) {
                    s.material = m.clone();
                }
            }
        }
        if self.solo {
            for s in &mut p.shapes {
                if self.selection != Selection::Shape(s.id.0) {
                    s.material.hidden = true;
                }
            }
        }
        p
    }

    /// v2 B8 (closes R11): each shape's material as PGPatcher would leave the edited NIF (`pg_entries::simulate_shape`
    /// over `files`, the JSON of read `gen`), with its lines. Shapes without a texture set keep their material and get no
    /// line; PG_IGNORE ones keep it with `PG_IGNORED`; with a crashing JSON loaded both get its lines first (GE15). Those two checks run every time; only the simulation is cached
    /// (fix F1, keyed on the parsed material, `gen` and the scan `rev`). From the parsed edit (not `preview_nif`).
    pub fn in_game_materials(
        &mut self,
        files: &[PgFile],
        gen: u64,
        rev: u64,
        exists: &dyn Fn(&str) -> bool,
    ) -> Vec<(ShapeId, MaterialState, Vec<String>)> {
        let nif_path = self.data_path.replace('/', "\\");
        let parsed = self.parsed.clone();
        let mut rows = Vec::with_capacity(parsed.shapes.len());
        // GE15: the whole run stops, so every shape gets the crash lines, ahead of the eligibility exits below.
        let crashes = crash_lines(files);
        for s in &parsed.shapes {
            let shape = self.model.shape(s.id.0);
            let lit = shape
                .and_then(|sm| sm.shader)
                .and_then(|b| self.model.lighting.get(&b))
                .is_some_and(|l| l.texture_set.is_some());
            if !lit {
                rows.push((s.id, s.material.clone(), crashes.clone()));
                continue;
            }
            if shape.is_some_and(|sm| pg_ignore(&self.model, sm)) {
                let mut notes = crashes.clone();
                notes.push(PG_IGNORED.to_string());
                rows.push((s.id, s.material.clone(), notes));
                continue;
            }
            // B-P41: outside the cache, whose key (the material) does not change when a controller is added.
            let env = shape
                .and_then(|sm| env_scale_ctrl(&self.model, sm))
                .is_some()
                && s.material.slsf2 & SLSF2_TRUE_PBR == 0;
            let c = self.sim_cache.entry(s.id).or_default();
            let hit = c
                .out
                .as_ref()
                .filter(|(m, g, r, _)| *m == s.material && *g == gen && *r == rev);
            let (m, mut notes) = match hit {
                Some((_, _, _, out)) => out.clone(),
                None => {
                    let cands = match &c.cands {
                        Some((slots, g, v)) if *slots == s.material.slots && *g == gen => v.clone(),
                        _ => {
                            #[cfg_attr(not(feature = "test-hooks"), allow(unused_variables))]
                            let (v, tested) = candidates(files, &nif_path, &s.material.slots);
                            #[cfg(feature = "test-hooks")]
                            {
                                self.match_tests += tested;
                            }
                            c.cands = Some((s.material.slots.clone(), gen, v.clone()));
                            v
                        }
                    };
                    let out = simulate_shape(files, &cands, &nif_path, &s.material, exists);
                    c.out = Some((s.material.clone(), gen, rev, out.clone()));
                    #[cfg(feature = "test-hooks")]
                    {
                        self.sim_runs += 1;
                    }
                    out
                }
            };
            if env && m.slsf2 & SLSF2_TRUE_PBR != 0 {
                notes.push(PGP_ENV_SCALE.to_string());
            }
            // GE6 (R3): badge only, outside the cache like the env check; the revert is not simulated.
            // GE16: a crashing run never reverts.
            if crashes.is_empty() && shape.is_some_and(|sm| pg_stock(&self.model, sm)) {
                notes.insert(0, PG_STOCK_REVERT.to_string());
            }
            rows.push((s.id, m, notes));
        }
        self.refresh_pg_warnings();
        self.refresh_problems();
        rows
    }

    /// Note GH1: the edited slot paths that `rows` (the simulation) replaces with another path, minus any path a
    /// shape or slot keeps as it is (that one is still loaded as named).
    pub(super) fn supplied_by(
        &self,
        rows: &[(ShapeId, MaterialState, Vec<String>)],
    ) -> Vec<String> {
        let (mut replaced, mut kept) = (BTreeSet::new(), BTreeSet::new());
        for s in &self.parsed.shapes {
            let sim = rows.iter().find(|(id, ..)| *id == s.id).map(|(_, m, _)| m);
            for (i, t) in s.material.slots.iter().enumerate() {
                if t.norm.is_empty() {
                    continue;
                }
                let by = sim
                    .and_then(|m| m.slots.get(i))
                    .map_or("", |q| q.norm.as_str());
                if by.is_empty() || by == t.norm {
                    kept.insert(t.norm.as_str());
                } else {
                    replaced.insert(t.norm.as_str());
                }
            }
        }
        replaced.difference(&kept).map(|p| p.to_string()).collect()
    }

    /// In game before the JSON lands, or when it could not be read: every shape as edited, no line.
    pub fn plain_rows(&self) -> Vec<(ShapeId, MaterialState, Vec<String>)> {
        self.parsed
            .shapes
            .iter()
            .map(|s| (s.id, s.material.clone(), vec![]))
            .collect()
    }

    pub fn materials(&self) -> Vec<(ShapeId, MaterialState)> {
        self.preview_nif()
            .shapes
            .into_iter()
            .map(|s| (s.id, s.material))
            .collect()
    }

    /// Switches what the preview animates (B7's clip selector) and rebuilds the track cache.
    pub fn set_playing(&mut self, p: Playing) {
        self.playing = p;
        self.playing_name = match p {
            Playing::Clip(b) => self
                .model
                .sequences
                .iter()
                .find(|s| s.block == b)
                .and_then(|s| s.name.as_deref().map(str::to_string)),
            Playing::Free => None,
        };
        self.anim = anim_tracks(&self.model, &self.patched, &self.parsed, p);
        self.anim_shown = None;
    }

    /// The Time slider's span in scene time (B-P33): from 0 to the longest cycle, `span / |freq|`, where a Reverse
    /// cycle counts both legs. `ctrl_time` still does each controller's own wrapping, phase included. A clip uses
    /// its sequence's timing. None when nothing is animated, a controller's times are not finite, or the span is zero
    /// (C-P34 (b): only held values, no clock; `tick_anim` still shows them).
    pub fn anim_range(&self) -> Option<(f32, f32)> {
        self.anim_end()
            .filter(|e| e.is_finite() && *e > 0.0)
            .map(|e| (0.0, e))
    }

    /// A controller's times are not finite (+inf, NaN): playback is off and the panel says so.
    pub fn anim_invalid(&self) -> bool {
        self.anim_end().is_some_and(|e| !e.is_finite())
    }

    /// The longest cycle in scene time, NaN or inf when a contributing controller's times are not finite.
    fn anim_end(&self) -> Option<f32> {
        if self.anim.is_empty() {
            return None;
        }
        // freq 0 or a span <= 0 contributes nothing; a NaN span or freq passes through to the result.
        let cycle = |start: f32, stop: f32, freq: f32, reverse: bool| {
            let span = (stop - start) * if reverse { 2.0 } else { 1.0 };
            if freq == 0.0 || span <= 0.0 {
                0.0
            } else {
                span / freq.abs()
            }
        };
        let ends: Vec<f32> = match self.playing {
            Playing::Clip(seq) => {
                let s = self.model.sequences.iter().find(|s| s.block == seq)?;
                vec![cycle(s.start, s.stop, s.frequency, s.cycle == 1)]
            }
            Playing::Free => self
                .anim
                .iter()
                .flat_map(|(_, _, t)| t)
                .map(|t| cycle(t.start, t.stop, t.freq, (t.flags >> 1) & 3 == 1))
                .collect(),
        };
        // f32::max would drop a NaN: keep it, so the range is rejected.
        Some(ends.into_iter().fold(0.0, |a: f32, e| {
            if a.is_nan() || e.is_nan() {
                f32::NAN
            } else {
                a.max(e)
            }
        }))
    }

    /// Each animated shape's material at clock time `t`: `apply_anim` over the cached tracks, nothing parsed.
    pub fn frame_materials(&self, t: f32) -> Vec<(ShapeId, MaterialState)> {
        self.anim
            .iter()
            .map(|(id, base, tracks)| {
                let mut m = apply_anim(base, tracks, t);
                // Solo hides every other shape (preview_nif): a tick must not show it again.
                m.hidden |= self.solo && self.selection != Selection::Shape(id.0);
                (*id, m)
            })
            .collect()
    }
}

/// The tracks of every shape in `p` (its shader's and its alpha property's), for `playing`; shapes with none are
/// left out.
fn anim_tracks(
    m: &NifModel,
    b: &[u8],
    p: &ParsedNif,
    playing: Playing,
) -> Vec<(ShapeId, MaterialState, Vec<Track>)> {
    // B-P41: one header parse, not one per property.
    let Ok(h) = nif::header::Header::parse(b) else {
        return vec![];
    };
    p.shapes
        .iter()
        .filter_map(|s| {
            let sm = m.shape(s.id.0)?;
            let tracks: Vec<Track> = [sm.shader, sm.alpha]
                .into_iter()
                .flatten()
                .flat_map(|prop| match playing {
                    Playing::Free => free_tracks_in(m, &h, b, prop),
                    Playing::Clip(seq) => clip_tracks_in(m, &h, b, seq, prop),
                })
                .collect();
            (!tracks.is_empty()).then(|| (s.id, s.material.clone(), tracks))
        })
        .collect()
}

/// Meshes UI T5b (Note CZ19): the heel height shape `shape` needs, Z = −(lowest bind-pose vertex Z) in root space,
/// rounded to 0.1. A skinned shape's vertices go through the inverse of its NiSkinData skin transform; an unskinned
/// one through its node chain. None when the shape has no drawable geometry (a non-lighting shape, an LE file).
pub fn heel_z_of(bytes: &[u8], shape: u32) -> Option<f32> {
    let parsed = render3d::parse(bytes);
    let s = parsed.shapes.iter().find(|s| s.id.0 == shape)?;
    let h = nif::header::Header::parse(bytes).ok()?;
    let m = nif::geom::skin_to_root(&h, bytes, shape as usize)
        .ok()?
        .unwrap_or(s.world);
    let low = s
        .mesh
        .positions
        .iter()
        .map(|p| m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3])
        .fold(f32::INFINITY, f32::min);
    let z = (-low * 10.0).round() / 10.0;
    z.is_finite().then_some(z + 0.0) // + 0.0: never "-0"
}

/// The other weight file of `data_path`: `…_0.nif` ↔ `…_1.nif` (any case, the case kept). None for other names.
pub fn weight_sibling_of(data_path: &str) -> Option<String> {
    let dot = data_path.len().checked_sub(4)?;
    if !data_path.get(dot..)?.eq_ignore_ascii_case(".nif") {
        return None;
    }
    let (stem, ext) = data_path.split_at(dot);
    let other = match stem.get(stem.len().checked_sub(2)?..)? {
        "_0" => "_1",
        "_1" => "_0",
        _ => return None,
    };
    Some(format!("{}{other}{ext}", &stem[..stem.len() - 2]))
}

/// The Z of the last "NPC" entry of an SDTA string (`[{"name":"NPC","pos":[x,y,Z]}, …]`): the entry RaceMenu uses.
pub fn sdta_npc_z(sdta: &str) -> Option<f32> {
    let v: serde_json::Value = serde_json::from_str(sdta).ok()?;
    let npc = v
        .as_array()?
        .iter()
        .rev()
        .find(|e| e.get("name").and_then(|n| n.as_str()) == Some("NPC"))?;
    npc.get("pos")?.get(2)?.as_f64().map(|z| z as f32)
}

/// `sdta` with its last "NPC" entry's Z set to `z`, other entries kept; an "NPC" entry is appended when there is
/// none. None when the string is not a JSON array.
pub fn sdta_with_npc_z(sdta: &str, z: f32) -> Option<String> {
    let mut v: serde_json::Value = serde_json::from_str(sdta).ok()?;
    let list = v.as_array_mut()?;
    // T5b review Important 2: `z` as given, in its shortest f32 form ("2.25", "7.3"), so it reads back as the same
    // f32 that HH_OFFSET holds.
    let zv: serde_json::Value = serde_json::from_str(&z.to_string()).ok()?;
    let npc = list
        .iter_mut()
        .rev()
        .find(|e| e.get("name").and_then(|n| n.as_str()) == Some("NPC"));
    match npc {
        Some(e) => {
            let pos = e
                .as_object_mut()?
                .entry("pos")
                .or_insert(serde_json::json!([0, 0, 0]));
            match pos.as_array_mut() {
                Some(p) if p.len() >= 3 => p[2] = zv,
                _ => *pos = serde_json::json!([0, 0, zv]),
            }
        }
        None => list.push(serde_json::json!({"name": "NPC", "pos": [0, 0, zv]})),
    }
    serde_json::to_string(&v).ok()
}

/// Meshes UI T2: per shape, "N: title, title (clip)" over its shader's and its alpha's controllers; "none" without.
/// A shader that is not a BSLightingShaderProperty has controllers the editor does not read: its shape lists only its
/// alpha controllers, and gets no line (rather than a wrong "none") when it has none.
pub(crate) fn card_ctrls(m: &NifModel) -> HashMap<u32, String> {
    m.shapes
        .iter()
        .filter_map(|s| {
            let other = s.shader.is_some_and(|b| !m.lighting.contains_key(&b));
            let lit = s.shader.and_then(|b| m.lighting.get(&b));
            let slsf2 = lit.map_or(0, |l| l.lighting.slsf2);
            let props = [
                lit.map(|l| &l.prop),
                s.alpha.and_then(|b| m.alphas.get(&b)).map(|a| &a.prop),
            ];
            let titles: Vec<String> = props
                .into_iter()
                .flatten()
                .flat_map(|p| &p.ctrls)
                .filter_map(|c| m.ctrls.get(c).map(|t| (c, t)))
                .map(|(c, t)| {
                    let title = nif::edit::validate::ctrl_title(t, slsf2);
                    match m.managed(*c) {
                        true => format!("{title} (clip)"),
                        false => title,
                    }
                })
                .collect();
            let line = match titles.len() {
                0 if other => return None,
                0 => "none".to_string(),
                n => format!("{n}: {}", titles.join(", ")),
            };
            Some((s.block, line))
        })
        .collect()
}

/// Meshes UI T2: where Edit opens. The card picked outside edit mode (same block and name, else the only shape with
/// that name, since the shown copy may be PGPatcher's and numbered differently), else the only shape, else Root.
pub fn first_selection(m: &NifModel, picked: Option<&(u32, Option<String>)>) -> Selection {
    if let Some((block, name)) = picked {
        let same = m
            .shapes
            .iter()
            .find(|s| s.block == *block && s.name.as_deref() == name.as_deref());
        let mut named = m
            .shapes
            .iter()
            .filter(|s| name.is_some() && s.name.as_deref() == name.as_deref());
        let by_name = match (named.next(), named.next()) {
            (Some(s), None) => Some(s),
            _ => None,
        };
        if let Some(s) = same.or(by_name) {
            return Selection::Shape(s.block);
        }
    }
    match m.shapes.as_slice() {
        [only] => Selection::Shape(only.block),
        _ => Selection::Root,
    }
}

fn read_source(source: &[u8]) -> Result<(nif::scan::NifScan, NifModel), String> {
    let cannot = |e: nif::NifError| format!("This NIF cannot be edited: {e}");
    let scan = nif::scan::scan(source).map_err(cannot)?;
    Ok((scan, read_model(source).map_err(cannot)?))
}

fn source_id(copy: &PathCopy, data_path: &str, source: &[u8]) -> SourceId {
    SourceId {
        origin: copy.origin.clone(),
        data_path: data_path.into(),
        archive: (copy.kind == KIND_BSA).then(|| copy.root.clone()),
        size: source.len() as u64,
        sha256: edit_doc::sha256_hex(source),
    }
}

/// Each shape's (shader type, slot paths) for [`texture_sources`].
fn slot_paths<'a>(shapes: impl IntoIterator<Item = &'a ParsedShape>) -> Vec<(u32, Vec<String>)> {
    shapes
        .into_iter()
        .map(|s| {
            let (ty, slots) = lookup_key(&s.material);
            (ty, slots.map(str::to_owned).collect())
        })
        .collect()
}

/// What [`texture_sources`] reads of a material: its shader type and slot paths. [`slot_paths`] and
/// [`same_lookup`] both go through it, so the store lookup and its trigger cannot drift apart.
fn lookup_key(m: &MaterialState) -> (u32, impl Iterator<Item = &str>) {
    (m.shader_type, m.slots.iter().map(|t| t.norm.as_str()))
}

/// Whether `a` and `b` name the same textures to [`texture_sources`].
fn same_lookup(a: &MaterialState, b: &MaterialState) -> bool {
    let ((ta, sa), (tb, sb)) = (lookup_key(a), lookup_key(b));
    ta == tb && sa.eq(sb)
}

/// The same folder, ignoring ASCII case, `/` versus `\` and a trailing separator (MO2 paths on Windows).
pub(crate) fn same_dir(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        let s = p.to_string_lossy().replace('/', "\\");
        s.trim_end_matches('\\').to_ascii_lowercase()
    };
    norm(a) == norm(b)
}

/// Note GP10: what an export of `out` to `path` finds there: None when the file is absent, equal to `out` or
/// Tessalith's own (its hash is `last`, the last export's, or `partial`, the NIF a partial export wrote, ruling
/// E6-I); Some(its SHA-256) when Tessalith did not write it; Err when it cannot be read.
fn foreign_hash(
    path: &Path,
    out: &[u8],
    last: Option<&str>,
    partial: Option<&str>,
) -> Result<Option<[u8; 32]>, String> {
    let cannot = |e: std::io::Error| format!("Cannot read {}: {e}", path.display());
    let ours = |b: &[u8]| {
        let h = edit_doc::sha256_hex(b);
        last == Some(h.as_str()) || partial == Some(h.as_str())
    };
    match engine::dds::may_replace(path, out, ours) {
        Ok(()) => Ok(None),
        Err(Kept::Unreadable(e)) => Err(cannot(e)),
        Err(Kept::Foreign) => std::fs::read(path)
            .map(|b| Some(Sha256::digest(&b).into()))
            .map_err(cannot),
    }
}

/// The header's block count; 0 when it does not parse.
fn block_count(b: &[u8]) -> u32 {
    nif::header::Header::parse(b).map_or(0, |h| h.sizes.len() as u32)
}

/// A new `Content::Parsed` key: every load of edited bytes is a new load (R10).
fn next_key() -> u64 {
    static KEY: AtomicU64 = AtomicU64::new(1);
    KEY.fetch_add(1, Ordering::Relaxed)
}

#[cfg(feature = "test-hooks")]
thread_local! {
    static CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many `refresh_blocked` calls this thread made (Note GP8's test: one per change).
#[cfg(feature = "test-hooks")]
pub fn block_check_count() -> usize {
    CHECKS.with(std::cell::Cell::get)
}

impl MeshesModel {
    /// Attempts of the project switch's two saves so far, by the open session and the ones before it (ruling r4-2).
    #[cfg(feature = "test-hooks")]
    pub fn switch_save_attempts_for_test(&self) -> u32 {
        self.switch_attempts_done + self.edit.as_ref().map_or(0, |e| e.switch_save_attempts)
    }

    /// Ends the session and opens the same NIF again in `project`, keeping the selection (what a project switch does
    /// once the edits are saved). Nothing when the session may not end (`may_end_edit`).
    fn reopen_keeping_selection(&mut self, project: Option<&Path>) {
        let Some(selection) = self.edit.as_ref().map(|e| e.selection) else {
            return;
        };
        if !self.drop_edit_for(true) {
            return;
        }
        self.open_edit_in(project.map(Path::to_path_buf));
        let keep = self.edit.as_ref().is_some_and(|e| match selection {
            Selection::Root => true,
            Selection::Shape(id) => e.parsed.shapes.iter().any(|s| s.id.0 == id),
        });
        if keep {
            self.select_edit(selection);
        }
    }

    /// A retry of a session that could not be saved worked (`NifEdit::save`). With a project switch waiting
    /// (`pending_root`, the root last asked for), the session ends and the NIF opens in that project; when that is the
    /// session's own project and its draft took the edits, the session simply goes on there. With none waiting (a
    /// refused Close edit or another NIF), the session goes on, saved.
    pub(super) fn finish_switch(&mut self) {
        let Some(e) = self.edit.as_mut().filter(|e| e.switch_saved) else {
            return;
        };
        e.switch_saved = false;
        let Some(to) = self.pending_root.take() else {
            return;
        };
        if e.project == to && e.saved_in_draft() {
            return;
        }
        self.reopen_keeping_selection(to.as_deref());
    }

    /// Ruling E6-C, the one check before any session ends (another NIF, Close edit, the Edit toggle, a project
    /// switch, a scan's reload): true when it may, because the next open would load its ops (`NifEdit::durable`,
    /// ruling E6-G) or a save worked (the draft, then the kept autosave; with no project the plain autosave; each never
    /// over a file that is not the session's, E6-F). Otherwise the session stays as it is, its footer and export say
    /// why (`retarget_error`), and so does `error` for the refused action. `switching`: the action is a project switch
    /// (its own wording, ruling r3-3).
    fn may_end_edit(&mut self, switching: bool) -> bool {
        let Some(e) = self.edit.as_mut() else {
            return true;
        };
        e.end_drag();
        if e.saved_to_leave || e.durable() {
            return true;
        }
        match e.save_for_switch() {
            Ok(()) => true,
            Err(why) => {
                e.set_switch_failed(&why, switching);
                self.error = e.retarget_error.clone();
                false
            }
        }
    }

    /// The opened project changed (`MeshesModel::set_project_root`): what the open edit does (review E6).
    /// - back in its own project while a switch is waiting (ruling r4-5): the draft is saved there and the session
    ///   goes on; when that save fails too, the switch waits as before;
    /// - none to a project with a touched session: it moves to the project's draft (Note GD5 (c)), the one carry-over;
    /// - otherwise the session ends once its edits are durable (`may_end_edit`: the old project's draft, or a kept
    ///   autosave) and the NIF opens again from the new place. When no save works the session stays open, blocked,
    ///   the wanted root waits in `pending_root`, and nothing is retried until the next edit (ruling r3-3).
    pub(super) fn switch_project(&mut self, to: Option<&Path>) {
        let Some(e) = self.edit.as_mut() else {
            self.pending_root = None;
            return;
        };
        if e.switch_failed && e.project.as_deref() == to {
            // the wanted root is A now; it stays wanted until a save into A works (Important 8)
            self.pending_root = Some(to.map(Path::to_path_buf));
            if e.save_back_into_own_project() {
                self.pending_root = None;
            }
            return;
        }
        if let (None, Some(p), true) = (&e.project, to, e.touched()) {
            // ruling E6-D: the live edit is durable in its autosave first, so the decision reads its time
            if !self.may_end_edit(true) {
                self.pending_root = Some(to.map(Path::to_path_buf));
                return;
            }
            let Some(e) = self.edit.as_mut() else { return };
            match e.retarget(p) {
                Ok(Some(lines)) => {
                    self.messages.extend(lines);
                    // ruling E6-H: ops the project would load already (`durable`, E6-G) write nothing; the next
                    // change saves them into it
                    if e.durable() {
                        e.unsaved = false;
                    } else {
                        e.save();
                    }
                    return;
                }
                Ok(None) => {} // the project's own edits are newer: the NIF opens there, below
                // never overwritten: the edits stay in the autosave, and the draft is not opened over them
                Err(why) => {
                    self.error = Some(format!(
                        "Could not move the edits of {} into the project: {why}. They stay in the autosave.",
                        e.data_path
                    ));
                    return;
                }
            }
        }
        if !self.may_end_edit(true) {
            self.pending_root = Some(to.map(Path::to_path_buf));
            return;
        }
        self.pending_root = None;
        self.reopen_keeping_selection(to);
    }

    /// Where NIF edits go: the chosen export folder, and nowhere without one (Note GD5 supersedes CI29's
    /// `<instance>\mods\sdc NIF Edits` default).
    pub fn edit_mod(&self) -> Option<PathBuf> {
        self.nif_export_root.clone()
    }

    /// Note GD5 (c), Review E6: the plain autosave of `path` (beside the scan store), the draft while no project is
    /// open; with a project, the folder its kept autosaves go to. The export folder plays no part.
    fn autosave_of(&self, path: &str) -> Option<PathBuf> {
        let dir = self.store_path.as_ref()?.parent()?;
        edit_doc::autosave_path(dir, path).ok()
    }

    /// Opens an edit session for the selected NIF and shows its patched bytes as `Content::Parsed` (an LE NIF
    /// opens with no preview). An open session that may not end (ruling E6-C) stays, and nothing opens.
    pub fn open_edit(&mut self) {
        self.open_edit_in(self.project_root.clone());
    }

    /// `open_edit` with the draft read from `project` (a project switch names the root it waited for).
    fn open_edit_in(&mut self, project: Option<PathBuf>) {
        if !self.drop_edit() {
            return;
        }
        // Final review Minor 8: the column before Edit (Edit locks it on Shapes); `drop_edit` puts it back.
        let column = self.shapes_col;
        let Some(row) = self.selected_row().cloned() else {
            return;
        };
        let edit_mod = self.edit_mod();
        match self.start_edit(&row.path, edit_mod.as_deref(), project.as_deref()) {
            Ok((mut e, content)) => {
                e.selection = first_selection(&e.model, self.card_sel.as_ref());
                self.messages.append(&mut e.open_lines);
                self.edit = Some(e);
                self.shapes_col_before = Some(column);
                // The edit opens on Edited source (v2 B8: In game is pgpsim over it).
                self.view_source = true;
                self.refresh_in_game(); // Note GH1: starts the JSON read for the paths PGPatcher supplies
                if let Some(c) = content {
                    self.v3.set_content(Some(c), false);
                    self.v3.nif_view = true;
                }
                self.refresh_placement();
                self.edit_checks();
            }
            Err(e) => {
                // A previous session's Parsed preview would no longer match the error.
                if !self.v3.is_mesh() {
                    self.load_shapes();
                }
                self.error = Some(e);
            }
        }
    }

    fn start_edit(
        &self,
        path: &str,
        edit_mod: Option<&Path>,
        project: Option<&Path>,
    ) -> Result<(NifEdit, Option<Content>), String> {
        let store = self.store.as_ref().ok_or("No scan store.")?;
        edit_doc::win(path)?; // before the read: a loose copy's root joined with `..` would read outside its mod
        let copies = store.copies_of_path(path).map_err(db_err)?;
        let src = pick_source(&copies, edit_mod.unwrap_or(Path::new("")), path)
            .ok_or_else(|| format!("No source copy of {path} to edit."))?
            .clone();
        let bytes = read_file(&FileSource::from_store(src.kind, &src.root, path))?;
        let autosave = self.autosave_of(path);
        let mut e = NifEdit::open(bytes, src, path, project, edit_mod, autosave.as_deref())?;
        e.copies = copies;
        e.weight_sibling = weight_sibling_of(path)
            .filter(|s| store.copies_of_path(s).is_ok_and(|c| !c.is_empty()));
        let c = self.edit_content(&e)?;
        Ok((e, c))
    }

    /// The session's patched bytes as a new `Content::Parsed` (None for an LE NIF).
    fn edit_content(&self, e: &NifEdit) -> Result<Option<Content>, String> {
        let store = self.store.as_ref().ok_or("No scan store.")?;
        if e.le_mesh {
            return Ok(None);
        }
        let mut nif = e.preview_nif();
        nif.head_part = store.is_head_part_mesh(&e.data_path).unwrap_or(false); // shading only, as mesh_spec
        let textures = texture_sources(store, &slot_paths(&nif.shapes)).map_err(db_err)?;
        Ok(Some(Content::Parsed {
            key: next_key(),
            nif: Arc::new(nif),
            textures: Arc::new(textures),
        }))
    }

    /// Shows the open session's edit preview, rebuilt from its current edits, on the toggle's side (v2 B8: In game
    /// overlays the simulated materials).
    /// False when there is none to show (no session, an LE NIF, or an error, which goes to `error`).
    pub(super) fn show_edit_preview(&mut self) -> bool {
        self.refresh_in_game();
        let Some(e) = &self.edit else { return false };
        match self.edit_content(e) {
            Ok(Some(c)) => {
                self.v3.set_content(Some(c), false);
                self.v3.nif_view = true;
                if let Some(e) = &mut self.edit {
                    e.anim_shown = None;
                }
                true
            }
            Ok(None) => false,
            Err(err) => {
                self.error = Some(err);
                false
            }
        }
    }

    /// An edit is open but the 3D view shows a plain copy (`show_edit_preview` failed): the edits are not on screen.
    pub fn edit_preview_hidden(&self) -> bool {
        self.edit.as_ref().is_some_and(|e| !e.le_mesh) && !self.v3.nif_view
    }

    /// Ends the session (what is not durable is saved first) and shows the NIF as a mesh again. A session that may not
    /// end stays (ruling E6-C).
    pub fn close_edit(&mut self) {
        if self.edit.is_none() {
            return;
        }
        if self.drop_edit() {
            self.load_shapes();
        }
    }

    /// Ends the session without touching the view's content, once `may_end_edit` allows it; false (nothing changed)
    /// when it does not.
    pub(super) fn drop_edit(&mut self) -> bool {
        self.drop_edit_for(false)
    }

    /// `drop_edit`; `switching`: for a project switch (the refusal's wording).
    fn drop_edit_for(&mut self, switching: bool) -> bool {
        if !self.may_end_edit(switching) {
            return false;
        }
        self.clear_nif_answers(); // Note GD29: `close_edit` too, which ends here
        if let Some(column) = self.shapes_col_before.take() {
            self.shapes_col = column;
        }
        if let Some(mut e) = self.edit.take() {
            #[cfg(feature = "test-hooks")]
            {
                self.switch_attempts_done += e.switch_save_attempts;
            }
            self.messages.append(&mut e.open_lines);
            if let Some(err) = e.error {
                self.error = Some(err);
            }
            self.pending_root = None; // a switch waiting on this session waits no more
        }
        self.v3.nif_view = false;
        self.v3.clock = None;
        true
    }

    /// One drawn frame of the animation (v2 B3). Call once per frame the Meshes 3D view is drawn, before
    /// `ui::viewport3d::show`, so that frame's Render carries the new time. Keeps the clock in step with the
    /// session's tracks, advances it, and sends each animated shape's material (cbuffer only) when the time moved.
    pub fn tick_anim(&mut self, dt: f32) {
        // A shape animated last frame that no longer is (a removed controller, B7's clip switch) shows its file
        // material again: nothing else resends it, its static material did not change.
        if let Some(e) = self.edit.as_mut() {
            let now: Vec<ShapeId> = e.anim.iter().map(|(id, _, _)| *id).collect();
            if e.anim_last.iter().any(|id| !now.contains(id)) {
                let gone: Vec<_> = e
                    .materials() // clones the parse (F22): only on the frame a shape leaves the set
                    .into_iter()
                    .filter(|(id, _)| e.anim_last.contains(id) && !now.contains(id))
                    .collect();
                for (id, m) in gone {
                    // false: no load sent yet; the next load carries the file material
                    self.v3.set_material(id, m);
                }
            }
            e.anim_last = now;
        }
        let in_game = self.edit.as_ref().is_some_and(|e| e.in_game.is_some());
        let Some(range) = self.edit.as_ref().and_then(NifEdit::anim_range) else {
            self.v3.clock = None;
            // C-P34 (b): a zero-length range (held values only) has no clock, but its values still show.
            let held = self
                .edit
                .as_ref()
                .is_some_and(|e| !e.anim.is_empty() && !e.anim_invalid());
            if held && self.v3.nif_view && !in_game {
                self.show_anim_frame(0.0);
            }
            return;
        };
        let c = self.v3.clock.get_or_insert(Clock::new(range));
        if c.range != range {
            c.range = range;
            c.t = c.t.clamp(range.0, range.1);
        }
        // In game (simulated) shows static PGPatcher materials (F21): the clock pauses there.
        if !self.v3.nif_view || in_game {
            c.playing = false;
            return;
        }
        self.v3.tick(dt);
        if let Some(c) = self.v3.clock {
            self.show_anim_frame(c.t);
        }
    }

    /// Sends each animated shape's material at time `t`, once per new `t`.
    fn show_anim_frame(&mut self, t: f32) {
        let Some(e) = self.edit.as_mut() else {
            return;
        };
        if e.anim_shown == Some(t) {
            return;
        }
        for (id, m) in e.frame_materials(t) {
            // false: no load of this content sent yet (a reload); the next frame tries again.
            if !self.v3.set_material(id, m) {
                return;
            }
        }
        e.anim_shown = Some(t);
    }

    pub fn edit_push(&mut self, ops: Vec<Op>, merge: Option<String>) {
        let Some(e) = &mut self.edit else { return };
        let before = e.materials();
        e.push(ops, merge);
        self.edit_changed(before);
        self.finish_switch();
    }

    pub fn undo_edit(&mut self) -> bool {
        let Some(e) = &mut self.edit else {
            return false;
        };
        let before = e.materials();
        let done = e.undo();
        self.edit_changed(before);
        self.finish_switch();
        done
    }

    pub fn redo_edit(&mut self) -> bool {
        let Some(e) = &mut self.edit else {
            return false;
        };
        let before = e.materials();
        let done = e.redo();
        self.edit_changed(before);
        self.finish_switch();
        done
    }

    pub fn select_edit(&mut self, s: Selection) {
        let Some(e) = &mut self.edit else { return };
        let before = e.materials();
        e.selection = s;
        self.edit_changed(before);
    }

    /// Runs `f` on the session (Solo, Keep mine, Take source, Revert) and updates the preview.
    pub fn edit_with(&mut self, f: impl FnOnce(&mut NifEdit)) {
        let Some(e) = &mut self.edit else { return };
        let before = e.materials();
        f(e);
        self.edit_changed(before);
        self.finish_switch();
    }

    /// "Check that files exist": on, the slot warnings include missing files; off, they are dropped again.
    pub fn set_edit_check_exists(&mut self, on: bool) {
        let Some(e) = &mut self.edit else { return };
        e.check_exists = on;
        e.warnings = validate(&e.model, None);
        demote_source(&e.source_warnings, &mut e.warnings);
        if !on || self.store.is_none() {
            e.refresh_blocked(); // the warnings changed without a new `rev`, and `edit_checks` does not validate (GP8)
        }
        self.edit_checks();
    }

    /// The Browse list: scanned winning texture paths that contain `filter` (at most `limit`).
    pub fn texture_paths(&self, filter: &str, limit: usize) -> Vec<String> {
        self.store
            .as_ref()
            .and_then(|s| s.texture_paths(filter, limit).ok())
            .unwrap_or_default()
    }

    /// The session's error, the missing-file check when it is on, the export block and the session's NIF badge.
    fn edit_checks(&mut self) {
        let Some(e) = &mut self.edit else { return };
        if let Some(err) = e.error.take() {
            self.error = Some(err);
        }
        let mut validated = false;
        if let (true, Some(store)) = (e.check_exists, &self.store) {
            let exists = |n: &str| matches!(store.texture_location(n), Ok(Some(_)));
            // shortcut: with the check on, every change queries the store once per slot, drag ticks included, upgrade
            // by caching the lookups per slot path if a drag stalls on a slow store.
            e.warnings = validate(&e.model, Some(&exists));
            demote_source(&e.source_warnings, &mut e.warnings);
            validated = true;
        }
        // Note GP8: a change already checked in `reapply` is not checked again; what changes the block's inputs
        // without a new `rev` refreshes it itself (`after_export`, `set_edit_check_exists`, the folder change)
        if validated || e.blocked_rev != e.rev {
            e.refresh_blocked();
        } else {
            debug_assert_eq!(
                e.blocked,
                e.export_block(),
                "stale export block at rev {}",
                e.rev
            );
            debug_assert_eq!(
                *e.problems,
                super::worklist::problems(e),
                "stale worklist rows at rev {}",
                e.rev
            );
        }
        if !e.edit_mod.as_os_str().is_empty() {
            match DocBadge::of(&e.doc) {
                Some(b) => self.doc_badges.insert(e.data_path.clone(), b),
                None => self.doc_badges.remove(&e.data_path),
            };
        }
    }

    /// The open session's result line after an export, and the preview (a changed source re-applied the ops).
    fn after_export(&mut self, before: Vec<(ShapeId, MaterialState)>, out: &ExportOutcome) {
        self.foreign_cache.take();
        let Some(e) = &mut self.edit else { return };
        e.export_note = match out {
            ExportOutcome::Written { rerun_pgpatcher } => Some((
                match rerun_pgpatcher {
                    true => RERUN_PGPATCHER.into(),
                    false => "Exported.".into(),
                },
                false,
            )),
            ExportOutcome::Blocked(s) => Some((s.clone(), true)),
            ExportOutcome::Foreign => {
                Some((format!("Not exported: {NIF_FOREIGN_AT_WRITE}."), true))
            }
        };
        // the re-run row follows `export_note`, which has no `rev` of its own (Note GP8)
        e.refresh_blocked();
        // A changed source re-applied the ops: the preview follows.
        self.edit_changed(before);
    }

    /// Note GD2: one row per NIF with pending edits (today the open edit only), for the Review. Reads the export
    /// path's state from the disk only when its length, time or the last export's hash changed.
    pub fn nif_review_rows(&self) -> Vec<NifReviewRow> {
        // a session no save works for has its row too, with that reason, even when it equals the last export (E6-C)
        let Some(e) = self
            .edit
            .as_ref()
            .filter(|e| e.switch_failed || e.pending())
        else {
            return vec![];
        };
        let mut blocked = e.blocked.clone();
        let foreign = match e.export_path.as_os_str().is_empty() {
            true => None,
            false => self.disk_state(e).unwrap_or_else(|why| {
                blocked.get_or_insert(why);
                None
            }),
        };
        let mut answer = None;
        if let (Some(f), Some((c, h))) = (
            foreign,
            self.nif_answers.get(&e.data_path.to_ascii_lowercase()),
        ) {
            match *h == f {
                true => answer = Some(*c),
                false => {
                    blocked.get_or_insert(NIF_CHANGED_SINCE.into());
                }
            }
        }
        vec![NifReviewRow {
            path: e.data_path.clone(),
            mod_name: e
                .edit_mod
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            blocked,
            foreign,
            answer,
            compaction: e.compaction,
        }]
    }

    /// The export path of `e` as the export would find it: None when absent or Tessalith's own, Some(SHA-256) when
    /// Tessalith did not write it, Err when it cannot be read.
    // shortcut: the key leaves out the edit's bytes, so a file that equals them reads as foreign until it changes;
    // add `e.rev` if that case shows up (it would re-read the file on every drag tick).
    fn disk_state(&self, e: &NifEdit) -> Result<Option<[u8; 32]>, String> {
        let meta = std::fs::metadata(&e.export_path)
            .ok()
            .map(|m| (m.len(), m.modified().ok()));
        let key = (
            e.export_path.clone(),
            e.doc.export.sha256.clone(),
            e.partial.clone(),
            meta,
        );
        if let Some((k, v)) = &*self.foreign_cache.borrow() {
            if *k == key {
                return v.clone();
            }
        }
        let v = foreign_hash(
            &e.export_path,
            &e.out,
            e.doc.export.sha256.as_deref(),
            e.partial.as_deref(),
        );
        *self.foreign_cache.borrow_mut() = Some((key, v.clone()));
        v
    }

    /// Note GD2: Review's answer for the file at `path`'s export path, given for the file with hash `sha256`. It
    /// counts only while the file still has that hash, and for one export.
    pub fn set_nif_answer(&mut self, path: &str, choice: GuardChoice, sha256: [u8; 32]) {
        self.nif_answers
            .insert(path.to_ascii_lowercase(), (choice, sha256));
    }

    /// Note GH9 (Q2, Q4): why the 3D view's `Export this NIF` is off, which is its hover; None = on. The row is the
    /// same one Review lists, so its `blocked` text is the reason.
    pub fn quick_export_block(&self) -> Option<String> {
        if self.export_running {
            return Some("An export is running".into());
        }
        // ruling E6-J: a failed switch's reason comes before the missing folder (it says the edits are not saved)
        if let Some(why) = self
            .edit
            .as_ref()
            .filter(|e| e.switch_failed)
            .and_then(|e| e.retarget_error.clone())
        {
            return Some(why);
        }
        if self.nif_export_root.is_none() {
            return Some(NO_EXPORT_FOLDER.into());
        }
        match self.nif_review_rows().into_iter().next() {
            Some(r) => r.blocked,
            None => Some(NO_EDITS.into()),
        }
    }

    /// Final review Minor 2: the click on `Export this NIF` opens Review instead of writing, because the file at the
    /// export path is foreign and has no answer yet.
    pub fn quick_export_asks(&self) -> bool {
        self.nif_review_rows()
            .iter()
            .any(|r| r.foreign.is_some() && r.answer.is_none())
    }

    /// Note GH9 (Q3, Q5): writes the open edit's NIF alone, as Review's Export would. A file at the export path that
    /// Tessalith did not write, with no Review answer that still matches it, is never overwritten: the click opens
    /// Review on that row instead. The result waits in `quick_export_result` for the app.
    pub fn quick_export(&mut self) {
        if self.quick_export_block().is_some() {
            return;
        }
        if self.quick_export_asks() {
            self.open_review_request = true;
            return;
        }
        self.quick_export_result = Some(self.export_nif_edits());
    }

    /// Note GD29: drops every Review answer, so each foreign row asks again. Review calls it when it closes and when
    /// the project changes (FC5); the edit's close and a folder change call it too.
    pub fn clear_nif_answers(&mut self) {
        self.nif_answers.clear();
    }

    /// Note GD2: the block diff for the Review row's Diff…: the file on disk against what the
    /// export writes. None unless `path`'s row has a foreign file. Reads the disk: call it on a click, not per frame.
    pub fn nif_diff(&self, path: &str) -> Option<Vec<String>> {
        let row = self
            .nif_review_rows()
            .into_iter()
            .find(|r| r.path.eq_ignore_ascii_case(path))?;
        row.foreign?;
        let e = self.edit.as_ref()?;
        let on_disk = std::fs::read(&e.export_path).ok()?;
        Some(
            nif::write::block_diff(&on_disk, &e.out).unwrap_or_else(|err| {
                vec![format!("The file on disk does not read as a NIF: {err}")]
            }),
        )
    }

    /// Note GD18: opens the edit panel's Diff view of `path` (Review's Diff…, FC5, and the panel's own Diff… call
    /// this). The lines are read here, once; None (no foreign file) closes the view.
    pub fn show_nif_diff(&mut self, path: &str) {
        self.nif_diff_view = self.nif_diff(path).map(|l| (path.to_string(), l));
    }

    /// Note GD2: writes every Review row that is not blocked and, when Tessalith did not write its file, is answered
    /// Overwrite for the bytes on disk now (read again here). The answers are used up. Ok(written) when nothing was
    /// skipped, else Err listing each skipped NIF and why. Each write is a line in Messages, with its placement.
    pub fn export_nif_edits(&mut self) -> Result<usize, String> {
        self.foreign_cache.take(); // check again at write time
        if let Some(e) = self.edit.as_mut() {
            e.recheck_record(); // ruling E6-F: a moved blocking record no longer blocks
        }
        let rows = self.nif_review_rows();
        self.nif_answers.clear();
        let (mut written, mut skipped) = (0, vec![]);
        for r in rows {
            let why = match (&r.blocked, r.foreign, r.answer) {
                (Some(b), _, _) => Some(b.clone()),
                (None, Some(_), None) => Some(NIF_UNANSWERED.to_string()),
                (None, Some(_), Some(GuardChoice::KeepTheirs)) => Some(NIF_KEPT.to_string()),
                _ => None,
            };
            if let Some(why) = why {
                skipped.push(format!("{}: {why}", r.path));
                continue;
            }
            let Some(e) = self.edit.as_mut().filter(|e| e.data_path == r.path) else {
                continue;
            };
            let before = e.materials();
            let out = e.write_out(r.foreign);
            let placement = e.placement.clone();
            self.after_export(before, &out);
            match out {
                ExportOutcome::Written { .. } => {
                    written += 1;
                    self.messages
                        .push(format!("Exported {} to {}.", r.path, r.mod_name));
                    if let Some(p) = placement.filter(|p| *p != Placement::Ok) {
                        self.messages.push(p.text(&r.mod_name));
                    }
                }
                ExportOutcome::Blocked(why) => skipped.push(format!("{}: {why}", r.path)),
                ExportOutcome::Foreign => {
                    skipped.push(format!("{}: {NIF_FOREIGN_AT_WRITE}", r.path))
                }
            }
        }
        match skipped.is_empty() {
            true => Ok(written),
            false => Err(format!("NIF edits skipped: {}", skipped.join("; "))),
        }
    }

    /// Note GD1: the open edit's worklist rows; the same `Arc` until they change (an empty one without an edit).
    pub fn nif_problems(&self) -> Arc<Vec<NifProblem>> {
        match &self.edit {
            Some(e) => e.problems.clone(),
            None => self.no_problems.clone(),
        }
    }

    /// A worklist row's fix (Note GD1): opens the NIF at `path` when another one (or none) is open, then asks the edit
    /// panel for `tab` and, when it draws it, focus on `field`.
    pub fn show_nif_tab(&mut self, path: &str, tab: NifTab, field: Option<Field>) {
        let open = self
            .edit
            .as_ref()
            .is_some_and(|e| e.data_path.eq_ignore_ascii_case(path));
        if !open {
            let row = self
                .winners
                .iter()
                .map(|(n, _)| n)
                .find(|n| n.path.eq_ignore_ascii_case(path))
                .map(|n| n.copy_id);
            let Some(id) = row else {
                self.error = Some(format!("{path} is not in the scanned meshes."));
                return;
            };
            // Note GQ4: select_nif refuses then, and open_edit would open whatever is still selected
            if !self.store_settled() {
                self.error = Some(format!(
                    "{path} opens once the Meshes scan finishes: wait for it, or scan again."
                ));
                return;
            }
            self.select_nif(id);
            if self.selected_nif != Some(id) {
                return; // the open session could not end (ruling E6-C): `error` says why
            }
            self.open_edit();
        }
        if self
            .edit
            .as_ref()
            .is_some_and(|e| e.data_path.eq_ignore_ascii_case(path))
        {
            self.nif_tab = Some((tab, field));
        }
    }

    /// The badge of every edit document in the edit mod, keyed by data path (walks `.sdc\nif-edits`).
    pub fn pending_docs(&self) -> HashMap<String, DocBadge> {
        self.edit_mod()
            .map(|m| super::export::pending_docs(&m).into_iter().collect())
            .unwrap_or_default()
    }

    /// `doc_badges` from disk; after a scan, or a change of instance.
    pub(super) fn refresh_doc_badges(&mut self) {
        self.doc_badges = self.pending_docs();
    }

    /// The session's placement line: from the store's copies and mod rows, and the modlist to tell a mod that is not
    /// scanned yet from one that is not enabled.
    pub(super) fn refresh_placement(&mut self) {
        let Some(e) = &mut self.edit else { return };
        let (Some(ScanTarget::Mo2 { root, profile }), false) =
            (&self.target, e.edit_mod.as_os_str().is_empty())
        else {
            e.placement = None;
            return;
        };
        let inst = Instance::load(root, Some(profile)).ok();
        // Only a folder directly in the instance's `mods` is an MO2 mod (Note CI29); its folder name is the mod name.
        let name = match (e.edit_mod.parent(), e.edit_mod.file_name(), &inst) {
            (Some(parent), Some(name), Some(i)) if same_dir(parent, &i.mods) => {
                name.to_string_lossy().into_owned()
            }
            _ => {
                e.placement = Some(Placement::NotInMo2);
                return;
            }
        };
        let mut p = placement(&e.copies, &self.mods, &name);
        if p == Placement::NotEnabled {
            let listed = inst
                .and_then(|i| modscan::mo2::loose_origins(&i).ok())
                .is_some_and(|(on, missing)| {
                    on.iter().any(|o| o.name.eq_ignore_ascii_case(&name))
                        || missing.iter().any(|n| n.eq_ignore_ascii_case(&name))
                });
            if listed {
                p = Placement::NotScanned;
            }
        }
        e.placement = Some(p);
    }

    /// The app sets the Textures export folder each frame (Note CI29). A change re-reads the chips and is a light
    /// switch for an open session: it keeps its draft and takes the new folder's record (`NifEdit::set_record_root`),
    /// writes nothing, and drops the Review answers given for the old folder's files. A folder that cannot be taken
    /// (its record does not load, or it holds the source copy) blocks the session's export with the reason and is
    /// not taken. The selected shape is kept.
    pub fn set_nif_export_root(&mut self, root: Option<PathBuf>) {
        // Note GD19: the session's lines (an autosave moved aside by a save) reach Messages on the next frame
        if let Some(e) = &mut self.edit {
            self.messages.append(&mut e.open_lines);
        }
        if self.nif_export_root == root {
            // Ruling E6-F: a record that blocks is read again once its file changed (a stat per frame while it blocks)
            if let Some(e) = self.edit.as_mut() {
                e.recheck_record();
            }
            // Note GD12: back to the folder the session never left: its record is current again
            if let Some(e) = self
                .edit
                .as_mut()
                .filter(|e| e.retarget_error.is_some() && !e.switch_failed)
            {
                e.retarget_error = None;
                e.retarget_fail = None;
                e.refresh_blocked();
            }
            return;
        }
        if let Some(e) = self.edit.as_mut() {
            // Note GD20: a failure is tried again only when the root or its record's length or time changed (a
            // stat per frame, never a read)
            let key = root.as_deref().map(|r| {
                let p = edit_doc::doc_path(r, &e.data_path).unwrap_or_default();
                let meta = std::fs::metadata(&p)
                    .ok()
                    .map(|m| (m.len(), m.modified().ok()));
                (r.to_path_buf(), p, meta)
            });
            if key.is_some() && e.retarget_fail == key {
                return;
            }
            match e.set_record_root(root.as_deref()) {
                Ok(()) => {
                    e.retarget_fail = None;
                    if !e.switch_failed {
                        e.retarget_error = None;
                    }
                    e.refresh_blocked();
                }
                Err(why) => {
                    e.retarget_fail = key;
                    if !e.switch_failed && e.retarget_error.as_ref() != Some(&why) {
                        e.retarget_error = Some(why);
                        e.refresh_blocked();
                    }
                    return;
                }
            }
        }
        self.nif_export_root = root;
        self.clear_nif_answers(); // Note GD29: they were given for the old folder's files
        self.refresh_doc_badges();
        self.refresh_placement();
        if self.edit.is_some() {
            self.edit_checks();
        }
    }
    /// The preview after a change (R10): each changed shape's material goes to the loaded content; a texture path
    /// the content's map lacks (and the store has), or a material the viewport refuses, reloads the same view with
    /// the map extended.
    pub(super) fn edit_changed(&mut self, before: Vec<(ShapeId, MaterialState)>) {
        // It sends static materials: the next tick_anim resends the animated ones.
        if let Some(e) = &mut self.edit {
            e.anim_shown = None;
        }
        self.edit_checks();
        self.refresh_in_game();
        let (Some(e), Some(store)) = (&self.edit, &self.store) else {
            return;
        };
        if e.le_mesh {
            return;
        }
        // A failed show_edit_preview left a plain copy on screen: the session is current, and the next switch
        // rebuilds the preview from it.
        let Some(Content::Parsed { nif, textures, .. }) = self.v3.content() else {
            return;
        };
        let (head_part, textures, retry) = (nif.head_part, textures.clone(), e.lookup_failed);
        let mut next = e.preview_nif();
        next.head_part = head_part;
        let pairs: Vec<(&ParsedShape, Option<&MaterialState>)> = next
            .shapes
            .iter()
            .map(|s| (s, before.iter().find(|(id, _)| *id == s.id).map(|(_, m)| m)))
            .collect();
        // The store is asked only for shapes whose shader type or slot paths changed (the only inputs of
        // `texture_sources`): a drag tick on a float field changes a material but names no new texture.
        let named: Vec<&ParsedShape> = pairs
            .iter()
            .filter(|(s, old)| retry || old.is_none_or(|m| !same_lookup(m, &s.material)))
            .map(|(s, _)| *s)
            .collect();
        let fresh = if named.is_empty() {
            HashMap::new()
        } else {
            match texture_sources(store, &slot_paths(named)) {
                Ok(f) => f,
                Err(err) => {
                    self.error = Some(db_err(err));
                    if let Some(e) = &mut self.edit {
                        e.lookup_failed = true;
                    }
                    return;
                }
            }
        };
        if let Some(e) = &mut self.edit {
            e.lookup_failed = false;
        }
        let mut reload = fresh.keys().any(|k| !textures.contains_key(k));
        if !reload {
            // The render thread keeps these edits and re-applies them on a reload it starts itself.
            for (s, old) in &pairs {
                if *old != Some(&s.material) && !self.v3.set_material(s.id, s.material.clone()) {
                    reload = true;
                    break;
                }
            }
        }
        if reload {
            let mut map = (*textures).clone();
            for (k, v) in fresh {
                map.entry(k).or_insert(v);
            }
            let c = Content::Parsed {
                key: next_key(),
                nif: Arc::new(next),
                textures: Arc::new(map),
            };
            self.v3.set_content(Some(c), true);
        }
    }
}
