//! Up to three likely types for textures that wait for one, each with the reason it is likely.
//! Three sources, in this order: the engine's own suggestions, the types written for files with the same name ending,
//! and what the pixels look like. Nothing here depends on selection, hash-map or file-system order.
use super::items::key_file_name as file_name;
use super::AppModel;
use engine::config::Subtype;
use std::collections::BTreeMap;

/// The most buttons a card offers.
const MAX_LIKELY: usize = 3;

/// The file-name ending the pairing compares: the stem after its last `_`, lower case, without trailing digits.
fn name_tail(key: &str) -> Option<String> {
    let name = file_name(key);
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    let (_, tail) = stem.rsplit_once('_')?;
    let tail = tail
        .to_lowercase()
        .trim_end_matches(|c: char| c.is_ascii_digit())
        .to_string();
    (!tail.is_empty()).then_some(tail)
}

/// Types ranked by how many keys back them (descending), then by id; each with the reason of its first key.
fn ranked(votes: BTreeMap<String, (Subtype, usize, String)>) -> Vec<(Subtype, String)> {
    let mut v: Vec<(Subtype, usize, String)> = votes.into_values().collect();
    // `votes` is keyed by id, so a stable sort by count leaves ties in id order.
    v.sort_by_key(|e| std::cmp::Reverse(e.1));
    v.into_iter().map(|(st, _, why)| (st, why)).collect()
}

/// The types worth trying first for `keys` (the textures a card asks about), at most three, with no duplicates, each
/// offered to every one of them under the target. The reason is the button's hover.
pub fn likely_types(m: &AppModel, keys: &[String]) -> Vec<(Subtype, String)> {
    let mut eligible: Vec<&String> = keys
        .iter()
        .filter(|k| m.item(k).is_some_and(|i| i.job.is_some()))
        .collect();
    eligible.sort();
    eligible.dedup();
    if eligible.is_empty() {
        return vec![];
    }
    let offered = |st: Subtype| eligible.iter().all(|k| m.type_offered_to(k, st));

    // 1. The engine's suggestions: one vote per key per type; the reason comes from the first key in key order.
    let mut engine_votes: BTreeMap<String, (Subtype, usize, String)> = BTreeMap::new();
    for k in &eligible {
        let Some(job) = m.item(k).and_then(|i| i.job.as_ref()) else {
            continue;
        };
        let mut seen: Vec<Subtype> = vec![];
        for s in &job.suggestions {
            if seen.contains(&s.subtype) {
                continue;
            }
            seen.push(s.subtype);
            let e = engine_votes
                .entry(s.subtype.id().to_string())
                .or_insert_with(|| (s.subtype, 0, s.reason.clone()));
            e.1 += 1;
        }
    }

    // 2. Types written for other files that end the same way, whoever wrote them.
    let mut written_votes: BTreeMap<String, (Subtype, usize, String)> = BTreeMap::new();
    if let Some(p) = &m.project {
        let tails: Vec<String> = eligible.iter().filter_map(|k| name_tail(k)).collect();
        let mut file_keys = p.file_keys();
        file_keys.sort();
        for k in file_keys {
            // `[files]` keys fold Unicode (Note BF), as the engine compares them.
            if eligible.iter().any(|e| engine::sets::same_id(e, &k)) {
                continue;
            }
            let Some(tail) = name_tail(&k) else { continue };
            if !tails.contains(&tail) {
                continue;
            }
            let Some(st) = p
                .file_value(&k, "subtype")
                .and_then(|v| serde_json::from_value::<Subtype>(v).ok())
            else {
                continue;
            };
            let e = written_votes
                .entry(st.id().to_string())
                .or_insert_with(|| (st, 0, format!("{} has this type.", file_name(&k))));
            e.1 += 1;
        }
    }

    // 3. The pixels, pooled over the textures: Normal, then Gray.
    let analyses = || {
        eligible
            .iter()
            .filter_map(|k| m.item(k).and_then(|i| i.analysis.as_ref()))
    };
    let mut pixels: Vec<(Subtype, String)> = vec![];
    if analyses().any(|a| a.looks_like_tangent_normal()) {
        pixels.push((Subtype::Normal, "The pixels look like a normal map.".into()));
    }
    if analyses().any(|a| a.grayscale) {
        pixels.push((Subtype::Gray, "The pixels are grey.".into()));
    }

    let mut out: Vec<(Subtype, String)> = vec![];
    for (st, why) in ranked(engine_votes)
        .into_iter()
        .chain(ranked(written_votes))
        .chain(pixels)
    {
        if offered(st) && !out.iter().any(|(s, _)| *s == st) {
            out.push((st, why));
        }
    }
    out.truncate(MAX_LIKELY);
    out
}
