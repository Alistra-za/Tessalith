//! Note JS7 (rule C, N-10 amended, N-27, N-33 as the owner narrowed it, N-34, N-37): a normal owned by a PBR or
//! Advanced Skin set and shared (slot 2, chains followed, own-first as JN1) into a set of any other shader keeps its
//! alpha. At the end of each user step that changed either document and plans again (once per user batch), the
//! follow-on compares the share before and after the step, both read inside it: it writes `alpha.policy = "data"` on
//! each owner's own Normal-slot file that became shared so, and lists it in `[project] auto_data`; it re-checks every
//! listed key and clears a value still `data` that no regular set shares. Nothing else is stored between steps. It
//! never runs in an automatic step (`AppModel::automatic_step`). A listed key the user moved to another value no
//! longer counts (the edit stays files-only); a user's Data pick takes the key off (N-34). What a step cannot see (a
//! set's automatic shader and its Normal-slot member are the last plan's) is Task 9's Note row.
use super::normal_follow::holders;
use super::normal_follow::tangent;
use super::sets::{locked_items, own_fills, views, SlotFill, SlotKind};
use super::AppModel;
use crate::project::ProjectDoc;
use crate::sets_doc::SetsDoc;
use engine::config::{AlphaPolicy, Subtype, Target};
use engine::presets::AlphaContent;
use engine::sets::fold_id;
use engine::shader::{Shader, ShaderOrigin};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// From the views (rebuilt when the sets document changed) and the last plan, by folded set id: the sets shown
/// (`known`), each set's automatic shader (a chosen one is read from the documents), its own Normal-slot file before
/// any borrow (`own_n`, the set resolver's own fills, N8-b), the files in locked slots, the files with no job or a kept
/// DDS (`inert`), the files the last plan planned `data` (review 1 Minor 7), the files whose analysis is known and
/// whose alpha does not vary (`flat`, N-39), the sets whose view holds no file (`memberless`), those of them read
/// while no plan is out (`empty`, N-40), and whether a plan is out (`plan_out`).
#[derive(Default)]
pub(super) struct Facts {
    plan_out: bool,
    known: BTreeSet<String>,
    auto: BTreeMap<String, Shader>,
    own_n: BTreeMap<String, String>,
    locked: BTreeSet<String>,
    inert: BTreeSet<String>,
    planned_data: BTreeSet<String>,
    flat: BTreeSet<String>,
    memberless: BTreeSet<String>,
    empty: BTreeSet<String>,
}

pub(super) fn facts(m: &AppModel) -> Facts {
    let mut f = Facts {
        locked: locked_items(m).into_keys().collect(),
        ..Facts::default()
    };
    // N-40 (review 5 Minor 1): view members come from the last landed plan, so while a plan is out a set it does not
    // know yet (New set, Duplicate into, a rename's new id) has none either; nothing counts as empty then, and such
    // sets stay `unsure`.
    let plan_out = m.full_plan_pending();
    f.plan_out = plan_out;
    for v in views(m).iter() {
        let id = fold_id(&v.id);
        f.known.insert(id.clone());
        if v.members.is_empty() {
            f.memberless.insert(id.clone());
        }
        if v.members.is_empty() && !plan_out {
            f.empty.insert(id.clone());
        }
        if let Some(p) = v
            .shader
            .as_ref()
            .filter(|p| p.provenance != ShaderOrigin::Set)
        {
            f.auto.insert(id.clone(), p.shader);
        }
        // N8-b: the set's own file, as the set resolver lends it (own-first, JN1), not the displayed slot, which shows
        // a borrow line in place of it.
        if let Some(SlotFill::Own { item }) = own_fills(m, &v.members).get(&SlotKind::Normal) {
            match m
                .item(item)
                .filter(|i| !i.kept_dds())
                .and_then(|i| i.job.as_ref())
            {
                None => {
                    f.inert.insert(item.clone());
                }
                Some(j) if j.profile.alpha.policy == AlphaPolicy::Data => {
                    f.planned_data.insert(item.clone());
                }
                Some(_) => {}
            }
            let known = m.item(item).and_then(|i| i.analysis.as_ref());
            if known.is_some_and(|a| a.alpha != AlphaContent::Varying) {
                f.flat.insert(item.clone());
            }
            f.own_n.insert(id, item.clone());
        }
    }
    f
}

/// `shared_into_regular`'s answer.
#[derive(Default)]
pub(super) struct Share {
    /// The owner normals a regular set shares.
    pub out: BTreeSet<String>,
    /// The owner normals of the pairs the step cannot judge (a set with no shader yet, a borrower with no files yet).
    pub unsure: BTreeSet<String>,
    /// A borrow reaches a set the step cannot read (no files yet, not gone), so which file it reads is not known: no
    /// listed key can be cleared in the step (N8-b).
    pub blind: bool,
}

