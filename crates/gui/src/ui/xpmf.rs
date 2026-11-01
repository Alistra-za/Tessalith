//! The set editor's XPMF profile card (Note GI, design X1, X3, X7, X9) on Landscape and PBR Landscape sets: the
//! `Write XPMF profile` toggle, then one row per `xpmf_json::FIELDS` key, laid out as the CS PBR card's rows. Drawing
//! only: every change is a model call made after drawing.
use super::status::AMBER;
use crate::model::labels::readable;
use crate::model::sets::SetView;
use crate::model::xpmf_json::{card_values, Field, FieldKind, FIELDS};
use crate::model::AppModel;
use eframe::egui;
use engine::sets::same_id;
use engine::shader::Shader;
use serde_json::{json, Map, Value};

pub const CARD: &str = "XPMF profile";
pub const TOGGLE: &str = "Write XPMF profile";
pub const NOT_PREVIEWED: &str = "Not shown in the preview";
const TOGGLE_HOVER: &str =
    "Writes SKSE\\Plugins\\XPMF\\tessalith_<set>.json on Export. A row left at its greyed \
                            default is written as Tessalith fills it, or left to XPMF.";
const RESET_HOVER: &str = "Back to the default";
/// The disabled card's hover when the model gives no reason.
const READ_ONLY: &str = "sdc-project.toml is read-only";

/// ui-rules 8.4: a repeated control gets a name with its key (call after `ui.add`).
fn named(r: egui::Response, kind: egui::WidgetType, name: String) -> egui::Response {
    let on = r.enabled();
    r.widget_info(|| egui::WidgetInfo::labeled(kind, on, name.clone()));
    r
}

enum Act {
    On(bool),
    Value(&'static str, Option<Value>),
}

/// The card's sets: Landscape and PBR Landscape, on either target.
pub fn shows(v: &SetView) -> bool {
    v.shader
        .as_ref()
        .is_some_and(|p| matches!(p.shader, Shader::Landscape | Shader::PbrLandscape))
}

pub fn card(ui: &mut egui::Ui, model: &mut AppModel, v: &SetView) {
    if !shows(v) {
        return;
    }
    let mut acts = vec![];
    let m: &AppModel = model;
    let block = m.sets_doc.as_ref().and_then(|d| {
        d.entries()
            .iter()
            .find(|e| same_id(&e.id, &v.id))
            .and_then(|e| e.xpmf.clone())
    });
    // The model's own check, so the card is never enabled while its edits are dropped.
    let editable = m.sets_editable();
    let why = m
        .sets_read_only_reason()
        .or_else(|| m.banner.clone())
        .unwrap_or_else(|| READ_ONLY.into());
    let head = egui::RichText::new(if block.is_some() { "on" } else { "off" }).weak();
    super::group::group(ui, ("xpmf", v.id.as_str()), CARD, Some(head), true, |ui| {
        ui.add_enabled_ui(editable, |ui| {
            let mut on = block.is_some();
            if ui
                .checkbox(&mut on, TOGGLE)
                .on_hover_text(TOGGLE_HOVER)
                .changed()
            {
                acts.push(Act::On(on));
            }
            let Some(b) = &block else {
                return;
            };
            let c = card_values(m, &v.id);
            if let Err(e) = &c.profile {
                ui.colored_label(AMBER, e);
            }
            let none = Map::new();
            let defaults = c.defaults.as_ref().unwrap_or(&none);
            // X9: the effective pbr, the block's over the default.
            let pbr = b.get("pbr").or(defaults.get("pbr")) == Some(&json!(true));
            for f in FIELDS {
                if f.key == "specularMult" && pbr {
                    continue;
                }
                row(ui, m, v, f, b.get(f.key), defaults.get(f.key), &mut acts);
            }
        })
        .response
        .on_disabled_hover_text(&why);
    });
    for a in acts {
        match a {
            Act::On(on) => model.set_xpmf_on(&v.id, on),
            Act::Value(k, x) => model.set_xpmf_value(&v.id, k, x),
        }
    }
}

/// A readable label; the key is on hover.
fn label_of(key: &str) -> String {
    match key {
        "editorIds" => "Editor IDs".into(),
        "excludeEditorIds" => "Exclude editor IDs".into(),
        "pbr" => "PBR".into(),
        "lodIdentifier" => "LOD identifier".into(),
        "lodMaterial" => "LOD material".into(),
        "lodMaterialHD" => "LOD material HD".into(),
        k => readable(k),
    }
}

/// The greyed default: Tessalith's value (`dflt`, from `defaults`) where it fills the key, else XPMF's own.
fn default_text(f: &Field, dflt: Option<&Value>) -> String {
    match dflt {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => (if *b { "on" } else { "off" }).into(),
        Some(Value::Array(a)) if a.is_empty() => "none".into(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| x.as_str().unwrap_or_default())
            .collect::<Vec<_>>()
            .join(", "),
        _ => match f.default {
            Some("true") => "on".into(),
            Some("false") => "off".into(),
            Some(d) => d.into(),
            None => "not written".into(),
        },
    }
}

/// Whether a stored value has the type the row edits (a hand-edited TOML may hold anything; the export's schema check
/// refuses it with a note).
fn typed(kind: FieldKind, v: &Value) -> bool {
    match kind {
        FieldKind::Bool => v.is_boolean(),
        FieldKind::Number { .. } => v.is_number(),
        FieldKind::Text | FieldKind::Texture => v.is_string(),
        FieldKind::Patterns => v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
    }
}

fn row(
    ui: &mut egui::Ui,
    m: &AppModel,
    v: &SetView,
    f: &'static Field,
    cur: Option<&Value>,
    dflt: Option<&Value>,
    acts: &mut Vec<Act>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label_of(f.key)).on_hover_ui(|ui| {
            ui.weak(format!("JSON key: {}", f.key));
            if !f.preview {
                ui.label(NOT_PREVIEWED);
            }
        });
        match cur {
            Some(c) if !typed(f.kind, c) => {
                ui.colored_label(AMBER, "wrong type in sdc-project.toml; not written");
            }
            _ => match f.kind {
                FieldKind::Bool => bool_value(ui, f, cur, dflt, acts),
                FieldKind::Number { .. } => number_value(ui, f, cur, dflt, acts),
                FieldKind::Text | FieldKind::Texture => text_value(ui, v, f, cur, dflt, acts),
                FieldKind::Patterns => patterns_value(ui, m, v, f, cur, dflt, acts),
            },
        }
        if cur.is_some() {
            let r = ui.small_button("Reset").on_hover_text(RESET_HOVER);
            let name = format!("Reset {}", f.key);
            if named(r, egui::WidgetType::Button, name).clicked() {
                acts.push(Act::Value(f.key, None));
            }
        }
    });
}

