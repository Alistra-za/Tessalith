//! `sdc.toml` as the GUI edits it (M3a spec §5.3–5.4): format-preserving (`toml_edit`), so comments, ordering and
//! hand-written tables survive. The GUI owns only `target`, `preset`, `[files.*]` and `[inputs]`.
use engine::codes::Code;
use engine::config::Target;
use engine::policy::SourceStamp;
use engine::presets::{ConfigFile, ExportConfig};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use toml_edit::{Array, DocumentMut, Item, Table, TableLike, Value};

#[derive(Debug, Clone, PartialEq)]
pub enum SaveError {
    /// The file did not parse when it was opened; the GUI does not write it.
    ReadOnly,
    /// The file changed on disk since it was loaded or saved; save again with `force` to overwrite.
    ChangedOnDisk,
    /// The file has the read-only flag, so it cannot be replaced.
    ReadOnlyFlag,
    Io(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::ReadOnly => {
                f.write_str("the file is read-only until its error is fixed and it is reloaded")
            }
            SaveError::ChangedOnDisk => f.write_str("the file changed on disk since it was loaded"),
            SaveError::ReadOnlyFlag => {
                f.write_str("the file is read-only: clear the read-only flag to save")
            }
            SaveError::Io(e) => write!(f, "could not save: {e}"),
        }
    }
}

/// The state both project files share (REPORT #15): the document, its disk stamp, unsaved-edit flag, load error and
/// revision. `ProjectDoc` and `SetsDoc` wrap it; every edit ends with `touch()`, so `rev` names the edited document.
#[derive(Default)]
pub(crate) struct TomlFile {
    pub(crate) path: PathBuf,
    pub(crate) doc: DocumentMut,
    disk: Option<SourceStamp>,
    pub(crate) dirty: bool,
    /// The text last loaded or saved: `restore` compares with it, so an Undo back to it is clean (Note FB3).
    pub(crate) saved_text: String,
    /// The saved text with subsequent automatic decisions applied. Undo compares against it for the title and prompts.
    pub(crate) baseline_text: String,
    /// The other automatic baseline while the undoable fuzz switch is on the history stack.
    before_switch: Option<String>,
    pub(crate) user_dirty: bool,
    pub(crate) error: Option<String>,
    /// Changes with every edit, restore and reload, from one counter for every document (DECISIONS Note Y7).
    pub(crate) rev: u64,
}

impl TomlFile {
    pub(crate) fn open(path: PathBuf) -> TomlFile {
        let mut f = TomlFile {
            path,
            ..Default::default()
        };
        f.reload();
        f
    }

    /// Re-read the file, dropping unsaved edits.
    pub(crate) fn reload(&mut self) {
        self.dirty = false;
        self.error = None;
        self.doc = DocumentMut::new();
        self.disk = SourceStamp::of(&self.path);
        match std::fs::read_to_string(&self.path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => self.error = Some(format!("{}: {e}", self.path.display())),
            Ok(text) => match text.parse::<DocumentMut>() {
                Ok(doc) => self.doc = doc,
                Err(e) => self.error = Some(format!("{}: {e}", self.path.display())),
            },
        }
        self.saved_text = self.doc.to_string();
        self.baseline_text.clone_from(&self.saved_text);
        self.before_switch = None;
        self.user_dirty = false;
        self.rev = crate::sets_doc::next_rev();
    }

    /// An unsaved edit, made just before.
    pub(crate) fn touch(&mut self) {
        self.dirty = true;
        self.update_user_dirty(&self.text());
        self.rev = crate::sets_doc::next_rev();
    }

    pub(crate) fn read_only(&self) -> bool {
        self.error.is_some()
    }

    pub(crate) fn text(&self) -> String {
        self.doc.to_string()
    }

    /// Replace the document with `text` (an undo, a redo or recovered edits): an unsaved change unless `text` is the
    /// saved text (Note FB3). The one place that recomputes `dirty` from the text. A read-only document is left alone:
    /// its edits never happened.
    pub(crate) fn restore(&mut self, text: &str) {
        if self.read_only() {
            return;
        }
        if let Ok(doc) = text.parse::<DocumentMut>() {
            self.doc = doc;
            self.rev = crate::sets_doc::next_rev();
            self.reconcile(&self.text());
        }
    }

