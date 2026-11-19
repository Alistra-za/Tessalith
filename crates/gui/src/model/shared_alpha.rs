//! Note JS7 (rule C, N-10 amended, N-27, N-33 as the owner narrowed it, N-34, N-37): a normal owned by a PBR or
//! Advanced Skin set and shared (slot 2, chains followed, own-first as JN1) into a set of any other shader keeps its
//! alpha. At the end of each user step that changed either document and plans again (once per user batch), the
//! follow-on compares the share before and after the step, both read inside it: it writes `alpha.policy = "data"` on
//! each owner's own Normal-slot file that became shared so, and lists it in `[project] auto_data`; it re-checks every
//! listed key and clears a value still `data` that no regular set shares. Nothing else is stored between steps. It
//! never runs in an automatic step (`AppModel::automatic_step`). A listed key the user moved to another value no
//! longer counts (the edit stays files-only); a user's Data pick takes the key off (N-34). What a step cannot see (a
//! set's automatic shader and its Normal-slot member are the last plan's) is Task 9's Note row.
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
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

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
// N8-f: a lender in `touched` (a set whose files the step's own edit moved, so the views' members are not what the
// documents say) is not known either: a chain through it is `blind`, and the key it would lend is not named, so
// nothing is written for it.
pub(super) fn shared_into_regular(
    f: &Facts,
    s: &SetsDoc,
    gone: &BTreeSet<String>,
    targets: &BTreeSet<String>,
    touched: &BTreeSet<String>,
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
            if touched.contains(&cur) {
                sh.blind = true;
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
/// owner's Normal-slot file held, the sets with no file (`empty`), the slot-2 borrow targets (`targets`, N8-b), and
/// for N8-f: whether the views' members are what the documents say (`fresh`), each entry's membership lines
/// (`members`) and the `[inputs]` aliases (`aliases`).
pub(super) struct Before {
    shared: BTreeSet<String>,
    sub: BTreeMap<String, Option<Value>>,
    empty: BTreeSet<String>,
    targets: BTreeSet<String>,
    fresh: bool,
    members: BTreeMap<String, Membership>,
    aliases: BTreeMap<String, String>,
}

/// What an `[[sets]]` entry says about which files the set holds: `created`, `renamed_from`, `add`, `remove`.
type Membership = (bool, Option<String>, Vec<String>, Vec<String>);

/// N8-f: every entry's membership lines, by folded id.
fn membership(s: &SetsDoc) -> BTreeMap<String, Membership> {
    s.entries()
        .iter()
        .map(|e| {
            let m = (
                e.created,
                e.renamed_from.clone(),
                e.add.clone(),
                e.remove.clone(),
            );
            (fold_id(&e.id), m)
        })
        .collect()
}

/// N8-f: the `[inputs]` aliases (a new input name moves a file to another automatic set at the next plan).
fn aliases(p: &ProjectDoc) -> BTreeMap<String, String> {
    p.read_as_all()
        .into_iter()
        .map(|(k, a)| (k, a.path))
        .collect()
}

/// N8-f: the sets whose files the step's own edit moved: an entry whose membership lines changed (a move, a replace,
/// Fill from names, a hand-over, a rename, a new or deleted entry), and every set whose view holds a file one of
/// those lines names (the set the file leaves). The views keep the last plan's members, so for these sets they are
/// not what the documents say until the plan lands.
fn touched(m: &AppModel, b: &Before, s: &SetsDoc) -> BTreeSet<String> {
    let now = membership(s);
    let mut out = BTreeSet::new();
    let mut lines = BTreeSet::new();
    // An entry that is not there says what an entry with no membership lines says (a borrow or a shader pick that
    // creates the entry moves no file).
    let none = Membership::default();
    for id in b.members.keys().chain(now.keys()) {
        let (x, y) = (
            b.members.get(id).unwrap_or(&none),
            now.get(id).unwrap_or(&none),
        );
        if x != y {
            out.insert(id.clone());
            for (_, _, add, remove) in [x, y] {
                lines.extend(add.iter().chain(remove).cloned());
            }
        }
    }
    if !lines.is_empty() {
        for v in views(m).iter() {
            let holds = v.members.iter().any(|mk| {
                lines
                    .iter()
                    .any(|l| engine::overrides::path_key_matches(l, mk))
            });
            if holds {
                out.insert(fold_id(&v.id));
            }
        }
    }
    out
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
    // N8-f: the views' members are what the documents say only when the last landed full plan was asked with the
    // documents as they are outside `[files]`: the fast re-plan's request signature (`plan_sig`, set by every
    // re-plan) still equals the landed one's. A set edit or an alias since then (a plan out, or failed) differs.
    let fresh = m.landed_sig.is_some_and(|l| l == m.plan_sig.1);
    Some(Before {
        shared: shared_into_regular(&f, s, &f.empty, &targets, &BTreeSet::new()).out,
        sub,
        empty: f.empty.clone(),
        targets,
        fresh,
        members: membership(s),
        aliases: aliases(p),
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

/// The forms a `[files]` key is compared with for project-relative path `rel`, as `engine::overrides::KeyForms` makes
/// them (its fields are the engine's own): the path lower-cased, and lower-cased without the last extension of its
/// file name when that leaves a non-empty stem. A key names the file when its lower-cased spelling is one of them.
fn forms(rel: &str) -> (String, Option<String>) {
    let full = rel.to_lowercase();
    let (dir, name) = full.rsplit_once('/').unwrap_or(("", full.as_str()));
    let stem = match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => Some(if dir.is_empty() {
            stem.to_string()
        } else {
            format!("{dir}/{stem}")
        }),
        _ => None,
    };
    (full, stem)
}

/// N8-j: the `[files]` tables of one document state, built in one pass over its keys: each table's spelling with its
/// `alpha.policy` and `subtype`, indexed by the lower-cased spelling. A file's matching tables are then two lookups (its
/// two `forms`), never a walk over every key.
pub(super) struct Tables {
    rows: rows::Rows,
    by_lower: HashMap<String, Vec<usize>>,
}

/// One table: its spelling, `alpha.policy` and `subtype`.
type Row = (String, Option<Value>, Option<Value>);

/// The rows, readable only one index at a time through `get`, which counts each read under `cfg(test)`: a lookup that
/// walked every row would have to read each one, and the count shows it.
mod rows {
    use super::Row;

    #[derive(Default)]
    pub(super) struct Rows {
        all: Vec<Row>,
        #[cfg(test)]
        pub(super) visits: std::cell::Cell<usize>,
    }

    impl Rows {
        pub(super) fn push(&mut self, row: Row) -> usize {
            self.all.push(row);
            self.all.len() - 1
        }

        pub(super) fn get(&self, i: usize) -> &Row {
            #[cfg(test)]
            self.visits.set(self.visits.get() + 1);
            &self.all[i]
        }

        #[cfg(test)]
        pub(super) fn len(&self) -> usize {
            self.all.len()
        }
    }
}

impl Tables {
    pub(super) fn of(p: &ProjectDoc) -> Tables {
        let mut t = Tables {
            rows: rows::Rows::default(),
            by_lower: HashMap::new(),
        };
        for k in p.file_keys() {
            let lower = k.to_lowercase();
            let row = (
                k.clone(),
                p.file_value(&k, "alpha.policy"),
                p.file_value(&k, "subtype"),
            );
            let i = t.rows.push(row);
            t.by_lower.entry(lower).or_default().push(i);
        }
        t
    }

    /// The tables that match file `key` as the engine matches them, in `[files]` order.
    fn matching(&self, key: &str) -> Vec<&Row> {
        let (full, stem) = forms(key);
        let mut at: Vec<usize> = std::iter::once(full)
            .chain(stem)
            .filter_map(|f| self.by_lower.get(&f))
            .flatten()
            .copied()
            .collect();
        at.sort_unstable();
        at.dedup();
        at.into_iter().map(|i| self.rows.get(i)).collect()
    }

    /// The spellings of the tables that hold an `alpha.policy` for file `key`, with it.
    fn alpha(&self, key: &str) -> BTreeMap<String, Value> {
        self.matching(key)
            .into_iter()
            .filter_map(|(k, a, _)| Some((k.clone(), a.clone()?)))
            .collect()
    }

    /// N8-b: every sub-type a table of file `key` holds is a tangent-space normal (none held: the plan's type stands).
    fn typed_normal(&self, key: &str) -> bool {
        self.matching(key).into_iter().all(|(_, _, st)| {
            st.as_ref().is_none_or(|v| {
                serde_json::from_value::<Subtype>(v.clone())
                    .ok()
                    .is_some_and(tangent)
            })
        })
    }

    /// N8-e: the `[files]` table the share step writes `data` into for file `key`, and so the exact string it lists:
    /// the table spelled `key`, else the one `find_key` picks among its full-name spellings in another case (the
    /// first in `[files]` order), else a new `[files."<key>"]`. `None` when the file's only table is spelled without
    /// the extension: `set_file_value` would write there, where a `data` is the user's (no write; Task 9's row).
    fn write_table(&self, key: &str) -> Option<String> {
        let tables = self.matching(key);
        if tables.iter().any(|(k, ..)| k == key) {
            return Some(key.to_string());
        }
        let lower = key.to_lowercase();
        if let Some((k, ..)) = tables.iter().find(|(k, ..)| k.to_lowercase() == lower) {
            return Some(k.clone());
        }
        tables.is_empty().then(|| key.to_string())
    }

    /// N8-e: the `alpha.policy` of the table spelled byte for byte `entry` (a list entry): no case or Unicode twin,
    /// no extension-less table, no fallback.
    fn exact_alpha(&self, entry: &str) -> Option<Value> {
        let at = self.by_lower.get(&entry.to_lowercase())?;
        at.iter()
            .map(|i| self.rows.get(*i))
            .find(|(k, ..)| k == entry)
            .and_then(|(_, a, _)| a.clone())
    }
}

/// N8-e: the `alpha.policy` of the table spelled byte for byte `entry`, read from the document (a few keys after a
/// write: `has_file_key` and an exact `file_value` are lookups, not walks).
fn exact_alpha(p: &ProjectDoc, entry: &str) -> Option<Value> {
    p.has_file_key(entry)
        .then(|| p.file_value(entry, "alpha.policy"))
        .flatten()
}

/// The forms of every key in `items`, so a list entry is matched to them by one lookup (`names`).
fn all_forms(items: &BTreeSet<String>) -> HashSet<String> {
    items
        .iter()
        .flat_map(|i| {
            let (full, stem) = forms(i);
            std::iter::once(full).chain(stem)
        })
        .collect()
}

/// Whether list entry `entry` names one of the files whose `all_forms` are `forms`.
fn names(entry: &str, forms: &HashSet<String>) -> bool {
    forms.contains(&entry.to_lowercase())
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
fn changes(
    f: &Facts,
    b: &Before,
    p: &ProjectDoc,
    s: &SetsDoc,
    touched: &BTreeSet<String>,
) -> Changes {
    let mut c = Changes::default();
    if p.read_only() || s.read_only() {
        return c;
    }
    // N8-f: views older than the documents (a set edit or an alias whose plan has not landed), or an alias the step
    // changed (it moves files between automatic sets at the next plan): no set is known, so nothing is written or
    // cleared.
    if !b.fresh || aliases(p) != b.aliases {
        return c;
    }
    // Review 4 Important 1: a set with no file before and after the step is one whose files are gone (a set the step
    // creates or renames has no view before it, so it is `unsure` instead).
    let gone: BTreeSet<String> = f.empty.intersection(&b.empty).cloned().collect();
    let sh = shared_into_regular(f, s, &gone, &b.targets, touched);
    let (now, unsure) = (&sh.out, &sh.unsure);
    let data = Some(json!("data"));
    let start = s.auto_data();
    // N8-a, N8-j: alpha is read as Task 6 reads it, from every `[files]` table that matches the file; the tables are
    // read in one pass over the keys, and each file asks two lookups.
    let tables = Tables::of(p);
    // N8-f: a key any borrower makes unsure is not written either.
    for k in now.difference(&b.shared).filter(|k| !unsure.contains(*k)) {
        // N-39: a known alpha that does not vary has nothing to keep; an unknown one is kept (the safe side).
        // N8-b: the document wins over the last plan: a file a `[files]` table types as anything but a tangent-space
        // normal is not an owner normal, whatever slot the last plan put it in.
        let free = !f.inert.contains(k)
            && !f.flat.contains(k)
            && !f.locked.contains(k)
            && tables.alpha(k).is_empty()
            && tables.typed_normal(k);
        if free && !already_data(f, b.sub.get(k), p, k) {
            // N8-e: the entry is the exact table written.
            if let Some(t) = tables.write_table(k) {
                c.write.push(t);
            }
        }
    }
    let (locked, now_f, unsure_f) = (all_forms(&f.locked), all_forms(now), all_forms(unsure));
    for k in &start {
        // N8-e: only the `data` the share step wrote: the table spelled exactly as the entry; a `data` in any other
        // table is the user's.
        let ours = tables.exact_alpha(k) == data && !names(k, &locked);
        // N8-b: in a `blind` step a borrow may read any of them.
        if ours && !names(k, &now_f) && !names(k, &unsure_f) && !sh.blind {
            c.clear.push(k.clone());
        }
    }
    if !c.write.is_empty() || !c.clear.is_empty() {
        // N-27: stale entries (no longer `data`) are pruned only on this, Tessalith's own, write.
        let cleared: HashSet<&String> = c.clear.iter().collect();
        let mut l: Vec<String> = start
            .iter()
            .filter(|k| !cleared.contains(k) && tables.exact_alpha(k) == data)
            .cloned()
            .collect();
        let mut kept: HashSet<String> = l.iter().cloned().collect();
        for k in &c.write {
            if kept.insert(k.clone()) {
                l.push(k.clone());
            }
        }
        c.relist = l != start;
        c.list = l;
    }
    c
}

/// Each list entry with the `alpha.policy` of every `[files]` table that matches its file, by the table's spelling.
pub(super) type ListedAlpha = Vec<(String, BTreeMap<String, Value>)>;

/// N8-c, N8-h (N-34): the list entries with the alpha of every table matching each, read before a user's step (one
/// pass over the `[files]` keys, N8-j).
pub(super) fn listed_alpha(p: &ProjectDoc, s: &SetsDoc) -> ListedAlpha {
    let listed = s.auto_data();
    if listed.is_empty() {
        return vec![];
    }
    let tables = Tables::of(p);
    listed
        .into_iter()
        .map(|e| {
            let t = tables.alpha(&e);
            (e, t)
        })
        .collect()
}

/// N8-c, N8-h (N-34): an entry the user's step itself wrote `data` for, in any table matching its file (one that held
/// no `data` before the step), holds the user's own value now: it leaves `auto_data` in that step, before the
/// follow-on runs, so the follow-on's own writes never count. Tables are compared, not the value the file resolves
/// to: a step that removes a table and so uncovers a `data` another table already held wrote nothing (N-27: a reset
/// is files-only). A pick of the `data` a table already holds changes no table: `data_pick_unlists` takes that one.
pub(super) fn unlist_user_data(held: &ListedAlpha, p: &ProjectDoc, s: &mut SetsDoc) {
    if held.is_empty() {
        return;
    }
    let data = json!("data");
    let tables = Tables::of(p);
    let mine: HashSet<&String> = held
        .iter()
        .filter(|(e, before)| {
            tables
                .alpha(e)
                .iter()
                .any(|(t, v)| *v == data && before.get(t) != Some(&data))
        })
        .map(|(e, _)| e)
        .collect();
    if mine.is_empty() {
        return;
    }
    let mut l = s.auto_data();
    l.retain(|k| !mine.contains(&k));
    s.set_auto_data(&l);
}

/// N8-d, N8-i: a value and its list entry are written together or not at all, and a failed attempt changes nothing.
/// The list goes first and is read back: one that cannot be written (`[project]` is not a table) leaves every value
/// alone. If a value then does not take (its `[files]` entry is not a table), both documents go back to the text they
/// had before the attempt: the list's membership as it was, no entry pruned, no file value changed. `fail` (tests
/// only) makes the file writes fail.
fn apply(c: &Changes, p: &mut ProjectDoc, s: &mut SetsDoc, fail: bool) {
    let (p0, s0) = (p.text(), s.text());
    if c.relist {
        s.set_auto_data(&c.list);
        if s.auto_data() != c.list {
            return;
        }
    }
    let data = Some(json!("data"));
    for k in c.write.iter().filter(|_| !fail) {
        p.set_file_value(k, "alpha.policy", &json!("data"));
    }
    for k in &c.clear {
        p.clear_file_value(k, "alpha.policy");
    }
    let took = c.write.iter().all(|k| exact_alpha(p, k) == data)
        && c.clear.iter().all(|k| exact_alpha(p, k) != data);
    if !took {
        p.restore(&p0);
        s.restore(&s0);
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
            .map(|(p, s)| changes(&fx, b, p, s, &touched(self, b, s)))
        else {
            return;
        };
        #[cfg(feature = "test-hooks")]
        let fail = self.fail_shared_alpha_write;
        #[cfg(not(feature = "test-hooks"))]
        let fail = false;
        if let (Some(p), Some(s)) = (self.project.as_mut(), self.sets_doc.as_mut()) {
            apply(&c, p, s, fail);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `forms` gives exactly the keys `engine::overrides::path_key_matches` accepts.
    #[test]
    fn forms_match_as_the_engine_matches() {
        let rels = [
            "pbr/iron_n.png",
            "PBR/Ärz_N.PNG",
            "iron_n.png",
            "a/.hidden",
            "a/b.c.png",
            "noext",
            "a/b/",
        ];
        let keys = [
            "pbr/iron_n.png",
            "PBR/IRON_N.PNG",
            "pbr/iron_n",
            "pbr/iron",
            "pbr\\iron_n.png",
            "pbr/ärz_n.png",
            "pbr/ärz_n",
            "iron_n",
            "a/.hidden",
            "a/",
            "a/b.c",
            "a/b",
            "noext",
            "a/b/",
        ];
        for rel in rels {
            let (full, stem) = forms(rel);
            for key in keys {
                let k = key.to_lowercase();
                let ours = k == full || stem.as_deref() == Some(k.as_str());
                assert_eq!(
                    ours,
                    engine::overrides::path_key_matches(key, rel),
                    "{key} vs {rel}"
                );
            }
        }
    }

    /// N8-j: on a project with 10,000 `[files]` keys and 2,000 listed entries, the tables are read in one pass and
    /// each entry's lookup examines only its own matching tables, never every key.
    #[test]
    fn the_holder_facts_are_built_once() {
        let d = tempfile::tempdir().unwrap();
        let mut text = String::new();
        for i in 0..10_000 {
            text.push_str(&format!(
                "[files.\"d/f{i:05}.png\"]\nalpha.policy = \"data\"\n"
            ));
        }
        std::fs::write(d.path().join("sdc.toml"), text).unwrap();
        let p = ProjectDoc::open(d.path());
        assert!(!p.read_only(), "{:?}", p.error());
        let t = Tables::of(&p);
        assert_eq!(t.rows.len(), 10_000, "one row per key: one pass");
        let listed: Vec<String> = (0..2_000).map(|i| format!("d/f{:05}.png", i * 5)).collect();
        for e in &listed {
            assert_eq!(t.alpha(e).len(), 1, "{e}");
            assert_eq!(t.exact_alpha(e), Some(json!("data")));
        }
        assert_eq!(
            t.rows.visits.get(),
            4_000,
            "each of the 4,000 lookups reads its own table only"
        );
    }
}
