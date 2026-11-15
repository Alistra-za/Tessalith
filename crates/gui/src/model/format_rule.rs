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
