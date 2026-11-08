//! Note DY17: the helper panel's edits. Each writes one `helper.*` selector of the helper file's own `[files]` entry in
//! `sdc.toml` as one undo step, then plans again; a read-only `sdc.toml` takes none (`AppModel::edit`). Note HN: it
//! also holds the read-only dock mapping (which chip and channel a fill lands on).
use super::sets::{SetView, SlotKind, Tails};
use super::AppModel;
use engine::helper::report::HelperReport;
use engine::helper::{FillTarget, HelperRead, HelperRole};
use engine::helper::{FILLS_FIELD, INVERT_FIELD, READ_FIELD, ROLE_FIELD};
use serde_json::{json, Value};
use std::path::Path;

/// The set's base name that a fill target with no output is filed under.
pub fn set_base(h: &HelperReport) -> String {
    Path::new(&h.set_key)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// A fill as the user reads it, the same on the badge, the panel and the Textures row: the chip and channel it
/// writes ("RMAOS R", "CNR alpha", "D alpha"). Note HP.
pub fn fill_text(t: FillTarget) -> String {
    use engine::helper::AlphaTarget as A;
    match t {
        FillTarget::Alpha(a) => {
            let chip = match a {
                A::Diffuse => "D",
                A::N => "N",
                A::F => "F",
                A::S => "S",
                A::Cnr => "CNR",
            };
            format!("{chip} alpha")
        }
        FillTarget::Packed { suffix, .. } => {
            format!("{} {}", suffix.to_uppercase(), fill_channel(t))
        }
    }
}

/// The file a fill target lands in: the stem of the planned output that ends in the target's suffix (so it matches
/// the `Show …` button), else the outputs' (or the set's) base name plus the suffix when the target did not land.
pub fn target_file(h: &HelperReport, t: FillTarget, tails: Tails) -> String {
    use engine::helper::AlphaTarget as A;
    let suffix = match t {
        FillTarget::Alpha(A::Diffuse) => None,
        FillTarget::Alpha(A::N) => Some("n"),
        FillTarget::Alpha(A::F) => Some("f"),
        FillTarget::Alpha(A::S) => Some("s"),
        FillTarget::Alpha(A::Cnr) => Some("cnr"),
        FillTarget::Packed { suffix, .. } => Some(suffix),
    };
    let stems = || h.outputs.iter().filter_map(|o| o.file_stem()?.to_str());
    let named = suffix
        .map(|s| format!("_{s}"))
        .and_then(|tail| stems().find_map(|s| s.strip_suffix(tail.as_str())));
    // A target with no output (or the colour one, which has no suffix) takes any output's base name, so a renamed
    // set reads `steel` beside the rows that read `steel_rmaos R`.
    let any = || {
        stems().next().map(|s| {
            let tail = super::sets::member_tail(s, tails).len();
            &s[..s.len() - tail]
        })
    };
    let base = set_base(h);
    let base = named.or_else(any).unwrap_or(&base);
    match suffix {
        Some(s) => format!("{base}_{s}"),
        None => base.to_string(),
    }
}

/// Note HN: the slot chip a fill target lands on. A `_m` target has no chip (CD8).
pub fn fill_slot(t: FillTarget) -> Option<SlotKind> {
    use engine::helper::AlphaTarget as A;
    match t {
        FillTarget::Alpha(A::Diffuse) => Some(SlotKind::Diffuse),
        FillTarget::Alpha(A::N) => Some(SlotKind::Normal),
        FillTarget::Alpha(A::F) => Some(SlotKind::Fuzz),
        FillTarget::Alpha(A::S) => Some(SlotKind::Subsurface),
        FillTarget::Alpha(A::Cnr) => Some(SlotKind::Coat),
        FillTarget::Packed { suffix, .. } => match suffix {
            "rmaos" | "rfaos" => Some(SlotKind::Rmaos),
            "p" => Some(SlotKind::Height),
            _ => None,
        },
    }
}

/// The channel a fill target writes: `alpha` or `R`/`G`/`B`/`A`.
pub fn fill_channel(t: FillTarget) -> &'static str {
    match t {
        FillTarget::Alpha(_) => "alpha",
        FillTarget::Packed { channel, .. } => {
            debug_assert!(channel < 4, "a packed channel is 0..=3, got {channel}");
            // the clamp keeps a release build from indexing past A
            ["R", "G", "B", "A"][usize::from(channel.min(3))]
        }
    }
}

