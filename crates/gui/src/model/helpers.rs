//! Note DY17: the helper panel's edits. Each writes one `helper.*` selector of the helper file's own `[files]` entry in
//! `sdc.toml` as one undo step, then plans again; a read-only `sdc.toml` takes none (`AppModel::edit`).
use super::sets::{SetView, SlotKind};
use super::AppModel;
use engine::helper::report::HelperReport;
use engine::helper::{FillTarget, HelperRead, HelperRole};
use engine::helper::{FILLS_FIELD, INVERT_FIELD, READ_FIELD, ROLE_FIELD};
use serde_json::{json, Value};
use std::path::Path;

/// The set's base name that fill targets are labelled with ("x" in "x_rmaos R").
pub fn set_base(h: &HelperReport) -> String {
    Path::new(&h.set_key)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// A fill target's name as the user sees it: the stem of the planned output that ends in the target's suffix, minus
/// the suffix (so it matches the `Show …` button), else the set's base name when the target did not land.
pub fn target_label(h: &HelperReport, t: FillTarget) -> String {
    use engine::helper::AlphaTarget as A;
    let suffix = match t {
        FillTarget::Alpha(A::Diffuse) => None,
        FillTarget::Alpha(A::N) => Some("n"),
        FillTarget::Alpha(A::F) => Some("f"),
        FillTarget::Alpha(A::S) => Some("s"),
        FillTarget::Alpha(A::Cnr) => Some("cnr"),
        FillTarget::Packed { suffix, .. } => Some(suffix),
        FillTarget::Rnm(engine::config::RnmTarget::Cnr) => Some("cnr"),
        FillTarget::Rnm(_) => Some("n"),
    };
    let stems = || h.outputs.iter().filter_map(|o| o.file_stem()?.to_str());
    let named = suffix
        .map(|s| format!("_{s}"))
        .and_then(|tail| stems().find_map(|s| s.strip_suffix(tail.as_str())));
    // A target with no output (or the colour one, which has no suffix) takes any output's base name, so a renamed
    // set reads `steel` beside the rows that read `steel_rmaos R`.
    let any = || {
        stems().next().map(|s| {
            let tail = super::sets::member_tail(s, true).len();
            &s[..s.len() - tail]
        })
    };
    let base = set_base(h);
    t.label(named.or_else(any).unwrap_or(&base))
}

/// The `helper.fills` entries of the report's fills: layer blends (`Rnm`) are reports only, never written.
pub fn written_fills(h: &HelperReport) -> Vec<FillTarget> {
    h.fills
        .iter()
        .copied()
        .filter(|t| !matches!(t, FillTarget::Rnm(_)))
        .collect()
}

/// Note HN: the slot chip a fill target lands on. A `_m` target has no chip (CD8). Both `Rnm` blends sit on the
/// normal chip unless they aim at `_cnr` (the rule `target_label` uses).
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
        FillTarget::Rnm(engine::config::RnmTarget::Cnr) => Some(SlotKind::Coat),
        FillTarget::Rnm(_) => Some(SlotKind::Normal),
    }
}

/// The channel a fill target writes: `alpha`, `R`/`G`/`B`/`A`, or `layer` for a blended normal layer.
pub fn fill_channel(t: FillTarget) -> &'static str {
    match t {
        FillTarget::Alpha(_) => "alpha",
        FillTarget::Packed { channel, .. } => ["R", "G", "B", "A"][usize::from(channel.min(3))],
        FillTarget::Rnm(_) => "layer",
    }
}

/// One helper filling one channel of one slot chip.
#[derive(Debug, Clone, PartialEq)]
pub struct Docked {
    /// The helper's item key.
    pub helper: String,
    pub slot: SlotKind,
    pub channel: &'static str,
}

/// The helpers of the set's members, one entry per reported fill that lands on a chip, in member then `fills` order.
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
                })
            })
        })
        .collect()
}

impl AppModel {
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
            _ => written_fills(h),
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
