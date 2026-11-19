//! Note JS5 (N-17, N-19, N-27): a tangent-space normal the user did not decide is `pbr_normal` in a PBR or Advanced
//! Skin set, and the engine's own type elsewhere. Untouched = listed in `[project] auto_normal` with the word its
//! `[files]` sub-type still holds, `alpha_free`, in no locked slot, not a kept DDS. Any other member storing a
//! tangent-space type is the user's: neither the recorder, a shader pick nor the diffuse's Switch changes it. A shader
//! pick resolves each key to one final value before it writes (`normal_plan`).
//! Two scopes (N-17, N-28): a shader pick moves the set's own Normal-slot member only; the diffuse's Switch and
//! Revert (`pbr_follow`) govern every free member that is inferred or untouched-listed. A key untouched in
//! `auto_normal` is decided by the normal rule alone, in every path: it is never in `auto_pbr` afterwards.
// shortcut: list keys are not renamed with `[files]` keys (`retarget_auto`, input renames), so a renamed file reads as
// the user's and its old entry stays until Tessalith next writes the list; rename list keys there if it matters.
use super::sets::{locked_items, views, SlotFill, SlotKind};
use super::AppModel;
use crate::project::ProjectDoc;
use crate::sets_doc::SetsDoc;
use engine::config::{Subtype, Target};
use engine::shader::Shader;
use serde_json::{json, Value};

/// A tangent-space normal type, the three the automatic typing moves between (`msn` is never one).
pub(crate) fn tangent(st: Subtype) -> bool {
    matches!(
        st,
        Subtype::Normal | Subtype::LandscapeNormal | Subtype::PbrNormal
    )
}

/// A shader pick's resolved writes (`AppModel::normal_plan`): each key has one final value.
#[derive(Default)]
pub(super) struct NormalPlan {
    /// `untouched_for_shader`'s entries that stand.
    pub retype: Vec<(String, Option<Value>, bool)>,
    /// The normal rule's writes: `Some` types and lists the key, `None` clears and unlists it.
    pub writes: Vec<(String, Option<Subtype>)>,
    /// Keys the pick takes off `auto_pbr`.
    pub off_auto_pbr: Vec<String>,
    /// Keys whose follow marker goes: the pick moves them away from `pbr_normal`.
    pub marker_off: Vec<String>,
}

/// The stored word of a sub-type (`pbr_normal`).
fn word(st: Subtype) -> String {
    json!(st).as_str().unwrap_or_default().to_string()
}

/// The shader the user chose for the set, as the sets document holds it now: the last plan's pick (`view.shader`)
/// lags a pick until the plan it starts lands. The one read of "which shader did the user choose".
pub(super) fn explicit_pick(v: &super::sets::SetView) -> Option<Shader> {
    v.shader_choice
}

/// The spellings among `keys` (`ProjectDoc::file_keys`) of the `[files]` tables that hold `field` for file `key`. The
/// engine applies every `[files]` key that matches the file (`path_key_matches`: any case, the full path or the
/// extension-less one), so this is the one reader of "does the file hold a value" (`alpha_free`, `shared_alpha`).
pub(super) fn holders(p: &ProjectDoc, keys: &[String], key: &str, field: &str) -> Vec<String> {
    keys.iter()
        .filter(|k| engine::overrides::path_key_matches(k, key))
        .filter(|k| p.file_value(k, field).is_some())
        .cloned()
        .collect()
}

/// N-17: the key has no `alpha.policy`, or only the `data` the share step wrote (a listed `auto_data` key still at
/// `data`). The engine applies every `[files]` table that matches the file, so the extension-less spelling counts
/// too: `alpha.policy` there is the user's. `untouched_normal`, the recorder and the Switch read this one rule.
pub(super) fn alpha_free(p: &ProjectDoc, s: &SetsDoc, key: &str) -> bool {
    // Runs per shader pick, diffuse edit and settled plan, never per frame.
    match holders(p, &p.file_keys(), key, "alpha.policy").as_slice() {
        [] => true,
        // N8-e (amends N6-j): the one holder is the table the share step wrote, spelled byte for byte as its list
        // entry, still holding `data`; a `data` in any other table (a case twin, the extension-less one) is the user's.
        [one] => {
            p.file_value(one, "alpha.policy") == Some(json!("data"))
                && s.auto_data().iter().any(|k| k == one)
        }
        _ => false,
    }
}

impl AppModel {
    /// The untouched normal's held type, or `None`.
    pub fn untouched_normal(&self, key: &str) -> Option<Subtype> {
        let (p, s) = (self.project.as_ref()?, self.sets_doc.as_ref()?);
        let listed = s.auto_normal().remove(key)?;
        let held = p.file_value(key, "subtype")?;
        if held.as_str() != Some(listed.as_str())
            || !alpha_free(p, s, key)
            || locked_items(self).contains_key(key)
            || self.item(key).is_none_or(|i| i.kept_dds())
        {
            return None;
        }
        serde_json::from_value::<Subtype>(held)
            .ok()
            .filter(|st| tangent(*st))
    }