fn bool_value(
    ui: &mut egui::Ui,
    f: &'static Field,
    cur: Option<&Value>,
    dflt: Option<&Value>,
    acts: &mut Vec<Act>,
) {
    let fallback = dflt
        .and_then(Value::as_bool)
        .unwrap_or(f.default == Some("true"));
    let mut on = cur.and_then(Value::as_bool).unwrap_or(fallback);
    let text = if cur.is_some() {
        egui::RichText::new("")
    } else {
        egui::RichText::new("default").weak()
    };
    let r = ui.checkbox(&mut on, text);
    let (enabled, name) = (r.enabled(), format!("{} enabled", label_of(f.key)));
    r.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, on, name.clone())
    });
    if r.changed() {
        acts.push(Act::Value(f.key, Some(json!(on))));
    }
}

/// The DragValue range: `None` for a key with no finite bound (`falloffScale`, `falloffBias`), so the drag is
/// unbounded. An exclusive minimum is stored at least 0.01 above it (the drag step); no bound is infinite.
fn range(kind: FieldKind) -> Option<std::ops::RangeInclusive<f64>> {
    let FieldKind::Number {
        min,
        max,
        exclusive_min,
    } = kind
    else {
        return None;
    };
    let lo = min
        .is_finite()
        .then_some(if exclusive_min { min + 0.01 } else { min });
    match (lo, max) {
        (None, None) => None,
        (lo, hi) => Some(lo.unwrap_or(f64::MIN)..=hi.unwrap_or(f64::MAX)),
    }
}

