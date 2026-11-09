//! Note HF4: set groups. Membership lives in `[[groups]]` of `sdc-project.toml` (`SetsDoc`) and is kept there whatever
//! the cards do; what a group shows reads the views, so a set with no card (Note GY5, or no set) is neither shown nor
//! counted in it, and a group never keeps a card. The view order pulls a group's members behind its first one
//! (`pull_groups`, `moved_order`). Every change here is one undo step and no re-plan: a group changes no output.
use super::input_name::match_key;
use super::sets::{set_views, SetView, SlotFill};
use super::AppModel;
use engine::sets::{fold_id, same_id};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

/// Note HF9: what Export set, Export group or Export N sets runs: set keys, and the heading Review and its button
/// show (`Export group Armor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportScope {
    pub keys: BTreeSet<String>,
    pub label: String,
}

/// A group of sets, as `[[groups]]` stores it: `sets` are set ids, `excluded` is left out of the file when false.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SetGroup {
    pub name: String,
    pub sets: Vec<String>,
    pub excluded: bool,
}

/// Where `move_to_group` puts the sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupTarget {
    Existing(String),
    New(String),
    None,
}

/// Where each of `ids` goes when a group's members are pulled behind its first member (the first in `ids`), as
/// indexes into `ids`. Sets in no group keep their place; a set named by two groups belongs to the first.
fn pull_order(ids: &[&str], groups: &[SetGroup]) -> Vec<usize> {
    let mut of: HashMap<String, usize> = HashMap::new();
    for (g, group) in groups.iter().enumerate() {
        for s in &group.sets {
            of.entry(fold_id(s)).or_insert(g);
        }
    }
    let mut blocks: Vec<Vec<usize>> = vec![vec![]; groups.len()];
    for (i, id) in ids.iter().enumerate() {
        if let Some(&g) = of.get(&fold_id(id)) {
            blocks[g].push(i);
        }
    }
    let mut placed = vec![false; groups.len()];
    let mut order: Vec<usize> = Vec::with_capacity(ids.len());
    for (i, id) in ids.iter().enumerate() {
        match of.get(&fold_id(id)) {
            None => order.push(i),
            Some(&g) if !placed[g] => {
                placed[g] = true;
                order.extend(&blocks[g]);
            }
            Some(_) => {}
        }
    }
    order
}

/// The view order with each group's members pulled behind its first member. A member with no view cannot decide
/// where the block sits.
pub(super) fn pull_groups(views: &mut Vec<SetView>, groups: &[SetGroup]) {
    if groups.is_empty() {
        return;
    }
    let order = pull_order(
        &views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
        groups,
    );
    let mut old: Vec<Option<SetView>> = std::mem::take(views).into_iter().map(Some).collect();
    *views = order.into_iter().filter_map(|i| old[i].take()).collect();
}

/// The on-screen order after set `id` moves to position `to` (counted in the list without it), or `None` when
/// nothing would visibly move. The first member of a group takes its block along (`block`, in view order); any other
/// set moves alone. A set put inside another group's block lands before the block when it came from below (`to` is
/// above its place) and after it when it came from above.
pub(super) fn moved_order(
    before: &[String],
    groups: &[SetGroup],
    id: &str,
    to: usize,
) -> Option<Vec<String>> {
    let from = before.iter().position(|i| same_id(i, id))?;
    let block: Vec<String> = groups
        .iter()
        .find(|g| g.sets.first().is_some_and(|f| same_id(f, id)))
        .map_or_else(|| vec![before[from].clone()], |g| g.sets.clone());
    let group_of = |s: &str| {
        groups
            .iter()
            .position(|g| g.sets.iter().any(|m| same_id(m, s)))
    };
    let mut ids = before.to_vec();
    ids.retain(|i| !block.iter().any(|b| same_id(b, i)));
    let mut at = to.min(ids.len());
    let (own, up) = (group_of(&block[0]), to < from);
    if let (Some(l), Some(r)) = (ids.get(at.wrapping_sub(1)), ids.get(at)) {
        let (gl, gr) = (group_of(l), group_of(r));
        if gl.is_some() && gl == gr && gl != own {
            let members: Vec<usize> = (0..ids.len())
                .filter(|&i| group_of(&ids[i]) == gl)
                .collect();
            at = if up {
                members[0]
            } else {
                members[members.len() - 1] + 1
            };
        }
    }
    ids.splice(at..at, block);
    let order = pull_order(&ids.iter().map(String::as_str).collect::<Vec<_>>(), groups);
    let after: Vec<String> = order.into_iter().map(|i| ids[i].clone()).collect();
    (after != before).then_some(after)
}

