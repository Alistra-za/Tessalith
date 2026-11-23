//! Undo and redo over every edit (M3b spec §7.2, DECISIONS P9): a command keeps the text of `sdc.toml` and
//! `sdc-project.toml` before and after. Opening another folder clears it; Export and Save are not steps.
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub sdc_toml: String,
    pub project: String,
}

/// A file a command created (Duplicate source, spec §3.4): its path, its source, and its SHA-256 when created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedFile {
    pub path: PathBuf,
    pub source: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub label: String,
    pub before: Snapshot,
    pub after: Snapshot,
    /// Consecutive commands with the same `Some` key are one step (a slider drag, typing in one field).
    pub merge: Option<String>,
    pub created_file: Option<CreatedFile>,
    /// Undo and Redo of this command claim a type or alpha write, even across a Save.
    pub type_alpha: bool,
    /// Automatic listing removals unique to the before or after snapshot, respectively.
    pub normal_prunes: [BTreeMap<String, String>; 2],
}

impl Command {
    fn record_prune(&mut self, side: usize, key: String, word: String) {
        let opposite = &mut self.normal_prunes[1 - side];
        if opposite.get(&key) == Some(&word) {
            opposite.remove(&key);
        } else {
            self.normal_prunes[side].insert(key, word);
        }
    }
}

#[derive(Debug, Default)]
pub struct History {
    pub undo: Vec<Command>,
    pub redo: Vec<Command>,
    /// Note FC3: every `push` counts here, merges included: a row that offers `Undo` remembers the count its step
    /// left, and offers it only while nothing was pushed since.
    pub pushes: u64,
    /// Note FC1 R5: the untouched recorded sub-types when the presets were saved; the settled plan re-types them.
    /// Part of the history so every reset (open, reload) drops it.
    pub retype_untouched: Vec<(String, serde_json::Value)>,
    /// A deliberate Undo of a shader follow survives the replan it starts.
    pub(super) normal_followed: std::collections::HashSet<String>,
}

impl History {
    pub const LIMIT: usize = 200;

    pub fn push(&mut self, c: Command) {
        self.redo.clear();
        self.pushes += 1;
        if let (Some(last), Some(k)) = (self.undo.last_mut(), c.merge.as_ref()) {
            if last.merge.as_ref() == Some(k)
                && last.created_file.is_none()
                && c.created_file.is_none()
            {
                last.after = c.after;
                last.type_alpha |= c.type_alpha;
                for (side, prunes) in c.normal_prunes.into_iter().enumerate() {
                    for (key, word) in prunes {
                        last.record_prune(side, key, word);
                    }
                }
                return;
            }
        }
        self.undo.push(c);
        if self.undo.len() > Self::LIMIT {
            self.undo.remove(0);
        }
    }

    pub fn apply_auto(&mut self, writes: &[crate::project::AutoWrite]) {
        self.apply_auto_in(writes, |state, _, apply| {
            apply(&mut state.sdc_toml);
            BTreeMap::new()
        });
    }

    pub(super) fn apply_auto_in(
        &mut self,
        writes: &[crate::project::AutoWrite],
        mut adapt: impl FnMut(
            &mut Snapshot,
            Option<&crate::project::NewFileEntries>,
            &mut dyn FnMut(&mut String),
        ) -> BTreeMap<String, String>,
    ) {
        if self.undo.is_empty() && self.redo.is_empty() {
            return;
        }
        let entries = crate::project::NewFileEntries::new(writes);
        let mut patched = std::collections::HashMap::<String, String>::new();
        let mut apply = |text: &mut String| {
            // `take` empties `text`, so `clone_from` below reuses no allocation here.
            let patched_text = patched
                .entry(std::mem::take(text))
                .or_insert_with_key(|original| {
                    entries
                        .as_ref()
                        .and_then(|new| new.appended(original))
                        .unwrap_or_else(|| crate::project::apply_auto(original, writes))
                });
            text.clone_from(patched_text);
        };
        for command in self.undo.iter_mut().chain(&mut self.redo) {
            let removed = [&mut command.before, &mut command.after]
                .map(|state| adapt(state, entries.as_ref(), &mut apply));
            for side in 0..2 {
                for (key, word) in &removed[side] {
                    if removed[1 - side].get(key) != Some(word) {
                        command.record_prune(side, key.clone(), word.clone());
                    }
                }
            }
        }
    }

