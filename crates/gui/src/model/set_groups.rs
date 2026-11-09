//! Note HF4: set groups. Membership lives in `[[groups]]` of `sdc-project.toml` (`SetsDoc`) and is kept there whatever
//! the cards do; what a group shows reads the views, so a set with no card (Note GY5, or no set) is neither shown nor
//! counted in it, and a group never keeps a card. The view order pulls a group's members behind its first one
//! (`pull_groups`, `moved_order`). Every change here is one undo step and no re-plan: a group changes no output.
use super::input_name::match_key;
use super::sets::{set_views, SetView};
use super::AppModel;
use engine::sets::{fold_id, same_id};
use std::collections::HashMap;

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
}
