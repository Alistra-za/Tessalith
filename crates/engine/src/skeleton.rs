//! Fast re-plan (R2 §1, §2): the global half of the last full plan (`Prepared`), kept per session and patched for
//! changed `[files]` keys. A patch either names the sets those keys hit or says why a full plan is needed.
use crate::api::{reencodes, Plan, Prepared};
use crate::config::Target;
use crate::discover::{InputFile, InputKind};
use crate::helper::HelperRole;
use crate::overrides::{project_rel, KeyForms, Scope, SetOverride};
use crate::pack::plan::{raw_role, set_is_advanced_skin};
use std::collections::{BTreeSet, HashMap};

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

/// The packed-output suffixes a `[files]` key can name (`RecipeKind::suffix`).
const PACKED_SUFFIXES: [&str; 4] = ["rmaos", "rfaos", "p", "m"];

#[derive(Clone)]
pub struct Skeleton {
    /// [`crate::api::BatchRequest::sig_without_files`] of the request it was planned with.
    req_sig: u64,
    prepared: Prepared,
    /// File index → index into `prepared.sets`; `u32::MAX` for a file in no set.
    set_of: Vec<u32>,
    /// Lower-cased key form (each member's full and stem form, and the packed-output keys of each set that packs)
    /// → the sets it names.
    key_sets: HashMap<String, Vec<u32>>,
    /// Sets whose `set_resolved` said `pbr` in the full plan.
    pbr_sets: BTreeSet<u32>,
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
        Skeleton {
            req_sig: prepared.req.sig_without_files(),
            prepared,
            set_of,
            key_sets,
            pbr_sets,
        }
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
    pub fn patch(
        &self,
        changed: &[(String, Option<toml::Table>)],
    ) -> Result<(Skeleton, BTreeSet<String>), NeedsFull> {
        let p = &self.prepared;
        let mut next = self.clone();
        let mut hit_sets: BTreeSet<u32> = BTreeSet::new();
        let mut hit_files: BTreeSet<usize> = BTreeSet::new();
        for (key, entry) in changed {
            let sets = self
                .key_sets
                .get(&key.to_lowercase())
                .ok_or(NeedsFull(HITS_NOTHING))?;
            let n = &mut next.prepared;
            n.file_scopes
                .set_entry(key, entry.as_ref())
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
            let value = entry
                .as_ref()
                .map(|t| serde_json::to_value(t).unwrap_or_default());
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
            let subtype = |ovs: &[SetOverride]| -> Vec<serde_json::Value> {
                ovs.iter()
                    .filter(|o| {
                        o.field == "subtype" && matches!(&o.scope, Scope::Path(k) if k == key)
                    })
                    .map(|o| o.value.clone())
                    .collect()
            };
            // DECISIONS P1: a PBR member's sub-type moves the GUI's output paths, which the request carries.
            if p.target == Target::Cs
                && sets.iter().any(|s| self.pbr_sets.contains(s))
                && subtype(&p.file_scopes.overrides) != subtype(&n.file_scopes.overrides)
            {
                return Err(NeedsFull(PBR_SUBTYPE));
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

        // What the edits change per hit file, under the old and the new layer.
        let mut flips: Vec<usize> = vec![];
        {
            let (old_ctx, new_ctx) = (p.ctx(), next.prepared.ctx());
            for &i in &hit_files {
                let f = &p.files[i];
                if f.kind == InputKind::Dds && reencodes(&old_ctx, f) != reencodes(&new_ctx, f) {
                    return Err(NeedsFull(REENCODE_CHANGE));
                }
                let hand = (f.kind == InputKind::Image)
                    .then(|| crate::policy::helper_requested(&new_ctx, f).role)
                    .flatten();
                if hand != p.hand[i] {
                    return Err(NeedsFull(HELPER_ROLE_CHANGE));
                }
                // As `prepare` builds `typed`.
                let typed = raw_role(f).is_some()
                    && match hand {
                        Some(r) => r == HelperRole::NotHelper,
                        None => crate::policy::subtype_requested(&new_ctx, f),
                    };
                if typed != p.typed.contains(&i) {
                    flips.push(i);
                }
            }
        }
        // `drop_typed_roles` for the flipped files: a typed raw map's role leaves its set's suffixes unless another
        // untyped raw member has it; an untyped one brings it back.
        let n = &mut next.prepared;
        for &i in &flips {
            if !n.typed.remove(&i) {
                n.typed.insert(i);
            }
        }
        for &i in &flips {
            let (Some(r), Some(set)) = (
                raw_role(&n.files[i]),
                n.sets.get_mut(self.set_of[i] as usize),
            ) else {
                continue;
            };
            let kept = set
                .members
                .iter()
                .any(|&j| !n.typed.contains(&j) && raw_role(&n.files[j]).is_some_and(|q| q == r));
            if kept {
                set.suffixes.insert(r.id());
            } else {
                set.suffixes.remove(&r.id());
            }
        }
        // Membership check: `x_wet` sits in set `x` only while `x` is Advanced skin (`join_skin_wet`, cs only).
        if p.target == Target::Cs {
            let (old_ctx, new_ctx) = (p.ctx(), next.prepared.ctx());
            for &s in &hit_sets {
                let (old, new) = (&p.sets[s as usize], &next.prepared.sets[s as usize]);
                let wet_key = format!("{}_{}", old.key, crate::group::SKIN_WET);
                let joins = old.suffixes.contains(crate::group::SKIN_WET)
                    || p.sets.iter().any(|t| t.key == wet_key);
                let Some(&first) = old.members.first() else {
                    continue;
                };
                let f = &p.files[first];
                if joins
                    && set_is_advanced_skin(&old_ctx, old, f)
                        != set_is_advanced_skin(&new_ctx, new, f)
                {
                    return Err(NeedsFull(MEMBERSHIP_CHANGE));
                }
            }
        }
        let keys = hit_sets
            .iter()
            .map(|&s| p.sets[s as usize].key.clone())
            .collect();
        Ok((next, keys))
    }
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
        let e = Engine::new().unwrap();
        let p = e.prepare(req, &rep, &CancelToken::default()).unwrap();
        let (plan, pbr) = e
            .plan_sets_pbr(
                &p,
                None,
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
            let si = s.set_of[i] as usize;
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
    /// `prepare` builds (`typed`, suffixes).
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
        // And back: the role returns to the set's suffixes.
        let (back, _) = typed.patch(&[("dir/q_height.png".into(), None)]).unwrap();
        assert_eq!(layer(&back.prepared), layer(&s.prepared));
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

    #[test]
    fn session_keeps_the_skeleton_of_its_last_full_plan() {
        let fx = fast_fixture();
        let session = crate::session::Session::new(encoders_sys::Bc1Model::Ideal).unwrap();
        assert!(session.skeleton().is_none());
        let rep = Reporter::new(Arc::new(NullSink), "t");
        session
            .plan(fx.req.clone(), &rep, &CancelToken::default())
            .unwrap();
        let s = session.skeleton().expect("a full plan stores its skeleton");
        let mut req = fx.req.clone();
        req.bc1_model = session.bc1_model();
        assert_eq!(s.req_sig, req.sig_without_files());
        assert!(!s.key_sets.is_empty());
    }
}
