//! Fast re-plan (R2 §1, §2): the global half of the last full plan (`Prepared`), kept per session and patched for
//! changed `[files]` keys. A patch either names the sets those keys hit or says why a full plan is needed.
use crate::api::{
    check_file_overrides, drop_typed_roles, explicit_shaders, hand_role, is_typed, join_members,
    predict_suffixes, reencodes, set_is_pbr, waiting_images, Plan, Prepared, BV2_REASON,
};
use crate::config::Target;
use crate::cube::plan::face_tag;
use crate::discover::{InputFile, InputKind};
use crate::group::{fold, TextureSet};
use crate::manifest::output_key;
use crate::overrides::{project_rel, KeyForms, Scope, SetOverride, SUBTYPE_FIELD};
use crate::pack::plan::raw_role;
use crate::policy::JobStatus;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

/// Why an edit cannot be patched in: the caller plans in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeedsFull(pub &'static str);

pub const NO_SKELETON: &str = "no skeleton";
pub const GLOBAL_CHANGE: &str = "global change";
pub const HITS_NOTHING: &str = "key hits no planned file";
pub const BAD_ENTRY: &str = "invalid [files] entry";
pub const REENCODE_CHANGE: &str = "reencode change";
pub const HELPER_ROLE_CHANGE: &str = "helper.role change";
pub const PBR_SUBTYPE: &str = "subtype on a PBR set member";
pub const MEMBERSHIP_CHANGE: &str = "membership change";
/// A file moved in or out of the set of images that write nothing while another file folds to its output: which of
/// them is a duplicate changes (Note HE7).
pub const DUPLICATE_CHANGE: &str = "duplicate change";
pub const PBR_CHANGE: &str = "set PBR status change";
pub const PATCHED: &str = "patched skeleton";
/// An Also export's derive step reads its PBR sets as a whole (`plan_derived`, `report_unread`).
pub const DERIVE: &str = "derive request";
/// The scope holds a set in a BV2 writer collision, or the partial writes an output another job writes.
pub const WRITER_CLASH: &str = "output writer clash";
/// A `packing.recipe` edit renames a packed output (and the GUI's output paths follow the suffix).
pub const RECIPE_CHANGE: &str = "packing.recipe change";
/// The partial plan was cancelled: some scope sets have no jobs.
pub const CANCELLED: &str = "partial plan cancelled";

const RECIPE_FIELD: &str = "packing.recipe";

/// The packed-output suffixes a `[files]` key can name (`RecipeKind::suffix`).
const PACKED_SUFFIXES: [&str; 4] = ["rmaos", "rfaos", "p", "m"];

#[derive(Clone)]
pub struct Skeleton {
    /// [`crate::api::BatchRequest::sig_without_files`] of the request it was planned with.
    pub(crate) req_sig: u64,
    pub(crate) prepared: Prepared,
    /// File index → index into `prepared.sets`; `u32::MAX` for a file in no set.
    pub(crate) set_of: Vec<u32>,
    /// Lower-cased key form (each member's full and stem form, and the packed-output keys of each set that packs)
    /// → the sets it names.
    pub(crate) key_sets: HashMap<String, Vec<u32>>,
    /// Sets whose `set_resolved` said `pbr` in the full plan.
    pub(crate) pbr_sets: BTreeSet<u32>,
    /// A clone `patch` returned: never patched again.
    pub(crate) patched: bool,
    /// The partner groups [`Skeleton::scope`] closes over (set keys), built once per full plan.
    pub(crate) partners: Arc<Vec<BTreeSet<String>>>,
    /// Output key of each `Ready` job of the full plan → its set (index into `prepared.sets`).
    pub(crate) writers: Arc<HashMap<String, u32>>,
    /// Keys of the sets in a BV2 writer collision of the full plan, winners and losers alike.
    pub(crate) clash_sets: Arc<BTreeSet<String>>,
    /// Two files fold to one output path: only then can which image writes nothing change a duplicate verdict.
    pub(crate) shared_output: bool,
}