impl AppModel {
    /// The groups with a header: each one's members that have a card (are in the views), in view order; a group with
    /// none is left out (its `[[groups]]` entry stays).
    pub fn groups(&self) -> Vec<SetGroup> {
        let Some(d) = self.sets_doc.as_ref() else {
            return vec![];
        };
        let views = set_views(self);
        d.groups()
            .into_iter()
            .filter_map(|mut g| {
                g.sets = views
                    .iter()
                    .filter(|v| g.sets.iter().any(|s| same_id(s, &v.id)))
                    .map(|v| v.id.clone())
                    .collect();
                (!g.sets.is_empty()).then_some(g)
            })
            .collect()
    }

    /// The group (as `groups` shows it) set `set_id` is in.
    pub fn group_of(&self, set_id: &str) -> Option<SetGroup> {
        self.groups()
            .into_iter()
            .find(|g| g.sets.iter().any(|s| same_id(s, set_id)))
    }

    /// The ids of the selected sets, in the order they were selected (a set with no view has none).
    pub fn selected_set_ids(&self) -> Vec<String> {
        let views = set_views(self);
        self.selected_sets()
            .iter()
            .filter_map(|k| views.iter().find(|v| v.key == *k).map(|v| v.id.clone()))
            .collect()
    }

    /// Select the sets of group `name` that the Sets column shows (a filter hides some): "Select all sets" on its
    /// header. With none shown the selection is kept.
    pub fn select_group(&mut self, name: &str) {
        let Some(g) = self.groups().into_iter().find(|g| same_id(&g.name, name)) else {
            return;
        };
        let views = set_views(self);
        let keys: Vec<String> = self
            .set_order()
            .into_iter()
            .filter(|k| {
                views
                    .iter()
                    .any(|v| v.key == *k && g.sets.iter().any(|s| same_id(s, &v.id)))
            })
            .collect();
        let active = keys.first().cloned();
        if active.is_some() {
            self.set_selection(keys, active, true);
        }
    }

    /// The stored group `name` (ignoring case), or why there is none.
    fn stored_group(&self, name: &str) -> Result<SetGroup, String> {
        self.sets_doc
            .as_ref()
            .and_then(|d| d.groups().into_iter().find(|g| same_id(&g.name, name)))
            .ok_or_else(|| format!("no group {name}"))
    }

    /// The reason `sdc-project.toml` takes no group edit, or the edit's check result.
    fn group_edit(&self) -> Result<(), String> {
        self.sets_read_only_reason().map_or(Ok(()), Err)
    }

    /// Put sets `ids` in a group (`Existing`, or `New` with a name no group has), or in none; one undo step, no
    /// re-plan. A group whose last set leaves goes.
    pub fn move_to_group(&mut self, ids: &[String], group: GroupTarget) -> Result<(), String> {
        self.group_edit()?;
        let (label, group) = match group {
            GroupTarget::Existing(n) => {
                let n = self.stored_group(&n)?.name;
                (format!("Move to group {n}"), GroupTarget::Existing(n))
            }
            GroupTarget::New(n) => {
                let n = n.trim().to_string();
                self.free_group_name(&n)?;
                (format!("New group {n}"), GroupTarget::New(n))
            }
            GroupTarget::None => ("Move out of its group".into(), GroupTarget::None),
        };
        // A set is stored under the id its view shows.
        let ids: Vec<String> = ids
            .iter()
            .map(|i| self.view_of(i).map_or_else(|| i.clone(), |v| v.id))
            .collect();
        self.edit_docs(&label, None, false, move |_, d| {
            d.move_to_group(&ids, &group)
        });
        self.prune_collapsed();
        Ok(())
    }

