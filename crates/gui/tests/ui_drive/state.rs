//! The model summary the UI driver writes beside every frame (`NN.state.json`) and the `expect` step that reads it.
//! Every field comes from the public model API; see the header of `main.rs` for the list.

use sdc_gui::model::items::TextureItem;
use sdc_gui::model::sets::{self, SlotFill};
use sdc_gui::model::{bottom, AppModel, Selection};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// A set's id from its engine key (a key with no view is shown as it is).
fn id_of<'a>(by_key: &'a HashMap<&str, &str>, key: &'a str) -> &'a str {
    by_key.get(key).copied().unwrap_or(key)
}

/// The output path relative to the export folder, with `/` separators.
fn output_of(m: &AppModel, i: &TextureItem) -> Value {
    let Some(job) = &i.job else {
        return Value::Null;
    };
    let root = m.export_root();
    let p = job.output.strip_prefix(&root).unwrap_or(&job.output);
    json!(p.to_string_lossy().replace('\\', "/"))
}

pub fn state_json(m: &AppModel) -> Value {
    let views = sets::views(m);
    let by_key: HashMap<&str, &str> = views
        .iter()
        .map(|v| (v.key.as_str(), v.id.as_str()))
        .collect();
    let visible_sets = m.visible_sets();
    let visible_items = m.visible_items();
    let problems = bottom::problems(m);
    let selected = m.selected_sets();

    let mut set_map = Map::new();
    for (n, v) in views.iter().enumerate() {
        let slots: Map<String, Value> = v
            .slots
            .iter()
            .map(|(k, f)| {
                let fill = match f {
                    SlotFill::Empty => "empty".to_string(),
                    SlotFill::Own { item } => format!("own:{item}"),
                    SlotFill::Borrowed { from, .. } => format!("borrowed:{from}"),
                };
                (format!("{k:?}"), json!(fill))
            })
            .collect();
        set_map.insert(
            v.id.clone(),
            json!({
                "key": v.key,
                "shader": v.shader.as_ref().map(|p| p.shader.label()),
                "visible": visible_sets.contains(&n),
                "selected": selected.contains(&v.key),
                "members": v.members.len(),
                "slots": slots,
            }),
        );
    }

    let mut item_map = Map::new();
    for (n, i) in m.items.iter().enumerate() {
        item_map.insert(
            i.key.clone(),
            json!({
                "name": i.name,
                "set": i.set_key.as_deref().map(|k| id_of(&by_key, k)),
                "status": format!("{:?}", i.status),
                "visible": visible_items.contains(&n),
                "output": output_of(m, i),
            }),
        );
    }

    let sf = &m.sets_filter;
    let tf = &m.filter;
    json!({
        "dialog": m.dialog.as_ref().map(|d| {
            let s = format!("{d:?}");
            s.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>()
        }),
        "review_open": m.review.is_some(),
        "selection": match &m.selection {
            Selection::None => "none".to_string(),
            Selection::Item(k) => format!("item:{k}"),
            Selection::Set(k) => format!("set:{}", id_of(&by_key, k)),
        },
        "selected_sets": selected.iter().map(|k| id_of(&by_key, k)).collect::<Vec<_>>(),
        "expanded": m.expanded,
        "dirty": m.is_dirty(),
        "counts": {
            "items": m.items.len(),
            "visible_items": visible_items.len(),
            "sets": views.len(),
            "visible_sets": visible_sets.len(),
            "problems": problems.len(),
        },
        "sets_filter": {
            "search": sf.search,
            "attention": sf.attention,
            "ready": sf.ready,
            "no_json": sf.no_json,
            "shader": sf.shader.map(|s| s.label()),
        },
        "textures_filter": {
            "search": m.search,
            "needs_decision": tf.needs_decision,
            "warnings": tf.warnings,
            "failed": tf.failed,
            "not_in_set": tf.not_in_set,
            "subtype": tf.subtype,
            "format": tf.format,
            "set": tf.set.name(),
        },
        "sets": set_map,
        "items": item_map,
        "problems": problems.iter().map(|p| json!({
            "level": format!("{:?}", p.level),
            "at": format!("{:?}", p.at),
            "text": p.text,
        })).collect::<Vec<_>>(),
    })
}

/// Checks one fact of `state` (a JSON Pointer; `~1` is a `/` inside a key). Exactly one of `equals`, `contains`
/// (array element, substring or object key) or `absent` (no such pointer) is given.
pub fn eval_expect(
    state: &Value,
    fact: &str,
    equals: Option<&Value>,
    contains: Option<&Value>,
    absent: bool,
) -> Result<(), String> {
    if usize::from(equals.is_some()) + usize::from(contains.is_some()) + usize::from(absent) != 1 {
        return Err(format!(
            "expect {fact}: give exactly one of equals, contains, absent"
        ));
    }
    let found = state.pointer(fact);
    let (ok, wanted) = match (equals, contains) {
        (Some(w), _) => (found == Some(w), format!("equal to {w}")),
        (_, Some(w)) => (
            match (found, w) {
                (Some(Value::Array(a)), w) => a.contains(w),
                (Some(Value::String(s)), Value::String(w)) => s.contains(w.as_str()),
                (Some(Value::Object(o)), Value::String(w)) => o.contains_key(w),
                _ => false,
            },
            format!("containing {w}"),
        ),
        _ => (found.is_none(), "nothing there".to_string()),
    };
    if ok {
        return Ok(());
    }
    let actual = found.map_or("nothing".to_string(), |v| v.to_string());
    Err(format!("expect {fact}: wanted {wanted}, found {actual}"))
}
