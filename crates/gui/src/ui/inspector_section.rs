//! The texture panel's folded "Where these settings come from" section (Note JI3): the preset inspector the Advanced
//! drawer used to hold. Drawing only; it reads the model and edits nothing.
use super::group::{collapsing_group, GroupHead};
use crate::model::fields::provenance_text;
use crate::model::inspector::{inspector_rows, InspectorRow};
use crate::model::AppModel;
use eframe::egui;
use serde_json::Value;

/// Slack kept off the inspector grid's width for the scroll bar and the grid's own cell padding (rule 3.7).
const GRID_MARGIN: f32 = 24.0;
/// The narrowest an inspector column gets.
const MIN_COL: f32 = 60.0;

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// A field's row text: the bulk panel's label, else the key as words ("alpha.compare" is "Alpha compare").
fn field_text(field: &str) -> String {
    let label = crate::model::bulk::field_label(field);
    if label != field {
        label.to_string()
    } else {
        crate::model::labels::readable(&field.replace('.', "_"))
    }
}

/// The "only what differs" tick lives in egui memory: one value for the app, kept across selections (Note JI3).
fn only_differences_id() -> egui::Id {
    egui::Id::unique("inspector-only-differences")
}

/// Folded by default; the body draws only while open, so a closed section costs one header. Returns whether the header
/// was toggled (the caller writes the open state).
pub(super) fn section(
    ui: &mut egui::Ui,
    model: &AppModel,
    key: &str,
    head: &GroupHead<'_>,
    open: bool,
    instant: bool,
) -> bool {
    collapsing_group(
        ui,
        crate::model::panel_groups::PanelGroup::Sources.id(),
        head,
        open,
        instant,
        |ui| body(ui, model, key),
    )
    .toggled
}

fn body(ui: &mut egui::Ui, model: &AppModel, key: &str) {
    let (Some(job), Some(session)) = (
        model.item(key).and_then(|i| i.job.as_ref()),
        model.session.as_ref(),
    ) else {
        ui.weak("This texture has no resolved settings yet.");
        return;
    };
    // D10: the section names the texture first (A18).
    ui.label(
        egui::RichText::new(format!(
            "How {}'s settings resolve",
            key.rsplit('/').next().unwrap_or(key)
        ))
        .strong(),
    );
    for (file, reason) in &model.skipped_user_presets {
        ui.label(
            egui::RichText::new(format!("Skipped: {file} — {reason}"))
                .small()
                .color(ui.visuals().warn_fg_color),
        );
    }
    let mut only = ui.data(|d| d.get_temp::<bool>(only_differences_id()).unwrap_or(false));
    if ui
        .checkbox(
            &mut only,
            format!("Show only what differs from {}", engine::presets::BASE_ID),
        )
        .changed()
    {
        ui.data_mut(|d| d.insert_temp(only_differences_id(), only));
    }
    let rows: Vec<InspectorRow> = inspector_rows(job, session.engine().registry())
        .into_iter()
        .filter(|r| !only || r.differs_from_base)
        .collect();
    if rows.is_empty() {
        ui.weak("Nothing differs from the base preset.");
        return;
    }
    // Three columns share the panel's width (rule 3.7: a margin for the scroll bar).
    let col = ((ui.available_width() - 2.0 * ui.spacing().item_spacing.x - GRID_MARGIN) / 3.0)
        .max(MIN_COL);
    egui::Grid::new("inspector")
        .num_columns(3)
        .striped(true)
        .max_col_width(col)
        .show(ui, |ui| {
            for r in rows {
                let text = |s: String| {
                    if r.differs_from_base {
                        egui::RichText::new(s).strong()
                    } else {
                        egui::RichText::new(s)
                    }
                };
                let shown = field_text(&r.field);
                // One tooltip: the name and its key (the automatic elided-text one is off for this label).
                let name = format!("{shown} ({})", r.field);
                let key_label = ui
                    .add(
                        egui::Label::new(text(shown))
                            .truncate()
                            .show_tooltip_when_elided(false),
                    )
                    .on_hover_text(&name);
                key_label.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
                ui.add(egui::Label::new(text(value_text(&r.value))).truncate());
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(provenance_text(&r.provenance))
                            .small()
                            .weak(),
                    )
                    .truncate(),
                );
                ui.end_row();
            }
        });
}