impl Skeleton {
    /// The skeleton of a full plan: `prepared` as `plan` was planned from, and `pbr` its PBR set keys.
    pub(crate) fn new(prepared: Prepared, plan: &Plan, pbr: &BTreeSet<String>) -> Skeleton {
        let mut set_of = vec![u32::MAX; prepared.files.len()];
        let mut key_sets: HashMap<String, Vec<u32>> = HashMap::new();
        let mut add = |form: String, s: u32| {
            let v = key_sets.entry(form).or_default();
            if !v.contains(&s) {
                v.push(s);
            }
        };
        let by_key: HashMap<&str, u32> = prepared
            .sets
            .iter()
            .enumerate()
            .map(|(i, s)| (s.key.as_str(), i as u32))
            .collect();
        for (si, set) in prepared.sets.iter().enumerate() {
            for &m in &set.members {
                set_of[m] = si as u32;
                if let Some(f) = prepared.file_scopes.forms_of(&prepared.files[m]) {
                    add(f.full.clone(), si as u32);
                    if let Some(s) = &f.stem {
                        add(s.clone(), si as u32);
                    }
                }
            }
        }
        // A packed output's key (`dir/x_rmaos`) names no real file: every recipe suffix on its base names its set,
        // since a recipe edit can change which one is built.
        if let Some(root) = prepared.file_scopes.root.as_deref() {
            for j in &plan.jobs {
                let (Some(pk), Some(&si)) = (&j.pack, by_key.get(j.set_key.as_str())) else {
                    continue;
                };
                let virt = InputFile {
                    path: j.input.clone(),
                    rel: j.output.file_name().unwrap_or_default().into(),
                    game_path: String::new(),
                    output: j.output.clone(),
                    kind: InputKind::Image,
                    real_rel: None,
                };
                let stem = project_rel(root, &virt).and_then(|r| KeyForms::of(&r).stem);
                let tail = format!("_{}", pk.kind.suffix());
                if let Some(base) = stem.as_deref().and_then(|s| s.strip_suffix(&tail)) {
                    for sfx in PACKED_SUFFIXES {
                        add(format!("{base}_{sfx}"), si);
                    }
                }
            }
        }
        let pbr_sets = pbr
            .iter()
            .filter_map(|k| by_key.get(k.as_str()).copied())
            .collect();
        let root = &prepared.req.output_root;
        let writers: HashMap<String, u32> = plan
            .jobs
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Ready))
            .filter_map(|j| {
                Some((
                    output_key(root, &j.output),
                    *by_key.get(j.set_key.as_str())?,
                ))
            })
            .collect();
        let mut clash_sets = BTreeSet::new();
        for s in plan
            .skipped
            .iter()
            .filter(|s| s.reason.starts_with(BV2_REASON))
        {
            clash_sets.extend(s.set.clone());
            if let Some(&w) = writers.get(&output_key(root, &s.output)) {
                clash_sets.insert(prepared.sets[w as usize].key.clone());
            }
        }
        let mut outs = HashSet::new();
        let shared_output = !prepared.files.iter().all(|f| outs.insert(fold(&f.output)));
        Skeleton {
            shared_output,
            req_sig: prepared.req.sig_without_files(),
            partners: Arc::new(partner_groups(&prepared)),
            prepared,
            set_of,
            key_sets,
            pbr_sets,
            patched: false,
            writers: Arc::new(writers),
            clash_sets: Arc::new(clash_sets),
        }
    }

    /// Whether `plan` (a partial over `scope`) writes an output another job writes in the next full plan: twice
    /// itself, or one the last full plan wrote from a set outside the scope (BV2, which a partial does not run).
    pub(crate) fn writer_clash(&self, scope: &BTreeSet<String>, plan: &Plan) -> bool {
        let root = &self.prepared.req.output_root;
        let mut seen = BTreeSet::new();
        plan.jobs
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Ready))
            .any(|j| {
                let k = output_key(root, &j.output);
                let outside = self
                    .writers
                    .get(&k)
                    .is_some_and(|&w| !scope.contains(&self.prepared.sets[w as usize].key));
                outside || !seen.insert(k)
            })
    }

    /// [`Skeleton::patch`] on the session's skeleton, checked against the current request's signature first.
    pub fn patch_sig(
        skel: Option<&Skeleton>,
        req_sig: u64,
        changed: &[(String, Option<toml::Table>)],
    ) -> Result<(Skeleton, BTreeSet<String>), NeedsFull> {
        let s = skel.ok_or(NeedsFull(NO_SKELETON))?;
        if s.req_sig != req_sig {
            return Err(NeedsFull(GLOBAL_CHANGE));
        }
        s.patch(changed)
    }

    /// A clone with each `(key, entry)` set in its `[files]` layer (`None` removes the key), and the keys of the
    /// sets those keys hit. `NeedsFull` when an edit could change membership, outputs or duplicates (R2 §2).
    /// Checks, first hit wins: a patched skeleton; per key, a key that hits nothing, then a bad entry; per hit file, a
    /// `reencode` change, then a `helper.role` change; the membership joins; a sub-type edit on a PBR set's member;
    /// a hit set whose PBR status flips.
    pub fn patch(
        &self,
        changed: &[(String, Option<toml::Table>)],
    ) -> Result<(Skeleton, BTreeSet<String>), NeedsFull> {
        // The GUI sends every key changed since the full plan, so a patch always starts from a full plan's skeleton
        // (`key_sets` and `pbr_sets` are only right for that one).
        if self.patched {
            return Err(NeedsFull(PATCHED));
        }
        let p = &self.prepared;
        let mut next = self.clone();
        next.patched = true;
        let mut hit_sets: BTreeSet<u32> = BTreeSet::new();
        let mut hit_files: BTreeSet<usize> = BTreeSet::new();
        let mut subtype_sets: BTreeSet<u32> = BTreeSet::new();
        let mut recipe_changed = false;
        for (key, entry) in changed {
            let sets = self
                .key_sets
                .get(&key.to_lowercase())
                .ok_or(NeedsFull(HITS_NOTHING))?;
            // The entry as `ConfigFile::files` holds it: once, for the layer and both documents.
            let value = entry
                .as_ref()
                .map(serde_json::to_value)
                .transpose()
                .map_err(|_| NeedsFull(BAD_ENTRY))?;
            let n = &mut next.prepared;
            n.file_scopes
                .set_entry(key, value.as_ref())
                .map_err(|_| NeedsFull(BAD_ENTRY))?;
            if let Some(codes) = n.req.file_allow.get(key) {
                n.file_scopes
                    .allow
                    .entry(key.clone())
                    .or_default()
                    .extend(codes.iter().copied());
                n.file_scopes.rebuild_keys();
            }
            // Keep the documents in step with the layer, as a fresh `prepare` would read them.
            for doc in [
                n.project_config.as_mut(),
                n.req.project_config_inline.as_mut(),
            ]
            .into_iter()
            .flatten()
            {
                match &value {
                    Some(v) => doc.files.insert(key.clone(), v.clone()),
                    None => doc.files.remove(key),
                };
            }
            let values = |ovs: &[SetOverride], field: &str| -> Vec<serde_json::Value> {
                ovs.iter()
                    .filter(|o| o.field == field && matches!(&o.scope, Scope::Path(k) if k == key))
                    .map(|o| o.value.clone())
                    .collect()
            };
            let differs = |field| {
                values(&p.file_scopes.overrides, field) != values(&n.file_scopes.overrides, field)
            };
            recipe_changed |= differs(RECIPE_FIELD);
            if differs(SUBTYPE_FIELD) {
                subtype_sets.extend(sets.iter().copied());
            }
            hit_sets.extend(sets.iter().copied());
            for &s in sets {
                for &m in &p.sets[s as usize].members {
                    if p.file_scopes
                        .forms_of(&p.files[m])
                        .is_some_and(|f| f.matches(key))
                    {
                        hit_files.insert(m);
                    }
                }
            }
        }

        // A value the full plan's up-front checks refuse (a wrong type, say) is that plan's config error, not a partial.
        let edited: Vec<&SetOverride> = next
            .prepared
            .file_scopes
            .overrides
            .iter()
            .filter(|o| matches!(&o.scope, Scope::Path(k) if changed.iter().any(|(c, _)| c == k)))
            .collect();
        check_file_overrides(&next.prepared, &edited).map_err(|_| NeedsFull(BAD_ENTRY))?;
        // What the edits change per hit file, under the old and the new layer (`prepare`'s own rules).
        let mut typed = p.typed.clone();
        {
            let (old_ctx, new_ctx) = (p.ctx(), next.prepared.ctx());
            for &i in &hit_files {
                let f = &p.files[i];
                if f.kind == InputKind::Dds && reencodes(&old_ctx, f) != reencodes(&new_ctx, f) {
                    return Err(NeedsFull(REENCODE_CHANGE));
                }
                let hand = hand_role(&new_ctx, f);
                if hand != p.hand[i] {
                    return Err(NeedsFull(HELPER_ROLE_CHANGE));
                }
                if is_typed(&new_ctx, f, hand) {
                    typed.insert(i);
                } else {
                    typed.remove(&i);
                }
            }
            // Note HE7: which images write nothing (`prepare`'s `waiting_images`) decides the duplicates. Run it on
            // the old and the new state over every set, so a skin `_f` (no `typed` flip), or a neighbour's edit
            // that unmakes the skin set, is seen too; a file that moved in or out and shares an output with
            // another file changes that pair's verdict.
            if self.shared_output {
                let mut sets = p.sets.clone();
                for s in typed
                    .symmetric_difference(&p.typed)
                    .map(|&i| self.set_of[i])
                    .filter(|&s| s != u32::MAX)
                    .collect::<BTreeSet<u32>>()
                {
                    refresh_suffixes(&mut sets[s as usize], &p.files, &typed);
                }
                let old_waits =
                    waiting_images(&old_ctx, p.target, &p.sets, &p.files, &p.typed, &p.hand);
                let new_waits =
                    waiting_images(&new_ctx, p.target, &sets, &p.files, &typed, &p.hand);
                if p.files.iter().any(|f| {
                    old_waits.contains(&f.path) != new_waits.contains(&f.path)
                        && p.files
                            .iter()
                            .any(|g| g.path != f.path && fold(&g.output) == fold(&f.output))
                }) {
                    return Err(NeedsFull(DUPLICATE_CHANGE));
                }
            }
            // Membership check: rerun `prepare`'s joins on its grouping under the new layer; any set whose members
            // differ from the kept joins means a full plan. All sets, since a join reaches its partner by key.
            let mut joined = p.grouped.clone();
            drop_typed_roles(&mut joined, &p.files, &typed);
            let explicit = explicit_shaders(&next.prepared.req, &joined).0;
            join_members(
                &mut joined,
                &p.files,
                p.registry.alias_table(),
                &new_ctx,
                &p.hand,
                &explicit,
            );
            let same = |a: &TextureSet, b: &TextureSet| {
                a.key == b.key && a.members == b.members && a.shared_skin == b.shared_skin
            };
            if joined.len() != p.joined.len()
                || !joined.iter().zip(&p.joined).all(|(a, b)| same(a, b))
            {
                return Err(NeedsFull(MEMBERSHIP_CHANGE));
            }
        }
        // DECISIONS P1: a PBR member's sub-type moves the GUI's output paths, which the request carries.
        if p.target == Target::Cs && subtype_sets.iter().any(|s| self.pbr_sets.contains(s)) {
            return Err(NeedsFull(PBR_SUBTYPE));
        }
        // The suffixes of the sets whose `typed` members changed: every raw role back, then `prepare`'s drop.
        let n = &mut next.prepared;
        let flipped: BTreeSet<u32> = typed
            .symmetric_difference(&n.typed)
            .map(|&i| self.set_of[i])
            .filter(|&s| s != u32::MAX)
            .collect();
        n.typed = typed;
        for s in flipped {
            refresh_suffixes(&mut n.sets[s as usize], &n.files, &n.typed);
        }
        // A hit set whose PBR status flips moves output paths too (P1); decided as `set_resolved` decides it.
        for &s in &hit_sets {
            let mut set = n.sets[s as usize].clone();
            let (explicit, _) = explicit_shaders(&n.req, std::slice::from_ref(&set));
            let complex = explicit.get(&set.key) == Some(&crate::shader::Shader::ComplexMaterial);
            predict_suffixes(&mut set, n.target, complex, &n.files, &n.typed, &n.hand);
            if set_is_pbr(n.target, &set, &n.files).is_some_and(|b| b != self.pbr_sets.contains(&s))
            {
                return Err(NeedsFull(PBR_CHANGE));
            }
        }
        // Last, so the more specific reasons above win (a recipe that unmakes skin is a membership change).
        if recipe_changed {
            return Err(NeedsFull(RECIPE_CHANGE));
        }
        let keys = hit_sets
            .iter()
            .map(|&s| p.sets[s as usize].key.clone())
            .collect();
        Ok((next, keys))
    }

    /// `hit` closed under the partners of R2 §2, repeated until nothing new is added: the sets of one cube (and a
    /// set writing the merged cube's output, `plan_cubes`' clash check), a borrowed normal's owner and every borrower,
    /// and the shared-roughness sets. The `_wet` and helper joins are not partners: `patch` refuses a membership change.
    pub fn scope(&self, hit: &BTreeSet<String>) -> BTreeSet<String> {
        let mut scope = hit.clone();
        loop {
            let before = scope.len();
            for g in self.partners.iter() {
                if g.iter().any(|k| scope.contains(k)) {
                    scope.extend(g.iter().cloned());
                }
            }
            if scope.len() == before {
                return scope;
            }
        }
    }
}

