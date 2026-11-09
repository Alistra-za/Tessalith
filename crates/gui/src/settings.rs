//! GUI-only settings (M3a spec §5.5): `%APPDATA%\sdc\gui.toml`. Never part of `sdc.toml`.
use serde::{Deserialize, Serialize};
mod layout;
pub use layout::*;

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GuiSettings {
    pub export_folder: Option<PathBuf>,
    /// Most recent first, at most 10.
    pub recent: Vec<PathBuf>,
    /// Live preview encoding; off on a fresh install (M3a spec §6.3).
    pub live_encode: bool,
    /// The Textures and Sets columns: shown at all (the header's checkboxes), and collapsed to a strip (the
    /// arrows on each column).
    pub textures_hidden: bool,
    pub sets_hidden: bool,
    pub textures_collapsed: bool,
    pub sets_collapsed: bool,
    /// A set shows its 3D preview instead of its slot cards (the set editor's 2D | 3D switch).
    pub set_view_3d: bool,
    /// The 3D view's settings window (TruePBR settings…) is open (AG132).
    pub set_settings_open: bool,
    pub advanced_open: bool,
    /// The Meshes view's source (plan A Task 12), `[meshes]` in the file.
    pub meshes: MeshesSettings,
    /// The window and arrangement the app comes back to, `[layout]` in the file (DECISIONS AG3).
    #[serde(deserialize_with = "layout::lenient")]
    pub layout: LayoutSettings,
    /// The artist checks' switches (DECISIONS Note CL), `[checks]` in the file.
    pub checks: ChecksSettings,
    /// Note FC12 (DO4–DO6): the Match selector a user chose, by project root and set id (`model::input_name::match_key`).
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub match_slots: std::collections::BTreeMap<String, u8>,
    /// Note HC4: the kinds of Problems row a user hid, by project root (lower case, `/`): code names.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub problems_hidden: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
    /// Note HF4: the set groups a user collapsed, by project root and group name (`model::input_name::match_key`). A
    /// group that is open is not stored.
    #[serde(skip_serializing_if = "std::collections::BTreeSet::is_empty")]
    pub group_collapsed: std::collections::BTreeSet<String>,
}

/// `[checks]`: one switch per artist check; an off check's results are hidden everywhere. All on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChecksSettings {
    pub low_detail: bool,
    pub green: bool,
    pub parallax: bool,
}

impl Default for ChecksSettings {
    fn default() -> Self {
        ChecksSettings {
            low_detail: true,
            green: true,
            parallax: true,
        }
    }
}

/// `[meshes]`: the MO2 instance and profile the Meshes view scans, or the folder chosen instead.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MeshesSettings {
    pub instance: Option<PathBuf>,
    pub profile: Option<String>,
    pub folder: Option<PathBuf>,
}

/// `%APPDATA%\sdc\gui.toml`.
pub fn settings_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("sdc").join("gui.toml"))
}