    pub(crate) fn reconcile(&mut self, text: &str) {
        self.dirty = text != self.saved_text;
        self.update_user_dirty(text);
    }

    pub(crate) fn update_user_dirty(&mut self, text: &str) {
        self.user_dirty = self.dirty
            && text != self.baseline_text
            && text != self.saved_text
            && self.before_switch.as_deref() != Some(text);
    }

    pub(crate) fn changed_on_disk(&self) -> bool {
        SourceStamp::of(&self.path) != self.disk
    }

    /// Write the document (atomically: a temporary file, then rename).
    pub(crate) fn save(&mut self, force: bool) -> Result<(), SaveError> {
        if self.read_only() {
            return Err(SaveError::ReadOnly);
        }
        self.write_text(self.doc.to_string(), force)?;
        // The `.recovery` file stays: only a user's Save holds the recovered edits (`AppModel::save`, wave 2 #3).
        self.dirty = false;
        Ok(())
    }

    /// The changed-on-disk check, then the atomic write of `text`, which becomes the saved text.
    fn write_text(&mut self, text: String, force: bool) -> Result<(), SaveError> {
        if !force && self.changed_on_disk() {
            return Err(SaveError::ChangedOnDisk);
        }
        engine::dds::write_atomic(&self.path, text.as_bytes())
            .map_err(|e| write_error(&self.path, e))?;
        self.baseline_text.clone_from(&text);
        self.before_switch = None;
        self.user_dirty = false;
        self.saved_text = text;
        self.disk = SourceStamp::of(&self.path);
        Ok(())
    }
}

/// A planner decision may fill an absent field or replace a particular previously recorded value.
#[derive(Clone)]
pub struct AutoWrite {
    pub key: String,
    pub field: &'static str,
    pub value: Json,
    pub only_if: Option<Json>,
}

/// Use the live editor's key matching, dotted fields and comment preservation on a history snapshot.
pub fn apply_auto(text: &str, writes: &[AutoWrite]) -> String {
    let Ok(doc) = text.parse() else {
        return text.to_owned();
    };
    let mut project = ProjectDoc {
        f: TomlFile {
            doc,
            ..Default::default()
        },
        stash: None,
        export_cache: Default::default(),
    };
    project.apply_writes(writes);
    project.text()
}

/// The editor's spelling of a batch of new file entries. For simple snapshots the editor appends
/// these same bytes, so Undo can receive them without rebuilding thousands of unrelated tables.
pub(crate) struct NewFileEntries {
    names: Vec<String>,
    tables: String,
}

impl NewFileEntries {
    pub(crate) fn new(writes: &[AutoWrite]) -> Option<Self> {
        if writes.is_empty()
            || writes
                .iter()
                .any(|w| w.only_if.is_some() || w.value.is_null())
        {
            return None;
        }
        let mut names: Vec<_> = writes
            .iter()
            .map(|w| {
                // Match find_key's order: a following extension can affect Unicode lowercasing.
                let lower = w.key.to_lowercase();
                without_ext(&lower).map(str::to_owned).unwrap_or(lower)
            })
            .collect();
        names.sort_unstable();
        names.dedup();
        let tables = apply_auto("", writes);
        (!tables.is_empty()).then_some(Self { names, tables })
    }

    pub(crate) fn appended(&self, text: &str) -> Option<String> {
        let mut out = String::new();
        if !text.is_empty() {
            let folded = text.to_lowercase();
            if self.names.iter().any(|key| folded.contains(key)) || !files_append_at_end(text) {
                return None;
            }
            out.reserve_exact(text.len() + self.tables.len() + 2);
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str(&self.tables);
        Some(out)
    }
}

/// History contains valid TOML. In this layout all file sections are flat, consecutive and last.
/// Other layouts, escaped keys, and trailing decorations require the editor's placement rules.
fn files_append_at_end(text: &str) -> bool {
    let body = text.strip_suffix('\n').unwrap_or(text);
    if body.is_empty()
        || body.ends_with('\n')
        || body.as_bytes().contains(&b'\r')
        || body.as_bytes().contains(&b'\\')
        || body.contains('{')
        || body.contains("\"\"\"")
        || body.contains("'''")
    {
        return false;
    }
    let bare = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    };
    let mut in_section = false;
    let mut in_files = false;
    for line in body.split('\n') {
        if line.is_empty() {
            continue;
        }
        if let Some(key) = line
            .strip_prefix("[files.\"")
            .and_then(|s| s.strip_suffix("\"]"))
        {
            if key.contains('"') {
                return false;
            }
            in_files = true;
            in_section = true;
        } else if let Some(table) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            if in_files || table == "files" || !bare(table) {
                return false;
            }
            in_section = true;
        } else {
            let Some((field, _)) = line.split_once('=') else {
                return false;
            };
            let field = field.trim_end();
            // The editor groups dotted fields even when another assignment separates them.
            if !bare(field) || (!in_section && field == "files") {
                return false;
            }
        }
    }
    true
}