fn number_value(
    ui: &mut egui::Ui,
    f: &'static Field,
    cur: Option<&Value>,
    dflt: Option<&Value>,
    acts: &mut Vec<Act>,
) {
    let r = range(f.kind);
    let Some(mut x) = cur.and_then(Value::as_f64) else {
        ui.weak(format!("default: {}", default_text(f, dflt)));
        let b = ui
            .small_button("Override")
            .on_hover_text("Write a value of your own");
        if named(b, egui::WidgetType::Button, format!("Override {}", f.key)).clicked() {
            // XPMF's numeric default where it has one, else 1 (inside the range).
            let start: f64 = f.default.and_then(|d| d.parse().ok()).unwrap_or(1.0);
            let start = r
                .clone()
                .map_or(start, |r| start.clamp(*r.start(), *r.end()));
            acts.push(Act::Value(f.key, Some(json!(start))));
        }
        return;
    };
    // About 256 drag steps over a bounded range; a stored value outside the range stays until the user edits it
    // (typed values always clamp, egui `clamp_value_to_range`).
    let speed = match &r {
        Some(r) if r.end() - r.start() < 1e6 => (r.end() - r.start()) / 256.0,
        _ => 0.01,
    };
    let mut d = egui::DragValue::new(&mut x)
        .speed(speed)
        .clamp_existing_to_range(false);
    if let Some(r) = r.clone() {
        d = d.range(r);
    }
    if ui.add(d).changed() {
        // An AccessKit SetValue skips egui's clamp (`clamp_existing_to_range(false)`), so clamp what is written.
        let x = r.map_or(x, |r| x.clamp(*r.start(), *r.end()));
        acts.push(Act::Value(f.key, Some(json!(x))));
    }
}

/// A text or texture path, committed on Enter or focus loss (Esc cancels). An empty `name` (R13) or text is a reset;
/// an empty texture over one Tessalith fills is stored as `""`, the game's own (X5).
fn text_value(
    ui: &mut egui::Ui,
    v: &SetView,
    f: &'static Field,
    cur: Option<&Value>,
    dflt: Option<&Value>,
    acts: &mut Vec<Act>,
) {
    let id = egui::Id::new(("xpmf-text", f.key, v.id.as_str()));
    let mut text = ui
        .data(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| cur.and_then(Value::as_str).unwrap_or_default().to_string());
    let hint = if f.kind == FieldKind::Texture && cur == Some(&json!("")) {
        "the game's".to_string()
    } else if dflt.is_some() || f.default.is_some() {
        default_text(f, dflt)
    } else {
        String::new()
    };
    let width = if f.kind == FieldKind::Texture {
        260.0
    } else {
        180.0
    };
    let r = ui.add(
        egui::TextEdit::singleline(&mut text)
            .hint_text(hint)
            .desired_width(width),
    );
    // ui-rules §3.4: a clipped value stays readable on hover.
    let r = if text.is_empty() {
        r
    } else {
        r.on_hover_ui(|ui| {
            ui.label(&text);
        })
    };
    if r.changed() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    if r.lost_focus() {
        // Review I1: only text changed during this focus is an edit; tabbing through an unset row writes nothing.
        let edited = ui.data_mut(|d| d.remove_temp::<String>(id)).is_some();
        let t = text.trim();
        if edited
            && !ui.input(|i| i.key_pressed(egui::Key::Escape))
            && cur.and_then(Value::as_str) != Some(t)
        {
            let game = f.kind == FieldKind::Texture && dflt.is_some();
            let new = match t {
                "" if game => Some(json!("")),
                "" => None,
                t => Some(json!(t)),
            };
            if new.as_ref() != cur {
                acts.push(Act::Value(f.key, new));
            }
        }
    }
}

