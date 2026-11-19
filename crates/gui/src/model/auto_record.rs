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
use crate::project::AutoWrite;
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
    /// The row's text, `{n} types set automatically (High confidence)`; its count is the number of decisions.
    pub text: String,
    /// `file name: value`, one per written value (a threshold rides on its policy's line), for the row's hover; the
    /// last line explains High confidence when a sub-type was set.
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FuzzNote {
    pub text: String,
    pub lines: Vec<String>,
    pub step: u64,
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
    /// The switch row offers Undo while its step (including any joined recording) is on top.
    step: u64,
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
        let keys: Vec<String> = self
            .items
            .iter()
            .map(|i| i.key.clone())
            .filter(|k| self.fuzz_recorded_skin(k))
            .collect();
        if keys.is_empty() {
            return false;
        }
        let merge = format!("fuzz-switch-{}", self.history.pushes);
        let n = keys.len();
        let s = if n == 1 { "" } else { "s" };
        let label = format!("Switch {n} fuzz-map set{s} off Advanced Skin");
        let cleared: Vec<_> = keys
            .iter()
            .map(|key| AutoWrite {
                key: key.clone(),
                field: "subtype",
                value: serde_json::Value::Null,
                only_if: Some(json!(Subtype::SkinAlbedo)),
            })
            .collect();
        self.edit_docs(&label, Some(merge.clone()), true, |p, _| {
            p.touch_auto(&cleared)
        });
        self.reencode_follow = Some(merge.clone());
        self.fuzz_switch = Some(FuzzSwitch {
            merge,
            lines: vec![],
            keys,
            step: self.history.pushes,
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
        self.fuzz_note = Some(FuzzNote {
            text: format!(
                "{n} set{s} left Advanced Skin (a _fuzz or _sheen map alone no longer picks it)"
            ),
            lines,
            // The switch's own step (I2): a user edit since then is the top step, and the row offers no Undo.
            step: switch.step,
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

    /// Record planner decisions into the baseline. Only a recording caused by a re-encode joins its step.
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
        let mut writes = vec![];
        let mut lines = vec![];
        // The key each line names, so a switch's row can drop the lines of the files its own lines already name.
        let mut line_keys: Vec<String> = vec![];
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
                    line_keys.push(k.clone());
                    writes.push(AutoWrite {
                        key: k,
                        field: "subtype",
                        value: new,
                        only_if: Some(old),
                    });
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
                if doc.file_value(&i.key, f).is_some() {
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
                    line_keys.push(i.key.clone());
                }
                writes.push(AutoWrite {
                    key: i.key.clone(),
                    field: f,
                    value: v,
                    only_if: None,
                });
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
        // A six-file cube's settings apply to all its faces (DT31).
        for (key, faces) in self.cube_face_keys() {
            let others: Vec<_> = writes
                .iter()
                .filter(|w| w.key == key)
                .flat_map(|w| {
                    faces.iter().map(|face| AutoWrite {
                        key: face.clone(),
                        ..w.clone()
                    })
                })
                .collect();
            writes.extend(others);
        }
        if let Some(k) = join {
            let label = format!("Set {n} {noun}{s} automatically");
            // One step with the re-encode (its label stays), so no row of its own offers a separate Undo. Fuzz-8: the
            // switch's row lists what it joined.
            let joins_switch = self.fuzz_switch.as_ref().is_some_and(|sw| sw.merge == k);
            let pushes = self.history.pushes;
            self.record_doc_edit(&label, Some(k), true, |p, _| {
                if joins_switch {
                    p.touch_auto(&writes);
                } else {
                    for w in &writes {
                        p.set_file_value(&w.key, w.field, &w.value);
                    }
                }
            });
            if let Some(sw) = self.fuzz_switch.as_mut().filter(|_| joins_switch) {
                // Final review B I1: a switched file is listed once, by the switch's own line (it names the type the
                // file now plans); the recording's lines for other files stay.
                let own: Vec<String> = lines
                    .into_iter()
                    .zip(&line_keys)
                    .filter(|(_, k)| !sw.keys.contains(k))
                    .map(|(l, _)| l)
                    .collect();
                sw.lines.extend(own);
                // The joined step is still the switch's (I2).
                sw.step = self.history.pushes;
            }
            return self.history.pushes != pushes;
        }
        let p = self
            .project
            .as_mut()
            .expect("writable document checked above");
        let rev = p.rev();
        p.touch_auto(&writes);
        if p.rev() == rev {
            return false;
        }
        self.history.apply_auto(&writes);
        let typed = writes.iter().any(|w| w.field == "subtype");
        self.auto_note = Some(AutoNote {
            text: format!("{n} {noun}{s} set automatically (High confidence)"),
            lines: lines
                .into_iter()
                .chain(typed.then(|| HIGH_CONFIDENCE.to_string()))
                .collect(),
        });
        self.invalidate_views();
        self.replan_changed(true);
        true
    }
}
