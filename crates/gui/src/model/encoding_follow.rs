//! Task 9 (owner, 2026-10-13; DECISIONS Z11), Note JG (owner, 2026-11-15): encoding follows type. A texture whose
//! resolved format and tag its sub-type does not allow (`format_rule::allowed`) is fixed on the settled plan: an
//! explicit pair is cleared, so the type's default applies; else the best allowed pair is pinned.
use super::fields::format_label;
use super::format_rule::{allowed, best};
use super::{history, AppModel};
use crate::project::ProjectDoc;
use engine::config::{ColorTag, Format, Subtype, Target};
use std::collections::HashMap;

/// What a cleanup did, waiting for the plan that shows the new formats.
#[derive(Debug, Default)]
pub(super) struct Pending {
    fixed: Vec<Fixed>,
    /// 1 after the first round (clear or pin), 2 after the round that pins what a clear left disallowed.
    round: u8,
    /// Folded into a user step, whose Undo takes the fix back with it (JG3).
    undo: bool,
}

#[derive(Debug)]
struct Fixed {
    key: String,
    /// "name: old type → new type", or "name: <pair> is not valid for <type>".
    head: String,
    pinned: bool,
}

/// What the cleanup does with one texture.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The sub-type allows the pair (or allows nothing: it is not written).
    Keep,
    /// The explicit pair is not allowed: clear it, so the type's default applies.
    Clear,
    /// Not allowed and nothing explicit to clear: pin the best allowed pair.
    Pin((Format, ColorTag)),
}

/// The rule for a texture of sub-type `st` resolving to `pair`: `explicit` when its `[files]` entry sets the format
/// or tag. Note JG3: no lock keeps a pair the type does not allow.
pub fn verdict(target: Target, st: Subtype, pair: (Format, ColorTag), explicit: bool) -> Verdict {
    decide(&allowed(st, target), target, st, pair, explicit)
}

fn decide(
    list: &[(Format, ColorTag)],
    target: Target,
    st: Subtype,
    pair: (Format, ColorTag),
    explicit: bool,
) -> Verdict {
    if list.is_empty() || list.contains(&pair) {
        return Verdict::Keep;
    }
    if explicit {
        return Verdict::Clear;
    }
    best(st, target, pair).map_or(Verdict::Keep, Verdict::Pin)
}

fn type_name(st: Subtype) -> String {
    super::labels::subtype_name(st)
}

fn explicit(doc: &ProjectDoc, key: &str) -> bool {
    ["format", "color.tag"]
        .iter()
        .any(|f| doc.file_value(key, f).is_some())
}

impl AppModel {
    /// Each planned texture's sub-type, by item key.
    pub(super) fn subtypes(&self) -> HashMap<String, Subtype> {
        self.items
            .iter()
            .filter_map(|i| Some((i.key.clone(), i.job.as_ref()?.profile.subtype)))
            .collect()
    }

    /// DECISIONS Z11: keep the sub-types before this plan chain; a fix's note names a change between them.
    pub(super) fn keep_type_base(&mut self) {
        if self.type_base.is_none() {
            self.type_base = Some(self.subtypes());
        }
    }

    /// Forget the cleanup state: an undo, redo, open or reload restores a state as it was, and a note or pending
    /// cleanup of before no longer applies. Its plan still runs the rule (Note JG3).
    pub(super) fn reset_type_follow(&mut self) {
        self.encoding_note = None;
        self.type_pending = None;
        self.type_base = None;
        self.type_cause = false;
        self.type_cleaned = false;
        self.forget_new_sets();
    }

    /// A user edit recorded a step and re-plans: a cleanup on its settled plan folds into that step.
    pub(super) fn type_edit_recorded(&mut self) {
        self.encoding_note = None;
        self.type_pending = None;
        self.type_cause = true;
        self.type_cleaned = false;
    }

    /// The fixes this settled plan needs, as (key, verdict, head). Round 1 looks at every texture; round 2 only pins
    /// a texture round 1 cleared whose default is not allowed either.
    fn format_fixes(
        &self,
        doc: &ProjectDoc,
        base: &HashMap<String, Subtype>,
        prev: Option<&Pending>,
        round: u8,
    ) -> Vec<(String, Verdict, String)> {
        let mut lists: HashMap<Subtype, Vec<(Format, ColorTag)>> = HashMap::new();
        let mut out = vec![];
        for i in &self.items {
            let Some(j) = &i.job else { continue };
            let st = j.profile.subtype;
            let e = (j.profile.format, j.profile.color.tag);
            let list = lists.entry(st).or_insert_with(|| allowed(st, self.target));
            if list.is_empty() || list.contains(&e) {
                continue;
            }
            let earlier = prev.and_then(|p| p.fixed.iter().find(|f| f.key == i.key));
            let ex = explicit(doc, &i.key);
            if round == 2 && (earlier.is_none_or(|f| f.pinned) || ex) {
                continue;
            }
            let head = earlier
                .map(|f| f.head.clone())
                .unwrap_or_else(|| match base.get(&i.key) {
                    Some(&old) if old != st => {
                        format!("{}: {} → {}", i.name, type_name(old), type_name(st))
                    }
                    _ => format!(
                        "{}: {} is not valid for {}",
                        i.name,
                        format_label(e.0, e.1),
                        type_name(st)
                    ),
                });
            match decide(list, self.target, st, e, ex) {
                Verdict::Keep => {}
                v => out.push((i.key.clone(), v, head)),
            }
        }
        out
    }

