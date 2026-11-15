//! Note JD (owner, 2026-11-15): the GUI's format rule. A grey map (every sub-type of `Category::Grayscale`) is BC4 or
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

/// The pair a disallowed `cur` takes when no layer gives `st` an allowed one (Note JD3): the best compressed format
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
    /// The write guard (Note JD2): false when writing `field` = `value` on `item` would leave a (format, tag) pair its
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