/// A set whose `typed` members changed: every raw role back, then `prepare`'s drop.
fn refresh_suffixes(set: &mut TextureSet, files: &[InputFile], typed: &BTreeSet<usize>) {
    let roles: Vec<String> = set
        .members
        .iter()
        .filter_map(|&i| raw_role(&files[i]).map(|r| r.id()))
        .collect();
    set.suffixes.extend(roles);
    drop_typed_roles(std::slice::from_mut(set), files, typed);
}

/// [`Skeleton::scope`]'s partner groups (set keys; a set may sit in several).
fn partner_groups(p: &Prepared) -> Vec<BTreeSet<String>> {
    let root = &p.req.output_root;
    let mut groups: Vec<BTreeSet<String>> = vec![];
    // Cube: `plan_cubes` groups face jobs by (folded output folder, base minus the face tag). Any sub-type, since
    // an edit can make or unmake a face; an untagged output named like the base clashes with the merged cube.
    let mut cube: HashMap<(String, String), (bool, BTreeSet<String>)> = HashMap::new();
    for set in &p.sets {
        for &m in &set.members {
            let out = &p.files[m].output;
            let Some(stem) = out.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let (base, tagged) = match face_tag(stem) {
                Some((b, _)) => (b, true),
                None => (stem, false),
            };
            let dir = fold(out.parent().unwrap_or(std::path::Path::new("")));
            let g = cube.entry((dir, base.to_lowercase())).or_default();
            g.0 |= tagged;
            g.1.insert(set.key.clone());
        }
    }
    groups.extend(
        cube.into_values()
            .filter(|(tagged, keys)| *tagged && keys.len() > 1)
            .map(|(_, keys)| keys),
    );
    // A borrowed normal: the owner and all its borrowers (`plan_bakes`' `lent` reads every borrower).
    let mut owners: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (b, o) in &p.req.borrowed_normal {
        let o = crate::sets::set_key(root, o);
        owners
            .entry(o.clone())
            .or_insert_with(|| BTreeSet::from([o]))
            .insert(crate::sets::set_key(root, b));
    }
    groups.extend(owners.into_values());
    let shared: BTreeSet<String> = p
        .req
        .shared_roughness
        .iter()
        .map(|id| crate::sets::set_key(root, id))
        .collect();
    if !shared.is_empty() {
        groups.push(shared);
    }
    groups
}