pub struct ProjectDoc {
    f: TomlFile,
    /// The document and dirty flag `set_error` dropped, kept until `unblame` or `reload` (trace-debug #9).
    stash: Option<(DocumentMut, bool)>,
    /// `[export]` as last parsed, by `rev` (Note CN: Review and `view_key` read it every frame).
    export_cache: std::cell::RefCell<(Option<u64>, ExportConfig)>,
}

/// `v` without the fields equal to `d`'s, recursively; an object left empty goes too. A field that `written` (the
/// table as it is in the file) holds with the same value stays, even at its default: the edit did not touch it, so
/// the user may have pinned it (final review M3). A field the edit sets to its default goes, as before (CN60).
fn prune(v: &mut Json, d: &Json, written: Option<&Json>) {
    if let (Json::Object(o), Json::Object(d)) = (v, d) {
        o.retain(|k, x| {
            let Some(dx) = d.get(k) else { return true };
            let w = written.and_then(|w| w.get(k));
            if x.is_object() {
                prune(x, dx, w);
                return !matches!(x, Json::Object(m) if m.is_empty());
            }
            x != dx || w == Some(&*x)
        });
    }
}

pub(crate) fn json_to_toml(v: &Json) -> Option<Value> {
    Some(match v {
        Json::Bool(b) => Value::from(*b),
        Json::Number(n) => match n.as_i64() {
            Some(i) => Value::from(i),
            None => Value::from(n.as_f64()?),
        },
        Json::String(s) => Value::from(s.as_str()),
        Json::Array(a) => {
            let mut arr = Array::new();
            for x in a {
                arr.push(json_to_toml(x)?);
            }
            Value::Array(arr)
        }
        Json::Null | Json::Object(_) => return None,
    })
}

pub(crate) fn toml_to_json(v: &Item) -> Option<Json> {
    let text = match v {
        Item::Value(v) => format!("v = {v}"),
        _ => return None,
    };
    let t: toml::Table = toml::from_str(&text).ok()?;
    serde_json::to_value(&t["v"]).ok()
}

/// A TOML item as JSON; a `[table]` (or dotted keys) becomes an object, as an inline table does.
pub(crate) fn item_json(item: &Item) -> Option<Json> {
    match item {
        Item::Table(t) => Some(Json::Object(
            t.iter()
                .filter_map(|(k, i)| Some((k.to_string(), item_json(i)?)))
                .collect(),
        )),
        _ => toml_to_json(item),
    }
}

/// A new table whose keys print as dotted keys (`alpha.policy = ...`) inside their parent.
pub(crate) fn dotted_table() -> Item {
    let mut t = Table::new();
    t.set_dotted(true);
    Item::Table(t)
}

/// `t[name]` as a table or inline table, created with `make()` when missing. `None` when it exists but is a
/// plain value. (`insert` rather than `entry`: inserting a `Table` item into an inline table converts it.)
pub(crate) fn sub_table<'a>(
    t: &'a mut dyn TableLike,
    name: &str,
    make: impl FnOnce() -> Item,
) -> Option<&'a mut dyn TableLike> {
    if !t.contains_key(name) {
        t.insert(name, make());
    }
    t.get_mut(name)?.as_table_like_mut()
}

/// Set `t[key] = v`, replacing an existing item in place so the key keeps its comments and position; a comment on
/// the value's own line (its decor) is carried over (audit 19).
pub(crate) fn set_value(t: &mut dyn TableLike, key: &str, mut v: Value) {
    match t.get_mut(key) {
        Some(slot) => {
            if let Some(old) = slot.as_value() {
                *v.decor_mut() = old.decor().clone();
            }
            *slot = Item::Value(v);
        }
        None => {
            t.insert(key, Item::Value(v));
        }
    }
}

