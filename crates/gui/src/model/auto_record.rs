//! Note FC1 (amends U3/U5): what the settled plan decided on its own is written into `sdc.toml`, so Save keeps it and
//! `sdc convert` plans the same. An accepted High suggestion (mesh-vote and GRAS sub-types are High suggestions too)
//! writes `subtype`; a mesh-alpha verdict (`remove` too, Note HR2) writes `alpha.policy` and its threshold. A written
//! value is a plain project value, as if the user had accepted it: later evidence never rewrites it.
//!
//! R5 (amends R1): a user action that re-types (a set-shader pick, PBR follow, a preset alias change) treats an
//! untouched recorded sub-type, one equal to what auto-accept picks from the plan, as not set, as before FC1. Scan
//! evidence still never rewrites one.
use super::items::{ItemKind, ItemStatus};
use super::sets::{SlotFill, SlotKind};
use super::{fields, AppModel};
use engine::config::{PackRecipe, Subtype, Target};
use engine::group::{is_fuzz_stem, split_suffix_with};
use engine::policy::{AcceptSuggestions, Provenance};
use engine::shader::{member_subtype, Shader, ShaderOrigin};
use engine::suggest::Confidence;
use serde_json::json;
use std::path::Path;

/// The last recording, for its Problems row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AutoNote {
    /// The row's text, `{n} types set automatically (High confidence)`; its count is the step label's.
    pub text: String,
    /// `file name: value`, one per written value (a threshold rides on its policy's line), for the row's hover; the
    /// last line explains High confidence when a sub-type was set.
    pub lines: Vec<String>,
    /// The `history.pushes` the step left: the row offers Undo only while it is the top step.
    pub step: u64,
    /// Note FI4: nothing prompted to be saved just before it, so while it is the top step nothing does.
    pub clean_before: bool,
}

/// A written value as the row lists it: `cutout`, a number.
fn show(v: &serde_json::Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_string)
}

/// A written sub-type as the type picker shows it (`landscape diffuse`), not its id.
fn show_type(v: &serde_json::Value) -> String {
    serde_json::from_value::<Subtype>(v.clone())
        .map_or_else(|_| show(v), super::labels::subtype_name)
}

/// The file's name, not its folder or game path (the hover lists files; the folder is in the Textures tree).
fn file_name(key: &str) -> &str {
    key.rsplit(['/', '\\']).next().unwrap_or(key)
}

/// Ends the row's hover when a sub-type was set (Note FI11, A17).
const HIGH_CONFIDENCE: &str = "High confidence: the file name and the pixels agree on the type.";

impl AppModel {
    /// What auto-accept picks for `key` from the plan: its top name-based suggestion when High (a `[files]` sub-type
    /// leaves `job.suggestions` as they would be without it), re-typed by `shader` as the engine does (S-shader-4).
    pub(crate) fn auto_pick(&self, key: &str, shader: Option<Shader>) -> Option<Subtype> {
        if self.accept != AcceptSuggestions::High {
            return None;
        }
        let job = self.item(key)?.job.as_ref()?;
        let s = job
            .suggestions
            .first()
            .filter(|s| s.confidence == Confidence::High)?;
        Some(
            shader
                .and_then(|sh| member_subtype(sh, s.subtype))
                .map_or(s.subtype, |(st, _)| st),
        )
    }

    /// R5: `key`'s `[files]` sub-type is untouched: it is what auto-accept picks from the plan, with or without the
    /// set's current `shader`, so a re-typing user action may replace it; so is a `skin_albedo` recorded only because
    /// of a fuzz map (Fuzz-5, `fuzz_recorded_skin`), in every accept mode. A different value is the user's.
    pub(crate) fn untouched_subtype(&self, key: &str, shader: Option<Shader>) -> bool {
        let Some(v) = self
            .project
            .as_ref()
            .and_then(|p| p.file_value(key, "subtype"))
            .and_then(|v| serde_json::from_value::<Subtype>(v).ok())
        else {
            return false;
        };
        self.auto_pick(key, None) == Some(v)
            || (shader.is_some() && self.auto_pick(key, shader) == Some(v))
            || self.fuzz_recorded_skin(key)
    }