#[cfg(test)]
#[path = "../tests/common/fast_fixture.rs"]
mod fast_fixture;

#[cfg(test)]
mod tests {
    use super::fast_fixture::{fast_fixture, with_entry};
    use super::*;
    use crate::api::{BatchRequest, CancelToken, Engine};
    use crate::events::{NullSink, Reporter};
    use crate::session::AnalysisCache;
    use std::sync::Arc;

    fn prepare(req: &BatchRequest) -> Prepared {
        let rep = Reporter::new(Arc::new(NullSink), "t");
        Engine::new()
            .unwrap()
            .prepare(req, &rep, &CancelToken::default())
            .unwrap()
    }

    fn skeleton(req: &BatchRequest) -> Skeleton {
        let rep = Reporter::new(Arc::new(NullSink), "t");
        let (plan, p, pbr) = Engine::new()
            .unwrap()
            .plan_full(
                req,
                &rep,
                &CancelToken::default(),
                &AnalysisCache::default(),
            )
            .unwrap();
        Skeleton::new(p, &plan, &pbr)
    }

    fn table(s: &str) -> toml::Table {
        toml::from_str(s).unwrap()
    }

    /// The set ids (`dir/x`) `key` names.
    fn ids(s: &Skeleton, key: &str) -> Vec<String> {
        let root = &s.prepared.req.output_root;
        let mut v: Vec<String> = s
            .key_sets
            .get(&key.to_lowercase())
            .into_iter()
            .flatten()
            .map(|&i| crate::sets::set_id(root, &s.prepared.sets[i as usize].key))
            .collect();
        v.sort();
        v
    }

