//! Note JF (owner, 2026-11-15): the GUI's format rule. A grey map (every sub-type of `Category::Grayscale`) is BC4 or
//! R8, UNORM: "a grayscale map is always either bc4 or r8, no bulk edit or setting should override that", a hard rule
//! in the GUI, not in the engine (which still accepts what `policy::allowed_encodings_for` lists). Every format
//! picker, the bulk and Copy settings check, the write guards and the settle cleanup (`encoding_follow`) read this
//! list, so a pair outside it is never offered, written or kept.
use engine::config::{Category, ColorTag, Format, Subtype, Target};

/// The (format, tag) pairs the GUI offers and keeps for `st` under `target`: the engine's list, narrowed to BC4 and
/// R8 UNORM for a grey map. Empty where the sub-type is unavailable (nothing is written).
pub fn allowed(st: Subtype, target: Target) -> Vec<(Format, ColorTag)> {
    let grey = st.category() == Category::Grayscale;
    let all = engine::policy::allowed_encodings_for(st, target, grey);
    if !grey {
        return all;
    }
    all.into_iter()
        .filter(|&(f, t)| matches!(f, Format::Bc4 | Format::R8) && t == ColorTag::Unorm)
        .collect()
}

/// The best compressed formats first (owner, 2026-11-15: "resolve to the best compressed version for that map type,
/// usually bc7"); the rest follow in the engine's list order.
const BEST: [Format; 5] = [
    Format::Bc7,
    Format::Bc5,
    Format::Bc4,
    Format::Bc3,
    Format::Bc1,
];

/// The pair a disallowed `cur` takes when no layer gives `st` an allowed one (Note JF3): the best compressed format
/// `allowed` lists, with `cur`'s tag where that format allows it, else the format's first allowed tag. `None` when
/// nothing is allowed (the sub-type is unavailable under `target`).
pub fn best(st: Subtype, target: Target, cur: (Format, ColorTag)) -> Option<(Format, ColorTag)> {
    let list = allowed(st, target);
    let f = BEST
        .into_iter()
        .find(|f| list.iter().any(|(x, _)| x == f))
        .or_else(|| list.first().map(|p| p.0))?;
    list.iter()
        .copied()
        .find(|&p| p == (f, cur.1))
        .or_else(|| list.iter().copied().find(|p| p.0 == f))
}

impl super::AppModel {
    /// The write guard (Note JF2): false when writing `field` = `value` on `item` would leave a (format, tag) pair its
    /// sub-type does not allow. A bare format pairs with the texture's current tag and a lone tag with its current
    /// format, as the bulk check does (DECISIONS AF5). The sub-type is the document's when it names one, so a sub-type
    /// written earlier in the same batch (Copy settings) counts before its plan lands. Other fields, and a texture
    /// with no plan yet, pass.
    pub(super) fn format_write_ok(
        &self,
        item: &str,
        field: &str,
        value: &serde_json::Value,
    ) -> bool {
        let Some(p) = self
            .item(item)
            .and_then(|i| i.job.as_ref())
            .map(|j| &j.profile)
        else {
            return true;
        };
        let st = self
            .project
            .as_ref()
            .and_then(|d| d.file_value(item, "subtype"))
            .and_then(|v| serde_json::from_value::<Subtype>(v).ok())
            .unwrap_or(p.subtype);
        let pair = match field {
            "format" => serde_json::from_value::<(Format, ColorTag)>(value.clone())
                .ok()
                .or_else(|| {
                    serde_json::from_value::<Format>(value.clone())
                        .ok()
                        .map(|f| (f, p.color.tag))
                }),
            "color.tag" => serde_json::from_value::<ColorTag>(value.clone())
                .ok()
                .map(|t| (p.format, t)),
            _ => return true,
        };
        pair.is_none_or(|e| allowed(st, self.target).contains(&e))
    }
}

/// The Presets tab's write guard (Note JF2): false when writing `field` = `value` in `st`'s scope would leave a pair
/// `st` does not allow under the preset's target. A lone tag pairs with the row's resolved format.
pub(crate) fn preset_write_ok(
    pm: &super::presets::PresetsModel,
    st: Subtype,
    field: &str,
    value: &serde_json::Value,
) -> bool {
    let row_format = || {
        pm.row(st)
            .and_then(|r| r.profile.as_ref().ok())
            .map(|p| p.format)
    };
    let pair = match field {
        "format" => serde_json::from_value::<(Format, ColorTag)>(value.clone()).ok(),
        "color.tag" => serde_json::from_value::<ColorTag>(value.clone())
            .ok()
            .and_then(|t| Some((row_format()?, t))),
        _ => return true,
    };
    pair.is_none_or(|e| allowed(st, pm.target).contains(&e))
}

/// Note JF4: fix an editable preset draft whose sub-type row resolves to a pair `allowed` does not list: the draft's
/// own format and tag are dropped, then, when the base still gives a disallowed pair, `best` is set. The lines it
/// wrote, or `None`.
// shortcut: suffix branch rows are not fixed here (their combo and setter are guarded); upgrade if a user preset's
// branch gives a grey type another format.
pub(crate) fn fix_preset(pm: &mut super::presets::PresetsModel) -> Option<String> {
    use super::fields::format_label;
    if pm.draft.as_ref().is_none_or(|d| d.read_only) {
        return None;
    }
    let bad = |pm: &super::presets::PresetsModel, st: Subtype| {
        let p = pm.row(st)?.profile.as_ref().ok()?;
        let e = (p.format, p.color.tag);
        let list = allowed(st, pm.target);
        (!list.is_empty() && !list.contains(&e)).then_some(e)
    };
    let mut lines = vec![];
    for st in Subtype::ALL {
        let Some(old) = bad(pm, st) else { continue };
        if pm.field(st, "format").is_some() || pm.field(st, "color.tag").is_some() {
            // Dropping the format drops its tag too (`drop_field`).
            pm.clear_field(st, "format");
        }
        if let Some(still) = bad(pm, st) {
            if let Some((f, t)) = best(st, pm.target, still) {
                pm.set_field(st, "format", serde_json::json!([f, t]));
            }
        }
        let now = pm
            .row(st)
            .and_then(|r| r.profile.as_ref().ok())
            .map_or_else(String::new, |p| format_label(p.format, p.color.tag));
        lines.push(format!(
            "{}: {} is not valid for this type; format set to {now}.",
            super::labels::subtype_name(st),
            format_label(old.0, old.1)
        ));
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}