/// The owner normals a regular set shares, with the borrows and chosen shaders of `s` as they are now (a chosen shader
/// wins over `f`'s automatic one), and the owner normals of the pairs where a set has no shader yet (`unsure`, review 4
/// Important 1). A borrower in `gone` (a set whose files are all gone) reads no texture and shares nothing. N8-b: a set
/// with no files that is not gone (New set, Duplicate into, a rename's new id: the plan has not given it its files) is
/// not known in the step: as a borrower its pair is `unsure`; on the way to the owner it makes the step `blind`. So does
/// a borrow target with no view while a plan is out, or one the step itself named (`targets` holds the borrow targets
/// the step started from): an automatic rename's new id has no entry and no view until its plan lands. A target with
/// no view that the step started from, read while no plan is out, is gone: the borrow line fills nothing.
// shortcut: a set with no shader until the next plan (New set, Duplicate into, a rename's `retarget_auto`, a set sent
// back to Auto, a set with no card) makes its pairs `unsure` (spec limit 4): that owner normal is neither written nor
// cleared in the step, and other keys are unaffected; after the plan, Task 9's row or a leftover `data` shows the result.
// A `blind` step clears no listed key at all, since the key the borrow reads is not known.
pub(super) fn shared_into_regular(
    f: &Facts,
    s: &SetsDoc,
    gone: &BTreeSet<String>,
    targets: &BTreeSet<String>,
) -> Share {
    let mut borrows = BTreeMap::new();
    let mut chosen = BTreeMap::new();
    for e in s.entries().iter() {
        let id = fold_id(&e.id);
        if let Some(l) = e.borrow.get(&2) {
            borrows.insert(id.clone(), fold_id(l));
        }
        if let Some(x) = e.shader {
            chosen.insert(id, x);
        }
    }
    let shader = |id: &str| chosen.get(id).copied().or_else(|| f.auto.get(id).copied());
    let unread = |id: &str| f.memberless.contains(id) && !gone.contains(id);
    let mut sh = Share::default();
    for (b, first) in &borrows {
        if gone.contains(b) {
            continue;
        }
        // JN1, walked as the set resolver walks it (`entry_view`): a lender with its own Normal-slot file lends its
        // own; a set with no card, or a loop, ends the chain with nothing.
        let mut cur = first.clone();
        let mut seen = BTreeSet::from([b.clone()]);
        let owner = loop {
            if !seen.insert(cur.clone()) {
                break None;
            }
            if !f.known.contains(&cur) {
                sh.blind |= f.plan_out || !targets.contains(&cur);
                break None;
            }
            if unread(&cur) {
                sh.blind = true;
                break None;
            }
            if let Some(n) = f.own_n.get(&cur) {
                break Some((cur, n));
            }
            let Some(next) = borrows.get(&cur) else {
                break None;
            };
            cur = next.clone();
        };
        // A chain that reaches no Normal-slot file shares nothing, whatever the shaders: it is read first.
        let Some((cur, n)) = owner else {
            continue;
        };
        let (Some(bs), Some(os)) = (shader(b), shader(&cur)) else {
            sh.unsure.insert(n.clone());
            continue;
        };
        if !f.known.contains(b) || unread(b) {
            sh.unsure.insert(n.clone());
            continue;
        }
        if bs.is_pbr_or_advanced_skin() || !os.is_pbr_or_advanced_skin() {
            continue;
        }
        sh.out.insert(n.clone());
    }
    sh
}

/// A step's starting point (N-33), read before the step: the owner normals regular sets shared, the sub-type each
/// owner's Normal-slot file held, the sets with no file (`empty`) and the slot-2 borrow targets (`targets`, N8-b).
pub(super) struct Before {
    shared: BTreeSet<String>,
    sub: BTreeMap<String, Option<Value>>,
    empty: BTreeSet<String>,
    targets: BTreeSet<String>,
}

/// The sets the slot-2 borrow lines of `s` name, by folded id.
fn borrow_targets(s: &SetsDoc) -> BTreeSet<String> {
    s.entries()
        .iter()
        .filter_map(|e| e.borrow.get(&2).map(|l| fold_id(l)))
        .collect()
}

pub(super) fn before(m: &AppModel) -> Option<Before> {
    if m.target != Target::Cs {
        return None;
    }
    let (p, s) = (m.project.as_ref()?, m.sets_doc.as_ref()?);
    let f = facts(m);
    let sub = f
        .own_n
        .values()
        .map(|k| (k.clone(), p.file_value(k, "subtype")))
        .collect();
    let targets = borrow_targets(s);
    Some(Before {
        shared: shared_into_regular(&f, s, &f.empty, &targets).out,
        sub,
        empty: f.empty.clone(),
        targets,
    })
}