    /// Whether `name` can name a group: not empty, and no group has it ignoring case.
    fn free_group_name(&self, name: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("a group needs a name".into());
        }
        match self.stored_group(name) {
            Ok(g) => Err(format!("a group {} already exists", g.name)),
            Err(_) => Ok(()),
        }
    }

    /// Rename group `old` to `new`: unique ignoring case (its own name in another case is its own).
    pub fn rename_group(&mut self, old: &str, new: &str) -> Result<(), String> {
        self.group_edit()?;
        let old = self.stored_group(old)?.name;
        let new = new.trim().to_string();
        if !same_id(&old, &new) {
            self.free_group_name(&new)?;
        } else if new.is_empty() {
            return Err("a group needs a name".into());
        }
        // The collapsed state goes with the name.
        let was_collapsed = self.group_collapsed(&old);
        self.edit_docs(&format!("Rename group {old}"), None, false, {
            let (old, new) = (old.clone(), new.clone());
            move |_, d| d.rename_group(&old, &new)
        });
        if was_collapsed && self.stored_group(&new).is_ok() {
            self.set_group_collapsed(&new, true);
        }
        self.prune_collapsed();
        Ok(())
    }

    /// Remove group `name`; its sets stay.
    pub fn ungroup(&mut self, name: &str) -> Result<(), String> {
        self.group_edit()?;
        let name = self.stored_group(name)?.name;
        self.edit_docs(&format!("Ungroup {name}"), None, false, move |_, d| {
            d.ungroup(&name)
        });
        self.prune_collapsed();
        Ok(())
    }

    /// Leave group `name` out of a whole-project export (`on`), or put it back.
    pub fn set_group_excluded(&mut self, name: &str, on: bool) -> Result<(), String> {
        self.group_edit()?;
        let name = self.stored_group(name)?.name;
        let label = if on {
            format!("Exclude group {name} from export")
        } else {
            format!("Include group {name} in export")
        };
        self.edit_docs(&label, None, false, move |_, d| {
            d.set_group_excluded(&name, on)
        });
        Ok(())
    }

    /// Forget the collapsed state of this project's groups that no longer exist (renamed, ungrouped, or gone with
    /// their last set), so a later group with the old name starts open.
    pub(super) fn prune_collapsed(&mut self) {
        let (Some(root), Some(d)) = (self.root.as_ref(), self.sets_doc.as_ref()) else {
            return;
        };
        let live: Vec<String> = d
            .groups()
            .iter()
            .map(|g| match_key(root, &g.name))
            .collect();
        let prefix = match_key(root, "");
        let before = self.settings.group_collapsed.len();
        self.settings
            .group_collapsed
            .retain(|k| !k.starts_with(&prefix) || live.contains(k));
        if self.settings.group_collapsed.len() != before {
            self.save_settings();
        }
    }

    /// Whether the user collapsed group `name` in this project (`gui.toml`, by project and name).
    pub fn group_collapsed(&self, name: &str) -> bool {
        self.root
            .as_ref()
            .is_some_and(|r| self.settings.group_collapsed.contains(&match_key(r, name)))
    }

    /// Keep group `name` collapsed (`on`) or open in `gui.toml`; no undo step, and the project file is not touched.
    pub fn set_group_collapsed(&mut self, name: &str, on: bool) {
        let Some(root) = self.root.as_ref() else {
            return;
        };
        let key = match_key(root, name);
        let changed = if on {
            self.settings.group_collapsed.insert(key)
        } else {
            self.settings.group_collapsed.remove(&key)
        };
        if changed {
            self.save_settings();
        }
    }

    /// The keys of the sets in an excluded group (as stored, cards or not), per view key.
    pub(super) fn excluded_sets(&self) -> Arc<HashSet<String>> {
        super::cached(
            self,
            |c| c.excluded_sets.clone(),
            |c, v| c.excluded_sets = Some(v),
            || {
                let root = self.sets_root();
                Arc::new(
                    self.sets_doc
                        .iter()
                        .flat_map(|d| d.groups())
                        .filter(|g| g.excluded)
                        .flat_map(|g| g.sets)
                        .map(|id| engine::sets::set_key(&root, &id))
                        .collect(),
                )
            },
        )
    }

    /// `in_run` for set id `id`.
    pub(super) fn in_run_id(&self, id: &str) -> bool {
        self.in_run(&engine::sets::set_key(&self.sets_root(), id))
    }

    /// The Export set / group / N sets that Review or the running export is for.
    pub fn export_scope(&self) -> Option<&ExportScope> {
        self.export_scope.as_ref()
    }

    pub(super) fn set_scope(&mut self, scope: Option<ExportScope>) {
        if self.export_scope != scope {
            self.export_scope = scope;
            self.scope_rev += 1;
        }
    }

    /// Note HF9: this run leaves sets out: a scope, an excluded group, or a texture in no set.
    fn leaves_sets_out(&self) -> bool {
        self.export_scope.is_some()
            || !self.excluded_sets().is_empty()
            || self
                .sets_doc
                .as_ref()
                .is_some_and(|d| !d.no_set_keys().is_empty())
    }

    /// The request's `write_only`: the scope's keys; on a whole-project run every shown set's key that is in the run,
    /// or `None` when no set is left out, so the ordinary path stays the ordinary path.
    pub fn write_only(&self) -> Option<BTreeSet<String>> {
        if let Some(s) = &self.export_scope {
            return Some(s.keys.clone());
        }
        self.leaves_sets_out().then(|| {
            set_views(self)
                .iter()
                .map(|v| v.key.clone())
                .filter(|k| self.in_run(k))
                .collect()
        })
    }

    /// The plan's output root when this run leaves sets out (a PGPatcher entry's untouched dep is checked on disk
    /// under it), else `None`.
    pub(super) fn untouched_root(&self) -> Option<std::path::PathBuf> {
        self.leaves_sets_out().then(|| self.sets_root())
    }

    /// Note HF9: per-set files no set writes any more are reported only on a whole-project run without an excluded
    /// group; otherwise a set left out of the run would read as gone.
    pub(super) fn reports_orphans(&self) -> bool {
        self.export_scope.is_none() && self.excluded_sets().is_empty()
    }

    /// Note HF9: export only sets `keys` (and the sets they borrow from), with Review and its button saying `label`.
    /// It runs as Export does: at once when clean, else through Review; the scope ends when Review closes or the
    /// export ends.
    pub fn export_sets(&mut self, keys: BTreeSet<String>, label: String) {
        let keys = self.with_hosts(keys);
        self.set_scope(Some(ExportScope { keys, label }));
        self.export_go();
        if self.review.is_none() && self.export.running.is_none() && !self.export_queued() {
            self.set_scope(None);
        }
    }

    /// Export group `name`'s shown sets, excluded or not (asking by name is explicit); the flag stays.
    pub fn export_group(&mut self, name: &str) {
        let Some(g) = self.groups().into_iter().find(|g| same_id(&g.name, name)) else {
            return;
        };
        let keys = set_views(self)
            .iter()
            .filter(|v| g.sets.iter().any(|s| same_id(s, &v.id)))
            .map(|v| v.key.clone())
            .collect();
        self.export_sets(keys, format!("Export group {}", g.name));
    }

    /// `keys` plus every set a slot of theirs borrows from, transitively. A host in an excluded group is left alone
    /// unless it is in `keys`: its file on disk serves the borrower.
    fn with_hosts(&self, mut keys: BTreeSet<String>) -> BTreeSet<String> {
        let views = set_views(self);
        let excluded = self.excluded_sets();
        let mut todo: Vec<String> = keys.iter().cloned().collect();
        while let Some(k) = todo.pop() {
            let Some(v) = views.iter().find(|v| v.key == k) else {
                continue;
            };
            for f in v.slots.values() {
                let SlotFill::Borrowed { from, .. } = f else {
                    continue;
                };
                let Some(h) = views.iter().find(|h| same_id(&h.id, from)) else {
                    continue;
                };
                if !excluded.contains(&h.key) && keys.insert(h.key.clone()) {
                    todo.push(h.key.clone());
                }
            }
        }
        keys
    }
}
