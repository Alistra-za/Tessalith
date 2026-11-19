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
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// The last recording, for its Problems row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AutoNote {
    /// The row's count is the step label's. A follow-only row names no suggestion confidence.
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

/// Fuzz-8: ends the switch row's hover.
pub const FUZZ_SWITCH_HOVER: &str = "A _fuzz or _sheen map alone no longer makes a set Advanced Skin. To keep a set skin, choose Advanced Skin as its shader.";

/// Fuzz-8: the switch on open while its step is still being completed (the recording that joins it).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FuzzSwitch {
    /// The step's history merge key; the recording joins it through `reencode_follow` (Note FI3).
    merge: String,
    /// The joined recording's lines.
    lines: Vec<String>,
    /// The Diffuse-slot keys whose `skin_albedo` the switch cleared.
    keys: Vec<String>,
    /// The `history.pushes` the switch's step left (moved by the joined recording): the row offers Undo, and the
    /// switch counts as automatic (FI4), only while it is the top step.
    step: u64,
    /// Note FI4: nothing prompted to be saved just before the switch.
    clean_before: bool,
}

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
    /// decides the lender's file), with an automatic shader, an untyped fuzz map image with no hand role, `key`
    /// neither made skin by a pick (`auto_skin`) nor locked, no recipe written on the set's packed key, and no other
    /// skin evidence (a skin-typed member, a hand-set fuzz role on a member or a shared file, a real `_rfaos` file, a
    /// shared skin set). The cheap `[files]` check runs first, as `presets_saved` asks this of every item.
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
        let stem = |k: &str| {
            Path::new(k)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };
        // A recipe written on the set's packed key, `<folder>/<base>_rfaos` (or `_rmaos`), built from `key` as the
        // engine's `set_is_advanced_skin` builds it from the file it resolves; `explicit_recipe` reads both keys.
        let base = split_suffix_with(&stem(key), &self.suffix_aliases).base;
        let packed = match key.rsplit_once('/') {
            Some((dir, _)) => format!("{dir}/{base}_rfaos"),
            None => format!("{base}_rfaos"),
        };
        if self.explicit_recipe(&packed) != PackRecipe::Auto {
            return false;
        }
        // Fuzz-22: a hand-set fuzz role is skin evidence, on a member or on a file shared in (Note HH). On an image
        // only, as the engine's `hand_role` reads it: a kept DDS takes no role.
        let hand_fuzz = |k: &str| {
            !k.to_ascii_lowercase().ends_with(".dds")
                && p.file_value(k, engine::helper::ROLE_FIELD) == Some(json!("fuzz"))
        };
        if v.shared_helpers.iter().any(|k| hand_fuzz(k)) {
            return false;
        }
        let mut fuzz = false;
        for k in &v.members {
            // Packed outputs are not files: the set's own `_rfaos` output is no evidence.
            if self.item(k).is_none_or(|i| i.kind == ItemKind::Packed) {
                continue;
            }
            if hand_fuzz(k) {
                return false;
            }
            let s = stem(k);
            // An image, as the engine's `untyped_fuzz_map`: a kept DDS is no fuzz map here.
            if is_fuzz_stem(&s)
                && !k.to_ascii_lowercase().ends_with(".dds")
                && typed(k).is_none()
                && p.file_value(k, engine::helper::ROLE_FIELD).is_none()
            {
                fuzz = true;
            }
            let skin_typed = typed(k).is_some_and(Subtype::is_skin);
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

    /// Fuzz-8: once per open, the settled plan clears each Diffuse-slot `skin_albedo` an older Tessalith recorded only
    /// because of a fuzz map (`fuzz_recorded_skin`), in one undo step the next recording joins. True when it wrote
    /// (a re-plan follows). Fuzz-26: nothing else is cleared; Fuzz-18: nothing is recorded on the fuzz map.
    pub(super) fn switch_fuzz_sets(&mut self) -> bool {
        if self.fuzz_switch_checked {
            return false;
        }
        // As the recording (Note FB5): a plan under a running export or the crash restore leaves the switch for the
        // next settled plan.
        if self.export.running.is_some() || self.crash_restore_on_top() {
            return false;
        }
        self.fuzz_switch_checked = true;
        // A read-only `sdc.toml` (unloadable, or flagged read-only so the change cannot be saved) is not switched.
        if !self.switch_fuzz_on_open || self.sdc_toml_writable().is_err() {
            return false;
        }
        let keys: Vec<String> = {
            let safe = super::shared_alpha::safe_subtype_write(self);
            self.items
                .iter()
                .filter(|i| self.fuzz_recorded_skin(&i.key) && safe(&i.key))
                .map(|i| i.key.clone())
                .collect()
        };
        if keys.is_empty() {
            return false;
        }
        let clean_before = !self.prompts_unsaved();
        let merge = format!("fuzz-switch-{}", self.history.pushes);
        let n = keys.len();
        let s = if n == 1 { "" } else { "s" };
        let label = format!("Switch {n} fuzz-map set{s} off Advanced Skin");
        let cleared = keys.clone();
        self.automatic_step = true;
        self.edit_docs(&label, Some(merge.clone()), true, move |p, _| {
            for k in &cleared {
                p.clear_file_value(k, "subtype");
            }
        });
        self.automatic_step = false;
        self.reencode_follow = Some(merge.clone());
        self.fuzz_switch = Some(FuzzSwitch {
            merge,
            lines: vec![],
            keys,
            step: self.history.pushes,
            clean_before,
        });
        true
    }

    /// Fuzz-8: a fresh read of the documents (an open or a reload) drops the previous switch, its row and a step
    /// still waiting for its recording, and lets the next settled plan switch the text it read.
    pub(super) fn reset_fuzz_switch(&mut self) {
        self.fuzz_note = None;
        if let Some(sw) = self.fuzz_switch.take() {
            if self.reencode_follow.as_ref() == Some(&sw.merge) {
                self.reencode_follow = None;
            }
        }
        self.fuzz_switch_checked = false;
    }

    /// FI4: the switch, with what joined it, is the only unsaved change since a document that asked nothing: its row
    /// (or, before the row is built, its step) is the top step.
    pub(super) fn only_switch_unsaved(&self) -> bool {
        let top = |clean: bool, step: u64| clean && step == self.history.pushes;
        self.fuzz_note
            .as_ref()
            .is_some_and(|n| top(n.clean_before, n.step))
            || self
                .fuzz_switch
                .as_ref()
                .is_some_and(|s| top(s.clean_before, s.step))
    }

    /// Fuzz-8: once the switch's step is complete (the plan settled with nothing more to record), its Problems row:
    /// each cleared file with the type it now plans, the joined recording's lines, then `FUZZ_SWITCH_HOVER`.
    pub(super) fn finish_fuzz_switch(&mut self) {
        let Some(switch) = self.fuzz_switch.take() else {
            return;
        };
        let n = switch.keys.len();
        let s = if n == 1 { "" } else { "s" };
        let was = super::labels::subtype_name(Subtype::SkinAlbedo);
        let lines = switch
            .keys
            .iter()
            .map(|k| {
                // A file with no job by now (deleted, unreadable) has no type to name.
                match self.item(k).and_then(|i| i.job.as_ref()) {
                    Some(j) => format!(
                        "{}: {was} → {}",
                        file_name(k),
                        super::labels::subtype_name(j.profile.subtype)
                    ),
                    None => format!("{}: {was}", file_name(k)),
                }
            })
            .chain(switch.lines)
            .chain([FUZZ_SWITCH_HOVER.to_string()])
            .collect();
        self.fuzz_note = Some(AutoNote {
            text: format!(
                "{n} set{s} left Advanced Skin (a _fuzz or _sheen map alone no longer picks it)"
            ),
            lines,
            // The switch's own step (I2): a user edit since then is the top step, and the row offers no Undo.
            step: switch.step,
            clean_before: switch.clean_before,
        });
        // Fuzz-29: as after `auto_note`, so the cached rows show it on the next frame.
        self.invalidate_views();
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
        let safe = super::shared_alpha::safe_subtype_write(self);
        let mut previous = HashMap::new();
        if !retype.is_empty() {
            let shaders = self.shaders_by_member();
            for (k, old) in retype {
                let new = self
                    .auto_pick(&k, shaders.get(&k).copied())
                    .map(|st| json!(st));
                if let Some(new) =
                    new.filter(|n| *n != old && doc.file_value(&k, "subtype") == Some(old.clone()))
                {
                    if safe(&k) {
                        previous.insert(k.clone(), old);
                        writes.push((k, "subtype", new));
                    }
                }
            }
        }
        // Note JS5: a type the list cannot hold is never written: with `sdc-project.toml` read-only the recorder
        // writes the engine's own types only, as `pbr_follow` skips its propagation then.
        let sets = self.sets_doc.as_ref().filter(|s| !s.read_only());
        // Note JS5 (N-17, G1): an untouched normal follows its set's settled pick into PBR, in this step and its row.
        // The marker keeps an Undo of the step undone, as `auto_written` does below.
        let mut followed: Vec<String> = vec![];
        let mut promote = vec![];
        if sets.is_some() {
            for (k, _) in self.normals_to_follow() {
                if previous.contains_key(&k)
                    || !safe(&k)
                    || self
                        .history
                        .auto_written
                        .contains(&(k.clone(), "normal-follow"))
                {
                    continue;
                }
                let old = doc.file_value(&k, "subtype").unwrap_or(json!(null));
                previous.insert(k.clone(), old.clone());
                followed.push(k.clone());
                promote.push(writes.len());
                writes.push((k, "subtype", old));
            }
        }
        // Note JS5 (N-19): a never-recorded tangent-space `_n` in its set's own Normal slot, under a pick that
        // `bakes_any_tangent`, is recorded `pbr_normal` straight away (a PBR Landscape set's `landscape_normal`
        // too), when `alpha_free` holds, it is not locked and it is not a kept DDS.
        let locked = super::sets::locked_items(self);
        let into_pbr: HashSet<String> = match sets {
            Some(s) if self.target == Target::Cs => super::sets::views(self)
                .iter()
                .filter(|v| v.shader.as_ref().is_some_and(|p| p.bakes_any_tangent()))
                .filter_map(|v| match v.slots.get(&SlotKind::Normal) {
                    Some(SlotFill::Own { item }) => Some(item.clone()),
                    _ => None,
                })
                .filter(|k| {
                    super::normal_follow::alpha_free(doc, s, k)
                        && !locked.contains_key(k)
                        && self.item(k).is_some_and(|i| !i.kept_dds())
                })
                .collect(),
            _ => Default::default(),
        };
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
                if f == "subtype" {
                    if !safe(&i.key) {
                        continue;
                    }
                    if into_pbr.contains(&i.key)
                        && matches!(
                            j.profile.subtype,
                            Subtype::Normal | Subtype::LandscapeNormal
                        )
                    {
                        promote.push(writes.len());
                    }
                }
                writes.push((i.key.clone(), f, v));
            }
        }
        drop(safe);
        // Note FI3: after a re-encode step still on top, the recording joins it (`set_reencode`).
        let join = self
            .reencode_follow
            .take()
            .filter(|k| self.history.undo.last().and_then(|c| c.merge.as_ref()) == Some(k));
        if writes.is_empty() {
            return false;
        }
        let n = writes
            .iter()
            .filter(|(_, f, _)| *f != "alpha.threshold")
            .count();
        let noun = if writes.iter().all(|(_, f, _)| *f == "subtype") {
            "type"
        } else {
            "value"
        };
        let s = if n == 1 { "" } else { "s" };
        let label = format!("Set {n} {noun}{s} automatically");
        for (k, f, _) in &writes {
            self.history.auto_written.insert((k.clone(), f));
        }
        let follow_only = followed.len() == writes.len();
        for k in followed {
            self.history.auto_written.insert((k, "normal-follow"));
        }
        let sets_clean = !self.sets_user_dirty();
        let pushes = self.history.pushes;
        let clean_before = !self.prompts_unsaved();
        self.automatic_step = true;
        self.edit_docs(&label, join.clone(), true, |p, s| {
            // A parseable document can still have a scalar `project`. Keep the original type unless the listing took.
            if !promote.is_empty() {
                let mut listing = s.auto_normal();
                for &idx in &promote {
                    listing.insert(writes[idx].0.clone(), "pbr_normal".into());
                }
                s.set_auto_normal(&listing);
                let recorded = s.auto_normal();
                for idx in promote {
                    let (k, _, v) = &mut writes[idx];
                    if recorded.get(k).is_some_and(|word| word == "pbr_normal") {
                        *v = json!(Subtype::PbrNormal);
                    }
                }
            }
            // Apply the checked subtype destinations before an alpha edit can create a shared stem table.
            let types = writes.iter().filter(|(_, f, _)| *f == "subtype");
            let other = writes.iter().filter(|(_, f, _)| *f != "subtype");
            for (k, f, v) in types.chain(other) {
                p.set_file_value(k, f, v);
            }
            super::normal_follow::list_normals(p, s, &writes);
        });
        self.automatic_step = false;
        self.note_listing(sets_clean);
        let lines = writes
            .iter()
            .filter(|(_, f, _)| *f != "alpha.threshold")
            .map(|(k, f, v)| {
                let name = file_name(k);
                let line = match *f {
                    "subtype" => match previous.get(k) {
                        Some(old) => format!("{name}: {} → {}", show_type(old), show_type(v)),
                        None => format!("{name}: {}", show_type(v)),
                    },
                    _ => format!("{name}: {}", show(v)),
                };
                (k, line)
            });
        if let Some(k) = join {
            if let Some(sw) = self.fuzz_switch.as_mut().filter(|sw| sw.merge == k) {
                // The switch already names each of its own files.
                sw.lines
                    .extend(lines.filter(|(k, _)| !sw.keys.contains(k)).map(|(_, l)| l));
                sw.step = self.history.pushes;
            }
            return self.history.pushes != pushes;
        }
        if self.history.pushes == pushes {
            return false;
        }
        let typed = !follow_only && writes.iter().any(|(_, f, _)| *f == "subtype");
        let confidence = if follow_only {
            ""
        } else {
            " (High confidence)"
        };
        self.auto_note = Some(AutoNote {
            text: format!("{n} {noun}{s} set automatically{confidence}"),
            lines: lines
                .map(|(_, l)| l)
                .chain(typed.then(|| HIGH_CONFIDENCE.to_string()))
                .collect(),
            step: self.history.pushes,
            clean_before,
        });
        self.invalidate_views();
        true
    }
}
