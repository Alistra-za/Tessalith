//! E8: a type choice that makes a set card says so. The choice is queued when it is written, with the card IDs listed
//! before the first of them; the plan that settles the chain (`finish_plan`) compares the cards then, so a partial or
//! an intermediate plan announces nothing, and an undo, a project switch or a failed plan drops the queue. The line
//! names each item's type as it stands at settlement, so a reset after the choice leaves nothing to announce.
use super::code_text::counted;
use super::messages::{EntryAction, Severity};
use super::{labels, sets, AppModel};
use engine::config::Subtype;
use serde_json::Value;

/// One type choice waiting for the settled plan.
#[derive(Debug, Clone)]
pub(super) struct NewSetNote {
    item: String,
}

/// The one line for `ids` (plan order) when two or more cards are new.
fn several_line(ids: &[&str]) -> String {
    match ids {
        [a, b] => format!("2 new sets: {a} and {b}."),
        [a, b, c] => format!("3 new sets: {a}, {b} and {c}."),
        [a, b, c, rest @ ..] => format!(
            "{} new sets: {a}, {b}, {c} and {} more.",
            ids.len(),
            rest.len()
        ),
        _ => String::new(),
    }
}

impl AppModel {
    /// The item's own written `subtype`, if any.
    pub(super) fn own_subtype(&self, key: &str) -> Option<Value> {
        self.project.as_ref()?.file_value(key, "subtype")
    }

    /// Every card ID listed now, document-only cards included, filtered or not (`sets::views`).
    pub(super) fn cards_now(&self) -> Vec<String> {
        sets::views(self).iter().map(|v| v.id.clone()).collect()
    }

    /// Whether a group already holds its snapshot of the cards.
    pub(super) fn new_set_group_open(&self) -> bool {
        self.carded_before.is_some()
    }

    /// A choice took: queue the item, keeping the first snapshot of the group (`cards` is that snapshot, read before
    /// the write, and is `None` while a group is open).
    pub(super) fn queue_new_set_note(&mut self, item: &str, cards: Option<Vec<String>>) {
        if let (Some(root), Some(cards)) = (self.root.clone(), cards) {
            self.carded_before.get_or_insert((root, cards));
        }
        if self.carded_before.is_some() && !self.pending_new_sets.iter().any(|n| n.item == item) {
            self.pending_new_sets.push(NewSetNote {
                item: item.to_string(),
            });
        }
    }

    /// An undo, redo, open, cancel or failed plan: the queued choices no longer describe what is on screen.
    pub(super) fn forget_new_sets(&mut self) {
        self.pending_new_sets.clear();
        self.carded_before = None;
    }

    /// Tests: how many type choices wait for the settled plan.
    #[cfg(feature = "test-hooks")]
    pub fn new_set_notes_pending(&self) -> usize {
        self.pending_new_sets.len()
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
        // The type each item has now; one that no longer has an explicit choice (a reset) is dropped.
        let chosen: Vec<(String, Subtype)> = pending
            .into_iter()
            .filter_map(|n| {
                let st = serde_json::from_value(self.own_subtype(&n.item)?).ok()?;
                Some((n.item, st))
            })
            .collect();
        let views = sets::views(self);
        // Plan order; each card with the chosen items it holds, in the order they were chosen.
        let mut fresh: Vec<(&sets::SetView, Vec<&(String, Subtype)>)> = vec![];
        for v in views.iter() {
            if before.iter().any(|b| engine::sets::same_id(b, &v.id)) {
                continue;
            }
            let held: Vec<_> = chosen
                .iter()
                .filter(|(item, _)| v.members.contains(item))
                .collect();
            if !held.is_empty() {
                fresh.push((v, held));
            }
        }
        let Some((first, held)) = fresh.first() else {
            return;
        };
        let ids: Vec<&str> = fresh.iter().map(|(v, _)| v.id.as_str()).collect();
        let (text, status) = if fresh.len() == 1 {
            let n = first
                .members
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len();
            let (item, st) = held[0];
            let file = self.item(item).map_or(item.as_str(), |i| i.name.as_str());
            (
                format!(
                    "{file} is now {}. New set {} ({}).",
                    labels::subtype_name(*st),
                    first.id,
                    counted(n, "texture", "textures")
                ),
                format!("New set {}: see Messages.", first.id),
            )
        } else {
            (
                several_line(&ids),
                format!("{} new sets: see Messages.", ids.len()),
            )
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

#[cfg(test)]
mod tests {
    use super::several_line;

    #[test]
    fn several_new_sets_read_as_the_brief_words_them() {
        assert_eq!(several_line(&["a", "b"]), "2 new sets: a and b.");
        assert_eq!(several_line(&["a", "b", "c"]), "3 new sets: a, b and c.");
        assert_eq!(
            several_line(&["a", "b", "c", "d"]),
            "4 new sets: a, b, c and 1 more."
        );
        assert_eq!(
            several_line(&["a", "b", "c", "d", "e", "f"]),
            "6 new sets: a, b, c and 3 more."
        );
    }
}