/// The spelling `key` has in `t`: an exact match, else a case-insensitive one, else one without the extension (the
/// engine matches `[files]` keys that way, so a GUI write must reuse a hand-written `Landscape/Rock01_N.png` or
/// `Landscape/Rock01_N` rather than add a twin).
pub(crate) fn find_key(t: &dyn TableLike, key: &str) -> Option<String> {
    if t.contains_key(key) {
        return Some(key.to_owned());
    }
    let lower = key.to_lowercase();
    let by = |want: &str| {
        t.iter()
            .map(|(k, _)| k)
            .find(|k| k.to_lowercase() == want)
            .map(str::to_owned)
    };
    // The engine also matches a key written without the file's extension (`path_key_matches`; trace #29).
    by(&lower).or_else(|| by(without_ext(&lower)?))
}

impl ProjectDoc {
    pub fn open(root: &Path) -> ProjectDoc {
        let mut d = ProjectDoc {
            f: TomlFile::open(root.join("sdc.toml")),
            stash: None,
            export_cache: Default::default(),
        };
        d.check();
        d
    }

    /// Re-read the file, dropping unsaved edits.
    pub fn reload(&mut self) {
        self.stash = None;
        self.f.reload();
        self.check();
    }

    /// The text of the stashed document when it held unsaved edits.
    pub fn stashed_edits(&self) -> Option<String> {
        self.stash
            .as_ref()
            .filter(|(_, dirty)| *dirty)
            .map(|(doc, _)| doc.to_string())
    }

    /// `stashed_edits().is_some()` without building the text.
    pub fn has_stashed_edits(&self) -> bool {
        self.stash.as_ref().is_some_and(|(_, dirty)| *dirty)
    }

    /// `set_error` holds the document (Note FC12: undoing the rejected step puts it back first, with `unblame`).
    pub fn blamed(&self) -> bool {
        self.stash.is_some()
    }

    /// Undo a `set_error` that blamed the file wrongly: its document and unsaved edits come back (or, with nothing
    /// stashed, the file is re-read).
    pub fn unblame(&mut self) {
        match self.stash.take() {
            Some((doc, dirty)) => {
                self.f.doc = doc;
                self.f.dirty = dirty;
                self.f.error = None;
                self.f.rev = crate::sets_doc::next_rev();
            }
            None => self.reload(),
        }
    }

    /// The engine's own up-front checks on `[files]` (unknown field or code) are parse errors too.
    fn check(&mut self) {
        if self.f.read_only() {
            return;
        }
        let checked = self
            .config()
            .and_then(|c| c.file_entries().map(|_| ()).map_err(|e| e.to_string()));
        if let Err(e) = checked {
            self.set_error(&e);
        }
    }

    /// Mark the file unusable (M3a spec §7.4): read-only with `message`, its contents and unsaved edits dropped. Used
    /// when the engine rejects a config that parsed; `reload` clears it. The dropped document is stashed for `unblame`.
    pub fn set_error(&mut self, message: &str) {
        self.f.error = Some(format!("{}: {message}", self.f.path.display()));
        let doc = std::mem::take(&mut self.f.doc);
        self.stash = Some((doc, std::mem::take(&mut self.f.dirty)));
        self.f.rev = crate::sets_doc::next_rev();
    }

    pub fn path(&self) -> &Path {
        &self.f.path
    }
    pub fn is_dirty(&self) -> bool {
        self.f.dirty
    }
    /// Put the flag back after a write that left the text as it was.
    pub fn set_dirty(&mut self, dirty: bool) {
        self.f.dirty = dirty;
        self.update_user_dirty();
    }

    fn update_user_dirty(&mut self) {
        if !self.blamed() {
            self.f.update_user_dirty(&self.text());
        }
    }
    pub fn user_dirty(&self) -> bool {
        self.f.user_dirty && !self.blamed()
    }

    pub(crate) fn stashed_user_edit(&self) -> bool {
        self.f.user_dirty && self.blamed()
    }

    fn apply_writes(&mut self, writes: &[AutoWrite]) -> bool {
        let mut changed = false;
        for w in writes {
            if self.file_value(&w.key, w.field) == w.only_if {
                changed |= if w.value.is_null() {
                    self.remove_field(&w.key, w.field)
                } else {
                    self.write_field(&w.key, w.field, &w.value)
                };
            }
        }
        changed
    }