/// X3: a list of EditorID patterns. The MATO lists take the scan's single-pass material objects as well as typed
/// wildcards; the `*Skip` lists are over STAT EditorIDs, which the scan does not list, so they take typed ones only.
/// An emptied list is a reset (R13 for `editorIds`).
fn patterns_value(
    ui: &mut egui::Ui,
    m: &AppModel,
    v: &SetView,
    f: &'static Field,
    cur: Option<&Value>,
    dflt: Option<&Value>,
    acts: &mut Vec<Act>,
) {
    let strs = |x: Option<&Value>| -> Vec<String> {
        x.and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let set = |l: Vec<String>| Act::Value(f.key, (!l.is_empty()).then(|| json!(l)));
    // An edit starts from the list shown: the block's, else the greyed default.
    let list = if cur.is_some() { strs(cur) } else { strs(dflt) };
    if cur.is_none() {
        ui.weak(format!("default: {}", default_text(f, dflt)));
    } else {
        for (i, p) in list.iter().enumerate() {
            ui.monospace(p);
            let r = ui.small_button("×").on_hover_text("Remove this pattern");
            let name = format!("Remove pattern {p} from {}", f.key);
            if named(r, egui::WidgetType::Button, name).clicked() {
                let mut l = list.clone();
                l.remove(i);
                acts.push(set(l));
            }
        }
    }
    let id = egui::Id::new(("xpmf-pattern", f.key, v.id.as_str()));
    let mut text: String = ui.data(|d| d.get_temp(id).unwrap_or_default());
    let r = ui.add(
        egui::TextEdit::singleline(&mut text)
            .hint_text("EditorID or *wildcard*")
            .desired_width(140.0),
    );
    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let add = ui.small_button("Add");
    let add = named(
        add,
        egui::WidgetType::Button,
        format!("Add pattern to {}", f.key),
    );
    let t = text.trim().to_string();
    if (add.clicked() || enter) && !t.is_empty() && !list.contains(&t) {
        let mut l = list.clone();
        l.push(t);
        acts.push(set(l));
        text.clear();
    }
    let picks = if matches!(f.key, "editorIds" | "excludeEditorIds") && !text.trim().is_empty() {
        cached_picks(ui, id, m, &text)
    } else {
        Default::default()
    };
    ui.data_mut(|d| d.insert_temp(id, text));
    let new: Vec<_> = picks.iter().filter(|(e, _)| !list.contains(e)).collect();
    if !new.is_empty() {
        ui.end_row();
        ui.weak("From the scan:");
        for (e, label) in new {
            let r = ui.small_button(format!("+ {label}"));
            if named(r, egui::WidgetType::Button, format!("Add {e} to {}", f.key)).clicked() {
                let mut l = list.clone();
                l.push(e.clone());
                acts.push(set(l));
            }
        }
    }
}

/// ui-rules 9: the picks are rebuilt only when the typed text or the scan (its cached `Arc`) changes, not by
/// lower-casing every scanned EDID each frame.
fn cached_picks(
    ui: &egui::Ui,
    id: egui::Id,
    m: &AppModel,
    text: &str,
) -> std::sync::Arc<Vec<(String, String)>> {
    type Cached = (String, usize, std::sync::Arc<Vec<(String, String)>>);
    let Ok(r) = m.scan_records() else {
        return Default::default();
    };
    let scan = std::sync::Arc::as_ptr(&r) as usize;
    let key = id.with("picks");
    if let Some((t, s, p)) = ui.data(|d| d.get_temp::<Cached>(key)) {
        if t == text && s == scan {
            return p;
        }
    }
    let p = std::sync::Arc::new(picks(&r, text));
    ui.data_mut(|d| d.insert_temp::<Cached>(key, (text.to_string(), scan, p.clone())));
    p
}

/// The XPMF picker's rows for the typed text: none until something is typed, single-pass material objects only.
fn picks(r: &crate::model::cs_pbr::ScanRecords, typed: &str) -> Vec<(String, String)> {
    if typed.trim().is_empty() {
        return vec![];
    }
    super::cs_pbr::mato_picks(r, typed, true)
}

#[cfg(test)]
mod tests {
    use super::picks;
    use crate::model::cs_pbr::ScanRecords;
    use modscan::plugin_store::WinRec;

    fn rec(edid: &str, flags: Option<u32>) -> WinRec {
        WinRec {
            formid: 1,
            edid: edid.into(),
            plugin: "Test.esp".into(),
            mato_flags: flags,
        }
    }

    /// Review I3: nothing until something is typed; then single-pass material objects only.
    #[test]
    fn picker_needs_a_query_and_keeps_single_pass() {
        let r = ScanRecords {
            txst: vec![],
            mato: vec![
                rec("SnowMat1P", Some(1)),
                rec("SnowMatMulti", Some(0)),
                rec("SnowMatOld", None),
                rec("AshMat1P", Some(3)),
            ],
        };
        assert!(picks(&r, "").is_empty(), "an empty query lists nothing");
        assert!(picks(&r, "  ").is_empty());
        let got: Vec<String> = picks(&r, "snow").into_iter().map(|(e, _)| e).collect();
        assert_eq!(got, ["SnowMat1P"]);
        let got: Vec<String> = picks(&r, "MAT1p").into_iter().map(|(e, _)| e).collect();
        assert_eq!(got, ["SnowMat1P", "AshMat1P"]);
    }
}
