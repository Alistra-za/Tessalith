//! Displaced own DDS files that no planned material or PGPatcher entry reads.
use super::sets::{slot_string, views, SetView, SlotFill, LAST_GAME_SLOT};
use super::AppModel;
use engine::codes::{Code, Warning};
use engine::sets::set_id;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

pub type WarningsBySet = BTreeMap<String, Vec<Warning>>;

pub fn borrowed_unused(m: &AppModel) -> Arc<WarningsBySet> {
    super::cached(
        m,
        |c| {
            c.borrowed_unused
                .as_ref()
                .filter(|(rev, _)| *rev == m.evidence_rev)
                .map(|(_, w)| w.clone())
        },
        |c, w| c.borrowed_unused = Some((m.evidence_rev, w)),
        || Arc::new(build_borrowed_unused(m)),
    )
}

pub(crate) fn all_set_warnings<'a>(
    v: &'a SetView,
    unused: &'a WarningsBySet,
) -> impl Iterator<Item = &'a Warning> {
    v.set_warnings
        .iter()
        .chain(unused.get(&v.key).into_iter().flatten())
}

fn build_borrowed_unused(m: &AppModel) -> WarningsBySet {
    use super::{cs_skin_json, pbr_export, txst_map::slot_path_key};
    let views = views(m);
    let mut candidates = Vec::new();
    for v in views.iter() {
        let Some(own) = m.producers.get(&v.key) else {
            continue;
        };
        for (&kind, fill) in &v.slots {
            let (Some(n), SlotFill::Borrowed { from, item }) = (kind.slot_number(), fill) else {
                continue;
            };
            if v.filled.contains_key(&n) {
                continue;
            }
            let Some(lender) = m.item(item).and_then(|i| i.job.as_ref()) else {
                continue;
            };
            let same: Vec<_> = own
                .iter()
                .filter(|p| p.ready && p.kind == Some(kind) && p.output != lender.output)
                .collect();
            if !same.is_empty() {
                candidates.push((v, kind, n, from, &lender.output, same));
            }
        }
    }
    let mut warnings = WarningsBySet::new();
    if candidates.is_empty() {
        return warnings;
    }
    let entries = pbr_export::entries_by_set(m);
    let mut used = HashSet::new();
    for e in entries.values() {
        used.extend(
            (1..=LAST_GAME_SLOT)
                .filter_map(|n| e.input.written_slot(n))
                .map(slot_path_key),
        );
    }
    let skin = cs_skin_json::plan(m);
    let mut skin_slots = HashMap::new();
    let mut skin_files = HashSet::new();
    for row in &skin.rows {
        for (slot, output) in &row.outputs {
            skin_files.insert(output);
            if let Some((_, kind)) = cs_skin_json::SLOTS.iter().find(|(s, _)| s == slot) {
                skin_slots.insert((&row.key, *kind), output);
            }
        }
    }
    for (v, kind, n, from, lender, mut own) in candidates {
        let pgp = entries.get(super::bottom::scope_key(v)).is_some_and(|e| {
            (2..=LAST_GAME_SLOT).contains(&n)
                && slot_string(&v.settings, n).is_none()
                && e.input.written_slot(n).is_some()
                && e.winners.get(&n) == Some(&kind)
        });
        if !pgp && skin_slots.get(&(&v.key, kind)) != Some(&lender) {
            continue;
        }
        own.retain(|p| {
            !p.game_key.as_ref().is_some_and(|key| used.contains(key))
                && !skin_files.contains(&p.output)
        });
        own.sort_by_cached_key(|p| p.output.to_string_lossy().to_lowercase());
        if let Some((first, rest)) = own.split_first() {
            let causes: Vec<_> = first
                .sources
                .iter()
                .map(|source| {
                    let shared = first.packed() && v.shared_helpers.contains(source);
                    let owner = m
                        .item(source)
                        .filter(|_| shared)
                        .and_then(|i| i.set_key.as_deref())
                        .map(|key| set_id(&m.sets_root(), key));
                    serde_json::json!({"source": source, "shared": shared, "set": owner})
                })
                .collect();
            let more: Vec<_> = rest.iter().map(|p| p.output.to_string_lossy()).collect();
            let data = serde_json::json!({"slot": n, "kind": kind.suffix(), "output": first.output.to_string_lossy(),
                "more": more, "lender": from, "causes": causes});
            let message = super::code_text::borrowed_line(&data, first.packed());
            warnings
                .entry(v.key.clone())
                .or_default()
                .push(Warning::new(Code::SetSlotBorrowedUnused, message).with_data(data));
        }
    }
    warnings
}