    fn set_ids(s: &Skeleton, keys: &BTreeSet<String>) -> Vec<String> {
        keys.iter()
            .map(|k| crate::sets::set_id(&s.prepared.req.output_root, k))
            .collect()
    }

    /// The `[files]` layer and grouping a patch must leave as a fresh `prepare` of the edited request does.
    fn layer(p: &Prepared) -> String {
        format!(
            "{:?}",
            (
                &p.file_scopes.overrides,
                &p.file_scopes.allow,
                &p.sets,
                &p.typed,
                &p.hand,
                &p.project_config,
                &p.req.project_config_inline,
            )
        )
    }

    #[test]
    fn key_sets_cover_forms() {
        let fx = fast_fixture();
        let s = skeleton(&fx.req);
        for (key, want) in [
            ("dir/a_n.png", "dir/a"),
            ("DIR/A_N", "dir/a"),
            ("dir/e_p", "dir/e"),
            ("dir/e_rmaos", "dir/e"),
            ("dir/h_rmaos", "dir/h"),
            ("dir/b_mask", "dir/b"),
            ("dir/b_mask.png", "dir/b"),
            ("dir/k.png", "dir/j"),
            ("dir/h_wet", "dir/h"),
            ("dir/f", "dir/f"),
            ("dir/f.dds", "dir/f"),
            ("dir/q_p", "dir/q"),
        ] {
            assert_eq!(ids(&s, key), vec![want.to_string()], "{key}");
        }
        assert!(ids(&s, "dir/zzz.png").is_empty());
        // `g.png` and `g.tga` share one stem and one set.
        assert_eq!(ids(&s, "dir/g"), vec!["dir/g".to_string()]);
        let a = s
            .prepared
            .sets
            .iter()
            .position(|t| t.key.ends_with("/dir/a"))
            .unwrap() as u32;
        assert!(s.pbr_sets.contains(&a), "set a is PBR");
        let q = s
            .prepared
            .sets
            .iter()
            .position(|t| t.key.ends_with("/dir/q"))
            .unwrap() as u32;
        assert!(!s.pbr_sets.contains(&q), "set q is not PBR");
        for (i, f) in s.prepared.files.iter().enumerate() {
            // A generated loser is in no set.
            let Some(si) = Some(s.set_of[i]).filter(|&x| x != u32::MAX) else {
                continue;
            };
            let si = si as usize;
            assert!(
                s.prepared.sets[si].members.contains(&i),
                "{}",
                f.path.display()
            );
        }
    }

