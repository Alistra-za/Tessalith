//! GUI set edits and output paths (M3b spec §2.1, DECISIONS P1–P2). Applied after the automatic grouping: edits
//! first (set keys are fixed from then on), output paths second, then outputs are compared for duplicates again.
//! The CLI never sets either.
use crate::discover::InputFile;
use crate::group::{fold, member_split, TextureSet};
use crate::overrides::KeyIndex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// One change to the automatic grouping. Sets are named by id (the set key without the output root in front),
/// files by their `[files]` key (project-relative, compared case-insensitively).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SetEdit {
    /// A new, empty set.
    Create {
        set: String,
    },
    /// The set keeps its members under a new id.
    Rename {
        from: String,
        to: String,
    },
    /// The file leaves its set and joins `to`.
    Move {
        file: String,
        to: String,
    },
    /// The file leaves its set and becomes a set of its own.
    Remove {
        file: String,
    },
    /// The file leaves its set and becomes a set of its own that writes nothing (Note HF1). It groups as `Remove`
    /// does; `prepare` also leaves it out of the `IN-DUPLICATE` comparison.
    NoSet {
        file: String,
    },
    // Note HH1: set `to` also reads the file as one of its helpers; the file stays a member of its own set.
    Share {
        file: String,
        to: String,
    },
}

/// An edit that could not apply: it names a file or set that is not there (it is reported, never fatal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleEdit {
    pub edit: SetEdit,
    pub reason: String,
}

/// The id of a set: its key without the output root in front (`landscape/rock01`). A key not under the root is
/// returned whole.
pub fn set_id(output_root: &Path, key: &str) -> String {
    let root = fold(output_root);
    let root = root.trim_end_matches('/');
    key.strip_prefix(root)
        .and_then(|r| r.strip_prefix('/'))
        .map_or_else(|| key.to_string(), str::to_string)
}

/// A set id as set keys hold it: '/'-separated, Unicode lower case, no leading or trailing '/'.
pub fn fold_id(id: &str) -> String {
    id.replace('\\', "/")
        .to_lowercase()
        .trim_matches('/')
        .to_string()
}

/// Whether two set ids (or `[files]` keys) name the same thing, as the engine compares them.
pub fn same_id(a: &str, b: &str) -> bool {
    fold_id(a) == fold_id(b)
}

/// The key of the set with id `id`: lower-case, '/'-separated, under the output root (as `group_sets_with` builds
/// keys).
pub fn set_key(output_root: &Path, id: &str) -> String {
    let root = fold(output_root);
    format!("{}/{}", root.trim_end_matches('/'), fold_id(id))
}

fn stem_lower(f: &InputFile) -> String {
    f.rel
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
}

/// Apply `edits` in order. Returns the edits that named something missing; they change nothing. Sets left with no
/// members are dropped (the GUI shows an empty created set from its own file). The result is sorted by key, as
/// `group_sets_with` returns it.
pub fn apply_set_edits(
    sets: &mut Vec<TextureSet>,
    files: &[InputFile],
    root: Option<&Path>,
    output_root: &Path,
    aliases: &BTreeMap<String, String>,
    edits: &[SetEdit],
) -> Vec<StaleEdit> {
    if edits.is_empty() {
        return vec![];
    }
    let index = root.map(|root| KeyIndex::new(root, files));
    let file_of = |k: &str| -> Option<usize> { index.as_ref()?.hits(k).first().copied() };
    let pos = |sets: &Vec<TextureSet>, id: &str| {
        let key = set_key(output_root, id);
        sets.iter().position(|s| s.key == key)
    };
    let mut touched: BTreeSet<String> = BTreeSet::new();
    let mut stale = vec![];
    // Note HH3: each share that applied and its set, so a set left with no member of its own reports it.
    let mut shared: Vec<(usize, &SetEdit)> = vec![];
    for e in edits {
        let r: Result<(), String> = match e {
            SetEdit::Create { set } => {
                // Idempotent: a created set whose name the automatic grouping also produced (e.g. after Duplicate
                // source wrote `<set>_rough.png`) is that set.
                if pos(sets, set).is_some() {
                    Ok(())
                } else {
                    sets.push(TextureSet {
                        key: set_key(output_root, set),
                        members: vec![],
                        suffixes: BTreeSet::new(),
                        shared_skin: false,
                        shares: vec![],
                    });
                    Ok(())
                }
            }
            SetEdit::Rename { from, to } => match (pos(sets, from), pos(sets, to)) {
                (None, _) => Err(format!("no set '{from}'")),
                (Some(_), Some(_)) => Err(format!("a set '{to}' already exists")),
                (Some(i), None) => {
                    sets[i].key = set_key(output_root, to);
                    touched.insert(sets[i].key.clone());
                    Ok(())
                }
            },
            SetEdit::Move { file, to } => match (file_of(file), pos(sets, to)) {
                (None, _) => Err(format!("no file '{file}'")),
                (_, None) => Err(format!("no set '{to}'")),
                (Some(f), Some(t)) => {
                    for s in sets.iter_mut() {
                        if s.members.contains(&f) {
                            touched.insert(s.key.clone());
                            s.members.retain(|&m| m != f);
                        }
                    }
                    sets[t].members.push(f);
                    touched.insert(sets[t].key.clone());
                    Ok(())
                }
            },
            SetEdit::Remove { file } | SetEdit::NoSet { file } => match file_of(file) {
                None => Err(format!("no file '{file}'")),
                Some(f) => {
                    for s in sets.iter_mut() {
                        if s.members.contains(&f) {
                            touched.insert(s.key.clone());
                            s.members.retain(|&m| m != f);
                        }
                    }
                    let dir = files[f].output.parent().map(fold).unwrap_or_default();
                    let base = format!("{dir}/{}", stem_lower(&files[f]));
                    let mut key = base.clone();
                    let mut n = 2;
                    while sets.iter().any(|s| s.key == key) {
                        key = format!("{base}~{n}");
                        n += 1;
                    }
                    touched.insert(key.clone());
                    sets.push(TextureSet {
                        key,
                        members: vec![f],
                        suffixes: BTreeSet::new(),
                        shared_skin: false,
                        shares: vec![],
                    });
                    Ok(())
                }
            },
            SetEdit::Share { file, to } => match (file_of(file), pos(sets, to)) {
                (None, _) => Err(format!("no file '{file}'")),
                (_, None) => Err(format!("no set '{to}'")),
                (Some(f), Some(t)) => {
                    let s = &mut sets[t];
                    if !s.members.contains(&f) && !s.shares.contains(&f) {
                        s.shares.push(f);
                        touched.insert(s.key.clone());
                        shared.push((t, e));
                    }
                    Ok(())
                }
            },
        };
        if let Err(reason) = r {
            stale.push(StaleEdit {
                edit: e.clone(),
                reason,
            });
        }
    }
    // A touched set's suffixes come from its members, as `group_sets_with` derives them (a skin `_wet` or `_detail_n` member that
    // `join_skin_wet` / `join_skin_detail` joined keeps its `wet` / `detail_n`).
    // Note HH1: a shared file adds its role, as `predict_suffixes` reads it; one that later joined the set is a member.
    for s in sets.iter_mut().filter(|s| touched.contains(&s.key)) {
        s.members.sort_unstable();
        let members = &s.members;
        s.shares.retain(|i| !members.contains(i));
        let suffixes = s
            .members
            .iter()
            .filter_map(|&i| member_split(&stem_lower(&files[i]), s, aliases).suffix)
            .chain(
                s.shares
                    .iter()
                    .filter_map(|&i| crate::pack::plan::raw_role(&files[i]).map(|r| r.id())),
            )
            .collect();
        s.suffixes = suffixes;
    }
    for (t, e) in shared {
        if let (true, SetEdit::Share { to, .. }) = (sets[t].members.is_empty(), e) {
            stale.push(StaleEdit {
                edit: e.clone(),
                reason: format!("set '{to}' has no texture of its own"),
            });
        }
    }
    sets.retain(|s| !s.members.is_empty());
    sets.sort_by(|a, b| a.key.cmp(&b.key));
    stale
}

