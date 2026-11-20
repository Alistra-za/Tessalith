//! The set editor's "Also export" tab (Note JI2): this set's Skip tick, then the project's "Also export as" ticks and
//! the `[export.legacy]` recipe. Drawing only: every change is a model call, made once the tab is drawn.
use super::header::ALSO_ITEMS;
use super::number_input::held_value;
use crate::model::also::{self, EXPORT_RUNNING};
use crate::model::sets::SetView;
use crate::model::AppModel;
use eframe::egui;
use engine::derive::recipe::LegacyParams;
use engine::presets::AlsoTarget;

/// The Skip tick's hover while both project ticks are off (correction 5).
const SKIP_OFF: &str = "Turn on Also export as Vanilla or as Complex material below first";
/// The recipe's hover while both project ticks are off (spec 2.3).
const RECIPE_OFF: &str = "Turn on Also export as Vanilla or as Complex material to use these";
/// The recipe's scope, under its boxes and never greyed with them (Note JK1).
const RECIPE_SCOPE: &str =
    "Saved in this project's sdc.toml; used when Vanilla or Complex material is ticked";

enum Act {
    Skip(bool),
    Tick(AlsoTarget, bool),
    Recipe(LegacyParams),
}

/// The tab's two blocks; applies its edits once they are drawn.
pub fn body(ui: &mut egui::Ui, model: &mut AppModel, v: &SetView) {
    let mut acts = vec![];
    this_set(ui, model, v, &mut acts);
    ui.add_space(8.0);
    all_sets(ui, model, &mut acts);
    for a in acts {
        match a {
            Act::Skip(on) => model.set_skip_also(&v.id, on),
            Act::Tick(t, on) => model.set_also_tick(t, on),
            Act::Recipe(p) => model.set_also_settings(|c| c.legacy = p),
        }
    }
}

/// Why an `[export]` edit is refused today: a running export, then a read-only `sdc.toml`.
fn edit_off(model: &AppModel) -> Option<String> {
    model
        .export
        .running
        .is_some()
        .then(|| EXPORT_RUNNING.to_string())
        .or_else(|| model.toml_read_only_reason())
}

/// "This set": the Skip tick on a PBR set (C2, `[export] skip`, shared with Review's "Skip these sets"); a sentence on
/// any other set, which Also export never reads (correction 5).
fn this_set(ui: &mut egui::Ui, model: &AppModel, v: &SetView, acts: &mut Vec<Act>) {
    ui.strong("This set");
    if !also::is_pbr_set(v) {
        if also::document_pbr_set(v) {
            ui.weak("This set has no texture files of its own for Also export to skip.");
        } else {
            ui.weak("Also export reads PBR sets only; this set is not one");
        }
        return;
    }
    let off = (!model.also_on())
        .then(|| SKIP_OFF.to_string())
        .or_else(|| edit_off(model));
    let mut now = model.also_skipped(&v.id);
    let r = ui
        .add_enabled(
            off.is_none(),
            egui::Checkbox::new(&mut now, "Skip vanilla/CM for this set"),
        )
        .on_hover_text("Leave this set out of the Vanilla and Complex material folders")
        .on_disabled_hover_text(off.as_deref().unwrap_or_default());
    if r.changed() {
        acts.push(Act::Skip(now));
    }
}

/// "All sets in this project": the ticks of Export to ▾ and Review, then the recipe, greyed while both ticks are off,
/// and the recipe's scope sentence.
fn all_sets(ui: &mut egui::Ui, model: &AppModel, acts: &mut Vec<Act>) {
    ui.strong("All sets in this project");
    let busy = model.export.running.is_some().then_some(EXPORT_RUNNING);
    let (v, c) = model.also_ticks();
    for ((t, text, hover), on) in ALSO_ITEMS.into_iter().zip([v, c]) {
        let why = also::tick_off(model, on, busy);
        let mut now = on;
        let r = ui.add_enabled(why.is_none(), egui::Checkbox::new(&mut now, text));
        // Rule 8.4: Export to ▾ shows the same text, and both can be on screen at once.
        let name = format!("All sets: {}", text.replacen("Also", "also", 1));
        let enabled = r.enabled();
        r.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, enabled, now, &name));
        let r = match &why {
            Some(w) => r.on_disabled_hover_text(w),
            None => r.on_hover_text(hover),
        };
        if r.changed() {
            acts.push(Act::Tick(t, now));
        }
    }
    ui.add_space(4.0);
    recipe(ui, model, acts);
}