    pub(crate) fn touch_auto(&mut self, writes: &[AutoWrite]) {
        if let Some(before) = &mut self.f.before_switch {
            *before = apply_auto(before, writes);
        }
        self.record_baseline(writes);
    }

    pub(crate) fn touch_switch(&mut self, writes: &[AutoWrite]) {
        self.f
            .before_switch
            .get_or_insert_with(|| self.f.baseline_text.clone());
        self.record_baseline(writes);
    }

    fn record_baseline(&mut self, writes: &[AutoWrite]) {
        let changed = self.apply_writes(writes);
        self.f.baseline_text = apply_auto(&self.f.baseline_text, writes);
        if changed {
            self.touch();
        } else {
            self.update_user_dirty();
        }
    }

    pub fn rev(&self) -> u64 {
        self.f.rev
    }
    fn touch(&mut self) {
        self.f.touch();
    }
    pub fn read_only(&self) -> bool {
        self.f.read_only()
    }
    pub fn error(&self) -> Option<&str> {
        self.f.error.as_deref()
    }

    /// The document as text (undo snapshots, DECISIONS P9).
    pub fn text(&self) -> String {
        self.f.text()
    }

    /// [`Self::text`] for an undo step: while a blame holds the document, the stashed text it stands for.
    pub fn snapshot_text(&self) -> String {
        match &self.stash {
            Some((doc, _)) => doc.to_string(),
            None => self.f.text(),
        }
    }

    /// Replace the document with `text` (an undo or redo). It is an unsaved change. A read-only document is left
    /// alone: its edits never happened.
    pub fn restore(&mut self, text: &str) {
        self.f.restore(text);
    }

    pub(crate) fn restore_snapshot(&mut self, text: &str) {
        if self.read_only() {
            return;
        }
        if self.text() == text {
            self.f.reconcile(text);
        } else {
            self.restore(text);
        }
    }

    /// The document as the engine parses it (version and removed-key checks included, Note FM10).
    pub fn config(&self) -> Result<ConfigFile, String> {
        ConfigFile::parse(&self.f.doc.to_string(), "sdc.toml").map_err(|e| e.to_string())
    }

    /// `target`, read from the document (no parse: the unmatched-entries row asks every frame, Note JI6).
    pub fn target(&self) -> Option<Target> {
        let s = self.f.doc.get("target")?.as_str()?;
        serde_json::from_value(Json::String(s.to_string())).ok()
    }
    pub fn set_target(&mut self, t: Target) {
        if self.read_only() {
            return;
        }
        let s = serde_json::to_value(t).ok();
        if let Some(Json::String(s)) = s {
            set_value(self.f.doc.as_table_mut(), "target", Value::from(s));
            self.touch();
        }
    }

    /// `preset`, read from the document (no parse: the header asks every frame).
    pub fn preset(&self) -> Option<String> {
        self.f.doc.get("preset")?.as_str().map(str::to_string)
    }

    /// `preset = "<id>"`, or removed with `None` (the target's default preset then applies).
    pub fn set_preset(&mut self, id: Option<&str>) {
        if self.read_only() {
            return;
        }
        match id {
            Some(id) => set_value(self.f.doc.as_table_mut(), "preset", Value::from(id)),
            None => {
                self.f.doc.as_table_mut().remove("preset");
            }
        }
        self.touch();
    }

    /// The `[files]` table (or inline table), if there is one.
    fn files(&self) -> Option<&dyn TableLike> {
        self.f.doc.get("files")?.as_table_like()
    }

    /// The entry for `key`, matched the way the engine matches (case-insensitively), as a table or inline table.
    fn entry(&self, key: &str) -> Option<&dyn TableLike> {
        let files = self.files()?;
        files.get(&find_key(files, key)?)?.as_table_like()
    }