    /// Fuzz-5: `key`'s `[files]` `skin_albedo` is one an older Tessalith recorded only because its set holds a fuzz
    /// map, so it is untouched: on cs, in the set whose own Diffuse slot holds `key` (Fuzz-30: a borrower's view never
    /// decides the lender's file), with an automatic shader, an untyped fuzz map with no hand role, `key` neither
    /// made skin by a pick (`auto_skin`) nor locked, no recipe written on the packed output, and no other skin
    /// evidence (a skin-typed member, a real `_rfaos` file, a shared skin set). The cheap `[files]` check runs first,
    /// as `presets_saved` asks this of every item.
    pub fn fuzz_recorded_skin(&self, key: &str) -> bool {
        let Some(p) = self.project.as_ref() else {
            return false;
        };
        let typed = |k: &str| {
            p.file_value(k, "subtype")
                .and_then(|v| serde_json::from_value::<Subtype>(v).ok())
        };
        if typed(key) != Some(Subtype::SkinAlbedo) || self.target != Target::Cs {
            return false;
        }
        let views = super::sets::views(self);
        let Some(v) = views.iter().find(|v| {
            matches!(v.slots.get(&SlotKind::Diffuse), Some(SlotFill::Own { item }) if item == key)
        }) else {
            return false;
        };
        if v.shared
            || v.shader
                .as_ref()
                .is_some_and(|s| s.provenance == ShaderOrigin::Set)
        {
            return false;
        }
        if self
            .sets_doc
            .as_ref()
            .is_some_and(|s| s.auto_skin().contains_key(key))
            || super::sets::locked_items(self).contains_key(key)
        {
            return false;
        }
        // The recipe of the set's packed output (`explicit_recipe` reads both the `_rmaos` and `_rfaos` keys).
        if let Some(SlotFill::Own { item }) = v.slots.get(&SlotKind::Rmaos) {
            if self.explicit_recipe(item) != PackRecipe::Auto {
                return false;
            }
        }
        let stem = |k: &str| {
            Path::new(k)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        let mut fuzz = false;
        for k in &v.members {
            // Packed outputs are not files: the set's own `_rfaos` output is no evidence.
            if self.item(k).is_none_or(|i| i.kind == ItemKind::Packed) {
                continue;
            }
            let s = stem(k);
            if is_fuzz_stem(&s)
                && typed(k).is_none()
                && p.file_value(k, engine::helper::ROLE_FIELD).is_none()
            {
                fuzz = true;
            }
            let skin_typed = matches!(
                typed(k),
                Some(
                    Subtype::SkinAlbedo
                        | Subtype::SkinRfaos
                        | Subtype::SkinSk
                        | Subtype::SkinWet
                        | Subtype::SkinDetail
                )
            );
            let rfaos = split_suffix_with(&s, &self.suffix_aliases)
                .suffix
                .as_deref()
                == Some("rfaos");
            if (k != key && skin_typed) || rfaos {
                return false;
            }
        }
        fuzz
    }

    /// Each set member's current shader (chosen or automatic).
    fn shaders_by_member(&self) -> std::collections::HashMap<String, Shader> {
        let mut out = std::collections::HashMap::new();
        for v in super::sets::views(self).iter() {
            if let Some(p) = &v.shader {
                for m in &v.members {
                    out.insert(m.clone(), p.shader);
                }
            }
        }
        out
    }

    /// The presets folder was saved (an alias may re-map a suffix): remember the untouched recorded sub-types, and
    /// plan again; the settled plan re-types those whose pick changed (R5).
    pub fn presets_saved(&mut self) {
        let shaders = self.shaders_by_member();
        let mut keep = vec![];
        for i in &self.items {
            if self.untouched_subtype(&i.key, shaders.get(&i.key).copied()) {
                if let Some(v) = self
                    .project
                    .as_ref()
                    .and_then(|p| p.file_value(&i.key, "subtype"))
                {
                    keep.push((i.key.clone(), v));
                }
            }
        }
        self.history.retype_untouched = keep;
        self.replan();
    }

    /// Write the settled plan's own decisions for the fields no entry sets yet, as one undo step of its own (`Set
    /// {n} types automatically`, `values` when an alpha is among them) and one Problems row. Each `(key, field)` is written once per history
    /// (`History::auto_written`), so an Undo of the step stays undone. True when it wrote (and re-planned).
    pub(super) fn record_auto_decisions(&mut self) -> bool {
        // Never over a running export, nor over the crash restore its toast can still undo (Note FB5).
        if self.export.running.is_some() || self.crash_restore_on_top() {
            return false;
        }
        let Some(doc) = self.project.as_ref().filter(|p| !p.read_only()) else {
            return false;
        };
        // R5: after a presets save, an untouched sub-type follows the new pick (`presets_saved`). Taken only once the
        // recording can write, so a blocked recording keeps it for the next one.
        let retype = std::mem::take(&mut self.history.retype_untouched);
        let mut writes: Vec<(String, &'static str, serde_json::Value)> = vec![];
        let mut lines = vec![];
        let mut only_types = true;
        if !retype.is_empty() {
            let shaders = self.shaders_by_member();
            for (k, old) in retype {
                let new = self
                    .auto_pick(&k, shaders.get(&k).copied())
                    .map(|st| json!(st));
                if let Some(new) =
                    new.filter(|n| *n != old && doc.file_value(&k, "subtype") == Some(old.clone()))
                {
                    lines.push(format!(
                        "{}: {} → {}",
                        file_name(&k),
                        show_type(&old),
                        show_type(&new)
                    ));
                    writes.push((k, "subtype", new));
                }
            }
        }
        for i in &self.items {
            let own = matches!(i.kind, ItemKind::Image | ItemKind::Dds)
                && !matches!(i.status, ItemStatus::PackedInto(_) | ItemStatus::Skipped(_));
            let Some(j) = i.job.as_ref().filter(|_| own) else {
                continue;
            };
            let st = fields::auto_subtype_reason(j).map(|_| json!(j.profile.subtype));
            let mesh = |f: &str| j.provenance.get(f) == Some(&Provenance::MeshAlpha);
            let mesh_policy = mesh("alpha.policy");
            let alpha = mesh_policy.then(|| json!(j.profile.alpha.policy));
            let threshold =
                (mesh_policy && mesh("alpha.threshold")).then(|| json!(j.profile.alpha.threshold));
            for (f, v) in [
                ("subtype", st),
                ("alpha.policy", alpha),
                ("alpha.threshold", threshold),
            ] {
                let Some(v) = v else { continue };
                let done = self.history.auto_written.contains(&(i.key.clone(), f));
                if done || doc.file_value(&i.key, f).is_some() {
                    continue;
                }
                if f != "alpha.threshold" {
                    only_types &= f == "subtype";
                    let v = if f == "subtype" {
                        show_type(&v)
                    } else {
                        show(&v)
                    };
                    lines.push(format!("{}: {v}", file_name(&i.key)));
                }
                writes.push((i.key.clone(), f, v));
            }
        }
        // Note FI3: after a re-encode step still on top, the recording joins it (`set_reencode`).
        let join = self
            .reencode_follow
            .take()
            .filter(|k| self.history.undo.last().and_then(|c| c.merge.as_ref()) == Some(k));
        if writes.is_empty() {
            return false;
        }
        let n = lines.len();
        let noun = if only_types { "type" } else { "value" };
        let s = if n == 1 { "" } else { "s" };
        let label = format!("Set {n} {noun}{s} automatically");
        for (k, f, _) in &writes {
            self.history.auto_written.insert((k.clone(), f));
        }
        if let Some(k) = join {
            // One step with the re-encode (its label stays), so no row of its own offers a separate Undo.
            let pushes = self.history.pushes;
            self.edit_docs(&label, Some(k), true, |p, _| {
                for (k, f, v) in &writes {
                    p.set_file_value(k, f, v);
                }
            });
            return self.history.pushes != pushes;
        }
        let clean_before = !self.prompts_unsaved();
        let wrote = self.batch_edit(&label, |m| {
            m.edit("Set automatically", |p| {
                for (k, f, v) in &writes {
                    p.set_file_value(k, f, v);
                }
            })
        });
        if wrote {
            let typed = writes.iter().any(|(_, f, _)| *f == "subtype");
            self.auto_note = Some(AutoNote {
                text: format!("{n} {noun}{s} set automatically (High confidence)"),
                lines: lines
                    .into_iter()
                    .chain(typed.then(|| HIGH_CONFIDENCE.to_string()))
                    .collect(),
                step: self.history.pushes,
                clean_before,
            });
            self.invalidate_views();
        }
        wrote
    }
}