    /// N-19: the type an untouched normal takes outside a PBR set: the engine's suggestion, `landscape_normal` under
    /// `textures/landscape/` (suggest.rs:90), else `normal`. `job.suggestions` ignore `[files]` (auto_record.rs:73-74).
    pub(crate) fn tangent_back(&self, key: &str) -> Subtype {
        let landscape = self
            .item(key)
            .and_then(|i| i.job.as_ref())
            .is_some_and(|j| {
                j.suggestions
                    .iter()
                    .any(|s| s.subtype == Subtype::LandscapeNormal)
            });
        if landscape {
            Subtype::LandscapeNormal
        } else {
            Subtype::Normal
        }
    }

    /// The type an untouched normal takes under `shader`, when it differs from what it holds.
    pub(crate) fn normal_for(&self, key: &str, shader: Shader) -> Option<Subtype> {
        let held = self.untouched_normal(key)?;
        let want = if shader.is_pbr_or_advanced_skin() {
            Subtype::PbrNormal
        } else {
            self.tangent_back(key)
        };
        (want != held).then_some(want)
    }

    /// Note JS6 (N-17, N-19, N-28): resolves a shader pick's writes for the set's tangent-space normals, once, before
    /// anything is written. `r5` is `untouched_for_shader`'s list (`key`, value or clear, the PBR step's own).
    /// - The untouched normal of the set's own Normal slot (K) takes the shader's type from this rule alone, in every
    ///   set: `pbr_normal` under a PBR-set shader, `tangent_back` under any other, cleared on Auto from a chosen PBR-set
    ///   shader. An untouched key lives in `auto_normal` only: it never stays in `auto_pbr`. R5's entry for K survives
    ///   only as a non-tangent value (Eye's `msn`); the key then keeps its listed word and this rule writes nothing.
    /// - A K holding a non-tangent type (R5's `msn`) that R5 takes back to a tangent value gets the final value from
    ///   this rule, listed; on Auto it is unlisted with R5's clear.
    /// - Any other member holding a tangent-space type is the user's (or recorded before Note JS, N-20): R5 changes
    ///   none of them but for the leave step on a key an older pick listed in `auto_pbr`.
    pub(super) fn normal_plan(
        &self,
        set: &str,
        shader: Option<Shader>,
        mut r5: Vec<(String, Option<Value>, bool)>,
    ) -> NormalPlan {
        let mut plan = NormalPlan::default();
        let (Some(view), Some(p), Some(s)) = (
            self.view_of(set),
            self.project.as_ref(),
            self.sets_doc.as_ref(),
        ) else {
            plan.retype = r5;
            return plan;
        };
        let chosen = explicit_pick(&view);
        let own_n = match view.slots.get(&SlotKind::Normal) {
            Some(SlotFill::Own { item }) => Some(item.clone()),
            _ => None,
        };
        let stored = |k: &str| {
            p.file_value(k, "subtype")
                .and_then(|v| serde_json::from_value::<Subtype>(v).ok())
        };
        let new_of = |v: &Option<Value>| {
            v.clone()
                .and_then(|v| serde_json::from_value::<Subtype>(v).ok())
        };
        let auto_pbr = s.auto_pbr();
        let untouched = own_n
            .clone()
            .and_then(|k| self.untouched_normal(&k).map(|st| (k, st)));
        let untouched_key = untouched.as_ref().map(|(k, _)| k.clone());
        let r5_of = |k: &str| {
            r5.iter()
                .find(|(rk, _, _)| rk == k)
                .map(|(_, v, _)| v.clone())
        };
        let mut drop_r5: Vec<String> = vec![];
        if let Some((k, held)) = untouched {
            plan.off_auto_pbr.push(k.clone());
            let keeps = r5_of(&k).is_some_and(|v| new_of(&v).is_some_and(|st| !tangent(st)));
            if keeps {
                if held == Subtype::PbrNormal {
                    plan.marker_off.push(k.clone());
                }
            } else {
                drop_r5.push(k.clone());
                let write = match shader {
                    Some(sh) => self.normal_for(&k, sh).map(Some),
                    None => (chosen.is_some_and(Shader::is_pbr_or_advanced_skin)
                        && held == Subtype::PbrNormal)
                        .then_some(None),
                };
                if let Some(w) = write {
                    if held == Subtype::PbrNormal && w != Some(Subtype::PbrNormal) {
                        plan.marker_off.push(k.clone());
                    }
                    plan.writes.push((k, w));
                }
            }
        } else if let Some(k) = own_n.filter(|k| stored(k).is_some_and(|st| !tangent(st))) {
            // R5's own `msn`: the way back is this rule's write, listed (Tessalith's own, N-27).
            let free = alpha_free(p, s, &k)
                && !locked_items(self).contains_key(&k)
                && self.item(&k).is_some_and(|i| !i.kept_dds());
            match (r5_of(&k), shader) {
                (Some(v), Some(sh)) if free && new_of(&v).is_some_and(tangent) => {
                    drop_r5.push(k.clone());
                    plan.off_auto_pbr.push(k.clone());
                    let want = if sh.is_pbr_or_advanced_skin() {
                        Subtype::PbrNormal
                    } else {
                        self.tangent_back(&k)
                    };
                    plan.writes.push((k, Some(want)));
                }
                (Some(None), None) if s.auto_normal().contains_key(&k) => {
                    // The clear is this rule's, once: R5's entry for the key goes.
                    drop_r5.push(k.clone());
                    plan.off_auto_pbr.push(k.clone());
                    plan.writes.push((k, None));
                }
                _ => {}
            }
        }
        r5.retain(|(k, new, step)| {
            let typed = untouched_key.as_ref() != Some(k)
                && stored(k).is_some_and(tangent)
                && view.members.contains(k);
            !drop_r5.contains(k) && (!typed || (new.is_none() && *step && auto_pbr.contains(k)))
        });
        plan.retype = r5;
        debug_assert!(
            plan.writes
                .iter()
                .all(|(k, _)| !plan.retype.iter().any(|(r, _, _)| r == k)),
            "a key has one final value per pick"
        );
        plan
    }