/// The `[export.legacy]` recipe (owner answer 5, C4): one number box per value, ranges as `LegacyParams::check`. P17:
/// Keep metal brightness and its strength are only in Review (with the CM tick), and Dielectric env is one box (0 = off),
/// not a box plus a checkbox.
fn recipe(ui: &mut egui::Ui, model: &AppModel, acts: &mut Vec<Act>) {
    let off = (!model.also_on())
        .then(|| RECIPE_OFF.to_string())
        .or_else(|| edit_off(model));
    let now = model.also_config().legacy;
    let mut p = now;
    let rows: [(&mut f64, &str, &str, f64, f64, &str); 9] = [
        (
            &mut p.brightness,
            "Diffuse brightness",
            "brightness",
            0.0,
            1.5,
            "The vanilla diffuse's brightness factor",
        ),
        (
            &mut p.ao_strength,
            "AO strength",
            "ao_strength",
            0.0,
            1.0,
            "How much AO darkens an object's diffuse",
        ),
        (
            &mut p.ao_strength_landscape,
            "AO strength (landscape)",
            "ao_strength_landscape",
            0.0,
            1.0,
            "How much AO darkens a landscape diffuse",
        ),
        (
            &mut p.curves_opacity,
            "Curves opacity",
            "curves_opacity",
            0.0,
            1.0,
            "Opacity of the curves soft light and the cavity multiply",
        ),
        (
            &mut p.metal_darken,
            "Metal darkening",
            "metal_darken",
            0.0,
            1.0,
            "The vanilla diffuse's factor at full metal",
        ),
        (
            &mut p.spec_scale,
            "Specular scale",
            "spec_scale",
            0.0,
            2.0,
            "Scale of the specular in the _n alpha",
        ),
        (
            &mut p.spec_metal_boost,
            "Specular boost at full metal",
            "spec_metal_boost",
            1.0,
            3.0,
            "Boost of the _n alpha at full metal",
        ),
        (
            &mut p.landscape_spec,
            "Landscape spec factor",
            "landscape_spec",
            0.0,
            5.0,
            "Landscape _n alpha factor, applied before saturation",
        ),
        (
            &mut p.cm_dielectric_env,
            "Dielectric env from gloss",
            "cm_dielectric_env",
            0.0,
            1.0,
            "0 = off; the spec's option is 0.35",
        ),
    ];
    let mut settled = false;
    ui.add_enabled_ui(off.is_none(), |ui| {
        egui::Grid::new("also-recipe")
            .num_columns(2)
            .show(ui, |ui| {
                // The number box first: in a narrow column the text is cut, never the box.
                for (v, label, key, lo, hi, hover) in rows {
                    // A hand-written value outside the range is shown as it is: egui's clamp on draw would report a
                    // change and write it back with no edit (final review I1; as `cube_section::num`).
                    let (r, done) = held_value(ui, ("also-recipe", key), *v, |ui, x| {
                        ui.add(
                            egui::DragValue::new(x)
                                .speed(0.01)
                                .range(lo..=hi)
                                .clamp_existing_to_range(false)
                                .max_decimals(3)
                                .update_while_editing(false),
                        )
                    });
                    let shown = r.enabled();
                    r.widget_info(|| {
                        let mut i = egui::WidgetInfo::drag_value(shown, *v);
                        i.label = Some(label.to_string());
                        i
                    });
                    if let Some(x) = done {
                        *v = x;
                        settled = true;
                    }
                    // Built only while hovered (ui-rules 9.3).
                    let tip = |ui: &mut egui::Ui| {
                        ui.label(format!("{hover} (sdc.toml export.legacy.{key})"));
                    };
                    r.on_hover_ui(tip)
                        .on_disabled_hover_text(off.as_deref().unwrap_or_default());
                    ui.horizontal(|ui| {
                        ui.label(label);
                        ui.label(egui::RichText::new(key).small().weak());
                    })
                    .response
                    .on_hover_ui(tip);
                    ui.end_row();
                }
            });
    });
    // Outside the greyed block: it says who the boxes are for while they are off (Note JK1).
    ui.add(egui::Label::new(RECIPE_SCOPE).wrap());
    if settled && p != now {
        acts.push(Act::Recipe(p));
    }
}