    #[test]
    fn patch_scopes_alpha_edit() {
        let fx = fast_fixture();
        let s = skeleton(&fx.req);
        let t = table("alpha.policy = \"cutout\"");
        let (next, hit) = s.patch(&[("dir/a.png".into(), Some(t.clone()))]).unwrap();
        assert_eq!(set_ids(&s, &hit), vec!["dir/a".to_string()]);
        assert_eq!(
            layer(&next.prepared),
            layer(&prepare(&with_entry(&fx.req, "dir/a.png", Some(&t))))
        );
        assert_eq!(next.req_sig, s.req_sig);
        // The index stays exact after the patch.
        assert!(next.prepared.file_scopes.keys.is_some());
    }

    /// Removing a key, and a raw map's sub-type flipping it to an ordinary file and back, patch to what a fresh
    /// `prepare` builds (`typed`, suffixes). Each step patches a skeleton of a full plan, never a patched one.
    #[test]
    fn patch_matches_fresh_prepare() {
        let fx = fast_fixture();
        let s = skeleton(&fx.req);
        let (next, hit) = s.patch(&[("dir/c_px".into(), None)]).unwrap();
        assert_eq!(set_ids(&s, &hit), vec!["dir/c_px".to_string()]);
        let removed = with_entry(&fx.req, "dir/c_px", None);
        assert_eq!(layer(&next.prepared), layer(&prepare(&removed)));

        let t = table("subtype = \"diffuse\"");
        let (typed, hit) = s
            .patch(&[("dir/q_height.png".into(), Some(t.clone()))])
            .unwrap();
        assert_eq!(set_ids(&s, &hit), vec!["dir/q".to_string()]);
        let edited = with_entry(&fx.req, "dir/q_height.png", Some(&t));
        let fresh = prepare(&edited);
        assert_ne!(fresh.typed, s.prepared.typed, "the edit types q_height");
        assert_eq!(layer(&typed.prepared), layer(&fresh));
        // And back, from the edited project's own full plan: the role returns to the set's suffixes.
        let (back, _) = skeleton(&edited)
            .patch(&[("dir/q_height.png".into(), None)])
            .unwrap();
        assert_eq!(layer(&back.prepared), layer(&s.prepared));
    }

    /// Minor 7: a file name with upper-case letters is hit by its lower-cased and its exact-case key.
    #[test]
    fn mixed_case_names_are_hit_by_any_case() {
        let fx = fast_fixture();
        let s = skeleton(&fx.req);
        for key in [
            "dir/MixedCase.png",
            "dir/mixedcase.png",
            "dir/MixedCase",
            "DIR/MIXEDCASE",
        ] {
            assert_eq!(ids(&s, key), vec!["dir/mixedcase".to_string()], "{key}");
        }
        let t = table("alpha.policy = \"cutout\"");
        let (next, hit) = s
            .patch(&[("dir/MixedCase.png".into(), Some(t.clone()))])
            .unwrap();
        assert_eq!(set_ids(&s, &hit), vec!["dir/mixedcase".to_string()]);
        assert_eq!(
            layer(&next.prepared),
            layer(&prepare(&with_entry(
                &fx.req,
                "dir/MixedCase.png",
                Some(&t)
            )))
        );
    }