    /// Run from `finish_plan` when a plan chain has settled (the final pass: output paths agree with sub-types).
    /// Writes the note of a cleanup this plan shows, then fixes every texture whose pair its sub-type does not allow
    /// (Note JG3): one edit and one replan. The edit is folded into the user step that started the chain, else it is
    /// its own step. True when it re-planned. The cleanup's own plan (`type_cleaned`) only runs a second, pinning
    /// round for what the first cleared, so a chain cleans at most twice; it does not use `auto_replans`.
    pub(super) fn settle_types(&mut self) -> bool {
        let base = self.type_base.take().unwrap_or_default();
        let prev = self.type_pending.take();
        let cause = std::mem::take(&mut self.type_cause);
        let cleaned = std::mem::take(&mut self.type_cleaned);
        let round = match (&prev, cleaned) {
            (Some(p), true) => p.round + 1,
            _ => 1,
        };
        let fixes = match self.project.as_ref().filter(|p| !p.read_only()) {
            Some(doc) if round <= 2 => self.format_fixes(doc, &base, prev.as_ref(), round),
            _ => vec![],
        };
        if fixes.is_empty() {
            if let Some(p) = prev {
                self.set_type_note(p);
            }
            return false;
        }
        let mut pending = match prev {
            Some(p) if round == 2 => p,
            _ => Pending {
                undo: cause,
                ..Pending::default()
            },
        };
        pending.round = round;
        let before = Self::snapshot_of(self.project.as_ref(), self.sets_doc.as_ref());
        self.batch = true;
        for (k, v, head) in fixes {
            let pinned = match v {
                // Clearing the format clears the explicit tag too (`clear_field`).
                Verdict::Clear => {
                    self.clear_field(&k, "format");
                    false
                }
                Verdict::Pin((f, t)) => {
                    self.set_format(&k, f, t);
                    true
                }
                Verdict::Keep => continue,
            };
            match pending.fixed.iter_mut().find(|f| f.key == k) {
                Some(f) => f.pinned = pinned,
                None => pending.fixed.push(Fixed {
                    key: k,
                    head,
                    pinned,
                }),
            }
        }
        self.batch = false;
        let after = Self::snapshot_of(self.project.as_ref(), self.sets_doc.as_ref());
        if after == before {
            self.set_type_note(pending);
            return false;
        }
        match self.history.undo.last_mut() {
            // The user step that started this chain, or the first round's step: one undo restores the edit and
            // the cleanup.
            Some(last) if (cause || round == 2) && last.after == before => last.after = after,
            // A change no step caused (an open, a reload, a scan vote) is its own step.
            _ => self.history.push(history::Command {
                label: "Encoding follows type".into(),
                before,
                after,
                merge: None,
                created_file: None,
                type_alpha: false,
                normal_prunes: Default::default(),
            }),
        }
        self.type_pending = Some(pending);
        self.type_cleaned = true;
        self.replan();
        true
    }

    /// The Problems note: one texture's line, or a count with every line listed.
    fn set_type_note(&mut self, p: Pending) {
        let lines: Vec<String> = p
            .fixed
            .iter()
            .map(|f| {
                let now = self
                    .item(&f.key)
                    .and_then(|i| i.job.as_ref())
                    .map(|j| format_label(j.profile.format, j.profile.color.tag))
                    .unwrap_or_else(|| "the type's".into());
                if f.pinned {
                    format!(
                        "{}; format set to {now}, the best this type allows.",
                        f.head
                    )
                } else {
                    format!("{}; format reset to the default ({now}).", f.head)
                }
            })
            .collect();
        let line = match lines.as_slice() {
            [] => return,
            [one] => one.clone(),
            _ => format!(
                "Encoding follows type: {} set to a format their type allows",
                textures(lines.len())
            ),
        };
        self.messages
            .log(super::messages::Severity::Info, line.clone());
        self.encoding_note = Some((line, lines.join("\n")));
        // Note FC3: the note is a Problems row; its Undo is offered while its step is the top one. Note JG3: a fix no
        // user step caused offers none, since the rule would fix it again.
        self.encoding_note_step = if p.undo {
            self.history.pushes
        } else {
            u64::MAX
        };
        self.invalidate_views();
    }
}

fn textures(n: usize) -> String {
    format!("{n} texture{}", if n == 1 { "" } else { "s" })
}