    /// The entry for `key`, created as `[files."<key>"]` when no spelling of it exists yet. `None` when `[files]`
    /// or the entry is not a table.
    fn entry_mut(&mut self, key: &str) -> Option<&mut dyn TableLike> {
        // Note FC1: a file of comments only holds them as the document's trailing text, which toml_edit writes after
        // a table added here; they go in front of the new entry instead, so the user's first line stays first.
        let mut lead = None;
        if self.f.doc.as_table().is_empty() {
            let t = self
                .f
                .doc
                .trailing()
                .as_str()
                .unwrap_or_default()
                .to_owned();
            if !t.trim().is_empty() {
                self.f.doc.set_trailing("");
                lead = Some(t);
            }
        }
        // `[files]` is created implicit (no bare `[files]` header).
        let files = sub_table(self.f.doc.as_table_mut(), "files", || {
            let mut t = Table::new();
            t.set_implicit(true);
            Item::Table(t)
        })?;
        let name = find_key(files, key).unwrap_or_else(|| key.to_owned());
        sub_table(files, &name, || {
            let mut t = Table::new();
            if let Some(l) = lead {
                t.decor_mut().set_prefix(l);
            }
            Item::Table(t)
        })
    }

    /// Remove the entry for `key` (any spelling of it); true when one existed.
    fn drop_entry(&mut self, key: &str) -> bool {
        let Some(files) = self
            .f
            .doc
            .get_mut("files")
            .and_then(Item::as_table_like_mut)
        else {
            return false;
        };
        let Some(name) = find_key(files, key) else {
            return false;
        };
        let Some(gone) = files.remove(&name) else {
            return false;
        };
        // Note FC1: the comments `entry_mut` put in front of the only entry go back to the document's trailing text.
        let lead = gone
            .as_table()
            .and_then(|t| t.decor().prefix()?.as_str().map(str::to_owned));
        if let Some(lead) = lead.filter(|l| !l.trim().is_empty()) {
            if files.is_empty() && self.f.doc.len() == 1 {
                let rest = self
                    .f
                    .doc
                    .trailing()
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                self.f.doc.set_trailing(lead + &rest);
            }
        }
        true
    }

    pub fn file_value(&self, key: &str, field: &str) -> Option<Json> {
        let mut item: &Item = self.entry(key)?.get(field.split('.').next()?)?;
        for part in field.split('.').skip(1) {
            item = item.get(part)?;
        }
        toml_to_json(item)
    }

    pub fn set_file_value(&mut self, key: &str, field: &str, value: &Json) {
        if self.write_field(key, field, value) {
            self.touch();
        }
    }

    fn write_field(&mut self, key: &str, field: &str, value: &Json) -> bool {
        if self.read_only() {
            return false;
        }
        let Some(v) = json_to_toml(value) else {
            return false;
        };
        let parts: Vec<&str> = field.split('.').collect();
        let (last, path) = parts.split_last().expect("split yields one part at least");
        let written = (|| {
            let mut t = self.entry_mut(key)?;
            for part in path {
                t = sub_table(t, part, dotted_table)?;
            }
            set_value(t, last, v);
            Some(())
        })();
        if written.is_none() && self.entry(key).is_some_and(|t| t.is_empty()) {
            // A field path ran into a non-table: do not leave the entry we just created behind.
            self.drop_entry(key);
        }
        written.is_some()
    }

    pub fn clear_file_value(&mut self, key: &str, field: &str) {
        if self.remove_field(key, field) {
            self.touch();
        }
    }

    fn remove_field(&mut self, key: &str, field: &str) -> bool {
        if self.read_only() || self.entry(key).is_none() {
            return false;
        }
        fn remove(t: &mut dyn TableLike, parts: &[&str]) -> bool {
            if parts.len() == 1 {
                return t.remove(parts[0]).is_some();
            }
            let Some(sub) = t.get_mut(parts[0]).and_then(Item::as_table_like_mut) else {
                return false;
            };
            let hit = remove(sub, &parts[1..]);
            if sub.is_empty() {
                t.remove(parts[0]);
            }
            hit
        }
        let parts: Vec<&str> = field.split('.').collect();
        let removed = self
            .entry_mut(key)
            .is_some_and(|entry| remove(entry, &parts));
        if self.entry(key).is_some_and(|t| t.is_empty()) {
            return self.drop_entry(key);
        }
        removed
    }

    /// Every value in `key`'s entry by its dotted field name (`cubemap.yaw`), as `file_value` reads it.
    pub fn entry_fields(&self, key: &str) -> BTreeMap<String, Json> {
        fn walk(prefix: String, v: Json, out: &mut BTreeMap<String, Json>) {
            match v {
                Json::Object(o) => {
                    for (k, v) in o {
                        walk(format!("{prefix}.{k}"), v, out);
                    }
                }
                v => {
                    out.insert(prefix, v);
                }
            }
        }
        let mut out = BTreeMap::new();
        for (k, item) in self.entry(key).into_iter().flat_map(|t| t.iter()) {
            if let Some(v) = item_json(item) {
                walk(k.to_string(), v, &mut out);
            }
        }
        out
    }

