//! E8: a type choice that makes a set card says so. The choice is queued when it is written, with the card IDs listed
//! before the first of them; the plan that settles the chain (`finish_plan`) compares the cards then, so a partial or
//! an intermediate plan announces nothing, and an undo, a project switch or a failed plan drops the queue.
use super::code_text::counted;
use super::messages::{EntryAction, Severity};
use super::{labels, sets, AppModel};
use engine::config::Subtype;
use serde_json::Value;

/// One type choice waiting for the settled plan: the item and the type it was last written as.
#[derive(Debug, Clone)]
pub(super) struct NewSetNote {
    item: String,
    subtype: Subtype,
}

impl AppModel {
    /// The item's own written `subtype`, if any.
    pub(super) fn own_subtype(&self, key: &str) -> Option<Value> {
        self.project.as_ref()?.file_value(key, "subtype")
    }

    /// Every card ID listed now, document-only cards included, filtered or not (`sets::views`); `None` while a group
    /// already holds its snapshot.
    pub(super) fn cards_now(&self) -> Option<Vec<String>> {
        self.carded_before.is_none().then(|| {
            sets::views(self)
                .iter()
                .map(|v| v.id.clone())
                .collect::<Vec<_>>()
        })
    }

    /// A choice took: queue it (the latest choice of an item stands), keeping the first snapshot of the group.
    pub(super) fn queue_new_set_note(
        &mut self,
        item: &str,
        subtype: Subtype,
        cards: Option<Vec<String>>,
    ) {
        if let (Some(root), Some(cards)) = (self.root.clone(), cards) {
            self.carded_before.get_or_insert((root, cards));
        }
        self.queue_into_group(item, subtype);
    }

    fn queue_into_group(&mut self, item: &str, subtype: Subtype) {
        if self.carded_before.is_none() {
            return;
        }
        match self.pending_new_sets.iter_mut().find(|n| n.item == item) {
            Some(n) => n.subtype = subtype,
            None => self.pending_new_sets.push(NewSetNote {
                item: item.to_string(),
                subtype,
            }),
        }
    }

    /// An undo, redo, open, cancel or failed plan: the queued choices no longer describe what is on screen.
    pub(super) fn forget_new_sets(&mut self) {
        self.pending_new_sets.clear();
        self.carded_before = None;
    }

    /// The settled plan: logs the cards that hold a queued item and were not listed before the first choice (compared
    /// by root-relative ID, so a card that gained a key or moved output root is not new), once, with Show.
    pub(super) fn note_new_sets(&mut self) {
        let pending = std::mem::take(&mut self.pending_new_sets);
        let Some((root, before)) = self.carded_before.take() else {
            return;
        };
        if pending.is_empty() || self.root.as_ref() != Some(&root) {
            return;
        }
        let views = sets::views(self);
        // Plan order; each card with the queued items it holds, in the order they were chosen.
        let mut fresh: Vec<(&sets::SetView, Vec<&NewSetNote>)> = vec![];
        for v in views.iter() {
            if before.iter().any(|b| engine::sets::same_id(b, &v.id)) {
                continue;
            }
            let held: Vec<&NewSetNote> = pending
                .iter()
                .filter(|n| v.members.contains(&n.item))
                .collect();
            if !held.is_empty() {
                fresh.push((v, held));
            }
        }
        let Some((first, held)) = fresh.first() else {
            return;
        };
        let ids: Vec<&str> = fresh.iter().map(|(v, _)| v.id.as_str()).collect();
        let text = match ids.as_slice() {
            [id] => {
                let n = first
                    .members
                    .iter()
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                let note = held[0];
                let file = self
                    .item(&note.item)
                    .map_or(note.item.as_str(), |i| i.name.as_str());
                format!(
                    "{file} is now {}. New set {id} ({}).",
                    labels::subtype_name(note.subtype),
                    counted(n, "texture", "textures")
                )
            }
            [a, b] => format!("2 new sets: {a} and {b}."),
            [a, b, c] => format!("3 new sets: {a}, {b} and {c}."),
            [a, b, c, rest @ ..] => {
                format!(
                    "{} new sets: {a}, {b}, {c} and {} more.",
                    ids.len(),
                    rest.len()
                )
            }
            [] => return,
        };
        let status = match ids.as_slice() {
            [id] => format!("New set {id}: see Messages."),
            _ => format!("{} new sets: see Messages.", ids.len()),
        };
        let action = EntryAction::ShowSet {
            root,
            id: first.id.clone(),
        };
        self.messages.log_with(Severity::Info, text, action);
        self.status_note = Some(status);
        self.counts_status();
    }
}
