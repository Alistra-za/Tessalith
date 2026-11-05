//! The Advanced skin card (Task 10b, DECISIONS AG79-AG86): Community Shaders `SkinProfile` values of a skin set.
//! Drawing only; edits go through `AppModel::set_skin_value`.
use crate::model::labels::readable;
use crate::model::sets::SetView;
use crate::model::skin::{Kind, SkinField, FIELDS, GROUPS};
use crate::model::AppModel;
use eframe::egui;
use engine::config::Subtype;
use serde_json::{json, Value};

pub const TITLE: &str = "Advanced skin (Community Shaders)";
const NOTE: &str = "Written beside the diffuse as a .csskin.json on export";

/// The set's skin members (`_rfaos`, `_sk`, `_wet`): CS binds them through its JSON, so they are listed, not edited.
fn members(model: &AppModel, v: &SetView) -> Vec<String> {
    model
        .items
        .iter()
        .filter(|i| v.members.contains(&i.key))
        .filter(|i| {
            matches!(
                i.job.as_ref().map(|j| j.profile.subtype),
                Some(Subtype::SkinRfaos | Subtype::SkinSk | Subtype::SkinWet | Subtype::SkinDetail)
            )
        })
        .map(|i| i.name.clone())
        .collect()
}

pub fn card(ui: &mut egui::Ui, model: &mut AppModel, v: &SetView) {
    let own = model.skin_values(&v.id);
    let off = model.sets_read_only_reason();
    let mut edits: Vec<(&str, Option<Value>)> = vec![];
    ui.add_space(6.0);
    egui::CollapsingHeader::new(egui::RichText::new(TITLE).strong())
        .id_salt(("skin-card", &v.id))
        .default_open(true)
        .show(ui, |ui| {
            ui.weak(NOTE);
            let m = members(model, v);
            ui.weak(if m.is_empty() {
                "Skin textures: none yet (_rfaos, _sk, _wet)".to_string()
            } else {
                format!("Skin textures: {}", m.join(", "))
            });
            let r = ui.add_enabled_ui(off.is_none(), |ui| {
                for g in GROUPS {
                    ui.label(egui::RichText::new(g).strong());
                    egui::Grid::new(("skin-grid", &v.id, g))
                        .num_columns(3)
                        .show(ui, |ui| {
                            for f in FIELDS.iter().filter(|f| f.group == g) {
                                field_row(ui, f, own.get(f.key), &mut edits);
                                ui.end_row();
                            }
                        });
                }
            });
            // Audit lead 24: a greyed card says why.
            if let Some(why) = &off {
                r.response.on_disabled_hover_text(why);
            }
        });
    for (k, v2) in edits {
        model.set_skin_value(&v.id, k, v2);
    }
}

/// A8: `label · value (or the weak default) · set / ↺`, one grid row; the key is the label's hover.
fn field_row(
    ui: &mut egui::Ui,
    f: &SkinField,
    cur: Option<&Value>,
    edits: &mut Vec<(&'static str, Option<Value>)>,
) {
    let name = readable(f.key);
    let label = ui.label(&name).on_hover_text(f.key);
    let set_name = format!("Set {name}");
    let reset_name = format!("Reset {name}");
    match (f.kind, cur) {
        (Kind::Num { default, lo, hi }, Some(c)) => {
            let mut x = c.as_f64().unwrap_or(default);
            let r = ui
                .add(
                    egui::DragValue::new(&mut x)
                        .range(lo..=hi)
                        .clamp_existing_to_range(false) // shortcut: Note FO3, an AccessKit SetValue skips the range; clamp what is written if that matters
                        .speed(0.01)
                        .max_decimals(4),
                )
                .labelled_by(label.id);
            if r.changed() && c.as_f64() != Some(x) {
                edits.push((f.key, Some(json!(x))));
            }
        }
        (Kind::Flag { default }, Some(c)) => {
            let mut on = c.as_bool().unwrap_or(default);
            let r = ui.checkbox(&mut on, "");
            let en = ui.is_enabled();
            r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, en, on, &name));
            if r.changed() {
                edits.push((f.key, Some(json!(on))));
            }
        }
        (kind, None) => {
            let (value, hint) = match kind {
                Kind::Num { default, .. } => (json!(default), format!("default {default:?}")),
                Kind::Flag { default } => (json!(default), format!("default {default}")),
            };
            ui.weak(hint);
            let b = ui.small_button("set");
            let en = ui.is_enabled();
            b.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, en, &set_name));
            if b.clicked() {
                edits.push((f.key, Some(value)));
            }
            return;
        }
    }
    let b = ui.small_button("↺").on_hover_text("Back to not set");
    let en = ui.is_enabled();
    b.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, en, &reset_name));
    if b.clicked() {
        edits.push((f.key, None));
    }
}