    /// Tests: the R5 entries `normal_plan` lets stand for a shader pick.
    #[cfg(feature = "test-hooks")]
    pub fn normal_plan_probe(
        &self,
        set: &str,
        shader: Option<Shader>,
        r5: Vec<(String, Option<Value>, bool)>,
    ) -> Vec<(String, Option<Value>, bool)> {
        self.normal_plan(set, shader, r5).retype
    }

    /// G1: each set's own Normal-slot member that its settled pick (not derived) moves into PBR.
    // shortcut: an automatic pick leaving the PBR family never reverts an automatic `pbr_normal` (a `pbr_normal` is PBR
    // evidence itself, shader.rs:665; spec limit 2); upgrade if a set is seen stuck that way.
    pub(crate) fn normals_to_follow(&self) -> Vec<(String, Subtype)> {
        if self.target != Target::Cs {
            return vec![];
        }
        views(self)
            .iter()
            .filter_map(|v| {
                let pick = v.shader.as_ref().filter(|p| p.bakes_any_tangent())?;
                let Some(SlotFill::Own { item }) = v.slots.get(&SlotKind::Normal) else {
                    return None;
                };
                let st = self.normal_for(item, pick.shader)?;
                (st == Subtype::PbrNormal).then(|| (item.clone(), st))
            })
            .collect()
    }
}

/// Lists every tangent-space sub-type in `writes` with its word (the recorder's step). N-27: entries whose `[files]`
/// value changed since are pruned on this, Tessalith's own, write.
pub(super) fn list_normals(
    p: &ProjectDoc,
    s: &mut SetsDoc,
    writes: &[(String, &'static str, Value)],
) {
    if s.read_only() {
        return;
    }
    let before = s.auto_normal();
    let mut l = before.clone();
    let mut added = false;
    for (k, f, v) in writes {
        let st = serde_json::from_value::<Subtype>(v.clone()).ok();
        if *f == "subtype" && st.is_some_and(tangent) {
            l.insert(k.clone(), word(st.unwrap_or(Subtype::Normal)));
            added = true;
        }
    }
    if added {
        l.retain(|k, w| {
            p.file_value(k, "subtype").as_ref().and_then(Value::as_str) == Some(w.as_str())
        });
    }
    if l != before {
        s.set_auto_normal(&l);
    }
}

/// A pick's, the Switch's or Revert's own write: `Some` lists the word, `None` (a cleared sub-type) takes the key off;
/// stale entries are pruned with it.
pub(super) fn relist(p: &ProjectDoc, s: &mut SetsDoc, changes: &[(String, Option<Subtype>)]) {
    if s.read_only() || changes.is_empty() {
        return;
    }
    let before = s.auto_normal();
    let mut l = before.clone();
    for (k, st) in changes {
        match st {
            Some(st) => {
                l.insert(k.clone(), word(*st));
            }
            None => {
                l.remove(k);
            }
        }
    }
    l.retain(|k, w| {
        p.file_value(k, "subtype").as_ref().and_then(Value::as_str) == Some(w.as_str())
    });
    if l != before {
        s.set_auto_normal(&l);
    }
}

/// N-27: the user picked the word the list holds, so the key is theirs now (the rare write to `sdc-project.toml`). A
/// different value needs no write: it already reads as the user's.
pub(super) fn unlist_same(s: &mut SetsDoc, key: &str, new: Option<Subtype>) {
    if s.read_only() {
        return;
    }
    let mut l = s.auto_normal();
    if new.is_some_and(|st| l.get(key).is_some_and(|w| *w == word(st))) {
        l.remove(key);
        s.set_auto_normal(&l);
    }
}