impl GuiSettings {
    /// Defaults when the file is missing; defaults and the error text when it cannot be read or parsed.
    pub fn load(path: &Path) -> (GuiSettings, Option<String>) {
        match std::fs::read_to_string(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (GuiSettings::default(), None),
            Err(e) => (
                GuiSettings::default(),
                Some(format!("{}: {e}", path.display())),
            ),
            Ok(text) => match toml::from_str::<GuiSettings>(&text) {
                // Keys of removed settings (Note FM25) are ignored here and left out of the next save.
                Ok(mut s) => {
                    s.layout.sanitize();
                    (s, None)
                }
                Err(e) => (
                    GuiSettings::default(),
                    Some(format!("{}: {e}", path.display())),
                ),
            },
        }
    }
    /// Write the file atomically (a temporary file, then rename). A file that exists but cannot be parsed (the user's
    /// hand edit that `load` reported) is kept as `gui.toml.bad` first, so the first save does not destroy it.
    /// Returns the `.bad` path when a broken file was kept.
    pub fn save(&self, path: &Path) -> std::io::Result<Option<PathBuf>> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // #45: anything `load` could not read (not UTF-8, unparsable) is kept, not only a parse error.
        let unreadable = path.exists() && GuiSettings::load(path).1.is_some();
        let kept = path.with_extension("toml.bad");
        if unreadable {
            // Review M3: `rename` replaces an earlier `.bad` only once the move succeeds; a locked file keeps it.
            std::fs::rename(path, &kept)?;
        }
        // TOML holds Unicode only: a path that is not (a lone surrogate in a folder name) is left out of the file
        // rather than failing every save (trace #31).
        let mut s = self.clone();
        let ok = |p: &PathBuf| p.to_str().is_some();
        s.recent.retain(ok);
        for p in [
            &mut s.export_folder,
            &mut s.meshes.instance,
            &mut s.meshes.folder,
        ] {
            *p = p.take().filter(ok);
        }
        let text = toml::to_string_pretty(&s).map_err(std::io::Error::other)?;
        engine::dds::write_atomic(path, text.as_bytes())?;
        Ok(unreadable.then_some(kept))
    }

    /// [`Self::save`] that keeps hand edits made while the app runs (audit 2026-10-15 item 15). `base` is what was
    /// last loaded or written and the file's stamp then; when the file changed since, it is read again and every
    /// field the app has not changed since `base` takes the file's value. A file that is gone or does not parse is
    /// not merged. `base` is updated after a write.
    pub fn save_merged(
        &mut self,
        path: &Path,
        base: &mut Option<(GuiSettings, Option<engine::policy::SourceStamp>)>,
    ) -> std::io::Result<Option<PathBuf>> {
        let now = engine::policy::SourceStamp::of(path);
        if let Some((b, _)) = base.as_ref().filter(|(_, s)| now.is_some() && *s != now) {
            if let (disk, None) = GuiSettings::load(path) {
                if let Some(merged) = merge(b, self, &disk) {
                    *self = merged;
                }
            }
        }
        let kept = self.save(path)?;
        *base = Some((self.clone(), engine::policy::SourceStamp::of(path)));
        Ok(kept)
    }
    pub fn push_recent(&mut self, p: PathBuf) {
        self.recent.retain(|r| r != &p);
        self.recent.insert(0, p);
        self.recent.truncate(10);
    }
}

/// Field by field (tables recursed): `mine` where it differs from `base`, else `disk`. `None` when a side does not
/// serialise or the result does not deserialise.
fn merge(base: &GuiSettings, mine: &GuiSettings, disk: &GuiSettings) -> Option<GuiSettings> {
    fn table(t: &toml::Table, base: &toml::Table, disk: &toml::Table) -> toml::Table {
        let mut out = disk.clone();
        let keys: std::collections::BTreeSet<&String> = t.keys().chain(base.keys()).collect();
        for k in keys {
            match (t.get(k), base.get(k), disk.get(k)) {
                (a, b, _) if a == b => {}
                // Recent folders merge per folder: the file's removals and additions, the app's additions and order.
                (Some(toml::Value::Array(a)), Some(toml::Value::Array(b)), d) if k == "recent" => {
                    let none = vec![];
                    let d = d.and_then(toml::Value::as_array).unwrap_or(&none);
                    let mut list: Vec<toml::Value> = a
                        .iter()
                        .filter(|x| d.contains(x) || !b.contains(x))
                        .cloned()
                        .collect();
                    for x in d {
                        if !b.contains(x) && !list.contains(x) {
                            list.push(x.clone());
                        }
                    }
                    out.insert(k.clone(), toml::Value::Array(list));
                }
                (
                    Some(toml::Value::Table(a)),
                    Some(toml::Value::Table(b)),
                    Some(toml::Value::Table(d)),
                ) => {
                    out.insert(k.clone(), toml::Value::Table(table(a, b, d)));
                }
                (Some(a), _, _) => {
                    out.insert(k.clone(), a.clone());
                }
                (None, _, _) => {
                    out.remove(k);
                }
            }
        }
        out
    }
    let v = |s: &GuiSettings| toml::Table::try_from(s).ok();
    table(&v(mine)?, &v(base)?, &v(disk)?).try_into().ok()
}