    /// The command to undo (moved to the redo list).
    pub fn undo_step(&mut self) -> Option<Command> {
        let c = self.undo.pop()?;
        // audit 10: a merge only continues an uninterrupted run.
        if let Some(top) = self.undo.last_mut() {
            top.merge = None;
        }
        self.redo.push(c.clone());
        Some(c)
    }

    /// The command to redo (moved back to the undo list).
    pub fn redo_step(&mut self) -> Option<Command> {
        let c = self.redo.pop()?;
        self.undo.push(Command {
            merge: None,
            ..c.clone()
        });
        Some(c)
    }

    /// Put back a step whose undo or redo was refused.
    pub fn refuse_undo(&mut self) {
        if let Some(c) = self.redo.pop() {
            self.undo.push(c);
        }
    }
    pub fn refuse_redo(&mut self) {
        if let Some(c) = self.undo.pop() {
            self.redo.push(c);
        }
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.retype_untouched.clear();
        self.normal_followed.clear();
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

pub fn sha256_of(p: &std::path::Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    Some(hex::encode(Sha256::digest(std::fs::read(p).ok()?)))
}

/// The key of a hash record for an exported file: its absolute path, `/`-separated and lower-cased.
pub fn record_key(p: &std::path::Path) -> String {
    std::path::absolute(p)
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase()
}

/// Audit 08c-1: a hash record map with every key a `record_key`. A key from before (a path relative to the export
/// folder) is read as a file under `root`.
pub fn records_at(
    shas: &std::collections::BTreeMap<String, String>,
    root: &std::path::Path,
) -> std::collections::BTreeMap<String, String> {
    shas.iter()
        .map(|(k, v)| {
            let p = std::path::Path::new(k);
            let k = if p.is_absolute() {
                k.clone()
            } else {
                record_key(&root.join(p))
            };
            (k, v.clone())
        })
        .collect()
}

/// The file on disk whose name equals `path`'s apart from case, when the case differs (Note O6: a case-only
/// rename). Records are lower-case (audit 08c-1), so the old spelling is read from the folder.
pub fn case_twin(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    std::fs::read_dir(path.parent()?)
        .ok()?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .find(|n| *n != name && n.to_lowercase() == name.to_lowercase())
        .map(|n| path.with_file_name(n))
}

/// O6: a case-only rename drops sdc's own unchanged file under the old name: `path`'s case twin is removed only when
/// a record `last` exists and the twin still hashes to it. No record, or a twin that cannot be read, is not sdc's
/// file and stays (wave 2 #48).
pub fn remove_own_case_twin(path: &std::path::Path, last: Option<&str>) {
    if let Some(old) = own_case_twin(path, last) {
        let _ = std::fs::remove_file(old);
    }
}

/// The case twin `remove_own_case_twin` removes: `path`'s twin when a record `last` exists and the twin hashes to it.
pub fn own_case_twin(path: &std::path::Path, last: Option<&str>) -> Option<std::path::PathBuf> {
    let (old, last) = (case_twin(path)?, last?);
    (sha256_of(&old).as_deref() == Some(last)).then_some(old)
}

/// The recorded files under `root` this run did not write (`this_run`, relative to `root`) that are still on disk,
/// spelled as on disk. They are reported, never deleted, and their records stay.
pub fn orphans<'a>(
    shas: &std::collections::BTreeMap<String, String>,
    root: &std::path::Path,
    this_run: impl Iterator<Item = &'a String>,
) -> Vec<std::path::PathBuf> {
    let prefix = format!("{}/", record_key(root));
    let mine: std::collections::BTreeSet<String> =
        this_run.map(|r| record_key(&root.join(r))).collect();
    shas.keys()
        .filter(|k| !mine.contains(*k))
        .filter_map(|k| k.strip_prefix(&prefix))
        .map(|rest| root.join(rest))
        .filter(|p| p.is_file())
        .map(|p| case_twin(&p).unwrap_or(p))
        .collect()
}

/// `ours` for `engine::dds::may_replace`: the bytes on disk are the ones whose hash is recorded (`last`).
pub fn recorded(last: Option<&str>) -> impl FnOnce(&[u8]) -> bool + '_ {
    use sha2::{Digest, Sha256};
    move |b| last == Some(hex::encode(Sha256::digest(b)).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::AutoWrite;
    use serde_json::json;

    #[test]
    fn automatic_values_patch_both_sides_of_undo_and_redo_without_overriding_a_choice() {
        let absent = Snapshot {
            sdc_toml: String::new(),
            project: "untouched".into(),
        };
        let recorded = Snapshot {
            sdc_toml: "[files.\"A.PNG\"]\nsubtype = \"normal\"\n".into(),
            ..absent.clone()
        };
        let chosen = Snapshot {
            sdc_toml: "[files.\"A.PNG\"]\nsubtype = \"msn\"\n".into(),
            ..absent.clone()
        };
        let mut history = History::default();
        for (before, after) in [
            (absent.clone(), recorded.clone()),
            (recorded, chosen.clone()),
        ] {
            history.push(Command {
                label: "edit".into(),
                before,
                after,
                merge: None,
                created_file: None,
                type_alpha: true,
                normal_prunes: Default::default(),
            });
        }
        history.undo_step();
        let mut write = AutoWrite {
            key: "a.png".into(),
            field: "subtype",
            value: json!("normal"),
            only_if: None,
        };
        history.apply_auto(std::slice::from_ref(&write));
        write.only_if = Some(json!("normal"));
        write.value = json!("diffuse");
        history.apply_auto(&[write]);
        for snapshot in [
            &history.undo[0].before,
            &history.undo[0].after,
            &history.redo[0].before,
        ] {
            assert!(snapshot.sdc_toml.contains("subtype = \"diffuse\""));
            assert_eq!(snapshot.project, absent.project);
        }
        assert_eq!(history.redo[0].after, chosen);
    }

    #[test]
    fn history_insertion_matches_the_toml_editor_byte_for_byte() {
        let writes = [AutoWrite {
            key: "new.png".into(),
            field: "subtype",
            value: json!("normal"),
            only_if: None,
        }];
        let entries = crate::project::NewFileEntries::new(&writes).unwrap();
        for (fast, text) in [
            (true, ""),
            (true, "target = 'cs'"),
            (true, "target = 'cs'\n"),
            (true, "[project]\nname = 'example'\n"),
            (true, "[files.\"other.png\"]\nsubtype = \"diffuse\""),
            (true, "[files.\"other.png\"]\nsubtype = \"diffuse\"\n"),
            (true, "[files.\"a.png\"]\nformat = 'bc7'\n[files.\"b.png\"]\nformat = 'bc4'\n"),
            (false, "[files.\"other.png\"]\nsubtype = \"diffuse\"\n\n"),
            (false, "# Notes only\n"),
            (false, "export = { names.pbr = 'P', also = ['vanilla'], names.cm = 'C' }\n"),
            (false, "targets = [{name = 'plain'}]\n"),
            (false, "[files.\"other.png\"]\nalpha.policy = 'cutout'\nformat = 'bc7'\nalpha.threshold = 128\n"),
            (false, "target = 'cs'\n# trailing comment\n"),
            (false, "files = { 'other.png' = { subtype = 'normal' } }\n"),
            (false, "[files.'NEW']\nformat = 'bc7'\n"),
            (false, "[files.\"n\\u0065w.png\"]\nformat = 'bc7'\n"),
            (false, "[files.\"a.png\"]\nformat = 'bc7'\n[other]\ntest = true\n[files.\"a.png\".alpha]\nthreshold = 128\n"),
        ] {
            let expected = crate::project::apply_auto(text, &writes);
            let snapshot = Snapshot { sdc_toml: text.to_owned(), project: "same".into() };
            let mut history = History::default();
            history.push(Command { label: "edit".into(), before: snapshot.clone(), after: snapshot, merge: None, created_file: None, type_alpha: false, normal_prunes: Default::default() });
            history.apply_auto(&writes);
            assert_eq!(history.undo[0].before.sdc_toml, expected, "before {text:?}");
            assert_eq!(history.undo[0].after.sdc_toml, expected, "after {text:?}");
            assert_eq!(entries.appended(text).as_ref(), fast.then_some(&expected), "{text:?}");
        }
    }

    #[test]
    fn automatic_fields_reach_alternate_key_spellings_in_history() {
        for (input_key, text, key) in [
            ("kéy", "[files.'Kéy']\nformat = 'bc7'\n", "Kéy"),
            ("kéy", "files.\"kéy\".format = 'bc7'\n", "kéy"),
            // These test-only characters do not belong in the UI font inventory.
            ("kéy", "[files.\"K\u{c9}Y\"]\nformat = 'bc7'\n", "K\u{c9}Y"),
            // Final sigma depends on whether the extension is still present during lowercasing.
            (
                "\u{39f}\u{3a3}.png",
                "[files.\"\u{3bf}\u{3c3}.png\"]\nformat = 'bc7'\n",
                "\u{3bf}\u{3c3}.png",
            ),
        ] {
            let write = AutoWrite {
                key: input_key.into(),
                field: "subtype",
                value: json!("normal"),
                only_if: None,
            };
            assert!(
                crate::project::NewFileEntries::new(std::slice::from_ref(&write))
                    .unwrap()
                    .appended(text)
                    .is_none(),
                "editor owns {text}"
            );
            let snapshot = Snapshot {
                sdc_toml: text.into(),
                project: String::new(),
            };
            let mut history = History::default();
            history.push(Command {
                label: "format".into(),
                before: snapshot.clone(),
                after: snapshot,
                merge: None,
                created_file: None,
                type_alpha: false,
                normal_prunes: Default::default(),
            });
            history.apply_auto(std::slice::from_ref(&write));
            for side in [&history.undo[0].before, &history.undo[0].after] {
                let doc: toml_edit::DocumentMut = side.sdc_toml.parse().unwrap();
                assert_eq!(
                    doc["files"][key]
                        .get("subtype")
                        .and_then(toml_edit::Item::as_str),
                    Some("normal"),
                    "{text}"
                );
            }
        }
    }

    #[test]
    #[ignore = "release timing"]
    fn apply_auto_timing() {
        use std::fmt::Write;
        let mut text = String::new();
        for i in 0..5_000 {
            writeln!(text, "[files.\"{i}.png\"]\nsubtype = \"normal\"").unwrap();
        }
        let mut history = History::default();
        for i in 0..History::LIMIT {
            let before = Snapshot {
                sdc_toml: text.clone(),
                project: String::new(),
            };
            text = text.replacen("subtype = \"normal\"", "subtype = \"diffuse\"", 1);
            history.push(Command {
                label: format!("Edit {i}"),
                before,
                after: Snapshot {
                    sdc_toml: text.clone(),
                    project: String::new(),
                },
                merge: None,
                created_file: None,
                type_alpha: true,
                normal_prunes: Default::default(),
            });
        }
        let writes = [AutoWrite {
            key: "new.png".into(),
            field: "subtype",
            value: json!("normal"),
            only_if: None,
        }];
        let start = std::time::Instant::now();
        history.apply_auto(&writes);
        let elapsed = start.elapsed();
        eprintln!("apply_auto took {elapsed:?}");
        assert!(history.undo.iter().all(|c| [&c.before, &c.after]
            .iter()
            .all(|s| s.sdc_toml.contains("[files.\"new.png\"]"))));
        assert!(
            elapsed < std::time::Duration::from_millis(250),
            "{elapsed:?}"
        );
    }
}
