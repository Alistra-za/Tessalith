//! Named Rust checks for the UI driver's `check` step. A filter's predicate is Rust (a script can't hold one), so it
//! lives here and a script runs it by name.

use sdc_gui::model::{sets, AppModel};

/// `Ok(None)`: holds. `Ok(Some(note))`: not decided, with the reason. `Err`: broken.
pub type Oracle = fn(&AppModel) -> Result<Option<String>, String>;

pub const ORACLES: &[(&str, Oracle)] = &[("sets_pills_match_their_counts", pills_match_counts)];

/// One pill on and nothing else: the sets shown are the pill's count. No filter: every set is shown.
fn pills_match_counts(m: &AppModel) -> Result<Option<String>, String> {
    let f = &m.sets_filter;
    let pills = [f.attention, f.ready, f.no_json];
    let on = pills.iter().filter(|p| **p).count()
        + usize::from(!f.search.trim().is_empty())
        + usize::from(f.shader.is_some());
    let shown = m.visible_sets().len();
    let c = m.set_counts();
    let (what, want) = match on {
        0 => ("no filter", sets::views(m).len()),
        1 if f.attention => ("Not ready", c.attention),
        1 if f.ready => ("Ready", c.ready),
        1 if f.no_json => ("No JSON", c.no_json),
        _ => return Ok(Some("not checked: more than one filter part is on".into())),
    };
    if shown == want {
        Ok(None)
    } else {
        Err(format!("{what} shows {shown} sets but counts {want}"))
    }
}

/// Compares what a filter shows with what `want` says it should: `Err` names the rows wrongly shown and wrongly
/// hidden. `Ok(Some(note))` when the filter hides everything or changes nothing, which no row-by-row test notices.
#[allow(dead_code)] // the filter oracles the fixtures run add are its callers
pub fn filter_matches(
    visible: &[String],
    all: &[String],
    want: impl Fn(&str) -> bool,
) -> Result<Option<String>, String> {
    let shown_wrong: Vec<&String> = visible.iter().filter(|r| !want(r)).collect();
    let hidden_wrong: Vec<&String> = all
        .iter()
        .filter(|r| want(r) && !visible.contains(r))
        .collect();
    if !shown_wrong.is_empty() || !hidden_wrong.is_empty() {
        return Err(format!(
            "shown but should be hidden: {shown_wrong:?}; hidden but should be shown: {hidden_wrong:?}"
        ));
    }
    if visible.is_empty() && !all.is_empty() {
        return Ok(Some("hides everything".into()));
    }
    if !all.is_empty() && visible.len() == all.len() {
        return Ok(Some("changes nothing".into()));
    }
    Ok(None)
}
