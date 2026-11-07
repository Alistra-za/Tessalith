//! Named Rust checks for the UI driver's `check` step. A filter's predicate is Rust (a script can't hold one), so it
//! lives here and a script runs it by name.

use engine::sets::fold_id;
use sdc_gui::model::bottom::{self, At};
use sdc_gui::model::sets::{self, SetView};
use sdc_gui::model::sets_filter::SetsFilter;
use sdc_gui::model::{cm_json, AppModel};

/// `Ok(None)`: holds. `Ok(Some(note))`: not decided, with the reason. `Err`: broken.
pub type Oracle = fn(&AppModel) -> Result<Option<String>, String>;

pub const ORACLES: &[(&str, Oracle)] = &[
    ("sets_pills_match_their_counts", pills_match_counts),
    ("sets_not_ready_shows_exactly_problem_sets", not_ready_only),
    ("sets_ready_shows_exactly_clean_sets", ready_only),
    ("sets_no_json_shows_exactly_json_less_sets", no_json_only),
    ("sets_search_shows_exactly_name_matches", search_only),
    ("sets_shader_shows_exactly_that_shader", shader_only),
    ("sets_filter_shows_the_and_of_its_parts", all_parts),
    ("selected_set_kept_while_hidden", selected_kept_while_hidden),
];

// ---- the Sets column filter, one predicate per row of sets-column/goals.md "Filters and controls" ----

/// The parts of the Sets filter that are on, by the table's control names.
fn parts_on(f: &SetsFilter) -> Vec<&'static str> {
    [
        (!f.search.trim().is_empty(), "Search sets"),
        (f.attention, "Not ready"),
        (f.ready, "Ready"),
        (f.no_json, "No JSON"),
        (f.shader.is_some(), "Filter"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect()
}

/// Not ready: a Problems row that is not Info or Check, on the set itself or on one of its member textures.
fn not_ready(problems: &[bottom::Problem], v: &SetView) -> bool {
    problems.iter().any(|p| {
        !matches!(p.level, bottom::Level::Info | bottom::Level::Check)
            && match &p.at {
                At::Set { key, .. } => *key == v.key,
                At::Texture(t) => v.members.contains(t),
                _ => false,
            }
    })
}

/// No JSON: not a cm-JSON set, not a PBR Landscape set writing CS or PGPatcher JSON, not a PBR set in CS mode (the
/// non-Landscape branch, as the table lists Landscape on its own); an Advanced skin set always counts.
fn no_json(m: &AppModel, v: &SetView) -> bool {
    let entries = m.sets_doc.as_ref().map(|d| d.entries()).unwrap_or_default();
    let writes = cm_json::exports(&entries, &v.id)
        || if sets::is_pbr_landscape(v) {
            m.cs_records_written(v) || m.pgp_written(v)
        } else {
            sets::is_json_set(v, m.target)
        };
    v.skin || !writes
}

/// Search sets: the id, folded like set ids, contains the folded text; empty text limits nothing.
fn name_matches(text: &str, v: &SetView) -> bool {
    fold_id(&v.id).contains(&fold_id(text.trim()))
}

/// Filter ▾: the shader pick (chosen or auto) is the chosen shader; Any limits nothing.
fn shader_matches(f: &SetsFilter, v: &SetView) -> bool {
    f.shader
        .is_none_or(|s| v.shader.as_ref().map(|p| p.shader) == Some(s))
}

/// `filter_matches` over every set, wanting the keyed sets `want` takes (Exceptions row: a set with no key passes
/// only an empty filter, and a set made under this filter stays shown).
fn shows_exactly(m: &AppModel, want: impl Fn(&SetView) -> bool) -> Result<Option<String>, String> {
    let views = sets::views(m);
    let empty = parts_on(&m.sets_filter).is_empty();
    let made_here = m.new_sets_shown();
    let all: Vec<String> = views.iter().map(|v| v.id.clone()).collect();
    let visible: Vec<String> = m
        .visible_sets()
        .iter()
        .map(|&i| views[i].id.clone())
        .collect();
    let wanted: Vec<&str> = views
        .iter()
        .filter(|v| empty || made_here.contains(&v.id) || !v.key.is_empty() && want(v))
        .map(|v| v.id.as_str())
        .collect();
    filter_matches(&visible, &all, |id| wanted.contains(&id))
}

/// `part` is the only filter part on (else `Err`: the goal's check names that state), then `shows_exactly`.
fn only(
    m: &AppModel,
    part: &str,
    want: impl Fn(&SetView) -> bool,
) -> Result<Option<String>, String> {
    let on = parts_on(&m.sets_filter);
    if on != [part] {
        return Err(format!(
            "{part} is not the only Sets filter part on; on: {on:?}"
        ));
    }
    shows_exactly(m, want)
}

fn not_ready_only(m: &AppModel) -> Result<Option<String>, String> {
    let p = bottom::problems(m);
    only(m, "Not ready", |v| not_ready(&p, v))
}

fn ready_only(m: &AppModel) -> Result<Option<String>, String> {
    let p = bottom::problems(m);
    only(m, "Ready", |v| !not_ready(&p, v))
}

fn no_json_only(m: &AppModel) -> Result<Option<String>, String> {
    only(m, "No JSON", |v| no_json(m, v))
}

fn search_only(m: &AppModel) -> Result<Option<String>, String> {
    let text = m.sets_filter.search.clone();
    only(m, "Search sets", |v| name_matches(&text, v))
}

fn shader_only(m: &AppModel) -> Result<Option<String>, String> {
    let f = m.sets_filter.clone();
    only(m, "Filter", |v| shader_matches(&f, v))
}

/// All parts: the AND of each part's set (with nothing on, every set).
fn all_parts(m: &AppModel) -> Result<Option<String>, String> {
    let f = m.sets_filter.clone();
    let p = bottom::problems(m);
    shows_exactly(m, |v| {
        name_matches(&f.search, v)
            && (!f.attention || not_ready(&p, v))
            && (!f.ready || !not_ready(&p, v))
            && (!f.no_json || no_json(m, v))
            && shader_matches(&f, v)
    })
}

/// Exactly one set is selected and the Sets filter hides it, and the model reports it hidden (the column's
/// "{id} is hidden by the Sets filter" strip). The set shown again is a note, not a pass: the goal is the hidden state.
fn selected_kept_while_hidden(m: &AppModel) -> Result<Option<String>, String> {
    let sel = m.selected_sets();
    let [key] = sel.as_slice() else {
        return Err(format!("{} sets selected, not one", sel.len()));
    };
    let views = sets::views(m);
    let Some(i) = views.iter().position(|v| v.key == *key) else {
        return Err(format!("the selected set {key} has no card"));
    };
    let id = &views[i].id;
    let hidden = !m.visible_sets().contains(&i);
    match (hidden, m.selected_set_hidden_by_filter()) {
        (true, Some(h)) if h == *id => Ok(None),
        (true, other) => Err(format!(
            "{id} is hidden but the model reports {other:?} as the hidden selected set"
        )),
        (false, _) => Ok(Some(format!("not checked: {id} is shown"))),
    }
}

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