    /// Minor 8, Note DT31: all six cube faces in one call scope the six face sets.
    #[test]
    fn one_patch_of_six_cube_faces_scopes_six_sets() {
        let fx = fast_fixture();
        let s = skeleton(&fx.req);
        let t = table("subtype = \"cubemap\"\nalpha.policy = \"cutout\"");
        let faces = ["px", "nx", "py", "ny", "pz", "nz"];
        let changed: Vec<(String, Option<toml::Table>)> = faces
            .iter()
            .map(|f| (format!("dir/c_{f}"), Some(t.clone())))
            .collect();
        let (next, hit) = s.patch(&changed).unwrap();
        let mut want: Vec<String> = faces.iter().map(|f| format!("dir/c_{f}")).collect();
        want.sort();
        assert_eq!(set_ids(&s, &hit), want);
        let mut edited = fx.req.clone();
        for (k, e) in &changed {
            edited = with_entry(&edited, k, e.as_ref());
        }
        assert_eq!(layer(&next.prepared), layer(&prepare(&edited)));
    }

    #[test]
    fn patch_needs_full_for() {
        let fx = fast_fixture();
        let s = skeleton(&fx.req);
        let one = |key: &str, entry: Option<&str>| {
            s.patch(&[(key.to_string(), entry.map(table))])
                .err()
                .map(|e| e.0)
        };
        assert_eq!(
            one("dir/f.dds", Some("reencode = true")),
            Some(REENCODE_CHANGE)
        );
        assert_eq!(one("dir/b_mask", None), Some(HELPER_ROLE_CHANGE));
        assert_eq!(
            one("dir/b.png", Some("helper.role = \"opacity\"")),
            Some(HELPER_ROLE_CHANGE)
        );
        // Without the recipe `h` is no longer Advanced skin, so `h_wet` would leave its set.
        assert_eq!(one("dir/h_rmaos", None), Some(MEMBERSHIP_CHANGE));
        // Typing both raw maps leaves `h` nothing to pack `_rfaos` from: no longer skin, `h_wet` leaves.
        let diffuse = table("subtype = \"diffuse\"");
        assert_eq!(
            s.patch(&[
                ("dir/h_roughness.png".into(), Some(diffuse.clone())),
                ("dir/h_ao.png".into(), Some(diffuse.clone())),
            ])
            .err(),
            Some(NeedsFull(MEMBERSHIP_CHANGE))
        );
        // Untyped, `q_rough` makes `q` pack `_rmaos`: the set turns PBR, which moves output paths (Note P1).
        assert_eq!(one("dir/q_rough", None), Some(PBR_CHANGE));
        // A patched skeleton is never patched again.
        let (patched, _) = s
            .patch(&[("dir/a.png".into(), Some(table("alpha.policy = \"cutout\"")))])
            .unwrap();
        assert_eq!(
            patched
                .patch(&[("dir/b.png".into(), Some(table("alpha.policy = \"cutout\"")))])
                .err(),
            Some(NeedsFull(PATCHED))
        );
        assert_eq!(
            one("dir/a.png", Some("subtype = \"diffuse\"")),
            Some(PBR_SUBTYPE)
        );
        assert_eq!(
            one("dir/zzz.png", Some("alpha.policy = \"cutout\"")),
            Some(HITS_NOTHING)
        );
        assert_eq!(one("dir/a.png", Some("no_such = 1")), Some(BAD_ENTRY));
        let sig = fx.req.sig_without_files();
        assert_eq!(
            Skeleton::patch_sig(None, sig, &[]).err(),
            Some(NeedsFull(NO_SKELETON))
        );
        let mut global = fx.req.clone();
        global.target = Some(Target::Vanilla);
        assert_eq!(
            Skeleton::patch_sig(Some(&s), global.sig_without_files(), &[]).err(),
            Some(NeedsFull(GLOBAL_CHANGE))
        );
        assert!(Skeleton::patch_sig(Some(&s), sig, &[]).is_ok());
    }

    /// Note HG2: a shader choice that makes a colourless set shared changes no member, only the flag, and still plans
    /// in full. A request's shader choice is part of its signature, so this edits the kept request as a patch would
    /// see it after a shader change.
    #[test]
    fn a_shader_edit_that_makes_a_shared_set_needs_a_full_plan() {
        let fx = fast_fixture();
        for f in ["w_normal", "w_sk"] {
            image::RgbImage::new(16, 16)
                .save(fx.dir.path().join(format!("in/dir/{f}.png")))
                .unwrap();
        }
        let s = skeleton(&fx.req);
        let mut shaded = s.clone();
        shaded
            .prepared
            .req
            .set_shaders
            .insert("dir/w".into(), crate::shader::Shader::AdvancedSkin);
        assert_eq!(
            shaded
                .patch(&[(
                    "dir/w_sk.png".into(),
                    Some(table("alpha.policy = \"cutout\""))
                )])
                .err(),
            Some(NeedsFull(MEMBERSHIP_CHANGE))
        );
        // Without the choice the same edit patches.
        assert!(s
            .patch(&[(
                "dir/w_sk.png".into(),
                Some(table("alpha.policy = \"cutout\""))
            )])
            .is_ok());
    }