/// One helper filling one channel of one slot chip.
#[derive(Debug, Clone, PartialEq)]
pub struct Docked {
    /// The helper's item key.
    pub helper: String,
    pub slot: SlotKind,
    pub channel: &'static str,
    /// The fill itself, for its wording (`fill_text`).
    pub target: FillTarget,
}

/// The helpers of the set's members, one entry per reported fill that lands on a chip.
pub fn docked(model: &AppModel, v: &SetView) -> Vec<Docked> {
    v.members
        .iter()
        .filter_map(|k| Some((k, model.item(k)?.helper.as_ref()?)))
        .flat_map(|(k, h)| {
            h.fills.iter().filter_map(move |&t| {
                Some(Docked {
                    helper: k.clone(),
                    slot: fill_slot(t)?,
                    channel: fill_channel(t),
                    target: t,
                })
            })
        })
        .collect()
}

impl AppModel {
    /// The tails of the set a helper report belongs to (`SetView::tails`), `Plain` when the set is not listed.
    pub fn helper_tails(&self, h: &HelperReport) -> Tails {
        super::sets::views(self)
            .iter()
            .find(|s| s.key == h.set_key)
            .map_or(Tails::Plain, |s| s.tails())
    }

    /// Note DY18b: a helper's current list. A hand-set list is its `sdc.toml` `helper.fills`, so a target that did not
    /// land is not lost; otherwise the engine's reported fills.
    pub fn current_fills(&self, key: &str, h: &HelperReport) -> Vec<FillTarget> {
        let hand = self
            .project
            .as_ref()
            .and_then(|p| p.file_value(key, FILLS_FIELD))
            .and_then(|v| engine::helper::fills_value(&v));
        match hand {
            Some(v) if h.fills_hand_set => v,
            _ => h.fills.clone(),
        }
    }

    fn helper_edit(&mut self, label: &str, key: &str, field: &'static str, v: Option<Value>) {
        let k = key.to_string();
        self.edit(label, move |p| match v {
            Some(v) => p.set_file_value(&k, field, &v),
            None => p.clear_file_value(&k, field),
        });
    }

    /// `None` clears `helper.role` (the role guessed from the name again).
    pub fn set_helper_role(&mut self, key: &str, role: Option<HelperRole>) {
        let v = role.map(|r| json!(r.id()));
        self.helper_edit("Helper role", key, ROLE_FIELD, v);
    }

    /// `None` clears `helper.fills` (the suggestion again); `Some(vec![])` writes `[]` (not used).
    pub fn set_helper_fills(&mut self, key: &str, fills: Option<Vec<FillTarget>>) {
        let v = fills.map(|f| json!(f.iter().map(FillTarget::id).collect::<Vec<_>>()));
        self.helper_edit("Helper fills", key, FILLS_FIELD, v);
    }

    /// `None` clears `helper.read` (R, or the recipe's luma).
    pub fn set_helper_read(&mut self, key: &str, read: Option<HelperRead>) {
        let v = read.map(|r| json!(r.id()));
        self.helper_edit("Helper read from", key, READ_FIELD, v);
    }

    /// `false` clears `helper.invert`.
    pub fn set_helper_invert(&mut self, key: &str, on: bool) {
        self.helper_edit(
            "Helper invert",
            key,
            INVERT_FIELD,
            on.then_some(json!(true)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::helper::AlphaTarget as A;

    #[test]
    fn fill_text_names_the_chip_and_channel() {
        let p = |suffix, channel| FillTarget::Packed { suffix, channel };
        for (t, want) in [
            (p("rmaos", 0), "RMAOS R"),
            (p("rfaos", 3), "RFAOS A"),
            (p("m", 1), "M G"),
            (p("p", 0), "P R"),
            (FillTarget::Alpha(A::Cnr), "CNR alpha"),
            (FillTarget::Alpha(A::Diffuse), "D alpha"),
            (FillTarget::Alpha(A::N), "N alpha"),
        ] {
            assert_eq!(fill_text(t), want, "{t:?}");
        }
    }
}
