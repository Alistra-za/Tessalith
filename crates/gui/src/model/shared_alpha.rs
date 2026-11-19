//! Note JS7 (rule C, N-10 amended, N-27, N-33 as the owner narrowed it, N-34, N-37): a normal owned by a PBR or
//! Advanced Skin set and shared (slot 2, chains followed, own-first as JN1) into a set of any other shader keeps its
//! alpha. At the end of each user step that changed either document and plans again (once per user batch), the
//! follow-on compares the share before and after the step, both read inside it: it writes `alpha.policy = "data"` on
//! each owner's own Normal-slot file that became shared so, and lists it in `[project] auto_data`; it re-checks every
//! listed key and clears a value still `data` that no regular set shares. Nothing else is stored between steps. It
//! never runs in an automatic step (`AppModel::automatic_step`). A listed key whose own table the user moved to another
//! value or removed leaves the list in the next step the follow-on runs in (N8-p); a user's Data pick takes the key off
//! (N-34). What a step cannot see (a set's automatic shader and its Normal-slot member are the last plan's) is Task 9's
//! Note row.
use super::bottom::{problems, At, Fix};
use super::normal_follow::tangent;
use super::sets::{lock_refusal, own_fills, views, SetView, SlotFill, SlotKind};
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
/// any borrow (`own_n`, the set resolver's own fills, N8-b), those with a locked owning slot, the files with no job
/// or a kept DDS (`inert`), the files the last plan planned `data` (review 1 Minor 7), the files whose analysis is known and
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
    // N-40 (review 5 Minor 1): view members come from the last landed plan, so while a plan is out a set it does not
    // know yet (New set, Duplicate into, a rename's new id) has none either; nothing counts as empty then, and such
    // sets stay `unsure`.
    let mut f = Facts {
        plan_out: m.full_plan_pending(),
        ..Facts::default()
    };
    for v in views(m).iter() {
        let id = fold_id(&v.id);
        f.known.insert(id.clone());
        if v.members.is_empty() {
            f.memberless.insert(id.clone());
        }
        if v.members.is_empty() && !f.plan_out {
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
            if v.locks.contains(&2) {
                f.locked.insert(item.clone());
            }
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
// N8-f, N8-m: a set in `touched` (one whose files the step's own edit added, removed, moved, duplicated or replaced,
// so the views' members are not what the documents say) is not known, on either side of the borrow: as a lender, a
// chain through it is `blind` and the key it would lend is not named, so nothing is written for it; as a borrower, it
// is never `gone` and the owner normal it reaches is `unsure`. A step that changes only a borrow line (an ordinary
// Share into a slot, an Unshare) touches no set.
pub(super) fn shared_into_regular(
    f: &Facts,
    s: &SetsDoc,
    borrows: &BTreeMap<String, String>,
    gone: &BTreeSet<String>,
    targets: &BTreeSet<String>,
    touched: &BTreeSet<String>,
) -> Share {
    let mut chosen = BTreeMap::new();
    for e in s.entries().iter() {
        if let Some(x) = e.shader {
            chosen.insert(fold_id(&e.id), x);
        }
    }
    let shader = |id: &str| chosen.get(id).copied().or_else(|| f.auto.get(id).copied());
    let unread = |id: &str| f.memberless.contains(id) && !gone.contains(id);
    let mut sh = Share::default();
    for (b, first) in borrows {
        // N8-m: a borrower whose membership the step changed is never gone from the views, which predate the step.
        let moved = touched.contains(b);
        if gone.contains(b) && !moved {
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
        // N8-m: nor is it known: its files and its automatic shader are the last plan's, so its pair is unsure.
        if moved {
            sh.unsure.insert(n.clone());
            continue;
        }
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

/// The planned regular borrowers' resolved Normal slots, including record fills. A Note carries the
/// exact table its button will edit, or the reason the button is disabled.
pub(super) fn rows(m: &AppModel, views: &[SetView]) -> Vec<(String, Result<String, String>)> {
    let (Target::Cs, Some(p), Some(_)) = (m.target, &m.project, &m.sets_doc) else {
        return vec![];
    };
    let owners: HashMap<_, _> = views
        .iter()
        .filter(|v| {
            v.shader
                .as_ref()
                .is_some_and(|p| p.shader.is_pbr_or_advanced_skin())
        })
        .map(|v| (fold_id(&v.id), v))
        .collect();
    let mut shared = BTreeMap::new();
    for v in views {
        if v.members.is_empty()
            || v.shader
                .as_ref()
                .is_none_or(|p| p.shader.is_pbr_or_advanced_skin())
        {
            continue;
        }
        let Some(SlotFill::Borrowed { from, item }) = v.slots.get(&SlotKind::Normal) else {
            continue;
        };
        if let Some(owner) = owners.get(&fold_id(from)) {
            shared.insert(item, *owner);
        }
    }
    if shared.is_empty() {
        return vec![];
    }
    let tables = Tables::of(p);
    let inputs = std::cell::LazyCell::new(|| input_matches(m, &m.cube_face_keys()));
    shared
        .into_iter()
        .filter_map(|(key, owner)| {
            let i = m.item(key)?;
            let profile = &i.job.as_ref()?.profile;
            if i.kept_dds()
                || !tangent(profile.subtype)
                || profile.alpha.policy == AlphaPolicy::Data
                || i.analysis.as_ref()?.alpha != AlphaContent::Varying
                || !tables.alpha(key).is_empty()
            {
                return None;
            }
            let table = tables
                .single_input_table(key, &inputs)
                .and_then(|(table, _)| {
                    lock_refusal(owner, Some(SlotKind::Normal))?;
                    Ok(table)
                });
            Some((key.clone(), table))
        })
        .collect()
}

#[derive(Clone, Copy)]
enum InputMatch {
    Full(Option<Subtype>),
    Stem,
    Multiple,
}

/// How each folded table key matches the landed inputs. Item keys keep the real name after an input alias.
fn input_matches(m: &AppModel, cubes: &[(String, Vec<String>)]) -> HashMap<String, InputMatch> {
    let mut matches = HashMap::new();
    let mut add = |key: &str, subtype| {
        let (full, stem) = forms(key);
        for (form, kind) in std::iter::once((full, InputMatch::Full(subtype)))
            .chain(stem.map(|s| (s, InputMatch::Stem)))
        {
            matches
                .entry(form)
                .and_modify(|m| *m = InputMatch::Multiple)
                .or_insert(kind);
        }
    };
    for item in &m.items {
        let subtype = item.job.as_ref().map(|j| j.profile.subtype);
        // A packed item's key is already its virtual stem. Resolve also matches its full virtual filename.
        if item.kind == super::ItemKind::Packed {
            if let Some((_, ext)) = item.job.as_ref().and_then(|j| j.game_path.rsplit_once('.')) {
                add(&format!("{}.{ext}", item.key), subtype);
                continue;
            }
        }
        add(&item.key, subtype);
    }
    // A six-face cube has one item for all six physical inputs.
    for (_, faces) in cubes {
        for face in faces {
            if m.item(face).is_none() {
                add(face, Some(Subtype::Cubemap));
            }
        }
    }
    matches
}

/// An automatic subtype edit uses the same one-input destination as Keep alpha.
pub(super) fn safe_subtype_write(m: &AppModel) -> impl Fn(&str) -> bool + '_ {
    let guard = std::cell::LazyCell::new(|| {
        m.project
            .as_ref()
            .map(|p| subtype_write_guard(m, Tables::of(p)))
    });
    move |key| guard.as_ref().is_some_and(|safe| safe(key))
}

/// The recorder also uses these tables to read the held subtype and alpha of each normal.
pub(super) fn subtype_write_guard(
    m: &AppModel,
    tables: impl std::borrow::Borrow<Tables>,
) -> impl Fn(&str) -> bool {
    let cubes = m.cube_face_keys();
    let inputs = input_matches(m, &cubes);
    // edit_docs copies the primary face's edit to all five secondary tables in the same step.
    let unsafe_cubes: HashSet<_> = cubes
        .into_iter()
        .filter(|(_, faces)| {
            faces
                .iter()
                .any(|k| tables.borrow().single_input_table(k, &inputs).is_err())
        })
        .map(|(key, _)| key)
        .collect();
    move |key| {
        !unsafe_cubes.contains(key) && tables.borrow().single_input_table(key, &inputs).is_ok()
    }
}

/// A step's starting point (N-33), read before the step: the owner normals regular sets shared, the sub-type each
/// owner's Normal-slot file held, the sets with no file (`empty`), the slot-2 borrow targets (`targets`, N8-b), and
/// for N8-f: whether the views' members are what the documents say (`fresh`), the sets document's membership fields
/// (`members`, N8-r) and the `[inputs]` aliases (`aliases`).
pub(super) struct Before {
    shared: BTreeSet<String>,
    sub: BTreeMap<String, Option<Value>>,
    empty: BTreeSet<String>,
    targets: BTreeSet<String>,
    fresh: bool,
    members: Membership,
    aliases: BTreeMap<String, String>,
}

/// N8-f, N8-r: what the sets document says about which files the sets hold, from every field the engine's set edits
/// are made of (`sets::engine_inputs`): each `[[sets]]` entry's `created`, `renamed_from`, `add`, `remove` and `share`
/// lines, by folded id, and `[project] no_set`. A borrow line, a shader pick or a lock moves no file. (The `[inputs]`
/// aliases, in the project document, are compared whole: `aliases`.) N8-u: every entry an id has, in document order:
/// the GUI edits the first entry of an id and the engine reads all of them, so a hand-written second entry must not
/// hide a change to the first.
#[derive(Debug, Default, PartialEq)]
struct Membership {
    entries: BTreeMap<String, Vec<Lines>>,
    no_set: BTreeSet<String>,
}

/// One entry's membership lines.
#[derive(Debug, Default, PartialEq)]
struct Lines {
    created: bool,
    renamed_from: Option<String>,
    add: Vec<String>,
    remove: Vec<String>,
    share: Vec<String>,
}

/// N8-f, N8-r: the one reader of `Membership`.
fn membership(s: &SetsDoc) -> Membership {
    let mut entries: BTreeMap<String, Vec<Lines>> = BTreeMap::new();
    for e in s.entries().iter() {
        entries.entry(fold_id(&e.id)).or_default().push(Lines {
            created: e.created,
            renamed_from: e.renamed_from.clone(),
            add: e.add.clone(),
            remove: e.remove.clone(),
            share: e.share.clone(),
        });
    }
    Membership {
        entries,
        no_set: s.no_set().into_iter().collect(),
    }
}

/// N8-f: the `[inputs]` aliases (a new input name moves a file to another automatic set at the next plan).
fn aliases(p: &ProjectDoc) -> BTreeMap<String, String> {
    p.read_as_all()
        .into_iter()
        .map(|(k, a)| (k, a.path))
        .collect()
}

/// N8-f, N8-r: the sets whose files the step's own edit moved: an entry whose membership lines changed (a move, a
/// replace, Fill from names, a hand-over, a rename, a share line, a new or deleted entry), and every set whose view
/// holds a file one of its `add` or `remove` lines names, or a file the step put in no set (the set the file leaves: a
/// Replace or Remove of an automatic member). The views keep the last plan's members, so for these sets they are not
/// what the documents say until the plan lands. (A file a share line names stays in its own set, Note HH1; a file the
/// step took out of no set makes the whole step abstain: `rejoins`.)
fn touched(m: &AppModel, b: &Before, s: &SetsDoc) -> BTreeSet<String> {
    let now = membership(s);
    let mut out = BTreeSet::new();
    let mut lines = BTreeSet::new();
    // An entry that is not there says what an entry with no membership lines says (a borrow or a shader pick that
    // creates the entry moves no file). N8-u: an id's entries are compared as a whole, in document order.
    fn rows<'a>(m: &'a Membership, id: &str, none: &'a [Lines]) -> &'a [Lines] {
        m.entries.get(id).map_or(none, Vec::as_slice)
    }
    let none = [Lines::default()];
    for id in b.members.entries.keys().chain(now.entries.keys()) {
        let (x, y) = (rows(&b.members, id, &none), rows(&now, id, &none));
        if x != y {
            out.insert(id.clone());
            for l in x.iter().chain(y) {
                lines.extend(l.add.iter().chain(&l.remove).cloned());
            }
        }
    }
    lines.extend(now.no_set.difference(&b.members.no_set).cloned());
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

/// N8-r: whether the step took a file out of no set. It rejoins a set the views cannot name (its automatic set, which
/// the engine finds by name and alias, or a Move's target), so, as for an alias, no set is known in the step.
fn rejoins(b: &Before, s: &SetsDoc) -> bool {
    !b.members.no_set.is_subset(&membership(s).no_set)
}

/// Slot 2 reads the first entry for a folded set id, even if that entry has no borrow.
fn normal_borrows(s: &SetsDoc) -> BTreeMap<String, String> {
    let mut seen = HashSet::new();
    s.entries()
        .iter()
        .filter_map(|e| {
            let id = fold_id(&e.id);
            if !seen.insert(id.clone()) {
                return None;
            }
            Some((id, fold_id(e.borrow.get(&2)?)))
        })
        .collect()
}

/// `pre` is the index of the project document as it stands before the step (N8-o).
pub(super) fn before(m: &AppModel, pre: &Tables) -> Option<Before> {
    if m.target != Target::Cs {
        return None;
    }
    let (p, s) = (m.project.as_ref()?, m.sets_doc.as_ref()?);
    let f = facts(m);
    let sub = f
        .own_n
        .values()
        .map(|k| (k.clone(), pre.subtype(k)))
        .collect();
    let borrows = normal_borrows(s);
    let targets = borrows.values().cloned().collect();
    // N8-f: the views' members are what the documents say only when the last landed full plan was asked with the
    // documents as they are outside `[files]`: the fast re-plan's request signature (`plan_sig`, set by every
    // re-plan) still equals the landed one's. A set edit or an alias since then (a plan out, or failed) differs.
    let fresh = m.landed_sig.is_some_and(|l| l == m.plan_sig.1);
    Some(Before {
        shared: shared_into_regular(&f, s, &borrows, &f.empty, &targets, &BTreeSet::new()).out,
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
fn already_data(f: &Facts, pre: Option<&Option<Value>>, tables: &Tables, k: &str) -> bool {
    let post = tables.subtype(k);
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

    /// N8-o: the table that `ProjectDoc::file_value` reads, through the index: the table spelled
    /// `key`, else the first (in `[files]` order) spelled as the full name in another case, else the first spelled as
    /// the extension-less stem, as `find_key` picks it.
    fn resolved_row(&self, key: &str) -> Option<&Row> {
        let (full, stem) = forms(key);
        let first = |f: &String| self.by_lower.get(f)?.first().copied();
        let exact = self
            .by_lower
            .get(&full)
            .and_then(|at| at.iter().copied().find(|i| self.rows.get(*i).0 == key));
        let at = exact
            .or_else(|| first(&full))
            .or_else(|| stem.as_ref().and_then(first))?;
        Some(self.rows.get(at))
    }

    pub(super) fn subtype(&self, key: &str) -> Option<Value> {
        self.resolved_row(key)?.2.clone()
    }

    pub(super) fn file_key(&self, key: &str) -> Option<&str> {
        Some(&self.resolved_row(key)?.0)
    }

    /// The spellings of the tables that hold an `alpha.policy` for file `key`, with it.
    pub(super) fn alpha(&self, key: &str) -> BTreeMap<String, Value> {
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

    /// Keep alpha and Share must both resolve the literal table to one landed input.
    fn single_input_table(
        &self,
        key: &str,
        inputs: &HashMap<String, InputMatch>,
    ) -> Result<(String, InputMatch), String> {
        let table = self.keep_table(key)?;
        match inputs.get(&table.to_lowercase()) {
            Some(kind @ (InputMatch::Full(_) | InputMatch::Stem)) => Ok((table, *kind)),
            _ => Err(format!(
                "The [files] key {table} in sdc.toml also matches another texture."
            )),
        }
    }

    /// Use the existing table Reset reads, including its stem fallback. Create a full key only for an unconfigured file.
    fn keep_table(&self, key: &str) -> Result<String, String> {
        let (full, stem) = forms(key);
        let Some(at) = self.by_lower.get(&full) else {
            let existing = stem
                .as_ref()
                .and_then(|s| self.by_lower.get(s))
                .and_then(|at| at.first());
            return Ok(existing.map_or_else(|| key.to_string(), |i| self.rows.get(*i).0.clone()));
        };
        if let [i] = at.as_slice() {
            return Ok(self.rows.get(*i).0.clone());
        }
        if at.iter().any(|i| self.rows.get(*i).0 == key) {
            return Ok(key.to_string());
        }
        Err(format!(
            "More than one [files] table in sdc.toml matches {key}. Use the file's exact spelling."
        ))
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
    /// N8-k, N8-q, N8-v: every table matching a written or cleared file that the step neither writes nor clears,
    /// which the attempt must leave byte for byte (another file's tables are not in it).
    others: BTreeSet<String>,
}

/// N8-k: `auto_data` as distinct strings, in list order: a repeated entry is one key.
fn distinct(list: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    list.into_iter()
        .filter(|k| seen.insert(k.clone()))
        .collect()
}

/// One run, from the documents as the step left them: `data` on each key shared now and not at `b`; every listed key
/// no regular set shares, still `data`, cleared (N-33); an entry whose own table no longer holds `data` dropped (N8-p).
fn changes(
    m: &AppModel,
    b: &Before,
    p: &ProjectDoc,
    s: &SetsDoc,
    touched: &BTreeSet<String>,
    tables: &Tables,
) -> Changes {
    let mut c = Changes::default();
    if p.read_only() || s.read_only() {
        return c;
    }
    // N8-f: views older than the documents (a set edit or an alias whose plan has not landed): no set is known, so
    // nothing is written, cleared or dropped.
    if !b.fresh {
        return c;
    }
    let data = Some(json!("data"));
    let raw = s.auto_data();
    // N8-p: the list holds only what is still true. Before any clear is decided, an entry whose exact table is absent
    // or holds no `data` is dropped; the drop is a list-only change in this step. An entry comes back only through the
    // step's own write.
    let true_now = |k: &String| tables.exact_alpha(k) == data;
    let stale = !raw.iter().all(true_now);
    let start: Vec<String> = distinct(raw.clone()).into_iter().filter(true_now).collect();
    // N8-f: an alias the step changed (it moves files between automatic sets at the next plan), N8-r: a file the step
    // took out of no set: no set is known, so no file value is written or cleared. N8-t: the drop above needs only the
    // exact table, not where a file goes, so it still lands.
    if aliases(p) != b.aliases || rejoins(b, s) {
        if stale {
            c.list = start;
            c.relist = c.list != raw;
        }
        return c;
    }
    // Review 4 Important 1: a set with no file before and after the step is one whose files are gone (a set the step
    // creates or renames has no view before it, so it is `unsure` instead).
    let f = facts(m);
    let inputs = std::cell::LazyCell::new(|| input_matches(m, &m.cube_face_keys()));
    let gone: BTreeSet<String> = f.empty.intersection(&b.empty).cloned().collect();
    let sh = shared_into_regular(&f, s, &normal_borrows(s), &gone, &b.targets, touched);
    let (now, unsure) = (&sh.out, &sh.unsure);
    // N8-a, N8-j, N8-o: alpha is read as Task 6 reads it, from every `[files]` table that matches the file, through the
    // step's one index of the document after the edit; each file asks two lookups.
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
        if free && !already_data(&f, b.sub.get(k), tables, k) {
            // N9-m: a safe stem table is for Keep alpha; Share owns only full-name tables.
            if let Ok((t, InputMatch::Full(Some(st)))) = tables.single_input_table(k, &inputs) {
                if tangent(st) {
                    c.write.push(t);
                }
            }
        }
    }
    let (locked, now_f, unsure_f) = (all_forms(&f.locked), all_forms(now), all_forms(unsure));
    for k in &start {
        // N8-e: only the `data` the share step wrote: the table spelled exactly as the entry (every entry left in
        // `start` holds it); a `data` in any other table is the user's.
        // N8-b: in a `blind` step a borrow may read any of them.
        // A listed full name can now be another input's stem. An orphaned table can still be cleared.
        if !names(k, &locked)
            && !names(k, &now_f)
            && !names(k, &unsure_f)
            && !sh.blind
            && match inputs.get(&k.to_lowercase()) {
                None => true,
                Some(InputMatch::Full(Some(st))) => tangent(*st) && tables.typed_normal(k),
                _ => false,
            }
        {
            c.clear.push(k.clone());
        }
    }
    if stale || !c.write.is_empty() || !c.clear.is_empty() {
        let cleared: HashSet<&String> = c.clear.iter().collect();
        let mut l: Vec<String> = start
            .iter()
            .filter(|k| !cleared.contains(k))
            .cloned()
            .collect();
        let mut kept: HashSet<String> = l.iter().cloned().collect();
        for k in &c.write {
            if kept.insert(k.clone()) {
                l.push(k.clone());
            }
        }
        c.relist = l != raw;
        c.list = l;
    }
    // N8-q, N8-v: the read-back protects every table that matches an intended target's file (its case, separator,
    // Unicode and stem twins) except the whole set the step means to write or clear: two listed spellings that are both
    // cleared are not each other's protected twins. Tables of other files are not read back: the exact setter and the
    // exact remover touch only their own key.
    let intended: HashSet<&String> = c.write.iter().chain(&c.clear).collect();
    for k in &intended {
        let other = tables.matching(k).into_iter().map(|(t, ..)| t);
        c.others
            .extend(other.filter(|t| !intended.contains(t)).cloned());
    }
    c
}

/// Listed tables that a user's edit could change to Data.
pub(super) fn listed_without_data(pre: &Tables, s: &SetsDoc) -> Vec<String> {
    let data = Some(json!("data"));
    s.auto_data()
        .into_iter()
        .filter(|e| pre.exact_alpha(e) != data)
        .collect()
}

/// A user's write claims only the exact table that gained Data. A same-value pick is handled by `data_pick_unlists`.
pub(super) fn unlist_user_data(
    without_data: &[String],
    post: &Tables,
    p: &mut ProjectDoc,
    s: &mut SetsDoc,
) {
    let data = Some(json!("data"));
    let mine: HashSet<&String> = without_data
        .iter()
        .filter(|e| post.exact_alpha(e) == data)
        .collect();
    if mine.is_empty() {
        return;
    }
    let mut l = s.auto_data();
    l.retain(|k| !mine.contains(&k));
    s.set_auto_data(&l);
    for k in mine {
        p.claim(k, "alpha.policy", "data");
    }
}

/// N8-d, N8-i: a value and its list entry are written together or not at all, and a failed attempt changes nothing.
/// The list goes first and is read back: one that cannot be written (`[project]` is not a table) leaves every value
/// alone. If a value then does not take (its `[files]` entry is not a table), both documents go back to the text they
/// had before the attempt: the list's membership as it was, no entry pruned, no file value changed. `fail` (tests
/// only) makes the file writes fail.
fn apply(c: &Changes, p: &mut ProjectDoc, s: &mut SetsDoc, fail: bool) {
    let (p0, s0) = (p.text(), s.text());
    let others = |p: &ProjectDoc| -> Vec<Option<String>> {
        c.others
            .iter()
            .map(|t| p.exact_file_entry_text(t))
            .collect()
    };
    let others0 = others(p);
    if c.relist {
        s.set_auto_data(&c.list);
        if s.auto_data() != c.list {
            return;
        }
    }
    let data = Some(json!("data"));
    // N8-s: the index chose each table, so the write takes its spelling as it is, with no lookup.
    for k in c.write.iter().filter(|_| !fail) {
        p.set_exact_file_value(k, "alpha.policy", &json!("data"));
    }
    // N8-k: each clear is exact and done once: the table spelled as the entry, and only while it holds `data`. One
    // that no longer does is left as it is; its entry is off the list either way.
    for k in &c.clear {
        p.clear_exact_file_value(k, "alpha.policy", &json!("data"));
    }
    let took = c.write.iter().all(|k| exact_alpha(p, k) == data)
        && c.clear.iter().all(|k| exact_alpha(p, k) != data)
        && others(p) == others0;
    if !took {
        p.restore(&p0);
        s.restore(&s0);
    }
}

impl AppModel {
    /// Note JS7: the user's Data pick stays theirs when the last regular borrower goes away.
    pub fn keep_alpha(&mut self, key: &str) -> Result<(), String> {
        self.keep_alpha_shown(key, None)
    }

    pub(crate) fn keep_alpha_shown(
        &mut self,
        key: &str,
        shown: Option<&str>,
    ) -> Result<(), String> {
        if let Some(why) = self.fix_off() {
            return Err(why);
        }
        let rows = problems(self);
        let current = rows.iter().find_map(|r| match (&r.at, &r.fix) {
            (At::Texture(k), Some(Fix::KeepAlpha { table, .. })) if k == key => Some(table),
            _ => None,
        });
        let changed = "The shared normal changed. Check Problems again.";
        let table = current.ok_or(changed)?.as_ref().map_err(Clone::clone)?;
        if shown.is_some_and(|shown| shown != table) {
            return Err(changed.into());
        }
        self.edit_docs("Keep alpha", None, true, |d, _| {
            d.set_exact_file_value(table, "alpha.policy", &json!("data"));
        });
        Ok(())
    }

    /// Note JS7 (N-10, N-33): the follow-on at the end of a user's step on cs, inside the step, from its starting
    /// point; `post` is the step's index of the project document after the edit (N8-o).
    pub(super) fn follow_shared(&mut self, b: &Before, post: &Tables) {
        let Some(c) = self
            .project
            .as_ref()
            .zip(self.sets_doc.as_ref())
            .map(|(p, s)| changes(self, b, p, s, &touched(self, b, s), post))
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

    /// Claim the listed table the picker writes, including when it already holds Data.
    pub(super) fn data_pick_unlists(&mut self, label: &str, key: &str) -> bool {
        let (Some(p), Some(s)) = (&self.project, &self.sets_doc) else {
            return false;
        };
        let mut list = s.auto_data();
        if list.is_empty() || p.read_only() || s.read_only() {
            return false;
        }
        let Some(table) = p.file_table_key(key).filter(|t| list.contains(t)) else {
            return false;
        };
        self.edit_docs(label, None, true, |p, s| {
            p.set_exact_file_value(&table, "alpha.policy", &json!("data"));
            if exact_alpha(p, &table) == Some(json!("data")) {
                list.retain(|e| e != &table);
                s.set_auto_data(&list);
                p.claim(&table, "alpha.policy", "data");
            }
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_alpha_reuses_the_single_unicode_folded_full_name() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("sdc.toml"), "[files.\"PBR/Ärz_N.PNG\"]\n").unwrap();
        let tables = Tables::of(&ProjectDoc::open(d.path()));
        assert_eq!(tables.keep_table("pbr/ärz_n.png").unwrap(), "PBR/Ärz_N.PNG");
    }

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

    /// N8-r: `membership` reads every field of the sets document that changes which files a set holds (the fields
    /// `sets::engine_inputs` makes the engine's set edits from), one row per field; a borrow line or a shader pick
    /// changes none.
    #[test]
    fn membership_reads_every_field_that_moves_a_file() {
        let read = |text: &str| {
            let d = tempfile::tempdir().unwrap();
            std::fs::write(d.path().join("sdc-project.toml"), text).unwrap();
            let s = SetsDoc::open(d.path());
            assert!(!s.read_only(), "{text}");
            membership(&s)
        };
        let start = read("[[sets]]\nid = \"a/b\"\n");
        let rows = [
            ("created", "[[sets]]\nid = \"a/b\"\ncreated = true\n"),
            (
                "renamed_from",
                "[[sets]]\nid = \"a/b\"\nrenamed_from = \"a/c\"\n",
            ),
            ("add", "[[sets]]\nid = \"a/b\"\nadd = [\"a/x.png\"]\n"),
            ("remove", "[[sets]]\nid = \"a/b\"\nremove = [\"a/x.png\"]\n"),
            (
                "share",
                "[[sets]]\nid = \"a/b\"\nshare = [\"a/x_ao.png\"]\n",
            ),
            (
                "[project] no_set",
                "[project]\nno_set = [\"a/x.png\"]\n\n[[sets]]\nid = \"a/b\"\n",
            ),
        ];
        for (field, text) in rows {
            assert_ne!(read(text), start, "{field} changes membership");
        }
        let controls = [
            (
                "borrow",
                "[[sets]]\nid = \"a/b\"\nborrow = { slot2 = \"c/d\" }\n",
            ),
            ("shader", "[[sets]]\nid = \"a/b\"\nshader = \"pbr\"\n"),
        ];
        for (field, text) in controls {
            assert_eq!(read(text), start, "{field} moves no file");
        }
        // N8-u: a change to the first of two entries under one id is not hidden by the second.
        let two = "[[sets]]\nid = \"a/b\"\n\n[[sets]]\nid = \"a/b\"\n";
        let first_moved = "[[sets]]\nid = \"a/b\"\nadd = [\"a/x.png\"]\n\n[[sets]]\nid = \"a/b\"\n";
        assert_ne!(read(first_moved), read(two), "the first of two entries");
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
