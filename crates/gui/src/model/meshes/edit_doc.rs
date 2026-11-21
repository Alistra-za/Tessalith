//! The NIF edit document (spec §5, plan A15): the op list that rides with one source NIF, stored as TOML in the
//! project's draft (or the app's autosave with no project) and, as the record of an export, next to the exported file;
//! plus the in-memory undo stack the editor uses. No egui types.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The mod NIF edits went to without an export folder until Note GD5 (CI3's default); nothing uses it as a default now.
pub const EDIT_MOD: &str = "sdc NIF Edits";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceId {
    pub origin: String,
    pub data_path: String,
    pub archive: Option<String>,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExportRec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub sha256: Option<String>,
    pub at: Option<u64>,
    pub ops_hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpStatus {
    Applied,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocOp {
    pub op: nif::edit::Op,
    pub status: OpStatus,
}

/// The `Op` variants clip editing used (serde names); a saved document may still hold them.
const REMOVED_CLIP_OPS: [&str; 5] = [
    "AddSequence",
    "RemoveSequence",
    "RenameSequence",
    "SetSequenceTiming",
    "ConvertChainToClips",
];

/// `[ops.op.<Variant>]` (a table with one key) or the bare string `op = "<Variant>"`.
fn is_clip_op(o: &toml::Value) -> bool {
    match o.get("op") {
        Some(toml::Value::Table(t)) => {
            t.len() == 1 && t.keys().any(|k| REMOVED_CLIP_OPS.contains(&k.as_str()))
        }
        Some(toml::Value::String(s)) => REMOVED_CLIP_OPS.contains(&s.as_str()),
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditDoc {
    pub version: u32,
    /// The draft's stable id: see `new_id`. A document without one gets one in memory when a session opens it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub source: SourceId,
    #[serde(default)]
    pub export: ExportRec,
    pub ops: Vec<DocOp>,
    /// Clip ops `from_text` removed from the loaded text (Note GR2); the next save writes the document without them.
    #[serde(skip)]
    pub dropped_clip_ops: usize,
}

impl EditDoc {
    pub fn new(source: SourceId) -> EditDoc {
        EditDoc {
            version: 1,
            id: None,
            source,
            export: ExportRec::default(),
            ops: Vec::new(),
            dropped_clip_ops: 0,
        }
    }

    pub fn to_text(&self) -> String {
        toml::to_string_pretty(self).expect("edit doc serialises")
    }

    /// Clip ops (removed, Note GR2) are dropped here and counted in `dropped_clip_ops`.
    pub fn from_text(t: &str) -> Result<EditDoc, String> {
        let mut v: toml::Table = toml::from_str(t).map_err(|e| e.to_string())?;
        let mut dropped = 0;
        if let Some(toml::Value::Array(ops)) = v.get_mut("ops") {
            let before = ops.len();
            ops.retain(|o| !is_clip_op(o));
            dropped = before - ops.len();
        }
        // Re-parse text, not the table, so a bad field reports its line.
        // shortcut: after a drop the line numbers are the filtered text's (the quoted line still shows the fault); keep
        // the spans by blanking the clip tables in the original text if a user reports a wrong line.
        let filtered;
        let text = if dropped == 0 {
            t
        } else {
            filtered = toml::to_string(&v).map_err(|e| e.to_string())?;
            &filtered
        };
        let mut d: EditDoc = toml::from_str(text).map_err(|e| e.to_string())?;
        d.dropped_clip_ops = dropped;
        if d.version > 1 {
            return Err(format!(
                "This edit file was written by a newer sdc (version {}).",
                d.version
            ));
        }
        Ok(d)
    }

    pub fn ops(&self) -> Vec<nif::edit::Op> {
        self.ops.iter().map(|d| d.op.clone()).collect()
    }

    /// SHA-256 hex of the TOML of the ops alone (a top-level table needs a wrapper key).
    pub fn ops_hash(&self) -> String {
        #[derive(Serialize)]
        struct Ops<'a> {
            ops: &'a [DocOp],
        }
        #[cfg(feature = "test-hooks")]
        HASHES.with(|n| n.set(n.get() + 1));
        let t = toml::to_string_pretty(&Ops { ops: &self.ops }).expect("ops serialise");
        sha256_hex(t.as_bytes())
    }

    /// The ops differ from the last export; a document never exported is pending once it has ops (an undo back to
    /// no ops after an export is pending: the exported file still has the edits).
    pub fn pending(&self) -> bool {
        match &self.export.ops_hash {
            None => !self.ops.is_empty(),
            Some(h) => *h != self.ops_hash(),
        }
    }

    pub fn has_conflicts(&self) -> bool {
        self.ops.iter().any(|d| d.status == OpStatus::Conflict)
    }
}

/// `data_path` with `\` separators, or Err unless every component is a plain name: a crafted BSA folder such as
/// `..\x` or `C:\x` must not lead a write out of the edit mod (Windows drops trailing dots and spaces, so `. .` too).
pub fn win(data_path: &str) -> Result<String, String> {
    let w = data_path.replace('/', "\\");
    let bad = |s: &str| s.trim_end_matches(['.', ' ']).is_empty() || s.contains(':');
    match w.split('\\').any(bad) {
        true => Err(format!(
            "This NIF's path cannot be used for an edit: {data_path}"
        )),
        false => Ok(w),
    }
}

/// The draft's stable id: the first 16 hex characters of the SHA-256 of `"{nanos since epoch}-{process id}-{data_path}"`.
pub fn new_id(data_path: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let h = sha256_hex(format!("{nanos}-{}-{data_path}", std::process::id()).as_bytes());
    h[..16].to_string()
}

/// Ruling E6-E: an id read from a file is used only when it has `new_id`'s form, 16 ASCII hex characters; it becomes
/// part of a file name (`kept_autosave_path`), so anything else is no id.
pub fn valid_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Where a NIF edit's draft lives with a project: `<project>\_sdc\nif-edits\<win>.sdc-nifedit.toml`.
pub fn draft_path(project: &Path, data_path: &str) -> Result<PathBuf, String> {
    Ok(project
        .join("_sdc")
        .join("nif-edits")
        .join(format!("{}.sdc-nifedit.toml", win(data_path)?)))
}

/// The record an export leaves in the export folder (`<export folder>\.sdc\nif-edits\<win>.sdc-nifedit.toml`): the
/// document as exported, which `pick_source` (Note GQ3) and the badges read.
pub fn doc_path(edit_mod: &Path, data_path: &str) -> Result<PathBuf, String> {
    Ok(edit_mod
        .join(".sdc")
        .join("nif-edits")
        .join(format!("{}.sdc-nifedit.toml", win(data_path)?)))
}

/// Where an edit with no project open saves its draft, the plain autosave (Review E6, Note GD5 (c)):
/// `<dir>\nif-edits\<SHA-256 of the lowercase data path>.toml`; `dir` is the scan store's folder, `%LOCALAPPDATA%\sdc`
/// in the app. The export folder plays no part.
pub fn autosave_path(dir: &Path, data_path: &str) -> Result<PathBuf, String> {
    let key = win(data_path)?.to_ascii_lowercase();
    Ok(dir
        .join("nif-edits")
        .join(format!("{}.toml", sha256_hex(key.as_bytes()))))
}

/// Where a kept autosave of one draft goes: `<dir>\nif-edits\<SHA-256 of the lowercase data path>.<id>.toml`. It
/// holds the edits a project switch could not save into the project, keyed on the draft's id so no other draft's
/// file is touched.
pub fn kept_autosave_path(dir: &Path, data_path: &str, id: &str) -> PathBuf {
    debug_assert!(valid_id(id), "an unchecked draft id: {id}");
    let key = win(data_path)
        .unwrap_or_else(|_| data_path.replace('/', "\\"))
        .to_ascii_lowercase();
    dir.join("nif-edits")
        .join(format!("{}.{id}.toml", sha256_hex(key.as_bytes())))
}

pub fn export_path(edit_mod: &Path, data_path: &str) -> Result<PathBuf, String> {
    Ok(edit_mod.join(win(data_path)?))
}

/// Writes `doc` to `path` (replacing what is there); Ok(the fingerprint of the bytes written, ruling E6-F).
pub fn save(path: &Path, doc: &EditDoc) -> Result<String, String> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let text = doc.to_text();
    engine::dds::write_atomic(path, text.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(sha256_hex(text.as_bytes()))
}

/// `save` into a project: refuses when `root` is not an existing folder (a renamed, moved or deleted project is never
/// re-created at its old path), then makes the folders under it one level at a time with the non-recursive
/// `create_dir`, so a root that went away in between fails the first one, and checks the root again just before the
/// write (whose own parent creation then finds them made). Ruling E6-E2: the root going away between that last check
/// and the rename is a window std cannot close; it is accepted.
pub fn save_in(root: &Path, path: &Path, doc: &EditDoc) -> Result<String, String> {
    let gone = || format!("{} no longer exists", root.display());
    if !root.is_dir() {
        return Err(gone());
    }
    write_hook("root checked");
    let under = path
        .parent()
        .and_then(|p| p.strip_prefix(root).ok())
        .ok_or_else(|| format!("{} is not under {}", path.display(), root.display()))?;
    let mut dir = root.to_path_buf();
    for c in under.components() {
        dir.push(c);
        match std::fs::create_dir(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && dir.is_dir() => {}
            Err(_) if !root.is_dir() => return Err(gone()),
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        }
    }
    if !root.is_dir() {
        return Err(gone());
    }
    let text = doc.to_text();
    engine::dds::write_atomic(path, text.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(sha256_hex(text.as_bytes()))
}

/// Writes `doc` to a file that does not exist yet, never replacing one (ruling E6-E): a temporary beside it is written
/// and synced, then linked to the name with `hard_link`, which fails when the name exists (a file another session
/// wrote there meanwhile stays as it is), and the temporary is removed. On a volume without hard links the document is
/// written through the handle `create_new` returned (it still never replaces a file, but a crash can leave it short).
/// Ruling E6-E2: two Tessalith instances working one kept autosave at the same moment are not coordinated beyond this.
pub fn save_new(path: &Path, doc: &EditDoc) -> Result<String, String> {
    use std::io::Write as _;
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let err = |e: std::io::Error| format!("{}: {e}", path.display());
    let bytes = doc.to_text();
    static TMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let (tmp, mut f) = loop {
        let n = TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = dir.join(format!(".sdc-kept-{}-{n}.tmp", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(f) => break (tmp, f),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("{}: {e}", tmp.display())),
        }
    };
    let written = f.write_all(bytes.as_bytes()).and_then(|()| f.sync_all());
    drop(f);
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(err(e));
    }
    write_hook("kept written");
    let linked = std::fs::hard_link(&tmp, path);
    let _ = std::fs::remove_file(&tmp); // the name holds the bytes now, or the link failed
    let fp = sha256_hex(bytes.as_bytes());
    match linked {
        Ok(()) => Ok(fp),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(err(e)),
        Err(_) => {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(err)?;
            let written = f.write_all(bytes.as_bytes()).and_then(|()| f.sync_all());
            drop(f);
            written.map(|()| fp).map_err(|e| {
                let _ = std::fs::remove_file(path); // the file this call made
                err(e)
            })
        }
    }
}

#[cfg(feature = "test-hooks")]
thread_local! {
    static LOADS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static HASHES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static WRITE_HOOK: std::cell::RefCell<Option<WriteHook>> = const { std::cell::RefCell::new(None) };
}

/// A test's stand-in for another process acting in a write's window (ruling E6-E's tests).
#[cfg(feature = "test-hooks")]
pub type WriteHook = Box<dyn FnMut(&'static str)>;

/// Runs `f` on this thread at each named point of a draft or kept autosave write: "root checked" (`save_in`, after
/// the project folder was found) and "kept written" (`save_new`, before the file takes its name). None removes it.
#[cfg(feature = "test-hooks")]
pub fn set_write_hook(f: Option<WriteHook>) {
    WRITE_HOOK.with(|h| *h.borrow_mut() = f);
}

fn write_hook(stage: &'static str) {
    #[cfg(feature = "test-hooks")]
    WRITE_HOOK.with(|h| {
        if let Some(f) = h.borrow_mut().as_mut() {
            f(stage);
        }
    });
    #[cfg(not(feature = "test-hooks"))]
    let _ = stage;
}

/// How many `EditDoc::ops_hash` this thread computed (Note GD23's test: idle frames hash nothing).
#[cfg(feature = "test-hooks")]
pub fn ops_hash_count() -> usize {
    HASHES.with(std::cell::Cell::get)
}

/// How many `load` calls this thread made (Note GD20's test: a failed folder change is not read each frame).
#[cfg(feature = "test-hooks")]
pub fn load_count() -> usize {
    LOADS.with(std::cell::Cell::get)
}

pub fn load(path: &Path) -> Result<Option<EditDoc>, String> {
    load_fp(path).map(|d| d.map(|(d, _)| d))
}

/// `load`, with the fingerprint (SHA-256 hex) of the bytes the document was parsed from (ruling E6-F: the session
/// remembers it, and a file is its own only while it still has exactly those bytes).
pub fn load_fp(path: &Path) -> Result<Option<(EditDoc, String)>, String> {
    #[cfg(feature = "test-hooks")]
    LOADS.with(|n| n.set(n.get() + 1));
    match std::fs::read(path) {
        // Note GD30: the file is named, as in the read error (an autosave's name alone is a hash)
        Ok(b) => {
            let fp = sha256_hex(&b);
            let t = String::from_utf8(b).map_err(|e| format!("{}: {e}", path.display()))?;
            EditDoc::from_text(&t)
                .map(|d| Some((d, fp)))
                .map_err(|e| format!("{}: {e}", path.display()))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// The fingerprint of the file at `path` now: Ok(None) when there is none, Err when it cannot be read.
pub fn fingerprint(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read(path) {
        Ok(b) => Ok(Some(sha256_hex(&b))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Ruling E6-F: the file at `path` is as this session knows it: absent, or holding exactly the bytes whose
/// fingerprint is `known` (what the session last loaded or wrote there). Anything else (other bytes, an unreadable
/// file, a file where it knew none) is not the session's to replace or delete.
pub fn is_known(path: &Path, known: Option<&str>) -> bool {
    match fingerprint(path) {
        Ok(None) => true,
        Ok(Some(h)) => known == Some(h.as_str()),
        Err(_) => false,
    }
}

pub fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// Undo/redo over whole-document snapshots (TOML text), each with the short name of the step it undoes (Note GD4).
/// A record with the same merge key as the previous one (one drag) keeps only the first snapshot and its name.
#[derive(Debug, Default)]
pub struct NifHistory {
    undo: Vec<(String, String)>,
    redo: Vec<(String, String)>,
    merge: Option<String>,
}

impl NifHistory {
    pub const LIMIT: usize = 200;

    /// `before` is the document before the step named `label` ("Set shader type").
    pub fn record(&mut self, before: String, label: &str, merge: Option<String>) {
        self.redo.clear();
        if merge.is_some() && merge == self.merge {
            return;
        }
        self.merge = merge;
        self.undo.push((before, label.to_string()));
        if self.undo.len() > Self::LIMIT {
            self.undo.remove(0);
        }
    }

    pub fn break_merge(&mut self) {
        self.merge = None;
    }

    pub fn undo(&mut self, current: String) -> Option<String> {
        let (s, label) = self.undo.pop()?;
        self.redo.push((current, label));
        self.merge = None;
        Some(s)
    }

    pub fn redo(&mut self, current: String) -> Option<String> {
        let (s, label) = self.redo.pop()?;
        self.undo.push((current, label));
        self.merge = None;
        Some(s)
    }

    /// The name of the step Undo would undo.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|(_, l)| l.as_str())
    }

    /// The name of the step Redo would redo.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|(_, l)| l.as_str())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}