/// Review 2 Important 3: whether `k` is planned `data` after the step. The last plan answers for a sub-type the step
/// left alone; a sub-type the step changed answers by its new type's built-in alpha (`normal`, `landscape_normal`:
/// `data`; `pbr_normal` and a cleared type: not).
fn already_data(f: &Facts, pre: Option<&Option<Value>>, p: &ProjectDoc, k: &str) -> bool {
    let post = p.file_value(k, "subtype");
    match pre {
        Some(pre) if *pre != post => matches!(
            post.and_then(|v| serde_json::from_value::<Subtype>(v).ok()),
            Some(Subtype::Normal | Subtype::LandscapeNormal)
        ),
        _ => f.planned_data.contains(k),
    }
}

/// N8-e: the `[files]` table the share step writes `data` into for file `key`, and so the exact string it lists: the
/// table spelled `key`, else the one `find_key` picks among its full-name spellings in another case (the first in
/// `[files]` order), else a new `[files."<key>"]`. `None` when the file's only table is spelled without the extension:
/// `set_file_value` would write there, where a `data` is the user's (no write; Task 9's row shows it).
fn write_table(keys: &[String], key: &str) -> Option<String> {
    if keys.iter().any(|k| k == key) {
        return Some(key.to_string());
    }
    let lower = key.to_lowercase();
    if let Some(k) = keys.iter().find(|k| k.to_lowercase() == lower) {
        return Some(k.clone());
    }
    (!keys
        .iter()
        .any(|k| engine::overrides::path_key_matches(k, key)))
    .then(|| key.to_string())
}

/// N8-e: the `alpha.policy` of the table spelled byte for byte `entry` (a list entry): no case or Unicode twin, no
/// extension-less table, no fallback.
fn exact_alpha(p: &ProjectDoc, entry: &str) -> Option<Value> {
    p.has_file_key(entry)
        .then(|| p.file_value(entry, "alpha.policy"))
        .flatten()
}

/// Whether list entry `entry` names one of `items` (item keys), as the engine matches a `[files]` key to a file.
fn names_any(entry: &str, items: &BTreeSet<String>) -> bool {
    items
        .iter()
        .any(|i| engine::overrides::path_key_matches(entry, i))
}

/// What one run changes (Note JS7).
#[derive(Default)]
struct Changes {
    write: Vec<String>,
    clear: Vec<String>,
    list: Vec<String>,
    relist: bool,
}

/// One run, from the documents as the step left them: `data` on each key shared now and not at `b`; every listed key
/// no regular set shares, still `data`, cleared (N-33).
fn changes(f: &Facts, b: &Before, p: &ProjectDoc, s: &SetsDoc) -> Changes {
    let mut c = Changes::default();
    if p.read_only() || s.read_only() {
        return c;
    }
    // Review 4 Important 1: a set with no file before and after the step is one whose files are gone (a set the step
    // creates or renames has no view before it, so it is `unsure` instead).
    let gone: BTreeSet<String> = f.empty.intersection(&b.empty).cloned().collect();
    let sh = shared_into_regular(f, s, &gone, &b.targets);
    let (now, unsure) = (&sh.out, &sh.unsure);
    let data = Some(json!("data"));
    let start = s.auto_data();
    // N8-a: alpha is read as Task 6 reads it, from every `[files]` table that matches the file (read once per run).
    let keys = p.file_keys();
    let alpha = |k: &str| holders(p, &keys, k, "alpha.policy");
    // N8-b: the document wins over the last plan: a file a `[files]` table types as anything but a tangent-space
    // normal is not an owner normal, whatever slot the last plan put it in.
    let typed_normal = |k: &str| {
        holders(p, &keys, k, "subtype").iter().all(|h| {
            p.file_value(h, "subtype")
                .and_then(|v| serde_json::from_value::<Subtype>(v).ok())
                .is_some_and(tangent)
        })
    };
    for k in now.difference(&b.shared) {
        // N-39: a known alpha that does not vary has nothing to keep; an unknown one is kept (the safe side).
        let free = !f.inert.contains(k)
            && !f.flat.contains(k)
            && !f.locked.contains(k)
            && alpha(k).is_empty()
            && typed_normal(k);
        if free && !already_data(f, b.sub.get(k), p, k) {
            // N8-e: the entry is the exact table written.
            if let Some(t) = write_table(&keys, k) {
                c.write.push(t);
            }
        }
    }
    for k in &start {
        // N8-e: only the `data` the share step wrote: the table spelled exactly as the entry; a `data` in any other
        // table is the user's.
        let ours = exact_alpha(p, k) == data && !names_any(k, &f.locked);
        // N8-b: in a `blind` step a borrow may read any of them.
        if ours && !names_any(k, now) && !names_any(k, unsure) && !sh.blind {
            c.clear.push(k.clone());
        }
    }
    if !c.write.is_empty() || !c.clear.is_empty() {
        // N-27: stale entries (no longer `data`) are pruned only on this, Tessalith's own, write.
        let mut l: Vec<String> = start
            .iter()
            .filter(|k| !c.clear.contains(k) && exact_alpha(p, k) == data)
            .cloned()
            .collect();
        for k in &c.write {
            if !l.contains(k) {
                l.push(k.clone());
            }
        }
        c.relist = l != start;
        c.list = l;
    }
    c
}