    /// Repeat on each of `to` what changed in `from`'s entry since it read `before` (its `entry_fields` then).
    pub fn copy_entry_changes(
        &mut self,
        from: &str,
        before: &BTreeMap<String, Json>,
        to: &[String],
    ) {
        let after = self.entry_fields(from);
        for k in to {
            for (f, v) in after.iter().filter(|(f, v)| before.get(*f) != Some(*v)) {
                self.set_file_value(k, f, v);
            }
            for f in before.keys().filter(|f| !after.contains_key(*f)) {
                self.clear_file_value(k, f);
            }
        }
    }

    pub fn allow(&self, key: &str) -> BTreeSet<Code> {
        self.file_value(key, "allow")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|c| c.as_str().and_then(Code::parse))
            .collect()
    }

    pub fn set_allow(&mut self, key: &str, codes: &BTreeSet<Code>) {
        if codes.is_empty() {
            self.clear_file_value(key, "allow");
        } else {
            let list: Vec<Json> = codes.iter().map(|c| Json::from(c.as_str())).collect();
            self.set_file_value(key, "allow", &Json::Array(list));
        }
    }

    /// The spelling of file `key`'s entry in `[inputs]`: `key` in any case, or without its extension.
    fn read_as_key(&self, key: &str) -> Option<String> {
        let t = self.f.doc.get("inputs")?.as_table_like()?;
        find_key(t, key)
    }

    /// F3b: the path file `key` is read as (its `[inputs]` alias), if it has one.
    pub fn read_as(&self, key: &str) -> Option<String> {
        let name = self.read_as_key(key)?;
        self.f
            .doc
            .get("inputs")?
            .get(&name)?
            .get("as")?
            .as_str()
            .map(str::to_string)
    }

    /// Every `[inputs]` alias, by the key as written: the table the engine validates all at once.
    pub fn read_as_all(&self) -> BTreeMap<String, engine::presets::ReadAs> {
        let Some(t) = self.f.doc.get("inputs").and_then(Item::as_table_like) else {
            return BTreeMap::new();
        };
        t.iter()
            .filter_map(|(k, v)| {
                let path = v.get("as")?.as_str()?.to_string();
                Some((k.to_string(), engine::presets::ReadAs { path }))
            })
            .collect()
    }

    /// F3b: set file `key`'s alias to `alias`, or with `None` remove its entry (an empty `[inputs]` goes too).
    pub fn set_read_as(&mut self, key: &str, alias: Option<&str>) {
        if self.read_only() {
            return;
        }
        let existing = self.read_as_key(key);
        match alias {
            Some(a) => {
                let Some(inputs) = sub_table(self.f.doc.as_table_mut(), "inputs", || {
                    let mut t = Table::new();
                    t.set_implicit(true);
                    Item::Table(t)
                }) else {
                    return;
                };
                let name = existing.unwrap_or_else(|| key.to_owned());
                let Some(entry) = sub_table(inputs, &name, || Item::Table(Table::new())) else {
                    return;
                };
                set_value(entry, "as", Value::from(a));
            }
            None => {
                let Some(name) = existing else { return };
                let Some(inputs) = self
                    .f
                    .doc
                    .get_mut("inputs")
                    .and_then(Item::as_table_like_mut)
                else {
                    return;
                };
                inputs.remove(&name);
                if inputs.is_empty() {
                    self.f.doc.as_table_mut().remove("inputs");
                }
            }
        }
        self.touch();
    }

    pub fn file_keys(&self) -> Vec<String> {
        self.files()
            .map(|t| t.iter().map(|(k, _)| k.to_string()).collect())
            .unwrap_or_default()
    }

    /// Whether `[files]` has an entry spelled exactly `key` (as `file_keys` lists it), without copying the keys.
    pub fn has_file_key(&self, key: &str) -> bool {
        self.files().is_some_and(|t| t.contains_key(key))
    }

    pub fn remove_file_entry(&mut self, key: &str) {
        if self.read_only() {
            return;
        }
        if self.drop_entry(key) {
            self.touch();
        }
    }

    /// `[export]` (Track A, TA9), the default when absent or unparsable. Cached by `rev`.
    pub fn export_config(&self) -> ExportConfig {
        self.with_export_config(Clone::clone)
    }

    pub(crate) fn with_export_config<T>(&self, read: impl FnOnce(&ExportConfig) -> T) -> T {
        let rev = self.rev();
        if self.export_cache.borrow().0 != Some(rev) {
            let config = self
                .config()
                .ok()
                .and_then(|c| c.export)
                .unwrap_or_default();
            *self.export_cache.borrow_mut() = (Some(rev), config);
        }
        read(&self.export_cache.borrow().1)
    }

    /// Write `[export]` as `cfg`: only the fields that differ from the default, so a tick does not freeze today's
    /// recipe defaults into the file (CN60); none at all removes the table. No edit when nothing changes.
    // shortcut: the table is rewritten, so comments inside `[export]` are lost on a GUI edit; upgrade to per-key
    // `set_value` if users hand-annotate it.
    pub fn set_export_config(&mut self, cfg: &ExportConfig) {
        if self.read_only() || self.export_config() == *cfg {
            return;
        }
        let (Ok(mut v), Ok(d)) = (
            serde_json::to_value(cfg),
            serde_json::to_value(ExportConfig::default()),
        ) else {
            return;
        };
        let written = toml::from_str::<Json>(&self.f.doc.to_string())
            .ok()
            .and_then(|mut t| t.get_mut("export").map(Json::take));
        prune(&mut v, &d, written.as_ref());
        let root = self.f.doc.as_table_mut();
        if v.as_object().is_none_or(|o| o.is_empty()) {
            root.remove("export");
        } else {
            let text = toml::to_string(&serde_json::json!({ "export": v })).unwrap_or_default();
            let Some(item) = text
                .parse::<DocumentMut>()
                .ok()
                .and_then(|mut d| d.remove("export"))
            else {
                return;
            };
            root.insert("export", item);
        }
        self.touch();
    }

    /// C2 (TA9): leave set `set_id` out of Vanilla and CM (`on`), or put it back.
    pub fn set_skip_also(&mut self, set_id: &str, on: bool) {
        let mut c = self.export_config();
        c.skip.retain(|s| !engine::sets::same_id(s, set_id));
        if on {
            c.skip.push(set_id.to_string());
        }
        self.set_export_config(&c);
    }

    /// TA31: a renamed set keeps its opt-out.
    pub fn rename_skip_also(&mut self, old: &str, new: &str) {
        let mut c = self.export_config();
        for s in c.skip.iter_mut().filter(|s| engine::sets::same_id(s, old)) {
            *s = new.to_string();
        }
        self.set_export_config(&c);
    }

    pub fn changed_on_disk(&self) -> bool {
        self.f.changed_on_disk()
    }

    /// Write the document (atomically: a temporary file, then rename).
    pub fn save(&mut self, force: bool) -> Result<(), SaveError> {
        self.f.save(force)
    }

    /// Note FP7 (T38): write the unsaved edits a blame stashed (the engine may still reject them: the file then opens
    /// read-only with that message). `Ok(false)` when nothing is stashed.
    pub fn save_stashed(&mut self) -> Result<bool, SaveError> {
        let Some((doc, dirty)) = self.stash.as_mut().filter(|(_, dirty)| *dirty) else {
            return Ok(false);
        };
        self.f.write_text(doc.to_string(), false)?;
        *dirty = false;
        Ok(true)
    }
}

/// `key` without the extension of its file name (`a/b.png` → `a/b`), as the engine also matches keys.
fn without_ext(key: &str) -> Option<&str> {
    let dir_len = key.rfind('/').map_or(0, |i| i + 1);
    let dot = key[dir_len..].rfind('.').filter(|&d| d > 0)?;
    Some(&key[..dir_len + dot])
}

/// A failed write of a project file: a file with the read-only flag says so (Windows refuses to replace it).
pub(crate) fn write_error(path: &Path, e: std::io::Error) -> SaveError {
    let flagged = std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly());
    if e.kind() == std::io::ErrorKind::PermissionDenied && flagged {
        SaveError::ReadOnlyFlag
    } else {
        SaveError::Io(e.to_string())
    }
}