/// The paths of the files `edits` mark `NoSet` (a key naming no file is the stale edit `apply_set_edits` reports).
pub(crate) fn no_set_paths(
    files: &[InputFile],
    root: Option<&Path>,
    edits: &[SetEdit],
) -> BTreeSet<PathBuf> {
    let Some(root) = root else {
        return BTreeSet::new();
    };
    let index = KeyIndex::new(root, files);
    edits
        .iter()
        .filter_map(|e| match e {
            SetEdit::NoSet { file } => {
                Some(index.hits(file).iter().map(|&i| files[i].path.clone()))
            }
            _ => None,
        })
        .flatten()
        .collect()
}

/// Note HF1: whether `[project] no_set` key `key` names the file at project-relative path `rel`: the `[files]` rule
/// (the path, or the path without its last extension, case-insensitively). Every file a key names is in no set.
pub fn no_set_names(key: &str, rel: &str) -> bool {
    crate::overrides::KeyForms::of(rel).matches(key)
}

/// What an explicit output path applies to (DECISIONS P1).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKey {
    /// A source file, by its `[files]` key.
    File(String),
    /// A packed output of a set: the set id and the recipe suffix (`rmaos`, `rfaos`, `p`, `m`).
    Packed { set: String, suffix: String },
}

/// Whether `rel` stays under the folder it is joined to: relative, with no `..`, root or drive part.
fn stays_under(rel: &Path) -> bool {
    use std::path::Component;
    !rel.is_absolute()
        && rel
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// Give each file named by an `OutputKey::File` the output `output_root/<path>`. Keys that match no file are
/// ignored (the GUI computes them from the previous plan; a vanished file is reported by its set edit, if any), and
/// so are paths that would leave the output root (absolute, or with `..`).
pub fn apply_output_paths(
    files: &mut [InputFile],
    root: Option<&Path>,
    output_root: &Path,
    paths: &BTreeMap<OutputKey, PathBuf>,
) {
    let Some(root) = root.filter(|_| !paths.is_empty()) else {
        return;
    };
    let index = KeyIndex::new(root, files);
    for (k, rel) in paths.iter().filter(|(_, rel)| stays_under(rel)) {
        let OutputKey::File(key) = k else { continue };
        for &i in index.hits(key) {
            files[i].output = output_root.join(rel);
        }
    }
}

/// The packed-output paths by (set key, recipe suffix), absolute, for `plan_packing`: set keys under `output_root`,
/// paths under `paths_root` (Note HQ1: the export folder). A path that would leave it is ignored, as in
/// `apply_output_paths`.
pub fn packed_outputs(
    output_root: &Path,
    paths_root: &Path,
    paths: &BTreeMap<OutputKey, PathBuf>,
) -> BTreeMap<(String, String), PathBuf> {
    paths
        .iter()
        .filter(|(_, rel)| stays_under(rel))
        .filter_map(|(k, rel)| match k {
            OutputKey::Packed { set, suffix } => Some((
                (set_key(output_root, set), suffix.to_lowercase()),
                paths_root.join(rel),
            )),
            OutputKey::File(_) => None,
        })
        .collect()
}