    #[test]
    fn sig_ignores_only_the_files_table() {
        let fx = fast_fixture();
        let sig = fx.req.sig_without_files();
        let edited = with_entry(
            &fx.req,
            "dir/a.png",
            Some(&table("alpha.policy = \"cutout\"")),
        );
        assert_eq!(edited.sig_without_files(), sig);
        let mut r = fx.req.clone();
        r.plan_progress = Some(Default::default());
        assert_eq!(r.sig_without_files(), sig, "live counters are not an input");
        let mut r = fx.req.clone();
        r.set_edits.clear();
        assert_ne!(r.sig_without_files(), sig);
        let mut r = fx.req.clone();
        r.project_config_inline.as_mut().unwrap().preset = Some("cs-quality".into());
        assert_ne!(r.sig_without_files(), sig);
    }

    /// Minor 4: equal requests built in different insertion orders sign equally.
    #[test]
    fn sig_ignores_insertion_order() {
        let fx = fast_fixture();
        let build = |order: &[usize]| {
            let mut r = fx.req.clone();
            let entries = [
                ("dir/a.png", crate::codes::Code::InDuplicate),
                ("dir/b.png", crate::codes::Code::InDdsSkipped),
                ("dir/c_px", crate::codes::Code::InDuplicate),
            ];
            for &i in order {
                let (k, c) = entries[i];
                r.file_allow.entry(k.into()).or_default().insert(c);
                r.pinned_files.insert(k.into());
                r.set_shaders
                    .insert(format!("set{i}"), crate::shader::Shader::Pbr);
            }
            let text = if order[0] == 0 {
                "[defaults]\nmips.filter = \"kaiser\"\nalpha.policy = \"cutout\"\n"
            } else {
                "[defaults]\nalpha.policy = \"cutout\"\nmips.filter = \"kaiser\"\n"
            };
            r.project_config_inline = Some(crate::presets::ConfigFile::parse(text, "t").unwrap());
            r
        };
        assert_eq!(
            build(&[0, 1, 2]).sig_without_files(),
            build(&[2, 1, 0]).sig_without_files()
        );
    }

    #[test]
    fn session_keeps_the_skeleton_of_its_last_full_plan() {
        let fx = fast_fixture();
        let session = crate::session::Session::new().unwrap();
        assert!(session.skeleton().is_none());
        let rep = Reporter::new(Arc::new(NullSink), "t");
        session
            .plan_and_keep(fx.req.clone(), &rep, &CancelToken::default())
            .unwrap();
        let s = session.skeleton().expect("a full plan stores its skeleton");
        assert_eq!(s.req_sig, fx.req.sig_without_files());
        assert!(!s.key_sets.is_empty());
    }

    /// Note FL3: a partial whose plan writes one output twice, or writes one a set outside its scope wrote in the
    /// last full plan, needs the full plan. No `[files]` edit the patch lets through moves an output (a cube face's
    /// edit pulls in the set its merged output meets), so the plan is edited by hand here.
    #[test]
    fn writer_clash_catches_a_new_collision() {
        let fx = fast_fixture();
        let session = crate::session::Session::new().unwrap();
        let rep = Reporter::new(Arc::new(NullSink), "t");
        let full = session
            .plan_and_keep(fx.req.clone(), &rep, &CancelToken::default())
            .unwrap();
        let s = session.skeleton().unwrap();
        let ready = |p: &Plan| {
            p.jobs
                .iter()
                .filter(|j| matches!(j.status, JobStatus::Ready) && !j.set_key.is_empty())
                .cloned()
                .collect::<Vec<_>>()
        };
        let jobs = ready(&full);
        let (a, b) = (
            &jobs[0],
            jobs.iter().find(|j| j.set_key != jobs[0].set_key).unwrap(),
        );
        let scope = |j: &crate::policy::ResolvedJob| BTreeSet::from([j.set_key.clone()]);
        let mut plan = full;
        plan.jobs = vec![a.clone()];
        assert!(!s.writer_clash(&scope(a), &plan), "its own output");
        plan.jobs = vec![a.clone(), a.clone()];
        assert!(s.writer_clash(&scope(a), &plan), "twice itself");
        let mut moved = a.clone();
        moved.output = b.output.clone();
        plan.jobs = vec![moved];
        assert!(
            s.writer_clash(&scope(a), &plan),
            "b's output, b out of scope"
        );
    }
}
