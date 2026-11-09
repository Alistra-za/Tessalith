//! Note HF4: set groups. Membership lives in `[[groups]]` of `sdc-project.toml` (`SetsDoc`) and is kept there whatever
//! the cards do; what a group shows reads the views, so a set with no card (Note GY5, or no set) is neither shown nor
//! counted in it, and a group never keeps a card. The view order pulls a group's members behind its first one
//! (`pull_groups`). Every change here is one undo step and no re-plan: a group changes no output.
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

/// The view order with each group's members pulled behind its first member (the first in `views`). Sets in no group
/// keep their place; a member with no view cannot decide where the block sits. A set named by two groups belongs to
/// the first.
pub(super) fn pull_groups(views: &mut Vec<SetView>, groups: &[SetGroup]) {
    if groups.is_empty() {
        return;
    }
    let mut of: HashMap<String, usize> = HashMap::new();
    for (g, group) in groups.iter().enumerate() {
        for s in &group.sets {
            of.entry(fold_id(s)).or_insert(g);
        }
    }
    let mut blocks: Vec<Vec<usize>> = vec![vec![]; groups.len()];
    for (i, v) in views.iter().enumerate() {
        if let Some(&g) = of.get(&fold_id(&v.id)) {
            blocks[g].push(i);
        }
    }
    let mut placed = vec![false; groups.len()];
    let mut order: Vec<usize> = Vec::with_capacity(views.len());
    for (i, v) in views.iter().enumerate() {
        match of.get(&fold_id(&v.id)) {
            None => order.push(i),
            Some(&g) if !placed[g] => {
                placed[g] = true;
                order.extend(&blocks[g]);
            }
            Some(_) => {}
        }
    }
    let mut old: Vec<Option<SetView>> = std::mem::take(views).into_iter().map(Some).collect();
    *views = order.into_iter().filter_map(|i| old[i].take()).collect();
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
        let label = match &group {
            GroupTarget::Existing(n) => format!("Move to group {}", self.stored_group(n)?.name),
            GroupTarget::New(n) => {
                let n = n.trim();
                self.free_group_name(n)?;
                format!("New group {n}")
            }
            GroupTarget::None => "Move out of its group".into(),
        };
        let group = match group {
            GroupTarget::Existing(n) => GroupTarget::Existing(self.stored_group(&n)?.name),
            GroupTarget::New(n) => GroupTarget::New(n.trim().to_string()),
            GroupTarget::None => GroupTarget::None,
        };
        // A set is stored under the id its view shows.
        let ids: Vec<String> = ids
            .iter()
            .map(|i| self.view_of(i).map_or_else(|| i.clone(), |v| v.id))
            .collect();
        self.edit_docs(&label, None, false, move |_, d| {
            d.move_to_group(&ids, &group)
        });
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
        self.edit_docs(&format!("Rename group {old}"), None, false, move |_, d| {
            d.rename_group(&old, &new)
        });
        Ok(())
    }

    /// Remove group `name`; its sets stay.
    pub fn ungroup(&mut self, name: &str) -> Result<(), String> {
        self.group_edit()?;
        let name = self.stored_group(name)?.name;
        self.edit_docs(&format!("Ungroup {name}"), None, false, move |_, d| {
            d.ungroup(&name)
        });
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