/// N8-c (N-34): the listed keys with the `alpha.policy` each holds, read before a user's step.
pub(super) fn listed_alpha(p: &ProjectDoc, s: &SetsDoc) -> Vec<(String, Option<Value>)> {
    s.auto_data()
        .into_iter()
        .map(|k| {
            let v = p.file_value(&k, "alpha.policy");
            (k, v)
        })
        .collect()
}

/// N8-c (N-34): a listed key whose `alpha.policy` the user's step changed to `data` holds the user's own value now: it
/// leaves `auto_data` in that step, before the follow-on runs, so the follow-on's own writes never count. A pick of the
/// `data` the key already holds changes nothing a compare can see: `data_pick_unlists` takes that one.
pub(super) fn unlist_user_data(held: &[(String, Option<Value>)], p: &ProjectDoc, s: &mut SetsDoc) {
    let data = Some(json!("data"));
    let mine: Vec<&String> = held
        .iter()
        .filter(|(k, v)| *v != data && p.file_value(k, "alpha.policy") == data)
        .map(|(k, _)| k)
        .collect();
    if mine.is_empty() {
        return;
    }
    let mut l = s.auto_data();
    l.retain(|k| !mine.contains(&k));
    s.set_auto_data(&l);
}

/// N8-d: a value and its list entry are written together or not at all. The list goes first and is read back: one that
/// cannot be written (`[project]` is not a table) leaves every value alone. A value that then does not take (its
/// `[files]` entry is not a table) puts its key's entry back as it was.
fn apply(c: &Changes, p: &mut ProjectDoc, s: &mut SetsDoc) {
    if c.relist {
        s.set_auto_data(&c.list);
        if s.auto_data() != c.list {
            return;
        }
    }
    let data = Some(json!("data"));
    for k in &c.write {
        p.set_file_value(k, "alpha.policy", &json!("data"));
    }
    for k in &c.clear {
        p.clear_file_value(k, "alpha.policy");
    }
    let mut l = c.list.clone();
    l.retain(|k| !c.write.contains(k) || exact_alpha(p, k) == data);
    for k in c.clear.iter().filter(|k| exact_alpha(p, k) == data) {
        l.push(k.clone());
    }
    if l != c.list {
        s.set_auto_data(&l);
    }
}

impl AppModel {
    /// Note JS7 (N-10, N-33): the follow-on at the end of a user's step on cs, inside the step, from its starting
    /// point.
    pub(super) fn follow_shared(&mut self, b: &Before) {
        let fx = facts(self);
        let Some(c) = self
            .project
            .as_ref()
            .zip(self.sets_doc.as_ref())
            .map(|(p, s)| changes(&fx, b, p, s))
        else {
            return;
        };
        if let (Some(p), Some(s)) = (self.project.as_mut(), self.sets_doc.as_mut()) {
            apply(&c, p, s);
        }
    }

    /// N-34 (review 2 Minor 3): the user's Data pick on a file the share step listed makes the value the user's: the
    /// entries naming the file leave `auto_data` in the pick's step (a rare write to `sdc-project.toml`). False when
    /// none is listed, so the caller writes the value as usual.
    pub(super) fn data_pick_unlists(&mut self, label: &str, key: &str) -> bool {
        let names = |e: &String| engine::overrides::path_key_matches(e, key);
        let listed = self
            .sets_doc
            .as_ref()
            .is_some_and(|s| !s.read_only() && s.auto_data().iter().any(names));
        if !listed || self.project.as_ref().is_none_or(ProjectDoc::read_only) {
            return false;
        }
        let k = key.to_string();
        self.edit_docs(label, None, true, move |p, s| {
            p.set_file_value(&k, "alpha.policy", &json!("data"));
            let mut l = s.auto_data();
            l.retain(|x| !engine::overrides::path_key_matches(x, &k));
            s.set_auto_data(&l);
        });
        true
    }
}
