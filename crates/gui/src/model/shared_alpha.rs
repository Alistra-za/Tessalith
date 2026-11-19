//! Note JS7 (rule C, N-10 amended, N-27, N-33 as the owner narrowed it, N-34, N-37): a normal owned by a PBR or
//! Advanced Skin set and shared (slot 2, chains followed, own-first as JN1) into a set of any other shader keeps its
//! alpha. At the end of each user step that changed either document and plans again (once per user batch), the
//! follow-on compares the share before and after the step, both read inside it: it writes `alpha.policy = "data"` on
//! each owner's own Normal-slot file that became shared so, and lists it in `[project] auto_data`; it re-checks every
//! listed key and clears a value still `data` that no regular set shares. Nothing else is stored between steps. It
//! never runs in an automatic step (`AppModel::automatic_step`). A listed key the user moved to another value no
//! longer counts (the edit stays files-only); a user's Data pick takes the key off (N-34). What a step cannot see (a
//! set's automatic shader and its Normal-slot member are the last plan's) is Task 9's Note row.
use super::sets::{locked_items, views, SlotFill, SlotKind};
use super::AppModel;
use crate::project::ProjectDoc;
use crate::sets_doc::SetsDoc;
use engine::config::{AlphaPolicy, Subtype, Target};
use engine::presets::AlphaContent;
use engine::sets::fold_id;
use engine::shader::{Shader, ShaderOrigin};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// From the views (rebuilt when the sets document changed) and the last plan, by folded set id: each set's automatic
/// shader (a chosen one is read from the documents), its own Normal-slot file, the files in locked slots, the files
/// with no job or a kept DDS (`inert`), the files the last plan planned `data` (review 1 Minor 7), the files whose
/// analysis is known and whose alpha does not vary (`flat`, N-39), and the sets whose view holds no file (`empty`,
/// filled only while no plan is out: N-40).
#[derive(Default)]
pub(super) struct Facts {
    auto: BTreeMap<String, Shader>,
    own_n: BTreeMap<String, String>,
    locked: BTreeSet<String>,
    inert: BTreeSet<String>,
    planned_data: BTreeSet<String>,
    flat: BTreeSet<String>,
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
    for v in views(m).iter() {
        let id = fold_id(&v.id);
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
        if let Some(SlotFill::Own { item }) = v.slots.get(&SlotKind::Normal) {
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

/// The owner normals a regular set shares, with the borrows and chosen shaders of `s` as they are now (a chosen shader
/// wins over `f`'s automatic one), and the owner normals of the pairs where a set has no shader yet (`unsure`, review 4
/// Important 1). A borrower in `gone` (a set whose files are all gone) reads no texture and shares nothing.
// shortcut: a set with no shader until the next plan (New set, Duplicate into, a rename's `retarget_auto`, a set sent
// back to Auto, a set with no card) makes its pairs `unsure` (spec limit 4): that owner normal is neither written nor
// cleared in the step, and other keys are unaffected; after the plan, Task 9's row or a leftover `data` shows the result.
pub(super) fn shared_into_regular(
    f: &Facts,
    s: &SetsDoc,
    gone: &BTreeSet<String>,
) -> (BTreeSet<String>, BTreeSet<String>) {
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
    let mut out = BTreeSet::new();
    let mut unsure = BTreeSet::new();
    for (b, first) in &borrows {
        if gone.contains(b) {
            continue;
        }
        // JN1: follow the chain to the set that owns the file; a lender with its own Normal-slot file lends its own.
        let mut cur = first.clone();
        let mut seen = BTreeSet::from([b.clone()]);
        while !f.own_n.contains_key(&cur) {
            let Some(next) = borrows.get(&cur) else {
                break;
            };
            if !seen.insert(cur.clone()) {
                break;
            }
            cur = next.clone();
        }
        // A chain that reaches no Normal-slot file shares nothing, whatever the shaders: it is read first.
        let Some(n) = f.own_n.get(&cur) else {
            continue;
        };
        let (Some(bs), Some(os)) = (shader(b), shader(&cur)) else {
            unsure.insert(n.clone());
            continue;
        };
        if bs.is_pbr_or_advanced_skin() || !os.is_pbr_or_advanced_skin() {
            continue;
        }
        out.insert(n.clone());
    }
    (out, unsure)
}

/// A step's starting point (N-33), read before the step: the owner normals regular sets shared, the sub-type each
/// owner's Normal-slot file held, and the sets with no file (`empty`).
pub(super) struct Before {
    shared: BTreeSet<String>,
    sub: BTreeMap<String, Option<Value>>,
    empty: BTreeSet<String>,
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
    Some(Before {
        shared: shared_into_regular(&f, s, &f.empty).0,
        sub,
        empty: f.empty.clone(),
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
    let (now, unsure) = shared_into_regular(f, s, &gone);
    let data = Some(json!("data"));
    let start = s.auto_data();
    for k in now.difference(&b.shared) {
        // N-39: a known alpha that does not vary has nothing to keep; an unknown one is kept (the safe side).
        let free = !f.inert.contains(k)
            && !f.flat.contains(k)
            && !f.locked.contains(k)
            && p.file_value(k, "alpha.policy").is_none();
        if free && !already_data(f, b.sub.get(k), p, k) {
            c.write.push(k.clone());
        }
    }
    for k in &start {
        let ours = p.file_value(k, "alpha.policy") == data && !f.locked.contains(k);
        if ours && !now.contains(k) && !unsure.contains(k) {
            c.clear.push(k.clone());
        }
    }
    if !c.write.is_empty() || !c.clear.is_empty() {
        // N-27: stale entries (no longer `data`) are pruned only on this, Tessalith's own, write.
        let mut l: Vec<String> = start
            .iter()
            .filter(|k| !c.clear.contains(k) && p.file_value(k, "alpha.policy") == data)
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

fn apply(c: &Changes, p: &mut ProjectDoc, s: &mut SetsDoc) {
    for k in &c.write {
        p.set_file_value(k, "alpha.policy", &json!("data"));
    }
    for k in &c.clear {
        p.clear_file_value(k, "alpha.policy");
    }
    if c.relist {
        s.set_auto_data(&c.list);
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

    /// N-34 (review 2 Minor 3): the user's Data pick on a key the share step listed makes the value the user's: the
    /// key leaves `auto_data` in the pick's step (a rare write to `sdc-project.toml`). False when the key is not
    /// listed, so the caller writes the value as usual.
    pub(super) fn data_pick_unlists(&mut self, label: &str, key: &str) -> bool {
        let listed = self
            .sets_doc
            .as_ref()
            .is_some_and(|s| !s.read_only() && s.auto_data().iter().any(|k| k == key));
        if !listed || self.project.as_ref().is_none_or(ProjectDoc::read_only) {
            return false;
        }
        let k = key.to_string();
        self.edit_docs(label, None, true, move |p, s| {
            p.set_file_value(&k, "alpha.policy", &json!("data"));
            let mut l = s.auto_data();
            l.retain(|x| *x != k);
            s.set_auto_data(&l);
        });
        true
    }
}
