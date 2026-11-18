//! Note JS5 (N-17, N-19, N-27): a tangent-space normal the user did not decide is `pbr_normal` in a PBR or Advanced
//! Skin set, and the engine's own type elsewhere. Untouched = listed in `[project] auto_normal` with the word its
//! `[files]` sub-type still holds, `alpha_free`, in no locked slot, not a kept DDS.
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

/// The stored word of a sub-type (`pbr_normal`).
fn word(st: Subtype) -> String {
    json!(st).as_str().unwrap_or_default().to_string()
}

/// N-17 (review 2 Minor 2): the key has no `alpha.policy`, or only the `data` the share step wrote (a listed
/// `auto_data` key still at `data`). `untouched_normal` and the recorder's first `pbr_normal` read this one rule.
pub(super) fn alpha_free(p: &ProjectDoc, s: &SetsDoc, key: &str) -> bool {
    match p.file_value(key, "alpha.policy") {
        None => true,
        Some(v) => v == json!("data") && s.auto_data().iter().any(|k| k == key),
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
