//! Note HF9/HF10: the export scope (Export set, Export group, Export N sets) and what a run leaves out: the request's
//! `write_only`, the untouched-root check, orphan reports, and the scope's life.
use super::sets::{set_views, SlotFill};
use super::AppModel;
use engine::sets::same_id;
use std::collections::BTreeSet;

/// Note HF9: what Export set, Export group or Export N sets runs: set keys, and the heading Review and its button
/// show (`Export group Armor`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportScope {
    pub keys: BTreeSet<String>,
    pub label: String,
}

impl AppModel {
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
        // Note HF10: the menus are off while an export runs; changing the scope then would recount its lists.
        if self.export.running.is_some() {
            return;
        }
        let keys = self.with_hosts(keys);
        self.set_scope(Some(ExportScope { keys, label }));
        self.export_go();
        self.end_idle_scope();
    }

    /// A scope lives while its Review is open, its export runs or waits for a plan, or a dialog is open (one may park
    /// its export); any path that drops all four (a refusal, a queued export dropped or cancelled, the dialog
    /// answered) ends it here. `poll` calls this every frame.
    pub(super) fn end_idle_scope(&mut self) {
        if self.review.is_none()
            && self.export.running.is_none()
            && !self.export_queued()
            && self.dialog.is_none()
        {
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
